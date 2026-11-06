//! **El chat «Asignaturas», agrupado por asignatura** (6-nov-2026, el
//! usuario: «para las asignaturas que se cree un chat en la que se agrupen
//! todas y ahi se vaya almacenando todo de estas, solo 1 para todos»).
//!
//! Es un chat como los demas (viaja, se busca, se comparte); lo que tiene de
//! suyo, todo aqui:
//!
//! - **La fila de las asignaturas** debajo de la cabecera: «Todas» y un chip
//!   por asignatura, con su color del horario (las que ya no estan en el
//!   horario pero aun tienen material, en gris). Pulsar uno deja ver solo
//!   lo de esa asignatura: su lista de tareas, sus clases grabadas y lo que
//!   se le mando; pulsarlo otra vez (o «Todas») lo quita.
//! - **Lo que se pone con un chip elegido es de esa asignatura**: una nota,
//!   un archivo soltado, una clase grabada desde aqui… en la siguiente
//!   vuelta del bucle se le apunta la etiqueta ([`etiquetar_lo_nuevo`]), por
//!   el camino que haya entrado.
//! - **«Asignatura…»** en el menu de un mensaje de este chat: ponerle,
//!   cambiarle o quitarle la asignatura. En los demas chats es **«Enviar a
//!   asignatura…»**: una copia con su etiqueta en este chat (`reenviar`).
//!
//! La etiqueta y sus reglas son de `pindesk_proyecto::asignatura`.

use pindesk_geom::Rect;
use pindesk_proyecto::asignatura::{self, Chip};
use pindesk_proyecto::cuaderno::Mensaje;
use pindesk_render::{Color, Pintor, RectF};
use pindesk_store::Catalogo;
use pindesk_ui::chat::{self, Disposicion};

use super::{Abierto, Tema, Zona, chips_de, hay_barra, hex, rf};

/// Lo del chat abierto que es de sus asignaturas.
#[derive(Debug, Default)]
pub(super) struct Estado {
    /// La asignatura elegida en la fila de arriba, si hay una.
    pub filtro: Option<String>,
    /// Desde cuando (ms UTC): lo de este equipo puesto desde entonces sin
    /// etiqueta es de ella.
    pub desde: i64,
    /// El codigo de este equipo, el de lo que se pone aqui.
    pub yo: String,
    /// Las del horario con su color; vacio en los demas chats.
    pub del_horario: Vec<Chip>,
}

impl Estado {
    /// Al abrir un chat: en el de las asignaturas, las del horario.
    pub fn de(raiz: &std::path::Path, ficha: &pindesk_proyecto::almacen::Ficha) -> Estado {
        Estado {
            del_horario: del_horario(raiz, ficha),
            ..Estado::default()
        }
    }

    /// Al releer el chat: el horario pudo cambiar.
    pub fn releer(&mut self, raiz: &std::path::Path, ficha: &pindesk_proyecto::almacen::Ficha) {
        self.del_horario = del_horario(raiz, ficha);
    }
}

fn del_horario(raiz: &std::path::Path, ficha: &pindesk_proyecto::almacen::Ficha) -> Vec<Chip> {
    if !ficha.es_asignaturas() {
        return Vec::new();
    }
    asignatura::del_horario(&crate::horario::asignaturas::con_su_color(raiz))
}

/// Los chips de la fila: vacio fuera del chat «Asignaturas».
pub(super) fn chips(a: &Abierto) -> Vec<Chip> {
    if !a.ficha.es_asignaturas() {
        return Vec::new();
    }
    asignatura::chips(&a.asignaturas.del_horario, &a.mensajes)
}

/// Donde va la fila de las asignaturas: debajo de la cabecera (y del
/// fijado, y de los chips de etiqueta si se busca). `None` si no toca.
pub(super) fn fila(d: &Disposicion, a: &Abierto, escala: u32) -> Option<Rect> {
    if !a.ficha.es_asignaturas()
        || (a.asignaturas.del_horario.is_empty()
            && !a.mensajes.iter().any(|m| asignatura::de(m).is_some()))
    {
        return None;
    }
    let base = d.historial(a.alto_caja.get(), hay_barra(a), chips_de(a).len(), escala);
    let alto = chat::alto_de_chips(1, escala).min(base.alto);
    (alto > 0).then_some(Rect {
        x: base.x,
        y: base.y,
        ancho: base.ancho,
        alto,
    })
}

