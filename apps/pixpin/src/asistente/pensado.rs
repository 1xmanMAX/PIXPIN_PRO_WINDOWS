//! **Lo que penso el asistente, guardado con su respuesta**, y lo que hace
//! falta para empezar un turno con memoria (pedido del usuario, 13-oct-2026:
//! ver lo que piensa, la peticion mejorada, la hoja de trabajo y el
//! reinicio del contexto; la logica esta en `pixpin_ia::{pensar,
//! hoja_trabajo, contexto}`).
//!
//! ## Donde se guarda
//!
//! Cada respuesta del asistente es una nota normal del cuaderno; lo que penso
//! al escribirla, cuanto, la peticion mejorada y la hoja de trabajo tal como
//! quedo van en `pins/draw/anot-<codigo>.asistente.json` con el codigo unico
//! de esa nota, como los comentarios de una nota (`comentarios_de_notas`):
//! asi **viajan con su nota** cuando el chat se sincroniza y se borran con
//! ella, sin nada nuevo en la sincronizacion. La hoja del chat es la del
//! registro de la ultima respuesta que lo tenga. Si esta rota, se mira la
//! anterior, y si no hay ninguna se empieza una hoja nueva.
//!
//! Lo que se lee se guarda en memoria por id de nota (tambien que no hay
//! nada): pintar la burbuja no va al disco en cada fotograma.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use pixpin_ia::contexto::{self, Fuente};
use pixpin_ia::herramientas::Herramienta;
use pixpin_ia::hoja_trabajo::{HojaDeTrabajo, Registro};
use pixpin_ia::motor::Mensaje;
use pixpin_ia::pensar::Plan;
use pixpin_proyecto::cuaderno::Mensaje as Nota;
use pixpin_sincro::anotado as an;
use pixpin_sincro::disco::Disco;

use super::chat::{EnVivo, es_del_asistente};
use super::turno::Memoria;

/// Lo que se pinta encima del texto de una respuesta.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Seccion {
    pub pensamiento: String,
    /// Cuanto penso; `None` mientras piensa.
    pub segundos: Option<f32>,
    /// Pensando ahora mismo (la seccion va abierta y crece).
    pub pensando: bool,
    pub plan: Option<Plan>,
    /// Este turno empezo con el contexto renovado.
    pub reinicio: bool,
}

impl Seccion {
    pub fn de_vivo(v: &EnVivo) -> Seccion {
        Seccion {
            pensamiento: v.pensamiento.clone(),
            segundos: v.segundos,
            pensando: v.pensando,
            plan: v.plan.clone(),
            reinicio: v.reiniciado,
        }
    }

    pub fn de_registro(r: &Registro) -> Seccion {
        Seccion {
            pensamiento: r.pensamiento.clone(),
            segundos: r.segundos,
            pensando: false,
            plan: r.plan.clone(),
            reinicio: r.reinicio,
        }
    }

    /// Si hay algo que ensenar.
    pub fn algo(&self) -> bool {
        !self.pensamiento.trim().is_empty() || self.plan.is_some() || self.reinicio
    }

    pub fn penso(&self) -> bool {
        !self.pensamiento.trim().is_empty()
    }
}

thread_local! {
    /// Los registros leidos, por id de nota (`None`: no hay o esta roto).
    static REGISTROS: RefCell<HashMap<String, Option<Rc<Registro>>>> = RefCell::new(HashMap::new());
    /// Lo de los turnos en marcha, por id de nota.
    static EN_VIVO: RefCell<HashMap<String, Seccion>> = RefCell::new(HashMap::new());
}

/// Donde va el registro de una respuesta.
pub fn ruta_del_registro(raiz: &Path, ficha: &str, codigo_unico: &str) -> PathBuf {
    let rel = an::rel(&format!("{}{codigo_unico}", an::PREFIJO), an::ASISTENTE);
    let chat =
        pixpin_proyecto::vista::chat_de_ficha(raiz, ficha).unwrap_or_else(|| ficha.to_string());
    pixpin_proyecto::vista::DiscoPc::nuevo(raiz).ruta(&chat, &rel)
}

