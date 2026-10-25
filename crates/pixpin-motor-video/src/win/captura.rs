//! **La pantalla, con Windows.Graphics.Capture**, sin copiar nada.
//!
//! El manejador de `FrameArrived` corre en un hilo del sistema y no toca la
//! GPU: saca el fotograma del pool y lo **retiene** (uno solo, el ultimo),
//! soltando el que hubiera. El hilo del motor lo toma cuando le toca
//! codificar (ver `ritmo`), lo convierte en la GPU y **solo entonces** lo
//! suelta. Asi se arregla lo de `pixpin_capture::SesionViva`, que guardaba
//! la textura del pool y soltaba el fotograma: Windows la volvia a llenar
//! mientras se leia y salian imagenes rasgadas.
//!
//! - El pool tiene dos superficies: una retenida y otra donde Windows pinta
//!   la siguiente. Retener el ultimo no frena la captura.
//! - Si la pantalla cambia de tamano (otra resolucion, una ventana que se
//!   estira) el pool se recrea (`Recreate`) al tamano nuevo; el video no
//!   cambia de tamano, el procesador de video pone bandas.
//! - Si la ventana grabada se cierra, se apunta y el video sigue con su
//!   ultima imagen: el sonido de la reunion no se corta por eso.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex, MutexGuard};

use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::core::{AgileReference, HSTRING, IInspectable, Interface};

use super::com;
use super::motor::Mensaje;
use crate::{ErrorMotor, Fuente};

/// Superficies del pool. Dos bastan (ver la cabecera) y cada una es una
/// pantalla entera en BGRA: 8 MB a 1080p.
const SUPERFICIES: i32 = 2;

/// Un fotograma de la pantalla aun sin soltar.
pub(crate) struct Retenido {
    pub fotograma: Direct3D11CaptureFrame,
    /// Cuando se capturo, en 100 ns del contador del sistema.
    pub hora: i64,
    /// Lo que tiene imagen de verdad (puede ser menos que la superficie
    /// justo tras un cambio de tamano).
    pub contenido: (u32, u32),
}

impl Retenido {
    /// La textura del fotograma. Vale mientras no se suelte.
    pub fn textura(&self) -> Result<ID3D11Texture2D, ErrorMotor> {
        let superficie = self
            .fotograma
            .Surface()
            .map_err(com::en("leer la superficie del fotograma"))?;
        let acceso: IDirect3DDxgiInterfaceAccess = superficie
            .cast()
            .map_err(com::en("ver la superficie como DXGI"))?;
        // SAFETY: `acceso` sale de la superficie viva de este fotograma;
        // `GetInterface` devuelve una referencia propia a su textura.
        unsafe { acceso.GetInterface::<ID3D11Texture2D>() }
            .map_err(com::en("sacar la textura del fotograma"))
    }

    /// Lo suelta al pool. Se llama cuando la GPU ya tiene en su cola todo
    /// lo que lo lee.
    pub fn soltar(self) {
        let _ = self.fotograma.Close();
    }
}

/// Lo que comparten el manejador y el motor.
#[derive(Default)]
struct Compartido {
    retenido: Mutex<Option<Retenido>>,
    tamano: Mutex<(i32, i32)>,
    cerrada: AtomicBool,
}

fn cerrojo<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) struct Captura {
    item: GraphicsCaptureItem,
    pool: Direct3D11CaptureFramePool,
    sesion: GraphicsCaptureSession,
    token: Option<i64>,
    token_cierre: Option<i64>,
    compartido: Arc<Compartido>,
    tamano: (u32, u32),
}

