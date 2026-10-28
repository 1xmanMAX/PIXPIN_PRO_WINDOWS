//! **El texto legible de una pagina** (a lo «modo lectura»): lo que el
//! modelo necesita para contestar, sin menus ni anuncios ni codigo.
//!
//! - Fuera: `<script>`, `<style>`, `<noscript>`, `<template>`, `<svg>`,
//!   `<iframe>`, formularios sueltos (`<select>`, `<button>`), la navegacion
//!   (`<nav>`, `role=navigation`), `<aside>`, `<footer>`, la cabecera del
//!   sitio (`<header>` fuera del articulo) y lo oculto (`hidden`,
//!   `aria-hidden=true`, `display:none`).
//! - Si hay `<main>` o `<article>` con texto de verdad, solo eso.
//! - Se quedan: los titulos (`# `, `## `...), las listas (`- `), las
//!   **tablas** (una fila por linea, `| a | b |`: un horario es una tabla),
//!   el texto preformateado y los enlaces, marcados `[n]` y listados al
//!   final con su direccion entera (para abrir el siguiente o citarlo).

use crate::direccion::{self, Url};
use crate::html::{Pieza, VACIAS, entidades, piezas};

/// Cuantos enlaces se listan como mucho.
pub const MAX_ENLACES: usize = 80;

/// Un enlace de la pagina.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enlace {
    pub texto: String,
    pub url: String,
}

/// Lo leido de una pagina.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pagina {
    pub titulo: String,
    pub texto: String,
    pub enlaces: Vec<Enlace>,
}

/// Lo que nunca se lee.
const FUERA: &[&str] = &[
    "script", "style", "noscript", "template", "svg", "math", "iframe", "object", "canvas",
    "select", "button", "textarea", "nav", "aside", "footer", "dialog", "head",
];

/// Lo que es un bloque: salto de linea antes y despues.
const BLOQUES: &[&str] = &[
    "div",
    "section",
    "article",
    "main",
    "ul",
    "ol",
    "dl",
    "dt",
    "dd",
    "figure",
    "figcaption",
    "address",
    "details",
    "summary",
    "fieldset",
    "legend",
    "caption",
    "form",
    "center",
    "header",
    "body",
    "label",
    "option",
];

/// Lo que es un parrafo: linea en blanco antes y despues.
const PARRAFOS: &[&str] = &["p", "blockquote", "table", "pre"];

/// El texto que se va escribiendo, con los espacios ya juntados.
#[derive(Default)]
struct Salida {
    s: String,
}

impl Salida {
    fn texto(&mut self, t: &str, pre: bool) {
        if pre {
            self.s.push_str(t);
            return;
        }
        for (i, palabra) in t.split_whitespace().enumerate() {
            if (i > 0 || t.starts_with(char::is_whitespace)) && self.cabe_espacio() {
                self.s.push(' ');
            }
            self.s.push_str(palabra);
        }
        if t.ends_with(char::is_whitespace) && self.cabe_espacio() {
            self.s.push(' ');
        }
    }

    fn cabe_espacio(&self) -> bool {
        !self.s.is_empty() && !self.s.ends_with([' ', '\n'])
    }

    fn crudo(&mut self, t: &str) {
        self.s.push_str(t);
    }

    fn salto(&mut self) {
        let sin = self.s.trim_end_matches(' ').len();
        self.s.truncate(sin);
        if !self.s.is_empty() && !self.s.ends_with('\n') {
            self.s.push('\n');
        }
    }

    fn parrafo(&mut self) {
        self.salto();
        if !self.s.is_empty() && !self.s.ends_with("\n\n") {
            self.s.push('\n');
        }
    }
}

/// Las dos salidas: la pagina entera y lo de `<main>`/`<article>`.
#[derive(Default)]
struct Doble {
    todo: Salida,
    principal: Salida,
    dentro: usize,
}

