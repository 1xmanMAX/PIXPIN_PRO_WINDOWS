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
//!
//! **Una hoja lleva su propio papel** (27-oct-2026, `PapelDeLaHoja.kt` del
//! movil; la regla en `pixpin_motor2d::papel_de_la_hoja`): las piezas de una
//! plantilla estampada se pintan como se disenaron, lo dibujado encima se
//! adapta a su pagina y, dentro de un simbolo, cada pieza a lo que tiene
//! debajo. Antes, en el lienzo de noche, la tinta oscura de una hoja de dia
//! salia clara sobre su pagina blanca y el papel azul de las gemelas
//! «(noche)» se aclaraba. Quien pinta fija las hojas de su escena
//! ([`fijar_hojas`], [`con_hojas`]) y cada elemento se pinta dentro de
//! [`con_la_pieza`] (lo hace `pintar::por_cada_orden`).

use pixpin_motor2d::ColorRgba;
use pixpin_motor2d::papel_de_la_hoja::{self as hoja, PapelDeLaPieza};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
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
    /// Lo ya adaptado: en un dibujo hay pocos colores distintos y adaptarlos
    /// es buscar paso a paso. Sin esto se repetiria la busqueda con cada
    /// orden de cada elemento en cada fotograma. La clave es el color y el
    /// papel contra el que se adapto (el del lienzo o el de una hoja).
    static HECHOS: RefCell<Vec<([u8; 4], [u8; 4], ColorRgba)>> = const { RefCell::new(Vec::new()) };
    /// Las hojas de la escena que se pinta ahora ([`fijar_hojas`]).
    static HOJAS: RefCell<Rc<hoja::Mapa>> = RefCell::new(Rc::new(hoja::Mapa::default()));
    /// El ultimo mapa hecho y la huella de los elementos de que salio: se
    /// rehace solo si el dibujo cambio.
    static HOJAS_HECHAS: RefCell<Option<(u64, Rc<hoja::Mapa>)>> = const { RefCell::new(None) };
    /// La pieza que se esta pintando ([`con_la_pieza`]).
    static PIEZA: Cell<Option<u64>> = const { Cell::new(None) };
    /// El papel del fotograma tal cual, de dia o de noche: la cota lo
    /// necesita siempre (ver [`papel_del_fotograma`]).
    static PAPEL_DEL_FOTOGRAMA: Cell<Option<ColorRgba>> = const { Cell::new(None) };
}

fn clave(c: ColorRgba) -> [u8; 4] {
    [a8(c.r), a8(c.g), a8(c.b), a8(c.a)]
}

/// Pinta lo que haga `f` con la tinta adaptada a `papel`, si es de noche.
/// Se puede anidar: al salir vuelve el papel que hubiera.
pub fn con_papel<R>(papel: Option<ColorRgba>, f: impl FnOnce() -> R) -> R {
    let crudo_antes = PAPEL_DEL_FOTOGRAMA.with(|c| c.replace(papel));
    let nuevo = papel.filter(|p| es_de_noche(*p));
    let antes = PAPEL.with(|c| c.replace(nuevo));
    if antes.map(clave) != nuevo.map(clave) {
        HECHOS.with(|h| h.borrow_mut().clear());
    }
    let r = f();
    PAPEL_DEL_FOTOGRAMA.with(|c| c.set(crudo_antes));
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
    PAPEL_DEL_FOTOGRAMA.with(|c| c.set(papel));
    let nuevo = papel.filter(|p| es_de_noche(*p));
    let antes = PAPEL.with(|c| c.replace(nuevo));
    if antes.map(clave) != nuevo.map(clave) {
        HECHOS.with(|h| h.borrow_mut().clear());
    }
}

/// **El papel sobre el que se pinta ahora**, sea de dia o de noche, o blanco
/// si nadie lo fijo. Lo pide la cota: el movil pinta su raya y su numero con
/// la tinta adaptada a CUALQUIER papel (`tema(...)` en `drawMeasure`), no
/// solo al de noche como el resto de la tinta aqui.
pub fn papel_del_fotograma() -> ColorRgba {
    PAPEL_DEL_FOTOGRAMA
        .with(Cell::get)
        .unwrap_or(ColorRgba::opaco(1.0, 1.0, 1.0))
}

/// **Si lo que se pinta ahora va sobre papel de noche.** La barra de
/// herramientas lo mira para pintarse con el tema oscuro de Excalidraw: una
/// isla blanca sobre la pizarra deslumbra, y el movil tambien oscurece sus
/// barras con el papel («un cambio dinamico segun el color del canvas»).
pub fn de_noche() -> bool {
    PAPEL.with(Cell::get).is_some()
}

