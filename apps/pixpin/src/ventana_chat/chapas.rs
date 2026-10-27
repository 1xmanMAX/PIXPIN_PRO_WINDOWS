//! **Las chapas de debajo del nombre** de un archivo en su burbuja, las del
//! movil y apagadas, para que digan sin llamar la atencion:
//!
//! - **«Escaneado»** (23-oct-2026, Android c4b7a194): llego del escaner del
//!   movil (`Mensaje.escaneado {app, documento}`, que el PC guarda tal cual
//!   en su `resto`). Solo dice de donde vino: «Editar en el escaner» es del
//!   telefono, que es donde esta el escaner.
//! - **«Lectura veloz · 47 %»** (27-oct-2026, Android da4acbc7): se esta
//!   leyendo en la app «Lectura Veloz» y por donde va alli (lo ultimo que
//!   mando, guardado en este equipo: `crate::lectura_veloz::porcentaje`).
//!   Tocarla sigue leyendo alli, como «⚡ Lectura veloz» del menu.
//!
//! Lo que mide y coloca es de `pixpin_ui::chat::chapas_de_la_burbuja`; aqui,
//! que chapas lleva cada mensaje, sus textos y como se pintan.

use std::path::PathBuf;

use pixpin_geom::Rect;
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_render::icono::{Icono, material as mi};
use pixpin_render::{Color, Pintor, RectF};
use pixpin_store::Catalogo;
use pixpin_ui::chat;

/// Que dice una chapa: lo que hace tocarla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Que {
    Escaneado,
    LecturaVeloz,
}

pub(super) struct Chapa {
    pub(super) que: Que,
    texto: String,
    ancho_texto: u32,
}

/// Las chapas de una burbuja, ya medidas.
pub(super) struct Chapas {
    pub(super) lista: Vec<Chapa>,
    pub(super) ancho: f32,
    pub(super) alto: f32,
}

/// **Si llego del escaner del movil**: trae su `escaneado` (un objeto con la
/// app y el documento; lo que no sea un objeto no cuenta).
pub(super) fn es_escaneado(m: &Mensaje) -> bool {
    m.resto.get("escaneado").is_some_and(|v| v.is_object())
}

/// **Si el menu del mensaje lleva «⚡ Lectura veloz»**: un archivo que ella
/// lee, por su nombre o por su ruta (`LecturaVelozEnElTelefono.sePuede`).
/// Sin mirar el disco: si el archivo ya no esta, al pulsarlo se dice.
pub(super) fn se_ofrece_lectura_veloz(m: &Mensaje) -> bool {
    use pixpin_docs::lectura_veloz::tipo_de;
    m.clase == Some(Clase::Archivo)
        && m.ruta.as_deref().is_some_and(|r| !r.is_empty())
        && (tipo_de(std::path::Path::new(&m.nombre)).is_some()
            || m.ruta
                .as_deref()
                .is_some_and(|r| tipo_de(r.as_ref()).is_some()))
}

/// El % de la chapa de Lectura Veloz de un mensaje: un archivo que ella lee
/// o una nota (su ⋮), si alguna vez se leyo alli.
pub(super) fn porcentaje(m: &Mensaje) -> Option<u8> {
    let nota = m.clase == Some(Clase::Nota) && m.miniapp.is_none();
    if !nota && !se_ofrece_lectura_veloz(m) {
        return None;
    }
    crate::lectura_veloz::porcentaje(&m.codigo_unico())
}

/// **Las chapas de un mensaje**, medidas; `None` si no lleva ninguna.
pub(super) fn medir(
    p: &Pintor,
    m: &Mensaje,
    textos: &Catalogo,
    e: f32,
    ancho_max: f32,
) -> Option<Chapas> {
    let mut lista = Vec::new();
    let mut poner = |que, texto: String| {
        let (w, _) = p.medir_texto(&texto, chat::CHAPA_TAM as f32 * e);
        lista.push(Chapa {
            que,
            texto,
            ancho_texto: w.ceil() as u32,
        });
    };
    if es_escaneado(m) {
        poner(Que::Escaneado, textos.t("chat-escaneado"));
    }
    if let Some(pct) = porcentaje(m) {
        poner(Que::LecturaVeloz, crate::lectura_veloz::chapa(textos, pct));
    }
    if lista.is_empty() {
        return None;
    }
    let escala = (e * 100.0).round() as u32;
    let anchos: Vec<u32> = lista.iter().map(|c| c.ancho_texto).collect();
    let ancho = (chat::ancho_de_chapas(&anchos, escala) as f32).min(ancho_max);
    Some(Chapas {
        lista,
        ancho,
        alto: chat::CHAPA_ALTO as f32 * e,
    })
}

fn icono_de(que: Que) -> &'static Icono {
    match que {
        Que::Escaneado => &mi::DOCUMENT_SCANNER,
        Que::LecturaVeloz => &mi::SPEED,
    }
}

