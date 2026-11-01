//! **Los perfiles**: «Ligero», «Normal» y «Nitido». Cada uno dice hasta que
//! tamano se graba, cuantos fotogramas por segundo como mucho y cuantos bits
//! se le dejan al codificador.
//!
//! Los numeros salen del presupuesto del equipo de referencia (un Core i3 de
//! tercera generacion con 4 GB y un disco pequeno): una hora de reunion en
//! «Normal» tiene que caber en unos 300 MB **en el peor caso** (la pantalla
//! cambiando sin parar, como una reunion con camaras). Con la pantalla
//! quieta —una diapositiva— el ritmo variable lo deja muy por debajo.

use crate::geometria;

/// Bytes por segundo del AAC (96 kbps), los mismos que la nota de voz.
pub use crate::audio::BYTES_POR_SEGUNDO_AAC;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Perfil {
    /// 720p a 10 fps: lo minimo para leer una diapositiva, y lo que menos
    /// pesa.
    Ligero,
    /// 1080p a 15 fps: el de salida.
    #[default]
    Normal,
    /// La pantalla tal cual (hasta 4K) a 30 fps: para cuando el texto
    /// pequeno o el movimiento importan mas que el disco.
    Nitido,
}

/// Lo que un perfil le pide al motor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ajustes {
    /// El tamano mas grande al que se graba; `None`, el de la pantalla
    /// (con el tope duro de [`geometria::TOPE_NATIVO`]).
    pub tope: Option<(u32, u32)>,
    /// Fotogramas por segundo como mucho. Es un tope: con la pantalla quieta
    /// se codifica uno por segundo.
    pub por_segundo: u32,
    /// Bits por segundo de media y de pico del video para un fotograma del
    /// tamano de [`Ajustes::referencia`]; para otros tamanos se escala (ver
    /// [`bits_por_segundo`]).
    pub media: u32,
    pub pico: u32,
    /// El tamano al que valen `media` y `pico`.
    pub referencia: (u32, u32),
    /// Calidad contra velocidad del codificador (0 lo mas rapido, 100 lo
    /// mejor). El de la grafica casi no lo mira; el de software si.
    pub calidad_vs_velocidad: u32,
}

impl Perfil {
    /// En el orden en que se ofrecen.
    pub const TODOS: [Perfil; 3] = [Perfil::Ligero, Perfil::Normal, Perfil::Nitido];

    pub fn ajustes(self) -> Ajustes {
        match self {
            Perfil::Ligero => Ajustes {
                tope: Some((1280, 720)),
                por_segundo: 10,
                media: 300_000,
                pico: 1_500_000,
                referencia: (1280, 720),
                calidad_vs_velocidad: 20,
            },
            Perfil::Normal => Ajustes {
                tope: Some((1920, 1080)),
                por_segundo: 15,
                media: 550_000,
                pico: 3_000_000,
                referencia: (1920, 1080),
                calidad_vs_velocidad: 33,
            },
            Perfil::Nitido => Ajustes {
                tope: None,
                por_segundo: 30,
                media: 1_200_000,
                pico: 6_000_000,
                referencia: (1920, 1080),
                calidad_vs_velocidad: 50,
            },
        }
    }

    /// El siguiente al pulsar (vuelve al primero tras el ultimo).
    pub fn siguiente(self) -> Perfil {
        match self {
            Perfil::Ligero => Perfil::Normal,
            Perfil::Normal => Perfil::Nitido,
            Perfil::Nitido => Perfil::Ligero,
        }
    }

    /// Un nombre fijo, para guardar la eleccion o elegir el rotulo.
    pub fn clave(self) -> &'static str {
        match self {
            Perfil::Ligero => "ligero",
            Perfil::Normal => "normal",
            Perfil::Nitido => "nitido",
        }
    }

    /// El perfil de una [`Perfil::clave`]; cualquier otra cosa, «Normal».
    pub fn de_clave(clave: &str) -> Perfil {
        Perfil::TODOS
            .into_iter()
            .find(|p| p.clave() == clave)
            .unwrap_or_default()
    }

    /// Las medidas del video para una fuente de `ancho`x`alto`.
    pub fn medidas(self, ancho: u32, alto: u32) -> (u32, u32) {
        geometria::medida_de_salida(ancho, alto, self.ajustes().tope)
    }
}

/// **Bits por segundo de media y de pico** para un video de `ancho`x`alto`
/// con estos ajustes: proporcionales a los pixeles respecto a la
/// referencia, pero nunca menos de la mitad (el texto de una ventana
/// pequena necesita sus bits igual) ni mas de cuatro veces (una 4K no tiene
/// por que pesar como cuatro 1080p: la pantalla es casi toda lisa).
pub fn bits_por_segundo(a: &Ajustes, ancho: u32, alto: u32) -> (u32, u32) {
    let px = u64::from(ancho.max(2)) * u64::from(alto.max(2));
    let referencia = u64::from(a.referencia.0.max(2)) * u64::from(a.referencia.1.max(2));
    // En milesimas para no perder la fraccion.
    let factor = (px * 1000 / referencia).clamp(500, 4000);
    let escalar = |v: u32| (u64::from(v) * factor / 1000).min(u64::from(u32::MAX)) as u32;
    (escalar(a.media), escalar(a.pico))
}

