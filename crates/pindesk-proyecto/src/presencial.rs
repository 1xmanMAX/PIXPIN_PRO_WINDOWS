//! **Grabar una reunion presencial** (27-oct-2026, el usuario: «supon que
//! estoy en una reunion PRESENCIAL: que siga grabando aunque haya salido de
//! la app, que aparezca un boton flotante para tomar FOTOS de la reunion y,
//! como las capturas, que todo se adjunte a la reunion; y las fotos y las
//! capturas a resolucion COMPLETA, para que se vean mas detalles»).
//!
//! Lo puro de «Grabar clase» con su tercer modo, sin camara ni ventana, para
//! probarlo en milisegundos y para que el movil haga las mismas cuentas:
//!
//! - **Los tres modos** ([`Modo`]): que se recuerda, cual va despues y que
//!   sonidos llevan encendidos al elegirlos (el presencial, solo el
//!   microfono: en una sala no hay Meet que oir).
//! - **Que camara** ([`camara_elegida`]): la recordada si sigue; si no, la
//!   de atras (en una tableta es la que mira a la sala), una USB y, al final,
//!   la de delante (la del portatil, que mira a quien graba).
//! - **A que resolucion** ([`mejor_modo`]): la de mas pixeles de las que da
//!   la camara. Nunca se reduce: la foto se guarda tal cual sale.
//! - **El minuto de cada foto** ([`ms_de_la_foto`]) y **su nombre**
//!   ([`nombre_de_la_foto`]).
//! - **Que una imagen salio de la camara** ([`marcar_de_camara`]): en el
//!   `resto` del mensaje, `fuente: "camara"` (el movil, `"movil"`). Es el unico campo nuevo; lo
//!   demas (`enGrabacion: {de, ms}`) es lo de las capturas.
//! - **Los clips de video** con la camara ([`modo_para_clip`],
//!   [`bits_del_clip`], [`poner_clips`]): 1080p a 30 fps como mucho, a ritmo
//!   variable (~5 Mbps en H.264, un 30 % menos en HEVC), y en la nota de
//!   voz `clips: [{ms, id, duracionMs}]`; cada clip lleva `enGrabacion:
//!   {de, ms, duracionMs}`.
//! - **Cuando hace falta la imagen entera** al acercarla en «Estudiar la
//!   clase» ([`hace_falta_la_entera`]).

use serde_json::{Map, Value};

/// **Que se graba**: el selector de arriba de «Grabar clase».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// El video de la pantalla con su sonido.
    Pantalla,
    /// Solo el sonido, con capturas de la pantalla entera.
    Capturas,
    /// Una reunion en persona: el microfono y fotos con la camara.
    Presencial,
}

impl Modo {
    /// Los tres, en el orden del selector.
    pub const TODOS: [Modo; 3] = [Modo::Pantalla, Modo::Capturas, Modo::Presencial];

    /// La palabra con que se recuerda.
    pub fn palabra(self) -> &'static str {
        match self {
            Modo::Pantalla => "pantalla",
            Modo::Capturas => "capturas",
            Modo::Presencial => "presencial",
        }
    }

    /// Lo recordado. Lo que no se entiende (o nada) es el modo de capturas:
    /// el que menos ocupa, que era el de siempre.
    pub fn de(palabra: &str) -> Modo {
        match palabra.trim() {
            "pantalla" => Modo::Pantalla,
            "presencial" => Modo::Presencial,
            _ => Modo::Capturas,
        }
    }

    /// El siguiente del selector (las flechas y el tabulador), en rueda.
    pub fn siguiente(self) -> Modo {
        match self {
            Modo::Pantalla => Modo::Capturas,
            Modo::Capturas => Modo::Presencial,
            Modo::Presencial => Modo::Pantalla,
        }
    }

    /// El anterior del selector, en rueda.
    pub fn anterior(self) -> Modo {
        match self {
            Modo::Pantalla => Modo::Presencial,
            Modo::Capturas => Modo::Pantalla,
            Modo::Presencial => Modo::Capturas,
        }
    }

    /// **Los sonidos al elegir el modo**, `(microfono, equipo)`. En persona
    /// no hay reunion en el equipo que oir: el sonido del equipo se apaga
    /// (se puede volver a encender). En los otros dos, los dos.
    pub fn sonidos(self) -> (bool, bool) {
        match self {
            Modo::Presencial => (true, false),
            Modo::Pantalla | Modo::Capturas => (true, true),
        }
    }

    /// Si se graba el video de la pantalla.
    pub fn con_video(self) -> bool {
        self == Modo::Pantalla
    }

    /// Si el boton grande de la barra es «Foto» (la camara) y no «Captura».
    pub fn con_camara(self) -> bool {
        self == Modo::Presencial
    }
}

