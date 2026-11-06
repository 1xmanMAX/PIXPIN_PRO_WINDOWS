//! **La revision de urgencia que ya se hizo, guardada y de viaje**
//! (4-nov-2026, Android v0.161.0, `jev/UrgenciasGuardadas.kt`). El usuario:
//! «si una tarea ya fue revisada, que no se vuelva a hacer; lo revisado en el
//! PC se sincroniza al celular y lo del celular al PC; y sin gastar tokens».
//!
//! Lo que dijo Jev de cada tarea de una lista va en
//! `pins/draw/anot-<uid de la lista>.urgencia.json`, al lado de lo demas
//! anotado de ese mensaje ([`crate::anotado::URGENCIA`]): viaja con su chat
//! como los comentarios o los marcos y, si cambio en los dos aparatos, se
//! junta al llegar ([`al_llegar`]). Se guardan **las senales**, no la
//! prioridad: la prioridad se sigue calculando gratis cada dia con su plazo.
//!
//! ```json
//! {
//!   "version": 1,
//!   "tareas": [
//!     {
//!       "texto": "Mandar la cotización (al cliente)",
//!       "consecuencia": [0.05, 0.15, 0.5, 0.3],
//!       "confianza": 0.6,
//!       "bloquea": 0.8,
//!       "plazo_ajeno": 0.9,
//!       "piden_ya": 0.2,
//!       "cuando": 1793582613265,
//!       "aparato": "pc1"
//!     }
//!   ]
//! }
//! ```
//!
//! - `texto`: lo que se le manda a Jev (el titulo sin marcas y su nota entre
//!   parentesis). Es la clave: si cambia, es otra tarea.
//! - `consecuencia`: la probabilidad de cada nivel del Score (0-3);
//!   `piden_ya` falta si se pregunto sin mensajes del chat.
//! - **Juntar**: por `texto`; la misma en los dos, gana la de `cuando` mayor
//!   y, a igualdad, la de `aparato` mayor. Conmutativo e idempotente.
//! - Se escribe con la sangria de kotlinx (dos espacios), las tareas por
//!   orden de texto, los numeros a diezmilesimas como `Double.toString` y
//!   `\n` al final. Las claves de arriba que no se conocen se conservan; las
//!   de dentro de una tarea, no (Android tampoco).
//!
//! Ver `docs/urgencia-que-viaja.md` de Android.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use crate::canonico::{self, Json, Objeto};

pub const VERSION: i64 = 1;

/// La confianza cuando el fichero no la trae (`Escala` de Android).
pub const CONFIANZA_POR_OMISION: f64 = 0.6;

/// Lo dicho de una tarea, cuando y en que aparato.
#[derive(Debug, Clone, PartialEq)]
pub struct Guardada {
    /// La probabilidad de cada nivel del Score (0 a 3).
    pub consecuencia: Vec<f64>,
    pub confianza: f64,
    pub bloquea: f64,
    pub plazo_ajeno: f64,
    /// Falta si se pregunto sin mensajes del chat.
    pub piden_ya: Option<f64>,
    /// Milisegundos UTC de la respuesta.
    pub cuando: i64,
    /// El codigo del aparato que pregunto ("" si no se sabe).
    pub aparato: String,
}

/// Lo guardado de una lista, por `texto`.
pub type Tareas = BTreeMap<String, Guardada>;

/// Lo guardado en el fichero `f`. Vacio si no esta o esta roto.
pub fn leer(f: &Path) -> Tareas {
    std::fs::read_to_string(f)
        .map(|t| leer_texto(&t))
        .unwrap_or_default()
}

