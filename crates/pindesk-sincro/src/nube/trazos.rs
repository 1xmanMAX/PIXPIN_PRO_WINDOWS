//! **Lo que viaja de un lienzo o una tinta: solo los trazos nuevos**
//! (capacidad `deltas1`, 1-nov-2026).
//!
//! Una raya sobre un plano no vuelve a subir el `.excalidraw.gz` entero:
//! sube un **delta**, un objeto JSON pequeno
//!
//! ```json
//! {"pixpinDelta":1, "tipo":"excalidraw", "rel":"pins/draw/anot-…-p0.excalidraw.gz",
//!  "base":"<sha del original>", "elementos":[…nuevos o cambiados, enteros…],
//!  "borrados":["id",…], "archivos":{"<fileId>":{…solo las fotos nuevas…}},
//!  "appState":{…}}
//! ```
//!
//! que se apunta en `deltas` de la linea del fichero. Lo que vale es el
//! original con los deltas puestos en orden. **La regla para juntar** es la
//! de Excalidraw (`reconcile.ts`) y la de `fusion` sin reloj: por cada id,
//! gana la `version` mayor y, a igualdad, el `versionNonce` menor. Lo
//! borrado es una lapida (`isDeleted: true`, con su version subida): asi dos
//! personas que pintan a la vez sobre la misma hoja suman sus trazos.
//!
//! Cuando los deltas pesan (mas de [MAX_DELTAS], o mas de la mitad del
//! original) se **compacta**: se sube la escena entera y `deltas: []`.
//!
//! Todo puro, sobre `serde_json::Value`: lo prueban las pruebas de aqui.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

/// Cuantos deltas puede llevar un fichero antes de compactarlo.
pub const MAX_DELTAS: usize = 20;

/// Si `rel` es un lienzo o una tinta (`*.excalidraw`, `*.excalidraw.gz`).
pub fn es_trazos(rel: &str) -> bool {
    rel.ends_with(".excalidraw.gz") || rel.ends_with(".excalidraw")
}

/// La `version` y el `versionNonce` de un elemento.
pub type Version = (i64, i64);

fn entero(v: Option<&Value>) -> i64 {
    match v {
        Some(Value::Number(n)) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f as i64))
            .unwrap_or(0),
        _ => 0,
    }
}

/// La version de un elemento.
pub fn version_de(e: &Value) -> Version {
    (entero(e.get("version")), entero(e.get("versionNonce")))
}

/// **La regla**: si la version `a` gana a la `b` (mayor `version`; a
/// igualdad, menor `versionNonce`). Iguales del todo: no gana ninguna.
pub fn gana(a: Version, b: Version) -> bool {
    a.0 > b.0 || (a.0 == b.0 && a.1 < b.1)
}

fn id_de(e: &Value) -> Option<&str> {
    e.get("id").and_then(Value::as_str)
}

fn elementos(escena: &Value) -> &[Value] {
    escena
        .get("elements")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// La version de cada elemento de una escena, por su id.
pub fn versiones(escena: &Value) -> BTreeMap<String, Version> {
    elementos(escena)
        .iter()
        .filter_map(|e| Some((id_de(e)?.to_string(), version_de(e))))
        .collect()
}

/// Los `fileId` de las fotos que lleva una escena.
pub fn imagenes(escena: &Value) -> BTreeSet<String> {
    escena
        .get("files")
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

/// El resumen del `appState` (para saber si cambio sin guardarlo entero).
pub fn resumen_de_estado(escena: &Value) -> Option<String> {
    let a = escena.get("appState")?;
    Some(crate::canonico::sha256_hex(
        crate::canonico::de(&a.to_string()).as_bytes(),
    ))
}

/// Lo que la nube ya tiene de un fichero de trazos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conocido {
    pub versiones: BTreeMap<String, Version>,
    pub imagenes: BTreeSet<String>,
    pub estado: Option<String>,
}

impl Conocido {
    /// Lo conocido de una escena entera.
    pub fn de(escena: &Value) -> Conocido {
        Conocido {
            versiones: versiones(escena),
            imagenes: imagenes(escena),
            estado: resumen_de_estado(escena),
        }
    }

    /// Lo conocido despues de poner encima un delta (las versiones que
    /// ganan, las fotos nuevas, el estado nuevo).
    pub fn con(&mut self, delta: &Value) {
        for e in delta
            .get("elementos")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(id) = id_de(e) else { continue };
            let v = version_de(e);
            match self.versiones.get(id) {
                Some(&k) if !gana(v, k) => {}
                _ => {
                    self.versiones.insert(id.to_string(), v);
                }
            }
        }
        for id in delta
            .get("borrados")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let v = self.versiones.get(id).copied().unwrap_or((0, 0));
            self.versiones.insert(id.to_string(), (v.0 + 1, v.1));
        }
        if let Some(m) = delta.get("archivos").and_then(Value::as_object) {
            self.imagenes.extend(m.keys().cloned());
        }
        if let Some(a) = delta.get("appState") {
            self.estado = Some(crate::canonico::sha256_hex(
                crate::canonico::de(&a.to_string()).as_bytes(),
            ));
        }
    }
}

