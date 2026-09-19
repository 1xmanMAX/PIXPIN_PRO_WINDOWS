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
/// La barra de titulo propia: la ventana no tiene marco del sistema.
pub const BARRA: u32 = 24;
/// Cada boton de la barra (minimizar, maximizar, cerrar).
pub const BOTON_BARRA_ANCHO: u32 = 36;
/// Margen de los bordes por los que se redimensiona.
pub const BORDE: u32 = 6;

/// Cabecera de la lista y del chat (`topBarHeight`).
pub const CABECERA: u32 = 54;
/// Dentro de la cabecera del chat: avatar de 42 en (19, 6) y el texto a 69,
/// con el nombre a 8 de arriba.
pub const CABECERA_AVATAR: u32 = 42;
pub const CABECERA_AVATAR_X: u32 = 19;
pub const CABECERA_AVATAR_Y: u32 = 6;
pub const CABECERA_TEXTO_X: u32 = 69;
pub const CABECERA_NOMBRE_Y: u32 = 8;
pub const CABECERA_MARGEN_DERECHO: u32 = 17;
/// El nombre va en seminegrita de 13 y el estado en normal de 13.
pub const CABECERA_TAM: f32 = 13.0;
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
/// El contador de pendientes: pildora de 19 de alto con texto de 12 en
/// negrita y 5 de relleno a cada lado. Con un solo digito sale un circulo.
pub const CONTADOR_ALTO: u32 = 19;
pub const CONTADOR_TAM: f32 = 12.0;
pub const CONTADOR_RELLENO: u32 = 5;
/// Lo minimo que se separa la hora del nombre.
pub const HORA_HUECO: u32 = 5;
/// El titulo de la barra, en seminegrita.
pub const TITULO_TAM: f32 = 12.0;

/// El buscador de la cabecera de la lista: capsula de 35 con 7 de margen.
pub const BUSCADOR_ALTO: u32 = 35;
/// El boton redondo de proyecto nuevo: el mismo tamano y el mismo aire que
/// el lapiz de Telegram en su esquina.
pub const NUEVO_LADO: u32 = 54;
pub const NUEVO_MARGEN: u32 = 16;
pub const BUSCADOR_RADIO: u32 = 18;
pub const BUSCADOR_MARGEN: u32 = 7;
pub const BUSCADOR_TEXTO_X: u32 = 12;
pub const BUSCADOR_TAM: f32 = 13.0;

/// Los botones de al lado de la caja de escribir (adjuntar, enviar).
pub const BOTON_ANCHO: u32 = 44;
pub const BOTON_ALTO: u32 = 46;
pub const BOTON_MARGEN: u32 = 2;

/// La barra del mensaje fijado, bajo la cabecera del proyecto.
pub const FIJADO_ALTO: u32 = 49;
pub const FIJADO_MARGEN_X: u32 = 17;
/// La rayita de color que lo marca a la izquierda.
pub const FIJADO_RAYA: u32 = 2;
/// Margen a la derecha de la fila para la hora y el contador.
pub const MARGEN_DERECHO: u32 = 10;

/// La caja de escribir, abajo de la columna del proyecto. Crece con el
/// texto hasta un tope; pasado ese tope se desplaza por dentro, que si no
/// una nota larga se comeria el historial entero.
///
/// Es la ISLA del movil (`BarraDeEscribir` de `MensajesActivity.kt`): una
/// pastilla flotando a 7 de los lados y 9 del borde de abajo, con 4 de aire
/// por dentro; dentro, el campo es otra pastilla de radio 22 y fila minima
/// de 44, con el clip dentro a la derecha, y el microfono (o enviar) fuera
/// del campo, en su hueco de 48.
pub const ISLA_MARGEN_X: u32 = 7;
pub const ISLA_BAJO: u32 = 9;
/// El aire de encima de la isla: el historial no se le pega.
pub const ISLA_ARRIBA: u32 = 6;
pub const ISLA_RELLENO_X: u32 = 6;
pub const ISLA_RELLENO_Y: u32 = 4;
pub const ISLA_RADIO: u32 = 22;
/// Lo alto de una fila del campo, y lo que mide el clip.
pub const CAMPO_FILA: u32 = 44;
/// El hueco del microfono o de enviar: la fila mas cuatro.
pub const BOTON_VOZ: u32 = CAMPO_FILA + 4;
/// Donde empieza el texto dentro del campo, y su aire arriba y abajo.
pub const CAMPO_TEXTO_X: u32 = 14;
pub const CAMPO_TEXTO_ARRIBA: u32 = 9;
pub const CAMPO_TEXTO_ABAJO: u32 = 10;
pub const REDACCION_MINIMA: u32 = ISLA_ARRIBA + ISLA_BAJO + 2 * ISLA_RELLENO_Y + CAMPO_FILA;
pub const REDACCION_MAXIMA: u32 = 224;
/// La letra del campo. El movil usa 18 sobre burbujas de 15; aqui las
/// burbujas van a 13, y se guarda la misma proporcion.
pub const REDACCION_TAM: f32 = 15.0;

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
/// Los botones de la barra de titulo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotonBarra {
    Minimizar,
    Maximizar,
    Cerrar,
}

