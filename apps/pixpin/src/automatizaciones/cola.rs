//! **La cola de encargos del asistente** (15-oct-2026). El usuario: «entro
//! [en Flow], pongo asistente y le escribo cualquier cosa; el asistente va
//! haciendo las tareas en background y me manda una notificación cuando
//! haya terminado».
//!
//! - `p asistente <texto>` (o `p ia`, `p a`) manda el pedido `asistente`;
//!   la app lo **apunta** aqui y contesta en el acto. Se pueden mandar varios
//!   seguidos: esperan su turno.
//! - **Uno a la vez**, en un solo hilo de fondo: con el modelo local solo
//!   cabe uno cargado (`pixpin_ia::relevo`), y con la nube, hacerlos en
//!   orden evita que dos turnos se pisen en el mismo chat «Asistente».
//!   Nada de esto corre en el hilo de la ventana: el pedido solo manda el
//!   encargo por un canal; otro hilo lo apunta en el disco.
//! - **Se guardan** en `<raiz>/asistente-encargos.json` (uno por equipo, no
//!   viaja con la sincro): el plugin de Flow lo lee para ensenar «en cola»,
//!   «haciendo», «hecha» o «falló», y lo que quedo en cola al cerrarse
//!   PixPin se hace al volver a abrirla. El que estaba **a medias** no se
//!   repite (podria apuntar dos veces la misma tarea): queda como fallido,
//!   «PixPin se cerró a medias», y se avisa.
//! - Al acabar cada uno, **notificacion de Windows** (`pixpin_shell::
//!   notificacion`) con «Abrir» (el lienzo si dibujo, si no la respuesta en
//!   el chat «Asistente»). Si Windows no la deja salir, el aviso flotante de
//!   siempre. Si un turno pide permiso para algo que borra o envia, sale el
//!   aviso flotante en ese momento, con «Sí»/«No».
//!
//! Android no tiene encargos en segundo plano ni notificaciones del
//! asistente (comprobado con `gh api`, 15-oct): esto es solo del PC.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use pixpin_shell::notificacion::{self, Boton, Toast};
use serde::{Deserialize, Serialize};

use super::encargo::{self, Encargo};

/// El fichero, en la raiz de los datos.
pub const FICHERO: &str = "asistente-encargos.json";
/// La version del fichero.
pub const VERSION: u32 = 1;
/// Cuantos acabados se guardan (para que Flow ensene los recientes).
pub const ACABADOS: usize = 10;
/// Lo que mide, como mucho, el titulo de la notificacion y su linea.
const TITULO: usize = 48;
const LINEA: usize = 140;

/// En que va un encargo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Estado {
    EnCola,
    Haciendo,
    Hecha,
    Fallo,
}

impl Estado {
    pub fn acabado(self) -> bool {
        matches!(self, Estado::Hecha | Estado::Fallo)
    }
}

/// Adonde lleva «Abrir» al acabar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Abrir {
    /// El chat «Asistente».
    pub proyecto: String,
    /// La nota de la respuesta.
    pub codigo: String,
    /// El lienzo donde dibujo, si dibujo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lienzo: Option<String>,
}

/// Un encargo, tal como se guarda (y lo lee el plugin).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Registro {
    pub id: String,
    pub texto: String,
    #[serde(default)]
    pub proyecto: Option<String>,
    #[serde(default)]
    pub imagenes: Vec<PathBuf>,
    pub estado: Estado,
    /// Cuando se pidio, empezo y acabo (ms).
    pub creado: i64,
    #[serde(default)]
    pub empezado: Option<i64>,
    #[serde(default)]
    pub acabado: Option<i64>,
    /// Al acabar: la primera linea de la respuesta, o el motivo del fallo.
    #[serde(default)]
    pub resumen: String,
    /// Acabo, pero con algun paso fallido.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub con_fallos: bool,
    #[serde(default)]
    pub abrir: Option<Abrir>,
}

