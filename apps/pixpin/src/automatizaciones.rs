//! **Automatizaciones del asistente** (pedido del usuario, 13-oct-2026):
//! «cada vez que capture, ponlo en el chat de la obra y hazle OCR», «cada
//! dia a las 8, resumen de tareas pendientes».
//!
//! Regla de la casa: **un solo componente con variables**. Una automatizacion
//! es un disparador ([`Disparador`]: al capturar, a una hora, al llegar algo
//! por la sincro, a mano) y una lista de pasos ([`Paso`]): una accion de la
//! tabla `pixpin_ia::acciones` (el mismo pedido que usaria el asistente) o
//! un pedido al asistente (un turno entero, con el motor de Ajustes). Los
//! pasos se encadenan con variables (`{captura}`, `{resultado}`, `{fecha}`,
//! `{hora}`, `{sincro}`, `{nombre}`): lo que contesto un paso es el
//! `{resultado}` del siguiente.
//!
//! - **Donde se guardan**: JSON en camelCase (como las lecciones) en la
//!   carpeta del chat «Asistente»,
//!   `proyectos/<asistente>/android/guardados/asistente/automatizaciones.json`,
//!   que es `pixpin:files/guardados/asistente/automatizaciones.json` para el
//!   movil (donde lo guardaria Android: `files/guardados/asistente/`). Un
//!   mensaje ARCHIVO fijo del chat «Asistente» ([`ID_MENSAJE`]) lo senala:
//!   asi viaja **solo con los aparatos propios** por la sincronizacion del
//!   grupo, como viaja un `.leccion`, sin tocar el protocolo. El chat
//!   «Asistente» no se comparte con otras personas.
//! - **Donde corren**: cada una lleva el `aparato` que la creo; las de otro
//!   aparato se listan pero no corren aqui (si no, «cada dia a las 8» saldria
//!   dos veces). Sin `aparato`, en todos.
//! - **Lo que ya corrio** (la ultima vez de cada una) es de este equipo y no
//!   viaja: `<raiz>/asistente/automatizaciones-estado.json`.
//! - **Nunca algo destructivo sin preguntar**: cada accion pasa por
//!   `asistente::acciones::hacer`, que pregunta antes de lo que borra o sale
//!   del PC.
//! - **Sin bucles**: una que salta al capturar no puede capturar, ni una que
//!   salta con la sincro sincronizar, ni ninguna crear automatizaciones (se
//!   rechaza al crearla y se bloquea al correr, tambien dentro de un paso del
//!   asistente); y aun asi, cada una tiene un tope de veces por hora
//!   ([`motor::Guardia`]).
//!
//! **Las recetas del asistente son automatizaciones** `a_mano` con un solo
//! paso «pedido al asistente» y, en `resto`, `boton: true`, `icono` y
//! `orden`: salen como botones encima de la caja del chat «Asistente»
//! (`asistente::recetas`).

pub mod cola;
pub mod encargo;
pub mod motor;
#[cfg(test)]
mod pruebas;

use std::path::{Path, PathBuf};

use pixpin_ia::acciones::{self, AccionApp};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// La version del fichero.
pub const VERSION: u32 = 1;
/// El id del mensaje del chat «Asistente» que senala el fichero.
pub const ID_MENSAJE: &str = "auto-automatizaciones";
/// La ruta con que viaja (y la del mensaje).
pub const RUTA_PORTATIL: &str = "pixpin:files/guardados/asistente/automatizaciones.json";
/// Dentro de la carpeta del chat.
pub const RUTA_EN_EL_CHAT: &str = "android/guardados/asistente/automatizaciones.json";
/// Cuantos pasos como mucho.
pub const MAX_PASOS: usize = 10;

/// **Cuando salta.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Disparador {
    /// Al guardarse una captura nueva en la carpeta de capturas
    /// (`{captura}` es su ruta).
    AlCapturar,
    /// A una hora local (`"08:00"`), los `dias` de la semana (1 lunes … 7
    /// domingo; vacio: todos).
    ALaHora {
        hora: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        dias: Vec<u8>,
    },
    /// Al acabar una vuelta de la sincronizacion que trajo algo (`{sincro}`
    /// es lo que dijo la vuelta).
    AlSincronizar,
    /// Solo cuando se pide (Ajustes, `p ia`, el pedido `automatizacion`).
    AMano,
}

/// El tipo del disparador, sin sus datos (para la guardia de bucles).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tipo {
    AlCapturar,
    ALaHora,
    AlSincronizar,
    AMano,
}