/// **Pinta las chapas** desde `x, y` sin pasar de `ancho_max`, en la tinta
/// apagada de la burbuja (la de la hora) sobre ella misma al 12 %, y
/// devuelve donde cayo cada una para el raton.
pub(super) fn pintar(
    p: &Pintor,
    c: &Chapas,
    x: i32,
    y: i32,
    ancho_max: u32,
    tinta: Color,
    e: f32,
) -> Vec<(Rect, Que)> {
    let escala = (e * 100.0).round() as u32;
    let anchos: Vec<u32> = c.lista.iter().map(|c| c.ancho_texto).collect();
    let cajas = chat::chapas_de_la_burbuja(x, y, &anchos, ancho_max, escala);
    let fondo = Color { a: 0.12, ..tinta };
    let tam = chat::CHAPA_TAM as f32 * e;
    let lado = chat::CHAPA_ICONO as f32 * e;
    let mut puestas = Vec::with_capacity(cajas.len());
    for (caja, chapa) in cajas.iter().zip(&c.lista) {
        let r = RectF {
            x: caja.x as f32,
            y: caja.y as f32,
            ancho: caja.ancho as f32,
            alto: caja.alto as f32,
        };
        p.rellenar_redondeado(r, r.alto / 2.0, fondo);
        let ix = r.x + chat::CHAPA_IZQUIERDA as f32 * e;
        p.icono(
            icono_de(chapa.que),
            RectF {
                x: ix,
                y: r.y + (r.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            tinta,
        );
        let tx = ix + lado + chat::CHAPA_AIRE_ICONO as f32 * e;
        let (_, alto_texto) = p.medir_texto(&chapa.texto, tam);
        p.texto_linea(
            &chapa.texto,
            tx,
            r.y + (r.alto - alto_texto) / 2.0,
            tam,
            (r.x + r.ancho - chat::CHAPA_DERECHA as f32 * e - tx).max(0.0) + 1.0,
            tinta,
        );
        puestas.push((*caja, chapa.que));
    }
    puestas
}

/// **«⚡ Lectura veloz» de un mensaje** (el menu o su chapa): su archivo en
/// Lectura Veloz con el codigo del mensaje. `None` si se abrio; si no, el
/// aviso con el motivo de verdad (ya apuntado en el registro).
pub(super) fn abrir(
    raiz: &std::path::Path,
    proyecto: &str,
    m: &Mensaje,
    textos: &Catalogo,
) -> Option<String> {
    // Sin ruta en este equipo, la que diria su nombre: el aviso es «ya no
    // esta (Tesis.pdf)».
    let ruta =
        super::ruta_del_mensaje(raiz, proyecto, m).unwrap_or_else(|| PathBuf::from(&m.nombre));
    let doc = m.codigo_unico();
    crate::lectura_veloz::abrir_guardado(&ruta, Some(&doc))
        .err()
        .map(|no| no.aviso(textos))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn archivo(nombre: &str, ruta: Option<&str>) -> Mensaje {
        Mensaje {
            id: "m1".into(),
            clase: Some(Clase::Archivo),
            nombre: nombre.into(),
            ruta: ruta.map(Into::into),
            ..Default::default()
        }
    }

    #[test]
    fn lo_escaneado_se_reconoce_por_su_objeto() {
        let mut m = archivo("Escaneo.pdf", Some("adjuntos/e.pdf"));
        assert!(!es_escaneado(&m));
        m.resto.insert(
            "escaneado".into(),
            serde_json::json!({"app": "com.escaner", "documento": "d1"}),
        );
        assert!(es_escaneado(&m));
        // Caso negativo: lo que no es un objeto no cuenta.
        m.resto.insert("escaneado".into(), serde_json::Value::Null);
        assert!(!es_escaneado(&m));
        m.resto.insert("escaneado".into(), serde_json::json!(true));
        assert!(!es_escaneado(&m));
    }

    #[test]
    fn lectura_veloz_se_ofrece_para_los_archivos_que_lee() {
        assert!(se_ofrece_lectura_veloz(&archivo(
            "Tesis.pdf",
            Some("a/x.pdf")
        )));
        assert!(se_ofrece_lectura_veloz(&archivo(
            "Libro.epub",
            Some("a/x.epub")
        )));
        // Un PDF que el movil guardo como `.bin`: vale por su nombre.
        assert!(se_ofrece_lectura_veloz(&archivo(
            "Tesis.pdf",
            Some("a/x.bin")
        )));
        // Caso negativo: una foto, un archivo sin ruta, una nota.
        assert!(!se_ofrece_lectura_veloz(&archivo(
            "foto.png",
            Some("a/foto.png")
        )));
        assert!(!se_ofrece_lectura_veloz(&archivo("Tesis.pdf", None)));
        let mut nota = archivo("Tesis.pdf", Some("a/x.pdf"));
        nota.clase = Some(Clase::Nota);
        assert!(!se_ofrece_lectura_veloz(&nota));
    }
}
