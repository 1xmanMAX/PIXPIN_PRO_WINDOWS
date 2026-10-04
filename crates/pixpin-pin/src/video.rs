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
//!
//! El tick lo marca la pantalla, no un temporizador: un hilo duerme hasta
//! el proximo refresco que anuncia el compositor y le manda un mensaje a la
//! ventana. `SetTimer(16)` no daba 60 Hz: con la resolucion de reloj de
//! 15,6 ms de Windows salta cada dos periodos (~31 ms), y un video de
//! 60 fps se quedaba en la mitad de sus fotogramas.

use std::path::Path;
use std::sync::Arc;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{RECT, S_OK};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, ID3D11Device, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Media::MediaFoundation::{
    CLSID_MFMediaEngineClassFactory, IMFActivate, IMFAttributes, IMFDXGIDeviceManager,
    IMFMediaEngine, IMFMediaEngineClassFactory, IMFMediaEngineEx, IMFMediaEngineNotify,
    IMFMediaEngineNotify_Impl, MF_MEDIA_ENGINE_CALLBACK, MF_MEDIA_ENGINE_DXGI_MANAGER,
    MF_MEDIA_ENGINE_EVENT, MF_MEDIA_ENGINE_EVENT_ERROR, MF_MEDIA_ENGINE_EVENT_LOADEDMETADATA,
    MF_MEDIA_ENGINE_SEEK_MODE_APPROXIMATE, MF_MEDIA_ENGINE_SEEK_MODE_NORMAL,
    MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT, MF_MT_FRAME_SIZE, MF_MT_SUBTYPE,
    MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_VERSION, MFCreateAttributes, MFCreateDXGIDeviceManager,
    MFCreateSourceReaderFromURL, MFMediaType_Video, MFSTARTUP_LITE, MFStartup,
    MFT_CATEGORY_VIDEO_DECODER, MFT_ENUM_FLAG_ALL, MFT_REGISTER_TYPE_INFO, MFTEnumEx,
    MFVideoFormat_MPEG2,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};
use windows::core::{BSTR, GUID, HSTRING, Interface, implement};

use crate::ventana::ErrorPin;

/// Media Foundation se arranca una vez por proceso y no se apaga: el
/// reproductor puede nacer y morir muchas veces en una sesion.
static MF: Once = Once::new();

fn arrancar_mf() {
    MF.call_once(|| {
        // SAFETY: arranque de Media Foundation, sin precondiciones; un
        // fallo aqui aparece despues como error de creacion del motor.
        unsafe {
            let _ = MFStartup(MF_VERSION, MFSTARTUP_LITE);
        }
    });
}

/// El codec del video, sacado del subtipo de Media Foundation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    H264,
    Hevc,
    Vp8,
    Vp9,
    Av1,
    Mpeg2,
    /// Cualquier otro, con su FOURCC (o el primer campo del GUID) para el
    /// registro.
    Otro(u32),
}

/// Los subtipos de video de Media Foundation son `FOURCC-0000-0010-8000-
/// 00AA00389B71`; el MPEG-2 es la excepcion, con su GUID propio.
const COLA_FOURCC: u128 = 0x0000_0010_8000_00aa_0038_9b71;

const fn fourcc(t: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*t)
}

/// Pura: de subtipo de Media Foundation a codec.
pub fn codec_de_subtipo(subtipo: GUID) -> Codec {
    if subtipo == MFVideoFormat_MPEG2 {
        return Codec::Mpeg2;
    }
    let n = subtipo.to_u128();
    let cc = (n >> 96) as u32;
    // El H.264 «elemental» de los .ts tiene GUID propio.
    if subtipo == windows::Win32::Media::MediaFoundation::MFVideoFormat_H264_ES {
        return Codec::H264;
    }
    if n & ((1u128 << 96) - 1) != COLA_FOURCC {
        return Codec::Otro(cc);
    }
    match cc {
        x if x == fourcc(b"H264") || x == fourcc(b"h264") || x == fourcc(b"AVC1") => Codec::H264,
        x if x == fourcc(b"HEVC") || x == fourcc(b"HVC1") || x == fourcc(b"H265") => Codec::Hevc,
        x if x == fourcc(b"VP80") => Codec::Vp8,
        x if x == fourcc(b"VP90") => Codec::Vp9,
        x if x == fourcc(b"AV01") => Codec::Av1,
        x if x == fourcc(b"MPG2") => Codec::Mpeg2,
        x => Codec::Otro(x),
    }
}