/// **El delta de `escena` sobre lo que la nube ya tiene** (`conocido`), o
/// `None` si no hay nada que subir. Van los elementos nuevos y los que
/// ganan a lo que la nube tiene (uno con version vieja, que la nube ya
/// supero, no viaja), las fotos nuevas y el `appState` si cambio.
///
/// Lo que falta en la escena **no** se manda como borrado: el PC escribe
/// lo borrado como lapida, y un elemento que no esta suele ser uno que
/// aqui aun no llego. `borrados` solo lo escribe [borrar] a mano.
pub fn calcular(rel: &str, base: &str, conocido: &Conocido, escena: &Value) -> Option<Value> {
    let nuevos: Vec<Value> = elementos(escena)
        .iter()
        .filter(|e| {
            let Some(id) = id_de(e) else { return false };
            match conocido.versiones.get(id) {
                None => true,
                Some(&k) => gana(version_de(e), k),
            }
        })
        .cloned()
        .collect();
    let mut archivos = Map::new();
    if let Some(m) = escena.get("files").and_then(Value::as_object) {
        for (k, v) in m {
            if !conocido.imagenes.contains(k) {
                archivos.insert(k.clone(), v.clone());
            }
        }
    }
    let estado = resumen_de_estado(escena)
        .filter(|r| conocido.estado.as_ref() != Some(r))
        .and_then(|_| escena.get("appState").cloned());
    if nuevos.is_empty() && archivos.is_empty() && estado.is_none() {
        return None;
    }
    let mut d = json!({
        "pixpinDelta": 1,
        "tipo": "excalidraw",
        "rel": rel,
        "base": base,
        "elementos": nuevos,
        "borrados": [],
    });
    if !archivos.is_empty() {
        d["archivos"] = Value::Object(archivos);
    }
    if let Some(a) = estado {
        d["appState"] = a;
    }
    Some(d)
}

/// Anade al delta `d` el borrado de `ids` (lapidas sin el elemento entero).
pub fn borrar(d: &mut Value, ids: &[&str]) {
    if let Some(l) = d.get_mut("borrados").and_then(Value::as_array_mut) {
        l.extend(ids.iter().map(|i| Value::String((*i).to_string())));
    }
}

/// Si `v` tiene la forma de un delta de trazos.
pub fn es_delta(v: &Value) -> bool {
    v.get("pixpinDelta").and_then(Value::as_i64) == Some(1)
        && v.get("tipo").and_then(Value::as_str) == Some("excalidraw")
}

/// Pone un elemento en la lista con la regla: sustituye al del mismo id si
/// gana, o va al final si no estaba.
fn poner_elemento(lista: &mut Vec<Value>, e: &Value) -> bool {
    let Some(id) = id_de(e) else { return false };
    match lista.iter().position(|x| id_de(x) == Some(id)) {
        Some(i) => {
            if gana(version_de(e), version_de(&lista[i])) {
                lista[i] = e.clone();
                true
            } else {
                false
            }
        }
        None => {
            lista.push(e.clone());
            true
        }
    }
}

fn como_objeto(escena: &mut Value) -> &mut Map<String, Value> {
    if !escena.is_object() {
        *escena = json!({"type": "excalidraw", "version": 2, "elements": [], "files": {}});
    }
    escena.as_object_mut().expect("es un objeto")
}

