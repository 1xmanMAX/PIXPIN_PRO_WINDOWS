//! **Una foto abierta con PixPin, como pin**: lo mas deprisa y gastando lo
//! menos posible (el usuario, 4-oct-2026).
//!
//! Antes: descodificar la foto entera (una de movil son 48 MB en RGBA),
//! copiarla, recodificarla a PNG para el almacen —lo que mas tardaba con
//! diferencia: segundos— y subirla entera a la GPU para verla al 30 %.
//! Ahora:
//!
//! 1. Se lee **a la medida en que se va a ver** y ya derecha segun su EXIF
//!    (`pixpin_codec::vista`).
//! 2. Una foto grande sale **al momento con una vista previa** (la miniatura
//!    EXIF, o la foto a un cuarto) y la buena se lee en otro hilo; al llegar,
//!    el pin la cambia sin moverse (`Pin::poner_imagen_leida`).
//! 3. El almacen guarda **una copia del fichero tal cual**, sin recodificar.
//! 4. El pin sube solo lo leido, y si luego se acerca mas alla (o saca la
//!    lupa) lee la entera de esa copia, una vez (`Pin::poner_resolucion_completa`).

use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

use anyhow::{Context, Result};
use pixpin_codec::vista::Vista;
use pixpin_geom::Monitor;
use pixpin_pin::Contenido;

use super::Pines;

/// El origen con que se guardan en el almacen: distingue una foto abierta
/// (que al restaurarse vuelve a leerse reducida) de una captura o una
/// pizarra (que se leen enteras, como siempre).
pub const ORIGEN_FOTO: &str = "foto";

/// La caja en que cabe un pin que no viene de un recorte: el 80 % del area
/// de trabajo (`region_centrada`). Es la medida a la que se lee la foto.
pub fn caja_de_lectura(monitor: &Monitor) -> (u32, u32) {
    (
        ((monitor.area_trabajo.ancho as f32 * 0.8) as u32).max(1),
        ((monitor.area_trabajo.alto as f32 * 0.8) as u32).max(1),
    )
}

/// Una foto leida en otro hilo, para el pin `id`.
pub struct FotoLeida {
    id: u64,
    vista: Result<Vista, String>,
    ms: f64,
}

/// El buzon de las fotos que se leen en otro hilo.
pub struct Lecturas {
    enviar: Sender<FotoLeida>,
    recibir: Receiver<FotoLeida>,
}

impl Lecturas {
    pub fn nuevas() -> Lecturas {
        let (enviar, recibir) = channel();
        Lecturas { enviar, recibir }
    }
}

impl Pines {
    /// Pinea la foto `ruta` centrada en `monitor`. Un error es «no es una
    /// imagen que Windows sepa leer»: quien llama la pinea como archivo.
    pub fn pinear_foto(&mut self, ruta: &Path, monitor: &Monitor) -> Result<u64> {
        let t0 = Instant::now();
        let caja = caja_de_lectura(monitor);
        // Una foto grande, primero su vista previa; la buena, despues.
        let previa = pixpin_codec::vista::vista_previa(ruta, caja).ok().flatten();
        let con_previa = previa.is_some();
        let vista = match previa {
            Some(v) => v,
            None => pixpin_codec::vista::cargar_para_ver(ruta, caja)
                .context("no es una imagen legible")?,
        };
        let ms_leer = t0.elapsed().as_secs_f64() * 1000.0;
        let nativa = (vista.ancho_completo, vista.alto_completo);
        let leida = (vista.imagen.ancho, vista.imagen.alto);
        let reducida = vista.reducida();
        // El pin nace del tamano al que se vera la buena, no del de la previa.
        let medida = pixpin_codec::vista::medida_de_lectura(nativa, caja);
        let region = self.region_centrada_de(medida, monitor);
        let contenido = Contenido::Imagen(vista.imagen);

        let t1 = Instant::now();
        let id = self
            .almacen
            .borrow_mut()
            .guardar_imagen_de_fichero(
                ruta,
                ORIGEN_FOTO,
                Some(Pines::guardado_desde(region, monitor.escala_por_cien, 100)),
            )
            .context("no se pudo guardar la foto en el almacen")?;
        let ms_guardar = t1.elapsed().as_secs_f64() * 1000.0;

        let t2 = Instant::now();
        self.crear_ventana(id, contenido, region, monitor.escala_por_cien)?;
        if reducida {
            self.dar_resolucion_completa(id, nativa);
        }
        let ms_ventana = t2.elapsed().as_secs_f64() * 1000.0;
        if con_previa {
            self.leer_aparte(id, ruta, caja);
        }
        tracing::info!(
            id,
            ?nativa,
            ?leida,
            con_previa,
            ms_leer = format!("{ms_leer:.1}"),
            ms_guardar = format!("{ms_guardar:.1}"),
            ms_ventana = format!("{ms_ventana:.1}"),
            ms = t0.elapsed().as_millis() as u64,
            "foto pineada"
        );
        Ok(id)
    }

