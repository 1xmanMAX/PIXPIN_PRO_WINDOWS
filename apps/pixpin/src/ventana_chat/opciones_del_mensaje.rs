//! **Que lleva el menu de un mensaje, segun lo que es** (29-oct-2026, como
//! el movil 442eadf, `guardados/OpcionesDelMensaje.kt`; el usuario: «mantengo
//! pulsado un PDF y me sale "recordatorio", "llamada falsa"… ¿como
//! funcionaria eso en un PDF? Es absurdo y solo ocupa espacio; analiza por
//! tipo —archivo, texto, audio, imagen…— y quita lo que no aplica»).
//!
//! El orden del menu ya era el del PC (`menu_de_mensaje`) y el movil lo
//! copio; lo que se trae de alli es **que sale en cada tipo** de lo que
//! antes salia en todos:
//!
//! - **Recordarmelo** (en una nota de voz, la llamada secreta): a la vista
//!   en una nota, un enlace, una mini-app y una nota de voz; en «Más» en
//!   una foto o una hoja; nunca en un documento, un archivo, un video, una
//!   cancion, una clase, una carpeta ni una respuesta del asistente. Con uno
//!   ya puesto se ofrece quitarlo siempre: si no, no habria por donde.
//! - **Hacer leccion**: a la vista de una nota, una nota de voz o una clase;
//!   en «Más» de un enlace, una foto, una hoja o una respuesta del
//!   asistente; nunca de un archivo (8-oct, «las lecciones no aceptan
//!   archivos»), un documento, un video, una cancion, una mini-app ni una
//!   carpeta.
//!
//! Sin ventanas: se prueba aqui abajo.

use pixpin_proyecto::cuaderno::{Clase, Mensaje};

/// Lo que es el mensaje, a efectos de su menu (`OpcionesDelMensaje.Tipo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tipo {
    /// Una nota escrita.
    Nota,
    /// Una nota escrita que lleva una direccion web.
    Enlace,
    /// Una nota de voz.
    Voz,
    /// Una cancion (`biblioteca_audio::es_musica`).
    Musica,
    /// La nota de una clase o reunion grabada.
    Grabacion,
    Foto,
    Video,
    /// Un documento que se lee: PDF, Word, libro, pagina, texto...
    Documento,
    /// Cualquier otro archivo: un zip, un plano, un instalador.
    Archivo,
    /// Lo que se abre como hoja: un lienzo, una pagina de un PDF, un croquis.
    Hoja,
    /// Una lista de tareas, una tabla u otra mini-app.
    MiniApp,
    /// Una carpeta de paginas, la de una grabacion o el acceso a un proyecto.
    Carpeta,
    /// Una respuesta del asistente.
    Asistente,
}

/// Donde va una entrada del menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Sitio {
    /// A la vista.
    Vista,
    /// En el submenu «Más».
    Mas,
}