impl Registro {
    pub fn nuevo(id: String, e: &Encargo, ahora: i64) -> Registro {
        Registro {
            id,
            texto: e.texto.clone(),
            proyecto: e.proyecto.clone(),
            imagenes: e.imagenes.clone(),
            estado: Estado::EnCola,
            creado: ahora,
            empezado: None,
            acabado: None,
            resumen: String::new(),
            con_fallos: false,
            abrir: None,
        }
    }

    pub fn encargo(&self) -> Encargo {
        Encargo {
            texto: self.texto.clone(),
            proyecto: self.proyecto.clone(),
            imagenes: self.imagenes.clone(),
            origen: None,
            bloquear: None,
        }
    }
}

/// Lo que sale de hacer un encargo.
#[derive(Debug, Clone, PartialEq)]
pub struct Salida {
    pub texto: String,
    pub bien: bool,
    pub abrir: Option<Abrir>,
}

/// La lista entera (el fichero).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Lista {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub encargos: Vec<Registro>,
}

impl Lista {
    /// La del disco; sin fichero o roto, vacia (un fichero roto no puede
    /// dejar la cola parada).
    pub fn leer(raiz: &Path) -> Lista {
        std::fs::read_to_string(raiz.join(FICHERO))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// La escribe entera, primero a un fichero aparte (un corte a medias no
    /// deja un JSON roto que el plugin no entienda).
    pub fn guardar(&self, raiz: &Path) -> std::io::Result<()> {
        let mut l = self.clone();
        l.version = VERSION;
        let t = serde_json::to_string_pretty(&l).map_err(std::io::Error::other)?;
        let tmp = raiz.join(format!("{FICHERO}.tmp"));
        std::fs::write(&tmp, t)?;
        std::fs::rename(&tmp, raiz.join(FICHERO))
    }

    /// El siguiente que toca: el primero en cola, por orden de llegada.
    pub fn siguiente(&self) -> Option<usize> {
        self.encargos
            .iter()
            .position(|r| r.estado == Estado::EnCola)
    }

    /// Quita los acabados mas viejos (se quedan [`ACABADOS`]); los que
    /// esperan no se quitan nunca.
    pub fn podar(&mut self) {
        let acabados = self.encargos.iter().filter(|r| r.estado.acabado()).count();
        let mut sobran = acabados.saturating_sub(ACABADOS);
        self.encargos.retain(|r| {
            if sobran > 0 && r.estado.acabado() {
                sobran -= 1;
                false
            } else {
                true
            }
        });
    }

    /// Al arrancar: lo que estaba haciendose cuando se cerro PixPin queda
    /// como fallido (no se repite). Devuelve esos, para avisar.
    pub fn tras_un_cierre(&mut self, ahora: i64) -> Vec<Registro> {
        let mut v = Vec::new();
        for r in self
            .encargos
            .iter_mut()
            .filter(|r| r.estado == Estado::Haciendo)
        {
            r.estado = Estado::Fallo;
            r.acabado = Some(ahora);
            r.resumen = A_MEDIAS.into();
            v.push(r.clone());
        }
        v
    }

    fn poner(&mut self, id: &str, cambio: impl FnOnce(&mut Registro)) -> Option<Registro> {
        let r = self.encargos.iter_mut().find(|r| r.id == id)?;
        cambio(r);
        Some(r.clone())
    }
}

/// El motivo de un encargo cortado por un cierre.
pub const A_MEDIAS: &str = "PixPin se cerró a medias: pídelo otra vez";

/// **La cola**: la lista en memoria (y en el disco) y la señal al hilo que
/// trabaja.
pub struct Cola {
    raiz: PathBuf,
    lista: Mutex<Lista>,
    senal: Condvar,
}

impl Cola {
    /// La del disco, con lo que quedo a medias ya marcado (y guardado).
    /// Devuelve tambien esos, para avisar.
    pub fn abrir(raiz: &Path, ahora: i64) -> (Arc<Cola>, Vec<Registro>) {
        let mut l = Lista::leer(raiz);
        let cortados = l.tras_un_cierre(ahora);
        if !cortados.is_empty() {
            let _ = l.guardar(raiz);
        }
        let c = Cola {
            raiz: raiz.to_path_buf(),
            lista: Mutex::new(l),
            senal: Condvar::new(),
        };
        (Arc::new(c), cortados)
    }

