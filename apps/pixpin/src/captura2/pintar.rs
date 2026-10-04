//! **El dibujo de la captura v2**: barra de modos, pistas, lupa, etiqueta de
//! la ventana, barras de despues y selector de proyecto.
//!
//! Los colores son los de la maqueta (paneles oscuros sobre la pantalla
//! atenuada): el overlay se pinta encima de una foto de cualquier color, y
//! es oscuro en los dos temas de la app, como lo era ya.
//!
//! Todo llega en pixeles del escritorio virtual (de `disposicion`) y se
//! pinta en los de la ventana del monitor: `Local` hace la resta.

use pixpin_geom::{Punto, Rect};
use pixpin_render::icono::{Icono, Pintura, TrazoIcono};
use pixpin_render::iconos_excalidraw as ix;
use pixpin_render::letras::Letra;
use pixpin_render::{Color, Pintor, RectF};

use super::disposicion::{
    Accion, BarraAcciones, BarraAnotar, BarraModos, COLORES, EnAnotar, EnSelector,
    Modo, PanelLupa, Proporcion, Selector, Util,
};
use super::{Campo, EstadoSelector, Sesion, Textos};

const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a,
    }
}

pub const PANEL: Color = rgba(28, 28, 30, 0.95);
const BORDE: Color = rgba(255, 255, 255, 0.10);
const SOMBRA: Color = rgba(0, 0, 0, 0.28);
pub const TEXTO: Color = rgba(0xF5, 0xF5, 0xF7, 1.0);
const TEXTO2: Color = rgba(0xE5, 0xE5, 0xEA, 1.0);
pub const GRIS: Color = rgba(0x98, 0x98, 0x9D, 1.0);
const APAGADO: Color = rgba(0x63, 0x63, 0x66, 1.0);
const ENCIMA: Color = rgba(255, 255, 255, 0.08);
const ENCIMA_FUERTE: Color = rgba(255, 255, 255, 0.14);
const ELEGIDO: Color = rgba(10, 132, 255, 0.22);
pub const AZUL: Color = rgba(0x0A, 0x84, 0xFF, 1.0);
const PRIMARIO: Color = rgba(0x00, 0x60, 0xDF, 1.0);
const PRIMARIO_ENCIMA: Color = rgba(0x1A, 0x72, 0xEA, 1.0);
const CAMPO_FONDO: Color = rgba(0x2C, 0x2C, 0x2E, 1.0);
const CHAPA_FONDO: Color = rgba(255, 255, 255, 0.12);
const CHAPA_BORDE: Color = rgba(255, 255, 255, 0.14);
const ROJO: Color = rgba(0xFF, 0x45, 0x3A, 1.0);

const SEMI: Letra<'static> = Letra {
    familia: "Segoe UI Semibold",
    negrita: false,
    cursiva: false,
    interlineado: None,
};

/// De pixeles del escritorio virtual a los de la ventana de un monitor.
#[derive(Debug, Clone, Copy)]
pub struct Local {
    pub ox: i32,
    pub oy: i32,
    /// Escala del monitor, 1.0 = 100 %.
    pub e: f32,
}

impl Local {
    pub fn r(&self, r: Rect) -> RectF {
        RectF {
            x: (r.x - self.ox) as f32,
            y: (r.y - self.oy) as f32,
            ancho: r.ancho as f32,
            alto: r.alto as f32,
        }
    }
    pub fn x(&self, x: i32) -> f32 {
        (x - self.ox) as f32
    }
    pub fn y(&self, y: i32) -> f32 {
        (y - self.oy) as f32
    }
}

fn encoger(r: RectF, d: f32) -> RectF {
    RectF {
        x: r.x + d,
        y: r.y + d,
        ancho: (r.ancho - 2.0 * d).max(0.0),
        alto: (r.alto - 2.0 * d).max(0.0),
    }
}

/// Un panel de la maqueta: sombra suave, borde finito y fondo oscuro.
pub fn panel(p: &Pintor, r: RectF, radio: f32, e: f32) {
    let sombra = RectF {
        x: r.x - 2.0 * e,
        y: r.y + 4.0 * e,
        ancho: r.ancho + 4.0 * e,
        alto: r.alto + 6.0 * e,
    };
    p.rellenar_redondeado(sombra, radio + 2.0 * e, SOMBRA);
    p.rellenar_redondeado(r, radio, BORDE);
    p.rellenar_redondeado(encoger(r, 1.0 * e), (radio - e).max(0.0), PANEL);
}

/// Un rectangulo redondeado con borde de un color y fondo de otro.
fn caja(p: &Pintor, r: RectF, radio: f32, fondo: Color, borde: Color, grosor: f32) {
    p.rellenar_redondeado(r, radio, borde);
    let dentro = encoger(r, grosor);
    let radio_dentro = (radio - grosor).max(0.0);
    // Un fondo translucido sobre el borde se veria del color del borde:
    // debajo va el del panel, y encima el translucido.
    if fondo.a < 1.0 {
        p.rellenar_redondeado(dentro, radio_dentro, Color { a: 1.0, ..PANEL });
    }
    p.rellenar_redondeado(dentro, radio_dentro, fondo);
}