// --- La camara --------------------------------------------------------------

/// Donde esta la camara en el aparato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lado {
    /// La de la pantalla: mira a quien graba.
    Delante,
    /// La de atras de una tableta: mira a la sala.
    Detras,
    /// Una suelta (USB) o que no dice donde esta.
    Fuera,
}

impl Lado {
    /// Lo que se prefiere para una reunion en persona: menos es mejor.
    fn preferencia(self) -> u8 {
        match self {
            Lado::Detras => 0,
            Lado::Fuera => 1,
            Lado::Delante => 2,
        }
    }
}

/// **La camara con que se empieza**: la recordada si sigue enchufada; si no,
/// la que mejor mira a la sala (atras, luego una USB, luego la de delante)
/// y, entre iguales, la primera que da el sistema. `None` sin camaras.
pub fn camara_elegida(camaras: &[(String, Lado)], recordada: Option<&str>) -> Option<usize> {
    if let Some(r) = recordada.map(str::trim).filter(|r| !r.is_empty())
        && let Some(i) = camaras.iter().position(|(id, _)| id == r)
    {
        return Some(i);
    }
    camaras
        .iter()
        .enumerate()
        .min_by_key(|(i, (_, lado))| (lado.preferencia(), *i))
        .map(|(i, _)| i)
}

/// Una forma de dar imagen que tiene la camara: su tamano y su formato
/// (`MJPG`, `NV12`, `YUY2`, `JPEG`…, como lo dice el sistema).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModoDeCamara {
    pub ancho: u32,
    pub alto: u32,
    pub formato: String,
    /// Fotogramas por segundo (0 si no lo dice: las fotos no tienen).
    pub fps: u32,
}

impl ModoDeCamara {
    fn pixeles(&self) -> u64 {
        self.ancho as u64 * self.alto as u64
    }

    /// Si ya llega comprimida en JPEG: la foto sale tal cual, sin volver a
    /// comprimir.
    fn en_jpeg(&self) -> bool {
        matches!(
            self.formato.trim().to_ascii_uppercase().as_str(),
            "MJPG" | "JPEG" | "JPG"
        )
    }
}

/// **La resolucion de las fotos: la mayor que tenga la camara.** Gana la de
/// mas pixeles; entre dos iguales, la que ya llega en JPEG (sin volver a
/// comprimir). Los modos sin tamano no cuentan. `None` si no queda ninguno:
/// entonces se deja la de la camara por defecto.
pub fn mejor_modo(modos: &[ModoDeCamara]) -> Option<usize> {
    modos
        .iter()
        .enumerate()
        .filter(|(_, m)| m.ancho > 0 && m.alto > 0)
        .max_by_key(|(i, m)| (m.pixeles(), m.en_jpeg(), std::cmp::Reverse(*i)))
        .map(|(i, _)| i)
}

/// El tope de los clips de video: 1080p a 30 fps. Mas no se ve mejor en una
/// reunion y pesa el doble.
pub const CLIP_ANCHO_MAXIMO: u32 = 1920;
pub const CLIP_ALTO_MAXIMO: u32 = 1080;
pub const CLIP_FPS_MAXIMO: u32 = 30;

