//! **En que orden se sincroniza** (27-oct-2026). Puerto de
//! `sincro/Prioridad.kt` de Android.
//!
//! El usuario: «añade un listado de prioridades al sincronizar, por defecto:
//! (1) las listas de tareas, primero y lo más rápido; (2) los chats
//! compartidos; (3) los archivos (PDF y demás); (4) los lienzos, textos,
//! notas y lo demás; (5) al final lo pesado: vídeos y grabaciones. Dentro de
//! cada nivel, y para los grupos (una grabación = su grupo de ficheros, su
//! peso total), lo más ligero primero, para avanzar más rápido».
//!
//! Por que importa: **una vuelta se puede cortar** (la Wi-Fi del telefono que
//! se duerme, la aplicacion que se cierra). Lo que haya pasado hasta entonces
//! queda bien puesto en los dos lados —cada cosa va por su lado y se escribe
//! entera o nada—, asi que lo que va primero es lo que llega siempre: una
//! casilla tachada no puede esperar detras de un video de 300 MB.
//!
//! La vuelta (`vuelta::una`) va en tres tiempos: **los mensajes de todos los
//! chats** (en el orden de [ordenar_chats]), **los archivos de todos juntos**
//! (en el de [ordenar]) y al final lo acordado de cada uno. Solo cambia el
//! orden de las peticiones: el otro contesta cada una por su lado.

use std::collections::{BTreeSet, HashMap};

use crate::canonico::{Json, comparar_como_kotlin};
use crate::disco::{Disco, PORTATIL, en_texto, limpio};
use crate::kotlin;

/// Los niveles, del primero al ultimo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Nivel {
    Tareas,
    Compartidos,
    Archivos,
    LienzosYNotas,
    Pesado,
}

/// Video y audio: lo pesado, al final.
const PESADOS: [&str; 18] = [
    "mp4", "m4v", "mov", "mkv", "webm", "avi", "3gp", "3gpp", "m4a", "mp3", "wav", "ogg", "oga",
    "opus", "aac", "flac", "amr", "wma",
];

/// Documentos y fotos: «los archivos (PDF y demás)».
const DOCUMENTOS: [&str; 28] = [
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "epub", "csv",
    "txt", "zip", "png", "jpg", "jpeg", "webp", "gif", "heic", "heif", "bmp", "svg", "tif", "tiff",
    "dwg", "dxf",
];

/// El nivel de un archivo solo por lo que es (sin mirar su chat).
pub fn nivel_de(rel: &str) -> Nivel {
    let nombre = rel.rsplit('/').next().unwrap_or(rel).to_lowercase();
    let sin_gz = nombre.strip_suffix(".gz").unwrap_or(&nombre);
    let ext = sin_gz.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    if PESADOS.contains(&ext) {
        Nivel::Pesado
    } else if nombre.ends_with(".excalidraw.gz")
        || nombre.ends_with(".croquis.gz")
        || rel.starts_with("tablas/")
        || rel.starts_with("notas/")
        || ext == "md"
        || ext == "json"
    {
        Nivel::LienzosYNotas
    } else if DOCUMENTOS.contains(&ext) {
        Nivel::Archivos
    } else {
        Nivel::LienzosYNotas
    }
}

/// Lo que se sabe de un archivo para ordenarlo: su ruta, lo que pesa (el
/// mayor de los dos lados), su grupo (una grabacion, un mensaje con su
/// adjunto y lo anotado; `None`: va solo), si lo nombra una lista de tareas
/// y si es de un chat compartido.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pieza {
    pub rel: String,
    pub bytes: i64,
    pub grupo: Option<String>,
    pub de_tareas: bool,
    pub compartido: bool,
}

impl Pieza {
    pub fn de(rel: &str, bytes: i64) -> Pieza {
        Pieza {
            rel: rel.into(),
            bytes,
            ..Default::default()
        }
    }
}

/// El nivel de una pieza: **lo pesado es pesado siempre** (un video de una
/// lista de tareas o de un chat compartido sigue yendo al final); lo demas
/// sube a [Nivel::Tareas] o [Nivel::Compartidos] si es de eso.
pub fn nivel(p: &Pieza) -> Nivel {
    let propio = nivel_de(&p.rel);
    if propio == Nivel::Pesado {
        Nivel::Pesado
    } else if p.de_tareas {
        Nivel::Tareas
    } else if p.compartido {
        Nivel::Compartidos
    } else {
        propio
    }
}