/// **Pone un delta encima de una escena**, con la regla. Dice si cambio algo.
pub fn aplicar(escena: &mut Value, delta: &Value) -> bool {
    let o = como_objeto(escena);
    let mut lista: Vec<Value> = o
        .get("elements")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut cambio = false;
    for e in delta
        .get("elementos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        cambio |= poner_elemento(&mut lista, e);
    }
    for id in delta
        .get("borrados")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if let Some(e) = lista.iter_mut().find(|x| id_de(x) == Some(id))
            && e.get("isDeleted").and_then(Value::as_bool) != Some(true)
        {
            let v = version_de(e).0 + 1;
            e["isDeleted"] = Value::Bool(true);
            e["version"] = json!(v);
            cambio = true;
        }
    }
    o.insert("elements".into(), Value::Array(lista));
    if let Some(m) = delta.get("archivos").and_then(Value::as_object) {
        let files = o
            .entry("files")
            .or_insert_with(|| Value::Object(Map::new()));
        if !files.is_object() {
            *files = Value::Object(Map::new());
        }
        let f = files.as_object_mut().expect("es un objeto");
        for (k, v) in m {
            if !f.contains_key(k) {
                f.insert(k.clone(), v.clone());
                cambio = true;
            }
        }
    }
    if let Some(a) = delta.get("appState").and_then(Value::as_object) {
        let st = o
            .entry("appState")
            .or_insert_with(|| Value::Object(Map::new()));
        if !st.is_object() {
            *st = Value::Object(Map::new());
        }
        let s = st.as_object_mut().expect("es un objeto");
        for (k, v) in a {
            if s.get(k) != Some(v) {
                s.insert(k.clone(), v.clone());
                cambio = true;
            }
        }
    }
    cambio
}

/// **Lo que vale un fichero**: el original con sus deltas en orden.
pub fn efectivo(base: &Value, deltas: &[Value]) -> Value {
    let mut e = base.clone();
    for d in deltas {
        aplicar(&mut e, d);
    }
    e
}

/// **Junta dos escenas enteras** elemento por elemento con la regla: lo de
/// `suya` entra en `mia` como si fuera un delta (los dos trazos quedan; el
/// mismo trazo cambiado en los dos, el de version mayor). Para cuando no
/// hay deltas que poner (la nube compacto, o se baja por primera vez).
pub fn juntar(mia: &Value, suya: &Value) -> Value {
    let mut salida = mia.clone();
    let mut como_delta = json!({
        "elementos": elementos(suya),
    });
    if let Some(f) = suya.get("files") {
        como_delta["archivos"] = f.clone();
    }
    aplicar(&mut salida, &como_delta);
    if !salida.get("appState").is_some_and(Value::is_object)
        && let Some(a) = suya.get("appState")
    {
        salida["appState"] = a.clone();
    }
    salida
}

/// **Si toca compactar**: con `cuantos` deltas que pesan `bytes_deltas`
/// sobre un original de `bytes_base`.
pub fn compactar(cuantos: usize, bytes_deltas: u64, bytes_base: u64) -> bool {
    cuantos > MAX_DELTAS || bytes_deltas.saturating_mul(2) > bytes_base
}

