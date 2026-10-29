//! **La escucha en vivo de los temas de la reunion** (28-oct-2026, el
//! usuario: «durante la reunion el telefono (o el PC) se queda escuchando y
//! transcribiendo; eso se manda a JEV y Jev marca que temas ya se
//! trataron»; en vivo de verdad el 29-oct-2026: «en cuanto termino de
//! decir un tema, que se marque»).
//!
//! Con temas en la agenda, al abrirse el audio de la grabacion:
//!
//! 1. **El oido** (hilo `temas-oido`): una suscripcion al PCM de la
//!    grabacion (`Grabadora::suscribir_pcm`; **no se abre otro
//!    microfono**). El hilo de la mezcla nunca espera: si este va tarde, la
//!    suscripcion le tira bloques solo a el. Corta **frases** en cuanto se
//!    calla quien habla (`pixpin_voz::en_vivo::Frasero`) y las deja en una
//!    cola de tres que **tira la mas vieja** si el texto va por detras.
//! 2. **El texto** (hilo `temas-texto`, por debajo de lo normal): el
//!    reconocedor de Windows si sabe el idioma (`sapi::DeFrases`: una
//!    frase de 3 s en unas decimas, sin red), y si no, o si falla, Whisper
//!    *base* con uno o dos hilos (`Whisper::cargar_ligero`, cargado a la
//!    primera frase; trozos de 5 s o mas). Se salta lo que no tiene voz,
//!    todo mientras el procesador pase del 85 % y, si una frase tardo mas
//!    del 80 % de lo que dura, la siguiente (`en_vivo::Freno`). Cada
//!    frase va al rollo de la conversacion (`pixpin_typesafe::temas::Rollo`).
//! 3. **Jev** (hilo `temas-jev`): en cuanto hay texto nuevo, **una**
//!    peticion con los ultimos ~40 s (lo nuevo marcado como
//!    `conversacion.ahora`) y los temas pendientes; como mucho una en vuelo
//!    y 1,5 s entre una y otra (`temas::Ritmo`: lo que llega mientras tanto
//!    va junto en la siguiente). La tabla (`temas::Seguimiento`) decide que
//!    se trato. Cada ✓ lleva cuanto tardo desde que se dejo de hablar (al
//!    registro y, en las compilaciones de depuracion, a la cabecera de la
//!    lista).
//! 4. **La barra** recoge lo marcado ([`Escucha::recoger`]) y lo mezcla
//!    con lo hecho a mano (`pixpin_proyecto::temas`: lo hecho a mano
//!    manda); cuando la persona marca uno, la escucha deja de preguntar por
//!    el ([`Escucha::pendientes`]).
//!
//! La grabacion nunca depende de esto: si algo falla, se dice en la barra
//! y los temas se marcan a mano. Al terminar, la escucha se para en el acto
//! (Whisper se cancela) y la transcripcion entera de despues es la de
//! siempre.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use pixpin_audio::{Receptor, SinBloque};
use pixpin_store::{Idioma, Ubicacion};
use pixpin_typesafe::temas::{
    self as jt, Cuando, Dicho, Pendiente, Ritmo, Rollo, Seguimiento, Tratado,
};
use pixpin_voz::en_vivo::{self, COLA, Cola, Frase, Frasero, Freno, Paso};

use crate::jev::temas::SinJev;

/// Bloques de PCM que esperan al oido (cada uno, 10-16 ms): unos 4 s.
const CAPACIDAD: usize = 256;
/// Lo que se mide el procesador antes de cada frase (lo de ahora, no la
/// media desde la anterior, que incluiria el texto de la anterior).
const MEDIR_CPU: Duration = Duration::from_millis(100);
/// Cuantas frases seguidas puede fallar el reconocedor de Windows antes de
/// pasar a Whisper.
const FALLOS_WINDOWS: u32 = 3;

/// Lo que la escucha le cuenta a la barra.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Vuelta {
    /// Jev marco un tema como tratado, y lo que tardo desde que se dejo de
    /// hablar.
    Tratado(Tratado, Duration),
    /// Como va la escucha (la pista de la barra).
    Aviso(Aviso),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Aviso {
    /// En marcha.
    Escuchando,
    /// El modelo de voz no se pudo cargar.
    SinModelo,
    /// Jev no contesto (sin red, la clave): eso no marca, y se reintenta.
    JevFalla,
    /// El procesador va cargado: se salto una frase.
    Cargado,
}