/// **El orden de los archivos.** Cada grupo va junto, en el nivel de su
/// pieza mas tardia (una grabacion con su video es pesada entera) y pesando
/// lo que pesan todas; los grupos de un nivel, del mas ligero al mas pesado;
/// dentro de un grupo, tambien lo ligero primero. A igual peso, por nombre,
/// para que el orden no dependa del azar.
pub fn ordenar<T>(cosas: Vec<T>, pieza: impl Fn(&T) -> Pieza) -> Vec<T> {
    if cosas.len() < 2 {
        return cosas;
    }
    let piezas: Vec<(T, Pieza)> = cosas
        .into_iter()
        .map(|c| {
            let p = pieza(&c);
            (c, p)
        })
        .collect();
    let clave = |p: &Pieza| p.grupo.clone().unwrap_or_else(|| format!("\u{0}{}", p.rel));
    let mut nivel_del_grupo: HashMap<String, Nivel> = HashMap::new();
    let mut peso_del_grupo: HashMap<String, i64> = HashMap::new();
    for (_, p) in &piezas {
        let k = clave(p);
        let n = nivel(p);
        nivel_del_grupo
            .entry(k.clone())
            .and_modify(|x| *x = (*x).max(n))
            .or_insert(n);
        *peso_del_grupo.entry(k).or_insert(0) += p.bytes.max(0);
    }
    let mut con_clave: Vec<(String, T, Pieza)> =
        piezas.into_iter().map(|(c, p)| (clave(&p), c, p)).collect();
    con_clave.sort_by(|(ka, _, a), (kb, _, b)| {
        nivel_del_grupo[ka]
            .cmp(&nivel_del_grupo[kb])
            .then(peso_del_grupo[ka].cmp(&peso_del_grupo[kb]))
            .then_with(|| comparar_como_kotlin(ka, kb))
            .then(a.bytes.cmp(&b.bytes))
            .then_with(|| comparar_como_kotlin(&a.rel, &b.rel))
    });
    con_clave.into_iter().map(|(_, c, _)| c).collect()
}

/// **El orden de los chats** para sus mensajes: primero los que tienen
/// listas de tareas, luego los compartidos con otras personas y luego los
/// demas, del mas ligero (`peso`: cuantos mensajes) al mas pesado. A
/// igualdad, por id.
pub fn ordenar_chats(
    chats: &[String],
    con_tareas: &BTreeSet<String>,
    compartidos: &BTreeSet<String>,
    peso: impl Fn(&str) -> i64,
) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for c in chats {
        if !v.contains(c) {
            v.push(c.clone());
        }
    }
    let rango = |c: &str| {
        if con_tareas.contains(c) {
            0
        } else if compartidos.contains(c) {
            1
        } else {
            2
        }
    };
    v.sort_by(|a, b| {
        rango(a)
            .cmp(&rango(b))
            .then(peso(a).cmp(&peso(b)))
            .then_with(|| comparar_como_kotlin(a, b))
    });
    v
}

/// Si un mensaje es una **lista de tareas** (`TodasLasTareas.esLista`) o una
/// **tabla del chat** (`TablasDelChat.esTabla`): sus chats van los primeros.
pub fn es_de_tareas(m: &Json) -> bool {
    kotlin::cadena(m, "clase") == Some("MINIAPP")
        && matches!(kotlin::cadena(m, "miniapp"), Some("tareas" | "tabla"))
}