/// La chapita de un atajo («Ctrl P», «Esc»...), con su centro vertical en
/// `cy`, empezando en `x`. Devuelve lo que mide de ancho.
pub fn chapa(p: &Pintor, texto: &str, x: f32, cy: f32, e: f32, sobre_azul: bool) -> f32 {
    let tam = 11.0 * e;
    let (tw, th) = p.medir_con_letra(texto, tam, 1.0e6, &SEMI);
    let alto = 20.0 * e;
    let ancho = (tw + 10.0 * e).max(20.0 * e);
    let r = RectF {
        x,
        y: cy - alto / 2.0,
        ancho,
        alto,
    };
    let (fondo, borde, color) = if sobre_azul {
        (rgba(255, 255, 255, 0.22), rgba(255, 255, 255, 0.30), Color::BLANCO)
    } else {
        (CHAPA_FONDO, CHAPA_BORDE, TEXTO2)
    };
    caja(p, r, 5.0 * e, fondo, borde, 1.0 * e);
    p.texto_con_letra(
        texto,
        r.x + (ancho - tw) / 2.0,
        cy - th / 2.0,
        tam,
        1.0e6,
        &SEMI,
        color,
    );
    ancho
}

/// Lo que mide una chapita, sin pintarla.
pub fn ancho_chapa(p: &Pintor, texto: &str, e: f32) -> f32 {
    let (tw, _) = p.medir_con_letra(texto, 11.0 * e, 1.0e6, &SEMI);
    (tw + 10.0 * e).max(20.0 * e)
}

fn texto_centrado_v(p: &Pintor, t: &str, x: f32, cy: f32, tam: f32, color: Color) -> f32 {
    let (w, h) = p.medir_texto(t, tam);
    p.texto(t, x, cy - h / 2.0, tam, color);
    w
}

/// Recorta un texto con «…» para que quepa en `max`.
fn caber(p: &Pintor, t: &str, tam: f32, max: f32) -> String {
    if p.medir_texto(t, tam).0 <= max {
        return t.to_string();
    }
    let mut s: Vec<char> = t.chars().collect();
    while !s.is_empty() {
        s.pop();
        let prueba: String = s.iter().collect::<String>() + "…";
        if p.medir_texto(&prueba, tam).0 <= max {
            return prueba;
        }
    }
    "…".to_string()
}

const fn trazo(d: &'static str, grosor: f32) -> TrazoIcono {
    TrazoIcono {
        d,
        relleno: Pintura::Nada,
        trazo: Pintura::Actual,
        grosor,
        extremo_redondo: true,
        union_redonda: true,
        opacidad: 1.0,
        par_impar: false,
        matriz: None,
        mascara: None,
    }
}

/// Las esquinas de una seleccion: lo que es «elegir una zona».
const ICONO_ZONA: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[trazo(
        "M3 9V5h4M17 5h4v4M21 15v4h-4M7 19H3v-4M10 5h4M10 19h4M3 11v2M21 11v2",
        2.0,
    )],
};
const ICONO_VENTANA: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[trazo("M5 5h14a2 2 0 0 1 2 2v10a2 2 0 0 1 -2 2h-14a2 2 0 0 1 -2 -2v-10a2 2 0 0 1 2 -2zM3 9h18", 2.0)],
};
const ICONO_PANTALLA: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[trazo("M4 4h16a2 2 0 0 1 2 2v10a2 2 0 0 1 -2 2h-16a2 2 0 0 1 -2 -2v-10a2 2 0 0 1 2 -2zM8 21h8M12 18v3", 2.0)],
};
const ICONO_SCROLL: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[trazo("M8 2h8a2 2 0 0 1 2 2v16a2 2 0 0 1 -2 2h-8a2 2 0 0 1 -2 -2v-16a2 2 0 0 1 2 -2zM12 7v10M9 14l3 3l3 -3", 2.0)],
};
const ICONO_GIF: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[trazo("M4 12a8 8 0 1 0 16 0a8 8 0 1 0 -16 0", 2.0)],
};
const ICONO_TEXTO: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[trazo("M4 7V5h16v2M12 5v14M9 19h6", 2.0)],
};
const ICONO_REPETIR: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[trazo("M3 12a9 9 0 1 0 3 -6.7L3 8M3 3v5h5", 2.0)],
};
const ICONO_PIN: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[trazo("M12 17v5M9 3h6l-1 6l3 3v2h-10v-2l3 -3z", 2.0)],
};
const ICONO_CHAT: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[trazo("M21 12a8 8 0 0 1 -11.6 7.1L4 20l1 -4.6A8 8 0 1 1 21 12z", 2.0)],
};
const ICONO_COPIAR: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[trazo("M10 8h9a2 2 0 0 1 2 2v9a2 2 0 0 1 -2 2h-9a2 2 0 0 1 -2 -2v-9a2 2 0 0 1 2 -2zM16 8V5a2 2 0 0 0 -2 -2H5a2 2 0 0 0 -2 2v9a2 2 0 0 0 2 2h3", 2.0)],
};
const ICONO_GUARDAR: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[trazo("M12 3v12M7 10l5 5l5 -5M5 21h14", 2.0)],
};
const ICONO_FLECHITA: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[trazo("M9 6l6 6l-6 6", 2.5)],
};
const ICONO_CERRAR: Icono = Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[trazo("M6 6l12 12M18 6l-12 12", 2.0)],
};

