//! **La historia** de un momento o de una leccion (5-oct-2026, noche).
//!
//! El usuario: «Momentos y Estado son lo mismo, lo que los diferencia es la
//! visualizacion. Quiero que cuando les de clic pueda entrar a una interfaz
//! como si fuera una aplicacion, una red social: cuando entramos a una
//! tarjeta, como Instagram, aparece el texto en el medio». Asi que:
//!
//! - con foto, la foto **llena** la ventana (recortada al centro) bajo un
//!   velo que la oscurece arriba y abajo; sin foto, el degradado estable del
//!   momento (`pixpin_timeline::estilo::degradado_de`);
//! - el titulo grande y la descripcion, **centrados en el medio**, con
//!   sombra para que se lean sobre cualquier foto, y los emoticonos en
//!   color;
//! - barritas arriba con varias fotos, y clic en el tercio izquierdo o
//!   derecho (o ←/→) para pasar; arriba/abajo o la rueda, de momento;
//! - abajo la chapita de estado (el emoticono, o la gravedad de una leccion
//!   que da la vuelta con un clic, y «Me volvio a pasar +1») y la nota de
//!   voz en grande.
//!
//! Lo mismo, sin botones, es la **tarjeta que se comparte** (`tarjeta`):
//! 1080 x 1350, como un post.
//!
//! Donde va cada cosa lo dice `disposicion::historia`, para pintar y para
//! el clic.

use super::compartir::{Contenido, contenido_del_detalle};
use super::*;
use pixpin_timeline::emoticonos::trozos;
use pixpin_timeline::estilo;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

fn de_hex(c: u32) -> Color {
    hex(c)
}

/// El ancho de un trozo **con** sus blancos de los bordes (DirectWrite no
/// cuenta los del final).
fn ancho_de(p: &Pintor, t: &str, tam: f32, negrita: bool) -> f32 {
    let medir = |x: &str| {
        if negrita {
            ui::medir_negrita(p, x, tam, 100_000.0).0
        } else {
            p.medir_texto(x, tam).0
        }
    };
    medir(&format!("{t}.")) - medir(".")
}

/// El ancho de un renglon con emoticonos.
pub(super) fn medir_trozos(p: &Pintor, t: &str, tam: f32, negrita: bool) -> f32 {
    trozos(t)
        .into_iter()
        .map(|(x, emo)| if emo { p.medir_texto(x, tam).0 } else { ancho_de(p, x, tam, negrita) })
        .sum()
}

/// Pinta un renglon con los emoticonos en color y lo demas con su letra.
/// `sombra` le pone debajo una sombra suave para leerlo sobre una foto.
#[allow(clippy::too_many_arguments)] // pintor, texto, donde, tamano, letra, color y sombra
pub(super) fn pintar_trozos(
    p: &Pintor,
    t: &str,
    x: f32,
    y: f32,
    tam: f32,
    negrita: bool,
    color: Color,
    sombra: bool,
) {
    // El renglon ya viene partido: DirectWrite no debe partirlo otra vez
    // (con el ancho justo, a veces lo hacia por un pixel de diferencia).
    let sin_partir = 100_000.0;
    let mut cx = x;
    for (trozo, emo) in trozos(t) {
        let w = if emo {
            p.medir_texto(trozo, tam).0
        } else {
            ancho_de(p, trozo, tam, negrita)
        };
        if emo {
            p.texto_color(trozo, cx, y, tam, color);
        } else {
            if sombra {
                let d = (tam / 16.0).max(1.0);
                for (dx, dy, a) in [(0.0, 2.0 * d, 0.35), (0.0, d, 0.35), (d, d, 0.2)] {
                    let c = Color { a, ..Color::NEGRO };
                    if negrita {
                        ui::negrita(p, trozo, cx + dx, y + dy, tam, sin_partir, c);
                    } else {
                        p.texto(trozo, cx + dx, y + dy, tam, c);
                    }
                }
            }
            if negrita {
                ui::negrita(p, trozo, cx, y, tam, sin_partir, color);
            } else {
                p.texto(trozo, cx, y, tam, color);
            }
        }
        cx += w;
    }
}

