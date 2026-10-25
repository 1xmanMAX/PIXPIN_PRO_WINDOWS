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
//! - **Lo que se aprendio de OBS, Cap y los ejemplos de Microsoft** (ver
//!   `docs/investigacion/2026-10-25-motores-de-grabacion.md`):
//!   - el borde amarillo se pide quitar con
//!     `GraphicsCaptureAccess::RequestAccessAsync(Borderless)` antes de
//!     `IsBorderRequired = false` (`libobs-winrt/winrt-capture.cpp`,
//!     `Win32CaptureSample/App.cpp`); sin la peticion Windows 11 lo ignora;
//!   - con Windows 11 24H2 se piden las regiones cambiadas
//!     (`DirtyRegionMode::ReportOnly`) y un fotograma sin ninguna no
//!     despierta al motor: el cursor que parpadea en una diapositiva
//!     quieta ya no cuesta fotogramas;
//!   - una ventana minimizada (`IsIconic`) no da imagen buena: sus
//!     fotogramas se sueltan sin mirarlos y el video sigue con la ultima
//!     (`window-capture.c` de OBS hace lo mismo);
//!   - al cerrar se espera a que acabe el manejador que estuviera corriendo
//!     antes de cerrar el pool (Cap, `scap-direct3d`, `CallbackActivity`).
//! - Si la ventana grabada se cierra, o el monitor se desconecta, se apunta
//!   ([`Captura::cerrada`]); el motor decide (`crate::reapertura`).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureAccess,
    GraphicsCaptureAccessKind, GraphicsCaptureDirtyRegionMode, GraphicsCaptureItem,
    GraphicsCaptureSession,
};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::IsIconic;
use windows::core::{AgileReference, HSTRING, IInspectable, Interface};

use super::com;
use super::motor::Mensaje;
use crate::ritmo::{Region, trae_cambios};
use crate::{ErrorMotor, Fuente};

/// Superficies del pool. Dos bastan (ver la cabecera) y cada una es una
/// pantalla entera en BGRA: 8 MB a 1080p.
const SUPERFICIES: i32 = 2;

/// Lo que se espera, al cerrar, a que acabe un manejador en marcha.
const ESPERA_AL_CERRAR: Duration = Duration::from_millis(500);

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
}

impl crate::plataforma::Retenido for Retenido {
    fn hora(&self) -> i64 {
        self.hora
    }

    /// Lo suelta al pool. Se llama cuando la GPU ya tiene en su cola todo
    /// lo que lo lee.
    fn soltar(self) {
        let _ = self.fotograma.Close();
    }
}

fn soltar(r: Retenido) {
    crate::plataforma::Retenido::soltar(r);
}

/// Lo que comparten el manejador y el motor.
#[derive(Default)]
struct Compartido {
    retenido: Mutex<Option<Retenido>>,
    tamano: Mutex<(i32, i32)>,
    cerrada: AtomicBool,
    /// Ya no se quiere nada: el manejador suelta y se va.
    parado: AtomicBool,
    /// Manejadores corriendo ahora mismo.
    en_marcha: AtomicUsize,
    /// Si se pidieron las regiones cambiadas (y entonces se miran).
    con_regiones: AtomicBool,
    /// El siguiente fotograma pasa aunque no diga que cambio nada (el
    /// primero, o el primero tras recrear el pool).
    pasa_el_siguiente: AtomicBool,
}

fn cerrojo<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Cuenta un manejador en marcha mientras vive.
struct EnMarcha<'a>(&'a AtomicUsize);

