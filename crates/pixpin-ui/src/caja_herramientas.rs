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

/// La barra de arriba del editor copia la de Excalidraw
/// (`docs/excalidraw/interfaz.md` §2.4): isla con 4 px de relleno, botones
/// de 36 px separados 4 px, a 16 px del borde, y separadores de 1 px con
/// 4 px de margen entre grupos.
const LADO_BARRA_LOGICO: u32 = 36;
const HUECO_BARRA_LOGICO: u32 = 4;
const RELLENO_BARRA_LOGICO: u32 = 4;
const DISTANCIA_BORDE_LOGICA: u32 = 16;
const SEPARADOR_LOGICO: u32 = 1;
const MARGEN_SEPARADOR_LOGICO: u32 = 4;
const ALTO_SEPARADOR_LOGICO: u32 = 24;

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
///
/// En el orden de la barra de Excalidraw (seleccion, rectangulo, elipse,
/// flecha, linea, dibujo, texto, borrador) y despues, en su propio grupo,
/// las que Excalidraw no tiene.
/// Las doce de la tanda cero van aqui con las demas, cada una en el grupo al
/// que pertenece y no todas juntas al final: la barra se lee por grupos, y un
/// cajon de «lo nuevo» deja de tener sentido en cuanto deja de ser nuevo. El
/// rombo entra entre el rectangulo y la elipse porque es una de las diez
/// figuras principales de Excalidraw, y las dos flechas nuevas, junto a la
/// flecha.
pub const BOTONES_EDITOR: [BotonCaja; 30] = [
    BotonCaja::Elegir(Herramienta::Mano),
    BotonCaja::Elegir(Herramienta::Lazo),
    BotonCaja::Elegir(Herramienta::Rectangulo),
    BotonCaja::Elegir(Herramienta::Rombo),
    BotonCaja::Elegir(Herramienta::Elipse),
    BotonCaja::Elegir(Herramienta::Flecha),
    BotonCaja::Elegir(Herramienta::FlechaCodos),
    BotonCaja::Elegir(Herramienta::FlechaLibre),
    BotonCaja::Elegir(Herramienta::Linea),
    BotonCaja::Elegir(Herramienta::Lapiz),
    BotonCaja::Elegir(Herramienta::Texto),
    BotonCaja::Elegir(Herramienta::Borrador),
    BotonCaja::Elegir(Herramienta::Resaltador),
    BotonCaja::Elegir(Herramienta::Foco),
    BotonCaja::Elegir(Herramienta::Lupa),
    BotonCaja::Elegir(Herramienta::Mosaico),
    BotonCaja::Elegir(Herramienta::Arco),
    BotonCaja::Elegir(Herramienta::Serie),
    BotonCaja::Elegir(Herramienta::Punto),
    BotonCaja::Elegir(Herramienta::Cota),
    BotonCaja::Elegir(Herramienta::Escalar),
    BotonCaja::Elegir(Herramienta::EscalaGrafica),
    BotonCaja::Elegir(Herramienta::Marco),
    // Las cuatro que no dibujan nada: miran lo que ya hay y lo cambian.
    BotonCaja::Elegir(Herramienta::Relleno),
    BotonCaja::Elegir(Herramienta::Recortar),
    BotonCaja::Elegir(Herramienta::Extender),
    BotonCaja::Elegir(Herramienta::CopiarEstilo),
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
    /// Barra de arriba al estilo Excalidraw en vez de columna al lado.
    horizontal: bool,
}

/// El grupo de un boton en la barra: entre grupos va un separador. Las de
/// dibujar que tiene Excalidraw, las propias de PixPin, las que trabajan
/// sobre lo que ya hay, y las acciones.
///
/// El tercer grupo es el que mas se agradece de un vistazo: el bote, recortar,
/// extender y copiar estilo **no dibujan nada**. Mezcladas con las que si
/// dibujan, un clic con una de ellas puesta parece que no ha hecho nada
/// cuando lo que ha pasado es que no habia nada cerca sobre lo que actuar.
pub fn grupo(b: BotonCaja) -> u8 {
    match b {
        BotonCaja::Elegir(
            Herramienta::Resaltador
            | Herramienta::Foco
            | Herramienta::Lupa
            | Herramienta::Mosaico
            | Herramienta::Arco
            | Herramienta::Serie
            | Herramienta::Punto
            | Herramienta::Cota
            | Herramienta::Escalar
            | Herramienta::EscalaGrafica
            | Herramienta::Marco,
        ) => 1,
        BotonCaja::Elegir(
            Herramienta::Relleno
            | Herramienta::Recortar
            | Herramienta::Extender
            | Herramienta::CopiarEstilo,
        ) => 2,
        BotonCaja::Elegir(_) => 0,
        BotonCaja::Deshacer | BotonCaja::Rehacer | BotonCaja::Color | BotonCaja::Salir => 3,
    }
}

