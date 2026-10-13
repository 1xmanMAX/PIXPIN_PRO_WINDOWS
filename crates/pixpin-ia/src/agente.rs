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
use crate::herramientas::{Herramienta, Llamada, colores_del_pedido, validar};
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
    for _ in 0..VUELTAS + REINTENTOS {
        let mut peticion = Peticion::nueva(historial.clone());
        peticion.herramientas = herramientas.to_vec();
        // Un dibujo con muchas figuras son 1500-2000 tokens de JSON: con el
        // tope de 1024 la llamada salia cortada (medido con el 2B, 13-oct).
        peticion.max_tokens = MAX_TOKENS_CON_HERRAMIENTAS;
        let respuesta = motor.completar(&peticion, &mut |t| match t {
            Trozo::Texto(s) => al_evento(Evento::Texto(s.clone())),
            Trozo::Llamando { nombre } => al_evento(Evento::Llamando(nombre.clone())),
            Trozo::Pensando(_) => Seguir::Si,
        });
        let respuesta = match respuesta {
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
}