/// Por donde se agarra para redimensionar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Borde {
    Izquierda,
    Derecha,
    Arriba,
    Abajo,
    ArribaIzquierda,
    ArribaDerecha,
    AbajoIzquierda,
    AbajoDerecha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disposicion {
    /// La barra de titulo, arriba del todo.
    pub barra: Rect,
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
    /// Lo que va ENCIMA de la isla de escribir y cuenta como parte de ella:
    /// la barra de «a quien se contesta». Cero si no hay nada. Lo pone quien
    /// pinta; el historial se aparta lo que mida.
    pub encima_de_la_isla: u32,
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
        // Todo cuelga de debajo de la barra de titulo.
        let barra = Rect {
            x: 0,
            y: 0,
            ancho,
            alto: e(BARRA).min(alto),
        };
        let arriba = barra.alto as i32;
        let alto = alto.saturating_sub(barra.alto);
        let plegada = ancho_lista <= e(LISTA_PLEGADA);
        let columna = |x: i32, w: u32| Rect {
            x,
            y: arriba,
            ancho: w,
            alto,
        };
        let con_cabecera = |r: Rect| Rect {
            x: r.x,
            y: arriba,
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
                    y: arriba + e(CABECERA).min(alto) as i32,
                    ancho: lista.ancho,
                    alto: alto.saturating_sub(e(CABECERA)),
                },
                cabecera_chat: if chat.ancho > 0 {
                    con_cabecera(chat)
                } else {
                    vacio()
                },
                barra,
                lista,
                chat,
                asa: vacio(),
                plegada,
                una_columna,
                encima_de_la_isla: 0,
            };
        }

        let w = ancho_lista.clamp(e(LISTA_PLEGADA), ancho.saturating_sub(e(CHAT_MINIMO)));
        let lista = columna(0, w);
        let chat = columna(w as i32, ancho - w);
        Disposicion {
            cabecera_lista: con_cabecera(lista),
            filas: Rect {
                x: 0,
                y: arriba + e(CABECERA).min(alto) as i32,
                ancho: w,
                alto: alto.saturating_sub(e(CABECERA)),
            },
            cabecera_chat: con_cabecera(chat),
            lista,
            chat,
            barra,
            asa: Rect {
                x: w as i32 - (e(ASA) / 2) as i32,
                y: arriba,
                ancho: e(ASA).max(1),
                alto,
            },
            plegada,
            una_columna,
            encima_de_la_isla: 0,
        }
    }

    /// Los tres botones de la barra, de derecha a izquierda: cerrar,
    /// maximizar y minimizar.
    pub fn botones_barra(&self, escala_por_cien: u32) -> [(BotonBarra, Rect); 3] {
        let w = BOTON_BARRA_ANCHO * escala_por_cien / 100;
        let mut x = self.barra.derecha();
        [
            BotonBarra::Cerrar,
            BotonBarra::Maximizar,
            BotonBarra::Minimizar,
        ]
        .map(|b| {
            x -= w as i32;
            (
                b,
                Rect {
                    x,
                    y: self.barra.y,
                    ancho: w,
                    alto: self.barra.alto,
                },
            )
        })
    }

    pub fn boton_barra_en(&self, p: Punto, escala_por_cien: u32) -> Option<BotonBarra> {
        self.botones_barra(escala_por_cien)
            .into_iter()
            .find(|(_, r)| r.contiene(p))
            .map(|(b, _)| b)
    }

    /// Si el punto sirve para arrastrar la ventana: la barra, menos sus
    /// botones.
    pub fn arrastra_ventana(&self, p: Punto, escala_por_cien: u32) -> bool {
        self.barra.contiene(p) && self.boton_barra_en(p, escala_por_cien).is_none()
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

    /// Lo alto que es la caja de escribir con un texto de `alto_texto`.
    ///
    /// Crece desde el minimo y se planta en el maximo: a partir de ahi el
    /// texto se desplaza por dentro. Una nota de cincuenta lineas no puede
    /// dejar el historial sin sitio.
    pub fn alto_redaccion(&self, alto_texto: u32, escala_por_cien: u32) -> u32 {
        let e = |v: u32| v * escala_por_cien / 100;
        let fijo =
            ISLA_ARRIBA + ISLA_BAJO + 2 * ISLA_RELLENO_Y + CAMPO_TEXTO_ARRIBA + CAMPO_TEXTO_ABAJO;
        ((alto_texto + e(fijo)).clamp(e(REDACCION_MINIMA), e(REDACCION_MAXIMA))
            + self.encima_de_la_isla)
            // Y nunca mas de media columna, por estrecha que sea la ventana.
            .min((self.chat.alto / 2).max(1))
    }

    /// La barra de encima de la isla (a quien se contesta), si hay sitio
    /// reservado para ella: del ancho de la isla y pegada a ella por arriba.
    pub fn encima_de_la_isla(&self, alto_texto: u32, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        let caja = self.redaccion(alto_texto, escala_por_cien);
        Rect {
            x: caja.x + e(ISLA_MARGEN_X) as i32,
            y: caja.y + e(ISLA_ARRIBA) as i32,
            ancho: caja.ancho.saturating_sub(2 * e(ISLA_MARGEN_X)),
            alto: self.encima_de_la_isla.min(caja.alto),
        }
    }

    /// El buscador, en la cabecera de la lista, con el boton de sincronizar a
    /// su derecha.
    pub fn buscador(&self, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        let c = self.cabecera_lista;
        if c.ancho == 0 {
            return vacio();
        }
        let alto = e(BUSCADOR_ALTO).min(c.alto);
        // El sitio del boton se quita solo si el boton esta: en una lista
        // estrecha no cabe, y el buscador se queda con todo el ancho.
        let boton = self.boton_sincro(escala_por_cien);
        let ocupado = if boton.ancho > 0 {
            boton.ancho + e(BUSCADOR_MARGEN)
        } else {
            0
        };
        Rect {
            x: c.x + e(BUSCADOR_MARGEN) as i32,
            y: c.y + (c.alto as i32 - alto as i32) / 2,
            ancho: c.ancho.saturating_sub(2 * e(BUSCADOR_MARGEN) + ocupado),
            alto,
        }
    }

    /// El boton redondo de sincronizar, a la derecha del buscador.
    ///
    /// Arriba y a la vista, como en el movil: recibir del otro aparato es lo
    /// que se hace cada vez que se cambia de sitio de trabajo, y metido en el
    /// menu de adjuntar de un proyecto no lo encontraba nadie.
    pub fn boton_sincro(&self, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        let c = self.cabecera_lista;
        let lado = e(BUSCADOR_ALTO).min(c.alto);
        // Plegada, la cabecera mide lo que un avatar. Y con menos de tres
        // botones de ancho, el buscador se quedaria sin sitio para escribir.
        if self.plegada || c.ancho < 3 * lado + 3 * e(BUSCADOR_MARGEN) {
            return vacio();
        }
        Rect {
            x: c.derecha() - (lado + e(BUSCADOR_MARGEN)) as i32,
            y: c.y + (c.alto as i32 - lado as i32) / 2,
            ancho: lado,
            alto: lado,
        }
    }

    /// El boton redondo de proyecto nuevo, flotando en la esquina de abajo a
    /// la derecha de la lista.
    ///
    /// Flotando y no en la cabecera porque la cabecera ya la ocupa entera el
    /// buscador, que es lo que se usa a diario; crear un proyecto se hace de
    /// vez en cuando. Es el mismo sitio y el mismo gesto que el lapiz de
    /// Telegram, y que el boton de anadir de tantas aplicaciones.
    pub fn boton_nuevo(&self, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        let lado = e(NUEVO_LADO);
        // Plegada la lista es una tira de avatares: un boton redondo de este
        // tamano taparia dos de ellos. Y en una lista mas estrecha que el
        // propio boton, tampoco: valdria mas tapar filas que poder pulsarlo.
        if self.plegada
            || self.filas.ancho < lado + e(NUEVO_MARGEN)
            || self.filas.alto < lado + e(NUEVO_MARGEN)
        {
            return vacio();
        }
        Rect {
            x: self.filas.derecha() - (lado + e(NUEVO_MARGEN)) as i32,
            y: self.filas.abajo() - (lado + e(NUEVO_MARGEN)) as i32,
            ancho: lado,
            alto: lado,
        }
    }

    /// La isla que flota abajo: la caja de escribir sin su aire alrededor.
    pub fn isla(&self, alto_texto: u32, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        let caja = self.redaccion(alto_texto, escala_por_cien);
        let encima = self.encima_de_la_isla.min(caja.alto);
        Rect {
            x: caja.x + e(ISLA_MARGEN_X) as i32,
            y: caja.y + (e(ISLA_ARRIBA) + encima) as i32,
            ancho: caja.ancho.saturating_sub(2 * e(ISLA_MARGEN_X)),
            alto: caja
                .alto
                .saturating_sub(e(ISLA_ARRIBA) + e(ISLA_BAJO) + encima),
        }
    }

    /// El campo donde se escribe: la pastilla de dentro de la isla, que deja
    /// a su derecha el hueco del microfono.
    pub fn campo(&self, alto_texto: u32, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        let isla = self.isla(alto_texto, escala_por_cien);
        let izquierda = isla.x + e(ISLA_RELLENO_X) as i32;
        let derecha = self.boton_enviar(alto_texto, escala_por_cien).x;
        Rect {
            x: izquierda,
            y: isla.y + e(ISLA_RELLENO_Y) as i32,
            ancho: (derecha - izquierda).max(0) as u32,
            alto: isla.alto.saturating_sub(2 * e(ISLA_RELLENO_Y)),
        }
    }

    /// El clip: DENTRO del campo, a su derecha y apoyado abajo, como en el
    /// movil (y no a la izquierda de la barra, como en Telegram).
    pub fn boton_adjuntar(&self, alto_texto: u32, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        let campo = self.campo(alto_texto, escala_por_cien);
        let lado = e(CAMPO_FILA).min(campo.ancho).min(campo.alto);
        Rect {
            x: campo.derecha() - lado as i32,
            y: campo.abajo() - lado as i32,
            ancho: lado,
            alto: lado,
        }
    }

    /// El microfono, o enviar si hay algo escrito: el mismo sitio con dos
    /// caras, fuera del campo y a la derecha de la isla.
    pub fn boton_enviar(&self, alto_texto: u32, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        let isla = self.isla(alto_texto, escala_por_cien);
        let lado = e(BOTON_VOZ).min(isla.alto).min(isla.ancho);
        Rect {
            x: isla.derecha() - e(ISLA_RELLENO_X) as i32 - lado as i32,
            y: isla.abajo() - e(ISLA_RELLENO_Y) as i32 - lado as i32,
            ancho: lado,
            alto: lado,
        }
    }

    /// Lo que queda para el texto dentro del campo, sin el clip.
    pub fn texto_redaccion(&self, alto_texto: u32, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        let campo = self.campo(alto_texto, escala_por_cien);
        let izquierda = campo.x + e(CAMPO_TEXTO_X) as i32;
        let derecha = self.boton_adjuntar(alto_texto, escala_por_cien).x;
        Rect {
            x: izquierda,
            y: campo.y + e(CAMPO_TEXTO_ARRIBA) as i32,
            ancho: (derecha - izquierda).max(0) as u32,
            alto: campo
                .alto
                .saturating_sub(e(CAMPO_TEXTO_ARRIBA) + e(CAMPO_TEXTO_ABAJO)),
        }
    }

    /// La barra del mensaje fijado, justo bajo la cabecera del proyecto.
    /// Vacia si no hay ninguno fijado (lo decide quien llama).
    pub fn fijado(&self, escala_por_cien: u32) -> Rect {
        let e = |v: u32| v * escala_por_cien / 100;
        if self.chat.ancho == 0 {
            return vacio();
        }
        Rect {
            x: self.chat.x,
            y: self.cabecera_chat.abajo(),
            ancho: self.chat.ancho,
            alto: e(FIJADO_ALTO).min(self.chat.alto),
        }
    }

    /// La caja de escribir, pegada abajo de la columna del proyecto.
    pub fn redaccion(&self, alto_texto: u32, escala_por_cien: u32) -> Rect {
        let alto = self.alto_redaccion(alto_texto, escala_por_cien);
        Rect {
            x: self.chat.x,
            y: self.chat.abajo() - alto as i32,
            ancho: self.chat.ancho,
            alto,
        }
    }

    /// El historial: lo que queda entre la cabecera (o la barra del mensaje
    /// fijado, si la hay) y la caja de escribir.
    pub fn historial(&self, alto_texto: u32, hay_fijado: bool, escala_por_cien: u32) -> Rect {
        let arriba = if hay_fijado {
            self.fijado(escala_por_cien).abajo()
        } else {
            self.cabecera_chat.abajo()
        };
        let abajo = self.redaccion(alto_texto, escala_por_cien).y;
        Rect {
            x: self.chat.x,
            y: arriba,
            ancho: self.chat.ancho,
            alto: (abajo - arriba).max(0) as u32,
        }
    }

    /// Hasta donde se puede bajar. Si todo cabe, cero: la lista corta no se
    /// mueve y no puede quedar en blanco por encima.
    pub fn scroll_maximo(&self, cuantas: usize, escala_por_cien: u32) -> i32 {
        let alto = (FILA * escala_por_cien / 100) as i32;
        (cuantas as i32 * alto - self.filas.alto as i32).max(0)
    }

    /// Deja el desplazamiento dentro de lo que existe.
    pub fn scroll_ajustado(&self, scroll: i32, cuantas: usize, escala_por_cien: u32) -> i32 {
        scroll.clamp(0, self.scroll_maximo(cuantas, escala_por_cien))
    }
}

