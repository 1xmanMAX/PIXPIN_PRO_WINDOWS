//! **Banco del asistente con documentos de todo tipo** (15-oct-2026, «muchas
//! pruebas del asistente en la nube con documentos de todo tipo, revisar que
//! salga bien y mejorar las herramientas a las que les falte algo»).
//!
//! 16 documentos hechos aqui ([`corpus`]: PDF, PNG, Word, PowerPoint y Excel;
//! solo texto, parrafos con titulos y listas, formulas, tablas, fotos, dos
//! columnas, una grafica, un diagrama de flujo, geometria y una hoja
//! escaneada torcida con ruido) y ~45 tareas de usuario sobre ellos: calcar al
//! lienzo, resumir, tabla a Excel, anotar lo importante, OCR, preguntas y
//! formulas a LaTeX.
//!
//! **Por el mismo camino que la app**: el documento entra al chat como
//! adjunto (almacen de mentira en el scratchpad), el turno se lanza con
//! `Turno::lanzar_con_memoria` y `pensado::preparar_turno` como al pulsar
//! Enviar, con el motor de `nube::motor` (la clave de Ajustes, leida EN
//! MEMORIA, nunca impresa), el catalogo entero de la nube, un lienzo
//! «abierto» que atiende la cola (`al_lienzo::atender_en_editor_con`,
//! con `Trabajo::Imagen` para «calca esto» con la imagen pegada en el
//! lienzo) y los pedidos a «PixPin» apuntados (nunca salen).
//!
//! **A ojo**: de cada caso, `scratchpad\banco-documentos\<motor>\<caso>.png`
//! con el original a la izquierda y el resultado a la derecha (lo dibujado en
//! el lienzo, la pagina con lo anotado, el Excel que salio o la respuesta),
//! y arriba el pedido, el veredicto, las herramientas y los tiempos (1º
//! visible y total).
//!
//! ```text
//! cargo test -p pixpin -- --ignored --nocapture --exact asistente::banco_documentos::generar_corpus
//! PIXPIN_IA_MOTOR=claude|deepseek  PIXPIN_IA_SOLO=<trozo|trozo>  PIXPIN_BANCO_A_LA_VEZ=7
//! cargo test -p pixpin -- --ignored --nocapture --exact asistente::banco_documentos::banco
//! ```
//!
//! `banco` lanza cada caso en su proceso (el mismo ejecutable con
//! `un_caso`), de 6 a 8 a la vez: la cola del lienzo y lo mandado son de
//! todo el proceso.

mod corpus;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use pixpin_ia::agente::Evento;
use pixpin_ia::motor::Motor;
use pixpin_ia::nube::Proveedor;
use pixpin_motor2d::elemento::Figura;
use serde_json::{Value, json};

use super::al_lienzo;
use super::turno::{Adjunto, Aviso, Turno};

const SCRATCHPAD: &str = r"C:\Users\MaxBook\AppData\Local\Temp\claude\F--THE-FORGE-PIXPIN-PC-VERSION-MAX\afe461c5-d9fa-4287-b8b5-3ab7961f6cb3\scratchpad";

fn base() -> PathBuf {
    std::env::var("PIXPIN_IA_SCRATCH").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(SCRATCHPAD)).join("banco-documentos")
}

/// Como llega el documento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entrada {
    /// Adjunto al chat (un archivo).
    Adjunto,
    /// Una foto mandada con el pedido (el modelo la ve).
    Foto,
    /// La imagen pegada en el lienzo abierto, nada adjunto
    /// (`Trabajo::Imagen`).
    EnLienzo,
}

/// Lo que paso en un caso.
#[derive(Default)]
struct Hecho {
    texto: String,
    /// (herramienta, sus argumentos o su detalle).
    llamadas: Vec<(String, String)>,
    /// (herramienta, ok, lo que contesto).
    resultados: Vec<(String, bool, String)>,
    pedidos: Vec<Value>,
    leido: Vec<String>,
    /// Lo que se dibujo en el lienzo (lo visible al acabar, sin la imagen
    /// de entrada).
    nuevos: usize,
    textos_lienzo: String,
    figuras: Vec<String>,
    anotado: Option<(PathBuf, String, Vec<(usize, PathBuf)>)>,
    t_primero: f32,
    t_total: f32,
    fallo_motor: bool,
}

impl Hecho {
    fn llamo(&self, h: &str) -> bool {
        self.llamadas.iter().any(|(n, _)| n == h)
    }
    fn n(&self) -> String {
        pixpin_ia::acciones::normalizar(&self.texto)
    }
    fn dice(&self, cosas: &[&str]) -> Result<(), String> {
        let t = self.n().replace(['.', ','], "");
        for c in cosas {
            let alternativas: Vec<String> = c.split('|').map(|x| pixpin_ia::acciones::normalizar(x).replace(['.', ','], "")).collect();
            if !alternativas.iter().any(|a| t.contains(a.as_str())) {
                return Err(format!("la respuesta no dice «{c}»"));
            }
        }
        Ok(())
    }
    fn excel(&self) -> Option<PathBuf> {
        self.pedidos
            .iter()
            .filter(|p| p["accion"] == "chat")
            .flat_map(|p| p["archivos"].as_array().cloned().unwrap_or_default())
            .filter_map(|a| a.as_str().map(PathBuf::from))
            .find(|a| a.extension().is_some_and(|e| e == "xlsx") && a.is_file())
    }
    fn anoto(&self) -> Result<(), String> {
        match &self.anotado {
            Some((_, _, capas)) if !capas.is_empty() => Ok(()),
            Some(_) => Err("anotar_inteligente no dejo nada en la capa".into()),
            None => Err("no anoto (sin anotar_inteligente)".into()),
        }
    }
    fn lienzo(&self, minimo: usize, textos: &[&str]) -> Result<(), String> {
        if self.nuevos < minimo {
            return Err(format!("en el lienzo hay {} elementos nuevos (minimo {minimo})", self.nuevos));
        }
        let t = pixpin_ia::acciones::normalizar(&self.textos_lienzo);
        for x in textos {
            if !x.split('|').any(|a| t.contains(&pixpin_ia::acciones::normalizar(a))) {
                return Err(format!("en el lienzo no esta el texto «{x}»"));
            }
        }
        Ok(())
    }
}

