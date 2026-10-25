//! **La cadena de caidas**: que codificador se intenta primero y cual
//! despues. Se elige una vez, al empezar, y se dice cual quedo
//! ([`crate::Estadisticas::codificador`]): a mitad de una reunion no se
//! cambia de codificador (si la grafica se pierde, se vuelve a montar el
//! mismo camino: `crate::recuperacion`).
//!
//! 1. **La grafica** ([`Codificador::Hardware`]): el H.264 del hardware
//!    (Quick Sync en el equipo de referencia) leyendo las texturas NV12 que
//!    deja el procesador de video. La CPU casi no se entera
//!    ([`Camino::Grafica`]). Si no arranca a mas de 1080p se reintenta mas
//!    pequeno ([`reintento_mas_pequeno`]) antes de bajar de camino, y en
//!    las Intel de hace diez anos nunca se le pide mas de lo que hacen
//!    ([`peculiaridades`]).
//! 2. **La grafica, desde memoria** ([`Camino::GraficaEnMemoria`], que
//!    tambien se ensena como `Hardware`): cuando el codificador de hardware
//!    esta en otra grafica que la de la pantalla, o no acepta las texturas,
//!    el NV12 baja a memoria (como en el camino de software) y se le da
//!    asi. Sigue sin gastar CPU en codificar.
//! 3. **Software** ([`Codificador::Software`]): el H.264 de Microsoft. La
//!    conversion y el encogido siguen en la GPU; solo baja el NV12 ya
//!    pequeno (1,5 bytes por pixel en vez de 4).
//! 4. **La CPU** ([`Codificador::Cpu`]): lo de antes de este motor
//!    (`pixpin-record`): se baja la pantalla en BGRA, se encoge en la CPU y
//!    se codifica por software, a lo de «Ligero». Para cuando la grafica no
//!    tiene procesador de video (un controlador roto, una maquina virtual).
//!
//! HEVC solo se intenta si se pide **y** la grafica lo tiene; si no, H.264,
//! que es lo que reproduce cualquier movil Android.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Codificador {
    Hardware,
    Software,
    Cpu,
}

impl Codificador {
    /// Un nombre fijo, para el registro y para elegir el rotulo.
    pub fn clave(self) -> &'static str {
        match self {
            Codificador::Hardware => "grafica",
            Codificador::Software => "software",
            Codificador::Cpu => "cpu",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Formato {
    H264,
    Hevc,
}

/// **Los caminos** de la cadena, de mejor a peor. Hay dos con el
/// codificador de la grafica: el bueno (las texturas NV12 van tal cual) y
/// el de cuando el codificador esta en otra grafica que la de la pantalla
/// (el NV12 baja a memoria y la grafica lo vuelve a leer, como el
/// «non-texture encoder» al que OBS cae con NVENC en otra GPU,
/// `obs-nvenc/nvenc.c`, `nvenc_create_base`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Camino {
    /// Texturas de la GPU al codificador de la misma grafica.
    Grafica,
    /// NV12 en memoria al codificador de hardware que haya.
    GraficaEnMemoria,
    /// NV12 en memoria al H.264 de Microsoft.
    Software,
    /// Lo de antes, todo por la CPU.
    Cpu,
}

impl Camino {
    /// Lo que se ensena: los dos de la grafica son «la grafica».
    pub fn codificador(self) -> Codificador {
        match self {
            Camino::Grafica | Camino::GraficaEnMemoria => Codificador::Hardware,
            Camino::Software => Codificador::Software,
            Camino::Cpu => Codificador::Cpu,
        }
    }

    /// Un nombre fijo para el registro.
    pub fn clave(self) -> &'static str {
        match self {
            Camino::Grafica => "grafica",
            Camino::GraficaEnMemoria => "grafica-en-memoria",
            Camino::Software => "software",
            Camino::Cpu => "cpu",
        }
    }
}

