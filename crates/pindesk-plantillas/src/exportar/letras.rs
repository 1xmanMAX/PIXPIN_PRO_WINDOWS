//! **Las letras para pintar sin sistema** (PNG y PDF): ficheros TTF/OTF que
//! da quien llama. En Windows, las de PinDesk desempaquetadas
//! (`pindesk_render::letras::ficheros_llanos`: Fraunces, Work Sans, Caveat)
//! y de reserva las de Windows (Segoe UI, SimHei, Malgun, Nirmala); en las
//! pruebas, las que haya en la maquina. Una letra que no tiene un caracter
//! lo busca en las de reserva, en orden.

use ab_glyph::{Font, FontArc, GlyphId, PxScale, ScaleFont};

use crate::tema::Letra;
use crate::texto::Medidor;

/// Una cara: su fuente y sus bytes (el PDF la mete entera).
#[derive(Clone)]
pub struct Cara {
    pub fuente: FontArc,
    pub bytes: std::sync::Arc<Vec<u8>>,
    /// Si es un TTF suelto (una coleccion `.ttc` no se puede meter en el PDF).
    pub suelta: bool,
}

impl Cara {
    pub fn de_bytes(bytes: Vec<u8>) -> Option<Cara> {
        let suelta = !bytes.starts_with(b"ttcf");
        let fuente = FontArc::try_from_vec(bytes.clone()).ok()?;
        Some(Cara {
            fuente,
            bytes: std::sync::Arc::new(bytes),
            suelta,
        })
    }

    pub fn tiene(&self, c: char) -> bool {
        self.fuente.glyph_id(c).0 != 0
    }

    fn em(&self) -> f32 {
        self.fuente.units_per_em().unwrap_or(1000.0)
    }

    /// El avance de un glifo, en veces el em.
    pub fn avance(&self, g: GlyphId) -> f32 {
        self.fuente.h_advance_unscaled(g) / self.em()
    }

    pub fn ascenso(&self) -> f32 {
        self.fuente.ascent_unscaled() / self.em()
    }

    /// La escala de ab_glyph para una letra de `em` pixeles.
    pub fn escala(&self, em: f32) -> PxScale {
        PxScale::from(em * self.fuente.height_unscaled() / self.em())
    }

    pub fn interletra(&self, a: GlyphId, b: GlyphId, em: f32) -> f32 {
        self.fuente.as_scaled(self.escala(em)).kern(a, b)
    }
}

/// **Las letras**: una cara por familia y peso, y las de reserva.
#[derive(Clone, Default)]
pub struct Letras {
    caras: Vec<((String, bool), Cara)>,
    reservas: Vec<Cara>,
}

/// **Las letras de una pagina**: con las familias de su tema (titulo,
/// cuerpo y mano), que es lo que mide y pinta.
#[derive(Clone, Copy)]
pub struct Vista<'a> {
    pub letras: &'a Letras,
    pub familias: &'a [String; 3],
}

/// Un trozo de renglon en una sola cara.
pub struct Trozo<'a> {
    pub cara: &'a Cara,
    pub glifos: Vec<(GlyphId, char)>,
}

impl Letras {
    pub fn nuevas() -> Letras {
        Letras::default()
    }

    /// La cara de una familia (`Work Sans`, `Fraunces`…; con `negrita`, la
    /// gruesa). La primera que se pone es la de reserva de las familias que
    /// no se den.
    pub fn poner(&mut self, familia: &str, negrita: bool, bytes: Vec<u8>) -> bool {
        let k = (familia.trim().to_lowercase(), negrita);
        match Cara::de_bytes(bytes) {
            Some(c) => {
                self.caras.retain(|(x, _)| *x != k);
                self.caras.push((k, c));
                true
            }
            None => false,
        }
    }

    /// Las de una pagina (sus familias: `PaginaHecha::familias`).
    pub fn vista<'a>(&'a self, familias: &'a [String; 3]) -> Vista<'a> {
        Vista {
            letras: self,
            familias,
        }
    }

    /// Una de reserva (para lo que las otras no tienen).
    pub fn reserva(&mut self, bytes: Vec<u8>) -> bool {
        match Cara::de_bytes(bytes) {
            Some(c) => {
                self.reservas.push(c);
                true
            }
            None => false,
        }
    }

    pub fn vacias(&self) -> bool {
        self.caras.is_empty() && self.reservas.is_empty()
    }

    /// La cara de una familia: la pedida, la del otro peso, la del cuerpo,
    /// la primera que se puso o la primera de reserva.
    pub fn cara_de(&self, familia: &str, negrita: bool, cuerpo: &str) -> Option<&Cara> {
        let buscar = |f: &str, n: bool| {
            let f = f.trim().to_lowercase();
            self.caras
                .iter()
                .find(|((x, m), _)| *x == f && *m == n)
                .map(|(_, c)| c)
        };
        buscar(familia, negrita)
            .or_else(|| buscar(familia, !negrita))
            .or_else(|| buscar(cuerpo, negrita))
            .or_else(|| buscar(cuerpo, !negrita))
            .or_else(|| {
                self.caras
                    .iter()
                    .find(|((_, m), _)| *m == negrita)
                    .map(|(_, c)| c)
            })
            .or_else(|| self.caras.first().map(|(_, c)| c))
            .or_else(|| self.reservas.first())
    }
}

impl<'a> Vista<'a> {
    fn familia(&self, l: Letra) -> &str {
        match l {
            Letra::Titulo => &self.familias[0],
            Letra::Cuerpo => &self.familias[1],
            Letra::Mano => &self.familias[2],
        }
    }

    pub fn vacias(&self) -> bool {
        self.letras.vacias()
    }

    pub fn cara(&self, letra: Letra, negrita: bool) -> Option<&'a Cara> {
        self.letras
            .cara_de(self.familia(letra), negrita, &self.familias[1])
    }

    /// **Un renglon en trozos de una sola cara**, buscando cada letra en las
    /// de reserva si la suya no la tiene.
    pub fn trozos(&self, texto: &str, letra: Letra, negrita: bool) -> Vec<Trozo<'a>> {
        let Some(principal) = self.cara(letra, negrita) else {
            return Vec::new();
        };
        let mut sal: Vec<Trozo> = Vec::new();
        for c in texto.chars() {
            let cara = if principal.tiene(c) || c.is_whitespace() {
                principal
            } else {
                self.letras
                    .reservas
                    .iter()
                    .find(|r| r.tiene(c))
                    .unwrap_or(principal)
            };
            let g = cara.fuente.glyph_id(c);
            match sal.last_mut() {
                Some(t) if std::ptr::eq(t.cara, cara) => t.glifos.push((g, c)),
                _ => sal.push(Trozo {
                    cara,
                    glifos: vec![(g, c)],
                }),
            }
        }
        sal
    }
}

impl Medidor for Vista<'_> {
    fn ancho(&self, texto: &str, tam: f32, letra: Letra, negrita: bool) -> f32 {
        if self.vacias() {
            return crate::texto::Estimado.ancho(texto, tam, letra, negrita);
        }
        let mut w = 0.0;
        for t in self.trozos(texto, letra, negrita) {
            let mut antes: Option<GlyphId> = None;
            for (g, _) in &t.glifos {
                if let Some(a) = antes {
                    w += t.cara.interletra(a, *g, 1000.0) / 1000.0 * tam;
                }
                w += t.cara.avance(*g) * tam;
                antes = Some(*g);
            }
        }
        w
    }

    fn ascenso(&self, letra: Letra) -> f32 {
        self.cara(letra, false).map_or(0.8, Cara::ascenso)
    }
}