/// La extension de Microsoft Store que trae el decodificador que falta.
/// Windows no las instala de serie: sin ella, Media Foundation abre el
/// contenedor pero no puede decodificar la imagen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtensionTienda {
    /// El nombre tal y como sale en la tienda (no se traduce: es lo que el
    /// usuario tiene que buscar).
    pub nombre: &'static str,
    /// Abre la ficha en la aplicacion de Microsoft Store.
    pub enlace: &'static str,
}

/// Pura: que extension de la tienda hace falta para un codec. `None` si no
/// tiene extension conocida (o viene de serie con Windows, como el H.264).
pub fn extension_para(codec: Codec) -> Option<ExtensionTienda> {
    match codec {
        Codec::Hevc => Some(ExtensionTienda {
            nombre: "HEVC Video Extensions",
            enlace: "ms-windows-store://pdp/?ProductId=9NMZLZ57R3T7",
        }),
        // La de VP9 trae tambien el VP8.
        Codec::Vp9 | Codec::Vp8 => Some(ExtensionTienda {
            nombre: "VP9 Video Extensions",
            enlace: "ms-windows-store://pdp/?ProductId=9N4D0MSMP0PT",
        }),
        Codec::Av1 => Some(ExtensionTienda {
            nombre: "AV1 Video Extension",
            enlace: "ms-windows-store://pdp/?ProductId=9MVZQVXJBQ9V",
        }),
        Codec::Mpeg2 => Some(ExtensionTienda {
            nombre: "MPEG-2 Video Extension",
            enlace: "ms-windows-store://pdp/?ProductId=9N95Q1ZZPMH4",
        }),
        Codec::H264 | Codec::Otro(_) => None,
    }
}

/// Lo que se sabe de un video sin reproducirlo. Se lee la cabecera con un
/// `IMFSourceReader` (unas decenas de ms) en vez de pedirle una miniatura a
/// la Shell, que decodifica un fotograma y tardaba cientos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FichaVideo {
    pub ancho: u32,
    pub alto: u32,
    pub codec: Codec,
    /// Si hay algun decodificador instalado para ese codec.
    pub decodificable: bool,
}

impl FichaVideo {
    /// La extension que falta instalar, si es eso lo que impide verlo.
    pub fn extension_que_falta(&self) -> Option<ExtensionTienda> {
        if self.decodificable {
            None
        } else {
            extension_para(self.codec)
        }
    }
}

/// Lee la cabecera del video. `None` si Media Foundation no sabe ni abrir
/// el contenedor (o no tiene pista de video).
pub fn examinar(ruta: &Path) -> Option<FichaVideo> {
    arrancar_mf();
    let url = HSTRING::from(ruta.as_os_str());
    // SAFETY: COM inicializado (o reutilizado) en este hilo; todo lo creado
    // es propio. El arreglo de MFTEnumEx se libera con CoTaskMemFree tras
    // soltar cada activador.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let lector = MFCreateSourceReaderFromURL(&url, None).ok()?;
        let tipo = lector
            .GetNativeMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, 0)
            .ok()?;
        let subtipo = tipo.GetGUID(&MF_MT_SUBTYPE).ok()?;
        let tam = tipo.GetUINT64(&MF_MT_FRAME_SIZE).unwrap_or(0);
        let (ancho, alto) = ((tam >> 32) as u32, tam as u32);

        let entrada = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Video,
            guidSubtype: subtipo,
        };
        let mut lista: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut cuantos = 0u32;
        let ok = MFTEnumEx(
            MFT_CATEGORY_VIDEO_DECODER,
            MFT_ENUM_FLAG_ALL,
            Some(&entrada),
            None,
            &mut lista,
            &mut cuantos,
        )
        .is_ok();
        if !lista.is_null() {
            for k in 0..cuantos as usize {
                std::ptr::drop_in_place(lista.add(k));
            }
            CoTaskMemFree(Some(lista as *const _));
        }
        Some(FichaVideo {
            ancho,
            alto,
            codec: codec_de_subtipo(subtipo),
            // Si no se pudo preguntar, se da por bueno: ya lo dira el motor.
            decodificable: !ok || cuantos > 0,
        })
    }
}

