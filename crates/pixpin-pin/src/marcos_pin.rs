//! **Los marcos numerados de un pin de imagen** (19-oct-2026), con la logica
//! de los del plano y de los demas visores (`marcos_doc` de la app): el
//! marco del riel (o la **I**) entra y sale del modo, arrastrar pone uno con
//! su numero, **Supr** quita el ultimo y **Ctrl+P** los imprime en grupo.
//!
//! El pin solo los ENSENA y recoge el gesto: la lista, su fichero al lado de
//! la imagen y la impresion los lleva el gestor con `marcos_doc::Marcos`
//! (este crate no puede ver la app). Por eso aqui viajan como
//! [`PedidoMarco`] y vuelven con `Pin::poner_marcos`.
//!
//! Las cajas van **en pixeles de la imagen original, sin girar** (los del
//! fichero guardado): asi se quedan pegadas a su sitio de la foto aunque el
//! pin se agrande, se acerque por dentro (Ctrl + rueda) o se gire, y se
//! recortan tal cual del fichero para imprimir a resolucion completa.

use pixpin_render::{Color, Pintor, RectF};

/// Lo que pide el pin al gestor sobre sus marcos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PedidoMarco {
    /// El boton del riel, la I o Esc: entrar o salir del modo.
    Alternar,
    /// Supr en el modo: quitar el ultimo.
    QuitarUltimo,
    /// «Borrar los marcos» del menu: todos.
    Borrar,
    /// Ctrl+P: imprimir todos en grupo (sin marcos, lo de siempre).
    Imprimir,
    /// Se solto un arrastre: sus dos esquinas en pixeles de la imagen y
    /// cuantos pixeles de pantalla mide cada uno (para no tomar un clic por
    /// un marco).
    Nuevo { a: [f64; 2], b: [f64; 2], px: f64 },
}

/// El azul de los marcos del plano (`0x0078D4`, `marcos_doc::AZUL`).
pub(crate) const AZUL: Color = Color {
    r: 0.0,
    g: 0x78 as f32 / 255.0,
    b: 0xD4 as f32 / 255.0,
    a: 1.0,
};
const CRISTAL: Color = Color {
    r: 0.08,
    g: 0.08,
    b: 0.09,
    a: 0.90,
};

/// Como esta puesta la imagen en la tarjeta: el tamano de la imagen SIN
/// girar (las coordenadas de los marcos), el de la tarjeta tal como se ve y
/// el giro y los volteos del pin (los de `Pintor::con_giro`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Encaje {
    pub imagen: (f64, f64),
    pub caja: (f64, f64),
    pub giro: u8,
    pub volteo_h: bool,
    pub volteo_v: bool,
}

impl Encaje {
    fn giro(&self) -> (f64, f64, f64, f64) {
        let (cos, sin) = match self.giro % 4 {
            0 => (1.0, 0.0),
            1 => (0.0, 1.0),
            2 => (-1.0, 0.0),
            _ => (0.0, -1.0),
        };
        let fx = if self.volteo_h { -1.0 } else { 1.0 };
        let fy = if self.volteo_v { -1.0 } else { 1.0 };
        (cos, sin, fx, fy)
    }

    /// Lo que mide la imagen dibujada antes de girarla: la caja, con los
    /// lados cambiados si va girada un cuarto (como la pinta el pin).
    fn dibujada(&self) -> (f64, f64) {
        if self.giro % 2 == 1 {
            (self.caja.1, self.caja.0)
        } else {
            self.caja
        }
    }

    /// Un punto de la imagen llevado a la tarjeta (coordenadas del
    /// contenido, sin el margen de la sombra): voltear y luego girar
    /// alrededor del centro, la misma cuenta que `Pintor::con_giro`.
    pub fn a_caja(&self, p: [f64; 2]) -> (f64, f64) {
        let (cos, sin, fx, fy) = self.giro();
        let (dw, dh) = self.dibujada();
        let dx = (p[0] / self.imagen.0.max(1e-9) - 0.5) * dw;
        let dy = (p[1] / self.imagen.1.max(1e-9) - 0.5) * dh;
        (
            self.caja.0 / 2.0 + fx * cos * dx - fy * sin * dy,
            self.caja.1 / 2.0 + fx * sin * dx + fy * cos * dy,
        )
    }