impl Disparador {
    pub fn tipo(&self) -> Tipo {
        match self {
            Disparador::AlCapturar => Tipo::AlCapturar,
            Disparador::ALaHora { .. } => Tipo::ALaHora,
            Disparador::AlSincronizar => Tipo::AlSincronizar,
            Disparador::AMano => Tipo::AMano,
        }
    }

    /// En palabras, para la lista.
    pub fn en_palabras(&self) -> String {
        match self {
            Disparador::AlCapturar => "Al capturar".into(),
            Disparador::ALaHora { hora, dias } if dias.is_empty() => {
                format!("Cada día a las {hora}")
            }
            Disparador::ALaHora { hora, dias } => {
                const NOMBRES: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];
                let d: Vec<&str> = dias
                    .iter()
                    .filter_map(|d| NOMBRES.get(usize::from(*d).wrapping_sub(1)).copied())
                    .collect();
                format!("{} a las {hora}", d.join(", "))
            }
            Disparador::AlSincronizar => "Al llegar algo por la sincronización".into(),
            Disparador::AMano => "A mano".into(),
        }
    }
}

/// **Un paso.**
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Paso {
    /// Una accion de la tabla del asistente (`pixpin_ia::acciones`), por el
    /// nombre de su herramienta, con sus argumentos. `proyecto` (nombre o id
    /// del chat) e `imagenes` van al pedido si la accion los entiende.
    Accion {
        herramienta: String,
        #[serde(default, skip_serializing_if = "Map::is_empty")]
        args: Map<String, Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        proyecto: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        imagenes: Vec<String>,
    },
    /// Un pedido al asistente, como escrito en su chat. `imagenes` van
    /// adjuntas (al capturar, sin decir nada, la captura).
    Asistente {
        pedido: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        proyecto: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        imagenes: Vec<String>,
    },
}

