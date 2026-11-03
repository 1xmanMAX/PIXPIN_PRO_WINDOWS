//! **La pegatina de un lienzo, copiada en el dibujo** como elemento
//! Excalidraw.
//!
//! Lo de verdad de una pegatina esta en su `.comentarios.json` (el hilo no
//! cabe en un elemento). Pero un movil que aun no conoce las pegatinas solo
//! ve el dibujo, y no veria nada. Por eso cada pegatina de un lienzo tiene
//! en el dibujo un **espejo**: un texto con el emoticono de su tipo, del
//! tamano de la pegatina, bloqueado, con
//!
//! ```json
//! "customData": {"pindesk":"emoji","pixpin":"emoji",
//!                "comentario":{"id":"pc1-18f2a-0","tipo":"urgente","resuelto":false}}
//! ```
//!
//! - Su `id` es `comentario-<id de la pegatina>`, el mismo en todos los
//!   aparatos: la sincronizacion junta figura a figura, y el espejo se junta
//!   como una figura mas.
//! - Esta en el **centro** de la pegatina menos la mitad de su lado
//!   ([`LADO`]), en coordenadas del mundo.
//! - Quitar la pegatina marca el espejo `isDeleted` (no se quita: volveria).
//! - Quien conoce las pegatinas **no pinta el espejo** (lo deja aparte, como
//!   un elemento ajeno, y lo reescribe al guardar) y, al abrir, si el espejo
//!   es mas nuevo que la pegatina (`updated` > `cambiado`: lo movio o lo
//!   borro un movil), lo aplica ([`aplicar`]).

use serde_json::{Value, json};

use crate::ancla::{Ancla, Sitio};
use crate::modelo::{Comentario, Pegatinas, Quien, Tipo};

/// Lo que va delante del id del espejo.
pub const PREFIJO: &str = "comentario-";

/// El lado del espejo, en unidades del mundo (el de la pegatina a zoom 1).
pub const LADO: f64 = 36.0;

/// Si un elemento Excalidraw es el espejo de una pegatina.
pub fn es_espejo(v: &Value) -> bool {
    v.get("customData")
        .and_then(|c| c.get("comentario"))
        .is_some_and(Value::is_object)
}

/// Lo que dice un espejo.
#[derive(Debug, Clone, PartialEq)]
pub struct Espejo {
    pub id: String,
    pub tipo: Tipo,
    /// El centro, en coordenadas del mundo.
    pub x: f64,
    pub y: f64,
    pub borrado: bool,
    pub resuelto: bool,
    /// `updated` del elemento (milisegundos).
    pub cuando: i64,
}

/// Los espejos de una lista de elementos.
pub fn leer(elementos: &[Value]) -> Vec<Espejo> {
    elementos
        .iter()
        .filter(|v| es_espejo(v))
        .filter_map(|v| {
            let c = &v["customData"]["comentario"];
            let id = c.get("id").and_then(Value::as_str)?.to_string();
            let n = |k: &str| v.get(k).and_then(Value::as_f64);
            let (x, y) = (n("x")?, n("y")?);
            let (w, h) = (n("width").unwrap_or(LADO), n("height").unwrap_or(LADO));
            Some(Espejo {
                id,
                tipo: Tipo::de_nombre(c.get("tipo").and_then(Value::as_str).unwrap_or("")),
                x: x + w / 2.0,
                y: y + h / 2.0,
                borrado: v.get("isDeleted").and_then(Value::as_bool).unwrap_or(false),
                resuelto: c.get("resuelto").and_then(Value::as_bool).unwrap_or(false),
                cuando: v.get("updated").and_then(Value::as_i64).unwrap_or(0),
            })
        })
        .collect()
}

fn hex(c: u32) -> String {
    format!("#{c:06x}")
}

