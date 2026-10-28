//! **Las fotos por su hora** (28-oct-2026).
//!
//! El usuario: «que se puedan tomar fotos normalmente aunque NO se este
//! conectado; cuando logre conectarse, como la foto lleva su hora y la
//! grabacion tambien guarda su hora, si la hora de la foto esta fuera de la
//! grabacion, que la ignore; si esta dentro, que calcule donde va y la
//! ponga ahi». Y: «agregar COLABORADORES: personas que pueden tomar fotos en
//! cualquier momento —no hace falta que se sincronicen antes—, cada uno toma
//! fotos y todo se adjunta a la grabacion».
//!
//! Hasta ahora una foto entraba en su minuto solo si el movil sabia de la
//! grabacion (el reloj que le pasa el PC, `pixpin_sincro::camara`). Con esto
//! **la hora de pared manda**: la grabacion guarda en su nota de voz cuando
//! empezo, cuando acabo y sus pausas, en el reloj de quien graba
//! ([`RelojDeGrabacion`], `resto.reloj`), y cada foto lleva la hora en que
//! se hizo (`resto.tomadaEn`) y de que aparato es (`aparato`). Llegue por
//! donde llegue (la sincronizacion normal o la del diario, «Enviar por
//! Wi-Fi», un chat compartido o la camara en directo), el aparato que grabo
//! hace la cuenta:
//!
//! 1. **La hora corregida** ([`hora_corregida`]): la de la foto menos lo que
//!    iba adelantado el reloj de su aparato, con la medida del desfase mas
//!    cercana a esa hora ([`Desfases::cercano`], medidas en cada saludo de
//!    sincronizar). Sin medida (un colaborador cuyo aparato nunca hablo con
//!    este), se cree su hora tal cual y la foto queda **«hora sin
//!    comprobar»**.
//! 2. **El minuto** ([`ms_en_la_grabacion`]): fuera de la ventana de la
//!    grabacion, `None` (la foto se queda como un mensaje normal); dentro,
//!    lo que corrio desde el principio sin contar lo pausado. Una foto hecha
//!    **en una pausa** va al minuto en que se paro: es lo ultimo que se oyo,
//!    y lo que se fotografia en una pausa (la pizarra mientras se copia) es
//!    de lo dicho justo antes.
//! 3. **Quien puede aportar** ([`admitido`]): lo de mis aparatos siempre;
//!    lo de otra persona, solo si quien graba la invito
//!    (`resto.colaboradores`, sus codigos o [`TODOS`] los del chat).
//! 4. **Se pone una vez** ([`decidir`], [`colocar`]): en la lista de la
//!    nota (`capturas`, con su autor) y en la carpeta de la grabacion, en
//!    su minuto. Lo ya puesto (por su id o por el uid de la foto, que la
//!    camara en directo guarda en `fotoUid`) no se repite.
//!
//! **Solo coloca el aparato que grabo** (`reloj.aparato`): si cada aparato
//! de uno lo hiciera, dos cambiarian la misma nota a la vez. La foto de otra
//! persona no se toca (en un chat compartido lo ajeno no se cambia): lo que
//! dice donde va esta en la nota, que es de quien graba.
//!
//! Todo es logica pura salvo [`colocar`], [`repasar`] y [`a_mano`], que
//! leen y escriben el cuaderno de un chat. La guia para Android esta en
//! `docs/movil-como-camara.md` («Fotos sin conexión y por su hora»,
//! «Colaboradores»).

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::carpeta_de_paginas::{self, Carpeta};
use crate::clase;
use crate::cuaderno::{self, Clase, Mensaje};
use crate::presencial;

/// La clave del reloj en el `resto` de la nota de voz.
pub const RELOJ: &str = "reloj";
/// La clave de la hora de una foto (ms desde 1970, en el reloj de su
/// aparato).
pub const TOMADA_EN: &str = "tomadaEn";
/// La copia de la hora del EXIF, por si falta `tomadaEn`:
/// `"2026:10:28 10:05:03"` (`DateTimeOriginal`).
pub const TOMADA_EN_EXIF: &str = "tomadaEnExif";
/// Y su desfase horario (`OffsetTimeOriginal`, `"+01:00"`).
pub const DESFASE_EXIF: &str = "desfaseExif";
/// Una foto que su aparato ya sabia para que grabacion era (el id de la
/// nota, o el de la grabacion de la camara en directo).
pub const PARA_GRABACION: &str = "paraGrabacion";
/// En la nota: quien puede aportar fotos, por su codigo de aparato.
pub const COLABORADORES: &str = "colaboradores";
/// En [`COLABORADORES`]: todos los del chat compartido.
pub const TODOS: &str = "*";
/// En una captura de la nota: su hora no se pudo comprobar.
pub const SIN_COMPROBAR: &str = "horaSinComprobar";
/// En un mensaje copiado por la camara en directo: el uid de la foto en su
/// aparato (su `clave`), para no ponerla dos veces si llega tambien por la
/// sincronizacion.
pub const FOTO_UID: &str = "fotoUid";
/// Lo que se deja pasar en los bordes de la grabacion: una foto de 20 s
/// antes de empezar va al principio y una de 20 s despues de acabar, al
/// final. Mas alla, fuera. Absorbe el error de un reloj medido y el de la
/// nota, que empieza un poco despues de pulsar.
pub const MARGEN_MS: i64 = 30_000;
/// Cuantas medidas del desfase se recuerdan de cada aparato.
pub const MEDIDAS_MAXIMAS: usize = 32;

fn es_cero(v: &i64) -> bool {
    *v == 0
}

// ------------------------------------------------------------ el reloj

/// **La ventana de una grabacion en la hora de pared**, como va en la nota
/// (`resto.reloj`). Todo en ms desde 1970 en el reloj de quien graba.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RelojDeGrabacion {
    /// El codigo del aparato que grabo (`K7Q2`): el unico que coloca.
    pub aparato: String,
    /// Cuando empezo.
    pub inicio: i64,
    /// Cuando acabo. 0: sigue (o se corto sin acabar, ver `visto`).
    #[serde(skip_serializing_if = "es_cero")]
    pub fin: i64,
    /// Las pausas, `[desde, hasta]`; `hasta` 0: en pausa ahora.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pausas: Vec<[i64; 2]>,
    /// La ultima vez que se escribio grabando (cada poco): si el programa
    /// se cae, la ventana acaba aqui y no queda abierta para siempre.
    #[serde(rename = "vistoEn", skip_serializing_if = "es_cero")]
    pub visto: i64,
}