/// El fondo de una historia: la foto llenando `caja` con su velo, o el
/// degradado del momento.
fn fondo(p: &Pintor, c: &Contenido, bm: Option<(&ID2D1Bitmap1, u32, u32)>, caja: RectF) {
    if let Some((b, bw, bh)) = bm {
        p.rellenar(caja, Color::NEGRO);
        crate::miniaturas::pintar_recortado(p, b, caja, bw, bh);
        // El velo: oscuro arriba (la fecha), algo en el medio (el texto) y
        // mas abajo (las chapitas). Con un pincel de degradado: sin franjas.
        p.rellenar(caja, Color { a: 0.30, ..Color::NEGRO });
        let arriba = RectF {
            alto: caja.alto * 0.28,
            ..caja
        };
        p.rect_degradado(
            arriba,
            (caja.x, caja.y),
            (caja.x, caja.y + arriba.alto),
            Color { a: 0.55, ..Color::NEGRO },
            Color { a: 0.0, ..Color::NEGRO },
        );
        let abajo = RectF {
            y: caja.y + caja.alto * 0.55,
            alto: caja.alto * 0.45,
            ..caja
        };
        p.rect_degradado(
            abajo,
            (caja.x, abajo.y),
            (caja.x, abajo.y + abajo.alto),
            Color { a: 0.0, ..Color::NEGRO },
            Color { a: 0.7, ..Color::NEGRO },
        );
    } else {
        let (a, b) = estilo::degradado_de(&c.semilla);
        p.rect_degradado(
            caja,
            (caja.x, caja.y),
            (caja.x + caja.ancho, caja.y + caja.alto),
            de_hex(a),
            de_hex(b),
        );
        // Un poco de sombra abajo, para las chapitas.
        let abajo = RectF {
            y: caja.y + caja.alto * 0.7,
            alto: caja.alto * 0.3,
            ..caja
        };
        p.rect_degradado(
            abajo,
            (caja.x, abajo.y),
            (caja.x, abajo.y + abajo.alto),
            Color { a: 0.0, ..Color::NEGRO },
            Color { a: 0.3, ..Color::NEGRO },
        );
    }
}

/// El titulo grande y la descripcion, centrados en `caja`.
fn texto_central(p: &Pintor, c: &Contenido, caja: RectF, tam_t: f32, tam_d: f32) {
    let lh_t = tam_t * 1.28;
    let lh_d = tam_d * 1.5;
    let titulo = dis::cortar_renglones(
        dis::renglones(&c.titulo, caja.ancho, &|t| medir_trozos(p, t, tam_t, true)),
        4,
    );
    let hueco = tam_t * 0.6;
    let sobra = caja.alto - titulo.len() as f32 * lh_t - hueco;
    let max_d = (sobra / lh_d).floor().max(0.0) as usize;
    let desc = if c.texto.trim().is_empty() {
        Vec::new()
    } else {
        dis::cortar_renglones(
            dis::renglones(c.texto.trim(), caja.ancho, &|t| medir_trozos(p, t, tam_d, false)),
            max_d.min(9),
        )
    };
    let total = titulo.len() as f32 * lh_t
        + if desc.is_empty() {
            0.0
        } else {
            hueco + desc.len() as f32 * lh_d
        };
    let mut y = caja.y + (caja.alto - total) / 2.0;
    let cx = caja.x + caja.ancho / 2.0;
    for l in &titulo {
        let w = medir_trozos(p, l, tam_t, true);
        pintar_trozos(p, l, cx - w / 2.0, y, tam_t, true, Color::BLANCO, true);
        y += lh_t;
    }
    if !desc.is_empty() {
        y += hueco;
        for l in &desc {
            let w = medir_trozos(p, l, tam_d, false);
            pintar_trozos(p, l, cx - w / 2.0, y + (lh_d - tam_d * 1.3) / 2.0, tam_d, false, blanco(0.92), true);
            y += lh_d;
        }
    }
}

/// Una pildora translucida con texto (y emoticonos en color), centrada en
/// `cx`. Devuelve su caja.
#[allow(clippy::too_many_arguments)] // pintor, texto, donde, tamano, tinta, fondo y escala
fn pildora(p: &Pintor, t: &str, x: f32, cy: f32, tam: f32, tinta: Color, fondo: Color, s: f32) -> RectF {
    let w = medir_trozos(p, t, tam, true);
    let alto = tam * 2.1;
    let r = RectF {
        x,
        y: cy - alto / 2.0,
        ancho: w + 2.0 * 14.0 * s,
        alto,
    };
    p.rellenar_redondeado(r, alto / 2.0, fondo);
    pintar_trozos(p, t, r.x + 14.0 * s, cy - tam * 0.66, tam, true, tinta, false);
    r
}

