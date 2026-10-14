//! **Banco de las herramientas de documentos con el modelo de verdad**
//! (14-oct-2026): tabla escaneada → tabla del chat o Excel, anotar un PDF y
//! un plano, buscar en un plano, abrir por lo que dice el usuario y
//! exportar. Cada caso guarda su entrada y su resultado pintados en
//! `PIXPIN_IA_IMAGENES` (por omision `scratchpad\banco-imagenes`)
//! `\<motor>\<caso>-{entrada,resultado}.png` para calificarlos a ojo, y mide
//! el tiempo hasta lo primero visible (el primer pedido a la app: la tabla
//! abierta, el visor en su sitio) y el total.
//!
//! Corre sobre COPIAS de las muestras (`E:\pixpin-demo-om`,
//! `E:\pixpin-cad\muestras`) en un almacen de mentira en el scratchpad; los
//! pedidos a la app se apuntan, nunca se mandan.
//!
//! ```text
//! PIXPIN_IA_MOTOR=deepseek|claude   (clave en DEEPSEEK_API_KEY / ANTHROPIC_API_KEY)
//! PIXPIN_IA_SOLO=<texto>            (solo los casos que lo contienen)
//! cargo test -p pixpin -- --ignored --nocapture banco_de_documentos
//! ```

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use pixpin_ia::agente::{self, Ejecutor, Evento};
use pixpin_ia::herramientas::Llamada;
use pixpin_ia::motor::{Mensaje, Motor, Seguir};

use super::pruebas::almacen_con;
use super::*;

const SCRATCHPAD: &str = r"C:\Users\MaxBook\AppData\Local\Temp\claude\F--THE-FORGE-PIXPIN-PC-VERSION-MAX\afe461c5-d9fa-4287-b8b5-3ab7961f6cb3\scratchpad";

fn scratchpad() -> PathBuf {
    std::env::var("PIXPIN_IA_SCRATCH").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(SCRATCHPAD))
}

/// Las muestras, copiadas al scratchpad (nunca se abren las originales).
fn muestra(nombre: &str) -> PathBuf {
    let dir = scratchpad().join("banco-docs").join("muestras");
    let _ = std::fs::create_dir_all(&dir);
    let r = dir.join(nombre);
    if !r.is_file() {
        for origen in [r"E:\pixpin-demo-om", r"E:\pixpin-cad\muestras"] {
            let o = Path::new(origen).join(nombre);
            if o.is_file() {
                std::fs::copy(&o, &r).unwrap();
                break;
            }
        }
    }
    assert!(r.is_file(), "falta la muestra {nombre}");
    r
}

// ---------------------------------------------------------------------------
// Pintar a PNG lo que se califica

fn png(ruta: &Path, ancho: u32, alto: u32, rgba: &[u8]) {
    let img = pixpin_codec::ImagenRgba { ancho, alto, pixeles: rgba.to_vec() };
    std::fs::write(ruta, pixpin_codec::imagen::codificar_png(&img).unwrap()).unwrap();
}

/// Un lienzo de pixeles donde se pintan las figuras del asistente encima
/// de una hoja o un plano ya pintados.
struct Pixeles<'a> {
    w: i64,
    h: i64,
    px: &'a mut [u8],
}

