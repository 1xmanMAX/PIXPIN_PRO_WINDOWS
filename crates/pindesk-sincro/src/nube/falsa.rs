//! **Una nube falsa en memoria** para las pruebas: las reglas de §3, §4 y
//! §10 del manual (PUT con `base`, PATCH, `cambios`, objetos), sin red, y
//! contando los bytes que suben.

use std::collections::{BTreeMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{Value, json};

use super::cliente::{Contestado, Transporte};
use super::tipos::{Archivo, Inventario, Parche};

#[derive(Default)]
struct Chat {
    inv: Inventario,
    revision: i64,
    /// Los parches desde el ultimo PUT entero.
    diario: Vec<Parche>,
    /// La revision del ultimo PUT entero (desde antes, `reiniciar`).
    corte: i64,
}

#[derive(Default)]
struct Dentro {
    chats: BTreeMap<String, Chat>,
    objetos: BTreeMap<String, Vec<u8>>,
    /// Bytes que subieron (cuerpos de PUT, PATCH y objetos).
    subidos: u64,
    pedidos: Vec<String>,
}

/// La nube falsa. Con `deltas: false` es una nube del protocolo 1.
pub struct NubeFalsa {
    pub deltas: bool,
    dentro: Mutex<Dentro>,
}

fn ok(v: Value) -> Result<Contestado, String> {
    Ok(Contestado {
        codigo: 200,
        cuerpo: serde_json::to_vec(&v).unwrap(),
    })
}

fn codigo(c: u16, v: Value) -> Result<Contestado, String> {
    Ok(Contestado {
        codigo: c,
        cuerpo: serde_json::to_vec(&v).unwrap(),
    })
}

fn id_de(m: &Value) -> String {
    m.get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

impl NubeFalsa {
    pub fn nueva(deltas: bool) -> NubeFalsa {
        NubeFalsa {
            deltas,
            dentro: Mutex::new(Dentro::default()),
        }
    }

    /// Bytes subidos hasta ahora, y los pedidos hechos (borra la cuenta).
    pub fn cuenta(&self) -> (u64, Vec<String>) {
        let mut g = self.dentro.lock().unwrap();
        let s = std::mem::take(&mut g.subidos);
        (s, std::mem::take(&mut g.pedidos))
    }

    pub fn revision(&self, chat: &str) -> i64 {
        self.dentro
            .lock()
            .unwrap()
            .chats
            .get(chat)
            .map_or(0, |c| c.revision)
    }

    /// El inventario de la nube.
    pub fn inventario(&self, chat: &str) -> Inventario {
        self.dentro.lock().unwrap().chats[chat].inv.clone()
    }

    pub fn objeto(&self, sha: &str) -> Option<Vec<u8>> {
        self.dentro.lock().unwrap().objetos.get(sha).cloned()
    }

    fn faltan(d: &Dentro, inv: &Inventario) -> Vec<String> {
        let mut v = Vec::new();
        for a in &inv.archivos {
            for s in std::iter::once(&a.sha256).chain(a.deltas().iter().map(|x| &x.sha256)) {
                if !d.objetos.contains_key(s) && !v.contains(s) {
                    v.push(s.clone());
                }
            }
        }
        v
    }
}

fn aplicar(inv: &mut Inventario, p: &Parche) {
    let quitar: HashSet<&String> = p.mensajes.quitar.iter().collect();
    inv.mensajes.retain(|m| !quitar.contains(&id_de(m)));
    for m in &p.mensajes.poner {
        match inv.mensajes.iter().position(|x| id_de(x) == id_de(m)) {
            Some(i) => inv.mensajes[i] = m.clone(),
            None => inv.mensajes.push(m.clone()),
        }
    }
    let quitar: HashSet<&String> = p.archivos.quitar.iter().collect();
    inv.archivos.retain(|a| !quitar.contains(&a.rel));
    for a in &p.archivos.poner {
        match inv.archivos.iter().position(|x| x.rel == a.rel) {
            Some(i) => inv.archivos[i] = a.clone(),
            None => inv.archivos.push(a.clone()),
        }
    }
    if let Some(n) = &p.nombre {
        inv.nombre = n.clone();
    }
    if let Some(pr) = &p.proyecto {
        inv.proyecto = (!pr.is_null()).then(|| pr.clone());
    }
    if let Some(c) = &p.compartido {
        inv.compartido = Some(c.clone());
    }
}

impl Transporte for NubeFalsa {
    fn enviar(
        &self,
        metodo: &str,
        url: &str,
        _cab: &[(&'static str, String)],
        cuerpo: &[u8],
        _t: Duration,
    ) -> Result<Contestado, String> {
        let ruta = url.split("/nube/api/v1/").nth(1).ok_or("ruta")?;
        let (ruta, q) = ruta.split_once('?').unwrap_or((ruta, ""));
        let param = |k: &str| {
            q.split('&')
                .find_map(|x| x.strip_prefix(&format!("{k}=")))
                .map(str::to_string)
        };
        let partes: Vec<&str> = ruta.split('/').collect();
        let mut d = self.dentro.lock().unwrap();
        d.pedidos
            .push(format!("{metodo} {}", partes.first().unwrap_or(&"")));
        if matches!(metodo, "PUT" | "PATCH") {
            d.subidos += cuerpo.len() as u64;
        }
        match (metodo, partes.as_slice()) {
            ("GET", ["hola"]) => ok(json!({
                "protocolo": "pixpin-nube", "version": 1,
                "aparato": {"id": "ap1", "nombre": "PC", "espacio": "max", "persona": null},
                "capacidades": if self.deltas { json!(["deltas1"]) } else { json!([]) },
            })),
            ("HEAD", ["objetos", s]) => codigo(
                if d.objetos.contains_key(*s) { 200 } else { 404 },
                json!({}),
            ),
            ("PUT", ["objetos", s]) => {
                if super::super::canonico::sha256_hex(cuerpo) != *s {
                    return codigo(400, json!({"error": "no coincide"}));
                }
                d.objetos.insert(s.to_string(), cuerpo.to_vec());
                codigo(201, json!({"ok": true}))
            }
            ("GET", ["objetos", s]) => match d.objetos.get(*s) {
                Some(b) => Ok(Contestado {
                    codigo: 200,
                    cuerpo: b.clone(),
                }),
                None => codigo(404, json!({"error": "no"})),
            },
            ("GET", ["chats", c]) => match d.chats.get(*c) {
                Some(ch) => {
                    let mut inv = ch.inv.clone();
                    inv.revision = Some(ch.revision);
                    inv.espacio = Some("max".into());
                    ok(serde_json::to_value(inv).unwrap())
                }
                None => codigo(404, json!({"error": "no está"})),
            },
            ("PUT", ["chats", c]) => {
                let inv: Inventario = serde_json::from_slice(cuerpo).map_err(|e| e.to_string())?;
                let rev = d.chats.get(*c).map_or(0, |x| x.revision);
                if d.chats.contains_key(*c) && inv.compartido.is_some() && inv.base.is_none() {
                    return codigo(409, json!({"revision": rev}));
                }
                if inv.base.is_some_and(|b| b != rev) {
                    return codigo(409, json!({"revision": rev}));
                }
                let ch = d.chats.entry(c.to_string()).or_default();
                ch.revision += 1;
                ch.inv = Inventario { base: None, ..inv };
                ch.diario.clear();
                ch.corte = ch.revision;
                let r = ch.revision;
                let inv = ch.inv.clone();
                let faltan = Self::faltan(&d, &inv);
                ok(json!({"ok": true, "espacio": "max", "revision": r, "faltan": faltan}))
            }
            ("PATCH", ["chats", c]) if self.deltas => {
                let mut p: Parche = serde_json::from_slice(cuerpo).map_err(|e| e.to_string())?;
                let rev = d.chats.get(*c).map_or(0, |x| x.revision);
                if p.base != rev {
                    return codigo(409, json!({"revision": rev}));
                }
                let ch = d.chats.entry(c.to_string()).or_default();
                aplicar(&mut ch.inv, &p);
                ch.revision += 1;
                p.revision = Some(ch.revision);
                p.espacio = Some("max".into());
                ch.diario.push(p);
                let r = ch.revision;
                let inv = ch.inv.clone();
                let faltan = Self::faltan(&d, &inv);
                ok(json!({"ok": true, "espacio": "max", "revision": r, "faltan": faltan}))
            }
            ("GET", ["chats", c, "cambios"]) if self.deltas => {
                let desde: i64 = param("desde")
                    .ok_or("sin desde")?
                    .parse()
                    .map_err(|_| "desde")?;
                let Some(ch) = d.chats.get(*c) else {
                    return codigo(404, json!({"error": "no está"}));
                };
                if desde > ch.revision || desde < ch.corte {
                    return ok(json!({"revision": ch.revision, "reiniciar": true, "cambios": []}));
                }
                let cambios: Vec<&Parche> = ch
                    .diario
                    .iter()
                    .filter(|p| p.revision.unwrap_or(0) > desde)
                    .collect();
                ok(json!({"revision": ch.revision, "reiniciar": false, "cambios": cambios}))
            }
            _ => codigo(404, json!({"error": format!("{metodo} {ruta}")})),
        }
    }
}

/// Una linea de archivo, para comparar en las pruebas.
pub fn linea<'a>(inv: &'a Inventario, rel: &str) -> &'a Archivo {
    inv.archivos
        .iter()
        .find(|a| a.rel == rel)
        .expect("la linea")
}