/// **El modo de los clips de video**: el de mas pixeles que no pase de
/// 1080p (en cualquiera de los dos lados, la camara puede ir de pie) ni de
/// 30 fps; entre iguales, el de mas fotogramas (hasta 30) y luego el que no
/// llega en JPEG (el codificador lo quiere sin comprimir). `None` si no hay
/// ninguno asi: entonces se graba con el de la camara por defecto.
pub fn modo_para_clip(modos: &[ModoDeCamara]) -> Option<usize> {
    modos
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            m.ancho > 0
                && m.alto > 0
                && m.ancho.max(m.alto) <= CLIP_ANCHO_MAXIMO
                && m.ancho.min(m.alto) <= CLIP_ALTO_MAXIMO
                && m.fps <= CLIP_FPS_MAXIMO
        })
        .max_by_key(|(i, m)| (m.pixeles(), m.fps, !m.en_jpeg(), std::cmp::Reverse(*i)))
        .map(|(i, _)| i)
}

/// **Lo que pesa un clip por segundo de imagen**, en bits: ~5 Mbps a 1080p
/// y en proporcion por debajo (720p, ~2,5), nunca menos de 1 Mbps. Con HEVC
/// un 30 % menos para la misma calidad. Se codifica a ritmo variable: es el
/// medio, no el tope.
pub fn bits_del_clip(ancho: u32, alto: u32, hevc: bool) -> u32 {
    let pixeles = ancho as u64 * alto as u64;
    let base = 1920u64 * 1080;
    let h264 = (5_000_000u64 * pixeles / base.max(1)).clamp(1_000_000, 6_000_000);
    let bits = if hevc { h264 * 7 / 10 } else { h264 };
    bits.max(1_000_000) as u32
}

/// El sonido de los clips: AAC a 96 kbps.
pub const CLIP_BITS_SONIDO: u32 = 96_000;

/// Un clip de video grabado en la reunion: cuando empezo, cuanto dura y su
/// mensaje.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clip {
    pub ms: i64,
    pub duracion_ms: i64,
    pub id: String,
}

/// La clave de los clips en la nota de voz.
pub const CLIPS: &str = "clips";

/// Lo que se apunta en la nota de voz: sus clips en orden,
/// `[{ms, id, duracionMs}]`. Sin clips **la clave se quita**: una clase sin
/// ellos sigue siendo byte a byte la de antes.
pub fn poner_clips(resto: &mut Map<String, Value>, clips: &[Clip]) {
    if clips.is_empty() {
        resto.remove(CLIPS);
        return;
    }
    let mut v: Vec<&Clip> = clips.iter().collect();
    v.sort_by_key(|c| c.ms);
    resto.insert(
        CLIPS.into(),
        Value::Array(
            v.into_iter()
                .map(|c| serde_json::json!({ "ms": c.ms, "id": c.id, "duracionMs": c.duracion_ms }))
                .collect(),
        ),
    );
}

