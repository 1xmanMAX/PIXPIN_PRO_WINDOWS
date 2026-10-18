//! **El pin de una tarea** (17-oct-2026): una tarea sola, fijada en la
//! pantalla como recordatorio, que solo ensena su titulo.
//!
//! El usuario: «que directamente se ponga un pin de esa tarea solo con la
//! opcion de poner notas de por que no se esta haciendo en ese momento […]
//! una forma de hacer recordar que tiene que hacer tareas sin necesidad de
//! mostrar todas las tareas, solo el titulo, de forma facil y sencilla».
//!
//! **No es otra ventana**: es una herramienta pineada mas
//! (`herramienta::Herramienta` con `cual = TAREA`), el mismo motor que la
//! lista de tareas sacada del chat, con su vinculo al mensaje. Lo unico
//! propio esta aqui: como se pinta (titulo, prioridad, hora, etiquetas y la
//! nota) y el boton de la nota con su caja de una linea. La nota se guarda
//! DENTRO de la tarea (`tareas::frase::con_nota`, tras `💬`), asi que viaja
//! al movil y se ve en la ventana de Tareas.
//!
//! La tarea se reconoce por su **titulo** y no por su numero: el chat o el
//! movil pueden reordenar la lista mientras el pin esta en la pantalla.
//!
//! Android no tiene este pin (su widget de tareas ensena la lista entera);
//! el formato de la nota es texto, asi que alli se lee tal cual.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use pixpin_geom::{Punto, Rect};
use pixpin_render::{Color, Pintor, RectF};
use pixpin_store::Catalogo;

use super::herramienta::{Herramienta, Tema, Vinculo};
use crate::tareas::frase::{self, Urgencia};

/// El `cual` de la herramienta (y su origen en el almacen: `mini:tarea@…`).
pub const TAREA: &str = "tarea";

/// La extension de la ruta de mentira con la que se pide el pin, como la de
/// las listas (`herramienta::EXTENSION_DE_LISTA`).
pub const EXTENSION: &str = "pixpin-tarea";

/// El tamano con que nace, en pixeles logicos: pequeño, lo justo para dos
/// renglones de titulo, la linea de la hora y la nota.
const ANCHO: u32 = 280;
const ALTO: u32 = 112;
const MARGEN: f32 = 10.0;
const BOTON_ALTO: f32 = 26.0;

/// La ruta con la que se pide el pin de la tarea `indice` de la lista `id`.
pub fn ruta_de_tarea(carpeta: &Path, id: &str, indice: usize) -> PathBuf {
    carpeta.join(format!("{id}.{indice}.{EXTENSION}"))
}

/// La lista y el numero que pide una ruta de [`ruta_de_tarea`].
pub fn tarea_de_ruta(ruta: &Path) -> Option<(Vinculo, usize)> {
    if !ruta
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
    {
        return None;
    }
    let (mensaje, n) = ruta.file_stem()?.to_str()?.rsplit_once('.')?;
    let indice = n.parse().ok()?;
    (!mensaje.is_empty()).then(|| {
        (
            Vinculo {
                carpeta: ruta.parent().map(Path::to_path_buf).unwrap_or_default(),
                mensaje: mensaje.to_string(),
            },
            indice,
        )
    })
}

/// El titulo por el que se reconoce la tarea de un pin.
pub fn titulo_de(h: &Herramienta) -> String {
    frase::marcas(&h.documento).titulo
}

/// El tamano con que nace, en pixeles fisicos.
pub fn tamano_natural(escala: u32) -> (u32, u32) {
    (ANCHO * escala / 100, ALTO * escala / 100)
}

/// El boton de la nota (y, escribiendo, el de guardar): abajo a lo ancho.
pub fn boton(tam: (u32, u32), escala: u32) -> Rect {
    let e = escala as f32 / 100.0;
    let m = (MARGEN * e) as i32;
    let alto = (BOTON_ALTO * e) as i32;
    Rect {
        x: m,
        y: tam.1 as i32 - m - alto,
        ancho: (tam.0 as i32 - 2 * m).max(1) as u32,
        alto: alto.max(1) as u32,
    }
}

/// Lo que pide un toque o una tecla en el pin de una tarea.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Respuesta {
    Nada,
    Repintar,
    /// Guardar esta nota en la tarea (vacia: quitarla).
    Guardar(String),
    Cerrar,
}

fn empezar(h: &mut Herramienta) -> Respuesta {
    h.nota = Some(frase::marcas(&h.documento).nota.unwrap_or_default());
    Respuesta::Repintar
}

/// Un toque, en pixeles del contenido.
pub fn clic(h: &mut Herramienta, p: Punto, tam: (u32, u32), escala: u32) -> Respuesta {
    if !boton(tam, escala).contiene(p) {
        return Respuesta::Nada;
    }
    match &h.nota {
        None => empezar(h),
        Some(n) => Respuesta::Guardar(n.clone()),
    }
}

