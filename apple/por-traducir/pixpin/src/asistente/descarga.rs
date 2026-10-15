//! **Bajar el asistente desde la tarjeta del chat**, en un hilo, con
//! progreso y cancelar.
//!
//! La primera vez el chat «Asistente» ensena una tarjeta con lo que pesa
//! cada modelo y pide confirmacion: son ~2 GB (o ~0,8 GB el ligero) y nunca
//! se bajan sin que el usuario pulse. Aqui solo se lanza
//! `pixpin_ia::descarga::asegurar` en su hilo y se reenvia lo que diga.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use pixpin_ia::ErrorIa;
use pixpin_ia::descarga::{self, Progreso};
use pixpin_ia::modelos::{LLAMA_CPP, Modelo};

/// Lo que llega de la descarga.
#[derive(Debug, Clone, PartialEq)]
pub enum AvisoDescarga {
    Progreso(Progreso),
    /// Acabo bien.
    Lista,
    /// El usuario la cancelo. Lo que ya estaba entero y verificado (el
    /// motor, uno de los dos ficheros) no se vuelve a bajar.
    Cancelada,
    Fallo(String),
}

/// ¿Esta el asistente listo para este modelo (motor y pesos)?
pub fn listo(raiz: &Path, modelo: &Modelo) -> bool {
    descarga::runtime_listo(raiz, &LLAMA_CPP) && descarga::modelo_listo(raiz, modelo)
}

/// Lo que falta por bajar, en bytes (para la tarjeta: «1,9 GB»).
pub fn falta(raiz: &Path, modelo: &Modelo) -> u64 {
    let c = descarga::carpeta_modelo(raiz, modelo);
    let pesos: u64 = modelo
        .ficheros()
        .iter()
        .filter(|f| !descarga::fichero_listo(&c, f))
        .map(|f| f.bytes)
        .sum();
    let motor = if descarga::runtime_listo(raiz, &LLAMA_CPP) { 0 } else { LLAMA_CPP.zip.bytes };
    pesos + motor
}

/// «1,9 GB», «740 MB»: como lo dice la tarjeta.
pub fn tamano(bytes: u64) -> String {
    let mb = bytes as f64 / 1_000_000.0;
    if mb >= 1000.0 {
        format!("{:.1} GB", mb / 1000.0).replace('.', ",")
    } else {
        format!("{:.0} MB", mb.max(1.0))
    }
}

/// Una descarga en marcha.
pub struct Descarga {
    pub modelo: Modelo,
    cancelar: Arc<AtomicBool>,
    rx: Receiver<AvisoDescarga>,
    /// Lo ultimo que se supo, para pintar la barra.
    pub ultimo: Option<Progreso>,
    pub acabada: Option<AvisoDescarga>,
}

impl Descarga {
    /// La de verdad: a `raiz` (normalmente `%LOCALAPPDATA%\PixPinMax\modelos`).
    pub fn lanzar(raiz: PathBuf, modelo: Modelo) -> Descarga {
        Descarga::lanzar_con(modelo, move |avance, cancelado| {
            descarga::asegurar(&raiz, &modelo, &LLAMA_CPP, avance, cancelado)
        })
    }

    /// Con otro «bajador» (las pruebas no bajan 2 GB).
    pub fn lanzar_con<F>(modelo: Modelo, bajar: F) -> Descarga
    where
        F: FnOnce(&mut dyn FnMut(Progreso), &AtomicBool) -> Result<(), ErrorIa> + Send + 'static,
    {
        let (tx, rx) = channel();
        let cancelar = Arc::new(AtomicBool::new(false));
        let c = cancelar.clone();
        let lanzado = std::thread::Builder::new().name("asistente-descarga".into()).spawn({
            let tx = tx.clone();
            move || {
                let mut ultimo_aviso = std::time::Instant::now() - std::time::Duration::from_secs(1);
                let r = bajar(
                    &mut |p: Progreso| {
                        // Diez avisos por segundo bastan para una barra; el
                        // bajador avisa en cada bloque.
                        if p.verificando || ultimo_aviso.elapsed().as_millis() >= 100 {
                            ultimo_aviso = std::time::Instant::now();
                            let _ = tx.send(AvisoDescarga::Progreso(p));
                        }
                    },
                    &c,
                );
                let _ = tx.send(match r {
                    Ok(()) => AvisoDescarga::Lista,
                    Err(ErrorIa::Cancelado) => AvisoDescarga::Cancelada,
                    Err(_) if c.load(Ordering::Relaxed) => AvisoDescarga::Cancelada,
                    Err(e) => AvisoDescarga::Fallo(e.to_string()),
                });
            }
        });
        if let Err(e) = lanzado {
            let _ = tx.send(AvisoDescarga::Fallo(e.to_string()));
        }
        Descarga {
            modelo,
            cancelar,
            rx,
            ultimo: None,
            acabada: None,
        }
    }

    pub fn cancelar(&self) {
        self.cancelar.store(true, Ordering::Relaxed);
    }

    /// Recoge lo que haya llegado. Devuelve `true` si cambio algo.
    pub fn mirar(&mut self) -> bool {
        let mut cambio = false;
        loop {
            match self.rx.try_recv() {
                Ok(AvisoDescarga::Progreso(p)) => {
                    self.ultimo = Some(p);
                    cambio = true;
                }
                Ok(fin) => {
                    self.acabada = Some(fin);
                    cambio = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if self.acabada.is_none() {
                        self.acabada = Some(AvisoDescarga::Fallo("la descarga se detuvo de golpe".into()));
                        cambio = true;
                    }
                    break;
                }
            }
        }
        cambio
    }

    pub fn fraccion(&self) -> f32 {
        self.ultimo.as_ref().map(Progreso::fraccion).unwrap_or(0.0)
    }
}
