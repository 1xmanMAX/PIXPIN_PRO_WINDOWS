//! **Las listas de tareas para sacarlas fuera** (1-nov-2026, el usuario:
//! «mejora la apariencia de mis quehaceres cuando los exporto; ahora es
//! solo una lista plana y queda un poco tonto: quiero algo mas cuidado»).
//!
//! Antes una lista salia por la hoja de compartir como texto corrido
//! (`☐ llamar al banco ⏫ #casa ⏰ 2026-10-17 17:00 ➕ 2026-10-17`, con los
//! enlaces de sus fotos a la vista). Aqui esta lo puro de sacarla bien:
//!
//! - [`ListaParaExportar`]: lo que lleva cada tarea **ya entendido** con
//!   los mismos lectores que la ventana de Tareas (`mini::partir`,
//!   `mini::imagenes_de` y `pindesk_frase::tarea::marcas`): su titulo
//!   limpio, la urgencia, las etiquetas, la nota, el recordatorio, el dia en
//!   que se creo y sus fotos. **Ni un dato inventado**: solo lo que ya va en
//!   el texto de la tarea.
//! - [`html`]: una pagina sola, sin nada de la red, en la linea de la de las
//!   grabaciones (`clase_exportar/pagina.css`): titulo y fecha, el resumen
//!   con su barrita, cada lista en su tarjeta con sus tareas en filas
//!   (casilla, chapitas de urgencia, recordatorio, etiquetas y nota) y lo
//!   hecho plegado al final. Claro y oscuro segun el sistema, para el movil
//!   y para imprimir.
//! - [`hojas`]: lo mismo dibujado en hojas A4 del motor, de donde salen el
//!   PDF, el PNG, el JPG y el SVG de la hoja de compartir.
//! - [`markdown`] (de ahi sale tambien el Word) y [`texto`].
//!
//! Nada de esto toca el disco ni sabe de idiomas: las fotos las da quien
//! llama y los rotulos vienen en [`Rotulos`].

use pindesk_frase::fechas::{self, Momento};
use pindesk_frase::tarea::{self, Urgencia};
use pindesk_motor2d::exportar::{self as ex, Hoja};
use pindesk_motor2d::pintado::Orden;
use pindesk_motor2d::vector::Punto2;
use pindesk_motor2d::{ColorRgba, EstiloTrazo};

use crate::clase_exportar::escapar;
use crate::mini;

// --- Lo que se saca -------------------------------------------------------------

/// **Una tarea ya entendida.**
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TareaParaExportar {
    /// Lo que hay que hacer, sin marcas ni fotos.
    pub titulo: String,
    pub hecha: bool,
    pub urgencia: Urgencia,
    /// Sin la almohadilla.
    pub etiquetas: Vec<String>,
    /// Cuando recordarla (`⏰`), en hora local.
    pub hora: Option<Momento>,
    /// La nota de por que no se esta haciendo (`💬`).
    pub nota: Option<String>,
    /// El dia en que se apunto (`➕`).
    pub creada: Option<mini::Fecha>,
    /// Los enlaces de sus fotos (`pixpin:files/…`), en orden.
    pub imagenes: Vec<String>,
}

impl TareaParaExportar {
    /// La de un texto guardado de tarea (lo que va tras `- [ ] `).
    pub fn del_texto(crudo: &str, hecha: bool) -> TareaParaExportar {
        let (visible, creada) = mini::partir(crudo);
        let (limpio, imagenes) = mini::imagenes_de(visible);
        let f = tarea::marcas(&limpio);
        TareaParaExportar {
            titulo: f.titulo,
            hecha,
            urgencia: f.urgencia,
            etiquetas: f.etiquetas,
            hora: f.hora,
            nota: f.nota,
            creada,
            imagenes,
        }
    }
}

/// **Una lista de tareas** (la mini-app `tareas` de un mensaje).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListaParaExportar {
    pub titulo: String,
    /// En el orden del documento.
    pub tareas: Vec<TareaParaExportar>,
}

impl ListaParaExportar {
    /// La del documento de la mini-app; sin titulo propio, `titulo`.
    pub fn del_documento(documento: &str, titulo: &str) -> ListaParaExportar {
        let propio = mini::titulo(documento);
        ListaParaExportar {
            titulo: if propio.trim().is_empty() {
                titulo.trim().to_string()
            } else {
                propio
            },
            tareas: mini::leer_tareas(documento)
                .iter()
                .map(|t| TareaParaExportar::del_texto(&t.texto, t.hecha))
                .collect(),
        }
    }

    /// Lo que queda por hacer, en su orden.
    pub fn pendientes(&self) -> impl Iterator<Item = &TareaParaExportar> {
        self.tareas.iter().filter(|t| !t.hecha)
    }

    /// Lo hecho, en su orden.
    pub fn hechas(&self) -> impl Iterator<Item = &TareaParaExportar> {
        self.tareas.iter().filter(|t| t.hecha)
    }

    /// `(hechas, total)`.
    pub fn cuenta(&self) -> (usize, usize) {
        (self.hechas().count(), self.tareas.len())
    }
}

/// `(hechas, total)` de varias listas juntas.
pub fn cuenta(listas: &[ListaParaExportar]) -> (usize, usize) {
    listas.iter().fold((0, 0), |(h, t), l| {
        let (a, b) = l.cuenta();
        (h + a, t + b)
    })
}

/// Lo hecho en tanto por ciento, redondeado (0 sin tareas).
pub fn por_ciento(hechas: usize, total: usize) -> u32 {
    if total == 0 {
        0
    } else {
        ((hechas as f64 / total as f64) * 100.0).round() as u32
    }
}

// --- Los rotulos -----------------------------------------------------------------

