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

/// **Que temas de la agenda ya se trataron** en una reunion que se esta
/// grabando, **en vivo** (29-oct-2026): una peticion cada vez que llega
/// texto nuevo (frase a frase, como mucho una cada 1,5 s), con tres Nouls
/// por cada tema aun pendiente (el tema va dentro de la pregunta: cada
/// tema, su pregunta). Estado: `reunion` (el titulo, si lo hay), `agenda`
/// (todos los temas, para que se sepa de que otros se hablara) y
/// `conversacion`, los ultimos ~40 s pasados a texto: `conversacion.antes`
/// (lo ya mirado en la peticion anterior; si no hay, no va) y
/// `conversacion.ahora` (lo que se acaba de decir y aun nadie miro). Lo
/// copia el movil tal cual (`docs/jev.md`).
pub mod temas {
    use super::*;

    /// El nombre de la respuesta de «se trato» del tema `id`.
    pub fn tratado(id: &str) -> String {
        format!("tratado_{id}")
    }

    /// El de «quedo cerrado».
    pub fn cerrado(id: &str) -> String {
        format!("cerrado_{id}")
    }

    /// Y el de «se habla de el en lo ultimo».
    pub fn ahora(id: &str) -> String {
        format!("ahora_{id}")
    }

    /// Por encima de esto en dos peticiones, la segunda con algo nuevo del
    /// tema, tratado (ver `crate::temas`): una conversacion larga sobre un
    /// tema se reparte en muchas frases y al principio parece a medias.
    pub const CASI: f64 = 0.5;

    /// Cuantos temas pendientes van en una peticion (tres preguntas cada
    /// uno). Los demas, en la siguiente.
    pub const POR_PETICION: usize = 20;

    /// **¿Se trato el tema en `conversacion`?** El caso que confunde, dicho
    /// en el «no»: nombrarlo de pasada o dejarlo para luego no es tratarlo.
    pub fn se_trato(tema: &str) -> Pregunta {
        Pregunta::noul(
            &format!(
                "In `conversacion`, do the speakers actually discuss the agenda topic «{tema}»?"
            ),
            "They talk about it in substance: they explain it, give opinions, ask and answer about it, or decide something about it, e.g. for «presupuesto 2027»: «el presupuesto queda en cuarenta mil, lo aprobamos».",
            "It is only named in passing or announced for later, e.g. «del presupuesto hablamos al final» or «eso lo vemos la próxima semana»; or `conversacion` is about other topics.",
        )
    }

    /// **¿Quedo cerrado?** Una decision, una respuesta o un siguiente paso
    /// acordado.
    pub fn quedo_cerrado(tema: &str) -> Pregunta {
        Pregunta::noul(
            &format!(
                "In `conversacion`, is the agenda topic «{tema}» settled with a decision, an answer or an agreed next step?"
            ),
            "Something is concluded about it, e.g. «entonces queda así», «lo aprobamos» or «Ana se encarga y lo revisamos el lunes».",
            "It is still open: options are being compared, it is postponed, e.g. «lo dejamos para la próxima», or it is not talked about in `conversacion`.",
        )
    }

    /// **¿Lo que se acaba de decir es de ese tema?** Para que dos peticiones
    /// a medias no cuenten dos veces lo mismo: la segunda tiene que traer
    /// algo nuevo del tema.
    pub fn en_lo_ultimo(tema: &str) -> Pregunta {
        Pregunta::noul(
            &format!("Is `conversacion.ahora` about the agenda topic «{tema}»?"),
            "What was just said adds something about it: a comment, a figure, a question, an answer or a decision about it, e.g. for «presupuesto 2027»: «y en personal nos quedamos en doce mil».",
            "`conversacion.ahora` is about something else, or only names it to leave it for later, e.g. «del presupuesto hablamos luego»; what was said only in `conversacion.antes` does not count.",
        )
    }

