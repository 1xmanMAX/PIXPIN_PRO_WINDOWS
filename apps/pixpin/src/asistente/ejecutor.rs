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
    /// Enganches de las acciones de la app (ver `acciones.rs`).
    confirmar: super::acciones::Confirmar,
    mandar: super::acciones::Mandar,
    /// Si contesta un modelo de la nube (lo encontrado en los documentos
    /// saldria a internet: ver `documentos::permiso_nube`).
    nube: bool,
    /// Lo citado de los documentos en este turno, numerado seguido.
    citas: pixpin_ia::documentos::Citas,
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
            confirmar: super::acciones::decir_que_no(),
            mandar: super::acciones::mandar_a_pixpin(),
            nube: false,
            citas: Default::default(),
        }
    }

    /// Adonde va lo dibujado si no hay un lienzo abierto.
    pub fn hacia(mut self, destino: Option<String>) -> EjecutorDelChat {
        self.destino = destino;
        self
    }

    /// Si el motor es de la nube.
    pub fn en_la_nube(mut self, nube: bool) -> EjecutorDelChat {
        self.nube = nube;
        self
    }

    /// **Busca en los documentos del usuario** desde este chat. Con la nube,
    /// solo si el usuario lo permite.
    fn buscar(&mut self, p: &pixpin_ia::documentos::PeticionBusqueda) -> Result<String, String> {
        if self.nube && !super::documentos::permiso_nube(&self.confirmar) {
            return Err(super::documentos::SIN_PERMISO_NUBE.into());
        }
        let (hallazgos, aviso) = super::documentos::buscar(p, &self.proyecto)?;
        let texto = self.citas.anadir(&p.consulta, hallazgos, aviso.as_deref());
        if !self.citas.lista.is_empty() {
            let _ = self.tx.send(Aviso::Citas(self.citas.lista.clone()));
        }
        Ok(texto)
    }

    /// Cambia como se pregunta antes de lo que borra y adonde van los
    /// pedidos (la UI, o las pruebas para no tocar la PixPin del usuario).
    pub fn con_acciones(
        mut self,
        confirmar: super::acciones::Confirmar,
        mandar: super::acciones::Mandar,
    ) -> EjecutorDelChat {
        self.confirmar = confirmar;
        self.mandar = mandar;
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
        self.al_lienzo_elementos(elementos)
    }

    fn al_lienzo_elementos(&mut self, elementos: Vec<serde_json::Value>) -> Result<String, String> {
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
            Llamada::Calcar(p) => {
                let ruta = super::acciones::imagen_para_calcar(&self.adjuntos, p.imagen.as_deref())?
                    .ruta
                    .clone();
                let semilla = self.semilla();
                let elementos = super::acciones::calcar_fichero(&ruta, p, semilla)?;
                self.al_lienzo_elementos(elementos)
            }
            Llamada::App(a) => super::acciones::hacer(a, &self.confirmar, &self.mandar),
            Llamada::Ayuda(pregunta) => Ok(pixpin_ia::conocimiento::responder(pregunta)),
            Llamada::Buscar(p) => self.buscar(p),
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