impl RelojDeGrabacion {
    /// El de la nota, si lo lleva.
    pub fn de(resto: &Map<String, Value>) -> Option<RelojDeGrabacion> {
        let r: RelojDeGrabacion = serde_json::from_value(resto.get(RELOJ)?.clone()).ok()?;
        (r.inicio > 0).then_some(r)
    }

    /// Lo pone en la nota.
    pub fn poner(&self, resto: &mut Map<String, Value>) {
        if let Ok(v) = serde_json::to_value(self) {
            resto.insert(RELOJ.into(), v);
        }
    }

    /// Donde acaba la ventana: el fin, o lo ultimo visto si se corto.
    /// `None`: sigue abierta (grabando ahora).
    pub fn final_(&self) -> Option<i64> {
        if self.fin > 0 {
            Some(self.fin.max(self.inicio))
        } else if self.visto > 0 {
            Some(self.visto.max(self.inicio))
        } else {
            None
        }
    }

    /// Lo que duro (o lleva, si sigue, hasta `ahora`), sin las pausas.
    pub fn duracion(&self, ahora: i64) -> i64 {
        let fin = self.final_().unwrap_or(ahora);
        self.ms_hasta(fin)
    }

    /// Lo corrido hasta `t` sin lo pausado (sin mirar si esta dentro).
    fn ms_hasta(&self, t: i64) -> i64 {
        let tope = self.final_().map_or(t, |f| t.min(f));
        if tope <= self.inicio {
            return 0;
        }
        let pausado: i64 = self
            .pausas
            .iter()
            .map(|[d, h]| {
                let h = if *h == 0 { tope } else { (*h).min(tope) };
                (h - (*d).max(self.inicio)).max(0)
            })
            .sum();
        (tope - self.inicio - pausado).max(0)
    }

    /// Si `t` cae en una pausa: su principio.
    fn pausa_en(&self, t: i64) -> Option<i64> {
        self.pausas
            .iter()
            .find(|[d, h]| t >= *d && (*h == 0 || t < *h))
            .map(|[d, _]| *d)
    }
}

/// **El milisegundo de la grabacion en la hora de pared `t`** (ya corregida
/// al reloj de quien graba). `None` si cae fuera de la ventana (antes de
/// empezar o despues de acabar). Dentro de una pausa, el minuto en que se
/// paro (ver el principio del modulo).
pub fn ms_en_la_grabacion(t: i64, reloj: &RelojDeGrabacion) -> Option<i64> {
    if reloj.inicio <= 0 || t < reloj.inicio {
        return None;
    }
    if reloj.final_().is_some_and(|f| t > f) {
        return None;
    }
    let t = reloj.pausa_en(t).unwrap_or(t);
    Some(reloj.ms_hasta(t))
}

/// [`ms_en_la_grabacion`] con [`MARGEN_MS`] en los bordes y sin pasar de
/// `duracion` (la de la nota, si se sabe): lo de justo antes va al 0 y lo de
/// justo despues, al final.
pub fn lugar(t: i64, reloj: &RelojDeGrabacion, duracion: Option<i64>, margen: i64) -> Option<i64> {
    let tope = duracion
        .filter(|d| *d > 0)
        .unwrap_or_else(|| reloj.duracion(t));
    let ms = match ms_en_la_grabacion(t, reloj) {
        Some(ms) => ms,
        None if reloj.inicio > 0 && t < reloj.inicio && reloj.inicio - t <= margen => 0,
        None => match reloj.final_() {
            Some(f) if t > f && t - f <= margen => reloj.ms_hasta(f),
            _ => return None,
        },
    };
    Some(ms.clamp(0, tope.max(0)))
}

// ------------------------------------------------------------ los desfases

/// Una medida del desfase de un aparato: cuando se midio (en mi reloj) y lo
/// que iba adelantado el suyo (negativo: atrasado).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Medida {
    pub cuando: i64,
    pub desfase: i64,
}

/// **La medida mas cercana a la hora `t`**: los relojes se van moviendo (un
/// movil que corrige su hora por la red, otro que no), y lo que valia el
/// martes no tiene por que valer el viernes.
pub fn desfase_cercano(medidas: &[Medida], t: i64) -> Option<i64> {
    medidas
        .iter()
        .min_by_key(|m| (m.cuando - t).unsigned_abs())
        .map(|m| m.desfase)
}

/// **Lo medido de cada aparato**, por su codigo (`K7Q2`), como se guarda en
/// `sincro/desfases.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Desfases {
    pub aparatos: BTreeMap<String, Vec<Medida>>,
}

impl Desfases {
    /// Apunta una medida. Dos del mismo minuto se quedan en la ultima; de
    /// cada aparato, las [`MEDIDAS_MAXIMAS`] mas nuevas.
    pub fn medir(&mut self, codigo: &str, cuando: i64, desfase: i64) {
        if codigo.trim().is_empty() {
            return;
        }
        let v = self.aparatos.entry(codigo.to_string()).or_default();
        v.retain(|m| (m.cuando - cuando).abs() >= 60_000);
        v.push(Medida { cuando, desfase });
        v.sort_by_key(|m| m.cuando);
        let sobran = v.len().saturating_sub(MEDIDAS_MAXIMAS);
        v.drain(..sobran);
    }

    /// La medida de `codigo` mas cercana a `t`.
    pub fn cercano(&self, codigo: &str, t: i64) -> Option<i64> {
        desfase_cercano(self.aparatos.get(codigo)?, t)
    }