/// Las chapitas de abajo, de una fila centrada en `cx`: el estado (los
/// emoticonos, o «💡 Leccion») o la gravedad con «↻ +1» y «3×». Devuelve
/// cada una con su accion (si la tiene).
fn chapas(c: &Contenido, textos: &Catalogo) -> Vec<(String, Color, Color, Option<Accion>)> {
    let mut v = Vec::new();
    let oscuro = Color { a: 0.45, ..Color::NEGRO };
    if let Some(g) = c.gravedad {
        let gr = super::super::tarjetas::gravedad(g);
        v.push((
            format!("{} {}", gr.emoticono, textos.t(gr.clave)),
            Color::BLANCO,
            Color { a: 0.85, ..gr.color },
            Some(Accion::GravedadDetalle),
        ));
        v.push((textos.t("timeline-otra-vez-corto"), Color::BLANCO, oscuro, Some(Accion::OtraVezDetalle)));
        if c.veces > 1 {
            v.push((format!("{}×", c.veces), Color::BLANCO, oscuro, None));
        }
    } else {
        if let Some(e) = &c.emoticono {
            v.push((e.clone(), Color::BLANCO, oscuro, None));
        }
        if let Some(m) = c.marca {
            v.push((
                format!("{m} {}", textos.t("timeline-leccion")),
                Color::BLANCO,
                oscuro,
                None,
            ));
        }
    }
    v
}

/// La nota de voz grande: «▶ Nota de voz · 0:42».
#[allow(clippy::too_many_arguments)] // pintor, caja, duracion, raton, sonando, escala y textos
fn voz_grande(p: &Pintor, r: RectF, dur: i64, encima: bool, s: f32, textos: &Catalogo) {
    p.rellenar_redondeado(r, r.alto / 2.0, if encima { blanco(0.32) } else { blanco(0.2) });
    let c = (r.x + r.alto / 2.0, r.y + r.alto / 2.0);
    p.circulo(c, r.alto / 2.0 - 6.0 * s, Color::BLANCO);
    p.icono(
        &mi::PLAY_ARROW,
        RectF {
            x: c.0 - 13.0 * s,
            y: c.1 - 13.0 * s,
            ancho: 26.0 * s,
            alto: 26.0 * s,
        },
        hex(0x111111),
    );
    let mut t = textos.t("timeline-nota-de-voz");
    let d = estilo::duracion(dur);
    if !d.is_empty() {
        t.push_str(" · ");
        t.push_str(&d);
    }
    let tam = 16.0 * s;
    let (_, th) = ui::medir_negrita(p, &t, tam, r.ancho);
    ui::negrita(p, &t, r.x + r.alto + 8.0 * s, r.y + (r.alto - th) / 2.0, tam, r.ancho, Color::BLANCO);
}