/// El historial sin la fila de las asignaturas.
pub(super) fn sin_la_fila(area: Rect, fila: Option<Rect>) -> Rect {
    match fila {
        Some(f) => Rect {
            y: f.abajo(),
            alto: area.alto.saturating_sub(f.alto),
            ..area
        },
        None => area,
    }
}

/// Elige (o quita, con `None`) la asignatura de la fila.
pub(super) fn elegir(a: &mut Abierto, cual: Option<String>, yo: &str) {
    a.asignaturas.filtro = cual;
    a.asignaturas.desde = pindesk_shell::entorno::ahora_utc_ms();
    a.asignaturas.yo = yo.to_string();
    // Arriba del todo no: al final, que es donde esta lo ultimo de ella.
    a.scroll = None;
}

/// El chip pulsado: el mismo otra vez lo quita, como los de etiqueta.
pub(super) fn pulsado(a: &mut Abierto, i: Option<usize>, yo: &str) {
    let cual = i.and_then(|i| chips(a).get(i).map(|c| c.nombre.clone()));
    let cual = match (&a.asignaturas.filtro, cual) {
        (Some(puesta), Some(nueva)) if asignatura::mismo(puesta, &nueva) => None,
        (_, cual) => cual,
    };
    elegir(a, cual, yo);
}

/// Si una burbuja pasa el chip elegido.
pub(super) fn pasa(a: &Abierto, m: &Mensaje) -> bool {
    match a.asignaturas.filtro.as_deref() {
        None => true,
        Some(f) => asignatura::pasa(
            m,
            f,
            &a.asignaturas.del_horario,
            &a.asignaturas.yo,
            a.asignaturas.desde,
        ),
    }
}

/// **Le pone su asignatura a lo nuevo** (con un chip elegido): lo de este
/// equipo puesto desde que se eligio, entrara por donde entrara. Con una
/// mini-app o un lienzo vivo abiertos espera, como `releer_lo_abierto`: lo
/// que se esta escribiendo manda. Devuelve si cambio algo.
pub(super) fn etiquetar_lo_nuevo(a: &mut Abierto) -> bool {
    let Some(nombre) = a.asignaturas.filtro.clone() else {
        return false;
    };
    if !a.ficha.es_asignaturas() || a.mini.is_some() || a.vivo.is_some() {
        return false;
    }
    let cuales = asignatura::por_etiquetar(&a.mensajes, &a.asignaturas.yo, a.asignaturas.desde);
    if cuales.is_empty() {
        return false;
    }
    let carpeta = pindesk_proyecto::almacen::carpeta(&a.raiz, &a.ficha.id);
    for i in cuales {
        let m = &mut a.mensajes[i];
        asignatura::poner(m, &nombre);
        // Una hoja que solo esta en `proyecto.json` no tiene linea en el
        // cuaderno: se queda con la etiqueta en memoria y nada mas.
        if let Err(e) = pindesk_proyecto::cuaderno::reemplazar(&carpeta, m) {
            tracing::warn!(?e, "asignaturas: no se apunto la asignatura de lo nuevo");
        }
    }
    a.colocado.borrow_mut().ancho = 0;
    true
}

/// **Pone (o quita, con `""`) la asignatura** de unos mensajes de este chat,
/// con lo que lleven dentro (una carpeta, sus paginas). Devuelve cuantos
/// cambiaron.
pub(super) fn poner_en(a: &mut Abierto, indices: &[usize], nombre: &str) -> std::io::Result<usize> {
    let carpeta = pindesk_proyecto::almacen::carpeta(&a.raiz, &a.ficha.id);
    let mut n = 0;
    for i in asignatura::paquete(&a.mensajes, indices) {
        let m = &mut a.mensajes[i];
        if asignatura::poner(m, nombre) && pindesk_proyecto::cuaderno::reemplazar(&carpeta, m)? {
            n += 1;
        }
    }
    a.colocado.borrow_mut().ancho = 0;
    Ok(n)
}