    pub fn leer(fichero: &Path) -> Desfases {
        std::fs::read_to_string(fichero)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn guardar(&self, fichero: &Path) -> std::io::Result<()> {
        if let Some(d) = fichero.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = fichero.with_extension("json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec(self).map_err(std::io::Error::other)?,
        )?;
        std::fs::rename(&tmp, fichero)
    }
}

// ------------------------------------------------------------ la foto

/// **La hora del EXIF** (`DateTimeOriginal`, `"2026:10:28 10:05:03"`, y
/// `OffsetTimeOriginal`, `"+01:00"`) en ms desde 1970. Sin desfase horario,
/// `None`: una hora local sin zona no se puede pasar a la de pared.
pub fn ms_de_exif(fecha: &str, desfase: &str) -> Option<i64> {
    let f = fecha.trim();
    let (dia, hora) = f.split_once([' ', 'T'])?;
    let mut d = dia.split([':', '-']).map(|x| x.parse::<i64>().ok());
    let (a, me, di) = (d.next()??, d.next()??, d.next()??);
    let mut h = hora.split(':').map(|x| x.parse::<i64>().ok());
    let (ho, mi, se) = (h.next()??, h.next()??, h.next().flatten().unwrap_or(0));
    if !(1..=12).contains(&me) || !(1..=31).contains(&di) || ho > 23 || mi > 59 || se > 60 {
        return None;
    }
    let z = desfase.trim();
    let (signo, resto) = match z.chars().next()? {
        '+' => (1, &z[1..]),
        '-' => (-1, &z[1..]),
        'Z' if z == "Z" => (1, "00:00"),
        _ => return None,
    };
    let (zh, zm) = resto.split_once(':').unwrap_or((resto, "0"));
    let zona = signo * (zh.parse::<i64>().ok()? * 60 + zm.parse::<i64>().ok()?);
    // Dias desde 1970 de una fecha civil (Howard Hinnant).
    let y = if me <= 2 { a - 1 } else { a };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (me + 9) % 12;
    let doy = (153 * mp + 2) / 5 + di - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let dias = era * 146_097 + doe - 719_468;
    Some(((dias * 86_400 + ho * 3600 + mi * 60 + se) - zona * 60) * 1000)
}

/// **Lo que hace falta de una foto** para ponerla por su hora.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FotoConHora {
    /// El id del mensaje en este chat.
    pub id: String,
    /// Su codigo unico (el mismo en todos los aparatos).
    pub uid: String,
    /// El codigo del aparato que la hizo (vacio: de los de antes, mio).
    pub aparato: String,
    /// Su hora, en el reloj de su aparato.
    pub tomada_en: i64,
    /// Para que grabacion la hizo, si lo sabia.
    pub para: Option<String>,
}

/// La foto de un mensaje: una IMAGEN con su hora (`tomadaEn` o, si falta,
/// la del EXIF). `None` lo demas.
pub fn foto_de(m: &Mensaje) -> Option<FotoConHora> {
    if m.clase != Some(Clase::Imagen) || cuaderno::es_de_sistema(m) {
        return None;
    }
    let tomada_en = m
        .resto
        .get(TOMADA_EN)
        .and_then(Value::as_i64)
        .filter(|t| *t > 0)
        .or_else(|| {
            let f = m.resto.get(TOMADA_EN_EXIF)?.as_str()?;
            let z = m.resto.get(DESFASE_EXIF)?.as_str()?;
            ms_de_exif(f, z)
        })?;
    Some(FotoConHora {
        id: m.id.clone(),
        uid: m.codigo_unico(),
        aparato: m.aparato.clone().unwrap_or_default().trim().to_string(),
        tomada_en,
        para: m
            .resto
            .get(PARA_GRABACION)
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string),
    })
}

/// La hora de una foto en el reloj de quien graba.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hora {
    pub utc: i64,
    /// `false`: no habia medida del reloj de su aparato y se creyo su hora.
    pub comprobada: bool,
}

/// **La hora de la foto pasada al reloj de quien graba** (`dueno`, su
/// codigo): la del mismo aparato, tal cual; la de otro, menos su desfase
/// mas cercano a esa hora; sin medida, tal cual y sin comprobar.
pub fn hora_corregida(foto: &FotoConHora, dueno: &str, desfases: &Desfases) -> Hora {
    if foto.aparato.is_empty() || foto.aparato == dueno {
        return Hora {
            utc: foto.tomada_en,
            comprobada: true,
        };
    }
    match desfases.cercano(&foto.aparato, foto.tomada_en) {
        Some(d) => Hora {
            utc: foto.tomada_en - d,
            comprobada: true,
        },
        None => Hora {
            utc: foto.tomada_en,
            comprobada: false,
        },
    }
}

// ------------------------------------------------------------ la grabacion

/// **Una grabacion de un chat que admite fotos por su hora**: su nota de
/// voz con reloj, su carpeta y lo que ya tiene dentro.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Grabacion {
    pub voz: String,
    pub carpeta: Option<String>,
    pub reloj: RelojDeGrabacion,
    /// La de la nota (la del audio de verdad), si se sabe.
    pub duracion_ms: i64,
    pub colaboradores: Vec<String>,
    /// Los ids (y uids) de lo que ya esta dentro.
    pub ya: HashSet<String>,
    /// El titulo de la clase, para el rotulo de la carpeta.
    pub titulo: String,
}

/// Los codigos invitados de una nota (`resto.colaboradores`).
pub fn colaboradores_de(resto: &Map<String, Value>) -> Vec<String> {
    resto
        .get(COLABORADORES)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// **Invita** (`true`) o deja de invitar a `codigo` ([`TODOS`]: a todos los
/// del chat). Devuelve si cambio.
pub fn invitar(resto: &mut Map<String, Value>, codigo: &str, si: bool) -> bool {
    let codigo = codigo.trim();
    if codigo.is_empty() {
        return false;
    }
    let mut v = colaboradores_de(resto);
    let estaba = v.iter().any(|c| c == codigo);
    if estaba == si {
        return false;
    }
    if si {
        v.push(codigo.to_string());
    } else {
        v.retain(|c| c != codigo);
    }
    if v.is_empty() {
        resto.remove(COLABORADORES);
    } else {
        resto.insert(COLABORADORES.into(), serde_json::json!(v));
    }
    true
}

/// **Si lo de `aparato` puede entrar** en esta grabacion: lo mio (`mios`,
/// mis codigos; vacio, un mensaje de antes, tambien) siempre; lo de otra
/// persona, si la invitaron a ella o a todos.
pub fn admitido(g: &Grabacion, aparato: &str, mios: &HashSet<String>) -> bool {
    aparato.is_empty()
        || aparato == g.reloj.aparato
        || mios.contains(aparato)
        || g.colaboradores.iter().any(|c| c == TODOS || c == aparato)
}

/// **Las grabaciones con reloj de un chat**, con su carpeta y lo que ya
/// tienen dentro.
pub fn grabaciones_de(mensajes: &[Mensaje]) -> Vec<Grabacion> {
    let carpetas: Vec<(String, Carpeta)> = mensajes
        .iter()
        .filter_map(|m| {
            let c = Carpeta::de_mensaje(m)?;
            c.es_grabacion().then(|| (m.id.clone(), c))
        })
        .collect();
    let fuid: BTreeMap<&str, &str> = mensajes
        .iter()
        .filter_map(|m| Some((m.id.as_str(), m.resto.get(FOTO_UID)?.as_str()?)))
        .collect();
    mensajes
        .iter()
        .filter(|m| m.clase == Some(Clase::Voz))
        .filter_map(|v| {
            let reloj = RelojDeGrabacion::de(&v.resto)?;
            let carpeta = carpetas.iter().find(|(_, c)| c.voz == v.id);
            let mut ya: HashSet<String> = clase::capturas_de(&v.resto)
                .into_iter()
                .map(|(_, id)| id)
                .chain(presencial::clips_de(&v.resto).into_iter().map(|c| c.id))
                .collect();
            for c in v
                .resto
                .get(clase::CAPTURAS)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(u) = c.get("uid").and_then(Value::as_str) {
                    ya.insert(u.to_string());
                }
            }
            if let Some((_, c)) = carpeta {
                ya.extend(c.paginas.iter().map(|p| p.id.clone()));
            }
            let fotos: Vec<String> = ya
                .iter()
                .filter_map(|id| fuid.get(id.as_str()).map(|u| u.to_string()))
                .collect();
            ya.extend(fotos);
            Some(Grabacion {
                voz: v.id.clone(),
                carpeta: carpeta.map(|(id, _)| id.clone()),
                reloj,
                duracion_ms: v.duracion_ms.max(0),
                colaboradores: colaboradores_de(&v.resto),
                ya,
                titulo: if v.nombre.trim().is_empty() {
                    v.texto.clone()
                } else {
                    v.nombre.trim_end_matches(".m4a").to_string()
                },
            })
        })
        .collect()
}

