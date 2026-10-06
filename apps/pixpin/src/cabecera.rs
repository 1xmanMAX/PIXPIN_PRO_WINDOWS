//! **La barra de arriba comun** de Galeria, Lecciones, Tareas y Timeline
//! (5-oct-2026).
//!
//! El usuario: «en estos 4 apartados quiero que uniformices la UI de forma
//! que todos en la parte superior se mantenga la barra de busqueda y las
//! funciones unicas de cada parte». Las medidas son las de la galeria, que
//! ya estaba asi: 64 de alto, el titulo y su subtitulo a la izquierda, el
//! buscador justo despues, los botones propios de cada ventana a la derecha
//! y la ✕ del todo. Una sola funcion para las cuatro: si una cambia, cambian
//! todas, y no vuelven a separarse.
//!
//! Solo pinta y apunta las zonas: lo que se escribe en el buscador lo lleva
//! cada ventana (cada una ya tenia su caja), y aqui llega como texto.

use pixpin_render::icono::Icono;
use pixpin_render::icono::material as mi;
use pixpin_render::{Color, Pintor, RectF};

use crate::caja_dibujo::hex;
use crate::ventanita::{Botones, dentro};

/// Alto de la barra, en pixeles logicos.
pub const ALTO: f32 = 64.0;

const FONDO: Color = hex(0x1C1C1E);
const CAJA: Color = hex(0x2A2A2D);
const BOTON: Color = hex(0x2C2C2E);
const TEXTO: Color = hex(0xF5F5F7);
const GRIS: Color = hex(0x98989D);
const AZUL: Color = hex(0x0A84FF);
const AZUL_LLENO: Color = hex(0x0060DF);

/// El buscador de la barra.
pub struct Buscador<'a, A> {
    pub texto: &'a str,
    pub pista: &'a str,
    /// Con el foco: borde azul y cursor.
    pub foco: bool,
    /// Donde va el cursor, en bytes de `texto` (`None` = al final).
    pub cursor: Option<usize>,
    /// Pulsar la caja: el foco a ella.
    pub enfocar: A,
    /// La ✕, con algo escrito.
    pub vaciar: A,
}

/// Un boton propio de la ventana, a la derecha.
pub struct Boton<'a, A> {
    pub icono: Option<&'a Icono>,
    pub rotulo: &'a str,
    /// La chapita de su atajo («Esc», «Ctrl N»).
    pub chapa: Option<&'a str>,
    /// Encendido (el modo de elegir, la vista abierta): azul.
    pub activo: bool,
    pub accion: A,
}

/// Una pestana de un control segmentado (las del timeline).
pub struct Segmento<'a, A> {
    pub rotulo: &'a str,
    pub activo: bool,
    pub accion: A,
}

/// Una mitad de un **conmutador de iconos** (la vista Lista / Tarjetas de
/// tareas): una sola pastilla gris con la mitad elegida mas clara.
///
/// POR QUE no son botones propios: dos botones sueltos, con el elegido en
/// azul lleno, parecian dos acciones distintas y el azul gritaba mas que la
/// accion principal de la ventana (el revisor de tareas-v4).
pub struct SegmentoIcono<'a, A> {
    pub icono: &'a Icono,
    pub activo: bool,
    pub accion: A,
    /// Lo que dice al pasar el raton («Tarjetas · Ctrl+T»).
    pub pista: &'a str,
}

/// El ancho fijo de la columna del titulo: asi el buscador no salta de
/// sitio al cambiar de pestana o de subtitulo (lo vio un revisor). Lo que
/// no cabe del subtitulo se corta con puntos suspensivos.
pub const ANCHO_TITULO: f32 = 200.0;

pub struct Cabecera<'a, A> {
    pub titulo: &'a str,
    pub subtitulo: &'a str,
    pub buscador: Option<Buscador<'a, A>>,
    /// De izquierda a derecha, como se leen.
    pub botones: Vec<Boton<'a, A>>,
    pub mover: A,
    pub cerrar: A,
}

