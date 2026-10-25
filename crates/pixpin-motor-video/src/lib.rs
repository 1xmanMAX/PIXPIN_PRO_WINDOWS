//! pixpin-motor-video — **el motor de video** de PixPin (25-oct-2026, el
//! usuario: «un motor de video que grabe a buena resolucion y buena calidad
//! pero que pese menos y gaste menos RAM mientras graba … ultra optimizado»).
//!
//! Ver `docs/motor-de-video.md`. En corto:
//!
//! ```text
//!  Windows.Graphics.Capture ──▶ ultimo fotograma retenido (sin copiar)
//!          │ (solo cuando la pantalla cambia)
//!          ▼
//!  ID3D11VideoProcessor: BGRA ─▶ NV12, encoge y pone bandas, en la GPU
//!          │                      (antes de soltar el fotograma)
//!          ▼
//!  IMFSample sobre la textura ─▶ H.264 de la grafica (IMFSinkWriter con
//!          │                      IMFDXGIDeviceManager)
//!          ▼
//!  MP4 (.parte) ◀── AAC del sonido que se le da (`poner_audio`)
//! ```
//!
//! - **La imagen no baja nunca a la CPU** en el camino bueno: la textura de
//!   la pantalla se convierte y se encoge en la GPU y el codificador de la
//!   grafica la lee de alli. En el equipo de referencia (Core i3 de tercera
//!   generacion, Quick Sync) es lo que deja la CPU casi quieta.
//! - **Ritmo variable**: solo se codifica cuando la pantalla cambio, con un
//!   fotograma de repeticion cada segundo; una diapositiva quieta cuesta casi
//!   cero bytes y cero trabajo.
//! - **Cadena de caidas**, elegida al empezar y dicha en
//!   [`Estadisticas::codificador`]: la grafica, luego el codificador por
//!   software con la conversion aun en la GPU, y por ultimo el camino de
//!   siempre por la CPU (`pixpin-record`).
//! - **Colas con tope** en todas partes: si el codificador se atrasa se
//!   saltan fotogramas, nunca sonido, y la memoria no crece.
//!
//! - **Si algo sale mal, sigue**: la grafica que se pierde se vuelve a
//!   montar sin cerrar el MP4 ([`recuperacion`]), el monitor que se
//!   desconecta se vuelve a buscar ([`reapertura`]), y se graba en la
//!   grafica del monitor, nunca en una virtual ([`adaptador`]).
//!
//! Las cuentas (perfiles, tiempos, ritmo, encaje, colas, caidas, el fichero
//! a medias) y el nucleo que decide ([`nucleo`], que habla con cada sistema
//! por los rasgos de [`plataforma`]) son puros y se prueban en cualquier
//! maquina; lo que habla con Windows va en `win/` y solo se compila alli.
//! Fuera de Windows, [`MotorVideo::empezar`] dice que no esta disponible
//! (el diseno de Android esta en `docs/motor-de-video.md`).
//!
//! Este crate habla con el sistema operativo. El `unsafe` esta permitido,
//! pero cada bloque lleva su comentario `// SAFETY:`.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod adaptador;
pub mod archivo;
pub mod audio;
pub mod cadena;
pub mod cola;
pub mod geometria;
pub mod lienzo;
pub mod nucleo;
pub mod perfil;
pub mod plataforma;
pub mod reapertura;
pub mod recuperacion;
pub mod reloj;
pub mod ritmo;

#[cfg(windows)]
mod win;

use std::path::PathBuf;

pub use audio::{CopiaMono, FormatoAudio, FuenteAudio, Siguiente};
pub use cadena::{Camino, Codificador, Formato};
pub use perfil::Perfil;

#[cfg(windows)]
pub use win::{MotorVideo, hay_h264_por_hardware, hay_hevc_por_hardware, monitor_principal};