/// Por que no se escucha (los temas, a mano).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NoSeEscucha {
    /// Sin clave de TypeSafe o con la funcion apagada.
    Jev(SinJev),
    /// Ni el reconocedor de Windows ni Whisper (sin el modelo bajado o
    /// sin ONNX Runtime).
    Modelo,
    /// La grabadora no dio PCM (ya se estaba cerrando).
    Pcm,
}

/// Lo que comparten el hilo del texto y el de Jev.
struct Mesa {
    rollo: Rollo,
    ritmo: Ritmo,
    /// Los temas por los que se pregunta (los pendientes que nadie toco).
    pendientes: Vec<Pendiente>,
}

/// Lo que comparten la barra y los tres hilos.
struct Compartido {
    /// Las frases que esperan (con cuando se dejo de hablar en ellas, en el
    /// reloj de la escucha), y si el oido ya acabo.
    cola: Mutex<(Cola<(Frase, u64)>, bool)>,
    hay: Condvar,
    mesa: Mutex<Mesa>,
    /// Se le avisa al hilo de Jev: texto nuevo, pendientes nuevos, parar.
    para_jev: Condvar,
    /// El oido corta con la medida de Whisper (paso de Windows a Whisper).
    para_whisper: AtomicBool,
    /// Acabo el texto (no llegara nada mas).
    sin_texto: AtomicBool,
    parar: AtomicBool,
    /// El reloj de la escucha: `Dicho::reloj_fin` y el `Ritmo` cuentan
    /// desde aqui.
    t0: Instant,
}

impl Compartido {
    fn reloj(&self) -> u64 {
        self.t0.elapsed().as_millis() as u64
    }
}

/// Con que se pasa a texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Motor {
    Windows,
    Whisper,
}

/// **La escucha en marcha.** Soltarla la para.
pub(super) struct Escucha {
    c: Arc<Compartido>,
    vueltas: Receiver<Vuelta>,
}

impl Escucha {
    /// **Empieza a escuchar** la grabacion `g`. `reunion` es el titulo,
    /// `agenda` todos los temas y `pendientes` los que Jev tiene que mirar.
    pub(super) fn empezar(
        g: &pixpin_audio::Grabadora,
        u: &Ubicacion,
        idioma: Idioma,
        reunion: String,
        agenda: Vec<String>,
        pendientes: Vec<Pendiente>,
    ) -> Result<Escucha, NoSeEscucha> {
        let cliente = crate::jev::temas::cliente(u).map_err(NoSeEscucha::Jev)?;
        let raiz = u.raiz().to_path_buf();
        let hay_whisper =
            crate::voz::whisper_posible(u, idioma) && pixpin_voz::whisper::modelo_listo(&raiz);
        let motor = if crate::voz::windows_sabe(idioma) {
            Motor::Windows
        } else if hay_whisper {
            Motor::Whisper
        } else {
            return Err(NoSeEscucha::Modelo);
        };
        let receptor = g.suscribir_pcm(CAPACIDAD).ok_or(NoSeEscucha::Pcm)?;
        let c = Arc::new(Compartido {
            cola: Mutex::new((Cola::nueva(COLA), false)),
            hay: Condvar::new(),
            mesa: Mutex::new(Mesa {
                rollo: Rollo::default(),
                ritmo: Ritmo::default(),
                pendientes,
            }),
            para_jev: Condvar::new(),
            para_whisper: AtomicBool::new(motor == Motor::Whisper),
            sin_texto: AtomicBool::new(false),
            parar: AtomicBool::new(false),
            t0: Instant::now(),
        });
        let (tx, vueltas) = channel();
        let parar = |c: &Compartido| {
            c.parar.store(true, Ordering::SeqCst);
            c.hay.notify_all();
            c.para_jev.notify_all();
        };
        let oido = {
            let c = Arc::clone(&c);
            std::thread::Builder::new()
                .name("temas-oido".into())
                .spawn(move || oir(&receptor, &c))
        };
        if oido.is_err() {
            return Err(NoSeEscucha::Pcm);
        }
        let voz = crate::voz::idioma_de_voz(idioma).to_string();
        let texto = {
            let c = Arc::clone(&c);
            let tx = tx.clone();
            std::thread::Builder::new()
                .name("temas-texto".into())
                .spawn(move || {
                    pasar_a_texto(&c, &tx, &raiz, &voz, motor, hay_whisper);
                    c.sin_texto.store(true, Ordering::SeqCst);
                    c.para_jev.notify_all();
                })
        };
        if texto.is_err() {
            parar(&c);
            return Err(NoSeEscucha::Pcm);
        }
        let jev = {
            let c = Arc::clone(&c);
            std::thread::Builder::new()
                .name("temas-jev".into())
                .spawn(move || preguntar_a_jev(&c, &tx, &cliente, &reunion, &agenda))
        };
        if jev.is_err() {
            parar(&c);
            return Err(NoSeEscucha::Pcm);
        }
        tracing::info!(?motor, "temas: escuchando en vivo");
        Ok(Escucha { c, vueltas })
    }