/// Una tecla con el pin enfocado: Intro guarda, Esc deja de escribir (o
/// cierra el pin, como en cualquier pin), Retroceso borra.
pub fn tecla(h: &mut Herramienta, vk: u32) -> Respuesta {
    const INTRO: u32 = 0x0D;
    const ESC: u32 = 0x1B;
    const RETROCESO: u32 = 0x08;
    match (&mut h.nota, vk) {
        (Some(n), INTRO) => Respuesta::Guardar(n.clone()),
        (Some(_), ESC) => {
            h.nota = None;
            Respuesta::Repintar
        }
        (Some(n), RETROCESO) => {
            n.pop();
            Respuesta::Repintar
        }
        (None, ESC) => Respuesta::Cerrar,
        (None, INTRO) => empezar(h),
        _ => Respuesta::Nada,
    }
}

/// Una letra escrita: va a la nota si se esta escribiendo.
pub fn caracter(h: &mut Herramienta, c: char) -> Respuesta {
    match &mut h.nota {
        Some(n) if !c.is_control() => {
            n.push(c);
            Respuesta::Repintar
        }
        _ => Respuesta::Nada,
    }
}

/// El color de la franja de la urgencia: el de su chapita, o el acento si
/// es normal.
fn color_de(u: Urgencia, tema: &Tema) -> Color {
    match u {
        Urgencia::MuyUrgente => crate::tareas::chapitas::ROJO,
        Urgencia::Urgente => crate::tareas::chapitas::AMBAR,
        Urgencia::Normal => tema.acento,
    }
}

