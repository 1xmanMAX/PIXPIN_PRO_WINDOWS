//! **Lo que se oye en la reunion, en la app** (29-oct-2026, el usuario:
//! «prototipar las ideas de Jev sin interfaz y luego implementar la misma
//! logica en la app»; `docs/probar-ideas-sin-interfaz.md`, «Cómo se lleva a
//! la app»).
//!
//! La escucha en vivo de «Grabar clase» (`ventana_chat::grabar_clase`) ya
//! corta frases y las pasa a texto para los temas; con alguna de estas
//! ideas encendida (`[jev]`), cada frase va ademas a
//! `pixpin_typesafe::en_la_reunion` en **una sola peticion** (la misma
//! mesa que `examples/reunion_completa.rs`). Aqui lo que es de la app:
//!
//! - el cliente y que ideas van ([`ideas`]), o por que no hay;
//! - las tareas que ya existen, con lo que Tareas ya sabe de su urgencia
//!   (`jev::urgencia`), para la urgencia en vivo ([`tareas_de_partida`]);
//! - **aceptar** una propuesta: una tarea al Inbox de Tareas con su
//!   urgencia, su fecha (`⏰`) y una nota con el responsable y la reunion
//!   ([`apuntar_tarea`]); un evento, una tarea con su hora, que sale en la
//!   vista «Horario» de Tareas y suena como recordatorio ([`apuntar_evento`]);
//! - **la urgencia**, lo unico que se aplica solo: la fecha nueva en el
//!   texto de la tarea y lo que la apura en lo que sabe Tareas, con su
//!   «Deshacer» ([`aplicar_cambio`], [`deshacer`]).
//!
//! Sin clave, con todo apagado o si Jev falla, nada de esto aparece y la
//! grabacion sigue como siempre.

use std::path::Path;
use std::time::Duration;

use pixpin_proyecto::acta_viva::Propuesta;
use pixpin_proyecto::mini;
use pixpin_store::Ubicacion;
use pixpin_typesafe::Cliente;
use pixpin_typesafe::en_la_reunion::Ideas;
use pixpin_typesafe::fechas::{Fecha, Hora};
use pixpin_typesafe::urgencia::{Prioridad, Senales};
use pixpin_typesafe::urgencia_viva::TareaViva;

use super::Funcion;
use super::temas::SinJev;
use crate::pedidos::{self, Fallo};
use crate::tareas::frase;

/// Lo que espera Jev a la peticion de una frase: lleva hasta veintitantas
/// preguntas, pero mientras esta en vuelo no sale la siguiente.
const TIEMPO: Duration = Duration::from_secs(3);

/// Cuantas tareas que ya existen van en cada peticion (lo que cabe en
/// `preguntas::vivo::POR_PETICION`).
const TAREAS: usize = 8;

/// La hora de una tarea con fecha y sin hora dicha (la de las tareas).
const HORA_POR_OMISION: u32 = pixpin_ia::fechas::HORA_POR_OMISION;

/// **El cliente y las ideas encendidas**, o por que no hay.
pub fn ideas(u: &Ubicacion) -> Result<(Cliente, Ideas), SinJev> {
    let encendidas = |a: &pixpin_store::ajustes::Jev| Ideas {
        tareas: Funcion::TareasDichas.encendida_en(a),
        urgencia: Funcion::UrgenciaEnVivo.encendida_en(a),
        decisiones: Funcion::Decisiones.encendida_en(a),
        preguntas: Funcion::PreguntasPendientes.encendida_en(a),
        citas: Funcion::FechasAlHorario.encendida_en(a),
        tiempo_por_tema: Funcion::TiempoPorTema.encendida_en(a),
        tema_olvidado: Funcion::TemaOlvidado.encendida_en(a),
    };
    if cfg!(test) || !super::hay_clave() {
        return Err(SinJev::SinClave);
    }
    let a = pixpin_store::cargar(u).unwrap_or_default();
    let ideas = encendidas(&a.jev);
    if !ideas.alguna() {
        return Err(SinJev::Apagado);
    }
    // `para` mira la clave y su interruptor: con una idea cualquiera vale.
    let f = [
        Funcion::TareasDichas,
        Funcion::UrgenciaEnVivo,
        Funcion::Decisiones,
        Funcion::PreguntasPendientes,
        Funcion::FechasAlHorario,
        Funcion::TiempoPorTema,
        Funcion::TemaOlvidado,
    ]
    .into_iter()
    .find(|f| f.encendida_en(&a.jev))
    .ok_or(SinJev::Apagado)?;
    let cliente = super::para(u, f).ok_or(SinJev::SinClave)?;
    Ok((cliente.con_tiempo(TIEMPO), ideas))
}