    /// Los temas por los que se pregunta desde ahora.
    pub(super) fn pendientes(&self, v: Vec<Pendiente>) {
        self.c
            .mesa
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pendientes = v;
        self.c.para_jev.notify_all();
    }

    /// Lo que llego desde la ultima vez.
    pub(super) fn recoger(&self) -> Vec<Vuelta> {
        self.vueltas.try_iter().collect()
    }
}

impl Drop for Escucha {
    fn drop(&mut self) {
        self.c.parar.store(true, Ordering::SeqCst);
        self.c.hay.notify_all();
        self.c.para_jev.notify_all();
    }
}

/// **El oido**: del PCM a frases, sin hacer nada pesado (el hilo de la
/// mezcla no espera, pero la cola de este se llenaria).
fn oir(receptor: &Receptor, c: &Compartido) {
    let medida = |whisper: bool| {
        if whisper {
            en_vivo::PARA_WHISPER
        } else {
            en_vivo::PARA_WINDOWS
        }
    };
    let mut whisper = c.para_whisper.load(Ordering::SeqCst);
    let mut f = Frasero::nuevo(medida(whisper));
    let meter = |x: Frase| {
        // Sale en cuanto se corta, con su silencio del final ya oido: se
        // dejo de hablar ese silencio antes.
        let fin = c
            .reloj()
            .saturating_sub(x.callado_al_final_ms.max(0) as u64);
        let mut g = c.cola.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((tirada, _)) = g.0.meter((x, fin)) {
            tracing::debug!(n = tirada.n, "temas: el texto va tarde; se tira una frase");
        }
        drop(g);
        c.hay.notify_all();
    };
    loop {
        if c.parar.load(Ordering::SeqCst) {
            break;
        }
        if whisper != c.para_whisper.load(Ordering::SeqCst) {
            whisper = !whisper;
            f.poner_medida(medida(whisper));
        }
        match receptor.recibir_hasta(Duration::from_millis(200)) {
            Ok(b) => {
                for x in f.meter(b.posicion, b.muestreo, b.canales, &b.muestras) {
                    meter(x);
                }
                receptor.devolver(b);
            }
            Err(SinBloque::Vacio) => {}
            Err(SinBloque::Cerrado) => {
                if let Some(x) = f.acabar() {
                    meter(x);
                }
                break;
            }
        }
    }
    if receptor.perdidos() > 0 {
        tracing::debug!(
            perdidos = receptor.perdidos(),
            "temas: bloques que no llegaron a tiempo"
        );
    }
    c.cola.lock().unwrap_or_else(|e| e.into_inner()).1 = true;
    c.hay.notify_all();
}

/// La siguiente frase, esperando, con cuando se dejo de hablar en ella (el
/// reloj de la escucha); `None` si hay que acabar.
fn siguiente(c: &Compartido) -> Option<(Frase, u64)> {
    let mut g = c.cola.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if c.parar.load(Ordering::SeqCst) {
            return None;
        }
        if let Some(x) = g.0.sacar() {
            return Some(x);
        }
        if g.1 {
            return None;
        }
        g = c
            .hay
            .wait_timeout(g, Duration::from_millis(500))
            .map(|(g, _)| g)
            .unwrap_or_else(|e| e.into_inner().0);
    }
}

