//! **Los «¿Sí o No?» de los turnos sin ventana** (13-oct-2026): el pedido
//! `asistente` de Flow Launcher y los pasos de las automatizaciones corren
//! en su hilo, sin el chat abierto, pero su respuesta es una burbuja del
//! chat «Asistente». Lo que borra o envia pregunta en ESA burbuja, igual que
//! en el chat (antes era un cuadro de Windows).
//!
//! La pregunta se apunta aqui (proceso entero, no del hilo de la ventana:
//! la ventana del chat vive en otro hilo) por su chat y su nota; la ventana,
//! al pintar esa burbuja, la ve y pone la tarjeta; el «Sí»/«No» vuelve por
//! el canal. Sin respuesta en [`super::chat::CADUCA_LA_PREGUNTA`], o si se
//! para, es un no.
//!
//! **El aviso flotante tambien la ve** (14-oct-2026): con el chat cerrado
//! nadie se enteraba de que el turno esperaba. Una tarjeta del aviso que
//! sigue trabajando se queda con la pregunta mas antigua sin dueno
//! ([`para_la_tarjeta`]) y la ensena con «Sí»/«No»; contestar alli es lo
//! mismo que en la burbuja ([`contestar`]).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use super::chat::Contestacion;

struct Pendiente {
    proyecto: String,
    nota: String,
    pregunta: String,
    responder: Sender<bool>,
    /// La tarjeta del aviso flotante que la ensena, si alguna.
    tarjeta: Option<u64>,
}

/// La pregunta que ensena una tarjeta: su chat, su nota y lo que pregunta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pregunta {
    pub proyecto: String,
    pub nota: String,
    pub texto: String,
}

static PENDIENTES: Mutex<Vec<Pendiente>> = Mutex::new(Vec::new());