/// Fotogramas entre dos fotogramas clave: dos segundos al ritmo maximo. Con
/// ritmo variable el tramo en tiempo real se alarga cuando la pantalla esta
/// quieta, que es justo cuando no importa.
pub fn grupo_de_imagenes(a: &Ajustes) -> u32 {
    a.por_segundo.max(1) * 2
}

/// **Megas por hora en el peor caso**: el video a su media todo el rato y
/// el sonido AAC. Es la cuenta del presupuesto de la cabecera.
pub fn megas_por_hora(media_video: u32, con_sonido: bool) -> u64 {
    let video = u64::from(media_video) / 8 * 3600;
    let sonido = if con_sonido {
        u64::from(BYTES_POR_SEGUNDO_AAC) * 3600
    } else {
        0
    };
    (video + sonido) / 1_000_000
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_perfil_tiene_su_tamano_y_su_ritmo() {
        assert_eq!(Perfil::Ligero.medidas(1920, 1080), (1280, 720));
        assert_eq!(Perfil::Normal.medidas(1920, 1080), (1920, 1080));
        assert_eq!(Perfil::Normal.medidas(2560, 1440), (1920, 1080));
        // Nitido es la pantalla tal cual.
        assert_eq!(Perfil::Nitido.medidas(2560, 1440), (2560, 1440));
        // Caso negativo: ningun perfil agranda una pantalla pequena.
        assert_eq!(Perfil::Normal.medidas(1366, 768), (1366, 768));
        assert_eq!(Perfil::Ligero.ajustes().por_segundo, 10);
        assert_eq!(Perfil::Normal.ajustes().por_segundo, 15);
        assert_eq!(Perfil::Nitido.ajustes().por_segundo, 30);
    }

    #[test]
    fn una_hora_de_normal_cabe_en_el_presupuesto() {
        // El objetivo: una hora de Meet a 1080p en 150-300 MB.
        let a = Perfil::Normal.ajustes();
        let (media, _) = bits_por_segundo(&a, 1920, 1080);
        let mb = megas_por_hora(media, true);
        assert!((150..=300).contains(&mb), "{mb} MB por hora");
        // Ligero pesa menos que Normal, y Normal menos que Nitido.
        let l = megas_por_hora(Perfil::Ligero.ajustes().media, true);
        let n = megas_por_hora(Perfil::Nitido.ajustes().media, true);
        assert!(l < mb && mb < n, "{l} {mb} {n}");
    }

    #[test]
    fn los_bits_escalan_con_los_pixeles_con_topes() {
        let a = Perfil::Normal.ajustes();
        assert_eq!(bits_por_segundo(&a, 1920, 1080), (a.media, a.pico));
        // Una ventanita no se queda sin bits: como mucho la mitad.
        assert_eq!(bits_por_segundo(&a, 320, 240), (a.media / 2, a.pico / 2));
        // Caso negativo: una pantalla enorme no se dispara.
        let n = Perfil::Nitido.ajustes();
        let (m, _) = bits_por_segundo(&n, 7680, 4320);
        assert_eq!(m, n.media * 4);
        // El pico siempre por encima de la media.
        for p in Perfil::TODOS {
            let (m, x) = bits_por_segundo(&p.ajustes(), 1280, 720);
            assert!(x > m);
        }
    }

    #[test]
    fn las_claves_van_y_vuelven_y_lo_raro_es_normal() {
        for p in Perfil::TODOS {
            assert_eq!(Perfil::de_clave(p.clave()), p);
        }
        assert_eq!(Perfil::de_clave("ultra"), Perfil::Normal);
        assert_eq!(Perfil::de_clave(""), Perfil::Normal);
    }

    #[test]
    fn pulsar_recorre_los_tres_y_vuelve() {
        let mut p = Perfil::Ligero;
        let mut vistos = Vec::new();
        for _ in 0..3 {
            vistos.push(p);
            p = p.siguiente();
        }
        assert_eq!(vistos, Perfil::TODOS);
        assert_eq!(p, Perfil::Ligero);
    }

    #[test]
    fn el_grupo_de_imagenes_son_dos_segundos() {
        assert_eq!(grupo_de_imagenes(&Perfil::Normal.ajustes()), 30);
        assert_eq!(grupo_de_imagenes(&Perfil::Ligero.ajustes()), 20);
    }
}
