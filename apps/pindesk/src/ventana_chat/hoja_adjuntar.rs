//! **La hoja del «+» de la caja**, como la «Adjuntar» del iPhone (2-nov-2026,
//! el usuario: «que sea mas como iOS: lista en dos columnas con sus iconos;
//! quita la opcion de buscar porque son pocas opciones»):
//!
//! - arriba, «Cerrar» en una pastilla a la izquierda y el titulo en medio;
//! - lo de todos los dias en una cuadricula de cuatro: el cuadrado de color
//!   con su icono blanco y el nombre debajo (foto, archivo, lienzo, nota,
//!   lista de tareas, tabla y captura, que en el PC hace de «Cámara»);
//! - una raya, «Más» y lo demas en **dos columnas** de pastillas grises con
//!   su icono y su nombre;
//! - otra raya y, abajo, «Pegar lo copiado» con su boton azul claro (apagado
//!   si no hay nada copiado).
//!
//! Sin buscador: son pocas y se ven todas. Teclado: las flechas recorren
//! (arriba y abajo van a lo que queda debajo o encima), Intro hace lo
//! elegido, los numeros del 1 al 7 las baldosas y Esc cierra.
//!
//! Cada entrada hace EXACTAMENTE lo que hacia la del menu del clip de antes
//! (la misma `Accion`); la hoja solo cambia como se elige.

use pindesk_geom::{Punto, Rect};
use pindesk_render::icono::Icono;
use pindesk_render::{Color, Pintor, RectF};
use pindesk_store::Catalogo;

use super::{Accion, Pinta, Respuesta, con_alfa, encoger, hex, mi, rf};

/// Medidas en pixeles logicos (al 100 %).
pub(super) const ANCHO: u32 = 480;
pub(super) const RELLENO: u32 = 16;
pub(super) const RADIO: u32 = 22;
pub(super) const CABECERA: u32 = 34;
pub(super) const HUECO: u32 = 12;
pub(super) const COLUMNAS: u32 = 4;
pub(super) const BALDOSA_ALTO: u32 = 90;
pub(super) const BALDOSA_HUECO: u32 = 4;
pub(super) const ICONO_CAJA: u32 = 56;
pub(super) const CABECERA_MAS: u32 = 20;
pub(super) const COLUMNAS_CHIP: u32 = 2;
pub(super) const CHIP_ALTO: u32 = 36;
pub(super) const CHIP_HUECO: u32 = 8;
pub(super) const PIE: u32 = 36;
/// Lo que se separa del boton que la abre.
pub(super) const SOBRE_EL_BOTON: u32 = 8;
const TAM: f32 = 13.0;

/// Como se ensena una entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Forma {
    /// El cuadrado de color de la cuadricula, con su tecla del 1 al 7.
    Baldosa(u8),
    /// Una pastilla de las dos columnas de «Más».
    Chip,
    /// El boton «Pegar» de abajo.
    Pegar,
}

/// Una entrada de la hoja, ya traducida.
pub(super) struct Opcion {
    pub rotulo: String,
    pub accion: Accion,
    /// El color de su cuadrado (solo las baldosas).
    pub color: u32,
    pub icono: &'static Icono,
    pub forma: Forma,
}

/// La hoja abierta y lo resaltado (por indice de `opciones`).
pub(super) struct Hoja {
    pub opciones: Vec<Opcion>,
    /// Bajo el raton.
    pub sobre: Option<usize>,
    /// Elegido con las flechas (lo que hace Intro).
    pub elegido: Option<usize>,
    /// Si hay algo en el portapapeles: si no, «Pegar» va apagado.
    pub hay_copiado: bool,
    /// Con el raton encima de «Cerrar».
    pub sobre_cerrar: bool,
    titulo: String,
    cerrar: String,
    titulo_mas: String,
    pegar_copiado: String,
}

