//! Las herramientas propias del universo. Las de dibujar son las del motor.
//!
//! H3: ademas del planeta, el emoji y las lineas, las **figuras** y los
//! **rotulos** del «Anadir» del movil (`UniversoModelo.kt`, `Cuerpo.FIGURAS`
//! y `Cuerpo.ROTULO`). Aqui se ponen como anotaciones del cielo, igual que el
//! emoji: se mueven con su planeta (D204), se deshacen y se guardan en
//! `universo.json` sin un tipo de astro nuevo que el resto tuviera que
//! conocer. Las hojas del proyecto entran por la nebulosa (ver el cargador).

use pixpin_motor2d::{ColorRgba, Elemento, Figura, Punto2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HerramientaUniverso {
    Planeta,
    Emoji,
    Figura,
    Rotulo,
    Conectar,
}

/// Las figuras de componer del movil, en su orden (`Cuerpo.FIGURAS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FiguraUniverso {
    #[default]
    Circulo,
    Cuadro,
    Triangulo,
    Estrella,
    Flecha,
}

pub const FIGURAS: [FiguraUniverso; 5] = [
    FiguraUniverso::Circulo,
    FiguraUniverso::Cuadro,
    FiguraUniverso::Triangulo,
    FiguraUniverso::Estrella,
    FiguraUniverso::Flecha,
];

/// El primer color de los cuerpos del movil (`COLORES_DE_CUERPO[0]`,
/// `#5B8CFF`): el de una figura recien puesta.
pub const COLOR_DE_CUERPO: u32 = 0x5b8cff;

fn color(rgb: u32) -> ColorRgba {
    ColorRgba::opaco(
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
    )
}

/// El color de un rotulo: el del cuerpo aclarado al 60 % hacia el blanco,
/// como `lerp(color, White, 0.6)` del movil. Sobre el cielo oscuro se lee.
pub fn color_de_rotulo() -> ColorRgba {
    let c = color(COLOR_DE_CUERPO);
    let a = |v: f32| v + (1.0 - v) * 0.6;
    ColorRgba::opaco(a(c.r), a(c.g), a(c.b))
}

/// Los vertices de un poligono de la figura, en un cuadrado de `lado` con la
/// esquina en `(x, y)`. Las proporciones son las del `Canvas` del movil
/// (`Universo.kt:700-728`).
pub fn vertices(f: FiguraUniverso, x: f32, y: f32, lado: f32) -> Vec<Punto2> {
    let (w, h) = (lado, lado);
    let p = |px: f32, py: f32| Punto2::nuevo(x + px, y + py);
    match f {
        FiguraUniverso::Triangulo => vec![
            p(w / 2.0, h * 0.1),
            p(w * 0.92, h * 0.88),
            p(w * 0.08, h * 0.88),
            p(w / 2.0, h * 0.1),
        ],
        FiguraUniverso::Estrella => {
            let mut v: Vec<Punto2> = (0..10)
                .map(|k| {
                    let r = if k % 2 == 0 { w * 0.46 } else { w * 0.2 };
                    let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
                    p(w / 2.0 + r * a.cos(), h / 2.0 + r * a.sin())
                })
                .collect();
            v.push(v[0]);
            v
        }
        FiguraUniverso::Flecha => vec![p(w * 0.08, h / 2.0), p(w * 0.9, h / 2.0)],
        FiguraUniverso::Cuadro => vec![
            p(w * 0.1, h * 0.1),
            p(w * 0.9, h * 0.1),
            p(w * 0.9, h * 0.9),
            p(w * 0.1, h * 0.9),
            p(w * 0.1, h * 0.1),
        ],
        FiguraUniverso::Circulo => Vec::new(),
    }
}

/// **La figura como anotacion del cielo**, centrada en `(cx, cy)` y de
/// `lado` de mundo. A trazo, del color del cuerpo y con el grosor del movil
/// (3 dp en 60: un 5 % del lado), sin temblor: es una pieza de componer.
pub fn elemento_de_figura(f: FiguraUniverso, cx: f32, cy: f32, lado: f32) -> Elemento {
    let (x, y) = (cx - lado / 2.0, cy - lado / 2.0);
    let figura = match f {
        FiguraUniverso::Circulo => Figura::Elipse,
        FiguraUniverso::Cuadro => Figura::Rectangulo,
        FiguraUniverso::Flecha => Figura::Flecha {
            puntos: vertices(f, x, y, lado),
            punta_inicio: Default::default(),
            punta_fin: pixpin_motor2d::formas::TipoPunta::Flecha,
            codos: false,
        },
        _ => Figura::Linea {
            puntos: vertices(f, x, y, lado),
        },
    };
    // El circulo del movil tiene radio 0,44 del lado y el cuadro va del 10 al
    // 90 %: su caja es la del trazo, no la del cuadrado entero.
    let (bx, by, bl) = match f {
        FiguraUniverso::Circulo => (x + lado * 0.06, y + lado * 0.06, lado * 0.88),
        FiguraUniverso::Cuadro => (x + lado * 0.1, y + lado * 0.1, lado * 0.8),
        _ => (x, y, lado),
    };
    Elemento {
        figura,
        x: bx,
        y: by,
        ancho: bl,
        alto: bl,
        trazo: color(COLOR_DE_CUERPO),
        grosor: lado * 0.05,
        rugosidad: 0.0,
        opacidad: 1.0,
        semilla: 1,
        ..Default::default()
    }
}