/// `deTexto`: lo guardado en un texto. Vacio si esta roto.
pub fn leer_texto(texto: &str) -> Tareas {
    let mut salida = Tareas::new();
    let Ok(Json::Objeto(raiz)) = Json::analizar(texto) else {
        return salida;
    };
    let Some(lista) = raiz.obtener("tareas").and_then(Json::como_lista) else {
        return salida;
    };
    for e in lista {
        let Some(o) = e.como_objeto() else { continue };
        let Some(t) = o
            .obtener("texto")
            .and_then(Json::como_cadena)
            .filter(|t| !t.trim().is_empty())
        else {
            continue;
        };
        let Some(g) = guardada_de(o) else { continue };
        let g = match salida.remove(t) {
            Some(antes) => gana(antes, g),
            None => g,
        };
        salida.insert(t.to_string(), g);
    }
    salida
}

/// `JsonPrimitive.doubleOrNull`: el contenido de un primitivo (cadena o
/// literal) leido como numero.
fn doble(j: &Json) -> Option<f64> {
    j.contenido()?.parse().ok()
}

fn num(o: &Objeto, k: &str) -> Option<f64> {
    o.obtener(k).and_then(doble)
}

fn guardada_de(o: &Objeto) -> Option<Guardada> {
    let lista = o.obtener("consecuencia")?.como_lista()?;
    // Lo que no es un primitivo tumba la entrada; un primitivo que no es
    // numero cuenta como 0 (`doubleOrNull ?: 0.0`).
    let mut consecuencia = Vec::with_capacity(lista.len());
    for e in lista {
        if !e.es_primitivo() {
            return None;
        }
        consecuencia.push(doble(e).unwrap_or(0.0));
    }
    if consecuencia.is_empty() {
        return None;
    }
    Some(Guardada {
        consecuencia,
        confianza: num(o, "confianza").unwrap_or(CONFIANZA_POR_OMISION),
        bloquea: num(o, "bloquea").unwrap_or(0.0),
        plazo_ajeno: num(o, "plazo_ajeno").unwrap_or(0.0),
        piden_ya: num(o, "piden_ya"),
        cuando: o
            .obtener("cuando")
            .filter(|j| j.es_primitivo())
            .and_then(Json::contenido)
            .and_then(|c| c.parse().ok())
            .unwrap_or(0),
        aparato: o
            .obtener("aparato")
            .and_then(Json::como_cadena)
            .unwrap_or_default()
            .to_string(),
    })
}

/// De dos de la misma tarea, la que se queda: la de `cuando` mayor y, a
/// igualdad, la de `aparato` mayor (como `String.compareTo` de Kotlin). Las
/// dos partes eligen la misma sin hablar.
pub fn gana(a: Guardada, b: Guardada) -> Guardada {
    match a.cuando.cmp(&b.cuando) {
        Ordering::Greater => a,
        Ordering::Less => b,
        Ordering::Equal => {
            if canonico::comparar_como_kotlin(&a.aparato, &b.aparato) != Ordering::Less {
                a
            } else {
                b
            }
        }
    }
}

/// Las dos, juntas.
pub fn juntar(a: &Tareas, b: &Tareas) -> Tareas {
    let mut salida = a.clone();
    for (t, g) in b {
        let g = match salida.remove(t) {
            Some(antes) => gana(antes, g.clone()),
            None => g.clone(),
        };
        salida.insert(t.clone(), g);
    }
    salida
}

/// `Math.round(v * 10000.0) / 10000.0` de Kotlin: a diezmilesimas, la mitad
/// hacia arriba (tambien en los negativos), y sin `-0.0`.
fn redondo(v: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    let x = v * 10_000.0;
    let f = x.floor();
    let r = if x - f >= 0.5 { f + 1.0 } else { f };
    r / 10_000.0 + 0.0
}

fn numero(v: f64) -> Json {
    Json::Literal(canonico::double_de_kotlin(redondo(v)))
}

fn a_json(t: &str, g: &Guardada) -> Json {
    let mut o = Objeto::nuevo();
    o.poner("texto", Json::cadena(t));
    o.poner(
        "consecuencia",
        Json::Lista(g.consecuencia.iter().map(|v| numero(*v)).collect()),
    );
    o.poner("confianza", numero(g.confianza));
    o.poner("bloquea", numero(g.bloquea));
    o.poner("plazo_ajeno", numero(g.plazo_ajeno));
    if let Some(p) = g.piden_ya {
        o.poner("piden_ya", numero(p));
    }
    o.poner("cuando", Json::numero(g.cuando));
    if !g.aparato.is_empty() {
        o.poner("aparato", Json::cadena(&g.aparato));
    }
    Json::Objeto(o)
}

