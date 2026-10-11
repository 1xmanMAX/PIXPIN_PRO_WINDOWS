//! **Pintar una pagina de Office** (`pixpin_office::Pagina`) con Direct2D, y
//! el medidor de verdad (DirectWrite) con el que se maqueta.
//!
//! El texto se pinta glifo a glifo (`DrawGlyphRun`) con las mismas caras y
//! los mismos avances con que se midio: asi lo que la maquetacion decidio
//! (donde parte cada renglon, donde cae cada palabra) es exactamente lo que
//! se ve. Si una letra del documento no esta instalada se usa una sustituta
//! de las mismas medidas si la hay (Carlito por Calibri...) y si no, Arial o
//! Times New Roman segun sea de palo seco o con remates.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use pixpin_office::medidor::{Medidor, Metricas};
use pixpin_office::pagina::{
    Campo, Color as ColorO, Discontinuo, Elemento, Figura, Letra, Pagina, Recursos, Relleno, Sombra, Subrayado, Texto, Tramo, Trazo,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_BEZIER_SEGMENT, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN,
    D2D1_FILL_MODE_ALTERNATE, D2D1_GRADIENT_STOP,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_CAP_STYLE_FLAT, D2D1_DASH_STYLE_DASH, D2D1_DASH_STYLE_DASH_DOT, D2D1_DASH_STYLE_DOT,
    D2D1_DASH_STYLE_SOLID, D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2, D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC, D2D1_LINE_JOIN_MITER,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES, D2D1_STROKE_STYLE_PROPERTIES1, ID2D1Bitmap1, ID2D1Brush,
    ID2D1DeviceContext, ID2D1Geometry, ID2D1PathGeometry, ID2D1RenderTarget, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_METRICS, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_GLYPH_METRICS, DWRITE_GLYPH_RUN, DWRITE_MEASURING_MODE_NATURAL, IDWriteFactory, IDWriteFontCollection,
    IDWriteFontFace,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICBitmapSource, IWICImagingFactory, WICBitmapDitherTypeNone,
    WICBitmapInterpolationModeHighQualityCubic, WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::core::{BOOL, HSTRING, Interface};
use windows_numerics::{Matrix3x2, Vector2};

use crate::lienzo::Pintor;
use crate::motor::MotorRender;

/// Una cara de letra ya abierta, con lo que se le pregunta a menudo.
pub struct Cara {
    cara: IDWriteFontFace,
    em: f32,
    /// Ascenso, descenso, hueco, posicion y grosor del subrayado, posicion
    /// y grosor del tachado, todo en fraccion de em.
    m: [f32; 7],
    /// char → (glifo, avance en em). 0 = no lo tiene.
    glifos: RefCell<HashMap<char, (u16, f32)>>,
}

impl Cara {
    fn glifo(&self, c: char) -> (u16, f32) {
        if let Some(g) = self.glifos.borrow().get(&c) {
            return *g;
        }
        let cp = c as u32;
        let mut g = [0u16; 1];
        // SAFETY: un codigo y un glifo de salida, locales; la cara vive.
        let _ = unsafe { self.cara.GetGlyphIndices(&cp, 1, g.as_mut_ptr()) };
        let mut avance = 0.0;
        if g[0] != 0 {
            let mut gm = [DWRITE_GLYPH_METRICS::default(); 1];
            // SAFETY: un glifo valido de esta cara y su metrica de salida.
            if unsafe { self.cara.GetDesignGlyphMetrics(g.as_ptr(), 1, gm.as_mut_ptr(), false) }.is_ok() {
                avance = gm[0].advanceWidth as f32 / self.em;
            }
        }
        self.glifos.borrow_mut().insert(c, (g[0], avance));
        (g[0], avance)
    }
}

/// Familia, negrita y cursiva.
type ClaveCara = (String, bool, bool);

/// **Las letras de Office**: buscar caras por nombre (con sustitutas),
/// medirlas y dar los glifos para pintarlas. Es el [`Medidor`] de la
/// maquetacion.
pub struct LetrasOffice {
    coleccion: IDWriteFontCollection,
    caras: RefCell<HashMap<ClaveCara, Option<Rc<Cara>>>>,
    /// Caras de reserva para los caracteres que la letra pedida no tiene.
    reserva: RefCell<Vec<Rc<Cara>>>,
    reserva_hecha: std::cell::Cell<bool>,
    existentes: RefCell<HashMap<String, bool>>,
}

/// Las sustitutas de las medidas de las letras de Office, y la que se usa
/// si tampoco esta.
fn sustitutas(familia: &str) -> &'static [&'static str] {
    match familia.to_ascii_lowercase().as_str() {
        "calibri" => &["Carlito"],
        "cambria" => &["Caladea"],
        "arial" | "helvetica" => &["Liberation Sans", "Arimo"],
        "times new roman" | "times" => &["Liberation Serif", "Tinos"],
        "courier new" | "courier" => &["Liberation Mono", "Cousine"],
        "aptos" | "aptos display" | "aptos narrow" => &["Calibri", "Carlito"],
        "calibri light" => &["Calibri", "Carlito"],
        "segoe ui light" | "segoe ui semibold" => &["Segoe UI"],
        "symbol" => &["Segoe UI Symbol"],
        _ => &[],
    }
}