/// Lo que comparten la ventana y el hilo del ritmo.
#[derive(Default)]
struct Pulso {
    /// Si hay que avisar: el video avanza, o hay un salto que recoger.
    activo: AtomicBool,
    /// Hay un aviso mandado que la ventana aun no ha atendido.
    pendiente: AtomicBool,
    /// Separacion minima entre avisos, en ms (el nivel Ligero pide 33).
    minimo_ms: AtomicU32,
}

/// El hilo que marca el ritmo: espera al refresco del monitor y avisa a la
/// ventana con un mensaje. Solo uno en vuelo: si la ventana va atrasada no
/// se le amontonan. En pausa duerme.
struct Ritmo {
    vivo: Arc<AtomicBool>,
    hilo: Option<std::thread::JoinHandle<()>>,
}

/// Pura: cuanto falta (en ticks del contador de rendimiento) hasta el
/// proximo refresco, sabiendo cuando fue uno (`vblank`) y cada cuanto llegan
/// (`periodo`). Sirve un `vblank` del pasado o del futuro. Sin periodo, cero.
pub fn hasta_el_refresco(ahora: i64, vblank: i64, periodo: i64) -> i64 {
    if periodo <= 0 {
        return 0;
    }
    let fase = (ahora - vblank).rem_euclid(periodo);
    if fase == 0 { 0 } else { periodo - fase }
}

impl Ritmo {
    fn arrancar(hwnd: isize, mensaje: u32, pulso: Arc<Pulso>) -> Ritmo {
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::Graphics::Dwm::{DWM_TIMING_INFO, DwmGetCompositionTimingInfo};
        use windows::Win32::System::Performance::{
            QueryPerformanceCounter, QueryPerformanceFrequency,
        };
        use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

        let vivo = Arc::new(AtomicBool::new(true));
        let sigue = Arc::clone(&vivo);
        let hilo = std::thread::Builder::new()
            .name("pin-video-ritmo".into())
            .spawn(move || {
                let mut frecuencia = 0i64;
                // SAFETY: salida a un local; no falla desde Windows XP.
                let _ = unsafe { QueryPerformanceFrequency(&mut frecuencia) };
                let frecuencia = frecuencia.max(1);
                let mut ultimo = Instant::now() - Duration::from_secs(1);
                while sigue.load(Ordering::Acquire) {
                    if !pulso.activo.load(Ordering::Acquire) {
                        std::thread::park_timeout(Duration::from_millis(250));
                        continue;
                    }
                    // Se duerme hasta el proximo refresco que anuncia el
                    // compositor (mas 1 ms, para caer ya dentro del periodo).
                    // `thread::sleep` usa en Windows el temporizador de alta
                    // resolucion: no se redondea a 15,6 ms como SetTimer.
                    // `WaitForVBlank` no sirve: en portatiles con dos GPU
                    // vuelve al instante o se queda colgado, segun la salida.
                    let mut info = DWM_TIMING_INFO {
                        cbSize: std::mem::size_of::<DWM_TIMING_INFO>() as u32,
                        ..Default::default()
                    };
                    let mut ahora = 0i64;
                    // SAFETY: estructura propia con su tamano; sin ventana
                    // (desde Windows 8.1 es la del escritorio entero).
                    let espera = unsafe {
                        let ok = DwmGetCompositionTimingInfo(HWND::default(), &mut info).is_ok();
                        let _ = QueryPerformanceCounter(&mut ahora);
                        ok
                    }
                    .then(|| {
                        hasta_el_refresco(
                            ahora,
                            info.qpcVBlank as i64,
                            info.qpcRefreshPeriod as i64,
                        )
                    })
                    .filter(|_| info.qpcRefreshPeriod > 0)
                    .map(|ticks| {
                        Duration::from_micros(
                            (ticks as u128 * 1_000_000 / frecuencia as u128) as u64,
                        ) + Duration::from_millis(1)
                    })
                    // Sin compositor (no deberia pasar), un periodo de 60 Hz.
                    .unwrap_or(Duration::from_micros(16_667));
                    std::thread::sleep(espera.min(Duration::from_millis(50)));
                    let minimo = pulso.minimo_ms.load(Ordering::Relaxed) as u128;
                    // Con 2 ms de margen: a 30 fps sobre 60 Hz el refresco
                    // llega a los 33,3 ms, y no hay que esperar al tercero.
                    if ultimo.elapsed().as_millis() + 2 < minimo {
                        continue;
                    }
                    if pulso.pendiente.swap(true, Ordering::AcqRel) {
                        continue;
                    }
                    ultimo = Instant::now();
                    // SAFETY: mensaje propio; si la ventana ya no existe,
                    // PostMessage falla y el hilo termina.
                    let ok = unsafe {
                        PostMessageW(Some(HWND(hwnd as *mut _)), mensaje, WPARAM(0), LPARAM(0))
                    };
                    if ok.is_err() {
                        break;
                    }
                }
            })
            .ok();
        Ritmo { vivo, hilo }
    }

