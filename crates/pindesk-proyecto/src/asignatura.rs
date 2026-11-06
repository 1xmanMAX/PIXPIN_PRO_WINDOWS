//! **Un solo chat para todas las asignaturas** (6-nov-2026).
//!
//! El usuario: «ademas para las asignaturas que se cree un chat en la que se
//! agrupen todas y ahi se vaya almacenando todo de estas, solo 1 para
//! todos». Desde el 18-oct cada asignatura del horario ya tenia su lista de
//! tareas en el chat «Asignaturas» (`horario::asignaturas` de la app); ahora
//! ese chat es ademas **donde va todo lo de las asignaturas**: las clases
//! grabadas, lo que se le manda desde otro chat («Enviar a asignatura…») y
//! lo que se escribe o se suelta en el con una asignatura elegida arriba.
//!
//! Lo puro esta aqui (sin ventanas, se prueba en Linux); la app lo usa en el
//! chat, en «Grabar clase» y en el vigia del horario.
//!
//! ## La etiqueta: `resto["asignatura"]`
//!
//! Cada mensaje que es de una asignatura lleva su **nombre** en
//! `"asignatura": "Geometría"`, un campo mas del mensaje. Viaja en `resto`
//! como `escaneado`, `enGrabacion` o `capturas`: el movil lo guarda tal cual
//! aunque no lo entienda (regla 1 de sincronizar), y uno que no lo sepa ve
//! el chat como siempre, todo junto. Sin el campo, el mensaje no es de
//! ninguna (el de antes sale igual que antes).
//!
//! Por que el nombre y no un codigo: el horario no viaja (es de este
//! equipo), asi que el otro aparato no tendria con que traducir un codigo;
//! el nombre se lee solo. Renombrar la asignatura renombra la etiqueta de
//! sus mensajes ([`renombrar`]), como ya renombraba su lista. Se compara sin
//! mirar mayusculas ni tildes ([`mismo`]).
//!
//! La lista de tareas de una asignatura no necesita etiqueta: es de la
//! asignatura cuyo nombre lleva ([`de_en_el_chat`]).
//!
//! ## Nunca se pierde nada
//!
//! Quitar una asignatura del horario no toca sus mensajes: siguen con su
//! etiqueta y su chip sale igual, en gris ([`chips`]). Mandar algo a una
//! asignatura **copia** (el original se queda donde estaba) y mandarlo dos
//! veces no lo duplica ([`ya_esta`]).

use std::collections::{HashMap, HashSet};

use pindesk_frase::fechas::normalizar;

use crate::almacen::Ficha;
use crate::carpeta_de_paginas::{self, Carpeta};
use crate::cuaderno::{Clase, Mensaje};

/// El campo del mensaje con el nombre de su asignatura.
pub const CLAVE: &str = "asignatura";
/// La marca del chat en su ficha del indice (`"asignaturas": true`).
pub const MARCA_CHAT: &str = "asignaturas";
/// Como se llama el chat al nacer.
pub const NOMBRE_CHAT: &str = "Asignaturas";
/// Su foto al nacer: el libro azul sobre su degradado.
pub const EMOJI: &str = "📘";
pub const FONDO: (&str, &str) = ("#5B9BFF", "#2F6FE0");
/// Los minutos antes de una clase en que una grabacion ya es de ella: quien
/// llega antes abre la grabadora antes de que empiece.
pub const ANTES_DE_CLASE: u32 = 10;

/// Si una ficha es la del chat «Asignaturas» (su marca).
pub fn es_el_chat(f: &Ficha) -> bool {
    f.resto.get(MARCA_CHAT).and_then(|v| v.as_bool()) == Some(true)
}

/// Dos nombres de asignatura son la misma: sin mirar mayusculas, tildes ni
/// espacios de los lados.
pub fn mismo(a: &str, b: &str) -> bool {
    normalizar(a.trim()) == normalizar(b.trim())
}