/// Los clips de una nota de voz, en orden. Lo que no se entiende se salta.
pub fn clips_de(resto: &Map<String, Value>) -> Vec<Clip> {
    let mut v: Vec<Clip> = resto
        .get(CLIPS)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    Some(Clip {
                        ms: c.get("ms")?.as_i64()?.max(0),
                        id: c.get("id")?.as_str()?.to_string(),
                        duracion_ms: c
                            .get("duracionMs")
                            .and_then(Value::as_i64)
                            .unwrap_or(0)
                            .max(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|c| c.ms);
    v
}

/// **El enlace de un clip con su grabacion**: `enGrabacion: {de, ms,
/// duracionMs}` (el de las capturas, con su duracion).
pub fn enlace_del_clip(de: &str, ms: i64, duracion_ms: i64) -> Value {
    serde_json::json!({ "de": de, "ms": ms.max(0), "duracionMs": duracion_ms.max(0) })
}

/// **Por que no hay foto**: lo que se le dice a quien graba.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalloDeCamara {
    /// El equipo no tiene camara (o esta desactivada).
    SinCamara,
    /// Otra aplicacion la esta usando (Teams, Zoom…).
    EnUso,
    /// La privacidad de Windows no deja a las aplicaciones usar la camara.
    SinPermiso,
    /// Cualquier otra cosa: se reintenta en la siguiente foto.
    Otro,
}

/// **El fallo de la camara a partir del codigo de Windows** (`HRESULT`).
/// Son los que da `MediaCapture` en los casos que se pueden explicar.
pub fn fallo_de_codigo(hresult: u32) -> FalloDeCamara {
    match hresult {
        // E_ACCESSDENIED: la privacidad dice que no.
        0x8007_0005 => FalloDeCamara::SinPermiso,
        // MF_E_NO_CAPTURE_DEVICES_AVAILABLE, MF_E_UNSUPPORTED_CAPTURE_
        // DEVICE_PRESENT y el dispositivo que ya no esta
        // (ERROR_FILE_NOT_FOUND, ERROR_DEVICE_NOT_CONNECTED,
        // MF_E_VIDEO_RECORDING_DEVICE_INVALIDATED).
        0xC00D_ABE0 | 0xC00D_ABED | 0x8007_0002 | 0x8007_048F | 0xC00D_3EA2 => {
            FalloDeCamara::SinCamara
        }
        // MF_E_HW_MFT_FAILED_START_STREAMING, MF_E_VIDEO_RECORDING_DEVICE_
        // PREEMPTED, ERROR_SHARING_VIOLATION y ERROR_BUSY: la tiene otra.
        0xC00D_3704 | 0xC00D_3EA3 | 0x8007_0020 | 0x8007_00AA => FalloDeCamara::EnUso,
        _ => FalloDeCamara::Otro,
    }
}

// --- Las fotos de la grabacion ---------------------------------------------

/// **El minuto de una foto**: el de cuando se pulso (no el de cuando la
/// camara la devolvio), sin pasar de lo grabado.
pub fn ms_de_la_foto(llevado_al_pulsar: i64, duracion: Option<i64>) -> i64 {
    let ms = llevado_al_pulsar.max(0);
    match duracion {
        Some(d) => ms.min(d.max(0)),
        None => ms,
    }
}

/// **El nombre del fichero de una imagen de la grabacion**: `foto-…` las de
/// la camara y `clase-…` las capturas, con la hora en que se guardo y su
/// numero (uno por foto: dos guardadas en el mismo milisegundo no se
/// pisan). La extension, la de lo que se guarda (`png`, `jpg`).
pub fn nombre_de_la_foto(
    de_camara: bool,
    cuando_ms: i64,
    numero: usize,
    extension: &str,
) -> String {
    let tronco = if de_camara { "foto" } else { "clase" };
    let extension = extension.trim().trim_start_matches('.');
    let extension = if extension.is_empty() {
        "png"
    } else {
        extension
    };
    format!(
        "{tronco}-{cuando_ms}-{numero}.{}",
        extension.to_ascii_lowercase()
    )
}

/// **La clave de donde salio una imagen o un clip** en `resto`:
/// `fuente`. **No `origen`**: ese nombre ya es un campo del mensaje
/// (`Mensaje::origen`, el aparato de donde se copio) y, escrito tambien en
/// `resto`, la linea del cuaderno salia con `"origen"` dos veces y **no se
/// podia volver a leer** (el mensaje desaparecia del chat). Lo escrito asi
/// se rescata al leer (`cuaderno::Cuaderno::leer`).
pub const FUENTE: &str = "fuente";
/// Su valor para lo de la camara del equipo (reunion presencial).
pub const DE_CAMARA: &str = "camara";
/// Su valor para lo de la camara del movil (`camara_del_movil`).
pub const DEL_MOVIL: &str = "movil";

/// Apunta en el `resto` de una imagen que salio de la camara.
pub fn marcar_de_camara(resto: &mut Map<String, Value>) {
    resto.insert(FUENTE.into(), Value::String(DE_CAMARA.into()));
}

/// Apunta en el `resto` de una foto o un clip que mando el movil.
pub fn marcar_del_movil(resto: &mut Map<String, Value>) {
    resto.insert(FUENTE.into(), Value::String(DEL_MOVIL.into()));
}

/// De donde salio: `camara`, `movil` o nada (la pantalla, y todo lo de
/// antes).
pub fn fuente_de(resto: &Map<String, Value>) -> Option<&str> {
    resto.get(FUENTE).and_then(Value::as_str)
}

/// Si una imagen salio de la camara (y no de la pantalla).
pub fn es_de_camara(resto: &Map<String, Value>) -> bool {
    fuente_de(resto) == Some(DE_CAMARA)
}

/// Si una foto o un clip lo mando el movil.
pub fn es_del_movil(resto: &Map<String, Value>) -> bool {
    fuente_de(resto) == Some(DEL_MOVIL)
}

// --- La resolucion completa al estudiar -------------------------------------

/// **Si hay que cargar la imagen entera** en «Estudiar la clase»: cuando se
/// pinta mas ancha que lo cargado (acercada) y la de verdad tiene mas
/// pixeles que lo cargado. Con un margen del 5 %: un pixel de mas al
/// redondear no carga una foto de 12 megapixeles.
pub fn hace_falta_la_entera(ancho_pintado: f32, ancho_cargado: u32, ancho_de_verdad: u32) -> bool {
    ancho_de_verdad > ancho_cargado && ancho_pintado > ancho_cargado as f32 * 1.05
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_modo_se_recuerda_por_su_palabra_y_da_la_vuelta() {
        for m in Modo::TODOS {
            assert_eq!(Modo::de(m.palabra()), m);
            assert_eq!(m.siguiente().anterior(), m);
            assert_eq!(m.siguiente().siguiente().siguiente(), m, "rueda de tres");
            assert_ne!(m.siguiente(), m);
        }
        assert_eq!(Modo::de("presencial\n"), Modo::Presencial);
        // Caso negativo: lo que no se entiende es el modo que menos ocupa.
        assert_eq!(Modo::de(""), Modo::Capturas);
        assert_eq!(Modo::de("camara"), Modo::Capturas);
    }

    #[test]
    fn en_persona_solo_suena_el_microfono() {
        assert_eq!(Modo::Presencial.sonidos(), (true, false));
        assert_eq!(Modo::Capturas.sonidos(), (true, true));
        assert_eq!(Modo::Pantalla.sonidos(), (true, true));
        assert!(Modo::Presencial.con_camara() && !Modo::Presencial.con_video());
        assert!(Modo::Pantalla.con_video() && !Modo::Pantalla.con_camara());
        // Caso negativo: el de capturas no lleva ni video ni camara.
        assert!(!Modo::Capturas.con_video() && !Modo::Capturas.con_camara());
    }

    fn cam(id: &str, lado: Lado) -> (String, Lado) {
        (id.to_string(), lado)
    }

    #[test]
    fn se_empieza_con_la_recordada_o_con_la_que_mira_a_la_sala() {
        let c = vec![
            cam("delante", Lado::Delante),
            cam("usb", Lado::Fuera),
            cam("atras", Lado::Detras),
        ];
        assert_eq!(camara_elegida(&c, None), Some(2), "la de atras");
        assert_eq!(camara_elegida(&c, Some("delante")), Some(0), "la recordada");
        assert_eq!(camara_elegida(&c[..2], None), Some(1), "sin atras, la USB");
        assert_eq!(camara_elegida(&c[..1], None), Some(0));
        // Entre iguales, la primera del sistema.
        let dos = vec![cam("a", Lado::Fuera), cam("b", Lado::Fuera)];
        assert_eq!(camara_elegida(&dos, None), Some(0));
        // Caso negativo: la recordada ya no esta (se desenchufo) y no hay
        // camaras.
        assert_eq!(camara_elegida(&c, Some("vieja")), Some(2));
        assert_eq!(camara_elegida(&c, Some("  ")), Some(2));
        assert_eq!(camara_elegida(&[], Some("delante")), None);
    }

    fn modo(ancho: u32, alto: u32, formato: &str) -> ModoDeCamara {
        ModoDeCamara {
            ancho,
            alto,
            formato: formato.into(),
            fps: 0,
        }
    }

    fn video(ancho: u32, alto: u32, formato: &str, fps: u32) -> ModoDeCamara {
        ModoDeCamara {
            fps,
            ..modo(ancho, alto, formato)
        }
    }

    #[test]
    fn el_clip_va_a_1080p_y_30_fps_como_mucho() {
        let modos = vec![
            video(3840, 2160, "NV12", 30),
            video(1920, 1080, "MJPG", 30),
            video(1920, 1080, "NV12", 30),
            video(1920, 1080, "NV12", 60),
            video(1280, 720, "NV12", 30),
        ];
        assert_eq!(modo_para_clip(&modos), Some(2), "1080p30 sin comprimir");
        let modos = vec![video(1280, 720, "NV12", 15), video(1280, 720, "YUY2", 30)];
        assert_eq!(modo_para_clip(&modos), Some(1), "mas fotogramas");
        // De pie (una tableta) tambien vale.
        assert_eq!(modo_para_clip(&[video(1080, 1920, "NV12", 30)]), Some(0));
        // Caso negativo: solo 4K o solo 60 fps, ninguno (el de por defecto).
        assert_eq!(modo_para_clip(&[video(3840, 2160, "NV12", 30)]), None);
        assert_eq!(modo_para_clip(&[video(1280, 720, "NV12", 60)]), None);
    }

    #[test]
    fn el_clip_pesa_poco_y_menos_con_hevc() {
        assert_eq!(bits_del_clip(1920, 1080, false), 5_000_000);
        assert_eq!(bits_del_clip(1920, 1080, true), 3_500_000);
        let hd = bits_del_clip(1280, 720, false);
        assert!((2_000_000..=2_600_000).contains(&hd), "{hd}");
        // Caso negativo: ni una camara enana baja de 1 Mbps ni una enorme
        // pasa de 6.
        assert_eq!(bits_del_clip(160, 120, false), 1_000_000);
        assert_eq!(bits_del_clip(0, 0, true), 1_000_000);
        assert_eq!(bits_del_clip(3840, 2160, false), 6_000_000);
    }

    #[test]
    fn los_clips_van_y_vuelven_en_orden() {
        let mut resto = Map::new();
        let clips = vec![
            Clip {
                ms: 90_000,
                duracion_ms: 12_000,
                id: "b".into(),
            },
            Clip {
                ms: 5_000,
                duracion_ms: 3_000,
                id: "a".into(),
            },
        ];
        poner_clips(&mut resto, &clips);
        let v = clips_de(&resto);
        assert_eq!(
            v.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(v[1].duracion_ms, 12_000);
        assert_eq!(
            enlace_del_clip("voz", 5_000, 3_000),
            serde_json::json!({ "de": "voz", "ms": 5_000, "duracionMs": 3_000 })
        );
        // Caso negativo: sin clips la clave se quita, y una entrada rota se
        // salta sin llevarse las demas.
        poner_clips(&mut resto, &[]);
        assert!(!resto.contains_key(CLIPS));
        resto.insert(
            CLIPS.into(),
            serde_json::json!([{ "ms": "x" }, { "ms": 7, "id": "c" }]),
        );
        assert_eq!(
            clips_de(&resto),
            vec![Clip {
                ms: 7,
                duracion_ms: 0,
                id: "c".into()
            }]
        );
    }

    #[test]
    fn la_foto_va_a_la_mayor_resolucion_de_la_camara() {
        let modos = vec![
            modo(640, 480, "YUY2"),
            modo(1920, 1080, "NV12"),
            modo(3840, 2160, "NV12"),
            modo(1280, 720, "MJPG"),
        ];
        assert_eq!(mejor_modo(&modos), Some(2));
        // Mas pixeles gana aunque sea menos ancha (4:3 frente a 16:9).
        let modos = vec![modo(1920, 1080, "NV12"), modo(1920, 1440, "YUY2")];
        assert_eq!(mejor_modo(&modos), Some(1));
        // Entre dos iguales, la que ya llega en JPEG; si no, la primera.
        let modos = vec![modo(1920, 1080, "NV12"), modo(1920, 1080, "mjpg")];
        assert_eq!(mejor_modo(&modos), Some(1));
        let modos = vec![modo(1920, 1080, "NV12"), modo(1920, 1080, "YUY2")];
        assert_eq!(mejor_modo(&modos), Some(0));
        // Caso negativo: sin modos, o solo modos sin tamano, ninguno.
        assert_eq!(mejor_modo(&[]), None);
        assert_eq!(mejor_modo(&[modo(0, 1080, "NV12")]), None);
    }

    #[test]
    fn los_fallos_de_la_camara_se_explican() {
        assert_eq!(fallo_de_codigo(0x8007_0005), FalloDeCamara::SinPermiso);
        assert_eq!(fallo_de_codigo(0xC00D_ABE0), FalloDeCamara::SinCamara);
        assert_eq!(fallo_de_codigo(0xC00D_3704), FalloDeCamara::EnUso);
        assert_eq!(fallo_de_codigo(0xC00D_3EA3), FalloDeCamara::EnUso);
        assert_eq!(fallo_de_codigo(0xC00D_3EA2), FalloDeCamara::SinCamara);
        // Caso negativo: lo desconocido no se disfraza de otra cosa.
        assert_eq!(fallo_de_codigo(0x8000_4005), FalloDeCamara::Otro);
        assert_eq!(fallo_de_codigo(0), FalloDeCamara::Otro);
    }

    #[test]
    fn el_minuto_de_la_foto_es_el_de_cuando_se_pulso() {
        assert_eq!(ms_de_la_foto(65_400, None), 65_400);
        assert_eq!(ms_de_la_foto(65_400, Some(90_000)), 65_400);
        // Caso negativo: no antes del principio ni despues del final.
        assert_eq!(ms_de_la_foto(-30, None), 0);
        assert_eq!(ms_de_la_foto(95_000, Some(90_000)), 90_000);
        assert_eq!(ms_de_la_foto(10, Some(-1)), 0);
    }

    #[test]
    fn cada_foto_tiene_su_nombre() {
        assert_eq!(
            nombre_de_la_foto(true, 1_761_500_000_000, 3, "jpg"),
            "foto-1761500000000-3.jpg"
        );
        assert_eq!(
            nombre_de_la_foto(false, 1_761_500_000_000, 1, ".PNG"),
            "clase-1761500000000-1.png"
        );
        assert_ne!(
            nombre_de_la_foto(true, 7, 1, "jpg"),
            nombre_de_la_foto(true, 7, 2, "jpg"),
            "dos en el mismo milisegundo no se pisan"
        );
        // Caso negativo: sin extension, PNG (lo que se guardaba siempre).
        assert_eq!(nombre_de_la_foto(false, 7, 1, " "), "clase-7-1.png");
    }

    #[test]
    fn la_foto_de_la_camara_lo_dice_en_su_resto() {
        let mut resto = Map::new();
        assert!(!es_de_camara(&resto));
        marcar_de_camara(&mut resto);
        assert!(es_de_camara(&resto));
        assert_eq!(resto.get("fuente"), Some(&Value::String("camara".into())));
        assert!(!es_del_movil(&resto));
        marcar_del_movil(&mut resto);
        assert!(es_del_movil(&resto) && !es_de_camara(&resto));
        // Caso negativo: otra fuente no es la camara.
        resto.insert(FUENTE.into(), Value::String("pantalla".into()));
        assert!(!es_de_camara(&resto));
        // Y el campo `origen` del mensaje no se toca.
        assert!(!resto.contains_key("origen"));
    }

    #[test]
    fn la_imagen_entera_solo_se_carga_al_acercar() {
        // Una foto de 4000 cargada a 2560: encajada en 1200 no hace falta.
        assert!(!hace_falta_la_entera(1200.0, 2560, 4000));
        // Acercada a 3000 de ancho, si.
        assert!(hace_falta_la_entera(3000.0, 2560, 4000));
        // Caso negativo: ya esta entera (o casi igual de ancha al pintarla).
        assert!(!hace_falta_la_entera(9000.0, 4000, 4000));
        assert!(!hace_falta_la_entera(2600.0, 2560, 4000), "el margen");
    }
}
