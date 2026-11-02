//! **Una vuelta de un chat con la nube**: bajar lo que subieron los demas,
//! juntarlo con lo de aqui y subir solo lo que la nube no tiene.
//!
//! - Con `deltas1` (si la nube lo dice en `hola`): `GET cambios` desde la
//!   ultima revision, y `PATCH` con los mensajes cambiados, el proyecto si
//!   cambio y los ficheros que cambiaron; de un lienzo o una tinta, **solo
//!   los trazos nuevos** ([`super::trazos`]). Un 409 (otro subio entre
//!   medias) se resuelve bajando sus cambios, juntando y volviendo a
//!   calcular el parche.
//! - Sin `deltas1` (una nube del protocolo 1): el inventario entero con
//!   `PUT` y `base`, los objetos que falten, y en un 409 `GET` del inventario,
//!   la misma fusion y otra vez.
//!
//! Lo que llega se junta como por la red local: los mensajes con
//! `Disco::aplicar_mensajes` (y `fusion::json` si aqui tambien cambiaron),
//! el proyecto con su fusion, los trazos elemento a elemento con la regla de
//! [`super::trazos::gana`]. Lo que llega nunca escribe fuera del chat
//! (`compartidos::salta_de_carpeta` y la valla de los demas chats), como lo
//! de una persona por la red local.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::PathBuf;

use serde_json::Value;

use super::cliente::{FalloNube, Nube};
use super::estado::{EnLaNube, EstadoDeChat};
use super::sin_conexion::Retencion;
use super::tipos::{Archivo, Delta, Inventario, Parche};
use super::trazos::{self, Conocido};
use crate::canonico::{self, Json};
use crate::compartidos::{self, ChatCompartido, CompartidosEnDisco};
use crate::disco::{self, Disco};
use crate::fusion::{self, Criterio, Cuenta};
use crate::kotlin;

/// Hasta este tamano un objeto se sube sin preguntar antes si ya esta
/// (`HEAD`): preguntar cuesta lo mismo que mandarlo.
pub const SIN_PREGUNTAR: u64 = 64 * 1024;
/// Cuantas veces se baja y se vuelve a intentar tras un 409.
pub const INTENTOS: u32 = 4;
/// Lo mas que pesa un parche (`PATCH`, §10.1 del manual).
pub const TOPE_DE_PARCHE: usize = 4 * 1024 * 1024;

/// Como va esta vuelta.
pub struct Opciones<'a> {
    /// La nube sabe `deltas1`.
    pub deltas: bool,
    pub ahora: i64,
    /// Lo que esta «sin conexión» en un visor: no se sube.
    pub retencion: &'a dyn Retencion,
    /// Bajar antes de subir (llego una novedad de este chat).
    pub bajar: bool,
}

/// Lo que hizo una vuelta.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Informe {
    /// Se subio algo (un parche o un inventario).
    pub subido: bool,
    /// Cuantos parches de otros se aplicaron (o un inventario entero).
    pub bajados: usize,
    /// Objetos subidos y sus bytes.
    pub objetos: usize,
    pub bytes_de_objetos: u64,
    /// Bytes del cuerpo JSON de las subidas (parche o inventario).
    pub bytes_de_parches: u64,
    pub conflictos: u32,
}

/// De donde salen los bytes de un objeto por subir.
enum Fuente {
    Memoria(Vec<u8>),
    Fichero(PathBuf),
}

struct Objeto {
    sha: String,
    bytes: u64,
    fuente: Fuente,
}

impl Objeto {
    fn leer(&self) -> std::io::Result<Vec<u8>> {
        match &self.fuente {
            Fuente::Memoria(b) => Ok(b.clone()),
            Fuente::Fichero(f) => std::fs::read(f),
        }
    }
}

/// Lo preparado para subir: el parche, sus objetos y como quedara lo sabido.
struct Preparado {
    parche: Parche,
    objetos: Vec<Objeto>,
    nuevo: EstadoDeChat,
    mensajes: Vec<Json>,
    proyecto: Option<Json>,
    compartido: Option<ChatCompartido>,
}

fn sha(b: &[u8]) -> String {
    canonico::sha256_hex(b)
}

/// `gzip` sin fecha (sale igual cada vez: el mismo resumen).
pub fn comprimir(texto: &str) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = e.write_all(texto.as_bytes());
    e.finish().unwrap_or_default()
}