impl Pixeles<'_> {
    fn mezclar(&mut self, x: i64, y: i64, c: (u8, u8, u8), a: f32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        for (k, v) in [c.0, c.1, c.2].into_iter().enumerate() {
            self.px[i + k] = (self.px[i + k] as f32 * (1.0 - a) + v as f32 * a) as u8;
        }
    }
    fn raya(&mut self, a: (f64, f64), b: (f64, f64), c: (u8, u8, u8), grosor: i64) {
        let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as usize).clamp(1, 20_000);
        for i in 0..=n {
            let t = i as f64 / n as f64;
            let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            for dx in -grosor / 2..=grosor / 2 {
                for dy in -grosor / 2..=grosor / 2 {
                    self.mezclar(x as i64 + dx, y as i64 + dy, c, 1.0);
                }
            }
        }
    }
    /// Las figuras, con `a_px` de las unidades de la capa a pixeles.
    fn figuras(&mut self, els: &[&Elemento], a_px: &dyn Fn(f32, f32) -> (f64, f64)) {
        for e in els {
            let c = |k: &ColorRgba| ((k.r * 255.0) as u8, (k.g * 255.0) as u8, (k.b * 255.0) as u8);
            let (p0, p1) = (a_px(e.x, e.y), a_px(e.x + e.ancho, e.y + e.alto));
            let (x0, y0, x1, y1) = (p0.0.min(p1.0), p0.1.min(p1.1), p0.0.max(p1.0), p0.1.max(p1.1));
            match &e.figura {
                Figura::Rectangulo | Figura::Marco { .. } => {
                    if let Some(r) = &e.relleno {
                        for y in y0 as i64..=y1 as i64 {
                            for x in x0 as i64..=x1 as i64 {
                                self.mezclar(x, y, c(r), e.opacidad.min(1.0) * 0.9);
                            }
                        }
                    }
                    if e.trazo.a > 0.1 {
                        let t = c(&e.trazo);
                        self.raya((x0, y0), (x1, y0), t, 3);
                        self.raya((x1, y0), (x1, y1), t, 3);
                        self.raya((x1, y1), (x0, y1), t, 3);
                        self.raya((x0, y1), (x0, y0), t, 3);
                    }
                }
                Figura::Elipse => {
                    let (cx, cy, rx, ry) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0, (y1 - y0) / 2.0);
                    let mut antes = (cx + rx, cy);
                    for k in 1..=96 {
                        let a = k as f64 / 96.0 * std::f64::consts::TAU;
                        let p = (cx + rx * a.cos(), cy + ry * a.sin());
                        self.raya(antes, p, c(&e.trazo), 3);
                        antes = p;
                    }
                }
                Figura::Linea { puntos } => {
                    for par in puntos.windows(2) {
                        self.raya(a_px(par[0].x, par[0].y), a_px(par[1].x, par[1].y), c(&e.trazo), 3);
                    }
                }
                // El texto de una nota no se escribe aqui: su papel amarillo
                // ya dice donde quedo (el texto se comprueba aparte).
                _ => {}
            }
        }
    }
}

/// La hoja `i` de un PDF a 1400 de ancho (las unidades de su capa) con lo
/// que el asistente le puso encima, si `con_capa`.
fn hoja_pintada(raiz: &Path, pdf: &Path, i: usize, con_capa: bool, salida: &Path) {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let doc = pixpin_pdf::Documento::abrir(pdf).unwrap();
    let mut img = doc.renderizar(i as u32, 1400).unwrap();
    if con_capa {
        let alto = img.alto as f32;
        let d = crate::lector_pdf_proyecto::DondeVa::de(raiz, pdf, 0);
        let capa = d.leer_capa(pdf, i, crate::anotado_del_adjunto::espacios_de_ahora(pdf), alto);
        let els: Vec<&Elemento> = capa.escena.elementos.iter().filter(|e| !e.borrado && e.grupos.iter().any(|g| g == GRUPO)).collect();
        let (w, h) = (img.ancho as i64, img.alto as i64);
        Pixeles { w, h, px: &mut img.pixeles }.figuras(&els, &|x, y| (x as f64, y as f64));
    }
    png(salida, img.ancho, img.alto, &img.pixeles);
}

/// El plano visto en `vista` (`[x, y, ancho]` del dibujo; `None`: entero)
/// con la capa del asistente encima.
fn plano_pintado(raiz: &Path, plano: &Path, vista: Option<[f64; 3]>, salida: &Path) {
    use pixpin_cad::pruebas_png::Lienzo;
    let m = crate::plano_cad::modelo(raiz, plano).unwrap();
    let (centro, ancho) = match vista {
        Some([x, y, a]) => ([x, y], a),
        None => {
            let c = m.caja_util();
            ([m.origen[0] + (c[0] + c[2]) as f64 / 2.0, m.origen[1] + (c[1] + c[3]) as f64 / 2.0], (c[2] - c[0]).max(c[3] - c[1]) as f64)
        }
    };
    let mut l = Lienzo::nuevo(&m, centro, ancho, 1000, 750);
    l.pintar(&m);
    let rutas = crate::plano_cad::anotar::rutas_de_la_capa(raiz, plano);
    let hoja = std::fs::read_to_string(&rutas.hoja).ok().and_then(|t| pixpin_sincro::anotado::MarcoDeLaHoja::de_texto(&t));
    if let Some(h) = hoja
        && let Some(encaje) = pixpin_cad::anotado::Encaje::de_la_hoja([h.x0 as f64, h.y0 as f64, h.x1 as f64, h.y1 as f64], m.caja)
    {
        let capa = crate::lector_tinta::Capa::leer(&rutas.tinta);
        let els: Vec<&Elemento> = capa.escena.elementos.iter().filter(|e| !e.borrado && e.grupos.iter().any(|g| g == GRUPO)).collect();
        let mut px: Vec<u8> = l.px.iter().flatten().copied().collect();
        {
            let a_px = |x: f32, y: f32| {
                let p = encaje.a_plano([x as f64, y as f64]);
                let q = l.a_pixel([p[0] as f32, p[1] as f32]);
                (q[0], q[1])
            };
            Pixeles { w: l.w as i64, h: l.h as i64, px: &mut px }.figuras(&els, &a_px);
        }
        png(salida, l.w as u32, l.h as u32, &px);
        return;
    }
    l.guardar(&salida.to_string_lossy());
}

