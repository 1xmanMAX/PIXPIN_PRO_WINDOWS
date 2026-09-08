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
///
/// Esta es la caja del anotador (la capa de pantalla y la paleta del pin):
/// `anotador.rs::construir` no sabe hacer `Cota`, `Escalar` ni
/// `EscalaGrafica` -su `match` cae en `_ => return None`-, asi que esos tres
/// botones no van aqui. Viven en `BOTONES_EDITOR`, la caja de la otra
/// superficie, que si los implementa.
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

/// La caja del editor avanzado (`apps/pixpin/src/ventana_editor.rs`), la
/// unica superficie que implementa `Cota`, `Escalar` y `EscalaGrafica` de
/// verdad: pinta `ordenes_medibles`, atiende `Peticion::Calibrar` y sabe
/// dibujar su cajetin. Por eso las tres van aqui, con las demas de dibujar,
/// antes de Deshacer.
pub const BOTONES_EDITOR: [BotonCaja; 17] = [
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
    BotonCaja::Elegir(Herramienta::Cota),
    BotonCaja::Elegir(Herramienta::Escalar),
    BotonCaja::Elegir(Herramienta::EscalaGrafica),
    BotonCaja::Deshacer,
    BotonCaja::Rehacer,
    BotonCaja::Salir,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CajaHerramientas {
    pub marco: Rect,
    escala_por_cien: u32,
    /// La lista de botones que representa esta caja. Cada superficie tiene
    /// la suya (`BOTONES` del anotador, `BOTONES_EDITOR` del editor): la caja
    /// es la misma geometria, pero da por hecho una lista, no lee una
    /// constante global.
    botones: &'static [BotonCaja],
}

impl CajaHerramientas {
    /// A la izquierda del contenido si cabe; si no, a la derecha; si tampoco,
    /// dentro y pegada al borde izquierdo. Siempre entera en el area de
    /// trabajo: una caja medio fuera de pantalla no se puede usar.
    pub fn colocar(
        contenido: Rect,
        area_trabajo: Rect,
        escala_por_cien: u32,
        botones: &'static [BotonCaja],
    ) -> CajaHerramientas {
        let e = |v: u32| v * escala_por_cien / 100;
        let lado = e(LADO_BOTON_LOGICO);
        let hueco = e(HUECO_LOGICO);
        let margen = e(MARGEN_LOGICO);
        let n = botones.len() as u32;

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
            botones,
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
        self.botones
            .iter()
            .enumerate()
            .find(|(i, _)| self.rect_de(*i).contiene(p))
            .map(|(_, b)| *b)
    }

    /// La lista de botones de esta caja, en orden: quien la pinta la
    /// necesita para saber que dibujar en cada indice.
    pub fn botones(&self) -> &'static [BotonCaja] {
        self.botones
    }

    /// Si el punto cae sobre la caja. Sirve para NO empezar un trazo al
    /// pulsar un boton: sin esto, elegir el lapiz dejaria un punto de tinta.
    pub fn contiene(&self, p: Punto) -> bool {
        self.marco.contiene(p)
    }

    /// A que le pertenece un punto del raton: a un boton concreto, al hueco
    /// de la caja (entre botones o en su margen), o al lienzo de debajo.
    ///
    /// Junta `boton_en` y `contiene` en la UNICA pregunta que hace falta
    /// antes de dejar pasar un clic al gesto: `ventana_editor::abrir` la
    /// resolvia a mano con un `if`/`continue` dentro del bucle de eventos, un
    /// sitio que no se puede probar sin ventana -y por eso un revisor pudo
    /// apagar la guarda entera con un `if false` sin que ninguna de las 842
    /// pruebas se enterara. Sacar la decision aqui, pura, es lo que la pone
    /// bajo vigilancia.
    pub fn destino(&self, p: Punto) -> DestinoClic {
        match self.boton_en(p) {
            Some(b) => DestinoClic::Boton(b),
            None if self.contiene(p) => DestinoClic::Caja,
            None => DestinoClic::Lienzo,
        }
    }
}