/// La asignatura que lleva puesta un mensaje, si lleva.
pub fn de(m: &Mensaje) -> Option<&str> {
    m.resto
        .get(CLAVE)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Le pone la asignatura a un mensaje (vacio la quita). Devuelve si cambio.
pub fn poner(m: &mut Mensaje, nombre: &str) -> bool {
    let nombre = nombre.trim();
    if nombre.is_empty() {
        return quitar(m);
    }
    if de(m) == Some(nombre) {
        return false;
    }
    m.resto
        .insert(CLAVE.into(), serde_json::Value::String(nombre.into()));
    true
}

/// Se la quita. Devuelve si la tenia.
pub fn quitar(m: &mut Mensaje) -> bool {
    m.resto.remove(CLAVE).is_some()
}

fn es_lista_de_tareas(m: &Mensaje) -> bool {
    m.clase == Some(Clase::MiniApp) && m.miniapp.as_deref() == Some(crate::mini::TAREAS)
}

/// **De que asignatura es un mensaje del chat «Asignaturas»**: la que lleva
/// puesta o, en una lista de tareas sin etiqueta, la de su nombre si es una
/// de `asignaturas` (la lista de cada asignatura se llama como ella).
pub fn de_en_el_chat<'a>(m: &'a Mensaje, asignaturas: &'a [Chip]) -> Option<&'a str> {
    if let Some(a) = de(m) {
        return Some(a);
    }
    if !es_lista_de_tareas(m) {
        return None;
    }
    let titulo = match crate::mini::titulo(&m.texto) {
        t if t.trim().is_empty() => m.nombre.clone(),
        t => t,
    };
    asignaturas
        .iter()
        .find(|c| mismo(&c.nombre, &titulo))
        .map(|c| c.nombre.as_str())
}

/// Un chip de la fila de arriba del chat: una asignatura y su color.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    pub nombre: String,
    /// `0xRRGGBB` de la asignatura del horario; `None` si ya no esta en el
    /// horario (se quito, o la etiqueta llego de otro aparato): sale en gris.
    pub color: Option<u32>,
}

/// Las asignaturas del horario como chips, `(nombre, 0xRRGGBB)` en su
/// orden y sin repetir.
pub fn del_horario(pares: &[(String, u32)]) -> Vec<Chip> {
    let mut v: Vec<Chip> = Vec::new();
    for (n, c) in pares {
        if !n.trim().is_empty() && !v.iter().any(|x| mismo(&x.nombre, n)) {
            v.push(Chip {
                nombre: n.trim().to_string(),
                color: Some(*c),
            });
        }
    }
    v
}

/// **Los chips de la fila de las asignaturas**: las del horario en su orden
/// (con su color, [`del_horario`]) y, detras, las que solo nombran los
/// mensajes (se quitaron del horario: su material sigue ahi y tiene que
/// poder verse), sin repetir.
pub fn chips(del_horario: &[Chip], mensajes: &[Mensaje]) -> Vec<Chip> {
    let mut v: Vec<Chip> = del_horario.to_vec();
    for m in mensajes {
        if let Some(n) = de(m)
            && !v.iter().any(|x| mismo(&x.nombre, n))
        {
            v.push(Chip {
                nombre: n.to_string(),
                color: None,
            });
        }
    }
    v
}

/// **Si un mensaje pasa el filtro de una asignatura** (el chip puesto).
/// `desde` y `aparato`: lo que se esta añadiendo ahora mismo en este equipo
/// con el chip puesto aun no lleva la etiqueta ([`por_etiquetar`] se la pone
/// en la siguiente vuelta), y no puede desaparecer al soltarlo.
pub fn pasa(m: &Mensaje, filtro: &str, asignaturas: &[Chip], aparato: &str, desde: i64) -> bool {
    match de_en_el_chat(m, asignaturas) {
        Some(a) => mismo(a, filtro),
        None => recien_puesto(m, aparato, desde),
    }
}

fn recien_puesto(m: &Mensaje, aparato: &str, desde: i64) -> bool {
    m.cuando >= desde && m.aparato.as_deref() == Some(aparato) && !crate::cuaderno::es_de_sistema(m)
}

/// **Lo nuevo que hay que etiquetar**: con una asignatura elegida arriba
/// del chat «Asignaturas», lo que se escribe, se graba o se suelta en el es
/// de ella. Por su posicion: lo de este equipo (lo que llega del movil ya
/// trae lo suyo), sin etiqueta y puesto desde que se eligio.
pub fn por_etiquetar(mensajes: &[Mensaje], aparato: &str, desde: i64) -> Vec<usize> {
    mensajes
        .iter()
        .enumerate()
        .filter(|(_, m)| de(m).is_none() && recien_puesto(m, aparato, desde))
        .map(|(i, _)| i)
        .collect()
}