fn es_serif(familia: &str) -> bool {
    let f = familia.to_ascii_lowercase();
    ["times", "serif", "georgia", "garamond", "cambria", "book", "roman", "palatino", "bodoni", "minion", "century", "baskerville", "mincho", "song"]
        .iter()
        .any(|k| f.contains(k))
        && !f.contains("sans")
}

impl LetrasOffice {
    pub fn nuevo(dwrite: &IDWriteFactory) -> Option<LetrasOffice> {
        let mut coleccion = None;
        // SAFETY: la factoria vive; `coleccion` es la salida local.
        unsafe { dwrite.GetSystemFontCollection(&mut coleccion, false).ok()? };
        Some(LetrasOffice {
            coleccion: coleccion?,
            caras: RefCell::new(HashMap::new()),
            reserva: RefCell::new(Vec::new()),
            reserva_hecha: std::cell::Cell::new(false),
            existentes: RefCell::new(HashMap::new()),
        })
    }

    fn abrir(&self, familia: &str, negrita: bool, cursiva: bool) -> Option<Rc<Cara>> {
        // SAFETY: consultas sobre la coleccion del sistema, viva; todas las
        // salidas son locales.
        unsafe {
            let (mut indice, mut existe) = (0u32, BOOL(0));
            self.coleccion.FindFamilyName(&HSTRING::from(familia), &mut indice, &mut existe).ok()?;
            if !existe.as_bool() {
                return None;
            }
            let fuente = self
                .coleccion
                .GetFontFamily(indice)
                .ok()?
                .GetFirstMatchingFont(
                    if negrita { DWRITE_FONT_WEIGHT_BOLD } else { DWRITE_FONT_WEIGHT_NORMAL },
                    DWRITE_FONT_STRETCH_NORMAL,
                    if cursiva { DWRITE_FONT_STYLE_ITALIC } else { DWRITE_FONT_STYLE_NORMAL },
                )
                .ok()?;
            let cara = fuente.CreateFontFace().ok()?;
            let mut m = DWRITE_FONT_METRICS::default();
            cara.GetMetrics(&mut m);
            let em = m.designUnitsPerEm.max(1) as f32;
            Some(Rc::new(Cara {
                cara,
                em,
                m: [
                    m.ascent as f32 / em,
                    m.descent as f32 / em,
                    m.lineGap.max(0) as f32 / em,
                    m.underlinePosition as f32 / em,
                    m.underlineThickness.max(1) as f32 / em,
                    m.strikethroughPosition as f32 / em,
                    m.strikethroughThickness.max(1) as f32 / em,
                ],
                glifos: RefCell::new(HashMap::new()),
            }))
        }
    }

    /// La cara de una letra: la pedida, una sustituta o la de reserva.
    pub fn cara(&self, letra: &Letra) -> Option<Rc<Cara>> {
        let familia = if letra.familia.trim().is_empty() { "Calibri" } else { letra.familia.trim() };
        let clave = (familia.to_string(), letra.negrita, letra.cursiva);
        if let Some(c) = self.caras.borrow().get(&clave) {
            return c.clone();
        }
        let mut c = self.abrir(familia, letra.negrita, letra.cursiva);
        if c.is_none() {
            for s in sustitutas(familia) {
                c = self.abrir(s, letra.negrita, letra.cursiva);
                if c.is_some() {
                    break;
                }
            }
        }
        if c.is_none() {
            let generica = if es_serif(familia) { "Times New Roman" } else { "Arial" };
            c = self.abrir(generica, letra.negrita, letra.cursiva).or_else(|| self.abrir("Segoe UI", letra.negrita, letra.cursiva));
        }
        self.caras.borrow_mut().insert(clave, c.clone());
        c
    }

