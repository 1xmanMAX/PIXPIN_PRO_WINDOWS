//! **La barrita de marcar texto** de los lectores (1-nov-2026), como logica
//! pura: donde va y que boton hay bajo el raton.
//!
//! El usuario: «en los pines, con la T reconozco el texto y luego puedo
//! seleccionar y subrayar, tachar, resaltar u ocultar; quiero la MISMA
//! herramienta en el visor de PDF». En el pin los botones van en su barra de
//! arriba (la que aparece y desaparece); un lector no tiene esa barra, asi
//! que aqui flotan **encima de lo elegido** (debajo si arriba no cabe), con
//! los mismos botones y en el mismo orden que en el pin: copiar, los cinco
//! colores, las cinco marcas y el circulo tachado de quitar.
//!
//! Todo en pixeles de la ventana. Las marcas y los colores son los de
//! `pindesk_motor2d::marcas_texto`.

use pindesk_motor2d::marcas_texto::{COLORES, MARCAS, Marca};

/// Un boton de la barrita.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotonMarcar {
    /// Lo elegido al portapapeles (Ctrl+C).
    Copiar,
    /// El color `k` de `marcas_texto::COLORES`.
    Color(u8),
    Marcar(Marca),
    /// Quitar las marcas que caen en lo elegido.
    Quitar,
}

/// Un rectangulo en pixeles de la ventana.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caja {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

impl Caja {
    pub fn dentro(&self, (x, y): (f32, f32)) -> bool {
        x >= self.x && x < self.x + self.ancho && y >= self.y && y < self.y + self.alto
    }
}

/// Las medidas, en pixeles logicos (los del pin: `pindesk_pin::barra`).
const ALTO: f32 = 44.0;
const RELLENO: f32 = 5.0;
const HUECO: f32 = 2.0;
const PUNTO_COLOR: f32 = 22.0;
const BOTON: f32 = 34.0;
/// Lo que se separa de lo elegido.
const SEPARACION: f32 = 10.0;
/// Lo que se separa del borde de la ventana.
const MARGEN: f32 = 8.0;
/// El hueco entre copiar y los colores (una raya).
const RAYA: f32 = 9.0;

/// La barrita ya dispuesta.
#[derive(Debug, Clone, PartialEq)]
pub struct Barrita {
    pub caja: Caja,
    pub botones: Vec<(Caja, BotonMarcar)>,
    /// Donde va la raya que separa copiar de lo demas (su x).
    pub raya: f32,
}

impl Barrita {
    /// El boton bajo el punto, si hay.
    pub fn boton_en(&self, p: (f32, f32)) -> Option<BotonMarcar> {
        self.botones
            .iter()
            .find(|(c, _)| c.dentro(p))
            .map(|(_, b)| *b)
    }

    /// Si el punto cae en la barrita (aunque sea entre dos botones): un clic
    /// ahi no es del documento.
    pub fn encima(&self, p: (f32, f32)) -> bool {
        self.caja.dentro(p)
    }
}