/// **Lo que dicen las paginas**, en el idioma de la app. Los patrones llevan
/// sus huecos entre llaves (`{fecha}`, `{dia}`...), que se llenan aqui.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rotulos {
    /// `es`, `en`... para el `lang` de la pagina.
    pub idioma: String,
    /// El titulo cuando no hay otro.
    pub tareas: String,
    /// «Exportado el {fecha}».
    pub exportado: String,
    pub pendientes: String,
    pub hechas: String,
    /// La cifra del avance («Completado»).
    pub completado: String,
    /// Lo que dice la casilla a quien no la ve.
    pub pendiente: String,
    pub hecha: String,
    /// Una lista sin nada pendiente.
    pub todo_hecho: String,
    /// Una lista sin tareas.
    pub vacia: String,
    pub urgente: String,
    pub muy_urgente: String,
    pub recordatorio: String,
    /// La chapita de un recordatorio que ya paso sin hacerla.
    pub pasada: String,
    /// «Creada el {fecha}».
    pub creada: String,
    pub hoy: String,
    pub manana: String,
    pub ayer: String,
    /// «{hechas} de {total}».
    pub de: String,
    /// «Foto», para el `alt` de las fotos.
    pub foto: String,
    /// lun ... dom.
    pub dias: [String; 7],
    /// enero ... diciembre.
    pub meses: [String; 12],
    /// «{dia} de {mes}».
    pub fecha: String,
    /// «{dia} de {mes} de {ano}».
    pub fecha_con_ano: String,
}

impl Default for Rotulos {
    fn default() -> Self {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        Rotulos {
            idioma: "es".into(),
            tareas: "Tareas".into(),
            exportado: "Exportado el {fecha}".into(),
            pendientes: "Pendientes".into(),
            hechas: "Hechas".into(),
            completado: "Completado".into(),
            pendiente: "Pendiente".into(),
            hecha: "Hecha".into(),
            todo_hecho: "Todo hecho".into(),
            vacia: "Sin tareas".into(),
            urgente: "Urgente".into(),
            muy_urgente: "Muy urgente".into(),
            recordatorio: "Recordatorio".into(),
            pasada: "Pasada".into(),
            creada: "Creada el {fecha}".into(),
            hoy: "Hoy".into(),
            manana: "Mañana".into(),
            ayer: "Ayer".into(),
            de: "{hechas} de {total}".into(),
            foto: "Foto".into(),
            dias: s(&["lun", "mar", "mié", "jue", "vie", "sáb", "dom"])
                .try_into()
                .unwrap_or_default(),
            meses: s(&[
                "enero",
                "febrero",
                "marzo",
                "abril",
                "mayo",
                "junio",
                "julio",
                "agosto",
                "septiembre",
                "octubre",
                "noviembre",
                "diciembre",
            ])
            .try_into()
            .unwrap_or_default(),
            fecha: "{dia} de {mes}".into(),
            fecha_con_ano: "{dia} de {mes} de {ano}".into(),
        }
    }
}

impl Rotulos {
    /// Lo que dice la chapita de una urgencia; normal no lleva.
    pub fn urgencia(&self, u: Urgencia) -> Option<&str> {
        match u {
            Urgencia::MuyUrgente => Some(&self.muy_urgente),
            Urgencia::Urgente => Some(&self.urgente),
            Urgencia::Normal => None,
        }
    }

    /// Un dia del calendario: sin el año si es el de `ahora`.
    pub fn dia(&self, dia: i64, ahora: Momento) -> String {
        let mismo_ano = fechas::fecha_de(dia).0 == fechas::fecha_de(ahora.dia).0;
        self.fecha_de(dia, !mismo_ano)
    }

    /// Un dia del calendario con su patron, con el año o sin el.
    fn fecha_de(&self, dia: i64, con_ano: bool) -> String {
        let (y, m, d) = fechas::fecha_de(dia);
        let mes = &self.meses[(m - 1).clamp(0, 11) as usize];
        let patron = if con_ano {
            &self.fecha_con_ano
        } else {
            &self.fecha
        };
        patron
            .replace("{dia}", &d.to_string())
            .replace("{mes}", mes)
            .replace("{ano}", &y.to_string())
    }

    /// El dia de la semana corto («vie»).
    fn semana(&self, dia: i64) -> &str {
        &self.dias[fechas::dia_de_la_semana(dia).clamp(0, 6) as usize]
    }

    /// «{hechas} de {total}».
    pub fn de(&self, hechas: usize, total: usize) -> String {
        self.de
            .replace("{hechas}", &hechas.to_string())
            .replace("{total}", &total.to_string())
    }

    /// «Exportado el 1 de noviembre de 2026 · 09:12».
    pub fn exportado(&self, ahora: Momento) -> String {
        let dia = self.fecha_de(ahora.dia, true);
        self.exportado
            .replace("{fecha}", &format!("{dia} · {}", hora_de(ahora.minuto)))
    }

    /// «Creada el 28 de octubre».
    pub fn creada(&self, f: mini::Fecha, ahora: Momento) -> String {
        self.creada.replace("{fecha}", &self.dia(f.dias(), ahora))
    }
}

/// `09:05`.
pub fn hora_de(minuto: u32) -> String {
    format!("{:02}:{:02}", minuto / 60, minuto % 60)
}

/// **Lo que dice el recordatorio**, de cerca y de lejos: `(cerca, lejos)`.
/// Hoy, mañana y ayer van por su nombre y llevan la fecha aparte («Mañana
/// 17:00», «2 de noviembre»); lo demas, con su dia de la semana («vie 6 de
/// noviembre 09:00») y sin nada aparte.
pub fn recordatorio(h: Momento, ahora: Momento, r: &Rotulos) -> (String, Option<String>) {
    let hora = hora_de(h.minuto);
    let cerca = match h.dia - ahora.dia {
        0 => Some(&r.hoy),
        1 => Some(&r.manana),
        -1 => Some(&r.ayer),
        _ => None,
    };
    match cerca {
        Some(c) => (format!("{c} {hora}"), Some(r.dia(h.dia, ahora))),
        None => (
            format!("{} {} {hora}", r.semana(h.dia), r.dia(h.dia, ahora)),
            None,
        ),
    }
}