/// El reconocedor de cada motor, cargado cuando hace falta.
enum Reconocedor {
    /// Aun nada.
    Nada,
    Windows(pixpin_voz::sapi::DeFrases),
    Whisper(Box<pixpin_voz::whisper::Whisper>),
}

/// **El texto**, frase a frase, al rollo de la conversacion.
fn pasar_a_texto(
    c: &Compartido,
    tx: &Sender<Vuelta>,
    raiz: &std::path::Path,
    idioma: &str,
    mut motor: Motor,
    hay_whisper: bool,
) {
    if !pixpin_shell::prioridad::hilo_por_debajo_de_lo_normal() {
        tracing::debug!("temas: el hilo del texto sigue a prioridad normal");
    }
    // El reconocedor de Windows es COM.
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let _ = tx.send(Vuelta::Aviso(Aviso::Escuchando));
    let mut rec = Reconocedor::Nada;
    let mut freno = Freno::default();
    let mut cpu = pixpin_shell::prioridad::MedidorCpu::default();
    let mut fallos_windows = 0u32;
    while let Some((frase, reloj_fin)) = siguiente(c) {
        let _ = cpu.medir();
        std::thread::sleep(MEDIR_CPU);
        match freno.decidir(&frase, cpu.medir()) {
            Paso::Pasar => {}
            Paso::SaltarCpu => {
                let _ = tx.send(Vuelta::Aviso(Aviso::Cargado));
                continue;
            }
            Paso::SaltarSilencio | Paso::SaltarRetraso => continue,
        }
        if c.mesa
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pendientes
            .is_empty()
        {
            continue;
        }
        let empezo = Instant::now();
        let texto = match motor {
            Motor::Windows => {
                let r = match &mut rec {
                    Reconocedor::Windows(r) => r,
                    otro => {
                        *otro = Reconocedor::Windows(pixpin_voz::sapi::DeFrases::nuevo(idioma));
                        let Reconocedor::Windows(r) = otro else {
                            return;
                        };
                        r
                    }
                };
                match r.pasar(&frase.muestras, &c.parar) {
                    Ok(t) => {
                        fallos_windows = 0;
                        t
                    }
                    Err(pixpin_voz::ErrorVoz::Cancelada) => return,
                    Err(e) => {
                        fallos_windows += 1;
                        tracing::debug!(?e, n = frase.n, "temas: Windows no paso una frase");
                        if fallos_windows >= FALLOS_WINDOWS {
                            if !hay_whisper {
                                tracing::warn!(?e, "temas: sin reconocedor no se escucha en vivo");
                                let _ = tx.send(Vuelta::Aviso(Aviso::SinModelo));
                                return;
                            }
                            tracing::info!("temas: el reconocedor de Windows falla; a Whisper");
                            motor = Motor::Whisper;
                            rec = Reconocedor::Nada;
                            c.para_whisper.store(true, Ordering::SeqCst);
                        }
                        String::new()
                    }
                }
            }
            Motor::Whisper => {
                if !matches!(rec, Reconocedor::Whisper(_)) {
                    let exe = pixpin_shell::entorno::directorio_del_ejecutable().ok();
                    match pixpin_voz::whisper::Whisper::cargar_ligero(raiz, exe.as_deref()) {
                        Ok(w) => rec = Reconocedor::Whisper(Box::new(w)),
                        Err(e) => {
                            tracing::warn!(?e, "temas: sin Whisper no se escucha en vivo");
                            let _ = tx.send(Vuelta::Aviso(Aviso::SinModelo));
                            return;
                        }
                    }
                }
                let Reconocedor::Whisper(w) = &rec else {
                    return;
                };
                match w.transcribir_pcm(&frase.muestras, idioma, &mut |_| {}, &c.parar) {
                    Ok(t) => t
                        .segmentos
                        .iter()
                        .map(|s| s.texto.trim())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" "),
                    Err(pixpin_voz::ErrorVoz::Cancelada) => return,
                    Err(e) => {
                        tracing::debug!(?e, n = frase.n, "temas: una frase sin texto");
                        String::new()
                    }
                }
            }
        };
        freno.tardo(frase.duracion(), empezo.elapsed());
        tracing::debug!(
            n = frase.n,
            ?motor,
            ms = empezo.elapsed().as_millis() as u64,
            palabras = texto.split_whitespace().count(),
            "temas: frase pasada a texto"
        );
        if texto.trim().is_empty() {
            continue;
        }
        c.mesa
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .rollo
            .meter(Dicho {
                ms: frase.ms,
                fin_ms: frase.fin_voz_ms,
                texto,
                reloj_fin,
            });
        c.para_jev.notify_all();
    }
}

