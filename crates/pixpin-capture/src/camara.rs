//! **La camara del equipo** (27-oct-2026): fotos a su resolucion entera y
//! clips de video cortos, para la reunion presencial de «Grabar clase».
//!
//! Va por `Windows.Media.Capture.MediaCapture`, que es lo que usa la camara
//! de Windows: elige el driver, pide el permiso de privacidad y sabe sacar
//! la foto del flujo de fotos de verdad cuando la camara lo tiene (a mucha
//! mas resolucion que el del video). Todo es sincrono (`join` de cada
//! operacion): quien lo use lo hace desde su propio hilo, y ese hilo tiene
//! que estar en el apartamento multihilo ([`Apartamento`]), porque las
//! operaciones acaban en el grupo de hilos del sistema.
//!
//! Lo que no se decide aqui (que camara, que resolucion, cuantos bits) lo
//! decide quien llama con [`Camara::modos`] y [`Camara::poner_modo`]: las
//! cuentas son puras y viven en `pixpin-proyecto::presencial`.
//!
//! - **La foto** sale en JPEG ([`Camara::foto_jpeg`]) al tamano del modo
//!   puesto, sin reducir: el JPEG de la propia camara cuando ya lo da asi y,
//!   si no, el del codificador de Windows con su calidad de siempre.
//! - **El clip** se graba a un MP4 ([`Camara::empezar_clip`]): H.264, o
//!   HEVC si se pide (y si el sistema no puede, H.264), a ritmo variable con
//!   los bits que se le den, y AAC.
//!
//! Este modulo no tiene `unsafe` salvo el del apartamento: las llamadas de
//! WinRT son seguras en `windows`.

use std::path::Path;

use windows::Devices::Enumeration::{DeviceClass, DeviceInformation, Panel};
use windows::Media::Capture::{
    MediaCapture, MediaCaptureInitializationSettings, MediaStreamType, PhotoCaptureSource,
    StreamingCaptureMode,
};
use windows::Media::MediaProperties::{
    IMediaEncodingProperties, ImageEncodingProperties, MediaEncodingProfile,
    VideoEncodingProperties, VideoEncodingQuality,
};
use windows::Storage::Streams::{DataReader, InMemoryRandomAccessStream};
use windows::Storage::{CreationCollisionOption, StorageFolder};
use windows::core::{HSTRING, Interface};

/// Por que no salio algo de la camara: el codigo de Windows (`HRESULT`,
/// para explicarlo con `pixpin_proyecto::presencial::fallo_de_codigo`) y su
/// texto, para el registro.
#[derive(Debug, Clone, thiserror::Error)]
#[error("camara: {mensaje} (0x{codigo:08X})")]
pub struct ErrorCamara {
    pub codigo: u32,
    pub mensaje: String,
}

impl From<windows::core::Error> for ErrorCamara {
    fn from(e: windows::core::Error) -> Self {
        ErrorCamara {
            codigo: e.code().0 as u32,
            mensaje: e.message().to_string(),
        }
    }
}

/// `MF_E_NO_CAPTURE_DEVICES_AVAILABLE`: el equipo no tiene camara.
const SIN_CAMARAS: u32 = 0xC00D_ABE0;

/// Donde esta la camara en el aparato, como lo dice Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LadoCamara {
    Delante,
    Detras,
    /// Una USB, o una que no dice donde esta.
    Fuera,
}

/// Una camara del equipo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoCamara {
    /// El identificador de Windows, para abrirla y recordarla.
    pub id: String,
    /// Su nombre («Integrated Camera», «Logitech BRIO»…).
    pub nombre: String,
    pub lado: LadoCamara,
}

/// **Las camaras que hay**, encendidas, en el orden de Windows. Vacio si no
/// hay ninguna.
pub fn camaras() -> Result<Vec<InfoCamara>, ErrorCamara> {
    let todas = DeviceInformation::FindAllAsyncDeviceClass(DeviceClass::VideoCapture)?.join()?;
    let mut v = Vec::new();
    for i in 0..todas.Size()? {
        let d = todas.GetAt(i)?;
        if !d.IsEnabled().unwrap_or(true) {
            continue;
        }
        let lado = match d.EnclosureLocation().and_then(|l| l.Panel()) {
            Ok(Panel::Front) => LadoCamara::Delante,
            Ok(Panel::Back) => LadoCamara::Detras,
            _ => LadoCamara::Fuera,
        };
        v.push(InfoCamara {
            id: d.Id()?.to_string(),
            nombre: d.Name().map(|n| n.to_string()).unwrap_or_default(),
            lado,
        });
    }
    Ok(v)
}

