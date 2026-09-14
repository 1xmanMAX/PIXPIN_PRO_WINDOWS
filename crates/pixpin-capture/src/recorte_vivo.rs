//! La zona de pantalla que alimenta un pin en vivo.
//!
//! Una `SesionViva` entrega el monitor entero; el pin solo quiere un trozo.
//! El recorte se hace EN LA GPU, sobre una textura propia del tamano de la
//! zona, y solo cuando llega un fotograma nuevo: una pantalla quieta no
//! cuesta nada, y un video en la zona cuesta una copia de su tamano, no la
//! del monitor.
//!
//! El pin envuelve esa textura propia como bitmap UNA vez; como el bitmap
//! comparte la memoria, basta con copiar dentro y repintar.
//!
//! La eleccion del monitor y el recorte son puros y se prueban sin GPU.

use std::time::Duration;

use pixpin_codec::ImagenRgba;
use pixpin_geom::{Monitor, Rect};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BOX, D3D11_TEXTURE2D_DESC, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};

use crate::dispositivo::{Dispositivo, ErrorCaptura};
use crate::instantanea::crear_textura;
use crate::mapa::textura_a_imagen;
use crate::sesion::SesionViva;

/// De que monitor sale la zona y que trozo de ella se ve de verdad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encuadre {
    pub id_monitor: u32,
    /// La zona recortada al monitor, en pixeles fisicos del escritorio.
    pub zona: Rect,
    /// La misma zona relativa al origen del monitor: lo que se copia de su
    /// textura.
    pub desde_x: u32,
    pub desde_y: u32,
}

/// Elige el monitor que mas zona contiene y recorta la zona a el.
///
/// Una sesion de captura es por monitor: una zona que cruza dos se queda con
/// la parte del que tiene mas, que es lo que el usuario estaba mirando. El
/// resto se pierde a proposito: componer dos sesiones en vivo duplicaria el
/// coste por algo que casi nunca se pide. `None` si la zona no toca ningun
/// monitor o queda vacia.
pub fn encuadrar(zona: Rect, monitores: &[Monitor]) -> Option<Encuadre> {
    let (monitor, recorte) = monitores
        .iter()
        .filter_map(|m| m.area.interseccion(zona).map(|r| (m, r)))
        .filter(|(_, r)| !r.esta_vacio())
        .max_by_key(|(_, r)| r.area())?;
    Some(Encuadre {
        id_monitor: monitor.id,
        zona: recorte,
        desde_x: (recorte.x - monitor.area.x) as u32,
        desde_y: (recorte.y - monitor.area.y) as u32,
    })
}

/// La caja a copiar de una textura de `ancho_origen` x `alto_origen`, o
/// `None` si la zona ya no cabe: un cambio de resolucion encoge la textura
/// del monitor, y `CopySubresourceRegion` con una caja fuera de rango no
/// falla, deja basura.
fn caja_dentro(e: &Encuadre, ancho_origen: u32, alto_origen: u32) -> Option<D3D11_BOX> {
    let derecha = e.desde_x.checked_add(e.zona.ancho)?;
    let abajo = e.desde_y.checked_add(e.zona.alto)?;
    (derecha <= ancho_origen && abajo <= alto_origen).then_some(D3D11_BOX {
        left: e.desde_x,
        top: e.desde_y,
        front: 0,
        right: derecha,
        bottom: abajo,
        back: 1,
    })
}

pub struct RecorteVivo {
    /// `Option` para poder cerrarla en `Drop`: `cerrar` consume la sesion.
    sesion: Option<SesionViva>,
    encuadre: Encuadre,
    d3d: ID3D11Device,
    contexto: ID3D11DeviceContext,
    destino: ID3D11Texture2D,
    /// Cuantos fotogramas llevaba la sesion en la ultima copia: si no ha
    /// cambiado, no hay nada nuevo que copiar.
    visto: u64,
}

impl RecorteVivo {
    /// Abre la sesion del monitor del encuadre. `minimo_entre_frames` es el
    /// tope del nivel (30 fps en `Ligero`), y `notificar` la ventana que se
    /// despierta con cada fotograma aceptado.
    pub fn nuevo(
        dispositivo: &Dispositivo,
        encuadre: Encuadre,
        minimo_entre_frames: Duration,
        notificar: Option<(isize, u32)>,
    ) -> Result<RecorteVivo, ErrorCaptura> {
        let destino = crear_textura(dispositivo, encuadre.zona.ancho, encuadre.zona.alto)?;
        let sesion = SesionViva::nueva(
            dispositivo,
            encuadre.id_monitor,
            minimo_entre_frames,
            notificar,
        )?;
        Ok(RecorteVivo {
            sesion: Some(sesion),
            encuadre,
            d3d: dispositivo.d3d().clone(),
            contexto: dispositivo.contexto().clone(),
            destino,
            visto: 0,
        })
    }

    pub fn encuadre(&self) -> Encuadre {
        self.encuadre
    }