/// Las paginas de una carpeta del chat (`CarpetaDePaginas.deMensaje`): un
/// mensaje `MINIAPP` con `miniapp = "carpeta"` y su documento en `texto`.
fn paginas_de_carpeta(m: &Json) -> Vec<String> {
    if kotlin::cadena(m, "clase") != Some("MINIAPP")
        || kotlin::cadena(m, "miniapp") != Some("carpeta")
    {
        return Vec::new();
    }
    let Some(doc) = kotlin::cadena(m, "texto").and_then(|t| Json::analizar(t).ok()) else {
        return Vec::new();
    };
    doc.como_objeto()
        .and_then(|o| o.obtener("paginas"))
        .and_then(Json::como_lista)
        .map(|l| {
            l.iter()
                .filter_map(|p| kotlin::cadena(p, "id").map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Lo que nombra un mensaje (portatil): su adjunto, lo que senala por su
/// clase, las rutas de su texto y lo anotado sobre el.
fn referencias(
    m: &Json,
    anotado: &std::collections::BTreeMap<String, Vec<String>>,
) -> BTreeSet<String> {
    let mut s: BTreeSet<String> = en_texto(&m.a_texto()).into_iter().collect();
    if let Some(r) = kotlin::cadena(m, "ruta").and_then(|r| r.strip_prefix(PORTATIL)) {
        s.insert(r.to_string());
    }
    let referencia = kotlin::cadena(m, "referencia");
    let id = kotlin::cadena(m, "id").unwrap_or_default();
    match kotlin::cadena(m, "clase").unwrap_or_default() {
        "DIBUJO" | "PAGINA" => {
            if let Some(r) = referencia {
                s.insert(format!("pins/draw/{r}.excalidraw.gz"));
            }
        }
        "IMAGEN" => {
            let foto = format!("foto-{id}");
            s.insert(format!(
                "pins/draw/{}.excalidraw.gz",
                referencia.unwrap_or(&foto)
            ));
        }
        "TABLA" => {
            if let Some(r) = referencia {
                s.insert(format!("tablas/{}.json", limpio(r)));
            }
        }
        "CROQUIS" => {
            if let Some(r) = referencia {
                s.insert(format!("croquis3d/{r}.croquis.gz"));
            }
        }
        _ => {}
    }
    if crate::anotado::lleva_anotado(m)
        && let Some(l) = anotado.get(&kotlin::unico(m))
    {
        s.extend(l.iter().cloned());
    }
    let marcas: Vec<String> = s
        .iter()
        .flat_map(|r| {
            crate::anotado::marcas_junto_a(r)
                .into_iter()
                .chain(crate::anotado::vistas_junto_a(r))
        })
        .collect();
    s.extend(marcas);
    s
}

/// **Los grupos de archivos de un chat** (`Disco.gruposDeArchivos`): lo de
/// una carpeta del chat (una clase grabada: su voz, sus capturas y su video)
/// va junto, con la clave de la carpeta; lo de un mensaje suelto (su adjunto
/// y lo anotado), con la del mensaje. Y si lo nombra una lista de tareas (o
/// una tabla del chat). Ruta portatil → (grupo, de tareas).
pub fn grupos_de_archivos<D: Disco + ?Sized>(d: &D, chat: &str) -> HashMap<String, (String, bool)> {
    let Ok(mensajes) = d.mensajes(chat) else {
        return HashMap::new();
    };
    let lista_anotado = d.anotado(chat);
    let anotado = crate::anotado::por_uid(lista_anotado.iter().map(String::as_str));
    let ids: BTreeSet<String> = mensajes
        .iter()
        .filter_map(|m| kotlin::cadena(m, "id").map(str::to_string))
        .collect();
    let mut grupo_de: HashMap<String, String> = HashMap::new();
    for m in &mensajes {
        let id = kotlin::cadena(m, "id").unwrap_or_default();
        for p in paginas_de_carpeta(m) {
            if ids.contains(&p) {
                grupo_de.entry(p).or_insert_with(|| format!("carpeta:{id}"));
            }
        }
    }
    let mut salida: HashMap<String, (String, bool)> = HashMap::new();
    for m in &mensajes {
        let id = kotlin::cadena(m, "id").unwrap_or_default().to_string();
        let tareas = es_de_tareas(m);
        let grupo = grupo_de
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("mensaje:{id}"));
        for r in referencias(m, &anotado) {
            let antes = salida.get(&r).cloned();
            let g = antes.as_ref().map_or(grupo.clone(), |a| a.0.clone());
            let t = tareas || antes.is_some_and(|a| a.1);
            salida.insert(r, (g, t));
        }
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_archivo_en_su_nivel() {
        assert_eq!(nivel_de("guardados/clase_1.mp4"), Nivel::Pesado);
        assert_eq!(nivel_de("guardados/voz.M4A"), Nivel::Pesado);
        assert_eq!(nivel_de("guardados/informe (1).pdf"), Nivel::Archivos);
        assert_eq!(nivel_de("guardados/foto.jpg"), Nivel::Archivos);
        assert_eq!(nivel_de("pins/draw/a.excalidraw.gz"), Nivel::LienzosYNotas);
        assert_eq!(nivel_de("tablas/t.json"), Nivel::LienzosYNotas);
        assert_eq!(nivel_de("notas/n.md"), Nivel::LienzosYNotas);
        assert_eq!(nivel_de("pins/draw/d1.marcas"), Nivel::LienzosYNotas);
        assert_eq!(nivel_de("voz/nota.m4a"), Nivel::Pesado);
    }

    #[test]
    fn tareas_compartidos_archivos_lienzos_y_lo_pesado_al_final_lo_ligero_primero() {
        let piezas = vec![
            Pieza::de("guardados/video.mp4", 50_000_000),
            Pieza::de("pins/draw/grande.excalidraw.gz", 900_000),
            Pieza::de("pins/draw/chico.excalidraw.gz", 1_000),
            Pieza::de("guardados/grande.pdf", 5_000_000),
            Pieza::de("guardados/chico.pdf", 10_000),
            Pieza {
                de_tareas: true,
                ..Pieza::de("tablas/tareas.json", 2_000)
            },
            Pieza {
                compartido: true,
                ..Pieza::de("guardados/de-ana.pdf", 3_000_000)
            },
            Pieza {
                compartido: true,
                ..Pieza::de("guardados/video-de-ana.mp4", 1_000)
            },
        ];
        let orden: Vec<String> = ordenar(piezas, Clone::clone)
            .into_iter()
            .map(|p| p.rel)
            .collect();
        assert_eq!(
            orden,
            [
                "tablas/tareas.json",
                "guardados/de-ana.pdf",
                "guardados/chico.pdf",
                "guardados/grande.pdf",
                "pins/draw/chico.excalidraw.gz",
                "pins/draw/grande.excalidraw.gz",
                // Lo pesado es pesado aunque sea de un chat compartido.
                "guardados/video-de-ana.mp4",
                "guardados/video.mp4",
            ]
        );
    }

    #[test]
    fn una_grabacion_va_junta_pesada_entera_y_por_lo_que_pesa_toda() {
        let g = |rel: &str, bytes: i64, grupo: Option<&str>| Pieza {
            grupo: grupo.map(str::to_string),
            ..Pieza::de(rel, bytes)
        };
        let piezas = vec![
            // Clase A: 10 MB en total (voz, capturas y video).
            g("guardados/a_voz.m4a", 2_000_000, Some("A")),
            g("guardados/a_cap1.png", 100_000, Some("A")),
            g("guardados/a_video.mp4", 7_900_000, Some("A")),
            // Clase B: 3 MB.
            g("guardados/b_voz.m4a", 2_900_000, Some("B")),
            g("guardados/b_cap1.png", 100_000, Some("B")),
            // Un video suelto de 5 MB: entre las dos.
            g("guardados/suelto.mp4", 5_000_000, None),
            g("guardados/doc.pdf", 50_000, None),
        ];
        let orden: Vec<String> = ordenar(piezas, Clone::clone)
            .into_iter()
            .map(|p| p.rel)
            .collect();
        assert_eq!(
            orden,
            [
                "guardados/doc.pdf",
                "guardados/b_cap1.png",
                "guardados/b_voz.m4a",
                "guardados/suelto.mp4",
                "guardados/a_cap1.png",
                "guardados/a_voz.m4a",
                "guardados/a_video.mp4",
            ]
        );
    }

    #[test]
    fn los_chats_los_de_tareas_los_compartidos_y_luego_del_mas_ligero_al_mas_pesado() {
        let peso: HashMap<&str, i64> = [
            ("general", 500),
            ("obra", 20),
            ("casa", 5),
            ("ana", 900),
            ("tareas", 1000),
        ]
        .into();
        let chats: Vec<String> = ["general", "obra", "casa", "ana", "tareas"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            ordenar_chats(
                &chats,
                &BTreeSet::from(["tareas".to_string()]),
                &BTreeSet::from(["ana".to_string()]),
                |c| peso[c]
            ),
            ["tareas", "ana", "casa", "obra", "general"]
        );
    }

    #[test]
    fn una_lista_de_tareas_y_una_carpeta_de_grabacion_se_reconocen() {
        let m = |t: &str| Json::analizar(t).unwrap();
        assert!(es_de_tareas(&m(
            r#"{"clase":"MINIAPP","miniapp":"tareas"}"#
        )));
        assert!(es_de_tareas(&m(r#"{"clase":"MINIAPP","miniapp":"tabla"}"#)));
        assert!(!es_de_tareas(&m(
            r#"{"clase":"MINIAPP","miniapp":"carpeta"}"#
        )));
        assert!(!es_de_tareas(&m(r#"{"clase":"NOTA","miniapp":"tareas"}"#)));
        let carpeta = m(
            r#"{"clase":"MINIAPP","miniapp":"carpeta","texto":"{\"formato\":1,\"paginas\":[{\"id\":\"1\"},{\"id\":\"2\"}]}"}"#,
        );
        assert_eq!(paginas_de_carpeta(&carpeta), ["1", "2"]);
    }
}