fn icono_modo(m: Modo) -> &'static Icono {
    match m {
        Modo::Zona => &ICONO_ZONA,
        Modo::Ventana => &ICONO_VENTANA,
        Modo::Pantalla => &ICONO_PANTALLA,
        Modo::Scroll => &ICONO_SCROLL,
        Modo::Gif => &ICONO_GIF,
        Modo::PinEnVivo => &ix::EYE_ICON,
        Modo::Texto => &ICONO_TEXTO,
    }
}

pub fn icono_util(u: Util) -> &'static Icono {
    // Los mismos que la caja de dibujo (`caja_dibujo::icono`): la misma
    // herramienta se ve igual en todas partes.
    match u {
        Util::Lapiz => &ix::FREEDRAW_ICON,
        Util::Flecha => &ix::ARROW_ICON,
        Util::Rectangulo => &ix::RECTANGLE_ICON,
        Util::Texto => &ix::TEXT_ICON,
        Util::Mosaico => &ix::EYE_CLOSED_ICON,
    }
}

fn icono_accion(a: Accion) -> Option<&'static Icono> {
    match a {
        Accion::Copiar => Some(&ICONO_COPIAR),
        Accion::Pinear => Some(&ICONO_PIN),
        Accion::AlChat => Some(&ICONO_CHAT),
        Accion::Texto => Some(&ICONO_TEXTO),
        Accion::Guardar => Some(&ICONO_GUARDAR),
        Accion::Descartar => None,
    }
}

fn cuadro_icono(cx: f32, cy: f32, lado: f32) -> RectF {
    RectF {
        x: cx - lado / 2.0,
        y: cy - lado / 2.0,
        ancho: lado,
        alto: lado,
    }
}

/// La barra de modos de abajo.
pub fn pintar_modos(
    p: &Pintor,
    l: Local,
    b: &BarraModos,
    s: &Sesion,
    t: &Textos,
    raton: Punto,
    medidas: (u32, u32),
) {
    let e = l.e;
    panel(p, l.r(b.panel), 16.0 * e, e);
    for (m, r) in &b.modos {
        let rf = l.r(*r);
        let elegido = s.modo == *m;
        let encima = r.contiene(raton);
        if elegido {
            caja(p, rf, 12.0 * e, ELEGIDO, AZUL, 1.0 * e);
        } else if encima {
            p.rellenar_redondeado(rf, 12.0 * e, ENCIMA);
        }
        let color = if elegido { Color::BLANCO } else { TEXTO2 };
        let cx = rf.x + rf.ancho / 2.0;
        let icono = cuadro_icono(cx, rf.y + 28.0 * e, 24.0 * e);
        p.icono(icono_modo(*m), icono, color);
        if *m == Modo::Gif {
            p.circulo((cx, rf.y + 28.0 * e), 3.0 * e, ROJO);
        }
        let rotulo = caber(p, t.modo(*m), 13.0 * e, rf.ancho - 8.0 * e);
        let (tw, _) = p.medir_texto(&rotulo, 13.0 * e);
        p.texto(&rotulo, cx - tw / 2.0, rf.y + 46.0 * e, 13.0 * e, color);
        // La letra, arriba a la derecha.
        let letra = m.letra().to_string();
        let w = ancho_chapa(p, &letra, e);
        let fondo_azul = elegido;
        if fondo_azul {
            let rr = RectF {
                x: rf.x + rf.ancho - 6.0 * e - w,
                y: rf.y + 6.0 * e,
                ancho: w,
                alto: 20.0 * e,
            };
            p.rellenar_redondeado(rr, 5.0 * e, AZUL);
            let (lw, lh) = p.medir_con_letra(&letra, 11.0 * e, 1.0e6, &SEMI);
            p.texto_con_letra(
                &letra,
                rr.x + (w - lw) / 2.0,
                rr.y + (rr.alto - lh) / 2.0,
                11.0 * e,
                1.0e6,
                &SEMI,
                Color::BLANCO,
            );
        } else {
            chapa(p, &letra, rf.x + rf.ancho - 6.0 * e - w, rf.y + 16.0 * e, e, false);
        }
    }
    // La raya entre los de capturar y los de hacer otra cosa.
    let sx = l.x(b.separador_x);
    let (y0, y1) = (
        l.y(b.modos[0].1.y) + 6.0 * e,
        l.y(b.modos[0].1.abajo()) - 6.0 * e,
    );
    p.linea((sx, y0), (sx, y1), 1.0 * e, BORDE);
    // La linea entre filas.
    let ly = l.y(b.linea_y);
    p.linea(
        (l.x(b.panel.x) + 8.0 * e, ly),
        (l.x(b.panel.derecha()) - 8.0 * e, ly),
        1.0 * e,
        rgba(255, 255, 255, 0.08),
    );

    // Los campos de medida.
    for (cual, r, rotulo, cifras, valor) in [
        (Campo::Ancho, b.ancho, &t.ancho, &s.ancho, medidas.0),
        (Campo::Alto, b.alto, &t.alto, &s.alto, medidas.1),
    ] {
        let rf = l.r(r);
        let foco = s.campo == Some(cual);
        let borde = if foco { AZUL } else { BORDE };
        caja(p, rf, 10.0 * e, CAMPO_FONDO, borde, 1.0 * e);
        let cy = rf.y + rf.alto / 2.0;
        let mut x = rf.x + 10.0 * e;
        x += texto_centrado_v(p, rotulo, x, cy, 12.0 * e, GRIS) + 6.0 * e;
        let numero = if foco {
            cifras.0.clone()
        } else if valor > 0 {
            valor.to_string()
        } else {
            "—".to_string()
        };
        let w = texto_centrado_v(p, &numero, x, cy, 15.0 * e, TEXTO);
        if foco {
            // El cursor de escribir.
            let cx = x + w + 1.0 * e;
            p.linea((cx, cy - 9.0 * e), (cx, cy + 9.0 * e), 1.5 * e, AZUL);
        }
        let (pw, _) = p.medir_texto("px", 12.0 * e);
        texto_centrado_v(p, "px", rf.x + rf.ancho - 10.0 * e - pw, cy, 12.0 * e, GRIS);
    }
    let por = l.r(b.por);
    let (xw, _) = p.medir_texto("×", 15.0 * e);
    texto_centrado_v(
        p,
        "×",
        por.x + (por.ancho - xw) / 2.0,
        por.y + por.alto / 2.0,
        15.0 * e,
        GRIS,
    );

    // Las proporciones.
    for (pr, r) in &b.chips {
        let rf = l.r(*r);
        let on = s.proporcion == *pr;
        let encima = r.contiene(raton);
        let fondo = if encima && !on { rgba(0x3A, 0x3A, 0x3C, 1.0) } else { CAMPO_FONDO };
        if on {
            caja(p, rf, 10.0 * e, ELEGIDO, AZUL, 1.0 * e);
        } else {
            caja(p, rf, 10.0 * e, fondo, BORDE, 1.0 * e);
        }
        let rotulo = if *pr == Proporcion::Libre { t.libre.as_str() } else { pr.rotulo() };
        let (tw, th) = p.medir_texto(rotulo, 13.0 * e);
        p.texto(
            rotulo,
            rf.x + (rf.ancho - tw) / 2.0,
            rf.y + (rf.alto - th) / 2.0,
            13.0 * e,
            if on { Color::BLANCO } else { TEXTO2 },
        );
    }

    // Repetir la ultima zona.
    if let Some(r) = b.repetir {
        let rf = l.r(r);
        let sep_x = rf.x - 6.0 * e;
        p.linea((sep_x, rf.y + 4.0 * e), (sep_x, rf.y + rf.alto - 4.0 * e), 1.0 * e, BORDE);
        let fondo = if r.contiene(raton) { rgba(0x3A, 0x3A, 0x3C, 1.0) } else { CAMPO_FONDO };
        caja(p, rf, 10.0 * e, fondo, BORDE, 1.0 * e);
        let cy = rf.y + rf.alto / 2.0;
        p.icono(&ICONO_REPETIR, cuadro_icono(rf.x + 22.0 * e, cy, 18.0 * e), TEXTO);
        let w_chapa = ancho_chapa(p, "R", e);
        let max = rf.ancho - 44.0 * e - w_chapa - 14.0 * e;
        let rotulo = caber(p, &t.repetir, 13.0 * e, max);
        texto_centrado_v(p, &rotulo, rf.x + 38.0 * e, cy, 13.0 * e, TEXTO);
        chapa(p, "R", rf.x + rf.ancho - 10.0 * e - w_chapa, cy, e, false);
    }
}

