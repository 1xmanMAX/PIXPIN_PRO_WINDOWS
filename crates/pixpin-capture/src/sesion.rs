//! Sesion WGC continua para el modo en vivo del overlay.
//!
//! El manejador de FrameArrived corre en un hilo del pool de WinRT. La
//! textura del ultimo fotograma aceptado se guarda bajo un Mutex, PISANDO
//! la anterior: una sola copia viva (presupuesto D18). El overlay se entera
//! por PostMessage y dibuja cuando quiere.
//!
//! El tope de FPS es el primer consumidor real del nivel de rendimiento:
//! en Ligero, refrescar a 60 Hz una previsualizacion sobre una iGPU
//! compartida roba la memoria y el bus que la captura final necesita.
//! TryGetNextFrame se llama TAMBIEN para los fotogramas descartados,
//! porque sin drenar el pool WGC deja de entregar; lo que se ahorra es la
//! copia, el lock y el aviso.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
use windows::core::Interface;

use crate::dispositivo::{Dispositivo, ErrorCaptura};
use crate::monitores::handle_de_monitor;

pub struct SesionViva {
    sesion: GraphicsCaptureSession,
    pool: Direct3D11CaptureFramePool,
    token: i64,
    ultimo: Arc<Mutex<Option<ID3D11Texture2D>>>,
    aceptados: Arc<AtomicU64>,
    /// Canal de vida: mantener el emisor vivo documenta que el manejador
    /// pertenece a esta sesion; se cierra al cerrar.
    _vida: mpsc::Sender<()>,
}

impl SesionViva {
    /// `minimo_entre_frames`: `Duration::ZERO` = sin tope (nivel Completo);
    /// `Ligero` pasa ~33 ms (30 fps). `notificar`: (hwnd crudo, mensaje) al
    /// que avisar con PostMessage por cada fotograma aceptado.
    pub fn nueva(
        dispositivo: &Dispositivo,
        id_monitor: u32,
        minimo_entre_frames: Duration,
        notificar: Option<(isize, u32)>,
    ) -> Result<SesionViva, ErrorCaptura> {
        let Some(handle) = handle_de_monitor(id_monitor) else {
            return Err(ErrorCaptura::MonitorDesconocido(id_monitor));
        };

        let interop: IGraphicsCaptureItemInterop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        // SAFETY: `handle` viene de la enumeracion de monitores y sigue
        // siendo valido; CreateForMonitor devuelve una interfaz nueva.
        let item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(handle)? };