    /// Y al reves: un punto de la tarjeta, en pixeles de la imagen (dentro
    /// de ella: arrastrar hasta fuera del pin deja el marco en su borde).
    pub fn a_imagen(&self, x: f64, y: f64) -> [f64; 2] {
        let (cos, sin, fx, fy) = self.giro();
        let (dw, dh) = self.dibujada();
        let (gx, gy) = (x - self.caja.0 / 2.0, y - self.caja.1 / 2.0);
        // La inversa de un giro con volteos es su traspuesta.
        let dx = fx * (cos * gx + sin * gy);
        let dy = fy * (-sin * gx + cos * gy);
        [
            ((dx / dw.max(1e-9) + 0.5) * self.imagen.0).clamp(0.0, self.imagen.0),
            ((dy / dh.max(1e-9) + 0.5) * self.imagen.1).clamp(0.0, self.imagen.1),
        ]
    }

    /// La caja de un marco en la tarjeta (un cuarto de vuelta la deja
    /// recta, asi que basta con sus dos esquinas).
    pub fn caja_de(&self, m: [f64; 4]) -> (f64, f64, f64, f64) {
        let (x0, y0) = self.a_caja([m[0], m[1]]);
        let (x1, y1) = self.a_caja([m[2], m[3]]);
        (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
    }
}

/// La caja `[x0, y0, x1, y1]` ordenada de dos esquinas.
pub(crate) fn ordenada(a: [f64; 2], b: [f64; 2]) -> [f64; 4] {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[0].max(b[0]),
        a[1].max(b[1]),
    ]
}

/// **Pinta los marcos** como `marcos_doc::Marcos::pintar`: la raya azul y el
/// numero blanco en su cuadrito azul arriba a la izquierda, y el que se esta
/// arrastrando con la raya fina. `margen` es donde empieza la tarjeta en el
/// pintado y `e` la escala de las rayas y las letras (la del monitor, ya
/// deshecho el zoom de dentro para que no engorden al acercarse).
pub(crate) fn pintar(
    p: &Pintor,
    encaje: &Encaje,
    marcos: &[[f64; 4]],
    nuevo: Option<[f64; 4]>,
    margen: f32,
    e: f32,
) {
    let rect = |m: [f64; 4]| {
        let (x0, y0, x1, y1) = encaje.caja_de(m);
        RectF {
            x: margen + x0 as f32,
            y: margen + y0 as f32,
            ancho: (x1 - x0) as f32,
            alto: (y1 - y0) as f32,
        }
    };
    for (n, m) in marcos.iter().enumerate() {
        let r = rect(*m);
        p.trazar(r, 2.0 * e, AZUL);
        let numero = (n + 1).to_string();
        let tam = 12.0 * e;
        let (w, _) = p.medir_texto(&numero, tam);
        p.rellenar(
            RectF {
                x: r.x,
                y: r.y,
                ancho: w + 12.0 * e,
                alto: 20.0 * e,
            },
            AZUL,
        );
        p.texto(&numero, r.x + 6.0 * e, r.y + 3.0 * e, tam, Color::BLANCO);
    }
    if let Some(m) = nuevo {
        p.trazar(rect(m), 1.5 * e, AZUL);
    }
}

/// Lado de una ficha de marco, en pixeles logicos (algo menos que las de
/// los visores: los pines suelen ser pequenos).
const LADO_FICHA: f32 = 24.0;
const HUECO_FICHA: f32 = 6.0;
const MARGEN_FICHA: f32 = 8.0;

