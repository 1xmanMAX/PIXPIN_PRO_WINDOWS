//! **El asistente dentro de una conversacion**: lo que no depende de la
//! ventana (que se le manda al modelo, que foto y que adjuntos ve, y como se
//! escribe la burbuja en vivo). La ventana lo engancha en
//! `ventana_chat/asistente.rs`.
//!
//! - En el chat fijo «Asistente» todo lo que se escribe es para el.
//! - En cualquier otro chat, `@asistente` al principio o en medio le
//!   pregunta sobre lo que hay ahi (ve los ultimos mensajes del chat).
//! - Su respuesta es una nota normal del cuaderno, con `aparato = "IA"`
//!   (la chapa dice `#48·IA`): se guarda, se busca y viaja como cualquier
//!   otra, sin un formato nuevo que el movil no sepa leer.

use std::path::PathBuf;

use pixpin_ia::agente::{Evento, Final};
use pixpin_ia::herramientas::{AYUDA, CALCAR, DIBUJAR, GRAFICA, OCR};
use pixpin_proyecto::cuaderno::{Clase, Mensaje as Nota};

use super::turno::Adjunto;

/// El «aparato» de las notas del asistente.
pub const APARATO: &str = "IA";

/// La mencion que lo llama desde otro chat.
pub const MENCION: &str = "@asistente";

pub fn es_del_asistente(m: &Nota) -> bool {
    m.aparato.as_deref() == Some(APARATO)
}

fn es_nota(m: &Nota) -> bool {
    matches!(m.clase, None | Some(Clase::Nota))
}

/// Si el texto lo menciona, la pregunta sin la mencion. Sin distinguir
/// mayusculas; la mencion tiene que ser palabra entera (`@asistentes` no).
pub fn mencion(texto: &str) -> Option<String> {
    let bajo = texto.to_lowercase();
    let mut desde = 0;
    while let Some(i) = bajo[desde..].find(MENCION) {
        let i = desde + i;
        let fin = i + MENCION.len();
        let antes_ok = i == 0 || !bajo[..i].chars().next_back().is_some_and(char::is_alphanumeric);
        let despues_ok = !bajo[fin..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_');
        if antes_ok && despues_ok {
            let resto = format!("{}{}", &texto[..i], &texto[fin..]);
            let limpio = resto
                .trim()
                .trim_start_matches([',', ':'])
                .trim()
                .to_string();
            return Some(limpio);
        }
        desde = fin;
    }
    None
}

/// La conversacion de antes, para el modelo: `(es_del_usuario, texto)` de
/// las notas (las fotos y ficheros van aparte).
pub fn conversacion(anteriores: &[Nota]) -> Vec<(bool, String)> {
    anteriores
        .iter()
        .filter(|m| es_nota(m) && !m.texto.trim().is_empty())
        .map(|m| (!es_del_asistente(m), m.texto.clone()))
        .collect()
}

/// Lo mismo con el id y el numero de cada nota (`(id, numero, del_usuario,
/// texto)`): la hoja de trabajo apunta desde que nota se reinicio, y
/// `buscar_historial` dice de que mensaje sale cada trozo.
pub fn conversacion_con_ids(anteriores: &[Nota]) -> Vec<(String, i64, bool, String)> {
    anteriores
        .iter()
        .filter(|m| es_nota(m) && !m.texto.trim().is_empty())
        .map(|m| (m.id.clone(), m.numero, !es_del_asistente(m), m.texto.clone()))
        .collect()
}

/// Cuantos mensajes hacia atras se busca la foto de la que se habla.
pub const FOTO_ATRAS: usize = 4;

/// La foto que se le enseña: la ultima imagen de los ultimos
/// [`FOTO_ATRAS`] mensajes (contando el nuevo), sea cual sea la respuesta
/// que hubo en medio.
///
/// Antes se cortaba en la ultima respuesta del asistente, y eso rompia lo
/// mas natural: mandar la foto (el asistente la describe) y luego pedir
/// «dibuja eso»; el modelo ya no la veia y dibujaba lineas a ciegas hasta
/// quedarse sin tokens (13-oct).
pub fn foto_reciente(mensajes: &[Nota]) -> Option<&Nota> {
    mensajes
        .iter()
        .rev()
        .take(FOTO_ATRAS)
        .find(|m| m.clase == Some(Clase::Imagen) && !es_del_asistente(m))
}

/// Lo que se le pregunta al modelo cuando llega una foto sin texto: que
/// conteste siempre, aunque el usuario no haya escrito nada.
pub const PREGUNTA_SOLO_FOTO: &str =
    "Te mando esta imagen. Descríbela en una o dos frases y pregúntame qué quiero hacer con ella.";

/// Si el mensaje nuevo es para el asistente, la pregunta (sin la mencion).
/// En el chat «Asistente» todo lo es: una foto sola tambien, con
/// [`PREGUNTA_SOLO_FOTO`]. En otro chat, solo lo que lo menciona (en una
/// foto, en su pie).
pub fn pregunta_de(m: &Nota, chat_del_asistente: bool) -> Option<String> {
    if es_del_asistente(m) {
        return None;
    }
    let foto = m.clase == Some(Clase::Imagen);
    if !(es_nota(m) || foto) {
        return None;
    }
    let pregunta = if chat_del_asistente {
        m.texto.trim().to_string()
    } else {
        mencion(&m.texto)?
    };
    if pregunta.is_empty() {
        return foto.then(|| PREGUNTA_SOLO_FOTO.to_string());
    }
    Some(pregunta)
}

// ---------------------------------------------------------------------------
// Lienzos: el propio, los mencionados y «Abrir lienzo»

/// En la nota de una respuesta, el lienzo donde dibujo (su referencia).
/// Va en `resto`, que el cuaderno guarda tal cual y el movil respeta.
pub const LIENZO_DE_LA_RESPUESTA: &str = "lienzoIa";

/// El prefijo de la mencion de un lienzo: `@lienzo:<nombre>`.
pub const MENCION_LIENZO: &str = "@lienzo:";

/// Si es un lienzo (o una hoja de PDF con dibujo) en el que se puede dibujar.
pub fn es_lienzo(m: &Nota) -> bool {
    matches!(m.clase, Some(Clase::Dibujo) | Some(Clase::Pagina)) && m.referencia.as_deref().is_some_and(|r| !r.is_empty())
}

/// Como se le llama a un lienzo en la lista y en la mencion.
pub fn nombre_de_lienzo(m: &Nota) -> String {
    // Un lienzo sin nombre lleva el de su fichero (`<id>.excalidraw`), que
    // no le dice nada a nadie.
    let n = m.nombre.trim();
    if !n.is_empty() && !n.ends_with(".excalidraw") {
        n.to_string()
    } else if m.clase == Some(Clase::Pagina) {
        format!("Hoja {}", m.pagina.map_or(m.numero, |p| p as i64 + 1))
    } else {
        format!("Lienzo {}", m.numero)
    }
}

/// La mencion que se escribe para un lienzo: sin espacios (el `_` los
/// hace), para que acabe donde acaba la palabra.
pub fn mencion_de_lienzo(m: &Nota) -> String {
    format!("{MENCION_LIENZO}{}", nombre_de_lienzo(m).replace(char::is_whitespace, "_"))
}

fn sin_tildes(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            '_' => ' ',
            c => c,
        })
        .collect()
}