/// **Renombra la etiqueta** de lo que era de `viejo` (la asignatura se
/// renombro en el horario). Devuelve los que cambiaron, por su posicion.
pub fn renombrar(mensajes: &mut [Mensaje], viejo: &str, nuevo: &str) -> Vec<usize> {
    if nuevo.trim().is_empty() || viejo.trim().is_empty() {
        return Vec::new();
    }
    let mut v = Vec::new();
    for (i, m) in mensajes.iter_mut().enumerate() {
        if de(m).is_some_and(|a| mismo(a, viejo) && a != nuevo.trim()) {
            poner(m, nuevo);
            v.push(i);
        }
    }
    v
}

/// **Lo que va con lo elegido** al mandarlo a una asignatura: una carpeta
/// (la de una clase grabada, o de paginas) se lleva sus paginas, que si no
/// llegaria vacia. Por su posicion, sin repetir y en el orden del chat.
pub fn paquete(mensajes: &[Mensaje], elegidos: &[usize]) -> Vec<usize> {
    let mut v: Vec<usize> = Vec::new();
    let mut vistos = HashSet::new();
    let mut meter = |i: usize, v: &mut Vec<usize>| {
        if i < mensajes.len() && vistos.insert(i) {
            v.push(i);
        }
    };
    for &i in elegidos {
        meter(i, &mut v);
        if let Some(c) = mensajes
            .get(i)
            .filter(|m| carpeta_de_paginas::es_carpeta(m))
            .and_then(Carpeta::de_mensaje)
        {
            for (j, _) in c.vivas(mensajes) {
                meter(j, &mut v);
            }
        }
    }
    v.sort_by_key(|&i| (mensajes[i].cuando, i));
    v
}

/// **Cambia los `id` de un paquete copiado** por los de sus copias: la
/// carpeta apunta a sus paginas por su `id` (en su `texto`) y la nota de una
/// clase a sus capturas, su video y sus clips (en `resto`: `capturas`,
/// `video`, `clips`, `enGrabacion`). Cualquier texto de esos campos que sea
/// exactamente uno de los `id` viejos pasa a ser el nuevo; lo demas, igual.
pub fn remapear(m: &mut Mensaje, mapa: &HashMap<String, String>) {
    if mapa.is_empty() {
        return;
    }
    for v in m.resto.values_mut() {
        cambiar_ids(v, mapa);
    }
    if carpeta_de_paginas::es_carpeta(m)
        && let Ok(mut doc) = serde_json::from_str::<serde_json::Value>(&m.texto)
    {
        cambiar_ids(&mut doc, mapa);
        if let Ok(t) = serde_json::to_string(&doc) {
            m.texto = t;
        }
    }
}

fn cambiar_ids(v: &mut serde_json::Value, mapa: &HashMap<String, String>) {
    match v {
        serde_json::Value::String(s) => {
            if let Some(n) = mapa.get(s.as_str()) {
                *s = n.clone();
            }
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(|x| cambiar_ids(x, mapa)),
        serde_json::Value::Object(o) => o.values_mut().for_each(|x| cambiar_ids(x, mapa)),
        _ => {}
    }
}

/// De donde sale una copia: lo que se apunta en su `origen`.
pub fn origen_de(m: &Mensaje) -> String {
    m.uid.clone().unwrap_or_else(|| m.id.clone())
}

/// **Si `m` ya se mando aqui** (una copia suya, por su `origen`): mandarlo
/// otra vez no lo duplica.
pub fn ya_esta<'a>(alli: &'a [Mensaje], m: &Mensaje) -> Option<&'a Mensaje> {
    let o = origen_de(m);
    alli.iter()
        .find(|x| x.origen.as_deref() == Some(o.as_str()))
}

/// Una clase del horario, lo que hace falta para saber si se esta en ella.
#[derive(Debug, Clone, Copy)]
pub struct Tramo<'a> {
    /// 0 lunes … 6 domingo.
    pub dias: &'a [u8],
    /// Minutos del dia.
    pub inicio: u32,
    pub fin: u32,
    pub nombre: &'a str,
}

