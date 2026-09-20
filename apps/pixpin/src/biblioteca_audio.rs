//! La biblioteca de audio: toda la musica y todas las notas de voz, juntas.
//!
//! Es `guardados/BibliotecaDeAudioActivity.kt` del movil, y lo primero que
//! hay que entender de ella es que **no es un almacen**: no guarda nada
//! propio (:47-56). Es una vista: filtra los mensajes de voz de todas las
//! conversaciones y los parte en dos grupos, musica y notas.
//!
//! Aqui esta solo el filtrado, que es puro y se comprueba entero sin ventana
//! ni sonido. Lo que pinte la lista vive en `ventana_chat.rs` y se engancha
//! en la tanda de costura.

use pixpin_proyecto::cuaderno::{Clase, Mensaje};

/// El valor de `estadoDelTexto` que marca «esto es musica, y lo que hay
/// escrito es su letra».
///
/// `TEXTO_LETRA` (`guardados/Mensajes.kt:594`). El campo no esta declarado
/// en el `Mensaje` de Rust, asi que se lee de `resto`, igual que `picos`.
pub const TEXTO_LETRA: &str = "letra";

/// En que grupo de la lista va un audio.
///
/// Dos y no mas porque son los dos que el movil rotula
/// (`BibliotecaDeAudioActivity.kt:100`): la musica primero, las notas de voz
/// despues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grupo {
    Musica,
    Notas,
}

/// Una fila de la biblioteca.
#[derive(Debug, Clone, PartialEq)]
pub struct Fila {
    /// El `id` del mensaje: lo que hace falta para abrir la letra.
    pub id: String,
    /// La ruta del fichero de audio. Nunca vacia: sin fichero no hay fila.
    pub ruta: String,
    pub titulo: String,
    pub duracion_ms: i64,
    /// De que conversacion es. `None` es la general.
    pub proyecto: Option<String>,
    pub grupo: Grupo,
    /// Si ya tiene transcripcion: el movil pinta el icono de la letra en
    /// color cuando la hay (`BibliotecaDeAudioActivity.kt:138`).
    pub tiene_letra: bool,
    pub cuando: i64,
}

/// Si un mensaje es musica: una nota de voz cuyo texto quedo marcado como
/// letra de cancion.
///
/// `val Mensaje.esMusica` (`Mensajes.kt:601-602`).
pub fn es_musica(m: &Mensaje) -> bool {
    m.clase == Some(Clase::Voz)
        && m.resto
            .get("estadoDelTexto")
            .and_then(|v| v.as_str())
            .is_some_and(|v| v == TEXTO_LETRA)
}

/// El rotulo de un audio en la lista.
///
/// `tituloDeAudio` (`guardados/LetraActivity.kt:236-240`): el nombre del
/// fichero sin su extension y, si no hay nombre, la palabra «voz» con la
/// fecha. La fecha la pone quien pinta, que es el unico que sabe el idioma y
/// la zona horaria; aqui se devuelve vacio para que se note que hay que
/// rellenarlo y no una cadena en ingles metida a hurtadillas.
pub fn titulo_de_audio(m: &Mensaje) -> String {
    let nombre = m.nombre.trim();
    match nombre.rsplit_once('.') {
        // `rsplit_once` con un punto al principio («.oculto») daria cadena
        // vacia: en ese caso vale mas el nombre entero.
        Some((base, _)) if !base.is_empty() => base.to_string(),
        _ => nombre.to_string(),
    }
}

/// La lista de la biblioteca a partir de todos los mensajes de todas las
/// conversaciones.
///
/// El filtro es el del movil, clavado
/// (`BibliotecaDeAudioActivity.kt:71-73`): **clase `VOZ`**, **con ruta** y
/// **fuera del buzon**. Lo del buzon importa: un audio borrado sigue en el
/// JSONL hasta que se vacia la papelera, y ensenarlo aqui seria resucitar
/// algo que el usuario tiro.
///
/// El orden: la musica primero y, dentro de cada grupo, lo mas nuevo arriba.
pub fn biblioteca(mensajes: &[Mensaje]) -> Vec<Fila> {
    let mut filas: Vec<Fila> = mensajes
        .iter()
        .filter(|m| m.clase == Some(Clase::Voz) && !m.en_buzon)
        .filter_map(|m| {
            let ruta = m.ruta.as_deref().map(str::trim).filter(|r| !r.is_empty())?;
            Some(Fila {
                id: m.id.clone(),
                ruta: ruta.to_string(),
                titulo: titulo_de_audio(m),
                duracion_ms: m.duracion_ms.max(0),
                proyecto: m.proyecto.clone(),
                grupo: if es_musica(m) {
                    Grupo::Musica
                } else {
                    Grupo::Notas
                },
                tiene_letra: m
                    .transcripcion
                    .as_ref()
                    .is_some_and(|t| !t.trim().is_empty()),
                cuando: m.cuando,
            })
        })
        .collect();

    // `sort_by` y no `sort_unstable_by`: dos notas grabadas en el mismo
    // milisegundo —una importacion en tanda— tienen que salir siempre en el
    // mismo orden, o la lista bailaria entre dos aperturas.
    filas.sort_by(|a, b| {
        let grupo = (a.grupo == Grupo::Notas).cmp(&(b.grupo == Grupo::Notas));
        grupo.then(b.cuando.cmp(&a.cuando))
    });
    filas
}