/// **La pregunta con sus menciones de lienzo resueltas**: el texto para el
/// modelo (la mencion cambiada por «nombre») y la referencia del lienzo
/// nombrado, si existe en la conversacion. Una mencion que no casa con
/// ningun lienzo se deja tal cual y no cambia el destino.
pub fn resolver_lienzo(pregunta: &str, mensajes: &[Nota]) -> (String, Option<String>) {
    let mut texto = String::new();
    let mut destino = None;
    let mut resto = pregunta;
    while let Some(i) = resto.to_lowercase().find(MENCION_LIENZO) {
        texto.push_str(&resto[..i]);
        let tras = &resto[i + MENCION_LIENZO.len()..];
        let fin = tras.find(char::is_whitespace).unwrap_or(tras.len());
        let nombre = tras[..fin].trim_end_matches([',', '.', ';', ':', '?', '!']);
        let buscado = sin_tildes(nombre);
        let hallado = mensajes
            .iter()
            .rev()
            .filter(|m| es_lienzo(m))
            .find(|m| sin_tildes(&nombre_de_lienzo(m)) == buscado);
        match hallado {
            Some(m) => {
                texto.push_str(&format!("el lienzo «{}»", nombre_de_lienzo(m)));
                destino = m.referencia.clone();
            }
            None => texto.push_str(&resto[i..i + MENCION_LIENZO.len() + nombre.len()]),
        }
        resto = &tras[nombre.len()..];
    }
    texto.push_str(resto);
    (texto, destino)
}