/// Si el recordatorio ya paso y la tarea sigue sin hacer.
pub fn pasada(t: &TareaParaExportar, ahora: Momento) -> bool {
    !t.hecha && t.hora.is_some_and(|h| h < ahora)
}

/// La fecha y hora de un momento para `<time datetime>`.
fn iso(h: Momento) -> String {
    let (y, m, d) = fechas::fecha_de(h.dia);
    format!("{y:04}-{m:02}-{d:02}T{}", hora_de(h.minuto))
}

// --- HTML ------------------------------------------------------------------------

const CSS: &str = include_str!("tareas_exportar/pagina.css");

/// La casilla: un cuadrado redondeado y, si esta hecha, su marca.
const MARCA_SVG: &str =
    "<svg viewBox=\"0 0 16 16\" aria-hidden=\"true\"><path d=\"M3.6 8.4l2.9 2.9 5.9-6.6\"/></svg>";
/// El reloj del recordatorio.
const RELOJ_SVG: &str = "<svg viewBox=\"0 0 16 16\" aria-hidden=\"true\"><circle cx=\"8\" cy=\"8.6\" r=\"5.6\"/><path d=\"M8 5.6v3.2l2 1.3M3.2 2.6 1.8 4M12.8 2.6 14.2 4\"/></svg>";

/// Una tarea como fila. `foto` da el `src` de cada foto (una `data:` ya
/// reducida); `None`, no va.
fn fila_html(
    t: &TareaParaExportar,
    r: &Rotulos,
    ahora: Momento,
    foto: &dyn Fn(&str) -> Option<String>,
) -> String {
    let mut clases = vec!["tarea"];
    if t.hecha {
        clases.push("hecha");
    }
    match t.urgencia {
        Urgencia::MuyUrgente => clases.push("u-muy"),
        Urgencia::Urgente => clases.push("u-alta"),
        Urgencia::Normal => {}
    }
    let mut h = format!(
        "<li class=\"{}\"><span class=\"casilla\" role=\"img\" aria-label=\"{}\">{}</span><div class=\"que\">",
        clases.join(" "),
        escapar(if t.hecha { &r.hecha } else { &r.pendiente }),
        if t.hecha { MARCA_SVG } else { "" }
    );
    let titulo = if t.titulo.trim().is_empty() {
        "—".to_string()
    } else {
        escapar(t.titulo.trim())
    };
    h.push_str(&format!("<p class=\"titulo\">{titulo}</p>"));
    let mut chips = Vec::new();
    if !t.hecha
        && let Some(u) = r.urgencia(t.urgencia)
    {
        let c = if t.urgencia == Urgencia::MuyUrgente {
            "muy"
        } else {
            "alta"
        };
        chips.push(format!(
            "<span class=\"chip prio {c}\"><i aria-hidden=\"true\"></i>{}</span>",
            escapar(u)
        ));
    }
    if let Some(hora) = t.hora {
        let (cerca, lejos) = recordatorio(hora, ahora, r);
        let pasada = pasada(t, ahora);
        let mut c = format!(
            "<span class=\"chip hora{}\" title=\"{}\">{RELOJ_SVG}<time datetime=\"{}\">{}</time>",
            if pasada { " pasada" } else { "" },
            escapar(&r.recordatorio),
            iso(hora),
            escapar(&cerca)
        );
        if let Some(l) = lejos {
            c.push_str(&format!("<span class=\"lejos\">{}</span>", escapar(&l)));
        }
        if pasada {
            c.push_str(&format!("<b>{}</b>", escapar(&r.pasada)));
        }
        c.push_str("</span>");
        chips.push(c);
    }
    for e in &t.etiquetas {
        chips.push(format!("<span class=\"chip tag\">#{}</span>", escapar(e)));
    }
    if let Some(f) = t.creada {
        chips.push(format!(
            "<span class=\"creada\">{}</span>",
            escapar(&r.creada(f, ahora))
        ));
    }
    if !chips.is_empty() {
        h.push_str(&format!("<div class=\"chips\">{}</div>", chips.concat()));
    }
    if let Some(n) = t.nota.as_deref().filter(|n| !n.trim().is_empty()) {
        h.push_str(&format!("<p class=\"nota\">{}</p>", escapar(n.trim())));
    }
    let fotos: Vec<String> = t.imagenes.iter().filter_map(|e| foto(e)).collect();
    if !fotos.is_empty() {
        h.push_str("<div class=\"fotos\">");
        for (i, src) in fotos.iter().enumerate() {
            h.push_str(&format!(
                "<img src=\"{}\" alt=\"{} {}\" loading=\"lazy\">",
                escapar(src),
                escapar(&r.foto),
                i + 1
            ));
        }
        h.push_str("</div>");
    }
    h.push_str("</div></li>");
    h
}

/// La barrita del avance.
fn barra_html(hechas: usize, total: usize, r: &Rotulos, clase: &str) -> String {
    let pc = por_ciento(hechas, total);
    format!(
        "<div class=\"{clase}\" role=\"progressbar\" aria-valuemin=\"0\" aria-valuemax=\"{total}\" aria-valuenow=\"{hechas}\" aria-label=\"{}\"><span style=\"width:{pc}%\"></span></div>",
        escapar(&r.de(hechas, total))
    )
}