/// Las asignaturas que ofrece el menu de un mensaje: en el chat
/// «Asignaturas», sus chips; en otro, las del horario (y la que ya lleve).
pub(super) fn para_el_menu(a: &Abierto, m: &Mensaje) -> Vec<String> {
    let cs = if a.ficha.es_asignaturas() {
        chips(a)
    } else {
        let mut v = asignatura::del_horario(&crate::horario::asignaturas::con_su_color(&a.raiz));
        if let Some(n) = asignatura::de(m)
            && !v.iter().any(|c| asignatura::mismo(&c.nombre, n))
        {
            v.push(Chip {
                nombre: n.to_string(),
                color: None,
            });
        }
        v
    };
    asignatura::para_enviar(&cs, asignatura::de(m))
}

/// El gris de una asignatura que ya no esta en el horario.
const GRIS: u32 = 0x8E8E93;

/// Lo que mide un chip de asignatura (logicos, al 100 %).
const ALTO: f32 = 28.0;
const RELLENO_X: f32 = 12.0;
const PUNTO: f32 = 8.0;
const AIRE_PUNTO: f32 = 6.0;
const AIRE: f32 = 6.0;
const MARGEN_X: f32 = 8.0;
const LETRA: f32 = 13.0;

/// **Pinta la fila de las asignaturas** y apunta donde cae cada chip.
pub(super) fn pintar(p: &Pintor, a: &Abierto, fila: Rect, tema: &Tema, textos: &Catalogo, e: f32) {
    p.rellenar(rf(fila), tema.cabecera);
    p.rellenar(
        RectF {
            x: fila.x as f32,
            y: fila.abajo() as f32 - (1.0 * e).max(1.0),
            ancho: fila.ancho as f32,
            alto: (1.0 * e).max(1.0),
        },
        tema.separador,
    );
    let todas = textos.t("asignaturas-todas");
    let cs = chips(a);
    let mut x = fila.x as f32 + MARGEN_X * e;
    let tope = fila.derecha() as f32 - MARGEN_X * e;
    let alto = (ALTO * e).min(fila.alto as f32);
    let y = fila.y as f32 + (fila.alto as f32 - alto) / 2.0;
    let puesta = a.asignaturas.filtro.as_deref();
    let mut chip = |rotulo: &str, color: Option<Color>, elegido: bool, zona: Zona| -> bool {
        let (w, h) = p.medir_texto(rotulo, LETRA * e);
        let con_punto = color.is_some();
        let ancho = 2.0 * RELLENO_X * e
            + w
            + if con_punto {
                (PUNTO + AIRE_PUNTO) * e
            } else {
                0.0
            };
        if x + ancho > tope {
            return false;
        }
        let caja = RectF { x, y, ancho, alto };
        let fondo = match (elegido, color) {
            (true, Some(c)) => c,
            (true, None) => tema.enviar,
            (false, _) => tema.pildora,
        };
        p.rellenar_redondeado(caja, alto / 2.0, fondo);
        let mut tx = x + RELLENO_X * e;
        if let Some(c) = color {
            let lado = PUNTO * e;
            p.rellenar_redondeado(
                RectF {
                    x: tx,
                    y: y + (alto - lado) / 2.0,
                    ancho: lado,
                    alto: lado,
                },
                lado / 2.0,
                if elegido { Color::BLANCO } else { c },
            );
            tx += (PUNTO + AIRE_PUNTO) * e;
        }
        p.texto(
            rotulo,
            tx,
            y + (alto - h) / 2.0,
            LETRA * e,
            if elegido {
                Color::BLANCO
            } else {
                tema.pildora_texto
            },
        );
        a.zonas.borrow_mut().push((
            Rect {
                x: caja.x as i32,
                y: caja.y as i32,
                ancho: caja.ancho.ceil() as u32,
                alto: caja.alto.ceil() as u32,
            },
            zona,
        ));
        x += ancho + AIRE * e;
        true
    };
    if !chip(&todas, None, puesta.is_none(), Zona::Asignatura(None)) {
        return;
    }
    for (i, c) in cs.iter().enumerate() {
        let color = hex(c.color.unwrap_or(GRIS));
        let elegido = puesta.is_some_and(|f| asignatura::mismo(f, &c.nombre));
        // Los que no caben no se apuntan: no hay donde pulsar uno que no
        // se ve. La fila es una sola, como la de las etiquetas.
        if !chip(&c.nombre, Some(color), elegido, Zona::Asignatura(Some(i))) {
            break;
        }
    }
}