impl Hoja {
    pub(super) fn nueva(textos: &Catalogo) -> Hoja {
        use pindesk_proyecto::mini;
        let baldosa =
            |n: u8, clave: &str, accion: Accion, color: u32, icono: &'static Icono| Opcion {
                rotulo: textos.t(clave),
                accion,
                color,
                icono,
                forma: Forma::Baldosa(n),
            };
        let chip = |clave: &str, accion: Accion, icono: &'static Icono| Opcion {
            rotulo: textos.t(clave),
            accion,
            color: 0,
            icono,
            forma: Forma::Chip,
        };
        // Los colores del iPhone: azul, gris, naranja, verde, verde, indigo
        // y rojo.
        let opciones = vec![
            baldosa(
                1,
                "adjuntar-imagen",
                Accion::AdjImagen,
                0x0a84ff,
                &mi::IMAGE,
            ),
            baldosa(
                2,
                "adjuntar-archivo",
                Accion::AdjArchivo,
                0x8e8e93,
                &mi::DESCRIPTION,
            ),
            baldosa(
                3,
                "v2menus-adj-lienzo",
                Accion::AdjLienzo,
                0xff9500,
                &mi::DRAW,
            ),
            baldosa(
                4,
                "chat-adj-nota-md",
                Accion::AdjNotaMd,
                0x34c759,
                &mi::LIST,
            ),
            baldosa(
                5,
                "v2menus-adj-tareas",
                Accion::AdjMini(mini::TAREAS),
                0x30b350,
                &mi::CHECKLIST,
            ),
            baldosa(
                6,
                "v2menus-adj-tabla",
                Accion::AdjTabla,
                0x5856d6,
                &mi::TABLE_CHART,
            ),
            // La «Cámara» del movil: en el PC, la captura de una zona.
            baldosa(
                7,
                "v2menus-adj-captura",
                Accion::AdjCaptura,
                0xff3b30,
                &mi::CROP,
            ),
            // «Más», de dos en dos en el orden del iPhone.
            chip("v2menus-adj-proyecto", Accion::AdjProyectos, &mi::FOLDER),
            chip("v2menus-adj-pagina", Accion::AdjPagina, &mi::DESCRIPTION),
            // Una figura de la biblioteca (25-oct-2026): abre la ventana de
            // las figuras para elegirla viendola, con su boton «Adjuntar».
            chip("asistente-adj-figura", Accion::AdjFigura, &mi::PALETTE),
            // Las carpetas de paginas (26-oct-2026): un cuaderno con nombre.
            chip("carpeta-nueva", Accion::AdjCarpeta, &mi::MENU_BOOK),
            chip(
                "mini-contador",
                Accion::AdjMini(mini::CONTADOR),
                &mi::NUMBERS,
            ),
            chip("mini-ruleta", Accion::AdjMini(mini::RULETA), &mi::REPLAY),
            chip("mini-gastos", Accion::AdjMini(mini::GASTOS), &mi::EDIT_NOTE),
            chip(
                "mini-cronometro",
                Accion::AdjMini(mini::CRONOMETRO),
                &mi::TIMER,
            ),
            chip(
                "mini-temporizador",
                Accion::AdjMini(mini::TEMPORIZADOR),
                &mi::SPEED,
            ),
            chip("mini-alarma", Accion::AdjMini(mini::ALARMA), &mi::ALARM),
            chip("v2menus-adj-conversacion", Accion::Conversacion, &mi::FORUM),
            chip("chat-adj-telepronter", Accion::Telepronter, &mi::SUBTITLES),
            chip(
                "chat-adj-pronunciar",
                Accion::Pronunciar,
                &mi::RECORD_VOICE_OVER,
            ),
            // Musica con su letra (27-oct-2026): canciones para cantarlas.
            chip("chat-adj-musica", Accion::AdjMusica, &mi::MUSIC_NOTE),
            chip("v2menus-adj-movil", Accion::AdjDelMovil, &mi::WIFI),
            Opcion {
                rotulo: textos.t("lienzo-pegar-menu"),
                accion: Accion::PegarLoCopiado,
                color: 0,
                icono: &mi::CONTENT_PASTE,
                forma: Forma::Pegar,
            },
        ];
        Hoja {
            opciones,
            sobre: None,
            elegido: None,
            hay_copiado: false,
            sobre_cerrar: false,
            titulo: textos.t("chat-adjuntar"),
            cerrar: textos.t("pin-cerrar"),
            titulo_mas: textos.t("v2menus-adj-mas"),
            pegar_copiado: textos.t("v2menus-adj-pegar-copiado"),
        }
    }

    /// Con lo que diga el portapapeles: sin nada copiado, «Pegar» va apagado.
    pub(super) fn con_copiado(mut self, hay: bool) -> Hoja {
        self.hay_copiado = hay;
        self
    }

    /// **La hoja del chat del asistente** (25-oct-2026, el usuario:
    /// «especializalo para el asistente … que pueda adjuntar figuras para que
    /// las mejore, o decirle hazme una figura»): primero **Figura** (una de la
    /// biblioteca, a un lienzo del chat, para mejorarla) y lo que se le suele
    /// dar a mirar; en «Más», pedir una figura nueva y lo demas que sirve.
    pub(super) fn para_el_asistente(textos: &Catalogo) -> Hoja {
        let mut h = Hoja::nueva(textos);
        let mut de_siempre = std::mem::take(&mut h.opciones);
        let mut sacar = |accion: Accion| {
            de_siempre
                .iter()
                .position(|o| o.accion == accion)
                .map(|i| de_siempre.remove(i))
        };
        let mut opciones = vec![Opcion {
            rotulo: textos.t("asistente-adj-figura"),
            accion: Accion::AdjFigura,
            color: 0xaf52de,
            icono: &mi::PALETTE,
            forma: Forma::Baldosa(1),
        }];
        for accion in [
            Accion::AdjImagen,
            Accion::AdjArchivo,
            Accion::AdjCaptura,
            Accion::AdjLienzo,
            Accion::AdjNotaMd,
        ] {
            opciones.extend(sacar(accion));
        }
        for (n, o) in opciones.iter_mut().enumerate() {
            o.forma = Forma::Baldosa(n as u8 + 1);
        }
        opciones.push(Opcion {
            rotulo: textos.t("asistente-adj-pedir-figura"),
            accion: Accion::PedirFigura,
            color: 0,
            icono: &mi::LIGHTBULB,
            forma: Forma::Chip,
        });
        for accion in [
            Accion::AdjPagina,
            Accion::AdjProyectos,
            Accion::AdjTabla,
            Accion::AdjDelMovil,
            Accion::PegarLoCopiado,
        ] {
            if let Some(mut o) = sacar(accion) {
                if o.forma != Forma::Pegar {
                    o.forma = Forma::Chip;
                }
                opciones.push(o);
            }
        }
        h.opciones = opciones;
        h
    }

    /// Lo que se puede elegir, en el orden de la hoja (baldosas, chips y
    /// «Pegar» si hay algo copiado).
    pub(super) fn elegibles(&self) -> Vec<usize> {
        self.opciones
            .iter()
            .enumerate()
            .filter(|(_, o)| o.forma != Forma::Pegar || self.hay_copiado)
            .map(|(n, _)| n)
            .collect()
    }

    /// Donde va todo, calculado igual al pintar y al pulsar.
    pub(super) fn colocar(&self, ancla: Punto, marco: Rect, escala: u32) -> Puesta {
        colocar(&self.opciones, ancla, marco, escala)
    }

    pub(super) fn pulsar(&mut self, p: Punto, ancla: Punto, marco: Rect, escala: u32) -> Respuesta {
        let puesta = self.colocar(ancla, marco, escala);
        if puesta.cerrar.contiene(p) {
            return Respuesta::Fuera;
        }
        match puesta.opcion_en(p) {
            Some(n) if self.elegibles().contains(&n) => {
                Respuesta::Hacer(self.opciones[n].accion.clone())
            }
            _ if puesta.caja.contiene(p) => Respuesta::Sigue,
            _ => Respuesta::Fuera,
        }
    }

    pub(super) fn mover(&mut self, p: Punto, ancla: Punto, marco: Rect, escala: u32) -> bool {
        let puesta = self.colocar(ancla, marco, escala);
        let elegibles = self.elegibles();
        let nuevo = puesta.opcion_en(p).filter(|n| elegibles.contains(n));
        let cerrar = puesta.cerrar.contiene(p);
        let cambia = nuevo != self.sobre || cerrar != self.sobre_cerrar;
        self.sobre = nuevo;
        self.sobre_cerrar = cerrar;
        cambia
    }

    pub(super) fn tecla(&mut self, vk: u32) -> Respuesta {
        match vk {
            super::VK_ESCAPE => Respuesta::Fuera,
            super::VK_ENTRAR => match self.elegido.filter(|n| self.elegibles().contains(n)) {
                Some(n) => Respuesta::Hacer(self.opciones[n].accion.clone()),
                None => Respuesta::Sigue,
            },
            super::VK_DERECHA | super::VK_IZQUIERDA | super::VK_ABAJO | super::VK_ARRIBA => {
                // Las cuentas de sitio no dependen de donde este la hoja:
                // se coloca en un marco cualquiera y se mira lo de alrededor.
                let marco = Rect {
                    x: 0,
                    y: 0,
                    ancho: 4 * ANCHO,
                    alto: 4_000,
                };
                let puesta = self.colocar(Punto { x: 0, y: 3_000 }, marco, 100);
                let sitios: Vec<(usize, Rect)> = self
                    .elegibles()
                    .into_iter()
                    .filter_map(|n| puesta.sitio_de(n).map(|r| (n, r)))
                    .collect();
                self.elegido = mover_eleccion(self.elegido, vk, &sitios);
                Respuesta::Sigue
            }
            _ => Respuesta::Sigue,
        }
    }

    /// Un numero del 1 al 7 hace su baldosa; lo demas no se escribe en
    /// ningun sitio (la hoja no tiene buscador).
    pub(super) fn caracter(&mut self, c: char) -> Respuesta {
        let Some(d) = c.to_digit(10) else {
            return Respuesta::Sigue;
        };
        match self
            .opciones
            .iter()
            .find(|o| o.forma == Forma::Baldosa(d as u8))
        {
            Some(o) => Respuesta::Hacer(o.accion.clone()),
            None => Respuesta::Sigue,
        }
    }
}

