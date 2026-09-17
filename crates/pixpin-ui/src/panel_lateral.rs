//! El panel de propiedades de Excalidraw, a la izquierda del editor.
//!
//! Geometria pura: que secciones salen, donde cae cada control y que accion
//! corresponde a un clic. Quien lo pinta es el consumidor; quien aplica la
//! accion, el editor. Medidas de `docs/excalidraw/interfaz.md` §2.6: isla de
//! 200 px con 12 de relleno, titulos de 12 px, muestras de color de 22 y
//! botones de opcion de 32 separados 8, y 12 entre secciones.
//!
//! Que secciones salen lo decide `propiedades` (lo que tiene sentido para lo
//! elegido o para la herramienta); capas, alinear y acciones dependen de
//! cuantos elementos hay elegidos, como en Excalidraw.

use pixpin_geom::{Punto, Rect};
use pixpin_motor2d::elemento::{ColorRgba, EstiloTrazo};
use pixpin_motor2d::estilo::{CambioEstilo, EstiloDibujo, NivelGrosor};
use pixpin_motor2d::organizar::{Alineacion, Reparto};
use pixpin_motor2d::relleno::EstiloRelleno;

use crate::propiedades::Propiedad;

const ANCHO_LOGICO: u32 = 200;
const RELLENO_LOGICO: u32 = 12;
const X_LOGICA: u32 = 16;
/// Debajo de donde ira el menu de Excalidraw (16 + 36 + 24).
const Y_LOGICA: u32 = 76;
const TITULO_LOGICO: u32 = 16;
const TRAS_TITULO_LOGICO: u32 = 4;
const ENTRE_SECCIONES_LOGICO: u32 = 12;
const MUESTRA_LOGICA: u32 = 22;
const MUESTRA_ACTIVA_LOGICA: u32 = 26;
const OPCION_LOGICA: u32 = 32;
const HUECO_OPCION_LOGICO: u32 = 8;
const DESLIZADOR_LOGICO: u32 = 24;

/// Los colores rapidos de Excalidraw (`colors.ts`).
pub const COLORES_TRAZO: [ColorRgba; 5] = [
    hex(0x1e1e1e),
    hex(0xe03131),
    hex(0x2f9e44),
    hex(0x1971c2),
    hex(0xf08c00),
];
pub const COLORES_FONDO: [Option<ColorRgba>; 5] = [
    None,
    Some(hex(0xffc9c9)),
    Some(hex(0xb2f2bb)),
    Some(hex(0xa5d8ff)),
    Some(hex(0xffec99)),
];

const fn hex(rgb: u32) -> ColorRgba {
    ColorRgba::opaco(
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capa {
    Fondo,
    Atras,
    Adelante,
    Frente,
}

/// Lo que hace un clic en el panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccionPanel {
    Estilo(CambioEstilo),
    Capa(Capa),
    Alinear(Alineacion),
    Repartir(Reparto),
    Duplicar,
    Borrar,
}

/// El titulo de cada seccion. Lo traduce quien pinta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seccion {
    Trazo,
    Fondo,
    Relleno,
    Grosor,
    EstiloTrazo,
    TrazoAMano,
    Opacidad,
    Capas,
    Alinear,
    Acciones,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Control {
    Titulo {
        seccion: Seccion,
        rect: Rect,
    },
    /// Una muestra de color. `color: None` es transparente. Sin `accion` es
    /// la muestra del color actual, que solo informa.
    Muestra {
        rect: Rect,
        color: Option<ColorRgba>,
        activa: bool,
        accion: Option<AccionPanel>,
    },
    /// Un boton con icono: el consumidor elige el icono por la accion.
    Opcion {
        rect: Rect,
        accion: AccionPanel,
        activa: bool,
    },
    /// La barra de opacidad: `rect` es la pista entera; `valor` de 0 a 1.
    Deslizador {
        rect: Rect,
        valor: f32,
    },
}

/// Lo que el panel necesita saber para montarse.
#[derive(Debug, Clone, Copy)]
pub struct ContextoPanel<'a> {
    /// Las propiedades que aplican (de la herramienta o de lo elegido).
    pub propiedades: &'a [Propiedad],
    /// Los valores que se marcan como activos.
    pub estilo: EstiloDibujo,
    pub seleccionados: usize,
}

/// A quien le toca un punto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DestinoPanel {
    Accion(AccionPanel),
    /// Dentro del panel pero en ningun control: no llega al lienzo.
    Panel,
    Fuera,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelLateral {
    pub marco: Rect,
    pub controles: Vec<Control>,
}