/// **La pagina de las listas**, entera en un fichero. `titulo`: el de lo
/// compartido (con una lista sola, el suyo); `ahora`: la hora local de
/// quien exporta; `foto` da el `src` de cada foto de una tarea.
pub fn html(
    listas: &[ListaParaExportar],
    titulo: &str,
    r: &Rotulos,
    ahora: Momento,
    foto: &dyn Fn(&str) -> Option<String>,
) -> String {
    let titulo = if titulo.trim().is_empty() {
        r.tareas.clone()
    } else {
        titulo.trim().to_string()
    };
    let (hechas, total) = cuenta(listas);
    let pendientes = total - hechas;
    let varias = listas.len() > 1;
    let mut h = String::with_capacity(16 * 1024);
    h.push_str(&format!(
        "<!doctype html>\n<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta name=\"color-scheme\" content=\"light dark\">\n\
         <meta name=\"generator\" content=\"PinDesk\">\n<title>{}</title>\n<style>\n{CSS}</style>\n</head>\n<body>\n<main class=\"hoja\">\n",
        escapar(&r.idioma),
        escapar(&titulo)
    ));
    h.push_str(&format!(
        "<header class=\"cab\">\n<p class=\"sello\">{}</p>\n<h1>{}</h1>\n",
        escapar(&r.exportado(ahora)),
        escapar(&titulo)
    ));
    h.push_str(&format!(
        "<div class=\"resumen\">\
         <div class=\"cifra\"><b>{pendientes}</b><span>{}</span></div>\
         <div class=\"cifra\"><b>{hechas}</b><span>{}</span></div>\
         <div class=\"cifra avance-cifra\"><b>{}<small>%</small></b><span>{}</span></div>\
         {}</div>\n</header>\n",
        escapar(&r.pendientes),
        escapar(&r.hechas),
        por_ciento(hechas, total),
        escapar(&r.completado),
        barra_html(hechas, total, r, "avance")
    ));
    for l in listas {
        let (lh, lt) = l.cuenta();
        h.push_str("<section class=\"lista\">\n");
        if varias {
            h.push_str(&format!(
                "<header class=\"lista-cab\"><h2>{}</h2><span class=\"cuenta\">{}</span>{}</header>\n",
                escapar(if l.titulo.trim().is_empty() {
                    &r.tareas
                } else {
                    l.titulo.trim()
                }),
                escapar(&r.de(lh, lt)),
                barra_html(lh, lt, r, "avance fina")
            ));
        }
        let pend: Vec<&TareaParaExportar> = l.pendientes().collect();
        if pend.is_empty() {
            h.push_str(&format!(
                "<p class=\"vacio\">{}</p>\n",
                escapar(if lt == 0 { &r.vacia } else { &r.todo_hecho })
            ));
        } else {
            h.push_str("<ul class=\"tareas\">\n");
            for t in pend {
                h.push_str(&fila_html(t, r, ahora, foto));
                h.push('\n');
            }
            h.push_str("</ul>\n");
        }
        if lh > 0 {
            h.push_str(&format!(
                "<details class=\"hechas\"><summary><span>{}</span><span class=\"n\">{lh}</span></summary>\n<ul class=\"tareas\">\n",
                escapar(&r.hechas)
            ));
            for t in l.hechas() {
                h.push_str(&fila_html(t, r, ahora, foto));
                h.push('\n');
            }
            h.push_str("</ul>\n</details>\n");
        }
        h.push_str("</section>\n");
    }
    // Al imprimir, lo plegado se abre: en papel no hay donde pulsar.
    h.push_str(
        "</main>\n<footer class=\"pie\">PinDesk</footer>\n<script>\
         addEventListener('beforeprint',function(){document.querySelectorAll('details.hechas').forEach(function(d){d.dataset.cerrada=d.open?'':'1';d.open=true;});});\
         addEventListener('afterprint',function(){document.querySelectorAll('details.hechas').forEach(function(d){if(d.dataset.cerrada)d.open=false;});});\
         </script>\n</body>\n</html>\n",
    );
    h
}

// --- Markdown y texto ---------------------------------------------------------

/// Lo que cambiaria la forma de un renglon de Markdown, escapado.
fn md(s: &str) -> String {
    let mut v = String::with_capacity(s.len());
    for c in s.trim().chars() {
        if "\\`*_[]<>~|".contains(c) {
            v.push('\\');
        }
        v.push(if c == '\n' || c == '\r' { ' ' } else { c });
    }
    v
}

/// Lo de detras del titulo de una tarea, en el mismo orden en todos los
/// formatos: urgencia, recordatorio y etiquetas.
fn sufijo(t: &TareaParaExportar, r: &Rotulos, ahora: Momento, negrita: bool) -> Vec<String> {
    let mut v = Vec::new();
    if !t.hecha
        && let Some(u) = r.urgencia(t.urgencia)
    {
        v.push(if negrita {
            format!("**{}**", md(u))
        } else {
            u.to_string()
        });
    }
    if let Some(h) = t.hora {
        let (cerca, lejos) = recordatorio(h, ahora, r);
        let mut s = format!("⏰ {cerca}");
        if let Some(l) = lejos {
            s.push_str(&format!(" ({l})"));
        }
        if pasada(t, ahora) {
            s.push_str(&format!(" · {}", r.pasada));
        }
        v.push(if negrita { md(&s) } else { s });
    }
    for e in &t.etiquetas {
        v.push(format!("#{e}"));
    }
    v
}