/// Que flujo de la camara.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flujo {
    /// El de las fotos (o, si la camara no tiene uno aparte, el de la vista
    /// previa, que es de donde salen entonces).
    Foto,
    /// El de grabar video.
    Video,
}

/// Una forma de dar imagen de un flujo: su tamano, su formato y sus
/// fotogramas por segundo (0 en las fotos).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModoCamara {
    pub ancho: u32,
    pub alto: u32,
    pub formato: String,
    pub fps: u32,
}

/// Como se graba un clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcionesClip {
    /// HEVC (si el sistema no puede, H.264).
    pub hevc: bool,
    /// Los bits por segundo de la imagen (el medio: va a ritmo variable).
    pub bits: u32,
    /// Los del sonido (AAC).
    pub bits_sonido: u32,
}

/// **Una camara abierta.** Se cierra al soltarla (y con ella el clip que
/// se estuviera grabando, que se para y se cierra bien).
pub struct Camara {
    captura: MediaCapture,
    /// Si se abrio con el microfono (para el sonido de los clips).
    con_sonido: bool,
    /// Grabando un clip, y si es HEVC.
    clip: Option<bool>,
}

impl Camara {
    /// **Abre la camara `id`.** Con `con_sonido`, tambien el microfono de
    /// siempre para los clips (compartido: la nota de voz de la reunion
    /// sigue grabando el suyo); si el microfono no se deja, se abre sin el.
    pub fn abrir(id: &str, con_sonido: bool) -> Result<Camara, ErrorCamara> {
        let abrir = |modo: StreamingCaptureMode| -> Result<MediaCapture, ErrorCamara> {
            let ajustes = MediaCaptureInitializationSettings::new()?;
            ajustes.SetVideoDeviceId(&HSTRING::from(id))?;
            ajustes.SetStreamingCaptureMode(modo)?;
            ajustes.SetPhotoCaptureSource(PhotoCaptureSource::Auto)?;
            let captura = MediaCapture::new()?;
            captura.InitializeWithSettingsAsync(&ajustes)?.join()?;
            Ok(captura)
        };
        if con_sonido {
            match abrir(StreamingCaptureMode::AudioAndVideo) {
                Ok(captura) => {
                    return Ok(Camara {
                        captura,
                        con_sonido: true,
                        clip: None,
                    });
                }
                // Lo que es de la camara (no esta, la tiene otra, no hay
                // permiso) no se arregla sin el microfono.
                Err(e) if e.codigo == SIN_CAMARAS => return Err(e),
                Err(_) => {}
            }
        }
        Ok(Camara {
            captura: abrir(StreamingCaptureMode::Video)?,
            con_sonido: false,
            clip: None,
        })
    }

    /// El tipo de flujo de Windows de `f`.
    fn tipo(&self, f: Flujo) -> MediaStreamType {
        match f {
            Flujo::Video => MediaStreamType::VideoRecord,
            Flujo::Foto => {
                // Sin flujo de fotos aparte, las fotos salen de la vista
                // previa: ese es el que hay que poner a lo mas grande.
                let fuente = self
                    .captura
                    .MediaCaptureSettings()
                    .and_then(|s| s.PhotoCaptureSource());
                match fuente {
                    Ok(PhotoCaptureSource::VideoPreview) => MediaStreamType::VideoPreview,
                    _ => match self.propiedades(MediaStreamType::Photo) {
                        Ok(v) if !v.is_empty() => MediaStreamType::Photo,
                        _ => MediaStreamType::VideoPreview,
                    },
                }
            }
        }
    }

    fn propiedades(
        &self,
        t: MediaStreamType,
    ) -> Result<Vec<IMediaEncodingProperties>, ErrorCamara> {
        let lista = self
            .captura
            .VideoDeviceController()?
            .GetAvailableMediaStreamProperties(t)?;
        let mut v = Vec::new();
        for i in 0..lista.Size()? {
            v.push(lista.GetAt(i)?);
        }
        Ok(v)
    }

