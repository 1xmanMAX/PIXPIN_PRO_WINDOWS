//! **La caja de buscar de los lectores** (D9): Ctrl+F en el de Word y
//! libros (`visor`) y en el de PDF (`lector_pdf`).
//!
//! Se copia la caja de buscar de un navegador, que es la que el usuario ya
//! conoce (el movil aun no la tiene: punto 12 de su hoja de ruta): Ctrl+F la
//! abre, lo que se teclea busca al momento, Intro o F3 van a la siguiente y
//! Mayus+Intro o Mayus+F3 a la anterior, «3 de 12», y Esc la cierra. Con la
//! caja cerrada, F3 sigue con lo ultimo buscado.
//!
//! Aqui solo va lo comun a los dos lectores: las teclas, lo escrito y como
//! se pinta la caja. Donde estan las coincidencias y como se salta a ellas
//! es de cada lector (`pixpin_docs::buscar` hace la cuenta).

#![forbid(unsafe_code)]

use pixpin_docs::buscar::Busqueda;
use pixpin_render::icono::material;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_store::Catalogo;

use crate::lector::{APAGADO, CRISTAL, DORADO, RAYA, TEXTO, con_alfa};

const VK_BACK: u32 = 0x08;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_F: u32 = 0x46;
const VK_F3: u32 = 0x72;

/// Cuantas letras se dejan escribir: mas es pegar un parrafo sin querer.
const LARGO_MAXIMO: usize = 200;

/// Las coincidencias que no son la actual: un dorado claro, que se lea el
/// texto por encima.
pub const MARCA: Color = Color {
    r: 0xe8 as f32 / 255.0,
    g: 0xc0 as f32 / 255.0,
    b: 0x6a as f32 / 255.0,
    a: 0.32,
};
/// La actual, en naranja y mas fuerte, como en el navegador.
pub const MARCA_ACTUAL: Color = Color {
    r: 1.0,
    g: 0x8c as f32 / 255.0,
    b: 0.0,
    a: 0.62,
};

/// Lo que ha pasado con una tecla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hecho {
    /// No era del buscador: que la atienda el lector.
    NoEsMia,
    /// Era del buscador y no cambia nada que ver (una letra que llega luego
    /// como caracter).
    Nada,
    /// Se abrio la caja.
    Abierta,
    /// Se cerro la caja.
    Cerrada,
    /// Cambio lo escrito: hay que volver a buscar.
    Escrito,
    Siguiente,
    Anterior,
}

/// Un boton de la caja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boton {
    Anterior,
    Siguiente,
    Cerrar,
}

#[derive(Debug, Clone, Default)]
pub struct Buscador {
    pub abierto: bool,
    pub busqueda: Busqueda,
}

impl Buscador {
    /// Atiende una tecla. Con la caja abierta, las letras sin Ctrl son suyas
    /// aunque lleguen luego como caracter: la `T` que se escribe no puede
    /// cambiar la letra del lector ni la `A` ponerse a anotar.
    pub fn tecla(&mut self, vk: u32, shift: bool, ctrl: bool) -> Hecho {
        if vk == VK_F && ctrl && !shift {
            self.abierto = true;
            return Hecho::Abierta;
        }
        if vk == VK_F3 && !self.busqueda.consulta.trim().is_empty() {
            return if shift { Hecho::Anterior } else { Hecho::Siguiente };
        }
        if !self.abierto {
            return Hecho::NoEsMia;
        }
        match vk {
            VK_ESCAPE => {
                self.abierto = false;
                Hecho::Cerrada
            }
            VK_RETURN if shift => Hecho::Anterior,
            VK_RETURN => Hecho::Siguiente,
            // Borrar llega tambien como caracter; se atiende alli.
            VK_BACK => Hecho::Nada,
            // Letras, cifras, espacio y signos: son de la caja.
            _ if !ctrl && es_de_escribir(vk) => Hecho::Nada,
            _ => Hecho::NoEsMia,
        }
    }

    /// Un caracter tecleado. `None` si no era para la caja.
    pub fn caracter(&mut self, c: char) -> Option<Hecho> {
        if !self.abierto {
            return None;
        }
        if c == '\u{8}' {
            return Some(if self.busqueda.consulta.pop().is_some() {
                Hecho::Escrito
            } else {
                Hecho::Nada
            });
        }
        // Intro, tabulador y demas de control: ya los atendio la tecla.
        if c < ' ' || c == '\u{7f}' {
            return Some(Hecho::Nada);
        }
        if self.busqueda.consulta.chars().count() >= LARGO_MAXIMO {
            return Some(Hecho::Nada);
        }
        self.busqueda.consulta.push(c);
        Some(Hecho::Escrito)
    }