/// Mover lo elegido con las flechas entre `sitios` (indice de opcion y su
/// caja, en el orden de la hoja): izquierda y derecha de uno en uno; arriba
/// y abajo, a la fila de encima o de debajo, a lo que cae mas cerca en x.
pub(super) fn mover_eleccion(
    actual: Option<usize>,
    vk: u32,
    sitios: &[(usize, Rect)],
) -> Option<usize> {
    if sitios.is_empty() {
        return None;
    }
    let Some(k) = actual.and_then(|a| sitios.iter().position(|(n, _)| *n == a)) else {
        return Some(sitios[0].0);
    };
    let centro = |r: &Rect| (r.x + r.ancho as i32 / 2, r.y + r.alto as i32 / 2);
    let (cx, cy) = centro(&sitios[k].1);
    let nuevo = match vk {
        super::VK_DERECHA => (k + 1).min(sitios.len() - 1),
        super::VK_IZQUIERDA => k.saturating_sub(1),
        super::VK_ABAJO | super::VK_ARRIBA => {
            let abajo = vk == super::VK_ABAJO;
            sitios
                .iter()
                .enumerate()
                .filter(|(_, (_, r))| {
                    let y = centro(r).1;
                    if abajo { y > cy } else { y < cy }
                })
                .min_by_key(|(_, (_, r))| {
                    let (x, y) = centro(r);
                    ((y - cy).abs(), (x - cx).abs())
                })
                .map(|(j, _)| j)
                .unwrap_or(k)
        }
        _ => k,
    };
    Some(sitios[nuevo].0)
}

