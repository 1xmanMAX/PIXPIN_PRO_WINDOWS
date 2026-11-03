//! **El pintado de las pegatinas**, el boton, la tira, el menu y la lista:
//! igual en todos los visores. Las cajas salen de `pindesk_ui::pegatinas`.

use pindesk_comentarios::lista::Fila;
use pindesk_comentarios::{Comentario, Tipo};
use pindesk_render::icono::material;
use pindesk_render::{Color, Pintor, RectF};
use pindesk_store::Catalogo;
use pindesk_ui::pegatinas::{
    self as ui, Boton, Caja, DestinoBoton, EnPantalla, Lista, Menu, Opcion,
};

use crate::caja_dibujo::{hex, sombra_isla};

pub(crate) const ISLA: Color = hex(0xffffff);
pub(crate) const TINTA: Color = hex(0x1b1b1f);
pub(crate) const APAGADO: Color = hex(0x6b6b74);
pub(crate) const ENCIMA: Color = hex(0xf1f0ff);
pub(crate) const ACTIVO: Color = hex(0xe0dfff);
pub(crate) const RAYA: Color = hex(0xe3e3ea);
pub(crate) const VERDE: Color = hex(0x2e9d4f);

pub(crate) fn con_alfa(c: Color, a: f32) -> Color {
    Color { a: c.a * a, ..c }
}

pub(crate) fn rf(c: Caja) -> RectF {
    RectF {
        x: c.x,
        y: c.y,
        ancho: c.ancho,
        alto: c.alto,
    }
}

/// El color de un tipo.
pub(crate) fn color(t: Tipo) -> Color {
    hex(t.color())
}

/// **Una pegatina**: el redondel del color de su tipo con su signo, el
/// piquito hasta el ancla, la sombra; resuelta, apagada y con la marca
/// verde; con entradas, el numerito.
#[allow(clippy::too_many_arguments)] // la pegatina, su sitio, y como se ve
pub(crate) fn pegatina(
    p: &Pintor<'_>,
    tipo: Tipo,
    ancla: (f32, f32),
    e: f32,
    resuelto: bool,
    entradas: usize,
    encima: bool,
    alfa: f32,
) {
    let c = ui::centro(ancla, e);
    let r = ui::RADIO * e * if encima { 1.15 } else { 1.0 };
    let alfa = if resuelto { alfa * 0.55 } else { alfa };
    let fondo = con_alfa(color(tipo), alfa);
    // La sombra, el piquito y el redondel con su borde blanco.
    p.circulo(
        (c.0 + 0.8 * e, c.1 + 1.6 * e),
        r + 1.5 * e,
        con_alfa(Color::NEGRO, 0.22 * alfa),
    );
    p.poligono(
        &[
            (c.0 - 0.5 * r, c.1 + 0.6 * r),
            (c.0 + 0.5 * r, c.1 + 0.6 * r),
            (ancla.0, ancla.1),
        ],
        fondo,
    );
    p.circulo(c, r + 2.0 * e, con_alfa(Color::BLANCO, alfa));
    p.circulo(c, r, fondo);
    let signo = con_alfa(hex(tipo.color_del_signo()), alfa);
    match ui::signo(tipo) {
        Some(s) => {
            let tam = r * 1.25;
            let (w, h) = p.medir_texto(s, tam);
            p.texto(s, c.0 - w / 2.0, c.1 - h / 2.0, tam, signo);
        }
        None => {
            let lado = r * 1.25;
            p.icono(
                &material::FORUM,
                RectF {
                    x: c.0 - lado / 2.0,
                    y: c.1 - lado / 2.0,
                    ancho: lado,
                    alto: lado,
                },
                signo,
            );
        }
    }
    // Resuelta: la marca verde abajo a la derecha.
    if resuelto {
        let m = (c.0 + r * 0.75, c.1 + r * 0.75);
        p.circulo(m, 6.5 * e, Color::BLANCO);
        p.circulo(m, 5.0 * e, VERDE);
        p.polilinea(
            &[
                (m.0 - 2.5 * e, m.1),
                (m.0 - 0.6 * e, m.1 + 2.0 * e),
                (m.0 + 2.8 * e, m.1 - 2.2 * e),
            ],
            1.4 * e,
            Color::BLANCO,
        );
    } else if entradas > 0 {
        // Cuantas entradas tiene, arriba a la derecha.
        let n = if entradas > 9 {
            "9+".to_string()
        } else {
            entradas.to_string()
        };
        let m = (c.0 + r * 0.8, c.1 - r * 0.8);
        let tam = 9.5 * e;
        let (w, h) = p.medir_texto(&n, tam);
        let rad = (w / 2.0 + 3.0 * e).max(7.0 * e);
        p.circulo(m, rad + 1.2 * e, Color::BLANCO);
        p.circulo(m, rad, TINTA);
        p.texto(&n, m.0 - w / 2.0, m.1 - h / 2.0, tam, Color::BLANCO);
    }
}