/// **El espejo de una pegatina** de lienzo; `None` si no esta en la escena.
pub fn elemento(c: &Comentario) -> Option<Value> {
    let Sitio::Escena { x, y } = c.ancla.sitio() else {
        return None;
    };
    let emoji = c.tipo.emoji();
    let version = (c.cambiado / 1000).clamp(1, i32::MAX as i64);
    Some(json!({
        "id": format!("{PREFIJO}{}", c.id),
        "type": "text",
        "x": x - LADO / 2.0,
        "y": y - LADO / 2.0,
        "width": LADO,
        "height": LADO,
        "angle": 0,
        "strokeColor": hex(c.tipo.color()),
        "backgroundColor": "transparent",
        "fillStyle": "solid",
        "strokeWidth": 1,
        "strokeStyle": "solid",
        "roughness": 0,
        "opacity": if c.resuelto { 50 } else { 100 },
        "groupIds": [],
        "frameId": null,
        "roundness": null,
        "seed": 1,
        "version": version,
        "versionNonce": 0,
        "isDeleted": c.borrado,
        "boundElements": null,
        "updated": c.cambiado,
        "link": null,
        "locked": true,
        "text": emoji,
        "originalText": emoji,
        "fontSize": LADO * 0.8,
        "fontFamily": 1,
        "textAlign": "center",
        "verticalAlign": "middle",
        "containerId": null,
        "lineHeight": 1.25,
        "customData": {
            "pindesk": "emoji",
            "pixpin": "emoji",
            "comentario": {
                "id": c.id,
                "tipo": c.tipo.nombre(),
                "resuelto": c.resuelto,
            }
        }
    }))
}

/// **Pone al dia los espejos** de una lista de elementos con las pegatinas
/// de lienzo: el de cada una en su sitio (si ya estaba, en el mismo hueco
/// del orden), los nuevos al final. Los espejos de pegatinas que no estan en
/// `p` se quedan como estaban (son de otro aparato y aun no han llegado).
/// Devuelve si cambio algo.
pub fn poner(elementos: &mut Vec<Value>, p: &Pegatinas) -> bool {
    let mut cambio = false;
    for c in &p.lista {
        let Some(nuevo) = elemento(c) else {
            continue;
        };
        let id = nuevo["id"].clone();
        match elementos.iter_mut().find(|v| v.get("id") == Some(&id)) {
            Some(v) => {
                if *v != nuevo {
                    *v = nuevo;
                    cambio = true;
                }
            }
            None => {
                // Una pegatina quitada que nunca tuvo espejo no lo necesita.
                if !c.borrado {
                    elementos.push(nuevo);
                    cambio = true;
                }
            }
        }
    }
    cambio
}

/// Quita los espejos de la lista y los devuelve (para pintar la lista sin
/// ellos y volver a ponerlos al guardar).
pub fn apartar(elementos: &mut Vec<Value>) -> Vec<Value> {
    let (espejos, resto): (Vec<Value>, Vec<Value>) =
        std::mem::take(elementos).into_iter().partition(es_espejo);
    *elementos = resto;
    espejos
}