/// Una tabla (filas de texto) pintada por el componente «Exportar» comun:
/// la rejilla de la hoja, con su texto.
fn tabla_pintada(filas: &[Vec<String>], cabecera: bool, salida: &Path) {
    let xlsx = escribir_excel("tabla", filas, cabecera).unwrap();
    excel_pintado(&xlsx, salida);
}

fn excel_pintado(xlsx: &Path, salida: &Path) {
    let dir = salida.with_extension("tmp");
    let _ = std::fs::remove_dir_all(&dir);
    let hechos = exportar_a(Path::new("."), xlsx, "png", &dir, pixpin_store::Idioma::Espanol).unwrap();
    std::fs::copy(&hechos[0], salida).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// El mundo del banco: un almacen con un chat de obra y sus archivos

struct Mundo {
    raiz: PathBuf,
    proyecto: String,
    adjuntos: Vec<Adjunto>,
    /// La foto de la tabla (la entrada de los casos de tabla).
    foto_tabla: PathBuf,
}

impl Mundo {
    fn adjunto(&self, nombre: &str) -> PathBuf {
        candidatos(&self.raiz).into_iter().find(|c| c.nombre == nombre).unwrap().ruta
    }
}

/// Las filas de la tabla de costos de la senalizacion (la «foto»).
fn costos() -> Vec<Vec<String>> {
    [
        ["Item", "Señal", "Ubicación", "Cant.", "P. unit. (S/)", "Parcial (S/)"],
        ["1", "R-1 Pare", "Jr. Antisuyo", "4", "185.50", "742.00"],
        ["2", "R-2 Ceda el paso", "Av. Tahuantinsuyo", "2", "172.00", "344.00"],
        ["3", "P-46 Zona escolar", "Jr. Antisuyo", "3", "210.00", "630.00"],
        ["4", "I-20 Nombre de calle", "Av. Florida", "6", "95.80", "574.80"],
        ["5", "R-30 Velocidad máx.", "Av. Tawa", "2", "185.50", "371.00"],
        ["", "", "", "", "Total", "2661.80"],
    ]
    .iter()
    .map(|f| f.iter().map(|c| c.to_string()).collect())
    .collect()
}

/// La «foto» de una tabla: una tabla de costos pintada por el componente
/// comun (texto de verdad y rejilla) y recortada sin las letras ni los
/// numeros de la hoja, como un papel: sin texto seleccionable, la tiene que
/// leer el OCR.
fn foto_de_tabla(dir: &Path) -> PathBuf {
    let anchos = [50u32, 170, 150, 60, 110, 110];
    let mut t = crate::pedidos::tabla_de_filas("costos", &costos(), true, 0);
    for (i, a) in anchos.iter().enumerate() {
        t.anchos.insert(((b'A' + i as u8) as char).to_string(), *a);
    }
    let bytes = pixpin_proyecto::tabla_xlsx::a_xlsx(&[pixpin_proyecto::tabla_xlsx::HojaParaVer { nombre: "costos", tabla: &t, valores: None }], 0).unwrap();
    let xlsx = dir.join("costos.xlsx");
    std::fs::write(&xlsx, bytes).unwrap();
    let entera = dir.join("costos-hoja.png");
    excel_pintado(&xlsx, &entera);
    // Sin la cabecera de la hoja (40 de ancho, una fila de 26 de alto).
    let img = pixpin_codec::imagen::cargar(&entera).unwrap();
    let escala = img.ancho as f32 / (40.0 + anchos.iter().sum::<u32>() as f32);
    let (x0, y0) = ((40.0 * escala).ceil() as u32 + 2, (26.0 * escala).ceil() as u32 + 2);
    let (w, h) = (img.ancho - x0, img.alto - y0);
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in y0..img.alto {
        let i = ((y * img.ancho + x0) * 4) as usize;
        px.extend_from_slice(&img.pixeles[i..i + (w * 4) as usize]);
    }
    let r = dir.join("tabla-costos.png");
    png(&r, w, h, &px);
    r
}

fn montar_mundo(motor: &str) -> Mundo {
    let dir = scratchpad().join("banco-docs").join(format!("mundo-{motor}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let foto = foto_de_tabla(&dir);
    let raiz = dir.join("PixPin Max");
    std::fs::create_dir_all(&raiz).unwrap();
    let leer = |n: &str| std::fs::read(muestra(n)).unwrap();
    let ids = almacen_con(
        &raiz,
        &[
            (
                "Obra Florida - Tawa",
                vec![
                    ("senal-vertical-tahuantinsuyo.dwg", leer("senal-vertical-tahuantinsuyo.dwg")),
                    ("senal-vertical-florida.dwg", leer("senal-vertical-florida.dwg")),
                    ("Tesis.pdf", leer("Tesis.pdf")),
                    ("tabla-costos.png", std::fs::read(&foto).unwrap()),
                ],
            ),
            ("Casa", vec![("presupuesto casa.xlsx", leer("mini.xlsx"))]),
        ],
    );
    let m = Mundo { raiz, proyecto: ids[0].clone(), adjuntos: Vec::new(), foto_tabla: foto };
    let ruta = m.adjunto("tabla-costos.png");
    Mundo { adjuntos: vec![Adjunto { nombre: "tabla-costos.png".into(), ruta }], ..m }
}

// ---------------------------------------------------------------------------
// Los casos

/// Lo que paso en un caso.
#[derive(Default)]
struct Hecho {
    llamadas: Vec<(String, Value)>,
    pedidos: Vec<Value>,
    texto: String,
    t_texto: Option<f32>,
    t_visible: Option<f32>,
    t_total: f32,
}

impl Hecho {
    fn pedido(&self, accion: &str) -> Option<&Value> {
        self.pedidos.iter().find(|p| p["accion"] == accion)
    }
    fn llamo(&self, h: &str) -> bool {
        self.llamadas.iter().any(|(n, _)| n == h)
    }
}

type Comprobar = fn(&Mundo, &Hecho) -> Result<(), String>;
type Pintar = fn(&Mundo, &Hecho, &Path, &Path);

struct Caso {
    nombre: &'static str,
    pedido: &'static str,
    /// Si el mensaje lleva la foto de la tabla (el modelo la ve).
    con_foto: bool,
    comprobar: Comprobar,
    pintar: Pintar,
}

fn si(c: bool, m: &str) -> Result<(), String> {
    if c { Ok(()) } else { Err(m.into()) }
}

fn ruta_de(p: &Value) -> PathBuf {
    PathBuf::from(p["que"]["ruta"].as_str().unwrap_or_default())
}

fn zona_de(p: &Value) -> Option<[f64; 3]> {
    let z = p["que"]["zona"].as_array()?;
    Some([z.first()?.as_f64()?, z.get(1)?.as_f64()?, z.get(2)?.as_f64()?])
}

/// La palabra de la Tesis que se pide resaltar (esta en su texto).
const PALABRA_TESIS: &str = "metodología";
/// Lo que se busca en el plano de Tahuantinsuyo.
const CALLE: &str = "JR. ANTISUYO";

fn casos() -> Vec<Caso> {
    vec![
        Caso {
            nombre: "tabla-escaneada-a-tabla",
            pedido: "Convierte esta tabla escaneada en una tabla del chat llamada Costos.",
            con_foto: true,
            comprobar: |_, h| {
                let p = h.pedido("tabla_nueva").ok_or("no creo la tabla del chat")?;
                let filas = p["filas"].as_array().map_or(0, Vec::len);
                si(filas >= 3, "la tabla tiene menos de 3 filas")?;
                si(p["nombre"].as_str().is_some_and(|n| n.to_lowercase().contains("costos")), "sin el nombre")
            },
            pintar: |m, h, entrada, resultado| {
                std::fs::copy(&m.foto_tabla, entrada).unwrap();
                if let Some(p) = h.pedido("tabla_nueva") {
                    let filas: Vec<Vec<String>> = serde_json::from_value(p["filas"].clone()).unwrap_or_default();
                    tabla_pintada(&filas, p["cabecera"] == true, resultado);
                }
            },
        },
        Caso {
            nombre: "tabla-escaneada-a-excel",
            pedido: "Pasa la tabla de esta foto a un Excel.",
            con_foto: true,
            comprobar: |_, h| {
                let p = h.pedido("chat").ok_or("no dejo el Excel en el chat")?;
                let xlsx = p["archivos"][0].as_str().unwrap_or_default();
                si(xlsx.ends_with(".xlsx") && Path::new(xlsx).is_file(), "sin el .xlsx")
            },
            pintar: |m, h, entrada, resultado| {
                std::fs::copy(&m.foto_tabla, entrada).unwrap();
                if let Some(x) = h.pedido("chat").and_then(|p| p["archivos"][0].as_str()) {
                    excel_pintado(Path::new(x), resultado);
                }
            },
        },
        Caso {
            nombre: "anotar-pdf",
            pedido: "En la Tesis, resalta en amarillo la palabra metodología.",
            con_foto: false,
            comprobar: |m, h| {
                si(h.llamo(pixpin_ia::acciones_documentos::ANOTAR), "no llamo a anotar_documento")?;
                let p = h.pedido("abrir").ok_or("no abrio la Tesis donde anoto")?;
                si(ruta_de(p).ends_with("Tesis.pdf"), "abrio otro")?;
                let i = p["que"]["pagina"].as_u64().ok_or("sin pagina")? as usize - 1;
                let pdf = m.adjunto("Tesis.pdf");
                let d = crate::lector_pdf_proyecto::DondeVa::de(&m.raiz, &pdf, 0);
                let capa = d.leer_capa(&pdf, i, 0, 1980.0);
                si(capa.escena.elementos.iter().any(|e| e.grupos.iter().any(|g| g == GRUPO) && e.relleno.is_some()), "la capa de la hoja no tiene el resaltado")
            },
            pintar: |m, h, entrada, resultado| {
                let pdf = m.adjunto("Tesis.pdf");
                let i = h.pedido("abrir").and_then(|p| p["que"]["pagina"].as_u64()).unwrap_or(1) as usize - 1;
                hoja_pintada(&m.raiz, &pdf, i, false, entrada);
                hoja_pintada(&m.raiz, &pdf, i, true, resultado);
            },
        },
        Caso {
            nombre: "buscar-en-plano",
            pedido: "¿Dónde dice JR. ANTISUYO en el plano de señalización de Tahuantinsuyo?",
            con_foto: false,
            comprobar: |_, h| {
                let p = h.pedido("abrir").ok_or("no llevo el visor alli")?;
                si(ruta_de(p).to_string_lossy().contains("tahuantinsuyo"), "otro plano")?;
                si(zona_de(p).is_some(), "sin la zona")
            },
            pintar: |m, h, entrada, resultado| {
                let plano = m.adjunto("senal-vertical-tahuantinsuyo.dwg");
                plano_pintado(&m.raiz, &plano, None, entrada);
                if let Some(z) = h.pedido("abrir").and_then(zona_de) {
                    plano_pintado(&m.raiz, &plano, Some(z), resultado);
                }
            },
        },
        Caso {
            nombre: "anotar-plano",
            pedido: "En el plano de señalización de Tahuantinsuyo encierra en rojo donde dice JR. ANTISUYO.",
            con_foto: false,
            comprobar: |m, h| {
                si(h.llamo(pixpin_ia::acciones_documentos::ANOTAR), "no llamo a anotar_documento")?;
                let plano = m.adjunto("senal-vertical-tahuantinsuyo.dwg");
                let capa = crate::lector_tinta::Capa::leer(&crate::plano_cad::anotar::rutas_de_la_capa(&m.raiz, &plano).tinta);
                si(capa.escena.elementos.iter().any(|e| matches!(e.figura, Figura::Elipse | Figura::Rectangulo) && e.grupos.iter().any(|g| g == GRUPO)), "la capa del plano no tiene nada del asistente")
            },
            pintar: |m, h, entrada, resultado| {
                let plano = m.adjunto("senal-vertical-tahuantinsuyo.dwg");
                let z = h.pedido("abrir").and_then(zona_de);
                plano_pintado(&m.raiz, &plano, None, entrada);
                plano_pintado(&m.raiz, &plano, z, resultado);
            },
        },
        Caso {
            nombre: "abrir-por-descripcion",
            pedido: "Abre el plano de señalización de Tahuantinsuyo.",
            con_foto: false,
            comprobar: |_, h| {
                let p = h.pedido("abrir").ok_or("no abrio nada")?;
                si(ruta_de(p).ends_with("senal-vertical-tahuantinsuyo.dwg"), "abrio otro")
            },
            pintar: |m, h, _entrada, resultado| {
                if let Some(p) = h.pedido("abrir") {
                    plano_pintado(&m.raiz, &ruta_de(p), None, resultado);
                }
            },
        },
        Caso {
            nombre: "abrir-ambiguo-pregunta",
            pedido: "Abre el plano de señalización vertical.",
            con_foto: false,
            comprobar: |_, h| {
                si(h.pedido("abrir").is_none(), "abrio uno sin preguntar")?;
                let t = normalizar(&h.texto);
                si(t.contains("tahuantinsuyo") && t.contains("florida"), "no pregunta con la lista")
            },
            pintar: |_, _, _, _| {},
        },
        Caso {
            nombre: "exportar-plano-pdf",
            pedido: "Exporta el plano de señalización de Florida a PDF.",
            con_foto: false,
            comprobar: |_, h| {
                let p = h.pedido("chat").ok_or("no dejo el PDF en el chat")?;
                let f = p["archivos"][0].as_str().unwrap_or_default();
                si(f.to_lowercase().ends_with(".pdf") && Path::new(f).is_file(), "sin el .pdf")
            },
            pintar: |m, h, entrada, resultado| {
                plano_pintado(&m.raiz, &m.adjunto("senal-vertical-florida.dwg"), None, entrada);
                if let Some(f) = h.pedido("chat").and_then(|p| p["archivos"][0].as_str()) {
                    let _com = pixpin_shell::ComDelHilo::iniciar();
                    let img = pixpin_pdf::Documento::abrir(Path::new(f)).unwrap().renderizar(0, 1400).unwrap();
                    png(resultado, img.ancho, img.alto, &img.pixeles);
                }
            },
        },
    ]
}

// ---------------------------------------------------------------------------
// El motor y el ejecutor del banco

fn motor() -> (Box<dyn Motor>, String) {
    use pixpin_ia::nube::{MotorNube, Proveedor};
    let eleccion = std::env::var("PIXPIN_IA_MOTOR").unwrap_or_else(|_| "deepseek".into()).to_lowercase();
    if eleccion == "local" {
        return (Box::new(motor_local()), eleccion);
    }
    let p = Proveedor::por_id(&eleccion).unwrap_or_else(|| panic!("motor «{eleccion}»: claude, deepseek o local"));
    let var = match p {
        Proveedor::Claude => "ANTHROPIC_API_KEY",
        Proveedor::DeepSeek => "DEEPSEEK_API_KEY",
    };
    let clave = std::env::var(var).unwrap_or_else(|_| panic!("sin {var}"));
    (Box::new(MotorNube::nuevo(p, clave)), eleccion)
}

/// El modelo local (Qwen3.5 2B con llama.cpp), donde lo deja la app; otro
/// con `PIXPIN_IA_MODELO`.
fn motor_local() -> pixpin_ia::servidor::MotorLlama {
    use pixpin_ia::servidor::{Config, MotorLlama, elegir_dispositivo, listar_dispositivos};
    let base = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap()).join("PixPinMax").join("modelos");
    let modelo = std::env::var("PIXPIN_IA_MODELO")
        .map(PathBuf::from)
        .unwrap_or_else(|_| base.join("qwen3.5-2b").join("Qwen3.5-2B-Q4_K_M.gguf"));
    let vision = modelo.parent().map(|d| d.join("mmproj-F16.gguf")).filter(|p| p.exists());
    let servidor = base.join("llama.cpp").join("b11580").join("llama-server.exe");
    let mut config = Config::nueva(servidor.clone(), modelo.clone(), vision);
    let bytes = std::fs::metadata(&modelo).map(|m| m.len()).unwrap_or(0);
    config.dispositivo = elegir_dispositivo(&listar_dispositivos(&servidor), bytes);
    MotorLlama::nuevo(config)
}

/// Hace de verdad las herramientas de documentos (sobre las copias) y
/// apunta lo demas; los pedidos se apuntan con su hora.
struct EjBanco<'a> {
    mundo: &'a Mundo,
    mandar: Mandar,
    llamadas: Vec<(String, Value)>,
}

impl Ejecutor for EjBanco<'_> {
    fn ejecutar(&mut self, llamada: &Llamada) -> Result<String, String> {
        let parar = AtomicBool::new(false);
        match llamada {
            Llamada::App(a) => {
                self.llamadas.push((a.herramienta.to_string(), a.pedido.clone()));
                if pixpin_ia::acciones_documentos::es_de_documentos(a.herramienta) {
                    let cx = Contexto {
                        raiz: &self.mundo.raiz,
                        proyecto: &self.mundo.proyecto,
                        adjuntos: &self.mundo.adjuntos,
                        mandar: &self.mandar,
                        parar: &parar,
                        idioma: pixpin_store::Idioma::Espanol,
                    };
                    hacer(a, &cx)
                } else {
                    (self.mandar)(&a.pedido.to_string());
                    Ok("hecho".into())
                }
            }
            otra => {
                self.llamadas.push((otra.nombre().to_string(), Value::Null));
                Ok("hecho (el banco no lo hace)".into())
            }
        }
    }
}