/// El texto de unos bytes, comprimidos o no (la nube mira los dos primeros).
pub fn descomprimir(b: &[u8]) -> Result<String, FalloNube> {
    if b.starts_with(&[0x1f, 0x8b]) {
        let mut s = String::new();
        flate2::read::GzDecoder::new(b)
            .read_to_string(&mut s)
            .map_err(|e| FalloNube::Formato(e.to_string()))?;
        Ok(s)
    } else {
        String::from_utf8(b.to_vec()).map_err(|e| FalloNube::Formato(e.to_string()))
    }
}

/// Los bytes con que sube un fichero de texto (`Disco::es_texto`).
fn bytes_de_texto(rel: &str, texto: &str) -> Vec<u8> {
    if rel.ends_with(".gz") {
        comprimir(texto)
    } else {
        texto.as_bytes().to_vec()
    }
}

/// El resumen de la lista firmada de un compartido.
fn resumen_de_compartido(c: &ChatCompartido) -> String {
    sha(serde_json::to_string(c).unwrap_or_default().as_bytes())
}

/// El `id` de un mensaje: con el pone y quita la nube (`mensajes.quitar`).
fn id_de(m: &Json) -> String {
    kotlin::cadena(m, "id").unwrap_or_default().to_string()
}

fn resumen_de_proyecto(p: &Json) -> String {
    canonico::resumen(&p.a_texto())
}

/// La lista firmada del chat, si es compartido.
fn compartido_de<D: Disco + ?Sized>(d: &D, chat: &str) -> Option<ChatCompartido> {
    CompartidosEnDisco::nuevo(&d.raiz()).de(chat)
}

// ------------------------------------------------------------------ subir