/// **Las fichas de los marcos** con el modo puesto (19-oct-2026, el usuario:
/// «en modo marcos me aparecen los numeros abajo; le doy a un numero y me
/// lleva directo a ese marco»): una por marco, en su orden, abajo a la
/// izquierda de la tarjeta `caja`, como `marcos_doc::Marcos::fichas`. En un
/// pin estrecho pasan a otra fila; si aun asi taparian mas de un tercio de
/// la foto, se quedan las que caben.
pub(crate) fn fichas(caja: RectF, n: usize, e: f32) -> Vec<RectF> {
    let lado = LADO_FICHA * e;
    let paso = lado + HUECO_FICHA * e;
    let margen = MARGEN_FICHA * e;
    let por_fila = (((caja.ancho - 2.0 * margen + HUECO_FICHA * e) / paso).floor() as usize).max(1);
    let filas_max = (((caja.alto / 3.0) / paso).floor() as usize).max(1);
    let n = n.min(por_fila * filas_max);
    let filas = n.div_ceil(por_fila);
    (0..n)
        .map(|k| {
            let (fila, col) = (k / por_fila, k % por_fila);
            RectF {
                x: caja.x + margen + col as f32 * paso,
                // La primera fila arriba: se leen 1, 2, 3… como un texto.
                y: caja.y + caja.alto - margen - lado - (filas - 1 - fila) as f32 * paso,
                ancho: lado,
                alto: lado,
            }
        })
        .collect()
}

/// La ficha bajo `(x, y)` (las coordenadas de `caja`): el marco al que ir,
/// desde cero.
pub(crate) fn ficha_en(caja: RectF, n: usize, e: f32, x: f32, y: f32) -> Option<usize> {
    fichas(caja, n, e)
        .iter()
        .position(|r| x >= r.x && x <= r.x + r.ancho && y >= r.y && y <= r.y + r.alto)
}

/// **El zoom de dentro que lleva al marco** `caja_marco` (`(x0, y0, x1, y1)`
/// en la tarjeta, sin zoom) en una tarjeta de `ancho` x `alto`: la escala
/// para que llene el pin con un poco de aire (como
/// `marcos_doc::px_para_llenar`) y el desplazamiento que lo centra, lo que
/// pinta `d + p * escala`. Quien lo usa aun lo pega a los bordes.
pub(crate) fn vista_del_marco(
    caja_marco: (f64, f64, f64, f64),
    ancho: f32,
    alto: f32,
) -> (f32, f32, f32) {
    let (x0, y0, x1, y1) = caja_marco;
    let (bw, bh) = ((x1 - x0).max(1e-6) as f32, (y1 - y0).max(1e-6) as f32);
    let escala = ((ancho * 0.94) / bw)
        .min((alto * 0.94) / bh)
        .clamp(1.0, 40.0);
    let (cx, cy) = (((x0 + x1) / 2.0) as f32, ((y0 + y1) / 2.0) as f32);
    (escala, ancho / 2.0 - cx * escala, alto / 2.0 - cy * escala)
}

/// Pinta las fichas (azules, con su numero en blanco).
fn pintar_fichas(p: &Pintor, fichas: &[RectF], e: f32) {
    for (k, r) in fichas.iter().enumerate() {
        p.rellenar_redondeado(*r, 6.0 * e, AZUL);
        let numero = (k + 1).to_string();
        let tam = 12.0 * e;
        let (w, h) = p.medir_texto(&numero, tam);
        p.texto(
            &numero,
            r.x + (r.ancho - w) / 2.0,
            r.y + (r.alto - h) / 2.0,
            tam,
            Color::BLANCO,
        );
    }
}