        let tamano = item.Size()?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            dispositivo.winrt(),
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            SizeInt32 {
                Width: tamano.Width,
                Height: tamano.Height,
            },
        )?;

        let ultimo = Arc::new(Mutex::new(None));
        let aceptados = Arc::new(AtomicU64::new(0));
        let ultimo_hilo = Arc::clone(&ultimo);
        let aceptados_hilo = Arc::clone(&aceptados);
        let ultimo_instante: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
        let (vida, _guardia) = mpsc::channel::<()>();

        let manejador =
            TypedEventHandler::<Direct3D11CaptureFramePool, windows::core::IInspectable>::new(
                move |pool, _| {
                    let Some(pool) = pool.as_ref() else {
                        return Ok(());
                    };
                    // Drenar SIEMPRE, aunque el fotograma vaya a descartarse.
                    let Ok(fotograma) = pool.TryGetNextFrame() else {
                        return Ok(());
                    };

                    // Tope de FPS: descartar antes de copiar nada.
                    if minimo_entre_frames > Duration::ZERO {
                        let mut guarda = match ultimo_instante.lock() {
                            Ok(g) => g,
                            Err(_) => return Ok(()),
                        };
                        if let Some(previo) = *guarda {
                            if previo.elapsed() < minimo_entre_frames {
                                return Ok(());
                            }
                        }
                        *guarda = Some(Instant::now());
                    }

                    let Ok(superficie) = fotograma.Surface() else {
                        return Ok(());
                    };
                    if let Ok(acceso) = superficie.cast::<IDirect3DDxgiInterfaceAccess>() {
                        // SAFETY: `acceso` procede de la superficie del
                        // fotograma, viva en este ambito; GetInterface
                        // devuelve una interfaz con su propia cuenta de
                        // referencias, asi que la textura sobrevive al Mutex.
                        if let Ok(textura) = unsafe { acceso.GetInterface::<ID3D11Texture2D>() } {
                            if let Ok(mut u) = ultimo_hilo.lock() {
                                // Pisa la anterior: una sola copia viva (D18).
                                *u = Some(textura);
                            }
                            aceptados_hilo.fetch_add(1, Ordering::Relaxed);
                            if let Some((hwnd, msg)) = notificar {
                                // SAFETY: PostMessageW tolera ventanas ya
                                // destruidas; hwnd crudo por diseno (HWND no
                                // es Send).
                                unsafe {
                                    let _ = PostMessageW(
                                        Some(HWND(hwnd as *mut _)),
                                        msg,
                                        Default::default(),
                                        Default::default(),
                                    );
                                }
                            }
                        }
                    }
                    Ok(())
                },
            );

        let token = pool.FrameArrived(&manejador)?;
        let sesion = pool.CreateCaptureSession(&item)?;

        // Mismo tratamiento del borde y el cursor que la instantanea: por
        // capacidad, no por version.
        if let Ok(soportado) = windows::Foundation::Metadata::ApiInformation::IsPropertyPresent(
            &windows::core::HSTRING::from("Windows.Graphics.Capture.GraphicsCaptureSession"),
            &windows::core::HSTRING::from("IsBorderRequired"),
        ) {
            if soportado {
                let _ = sesion.SetIsBorderRequired(false);
            }
        }
        let _ = sesion.SetIsCursorCaptureEnabled(false);

        sesion.StartCapture()?;

        Ok(SesionViva {
            sesion,
            pool,
            token,
            ultimo,
            aceptados,
            _vida: vida,
        })
    }

    /// La textura del ultimo fotograma aceptado, si ya llego alguno.
    pub fn ultimo(&self) -> Option<ID3D11Texture2D> {
        self.ultimo.lock().ok().and_then(|g| g.clone())
    }

    /// Fotogramas aceptados desde el arranque de la sesion. Existe para el
    /// test del tope y para el HUD de metricas de S6.
    pub fn aceptados(&self) -> u64 {
        self.aceptados.load(Ordering::Relaxed)
    }

    pub fn cerrar(self) {
        let _ = self.pool.RemoveFrameArrived(self.token);
        let _ = self.sesion.Close();
        let _ = self.pool.Close();
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::dispositivo::Dispositivo;
    use crate::monitores::enumerar_monitores;

    fn preparar() -> (Dispositivo, u32) {
        let d = Dispositivo::nuevo().unwrap();
        let m = enumerar_monitores().unwrap();
        let principal = m.principal().unwrap().id;
        (d, principal)
    }

    use crate::pruebas_util::con_movimiento;

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn la_sesion_entrega_fotogramas_y_ultimo_los_pisa() {
        let (d, id) = preparar();
        let sesion = SesionViva::nueva(&d, id, Duration::ZERO, None).unwrap();
        // Esperar el primer fotograma con cota.
        let limite = Instant::now() + Duration::from_secs(3);
        let mut primero = None;
        while Instant::now() < limite {
            primero = sesion.ultimo();
            if primero.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(primero.is_some(), "no llego ningun fotograma en 3 s");
        sesion.cerrar();
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn el_tope_de_fps_descarta_fotogramas() {
        // Con tope de 10 fps, en un segundo no pueden aceptarse mas de ~12
        // (margen para el arranque). El caso negativo es la misma medida sin
        // tope, que en un monitor de 60 Hz acepta bastantes mas: si ambos
        // numeros salieran iguales, el tope no estaria funcionando.
        let (d, id) = preparar();

        let aceptados_sin_tope = con_movimiento(Duration::from_millis(1400), || {
            let sin_tope = SesionViva::nueva(&d, id, Duration::ZERO, None).unwrap();
            std::thread::sleep(Duration::from_secs(1));
            let n = sin_tope.aceptados();
            sin_tope.cerrar();
            n
        });

        let aceptados_con_tope = con_movimiento(Duration::from_millis(1400), || {
            let con_tope = SesionViva::nueva(&d, id, Duration::from_millis(100), None).unwrap();
            std::thread::sleep(Duration::from_secs(1));
            let n = con_tope.aceptados();
            con_tope.cerrar();
            n
        });

        assert!(
            aceptados_con_tope <= 12,
            "el tope de 10 fps acepto {aceptados_con_tope} fotogramas en 1 s"
        );
        assert!(
            aceptados_sin_tope > aceptados_con_tope,
            "sin tope ({aceptados_sin_tope}) deberia aceptar mas que con tope \
             ({aceptados_con_tope}); si el escritorio esta completamente estatico WGC puede no \
             entregar fotogramas: mueve algo en pantalla y repite"
        );
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn cerrar_detiene_la_entrega() {
        let (d, id) = preparar();
        let sesion = SesionViva::nueva(&d, id, Duration::ZERO, None).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        sesion.cerrar();
        // El criterio practico es que no entre en panico y termine: si la
        // sesion quedara viva, el dispositivo seguiria ocupado y las
        // siguientes pruebas se resentirian.
    }
}
