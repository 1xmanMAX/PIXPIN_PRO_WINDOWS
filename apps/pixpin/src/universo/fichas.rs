//! Del mensaje del chat a lo que el universo necesita (D237).

use std::collections::HashMap;
use std::path::Path;

use pixpin_proyecto::cuaderno::{Clase, Cuaderno, Mensaje};
use pixpin_universo::buscar::normalizar;
use pixpin_universo::desde_el_chat::NodoDelChat;
use pixpin_universo::ficha::{ClaseLuna, FichaLuna, es_colocable, es_solo_emoji, extracto};

pub fn palabra_de(m: &Mensaje) -> &str {
    match &m.clase {
        Some(Clase::Nota) => "NOTA",
        Some(Clase::Imagen) => "IMAGEN",
        Some(Clase::Archivo) => "ARCHIVO",
        Some(Clase::Voz) => "VOZ",
        Some(Clase::Dibujo) => "DIBUJO",
        Some(Clase::Pagina) => "PAGINA",
        Some(Clase::Proyecto) => "PROYECTO",
        Some(Clase::MiniApp) => "MINIAPP",
        Some(Clase::Otra(p)) => p,
        None => "NOTA",
    }
}

pub fn de_mensaje(m: &Mensaje, raiz: &Path, proyecto: &str) -> FichaLuna {
    let clase = ClaseLuna::de_palabra(palabra_de(m));
    let texto = m.resumen();
    let ruta = m.ruta.clone().filter(|r| !r.is_empty());
    // Misma regla que `ventana_chat::ruta_del_mensaje`: una ruta absoluta es
    // de otro aparato y aqui no significa nada.
    let en_equipo = ruta.as_deref().is_some_and(|r| {
        !Path::new(r).is_absolute()
            && pixpin_proyecto::almacen::carpeta(raiz, proyecto)
                .join(r)
                .is_file()
    }) || matches!(clase, ClaseLuna::Nota | ClaseLuna::MiniApp);
    FichaLuna {
        codigo: m.codigo_unico(),
        proyecto: proyecto.to_string(),
        clase,
        nombre: if m.nombre.is_empty() {
            extracto(&texto)
        } else {
            m.nombre.clone()
        },
        ruta,
        bytes: m.bytes,
        cuando: m.cuando,
        codigo_chat: m.codigo_chat(),
        extracto: extracto(&texto),
        busqueda: normalizar(&format!("{} {}", m.nombre, extracto(&texto))),
        en_equipo,
        referencia: m.referencia.clone(),
        solo_emoji: clase == ClaseLuna::Nota && es_solo_emoji(&m.texto),
    }
}

/// Si el mensaje tiene luna en el universo. Lo usa el chat para ofrecer
/// «Mostrar en el universo» (D212) solo donde hay algo que ensenar.
///
/// Desde H2 las notas tambien: el universo se arma con el chat entero, y un
/// comentario es un cuerpo pequeno en orbita, como en el movil. Lo del buzon
/// sigue fuera: no es de ningun proyecto todavia.
pub fn es_luna(m: &Mensaje) -> bool {
    es_colocable(ClaseLuna::de_palabra(palabra_de(m)), m.en_buzon, true)
}

/// **El chat con la forma que le da al universo** (`nodosDelChat` del movil,
/// `Universo.kt:817`): a que contesta cada mensaje, por codigo unico, y que
/// es. Una pagina cuelga de su documento por `referencia`; una respuesta a
/// algo que no esta en el chat sube al proyecto. Sin lo del buzon.
pub fn nodos_de(c: &Cuaderno) -> Vec<NodoDelChat> {
    let codigos: HashMap<&str, String> = c
        .mensajes
        .iter()
        .filter(|m| !m.en_buzon)
        .map(|m| (m.id.as_str(), m.codigo_unico()))
        .collect();
    c.mensajes
        .iter()
        .filter(|m| !m.en_buzon)
        .map(|m| {
            let clase = ClaseLuna::de_palabra(palabra_de(m));
            let padre = m.responde_a.as_deref().or(if clase == ClaseLuna::Pagina {
                m.referencia.as_deref()
            } else {
                None
            });
            let es_texto = clase == ClaseLuna::Nota;
            NodoDelChat {
                codigo: m.codigo_unico(),
                responde_a: padre.and_then(|p| codigos.get(p)).cloned(),
                pagina: m.pagina.map(|p| p + 1),
                es_documento: clase == ClaseLuna::Archivo
                    && m.nombre.to_lowercase().ends_with(".pdf"),
                es_texto,
                solo_emoji: es_texto && es_solo_emoji(&m.texto),
            }
        })
        .collect()
}