impl Doble {
    fn con(&mut self, f: impl Fn(&mut Salida)) {
        f(&mut self.todo);
        if self.dentro > 0 {
            f(&mut self.principal);
        }
    }
}

fn oculto(p: &Pieza) -> bool {
    if p.atributo("hidden").is_some() || p.atributo("aria-hidden") == Some("true") {
        return true;
    }
    if let Some(r) = p.atributo("role")
        && matches!(
            r,
            "navigation" | "banner" | "contentinfo" | "search" | "menu" | "menubar"
        )
    {
        return true;
    }
    p.atributo("style").is_some_and(|s| {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        let s = s.to_ascii_lowercase();
        s.contains("display:none") || s.contains("visibility:hidden")
    })
}

/// **Lee una pagina HTML.** `base`: su direccion (para los enlaces).
pub fn de_html(html: &str, base: &Url) -> Pagina {
    let mut d = Doble::default();
    let mut titulo = String::new();
    let mut en_titulo = false;
    // Lo que no se lee, con su nombre (para saber cuando acaba).
    let mut fuera: Vec<String> = Vec::new();
    let mut pre = 0usize;
    let mut celdas = 0usize;
    let mut enlaces: Vec<Enlace> = Vec::new();
    // El enlace abierto: su direccion y su texto.
    let mut enlace: Option<(String, String)> = None;
    for p in piezas(html) {
        match &p {
            Pieza::Abre { nombre, .. }
                if nombre == "title" && fuera.iter().all(|f| f == "head") =>
            {
                en_titulo = true;
            }
            Pieza::Cierra(n) if n == "title" => en_titulo = false,
            Pieza::Texto(t) if en_titulo => titulo.push_str(t),
            Pieza::Abre { nombre, .. } if !fuera.is_empty() => {
                if !VACIAS.contains(&nombre.as_str()) && FUERA.contains(&nombre.as_str()) {
                    fuera.push(nombre.clone());
                }
            }
            Pieza::Cierra(n) if !fuera.is_empty() => {
                if let Some(i) = fuera.iter().rposition(|f| f == n) {
                    fuera.truncate(i);
                }
                if n == "head" {
                    fuera.retain(|f| f != "head");
                }
            }
            Pieza::Texto(_) if !fuera.is_empty() => {}
            Pieza::Abre { nombre, .. } => {
                let n = nombre.as_str();
                let vacia = VACIAS.contains(&n);
                if !vacia && (FUERA.contains(&n) || (n == "header" && d.dentro == 0) || oculto(&p))
                {
                    fuera.push(nombre.clone());
                    continue;
                }
                match n {
                    "main" | "article" => {
                        d.dentro += 1;
                        d.con(Salida::parrafo);
                    }
                    "br" if celdas > 0 => d.con(|s| s.texto(" ", false)),
                    "br" => d.con(Salida::salto),
                    "hr" => d.con(|s| {
                        s.parrafo();
                        s.crudo("---");
                        s.parrafo();
                    }),
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let nivel = n[1..].parse::<usize>().unwrap_or(1);
                        d.con(|s| {
                            s.parrafo();
                            s.crudo(&"#".repeat(nivel));
                            s.crudo(" ");
                        });
                    }
                    "li" => d.con(|s| {
                        s.salto();
                        s.crudo("- ");
                    }),
                    "tr" => d.con(|s| {
                        s.salto();
                        s.crudo("|");
                    }),
                    "td" | "th" => {
                        celdas += 1;
                        d.con(|s| s.crudo(" "));
                    }
                    "pre" => {
                        pre += 1;
                        d.con(Salida::parrafo);
                    }
                    "a" => {
                        if let Some(h) = p.atributo("href")
                            && let Some(u) = direccion::resolver(base, h)
                        {
                            enlace = Some((u, String::new()));
                        }
                    }
                    "img" => {
                        // Una imagen que es el unico texto de un enlace.
                        if let Some((_, t)) = &mut enlace
                            && t.trim().is_empty()
                            && let Some(alt) = p.atributo("alt").filter(|a| !a.trim().is_empty())
                        {
                            t.push_str(alt.trim());
                            let alt = alt.trim().to_string();
                            d.con(|s| s.texto(&alt, false));
                        }
                    }
                    _ if celdas > 0 && (BLOQUES.contains(&n) || PARRAFOS.contains(&n)) => {
                        d.con(|s| s.texto(" ", false));
                    }
                    _ if PARRAFOS.contains(&n) => d.con(Salida::parrafo),
                    _ if BLOQUES.contains(&n) => d.con(Salida::salto),
                    _ => {}
                }
            }
            Pieza::Cierra(n) => match n.as_str() {
                "main" | "article" => {
                    d.con(Salida::parrafo);
                    d.dentro = d.dentro.saturating_sub(1);
                }
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => d.con(Salida::parrafo),
                "td" | "th" => {
                    celdas = celdas.saturating_sub(1);
                    d.con(|s| {
                        let sin = s.s.trim_end_matches(' ').len();
                        s.s.truncate(sin);
                        s.crudo(" |");
                    });
                }
                "tr" => d.con(Salida::salto),
                "pre" => {
                    pre = pre.saturating_sub(1);
                    d.con(Salida::parrafo);
                }
                "a" => {
                    if let Some((url, texto)) = enlace.take() {
                        let texto = texto.split_whitespace().collect::<Vec<_>>().join(" ");
                        if !texto.is_empty() {
                            let n = match enlaces.iter().position(|e| e.url == url) {
                                Some(i) => Some(i + 1),
                                None if enlaces.len() < MAX_ENLACES => {
                                    enlaces.push(Enlace { texto, url });
                                    Some(enlaces.len())
                                }
                                None => None,
                            };
                            if let Some(n) = n {
                                d.con(|s| {
                                    let sin = s.s.trim_end_matches(' ').len();
                                    s.s.truncate(sin);
                                    s.crudo(&format!(" [{n}]"));
                                });
                            }
                        }
                    }
                }
                x if celdas > 0 && (BLOQUES.contains(&x) || PARRAFOS.contains(&x)) => {
                    d.con(|s| s.texto(" ", false));
                }
                x if PARRAFOS.contains(&x) => d.con(Salida::parrafo),
                x if BLOQUES.contains(&x) || x == "li" => d.con(Salida::salto),
                _ => {}
            },
            Pieza::Texto(t) => {
                let t = entidades(t);
                if let Some((_, et)) = &mut enlace {
                    et.push_str(&t);
                    et.push(' ');
                }
                let en_pre = pre > 0;
                d.con(|s| s.texto(&t, en_pre));
            }
        }
    }
    let principal = limpiar(&d.principal.s);
    let texto = if principal.chars().count() >= 200 {
        principal
    } else {
        limpiar(&d.todo.s)
    };
    Pagina {
        titulo: titulo.split_whitespace().collect::<Vec<_>>().join(" "),
        texto,
        enlaces,
    }
}

