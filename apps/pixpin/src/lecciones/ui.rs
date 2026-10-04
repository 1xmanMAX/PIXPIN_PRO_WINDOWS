//! Lo comun de las dos ventanas de lecciones: la caja de texto, las fichas
//! (chips) que se marcan con un toque, el color de cada area y el bucle de
//! una ventana propia (como `galeria_capturas`).
//!
//! La caja de texto es minima a proposito: escribir, borrar, moverse con las
//! flechas, Inicio/Fin, pegar con Ctrl+V. Con eso se rellena una leccion; lo
//! demas (seleccionar con el raton, deshacer) no hace falta para una frase.

#![forbid(unsafe_code)]

use pixpin_render::{Color, Pintor, RectF};

use crate::caja_dibujo::hex;
use crate::ventanita::{APAGADO, Botones, CRISTAL, TEXTO};

pub const VK_RETROCESO: u32 = 0x08;
pub const VK_TAB: u32 = 0x09;
pub const VK_ENTRAR: u32 = 0x0D;
pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_FIN: u32 = 0x23;
pub const VK_INICIO: u32 = 0x24;
pub const VK_IZQUIERDA: u32 = 0x25;
pub const VK_DERECHA: u32 = 0x27;
pub const VK_SUPRIMIR: u32 = 0x2E;
pub const VK_V: u32 = 0x56;

/// El acento de las lecciones: el amarillo de la bombilla.
pub const PUESTO: Color = hex(0xffca28);
pub const PUESTO_FONDO: Color = hex(0x3d3520);
pub const FICHA: Color = hex(0x2c2c34);

/// `LeccionesActivity.colorDe`: las cuatro areas de inicio con su color fijo;
/// las demas, sacado de su nombre.
pub fn color_de(area: &str) -> Color {
    match area {
        "Trabajo" => hex(0x42a5f5),
        "Construcción" => hex(0xff9800),
        "Estudio" => hex(0xab47bc),
        "Vida diaria" => hex(0x66bb6a),
        "" => hex(0x9e9e9e),
        otra => {
            // `hashCode` de Kotlin sobre UTF-16, como el movil.
            let mut h: i32 = 0;
            for u in otra.encode_utf16() {
                h = h.wrapping_mul(31).wrapping_add(u as i32);
            }
            hsv(((h & 0x7fff_ffff) % 360) as f32, 0.55, 0.85)
        }
    }
}

fn hsv(h: f32, s: f32, v: f32) -> Color {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color {
        r: r + m,
        g: g + m,
        b: b + m,
        a: 1.0,
    }
}