/// **Una automatizacion.**
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Automatizacion {
    pub id: String,
    pub nombre: String,
    #[serde(default = "si")]
    pub activa: bool,
    pub disparador: Disparador,
    pub pasos: Vec<Paso>,
    /// El codigo del aparato donde corre (el que la creo). `None`: en todos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aparato: Option<String>,
    #[serde(default)]
    pub creada: i64,
    #[serde(default)]
    pub tocada: i64,
    /// Lo que no se entiende (lo que anada el movil), tal cual.
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

fn si() -> bool {
    true
}

impl Automatizacion {
    /// Si corre en este aparato.
    pub fn corre_aqui(&self, aparato: &str) -> bool {
        self.aparato
            .as_deref()
            .is_none_or(|a| a.is_empty() || a == aparato)
    }

    /// Los pasos en una linea, para la lista.
    pub fn pasos_en_palabras(&self) -> String {
        self.pasos
            .iter()
            .map(|p| match p {
                Paso::Accion {
                    herramienta,
                    proyecto,
                    ..
                } => match proyecto {
                    Some(pr) => format!("{herramienta} en «{pr}»"),
                    None => herramienta.clone(),
                },
                Paso::Asistente { pedido, .. } => format!("«{}»", recortar(pedido, 40)),
            })
            .collect::<Vec<_>>()
            .join(" → ")
    }
}

/// Lo que hay en el fichero, en su orden: las que se entienden y las que no
/// (de una version mas nueva, o rotas), que se guardan tal cual.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lista {
    pub entradas: Vec<Entrada>,
    /// Lo de mas del fichero, tal cual.
    pub resto: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entrada {
    Buena(Automatizacion),
    Rota { valor: Value, motivo: String },
}

impl Lista {
    /// Lee el JSON. Un fichero que no es JSON es un error (no se pisa); una
    /// automatizacion que no se entiende se queda como [`Entrada::Rota`].
    pub fn de_json(texto: &str) -> Result<Lista, String> {
        let v: Value = serde_json::from_str(texto).map_err(|e| format!("no es JSON: {e}"))?;
        let Value::Object(mut o) = v else {
            return Err("no es un objeto JSON".into());
        };
        let items = match o.remove("automatizaciones") {
            Some(Value::Array(a)) => a,
            None => Vec::new(),
            Some(_) => return Err("«automatizaciones» no es una lista".into()),
        };
        o.remove("version");
        let entradas = items
            .into_iter()
            .map(
                |valor| match serde_json::from_value::<Automatizacion>(valor.clone()) {
                    Ok(a) => match validar(&a) {
                        Ok(()) => Entrada::Buena(a),
                        Err(motivo) => Entrada::Rota { valor, motivo },
                    },
                    Err(e) => Entrada::Rota {
                        valor,
                        motivo: motivo_legible(&e.to_string()),
                    },
                },
            )
            .collect();
        Ok(Lista { entradas, resto: o })
    }

    pub fn a_json(&self) -> String {
        let mut o = self.resto.clone();
        o.insert("version".into(), json!(VERSION));
        let items: Vec<Value> = self
            .entradas
            .iter()
            .map(|e| match e {
                Entrada::Buena(a) => serde_json::to_value(a).unwrap_or(Value::Null),
                Entrada::Rota { valor, .. } => valor.clone(),
            })
            .collect();
        o.insert("automatizaciones".into(), Value::Array(items));
        serde_json::to_string_pretty(&Value::Object(o)).unwrap_or_default()
    }

    pub fn buenas(&self) -> impl Iterator<Item = &Automatizacion> {
        self.entradas.iter().filter_map(|e| match e {
            Entrada::Buena(a) => Some(a),
            Entrada::Rota { .. } => None,
        })
    }

    pub fn buscar(&self, id: &str) -> Option<&Automatizacion> {
        self.buenas().find(|a| a.id == id)
    }

    fn buscar_mut(&mut self, id: &str) -> Option<&mut Automatizacion> {
        self.entradas.iter_mut().find_map(|e| match e {
            Entrada::Buena(a) if a.id == id => Some(a),
            _ => None,
        })
    }

    /// Enciende o apaga. `false` si no esta.
    pub fn activar(&mut self, id: &str, activa: bool, ahora: i64) -> bool {
        match self.buscar_mut(id) {
            Some(a) => {
                a.activa = activa;
                a.tocada = ahora;
                true
            }
            None => false,
        }
    }

    /// Quita una (buena o rota, por su id). `false` si no estaba.
    pub fn borrar(&mut self, id: &str) -> bool {
        let antes = self.entradas.len();
        self.entradas.retain(|e| match e {
            Entrada::Buena(a) => a.id != id,
            Entrada::Rota { valor, .. } => valor.get("id").and_then(Value::as_str) != Some(id),
        });
        self.entradas.len() != antes
    }

    /// Anade una nueva, ya validada.
    pub fn anadir(&mut self, a: Automatizacion) -> Result<(), String> {
        validar(&a)?;
        if self.buscar(&a.id).is_some() {
            return Err(format!("ya hay una automatización «{}»", a.id));
        }
        self.entradas.push(Entrada::Buena(a));
        Ok(())
    }
}

/// Lo que dice serde, en palabras: «unknown variant `x`» es algo de una
/// version mas nueva (el movil, mas adelante).
fn motivo_legible(e: &str) -> String {
    if let Some(resto) = e.split("unknown variant `").nth(1)
        && let Some(x) = resto.split('`').next()
    {
        return format!("«{x}» es de una versión más nueva");
    }
    if let Some(resto) = e.split("missing field `").nth(1)
        && let Some(x) = resto.split('`').next()
    {
        return format!("le falta «{x}»");
    }
    e.to_string()
}

// ---------------------------------------------------------------------------
// Validar

/// Las acciones que harian saltar a cada disparador (un bucle).
fn salta_con(accion: &str) -> Option<Tipo> {
    match accion {
        "capturar" => Some(Tipo::AlCapturar),
        "sincronizar" => Some(Tipo::AlSincronizar),
        _ => None,
    }
}

/// **Si un pedido no se puede hacer desde una automatizacion de este
/// disparador**: el motivo. Vale para los pasos y para lo que pida el
/// asistente dentro de un paso.
pub fn bloqueado(tipo: Option<Tipo>, pedido: &Value) -> Option<String> {
    let accion = pedido
        .get("accion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if tipo.is_some() && accion == "automatizacion" {
        return Some("una automatización no puede crear ni tocar automatizaciones".into());
    }
    match (tipo, salta_con(accion)) {
        (Some(t), Some(s)) if t == s => Some(format!(
            "«{accion}» haría saltar otra vez esta automatización: sería un bucle"
        )),
        _ => None,
    }
}

/// Valores de ejemplo de las variables, para validar los pasos al crearlos.
fn vars_de_ejemplo() -> Vars {
    Vars {
        captura: Some(PathBuf::from(r"C:\capturas\captura.png")),
        resultado: "texto".into(),
        fecha: "2026-10-13".into(),
        hora: "08:00".into(),
        sincro: "Al día".into(),
        nombre: "automatización".into(),
    }
}

/// **Comprueba** una automatizacion: nombre, disparador, pasos conocidos
/// con sus argumentos, y que no se dispare a si misma.
pub fn validar(a: &Automatizacion) -> Result<(), String> {
    if a.id.trim().is_empty() {
        return Err("le falta el id".into());
    }
    if a.nombre.trim().is_empty() {
        return Err("le falta el nombre".into());
    }
    if let Disparador::ALaHora { hora, dias } = &a.disparador {
        minuto_del_dia(hora).ok_or_else(|| format!("la hora «{hora}» no se entiende (HH:MM)"))?;
        if dias.iter().any(|d| !(1..=7).contains(d)) {
            return Err("los días van del 1 (lunes) al 7 (domingo)".into());
        }
    }
    if a.pasos.is_empty() {
        return Err("no tiene pasos".into());
    }
    if a.pasos.len() > MAX_PASOS {
        return Err(format!(
            "tiene {} pasos; como mucho {MAX_PASOS}",
            a.pasos.len()
        ));
    }
    let vars = vars_de_ejemplo();
    for (i, p) in a.pasos.iter().enumerate() {
        let n = i + 1;
        match p {
            Paso::Accion { .. } => {
                let hecha = armar_accion(p, &vars, &|_| Some("pr-1".into()))
                    .map_err(|e| format!("paso {n}: {e}"))?;
                if let Some(m) = bloqueado(Some(a.disparador.tipo()), &hecha.pedido) {
                    return Err(format!("paso {n}: {m}"));
                }
            }
            Paso::Asistente { pedido, .. } => {
                if pedido.trim().is_empty() {
                    return Err(format!("paso {n}: el pedido al asistente está vacío"));
                }
            }
        }
    }
    Ok(())
}

/// `"8:05"` → 485. `None` si no es una hora.
pub fn minuto_del_dia(hora: &str) -> Option<u32> {
    let (h, m) = hora.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

// ---------------------------------------------------------------------------
// Las variables

/// Lo que se sabe al correr una automatizacion.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Vars {
    pub captura: Option<PathBuf>,
    /// Lo que contesto el paso anterior.
    pub resultado: String,
    pub fecha: String,
    pub hora: String,
    pub sincro: String,
    pub nombre: String,
}

impl Vars {
    /// Fecha y hora de ahora (hora local en ms).
    pub fn con_hora(mut self, local_ms: i64) -> Vars {
        let (a, m, d) = fecha_civil(local_ms.div_euclid(86_400_000));
        let min = local_ms.rem_euclid(86_400_000) / 60_000;
        self.fecha = format!("{a:04}-{m:02}-{d:02}");
        self.hora = format!("{:02}:{:02}", min / 60, min % 60);
        self
    }

    /// Cambia las `{variables}` de un texto. Una que no se conoce se deja.
    pub fn sustituir(&self, texto: &str) -> String {
        let captura = self
            .captura
            .as_ref()
            .map(|r| r.display().to_string())
            .unwrap_or_default();
        texto
            .replace("{captura}", &captura)
            .replace("{resultado}", &self.resultado)
            .replace("{fecha}", &self.fecha)
            .replace("{hora}", &self.hora)
            .replace("{sincro}", &self.sincro)
            .replace("{nombre}", &self.nombre)
    }

    fn sustituir_valor(&self, v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(self.sustituir(s)),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.sustituir_valor(x)).collect()),
            Value::Object(o) => Value::Object(
                o.iter()
                    .map(|(k, x)| (k.clone(), self.sustituir_valor(x)))
                    .collect(),
            ),
            otro => otro.clone(),
        }
    }

    /// Las rutas de `imagenes` ya sustituidas (las vacias fuera).
    pub fn rutas(&self, imagenes: &[String]) -> Vec<PathBuf> {
        imagenes
            .iter()
            .map(|i| self.sustituir(i))
            .filter(|i| !i.trim().is_empty())
            .map(PathBuf::from)
            .collect()
    }
}

