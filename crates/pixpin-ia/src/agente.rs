//! **Un turno del asistente**: pedir al modelo, validar lo que pide, hacerlo
//! y devolverle el resultado, hasta que conteste con texto.
//!
//! ```text
//! usuario ─▶ modelo ─▶ ¿llamadas? ──no──▶ texto final
//!                         │ si
//!                         ▼
//!                     validar ──mal──▶ error al modelo (reintento, max. 2)
//!                         │ bien
//!                         ▼
//!                  Ejecutor (la UI) ──▶ resultado al modelo ──▶ otra vuelta
//! ```
//!
//! El bucle no sabe dibujar ni leer PDFs: eso lo hace el [`Ejecutor`], que
//! implementa el chat con el lienzo y el OCR de verdad. Asi el mismo bucle
//! sirve para las pruebas (con un ejecutor de mentira) y, portado, para
//! Android.

use crate::ErrorIa;
use crate::contexto;
use crate::herramientas::{Herramienta, Llamada, colores_del_pedido, validar};
use crate::hoja_trabajo::HojaDeTrabajo;
use crate::motor::{Fin, Mensaje, Motor, Peticion, Respuesta, Rol, Seguir, Trozo};
use crate::pensar::{Cronometro, Plan, Separador, cortar, instrucciones_del_plan, separar};

/// Cuantas veces se deja al modelo corregir una llamada mal hecha en un
/// mismo turno. Con dos, en las medidas el 2B siempre acabo bien; mas solo
/// alarga la espera cuando el modelo no sabe hacerlo.
pub const REINTENTOS: usize = 2;

/// Cuantas vueltas modelo→herramienta→modelo como mucho en un turno (un
/// modelo pequeno puede quedarse llamando en bucle).
pub const VUELTAS: usize = 4;

/// Lo que pasa durante un turno, para pintarlo en vivo.
#[derive(Debug, Clone, PartialEq)]
pub enum Evento {
    /// Texto nuevo de la respuesta.
    Texto(String),
    /// El modelo empezo a pedir una herramienta.
    Llamando(String),
    /// Una llamada validada que se va a ejecutar, con sus avisos cosmeticos.
    Herramienta { llamada: Llamada, avisos: Vec<String> },
    /// Lo que devolvio el ejecutor.
    Resultado { nombre: String, ok: bool, texto: String },
    /// La llamada estaba mal; se le pide al modelo que la corrija.
    Reintento { motivo: String },
    /// Un trozo de lo que esta pensando el modelo (se pinta en vivo en la
    /// seccion plegable «Pensando…»).
    Pensando(String),
    /// Acabo de pensar, tras tantos segundos («Penso 12 s»).
    PensoListo { segundos: f32 },
    /// La peticion mejorada, antes de actuar.
    Peticion(Plan),
    /// Se aligero el contexto al pasar del 70 % (tokens estimados).
    Reinicio { antes: u32, despues: u32 },
}

/// **El punto de enganche de la UI**: hace de verdad cada herramienta.
pub trait Ejecutor {
    /// Ejecuta la llamada. `Ok` lleva un resumen corto para el modelo
    /// («dibujadas 4 figuras», o el texto del OCR); `Err`, el motivo.
    fn ejecutar(&mut self, llamada: &Llamada) -> Result<String, String>;
}

/// Como acabo el turno.
#[derive(Debug, Clone, PartialEq)]
pub struct Final {
    pub texto: String,
    pub herramientas_hechas: usize,
    pub reintentos: usize,
    pub cortado: bool,
}

/// **Lo que el turno sabe hacer ademas de contestar** (pedido del 13-oct:
/// ver lo que piensa, la peticion mejorada, la hoja de trabajo y no quedarse
/// sin contexto). Por omision todo apagado: el turno de siempre.
#[derive(Default)]
pub struct Opciones<'a> {
    /// Antes de actuar, una vuelta corta en la que el modelo escribe que
    /// entendio, que hara y con que herramientas ([`Plan`]).
    pub planificar: bool,
    /// Que piense en esa vuelta (el modo pensar de Qwen; Claude y DeepSeek
    /// piensan por su cuenta). Con el 0.8B se puede apagar si va lento.
    pub pensar: bool,
    /// La hoja de trabajo del chat: va en las instrucciones de sistema.
    pub hoja: Option<&'a HojaDeTrabajo>,
    /// Los tokens de la ventana del motor. 0: no se mide.
    pub ventana: u32,
    /// Buscar en la conversacion guardada: con esto el modelo tiene la
    /// herramienta `buscar_historial`.
    pub buscar: Option<&'a dyn Fn(&str) -> String>,
    /// Todas las herramientas que podria usar (el catalogo entero). La
    /// vuelta de planear elige entre ellas, y las que nombre se anaden a las
    /// del turno aunque la seleccion por palabras no las diera.
    pub ampliar: Option<&'a [Herramienta]>,
}

/// Tope de la vuelta de planear: sin pensar, cinco lineas; pensando, lo que
/// piense el 2B en una peticion normal.
pub const TOKENS_PLAN: u32 = 320;
pub const TOKENS_PLAN_PENSANDO: u32 = 1536;

