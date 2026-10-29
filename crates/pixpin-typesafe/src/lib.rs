//! **Jev en PixPin** (28-oct-2026, ver `docs/jev.md`).
//!
//! Jev, de TypeSafe, es un modelo «System One»: contesta preguntas tipadas
//! sobre un estado que se le da (texto, JSON con campos con nombre e
//! imagenes) con probabilidades calibradas, en 70-500 ms. Cuatro tipos:
//! **Noul** (la probabilidad de que algo sea verdad), **Choice** (una de
//! varias opciones), **Score** (un nivel de una escala descrita nivel a
//! nivel) y **Box** (donde esta algo en una imagen).
//!
//! La regla de este crate: **los juicios de Jev, las decisiones en codigo**.
//! Jev dice «esta linea encarga una tarea» con su probabilidad; que se marca,
//! que tarjeta queda, que prioridad tiene una tarea o que modelo contesta lo
//! decide una tabla escrita aqui, con sus pruebas.
//!
//! - [`api`]: la peticion y la respuesta tal como las describe el OpenAPI.
//! - [`cliente`]: el POST con tiempo (2 s), registro del cuerpo exacto,
//!   cache de lo identico y los fallos como errores (nunca como «no»).
//! - [`transporte`]: WinHTTP en Windows; un transporte grabado en las pruebas.
//! - [`claves`]: la clave, cifrada con DPAPI fuera de lo que se sincroniza.
//! - [`preguntas`]: **todo el texto de las preguntas** y los umbrales.
//! - Lo de cada funcion, con su tabla de decision: [`reunion`] (las partes
//!   importantes de una clase o reunion), [`anki`] (que tarjetas valen la
//!   pena), [`urgencia`] (la prioridad de una tarea) y [`modelo`] (que modelo
//!   contesta al asistente) y [`temas`] (que temas de la agenda ya se
//!   trataron, mientras se graba la reunion).
//! - Las ideas en vivo (29-oct-2026, probadas sin interfaz con los
//!   ejemplos de `examples/`, ver `docs/probar-ideas-sin-interfaz.md`):
//!   [`vivo`] (una peticion por frase que sirve a varias), [`fechas`] (los
//!   dias y horas dichos, en codigo), [`compromisos`] (tareas dichas),
//!   [`urgencia_viva`] (la prioridad que cambia al hablar),
//!   [`tiempo_de_temas`] (minutos por tema y el tema olvidado),
//!   [`pendientes`] (preguntas sin respuesta), [`citas`] (al horario),
//!   [`dictado`] (anotar en un chat) y [`repaso`] (repaso oral).
//!
//! Sin clave no se llama a nadie: quien usa esto sigue como siempre.

pub mod anki;
pub mod api;
pub mod citas;
pub mod claves;
pub mod cliente;
pub mod compromisos;
pub mod dictado;
pub mod fechas;
pub mod modelo;
pub mod pendientes;
pub mod preguntas;
pub mod repaso;
pub mod reunion;
pub mod temas;
pub mod tiempo_de_temas;
pub mod transporte;
pub mod urgencia;
pub mod urgencia_viva;
pub mod vivo;

pub use api::{Escala, Imagen, Peticion, Pregunta, Respuestas};
pub use cliente::Cliente;

/// **Por que no hubo respuesta.** Ninguno de estos es un «no»: quien llama
/// sigue con lo de siempre y lo apunta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No hay clave guardada.
    SinClave,
    /// No se llego a hablar con TypeSafe (red, DNS, TLS, tiempo).
    Red(String),
    /// TypeSafe contesto con un error (401 clave, 422 peticion, 429...).
    Http { codigo: u16, detalle: String },
    /// Lo que llego no se entiende, o la peticion no se pudo escribir.
    Formato(String),
    /// Falta la respuesta a una pregunta.
    FaltaRespuesta(String),
    /// La respuesta es de otro tipo que la pregunta.
    OtroTipo {
        nombre: String,
        esperado: &'static str,
        llego: &'static str,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::SinClave => write!(f, "falta la clave de TypeSafe"),
            Error::Red(e) => write!(f, "sin conexion con TypeSafe: {e}"),
            Error::Http { codigo: 401, .. } => write!(f, "TypeSafe no acepta la clave (401)"),
            Error::Http { codigo: 429, .. } => {
                write!(f, "TypeSafe pide esperar: limite de peticiones (429)")
            }
            Error::Http { codigo, detalle } => write!(f, "TypeSafe contesto {codigo}: {detalle}"),
            Error::Formato(e) => write!(f, "respuesta de TypeSafe que no se entiende: {e}"),
            Error::FaltaRespuesta(n) => write!(f, "TypeSafe no contesto «{n}»"),
            Error::OtroTipo {
                nombre,
                esperado,
                llego,
            } => write!(f, "«{nombre}»: se esperaba {esperado} y llego {llego}"),
        }
    }
}

impl std::error::Error for Error {}

