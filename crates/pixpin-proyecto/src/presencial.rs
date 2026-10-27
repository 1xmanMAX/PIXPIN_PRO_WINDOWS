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
//!   `resto` del mensaje, `origen: "camara"`. Es el unico campo nuevo; lo
//!   demas (`enGrabacion: {de, ms}`) es lo de las capturas.
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

/// La clave del origen de una imagen en `resto`.
pub const ORIGEN: &str = "origen";
/// Su valor para las fotos de la camara.
pub const DE_CAMARA: &str = "camara";

/// Apunta en el `resto` de una imagen que salio de la camara.
pub fn marcar_de_camara(resto: &mut Map<String, Value>) {
    resto.insert(ORIGEN.into(), Value::String(DE_CAMARA.into()));
}

/// Si una imagen salio de la camara (y no de la pantalla).
pub fn es_de_camara(resto: &Map<String, Value>) -> bool {
    resto.get(ORIGEN).and_then(Value::as_str) == Some(DE_CAMARA)
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
        }
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
        assert_eq!(resto.get("origen"), Some(&Value::String("camara".into())));
        // Caso negativo: otro origen no es la camara.
        resto.insert(ORIGEN.into(), Value::String("pantalla".into()));
        assert!(!es_de_camara(&resto));
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
