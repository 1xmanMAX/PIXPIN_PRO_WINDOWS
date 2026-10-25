//! **Lo que cada sistema tiene que dar** para que el motor grabe: una
//! captura de la pantalla y una tuberia (codificador + contenedor).
//!
//! El nucleo del motor ([`crate::nucleo`]: el ritmo variable, el reloj, las
//! colas, el atasco, el cierre) no sabe nada de Windows ni de Android:
//! habla con estos dos rasgos. El de Windows (`win/`) los cumple con
//! Windows.Graphics.Capture, el procesador de video de D3D11 y el
//! `IMFSinkWriter`; uno de Android los cumpliria con MediaProjection,
//! MediaCodec y MediaMuxer (ver la seccion «Android» de
//! `docs/motor-de-video.md`).
//!
//! - [`Captura`] retiene **un** fotograma, el ultimo, sin copiarlo, hasta
//!   que el nucleo lo toma (o lo suelta si no le toca).
//! - [`Tuberia`] recibe ese fotograma fechado (en unidades de 100 ns del
//!   reloj del MP4), o la orden de repetir el ultimo, el sonido con su
//!   posicion, y al final hasta donde llega el ultimo fotograma. En Windows
//!   el `IMFSinkWriter` es a la vez codificador y contenedor; en Android
//!   son dos piezas (MediaCodec y MediaMuxer) detras de la misma cara.

use crate::ErrorMotor;

/// Como le fue a un fotograma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Escrito {
    Hecho,
    /// El codificador va atrasado: se salto.
    Caido,
}

/// Un fotograma de la pantalla aun sin soltar.
pub trait Retenido {
    /// Cuando se capturo, en 100 ns del contador del sistema.
    fn hora(&self) -> i64;
    /// Lo devuelve a la captura. Se llama cuando la tuberia ya tiene en su
    /// cola (de la GPU) todo lo que lo lee.
    fn soltar(self);
}

/// **La pantalla** que se graba.
pub trait Captura {
    type Retenido: Retenido;
    /// La hora del fotograma retenido, si lo hay (sin tomarlo).
    fn hora_retenida(&self) -> Option<i64>;
    /// Toma el fotograma retenido. Quien lo toma lo suelta.
    fn tomar(&self) -> Option<Self::Retenido>;
    /// Si la fuente se fue (ventana cerrada, monitor desconectado).
    fn cerrada(&self) -> bool;
}

/// **El codificador y el contenedor**.
pub trait Tuberia {
    /// El fotograma que sabe leer: el de su captura.
    type Retenido;
    /// Un fotograma de la pantalla que empieza en `inicio`. `duracion` es la
    /// de un fotograma al ritmo maximo (la minima).
    fn fotograma(
        &mut self,
        r: &Self::Retenido,
        inicio: i64,
        duracion: i64,
    ) -> Result<Escrito, ErrorMotor>;
    /// El ultimo otra vez (o uno negro si aun no hay ninguno).
    fn repetir(&mut self, inicio: i64, duracion: i64) -> Result<Escrito, ErrorMotor>;
    /// Sonido intercalado que empieza en `posicion` (por canal). Nunca
    /// pisa lo ya escrito (el nucleo lo recorta antes).
    fn audio(&mut self, muestras: &[i16], posicion: u64) -> Result<(), ErrorMotor>;
    /// El ultimo fotograma, hasta `fin`, para que el video dure lo mismo
    /// que el sonido.
    fn vaciar(&mut self, fin: i64, duracion: i64) -> Result<(), ErrorMotor>;
    /// Fotogramas dados hasta ahora.
    fn escritos(&self) -> u64;
}