/// **Lo que hay que subir**: lo de aqui contra lo que la nube ya tiene.
fn preparar<D: Disco + ?Sized>(
    d: &D,
    chat: &str,
    est: &EstadoDeChat,
    op: &Opciones<'_>,
) -> Result<Preparado, FalloNube> {
    let mut nuevo = est.clone();
    nuevo.faltan.clear();
    let mut p = Parche {
        espacio: (!est.espacio.is_empty()).then(|| est.espacio.clone()),
        base: est.revision,
        tocado: op.ahora,
        ..Parche::default()
    };
    let mut objetos = Vec::new();

    let nombre = d
        .chats()?
        .into_iter()
        .find(|c| c.id == chat)
        .map(|c| c.nombre)
        .unwrap_or_default();
    if nombre != est.nombre {
        p.nombre = Some(nombre.clone());
        nuevo.nombre = nombre;
    }
    let compartido = compartido_de(d, chat);
    if let Some(c) = &compartido {
        let r = resumen_de_compartido(c);
        if est.compartido.as_ref() != Some(&r) {
            p.compartido = Some(c.clone());
            nuevo.compartido = Some(r);
        }
    }

    let mensajes = d.mensajes(chat)?;
    let mut vistos = HashSet::new();
    for m in &mensajes {
        let k = id_de(m);
        let r = kotlin::resumen_de(m);
        if est.mensajes.get(&k) != Some(&r) {
            p.mensajes.poner.push(m.a_valor());
            nuevo.mensajes.insert(k.clone(), r);
        }
        vistos.insert(k);
    }
    for k in est.mensajes.keys().filter(|k| !vistos.contains(*k)) {
        p.mensajes.quitar.push(k.clone());
        nuevo.mensajes.remove(k);
    }

    let proyecto = d.proyecto_portatil(chat)?;
    let rp = proyecto.as_ref().map(resumen_de_proyecto);
    if rp != est.proyecto {
        p.proyecto = Some(proyecto.as_ref().map(Json::a_valor).unwrap_or(Value::Null));
        nuevo.proyecto = rp;
    }

    let mut en_alcance = HashSet::new();
    for info in d.archivos(chat, &HashMap::new())? {
        let rel = info.ruta.clone();
        en_alcance.insert(rel.clone());
        if op.retencion.retenido(chat, &rel) {
            continue;
        }
        let prev = est.archivos.get(&rel);
        if prev.is_some_and(|x| x.local == info.resumen && !x.sha256.is_empty()) {
            continue;
        }
        let tocado = Some(info.tocado);
        if trazos::es_trazos(&rel) {
            let texto = d.texto_de(chat, &rel)?;
            let escena = trazos::escena_de(&texto);
            if let Some(x) = prev.filter(|x| op.deltas && x.con_trazos()) {
                match trazos::calcular(&rel, &x.sha256, &x.conocido(), &escena) {
                    None => {
                        // Lo mismo que la nube (otra forma de escribirlo).
                        if let Some(e) = nuevo.archivos.get_mut(&rel) {
                            e.local = info.resumen.clone();
                        }
                        continue;
                    }
                    Some(delta) => {
                        // Con gzip: la nube mira `1f 8b` (§10.2).
                        let b = comprimir(&delta.to_string());
                        let n = b.len() as u64;
                        if !trazos::compactar(x.deltas.len() + 1, x.bytes_de_deltas() + n, x.bytes)
                        {
                            let s = sha(&b);
                            let mut e = x.clone();
                            e.deltas.push(Delta {
                                sha256: s.clone(),
                                bytes: n,
                            });
                            let mut k = x.conocido();
                            k.con(&delta);
                            e.poner_conocido(k);
                            e.local = info.resumen.clone();
                            p.archivos.poner.push(Archivo {
                                rel: rel.clone(),
                                sha256: x.sha256.clone(),
                                bytes: x.bytes,
                                tocado,
                                deltas: Some(e.deltas.clone()),
                            });
                            objetos.push(Objeto {
                                sha: s,
                                bytes: n,
                                fuente: Fuente::Memoria(b),
                            });
                            nuevo.archivos.insert(rel, e);
                            continue;
                        }
                    }
                }
            }
            // El original entero: nuevo, o toca compactar.
            let b = bytes_de_texto(&rel, &texto);
            let s = sha(&b);
            let mut e = EnLaNube {
                sha256: s.clone(),
                bytes: b.len() as u64,
                local: info.resumen.clone(),
                ..EnLaNube::default()
            };
            if op.deltas {
                e.poner_conocido(Conocido::de(&escena));
            }
            let igual = prev.is_some_and(|x| x.sha256 == s && x.deltas.is_empty());
            if !igual {
                p.archivos.poner.push(Archivo {
                    rel: rel.clone(),
                    sha256: s.clone(),
                    bytes: e.bytes,
                    tocado,
                    deltas: op.deltas.then(Vec::new),
                });
                objetos.push(Objeto {
                    sha: s,
                    bytes: e.bytes,
                    fuente: Fuente::Memoria(b),
                });
            }
            nuevo.archivos.insert(rel, e);
            continue;
        }
        let (s, bytes, fuente) = if disco::es_texto(&rel) {
            let b = bytes_de_texto(&rel, &d.texto_de(chat, &rel)?);
            (sha(&b), b.len() as u64, Fuente::Memoria(b))
        } else {
            (
                info.resumen.clone(),
                info.bytes.max(0) as u64,
                Fuente::Fichero(d.ruta(chat, &rel)),
            )
        };
        if prev.is_none_or(|x| x.sha256 != s) {
            p.archivos.poner.push(Archivo {
                rel: rel.clone(),
                sha256: s.clone(),
                bytes,
                tocado,
                deltas: None,
            });
            objetos.push(Objeto {
                sha: s.clone(),
                bytes,
                fuente,
            });
        }
        nuevo.archivos.insert(
            rel,
            EnLaNube {
                sha256: s,
                bytes,
                local: info.resumen,
                ..EnLaNube::default()
            },
        );
    }
    for rel in est.archivos.keys() {
        if !en_alcance.contains(rel) && !op.retencion.retenido(chat, rel) {
            p.archivos.quitar.push(rel.clone());
            nuevo.archivos.remove(rel);
        }
    }
    Ok(Preparado {
        parche: p,
        objetos,
        nuevo,
        mensajes,
        proyecto,
        compartido,
    })
}

/// Los bytes de un fichero ya sabido, vueltos a sacar del disco (lo que la
/// nube dice que le falta y no estaba en esta subida).
fn bytes_de<D: Disco + ?Sized>(d: &D, chat: &str, rel: &str, s: &str) -> Option<Vec<u8>> {
    let b = if disco::es_texto(rel) {
        bytes_de_texto(rel, &d.texto_de(chat, rel).ok()?)
    } else {
        std::fs::read(d.ruta(chat, rel)).ok()?
    };
    (sha(&b) == s).then_some(b)
}

fn subir_objetos(nube: &Nube<'_>, objetos: &[Objeto], inf: &mut Informe) -> Result<(), FalloNube> {
    let mut hechos = HashSet::new();
    for o in objetos {
        if !hechos.insert(o.sha.clone()) {
            continue;
        }
        if o.bytes > SIN_PREGUNTAR && nube.hay_objeto(&o.sha)? {
            continue;
        }
        nube.subir_objeto(&o.sha, &o.leer()?)?;
        inf.objetos += 1;
        inf.bytes_de_objetos += o.bytes;
    }
    Ok(())
}