/// El texto del fichero: `version`, las tareas por orden de texto y el resto
/// de claves de `antes` tal cual y en su orden.
pub fn texto(tareas: &Tareas, antes: Option<&Objeto>) -> String {
    let mut orden: Vec<(&String, &Guardada)> = tareas.iter().collect();
    orden.sort_by(|a, b| canonico::comparar_como_kotlin(a.0, b.0));
    let mut raiz = Objeto::nuevo();
    raiz.poner("version", Json::numero(VERSION));
    raiz.poner(
        "tareas",
        Json::Lista(orden.into_iter().map(|(t, g)| a_json(t, g)).collect()),
    );
    if let Some(antes) = antes {
        for (k, v) in antes.iter() {
            if k != "version" && k != "tareas" {
                raiz.poner(k, v.clone());
            }
        }
    }
    let mut s = Json::Objeto(raiz).a_texto_con_sangria("  ");
    s.push('\n');
    s
}

/// Lo que hay en el texto de un fichero y su objeto de arriba, o `None` si
/// no es un objeto JSON (y entonces no se escribe encima).
fn de_texto_entero(crudo: &str) -> Option<(Tareas, Objeto)> {
    match Json::analizar(crudo) {
        Ok(Json::Objeto(o)) => Some((leer_texto(crudo), o)),
        _ => None,
    }
}

/// **Lo de `nuevas` juntado con el texto `crudo` de un fichero** (`None`: no
/// habia fichero). `None` si no hay que escribir: nada cambia o el fichero
/// esta roto (se perderia lo que tuviera).
pub fn anadir_a_texto(crudo: Option<&str>, nuevas: &Tareas) -> Option<String> {
    if nuevas.is_empty() {
        return None;
    }
    let (previas, antes) = match crudo {
        Some(c) => {
            let (p, a) = de_texto_entero(c)?;
            (p, Some(a))
        }
        None => (Tareas::new(), None),
    };
    let juntas = juntar(&previas, nuevas);
    if juntas == previas {
        return None;
    }
    Some(texto(&juntas, antes.as_ref()))
}

/// **Anade `nuevas` al fichero `f`**, juntadas con lo que ya hay. No escribe
/// si no cambia nada, ni encima de un JSON roto. Escribe en un temporal y lo
/// cambia de nombre. Devuelve si escribio.
pub fn anadir(f: &Path, nuevas: &Tareas) -> io::Result<bool> {
    if nuevas.is_empty() {
        return Ok(false);
    }
    let crudo = if f.is_file() {
        match std::fs::read_to_string(f) {
            Ok(t) => Some(t),
            Err(_) => return Ok(false),
        }
    } else {
        None
    };
    let Some(nuevo) = anadir_a_texto(crudo.as_deref(), nuevas) else {
        return Ok(false);
    };
    crate::disco::escribir_atomico(f, nuevo.as_bytes())?;
    Ok(true)
}

