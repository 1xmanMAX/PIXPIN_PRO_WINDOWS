//! **Lo que se lee de la carpeta de datos**, sin la app y sin escribir nada:
//! las tareas, los chats y el indice de documentos del asistente.
//!
//! - **Tareas**: las listas son mensajes `MINIAPP` «tareas» de cada chat
//!   (`pindesk_proyecto::mini`), y la consulta es la del asistente
//!   (`pindesk_ia::tareas::consultar`, la misma frase de respuesta). Lo unico
//!   propio es [`FuenteDelDisco`], el adaptador que en la app es
//!   `asistente::acciones::FuenteDeLaApp` (que no se puede usar desde aqui:
//!   vive en el ejecutable).
//! - **Buscar**: el indice BM25 que la app tiene al dia en
//!   `<raiz>\cache\documentos` (`pindesk_ia::documentos::Indice::cargar`, solo
//!   lee). Si aun no existe (el asistente no se ha usado nunca), se busca en
//!   los chats con un indice en memoria, que no se guarda.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use pindesk_ia::documentos::{
    Abrir, Alcance, Cita, Firma, Fuente, Hallazgo, Indice, Lector, Tipo, Unidad,
};
use pindesk_ia::fechas::Momento;
use pindesk_ia::tareas::{Chat, FuenteTareas, VistaLista, VistaTarea};
use pindesk_proyecto::cuaderno::{Clase, Cuaderno};
use pindesk_proyecto::{almacen, mini};

use crate::herramientas::{Contenido, Resultado};

/// Otra raiz de datos (pruebas). Sin ella, la de PixPin: la carpeta de este
/// exe si a su lado hay un `pindesk.toml` (portable), si no
/// `%APPDATA%\PinDesk`, igual que decide la app.
pub const VAR_RAIZ: &str = "PINDESK_RAIZ";

/// **Donde estan los datos.**
pub fn raiz() -> PathBuf {
    if let Some(r) = std::env::var_os(VAR_RAIZ).filter(|r| !r.is_empty()) {
        return PathBuf::from(r);
    }
    let dir_exe = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let appdata = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default();
    pindesk_store::rutas::resolver(&dir_exe, &appdata)
        .raiz()
        .to_path_buf()
}

/// **El OCR con el motor que eligio el usuario en PinDesk** (`[ocr]`): leer
/// un documento desde fuera da lo mismo que leerlo en la app. Sin ajustes
/// legibles, el de Windows. Nunca baja nada: eso se pide en Ajustes.
pub fn configurar_ocr() {
    let raiz = raiz();
    let u = pindesk_store::Ubicacion::Instalado { raiz: raiz.clone() };
    let a = pindesk_store::ajustes::cargar(&u).unwrap_or_default();
    let exe = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf));
    pindesk_ocr::configurar(pindesk_ocr::Config {
        motor: match a.ocr.motor {
            pindesk_store::ajustes::MotorOcr::Windows => pindesk_ocr::Motor::Windows,
            pindesk_store::ajustes::MotorOcr::Indra => pindesk_ocr::Motor::Indra,
        },
        raiz: Some(raiz),
        junto_al_exe: exe,
        corregir: a.ocr.corregir,
        vocabulario: a.ocr.vocabulario,
        // Un proceso aparte, a la vez que la app: dos hilos.
        hilos: 2,
    });
}

// ---------------------------------------------------------------------------
// Tareas

/// Las listas de tareas de todos los chats, como las ve el asistente.
pub struct FuenteDelDisco {
    pub raiz: PathBuf,
}

/// Una tarea como la ve el asistente: sin fecha de creacion, sin imagenes y
/// sin su marca ⏰ (que va aparte, en `vence`).
fn vista_de_tarea(indice: usize, t: &mini::Tarea) -> VistaTarea {
    let (visible, creada) = mini::partir(&t.texto);
    let (texto, _) = mini::imagenes_de(visible);
    let vence = Momento::de_marca(&texto);
    let texto = match (vence, texto.rfind('⏰')) {
        (Some(_), Some(i)) => texto[..i].trim_end().to_string(),
        _ => texto,
    };
    VistaTarea {
        indice,
        texto: texto.trim().to_string(),
        hecha: t.hecha,
        creada: creada.map(|d| d.dias()),
        vence,
    }
}