/// **Las listas en Markdown**: un titulo por lista, las casillas de
/// siempre (`- [ ]`) con la urgencia, el recordatorio y las etiquetas
/// detras, la nota en cursiva y lo hecho al final. De aqui sale el Word.
pub fn markdown(listas: &[ListaParaExportar], titulo: &str, r: &Rotulos, ahora: Momento) -> String {
    let titulo = if titulo.trim().is_empty() {
        &r.tareas
    } else {
        titulo.trim()
    };
    let (hechas, total) = cuenta(listas);
    let mut v = format!("# {}\n\n", md(titulo));
    v.push_str(&format!(
        "*{} · {}: {} · {}: {hechas} · {} %*\n",
        md(&r.exportado(ahora)),
        md(&r.pendientes),
        total - hechas,
        md(&r.hechas),
        por_ciento(hechas, total)
    ));
    let varias = listas.len() > 1;
    for l in listas {
        let (lh, lt) = l.cuenta();
        if varias {
            v.push_str(&format!(
                "\n## {} — {}\n",
                md(if l.titulo.trim().is_empty() {
                    &r.tareas
                } else {
                    &l.titulo
                }),
                md(&r.de(lh, lt))
            ));
        }
        v.push('\n');
        let pend: Vec<&TareaParaExportar> = l.pendientes().collect();
        if pend.is_empty() {
            v.push_str(&format!(
                "*{}*\n",
                md(if lt == 0 { &r.vacia } else { &r.todo_hecho })
            ));
        }
        for t in pend {
            v.push_str(&linea_md(t, r, ahora));
        }
        if lh > 0 {
            v.push_str(&format!(
                "\n{} {} ({lh})\n\n",
                if varias { "###" } else { "##" },
                md(&r.hechas)
            ));
            for t in l.hechas() {
                v.push_str(&linea_md(t, r, ahora));
            }
        }
    }
    v
}

fn linea_md(t: &TareaParaExportar, r: &Rotulos, ahora: Momento) -> String {
    let mut s = format!("- [{}] {}", if t.hecha { "x" } else { " " }, md(&t.titulo));
    for p in sufijo(t, r, ahora, true) {
        s.push_str(" · ");
        s.push_str(&p);
    }
    if let Some(n) = t.nota.as_deref().filter(|n| !n.trim().is_empty()) {
        s.push_str(&format!(" — *{}*", md(n)));
    }
    s.push('\n');
    s
}

/// **Las listas en texto llano**, con casillas `☐`/`☑`.
pub fn texto(listas: &[ListaParaExportar], titulo: &str, r: &Rotulos, ahora: Momento) -> String {
    let titulo = if titulo.trim().is_empty() {
        &r.tareas
    } else {
        titulo.trim()
    };
    let (hechas, total) = cuenta(listas);
    let mut v = format!(
        "{titulo}\n{} · {}: {} · {}: {hechas}\n",
        r.exportado(ahora),
        r.pendientes,
        total - hechas,
        r.hechas
    );
    let varias = listas.len() > 1;
    let linea = |t: &TareaParaExportar| {
        let mut s = format!("{} {}", if t.hecha { '☑' } else { '☐' }, t.titulo.trim());
        for p in sufijo(t, r, ahora, false) {
            s.push_str(" · ");
            s.push_str(&p);
        }
        if let Some(n) = t.nota.as_deref().filter(|n| !n.trim().is_empty()) {
            s.push_str(&format!("\n   💬 {}", n.trim()));
        }
        s.push('\n');
        s
    };
    for l in listas {
        let (lh, lt) = l.cuenta();
        if varias {
            v.push_str(&format!("\n{} ({})\n", l.titulo.trim(), r.de(lh, lt)));
        }
        v.push('\n');
        for t in l.pendientes() {
            v.push_str(&linea(t));
        }
        if lh > 0 {
            v.push_str(&format!("\n{} ({lh})\n", r.hechas));
            for t in l.hechas() {
                v.push_str(&linea(t));
            }
        }
    }
    v
}

// --- Hojas A4 (PDF, PNG, JPG, SVG) -------------------------------------------

pub const ANCHO_A4: f32 = 794.0;
pub const ALTO_A4: f32 = 1123.0;
const MARGEN: f32 = 60.0;
const FAMILIA: &str = "Segoe UI";