/// El dia de hoy, en hora local.
pub fn hoy() -> Fecha {
    Fecha::de_dias(pixpin_shell::entorno::ahora_local_ms().div_euclid(86_400_000))
}

/// **Donde vive una tarea** de la reunion (la que existia o la apuntada al
/// aceptar una propuesta): para cambiarla y deshacerlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TareaEnLista {
    pub proyecto: String,
    pub codigo: String,
    /// El titulo de su lista (la clave de lo que sabe `jev::urgencia`).
    pub lista: String,
    /// El texto tal cual esta guardado (con su fecha de creada).
    pub crudo: String,
}

impl TareaEnLista {
    /// Lo que se ve: sin la fecha de creada ni las imagenes.
    fn visible(&self) -> String {
        let (v, _) = mini::partir(&self.crudo);
        mini::imagenes_de(v).0
    }
}

/// La fecha de una tarea (la de su `⏰`).
fn fecha_de_hora(visible: &str) -> Option<Fecha> {
    crate::tareas::hora_de_tarea(visible).map(|ms| Fecha::de_dias(ms.div_euclid(86_400_000)))
}

/// **Las tareas pendientes que pueden cambiar de urgencia en la reunion**:
/// las del chat de la reunion primero y luego las del Inbox, menos la
/// lista de los temas (eso es la agenda). Cada una con lo que Tareas ya
/// sabe de su urgencia (sin eso, su prioridad de partida es «normal»).
pub fn tareas_de_partida(
    raiz: &Path,
    proyecto: &str,
    sin_la_lista: Option<&(String, String)>,
) -> Vec<(TareaViva, TareaEnLista)> {
    let (listas, _) = crate::tareas::reunir(raiz);
    let hoy = hoy();
    let mut v: Vec<(TareaViva, TareaEnLista)> = Vec::new();
    let de_la_reunion = listas.iter().filter(|l| l.proyecto == proyecto);
    let del_inbox = listas
        .iter()
        .filter(|l| l.proyecto != proyecto && crate::tareas::es_inbox(l));
    for l in de_la_reunion.chain(del_inbox) {
        if sin_la_lista.is_some_and(|(p, c)| *p == l.proyecto && *c == l.codigo) {
            continue;
        }
        for f in l.filas.iter().filter(|f| !f.hecha) {
            if v.len() >= TAREAS {
                return v;
            }
            let m = frase::marcas(&f.texto);
            if m.titulo.trim().is_empty() {
                continue;
            }
            let senales = super::urgencia::sabidas(&l.proyecto, &l.titulo, &f.texto);
            let id = format!("t{}", v.len() + 1);
            let t = TareaViva::nueva(&id, m.titulo.trim(), fecha_de_hora(&f.texto), senales, hoy);
            v.push((
                t,
                TareaEnLista {
                    proyecto: l.proyecto.clone(),
                    codigo: l.codigo.clone(),
                    lista: l.titulo.clone(),
                    crudo: f.crudo.clone(),
                },
            ));
        }
    }
    v
}

/// La urgencia de Tareas para una prioridad de Jev: urgente → ⏫, alta →
/// 🔼; lo demas, sin marca.
fn urgencia_de(p: Option<&str>) -> frase::Urgencia {
    match p.and_then(Prioridad::de_id) {
        Some(Prioridad::Urgente) => frase::Urgencia::MuyUrgente,
        Some(Prioridad::Alta) => frase::Urgencia::Urgente,
        _ => frase::Urgencia::Normal,
    }
}

/// El momento (`⏰`) de una fecha y una hora escritas en la propuesta.
fn momento_de(fecha: Option<&str>, hora: Option<&str>) -> Option<pixpin_ia::fechas::Momento> {
    let f = Fecha::de_texto(fecha?)?;
    let minuto = hora
        .and_then(|h| {
            let (a, b) = h.split_once(':')?;
            Some(a.trim().parse::<u32>().ok()? * 60 + b.trim().parse::<u32>().ok()?)
        })
        .filter(|m| *m < 24 * 60)
        .unwrap_or(HORA_POR_OMISION);
    Some(pixpin_ia::fechas::Momento {
        dia: f.dias(),
        minuto,
    })
}