fn correr(motor: &dyn Motor, m: &Mundo, caso: &Caso) -> Hecho {
    let mut historial = vec![Mensaje::sistema(agente::instrucciones())];
    historial.push(if caso.con_foto {
        Mensaje::usuario_con_imagen(caso.pedido, std::fs::read(&m.foto_tabla).unwrap(), "image/png")
    } else {
        Mensaje::usuario(caso.pedido)
    });
    // Como en la app: la nube recibe el catalogo entero.
    let herramientas = if motor.catalogo_completo() {
        pixpin_ia::herramientas::catalogo()
    } else {
        super::super::acciones::herramientas_para(&historial, &m.adjuntos, caso.con_foto)
    };
    let t0 = Instant::now();
    let mandados: Arc<Mutex<Vec<(f32, Value)>>> = Arc::default();
    let m2 = mandados.clone();
    let mandar: Mandar = Box::new(move |j| {
        m2.lock().unwrap().push((t0.elapsed().as_secs_f32(), serde_json::from_str(j).unwrap_or(Value::Null)));
        1
    });
    let mut ej = EjBanco { mundo: m, mandar, llamadas: Vec::new() };
    let mut t_texto = None;
    let fin = agente::turno(motor, &mut historial, &herramientas, &mut ej, &mut |e| {
        if matches!(e, Evento::Texto(_)) && t_texto.is_none() {
            t_texto = Some(t0.elapsed().as_secs_f32());
        }
        if let Evento::Resultado { nombre, ok, texto } = &e {
            println!("    {nombre} -> {} {}", if *ok { "ok" } else { "ERR" }, texto.chars().take(300).collect::<String>());
        }
        Seguir::Si
    });
    let pedidos = mandados.lock().unwrap().clone();
    Hecho {
        llamadas: ej.llamadas,
        t_visible: pedidos.first().map(|p| p.0),
        pedidos: pedidos.into_iter().map(|p| p.1).collect(),
        texto: fin.map(|f| f.texto).unwrap_or_else(|e| format!("ERROR DEL MOTOR: {e}")),
        t_texto,
        t_total: t0.elapsed().as_secs_f32(),
    }
}