    /// El boton pulsado, como si fuera su tecla.
    pub fn boton(&mut self, b: Boton) -> Hecho {
        match b {
            Boton::Anterior => Hecho::Anterior,
            Boton::Siguiente => Hecho::Siguiente,
            Boton::Cerrar => {
                self.abierto = false;
                Hecho::Cerrada
            }
        }
    }

    /// **Pinta la caja**, arriba en medio, y devuelve donde cayo cada boton.
    /// `aviso` sustituye a «n de m» cuando el lector tiene algo que decir
    /// («leyendo el texto…», «este PDF no tiene texto»).
    pub fn pintar(
        &self,
        p: &Pintor,
        ancho: f32,
        e: f32,
        textos: &Catalogo,
        aviso: Option<&str>,
    ) -> Vec<(RectF, Boton)> {
        let mut botones = Vec::new();
        if !self.abierto {
            return botones;
        }
        let tam = 15.0 * e;
        let alto = 40.0 * e;
        let ancho_caja = (460.0 * e).min(ancho - 32.0 * e);
        let caja = RectF {
            x: (ancho - ancho_caja) / 2.0,
            y: 14.0 * e,
            ancho: ancho_caja,
            alto,
        };
        p.rellenar_redondeado(caja, alto / 2.0, con_alfa(CRISTAL, 0.96));
        let lado_icono = 18.0 * e;
        p.icono(
            &material::SEARCH,
            RectF {
                x: caja.x + 12.0 * e,
                y: caja.y + (alto - lado_icono) / 2.0,
                ancho: lado_icono,
                alto: lado_icono,
            },
            APAGADO,
        );
        // A la derecha: la cuenta y los tres botones.
        let lado = 32.0 * e;
        let mut x = caja.x + caja.ancho - 6.0 * e - 3.0 * lado;
        let cuenta = match (aviso, self.busqueda.cuenta()) {
            (Some(a), _) => a.to_string(),
            (None, Some((n, m))) => {
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("n", n);
                args.set("total", m);
                textos.t_args("buscar-cuenta", &args)
            }
            (None, None) if self.busqueda.consulta.trim().is_empty() => String::new(),
            (None, None) => textos.t("buscar-nada"),
        };
        let tam_cuenta = 12.5 * e;
        let (wc, hc) = p.medir_texto(&cuenta, tam_cuenta);
        let color_cuenta = if self.busqueda.cuenta().is_none() && aviso.is_none() && !cuenta.is_empty() {
            DORADO
        } else {
            APAGADO
        };
        p.texto(&cuenta, x - wc - 8.0 * e, caja.y + (alto - hc) / 2.0, tam_cuenta, color_cuenta);
        // Lo escrito, con el cursor detras.
        let izquierda = caja.x + 12.0 * e + lado_icono + 10.0 * e;
        let hueco = (x - wc - 16.0 * e - izquierda).max(20.0 * e);
        let escrito = if self.busqueda.consulta.is_empty() {
            None
        } else {
            Some(self.busqueda.consulta.as_str())
        };
        let (texto, color) = match escrito {
            Some(t) => (t.to_string(), TEXTO),
            None => (textos.t("buscar-pista"), APAGADO),
        };
        let (wt, ht) = p.medir_texto(&texto, tam);
        let y_texto = caja.y + (alto - ht) / 2.0;
        p.texto_linea(&texto, izquierda, y_texto, tam, hueco, color);
        let x_cursor = if escrito.is_some() { izquierda + wt.min(hueco) + 1.0 } else { izquierda };
        p.rellenar(
            RectF {
                x: x_cursor,
                y: y_texto + 2.0 * e,
                ancho: 1.5 * e,
                alto: ht - 4.0 * e,
            },
            DORADO,
        );
        // Una raya fina separa los botones.
        p.rellenar(
            RectF {
                x: x - 2.0 * e,
                y: caja.y + 9.0 * e,
                ancho: 1.0,
                alto: alto - 18.0 * e,
            },
            RAYA,
        );
        let hay = self.busqueda.cuenta().is_some();
        for (b, flecha) in [(Boton::Anterior, "↑"), (Boton::Siguiente, "↓"), (Boton::Cerrar, "")] {
            let zona = RectF {
                x,
                y: caja.y,
                ancho: lado,
                alto,
            };
            let color = if b == Boton::Cerrar || hay { TEXTO } else { con_alfa(APAGADO, 0.5) };
            if flecha.is_empty() {
                p.icono(
                    &material::CLOSE,
                    RectF {
                        x: zona.x + (lado - lado_icono) / 2.0,
                        y: zona.y + (alto - lado_icono) / 2.0,
                        ancho: lado_icono,
                        alto: lado_icono,
                    },
                    color,
                );
            } else {
                let (wf, hf) = p.medir_texto(flecha, tam);
                p.texto(flecha, zona.x + (lado - wf) / 2.0, zona.y + (alto - hf) / 2.0, tam, color);
            }
            botones.push((zona, b));
            x += lado;
        }
        botones
    }
}

