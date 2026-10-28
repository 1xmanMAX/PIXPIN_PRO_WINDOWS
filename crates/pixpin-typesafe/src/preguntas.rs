//! **Todas las preguntas que PixPin le hace a Jev**, en un solo sitio: su
//! texto, sus criterios, los niveles de cada Score y los umbrales con que se
//! leen. Es lo que mas se revisa y se cambia; el resto del crate solo arma el
//! estado y decide con las respuestas (ver `docs/jev.md`).
//!
//! Reglas que se siguen (las de la guia de TypeSafe):
//!
//! - **Una sola cosa por pregunta**, la que una persona juzgaria de un
//!   vistazo; la decision (que se marca, que tarjeta queda, que prioridad,
//!   que modelo) la toma el codigo con estas respuestas.
//! - Las preguntas nombran los campos del estado por su ruta entre
//!   comillas invertidas (`linea`, `dicho`, `tarea`, `pedido`).
//! - Cada nivel de un Score es una situacion que se entiende sola, y el
//!   «no» de cada Noul es una situacion concreta, no «lo contrario».
//! - **En ingles**: es el idioma en que Jev acierta mas (pagina de modelos,
//!   «Language support»). El estado va tal cual, en el idioma de la clase o
//!   del chat; los ejemplos de los criterios estan en espanol porque es lo
//!   que Jev va a leer.
//!
//! Cambiar un texto cambia la huella de la peticion: lo ya guardado en el
//! cache del proceso deja de valer, que es lo que debe pasar.

use crate::api::Pregunta;

/// Por encima, un Noul se lee «si».
pub const SI: f64 = 0.7;
/// Por debajo, «no». Entre los dos, **sin decidir**: tiene su propia fila en
/// cada tabla.
pub const NO: f64 = 0.3;

/// Como se lee un Noul.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lectura {
    Si,
    Dudoso,
    No,
}

/// Lee una probabilidad con [`SI`] y [`NO`].
pub fn leer(p: f64) -> Lectura {
    if p >= SI {
        Lectura::Si
    } else if p <= NO {
        Lectura::No
    } else {
        Lectura::Dudoso
    }
}

/// **Partes importantes de una reunion o clase**, una peticion por renglon
/// de la transcripcion. Estado: `clase` (el titulo), `linea` (el renglon),
/// `antes` y `despues` (hasta dos renglones de cada lado; si no hay, el
/// campo no va).
pub mod reunion {
    use super::*;

    pub const TAREA: &str = "tarea";
    pub const TAREA_QUIEN: &str = "tarea_quien";
    pub const TAREA_CUANDO: &str = "tarea_cuando";
    pub const DECISION: &str = "decision";
    pub const PREGUNTA: &str = "pregunta";
    pub const EXAMEN: &str = "examen";
    pub const DATO: &str = "dato";
    pub const CHARLA: &str = "charla";

    /// Cuantos renglones de cada lado van de contexto.
    pub const VECINOS: usize = 2;

    pub fn preguntas() -> Vec<(&'static str, Pregunta)> {
        vec![
            (
                TAREA,
                Pregunta::noul(
                    "Does `linea` contain an action item: something a person has to do after this moment?",
                    "Someone is given a task or says they will do one, e.g. «para el lunes resuelvan el ejercicio 3» or «yo mando el informe mañana».",
                    "The speaker explains or discusses the topic and nobody is asked or offers to do anything.",
                ),
            ),
            // Especulativas: solo se leen si `tarea` es «si».
            (
                TAREA_QUIEN,
                Pregunta::noul(
                    "Does `linea` say which person or group has to do the task?",
                    "It names who does it, e.g. «Ana», «el grupo 2», «todos», «yo».",
                    "Nobody is named as responsible, e.g. «hay que revisar el capítulo».",
                ),
            ),
            (
                TAREA_CUANDO,
                Pregunta::noul(
                    "Does `linea` say when the task is due?",
                    "A day, date or moment is given, e.g. «para el lunes», «antes del parcial», «mañana».",
                    "No deadline is mentioned, e.g. «lean el capítulo 4».",
                ),
            ),
            (
                DECISION,
                Pregunta::noul(
                    "Does `linea` announce a decision that has been made?",
                    "Something is settled from now on, e.g. «entonces el examen será oral» or «queda aprobado el presupuesto».",
                    "Options are only being discussed or proposed, e.g. «podríamos hacerlo oral».",
                ),
            ),
            (
                PREGUNTA,
                Pregunta::noul(
                    "Does someone in `linea` ask a question that expects an answer from another person?",
                    "A real question is asked, e.g. «¿alguien sabe por qué baja la presión?» or «profe, ¿esto entra?».",
                    "The speaker asks and answers at once, e.g. «¿y por qué pasa esto? Porque…», or there is no question.",
                ),
            ),
            (
                EXAMEN,
                Pregunta::noul(
                    "Does `linea` say that something will be on an exam, a test or a graded assignment?",
                    "Assessment is named, e.g. «esto entra en el parcial» or «lo voy a tomar en la práctica».",
                    "Exams, tests and grades are not mentioned.",
                ),
            ),
            (
                DATO,
                Pregunta::noul(
                    "Does `linea` state a definition, formula, rule or key fact of the topic of `clase`?",
                    "A student would copy it into their notes, e.g. «la velocidad es la derivada de la posición».",
                    "It is an example, an anecdote, a transition or a remark about the class itself.",
                ),
            ),
            (
                CHARLA,
                Pregunta::noul(
                    "Is `linea` small talk unrelated to the topic of `clase`?",
                    "Greetings, jokes, chit-chat or the logistics of the room or the call, e.g. «¿me escuchan?» or «buenos días a todos».",
                    "The line is about the subject that is being taught or discussed.",
                ),
            ),
        ]
    }
}

