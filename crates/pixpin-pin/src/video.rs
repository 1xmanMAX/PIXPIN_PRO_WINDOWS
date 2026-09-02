//! El reproductor de video del pin (D63/D64): Media Foundation en modo
//! *frame server* sobre el dispositivo D3D11 compartido.
//!
//! `IMFMediaEngine` decodifica en sus hilos (por hardware cuando lo hay) y
//! nosotros solo preguntamos, al ritmo del temporizador del pin, si hay
//! fotograma nuevo; si lo hay, lo transferimos a una textura BGRA propia y
//! el pin la pinta como pinta una imagen. Ni una copia por la CPU.
//!
//! La carga es asincrona: los metadatos (tamano) y los errores llegan por
//! el callback `IMFMediaEngineNotify`, que corre en un hilo de Media
//! Foundation. Por eso el callback solo escribe atomicos y el pin los lee
//! en su tick; nada de tocar ventanas desde ahi.

use std::path::Path;
use std::sync::Arc;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{RECT, S_OK};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, ID3D11Device, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Media::MediaFoundation::{
    CLSID_MFMediaEngineClassFactory, IMFAttributes, IMFDXGIDeviceManager, IMFMediaEngine,
    IMFMediaEngineClassFactory, IMFMediaEngineNotify, IMFMediaEngineNotify_Impl,
    MF_MEDIA_ENGINE_CALLBACK, MF_MEDIA_ENGINE_DXGI_MANAGER, MF_MEDIA_ENGINE_EVENT,
    MF_MEDIA_ENGINE_EVENT_ERROR, MF_MEDIA_ENGINE_EVENT_LOADEDMETADATA,
    MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT, MF_VERSION, MFCreateAttributes, MFCreateDXGIDeviceManager,
    MFSTARTUP_LITE, MFStartup,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::core::{BSTR, Interface, implement};

use crate::ventana::ErrorPin;

/// Media Foundation se arranca una vez por proceso y no se apaga: el
/// reproductor puede nacer y morir muchas veces en una sesion.
static MF: Once = Once::new();

/// Lo que el callback comunica al hilo de interfaz.
#[derive(Default)]
struct Estado {
    metadatos: AtomicBool,
    error: AtomicBool,
}

/// El callback de Media Foundation. Corre en SUS hilos: solo atomicos.
#[implement(IMFMediaEngineNotify)]
struct Aviso {
    estado: Arc<Estado>,
}

impl IMFMediaEngineNotify_Impl for Aviso_Impl {
    fn EventNotify(&self, event: u32, _param1: usize, _param2: u32) -> windows::core::Result<()> {
        match MF_MEDIA_ENGINE_EVENT(event as i32) {
            MF_MEDIA_ENGINE_EVENT_LOADEDMETADATA => {
                self.estado.metadatos.store(true, Ordering::Release);
            }
            MF_MEDIA_ENGINE_EVENT_ERROR => {
                self.estado.error.store(true, Ordering::Release);
            }
            _ => {}
        }
        Ok(())
    }
}

pub struct Reproductor {
    motor: IMFMediaEngine,
    /// Vive lo que vive el motor: soltarlo antes rompe la decodificacion.
    _manager: IMFDXGIDeviceManager,
    d3d: ID3D11Device,
    estado: Arc<Estado>,
    /// La textura destino, creada con el tamano nativo en cuanto llegan los
    /// metadatos. El pin la envuelve como bitmap una sola vez.
    textura: Option<ID3D11Texture2D>,
    dimensiones: Option<(u32, u32)>,
}

impl Reproductor {
    /// Abre el archivo en bucle, silenciado y con reproduccion automatica
    /// (D63/D69). La carga es asincrona: `fallo()` se vuelve `true` si Media
    /// Foundation no puede con el (D72).
    pub fn nuevo(d3d: &ID3D11Device, ruta: &Path) -> Result<Reproductor, ErrorPin> {
        MF.call_once(|| {
            // SAFETY: arranque de Media Foundation, sin precondiciones; un
            // fallo aqui aparece despues como error de creacion del motor.
            unsafe {
                let _ = MFStartup(MF_VERSION, MFSTARTUP_LITE);
            }
        });

        // SAFETY: COM se inicializa (o se reutiliza) en el hilo del pin y
        // no se libera: el pin vive lo que vive la aplicacion. Todo lo que
        // se crea es propio y se suelta en `Drop` via `Shutdown`.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let mut token = 0u32;
            let mut manager: Option<IMFDXGIDeviceManager> = None;
            MFCreateDXGIDeviceManager(&mut token, &mut manager).map_err(ErrorPin::Video)?;
            let manager = manager.ok_or_else(|| ErrorPin::Video(windows::core::Error::empty()))?;
            manager.ResetDevice(d3d, token).map_err(ErrorPin::Video)?;

            let mut atributos: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut atributos, 3).map_err(ErrorPin::Video)?;
            let atributos =
                atributos.ok_or_else(|| ErrorPin::Video(windows::core::Error::empty()))?;

            let estado = Arc::new(Estado::default());
            let aviso: IMFMediaEngineNotify = Aviso {
                estado: Arc::clone(&estado),
            }
            .into();
            atributos
                .SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, &aviso)
                .map_err(ErrorPin::Video)?;
            atributos
                .SetUnknown(&MF_MEDIA_ENGINE_DXGI_MANAGER, &manager)
                .map_err(ErrorPin::Video)?;
            atributos
                .SetUINT32(
                    &MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT,
                    DXGI_FORMAT_B8G8R8A8_UNORM.0 as u32,
                )
                .map_err(ErrorPin::Video)?;

            let fabrica: IMFMediaEngineClassFactory =
                CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_INPROC_SERVER)
                    .map_err(ErrorPin::Video)?;
            let motor = fabrica
                .CreateInstance(0, &atributos)
                .map_err(ErrorPin::Video)?;

            motor.SetAutoPlay(true).map_err(ErrorPin::Video)?;
            motor.SetLoop(true).map_err(ErrorPin::Video)?;
            motor.SetMuted(true).map_err(ErrorPin::Video)?;
            let url = BSTR::from(ruta.to_string_lossy().as_ref());
            motor.SetSource(&url).map_err(ErrorPin::Video)?;

            Ok(Reproductor {
                motor,
                _manager: manager,
                d3d: d3d.clone(),
                estado,
                textura: None,
                dimensiones: None,
            })
        }
    }

    /// Si Media Foundation no pudo con el archivo (D72).
    pub fn fallo(&self) -> bool {
        self.estado.error.load(Ordering::Acquire)
    }

    /// El tamano nativo, en cuanto llegan los metadatos.
    pub fn dimensiones(&mut self) -> Option<(u32, u32)> {
        if self.dimensiones.is_none() && self.estado.metadatos.load(Ordering::Acquire) {
            let (mut cx, mut cy) = (0u32, 0u32);
            // SAFETY: punteros a locales; el motor esta vivo.
            let ok = unsafe { self.motor.GetNativeVideoSize(Some(&mut cx), Some(&mut cy)) };
            if ok.is_ok() && cx > 0 && cy > 0 {
                self.dimensiones = Some((cx, cy));
            }
        }
        self.dimensiones
    }

    /// Si hay fotograma nuevo, lo transfiere a la textura y la devuelve.
    /// `None` si no hay nada nuevo que pintar (o todavia no hay tamano).
    pub fn tick(&mut self) -> Option<&ID3D11Texture2D> {
        if self.fallo() {
            return None;
        }
        let (ancho, alto) = self.dimensiones()?;
        if self.textura.is_none() {
            self.textura = Some(crear_textura(&self.d3d, ancho, alto)?);
        }

        // OnVideoStreamTick devuelve S_OK con fotograma nuevo y S_FALSE sin
        // el; el envoltorio seguro pierde esa diferencia, asi que se llama
        // por la vtable.
        let mut pts: i64 = 0;
        // SAFETY: llamada directa al metodo COM con un puntero a local; el
        // motor esta vivo mientras `self` exista.
        let hr = unsafe {
            (Interface::vtable(&self.motor).OnVideoStreamTick)(
                Interface::as_raw(&self.motor),
                &mut pts,
            )
        };
        if hr != S_OK {
            return None;
        }

        let destino = RECT {
            left: 0,
            top: 0,
            right: ancho as i32,
            bottom: alto as i32,
        };
        let textura = self.textura.as_ref()?;
        // SAFETY: textura propia del tamano del video; el rect la cubre
        // entera; sin recorte de origen ni color de borde.
        let ok = unsafe { self.motor.TransferVideoFrame(textura, None, &destino, None) };
        if ok.is_err() {
            return None;
        }
        Some(textura)
    }

    pub fn reproduciendo(&self) -> bool {
        // SAFETY: consulta sobre el motor vivo.
        !unsafe { self.motor.IsPaused() }.as_bool()
    }

    pub fn alternar_pausa(&self) {
        // SAFETY: Play/Pause sobre el motor vivo; el resultado no importa
        // (un motor en error ya avisa por `fallo`).
        unsafe {
            if self.reproduciendo() {
                let _ = self.motor.Pause();
            } else {
                let _ = self.motor.Play();
            }
        }
    }

    pub fn silenciado(&self) -> bool {
        // SAFETY: consulta sobre el motor vivo.
        unsafe { self.motor.GetMuted() }.as_bool()
    }

    pub fn alternar_sonido(&self) {
        let ahora = !self.silenciado();
        // SAFETY: cambio de estado sobre el motor vivo.
        unsafe {
            let _ = self.motor.SetMuted(ahora);
        }
    }
}