/// Las teclas que escriben algo (letras, cifras, espacio, signos del
/// teclado): con la caja abierta son de ella.
fn es_de_escribir(vk: u32) -> bool {
    matches!(vk, 0x20 | 0x30..=0x39 | 0x41..=0x5A | 0x60..=0x6F | 0xBA..=0xC0 | 0xDB..=0xDF | 0xE2)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn ctrl_f_abre_y_escape_cierra() {
        let mut b = Buscador::default();
        assert_eq!(b.tecla(VK_F, false, true), Hecho::Abierta);
        assert!(b.abierto);
        assert_eq!(b.tecla(VK_ESCAPE, false, false), Hecho::Cerrada);
        assert!(!b.abierto);
        // Cerrada, Escape vuelve a ser del lector (que se cierra con el).
        assert_eq!(b.tecla(VK_ESCAPE, false, false), Hecho::NoEsMia);
    }

    #[test]
    fn con_la_caja_abierta_las_letras_son_suyas_y_no_atajos_del_lector() {
        let mut b = Buscador::default();
        // Cerrada, la T (tipo de letra) y la A (anotar) son del lector.
        assert_eq!(b.tecla(0x54, false, false), Hecho::NoEsMia);
        b.tecla(VK_F, false, true);
        assert_eq!(b.tecla(0x54, false, false), Hecho::Nada);
        assert_eq!(b.tecla(0x41, true, false), Hecho::Nada, "con Mayus tambien");
        // Las flechas y AvPag siguen moviendo el documento.
        assert_eq!(b.tecla(0x28, false, false), Hecho::NoEsMia);
        assert_eq!(b.tecla(0x22, false, false), Hecho::NoEsMia);
        // Y los atajos con Ctrl siguen siendo del lector.
        assert_eq!(b.tecla(0x5A, false, true), Hecho::NoEsMia);
    }

    #[test]
    fn lo_tecleado_se_escribe_y_borrar_quita_la_ultima_letra() {
        let mut b = Buscador::default();
        // Cerrada no escribe: la letra es un atajo del lector.
        assert_eq!(b.caracter('a'), None);
        b.tecla(VK_F, false, true);
        assert_eq!(b.caracter('á'), Some(Hecho::Escrito));
        assert_eq!(b.caracter('r'), Some(Hecho::Escrito));
        assert_eq!(b.busqueda.consulta, "ár");
        assert_eq!(b.caracter('\u{8}'), Some(Hecho::Escrito));
        assert_eq!(b.busqueda.consulta, "á");
        b.caracter('\u{8}');
        assert_eq!(b.caracter('\u{8}'), Some(Hecho::Nada), "borrar en vacio no cambia nada");
        // Intro llega tambien como caracter y no se escribe.
        assert_eq!(b.caracter('\r'), Some(Hecho::Nada));
        assert!(b.busqueda.consulta.is_empty());
    }

    #[test]
    fn intro_y_f3_van_adelante_y_con_mayus_atras() {
        let mut b = Buscador::default();
        // F3 sin nada buscado no hace nada.
        assert_eq!(b.tecla(VK_F3, false, false), Hecho::NoEsMia);
        b.tecla(VK_F, false, true);
        b.caracter('x');
        assert_eq!(b.tecla(VK_RETURN, false, false), Hecho::Siguiente);
        assert_eq!(b.tecla(VK_RETURN, true, false), Hecho::Anterior);
        assert_eq!(b.tecla(VK_F3, true, false), Hecho::Anterior);
        // Cerrada, F3 sigue con lo ultimo buscado, como en el navegador.
        b.tecla(VK_ESCAPE, false, false);
        assert_eq!(b.tecla(VK_F3, false, false), Hecho::Siguiente);
    }

    #[test]
    fn no_se_escribe_un_parrafo_sin_querer() {
        let mut b = Buscador::default();
        b.tecla(VK_F, false, true);
        for _ in 0..500 {
            b.caracter('a');
        }
        assert_eq!(b.busqueda.consulta.chars().count(), LARGO_MAXIMO);
    }
}