/// La fila de los gestos con Alt, arriba.
pub fn pintar_pistas(p: &Pintor, l: Local, r: Rect, t: &Textos) {
    let e = l.e;
    let rf = l.r(r);
    panel(p, rf, 12.0 * e, e);
    let cy = rf.y + rf.alto / 2.0;
    let mut x = rf.x + 16.0 * e;
    x += texto_centrado_v(p, &t.sin_abrir, x, cy, 12.0 * e, GRIS) + 22.0 * e;
    let limite = rf.x + rf.ancho - 120.0 * e;
    for (gesto, que) in &t.gestos {
        let w_alt = ancho_chapa(p, "Alt", e);
        let w_g = p.medir_texto(gesto, 13.0 * e).0;
        let w_q = p.medir_texto(que, 13.0 * e).0;
        if x + w_alt + w_g + w_q + 16.0 * e > limite {
            break;
        }
        x += chapa(p, "Alt", x, cy, e, false) + 6.0 * e;
        x += texto_centrado_v(p, gesto, x, cy, 13.0 * e, TEXTO2) + 8.0 * e;
        x += texto_centrado_v(p, que, x, cy, 13.0 * e, GRIS) + 22.0 * e;
    }
    // «Esc salir», a la derecha.
    let w_s = p.medir_texto(&t.salir, 13.0 * e).0;
    let w_esc = ancho_chapa(p, "Esc", e);
    let xd = rf.x + rf.ancho - 16.0 * e - w_s - 8.0 * e - w_esc;
    let xd = xd + chapa(p, "Esc", xd, cy, e, false) + 8.0 * e;
    texto_centrado_v(p, &t.salir, xd, cy, 13.0 * e, GRIS);
}