type Comprobar = fn(&Hecho) -> Result<(), String>;

struct Caso {
    nombre: &'static str,
    /// El documento (nombre en `corpus::DOCS`).
    doc: &'static str,
    entrada: Entrada,
    tarea: &'static str,
    pedido: &'static str,
    comprobar: Comprobar,
    /// Pasa por `hoja_a_lienzo`/`diagrama` (lo de otro agente).
    de_geometria: bool,
}

const fn c(nombre: &'static str, doc: &'static str, entrada: Entrada, tarea: &'static str, pedido: &'static str, comprobar: Comprobar) -> Caso {
    Caso { nombre, doc, entrada, tarea, pedido, comprobar, de_geometria: false }
}

const fn g(nombre: &'static str, doc: &'static str, entrada: Entrada, tarea: &'static str, pedido: &'static str, comprobar: Comprobar) -> Caso {
    Caso { nombre, doc, entrada, tarea, pedido, comprobar, de_geometria: true }
}

fn filas_del_excel(x: &Path) -> Vec<String> {
    super::en_el_chat::celdas_de_excel(x, None, None).unwrap_or_default().lines().skip(2).map(str::to_string).collect()
}

/// El Excel lleva el fondo azul claro de la cabecera de la foto (un relleno
/// solido de color, no el gris de siempre).
fn excel_con_fondo(h: &Hecho) -> Result<(), String> {
    let x = h.excel().ok_or("no dejo ningun .xlsx en el chat")?;
    let mut p = pixpin_docs::Paquete::abrir(&x).map_err(|e| e.to_string())?;
    let estilos = p.texto("xl/styles.xml", 1 << 20).unwrap_or_default();
    let colores: Vec<&str> = estilos.match_indices("rgb=\"").map(|(i, _)| &estilos[i + 5..(i + 13).min(estilos.len())]).collect();
    let azulado = colores.iter().any(|c| {
        let v = |k: usize| u8::from_str_radix(c.get(k..k + 2).unwrap_or("00"), 16).unwrap_or(0) as i32;
        let (r, b) = (v(2), v(6));
        b > r + 10 && b > 150 && r > 120
    });
    if azulado { Ok(()) } else { Err(format!("el Excel no lleva el fondo de la cabecera (colores {colores:?})")) }
}

fn excel_con(h: &Hecho, minimo: usize, contiene: &[&str]) -> Result<(), String> {
    let x = h.excel().ok_or("no dejo ningun .xlsx en el chat")?;
    let filas = filas_del_excel(&x);
    if filas.len() < minimo {
        return Err(format!("el Excel tiene {} filas (minimo {minimo})", filas.len()));
    }
    let todo = pixpin_ia::acciones::normalizar(&filas.join("\n"));
    for c in contiene {
        if !todo.contains(&pixpin_ia::acciones::normalizar(c)) {
            return Err(format!("el Excel no tiene «{c}»"));
        }
    }
    Ok(())
}

use Entrada::{Adjunto as A, EnLienzo as L, Foto as F};

