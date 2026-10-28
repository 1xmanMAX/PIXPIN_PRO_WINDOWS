//! **La escucha en vivo de los temas de la reunion** (28-oct-2026, el
//! usuario: «durante la reunion el telefono (o el PC) se queda escuchando y
//! transcribiendo; eso se manda a JEV y Jev marca que temas ya se
//! trataron»).
//!
//! Con temas en la agenda, al abrirse el audio de la grabacion:
//!
//! 1. **El oido** (hilo `temas-oido`): una suscripcion al PCM de la
//!    grabacion (`Grabadora::suscribir_pcm`; **no se abre otro
//!    microfono**). El hilo de la mezcla nunca espera: si este va tarde, la
//!    suscripcion le tira bloques solo a el. Junta tramos de unos 15 s
//!    (`pixpin_voz::en_vivo::Ventanero`) y los deja en una cola de dos que
//!    **tira el mas viejo** si el texto va por detras.
//! 2. **El texto** (hilo `temas-texto`, por debajo de lo normal): Whisper
//!    *base* con uno o dos hilos (`Whisper::cargar_ligero`, cargado al
//!    primer tramo con voz), y **se salta** lo que no tiene voz, todo
//!    mientras el procesador pase del 85 % y, si un tramo tardo mas del 80 %
//!    de lo que dura, el siguiente (`en_vivo::Freno`). Luego **una**
//!    peticion a Jev por tramo con el anterior de contexto
//!    (`pixpin_typesafe::temas`), y la tabla decide que temas se trataron.
//! 3. **La barra** recoge lo marcado ([`Escucha::recoger`]) y lo mezcla
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
use pixpin_typesafe::temas::{self as jt, Pendiente, Seguimiento, Tratado};
use pixpin_voz::en_vivo::{COLA, Cola, Freno, Paso, Tramo, Ventanero};

use crate::jev::temas::SinJev;

/// Bloques de PCM que esperan al oido (cada uno, 10-16 ms): unos 4 s.
const CAPACIDAD: usize = 256;

/// Lo que la escucha le cuenta a la barra.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Vuelta {
    /// Jev marco un tema como tratado.
    Tratado(Tratado),
    /// Como va la escucha (la pista de la barra).
    Aviso(Aviso),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Aviso {
    /// En marcha.
    Escuchando,
    /// El modelo de voz no se pudo cargar.
    SinModelo,
    /// Jev no contesto a un tramo (sin red, la clave): ese tramo no marca.
    JevFalla,
    /// El procesador va cargado: se salto un tramo.
    Cargado,
}

/// Por que no se escucha (los temas, a mano).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NoSeEscucha {
    /// Sin clave de TypeSafe o con la funcion apagada.
    Jev(SinJev),
    /// Sin el modelo de Whisper bajado o sin ONNX Runtime.
    Modelo,
    /// La grabadora no dio PCM (ya se estaba cerrando).
    Pcm,
}

/// Lo que comparten la barra y los dos hilos.
struct Compartido {
    /// Los tramos que esperan, y si el oido ya acabo.
    cola: Mutex<(Cola<Tramo>, bool)>,
    hay: Condvar,
    /// Los temas por los que se pregunta (los pendientes que nadie toco).
    pendientes: Mutex<Vec<Pendiente>>,
    parar: AtomicBool,
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
        if !crate::voz::whisper_posible(u, idioma) || !pixpin_voz::whisper::modelo_listo(&raiz) {
            return Err(NoSeEscucha::Modelo);
        }
        let receptor = g.suscribir_pcm(CAPACIDAD).ok_or(NoSeEscucha::Pcm)?;
        let c = Arc::new(Compartido {
            cola: Mutex::new((Cola::nueva(COLA), false)),
            hay: Condvar::new(),
            pendientes: Mutex::new(pendientes),
            parar: AtomicBool::new(false),
        });
        let (tx, vueltas) = channel();
        let oido = {
            let c = Arc::clone(&c);
            std::thread::Builder::new()
                .name("temas-oido".into())
                .spawn(move || oir(&receptor, &c))
        };
        if oido.is_err() {
            return Err(NoSeEscucha::Pcm);
        }
        let idioma = crate::voz::idioma_de_voz(idioma).to_string();
        let texto = {
            let c = Arc::clone(&c);
            std::thread::Builder::new()
                .name("temas-texto".into())
                .spawn(move || {
                    pasar_a_texto(&c, &tx, &raiz, &idioma, &cliente, &reunion, &agenda);
                })
        };
        if texto.is_err() {
            c.parar.store(true, Ordering::SeqCst);
            return Err(NoSeEscucha::Pcm);
        }
        tracing::info!("temas: escuchando en vivo");
        Ok(Escucha { c, vueltas })
    }

    /// Los temas por los que se pregunta desde ahora.
    pub(super) fn pendientes(&self, v: Vec<Pendiente>) {
        *self.c.pendientes.lock().unwrap_or_else(|e| e.into_inner()) = v;
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
    }
}

