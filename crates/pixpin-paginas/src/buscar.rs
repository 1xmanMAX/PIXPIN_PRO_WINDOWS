//! **Los buscadores**: donde se pregunta y como se leen sus resultados.
//!
//! - **DuckDuckGo** (por omision, sin clave ni cuenta): su pagina HTML
//!   sin JavaScript (`html.duckduckgo.com/html/`), la que hace para los
//!   navegadores de texto. Gratis y sin registro; a cambio, si se le pregunta
//!   mucho seguido puede pedir comprobar que no es un robot
//!   ([`ErrorBusqueda::Bloqueado`]).
//! - **Brave Search** (con la clave del usuario, `PIXPIN_CLAVE_BRAVE`): su
//!   API oficial en JSON. Mas estable; tiene un plan gratis con clave.
//!
//! Lo que llega se trata como datos: titulo, direccion y resumen, nada mas.

use serde_json::Value;

use crate::direccion;
use crate::html::{Pieza, entidades, piezas};

/// A quien se pregunta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Buscador {
    DuckDuckGo,
    Brave,
}

impl Buscador {
    pub fn nombre(self) -> &'static str {
        match self {
            Buscador::DuckDuckGo => "DuckDuckGo",
            Buscador::Brave => "Brave Search",
        }
    }

    /// La direccion de la busqueda.
    pub fn url(self, consulta: &str, maximo: usize) -> String {
        let q = direccion::codificar(consulta.trim());
        match self {
            Buscador::DuckDuckGo => format!("https://html.duckduckgo.com/html/?q={q}"),
            Buscador::Brave => format!(
                "https://api.search.brave.com/res/v1/web/search?q={q}&count={}",
                maximo.clamp(1, 20)
            ),
        }
    }
}

/// Un resultado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resultado {
    pub titulo: String,
    pub url: String,
    pub resumen: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorBusqueda {
    /// El buscador pidio comprobar que no es un robot.
    Bloqueado,
    /// Contesto algo que no se entiende.
    Raro,
}