impl Captura {
    /// Empieza a mirar `fuente`. Cada fotograma nuevo despierta al motor con
    /// [`Mensaje::Fotograma`] (si el canal esta lleno no hace falta: el
    /// fotograma queda retenido y el motor lo ve en su siguiente vuelta).
    pub fn abrir(
        dispositivo: &pixpin_capture::Dispositivo,
        fuente: Fuente,
        con_cursor: bool,
        minimo_entre: i64,
        despertar: SyncSender<Mensaje>,
    ) -> Result<Captura, ErrorMotor> {
        let fuera = |e: windows::core::Error| ErrorMotor::Fuente(e.to_string());
        if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
            return Err(ErrorMotor::Fuente(
                "este Windows no tiene Windows.Graphics.Capture".into(),
            ));
        }
        let interop: IGraphicsCaptureItemInterop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
                .map_err(fuera)?;
        let item: GraphicsCaptureItem = match fuente {
            // SAFETY: el manejador lo da quien llama; si ya no es valido la
            // funcion devuelve un error, no lee fuera.
            Fuente::Monitor(h) => unsafe { interop.CreateForMonitor(HMONITOR(h as *mut _)) },
            // SAFETY: igual, con una ventana.
            Fuente::Ventana(h) => unsafe { interop.CreateForWindow(HWND(h as *mut _)) },
        }
        .map_err(fuera)?;
        let size = item.Size().map_err(fuera)?;
        if size.Width <= 0 || size.Height <= 0 {
            return Err(ErrorMotor::Fuente("la fuente no tiene tamano".into()));
        }
        let winrt = dispositivo.winrt();
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            winrt,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            SUPERFICIES,
            size,
        )
        .map_err(fuera)?;

        let compartido = Arc::new(Compartido::default());
        *cerrojo(&compartido.tamano) = (size.Width, size.Height);
        let del_manejador = Arc::clone(&compartido);
        // El dispositivo WinRT no es `Send`; el manejador corre en otro hilo
        // y lo necesita para recrear el pool. Una referencia agil sirve.
        let para_recrear = AgileReference::new(winrt).map_err(fuera)?;
        let manejador =
            TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(move |pool, _| {
                let Some(pool) = pool.as_ref() else {
                    return Ok(());
                };
                // Se saca siempre: sin drenar el pool, Windows deja de dar.
                let Ok(fotograma) = pool.TryGetNextFrame() else {
                    return Ok(());
                };
                let hora = fotograma
                    .SystemRelativeTime()
                    .map(|t| t.Duration)
                    .unwrap_or_else(|_| com::ahora());
                let contenido = fotograma.ContentSize().unwrap_or(SizeInt32 {
                    Width: 0,
                    Height: 0,
                });
                if contenido.Width <= 0 || contenido.Height <= 0 {
                    let _ = fotograma.Close();
                    return Ok(());
                }
                let cambio = {
                    let mut t = cerrojo(&del_manejador.tamano);
                    let nuevo = (contenido.Width, contenido.Height);
                    let cambio = *t != nuevo;
                    *t = nuevo;
                    cambio
                };
                if cambio {
                    // Otro tamano: este fotograma y el retenido son del pool
                    // viejo. Se sueltan, se recrea y el siguiente ya llega
                    // entero; mientras, el motor repite el ultimo.
                    let _ = fotograma.Close();
                    if let Some(previo) = cerrojo(&del_manejador.retenido).take() {
                        previo.soltar();
                    }
                    let Ok(winrt) = para_recrear.resolve() else {
                        return Ok(());
                    };
                    let _ = pool.Recreate(
                        &winrt,
                        DirectXPixelFormat::B8G8R8A8UIntNormalized,
                        SUPERFICIES,
                        contenido,
                    );
                    return Ok(());
                }
                let nuevo = Retenido {
                    fotograma,
                    hora,
                    contenido: (contenido.Width as u32, contenido.Height as u32),
                };
                let previo = cerrojo(&del_manejador.retenido).replace(nuevo);
                // Fuera del cerrojo: soltar no tiene por que esperar al motor.
                if let Some(previo) = previo {
                    previo.soltar();
                }
                let _ = despertar.try_send(Mensaje::Fotograma);
                Ok(())
            });
        let token = pool.FrameArrived(&manejador).map_err(fuera)?;

        // La ventana que se cierra, o el monitor que se desconecta.
        let del_cierre = Arc::clone(&compartido);
        let token_cierre = item
            .Closed(
                &TypedEventHandler::<GraphicsCaptureItem, IInspectable>::new(move |_, _| {
                    del_cierre.cerrada.store(true, Ordering::Release);
                    Ok(())
                }),
            )
            .ok();

        let sesion = match pool.CreateCaptureSession(&item) {
            Ok(s) => s,
            Err(e) => {
                let _ = pool.RemoveFrameArrived(token);
                let _ = pool.Close();
                return Err(fuera(e));
            }
        };
        // Por capacidad, no por version: el borde amarillo solo se puede
        // quitar en Windows 11; el tope de ritmo, desde Windows 11 24H2.
        if hay_propiedad("IsBorderRequired") {
            let _ = sesion.SetIsBorderRequired(false);
        }
        if hay_propiedad("MinUpdateInterval") {
            let _ = sesion.SetMinUpdateInterval(windows::Foundation::TimeSpan {
                Duration: minimo_entre,
            });
        }
        let _ = sesion.SetIsCursorCaptureEnabled(con_cursor);
        let captura = Captura {
            item,
            pool,
            sesion,
            token: Some(token),
            token_cierre,
            compartido,
            tamano: (size.Width as u32, size.Height as u32),
        };
        captura.sesion.StartCapture().map_err(fuera)?;
        Ok(captura)
    }

    /// El tamano de la fuente al empezar, en pixeles de verdad.
    pub fn tamano(&self) -> (u32, u32) {
        self.tamano
    }

    /// La hora del fotograma retenido, si lo hay (sin tomarlo).
    pub fn hora_retenida(&self) -> Option<i64> {
        cerrojo(&self.compartido.retenido).as_ref().map(|r| r.hora)
    }

    /// Toma el fotograma retenido. Quien lo toma lo suelta
    /// ([`Retenido::soltar`]).
    pub fn tomar(&self) -> Option<Retenido> {
        cerrojo(&self.compartido.retenido).take()
    }

    /// Si la ventana (o el monitor) se fue.
    pub fn cerrada(&self) -> bool {
        self.compartido.cerrada.load(Ordering::Acquire)
    }
}

impl Drop for Captura {
    fn drop(&mut self) {
        // Primero que no llegue nada mas; luego lo retenido; luego la
        // sesion y el pool. Un manejador que ya estuviera corriendo termina
        // contra un canal o un cerrojo vivos (los tiene el `Arc`).
        if let Some(t) = self.token.take() {
            let _ = self.pool.RemoveFrameArrived(t);
        }
        if let Some(t) = self.token_cierre.take() {
            let _ = self.item.RemoveClosed(t);
        }
        if let Some(r) = self.tomar() {
            r.soltar();
        }
        let _ = self.sesion.Close();
        let _ = self.pool.Close();
    }
}

fn hay_propiedad(nombre: &str) -> bool {
    windows::Foundation::Metadata::ApiInformation::IsPropertyPresent(
        &HSTRING::from("Windows.Graphics.Capture.GraphicsCaptureSession"),
        &HSTRING::from(nombre),
    )
    .unwrap_or(false)
}