/// Las piezas de una fila, ya colocadas dentro de su rectangulo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartesFila {
    pub avatar: Rect,
    /// Esquina donde empieza el nombre.
    pub nombre: Punto,
    /// Esquina donde empieza la ultima linea.
    pub resumen: Punto,
    /// Lo ancho que puede ser el texto sin chocar con la hora o el contador.
    pub ancho_texto: u32,
    /// Donde termina la hora, arriba a la derecha.
    pub derecha: i32,
}

/// Coloca lo de dentro de una fila. Con la lista plegada solo hay avatar,
/// centrado, y el texto no tiene sitio.
pub fn partes_fila(fila: Rect, plegada: bool, escala_por_cien: u32) -> PartesFila {
    let e = |v: u32| v * escala_por_cien / 100;
    let avatar = e(AVATAR);
    if plegada {
        let x = fila.x + (fila.ancho as i32 - avatar as i32) / 2;
        return PartesFila {
            avatar: Rect {
                x,
                y: fila.y + e(AVATAR_Y) as i32,
                ancho: avatar,
                alto: avatar,
            },
            nombre: Punto { x, y: fila.y },
            resumen: Punto { x, y: fila.y },
            ancho_texto: 0,
            derecha: fila.derecha(),
        };
    }
    let derecha = fila.derecha() - e(MARGEN_DERECHO) as i32;
    let texto_x = fila.x + e(TEXTO_X) as i32;
    PartesFila {
        avatar: Rect {
            x: fila.x + e(AVATAR_X) as i32,
            y: fila.y + e(AVATAR_Y) as i32,
            ancho: avatar,
            alto: avatar,
        },
        nombre: Punto {
            x: texto_x,
            y: fila.y + e(NOMBRE_Y) as i32,
        },
        resumen: Punto {
            x: texto_x,
            y: fila.y + e(RESUMEN_Y) as i32,
        },
        ancho_texto: (derecha - texto_x).max(0) as u32,
        derecha,
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
        assert_eq!(d.filas.y, (BARRA + CABECERA) as i32);
        assert_eq!(d.filas.alto, 800 - BARRA - CABECERA);
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

/// Por que borde se agarra el punto, si por alguno. Los bordes ganan a todo
/// lo demas: son solo unos pixeles y sin ellos no se puede redimensionar.
pub fn borde_en(p: Punto, ancho: u32, alto: u32, escala_por_cien: u32) -> Option<Borde> {
    let m = (BORDE * escala_por_cien / 100).max(2) as i32;
    if p.x < 0 || p.y < 0 || p.x >= ancho as i32 || p.y >= alto as i32 {
        return None;
    }
    let izquierda = p.x < m;
    let derecha = p.x >= ancho as i32 - m;
    let arriba = p.y < m;
    let abajo = p.y >= alto as i32 - m;
    Some(match (izquierda, derecha, arriba, abajo) {
        (true, _, true, _) => Borde::ArribaIzquierda,
        (_, true, true, _) => Borde::ArribaDerecha,
        (true, _, _, true) => Borde::AbajoIzquierda,
        (_, true, _, true) => Borde::AbajoDerecha,
        (true, ..) => Borde::Izquierda,
        (_, true, ..) => Borde::Derecha,
        (_, _, true, _) => Borde::Arriba,
        (.., true) => Borde::Abajo,
        _ => return None,
    })
}

/// La ventana redimensionada al arrastrar `borde` hasta `cursor` (en
/// coordenadas del escritorio), sin bajar de los minimos.
pub fn redimensionar(marco: Rect, borde: Borde, cursor: Punto, escala_por_cien: u32) -> Rect {
    let e = |v: u32| (v * escala_por_cien / 100) as i32;
    let (mut x0, mut y0) = (marco.x, marco.y);
    let (mut x1, mut y1) = (marco.derecha(), marco.abajo());
    let toca_izquierda = matches!(
        borde,
        Borde::Izquierda | Borde::ArribaIzquierda | Borde::AbajoIzquierda
    );
    let toca_derecha = matches!(
        borde,
        Borde::Derecha | Borde::ArribaDerecha | Borde::AbajoDerecha
    );
    let toca_arriba = matches!(
        borde,
        Borde::Arriba | Borde::ArribaIzquierda | Borde::ArribaDerecha
    );
    let toca_abajo = matches!(
        borde,
        Borde::Abajo | Borde::AbajoIzquierda | Borde::AbajoDerecha
    );
    if toca_izquierda {
        x0 = cursor.x.min(x1 - e(ANCHO_MINIMO_VENTANA));
    }
    if toca_derecha {
        x1 = cursor.x.max(x0 + e(ANCHO_MINIMO_VENTANA));
    }
    if toca_arriba {
        y0 = cursor.y.min(y1 - e(ALTO_MINIMO_VENTANA));
    }
    if toca_abajo {
        y1 = cursor.y.max(y0 + e(ALTO_MINIMO_VENTANA));
    }
    Rect {
        x: x0,
        y: y0,
        ancho: (x1 - x0) as u32,
        alto: (y1 - y0) as u32,
    }
}

#[cfg(test)]
mod pruebas_ventana {
    use super::*;

    fn marco() -> Rect {
        Rect {
            x: 100,
            y: 100,
            ancho: 1000,
            alto: 700,
        }
    }

    #[test]
    fn el_boton_de_sincronizar_va_a_la_derecha_del_buscador_sin_pisarlo() {
        let d = Disposicion::calcular(1000, 700, 100, 320, Vista::Ambas);
        let (b, s) = (d.buscador(100), d.boton_sincro(100));
        assert_eq!((s.ancho, s.alto), (BUSCADOR_ALTO, BUSCADOR_ALTO));
        assert!(
            b.derecha() < s.x,
            "el buscador acaba antes de que empiece el boton"
        );
        assert!(s.derecha() <= d.cabecera_lista.derecha());
    }

    #[test]
    fn con_la_lista_plegada_no_hay_boton_y_el_buscador_no_le_guarda_sitio() {
        let d = Disposicion::calcular(1200, 800, 100, LISTA_PLEGADA, Vista::Ambas);
        assert_eq!(d.boton_sincro(100).ancho, 0);
        // Un punto cualquiera no cae en un boton que no esta.
        assert!(!d.boton_sincro(100).contiene(Punto { x: 0, y: 0 }));
    }

    #[test]
    fn el_boton_de_proyecto_nuevo_flota_en_la_esquina_de_la_lista() {
        let d = Disposicion::calcular(1000, 700, 100, 320, Vista::Ambas);
        let b = d.boton_nuevo(100);
        assert_eq!((b.ancho, b.alto), (NUEVO_LADO, NUEVO_LADO));
        // Abajo a la derecha de las filas, con el mismo aire por los dos
        // lados: es lo que lo hace leerse como flotante y no como pegado.
        assert_eq!(d.filas.derecha() - b.derecha(), NUEVO_MARGEN as i32);
        assert_eq!(d.filas.abajo() - b.abajo(), NUEVO_MARGEN as i32);
        assert!(d.filas.contiene(Punto { x: b.x, y: b.y }));
    }

    #[test]
    fn en_una_lista_plegada_no_hay_boton_de_proyecto_nuevo() {
        // Caso negativo: en una columna mas estrecha que el propio boton,
        // ensenarlo taparia las filas y ni siquiera se podria pulsar bien.
        let d = Disposicion::calcular(1000, 700, 100, 20, Vista::Ambas);
        assert_eq!(d.boton_nuevo(100).ancho, 0);
        // Y en una ventana tan estrecha que solo cabe una columna, con el
        // proyecto a la vista, no hay lista donde ponerlo.
        let d = Disposicion::calcular(420, 700, 100, 320, Vista::SoloChat);
        assert_eq!(d.filas.ancho, 0, "no hay lista");
        assert_eq!(d.boton_nuevo(100).ancho, 0);
    }

    #[test]
    fn los_botones_van_a_la_derecha_de_la_barra_en_el_orden_de_windows() {
        let d = Disposicion::calcular(1000, 700, 100, 320, Vista::Ambas);
        let bs = d.botones_barra(100);
        assert_eq!(bs[0].0, BotonBarra::Cerrar);
        assert_eq!(bs[0].1.derecha(), 1000, "cerrar toca el borde derecho");
        assert_eq!(bs[1].0, BotonBarra::Maximizar);
        assert_eq!(bs[2].0, BotonBarra::Minimizar);
        assert!(bs[2].1.x < bs[1].1.x && bs[1].1.x < bs[0].1.x);
        let centro = |r: Rect| Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        };
        assert_eq!(
            d.boton_barra_en(centro(bs[0].1), 100),
            Some(BotonBarra::Cerrar)
        );
        // A la izquierda de los botones se arrastra la ventana.
        assert!(d.arrastra_ventana(Punto { x: 200, y: 10 }, 100));
        assert!(
            !d.arrastra_ventana(centro(bs[0].1), 100),
            "cerrar no arrastra"
        );
        // Caso negativo: bajo la barra ya es contenido.
        assert!(!d.arrastra_ventana(
            Punto {
                x: 200,
                y: BARRA as i32 + 5
            },
            100
        ));
    }

    #[test]
    fn las_columnas_empiezan_bajo_la_barra_de_titulo() {
        let d = Disposicion::calcular(1000, 700, 100, 320, Vista::Ambas);
        assert_eq!(d.barra.alto, BARRA);
        assert_eq!(d.lista.y, BARRA as i32);
        assert_eq!(d.cabecera_lista.y, BARRA as i32);
        assert_eq!(d.filas.y, (BARRA + CABECERA) as i32);
        assert_eq!(d.lista.abajo(), 700);
    }

    #[test]
    fn las_esquinas_y_los_lados_se_reconocen_y_el_centro_no() {
        let en = |x: i32, y: i32| borde_en(Punto { x, y }, 1000, 700, 100);
        assert_eq!(en(0, 0), Some(Borde::ArribaIzquierda));
        assert_eq!(en(999, 0), Some(Borde::ArribaDerecha));
        assert_eq!(en(0, 699), Some(Borde::AbajoIzquierda));
        assert_eq!(en(999, 699), Some(Borde::AbajoDerecha));
        assert_eq!(en(500, 1), Some(Borde::Arriba));
        assert_eq!(en(2, 300), Some(Borde::Izquierda));
        assert_eq!(en(500, 300), None, "el centro no redimensiona");
        assert_eq!(en(-1, 300), None, "fuera de la ventana tampoco");
    }

    #[test]
    fn redimensionar_mueve_el_lado_que_se_agarra_y_respeta_los_minimos() {
        let r = redimensionar(marco(), Borde::Derecha, Punto { x: 1400, y: 0 }, 100);
        assert_eq!((r.x, r.ancho), (100, 1300));
        let r = redimensionar(marco(), Borde::Izquierda, Punto { x: 300, y: 0 }, 100);
        assert_eq!(
            (r.x, r.derecha()),
            (300, 1100),
            "el lado opuesto no se mueve"
        );
        // Caso negativo: no se puede encoger por debajo del minimo.
        let r = redimensionar(marco(), Borde::Derecha, Punto { x: 120, y: 0 }, 100);
        assert_eq!(r.ancho, ANCHO_MINIMO_VENTANA);
        let r = redimensionar(
            marco(),
            Borde::AbajoDerecha,
            Punto { x: 1400, y: 1000 },
            100,
        );
        assert_eq!((r.ancho, r.alto), (1300, 900));
    }
}

#[cfg(test)]
mod pruebas_filas {
    use super::*;

    fn disposicion(alto: u32) -> Disposicion {
        Disposicion::calcular(1024, alto, 100, 300, Vista::Ambas)
    }

    #[test]
    fn la_lista_no_se_desplaza_si_todo_cabe_y_se_para_al_final_si_no() {
        let d = disposicion(768);
        let caben = d.filas.alto / FILA;
        assert_eq!(d.scroll_maximo(caben as usize, 100), 0, "todo a la vista");
        // Con diez filas de mas, se puede bajar hasta que la ultima quede
        // pegada abajo, ni un pixel mas.
        let muchas = caben as usize + 10;
        let tope = muchas as i32 * FILA as i32 - d.filas.alto as i32;
        assert!(tope > 0);
        assert_eq!(d.scroll_maximo(muchas, 100), tope);
        assert_eq!(d.scroll_ajustado(9999, muchas, 100), tope);
        assert_eq!(d.scroll_ajustado(-50, muchas, 100), 0, "no se sube de mas");
    }

    #[test]
    fn de_mil_proyectos_solo_se_pintan_los_que_entran() {
        let d = disposicion(768);
        let (primera, cuantas) = d.visibles(0, 1000, 100);
        assert_eq!(primera, 0);
        assert!(cuantas < 20, "no puede pintar las mil: {cuantas}");
        // Bajando, empieza por otra y sigue pintando pocas.
        let (primera, cuantas2) = d.visibles(10 * FILA as i32, 1000, 100);
        assert_eq!(primera, 10);
        assert_eq!(cuantas, cuantas2);
    }

    #[test]
    fn lo_de_dentro_de_una_fila_va_donde_telegram() {
        let d = disposicion(768);
        let fila = d.fila(0, 0, 100);
        let p = partes_fila(fila, false, 100);
        assert_eq!(p.avatar.ancho, AVATAR);
        assert_eq!(p.avatar.x, fila.x + AVATAR_X as i32);
        assert_eq!(p.nombre.x, fila.x + TEXTO_X as i32);
        assert!(p.resumen.y > p.nombre.y, "la ultima linea va debajo");
        // El texto no llega al borde: deja sitio a la hora.
        assert!(p.derecha < fila.derecha());
        assert_eq!(p.ancho_texto, (p.derecha - p.nombre.x) as u32);
        // Plegada: solo el avatar, centrado, y sin sitio para texto.
        let q = partes_fila(fila, true, 100);
        assert_eq!(q.ancho_texto, 0);
        assert_eq!(
            q.avatar.x - fila.x,
            fila.derecha() - q.avatar.derecha(),
            "centrado"
        );
    }

    #[test]
    fn pinchar_bajo_la_ultima_fila_no_elige_ninguna() {
        let d = disposicion(768);
        let dentro = Punto {
            x: d.filas.x + 10,
            y: d.filas.y + FILA as i32 + 5,
        };
        assert_eq!(d.fila_en(dentro, 0, 3, 100), Some(1));
        // Con solo una fila, ese mismo punto es hueco.
        assert_eq!(d.fila_en(dentro, 0, 1, 100), None);
        // Y la cabecera nunca es una fila.
        let cabecera = Punto {
            x: d.filas.x + 10,
            y: d.cabecera_lista.y + 2,
        };
        assert_eq!(d.fila_en(cabecera, 0, 3, 100), None);
    }
}

/// La hora que se ensena a la derecha de una fila.
///
/// Ambos instantes en milisegundos **de hora local** (ver
/// `pixpin_shell::entorno::ahora_local_ms`), que asi el mismo dia es la
/// misma division y no hace falta saber de husos.
///
/// Lo de hoy va con la hora; lo de antes, con la fecha, y solo lleva el ano
/// si es de otro. Sin palabras, para que valga en cualquier idioma.
pub fn etiqueta_hora(cuando_ms: i64, ahora_ms: i64) -> String {
    const DIA: i64 = 86_400_000;
    if cuando_ms <= 0 {
        return String::new();
    }
    let dia = cuando_ms.div_euclid(DIA);
    if dia == ahora_ms.div_euclid(DIA) {
        let del_dia = cuando_ms.rem_euclid(DIA) / 1000;
        return format!("{:02}:{:02}", del_dia / 3600, (del_dia % 3600) / 60);
    }
    let (a, m, d) = civil(dia);
    let (ahora_a, _, _) = civil(ahora_ms.div_euclid(DIA));
    if a == ahora_a {
        format!("{d:02}/{m:02}")
    } else {
        format!("{d:02}/{m:02}/{:02}", a.rem_euclid(100))
    }
}

/// La fecha sola, para la pildora que separa los dias del historial.
///
/// Nunca da una hora, ni siquiera para hoy: una pildora que pone «13:13»
/// entre dos mensajes no separa nada, confunde.
pub fn etiqueta_fecha(cuando_ms: i64, ahora_ms: i64) -> String {
    const DIA: i64 = 86_400_000;
    if cuando_ms <= 0 {
        return String::new();
    }
    let (a, m, d) = civil(cuando_ms.div_euclid(DIA));
    let (ahora_a, _, _) = civil(ahora_ms.div_euclid(DIA));
    if a == ahora_a {
        format!("{d:02}/{m:02}")
    } else {
        format!("{d:02}/{m:02}/{:02}", a.rem_euclid(100))
    }
}

/// El dia de una fecha, partido en ano, mes (1-12) y dia, para quien tenga
/// que componer el nombre del mes en su idioma.
pub fn partes_fecha(cuando_ms: i64) -> (i64, u32, u32) {
    civil(cuando_ms.div_euclid(86_400_000))
}

/// Dia desde 1970 a (ano, mes, dia). Algoritmo `civil_from_days` de Howard
/// Hinnant, de dominio publico; vale de 1601 en adelante de sobra.
fn civil(dias: i64) -> (i64, u32, u32) {
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod pruebas_hora {
    use super::*;

    /// 15 de septiembre de 2026, 14:32:05 locales.
    const AHORA: i64 = 1_789_482_725_000;

    #[test]
    fn lo_de_hoy_va_con_la_hora_y_lo_viejo_con_la_fecha() {
        assert_eq!(etiqueta_hora(AHORA, AHORA), "14:32");
        // Esta madrugada, aunque sea hace poco, sigue siendo hoy.
        let madrugada = AHORA - 14 * 3_600_000 - 32 * 60_000 - 5_000;
        assert_eq!(etiqueta_hora(madrugada, AHORA), "00:00");
        // Ayer ya lleva fecha, sin ano por ser del mismo.
        assert_eq!(etiqueta_hora(AHORA - 86_400_000, AHORA), "14/09");
        // Y de otro ano, con ano.
        assert_eq!(etiqueta_hora(AHORA - 400 * 86_400_000, AHORA), "11/08/25");
    }

    #[test]
    fn la_pildora_del_dia_siempre_es_fecha_nunca_una_hora() {
        // Justo el caso que fallaba: un mensaje de hoy separaba los dias
        // con «14:32», que no separa nada.
        assert_eq!(etiqueta_fecha(AHORA, AHORA), "15/09");
        assert_eq!(etiqueta_fecha(AHORA - 86_400_000, AHORA), "14/09");
        assert_eq!(etiqueta_fecha(AHORA - 400 * 86_400_000, AHORA), "11/08/25");
        assert_eq!(etiqueta_fecha(0, AHORA), "");
    }

    #[test]
    fn sin_fecha_no_se_inventa_nada() {
        assert_eq!(etiqueta_hora(0, AHORA), "");
        assert_eq!(etiqueta_hora(-5, AHORA), "");
    }

    #[test]
    fn el_calendario_acierta_en_los_bisiestos() {
        // 2000 es bisiesto (divisible entre 400) y 1900 no lo era.
        assert_eq!(civil(11_016), (2000, 2, 29));
        assert_eq!(civil(11_017), (2000, 3, 1));
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(-1), (1969, 12, 31));
    }

    #[test]
    fn la_caja_de_escribir_crece_con_el_texto_pero_no_se_come_el_historial() {
        let d = Disposicion::calcular(1024, 768, 100, 300, Vista::Ambas);
        let una_linea = d.alto_redaccion(18, 100);
        assert_eq!(una_linea, REDACCION_MINIMA, "vacia, la de siempre");
        // Con varias lineas sube...
        let cinco = d.alto_redaccion(5 * 18, 100);
        assert!(cinco > una_linea);
        // ...pero se planta.
        assert_eq!(d.alto_redaccion(10_000, 100), REDACCION_MAXIMA);
        // Y el historial siempre queda entre la cabecera y la caja.
        for alto_texto in [18, 90, 10_000] {
            let caja = d.redaccion(alto_texto, 100);
            let hist = d.historial(alto_texto, false, 100);
            assert_eq!(caja.abajo(), d.chat.abajo(), "pegada abajo");
            assert_eq!(hist.y, d.cabecera_chat.abajo());
            assert_eq!(hist.abajo(), caja.y, "sin hueco ni solape");
            assert!(hist.alto > 0, "el historial no puede desaparecer");
        }
    }

    #[test]
    fn en_una_ventana_bajita_la_caja_no_se_lleva_mas_de_media_columna() {
        // Caso negativo: la mas pequena que se admite, con un texto enorme.
        let d = Disposicion::calcular(
            ANCHO_MINIMO_VENTANA * 2,
            ALTO_MINIMO_VENTANA,
            100,
            LISTA_MINIMA,
            Vista::Ambas,
        );
        let caja = d.redaccion(10_000, 100);
        assert!(
            caja.alto <= d.chat.alto / 2,
            "{} de {}",
            caja.alto,
            d.chat.alto
        );
        assert!(d.historial(10_000, false, 100).alto > 0);
    }
}

// --- Como en el chat del movil -------------------------------------------
//
// Lo que sigue se copia de PixPin Android (`guardados/MensajesActivity.kt`,
// `CabeceraFlotante.kt` y `HojasDelProyecto.kt`, v0.51.0): el usuario compara
// pantalla con pantalla y las dos apps tienen que ser la misma.

/// El alto de cada pastilla de la cabecera y su separacion
/// (`ALTO_DE_LA_PILDORA` y `AIRE_DE_LA_PILDORA` de `CabeceraFlotante.kt`).
pub const PILDORA: u32 = 46;
pub const PILDORA_AIRE: u32 = 6;
/// La de la derecha lleva dos botones de 48: la lupa y los tres puntos.
pub const PILDORA_BOTON: u32 = 48;
/// La del centro se mide a su contenido, pero no pasa de aqui.
pub const PILDORA_CENTRO_MAXIMA: u32 = 420;

/// Las tres pastillas flotantes de la cabecera del proyecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pildoras {
    /// El circulo de volver, a la izquierda.
    pub volver: Rect,
    /// El sitio donde se centra la del titulo: quien pinta la mide a su
    /// texto y la centra aqui dentro.
    pub centro: Rect,
    /// La pastilla de la derecha entera, y sus dos botones.
    pub derecha: Rect,
    pub buscar: Rect,
    pub menu: Rect,
}

