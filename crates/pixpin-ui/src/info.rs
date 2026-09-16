//! La pantalla de informacion de un proyecto: lo que sale al pulsar su
//! nombre en la cabecera.
//!
//! Geometria pura, como `chat` e `historial`. Las medidas salen del analisis
//! de Telegram Desktop; **de ahi solo medidas y tecnicas, nunca su codigo,
//! que es GPL-3.0.**
//!
//! Las SECCIONES, en cambio, son las de PixPin Android y viven en
//! `pixpin_proyecto::cuaderno::Seccion`: todo, fotos, archivos, voz, dibujos,
//! fijados y buzon. Telegram tiene otras (musica, enlaces, GIF) que aqui no
//! significan nada. La disposicion es la suya; lo que se lista es lo nuestro.
//!
//! Tres decisiones que no son evidentes:
//!
//! - **La pildora de la pestana activa NO es una rayita.** En Telegram es una
//!   pildora rellena que se desliza Y cambia de ancho entre pestana y
//!   pestana. Una rayita es mas facil y se ve distinto.
//! - **La cuadricula se centra.** Se calcula cuantas columnas caben con la
//!   celda minima, se reparte el sobrante entre ellas y el margen se recalcula
//!   para centrarla. Sin eso sobra un pico a la derecha que canta.
//! - **Las celdas son cuadradas.** Una foto apaisada y una vertical ocupan lo
//!   mismo; es lo que hace que una cuadricula se lea de un vistazo.

use pixpin_geom::{Punto, Rect};

/// Medidas en pixeles logicos (al 100 %).
/// La columna de informacion, a la derecha del todo.
pub const ANCHO_MINIMO: u32 = 292;
pub const ANCHO_MAXIMO: u32 = 392;
/// Por debajo de este ancho de ventana no cabe como columna y se ensena a
/// pantalla completa, tapando la conversacion.
pub const ANCHO_TRES_COLUMNAS: u32 = 932;
/// La cabecera del panel, con su boton de volver.
pub const CABECERA: u32 = 54;
pub const VOLVER_ANCHO: u32 = 60;
pub const TITULO_TAM: f32 = 14.0;

/// La ficha de arriba: avatar grande, nombre y estado.
pub const FICHA_ALTO: u32 = 108;
pub const FICHA_AVATAR: u32 = 72;
pub const FICHA_AVATAR_X: u32 = 19;
pub const FICHA_AVATAR_Y: u32 = 18;
pub const FICHA_TEXTO_X: u32 = 109;
pub const FICHA_NOMBRE_Y: u32 = 32;
pub const FICHA_ESTADO_Y: u32 = 58;
pub const FICHA_NOMBRE_TAM: f32 = 16.0;
pub const FICHA_ESTADO_TAM: f32 = 13.0;

/// La tira de pestanas: 48 de alto, con una «isla» de 36 dentro.
pub const TIRA_ALTO: u32 = 48;
pub const TIRA_MARGEN_X: u32 = 8;
pub const TIRA_MARGEN_Y: u32 = 6;
/// Lo que se separa la pildora del borde de la isla.
pub const PILDORA_HUECO: u32 = 4;
/// Relleno a cada lado del texto de una pestana.
pub const PESTANA_RELLENO: u32 = 16;
pub const PESTANA_TAM: f32 = 13.0;
/// Lo que tarda la pildora en deslizarse de una pestana a otra.
pub const PILDORA_MS: u32 = 200;

/// La cuadricula de fotos.
pub const CELDA_MINIMA: u32 = 82;
pub const CELDA_HUECO: u32 = 2;
pub const REJILLA_MARGEN: u32 = 3;

/// Las filas de archivo: miniatura de 70 con 3 de aire arriba y abajo.
pub const ARCHIVO_ALTO: u32 = 77;
pub const ARCHIVO_MINIATURA: u32 = 70;
pub const ARCHIVO_MARGEN_X: u32 = 14;
pub const ARCHIVO_NOMBRE_Y: u32 = 7;
pub const ARCHIVO_ESTADO_Y: u32 = 24;
pub const ARCHIVO_FECHA_Y: u32 = 49;
pub const ARCHIVO_NOMBRE_TAM: f32 = 13.0;
pub const ARCHIVO_ESTADO_TAM: f32 = 13.0;