fn con<R>(f: impl FnOnce(&mut Vec<Pendiente>) -> R) -> R {
    f(&mut PENDIENTES.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Apunta la pregunta de la nota `nota` del chat `proyecto`. Una anterior de
/// la misma nota se sustituye (y cuenta como no).
pub fn registrar(proyecto: &str, nota: &str, pregunta: &str) -> Receiver<bool> {
    let (tx, rx) = channel();
    con(|v| {
        v.retain(|p| !(p.proyecto == proyecto && p.nota == nota));
        v.push(Pendiente {
            proyecto: proyecto.into(),
            nota: nota.into(),
            pregunta: pregunta.trim().into(),
            responder: tx,
            tarjeta: None,
        });
    });
    rx
}

/// La pregunta que espera en esa burbuja, si la hay.
pub fn pregunta(proyecto: &str, nota: &str) -> Option<String> {
    con(|v| {
        v.iter()
            .find(|p| p.proyecto == proyecto && p.nota == nota)
            .map(|p| p.pregunta.clone())
    })
}

/// **La pregunta de la tarjeta `tarjeta`** del aviso flotante: la que ya
/// ensenaba o, si no tiene, la mas antigua que no ensene otra. `None` si no
/// queda ninguna (contestada en el chat, caducada).
pub fn para_la_tarjeta(tarjeta: u64) -> Option<Pregunta> {
    con(|v| {
        let i = v
            .iter()
            .position(|p| p.tarjeta == Some(tarjeta))
            .or_else(|| v.iter().position(|p| p.tarjeta.is_none()))?;
        v[i].tarjeta = Some(tarjeta);
        let p = &v[i];
        Some(Pregunta {
            proyecto: p.proyecto.clone(),
            nota: p.nota.clone(),
            texto: p.pregunta.clone(),
        })
    })
}

/// La tarjeta se fue sin contestar: su pregunta queda libre para otra.
pub fn soltar_la_tarjeta(tarjeta: u64) {
    con(|v| {
        for p in v.iter_mut().filter(|p| p.tarjeta == Some(tarjeta)) {
            p.tarjeta = None;
        }
    })
}

/// Contesta desde la tarjeta (la del aviso o la de la burbuja). `false` si ya no esperaba nadie.
pub fn contestar(proyecto: &str, nota: &str, si: bool) -> bool {
    con(|v| {
        let Some(i) = v
            .iter()
            .position(|p| p.proyecto == proyecto && p.nota == nota)
        else {
            return false;
        };
        v.remove(i).responder.send(si).is_ok()
    })
}

/// Olvida la pregunta (contestada o caducada).
pub fn quitar(proyecto: &str, nota: &str) {
    con(|v| v.retain(|p| !(p.proyecto == proyecto && p.nota == nota)));
}

/// **Espera la respuesta** sin pasar de `caduca`; `parar` la corta.
pub fn esperar(rx: &Receiver<bool>, caduca: Duration, parar: Option<&AtomicBool>) -> Contestacion {
    let hasta = Instant::now() + caduca;
    loop {
        if parar.is_some_and(|p| p.load(Ordering::Relaxed)) {
            return Contestacion::Caducada;
        }
        let queda = hasta.saturating_duration_since(Instant::now());
        if queda.is_zero() {
            return Contestacion::Caducada;
        }
        match rx.recv_timeout(queda.min(Duration::from_millis(100))) {
            Ok(true) => return Contestacion::Si,
            Ok(false) => return Contestacion::No,
            Err(RecvTimeoutError::Disconnected) => return Contestacion::Caducada,
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

/// **Una accion de una automatizacion pide permiso** (sin turno del
/// asistente ni burbuja suya): nace una nota del asistente en su chat con
/// la pregunta, se espera el «Sí»/«No» de su tarjeta y la nota queda con la
/// respuesta. `true` si dijo que si.
pub fn preguntar_en_el_chat(
    ubicacion: &pixpin_store::Ubicacion,
    aparato: &str,
    de: &str,
    pregunta: &str,
) -> bool {
    let raiz = ubicacion.raiz();
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let Ok((proyecto, carpeta)) = crate::automatizaciones::chat_del_asistente(raiz, aparato, ahora)
    else {
        return false;
    };
    let mut vivo = super::chat::EnVivo {
        texto: format!("⚙️ {de}\n"),
        ..Default::default()
    };
    vivo.preguntar(pregunta);
    let Ok(numero) = crate::ventana_chat::siguiente_numero_en(raiz, &proyecto) else {
        return false;
    };
    let Ok(nota) = crate::ventana_chat::escribir_nota(
        raiz,
        &proyecto,
        super::chat::APARATO,
        &vivo.mostrar(),
        ahora,
        numero,
        None,
    ) else {
        return false;
    };
    let rx = registrar(&proyecto, &nota.id, pregunta);
    crate::ventana_chat::refrescar();
    let c = esperar(&rx, super::chat::CADUCA_LA_PREGUNTA, None);
    quitar(&proyecto, &nota.id);
    vivo.contestar(c);
    vivo.acabado = true;
    vivo.estado = None;
    let texto = vivo.mostrar();
    let _ = pixpin_proyecto::cuaderno::cambiar(&carpeta, &nota.id, |m| {
        m.texto = texto.clone();
        true
    });
    crate::ventana_chat::refrescar();
    c == Contestacion::Si
}
#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_pregunta_se_ve_se_contesta_una_vez_y_caduca() {
        let rx = registrar("p-permiso", "n1", "¿Borro la lista?");
        assert_eq!(
            pregunta("p-permiso", "n1").as_deref(),
            Some("¿Borro la lista?")
        );
        assert_eq!(pregunta("p-permiso", "otra"), None);
        assert!(contestar("p-permiso", "n1", true));
        assert_eq!(esperar(&rx, Duration::from_secs(1), None), Contestacion::Si);
        // Ya contestada: ni se ve ni se contesta otra vez (negativo).
        assert_eq!(pregunta("p-permiso", "n1"), None);
        assert!(!contestar("p-permiso", "n1", false));
        // Sin respuesta, caduca: es un no.
        let rx = registrar("p-permiso", "n2", "¿Envio?");
        assert_eq!(
            esperar(&rx, Duration::from_millis(50), None),
            Contestacion::Caducada
        );
        quitar("p-permiso", "n2");
        assert_eq!(pregunta("p-permiso", "n2"), None);
        // Parar tambien.
        let rx = registrar("p-permiso", "n3", "¿Envio?");
        assert_eq!(
            esperar(&rx, Duration::from_secs(5), Some(&AtomicBool::new(true))),
            Contestacion::Caducada
        );
        quitar("p-permiso", "n3");
    }

    #[test]
    fn cada_tarjeta_del_aviso_se_queda_con_una_pregunta_y_la_suelta_al_irse() {
        let rx1 = registrar("p-tarjeta", "a", "¿Borro la lista?");
        let rx2 = registrar("p-tarjeta", "b", "¿Envio el correo?");
        let (t1, t2) = (9_000_001, 9_000_002);
        let a = para_la_tarjeta(t1).expect("la mas antigua");
        assert_eq!((a.proyecto.as_str(), a.nota.as_str()), ("p-tarjeta", "a"));
        assert_eq!(a.texto, "¿Borro la lista?");
        // La misma tarjeta sigue con la suya; otra coge la siguiente.
        assert_eq!(para_la_tarjeta(t1).map(|p| p.nota), Some("a".into()));
        assert_eq!(para_la_tarjeta(t2).map(|p| p.nota), Some("b".into()));
        // Contestada desde la tarjeta con un no: el turno lo recibe y ya no
        // se ensena.
        assert!(contestar("p-tarjeta", "a", false));
        assert_eq!(
            esperar(&rx1, Duration::from_secs(1), None),
            Contestacion::No
        );
        assert_eq!(pregunta("p-tarjeta", "a"), None);
        // Caso negativo: la tarjeta que se va suelta la suya, y otra la recoge.
        soltar_la_tarjeta(t2);
        assert_eq!(para_la_tarjeta(t1).map(|p| p.nota), Some("b".into()));
        // Caducada (la quita el turno): ninguna tarjeta la ensena ya, y
        // contestar tarde no hace nada.
        assert_eq!(
            esperar(&rx2, Duration::from_millis(20), None),
            Contestacion::Caducada
        );
        quitar("p-tarjeta", "b");
        assert_eq!(para_la_tarjeta(t1), None);
        assert!(!contestar("p-tarjeta", "b", true));
    }
}