/// Dias desde 1970 a (año, mes, dia) (Howard Hinnant, `civil_from_days`).
pub fn fecha_civil(dias: i64) -> (i64, i64, i64) {
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// El dia de la semana (1 lunes … 7 domingo) de una hora local en ms.
pub fn dia_de_la_semana(local_ms: i64) -> u8 {
    // El 1-ene-1970 fue jueves (4).
    ((local_ms.div_euclid(86_400_000) + 3).rem_euclid(7) + 1) as u8
}

// ---------------------------------------------------------------------------
// Armar un paso

/// Las acciones cuyo pedido entiende `proyecto` (`docs/protocolo-pedidos.md`).
const CON_PROYECTO: [&str; 9] = [
    "chat",
    "nota_nueva",
    "lienzo_nuevo",
    "grabar",
    "lista_nueva",
    "anadir_tarea",
    "soltar",
    "leccion_nueva",
    "lecciones",
];
/// Y las que entienden `imagenes`.
const CON_IMAGENES: [&str; 5] = [
    "chat",
    "nota_nueva",
    "anadir_tarea",
    "leccion_nueva",
    "enviar_al_movil",
];

/// Pone `proyecto` en un pedido que lo entienda y no lo traiga (ni una lista
/// concreta: `anadir_tarea` con `codigo` ya sabe donde va).
pub fn con_proyecto(pedido: &mut Value, proyecto: &str) {
    let accion = pedido
        .get("accion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !CON_PROYECTO.contains(&accion) {
        return;
    }
    if pedido.get("proyecto").is_some_and(|p| !p.is_null())
        || pedido.get("codigo").is_some_and(|c| !c.is_null())
    {
        return;
    }
    pedido["proyecto"] = json!(proyecto);
}

/// **El pedido de un paso de accion**, con las variables puestas y
/// validado por la tabla. `resolver` pasa el nombre (o el id) de un chat a su
/// id.
pub fn armar_accion(
    paso: &Paso,
    vars: &Vars,
    resolver: &dyn Fn(&str) -> Option<String>,
) -> Result<AccionApp, String> {
    let Paso::Accion {
        herramienta,
        args,
        proyecto,
        imagenes,
    } = paso
    else {
        return Err("no es un paso de acción".into());
    };
    let fila = acciones::por_nombre(herramienta)
        .ok_or_else(|| format!("no hay ninguna acción «{herramienta}»"))?;
    let args: Map<String, Value> = args
        .iter()
        .map(|(k, v)| (k.clone(), vars.sustituir_valor(v)))
        .collect();
    let mut hecha = acciones::validar(fila, &args).map_err(|e| e.to_string())?;
    if let Some(p) = proyecto.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        if !CON_PROYECTO.contains(&fila.accion) {
            return Err(format!("«{herramienta}» no va a un chat concreto"));
        }
        let id = resolver(p).ok_or_else(|| format!("no hay ningún chat «{p}»"))?;
        con_proyecto(&mut hecha.pedido, &id);
    }
    let rutas = vars.rutas(imagenes);
    if !rutas.is_empty() {
        if !CON_IMAGENES.contains(&fila.accion) {
            return Err(format!("«{herramienta}» no lleva imágenes"));
        }
        let mut todas: Vec<Value> = hecha.pedido["imagenes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        todas.extend(rutas.iter().map(|r| json!(r.display().to_string())));
        hecha.pedido["imagenes"] = Value::Array(todas);
        // Un `texto` obligatorio vacio no deja mandar solo la imagen; lo de
        // la tabla ya lo comprobo.
    }
    Ok(hecha)
}

// ---------------------------------------------------------------------------
// Correr

/// Lo que hace de verdad cada paso (la app, o las pruebas).
pub trait Hacedor {
    /// Una accion de la app ya armada; pregunta antes si lleva `confirmar`.
    fn accion(&mut self, a: &AccionApp) -> Result<String, String>;
    /// Un pedido al asistente: lo que contesto.
    fn asistente(
        &mut self,
        pedido: &str,
        proyecto: Option<&str>,
        imagenes: &[PathBuf],
        bloquear: Tipo,
    ) -> Result<String, String>;
    /// El id de un chat por su nombre o su id.
    fn proyecto(&self, nombre: &str) -> Option<String>;
}

/// Como fue cada paso.
#[derive(Debug, Clone, PartialEq)]
pub struct Informe {
    pub pasos: Vec<Result<String, String>>,
}

impl Informe {
    pub fn bien(&self) -> bool {
        !self.pasos.is_empty() && self.pasos.iter().all(Result::is_ok)
    }

    /// Lo que contesto el ultimo paso que salio bien.
    pub fn ultimo(&self) -> Option<&str> {
        self.pasos
            .iter()
            .rev()
            .find_map(|r| r.as_ref().ok().map(String::as_str))
    }

    /// En una o dos lineas, para el aviso.
    pub fn resumen(&self, total: usize) -> String {
        match self.pasos.iter().position(Result::is_err) {
            Some(i) => format!(
                "⚠️ Paso {} de {total}: {}",
                i + 1,
                self.pasos[i]
                    .as_ref()
                    .err()
                    .map(String::as_str)
                    .unwrap_or_default()
            ),
            None => match self.ultimo() {
                Some(t) if !t.trim().is_empty() => recortar(t, 220),
                _ => format!("Hecho ({total} pasos)"),
            },
        }
    }
}

/// **Corre los pasos en orden.** El primero que falla para los demas (el
/// siguiente podria depender de el: «ponlo en el chat» y luego «hazle OCR»).
pub fn ejecutar(a: &Automatizacion, mut vars: Vars, h: &mut dyn Hacedor) -> Informe {
    vars.nombre = a.nombre.clone();
    let tipo = a.disparador.tipo();
    let mut pasos = Vec::new();
    for p in &a.pasos {
        let r = match p {
            Paso::Accion { .. } => armar_accion(p, &vars, &|n| h.proyecto(n)).and_then(|hecha| {
                match bloqueado(Some(tipo), &hecha.pedido) {
                    Some(m) => Err(m),
                    None => h.accion(&hecha),
                }
            }),
            Paso::Asistente {
                pedido,
                proyecto,
                imagenes,
            } => {
                let mut rutas = vars.rutas(imagenes);
                // Al capturar, la captura va con el pedido aunque no se diga.
                if rutas.is_empty()
                    && tipo == Tipo::AlCapturar
                    && let Some(c) = &vars.captura
                {
                    rutas.push(c.clone());
                }
                let proyecto = match proyecto.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
                    Some(p) => h
                        .proyecto(p)
                        .ok_or_else(|| format!("no hay ningún chat «{p}»")),
                    None => Ok(String::new()),
                };
                proyecto.and_then(|pr| {
                    h.asistente(
                        &vars.sustituir(pedido),
                        Some(pr.as_str()).filter(|p| !p.is_empty()),
                        &rutas,
                        tipo,
                    )
                })
            }
        };
        let fallo = r.is_err();
        if let Ok(t) = &r {
            vars.resultado = t.clone();
        }
        pasos.push(r);
        if fallo {
            break;
        }
    }
    Informe { pasos }
}

