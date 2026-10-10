//! **El cursor de la caja de escribir del chat**: donde esta la rayita, lo
//! seleccionado, y que letra cae debajo del raton (10-oct-2026).
//!
//! Antes la caja solo sabia anadir al final y borrar la ultima letra: el
//! cursor iba pegado a lo escrito. El usuario lo conto asi: «no puedo mover
//! la rayita de escritura, no me deja seleccionar nada y tampoco ponerle la
//! rayita para escribir en otras partes del texto».
//!
//! Aqui no se pinta nada. Quien pinta apunta la caja de cada letra (la que
//! le da DirectWrite, la MISMA disposicion con la que pinta el texto) y con
//! esas cajas se decide donde cae un clic ([`indice_en_punto`]) y donde se
//! dibuja la rayita ([`sitio_del_cursor`]).

use std::ops::Range;

use pixpin_render::RectF;

/// La rayita y la seleccion. Las posiciones son bytes de `borrador`, siempre
/// en el borde de una letra.
///
/// `pos` vacio es «al final»: asi lo que se mete por fuera (el dictado, lo
/// que vuelve de un cuadro de enviar) sigue dejando la rayita detras de lo
/// escrito, como antes, sin que cada uno de esos sitios tenga que saber que
/// hay un cursor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Cursor {
    pos: Option<usize>,
    /// El otro extremo de la seleccion, si se esta seleccionando.
    ancla: Option<usize>,
}