/// **Lo que llega, junto con lo de aqui** (`Disco.escribirArchivo` de
/// Android): el texto que hay que escribir en vez de `llega`, o `None` si
/// `llega` ya lo trae todo (o esta roto, o aqui no hay nada que juntar) y se
/// escribe tal cual. Se conservan las claves desconocidas de lo que llega.
pub fn al_llegar(aqui: &str, llega: &str) -> Option<String> {
    let de_aqui = leer_texto(aqui);
    if de_aqui.is_empty() {
        return None;
    }
    anadir_a_texto(Some(llega), &de_aqui)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn g(c: &[f64], piden: Option<f64>, cuando: i64, aparato: &str) -> Guardada {
        Guardada {
            consecuencia: c.to_vec(),
            confianza: CONFIANZA_POR_OMISION,
            bloquea: 0.1,
            plazo_ajeno: 0.0,
            piden_ya: piden,
            cuando,
            aparato: aparato.into(),
        }
    }

    fn una(t: &str, x: Guardada) -> Tareas {
        Tareas::from([(t.to_string(), x)])
    }

    fn carpeta(nombre: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("pindesk-urgencias-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// El ejemplo de `docs/urgencia-que-viaja.md` de Android se lee entero
    /// y, escrito de nuevo, sale como lo escribe kotlinx.
    #[test]
    fn el_ejemplo_del_manual_de_android() {
        let doc = r#"{
  "version": 1,
  "tareas": [
    {
      "texto": "Mandar la cotización (al cliente)",
      "consecuencia": [0.05, 0.15, 0.5, 0.3],
      "confianza": 0.6,
      "bloquea": 0.8,
      "plazo_ajeno": 0.9,
      "piden_ya": 0.2,
      "cuando": 1793582613265,
      "aparato": "pc1"
    }
  ]
}"#;
        let t = leer_texto(doc);
        let x = &t["Mandar la cotización (al cliente)"];
        assert_eq!(x.consecuencia, vec![0.05, 0.15, 0.5, 0.3]);
        assert_eq!((x.confianza, x.bloquea, x.plazo_ajeno), (0.6, 0.8, 0.9));
        assert_eq!(x.piden_ya, Some(0.2));
        assert_eq!((x.cuando, x.aparato.as_str()), (1793582613265, "pc1"));
        let esperado = "{\n  \"version\": 1,\n  \"tareas\": [\n    {\n      \"texto\": \"Mandar la cotización (al cliente)\",\n      \"consecuencia\": [\n        0.05,\n        0.15,\n        0.5,\n        0.3\n      ],\n      \"confianza\": 0.6,\n      \"bloquea\": 0.8,\n      \"plazo_ajeno\": 0.9,\n      \"piden_ya\": 0.2,\n      \"cuando\": 1793582613265,\n      \"aparato\": \"pc1\"\n    }\n  ]\n}\n";
        assert_eq!(texto(&t, None), esperado);
    }

    /// `ida_y_vuelta_con_lo_desconocido` de `UrgenciasGuardadasTest`.
    #[test]
    fn ida_y_vuelta_con_lo_desconocido() {
        let d = carpeta("ida");
        let f = d.join("anot-ABCDE23456.urgencia.json");
        std::fs::write(&f, r#"{"version":1,"tareas":[],"otra":{"x":1}}"#).unwrap();
        let x = g(&[0.1, 0.2, 0.3, 0.4], Some(0.9), 1000, "pc1");
        assert!(anadir(&f, &una("Mandar la cotización", x.clone())).unwrap());
        let t = std::fs::read_to_string(&f).unwrap();
        assert!(t.ends_with('\n'));
        assert!(t.contains("\"otra\": {\n    \"x\": 1\n  }"), "{t}");
        let leido = &leer(&f)["Mandar la cotización"];
        assert_eq!(leido.consecuencia, vec![0.1, 0.2, 0.3, 0.4]);
        assert_eq!(leido.piden_ya, Some(0.9));
        assert_eq!(leido.aparato, "pc1");
        // Lo mismo otra vez no reescribe.
        assert!(!anadir(&f, &una("Mandar la cotización", x)).unwrap());
        assert!(!con_tmp(&f).exists(), "no se queda el temporal");
    }

    fn con_tmp(f: &Path) -> std::path::PathBuf {
        crate::disco::con_sufijo(f, ".tmp")
    }

    /// `sin_piden_ya_y_gana_la_mas_nueva`.
    #[test]
    fn sin_piden_ya_y_gana_la_mas_nueva() {
        let a = g(&[1.0, 0.0, 0.0, 0.0], None, 10, "a");
        let b = g(&[0.0, 0.0, 0.0, 1.0], None, 20, "b");
        let mut ab = una("t", a.clone());
        ab.insert("u".into(), a.clone());
        let j = juntar(&ab, &una("t", b.clone()));
        assert_eq!(j["t"], b);
        assert_eq!(j["u"], a);
        // Conmutativo e idempotente.
        assert_eq!(j, juntar(&una("t", b.clone()), &ab));
        assert_eq!(j, juntar(&j, &j));
        let vuelta = leer_texto(&texto(&j, None));
        assert_eq!(vuelta["u"].piden_ya, None);
        assert!(!texto(&j, None).contains("piden_ya"));
    }

    /// A igualdad de `cuando`, el aparato mayor, comparado como Kotlin.
    #[test]
    fn a_igualdad_de_hora_gana_el_aparato_mayor() {
        let a = g(&[1.0], None, 5, "pc1");
        let b = g(&[0.0, 1.0], None, 5, "tel");
        assert_eq!(gana(a.clone(), b.clone()), b);
        assert_eq!(gana(b.clone(), a.clone()), b);
        // Por unidades UTF-16: un caracter de la zona privada va DESPUES de
        // un emoji en Kotlin (al reves que por puntos de codigo).
        let emoji = g(&[1.0], None, 5, "\u{1F600}");
        let privada = g(&[0.0, 1.0], None, 5, "\u{F000}");
        assert_eq!(gana(emoji.clone(), privada.clone()), privada);
    }

    /// `un_json_roto_no_se_pisa`.
    #[test]
    fn un_json_roto_no_se_pisa() {
        let d = carpeta("roto");
        let f = d.join("x.urgencia.json");
        std::fs::write(&f, "{roto").unwrap();
        assert!(!anadir(&f, &una("t", g(&[0.5, 0.5], None, 1, ""))).unwrap());
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "{roto");
        // Tampoco encima de algo que no es un objeto.
        std::fs::write(&f, "[1]").unwrap();
        assert!(!anadir(&f, &una("t", g(&[0.5, 0.5], None, 1, ""))).unwrap());
        // Y sin nada nuevo no se crea el fichero.
        let otro = d.join("y.urgencia.json");
        assert!(!anadir(&otro, &Tareas::new()).unwrap());
        assert!(!otro.exists());
    }

    /// Lo que no vale se salta; lo que falta toma su valor por omision; y
    /// las claves desconocidas de una tarea se tiran.
    #[test]
    fn lo_que_falta_y_lo_que_no_vale() {
        let t = leer_texto(
            r#"{"tareas":[
            {"texto":"a","consecuencia":[0.5,"0.25",null,true],"x":1},
            {"texto":"  ","consecuencia":[1]},
            {"texto":"b","consecuencia":[]},
            {"texto":"c"},
            {"texto":"d","consecuencia":[{"no":1}]},
            {"texto":7,"consecuencia":[1]},
            {"texto":"e","consecuencia":[1],"cuando":1.5,"aparato":3}
        ]}"#,
        );
        assert_eq!(t.keys().collect::<Vec<_>>(), ["a", "e"]);
        let a = &t["a"];
        assert_eq!(a.consecuencia, vec![0.5, 0.25, 0.0, 0.0]);
        assert_eq!((a.confianza, a.bloquea, a.plazo_ajeno), (0.6, 0.0, 0.0));
        assert_eq!((a.piden_ya, a.cuando, a.aparato.as_str()), (None, 0, ""));
        assert_eq!((t["e"].cuando, t["e"].aparato.as_str()), (0, ""));
        let escrito = texto(&t, None);
        assert!(!escrito.contains("\"x\""));
        assert!(!escrito.contains("aparato"), "sin aparato no se escribe");
        // Caso negativo: lo que no es un objeto, o no trae lista, no es nada.
        assert!(leer_texto("[]").is_empty());
        assert!(leer_texto(r#"{"tareas":{}}"#).is_empty());
        assert!(leer_texto("no").is_empty());
    }

    /// Diezmilesimas, la mitad hacia arriba, y como `Double.toString`.
    #[test]
    fn los_numeros_a_diezmilesimas_como_kotlin() {
        let x = Guardada {
            consecuencia: vec![0.00004, 0.00005, 0.123456, 1.0],
            confianza: 0.99995,
            bloquea: -0.00005,
            plazo_ajeno: 0.0001,
            piden_ya: Some(-0.0),
            cuando: 3,
            aparato: "pc1".into(),
        };
        let s = texto(&una("t", x), None);
        assert!(
            s.contains("\"consecuencia\": [\n        0.0,\n        1.0E-4,\n        0.1235,\n        1.0\n      ]"),
            "{s}"
        );
        assert!(s.contains("\"confianza\": 1.0,"), "{s}");
        assert!(s.contains("\"bloquea\": 0.0,"), "{s}");
        assert!(s.contains("\"plazo_ajeno\": 1.0E-4,"), "{s}");
        assert!(s.contains("\"piden_ya\": 0.0,"), "{s}");
    }

    /// Las tareas por orden de texto (UTF-16) y el resto de arriba detras,
    /// en su orden.
    #[test]
    fn el_orden_de_lo_escrito() {
        let mut t = una("b", g(&[1.0], None, 1, ""));
        t.insert("a".into(), g(&[1.0], None, 1, ""));
        t.insert("\u{1F600}".into(), g(&[1.0], None, 1, ""));
        t.insert("\u{F000}".into(), g(&[1.0], None, 1, ""));
        let antes = Json::analizar(r#"{"z":1,"tareas":[],"a":2,"version":9}"#).unwrap();
        let s = texto(&t, antes.como_objeto());
        let j = Json::analizar(&s).unwrap();
        let o = j.como_objeto().unwrap();
        assert_eq!(
            o.claves().collect::<Vec<_>>(),
            ["version", "tareas", "z", "a"]
        );
        assert_eq!(o.obtener("version"), Some(&Json::numero(1)));
        let textos: Vec<&str> = o
            .obtener("tareas")
            .and_then(Json::como_lista)
            .unwrap()
            .iter()
            .filter_map(|e| e.como_objeto()?.obtener("texto")?.como_cadena())
            .collect();
        assert_eq!(textos, ["a", "b", "\u{1F600}", "\u{F000}"]);
    }

    /// La parte de la urgencia de `SincronizarDeVerdadTest` (la lista que
    /// revisan los dos a la vez): lo que llega se junta con lo de aqui, y
    /// con la vuelta de lo juntado los dos quedan iguales.
    #[test]
    fn al_llegar_se_juntan_las_dos_y_quedan_iguales() {
        let s = |cuando, aparato: &str| g(&[0.1, 0.2, 0.3, 0.4], None, cuando, aparato);
        let base = texto(&una("Mandar la cotización", s(5, "tel")), None);
        let tel = anadir_a_texto(Some(&base), &una("Pedir cemento", s(7, "tel"))).unwrap();
        let tab = anadir_a_texto(Some(&base), &una("Llamar al cliente", s(8, "tab"))).unwrap();
        // Llega lo de la tableta al telefono: se junta.
        let en_tel = al_llegar(&tel, &tab).expect("se junta");
        assert_eq!(
            leer_texto(&en_tel).keys().collect::<Vec<_>>(),
            ["Llamar al cliente", "Mandar la cotización", "Pedir cemento"]
        );
        // Vuelve a la tableta lo juntado: ya lo trae todo, se escribe tal cual.
        assert_eq!(al_llegar(&tab, &en_tel), None);
        // Caso negativo: lo que llega roto no se toca, y sin nada aqui
        // tampoco se junta.
        assert_eq!(al_llegar(&tel, "{roto"), None);
        assert_eq!(al_llegar("{roto", &tab), None);
        assert_eq!(al_llegar(&texto(&Tareas::new(), None), &tab), None);
        // Lo desconocido de lo que llega se queda.
        let con_otra = tab.replacen("{\n", "{\n  \"otra\": true,\n", 1);
        assert!(
            al_llegar(&tel, &con_otra)
                .unwrap()
                .contains("\"otra\": true")
        );
    }
}