/// Sube lo que la nube dice que le falta.
fn subir_faltan<D: Disco + ?Sized>(
    d: &D,
    nube: &Nube<'_>,
    chat: &str,
    faltan: &[String],
    prep: &Preparado,
    inf: &mut Informe,
) -> Vec<String> {
    let mut quedan = Vec::new();
    for s in faltan {
        let bytes = prep
            .objetos
            .iter()
            .find(|o| o.sha == *s)
            .and_then(|o| o.leer().ok())
            .or_else(|| {
                prep.nuevo
                    .archivos
                    .iter()
                    .find(|(_, e)| e.sha256 == *s)
                    .and_then(|(rel, _)| bytes_de(d, chat, rel, s))
            });
        match bytes.map(|b| nube.subir_objeto(s, &b).map(|_| b.len())) {
            Some(Ok(n)) => {
                inf.objetos += 1;
                inf.bytes_de_objetos += n as u64;
            }
            _ => quedan.push(s.clone()),
        }
    }
    quedan
}

/// El inventario entero (protocolo 1).
fn inventario(prep: &Preparado, base: i64, ahora: i64, deltas: bool) -> Inventario {
    let mut archivos: Vec<Archivo> = prep
        .nuevo
        .archivos
        .iter()
        .map(|(rel, e)| Archivo {
            rel: rel.clone(),
            sha256: e.sha256.clone(),
            bytes: e.bytes,
            tocado: None,
            deltas: (deltas && trazos::es_trazos(rel)).then(|| e.deltas.clone()),
        })
        .collect();
    archivos.sort_by(|a, b| a.rel.cmp(&b.rel));
    Inventario {
        nombre: prep.nuevo.nombre.clone(),
        tocado: Some(ahora),
        mensajes: prep.mensajes.iter().map(Json::a_valor).collect(),
        proyecto: prep.proyecto.as_ref().map(Json::a_valor),
        archivos,
        base: (base > 0).then_some(base),
        compartido: prep.compartido.clone(),
        revision: None,
        espacio: None,
    }
}

/// **Sube lo de aqui** (y en un 409 baja, junta y vuelve a intentarlo).
pub fn subir<D: Disco + ?Sized>(
    d: &D,
    nube: &Nube<'_>,
    chat: &str,
    est: &mut EstadoDeChat,
    op: &Opciones<'_>,
    inf: &mut Informe,
) -> Result<(), FalloNube> {
    for _ in 0..INTENTOS {
        let mut prep = preparar(d, chat, est, op)?;
        if prep.parche.vacio() && est.faltan.is_empty() {
            *est = prep.nuevo;
            return Ok(());
        }
        let cuerpo = serde_json::to_vec(&prep.parche).unwrap_or_default();
        // Un parche de mas de 4 MB no lo acepta la nube (§10.1): entonces
        // el inventario entero, como en el protocolo 1.
        if op.deltas && cuerpo.len() <= TOPE_DE_PARCHE {
            subir_objetos(nube, &prep.objetos, inf)?;
            inf.bytes_de_parches += cuerpo.len() as u64;
            match nube.parche(chat, &prep.parche) {
                Ok(s) => {
                    let mut faltan = s.faltan.clone();
                    faltan.extend(est.faltan.iter().cloned());
                    faltan.dedup();
                    let quedan = subir_faltan(d, nube, chat, &faltan, &prep, inf);
                    prep.nuevo.revision = s.revision;
                    if !s.espacio.is_empty() {
                        prep.nuevo.espacio = s.espacio;
                    }
                    prep.nuevo.faltan = quedan;
                    *est = prep.nuevo;
                    inf.subido = true;
                    return Ok(());
                }
                Err(FalloNube::Conflicto(_)) => {
                    inf.conflictos += 1;
                    bajar(d, nube, chat, est, op, inf)?;
                }
                Err(e) => return Err(e),
            }
        } else {
            let inv = inventario(&prep, est.revision, op.ahora, op.deltas);
            inf.bytes_de_parches += serde_json::to_vec(&inv).map_or(0, |v| v.len() as u64);
            match nube.subir_inventario(chat, &inv) {
                Ok(s) => {
                    let mut s = s;
                    if !s.faltan.is_empty() {
                        let quedan = subir_faltan(d, nube, chat, &s.faltan, &prep, inf);
                        let mut otra = inv.clone();
                        otra.base = Some(s.revision);
                        s = nube.subir_inventario(chat, &otra)?;
                        if !quedan.is_empty() {
                            prep.nuevo.faltan = quedan;
                        }
                    }
                    prep.nuevo.revision = s.revision;
                    if !s.espacio.is_empty() {
                        prep.nuevo.espacio = s.espacio;
                    }
                    *est = prep.nuevo;
                    inf.subido = true;
                    return Ok(());
                }
                Err(FalloNube::Conflicto(_)) => {
                    inf.conflictos += 1;
                    bajar(d, nube, chat, est, op, inf)?;
                }
                Err(e) => return Err(e),
            }
        }
    }
    Err(FalloNube::Conflicto(est.revision))
}