/// La lupa con el codigo de color. `fondo` y `fuente` son el bitmap de la
/// pantalla y el trozo que se amplia, ya en coordenadas de la ventana.
#[allow(clippy::too_many_arguments)]
pub fn pintar_lupa(
    p: &Pintor,
    l: Local,
    lupa: &PanelLupa,
    fondo: &windows::Win32::Graphics::Direct2D::ID2D1Bitmap1,
    fuente: RectF,
    cursor: Punto,
    color: [u8; 4],
    s: &Sesion,
    t: &Textos,
) {
    let e = l.e;
    let rf = l.r(lupa.panel);
    panel(p, rf, 14.0 * e, e);
    let img = l.r(lupa.imagen);
    p.con_recorte(img, |p| {
        p.bitmap(fondo, img, Some(fuente), true);
        // La cruz: la fila y la columna del pixel, en azul suave.
        let (fw, fh) = lupa.fuente;
        let cw = img.ancho / fw as f32;
        let ch = img.alto / fh as f32;
        let px = img.x + (cursor.x as f32 - (fuente.x + l.ox as f32)) * cw;
        let py = img.y + (cursor.y as f32 - (fuente.y + l.oy as f32)) * ch;
        let banda = rgba(10, 132, 255, 0.25);
        p.rellenar(
            RectF {
                x: img.x,
                y: py,
                ancho: img.ancho,
                alto: ch,
            },
            banda,
        );
        p.rellenar(
            RectF {
                x: px,
                y: img.y,
                ancho: cw,
                alto: img.alto,
            },
            banda,
        );
        p.trazar(
            RectF {
                x: px,
                y: py,
                ancho: cw,
                alto: ch,
            },
            2.0 * e,
            AZUL,
        );
    });
    let [r, g, b, _] = color;
    let mut y = img.y + img.alto + 10.0 * e;
    let muestra = RectF {
        x: img.x,
        y,
        ancho: 22.0 * e,
        alto: 22.0 * e,
    };
    caja(
        p,
        muestra,
        6.0 * e,
        Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        },
        rgba(255, 255, 255, 0.3),
        1.0 * e,
    );
    let principal = pixpin_ui::texto_color(s.formato, color);
    let cy = y + 11.0 * e;
    let (_, th) = p.medir_con_letra(&principal, 15.0 * e, 1.0e6, &SEMI);
    let max = img.ancho - 30.0 * e - 70.0 * e;
    let principal = caber(p, &principal, 15.0 * e, max);
    p.texto_con_letra(&principal, img.x + 30.0 * e, cy - th / 2.0, 15.0 * e, 1.0e6, &SEMI, TEXTO);
    let coords = format!("{}, {}", cursor.x, cursor.y);
    let (cw, _) = p.medir_texto(&coords, 12.0 * e);
    texto_centrado_v(p, &coords, img.x + img.ancho - cw, cy, 12.0 * e, GRIS);
    y += 26.0 * e;
    // La otra forma de escribirlo, debajo.
    let otra = match s.formato {
        pixpin_ui::FormatoColorLupa::Hex => format!("RGB {r} {g} {b}"),
        _ => format!("#{r:02X}{g:02X}{b:02X}"),
    };
    p.texto(&otra, img.x, y, 12.0 * e, GRIS);
    y += 26.0 * e;
    for (tecla, que) in [("C", &t.copiar_color), ("Shift", &t.cambiar_formato)] {
        let cy = y + 10.0 * e;
        let w = chapa(p, tecla, img.x, cy, e, false);
        texto_centrado_v(p, que, img.x + w + 6.0 * e, cy, 12.0 * e, TEXTO2);
        y += 26.0 * e;
    }
}

/// La etiqueta de lo resaltado: el nombre de la ventana, lo que mide y la
/// pista de que hace el clic.
pub fn pintar_etiqueta(p: &Pintor, l: Local, r: Rect, titulo: &str, medidas: (u32, u32), pista: &str) {
    let e = l.e;
    let rf = l.r(r);
    panel(p, rf, 10.0 * e, e);
    let cy = rf.y + rf.alto / 2.0;
    let mut x = rf.x + 12.0 * e;
    p.icono(&ICONO_VENTANA, cuadro_icono(x + 8.0 * e, cy, 16.0 * e), AZUL);
    x += 26.0 * e;
    let tam = 13.0 * e;
    let m = format!("{} × {}", medidas.0, medidas.1);
    let wm = p.medir_texto(&m, tam).0;
    let wp = p.medir_texto(pista, tam).0;
    let max_t = (rf.ancho - (x - rf.x) - wm - wp - 60.0 * e).max(40.0 * e);
    let nombre = caber(p, titulo, tam, max_t);
    if !nombre.is_empty() {
        x += texto_centrado_v(p, &nombre, x, cy, tam, TEXTO) + 10.0 * e;
    }
    x += texto_centrado_v(p, &m, x, cy, tam, GRIS) + 10.0 * e;
    p.linea((x, cy - 9.0 * e), (x, cy + 9.0 * e), 1.0 * e, BORDE);
    x += 10.0 * e;
    texto_centrado_v(p, pista, x, cy, tam, GRIS);
}

/// Lo que mide de ancho la etiqueta (para colocarla antes de pintarla).
pub fn ancho_etiqueta(p: &Pintor, e: f32, titulo: &str, medidas: (u32, u32), pista: &str) -> u32 {
    let tam = 13.0 * e;
    let t = p.medir_texto(titulo, tam).0.min(420.0 * e);
    let m = p.medir_texto(&format!("{} × {}", medidas.0, medidas.1), tam).0;
    let w = p.medir_texto(pista, tam).0;
    let extra = if titulo.is_empty() { 0.0 } else { 10.0 * e };
    (12.0 * e + 26.0 * e + t + extra + m + 30.0 * e + w + 14.0 * e) as u32
}