/// **El modo de los marcos, abajo a la izquierda de la tarjeta `caja`**: las
/// fichas de los `n` marcos y la pista. La pista va a la derecha de las
/// fichas si cabe en una linea (como en los visores); si no, encima. En un
/// pin estrecho no se corta por el borde: se parte en renglones y, si aun
/// asi taparia mas de un tercio de la foto, se quedan solo sus primeras
/// partes (las separa « · »: arrastrar, Ctrl+P, Supr, I) hasta que quepa.
pub(crate) fn pintar_pista(p: &Pintor, caja: RectF, texto: &str, n: usize, e: f32) {
    let fichas = fichas(caja, n, e);
    p.con_recorte(caja, |p| pintar_fichas(p, &fichas, e));
    if texto.is_empty() {
        return;
    }
    let tam = 12.0 * e;
    // A la derecha de las fichas, si son una fila y la pista entera cabe.
    if let Some(ultima) = fichas.last()
        && fichas.iter().all(|r| r.y == ultima.y)
    {
        let x0 = ultima.x + ultima.ancho + 10.0 * e;
        let (w, h) = p.medir_texto(texto, tam);
        if x0 + w + 24.0 * e <= caja.x + caja.ancho - 8.0 * e {
            let fondo = RectF {
                x: x0,
                y: ultima.y + (ultima.alto - h - 12.0 * e) / 2.0,
                ancho: w + 24.0 * e,
                alto: h + 12.0 * e,
            };
            p.con_recorte(caja, |p| {
                p.rellenar_redondeado(fondo, fondo.alto / 2.0, CRISTAL);
                p.texto(
                    texto,
                    fondo.x + 12.0 * e,
                    fondo.y + 6.0 * e,
                    tam,
                    Color::BLANCO,
                );
            });
            return;
        }
    }
    // Encima de las fichas (o abajo del todo, sin ellas).
    let reservado = fichas
        .first()
        .map_or(0.0, |r| caja.y + caja.alto - r.y + 4.0 * e);
    let ancho_max = (caja.ancho - 16.0 * e - 24.0 * e).max(40.0 * e);
    let partes: Vec<&str> = texto.split(" · ").collect();
    let tope = caja.alto / 3.0;
    let mut n = partes.len();
    let (mut corto, mut medida) = (
        texto.to_string(),
        p.medir_texto_ajustado(texto, tam, ancho_max),
    );
    while n > 1 && medida.1 + 12.0 * e > tope {
        n -= 1;
        corto = partes[..n].join(" · ");
        medida = p.medir_texto_ajustado(&corto, tam, ancho_max);
    }
    let (w, h) = medida;
    let fondo = RectF {
        x: caja.x + 8.0 * e,
        y: caja.y + caja.alto - reservado - h - 20.0 * e,
        ancho: w + 24.0 * e,
        alto: h + 12.0 * e,
    };
    // Una sola linea, pildora; varias, esquinas redondeadas sin mas.
    let radio = if h <= tam * 1.6 {
        fondo.alto / 2.0
    } else {
        8.0 * e
    };
    p.con_recorte(caja, |p| {
        p.rellenar_redondeado(fondo, radio, CRISTAL);
        p.texto_ajustado(
            &corto,
            fondo.x + 12.0 * e,
            fondo.y + 6.0 * e,
            tam,
            ancho_max,
            Color::BLANCO,
        );
    });
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn encaje(giro: u8, h: bool, v: bool) -> Encaje {
        // Una foto de 400 x 200 ensenada a la mitad (girada, 100 x 200).
        let caja = if giro % 2 == 1 {
            (100.0, 200.0)
        } else {
            (200.0, 100.0)
        };
        Encaje {
            imagen: (400.0, 200.0),
            caja,
            giro,
            volteo_h: h,
            volteo_v: v,
        }
    }

    fn cerca(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn sin_giro_el_marco_se_escala_con_el_pin() {
        let e = encaje(0, false, false);
        assert!(cerca(e.a_caja([100.0, 50.0]), (50.0, 25.0)));
        assert_eq!(e.a_imagen(50.0, 25.0), [100.0, 50.0]);
        // El pin al doble: el mismo pixel de la foto, el doble de lejos.
        let doble = Encaje {
            caja: (400.0, 200.0),
            ..e
        };
        assert!(cerca(doble.a_caja([100.0, 50.0]), (100.0, 50.0)));
    }

    #[test]
    fn girado_un_cuarto_la_esquina_de_arriba_a_la_izquierda_va_arriba_a_la_derecha() {
        // Como `con_giro` con un cuarto a la derecha: (0, 0) de la foto cae
        // en la esquina de arriba a la derecha de la tarjeta.
        let e = encaje(1, false, false);
        assert!(
            cerca(e.a_caja([0.0, 0.0]), (100.0, 0.0)),
            "{:?}",
            e.a_caja([0.0, 0.0])
        );
        assert!(cerca(e.a_caja([400.0, 200.0]), (0.0, 200.0)));
    }

    #[test]
    fn ida_y_vuelta_con_cualquier_giro_y_volteo() {
        for giro in 0..4 {
            for (h, v) in [(false, false), (true, false), (false, true), (true, true)] {
                let e = encaje(giro, h, v);
                let p = [123.0, 45.0];
                let (x, y) = e.a_caja(p);
                let q = e.a_imagen(x, y);
                assert!(
                    (q[0] - p[0]).abs() < 1e-9 && (q[1] - p[1]).abs() < 1e-9,
                    "giro {giro} h {h} v {v}: {q:?}"
                );
                // La caja de un marco sigue recta y dentro de la tarjeta.
                let (x0, y0, x1, y1) = e.caja_de([0.0, 0.0, 400.0, 200.0]);
                assert!(cerca((x0, y0), (0.0, 0.0)) && cerca((x1, y1), e.caja));
            }
        }
    }

    #[test]
    fn las_fichas_van_abajo_a_la_izquierda_y_pasan_de_fila_en_un_pin_estrecho() {
        let ancho = RectF {
            x: 0.0,
            y: 0.0,
            ancho: 600.0,
            alto: 400.0,
        };
        let f = fichas(ancho, 3, 1.0);
        assert_eq!(f.len(), 3);
        assert_eq!((f[0].x, f[0].y), (8.0, 400.0 - 8.0 - 24.0));
        assert_eq!(f[1].x, 8.0 + 30.0);
        assert_eq!(ficha_en(ancho, 3, 1.0, 40.0, 380.0), Some(1));
        // Caso negativo: fuera de las fichas, ninguna (el clic pone un marco).
        assert_eq!(ficha_en(ancho, 3, 1.0, 300.0, 200.0), None);
        // Un pin de 110 px: 3 por fila; la 4 en la fila de abajo.
        let estrecho = RectF {
            x: 0.0,
            y: 0.0,
            ancho: 110.0,
            alto: 300.0,
        };
        let f = fichas(estrecho, 4, 1.0);
        assert_eq!(f.len(), 4);
        assert!(f[0].y < f[3].y, "la primera fila arriba");
        assert_eq!(f[3].x, 8.0);
        // Sin marcos, ninguna.
        assert!(fichas(ancho, 0, 1.0).is_empty());
    }

    #[test]
    fn ir_a_un_marco_lo_centra_y_lo_hace_llenar_el_pin() {
        // Un marco de 100 x 50 en una tarjeta de 400 x 200: escala 3,76.
        let (escala, dx, dy) = vista_del_marco((100.0, 50.0, 200.0, 100.0), 400.0, 200.0);
        assert!((escala - 3.76).abs() < 1e-4, "{escala}");
        // Su centro (150, 75) cae en el centro de la tarjeta.
        assert!(
            (dx + 150.0 * escala - 200.0).abs() < 1e-3 && (dy + 75.0 * escala - 100.0).abs() < 1e-3
        );
        // Caso negativo: un marco tan grande como el pin no aleja (minimo 1).
        assert_eq!(
            vista_del_marco((0.0, 0.0, 400.0, 200.0), 400.0, 200.0).0,
            1.0
        );
    }

    #[test]
    fn caso_negativo_arrastrar_fuera_del_pin_se_queda_en_el_borde() {
        let e = encaje(0, false, false);
        assert_eq!(e.a_imagen(-30.0, 500.0), [0.0, 200.0]);
        assert_eq!(ordenada([10.0, 40.0], [5.0, 2.0]), [5.0, 2.0, 10.0, 40.0]);
    }
}