fn casi(a: ColorRgba, b: ColorRgba) -> bool {
    (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
}

/// Va colocando secciones de arriba abajo.
struct Colocador {
    e: u32,
    dentro: i32,
    ancho_util: i32,
    y: i32,
    controles: Vec<Control>,
}

impl Colocador {
    fn px(&self, v: u32) -> i32 {
        (v * self.e / 100) as i32
    }

    fn titulo(&mut self, seccion: Seccion) {
        let alto = self.px(TITULO_LOGICO);
        self.controles.push(Control::Titulo {
            seccion,
            rect: Rect {
                x: self.dentro,
                y: self.y,
                ancho: self.ancho_util as u32,
                alto: alto as u32,
            },
        });
        self.y += alto + self.px(TRAS_TITULO_LOGICO);
    }

    /// Una fila de botones de opcion; parte en varias si no caben.
    fn fila(&mut self, opciones: &[(AccionPanel, bool)]) {
        let lado = self.px(OPCION_LOGICA);
        let hueco = self.px(HUECO_OPCION_LOGICO);
        let paso = lado + hueco;
        let por_fila = ((self.ancho_util + hueco) / paso).max(1) as usize;
        for (k, (accion, activa)) in opciones.iter().enumerate() {
            let (col, fil) = ((k % por_fila) as i32, (k / por_fila) as i32);
            self.controles.push(Control::Opcion {
                rect: Rect {
                    x: self.dentro + col * paso,
                    y: self.y + fil * paso,
                    ancho: lado as u32,
                    alto: lado as u32,
                },
                accion: *accion,
                activa: *activa,
            });
        }
        let filas = opciones.len().div_ceil(por_fila) as i32;
        self.y += filas * lado + (filas - 1).max(0) * hueco;
    }

    /// Rejilla de Excalidraw `1fr 20px 26px`: los colores rapidos a la
    /// izquierda y el actual, algo mayor, al final.
    fn colores(&mut self, lista: &[(Option<ColorRgba>, AccionPanel)], actual: Option<ColorRgba>) {
        let activa = self.px(MUESTRA_ACTIVA_LOGICA);
        let muestra = self.px(MUESTRA_LOGICA);
        let zona = self.ancho_util - activa - self.px(20);
        let paso = (zona - muestra) / (lista.len() as i32 - 1).max(1);
        let arriba = self.y + (activa - muestra) / 2;
        for (k, (color, accion)) in lista.iter().enumerate() {
            let es = match (color, actual) {
                (None, None) => true,
                (Some(a), Some(b)) => casi(*a, b),
                _ => false,
            };
            self.controles.push(Control::Muestra {
                rect: Rect {
                    x: self.dentro + k as i32 * paso,
                    y: arriba,
                    ancho: muestra as u32,
                    alto: muestra as u32,
                },
                color: *color,
                activa: es,
                accion: Some(*accion),
            });
        }
        self.controles.push(Control::Muestra {
            rect: Rect {
                x: self.dentro + self.ancho_util - activa,
                y: self.y,
                ancho: activa as u32,
                alto: activa as u32,
            },
            color: actual,
            activa: false,
            accion: None,
        });
        self.y += activa;
    }

    fn deslizador(&mut self, valor: f32) {
        let alto = self.px(DESLIZADOR_LOGICO);
        self.controles.push(Control::Deslizador {
            rect: Rect {
                x: self.dentro,
                y: self.y,
                ancho: self.ancho_util as u32,
                alto: alto as u32,
            },
            valor,
        });
        self.y += alto;
    }

    fn separar(&mut self) {
        self.y += self.px(ENTRE_SECCIONES_LOGICO);
    }
}

impl PanelLateral {
    /// `None` si no hay nada que ajustar (la mano sin nada elegido, la lupa):
    /// una isla vacia solo tapa lienzo.
    pub fn construir(area: Rect, escala_por_cien: u32, ctx: ContextoPanel) -> Option<PanelLateral> {
        let e = |v: u32| (v * escala_por_cien / 100) as i32;
        let x0 = area.x + e(X_LOGICA);
        let y0 = area.y + e(Y_LOGICA);
        let mut c = Colocador {
            e: escala_por_cien,
            dentro: x0 + e(RELLENO_LOGICO),
            ancho_util: e(ANCHO_LOGICO) - 2 * e(RELLENO_LOGICO),
            y: y0 + e(RELLENO_LOGICO),
            controles: Vec::new(),
        };
        let tiene = |p: Propiedad| ctx.propiedades.contains(&p);
        let s = ctx.estilo;

        if tiene(Propiedad::ColorTrazo) {
            c.titulo(Seccion::Trazo);
            let lista: Vec<_> = COLORES_TRAZO
                .iter()
                .map(|col| (Some(*col), AccionPanel::Estilo(CambioEstilo::Trazo(*col))))
                .collect();
            c.colores(&lista, Some(s.trazo));
            c.separar();
        }
        if tiene(Propiedad::Relleno) {
            c.titulo(Seccion::Fondo);
            let lista: Vec<_> = COLORES_FONDO
                .iter()
                .map(|col| (*col, AccionPanel::Estilo(CambioEstilo::Relleno(*col))))
                .collect();
            c.colores(&lista, s.relleno);
            c.separar();
        }
        // Con el fondo transparente no hay nada que rayar: Excalidraw esconde
        // esta seccion igual (su `hasBackground` mira el color, no la figura).
        if tiene(Propiedad::EstiloRelleno) && s.relleno.is_some() {
            c.titulo(Seccion::Relleno);
            let f = |v| {
                (
                    AccionPanel::Estilo(CambioEstilo::EstiloRelleno(v)),
                    s.estilo_relleno == v,
                )
            };
            c.fila(&[
                f(EstiloRelleno::Rayado),
                f(EstiloRelleno::Cruzado),
                f(EstiloRelleno::Solido),
            ]);
            c.separar();
        }
        if tiene(Propiedad::Grosor) {
            c.titulo(Seccion::Grosor);
            let g = |n| (AccionPanel::Estilo(CambioEstilo::Grosor(n)), s.grosor == n);
            c.fila(&[
                g(NivelGrosor::Fino),
                g(NivelGrosor::Medio),
                g(NivelGrosor::Grueso),
            ]);
            c.separar();
        }
        if tiene(Propiedad::Estilo) {
            c.titulo(Seccion::EstiloTrazo);
            let t = |v| (AccionPanel::Estilo(CambioEstilo::Estilo(v)), s.estilo == v);
            c.fila(&[
                t(EstiloTrazo::Solido),
                t(EstiloTrazo::Discontinuo),
                t(EstiloTrazo::Punteado),
            ]);
            c.separar();
        }
        if tiene(Propiedad::Rugosidad) {
            c.titulo(Seccion::TrazoAMano);
            let r = |v: f32| {
                (
                    AccionPanel::Estilo(CambioEstilo::Rugosidad(v)),
                    (s.rugosidad - v).abs() < 0.01,
                )
            };
            c.fila(&[r(0.0), r(1.0), r(2.0)]);
            c.separar();
        }
        if tiene(Propiedad::Opacidad) {
            c.titulo(Seccion::Opacidad);
            c.deslizador(s.opacidad);
            c.separar();
        }
        if ctx.seleccionados >= 1 {
            c.titulo(Seccion::Capas);
            let k = |capa| (AccionPanel::Capa(capa), false);
            c.fila(&[
                k(Capa::Fondo),
                k(Capa::Atras),
                k(Capa::Adelante),
                k(Capa::Frente),
            ]);
            c.separar();
        }
        if ctx.seleccionados >= 2 {
            c.titulo(Seccion::Alinear);
            let a = |al| (AccionPanel::Alinear(al), false);
            let mut opciones = vec![
                a(Alineacion::Izquierda),
                a(Alineacion::CentroHorizontal),
                a(Alineacion::Derecha),
                a(Alineacion::Arriba),
                a(Alineacion::CentroVertical),
                a(Alineacion::Abajo),
            ];
            // Repartir pide tres: con dos no hay nada entre medias.
            if ctx.seleccionados >= 3 {
                opciones.push((AccionPanel::Repartir(Reparto::Horizontal), false));
                opciones.push((AccionPanel::Repartir(Reparto::Vertical), false));
            }
            c.fila(&opciones);
            c.separar();
        }
        if ctx.seleccionados >= 1 {
            c.titulo(Seccion::Acciones);
            c.fila(&[(AccionPanel::Duplicar, false), (AccionPanel::Borrar, false)]);
            c.separar();
        }

        if c.controles.is_empty() {
            return None;
        }
        // El ultimo separador sobra: se cambia por el relleno de abajo.
        let alto = (c.y - e(ENTRE_SECCIONES_LOGICO) + e(RELLENO_LOGICO) - y0) as u32;
        Some(PanelLateral {
            marco: Rect {
                x: x0,
                y: y0,
                ancho: e(ANCHO_LOGICO) as u32,
                alto,
            },
            controles: c.controles,
        })
    }

    pub fn destino(&self, p: Punto) -> DestinoPanel {
        if !self.marco.contiene(p) {
            return DestinoPanel::Fuera;
        }
        for c in &self.controles {
            match *c {
                Control::Muestra {
                    rect,
                    accion: Some(a),
                    ..
                }
                | Control::Opcion {
                    rect, accion: a, ..
                } if rect.contiene(p) => return DestinoPanel::Accion(a),
                Control::Deslizador { rect, .. } if rect.contiene(p) => {
                    // Pasos de 10 %, como el deslizador de Excalidraw.
                    let t = (p.x - rect.x) as f32 / rect.ancho.max(1) as f32;
                    let valor = ((t * 10.0).round() / 10.0).clamp(0.0, 1.0);
                    return DestinoPanel::Accion(AccionPanel::Estilo(CambioEstilo::Opacidad(
                        valor,
                    )));
                }
                _ => {}
            }
        }
        DestinoPanel::Panel
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

    fn ctx(propiedades: &[Propiedad], seleccionados: usize) -> ContextoPanel<'_> {
        ContextoPanel {
            propiedades,
            estilo: EstiloDibujo::default(),
            seleccionados,
        }
    }

    /// Como `ctx` pero con fondo puesto, que es cuando el relleno se ofrece.
    fn ctx_con_fondo(propiedades: &[Propiedad]) -> ContextoPanel<'_> {
        ContextoPanel {
            propiedades,
            estilo: EstiloDibujo {
                relleno: COLORES_FONDO[1],
                ..EstiloDibujo::default()
            },
            seleccionados: 0,
        }
    }

    fn secciones(p: &PanelLateral) -> Vec<Seccion> {
        p.controles
            .iter()
            .filter_map(|c| match c {
                Control::Titulo { seccion, .. } => Some(*seccion),
                _ => None,
            })
            .collect()
    }

    fn centro(r: Rect) -> Punto {
        Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        }
    }

    #[test]
    fn el_rectangulo_ensena_trazo_fondo_grosor_estilo_trazo_a_mano_y_opacidad() {
        let props =
            crate::propiedades::de_herramienta(pixpin_motor2d::gesto::Herramienta::Rectangulo);
        let p = PanelLateral::construir(area(), 100, ctx(props, 0)).unwrap();
        assert_eq!(
            secciones(&p),
            vec![
                Seccion::Trazo,
                Seccion::Fondo,
                Seccion::Grosor,
                Seccion::EstiloTrazo,
                Seccion::TrazoAMano,
                Seccion::Opacidad
            ]
        );
        assert_eq!(p.marco.ancho, 200);
        assert_eq!((p.marco.x, p.marco.y), (16, 76));
    }

    #[test]
    fn el_relleno_se_ofrece_con_fondo_puesto_y_se_esconde_sin_el() {
        let props = [Propiedad::Relleno, Propiedad::EstiloRelleno];
        let con = PanelLateral::construir(area(), 100, ctx_con_fondo(&props)).unwrap();
        assert_eq!(secciones(&con), vec![Seccion::Fondo, Seccion::Relleno]);

        // Caso negativo: con el fondo transparente no hay nada que rayar, y
        // ofrecer tres botones que no cambian nada visible es enganar.
        let sin = PanelLateral::construir(area(), 100, ctx(&props, 0)).unwrap();
        assert_eq!(secciones(&sin), vec![Seccion::Fondo]);
    }

    #[test]
    fn una_linea_elegida_no_ensena_la_seccion_de_relleno() {
        // Caso negativo del otro lado: la propiedad ni siquiera llega, porque
        // una linea no tiene interior.
        let props = crate::propiedades::de_figura(&pixpin_motor2d::elemento::Figura::Linea {
            puntos: Vec::new(),
        });
        let p = PanelLateral::construir(area(), 100, ctx_con_fondo(props)).unwrap();
        assert!(!secciones(&p).contains(&Seccion::Relleno));
    }

    #[test]
    fn pulsar_el_rayado_cruzado_pide_ese_estilo_de_relleno() {
        let props = [Propiedad::Relleno, Propiedad::EstiloRelleno];
        let p = PanelLateral::construir(area(), 100, ctx_con_fondo(&props)).unwrap();
        let cruzado = AccionPanel::Estilo(CambioEstilo::EstiloRelleno(EstiloRelleno::Cruzado));
        let rect = p
            .controles
            .iter()
            .find_map(|c| match *c {
                Control::Opcion { rect, accion, .. } if accion == cruzado => Some(rect),
                _ => None,
            })
            .unwrap();
        assert_eq!(p.destino(centro(rect)), DestinoPanel::Accion(cruzado));

        // Y el boton marcado es el del estilo actual, el rayado.
        let marcados: Vec<AccionPanel> = p
            .controles
            .iter()
            .filter_map(|c| match *c {
                Control::Opcion {
                    accion,
                    activa: true,
                    ..
                } => Some(accion),
                _ => None,
            })
            .collect();
        assert_eq!(
            marcados,
            vec![AccionPanel::Estilo(CambioEstilo::EstiloRelleno(
                EstiloRelleno::Rayado
            ))]
        );
    }

    #[test]
    fn sin_nada_que_ajustar_no_hay_panel() {
        assert!(PanelLateral::construir(area(), 100, ctx(&[], 0)).is_none());
    }

    #[test]
    fn todos_los_controles_caben_dentro_del_panel_y_no_se_pisan() {
        let props = [
            Propiedad::ColorTrazo,
            Propiedad::Relleno,
            Propiedad::EstiloRelleno,
            Propiedad::Grosor,
            Propiedad::Estilo,
            Propiedad::Rugosidad,
            Propiedad::Opacidad,
        ];
        // Con fondo puesto y tres elegidos: el panel mas alto que existe.
        let p = PanelLateral::construir(
            area(),
            150,
            ContextoPanel {
                seleccionados: 3,
                ..ctx_con_fondo(&props)
            },
        )
        .unwrap();
        let rects: Vec<Rect> = p
            .controles
            .iter()
            .map(|c| match *c {
                Control::Titulo { rect, .. }
                | Control::Muestra { rect, .. }
                | Control::Opcion { rect, .. }
                | Control::Deslizador { rect, .. } => rect,
            })
            .collect();
        for (i, r) in rects.iter().enumerate() {
            assert_eq!(
                p.marco.interseccion(*r),
                Some(*r),
                "control {i} fuera: {r:?}"
            );
            for (j, s) in rects.iter().enumerate().skip(i + 1) {
                assert!(r.interseccion(*s).is_none(), "{i} pisa a {j}");
            }
        }
    }

    #[test]
    fn un_clic_en_el_color_rojo_cambia_el_trazo_a_rojo() {
        let p = PanelLateral::construir(area(), 100, ctx(&[Propiedad::ColorTrazo], 0)).unwrap();
        let rojo = p
            .controles
            .iter()
            .find_map(|c| match *c {
                Control::Muestra {
                    rect,
                    color: Some(col),
                    accion: Some(_),
                    ..
                } if casi(col, COLORES_TRAZO[1]) => Some(rect),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            p.destino(centro(rojo)),
            DestinoPanel::Accion(AccionPanel::Estilo(CambioEstilo::Trazo(COLORES_TRAZO[1])))
        );
    }

    #[test]
    fn la_opacidad_se_elige_por_donde_se_pulsa_en_pasos_de_diez() {
        let p = PanelLateral::construir(area(), 100, ctx(&[Propiedad::Opacidad], 0)).unwrap();
        let pista = p
            .controles
            .iter()
            .find_map(|c| match *c {
                Control::Deslizador { rect, .. } => Some(rect),
                _ => None,
            })
            .unwrap();
        let en = |fraccion: f32| Punto {
            x: pista.x + (pista.ancho as f32 * fraccion) as i32,
            y: pista.y + 2,
        };
        assert_eq!(
            p.destino(en(0.52)),
            DestinoPanel::Accion(AccionPanel::Estilo(CambioEstilo::Opacidad(0.5)))
        );
        assert_eq!(
            p.destino(en(0.0)),
            DestinoPanel::Accion(AccionPanel::Estilo(CambioEstilo::Opacidad(0.0)))
        );
    }

    #[test]
    fn alinear_sale_con_dos_elegidos_y_repartir_solo_con_tres() {
        let cuenta = |n: usize| {
            PanelLateral::construir(area(), 100, ctx(&[], n)).map_or(0, |p| {
                p.controles
                    .iter()
                    .filter(|c| {
                        matches!(
                            c,
                            Control::Opcion {
                                accion: AccionPanel::Alinear(_) | AccionPanel::Repartir(_),
                                ..
                            }
                        )
                    })
                    .count()
            })
        };
        assert_eq!(cuenta(1), 0);
        assert_eq!(cuenta(2), 6);
        assert_eq!(cuenta(3), 8);
    }

    #[test]
    fn el_hueco_del_panel_no_es_lienzo_y_fuera_si() {
        let p = PanelLateral::construir(area(), 100, ctx(&[Propiedad::Grosor], 0)).unwrap();
        assert_eq!(
            p.destino(Punto {
                x: p.marco.x + 2,
                y: p.marco.y + 2
            }),
            DestinoPanel::Panel
        );
        assert_eq!(
            p.destino(Punto {
                x: p.marco.derecha() + 5,
                y: p.marco.y + 5
            }),
            DestinoPanel::Fuera
        );
    }
}