/// Todas las pegatinas que se ven (la de `encima`, mas grande).
pub(crate) fn pegatinas(p: &Pintor<'_>, lista: &[EnPantalla], e: f32, encima: Option<&str>) {
    for x in lista {
        pegatina(
            p,
            x.tipo,
            x.ancla,
            e,
            x.resuelto,
            x.entradas,
            encima == Some(x.id.as_str()),
            1.0,
        );
    }
}

/// Una isla blanca con sombra, como las del lienzo.
pub(crate) fn isla(p: &Pintor<'_>, c: Caja, e: f32) {
    let r = rf(c);
    sombra_isla(p, r, 10.0 * e, e);
    p.rellenar_redondeado(r, 10.0 * e, ISLA);
}

/// **El boton de comentar y su tira.**
pub(crate) fn boton(
    p: &Pintor<'_>,
    b: &Boton,
    e: f32,
    abiertas: usize,
    activo: bool,
    plantando: Option<Tipo>,
    encima: Option<DestinoBoton>,
) {
    isla(p, b.boton, e);
    let dentro = Caja {
        x: b.boton.x + 4.0 * e,
        y: b.boton.y + 4.0 * e,
        ancho: b.boton.ancho - 8.0 * e,
        alto: b.boton.alto - 8.0 * e,
    };
    if activo || encima == Some(DestinoBoton::Comentar) {
        p.rellenar_redondeado(rf(dentro), 8.0 * e, if activo { ACTIVO } else { ENCIMA });
    }
    let lado = 22.0 * e;
    let (cx, cy) = b.boton.centro();
    p.icono(
        &material::FORUM,
        RectF {
            x: cx - lado / 2.0,
            y: cy - lado / 2.0,
            ancho: lado,
            alto: lado,
        },
        TINTA,
    );
    if abiertas > 0 {
        let n = if abiertas > 99 {
            "99+".to_string()
        } else {
            abiertas.to_string()
        };
        let tam = 10.0 * e;
        let (w, h) = p.medir_texto(&n, tam);
        let m = (b.boton.derecha() - 4.0 * e, b.boton.y + 4.0 * e);
        let rad = (w / 2.0 + 3.5 * e).max(8.0 * e);
        p.circulo(m, rad, color(Tipo::Urgente));
        p.texto(&n, m.0 - w / 2.0, m.1 - h / 2.0, tam, Color::BLANCO);
    }
    let Some(t) = b.tira else {
        return;
    };
    isla(p, t, e);
    for (c, d) in &b.celdas {
        let elegido = matches!(d, DestinoBoton::Tipo(x) if Some(*x) == plantando);
        if elegido || encima == Some(*d) {
            p.rellenar_redondeado(
                rf(Caja {
                    x: c.x + 2.0 * e,
                    y: c.y + 2.0 * e,
                    ancho: c.ancho - 4.0 * e,
                    alto: c.alto - 4.0 * e,
                }),
                8.0 * e,
                if elegido { ACTIVO } else { ENCIMA },
            );
        }
        match d {
            DestinoBoton::Tipo(tipo) => {
                // La pegatina misma, con el ancla abajo de la celda.
                let ancla = (c.x + c.ancho / 2.0, c.abajo() - 5.0 * e);
                pegatina(p, *tipo, ancla, e * 0.72, false, 0, false, 1.0);
            }
            DestinoBoton::Lista => {
                let lado = 22.0 * e;
                let (x, y) = c.centro();
                p.icono(
                    &material::LIST,
                    RectF {
                        x: x - lado / 2.0,
                        y: y - lado / 2.0,
                        ancho: lado,
                        alto: lado,
                    },
                    TINTA,
                );
            }
            _ => {}
        }
    }
}