/// **Lo que hizo otro con los espejos**: un espejo mas nuevo que su pegatina
/// la mueve, la quita o le cambia el tipo; un espejo sin pegatina (alguien
/// copio el elemento, o el fichero de comentarios aun no ha llegado) se
/// vuelve pegatina sin hilo. Devuelve si cambio algo.
pub fn aplicar(p: &mut Pegatinas, espejos: &[Espejo], quien: &Quien) -> bool {
    let mut cambio = false;
    for e in espejos {
        match p.lista.iter_mut().find(|c| c.id == e.id) {
            Some(c) => {
                if e.cuando <= c.cambiado {
                    continue;
                }
                let antes = c.clone();
                if e.borrado && !c.borrado {
                    c.borrado = true;
                }
                if let Sitio::Escena { x, y } = c.ancla.sitio()
                    && ((x - e.x).abs() > 0.5 || (y - e.y).abs() > 0.5)
                {
                    c.ancla = c.ancla.movida(e.x, e.y, None);
                }
                c.tipo = e.tipo;
                c.resuelto = e.resuelto;
                if *c != antes {
                    c.cambiado = e.cuando;
                    cambio = true;
                }
            }
            None if !e.borrado => {
                p.lista.push(Comentario {
                    id: e.id.clone(),
                    tipo: e.tipo,
                    ancla: Ancla::escena(e.x, e.y),
                    autor: quien.autor.clone(),
                    creado: e.cuando,
                    cambiado: e.cuando,
                    resuelto: e.resuelto,
                    ..Comentario::default()
                });
                cambio = true;
            }
            None => {}
        }
    }
    cambio
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn q() -> Quien {
        Quien {
            autor: "PC".into(),
            aparato: "pc".into(),
        }
    }

    #[test]
    fn el_espejo_va_centrado_y_vuelve_igual() {
        let mut p = Pegatinas::default();
        let id = p.poner(Tipo::Urgente, Ancla::escena(100.0, 200.0), &q(), 5000);
        let mut elementos = vec![json!({"id":"a","type":"rectangle"})];
        assert!(poner(&mut elementos, &p));
        assert_eq!(elementos.len(), 2);
        let e = &elementos[1];
        assert_eq!(e["id"], format!("comentario-{id}"));
        assert_eq!(
            (e["x"].as_f64(), e["y"].as_f64()),
            (Some(82.0), Some(182.0))
        );
        assert_eq!(e["text"], "\u{1F534}");
        assert_eq!(e["locked"], true);
        assert_eq!(e["customData"]["pindesk"], "emoji");
        let l = leer(&elementos);
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].x, l[0].y, l[0].tipo), (100.0, 200.0, Tipo::Urgente));
        // Sin cambios, poner no toca nada.
        assert!(!poner(&mut elementos, &p));
        // Solo las del lienzo tienen espejo.
        let mut q2 = Pegatinas::default();
        q2.poner(Tipo::Nota, Ancla::pagina(0, 1.0, 1.0), &q(), 1);
        let mut v = Vec::new();
        assert!(!poner(&mut v, &q2));
    }

    #[test]
    fn lo_que_hizo_el_movil_con_el_espejo_se_aplica_si_es_mas_nuevo() {
        let mut p = Pegatinas::default();
        let id = p.poner(Tipo::Media, Ancla::escena(0.0, 0.0), &q(), 1000);
        let mut elementos = Vec::new();
        poner(&mut elementos, &p);
        // El movil lo arrastra a (50, 60) y lo guarda despues.
        elementos[0]["x"] = json!(32.0);
        elementos[0]["y"] = json!(42.0);
        elementos[0]["updated"] = json!(2000);
        assert!(aplicar(&mut p, &leer(&elementos), &q()));
        assert_eq!(p.get(&id).unwrap().ancla, Ancla::escena(50.0, 60.0));
        assert_eq!(p.get(&id).unwrap().cambiado, 2000);
        // Caso negativo: un espejo viejo no deshace lo de aqui.
        p.mover(&id, Ancla::escena(7.0, 7.0), 3000);
        assert!(!aplicar(&mut p, &leer(&elementos), &q()));
        assert_eq!(p.get(&id).unwrap().ancla, Ancla::escena(7.0, 7.0));
        // Lo borra el movil despues: queda quitada.
        elementos[0]["isDeleted"] = json!(true);
        elementos[0]["updated"] = json!(4000);
        assert!(aplicar(&mut p, &leer(&elementos), &q()));
        assert!(p.get(&id).is_none());
        // Y al guardar el espejo sale borrado.
        poner(&mut elementos, &p);
        assert_eq!(elementos[0]["isDeleted"], true);
    }

    #[test]
    fn un_espejo_sin_pegatina_se_vuelve_pegatina_y_apartar_los_separa() {
        let mut elementos = vec![
            json!({"id":"r","type":"rectangle"}),
            json!({"id":"comentario-z","type":"text","x":0,"y":0,"width":36,"height":36,"updated":9,
                   "customData":{"comentario":{"id":"z","tipo":"alta"}}}),
            json!({"id":"t","type":"text","customData":{"pindesk":"emoji"}}),
        ];
        let mut p = Pegatinas::default();
        assert!(aplicar(&mut p, &leer(&elementos), &q()));
        assert_eq!(p.get("z").unwrap().tipo, Tipo::Alta);
        assert_eq!(p.get("z").unwrap().ancla, Ancla::escena(18.0, 18.0));
        let aparte = apartar(&mut elementos);
        assert_eq!(aparte.len(), 1);
        assert_eq!(elementos.len(), 2);
        // Caso negativo: un emoji normal no es espejo.
        assert!(!es_espejo(&elementos[1]));
    }
}
