//! El pintor seguro: la unica cara de Direct2D que ven las capas sin unsafe.
//!
//! `apps/pixpin` es `forbid(unsafe_code)` y aun asi orquesta todo el dibujo
//! del overlay. Este modulo le da primitivas seguras —rellenar, trazar,
//! bitmap, texto— y encierra el protocolo SetTarget/BeginDraw/EndDraw en
//! una unica funcion con clausura, donde no se puede olvidar ningun paso.

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_CAP_STYLE_FLAT, D2D1_DASH_STYLE_DASH,
    D2D1_INTERPOLATION_MODE_LINEAR, D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_LINE_JOIN_MITER,
    D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES1, ID2D1Bitmap1, ID2D1PathGeometry1,
    ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_METRICS, DWRITE_TEXT_RANGE,
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
    IDWriteFactory, IDWriteTextLayout,
};
use windows::core::Interface;
use windows::core::w;
use windows_numerics::Vector2;

use crate::motor::{Color, ErrorRender, MotorRender};

/// Rectangulo en coordenadas de dibujo (pixeles del destino, coma flotante).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

impl RectF {
    pub fn desde(r: pixpin_geom_compat::Rect) -> RectF {
        RectF {
            x: r.0 as f32,
            y: r.1 as f32,
            ancho: r.2 as f32,
            alto: r.3 as f32,
        }
    }

    fn a_d2d(self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.x,
            top: self.y,
            right: self.x + self.ancho,
            bottom: self.y + self.alto,
        }
    }
}

/// Compatibilidad sin dependencia: (x, y, ancho, alto). `pixpin-render` no
/// depende de `pixpin-geom` a proposito (son de la misma capa L0/L1 en
/// espiritu distinto); quien tiene un Rect de geom lo convierte en tupla.
pub mod pixpin_geom_compat {
    pub type Rect = (i32, i32, u32, u32);
}

impl MotorRender {
    /// Dibuja sobre `destino` con el protocolo completo encerrado: SetTarget,
    /// BeginDraw, la clausura, EndDraw y SetTarget(None). El error de EndDraw
    /// se devuelve; el destino queda siempre desligado.
    pub fn dibujar(
        &self,
        destino: &ID2D1Bitmap1,
        pintar: impl FnOnce(&Pintor),
    ) -> Result<(), ErrorRender> {
        let c = self.contexto();
        // SAFETY: protocolo documentado de D2D sobre un contexto vivo; el
        // SetTarget(None) final corre tanto en exito como en error de
        // EndDraw, porque va antes del `?`.
        unsafe {
            c.SetTarget(destino);
            c.BeginDraw();
            // El contexto es compartido entre ventanas: un desplazamiento
            // que dejara el fotograma anterior no debe contaminar este.
            c.SetTransform(&windows_numerics::Matrix3x2::identity());
        }
        let pintor = Pintor { motor: self };
        pintar(&pintor);
        // SAFETY: cierra el BeginDraw de arriba; None desliga el destino.
        let fin = unsafe { c.EndDraw(None, None) };
        // SAFETY: desligar siempre, tambien si EndDraw fallo.
        unsafe { c.SetTarget(None) };
        fin?;
        Ok(())
    }
}

/// Primitivas de dibujo validas SOLO dentro de `MotorRender::dibujar`.
pub struct Pintor<'a> {
    motor: &'a MotorRender,
}

/// Estilo de un tramo de texto. Sin nada marcado es el texto normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EstiloTexto {
    pub negrita: bool,
    pub cursiva: bool,
    /// Monoespaciada (codigo).
    pub mono: bool,
}

/// Un tramo de texto con estilo. Las posiciones van en unidades UTF-16,
/// que es lo que cuenta DirectWrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tramo {
    pub inicio: u32,
    pub longitud: u32,
    pub estilo: EstiloTexto,
}

