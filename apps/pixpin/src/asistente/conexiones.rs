//! **Las conexiones del asistente en la app** (17-oct-2026): Google Drive y
//! los que vengan, de `pixpin_ia::conexiones`.
//!
//! - [`hacer`]: las cuatro herramientas (buscar, leer, bajar, subir) con las
//!   piezas de siempre del chat: el archivo se elige como en `en_el_chat`,
//!   el texto de un PDF o un Word lo saca su lector (con el OCR si hace
//!   falta), lo bajado entra al chat con el pedido `chat`, y subir pregunta
//!   en la burbuja («¿Sí?») antes de mandar un byte.
//! - Ajustes → Asistente → Conexiones: [`guardar_cliente`], [`conectar`],
//!   [`desconectar`] y [`estado`]. Conectar abre el navegador y espera la
//!   vuelta en su hilo; la ventana se despierta al acabar ([`novedades`]).
//!
//! Los tokens y el ID de cliente van cifrados con DPAPI
//! (`pixpin_ia::conexiones::guardado`): por aqui no pasan a `tracing` ni a
//! los ajustes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use pixpin_ia::acciones::AccionApp;
use pixpin_ia::conexiones::{self, Servicio, acciones::Chat, guardado};
use serde_json::json;

use super::acciones::Confirmar;
use super::en_el_chat::Contexto;

/// Lo del chat que piden las herramientas de las conexiones.
struct DelChat<'a> {
    cx: &'a Contexto<'a>,
    confirmar: &'a Confirmar,
}

impl Chat for DelChat<'_> {
    fn archivo(&self, descripcion: Option<&str>) -> Result<(String, PathBuf), String> {
        super::en_el_chat::archivo_del_chat(self.cx, descripcion)
    }

    fn texto_de(&self, ruta: &Path) -> Result<String, String> {
        super::en_el_chat::texto_del_documento(ruta, &[], self.cx.parar)
    }

    fn al_chat(&self, ruta: &Path, frase: &str) -> Result<(), String> {
        let mut pedido = json!({"pixpin": 1, "accion": "chat", "texto": frase, "archivos": [ruta]});
        if !self.cx.proyecto.trim().is_empty() {
            pedido["proyecto"] = json!(self.cx.proyecto);
        }
        match (self.cx.mandar)(&pedido.to_string()) {
            pixpin_shell::mensajero::respuesta::ACEPTADO => Ok(()),
            _ => Err("PixPin no acepto el archivo en el chat".into()),
        }
    }

    fn confirmar(&self, pregunta: &str) -> bool {
        (self.confirmar)(pregunta)
    }

    fn carpeta(&self) -> PathBuf {
        std::env::temp_dir().join("pixpin-asistente").join("conexiones")
    }
}

/// **Hace una herramienta de las conexiones** desde el chat. Corre en el
/// hilo del turno: la ventana sigue suelta.
pub fn hacer(a: &AccionApp, cx: &Contexto, confirmar: &Confirmar) -> Result<String, String> {
    if a.herramienta == conexiones::acciones::VER_CALENDARIO {
        return Ok(ver_calendario(&a.pedido));
    }
    let r = conexiones::acciones::hacer(a, &|s| conexiones::Conexion::abrir(s), &DelChat { cx, confirmar });
    // Un permiso retirado borra los tokens: que Ajustes lo vea.
    olvidar();
    r
}

// ---------------------------------------------------------------------------
// Ajustes → Asistente → Conexiones

/// Lo que se ensena de cada servicio.
#[derive(Debug, Clone, Default)]
struct Ficha {
    /// El ID de cliente guardado (no es secreto: se ensena entero).
    cliente: Option<String>,
    conectado: bool,
    cuenta: Option<String>,
    /// Esperando al navegador.
    conectando: bool,
    /// Lo ultimo que paso (un error al guardar o al conectar).
    aviso: Option<Result<String, String>>,
}

static FICHAS: Mutex<Option<HashMap<&'static str, Ficha>>> = Mutex::new(None);
static NOVEDADES: AtomicU64 = AtomicU64::new(0);
/// Para cortar una espera al navegador (Desconectar, o Conectar otra vez).
static ESPERA: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// Sube cuando cambia algo de las conexiones: la ventana de ajustes repinta.
pub fn novedades() -> u64 {
    NOVEDADES.load(Ordering::SeqCst)
}

fn leer_ficha(s: &Servicio) -> Ficha {
    let g = guardado::carpeta().map(|c| guardado::leer_de(&c, s)).unwrap_or_default();
    Ficha {
        cliente: g.cliente.as_ref().map(|c| c.id.clone()),
        conectado: g.conectado(),
        cuenta: g.cuenta.clone(),
        ..Default::default()
    }
}

