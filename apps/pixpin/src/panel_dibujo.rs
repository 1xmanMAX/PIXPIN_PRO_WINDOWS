//! Como se pinta el panel lateral de Excalidraw (`pixpin_ui::panel_lateral`)
//! y como se decide que muestra en el editor.
//!
//! La geometria y las acciones son de `pixpin-ui`; aqui van los colores del
//! tema claro de Excalidraw (`docs/excalidraw/interfaz.md` §2 y §5), los
//! iconos de cada opcion y los titulos.

use pixpin_geom::Rect;
use pixpin_motor2d::elemento::{ColorRgba, EstiloTrazo};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::estilo::{CambioEstilo, EstiloDibujo, NivelGrosor};
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::organizar::{self, Alineacion, Reparto};
use pixpin_render::iconos_excalidraw as i;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_ui::panel_lateral::{AccionPanel, Capa, ContextoPanel, Control, PanelLateral, Seccion};

use crate::caja_dibujo::{hex, sombra_isla};

const ISLA: Color = hex(0xffffff);
const TEXTO: Color = hex(0x1b1b1f);
const BOTON: Color = hex(0xf6f6f9);
const ACTIVO_FONDO: Color = hex(0xe0dfff);
const ACTIVO_ICONO: Color = hex(0x030064);
const CONTORNO_CLARO: Color = hex(0xebebeb);
const CONTORNO_ACTIVO: Color = hex(0x4a47b1);
const PISTA_RECORRIDA: Color = hex(0xccccff);
const PULGAR: Color = hex(0x3d3d3d);

fn rf(r: Rect) -> RectF {
    RectF {
        x: r.x as f32,
        y: r.y as f32,
        ancho: r.ancho as f32,
        alto: r.alto as f32,
    }
}

fn color_de(c: ColorRgba) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

/// El panel que toca ahora: de lo elegido si hay algo, y si no de la
/// herramienta, marcando los valores actuales. Como en Excalidraw.
pub fn panel_para(
    gesto: &Gesto,
    escena: &Escena,
    area: Rect,
    escala_por_cien: u32,
) -> Option<PanelLateral> {
    let elegidos: Vec<&pixpin_motor2d::Elemento> = gesto
        .seleccion
        .ids()
        .iter()
        .filter_map(|id| escena.buscar(*id))
        .filter(|e| !e.borrado)
        .collect();
    let (propiedades, estilo) = match elegidos.first() {
        Some(primero) => (
            pixpin_ui::propiedades::comunes(&elegidos),
            EstiloDibujo {
                trazo: primero.trazo,
                relleno: primero.relleno,
                grosor: NivelGrosor::de_elemento(&primero.figura, primero.grosor),
                estilo: primero.estilo,
                rugosidad: primero.rugosidad,
                opacidad: primero.opacidad,
            },
        ),
        None => {
            let mut s = gesto.estilo;
            if gesto.herramienta == Herramienta::Lapiz {
                s.grosor = NivelGrosor::de_elemento(
                    &pixpin_motor2d::Figura::Lapiz {
                        puntos: Vec::new(),
                        presiones: Vec::new(),
                        opciones: None,
                    },
                    gesto.grosor_tinta,
                );
            }
            (
                pixpin_ui::propiedades::de_herramienta(gesto.herramienta).to_vec(),
                s,
            )
        }
    };
    PanelLateral::construir(
        area,
        escala_por_cien,
        ContextoPanel {
            propiedades: &propiedades,
            estilo,
            seleccionados: elegidos.len(),
        },
    )
}

/// Aplica lo que se pulso en el panel. Devuelve si cambio algo que haya que
/// repintar.
pub fn aplicar(accion: AccionPanel, gesto: &mut Gesto, escena: &mut Escena) -> bool {
    let sel = gesto.seleccion.clone();
    match accion {
        AccionPanel::Estilo(cambio) => {
            // Siempre queda como el estilo de lo proximo que se dibuje; y si
            // hay algo elegido, se le aplica tambien.
            gesto.estilo.aplicar(cambio);
            if let CambioEstilo::Grosor(n) = cambio {
                gesto.grosor_tinta = n.de_tinta();
            }
            if !sel.esta_vacia() {
                pixpin_motor2d::estilo::aplicar_a(escena, &sel, cambio);
            }
        }
        AccionPanel::Capa(Capa::Fondo) => organizar::al_fondo(escena, &sel),
        AccionPanel::Capa(Capa::Atras) => organizar::atras(escena, &sel),
        AccionPanel::Capa(Capa::Adelante) => organizar::adelante(escena, &sel),
        AccionPanel::Capa(Capa::Frente) => organizar::al_frente(escena, &sel),
        AccionPanel::Alinear(a) => organizar::alinear(escena, &sel, a),
        AccionPanel::Repartir(r) => organizar::repartir(escena, &sel, r),
        AccionPanel::Duplicar => {
            // Como Ctrl+D en Excalidraw: copias desplazadas 10 unidades, y
            // quedan elegidas las copias.
            let copias: Vec<pixpin_motor2d::Elemento> = sel
                .ids()
                .iter()
                .filter_map(|id| escena.buscar(*id).cloned())
                .collect();
            if copias.is_empty() {
                return false;
            }
            escena.abrir_paso();
            let mut nuevos = Vec::new();
            for mut c in copias {
                c.mover(10.0, 10.0);
                nuevos.push(escena.anadir(c));
            }
            escena.cerrar_paso();
            gesto.seleccion.poner_todos(nuevos);
        }
        AccionPanel::Borrar => {
            gesto.evento(EventoGesto::Suprimir, escena, 1.0);
        }
    }
    true
}