    fn con<R>(&self, f: impl FnOnce(&mut Lista) -> R) -> R {
        let mut g = self.lista.lock().unwrap_or_else(|e| e.into_inner());
        let r = f(&mut g);
        if let Err(e) = g.guardar(&self.raiz) {
            tracing::warn!(?e, "cola del asistente: no se pudo guardar");
        }
        r
    }

    /// Apunta un encargo (al final) y despierta al hilo.
    pub fn anadir(&self, r: Registro) {
        self.con(|l| {
            // El mismo id dos veces (Flow repitio el Intro): una vez.
            if !l.encargos.iter().any(|x| x.id == r.id) {
                l.encargos.push(r);
            }
        });
        self.senal.notify_all();
    }

    #[cfg(test)]
    pub fn lista(&self) -> Lista {
        self.lista.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Toma el siguiente y lo marca «haciendo». Con `esperar`, duerme hasta
    /// que llegue uno; sin el, `None` si no hay.
    fn tomar(&self, esperar: bool, ahora: impl Fn() -> i64) -> Option<Registro> {
        let mut g = self.lista.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(i) = g.siguiente() {
                let r = &mut g.encargos[i];
                r.estado = Estado::Haciendo;
                r.empezado = Some(ahora());
                let r = r.clone();
                if let Err(e) = g.guardar(&self.raiz) {
                    tracing::warn!(?e, "cola del asistente: no se pudo guardar");
                }
                return Some(r);
            }
            if !esperar {
                return None;
            }
            g = self.senal.wait(g).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// **El bucle del hilo**: uno tras otro, cada uno con `hacer`; al acabar
    /// cada uno, `avisar`. Un encargo que falla (o revienta) no para a los
    /// demas. Con `esperar = false` vuelve cuando no queda ninguno (las
    /// pruebas).
    pub fn trabajar(
        &self,
        esperar: bool,
        ahora: impl Fn() -> i64,
        mut hacer: impl FnMut(&Registro) -> Result<Salida, String>,
        mut avisar: impl FnMut(&Registro),
    ) {
        while let Some(r) = self.tomar(esperar, &ahora) {
            let salida = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hacer(&r)))
                .unwrap_or_else(|_| Err("el asistente se rompió haciéndolo".into()));
            let fin = ahora();
            let hecho = self.con(|l| {
                let h = l.poner(&r.id, |x| {
                    x.acabado = Some(fin);
                    match salida {
                        Ok(s) => {
                            x.estado = Estado::Hecha;
                            x.con_fallos = !s.bien;
                            x.resumen = resumen(&s.texto, s.bien);
                            x.abrir = s.abrir;
                        }
                        Err(m) => {
                            x.estado = Estado::Fallo;
                            x.resumen = recortar_linea(&m);
                        }
                    }
                });
                l.podar();
                h
            });
            if let Some(h) = hecho {
                avisar(&h);
            }
        }
    }
}

// --- Los textos --------------------------------------------------------------

fn recortar_linea(t: &str) -> String {
    super::recortar(&t.split_whitespace().collect::<Vec<_>>().join(" "), LINEA)
}

/// Una linea sin las marcas de Markdown del principio ni los `**`.
fn limpia(l: &str) -> String {
    l.trim()
        .trim_start_matches(['#', '>', '-', '*', '•', ' '])
        .replace("**", "")
        .replace('`', "")
        .trim()
        .to_string()
}

/// **La linea de resumen** de una respuesta: con fallos, el aviso del fallo
/// (`⚠️ …`, al final de la respuesta); si no, su primera linea con algo.
pub fn resumen(texto: &str, bien: bool) -> String {
    let lineas: Vec<String> = texto
        .lines()
        .map(limpia)
        .filter(|l| !l.is_empty())
        .collect();
    let l = if bien {
        lineas.first()
    } else {
        lineas
            .iter()
            .rev()
            .find(|l| l.starts_with('⚠'))
            .or(lineas.first())
    };
    match l {
        Some(l) => recortar_linea(l),
        None => "Hecho.".into(),
    }
}

/// **La notificacion de un encargo acabado**: titulo corto con el resultado
/// y lo pedido, una linea y «Abrir».
pub fn notificacion(r: &Registro) -> Toast {
    let pedido = encargo::sin_fichas(&r.texto);
    let pedido = if pedido.is_empty() {
        "la foto".to_string()
    } else {
        pedido
    };
    let que = super::recortar(&pedido, TITULO);
    let titulo = match (r.estado, r.con_fallos) {
        (Estado::Hecha, false) => format!("✅ Hecho: {que}"),
        (Estado::Hecha, true) => format!("⚠️ Hecho a medias: {que}"),
        _ => format!("❌ No se pudo: {que}"),
    };
    let abrir = pedido_de_abrir(r.abrir.as_ref());
    Toast {
        titulo,
        texto: r.resumen.clone(),
        al_pulsar: Some(abrir.clone()),
        botones: vec![Boton::nuevo("Abrir", abrir)],
        ..Default::default()
    }
}

/// El pedido de «Abrir»: el lienzo si dibujo, si no la respuesta en el chat
/// «Asistente»; sin respuesta (fallo), el chat «Asistente».
pub fn pedido_de_abrir(a: Option<&Abrir>) -> serde_json::Value {
    use serde_json::json;
    match a {
        Some(Abrir {
            proyecto,
            lienzo: Some(l),
            ..
        }) => json!({
            "pixpin": 1, "accion": "abrir",
            "que": { "tipo": "hoja", "proyecto": proyecto, "referencia": l }
        }),
        Some(Abrir {
            proyecto, codigo, ..
        }) => json!({
            "pixpin": 1, "accion": "abrir",
            "que": { "tipo": "nota", "proyecto": proyecto, "codigo": codigo }
        }),
        None => json!({ "pixpin": 1, "accion": "asistente" }),
    }
}

/// Un id nuevo (el plugin manda el suyo; este es para los que no).
pub fn id_nuevo(ahora: i64) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    format!("e{ahora}-{}", N.fetch_add(1, Ordering::Relaxed))
}