/// La hoja ya colocada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Puesta {
    pub caja: Rect,
    /// La pastilla «Cerrar» y la fila del titulo.
    pub cerrar: Rect,
    pub titulo: Rect,
    /// Indice de `opciones` y su sitio.
    pub baldosas: Vec<(usize, Rect)>,
    /// Las rayas: debajo de las baldosas y encima del pie.
    pub rayas: Vec<Rect>,
    pub cabecera_mas: Option<Rect>,
    pub chips: Vec<(usize, Rect)>,
    /// La fila de abajo («Pegar lo copiado») y su boton.
    pub pie: Option<Rect>,
    pub pegar: Option<(usize, Rect)>,
}

impl Puesta {
    pub(super) fn opcion_en(&self, p: Punto) -> Option<usize> {
        self.baldosas
            .iter()
            .chain(self.chips.iter())
            .chain(self.pegar.iter())
            .find(|(_, r)| r.contiene(p))
            .map(|(n, _)| *n)
    }

    pub(super) fn sitio_de(&self, n: usize) -> Option<Rect> {
        self.baldosas
            .iter()
            .chain(self.chips.iter())
            .chain(self.pegar.iter())
            .find(|(k, _)| *k == n)
            .map(|(_, r)| *r)
    }
}

/// Coloca la hoja: encima de `ancla` (el boton «+»), alineada con el y
/// dentro de la ventana.
pub(super) fn colocar(opciones: &[Opcion], ancla: Punto, marco: Rect, escala: u32) -> Puesta {
    let e = |v: u32| v * escala / 100;
    let ancho = e(ANCHO).min(marco.ancho.saturating_sub(e(16)));
    let dentro = ancho.saturating_sub(2 * e(RELLENO));
    // Primero todo con la caja en (0, 0); luego se mueve a su sitio.
    let x0 = e(RELLENO) as i32;
    let mut y = e(RELLENO) as i32;
    let titulo = Rect {
        x: x0,
        y,
        ancho: dentro,
        alto: e(CABECERA),
    };
    // «Cerrar» lo mide el pintor; aqui un ancho que le sobra a todos.
    let cerrar = Rect {
        x: x0,
        y,
        ancho: e(84).min(dentro / 3),
        alto: e(CABECERA),
    };
    y = titulo.abajo() + e(HUECO) as i32;
    let col = COLUMNAS;
    let ancho_baldosa = dentro.saturating_sub((col - 1) * e(BALDOSA_HUECO)) / col;
    let de_baldosa: Vec<usize> = (0..opciones.len())
        .filter(|n| matches!(opciones[*n].forma, Forma::Baldosa(_)))
        .collect();
    let mut baldosas = Vec::new();
    for (k, n) in de_baldosa.iter().enumerate() {
        let (f, c) = (k as u32 / col, k as u32 % col);
        baldosas.push((
            *n,
            Rect {
                x: x0 + (c * (ancho_baldosa + e(BALDOSA_HUECO))) as i32,
                y: y + (f * (e(BALDOSA_ALTO) + e(BALDOSA_HUECO))) as i32,
                ancho: ancho_baldosa,
                alto: e(BALDOSA_ALTO),
            },
        ));
    }
    if !de_baldosa.is_empty() {
        let filas = (de_baldosa.len() as u32).div_ceil(col);
        y += (filas * e(BALDOSA_ALTO) + (filas - 1) * e(BALDOSA_HUECO)) as i32;
    }
    let raya = |y: i32| Rect {
        x: x0,
        y,
        ancho: dentro,
        alto: e(1).max(1),
    };
    let mut rayas = Vec::new();
    let de_chip: Vec<usize> = (0..opciones.len())
        .filter(|n| opciones[*n].forma == Forma::Chip)
        .collect();
    let mut cabecera_mas = None;
    let mut chips = Vec::new();
    if !de_chip.is_empty() {
        y += e(HUECO) as i32;
        rayas.push(raya(y));
        y += e(HUECO) as i32;
        let cab = Rect {
            x: x0,
            y,
            ancho: dentro,
            alto: e(CABECERA_MAS),
        };
        cabecera_mas = Some(cab);
        y = cab.abajo() + e(CHIP_HUECO) as i32;
        let cc = COLUMNAS_CHIP;
        let ancho_chip = dentro.saturating_sub((cc - 1) * e(CHIP_HUECO)) / cc;
        for (k, n) in de_chip.iter().enumerate() {
            let (f, c) = (k as u32 / cc, k as u32 % cc);
            chips.push((
                *n,
                Rect {
                    x: x0 + (c * (ancho_chip + e(CHIP_HUECO))) as i32,
                    y: y + (f * (e(CHIP_ALTO) + e(CHIP_HUECO))) as i32,
                    ancho: ancho_chip,
                    alto: e(CHIP_ALTO),
                },
            ));
        }
        let filas = (de_chip.len() as u32).div_ceil(cc);
        y += (filas * e(CHIP_ALTO) + (filas - 1) * e(CHIP_HUECO)) as i32;
    }
    let mut pie = None;
    let mut pegar = None;
    if let Some(n) = (0..opciones.len()).find(|n| opciones[*n].forma == Forma::Pegar) {
        y += e(HUECO) as i32;
        rayas.push(raya(y));
        y += e(HUECO) as i32;
        let fila = Rect {
            x: x0,
            y,
            ancho: dentro,
            alto: e(PIE),
        };
        let boton = e(108).min(dentro / 2);
        pegar = Some((
            n,
            Rect {
                x: fila.derecha() - boton as i32,
                ancho: boton,
                ..fila
            },
        ));
        pie = Some(fila);
        y = fila.abajo();
    }
    let alto = (y + e(RELLENO) as i32).max(0) as u32;
    // A su sitio: la base encima del boton, el borde izquierdo con el suyo.
    let limite_x = marco.ancho as i32 - ancho as i32 - e(8) as i32;
    let x = (ancla.x - e(48) as i32).min(limite_x).max(e(8) as i32);
    let y = (ancla.y - e(SOBRE_EL_BOTON) as i32 - alto as i32).max(e(8) as i32);
    let mover = |r: Rect| Rect {
        x: r.x + x,
        y: r.y + y,
        ..r
    };
    Puesta {
        caja: Rect { x, y, ancho, alto },
        cerrar: mover(cerrar),
        titulo: mover(titulo),
        baldosas: baldosas.into_iter().map(|(n, r)| (n, mover(r))).collect(),
        rayas: rayas.into_iter().map(mover).collect(),
        cabecera_mas: cabecera_mas.map(mover),
        chips: chips.into_iter().map(|(n, r)| (n, mover(r))).collect(),
        pie: pie.map(mover),
        pegar: pegar.map(|(n, r)| (n, mover(r))),
    }
}