impl Disposicion {
    /// Las pastillas de la cabecera del proyecto, flotando sobre el papel.
    ///
    /// La del centro se queda con lo que sobra entre las otras dos, como en
    /// el movil («centrada entre las otras dos»); en una columna estrecha se
    /// encoge hasta nada antes de montarse encima de los botones.
    pub fn pildoras(&self, escala_por_cien: u32) -> Pildoras {
        let e = |v: u32| v * escala_por_cien / 100;
        let c = self.cabecera_chat;
        let alto = e(PILDORA).min(c.alto);
        let y = c.y + (c.alto as i32 - alto as i32) / 2;
        let aire = e(PILDORA_AIRE) as i32;
        let volver = Rect {
            x: c.x + aire,
            y,
            ancho: alto,
            alto,
        };
        let ancho_derecha = (2 * e(PILDORA_BOTON)).min(c.ancho);
        let derecha = Rect {
            x: c.derecha() - aire - ancho_derecha as i32,
            y,
            ancho: ancho_derecha,
            alto,
        };
        let mitad = ancho_derecha / 2;
        let buscar = Rect {
            ancho: mitad,
            ..derecha
        };
        let menu = Rect {
            x: derecha.x + mitad as i32,
            ancho: ancho_derecha - mitad,
            ..derecha
        };
        let izquierda = volver.derecha() + aire;
        let centro = Rect {
            x: izquierda,
            y,
            ancho: (derecha.x - aire - izquierda).max(0) as u32,
            alto,
        };
        Pildoras {
            volver,
            centro,
            derecha,
            buscar,
            menu,
        }
    }
}