/// Como queda repartido el panel.
///
/// Que secciones hay y como se llaman vive en `pixpin_proyecto::cuaderno`:
/// son del dominio, no de la interfaz. Aqui solo se reparten rectangulos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disposicion {
    /// Todo el panel.
    pub panel: Rect,
    pub cabecera: Rect,
    /// El boton de volver, dentro de la cabecera.
    pub volver: Rect,
    pub ficha: Rect,
    /// La tira de pestanas entera.
    pub tira: Rect,
    /// La «isla» de dentro de la tira, que es lo que lleva fondo.
    pub isla: Rect,
    /// Lo que queda debajo para el contenido.
    pub contenido: Rect,
    /// El panel ocupa la ventana entera porque no cabia como columna.
    pub pantalla_completa: bool,
}

/// El ancho que le toca al panel en una ventana de `ancho_ventana`.
pub fn ancho_panel(ancho_ventana: u32, escala_por_cien: u32) -> u32 {
    let e = |v: u32| v * escala_por_cien / 100;
    if ancho_ventana < e(ANCHO_TRES_COLUMNAS) {
        // No cabe de columna: se lo queda todo. Mejor eso que una columna de
        // cien pixeles donde no se lee nada.
        return ancho_ventana;
    }
    e(ANCHO_MINIMO).max(e(ANCHO_MAXIMO).min(ancho_ventana / 3))
}

impl Disposicion {
    /// Reparte el panel dentro de `hueco`, que es la columna de la derecha o
    /// la ventana entera.
    pub fn calcular(hueco: Rect, pantalla_completa: bool, escala_por_cien: u32) -> Disposicion {
        let e = |v: u32| v * escala_por_cien / 100;
        let cabecera = Rect {
            alto: e(CABECERA).min(hueco.alto),
            ..hueco
        };
        let ficha = Rect {
            x: hueco.x,
            y: cabecera.abajo(),
            ancho: hueco.ancho,
            alto: e(FICHA_ALTO).min(hueco.alto.saturating_sub(cabecera.alto)),
        };
        let tira = Rect {
            x: hueco.x,
            y: ficha.abajo(),
            ancho: hueco.ancho,
            alto: e(TIRA_ALTO).min(hueco.alto.saturating_sub(cabecera.alto + ficha.alto)),
        };
        let isla = Rect {
            x: tira.x + e(TIRA_MARGEN_X) as i32,
            y: tira.y + e(TIRA_MARGEN_Y) as i32,
            ancho: tira.ancho.saturating_sub(2 * e(TIRA_MARGEN_X)),
            alto: tira.alto.saturating_sub(2 * e(TIRA_MARGEN_Y)),
        };
        let arriba = tira.abajo();
        Disposicion {
            panel: hueco,
            cabecera,
            volver: Rect {
                x: cabecera.x,
                y: cabecera.y,
                ancho: e(VOLVER_ANCHO).min(cabecera.ancho),
                alto: cabecera.alto,
            },
            ficha,
            tira,
            isla,
            contenido: Rect {
                x: hueco.x,
                y: arriba,
                ancho: hueco.ancho,
                alto: (hueco.abajo() - arriba).max(0) as u32,
            },
            pantalla_completa,
        }
    }

    /// Donde cae cada pestana dentro de la isla, en el mismo orden que se le
    /// pasan los anchos de texto (que los mide quien pinta).
    ///
    /// Si no caben todas, se reparte el hueco a partes iguales: es preferible
    /// apretarlas a que la ultima se salga.
    pub fn pestanas(&self, anchos_de_texto: &[f32], escala_por_cien: u32) -> Vec<Rect> {
        let e = |v: u32| v * escala_por_cien / 100;
        if anchos_de_texto.is_empty() || self.isla.ancho == 0 {
            return Vec::new();
        }
        let relleno = 2.0 * e(PESTANA_RELLENO) as f32;
        let naturales: Vec<f32> = anchos_de_texto.iter().map(|w| w + relleno).collect();
        let total: f32 = naturales.iter().sum();
        let cabe = total <= self.isla.ancho as f32;

        let mut x = self.isla.x as f32;
        naturales
            .iter()
            .map(|natural| {
                let ancho = if cabe {
                    *natural
                } else {
                    self.isla.ancho as f32 / naturales.len() as f32
                };
                let r = Rect {
                    x: x.round() as i32,
                    y: self.isla.y,
                    ancho: ancho.round().max(0.0) as u32,
                    alto: self.isla.alto,
                };
                x += ancho;
                r
            })
            .collect()
    }