/// **Que [`Tipo`] es `m`.**
pub(super) fn tipo_de(m: &Mensaje) -> Tipo {
    if crate::asistente::chat::es_del_asistente(m) && matches!(m.clase, None | Some(Clase::Nota)) {
        return Tipo::Asistente;
    }
    if pixpin_proyecto::carpeta_de_paginas::es_carpeta(m) || m.clase == Some(Clase::Proyecto) {
        return Tipo::Carpeta;
    }
    match &m.clase {
        None | Some(Clase::Nota) => {
            if super::tarjetas::primer_enlace(&m.texto).is_some() {
                Tipo::Enlace
            } else {
                Tipo::Nota
            }
        }
        Some(Clase::Voz) => {
            if crate::biblioteca_audio::es_musica(m) {
                Tipo::Musica
            } else if super::estudiar_clase::de_una_clase(m).is_some()
                || m.resto.contains_key("capturas")
            {
                Tipo::Grabacion
            } else {
                Tipo::Voz
            }
        }
        Some(Clase::Imagen) => Tipo::Foto,
        Some(Clase::Archivo) => {
            let ext = |s: &str| {
                std::path::Path::new(s)
                    .extension()
                    .map(|e| e.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default()
            };
            let es_video = [m.nombre.as_str(), m.ruta.as_deref().unwrap_or_default()]
                .iter()
                .any(|s| pixpin_shell::asociaciones::VIDEOS.contains(&ext(s).as_str()));
            if es_video {
                Tipo::Video
            } else if !crate::lecciones::almacen::es_leccion(m)
                && super::chapas::se_ofrece_lectura_veloz(m)
            {
                Tipo::Documento
            } else {
                Tipo::Archivo
            }
        }
        Some(Clase::MiniApp) => Tipo::MiniApp,
        // Un lienzo, una pagina, un croquis o lo que venga del movil.
        Some(Clase::Dibujo | Clase::Pagina | Clase::Proyecto | Clase::Otra(_)) => Tipo::Hoja,
    }
}

/// **Donde va «Recordármelo»** (o la llamada secreta), o `None` si en este
/// tipo no tiene sentido. `recordado`: ya tiene una hora puesta, y entonces
/// la entrada es la de quitarla, que va siempre a la vista.
pub(super) fn recordar(tipo: Tipo, recordado: bool) -> Option<Sitio> {
    if recordado {
        return Some(Sitio::Vista);
    }
    match tipo {
        Tipo::Nota | Tipo::Enlace | Tipo::MiniApp | Tipo::Voz => Some(Sitio::Vista),
        Tipo::Foto | Tipo::Hoja => Some(Sitio::Mas),
        _ => None,
    }
}

/// **Donde va «Hacer lección»**, o `None` si de este tipo no sale una (ni de
/// una leccion: eso lo mira quien llama).
pub(super) fn leccion(tipo: Tipo) -> Option<Sitio> {
    match tipo {
        Tipo::Nota | Tipo::Voz | Tipo::Grabacion => Some(Sitio::Vista),
        Tipo::Enlace | Tipo::Foto | Tipo::Hoja | Tipo::Asistente => Some(Sitio::Mas),
        _ => None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn de(clase: Clase, nombre: &str) -> Mensaje {
        Mensaje {
            id: "m".into(),
            clase: Some(clase),
            texto: "algo".into(),
            nombre: nombre.into(),
            ruta: (!nombre.is_empty()).then(|| nombre.to_string()),
            ..Mensaje::default()
        }
    }

    #[test]
    fn cada_mensaje_es_de_su_tipo() {
        assert_eq!(tipo_de(&de(Clase::Nota, "")), Tipo::Nota);
        let mut enlace = de(Clase::Nota, "");
        enlace.texto = "mira https://example.com/a".into();
        assert_eq!(tipo_de(&enlace), Tipo::Enlace);
        assert_eq!(tipo_de(&de(Clase::Voz, "voz.m4a")), Tipo::Voz);
        let mut clase = de(Clase::Voz, "clase.m4a");
        clase
            .resto
            .insert("grabacion".into(), serde_json::json!("clase"));
        assert_eq!(tipo_de(&clase), Tipo::Grabacion);
        let mut cancion = de(Clase::Voz, "cancion.mp3");
        cancion
            .resto
            .insert("estadoDelTexto".into(), serde_json::json!("letra"));
        assert_eq!(tipo_de(&cancion), Tipo::Musica);
        assert_eq!(tipo_de(&de(Clase::Imagen, "f.png")), Tipo::Foto);
        assert_eq!(tipo_de(&de(Clase::Archivo, "Tesis.pdf")), Tipo::Documento);
        assert_eq!(tipo_de(&de(Clase::Archivo, "clase.mp4")), Tipo::Video);
        assert_eq!(tipo_de(&de(Clase::Archivo, "fotos.zip")), Tipo::Archivo);
        assert_eq!(tipo_de(&de(Clase::Dibujo, "")), Tipo::Hoja);
        let mut lista = de(Clase::MiniApp, "");
        lista.miniapp = Some(pixpin_proyecto::mini::TAREAS.into());
        assert_eq!(tipo_de(&lista), Tipo::MiniApp);
        let mut carpeta = de(Clase::MiniApp, "");
        carpeta.miniapp = Some(pixpin_proyecto::carpeta_de_paginas::MINIAPP.into());
        assert_eq!(tipo_de(&carpeta), Tipo::Carpeta);
    }

    /// El caso del usuario: un PDF no ofrece ni recordatorio ni leccion.
    #[test]
    fn un_pdf_no_ofrece_recordatorio_ni_leccion() {
        let pdf = tipo_de(&de(Clase::Archivo, "Tesis.pdf"));
        assert_eq!(recordar(pdf, false), None);
        assert_eq!(leccion(pdf), None);
        // Pero si ya tiene uno puesto (del movil de antes), se puede quitar.
        assert_eq!(recordar(pdf, true), Some(Sitio::Vista));
    }

    #[test]
    fn recordar_solo_donde_tiene_sentido() {
        for t in [Tipo::Nota, Tipo::Enlace, Tipo::MiniApp, Tipo::Voz] {
            assert_eq!(recordar(t, false), Some(Sitio::Vista), "{t:?}");
        }
        for t in [Tipo::Foto, Tipo::Hoja] {
            assert_eq!(recordar(t, false), Some(Sitio::Mas), "{t:?}");
        }
        for t in [
            Tipo::Musica,
            Tipo::Grabacion,
            Tipo::Video,
            Tipo::Documento,
            Tipo::Archivo,
            Tipo::Carpeta,
            Tipo::Asistente,
        ] {
            assert_eq!(recordar(t, false), None, "{t:?}");
        }
    }

    #[test]
    fn la_leccion_sale_de_lo_que_se_estudia() {
        for t in [Tipo::Nota, Tipo::Voz, Tipo::Grabacion] {
            assert_eq!(leccion(t), Some(Sitio::Vista), "{t:?}");
        }
        for t in [Tipo::Enlace, Tipo::Foto, Tipo::Hoja, Tipo::Asistente] {
            assert_eq!(leccion(t), Some(Sitio::Mas), "{t:?}");
        }
        for t in [
            Tipo::Musica,
            Tipo::Video,
            Tipo::Documento,
            Tipo::Archivo,
            Tipo::MiniApp,
            Tipo::Carpeta,
        ] {
            assert_eq!(leccion(t), None, "{t:?}");
        }
    }
}