#[test]
#[ignore = "necesita la clave de un motor de la nube y las muestras de E: (ver la cabecera)"]
fn banco_de_documentos() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let (motor, nombre) = motor();
    let mundo = montar_mundo(&nombre);
    let solo = std::env::var("PIXPIN_IA_SOLO").ok();
    let carpeta = std::env::var("PIXPIN_IA_IMAGENES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| scratchpad().join("banco-imagenes"))
        .join(&nombre);
    std::fs::create_dir_all(&carpeta).unwrap();
    let (mut bien, mut total) = (0, 0);
    let mut informe = Vec::new();
    for caso in casos() {
        if solo.as_deref().is_some_and(|s| !caso.nombre.contains(s)) {
            continue;
        }
        total += 1;
        println!("--- {}: «{}»", caso.nombre, caso.pedido);
        let h = correr(motor.as_ref(), &mundo, &caso);
        let v = (caso.comprobar)(&mundo, &h);
        bien += usize::from(v.is_ok());
        let linea = format!(
            "{} {:<26} visible {:>5} s  total {:>5.1} s  {}{}",
            if v.is_ok() { "BIEN" } else { "MAL " },
            caso.nombre,
            h.t_visible.or(h.t_texto).map(|t| format!("{t:.1}")).unwrap_or("-".into()),
            h.t_total,
            h.llamadas.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(","),
            v.err().map(|e| format!("  <- {e}")).unwrap_or_default()
        );
        println!("{linea}\n    texto: {}", h.texto.trim().chars().take(400).collect::<String>());
        informe.push(format!("{linea}\n    pedido: {}\n    texto: {}", caso.pedido, h.texto.trim()));
        let (entrada, resultado) = (carpeta.join(format!("{}-entrada.png", caso.nombre)), carpeta.join(format!("{}-resultado.png", caso.nombre)));
        (caso.pintar)(&mundo, &h, &entrada, &resultado);
        std::fs::write(carpeta.join(format!("{}-respuesta.txt", caso.nombre)), format!("{}\n\n{}", caso.pedido, h.texto)).unwrap();
    }
    let nota = format!("BANCO DOCUMENTOS ({nombre}): {bien}/{total} = {:.0} %", bien as f64 * 100.0 / total.max(1) as f64);
    println!("\n{nota}");
    informe.push(nota);
    std::fs::write(carpeta.join("documentos-informe.txt"), informe.join("\n")).unwrap();
    motor.soltar();
}