/// El registro de una nota, del disco la primera vez.
fn registro_de(raiz: &Path, ficha: &str, nota: &Nota) -> Option<Rc<Registro>> {
    if let Some(r) = REGISTROS.with_borrow(|c| c.get(&nota.id).cloned()) {
        return r;
    }
    let ruta = ruta_del_registro(raiz, ficha, &nota.codigo_unico());
    let r = match Registro::leer(&ruta) {
        Ok(r) => r.map(Rc::new),
        Err(e) => {
            tracing::warn!(%e, ruta = %ruta.display(), "asistente: registro roto, se ignora");
            None
        }
    };
    REGISTROS.with_borrow_mut(|c| c.insert(nota.id.clone(), r.clone()));
    r
}

/// **Lo que se pinta encima de una respuesta**, en vivo o guardado.
pub fn seccion_de(raiz: &Path, ficha: &str, nota: &Nota) -> Option<Seccion> {
    if !es_del_asistente(nota) {
        return None;
    }
    if let Some(s) = EN_VIVO.with_borrow(|v| v.get(&nota.id).cloned()) {
        return s.algo().then_some(s);
    }
    let r = registro_de(raiz, ficha, nota)?;
    let s = Seccion::de_registro(&r);
    s.algo().then_some(s)
}

/// Lo de un turno en marcha. Devuelve si cambio (hay que volver a medir).
pub fn poner_en_vivo(nota: &str, s: Seccion) -> bool {
    EN_VIVO.with_borrow_mut(|v| {
        if v.get(nota) == Some(&s) {
            return false;
        }
        v.insert(nota.to_string(), s);
        true
    })
}

/// **Al acabar el turno**: el registro al disco y a la memoria.
pub fn guardar(raiz: &Path, ficha: &str, nota_id: &str, codigo_unico: &str, r: Registro) {
    let ruta = ruta_del_registro(raiz, ficha, codigo_unico);
    if let Err(e) = r.escribir(&ruta) {
        tracing::warn!(?e, "asistente: no se pudo guardar lo que penso");
    }
    REGISTROS.with_borrow_mut(|c| c.insert(nota_id.to_string(), Some(Rc::new(r))));
    EN_VIVO.with_borrow_mut(|v| v.remove(nota_id));
}

/// Un turno que acabo sin registro (fallo antes de empezar): fuera lo vivo.
pub fn olvidar_en_vivo(nota_id: &str) {
    EN_VIVO.with_borrow_mut(|v| v.remove(nota_id));
}

/// Cuantas respuestas hacia atras se mira para encontrar la hoja.
const BUSCAR_HOJA_EN: usize = 30;

/// **La hoja del chat**: la del registro de la ultima respuesta que lo tenga.
pub fn hoja_del_chat(raiz: &Path, ficha: &str, notas: &[Nota]) -> HojaDeTrabajo {
    notas
        .iter()
        .rev()
        .filter(|n| es_del_asistente(n))
        .take(BUSCAR_HOJA_EN)
        .find_map(|n| registro_de(raiz, ficha, n))
        .map(|r| r.hoja.clone())
        .unwrap_or_default()
}

/// **Donde busca `buscar_historial`**: las notas del chat y lo pensado en
/// cada turno de la hoja.
pub fn fuentes(notas: &[Nota], hoja: &HojaDeTrabajo) -> Vec<Fuente> {
    let mut v: Vec<Fuente> = super::chat::conversacion_con_ids(notas)
        .into_iter()
        .map(|(_, n, del_usuario, texto)| Fuente {
            etiqueta: format!("#{n} {}", if del_usuario { "usuario" } else { "asistente" }),
            texto,
        })
        .collect();
    for t in &hoja.turnos {
        if !t.pensado.is_empty() {
            v.push(Fuente {
                etiqueta: format!("T{} pensado", t.n),
                texto: format!("{} — {}", t.peticion, t.pensado),
            });
        }
    }
    v
}