const fn rgb(r: u8, g: u8, b: u8) -> ColorRgba {
    ColorRgba::opaco(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

// Los de `pagina.css` en claro, ya mezclados con el papel: el PDF y el
// SVG no tienen por que saber de transparencias.
const TINTA: ColorRgba = rgb(0x1b, 0x1b, 0x1f);
const SUAVE: ColorRgba = rgb(0x63, 0x66, 0x6e);
const APAGADA: ColorRgba = rgb(0x9a, 0x9c, 0xa3);
const RAYA: ColorRgba = rgb(0xe3, 0xe4, 0xe8);
const RIEL: ColorRgba = rgb(0xec, 0xed, 0xf0);
const ACENTO: ColorRgba = rgb(0x2f, 0x6f, 0xde);
const ACENTO_SUAVE: ColorRgba = rgb(0xe6, 0xee, 0xfb);
const ROJO: ColorRgba = rgb(0xb4, 0x23, 0x18);
const ROJO_SUAVE: ColorRgba = rgb(0xfb, 0xe7, 0xe5);
const AMBAR: ColorRgba = rgb(0x9a, 0x5b, 0x00);
const AMBAR_SUAVE: ColorRgba = rgb(0xfc, 0xf0, 0xd9);
const GRIS_SUAVE: ColorRgba = rgb(0xf1, 0xf2, 0xf4);
const BLANCO: ColorRgba = ColorRgba::opaco(1.0, 1.0, 1.0);

const LETRA_TITULO: f32 = 26.0;
const LETRA_LISTA: f32 = 16.0;
const LETRA: f32 = 13.0;
const LETRA_CHIP: f32 = 10.0;
const LADO_CASILLA: f32 = 15.0;
const SANGRIA: f32 = 28.0;
const ALTO_FOTO: f32 = 64.0;

/// Una caja con las esquinas redondeadas, como poligono.
fn redondeada(x0: f32, y0: f32, x1: f32, y1: f32, radio: f32) -> Vec<Punto2> {
    let r = radio.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0).max(0.0);
    let mut v = Vec::with_capacity(28);
    let esquinas = [
        (x1 - r, y0 + r, -90.0f32),
        (x1 - r, y1 - r, 0.0),
        (x0 + r, y1 - r, 90.0),
        (x0 + r, y0 + r, 180.0),
    ];
    for (cx, cy, desde) in esquinas {
        for i in 0..=6 {
            let a = (desde + 15.0 * i as f32).to_radians();
            v.push(Punto2::nuevo(cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    v
}

fn relleno(puntos: Vec<Punto2>, color: ColorRgba) -> Orden {
    Orden::Relleno { puntos, color }
}

fn raya(puntos: Vec<Punto2>, color: ColorRgba, grosor: f32) -> Orden {
    Orden::Polilinea {
        puntos,
        color,
        grosor,
        estilo: EstiloTrazo::Solido,
    }
}

fn letras(texto: String, x: f32, y: f32, tam: f32, color: ColorRgba, negrita: bool) -> Orden {
    Orden::Texto {
        texto,
        x,
        y,
        tam,
        familia: FAMILIA.into(),
        color,
        // Holgado: la linea ya viene partida con la tabla de anchos (ver
        // `hojas_de_texto` de la hoja de compartir).
        ancho_max: (ANCHO_A4 - x).max(40.0) * 1.5,
        negrita,
        cursiva: false,
    }
}

/// Las hojas que se van llenando de arriba abajo.
struct Paginas {
    hojas: Vec<Hoja>,
    ordenes: Vec<Orden>,
    y: f32,
}

impl Paginas {
    /// Si `alto` no cabe en lo que queda, hoja nueva.
    fn cabe(&mut self, alto: f32) {
        if self.y + alto > ALTO_A4 - MARGEN && !self.ordenes.is_empty() {
            self.cerrar();
        }
    }

    fn cerrar(&mut self) {
        self.hojas.push(Hoja {
            nombre: String::new(),
            caja: (0.0, 0.0, ANCHO_A4, ALTO_A4),
            ordenes: std::mem::take(&mut self.ordenes),
            marcos: Vec::new(),
            granos: Vec::new(),
            grafitos: Vec::new(),
        });
        self.y = MARGEN;
    }

    /// Una barrita de avance de `ancho` en `(x, y)`.
    fn barra(&mut self, x: f32, y: f32, ancho: f32, alto: f32, hechas: usize, total: usize) {
        self.ordenes.push(relleno(
            redondeada(x, y, x + ancho, y + alto, alto / 2.0),
            RIEL,
        ));
        let lleno = if total == 0 {
            0.0
        } else {
            ancho * hechas as f32 / total as f32
        };
        if lleno > 0.5 {
            self.ordenes.push(relleno(
                redondeada(x, y, x + lleno.max(alto), y + alto, alto / 2.0),
                ACENTO,
            ));
        }
    }

    /// Una chapita en `(x, y)`; devuelve su ancho.
    fn chip(&mut self, x: f32, y: f32, texto: &str, tinta: ColorRgba, fondo: ColorRgba) -> f32 {
        let w = ancho_chip(texto);
        let alto = alto_chip();
        self.ordenes.push(relleno(
            redondeada(x, y, x + w, y + alto, alto / 2.0),
            fondo,
        ));
        self.ordenes.push(letras(
            texto.to_string(),
            x + 8.0,
            y + (alto - LETRA_CHIP * ex::INTERLINEA) / 2.0,
            LETRA_CHIP,
            tinta,
            true,
        ));
        w
    }
}

/// Lo que mide una chapita: su texto en negrita (la tabla es de la letra
/// normal; la negrita es algo mas ancha) y su relleno.
fn ancho_chip(texto: &str) -> f32 {
    ex::ancho_de_texto(texto, LETRA_CHIP) * NEGRITA + 16.0
}

/// Cuanto mas ancha es la negrita que la letra de la tabla de anchos.
const NEGRITA: f32 = 1.08;

fn alto_chip() -> f32 {
    LETRA_CHIP * ex::INTERLINEA + 5.0
}

/// La casilla en `(x, y)`.
fn casilla(o: &mut Vec<Orden>, x: f32, y: f32, hecha: bool, tinta: ColorRgba) {
    let l = LADO_CASILLA;
    if hecha {
        o.push(relleno(redondeada(x, y, x + l, y + l, 4.0), ACENTO));
        o.push(raya(
            vec![
                Punto2::nuevo(x + l * 0.24, y + l * 0.52),
                Punto2::nuevo(x + l * 0.43, y + l * 0.70),
                Punto2::nuevo(x + l * 0.77, y + l * 0.31),
            ],
            BLANCO,
            1.8,
        ));
    } else {
        o.push(relleno(redondeada(x, y, x + l, y + l, 4.0), tinta));
        o.push(relleno(
            redondeada(x + 1.5, y + 1.5, x + l - 1.5, y + l - 1.5, 2.8),
            BLANCO,
        ));
    }
}

/// **Las listas en hojas A4**: el titulo con la fecha, el resumen con su
/// barrita y cada lista con sus tareas en filas (casilla, titulo, chapitas,
/// nota y fotos), lo hecho al final y tachado. `foto` da, de cada enlace,
/// el id de su imagen y sus medidas; `None`, no va.
pub fn hojas(
    listas: &[ListaParaExportar],
    titulo: &str,
    r: &Rotulos,
    ahora: Momento,
    foto: &mut dyn FnMut(&str) -> Option<(u64, f32, f32)>,
) -> Vec<Hoja> {
    let ancho = ANCHO_A4 - 2.0 * MARGEN;
    let titulo = if titulo.trim().is_empty() {
        r.tareas.clone()
    } else {
        titulo.trim().to_string()
    };
    let mut p = Paginas {
        hojas: Vec::new(),
        ordenes: Vec::new(),
        y: MARGEN,
    };
    p.ordenes.push(letras(
        r.exportado(ahora).to_uppercase(),
        MARGEN,
        p.y,
        9.5,
        SUAVE,
        true,
    ));
    p.y += 9.5 * ex::INTERLINEA + 6.0;
    for l in ex::partir_texto(&titulo, LETRA_TITULO, ancho) {
        p.ordenes
            .push(letras(l, MARGEN, p.y, LETRA_TITULO, TINTA, true));
        p.y += LETRA_TITULO * ex::INTERLINEA;
    }
    // El resumen: las tres cifras y la barrita.
    let (hechas, total) = cuenta(listas);
    p.y += 10.0;
    let mut x = MARGEN;
    for (n, rotulo) in [
        ((total - hechas).to_string(), &r.pendientes),
        (hechas.to_string(), &r.hechas),
        (format!("{} %", por_ciento(hechas, total)), &r.completado),
    ] {
        p.ordenes.push(letras(n.clone(), x, p.y, 20.0, TINTA, true));
        let wn = ex::ancho_de_texto(&n, 20.0) * NEGRITA;
        p.ordenes.push(letras(
            rotulo.clone(),
            x + wn + 7.0,
            p.y + 20.0 * ex::INTERLINEA - 11.0 * ex::INTERLINEA - 3.0,
            11.0,
            SUAVE,
            false,
        ));
        x += wn + 7.0 + ex::ancho_de_texto(rotulo, 11.0) + 28.0;
    }
    p.y += 20.0 * ex::INTERLINEA + 8.0;
    p.barra(MARGEN, p.y, ancho, 6.0, hechas, total);
    p.y += 6.0 + 22.0;
    let varias = listas.len() > 1;
    for (i, l) in listas.iter().enumerate() {
        let (lh, lt) = l.cuenta();
        if varias {
            if i > 0 {
                p.y += 18.0;
            }
            p.cabe(LETRA_LISTA * ex::INTERLINEA + 60.0);
            let nombre = if l.titulo.trim().is_empty() {
                &r.tareas
            } else {
                l.titulo.trim()
            };
            let cuenta = r.de(lh, lt);
            let wc = ex::ancho_de_texto(&cuenta, 11.0);
            for (k, linea) in ex::partir_texto(nombre, LETRA_LISTA, ancho - wc - 140.0)
                .into_iter()
                .enumerate()
            {
                if k > 0 {
                    p.y += LETRA_LISTA * ex::INTERLINEA;
                }
                p.ordenes
                    .push(letras(linea, MARGEN, p.y, LETRA_LISTA, TINTA, true));
            }
            let yb = p.y + (LETRA_LISTA * ex::INTERLINEA - 4.0) / 2.0;
            p.barra(MARGEN + ancho - 90.0, yb, 90.0, 4.0, lh, lt);
            p.ordenes.push(letras(
                cuenta,
                MARGEN + ancho - 100.0 - wc,
                p.y + (LETRA_LISTA - 11.0) * ex::INTERLINEA / 2.0,
                11.0,
                SUAVE,
                false,
            ));
            p.y += LETRA_LISTA * ex::INTERLINEA + 6.0;
            p.ordenes.push(raya(
                vec![
                    Punto2::nuevo(MARGEN, p.y),
                    Punto2::nuevo(MARGEN + ancho, p.y),
                ],
                RAYA,
                1.0,
            ));
            p.y += 4.0;
        }
        let pend: Vec<&TareaParaExportar> = l.pendientes().collect();
        if pend.is_empty() {
            p.y += 8.0;
            p.ordenes.push(letras(
                if lt == 0 { &r.vacia } else { &r.todo_hecho }.clone(),
                MARGEN,
                p.y,
                LETRA,
                SUAVE,
                false,
            ));
            p.y += LETRA * ex::INTERLINEA + 4.0;
        }
        for (k, t) in pend.iter().enumerate() {
            fila_hoja(&mut p, t, r, ahora, ancho, k > 0, foto);
        }
        if lh > 0 {
            p.y += 14.0;
            p.cabe(40.0);
            let rotulo = format!("{} · {lh}", r.hechas.to_uppercase());
            p.ordenes
                .push(letras(rotulo, MARGEN, p.y, 9.5, SUAVE, true));
            p.y += 9.5 * ex::INTERLINEA + 2.0;
            for (k, t) in l.hechas().enumerate() {
                fila_hoja(&mut p, t, r, ahora, ancho, k > 0, foto);
            }
        }
    }
    if !p.ordenes.is_empty() || p.hojas.is_empty() {
        p.cerrar();
    }
    p.hojas
}

/// Una tarea como fila de la hoja.
fn fila_hoja(
    p: &mut Paginas,
    t: &TareaParaExportar,
    r: &Rotulos,
    ahora: Momento,
    ancho: f32,
    raya_encima: bool,
    foto: &mut dyn FnMut(&str) -> Option<(u64, f32, f32)>,
) {
    let x = MARGEN + SANGRIA;
    let ancho_texto = ancho - SANGRIA;
    let titulo = if t.titulo.trim().is_empty() {
        "—"
    } else {
        t.titulo.trim()
    };
    // Lo pendiente va en negrita: se parte un poco antes.
    let lineas = ex::partir_texto(titulo, LETRA, ancho_texto / NEGRITA);
    // Las chapitas: urgencia, recordatorio, etiquetas y, suelta, la fecha
    // en que se creo.
    let mut chips: Vec<(String, ColorRgba, ColorRgba)> = Vec::new();
    if !t.hecha
        && let Some(u) = r.urgencia(t.urgencia)
    {
        chips.push(match t.urgencia {
            Urgencia::MuyUrgente => (u.to_string(), ROJO, ROJO_SUAVE),
            _ => (u.to_string(), AMBAR, AMBAR_SUAVE),
        });
    }
    if let Some(h) = t.hora {
        let (cerca, lejos) = recordatorio(h, ahora, r);
        let mut s = cerca;
        if let Some(l) = lejos {
            s.push_str(&format!(" · {l}"));
        }
        if pasada(t, ahora) {
            s.push_str(&format!(" · {}", r.pasada));
            chips.push((s, ROJO, ROJO_SUAVE));
        } else if t.hecha {
            chips.push((s, SUAVE, GRIS_SUAVE));
        } else {
            chips.push((s, ACENTO, ACENTO_SUAVE));
        }
    }
    for e in &t.etiquetas {
        chips.push((format!("#{e}"), SUAVE, GRIS_SUAVE));
    }
    let creada = t.creada.map(|f| r.creada(f, ahora));
    // Cuantas filas de chapitas hacen falta.
    let mut filas_chips = 0usize;
    {
        let mut cx = x;
        let mut empezada = false;
        for (s, _, _) in &chips {
            let w = ancho_chip(s);
            if empezada && cx + w > MARGEN + ancho {
                filas_chips += 1;
                cx = x;
            }
            empezada = true;
            cx += w + 5.0;
        }
        if let Some(c) = &creada {
            let w = ex::ancho_de_texto(c, LETRA_CHIP);
            if empezada && cx + w > MARGEN + ancho {
                filas_chips += 1;
            }
            empezada = true;
        }
        if empezada {
            filas_chips += 1;
        }
    }
    let nota: Vec<String> = t
        .nota
        .as_deref()
        .filter(|n| !n.trim().is_empty())
        .map(|n| ex::partir_texto(n.trim(), 11.5, ancho_texto - 12.0))
        .unwrap_or_default();
    let fotos: Vec<(u64, f32, f32)> = t.imagenes.iter().filter_map(|e| foto(e)).collect();
    let alto_titulo = lineas.len() as f32 * LETRA * ex::INTERLINEA;
    let alto_chips = filas_chips as f32 * (alto_chip() + 4.0);
    let alto_nota = if nota.is_empty() {
        0.0
    } else {
        nota.len() as f32 * 11.5 * ex::INTERLINEA + 6.0
    };
    let alto_fotos = if fotos.is_empty() {
        0.0
    } else {
        ALTO_FOTO + 6.0
    };
    let alto = 10.0 + alto_titulo + 4.0 + alto_chips + alto_nota + alto_fotos + 8.0;
    p.cabe(alto);
    if raya_encima {
        p.ordenes.push(raya(
            vec![Punto2::nuevo(x, p.y), Punto2::nuevo(MARGEN + ancho, p.y)],
            RAYA,
            0.8,
        ));
    }
    let y0 = p.y + 10.0;
    // Una urgente lleva su raya de color a la izquierda.
    let franja = match (t.hecha, t.urgencia) {
        (false, Urgencia::MuyUrgente) => Some(ROJO),
        (false, Urgencia::Urgente) => Some(AMBAR),
        _ => None,
    };
    if let Some(c) = franja {
        p.ordenes.push(relleno(
            redondeada(MARGEN - 10.0, y0 - 2.0, MARGEN - 7.0, y0 + alto - 16.0, 1.5),
            c,
        ));
    }
    let tinta_casilla = match franja {
        Some(c) => c,
        None => rgb(0xb8, 0xba, 0xc1),
    };
    casilla(
        &mut p.ordenes,
        MARGEN,
        y0 + (LETRA * ex::INTERLINEA - LADO_CASILLA) / 2.0,
        t.hecha,
        tinta_casilla,
    );
    let color = if t.hecha { APAGADA } else { TINTA };
    let mut y = y0;
    for l in lineas {
        if t.hecha {
            // Tachada, como en la ventana.
            let w = ex::ancho_de_texto(&l, LETRA);
            let ym = y + LETRA * 0.72;
            p.ordenes.push(raya(
                vec![Punto2::nuevo(x, ym), Punto2::nuevo(x + w, ym)],
                APAGADA,
                1.0,
            ));
        }
        p.ordenes.push(letras(l, x, y, LETRA, color, !t.hecha));
        y += LETRA * ex::INTERLINEA;
    }
    y += 4.0;
    if filas_chips > 0 {
        let mut cx = x;
        let mut empezada = false;
        for (s, tinta, fondo) in &chips {
            let w = ancho_chip(s);
            if empezada && cx + w > MARGEN + ancho {
                y += alto_chip() + 4.0;
                cx = x;
            }
            empezada = true;
            cx += p.chip(cx, y, s, *tinta, *fondo) + 5.0;
        }
        if let Some(c) = creada {
            let w = ex::ancho_de_texto(&c, LETRA_CHIP);
            if empezada && cx + w > MARGEN + ancho {
                y += alto_chip() + 4.0;
                cx = x;
            }
            p.ordenes.push(letras(
                c,
                cx + if empezada { 3.0 } else { 0.0 },
                y + (alto_chip() - LETRA_CHIP * ex::INTERLINEA) / 2.0,
                LETRA_CHIP,
                APAGADA,
                false,
            ));
        }
        y += alto_chip() + 4.0;
    }
    if !nota.is_empty() {
        y += 2.0;
        let alto_n = nota.len() as f32 * 11.5 * ex::INTERLINEA;
        p.ordenes
            .push(relleno(redondeada(x, y, x + 2.5, y + alto_n, 1.2), RAYA));
        for l in nota {
            p.ordenes.push(letras(l, x + 12.0, y, 11.5, SUAVE, false));
            y += 11.5 * ex::INTERLINEA;
        }
        y += 4.0;
    }
    if !fotos.is_empty() {
        y += 2.0;
        let mut fx = x;
        for (id, w, h) in fotos {
            let (w, h) = (w.max(1.0), h.max(1.0));
            let k = ALTO_FOTO / h;
            let (iw, ih) = ((w * k).min(ALTO_FOTO * 2.0), ALTO_FOTO);
            if fx + iw > MARGEN + ancho {
                break;
            }
            p.ordenes.push(Orden::Imagen {
                id_objeto: id,
                x: fx,
                y,
                ancho: iw,
                alto: ih,
                opacidad: if t.hecha { 0.6 } else { 1.0 },
                recorte: None,
                angulo: 0.0,
            });
            fx += iw + 6.0;
        }
        y += ALTO_FOTO + 4.0;
    }
    p.y = y.max(p.y + alto - 8.0) + 4.0;
}

#[cfg(test)]
mod pruebas;