/// **La asignatura de la clase de ahora**: la que esta en marcha o, si no
/// hay, la que empieza dentro de [`ANTES_DE_CLASE`] minutos. `dia` es el de
/// la semana (0 lunes) y `minuto` el del dia, en hora local.
pub fn en_clase<'a>(tramos: &[Tramo<'a>], dia: u8, minuto: u32) -> Option<&'a str> {
    let hoy = |t: &&Tramo| t.dias.contains(&dia);
    tramos
        .iter()
        .filter(hoy)
        .find(|t| t.inicio <= minuto && minuto < t.fin)
        .or_else(|| {
            tramos
                .iter()
                .filter(hoy)
                .filter(|t| t.inicio > minuto && t.inicio - minuto <= ANTES_DE_CLASE)
                .min_by_key(|t| t.inicio)
        })
        .map(|t| t.nombre)
}

/// Que chat estaba abierto al pulsar «Grabar clase».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abierto<'a> {
    /// Ninguno: la bandeja, el atajo, Flow Launcher.
    Ninguno,
    /// «Mensajes guardados» o el «Asistente»: no se eligieron para guardar.
    DeLaApp,
    /// El chat «Asignaturas», con el chip de una asignatura puesto o no.
    Asignaturas(Option<&'a str>),
    /// Otro chat cualquiera, abierto a proposito.
    Otro,
}

/// Adonde va una grabacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adonde {
    /// Al chat que estaba abierto.
    ElAbierto,
    /// Al chat «Asignaturas».
    Asignaturas,
    /// Como siempre: el ultimo chat usado primero.
    ComoSiempre,
}

/// **La regla de las clases grabadas** (ver
/// `docs/novedades-2026-11-06-un-chat-para-las-asignaturas.md`):
///
/// 1. Grabando desde **otro chat abierto a proposito**, se queda en el (lo
///    eligio el usuario); si es la hora de una clase, lleva su asignatura,
///    y «Enviar a asignatura…» la copia al chat de las asignaturas.
/// 2. Desde el chat **«Asignaturas»**, ahi; con el chip de una asignatura
///    puesto, de esa (lo eligio el usuario), y si no, de la clase de ahora.
/// 3. Desde **cualquier otro sitio** (la bandeja, el atajo, Flow, «Mensajes
///    guardados»), en la hora de una clase va al chat «Asignaturas» con su
///    asignatura; fuera de clase, como siempre.
///
/// El chat se puede cambiar en la grabadora antes de empezar; la asignatura
/// va con la grabacion a donde se guarde.
pub fn adonde_la_grabacion(abierto: Abierto, en_clase: Option<&str>) -> (Adonde, Option<String>) {
    let clase = en_clase.map(str::to_string);
    match abierto {
        Abierto::Otro => (Adonde::ElAbierto, clase),
        Abierto::Asignaturas(Some(f)) => (Adonde::Asignaturas, Some(f.to_string())),
        Abierto::Asignaturas(None) => (Adonde::Asignaturas, clase),
        Abierto::Ninguno | Abierto::DeLaApp => match clase {
            Some(c) => (Adonde::Asignaturas, Some(c)),
            None => (Adonde::ComoSiempre, None),
        },
    }
}