fn casos() -> Vec<Caso> {
    vec![
        // 01: solo texto (PDF)
        c("01-resumir", "01-comunicado-reunion.pdf", A, "resumir", "Resúmeme este comunicado en tres puntos.", |h| h.dice(&["22", "losa|acero", "antisuyo|desague|desagüe"])),
        c("01-pregunta", "01-comunicado-reunion.pdf", A, "pregunta", "¿Qué día, a qué hora y dónde es la reunión del comunicado?", |h| h.dice(&["22", "9", "florida"])),
        // 02: parrafos con titulos y listas (Word)
        c("02-resumir", "02-informe-drenaje.docx", A, "resumir", "Hazme un resumen del informe de drenaje que te mandé.", |h| h.dice(&["cuneta", "sumidero", "184"])),
        c("02-pregunta", "02-informe-drenaje.docx", A, "pregunta", "Según el informe de drenaje, ¿cuánto cuesta la obra y en cuántos días se hace?", |h| h.dice(&["184", "45"])),
        c("02-anotar", "02-informe-drenaje.docx", A, "anotar", "En el informe de drenaje, señálame lo importante.", |h| h.anoto()),
        // 03: formulas (PDF)
        c("03-latex", "03-formulario-fisica.pdf", A, "formulas", "Pásame en LaTeX todas las fórmulas de este formulario.", |h| {
            h.dice(&["\\frac", "\\sqrt", "\\int"])?;
            h.dice(&["\\pi", "\\theta|\\sin"])
        }),
        g("03-calcar", "03-formulario-fisica.pdf", A, "calcar", "Pasa esta página del formulario al lienzo.", |h| h.lienzo(8, &["cinematica"])),
        c("03-ocr", "03-formulario-fisica.pdf", A, "ocr", "Lee el texto de este PDF.", |h| {
            if h.leido.is_empty() && !h.n().contains("cinematica") {
                return Err("ni OCR ni el texto en la respuesta".into());
            }
            Ok(())
        }),
        // 04: formulas en imagen
        c("04-ecuaciones", "04-pizarra-formulas.png", F, "formulas", "Escribe en el lienzo las fórmulas de esta foto como ecuaciones.", |h| {
            if h.nuevos < 4 {
                return Err(format!("solo {} elementos en el lienzo", h.nuevos));
            }
            Ok(())
        }),
        c("04-ocr", "04-pizarra-formulas.png", F, "ocr", "¿Qué dice esta pizarra? Cópiame el texto.", |h| h.dice(&["ohm", "bernoulli", "pitagoras"])),
        c("04-latex", "04-pizarra-formulas.png", F, "formulas", "Convierte las fórmulas de esta foto a LaTeX.", |h| h.dice(&["\\frac", "\\rho", "\\sqrt", "\\sigma"])),
        // 05: tabla en Excel
        c("05-pregunta", "05-presupuesto-obra.xlsx", A, "pregunta", "¿Cuál es la partida más cara del presupuesto de obra y cuánto cuesta?", |h| h.dice(&["tuberia|pvc", "60800|60 800"])),
        c("05-grafica", "05-presupuesto-obra.xlsx", A, "grafica", "Haz en el lienzo una gráfica de barras con los parciales del presupuesto de obra.", |h| {
            if !h.llamo("grafica") {
                return Err("no llamo a grafica".into());
            }
            h.lienzo(6, &[])
        }),
        // 06: tabla en PDF
        c("06-excel", "06-notas-curso.pdf", A, "tabla", "Pasa la tabla de notas de este PDF a un Excel.", |h| {
            excel_con(h, 9, &["Mendoza", "Ticona"])?;
            excel_con_fondo(h)?;
            let n = filas_del_excel(&h.excel().unwrap()).len();
            if n > 10 { return Err(format!("el Excel lleva {n} filas: se colo el texto de la pagina")); }
            Ok(())
        }),
        c("06-pregunta", "06-notas-curso.pdf", A, "pregunta", "En el registro de notas, ¿quién tiene el promedio más alto y quiénes desaprobaron?", |h| h.dice(&["mendoza|elena", "flores|jorge", "laura|pedro"])),
        // 07: tabla en foto
        c("07-excel", "07-stock-materiales.png", F, "tabla", "Extrae la tabla de esta foto a un Excel.", |h| excel_con(h, 7, &["Yura", "Magollo"]).and_then(|_| excel_con_fondo(h))),
        c("07-tabla-lienzo", "07-stock-materiales.png", F, "calcar", "Haz esta tabla en el lienzo.", |h| {
            h.lienzo(6, &["yura", "cemento"])?;
            if !h.resultados.iter().any(|(n, ok, t)| n == "tabla_de_imagen" && *ok && t.contains("como en la foto")) {
                return Err("no la paso con su aspecto medido (tabla_de_imagen al lienzo)".into());
            }
            Ok(())
        }),
        // 08: texto con fotos (Word)
        c("08-resumir", "08-articulo-con-fotos.docx", A, "resumir", "Resume el artículo de Tacna.", |h| h.dice(&["olivo|aceituna", "malecon|playa"])),
        c("08-anotar", "08-articulo-con-fotos.docx", A, "anotar", "En el artículo de Tacna, resume cada sección al margen.", |h| h.anoto()),
        // 09: dos columnas con foto (PDF)
        c("09-resumir", "09-boletin-dos-columnas.pdf", A, "resumir", "¿Qué dice el boletín de obra? Resúmelo.", |h| h.dice(&["68", "120"])),
        g("09-calcar", "09-boletin-dos-columnas.pdf", A, "calcar", "Calca esta página del boletín en el lienzo.", |h| h.lienzo(10, &["boletin", "seguridad"])),
        // 10: una grafica
        c("10-pregunta", "10-grafica-produccion.png", F, "pregunta", "¿En qué mes hubo más producción y cuánto fue?", |h| h.dice(&["abril|abr", "160"])),
        c("10-rehacer", "10-grafica-produccion.png", F, "calcar", "Rehaz esta gráfica en el lienzo con los mismos datos.", |h| {
            if !h.llamo("grafica") {
                return Err("no la rehizo con grafica".into());
            }
            let args: String = h.llamadas.iter().filter(|(n, _)| n == "grafica").map(|(_, a)| a.as_str()).collect();
            for v in ["120", "95", "140", "160", "130"] {
                if !args.contains(v) {
                    return Err(format!("a la grafica le falta el dato {v}"));
                }
            }
            Ok(())
        }),
        g("10-calca-esto", "10-grafica-produccion.png", L, "calcar", "Calca esto en el lienzo.", |h| h.lienzo(5, &["160|abr"])),
        // 11: diagrama de flujo
        g("11-calcar", "11-diagrama-flujo.png", F, "calcar", "Calca este diagrama de flujo en el lienzo.", |h| {
            h.lienzo(8, &["inicio", "fin", "impar", "leer"])?;
            if !h.figuras.iter().any(|f| f == "rombo") {
                return Err("sin el rombo de la decision".into());
            }
            Ok(())
        }),
        g("11-calca-esto", "11-diagrama-flujo.png", L, "calcar", "Pasa esto al lienzo con figuras.", |h| h.lienzo(8, &["inicio", "impar"])),
        c("11-explicar", "11-diagrama-flujo.png", F, "pregunta", "Explícame qué hace este diagrama.", |h| h.dice(&["par", "impar", "2|dos|residuo|resto|mod"])),
        // 12: geometria
        g("12-calcar", "12-geometria-circunferencia.png", F, "calcar", "Calca esta figura de geometría en el lienzo.", |h| h.lienzo(6, &["a", "b", "c", "o", "35"])),
        c("12-resolver", "12-geometria-circunferencia.png", F, "pregunta", "Resuelve el problema de la figura.", |h| h.dice(&["55"])),
        c("12-anotar", "12-geometria-circunferencia.png", A, "anotar", "Anota en la imagen de geometría los pasos para hallar x.", |h| h.anoto()),
        // 13: hoja escaneada
        c("13-ocr", "13-guia-escaneada.png", F, "ocr", "Lee el texto de esta hoja escaneada.", |h| {
            let todo = pixpin_ia::acciones::normalizar(&format!("{} {}", h.texto, h.leido.join(" ")));
            for x in ["compresion", "probeta", "210"] {
                if !todo.contains(x) {
                    return Err(format!("no leyo «{x}»"));
                }
            }
            Ok(())
        }),
        c("13-excel", "13-guia-escaneada.png", F, "tabla", "Pasa la tabla de resultados de esta hoja escaneada a un Excel.", |h| {
            excel_con(h, 5, &["P-03", "246"])?;
            excel_con_fondo(h)?;
            let n = filas_del_excel(&h.excel().unwrap()).len();
            if n > 6 { return Err(format!("el Excel lleva {n} filas: se colo el texto de la hoja")); }
            Ok(())
        }),
        c("13-pregunta", "13-guia-escaneada.png", F, "pregunta", "Según esta guía escaneada, ¿qué resistencia media dieron las probetas a 28 días?", |h| h.dice(&["242"])),
        c("13-resumir", "13-guia-escaneada.png", F, "resumir", "Resume esta hoja escaneada.", |h| h.dice(&["compresion", "28"])),
        // 14: PowerPoint
        c("14-resumir", "14-charla-seguridad.pptx", A, "resumir", "Resume la presentación de seguridad en obra.", |h| h.dice(&["casco", "arnes", "12"])),
        c("14-anotar", "14-charla-seguridad.pptx", A, "anotar", "En la presentación de seguridad, señala lo importante de la diapositiva 2.", |h| h.anoto()),
        // 15: Excel con un error
        c("15-errores", "15-metrados-con-error.xlsx", A, "anotar", "Revisa el Excel de metrados y marca los errores de cálculo.", |h| {
            h.anoto()?;
            h.dice(&["tarrajeo|3420|3 420"])
        }),
        c("15-pregunta", "15-metrados-con-error.xlsx", A, "pregunta", "¿Cuánto debería sumar el total del Excel de metrados si se corrigen los errores?", |h| h.dice(&["24600|24 600"])),
        // 16: examen mixto (PDF)
        c("16-pasos", "16-examen-mixto.pdf", A, "anotar", "En el examen, explícame paso a paso el problema 1 al margen.", |h| h.anoto()),
        c("16-limpio", "16-examen-mixto.pdf", A, "calcar", "Pasa en limpio al lienzo el problema 1 del examen.", |h| h.lienzo(3, &[])),
        c("16-resolver", "16-examen-mixto.pdf", A, "pregunta", "Resuelve el problema 1 del examen.", |h| h.dice(&["44|4,4|4.4"])),
        c("16-latex", "16-examen-mixto.pdf", A, "formulas", "Pásame las fórmulas del examen en LaTeX.", |h| h.dice(&["\\frac", "\\sin", "\\mu"])),
    ]
}