fn icono_de(accion: AccionPanel) -> Option<&'static pixpin_render::icono::Icono> {
    Some(match accion {
        AccionPanel::Estilo(CambioEstilo::Grosor(NivelGrosor::Fino)) => &i::STROKE_WIDTH_BASE_ICON,
        AccionPanel::Estilo(CambioEstilo::Grosor(NivelGrosor::Medio)) => &i::STROKE_WIDTH_BOLD_ICON,
        AccionPanel::Estilo(CambioEstilo::Grosor(NivelGrosor::Grueso)) => {
            &i::STROKE_WIDTH_EXTRA_BOLD_ICON
        }
        AccionPanel::Estilo(CambioEstilo::Estilo(EstiloTrazo::Solido)) => {
            &i::STROKE_STYLE_SOLID_ICON
        }
        AccionPanel::Estilo(CambioEstilo::Estilo(EstiloTrazo::Discontinuo)) => {
            &i::STROKE_STYLE_DASHED_ICON
        }
        AccionPanel::Estilo(CambioEstilo::Estilo(EstiloTrazo::Punteado)) => {
            &i::STROKE_STYLE_DOTTED_ICON
        }
        AccionPanel::Estilo(CambioEstilo::Rugosidad(r)) if r < 0.5 => &i::SLOPPINESS_ARCHITECT_ICON,
        AccionPanel::Estilo(CambioEstilo::Rugosidad(r)) if r < 1.5 => &i::SLOPPINESS_ARTIST_ICON,
        AccionPanel::Estilo(CambioEstilo::Rugosidad(_)) => &i::SLOPPINESS_CARTOONIST_ICON,
        AccionPanel::Capa(Capa::Fondo) => &i::SEND_TO_BACK_ICON,
        AccionPanel::Capa(Capa::Atras) => &i::SEND_BACKWARD_ICON,
        AccionPanel::Capa(Capa::Adelante) => &i::BRING_FORWARD_ICON,
        AccionPanel::Capa(Capa::Frente) => &i::BRING_TO_FRONT_ICON,
        AccionPanel::Alinear(Alineacion::Izquierda) => &i::ALIGN_LEFT_ICON,
        AccionPanel::Alinear(Alineacion::CentroHorizontal) => &i::CENTER_HORIZONTALLY_ICON,
        AccionPanel::Alinear(Alineacion::Derecha) => &i::ALIGN_RIGHT_ICON,
        AccionPanel::Alinear(Alineacion::Arriba) => &i::ALIGN_TOP_ICON,
        AccionPanel::Alinear(Alineacion::CentroVertical) => &i::CENTER_VERTICALLY_ICON,
        AccionPanel::Alinear(Alineacion::Abajo) => &i::ALIGN_BOTTOM_ICON,
        AccionPanel::Repartir(Reparto::Horizontal) => &i::DISTRIBUTE_HORIZONTALLY_ICON,
        AccionPanel::Repartir(Reparto::Vertical) => &i::DISTRIBUTE_VERTICALLY_ICON,
        AccionPanel::Duplicar => &i::DUPLICATE_ICON,
        AccionPanel::Borrar => &i::TRASH_ICON,
        _ => return None,
    })
}

/// Las etiquetas de Excalidraw en espanol (`locales/es-ES.json`).
fn titulo(s: Seccion) -> &'static str {
    match s {
        Seccion::Trazo => "Trazo",
        Seccion::Fondo => "Fondo",
        Seccion::Grosor => "Grosor del trazo",
        Seccion::EstiloTrazo => "Estilo del trazo",
        Seccion::TrazoAMano => "Estilo de trazo a mano",
        Seccion::Opacidad => "Opacidad",
        Seccion::Capas => "Capas",
        Seccion::Alinear => "Alinear",
        Seccion::Acciones => "Acciones",
    }
}