// ------------------------------------------------------------------ bajar

/// **Baja lo que subieron los demas** desde la ultima revision y lo junta.
pub fn bajar<D: Disco + ?Sized>(
    d: &D,
    nube: &Nube<'_>,
    chat: &str,
    est: &mut EstadoDeChat,
    op: &Opciones<'_>,
    inf: &mut Informe,
) -> Result<(), FalloNube> {
    if op.deltas {
        let c = match nube.cambios(&est.espacio, chat, est.revision) {
            Ok(c) => c,
            Err(FalloNube::NoEsta) => return Ok(()),
            Err(e) => return Err(e),
        };
        if !c.reiniciar {
            let mut v = Valla::default();
            for p in &c.cambios {
                aplicar_parche(d, nube, chat, est, op, p, &mut v)?;
                if let Some(r) = p.revision {
                    est.revision = est.revision.max(r);
                }
                inf.bajados += 1;
            }
            est.revision = est.revision.max(c.revision);
            return Ok(());
        }
    }
    bajar_entero(d, nube, chat, est, op, inf)
}

/// Baja el inventario entero y lo junta como si fuera un parche.
fn bajar_entero<D: Disco + ?Sized>(
    d: &D,
    nube: &Nube<'_>,
    chat: &str,
    est: &mut EstadoDeChat,
    op: &Opciones<'_>,
    inf: &mut Informe,
) -> Result<(), FalloNube> {
    let inv = match nube.inventario(&est.espacio, chat) {
        Ok(i) => i,
        Err(FalloNube::NoEsta) => return Ok(()),
        Err(e) => return Err(e),
    };
    let p = parche_de_inventario(est, &inv);
    aplicar_parche(d, nube, chat, est, op, &p, &mut Valla::default())?;
    if let Some(r) = inv.revision {
        est.revision = r;
    }
    if let Some(e) = inv.espacio.filter(|e| !e.is_empty()) {
        est.espacio = e;
    }
    inf.bajados += 1;
    Ok(())
}

/// **Un inventario como parche** contra lo sabido: lo que difiere.
pub fn parche_de_inventario(est: &EstadoDeChat, inv: &Inventario) -> Parche {
    let mut p = Parche {
        base: est.revision,
        tocado: inv.tocado.unwrap_or(0),
        nombre: (inv.nombre != est.nombre).then(|| inv.nombre.clone()),
        compartido: inv.compartido.clone(),
        revision: inv.revision,
        ..Parche::default()
    };
    let mut vistos = HashSet::new();
    for v in &inv.mensajes {
        let m = Json::de_valor(v);
        let k = id_de(&m);
        if est.mensajes.get(&k) != Some(&kotlin::resumen_de(&m)) {
            p.mensajes.poner.push(v.clone());
        }
        vistos.insert(k);
    }
    p.mensajes.quitar = est
        .mensajes
        .keys()
        .filter(|k| !vistos.contains(*k))
        .cloned()
        .collect();
    let rp = inv
        .proyecto
        .as_ref()
        .filter(|v| !v.is_null())
        .map(|v| resumen_de_proyecto(&Json::de_valor(v)));
    if rp.is_some() && rp != est.proyecto {
        p.proyecto = inv.proyecto.clone();
    }
    let mut rels = HashSet::new();
    for a in &inv.archivos {
        rels.insert(a.rel.clone());
        let igual = est
            .archivos
            .get(&a.rel)
            .is_some_and(|x| x.sha256 == a.sha256 && x.deltas == a.deltas());
        if !igual {
            p.archivos.poner.push(a.clone());
        }
    }
    p.archivos.quitar = est
        .archivos
        .keys()
        .filter(|r| !rels.contains(*r))
        .cloned()
        .collect();
    p
}

/// Lo que no se deja escribir a lo que llega: los ficheros de los demas
/// chats (se calcula una vez por vuelta, y solo si llega algun fichero).
#[derive(Default)]
struct Valla {
    prohibidos: Option<HashSet<PathBuf>>,
}