/// Lo que se graba. Los manejadores van crudos (`isize`) para que las
/// opciones se puedan escribir y probar en cualquier maquina.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fuente {
    /// Un monitor entero, por su `HMONITOR`.
    Monitor(isize),
    /// Una ventana, por su `HWND`. Si se cierra, el video sigue con la
    /// ultima imagen hasta que se termine (el sonido no se corta).
    Ventana(isize),
}

/// Como se graba.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opciones {
    pub fuente: Fuente,
    pub perfil: Perfil,
    /// El raton dentro del video.
    pub con_cursor: bool,
    /// El sonido que llegara por [`MotorVideo::poner_audio`], o `None` para
    /// un video mudo (entonces el reloj es el del sistema).
    pub audio: Option<FormatoAudio>,
    /// Pedir HEVC (H.265). Solo se usa si la grafica lo codifica; si no, se
    /// queda en H.264, que es lo que reproduce cualquier movil.
    pub hevc: bool,
    /// Intentar el MP4 fragmentado, que se puede abrir aunque la aplicacion
    /// se cierre de golpe. Apagado de salida: ver `docs/motor-de-video.md`
    /// (no todos los reproductores lo buscan bien y Windows 10 no lo
    /// garantiza en el escritor). Si no se puede, MP4 normal.
    pub fragmentado: bool,
}

impl Opciones {
    /// Las opciones de siempre para una fuente: perfil «Normal», con cursor,
    /// sin sonido, H.264 y MP4 normal.
    pub fn de(fuente: Fuente) -> Opciones {
        Opciones {
            fuente,
            perfil: Perfil::Normal,
            con_cursor: true,
            audio: None,
            hevc: false,
            fragmentado: false,
        }
    }

    /// Si se puede grabar con ellas. Se mira antes de arrancar nada: un
    /// muestreo que el AAC no admite fallaria tarde y sin contexto.
    pub fn validar(&self) -> Result<(), ErrorMotor> {
        let nulo = match self.fuente {
            Fuente::Monitor(h) | Fuente::Ventana(h) => h == 0,
        };
        if nulo {
            return Err(ErrorMotor::Opciones("la fuente no tiene manejador"));
        }
        if let Some(a) = &self.audio {
            a.validar().map_err(ErrorMotor::Opciones)?;
        }
        Ok(())
    }
}

/// Como va la grabacion, para ensenarlo mientras dura.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estadisticas {
    /// Fotogramas codificados en el ultimo segundo (ritmo variable: con la
    /// pantalla quieta baja a uno).
    pub fps: f32,
    pub fotogramas: u64,
    /// Fotogramas que tocaban y no se pudieron codificar (codificador
    /// atrasado). Los que se juntan porque la pantalla cambia mas deprisa
    /// que el tope no cuentan: eso es el ritmo, no una perdida.
    pub caidos: u64,
    /// Lo que ocupa ya el fichero.
    pub bytes: u64,
    pub codificador: Codificador,
    pub formato: Formato,
    /// Medidas del video.
    pub medidas: (u32, u32),
    /// El video dejo de grabarse a mitad (se guarda lo que habia).
    pub cortado: bool,
    /// Veces que la grafica se perdio y se volvio a montar sin parar la
    /// grabacion (ver `recuperacion`).
    pub vueltas: u32,
}