/// La historia, encima de toda la ventana.
pub(super) fn pintar(e: &mut Estado, p: &Pintor, w: f32, h: f32, s: f32, textos: &Catalogo) {
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: h,
    };
    p.rellenar(todo, Color::NEGRO);
    // Lo de debajo no responde mientras la historia esta abierta.
    e.botones.zona(todo, Accion::Fondo);
    let Some(c) = contenido_del_detalle(e, textos) else {
        // Lo que ensenaba ya no esta (se borro, o se releyo sin el).
        e.detalle = None;
        return;
    };
    let (pos, total, k) = e
        .detalle
        .as_ref()
        .map_or((0, 0, 0), |d| (d.pos, d.lista.len(), d.foto));
    let k = k.min(c.fotos.len().saturating_sub(1));
    let d = dis::historia(w, h, s, c.fotos.len(), c.audio.is_some());
    let raton = e.botones.raton;

    let bm = c.fotos.get(k).and_then(|r| e.grande.ya(r).or_else(|| e.minis.ya(r)));
    fondo(p, &c, bm, todo);
    if !c.fotos.is_empty() && bm.is_none() {
        p.icono(
            &mi::IMAGE,
            RectF {
                x: w / 2.0 - 24.0 * s,
                y: h / 2.0 - 24.0 * s,
                ancho: 48.0 * s,
                alto: 48.0 * s,
            },
            GRIS,
        );
    }
    // Pasar: los tercios, antes que todo lo demas (que les gana).
    e.botones.zona(d.anterior, Accion::FotoAnterior);
    e.botones.zona(d.siguiente, Accion::FotoSiguiente);
    let tam_t = if d.texto.ancho >= 520.0 * s { 32.0 * s } else { 26.0 * s };
    texto_central(p, &c, d.texto, tam_t, 17.0 * s);

    // Arriba: mover la ventana, las barritas, la fecha y los botones.
    e.botones.zona(
        RectF {
            alto: dis::ARRIBA_DETALLE * s,
            ..todo
        },
        Accion::Mover,
    );
    for (n, r) in d.barras.iter().enumerate() {
        p.rellenar_redondeado(*r, r.alto / 2.0, if n == k { Color::BLANCO } else { blanco(0.35) });
        e.botones.zona(
            RectF {
                y: 0.0,
                alto: 20.0 * s,
                ..*r
            },
            Accion::IrAFoto(n),
        );
    }
    let fecha = estilo::fecha_larga(c.cuando, e.desfase, e.ingles);
    let (fw, fh) = p.medir_texto(&fecha, 15.0 * s);
    let fy = d.cerrar.y + (d.cerrar.alto - fh) / 2.0;
    pintar_trozos(p, &fecha, 20.0 * s, fy, 15.0 * s, false, Color::BLANCO, true);
    if total > 1 {
        p.texto(
            &format!("{} / {}", pos + 1, total),
            20.0 * s + fw + 14.0 * s,
            fy + 1.0 * s,
            13.5 * s,
            blanco(0.65),
        );
    }
    boton_redondo(p, &mut e.botones, d.cerrar, Accion::CerrarDetalle, &mi::CLOSE, None, raton);
    if c.gravedad.is_none() {
        boton_redondo(
            p,
            &mut e.botones,
            d.leccion,
            Accion::LeccionDetalle,
            &mi::LIGHTBULB,
            c.marca.map(|_| ui::v2::AMARILLO),
            raton,
        );
    }
    if let Some(r) = d.pinear {
        boton_redondo(p, &mut e.botones, r, Accion::PinearDetalle, &mi::PUSH_PIN, None, raton);
    }
    boton_redondo(p, &mut e.botones, d.compartir, Accion::CompartirDetalle, &mi::IOS_SHARE, None, raton);

    // Abajo: las chapitas, centradas, y la nota de voz.
    let lista = chapas(&c, textos);
    if !lista.is_empty() {
        let tam = 15.0 * s;
        let anchos: Vec<f32> = lista
            .iter()
            .map(|(t, ..)| medir_trozos(p, t, tam, true) + 28.0 * s)
            .collect();
        let todo_w: f32 = anchos.iter().sum::<f32>() + (anchos.len() - 1) as f32 * 8.0 * s;
        let mut x = (w - todo_w) / 2.0;
        for ((t, tinta, fondo_c, a), aw) in lista.into_iter().zip(anchos) {
            let fondo_c = match a {
                Some(_) if dentro(RectF { x, y: d.chapas_y - tam, ancho: aw, alto: 2.0 * tam }, raton) => {
                    ui::aclarar(fondo_c, 0.1)
                }
                _ => fondo_c,
            };
            let r = pildora(p, &t, x, d.chapas_y, tam, tinta, fondo_c, s);
            if let Some(a) = a {
                e.botones.zona(r, a);
            }
            x += aw + 8.0 * s;
        }
    }
    if let (Some(r), Some((_, dur))) = (d.voz, &c.audio) {
        voz_grande(p, r, *dur, dentro(r, raton), s, textos);
        e.botones.zona(r, Accion::OirDetalle);
    }
}