/// Si el tema es el de dia (la hoja va blanca).
fn es_claro(c: Color) -> bool {
    c.r + c.g + c.b > 1.5
}

/// Pinta la hoja abierta.
pub(super) fn pintar(p: &Pintor, c: &Pinta, h: &Hoja, ancla: Punto, marco: Rect) {
    let (tema, escala) = (c.tema, c.escala);
    let e = escala as f32 / 100.0;
    let puesta = h.colocar(ancla, marco, escala);
    let claro = es_claro(tema.cabecera);
    // Lo resaltado: con el raton o con las flechas.
    let estado = |n: usize| -> (bool, bool) { (h.sobre == Some(n), h.elegido == Some(n)) };

    let caja = rf(puesta.caja);
    let radio = RADIO as f32 * e;
    p.rellenar_redondeado(
        RectF {
            y: caja.y + 8.0 * e,
            ..encoger(caja, -2.0 * e)
        },
        radio + 2.0,
        con_alfa(hex(0x000000), if claro { 0.16 } else { 0.30 }),
    );
    p.rellenar_redondeado(caja, radio, tema.pildora_borde);
    p.rellenar_redondeado(encoger(caja, 1.0), radio - 1.0, tema.cabecera);

    // Lo gris de las pastillas: el de los botones de una hoja del iPhone.
    let gris = if claro {
        hex(0xf2f2f7)
    } else {
        con_alfa(tema.texto, 0.08)
    };
    let gris_sobre = if claro {
        hex(0xe5e5ea)
    } else {
        con_alfa(tema.texto, 0.15)
    };
    let tam = TAM * e;

    // La cabecera: «Cerrar» en su pastilla y el titulo en negrita en medio.
    let (cw, ch) = p.medir_texto(&h.cerrar, tam);
    let cerrar = RectF {
        ancho: (cw + 28.0 * e).min(puesta.cerrar.ancho as f32),
        ..rf(puesta.cerrar)
    };
    p.rellenar_redondeado(
        cerrar,
        cerrar.alto / 2.0,
        if h.sobre_cerrar { gris_sobre } else { gris },
    );
    p.texto_linea(
        &h.cerrar,
        cerrar.x + (cerrar.ancho - cw).max(0.0) / 2.0,
        cerrar.y + (cerrar.alto - ch) / 2.0,
        tam,
        cerrar.ancho,
        tema.texto,
    );
    let tam_t = 16.0 * e;
    let t = rf(puesta.titulo);
    let ancho_t = t.ancho - 2.0 * (cerrar.ancho + 8.0 * e);
    let (tw, th) = crate::lecciones::ui::medir_negrita(p, &h.titulo, tam_t, ancho_t.max(1.0));
    crate::lecciones::ui::negrita(
        p,
        &h.titulo,
        t.x + (t.ancho - tw) / 2.0,
        t.y + (t.alto - th) / 2.0,
        tam_t,
        ancho_t.max(1.0),
        tema.texto,
    );

    // Las baldosas: el cuadrado de color con su icono y el nombre debajo.
    for (n, r) in &puesta.baldosas {
        let o = &h.opciones[*n];
        let (sobre, elegida) = estado(*n);
        if elegida {
            p.rellenar_redondeado(rf(*r), 14.0 * e, con_alfa(super::AZUL_ELEGIDO, 0.22));
        } else if sobre {
            p.rellenar_redondeado(rf(*r), 14.0 * e, con_alfa(tema.texto, 0.06));
        }
        let lado = ICONO_CAJA as f32 * e;
        let cuadro = RectF {
            x: r.x as f32 + (r.ancho as f32 - lado) / 2.0,
            y: r.y as f32 + 6.0 * e,
            ancho: lado,
            alto: lado,
        };
        p.rellenar_redondeado(cuadro, 14.0 * e, hex(o.color));
        let ic = 28.0 * e;
        p.icono(
            o.icono,
            RectF {
                x: cuadro.x + (lado - ic) / 2.0,
                y: cuadro.y + (lado - ic) / 2.0,
                ancho: ic,
                alto: ic,
            },
            hex(0xffffff),
        );
        let tam_b = 12.0 * e;
        let max = r.ancho as f32 - 4.0 * e;
        let (w, _) = p.medir_texto(&o.rotulo, tam_b);
        let w = w.min(max);
        p.texto_linea(
            &o.rotulo,
            r.x as f32 + (r.ancho as f32 - w) / 2.0,
            cuadro.y + lado + 7.0 * e,
            tam_b,
            max,
            tema.texto,
        );
    }

    for r in &puesta.rayas {
        p.rellenar(
            rf(*r),
            con_alfa(tema.texto, if claro { 0.10 } else { 0.14 }),
        );
    }

    // «Más» en gris y las dos columnas de pastillas.
    if let Some(cab) = puesta.cabecera_mas {
        let tam_m = 12.0 * e;
        let (_, alto) = p.medir_texto(&h.titulo_mas, tam_m);
        p.texto_linea(
            &h.titulo_mas,
            cab.x as f32 + 4.0 * e,
            cab.y as f32 + (cab.alto as f32 - alto) / 2.0,
            tam_m,
            cab.ancho as f32,
            tema.apagado,
        );
    }
    for (n, r) in &puesta.chips {
        let o = &h.opciones[*n];
        let (sobre, elegida) = estado(*n);
        let caja = rf(*r);
        let radio = caja.alto / 2.0;
        if elegida {
            p.rellenar_redondeado(
                encoger(caja, -2.0 * e),
                radio + 2.0 * e,
                super::AZUL_ELEGIDO,
            );
        }
        p.rellenar_redondeado(caja, radio, if sobre { gris_sobre } else { gris });
        let ic = 18.0 * e;
        p.icono(
            o.icono,
            RectF {
                x: caja.x + 14.0 * e,
                y: caja.y + (caja.alto - ic) / 2.0,
                ancho: ic,
                alto: ic,
            },
            tema.texto,
        );
        let (_, alto) = p.medir_texto(&o.rotulo, tam);
        let x = caja.x + 14.0 * e + ic + 10.0 * e;
        p.texto_linea(
            &o.rotulo,
            x,
            caja.y + (caja.alto - alto) / 2.0,
            tam,
            (caja.x + caja.ancho - x - 10.0 * e).max(0.0),
            tema.texto,
        );
    }

    // Abajo: «Pegar lo copiado» y su boton azul claro.
    if let (Some(fila), Some((n, r))) = (puesta.pie, puesta.pegar) {
        let o = &h.opciones[n];
        let (_, alto) = p.medir_texto(&h.pegar_copiado, tam);
        p.texto_linea(
            &h.pegar_copiado,
            fila.x as f32 + 4.0 * e,
            fila.y as f32 + (fila.alto as f32 - alto) / 2.0,
            tam,
            (r.x - fila.x) as f32 - 12.0 * e,
            tema.apagado,
        );
        let (sobre, elegida) = estado(n);
        let azul = hex(0x0a84ff);
        let boton = rf(r);
        let radio = boton.alto / 2.0;
        if elegida {
            p.rellenar_redondeado(
                encoger(boton, -2.0 * e),
                radio + 2.0 * e,
                super::AZUL_ELEGIDO,
            );
        }
        let (fondo, tinta) = if !h.hay_copiado {
            (gris, con_alfa(tema.texto, 0.35))
        } else if sobre {
            (con_alfa(azul, if claro { 0.22 } else { 0.32 }), azul)
        } else {
            (con_alfa(azul, if claro { 0.14 } else { 0.22 }), azul)
        };
        p.rellenar_redondeado(boton, radio, fondo);
        let ic = 17.0 * e;
        let (tw, th) = p.medir_texto(&o.rotulo, tam);
        let total = ic + 6.0 * e + tw;
        let x = boton.x + (boton.ancho - total).max(0.0) / 2.0;
        p.icono(
            o.icono,
            RectF {
                x,
                y: boton.y + (boton.alto - ic) / 2.0,
                ancho: ic,
                alto: ic,
            },
            tinta,
        );
        p.texto_linea(
            &o.rotulo,
            x + ic + 6.0 * e,
            boton.y + (boton.alto - th) / 2.0,
            tam,
            boton.ancho - ic - 12.0 * e,
            tinta,
        );
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pindesk_store::Idioma;

    fn hoja() -> Hoja {
        Hoja::nueva(&Catalogo::nuevo(Idioma::Espanol)).con_copiado(true)
    }

    fn marco() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1024,
            alto: 900,
        }
    }

    const ANCLA: Punto = Punto { x: 300, y: 860 };

    #[test]
    fn siete_baldosas_de_cuatro_en_cuatro_y_los_chips_en_dos_columnas() {
        let h = hoja();
        let numeros: Vec<u8> = h
            .opciones
            .iter()
            .filter_map(|o| match o.forma {
                Forma::Baldosa(n) => Some(n),
                _ => None,
            })
            .collect();
        assert_eq!(numeros, [1, 2, 3, 4, 5, 6, 7]);
        let p = h.colocar(ANCLA, marco(), 100);
        assert_eq!(p.baldosas.len(), 7);
        // Dos filas: cuatro y tres.
        assert_eq!(p.baldosas[0].1.y, p.baldosas[3].1.y);
        assert!(p.baldosas[4].1.y > p.baldosas[0].1.y);
        // Los chips de dos en dos: misma fila, columnas distintas.
        assert_eq!(p.chips[0].1.y, p.chips[1].1.y);
        assert!(p.chips[1].1.x > p.chips[0].1.x);
        assert!(p.chips[2].1.y > p.chips[0].1.y);
        assert_eq!(p.chips[2].1.x, p.chips[0].1.x);
        // Todo debajo de las baldosas y el pie lo ultimo, dentro de la caja.
        let ultima = p.baldosas[6].1.abajo();
        assert!(p.chips.iter().all(|(_, r)| r.y > ultima));
        let pie = p.pie.expect("la fila de pegar");
        assert!(p.chips.iter().all(|(_, r)| r.abajo() < pie.y));
        assert!(pie.abajo() <= p.caja.abajo());
        // La hoja apoya su base encima del boton.
        assert!(p.caja.abajo() <= ANCLA.y);
        // Caso negativo: nada se sale de la ventana.
        assert!(p.caja.x >= 0 && p.caja.derecha() <= 1024 && p.caja.y >= 0);
    }

    #[test]
    fn cada_entrada_hace_lo_del_clip_de_antes() {
        let h = hoja();
        let acciones: Vec<&Accion> = h.opciones.iter().map(|o| &o.accion).collect();
        for a in [
            Accion::AdjImagen,
            Accion::AdjArchivo,
            Accion::AdjLienzo,
            Accion::AdjNotaMd,
            Accion::AdjTabla,
            Accion::AdjCaptura,
            Accion::AdjPagina,
            Accion::AdjProyectos,
            Accion::AdjFigura,
            Accion::AdjMusica,
            Accion::Conversacion,
            Accion::Telepronter,
            Accion::Pronunciar,
            Accion::AdjDelMovil,
            Accion::AdjCarpeta,
            Accion::PegarLoCopiado,
        ] {
            assert!(acciones.contains(&&a), "falta {a:?}");
        }
        // Las siete mini-apps del movil, cada una.
        for cual in pindesk_proyecto::mini::TODAS {
            assert!(acciones.contains(&&Accion::AdjMini(cual)), "falta {cual}");
        }
        // Caso negativo: ninguna repetida.
        for (n, a) in acciones.iter().enumerate() {
            assert!(!acciones[..n].contains(a), "{a:?} dos veces");
        }
    }

    #[test]
    fn sin_buscador_las_letras_no_hacen_nada() {
        let mut h = hoja();
        for c in "cronometro".chars() {
            assert_eq!(h.caracter(c), Respuesta::Sigue);
        }
        // Todas siguen a la vista.
        assert_eq!(h.elegibles().len(), h.opciones.len());
        // Sin nada elegido, Intro no hace nada; Esc cierra.
        assert_eq!(h.tecla(super::super::VK_ENTRAR), Respuesta::Sigue);
        assert_eq!(h.tecla(super::super::VK_ESCAPE), Respuesta::Fuera);
    }

    #[test]
    fn los_numeros_eligen_su_baldosa() {
        let mut h = hoja();
        assert_eq!(h.caracter('1'), Respuesta::Hacer(Accion::AdjImagen));
        assert_eq!(h.caracter('6'), Respuesta::Hacer(Accion::AdjTabla));
        assert_eq!(h.caracter('7'), Respuesta::Hacer(Accion::AdjCaptura));
        // Casos negativos: el 8, el 9 y el 0 no son de nadie.
        assert_eq!(h.caracter('8'), Respuesta::Sigue);
        assert_eq!(h.caracter('0'), Respuesta::Sigue);
    }

    #[test]
    fn un_clic_en_una_baldosa_la_hace_cerrar_cierra_y_fuera_tambien() {
        let mut h = hoja();
        let p = h.colocar(ANCLA, marco(), 100);
        let (n, r) = p.baldosas[2];
        let dentro = Punto {
            x: r.x + 5,
            y: r.y + 5,
        };
        assert_eq!(
            h.pulsar(dentro, ANCLA, marco(), 100),
            Respuesta::Hacer(h.opciones[n].accion.clone())
        );
        let (n, r) = p.chips[3];
        assert_eq!(
            h.pulsar(
                Punto {
                    x: r.x + 4,
                    y: r.y + 4
                },
                ANCLA,
                marco(),
                100
            ),
            Respuesta::Hacer(h.opciones[n].accion.clone())
        );
        let cerrar = Punto {
            x: p.cerrar.x + 4,
            y: p.cerrar.y + 4,
        };
        assert_eq!(h.pulsar(cerrar, ANCLA, marco(), 100), Respuesta::Fuera);
        // Caso negativo: el relleno no hace nada y fuera cierra.
        let relleno = Punto {
            x: p.caja.x + 3,
            y: p.caja.y + 3,
        };
        assert_eq!(h.pulsar(relleno, ANCLA, marco(), 100), Respuesta::Sigue);
        assert_eq!(
            h.pulsar(Punto { x: 2, y: 890 }, ANCLA, marco(), 100),
            Respuesta::Fuera
        );
    }

    #[test]
    fn pegar_apagado_sin_nada_copiado() {
        let catalogo = Catalogo::nuevo(Idioma::Espanol);
        let mut h = Hoja::nueva(&catalogo);
        let p = h.colocar(ANCLA, marco(), 100);
        let (n, r) = p.pegar.expect("el boton de pegar");
        let en_pegar = Punto {
            x: r.x + 4,
            y: r.y + 4,
        };
        // Sin nada copiado: se ve, pero ni se elige ni se pulsa.
        assert!(!h.elegibles().contains(&n));
        assert_eq!(h.pulsar(en_pegar, ANCLA, marco(), 100), Respuesta::Sigue);
        let mut h = h.con_copiado(true);
        assert_eq!(
            h.pulsar(en_pegar, ANCLA, marco(), 100),
            Respuesta::Hacer(Accion::PegarLoCopiado)
        );
    }

    #[test]
    fn las_flechas_bajan_de_las_baldosas_a_los_chips_y_al_pie() {
        use super::super::{VK_ABAJO, VK_ARRIBA, VK_DERECHA, VK_IZQUIERDA};
        let mut h = hoja();
        // Sin nada elegido, la primera.
        h.tecla(VK_DERECHA);
        assert_eq!(h.elegido, Some(0));
        h.tecla(VK_DERECHA);
        assert_eq!(h.elegido, Some(1));
        // Abajo: la de debajo en la cuadricula (la sexta, Tabla).
        h.tecla(VK_ABAJO);
        assert_eq!(h.opciones[h.elegido.unwrap()].accion, Accion::AdjTabla);
        // Otra vez abajo: a los chips, al de la izquierda (lo mas cerca).
        h.tecla(VK_ABAJO);
        let k = h.elegido.unwrap();
        assert_eq!(h.opciones[k].forma, Forma::Chip);
        assert_eq!(h.opciones[k].accion, Accion::AdjProyectos);
        h.tecla(VK_DERECHA);
        h.tecla(VK_ABAJO);
        assert_eq!(h.opciones[h.elegido.unwrap()].accion, Accion::AdjCarpeta);
        // Muy abajo se llega a «Pegar» y de ahi no se pasa.
        for _ in 0..20 {
            h.tecla(VK_ABAJO);
        }
        assert_eq!(
            h.opciones[h.elegido.unwrap()].accion,
            Accion::PegarLoCopiado
        );
        assert_eq!(
            h.tecla(super::super::VK_ENTRAR),
            Respuesta::Hacer(Accion::PegarLoCopiado)
        );
        // Y de vuelta arriba hasta la primera fila.
        for _ in 0..20 {
            h.tecla(VK_ARRIBA);
        }
        let arriba = h.colocar(ANCLA, marco(), 100).baldosas[0].1.y;
        let ahora = h.colocar(ANCLA, marco(), 100);
        assert_eq!(ahora.sitio_de(h.elegido.unwrap()).unwrap().y, arriba);
        // Casos negativos: a la izquierda de la primera no hay nada.
        h.elegido = Some(0);
        h.tecla(VK_IZQUIERDA);
        assert_eq!(h.elegido, Some(0));
        assert_eq!(mover_eleccion(Some(0), VK_DERECHA, &[]), None);
    }

    #[test]
    fn la_del_asistente_empieza_por_figura_y_tambien_pega() {
        let h = Hoja::para_el_asistente(&Catalogo::nuevo(Idioma::Espanol));
        assert_eq!(h.opciones[0].accion, Accion::AdjFigura);
        assert_eq!(h.opciones[0].forma, Forma::Baldosa(1));
        assert!(
            h.opciones
                .iter()
                .any(|o| o.accion == Accion::PedirFigura && o.forma == Forma::Chip)
        );
        assert!(h.opciones.iter().any(|o| o.forma == Forma::Pegar));
        // Caso negativo: Figura no sale dos veces.
        assert_eq!(
            h.opciones
                .iter()
                .filter(|o| o.accion == Accion::AdjFigura)
                .count(),
            1
        );
    }
}
