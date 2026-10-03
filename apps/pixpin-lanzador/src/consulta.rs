//! Lo que se teclea detras de `pp`, entendido.
//!
//! - nada: las funciones y los ultimos proyectos;
//! - `<verbo> [texto] [@proyecto]`: una funcion con lo que haga falta
//!   (`chat hola`, `nota idea`, `lienzo plano`, `grabar clase 3`, `tareas`);
//! - `tareas <lista> > [texto]`: las tareas de esa lista;
//! - `<proyecto> > [texto]`: el chat de ese proyecto, de lo ultimo a lo
//!   primero (con un verbo delante tambien, si lo de antes de `>` es el
//!   nombre entero de un proyecto: eso lo mira `resultados`, que conoce los
//!   nombres);
//! - cualquier otra cosa: buscar en todo (con `@proyecto` para quedarse en uno).

use crate::normalizar::normalizar;

/// Las funciones de PixPin que se llaman por su nombre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Funcion {
    Chat,
    Tareas,
    Lienzo,
    Nota,
    Grabar,
    /// El recuadro flotante donde se sueltan archivos (pedido `soltar`).
    Soltar,
    Abrir,
}

impl Funcion {
    pub const TODAS: [Funcion; 7] = [
        Funcion::Chat,
        Funcion::Tareas,
        Funcion::Lienzo,
        Funcion::Nota,
        Funcion::Grabar,
        Funcion::Soltar,
        Funcion::Abrir,
    ];

    /// Las palabras que la llaman, ya normalizadas; la primera es la que se
    /// escribe al completar.
    pub fn alias(self) -> &'static [&'static str] {
        match self {
            Funcion::Chat => &["chat", "mensaje", "escribir", "message"],
            Funcion::Tareas => &["tareas", "tarea", "todo", "todos", "tasks", "task", "pendientes"],
            Funcion::Lienzo => &["lienzo", "canvas", "dibujo", "dibujar", "pizarra"],
            Funcion::Nota => &["nota", "note", "notas"],
            Funcion::Grabar => &["grabar", "audio", "voz", "record", "grabacion"],
            // «añadir» normalizado es «anadir».
            Funcion::Soltar => &["anadir", "soltar", "agregar", "subir", "adjuntar", "drop"],
            Funcion::Abrir => &["pixpin", "abrir", "open"],
        }
    }

    /// La palabra que se escribe al completar (`pp tareas `).
    pub fn verbo(self) -> &'static str {
        match self {
            Funcion::Soltar => "añadir",
            _ => self.alias()[0],
        }
    }

    pub fn de_palabra(palabra: &str) -> Option<Funcion> {
        let p = normalizar(palabra);
        Funcion::TODAS.into_iter().find(|f| f.alias().contains(&p.as_str()))
    }
}

/// Lo tecleado, entendido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modo {
    Vacio,
    /// Buscar en todo. `proyecto` es lo escrito detras de `@`.
    Buscar { texto: String, proyecto: Option<String> },
    /// Una funcion y lo que va detras (sin el `@proyecto`).
    Verbo { funcion: Funcion, resto: String, proyecto: Option<String> },
    /// Dentro de una lista de tareas: `tareas <lista> > <filtro>`.
    Lista { lista: String, filtro: String },
    /// Dentro del chat de un proyecto: `<proyecto> > <filtro>`.
    Proyecto { proyecto: String, filtro: String },
}

/// El separador entre la lista y la tarea, y entre el proyecto y lo que se
/// busca en su chat.
pub const SEPARADOR: char = '>';

pub fn analizar(busqueda: &str) -> Modo {
    let t = busqueda.trim_start();
    if t.trim().is_empty() {
        return Modo::Vacio;
    }
    let (primera, resto) = match t.find(char::is_whitespace) {
        Some(i) => (&t[..i], &t[i..]),
        None => (t, ""),
    };
    if let Some(funcion) = Funcion::de_palabra(primera) {
        if funcion == Funcion::Tareas {
            if let Some((lista, filtro)) = resto.split_once(SEPARADOR) {
                return Modo::Lista { lista: lista.trim().to_string(), filtro: filtro.trim().to_string() };
            }
        }
        let (resto, proyecto) = separar_proyecto(resto);
        return Modo::Verbo { funcion, resto, proyecto };
    }
    if let Some((proyecto, filtro)) = t.split_once(SEPARADOR) {
        return Modo::Proyecto { proyecto: proyecto.trim().to_string(), filtro: filtro.trim().to_string() };
    }
    let (texto, proyecto) = separar_proyecto(t);
    Modo::Buscar { texto, proyecto }
}