/// **El oido**: del PCM a tramos, sin hacer nada pesado (el hilo de la
/// mezcla no espera, pero la cola de este se llenaria).
fn oir(receptor: &Receptor, c: &Compartido) {
    let mut v = Ventanero::nuevo();
    let meter = |t: Tramo| {
        let mut g = c.cola.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(tirado) = g.0.meter(t) {
            tracing::debug!(n = tirado.n, "temas: el texto va tarde; se tira un tramo");
        }
        drop(g);
        c.hay.notify_all();
    };
    loop {
        if c.parar.load(Ordering::SeqCst) {
            break;
        }
        match receptor.recibir_hasta(Duration::from_millis(200)) {
            Ok(b) => {
                for t in v.meter(b.posicion, b.muestreo, b.canales, &b.muestras) {
                    meter(t);
                }
                receptor.devolver(b);
            }
            Err(SinBloque::Vacio) => {}
            Err(SinBloque::Cerrado) => {
                if let Some(t) = v.acabar() {
                    meter(t);
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

/// El siguiente tramo, esperando; `None` si hay que acabar.
fn siguiente(c: &Compartido) -> Option<Tramo> {
    let mut g = c.cola.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if c.parar.load(Ordering::SeqCst) {
            return None;
        }
        if let Some(t) = g.0.sacar() {
            return Some(t);
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

/// **El texto y Jev**, tramo a tramo.
fn pasar_a_texto(
    c: &Compartido,
    tx: &Sender<Vuelta>,
    raiz: &std::path::Path,
    idioma: &str,
    cliente: &pixpin_typesafe::Cliente,
    reunion: &str,
    agenda: &[String],
) {
    if !pixpin_shell::prioridad::hilo_por_debajo_de_lo_normal() {
        tracing::debug!("temas: el hilo del texto sigue a prioridad normal");
    }
    let _ = tx.send(Vuelta::Aviso(Aviso::Escuchando));
    let mut whisper: Option<pixpin_voz::whisper::Whisper> = None;
    let mut freno = Freno::default();
    let mut cpu = pixpin_shell::prioridad::MedidorCpu::default();
    let mut seguimiento = Seguimiento::default();
    let mut antes: Option<String> = None;
    let mut fallo_dicho = false;
    while let Some(tramo) = siguiente(c) {
        // La carga de ahora mismo (un tercio de segundo), no la media de
        // los 15 s del tramo: esa incluiria el texto del tramo anterior.
        let _ = cpu.medir();
        std::thread::sleep(Duration::from_millis(300));
        match freno.decidir(&tramo, cpu.medir()) {
            Paso::Pasar => {}
            Paso::SaltarCpu => {
                let _ = tx.send(Vuelta::Aviso(Aviso::Cargado));
                continue;
            }
            Paso::SaltarSilencio | Paso::SaltarRetraso => continue,
        }
        let pendientes = c
            .pendientes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if pendientes.is_empty() {
            continue;
        }
        if whisper.is_none() {
            let exe = pixpin_shell::entorno::directorio_del_ejecutable().ok();
            match pixpin_voz::whisper::Whisper::cargar_ligero(raiz, exe.as_deref()) {
                Ok(w) => whisper = Some(w),
                Err(e) => {
                    tracing::warn!(?e, "temas: sin Whisper no se escucha en vivo");
                    let _ = tx.send(Vuelta::Aviso(Aviso::SinModelo));
                    return;
                }
            }
        }
        let Some(w) = whisper.as_ref() else { return };
        let empezo = Instant::now();
        let texto = match w.transcribir_pcm(&tramo.muestras, idioma, &mut |_| {}, &c.parar) {
            Ok(t) => t
                .segmentos
                .iter()
                .map(|s| s.texto.trim())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
            Err(pixpin_voz::ErrorVoz::Cancelada) => return,
            Err(e) => {
                tracing::debug!(?e, n = tramo.n, "temas: un tramo sin texto");
                String::new()
            }
        };
        freno.tardo(tramo.duracion(), empezo.elapsed());
        tracing::debug!(
            n = tramo.n,
            ms = empezo.elapsed().as_millis() as u64,
            palabras = texto.split_whitespace().count(),
            "temas: tramo pasado a texto"
        );
        if !jt::se_mira(&texto, &pendientes) {
            antes = Some(texto).filter(|t| !t.is_empty());
            continue;
        }
        match jt::preguntar(
            cliente,
            reunion,
            agenda,
            antes.as_deref(),
            &texto,
            &pendientes,
        ) {
            Ok((senales, _)) => {
                if fallo_dicho {
                    fallo_dicho = false;
                    let _ = tx.send(Vuelta::Aviso(Aviso::Escuchando));
                }
                for t in seguimiento.decidir(tramo.n, tramo.ms, &senales) {
                    tracing::info!(id = %t.id, ms = t.ms, p = t.p, "temas: Jev marca un tema tratado");
                    let _ = tx.send(Vuelta::Tratado(t));
                }
            }
            // Un fallo no es un «no se trato»: ese tramo no marca nada.
            Err(e) => {
                tracing::warn!("temas: Jev no contesto un tramo: {e}");
                if !fallo_dicho {
                    fallo_dicho = true;
                    let _ = tx.send(Vuelta::Aviso(Aviso::JevFalla));
                }
            }
        }
        antes = Some(texto);
    }
}