/// Lo que se hace con una foto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Va en la grabacion de la nota `voz`, en `ms`.
    Colocar {
        voz: String,
        ms: i64,
        sin_comprobar: bool,
    },
    /// Ya esta dentro de una (llego dos veces).
    Ya,
    /// Su hora no cae en ninguna: se queda como un mensaje normal.
    Fuera,
    /// Cae en una, pero quien la hizo no esta invitado.
    NoAdmitida,
}

/// **Donde va una foto** entre las grabaciones de su chat que grabo este
/// aparato (`yo`, su codigo; las demas no se tocan). Si cae en dos (una
/// dentro del margen de la otra), la que empezo despues.
pub fn decidir(
    foto: &FotoConHora,
    grabaciones: &[Grabacion],
    yo: &str,
    mios: &HashSet<String>,
    desfases: &Desfases,
) -> Decision {
    if grabaciones
        .iter()
        .any(|g| g.ya.contains(&foto.id) || g.ya.contains(&foto.uid))
    {
        return Decision::Ya;
    }
    let mut no_admitida = false;
    let mut mejor: Option<(&Grabacion, i64, bool)> = None;
    for g in grabaciones.iter().filter(|g| g.reloj.aparato == yo) {
        if let Some(p) = &foto.para
            && *p != g.voz
        {
            continue;
        }
        let hora = hora_corregida(foto, &g.reloj.aparato, desfases);
        let duracion = (g.duracion_ms > 0).then_some(g.duracion_ms);
        let Some(ms) = lugar(hora.utc, &g.reloj, duracion, MARGEN_MS) else {
            continue;
        };
        if !admitido(g, &foto.aparato, mios) {
            no_admitida = true;
            continue;
        }
        if mejor.is_none_or(|(b, _, _)| g.reloj.inicio > b.reloj.inicio) {
            mejor = Some((g, ms, !hora.comprobada));
        }
    }
    match mejor {
        Some((g, ms, sin_comprobar)) => Decision::Colocar {
            voz: g.voz.clone(),
            ms,
            sin_comprobar,
        },
        None if no_admitida => Decision::NoAdmitida,
        None => Decision::Fuera,
    }
}

// ------------------------------------------------------------ en disco

/// **Anade una foto a la lista de la nota** con su autor: `{ms, id,
/// aparato, uid, horaSinComprobar}`. Un id que ya estaba no se repite.
/// Devuelve si cambio.
pub fn poner_en_la_nota(
    resto: &mut Map<String, Value>,
    ms: i64,
    foto: &FotoConHora,
    sin_comprobar: bool,
) -> bool {
    let mut lista: Vec<Value> = resto
        .get(clase::CAPTURAS)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if lista.iter().any(|c| {
        let id = c.get("id").and_then(Value::as_str);
        id == Some(foto.id.as_str()) || c.get("uid").and_then(Value::as_str) == Some(&foto.uid)
    }) {
        return false;
    }
    let mut c = serde_json::json!({ "ms": ms.max(0), "id": foto.id });
    if !foto.aparato.is_empty() {
        c["aparato"] = serde_json::json!(foto.aparato);
    }
    if !foto.uid.is_empty() {
        c["uid"] = serde_json::json!(foto.uid);
    }
    if sin_comprobar {
        c[SIN_COMPROBAR] = serde_json::json!(true);
    }
    lista.push(c);
    lista.sort_by_key(|c| c.get("ms").and_then(Value::as_i64).unwrap_or(0));
    resto.insert(clase::CAPTURAS.into(), Value::Array(lista));
    true
}

/// Si la captura `id` de la nota tiene la hora sin comprobar.
pub fn sin_comprobar(voz: &Mensaje, id: &str) -> bool {
    voz.resto
        .get(clase::CAPTURAS)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|c| {
            c.get("id").and_then(Value::as_str) == Some(id)
                && c.get(SIN_COMPROBAR).and_then(Value::as_bool) == Some(true)
        })
}

/// El rotulo de la foto en la carpeta: «⏱ 12:34».
fn rotulo(ms: i64) -> String {
    format!("⏱ {}", crate::camara_del_movil::minuto(ms))
}

/// **Pone la foto en la grabacion** de la nota `voz` del chat en `dir`, en
/// `ms`: en la lista de la nota, en su carpeta y, si la foto es mia
/// (`mia`), con su enlace `enGrabacion` y su fuente. Devuelve si cambio
/// algo (nada si ya estaba: se puede llamar dos veces).
pub fn colocar(
    dir: &Path,
    g: &Grabacion,
    foto: &FotoConHora,
    ms: i64,
    sin_comprobar: bool,
    mia: bool,
) -> std::io::Result<bool> {
    let mut cambio = false;
    cuaderno::cambiar(dir, &g.voz, |n| {
        let si = poner_en_la_nota(&mut n.resto, ms, foto, sin_comprobar);
        cambio |= si;
        si
    })?;
    if mia {
        cuaderno::cambiar(dir, &foto.id, |m| {
            if m.resto.contains_key("enGrabacion") {
                return false;
            }
            m.resto.insert(
                "enGrabacion".into(),
                serde_json::json!({ "de": g.voz, "ms": ms }),
            );
            // La de otro aparato mio es «del movil»; la del que grabo (una
            // captura de su pantalla) ya dice la suya o no dice nada.
            if presencial::fuente_de(&m.resto).is_none() && foto.aparato != g.reloj.aparato {
                presencial::marcar_del_movil(&mut m.resto);
            }
            cambio = true;
            true
        })?;
    }
    if let Some(c) = &g.carpeta {
        let mut nueva = false;
        carpeta_de_paginas::cambiar_en_disco(dir, c, |c| {
            nueva = c.anadir_en_su_minuto(&foto.id, &rotulo(ms), ms);
            nueva
        })?;
        cambio |= nueva;
    }
    Ok(cambio)
}