impl FuenteTareas for FuenteDelDisco {
    fn listas(&self) -> Vec<VistaLista> {
        let mut v = Vec::new();
        for ficha in almacen::Indice::leer(&self.raiz).proyectos {
            let Ok(c) = Cuaderno::leer_de(&almacen::carpeta(&self.raiz, &ficha.id)) else {
                continue;
            };
            for m in &c.mensajes {
                if m.clase != Some(Clase::MiniApp) || m.miniapp.as_deref() != Some(mini::TAREAS) {
                    continue;
                }
                let titulo = match mini::titulo(&m.texto) {
                    t if t.is_empty() => m.nombre.clone(),
                    t => t,
                };
                v.push(VistaLista {
                    proyecto: ficha.id.clone(),
                    chat: ficha.nombre.clone(),
                    guardados: ficha.es_guardados(),
                    codigo: m.id.clone(),
                    titulo,
                    cuando: m.cuando,
                    tareas: mini::leer_tareas(&m.texto)
                        .iter()
                        .enumerate()
                        .map(|(i, t)| vista_de_tarea(i, t))
                        .collect(),
                });
            }
        }
        // El Inbox primero, luego lo mas nuevo (como la ventana de tareas).
        v.sort_by(|a, b| {
            b.es_inbox()
                .cmp(&a.es_inbox())
                .then(b.cuando.cmp(&a.cuando))
        });
        v
    }

    fn chats(&self) -> Vec<Chat> {
        almacen::Indice::leer(&self.raiz)
            .proyectos
            .iter()
            .map(|f| Chat {
                id: f.id.clone(),
                nombre: f.nombre.clone(),
                guardados: f.es_guardados(),
            })
            .collect()
    }

    fn ahora(&self) -> Momento {
        Momento::de_ms_locales(pindesk_shell::entorno::ahora_local_ms())
    }
}

/// **`consultar_tareas`**: la fila de la tabla, validada con la tabla y
/// contestada con la consulta del asistente.
pub fn consultar_tareas(args: &Map<String, Value>) -> Resultado {
    let fila = pindesk_ia::acciones::por_nombre("consultar_tareas").expect("esta en la tabla");
    let a = pindesk_ia::acciones::validar(fila, args).map_err(|e| e.to_string())?;
    let texto = pindesk_ia::tareas::consultar(&a.por_resolver, &FuenteDelDisco { raiz: raiz() })?;
    Ok(vec![Contenido::Texto(texto)])
}

/// **`listar_chats`**.
pub fn listar_chats(_: &Map<String, Value>) -> Resultado {
    let raiz = raiz();
    let indice = almacen::Indice::leer(&raiz);
    if indice.proyectos.is_empty() {
        return Ok(vec![Contenido::Texto(format!(
            "No hay chats en {}.",
            raiz.display()
        ))]);
    }
    let listas = FuenteDelDisco { raiz: raiz.clone() }.listas();
    let mut s = String::new();
    for f in indice.ordenadas() {
        let nombre = if f.es_guardados() {
            "Mensajes guardados".to_string()
        } else {
            f.nombre.clone()
        };
        s.push_str(&format!("- {nombre} (id {})", f.id));
        if let Some(fecha) = pindesk_ia::documentos::herramienta::fecha_corta(f.tocado) {
            s.push_str(&format!(", tocado el {fecha}"));
        }
        let suyas: Vec<String> = listas
            .iter()
            .filter(|l| l.proyecto == f.id)
            .map(|l| {
                format!(
                    "«{}» ({} pendientes)",
                    l.titulo,
                    l.tareas.iter().filter(|t| !t.hecha).count()
                )
            })
            .collect();
        if !suyas.is_empty() {
            s.push_str(&format!("; listas: {}", suyas.join(", ")));
        }
        s.push('\n');
    }
    Ok(vec![Contenido::Texto(s.trim_end().to_string())])
}

// ---------------------------------------------------------------------------
// Buscar

/// Como se llama un chat al decirlo.
fn nombre_de(f: &almacen::Ficha) -> String {
    if f.es_guardados() {
        "Mensajes guardados".into()
    } else {
        f.nombre.clone()
    }
}

/// Lee los chats para el indice en memoria (sin el de la app).
struct LectorDeChats;

impl Lector for LectorDeChats {
    fn leer(&self, f: &Fuente) -> Result<Vec<Unidad>, String> {
        let texto = std::fs::read_to_string(&f.ruta).map_err(|e| e.to_string())?;
        let proyecto = match &f.abrir {
            Abrir::Mensaje { proyecto, .. } => proyecto.clone(),
            _ => None,
        };
        Ok(Cuaderno::leer(&texto)
            .mensajes
            .into_iter()
            .filter(|m| !m.en_buzon && !m.id.is_empty())
            .filter_map(|m| {
                let t = match m.clase {
                    Some(Clase::Voz) => m.transcripcion.clone().unwrap_or_default(),
                    Some(Clase::Archivo) | Some(Clase::Imagen) | Some(Clase::Dibujo)
                    | Some(Clase::Pagina) => {
                        format!("{} {}", m.nombre, m.texto)
                    }
                    _ => m.texto.clone(),
                };
                (!t.trim().is_empty()).then(|| Unidad {
                    texto: t,
                    abrir: Some(Abrir::Mensaje {
                        proyecto: proyecto.clone(),
                        codigo: m.id.clone(),
                    }),
                    cuando: Some(m.cuando).filter(|c| *c > 0),
                    ..Default::default()
                })
            })
            .collect())
    }
}

