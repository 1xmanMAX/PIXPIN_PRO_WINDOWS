//! La ventana de chat: dos columnas, como Telegram Desktop.
//!
//! Geometria pura. Las medidas salen de
//! `docs/investigacion/2026-09-15-telegram-desktop-estructura.md`, que las
//! midio en el codigo de Telegram Desktop. **De ahi solo se toman medidas,
//! colores y tecnicas: su codigo es GPL-3.0 y aqui no hay ni una linea
//! suya.**
//!
//! En PixPin cada chat es un PROYECTO: la columna de la izquierda lista los
//! proyectos y la de la derecha ensena lo que tiene dentro (hojas, lienzos,
//! notas y archivos) como mensajes.
//!
//! Reglas que copia de Telegram, y por que:
//! - La lista tiene un ancho minimo y otro maximo: mas estrecha no cabe el
//!   nombre con la hora, y mas ancha roba sitio al contenido.
//! - Por debajo de cierto ancho de ventana solo cabe una columna, y se
//!   ensena la lista o el chat, no las dos a medias.
//! - Arrastrando el asa hasta muy estrecho, la lista se PLIEGA a solo
//!   avatares en vez de quedarse en un ancho inservible.

use pixpin_geom::{Punto, Rect};

/// Medidas en pixeles logicos (al 100 %).
pub const ANCHO_MINIMO_VENTANA: u32 = 380;
pub const ALTO_MINIMO_VENTANA: u32 = 480;
pub const LISTA_MINIMA: u32 = 260;
pub const LISTA_MAXIMA: u32 = 540;
/// Proporcion con la que nace la lista: 5/14 del ancho, como Telegram.
pub const LISTA_PROPORCION: (u32, u32) = (5, 14);
pub const CHAT_MINIMO: u32 = 380;
/// Con menos ancho que esto no caben las dos columnas.
pub const ANCHO_DOS_COLUMNAS: u32 = 640;
/// Arrastrar el asa por debajo de esto pliega la lista.
pub const PLEGAR_BAJO: u32 = 130;
/// La lista plegada: solo los avatares.
pub const LISTA_PLEGADA: u32 = 66;
pub const ASA: u32 = 6;
/// Cabecera de la lista y del chat.
pub const CABECERA: u32 = 54;
/// Fila de la lista de chats.
pub const FILA: u32 = 62;
pub const AVATAR: u32 = 46;
pub const AVATAR_X: u32 = 10;
pub const AVATAR_Y: u32 = 8;
/// Donde empiezan el nombre y el ultimo mensaje dentro de la fila.
pub const TEXTO_X: u32 = 68;
pub const NOMBRE_Y: u32 = 10;
pub const RESUMEN_Y: u32 = 34;
pub const TEXTO_TAM: f32 = 13.0;
/// El contador de pendientes: pildora de 19 de alto con texto de 12.
pub const CONTADOR_ALTO: u32 = 19;
pub const CONTADOR_TAM: f32 = 12.0;
/// Margen a la derecha de la fila para la hora y el contador.
pub const MARGEN_DERECHO: u32 = 10;

/// Que se ve cuando solo cabe una columna.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vista {
    /// Las dos columnas a la vez.
    Ambas,
    /// Solo la lista de proyectos.
    SoloLista,
    /// Solo el proyecto abierto.
    SoloChat,
}

/// Como queda repartida la ventana.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disposicion {
    /// La columna de proyectos (vacia si no se ve).
    pub lista: Rect,
    /// La cabecera de la lista, dentro de `lista`.
    pub cabecera_lista: Rect,
    /// El area de las filas, bajo la cabecera.
    pub filas: Rect,
    /// La columna del proyecto abierto (vacia si no se ve).
    pub chat: Rect,
    pub cabecera_chat: Rect,
    /// Donde se agarra para cambiar el ancho de la lista.
    pub asa: Rect,
    /// La lista esta plegada a solo avatares.
    pub plegada: bool,
    /// Solo cabe una columna.
    pub una_columna: bool,
}

fn vacio() -> Rect {
    Rect {
        x: 0,
        y: 0,
        ancho: 0,
        alto: 0,
    }
}

/// El ancho con el que nace la lista en una ventana de `ancho` (px fisicos).
pub fn ancho_inicial(ancho: u32, escala_por_cien: u32) -> u32 {
    let e = |v: u32| v * escala_por_cien / 100;
    (ancho * LISTA_PROPORCION.0 / LISTA_PROPORCION.1).clamp(e(LISTA_MINIMA), e(LISTA_MAXIMA))
}