/// La pastilla de «haz clic donde quieras pegarla», arriba al centro.
pub(crate) fn aviso(p: &Pintor<'_>, texto: &str, tipo: Tipo, ventana: (f32, f32), y: f32, e: f32) {
    let tam = 13.0 * e;
    let (w, h) = p.medir_texto(texto, tam);
    let caja = Caja {
        x: (ventana.0 - (w + 56.0 * e)) / 2.0,
        y,
        ancho: w + 56.0 * e,
        alto: h + 18.0 * e,
    };
    isla(p, caja, e);
    pegatina(
        p,
        tipo,
        (caja.x + 20.0 * e, caja.abajo() - 6.0 * e),
        e * 0.62,
        false,
        0,
        false,
        1.0,
    );
    p.texto(texto, caja.x + 40.0 * e, caja.y + 9.0 * e, tam, TINTA);
}

/// El rotulo de una opcion del menu.
pub(crate) fn rotulo(t: &Catalogo, o: Opcion) -> String {
    match o {
        Opcion::Tipo(x) => t.t(x.clave()),
        Opcion::Abrir => t.t("comentario-abrir"),
        Opcion::Resolver => t.t("comentario-resolver"),
        Opcion::Reabrir => t.t("comentario-reabrir"),
        Opcion::CrearTarea => t.t("comentario-crear-tarea"),
        Opcion::Quitar => t.t("comentario-quitar"),
    }
}

/// **El menu del clic derecho.**
pub(crate) fn menu(
    p: &Pintor<'_>,
    m: &Menu,
    c: &Comentario,
    textos: &Catalogo,
    e: f32,
    raton: (f32, f32),
) {
    isla(p, m.caja, e);
    for (k, (caja, o)) in m.filas.iter().enumerate() {
        if caja.dentro(raton) {
            p.rellenar_redondeado(
                rf(Caja {
                    x: caja.x + 4.0 * e,
                    ancho: caja.ancho - 8.0 * e,
                    ..*caja
                }),
                6.0 * e,
                ENCIMA,
            );
        }
        // Una raya antes de resolver y antes de quitar.
        if k > 0
            && matches!(
                o,
                Opcion::Resolver | Opcion::Reabrir | Opcion::Quitar | Opcion::Tipo(Tipo::Urgente)
            )
        {
            p.linea(
                (caja.x + 10.0 * e, caja.y),
                (caja.derecha() - 10.0 * e, caja.y),
                1.0 * e,
                RAYA,
            );
        }
        let x = caja.x + 14.0 * e;
        let (cx, cy) = (x + 9.0 * e, caja.y + caja.alto / 2.0);
        match o {
            Opcion::Tipo(t) => {
                pegatina(p, *t, (cx, cy + 9.0 * e), e * 0.55, false, 0, false, 1.0);
                if *t == c.tipo {
                    p.polilinea(
                        &[
                            (caja.derecha() - 30.0 * e, cy),
                            (caja.derecha() - 26.0 * e, cy + 4.0 * e),
                            (caja.derecha() - 18.0 * e, cy - 5.0 * e),
                        ],
                        1.8 * e,
                        TINTA,
                    );
                }
            }
            otra => {
                let icono = match otra {
                    Opcion::Abrir => &material::FORUM,
                    Opcion::Resolver | Opcion::Reabrir => &material::CHECK_CIRCLE,
                    Opcion::CrearTarea => &material::CHECKLIST,
                    _ => &material::DELETE,
                };
                let lado = 18.0 * e;
                p.icono(
                    icono,
                    RectF {
                        x: cx - lado / 2.0,
                        y: cy - lado / 2.0,
                        ancho: lado,
                        alto: lado,
                    },
                    if *otra == Opcion::Quitar {
                        color(Tipo::Urgente)
                    } else {
                        TINTA
                    },
                );
            }
        }
        let texto = rotulo(textos, *o);
        let tam = 13.0 * e;
        let (_, h) = p.medir_texto(&texto, tam);
        p.texto(&texto, x + 28.0 * e, cy - h / 2.0, tam, TINTA);
    }
}