/// **Repasa un chat**: cada foto con hora que aun no esta en ninguna
/// grabacion de las que grabo este aparato (`yo`) y cae en una, entra.
/// Devuelve cuantas entraron. Sin grabaciones con reloj, ni mira las fotos.
pub fn repasar(
    dir: &Path,
    yo: &str,
    mios: &HashSet<String>,
    desfases: &Desfases,
) -> std::io::Result<usize> {
    let c = match cuaderno::Cuaderno::leer_de(dir) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let mut grabaciones: Vec<Grabacion> = grabaciones_de(&c.mensajes)
        .into_iter()
        .filter(|g| g.reloj.aparato == yo)
        .collect();
    if grabaciones.is_empty() {
        return Ok(0);
    }
    let mut hechas = 0;
    for m in &c.mensajes {
        let Some(foto) = foto_de(m) else { continue };
        if let Decision::Colocar {
            voz,
            ms,
            sin_comprobar,
        } = decidir(&foto, &grabaciones, yo, mios, desfases)
        {
            let Some(g) = grabaciones.iter_mut().find(|g| g.voz == voz) else {
                continue;
            };
            let mia = foto.aparato.is_empty() || mios.contains(&foto.aparato);
            if colocar(dir, g, &foto, ms, sin_comprobar, mia)? {
                hechas += 1;
            }
            g.ya.insert(foto.id.clone());
            g.ya.insert(foto.uid.clone());
        }
    }
    Ok(hechas)
}

/// **«Adjuntar a la grabación…»** a mano: la foto `foto` en la grabacion de
/// la nota `voz`, en su minuto si su hora cae dentro y, si no (o no tiene
/// hora), en `ms_si_no` (el minuto que se eligio, o el final). Para los
/// casos raros: un reloj roto, una foto sin hora, una persona sin invitar.
/// Devuelve el minuto donde quedo.
pub fn a_mano(
    dir: &Path,
    foto: &str,
    voz: &str,
    ms_si_no: Option<i64>,
    mios: &HashSet<String>,
    desfases: &Desfases,
) -> std::io::Result<i64> {
    let c = cuaderno::Cuaderno::leer_de(dir)?;
    let m =
        c.mensajes.iter().find(|m| m.id == foto).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "la foto ya no está")
        })?;
    let v =
        c.mensajes.iter().find(|m| m.id == voz).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "la nota ya no está")
        })?;
    let g = grabaciones_de(&c.mensajes)
        .into_iter()
        .find(|g| g.voz == voz)
        .unwrap_or_else(|| Grabacion {
            voz: voz.to_string(),
            carpeta: c
                .mensajes
                .iter()
                .find(|x| Carpeta::de_mensaje(x).is_some_and(|k| k.voz == voz))
                .map(|x| x.id.clone()),
            duracion_ms: v.duracion_ms.max(0),
            ..Default::default()
        });
    let f = foto_de(m).unwrap_or_else(|| FotoConHora {
        id: m.id.clone(),
        uid: m.codigo_unico(),
        aparato: m.aparato.clone().unwrap_or_default(),
        ..Default::default()
    });
    let duracion = (g.duracion_ms > 0).then_some(g.duracion_ms);
    let (ms, sin) = match (f.tomada_en > 0 && g.reloj.inicio > 0).then(|| {
        let h = hora_corregida(&f, &g.reloj.aparato, desfases);
        (lugar(h.utc, &g.reloj, duracion, MARGEN_MS), h.comprobada)
    }) {
        Some((Some(ms), comprobada)) => (ms, !comprobada),
        _ => (
            ms_si_no
                .unwrap_or(g.duracion_ms)
                .clamp(0, duracion.unwrap_or(i64::MAX)),
            false,
        ),
    };
    let mia = f.aparato.is_empty() || mios.contains(&f.aparato);
    colocar(dir, &g, &f, ms, sin, mia)?;
    Ok(ms)
}

// ------------------------------------------------------------ todos los chats

/// Si el cuaderno de un chat puede tener una grabacion con reloj, sin
/// leerlo entero: lo que no lleva la palabra no la tiene.
fn puede_tener_reloj(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("guardados.jsonl")).is_ok_and(|t| t.contains("\"reloj\""))
}

/// **Repasa todos los chats** de `raiz` ([`repasar`] en cada uno con una
/// grabacion de este aparato). Devuelve los chats donde entro algo, con
/// cuantas.
pub fn repasar_todos(
    raiz: &Path,
    yo: &str,
    mios: &HashSet<String>,
    desfases: &Desfases,
) -> Vec<(String, usize)> {
    let mut hechas = Vec::new();
    for f in crate::almacen::Indice::leer(raiz).proyectos {
        let dir = crate::almacen::carpeta(raiz, &f.id);
        if !puede_tener_reloj(&dir) {
            continue;
        }
        match repasar(&dir, yo, mios, desfases) {
            Ok(0) => {}
            Ok(n) => hechas.push((f.id.clone(), n)),
            Err(_) => {}
        }
    }
    hechas
}