/// **Sin modelo**: las herramientas llamadas a mano sobre las muestras, con
/// sus PNG en `banco-imagenes\referencia` y sus tiempos (lo que tarda cada
/// una sin red: la parte de PixPin de la latencia).
#[test]
#[ignore = "lee las muestras de E: y escribe PNG en el scratchpad"]
fn muestras_de_documentos() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let m = montar_mundo("referencia");
    let dir = scratchpad().join("banco-imagenes").join("referencia");
    std::fs::create_dir_all(&dir).unwrap();
    let parar = AtomicBool::new(false);
    let t = Instant::now();
    let tabla = leer_tabla(&m.foto_tabla, None).unwrap();
    println!("tabla de la foto en {:?}: {} x {}\n{}", t.elapsed(), tabla.filas.len(), tabla.columnas(), tabla.a_tsv());
    std::fs::copy(&m.foto_tabla, dir.join("documentos-tabla-entrada.png")).unwrap();
    tabla_pintada(&tabla.filas, tabla.parece_cabecera(), &dir.join("documentos-tabla-resultado.png"));
    let plano = m.adjunto("senal-vertical-tahuantinsuyo.dwg");
    let t = Instant::now();
    let textos = crate::plano_cad::textos::del_plano(&m.raiz, &plano).unwrap();
    println!("{} textos del plano en {:?}", textos.len(), t.elapsed());
    let t = Instant::now();
    let otra_vez = crate::plano_cad::textos::del_plano(&m.raiz, &plano).unwrap();
    println!("otra vez (cache) en {:?}", t.elapsed());
    assert_eq!(otra_vez.len(), textos.len());
    for h in crate::plano_cad::textos::buscar(&textos, "antisuyo", 8) {
        println!("  {h:?}");
    }
    let e = buscar_dentro(&m.raiz, &plano, CALLE, &parar);
    println!("buscar «{CALLE}»: {:?}", e.as_ref().map(|e| &e.texto));
    let a = anotar_en(&m.raiz, &plano, &json!({"tipo": "encerrar", "texto": CALLE}), &parar).unwrap();
    plano_pintado(&m.raiz, &plano, a.vista, &dir.join("documentos-plano-encerrar.png"));
    let pdf = m.adjunto("Tesis.pdf");
    let t = Instant::now();
    let a = anotar_en(&m.raiz, &pdf, &json!({"tipo": "resaltar", "texto": PALABRA_TESIS}), &parar).unwrap();
    println!("resaltar en la Tesis en {:?}: {a:?}", t.elapsed());
    hoja_pintada(&m.raiz, &pdf, a.pagina.unwrap(), true, &dir.join("documentos-tesis-resaltar.png"));
    let t = Instant::now();
    let hechos = exportar_a(&m.raiz, &m.adjunto("senal-vertical-florida.dwg"), "pdf", &scratchpad().join("banco-docs").join("export"), pixpin_store::Idioma::Espanol).unwrap();
    println!("exportar el plano a PDF en {:?}: {hechos:?}", t.elapsed());
}
