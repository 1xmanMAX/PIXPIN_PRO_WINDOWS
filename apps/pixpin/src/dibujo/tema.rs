//! **La tinta sobre papel oscuro.** Puerto de `DrawTheme.adaptar` y
//! `DrawTheme.esDeNoche` (`motor/Theme.kt` del movil).
//!
//! Es pintura, no dibujo: el color guardado NO cambia nunca. Asi, cambiar de
//! papel y volver deja todo como estaba, y lo que se exporta o viaja al movil
//! es lo que se eligio. La regla es la del movil (WCAG 2.2): si la tinta ya
//! se lee sobre ese papel (3:1) no se toca; si no, se le cambia la claridad y
//! se le deja el tono hasta 4,5:1; y un gris es tinta, y se le da la vuelta.
//!
//! **Solo en papel de noche** (Pizarra, Cosmos, Azul noche, Verde pizarra,
//! Negro, el papel oscuro del lector). El movil adapta tambien sobre papel
//! claro (un amarillo sobre blanco pasa a ocre); aqui eso cambiaria dibujos
//! que el usuario ya hizo en el PC y que se ven como quiere, y lo que pidio
//! fue lo de los papeles oscuros. Sobre papel claro se pinta como siempre.
//!
//! El papel se fija por fotograma con [`con_papel`]: sin fijarlo nada cambia,
//! asi que las pruebas y los bancos que no lo fijan no notan nada.

use pixpin_motor2d::ColorRgba;
use std::cell::{Cell, RefCell};

/// Por debajo de esta saturacion, un color es un gris: tinta, no color.
const ES_GRIS: f64 = 0.14;
/// Por debajo de esto, la tinta se adapta. WCAG 1.4.11: trazos y graficos.
const CONTRASTE_QUE_VALE: f64 = 3.0;
/// Adonde se lleva la que se adapta: WCAG 1.4.3, texto normal.
const CONTRASTE_BUSCADO: f64 = 4.5;

fn a8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Luminancia relativa de WCAG, de canales de 0 a 255.
fn luminancia(r: u8, g: u8, b: u8) -> f64 {
    let lineal = |c: u8| {
        let v = c as f64 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lineal(r) + 0.7152 * lineal(g) + 0.0722 * lineal(b)
}

fn contraste(l1: f64, l2: f64) -> f64 {
    (l1.max(l2) + 0.05) / (l1.min(l2) + 0.05)
}

/// **Si un papel de este color es papel de noche**: la media ponderada de
/// sus canales (sin linealizar, como el movil) por debajo del gris medio.
pub fn es_de_noche(papel: ColorRgba) -> bool {
    (0.2126 * papel.r as f64 + 0.7152 * papel.g as f64 + 0.0722 * papel.b as f64) < 0.5
}

fn a_hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let alto = r.max(g).max(b);
    let bajo = r.min(g).min(b);
    let l = (alto + bajo) / 2.0;
    if alto == bajo {
        return (0.0, 0.0, l);
    }
    let d = alto - bajo;
    let s = if l > 0.5 {
        d / (2.0 - alto - bajo)
    } else {
        d / (alto + bajo)
    };
    let h = if alto == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if alto == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    (h, s, l)
}

