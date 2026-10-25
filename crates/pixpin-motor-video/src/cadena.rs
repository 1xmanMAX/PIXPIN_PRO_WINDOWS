//! **La cadena de caidas**: que codificador se intenta primero y cual
//! despues. Se elige una vez, al empezar, y se dice cual quedo
//! ([`crate::Estadisticas::codificador`]): a mitad de una reunion no se
//! cambia de codificador.
//!
//! 1. **La grafica** ([`Codificador::Hardware`]): el H.264 del hardware
//!    (Quick Sync en el equipo de referencia) leyendo las texturas NV12 que
//!    deja el procesador de video. La CPU casi no se entera.
//! 2. **Software** ([`Codificador::Software`]): el H.264 de Microsoft. La
//!    conversion y el encogido siguen en la GPU; solo baja el NV12 ya
//!    pequeno (1,5 bytes por pixel en vez de 4).
//! 3. **La CPU** ([`Codificador::Cpu`]): lo de antes de este motor
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

/// Lo que se sabe del equipo antes de intentar nada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capacidades {
    /// La GPU tiene procesador de video (BGRA -> NV12) en el dispositivo.
    pub conversor_gpu: bool,
    /// Hay un codificador H.264 de hardware registrado.
    pub h264_hardware: bool,
    /// Y uno HEVC.
    pub hevc_hardware: bool,
}

/// **Los intentos, en orden**, para estas capacidades.
pub fn intentos(c: Capacidades, pedir_hevc: bool) -> Vec<(Codificador, Formato)> {
    let mut v = Vec::with_capacity(4);
    if c.conversor_gpu {
        if pedir_hevc && c.hevc_hardware {
            v.push((Codificador::Hardware, Formato::Hevc));
        }
        if c.h264_hardware {
            v.push((Codificador::Hardware, Formato::H264));
        }
        v.push((Codificador::Software, Formato::H264));
    }
    v.push((Codificador::Cpu, Formato::H264));
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const TODO: Capacidades = Capacidades {
        conversor_gpu: true,
        h264_hardware: true,
        hevc_hardware: true,
    };

    #[test]
    fn con_todo_se_empieza_por_la_grafica_en_h264() {
        assert_eq!(
            intentos(TODO, false),
            vec![
                (Codificador::Hardware, Formato::H264),
                (Codificador::Software, Formato::H264),
                (Codificador::Cpu, Formato::H264),
            ]
        );
    }

    #[test]
    fn hevc_solo_si_se_pide_y_hay_grafica_que_lo_haga() {
        assert_eq!(
            intentos(TODO, true)[0],
            (Codificador::Hardware, Formato::Hevc)
        );
        // Y si falla, H.264 de la grafica detras.
        assert_eq!(
            intentos(TODO, true)[1],
            (Codificador::Hardware, Formato::H264)
        );
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
        assert_eq!(intentos(c, true)[0], (Codificador::Software, Formato::H264));
    }

    #[test]
    fn sin_procesador_de_video_solo_queda_la_cpu() {
        // Caso negativo: aunque haya codificador de hardware, sin el
        // conversor no hay NV12 en la GPU que darle.
        let c = Capacidades {
            conversor_gpu: false,
            ..TODO
        };
        assert_eq!(intentos(c, true), vec![(Codificador::Cpu, Formato::H264)]);
        // La CPU siempre es el ultimo: la lista nunca esta vacia.
        assert_eq!(
            intentos(Capacidades::default(), false).last(),
            Some(&(Codificador::Cpu, Formato::H264))
        );
    }
}
