//! **Los textos puestos en renglones**: lo que comparten todos los que
//! pintan, para que la pantalla, el PNG, el PDF y el SVG partan y encojan
//! igual. Cada uno da su [`Medidor`] (DirectWrite, las letras TTF, o el
//! estimado sin letras) y recibe renglones ya colocados.

use crate::modelo::Alineacion;
use crate::prim::{Prim, Vertical};
use crate::tema::{Color, Letra};

/// El interlineado (veces el tamaño de la letra).
pub const INTERLINEADO: f32 = 1.3;
/// Lo que se encoge como mucho un texto de un renglon que no cabe.
pub const ENCOGER_MIN: f32 = 0.45;

/// **Mide un renglon.** `tam` en las unidades que se quiera (mm aqui); el
/// ancho sale en las mismas.
pub trait Medidor {
    fn ancho(&self, texto: &str, tam: f32, letra: Letra, negrita: bool) -> f32;
    /// La parte de la letra sobre la linea base (veces `tam`).
    fn ascenso(&self, _letra: Letra) -> f32 {
        0.8
    }
}

/// **Sin letras**: medio em por letra latina y uno entero por ideograma.
pub struct Estimado;

impl Medidor for Estimado {
    fn ancho(&self, texto: &str, tam: f32, _letra: Letra, negrita: bool) -> f32 {
        ancho_estimado(texto, tam) * if negrita { 1.06 } else { 1.0 }
    }
}

/// El ancho estimado de un renglon.
pub fn ancho_estimado(texto: &str, tam: f32) -> f32 {
    texto
        .chars()
        .map(|c| {
            if c.is_whitespace() {
                0.28
            } else if (c as u32) >= 0x2E80 {
                1.0
            } else if c.is_uppercase() {
                0.64
            } else if ".,;:!|il'".contains(c) {
                0.28
            } else {
                0.52
            }
        })
        .sum::<f32>()
        * tam
}

/// **Un renglon listo para pintar**: `x` a la izquierda, `y` la linea base.
#[derive(Debug, Clone, PartialEq)]
pub struct Renglon {
    pub texto: String,
    pub x: f32,
    pub base: f32,
    /// Lo alto de la letra (mm), ya encogida si hacia falta.
    pub tam: f32,
    pub ancho: f32,
    pub letra: Letra,
    pub negrita: bool,
    pub color: Color,
}

/// Parte `texto` en renglones de `ancho` como mucho (por palabras; una
/// palabra mas larga que el ancho se corta por letras). Los saltos de linea
/// del texto se respetan.
pub fn partir(
    texto: &str,
    ancho: f32,
    tam: f32,
    letra: Letra,
    negrita: bool,
    m: &dyn Medidor,
) -> Vec<String> {
    let mut sal = Vec::new();
    for parrafo in texto.split('\n') {
        let mut linea = String::new();
        for palabra in parrafo.split(' ').filter(|p| !p.is_empty()) {
            let prueba = if linea.is_empty() {
                palabra.to_string()
            } else {
                format!("{linea} {palabra}")
            };
            if m.ancho(&prueba, tam, letra, negrita) <= ancho || linea.is_empty() {
                linea = prueba;
            } else {
                sal.push(std::mem::take(&mut linea));
                linea = palabra.to_string();
            }
            // Una palabra sola que no cabe: por letras.
            while m.ancho(&linea, tam, letra, negrita) > ancho && linea.chars().count() > 1 {
                let mut corte = String::new();
                let mut resto = String::new();
                for c in linea.chars() {
                    if resto.is_empty()
                        && m.ancho(&format!("{corte}{c}"), tam, letra, negrita) <= ancho
                    {
                        corte.push(c);
                    } else {
                        resto.push(c);
                    }
                }
                if corte.is_empty() {
                    break;
                }
                sal.push(corte);
                linea = resto;
            }
        }
        sal.push(linea);
    }
    sal
}