// ---------------------------------------------------------------------------
// El mundo de un caso y su turno

/// Lo que se ve primero en la burbuja.
fn se_ve(e: &Evento) -> bool {
    match e {
        Evento::Texto(t) | Evento::Pensando(t) => !t.trim().is_empty(),
        Evento::Reinicio { .. } | Evento::Reintento { .. } => false,
        _ => true,
    }
}

fn motor_de(p: Proveedor) -> Result<(Arc<dyn Motor>, Arc<dyn Motor>), String> {
    // La clave de Ajustes, en memoria (nunca se imprime).
    let clave = pixpin_ia::nube::claves::leer(p).ok_or_else(|| format!("sin clave de {}", p.nombre()))?;
    let vision: Arc<dyn Motor> = Arc::new(pixpin_ia::nube::MotorNube::nuevo(p, clave).ligero());
    let pensar = false;
    let motor = super::nube::motor(p, None, pensar)?;
    Ok((motor, vision))
}

fn proveedor() -> Proveedor {
    let m = std::env::var("PIXPIN_IA_MOTOR").unwrap_or_else(|_| "claude".into());
    Proveedor::por_id(&m.to_lowercase()).unwrap_or(Proveedor::Claude)
}

/// **Un caso**, de punta a punta, en este proceso.
fn correr(caso: &Caso, p: Proveedor) -> Result<(Hecho, PathBuf, pixpin_motor2d::Escena, crate::imagenes_lienzo::ImagenesLienzo, usize), String> {
    use pixpin_proyecto::{almacen, cuaderno};
    let (motor, vision) = motor_de(p)?;
    let dir = base().join("mundos").join(p.id()).join(caso.nombre);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let raiz = dir.join("PixPin Max");
    std::fs::create_dir_all(&raiz).unwrap();
    let doc = corpus::ruta(caso.doc);
    let bytes = std::fs::read(&doc).map_err(|e| format!("falta {} (genera el corpus): {e}", caso.doc))?;
    let adjuntar = caso.entrada != Entrada::EnLienzo;
    let ids = super::documentos_acciones::pruebas::almacen_con(&raiz, &[("Documentos", if adjuntar { vec![(caso.doc, bytes)] } else { vec![] })]);
    let proyecto = ids[0].clone();
    super::documentos::poner_ubicacion(pixpin_store::Ubicacion::Portable { raiz: raiz.clone() });
    let en_el_chat = super::documentos_acciones::candidatos(&raiz).into_iter().find(|c| c.nombre == caso.doc).map(|c| c.ruta);
    // Como `chat::adjuntos`: solo lo que el OCR lee (fotos y PDF).
    let adjuntos: Vec<Adjunto> = en_el_chat
        .iter()
        .filter(|r| super::ocr::es_pdf(r) || super::ocr::es_imagen(r))
        .map(|r| Adjunto { nombre: caso.doc.to_string(), ruta: r.clone() })
        .collect();
    let foto = if caso.entrada == Entrada::Foto { en_el_chat.clone() } else { None };
    // El lienzo abierto, con la imagen pegada si el caso es «calca esto».
    al_lienzo::vaciar_para_pruebas();
    let editor = al_lienzo::EditorAbierto::nuevo(1);
    let mut escena = pixpin_motor2d::Escena::nueva();
    let mut imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(16_384);
    if caso.entrada == Entrada::EnLienzo {
        let img = pixpin_codec::imagen::cargar(&doc).map_err(|e| e.to_string())?;
        let (w, h) = (img.ancho as f32, img.alto as f32);
        let id = imagenes.guardar(img).ok_or("no se guardo la imagen")?;
        let k = 640.0 / w;
        escena.anadir(crate::imagenes_lienzo::elemento_imagen(id, -300.0, -260.0, w * k, h * k));
    }
    let antes = escena.cuantos_visibles();
    super::acciones::RESPUESTA_EN_PRUEBAS.store(pixpin_shell::mensajero::respuesta::ACEPTADO, std::sync::atomic::Ordering::Relaxed);
    super::acciones::MANDADOS_EN_PRUEBAS.lock().unwrap().clear();
    *super::turno::VISION_EN_PRUEBAS.lock().unwrap() = Some(vision);
    *super::anotar_inteligente::ULTIMO_EN_PRUEBAS.lock().unwrap() = None;
    // La pregunta, en el chat (como al pulsar Enviar) y el turno por
    // `pensado::preparar_turno`.
    let carpeta = almacen::carpeta(&raiz, &proyecto);
    let sello = cuaderno::Sello { cuando: 5_000, numero: 50, aparato: "PC01".into(), proyecto: proyecto.clone() };
    cuaderno::anadir(&carpeta, &cuaderno::Mensaje::nota(caso.pedido, &sello)).unwrap();
    let mensajes = cuaderno::Cuaderno::leer_de(&carpeta).unwrap().mensajes;
    let anteriores = &mensajes[..mensajes.len() - 1];
    let herramientas = pixpin_ia::herramientas::catalogo();
    let pensar = super::pensado::pensar(None, caso.pedido);
    let (historial, memoria) = super::pensado::preparar_turno(&raiz, &proyecto, anteriores, caso.pedido, pensar, super::pensado::VENTANA_NUBE, &herramientas);
    let t0 = Instant::now();
    let mut t = Turno::lanzar_con_memoria(motor.clone(), proyecto.clone(), historial, adjuntos, foto, None, herramientas, Some(memoria), Some(raiz.clone()));
    let mut h = Hecho::default();
    let mut primero = None;
    let vista = (-400.0, -300.0, 400.0, 300.0);
    let hasta = t0 + Duration::from_secs(600);
    let mut vivo = super::chat::EnVivo::default();
    while Instant::now() < hasta {
        al_lienzo::atender_en_editor_con(&mut escena, vista, &[], &mut imagenes);
        for a in t.recibir() {
            match a {
                Aviso::Evento(e) => {
                    if primero.is_none() && se_ve(&e) {
                        primero = Some(t0.elapsed().as_secs_f32());
                    }
                    match &e {
                        Evento::Herramienta { llamada, .. } => {
                            let detalle = match llamada {
                                pixpin_ia::herramientas::Llamada::App(a) => a.pedido.to_string(),
                                pixpin_ia::herramientas::Llamada::Editar(x) => Value::Object(x.args.clone()).to_string(),
                                otra => format!("{otra:?}"),
                            };
                            h.llamadas.push((llamada.nombre().to_string(), detalle));
                        }
                        Evento::Resultado { nombre, ok, texto } => h.resultados.push((nombre.clone(), *ok, texto.clone())),
                        Evento::Reintento { motivo } => h.resultados.push(("(reintento)".into(), false, motivo.clone())),
                        _ => {}
                    }
                    vivo.aplicar(&e);
                }
                Aviso::Fin { resultado } => {
                    match &resultado {
                        Ok(f) => h.texto = f.texto.clone(),
                        Err(e) => {
                            h.texto = format!("ERROR: {e}");
                            h.fallo_motor = true;
                        }
                    }
                    vivo.acabar(&resultado);
                }
                Aviso::Confirmar { responder, .. } => {
                    let _ = responder.send(true);
                }
                Aviso::TextoLeido { texto, .. } => h.leido.push(texto),
                Aviso::Registro(_) | Aviso::Citas(_) => {}
            }
        }
        if t.acabado() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    if !t.acabado() {
        t.parar();
        h.texto = format!("COLGADO a los 600 s: {}", vivo.mostrar());
        h.fallo_motor = true;
    }
    if h.texto.trim().is_empty() {
        h.texto = vivo.mostrar();
    }
    drop(editor);
    h.t_total = t0.elapsed().as_secs_f32();
    h.t_primero = primero.unwrap_or(h.t_total);
    h.pedidos = super::acciones::MANDADOS_EN_PRUEBAS.lock().unwrap().iter().filter_map(|j| serde_json::from_str(j).ok()).collect();
    h.anotado = super::anotar_inteligente::ULTIMO_EN_PRUEBAS.lock().unwrap().take();
    let vivos: Vec<&pixpin_motor2d::Elemento> = escena.visibles().collect();
    h.nuevos = vivos.len().saturating_sub(antes);
    h.textos_lienzo = vivos
        .iter()
        .filter_map(|e| match &e.figura {
            Figura::Texto { texto, .. } => Some(texto.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" | ");
    h.figuras = vivos
        .iter()
        .map(|e| match &e.figura {
            Figura::Rombo => "rombo".to_string(),
            Figura::Elipse => "elipse".into(),
            Figura::Rectangulo => "rectangulo".into(),
            Figura::Texto { .. } => "texto".into(),
            Figura::Imagen { .. } => "imagen".into(),
            Figura::Flecha { .. } => "flecha".into(),
            Figura::Linea { .. } => "linea".into(),
            otra => format!("{:?}", std::mem::discriminant(otra)),
        })
        .collect();
    Ok((h, raiz, escena, imagenes, antes))
}

// ---------------------------------------------------------------------------
// A PNG: original | resultado

fn escalar(img: pixpin_codec::ImagenRgba, max_w: u32, max_h: u32) -> pixpin_codec::ImagenRgba {
    let k = (max_w as f32 / img.ancho as f32).min(max_h as f32 / img.alto as f32).min(1.0);
    if k >= 0.999 {
        return img;
    }
    let (w, h) = (((img.ancho as f32 * k) as u32).max(1), ((img.alto as f32 * k) as u32).max(1));
    pixpin_codec::imagen::redimensionar(img, w, h).unwrap()
}

fn lado_a_lado(a: &pixpin_codec::ImagenRgba, b: &pixpin_codec::ImagenRgba) -> pixpin_codec::ImagenRgba {
    let sep = 24u32;
    let w = a.ancho + sep + b.ancho;
    let h = a.alto.max(b.alto);
    let mut px = vec![200u8; (w * h * 4) as usize];
    let mut poner = |img: &pixpin_codec::ImagenRgba, ox: u32| {
        for y in 0..img.alto {
            let d = ((y * w + ox) * 4) as usize;
            let o = (y * img.ancho * 4) as usize;
            px[d..d + (img.ancho * 4) as usize].copy_from_slice(&img.pixeles[o..o + (img.ancho * 4) as usize]);
        }
    };
    poner(a, 0);
    poner(b, a.ancho + sep);
    pixpin_codec::ImagenRgba { ancho: w, alto: h, pixeles: px }
}

fn encima(a: &pixpin_codec::ImagenRgba, b: &pixpin_codec::ImagenRgba) -> pixpin_codec::ImagenRgba {
    let w = a.ancho.max(b.ancho);
    let h = a.alto + 12 + b.alto;
    let mut px = vec![255u8; (w * h * 4) as usize];
    let mut poner = |img: &pixpin_codec::ImagenRgba, oy: u32| {
        for y in 0..img.alto {
            let d = ((oy + y) * w * 4) as usize;
            let o = (y * img.ancho * 4) as usize;
            px[d..d + (img.ancho * 4) as usize].copy_from_slice(&img.pixeles[o..o + (img.ancho * 4) as usize]);
        }
    };
    poner(a, 0);
    poner(b, a.alto + 12);
    pixpin_codec::ImagenRgba { ancho: w, alto: h, pixeles: px }
}

/// Un texto largo en una tarjeta blanca (la respuesta del chat).
fn tarjeta(titulo: &str, texto: &str, ancho: f32) -> pixpin_codec::ImagenRgba {
    let mut hoja = corpus::Hoja::nueva(ancho, 2000.0);
    let mut y = 16.0;
    y += hoja.texto(titulo, 16.0, y, 15.0, true) + 8.0;
    for renglon in texto.lines().take(70) {
        if renglon.trim().is_empty() {
            y += 8.0;
            continue;
        }
        y = hoja.parrafo(renglon, 16.0, y, ancho - 32.0, 13.0);
    }
    hoja.h = (y + 20.0).min(2000.0);
    corpus::hoja_a_png(hoja, (ancho * 1.25) as u32)
}

fn escena_a_png(escena: &pixpin_motor2d::Escena, imagenes: &crate::imagenes_lienzo::ImagenesLienzo, ancho: u32) -> Option<pixpin_codec::ImagenRgba> {
    let mut hojas = pixpin_motor2d::exportar::hojas(escena, pixpin_motor2d::exportar::Alcance::Todo, &[], None);
    if hojas.is_empty() {
        return None;
    }
    hojas.truncate(1);
    let c = hojas[0].caja;
    let c = (c.0 - 20.0, c.1 - 20.0, c.2 + 20.0, c.3 + 20.0);
    hojas[0].caja = c;
    let de = |id: u64| imagenes.rgba(id).map(|i| pixpin_pdf::escribir::Pixeles { ancho: i.ancho, alto: i.alto, rgba: i.pixeles.clone() });
    let pdf = pixpin_pdf::escribir::de_hojas_a_medida(&hojas, &[(c.2 - c.0, c.3 - c.1)], &de, super::hoja_a_lienzo::letra_segoe())?;
    let tmp = std::env::temp_dir().join(format!("pixpin-banco-docs-{}.pdf", std::process::id()));
    std::fs::write(&tmp, pdf).ok()?;
    let img = pixpin_pdf::Documento::abrir(&tmp).and_then(|d| d.renderizar(0, ancho)).ok();
    let _ = std::fs::remove_file(&tmp);
    img
}

/// El documento a PNG: el PDF o la foto tal cual, y el Word, el PowerPoint
/// o el Excel por el componente «Exportar» comun.
fn original(raiz: &Path, doc: &Path, dir: &Path) -> Option<pixpin_codec::ImagenRgba> {
    let nombre = pixpin_docs::nombre(doc);
    if let Some(i) = corpus::vista_previa(&nombre, 1100) {
        return Some(i);
    }
    let tmp = dir.join("exportado");
    let _ = std::fs::remove_dir_all(&tmp);
    let hechos = super::documentos_acciones::exportar_a(raiz, doc, "png", &tmp, pixpin_store::Idioma::Espanol).ok()?;
    pixpin_codec::imagen::cargar(hechos.first()?).ok()
}

/// **El PNG del caso**: franja con pedido y tiempos; original | resultado.
fn pintar_caso(caso: &Caso, motor: &str, h: &Hecho, veredicto: &Result<(), String>, raiz: &Path, escena: &pixpin_motor2d::Escena, imagenes: &crate::imagenes_lienzo::ImagenesLienzo) -> PathBuf {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let dir = base().join(motor);
    std::fs::create_dir_all(&dir).unwrap();
    let doc = super::documentos_acciones::candidatos(raiz).into_iter().find(|c| c.nombre == caso.doc).map(|c| c.ruta).unwrap_or_else(|| corpus::ruta(caso.doc));
    let izq = original(raiz, &doc, &dir).map(|i| escalar(i, 1000, 1300)).unwrap_or_else(|| tarjeta("(sin vista previa)", caso.doc, 500.0));
    // El resultado: lo anotado, el Excel, el lienzo o la respuesta.
    let mut der = None;
    if let Some((ruta, grupo, capas)) = &h.anotado
        && !capas.is_empty()
    {
        let tmp = dir.join("anotado");
        let _ = std::fs::create_dir_all(&tmp);
        let pngs = super::anotar_inteligente::banco::pintar_lo_anotado(raiz, ruta, grupo, capas, &tmp, caso.nombre);
        der = pngs.first().and_then(|p| pixpin_codec::imagen::cargar(p).ok());
    }
    if der.is_none()
        && let Some(x) = h.excel()
    {
        let tmp = dir.join("excel");
        let _ = std::fs::remove_dir_all(&tmp);
        der = super::documentos_acciones::exportar_a(raiz, &x, "png", &tmp, pixpin_store::Idioma::Espanol)
            .ok()
            .and_then(|v| v.first().and_then(|p| pixpin_codec::imagen::cargar(p).ok()));
    }
    if der.is_none() && h.nuevos > 0 {
        der = escena_a_png(escena, imagenes, 1100);
    }
    let der = escalar(der.unwrap_or_else(|| tarjeta("Respuesta del asistente", &h.texto, 760.0)), 1100, 1300);
    let cuerpo = lado_a_lado(&izq, &der);
    let herramientas: Vec<&str> = h.llamadas.iter().map(|(n, _)| n.as_str()).collect();
    let mut cab = format!(
        "{} · {} · {} · 1º visible {:.1} s · total {:.1} s · {}\nPedido: «{}» ({})",
        caso.nombre,
        motor,
        match veredicto {
            Ok(()) => "BIEN".to_string(),
            Err(e) => format!("MAL: {e}"),
        },
        h.t_primero,
        h.t_total,
        if herramientas.is_empty() { "sin herramientas".to_string() } else { herramientas.join(", ") },
        caso.pedido,
        caso.doc
    );
    let resp: String = h.texto.chars().take(700).collect();
    cab.push_str(&format!("\nRespuesta: {}", resp.replace('\n', " ⏎ ")));
    let franja = tarjeta("", &cab, (cuerpo.ancho as f32 / 1.25).max(600.0));
    let todo = encima(&franja, &cuerpo);
    let png = dir.join(format!("{}.png", caso.nombre));
    std::fs::write(&png, pixpin_codec::imagen::codificar_png(&todo).unwrap()).unwrap();
    png
}

// ---------------------------------------------------------------------------
// Las pruebas

#[test]
#[ignore = "escribe el corpus en F:\\pixpin-banco-docs"]
fn generar_corpus() {
    corpus::generar();
    for d in corpus::DOCS {
        let r = corpus::ruta(d.nombre);
        println!("{:<34} {:>8} bytes  {}", d.nombre, std::fs::metadata(&r).map(|m| m.len()).unwrap_or(0), d.tipo);
    }
}

/// **Sin modelo**: lo que `leer_documento` saca de cada documento del corpus
/// y cuanto tarda (la parte de PixPin de la latencia).
#[test]
#[ignore = "lee el corpus de F:\\pixpin-banco-docs"]
fn leer_corpus() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let parar = std::sync::atomic::AtomicBool::new(false);
    for d in corpus::DOCS {
        let t = Instant::now();
        let r = super::en_el_chat::texto_del_documento(&corpus::ruta(d.nombre), &[], &parar);
        let ms = t.elapsed().as_millis();
        match r {
            Ok(s) => println!("{:<34} {ms:>5} ms {:>6} letras  {}", d.nombre, s.chars().count(), s.chars().take(160).collect::<String>().replace('\n', " ⏎ ")),
            Err(e) => println!("{:<34} {ms:>5} ms ERROR {e}", d.nombre),
        }
    }
}

/// **Sin modelo**: el aspecto medido de las tablas del corpus y la tabla
/// dibujada con el, junto al original (`banco-documentos\referencia`).
#[test]
#[ignore = "lee el corpus de F:\\pixpin-banco-docs y escribe PNG"]
fn medir_tablas() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let dir = base().join("referencia");
    std::fs::create_dir_all(&dir).unwrap();
    for doc in ["07-stock-materiales.png", "06-notas-curso.pdf", "13-guia-escaneada.png"] {
        let ruta = corpus::ruta(doc);
        let t = Instant::now();
        let Some((m, ancho)) = super::documentos_acciones::aspecto_de_tabla(&ruta, None) else {
            println!("{doc}: sin medida");
            continue;
        };
        let leida = super::documentos_acciones::leer_tabla(&ruta, None).unwrap();
        println!(
            "{doc} ({ancho} px, {} ms): letra {:.1} px, {} x {} rayas (tabla {} x {}), cuadra {}, borde {:?}, aire {:?}, texto {:?}\n  anchos {:?}\n  fondos fila 0 {:?}
  fuera {:?}",
            t.elapsed().as_millis(),
            m.tam_letra,
            m.filas.len(),
            m.columnas.len(),
            leida.filas.len(),
            leida.columnas(),
            m.cuadra(leida.filas.len(), leida.columnas()),
            m.borde,
            m.aire,
            m.texto,
            m.anchos(),
            m.fondos.first(),
            m.fuera
        );
        let raiz = dir.join("raiz");
        std::fs::create_dir_all(&raiz).unwrap();
        let mandar: super::acciones::Mandar = Box::new(|_| 1);
        let parar = std::sync::atomic::AtomicBool::new(false);
        let adj = vec![Adjunto { nombre: doc.into(), ruta: ruta.clone() }];
        let cx = super::documentos_acciones::Contexto { raiz: &raiz, proyecto: "p", adjuntos: &adj, mandar: &mandar, parar: &parar, idioma: pixpin_store::Idioma::Espanol };
        let (els, dicho) = super::documentos_acciones::tabla_para_el_lienzo(&json!({}), &cx, None, None).unwrap();
        println!("  {dicho}");
        let mut escena = pixpin_motor2d::Escena::nueva();
        for e in els {
            escena.anadir(e);
        }
        let imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(16_384);
        let res = escena_a_png(&escena, &imagenes, 1200).unwrap();
        let ori = corpus::vista_previa(doc, 1200).unwrap();
        let png = dir.join(format!("tabla-{doc}.png"));
        std::fs::write(&png, pixpin_codec::imagen::codificar_png(&lado_a_lado(&escalar(ori, 1100, 1300), &escalar(res, 1100, 1300))).unwrap()).unwrap();
        println!("  PNG {}", png.display());
    }
}

/// Lo que un caso deja para el que lanza (una linea JSON).
fn informe(caso: &Caso, motor: &str, h: &Hecho, v: &Result<(), String>, png: &Path) -> Value {
    json!({
        "caso": caso.nombre, "motor": motor, "tarea": caso.tarea, "doc": caso.doc,
        "bien": v.is_ok(), "fallo": v.as_ref().err(), "primero": h.t_primero, "total": h.t_total,
        "herramientas": h.llamadas.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
        "resultados": h.resultados.iter().map(|(n, ok, t)| format!("{n} {} {}", if *ok { "ok" } else { "ERR" }, t.chars().take(300).collect::<String>())).collect::<Vec<_>>(),
        "llamadas": h.llamadas.iter().map(|(n, a)| format!("{n} {}", a.chars().take(500).collect::<String>())).collect::<Vec<_>>(),
        "texto": h.texto, "png": png, "geometria": caso.de_geometria, "nuevos": h.nuevos,
    })
}

/// **Un caso** (`PIXPIN_BANCO_CASO`), en este proceso: lo lanza [`banco`].
#[test]
#[ignore = "habla con la nube (lo lanza `banco`)"]
fn un_caso() {
    let Ok(nombre) = std::env::var("PIXPIN_BANCO_CASO") else { return };
    let _com = pixpin_shell::ComDelHilo::iniciar();
    super::turno::apagar_ojos(std::env::var("PIXPIN_BANCO_OJOS").as_deref() == Ok("0"));
    let p = proveedor();
    let caso = casos().into_iter().find(|c| c.nombre == nombre).expect("caso desconocido");
    let (h, raiz, escena, imagenes, _) = match correr(&caso, p) {
        Ok(x) => x,
        Err(e) => {
            let inf = json!({"caso": caso.nombre, "motor": p.id(), "bien": false, "fallo": e, "primero": 0, "total": 0});
            let _ = std::fs::create_dir_all(base().join(p.id()));
            let _ = std::fs::write(base().join(p.id()).join(format!("{}.json", caso.nombre)), inf.to_string());
            println!("BANCO-JSON {inf}");
            return;
        }
    };
    let v = if h.fallo_motor { Err(h.texto.chars().take(200).collect()) } else { (caso.comprobar)(&h) };
    let png = pintar_caso(&caso, p.id(), &h, &v, &raiz, &escena, &imagenes);
    let inf = informe(&caso, p.id(), &h, &v, &png);
    std::fs::write(base().join(p.id()).join(format!("{}.json", caso.nombre)), serde_json::to_string_pretty(&inf).unwrap()).unwrap();
    println!("BANCO-JSON {inf}");
    // Sin esperar a soltar la GPU y el COM de los hilos: alguno se quedaba
    // colgado al salir (16-oct) y con el, el banco entero.
    std::process::exit(0);
}

/// **El banco**: cada caso en su proceso, `PIXPIN_BANCO_A_LA_VEZ` a la vez
/// (7), y la tabla al final.
#[test]
#[ignore = "habla con la nube y escribe PNG en el scratchpad"]
fn banco() {
    let p = proveedor();
    let solo = std::env::var("PIXPIN_IA_SOLO").ok();
    let a_la_vez: usize = std::env::var("PIXPIN_BANCO_A_LA_VEZ").ok().and_then(|n| n.parse().ok()).unwrap_or(7);
    let lista: Vec<Caso> = casos().into_iter().filter(|c| solo.as_deref().is_none_or(|s| s.split('|').any(|x| c.nombre.contains(x)))).collect();
    let exe = std::env::current_exe().unwrap();
    let t0 = Instant::now();
    let mut vivos: Vec<(String, std::process::Child, Instant)> = Vec::new();
    let mut pendientes: std::collections::VecDeque<&Caso> = lista.iter().collect();
    let mut filas: Vec<Value> = Vec::new();
    let registros = base().join("registros").join(p.id());
    std::fs::create_dir_all(&registros).unwrap();
    std::fs::create_dir_all(base().join(p.id())).unwrap();
    let json_de = |nombre: &str| base().join(p.id()).join(format!("{nombre}.json"));
    // Lo que dejo el caso en su `.json` (no por la tuberia: un proceso que
    // se queda colgado al salir, con la GPU, no deja leer su salida).
    let recoger = |nombre: &str, filas: &mut Vec<Value>| {
        let v: Value = std::fs::read_to_string(json_de(nombre))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(|| json!({"caso": nombre, "bien": false, "fallo": "el proceso no dio informe (ver registros)", "primero": 0, "total": 0}));
        println!(
            "{} {:<16} {:>5.1} s {:>6.1} s  {}  {}",
            if v["bien"] == true { "BIEN" } else { "MAL " },
            nombre,
            v["primero"].as_f64().unwrap_or(0.0),
            v["total"].as_f64().unwrap_or(0.0),
            v["herramientas"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(",")).unwrap_or_default(),
            v["fallo"].as_str().map(|f| format!("<- {f}")).unwrap_or_default()
        );
        filas.push(v);
    };
    while !pendientes.is_empty() || !vivos.is_empty() {
        while vivos.len() < a_la_vez
            && let Some(c) = pendientes.pop_front()
        {
            let _ = std::fs::remove_file(json_de(c.nombre));
            let salida = std::fs::File::create(registros.join(format!("{}.txt", c.nombre))).unwrap();
            let hijo = std::process::Command::new(&exe)
                .args(["--ignored", "--nocapture", "--exact", "asistente::banco_documentos::un_caso", "--test-threads=1"])
                .env("PIXPIN_BANCO_CASO", c.nombre)
                .env("PIXPIN_IA_MOTOR", p.id())
                .stdout(salida.try_clone().unwrap())
                .stderr(salida)
                .spawn()
                .unwrap();
            vivos.push((c.nombre.to_string(), hijo, Instant::now()));
        }
        let mut i = 0;
        while i < vivos.len() {
            let acabo = vivos[i].1.try_wait().ok().flatten().is_some();
            // Con su informe escrito hace rato, o 12 minutos sin acabar: fuera.
            let informado = std::fs::metadata(json_de(&vivos[i].0)).and_then(|m| m.modified()).is_ok_and(|t| t.elapsed().unwrap_or_default() > Duration::from_secs(30));
            let colgado = vivos[i].2.elapsed() > Duration::from_secs(720);
            if acabo || informado || colgado {
                let (n, mut hijo, _) = vivos.swap_remove(i);
                if !acabo {
                    let _ = hijo.kill();
                }
                recoger(&n, &mut filas);
            } else {
                i += 1;
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let bien = filas.iter().filter(|v| v["bien"] == true).count();
    println!("\n| caso | motor | bien | 1º visible | total | herramientas | fallo |\n|---|---|---|---|---|---|---|");
    filas.sort_by(|a, b| a["caso"].as_str().cmp(&b["caso"].as_str()));
    for v in &filas {
        println!(
            "| {} | {} | {} | {:.1} s | {:.1} s | {} | {} |",
            v["caso"].as_str().unwrap_or(""),
            p.id(),
            if v["bien"] == true { "✓" } else { "✗" },
            v["primero"].as_f64().unwrap_or(0.0),
            v["total"].as_f64().unwrap_or(0.0),
            v["herramientas"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default(),
            v["fallo"].as_str().unwrap_or("").replace('|', "/")
        );
    }
    let primeros: Vec<f64> = filas.iter().filter_map(|v| v["primero"].as_f64()).collect();
    let totales: Vec<f64> = filas.iter().filter_map(|v| v["total"].as_f64()).collect();
    let media = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    println!(
        "\nBANCO DOCUMENTOS {}: {bien}/{} = {:.0} % en {:.0} s · 1º visible medio {:.1} s · total medio {:.1} s",
        p.id(),
        filas.len(),
        bien as f64 * 100.0 / filas.len().max(1) as f64,
        t0.elapsed().as_secs_f32(),
        media(&primeros),
        media(&totales)
    );
    std::fs::write(base().join(format!("tabla-{}.json", p.id())), serde_json::to_string_pretty(&filas).unwrap()).unwrap();
}