/// Quita lo que sobra: espacios al final, filas de tabla vacias, lineas en
/// blanco de mas.
pub fn limpiar(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    let mut blancas = 0;
    for linea in s.lines() {
        let l = linea.trim_end();
        let vacia = l.trim().is_empty()
            || l.trim() == "-"
            || l.chars().all(|c| c == '|' || c.is_whitespace());
        if vacia {
            blancas += 1;
            continue;
        }
        if !o.is_empty() {
            o.push('\n');
            if blancas > 0 {
                o.push('\n');
            }
        }
        blancas = 0;
        o.push_str(l);
    }
    o
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn base() -> Url {
        direccion::leer("https://www.uni.edu/estudios/grado.html").unwrap()
    }

    #[test]
    fn se_queda_con_el_articulo_y_tira_menus_y_codigo() {
        let html = r#"<!DOCTYPE html><html><head><title>Horarios — Uni</title>
            <style>body{color:red}</style><script>var x = "<p>no</p>";</script></head>
            <body><header><a href="/">Inicio</a> <a href="/login">Entrar</a></header>
            <nav><ul><li><a href="/a">Menu A</a></li></ul></nav>
            <main><h1>Horario de 1º</h1>
            <p>Las clases empiezan el <b>15 de septiembre</b>. Consulta las
               aulas en la <a href="aulas.pdf">lista de aulas</a>.</p>
            <p>Texto de relleno para que el articulo tenga suficiente texto y se elija
               como lo principal de la pagina en vez del resto, que es menu.</p>
            <div hidden>Oculto</div><span style="display: none">Tambien oculto</span>
            </main>
            <aside>Publicidad</aside><footer>© Uni</footer></body></html>"#;
        let p = de_html(html, &base());
        assert_eq!(p.titulo, "Horarios — Uni");
        assert!(p.texto.starts_with("# Horario de 1º"), "{}", p.texto);
        assert!(
            p.texto.contains("empiezan el 15 de septiembre. Consulta"),
            "{}",
            p.texto
        );
        assert!(p.texto.contains("lista de aulas [1]."), "{}", p.texto);
        for fuera in [
            "Menu A",
            "Entrar",
            "Publicidad",
            "© Uni",
            "var x",
            "color:red",
            "Oculto",
            "no</p>",
        ] {
            assert!(!p.texto.contains(fuera), "{fuera} en {}", p.texto);
        }
        assert_eq!(
            p.enlaces,
            vec![Enlace {
                texto: "lista de aulas".into(),
                url: "https://www.uni.edu/estudios/aulas.pdf".into()
            }]
        );
    }

    #[test]
    fn las_tablas_salen_fila_a_fila() {
        let html = "<table><thead><tr><th>Hora</th><th>Lunes</th><th>Martes</th></tr></thead>
            <tbody><tr><td>8:00</td><td><p>Cálculo</p><br>Aula 3</td><td></td></tr>
            <tr><td>10:00</td><td>Física&nbsp;I</td><td>Química</td></tr></tbody></table>";
        let p = de_html(html, &base());
        assert_eq!(
            p.texto,
            "| Hora | Lunes | Martes |\n| 8:00 | Cálculo Aula 3 | |\n| 10:00 | Física I | Química |",
            "{}",
            p.texto
        );
    }

    #[test]
    fn listas_titulos_y_pre() {
        let html = "<h2>Pasos</h2><ol><li>Uno</li><li>Dos <a href='https://x.com/d'>aqui</a></li></ol>\
                    <pre>a   b\n  c</pre><p>fin</p>";
        let p = de_html(html, &base());
        assert_eq!(
            p.texto, "## Pasos\n\n- Uno\n- Dos aqui [1]\n\na   b\n  c\n\nfin",
            "{}",
            p.texto
        );
    }

    #[test]
    fn sin_main_lee_el_cuerpo_entero_y_repite_el_numero_del_mismo_enlace() {
        let html = "<body><div>Hola <a href='/x'>x</a></div><div>otra vez <a href='/x'>x</a></div>\
                    <a href='mailto:a@b.c'>correo</a> <a href='#top'>arriba</a></body>";
        let p = de_html(html, &base());
        assert_eq!(p.texto, "Hola x [1]\notra vez x [1]\ncorreo arriba");
        assert_eq!(p.enlaces.len(), 1);
    }

    #[test]
    fn no_pasa_de_los_enlaces_maximos() {
        let mut html = String::new();
        for i in 0..200 {
            html.push_str(&format!("<p><a href='/p{i}'>p{i}</a></p>"));
        }
        let p = de_html(&html, &base());
        assert_eq!(p.enlaces.len(), MAX_ENLACES);
        assert!(p.texto.contains("p199"));
    }
}