/// **Un turno.** `historial` ya trae el mensaje nuevo del usuario al final;
/// al acabar trae tambien todo lo que dijo el asistente (incluidas las
/// llamadas y sus resultados), listo para la vuelta siguiente.
pub fn turno(
    motor: &dyn Motor,
    historial: &mut Vec<Mensaje>,
    herramientas: &[Herramienta],
    ejecutor: &mut dyn Ejecutor,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Result<Final, ErrorIa> {
    turno_con(motor, historial, herramientas, ejecutor, &Opciones::default(), al_evento)
}

/// **Un turno con pensar, plan, hoja y contexto medido** ([`Opciones`]).
pub fn turno_con(
    motor: &dyn Motor,
    historial: &mut Vec<Mensaje>,
    herramientas: &[Herramienta],
    ejecutor: &mut dyn Ejecutor,
    opciones: &Opciones,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Result<Final, ErrorIa> {
    let mut crono = Cronometro::default();
    let r = turno_dentro(motor, historial, herramientas, ejecutor, opciones, &mut crono, al_evento);
    // Penso y no llego a decir nada mas (cortado, o solo pensamiento).
    if let Some(segundos) = crono.otra_cosa() {
        al_evento(Evento::PensoListo { segundos });
    }
    r
}

/// Pasa un trozo del motor al turno: separa el pensamiento escrito entre
/// etiquetas, lo cronometra y lo convierte en eventos. Con `plan`, el texto
/// va ahi (la vuelta de planear: su texto es el plan, no la respuesta).
fn repartir(
    t: &Trozo,
    sep: &mut Separador,
    crono: &mut Cronometro,
    mut plan: Option<&mut String>,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Seguir {
    let mut seguir = Seguir::Si;
    for x in sep.meter(t) {
        let s = match x {
            Trozo::Pensando(s) => {
                crono.pensando(&s);
                // Un pensamiento en blanco no abre la seccion «Pensando…».
                if crono.penso() {
                    al_evento(Evento::Pensando(s))
                } else {
                    Seguir::Si
                }
            }
            otro => {
                if let Some(segundos) = crono.otra_cosa() {
                    al_evento(Evento::PensoListo { segundos });
                }
                match otro {
                    Trozo::Texto(s) => match plan.as_deref_mut() {
                        Some(destino) => {
                            destino.push_str(&s);
                            Seguir::Si
                        }
                        None => al_evento(Evento::Texto(s)),
                    },
                    Trozo::Llamando { nombre } => al_evento(Evento::Llamando(nombre)),
                    Trozo::Pensando(_) => Seguir::Si,
                }
            }
        };
        if s == Seguir::Parar {
            seguir = Seguir::Parar;
        }
    }
    seguir
}

/// **La vuelta de planear.** `Ok(None)`: el usuario paro.
fn planear(
    motor: &dyn Motor,
    historial: &[Mensaje],
    herramientas: &[Herramienta],
    opciones: &Opciones,
    crono: &mut Cronometro,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Result<Option<Plan>, ErrorIa> {
    let u = historial.iter().rposition(|m| m.rol == Rol::Usuario);
    let pedido = u.map(|u| historial[u].texto_entero()).unwrap_or_default();
    let mut texto = String::new();
    if let Some(h) = opciones.hoja.filter(|h| !h.esta_vacia()) {
        texto.push_str(h.a_texto().trim_end());
        texto.push_str("\n\n");
    }
    let antes: Vec<&Mensaje> = historial[..u.unwrap_or(0)]
        .iter()
        .filter(|m| matches!(m.rol, Rol::Usuario | Rol::Asistente) && !m.texto_entero().trim().is_empty())
        .collect();
    if !antes.is_empty() {
        texto.push_str("## Conversacion reciente\n");
        for m in &antes[antes.len().saturating_sub(2)..] {
            let quien = if m.rol == Rol::Usuario { "Usuario" } else { "Asistente" };
            texto.push_str(&format!("{quien}: {}\n", cortar(&m.texto_entero(), 400)));
        }
        texto.push('\n');
    }
    texto.push_str("## Mensaje nuevo\n");
    texto.push_str(&pedido);
    if u.is_some_and(|u| historial[u].tiene_imagen()) {
        texto.push_str("\n(adjunta una foto)");
    }
    let mut p = Peticion::nueva(vec![Mensaje::sistema(instrucciones_del_plan(herramientas)), Mensaje::usuario(texto)]);
    p.pensar = opciones.pensar;
    p.max_tokens = if opciones.pensar { TOKENS_PLAN_PENSANDO } else { TOKENS_PLAN };
    p.temperatura = 0.6;
    let mut sep = Separador::nuevo();
    let mut plan_texto = String::new();
    let r = motor.completar(&p, &mut |t| repartir(t, &mut sep, crono, Some(&mut plan_texto), al_evento));
    for t in sep.acabar() {
        repartir(&t, &mut Separador::nuevo(), crono, Some(&mut plan_texto), al_evento);
    }
    let r = match r {
        Ok(r) => r,
        // Sin plan no se deja de contestar: se sigue con el pedido tal cual
        // (si el motor esta caido, la vuelta de actuar lo dira).
        Err(e) => {
            tracing::warn!(%e, "asistente: la vuelta de planear fallo");
            let plan = Plan::por_omision(&pedido);
            al_evento(Evento::Peticion(plan.clone()));
            return Ok(Some(plan));
        }
    };
    // Un motor que no manda el pensamiento a trozos lo trae al final.
    if !crono.penso() && !r.pensamiento.trim().is_empty() {
        crono.pensando(&r.pensamiento);
        al_evento(Evento::Pensando(r.pensamiento.clone()));
    }
    if r.fin == Fin::Cortado {
        return Ok(None);
    }
    if plan_texto.trim().is_empty() {
        plan_texto = r.texto.clone();
    }
    // Un `</think>` suelto en el plan: lo de antes era pensamiento.
    if plan_texto.contains(CIERRE_PENSAR) {
        let (pensado, resto) = separar(&plan_texto);
        if !crono.penso() && !pensado.trim().is_empty() {
            crono.pensando(&pensado);
            al_evento(Evento::Pensando(pensado));
        }
        plan_texto = resto;
    }
    if let Some(segundos) = crono.otra_cosa() {
        al_evento(Evento::PensoListo { segundos });
    }
    let plan = Plan::leer(&plan_texto, herramientas).unwrap_or_else(|| Plan::por_omision(&pedido));
    if al_evento(Evento::Peticion(plan.clone())) == Seguir::Parar {
        return Ok(None);
    }
    Ok(Some(plan))
}

/// Mide el historial y, si pasa del 70 %, poda los resultados viejos y, si
/// no basta, reinicia. Avisa si cambio.
fn vigilar_contexto(
    historial: &mut Vec<Mensaje>,
    herramientas: &[Herramienta],
    ventana: u32,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) {
    let antes = contexto::estimar(historial, herramientas);
    if !contexto::pasa_del_umbral(antes, MAX_TOKENS_CON_HERRAMIENTAS, ventana) {
        return;
    }
    contexto::podar_en_turno(historial);
    if contexto::pasa_del_umbral(contexto::estimar(historial, herramientas), MAX_TOKENS_CON_HERRAMIENTAS, ventana) {
        *historial = contexto::reiniciar(historial);
    }
    let despues = contexto::estimar(historial, herramientas);
    if despues < antes {
        al_evento(Evento::Reinicio { antes, despues });
    }
}

/// La marca de fin de pensamiento que a veces queda en el texto.
const CIERRE_PENSAR: &str = "</think>";

fn turno_dentro(
    motor: &dyn Motor,
    historial: &mut Vec<Mensaje>,
    herramientas_base: &[Herramienta],
    ejecutor: &mut dyn Ejecutor,
    opciones: &Opciones,
    crono: &mut Cronometro,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Result<Final, ErrorIa> {
    let mut lista = herramientas_base.to_vec();
    if opciones.buscar.is_some() && !lista.iter().any(|h| h.nombre == contexto::BUSCAR_HISTORIAL) {
        lista.push(contexto::herramienta_buscar());
    }
    let mut plan = None;
    if opciones.planificar {
        // El plan elige entre todas las que hay (`ampliar`), no solo entre
        // las que la seleccion por palabras dio a este turno: «ponle tres
        // ventanas» no dice «dibuja», pero el plan si lo entiende.
        let mut posibles = lista.clone();
        for h in opciones.ampliar.unwrap_or_default() {
            if !posibles.iter().any(|x| x.nombre == h.nombre) {
                posibles.push(h.clone());
            }
        }
        match planear(motor, historial, &posibles, opciones, crono, al_evento)? {
            Some(p) => {
                for n in &p.herramientas {
                    if !lista.iter().any(|x| &x.nombre == n)
                        && let Some(h) = posibles.iter().find(|x| &x.nombre == n)
                    {
                        lista.push(h.clone());
                    }
                }
                plan = Some(p);
            }
            None => {
                return Ok(Final {
                    texto: String::new(),
                    herramientas_hechas: 0,
                    reintentos: 0,
                    cortado: true,
                });
            }
        }
    }
    let herramientas = &lista[..];
    if opciones.hoja.is_some() || plan.is_some() {
        let base = match historial.first() {
            Some(m) if m.rol == Rol::Sistema => m.texto_entero(),
            _ => instrucciones(),
        };
        let hoja = opciones.hoja.cloned().unwrap_or_default();
        contexto::poner_sistema(historial, contexto::sistema_con_hoja(&base, &hoja, plan.as_ref()));
    }
    let mut hechas = 0;
    let mut reintentos = 0;
    let mut cortado = false;
    for _ in 0..VUELTAS + REINTENTOS {
        if opciones.ventana > 0 {
            vigilar_contexto(historial, herramientas, opciones.ventana, al_evento);
        }
        let mut peticion = Peticion::nueva(historial.clone());
        peticion.herramientas = herramientas.to_vec();
        // Un dibujo con muchas figuras son 1500-2000 tokens de JSON: con el
        // tope de 1024 la llamada salia cortada (medido con el 2B, 13-oct).
        peticion.max_tokens = MAX_TOKENS_CON_HERRAMIENTAS;
        let mut sep = Separador::nuevo();
        let respuesta = motor.completar(&peticion, &mut |t| repartir(t, &mut sep, crono, None, al_evento));
        for t in sep.acabar() {
            repartir(&t, &mut Separador::nuevo(), crono, None, al_evento);
        }
        let mut respuesta = match respuesta {
            Ok(r) => r,
            // `llama-server` no deja pasar una llamada cuyo JSON no cierra
            // (HTTP 500): es una llamada mal hecha como cualquier otra, y se
            // le pide al modelo que la repita mas corta.
            Err(ErrorIa::Protocolo(m)) if llamada_rota(&m) && reintentos < REINTENTOS => {
                reintentos += 1;
                let motivo = "ERROR: tu llamada a la herramienta salio cortada o con JSON roto. \
                              Vuelve a llamarla con menos figuras (20 como mucho) y un JSON completo."
                    .to_string();
                if al_evento(Evento::Reintento { motivo: motivo.clone() }) == Seguir::Parar {
                    cortado = true;
                    break;
                }
                historial.push(Mensaje::usuario(motivo));
                continue;
            }
            Err(e) => return Err(e),
        };
        if respuesta.texto.contains(CIERRE_PENSAR) {
            respuesta.texto = separar(&respuesta.texto).1;
        }
        historial.push(mensaje_del_asistente(&respuesta));
        if respuesta.fin == Fin::Cortado {
            cortado = true;
            break;
        }
        if respuesta.llamadas.is_empty() {
            return Ok(Final {
                texto: respuesta.texto,
                herramientas_hechas: hechas,
                reintentos,
                cortado: false,
            });
        }
        let mut alguna_mal = false;
        // Lo que pidio el usuario, para los colores que el modelo olvide.
        let pedido = historial
            .iter()
            .rev()
            .find(|m| m.rol == Rol::Usuario)
            .map(Mensaje::texto_entero)
            .unwrap_or_default();
        for cruda in &respuesta.llamadas {
            // `buscar_historial` no es del chat: lo contesta el propio turno.
            if let Some(buscar) = opciones.buscar.filter(|_| cruda.nombre.trim() == contexto::BUSCAR_HISTORIAL) {
                let texto = match contexto::consulta_de(&cruda.argumentos) {
                    Some(c) => buscar(&c),
                    None => "ERROR: falta «consulta». Llama a buscar_historial con {\"consulta\": \"que buscar\"}."
                        .into(),
                };
                let ok = !texto.starts_with("ERROR");
                alguna_mal |= !ok;
                al_evento(Evento::Resultado {
                    nombre: contexto::BUSCAR_HISTORIAL.to_string(),
                    ok,
                    texto: cortar(&texto, 300),
                });
                historial.push(Mensaje::resultado(&cruda.id, texto));
                continue;
            }
            match validar(cruda, herramientas) {
                Ok(mut v) => {
                    if let Llamada::Dibujar(figuras) = &mut v.llamada {
                        colores_del_pedido(figuras, &pedido);
                    }
                    al_evento(Evento::Herramienta {
                        llamada: v.llamada.clone(),
                        avisos: v.avisos.clone(),
                    });
                    let (ok, texto) = match ejecutor.ejecutar(&v.llamada) {
                        Ok(t) => (true, t),
                        Err(t) => (false, format!("ERROR: {t}")),
                    };
                    hechas += usize::from(ok);
                    al_evento(Evento::Resultado {
                        nombre: v.llamada.nombre().to_string(),
                        ok,
                        texto: texto.clone(),
                    });
                    let mut para_el_modelo = texto;
                    if !v.avisos.is_empty() {
                        para_el_modelo.push_str(&format!(" (avisos: {})", v.avisos.join("; ")));
                    }
                    historial.push(Mensaje::resultado(&cruda.id, para_el_modelo));
                }
                Err(e) => {
                    alguna_mal = true;
                    let motivo = e.para_el_modelo(herramientas);
                    if al_evento(Evento::Reintento { motivo: motivo.clone() }) == Seguir::Parar {
                        cortado = true;
                    }
                    historial.push(Mensaje::resultado(&cruda.id, motivo));
                }
            }
        }
        if cortado {
            break;
        }
        if alguna_mal {
            reintentos += 1;
            if reintentos > REINTENTOS {
                break;
            }
        }
    }
    // Se acabaron las vueltas o los reintentos (o se corto): se dice claro en
    // vez de dejar la burbuja vacia.
    let texto = if cortado {
        String::new()
    } else if hechas > 0 {
        // Algo si salio (medido: el 2B dibuja bien y luego insiste en
        // anadir mas con datos rotos): no se dice que no se hizo nada.
        "Hecho en parte: lo demas no me salio con las herramientas.".to_string()
    } else {
        "No he sabido hacerlo con las herramientas; prueba a pedirlo de otra forma.".to_string()
    };
    Ok(Final {
        texto,
        herramientas_hechas: hechas,
        reintentos,
        cortado,
    })
}

/// Tope de tokens de cada vuelta del agente.
pub const MAX_TOKENS_CON_HERRAMIENTAS: u32 = 2048;

/// Si el error del motor es una llamada a herramienta con el JSON roto (lo
/// que contesta `llama-server` cuando no la puede leer).
fn llamada_rota(m: &str) -> bool {
    let m = m.to_lowercase();
    m.contains("tool call") && (m.contains("json") || m.contains("parse"))
}

fn mensaje_del_asistente(r: &Respuesta) -> Mensaje {
    let mut m = Mensaje::texto(Rol::Asistente, r.texto.clone());
    m.llamadas = r.llamadas.clone();
    m
}

/// Las instrucciones de sistema del asistente. Cortas: cada token cuenta en
/// cada vuelta, y los modelos pequenos siguen mejor pocas reglas claras.
pub fn instrucciones() -> String {
    "Eres el asistente de PixPin, una app de capturas, notas y lienzo. \
     Contesta en el idioma del usuario, breve y claro. \
     Si pide dibujar algo, usa dibujar_en_lienzo, \
     poniendo el color de cada figura. \
     Si pide copiar o calcar al lienzo una imagen adjunta, usa calcar_imagen. \
     Si da datos para graficar, usa grafica. \
     Si quiere el texto de un documento escaneado, usa ocr. \
     Si pide hacer algo en la app (notas, tareas, ventanas, capturas, abrir ficheros), usa su herramienta. \
     Si pregunta donde esta algo o como se hace en PixPin, usa ayuda_app y contesta con lo que diga, sin inventar. \
     Despues de usar una herramienta, di en una frase lo que hiciste. "
        .to_string()
        + crate::conocimiento::RESUMEN
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::herramientas::catalogo;
    use crate::motor::MotorGuionado;

    /// Un ejecutor que apunta lo que le piden.
    #[derive(Default)]
    struct Apunta {
        hechas: Vec<Llamada>,
        fallar: bool,
    }

    impl Ejecutor for Apunta {
        fn ejecutar(&mut self, llamada: &Llamada) -> Result<String, String> {
            self.hechas.push(llamada.clone());
            if self.fallar {
                Err("el lienzo esta cerrado".into())
            } else {
                Ok("hecho".into())
            }
        }
    }

    fn historial() -> Vec<Mensaje> {
        vec![Mensaje::sistema(instrucciones()), Mensaje::usuario("dibuja una casa")]
    }

    fn correr(guion: Vec<Respuesta>, ejecutor: &mut Apunta) -> (Final, Vec<Evento>, Vec<Mensaje>, MotorGuionado) {
        let motor = MotorGuionado::nuevo(guion);
        let mut h = historial();
        let mut eventos = Vec::new();
        let f = turno(&motor, &mut h, &catalogo(), ejecutor, &mut |e| {
            eventos.push(e);
            Seguir::Si
        })
        .unwrap();
        (f, eventos, h, motor)
    }

    #[test]
    fn solo_texto_acaba_en_una_vuelta() {
        let mut ej = Apunta::default();
        let (f, eventos, h, _) = correr(vec![Respuesta::de_texto("Hola que tal")], &mut ej);
        assert_eq!(f.texto, "Hola que tal");
        assert_eq!(f.herramientas_hechas, 0);
        assert!(matches!(&eventos[0], Evento::Texto(t) if t == "Hola "));
        assert_eq!(h.last().unwrap().rol, Rol::Asistente);
    }

    #[test]
    fn llamada_buena_se_ejecuta_y_el_resultado_vuelve_al_modelo() {
        let mut ej = Apunta::default();
        let (f, eventos, h, motor) = correr(
            vec![
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"rectangulo","x":10,"y":10}]}"#),
                Respuesta::de_texto("Listo, dibuje un rectangulo."),
            ],
            &mut ej,
        );
        assert_eq!(f.herramientas_hechas, 1);
        assert_eq!(f.texto, "Listo, dibuje un rectangulo.");
        assert_eq!(ej.hechas.len(), 1);
        assert!(eventos.iter().any(|e| matches!(e, Evento::Resultado { ok: true, .. })));
        // La segunda peticion lleva la llamada y su resultado.
        let pedidas = motor.recibido.lock().unwrap();
        let segunda = &pedidas[1].mensajes;
        assert_eq!(segunda[segunda.len() - 2].llamadas.len(), 1);
        assert_eq!(segunda.last().unwrap().rol, Rol::Herramienta);
        assert_eq!(segunda.last().unwrap().texto_entero(), "hecho");
        assert!(!pedidas[0].herramientas.is_empty());
        assert_eq!(h.len(), 5);
    }

    #[test]
    fn llamada_mala_se_reintenta_con_el_motivo() {
        let mut ej = Apunta::default();
        let (f, eventos, _, motor) = correr(
            vec![
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"dragon","x":1,"y":1}]}"#),
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"texto","x":1,"y":1,"texto":"CASA"}]}"#),
                Respuesta::de_texto("Hecho."),
            ],
            &mut ej,
        );
        assert_eq!(f.reintentos, 1);
        assert_eq!(f.herramientas_hechas, 1);
        assert_eq!(ej.hechas.len(), 1, "la mala no se ejecuta");
        assert!(eventos.iter().any(|e| matches!(e, Evento::Reintento { motivo } if motivo.contains("dragon"))));
        let pedidas = motor.recibido.lock().unwrap();
        assert!(pedidas[1].mensajes.last().unwrap().texto_entero().starts_with("ERROR:"));
    }

    #[test]
    fn herramienta_inventada_tambien_se_reintenta() {
        let mut ej = Apunta::default();
        let (f, _, _, _) = correr(
            vec![Respuesta::de_llamada("pintar_cielo", "{}"), Respuesta::de_texto("No puedo hacer eso.")],
            &mut ej,
        );
        assert_eq!(f.reintentos, 1);
        assert_eq!(f.texto, "No puedo hacer eso.");
        assert!(ej.hechas.is_empty());
    }

    #[test]
    fn si_se_equivoca_siempre_se_rinde_con_un_mensaje() {
        let mut ej = Apunta::default();
        let malas = (0..10).map(|_| Respuesta::de_llamada("grafica", "{roto")).collect();
        let (f, _, _, motor) = correr(malas, &mut ej);
        assert_eq!(f.reintentos, REINTENTOS + 1);
        assert!(f.texto.contains("No he sabido"));
        assert_eq!(motor.recibido.lock().unwrap().len(), REINTENTOS + 1);
    }

    #[test]
    fn el_fallo_del_ejecutor_se_le_cuenta_al_modelo() {
        let mut ej = Apunta {
            fallar: true,
            ..Default::default()
        };
        let (f, eventos, _, motor) = correr(
            vec![
                Respuesta::de_llamada("ocr", "{}"),
                Respuesta::de_texto("No pude: el lienzo esta cerrado."),
            ],
            &mut ej,
        );
        assert_eq!(f.herramientas_hechas, 0);
        assert!(eventos.iter().any(|e| matches!(e, Evento::Resultado { ok: false, .. })));
        let pedidas = motor.recibido.lock().unwrap();
        assert!(pedidas[1].mensajes.last().unwrap().texto_entero().contains("lienzo esta cerrado"));
    }

    #[test]
    fn un_modelo_que_llama_en_bucle_se_corta() {
        let mut ej = Apunta::default();
        let bucle = (0..20)
            .map(|_| Respuesta::de_llamada("ocr", "{}"))
            .collect();
        let (f, _, _, motor) = correr(bucle, &mut ej);
        // Las llamadas si se hicieron: no se dice que no se hizo nada.
        assert!(f.texto.contains("Hecho en parte"), "{}", f.texto);
        assert_eq!(motor.recibido.lock().unwrap().len(), VUELTAS + REINTENTOS);
    }

    /// Falla la primera vez como `llama-server` con una llamada cortada.
    struct CortaUnaVez(std::sync::atomic::AtomicBool, MotorGuionado);

    impl Motor for CortaUnaVez {
        fn completar(
            &self,
            p: &Peticion,
            t: &mut dyn FnMut(&Trozo) -> Seguir,
        ) -> Result<Respuesta, ErrorIa> {
            assert_eq!(p.max_tokens, MAX_TOKENS_CON_HERRAMIENTAS);
            if !self.0.swap(true, std::sync::atomic::Ordering::SeqCst) {
                return Err(ErrorIa::Protocolo(
                    "HTTP 500: Failed to parse tool call arguments as JSON: unexpected end of input".into(),
                ));
            }
            self.1.completar(p, t)
        }
    }

    #[test]
    fn una_llamada_cortada_por_el_servidor_se_reintenta() {
        let motor = CortaUnaVez(
            Default::default(),
            MotorGuionado::nuevo(vec![
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"rombo","x":1,"y":1}]}"#),
                Respuesta::de_texto("Listo."),
            ]),
        );
        let mut h = historial();
        let mut ej = Apunta::default();
        let f = turno(&motor, &mut h, &catalogo(), &mut ej, &mut |_| Seguir::Si).unwrap();
        assert_eq!(f.reintentos, 1);
        assert_eq!(f.herramientas_hechas, 1);
        assert!(h.iter().any(|m| m.texto_entero().contains("salio cortada")));
        // Otro error del motor no se reintenta.
        let vacio = MotorGuionado::nuevo(vec![]);
        assert!(turno(&vacio, &mut historial(), &catalogo(), &mut ej, &mut |_| Seguir::Si).is_err());
    }

    // -----------------------------------------------------------------------
    // Pensar, plan, hoja y contexto (13-oct)

    fn plan_del_modelo(pensamiento: &str, texto: &str) -> Respuesta {
        Respuesta {
            pensamiento: pensamiento.into(),
            ..Respuesta::de_texto(texto)
        }
    }

    const PLAN_CASA: &str = "ENTIENDO: Dibujar una casa roja en el lienzo.\nHARE: llamar a dibujar_en_lienzo con figuras rojas.\nHERRAMIENTAS: dibujar_en_lienzo\nDATOS: color rojo\nDECIDIDO: nada";

    fn con_opciones(
        guion: Vec<Respuesta>,
        h: &mut Vec<Mensaje>,
        o: &Opciones,
    ) -> (Final, Vec<Evento>, MotorGuionado) {
        let motor = MotorGuionado::nuevo(guion);
        let mut eventos = Vec::new();
        let f = turno_con(&motor, h, &catalogo(), &mut Apunta::default(), o, &mut |e| {
            eventos.push(e);
            Seguir::Si
        })
        .unwrap();
        (f, eventos, motor)
    }

    #[test]
    fn el_pensamiento_llega_a_trozos_se_pliega_y_luego_va_el_plan() {
        let mut h = historial();
        let o = Opciones {
            planificar: true,
            pensar: true,
            ..Default::default()
        };
        let (f, eventos, motor) = con_opciones(
            vec![
                plan_del_modelo("El usuario quiere una casa. La hare roja.", PLAN_CASA),
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"rectangulo","x":1,"y":1}]}"#),
                Respuesta::de_texto("Listo, una casa roja."),
            ],
            &mut h,
            &o,
        );
        assert_eq!(f.texto, "Listo, una casa roja.");
        let pensados: Vec<&str> = eventos
            .iter()
            .filter_map(|e| if let Evento::Pensando(s) = e { Some(s.as_str()) } else { None })
            .collect();
        assert!(pensados.len() > 3, "a trozos: {pensados:?}");
        assert_eq!(pensados.concat(), "El usuario quiere una casa. La hare roja.");
        // Se pliega una vez, despues de pensar y antes del plan.
        let i_listo: Vec<usize> = eventos
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e, Evento::PensoListo { .. }))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(i_listo.len(), 1);
        let i_plan = eventos.iter().position(|e| matches!(e, Evento::Peticion(_))).unwrap();
        let i_ultimo_pensado = eventos.iter().rposition(|e| matches!(e, Evento::Pensando(_))).unwrap();
        let i_texto = eventos.iter().position(|e| matches!(e, Evento::Texto(_))).unwrap();
        assert!(i_ultimo_pensado < i_listo[0] && i_listo[0] < i_plan && i_plan < i_texto);
        let Evento::Peticion(plan) = &eventos[i_plan] else { unreachable!() };
        assert_eq!(plan.entiendo, "Dibujar una casa roja en el lienzo.");
        assert!(plan.del_modelo);
        // El texto del plan no sale como respuesta.
        assert!(!eventos.iter().any(|e| matches!(e, Evento::Texto(t) if t.contains("ENTIENDO"))));
        let pedidas = motor.recibido.lock().unwrap();
        assert!(pedidas[0].pensar && pedidas[0].herramientas.is_empty(), "planear piensa y no usa herramientas");
        assert_eq!(pedidas[0].max_tokens, TOKENS_PLAN_PENSANDO);
        assert!(!pedidas[1].pensar);
        let sistema = pedidas[1].mensajes[0].texto_entero();
        assert!(sistema.contains("Peticion mejorada") && sistema.contains("casa roja"), "{sistema}");
    }

    #[test]
    fn un_modelo_que_no_piensa_no_abre_la_seccion_pero_si_planea() {
        let mut h = historial();
        let o = Opciones {
            planificar: true,
            pensar: true,
            ..Default::default()
        };
        let (_, eventos, _) = con_opciones(
            vec![plan_del_modelo("", PLAN_CASA), Respuesta::de_texto("Vale.")],
            &mut h,
            &o,
        );
        assert!(!eventos.iter().any(|e| matches!(e, Evento::Pensando(_) | Evento::PensoListo { .. })));
        assert!(eventos.iter().any(|e| matches!(e, Evento::Peticion(_))));
    }

    #[test]
    fn un_pensamiento_en_blanco_tampoco() {
        let mut h = historial();
        let o = Opciones {
            planificar: true,
            pensar: true,
            ..Default::default()
        };
        let (_, eventos, _) = con_opciones(
            vec![plan_del_modelo("   \n  ", PLAN_CASA), Respuesta::de_texto("Vale.")],
            &mut h,
            &o,
        );
        assert!(!eventos.iter().any(|e| matches!(e, Evento::Pensando(_) | Evento::PensoListo { .. })));
    }

    #[test]
    fn si_el_plan_no_se_entiende_se_usa_el_pedido() {
        let mut h = historial();
        let o = Opciones {
            planificar: true,
            ..Default::default()
        };
        let (f, eventos, motor) = con_opciones(
            vec![Respuesta::de_texto("¡Claro! Aqui tienes tu casa."), Respuesta::de_texto("Hecho.")],
            &mut h,
            &o,
        );
        assert_eq!(f.texto, "Hecho.");
        let plan = eventos
            .iter()
            .find_map(|e| if let Evento::Peticion(p) = e { Some(p.clone()) } else { None })
            .unwrap();
        assert_eq!(plan.entiendo, "dibuja una casa");
        assert!(!plan.del_modelo);
        assert_eq!(motor.recibido.lock().unwrap()[0].max_tokens, TOKENS_PLAN);
    }

    #[test]
    fn el_pensamiento_entre_etiquetas_no_sale_en_la_respuesta() {
        let mut h = historial();
        let (f, eventos, _) = con_opciones(
            vec![Respuesta::de_texto("<think>Es un saludo. </think>Hola, que tal")],
            &mut h,
            &Opciones::default(),
        );
        assert_eq!(f.texto, "Hola, que tal");
        let texto: String = eventos
            .iter()
            .filter_map(|e| if let Evento::Texto(s) = e { Some(s.as_str()) } else { None })
            .collect();
        assert_eq!(texto, "Hola, que tal");
        assert!(eventos.iter().any(|e| matches!(e, Evento::Pensando(s) if s.contains("saludo"))));
        assert!(eventos.iter().any(|e| matches!(e, Evento::PensoListo { .. })));
        assert_eq!(h.last().unwrap().texto_entero(), "Hola, que tal");
    }

    #[test]
    fn buscar_historial_contesta_desde_el_turno() {
        let fuentes = vec![
            contexto::Fuente { etiqueta: "#3 usuario".into(), texto: "Las ventas de enero fueron 120.".into() },
            contexto::Fuente { etiqueta: "#4 asistente".into(), texto: "Apuntado.".into() },
        ];
        let buscar = |c: &str| contexto::buscar(&fuentes, c, 3);
        let o = Opciones {
            buscar: Some(&buscar),
            ..Default::default()
        };
        let mut h = historial();
        let (f, eventos, motor) = con_opciones(
            vec![
                Respuesta::de_llamada(contexto::BUSCAR_HISTORIAL, r#"{"consulta":"ventas de enero"}"#),
                Respuesta::de_llamada(contexto::BUSCAR_HISTORIAL, r#"{}"#),
                Respuesta::de_texto("Fueron 120."),
            ],
            &mut h,
            &o,
        );
        assert_eq!(f.texto, "Fueron 120.");
        assert_eq!(f.herramientas_hechas, 0, "no es una herramienta del chat");
        assert_eq!(f.reintentos, 1, "la llamada sin consulta cuenta como mal hecha");
        let pedidas = motor.recibido.lock().unwrap();
        assert!(pedidas[0].herramientas.iter().any(|x| x.nombre == contexto::BUSCAR_HISTORIAL));
        assert!(pedidas[1].mensajes.last().unwrap().texto_entero().contains("[#3 usuario] Las ventas de enero fueron 120."));
        assert!(pedidas[2].mensajes.last().unwrap().texto_entero().starts_with("ERROR"));
        assert!(eventos.iter().any(|e| matches!(e, Evento::Resultado { ok: true, nombre, .. } if nombre == contexto::BUSCAR_HISTORIAL)));
        // Sin `buscar`, la herramienta no se ofrece y es una inventada.
        let mut h = historial();
        let (f, _, motor) = con_opciones(
            vec![Respuesta::de_llamada(contexto::BUSCAR_HISTORIAL, r#"{"consulta":"x"}"#), Respuesta::de_texto("No.")],
            &mut h,
            &Opciones::default(),
        );
        assert_eq!(f.reintentos, 1);
        assert!(!motor.recibido.lock().unwrap()[0].herramientas.iter().any(|x| x.nombre == contexto::BUSCAR_HISTORIAL));
    }

    #[test]
    fn al_pasar_del_70_se_reinicia_sin_perder_el_objetivo() {
        let hoja = HojaDeTrabajo {
            objetivo: "Hacer el plano de la casa de Ana con 3 ventanas".into(),
            reinicios: 0,
            ..Default::default()
        };
        let mut h = vec![Mensaje::sistema(instrucciones())];
        for i in 0..30 {
            h.push(Mensaje::usuario(format!("mensaje viejo {i} {}", "bla ".repeat(60))));
            h.push(Mensaje::texto(Rol::Asistente, format!("respuesta vieja {i} {}", "ble ".repeat(60))));
        }
        h.push(Mensaje::usuario("y ahora la puerta"));
        let antes = h.len();
        let o = Opciones {
            planificar: true,
            hoja: Some(&hoja),
            ventana: 8192,
            ..Default::default()
        };
        // Con las herramientas de un turno (la seleccion), no el catalogo
        // entero: ese solo ya no cabe en una ventana pequena.
        let motor = MotorGuionado::nuevo(vec![
            Respuesta::de_texto("ENTIENDO: Anadir la puerta al plano de la casa de Ana.\nHARE: dibujarla."),
            Respuesta::de_texto("Puerta puesta."),
        ]);
        let mut eventos = Vec::new();
        turno_con(&motor, &mut h, &crate::herramientas::catalogo_base(), &mut Apunta::default(), &o, &mut |e| {
            eventos.push(e);
            Seguir::Si
        })
        .unwrap();
        let reinicio = eventos.iter().find_map(|e| match e {
            Evento::Reinicio { antes, despues } => Some((*antes, *despues)),
            _ => None,
        });
        let (t_antes, t_despues) = reinicio.expect("tenia que reiniciar");
        assert!(t_despues < t_antes);
        assert!(!contexto::pasa_del_umbral(t_despues, MAX_TOKENS_CON_HERRAMIENTAS, 8192));
        let pedidas = motor.recibido.lock().unwrap();
        // La vuelta de planear ve la hoja; la de actuar, solo lo justo.
        assert!(pedidas[0].mensajes[1].texto_entero().contains("casa de Ana"));
        let actuar = &pedidas[1].mensajes;
        assert!(actuar.len() <= 4, "sistema + 2 + el nuevo: {}", actuar.len());
        assert!(actuar.len() < antes);
        let sistema = actuar[0].texto_entero();
        assert!(sistema.contains("Objetivo: Hacer el plano de la casa de Ana"), "{sistema}");
        assert!(sistema.contains("Anadir la puerta"), "{sistema}");
        assert_eq!(actuar.last().unwrap().texto_entero(), "y ahora la puerta");
    }

    #[test]
    fn el_plan_anade_las_herramientas_que_la_seleccion_no_dio() {
        let todas = catalogo();
        let solo_ocr: Vec<Herramienta> = todas.iter().filter(|h| h.nombre == "ocr").cloned().collect();
        let o = Opciones {
            planificar: true,
            ampliar: Some(&todas),
            ..Default::default()
        };
        let motor = MotorGuionado::nuevo(vec![Respuesta::de_texto(PLAN_CASA), Respuesta::de_texto("Vale.")]);
        let mut h = historial();
        turno_con(&motor, &mut h, &solo_ocr, &mut Apunta::default(), &o, &mut |_| Seguir::Si).unwrap();
        let pedidas = motor.recibido.lock().unwrap();
        // Planear ve todos los nombres; actuar, la seleccion mas lo del plan.
        assert!(pedidas[0].mensajes[0].texto_entero().contains("dibujar_en_lienzo"));
        let nombres: Vec<&str> = pedidas[1].herramientas.iter().map(|x| x.nombre.as_str()).collect();
        assert_eq!(nombres, vec!["ocr", "dibujar_en_lienzo"]);
    }

    #[test]
    fn parar_mientras_piensa_corta_sin_actuar() {
        let motor = MotorGuionado::nuevo(vec![plan_del_modelo("uno dos tres cuatro cinco", PLAN_CASA)]);
        let mut h = historial();
        let mut n = 0;
        let o = Opciones {
            planificar: true,
            pensar: true,
            ..Default::default()
        };
        let mut eventos = Vec::new();
        let f = turno_con(&motor, &mut h, &catalogo(), &mut Apunta::default(), &o, &mut |e| {
            eventos.push(e);
            n += 1;
            if n >= 2 { Seguir::Parar } else { Seguir::Si }
        })
        .unwrap();
        assert!(f.cortado);
        assert_eq!(motor.recibido.lock().unwrap().len(), 1, "no llego a actuar");
        assert!(matches!(eventos.last(), Some(Evento::PensoListo { .. })), "se pliega al parar");
    }

    #[test]
    fn parar_corta_el_turno() {
        let motor = MotorGuionado::nuevo(vec![Respuesta::de_texto("uno dos tres cuatro")]);
        let mut h = historial();
        let mut n = 0;
        let f = turno(&motor, &mut h, &catalogo(), &mut Apunta::default(), &mut |_| {
            n += 1;
            if n >= 2 { Seguir::Parar } else { Seguir::Si }
        })
        .unwrap();
        assert!(f.cortado);
    }
}