/// **Tarjetas de Anki que valen la pena**: una peticion por tarjeta de la
/// clase. Estado: `clase` (el titulo), `minuto` («12:30») y `dicho` (lo que
/// se dijo en ese tramo, el reverso de la tarjeta).
pub mod anki {
    use super::*;

    pub const VALOR: &str = "valor";
    pub const EXAMEN: &str = "examen";

    /// Los niveles de [`VALOR`], del 0 al 3.
    pub const NIVELES: [&str; 4] = [
        "Nothing to study: greetings, logistics, jokes, or words cut off so they make no sense on their own.",
        "An example or a comment that only makes sense while watching the class.",
        "It explains part of the topic, but a student would rather study it from another moment.",
        "It states a definition, formula, rule or key idea of the topic that a student would want to memorise.",
    ];

    pub fn preguntas() -> Vec<(&'static str, Pregunta)> {
        vec![
            (
                VALOR,
                Pregunta::score(
                    "How useful is `dicho` as the back of a flashcard for studying `clase`?",
                    &NIVELES,
                ),
            ),
            (
                EXAMEN,
                Pregunta::noul(
                    "Does `dicho` say that this will be on an exam, a test or a graded assignment?",
                    "Assessment is named, e.g. «esto entra en el parcial».",
                    "Exams, tests and grades are not mentioned.",
                ),
            ),
        ]
    }
}

/// **La urgencia de una tarea por su contexto**: una peticion por tarea
/// pendiente. Estado: `tarea` (su texto, sin marcas), `lista` (el titulo de
/// su lista), `chat` (el nombre del chat) y, si hay, `mensajes_recientes`
/// (los ultimos del chat, del mas viejo al mas nuevo). Las fechas **no**
/// van: cuanto falta para la hora de la tarea lo cuenta el codigo.
pub mod urgencia {
    use super::*;

    pub const CONSECUENCIA: &str = "consecuencia";
    pub const BLOQUEA: &str = "bloquea";
    pub const PLAZO_AJENO: &str = "plazo_ajeno";
    pub const PIDEN_YA: &str = "piden_ya";

    /// Los niveles de [`CONSECUENCIA`], del 0 al 3.
    pub const NIVELES: [&str; 4] = [
        "Nothing happens: it is an idea, a wish or something optional.",
        "A small inconvenience for the person, e.g. a chore at home takes longer.",
        "It hurts work or studies, e.g. a lower grade or a colleague kept waiting.",
        "Serious harm, e.g. a failed course, a missed payment or appointment, or a legal or health problem.",
    ];

