//! **El banco de los ojos** (15-oct-2026): los mismos pedidos con la nube de
//! verdad, sin ojos y con ojos, medidos y pintados a PNG.
//!
//! Cuatro casos donde ver deberia ayudar:
//!
//! - `etiquetas`: «pon una etiqueta debajo de cada figura» sobre un lienzo
//!   ya lleno (lo dibujado cae centrado en la vista: sin ver, las etiquetas
//!   no caen bajo sus figuras).
//! - `solapes`: «corrige lo que se solapa» (textos que se salen de su caja o
//!   se pisan).
//! - `flechas`: «une las cajas con flechas en orden» en zigzag.
//! - `plano`: un plano en **imagen** (sin texto en el resumen) y «encierra
//!   la cocina con un circulo rojo»: sin ver, el modelo no sabe donde esta.
//!
//! Cada caso tiene su nota (0 a 1) calculada sobre la escena final, y se
//! apuntan los segundos y cuantas vistas hubo. Las claves se leen en memoria
//! (`ANTHROPIC_API_KEY`/`DEEPSEEK_API_KEY` o las de Ajustes con DPAPI) y no
//! se imprimen nunca.
//!
//! ```text
//! PIXPIN_OJOS_SALIDA=<carpeta> [PIXPIN_OJOS_MOTOR=claude|deepseek] [PIXPIN_OJOS_CASO=plano]
//! cargo test -p pixpin --bin pixpinmax -- --ignored --nocapture --test-threads=1 asistente::ojos::banco
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pixpin_ia::Evento;
use pixpin_ia::nube::{MotorNube, Proveedor};
use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::{Elemento, Figura};

use super::super::al_lienzo;
use super::super::chat::EnVivo;
use super::super::turno::{Aviso, Turno};
use super::pruebas::escena_de;

type Caja = (f32, f32, f32, f32);

/// Un caso: el lienzo de partida, el pedido y como se puntua.
struct Caso {
    nombre: &'static str,
    pedido: &'static str,
    escena: Escena,
    /// Las fotos del lienzo (el plano).
    fotos: Vec<(u64, pixpin_codec::ImagenRgba)>,
    nota: fn(&Escena, &[u64]) -> f32,
}

fn centro(c: Caja) -> (f32, f32) {
    ((c.0 + c.2) / 2.0, (c.1 + c.3) / 2.0)
}

fn dentro(p: (f32, f32), c: Caja) -> bool {
    p.0 >= c.0 && p.0 <= c.2 && p.1 >= c.1 && p.1 <= c.3
}

fn se_pisan(a: Caja, b: Caja) -> bool {
    a.0 < b.2 - 2.0 && b.0 < a.2 - 2.0 && a.1 < b.3 - 2.0 && b.1 < a.3 - 2.0
}

fn es_texto(e: &Elemento) -> bool {
    matches!(e.figura, Figura::Texto { .. })
}

fn texto_de(e: &Elemento) -> String {
    match &e.figura {
        Figura::Texto { texto, .. } => pixpin_ia::acciones::normalizar(texto),
        _ => String::new(),
    }
}

// --- etiquetas -------------------------------------------------------------

const FIGURAS: &str = r#"{"elementos":[
  {"tipo":"rectangulo","caja":[60,60,170,110],"color":"azul","relleno":"azul"},
  {"tipo":"elipse","caja":[320,40,150,150],"color":"rojo"},
  {"tipo":"triangulo","caja":[560,60,170,140],"color":"verde","relleno":"verde"},
  {"tipo":"estrella","caja":[300,330,170,170],"color":"naranja","relleno":"naranja"}]}"#;

/// Cada figura con una etiqueta debajo (su centro en el ancho de la figura,
/// y arriba entre su borde de abajo y 90 mas abajo) que no pisa otra figura.
fn nota_etiquetas(e: &Escena, originales: &[u64]) -> f32 {
    let figuras: Vec<Caja> = e
        .visibles()
        .filter(|x| originales.contains(&x.id))
        .map(Elemento::caja)
        .collect();
    let textos: Vec<Caja> = e
        .visibles()
        .filter(|x| es_texto(x) && !originales.contains(&x.id))
        .map(Elemento::caja)
        .collect();
    let bien = figuras
        .iter()
        .filter(|f| {
            textos.iter().any(|t| {
                let (cx, _) = centro(*t);
                cx >= f.0
                    && cx <= f.2
                    && t.1 >= f.3 - 15.0
                    && t.1 <= f.3 + 90.0
                    && !figuras.iter().any(|g| se_pisan(*t, *g))
            })
        })
        .count();
    bien as f32 / figuras.len().max(1) as f32
}