/// Pinta la barra en lo alto de una ventana de ancho `w` y apunta sus zonas.
/// Devuelve su alto. Llamar DESPUES de pintar el contenido, para que tape lo
/// que se desplaza debajo y sus zonas ganen a las de el.
pub fn pintar<A: Copy>(p: &Pintor, botones: &mut Botones<A>, w: f32, s: f32, c: &Cabecera<'_, A>) -> f32 {
    pintar_con_segmentos(p, botones, w, s, c, &[])
}

/// Como [`pintar`], con un **control segmentado** a la izquierda de los
/// botones propios (las pestanas del timeline): un contenedor redondeado
/// con los segmentos dentro y el activo en una pildora azul.
pub fn pintar_con_segmentos<A: Copy>(
    p: &Pintor,
    botones: &mut Botones<A>,
    w: f32,
    s: f32,
    c: &Cabecera<'_, A>,
    segmentos: &[Segmento<'_, A>],
) -> f32 {
    pintar_todo(p, botones, w, s, c, segmentos, &[])
}

/// Como [`pintar`], con un **conmutador de iconos** a la izquierda de los
/// botones propios: una pastilla de fondo 2C2C2E, la mitad elegida en
/// 3A3A3C y su icono en blanco (la otra, en gris). Con el raton encima de
/// una mitad, su pista debajo.
pub fn pintar_con_iconos<A: Copy>(
    p: &Pintor,
    botones: &mut Botones<A>,
    w: f32,
    s: f32,
    c: &Cabecera<'_, A>,
    iconos: &[SegmentoIcono<'_, A>],
) -> f32 {
    pintar_todo(p, botones, w, s, c, &[], iconos)
}

#[allow(clippy::too_many_arguments)] // pintor, botones, ancho, escala, barra y sus dos controles
fn pintar_todo<A: Copy>(
    p: &Pintor,
    botones: &mut Botones<A>,
    w: f32,
    s: f32,
    c: &Cabecera<'_, A>,
    segmentos: &[Segmento<'_, A>],
    iconos: &[SegmentoIcono<'_, A>],
) -> f32 {
    let alto = ALTO * s;
    let caja = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto,
    };
    p.rellenar(caja, FONDO);
    p.rellenar(
        RectF {
            x: 0.0,
            y: alto - 1.0 * s,
            ancho: w,
            alto: 1.0 * s,
        },
        Color {
            a: 0.07,
            ..Color::BLANCO
        },
    );
    botones.zona(caja, c.mover);

    let x0 = 20.0 * s;
    let col = ANCHO_TITULO * s;
    crate::lecciones::ui::negrita(p, c.titulo, x0, 12.0 * s, 17.0 * s, col, TEXTO);
    p.texto_linea(c.subtitulo, x0, 36.0 * s, 12.0 * s, col, GRIS);
    let fin_titulo = x0 + col;

    // A la derecha, de fuera adentro: la ✕ y los botones propios.
    let lado = 40.0 * s;
    let yb = (alto - lado) / 2.0;
    let cerrar = RectF {
        x: w - 16.0 * s - lado,
        y: yb,
        ancho: lado,
        alto: lado,
    };
    if dentro(cerrar, botones.raton) {
        p.rellenar_redondeado(cerrar, 10.0 * s, BOTON);
    }
    p.icono(&mi::CLOSE, encoger(cerrar, 10.0 * s), GRIS);
    botones.zona(cerrar, c.cerrar);
    let mut x = cerrar.x;
    for b in c.botones.iter().rev() {
        let ancho = crate::lecciones::ui::ancho_de_boton(p, b.icono.is_some(), b.rotulo, b.chapa, s);
        // Un boton solo de icono es cuadrado.
        let ancho = if b.rotulo.is_empty() { lado } else { ancho };
        x -= 10.0 * s + ancho;
        let r = RectF {
            x,
            y: yb,
            ancho,
            alto: lado,
        };
        if b.rotulo.is_empty() {
            let fondo = if b.activo { AZUL_LLENO } else { BOTON };
            let fondo = if dentro(r, botones.raton) {
                crate::lecciones::ui::aclarar(fondo, 0.07)
            } else {
                fondo
            };
            p.rellenar_redondeado(r, 10.0 * s, fondo);
            if let Some(i) = b.icono {
                p.icono(i, encoger(r, 10.0 * s), if b.activo { Color::BLANCO } else { TEXTO });
            }
            botones.zona(r, b.accion);
        } else {
            crate::lecciones::ui::boton_v2(
                p,
                botones,
                r,
                b.accion,
                b.icono,
                b.rotulo,
                b.chapa,
                Some(if b.activo { AZUL_LLENO } else { BOTON }),
                if b.activo { Color::BLANCO } else { TEXTO },
                s,
            );
        }
    }

    // El control segmentado, 12 px a la izquierda de los botones.
    if !segmentos.is_empty() {
        let tam = 13.5 * s;
        let anchos: Vec<f32> = segmentos
            .iter()
            .map(|g| p.medir_texto(g.rotulo, tam).0 + 28.0 * s)
            .collect();
        let pad = 3.0 * s;
        let total: f32 = anchos.iter().sum::<f32>() + 2.0 * pad;
        x -= 12.0 * s + total;
        let caja = RectF {
            x,
            y: yb,
            ancho: total,
            alto: lado,
        };
        p.rellenar_redondeado(caja, 12.0 * s, BOTON);
        let mut sx = caja.x + pad;
        for (g, aw) in segmentos.iter().zip(anchos) {
            let r = RectF {
                x: sx,
                y: caja.y + pad,
                ancho: aw,
                alto: lado - 2.0 * pad,
            };
            if g.activo {
                p.rellenar_redondeado(r, r.alto / 2.0, AZUL);
            } else if dentro(r, botones.raton) {
                p.rellenar_redondeado(r, r.alto / 2.0, Color { a: 0.08, ..Color::BLANCO });
            }
            let (tw, th) = p.medir_texto(g.rotulo, tam);
            p.texto(
                g.rotulo,
                r.x + (r.ancho - tw) / 2.0,
                r.y + (r.alto - th) / 2.0,
                tam,
                if g.activo { Color::BLANCO } else { TEXTO },
            );
            botones.zona(r, g.accion);
            sx += aw;
        }
    }

    // El conmutador de iconos, igual: 12 px a la izquierda de los botones.
    let mut pista: Option<(RectF, &str)> = None;
    if !iconos.is_empty() {
        let pad = 3.0 * s;
        let mitad = 40.0 * s;
        let total = iconos.len() as f32 * mitad + 2.0 * pad;
        x -= 12.0 * s + total;
        let caja = RectF {
            x,
            y: yb,
            ancho: total,
            alto: lado,
        };
        p.rellenar_redondeado(caja, lado / 2.0, BOTON);
        for (k, g) in iconos.iter().enumerate() {
            let r = RectF {
                x: caja.x + pad + k as f32 * mitad,
                y: caja.y + pad,
                ancho: mitad,
                alto: lado - 2.0 * pad,
            };
            let encima = dentro(r, botones.raton);
            if g.activo {
                p.rellenar_redondeado(r, r.alto / 2.0, hex(0x3A3A3C));
            } else if encima {
                p.rellenar_redondeado(r, r.alto / 2.0, Color { a: 0.06, ..Color::BLANCO });
            }
            let li = 20.0 * s;
            let tinta = if g.activo || encima { Color::BLANCO } else { GRIS };
            p.icono(
                g.icono,
                RectF {
                    x: r.x + (r.ancho - li) / 2.0,
                    y: r.y + (r.alto - li) / 2.0,
                    ancho: li,
                    alto: li,
                },
                tinta,
            );
            botones.zona(r, g.accion);
            if encima && !g.pista.is_empty() {
                pista = Some((r, g.pista));
            }
        }
    }
    // La pista, lo ultimo de la barra: encima del buscador si se cruza.
    let pintar_pista = |p: &Pintor| {
        if let Some((r, t)) = pista {
            let limite = RectF {
                x: 0.0,
                y: 0.0,
                ancho: w,
                alto: 100_000.0,
            };
            crate::v2::pista::pintar(p, r, t, limite, s);
        }
    };

    // El buscador, entre el titulo y los botones.
    let Some(bus) = &c.buscador else {
        pintar_pista(p);
        return alto;
    };
    let xb = fin_titulo + 24.0 * s;
    let ancho_b = (x - 16.0 * s - xb).min(520.0 * s);
    if ancho_b < 120.0 * s {
        pintar_pista(p);
        return alto;
    }
    let caja_b = RectF {
        x: xb,
        y: yb,
        ancho: ancho_b,
        alto: lado,
    };
    let borde = if bus.foco {
        AZUL
    } else if dentro(caja_b, botones.raton) {
        Color {
            a: 0.2,
            ..Color::BLANCO
        }
    } else {
        Color {
            a: 0.1,
            ..Color::BLANCO
        }
    };
    p.rellenar_redondeado(caja_b, 10.0 * s, borde);
    p.rellenar_redondeado(encoger(caja_b, 1.0 * s), 9.0 * s, CAJA);
    botones.zona(caja_b, bus.enfocar);
    let li = 16.0 * s;
    p.icono(
        &mi::SEARCH,
        RectF {
            x: caja_b.x + 12.0 * s,
            y: caja_b.y + (lado - li) / 2.0,
            ancho: li,
            alto: li,
        },
        if bus.foco { AZUL } else { GRIS },
    );
    let tx = caja_b.x + 36.0 * s;
    let tam = 14.0 * s;
    let derecha = if bus.texto.is_empty() {
        let cw = crate::lecciones::ui::ancho_de_chapa(p, "Ctrl F", s);
        crate::lecciones::ui::chapa(
            p,
            "Ctrl F",
            caja_b.x + caja_b.ancho - 12.0 * s - cw,
            caja_b.y + lado / 2.0,
            hex(0xD1D1D6),
            hex(0x2E2E31),
            s,
        );
        cw + 20.0 * s
    } else {
        let l = 32.0 * s;
        let xr = RectF {
            x: caja_b.x + caja_b.ancho - 4.0 * s - l,
            y: caja_b.y + (lado - l) / 2.0,
            ancho: l,
            alto: l,
        };
        if dentro(xr, botones.raton) {
            p.rellenar_redondeado(xr, 8.0 * s, BOTON);
        }
        p.icono(&mi::CLOSE, encoger(xr, 8.0 * s), GRIS);
        botones.zona(xr, bus.vaciar);
        l + 8.0 * s
    };
    let campo = RectF {
        x: tx,
        y: caja_b.y,
        ancho: (caja_b.x + caja_b.ancho - derecha - tx).max(0.0),
        alto: lado,
    };
    let (_, th) = p.medir_texto("Ag", tam);
    let ty = caja_b.y + (lado - th) / 2.0;
    p.con_recorte(campo, |p| {
        if bus.texto.is_empty() {
            p.texto(bus.pista, tx, ty, tam, GRIS);
        } else {
            p.texto(bus.texto, tx, ty, tam, TEXTO);
        }
        if bus.foco {
            // El cursor donde diga la ventana: una marca de ancho cero en su
            // sitio y la caja que le da DirectWrite.
            let cursor = bus.cursor.unwrap_or(bus.texto.len()).min(bus.texto.len());
            let cx = if bus.texto.is_empty() {
                0.0
            } else {
                let mut con_marca = bus.texto.to_string();
                con_marca.insert(cursor, '\u{200B}');
                let i = bus.texto[..cursor].encode_utf16().count() as u32;
                p.cajas_de_trozo(&con_marca, tam, 100_000.0, &[], i, 1)
                    .first()
                    .map_or(0.0, |b| b.x)
            };
            p.linea((tx + cx, ty), (tx + cx, ty + th), 1.5 * s, AZUL);
        }
    });
    pintar_pista(p);
    alto
}

fn encoger(r: RectF, m: f32) -> RectF {
    RectF {
        x: r.x + m,
        y: r.y + m,
        ancho: (r.ancho - 2.0 * m).max(0.0),
        alto: (r.alto - 2.0 * m).max(0.0),
    }
}
