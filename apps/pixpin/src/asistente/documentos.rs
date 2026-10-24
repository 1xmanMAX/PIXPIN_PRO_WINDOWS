//! **Lo que el asistente sabe de los documentos del usuario** (encargo del
//! 13-oct-2026: «¿que dice la tesis sobre…?», «busca en mis notas…»,
//! «¿donde mande el plano de cunetas?»).
//!
//! El indice y la busqueda (BM25, trozos con su origen, citas) estan en
//! `pixpin_ia::documentos`, sin nada de Windows, para que el movil los porte
//! igual. Aqui va lo que solo sabe la app:
//!
//! - **Que se indexa** ([`fuentes`]): de cada proyecto de la lista, su chat
//!   (notas, voz pasada a texto, listas, los nombres de los adjuntos y lo
//!   que se escribio al mandarlos), los PDF, Word, Excel, PowerPoint, `.md`,
//!   `.txt` y `.csv` que se adjuntaron, y sus notas Markdown (`notas/`). El
//!   chat «Asistente» no: son sus propias respuestas.
//! - **Como se lee cada formato** ([`LectorDeLaApp`]): el texto de un PDF con
//!   `pixpin_pdf::texto` (pagina a pagina); un escaneado no tiene texto y no
//!   se indexa… salvo que ya se le pasara el OCR: el asistente deja su
//!   texto en el chat como `<nombre> (texto).txt` con `--- Hoja N ---`, y
//!   ese `.txt` se indexa **como el PDF**, con sus paginas, y abre el PDF.
//!   Un Word con `pixpin_docs` (por apartados: sus titulos), un PowerPoint
//!   con `pixpin_office` (por diapositiva), un Excel con `pixpin_hojas` (por
//!   hoja).
//! - **En segundo plano** ([`arrancar`]): un hilo con prioridad por debajo
//!   de lo normal (`pixpin_shell::prioridad`: cede la CPU a la ventana y al
//!   lapiz) que pone el indice al dia al rato de arrancar y cada vez que
//!   cambia algo bajo `proyectos\` (`pixpin_shell::vigia_carpeta`: Windows
//!   avisa y el hilo lo mira una vez por minuto, sin recorrer el disco), tras
//!   [`CALMA`] de silencio. Una busqueda tambien lo despierta. Solo si el
//!   asistente se puede usar (modelo bajado o nube elegida): quien no lo usa
//!   no paga nada.
//! - **En memoria solo mientras se usa**: tras [`REPOSO`] sin buscar, el
//!   indice se suelta (como el modelo); vive en `<raiz>\cache\documentos`,
//!   que no se sincroniza.
//! - **El alcance** ([`alcance_desde`]): desde un chat compartido solo se
//!   busca en ese chat. Lo filtra el indice, no el modelo.
//! - **La nube** ([`permiso_nube`]): con Claude o DeepSeek, los trozos
//!   encontrados salen a internet. La primera vez se pregunta y la
//!   respuesta queda en `[asistente] documentos_a_la_nube` (se cambia en
//!   Ajustes).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use pixpin_ia::documentos::{
    Abrir, Alcance, Firma, Fuente, Hallazgo, Indice, Lector, PeticionBusqueda, Tipo, Unidad, texto,
};
use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{Clase, Cuaderno, Mensaje};
use pixpin_store::Ubicacion;

/// Tras cuanto sin buscar se suelta el indice de la memoria.
pub const REPOSO: Duration = Duration::from_secs(5 * 60);
/// Cuanto se espera al arrancar antes de la primera puesta al dia (que
/// arrancar PixPin no compita con nada).
pub const ESPERA_INICIAL: Duration = Duration::from_secs(90);
/// Cuanto silencio tras un cambio antes de ponerse al dia (un `.pixpin`
/// que entra escribe cientos de ficheros).
pub const CALMA: Duration = Duration::from_secs(20);
/// Lo mas grande que se lee de un fichero (un PDF de 300 MB de fotos no
/// tiene texto que buscar y se come la memoria).
pub const MAX_BYTES: u64 = 200 * 1024 * 1024;
/// Cuantas celdas de cada hoja de Excel como mucho.
pub const MAX_CELDAS_HOJA: usize = 30_000;