/// Tablero de ajedrez del color transparente, como Excalidraw.
fn transparente(p: &Pintor, r: RectF) {
    let n = 4;
    let (w, h) = (r.ancho / n as f32, r.alto / n as f32);
    for fil in 0..n {
        for col in 0..n {
            let c = if (fil + col) % 2 == 0 {
                hex(0xffffff)
            } else {
                hex(0xd6d6d6)
            };
            p.rellenar(
                RectF {
                    x: r.x + col as f32 * w,
                    y: r.y + fil as f32 * h,
                    ancho: w,
                    alto: h,
                },
                c,
            );
        }
    }
}

pub fn pintar(p: &Pintor, panel: &PanelLateral, escala_por_cien: u32) {
    let e = escala_por_cien as f32 / 100.0;
    let marco = rf(panel.marco);
    sombra_isla(p, marco, 8.0 * e, e);
    p.rellenar_redondeado(marco, 8.0 * e, ISLA);

    for c in &panel.controles {
        match *c {
            Control::Titulo { seccion, rect } => {
                p.texto(
                    titulo(seccion),
                    rect.x as f32,
                    rect.y as f32,
                    12.0 * e,
                    TEXTO,
                );
            }
            Control::Muestra {
                rect,
                color,
                activa,
                accion,
            } => {
                let r = rf(rect);
                let radio = if accion.is_some() { 4.0 } else { 5.0 } * e;
                if activa {
                    let fuera = RectF {
                        x: r.x - 2.0 * e,
                        y: r.y - 2.0 * e,
                        ancho: r.ancho + 4.0 * e,
                        alto: r.alto + 4.0 * e,
                    };
                    p.rellenar_redondeado(fuera, radio + 2.0 * e, CONTORNO_ACTIVO);
                    p.rellenar_redondeado(
                        RectF {
                            x: r.x - 1.0 * e,
                            y: r.y - 1.0 * e,
                            ancho: r.ancho + 2.0 * e,
                            alto: r.alto + 2.0 * e,
                        },
                        radio + 1.0 * e,
                        ISLA,
                    );
                }
                match color {
                    None => p.con_recorte(r, |p| transparente(p, r)),
                    Some(col) => {
                        // Los claros llevan contorno para no perderse en la
                        // isla blanca.
                        if col.r + col.g + col.b > 2.4 {
                            p.rellenar_redondeado(
                                RectF {
                                    x: r.x - 1.0,
                                    y: r.y - 1.0,
                                    ancho: r.ancho + 2.0,
                                    alto: r.alto + 2.0,
                                },
                                radio + 1.0,
                                CONTORNO_CLARO,
                            );
                        }
                        p.rellenar_redondeado(r, radio, color_de(col));
                    }
                }
            }
            Control::Opcion {
                rect,
                accion,
                activa,
            } => {
                let r = rf(rect);
                p.rellenar_redondeado(r, 8.0 * e, if activa { ACTIVO_FONDO } else { BOTON });
                if let Some(ic) = icono_de(accion) {
                    let lado = 16.0 * e;
                    p.icono(
                        ic,
                        RectF {
                            x: r.x + (r.ancho - lado) / 2.0,
                            y: r.y + (r.alto - lado) / 2.0,
                            ancho: lado,
                            alto: lado,
                        },
                        if activa { ACTIVO_ICONO } else { TEXTO },
                    );
                }
            }
            Control::Deslizador { rect, valor } => {
                let r = rf(rect);
                let alto_pista = 4.0 * e;
                let pista = RectF {
                    x: r.x,
                    y: r.y + (r.alto - alto_pista) / 2.0,
                    ancho: r.ancho,
                    alto: alto_pista,
                };
                p.rellenar_redondeado(pista, 2.0 * e, BOTON);
                p.rellenar_redondeado(
                    RectF {
                        ancho: r.ancho * valor,
                        ..pista
                    },
                    2.0 * e,
                    PISTA_RECORRIDA,
                );
                let lado = 16.0 * e;
                p.rellenar_redondeado(
                    RectF {
                        x: (r.x + r.ancho * valor - lado / 2.0).clamp(r.x, r.x + r.ancho - lado),
                        y: r.y + (r.alto - lado) / 2.0,
                        ancho: lado,
                        alto: lado,
                    },
                    lado / 2.0,
                    PULGAR,
                );
            }
        }
    }
}
