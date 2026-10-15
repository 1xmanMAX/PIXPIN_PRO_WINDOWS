//! **Lo que piensa el asistente, encima de su respuesta** (pedido del
//! usuario, 13-oct-2026: «que pueda ver lo que esta pensando como en
//! Claude»).
//!
//! Encima del texto de una burbuja del asistente, hasta tres renglones:
//!
//! - **«Pensando…»** mientras piensa, abierto, con lo que va pensando
//!   escribiendose en vivo (solo el final: lo ultimo es lo que se esta
//!   leyendo, y una burbuja que crece sin tope empuja todo el chat). Al
//!   acabar se pliega solo y dice **«Penso 12 s»**; un clic lo abre entero.
//! - **La peticion mejorada** («📝 Entiendo: …»): plegada en una linea, que
//!   ya dice lo esencial; un clic ensena las dos a cuatro lineas.
//! - **«Contexto renovado»** si ese turno empezo con la conversacion
//!   reiniciada (la hoja de trabajo hizo de memoria).
//!
//! Lo que se pinta sale de `asistente::pensado` (en vivo o del registro
//! guardado con la nota). Plegado o abierto se apunta en `a.desplegados`,
//! como el texto de las notas de voz.

use super::*;
use crate::asistente::pensado::{self as datos, Seccion};

/// La letra de la cabecera y del pensamiento.
const CABECERA_TAM: f32 = 12.5;
const PENSADO_TAM: f32 = 12.0;
/// Lo que se ensena del pensamiento mientras llega (el final), y abierto
/// despues (todo, hasta este tope).
const EN_VIVO_LETRAS: usize = 700;
const ABIERTO_LETRAS: usize = 6000;
/// La sangria del pensamiento, con su raya a la izquierda.
const SANGRIA: f32 = 10.0;
const HUECO: f32 = 4.0;

/// Las claves de `a.desplegados` (comparte el conjunto con la voz).
fn clave_pensado(id: &str) -> String {
    format!("ia-pensado:{id}")
}
fn clave_peticion(id: &str) -> String {
    format!("ia-peticion:{id}")
}

/// Un clic en la cabecera (`pensado`) o en la peticion: pliega o despliega.
pub(super) fn alternar(
    desplegados: &mut std::collections::BTreeSet<String>,
    id: &str,
    pensado: bool,
) {
    let k = if pensado {
        clave_pensado(id)
    } else {
        clave_peticion(id)
    };
    if !desplegados.remove(&k) {
        desplegados.insert(k);
    }
}

/// La pieza ya medida.
pub(super) struct Pieza {
    /// La cabecera («Pensando…», «Penso 12 s») y si va abierta.
    cabecera: Option<(String, bool)>,
    alto_cabecera: f32,
    /// Lo pensado que se ve, y su alto.
    pensado: Option<(String, f32)>,
    /// La peticion: texto, alto, abierta.
    peticion: Option<(String, f32, bool)>,
    /// «Contexto renovado» y su alto.
    reinicio: Option<(String, f32)>,
    pub ancho: f32,
    pub alto: f32,
}

/// La cabecera: lo que dice.
fn rotulo(s: &Seccion, textos: &Catalogo) -> String {
    if s.pensando || s.segundos.is_none() {
        return textos.t("asistente-pensando");
    }
    let seg = s.segundos.unwrap_or(0.0);
    if seg < 1.0 {
        return textos.t("asistente-penso-poco");
    }
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("s", seg.round() as i64);
    textos.t_args("asistente-penso", &args)
}

/// Lo que se ensena del pensamiento: el final mientras llega, todo despues.
fn texto_del_pensamiento(s: &Seccion) -> String {
    let t = s.pensamiento.trim();
    let n = t.chars().count();
    if s.pensando && n > EN_VIVO_LETRAS {
        let cola: String = t.chars().skip(n - EN_VIVO_LETRAS).collect();
        format!("…{}", cola.trim_start())
    } else if n > ABIERTO_LETRAS {
        format!("{}…", t.chars().take(ABIERTO_LETRAS).collect::<String>())
    } else {
        t.to_string()
    }
}

/// **Mide** la pieza de una nota, o `None` si no es del asistente o no hay
/// nada que ensenar (un modelo que no penso y sin peticion mejorada).
pub(super) fn medir(
    p: &Pintor,
    a: &Abierto,
    m: &pixpin_proyecto::cuaderno::Mensaje,
    ancho_max: f32,
    textos: &Catalogo,
    e: f32,
) -> Option<Pieza> {
    let s = datos::seccion_de(&a.raiz, &a.ficha.id, m)?;
    medir_seccion(
        p,
        &s,
        a.desplegados.contains(&clave_pensado(&m.id)),
        a.desplegados.contains(&clave_peticion(&m.id)),
        ancho_max,
        textos,
        e,
    )
}