impl CajaHerramientas {
    /// La barra de herramientas de Excalidraw: en horizontal, centrada
    /// arriba del `area` y a 16 px de su borde. Si no cabe a lo ancho, se
    /// pega a la izquierda en vez de salirse por los dos lados.
    pub fn barra_superior(
        area: Rect,
        escala_por_cien: u32,
        botones: &'static [BotonCaja],
    ) -> CajaHerramientas {
        let e = |v: u32| v * escala_por_cien / 100;
        let n = botones.len() as u32;
        let cortes = botones
            .windows(2)
            .filter(|par| grupo(par[0]) != grupo(par[1]))
            .count() as u32;
        let ancho = 2 * e(RELLENO_BARRA_LOGICO)
            + n * e(LADO_BARRA_LOGICO)
            + n.saturating_sub(1) * e(HUECO_BARRA_LOGICO)
            + cortes * (e(SEPARADOR_LOGICO) + e(MARGEN_SEPARADOR_LOGICO) + e(HUECO_BARRA_LOGICO));
        let alto = 2 * e(RELLENO_BARRA_LOGICO) + e(LADO_BARRA_LOGICO);
        let x_ideal = area.x + (area.ancho as i32 - ancho as i32) / 2;
        let x_max = (area.derecha() - ancho as i32).max(area.izquierda());
        CajaHerramientas {
            marco: Rect {
                x: x_ideal.clamp(area.izquierda(), x_max),
                y: area.y + e(DISTANCIA_BORDE_LOGICA) as i32,
                ancho,
                alto,
            },
            escala_por_cien,
            botones,
            horizontal: true,
        }
    }

    /// Cuanto se desplaza el boton `indice` por los separadores que tiene
    /// delante, en pixeles fisicos.
    fn desplazamiento_separadores(&self, indice: usize) -> u32 {
        let e = |v: u32| v * self.escala_por_cien / 100;
        let antes = self.botones[..=indice.min(self.botones.len().saturating_sub(1))]
            .windows(2)
            .filter(|par| grupo(par[0]) != grupo(par[1]))
            .count() as u32;
        antes * (e(SEPARADOR_LOGICO) + e(MARGEN_SEPARADOR_LOGICO) + e(HUECO_BARRA_LOGICO))
    }

    /// Los separadores de 1 px entre grupos de la barra. Vacio en la caja
    /// vertical, que no los lleva.
    pub fn separadores(&self) -> Vec<Rect> {
        if !self.horizontal {
            return Vec::new();
        }
        let e = |v: u32| v * self.escala_por_cien / 100;
        let alto = e(ALTO_SEPARADOR_LOGICO);
        (1..self.botones.len())
            .filter(|&i| grupo(self.botones[i - 1]) != grupo(self.botones[i]))
            .map(|i| {
                let previo = self.rect_de(i - 1);
                Rect {
                    x: previo.derecha() + e(HUECO_BARRA_LOGICO) as i32,
                    y: self.marco.y + (self.marco.alto as i32 - alto as i32) / 2,
                    ancho: e(SEPARADOR_LOGICO).max(1),
                    alto,
                }
            })
            .collect()
    }

