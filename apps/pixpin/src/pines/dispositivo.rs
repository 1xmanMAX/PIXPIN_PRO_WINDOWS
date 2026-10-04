//! **Los pines, a un dispositivo grafico nuevo** (el suyo se perdio).
//!
//! Cada pin guarda su estado fuera de la GPU (sitio, tamano, zoom, giro,
//! anotaciones como ordenes, la imagen en pixeles), asi que pasar a otro
//! dispositivo es rehacer lo que si vive en ella: superficie y bitmap
//! (`Pin::cambiar_dispositivo`). Lo que tiene ventanitas propias con su
//! superficie —la anotacion en curso con su paleta, el panel «Pines
//! abiertos»— se cierra y se vuelve a abrir tal cual, y la captura de un pin
//! en vivo se reabre sobre el dispositivo nuevo.

use std::rc::Rc;

use anyhow::Context;
use pixpin_render::MotorRender;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

use super::Pines;

/// Lo que se rehizo, para el registro.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Rehechos {
    pub pines: usize,
    pub fallidos: usize,
}

impl Pines {
    /// Pasa todo lo del gestor al dispositivo `dispositivo` y su `motor`.
    pub fn cambiar_dispositivo(
        &mut self,
        dispositivo: &pixpin_capture::Dispositivo,
        motor: Rc<MotorRender>,
    ) -> Rehechos {
        let d3d: ID3D11Device = dispositivo.d3d().clone();
        // La anotacion en curso: se guarda y se vuelve a abrir al final, con
        // su paleta ya sobre el dispositivo nuevo.
        let anotando = self.anotacion.as_ref().map(|a| a.id);
        if let Err(e) = self.salir_de_anotar() {
            tracing::warn!(
                ?e,
                "no se pudo guardar la anotacion al rehacer el dispositivo"
            );
        }
        let con_panel = self.panel.take().is_some();

        self.d3d = d3d.clone();
        self.motor = Rc::clone(&motor);
        let ritmo = self.ritmo_video.unwrap_or(16);
        let mut hecho = Rehechos::default();
        for (id, pin) in &self.vivos {
            match pin.cambiar_dispositivo(&d3d, Rc::clone(&motor), ritmo) {
                Ok(()) => hecho.pines += 1,
                Err(e) => {
                    hecho.fallidos += 1;
                    tracing::warn!(?e, id, "un pin no pudo pasar al dispositivo nuevo");
                }
            }
        }
        // Los pines en vivo: su ventana como los demas y, ademas, su
        // captura, que copiaba fotogramas a una textura del viejo.
        let mut muertos = Vec::new();
        for (id, (pin, recorte)) in &self.en_vivo {
            let reabierto = pin
                .cambiar_dispositivo(&d3d, Rc::clone(&motor), ritmo)
                .context("la ventana")
                .and_then(|()| {
                    let encuadre = recorte.borrow().encuadre();
                    pixpin_capture::RecorteVivo::nuevo(
                        dispositivo,
                        encuadre,
                        self.tope_en_vivo,
                        Some((pin.hwnd().0 as isize, pixpin_pin::MSG_FOTOGRAMA_VIVO)),
                    )
                    .context("la captura")
                });
            match reabierto {
                Ok(nuevo) => {
                    *recorte.borrow_mut() = nuevo;
                    hecho.pines += 1;
                }
                Err(e) => {
                    hecho.fallidos += 1;
                    tracing::warn!(?e, id, "un pin en vivo no pudo reabrirse; se cierra");
                    muertos.push(*id);
                }
            }
        }
        // Un pin en vivo sin captura seria un recuadro congelado sin
        // explicacion: mejor cerrarlo.
        for id in muertos {
            self.en_vivo.remove(&id);
        }

        if con_panel && let Err(e) = self.abrir_panel_abiertos() {
            tracing::warn!(?e, "el panel de pines no pudo reabrirse");
        }
        if let Some(id) = anotando
            && let Err(e) = self.anotar_pin(id)
        {
            tracing::warn!(?e, id, "la anotacion no pudo reabrirse");
        }
        hecho
    }

    /// Si todos los pines abiertos pintaron bien su ultimo fotograma.
    #[cfg(test)]
    pub fn todos_pintan_bien(&self) -> bool {
        self.vivos.values().all(|p| p.pintado_bien())
    }

    /// Repinta todos los pines (para las pruebas de la perdida).
    #[cfg(test)]
    pub fn repintar_todos(&self) {
        for p in self.vivos.values() {
            p.poner_opacidad(p.opacidad());
        }
    }
}