/// Lo que se sabe del equipo antes de intentar nada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capacidades {
    /// La GPU tiene procesador de video (BGRA -> NV12) en el dispositivo.
    pub conversor_gpu: bool,
    /// Hay un codificador H.264 de hardware registrado (en cualquier
    /// grafica).
    pub h264_hardware: bool,
    /// Y uno HEVC.
    pub hevc_hardware: bool,
    /// Alguno de los de hardware es del fabricante de la grafica del
    /// dispositivo: solo entonces vale la pena darle las texturas. Si no se
    /// sabe, `true` (lo comprueba el escritor al abrirse).
    pub hardware_en_la_grafica: bool,
}

/// **Los intentos, en orden**, para estas capacidades.
pub fn intentos(c: Capacidades, pedir_hevc: bool) -> Vec<(Camino, Formato)> {
    let mut v = Vec::with_capacity(5);
    if c.conversor_gpu {
        if c.hardware_en_la_grafica {
            if pedir_hevc && c.hevc_hardware {
                v.push((Camino::Grafica, Formato::Hevc));
            }
            if c.h264_hardware {
                v.push((Camino::Grafica, Formato::H264));
            }
        }
        if c.h264_hardware {
            v.push((Camino::GraficaEnMemoria, Formato::H264));
        }
        v.push((Camino::Software, Formato::H264));
    }
    v.push((Camino::Cpu, Formato::H264));
    v
}

// --- Las peculiaridades de cada grafica ----------------------------------

/// Las Intel de segunda y tercera generacion (Sandy Bridge, Ivy Bridge: HD
/// 2000/2500/3000/4000), por su identificador PCI. La del equipo de
/// referencia es una de estas.
const INTEL_GEN6_GEN7: &[u32] = &[
    0x0102, 0x0106, 0x010A, 0x0112, 0x0116, 0x0122, 0x0126, // Sandy Bridge
    0x0152, 0x0156, 0x015A, 0x0162, 0x0166, 0x016A, // Ivy Bridge
];

/// Lo que se sabe de un codificador de hardware antes de abrirlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Peculiaridades {
    /// El tamano mas grande que codifica de verdad.
    pub tope: Option<(u32, u32)>,
    /// Sin fotogramas B (los pide cero siempre este motor, pero se apunta
    /// por que).
    pub sin_b: bool,
}

/// **Las peculiaridades de la grafica** de `fabricante` y `dispositivo`
/// (identificadores PCI). De OBS (`obs-qsv11/obs-qsv11.c`): en Sandy Bridge
/// e Ivy Bridge, Quick Sync no pasa de 1920x1200 (`cap_resolution`) ni hace
/// bien los fotogramas B (`bFrames = 0`).
pub fn peculiaridades(fabricante: u32, dispositivo: u32) -> Peculiaridades {
    if fabricante == 0x8086 && INTEL_GEN6_GEN7.contains(&dispositivo) {
        return Peculiaridades {
            tope: Some((1920, 1200)),
            sin_b: true,
        };
    }
    Peculiaridades::default()
}

