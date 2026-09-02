//! La caja de herramientas de anotación: dónde está cada botón.
//!
//! Es geometría pura, como la barra de resultado: decide posiciones y dice
//! qué botón hay bajo un punto. Quien la dibuja es el consumidor, con su
//! pintor; quien reacciona es la máquina de anotar.
//!
//! Va en vertical y pegada a un lado, no en horizontal sobre el contenido:
//! anotando se mira lo que hay debajo, y una barra ancha atravesada tapa
//! justo lo que se quiere anotar.

use pixpin_geom::{Punto, Rect};

use crate::anotador::Herramienta;

/// Medidas en pixeles logicos (al 100 %).
const LADO_BOTON_LOGICO: u32 = 40;
const HUECO_LOGICO: u32 = 2;
const MARGEN_LOGICO: u32 = 6;
/// Separacion entre la caja y el borde del contenido.
const SEPARACION_LOGICA: u32 = 12;

/// Lo que se puede pulsar. Las herramientas y, al final, las acciones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotonCaja {
    Elegir(Herramienta),
    Deshacer,
    Rehacer,
    /// Abre la paleta de colores (el consumidor decide como).
    Color,
    Salir,
}

/// El orden en que se ven. La mano primero porque es a la que se vuelve, y
/// las acciones al final, separadas por su propio grupo.
pub const BOTONES: [BotonCaja; 14] = [
    BotonCaja::Elegir(Herramienta::Mano),
    BotonCaja::Elegir(Herramienta::Lapiz),
    BotonCaja::Elegir(Herramienta::Resaltador),
    BotonCaja::Elegir(Herramienta::Linea),
    BotonCaja::Elegir(Herramienta::Flecha),
    BotonCaja::Elegir(Herramienta::Rectangulo),
    BotonCaja::Elegir(Herramienta::Elipse),
    BotonCaja::Elegir(Herramienta::Texto),
    BotonCaja::Elegir(Herramienta::Foco),
    BotonCaja::Elegir(Herramienta::Lupa),
    BotonCaja::Elegir(Herramienta::Borrador),
    BotonCaja::Deshacer,
    BotonCaja::Rehacer,
    BotonCaja::Salir,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CajaHerramientas {
    pub marco: Rect,
    escala_por_cien: u32,
}

impl CajaHerramientas {
    /// A la izquierda del contenido si cabe; si no, a la derecha; si tampoco,
    /// dentro y pegada al borde izquierdo. Siempre entera en el area de
    /// trabajo: una caja medio fuera de pantalla no se puede usar.
    pub fn colocar(contenido: Rect, area_trabajo: Rect, escala_por_cien: u32) -> CajaHerramientas {
        let e = |v: u32| v * escala_por_cien / 100;
        let lado = e(LADO_BOTON_LOGICO);
        let hueco = e(HUECO_LOGICO);
        let margen = e(MARGEN_LOGICO);
        let n = BOTONES.len() as u32;

        let ancho = lado + 2 * margen;
        let alto = n * lado + (n - 1) * hueco + 2 * margen;
        let sep = e(SEPARACION_LOGICA) as i32;

        let izquierda = contenido.x - sep - ancho as i32;
        let derecha = contenido.x + contenido.ancho as i32 + sep;
        let x = if izquierda >= area_trabajo.izquierda() {
            izquierda
        } else if derecha + ancho as i32 <= area_trabajo.derecha() {
            derecha
        } else {
            contenido.x
        };

        // Centrada en vertical sobre el contenido, y luego sujeta al area:
        // con muchas herramientas la caja es alta y en un monitor pequeño se
        // saldria por arriba y por abajo a la vez.
        let y_ideal = contenido.y + (contenido.alto as i32 - alto as i32) / 2;
        let y_max = (area_trabajo.abajo() - alto as i32).max(area_trabajo.arriba());
        let y = y_ideal.clamp(area_trabajo.arriba(), y_max);

        CajaHerramientas {
            marco: Rect {
                x: x.clamp(
                    area_trabajo.izquierda(),
                    (area_trabajo.derecha() - ancho as i32).max(area_trabajo.izquierda()),
                ),
                y,
                ancho,
                alto,
            },
            escala_por_cien,
        }
    }

    /// El rectangulo de un boton por su indice.
    pub fn rect_de(&self, indice: usize) -> Rect {
        let e = |v: u32| v * self.escala_por_cien / 100;
        let lado = e(LADO_BOTON_LOGICO);
        let hueco = e(HUECO_LOGICO);
        let margen = e(MARGEN_LOGICO);
        Rect {
            x: self.marco.x + margen as i32,
            y: self.marco.y + margen as i32 + indice as i32 * (lado + hueco) as i32,
            ancho: lado,
            alto: lado,
        }
    }

    /// Que boton hay bajo el punto, si hay alguno.
    pub fn boton_en(&self, p: Punto) -> Option<BotonCaja> {
        if !self.marco.contiene(p) {
            return None;
        }
        BOTONES
            .iter()
            .enumerate()
            .find(|(i, _)| self.rect_de(*i).contiene(p))
            .map(|(_, b)| *b)
    }

    /// Si el punto cae sobre la caja. Sirve para NO empezar un trazo al
    /// pulsar un boton: sin esto, elegir el lapiz dejaria un punto de tinta.
    pub fn contiene(&self, p: Punto) -> bool {
        self.marco.contiene(p)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn area() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        }
    }

    fn contenido() -> Rect {
        Rect {
            x: 600,
            y: 300,
            ancho: 400,
            alto: 300,
        }
    }

    #[test]
    fn la_caja_va_a_la_izquierda_si_cabe() {
        let c = CajaHerramientas::colocar(contenido(), area(), 100);
        assert!(
            c.marco.derecha() <= contenido().x,
            "deberia quedar a la izquierda del contenido"
        );
        assert!(c.marco.x >= 0);
    }

    #[test]
    fn si_no_cabe_a_la_izquierda_se_va_a_la_derecha() {
        // Un pin pegado al borde izquierdo de la pantalla.
        let pegado = Rect {
            x: 5,
            y: 300,
            ancho: 400,
            alto: 300,
        };
        let c = CajaHerramientas::colocar(pegado, area(), 100);
        assert!(
            c.marco.x >= pegado.derecha(),
            "deberia irse a la derecha, esta en {}",
            c.marco.x
        );
    }

    #[test]
    fn la_caja_nunca_se_sale_del_area_de_trabajo() {
        // Caso negativo del centrado: en un monitor bajo, una caja de 14
        // botones no cabe centrada y se saldria por arriba y por abajo.
        let bajo = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 500,
        };
        for y in [-200, 0, 250, 480, 900] {
            let c = CajaHerramientas::colocar(
                Rect {
                    x: 600,
                    y,
                    ancho: 400,
                    alto: 300,
                },
                bajo,
                100,
            );
            assert!(
                c.marco.arriba() >= bajo.arriba(),
                "se sale por arriba: {c:?}"
            );
            assert!(
                c.marco.izquierda() >= bajo.izquierda() && c.marco.derecha() <= bajo.derecha(),
                "se sale de lado: {c:?}"
            );
        }
    }

    #[test]
    fn cada_boton_cae_dentro_del_marco_y_no_se_solapa_con_el_siguiente() {
        let c = CajaHerramientas::colocar(contenido(), area(), 150);
        for i in 0..BOTONES.len() {
            let r = c.rect_de(i);
            assert!(
                r.arriba() >= c.marco.arriba() && r.abajo() <= c.marco.abajo(),
                "el boton {i} se sale del marco"
            );
            if i + 1 < BOTONES.len() {
                assert!(
                    r.abajo() <= c.rect_de(i + 1).arriba(),
                    "los botones {i} y {} se solapan",
                    i + 1
                );
            }
        }
    }

    #[test]
    fn se_encuentra_el_boton_bajo_el_punto() {
        let c = CajaHerramientas::colocar(contenido(), area(), 100);
        let r = c.rect_de(1); // el lapiz
        let centro = Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        };
        assert_eq!(
            c.boton_en(centro),
            Some(BotonCaja::Elegir(Herramienta::Lapiz))
        );
    }

    #[test]
    fn fuera_de_la_caja_no_hay_boton() {
        // Es lo que distingue "elegir herramienta" de "empezar a dibujar":
        // sin esto, pulsar junto a la caja no dibujaria.
        let c = CajaHerramientas::colocar(contenido(), area(), 100);
        assert_eq!(c.boton_en(Punto { x: 1500, y: 900 }), None);
        assert!(!c.contiene(Punto { x: 1500, y: 900 }));
    }

    #[test]
    fn en_el_hueco_entre_botones_no_hay_boton_pero_si_caja() {
        // El hueco pertenece a la caja: pulsar ahi no debe empezar un trazo
        // por detras de la barra.
        let c = CajaHerramientas::colocar(contenido(), area(), 100);
        let r0 = c.rect_de(0);
        let hueco = Punto {
            x: r0.x + 1,
            y: r0.abajo() + 1,
        };
        assert_eq!(c.boton_en(hueco), None);
        assert!(c.contiene(hueco), "el hueco sigue siendo de la caja");
    }

    #[test]
    fn estan_las_once_herramientas_y_las_tres_acciones() {
        let herramientas = BOTONES
            .iter()
            .filter(|b| matches!(b, BotonCaja::Elegir(_)))
            .count();
        assert_eq!(herramientas, 11, "faltan herramientas en la caja");
        assert!(BOTONES.contains(&BotonCaja::Deshacer));
        assert!(BOTONES.contains(&BotonCaja::Rehacer));
        assert!(BOTONES.contains(&BotonCaja::Salir));
    }
}