    fn reservas(&self) -> std::cell::Ref<'_, Vec<Rc<Cara>>> {
        if !self.reserva_hecha.get() {
            self.reserva_hecha.set(true);
            let mut v = Vec::new();
            for f in [
                "Segoe UI",
                "Segoe UI Symbol",
                "Cambria Math",
                "Microsoft YaHei",
                "Yu Gothic",
                "Malgun Gothic",
                "Nirmala UI",
                "Ebrima",
                "Segoe UI Historic",
                "Segoe UI Emoji",
                "Arial Unicode MS",
            ] {
                if let Some(c) = self.abrir(f, false, false) {
                    v.push(c);
                }
            }
            *self.reserva.borrow_mut() = v;
        }
        self.reserva.borrow()
    }

    /// Para cada caracter: la cara que lo tiene, su glifo y su avance en em.
    pub fn glifos(&self, texto: &str, letra: &Letra, salida: &mut Vec<(Rc<Cara>, u16, f32)>) {
        let Some(principal) = self.cara(letra) else {
            return;
        };
        for c in texto.chars() {
            let (g, a) = principal.glifo(c);
            if g != 0 || c == '\u{200B}' || c == '\u{AD}' {
                salida.push((principal.clone(), g, a));
                continue;
            }
            // Los simbolos de Symbol/Wingdings en el area privada (F0xx) a
            // su equivalente Unicode si la letra no los tiene.
            let c2 = simbolo_a_unicode(c).unwrap_or(c);
            if c2 != c {
                let (g, a) = principal.glifo(c2);
                if g != 0 {
                    salida.push((principal.clone(), g, a));
                    continue;
                }
            }
            let reservas = self.reservas();
            let mut hecho = false;
            for r in reservas.iter() {
                let (g, a) = r.glifo(c2);
                if g != 0 {
                    salida.push((r.clone(), g, a));
                    hecho = true;
                    break;
                }
            }
            if !hecho {
                salida.push((principal.clone(), 0, principal.glifo('?').1));
            }
        }
    }
}

/// Los simbolos mas comunes de Symbol y Wingdings (en F0xx) a Unicode.
fn simbolo_a_unicode(c: char) -> Option<char> {
    let n = c as u32;
    if !(0xF000..=0xF0FF).contains(&n) {
        return None;
    }
    Some(match n - 0xF000 {
        0xB7 => '•',
        0xA7 => '▪',
        0xA8 => '◆',
        0x6E => '■',
        0x71 => '❑',
        0x76 => '❖',
        0xD8 => '➢',
        0xFC => '✓',
        0xFB => '✗',
        0x6F => '○',
        0x6C => '●',
        0xE0 => '→',
        0xE8 => '➔',
        0x2D => '−',
        b if (0x20..0x7F).contains(&b) => char::from_u32(b)?,
        _ => '•',
    })
}

impl Medidor for LetrasOffice {
    fn avances(&self, texto: &str, letra: &Letra, salida: &mut Vec<f32>) {
        let mut g = Vec::with_capacity(texto.len());
        self.glifos(texto, letra, &mut g);
        if g.is_empty() {
            salida.extend(texto.chars().map(|_| letra.tam * 0.5));
            return;
        }
        salida.extend(g.iter().map(|(_, _, a)| a * letra.tam));
    }

    fn existe(&self, familia: &str) -> bool {
        if let Some(e) = self.existentes.borrow().get(familia) {
            return *e;
        }
        let (mut indice, mut existe) = (0u32, BOOL(0));
        // SAFETY: consulta sobre la coleccion viva con salidas locales.
        let ok = unsafe { self.coleccion.FindFamilyName(&HSTRING::from(familia), &mut indice, &mut existe) }.is_ok() && existe.as_bool();
        self.existentes.borrow_mut().insert(familia.to_string(), ok);
        ok
    }

    fn metricas(&self, letra: &Letra) -> Metricas {
        match self.cara(letra) {
            Some(c) => Metricas { ascenso: c.m[0] * letra.tam, descenso: c.m[1] * letra.tam, hueco: c.m[2] * letra.tam },
            None => Metricas { ascenso: letra.tam * 0.9, descenso: letra.tam * 0.25, hueco: 0.0 },
        }
    }
}

/// **El pintor de paginas de Office.** Guarda lo que cuesta crear (las
/// imagenes decodificadas, los pinceles, los estilos de trazo); vive lo que
/// viva el `MotorRender` con que se creo.
pub struct PintorOffice {
    pub letras: LetrasOffice,
    wic: Option<IWICImagingFactory>,
    imagenes: RefCell<HashMap<Arc<str>, Option<ID2D1Bitmap1>>>,
    pinceles: RefCell<HashMap<[u8; 4], ID2D1SolidColorBrush>>,
    trazos: RefCell<HashMap<u8, Option<ID2D1StrokeStyle>>>,
}