/// Construye la disposicion DirectWrite de un texto: partido a `ancho_max`
/// (o en una sola linea con puntos suspensivos si `una_linea`), con los
/// tramos de estilo aplicados. Es la unica fabrica de disposiciones: la
/// usan el pintor (dentro del fotograma) y el motor (para medir fuera).
pub(crate) fn disposicion_dwrite(
    dwrite: &IDWriteFactory,
    texto: &str,
    tam: f32,
    ancho_max: f32,
    tramos: &[Tramo],
    una_linea: bool,
) -> Option<(IDWriteTextLayout, f32, f32)> {
    let contenido: Vec<u16> = texto.encode_utf16().collect();
    // SAFETY: cadenas constantes terminadas en cero; la disposicion copia
    // el texto y no retiene nada del llamante; los rangos se limitan al
    // texto (DirectWrite recorta los que se pasen).
    unsafe {
        let formato = dwrite
            .CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                tam,
                w!("es-ES"),
            )
            .ok()?;
        let disposicion = dwrite
            .CreateTextLayout(&contenido, &formato, ancho_max, f32::MAX)
            .ok()?;
        if una_linea {
            disposicion
                .SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)
                .ok()?;
            let signo = dwrite.CreateEllipsisTrimmingSign(&formato).ok()?;
            let recorte = DWRITE_TRIMMING {
                granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                delimiter: 0,
                delimiterCount: 0,
            };
            disposicion.SetTrimming(&recorte, &signo).ok()?;
        }
        for t in tramos {
            let rango = DWRITE_TEXT_RANGE {
                startPosition: t.inicio,
                length: t.longitud,
            };
            if t.estilo.negrita {
                disposicion
                    .SetFontWeight(DWRITE_FONT_WEIGHT_BOLD, rango)
                    .ok()?;
            }
            if t.estilo.cursiva {
                disposicion
                    .SetFontStyle(DWRITE_FONT_STYLE_ITALIC, rango)
                    .ok()?;
            }
            if t.estilo.mono {
                disposicion.SetFontFamilyName(w!("Consolas"), rango).ok()?;
            }
        }
        let mut metricas = DWRITE_TEXT_METRICS::default();
        disposicion.GetMetrics(&mut metricas).ok()?;
        Some((disposicion, metricas.width, metricas.height))
    }
}