// --- En la app ---------------------------------------------------------------

struct EnMarcha {
    entrada: Mutex<Sender<Registro>>,
}

static EN_MARCHA: OnceLock<EnMarcha> = OnceLock::new();

/// **Arranca la cola** (una vez, al empezar la app): el hilo que apunta y
/// el que trabaja. Lo que quedo en cola de la vez anterior se hace ya.
pub fn arrancar(app: encargo::DeLaApp) {
    let (tx, rx) = channel::<Registro>();
    if EN_MARCHA
        .set(EnMarcha {
            entrada: Mutex::new(tx),
        })
        .is_err()
    {
        return;
    }
    let raiz = app.entorno.ubicacion.raiz().to_path_buf();
    let lanzado = std::thread::Builder::new()
        .name("asistente-cola".into())
        .spawn(move || {
            let (cola, cortados) = Cola::abrir(&raiz, pixpin_shell::entorno::ahora_utc_ms());
            // El que apunta: el pedido no espera al disco.
            let c = cola.clone();
            let _ = std::thread::Builder::new()
                .name("asistente-cola-entrada".into())
                .spawn(move || {
                    for r in rx {
                        c.anadir(r);
                    }
                });
            let app = Arc::new(app);
            for r in &cortados {
                avisar_al_acabar(&app, r, None);
            }
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let tarjeta: Arc<Mutex<Option<crate::ventana_chat::sin_ventana::Mando>>> =
                Arc::new(Mutex::new(None));
            let (a1, t1) = (app.clone(), tarjeta.clone());
            cola.trabajar(
                true,
                pixpin_shell::entorno::ahora_utc_ms,
                move |r| hacer_de_verdad(&a1, r, &t1),
                move |r| {
                    let mando = tarjeta.lock().unwrap_or_else(|e| e.into_inner()).take();
                    avisar_al_acabar(&app, r, mando);
                },
            );
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar la cola del asistente");
    }
}

/// El icono de las notificaciones (el del plugin de Flow), que Windows lee
/// de un PNG en el disco.
const ICONO: &[u8] = include_bytes!("../../../pixpin-lanzador/plugin/Images/pixpin.png");

/// **Da de alta la identidad de las notificaciones y el protocolo de
/// «Abrir»** (`pixpin_shell::notificacion`), en su hilo: el arranque no
/// espera al registro.
pub fn registrar_notificaciones(exe: PathBuf, raiz: PathBuf) {
    let _ = std::thread::Builder::new()
        .name("notificaciones-alta".into())
        .spawn(move || {
            let png = raiz.join("cache").join("notificacion.png");
            let icono = match std::fs::metadata(&png) {
                Ok(m) if m.len() == ICONO.len() as u64 => Some(png),
                _ => {
                    let _ = std::fs::create_dir_all(raiz.join("cache"));
                    std::fs::write(&png, ICONO).ok().map(|_| png)
                }
            };
            if let Err(e) = notificacion::registrar(&exe, icono.as_deref()) {
                tracing::warn!(?e, "no se pudo dar de alta las notificaciones de Windows");
            }
        });
}

/// **El pedido `asistente` con texto**: a la cola. Vuelve en el acto (ni
/// toca el disco). Sin la cola en marcha (no deberia pasar), como antes:
/// en su hilo con el aviso flotante.
pub fn encolar(app: encargo::DeLaApp, e: Encargo, id: Option<String>) {
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let id = id
        .filter(|i| !i.trim().is_empty() && i.len() <= 64)
        .unwrap_or_else(|| id_nuevo(ahora));
    let r = Registro::nuevo(id, &e, ahora);
    if let Some(m) = EN_MARCHA.get() {
        let enviado = m
            .entrada
            .lock()
            .map(|tx| tx.send(r).is_ok())
            .unwrap_or(false);
        if enviado {
            return;
        }
    }
    encargo::lanzar(app, e);
}

fn chat_de(app: &encargo::DeLaApp) -> crate::ventana_chat::sin_ventana::Chat {
    crate::ventana_chat::sin_ventana::Chat {
        idioma: app.idioma,
        ubicacion: app.entorno.ubicacion.clone(),
        opciones: app.opciones,
    }
}

fn hacer_de_verdad(
    app: &Arc<encargo::DeLaApp>,
    r: &Registro,
    tarjeta: &Arc<Mutex<Option<crate::ventana_chat::sin_ventana::Mando>>>,
) -> Result<Salida, String> {
    let textos = pixpin_store::Catalogo::nuevo(app.idioma);
    let e = r.encargo();
    let motor = crate::ventana_chat::sin_ventana::motor(&app.entorno.ubicacion, &textos);
    let motor = crate::asistente::rapido::motor_o_sin(motor, &e.texto)?;
    // Una pregunta de permiso: la tarjeta flotante, para contestarla (una
    // por encargo; al acabar ensena la respuesta).
    let (a, t, titulo) = (app.clone(), tarjeta.clone(), titulo_de_la_tarjeta(r));
    let al_preguntar = move |_: &str| {
        let mut g = t.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_none() {
            *g = Some(crate::ventana_chat::sin_ventana::avisar(
                crate::ventana_chat::sin_ventana::Aviso {
                    titulo: titulo.clone(),
                    texto: crate::asistente::rapido::ESTADO_INICIAL.into(),
                    abrir: None,
                    acabado: false,
                    chat: Some(chat_de(&a)),
                },
            ));
        }
    };
    let h = encargo::hacer(
        &app.entorno,
        motor,
        &e,
        encargo::Permiso::EnLaBurbujaAvisando(Box::new(al_preguntar)),
        crate::asistente::acciones::mandar_a_pixpin(),
    )?;
    Ok(Salida {
        abrir: Some(Abrir {
            proyecto: h.proyecto,
            codigo: h.codigo,
            lienzo: h.lienzo,
        }),
        bien: h.bien,
        texto: h.texto,
    })
}

fn titulo_de_la_tarjeta(r: &Registro) -> String {
    let t = encargo::sin_fichas(&r.texto);
    if t.is_empty() {
        "🤖 Asistente".into()
    } else {
        format!("🤖 {}", super::recortar(&t, 48))
    }
}

/// Al acabar: la notificacion de Windows; si no sale, el aviso flotante.
/// Si ya habia tarjeta (por una pregunta), ademas ensena alli la respuesta.
fn avisar_al_acabar(
    app: &encargo::DeLaApp,
    r: &Registro,
    tarjeta: Option<crate::ventana_chat::sin_ventana::Mando>,
) {
    // **Una respuesta de texto, en un pin para leerla ya** (20-oct-2026, el
    // usuario: «que me muestre su respuesta en formato de pin… para que
    // pueda leer rapido las respuestas», sin abrir la app): el lector de
    // notas con la nota de la respuesta, y sin notificacion encima. Lo que
    // dibujo, fallo o ya tiene tarjeta (pregunto permiso) avisa como antes.
    if tarjeta.is_none()
        && r.estado == Estado::Hecha
        && let Some(Abrir {
            proyecto,
            codigo,
            lienzo: None,
        }) = &r.abrir
    {
        crate::notas_md::abrir(
            app.idioma,
            app.entorno.ubicacion.clone(),
            crate::notas_md::Destino::Mensaje {
                proyecto: proyecto.clone(),
                codigo: codigo.clone(),
            },
        );
        return;
    }
    let toast = notificacion(r);
    let salio = match notificacion::mostrar(&toast) {
        Ok(notificacion::Mostrado::Si) => true,
        Ok(notificacion::Mostrado::Apagadas) => {
            tracing::info!("asistente: notificaciones apagadas, va el aviso flotante");
            false
        }
        Err(e) => {
            tracing::warn!(
                ?e,
                "asistente: sin notificacion de Windows, va el aviso flotante"
            );
            false
        }
    };
    let abrir = r.abrir.as_ref().map(|a| {
        (
            app.idioma,
            app.entorno.ubicacion.clone(),
            app.opciones,
            a.proyecto.clone(),
            a.codigo.clone(),
        )
    });
    let texto = if r.estado == Estado::Hecha {
        r.resumen.clone()
    } else {
        format!("⚠️ {}", r.resumen)
    };
    match tarjeta {
        Some(m) => m.poner(texto, abrir),
        None if !salio => {
            let m =
                crate::ventana_chat::sin_ventana::avisar(crate::ventana_chat::sin_ventana::Aviso {
                    titulo: toast.titulo.clone(),
                    texto: String::new(),
                    abrir: None,
                    acabado: false,
                    chat: Some(chat_de(app)),
                });
            m.poner(texto, abrir);
        }
        None => {}
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::cell::Cell;

    fn raiz(nombre: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("pixpin-cola-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    fn enc(texto: &str) -> Encargo {
        Encargo {
            texto: texto.into(),
            ..Default::default()
        }
    }

    fn reloj() -> impl Fn() -> i64 {
        let t = Cell::new(1000);
        move || {
            t.set(t.get() + 1);
            t.get()
        }
    }

    fn bien(texto: &str) -> Result<Salida, String> {
        Ok(Salida {
            texto: texto.into(),
            bien: true,
            abrir: Some(Abrir {
                proyecto: "asis".into(),
                codigo: "IA-1".into(),
                lienzo: None,
            }),
        })
    }

    #[test]
    fn hace_los_encargos_en_orden_de_llegada() {
        let r = raiz("orden");
        let (c, _) = Cola::abrir(&r, 0);
        for (i, t) in ["uno", "dos", "tres"].iter().enumerate() {
            c.anadir(Registro::nuevo(format!("e{i}"), &enc(t), i as i64));
        }
        let mut hechos = Vec::new();
        let mut avisos = Vec::new();
        c.trabajar(
            false,
            reloj(),
            |x| {
                hechos.push(x.texto.clone());
                // Mientras hace uno, el siguiente sigue en cola en el disco.
                let l = Lista::leer(&r);
                assert_eq!(
                    l.encargos.iter().find(|y| y.id == x.id).unwrap().estado,
                    Estado::Haciendo
                );
                bien(&format!("Hecho {}", x.texto))
            },
            |x| avisos.push(x.clone()),
        );
        assert_eq!(hechos, ["uno", "dos", "tres"]);
        assert_eq!(
            avisos.iter().map(|a| a.estado).collect::<Vec<_>>(),
            [Estado::Hecha; 3]
        );
        assert_eq!(avisos[1].resumen, "Hecho dos");
        let l = Lista::leer(&r);
        assert!(
            l.encargos
                .iter()
                .all(|x| x.estado == Estado::Hecha && x.acabado.is_some())
        );
        assert_eq!(l.version, VERSION);
    }

    #[test]
    fn uno_que_falla_o_revienta_no_para_a_los_demas() {
        let r = raiz("fallo");
        let (c, _) = Cola::abrir(&r, 0);
        for (i, t) in ["bien", "mal", "panico", "bien otra vez"]
            .iter()
            .enumerate()
        {
            c.anadir(Registro::nuevo(format!("e{i}"), &enc(t), i as i64));
        }
        let mut avisos = Vec::new();
        c.trabajar(
            false,
            reloj(),
            |x| match x.texto.as_str() {
                "mal" => Err("sin clave de Claude".into()),
                "panico" => panic!("prueba"),
                t => bien(t),
            },
            |x| avisos.push((x.texto.clone(), x.estado, x.resumen.clone())),
        );
        assert_eq!(
            avisos,
            [
                ("bien".into(), Estado::Hecha, "bien".into()),
                ("mal".into(), Estado::Fallo, "sin clave de Claude".into()),
                (
                    "panico".into(),
                    Estado::Fallo,
                    "el asistente se rompió haciéndolo".into()
                ),
                (
                    "bien otra vez".into(),
                    Estado::Hecha,
                    "bien otra vez".into()
                ),
            ]
        );
    }

    #[test]
    fn lo_de_en_cola_se_hace_al_volver_y_lo_de_a_medias_no_se_repite() {
        let r = raiz("cierre");
        let mut l = Lista::default();
        let mut a = Registro::nuevo("a".into(), &enc("a medias"), 1);
        a.estado = Estado::Haciendo;
        l.encargos.push(a);
        l.encargos
            .push(Registro::nuevo("b".into(), &enc("esperando"), 2));
        l.guardar(&r).unwrap();
        let (c, cortados) = Cola::abrir(&r, 9);
        assert_eq!(cortados.len(), 1);
        assert_eq!(cortados[0].resumen, A_MEDIAS);
        assert_eq!(
            Lista::leer(&r).encargos[0].estado,
            Estado::Fallo,
            "ya guardado"
        );
        let mut hechos = Vec::new();
        c.trabajar(
            false,
            reloj(),
            |x| {
                hechos.push(x.id.clone());
                bien("ok")
            },
            |_| {},
        );
        assert_eq!(hechos, ["b"], "el cortado no se repite");
    }

    #[test]
    fn un_fichero_roto_no_para_la_cola_y_el_mismo_id_va_una_vez() {
        let r = raiz("roto");
        std::fs::write(r.join(FICHERO), "{roto").unwrap();
        let (c, _) = Cola::abrir(&r, 0);
        c.anadir(Registro::nuevo("x".into(), &enc("hola"), 1));
        c.anadir(Registro::nuevo("x".into(), &enc("hola"), 1));
        assert_eq!(c.lista().encargos.len(), 1);
        assert_eq!(Lista::leer(&r).encargos.len(), 1);
    }

    #[test]
    fn se_guardan_los_ultimos_acabados_y_todos_los_que_esperan() {
        let mut l = Lista::default();
        for i in 0..15 {
            let mut x = Registro::nuevo(format!("h{i}"), &enc("x"), i);
            x.estado = if i % 5 == 0 {
                Estado::EnCola
            } else {
                Estado::Hecha
            };
            l.encargos.push(x);
        }
        l.podar();
        assert_eq!(
            l.encargos
                .iter()
                .filter(|x| x.estado == Estado::EnCola)
                .count(),
            3
        );
        assert_eq!(
            l.encargos.iter().filter(|x| x.estado.acabado()).count(),
            ACABADOS
        );
        // Se van los mas viejos.
        assert!(!l.encargos.iter().any(|x| x.id == "h1"));
        assert!(l.encargos.iter().any(|x| x.id == "h14"));
    }

    #[test]
    fn el_texto_de_la_notificacion() {
        let mut r = Registro::nuevo(
            "e".into(),
            &enc("[img 01] apunta comprar yeso y cemento para la obra del lunes"),
            1,
        );
        r.estado = Estado::Hecha;
        r.resumen = resumen(
            "## Listo\n\n- **Apunté** «comprar yeso» en `Inbox`\nmas",
            true,
        );
        r.abrir = Some(Abrir {
            proyecto: "asis".into(),
            codigo: "IA-7".into(),
            lienzo: None,
        });
        let t = notificacion(&r);
        assert_eq!(
            t.titulo,
            "✅ Hecho: apunta comprar yeso y cemento para la obra del l…"
        );
        assert_eq!(t.texto, "Listo");
        assert_eq!(
            t.botones[0].pedido["que"],
            serde_json::json!({"tipo": "nota", "proyecto": "asis", "codigo": "IA-7"})
        );
        assert_eq!(t.al_pulsar.as_ref(), Some(&t.botones[0].pedido));
        let xml = notificacion::xml(&t);
        assert!(xml.contains("<action content=\"Abrir\""), "{xml}");

        // Con lienzo, «Abrir» lleva al lienzo.
        r.abrir.as_mut().unwrap().lienzo = Some("L1".into());
        assert_eq!(
            notificacion(&r).botones[0].pedido["que"],
            serde_json::json!({"tipo": "hoja", "proyecto": "asis", "referencia": "L1"})
        );

        // A medias: el aviso del fallo como linea.
        r.con_fallos = true;
        r.resumen = resumen("Apunté una.\n⚠️ no se pudo la segunda", false);
        let t = notificacion(&r);
        assert!(t.titulo.starts_with("⚠️ Hecho a medias"), "{}", t.titulo);
        assert_eq!(t.texto, "⚠️ no se pudo la segunda");

        // Fallo: el motivo y «Abrir» al chat «Asistente».
        let mut f = Registro::nuevo("f".into(), &enc("dibuja un gato"), 1);
        f.estado = Estado::Fallo;
        f.resumen = "Falta la clave de Claude".into();
        let t = notificacion(&f);
        assert_eq!(t.titulo, "❌ No se pudo: dibuja un gato");
        assert_eq!(t.texto, "Falta la clave de Claude");
        assert_eq!(
            t.botones[0].pedido,
            serde_json::json!({"pixpin": 1, "accion": "asistente"})
        );
        // Y el boton pasa el filtro del protocolo.
        assert!(notificacion::permitido(&t.botones[0].pedido));

        // Caso negativo: sin texto ni respuesta.
        assert_eq!(resumen("  \n ", true), "Hecho.");
        let mut v = Registro::nuevo("v".into(), &enc("[img 01]"), 1);
        v.estado = Estado::Hecha;
        assert_eq!(notificacion(&v).titulo, "✅ Hecho: la foto");
    }

    /// Lo que cuesta apuntar un encargo (lo que separa el Intro de Flow de
    /// verlo «en cola» en su lista).
    #[test]
    fn apuntar_es_rapido() {
        let r = raiz("rapido");
        let (c, _) = Cola::abrir(&r, 0);
        for i in 0..ACABADOS {
            let mut x = Registro::nuevo(format!("v{i}"), &enc("una respuesta vieja"), i as i64);
            x.estado = Estado::Hecha;
            x.resumen = "x".repeat(140);
            c.anadir(x);
        }
        // Cinco veces: la primera escritura en una carpeta nueva paga de mas
        // (el antivirus mira el fichero nuevo).
        let mut ms: Vec<f64> = (0..5)
            .map(|i| {
                let t = std::time::Instant::now();
                c.anadir(Registro::nuevo(format!("nuevo{i}"), &enc("apunta pan"), 99));
                t.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        println!("apuntar un encargo (con {ACABADOS} acabados), ms: {ms:.2?}");
        ms.sort_by(f64::total_cmp);
        assert!(ms[2] < 250.0, "mediana {} ms", ms[2]);
        assert!(Lista::leer(&r).encargos.iter().any(|x| x.id == "nuevo4"));
    }
}