impl PintorOffice {
    pub fn nuevo(motor: &MotorRender) -> Option<PintorOffice> {
        // SAFETY: crear la fabrica de WIC no tiene precondiciones (COM ya
        // iniciado por quien llama; si no, queda sin imagenes).
        let wic: Option<IWICImagingFactory> = unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok() };
        Some(PintorOffice {
            letras: LetrasOffice::nuevo(motor.dwrite())?,
            wic,
            imagenes: RefCell::new(HashMap::new()),
            pinceles: RefCell::new(HashMap::new()),
            trazos: RefCell::new(HashMap::new()),
        })
    }

    /// Olvida las imagenes ya subidas (al cambiar de documento).
    pub fn olvidar_imagenes(&self) {
        self.imagenes.borrow_mut().clear();
    }

    /// Pinta `pagina` con su esquina en `origen` (pixeles del destino) a
    /// `escala` pixeles por punto. `total` es el numero de paginas para los
    /// campos `NUMPAGES`. Hay que llamarlo dentro de `MotorRender::dibujar`.
    pub fn pintar(&self, p: &Pintor, pagina: &Pagina, recursos: &Recursos, escala: f32, origen: (f32, f32), total: u32) {
        let c = p.motor.contexto();
        let mut previa = Matrix3x2::default();
        // SAFETY: GetTransform rellena la matriz local; el contexto vive.
        unsafe { c.GetTransform(&mut previa) };
        let base = componer(&Matrix3x2 { M11: escala, M12: 0.0, M21: 0.0, M22: escala, M31: origen.0, M32: origen.1 }, &previa);
        // SAFETY: dentro del fotograma; se restaura al acabar.
        unsafe { c.SetTransform(&base) };
        let hoja = D2D_RECT_F { left: 0.0, top: 0.0, right: pagina.ancho, bottom: pagina.alto };
        match &pagina.fondo {
            Some(r) => self.rellenar_rect(c, hoja, r),
            None => {
                if let Some(b) = self.pincel(c, ColorO::BLANCO) {
                    // SAFETY: dentro del fotograma.
                    unsafe { c.FillRectangle(&hoja, &b) };
                }
            }
        }
        // Lo que se salga de la hoja no se ve.
        // SAFETY: dentro del fotograma; se quita al acabar.
        unsafe { c.PushAxisAlignedClip(&hoja, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE) };
        let ctx = Ctx { c, recursos, total };
        for e in &pagina.elementos {
            self.elemento(&ctx, e, &base);
        }
        // SAFETY: empareja el Push de arriba y restaura la transformada.
        unsafe {
            c.PopAxisAlignedClip();
            c.SetTransform(&previa);
        }
    }

    fn pincel(&self, c: &ID2D1DeviceContext, col: ColorO) -> Option<ID2D1SolidColorBrush> {
        let k = [col.r, col.g, col.b, col.a];
        if let Some(b) = self.pinceles.borrow().get(&k) {
            return Some(b.clone());
        }
        // SAFETY: dentro del fotograma; el color es local.
        let b = unsafe { c.CreateSolidColorBrush(&d2d(col), None) }.ok()?;
        self.pinceles.borrow_mut().insert(k, b.clone());
        Some(b)
    }

    fn estilo_trazo(&self, c: &ID2D1DeviceContext, d: Discontinuo) -> Option<ID2D1StrokeStyle> {
        let k = d as u8;
        if let Some(s) = self.trazos.borrow().get(&k) {
            return s.clone();
        }
        let dash = match d {
            Discontinuo::Solida => D2D1_DASH_STYLE_SOLID,
            Discontinuo::Rayas => D2D1_DASH_STYLE_DASH,
            Discontinuo::Puntos => D2D1_DASH_STYLE_DOT,
            Discontinuo::RayaPunto => D2D1_DASH_STYLE_DASH_DOT,
        };
        let props = D2D1_STROKE_STYLE_PROPERTIES1 {
            startCap: D2D1_CAP_STYLE_FLAT,
            endCap: D2D1_CAP_STYLE_FLAT,
            dashCap: D2D1_CAP_STYLE_FLAT,
            lineJoin: D2D1_LINE_JOIN_MITER,
            miterLimit: 10.0,
            dashStyle: dash,
            dashOffset: 0.0,
            ..Default::default()
        };
        // SAFETY: la fabrica del contexto vive; las propiedades son locales.
        let s = unsafe {
            c.GetFactory()
                .ok()
                .and_then(|f| f.cast::<windows::Win32::Graphics::Direct2D::ID2D1Factory1>().ok())
                .and_then(|f| f.CreateStrokeStyle(&props, None).ok())
                .and_then(|s| s.cast::<ID2D1StrokeStyle>().ok())
        };
        self.trazos.borrow_mut().insert(k, s.clone());
        s
    }

    fn pincel_relleno(&self, c: &ID2D1DeviceContext, r: &Relleno, caja: D2D_RECT_F) -> Option<ID2D1Brush> {
        let rt: &ID2D1RenderTarget = c;
        match r {
            Relleno::Solido(col) => self.pincel(c, *col).and_then(|b| b.cast().ok()),
            Relleno::Lineal { paradas, .. } if paradas.len() == 1 => self.pincel(c, paradas[0].1).and_then(|b| b.cast().ok()),
            Relleno::Radial { paradas } if paradas.len() == 1 => self.pincel(c, paradas[0].1).and_then(|b| b.cast().ok()),
            Relleno::Lineal { paradas, angulo } => {
                let stops: Vec<D2D1_GRADIENT_STOP> = paradas.iter().map(|(p, col)| D2D1_GRADIENT_STOP { position: *p, color: d2d(*col) }).collect();
                // SAFETY: dentro del fotograma; las paradas se copian.
                let col = unsafe { rt.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP) }.ok()?;
                let (w, h) = (caja.right - caja.left, caja.bottom - caja.top);
                let (cx, cy) = (caja.left + w / 2.0, caja.top + h / 2.0);
                let (s, co) = angulo.to_radians().sin_cos();
                let mitad = (w * co.abs() + h * s.abs()) / 2.0;
                let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                    startPoint: Vector2 { X: cx - co * mitad, Y: cy - s * mitad },
                    endPoint: Vector2 { X: cx + co * mitad, Y: cy + s * mitad },
                };
                // SAFETY: propiedades y coleccion vivas.
                unsafe { rt.CreateLinearGradientBrush(&props, None, &col) }.ok().and_then(|b| b.cast().ok())
            }
            Relleno::Radial { paradas } => {
                let stops: Vec<D2D1_GRADIENT_STOP> = paradas.iter().map(|(p, col)| D2D1_GRADIENT_STOP { position: *p, color: d2d(*col) }).collect();
                // SAFETY: dentro del fotograma; las paradas se copian.
                let col = unsafe { rt.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP) }.ok()?;
                let (w, h) = (caja.right - caja.left, caja.bottom - caja.top);
                let props = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
                    center: Vector2 { X: caja.left + w / 2.0, Y: caja.top + h / 2.0 },
                    gradientOriginOffset: Vector2 { X: 0.0, Y: 0.0 },
                    radiusX: w / 2.0 * std::f32::consts::SQRT_2,
                    radiusY: h / 2.0 * std::f32::consts::SQRT_2,
                };
                // SAFETY: propiedades y coleccion vivas.
                unsafe { rt.CreateRadialGradientBrush(&props, None, &col) }.ok().and_then(|b| b.cast().ok())
            }
        }
    }

    fn rellenar_rect(&self, c: &ID2D1DeviceContext, r: D2D_RECT_F, relleno: &Relleno) {
        if let Some(b) = self.pincel_relleno(c, relleno, r) {
            // SAFETY: dentro del fotograma.
            unsafe { c.FillRectangle(&r, &b) };
        }
    }

    fn elemento(&self, ctx: &Ctx, e: &Elemento, transformada: &Matrix3x2) {
        let c = ctx.c;
        match e {
            Elemento::Texto(t) => self.texto(ctx, t),
            Elemento::Rect { rect, relleno, borde } => {
                let r = D2D_RECT_F { left: rect.x, top: rect.y, right: rect.x + rect.ancho, bottom: rect.y + rect.alto };
                if let Some(rel) = relleno {
                    self.rellenar_rect(c, r, rel);
                }
                if let Some(t) = borde
                    && let Some(b) = self.pincel(c, t.color)
                {
                    let s = self.estilo_trazo(c, t.discontinuo);
                    // SAFETY: dentro del fotograma.
                    unsafe { c.DrawRectangle(&r, &b, t.grosor, s.as_ref()) };
                }
            }
            Elemento::Linea { desde, hasta, trazo } => self.linea(c, *desde, *hasta, trazo),
            Elemento::Imagen { id, rect, recorte, opacidad } => self.imagen(ctx, id, *rect, *recorte, *opacidad),
            Elemento::Trayecto { figuras, relleno, borde } => self.trayecto(c, figuras, relleno.as_ref(), borde.as_ref()),
            Elemento::Grupo { transformada: t, recorte, elementos } => {
                let propia = Matrix3x2 { M11: t[0], M12: t[1], M21: t[2], M22: t[3], M31: t[4], M32: t[5] };
                let nueva = componer(&propia, transformada);
                // SAFETY: dentro del fotograma; se restaura abajo.
                unsafe { c.SetTransform(&nueva) };
                if let Some(r) = recorte {
                    let d = D2D_RECT_F { left: r.x, top: r.y, right: r.x + r.ancho, bottom: r.y + r.alto };
                    // SAFETY: dentro del fotograma; se quita abajo.
                    unsafe { c.PushAxisAlignedClip(&d, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE) };
                }
                for h in elementos {
                    self.elemento(ctx, h, &nueva);
                }
                // SAFETY: empareja lo de arriba.
                unsafe {
                    if recorte.is_some() {
                        c.PopAxisAlignedClip();
                    }
                    c.SetTransform(transformada);
                }
            }
        }
    }

    fn linea(&self, c: &ID2D1DeviceContext, a: (f32, f32), b: (f32, f32), t: &Trazo) {
        let Some(p) = self.pincel(c, t.color) else { return };
        let s = self.estilo_trazo(c, t.discontinuo);
        // SAFETY: dentro del fotograma.
        unsafe { c.DrawLine(Vector2 { X: a.0, Y: a.1 }, Vector2 { X: b.0, Y: b.1 }, &p, t.grosor, s.as_ref()) };
    }

    fn geometria(&self, c: &ID2D1DeviceContext, figuras: &[&Figura], rellenas: bool) -> Option<ID2D1PathGeometry> {
        let v = |p: (f32, f32)| Vector2 { X: p.0, Y: p.1 };
        // SAFETY: la geometria se crea, se rellena y se cierra aqui; si algo
        // falla se descarta.
        unsafe {
            let g = c.GetFactory().ok()?.CreatePathGeometry().ok()?;
            let s = g.Open().ok()?;
            s.SetFillMode(D2D1_FILL_MODE_ALTERNATE);
            for fig in figuras {
                s.BeginFigure(v(fig.inicio), if rellenas { D2D1_FIGURE_BEGIN_FILLED } else { D2D1_FIGURE_BEGIN_HOLLOW });
                for t in &fig.tramos {
                    match *t {
                        Tramo::Linea(x, y) => s.AddLine(Vector2 { X: x, Y: y }),
                        Tramo::Bezier(a, b, d) => s.AddBezier(&D2D1_BEZIER_SEGMENT { point1: v(a), point2: v(b), point3: v(d) }),
                    }
                }
                s.EndFigure(if fig.cerrada { D2D1_FIGURE_END_CLOSED } else { D2D1_FIGURE_END_OPEN });
            }
            s.Close().ok()?;
            Some(g)
        }
    }

    fn trayecto(&self, c: &ID2D1DeviceContext, figuras: &[Figura], relleno: Option<&Relleno>, borde: Option<&Trazo>) {
        if let Some(r) = relleno {
            let llenas: Vec<&Figura> = figuras.iter().filter(|f| f.rellenar && f.sombra == Sombra::Ninguna).collect();
            let todas: Vec<&Figura> = figuras.iter().filter(|f| f.rellenar).collect();
            // La caja del degradado es la de toda la forma, no la de cada grupo.
            let caja = self
                .geometria(c, &todas, true)
                .and_then(|g| g.cast::<ID2D1Geometry>().ok())
                // SAFETY: geometria viva; Bounds rellena el rect local.
                .map(|geo| unsafe { geo.GetBounds(None) }.unwrap_or_default());
            if let Some(caja) = caja
                && let Some(b) = self.pincel_relleno(c, r, caja)
            {
                if !llenas.is_empty()
                    && let Some(g) = self.geometria(c, &llenas, true)
                {
                    let geo: ID2D1Geometry = g.cast().expect("una geometria de trayecto es una geometria");
                    // SAFETY: dentro del fotograma.
                    unsafe { c.FillGeometry(&geo, &b, None) };
                }
                // Las caras tenidas (cubo, bisel) y las que van aparte: cada
                // clase en su geometria, con el relleno y encima el tinte.
                for (s, tinte) in [
                    (Sombra::Aparte, None),
                    (Sombra::ClaraSuave, Some(ColorO { r: 255, g: 255, b: 255, a: 46 })),
                    (Sombra::Clara, Some(ColorO { r: 255, g: 255, b: 255, a: 92 })),
                    (Sombra::OscuraSuave, Some(ColorO { r: 0, g: 0, b: 0, a: 46 })),
                    (Sombra::Oscura, Some(ColorO { r: 0, g: 0, b: 0, a: 102 })),
                ] {
                    let grupo: Vec<&Figura> = figuras.iter().filter(|f| f.rellenar && f.sombra == s).collect();
                    if grupo.is_empty() {
                        continue;
                    }
                    let Some(g) = self.geometria(c, &grupo, true) else { continue };
                    let geo: ID2D1Geometry = g.cast().expect("una geometria de trayecto es una geometria");
                    // SAFETY: dentro del fotograma.
                    unsafe { c.FillGeometry(&geo, &b, None) };
                    if let Some(t) = tinte
                        && let Some(p) = self.pincel(c, t)
                    {
                        // SAFETY: dentro del fotograma.
                        unsafe { c.FillGeometry(&geo, &p, None) };
                    }
                }
            }
        }
        if let Some(t) = borde {
            let trazadas: Vec<&Figura> = figuras.iter().filter(|f| f.trazar).collect();
            if !trazadas.is_empty()
                && let Some(g) = self.geometria(c, &trazadas, false)
                && let Some(p) = self.pincel(c, t.color)
            {
                let s = self.estilo_trazo(c, t.discontinuo);
                // SAFETY: dentro del fotograma.
                unsafe { c.DrawGeometry(&g, &p, t.grosor, s.as_ref()) };
            }
        }
    }

    fn texto(&self, ctx: &Ctx, t: &Texto) {
        let c = ctx.c;
        let total;
        let texto: &str = match t.campo {
            Some(Campo::TotalPaginas) => {
                total = ctx.total.to_string();
                &total
            }
            _ => &t.texto,
        };
        let mut glifos = Vec::with_capacity(texto.len());
        self.letras.glifos(texto, &t.letra, &mut glifos);
        let Some(pincel) = self.pincel(c, t.color) else { return };
        let tam = t.letra.tam;
        let mut x = t.x;
        let chars: Vec<char> = texto.chars().collect();
        let mut i = 0;
        while i < glifos.len() {
            let cara = glifos[i].0.clone();
            let mut indices = Vec::new();
            let mut avances = Vec::new();
            let inicio = x;
            while i < glifos.len() && Rc::ptr_eq(&glifos[i].0, &cara) {
                let (_, g, a) = &glifos[i];
                let mut av = a * tam + t.espaciado;
                if chars.get(i).is_some_and(|ch| *ch == ' ') {
                    av += t.extra_espacio;
                }
                indices.push(*g);
                avances.push(av);
                x += av;
                i += 1;
            }
            let corrida = DWRITE_GLYPH_RUN {
                fontFace: std::mem::ManuallyDrop::new(Some(cara.cara.clone())),
                fontEmSize: tam,
                glyphCount: indices.len() as u32,
                glyphIndices: indices.as_ptr(),
                glyphAdvances: avances.as_ptr(),
                glyphOffsets: std::ptr::null(),
                isSideways: BOOL(0),
                bidiLevel: 0,
            };
            // SAFETY: los arrays viven hasta despues de la llamada; la cara
            // se suelta abajo (ManuallyDrop).
            unsafe { c.DrawGlyphRun(Vector2 { X: inicio, Y: t.y }, &corrida, None, &pincel, DWRITE_MEASURING_MODE_NATURAL) };
            drop(std::mem::ManuallyDrop::into_inner(corrida.fontFace));
        }
        // Subrayado y tachado, con las medidas de la letra.
        let ancho = if t.campo == Some(Campo::TotalPaginas) { x - t.x } else { t.ancho.max(x - t.x - t.extra_espacio) };
        let cara = glifos.first().map(|g| g.0.clone());
        let m = cara.map(|c| c.m).unwrap_or([0.9, 0.25, 0.0, -0.1, 0.05, 0.3, 0.05]);
        if let Some(s) = t.subrayado {
            let fin = texto.trim_end().chars().count();
            let ancho_sub = if fin < chars.len() {
                // Sin los espacios del final.
                let quitar: f32 = glifos[fin.min(glifos.len())..].iter().map(|(_, _, a)| a * tam + t.espaciado + t.extra_espacio).sum();
                (ancho - quitar).max(0.0)
            } else {
                ancho
            };
            let grosor = (m[4] * tam).max(0.5) * if s == Subrayado::Grueso { 2.0 } else { 1.0 };
            let y = t.y - m[3] * tam;
            let trazo = Trazo {
                color: t.color,
                grosor,
                discontinuo: match s {
                    Subrayado::Puntos => Discontinuo::Puntos,
                    Subrayado::Rayas => Discontinuo::Rayas,
                    _ => Discontinuo::Solida,
                },
            };
            self.linea(c, (t.x, y), (t.x + ancho_sub, y), &trazo);
            if s == Subrayado::Doble {
                self.linea(c, (t.x, y + grosor * 2.0), (t.x + ancho_sub, y + grosor * 2.0), &trazo);
            }
        }
        if t.tachado {
            let y = t.y - m[5] * tam;
            self.linea(c, (t.x, y), (t.x + ancho, y), &Trazo::solido(t.color, (m[6] * tam).max(0.5)));
        }
    }

    fn imagen(&self, ctx: &Ctx, id: &Arc<str>, rect: pixpin_office::pagina::Rect, recorte: Option<[f32; 4]>, opacidad: f32) {
        let c = ctx.c;
        let bitmap = {
            let cache = self.imagenes.borrow();
            cache.get(id).cloned()
        };
        let bitmap = match bitmap {
            Some(b) => b,
            None => {
                let b = ctx.recursos.bytes(id).and_then(|bytes| self.decodificar(c, &bytes));
                self.imagenes.borrow_mut().insert(id.clone(), b.clone());
                b
            }
        };
        let Some(b) = bitmap else {
            // Una imagen que no se sabe leer (EMF, WMF...): un marco gris
            // claro en su sitio.
            if let Some(p) = self.pincel(c, ColorO::rgb(0xF0, 0xF0, 0xF0)) {
                let r = D2D_RECT_F { left: rect.x, top: rect.y, right: rect.x + rect.ancho, bottom: rect.y + rect.alto };
                // SAFETY: dentro del fotograma.
                unsafe { c.FillRectangle(&r, &p) };
            }
            return;
        };
        // SAFETY: GetSize es una consulta del bitmap vivo.
        let tam = unsafe { b.GetSize() };
        let (w, h) = (tam.width, tam.height);
        let mut destino = D2D_RECT_F { left: rect.x, top: rect.y, right: rect.x + rect.ancho, bottom: rect.y + rect.alto };
        let mut fuente = D2D_RECT_F { left: 0.0, top: 0.0, right: w, bottom: h };
        if let Some([l, t, r, bo]) = recorte {
            // La parte de la imagen que va en la caja; lo que cae fuera de
            // la imagen (recorte negativo) deja hueco.
            let (x0, x1) = (l * w, (1.0 - r) * w);
            let (y0, y1) = (t * h, (1.0 - bo) * h);
            if x1 - x0 > 0.0 && y1 - y0 > 0.0 {
                let sx = rect.ancho / (x1 - x0);
                let sy = rect.alto / (y1 - y0);
                let (fx0, fx1) = (x0.max(0.0), x1.min(w));
                let (fy0, fy1) = (y0.max(0.0), y1.min(h));
                destino = D2D_RECT_F {
                    left: rect.x + (fx0 - x0) * sx,
                    top: rect.y + (fy0 - y0) * sy,
                    right: rect.x + (fx1 - x0) * sx,
                    bottom: rect.y + (fy1 - y0) * sy,
                };
                fuente = D2D_RECT_F { left: fx0, top: fy0, right: fx1, bottom: fy1 };
            }
        }
        // SAFETY: dentro del fotograma; bitmap y rectangulos vivos.
        unsafe { c.DrawBitmap(&b, Some(&destino), opacidad, D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC, Some(&fuente), None) };
    }

    fn decodificar(&self, c: &ID2D1DeviceContext, bytes: &[u8]) -> Option<ID2D1Bitmap1> {
        let wic = self.wic.as_ref()?;
        // SAFETY: la cadena de WIC se crea y se usa aqui; los bytes viven
        // mientras dura la decodificacion (el bitmap de D2D copia los
        // pixeles).
        unsafe {
            let flujo = wic.CreateStream().ok()?;
            flujo.InitializeFromMemory(bytes).ok()?;
            let deco = wic.CreateDecoderFromStream(&flujo, std::ptr::null(), WICDecodeMetadataCacheOnDemand).ok()?;
            let marco = deco.GetFrame(0).ok()?;
            let (mut w, mut h) = (0u32, 0u32);
            marco.GetSize(&mut w, &mut h).ok()?;
            let mut fuente: IWICBitmapSource = marco.cast().ok()?;
            // Las enormes se reducen: D2D no pasa de 16384 y una foto de 40
            // megapixeles en una pagina no necesita mas que esto.
            let tope = 4096u32;
            if w.max(h) > tope {
                let k = tope as f32 / w.max(h) as f32;
                let esc = wic.CreateBitmapScaler().ok()?;
                esc.Initialize(&fuente, ((w as f32 * k) as u32).max(1), ((h as f32 * k) as u32).max(1), WICBitmapInterpolationModeHighQualityCubic)
                    .ok()?;
                fuente = esc.cast().ok()?;
            }
            let conv = wic.CreateFormatConverter().ok()?;
            conv.Initialize(&fuente, &GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeMedianCut).ok()?;
            c.CreateBitmapFromWicBitmap(&conv, None).ok()
        }
    }
}

struct Ctx<'a> {
    c: &'a ID2D1DeviceContext,
    recursos: &'a Recursos,
    total: u32,
}

fn d2d(c: ColorO) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r: c.r as f32 / 255.0, g: c.g as f32 / 255.0, b: c.b as f32 / 255.0, a: c.a as f32 / 255.0 }
}

/// `a` y despues `b` (como en Direct2D: el punto se multiplica por `a`
/// primero).
fn componer(a: &Matrix3x2, b: &Matrix3x2) -> Matrix3x2 {
    Matrix3x2 {
        M11: a.M11 * b.M11 + a.M12 * b.M21,
        M12: a.M11 * b.M12 + a.M12 * b.M22,
        M21: a.M21 * b.M11 + a.M22 * b.M21,
        M22: a.M21 * b.M12 + a.M22 * b.M22,
        M31: a.M31 * b.M11 + a.M32 * b.M21 + b.M31,
        M32: a.M31 * b.M12 + a.M32 * b.M22 + b.M32,
    }
}