    /// `piden_ya` solo va si hay mensajes recientes: una lista vacia se lee
    /// como una senal.
    pub fn preguntas(con_mensajes: bool) -> Vec<(&'static str, Pregunta)> {
        let mut v = vec![
            (
                CONSECUENCIA,
                Pregunta::score("What happens if `tarea` is done late?", &NIVELES),
            ),
            (
                BLOQUEA,
                Pregunta::noul(
                    "Is someone else waiting for `tarea` to be done before they can continue?",
                    "Other people depend on it, e.g. the group needs this part for the report, or a client waits for the invoice.",
                    "Only the person who wrote it is affected, e.g. «ordenar mis apuntes».",
                ),
            ),
            (
                PLAZO_AJENO,
                Pregunta::noul(
                    "Is `tarea` tied to a deadline that someone else set, such as a teacher, an office or an event?",
                    "e.g. handing in homework, paying a bill, signing up for an exam, a flight.",
                    "The person can choose when to do it, e.g. «leer ese libro» or «ordenar el escritorio».",
                ),
            ),
        ];
        if con_mensajes {
            v.push((
                PIDEN_YA,
                Pregunta::noul(
                    "Does someone in `mensajes_recientes` ask for `tarea` to be done soon?",
                    "e.g. «¿ya está lo del informe?» or «lo necesito para hoy».",
                    "The recent messages are about other things, or mention it without hurry.",
                ),
            ));
        }
        v
    }
}

/// **Que modelo contesta al asistente**: una peticion por pedido. Estado:
/// `pedido` (lo escrito) y, si hay, `conversacion_reciente` (los ultimos
/// mensajes, con quien los dijo).
pub mod modelo {
    use super::*;

    pub const DIFICULTAD: &str = "dificultad";
    pub const ORDEN: &str = "orden_simple";
    pub const RAZONAR: &str = "razonar";
    pub const IMPORTA: &str = "importa";

    /// Los niveles de [`DIFICULTAD`], del 0 al 3.
    pub const NIVELES: [&str; 4] = [
        "A single direct action in the app, e.g. «abre la calculadora» or «apunta comprar pan».",
        "A short answer or a simple drawing, e.g. «¿cuánto es el 15 % de 80?» or «dibuja un círculo rojo».",
        "An explanation or a piece of work with several parts, e.g. «explícame la fotosíntesis con un esquema».",
        "Careful reasoning over a lot of material, e.g. a multi-step physics problem, planning a project or analysing a long document.",
    ];

    pub fn preguntas() -> Vec<(&'static str, Pregunta)> {
        vec![
            (
                DIFICULTAD,
                Pregunta::score(
                    "How hard is it to do well what `pedido` asks for?",
                    &NIVELES,
                ),
            ),
            (
                ORDEN,
                Pregunta::noul(
                    "Is `pedido` a simple command for the app, such as opening, creating or noting something?",
                    "e.g. «abre el lector», «pon un temporizador de 5 minutos», «nueva nota».",
                    "It asks a question, or asks for an explanation, a text or a drawing to be made.",
                ),
            ),
            (
                RAZONAR,
                Pregunta::noul(
                    "Does `pedido` need several steps of reasoning, such as solving a problem, proving something or comparing options?",
                    "e.g. «resuelve: un tren sale a 80 km/h…», «compara estos dos planes».",
                    "The answer is direct, e.g. «¿qué hora es en Tokio?» or «dibuja un triángulo».",
                ),
            ),
            (
                IMPORTA,
                Pregunta::noul(
                    "Would a mistake in the answer to `pedido` have real consequences for the user, such as a grade, money, health or a work deadline?",
                    "e.g. the homework due tomorrow, a budget, a dose.",
                    "A mistake costs nothing, e.g. a doodle, a curiosity or a test of the app.",
                ),
            ),
        ]
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_umbrales_tienen_su_franja_de_duda() {
        assert_eq!(leer(0.71), Lectura::Si);
        assert_eq!(leer(0.7), Lectura::Si);
        assert_eq!(leer(0.5), Lectura::Dudoso);
        assert_eq!(leer(0.3), Lectura::No);
        assert_eq!(leer(0.05), Lectura::No);
    }

    #[test]
    fn cada_pregunta_tiene_texto_y_un_nombre_unico() {
        for lista in [
            reunion::preguntas(),
            anki::preguntas(),
            urgencia::preguntas(true),
            modelo::preguntas(),
        ] {
            let mut nombres: Vec<&str> = lista.iter().map(|(n, _)| *n).collect();
            nombres.sort();
            nombres.dedup();
            assert_eq!(nombres.len(), lista.len());
            for (n, p) in &lista {
                let texto = serde_json::to_string(p).unwrap();
                assert!(texto.contains('`'), "{n} no nombra su campo del estado");
            }
        }
        // Sin mensajes recientes, no se pregunta por ellos.
        assert!(
            !urgencia::preguntas(false)
                .iter()
                .any(|(n, _)| *n == urgencia::PIDEN_YA)
        );
    }
}