/// Lo que quedo al terminar.
#[derive(Debug, Clone, PartialEq)]
pub struct Resumen {
    pub ruta: PathBuf,
    /// Lo que dura el video, en milisegundos.
    pub duracion_ms: u64,
    pub fotogramas: u64,
    pub caidos: u64,
    pub bytes: u64,
    pub codificador: Codificador,
    pub formato: Formato,
    pub medidas: (u32, u32),
    /// Si salio fragmentado (ver [`Opciones::fragmentado`]).
    pub fragmentado: bool,
    /// Por que se corto a mitad, si se corto.
    pub cortado: Option<String>,
    /// Veces que la grafica se perdio y se volvio a montar.
    pub vueltas: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorMotor {
    #[error("el motor de video solo funciona en Windows")]
    SoloWindows,
    #[error("las opciones del video no valen: {0}")]
    Opciones(&'static str),
    #[error("no se pudo mirar la pantalla: {0}")]
    Fuente(String),
    #[error("ningun codificador de video arranco: {0}")]
    SinCodificador(String),
    #[error("Windows fallo al {paso}: {detalle}")]
    Sistema {
        paso: &'static str,
        detalle: String,
        /// El `HRESULT` (cero si no lo hubo).
        codigo: i32,
    },
    #[error("no se grabo ningun fotograma")]
    SinFotogramas,
    #[error("no se pudo poner el video en {ruta}: {fuente}")]
    Renombrar {
        ruta: PathBuf,
        #[source]
        fuente: std::io::Error,
    },
    #[error("el hilo del motor de video se cayo")]
    HiloCaido,
    #[error("la grabacion del video se tiro")]
    Tirado,
}

impl ErrorMotor {
    /// **Si el fallo es que la grafica se fue** (TDR, controlador
    /// actualizado): entonces se vuelve a montar todo en vez de cortar.
    pub fn es_perdida_de_la_grafica(&self) -> bool {
        matches!(self, ErrorMotor::Sistema { codigo, .. } if recuperacion::es_perdida(*codigo))
    }
}

/// Fuera de Windows el motor no existe, pero su forma si: quien lo usa
/// compila igual y se entera al empezar.
#[cfg(not(windows))]
pub struct MotorVideo {
    _nunca: std::convert::Infallible,
}

#[cfg(not(windows))]
impl MotorVideo {
    pub fn empezar(opciones: Opciones, destino: PathBuf) -> Result<MotorVideo, ErrorMotor> {
        opciones.validar()?;
        let _ = destino;
        Err(ErrorMotor::SoloWindows)
    }

    pub fn poner_audio(&self, _muestras: &[i16], _posicion: u64) -> Result<(), ErrorMotor> {
        match self._nunca {}
    }

    pub fn pausar(&self) {
        match self._nunca {}
    }

    pub fn seguir(&self) {
        match self._nunca {}
    }

    pub fn estado(&self) -> Estadisticas {
        match self._nunca {}
    }

    pub fn terminar(self) -> Result<Resumen, ErrorMotor> {
        match self._nunca {}
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_opciones_de_siempre_son_normal_con_cursor_y_h264() {
        let o = Opciones::de(Fuente::Monitor(7));
        assert_eq!(o.perfil, Perfil::Normal);
        assert!(o.con_cursor);
        assert!(!o.hevc);
        assert!(!o.fragmentado);
        assert!(o.validar().is_ok());
    }

    #[test]
    fn unas_opciones_sin_fuente_o_con_un_muestreo_raro_no_valen() {
        // Caso negativo: un manejador nulo no es ninguna pantalla.
        assert!(Opciones::de(Fuente::Ventana(0)).validar().is_err());
        let mut o = Opciones::de(Fuente::Monitor(1));
        o.audio = Some(FormatoAudio {
            muestreo: 96_000,
            canales: 1,
        });
        assert!(matches!(o.validar(), Err(ErrorMotor::Opciones(_))));
        o.audio = Some(FormatoAudio {
            muestreo: 48_000,
            canales: 2,
        });
        assert!(o.validar().is_ok());
    }

    #[cfg(not(windows))]
    #[test]
    fn fuera_de_windows_empezar_lo_dice_claro() {
        let o = Opciones::de(Fuente::Monitor(1));
        assert!(matches!(
            MotorVideo::empezar(o, PathBuf::from("x.mp4")),
            Err(ErrorMotor::SoloWindows)
        ));
        // Y las opciones se miran antes: lo que no vale no llega a eso.
        let o = Opciones::de(Fuente::Monitor(0));
        assert!(matches!(
            MotorVideo::empezar(o, PathBuf::from("x.mp4")),
            Err(ErrorMotor::Opciones(_))
        ));
    }
}