fn con_fichas<R>(f: impl FnOnce(&mut HashMap<&'static str, Ficha>) -> R) -> R {
    let mut g = FICHAS.lock().unwrap_or_else(|e| e.into_inner());
    let mapa = g.get_or_insert_with(|| conexiones::SERVICIOS.iter().map(|s| (s.id, leer_ficha(s))).collect());
    f(mapa)
}

/// Vuelve a mirar el disco (lo guardado cambio por otro camino: un permiso
/// retirado al usar una herramienta). Lo que esta en curso se conserva.
fn olvidar() {
    con_fichas(|m| {
        for s in conexiones::SERVICIOS {
            let antes = m.remove(s.id).unwrap_or_default();
            let mut f = leer_ficha(s);
            f.conectando = antes.conectando;
            f.aviso = antes.aviso;
            m.insert(s.id, f);
        }
    });
    NOVEDADES.fetch_add(1, Ordering::SeqCst);
}

fn avisar(despertar: &(dyn Fn() + Send + Sync)) {
    NOVEDADES.fetch_add(1, Ordering::SeqCst);
    despertar();
}

pub fn hay_cliente(s: &Servicio) -> bool {
    con_fichas(|m| m.get(s.id).is_some_and(|f| f.cliente.is_some()))
}

pub fn conectado(s: &Servicio) -> bool {
    con_fichas(|m| m.get(s.id).is_some_and(|f| f.conectado))
}

pub fn conectando(s: &Servicio) -> bool {
    con_fichas(|m| m.get(s.id).is_some_and(|f| f.conectando))
}

/// La linea de ayuda de la fila del ID de cliente.
pub fn estado_cliente(s: &Servicio, t: &pixpin_store::Catalogo) -> String {
    con_fichas(|m| {
        let f = m.get(s.id).cloned().unwrap_or_default();
        match &f.cliente {
            Some(id) => format!("{} {id}", t.t("ajustes2-conexion-cliente-guardado")),
            None => {
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("consola", s.consola);
                t.t_args("ajustes2-conexion-sin-cliente", &args)
            }
        }
    })
}

/// La linea de ayuda de la fila de Conectar/Desconectar.
pub fn estado(s: &Servicio, t: &pixpin_store::Catalogo) -> String {
    con_fichas(|m| {
        let f = m.get(s.id).cloned().unwrap_or_default();
        let base = if f.conectando {
            t.t("ajustes2-conexion-conectando")
        } else if f.conectado {
            match &f.cuenta {
                Some(c) if !c.is_empty() => format!("✓ {} · {c}", t.t("ajustes2-conexion-conectado")),
                _ => format!("✓ {}", t.t("ajustes2-conexion-conectado")),
            }
        } else {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("servicio", s.nombre);
            t.t_args("ajustes2-conexion-sin-conectar", &args)
        };
        match f.aviso {
            Some(Err(e)) => format!("{base} · ⚠️ {e}"),
            _ => base,
        }
    })
}

/// **Guarda el ID de cliente** pegado (cifrado). Lo que no lo parece, ni
/// se guarda.
pub fn guardar_cliente(s: &'static Servicio, escrito: &str, despertar: Arc<dyn Fn() + Send + Sync>) {
    let r = guardado::carpeta()
        .ok_or_else(|| "no se encuentra LOCALAPPDATA".to_string())
        .and_then(|c| conexiones::guardar_cliente_en(s, &c, escrito));
    match r {
        Ok(_) => {
            olvidar();
            con_fichas(|m| m.entry(s.id).or_default().aviso = None);
        }
        Err(e) => con_fichas(|m| m.entry(s.id).or_default().aviso = Some(Err(e))),
    }
    avisar(despertar.as_ref());
}

/// Abre la pagina de permiso en el navegador del usuario. Solo `https://`.
fn abrir_navegador(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("direccion de permiso no segura".into());
    }
    pixpin_shell::abrir::abrir(Path::new(url)).map_err(|e| format!("no se pudo abrir el navegador: {}", e.message()))
}

/// **Conecta**: el navegador y la espera van en su hilo; la ventana se
/// despierta al acabar.
pub fn conectar(s: &'static Servicio, despertar: Arc<dyn Fn() + Send + Sync>) {
    let parar = Arc::new(AtomicBool::new(false));
    if let Some(antes) = ESPERA.lock().unwrap_or_else(|e| e.into_inner()).replace(parar.clone()) {
        antes.store(true, Ordering::SeqCst);
    }
    con_fichas(|m| {
        let f = m.entry(s.id).or_default();
        f.conectando = true;
        f.aviso = None;
    });
    avisar(despertar.as_ref());
    let d = despertar.clone();
    let lanzado = std::thread::Builder::new().name("asistente-conectar".into()).spawn(move || {
        let r = guardado::carpeta()
            .ok_or_else(|| "no se encuentra LOCALAPPDATA".to_string())
            .and_then(|c| conexiones::conectar_en(s, &s.extremos(), &c, &abrir_navegador, &parar, conexiones::oauth::ESPERA));
        if let Err(e) = &r {
            tracing::warn!("conexiones: no se conecto {}: {e}", s.nombre);
        }
        olvidar();
        // Google Calendar recien conectado: sus eventos, ya.
        if r.is_ok() && s.id == pixpin_ia::conexiones::calendario::GOOGLE_CALENDAR.id {
            crate::calendario_google::leer_ya();
        }
        con_fichas(|m| {
            let f = m.entry(s.id).or_default();
            f.conectando = false;
            // Cortado a proposito: sin aviso.
            f.aviso = r.err().filter(|e| e != "cancelado").map(Err);
        });
        avisar(d.as_ref());
    });
    if lanzado.is_err() {
        con_fichas(|m| {
            let f = m.entry(s.id).or_default();
            f.conectando = false;
            f.aviso = Some(Err("no se pudo empezar a conectar".into()));
        });
        avisar(despertar.as_ref());
    }
}