/// La pastilla del titulo, ya medida: `contenido` es lo que ocupa lo de
/// dentro (disco, nombre y flecha) sin su relleno de 14 por lado.
pub fn pildora_centro(sitio: Rect, contenido: u32, escala_por_cien: u32) -> Rect {
    let e = |v: u32| v * escala_por_cien / 100;
    let ancho = (contenido + 2 * e(14))
        .min(e(PILDORA_CENTRO_MAXIMA))
        .min(sitio.ancho);
    Rect {
        x: sitio.x + (sitio.ancho as i32 - ancho as i32) / 2,
        ancho,
        ..sitio
    }
}

/// Coloca un menu que nace en un punto: hacia abajo y a la derecha si cabe,
/// y si no, hacia arriba o hacia la izquierda. Nunca se sale de `limite`.
///
/// Es como sale el menu de un mensaje en el movil, donde se toco
/// (`DropdownMenu` con `offset = dondeElDedo`), y el de los tres puntos,
/// colgando de su boton.
pub fn colocar_menu(
    ancla: Punto,
    limite: Rect,
    anchos_de_texto: &[f32],
    alto_texto: u32,
    escala_por_cien: u32,
) -> crate::menu::Menu {
    use crate::menu;
    // El calculo del ancho y del alto es el de siempre; solo cambia donde va.
    let base = menu::desplegar(
        Rect {
            x: 0,
            y: 0,
            ancho: 0,
            alto: 0,
        },
        Rect {
            x: 0,
            y: 0,
            ancho: u32::MAX / 4,
            alto: u32::MAX / 4,
        },
        anchos_de_texto,
        alto_texto,
        escala_por_cien,
    );
    let (ancho, alto) = (base.caja.ancho as i32, base.caja.alto as i32);
    let x = if ancla.x + ancho <= limite.derecha() {
        ancla.x
    } else {
        ancla.x - ancho
    };
    let y = if ancla.y + alto <= limite.abajo() {
        ancla.y
    } else {
        ancla.y - alto
    };
    menu::Menu {
        caja: Rect {
            x: x.min(limite.derecha() - ancho).max(limite.x),
            y: y.min(limite.abajo() - alto).max(limite.y),
            ancho: base.caja.ancho,
            alto: base.caja.alto,
        },
        ..base
    }
}