fn de_hsl(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (l, l, l);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let canal = |t0: f64| {
        let mut t = t0;
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (canal(h + 1.0 / 3.0), canal(h), canal(h - 1.0 / 3.0))
}

/// **La tinta adaptada al papel** (`DrawTheme.adaptar`), sobre cualquier
/// papel. El alfa no cuenta para el contraste y se conserva.
pub fn adaptar(c: ColorRgba, papel: ColorRgba) -> ColorRgba {
    let lp = luminancia(a8(papel.r), a8(papel.g), a8(papel.b));
    let (r8, g8, b8) = (a8(c.r), a8(c.g), a8(c.b));
    if contraste(luminancia(r8, g8, b8), lp) >= CONTRASTE_QUE_VALE {
        return c;
    }
    let (h, s, l) = a_hsl(r8 as f64 / 255.0, g8 as f64 / 255.0, b8 as f64 / 255.0);
    let gris = s < ES_GRIS;
    // Hacia donde hay mas contraste: oscurecer sobre papel claro, aclarar
    // sobre oscuro.
    let oscurecer = contraste(0.0, lp) >= contraste(1.0, lp);
    let con = |claridad: f64| {
        let (r, g, b) = de_hsl(h, if gris { 0.0 } else { s }, claridad.clamp(0.0, 1.0));
        let r8 = (r * 255.0).round().clamp(0.0, 255.0) as u8;
        let g8 = (g * 255.0).round().clamp(0.0, 255.0) as u8;
        let b8 = (b * 255.0).round().clamp(0.0, 255.0) as u8;
        (r8, g8, b8)
    };
    // Un gris empieza en su espejo (blanco -> negro); un color, en su propia
    // claridad.
    let mut claridad = if gris { 1.0 - l } else { l };
    let paso = if oscurecer { -0.02 } else { 0.02 };
    let mut mejor = con(claridad);
    while (0.0..=1.0).contains(&claridad) {
        let x = con(claridad);
        mejor = x;
        if contraste(luminancia(x.0, x.1, x.2), lp) >= CONTRASTE_BUSCADO {
            break;
        }
        claridad += paso;
    }
    ColorRgba {
        r: mejor.0 as f32 / 255.0,
        g: mejor.1 as f32 / 255.0,
        b: mejor.2 as f32 / 255.0,
        a: c.a,
    }
}

thread_local! {
    /// El papel de noche del fotograma que se esta pintando en este hilo.
    static PAPEL: Cell<Option<ColorRgba>> = const { Cell::new(None) };
    /// Lo ya adaptado contra ese papel: en un dibujo hay pocos colores
    /// distintos y adaptarlos es buscar paso a paso. Sin esto se repetiria
    /// la busqueda con cada orden de cada elemento en cada fotograma.
    static HECHOS: RefCell<Vec<([u8; 4], ColorRgba)>> = const { RefCell::new(Vec::new()) };
}

fn clave(c: ColorRgba) -> [u8; 4] {
    [a8(c.r), a8(c.g), a8(c.b), a8(c.a)]
}

/// Pinta lo que haga `f` con la tinta adaptada a `papel`, si es de noche.
/// Se puede anidar: al salir vuelve el papel que hubiera.
pub fn con_papel<R>(papel: Option<ColorRgba>, f: impl FnOnce() -> R) -> R {
    let nuevo = papel.filter(|p| es_de_noche(*p));
    let antes = PAPEL.with(|c| c.replace(nuevo));
    if antes.map(clave) != nuevo.map(clave) {
        HECHOS.with(|h| h.borrow_mut().clear());
    }
    let r = f();
    let ahora = PAPEL.with(|c| c.replace(antes));
    if antes.map(clave) != ahora.map(clave) {
        HECHOS.with(|h| h.borrow_mut().clear());
    }
    r
}

/// Fija el papel de este hilo hasta nuevo aviso. Para los anfitriones con
/// un bucle propio (el lienzo), que pintan desde varios sitios (la escena, la
/// capa de la tinta, el horneado, las zonas) y fijan el papel una vez por
/// vuelta en vez de envolver cada uno. `None` lo quita.
pub fn fijar_papel(papel: Option<ColorRgba>) {
    let nuevo = papel.filter(|p| es_de_noche(*p));
    let antes = PAPEL.with(|c| c.replace(nuevo));
    if antes.map(clave) != nuevo.map(clave) {
        HECHOS.with(|h| h.borrow_mut().clear());
    }
}

/// **Si lo que se pinta ahora va sobre papel de noche.** La barra de
/// herramientas lo mira para pintarse con el tema oscuro de Excalidraw: una
/// isla blanca sobre la pizarra deslumbra, y el movil tambien oscurece sus
/// barras con el papel («un cambio dinamico segun el color del canvas»).
pub fn de_noche() -> bool {
    PAPEL.with(Cell::get).is_some()
}

/// La tinta `c` tal como se pinta ahora: adaptada si hay papel de noche
/// fijado, y tal cual si no (el camino de siempre, sin coste).
pub fn tinta(c: ColorRgba) -> ColorRgba {
    let Some(papel) = PAPEL.with(Cell::get) else {
        return c;
    };
    let k = clave(c);
    HECHOS.with(|h| {
        let mut h = h.borrow_mut();
        if let Some((_, hecho)) = h.iter().find(|(kk, _)| *kk == k) {
            return *hecho;
        }
        let hecho = adaptar(c, papel);
        // Tope: un dibujo con cientos de colores distintos no puede hacer
        // crecer esto sin fin.
        if h.len() >= 256 {
            h.clear();
        }
        h.push((k, hecho));
        hecho
    })
}

/// **El grafito, con su tinta adaptada** al papel de noche fijado, o `None`
/// si no hace falta (sin papel de noche, o si su tinta ya se lee).
///
/// El grafito no pasa sus colores por [`tinta`] al pintar como lo demas: su
/// color va DENTRO del mapa de casillas ya cocido (`tinta::grafito::cocer`),
/// y lo que se pinta es ese mapa tal cual. Por eso sobre la pizarra un
/// trazo de grafito negro salia negro sobre negro. El movil lo cuece con el
/// color ya adaptado (`Renderer.pintarElLapiz`: `cocerElLapiz(e,
/// tema(color))`); aqui igual: se cuece una copia con la tinta y el fondo
/// adaptados. Su huella cambia con el color, asi que el horno y los bitmaps
/// la guardan aparte y cambiar de papel la vuelve a cocer una sola vez.
pub fn grafito_adaptado(e: &pixpin_motor2d::Elemento) -> Option<pixpin_motor2d::Elemento> {
    PAPEL.with(Cell::get)?;
    let trazo = tinta(e.trazo);
    let relleno = e.relleno.map(tinta);
    if trazo == e.trazo && relleno == e.relleno {
        return None;
    }
    let mut copia = e.clone();
    copia.trazo = trazo;
    copia.relleno = relleno;
    Some(copia)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_grafito_negro_sobre_la_pizarra_se_cuece_claro() {
        use pixpin_motor2d::tinta::{MaterialTinta, grafito};
        use pixpin_motor2d::{Elemento, Figura, vector::Punto2};
        let puntos: Vec<Punto2> = (0..40).map(|i| Punto2::nuevo(i as f32 * 4.0, 20.0)).collect();
        let e = Elemento {
            id: 7,
            figura: Figura::Lapiz {
                puntos,
                presiones: Vec::new(),
                opciones: Some(pixpin_motor2d::tinta::OpcionesTinta::default()),
            },
            trazo: hex(0x1e1e1e),
            grosor: 2.0,
            material: MaterialTinta::Cuadritos,
            ..Default::default()
        };
        assert!(grafito::es_de_grafito(&e));
        // La casilla mas cargada del mapa: la tinta de verdad del trazo.
        let mas_cargada = |c: &grafito::Cocido| {
            c.rgba
                .chunks_exact(4)
                .max_by_key(|p| p[3])
                .map(|p| (p[0], p[1], p[2]))
                .unwrap()
        };
        let copia = con_papel(Some(hex(PIZARRA)), || grafito_adaptado(&e)).expect("se adapta");
        let claro = mas_cargada(&grafito::cocer_sin_horno(&copia).unwrap());
        assert!(claro.0 > 200 && claro.1 > 200 && claro.2 > 200, "{claro:?}");
        // Caso negativo: el original sigue negro, y sobre papel blanco no
        // hay copia (se cuece el de siempre).
        let oscuro = mas_cargada(&grafito::cocer_sin_horno(&e).unwrap());
        assert!(oscuro.0 < 60, "{oscuro:?}");
        assert!(con_papel(Some(hex(0xffffff)), || grafito_adaptado(&e)).is_none());
        assert!(grafito_adaptado(&e).is_none(), "sin papel fijado, nada");
    }

    #[test]
    fn un_grafito_que_ya_se_lee_en_la_pizarra_no_se_copia() {
        let e = pixpin_motor2d::Elemento {
            trazo: hex(0xffd43b),
            material: pixpin_motor2d::tinta::MaterialTinta::Cuadritos,
            ..Default::default()
        };
        assert!(con_papel(Some(hex(PIZARRA)), || grafito_adaptado(&e)).is_none());
    }

    fn hex(rgb: u32) -> ColorRgba {
        ColorRgba::opaco(
            ((rgb >> 16) & 0xff) as f32 / 255.0,
            ((rgb >> 8) & 0xff) as f32 / 255.0,
            (rgb & 0xff) as f32 / 255.0,
        )
    }

    const PIZARRA: u32 = 0x121212;
    const COSMOS: u32 = 0x0b0f24;
    const NEGRO: u32 = 0x000000;

    fn contraste_de(a: ColorRgba, b: ColorRgba) -> f64 {
        contraste(
            luminancia(a8(a.r), a8(a.g), a8(a.b)),
            luminancia(a8(b.r), a8(b.g), a8(b.b)),
        )
    }

    #[test]
    fn los_papeles_oscuros_del_movil_son_de_noche_y_los_claros_no() {
        for p in [PIZARRA, COSMOS, NEGRO, 0x14213d, 0x1f3b33, 0x14182b] {
            assert!(es_de_noche(hex(p)), "{p:06x}");
        }
        for p in [0xffffff, 0xfdf6e3, 0xd8dade, 0xe8f1fb] {
            assert!(!es_de_noche(hex(p)), "{p:06x}");
        }
    }

    #[test]
    fn la_tinta_negra_sobre_pizarra_sale_clara_y_se_lee() {
        let negro = hex(0x1e1e1e);
        for p in [PIZARRA, COSMOS, NEGRO] {
            let c = adaptar(negro, hex(p));
            assert!(c.r > 0.8 && c.g > 0.8 && c.b > 0.8, "un gris se da la vuelta: {c:?}");
            assert!(contraste_de(c, hex(p)) >= 4.5);
        }
    }

    #[test]
    fn un_azul_marino_sobre_negro_sigue_siendo_azul_pero_claro() {
        let marino = hex(0x1b2a6b);
        let c = adaptar(marino, hex(NEGRO));
        assert!(c.b > c.r && c.b > c.g, "conserva el tono: {c:?}");
        assert!(contraste_de(c, hex(NEGRO)) >= 4.5);
    }

    #[test]
    fn un_color_que_ya_se_lee_sobre_el_papel_no_se_toca() {
        // Caso negativo: el rojo vivo del panel se lee sobre la pizarra.
        let rojo = hex(0xe03131);
        assert_eq!(adaptar(rojo, hex(PIZARRA)), rojo);
        let amarillo = hex(0xffd43b);
        assert_eq!(adaptar(amarillo, hex(PIZARRA)), amarillo);
    }

    #[test]
    fn el_alfa_se_conserva_al_adaptar() {
        let c = ColorRgba { a: 0.35, ..hex(0x1e1e1e) };
        assert!((adaptar(c, hex(PIZARRA)).a - 0.35).abs() < 1e-6);
    }

    #[test]
    fn sobre_papel_claro_se_pinta_como_siempre() {
        // Caso negativo: el papel blanco no fija nada y la tinta sale tal cual,
        // aunque el movil la llevaria a ocre.
        let amarillo = hex(0xffec99);
        let c = con_papel(Some(hex(0xffffff)), || tinta(amarillo));
        assert_eq!(c, amarillo);
        // Sin papel, igual.
        assert_eq!(tinta(amarillo), amarillo);
    }

    #[test]
    fn con_papel_de_noche_la_tinta_se_adapta_y_al_salir_vuelve_como_estaba() {
        let negro = hex(0x1e1e1e);
        let dentro = con_papel(Some(hex(PIZARRA)), || {
            let a = tinta(negro);
            // La segunda vez sale de lo ya hecho, igual.
            assert_eq!(tinta(negro), a);
            a
        });
        assert_ne!(dentro, negro);
        assert_eq!(tinta(negro), negro, "fuera del fotograma, nada");
    }
}