/// **Desconecta**: corta una espera en curso, borra los tokens de este PC
/// ya y retira el permiso en el servicio en su hilo (es una peticion de
/// red).
pub fn desconectar(s: &'static Servicio, despertar: Arc<dyn Fn() + Send + Sync>) {
    if let Some(antes) = ESPERA.lock().unwrap_or_else(|e| e.into_inner()).take() {
        antes.store(true, Ordering::SeqCst);
    }
    let Some(carpeta) = guardado::carpeta() else { return };
    // Los tokens se quitan del disco aqui mismo (lo que el usuario espera al
    // pulsar); el aviso al servicio va despues, sin esperarlo.
    let tokens = {
        let mut g = guardado::leer_de(&carpeta, s);
        let t = g.tokens.take();
        g.cuenta = None;
        if let Err(e) = guardado::guardar_en(&carpeta, s, &g) {
            con_fichas(|m| m.entry(s.id).or_default().aviso = Some(Err(e)));
        }
        t
    };
    olvidar();
    con_fichas(|m| m.entry(s.id).or_default().conectando = false);
    avisar(despertar.as_ref());
    if let Some(t) = tokens {
        let token = if t.refresco.is_empty() { t.acceso.clone() } else { t.refresco.clone() };
        let _ = std::thread::Builder::new().name("asistente-desconectar".into()).spawn(move || {
            if let Err(e) = conexiones::oauth::revocar(&s.extremos(), &token) {
                tracing::info!("conexiones: no se pudo retirar el permiso de {} en el servicio: {e}", s.nombre);
            }
        });
    }
}

/// **«¿Que tengo hoy?»**: los eventos de Google Calendar de los dias
/// pedidos (`desde`: hoy, mañana, un dia de la semana, «semana» o
/// AAAA-MM-DD; `dias`: cuantos), de lo que la app ya leyo.
fn ver_calendario(args: &serde_json::Value) -> String {
    const DIA: i64 = 86_400_000;
    let hoy = pixpin_shell::entorno::ahora_local_ms().div_euclid(DIA);
    let desde = args["desde"].as_str().unwrap_or("hoy").trim().to_lowercase();
    let mut dias = args["dias"].as_i64().unwrap_or(1).clamp(1, 31);
    // (El dia 0, el 1-1-1970, fue jueves: lunes = 0.)
    let semana_de = |d: i64| (d + 3).rem_euclid(7);
    let nombres = ["lunes", "martes", "miercoles", "jueves", "viernes", "sabado", "domingo"];
    let sin_tildes = desde.replace('á', "a").replace('é', "e");
    let primero = match sin_tildes.as_str() {
        "" | "hoy" => hoy,
        "mañana" | "manana" => hoy + 1,
        "pasado mañana" | "pasado manana" | "pasado" => hoy + 2,
        "semana" | "esta semana" => {
            dias = dias.max(7);
            hoy - semana_de(hoy)
        }
        "semana que viene" | "proxima semana" | "la semana que viene" => {
            dias = dias.max(7);
            hoy - semana_de(hoy) + 7
        }
        otro => {
            let sin = otro.replace("sábado", "sabado").replace("miércoles", "miercoles");
            if let Some(k) = nombres.iter().position(|n| sin.contains(n)) {
                hoy + (k as i64 - semana_de(hoy)).rem_euclid(7)
            } else {
                let p: Vec<i64> = otro.split(['-', '/']).filter_map(|x| x.parse().ok()).collect();
                match p.as_slice() {
                    [a, m, d] if *a > 1900 => pixpin_ia::conexiones::calendario::dias_desde_1970(*a, *m as u32, *d as u32),
                    [d, m, a] if *a > 1900 => pixpin_ia::conexiones::calendario::dias_desde_1970(*a, *m as u32, *d as u32),
                    _ => hoy,
                }
            }
        }
    };
    crate::calendario_google::para_el_modelo(primero * DIA, (primero + dias) * DIA)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lo_que_no_parece_un_id_de_cliente_ni_se_guarda_y_se_dice() {
        let despertado = Arc::new(AtomicU64::new(0));
        let d = despertado.clone();
        let s = &conexiones::SERVICIOS[0];
        let antes = hay_cliente(s);
        guardar_cliente(s, "hola que tal", Arc::new(move || {
            d.fetch_add(1, Ordering::SeqCst);
        }));
        assert_eq!(despertado.load(Ordering::SeqCst), 1);
        assert_eq!(hay_cliente(s), antes, "no cambio lo guardado");
        let t = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        assert!(estado(s, &t).contains("no parece un ID de cliente"), "{}", estado(s, &t));
        // La fila del ID dice donde se crea.
        if !antes {
            assert!(estado_cliente(s, &t).contains("console.cloud.google.com"));
        }
    }
}