impl Drop for Reproductor {
    fn drop(&mut self) {
        // SAFETY: Shutdown es el cierre ordenado del motor; sin el, los
        // hilos de decodificacion seguirian vivos con la ventana muerta.
        unsafe {
            let _ = self.motor.Shutdown();
        }
    }
}

/// La textura destino: BGRA, del tamano del video, dibujable por Direct2D.
fn crear_textura(d3d: &ID3D11Device, ancho: u32, alto: u32) -> Option<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: ancho,
        Height: alto,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut textura: Option<ID3D11Texture2D> = None;
    // SAFETY: descripcion completa y valida; puntero de salida a un local.
    unsafe {
        d3d.CreateTexture2D(&desc, None, Some(&mut textura)).ok()?;
    }
    textura
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn dispositivo_con_video() -> ID3D11Device {
        use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
        use windows::Win32::Graphics::Direct3D11::{
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_SDK_VERSION,
            D3D11CreateDevice, ID3D11Multithread,
        };
        let mut d3d = None;
        // SAFETY: creacion estandar con punteros a locales.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                windows::Win32::Foundation::HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                None,
            )
            .expect("dispositivo con soporte de video");
            let d3d: ID3D11Device = d3d.unwrap();
            let multi: ID3D11Multithread = d3d.cast().unwrap();
            let _ = multi.SetMultithreadProtected(true);
            d3d
        }
    }

    #[test]
    #[ignore = "necesita GPU y Media Foundation; ejecutar con --ignored"]
    fn un_archivo_inexistente_termina_en_fallo_y_sin_tamano() {
        // D72: el error llega por el callback, no al crear; el pin lo lee
        // en su tick y degrada a documento.
        let d3d = dispositivo_con_video();
        let mut r = Reproductor::nuevo(&d3d, Path::new(r"C:\no\existe\clip.mp4"))
            .expect("crear el motor no depende del archivo");
        let inicio = std::time::Instant::now();
        while !r.fallo() && inicio.elapsed() < std::time::Duration::from_secs(5) {
            let _ = r.tick();
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(r.fallo(), "deberia haber avisado del error");
        assert!(r.dimensiones().is_none());
    }

    #[test]
    #[ignore = "necesita GPU, Media Foundation y un video en Videos; --ignored"]
    fn un_video_real_entrega_tamano_y_fotogramas() {
        let Some(perfil) = std::env::var_os("USERPROFILE") else {
            return;
        };
        let ruta = std::path::PathBuf::from(perfil).join(r"Videos\2025-10-05 14-05-37.mkv");
        if !ruta.is_file() {
            eprintln!("sin video de prueba en {}; se omite", ruta.display());
            return;
        }
        let d3d = dispositivo_con_video();
        let mut r = Reproductor::nuevo(&d3d, &ruta).expect("motor");
        let inicio = std::time::Instant::now();
        let mut fotogramas = 0;
        while inicio.elapsed() < std::time::Duration::from_secs(8) && fotogramas < 3 {
            if r.tick().is_some() {
                fotogramas += 1;
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        assert!(!r.fallo(), "Media Foundation no pudo con el MKV");
        let (w, h) = r.dimensiones().expect("tamano tras los metadatos");
        assert!(w > 0 && h > 0);
        assert!(fotogramas >= 3, "solo llegaron {fotogramas} fotogramas");
        assert!(r.reproduciendo() && r.silenciado());
    }
}