// ---------------------------------------------------------------------------
// Crear desde un pedido (el asistente, «cada vez que…»)

/// Lo que llega en `receta` del pedido `automatizacion` (lo arma la fila
/// `crear_automatizacion` de la tabla): `nombre`, `disparador.tipo`,
/// `disparador.hora?`, `disparador.dias?` y `pasos` (una lista, o un texto
/// con una lista JSON; cada paso es un texto —un pedido al asistente— o un
/// objeto `{herramienta, args?, proyecto?, imagenes?}` o
/// `{asistente: "…", proyecto?, imagenes?}`).
pub fn de_receta(
    receta: &Value,
    id: String,
    aparato: &str,
    ahora: i64,
) -> Result<Automatizacion, String> {
    let nombre = receta
        .get("nombre")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .ok_or("falta el nombre")?
        .to_string();
    let d = receta.get("disparador").cloned().unwrap_or(Value::Null);
    let tipo = d
        .get("tipo")
        .and_then(Value::as_str)
        .or_else(|| d.as_str())
        .unwrap_or_default();
    let disparador = match acciones::normalizar(tipo.trim())
        .replace([' ', '-'], "_")
        .as_str()
    {
        "al_capturar" => Disparador::AlCapturar,
        "al_sincronizar" => Disparador::AlSincronizar,
        "a_mano" => Disparador::AMano,
        "a_la_hora" => Disparador::ALaHora {
            hora: d
                .get("hora")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string(),
            dias: d
                .get("dias")
                .and_then(Value::as_array)
                .map(|l| {
                    l.iter()
                        .filter_map(|x| x.as_u64().map(|n| n as u8))
                        .collect()
                })
                .unwrap_or_default(),
        },
        otro => return Err(format!("disparador desconocido «{otro}»")),
    };
    let pasos_v = match receta.get("pasos") {
        Some(Value::String(s)) => match serde_json::from_str::<Value>(s) {
            Ok(v @ Value::Array(_)) => v,
            // Un texto que no es una lista: un pedido al asistente.
            _ if !s.trim().is_empty() => json!([s.trim()]),
            _ => return Err("faltan los pasos".into()),
        },
        Some(v @ Value::Array(_)) => v.clone(),
        _ => return Err("faltan los pasos".into()),
    };
    let mut pasos = Vec::new();
    for (i, p) in pasos_v.as_array().into_iter().flatten().enumerate() {
        pasos.push(paso_de(p).map_err(|e| format!("paso {}: {e}", i + 1))?);
    }
    let a = Automatizacion {
        id,
        nombre,
        activa: true,
        disparador,
        pasos,
        aparato: Some(aparato.to_string()).filter(|a| !a.is_empty()),
        creada: ahora,
        tocada: ahora,
        resto: Map::new(),
    };
    validar(&a)?;
    Ok(a)
}