/// **Las asignaturas que ofrece «Enviar a asignatura…»**: la que ya lleva
/// el mensaje primero (es lo mas probable), luego las demas en su orden.
pub fn para_enviar(chips: &[Chip], puesta: Option<&str>) -> Vec<String> {
    let mut v: Vec<String> = chips.iter().map(|c| c.nombre.clone()).collect();
    if let Some(p) = puesta
        && let Some(i) = v.iter().position(|n| mismo(n, p))
    {
        let n = v.remove(i);
        v.insert(0, n);
    }
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn nota(id: &str, cuando: i64) -> Mensaje {
        Mensaje {
            id: id.into(),
            cuando,
            clase: Some(Clase::Nota),
            texto: format!("nota {id}"),
            aparato: Some("PC01".into()),
            ..Default::default()
        }
    }

    fn con(mut m: Mensaje, a: &str) -> Mensaje {
        poner(&mut m, a);
        m
    }

    fn lista(id: &str, titulo: &str) -> Mensaje {
        Mensaje {
            id: id.into(),
            clase: Some(Clase::MiniApp),
            miniapp: Some(crate::mini::TAREAS.into()),
            texto: crate::mini::escribir_tareas(titulo, &[]),
            nombre: titulo.into(),
            ..Default::default()
        }
    }

    #[test]
    fn la_etiqueta_se_pone_se_lee_y_se_quita_y_viaja_en_resto() {
        let mut m = nota("1", 1);
        assert_eq!(de(&m), None);
        assert!(poner(&mut m, "  Geometría "));
        assert_eq!(de(&m), Some("Geometría"));
        assert!(!poner(&mut m, "Geometría"), "la misma: nada");
        // Sale en el JSON como un campo mas y vuelve al leerlo.
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains(r#""asignatura":"Geometría""#), "{json}");
        let leido: Mensaje = serde_json::from_str(&json).unwrap();
        assert_eq!(de(&leido), Some("Geometría"));
        // Vacio es quitarla.
        assert!(poner(&mut m, "   "));
        assert_eq!(de(&m), None);
        assert!(!quitar(&mut m));
        // Caso negativo: un mensaje de antes, sin el campo, sale igual.
        let antes = serde_json::to_string(&nota("2", 2)).unwrap();
        assert!(!antes.contains("asignatura"));
    }

    #[test]
    fn los_chips_son_las_del_horario_y_las_que_solo_quedan_en_el_chat() {
        let horario = del_horario(&[
            ("Geometría".to_string(), 0xFF0000),
            ("Física".into(), 0x00FF00),
            ("geometria".into(), 0x0000FF),
        ]);
        let ms = vec![
            con(nota("1", 1), "geometria"),
            con(nota("2", 2), "Historia"),
            con(nota("3", 3), "historia"),
            nota("4", 4),
        ];
        assert_eq!(
            chips(&horario, &ms),
            vec![
                Chip {
                    nombre: "Geometría".into(),
                    color: Some(0xFF0000)
                },
                Chip {
                    nombre: "Física".into(),
                    color: Some(0x00FF00)
                },
                // Quitada del horario: su material sigue y su chip tambien.
                Chip {
                    nombre: "Historia".into(),
                    color: None
                },
            ]
        );
        // Caso negativo: sin horario ni etiquetas, ningun chip.
        assert!(chips(&[], &[nota("1", 1)]).is_empty());
    }

    #[test]
    fn el_filtro_deja_lo_suyo_su_lista_y_lo_que_se_acaba_de_poner() {
        let cs = del_horario(&[("Geometría".into(), 1), ("Física".into(), 2)]);
        let geo = con(nota("1", 10), "Geometría");
        let fis = con(nota("2", 20), "Física");
        let suelta = nota("3", 30);
        let su_lista = lista("4", "geometría");
        let otra_lista = lista("5", "Compras");
        let pasan = |m: &Mensaje| pasa(m, "geometria", &cs, "PC01", 1_000);
        assert!(pasan(&geo));
        assert!(pasan(&su_lista), "la lista de la asignatura, por su nombre");
        assert!(!pasan(&fis));
        assert!(!pasan(&suelta), "lo de antes sin etiqueta no");
        assert!(!pasan(&otra_lista));
        // Lo que se acaba de escribir aqui con el chip puesto si se ve...
        let nueva = nota("6", 1_500);
        assert!(pasan(&nueva));
        assert_eq!(
            por_etiquetar(&[suelta.clone(), nueva.clone()], "PC01", 1_000),
            vec![1]
        );
        // ...y lo que llega del movil en ese rato, no: no lo puso nadie aqui.
        let del_movil = Mensaje {
            aparato: Some("MOVIL".into()),
            ..nota("7", 1_600)
        };
        assert!(!pasan(&del_movil));
        assert!(por_etiquetar(&[del_movil], "PC01", 1_000).is_empty());
    }

    #[test]
    fn renombrar_la_asignatura_renombra_su_etiqueta_y_nada_mas() {
        let mut ms = vec![
            con(nota("1", 1), "Física"),
            con(nota("2", 2), "fisica"),
            con(nota("3", 3), "Geometría"),
            nota("4", 4),
        ];
        assert_eq!(renombrar(&mut ms, "Física", "Física II"), vec![0, 1]);
        assert_eq!(de(&ms[0]), Some("Física II"));
        assert_eq!(de(&ms[1]), Some("Física II"));
        assert_eq!(de(&ms[2]), Some("Geometría"));
        assert_eq!(de(&ms[3]), None);
        // Otra vez: ya esta.
        assert!(renombrar(&mut ms, "Física II", "Física II").is_empty());
        // Caso negativo: a nada no se renombra (no se pierde la etiqueta).
        assert!(renombrar(&mut ms, "Física II", "  ").is_empty());
        assert_eq!(de(&ms[0]), Some("Física II"));
    }

    #[test]
    fn una_carpeta_se_manda_con_sus_paginas_y_sus_ids_nuevos() {
        let voz = Mensaje {
            clase: Some(Clase::Voz),
            ..nota("100", 100)
        };
        let mut voz = voz;
        voz.resto.insert(
            "capturas".into(),
            serde_json::json!([{ "ms": 5000, "id": "101" }]),
        );
        let mut foto = nota("101", 101);
        foto.resto.insert(
            "enGrabacion".into(),
            serde_json::json!({ "de": "100", "ms": 5000 }),
        );
        let mut c = Carpeta::grabacion("100", 60_000);
        c.anadir_al_minuto("100", "Nota", 0);
        c.anadir_al_minuto("101", "Captura", 5000);
        let carpeta = carpeta_de_paginas::mensaje_de_grabacion(
            "Clase",
            &c,
            &crate::cuaderno::Sello {
                cuando: 102,
                numero: 3,
                aparato: "PC01".into(),
                proyecto: "p".into(),
            },
        );
        let otra = nota("50", 50);
        let ms = vec![otra, voz, foto, carpeta.clone()];
        // Elegida la carpeta: van ella, la voz y la captura, en su orden.
        assert_eq!(paquete(&ms, &[3]), vec![1, 2, 3]);
        // Una nota suelta va sola.
        assert_eq!(paquete(&ms, &[0]), vec![0]);
        // Con sus ids nuevos, los enlaces siguen apuntando a lo suyo.
        let mapa: HashMap<String, String> = [("100", "900"), ("101", "901")]
            .into_iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        let mut v = ms[1].clone();
        remapear(&mut v, &mapa);
        assert_eq!(v.resto["capturas"][0]["id"], "901");
        assert_eq!(v.resto["capturas"][0]["ms"], 5000, "lo demas igual");
        let mut f = ms[2].clone();
        remapear(&mut f, &mapa);
        assert_eq!(f.resto["enGrabacion"]["de"], "900");
        let mut k = carpeta;
        remapear(&mut k, &mapa);
        let leida = Carpeta::de_mensaje(&k).unwrap();
        let ids: Vec<&str> = leida.paginas.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["900", "901"]);
        assert_eq!(leida.voz, "900", "y su nota");
        // Caso negativo: un texto que no es un id del paquete no cambia.
        let mut n = nota("7", 7);
        n.resto.insert("x".into(), serde_json::json!("1000"));
        remapear(&mut n, &mapa);
        assert_eq!(n.resto["x"], "1000");
    }

    #[test]
    fn mandarlo_dos_veces_no_lo_duplica() {
        let mut m = nota("1", 1);
        m.uid = Some("U1".into());
        let mut copia = nota("9", 9);
        copia.origen = Some("U1".into());
        assert!(ya_esta(std::slice::from_ref(&copia), &m).is_some());
        // Caso negativo: otra cosa no esta.
        let otro = nota("2", 2);
        assert!(ya_esta(&[copia], &otro).is_none());
    }

    #[test]
    fn en_clase_es_la_de_ahora_o_la_que_empieza_en_diez_minutos() {
        let lun_mie = [0u8, 2];
        let mar = [1u8];
        let t = [
            Tramo {
                dias: &lun_mie,
                inicio: 8 * 60,
                fin: 10 * 60,
                nombre: "Geometría",
            },
            Tramo {
                dias: &lun_mie,
                inicio: 10 * 60 + 15,
                fin: 12 * 60,
                nombre: "Física",
            },
            Tramo {
                dias: &mar,
                inicio: 9 * 60,
                fin: 11 * 60,
                nombre: "Arte",
            },
        ];
        assert_eq!(en_clase(&t, 0, 9 * 60), Some("Geometría"));
        assert_eq!(
            en_clase(&t, 0, 7 * 60 + 52),
            Some("Geometría"),
            "8 min antes"
        );
        // A las 10:05 ya acabo Geometría y Física empieza en 10 min.
        assert_eq!(en_clase(&t, 2, 10 * 60 + 5), Some("Física"));
        assert_eq!(en_clase(&t, 1, 10 * 60), Some("Arte"));
        // Casos negativos: muy pronto, otro dia, ya acabada.
        assert_eq!(en_clase(&t, 0, 7 * 60 + 30), None);
        assert_eq!(en_clase(&t, 4, 9 * 60), None);
        assert_eq!(en_clase(&t, 0, 12 * 60), None);
    }

    #[test]
    fn la_regla_de_las_grabaciones() {
        use Abierto::*;
        let g = Some("Geometría");
        assert_eq!(
            adonde_la_grabacion(Otro, g),
            (Adonde::ElAbierto, Some("Geometría".into()))
        );
        assert_eq!(adonde_la_grabacion(Otro, None), (Adonde::ElAbierto, None));
        assert_eq!(
            adonde_la_grabacion(Asignaturas(Some("Física")), g),
            (Adonde::Asignaturas, Some("Física".into())),
            "el chip elegido manda sobre el horario"
        );
        assert_eq!(
            adonde_la_grabacion(Asignaturas(None), g),
            (Adonde::Asignaturas, Some("Geometría".into()))
        );
        assert_eq!(
            adonde_la_grabacion(Asignaturas(None), None),
            (Adonde::Asignaturas, None)
        );
        assert_eq!(
            adonde_la_grabacion(Ninguno, g),
            (Adonde::Asignaturas, Some("Geometría".into()))
        );
        assert_eq!(
            adonde_la_grabacion(DeLaApp, g),
            (Adonde::Asignaturas, Some("Geometría".into()))
        );
        // Caso negativo: fuera de clase y sin chat elegido, como siempre.
        assert_eq!(
            adonde_la_grabacion(Ninguno, None),
            (Adonde::ComoSiempre, None)
        );
    }

    #[test]
    fn el_chat_va_fijo_detras_de_guardados_y_el_asistente() {
        let ficha = |nombre: &str, tocado: i64, marca: Option<&str>| {
            let mut f = Ficha::nueva(nombre, tocado, "PC01");
            if let Some(k) = marca {
                f.resto.insert(k.into(), serde_json::Value::Bool(true));
            }
            f
        };
        let mut i = crate::almacen::Indice::default();
        i.proyectos.push(ficha("Obra", 50, None));
        i.proyectos.push(ficha("Asignaturas", 1, Some(MARCA_CHAT)));
        i.proyectos.push(ficha("Asistente", 2, Some("asistente")));
        i.proyectos
            .push(ficha("Mensajes guardados", 3, Some("guardados")));
        i.proyectos.push(ficha("Casa", 40, None));
        let nombres: Vec<&str> = i.ordenadas().iter().map(|f| f.nombre.as_str()).collect();
        assert_eq!(
            nombres,
            [
                "Mensajes guardados",
                "Asistente",
                "Asignaturas",
                "Obra",
                "Casa"
            ]
        );
        assert!(i.proyectos[1].es_asignaturas());
        // Caso negativo: uno que solo se llama asi no es el (sin su marca).
        assert!(!ficha("Asignaturas", 1, None).es_asignaturas());
    }

    #[test]
    fn enviar_ofrece_primero_la_que_ya_lleva() {
        let cs = del_horario(&[("A".into(), 1), ("B".into(), 2), ("C".into(), 3)]);
        assert_eq!(para_enviar(&cs, Some("c")), vec!["C", "A", "B"]);
        assert_eq!(para_enviar(&cs, None), vec!["A", "B", "C"]);
        assert_eq!(para_enviar(&cs, Some("Z")), vec!["A", "B", "C"]);
    }
}
