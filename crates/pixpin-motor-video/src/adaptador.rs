//! **En que grafica se graba** cuando el equipo tiene mas de una.
//!
//! La captura, la conversion y el codificador tienen que vivir en la misma
//! grafica para que la imagen no salga de la memoria de video. Lo que se
//! aprendio de otros grabadores (ver
//! `docs/investigacion/2026-10-25-motores-de-grabacion.md`):
//!
//! - **La grafica que tiene conectado el monitor que se graba**. En un
//!   portatil con Intel + NVIDIA la pantalla la pinta la Intel; si el
//!   dispositivo va en la NVIDIA, Windows copia cada fotograma de una a otra
//!   y, cuando la NVIDIA se apaga para ahorrar, el dispositivo se pierde
//!   (lo mismo que mide `pixpin-capture`). OBS elige igual: la captura de
//!   pantalla va en el adaptador de la salida (`duplicator-monitor-capture.c`,
//!   `choose_method`).
//! - **Nunca una grafica virtual ni la de software** si hay otra: los
//!   controladores de Parsec, DisplayLink, Splashtop... no aguantan bien el
//!   trabajo de video de Media Foundation y se han visto cierres sin aviso
//!   (Cap, `crates/d3d-adapter`, `VIRTUAL_ADAPTER_MARKERS`).
//! - Si no hay ninguna buena, `None`: que decida Windows (y, como ultimo
//!   recurso, WARP), como antes.
//!
//! Es puro: el backend de cada sistema rellena los [`Candidato`] y aplica la
//! eleccion.

/// Los fabricantes que importan para las peculiaridades del codificador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fabricante {
    Intel,
    Amd,
    Nvidia,
    /// WARP y el adaptador basico de pantalla.
    Microsoft,
    Otro(u32),
}

impl Fabricante {
    /// Por el identificador PCI del fabricante (`VendorId` de DXGI).
    pub fn de_id(id: u32) -> Fabricante {
        match id {
            0x8086 => Fabricante::Intel,
            0x1002 | 0x1022 => Fabricante::Amd,
            0x10DE => Fabricante::Nvidia,
            0x1414 => Fabricante::Microsoft,
            otro => Fabricante::Otro(otro),
        }
    }

    /// Lo que pone Media Foundation en `MFT_ENUM_HARDWARE_VENDOR_ID_Attribute`
    /// de un codificador de hardware: `VEN_8086` y similares.
    pub fn de_cadena_mft(cadena: &str) -> Option<Fabricante> {
        let hex = cadena.trim().strip_prefix("VEN_")?;
        u32::from_str_radix(hex, 16).ok().map(Fabricante::de_id)
    }
}

/// Una grafica tal como la ve el sistema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidato {
    pub descripcion: String,
    pub fabricante: u32,
    pub dispositivo: u32,
    /// `DXGI_ADAPTER_FLAG_SOFTWARE` (WARP y similares).
    pub software: bool,
    /// Si tiene conectado el monitor que se graba.
    pub tiene_el_monitor: bool,
}

/// Trozos del nombre de los controladores de pantalla virtuales (los de
/// Cap, mas los que se ven en equipos de oficina).
const VIRTUALES: &[&str] = &[
    "Parsec",
    "DisplayLink",
    "Splashtop",
    "Synergy",
    "Virtual Display",
    "IddSampleDriver",
    "Microsoft Basic",
    "Microsoft Remote Display",
    "WARP",
];

/// Si la grafica no es una de verdad: de software o un controlador virtual.
pub fn es_virtual(c: &Candidato) -> bool {
    c.software
        || Fabricante::de_id(c.fabricante) == Fabricante::Microsoft
        || VIRTUALES.iter().any(|v| c.descripcion.contains(v))
}

/// **La grafica donde grabar**, por su indice en `candidatos`: la de verdad
/// que tiene el monitor; si el monitor esta en una virtual, la primera de
/// verdad (Windows lleva la imagen de una a otra); `None` si no hay
/// ninguna de verdad.
pub fn elegir(candidatos: &[Candidato]) -> Option<usize> {
    let de_verdad = |c: &&Candidato| !es_virtual(c);
    candidatos
        .iter()
        .position(|c| c.tiene_el_monitor && de_verdad(&c))
        .or_else(|| candidatos.iter().position(|c| de_verdad(&c)))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn c(descripcion: &str, fabricante: u32, monitor: bool) -> Candidato {
        Candidato {
            descripcion: descripcion.into(),
            fabricante,
            dispositivo: 0,
            software: false,
            tiene_el_monitor: monitor,
        }
    }

    #[test]
    fn en_un_portatil_con_dos_graficas_va_la_del_monitor() {
        // Windows enumera primero la NVIDIA (la de alto rendimiento), pero
        // la pantalla la pinta la Intel.
        let v = [
            c("NVIDIA GeForce MX150", 0x10DE, false),
            c("Intel(R) HD Graphics 4000", 0x8086, true),
        ];
        assert_eq!(elegir(&v), Some(1));
        // Un monitor externo en la NVIDIA: la NVIDIA.
        let v = [
            c("NVIDIA GeForce MX150", 0x10DE, true),
            c("Intel(R) HD Graphics 4000", 0x8086, true),
        ];
        assert_eq!(elegir(&v), Some(0));
    }

    #[test]
    fn las_graficas_virtuales_no_se_eligen_aunque_tengan_el_monitor() {
        let v = [
            c("Parsec Virtual Display Adapter", 0x1AB8, true),
            c("Intel(R) HD Graphics 4000", 0x8086, false),
        ];
        assert_eq!(elegir(&v), Some(1));
        // Caso negativo: solo virtuales y de software, ninguna: que decida
        // Windows.
        let mut warp = c("Microsoft Basic Render Driver", 0x1414, true);
        warp.software = true;
        assert_eq!(elegir(&[warp, c("DisplayLink USB", 0x17E9, true)]), None);
        assert_eq!(elegir(&[]), None);
    }

    #[test]
    fn el_fabricante_sale_del_id_y_de_la_cadena_del_mft() {
        assert_eq!(Fabricante::de_id(0x8086), Fabricante::Intel);
        assert_eq!(Fabricante::de_id(0x1002), Fabricante::Amd);
        assert_eq!(Fabricante::de_id(0x10DE), Fabricante::Nvidia);
        assert_eq!(
            Fabricante::de_cadena_mft("VEN_10DE"),
            Some(Fabricante::Nvidia)
        );
        assert_eq!(
            Fabricante::de_cadena_mft(" VEN_8086 "),
            Some(Fabricante::Intel)
        );
        // Caso negativo: lo que no tiene la forma no es nadie.
        assert_eq!(Fabricante::de_cadena_mft("8086"), None);
        assert_eq!(Fabricante::de_cadena_mft("VEN_zz"), None);
    }
}