/// Pinta el pin: la franja de la urgencia, el titulo, la linea de la hora
/// con las chapitas de la urgencia y las etiquetas, la nota y el boton (o
/// la caja donde se escribe).
pub(super) fn pintar(p: &Pintor, caja: RectF, h: &Herramienta, escala: u32, tema: &Tema, textos: &Catalogo) {
    let e = escala as f32 / 100.0;
    let m = frase::marcas(&h.documento);
    let margen = MARGEN * e;
    p.rellenar(
        RectF {
            ancho: 4.0 * e,
            ..caja
        },
        color_de(m.urgencia, tema),
    );
    let x = caja.x + margen + 4.0 * e;
    let ancho = (caja.ancho - 2.0 * margen - 4.0 * e).max(1.0);
    let b = boton((caja.ancho as u32, caja.alto as u32), escala);
    let boton = RectF {
        x: caja.x + b.x as f32,
        y: caja.y + b.y as f32,
        ancho: b.ancho as f32,
        alto: b.alto as f32,
    };
    let mut y = caja.y + margen;
    // El titulo, hasta donde quepa sobre lo de abajo.
    let tam = 14.5 * e;
    let (_, alto_t) = p.medir_texto_ajustado(&m.titulo, tam, ancho);
    let tope = boton.y - y - 4.0 * e;
    p.con_recorte(
        RectF {
            x,
            y,
            ancho,
            alto: alto_t.min(tope).max(1.0),
        },
        |p| p.texto_ajustado(&m.titulo, x, y, tam, ancho, tema.texto),
    );
    y += alto_t.min(tope) + 3.0 * e;
    // La hora y, detras, las chapitas (urgencia y etiquetas) en una linea;
    // la nota en otra.
    let vista = frase::a_la_vista(&h.documento, None);
    let chico = 11.5 * e;
    let alto_linea = if vista.chapas.is_empty() {
        chico * 1.4
    } else {
        crate::tareas::chapitas::ALTO * e + 2.0 * e
    };
    if (m.hora.is_some() || !vista.chapas.is_empty()) && y + alto_linea <= boton.y {
        let mut cx = x;
        if let Some(t) = m.hora {
            let hora = format!("⏰ {}", t.legible(frase::ahora()));
            let (hw, hh) = p.medir_texto(&hora, chico);
            p.texto_linea(&hora, x, y + (alto_linea - hh) / 2.0, chico, ancho, tema.apagado);
            cx += hw + 8.0 * e;
        }
        let alto_c = crate::tareas::chapitas::ALTO * e;
        crate::tareas::chapitas::pintar(
            p,
            &vista.chapas,
            cx,
            y + (alto_linea - alto_c) / 2.0,
            (x + ancho - cx).max(0.0),
            // (El pin en tema claro: el papel es claro.)
            if tema.papel.r + tema.papel.g + tema.papel.b > 1.5 {
                crate::tareas::chapitas::Sobre::Claro
            } else {
                crate::tareas::chapitas::Sobre::Oscuro
            },
            e,
            textos,
        );
        y += alto_linea;
    }
    if h.nota.is_none()
        && let Some(n) = &m.nota
        && y + chico * 1.3 <= boton.y
    {
        p.texto_linea(&format!("{} {n}", frase::NOTA), x, y, chico, ancho, tema.apagado);
    }
    // Abajo, el boton de la nota; escribiendo, la caja con lo escrito.
    match &h.nota {
        None => {
            p.rellenar_redondeado(boton, boton.alto / 2.0, tema.caja);
            let r = textos.t("pin-tarea-nota");
            let (w, hh) = p.medir_texto(&r, 12.0 * e);
            p.texto(
                &r,
                boton.x + (boton.ancho - w).max(0.0) / 2.0,
                boton.y + (boton.alto - hh) / 2.0,
                12.0 * e,
                tema.acento,
            );
        }
        Some(n) => {
            p.rellenar_redondeado(boton, 6.0 * e, tema.caja);
            p.trazar(boton, 1.0, tema.acento);
            let (texto, color) = if n.is_empty() {
                (textos.t("pin-tarea-pista"), tema.apagado)
            } else {
                (format!("{n}▏"), tema.texto)
            };
            let (_, hh) = p.medir_texto(&texto, 12.0 * e);
            p.texto_linea(
                &texto,
                boton.x + 8.0 * e,
                boton.y + (boton.alto - hh) / 2.0,
                12.0 * e,
                boton.ancho - 16.0 * e,
                color,
            );
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn tarea(texto: &str) -> Herramienta {
        Herramienta::nueva(TAREA, texto.to_string())
    }

    /// `cargo test -p pixpin --bin pixpinmax muestra_del_pin_de_tarea --
    /// --ignored --nocapture`: el pin quieto y escribiendo la nota.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_del_pin_de_tarea() {
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let (w, h) = tamano_natural(100);
        let caja = RectF {
            x: 0.0,
            y: 0.0,
            ancho: w as f32,
            alto: h as f32,
        };
        let crudo = "llamar al banco por la tarjeta ⏫ #casa 💬 no abren hasta las 4 ⏰ 2026-10-17 17:00 ➕ 2026-10-17";
        for (nombre, escribiendo, claro) in [("pin-tarea", None, false), ("pin-tarea-nota", Some("sin saldo"), true)] {
            let mut t = tarea(crudo);
            t.nota = escribiendo.map(str::to_string);
            let cuadro = super::super::herramienta::Cuadro {
                herramienta: t,
                escala: 100,
                claro,
                moneda: super::super::herramienta::moneda(&textos),
            };
            crate::ventanita::muestra(nombre, w, h, |p, _| cuadro.pintar(p, caja, &textos));
        }
    }

    #[test]
    fn la_ruta_dice_la_lista_y_el_numero() {
        let r = ruta_de_tarea(Path::new("C:/p/proyectos/abc"), "m.1-2", 3);
        let (v, n) = tarea_de_ruta(&r).unwrap();
        assert_eq!((v.mensaje.as_str(), n), ("m.1-2", 3));
        assert_eq!(v.carpeta, Path::new("C:/p/proyectos/abc"));
        // Casos negativos: otra extension o sin numero.
        assert!(tarea_de_ruta(Path::new("C:/p/m.3.pixpin-lista")).is_none());
        assert!(tarea_de_ruta(Path::new("C:/p/m.pixpin-tarea")).is_none());
        assert!(tarea_de_ruta(Path::new("C:/p/m.x.pixpin-tarea")).is_none());
    }

    #[test]
    fn el_boton_abre_la_caja_y_intro_guarda_la_nota() {
        let mut h = tarea("llamar al banco ⏫ 💬 cerrado ➕ 2026-10-14");
        let tam = tamano_natural(100);
        // Un toque fuera del boton no hace nada.
        assert_eq!(clic(&mut h, Punto { x: 20, y: 10 }, tam, 100), Respuesta::Nada);
        let b = boton(tam, 100);
        let dentro = Punto {
            x: b.x + 5,
            y: b.y + 5,
        };
        assert_eq!(clic(&mut h, dentro, tam, 100), Respuesta::Repintar);
        assert_eq!(h.nota.as_deref(), Some("cerrado"), "empieza con la que tenia");
        for _ in 0.."cerrado".len() {
            tecla(&mut h, 0x08);
        }
        for c in "sin saldo".chars() {
            caracter(&mut h, c);
        }
        assert_eq!(caracter(&mut h, '\u{7}'), Respuesta::Nada, "un control no se escribe");
        assert_eq!(tecla(&mut h, 0x0D), Respuesta::Guardar("sin saldo".into()));
        assert_eq!(clic(&mut h, dentro, tam, 100), Respuesta::Guardar("sin saldo".into()));
        // Esc deja de escribir; otro Esc cierra el pin.
        assert_eq!(tecla(&mut h, 0x1B), Respuesta::Repintar);
        assert!(h.nota.is_none());
        assert_eq!(tecla(&mut h, 0x1B), Respuesta::Cerrar);
        assert_eq!(titulo_de(&h), "llamar al banco");
    }
}