/// Lo mismo a partir de la seccion (para las pruebas).
pub(super) fn medir_seccion(
    p: &Pintor,
    s: &Seccion,
    pensado_abierto: bool,
    peticion_abierta: bool,
    ancho_max: f32,
    textos: &Catalogo,
    e: f32,
) -> Option<Pieza> {
    if !s.algo() {
        return None;
    }
    let (mut ancho, mut alto) = (0.0f32, 0.0f32);
    let mut sumar = |w: f32, h: f32| {
        ancho = ancho.max(w.min(ancho_max));
        if alto > 0.0 {
            alto += HUECO * e;
        }
        alto += h;
    };
    let (mut cabecera, mut alto_cabecera, mut pensado) = (None, 0.0, None);
    if s.penso() || s.pensando {
        // Mientras piensa va abierto; al acabar, plegado salvo que se abra.
        let abierto = s.pensando || pensado_abierto;
        let r = rotulo(s, textos);
        let (w, h) = p.medir_texto(&r, CABECERA_TAM * e);
        sumar(w + 18.0 * e, h);
        alto_cabecera = h;
        cabecera = Some((r, abierto));
        if abierto {
            let t = texto_del_pensamiento(s);
            if !t.is_empty() {
                let dentro = (ancho_max - SANGRIA * e).max(1.0);
                let (w, h) = p.medir_texto_ajustado(&t, PENSADO_TAM * e, dentro);
                sumar(w + SANGRIA * e, h);
                pensado = Some((t, h));
            }
        }
    }
    let peticion = s
        .plan
        .as_ref()
        .filter(|pl| !pl.entiendo.is_empty())
        .map(|pl| {
            if peticion_abierta {
                let t = format!("📝 {}", pl.lineas());
                let (w, h) = p.medir_texto_ajustado(&t, PENSADO_TAM * e, ancho_max);
                sumar(w, h);
                (t, h, true)
            } else {
                let t = format!("📝 {}", pl.entiendo);
                let (w, h) = p.medir_texto(&t, PENSADO_TAM * e);
                sumar(w, h);
                (t, h, false)
            }
        });
    let reinicio = s.reinicio.then(|| {
        let t = format!("🔄 {}", textos.t("asistente-reiniciado"));
        let (w, h) = p.medir_texto(&t, PENSADO_TAM * e);
        sumar(w, h);
        (t, h)
    });
    Some(Pieza {
        cabecera,
        alto_cabecera,
        pensado,
        peticion,
        reinicio,
        ancho,
        alto,
    })
}

/// **Pinta** la pieza en `x, y` con `ancho` de sitio. Devuelve las zonas
/// que se pueden pulsar: la cabecera y la peticion.
#[allow(clippy::too_many_arguments)] // donde, de que color y a que escala
pub(super) fn pintar(
    p: &Pintor,
    pieza: &Pieza,
    x: f32,
    y: f32,
    ancho: f32,
    tinta: Color,
    apagado: Color,
    e: f32,
) -> (Option<Rect>, Option<Rect>) {
    let mut y = y;
    let mut primero = true;
    let mut aire = |y: &mut f32| {
        if !primero {
            *y += HUECO * e;
        }
        primero = false;
    };
    let caja = |x: f32, y: f32, w: f32, h: f32| Rect {
        x: x.floor() as i32,
        y: y.floor() as i32,
        ancho: w.max(1.0).ceil() as u32,
        alto: h.max(1.0).ceil() as u32,
    };
    let (mut zona_pensado, mut zona_peticion) = (None, None);
    if let Some((r, abierto)) = &pieza.cabecera {
        aire(&mut y);
        let lado = 14.0 * e;
        pintar_flechita(
            p,
            RectF {
                x,
                y: y + (pieza.alto_cabecera - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            *abierto,
            apagado,
            e,
        );
        p.texto_linea(
            r,
            x + 18.0 * e,
            y,
            CABECERA_TAM * e,
            (ancho - 18.0 * e).max(1.0),
            apagado,
        );
        zona_pensado = Some(caja(x, y, ancho, pieza.alto_cabecera));
        y += pieza.alto_cabecera;
        if let Some((t, h)) = &pieza.pensado {
            y += HUECO * e;
            p.rellenar_redondeado(
                RectF {
                    x: x + 2.0 * e,
                    y,
                    ancho: 2.0 * e,
                    alto: *h,
                },
                1.0 * e,
                con_alfa(apagado, 0.6),
            );
            p.texto_ajustado(
                t,
                x + SANGRIA * e,
                y,
                PENSADO_TAM * e,
                (ancho - SANGRIA * e).max(1.0),
                con_alfa(tinta, 0.72),
            );
            y += h;
        }
    }
    if let Some((t, h, abierta)) = &pieza.peticion {
        aire(&mut y);
        if *abierta {
            p.texto_ajustado(t, x, y, PENSADO_TAM * e, ancho, apagado);
        } else {
            p.texto_linea(t, x, y, PENSADO_TAM * e, ancho, apagado);
        }
        zona_peticion = Some(caja(x, y, ancho, *h));
        y += h;
    }
    if let Some((t, h)) = &pieza.reinicio {
        aire(&mut y);
        p.texto_linea(t, x, y, PENSADO_TAM * e, ancho, apagado);
        y += h;
    }
    let _ = y;
    (zona_pensado, zona_peticion)
}