/// El lienzo propio del asistente en esta conversacion: el ultimo que creo
/// el (su `aparato` es [`APARATO`]).
pub fn lienzo_propio(mensajes: &[Nota]) -> Option<usize> {
    mensajes
        .iter()
        .rposition(|m| m.clase == Some(Clase::Dibujo) && es_del_asistente(m) && es_lienzo(m))
}

/// Si en la pregunta se pide abrir el lienzo al terminar («dibuja… y
/// ábrelo»). Sin pedirlo, se dibuja en segundo plano y la respuesta lleva
/// «Abrir lienzo».
pub fn pide_abrir(pregunta: &str) -> bool {
    let t = sin_tildes(pregunta);
    [
        "abrelo",
        "abremelo",
        "abre el lienzo",
        "abrir el lienzo",
        "abre el dibujo",
        "abrelo al terminar",
        "muestramelo",
        "ensenamelo",
        "y abre",
        "y abrir",
    ]
    .iter()
    .any(|p| t.contains(p))
}

/// El lienzo al que apunta una respuesta, por su posicion en la
/// conversacion.
pub fn lienzo_de_la_respuesta(m: &Nota, mensajes: &[Nota]) -> Option<usize> {
    if !es_del_asistente(m) {
        return None;
    }
    let r = m.resto.get(LIENZO_DE_LA_RESPUESTA)?.as_str()?;
    mensajes.iter().rposition(|x| es_lienzo(x) && x.referencia.as_deref() == Some(r))
}

// ---------------------------------------------------------------------------
// Las fuentes citadas (`buscar_en_mis_cosas`)

/// En la nota de una respuesta, lo que cito de los documentos del usuario:
/// `[{n, titulo, proyecto, pedido}]` (`documentos::herramienta::cita_a_json`).
/// Va en `resto`, como `lienzoIa`.
pub const FUENTES_DE_LA_RESPUESTA: &str = "fuentesIa";

/// Lo que se apunta en la respuesta al acabar: las citas que el modelo
/// nombro (o la primera, si no nombro ninguna). `None` si no busco nada.
pub fn fuentes_para_la_nota(citas: &[pixpin_ia::documentos::Cita], respuesta: &str) -> Option<serde_json::Value> {
    if citas.is_empty() {
        return None;
    }
    let c = pixpin_ia::documentos::Citas { lista: citas.to_vec() };
    let citadas: Vec<serde_json::Value> = c
        .citadas(respuesta)
        .into_iter()
        .map(pixpin_ia::documentos::herramienta::cita_a_json)
        .collect();
    Some(serde_json::Value::Array(citadas))
}

/// Una fuente citada en una respuesta: como se ensena y el pedido que la
/// abre.
#[derive(Debug, Clone, PartialEq)]
pub struct FuenteCitada {
    pub titulo: String,
    pub proyecto: String,
    pub pedido: serde_json::Value,
    /// Cuantas mas cito ademas de esta.
    pub mas: usize,
}

/// La primera fuente citada de una respuesta del asistente, si la hay: de
/// ahi sale su tarjeta «Abrir fuente».
pub fn fuente_de_la_respuesta(m: &Nota) -> Option<FuenteCitada> {
    if !es_del_asistente(m) {
        return None;
    }
    let lista = m.resto.get(FUENTES_DE_LA_RESPUESTA)?.as_array()?;
    let f = lista.first()?;
    Some(FuenteCitada {
        titulo: f.get("titulo")?.as_str()?.to_string(),
        proyecto: f.get("proyecto").and_then(|p| p.as_str()).unwrap_or_default().to_string(),
        pedido: f.get("pedido")?.clone(),
        mas: lista.len() - 1,
    })
}

// ---------------------------------------------------------------------------
// La lista de `@` mientras se escribe

/// Lo que se puede elegir en la lista de `@`.
#[derive(Debug, Clone, PartialEq)]
pub struct Opcion {
    /// Lo que se ve en la lista.
    pub rotulo: String,
    /// Lo que se escribe en la caja al elegirla (con un espacio detras).
    pub mencion: String,
}

/// Cuantas opciones se ensenan como mucho.
pub const OPCIONES_MAX: usize = 6;

/// **Si se esta escribiendo una mencion**: donde empieza su `@` (en bytes)
/// y lo escrito detras, hasta la rayita. Solo si el `@` empieza palabra y
/// no hay un espacio entre el y la rayita.
pub fn consulta_de_mencion(texto: &str, cursor: usize) -> Option<(usize, String)> {
    let antes = texto.get(..cursor)?;
    let arroba = antes.rfind('@')?;
    let escrito = &antes[arroba + 1..];
    if escrito.chars().any(char::is_whitespace) {
        return None;
    }
    let empieza = antes[..arroba].chars().next_back().is_none_or(char::is_whitespace);
    empieza.then(|| (arroba, escrito.to_string()))
}