/// `texto @proyecto` → (`texto`, `Some("proyecto")`). Vale la ultima `@` que
/// este al principio o detras de un blanco (un correo no cuenta).
pub fn separar_proyecto(texto: &str) -> (String, Option<String>) {
    let mut corte = None;
    let mut anterior: Option<char> = None;
    for (i, c) in texto.char_indices() {
        if c == '@' && anterior.is_none_or(char::is_whitespace) {
            corte = Some(i);
        }
        anterior = Some(c);
    }
    match corte {
        Some(i) => (texto[..i].trim().to_string(), Some(texto[i + 1..].trim().to_string())),
        None => (texto.trim().to_string(), None),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn buscar(t: &str, p: Option<&str>) -> Modo {
        Modo::Buscar { texto: t.into(), proyecto: p.map(Into::into) }
    }
    fn verbo(f: Funcion, r: &str, p: Option<&str>) -> Modo {
        Modo::Verbo { funcion: f, resto: r.into(), proyecto: p.map(Into::into) }
    }

    #[test]
    fn vacio_y_buscar() {
        assert_eq!(analizar(""), Modo::Vacio);
        assert_eq!(analizar("   "), Modo::Vacio);
        assert_eq!(analizar("objetivos  "), buscar("objetivos", None));
        assert_eq!(analizar("tare"), buscar("tare", None));
        assert_eq!(analizar("plano @Thesis"), buscar("plano", Some("Thesis")));
    }

    #[test]
    fn los_verbos_en_espanol_e_ingles_y_con_tildes() {
        assert_eq!(analizar("chat hola que tal"), verbo(Funcion::Chat, "hola que tal", None));
        assert_eq!(analizar("todo"), verbo(Funcion::Tareas, "", None));
        assert_eq!(analizar("Canvas plano"), verbo(Funcion::Lienzo, "plano", None));
        assert_eq!(analizar("grabación clase 3"), verbo(Funcion::Grabar, "clase 3", None));
        assert_eq!(analizar("note idea"), verbo(Funcion::Nota, "idea", None));
        assert_eq!(analizar("grabar "), verbo(Funcion::Grabar, "", None));
        assert_eq!(analizar("Añadir thesis"), verbo(Funcion::Soltar, "thesis", None));
        assert_eq!(analizar("drop"), verbo(Funcion::Soltar, "", None));
        assert_eq!(analizar("adjuntar ges"), verbo(Funcion::Soltar, "ges", None));
    }

    #[test]
    fn la_arroba_elige_proyecto_pero_no_en_un_correo() {
        assert_eq!(analizar("chat hola @Gestión de pro"), verbo(Funcion::Chat, "hola", Some("Gestión de pro")));
        assert_eq!(analizar("chat escribe a max@x.com"), verbo(Funcion::Chat, "escribe a max@x.com", None));
        assert_eq!(analizar("lienzo @"), verbo(Funcion::Lienzo, "", Some("")));
        assert_eq!(analizar("nota a @uno @dos"), verbo(Funcion::Nota, "a @uno", Some("dos")));
    }

    #[test]
    fn dentro_de_una_lista_con_el_separador() {
        assert_eq!(analizar("tareas Compra > "), Modo::Lista { lista: "Compra".into(), filtro: "".into() });
        assert_eq!(
            analizar("tasks Compra · Casa > leche @casa"),
            Modo::Lista { lista: "Compra · Casa".into(), filtro: "leche @casa".into() }
        );
        // El separador solo vale detras de «tareas».
        assert_eq!(analizar("chat a > b"), verbo(Funcion::Chat, "a > b", None));
        // Sin verbo delante es el chat de un proyecto.
        assert_eq!(
            analizar("Mensajes guardados > foto"),
            Modo::Proyecto { proyecto: "Mensajes guardados".into(), filtro: "foto".into() }
        );
        assert_eq!(analizar("Thesis >"), Modo::Proyecto { proyecto: "Thesis".into(), filtro: "".into() });
    }
}