/// **Un rotulo**: texto grande y claro sobre el cielo (`Cuerpo.ROTULO`,
/// 24 sp en negrita en el movil). `tam` en mundo; la caja se estima a 0,6
/// del tamano por letra, que es lo que ocupa de media la letra del lienzo, y
/// el editor la vuelve a medir al tocarla.
pub fn elemento_de_rotulo(texto: &str, x: f32, y: f32, tam: f32) -> Elemento {
    let lineas = texto.lines().count().max(1) as f32;
    let largo = texto
        .lines()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0)
        .max(1) as f32;
    Elemento {
        figura: Figura::Texto {
            texto: texto.to_string(),
            tam,
            familia: "Segoe UI".to_string(),
        },
        x,
        y,
        ancho: largo * tam * 0.6,
        alto: lineas * tam * 1.25,
        trazo: color_de_rotulo(),
        grosor: 1.0,
        rugosidad: 0.0,
        opacidad: 1.0,
        semilla: 1,
        ..Default::default()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_cinco_figuras_del_movil_estan_y_en_su_orden() {
        assert_eq!(FIGURAS.len(), 5);
        assert_eq!(FIGURAS[0], FiguraUniverso::Circulo);
        assert_eq!(FIGURAS[4], FiguraUniverso::Flecha);
    }

    #[test]
    fn cada_figura_cabe_en_su_cuadrado_y_queda_centrada_donde_se_pulso() {
        for f in FIGURAS {
            let e = elemento_de_figura(f, 100.0, 200.0, 60.0);
            let (x0, y0, x1, y1) = e.caja();
            // El medio grosor puede asomar un pelo por el borde.
            let m = e.grosor;
            assert!(x0 >= 70.0 - m && x1 <= 130.0 + m, "{f:?} en x: {x0}..{x1}");
            assert!(y0 >= 170.0 - m && y1 <= 230.0 + m, "{f:?} en y: {y0}..{y1}");
            assert!(e.trazo.a > 0.99 && e.relleno.is_none(), "a trazo");
        }
    }

    #[test]
    fn la_estrella_tiene_cinco_puntas_y_cierra_y_el_triangulo_tambien_cierra() {
        let v = vertices(FiguraUniverso::Estrella, 0.0, 0.0, 100.0);
        assert_eq!(v.len(), 11);
        assert_eq!(v[0], v[10]);
        // La primera punta arriba del todo, en el centro.
        assert!((v[0].x - 50.0).abs() < 1e-3 && (v[0].y - 4.0).abs() < 1e-3);
        let t = vertices(FiguraUniverso::Triangulo, 0.0, 0.0, 100.0);
        assert_eq!(t.first(), t.last());
        // Caso negativo: el circulo no es un poligono.
        assert!(vertices(FiguraUniverso::Circulo, 0.0, 0.0, 100.0).is_empty());
    }

    #[test]
    fn un_rotulo_es_texto_claro_con_su_caja_segun_lo_escrito() {
        let e = elemento_de_rotulo("Fase 2", 10.0, 20.0, 40.0);
        let Figura::Texto { texto, tam, .. } = &e.figura else {
            panic!("no es texto");
        };
        assert_eq!((texto.as_str(), *tam), ("Fase 2", 40.0));
        assert!((e.ancho - 6.0 * 40.0 * 0.6).abs() < 1e-3);
        let c = e.trazo;
        assert!(c.r > 0.7 && c.g > 0.7 && c.b > 0.9, "aclarado: {c:?}");
        // Dos lineas, el doble de alto.
        let dos = elemento_de_rotulo("a\nb", 0.0, 0.0, 40.0);
        assert!((dos.alto - 2.0 * e.alto).abs() < 1e-3);
    }
}
