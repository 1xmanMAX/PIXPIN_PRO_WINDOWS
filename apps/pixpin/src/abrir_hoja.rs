//! Abrir una hoja de un proyecto (un dibujo o una pagina) por su referencia,
//! sin tener el chat abierto: lo usan los pedidos de fuera, los grupos de
//! ventanas y las paginas vivas de las notas.

use std::path::Path;

/// Abre una hoja de un proyecto en el lienzo y la guarda al cerrar,
/// siguiendo los enlaces a otras hojas: lo mismo que pulsarla en el chat,
/// con la misma funcion (`ventana_chat::abrir_hojas`). Los mensajes se
/// leen aqui porque quien la pide no tiene el proyecto abierto como el chat.
pub fn abrir_hoja(
    raiz: &Path,
    proyecto: &str,
    referencia: &str,
    opciones: crate::ventana_chat::OpcionesLienzo,
) {
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, proyecto);
    let mensajes = match pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta) {
        Ok(c) => c.mensajes,
        Err(e) => {
            tracing::warn!(?e, %proyecto, "no se pudo leer el cuaderno de la hoja");
            return;
        }
    };
    match indice_de_referencia(&mensajes, referencia) {
        Some(i) => {
            crate::ventana_chat::abrir_hojas(raiz, proyecto, &mensajes, i, opciones);
        }
        // Una hoja que no esta en el chat (un sublienzo que solo esta en
        // `proyecto.json`, uno con el nombre de antes de importarse) se busca
        // como la busca una zona vinculada.
        None => {
            match crate::salto_por_enlace::hoja_del_enlace(raiz, proyecto, &mensajes, referencia) {
                Some(crate::salto_por_enlace::HojaDelEnlace::EnLaLista(i)) => {
                    crate::ventana_chat::abrir_hojas(raiz, proyecto, &mensajes, i, opciones);
                }
                Some(crate::salto_por_enlace::HojaDelEnlace::Leida(m)) => {
                    let mut todos = mensajes;
                    todos.push(m);
                    let i = todos.len() - 1;
                    crate::ventana_chat::abrir_hojas(raiz, proyecto, &todos, i, opciones);
                }
                None => tracing::warn!(%referencia, %proyecto, "la hoja no esta en el chat"),
            }
        }
    }
}

/// El mensaje que lleva esa hoja, que es por donde el chat la abre.
fn indice_de_referencia(
    mensajes: &[pixpin_proyecto::cuaderno::Mensaje],
    referencia: &str,
) -> Option<usize> {
    mensajes
        .iter()
        .position(|m| m.referencia.as_deref() == Some(referencia))
        // Una pagina sin dibujo se pide por su codigo unico.
        .or_else(|| {
            mensajes.iter().position(|m| {
                crate::pdf_en_chat::es_pagina_sin_dibujo(m) && m.codigo_unico() == referencia
            })
        })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::cuaderno::{Clase, Mensaje};

    #[test]
    fn la_hoja_se_busca_por_su_referencia_y_una_que_no_esta_no_se_abre() {
        let v = vec![
            Mensaje {
                id: "1".into(),
                ..Default::default()
            },
            Mensaje {
                id: "2".into(),
                referencia: Some("d1".into()),
                ..Default::default()
            },
        ];
        assert_eq!(indice_de_referencia(&v, "d1"), Some(1));
        assert_eq!(indice_de_referencia(&v, "otra"), None);
    }

    #[test]
    fn una_pagina_sin_dibujo_se_busca_por_su_codigo() {
        let v = vec![Mensaje {
            id: "7".into(),
            uid: Some("u-pag-3".into()),
            clase: Some(Clase::Pagina),
            pagina: Some(3),
            ..Default::default()
        }];
        assert_eq!(indice_de_referencia(&v, "u-pag-3"), Some(0));
        // Caso negativo: un codigo que no es de una pagina no encuentra nada.
        assert_eq!(indice_de_referencia(&v, "u-otra"), None);
    }
}