impl Valla {
    fn deja<D: Disco + ?Sized>(&mut self, d: &D, chat: &str, rel: &str) -> bool {
        if !disco::permitida(rel) || compartidos::salta_de_carpeta(chat, rel) {
            return false;
        }
        let p = self
            .prohibidos
            .get_or_insert_with(|| compartidos::prohibidos_para(d, &[chat.to_string()]));
        !p.contains(&d.ruta(chat, rel))
    }
}

/// **Pone un parche de la nube aqui.**
fn aplicar_parche<D: Disco + ?Sized>(
    d: &D,
    nube: &Nube<'_>,
    chat: &str,
    est: &mut EstadoDeChat,
    op: &Opciones<'_>,
    p: &Parche,
    valla: &mut Valla,
) -> Result<(), FalloNube> {
    let mut cuenta = Cuenta::default();
    let criterio = Criterio::default();
    if let Some(n) = &p.nombre {
        est.nombre = n.clone();
    }
    if let Some(c) = &p.compartido
        && c.chat == chat
    {
        let cd = CompartidosEnDisco::nuevo(&d.raiz());
        cd.cambiar(|l| -> std::io::Result<_> {
            let mio = l.iter().find(|x| x.chat == chat).cloned();
            let junto = compartidos::juntar(mio.as_ref(), c);
            let mut l: Vec<ChatCompartido> = l.into_iter().filter(|x| x.chat != chat).collect();
            l.push(junto);
            Ok(l)
        })?;
        est.compartido = Some(resumen_de_compartido(c));
    }
    if let Some(v) = p.proyecto.as_ref().filter(|v| !v.is_null()) {
        let suyo = Json::de_valor(v);
        let suyo = kotlin::normalizar_proyecto(&suyo).unwrap_or(suyo);
        let rs = resumen_de_proyecto(&suyo);
        let mio = d.proyecto_portatil(chat)?;
        let fin = match &mio {
            Some(m) if Some(resumen_de_proyecto(m)) != est.proyecto => {
                fusion::json(None, Some(m), Some(&suyo), &criterio, &mut cuenta).unwrap_or(suyo)
            }
            _ => suyo,
        };
        if mio.as_ref().map(resumen_de_proyecto) != Some(resumen_de_proyecto(&fin)) {
            d.guardar_proyecto(&disco::actualizado(mio.as_ref(), &fin))?;
        }
        est.proyecto = Some(rs);
    }

    if !p.mensajes.vacia() {
        let locales: HashMap<String, Json> = d
            .mensajes(chat)?
            .into_iter()
            .map(|m| (id_de(&m), m))
            .collect();
        let mut poner = Vec::new();
        for v in &p.mensajes.poner {
            let m = Json::de_valor(v);
            let m = kotlin::normalizar_mensaje(&m).unwrap_or(m);
            let k = id_de(&m);
            let r = kotlin::resumen_de(&m);
            let fin = match locales.get(&k) {
                Some(l)
                    if est.mensajes.get(&k) != Some(&kotlin::resumen_de(l))
                        && kotlin::resumen_de(l) != r =>
                {
                    fusion::json(None, Some(l), Some(&m), &criterio, &mut cuenta).unwrap_or(m)
                }
                _ => m,
            };
            if locales.get(&k).map(kotlin::resumen_de) != Some(kotlin::resumen_de(&fin)) {
                poner.push(fin.a_texto());
            }
            est.mensajes.insert(k, r);
        }
        let mut quitar = Vec::new();
        for k in &p.mensajes.quitar {
            if let Some(l) = locales.get(k)
                && est.mensajes.get(k) == Some(&kotlin::resumen_de(l))
            {
                quitar.push(kotlin::unico(l));
            }
            est.mensajes.remove(k);
        }
        if !poner.is_empty() || !quitar.is_empty() {
            d.aplicar_mensajes(chat, &poner, &quitar, op.ahora)?;
        }
    }

    for a in &p.archivos.poner {
        if !valla.deja(d, chat, &a.rel) {
            // Un fichero fuera de este chat: no se escribe.
            continue;
        }
        aplicar_archivo(d, nube, chat, est, op, a)?;
    }
    for rel in &p.archivos.quitar {
        let Some(x) = est.archivos.remove(rel) else {
            continue;
        };
        if valla.deja(d, chat, rel)
            && !op.retencion.retenido(chat, rel)
            && d.resumen_de_archivo(chat, rel).ok().as_ref() == Some(&x.local)
        {
            let _ = std::fs::remove_file(d.ruta(chat, rel));
        }
    }
    Ok(())
}