    /// Lee la foto buena en otro hilo y avisa al bucle al acabar.
    fn leer_aparte(&self, id: u64, ruta: &Path, caja: (u32, u32)) {
        let enviar = self.fotos_leidas.enviar.clone();
        let ruta = ruta.to_path_buf();
        // Un HWND no cruza hilos; su valor si, y PostMessage vale desde
        // cualquiera.
        let aviso = self.hwnd_app.0 as isize;
        let lanzado = std::thread::Builder::new()
            .name("leer-foto".into())
            .spawn(move || {
                let t = Instant::now();
                let vista =
                    pixpin_codec::vista::cargar_para_ver(&ruta, caja).map_err(|e| e.to_string());
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                if enviar.send(FotoLeida { id, vista, ms }).is_ok() {
                    pixpin_shell::despertar(windows::Win32::Foundation::HWND(aviso as *mut _));
                }
            });
        if let Err(e) = lanzado {
            tracing::warn!(?e, id, "no se pudo lanzar la lectura de la foto");
        }
    }

    /// Las fotos que acabaron de leerse: cada una a su pin, si sigue abierto.
    pub(super) fn atender_fotos_leidas(&mut self) {
        while let Ok(f) = self.fotos_leidas.recibir.try_recv() {
            self.atender_foto_leida(f);
        }
    }

    fn atender_foto_leida(&mut self, FotoLeida { id, vista, ms }: FotoLeida) {
        match vista {
            Ok(v) => {
                if let Some(pin) = self.vivos.get(&id) {
                    let leida = (v.imagen.ancho, v.imagen.alto);
                    pin.poner_imagen_leida(v.imagen);
                    tracing::info!(id, ?leida, ms = format!("{ms:.1}"), "foto afinada");
                }
            }
            Err(e) => tracing::warn!(%e, id, "la foto no se pudo leer entera; queda la previa"),
        }
    }

    /// Espera (hasta `hasta`) a que llegue una foto que se esta leyendo y la
    /// atiende como el bucle. Para las pruebas.
    #[cfg(test)]
    pub fn esperar_foto_leida(&mut self, hasta: std::time::Duration) -> bool {
        match self.fotos_leidas.recibir.recv_timeout(hasta) {
            Ok(f) => {
                self.atender_foto_leida(f);
                true
            }
            Err(_) => false,
        }
    }

    /// Le dice al pin `id` donde esta su imagen entera y cuanto mide.
    pub(super) fn dar_resolucion_completa(&self, id: u64, nativa: (u32, u32)) {
        let objeto = {
            let a = self.almacen.borrow();
            a.entradas()
                .iter()
                .find(|e| e.id == id)
                .map(|e| a.ruta_objeto(e))
        };
        if let (Some(pin), Some(objeto)) = (self.vivos.get(&id), objeto) {
            pin.poner_resolucion_completa(objeto, nativa);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::Rect;

    fn monitor(ancho: u32, alto: u32) -> Monitor {
        Monitor {
            id: 1,
            area: Rect {
                x: 0,
                y: 0,
                ancho,
                alto,
            },
            area_trabajo: Rect {
                x: 0,
                y: 0,
                ancho,
                alto,
            },
            escala_por_cien: 100,
            principal: true,
        }
    }

    #[test]
    fn la_foto_se_lee_al_80_por_ciento_del_area_de_trabajo() {
        assert_eq!(caja_de_lectura(&monitor(1920, 1080)), (1536, 864));
        assert_eq!(caja_de_lectura(&monitor(3000, 2000)), (2400, 1600));
    }

    #[test]
    fn caso_negativo_un_area_nula_no_da_una_caja_de_cero() {
        assert_eq!(caja_de_lectura(&monitor(0, 0)), (1, 1));
    }
}