    fn despertar(&self) {
        if let Some(h) = &self.hilo {
            h.thread().unpark();
        }
    }
}

impl Drop for Ritmo {
    fn drop(&mut self) {
        self.vivo.store(false, Ordering::Release);
        if let Some(h) = self.hilo.take() {
            h.thread().unpark();
            // Como mucho espera un refresco o lo que quede de la siesta.
            let _ = h.join();
        }
    }
}

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
    pulso: Arc<Pulso>,
    ritmo: Option<Ritmo>,
    /// Cuando se creo, para medir lo que tarda el primer fotograma.
    nacido: Instant,
    primer_fotograma: bool,
    /// Fotogramas transferidos desde que nacio (registro y pruebas).
    fotogramas: u64,
    /// Lo pauso el usuario. Mientras carga, el motor tambien dice «en
    /// pausa» (la reproduccion automatica aun no ha empezado), y dormir el
    /// ritmo entonces dejaba el video congelado para siempre.
    pausado_a_mano: std::cell::Cell<bool>,
}

impl Reproductor {
    /// Abre el archivo en bucle, silenciado y con reproduccion automatica
    /// (D63/D69). La carga es asincrona: `fallo()` se vuelve `true` si Media
    /// Foundation no puede con el (D72).
    pub fn nuevo(d3d: &ID3D11Device, ruta: &Path) -> Result<Reproductor, ErrorPin> {
        arrancar_mf();

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
                pulso: Arc::new(Pulso {
                    activo: AtomicBool::new(true),
                    pendiente: AtomicBool::new(false),
                    minimo_ms: AtomicU32::new(0),
                }),
                ritmo: None,
                nacido: Instant::now(),
                primer_fotograma: false,
                fotogramas: 0,
                pausado_a_mano: std::cell::Cell::new(false),
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
        self.fotogramas += 1;
        if !self.primer_fotograma {
            self.primer_fotograma = true;
            tracing::info!(
                ms = self.nacido.elapsed().as_millis() as u64,
                ancho,
                alto,
                "primer fotograma del video"
            );
        }
        Some(textura)
    }

    /// Pone en marcha el hilo que marca el ritmo con el refresco del
    /// monitor: mandara `mensaje` a `hwnd` mientras el video avance, con al
    /// menos `minimo_ms` entre avisos. Quien recibe el mensaje llama a
    /// `atendido()` y luego a `tick()`.
    pub fn marcar_ritmo(&mut self, hwnd: isize, mensaje: u32, minimo_ms: u32) {
        self.pulso.minimo_ms.store(minimo_ms, Ordering::Relaxed);
        if self.ritmo.is_none() {
            self.ritmo = Some(Ritmo::arrancar(hwnd, mensaje, Arc::clone(&self.pulso)));
        }
    }

    /// Cuantos fotogramas se han transferido desde que nacio.
    pub fn fotogramas(&self) -> u64 {
        self.fotogramas
    }

    /// El aviso del ritmo ya se esta atendiendo: puede llegar el siguiente.
    pub fn atendido(&self) {
        self.pulso.pendiente.store(false, Ordering::Release);
    }

    /// Despierta el ritmo (para recoger un fotograma aunque este en pausa).
    fn avisar_ritmo(&self) {
        self.pulso.activo.store(true, Ordering::Release);
        if let Some(r) = &self.ritmo {
            r.despertar();
        }
    }

    /// En pausa, una vez recogido el fotograma de un salto, el hilo del
    /// ritmo vuelve a dormir. Se llama en cada tick.
    pub fn reposar_si_parado(&self) {
        // SAFETY: consulta sobre el motor vivo.
        let buscando = unsafe { self.motor.IsSeeking() }.as_bool();
        if self.pausado_a_mano.get() && !self.reproduciendo() && !buscando {
            self.pulso.activo.store(false, Ordering::Release);
        }
    }

