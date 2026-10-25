//! **Las capturas, fuera del hilo de la barra** (25-oct-2026).
//!
//! Antes «Captura» hacia la foto y el PNG en el mismo hilo que pinta la
//! barra: con una zona grande, en el equipo de referencia (un Core i3 de
//! tercera generacion), la barra se quedaba congelada medio segundo en
//! mitad de la reunion. Ahora la barra encarga la foto y sigue; la foto
//! vuelve por un canal con el minuto en que se pidio.
//!
//! Un solo hilo con su propio dispositivo de captura, que nace con la
//! primera foto y muere cuando se suelta el [`Fotografo`]: dos fotos
//! seguidas salen en orden y no se pelean por la GPU.

use std::sync::mpsc::{Receiver, Sender, channel};

use anyhow::Result;
use pixpin_codec::imagen::ImagenRgba;
use pixpin_geom::Rect;

/// Lo que se le pide al hilo de las fotos.
enum Encargo {
    /// Fotografiar `zona` ahora (la barra no sale: esta excluida).
    Zona(i64, Rect),
    /// Una foto ya hecha (la primera, al elegir la zona) que solo falta
    /// pasar a PNG.
    Imagen(i64, ImagenRgba),
}

/// Una foto hecha: el milisegundo de la nota y el PNG, o por que no salio.
pub(super) type Hecha = (i64, Result<Vec<u8>>);

#[derive(Default)]
pub(super) struct Fotografo {
    canal: Option<(Sender<Encargo>, Receiver<Hecha>)>,
    /// Encargos que aun no han vuelto.
    pendientes: usize,
}

impl Fotografo {
    /// Encarga la foto de `zona` en el milisegundo `ms`.
    pub(super) fn zona(&mut self, ms: i64, zona: Rect) -> bool {
        self.encargar(Encargo::Zona(ms, zona))
    }

    /// Encarga pasar a PNG una foto ya hecha.
    pub(super) fn imagen(&mut self, ms: i64, imagen: ImagenRgba) -> bool {
        self.encargar(Encargo::Imagen(ms, imagen))
    }

    /// Cuantas fotos faltan por volver.
    pub(super) fn pendientes(&self) -> usize {
        self.pendientes
    }

    /// Lo que ya volvio, sin esperar.
    pub(super) fn recoger(&mut self) -> Vec<Hecha> {
        let Some((_, rx)) = &self.canal else {
            return Vec::new();
        };
        let hechas: Vec<Hecha> = rx.try_iter().collect();
        self.pendientes = self.pendientes.saturating_sub(hechas.len());
        hechas
    }

    /// Espera a que vuelvan las que faltan (como mucho `tope`) y cierra el
    /// hilo. Es lo que se hace al terminar: una foto pedida en el ultimo
    /// segundo de la reunion no se puede perder.
    pub(super) fn acabar(&mut self, tope: std::time::Duration) -> Vec<Hecha> {
        let Some((tx, rx)) = self.canal.take() else {
            return Vec::new();
        };
        drop(tx);
        let hasta = std::time::Instant::now() + tope;
        let mut hechas = Vec::new();
        while self.pendientes > 0 {
            let queda = hasta.saturating_duration_since(std::time::Instant::now());
            match rx.recv_timeout(queda) {
                Ok(h) => {
                    hechas.push(h);
                    self.pendientes -= 1;
                }
                Err(_) => break,
            }
        }
        if self.pendientes > 0 {
            tracing::warn!(
                faltan = self.pendientes,
                "capturas de la clase que no llegaron a tiempo"
            );
        }
        self.pendientes = 0;
        hechas
    }

    fn encargar(&mut self, e: Encargo) -> bool {
        if self.canal.is_none() {
            self.canal = lanzar();
        }
        let Some((tx, _)) = &self.canal else {
            return false;
        };
        if tx.send(e).is_err() {
            // El hilo se cayo: el siguiente encargo lo vuelve a lanzar.
            self.canal = None;
            return false;
        }
        self.pendientes += 1;
        true
    }
}

fn lanzar() -> Option<(Sender<Encargo>, Receiver<Hecha>)> {
    let (tx, encargos) = channel::<Encargo>();
    let (hechas, rx) = channel::<Hecha>();
    let lanzado = std::thread::Builder::new()
        .name("clase-capturas".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let mut dispositivo: Option<pixpin_capture::Dispositivo> = None;
            for e in encargos {
                let hecha = match e {
                    Encargo::Imagen(ms, imagen) => (
                        ms,
                        pixpin_codec::imagen::codificar_png(&imagen).map_err(Into::into),
                    ),
                    Encargo::Zona(ms, zona) => {
                        if dispositivo.is_none() {
                            dispositivo = pixpin_capture::Dispositivo::nuevo().ok();
                        }
                        let foto = match &dispositivo {
                            Some(d) => fotografiar(d, zona),
                            None => Err(anyhow::anyhow!("sin dispositivo de captura")),
                        };
                        if foto.is_err() {
                            // Un dispositivo perdido (se cambio de pantalla o
                            // de driver) se rehace en la siguiente.
                            dispositivo = None;
                        }
                        (ms, foto)
                    }
                };
                if hechas.send(hecha).is_err() {
                    break;
                }
            }
        });
    match lanzado {
        Ok(_) => Some((tx, rx)),
        Err(e) => {
            tracing::warn!(?e, "no se pudo lanzar el hilo de las capturas de la clase");
            None
        }
    }
}

/// La foto de `zona` ahora mismo, en PNG.
fn fotografiar(d: &pixpin_capture::Dispositivo, zona: Rect) -> Result<Vec<u8>> {
    let monitores = pixpin_capture::enumerar_monitores()?;
    let mut fuentes = Vec::new();
    for m in monitores.monitores() {
        if m.area.interseccion(zona).is_some() {
            fuentes.push(pixpin_capture::capturar_monitor(d, m.id, m.area)?);
        }
    }
    anyhow::ensure!(!fuentes.is_empty(), "la zona no cae en ningun monitor");
    let recorte = pixpin_capture::componer_region(d, &fuentes, zona)?;
    let imagen = pixpin_capture::a_imagen(d, &recorte)?;
    Ok(pixpin_codec::imagen::codificar_png(&imagen)?)
}