/// Los colores del contorno de una burbuja que viene de un lienzo: azul,
/// verde, naranja, morado, rojo y turquesa (`HojasDelProyecto.COLORES`).
pub const COLORES_DEL_BORDE: [u32; 6] =
    [0x4c7df0, 0x2fa84f, 0xe0803a, 0x9b59d0, 0xd64b6a, 0x12a5a5];

/// El color del contorno de la burbuja de un mensaje con `referencia` (el
/// lienzo, la tabla o el dibujo de la foto), o `None` si no tiene.
///
/// Es la cuenta del movil (`HojasDelProyecto.colorDe`): el `hashCode` de Java
/// de la cadena, en modulo positivo. Tiene que ser EXACTAMENTE esa, o el mismo
/// lienzo saldria de un color en el movil y de otro aqui, y el color es lo que
/// dice de que lienzo viene cada hoja.
pub fn color_del_borde(referencia: &str) -> Option<u32> {
    if referencia.is_empty() {
        return None;
    }
    // `String.hashCode`: s[0]*31^(n-1) + ... sobre unidades UTF-16, con el
    // desbordamiento de un `int`.
    let hash = referencia
        .encode_utf16()
        .fold(0i32, |h, u| h.wrapping_mul(31).wrapping_add(u as i32));
    let cual = hash.rem_euclid(COLORES_DEL_BORDE.len() as i32) as usize;
    Some(COLORES_DEL_BORDE[cual])
}

