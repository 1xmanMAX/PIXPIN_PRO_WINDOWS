//! **Las tablas del Word, pintadas como tablas** (fila D1).
//!
//! El movil las pinta como una `<table>` de su `WebView` (`DocxAHtml.kt`,
//! `tabla`, y la hoja de estilo `td{border:1px solid;padding:4px 8px;
//! vertical-align:top}`); aqui **cada celda es un `Colocado` con su caja**,
//! en su columna, con su raya alrededor. Donde cae cada celda lo decide la
//! maqueta del movil (`maqueta_movil::tabla`, K16: el reparto automatico del
//! navegador, sin la rejilla de Word, que el movil tampoco usa); aqui solo
//! se pintan su fondo y sus rayas.

use super::*;

/// La raya de las celdas: la de `td{border-color:#4a4a52}` del modo noche
/// del movil.
pub(crate) const RAYA_DE_CELDA: Color = Color {
    r: 0x4a as f32 / 255.0,
    g: 0x4a as f32 / 255.0,
    b: 0x52 as f32 / 255.0,
    a: 1.0,
};

/// Donde cae una celda: su caja entera (con sus rayas) y cuales lleva.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Caja {
    pub(crate) x: f32,
    pub(crate) ancho: f32,
    pub(crate) y: f32,
    pub(crate) alto: f32,
    /// Sin raya arriba: es la continuacion de una celda unida hacia abajo.
    pub(crate) arriba: bool,
    /// Sin raya abajo: la sigue su continuacion.
    pub(crate) abajo: bool,
    pub(crate) relleno: bool,
}

/// La caja entera de la celda de un `Colocado`, en unidades del documento.
pub(crate) fn caja_entera(_c: &Colocado, k: &Caja) -> RectF {
    RectF {
        x: k.x,
        y: k.y,
        ancho: k.ancho,
        alto: k.alto,
    }
}

/// Pinta el fondo y las rayas de una celda. `linea` es un pixel de pantalla
/// en unidades del documento: la raya es de 1 px como la del movil, de
/// cerca y de lejos.
pub(crate) fn pintar_caja(p: &Pintor, c: &Colocado, k: &Caja, linea: f32) {
    for (r, color) in rectangulos(c, k, linea) {
        p.rellenar(r, color);
    }
}

/// El fondo y las rayas de una celda como rectangulos: lo mismo pinta el
/// lector y lo mismo sale al compartir el documento.
pub(crate) fn rectangulos(c: &Colocado, k: &Caja, linea: f32) -> Vec<(RectF, Color)> {
    let r = caja_entera(c, k);
    let mut v = Vec::with_capacity(5);
    if k.relleno {
        v.push((r, CRISTAL));
    }
    let mut raya = |x: f32, y: f32, ancho: f32, alto: f32| {
        v.push((RectF { x, y, ancho, alto }, RAYA_DE_CELDA))
    };
    raya(r.x, r.y, linea, r.alto);
    raya(r.x + r.ancho - linea, r.y, linea, r.alto);
    if k.arriba {
        raya(r.x, r.y, r.ancho, linea);
    }
    if k.abajo {
        raya(r.x, r.y + r.alto - linea, r.ancho, linea);
    }
    v
}

/// Para compartir: rayas de una unidad del documento.
pub(crate) fn ordenes_de_celda(c: &Colocado, k: &Caja) -> Vec<(RectF, Color)> {
    rectangulos(c, k, 1.0)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_celda_unida_hacia_abajo_no_raya_por_donde_sigue() {
        let c = super::super::pruebas::colocado("x", 10.0);
        let k = Caja {
            x: 0.0,
            ancho: 100.0,
            y: 10.0,
            alto: 40.0,
            arriba: true,
            abajo: false,
            relleno: false,
        };
        let r = rectangulos(&c, &k, 1.0);
        assert_eq!(r.len(), 3, "dos lados y arriba");
        assert!(
            r.iter()
                .all(|(x, _)| x.y + x.alto <= 50.0 + 1e-3 && x.y >= 10.0)
        );
        // Con fondo, el velo va primero (debajo de las rayas).
        let r = rectangulos(
            &c,
            &Caja {
                relleno: true,
                abajo: true,
                ..k
            },
            1.0,
        );
        assert_eq!(r.len(), 5);
        assert_eq!(r[0].1, CRISTAL);
    }
}