/// **El texto de la tarea** de una propuesta: lo dicho, su urgencia, la
/// nota (quien y de que reunion) y su `⏰`.
pub fn texto_de(p: &Propuesta, reunion: &str) -> String {
    let mut nota = Vec::new();
    if let Some(r) = p.responsable.as_deref().filter(|r| !r.trim().is_empty()) {
        nota.push(r.trim().to_string());
    }
    if !reunion.trim().is_empty() {
        nota.push(format!("🎙 {}", reunion.trim()));
    }
    frase::Frase {
        titulo: p.texto.trim().to_string(),
        urgencia: urgencia_de(p.prioridad.as_deref()),
        etiquetas: Vec::new(),
        hora: momento_de(p.fecha.as_deref(), p.hora.as_deref()),
        nota: (!nota.is_empty()).then(|| nota.join(" · ")),
    }
    .texto()
}

/// La ultima tarea de una lista, donde la deja `anadir_tarea`.
fn ultima_de(m: &pixpin_proyecto::cuaderno::Mensaje) -> Option<TareaEnLista> {
    let t = mini::leer_tareas(&m.texto).pop()?;
    Some(TareaEnLista {
        proyecto: String::new(),
        codigo: m.id.clone(),
        lista: pedidos::nombre_de_lista(m),
        crudo: t.texto,
    })
}

/// **Acepta una tarea propuesta**: al Inbox de Tareas. Devuelve donde quedo
/// (para su urgencia en vivo desde ahora) y su prioridad de partida.
pub fn apuntar_tarea(
    raiz: &Path,
    p: &Propuesta,
    reunion: &str,
) -> Result<(TareaEnLista, TareaViva), Fallo> {
    let aparato = crate::letras_que_viajan::aparato(raiz);
    let m = crate::tareas::apuntar(raiz, &aparato, &texto_de(p, reunion))?;
    crate::tareas::refrescar();
    let mut sitio = ultima_de(&m).ok_or(Fallo::NoEsLista)?;
    // El Inbox vive en «Mensajes guardados».
    sitio.proyecto =
        pedidos::ficha_de(raiz, None, &aparato, pixpin_shell::entorno::ahora_utc_ms())?.id;
    let hoy = hoy();
    let t = TareaViva::nueva(
        &format!("a{}", p.id),
        p.texto.trim(),
        p.fecha.as_deref().and_then(Fecha::de_texto),
        None,
        hoy,
    );
    Ok((sitio, t))
}

/// **Acepta un evento propuesto**: una tarea con su hora (de dia entero, a
/// la hora de siempre) en el Inbox, que sale en el «Horario» de Tareas.
pub fn apuntar_evento(raiz: &Path, p: &Propuesta, reunion: &str) -> Result<(), Fallo> {
    let aparato = crate::letras_que_viajan::aparato(raiz);
    crate::tareas::apuntar(raiz, &aparato, &texto_de(p, reunion))?;
    crate::tareas::refrescar();
    Ok(())
}

/// **Apunta en Tareas** las preguntas que nadie contesto, cada una con la
/// reunion de nota. Devuelve cuantas.
pub fn preguntas_a_tareas(raiz: &Path, preguntas: &[String], reunion: &str) -> usize {
    super::temas::a_tareas(raiz, preguntas, reunion)
}

/// Lo que hace falta para deshacer un cambio de urgencia.
#[derive(Debug, Clone)]
pub struct Deshacer {
    /// La tarea como quedo (para encontrarla) y como estaba.
    pub ahora: TareaEnLista,
    pub antes: TareaEnLista,
    /// Lo que Tareas sabia de su urgencia antes.
    pub senales_antes: Option<Senales>,
}

/// Cambia el texto de la tarea `t` (encontrada por su texto, no por su
/// numero: la lista pudo cambiar) por `nuevo` (lo visible). Devuelve el
/// crudo nuevo.
fn reescribir_tarea(raiz: &Path, t: &TareaEnLista, nuevo: &str) -> Result<String, Fallo> {
    let mut m = pedidos::mensaje_de(raiz, &t.proyecto, &t.codigo)?;
    let tareas = mini::leer_tareas(&m.texto);
    let i = tareas
        .iter()
        .position(|x| x.texto == t.crudo)
        .ok_or(Fallo::NoEsLista)?;
    m.texto = mini::renombrar(&m.texto, i, nuevo);
    let crudo = mini::leer_tareas(&m.texto)
        .get(i)
        .map(|x| x.texto.clone())
        .unwrap_or_default();
    pedidos::reescribir(raiz, &t.proyecto, &m)?;
    Ok(crudo)
}