/// La escena de un texto (lo que no se entiende es una escena vacia).
pub fn escena_de(texto: &str) -> Value {
    serde_json::from_str(texto).unwrap_or_else(|_| json!({"elements": [], "files": {}}))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn trazo(id: &str, v: i64, n: i64) -> Value {
        json!({"id": id, "type": "freedraw", "version": v, "versionNonce": n,
               "points": [[0, 0], [10, 10]], "isDeleted": false})
    }

    fn escena(es: Vec<Value>) -> Value {
        json!({"type": "excalidraw", "version": 2, "elements": es, "files": {},
               "appState": {"viewBackgroundColor": "#fff"}})
    }

    #[test]
    fn la_regla_version_mayor_y_a_igualdad_nonce_menor() {
        assert!(gana((3, 9), (2, 1)));
        assert!(!gana((2, 1), (3, 9)));
        assert!(gana((2, 1), (2, 5)));
        assert!(!gana((2, 5), (2, 1)));
        assert!(!gana((2, 5), (2, 5)));
    }

    #[test]
    fn un_trazo_nuevo_sube_solo_el() {
        let antes = escena(vec![trazo("a", 1, 10), trazo("b", 1, 20)]);
        let conocido = Conocido::de(&antes);
        let despues = escena(vec![trazo("a", 1, 10), trazo("b", 1, 20), trazo("c", 1, 5)]);
        let d = calcular("pins/draw/x.excalidraw.gz", "s", &conocido, &despues).unwrap();
        assert_eq!(d["elementos"].as_array().unwrap().len(), 1);
        assert_eq!(d["elementos"][0]["id"], "c");
        assert!(d.get("appState").is_none());
        assert!(d.get("archivos").is_none());
        assert!(es_delta(&d));
        // Sin cambios, nada que subir.
        assert!(calcular("r", "s", &conocido, &antes).is_none());
    }

    #[test]
    fn lo_cambiado_y_lo_borrado_viajan_enteros_y_lo_viejo_no() {
        let conocido = Conocido::de(&escena(vec![trazo("a", 2, 10), trazo("b", 5, 20)]));
        let mut borrado = trazo("a", 3, 4);
        borrado["isDeleted"] = json!(true);
        // `b` en version 4: la nube ya va por la 5, no se manda.
        let ahora = escena(vec![borrado, trazo("b", 4, 1)]);
        let d = calcular("r", "s", &conocido, &ahora).unwrap();
        let ids: Vec<&str> = d["elementos"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["a"]);
        assert_eq!(d["elementos"][0]["isDeleted"], true);
    }

    #[test]
    fn dos_personas_pintan_a_la_vez_y_quedan_los_dos_trazos() {
        let base = escena(vec![trazo("a", 1, 10)]);
        let k = Conocido::de(&base);
        let mio = escena(vec![trazo("a", 1, 10), trazo("max", 1, 7)]);
        let suyo = escena(vec![trazo("a", 1, 10), trazo("ana", 1, 3)]);
        let dm = calcular("r", "s", &k, &mio).unwrap();
        let ds = calcular("r", "s", &k, &suyo).unwrap();
        // En cualquier orden, lo mismo.
        let x = efectivo(&base, &[dm.clone(), ds.clone()]);
        let y = efectivo(&base, &[ds, dm]);
        assert_eq!(versiones(&x), versiones(&y));
        assert_eq!(versiones(&x).len(), 3);
    }

    #[test]
    fn el_mismo_trazo_cambiado_en_los_dos_gana_la_version_mayor_y_si_empatan_el_nonce_menor() {
        let mut s = escena(vec![trazo("a", 1, 10)]);
        aplicar(&mut s, &json!({"elementos": [trazo("a", 3, 50)]}));
        aplicar(&mut s, &json!({"elementos": [trazo("a", 2, 1)]}));
        assert_eq!(version_de(&s["elements"][0]), (3, 50));
        aplicar(&mut s, &json!({"elementos": [trazo("a", 3, 49)]}));
        assert_eq!(version_de(&s["elements"][0]), (3, 49));
        aplicar(&mut s, &json!({"elementos": [trazo("a", 3, 60)]}));
        assert_eq!(version_de(&s["elements"][0]), (3, 49));
    }

    #[test]
    fn un_borrado_es_una_lapida_que_gana_al_trazo_de_antes() {
        let mut s = escena(vec![trazo("a", 4, 10)]);
        let mut d = json!({"elementos": [], "borrados": []});
        borrar(&mut d, &["a", "no-esta"]);
        assert!(aplicar(&mut s, &d));
        assert_eq!(s["elements"][0]["isDeleted"], true);
        assert_eq!(s["elements"][0]["version"], 5);
        // Y la version de antes ya no lo resucita.
        aplicar(&mut s, &json!({"elementos": [trazo("a", 4, 10)]}));
        assert_eq!(s["elements"][0]["isDeleted"], true);
        // Lo conocido sigue la misma cuenta.
        let mut k = Conocido::de(&escena(vec![trazo("a", 4, 10)]));
        k.con(&d);
        assert_eq!(k.versiones["a"], (5, 10));
    }

    #[test]
    fn solo_viajan_las_fotos_nuevas_y_el_estado_si_cambio() {
        let mut antes = escena(vec![]);
        antes["files"] = json!({"f1": {"id": "f1", "dataURL": "data:image/png;base64,AAAA"}});
        let k = Conocido::de(&antes);
        let mut ahora = antes.clone();
        ahora["files"]["f2"] = json!({"id": "f2", "dataURL": "data:image/png;base64,BBBB"});
        ahora["appState"]["viewBackgroundColor"] = json!("#000");
        let d = calcular("r", "s", &k, &ahora).unwrap();
        assert_eq!(
            d["archivos"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            ["f2"]
        );
        assert_eq!(d["appState"]["viewBackgroundColor"], "#000");
        let mut k2 = k.clone();
        k2.con(&d);
        assert!(calcular("r", "s", &k2, &ahora).is_none());
    }

    #[test]
    fn juntar_dos_escenas_enteras_suma_los_trazos() {
        let a = escena(vec![trazo("x", 1, 1), trazo("y", 2, 1)]);
        let b = escena(vec![trazo("y", 3, 1), trazo("z", 1, 1)]);
        let j = juntar(&a, &b);
        let v = versiones(&j);
        assert_eq!(v.len(), 3);
        assert_eq!(v["y"], (3, 1));
    }

    #[test]
    fn se_compacta_con_muchos_deltas_o_si_pesan_mas_de_la_mitad() {
        assert!(!compactar(20, 100, 1000));
        assert!(compactar(21, 100, 1000));
        assert!(!compactar(3, 500, 1000));
        assert!(compactar(3, 501, 1000));
    }
}