/// Los deltas de un fichero, bajados y leidos.
fn bajar_deltas(nube: &Nube<'_>, deltas: &[Delta]) -> Result<Vec<Value>, FalloNube> {
    deltas
        .iter()
        .map(|x| {
            let t = descomprimir(&nube.bajar_objeto(&x.sha256)?)?;
            let v: Value =
                serde_json::from_str(&t).map_err(|e| FalloNube::Formato(e.to_string()))?;
            if !trazos::es_delta(&v) {
                return Err(FalloNube::Formato("un delta que no lo es".into()));
            }
            Ok(v)
        })
        .collect()
}

/// Lee la copia de un fichero sin conexion (comprimida o no) como texto.
fn leer_copia(f: &std::path::Path) -> Option<(String, bool)> {
    let b = std::fs::read(f).ok()?;
    let gz = b.starts_with(&[0x1f, 0x8b]);
    Some((descomprimir(&b).ok()?, gz))
}

fn escribir_copia(f: &std::path::Path, texto: &str, gz: bool) -> std::io::Result<()> {
    let b = if gz {
        comprimir(texto)
    } else {
        texto.as_bytes().to_vec()
    };
    disco::escribir_atomico(f, &b)
}

/// **Pone un fichero de la nube aqui**: de un lienzo, juntando trazo a
/// trazo; de lo demas, entero (salvo que aqui tambien cambiara: entonces
/// un JSON se junta y lo demas se queda como esta, que sube despues).
fn aplicar_archivo<D: Disco + ?Sized>(
    d: &D,
    nube: &Nube<'_>,
    chat: &str,
    est: &mut EstadoDeChat,
    op: &Opciones<'_>,
    a: &Archivo,
) -> Result<(), FalloNube> {
    let rel = &a.rel;
    let prev = est.archivos.get(rel).cloned();
    if prev
        .as_ref()
        .is_some_and(|x| x.sha256 == a.sha256 && x.deltas == a.deltas())
    {
        return Ok(());
    }
    let existe = d.ruta(chat, rel).is_file();
    let local = if existe {
        d.resumen_de_archivo(chat, rel).ok()
    } else {
        None
    };
    // Lo de aqui cambio desde lo ultimo que se supo de la nube.
    let cambiado_aqui = local.is_some() && prev.as_ref().map(|x| &x.local) != local.as_ref();
    let copia = op
        .retencion
        .copia(chat, rel)
        .filter(|_| op.retencion.retenido(chat, rel));

    if trazos::es_trazos(rel) {
        let incremental = prev.as_ref().filter(|x| {
            existe && x.con_trazos() && x.sha256 == a.sha256 && a.deltas().starts_with(&x.deltas)
        });
        let (nuevo_aqui, conocido, nuevos_deltas, entera) = match incremental {
            Some(x) => {
                let ds = bajar_deltas(nube, &a.deltas()[x.deltas.len()..])?;
                let mut escena = trazos::escena_de(&d.texto_de(chat, rel)?);
                let mut k = x.conocido();
                for v in &ds {
                    trazos::aplicar(&mut escena, v);
                    k.con(v);
                }
                (escena, k, ds, None)
            }
            None => {
                let base = descomprimir(&nube.bajar_objeto(&a.sha256)?)?;
                let ds = bajar_deltas(nube, a.deltas())?;
                let efectivo = trazos::efectivo(&trazos::escena_de(&base), &ds);
                let k = Conocido::de(&efectivo);
                let aqui = match existe.then(|| d.texto_de(chat, rel)).transpose()? {
                    Some(t) => trazos::juntar(&trazos::escena_de(&t), &efectivo),
                    None => efectivo.clone(),
                };
                (aqui, k, ds, Some(efectivo))
            }
        };
        d.escribir_texto(chat, rel, &nuevo_aqui.to_string())?;
        if let Some(f) = &copia {
            let (t, gz) = leer_copia(f).unwrap_or_default();
            let mut c = trazos::escena_de(&t);
            match &entera {
                Some(e) => c = trazos::juntar(&c, e),
                None => {
                    for v in &nuevos_deltas {
                        trazos::aplicar(&mut c, v);
                    }
                }
            }
            escribir_copia(f, &d.a_local(chat, c.to_string()), gz)?;
        }
        let mut e = EnLaNube {
            sha256: a.sha256.clone(),
            bytes: a.bytes,
            deltas: a.deltas().to_vec(),
            ..EnLaNube::default()
        };
        e.poner_conocido(conocido);
        e.local = if cambiado_aqui {
            // Lo de aqui sin subir sigue pendiente: la proxima vuelta saca
            // su delta contra lo conocido.
            prev.map(|x| x.local).unwrap_or_default()
        } else {
            d.resumen_de_archivo(chat, rel)?
        };
        est.archivos.insert(rel.clone(), e);
        return Ok(());
    }

    let mut e = EnLaNube {
        sha256: a.sha256.clone(),
        bytes: a.bytes,
        ..EnLaNube::default()
    };
    if !disco::es_texto(rel) && local.as_deref() == Some(a.sha256.as_str()) {
        e.local = a.sha256.clone();
        est.archivos.insert(rel.clone(), e);
        return Ok(());
    }
    let bytes = nube.bajar_objeto(&a.sha256)?;
    let es_json = rel.ends_with(".json");
    let juntado = |mio: &str, suyo: &str| -> Option<String> {
        let m = Json::analizar(mio).ok()?;
        let s = Json::analizar(suyo).ok()?;
        fusion::json(
            None,
            Some(&m),
            Some(&s),
            &Criterio::default(),
            &mut Cuenta::default(),
        )
        .map(|j| j.a_texto())
    };
    let mut escrito = true;
    if disco::es_texto(rel) {
        let suyo = descomprimir(&bytes)?;
        let fin = if cambiado_aqui && es_json {
            juntado(&d.texto_de(chat, rel)?, &suyo).unwrap_or(suyo.clone())
        } else {
            suyo.clone()
        };
        d.escribir_texto(chat, rel, &fin)?;
        if let Some(f) = &copia {
            let gz = leer_copia(f).is_some_and(|x| x.1);
            escribir_copia(f, &d.a_local(chat, suyo), gz)?;
        }
    } else {
        let fin = if cambiado_aqui {
            if es_json {
                let mio = std::fs::read_to_string(d.ruta(chat, rel)).unwrap_or_default();
                std::str::from_utf8(&bytes)
                    .ok()
                    .and_then(|s| juntado(&mio, s))
                    .map(String::into_bytes)
            } else {
                // Lo de aqui gana: sube en la vuelta siguiente.
                escrito = false;
                None
            }
        } else {
            Some(bytes.clone())
        };
        if let Some(b) = fin {
            d.escribir_archivo_de(chat, rel, b.len() as i64, &mut |w| {
                w.write_all(&b)?;
                Ok(true)
            })?;
        }
        if let Some(f) = &copia {
            disco::escribir_atomico(f, &bytes)?;
        }
    }
    e.local = if cambiado_aqui || !escrito {
        prev.map(|x| x.local).unwrap_or_default()
    } else {
        d.resumen_de_archivo(chat, rel)?
    };
    est.archivos.insert(rel.clone(), e);
    Ok(())
}