/// **La grabacion guardada de este aparato (`yo`) donde cae la hora `t`**
/// (ya en su reloj), en cualquier chat: el chat, la grabacion y el minuto.
/// Es lo que hace la camara en directo con una foto que no sabe de que
/// grabacion es (se hizo sin conexion) o de una que ya no recuerda.
pub fn buscar_por_hora(raiz: &Path, t: i64, yo: &str) -> Option<(String, Grabacion, i64)> {
    let mut mejor: Option<(String, Grabacion, i64)> = None;
    for f in crate::almacen::Indice::leer(raiz).proyectos {
        let dir = crate::almacen::carpeta(raiz, &f.id);
        if !puede_tener_reloj(&dir) {
            continue;
        }
        let Ok(c) = cuaderno::Cuaderno::leer_de(&dir) else {
            continue;
        };
        for g in grabaciones_de(&c.mensajes) {
            if g.reloj.aparato != yo {
                continue;
            }
            let duracion = (g.duracion_ms > 0).then_some(g.duracion_ms);
            let Some(ms) = lugar(t, &g.reloj, duracion, MARGEN_MS) else {
                continue;
            };
            if mejor
                .as_ref()
                .is_none_or(|(_, b, _)| g.reloj.inicio > b.reloj.inicio)
            {
                mejor = Some((f.id.clone(), g, ms));
            }
        }
    }
    mejor
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const S: i64 = 1000;
    const PARED: i64 = 1_761_640_000_000;

    fn reloj() -> RelojDeGrabacion {
        // Graba el PC «PC01» 100 s, con una pausa de 30 s en el 40.
        RelojDeGrabacion {
            aparato: "PC01".into(),
            inicio: PARED,
            fin: PARED + 130 * S,
            pausas: vec![[PARED + 40 * S, PARED + 70 * S]],
            visto: 0,
        }
    }

    #[test]
    fn dentro_de_la_ventana_el_minuto_sin_lo_pausado() {
        let r = reloj();
        assert_eq!(ms_en_la_grabacion(PARED, &r), Some(0));
        assert_eq!(ms_en_la_grabacion(PARED + 10 * S, &r), Some(10 * S));
        // Despues de la pausa: 30 s menos.
        assert_eq!(ms_en_la_grabacion(PARED + 90 * S, &r), Some(60 * S));
        assert_eq!(ms_en_la_grabacion(PARED + 130 * S, &r), Some(100 * S));
        assert_eq!(r.duracion(PARED + 999 * S), 100 * S);
    }

    #[test]
    fn en_una_pausa_va_al_minuto_en_que_se_paro() {
        let r = reloj();
        assert_eq!(ms_en_la_grabacion(PARED + 40 * S, &r), Some(40 * S));
        assert_eq!(ms_en_la_grabacion(PARED + 55 * S, &r), Some(40 * S));
        assert_eq!(ms_en_la_grabacion(PARED + 70 * S, &r), Some(40 * S));
        // Una pausa sin acabar (se acabo en pausa): hasta el final.
        let mut r = reloj();
        r.pausas.push([PARED + 120 * S, 0]);
        assert_eq!(ms_en_la_grabacion(PARED + 125 * S, &r), Some(90 * S));
    }

    #[test]
    fn caso_negativo_fuera_de_la_ventana_nada() {
        let r = reloj();
        assert_eq!(ms_en_la_grabacion(PARED - 1, &r), None);
        assert_eq!(ms_en_la_grabacion(PARED + 130 * S + 1, &r), None);
        assert_eq!(
            ms_en_la_grabacion(PARED, &RelojDeGrabacion::default()),
            None
        );
        // Con el margen, lo de justo antes y justo despues entra en los
        // bordes; lo de mas alla, no.
        assert_eq!(lugar(PARED - 20 * S, &r, None, MARGEN_MS), Some(0));
        assert_eq!(lugar(PARED + 150 * S, &r, None, MARGEN_MS), Some(100 * S));
        assert_eq!(lugar(PARED - 31 * S, &r, None, MARGEN_MS), None);
        assert_eq!(lugar(PARED + 161 * S, &r, None, MARGEN_MS), None);
        // La nota duro menos que el reloj: manda la nota.
        assert_eq!(
            lugar(PARED + 129 * S, &r, Some(98 * S), MARGEN_MS),
            Some(98 * S)
        );
    }

    #[test]
    fn una_grabacion_cortada_acaba_en_lo_ultimo_visto_y_una_viva_no_acaba() {
        let mut r = reloj();
        r.fin = 0;
        r.visto = PARED + 100 * S;
        assert_eq!(ms_en_la_grabacion(PARED + 100 * S, &r), Some(70 * S));
        assert_eq!(ms_en_la_grabacion(PARED + 101 * S, &r), None);
        r.visto = 0;
        assert_eq!(ms_en_la_grabacion(PARED + 10_000 * S, &r), Some(9_970 * S));
    }

    #[test]
    fn el_reloj_va_y_vuelve_en_la_nota() {
        let mut resto = Map::new();
        assert_eq!(RelojDeGrabacion::de(&resto), None);
        reloj().poner(&mut resto);
        assert_eq!(
            serde_json::to_string(&reloj()).unwrap(),
            format!(
                r#"{{"aparato":"PC01","inicio":{PARED},"fin":{},"pausas":[[{},{}]]}}"#,
                PARED + 130 * S,
                PARED + 40 * S,
                PARED + 70 * S
            )
        );
        assert_eq!(RelojDeGrabacion::de(&resto), Some(reloj()));
        // Uno roto o sin inicio no es un reloj.
        resto.insert(RELOJ.into(), serde_json::json!({ "aparato": "X" }));
        assert_eq!(RelojDeGrabacion::de(&resto), None);
    }

    #[test]
    fn el_desfase_es_la_medida_mas_cercana_a_la_hora_de_la_foto() {
        let mut d = Desfases::default();
        d.medir("TEL1", PARED, 5 * S);
        d.medir("TEL1", PARED + 86_400 * S, 9 * S);
        assert_eq!(d.cercano("TEL1", PARED + 3600 * S), Some(5 * S));
        assert_eq!(d.cercano("TEL1", PARED + 80_000 * S), Some(9 * S));
        assert_eq!(d.cercano("OTRO", PARED), None);
        // Dos del mismo minuto: la ultima.
        d.medir("TEL1", PARED + 10 * S, 6 * S);
        assert_eq!(d.aparatos["TEL1"].len(), 2);
        assert_eq!(d.cercano("TEL1", PARED), Some(6 * S));
        // Y no crece sin fin.
        for i in 0..100 {
            d.medir("TEL1", PARED + i * 120 * S, i);
        }
        assert_eq!(d.aparatos["TEL1"].len(), MEDIDAS_MAXIMAS);
        d.medir("", PARED, 1);
        assert!(!d.aparatos.contains_key(""));
    }

    #[test]
    fn la_hora_del_exif_con_su_zona() {
        // 28-oct-2026 10:05:03 en Madrid (+01:00) = 09:05:03 UTC.
        let ms = ms_de_exif("2026:10:28 10:05:03", "+01:00").unwrap();
        assert_eq!(ms, 1_793_178_303_000);
        assert_eq!(ms_de_exif("2026:10:28 09:05:03", "Z"), Some(ms));
        assert_eq!(ms_de_exif("2026-10-28T04:05:03", "-05:00"), Some(ms));
        assert_eq!(ms_de_exif("1970:01:01 00:00:00", "+00:00"), Some(0));
        // Casos negativos: sin zona, o una fecha rota.
        assert_eq!(ms_de_exif("2026:10:28 10:05:03", ""), None);
        assert_eq!(ms_de_exif("2026:13:28 10:05:03", "+01:00"), None);
        assert_eq!(ms_de_exif("ayer", "+01:00"), None);
    }

    fn foto(id: &str, aparato: &str, tomada_en: i64) -> FotoConHora {
        FotoConHora {
            id: id.into(),
            uid: format!("u-{id}"),
            aparato: aparato.into(),
            tomada_en,
            para: None,
        }
    }

    fn grabacion() -> Grabacion {
        Grabacion {
            voz: "voz1".into(),
            carpeta: Some("car1".into()),
            reloj: reloj(),
            duracion_ms: 100 * S,
            ..Default::default()
        }
    }

    fn mios() -> HashSet<String> {
        ["PC01".to_string(), "TEL1".to_string()].into()
    }

    #[test]
    fn la_foto_de_mi_movil_con_su_desfase_va_a_su_minuto() {
        let mut d = Desfases::default();
        // El movil va 8 s adelantado.
        d.medir("TEL1", PARED - 3600 * S, 8 * S);
        let g = [grabacion()];
        // Su hora +98 s es la +90 del PC: el 60 (sin la pausa).
        let f = foto("f1", "TEL1", PARED + 98 * S);
        assert_eq!(
            decidir(&f, &g, "PC01", &mios(), &d),
            Decision::Colocar {
                voz: "voz1".into(),
                ms: 60 * S,
                sin_comprobar: false
            }
        );
        // Una del propio PC, sin desfase.
        let f = foto("f2", "PC01", PARED + 10 * S);
        assert!(matches!(
            decidir(&f, &g, "PC01", &mios(), &d),
            Decision::Colocar { ms, .. } if ms == 10 * S
        ));
    }

    #[test]
    fn caso_negativo_fuera_ya_puesta_de_otro_dueno_o_sin_invitar() {
        let d = Desfases::default();
        let mut g = grabacion();
        // Fuera de la ventana: se queda como un mensaje normal.
        let f = foto("f1", "TEL1", PARED + 3600 * S);
        assert_eq!(
            decidir(&f, &[g.clone()], "PC01", &mios(), &d),
            Decision::Fuera
        );
        // Llego dos veces (por su id o por su uid): una sola.
        let f = foto("f2", "TEL1", PARED + 5 * S);
        g.ya.insert("u-f2".into());
        assert_eq!(decidir(&f, &[g.clone()], "PC01", &mios(), &d), Decision::Ya);
        // La grabacion es de otro aparato: aqui no se coloca.
        let f = foto("f3", "TEL1", PARED + 5 * S);
        assert_eq!(
            decidir(&f, &[g.clone()], "TEL1", &mios(), &d),
            Decision::Fuera
        );
        // De otra persona sin invitar: no.
        let f = foto("f4", "ANA1", PARED + 5 * S);
        assert_eq!(
            decidir(&f, &[g.clone()], "PC01", &mios(), &d),
            Decision::NoAdmitida
        );
        // Para otra grabacion concreta: no va en esta.
        let mut f = foto("f5", "TEL1", PARED + 5 * S);
        f.para = Some("otra".into());
        assert_eq!(decidir(&f, &[g], "PC01", &mios(), &d), Decision::Fuera);
    }

    #[test]
    fn el_colaborador_sin_medida_entra_con_la_hora_sin_comprobar() {
        let mut g = grabacion();
        let mut resto = Map::new();
        assert!(invitar(&mut resto, "ANA1", true));
        assert!(!invitar(&mut resto, "ANA1", true), "ya estaba");
        g.colaboradores = colaboradores_de(&resto);
        let f = foto("f1", "ANA1", PARED + 20 * S);
        assert_eq!(
            decidir(&f, &[g.clone()], "PC01", &mios(), &Desfases::default()),
            Decision::Colocar {
                voz: "voz1".into(),
                ms: 20 * S,
                sin_comprobar: true
            }
        );
        // Con su medida (su aparato hablo con este alguna vez), comprobada.
        let mut d = Desfases::default();
        d.medir("ANA1", PARED, -4 * S);
        assert_eq!(
            decidir(
                &foto("f1", "ANA1", PARED + 16 * S),
                &[g.clone()],
                "PC01",
                &mios(),
                &d
            ),
            Decision::Colocar {
                voz: "voz1".into(),
                ms: 20 * S,
                sin_comprobar: false
            }
        );
        // Todos los del chat.
        assert!(invitar(&mut resto, "ANA1", false));
        assert!(!resto.contains_key(COLABORADORES));
        invitar(&mut resto, TODOS, true);
        g.colaboradores = colaboradores_de(&resto);
        assert!(admitido(&g, "EVA9", &mios()));
    }

    #[test]
    fn de_dos_grabaciones_que_se_tocan_la_que_empezo_despues() {
        let primera = grabacion();
        let mut segunda = grabacion();
        segunda.voz = "voz2".into();
        segunda.reloj.inicio = PARED + 140 * S;
        segunda.reloj.fin = PARED + 300 * S;
        segunda.reloj.pausas.clear();
        segunda.duracion_ms = 160 * S;
        // En el margen de despues de la primera y dentro de la segunda.
        let f = foto("f1", "PC01", PARED + 150 * S);
        assert!(matches!(
            decidir(&f, &[primera, segunda], "PC01", &mios(), &Desfases::default()),
            Decision::Colocar { voz, ms, .. } if voz == "voz2" && ms == 10 * S
        ));
    }

    fn mensaje_foto(id: &str, aparato: &str, tomada_en: i64) -> Mensaje {
        let mut m = Mensaje::adjunto(
            Clase::Imagen,
            &format!("{id}.jpg"),
            &format!("{id}.jpg"),
            4,
            &cuaderno::Sello {
                cuando: tomada_en,
                numero: 1,
                aparato: aparato.into(),
                proyecto: "p1".into(),
            },
        );
        m.id = id.into();
        m.uid = Some(format!("u-{id}"));
        if tomada_en > 0 {
            m.resto
                .insert(TOMADA_EN.into(), serde_json::json!(tomada_en));
        }
        m
    }

    fn chat_con_grabacion(nombre: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-por-su-hora-{nombre}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sello = cuaderno::Sello {
            cuando: PARED,
            numero: 1,
            aparato: "PC01".into(),
            proyecto: "p1".into(),
        };
        let mut voz = Mensaje::adjunto(Clase::Voz, "Clase.m4a", "Clase.m4a", 9, &sello);
        voz.id = "voz1".into();
        voz.duracion_ms = 100 * S;
        reloj().poner(&mut voz.resto);
        cuaderno::anadir(&dir, &voz).unwrap();
        let mut carpeta = Carpeta::grabacion("voz1", 100 * S);
        carpeta.anadir_al_minuto("voz1", "", 0);
        let mut c = carpeta_de_paginas::mensaje_de_grabacion("Clase", &carpeta, &sello);
        c.id = "car1".into();
        cuaderno::anadir(&dir, &c).unwrap();
        dir
    }

    #[test]
    fn el_repaso_pone_cada_foto_una_vez_en_la_nota_y_en_la_carpeta() {
        let dir = chat_con_grabacion("repaso");
        let mut d = Desfases::default();
        d.medir("TEL1", PARED, 8 * S);
        // Hecha sin conexion en el movil; llega tarde por la sincronizacion.
        cuaderno::anadir(&dir, &mensaje_foto("f1", "TEL1", PARED + 98 * S)).unwrap();
        // De una colaboradora invitada, sin medida de su reloj.
        cuaderno::anadir(&dir, &mensaje_foto("f2", "ANA1", PARED + 20 * S)).unwrap();
        // Fuera de la ventana y sin hora: se quedan como estan.
        cuaderno::anadir(&dir, &mensaje_foto("f3", "TEL1", PARED + 7200 * S)).unwrap();
        cuaderno::anadir(&dir, &mensaje_foto("f4", "TEL1", 0)).unwrap();
        cuaderno::cambiar(&dir, "voz1", |v| invitar(&mut v.resto, "ANA1", true)).unwrap();

        assert_eq!(repasar(&dir, "PC01", &mios(), &d).unwrap(), 2);
        // Otra vez (o la misma foto llegada otra vez): nada.
        assert_eq!(repasar(&dir, "PC01", &mios(), &d).unwrap(), 0);
        // Desde otro aparato mio que no grabo: tampoco.
        assert_eq!(repasar(&dir, "TEL1", &mios(), &d).unwrap(), 0);

        let c = cuaderno::Cuaderno::leer_de(&dir).unwrap();
        let voz = c.mensajes.iter().find(|m| m.id == "voz1").unwrap();
        assert_eq!(
            clase::capturas_de(&voz.resto),
            vec![(20 * S, "f2".to_string()), (60 * S, "f1".to_string())]
        );
        assert!(sin_comprobar(voz, "f2"));
        assert!(!sin_comprobar(voz, "f1"));
        let car = c.mensajes.iter().find_map(Carpeta::de_mensaje).unwrap();
        let ids: Vec<&str> = car.paginas.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["voz1", "f2", "f1"]);
        // La mia lleva su enlace; la de la otra persona no se toco.
        let f1 = c.mensajes.iter().find(|m| m.id == "f1").unwrap();
        assert_eq!(f1.resto["enGrabacion"]["ms"], 60 * S);
        assert!(presencial::es_del_movil(&f1.resto));
        let f2 = c.mensajes.iter().find(|m| m.id == "f2").unwrap();
        assert!(!f2.resto.contains_key("enGrabacion"));
        let f3 = c.mensajes.iter().find(|m| m.id == "f3").unwrap();
        assert!(!f3.resto.contains_key("enGrabacion"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_que_entro_por_la_camara_en_directo_no_se_repite_al_sincronizar() {
        let dir = chat_con_grabacion("directo");
        // La camara en directo la copio con su uid…
        let mut copia = mensaje_foto("copia", "PC01", 0);
        copia
            .resto
            .insert(FOTO_UID.into(), serde_json::json!("u-f1"));
        cuaderno::anadir(&dir, &copia).unwrap();
        cuaderno::cambiar(&dir, "voz1", |v| {
            clase::poner_captura(&mut v.resto, 5 * S, "copia");
            true
        })
        .unwrap();
        // …y luego llega la original por la sincronizacion.
        cuaderno::anadir(&dir, &mensaje_foto("f1", "TEL1", PARED + 5 * S)).unwrap();
        assert_eq!(
            repasar(&dir, "PC01", &mios(), &Desfases::default()).unwrap(),
            0
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_mano_entra_aunque_no_tenga_hora_o_caiga_fuera() {
        let dir = chat_con_grabacion("a-mano");
        cuaderno::anadir(&dir, &mensaje_foto("f1", "EVA9", 0)).unwrap();
        cuaderno::anadir(&dir, &mensaje_foto("f2", "PC01", PARED + 10 * S)).unwrap();
        let d = Desfases::default();
        assert_eq!(
            a_mano(&dir, "f1", "voz1", Some(33 * S), &mios(), &d).unwrap(),
            33 * S
        );
        // Con hora dentro, la suya y no la elegida.
        assert_eq!(
            a_mano(&dir, "f2", "voz1", Some(80 * S), &mios(), &d).unwrap(),
            10 * S
        );
        // Sin minuto elegido, al final.
        cuaderno::anadir(&dir, &mensaje_foto("f3", "EVA9", 0)).unwrap();
        assert_eq!(
            a_mano(&dir, "f3", "voz1", None, &mios(), &d).unwrap(),
            100 * S
        );
        let c = cuaderno::Cuaderno::leer_de(&dir).unwrap();
        let voz = c.mensajes.iter().find(|m| m.id == "voz1").unwrap();
        assert_eq!(clase::capturas_de(&voz.resto).len(), 3);
        // Caso negativo: una foto que ya no esta.
        assert!(a_mano(&dir, "nada", "voz1", None, &mios(), &d).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_foto_sin_tomada_en_usa_la_hora_del_exif() {
        let mut m = mensaje_foto("f1", "TEL1", 0);
        assert_eq!(foto_de(&m), None);
        m.resto.insert(
            TOMADA_EN_EXIF.into(),
            serde_json::json!("2026:10:28 10:05:03"),
        );
        m.resto
            .insert(DESFASE_EXIF.into(), serde_json::json!("+01:00"));
        assert_eq!(foto_de(&m).unwrap().tomada_en, 1_793_178_303_000);
        // Caso negativo: lo que no es una IMAGEN no es una foto.
        m.clase = Some(Clase::Nota);
        assert_eq!(foto_de(&m), None);
    }
}