/// **Los renglones de una primitiva de texto** (vacio si no lo es).
pub fn renglones(p: &Prim, m: &dyn Medidor) -> Vec<Renglon> {
    let Prim::Texto {
        caja,
        texto,
        tam,
        letra,
        negrita,
        color,
        al,
        v,
        partir: partido,
    } = p
    else {
        return Vec::new();
    };
    if texto.trim().is_empty() || caja.w <= 0.0 {
        return Vec::new();
    }
    let (lineas, tam) = if *partido {
        let mut l = partir(texto, caja.w, *tam, *letra, *negrita, m);
        let alto_linea = tam * INTERLINEADO;
        let caben = ((caja.h / alto_linea).floor() as usize).max(1);
        l.truncate(caben);
        (l, *tam)
    } else {
        let ancho = m.ancho(texto, *tam, *letra, *negrita);
        let mut t = *tam;
        if ancho > caja.w && ancho > 0.0 {
            t = (*tam * caja.w / ancho).max(*tam * ENCOGER_MIN);
        }
        // Tampoco mas alto que su caja.
        t = t.min(caja.h.max(0.5));
        (vec![texto.replace('\n', " ")], t)
    };
    let alto_linea = tam * INTERLINEADO;
    let total = alto_linea * lineas.len() as f32;
    let y0 = match v {
        Vertical::Arriba => caja.y,
        Vertical::Centro => caja.y + (caja.h - total) / 2.0,
        Vertical::Abajo => caja.y + caja.h - total,
    };
    // La linea base: el ascenso dentro del renglon, centrado el sobrante.
    let asc = m.ascenso(*letra);
    lineas
        .into_iter()
        .enumerate()
        .filter(|(_, l)| !l.is_empty())
        .map(|(i, l)| {
            let ancho = m.ancho(&l, tam, *letra, *negrita);
            let x = match al {
                Alineacion::Izquierda => caja.x,
                Alineacion::Centro => caja.x + (caja.w - ancho) / 2.0,
                Alineacion::Derecha => caja.x + caja.w - ancho,
            };
            let base = y0 + alto_linea * i as f32 + (alto_linea - tam) / 2.0 + tam * asc;
            Renglon {
                texto: l,
                x,
                base,
                tam,
                ancho,
                letra: *letra,
                negrita: *negrita,
                color: *color,
            }
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::prim::Caja;

    #[test]
    fn partir_por_palabras_y_por_letras() {
        let m = Estimado;
        let l = partir(
            "uno dos tres cuatro cinco",
            8.0,
            2.0,
            Letra::Cuerpo,
            false,
            &m,
        );
        assert!(l.len() > 1);
        assert!(
            l.iter()
                .all(|x| m.ancho(x, 2.0, Letra::Cuerpo, false) <= 8.0 + 1e-3)
        );
        assert_eq!(l.join(" "), "uno dos tres cuatro cinco");
        let largo = partir("supercalifragilistico", 5.0, 2.0, Letra::Cuerpo, false, &m);
        assert!(largo.len() > 2);
        assert_eq!(largo.concat(), "supercalifragilistico");
        assert_eq!(
            partir("a\nb", 100.0, 2.0, Letra::Cuerpo, false, &m),
            vec!["a", "b"]
        );
    }

    #[test]
    fn un_renglon_que_no_cabe_se_encoge_y_se_centra() {
        let p = Prim::Texto {
            caja: Caja::nueva(10.0, 10.0, 40.0, 10.0),
            texto: "Un titulo bastante largo".into(),
            tam: 5.0,
            letra: Letra::Titulo,
            negrita: true,
            color: Color::hex(0),
            al: Alineacion::Centro,
            v: Vertical::Centro,
            partir: false,
        };
        let r = renglones(&p, &Estimado);
        assert_eq!(r.len(), 1);
        assert!(r[0].tam < 5.0);
        assert!(r[0].ancho <= 40.0 + 1e-3);
        assert!(r[0].x >= 10.0 - 1e-3);
        assert!(r[0].base > 10.0 && r[0].base < 20.0);
    }
}