    pub fn es_horizontal(&self) -> bool {
        self.horizontal
    }

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
            horizontal: false,
        }
    }

    /// El rectangulo de un boton por su indice.
    pub fn rect_de(&self, indice: usize) -> Rect {
        let e = |v: u32| v * self.escala_por_cien / 100;
        if self.horizontal {
            let lado = e(LADO_BARRA_LOGICO);
            let relleno = e(RELLENO_BARRA_LOGICO);
            let paso = lado + e(HUECO_BARRA_LOGICO);
            return Rect {
                x: self.marco.x
                    + (relleno + indice as u32 * paso + self.desplazamiento_separadores(indice))
                        as i32,
                y: self.marco.y + relleno as i32,
                ancho: lado,
                alto: lado,
            };
        }
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
        assert_eq!(herramientas, 27, "faltan o sobran herramientas en la caja");
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
    fn la_columna_del_anotador_cabe_entera_en_el_area_de_trabajo() {
        // `colocar` promete en su documentacion que la caja siempre queda
        // entera en el area de trabajo. Se mide con `BOTONES`, que es la
        // lista que de verdad se pinta en columna: el editor —el unico que
        // usa `BOTONES_EDITOR`— la pone en `barra_superior`, y sus treinta
        // botones puestos uno encima de otro miden 1270 px, mas alto que un
        // monitor de 1080. Probar la columna con una lista que nadie pone en
        // columna seria probar un caso que no existe.
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        assert!(
            c.marco.arriba() >= area().arriba() && c.marco.abajo() <= area().abajo(),
            "se sale por arriba o por abajo: {c:?}"
        );
        assert!(
            c.marco.izquierda() >= area().izquierda() && c.marco.derecha() <= area().derecha(),
            "se sale de lado: {c:?}"
        );
        for i in 0..BOTONES.len() {
            let r = c.rect_de(i);
            assert!(
                r.arriba() >= c.marco.arriba() && r.abajo() <= c.marco.abajo(),
                "el boton {i} se sale del marco"
            );
        }
    }

    /// **La barra del editor con sus treinta botones cabe en un monitor
    /// normal**, y sobre todo: Salir cabe.
    ///
    /// Es la medida que hay que rehacer cada vez que entra una herramienta
    /// nueva. `barra_superior` no parte la barra en dos filas: si no cabe, la
    /// pega a la izquierda y lo que sobra por la derecha —que son justo
    /// Deshacer, Rehacer y Salir— queda fuera de la pantalla y deja de poder
    /// pulsarse.
    #[test]
    fn los_treinta_botones_del_editor_caben_a_lo_ancho_y_salir_el_ultimo() {
        assert_eq!(BOTONES_EDITOR.len(), 30);
        let b = CajaHerramientas::barra_superior(area(), 100, &BOTONES_EDITOR);
        assert_eq!(b.marco.ancho, 1231, "la barra mide otra cosa: {b:?}");
        let ultimo = b.rect_de(BOTONES_EDITOR.len() - 1);
        assert_eq!(BOTONES_EDITOR[BOTONES_EDITOR.len() - 1], BotonCaja::Salir);
        assert!(
            ultimo.derecha() <= area().derecha(),
            "Salir se sale de la pantalla: {ultimo:?}"
        );
        // Caso negativo: en un portatil estrecho **no** cabe, y esto lo deja
        // dicho en vez de descubrirse en pantalla. El dia que la barra sepa
        // partirse en dos filas, esta mitad de la prueba se cae sola.
        let estrecha = Rect {
            x: 0,
            y: 0,
            ancho: 1200,
            alto: 800,
        };
        let b = CajaHerramientas::barra_superior(estrecha, 100, &BOTONES_EDITOR);
        assert!(
            b.rect_de(BOTONES_EDITOR.len() - 1).derecha() > estrecha.derecha(),
            "si esto deja de fallar, la barra ya cabe y sobra media prueba"
        );
    }

    #[test]
    fn la_barra_de_excalidraw_va_centrada_arriba_a_16_px_y_mide_44_de_alto() {
        let b = CajaHerramientas::barra_superior(area(), 100, &BOTONES_EDITOR);
        assert!(b.es_horizontal());
        assert_eq!(b.marco.y, 16);
        assert_eq!(
            b.marco.alto, 44,
            "36 de boton mas 4 de relleno arriba y abajo"
        );
        let izq = b.marco.x - area().x;
        let der = area().derecha() - b.marco.derecha();
        assert!((izq - der).abs() <= 1, "centrada: {izq} y {der}");
    }

    #[test]
    fn los_botones_de_la_barra_son_de_36_y_no_se_pisan_con_los_separadores() {
        let b = CajaHerramientas::barra_superior(area(), 100, &BOTONES_EDITOR);
        let seps = b.separadores();
        assert_eq!(
            seps.len(),
            3,
            "dibujar | propias de PixPin | sobre lo que ya hay | acciones"
        );
        for i in 0..BOTONES_EDITOR.len() {
            let r = b.rect_de(i);
            assert_eq!((r.ancho, r.alto), (36, 36));
            assert!(
                r.derecha() <= b.marco.derecha() - 4,
                "boton {i} fuera: {r:?}"
            );
            if i > 0 {
                assert!(
                    r.x >= b.rect_de(i - 1).derecha() + 4,
                    "boton {i} pisa al anterior"
                );
            }
            for s in &seps {
                assert!(r.interseccion(*s).is_none(), "boton {i} pisa un separador");
            }
        }
    }

    #[test]
    fn un_clic_en_la_barra_elige_su_boton_y_debajo_de_ella_es_lienzo() {
        let b = CajaHerramientas::barra_superior(area(), 150, &BOTONES_EDITOR);
        let r = b.rect_de(2);
        let centro = Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        };
        assert_eq!(b.destino(centro), DestinoClic::Boton(BOTONES_EDITOR[2]));
        // Caso negativo: el separador es de la barra, pero no es un boton.
        let s = b.separadores()[0];
        assert_eq!(b.destino(Punto { x: s.x, y: s.y }), DestinoClic::Caja);
        assert_eq!(
            b.destino(Punto {
                x: centro.x,
                y: b.marco.abajo() + 1
            }),
            DestinoClic::Lienzo
        );
    }

    #[test]
    fn en_una_pantalla_estrecha_la_barra_se_pega_a_la_izquierda_sin_salirse() {
        let estrecha = Rect {
            x: 0,
            y: 0,
            ancho: 400,
            alto: 800,
        };
        let b = CajaHerramientas::barra_superior(estrecha, 100, &BOTONES_EDITOR);
        assert_eq!(b.marco.x, 0);
    }

    #[test]
    fn la_caja_vertical_no_tiene_separadores() {
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES_EDITOR);
        assert!(!c.es_horizontal());
        assert!(c.separadores().is_empty());
    }
}