/// La tinta `c` tal como se pinta ahora: adaptada si hay papel de noche
/// fijado, y tal cual si no (el camino de siempre, sin coste). Si la pieza
/// que se pinta es de una hoja, tal cual; si va encima de una, contra el
/// papel de esa hoja (ver [`con_la_pieza`]).
pub fn tinta(c: ColorRgba) -> ColorRgba {
    let Some(papel) = PAPEL.with(Cell::get) else {
        return c;
    };
    let contra = match PIEZA.with(Cell::get) {
        None => papel,
        Some(id) => match HOJAS.with(|h| h.borrow().de_la_pieza(id)) {
            PapelDeLaPieza::ComoSeDiseno => return c,
            PapelDeLaPieza::Papel(p) => p,
            PapelDeLaPieza::Lienzo => papel,
        },
    };
    let (k, kp) = (clave(c), clave(contra));
    HECHOS.with(|h| {
        let mut h = h.borrow_mut();
        if let Some((_, _, hecho)) = h.iter().find(|(kk, pp, _)| *kk == k && *pp == kp) {
            return *hecho;
        }
        let hecho = adaptar(c, contra);
        // Tope: un dibujo con cientos de colores distintos no puede hacer
        // crecer esto sin fin.
        if h.len() >= 256 {
            h.clear();
        }
        h.push((k, kp, hecho));
        hecho
    })
}

/// La huella de lo que decide las hojas: que piezas hay, en que orden, de
/// que version y donde. Cambia con cualquier edicion.
fn huella(elementos: &[pixpin_motor2d::Elemento]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    elementos.len().hash(&mut h);
    for e in elementos {
        (e.id, e.version, e.borrado, e.grupos.len()).hash(&mut h);
        (
            e.x.to_bits(),
            e.y.to_bits(),
            e.ancho.to_bits(),
            e.alto.to_bits(),
        )
            .hash(&mut h);
    }
    h.finish()
}

/// **Las hojas de estos elementos**, hechas una vez por cambio del dibujo.
/// Sin papel de noche no hacen falta (la tinta va tal cual) y sale vacio.
fn mapa_de(elementos: &[pixpin_motor2d::Elemento]) -> Rc<hoja::Mapa> {
    if PAPEL.with(Cell::get).is_none() || elementos.iter().all(|e| e.grupos.is_empty()) {
        return Rc::new(hoja::Mapa::default());
    }
    let k = huella(elementos);
    HOJAS_HECHAS.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((kk, m)) = c.as_ref()
            && *kk == k
        {
            return m.clone();
        }
        let m = Rc::new(hoja::de(elementos));
        *c = Some((k, m.clone()));
        m
    })
}

/// **Fija las hojas de la escena** que se pinta en este hilo hasta nuevo
/// aviso, como [`fijar_papel`] (despues de el: sin papel de noche no se
/// mira nada). Para el lienzo, que pinta desde varios sitios.
pub fn fijar_hojas(elementos: &[pixpin_motor2d::Elemento]) {
    let m = mapa_de(elementos);
    HOJAS.with(|h| *h.borrow_mut() = m);
}

/// Pinta lo que haga `f` con las hojas de `elementos` (ver [`fijar_hojas`]).
/// Al salir vuelven las que hubiera.
pub fn con_hojas<R>(elementos: &[pixpin_motor2d::Elemento], f: impl FnOnce() -> R) -> R {
    let m = mapa_de(elementos);
    let antes = HOJAS.with(|h| std::mem::replace(&mut *h.borrow_mut(), m));
    let r = f();
    HOJAS.with(|h| *h.borrow_mut() = antes);
    r
}