// --- solapes ---------------------------------------------------------------

const SOLAPES: &str = r#"{"elementos":[
  {"tipo":"rectangulo","caja":[40,200,190,90],"color":"azul"},
  {"tipo":"rectangulo","caja":[300,200,190,90],"color":"azul"},
  {"tipo":"rectangulo","caja":[560,200,190,90],"color":"azul"},
  {"tipo":"texto","x":50,"y":230,"texto":"Recepcion de pedidos del cliente","tam_letra":26},
  {"tipo":"texto","x":330,"y":225,"texto":"Almacen","tam_letra":24},
  {"tipo":"texto","x":320,"y":240,"texto":"Control de calidad","tam_letra":24},
  {"tipo":"texto","x":575,"y":230,"texto":"Envio","tam_letra":24}]}"#;

/// Problemas que se ven: textos que se pisan y textos que se salen de la
/// caja en la que tienen el centro.
fn problemas_de_solapes(e: &Escena) -> usize {
    let cajas: Vec<Caja> = e
        .visibles()
        .filter(|x| matches!(x.figura, Figura::Rectangulo))
        .map(Elemento::caja)
        .collect();
    let textos: Vec<Caja> = e
        .visibles()
        .filter(|x| es_texto(x))
        .map(caja_pintada)
        .collect();
    let mut n = 0;
    for (i, a) in textos.iter().enumerate() {
        n += textos[i + 1..].iter().filter(|b| se_pisan(*a, **b)).count();
        // Cada texto, entero dentro de alguna caja.
        if !cajas
            .iter()
            .any(|c| a.0 >= c.0 - 4.0 && a.2 <= c.2 + 4.0 && a.1 >= c.1 - 4.0 && a.3 <= c.3 + 4.0)
        {
            n += 1;
        }
    }
    n
}

fn nota_solapes(e: &Escena, _: &[u64]) -> f32 {
    let antes = problemas_de_solapes(&escena_de(SOLAPES)).max(1);
    // Que no se pierda ningun texto por el camino (borrar no es arreglar).
    let textos = e.visibles().filter(|x| es_texto(x)).count();
    if textos < 4 {
        return 0.0;
    }
    (1.0 - problemas_de_solapes(e) as f32 / antes as f32).max(0.0)
}

// --- flechas ---------------------------------------------------------------

const ZIGZAG: &str = r#"{"elementos":[
  {"tipo":"rectangulo","caja":[60,60,160,70],"color":"verde","texto":"Inicio"},
  {"tipo":"rectangulo","caja":[520,230,160,70],"color":"azul","texto":"Proceso"},
  {"tipo":"rectangulo","caja":[120,430,160,70],"color":"rojo","texto":"Fin"}]}"#;

fn caja_con_texto(e: &Escena, t: &str) -> Option<Caja> {
    let txt = e.visibles().find(|x| texto_de(x) == t)?.caja();
    e.visibles()
        .filter(|x| matches!(x.figura, Figura::Rectangulo))
        .map(Elemento::caja)
        .find(|c| dentro(centro(txt), *c))
}

/// Una flecha por par (Inicio→Proceso, Proceso→Fin) que sale del borde de
/// la primera y llega al de la segunda (a 25 o menos).
fn nota_flechas(e: &Escena, _: &[u64]) -> f32 {
    let (Some(a), Some(b), Some(c)) = (
        caja_con_texto(e, "inicio"),
        caja_con_texto(e, "proceso"),
        caja_con_texto(e, "fin"),
    ) else {
        return 0.0;
    };
    let cerca = |p: pixpin_motor2d::Punto2, k: Caja| {
        dentro((p.x, p.y), (k.0 - 25.0, k.1 - 25.0, k.2 + 25.0, k.3 + 25.0))
    };
    let flechas: Vec<Vec<pixpin_motor2d::Punto2>> = e
        .visibles()
        .filter(|x| matches!(x.figura, Figura::Flecha { .. }))
        .filter_map(|x| x.puntos().map(|p| p.to_vec()))
        .filter(|p| p.len() >= 2)
        .collect();
    let une = |de: Caja, a: Caja| {
        flechas
            .iter()
            .any(|p| cerca(p[0], de) && cerca(p[p.len() - 1], a))
    };
    (u8::from(une(a, b)) + u8::from(une(b, c))) as f32 / 2.0
}

// --- plano -----------------------------------------------------------------