/// **La lista «Comentarios»** del documento.
pub(crate) fn lista(
    p: &Pintor<'_>,
    l: &Lista,
    filas: &[Fila],
    textos: &Catalogo,
    e: f32,
    raton: (f32, f32),
    ahora_local: i64,
) {
    isla(p, l.caja, e);
    let tam = 14.0 * e;
    let titulo = format!("{} ({})", textos.t("comentario-lista-titulo"), filas.len());
    let (_, h) = p.medir_texto(&titulo, tam);
    p.texto(
        &titulo,
        l.cabecera.x + 14.0 * e,
        l.cabecera.y + (l.cabecera.alto - h) / 2.0,
        tam,
        TINTA,
    );
    let lado = 18.0 * e;
    let (cx, cy) = l.cerrar.centro();
    p.icono(
        &material::CLOSE,
        RectF {
            x: cx - lado / 2.0,
            y: cy - lado / 2.0,
            ancho: lado,
            alto: lado,
        },
        APAGADO,
    );
    if filas.is_empty() {
        p.texto_ajustado(
            &textos.t("comentario-lista-vacia"),
            l.caja.x + 14.0 * e,
            l.cabecera.abajo() + 6.0 * e,
            12.0 * e,
            l.caja.ancho - 28.0 * e,
            APAGADO,
        );
        return;
    }
    for (i, caja) in &l.filas {
        let Some(f) = filas.get(*i) else {
            continue;
        };
        if caja.dentro(raton) {
            p.rellenar(rf(*caja), ENCIMA);
        }
        p.linea(
            (caja.x + 12.0 * e, caja.y),
            (caja.derecha() - 12.0 * e, caja.y),
            1.0 * e,
            RAYA,
        );
        pegatina(
            p,
            f.tipo,
            (caja.x + 26.0 * e, caja.y + caja.alto / 2.0 + 12.0 * e),
            e * 0.8,
            f.resuelto,
            0,
            false,
            1.0,
        );
        let x = caja.x + 50.0 * e;
        let ancho = caja.derecha() - x - 12.0 * e;
        let resumen = if f.resumen.is_empty() {
            textos.t(f.tipo.clave())
        } else {
            f.resumen.clone()
        };
        p.texto_linea(
            &resumen,
            x,
            caja.y + 8.0 * e,
            13.0 * e,
            ancho,
            if f.resuelto { APAGADO } else { TINTA },
        );
        let mut pie = crate::recordatorios::cuando_legible(
            pindesk_shell::entorno::a_local(f.cuando),
            ahora_local,
        );
        if f.entradas > 0 {
            pie.push_str(&format!("  \u{00b7}  {} \u{1F4AC}", f.entradas));
        }
        if f.con_tarea {
            pie.push_str("  \u{00b7}  \u{2705}");
        }
        p.texto_linea(&pie, x, caja.y + 28.0 * e, 11.0 * e, ancho, APAGADO);
    }
}