    /// Fotogramas que entrego la captura. Con la zona quieta deberia crecer
    /// poco: si crece al ritmo del tope, algo (el propio pin repintandose)
    /// esta provocando fotogramas, y el registro lo delata.
    pub fn aceptados(&self) -> u64 {
        self.sesion.as_ref().map_or(0, SesionViva::aceptados)
    }

    /// Si llego un fotograma desde la ultima vez, copia la zona a la textura
    /// propia y la devuelve. `None` si no hay nada nuevo.
    pub fn tick(&mut self) -> Option<&ID3D11Texture2D> {
        let sesion = self.sesion.as_ref()?;
        let aceptados = sesion.aceptados();
        if aceptados == self.visto {
            return None;
        }
        let origen = sesion.ultimo()?;
        self.visto = aceptados;

        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: `origen` es una textura viva (una cuenta de referencia
        // propia); GetDesc solo rellena la estructura local.
        unsafe { origen.GetDesc(&mut desc) };
        let caja = caja_dentro(&self.encuadre, desc.Width, desc.Height)?;
        // SAFETY: las dos texturas son del mismo dispositivo y formato BGRA;
        // la caja cabe en el origen (lo comprueba `caja_dentro`) y mide lo
        // mismo que el destino, que se creo con el tamano de la zona.
        unsafe {
            self.contexto
                .CopySubresourceRegion(&self.destino, 0, 0, 0, 0, &origen, 0, Some(&caja));
        }
        Some(&self.destino)
    }

    /// El ultimo fotograma copiado, en CPU: para copiarlo o congelarlo.
    pub fn imagen(&self) -> Result<ImagenRgba, ErrorCaptura> {
        textura_a_imagen(&self.d3d, &self.contexto, &self.destino)
    }
}

impl Drop for RecorteVivo {
    fn drop(&mut self) {
        if let Some(s) = self.sesion.take() {
            s.cerrar();
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn monitor(id: u32, x: i32, ancho: u32) -> Monitor {
        let area = Rect {
            x,
            y: 0,
            ancho,
            alto: 1080,
        };
        Monitor {
            id,
            area,
            area_trabajo: area,
            escala_por_cien: 100,
            principal: id == 0,
        }
    }

    #[test]
    fn una_zona_dentro_de_un_monitor_se_queda_entera() {
        let ms = [monitor(0, 0, 1920), monitor(1, 1920, 1920)];
        let zona = Rect {
            x: 2000,
            y: 100,
            ancho: 300,
            alto: 200,
        };
        let e = encuadrar(zona, &ms).unwrap();
        assert_eq!(e.id_monitor, 1);
        assert_eq!(e.zona, zona);
        assert_eq!((e.desde_x, e.desde_y), (80, 100));
    }

    #[test]
    fn una_zona_que_cruza_dos_monitores_se_queda_con_el_que_mas_tiene() {
        let ms = [monitor(0, 0, 1920), monitor(1, 1920, 1920)];
        // 100 px en el primero, 300 en el segundo.
        let zona = Rect {
            x: 1820,
            y: 0,
            ancho: 400,
            alto: 50,
        };
        let e = encuadrar(zona, &ms).unwrap();
        assert_eq!(e.id_monitor, 1);
        assert_eq!(e.zona.x, 1920);
        assert_eq!(e.zona.ancho, 300);
        assert_eq!(e.desde_x, 0);
    }

    #[test]
    fn una_zona_fuera_de_todo_monitor_no_da_encuadre() {
        let ms = [monitor(0, 0, 1920)];
        let zona = Rect {
            x: 5000,
            y: 0,
            ancho: 10,
            alto: 10,
        };
        assert_eq!(encuadrar(zona, &ms), None);
    }

    #[test]
    fn una_zona_en_un_monitor_con_coordenadas_negativas_se_mide_desde_su_origen() {
        let ms = [monitor(0, 0, 1920), monitor(1, -1280, 1280)];
        let zona = Rect {
            x: -1000,
            y: 10,
            ancho: 100,
            alto: 100,
        };
        let e = encuadrar(zona, &ms).unwrap();
        assert_eq!(e.id_monitor, 1);
        assert_eq!((e.desde_x, e.desde_y), (280, 10));
    }

    #[test]
    fn la_caja_cabe_justa_en_el_borde_y_no_un_pixel_mas() {
        let e = Encuadre {
            id_monitor: 0,
            zona: Rect {
                x: 1820,
                y: 980,
                ancho: 100,
                alto: 100,
            },
            desde_x: 1820,
            desde_y: 980,
        };
        let caja = caja_dentro(&e, 1920, 1080).unwrap();
        assert_eq!((caja.right, caja.bottom), (1920, 1080));
        // Caso negativo: la resolucion bajo y la zona ya no cabe.
        assert!(caja_dentro(&e, 1919, 1080).is_none());
        assert!(caja_dentro(&e, 1920, 1079).is_none());
    }
}