    /// **Las formas de dar imagen del flujo `f`**, en el orden de la
    /// camara (el indice es el de [`Camara::poner_modo`]).
    pub fn modos(&self, f: Flujo) -> Vec<ModoCamara> {
        let Ok(v) = self.propiedades(self.tipo(f)) else {
            return Vec::new();
        };
        v.iter().map(modo_de).collect()
    }

    /// Pone el flujo `f` en su modo numero `i` de [`Camara::modos`].
    pub fn poner_modo(&self, f: Flujo, i: usize) -> Result<(), ErrorCamara> {
        let t = self.tipo(f);
        let v = self.propiedades(t)?;
        let Some(p) = v.get(i) else {
            return Err(ErrorCamara {
                codigo: 0x8000_FFFF,
                mensaje: "ese modo de la camara no existe".into(),
            });
        };
        self.captura
            .VideoDeviceController()?
            .SetMediaStreamPropertiesAsync(t, p)?
            .join()?;
        Ok(())
    }

    /// **Una foto en JPEG**, al tamano del flujo de fotos tal cual (sin
    /// reducir).
    pub fn foto_jpeg(&self) -> Result<Vec<u8>, ErrorCamara> {
        let flujo = InMemoryRandomAccessStream::new()?;
        self.captura
            .CapturePhotoToStreamAsync(&ImageEncodingProperties::CreateJpeg()?, &flujo)?
            .join()?;
        let tam = flujo.Size()?;
        let lector = DataReader::CreateDataReader(&flujo.GetInputStreamAt(0)?)?;
        let leidos = lector.LoadAsync(tam as u32)?.join()?;
        let mut bytes = vec![0u8; leidos as usize];
        lector.ReadBytes(&mut bytes)?;
        if bytes.is_empty() {
            return Err(ErrorCamara {
                codigo: 0x8000_4005,
                mensaje: "la camara dio una foto vacia".into(),
            });
        }
        Ok(bytes)
    }

    /// Si se esta grabando un clip.
    pub fn grabando_clip(&self) -> bool {
        self.clip.is_some()
    }

    /// **Empieza un clip** en `ruta` (un `.mp4`; se pisa si ya estaba). El
    /// flujo de video ya tiene que estar en su modo ([`Camara::poner_modo`]).
    /// Devuelve si va en HEVC.
    pub fn empezar_clip(&mut self, ruta: &Path, o: OpcionesClip) -> Result<bool, ErrorCamara> {
        if self.clip.is_some() {
            return Ok(self.clip == Some(true));
        }
        let carpeta = ruta.parent().ok_or_else(|| ErrorCamara {
            codigo: 0x8007_0003,
            mensaje: "el clip no tiene carpeta".into(),
        })?;
        let nombre = ruta.file_name().ok_or_else(|| ErrorCamara {
            codigo: 0x8007_007B,
            mensaje: "el clip no tiene nombre".into(),
        })?;
        let carpeta =
            StorageFolder::GetFolderFromPathAsync(&HSTRING::from(carpeta.as_os_str()))?.join()?;
        let fichero = carpeta
            .CreateFileAsync(
                &HSTRING::from(nombre),
                CreationCollisionOption::ReplaceExisting,
            )?
            .join()?;
        let intentos: &[bool] = if o.hevc { &[true, false] } else { &[false] };
        let mut ultimo = None;
        for &hevc in intentos {
            let hecho = self.perfil(hevc, o).and_then(|perfil| {
                self.captura
                    .StartRecordToStorageFileAsync(&perfil, &fichero)?
                    .join()?;
                Ok(())
            });
            match hecho {
                Ok(()) => {
                    self.clip = Some(hevc);
                    return Ok(hevc);
                }
                Err(e) => ultimo = Some(e),
            }
        }
        Err(ultimo.unwrap_or_else(|| ErrorCamara {
            codigo: 0x8000_4005,
            mensaje: "el clip no empezo".into(),
        }))
    }