/// Las dos barras de despues.
#[allow(clippy::too_many_arguments)]
pub fn pintar_despues(
    p: &Pintor,
    l: Local,
    anotar: &BarraAnotar,
    acciones: &BarraAcciones,
    s: &Sesion,
    t: &Textos,
    raton: Punto,
    medidas: (u32, u32),
) {
    let e = l.e;
    // --- Anotar ---
    panel(p, l.r(anotar.panel), 12.0 * e, e);
    let tam = l.r(anotar.tamano);
    let m = format!("{} × {}", medidas.0, medidas.1);
    let (mw, _) = p.medir_texto(&m, 13.0 * e);
    texto_centrado_v(p, &m, tam.x + (tam.ancho - mw) / 2.0, tam.y + tam.alto / 2.0, 13.0 * e, GRIS);
    let sep = |x: i32| {
        let rf = l.r(anotar.panel);
        let sx = l.x(x);
        p.linea(
            (sx, rf.y + rf.alto / 2.0 - 14.0 * e),
            (sx, rf.y + rf.alto / 2.0 + 14.0 * e),
            1.0 * e,
            rgba(255, 255, 255, 0.12),
        );
    };
    for x in &anotar.separadores {
        sep(*x);
    }
    let encima_de = anotar.en(raton);
    let util = s.anotacion.as_ref().and_then(|a| a.util).or(s.util);
    for (u, r) in &anotar.utiles {
        let rf = l.r(*r);
        let on = util == Some(*u);
        if on {
            caja(p, rf, 10.0 * e, ELEGIDO, AZUL, 1.0 * e);
        } else if encima_de == Some(EnAnotar::Util(*u)) {
            p.rellenar_redondeado(rf, 10.0 * e, ENCIMA);
        }
        let color = if on { Color::BLANCO } else { TEXTO2 };
        p.icono(icono_util(*u), cuadro_icono(rf.x + rf.ancho / 2.0, rf.y + rf.alto / 2.0, 22.0 * e), color);
        let n = u.numero().to_string();
        let (nw, nh) = p.medir_con_letra(&n, 10.0 * e, 1.0e6, &SEMI);
        p.texto_con_letra(
            &n,
            rf.x + rf.ancho - nw - 3.0 * e,
            rf.y + rf.alto - nh - 2.0 * e,
            10.0 * e,
            1.0e6,
            &SEMI,
            if on { rgba(0xBF, 0xDC, 0xFF, 1.0) } else { GRIS },
        );
    }
    for (i, r) in anotar.colores.iter().enumerate() {
        let rf = l.r(*r);
        let [cr, cg, cb] = COLORES[i];
        let centro = (rf.x + rf.ancho / 2.0, rf.y + rf.alto / 2.0);
        let radio = 13.0 * e;
        if s.color == i {
            p.circulo(centro, radio + 4.0 * e, AZUL);
            p.circulo(centro, radio + 2.0 * e, Color::BLANCO);
        } else if encima_de == Some(EnAnotar::Color(i)) {
            p.circulo(centro, radio + 3.0 * e, ENCIMA_FUERTE);
        }
        p.circulo(centro, radio, rgba(cr, cg, cb, 1.0));
        if i == COLORES.len() - 1 {
            p.anillo(centro, radio, 1.0 * e, rgba(255, 255, 255, 0.4));
        }
    }
    for (i, r) in anotar.grosores.iter().enumerate() {
        let rf = l.r(*r);
        let on = s.grosor == i;
        if on {
            caja(p, rf, 10.0 * e, ELEGIDO, AZUL, 1.0 * e);
        } else if encima_de == Some(EnAnotar::Grosor(i)) {
            p.rellenar_redondeado(rf, 10.0 * e, ENCIMA);
        }
        let radio = [3.0, 5.0, 7.5][i] * e;
        p.circulo(
            (rf.x + rf.ancho / 2.0, rf.y + rf.alto / 2.0),
            radio,
            if on { Color::BLANCO } else { TEXTO2 },
        );
    }
    let hay = s.anotacion.as_ref().is_some_and(|a| !a.vacia());
    for (r, icono, activo, que) in [
        (anotar.deshacer, &ix::UNDO_ICON, hay, EnAnotar::Deshacer),
        (anotar.rehacer, &ix::REDO_ICON, s.anotacion.is_some(), EnAnotar::Rehacer),
    ] {
        let rf = l.r(r);
        if activo && encima_de == Some(que) {
            p.rellenar_redondeado(rf, 10.0 * e, ENCIMA);
        }
        p.icono(
            icono,
            cuadro_icono(rf.x + rf.ancho / 2.0, rf.y + rf.alto / 2.0, 22.0 * e),
            if activo { TEXTO2 } else { APAGADO },
        );
    }
    let ch = l.r(anotar.chapa);
    chapa(p, "Ctrl Z", ch.x + 4.0 * e, ch.y + ch.alto / 2.0, e, false);

    // --- Acciones ---
    panel(p, l.r(acciones.panel), 14.0 * e, e);
    let sx = l.x(acciones.separador_x);
    let pr = l.r(acciones.panel);
    p.linea(
        (sx, pr.y + pr.alto / 2.0 - 16.0 * e),
        (sx, pr.y + pr.alto / 2.0 + 16.0 * e),
        1.0 * e,
        rgba(255, 255, 255, 0.12),
    );
    let sobre = acciones.en(raton).flatten();
    for (a, r) in &acciones.botones {
        let rf = l.r(*r);
        let primaria = *a == Accion::Copiar;
        let abierto = *a == Accion::AlChat && s.selector.is_some();
        let encima = sobre == Some(*a);
        let fondo = match (primaria, abierto, encima, *a == Accion::Descartar) {
            (true, _, true, _) => PRIMARIO_ENCIMA,
            (true, _, false, _) => PRIMARIO,
            (_, true, _, _) => ELEGIDO,
            (_, _, true, _) => ENCIMA_FUERTE,
            (_, _, false, true) => rgba(0, 0, 0, 0.0),
            _ => ENCIMA,
        };
        if abierto {
            caja(p, rf, 10.0 * e, fondo, AZUL, 1.0 * e);
        } else {
            p.rellenar_redondeado(rf, 10.0 * e, fondo);
        }
        let color = if primaria {
            Color::BLANCO
        } else if *a == Accion::Descartar {
            GRIS
        } else {
            TEXTO
        };
        let cy = rf.y + rf.alto / 2.0;
        let mut x = rf.x + 14.0 * e;
        if let Some(i) = icono_accion(*a) {
            p.icono(i, cuadro_icono(x + 9.0 * e, cy, 18.0 * e), color);
            x += 26.0 * e;
        }
        let w_chapa = ancho_chapa(p, a.atajo(), e);
        let flechita = if *a == Accion::AlChat { 16.0 * e } else { 0.0 };
        let max = rf.x + rf.ancho - 10.0 * e - w_chapa - flechita - 8.0 * e - x;
        let rotulo = caber(p, t.accion(*a), 14.0 * e, max);
        let (tw, th) = if primaria {
            p.medir_con_letra(&rotulo, 14.0 * e, 1.0e6, &SEMI)
        } else {
            p.medir_texto(&rotulo, 14.0 * e)
        };
        if primaria {
            p.texto_con_letra(&rotulo, x, cy - th / 2.0, 14.0 * e, 1.0e6, &SEMI, color);
        } else {
            p.texto(&rotulo, x, cy - th / 2.0, 14.0 * e, color);
        }
        x += tw + 8.0 * e;
        x += chapa(p, a.atajo(), x, cy, e, primaria);
        if flechita > 0.0 {
            p.icono(&ICONO_FLECHITA, cuadro_icono(x + 10.0 * e, cy, 12.0 * e), color);
        }
    }
}