/// Las opciones para lo escrito tras el `@`: el asistente (fuera de su
/// chat) y los lienzos y hojas de la conversacion, del mas nuevo al mas
/// viejo.
pub fn opciones_de_mencion(mensajes: &[Nota], escrito: &str, chat_del_asistente: bool) -> Vec<Opcion> {
    let buscado = sin_tildes(escrito);
    let casa = |o: &Opcion| {
        buscado.is_empty()
            || sin_tildes(&o.rotulo).contains(&buscado)
            || sin_tildes(o.mencion.trim_start_matches('@')).starts_with(&buscado)
    };
    let mut v = Vec::new();
    if !chat_del_asistente {
        v.push(Opcion {
            rotulo: "🤖 Asistente".into(),
            mencion: MENCION.into(),
        });
    }
    let mut vistas = std::collections::HashSet::new();
    for m in mensajes.iter().rev().filter(|m| es_lienzo(m)) {
        // Una hoja repetida (la misma referencia dos veces) sale una vez.
        if !vistas.insert(m.referencia.clone()) {
            continue;
        }
        let icono = if m.clase == Some(Clase::Pagina) { "📄" } else { "🎨" };
        v.push(Opcion {
            rotulo: format!("{icono} {}", nombre_de_lienzo(m)),
            mencion: mencion_de_lienzo(m),
        });
    }
    v.retain(casa);
    v.truncate(OPCIONES_MAX);
    v
}

/// Escribe la opcion elegida en lugar de lo que se llevaba escrito desde el
/// `@`. Devuelve donde queda la rayita (en bytes).
pub fn poner_mencion(texto: &mut String, arroba: usize, cursor: usize, o: &Opcion) -> usize {
    let cursor = cursor.min(texto.len()).max(arroba);
    let puesta = format!("{} ", o.mencion);
    texto.replace_range(arroba..cursor, &puesta);
    arroba + puesta.len()
}

/// Los adjuntos que el OCR puede leer (fotos y PDF), del mas viejo al mas
/// nuevo. `ruta` resuelve donde esta el fichero de cada mensaje.
pub fn adjuntos(mensajes: &[Nota], ruta: impl Fn(&Nota) -> Option<PathBuf>) -> Vec<Adjunto> {
    mensajes
        .iter()
        .filter(|m| matches!(m.clase, Some(Clase::Imagen) | Some(Clase::Archivo)))
        .filter_map(|m| {
            let r = ruta(m)?;
            (super::ocr::es_pdf(&r) || super::ocr::es_imagen(&r)).then(|| Adjunto {
                nombre: if m.nombre.is_empty() {
                    r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                } else {
                    m.nombre.clone()
                },
                ruta: r,
            })
        })
        .collect()
}

/// **La burbuja que se escribe en vivo.**
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnVivo {
    pub texto: String,
    /// «Dibujando…» mientras el modelo usa una herramienta.
    pub estado: Option<String>,
    /// Lo que fue mal por el camino (se dice al final, en una linea).
    pub fallos: Vec<String>,
    pub acabado: bool,
    /// La pregunta de una accion que borra o envia, mientras espera el «Sí»
    /// o el «No» de la tarjeta de la burbuja. Contestada, pasa al texto.
    pub pendiente: Option<String>,
    /// Lo que va pensando (la seccion plegable «Pensando…»).
    pub pensamiento: String,
    /// Pensando ahora mismo.
    pub pensando: bool,
    /// Cuanto penso, al acabar de pensar.
    pub segundos: Option<f32>,
    /// La peticion mejorada.
    pub plan: Option<pixpin_ia::pensar::Plan>,
    /// El turno empezo con el contexto renovado.
    pub reiniciado: bool,
}

/// Como acabo una pregunta de confirmar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contestacion {
    Si,
    No,
    /// Nadie contesto: se cerro el chat, se paro o paso el tiempo.
    Caducada,
}

/// Cuanto espera una pregunta antes de darse por no contestada.
pub const CADUCA_LA_PREGUNTA: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// Lo que se ensena mientras se usa cada herramienta.
pub fn rotulo_de(nombre: &str) -> String {
    match nombre {
        DIBUJAR => "✏️ dibujando en el lienzo…",
        GRAFICA => "📊 haciendo la gráfica…",
        OCR => "🔎 leyendo el documento…",
        pixpin_ia::contexto::BUSCAR_HISTORIAL => "🗂️ buscando en la conversación…",
        CALCAR => "🖼️ calcando la imagen…",
        AYUDA => "📖 mirando la ayuda…",
        _ => "⚙️ trabajando…",
    }
    .to_string()
}