    /// Posicion y duracion en segundos. La duracion es `None` mientras no se
    /// sepa (o si no tiene fin).
    pub fn posicion(&self) -> (f64, Option<f64>) {
        // SAFETY: consultas sobre el motor vivo.
        let (t, d) = unsafe { (self.motor.GetCurrentTime(), self.motor.GetDuration()) };
        let d = (d.is_finite() && d > 0.0).then_some(d);
        (t.max(0.0), d)
    }

    /// Salta a `segundos`. `aproximado` para arrastrar la barra: va al
    /// fotograma clave mas cercano, que es instantaneo.
    pub fn buscar(&self, segundos: f64, aproximado: bool) {
        let modo = if aproximado {
            MF_MEDIA_ENGINE_SEEK_MODE_APPROXIMATE
        } else {
            MF_MEDIA_ENGINE_SEEK_MODE_NORMAL
        };
        let destino = segundos.max(0.0);
        // SAFETY: llamadas sobre el motor vivo; sin la interfaz Ex se usa
        // el salto normal.
        unsafe {
            match self.motor.cast::<IMFMediaEngineEx>() {
                Ok(ex) => {
                    let _ = ex.SetCurrentTimeEx(destino, modo);
                }
                Err(_) => {
                    let _ = self.motor.SetCurrentTime(destino);
                }
            }
        }
        // En pausa, el fotograma nuevo tambien hay que recogerlo.
        self.avisar_ritmo();
    }

    /// Volumen de 0 a 1.
    pub fn volumen(&self) -> f64 {
        // SAFETY: consulta sobre el motor vivo.
        unsafe { self.motor.GetVolume() }
    }

    /// Cambia el volumen (0 a 1). Subirlo quita el silencio: es lo que se
    /// espera al girar la rueda hacia arriba.
    pub fn poner_volumen(&self, v: f64) {
        let v = v.clamp(0.0, 1.0);
        // SAFETY: cambios de estado sobre el motor vivo.
        unsafe {
            let _ = self.motor.SetVolume(v);
            if v > 0.0 && self.silenciado() {
                let _ = self.motor.SetMuted(false);
            }
        }
    }

    pub fn poner_silencio(&self, silencio: bool) {
        // SAFETY: cambio de estado sobre el motor vivo.
        unsafe {
            let _ = self.motor.SetMuted(silencio);
        }
    }