/// El boton unico de confirmar de los modos que no preguntan.
pub fn pintar_confirmar(p: &Pintor, l: Local, r: Rect, rotulo: &str, raton: Punto) {
    let e = l.e;
    let rf = l.r(r);
    let fondo = if r.contiene(raton) { PRIMARIO_ENCIMA } else { PRIMARIO };
    p.rellenar_redondeado(rf, 10.0 * e, fondo);
    let cy = rf.y + rf.alto / 2.0;
    let w_chapa = ancho_chapa(p, "Enter", e);
    let max = rf.ancho - 28.0 * e - w_chapa - 8.0 * e;
    let rotulo = caber(p, rotulo, 14.0 * e, max);
    let (_, th) = p.medir_con_letra(&rotulo, 14.0 * e, 1.0e6, &SEMI);
    p.texto_con_letra(&rotulo, rf.x + 14.0 * e, cy - th / 2.0, 14.0 * e, 1.0e6, &SEMI, Color::BLANCO);
    chapa(p, "Enter", rf.x + rf.ancho - 12.0 * e - w_chapa, cy, e, true);
}

/// El selector de proyecto de «Al chat».
#[allow(clippy::too_many_arguments)]
pub fn pintar_selector(
    p: &Pintor,
    l: Local,
    sel: &Selector,
    es: &EstadoSelector,
    proyectos: &[(String, String)],
    ultimo: Option<&str>,
    t: &Textos,
    raton: Punto,
) {
    let e = l.e;
    panel(p, l.r(sel.panel), 14.0 * e, e);
    // Buscar.
    let b = l.r(sel.buscar);
    caja(
        p,
        b,
        10.0 * e,
        CAMPO_FONDO,
        if es.en_comentario { BORDE } else { AZUL },
        1.0 * e,
    );
    let cy = b.y + b.alto / 2.0;
    p.icono(&ix::SEARCH_ICON, cuadro_icono(b.x + 18.0 * e, cy, 16.0 * e), GRIS);
    let tx = b.x + 34.0 * e;
    if es.busqueda.is_empty() {
        texto_centrado_v(p, &t.buscar_proyecto, tx, cy, 14.0 * e, GRIS);
        if !es.en_comentario {
            p.linea((tx, cy - 9.0 * e), (tx, cy + 9.0 * e), 1.5 * e, AZUL);
        }
    } else {
        let w = texto_centrado_v(p, &es.busqueda, tx, cy, 14.0 * e, TEXTO);
        if !es.en_comentario {
            p.linea((tx + w + 1.0 * e, cy - 9.0 * e), (tx + w + 1.0 * e, cy + 9.0 * e), 1.5 * e, AZUL);
        }
    }
    // Recientes.
    p.texto(
        &t.recientes,
        b.x + 4.0 * e,
        l.y(sel.rotulo_y) + 8.0 * e,
        12.0 * e,
        GRIS,
    );
    let visibles = es.visibles(proyectos);
    if visibles.is_empty() {
        let y = l.y(sel.rotulo_y) + 28.0 * e;
        texto_centrado_v(p, &t.sin_proyectos, b.x + 12.0 * e, y + 22.0 * e, 14.0 * e, GRIS);
    }
    let sobre = sel.en(raton);
    for (i, ((id, nombre), r)) in visibles.iter().map(|x| (&x.0, &x.1)).zip(&sel.filas).enumerate() {
        let rf = l.r(*r);
        let elegida = i == es.elegida.min(visibles.len().saturating_sub(1));
        if elegida || sobre == Some(EnSelector::Fila(i)) {
            p.rellenar_redondeado(rf, 9.0 * e, rgba(10, 132, 255, 0.20));
        }
        let cy = rf.y + rf.alto / 2.0;
        p.icono(&ICONO_CHAT, cuadro_icono(rf.x + 20.0 * e, cy, 16.0 * e), AZUL);
        let n = (i + 1).to_string();
        let w_chapa = ancho_chapa(p, &n, e);
        let es_ultimo = ultimo == Some(id.as_str());
        let w_ult = if es_ultimo {
            p.medir_texto(&t.el_ultimo, 12.0 * e).0 + 10.0 * e
        } else {
            0.0
        };
        let max = rf.ancho - 40.0 * e - w_chapa - w_ult - 20.0 * e;
        let nombre = caber(p, nombre, 14.0 * e, max);
        texto_centrado_v(p, &nombre, rf.x + 36.0 * e, cy, 14.0 * e, TEXTO);
        let xc = rf.x + rf.ancho - 12.0 * e - w_chapa;
        if es_ultimo {
            texto_centrado_v(p, &t.el_ultimo, xc - w_ult, cy, 12.0 * e, GRIS);
        }
        chapa(p, &n, xc, cy, e, false);
    }
    let ly = l.y(sel.linea_y);
    let pr = l.r(sel.panel);
    p.linea(
        (pr.x + 12.0 * e, ly),
        (pr.x + pr.ancho - 12.0 * e, ly),
        1.0 * e,
        rgba(255, 255, 255, 0.08),
    );
    // Comentario y Enviar.
    let c = l.r(sel.comentario);
    caja(
        p,
        c,
        10.0 * e,
        CAMPO_FONDO,
        if es.en_comentario { AZUL } else { BORDE },
        1.0 * e,
    );
    let cy = c.y + c.alto / 2.0;
    p.con_recorte(c, |p| {
        if es.comentario.is_empty() {
            texto_centrado_v(p, &t.comentario, c.x + 10.0 * e, cy, 13.0 * e, GRIS);
            if es.en_comentario {
                p.linea((c.x + 10.0 * e, cy - 9.0 * e), (c.x + 10.0 * e, cy + 9.0 * e), 1.5 * e, AZUL);
            }
        } else {
            // Lo ultimo escrito siempre a la vista: si no cabe, se corre.
            let (w, _) = p.medir_texto(&es.comentario, 13.0 * e);
            let x = (c.x + 10.0 * e).min(c.x + c.ancho - 12.0 * e - w);
            texto_centrado_v(p, &es.comentario, x, cy, 13.0 * e, TEXTO);
            if es.en_comentario {
                p.linea((x + w + 1.0 * e, cy - 9.0 * e), (x + w + 1.0 * e, cy + 9.0 * e), 1.5 * e, AZUL);
            }
        }
    });
    let en = l.r(sel.enviar);
    let hay = es.destino(proyectos).is_some();
    let fondo = if !hay {
        rgba(255, 255, 255, 0.08)
    } else if sobre == Some(EnSelector::Enviar) {
        PRIMARIO_ENCIMA
    } else {
        PRIMARIO
    };
    p.rellenar_redondeado(en, 10.0 * e, fondo);
    let (tw, th) = p.medir_con_letra(&t.enviar, 13.0 * e, 1.0e6, &SEMI);
    p.texto_con_letra(
        &t.enviar,
        en.x + (en.ancho - tw) / 2.0,
        en.y + (en.alto - th) / 2.0,
        13.0 * e,
        1.0e6,
        &SEMI,
        if hay { Color::BLANCO } else { APAGADO },
    );
    let _ = ICONO_CERRAR;
}

/// Los tiradores de la zona elegida, como en la maqueta: cuadraditos
/// blancos con borde azul.
pub fn pintar_tiradores(p: &Pintor, s: RectF, e: f32) {
    let lado = 12.0 * e;
    for (tx, ty) in [
        (s.x, s.y),
        (s.x + s.ancho / 2.0, s.y),
        (s.x + s.ancho, s.y),
        (s.x + s.ancho, s.y + s.alto / 2.0),
        (s.x + s.ancho, s.y + s.alto),
        (s.x + s.ancho / 2.0, s.y + s.alto),
        (s.x, s.y + s.alto),
        (s.x, s.y + s.alto / 2.0),
    ] {
        let c = RectF {
            x: tx - lado / 2.0,
            y: ty - lado / 2.0,
            ancho: lado,
            alto: lado,
        };
        caja(p, c, 3.0 * e, Color::BLANCO, AZUL, 2.0 * e);
    }
}