/// **Si piensa este turno**, el modelo local o la nube: lo de los ajustes
/// («Siempre», «Nunca») o, en automatico, solo si la tarea es compleja
/// (`pixpin_ia::complejidad`, 15-oct: «que este apagado por defecto pero
/// que piense en tareas complejas»). Medido el 14-oct con el 2B en este PC:
/// pensar le costaba 100-240 s mas por turno, y en la nube son segundos y
/// tokens de pago: para dibujar una figura no compensa.
pub fn pensar(ajuste: Option<bool>, pedido: &str) -> bool {
    let si = pixpin_ia::complejidad::decidir(ajuste, pedido);
    if ajuste.is_none() {
        tracing::debug!(si, motivo = ?pixpin_ia::complejidad::clasificar(pedido), "asistente: pensar en automatico");
    }
    si
}

/// La ventana que se mide con Claude o DeepSeek. Les caben mucho mas
/// (200k y 128k), pero cada token se paga en cada vuelta: reiniciar al 70 %
/// de esto deja cada peticion por debajo de ~45k tokens.
pub const VENTANA_NUBE: u32 = 64_000;

/// La ventana del motor local (`servidor::CONTEXTO`, la que se le pide a
/// `llama-server`).
pub fn ventana_local() -> u32 {
    pixpin_ia::servidor::CONTEXTO
}

/// Cuantos mensajes de antes se mandan como mucho, y cuanto de cada uno
/// (los de `turno::historial_para_el_modelo`), con el modelo local (no se
/// toca: la IA local esta congelada).
const ULTIMOS: usize = 12;
const LARGO: usize = 1500;

/// **Lo mismo con la nube** (17-oct-2026, el usuario: «que reciba mas
/// contexto»). Con 12 mensajes de 1500 letras una respuesta larga de Claude
/// (una lista de 20 archivos) llegaba cortada al turno siguiente y lo de
/// hace 7 preguntas ya no estaba. Medido con `contexto::estimar` (prueba
/// `la_nube_recibe_mas_contexto_sin_reiniciar`, 17-oct): el catalogo
/// entero son 14 821 tokens; con 40 mensajes normales (~600 letras) y una
/// lista larga, la peticion entera son 23 928 (el umbral es 44 800, el 70 %
/// de 64 000); el peor caso, 40 de 4000 letras, pasa y se reinicia como
/// siempre (con la hoja de trabajo), que es lo que debe pasar.
///
/// **Sin tope de mensajes desde el 22-oct-2026** (el usuario: «que cada
/// conversacion no se limite a 2 o 3 mensajes sino que todo lo de esa
/// conversacion»): va todo lo de despues de la ultima «Conversación nueva»
/// y lo unico que corta es la ventana (al 70 % se reinicia con la hoja de
/// trabajo, que resume lo de antes).
pub const ULTIMOS_NUBE: usize = usize::MAX;
pub const LARGO_NUBE: usize = 4000;

/// Los topes de historial para esta ventana: los de la nube con su ventana,
/// los de siempre con la del local.
pub fn topes(ventana: u32) -> (usize, usize) {
    if ventana >= VENTANA_NUBE {
        (ULTIMOS_NUBE, LARGO_NUBE)
    } else {
        (ULTIMOS, LARGO)
    }
}

/// **El historial y la memoria de un turno nuevo.** `anteriores`: las notas
/// del chat antes de la pregunta; `herramientas`: las de este turno (las
/// que se mandan, y por eso cuentan en la ventana).
pub fn preparar_turno(
    raiz: &Path,
    ficha: &str,
    anteriores: &[Nota],
    pregunta: &str,
    pensar: bool,
    ventana: u32,
    herramientas: &[Herramienta],
) -> (Vec<Mensaje>, Memoria) {
    let mut hoja = hoja_del_chat(raiz, ficha, anteriores);
    let fuentes = fuentes(anteriores, &hoja);
    // La memoria de largo plazo (22-oct), solo con la nube: la IA local
    // esta congelada y su ventana no da para mas.
    let largo_plazo = (ventana >= VENTANA_NUBE)
        .then(|| super::memoria::de(raiz, &quien_escribe(raiz)))
        .flatten();
    let bloque = super::memoria::bloque(largo_plazo.as_ref());
    let p = preparar_con_memoria(
        &mut hoja,
        anteriores,
        pregunta,
        ventana,
        herramientas,
        largo_plazo.as_ref().map(|_| bloque.as_str()),
    );
    let memoria = Memoria {
        hoja,
        fuentes,
        pensar,
        planificar: true,
        ventana,
        reiniciado: p.reiniciado,
        ampliar: true,
        // El camino rapido (14-oct).
        rapido: true,
        largo_plazo,
        a_guardar: p.olvidado,
    };
    (p.historial, memoria)
}