/// Pinta lo que haga `f` como la pieza `id`: su tinta, segun sea de una
/// hoja o vaya encima de una. Se anida (el mosaico pinta lo de debajo desde
/// dentro de su propio pintado): al salir vuelve la que hubiera.
pub fn con_la_pieza<R>(id: u64, f: impl FnOnce() -> R) -> R {
    let antes = PIEZA.with(|p| p.replace(Some(id)));
    let r = f();
    PIEZA.with(|p| p.set(antes));
    r
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

/// **Unas ordenes con la tinta tal como las pinta el lienzo sobre `papel`**,
/// para quien las pinta fuera del lienzo: las vistas previas del chat y de
/// Proyectos. Es la misma regla y la misma cuenta que [`tinta`] al pintar
/// (solo en papel de noche), hecha una vez al leer en vez de en cada
/// fotograma. Sin esto la vista previa de un lienzo en la pizarra ensenaba
/// la tinta negra guardada sobre el papel negro: nada, mientras en el lienzo
/// se leia blanca. El movil hace lo mismo en `DrawExport.aBitmap`
/// (`Renderer(dark = DrawTheme.esDeNoche(scene.backgroundColor))`).
pub fn ordenes_como_en_el_lienzo(
    ordenes: Vec<pixpin_motor2d::Orden>,
    papel: ColorRgba,
) -> Vec<pixpin_motor2d::Orden> {
    use pixpin_motor2d::Orden;
    if !es_de_noche(papel) {
        return ordenes;
    }
    con_papel(Some(papel), || {
        ordenes
            .into_iter()
            .map(|mut o| {
                match &mut o {
                    Orden::Poligono { color, .. }
                    | Orden::Tinta { color, .. }
                    | Orden::Polilinea { color, .. }
                    | Orden::Relleno { color, .. }
                    | Orden::Velo { color, .. }
                    | Orden::Texto { color, .. }
                    | Orden::Rotulo { color, .. } => *color = tinta(*color),
                    Orden::Imagen { .. } => {}
                }
                o
            })
            .collect()
    })
}

/// **El grafito de `escena` con la tinta del lienzo sobre su papel**
/// (`escena.fondo`), antes de cocerlo para una vista previa: su color va
/// dentro del mapa y [`ordenes_como_en_el_lienzo`] no llega a verlo. Es
/// [`grafito_adaptado`], lo mismo que hace el lienzo al cocerlo.
pub fn grafito_como_en_el_lienzo(escena: &mut pixpin_motor2d::Escena) {
    if !es_de_noche(escena.fondo) {
        return;
    }
    con_papel(Some(escena.fondo), || {
        for e in escena.elementos.iter_mut() {
            if pixpin_motor2d::tinta::grafito::es_de_grafito(e)
                && let Some(copia) = grafito_adaptado(e)
            {
                *e = copia;
            }
        }
    });
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_grafito_negro_sobre_la_pizarra_se_cuece_claro() {
        use pixpin_motor2d::tinta::{MaterialTinta, grafito};
        use pixpin_motor2d::{Elemento, Figura, vector::Punto2};
        let puntos: Vec<Punto2> = (0..40)
            .map(|i| Punto2::nuevo(i as f32 * 4.0, 20.0))
            .collect();
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
            assert!(
                c.r > 0.8 && c.g > 0.8 && c.b > 0.8,
                "un gris se da la vuelta: {c:?}"
            );
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
        let c = ColorRgba {
            a: 0.35,
            ..hex(0x1e1e1e)
        };
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

    // --- Una hoja lleva su propio papel ------------------------------------

    /// Una hoja blanca agrupada con su tinta oscura, y un trazo suelto
    /// encima en el centro.
    fn hoja_con_trazo() -> Vec<pixpin_motor2d::Elemento> {
        use pixpin_motor2d::relleno::EstiloRelleno;
        use pixpin_motor2d::{Elemento, Figura};
        let g = vec!["g1".to_string()];
        let pagina = Elemento {
            id: 1,
            figura: Figura::Rectangulo,
            ancho: 200.0,
            alto: 200.0,
            relleno: Some(hex(0xffffff)),
            estilo_relleno: EstiloRelleno::Solido,
            grupos: g.clone(),
            ..Default::default()
        };
        let raya = Elemento {
            id: 2,
            figura: Figura::Rectangulo,
            x: 10.0,
            y: 10.0,
            ancho: 50.0,
            alto: 5.0,
            trazo: hex(0x22304a),
            grupos: g,
            ..Default::default()
        };
        let encima = Elemento {
            id: 3,
            figura: Figura::Rectangulo,
            x: 90.0,
            y: 90.0,
            ancho: 20.0,
            alto: 20.0,
            trazo: hex(0xffffff),
            ..Default::default()
        };
        vec![pagina, raya, encima]
    }

    #[test]
    fn en_el_lienzo_de_noche_la_tinta_de_una_hoja_va_como_se_diseno() {
        let v = hoja_con_trazo();
        let oscura = hex(0x22304a);
        let (de_la_hoja, encima, suelta) = con_papel(Some(hex(PIZARRA)), || {
            con_hojas(&v, || {
                (
                    con_la_pieza(2, || tinta(oscura)),
                    con_la_pieza(3, || tinta(hex(0xffffff))),
                    con_la_pieza(99, || tinta(oscura)),
                )
            })
        });
        // La de la hoja, tal cual: oscura sobre su pagina blanca.
        assert_eq!(de_la_hoja, oscura);
        // Lo de encima, contra la hoja: el blanco pasa a oscuro y se lee.
        assert!(contraste_de(encima, hex(0xffffff)) >= 4.5, "{encima:?}");
        // Caso negativo: lo que no es de la hoja, contra la pizarra, como siempre.
        assert_eq!(suelta, adaptar(oscura, hex(PIZARRA)));
        // Y fuera de todo, las hojas se van con el fotograma.
        assert_eq!(
            con_papel(Some(hex(PIZARRA)), || con_la_pieza(2, || tinta(oscura))),
            suelta
        );
    }

    #[test]
    fn sobre_papel_claro_las_hojas_no_cambian_nada() {
        // Caso negativo: de dia nada se adapta, haya hojas o no.
        let v = hoja_con_trazo();
        let blanco = hex(0xffffff);
        let c = con_papel(Some(blanco), || {
            con_hojas(&v, || con_la_pieza(3, || tinta(blanco)))
        });
        assert_eq!(c, blanco);
    }

    // --- Fuera del lienzo: las vistas previas ------------------------------

    fn texto(color: ColorRgba) -> pixpin_motor2d::Orden {
        pixpin_motor2d::Orden::Texto {
            texto: "Hola".into(),
            x: 0.0,
            y: 0.0,
            tam: 20.0,
            familia: "Virgil".into(),
            color,
            ancho_max: 100.0,
            negrita: false,
            cursiva: false,
        }
    }

    fn color_de(o: &pixpin_motor2d::Orden) -> ColorRgba {
        match o {
            pixpin_motor2d::Orden::Texto { color, .. }
            | pixpin_motor2d::Orden::Tinta { color, .. } => *color,
            _ => unreachable!(),
        }
    }

    #[test]
    fn la_vista_previa_sobre_papel_de_noche_pinta_la_tinta_como_el_lienzo() {
        let negro = hex(0x1e1e1e);
        let ordenes = vec![
            texto(negro),
            pixpin_motor2d::Orden::Tinta {
                contorno: Vec::new(),
                color: negro,
            },
        ];
        let vistas = ordenes_como_en_el_lienzo(ordenes, hex(NEGRO));
        // Lo mismo que el lienzo pinta con ese papel fijado.
        let en_el_lienzo = con_papel(Some(hex(NEGRO)), || tinta(negro));
        for o in &vistas {
            assert_eq!(color_de(o), en_el_lienzo);
            assert!(color_de(o).r > 0.8, "{:?}", color_de(o));
        }
        // Y no deja el papel fijado: lo que se pinte despues sale tal cual.
        assert_eq!(tinta(negro), negro);
    }

    #[test]
    fn la_vista_previa_sobre_papel_claro_o_con_tinta_que_ya_se_lee_no_cambia_nada() {
        // Casos negativos: tinta negra en papel blanco; tinta blanca en
        // papel negro (ya se lee: no pasa a negra); y papel crema.
        let negro = hex(0x1e1e1e);
        let blanco = hex(0xffffff);
        assert_eq!(
            color_de(&ordenes_como_en_el_lienzo(vec![texto(negro)], hex(0xffffff))[0]),
            negro
        );
        assert_eq!(
            color_de(&ordenes_como_en_el_lienzo(vec![texto(blanco)], hex(NEGRO))[0]),
            blanco
        );
        assert_eq!(
            color_de(&ordenes_como_en_el_lienzo(vec![texto(blanco)], hex(0xfdf6e3))[0]),
            blanco
        );
    }

    #[test]
    fn el_grafito_de_la_vista_previa_se_cuece_con_la_tinta_del_lienzo() {
        let mut escena = pixpin_motor2d::Escena::nueva();
        escena.fondo = hex(PIZARRA);
        escena.elementos.push(pixpin_motor2d::Elemento {
            trazo: hex(0x1e1e1e),
            material: pixpin_motor2d::tinta::MaterialTinta::Cuadritos,
            ..Default::default()
        });
        // Uno que no es de grafito no se toca aqui: su tinta la adapta
        // `ordenes_como_en_el_lienzo`, como en el lienzo.
        escena.elementos.push(pixpin_motor2d::Elemento {
            trazo: hex(0x1e1e1e),
            ..Default::default()
        });
        grafito_como_en_el_lienzo(&mut escena);
        assert!(
            escena.elementos[0].trazo.r > 0.8,
            "{:?}",
            escena.elementos[0].trazo
        );
        assert_eq!(escena.elementos[1].trazo, hex(0x1e1e1e));
        // Caso negativo: en papel blanco el grafito se queda como es.
        escena.fondo = hex(0xffffff);
        escena.elementos[0].trazo = hex(0x1e1e1e);
        grafito_como_en_el_lienzo(&mut escena);
        assert_eq!(escena.elementos[0].trazo, hex(0x1e1e1e));
    }
}