/// Las filas de un grupo, para pintar sus dos rotulos.
pub fn del_grupo(filas: &[Fila], grupo: Grupo) -> Vec<&Fila> {
    filas.iter().filter(|f| f.grupo == grupo).collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    fn voz(id: &str, cuando: i64) -> Mensaje {
        Mensaje {
            id: id.to_string(),
            cuando,
            clase: Some(Clase::Voz),
            ruta: Some(format!("C:/voz/{id}.m4a")),
            nombre: format!("{id}.m4a"),
            duracion_ms: 3_000,
            ..Mensaje::default()
        }
    }

    #[test]
    fn la_biblioteca_solo_trae_notas_de_voz_con_fichero() {
        let mut nota = voz("a", 10);
        let mut sin_ruta = voz("b", 20);
        sin_ruta.ruta = None;
        let mut ruta_vacia = voz("c", 30);
        ruta_vacia.ruta = Some("   ".into());
        let mut texto = voz("d", 40);
        texto.clase = Some(Clase::Nota);
        nota.nombre = "a.m4a".into();

        let filas = biblioteca(&[nota, sin_ruta, ruta_vacia, texto]);
        assert_eq!(
            filas.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(),
            ["a"]
        );
    }

    #[test]
    fn un_audio_tirado_a_la_papelera_no_sale_en_la_biblioteca() {
        let mut tirado = voz("x", 10);
        tirado.en_buzon = true;
        assert!(biblioteca(&[tirado]).is_empty());
    }

    #[test]
    fn la_musica_va_primero_y_dentro_de_cada_grupo_lo_mas_nuevo_arriba() {
        let nota_vieja = voz("n1", 100);
        let nota_nueva = voz("n2", 300);
        let mut cancion = voz("m1", 50);
        cancion
            .resto
            .insert("estadoDelTexto".into(), json!(TEXTO_LETRA));

        let filas = biblioteca(&[nota_vieja, nota_nueva, cancion]);
        assert_eq!(
            filas.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(),
            ["m1", "n2", "n1"]
        );
        assert_eq!(filas[0].grupo, Grupo::Musica);
        assert_eq!(del_grupo(&filas, Grupo::Notas).len(), 2);
    }

    #[test]
    fn un_estado_del_texto_que_no_es_letra_no_convierte_una_nota_en_musica() {
        let mut m = voz("y", 1);
        m.resto.insert("estadoDelTexto".into(), json!("bien"));
        assert!(!es_musica(&m));
        // Y un valor que ni siquiera es texto tampoco tumba nada.
        m.resto.insert("estadoDelTexto".into(), json!(7));
        assert!(!es_musica(&m));
    }

    #[test]
    fn el_titulo_quita_la_extension_y_aguanta_los_nombres_raros() {
        let mut m = voz("z", 1);
        m.nombre = "lectura-1758351600000.m4a".into();
        assert_eq!(titulo_de_audio(&m), "lectura-1758351600000");
        m.nombre = "sin extension".into();
        assert_eq!(titulo_de_audio(&m), "sin extension");
        m.nombre = ".oculto".into();
        assert_eq!(titulo_de_audio(&m), ".oculto");
        // Sin nombre devuelve vacio a proposito: la fecha la pone quien
        // pinta, que es el que sabe el idioma.
        m.nombre = String::new();
        assert_eq!(titulo_de_audio(&m), "");
    }

    #[test]
    fn una_duracion_negativa_llegada_de_fuera_no_pinta_una_barra_al_reves() {
        let mut m = voz("w", 1);
        m.duracion_ms = -5;
        assert_eq!(biblioteca(&[m])[0].duracion_ms, 0);
    }

    #[test]
    fn la_letra_solo_cuenta_si_tiene_algo_escrito() {
        let mut m = voz("v", 1);
        assert!(!biblioteca(&[m.clone()])[0].tiene_letra);
        m.transcripcion = Some("   ".into());
        assert!(!biblioteca(&[m.clone()])[0].tiene_letra);
        m.transcripcion = Some("hola".into());
        assert!(biblioteca(&[m])[0].tiene_letra);
    }
}