/// Antes de mandar el pedido de crear una: si la receta no vale, el motivo
/// (para el modelo, que la arregla). Otro pedido pasa.
pub fn revisar_pedido(pedido: &Value) -> Result<(), String> {
    let es_crear = pedido.get("accion").and_then(Value::as_str) == Some("automatizacion")
        && pedido.get("operacion").and_then(Value::as_str) == Some("crear");
    if !es_crear {
        return Ok(());
    }
    let receta = pedido.get("receta").ok_or("falta la receta")?;
    de_receta(receta, "revisar".into(), "", 0)
        .map(|_| ())
        .map_err(|e| format!("la automatización no vale: {e}"))
}

fn paso_de(v: &Value) -> Result<Paso, String> {
    let cadena = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let imagenes: Vec<String> = v
        .get("imagenes")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    match v {
        Value::String(s) if !s.trim().is_empty() => Ok(Paso::Asistente {
            pedido: s.trim().to_string(),
            proyecto: None,
            imagenes: Vec::new(),
        }),
        Value::Object(o) => {
            if let Ok(p) = serde_json::from_value::<Paso>(v.clone()) {
                return Ok(p);
            }
            if let Some(pedido) = cadena("asistente").or_else(|| cadena("pedido")) {
                return Ok(Paso::Asistente {
                    pedido,
                    proyecto: cadena("proyecto"),
                    imagenes,
                });
            }
            let herramienta = cadena("herramienta")
                .or_else(|| cadena("accion"))
                .ok_or("ni «herramienta» ni «asistente»")?;
            let args = match o.get("args") {
                Some(Value::Object(a)) => a.clone(),
                _ => Map::new(),
            };
            Ok(Paso::Accion {
                herramienta,
                args,
                proyecto: cadena("proyecto"),
                imagenes,
            })
        }
        _ => Err("un paso es un texto o un objeto".into()),
    }
}