pub fn de_cuaderno(
    c: &Cuaderno,
    raiz: &Path,
    proyecto: &str,
    incluir_notas: bool,
) -> Vec<FichaLuna> {
    c.mensajes
        .iter()
        .filter(|m| {
            es_colocable(
                ClaseLuna::de_palabra(palabra_de(m)),
                m.en_buzon,
                incluir_notas,
            )
        })
        .map(|m| de_mensaje(m, raiz, proyecto))
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::cuaderno::{Clase, Mensaje};
    use pixpin_universo::ficha::ClaseLuna;

    fn mensaje(clase: Clase, ruta: Option<&str>) -> Mensaje {
        Mensaje {
            id: "1".into(),
            cuando: 5,
            clase: Some(clase),
            nombre: "Plano Cocina.pdf".into(),
            ruta: ruta.map(String::from),
            texto: "x".repeat(500),
            uid: Some("U1".into()),
            ..Default::default()
        }
    }

    #[test]
    fn una_ficha_lleva_el_codigo_unico_la_clase_y_un_extracto_de_doscientas() {
        let raiz = std::env::temp_dir();
        let f = de_mensaje(
            &mensaje(Clase::Archivo, Some("archivos/p.pdf")),
            &raiz,
            "p1",
        );
        assert_eq!(f.codigo, mensaje(Clase::Archivo, None).codigo_unico());
        assert_eq!(f.clase, ClaseLuna::Archivo);
        assert_eq!(f.extracto.chars().count(), 200);
        assert!(f.busqueda.contains("plano cocina"));
    }

    #[test]
    fn una_ruta_absoluta_del_movil_no_esta_en_este_equipo() {
        let raiz = std::env::temp_dir();
        let f = de_mensaje(
            &mensaje(Clase::Imagen, Some("/storage/emulated/0/a.jpg")),
            &raiz,
            "p1",
        );
        assert!(!f.en_equipo);
    }

    #[test]
    fn el_cuaderno_deja_fuera_el_buzon_y_las_notas_salvo_que_se_pidan() {
        let raiz = std::env::temp_dir();
        let mut buzon = mensaje(Clase::Imagen, None);
        buzon.en_buzon = true;
        buzon.id = "2".into();
        buzon.uid = Some("U2".into());
        let mut nota = mensaje(Clase::Nota, None);
        nota.id = "3".into();
        nota.uid = Some("U3".into());
        let c = pixpin_proyecto::cuaderno::Cuaderno {
            mensajes: vec![mensaje(Clase::Archivo, None), buzon, nota],
            lineas_rotas: 0,
        };
        assert_eq!(de_cuaderno(&c, &raiz, "p1", false).len(), 1);
        assert_eq!(de_cuaderno(&c, &raiz, "p1", true).len(), 2);
    }

    #[test]
    fn mostrar_en_el_universo_sale_en_archivos_fotos_y_notas_y_no_en_el_buzon() {
        assert!(es_luna(&mensaje(Clase::Archivo, Some("archivos/a.pdf"))));
        assert!(es_luna(&mensaje(Clase::Imagen, Some("archivos/a.jpg"))));
        assert!(es_luna(&mensaje(Clase::Dibujo, None)));
        // H2: un comentario tambien esta en el universo, en orbita.
        assert!(es_luna(&mensaje(Clase::Nota, None)));
        let mut buzon = mensaje(Clase::Imagen, None);
        buzon.en_buzon = true;
        assert!(!es_luna(&buzon));
    }

    fn con(id: &str, clase: Clase, nombre: &str, texto: &str) -> Mensaje {
        Mensaje {
            id: id.into(),
            clase: Some(clase),
            nombre: nombre.into(),
            texto: texto.into(),
            uid: Some(format!("U{id}")),
            ..Default::default()
        }
    }

    #[test]
    fn los_nodos_del_chat_llevan_a_que_contestan_por_codigo_y_que_son() {
        let pdf = con("1", Clase::Archivo, "Plano.PDF", "");
        let mut pag = con("2", Clase::Pagina, "Plano · 1", "");
        pag.referencia = Some("1".into());
        pag.pagina = Some(0);
        let mut nota = con("3", Clase::Nota, "", "ojo con la cota");
        nota.responde_a = Some("2".into());
        let mut pulgar = con("4", Clase::Nota, "", "👍");
        pulgar.responde_a = Some("no-esta".into());
        let mut buzon = con("5", Clase::Imagen, "b.jpg", "");
        buzon.en_buzon = true;
        let c = Cuaderno {
            mensajes: vec![pdf.clone(), pag.clone(), nota, pulgar, buzon],
            lineas_rotas: 0,
        };
        let n = nodos_de(&c);
        assert_eq!(n.len(), 4, "sin el buzon");
        assert!(n[0].es_documento && n[0].responde_a.is_none());
        // La pagina cuelga del PDF por `referencia`, con su numero desde 1.
        assert_eq!(n[1].responde_a, Some(pdf.codigo_unico()));
        assert_eq!(n[1].pagina, Some(1));
        assert_eq!(n[2].responde_a, Some(pag.codigo_unico()));
        assert!(n[2].es_texto && !n[2].solo_emoji);
        // Una respuesta a algo que no esta en el chat sube al proyecto.
        assert!(n[3].responde_a.is_none() && n[3].solo_emoji);
    }
}