/// `i` dentro de `t` y en el borde de una letra (hacia atras).
fn ajustar(t: &str, i: usize) -> usize {
    let mut i = i.min(t.len());
    while !t.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn anterior(t: &str, i: usize) -> usize {
    t[..i].char_indices().next_back().map_or(0, |(j, _)| j)
}

fn siguiente(t: &str, i: usize) -> usize {
    t[i..].chars().next().map_or(i, |c| i + c.len_utf8())
}

/// Las tres clases de letra para saltar y elegir palabras: blancos, letras
/// de palabra y signos.
fn clase(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if c.is_alphanumeric() || c == '_' {
        1
    } else {
        2
    }
}

/// Donde empieza la palabra de antes de `i` (Ctrl+flecha izquierda).
fn palabra_atras(t: &str, i: usize) -> usize {
    let antes = &t[..i];
    let sin_blancos = antes.trim_end_matches(|c: char| c.is_whitespace());
    let Some(ultima) = sin_blancos.chars().next_back() else {
        return 0;
    };
    let k = clase(ultima);
    sin_blancos
        .char_indices()
        .rev()
        .take_while(|(_, c)| clase(*c) == k)
        .last()
        .map_or(sin_blancos.len(), |(j, _)| j)
}

/// Donde acaba la palabra de despues de `i` (Ctrl+flecha derecha), con los
/// blancos que la siguen, como en Windows.
fn palabra_delante(t: &str, i: usize) -> usize {
    let resto = &t[i..];
    let Some(primera) = resto.chars().next() else {
        return i;
    };
    let k = clase(primera);
    let mut fin = resto
        .char_indices()
        .find(|(_, c)| clase(*c) != k)
        .map_or(resto.len(), |(j, _)| j);
    if k != 0 {
        fin += resto[fin..]
            .char_indices()
            .find(|(_, c)| !c.is_whitespace() || *c == '\n')
            .map_or(resto.len() - fin, |(j, _)| j);
    }
    i + fin
}

/// La palabra bajo `i` (doble clic): las letras seguidas de su misma clase.
/// Al final del texto vale la de antes.
pub(crate) fn palabra_en(t: &str, i: usize) -> Range<usize> {
    let mut i = ajustar(t, i);
    if i == t.len() {
        if i == 0 {
            return 0..0;
        }
        i = anterior(t, i);
    }
    let Some(c) = t[i..].chars().next() else {
        return i..i;
    };
    if c == '\n' {
        return i..i;
    }
    let k = clase(c);
    let mismo = |c: char| c != '\n' && clase(c) == k;
    let inicio = t[..i]
        .char_indices()
        .rev()
        .take_while(|(_, c)| mismo(*c))
        .last()
        .map_or(i, |(j, _)| j);
    let fin = t[i..]
        .char_indices()
        .find(|(_, c)| !mismo(*c))
        .map_or(t.len(), |(j, _)| i + j);
    inicio..fin
}

impl Cursor {
    /// Donde esta la rayita en `t`.
    pub(crate) fn pos(&self, t: &str) -> usize {
        ajustar(t, self.pos.unwrap_or(t.len()))
    }

    /// Lo seleccionado, si hay algo (una seleccion vacia no cuenta).
    pub(crate) fn seleccion(&self, t: &str) -> Option<Range<usize>> {
        let a = ajustar(t, self.ancla?);
        let p = self.pos(t);
        (a != p).then(|| a.min(p)..a.max(p))
    }

    /// El texto seleccionado.
    pub(crate) fn seleccionado<'a>(&self, t: &'a str) -> Option<&'a str> {
        self.seleccion(t).map(|r| &t[r])
    }

    /// Lleva la rayita a `i`. Con `extender` (Mayusculas) lo de en medio
    /// queda seleccionado; sin el, la seleccion se suelta.
    pub(crate) fn poner(&mut self, t: &str, i: usize, extender: bool) {
        if extender {
            if self.ancla.is_none() {
                self.ancla = Some(self.pos(t));
            }
        } else {
            self.ancla = None;
        }
        let i = ajustar(t, i);
        self.pos = (i < t.len()).then_some(i);
    }

    /// Selecciona `r` dejando la rayita en su final.
    pub(crate) fn elegir(&mut self, t: &str, r: Range<usize>) {
        self.ancla = Some(ajustar(t, r.start));
        let fin = ajustar(t, r.end);
        self.pos = (fin < t.len()).then_some(fin);
    }

    /// Ctrl+A.
    pub(crate) fn todo(&mut self, t: &str) {
        self.elegir(t, 0..t.len());
    }

    /// Vuelve a «al final y sin seleccion»: tras enviar o vaciar la caja.
    pub(crate) fn soltar(&mut self) {
        *self = Cursor::default();
    }

    /// Quita lo seleccionado de `t`, si hay algo. Devuelve si quito.
    fn quitar_seleccion(&mut self, t: &mut String) -> bool {
        let Some(r) = self.seleccion(t) else {
            self.ancla = None;
            return false;
        };
        t.replace_range(r.clone(), "");
        self.ancla = None;
        self.pos = (r.start < t.len()).then_some(r.start);
        true
    }

    /// Escribe `s` donde esta la rayita, en lugar de lo seleccionado.
    pub(crate) fn escribir(&mut self, t: &mut String, s: &str) {
        self.quitar_seleccion(t);
        let p = self.pos(t);
        t.insert_str(p, s);
        let fin = p + s.len();
        self.pos = (fin < t.len()).then_some(fin);
    }

    /// Retroceso: lo seleccionado, o la letra (con Ctrl, la palabra) de
    /// antes de la rayita.
    pub(crate) fn borrar_atras(&mut self, t: &mut String, palabra: bool) {
        if self.quitar_seleccion(t) {
            return;
        }
        let p = self.pos(t);
        let desde = if palabra {
            palabra_atras(t, p)
        } else {
            anterior(t, p)
        };
        t.replace_range(desde..p, "");
        self.pos = (desde < t.len()).then_some(desde);
    }

    /// Suprimir: lo seleccionado, o la letra (con Ctrl, la palabra) de
    /// despues de la rayita.
    pub(crate) fn borrar_delante(&mut self, t: &mut String, palabra: bool) {
        if self.quitar_seleccion(t) {
            return;
        }
        let p = self.pos(t);
        let hasta = if palabra {
            palabra_delante(t, p)
        } else {
            siguiente(t, p)
        };
        t.replace_range(p..hasta, "");
        self.pos = (p < t.len()).then_some(p);
    }

    /// Flecha izquierda. Sin Mayusculas y con algo seleccionado, la rayita
    /// va al principio de la seleccion, como en cualquier caja de Windows.
    pub(crate) fn izquierda(&mut self, t: &str, shift: bool, ctrl: bool) {
        if !shift && let Some(r) = self.seleccion(t) {
            self.poner(t, r.start, false);
            return;
        }
        let p = self.pos(t);
        let i = if ctrl {
            palabra_atras(t, p)
        } else {
            anterior(t, p)
        };
        self.poner(t, i, shift);
    }

    /// Flecha derecha.
    pub(crate) fn derecha(&mut self, t: &str, shift: bool, ctrl: bool) {
        if !shift && let Some(r) = self.seleccion(t) {
            self.poner(t, r.end, false);
            return;
        }
        let p = self.pos(t);
        let i = if ctrl {
            palabra_delante(t, p)
        } else {
            siguiente(t, p)
        };
        self.poner(t, i, shift);
    }

    /// Inicio: al principio del renglon (con Ctrl, del texto).
    pub(crate) fn inicio(&mut self, t: &str, shift: bool, ctrl: bool) {
        let p = self.pos(t);
        let i = if ctrl {
            0
        } else {
            t[..p].rfind('\n').map_or(0, |j| j + 1)
        };
        self.poner(t, i, shift);
    }

    /// Fin: al final del renglon (con Ctrl, del texto).
    pub(crate) fn fin(&mut self, t: &str, shift: bool, ctrl: bool) {
        let p = self.pos(t);
        let i = if ctrl {
            t.len()
        } else {
            t[p..].find('\n').map_or(t.len(), |j| p + j)
        };
        self.poner(t, i, shift);
    }
}