/// Lo que se marca en el `.txt` del OCR al empezar cada hoja
/// (`asistente::ocr`).
const MARCA_HOJA: &str = "--- Hoja ";
/// Como acaba el nombre del `.txt` que deja el OCR (`dejar_texto_leido`).
const SUFIJO_OCR: &str = " (texto).txt";

/// Las extensiones que se leen.
fn tipo_de(nombre: &str) -> Option<Tipo> {
    Some(match pixpin_docs::extension(nombre).as_str() {
        "pdf" => Tipo::Pdf,
        "docx" | "docm" | "dotx" | "dotm" => Tipo::Word,
        "xlsx" | "xlsm" | "xls" => Tipo::Excel,
        "pptx" | "pptm" | "ppsx" | "ppsm" | "potx" | "potm" => Tipo::PowerPoint,
        "md" | "markdown" => Tipo::Nota,
        "txt" | "csv" => Tipo::Texto,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Que se indexa

/// El `proyecto` de un pedido: `None` es «Mensajes guardados».
fn proyecto_de_pedido(f: &almacen::Ficha) -> Option<String> {
    (!f.es_guardados()).then(|| f.id.clone())
}

/// Como se le llama a un proyecto.
fn nombre_de(f: &almacen::Ficha) -> String {
    if f.es_guardados() {
        "Mensajes guardados".into()
    } else {
        f.nombre.clone()
    }
}

fn es_de_la_ia(m: &Mensaje) -> bool {
    m.aparato.as_deref() == Some(super::chat::APARATO)
}

/// **Todas las fuentes** de los datos de `raiz`. Solo mira el disco (firmas
/// y `guardados.jsonl`), no lee ningun documento.
pub fn fuentes(raiz: &Path) -> Vec<Fuente> {
    let mut v = Vec::new();
    for ficha in almacen::Indice::leer(raiz).proyectos {
        if ficha.es_asistente() {
            continue;
        }
        let carpeta = almacen::carpeta(raiz, &ficha.id);
        let pedido = proyecto_de_pedido(&ficha);
        let pnombre = nombre_de(&ficha);
        let base = |clave: String,
                    nombre: String,
                    tipo: Tipo,
                    ruta: PathBuf,
                    firma: Firma,
                    abrir: Abrir| Fuente {
            clave,
            proyecto: ficha.id.clone(),
            proyecto_nombre: pnombre.clone(),
            nombre,
            tipo,
            ruta,
            firma,
            abrir,
        };
        // 1. El chat entero.
        let jsonl = carpeta.join("guardados.jsonl");
        let Some(firma) = Firma::de(&jsonl) else {
            continue;
        };
        v.push(base(
            format!("chat:{}", ficha.id),
            format!("Chat de {pnombre}"),
            Tipo::Chat,
            jsonl.clone(),
            firma,
            Abrir::Mensaje {
                proyecto: pedido.clone(),
                codigo: String::new(),
            },
        ));
        // 2. Sus adjuntos que se pueden leer.
        let cuaderno = Cuaderno::leer_de(&carpeta).unwrap_or_default();
        let adjuntos: Vec<(String, PathBuf)> = cuaderno
            .mensajes
            .iter()
            .filter(|m| m.clase == Some(Clase::Archivo) && !m.en_buzon)
            .filter_map(|m| {
                let ruta = pixpin_proyecto::vista::ruta_real(raiz, &ficha.id, m.ruta.as_deref()?)?;
                // `archivos/x.pdf` junto a la carpeta: con las barras de Windows,
                // que es como se ensena y como lo abre el lector.
                let ruta: PathBuf = ruta.components().collect();
                let nombre = if m.nombre.trim().is_empty() {
                    pixpin_docs::nombre(&ruta)
                } else {
                    m.nombre.trim().to_string()
                };
                Some((nombre, ruta))
            })
            .collect();
        let mut vistas = std::collections::HashSet::new();
        for (nombre, ruta) in &adjuntos {
            let Some(mut tipo) = tipo_de(nombre).or_else(|| tipo_de(&pixpin_docs::nombre(ruta)))
            else {
                continue;
            };
            let Some(firma) = Firma::de(ruta) else {
                continue;
            };
            if firma.bytes > MAX_BYTES || !vistas.insert(ruta.clone()) {
                continue;
            }
            let mut mostrado = nombre.clone();
            let mut abrir = Abrir::Fichero {
                ruta: ruta.clone(),
                pagina: None,
            };
            // El texto del OCR de un PDF escaneado: cuenta como el PDF.
            if let Some(base_pdf) = nombre.strip_suffix(SUFIJO_OCR)
                && let Some((pdf_nombre, pdf)) = adjuntos.iter().find(|(n, _)| {
                    pixpin_docs::extension(n) == "pdf" && pixpin_docs::sin_extension(n) == base_pdf
                })
            {
                tipo = Tipo::Pdf;
                mostrado = pdf_nombre.clone();
                abrir = Abrir::Fichero {
                    ruta: pdf.clone(),
                    pagina: None,
                };
            }
            v.push(base(
                format!("f:{}", ruta.display()),
                mostrado,
                tipo,
                ruta.clone(),
                firma,
                abrir,
            ));
        }
        // 3. Sus notas Markdown.
        if let Ok(dir) = std::fs::read_dir(carpeta.join("notas")) {
            for e in dir.flatten() {
                let ruta = e.path();
                if tipo_de(&pixpin_docs::nombre(&ruta)) != Some(Tipo::Nota)
                    || !vistas.insert(ruta.clone())
                {
                    continue;
                }
                let Some(firma) = Firma::de(&ruta) else {
                    continue;
                };
                let titulo = std::fs::read_to_string(&ruta)
                    .ok()
                    .and_then(|t| {
                        t.lines()
                            .map(|l| l.trim().trim_start_matches('#').trim().to_string())
                            .find(|l| !l.is_empty())
                    })
                    .map(|t| t.chars().take(80).collect::<String>())
                    .unwrap_or_else(|| pixpin_docs::sin_extension(&pixpin_docs::nombre(&ruta)));
                v.push(base(
                    format!("f:{}", ruta.display()),
                    format!("Nota «{titulo}»"),
                    Tipo::Nota,
                    ruta.clone(),
                    firma,
                    Abrir::Fichero { ruta, pagina: None },
                ));
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------
// Como se lee cada cosa

/// Lee cada formato con lo que la app ya tiene.
pub struct LectorDeLaApp;

impl Lector for LectorDeLaApp {
    fn leer(&self, f: &Fuente) -> Result<Vec<Unidad>, String> {
        if f.tipo == Tipo::Chat {
            return leer_chat(f);
        }
        leer_fichero(&f.ruta)
    }
}

/// **El texto de un fichero** (PDF, Word, PowerPoint, Excel, nota, texto),
/// por unidades: lo que indexa la busqueda y lo que lee en voz alta el
/// asistente (`pedidos::leer_en_voz`, 15-oct).
pub fn leer_fichero(ruta: &Path) -> Result<Vec<Unidad>, String> {
    if !ruta.is_file() {
        return Err("el fichero ya no esta".into());
    }
    let nombre = pixpin_docs::nombre(ruta);
    // Por la extension del fichero de verdad (el `.txt` del OCR cuenta
    // como PDF para el usuario, pero se lee como texto).
    match tipo_de(&nombre) {
        Some(Tipo::Pdf) => leer_pdf(ruta),
        Some(Tipo::Word) => leer_word(ruta),
        Some(Tipo::PowerPoint) => leer_powerpoint(ruta),
        Some(Tipo::Excel) => leer_excel(ruta),
        Some(Tipo::Nota) | Some(Tipo::Texto) => leer_texto(ruta),
        _ => Err(format!("PixPin no sabe leer «{nombre}»")),
    }
}

/// Las notas, la voz pasada a texto, las listas y los adjuntos de un chat:
/// cada mensaje, su unidad, que abre el chat en el.
fn leer_chat(f: &Fuente) -> Result<Vec<Unidad>, String> {
    let texto =
        std::fs::read_to_string(&f.ruta).map_err(|e| format!("no se pudo leer el chat: {e}"))?;
    let proyecto = match &f.abrir {
        Abrir::Mensaje { proyecto, .. } => proyecto.clone(),
        _ => Some(f.proyecto.clone()),
    };
    let mut v = Vec::new();
    for m in Cuaderno::leer(&texto).mensajes {
        if m.en_buzon || es_de_la_ia(&m) || m.id.is_empty() {
            continue;
        }
        let (texto, seccion) = match &m.clase {
            None | Some(Clase::Nota) => (m.texto.clone(), None),
            Some(Clase::Voz) => (
                m.transcripcion
                    .clone()
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_default(),
                Some("nota de voz".to_string()),
            ),
            Some(Clase::MiniApp) => (m.texto.clone(), Some("lista".to_string())),
            Some(Clase::Archivo) | Some(Clase::Imagen) => {
                // Las lecciones tienen su buscador; su fichero no dice nada aqui.
                if m.ruta.as_deref().is_some_and(|r| r.ends_with(".leccion")) {
                    continue;
                }
                let nombre = if m.nombre.trim().is_empty() {
                    m.ruta
                        .as_deref()
                        .map(|r| r.rsplit(['/', '\\']).next().unwrap_or(r).to_string())
                        .unwrap_or_default()
                } else {
                    m.nombre.clone()
                };
                (format!("{nombre} {}", m.texto), Some("adjunto".to_string()))
            }
            Some(Clase::Dibujo) | Some(Clase::Pagina) => (
                format!("{} {}", m.nombre, m.texto),
                Some("lienzo".to_string()),
            ),
            _ => (m.resumen(), None),
        };
        if texto.trim().is_empty() {
            continue;
        }
        v.push(Unidad {
            texto,
            pagina: None,
            seccion,
            abrir: Some(Abrir::Mensaje {
                proyecto: proyecto.clone(),
                codigo: m.id.clone(),
            }),
            cuando: Some(m.cuando).filter(|c| *c > 0),
        });
    }
    Ok(v)
}

/// Un PDF, pagina a pagina. Sin texto (escaneado) no da nada: lo que se
/// sepa de el vendra del `.txt` de su OCR, si se hizo.
fn leer_pdf(ruta: &Path) -> Result<Vec<Unidad>, String> {
    let paginas = pixpin_pdf::texto::de_fichero(ruta)
        .map_err(|e| format!("el PDF no se deja leer: {e:?}"))?;
    Ok(paginas
        .into_iter()
        .enumerate()
        .filter(|(_, p)| !p.texto.trim().is_empty())
        .map(|(i, p)| Unidad {
            texto: p.texto,
            pagina: Some(i as u32 + 1),
            ..Default::default()
        })
        .collect())
}

/// Un Word, por apartados: cada titulo empieza una unidad con su nombre.
fn leer_word(ruta: &Path) -> Result<Vec<Unidad>, String> {
    let doc = pixpin_docs::abrir(ruta).map_err(|e| format!("el Word no se deja leer: {e}"))?;
    let mut v: Vec<Unidad> = Vec::new();
    let mut actual = Unidad::default();
    for b in &doc.bloques {
        let t = b.texto();
        let t = t.replace(pixpin_docs::documento::SEPARADOR_DE_CELDA, " | ");
        if t.trim().is_empty() || t.trim() == pixpin_docs::documento::MARCA_IMAGEN {
            continue;
        }
        if matches!(
            b.clase,
            pixpin_docs::documento::Clase::Titulo(_) | pixpin_docs::documento::Clase::Capitulo
        ) {
            if !actual.texto.trim().is_empty() {
                v.push(std::mem::take(&mut actual));
            }
            actual.seccion = Some(t.trim().chars().take(80).collect());
        }
        actual.texto.push_str(&t);
        actual.texto.push('\n');
    }
    if !actual.texto.trim().is_empty() {
        v.push(actual);
    }
    Ok(v)
}

/// Un PowerPoint, por diapositiva (maquetada con medidas fijas: solo se
/// quiere el texto, no donde cae).
fn leer_powerpoint(ruta: &Path) -> Result<Vec<Unidad>, String> {
    let doc = pixpin_office::Documento::abrir(ruta)
        .map_err(|e| format!("el PowerPoint no se deja leer: {e}"))?;
    let pixpin_office::Documento::PowerPoint(mut p) = doc else {
        return Err("no es un PowerPoint".into());
    };
    let medidor = pixpin_office::MedidorFijo::default();
    let mut v = Vec::new();
    for n in 0..p.cuantas() {
        let Some(pagina) = p.diapositiva(n, &medidor) else {
            continue;
        };
        let texto = pagina
            .textos()
            .iter()
            .map(|(t, _)| t.texto.trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if !texto.is_empty() {
            v.push(Unidad {
                texto,
                pagina: Some(n as u32 + 1),
                ..Default::default()
            });
        }
    }
    Ok(v)
}

/// Un Excel, por hoja: «A1: valor» no hace falta; las celdas de cada fila
/// separadas por « | », que es como se lee una tabla.
fn leer_excel(ruta: &Path) -> Result<Vec<Unidad>, String> {
    let libro = pixpin_hojas::Libro::abrir(ruta, pixpin_hojas::Regional::default())
        .map_err(|e| format!("el Excel no se deja leer: {e}"))?;
    let mut v = Vec::new();
    for (i, info) in libro.hojas.iter().enumerate() {
        if info.tipo != pixpin_hojas::TipoHoja::Hoja {
            continue;
        }
        let mut hoja = libro.abrir_hoja(i);
        hoja.cargar_todo();
        let mut texto = String::new();
        let mut celdas = 0;
        let filas: Vec<u32> = {
            let f = hoja.filas_entre(0, u32::MAX);
            if f.is_empty() {
                (0..=hoja.max_fila().min(MAX_CELDAS_HOJA as u32)).collect()
            } else {
                f.iter().map(|f| f.indice).collect()
            }
        };
        for i in filas {
            let Some(fila) = hoja.fila(i) else { continue };
            let linea: Vec<String> = hoja
                .celdas_de(fila)
                .iter()
                .map(|c| libro.texto_crudo(&hoja, c))
                .filter(|t| !t.trim().is_empty())
                .collect();
            celdas += linea.len();
            if !linea.is_empty() {
                texto.push_str(&linea.join(" | "));
                texto.push('\n');
            }
            if celdas >= MAX_CELDAS_HOJA {
                break;
            }
        }
        if !texto.trim().is_empty() {
            v.push(Unidad {
                texto,
                seccion: Some(format!("hoja «{}»", info.nombre)),
                ..Default::default()
            });
        }
    }
    Ok(v)
}

/// Un texto llano o una nota. Si es lo que dejo el OCR (`--- Hoja N ---`),
/// por paginas.
fn leer_texto(ruta: &Path) -> Result<Vec<Unidad>, String> {
    let bytes = std::fs::read(ruta).map_err(|e| format!("no se pudo leer: {e}"))?;
    let texto = pixpin_docs::texto_de(&bytes);
    if !texto.contains(MARCA_HOJA) {
        return Ok(vec![Unidad {
            texto,
            ..Default::default()
        }]);
    }
    let mut v = Vec::new();
    let mut pagina = None;
    let mut actual = String::new();
    for linea in texto.lines() {
        if let Some(n) = linea
            .trim()
            .strip_prefix(MARCA_HOJA)
            .and_then(|r| r.trim_end_matches('-').trim().parse::<u32>().ok())
        {
            if !actual.trim().is_empty() {
                v.push(Unidad {
                    texto: std::mem::take(&mut actual),
                    pagina,
                    ..Default::default()
                });
            }
            actual.clear();
            pagina = Some(n);
            continue;
        }
        actual.push_str(linea);
        actual.push('\n');
    }
    if !actual.trim().is_empty() {
        v.push(Unidad {
            texto: actual,
            pagina,
            ..Default::default()
        });
    }
    Ok(v)
}

// ---------------------------------------------------------------------------
// El servicio: un indice por proceso, al dia en segundo plano

struct Servicio {
    ubicacion: Option<Ubicacion>,
    /// El indice si esta en memoria, y cuando se uso por ultima vez.
    indice: Option<(Indice, Instant)>,
}

static SERVICIO: Mutex<Servicio> = Mutex::new(Servicio {
    ubicacion: None,
    indice: None,
});
/// Solo una puesta al dia a la vez (la del hilo de fondo o la de una
/// busqueda con el indice aun vacio).
static PONIENDO_AL_DIA: Mutex<()> = Mutex::new(());
/// Para despertar al hilo de fondo (una busqueda pide ponerse al dia).
static DESPERTAR: OnceLock<std::sync::mpsc::Sender<()>> = OnceLock::new();

fn servicio() -> std::sync::MutexGuard<'static, Servicio> {
    SERVICIO.lock().unwrap_or_else(|e| e.into_inner())
}

/// Donde vive el indice.
pub fn carpeta_cache(raiz: &Path) -> PathBuf {
    raiz.join("cache").join("documentos")
}

/// La ubicacion de los datos (la pone [`arrancar`]; las pruebas, la suya).
pub fn poner_ubicacion(u: Ubicacion) {
    let mut s = servicio();
    if s.ubicacion.as_ref().map(|x| x.raiz()) != Some(u.raiz()) {
        s.indice = None;
    }
    s.ubicacion = Some(u);
}

pub(crate) fn raiz() -> Option<PathBuf> {
    servicio()
        .ubicacion
        .as_ref()
        .map(|u| u.raiz().to_path_buf())
}

/// **Pone el indice al dia** con los datos de ahora. Lee los documentos sin
/// tener el indice cogido (una busqueda no espera a que acabe un PDF largo).
pub fn poner_al_dia() -> Option<pixpin_ia::documentos::Informe> {
    let raiz = raiz()?;
    let _uno = PONIENDO_AL_DIA.lock().unwrap_or_else(|e| e.into_inner());
    let t0 = Instant::now();
    let todas = fuentes(&raiz);
    let (leer, quitar) = {
        let mut s = servicio();
        let (ix, _) = s
            .indice
            .get_or_insert_with(|| (Indice::cargar(&carpeta_cache(&raiz)), Instant::now()));
        let p = ix.pendientes(&todas);
        for c in &p.1 {
            ix.quitar(c);
        }
        p
    };
    let mut inf = pixpin_ia::documentos::Informe {
        sin_cambios: todas.len() - leer.len(),
        quitadas: quitar.len(),
        ..Default::default()
    };
    let lector = LectorDeLaApp;
    for f in leer {
        // Un fichero raro no tumba el hilo ni el indice.
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| lector.leer(&f)))
            .unwrap_or_else(|_| Err("el lector se rompio leyendolo".into()));
        if let Err(e) = &r {
            inf.rotas.push((f.nombre.clone(), e.clone()));
        }
        inf.leidas += 1;
        let mut s = servicio();
        let (ix, _) = s
            .indice
            .get_or_insert_with(|| (Indice::cargar(&carpeta_cache(&raiz)), Instant::now()));
        ix.aplicar(f, r);
    }
    if let Some((ix, _)) = servicio().indice.as_mut() {
        ix.guardar();
    }
    if inf.leidas > 0 || inf.quitadas > 0 {
        tracing::info!(
            leidas = inf.leidas,
            quitadas = inf.quitadas,
            rotas = inf.rotas.len(),
            ms = t0.elapsed().as_millis() as u64,
            "documentos: indice al dia"
        );
    }
    Some(inf)
}

/// **El alcance desde un chat**: si es un chat compartido con otra persona,
/// solo ese chat; si no, todo lo del usuario.
pub fn alcance_desde(raiz: &Path, proyecto: &str) -> Alcance {
    let chat = pixpin_proyecto::vista::chat_de_ficha(raiz, proyecto)
        .unwrap_or_else(|| proyecto.to_string());
    let compartidos = pixpin_sincro::compartidos::CompartidosEnDisco::nuevo(raiz);
    if compartidos.de(&chat).is_some() || compartidos.de(proyecto).is_some() {
        Alcance::solo(proyecto)
    } else {
        Alcance::Todo
    }
}

/// Los proyectos que se llaman como dice el modelo (sin mayusculas ni
/// tildes, y vale un trozo: «obra» encuentra «Obra Carretera Sur»).
fn proyectos_llamados(raiz: &Path, nombre: &str) -> Vec<String> {
    let buscado = texto::plano(nombre.trim());
    almacen::Indice::leer(raiz)
        .proyectos
        .into_iter()
        .filter(|f| {
            let n = texto::plano(&nombre_de(f));
            !buscado.is_empty() && (n == buscado || n.contains(&buscado))
        })
        .map(|f| f.id)
        .collect()
}

/// **Busca** lo que pide el modelo desde el chat `proyecto`. Devuelve lo
/// encontrado y un aviso para el modelo, si lo hay (el proyecto que nombro
/// no existe o no se ve desde aqui).
pub fn buscar(
    p: &PeticionBusqueda,
    proyecto: &str,
) -> Result<(Vec<Hallazgo>, Option<String>), String> {
    let raiz = raiz().ok_or("el indice de documentos no esta listo")?;
    let base = alcance_desde(&raiz, proyecto);
    let compartido = base != Alcance::Todo;
    let mut aviso = None;
    let alcance = match &p.proyecto {
        None => base.clone(),
        Some(nombre) => {
            let ids: Vec<String> = proyectos_llamados(&raiz, nombre)
                .into_iter()
                .filter(|id| base.deja(id))
                .collect();
            if ids.is_empty() {
                // No se dice si existe en otro lado: desde un chat compartido
                // eso tambien es privado.
                aviso = Some(if compartido {
                    "Desde este chat compartido solo se busca en este chat.".to_string()
                } else {
                    format!("No hay ningun proyecto llamado «{nombre}»; se busco en todos.")
                });
                base.clone()
            } else {
                Alcance::Proyectos(ids.into_iter().collect())
            }
        }
    };
    // La primera vez (sin indice en el disco), se hace ahora: mejor esperar
    // que contestar «no hay nada».
    let vacio = {
        let mut s = servicio();
        let (ix, usado) = s
            .indice
            .get_or_insert_with(|| (Indice::cargar(&carpeta_cache(&raiz)), Instant::now()));
        *usado = Instant::now();
        ix.tamano().0 == 0
    };
    if vacio {
        poner_al_dia();
    } else if let Some(tx) = DESPERTAR.get() {
        // Lo que haya cambiado desde la ultima vez, para la siguiente.
        let _ = tx.send(());
    }
    let mut s = servicio();
    let (ix, usado) = s
        .indice
        .get_or_insert_with(|| (Indice::cargar(&carpeta_cache(&raiz)), Instant::now()));
    *usado = Instant::now();
    Ok((
        ix.buscar(
            &p.consulta,
            &alcance,
            pixpin_ia::documentos::herramienta::RESULTADOS,
        ),
        aviso,
    ))
}

/// Suelta el indice si lleva [`REPOSO`] sin usarse.
fn soltar_si_reposa() {
    let mut s = servicio();
    if s.indice
        .as_ref()
        .is_some_and(|(_, u)| u.elapsed() >= REPOSO)
    {
        s.indice = None;
        tracing::debug!("documentos: indice fuera de la memoria");
    }
}

/// Si el asistente se puede usar: modelo bajado o un motor de la nube.
fn asistente_en_uso(u: &Ubicacion) -> bool {
    let ajustes = pixpin_store::ajustes::cargar(u).unwrap_or_default();
    if !ajustes.asistente.motor.is_empty() && ajustes.asistente.motor != "local" {
        return true;
    }
    super::motor::raiz().is_some_and(|r| {
        let m = super::motor::modelo_elegido(Some(&ajustes.asistente.modelo));
        pixpin_ia::descarga::carpeta_modelo(&r, &m)
            .join(m.pesos.nombre)
            .is_file()
    })
}

/// **Arranca el hilo de fondo** que tiene el indice al dia. Una vez por
/// proceso.
pub fn arrancar(ubicacion: Ubicacion) {
    poner_ubicacion(ubicacion.clone());
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    if DESPERTAR.set(tx).is_err() {
        return;
    }
    let hecho = std::thread::Builder::new()
        .name("indice-documentos".into())
        .spawn(move || {
            // Por debajo de lo normal y no el «modo de fondo» de Windows: ese
            // baja tambien la prioridad de la memoria y lo hace veinte veces
            // mas lento (medido con el compresor de PDF, `pixpin_shell::prioridad`).
            pixpin_shell::prioridad::hilo_por_debajo_de_lo_normal();
            let proyectos = ubicacion.raiz().join("proyectos");
            let mut vigia: Option<pixpin_shell::vigia_carpeta::VigiaCarpeta> = None;
            let mut proxima = Some(Instant::now() + ESPERA_INICIAL);
            loop {
                // Despierta por una busqueda, por un cambio en el disco o
                // cada minuto (para soltar la memoria).
                let pedido = rx.recv_timeout(Duration::from_secs(60)).is_ok();
                if pedido {
                    proxima = Some(
                        proxima.map_or(Instant::now() + CALMA, |p| p.min(Instant::now() + CALMA)),
                    );
                }
                if vigia.is_none() {
                    vigia = pixpin_shell::vigia_carpeta::VigiaCarpeta::nueva(&proyectos);
                }
                if vigia
                    .as_ref()
                    .is_some_and(pixpin_shell::vigia_carpeta::VigiaCarpeta::cambio)
                {
                    proxima = Some(Instant::now() + CALMA);
                }
                if proxima.is_some_and(|p| Instant::now() >= p) {
                    if asistente_en_uso(&ubicacion) {
                        proxima = None;
                        poner_al_dia();
                    } else {
                        // Sin asistente no se gasta nada; se mira otra vez luego.
                        proxima = Some(Instant::now() + Duration::from_secs(10 * 60));
                    }
                }
                soltar_si_reposa();
            }
        });
    if let Err(e) = hecho {
        tracing::warn!(?e, "documentos: no se pudo lanzar el hilo del indice");
    }
}

// ---------------------------------------------------------------------------
// La nube

/// Lo que dice el ajuste `documentos_a_la_nube`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermisoNube {
    Preguntar,
    Si,
    No,
}

impl PermisoNube {
    pub fn de(ajuste: &str) -> PermisoNube {
        match ajuste.trim() {
            "si" | "sí" | "permitir" => PermisoNube::Si,
            "no" => PermisoNube::No,
            _ => PermisoNube::Preguntar,
        }
    }

    pub fn clave(self) -> &'static str {
        match self {
            PermisoNube::Preguntar => "",
            PermisoNube::Si => "si",
            PermisoNube::No => "no",
        }
    }
}

/// Lo que se pregunta la primera vez.
pub const PREGUNTA_NUBE: &str = "El asistente está usando un modelo de la nube. Para contestar sobre tus \
documentos le mandaría trozos de ellos (lo que encuentre en tus PDF, notas y chats) por internet.\n\n\
¿Lo permites? Lo que elijas se recuerda y se cambia en Ajustes › Asistente.";

/// **¿Pueden salir los trozos a la nube?** Con el ajuste en «preguntar», se
/// pregunta (con `confirmar`) y la respuesta se guarda. Sin ubicacion (las
/// pruebas) no se guarda nada.
pub fn permiso_nube(confirmar: &super::acciones::Confirmar) -> bool {
    let u = servicio().ubicacion.clone();
    let ajustes = u
        .as_ref()
        .and_then(|u| pixpin_store::ajustes::cargar(u).ok());
    let actual = ajustes.as_ref().map_or(PermisoNube::Preguntar, |a| {
        PermisoNube::de(&a.asistente.documentos_a_la_nube)
    });
    match actual {
        PermisoNube::Si => true,
        PermisoNube::No => false,
        PermisoNube::Preguntar => {
            let si = confirmar(PREGUNTA_NUBE);
            if let (Some(u), Some(mut a)) = (u, ajustes) {
                a.asistente.documentos_a_la_nube =
                    if si { PermisoNube::Si } else { PermisoNube::No }
                        .clave()
                        .to_string();
                if let Err(e) = pixpin_store::ajustes::guardar_conservando(&u, &a) {
                    tracing::warn!(?e, "documentos: no se pudo guardar el permiso de la nube");
                }
            }
            si
        }
    }
}

/// Lo que se le dice al modelo si no hay permiso.
pub const SIN_PERMISO_NUBE: &str = "el usuario no permite mandar sus documentos a la nube; dile que puede \
permitirlo en Ajustes › Asistente o usar el modelo local, que busca sin salir del PC";

#[cfg(test)]
pub(super) fn vaciar_para_pruebas() {
    let mut s = servicio();
    s.indice = None;
    s.ubicacion = None;
}

#[cfg(test)]
mod pruebas;
