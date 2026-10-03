//! **Los colores del editor**, oscuros o claros segun el tema de Windows.
//!
//! Los oscuros estan medidos en las capturas del editor de documentos de
//! Claude que mando el usuario (papel `#0b0b0b`, cabecera `#20201f`, la
//! pastilla del titulo `#313130`, lo elegido en un menu `#414140`, rayas de
//! tabla `#242424`); los claros son su version de dia, con el mismo papel
//! calido. Los del Markdown (codigo dorado, enlaces azules, casillas) son
//! los del lector y del modo noche del movil, como hasta ahora.

/// Un color `0xRRGGBB`.
pub type Rgb = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tema {
    pub oscuro: bool,
    /// El papel: fondo de la nota y de la barra de herramientas.
    pub papel: Rgb,
    /// La franja de arriba, la del titulo.
    pub cabecera: Rgb,
    /// La pastilla del titulo y los botones al pasar por encima.
    pub pastilla: Rgb,
    /// Rayas finas: bajo la barra y alrededor de los menus.
    pub raya: Rgb,
    pub texto: Rgb,
    /// Texto secundario: la pista del menu, el de los botones.
    pub tenue: Rgb,
    /// Las marcas del Markdown a la vista (renglon del cursor).
    pub apagado: Rgb,
    /// El fondo de los menus.
    pub menu: Rgb,
    /// La entrada elegida de un menu.
    pub elegido: Rgb,
    /// El boton de compartir, que va al reves (claro sobre oscuro).
    pub boton_fondo: Rgb,
    pub boton_texto: Rgb,
    /// La fila de cabecera de una tabla y sus rayas.
    pub tabla_cabecera: Rgb,
    pub tabla_raya: Rgb,
    pub fondo_codigo: Rgb,
    pub codigo: Rgb,
    pub enlace: Rgb,
    pub acento: Rgb,
    pub hecha: Rgb,
    pub formula: Rgb,
}

impl Tema {
    pub fn oscuro() -> Tema {
        Tema {
            oscuro: true,
            papel: 0x0b0b0b,
            cabecera: 0x20201f,
            pastilla: 0x313130,
            raya: 0x2a2a29,
            texto: 0xe6e4df,
            tenue: 0x9a9893,
            apagado: 0x6e6d69,
            menu: 0x20201f,
            elegido: 0x414140,
            boton_fondo: 0xf5f4ef,
            boton_texto: 0x1a1a19,
            tabla_cabecera: 0x151515,
            tabla_raya: 0x2e2e2d,
            fondo_codigo: 0x1d1d1c,
            codigo: 0xe8c06a,
            enlace: 0x8ab4f8,
            acento: 0x5b9bf0,
            hecha: 0x66bb6a,
            formula: 0xc3a6ff,
        }
    }

    pub fn claro() -> Tema {
        Tema {
            oscuro: false,
            papel: 0xfdfcf9,
            cabecera: 0xf2f0ea,
            pastilla: 0xe4e2da,
            raya: 0xe2dfd6,
            texto: 0x1f1e1d,
            tenue: 0x6b6a66,
            apagado: 0xa3a19b,
            menu: 0xffffff,
            elegido: 0xecebe4,
            boton_fondo: 0x1f1e1d,
            boton_texto: 0xfaf9f5,
            tabla_cabecera: 0xf2f0ea,
            tabla_raya: 0xdcd9cf,
            fondo_codigo: 0xf0eee8,
            codigo: 0x9a5b13,
            enlace: 0x1a62c9,
            acento: 0x1e73d8,
            hecha: 0x2e8b3a,
            formula: 0x7447c7,
        }
    }

    /// El de Windows ahora mismo (el tema de las aplicaciones).
    pub fn del_sistema() -> Tema {
        if pixpin_shell::entorno::tema_claro() {
            Tema::claro()
        } else {
            Tema::oscuro()
        }
    }
}

/// `0xRRGGBB` al `0x00BBGGRR` que quieren GDI y el `RichEdit`.
pub const fn bgr(c: Rgb) -> u32 {
    ((c & 0xff) << 16) | (c & 0xff00) | ((c >> 16) & 0xff)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_colores_van_al_reves_como_los_quiere_windows() {
        assert_eq!(bgr(0x112233), 0x00332211);
    }

    #[test]
    fn el_texto_se_lee_sobre_el_papel_en_los_dos_temas() {
        // Contraste de luminancia simple: lo bastante lejos para leerse.
        let luz = |c: Rgb| ((c >> 16) & 0xff) * 299 + ((c >> 8) & 0xff) * 587 + (c & 0xff) * 114;
        for t in [Tema::oscuro(), Tema::claro()] {
            let d = luz(t.texto).abs_diff(luz(t.papel));
            assert!(d > 150_000, "poco contraste en {}", if t.oscuro { "oscuro" } else { "claro" });
            assert_ne!(t.elegido, t.menu, "lo elegido no se distingue");
        }
    }
}