    /// La pildora de la pestana activa: su rectangulo, ya encogido.
    pub fn pildora(&self, pestana: Rect, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        Rect {
            x: pestana.x,
            y: pestana.y + e(PILDORA_HUECO) as i32,
            ancho: pestana.ancho,
            alto: pestana.alto.saturating_sub(2 * e(PILDORA_HUECO)),
        }
    }

    /// Que pestana hay bajo el punto.
    pub fn pestana_en(&self, p: Punto, pestanas: &[Rect]) -> Option<usize> {
        pestanas.iter().position(|r| r.contiene(p))
    }
}

/// Como queda la cuadricula de fotos en un ancho dado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rejilla {
    pub columnas: u32,
    /// El lado de una celda: son cuadradas.
    pub lado: u32,
    /// Donde empieza la primera columna, ya centrada.
    pub margen: u32,
}

/// Reparte el ancho en columnas cuadradas.
pub fn rejilla(ancho: u32, escala_por_cien: u32) -> Rejilla {
    let e = |v: u32| v * escala_por_cien / 100;
    let hueco = e(CELDA_HUECO);
    let minima = e(CELDA_MINIMA).max(1);
    let util = ancho.saturating_sub(2 * e(REJILLA_MARGEN));
    // Cuantas caben, contando que entre dos hay un hueco.
    let columnas = ((util + hueco) / (minima + hueco)).max(1);
    let lado = ((util + hueco) / columnas).saturating_sub(hueco).max(1);
    let ocupa = columnas * lado + (columnas - 1) * hueco;
    Rejilla {
        columnas,
        lado,
        margen: ancho.saturating_sub(ocupa) / 2,
    }
}

impl Rejilla {
    /// La celda numero `indice`, con el desplazamiento ya restado.
    pub fn celda(&self, indice: usize, area: Rect, scroll: i32, escala_por_cien: u32) -> Rect {
        let hueco = CELDA_HUECO * escala_por_cien / 100;
        let fila = indice as u32 / self.columnas;
        let columna = indice as u32 % self.columnas;
        Rect {
            x: area.x + self.margen as i32 + (columna * (self.lado + hueco)) as i32,
            y: area.y + (fila * (self.lado + hueco)) as i32 - scroll,
            ancho: self.lado,
            alto: self.lado,
        }
    }

    /// Lo que ocupa todo junto.
    pub fn alto_total(&self, cuantas: usize, escala_por_cien: u32) -> u32 {
        if cuantas == 0 {
            return 0;
        }
        let hueco = CELDA_HUECO * escala_por_cien / 100;
        let filas = cuantas.div_ceil(self.columnas as usize) as u32;
        filas * self.lado + (filas - 1) * hueco
    }

    /// Que celdas se ven, para pintar solo esas.
    pub fn visibles(
        &self,
        area: Rect,
        cuantas: usize,
        scroll: i32,
        escala_por_cien: u32,
    ) -> (usize, usize) {
        let hueco = CELDA_HUECO * escala_por_cien / 100;
        let paso = (self.lado + hueco).max(1) as i32;
        let primera_fila = (scroll / paso).max(0) as usize;
        let filas_que_caben = (area.alto as i32 / paso + 2) as usize;
        let primera = primera_fila * self.columnas as usize;
        let cuantas_ver = filas_que_caben * self.columnas as usize;
        (
            primera.min(cuantas),
            cuantas_ver.min(cuantas.saturating_sub(primera)),
        )
    }
}

/// La fila de un archivo, con su miniatura y sus tres lineas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilaArchivo {
    pub fila: Rect,
    pub miniatura: Rect,
    pub nombre: Punto,
    pub estado: Punto,
    pub fecha: Punto,
    pub ancho_texto: u32,
}