    /// Por que fallo, para el registro: el codigo de Media Foundation y el
    /// HRESULT del sistema.
    pub fn motivo_fallo(&self) -> Option<String> {
        // SAFETY: consultas sobre el motor vivo.
        unsafe {
            let e = self.motor.GetError().ok()?;
            let codigo = e.GetErrorCode();
            let sistema = e.GetExtendedErrorCode().err().map(|x| x.code().0 as u32);
            Some(format!(
                "codigo={codigo} hresult={:#010x}",
                sistema.unwrap_or(0)
            ))
        }
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
                self.pausado_a_mano.set(true);
            } else {
                let _ = self.motor.Play();
                self.pausado_a_mano.set(false);
            }
        }
        self.avisar_ritmo();
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
        // El hilo del ritmo primero: no debe avisar a una ventana sin motor.
        self.ritmo = None;
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

    fn subtipo(cc: &[u8; 4]) -> GUID {
        GUID::from_u128(((fourcc(cc) as u128) << 96) | COLA_FOURCC)
    }

    #[test]
    fn los_subtipos_de_media_foundation_se_reconocen() {
        use windows::Win32::Media::MediaFoundation::{
            MFVideoFormat_AV1, MFVideoFormat_H264, MFVideoFormat_HEVC, MFVideoFormat_VP90,
        };
        assert_eq!(codec_de_subtipo(MFVideoFormat_H264), Codec::H264);
        assert_eq!(codec_de_subtipo(MFVideoFormat_HEVC), Codec::Hevc);
        assert_eq!(codec_de_subtipo(MFVideoFormat_VP90), Codec::Vp9);
        assert_eq!(codec_de_subtipo(MFVideoFormat_AV1), Codec::Av1);
        assert_eq!(codec_de_subtipo(MFVideoFormat_MPEG2), Codec::Mpeg2);
        assert_eq!(codec_de_subtipo(subtipo(b"VP80")), Codec::Vp8);
    }

    #[test]
    fn caso_negativo_un_guid_que_no_es_fourcc_es_otro() {
        // Mismo primer campo que el H.264 pero otra cola: no es un FOURCC.
        let raro = GUID::from_u128(((fourcc(b"H264") as u128) << 96) | 0x1234);
        assert_eq!(codec_de_subtipo(raro), Codec::Otro(fourcc(b"H264")));
        assert_eq!(
            codec_de_subtipo(subtipo(b"WMV3")),
            Codec::Otro(fourcc(b"WMV3"))
        );
    }

    #[test]
    fn cada_codec_de_pago_apunta_a_su_extension_de_la_tienda() {
        assert_eq!(
            extension_para(Codec::Hevc).unwrap().nombre,
            "HEVC Video Extensions"
        );
        assert_eq!(extension_para(Codec::Vp9), extension_para(Codec::Vp8));
        assert_eq!(
            extension_para(Codec::Av1).unwrap().nombre,
            "AV1 Video Extension"
        );
        for c in [Codec::Hevc, Codec::Vp9, Codec::Av1, Codec::Mpeg2] {
            assert!(
                extension_para(c)
                    .unwrap()
                    .enlace
                    .starts_with("ms-windows-store://pdp/")
            );
        }
    }

    #[test]
    fn caso_negativo_lo_que_trae_windows_no_pide_extension() {
        assert_eq!(extension_para(Codec::H264), None);
        assert_eq!(extension_para(Codec::Otro(0)), None);
        // Y con decodificador instalado, aunque sea HEVC, no falta nada.
        let f = FichaVideo {
            ancho: 1,
            alto: 1,
            codec: Codec::Hevc,
            decodificable: true,
        };
        assert_eq!(f.extension_que_falta(), None);
        let sin = FichaVideo {
            decodificable: false,
            ..f
        };
        assert_eq!(sin.extension_que_falta(), extension_para(Codec::Hevc));
    }

    /// La tabla de formatos de este equipo: para cada archivo de
    /// `PIXPIN_VIDEOS_PRUEBA` (una carpeta), que dice la cabecera y cuanto
    /// tarda el primer fotograma. Informativa: imprime y no falla.
    #[test]
    #[ignore = "necesita GPU, Media Foundation y PIXPIN_VIDEOS_PRUEBA; --ignored --nocapture"]
    fn tabla_de_formatos_de_este_equipo() {
        let Some(dir) = std::env::var_os("PIXPIN_VIDEOS_PRUEBA") else {
            return;
        };
        let d3d = dispositivo_con_video();
        let mut rutas: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        rutas.sort();
        for ruta in rutas {
            let t0 = Instant::now();
            let ficha = examinar(&ruta);
            let ms_cabecera = t0.elapsed().as_millis();
            let t1 = Instant::now();
            let mut r = Reproductor::nuevo(&d3d, &ruta).expect("motor");
            let mut primero = None;
            let mut fotogramas = 0u32;
            while t1.elapsed() < Duration::from_secs(4) && !r.fallo() {
                if r.tick().is_some() {
                    fotogramas += 1;
                    primero.get_or_insert(t1.elapsed().as_millis());
                }
                std::thread::sleep(Duration::from_millis(4));
            }
            println!(
                "{:<14} cabecera={:?} ({ms_cabecera} ms) primer_fotograma_ms={:?} fotogramas_en_4s={fotogramas} fallo={} {:?}",
                ruta.file_name().unwrap().to_string_lossy(),
                ficha,
                primero,
                r.fallo(),
                r.motivo_fallo()
            );
        }
    }

    #[test]
    fn el_refresco_siguiente_se_calcula_desde_cualquiera() {
        // Periodo 100: desde un refresco en 1000, a las 1030 faltan 70.
        assert_eq!(hasta_el_refresco(1030, 1000, 100), 70);
        // Un refresco anunciado en el futuro vale igual.
        assert_eq!(hasta_el_refresco(1030, 1300, 100), 70);
        assert_eq!(
            hasta_el_refresco(1100, 1000, 100),
            0,
            "justo en el refresco"
        );
    }

    #[test]
    fn caso_negativo_sin_periodo_no_se_espera() {
        assert_eq!(hasta_el_refresco(1030, 1000, 0), 0);
        assert_eq!(hasta_el_refresco(1030, 1000, -5), 0);
    }
}
