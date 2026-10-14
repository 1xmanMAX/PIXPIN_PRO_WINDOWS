//! **El ejecutor de las herramientas del asistente** (encargos 2 y 3).
//!
//! Corre en el hilo del turno. Lo que va al lienzo no se puede tocar desde
//! aqui (el lienzo es de la ventana y de su hilo): se convierte a Excalidraw
//! con `pixpin_ia::lienzo`, se comprueba que el lienzo lo lee, y se le pide
//! a la UI por la cola de [`super::al_lienzo`]; el turno espera la respuesta para
//! contarle al modelo lo que paso de verdad (si no hay donde dibujar, el
//! modelo lo sabe y lo dice). El OCR si corre aqui: tarda, y asi la ventana
//! sigue suelta.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use pixpin_ia::agente::Ejecutor;
use pixpin_ia::herramientas::Llamada;
use pixpin_ia::lienzo;

use super::turno::{Adjunto, Aviso};

/// Cuanto se espera a que la UI meta lo dibujado. Normalmente es una vuelta
/// del bucle del chat (decimas); si la ventana estuviera colgada, mejor
/// decirselo al modelo que dejar el turno parado para siempre.
pub const ESPERA_LIENZO: Duration = Duration::from_secs(60);

/// Cuanto texto del OCR se le devuelve al modelo. El contexto es de 8192
/// tokens; el texto entero va aparte al chat ([`Aviso::TextoLeido`]).
pub const OCR_PARA_EL_MODELO: usize = 4000;

pub struct EjecutorDelChat {
    tx: Sender<Aviso>,
    proyecto: String,
    adjuntos: Vec<Adjunto>,
    parar: Arc<AtomicBool>,
    espera: Duration,
    llamadas: u64,
    /// El lienzo del chat donde dibujar si no hay uno abierto (la mencion
    /// `@lienzo:`), por su referencia.
    destino: Option<String>,
}

impl EjecutorDelChat {
    pub fn nuevo(tx: Sender<Aviso>, proyecto: String, adjuntos: Vec<Adjunto>, parar: Arc<AtomicBool>) -> EjecutorDelChat {
        EjecutorDelChat {
            tx,
            proyecto,
            adjuntos,
            parar,
            espera: ESPERA_LIENZO,
            llamadas: 0,
            destino: None,
        }
    }

    /// Adonde va lo dibujado si no hay un lienzo abierto.
    pub fn hacia(mut self, destino: Option<String>) -> EjecutorDelChat {
        self.destino = destino;
        self
    }

    #[cfg(test)]
    pub fn con_espera(mut self, espera: Duration) -> EjecutorDelChat {
        self.espera = espera;
        self
    }

    /// Una semilla distinta en cada llamada, para que los ids de Excalidraw
    /// no choquen con los de otra respuesta ni con los de este mismo turno.
    fn semilla(&mut self) -> u64 {
        self.llamadas += 1;
        let ahora = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        ahora.wrapping_mul(31).wrapping_add(self.llamadas)
    }

    fn al_lienzo(&mut self, llamada: &Llamada) -> Result<String, String> {
        let semilla = self.semilla();
        let elementos = lienzo::elementos(llamada, (0.0, 0.0), semilla);
        if elementos.is_empty() {
            return Err("no salio ninguna figura".into());
        }
        let documento = lienzo::documento(elementos);
        // Que lo lea el lienzo de verdad ANTES de molestar a la ventana.
        pixpin_motor2d::excalidraw::leer(&documento).map_err(|e| {
            tracing::warn!(%e, "asistente: el lienzo no lee lo dibujado");
            "el lienzo no pudo leer las figuras".to_string()
        })?;
        let (responder, rx) = channel();
        super::al_lienzo::pedir(super::al_lienzo::Pedido::nuevo(
            self.proyecto.clone(),
            self.destino.clone(),
            documento,
            responder,
        ));
        let hasta = Instant::now() + self.espera;
        loop {
            if self.parar.load(Ordering::Relaxed) {
                return Err("el usuario lo paro".into());
            }
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(r) => return r,
                Err(RecvTimeoutError::Disconnected) => return Err("la ventana del chat no contesto".into()),
                Err(RecvTimeoutError::Timeout) if Instant::now() >= hasta => {
                    return Err("la ventana del chat no contesto a tiempo".into());
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }
}

impl Ejecutor for EjecutorDelChat {
    fn ejecutar(&mut self, llamada: &Llamada) -> Result<String, String> {
        match llamada {
            Llamada::Dibujar(_) | Llamada::Grafica(_) => self.al_lienzo(llamada),
            Llamada::Ocr(p) => {
                let adjunto = super::ocr::elegir(&self.adjuntos, p.archivo.as_deref())?.clone();
                let texto = super::ocr::leer(&adjunto.ruta, &p.paginas, &self.parar)?;
                let para_el_modelo = recortar(&texto, OCR_PARA_EL_MODELO);
                let _ = self.tx.send(Aviso::TextoLeido { de: adjunto, texto });
                Ok(para_el_modelo)
            }
        }
    }
}

/// Recorta a `n` letras diciendolo, para que el modelo no crea que el
/// documento acaba ahi.
pub fn recortar(texto: &str, n: usize) -> String {
    if texto.chars().count() <= n {
        return texto.to_string();
    }
    let mut s: String = texto.chars().take(n).collect();
    s.push_str("\n[...texto recortado: el usuario tiene el texto entero en el chat]");
    s
}