/// **Una vuelta entera de un chat**: lo sabido de disco, bajar (si toca o
/// es la primera vez), subir, y lo sabido otra vez a disco.
pub fn vuelta_de_chat<D: Disco + ?Sized>(
    d: &D,
    nube: &Nube<'_>,
    chat: &str,
    op: &Opciones<'_>,
) -> Result<Informe, FalloNube> {
    let sincro = d.sincro();
    let mut est = EstadoDeChat::leer(&sincro, chat);
    if est.espacio.is_empty()
        && let Some(c) = compartido_de(d, chat)
    {
        est.espacio = super::espacio_compartido(&c.creador, chat);
    }
    let mut inf = Informe::default();
    let r = (|| {
        if op.bajar || est.revision == 0 {
            match bajar(d, nube, chat, &mut est, op, &mut inf) {
                // Un compartido que aun no esta en la nube: la nube no sabe
                // si estoy dentro hasta que suba su lista (403). Se sube.
                Err(FalloNube::Llave(_)) if est.revision == 0 => {}
                r => r?,
            }
        }
        subir(d, nube, chat, &mut est, op, &mut inf)
    })();
    match &r {
        Ok(()) => {
            est.al_dia = op.ahora;
            est.error = None;
        }
        Err(e) => est.error = Some(e.to_string()),
    }
    est.guardar(&sincro, chat)?;
    r.map(|_| inf)
}

/// Apunta en que espacio vive un chat (lo que dijo `GET chats`), para
/// bajarlo la primera vez sin tener aun su lista firmada.
pub fn conocer_espacio(sincro: &std::path::Path, chat: &str, espacio: &str) -> std::io::Result<()> {
    let mut est = EstadoDeChat::leer(sincro, chat);
    if est.espacio != espacio {
        est.espacio = espacio.to_string();
        est.guardar(sincro, chat)?;
    }
    Ok(())
}