/// La caja de cada letra de `t` tal como la pinta DirectWrite, con el origen
/// en la esquina del texto: `(byte donde empieza la letra, su caja)`. Las
/// apunta quien pinta; `caja_de` es la pregunta a la disposicion (desde, en
/// unidades UTF-16, y cuantas).
pub(crate) fn cajas_de_letras(
    t: &str,
    mut caja_de: impl FnMut(u32, u32) -> Option<RectF>,
) -> Vec<(usize, RectF)> {
    let mut u = 0u32;
    let mut cajas = Vec::with_capacity(t.len());
    for (i, c) in t.char_indices() {
        let largo = c.len_utf16() as u32;
        if let Some(b) = caja_de(u, largo) {
            cajas.push((i, b));
        }
        u += largo;
    }
    cajas
}

/// Los renglones de las cajas: se parte donde una letra baja de altura.
fn renglones(cajas: &[(usize, RectF)]) -> Vec<&[(usize, RectF)]> {
    let mut out = Vec::new();
    let mut desde = 0;
    for k in 1..cajas.len() {
        let (a, b) = (cajas[k - 1].1, cajas[k].1);
        if (b.y - a.y).abs() > a.alto.max(1.0) / 2.0 {
            out.push(&cajas[desde..k]);
            desde = k;
        }
    }
    if desde < cajas.len() {
        out.push(&cajas[desde..]);
    }
    out
}

/// **Que hueco entre letras cae bajo `(x, y)`**, con el punto relativo a la
/// esquina del texto: el indice (en bytes de `t`) donde se pondria la rayita
/// al pulsar ahi.
///
/// Se elige el renglon por la altura (por encima del primero, el primero;
/// por debajo del ultimo, el ultimo, o el renglon vacio que deja un salto
/// final) y en el la letra por la mitad de su caja: pulsar en la mitad
/// derecha de una letra deja la rayita detras de ella. Pasado el final de
/// un renglon, la rayita va a su final (antes del salto de renglon).
pub(crate) fn indice_en_punto(t: &str, cajas: &[(usize, RectF)], x: f32, y: f32) -> usize {
    let lineas = renglones(cajas);
    let (Some(primera), Some(ultima)) = (lineas.first(), lineas.last()) else {
        return t.len();
    };
    let abajo = |l: &[(usize, RectF)]| l.iter().map(|(_, b)| b.y + b.alto).fold(f32::MIN, f32::max);
    let linea = if y < primera[0].1.y {
        primera
    } else if let Some(l) = lineas.iter().find(|l| y < abajo(l)) {
        l
    } else if t.ends_with('\n') {
        // Debajo del todo con un salto al final: el renglon vacio de abajo,
        // que no tiene letras propias.
        return t.len();
    } else {
        ultima
    };
    for (i, b) in linea.iter() {
        if t[*i..].starts_with('\n') || x < b.x + b.ancho / 2.0 {
            return *i;
        }
    }
    let (i, _) = linea[linea.len() - 1];
    siguiente(t, i)
}

/// Lo mismo, pero solo si el punto cae dentro de `zona` (todo en
/// coordenadas de la ventana, y `origen` es la esquina del texto). Fuera de
/// la caja no hay letra que buscar.
pub(crate) fn indice_en(
    t: &str,
    cajas: &[(usize, RectF)],
    origen: (f32, f32),
    zona: RectF,
    x: f32,
    y: f32,
) -> Option<usize> {
    let dentro =
        x >= zona.x && y >= zona.y && x < zona.x + zona.ancho && y < zona.y + zona.alto;
    dentro.then(|| indice_en_punto(t, cajas, x - origen.0, y - origen.1))
}