/// **Dispone la barrita** junto a lo elegido: `eleccion` es la caja que
/// abarca lo elegido que se ve (`[x0, y0, x1, y1]` en pixeles de la
/// ventana), `ventana` lo que mide la ventana y `e` la escala de la
/// pantalla. Va centrada encima; si arriba no cabe, debajo; si tampoco,
/// pegada arriba de la ventana. Nunca se sale por los lados.
pub fn disponer(eleccion: [f32; 4], ventana: (f32, f32), e: f32) -> Barrita {
    let e = e.max(0.5);
    let alto = ALTO * e;
    let lado = BOTON * e;
    let mut botones = Vec::new();
    let mut x = RELLENO * e;
    let y_boton = (alto - lado) / 2.0;
    let mut poner = |b: BotonMarcar, ancho: f32, x: &mut f32| {
        botones.push((
            Caja {
                x: *x,
                y: y_boton,
                ancho,
                alto: lado,
            },
            b,
        ));
        *x += ancho + HUECO * e;
    };
    poner(BotonMarcar::Copiar, BOTON * e, &mut x);
    let raya = x + RAYA * e / 2.0 - HUECO * e / 2.0;
    x += RAYA * e;
    for k in 0..COLORES.len() {
        poner(BotonMarcar::Color(k as u8), PUNTO_COLOR * e, &mut x);
    }
    for m in MARCAS {
        poner(BotonMarcar::Marcar(m), BOTON * e, &mut x);
    }
    poner(BotonMarcar::Quitar, BOTON * e, &mut x);
    let ancho = x - HUECO * e + RELLENO * e;

    let (vw, vh) = ventana;
    let margen = MARGEN * e;
    let centro = (eleccion[0] + eleccion[2]) / 2.0;
    let bx = (centro - ancho / 2.0).clamp(margen, (vw - ancho - margen).max(margen));
    let arriba = eleccion[1] - SEPARACION * e - alto;
    let abajo = eleccion[3] + SEPARACION * e;
    let by = if arriba >= margen {
        arriba
    } else if abajo + alto <= vh - margen {
        abajo
    } else {
        margen
    };
    let botones = botones
        .into_iter()
        .map(|(c, b)| {
            (
                Caja {
                    x: c.x + bx,
                    y: c.y + by,
                    ..c
                },
                b,
            )
        })
        .collect();
    Barrita {
        caja: Caja {
            x: bx,
            y: by,
            ancho,
            alto,
        },
        botones,
        raya: raya + bx,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_botones_del_pin_en_su_orden() {
        let b = disponer([400.0, 300.0, 600.0, 320.0], (1200.0, 800.0), 1.0);
        let v: Vec<BotonMarcar> = b.botones.iter().map(|(_, b)| *b).collect();
        assert_eq!(v.len(), 1 + 5 + 5 + 1);
        assert_eq!(v[0], BotonMarcar::Copiar);
        assert_eq!(v[1], BotonMarcar::Color(0));
        assert_eq!(v[5], BotonMarcar::Color(4));
        assert_eq!(v[6], BotonMarcar::Marcar(Marca::Resaltar));
        assert_eq!(v[10], BotonMarcar::Marcar(Marca::Tapar));
        assert_eq!(v[11], BotonMarcar::Quitar);
        // Cada boton cae dentro de la barrita y no se pisan.
        for w in b.botones.windows(2) {
            assert!(w[0].0.x + w[0].0.ancho <= w[1].0.x);
        }
        let ultimo = b.botones.last().unwrap().0;
        assert!(ultimo.x + ultimo.ancho <= b.caja.x + b.caja.ancho);
    }

    #[test]
    fn va_encima_de_lo_elegido_y_debajo_si_arriba_no_cabe() {
        let b = disponer([400.0, 300.0, 600.0, 320.0], (1200.0, 800.0), 1.0);
        assert!(b.caja.y + b.caja.alto <= 300.0, "encima");
        let centro = b.caja.x + b.caja.ancho / 2.0;
        assert!((centro - 500.0).abs() < 1.0, "centrada");
        let b = disponer([400.0, 20.0, 600.0, 40.0], (1200.0, 800.0), 1.0);
        assert!(b.caja.y >= 40.0, "debajo");
        // Lo elegido ocupa toda la ventana: arriba del todo, sin salirse.
        let b = disponer([0.0, 5.0, 1200.0, 795.0], (1200.0, 800.0), 1.0);
        assert!(b.caja.y >= 0.0 && b.caja.y < 20.0);
    }

    #[test]
    fn no_se_sale_por_los_lados_y_crece_con_la_escala() {
        let b = disponer([0.0, 300.0, 10.0, 320.0], (1200.0, 800.0), 1.0);
        assert!(b.caja.x >= 8.0);
        let b = disponer([1190.0, 300.0, 1200.0, 320.0], (1200.0, 800.0), 1.0);
        assert!(b.caja.x + b.caja.ancho <= 1192.0 + 0.01);
        let doble = disponer([400.0, 300.0, 600.0, 320.0], (2400.0, 1600.0), 2.0);
        let uno = disponer([400.0, 300.0, 600.0, 320.0], (2400.0, 1600.0), 1.0);
        assert!((doble.caja.ancho - 2.0 * uno.caja.ancho).abs() < 0.1);
    }

    #[test]
    fn el_raton_encuentra_su_boton() {
        let b = disponer([400.0, 300.0, 600.0, 320.0], (1200.0, 800.0), 1.0);
        let (c, boton) = b.botones[7];
        assert_eq!(
            b.boton_en((c.x + c.ancho / 2.0, c.y + c.alto / 2.0)),
            Some(boton)
        );
        assert!(b.encima((c.x + 1.0, c.y + 1.0)));
        // Caso negativo: fuera no hay boton ni barrita.
        assert_eq!(b.boton_en((5.0, 5.0)), None);
        assert!(!b.encima((5.0, 5.0)));
    }
}