// ---------------------------------------------------------------------------
// La lista de Ajustes

/// Una fila de la lista (Ajustes → Asistente).
#[derive(Debug, Clone, PartialEq)]
pub struct Resumen {
    pub id: String,
    pub nombre: String,
    /// «Al capturar · escribir_en_chat en «Obra» → «Hazle OCR»».
    pub detalle: String,
    pub activa: bool,
    /// `None`: corre aqui; si no, el aparato donde corre.
    pub otro_aparato: Option<String>,
    pub a_mano: bool,
    /// Si no se entiende, el motivo.
    pub rota: Option<String>,
}

/// Las filas de la lista, en su orden.
pub fn resumenes(lista: &Lista, aparato: &str) -> Vec<Resumen> {
    lista
        .entradas
        .iter()
        .map(|e| match e {
            Entrada::Buena(a) => Resumen {
                id: a.id.clone(),
                nombre: a.nombre.clone(),
                detalle: format!("{} · {}", a.disparador.en_palabras(), a.pasos_en_palabras()),
                activa: a.activa,
                otro_aparato: (!a.corre_aqui(aparato))
                    .then(|| a.aparato.clone().unwrap_or_default()),
                a_mano: a.disparador == Disparador::AMano,
                rota: None,
            },
            Entrada::Rota { valor, motivo } => Resumen {
                id: valor
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                nombre: valor
                    .get("nombre")
                    .and_then(Value::as_str)
                    .unwrap_or("(sin nombre)")
                    .to_string(),
                detalle: String::new(),
                activa: false,
                otro_aparato: None,
                a_mano: false,
                rota: Some(motivo.clone()),
            },
        })
        .collect()
}

// ---------------------------------------------------------------------------
// El disco

/// El fichero, en la carpeta de un chat.
pub fn fichero_en(carpeta_del_chat: &Path) -> PathBuf {
    carpeta_del_chat.join(RUTA_EN_EL_CHAT)
}

/// Lee la lista del chat. Sin fichero, vacia.
pub fn leer_de(carpeta_del_chat: &Path) -> Result<Lista, String> {
    match std::fs::read_to_string(fichero_en(carpeta_del_chat)) {
        Ok(t) => Lista::de_json(&t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Lista::default()),
        Err(e) => Err(e.to_string()),
    }
}

/// El chat «Asistente» (creandolo si no esta): su id y su carpeta.
pub fn chat_del_asistente(
    raiz: &Path,
    aparato: &str,
    ahora: i64,
) -> std::io::Result<(String, PathBuf)> {
    let f = pixpin_proyecto::almacen::asegurar_asistente(raiz, ahora, aparato)?;
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &f.id);
    Ok((f.id, carpeta))
}