/// Coloca lo de dentro de una fila de archivo.
pub fn fila_archivo(fila: Rect, escala_por_cien: u32) -> FilaArchivo {
    let e = |v: u32| v * escala_por_cien / 100;
    let miniatura = e(ARCHIVO_MINIATURA);
    let x = fila.x + e(ARCHIVO_MARGEN_X) as i32;
    let texto_x = x + miniatura as i32 + e(ARCHIVO_MARGEN_X) as i32;
    let derecha = fila.derecha() - e(ARCHIVO_MARGEN_X) as i32;
    FilaArchivo {
        fila,
        miniatura: Rect {
            x,
            y: fila.y + e(3) as i32,
            ancho: miniatura,
            alto: miniatura,
        },
        nombre: Punto {
            x: texto_x,
            y: fila.y + e(ARCHIVO_NOMBRE_Y) as i32,
        },
        estado: Punto {
            x: texto_x,
            y: fila.y + e(ARCHIVO_ESTADO_Y) as i32,
        },
        fecha: Punto {
            x: texto_x,
            y: fila.y + e(ARCHIVO_FECHA_Y) as i32,
        },
        ancho_texto: (derecha - texto_x).max(0) as u32,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn hueco() -> Rect {
        Rect {
            x: 700,
            y: 24,
            ancho: 392,
            alto: 700,
        }
    }

    #[test]
    fn el_panel_es_columna_en_una_ventana_ancha_y_lo_ocupa_todo_en_una_estrecha() {
        let ancho = ancho_panel(1400, 100);
        assert!((ANCHO_MINIMO..=ANCHO_MAXIMO).contains(&ancho), "{ancho}");
        // Justo en el umbral ya cabe como columna.
        assert!(ancho_panel(ANCHO_TRES_COLUMNAS, 100) < ANCHO_TRES_COLUMNAS);
        // Estrecha: no cabe de columna y se lo queda todo.
        assert_eq!(ancho_panel(800, 100), 800);
        // Y escala con el DPI.
        assert!(ancho_panel(2800, 200) >= ANCHO_MINIMO * 2);
    }

    #[test]
    fn las_piezas_del_panel_van_en_orden_y_sin_solaparse() {
        let d = Disposicion::calcular(hueco(), false, 100);
        assert_eq!(d.cabecera.y, hueco().y);
        assert_eq!(d.ficha.y, d.cabecera.abajo());
        assert_eq!(d.tira.y, d.ficha.abajo());
        assert_eq!(d.contenido.y, d.tira.abajo());
        assert_eq!(d.contenido.abajo(), hueco().abajo());
        assert!(d.contenido.alto > 0);
        // La isla vive dentro de la tira, con su aire.
        assert!(d.isla.y > d.tira.y);
        assert!(d.isla.abajo() < d.tira.abajo());
        assert_eq!(d.isla.alto, TIRA_ALTO - 2 * TIRA_MARGEN_Y);
    }

    #[test]
    fn las_pestanas_se_reparten_y_se_aprietan_si_no_caben() {
        let d = Disposicion::calcular(hueco(), false, 100);
        // Holgadas: cada una con su ancho natural.
        let anchos = [40.0, 50.0];
        let p = d.pestanas(&anchos, 100);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].ancho, 40 + 2 * PESTANA_RELLENO);
        assert_eq!(p[1].x, p[0].derecha(), "pegadas, sin hueco entre ellas");

        // Las siete de verdad no caben en 392: se reparten a partes iguales
        // en vez de dejar que la ultima se salga.
        let muchas: Vec<f32> = (0..7).map(|_| 60.0).collect();
        let p = d.pestanas(&muchas, 100);
        assert_eq!(p.len(), 7);
        assert!(p[6].derecha() <= d.isla.derecha() + 1, "{:?}", p[6]);
        assert!(p.windows(2).all(|w| w[0].ancho == w[1].ancho));
    }

    #[test]
    fn la_pildora_se_encoge_dentro_de_su_pestana() {
        let d = Disposicion::calcular(hueco(), false, 100);
        let p = d.pestanas(&[40.0], 100);
        let pildora = d.pildora(p[0], 100);
        assert_eq!(pildora.ancho, p[0].ancho, "de ancho, la misma");
        assert_eq!(pildora.alto, p[0].alto - 2 * PILDORA_HUECO);
        assert!(pildora.y > p[0].y && pildora.abajo() < p[0].abajo());
        // Y se acierta donde se pulsa.
        let dentro = Punto {
            x: p[0].x + 5,
            y: p[0].y + 5,
        };
        assert_eq!(d.pestana_en(dentro, &p), Some(0));
        let fuera = Punto {
            x: p[0].x + 5,
            y: d.contenido.y + 5,
        };
        assert_eq!(d.pestana_en(fuera, &p), None);
    }

    #[test]
    fn la_cuadricula_reparte_el_sobrante_y_queda_centrada() {
        // Con el panel a 392 caben cuatro columnas.
        let r = rejilla(392, 100);
        assert_eq!(r.columnas, 4);
        assert!(
            r.lado >= CELDA_MINIMA,
            "ninguna celda por debajo del minimo"
        );
        // Lo que ocupa, mas los dos margenes, es el ancho entero (o uno menos
        // por el redondeo): no sobra un pico a la derecha.
        let ocupa = r.columnas * r.lado + (r.columnas - 1) * CELDA_HUECO;
        assert!(392 - (ocupa + 2 * r.margen) <= 1, "sobra demasiado");

        // En un panel estrechisimo queda una sola columna, no cero.
        let r = rejilla(60, 100);
        assert_eq!(r.columnas, 1);
        assert!(r.lado >= 1);
    }

    #[test]
    fn de_mil_fotos_solo_se_pintan_las_que_entran() {
        let d = Disposicion::calcular(hueco(), false, 100);
        let r = rejilla(d.contenido.ancho, 100);
        let (primera, cuantas) = r.visibles(d.contenido, 1000, 0, 100);
        assert_eq!(primera, 0);
        assert!(cuantas < 60, "demasiadas: {cuantas}");

        // Bajando, se empieza por otra fila entera.
        let paso = (r.lado + CELDA_HUECO) as i32;
        let (primera, _) = r.visibles(d.contenido, 1000, paso * 3, 100);
        assert_eq!(primera, 3 * r.columnas as usize, "filas enteras");

        // Las celdas de una fila van en linea y las de la siguiente, debajo.
        let a = r.celda(0, d.contenido, 0, 100);
        let b = r.celda(1, d.contenido, 0, 100);
        let c = r.celda(r.columnas as usize, d.contenido, 0, 100);
        assert_eq!(a.y, b.y);
        assert_eq!(b.x - a.x, (r.lado + CELDA_HUECO) as i32);
        assert_eq!(c.x, a.x);
        assert!(c.y > a.y);
        assert_eq!(a.ancho, a.alto, "cuadradas");
    }

    #[test]
    fn sin_fotos_la_cuadricula_no_ocupa_nada() {
        let r = rejilla(392, 100);
        assert_eq!(r.alto_total(0, 100), 0);
        let d = Disposicion::calcular(hueco(), false, 100);
        assert_eq!(r.visibles(d.contenido, 0, 0, 100), (0, 0));
        // Una sola foto ocupa una fila.
        assert_eq!(r.alto_total(1, 100), r.lado);
    }

    #[test]
    fn la_fila_de_un_archivo_tiene_sus_tres_lineas_en_su_sitio() {
        let fila = Rect {
            x: 700,
            y: 200,
            ancho: 392,
            alto: ARCHIVO_ALTO,
        };
        let f = fila_archivo(fila, 100);
        assert_eq!(f.miniatura.ancho, ARCHIVO_MINIATURA);
        assert_eq!(f.miniatura.alto, ARCHIVO_MINIATURA);
        assert!(f.miniatura.abajo() <= fila.abajo(), "cabe en su fila");
        // Las tres lineas, de arriba abajo y a la derecha de la miniatura.
        assert!(f.nombre.y < f.estado.y && f.estado.y < f.fecha.y);
        assert_eq!(f.nombre.x, f.estado.x);
        assert!(f.nombre.x >= f.miniatura.derecha());
        assert!(f.ancho_texto > 0);
    }
}