/// Un tamano como lo escribe el movil (`Formatter.formatShortFileSize`): de
/// mil en mil, con un decimal por debajo de diez y sin el de ahi en
/// adelante. «949 kB», «66 kB», «1.1 MB».
pub fn tamano_corto(bytes: u64) -> String {
    const UNIDADES: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut valor = bytes as f64;
    let mut cual = 0;
    while valor >= 1000.0 && cual + 1 < UNIDADES.len() {
        valor /= 1000.0;
        cual += 1;
    }
    if cual == 0 {
        return format!("{bytes} B");
    }
    if valor < 10.0 {
        format!("{valor:.1} {}", UNIDADES[cual])
    } else {
        format!("{valor:.0} {}", UNIDADES[cual])
    }
}

/// Las etiquetas que se le pueden poner a un mensaje, las del movil
/// (`ETIQUETAS` de `MensajesActivity.kt`) y en su orden.
pub const ETIQUETAS: [&str; 6] = [
    "\u{2b50}",
    "\u{2705}",
    "\u{23f3}",
    "\u{1f4a1}",
    "\u{1f4b0}",
    "\u{1f4cd}",
];

/// La fila de dentro de una burbuja: el boton redondo de 44, el punto de si
/// vive en un proyecto, el nombre y el boton de sacarlo a la pantalla
/// (`FilaDeArchivo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilaArchivo {
    pub circulo: Rect,
    pub punto: Rect,
    /// Donde empieza el texto y cuanto puede ocupar.
    pub texto: Rect,
    pub abrir: Rect,
}

/// Lo que mide el boton redondo de la fila (`BOTON_DEL_ARCHIVO`).
pub const FILA_CIRCULO: u32 = 44;
/// El punto verde, con su halo blanco de 2.
pub const FILA_PUNTO: u32 = 12;
/// La pastilla de «abrir fuera»: 30 x 24, esquinas de 8
/// (`ANCHO_DEL_BOTON_DE_TEXTO`, `ALTO_DEL_BOTON_DE_TEXTO`).
pub const FILA_ABRIR_ANCHO: u32 = 30;
pub const FILA_ABRIR_ALTO: u32 = 24;
pub const FILA_ABRIR_RADIO: u32 = 8;

/// Coloca la fila de un archivo en `x, y`, con un texto que mide
/// `ancho_texto` y sin pasar de `ancho_max` en total.
///
/// El nombre CEDE sitio a la pastilla: con un nombre largo, la pastilla no
/// puede quedarse fuera de la burbuja (lo reporto el usuario en el movil el
/// 9-sep-2026, `weight(1f, fill = false)`).
pub fn fila_archivo(
    x: i32,
    y: i32,
    ancho_texto: u32,
    ancho_max: u32,
    escala_por_cien: u32,
) -> FilaArchivo {
    let e = |v: u32| v * escala_por_cien / 100;
    let circulo = Rect {
        x,
        y,
        ancho: e(FILA_CIRCULO),
        alto: e(FILA_CIRCULO),
    };
    let medio = y + e(FILA_CIRCULO) as i32 / 2;
    let punto = Rect {
        x: circulo.derecha() + e(6) as i32,
        y: medio - e(FILA_PUNTO) as i32 / 2,
        ancho: e(FILA_PUNTO),
        alto: e(FILA_PUNTO),
    };
    let texto_x = punto.derecha() + e(10) as i32;
    let reservado = (texto_x - x).max(0) as u32 + e(8) + e(FILA_ABRIR_ANCHO);
    let ancho = ancho_texto.min(ancho_max.saturating_sub(reservado));
    let texto = Rect {
        x: texto_x,
        y,
        ancho,
        alto: e(FILA_CIRCULO),
    };
    let abrir = Rect {
        x: texto.derecha() + e(8) as i32,
        y: medio - e(FILA_ABRIR_ALTO) as i32 / 2,
        ancho: e(FILA_ABRIR_ANCHO),
        alto: e(FILA_ABRIR_ALTO),
    };
    FilaArchivo {
        circulo,
        punto,
        texto,
        abrir,
    }
}

/// Lo ancho que ocupa una fila de archivo con un texto de `ancho_texto`.
pub fn ancho_fila_archivo(ancho_texto: u32, escala_por_cien: u32) -> u32 {
    let f = fila_archivo(0, 0, ancho_texto, u32::MAX / 4, escala_por_cien);
    f.abrir.derecha().max(0) as u32
}

#[cfg(test)]
mod pruebas_movil {
    use super::*;

    fn disposicion() -> Disposicion {
        Disposicion::calcular(1024, 768, 100, 300, Vista::Ambas)
    }