/// Un boton redondo sobre la foto: fondo oscuro translucido para que se vea
/// sobre cualquier imagen. `tinta` lo enciende (la bombilla de un momento
/// que ya es leccion).
fn boton_redondo(
    p: &Pintor,
    botones: &mut Botones<Accion>,
    r: RectF,
    a: Accion,
    icono: &pixpin_render::icono::Icono,
    tinta: Option<Color>,
    raton: (f32, f32),
) {
    let c = (r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
    let fondo = if dentro(r, raton) {
        blanco(0.25)
    } else {
        Color {
            a: 0.4,
            ..Color::NEGRO
        }
    };
    p.circulo(c, r.ancho / 2.0, fondo);
    if let Some(t) = tinta {
        p.circulo(c, r.ancho / 2.0, Color { a: 0.22, ..t });
    }
    p.icono(icono, encoger(r, r.ancho * 0.24), tinta.unwrap_or(TEXTO_V));
    botones.zona(r, a);
}

/// **La tarjeta que se comparte**: la historia sin botones, en `w` x `h`
/// (1080 x 1350, como un post): fondo, la fecha arriba, el texto en el
/// medio, abajo el estado y la nota de voz, y una firma discreta.
#[allow(clippy::too_many_arguments)] // pintor, contenido, foto, tamano, textos, huso e idioma
pub(super) fn tarjeta(
    p: &Pintor,
    c: &Contenido,
    bm: Option<(&ID2D1Bitmap1, u32, u32)>,
    w: f32,
    h: f32,
    textos: &Catalogo,
    desfase: i64,
    ingles: bool,
) {
    let s = w / 432.0;
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: h,
    };
    fondo(p, c, bm, todo);
    let fecha = estilo::fecha_larga(c.cuando, desfase, ingles);
    pintar_trozos(p, &fecha, 28.0 * s, 26.0 * s, 13.0 * s, false, blanco(0.9), true);
    if c.fotos.len() > 1 {
        // Cuantas fotos son, como las barritas de la historia.
        let n = c.fotos.len().min(10);
        let (x0, x1, hueco) = (28.0 * s, w - 28.0 * s, 3.0 * s);
        let ancho = (x1 - x0 - (n - 1) as f32 * hueco) / n as f32;
        for k in 0..n {
            p.rellenar_redondeado(
                RectF {
                    x: x0 + k as f32 * (ancho + hueco),
                    y: 14.0 * s,
                    ancho,
                    alto: 2.0 * s,
                },
                1.0 * s,
                if k == 0 { Color::BLANCO } else { blanco(0.4) },
            );
        }
    }
    let caja = RectF {
        x: 36.0 * s,
        y: 70.0 * s,
        ancho: w - 72.0 * s,
        alto: h - 70.0 * s - 150.0 * s,
    };
    texto_central(p, c, caja, 28.0 * s, 15.0 * s);
    // Abajo: las chapitas (sin las que solo valen en la ventana) y la voz.
    let lista: Vec<_> = chapas(c, textos)
        .into_iter()
        .filter(|(_, _, _, a)| !matches!(a, Some(Accion::OtraVezDetalle)))
        .collect();
    let mut y = h - 52.0 * s;
    if let Some((_, dur)) = &c.audio {
        let mut t = format!("🎵 {}", textos.t("timeline-nota-de-voz"));
        let d = estilo::duracion(*dur);
        if !d.is_empty() {
            t.push_str(" · ");
            t.push_str(&d);
        }
        let tam = 13.0 * s;
        let tw = medir_trozos(p, &t, tam, true) + 28.0 * s;
        pildora(p, &t, (w - tw) / 2.0, y - 26.0 * s, tam, Color::BLANCO, blanco(0.2), s);
        y -= 46.0 * s;
    }
    if !lista.is_empty() {
        let tam = 13.0 * s;
        let anchos: Vec<f32> = lista
            .iter()
            .map(|(t, ..)| medir_trozos(p, t, tam, true) + 28.0 * s)
            .collect();
        let todo_w: f32 = anchos.iter().sum::<f32>() + (anchos.len() - 1) as f32 * 6.0 * s;
        let mut x = (w - todo_w) / 2.0;
        for ((t, tinta, fondo_c, _), aw) in lista.into_iter().zip(anchos) {
            pildora(p, &t, x, y - 26.0 * s, tam, tinta, fondo_c, s);
            x += aw + 6.0 * s;
        }
    }
    // La firma.
    let firma = textos.t("timeline-compartido-pie");
    let tam = 10.5 * s;
    let (fw, _) = p.medir_texto(&firma, tam);
    p.texto(&firma, (w - fw) / 2.0, h - 22.0 * s, tam, blanco(0.55));
}