pub fn con_alfa(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

/// Una caja de texto.
#[derive(Debug, Clone, Default)]
pub struct Campo {
    pub texto: String,
    /// Donde esta el cursor, en bytes (siempre en el borde de una letra).
    pub cursor: usize,
}

impl Campo {
    pub fn con(texto: &str) -> Campo {
        Campo {
            texto: texto.to_string(),
            cursor: texto.len(),
        }
    }

    pub fn poner(&mut self, texto: &str) {
        self.texto = texto.to_string();
        self.cursor = self.texto.len();
    }

    pub fn escribir(&mut self, s: &str) {
        let s: String = s
            .chars()
            .filter(|c| *c == '\n' || !c.is_control())
            .collect();
        self.texto.insert_str(self.cursor, &s);
        self.cursor += s.len();
    }

    fn anterior(&self) -> usize {
        self.texto[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn siguiente(&self) -> usize {
        self.texto[self.cursor..]
            .chars()
            .next()
            .map_or(self.cursor, |c| self.cursor + c.len_utf8())
    }

    /// Atiende una tecla. `multilinea`: si Intro (con Mayusculas) parte el
    /// renglon. Devuelve si la tecla era suya.
    pub fn tecla(&mut self, vk: u32, ctrl: bool, shift: bool, multilinea: bool) -> bool {
        match vk {
            VK_RETROCESO => {
                if ctrl {
                    // La palabra de antes entera, como en cualquier caja.
                    let antes = &self.texto[..self.cursor];
                    let sin_blancos = antes.trim_end();
                    let corte = sin_blancos.rfind(char::is_whitespace).map_or(0, |i| i + 1);
                    self.texto.replace_range(corte..self.cursor, "");
                    self.cursor = corte;
                } else if self.cursor > 0 {
                    let a = self.anterior();
                    self.texto.replace_range(a..self.cursor, "");
                    self.cursor = a;
                }
                true
            }
            VK_SUPRIMIR => {
                let s = self.siguiente();
                self.texto.replace_range(self.cursor..s, "");
                true
            }
            VK_IZQUIERDA => {
                self.cursor = self.anterior();
                true
            }
            VK_DERECHA => {
                self.cursor = self.siguiente();
                true
            }
            VK_INICIO => {
                self.cursor = if ctrl {
                    0
                } else {
                    self.texto[..self.cursor].rfind('\n').map_or(0, |i| i + 1)
                };
                true
            }
            VK_FIN => {
                self.cursor = if ctrl {
                    self.texto.len()
                } else {
                    self.texto[self.cursor..]
                        .find('\n')
                        .map_or(self.texto.len(), |i| self.cursor + i)
                };
                true
            }
            VK_V if ctrl => {
                if let Some(pixpin_codec::ContenidoPortapapeles::Texto(t)) =
                    pixpin_codec::portapapeles::leer()
                {
                    let t = t.replace("\r\n", "\n");
                    let t = if multilinea { t } else { t.replace('\n', " ") };
                    self.escribir(&t);
                }
                true
            }
            VK_ENTRAR if multilinea && shift => {
                self.escribir("\n");
                true
            }
            _ => false,
        }
    }

    /// Una letra escrita (`WM_CHAR`). Las de control llegan por [`tecla`].
    pub fn letra(&mut self, c: char) -> bool {
        if c.is_control() {
            return false;
        }
        let mut b = [0u8; 4];
        self.escribir(c.encode_utf8(&mut b));
        true
    }

    /// Pinta la caja en `caja` (de alto lo que devuelve [`alto`]) con la
    /// `pista` si esta vacia y el cursor si tiene el foco.
    #[allow(clippy::too_many_arguments)] // caja, tamano, foco, pista, color y escala
    pub fn pintar(
        &self,
        p: &Pintor,
        caja: RectF,
        tam: f32,
        foco: bool,
        pista: &str,
        reserva: f32,
        e: f32,
    ) {
        let fondo = if foco { hex(0x30303a) } else { CRISTAL };
        if foco {
            p.rellenar_redondeado(caja, 12.0 * e, con_alfa(PUESTO, 0.65));
            p.rellenar_redondeado(encoger(caja, 1.5 * e), 11.0 * e, fondo);
        } else {
            p.rellenar_redondeado(caja, 12.0 * e, fondo);
        }
        let pad = 10.0 * e;
        let ancho = (caja.ancho - 2.0 * pad - reserva).max(10.0);
        self.pintar_texto(p, caja.x + pad, caja.y + pad, ancho, tam, foco, pista, e);
    }

    /// Solo el texto (o la pista) y el cursor, sin la caja: para quien pone
    /// el fondo por su cuenta (la pastilla del buscador).
    #[allow(clippy::too_many_arguments)] // sitio, ancho, tamano, foco, pista y escala
    pub fn pintar_texto(
        &self,
        p: &Pintor,
        x0: f32,
        y0: f32,
        ancho: f32,
        tam: f32,
        foco: bool,
        pista: &str,
        e: f32,
    ) {
        if self.texto.is_empty() {
            p.texto_ajustado(pista, x0, y0, tam, ancho, APAGADO);
        } else {
            p.texto_ajustado(&self.texto, x0, y0, tam, ancho, TEXTO);
        }
        if foco {
            let (_, alto_linea) = p.medir_texto("Ag", tam);
            let (cx, cy) = if self.texto.is_empty() {
                (x0, y0)
            } else {
                // Una marca de ancho cero donde esta el cursor: la caja que
                // DirectWrite le da es donde va la raya.
                let mut con_marca = self.texto.clone();
                con_marca.insert(self.cursor, '\u{200B}');
                let i = self.texto[..self.cursor].encode_utf16().count() as u32;
                p.cajas_de_trozo(&con_marca, tam, ancho, &[], i, 1)
                    .first()
                    .map_or((x0, y0), |b| (x0 + b.x, y0 + b.y))
            };
            p.linea((cx, cy), (cx, cy + alto_linea), 1.5 * e, PUESTO);
        }
    }

    /// Lo que mide la caja con este texto y al menos `lineas` renglones.
    pub fn alto(
        &self,
        p: &Pintor,
        ancho: f32,
        tam: f32,
        lineas: usize,
        reserva: f32,
        e: f32,
    ) -> f32 {
        let pad = 10.0 * e;
        let (_, alto_linea) = p.medir_texto("Ag", tam);
        let (_, h) = if self.texto.is_empty() {
            (0.0, alto_linea)
        } else {
            // Un renglon que acaba en salto no lo cuenta DirectWrite.
            let t = if self.texto.ends_with('\n') {
                format!("{}.", self.texto)
            } else {
                self.texto.clone()
            };
            p.medir_texto_ajustado(&t, tam, (ancho - 2.0 * pad - reserva).max(10.0))
        };
        h.max(alto_linea * lineas as f32) + 2.0 * pad
    }
}

pub fn encoger(r: RectF, m: f32) -> RectF {
    RectF {
        x: r.x + m,
        y: r.y + m,
        ancho: (r.ancho - 2.0 * m).max(0.0),
        alto: (r.alto - 2.0 * m).max(0.0),
    }
}

/// Lo que mide una ficha con ese rotulo.
pub fn ancho_de_ficha(p: &Pintor, rotulo: &str, tam: f32, con_x: bool, e: f32) -> f32 {
    p.medir_texto(rotulo, tam).0 + 24.0 * e + if con_x { 16.0 * e } else { 0.0 }
}

/// Una ficha (chip) de las de Material: puesta (rellena de amarillo
/// apagado) o no, con una «×» si se quita con un toque. Se apunta en
/// `botones` con `que`.
#[allow(clippy::too_many_arguments)] // pintor, caja, que, rotulo, estado y escala
pub fn ficha<A: Copy>(
    p: &Pintor,
    botones: &mut Botones<A>,
    caja: RectF,
    que: A,
    rotulo: &str,
    puesta: bool,
    con_x: bool,
    punto: Option<Color>,
    tam: f32,
    e: f32,
) {
    let encima = crate::ventanita::dentro(caja, botones.raton);
    let radio = 8.0 * e;
    if puesta {
        p.rellenar_redondeado(caja, radio, PUESTO_FONDO);
    } else {
        p.rellenar_redondeado(caja, radio, hex(0x4a4a52));
        p.rellenar_redondeado(
            encoger(caja, 1.0 * e),
            radio - 1.0,
            if encima { hex(0x34343c) } else { FICHA },
        );
    }
    let (_, th) = p.medir_texto(rotulo, tam);
    let mut tx = caja.x + 12.0 * e;
    if let Some(c) = punto {
        p.circulo((tx + 5.0 * e, caja.y + caja.alto / 2.0), 5.0 * e, c);
        tx += 16.0 * e;
    }
    p.texto_color(
        rotulo,
        tx,
        caja.y + (caja.alto - th) / 2.0,
        tam,
        if puesta { PUESTO } else { TEXTO },
    );
    if con_x {
        let x = caja.x + caja.ancho - 18.0 * e;
        let cy = caja.y + caja.alto / 2.0;
        let l = 4.0 * e;
        let c = if puesta { PUESTO } else { APAGADO };
        p.linea((x - l, cy - l), (x + l, cy + l), 1.4 * e, c);
        p.linea((x - l, cy + l), (x + l, cy - l), 1.4 * e, c);
    }
    botones.zona(caja, que);
}

/// Fichas en renglones que se parten al llegar al borde. Devuelve la `y`
/// de debajo de la ultima.
#[allow(clippy::too_many_arguments)] // pintor, botones, origen, ancho, fichas y escala
pub fn fila_de_fichas<A: Copy>(
    p: &Pintor,
    botones: &mut Botones<A>,
    x0: f32,
    y0: f32,
    ancho: f32,
    fichas: &[(String, A, bool, bool)],
    tam: f32,
    e: f32,
) -> f32 {
    fila_con_puntos(p, botones, x0, y0, ancho, fichas, &[], tam, e)
}

/// Como [`fila_de_fichas`], con un punto de color delante de las que lo
/// tengan en `puntos` (las areas, como el movil).
#[allow(clippy::too_many_arguments)] // pintor, botones, origen, ancho, fichas, puntos y escala
pub fn fila_con_puntos<A: Copy>(
    p: &Pintor,
    botones: &mut Botones<A>,
    x0: f32,
    y0: f32,
    ancho: f32,
    fichas: &[(String, A, bool, bool)],
    puntos: &[Option<Color>],
    tam: f32,
    e: f32,
) -> f32 {
    let alto = 32.0 * e;
    let hueco = 6.0 * e;
    let (mut x, mut y) = (x0, y0);
    for (i, (rotulo, que, puesta, con_x)) in fichas.iter().enumerate() {
        let punto = puntos.get(i).copied().flatten();
        let extra = if punto.is_some() { 16.0 * e } else { 0.0 };
        let w = (ancho_de_ficha(p, rotulo, tam, *con_x, e) + extra).min(ancho);
        if x > x0 && x + w > x0 + ancho {
            x = x0;
            y += alto + hueco;
        }
        ficha(
            p,
            botones,
            RectF {
                x,
                y,
                ancho: w,
                alto,
            },
            *que,
            rotulo,
            *puesta,
            *con_x,
            punto,
            tam,
            e,
        );
        x += w + hueco;
    }
    if fichas.is_empty() { y0 } else { y + alto }
}

/// Un boton con rotulo (que puede llevar emoji) y fondo propio.
#[allow(clippy::too_many_arguments)] // pintor, botones, caja, que, rotulo, colores y escala
pub fn boton<A: Copy>(
    p: &Pintor,
    botones: &mut Botones<A>,
    caja: RectF,
    que: A,
    rotulo: &str,
    fondo: Color,
    tinta: Color,
    tam: f32,
    e: f32,
) {
    let encima = crate::ventanita::dentro(caja, botones.raton);
    let f = if encima {
        Color {
            r: (fondo.r + 0.06).min(1.0),
            g: (fondo.g + 0.06).min(1.0),
            b: (fondo.b + 0.06).min(1.0),
            a: fondo.a,
        }
    } else {
        fondo
    };
    p.rellenar_redondeado(caja, 10.0 * e, f);
    let (w, h) = p.medir_texto(rotulo, tam);
    p.texto_color(
        rotulo,
        caja.x + (caja.ancho - w) / 2.0,
        caja.y + (caja.alto - h) / 2.0,
        tam,
        tinta,
    );
    botones.zona(caja, que);
}

/// Un aviso abajo, como el `Toast` del movil.
pub fn aviso(p: &Pintor, texto: &str, ancho: f32, alto: f32, e: f32) {
    let tam = 14.0 * e;
    let (tw, th) = p.medir_texto_ajustado(texto, tam, ancho - 60.0 * e);
    let caja = RectF {
        x: (ancho - tw) / 2.0 - 14.0 * e,
        y: alto - th - 90.0 * e,
        ancho: tw + 28.0 * e,
        alto: th + 16.0 * e,
    };
    p.rellenar_redondeado(
        caja,
        8.0 * e,
        Color {
            a: 0.92,
            ..Color::NEGRO
        },
    );
    p.texto_ajustado(
        texto,
        caja.x + 14.0 * e,
        caja.y + 8.0 * e,
        tam,
        tw + 2.0,
        TEXTO,
    );
}

/// Hasta `n` letras, con «…» si se corta.
pub fn corto(s: &str, n: usize) -> String {
    let s = s.replace('\n', " ");
    if s.chars().count() <= n {
        s
    } else {
        format!("{}…", s.chars().take(n).collect::<String>().trim_end())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_caja_escribe_borra_y_se_mueve_por_letras_y_no_por_bytes() {
        let mut c = Campo::con("año");
        assert!(c.tecla(VK_IZQUIERDA, false, false, false));
        assert!(c.tecla(VK_IZQUIERDA, false, false, false));
        c.letra('x');
        assert_eq!(c.texto, "axño");
        c.tecla(VK_SUPRIMIR, false, false, false);
        assert_eq!(c.texto, "axo");
        c.tecla(VK_FIN, false, false, false);
        c.tecla(VK_RETROCESO, false, false, false);
        assert_eq!(c.texto, "ax");
        // Ctrl+Retroceso se lleva la palabra.
        let mut d = Campo::con("revisar la escala");
        d.tecla(VK_RETROCESO, true, false, false);
        assert_eq!(d.texto, "revisar la ");
        // Intro solo parte renglon con Mayusculas y si la caja es de varios.
        assert!(!d.tecla(VK_ENTRAR, false, false, true));
        assert!(!d.tecla(VK_ENTRAR, false, true, false));
        assert!(d.tecla(VK_ENTRAR, false, true, true));
        assert!(d.texto.ends_with('\n'));
        // Caso negativo: una letra de control no se escribe.
        assert!(!d.letra('\u{8}'));
    }

    #[test]
    fn las_areas_de_inicio_tienen_su_color_y_las_demas_uno_estable() {
        assert_eq!(color_de("Trabajo"), hex(0x42a5f5));
        assert_eq!(color_de("Mi área"), color_de("Mi área"));
        assert_ne!(color_de("Mi área"), color_de("Otra"));
    }
}

// ------------------------------------------------------------------ v2

/// Los colores del rediseno v2 (maqueta `Lecciones2.dc.html`): el oscuro de
/// siempre de las lecciones, con el azul de la accion principal y el rojo
/// de borrar como en el resto de la app.
pub mod v2 {
    use crate::caja_dibujo::hex;
    use pixpin_render::Color;

    pub const FONDO: Color = hex(0x1c1c1e);
    pub const COLUMNA: Color = hex(0x232326);
    pub const CAJA: Color = hex(0x2c2c2e);
    pub const ENCIMA: Color = hex(0x343437);
    pub const LINEA: Color = hex(0x2e2e31);
    pub const TEXTO: Color = hex(0xf5f5f7);
    pub const CUERPO: Color = hex(0xededf0);
    pub const SUAVE: Color = hex(0xc7c7cc);
    pub const APAGADO: Color = hex(0x98989d);
    /// La accion principal (Guardar, Siguiente).
    pub const AZUL: Color = hex(0x0060df);
    /// Lo elegido (la fila, el foco).
    pub const ELEGIDO: Color = hex(0x0a84ff);
    pub const CIAN: Color = hex(0x64d2ff);
    pub const NARANJA: Color = hex(0xff9f0a);
    pub const VERDE: Color = hex(0x30d158);
    pub const ROJO: Color = hex(0xff453a);
    pub const ROJO_TEXTO: Color = hex(0xff6961);
    pub const AMARILLO: Color = hex(0xffd60a);
    pub const OSCURO: Color = hex(0x1c1c1e);
    pub const BLANCO: Color = hex(0xffffff);
}

/// El color de cada gravedad: leve verde, importante naranja, grave rojo.
pub fn color_de_gravedad(g: i64) -> Color {
    match g {
        i64::MIN..=1 => v2::VERDE,
        2 => v2::NARANJA,
        _ => v2::ROJO,
    }
}

fn letra_negrita() -> pixpin_render::letras::Letra<'static> {
    pixpin_render::letras::Letra {
        negrita: true,
        ..pixpin_render::letras::Letra::de(pixpin_render::letras::LETRA_DEL_SISTEMA)
    }
}

/// Un texto en negrita, partido en renglones a `ancho`.
#[allow(clippy::too_many_arguments)] // pintor, texto, sitio, tamano, ancho y color
pub fn negrita(p: &Pintor, texto: &str, x: f32, y: f32, tam: f32, ancho: f32, color: Color) {
    p.texto_con_letra(texto, x, y, tam, ancho, &letra_negrita(), color);
}

/// Lo que mide en negrita, partido a `ancho`.
pub fn medir_negrita(p: &Pintor, texto: &str, tam: f32, ancho: f32) -> (f32, f32) {
    p.medir_con_letra(texto, tam, ancho, &letra_negrita())
}

/// **La chapita de un atajo** («Ctrl V», «Esc», «1»): lo que mide.
pub fn ancho_de_chapa(p: &Pintor, tecla: &str, e: f32) -> f32 {
    (p.medir_texto(tecla, 11.5 * e).0 + 12.0 * e).max(20.0 * e)
}

/// Pinta la chapita de un atajo con su borde fino, centrada en `cy`, desde
/// `x`. Devuelve su ancho. `tinta` es el color de la letra (sobre un boton
/// azul va en blanco).
pub fn chapa(p: &Pintor, tecla: &str, x: f32, cy: f32, tinta: Color, fondo: Color, e: f32) -> f32 {
    let tam = 11.5 * e;
    let w = ancho_de_chapa(p, tecla, e);
    let h = 20.0 * e;
    let caja = RectF {
        x,
        y: cy - h / 2.0,
        ancho: w,
        alto: h,
    };
    p.rellenar_redondeado(caja, 5.0 * e, con_alfa(v2::BLANCO, 0.12));
    p.rellenar_redondeado(encoger(caja, 1.0 * e), 4.0 * e, fondo);
    let (tw, th) = p.medir_texto(tecla, tam);
    p.texto(tecla, x + (w - tw) / 2.0, cy - th / 2.0, tam, tinta);
    w
}

/// Lo que mide un boton v2 con icono, rotulo y chapita.
pub fn ancho_de_boton(p: &Pintor, icono: bool, rotulo: &str, tecla: Option<&str>, e: f32) -> f32 {
    let mut w = 28.0 * e + p.medir_texto(rotulo, 14.0 * e).0;
    if icono {
        w += 26.0 * e;
    }
    if let Some(t) = tecla {
        w += 8.0 * e + ancho_de_chapa(p, t, e);
    }
    w
}

/// **Un boton del v2**: con icono, rotulo y la chapita de su atajo.
/// `fondo` None = transparente (el de «Borrar»). Con el raton encima se
/// aclara. Se apunta en `botones`.
#[allow(clippy::too_many_arguments)] // pintor, botones, caja, que, icono, rotulo, tecla, colores y escala
pub fn boton_v2<A: Copy>(
    p: &Pintor,
    botones: &mut Botones<A>,
    caja: RectF,
    que: A,
    icono: Option<&pixpin_render::icono::Icono>,
    rotulo: &str,
    tecla: Option<&str>,
    fondo: Option<Color>,
    tinta: Color,
    e: f32,
) {
    let encima = crate::ventanita::dentro(caja, botones.raton);
    match (fondo, encima) {
        (Some(c), true) => p.rellenar_redondeado(caja, 10.0 * e, aclarar(c, 0.07)),
        (Some(c), false) => p.rellenar_redondeado(caja, 10.0 * e, c),
        (None, true) => p.rellenar_redondeado(caja, 10.0 * e, con_alfa(v2::BLANCO, 0.08)),
        (None, false) => {}
    }
    let ancho = ancho_de_boton(p, icono.is_some(), rotulo, tecla, e);
    let mut x = caja.x + ((caja.ancho - ancho) / 2.0).max(0.0) + 14.0 * e;
    let cy = caja.y + caja.alto / 2.0;
    if let Some(i) = icono {
        p.icono(
            i,
            RectF {
                x,
                y: cy - 9.0 * e,
                ancho: 18.0 * e,
                alto: 18.0 * e,
            },
            tinta,
        );
        x += 26.0 * e;
    }
    let (tw, th) = p.medir_texto(rotulo, 14.0 * e);
    p.texto(rotulo, x, cy - th / 2.0, 14.0 * e, tinta);
    x += tw + 8.0 * e;
    if let Some(t) = tecla {
        let azul = fondo.is_some_and(|c| c == v2::AZUL);
        let fondo_chapa = if azul {
            aclarar(v2::AZUL, 0.12)
        } else {
            hex(0x2a2a2d)
        };
        let tinta_chapa = if azul { v2::BLANCO } else { v2::SUAVE };
        chapa(p, t, x, cy, tinta_chapa, fondo_chapa, e);
    }
    botones.zona(caja, que);
}

pub fn aclarar(c: Color, d: f32) -> Color {
    Color {
        r: (c.r + d).min(1.0),
        g: (c.g + d).min(1.0),
        b: (c.b + d).min(1.0),
        a: c.a,
    }
}

/// Dias de un instante en ms (desde 1970, en UTC).
fn dias(ms: i64) -> i64 {
    ms.div_euclid(pixpin_lecciones::leccion::DIA)
}

/// `(anio, mes 1..=12, dia)` de un instante en ms (UTC; Howard Hinnant,
/// `civil_from_days`).
pub fn fecha_de(ms: i64) -> (i64, u32, u32) {
    let z = dias(ms) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Si dos instantes caen en el mismo mes (para «Este mes» / «Antes»).
pub fn mismo_mes(a: i64, b: i64) -> bool {
    let (ya, ma, _) = fecha_de(a);
    let (yb, mb, _) = fecha_de(b);
    (ya, ma) == (yb, mb)
}

/// Cuanto hace, en la unidad que se lee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hace {
    Hoy,
    Ayer,
    Dias(i64),
    Semanas(i64),
    Meses(i64),
    Anios(i64),
}

pub fn hace(cuando: i64, ahora: i64) -> Hace {
    let d = dias(ahora) - dias(cuando);
    match d {
        i64::MIN..=0 => Hace::Hoy,
        1 => Hace::Ayer,
        2..=13 => Hace::Dias(d),
        14..=59 => Hace::Semanas(d / 7),
        60..=729 => Hace::Meses(d / 30),
        _ => Hace::Anios(d / 365),
    }
}

#[cfg(test)]
mod pruebas_v2 {
    use super::*;
    use pixpin_lecciones::leccion::DIA;

    #[test]
    fn las_fechas_se_cuentan_bien_y_hace_elige_la_unidad() {
        // 4-oct-2026 = 20730 dias desde 1970.
        let hoy = 20_730 * DIA + 5_000;
        assert_eq!(fecha_de(hoy), (2026, 10, 4));
        assert_eq!(fecha_de(0), (1970, 1, 1));
        assert_eq!(fecha_de(20_729 * DIA - 1), (2026, 10, 2));
        assert!(mismo_mes(hoy, 20_727 * DIA));
        assert_eq!(hace(hoy - 3 * DIA, hoy), Hace::Dias(3));
        assert_eq!(hace(hoy - 15 * DIA, hoy), Hace::Semanas(2));
        assert_eq!(hace(hoy - 65 * DIA, hoy), Hace::Meses(2));
        assert_eq!(hace(hoy - DIA, hoy), Hace::Ayer);
        // Caso negativo: el mes de antes no es «este mes», y algo del futuro
        // (un reloj adelantado del otro aparato) es «hoy».
        assert!(!mismo_mes(hoy, 20_725 * DIA));
        assert_eq!(hace(hoy + 9 * DIA, hoy), Hace::Hoy);
    }

    #[test]
    fn cada_gravedad_tiene_su_color() {
        assert_eq!(color_de_gravedad(1), v2::VERDE);
        assert_eq!(color_de_gravedad(2), v2::NARANJA);
        assert_eq!(color_de_gravedad(3), v2::ROJO);
        // Caso negativo: un valor raro del otro aparato no rompe nada.
        assert_eq!(color_de_gravedad(0), v2::VERDE);
        assert_eq!(color_de_gravedad(7), v2::ROJO);
    }
}

/// `a` con un `t` de `b` encima (0 = todo `a`).
pub fn mezclar(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a,
    }
}