    #[test]
    fn las_tres_pastillas_caben_en_la_cabecera_sin_pisarse() {
        let d = disposicion();
        let p = d.pildoras(100);
        let c = d.cabecera_chat;
        for r in [p.volver, p.centro, p.derecha] {
            assert!(
                r.x >= c.x && r.derecha() <= c.derecha(),
                "{r:?} fuera de {c:?}"
            );
            assert!(r.y >= c.y && r.abajo() <= c.abajo(), "{r:?} fuera de {c:?}");
        }
        assert!(p.volver.derecha() < p.centro.x);
        assert!(p.centro.derecha() < p.derecha.x);
        // Volver es un circulo de 46 y la de la derecha, dos botones de 48.
        assert_eq!((p.volver.ancho, p.volver.alto), (PILDORA, PILDORA));
        assert_eq!(p.derecha.ancho, 2 * PILDORA_BOTON);
        assert_eq!(p.buscar.derecha(), p.menu.x, "la lupa y los puntos, juntos");
        assert_eq!(p.menu.derecha(), p.derecha.derecha());
    }

    #[test]
    fn en_una_columna_estrecha_el_titulo_se_encoge_y_no_monta_los_botones() {
        // Caso negativo: una columna apenas mas ancha que los botones.
        let mut d = disposicion();
        d.cabecera_chat.ancho = 120;
        let p = d.pildoras(100);
        assert_eq!(p.centro.ancho, 0);
        assert_eq!(pildora_centro(p.centro, 500, 100).ancho, 0);
    }

    #[test]
    fn la_pastilla_del_titulo_se_mide_a_su_texto_y_se_centra() {
        let sitio = Rect {
            x: 100,
            y: 10,
            ancho: 600,
            alto: 46,
        };
        let r = pildora_centro(sitio, 200, 100);
        assert_eq!(r.ancho, 228, "el texto y 14 por cada lado");
        assert_eq!(r.x - sitio.x, sitio.derecha() - r.derecha(), "centrada");
        // Y no pasa de 420 por largo que sea el nombre.
        assert_eq!(
            pildora_centro(sitio, 5000, 100).ancho,
            PILDORA_CENTRO_MAXIMA
        );
    }

    #[test]
    fn el_menu_nace_donde_se_pulso_y_se_da_la_vuelta_si_no_cabe() {
        let limite = Rect {
            x: 0,
            y: 0,
            ancho: 800,
            alto: 600,
        };
        let anchos = [80.0; 5];
        let m = colocar_menu(Punto { x: 100, y: 100 }, limite, &anchos, 16, 100);
        assert_eq!(
            (m.caja.x, m.caja.y),
            (100, 100),
            "hacia abajo y a la derecha"
        );
        // Caso negativo: pegado a la esquina de abajo a la derecha no puede
        // salirse; se abre hacia arriba y hacia la izquierda.
        let m = colocar_menu(Punto { x: 790, y: 590 }, limite, &anchos, 16, 100);
        assert!(
            m.caja.derecha() <= 790 && m.caja.abajo() <= 590,
            "{:?}",
            m.caja
        );
        assert!(m.caja.x >= 0 && m.caja.y >= 0);
        assert_eq!(m.cuantas, 5);
    }

    #[test]
    fn un_menu_mas_alto_que_la_ventana_se_pega_arriba_y_no_se_sale() {
        let limite = Rect {
            x: 0,
            y: 0,
            ancho: 800,
            alto: 120,
        };
        let m = colocar_menu(Punto { x: 10, y: 60 }, limite, &[50.0; 12], 16, 100);
        assert_eq!(m.caja.y, 0);
    }

    #[test]
    fn el_color_del_borde_es_el_mismo_que_calcula_el_movil() {
        // "abc".hashCode() de Java es 96354, y 96354 % 6 = 0: el azul.
        assert_eq!(color_del_borde("abc"), Some(COLORES_DEL_BORDE[0]));
        // "polygenelubricants".hashCode() es Integer.MIN_VALUE: floorMod da
        // 4 (el rojo) y no un indice negativo.
        assert_eq!(
            color_del_borde("polygenelubricants"),
            Some(COLORES_DEL_BORDE[4])
        );
        // El mismo lienzo, siempre el mismo color.
        assert_eq!(color_del_borde("ZD8TP7BZYC"), color_del_borde("ZD8TP7BZYC"));
    }

    #[test]
    fn sin_referencia_no_hay_contorno() {
        assert_eq!(color_del_borde(""), None);
    }

    #[test]
    fn el_tamano_se_escribe_como_en_el_movil() {
        assert_eq!(tamano_corto(949_000), "949 kB");
        assert_eq!(tamano_corto(66_000), "66 kB");
        assert_eq!(tamano_corto(1_100_000), "1.1 MB");
        assert_eq!(tamano_corto(512), "512 B");
        // Caso negativo: nada no es «0.0 kB».
        assert_eq!(tamano_corto(0), "0 B");
    }

    #[test]
    fn la_fila_de_un_archivo_va_en_su_orden_y_el_nombre_cede_sitio() {
        let f = fila_archivo(10, 20, 150, 400, 100);
        assert_eq!((f.circulo.ancho, f.circulo.alto), (44, 44));
        assert!(f.circulo.derecha() < f.punto.x);
        assert!(f.punto.derecha() < f.texto.x);
        assert!(f.texto.derecha() < f.abrir.x);
        assert_eq!(f.texto.ancho, 150);
        assert_eq!(
            ancho_fila_archivo(150, 100),
            (f.abrir.derecha() - 10) as u32
        );
        // Caso negativo: un nombre larguisimo no empuja la pastilla fuera.
        let f = fila_archivo(10, 20, 5000, 300, 100);
        assert!(f.abrir.derecha() <= 10 + 300, "{:?}", f.abrir);
    }

    #[test]
    fn la_isla_de_escribir_lleva_el_clip_dentro_y_el_microfono_fuera() {
        let d = disposicion();
        let isla = d.isla(18, 100);
        let campo = d.campo(18, 100);
        let clip = d.boton_adjuntar(18, 100);
        let voz = d.boton_enviar(18, 100);
        let caja = d.redaccion(18, 100);
        assert_eq!(isla.x - caja.x, ISLA_MARGEN_X as i32);
        assert_eq!(caja.abajo() - isla.abajo(), ISLA_BAJO as i32);
        assert!(campo.x >= isla.x && campo.derecha() <= voz.x);
        assert!(
            clip.x >= campo.x && clip.derecha() == campo.derecha(),
            "el clip, dentro del campo"
        );
        assert!(voz.x >= campo.derecha() && voz.derecha() <= isla.derecha());
        assert_eq!(campo.alto, CAMPO_FILA, "vacia, una fila de 44");
        let texto = d.texto_redaccion(18, 100);
        assert!(
            texto.derecha() <= clip.x,
            "el texto no se mete bajo el clip"
        );
    }

    #[test]
    fn la_barra_de_responder_aparta_el_historial_y_no_encoge_la_isla() {
        let sin = disposicion();
        let mut con = disposicion();
        con.encima_de_la_isla = 40;
        let (isla_sin, isla_con) = (sin.isla(18, 100), con.isla(18, 100));
        assert_eq!(isla_sin.alto, isla_con.alto, "la isla mide lo mismo");
        assert_eq!(isla_sin.abajo(), isla_con.abajo(), "y sigue abajo");
        let barra = con.encima_de_la_isla(18, 100);
        assert_eq!(barra.alto, 40);
        assert!(
            barra.abajo() <= isla_con.y,
            "la barra va encima, sin pisarla"
        );
        assert_eq!(
            con.historial(18, false, 100).abajo(),
            con.redaccion(18, 100).y,
            "el historial se aparta"
        );
        // Caso negativo: sin barra no se reserva nada.
        assert_eq!(sin.encima_de_la_isla(18, 100).alto, 0);
    }
}