impl Pintor<'_> {
    fn pincel(&self, color: Color) -> Option<ID2D1SolidColorBrush> {
        // SAFETY: crear un pincel sobre el contexto vivo no tiene mas
        // precondiciones; se usa y se suelta dentro del fotograma.
        unsafe {
            self.motor
                .contexto()
                .CreateSolidColorBrush(&color.a_d2d(), None)
                .ok()
        }
    }

    pub fn limpiar(&self, color: Color) {
        // SAFETY: Clear dentro de BeginDraw/EndDraw (lo garantiza `dibujar`).
        unsafe { self.motor.contexto().Clear(Some(&color.a_d2d())) };
    }

    /// Limpia a transparente total: el fondo de una ventana de composicion.
    pub fn limpiar_transparente(&self) {
        self.limpiar(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
    }

    /// Desplaza el origen de todo lo que se pinte a partir de aqui. Lo usa
    /// el pin cuando su ventana es mas pequena que su contenido (recortada
    /// al escritorio): el contenido se dibuja donde le toca y la ventana
    /// ensena la parte visible.
    pub fn desplazar(&self, dx: f32, dy: f32) {
        // SAFETY: SetTransform sobre el contexto vivo, dentro del fotograma.
        unsafe {
            self.motor
                .contexto()
                .SetTransform(&windows_numerics::Matrix3x2::translation(dx, dy));
        }
    }

    /// Ejecuta `pintar` con todo lo que dibuje recortado a `r`. Lo que caiga
    /// fuera no se rasteriza siquiera, asi que sirve para no pagar el relleno
    /// de una zona que luego va a quedar tapada (la sombra bajo la tarjeta).
    pub fn con_recorte(&self, r: RectF, pintar: impl FnOnce(&Pintor)) {
        // SAFETY: Push/Pop emparejados dentro del fotograma; el modo de
        // suavizado por defecto (aliased) es el mas barato y basta para un
        // recorte con bordes rectos.
        unsafe {
            self.motor
                .contexto()
                .PushAxisAlignedClip(&r.a_d2d(), D2D1_ANTIALIAS_MODE_ALIASED);
        }
        pintar(self);
        // SAFETY: cierra el Push de arriba, siempre.
        unsafe { self.motor.contexto().PopAxisAlignedClip() };
    }

    /// Dibuja girado en cuartos de vuelta y volteado alrededor de `centro`,
    /// encima del desplazamiento `base` que ya tuviera la escena. Al salir
    /// deja solo `base`, para que lo de despues no herede el giro.
    ///
    /// Se hace con una transformada y no rotando los pixeles: girar y
    /// volver a girar devuelve la imagen exacta, y una imagen grande no se
    /// copia en memoria cada vez.
    pub fn con_giro(
        &self,
        base: (f32, f32),
        centro: (f32, f32),
        cuartos: u8,
        volteo_h: bool,
        volteo_v: bool,
        pintar: impl FnOnce(&Pintor),
    ) {
        let (cos, sin) = match cuartos % 4 {
            0 => (1.0f32, 0.0f32),
            1 => (0.0, 1.0),
            2 => (-1.0, 0.0),
            _ => (0.0, -1.0),
        };
        let (fx, fy) = (
            if volteo_h { -1.0f32 } else { 1.0 },
            if volteo_v { -1.0f32 } else { 1.0 },
        );
        // Voltear y luego girar, todo alrededor del centro; el
        // desplazamiento base se suma al final.
        let (m11, m12) = (fx * cos, fx * sin);
        let (m21, m22) = (-fy * sin, fy * cos);
        let m = windows_numerics::Matrix3x2 {
            M11: m11,
            M12: m12,
            M21: m21,
            M22: m22,
            M31: centro.0 - (centro.0 * m11 + centro.1 * m21) + base.0,
            M32: centro.1 - (centro.0 * m12 + centro.1 * m22) + base.1,
        };
        // SAFETY: SetTransform sobre el contexto vivo, dentro del fotograma;
        // se restaura siempre justo despues.
        unsafe { self.motor.contexto().SetTransform(&m) };
        pintar(self);
        self.desplazar(base.0, base.1);
    }

    pub fn rellenar(&self, r: RectF, color: Color) {
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; pincel y contexto vivos.
            unsafe { self.motor.contexto().FillRectangle(&r.a_d2d(), &p) };
        }
    }

    pub fn rellenar_redondeado(&self, r: RectF, radio: f32, color: Color) {
        if let Some(p) = self.pincel(color) {
            let rr = D2D1_ROUNDED_RECT {
                rect: r.a_d2d(),
                radiusX: radio,
                radiusY: radio,
            };
            // SAFETY: dentro del fotograma; pincel y contexto vivos.
            unsafe { self.motor.contexto().FillRoundedRectangle(&rr, &p) };
        }
    }

    pub fn trazar(&self, r: RectF, grosor: f32, color: Color) {
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; pincel y contexto vivos.
            unsafe {
                self.motor
                    .contexto()
                    .DrawRectangle(&r.a_d2d(), &p, grosor, None)
            };
        }
    }

    pub fn trazar_discontinuo(&self, r: RectF, grosor: f32, color: Color) {
        let estilo: Option<ID2D1StrokeStyle> = {
            let propiedades = D2D1_STROKE_STYLE_PROPERTIES1 {
                startCap: D2D1_CAP_STYLE_FLAT,
                endCap: D2D1_CAP_STYLE_FLAT,
                dashCap: D2D1_CAP_STYLE_FLAT,
                lineJoin: D2D1_LINE_JOIN_MITER,
                miterLimit: 10.0,
                dashStyle: D2D1_DASH_STYLE_DASH,
                dashOffset: 0.0,
                ..Default::default()
            };
            // SAFETY: la factoria vive en el motor; crear un estilo de trazo
            // no tiene precondiciones. El cast es el upcast StrokeStyle1 ->
            // StrokeStyle, que DrawRectangle espera.
            unsafe {
                self.motor
                    .fabrica()
                    .CreateStrokeStyle(&propiedades, None)
                    .ok()
                    .and_then(|e| e.cast::<ID2D1StrokeStyle>().ok())
            }
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; objetos vivos. StrokeStyle1
            // hereda de StrokeStyle, que es lo que DrawRectangle espera.
            unsafe {
                self.motor
                    .contexto()
                    .DrawRectangle(&r.a_d2d(), &p, grosor, estilo.as_ref())
            };
        }
    }

    pub fn linea(&self, desde: (f32, f32), hasta: (f32, f32), grosor: f32, color: Color) {
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; pincel y contexto vivos.
            unsafe {
                self.motor.contexto().DrawLine(
                    Vector2 {
                        X: desde.0,
                        Y: desde.1,
                    },
                    Vector2 {
                        X: hasta.0,
                        Y: hasta.1,
                    },
                    &p,
                    grosor,
                    None,
                )
            };
        }
    }

    /// Dibuja un bitmap. `nitido` usa vecino mas cercano (la lupa: pixeles
    /// reales); si no, interpolacion lineal (reescalados suaves).
    pub fn bitmap(&self, b: &ID2D1Bitmap1, destino: RectF, fuente: Option<RectF>, nitido: bool) {
        let modo = if nitido {
            D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR
        } else {
            D2D1_INTERPOLATION_MODE_LINEAR
        };
        let fuente_d2d = fuente.map(|f| f.a_d2d());
        // SAFETY: dentro del fotograma; bitmap del mismo dispositivo D2D
        // (obligacion del llamante: todos los bitmaps salen de este motor).
        unsafe {
            self.motor.contexto().DrawBitmap(
                b,
                Some(&destino.a_d2d()),
                1.0,
                modo,
                fuente_d2d.as_ref().map(|f| f as *const _),
                None,
            )
        };
    }

    fn disposicion(&self, texto: &str, tam: f32) -> Option<(IDWriteTextLayout, f32, f32)> {
        self.disposicion_ajustada(texto, tam, f32::MAX)
    }

    /// Como `disposicion`, pero partiendo las lineas al llegar a
    /// `ancho_max`. Un ancho infinito da una sola linea.
    fn disposicion_ajustada(
        &self,
        texto: &str,
        tam: f32,
        ancho_max: f32,
    ) -> Option<(IDWriteTextLayout, f32, f32)> {
        disposicion_dwrite(self.motor.dwrite(), texto, tam, ancho_max, &[], false)
    }

    /// Un parrafo con tramos de estilo (negrita, cursiva, monoespaciada):
    /// lo que pinta una nota en Markdown.
    #[allow(clippy::too_many_arguments)] // texto, posicion, tamano, ancho, tramos y color
    pub fn parrafo(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        tam: f32,
        ancho_max: f32,
        tramos: &[Tramo],
        color: Color,
    ) {
        let Some((disposicion, _, _)) =
            disposicion_dwrite(self.motor.dwrite(), texto, tam, ancho_max, tramos, false)
        else {
            return;
        };
        self.dibujar_disposicion(&disposicion, x, y, color);
    }

    /// Mide un parrafo con estilos, dentro de un fotograma.
    pub fn medir_parrafo(
        &self,
        texto: &str,
        tam: f32,
        ancho_max: f32,
        tramos: &[Tramo],
    ) -> (f32, f32) {
        disposicion_dwrite(self.motor.dwrite(), texto, tam, ancho_max, tramos, false)
            .map(|(_, w, h)| (w, h))
            .unwrap_or((0.0, 0.0))
    }

    /// Una sola linea que, si no cabe en `ancho_max`, termina en puntos
    /// suspensivos en vez de partirse o salirse: el nombre de una ficha.
    pub fn texto_linea(&self, texto: &str, x: f32, y: f32, tam: f32, ancho_max: f32, color: Color) {
        let Some((disposicion, _, _)) =
            disposicion_dwrite(self.motor.dwrite(), texto, tam, ancho_max, &[], true)
        else {
            return;
        };
        self.dibujar_disposicion(&disposicion, x, y, color);
    }

    fn dibujar_disposicion(&self, disposicion: &IDWriteTextLayout, x: f32, y: f32, color: Color) {
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; objetos vivos.
            unsafe {
                self.motor.contexto().DrawTextLayout(
                    Vector2 { X: x, Y: y },
                    disposicion,
                    &p,
                    Default::default(),
                )
            };
        }
    }

    /// Rellena un poligono cerrado dado por sus vertices.
    ///
    /// Es como se pinta la tinta de un trazo a mano: NO es una linea gruesa,
    /// es una mancha con forma, y por eso puede adelgazar en los extremos.
    /// Recibe pares de `f32` en vez de un tipo propio para no atar este crate
    /// al motor de dibujo, que vive en su misma capa.
    pub fn poligono(&self, vertices: &[(f32, f32)], color: Color) {
        if vertices.len() < 3 {
            return;
        }
        let Some(geometria) = self.geometria(vertices, true) else {
            return;
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; geometria y pincel vivos.
            unsafe { self.motor.contexto().FillGeometry(&geometria, &p, None) };
        }
    }

    /// Rellena `marco` dejando sin pintar el poligono `hueco`: es el foco
    /// de D51. Dos figuras cerradas en una misma geometria y la regla de
    /// relleno alternada de Direct2D hacen el agujero sin recortes ni
    /// capas.
    pub fn velo(&self, marco: RectF, hueco: &[(f32, f32)], color: Color) {
        use windows::Win32::Graphics::Direct2D::Common::{
            D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D1_FILL_MODE_ALTERNATE,
        };
        if hueco.len() < 3 {
            self.rellenar(marco, color);
            return;
        }
        let esquinas = [
            (marco.x, marco.y),
            (marco.x + marco.ancho, marco.y),
            (marco.x + marco.ancho, marco.y + marco.alto),
            (marco.x, marco.y + marco.alto),
        ];
        // SAFETY: igual que `geometria`: crear, rellenar entre Open/Close y
        // descartar si algo falla a mitad, sin usarla nunca a medias.
        let geometria = unsafe {
            let Ok(geometria) = self.motor.fabrica().CreatePathGeometry() else {
                return;
            };
            let Ok(sumidero) = geometria.Open() else {
                return;
            };
            sumidero.SetFillMode(D2D1_FILL_MODE_ALTERNATE);
            for figura in [&esquinas[..], hueco] {
                sumidero.BeginFigure(
                    Vector2 {
                        X: figura[0].0,
                        Y: figura[0].1,
                    },
                    D2D1_FIGURE_BEGIN_FILLED,
                );
                let resto: Vec<Vector2> = figura[1..]
                    .iter()
                    .map(|(x, y)| Vector2 { X: *x, Y: *y })
                    .collect();
                sumidero.AddLines(&resto);
                sumidero.EndFigure(D2D1_FIGURE_END_CLOSED);
            }
            if sumidero.Close().is_err() {
                return;
            }
            geometria
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; geometria y pincel vivos.
            unsafe { self.motor.contexto().FillGeometry(&geometria, &p, None) };
        }
    }

    /// Traza una polilinea abierta de grosor constante.
    pub fn polilinea(&self, vertices: &[(f32, f32)], grosor: f32, color: Color) {
        if vertices.len() < 2 {
            return;
        }
        let Some(geometria) = self.geometria(vertices, false) else {
            return;
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: igual que arriba.
            unsafe {
                self.motor
                    .contexto()
                    .DrawGeometry(&geometria, &p, grosor, None)
            };
        }
    }

    /// Construye una geometria a partir de los vertices. `cerrada` decide si
    /// el ultimo punto se une con el primero.
    fn geometria(&self, vertices: &[(f32, f32)], cerrada: bool) -> Option<ID2D1PathGeometry1> {
        use windows::Win32::Graphics::Direct2D::Common::{
            D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED,
            D2D1_FIGURE_END_OPEN,
        };

        // SAFETY: la geometria se crea, se rellena entre Open/Close y se
        // devuelve cerrada. Si algo falla a mitad se descarta sin usarla:
        // una geometria sin cerrar reventaria al dibujarla.
        unsafe {
            let geometria = self.motor.fabrica().CreatePathGeometry().ok()?;
            let sumidero = geometria.Open().ok()?;
            sumidero.BeginFigure(
                Vector2 {
                    X: vertices[0].0,
                    Y: vertices[0].1,
                },
                if cerrada {
                    D2D1_FIGURE_BEGIN_FILLED
                } else {
                    D2D1_FIGURE_BEGIN_HOLLOW
                },
            );
            let resto: Vec<Vector2> = vertices[1..]
                .iter()
                .map(|(x, y)| Vector2 { X: *x, Y: *y })
                .collect();
            sumidero.AddLines(&resto);
            sumidero.EndFigure(if cerrada {
                D2D1_FIGURE_END_CLOSED
            } else {
                D2D1_FIGURE_END_OPEN
            });
            sumidero.Close().ok()?;
            Some(geometria)
        }
    }

    /// Mide un texto ya ajustado a un ancho: lo que necesita una nota para
    /// saber de que tamano nace (S2-B).
    pub fn medir_texto_ajustado(&self, texto: &str, tam: f32, ancho_max: f32) -> (f32, f32) {
        self.disposicion_ajustada(texto, tam, ancho_max)
            .map(|(_, w, h)| (w, h))
            .unwrap_or((0.0, 0.0))
    }

    /// Dibuja texto partido a un ancho maximo, desde la esquina indicada.
    pub fn texto_ajustado(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        tam: f32,
        ancho_max: f32,
        color: Color,
    ) {
        let Some((disposicion, _, _)) = self.disposicion_ajustada(texto, tam, ancho_max) else {
            return;
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; objetos vivos.
            unsafe {
                self.motor.contexto().DrawTextLayout(
                    Vector2 { X: x, Y: y },
                    &disposicion,
                    &p,
                    Default::default(),
                )
            };
        }
    }

    /// Mide el texto sin dibujarlo: para colocar cajas.
    pub fn medir_texto(&self, texto: &str, tam: f32) -> (f32, f32) {
        self.disposicion(texto, tam)
            .map(|(_, w, h)| (w, h))
            .unwrap_or((0.0, 0.0))
    }

    pub fn texto(&self, texto: &str, x: f32, y: f32, tam: f32, color: Color) {
        let Some((disposicion, _, _)) = self.disposicion(texto, tam) else {
            return;
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; objetos vivos.
            unsafe {
                self.motor.contexto().DrawTextLayout(
                    Vector2 { X: x, Y: y },
                    &disposicion,
                    &p,
                    Default::default(),
                )
            };
        }
    }

    /// Texto sobre una caja redondeada semitransparente: las etiquetas del
    /// overlay (dimensiones, color de la lupa).
    pub fn texto_con_fondo(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        tam: f32,
        color_texto: Color,
        color_fondo: Color,
    ) {
        let (ancho, alto) = self.medir_texto(texto, tam);
        let relleno = tam * 0.4;
        self.rellenar_redondeado(
            RectF {
                x: x - relleno,
                y: y - relleno * 0.5,
                ancho: ancho + relleno * 2.0,
                alto: alto + relleno,
            },
            tam * 0.25,
            color_fondo,
        );
        self.texto(texto, x, y, tam, color_texto);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_READ,
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
        D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};

    fn dispositivo() -> (ID3D11Device, ID3D11DeviceContext) {
        let mut d3d = None;
        let mut ctx = None;
        // SAFETY: salidas locales, constantes documentadas.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                Some(&mut ctx),
            )
            .expect("GPU real");
        }
        (d3d.unwrap(), ctx.unwrap())
    }

    fn textura(d3d: &ID3D11Device, ancho: u32, alto: u32) -> ID3D11Texture2D {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: ancho,
            Height: alto,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
            ..Default::default()
        };
        let mut t = None;
        // SAFETY: desc inicializada; t es la salida local.
        unsafe { d3d.CreateTexture2D(&desc, None, Some(&mut t)) }.unwrap();
        t.unwrap()
    }

    fn pixel(
        d3d: &ID3D11Device,
        ctx: &ID3D11DeviceContext,
        t: &ID3D11Texture2D,
        x: u32,
        y: u32,
    ) -> [u8; 4] {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: GetDesc rellena la estructura local.
        unsafe { t.GetDesc(&mut desc) };
        let e_desc = D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
            ..desc
        };
        let mut e = None;
        // SAFETY: desc inicializada; e es la salida local.
        unsafe { d3d.CreateTexture2D(&e_desc, None, Some(&mut e)) }.unwrap();
        let e = e.unwrap();
        // SAFETY: mismo formato/tamano; staging legible tras CopyResource.
        unsafe { ctx.CopyResource(&e, t) };
        let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: staging con lectura; se desmapea justo despues.
        unsafe { ctx.Map(&e, 0, D3D11_MAP_READ, 0, Some(&mut mapa)) }.unwrap();
        // SAFETY: (x, y) dentro de la textura, obligacion del llamante.
        let v = unsafe {
            let base =
                (mapa.pData as *const u8).add(y as usize * mapa.RowPitch as usize + x as usize * 4);
            [*base, *base.add(1), *base.add(2), *base.add(3)]
        };
        // SAFETY: empareja el Map de arriba.
        unsafe { ctx.Unmap(&e, 0) };
        v
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn el_velo_oscurece_fuera_del_hueco_y_deja_el_hueco_intacto() {
        // D51: si la regla de relleno no fuera la alternada, el hueco se
        // pintaria tambien y el foco oscureceria justo lo que se ensena.
        let (d3d, ctx) = dispositivo();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let destino_tex = textura(&d3d, 128, 128);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                });
                p.velo(
                    RectF {
                        x: 0.0,
                        y: 0.0,
                        ancho: 128.0,
                        alto: 128.0,
                    },
                    &[(32.0, 32.0), (96.0, 32.0), (96.0, 96.0), (32.0, 96.0)],
                    Color {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                );
            })
            .unwrap();
        // BGRA: fuera del hueco, rojo opaco; dentro, el azul de la limpieza.
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 8, 8), [0, 0, 255, 255]);
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 64, 64), [255, 0, 0, 255]);
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn el_pintor_seguro_dibuja_sin_una_linea_de_unsafe_en_el_llamante() {
        let (d3d, ctx) = dispositivo();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let destino_tex = textura(&d3d, 128, 128);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();

        // Este bloque es exactamente lo que hara apps/pixpin: cero unsafe.
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                });
                p.rellenar(
                    RectF {
                        x: 8.0,
                        y: 8.0,
                        ancho: 24.0,
                        alto: 24.0,
                    },
                    Color {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                );
                p.rellenar_redondeado(
                    RectF {
                        x: 60.0,
                        y: 60.0,
                        ancho: 40.0,
                        alto: 20.0,
                    },
                    4.0,
                    Color::BLANCO,
                );
                p.trazar_discontinuo(
                    RectF {
                        x: 40.0,
                        y: 8.0,
                        ancho: 30.0,
                        alto: 30.0,
                    },
                    2.0,
                    Color::ACENTO,
                );
                p.linea((0.0, 120.0), (128.0, 120.0), 2.0, Color::BLANCO);
                p.texto("42", 100.0, 8.0, 12.0, Color::BLANCO);
                let (w, h) = p.medir_texto("970x542", 14.0);
                assert!(w > 10.0 && h > 5.0, "medir_texto devolvio ({w}, {h})");
            })
            .expect("el fotograma completo deberia dibujarse");

        // Rojo dentro del rectangulo, azul fuera: la tuberia entera funciona.
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 16, 16), [0, 0, 255, 255]);
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 110, 110), [255, 0, 0, 255]);
        // La caja blanca redondeada dejo su centro blanco.
        assert_eq!(
            pixel(&d3d, &ctx, &destino_tex, 80, 70),
            [255, 255, 255, 255]
        );
    }
}