impl<'a> EnMarcha<'a> {
    fn entrar(c: &'a AtomicUsize) -> EnMarcha<'a> {
        c.fetch_add(1, Ordering::AcqRel);
        EnMarcha(c)
    }
}

impl Drop for EnMarcha<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(crate) struct Captura {
    item: GraphicsCaptureItem,
    pool: Direct3D11CaptureFramePool,
    sesion: GraphicsCaptureSession,
    token: Option<i64>,
    token_cierre: Option<i64>,
    compartido: Arc<Compartido>,
    tamano: (u32, u32),
    fuente: Fuente,
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
        compartido.pasa_el_siguiente.store(true, Ordering::Release);
        let del_manejador = Arc::clone(&compartido);
        let ventana = match fuente {
            Fuente::Ventana(h) => Some(h),
            Fuente::Monitor(_) => None,
        };
        // El dispositivo WinRT no es `Send`; el manejador corre en otro hilo
        // y lo necesita para recrear el pool. Una referencia agil sirve.
        let para_recrear = AgileReference::new(winrt).map_err(fuera)?;
        let manejador =
            TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(move |pool, _| {
                let c = &del_manejador;
                let _en_marcha = EnMarcha::entrar(&c.en_marcha);
                let Some(pool) = pool.as_ref() else {
                    return Ok(());
                };
                // Se saca siempre: sin drenar el pool, Windows deja de dar.
                let Ok(fotograma) = pool.TryGetNextFrame() else {
                    return Ok(());
                };
                if c.parado.load(Ordering::Acquire) {
                    let _ = fotograma.Close();
                    return Ok(());
                }
                // Una ventana minimizada no tiene imagen buena: el video
                // sigue con la ultima.
                if let Some(h) = ventana
                    // SAFETY: consulta de solo lectura; con un manejador
                    // que ya no existe devuelve falso.
                    && unsafe { IsIconic(HWND(h as *mut _)) }.as_bool()
                {
                    let _ = fotograma.Close();
                    return Ok(());
                }
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
                    let mut t = cerrojo(&c.tamano);
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
                    if let Some(previo) = cerrojo(&c.retenido).take() {
                        soltar(previo);
                    }
                    c.pasa_el_siguiente.store(true, Ordering::Release);
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
                let contenido = (contenido.Width as u32, contenido.Height as u32);
                // Con las regiones cambiadas pedidas, uno sin ninguna es la
                // misma imagen: se suelta y no se despierta a nadie.
                if c.con_regiones.load(Ordering::Acquire)
                    && !c.pasa_el_siguiente.swap(false, Ordering::AcqRel)
                    && !trae_cambios(&regiones(&fotograma), contenido)
                {
                    let _ = fotograma.Close();
                    return Ok(());
                }
                let nuevo = Retenido {
                    fotograma,
                    hora,
                    contenido,
                };
                let previo = cerrojo(&c.retenido).replace(nuevo);
                // Fuera del cerrojo: soltar no tiene por que esperar al motor.
                if let Some(previo) = previo {
                    soltar(previo);
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
        let captura = Captura {
            item,
            pool,
            sesion,
            token: Some(token),
            token_cierre,
            compartido,
            tamano: (size.Width as u32, size.Height as u32),
            fuente,
        };
        captura.opciones(con_cursor, minimo_entre);
        captura.sesion.StartCapture().map_err(fuera)?;
        Ok(captura)
    }

    /// Las opciones de la sesion, **por capacidad y no por version**: cada
    /// una solo si este Windows la tiene. Ninguna es imprescindible.
    fn opciones(&self, con_cursor: bool, minimo_entre: i64) {
        let s = &self.sesion;
        // Sin borde amarillo (Windows 11). Primero se pide permiso: sin la
        // peticion la propiedad se ignora. Si se niega, ponerla es seguro y
        // vale en cuanto la politica cambie (lo dice Win32CaptureSample).
        if hay_propiedad("IsBorderRequired") {
            if hay_metodo(
                "Windows.Graphics.Capture.GraphicsCaptureAccess",
                "RequestAccessAsync",
            ) && let Ok(op) =
                GraphicsCaptureAccess::RequestAccessAsync(GraphicsCaptureAccessKind::Borderless)
            {
                let _ = op.join();
            }
            let _ = s.SetIsBorderRequired(false);
        }
        if hay_propiedad("IsCursorCaptureEnabled") {
            let _ = s.SetIsCursorCaptureEnabled(con_cursor);
        }
        // El tope de ritmo (Windows 11 24H2): la captura no despierta mas
        // deprisa de lo que se va a codificar.
        if hay_propiedad("MinUpdateInterval") {
            let _ = s.SetMinUpdateInterval(windows::Foundation::TimeSpan {
                Duration: minimo_entre,
            });
        }
        // Los menus y dialogos de la ventana grabada salen con ella.
        if matches!(self.fuente, Fuente::Ventana(_)) && hay_propiedad("IncludeSecondaryWindows") {
            let _ = s.SetIncludeSecondaryWindows(true);
        }
        if hay_propiedad("DirtyRegionMode")
            && s.SetDirtyRegionMode(GraphicsCaptureDirtyRegionMode::ReportOnly)
                .is_ok()
        {
            self.compartido.con_regiones.store(true, Ordering::Release);
        }
    }

    /// El tamano de la fuente al empezar, en pixeles de verdad.
    pub fn tamano(&self) -> (u32, u32) {
        self.tamano
    }
}

impl crate::plataforma::Captura for Captura {
    type Retenido = Retenido;

    fn hora_retenida(&self) -> Option<i64> {
        cerrojo(&self.compartido.retenido).as_ref().map(|r| r.hora)
    }

    fn tomar(&self) -> Option<Retenido> {
        cerrojo(&self.compartido.retenido).take()
    }

    fn cerrada(&self) -> bool {
        self.compartido.cerrada.load(Ordering::Acquire)
    }
}

impl Drop for Captura {
    fn drop(&mut self) {
        // Primero que no llegue nada mas y que el manejador que ya corre
        // acabe; luego lo retenido; luego la sesion y el pool. Un manejador
        // tardio termina contra un canal o un cerrojo vivos (los tiene el
        // `Arc`) y suelta lo que saque.
        self.compartido.parado.store(true, Ordering::Release);
        if let Some(t) = self.token.take() {
            let _ = self.pool.RemoveFrameArrived(t);
        }
        if let Some(t) = self.token_cierre.take() {
            let _ = self.item.RemoveClosed(t);
        }
        let desde = Instant::now();
        while self.compartido.en_marcha.load(Ordering::Acquire) > 0
            && desde.elapsed() < ESPERA_AL_CERRAR
        {
            std::thread::sleep(Duration::from_millis(1));
        }
        if let Some(r) = cerrojo(&self.compartido.retenido).take() {
            soltar(r);
        }
        let _ = self.sesion.Close();
        let _ = self.pool.Close();
    }
}

/// Las regiones cambiadas de un fotograma (vacio si no las da).
fn regiones(f: &Direct3D11CaptureFrame) -> Vec<Region> {
    let Ok(v) = f.DirtyRegions() else {
        // Sin poder preguntar, se da por cambiado: mejor un fotograma de
        // mas que uno de menos.
        return vec![(0, 0, 1, 1)];
    };
    let n = v.Size().unwrap_or(0);
    (0..n)
        .filter_map(|i| v.GetAt(i).ok())
        .map(|r| (r.X, r.Y, r.Width, r.Height))
        .collect()
}

fn hay_propiedad(nombre: &str) -> bool {
    windows::Foundation::Metadata::ApiInformation::IsPropertyPresent(
        &HSTRING::from("Windows.Graphics.Capture.GraphicsCaptureSession"),
        &HSTRING::from(nombre),
    )
    .unwrap_or(false)
}

fn hay_metodo(tipo: &str, nombre: &str) -> bool {
    windows::Foundation::Metadata::ApiInformation::IsMethodPresent(
        &HSTRING::from(tipo),
        &HSTRING::from(nombre),
    )
    .unwrap_or(false)
}