impl EnVivo {
    pub fn aplicar(&mut self, e: &Evento) {
        match e {
            Evento::Texto(t) => {
                self.texto.push_str(t);
                self.estado = None;
                self.pensando = false;
            }
            Evento::Pensando(t) => {
                self.pensamiento.push_str(t);
                self.pensando = true;
            }
            Evento::PensoListo { segundos } => {
                self.pensando = false;
                // Con la nube piensa en cada vuelta: se suma.
                *self.segundos.get_or_insert(0.0) += *segundos;
            }
            Evento::Peticion(p) => self.plan = Some(p.clone()),
            Evento::Reinicio { .. } => self.reiniciado = true,
            Evento::Llamando(n) => self.estado = Some(rotulo_de(n)),
            Evento::Herramienta { llamada, .. } => {
                // Lo que diga despues va en su linea, no pegado a lo de antes.
                if !self.texto.is_empty() && !self.texto.ends_with('\n') {
                    self.texto.push('\n');
                }
                self.estado = Some(rotulo_de(llamada.nombre()));
            }
            Evento::Resultado { ok, texto, .. } => {
                // Un «No» en la tarjeta no es un fallo: ya lo dice su linea.
                if !ok && !texto.contains("dijo que no") {
                    self.fallos.push(texto.trim_start_matches("ERROR: ").to_string());
                }
                self.estado = Some("💭 …".into());
            }
            Evento::Reintento { .. } => self.estado = Some("🔁 corrigiendo…".into()),
        }
    }

    /// Llega una pregunta: la burbuja ensena su tarjeta con «Sí» y «No».
    pub fn preguntar(&mut self, pregunta: &str) {
        // Una anterior sin contestar (no deberia: el turno espera) caduca.
        self.contestar(Contestacion::Caducada);
        self.pendiente = Some(pregunta.trim().to_string());
        self.estado = Some("✋ esperando tu respuesta…".into());
    }

    /// **La pregunta contestada queda en el historial**, en su linea:
    /// «❓ ¿Borrar la lista «Obra»? → ✅ Sí». `false` si no habia ninguna.
    pub fn contestar(&mut self, c: Contestacion) -> bool {
        let Some(p) = self.pendiente.take() else { return false };
        if !self.texto.is_empty() && !self.texto.ends_with('\n') {
            self.texto.push('\n');
        }
        let fin = match c {
            Contestacion::Si => "✅ Sí",
            Contestacion::No => "✖️ No",
            Contestacion::Caducada => "⌛ sin respuesta: no se hizo nada",
        };
        self.texto.push_str(&format!("❓ {p} → {fin}\n"));
        self.estado = (!self.acabado).then(|| "💭 …".to_string());
        true
    }

    /// El final del turno.
    pub fn acabar(&mut self, r: &Result<Final, String>) {
        self.contestar(Contestacion::Caducada);
        self.acabado = true;
        self.estado = None;
        self.pensando = false;
        // Un `</think>` suelto que se colo en el texto (ver `pensar::Separador`).
        if self.texto.contains("</think>") {
            let (pensado, texto) = pixpin_ia::pensar::separar(&self.texto);
            if self.pensamiento.trim().is_empty() {
                self.pensamiento = pensado;
            }
            self.texto = texto;
        }
        match r {
            Ok(f) => {
                if self.texto.trim().is_empty() {
                    self.texto = f.texto.clone();
                }
                if f.cortado {
                    if !self.texto.is_empty() {
                        self.texto.push(' ');
                    }
                    self.texto.push_str("⏹ (parado)");
                }
            }
            Err(e) => {
                // `e` ya viene en palabras del usuario
                // (`agente::para_el_usuario`): lo tecnico esta en el registro.
                if !self.texto.is_empty() {
                    self.texto.push_str("\n\n");
                }
                self.texto.push_str(&format!("⚠️ {e}"));
            }
        }
    }

    /// El texto de la burbuja tal como se ve ahora.
    pub fn mostrar(&self) -> String {
        let mut s = self.texto.trim_end().to_string();
        if let Some(e) = &self.estado {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(e);
        } else if !self.acabado {
            s.push_str(" ▍");
        }
        if self.acabado && !self.fallos.is_empty() {
            s.push_str(&format!("\n⚠️ {}", self.fallos.join(" · ")));
        }
        s.trim_start().to_string()
    }
}