/// El tamano al que se reintenta la grafica si no arranco a uno mas grande
/// (como `screenrecord` de Android, que reintenta a 720p:
/// `frameworks/av/cmds/screenrecord/screenrecord.cpp`, `kFallbackWidth`):
/// primero 1080p y luego 720p, con la misma forma. `None` si ya es pequeno.
pub fn reintento_mas_pequeno(medidas: (u32, u32)) -> Option<(u32, u32)> {
    let (w, h) = medidas;
    for tope in [(1920, 1080), (1280, 720)] {
        // Con la forma de la pantalla (vertical u horizontal).
        let tope = if h > w { (tope.1, tope.0) } else { tope };
        if w > tope.0 || h > tope.1 {
            return Some(crate::geometria::medida_de_salida(w, h, Some(tope)));
        }
    }
    None
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const TODO: Capacidades = Capacidades {
        conversor_gpu: true,
        h264_hardware: true,
        hevc_hardware: true,
        hardware_en_la_grafica: true,
    };

    #[test]
    fn con_todo_se_empieza_por_la_grafica_en_h264() {
        assert_eq!(
            intentos(TODO, false),
            vec![
                (Camino::Grafica, Formato::H264),
                (Camino::GraficaEnMemoria, Formato::H264),
                (Camino::Software, Formato::H264),
                (Camino::Cpu, Formato::H264),
            ]
        );
    }

    #[test]
    fn hevc_solo_si_se_pide_y_hay_grafica_que_lo_haga() {
        assert_eq!(intentos(TODO, true)[0], (Camino::Grafica, Formato::Hevc));
        // Y si falla, H.264 de la grafica detras.
        assert_eq!(intentos(TODO, true)[1], (Camino::Grafica, Formato::H264));
        // Caso negativo: pedido pero sin grafica que lo haga, ni se intenta.
        let sin_hevc = Capacidades {
            hevc_hardware: false,
            ..TODO
        };
        assert!(
            intentos(sin_hevc, true)
                .iter()
                .all(|(_, f)| *f == Formato::H264)
        );
    }

    #[test]
    fn sin_codificador_de_hardware_se_va_al_software_por_la_gpu() {
        let c = Capacidades {
            conversor_gpu: true,
            ..Default::default()
        };
        assert_eq!(intentos(c, true)[0], (Camino::Software, Formato::H264));
    }

    #[test]
    fn sin_procesador_de_video_solo_queda_la_cpu() {
        // Caso negativo: aunque haya codificador de hardware, sin el
        // conversor no hay NV12 en la GPU que darle.
        let c = Capacidades {
            conversor_gpu: false,
            ..TODO
        };
        assert_eq!(intentos(c, true), vec![(Camino::Cpu, Formato::H264)]);
        // La CPU siempre es el ultimo: la lista nunca esta vacia.
        assert_eq!(
            intentos(Capacidades::default(), false).last(),
            Some(&(Camino::Cpu, Formato::H264))
        );
    }

    #[test]
    fn con_el_codificador_en_otra_grafica_se_le_da_el_nv12_en_memoria() {
        // Un portatil con la pantalla en una grafica sin codificador y el
        // codificador en la otra: nada de texturas, pero la grafica
        // codifica igual.
        let c = Capacidades {
            hardware_en_la_grafica: false,
            ..TODO
        };
        assert_eq!(
            intentos(c, true),
            vec![
                (Camino::GraficaEnMemoria, Formato::H264),
                (Camino::Software, Formato::H264),
                (Camino::Cpu, Formato::H264),
            ]
        );
        // Los dos de la grafica se ensenan como «la grafica».
        assert_eq!(
            Camino::GraficaEnMemoria.codificador(),
            Codificador::Hardware
        );
        assert_eq!(Camino::Grafica.codificador(), Codificador::Hardware);
        assert_eq!(Camino::Software.codificador(), Codificador::Software);
    }

    #[test]
    fn quick_sync_de_ivy_bridge_no_pasa_de_1920x1200() {
        // HD Graphics 4000, la del equipo de referencia.
        let p = peculiaridades(0x8086, 0x0166);
        assert_eq!(p.tope, Some((1920, 1200)));
        assert!(p.sin_b);
        // Caso negativo: una Intel moderna (UHD 620) o una NVIDIA, sin tope.
        assert_eq!(peculiaridades(0x8086, 0x5917), Peculiaridades::default());
        assert_eq!(peculiaridades(0x10DE, 0x0166), Peculiaridades::default());
    }

    #[test]
    fn el_reintento_baja_a_1080p_y_luego_a_720p() {
        assert_eq!(reintento_mas_pequeno((3840, 2160)), Some((1920, 1080)));
        assert_eq!(reintento_mas_pequeno((1920, 1080)), Some((1280, 720)));
        // Una pantalla vertical sigue vertical.
        assert_eq!(reintento_mas_pequeno((1080, 1920)), Some((720, 1280)));
        // Caso negativo: lo que ya es pequeno no se reintenta.
        assert_eq!(reintento_mas_pequeno((1280, 720)), None);
        assert_eq!(reintento_mas_pequeno((1366, 768)), Some((1280, 718)));
    }
}