/// Donde se dibuja la rayita para la posicion `pos`: la esquina de arriba,
/// relativa al texto, y su alto. `alto_linea` vale para el texto vacio.
pub(crate) fn sitio_del_cursor(
    t: &str,
    cajas: &[(usize, RectF)],
    pos: usize,
    alto_linea: f32,
) -> (f32, f32, f32) {
    if let Some((_, b)) = cajas.iter().find(|(i, _)| *i == pos) {
        return (b.x, b.y, b.alto);
    }
    match cajas.last() {
        None => (0.0, 0.0, alto_linea),
        Some((i, b)) if t[*i..].starts_with('\n') => (0.0, b.y + b.alto, b.alto),
        Some((_, b)) => (b.x + b.ancho, b.y, b.alto),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Letras de 10 de ancho y renglones de 20, partiendo en cada salto.
    /// Asi las cajas se pueden escribir a mano, sin DirectWrite.
    fn cajas(t: &str) -> Vec<(usize, RectF)> {
        let (mut x, mut y) = (0.0, 0.0);
        let mut out = Vec::new();
        for (i, c) in t.char_indices() {
            let ancho = if c == '\n' { 0.0 } else { 10.0 };
            out.push((
                i,
                RectF {
                    x,
                    y,
                    ancho,
                    alto: 20.0,
                },
            ));
            if c == '\n' {
                x = 0.0;
                y += 20.0;
            } else {
                x += ancho;
            }
        }
        out
    }

    #[test]
    fn un_clic_pone_la_rayita_en_la_letra_pulsada() {
        let t = "hola mundo";
        let c = cajas(t);
        // Mitad izquierda de la «m» (x 50..60): delante de ella.
        assert_eq!(indice_en_punto(t, &c, 52.0, 10.0), 5);
        // Mitad derecha: detras.
        assert_eq!(indice_en_punto(t, &c, 58.0, 10.0), 6);
        // Delante de todo y pasado el final.
        assert_eq!(indice_en_punto(t, &c, -5.0, 10.0), 0);
        assert_eq!(indice_en_punto(t, &c, 500.0, 10.0), t.len());
    }

    #[test]
    fn varios_renglones() {
        let t = "uno\ndos tres\nfin";
        let c = cajas(t);
        // Segundo renglon (y 20..40), sobre la «t» de «tres» (x 40..50).
        assert_eq!(indice_en_punto(t, &c, 41.0, 30.0), 8);
        // Pasado el final del primero: antes de su salto, no en el segundo.
        assert_eq!(indice_en_punto(t, &c, 300.0, 5.0), 3);
        // Tercero, al principio.
        assert_eq!(indice_en_punto(t, &c, 1.0, 45.0), 13);
        // Por debajo del ultimo renglon: su letra mas cercana.
        assert_eq!(indice_en_punto(t, &c, 300.0, 400.0), t.len());
        // Por encima del primero: el primero.
        assert_eq!(indice_en_punto(t, &c, 12.0, -30.0), 1);
    }

    #[test]
    fn un_salto_al_final_deja_un_renglon_vacio_donde_pulsar() {
        let t = "uno\n";
        let c = cajas(t);
        assert_eq!(indice_en_punto(t, &c, 5.0, 30.0), t.len());
        assert_eq!(sitio_del_cursor(t, &c, t.len(), 20.0), (0.0, 20.0, 20.0));
    }

    #[test]
    fn un_clic_fuera_de_la_caja_no_es_del_texto() {
        let t = "hola";
        let c = cajas(t);
        let zona = RectF {
            x: 100.0,
            y: 200.0,
            ancho: 300.0,
            alto: 40.0,
        };
        let origen = (100.0, 210.0);
        assert_eq!(indice_en(t, &c, origen, zona, 50.0, 215.0), None);
        assert_eq!(indice_en(t, &c, origen, zona, 120.0, 260.0), None);
        // Dentro: la «l» (x 120..130 en la ventana) por su mitad derecha.
        assert_eq!(indice_en(t, &c, origen, zona, 127.0, 215.0), Some(3));
        // Dentro de la caja pero lejos de las letras: al final.
        assert_eq!(indice_en(t, &c, origen, zona, 390.0, 215.0), Some(4));
    }

    #[test]
    fn las_cajas_se_piden_en_unidades_utf16() {
        let t = "a😀b";
        let mut pedidas = Vec::new();
        let c = cajas_de_letras(t, |u, n| {
            pedidas.push((u, n));
            Some(RectF {
                x: u as f32,
                y: 0.0,
                ancho: 1.0,
                alto: 1.0,
            })
        });
        assert_eq!(pedidas, vec![(0, 1), (1, 2), (3, 1)]);
        assert_eq!(c.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![0, 1, 5]);
    }

    #[test]
    fn la_rayita_se_dibuja_en_su_sitio() {
        let t = "ab\ncd";
        let c = cajas(t);
        assert_eq!(sitio_del_cursor(t, &c, 1, 20.0), (10.0, 0.0, 20.0));
        assert_eq!(sitio_del_cursor(t, &c, 4, 20.0), (10.0, 20.0, 20.0));
        assert_eq!(sitio_del_cursor(t, &c, t.len(), 20.0), (20.0, 20.0, 20.0));
        assert_eq!(sitio_del_cursor("", &[], 0, 20.0), (0.0, 0.0, 20.0));
    }

    #[test]
    fn escribir_en_medio_y_sobre_lo_seleccionado() {
        let mut t = String::from("hola mundo");
        let mut c = Cursor::default();
        c.poner(&t, 4, false);
        c.escribir(&mut t, ",");
        assert_eq!(t, "hola, mundo");
        assert_eq!(c.pos(&t), 5);
        // Seleccionar «mundo» con Mayusculas y escribir encima.
        c.poner(&t, 6, false);
        c.poner(&t, t.len(), true);
        assert_eq!(c.seleccionado(&t), Some("mundo"));
        c.escribir(&mut t, "gente");
        assert_eq!(t, "hola, gente");
        // Al final, lo de fuera sigue entrando detras.
        assert_eq!(c.pos(&t), t.len());
        t.push('!');
        assert_eq!(c.pos(&t), t.len());
    }

    #[test]
    fn borrar_y_flechas() {
        let mut t = String::from("añadir");
        let mut c = Cursor::default();
        c.izquierda(&t, false, false);
        assert_eq!(c.pos(&t), 6);
        c.inicio(&t, false, false);
        c.derecha(&t, false, false);
        assert_eq!(c.pos(&t), 1);
        c.derecha(&t, false, false);
        assert_eq!(c.pos(&t), 3, "la ñ ocupa dos bytes");
        c.borrar_atras(&mut t, false);
        assert_eq!(t, "aadir");
        c.borrar_delante(&mut t, false);
        assert_eq!(t, "adir");
        c.inicio(&t, false, false);
        c.derecha(&t, true, false);
        c.derecha(&t, true, false);
        assert_eq!(c.seleccionado(&t), Some("ad"));
        // Sin Mayusculas, la flecha suelta la seleccion por su lado.
        c.izquierda(&t, false, false);
        assert_eq!((c.pos(&t), c.seleccion(&t)), (0, None));
        c.fin(&t, false, false);
        assert_eq!(c.pos(&t), t.len());
    }

    #[test]
    fn inicio_y_fin_van_por_renglones() {
        let t = "uno\ndos\ntres";
        let mut c = Cursor::default();
        c.poner(t, 5, false);
        c.inicio(t, false, false);
        assert_eq!(c.pos(t), 4);
        c.fin(t, false, false);
        assert_eq!(c.pos(t), 7);
        c.inicio(t, false, true);
        assert_eq!(c.pos(t), 0);
    }

    #[test]
    fn palabras() {
        let t = "hola, buen dia";
        assert_eq!(&t[palabra_en(t, 7)], "buen");
        assert_eq!(&t[palabra_en(t, 0)], "hola");
        assert_eq!(&t[palabra_en(t, t.len())], "dia");
        assert_eq!(&t[palabra_en(t, 4)], ",");
        let mut s = String::from("hola buen dia");
        let mut c = Cursor::default();
        c.borrar_atras(&mut s, true);
        assert_eq!(s, "hola buen ");
        c.izquierda(&s, false, true);
        assert_eq!(c.pos(&s), 5);
        c.derecha(&s, false, true);
        assert_eq!(c.pos(&s), s.len());
    }

    #[test]
    fn todo_y_soltar() {
        let mut t = String::from("abc");
        let mut c = Cursor::default();
        c.todo(&t);
        assert_eq!(c.seleccionado(&t), Some("abc"));
        c.borrar_atras(&mut t, false);
        assert!(t.is_empty());
        c.soltar();
        assert_eq!(c, Cursor::default());
    }

    #[test]
    fn una_posicion_vieja_no_se_sale_de_un_texto_mas_corto() {
        let mut c = Cursor::default();
        c.poner("hola mundo", 8, false);
        c.ancla = Some(9);
        // El texto cambio por fuera (el dictado, una sincronizacion).
        assert_eq!(c.pos("hola"), 4);
        assert_eq!(c.seleccion("hola"), None);
    }
}