    /// El perfil del clip: el tamano y los fotogramas del flujo de video tal
    /// como esta, los bits pedidos y, con microfono, AAC.
    fn perfil(&self, hevc: bool, o: OpcionesClip) -> Result<MediaEncodingProfile, ErrorCamara> {
        let perfil = if hevc {
            MediaEncodingProfile::CreateHevc(VideoEncodingQuality::Auto)?
        } else {
            MediaEncodingProfile::CreateMp4(VideoEncodingQuality::Auto)?
        };
        let video = perfil.Video()?;
        if let Ok(actual) = self
            .captura
            .VideoDeviceController()?
            .GetMediaStreamProperties(MediaStreamType::VideoRecord)?
            .cast::<VideoEncodingProperties>()
        {
            let (w, h) = (actual.Width()?, actual.Height()?);
            if w > 0 && h > 0 {
                video.SetWidth(w)?;
                video.SetHeight(h)?;
            }
            if let Ok(r) = actual.FrameRate()
                && r.Numerator()? > 0
                && r.Denominator()? > 0
            {
                let f = video.FrameRate()?;
                f.SetNumerator(r.Numerator()?)?;
                f.SetDenominator(r.Denominator()?)?;
            }
        }
        video.SetBitrate(o.bits.max(500_000))?;
        if self.con_sonido {
            perfil.Audio()?.SetBitrate(o.bits_sonido.max(32_000))?;
        } else {
            perfil.SetAudio(None)?;
        }
        Ok(perfil)
    }

    /// **Para el clip** y cierra su MP4. Sin clip, no hace nada.
    pub fn parar_clip(&mut self) -> Result<(), ErrorCamara> {
        if self.clip.take().is_none() {
            return Ok(());
        }
        self.captura.StopRecordAsync()?.join()?;
        Ok(())
    }
}

impl Drop for Camara {
    fn drop(&mut self) {
        if let Err(e) = self.parar_clip() {
            // Se suelta igual: lo grabado hasta aqui puede haber quedado sin
            // cerrar, y eso lo dice quien llama al no encontrar el fichero.
            let _ = e;
        }
        let _ = self.captura.Close();
    }
}

/// Lo que dice un modo de Windows: el de una imagen o el de un video.
fn modo_de(p: &IMediaEncodingProperties) -> ModoCamara {
    let formato = p.Subtype().map(|s| s.to_string()).unwrap_or_default();
    if let Ok(v) = p.cast::<VideoEncodingProperties>() {
        let fps = v
            .FrameRate()
            .ok()
            .and_then(|r| {
                let (n, d) = (r.Numerator().ok()?, r.Denominator().ok()?);
                (d > 0).then(|| (n + d / 2) / d)
            })
            .unwrap_or(0);
        return ModoCamara {
            ancho: v.Width().unwrap_or(0),
            alto: v.Height().unwrap_or(0),
            formato,
            fps,
        };
    }
    if let Ok(i) = p.cast::<ImageEncodingProperties>() {
        return ModoCamara {
            ancho: i.Width().unwrap_or(0),
            alto: i.Height().unwrap_or(0),
            formato,
            fps: 0,
        };
    }
    ModoCamara {
        ancho: 0,
        alto: 0,
        formato,
        fps: 0,
    }
}

/// **El apartamento multihilo** para el hilo de la camara, mientras viva.
/// Si el hilo ya tenia COM (de otro tipo), no se toca.
pub struct Apartamento {
    iniciado: bool,
    /// No cruza hilos: `CoUninitialize` va en el mismo que lo inicio.
    _no_send: std::marker::PhantomData<*const ()>,
}

impl Apartamento {
    pub fn iniciar() -> Apartamento {
        use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
        // SAFETY: sin punteros; se empareja con `CoUninitialize` en `Drop`
        // solo si fue bien (S_OK o S_FALSE), y en el mismo hilo: el guardia
        // no es `Send`.
        let r = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Apartamento {
            iniciado: r.is_ok(),
            _no_send: std::marker::PhantomData,
        }
    }
}

impl Drop for Apartamento {
    fn drop(&mut self) {
        if self.iniciado {
            // SAFETY: empareja el `CoInitializeEx` que fue bien, en el
            // mismo hilo (el guardia no cruza hilos).
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}