/// Recorta un texto a `n` caracteres (con «…»), sin partir un caracter.
pub(crate) fn recortar(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut r: String = s.chars().take(n).collect();
        r.push('…');
        r
    }
}

/// Corre `f` sobre cada elemento con hasta `hilos` a la vez, en orden. El
/// primer error para lo que falte (lo hecho se tira: un resultado a medias
/// no se guarda como si fuera entero).
pub(crate) fn en_paralelo<T: Sync, R: Send>(
    cosas: &[T],
    hilos: usize,
    f: &(dyn Fn(&T) -> Result<R, Error> + Sync),
) -> Result<Vec<R>, Error> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let siguiente = AtomicUsize::new(0);
    let parar = AtomicBool::new(false);
    let salida: Mutex<Vec<Option<R>>> = Mutex::new((0..cosas.len()).map(|_| None).collect());
    let error: Mutex<Option<Error>> = Mutex::new(None);
    std::thread::scope(|s| {
        for _ in 0..hilos.clamp(1, 8).min(cosas.len().max(1)) {
            s.spawn(|| {
                loop {
                    if parar.load(Ordering::SeqCst) {
                        return;
                    }
                    let i = siguiente.fetch_add(1, Ordering::SeqCst);
                    let Some(c) = cosas.get(i) else { return };
                    match f(c) {
                        Ok(r) => {
                            if let Ok(mut v) = salida.lock() {
                                v[i] = Some(r);
                            }
                        }
                        Err(e) => {
                            parar.store(true, Ordering::SeqCst);
                            if let Ok(mut g) = error.lock() {
                                g.get_or_insert(e);
                            }
                            return;
                        }
                    }
                }
            });
        }
    });
    if let Some(e) = error.into_inner().ok().flatten() {
        return Err(e);
    }
    salida
        .into_inner()
        .map_err(|_| Error::Formato("un hilo se cayo".into()))?
        .into_iter()
        .map(|r| r.ok_or_else(|| Error::Formato("falto un resultado".into())))
        .collect()
}

#[cfg(test)]
pub(crate) mod grabadas {
    //! Respuestas grabadas para las pruebas, con la forma del OpenAPI.

    /// Una respuesta con estos Nouls (`nombre`, probabilidad).
    pub fn nouls(v: &[(&str, f64)]) -> String {
        respuesta(
            &v.iter()
                .map(|(n, p)| format!("\"{n}\":{{\"type\":\"noul\",\"noul\":{p}}}"))
                .collect::<Vec<_>>(),
        )
    }

    /// Un Score con la probabilidad de cada nivel.
    pub fn score(nombre: &str, niveles: &[f64]) -> String {
        let valor: f64 = niveles.iter().enumerate().map(|(i, p)| i as f64 * p).sum();
        let probs: Vec<String> = niveles
            .iter()
            .enumerate()
            .map(|(i, p)| format!("\"{i}\":{p}"))
            .collect();
        format!(
            "\"{nombre}\":{{\"type\":\"score\",\"score\":{valor},\"confidence\":0.6,\"legend\":{{}},\"probabilities\":{{{}}}}}",
            probs.join(",")
        )
    }

    /// Una Choice: `elegida` con 0.8 y el resto repartido entre `otras`.
    pub fn choice(nombre: &str, elegida: &str, otras: &[&str]) -> String {
        let resto = if otras.is_empty() {
            0.0
        } else {
            0.2 / otras.len() as f64
        };
        let mut probs = vec![format!("\"{elegida}\":0.8")];
        probs.extend(otras.iter().map(|o| format!("\"{o}\":{resto}")));
        format!(
            "\"{nombre}\":{{\"type\":\"choice\",\"choice\":\"{elegida}\",\"confidence\":0.6,\"probabilities\":{{{}}}}}",
            probs.join(",")
        )
    }

    pub fn noul(nombre: &str, p: f64) -> String {
        format!("\"{nombre}\":{{\"type\":\"noul\",\"noul\":{p}}}")
    }

    pub fn respuesta(partes: &[String]) -> String {
        format!(
            "{{\"model\":\"jev-1.14.0\",\"answers\":{{{}}},\"usage\":{{\"input_tokens\":180,\"output_tokens\":9}}}}",
            partes.join(",")
        )
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn en_paralelo_conserva_el_orden_y_corta_al_primer_error() {
        let v: Vec<u32> = (0..40).collect();
        let r = en_paralelo(&v, 4, &|x| Ok(x * 2)).unwrap();
        assert_eq!(r, v.iter().map(|x| x * 2).collect::<Vec<_>>());
        let e = en_paralelo(&v, 3, &|x| {
            if *x == 7 {
                Err(Error::Red("caida".into()))
            } else {
                Ok(*x)
            }
        });
        assert_eq!(e, Err(Error::Red("caida".into())));
        assert!(
            en_paralelo::<u32, u32>(&[], 4, &|x| Ok(*x))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn recortar_no_parte_caracteres() {
        assert_eq!(recortar("  ñandú  ", 10), "ñandú");
        assert_eq!(recortar("ñandúes", 3), "ñan…");
    }
}
