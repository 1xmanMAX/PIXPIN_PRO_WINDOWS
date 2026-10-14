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
use crate::herramientas::{ErrorHerramienta, Herramienta, Llamada, colores_del_pedido, validar};
use crate::motor::{Fin, Mensaje, Motor, Peticion, Respuesta, Rol, Seguir, Trozo};

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

/// **Un turno.** `historial` ya trae el mensaje nuevo del usuario al final;
/// al acabar trae tambien todo lo que dijo el asistente (incluidas las
/// llamadas y sus resultados), listo para la vuelta siguiente.
///
/// Nunca acaba con la burbuja vacia: si el modelo no dice nada, se dice algo
/// por el ([`SIN_PALABRAS`], [`HECHO`]).
pub fn turno(
    motor: &dyn Motor,
    historial: &mut Vec<Mensaje>,
    herramientas: &[Herramienta],
    ejecutor: &mut dyn Ejecutor,
    al_evento: &mut dyn FnMut(Evento) -> Seguir,
) -> Result<Final, ErrorIa> {
    let mut hechas = 0;
    let mut reintentos = 0;
    let mut cortado = false;
    // Cuantos elementos puede llevar cada lista de una llamada: empieza en
    // [`TOPE_LISTAS`] y baja a [`TOPE_LISTAS_REINTENTO`] si una salio
    // cortada. Con la gramatica de llama.cpp el modelo NO PUEDE pasarse.
    let mut tope = TOPE_LISTAS;
    for _ in 0..VUELTAS + REINTENTOS {
        let mut peticion = Peticion::nueva(historial.clone());
        peticion.herramientas = con_topes(herramientas, tope);
        peticion.max_tokens = tope_de_tokens(&peticion);
        let respuesta = motor.completar(&peticion, &mut |t| match t {
            Trozo::Texto(s) => al_evento(Evento::Texto(s.clone())),
            Trozo::Llamando { nombre } => al_evento(Evento::Llamando(nombre.clone())),
            Trozo::Pensando(_) => Seguir::Si,
        });
        let respuesta = match respuesta {
            Ok(r) => r,
            // `llama-server` no deja pasar una llamada cuyo JSON no cierra
            // (HTTP 500, «Failed to parse tool call arguments»): es una
            // llamada cortada por el tope de tokens. Se le pide que la repita
            // MAS CORTA, y la gramatica se lo impone (listas mas cortas).
            Err(ErrorIa::Protocolo(m)) if llamada_rota(&m) => {
                tracing::warn!(detalle = %m, tope, "asistente: llamada a herramienta cortada");
                reintentos += 1;
                if reintentos > REINTENTOS {
                    break;
                }
                tope = TOPE_LISTAS_REINTENTO;
                let motivo = pedir_mas_corto();
                if al_evento(Evento::Reintento { motivo: motivo.clone() }) == Seguir::Parar {
                    cortado = true;
                    break;
                }
                historial.push(Mensaje::usuario(motivo));
                continue;
            }
            Err(e) => return Err(e),
        };
        historial.push(mensaje_del_asistente(&respuesta));
        if respuesta.fin == Fin::Cortado {
            cortado = true;
            break;
        }
        if respuesta.llamadas.is_empty() {
            let texto = if !respuesta.texto.trim().is_empty() {
                respuesta.texto
            } else if hechas > 0 {
                HECHO.to_string()
            } else {
                SIN_PALABRAS.to_string()
            };
            return Ok(Final {
                texto,
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
            match validar(&sanear(cruda), herramientas) {
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
                    let mut motivo = e.para_el_modelo(herramientas);
                    // Unos argumentos que no cierran casi siempre son una
                    // llamada que se quedo sin tokens: la siguiente, corta.
                    // La siguiente, corta: una llamada larga que sale mal es
                    // la que mas tarda en repetirse.
                    tope = TOPE_LISTAS_REINTENTO;
                    if matches!(e, ErrorHerramienta::JsonRoto) || respuesta.fin == Fin::Longitud {
                        motivo = pedir_mas_corto();
                    }
                    tracing::warn!(herramienta = %cruda.nombre, %motivo, "asistente: llamada mal hecha");
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
        "No he sabido hacerlo con las herramientas; prueba a pedirlo de otra forma o por partes.".to_string()
    };
    Ok(Final {
        texto,
        herramientas_hechas: hechas,
        reintentos,
        cortado,
    })
}

/// Lo que se dice si el modelo acaba sin una palabra y sin hacer nada:
/// mejor una pregunta que una burbuja vacia.
pub const SIN_PALABRAS: &str = "No sé qué decir sobre eso. ¿Qué quieres que haga?";
/// Y si hizo algo pero no lo conto.
pub const HECHO: &str = "Hecho.";

/// Tope de tokens de cada vuelta del agente. Un dibujo de [`TOPE_LISTAS`]
/// figuras completas son unos 1500 tokens de JSON (medido con el 2B: ~1,7
/// letras por token); con el doble de margen es la gramatica, y no el tope,
/// quien corta.
pub const MAX_TOKENS_CON_HERRAMIENTAS: u32 = 3072;
/// Lo minimo que se le deja contestar aunque el contexto vaya lleno.
pub const MIN_TOKENS: u32 = 512;
/// Cuantas figuras puede llevar una llamada (las listas de objetos). Las
/// de datos sueltos (numeros, etiquetas) el doble: pesan poco. 16 figuras
/// completas son ~1600 tokens: en la CPU de este PC, unos 4 minutos; con
/// 24 y textos largos se fueron a 7 (medido, 13-oct).
pub const TOPE_LISTAS: u64 = 16;
/// Y tras una llamada cortada.
pub const TOPE_LISTAS_REINTENTO: u64 = 10;
/// Lo mas largo que puede ser un texto suelto dentro de una llamada.
pub const TOPE_TEXTO: u64 = 120;

/// **Cuanta respuesta se pide**: [`MAX_TOKENS_CON_HERRAMIENTAS`], o lo que
/// quepa en el contexto con lo que ya lleva la conversacion (estimado por
/// lo alto: 3 letras por token y ~1000 por foto).
pub fn tope_de_tokens(p: &Peticion) -> u32 {
    let letras: usize = p
        .mensajes
        .iter()
        .map(|m| m.texto_entero().chars().count() + m.llamadas.iter().map(|l| l.argumentos.len()).sum::<usize>())
        .sum::<usize>()
        + p.herramientas.iter().map(|h| h.a_json().to_string().len()).sum::<usize>();
    let fotos = p.mensajes.iter().filter(|m| m.tiene_imagen()).count() as u32;
    let entrada = (letras / 3) as u32 + 1000 * fotos + 200;
    crate::servidor::CONTEXTO
        .saturating_sub(entrada)
        .clamp(MIN_TOKENS, MAX_TOKENS_CON_HERRAMIENTAS)
}

/// **Las herramientas con sus listas y textos acotados.** `llama-server`
/// convierte el esquema de cada herramienta en una gramatica que restringe
/// lo que el modelo puede escribir (comprobado con b11580 y el 2B: con
/// `maxItems: 3` y pidiendo 15 figuras salen 3; con 100, salen las 15). Asi
/// una llamada no puede crecer hasta cortarse a medias por el tope de
/// tokens, que es lo que dejaba el JSON roto («missing closing quote») y el
/// lienzo vacio. Solo se recorta lo de la peticion: el catalogo y su
/// validacion no cambian.
pub fn con_topes(herramientas: &[Herramienta], tope: u64) -> Vec<Herramienta> {
    fn acotar(v: &mut serde_json::Value, tope: u64) {
        let Some(o) = v.as_object_mut() else { return };
        match o.get("type").and_then(|t| t.as_str()) {
            Some("array") => {
                let de_objetos = o.get("items").and_then(|i| i.get("type")).and_then(|t| t.as_str()) == Some("object");
                let tope = if de_objetos { tope } else { 2 * tope };
                let max = o.get("maxItems").and_then(|m| m.as_u64()).map_or(tope, |m| m.min(tope));
                o.insert("maxItems".into(), max.into());
            }
            Some("string") if !o.contains_key("enum") => {
                let max = o
                    .get("maxLength")
                    .and_then(|m| m.as_u64())
                    .map_or(TOPE_TEXTO, |m| m.min(TOPE_TEXTO));
                o.insert("maxLength".into(), max.into());
            }
            _ => {}
        }
        for (_, hijo) in o.iter_mut() {
            match hijo {
                serde_json::Value::Object(_) => acotar(hijo, tope),
                serde_json::Value::Array(lista) => lista.iter_mut().for_each(|x| acotar(x, tope)),
                _ => {}
            }
        }
    }
    herramientas
        .iter()
        .map(|h| {
            let mut h = h.clone();
            acotar(&mut h.parametros, tope);
            h
        })
        .collect()
}

/// **Lleva al borde las coordenadas que se salen un poco**, antes de
/// validar. El 2B escribe a veces `x: 2010` en la figura 12 de 16, y la
/// validacion (con razon) rechaza la llamada entera: eran 7 minutos de CPU
/// tirados por un numero (medido, 13-oct). Un punto pegado al borde se ve
/// igual; lo que no es numero o no casa, lo sigue diciendo la validacion.
pub fn sanear(cruda: &crate::motor::LlamadaCruda) -> crate::motor::LlamadaCruda {
    use crate::herramientas::{COORDENADA_MAXIMA, COORDENADA_MINIMA};
    fn llevar(v: &mut serde_json::Value, cambios: &mut usize) {
        match v {
            serde_json::Value::Object(o) => {
                for (k, x) in o.iter_mut() {
                    let rango = match k.as_str() {
                        "x" | "y" | "x2" | "y2" => Some((COORDENADA_MINIMA, COORDENADA_MAXIMA)),
                        "ancho" | "alto" => Some((-COORDENADA_MAXIMA, COORDENADA_MAXIMA)),
                        _ => None,
                    };
                    match (rango, x.as_f64()) {
                        (Some((a, b)), Some(n)) if n.is_finite() && (n < a || n > b) => {
                            *x = serde_json::json!(n.clamp(a, b));
                            *cambios += 1;
                        }
                        _ => llevar(x, cambios),
                    }
                }
            }
            serde_json::Value::Array(l) => l.iter_mut().for_each(|x| llevar(x, cambios)),
            _ => {}
        }
    }
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&cruda.argumentos) else {
        return cruda.clone();
    };
    let mut cambios = 0;
    llevar(&mut v, &mut cambios);
    if cambios == 0 {
        return cruda.clone();
    }
    tracing::info!(cambios, herramienta = %cruda.nombre, "asistente: coordenadas llevadas al borde");
    crate::motor::LlamadaCruda {
        argumentos: v.to_string(),
        ..cruda.clone()
    }
}

/// Lo que se le dice al modelo tras una llamada cortada: mas corta, y lo
/// grande, en varias llamadas.
fn pedir_mas_corto() -> String {
    format!(
        "ERROR: tu llamada a la herramienta salio cortada porque era demasiado larga. \
         Vuelve a llamarla con {TOPE_LISTAS_REINTENTO} elementos como mucho, solo las formas principales; \
         si hace falta mas, haz varias llamadas seguidas."
    )
}

/// Si el error del motor es una llamada a herramienta con el JSON roto (lo
/// que contesta `llama-server` cuando no la puede leer).
fn llamada_rota(m: &str) -> bool {
    let m = m.to_lowercase();
    m.contains("tool call") && (m.contains("json") || m.contains("parse"))
}

/// **El fallo en palabras del usuario.** Lo tecnico (codigos HTTP, el JSON
/// del servidor) va al registro, nunca a la burbuja.
pub fn para_el_usuario(e: &ErrorIa) -> String {
    match e {
        ErrorIa::Falta(_) => "Falta bajar el asistente: abre el chat «Asistente» para bajarlo.".into(),
        ErrorIa::Arranque(_) => "No pude arrancar el asistente. Prueba otra vez en un momento.".into(),
        ErrorIa::Conexion(_) => "El asistente dejó de responder. Prueba otra vez.".into(),
        ErrorIa::Protocolo(_) => "Esta vez no me salió la respuesta. Prueba otra vez o pídelo más corto.".into(),
        ErrorIa::Cancelado => "Cancelado.".into(),
        ErrorIa::Descarga { .. } | ErrorIa::HuellaDistinta { .. } => e.to_string(),
        ErrorIa::Io(_) => "No pude contestar por un fallo del equipo. Prueba otra vez.".into(),
    }
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
     Si pide dibujar algo, o copiar una imagen al lienzo, usa dibujar_en_lienzo, \
     poniendo el color de cada figura. \
     Si da datos para graficar, usa grafica. \
     Si quiere el texto de un documento escaneado, usa ocr. \
     Despues de usar una herramienta, di en una frase lo que hiciste."
        .to_string()
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
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"texto","x":1,"y":1}]}"#),
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"texto","x":1,"y":1,"texto":"CASA"}]}"#),
                Respuesta::de_texto("Hecho."),
            ],
            &mut ej,
        );
        assert_eq!(f.reintentos, 1);
        assert_eq!(f.herramientas_hechas, 1);
        assert_eq!(ej.hechas.len(), 1, "la mala no se ejecuta");
        assert!(eventos.iter().any(|e| matches!(e, Evento::Reintento { motivo } if motivo.contains("«texto»"))));
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

    #[test]
    fn una_respuesta_vacia_igual_dice_algo() {
        // Lo que hacia el 2B con una foto sola: ni texto ni herramienta.
        let (f, _, _, _) = correr(vec![Respuesta::de_texto("  ")], &mut Apunta::default());
        assert_eq!(f.texto, SIN_PALABRAS);
        // Y si hizo algo y luego no dijo nada, se dice que esta hecho.
        let (f, _, _, _) = correr(
            vec![
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"rectangulo","x":1,"y":1}]}"#),
                Respuesta::de_texto(""),
            ],
            &mut Apunta::default(),
        );
        assert_eq!(f.texto, HECHO);
        assert_eq!(f.herramientas_hechas, 1);
    }

    fn max_items_de_elementos(h: &[Herramienta]) -> u64 {
        let d = h.iter().find(|h| h.nombre == crate::herramientas::DIBUJAR).unwrap();
        d.parametros["properties"]["elementos"]["maxItems"].as_u64().unwrap()
    }

    #[test]
    fn las_listas_van_acotadas_y_tras_un_json_roto_mas_aun() {
        let mut ej = Apunta::default();
        let (_, _, _, motor) = correr(
            vec![
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"linea","x":1,"y":1},{"tipo":"li"#),
                Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"linea","x":1,"y":1,"x2":5,"y2":5}]}"#),
                Respuesta::de_texto("Listo."),
            ],
            &mut ej,
        );
        let pedidas = motor.recibido.lock().unwrap();
        assert_eq!(max_items_de_elementos(&pedidas[0].herramientas), TOPE_LISTAS);
        assert_eq!(max_items_de_elementos(&pedidas[1].herramientas), TOPE_LISTAS_REINTENTO);
        // Los textos sueltos tambien llevan tope; los de una lista cerrada no.
        let props = &pedidas[0].herramientas[0].parametros["properties"]["elementos"]["items"]["properties"];
        assert_eq!(props["texto"]["maxLength"].as_u64(), Some(TOPE_TEXTO));
        assert!(props["tipo"].get("maxLength").is_none());
        // Al modelo se le pide mas corto y en varias llamadas.
        assert!(pedidas[1].mensajes.last().unwrap().texto_entero().contains("varias llamadas"));
        // El catalogo de verdad (el que valida) no cambia.
        assert_eq!(max_items_de_elementos(&catalogo()), crate::herramientas::MAX_ELEMENTOS as u64);
        assert_eq!(ej.hechas.len(), 1);
    }

    #[test]
    fn con_topes_nunca_amplia_un_tope_mas_pequeno() {
        let h = vec![Herramienta {
            nombre: "x".into(),
            descripcion: String::new(),
            parametros: serde_json::json!({"type":"object","properties":{
                "a":{"type":"array","maxItems":3,"items":{"type":"string","maxLength":5}},
                "b":{"type":"array","items":{"type":"number"}}}}),
        }];
        let c = con_topes(&h, 10);
        assert_eq!(c[0].parametros["properties"]["a"]["maxItems"], 3);
        assert_eq!(c[0].parametros["properties"]["a"]["items"]["maxLength"], 5);
        // Una lista de numeros pesa poco: el doble.
        assert_eq!(c[0].parametros["properties"]["b"]["maxItems"], 20);
    }

    #[test]
    fn una_coordenada_que_se_sale_un_poco_se_lleva_al_borde_y_no_tira_la_llamada() {
        let mut ej = Apunta::default();
        let (f, eventos, _, _) = correr(
            vec![
                Respuesta::de_llamada(
                    "dibujar_en_lienzo",
                    r#"{"elementos":[{"tipo":"linea","x":10,"y":10,"x2":2010,"y2":-350}]}"#,
                ),
                Respuesta::de_texto("Listo."),
            ],
            &mut ej,
        );
        assert_eq!(f.reintentos, 0);
        assert_eq!(ej.hechas.len(), 1);
        assert!(!eventos.iter().any(|e| matches!(e, Evento::Reintento { .. })));
        let Llamada::Dibujar(figs) = &ej.hechas[0] else { panic!() };
        assert_eq!((figs[0].x2, figs[0].y2), (crate::herramientas::COORDENADA_MAXIMA, crate::herramientas::COORDENADA_MINIMA));
        // Lo que no es JSON se deja tal cual (lo dice la validacion).
        let rota = crate::motor::LlamadaCruda { id: "1".into(), nombre: "x".into(), argumentos: "{roto".into() };
        assert_eq!(sanear(&rota), rota);
    }

    /// Siempre falla como `llama-server` con una llamada cortada.
    struct SiempreCortada(std::sync::atomic::AtomicUsize);

    impl Motor for SiempreCortada {
        fn completar(&self, _: &Peticion, _: &mut dyn FnMut(&Trozo) -> Seguir) -> Result<Respuesta, ErrorIa> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(ErrorIa::Protocolo(
                "HTTP 500: {\"error\":{\"code\":500,\"message\":\"Failed to parse tool call arguments as JSON: \
                 [json.exception.parse_error.101] parse error at line 1, column 3514: missing closing quote\"}}"
                    .into(),
            ))
        }
    }

    #[test]
    fn una_llamada_siempre_cortada_acaba_en_palabras_y_no_en_error() {
        let motor = SiempreCortada(Default::default());
        let mut eventos = Vec::new();
        let f = turno(&motor, &mut historial(), &catalogo(), &mut Apunta::default(), &mut |e| {
            eventos.push(e);
            Seguir::Si
        })
        .expect("no es un error para el usuario");
        assert!(f.texto.contains("No he sabido"), "{}", f.texto);
        assert!(!f.texto.contains("HTTP") && !f.texto.contains('{'));
        assert_eq!(motor.0.load(std::sync::atomic::Ordering::SeqCst), REINTENTOS + 1);
        assert_eq!(eventos.iter().filter(|e| matches!(e, Evento::Reintento { .. })).count(), REINTENTOS);
    }

    #[test]
    fn el_fallo_para_el_usuario_no_lleva_nada_tecnico() {
        for e in [
            ErrorIa::Protocolo("HTTP 500: {\"error\":{\"code\":500}}".into()),
            ErrorIa::Conexion("os error 10054".into()),
            ErrorIa::Arranque("el motor se cerro (exit code: 0xc0000005)".into()),
            ErrorIa::Falta("C:\\x\\llama-server.exe".into()),
        ] {
            let t = para_el_usuario(&e);
            assert!(!t.contains("HTTP") && !t.contains('{') && !t.contains("0x") && !t.contains("C:\\"), "{t}");
            assert!(!t.is_empty());
        }
    }

    #[test]
    fn el_tope_de_tokens_cabe_en_el_contexto() {
        let corta = Peticion::nueva(historial());
        assert_eq!(tope_de_tokens(&corta), MAX_TOKENS_CON_HERRAMIENTAS);
        // Una conversacion que llena el contexto: menos respuesta, nunca
        // menos del minimo.
        let mut larga = historial();
        larga.push(Mensaje::usuario("x".repeat(15_000)));
        let t = tope_de_tokens(&Peticion::nueva(larga.clone()));
        assert!(t < MAX_TOKENS_CON_HERRAMIENTAS && t >= MIN_TOKENS, "{t}");
        larga.push(Mensaje::usuario("x".repeat(60_000)));
        assert_eq!(tope_de_tokens(&Peticion::nueva(larga)), MIN_TOKENS);
    }
}