fn junta(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// La direccion de verdad de un enlace de DuckDuckGo
/// (`//duckduckgo.com/l/?uddg=<codificada>&rut=...`), o el enlace tal cual.
fn destino_ddg(href: &str) -> Option<String> {
    let h = href.trim();
    let h = if h.starts_with("//") {
        format!("https:{h}")
    } else if h.starts_with('/') {
        format!("https://duckduckgo.com{h}")
    } else {
        h.to_string()
    };
    let u = if let Some(i) = h.find("uddg=") {
        let v = &h[i + 5..];
        direccion::decodificar(v.split('&').next().unwrap_or(""), false)
    } else {
        h
    };
    let leida = direccion::leer(&u).ok()?;
    // Los anuncios (`duckduckgo.com/y.js`) y lo de la propia casa, fuera.
    if leida.dominio() == "duckduckgo.com" || leida.servidor.ends_with(".duckduckgo.com") {
        return None;
    }
    Some(u)
}

fn tiene_clase(p: &Pieza, clase: &str) -> bool {
    p.atributo("class")
        .is_some_and(|c| c.split_whitespace().any(|x| x == clase))
}

/// **Lee la pagina de resultados de DuckDuckGo.**
pub fn leer_duckduckgo(html: &str, maximo: usize) -> Result<Vec<Resultado>, ErrorBusqueda> {
    if html.contains("anomaly-modal") || html.contains("challenge-form") {
        return Err(ErrorBusqueda::Bloqueado);
    }
    #[derive(PartialEq)]
    enum Que {
        Titulo,
        Resumen,
    }
    let mut v: Vec<Resultado> = Vec::new();
    // Lo que se esta leyendo, la etiqueta que lo cierra y cuantas iguales
    // hay abiertas dentro.
    let mut leyendo: Option<(Que, String, usize)> = None;
    let mut anuncio = false;
    let mut actual: Option<Resultado> = None;
    for p in piezas(html) {
        match &p {
            Pieza::Abre { nombre, .. } => {
                if let Some((_, n, dentro)) = &mut leyendo {
                    if n == nombre {
                        *dentro += 1;
                    }
                    continue;
                }
                if tiene_clase(&p, "result") || tiene_clase(&p, "web-result") {
                    anuncio = tiene_clase(&p, "result--ad");
                }
                if tiene_clase(&p, "result__a") {
                    if let Some(r) = actual.take() {
                        v.push(r);
                    }
                    let url = p.atributo("href").and_then(destino_ddg);
                    if let Some(url) = url.filter(|_| !anuncio) {
                        actual = Some(Resultado {
                            titulo: String::new(),
                            url,
                            resumen: String::new(),
                        });
                        leyendo = Some((Que::Titulo, nombre.clone(), 0));
                    }
                } else if tiene_clase(&p, "result__snippet") && actual.is_some() {
                    leyendo = Some((Que::Resumen, nombre.clone(), 0));
                }
            }
            Pieza::Cierra(nombre) => {
                if let Some((_, n, dentro)) = &mut leyendo
                    && n == nombre
                {
                    if *dentro == 0 {
                        leyendo = None;
                    } else {
                        *dentro -= 1;
                    }
                }
            }
            Pieza::Texto(t) => {
                if let (Some((que, _, _)), Some(r)) = (&leyendo, &mut actual) {
                    let destino = if *que == Que::Titulo {
                        &mut r.titulo
                    } else {
                        &mut r.resumen
                    };
                    destino.push_str(&entidades(t));
                }
            }
        }
    }
    if let Some(r) = actual.take() {
        v.push(r);
    }
    Ok(pulir(v, maximo))
}

/// **Lee la respuesta de Brave Search** (`web.results[]`).
pub fn leer_brave(json: &str, maximo: usize) -> Result<Vec<Resultado>, ErrorBusqueda> {
    let v: Value = serde_json::from_str(json).map_err(|_| ErrorBusqueda::Raro)?;
    let Some(lista) = v.pointer("/web/results").and_then(Value::as_array) else {
        // Sin resultados de la web (una consulta rara): lista vacia.
        return if v.get("type").is_some() {
            Ok(Vec::new())
        } else {
            Err(ErrorBusqueda::Raro)
        };
    };
    let sin_etiquetas = |s: &str| {
        piezas(s)
            .into_iter()
            .filter_map(|p| match p {
                Pieza::Texto(t) => Some(entidades(t)),
                _ => None,
            })
            .collect::<String>()
    };
    let r = lista
        .iter()
        .filter_map(|x| {
            let url = x.get("url")?.as_str()?.to_string();
            direccion::leer(&url).ok()?;
            Some(Resultado {
                titulo: sin_etiquetas(x.get("title").and_then(Value::as_str).unwrap_or("")),
                url,
                resumen: sin_etiquetas(x.get("description").and_then(Value::as_str).unwrap_or("")),
            })
        })
        .collect();
    Ok(pulir(r, maximo))
}

/// Espacios juntados, sin repetidos ni vacios, y no mas de `maximo`.
fn pulir(v: Vec<Resultado>, maximo: usize) -> Vec<Resultado> {
    let mut o: Vec<Resultado> = Vec::new();
    for mut r in v {
        r.titulo = junta(&r.titulo);
        r.resumen = junta(&r.resumen);
        if r.titulo.is_empty() {
            r.titulo = r.url.clone();
        }
        if !o.iter().any(|x| x.url == r.url) {
            o.push(r);
        }
        if o.len() >= maximo {
            break;
        }
    }
    o
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const DDG: &str = r##"<div class="result results_links results_links_deep result--ad">
      <h2 class="result__title"><a class="result__a" href="https://duckduckgo.com/y.js?ad_domain=x.com&amp;u3=1">Anuncio</a></h2>
      <a class="result__snippet" href="#">Compra ya</a></div>
    <div class="result results_links results_links_deep web-result ">
      <h2 class="result__title"><a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.ugr.es%2Festudiantes%2Fhorarios&amp;rut=abc">Horarios &amp; aulas | <b>UGR</b></a></h2>
      <div class="result__extras"><a class="result__url" href="x">www.ugr.es</a></div>
      <a class="result__snippet" href="//duckduckgo.com/l/?uddg=x">Consulta los <b>horarios</b> de
         grado del curso 2026/27.</a></div>
    <div class="result web-result"><h2><a class="result__a" href="https://es.wikipedia.org/wiki/Universidad_de_Granada">Universidad de Granada - Wikipedia</a></h2>
      <div class="result__snippet">La <span>UGR</span> es...</div></div>
    <div class="result web-result"><h2><a class="result__a" href="http://192.168.1.1/">trampa</a></h2></div>"##;

    #[test]
    fn lee_duckduckgo_sin_anuncios_ni_direcciones_locales() {
        let r = leer_duckduckgo(DDG, 10).unwrap();
        assert_eq!(r.len(), 2, "{r:?}");
        assert_eq!(r[0].titulo, "Horarios & aulas | UGR");
        assert_eq!(r[0].url, "https://www.ugr.es/estudiantes/horarios");
        assert_eq!(
            r[0].resumen,
            "Consulta los horarios de grado del curso 2026/27."
        );
        assert_eq!(
            r[1].url,
            "https://es.wikipedia.org/wiki/Universidad_de_Granada"
        );
        assert_eq!(r[1].resumen, "La UGR es...");
        assert_eq!(leer_duckduckgo(DDG, 1).unwrap().len(), 1);
    }

    #[test]
    fn duckduckgo_que_pide_captcha_lo_dice() {
        let html =
            r#"<form id="challenge-form"><div class="anomaly-modal__title">...</div></form>"#;
        assert_eq!(leer_duckduckgo(html, 5), Err(ErrorBusqueda::Bloqueado));
        assert_eq!(
            leer_duckduckgo("<html><body>Sin nada</body></html>", 5),
            Ok(vec![])
        );
    }

    #[test]
    fn lee_brave() {
        let json = r#"{"type":"search","web":{"results":[
            {"title":"Horarios <strong>UGR</strong>","url":"https://www.ugr.es/h","description":"Los <strong>horarios</strong> &amp; aulas"},
            {"title":"Local","url":"http://10.0.0.1/","description":"x"},
            {"title":"Repetido","url":"https://www.ugr.es/h","description":"y"}]}}"#;
        let r = leer_brave(json, 10).unwrap();
        assert_eq!(
            r,
            vec![Resultado {
                titulo: "Horarios UGR".into(),
                url: "https://www.ugr.es/h".into(),
                resumen: "Los horarios & aulas".into()
            }]
        );
        assert_eq!(leer_brave(r#"{"type":"search"}"#, 5), Ok(vec![]));
        assert_eq!(leer_brave("no es json", 5), Err(ErrorBusqueda::Raro));
    }

    #[test]
    fn las_direcciones_de_busqueda() {
        assert_eq!(
            Buscador::DuckDuckGo.url(" horario ugr ", 5),
            "https://html.duckduckgo.com/html/?q=horario+ugr"
        );
        assert_eq!(
            Buscador::Brave.url("a&b", 50),
            "https://api.search.brave.com/res/v1/web/search?q=a%26b&count=20"
        );
    }
}