/// Un indice de los chats, solo en memoria.
fn indice_de_chats(raiz: &Path) -> Indice {
    let fuentes: Vec<Fuente> = almacen::Indice::leer(raiz)
        .proyectos
        .iter()
        .filter(|f| !f.es_asistente())
        .filter_map(|f| {
            let ruta = almacen::carpeta(raiz, &f.id).join("guardados.jsonl");
            Some(Fuente {
                clave: format!("chat:{}", f.id),
                proyecto: f.id.clone(),
                proyecto_nombre: nombre_de(f),
                nombre: format!("Chat de {}", nombre_de(f)),
                tipo: Tipo::Chat,
                firma: Firma::de(&ruta)?,
                ruta,
                abrir: Abrir::Mensaje {
                    proyecto: (!f.es_guardados()).then(|| f.id.clone()),
                    codigo: String::new(),
                },
            })
        })
        .collect();
    let mut ix = Indice::nuevo(None);
    ix.poner_al_dia(&fuentes, &LectorDeChats);
    ix
}

/// Lo encontrado, para el modelo: numerado, con su origen, lo que dice y
/// como abrirlo con las herramientas de la app.
pub fn texto_de_hallazgos(consulta: &str, hallazgos: &[Hallazgo], aviso: Option<&str>) -> String {
    if hallazgos.is_empty() {
        let mut s =
            format!("No hay nada sobre «{consulta}» en los documentos y chats del usuario.");
        if let Some(a) = aviso {
            s.push(' ');
            s.push_str(a);
        }
        return s;
    }
    let mut s = String::new();
    for (i, h) in hallazgos.iter().enumerate() {
        let cita = Cita {
            numero: i + 1,
            hallazgo: h.clone(),
        };
        s.push_str(&format!(
            "[{}] {} — chat «{}»",
            i + 1,
            cita.titulo(),
            h.proyecto_nombre
        ));
        if let Some(f) = h
            .cuando
            .and_then(pindesk_ia::documentos::herramienta::fecha_corta)
        {
            s.push_str(&format!(", {f}"));
        }
        s.push_str(&format!("\n«{}»\n", h.fragmento.replace('\n', " ").trim()));
        match &h.abrir {
            Abrir::Fichero { ruta, pagina } => {
                s.push_str(&format!(
                    "abrir: abrir_fichero {}",
                    json!({"ruta": ruta.display().to_string()})
                ));
                if let Some(p) = pagina {
                    s.push_str(&format!(" (pág. {p})"));
                }
            }
            Abrir::Mensaje { codigo, .. } if !codigo.is_empty() => {
                s.push_str(&format!(
                    "abrir: abrir_mensaje {}",
                    json!({"codigo": codigo})
                ));
            }
            Abrir::Mensaje { .. } => s.push_str("abrir: abrir_chat"),
        }
        s.push_str("\n\n");
    }
    if let Some(a) = aviso {
        s.push_str(a);
    }
    s.trim_end().to_string()
}

/// Cuantos trozos se devuelven (el cliente tiene mas contexto que el 2B del
/// asistente, que se queda en 5).
pub const RESULTADOS: usize = 8;

/// **`buscar_en_mis_cosas`**, con la validacion del asistente.
pub fn buscar(args: &Map<String, Value>) -> Resultado {
    let p = pindesk_ia::documentos::herramienta::validar(args).map_err(|e| e.to_string())?;
    let raiz = raiz();
    let mut aviso = None;
    let alcance = match &p.proyecto {
        None => Alcance::Todo,
        Some(nombre) => {
            let buscado = pindesk_ia::documentos::texto::plano(nombre.trim());
            let ids: std::collections::HashSet<String> = almacen::Indice::leer(&raiz)
                .proyectos
                .iter()
                .filter(|f| {
                    let n = pindesk_ia::documentos::texto::plano(&nombre_de(f));
                    !buscado.is_empty() && (n == buscado || n.contains(&buscado))
                })
                .map(|f| f.id.clone())
                .collect();
            if ids.is_empty() {
                aviso = Some(format!(
                    "No hay ningun chat llamado «{nombre}»; se busco en todos."
                ));
                Alcance::Todo
            } else {
                Alcance::Proyectos(ids)
            }
        }
    };
    let mut ix = Indice::cargar(&raiz.join("cache").join("documentos"));
    if ix.tamano().0 == 0 {
        ix = indice_de_chats(&raiz);
        aviso = Some(
            [aviso.unwrap_or_default(), "(Solo se busco en los chats: el indice de documentos de PinDesk aun no esta hecho.)".into()]
                .join(" ")
                .trim()
                .to_string(),
        );
    }
    let hallazgos = ix.buscar(&p.consulta, &alcance, RESULTADOS);
    Ok(vec![Contenido::Texto(texto_de_hallazgos(
        &p.consulta,
        &hallazgos,
        aviso.as_deref(),
    ))])
}