/// Lee la lista de este PC.
pub fn leer(raiz: &Path, aparato: &str) -> Result<Lista, String> {
    let (_, carpeta) = chat_del_asistente(raiz, aparato, pixpin_shell::entorno::ahora_utc_ms())
        .map_err(|e| e.to_string())?;
    leer_de(&carpeta)
}

/// Un solo cambio a la vez en el fichero (la UI, el pedido y el motor).
static CERROJO: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// **Cambia la lista** y la guarda (escritura atomica) y se asegura de que
/// el mensaje que la hace viajar este en el chat. `cambio` devuelve lo que se
/// le contesta al que pidio; si devuelve `Err`, no se escribe nada.
pub fn cambiar<R>(
    raiz: &Path,
    aparato: &str,
    ahora: i64,
    cambio: impl FnOnce(&mut Lista) -> Result<R, String>,
) -> Result<R, String> {
    let _g = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    let (proyecto, carpeta) =
        chat_del_asistente(raiz, aparato, ahora).map_err(|e| e.to_string())?;
    let mut lista = leer_de(&carpeta)?;
    let r = cambio(&mut lista)?;
    let ruta = fichero_en(&carpeta);
    if let Some(dir) = ruta.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = ruta.with_extension("json.tmp");
    std::fs::write(&tmp, lista.a_json()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &ruta).map_err(|e| e.to_string())?;
    asegurar_mensaje(raiz, &proyecto, aparato, ahora).map_err(|e| e.to_string())?;
    Ok(r)
}

/// El mensaje ARCHIVO del chat «Asistente» que senala el fichero (asi viaja
/// con la sincronizacion, como un `.leccion`). Si ya esta, no se toca.
fn asegurar_mensaje(raiz: &Path, proyecto: &str, aparato: &str, ahora: i64) -> std::io::Result<()> {
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, proyecto);
    senalar_fichero(
        &carpeta,
        proyecto,
        aparato,
        ahora,
        &FicheroQueViaja {
            id_mensaje: ID_MENSAJE,
            nombre: "automatizaciones.json",
            ruta_portatil: RUTA_PORTATIL,
            en_el_chat: RUTA_EN_EL_CHAT,
            texto: "⚙️ Las automatizaciones del asistente (se cambian en Ajustes → Asistente)",
        },
    )
}

/// Un fichero suelto que viaja **solo con los aparatos propios**, senalado
/// por un mensaje ARCHIVO fijo del chat «Asistente» (que no se comparte con
/// personas): las automatizaciones y la biblioteca de figuras.
pub struct FicheroQueViaja {
    /// El id fijo del mensaje (uno por fichero, nunca dos).
    pub id_mensaje: &'static str,
    pub nombre: &'static str,
    /// `pixpin:files/...`: donde lo deja el movil.
    pub ruta_portatil: &'static str,
    /// Dentro de la carpeta del chat (`android/...`).
    pub en_el_chat: &'static str,
    pub texto: &'static str,
}

/// **Pone el mensaje ARCHIVO que hace viajar el fichero**, si no esta (el
/// fichero cambia; el mensaje es el mismo y la sincro mira el fichero).
pub fn senalar_fichero(
    carpeta: &Path,
    proyecto: &str,
    aparato: &str,
    ahora: i64,
    f: &FicheroQueViaja,
) -> std::io::Result<()> {
    use pixpin_proyecto::cuaderno;
    let c = match cuaderno::Cuaderno::leer_de(carpeta) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => return Err(e),
    };
    if c.mensajes.iter().any(|m| m.id == f.id_mensaje) {
        return Ok(());
    }
    let numero = c.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let bytes = std::fs::metadata(carpeta.join(f.en_el_chat))
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    let mut m = cuaderno::Mensaje::adjunto(
        cuaderno::Clase::Archivo,
        f.nombre,
        f.ruta_portatil,
        bytes,
        &cuaderno::Sello {
            cuando: ahora,
            numero,
            aparato: aparato.to_string(),
            proyecto: proyecto.to_string(),
        },
    );
    m.id = f.id_mensaje.into();
    m.texto = f.texto.into();
    cuaderno::anadir(carpeta, &m)
}

/// Un id nuevo.
pub fn id_nuevo(ahora: i64) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    format!("auto-{ahora}-{}", N.fetch_add(1, Ordering::Relaxed))
}

/// Corta a `n` letras con «…».
pub fn recortar(t: &str, n: usize) -> String {
    let t = t.trim();
    if t.chars().count() <= n {
        return t.to_string();
    }
    let mut s: String = t.chars().take(n).collect();
    s.push('…');
    s
}