/// Ajusta el ancho pedido al arrastrar el asa: lo sujeta entre el minimo y
/// el maximo, y lo pliega si se pide demasiado estrecho.
pub fn ancho_ajustado(pedido: i32, ancho_ventana: u32, escala_por_cien: u32) -> u32 {
    let e = |v: u32| (v * escala_por_cien / 100) as i32;
    if pedido < e(PLEGAR_BAJO) {
        return e(LISTA_PLEGADA) as u32;
    }
    let tope = (ancho_ventana as i32 - e(CHAT_MINIMO)).max(e(LISTA_MINIMA));
    pedido.clamp(e(LISTA_MINIMA), e(LISTA_MAXIMA).min(tope)) as u32
}

impl Disposicion {
    /// Reparte una ventana de `ancho` x `alto` (fisicos) con la lista al
    /// ancho `ancho_lista`. `vista` solo manda cuando hay una sola columna.
    pub fn calcular(
        ancho: u32,
        alto: u32,
        escala_por_cien: u32,
        ancho_lista: u32,
        vista: Vista,
    ) -> Disposicion {
        let e = |v: u32| v * escala_por_cien / 100;
        let una_columna = ancho < e(ANCHO_DOS_COLUMNAS);
        let plegada = ancho_lista <= e(LISTA_PLEGADA);
        let columna = |x: i32, w: u32| Rect {
            x,
            y: 0,
            ancho: w,
            alto,
        };
        let con_cabecera = |r: Rect| Rect {
            x: r.x,
            y: 0,
            ancho: r.ancho,
            alto: e(CABECERA).min(alto),
        };

        if una_columna {
            let (lista, chat) = match vista {
                Vista::SoloChat => (vacio(), columna(0, ancho)),
                // Sin sitio para las dos, la lista manda: es de donde se
                // elige, y un chat a medias no se puede usar.
                _ => (columna(0, ancho), vacio()),
            };
            return Disposicion {
                cabecera_lista: if lista.ancho > 0 {
                    con_cabecera(lista)
                } else {
                    vacio()
                },
                filas: Rect {
                    x: lista.x,
                    y: e(CABECERA).min(alto) as i32,
                    ancho: lista.ancho,
                    alto: alto.saturating_sub(e(CABECERA)),
                },
                cabecera_chat: if chat.ancho > 0 {
                    con_cabecera(chat)
                } else {
                    vacio()
                },
                lista,
                chat,
                asa: vacio(),
                plegada,
                una_columna,
            };
        }

        let w = ancho_lista.clamp(e(LISTA_PLEGADA), ancho.saturating_sub(e(CHAT_MINIMO)));
        let lista = columna(0, w);
        let chat = columna(w as i32, ancho - w);
        Disposicion {
            cabecera_lista: con_cabecera(lista),
            filas: Rect {
                x: 0,
                y: e(CABECERA).min(alto) as i32,
                ancho: w,
                alto: alto.saturating_sub(e(CABECERA)),
            },
            cabecera_chat: con_cabecera(chat),
            lista,
            chat,
            asa: Rect {
                x: w as i32 - (e(ASA) / 2) as i32,
                y: 0,
                ancho: e(ASA).max(1),
                alto,
            },
            plegada,
            una_columna,
        }
    }

    /// La fila `indice` de la lista, con el desplazamiento `scroll` ya
    /// restado. Puede caer fuera de `filas`: quien pinta se queda con las
    /// que se ven (`visibles`).
    pub fn fila(&self, indice: usize, scroll: i32, escala_por_cien: u32) -> Rect {
        let alto = FILA * escala_por_cien / 100;
        Rect {
            x: self.filas.x,
            y: self.filas.y + indice as i32 * alto as i32 - scroll,
            ancho: self.filas.ancho,
            alto,
        }
    }

    /// Que fila hay bajo el punto, si hay alguna. `cuantas` evita devolver
    /// una fila que no existe al pinchar el hueco de debajo de la lista.
    pub fn fila_en(
        &self,
        p: Punto,
        scroll: i32,
        cuantas: usize,
        escala_por_cien: u32,
    ) -> Option<usize> {
        if !self.filas.contiene(p) {
            return None;
        }
        let alto = (FILA * escala_por_cien / 100) as i32;
        let indice = (p.y - self.filas.y + scroll) / alto.max(1);
        (indice >= 0 && (indice as usize) < cuantas).then_some(indice as usize)
    }