/// **Aplica un cambio de urgencia** de la reunion: la fecha nueva (si
/// cambio) en el `⏰` de la tarea, con la hora que tenia o la de siempre, y
/// lo que la apura (`senales`) en lo que sabe Tareas. Devuelve donde quedo
/// y como deshacerlo.
pub fn aplicar_cambio(
    raiz: &Path,
    t: &TareaEnLista,
    fecha: Option<(Fecha, Option<Hora>)>,
    senales: Option<Senales>,
) -> Result<(TareaEnLista, Deshacer), Fallo> {
    let visible = t.visible();
    let senales_antes = super::urgencia::sabidas(&t.proyecto, &t.lista, &visible);
    let mut ahora = t.clone();
    if let Some((f, hora)) = fecha {
        let minuto = match hora {
            Some(h) => h.h * 60 + h.m,
            None => crate::tareas::hora_de_tarea(&visible)
                .map(|ms| (ms.rem_euclid(86_400_000) / 60_000) as u32)
                .unwrap_or(HORA_POR_OMISION),
        };
        let local = f.dias() * 86_400_000 + i64::from(minuto) * 60_000;
        let nuevo = crate::tareas::con_hora(mini::partir(&t.crudo).0, local);
        ahora.crudo = reescribir_tarea(raiz, t, &nuevo)?;
    }
    if senales.is_some() {
        super::urgencia::poner_sabidas(&ahora.proyecto, &ahora.lista, &ahora.visible(), senales);
    }
    crate::tareas::refrescar();
    Ok((
        ahora.clone(),
        Deshacer {
            ahora,
            antes: t.clone(),
            senales_antes,
        },
    ))
}

/// **«Deshacer»** de un cambio de urgencia: el texto de antes y lo que
/// Tareas sabia antes.
pub fn deshacer(raiz: &Path, d: &Deshacer) -> Result<(), Fallo> {
    if d.ahora.crudo != d.antes.crudo {
        reescribir_tarea(raiz, &d.ahora, mini::partir(&d.antes.crudo).0)?;
    }
    super::urgencia::poner_sabidas(
        &d.antes.proyecto,
        &d.antes.lista,
        &d.antes.visible(),
        d.senales_antes.clone(),
    );
    crate::tareas::refrescar();
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::acta_viva::{EstadoPropuesta, Tipo};

    fn propuesta() -> Propuesta {
        Propuesta {
            id: "p1".into(),
            tipo: Tipo::Tarea,
            ms: 0,
            texto: "mandar la cotización".into(),
            responsable: Some("Luis".into()),
            fecha: Some("2026-10-30".into()),
            hora: None,
            prioridad: Some("alta".into()),
            estado: EstadoPropuesta::Pendiente,
        }
    }

    #[test]
    fn sin_clave_no_hay_ideas() {
        let u = Ubicacion::Instalado {
            raiz: std::env::temp_dir().join("pixpin-jev-en-vivo-sin-clave"),
        };
        assert_eq!(ideas(&u).err(), Some(SinJev::SinClave));
    }

    #[test]
    fn el_texto_de_una_tarea_propuesta() {
        let t = texto_de(&propuesta(), "Coordinación");
        assert_eq!(
            t,
            "mandar la cotización 🔼 💬 Luis · 🎙 Coordinación ⏰ 2026-10-30 09:00"
        );
        let m = frase::marcas(&t);
        assert_eq!(m.titulo, "mandar la cotización");
        assert_eq!(m.urgencia, frase::Urgencia::Urgente);
        // Un evento con su hora, sin nadie ni urgencia.
        let e = Propuesta {
            tipo: Tipo::Evento,
            responsable: None,
            hora: Some("15:00".into()),
            prioridad: None,
            texto: "nos vemos el martes a las 3".into(),
            ..propuesta()
        };
        assert_eq!(
            texto_de(&e, ""),
            "nos vemos el martes a las 3 ⏰ 2026-10-30 15:00"
        );
        assert_eq!(urgencia_de(Some("urgente")), frase::Urgencia::MuyUrgente);
        assert_eq!(urgencia_de(Some("normal")), frase::Urgencia::Normal);
    }
}