/// Quien firma lo que se guarda en la memoria: el modelo elegido.
fn quien_escribe(raiz: &Path) -> String {
    match super::nube::eleccion_reciente(raiz) {
        super::nube::Eleccion::Nube(p) => p.nombre().to_string(),
        super::nube::Eleccion::Local => "modelo local".into(),
    }
}

/// Lo de [`preparar_turno`] sin disco, para probarlo: desde el ultimo
/// reinicio, y si pasa del 70 % se reinicia (y la hoja lo apunta).
#[cfg_attr(not(test), allow(dead_code))]
pub fn preparar_con_hoja(
    hoja: &mut HojaDeTrabajo,
    anteriores: &[Nota],
    pregunta: &str,
    ventana: u32,
    herramientas: &[Herramienta],
) -> (Vec<Mensaje>, bool) {
    let p = preparar_con_memoria(hoja, anteriores, pregunta, ventana, herramientas, None);
    (p.historial, p.reiniciado)
}

/// Lo que deja [`preparar_con_memoria`].
#[derive(Debug, Clone, PartialEq)]
pub struct Preparado {
    pub historial: Vec<Mensaje>,
    pub reiniciado: bool,
    /// Si se reinicio: lo que se deja de mandar (`(es_del_usuario, texto)`),
    /// para guardar antes lo duradero en la memoria (el 2.º escalon).
    pub olvidado: Vec<(bool, String)>,
}

/// **Lo mismo con la memoria de largo plazo** (`bloque`: el perfil y el
/// indice, `memoria::bloque`; `None`: sin memoria, ni su herramienta) en el
/// sistema, detras de las instrucciones y antes de la hoja: cuenta en la
/// ventana como lo demas. Tras «Conversación
/// nueva» el turno empieza con solo esto: instrucciones, memoria y pregunta.
pub fn preparar_con_memoria(
    hoja: &mut HojaDeTrabajo,
    anteriores: &[Nota],
    pregunta: &str,
    ventana: u32,
    herramientas: &[Herramienta],
    bloque: Option<&str>,
) -> Preparado {
    let mut conv = super::chat::conversacion_con_ids(anteriores);
    if let Some(desde) = hoja.reinicio_desde.as_deref()
        && let Some(i) = conv.iter().position(|(id, _, _, _)| id == desde)
    {
        conv.drain(..i);
    }
    let (ultimos, largo) = topes(ventana);
    let desde = conv.len().saturating_sub(ultimos);
    let conv = &conv[desde..];
    let pares: Vec<(bool, String)> = conv
        .iter()
        .map(|(_, _, u, t)| (*u, t.chars().take(largo).collect()))
        .collect();
    let base = pixpin_ia::memoria::con_memoria(
        &pixpin_ia::agente::instrucciones(),
        bloque.unwrap_or_default(),
    );
    let sistema = contexto::sistema_con_hoja(&base, hoja, None);
    let mut herramientas = herramientas.to_vec();
    herramientas.push(contexto::herramienta_buscar());
    if bloque.is_some() {
        herramientas.push(pixpin_ia::memoria::herramienta());
    }
    let p = contexto::preparar(
        sistema,
        &pares,
        Mensaje::usuario(pregunta),
        &herramientas,
        ventana,
        contexto::RESERVA_RESPUESTA,
    );
    let mut olvidado = Vec::new();
    if p.reiniciado {
        hoja.reinicio_desde = conv.get(p.primero).map(|(id, _, _, _)| id.clone());
        hoja.reinicios += 1;
        olvidado = pares[..p.primero.min(pares.len())].to_vec();
    }
    Preparado {
        historial: p.historial,
        reiniciado: p.reiniciado,
        olvidado,
    }
}

#[cfg(test)]
pub fn olvidar_para_pruebas() {
    REGISTROS.with_borrow_mut(HashMap::clear);
    EN_VIVO.with_borrow_mut(HashMap::clear);
}