/// El resultado de `CajaHerramientas::destino`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinoClic {
    /// Encima de un boton: el clic lo elige o dispara, no llega al lienzo.
    Boton(BotonCaja),
    /// Dentro del marco pero fuera de todo boton (el hueco o el margen):
    /// sigue sin ser del lienzo, para no dejar un punto de tinta detras de
    /// la barra.
    Caja,
    /// Fuera de la caja: le toca al gesto de siempre.
    Lienzo,
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
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
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
        let c = CajaHerramientas::colocar(pegado, area(), 100, &BOTONES);
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
                &BOTONES,
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
        let c = CajaHerramientas::colocar(contenido(), area(), 150, &BOTONES);
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
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
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
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        assert_eq!(c.boton_en(Punto { x: 1500, y: 900 }), None);
        assert!(!c.contiene(Punto { x: 1500, y: 900 }));
    }

    #[test]
    fn en_el_hueco_entre_botones_no_hay_boton_pero_si_caja() {
        // El hueco pertenece a la caja: pulsar ahi no debe empezar un trazo
        // por detras de la barra.
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        let r0 = c.rect_de(0);
        let hueco = Punto {
            x: r0.x + 1,
            y: r0.abajo() + 1,
        };
        assert_eq!(c.boton_en(hueco), None);
        assert!(c.contiene(hueco), "el hueco sigue siendo de la caja");
    }

    #[test]
    fn estan_las_once_herramientas_del_anotador_y_las_tres_acciones() {
        // BOTONES es la caja del anotador: no lleva Cota, Escalar ni
        // EscalaGrafica porque `anotador::construir` no sabe hacerlas -su
        // `match` cae en `_ => return None`-. Ofrecer un boton que no hace
        // nada es peor que no ofrecerlo.
        let herramientas = BOTONES
            .iter()
            .filter(|b| matches!(b, BotonCaja::Elegir(_)))
            .count();
        assert_eq!(herramientas, 11, "faltan o sobran herramientas en la caja");
        assert!(!BOTONES.contains(&BotonCaja::Elegir(Herramienta::Cota)));
        assert!(!BOTONES.contains(&BotonCaja::Elegir(Herramienta::Escalar)));
        assert!(!BOTONES.contains(&BotonCaja::Elegir(Herramienta::EscalaGrafica)));
        assert!(BOTONES.contains(&BotonCaja::Deshacer));
        assert!(BOTONES.contains(&BotonCaja::Rehacer));
        assert!(BOTONES.contains(&BotonCaja::Salir));
    }

    #[test]
    fn estan_las_tres_herramientas_de_medir_en_la_caja_del_editor() {
        // BOTONES_EDITOR es la caja de `ventana_editor.rs`, la unica
        // superficie que implementa medir de verdad.
        let herramientas = BOTONES_EDITOR
            .iter()
            .filter(|b| matches!(b, BotonCaja::Elegir(_)))
            .count();
        assert_eq!(herramientas, 14, "faltan o sobran herramientas en la caja");
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Elegir(Herramienta::Cota)));
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Elegir(Herramienta::Escalar)));
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Elegir(Herramienta::EscalaGrafica)));
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Deshacer));
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Rehacer));
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Salir));
    }

    #[test]
    fn destino_de_un_punto_dentro_de_un_boton_es_ese_boton() {
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        let r = c.rect_de(1); // el lapiz
        let centro = Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        };
        assert_eq!(
            c.destino(centro),
            DestinoClic::Boton(BotonCaja::Elegir(Herramienta::Lapiz))
        );
    }

    #[test]
    fn destino_del_hueco_entre_botones_es_la_caja_no_el_lienzo() {
        // El caso que protegia la guarda de `ventana_editor::abrir`: sin
        // esto, un clic en el hueco caeria al lienzo y dejaria un trazo
        // detras de la barra.
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        let r0 = c.rect_de(0);
        let hueco = Punto {
            x: r0.x + 1,
            y: r0.abajo() + 1,
        };
        assert_eq!(c.destino(hueco), DestinoClic::Caja);
    }

    #[test]
    fn destino_justo_fuera_del_marco_es_el_lienzo() {
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        let fuera = Punto {
            x: c.marco.derecha() + 50,
            y: c.marco.y,
        };
        assert_eq!(c.destino(fuera), DestinoClic::Lienzo);
    }

    #[test]
    fn destino_en_los_bordes_del_marco() {
        // Media apertura (`Rect::contiene`): el borde superior/izquierdo
        // pertenece al marco, el primer pixel tras el inferior/derecho no.
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        let esquina_dentro = Punto {
            x: c.marco.izquierda(),
            y: c.marco.arriba(),
        };
        assert_ne!(c.destino(esquina_dentro), DestinoClic::Lienzo);

        let justo_fuera = Punto {
            x: c.marco.derecha(),
            y: c.marco.abajo() - 1,
        };
        assert_eq!(c.destino(justo_fuera), DestinoClic::Lienzo);

        let tambien_fuera = Punto {
            x: c.marco.derecha() - 1,
            y: c.marco.abajo(),
        };
        assert_eq!(c.destino(tambien_fuera), DestinoClic::Lienzo);
    }

    #[test]
    fn la_caja_con_diecisiete_botones_sigue_cabiendo_entera_en_el_area_de_trabajo() {
        // `colocar` promete en su documentacion que la caja siempre queda
        // entera en el area de trabajo. BOTONES_EDITOR es la lista mas
        // larga de las dos (17, tres mas que BOTONES): si la promesa se
        // sostiene para ella, se sostiene para cualquiera de las dos.
        //
        // El area es la de un monitor normal, no la "bajo" de
        // `la_caja_nunca_se_sale_del_area_de_trabajo`: esa es a proposito
        // mas baja que diecisiete botones (para probar el tope, no el caso
        // de uso), asi que no sirve para comprobar que "cabe entera".
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES_EDITOR);
        assert_eq!(BOTONES_EDITOR.len(), 17);
        assert!(
            c.marco.arriba() >= area().arriba() && c.marco.abajo() <= area().abajo(),
            "se sale por arriba o por abajo: {c:?}"
        );
        assert!(
            c.marco.izquierda() >= area().izquierda() && c.marco.derecha() <= area().derecha(),
            "se sale de lado: {c:?}"
        );
        for i in 0..BOTONES_EDITOR.len() {
            let r = c.rect_de(i);
            assert!(
                r.arriba() >= c.marco.arriba() && r.abajo() <= c.marco.abajo(),
                "el boton {i} se sale del marco"
            );
        }
    }
}