/// **Jev**: en cuanto hay texto nuevo y le toca (`Ritmo`), una peticion.
fn preguntar_a_jev(
    c: &Compartido,
    tx: &Sender<Vuelta>,
    cliente: &pixpin_typesafe::Cliente,
    reunion: &str,
    agenda: &[String],
) {
    let mut seguimiento = Seguimiento::default();
    let mut fallo_dicho = false;
    let mut g = c.mesa.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if c.parar.load(Ordering::SeqCst) {
            return;
        }
        let ahora = c.reloj();
        let hay = g.rollo.hay_nuevo() && !g.pendientes.is_empty();
        let esperar = match g.ritmo.cuando(ahora, hay) {
            Cuando::Ya => None,
            Cuando::Esperar(ms) => Some(Duration::from_millis(ms)),
            // Sin nada que preguntar y sin texto que vaya a llegar: se acabo.
            Cuando::Nada if c.sin_texto.load(Ordering::SeqCst) => return,
            Cuando::Nada => Some(Duration::from_millis(500)),
        };
        if let Some(d) = esperar {
            g = c
                .para_jev
                .wait_timeout(g, d)
                .map(|(g, _)| g)
                .unwrap_or_else(|e| e.into_inner().0);
            continue;
        }
        let Some(v) = g.rollo.ventana() else {
            continue;
        };
        let pendientes = g.pendientes.clone();
        g.ritmo.sale(ahora);
        drop(g);
        // Sin el candado: lo que llegue mientras tanto se junta para la
        // siguiente.
        let empezo = Instant::now();
        let r = jt::preguntar(cliente, reunion, agenda, &v, &pendientes);
        let jev_ms = empezo.elapsed().as_millis() as u64;
        g = c.mesa.lock().unwrap_or_else(|e| e.into_inner());
        g.ritmo.vuelve(c.reloj(), r.is_ok());
        match r {
            Ok((senales, _)) => {
                g.rollo.mirado(&v);
                if fallo_dicho {
                    fallo_dicho = false;
                    let _ = tx.send(Vuelta::Aviso(Aviso::Escuchando));
                }
                // Solo lo que aun sigue pendiente (alguien pudo marcarlo a
                // mano mientras Jev pensaba).
                let siguen: Vec<_> = senales
                    .into_iter()
                    .filter(|(id, _)| g.pendientes.iter().any(|p| p.id == *id))
                    .collect();
                for t in seguimiento.decidir(&v, &siguen) {
                    let tarde = Duration::from_millis(c.reloj().saturating_sub(v.reloj_ultima));
                    let tarde_primera =
                        Duration::from_millis(c.reloj().saturating_sub(v.reloj_primera));
                    tracing::info!(
                        id = %t.id,
                        ms = t.ms,
                        p = t.p,
                        desde_que_se_callo_ms = tarde.as_millis() as u64,
                        desde_la_primera_frase_ms = tarde_primera.as_millis() as u64,
                        jev_ms,
                        "temas: Jev marca un tema tratado"
                    );
                    g.pendientes.retain(|p| p.id != t.id);
                    let _ = tx.send(Vuelta::Tratado(t, tarde));
                }
                tracing::debug!(jev_ms, "temas: Jev contesto");
            }
            // Un fallo no es un «no se trato»: no marca, y lo nuevo va en la
            // siguiente.
            Err(e) => {
                tracing::warn!(jev_ms, "temas: Jev no contesto: {e}");
                if !fallo_dicho {
                    fallo_dicho = true;
                    let _ = tx.send(Vuelta::Aviso(Aviso::JevFalla));
                }
            }
        }
    }
}