    /// Las tres preguntas de cada tema, con su nombre.
    pub fn preguntas(temas: &[(&str, &str)]) -> Vec<(String, Pregunta)> {
        temas
            .iter()
            .flat_map(|(id, texto)| {
                [
                    (tratado(id), se_trato(texto)),
                    (cerrado(id), quedo_cerrado(texto)),
                    (ahora(id), en_lo_ultimo(texto)),
                ]
            })
            .collect()
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

/// **Lo que se dice en vivo, frase a frase** (29-oct-2026, los prototipos de
/// `docs/probar-ideas-sin-interfaz.md`): **una sola peticion por frase** con
/// las preguntas de varias ideas, porque todas comparten el estado (ver
/// `crate::vivo`). Estado: `reunion` (el titulo), `frase.quien` y
/// `frase.dice` (quien habla y lo que acaba de decir), `antes` (las dos
/// frases anteriores, «Quien: lo que dijo»; si no hay, no va) y, solo si
/// van sus preguntas, `participantes` (los nombres) y `agenda` (los temas).
///
/// Lo que va dentro de cada pregunta (una tarea, una pregunta abierta, las
/// expresiones de fecha, los chats) va en su texto o en sus opciones: cada
/// cosa, su pregunta.
pub mod vivo {
    use super::*;

    pub const COMPROMISO: &str = "compromiso";
    pub const QUIEN_DICHO: &str = "quien_dicho";
    pub const QUIEN: &str = "quien";
    pub const PLAZO_DICHO: &str = "plazo_dicho";
    pub const PLAZO: &str = "plazo";
    pub const URGENTE: &str = "urgente";
    pub const DECISION: &str = "decision";
    pub const PREGUNTA: &str = "pregunta";
    pub const CITA: &str = "cita";
    pub const CITA_CUANDO: &str = "cita_cuando";
    pub const CIERRE: &str = "cierre";
    pub const DE_LA_AGENDA: &str = "de_la_agenda";
    pub const TEMA: &str = "tema";
    pub const DICTADO: &str = "dictado";
    pub const DICTADO_CHAT_DICHO: &str = "dictado_chat_dicho";
    pub const DICTADO_CHAT: &str = "dictado_chat";
    pub const DICTADO_QUE: &str = "dictado_que";

    /// La opcion de [`QUIEN`] para «todos».
    pub const TODOS: &str = "todos";
    /// Las opciones de [`DICTADO_QUE`].
    pub const LO_QUE_SIGUE: &str = "lo_que_sigue_en_la_frase";
    pub const LA_ANTERIOR: &str = "la_frase_anterior";

    /// Cuantas tareas y cuantas preguntas abiertas van en una peticion.
    pub const POR_PETICION: usize = 8;

    /// «¿La frase es de la tarea `id`?»
    pub fn sobre(id: &str) -> String {
        format!("sobre_{id}")
    }

    /// «¿La apura?»
    pub fn apura(id: &str) -> String {
        format!("apura_{id}")
    }

    /// «¿Contesta la pregunta abierta `id`?»
    pub fn respondida(id: &str) -> String {
        format!("respondida_{id}")
    }

    pub fn compromiso() -> Pregunta {
        Pregunta::noul(
            "Does `frase.dice` contain an action item: something concrete that has to be done after this moment?",
            "A job is taken on, given or stated as needed, with or without a person, e.g. «yo me encargo de mandar la cotización el viernes», «Luis, tú llamas al proveedor» or «hay que pedir los permisos este mes».",
            "It describes, discusses or reports something and no job comes out of it, e.g. «la cotización llegó ayer» or an idea still open, «podríamos pedir otra».",
        )
    }

    pub fn quien_dicho() -> Pregunta {
        Pregunta::noul(
            "Does `frase.dice` say who will do it?",
            "A person or group is named, or the speaker takes it on, e.g. «yo me encargo», «Marta lo llama», «lo hacemos todos».",
            "Nobody is named, e.g. «hay que mandar la cotización».",
        )
    }

    /// `opciones`: (clave, nombre) de cada participante; `habla`, el nombre
    /// de quien habla.
    pub fn quien(opciones: &[(String, String)], habla: &str) -> Pregunta {
        let mut v: Vec<(String, String)> = opciones
            .iter()
            .map(|(k, n)| {
                let d = if n == habla {
                    format!(
                        "«{n}», the person speaking in `frase.quien`, will do it (e.g. «yo me encargo»)."
                    )
                } else {
                    format!("«{n}» will do it.")
                };
                (k.clone(), d)
            })
            .collect();
        v.push((
            TODOS.to_string(),
            "Everyone in the meeting will do it together.".to_string(),
        ));
        Pregunta::Choice {
            instructions: "Who will do what `frase.dice` commits to?".to_string(),
            criteria: v,
        }
    }

    pub fn plazo_dicho() -> Pregunta {
        Pregunta::noul(
            "Does `frase.dice` give a day or time by which something has to be done?",
            "A deadline is set, e.g. «la mando el viernes», «esto es para hoy», «para mañana a las 10».",
            "No deadline is set: a day only names a topic or a past event, e.g. «la cotización del viernes» or «el material llegó el lunes».",
        )
    }

    /// `opciones`: (clave, palabras) de cada expresion de fecha de la frase.
    pub fn plazo(opciones: &[(String, String)]) -> Pregunta {
        Pregunta::Choice {
            instructions: "Which words in `frase.dice` say by when it has to be done?".to_string(),
            criteria: opciones
                .iter()
                .map(|(k, t)| (k.clone(), format!("«{t}»")))
                .collect(),
        }
    }

    pub fn urgente() -> Pregunta {
        Pregunta::noul(
            "Does `frase.dice` stress that something is urgent?",
            "Hurry is expressed, e.g. «es urgente», «cuanto antes», «sin falta hoy».",
            "No hurry is expressed, e.g. «cuando puedas» or only a plain date.",
        )
    }

    pub fn decision() -> Pregunta {
        Pregunta::noul(
            "Does `frase.dice` announce a decision that the group has made?",
            "Something is settled from now on, e.g. «entonces se decidió: compramos los proyectores en enero» or «queda aprobado».",
            "Options are only discussed or proposed, e.g. «podríamos comprarlos en enero» or «yo propondría otra imprenta».",
        )
    }

    pub fn pregunta() -> Pregunta {
        Pregunta::noul(
            "Does `frase.dice` ask a question that expects an answer from someone else in the meeting?",
            "A real question to the others, e.g. «¿alguien sabe cuándo llega el material?».",
            "There is no question, or the speaker asks and answers at once, e.g. «¿y por qué? Porque…», or it is rhetorical.",
        )
    }

    /// La pregunta abierta va dentro.
    pub fn contesta(pregunta: &str) -> Pregunta {
        Pregunta::noul(
            &format!("Does `frase.dice` answer the question «{pregunta}»?"),
            "It gives what was asked, also a plain yes or no, e.g. for «¿cuándo llega el material?»: «llega el jueves».",
            "It is about something else, or only puts the answer off, e.g. «lo averiguo y te digo».",
        )
    }

    pub fn cita() -> Pregunta {
        Pregunta::noul(
            "Does `frase.dice` set a day or time for people to meet or for an event to take place?",
            "A meeting, visit, class or event gets a moment, e.g. «nos vemos el martes a las 3» or «la visita a la obra es el jueves a las 9».",
            "A day is only a deadline for a task or names a past event, e.g. «mándalo el viernes» or «el material llegó el lunes».",
        )
    }

    pub fn cita_cuando(opciones: &[(String, String)]) -> Pregunta {
        Pregunta::Choice {
            instructions: "Which words in `frase.dice` say when they meet or when the event is?"
                .to_string(),
            criteria: opciones
                .iter()
                .map(|(k, t)| (k.clone(), format!("«{t}»")))
                .collect(),
        }
    }

    pub fn cierre() -> Pregunta {
        Pregunta::noul(
            "Is the speaker wrapping up the meeting in `frase.dice`?",
            "The meeting is being closed, e.g. «bueno, si no hay nada más, lo dejamos aquí» or «vamos cerrando».",
            "The discussion goes on, e.g. a new point is opened or someone is still explaining.",
        )
    }

    pub fn de_la_agenda() -> Pregunta {
        Pregunta::noul(
            "Is `frase.dice` about one of the topics in `agenda`?",
            "It talks about one of them: a figure, an opinion, a question or a decision about it, also through «eso» or «esto» pointing to `antes`.",
            "It is about something else: greetings, jokes, the room or the call, or a matter that is not in `agenda`; or a topic is only named to leave it for later, e.g. «eso lo vemos al final».",
        )
    }

    /// `opciones`: (clave, tema) de cada tema de la agenda.
    /// `con_anterior`: si el estado lleva `tema_anterior`.
    pub fn tema(opciones: &[(String, String)], con_anterior: bool) -> Pregunta {
        let instrucciones = if con_anterior {
            "Which topic in `agenda` is being discussed in `frase.dice`? `antes` is what was said just before, and it was about `tema_anterior`."
        } else {
            "Which topic in `agenda` is being discussed in `frase.dice`, given what was said just before in `antes`?"
        };
        Pregunta::Choice {
            instructions: instrucciones.to_string(),
            criteria: opciones
                .iter()
                .map(|(k, t)| (k.clone(), format!("The topic «{t}».")))
                .collect(),
        }
    }

    pub fn dictado() -> Pregunta {
        Pregunta::noul(
            "Does the speaker in `frase.dice` ask for something to be written down?",
            "They ask to note, write or post something, with or without saying where, e.g. «anota esto en el chat de la obra», «apúntalo» or «pon en el grupo de compras que faltan tornillos».",
            "Nothing is to be written down; a chat may only be mentioned, e.g. «lo vi en el chat de la obra».",
        )
    }

    pub fn dictado_chat_dicho() -> Pregunta {
        Pregunta::noul(
            "Does `frase.dice` say in which chat to note it?",
            "A chat is named, e.g. «en el chat de la obra» or «en el grupo de compras».",
            "No chat is named, e.g. «anótalo».",
        )
    }

    /// `opciones`: (clave, nombre) de cada chat.
    pub fn dictado_chat(opciones: &[(String, String)]) -> Pregunta {
        Pregunta::Choice {
            instructions: "In which chat does the speaker want it noted?".to_string(),
            criteria: opciones
                .iter()
                .map(|(k, n)| (k.clone(), format!("The chat «{n}».")))
                .collect(),
        }
    }

    /// `opciones`: (clave, texto) de lo que se podria anotar.
    pub fn dictado_que(opciones: &[(String, String)]) -> Pregunta {
        Pregunta::Choice {
            instructions: "What does the speaker want noted?".to_string(),
            criteria: opciones
                .iter()
                .map(|(k, t)| (k.clone(), format!("«{t}»")))
                .collect(),
        }
    }

    /// La tarea va dentro.
    pub fn es_de_la_tarea(tarea: &str) -> Pregunta {
        Pregunta::noul(
            &format!("Is `frase.dice` about the task «{tarea}»?"),
            "It talks about doing it, its date or its state, also through «esto» or «eso» pointing to it in `antes`.",
            "It is about other things.",
        )
    }

    pub fn la_apura(tarea: &str) -> Pregunta {
        Pregunta::noul(
            &format!(
                "Does `frase.dice` ask for the task «{tarea}» to be done sooner or say it is more urgent?"
            ),
            "e.g. «esto es para hoy», «lo necesito ya», «hay que adelantarlo al lunes».",
            "No hurry is added for it, or it is given more time, e.g. «eso puede esperar».",
        )
    }
}

/// **Repaso oral**: lo que explica un alumno, contra los puntos clave de la
/// clase. Estado: `clase` (el titulo) y `explicacion` (lo dicho hasta
/// ahora). Dos Nouls por punto, con el punto dentro.
pub mod repaso {
    use super::*;

    pub fn cubre(id: &str) -> String {
        format!("cubre_{id}")
    }

    pub fn contradice(id: &str) -> String {
        format!("contradice_{id}")
    }

    /// Cuantos puntos van en una peticion.
    pub const POR_PETICION: usize = 12;

    pub fn explica(punto: &str) -> Pregunta {
        Pregunta::noul(
            &format!("Does `explicacion` explain the key point «{punto}»?"),
            "The student states the idea in their own words, e.g. for «las plantas producen oxígeno»: «al hacer la fotosíntesis la planta suelta oxígeno».",
            "The point is missing or only named, e.g. «y también está lo del oxígeno».",
        )
    }

    pub fn lo_contradice(punto: &str) -> Pregunta {
        Pregunta::noul(
            &format!("Does `explicacion` say something that contradicts the key point «{punto}»?"),
            "Something said is wrong about it, e.g. for «las plantas producen oxígeno»: «las plantas producen dióxido de carbono en la fotosíntesis».",
            "What is said about it agrees with it, or it is not mentioned.",
        )
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
        // Los temas: tres por tema, cada una con el tema dentro y nombrando
        // `conversacion` (la de lo ultimo, `conversacion.ahora`).
        let t = temas::preguntas(&[("t1", "Presupuesto"), ("t2", "Viaje")]);
        assert_eq!(t.len(), 6);
        for (n, p) in &t {
            let texto = serde_json::to_string(p).unwrap();
            assert!(texto.contains("`conversacion"), "{n}");
            assert!(texto.contains("Presupuesto") || texto.contains("Viaje"));
        }
        assert_eq!(t[0].0, "tratado_t1");
        assert_eq!(t[1].0, "cerrado_t1");
        assert_eq!(t[2].0, "ahora_t1");
        let ahora = serde_json::to_string(&t[2].1).unwrap();
        assert!(ahora.contains("`conversacion.ahora`"));
        // Sin mensajes recientes, no se pregunta por ellos.
        assert!(
            !urgencia::preguntas(false)
                .iter()
                .any(|(n, _)| *n == urgencia::PIDEN_YA)
        );
    }
}