/// Las habitaciones del plano (en la imagen de 800x560 que se pega en 0,0).
const HABITACIONES: &[(&str, Caja)] = &[
    ("Sala", (20.0, 20.0, 420.0, 300.0)),
    ("Cocina", (420.0, 20.0, 780.0, 240.0)),
    ("Dormitorio", (20.0, 300.0, 380.0, 540.0)),
    ("Baño", (580.0, 240.0, 780.0, 400.0)),
    ("Pasillo", (380.0, 300.0, 580.0, 540.0)),
];
const ID_PLANO: u64 = 0x0001_2345_6789;

/// El plano en pixeles: paredes y nombres pintados con el lienzo y leidos
/// como imagen (el modelo no tiene sus textos en el resumen).
fn plano() -> pixpin_codec::ImagenRgba {
    let mut els = Vec::new();
    for (n, (x0, y0, x1, y1)) in HABITACIONES {
        els.push(format!(
            r#"{{"tipo":"rectangulo","caja":[{x0},{y0},{},{}],"color":"negro","grosor":4}}"#,
            x1 - x0,
            y1 - y0
        ));
        els.push(format!(
            r#"{{"tipo":"texto","x":{},"y":{},"texto":"{n}","tam_letra":22,"color":"gris"}}"#,
            x0 + 14.0,
            y0 + 12.0
        ));
    }
    // Unos muebles para que no sea solo texto.
    els.push(r#"{"tipo":"rectangulo","caja":[600,60,150,40],"color":"gris"}"#.into());
    els.push(r#"{"tipo":"elipse","caja":[640,300,60,50],"color":"gris"}"#.into());
    els.push(r#"{"tipo":"rectangulo","caja":[60,380,160,120],"color":"gris"}"#.into());
    let escena = escena_de(&format!(r#"{{"elementos":[{}]}}"#, els.join(",")));
    let enc = super::copiar(&escena, (0.0, 0.0, 800.0, 560.0), (0.0, 0.0), false, |_| {
        None
    });
    let v = super::pintar(enc).expect("el plano se pinta");
    leer_png(&v.png)
}

fn leer_png(png: &[u8]) -> pixpin_codec::ImagenRgba {
    let r = std::env::temp_dir().join(format!("pixpin-banco-ojos-{}.png", std::process::id()));
    std::fs::write(&r, png).unwrap();
    let img = pixpin_codec::imagen::cargar(&r).unwrap();
    let _ = std::fs::remove_file(&r);
    img
}

/// Un trazo nuevo rojo (elipse, rectangulo o a mano) cuyo centro cae en la
/// cocina y que no es mas grande que dos habitaciones.
fn nota_plano(e: &Escena, originales: &[u64]) -> f32 {
    let (_, cocina) = HABITACIONES[1];
    // La imagen esta en el lienzo donde quedo: se mide relativo a ella.
    let Some(img) = e
        .visibles()
        .find(|x| matches!(x.figura, Figura::Imagen { .. }))
        .map(Elemento::caja)
    else {
        return 0.0;
    };
    let sx = (img.2 - img.0) / 800.0;
    let sy = (img.3 - img.1) / 560.0;
    let cocina = (
        img.0 + cocina.0 * sx,
        img.1 + cocina.1 * sy,
        img.0 + cocina.2 * sx,
        img.1 + cocina.3 * sy,
    );
    let ok = e
        .visibles()
        .filter(|x| !originales.contains(&x.id) && !es_texto(x))
        .any(|x| {
            let c = x.caja();
            let rojo = x.trazo.r > 0.6 && x.trazo.g < 0.45;
            rojo && dentro(centro(c), cocina) && (c.2 - c.0) < 1.6 * (cocina.2 - cocina.0)
        });
    f32::from(u8::from(ok))
}

fn casos(solo: &str) -> Vec<Caso> {
    let mut v = vec![
        Caso {
            nombre: "etiquetas",
            pedido: "Pon una etiqueta de texto con el nombre de cada figura justo debajo de ella.",
            escena: escena_de(FIGURAS),
            fotos: vec![],
            nota: nota_etiquetas,
        },
        Caso {
            nombre: "solapes",
            pedido: "Corrige lo que se solapa en el lienzo: que ningún texto quede encima de otro ni se salga de su caja.",
            escena: escena_de(SOLAPES),
            fotos: vec![],
            nota: nota_solapes,
        },
        Caso {
            nombre: "flechas",
            pedido: "Une las cajas del lienzo con flechas en orden: Inicio, luego Proceso, luego Fin.",
            escena: escena_de(ZIGZAG),
            fotos: vec![],
            nota: nota_flechas,
        },
    ];
    let mut escena = Escena::nueva();
    escena.anadir(crate::imagenes_lienzo::elemento_imagen(
        ID_PLANO, 0.0, 20.0, 800.0, 560.0,
    ));
    v.push(Caso {
        nombre: "plano",
        pedido: "En el plano del lienzo, encierra la cocina con un círculo rojo.",
        escena,
        fotos: vec![(ID_PLANO, plano())],
        nota: nota_plano,
    });
    v.retain(|c| solo.is_empty() || solo.contains(c.nombre));
    v
}

fn motores(solo: &str) -> Vec<(&'static str, Arc<dyn pixpin_ia::Motor>)> {
    let clave = |var: &str, p: Proveedor| {
        std::env::var(var)
            .ok()
            .filter(|k| !k.trim().is_empty())
            .or_else(|| pixpin_ia::nube::claves::leer(p))
    };
    let mut v: Vec<(&'static str, Arc<dyn pixpin_ia::Motor>)> = Vec::new();
    if (solo.is_empty() || solo.contains("claude"))
        && let Some(k) = clave("ANTHROPIC_API_KEY", Proveedor::Claude)
    {
        v.push(("claude", Arc::new(MotorNube::nuevo(Proveedor::Claude, k))));
    }
    if (solo.is_empty() || solo.contains("deepseek"))
        && let Some(k) = clave("DEEPSEEK_API_KEY", Proveedor::DeepSeek)
    {
        v.push((
            "deepseek",
            Arc::new(MotorNube::nuevo(Proveedor::DeepSeek, k)),
        ));
    }
    v
}

/// Lo que salio de una pasada.
struct Pasada {
    nota: f32,
    segundos: f32,
    primero: Option<f32>,
    vistas: usize,
    texto: String,
    fallo: Option<String>,
}

/// **Una pasada**: el lienzo del caso abierto en un «editor» de mentira (la
/// cola de `al_lienzo` atendida aqui), el turno de verdad y la nota.
fn pasar(
    motor: Arc<dyn pixpin_ia::Motor>,
    caso: &Caso,
    ojos: bool,
    png: &std::path::Path,
) -> Pasada {
    super::super::turno::apagar_ojos(!ojos);
    al_lienzo::vaciar_para_pruebas();
    let editor = al_lienzo::EditorAbierto::nuevo(1);
    let mut escena = caso.escena.clone();
    let originales: Vec<u64> = escena.visibles().map(|e| e.id).collect();
    let mut imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(4096);
    for (id, img) in &caso.fotos {
        imagenes.guardar_con_id(*id, img.clone());
    }
    let historial = super::super::turno::historial_para_el_modelo(&[], caso.pedido, None);
    let t0 = Instant::now();
    let mut t = Turno::lanzar_hacia(motor, "banco-ojos".into(), historial, vec![], None, None);
    let mut vivo = EnVivo::default();
    let mut vistas_pedidas = 0;
    let pintadas_antes = super::PINTADAS.load(std::sync::atomic::Ordering::SeqCst);
    let mut primero = None;
    let vista = (0.0, 0.0, 800.0, 600.0);
    let hasta = t0 + Duration::from_secs(400);
    while Instant::now() < hasta && !t.acabado() {
        if al_lienzo::atender_en_editor_con(&mut escena, vista, &[], &mut imagenes)
            && primero.is_none()
        {
            primero = Some(t0.elapsed().as_secs_f32());
        }
        for a in t.recibir() {
            match &a {
                Aviso::Evento(e) => {
                    if let Evento::Resultado { nombre, ok, texto } = e {
                        if nombre == pixpin_ia::ojos::NOMBRE {
                            vistas_pedidas += 1;
                        }
                        println!(
                            "    [{:>5.1} s] {nombre} {} {}",
                            t0.elapsed().as_secs_f32(),
                            if *ok { "ok" } else { "MAL" },
                            texto.lines().next().unwrap_or("")
                        );
                    }
                    vivo.aplicar(e);
                }
                Aviso::Fin { resultado } => vivo.acabar(resultado),
                Aviso::Confirmar { responder, .. } => {
                    let _ = responder.send(false);
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // Lo que quede en la cola (juntar pasos al final).
    al_lienzo::atender_en_editor_con(&mut escena, vista, &[], &mut imagenes);
    drop(editor);
    let segundos = t0.elapsed().as_secs_f32();
    let fallo = (!t.acabado())
        .then(|| "no acabo a tiempo".to_string())
        .or_else(|| vivo.fallos.first().cloned());
    // Todas las vistas pintadas (las pedidas, la del principio y las de
    // la revision).
    let vistas = super::PINTADAS.load(std::sync::atomic::Ordering::SeqCst) - pintadas_antes;
    let _ = vistas_pedidas;
    // El resultado, pintado sin marcas.
    let caja = escena
        .caja()
        .map(|c| (c.0 - 20.0, c.1 - 20.0, c.2 + 20.0, c.3 + 20.0))
        .unwrap_or((0.0, 0.0, 800.0, 600.0));
    let enc = super::copiar(&escena, caja, (0.0, 0.0), false, |id| {
        imagenes.rgba(id).cloned().map(super::Pixeles::Rgba)
    });
    if let Ok(v) = super::pintar(enc) {
        let _ = std::fs::write(png, &v.png);
    }
    super::super::turno::apagar_ojos(false);
    Pasada {
        nota: (caso.nota)(&escena, &originales),
        segundos,
        primero,
        vistas,
        texto: vivo.texto.clone(),
        fallo,
    }
}

#[test]
#[ignore = "banco con la nube de verdad"]
fn banco() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let base = std::env::var_os("PIXPIN_OJOS_SALIDA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("banco-ojos"));
    let solo_motor = std::env::var("PIXPIN_OJOS_MOTOR").unwrap_or_default();
    let solo_caso = std::env::var("PIXPIN_OJOS_CASO").unwrap_or_default();
    let motores = motores(&solo_motor);
    assert!(!motores.is_empty(), "no hay ninguna clave de la nube");
    let mut tabla = vec!["motor\tcaso\tojos\tnota\tsegundos\tprimero_s\tvistas\tfallo".to_string()];
    for (nombre, motor) in motores {
        let carpeta = base.join(nombre);
        std::fs::create_dir_all(&carpeta).unwrap();
        for caso in casos(&solo_caso) {
            for ojos in [false, true] {
                let modo = if ojos { "con-ojos" } else { "sin-ojos" };
                let png = carpeta.join(format!("{}-{modo}.png", caso.nombre));
                println!("== {nombre} {} {modo}", caso.nombre);
                let p = pasar(motor.clone(), &caso, ojos, &png);
                // Sin claves en lo que se imprime (por si un error las cita).
                let fallo = p.fallo.map(|f| tapar(&f)).unwrap_or_default();
                println!(
                    "   nota {:.2}  {:.1} s (primero en el lienzo {:?} s)  vistas {}  {}\n   «{}»",
                    p.nota,
                    p.segundos,
                    p.primero,
                    p.vistas,
                    fallo,
                    tapar(&p.texto).replace('\n', " ")
                );
                tabla.push(format!(
                    "{nombre}\t{}\t{modo}\t{:.2}\t{:.1}\t{}\t{}\t{fallo}",
                    caso.nombre,
                    p.nota,
                    p.segundos,
                    p.primero.map(|s| format!("{s:.1}")).unwrap_or_default(),
                    p.vistas
                ));
                if fallo.contains("401") || fallo.to_lowercase().contains("clave") {
                    println!("   la clave de {nombre} no vale: se sigue con la otra");
                    break;
                }
            }
        }
    }
    let t = tabla.join("\n");
    println!("\n{t}");
    let _ = std::fs::write(
        base.join(format!(
            "tabla-{}-{}.tsv",
            if solo_motor.is_empty() {
                "todos"
            } else {
                &solo_motor
            },
            if solo_caso.is_empty() {
                "todos"
            } else {
                &solo_caso
            }
        )),
        t,
    );
}

/// Tapa cualquier cosa con forma de clave (`sk-…`).
fn tapar(s: &str) -> String {
    s.split(' ')
        .map(|p| {
            if p.contains("sk-") {
                "sk-…".to_string()
            } else {
                p.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Lo que ocupa un texto pintado, medido a lo ancho de sus letras (la caja
/// del elemento puede quedarse corta con letras grandes): ~0,55 del tamano
/// por letra, 1,25 por linea.
fn caja_pintada(e: &Elemento) -> Caja {
    let Figura::Texto { texto, tam, .. } = &e.figura else {
        return e.caja();
    };
    let (x0, y0, x1, y1) = e.caja();
    let lineas: Vec<&str> = texto.lines().collect();
    // Partido por su ancho (mas lineas de alto que saltos): su caja vale.
    if ((y1 - y0) / (tam * 1.25)).round() as usize > lineas.len().max(1) {
        return e.caja();
    }
    let letras = lineas.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32;
    let ancho = (letras * tam * 0.55).max(x1 - x0);
    let alto = (lineas.len().max(1) as f32 * tam * 1.25).max(y1 - y0);
    (x0, y0, x0 + ancho, y0 + alto)
}