    /// El primer indice que se ve con este desplazamiento, y cuantos caben.
    /// Es lo que hace que una lista de mil proyectos cueste lo mismo que una
    /// de diez: se pintan solo los que entran en pantalla.
    pub fn visibles(&self, scroll: i32, cuantas: usize, escala_por_cien: u32) -> (usize, usize) {
        let alto = (FILA * escala_por_cien / 100).max(1) as i32;
        let primera = (scroll / alto).max(0) as usize;
        let caben = (self.filas.alto as i32 / alto + 2) as usize;
        (
            primera.min(cuantas),
            caben.min(cuantas.saturating_sub(primera)),
        )
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_lista_nace_a_cinco_catorceavos_y_respeta_sus_topes() {
        assert_eq!(ancho_inicial(1400, 100), 500);
        // En una ventana pequena, el minimo; en una enorme, el maximo.
        assert_eq!(ancho_inicial(700, 100), LISTA_MINIMA);
        assert_eq!(ancho_inicial(3000, 100), LISTA_MAXIMA);
        // Y escala con el DPI.
        assert_eq!(ancho_inicial(2800, 200), 1000);
    }

    #[test]
    fn las_dos_columnas_se_reparten_la_ventana_sin_huecos_ni_solapes() {
        let d = Disposicion::calcular(1200, 800, 100, 320, Vista::Ambas);
        assert!(!d.una_columna && !d.plegada);
        assert_eq!(d.lista.ancho, 320);
        assert_eq!(d.chat.x, 320);
        assert_eq!(d.lista.ancho + d.chat.ancho, 1200);
        assert_eq!(d.cabecera_lista.alto, CABECERA);
        assert_eq!(d.filas.y, CABECERA as i32);
        assert_eq!(d.filas.alto, 800 - CABECERA);
        // El asa cae sobre la linea que separa las dos columnas.
        assert!(d.asa.x <= d.chat.x && d.asa.derecha() >= d.chat.x);
    }

    #[test]
    fn el_chat_nunca_baja_de_su_minimo_aunque_se_pida_una_lista_enorme() {
        let d = Disposicion::calcular(800, 600, 100, 700, Vista::Ambas);
        assert_eq!(d.chat.ancho, CHAT_MINIMO);
        assert_eq!(d.lista.ancho, 800 - CHAT_MINIMO);
    }

    #[test]
    fn arrastrar_muy_estrecho_pliega_la_lista_en_vez_de_dejarla_inservible() {
        assert_eq!(ancho_ajustado(120, 1200, 100), LISTA_PLEGADA);
        assert_eq!(ancho_ajustado(200, 1200, 100), LISTA_MINIMA);
        assert_eq!(ancho_ajustado(900, 1200, 100), LISTA_MAXIMA);
        // Caso negativo: con la ventana justa, el tope lo pone el chat.
        assert_eq!(ancho_ajustado(500, 700, 100), 700 - CHAT_MINIMO);
        let d = Disposicion::calcular(1200, 800, 100, LISTA_PLEGADA, Vista::Ambas);
        assert!(d.plegada);
    }

    #[test]
    fn en_una_ventana_estrecha_solo_se_ve_una_columna_entera() {
        let lista = Disposicion::calcular(500, 700, 100, 260, Vista::SoloLista);
        assert!(lista.una_columna);
        assert_eq!(lista.lista.ancho, 500);
        assert_eq!(lista.chat.ancho, 0);
        let chat = Disposicion::calcular(500, 700, 100, 260, Vista::SoloChat);
        assert_eq!(chat.chat.ancho, 500);
        assert_eq!(chat.lista.ancho, 0);
        assert_eq!(chat.cabecera_lista, vacio());
    }

    #[test]
    fn las_filas_van_seguidas_y_se_acierta_la_de_debajo_del_raton() {
        let d = Disposicion::calcular(1200, 800, 100, 320, Vista::Ambas);
        let primera = d.fila(0, 0, 100);
        assert_eq!(primera.y, d.filas.y);
        assert_eq!(primera.alto, FILA);
        assert_eq!(d.fila(1, 0, 100).y, d.filas.y + FILA as i32);
        // Con scroll, las filas suben.
        assert_eq!(d.fila(2, 30, 100).y, d.filas.y + 2 * FILA as i32 - 30);

        let en = |y: i32, scroll: i32| {
            d.fila_en(
                Punto {
                    x: 100,
                    y: d.filas.y + y,
                },
                scroll,
                10,
                100,
            )
        };
        assert_eq!(en(5, 0), Some(0));
        assert_eq!(en(FILA as i32 + 5, 0), Some(1));
        assert_eq!(en(5, FILA as i32), Some(1), "con scroll de una fila");
        // Caso negativo: la cabecera no es lista, y bajo la ultima no hay fila.
        assert_eq!(d.fila_en(Punto { x: 100, y: 10 }, 0, 10, 100), None);
        assert_eq!(en(FILA as i32 * 20, 0), None);
    }

    #[test]
    fn solo_se_pintan_las_filas_que_entran_en_pantalla() {
        let d = Disposicion::calcular(1200, 800, 100, 320, Vista::Ambas);
        let (primera, cuantas) = d.visibles(0, 1000, 100);
        assert_eq!(primera, 0);
        assert!(
            cuantas <= (800 / FILA + 2) as usize,
            "mil proyectos no cuestan mil filas: {cuantas}"
        );
        let (primera, _) = d.visibles(FILA as i32 * 40, 1000, 100);
        assert_eq!(primera, 40);
        // Caso negativo: con pocas, no se inventan filas de mas.
        let (p, c) = d.visibles(0, 3, 100);
        assert_eq!((p, c), (0, 3));
    }
}
