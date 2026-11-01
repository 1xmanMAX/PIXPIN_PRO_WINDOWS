//! **El camino rapido: ordenes simples sin modelo** (14-oct).
//!
//! Regla del usuario: «siempre reducir al maximo la latencia; que sea
//! fluido, rapido y comodo». Medido el 14-oct en el banco, una orden como
//! «abre la galería» tardaba 2-3 s con la nube y 7-20 s con el modelo local
//! (dos vueltas: la llamada y la frase de despues). Para las ordenes cortas
//! y frecuentes no hace falta pensar: este interprete, determinista y en
//! español tolerante (sin tildes, con «por favor», con o sin «la tarea»),
//! las convierte en la llamada a la herramienta de la tabla de
//! [`crate::acciones`] y el agente la hace al instante (<100 ms), con la
//! frase corta de [`crate::acciones::Accion::hecho`] como respuesta.
//!
//! **Si no esta seguro, no hace nada** y contesta el modelo: dos ordenes en
//! una frase («apunta X y dibuja Y»), una fecha que no entiende, algo que no
//! empieza como una orden conocida. **Nunca lo que borra** (quitar una tarea,
//! borrar una lista): eso siempre pasa por el modelo y su confirmacion.

use serde_json::{Map, Value, json};

use crate::acciones::normalizar;
use crate::fechas::{self, Momento};

/// Una orden entendida: que herramienta y con que argumentos.
#[derive(Debug, Clone, PartialEq)]
pub struct Orden {
    pub herramienta: &'static str,
    pub args: Map<String, Value>,
}

impl Orden {
    fn nueva(herramienta: &'static str, args: Value) -> Orden {
        Orden {
            herramienta,
            args: args.as_object().cloned().unwrap_or_default(),
        }
    }
}

/// Lo mas largo que se acepta: una orden rapida es corta.
const LARGO_MAXIMO: usize = 160;

/// Los verbos que, detras de « y », empiezan OTRA orden.
const VERBOS: &[&str] = &[
    "abre", "dibuja", "crea", "haz", "pon", "apunta", "anade", "agrega", "marca", "graba", "captura", "borra", "quita",
    "mueve", "manda", "envia", "busca", "pinea", "saca", "copia", "calca", "lee", "dime", "explica", "luego", "despues",
    "tambien", "ademas", "ensename", "muestra",
];

/// El texto con su version normalizada letra a letra (para cortar el
/// original donde casa lo normalizado).
struct Frase {
    original: Vec<char>,
    q: String,
}

impl Frase {
    fn nueva(texto: &str) -> Option<Frase> {
        let original: Vec<char> = texto.chars().collect();
        let q: String = original.iter().map(|c| normalizar(&c.to_string()).chars().next().unwrap_or(*c)).collect();
        (q.chars().count() == original.len()).then_some(Frase { original, q })
    }

    /// Lo que queda del original tras el prefijo normalizado mas largo de
    /// `prefijos` (que tiene que acabar en palabra).
    fn tras(&self, prefijos: &[&str]) -> Option<String> {
        let mut mejor: Option<usize> = None;
        for p in prefijos {
            if self.q.starts_with(p) {
                let n = p.chars().count();
                let corta_palabra = self.q.chars().nth(n).is_some_and(|c| c.is_alphanumeric());
                if !corta_palabra && mejor.is_none_or(|m| n > m) {
                    mejor = Some(n);
                }
            }
        }
        mejor.map(|n| self.original[n..].iter().collect::<String>().trim().to_string())
    }

    fn es(&self, opciones: &[&str]) -> bool {
        opciones.contains(&self.q.as_str())
    }
}

/// Quita la cortesia y la puntuacion de los bordes.
fn limpiar(texto: &str) -> String {
    let mut s = texto.trim().to_string();
    loop {
        let antes = s.clone();
        let q = normalizar(&s);
        for p in ["@asistente", "asistente,", "oye,", "oye", "hey", "por favor,", "por favor", "porfa", "porfavor"] {
            if q.starts_with(p) && q[p.len()..].starts_with(|c: char| !c.is_alphanumeric()) {
                s = s.chars().skip(p.chars().count()).collect::<String>().trim_start_matches([',', ' ', ':']).to_string();
                break;
            }
        }
        let q = normalizar(&s);
        for p in [", por favor", " por favor", " porfa", ", gracias", " gracias"] {
            if q.ends_with(p) {
                let n = s.chars().count() - p.chars().count();
                s = s.chars().take(n).collect();
                break;
            }
        }
        s = s.trim().trim_end_matches(['.', '!', '¡']).trim_start_matches(['¡', '¿']).trim().to_string();
        if s == antes {
            return s;
        }
    }
}

/// Si hay otra orden detras de un « y » o de un punto: entonces no es
/// simple.
fn hay_otra_orden(q: &str) -> bool {
    let partes: Vec<&str> = q.split(['.', ';', '\n']).filter(|p| !p.trim().is_empty()).collect();
    if partes.len() > 1 {
        return true;
    }
    for sep in [" y ", ", y ", ", luego ", ", despues ", " luego ", " despues "] {
        for (i, _) in q.match_indices(sep) {
            let resto = q[i + sep.len()..].trim_start();
            let palabra = resto.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("");
            if VERBOS.iter().any(|v| palabra.starts_with(v)) {
                return true;
            }
        }
    }
    false
}

/// Quita un prefijo de conexion del principio («: », «que diga», «llamada»).
fn sin_conector(s: &str, conectores: &[&str]) -> String {
    let mut s = s.trim().trim_start_matches([':', '-', ',']).trim().to_string();
    let q = normalizar(&s);
    for c in conectores {
        if q.starts_with(c) && q[c.len()..].chars().next().is_none_or(|x| !x.is_alphanumeric()) {
            s = s.chars().skip(c.chars().count()).collect::<String>();
            break;
        }
    }
    s.trim().trim_start_matches([':', '-', ',']).trim().trim_matches(['"', '«', '»', '“', '”']).trim().to_string()
}

/// « en la lista X» al final: (resto, lista).
fn lista_al_final(s: &str) -> (String, Option<String>) {
    let q = normalizar(s);
    for marca in [" en la lista ", " a la lista ", " en mi lista ", " de la lista ", " en la lista de ", " a la lista de "] {
        if let Some(i) = q.rfind(marca) {
            let antes: String = s.chars().take(q[..i].chars().count()).collect();
            let lista: String = s.chars().skip(q[..i + marca.len()].chars().count()).collect();
            let lista = lista.trim().trim_matches(['"', '«', '»']).to_string();
            if !lista.is_empty() && !antes.trim().is_empty() {
                return (antes.trim().to_string(), Some(lista));
            }
        }
    }
    for marca in [" en el inbox", " al inbox", " en la bandeja"] {
        if q.ends_with(marca) {
            let n = s.chars().count() - marca.chars().count();
            return (s.chars().take(n).collect::<String>().trim().to_string(), Some("Inbox".into()));
        }
    }
    (s.trim().to_string(), None)
}

/// «Compras del chat Obra» → («Compras», «Obra»): la lista y su chat.
fn partir_chat(lista: Option<String>) -> (Option<String>, Option<String>) {
    let Some(l) = lista else { return (None, None) };
    let q = normalizar(&l);
    for marca in [" del chat ", " en el chat ", " del proyecto ", " en el proyecto ", " de el chat "] {
        if let Some(i) = q.find(marca) {
            let lista: String = l.chars().take(q[..i].chars().count()).collect();
            let chat: String = l.chars().skip(q[..i + marca.len()].chars().count()).collect();
            if !lista.trim().is_empty() && !chat.trim().is_empty() {
                return (Some(lista.trim().to_string()), Some(chat.trim().to_string()));
            }
        }
    }
    (Some(l), None)
}

/// **Interpreta una orden simple.** `None`: no es seguro; que conteste el
/// modelo.
pub fn interpretar(texto: &str, ahora: Momento) -> Option<Orden> {
    let limpio = limpiar(texto);
    if limpio.is_empty() || limpio.chars().count() > LARGO_MAXIMO {
        return None;
    }
    let pregunta = limpio.ends_with('?') || texto.trim_start().starts_with('¿');
    let limpio = limpio.trim_end_matches('?').trim().to_string();
    let f = Frase::nueva(&limpio)?;
    let q = f.q.as_str();
    if hay_otra_orden(q) {
        return None;
    }

    // --- Consultar las tareas (lo unico que vale con «?»).
    if let Some(o) = consulta(q) {
        return Some(o);
    }
    if pregunta {
        return None;
    }

    // --- Ventanas.
    for (ventana, nombres) in [
        ("galeria", &["galeria", "galeria de capturas", "las capturas", "mis capturas"][..]),
        ("tareas", &["tareas", "ventana de tareas", "mis tareas", "lista de tareas"][..]),
        ("lecciones", &["lecciones", "mis lecciones", "lecciones aprendidas"][..]),
        ("sincronizar", &["sincronizar", "sincronizacion", "ventana de sincronizar"][..]),
    ] {
        for v in ["abre", "abrir", "muestra", "muestrame", "ensename", "ve a", "ir a", "abreme"] {
            for art in ["", "la ", "las ", "mis ", "la ventana de ", "la ventana de la ", "la ventana de las "] {
                for n in nombres {
                    let n = n.strip_prefix("mis ").filter(|_| art == "mis ").unwrap_or(n);
                    if q == format!("{v} {art}{n}") {
                        return Some(Orden::nueva("abrir_ventana", json!({"cual": ventana})));
                    }
                }
            }
        }
    }

    // --- Capturas.
    if f.es(&[
        "captura", "capturar", "haz una captura", "haz una captura de pantalla", "captura la pantalla", "captura de pantalla",
        "haz una captura de una zona", "haz una captura de una zona de la pantalla", "pantallazo", "haz un pantallazo",
        "toma una captura", "saca una captura", "captura una zona", "recorta la pantalla",
    ]) {
        return Some(Orden::nueva("capturar_pantalla", json!({})));
    }
    if f.es(&[
        "pinea la ultima", "pinea la ultima captura", "saca la ultima captura", "saca como pin la ultima captura",
        "saca la ultima captura como pin", "pon la ultima captura como pin", "fija la ultima captura", "pin de la ultima captura",
        "pinear la ultima captura", "muestra la ultima captura como pin",
    ]) {
        return Some(Orden::nueva("pinear_ultima_captura", json!({})));
    }

    // --- Lienzo y notas.
    if let Some(resto) = f.tras(&[
        "crea un lienzo nuevo", "crea un lienzo", "crear un lienzo", "nuevo lienzo", "lienzo nuevo", "abre un lienzo nuevo",
        "haz un lienzo nuevo", "un lienzo nuevo",
    ]) {
        let nombre = sin_conector(&resto, &["llamado", "que se llame", "con el nombre", "de nombre", "nuevo llamado"]);
        let mut args = json!({});
        if !nombre.is_empty() {
            args["nombre"] = json!(nombre);
        }
        return Some(Orden::nueva("lienzo_nuevo", args));
    }
    if let Some(resto) = f.tras(&[
        "crea una nota nueva", "crea una nota", "nueva nota", "nota nueva", "escribe una nota", "haz una nota", "anota en una nota",
    ]) {
        let texto = sin_conector(&resto, &["que diga", "que ponga", "con el texto", "con", "diciendo", "sobre"]);
        let mut args = json!({});
        if !texto.is_empty() {
            args["texto"] = json!(texto);
        }
        return Some(Orden::nueva("nota_nueva", args));
    }

    // --- Grabar una nota de voz.
    if let Some(resto) = f.tras(&[
        "graba una nota de voz", "graba nota de voz", "grabar una nota de voz", "graba un audio", "graba audio", "nota de voz",
        "empieza a grabar", "graba un mensaje de voz", "graba",
    ]) {
        let nombre = sin_conector(&resto, &["llamada", "llamado", "que se llame", "con el nombre", "de nombre", "sobre", "de"]);
        // «graba la pantalla» no es una nota de voz.
        if q.starts_with("graba ") && !q.contains("voz") && !q.contains("audio") && !resto.is_empty() && !normalizar(&resto).starts_with("llamad") && !normalizar(&resto).starts_with("que se llame") {
            return None;
        }
        let nombre = if nombre.is_empty() { "Nota de voz".to_string() } else { nombre };
        return Some(Orden::nueva("grabar_voz", json!({"nombre": nombre})));
    }

    // --- Listas de tareas.
    if let Some(resto) = f.tras(&[
        "crea una lista de tareas", "crea una lista", "crea la lista", "nueva lista de tareas", "nueva lista", "lista nueva",
        "haz una lista de tareas", "haz una lista", "crear una lista",
    ]) {
        let titulo = sin_conector(&resto, &["llamada", "que se llame", "con el nombre", "de nombre", "titulada", "para"]);
        if titulo.is_empty() || titulo.chars().count() > 60 {
            return None;
        }
        return Some(Orden::nueva("lista_nueva", json!({"titulo": titulo})));
    }

    // --- Marcar (y desmarcar) una tarea, o cambiarle la fecha.
    if let Some(o) = marcar(&f) {
        return Some(o);
    }
    if let Some(o) = aplazar(&f, ahora) {
        return Some(o);
    }

    // --- Apuntar una tarea (con su lista y su fecha, si las dice).
    if let Some(resto) = f.tras(&[
        "crea una tarea", "crea la tarea", "crea tarea", "anade una tarea", "anade la tarea", "anade tarea", "agrega una tarea",
        "agrega la tarea", "agrega tarea", "apunta la tarea", "apunta una tarea", "apunta tarea", "anota la tarea",
        "anota una tarea", "nueva tarea", "tarea nueva", "pon una tarea", "pon la tarea", "recuerdame", "recordarme",
        "apunta", "anota", "anade", "agrega",
    ]) {
        let r = normalizar(&resto);
        // «apunta en el chat…» es otra cosa.
        if r.starts_with("en el chat") || r.starts_with("en una nota") || r.is_empty() {
            return None;
        }
        let texto = sin_conector(&resto, &["que diga", "de", "para", "llamada", "que"]);
        let (texto, lista) = lista_al_final(&texto);
        let (lista, chat) = partir_chat(lista);
        let (texto, fecha) = con_fecha(&texto, ahora)?;
        let texto = texto.trim_end_matches([',', ':']).trim().to_string();
        if texto.is_empty() || texto.chars().count() > 120 {
            return None;
        }
        let mut args = json!({"texto": texto});
        if let Some(l) = lista {
            args["lista"] = json!(l);
        }
        if let Some(c) = chat {
            args["chat"] = json!(c);
        }
        if let Some(f) = fecha {
            args["fecha"] = json!(f);
        }
        return Some(Orden::nueva("anadir_tarea", args));
    }
    None
}

/// La fecha del final de una frase, como la lee `tareas::resolver`: la marca
/// `AAAA-MM-DD HH:MM` si se entiende, o **la cola tal cual** si parece una
/// fecha pero no existe o ya paso (el resolver dira por que, sin modelo).
/// `None`: algo raro; que conteste el modelo.
fn con_fecha(texto: &str, ahora: Momento) -> Option<(String, Option<String>)> {
    match fechas::separar(texto, ahora) {
        Ok((t, Some(m))) => {
            let (y, mo, d) = fechas::fecha_de(m.dia);
            Some((t, Some(format!("{y:04}-{mo:02}-{d:02} {:02}:{:02}", m.minuto / 60, m.minuto % 60))))
        }
        Ok((t, None)) => Some((t, None)),
        Err(fechas::ErrorFecha::NoExiste(cola) | fechas::ErrorFecha::Pasada(cola)) => {
            let t = texto.split_whitespace().collect::<Vec<_>>().join(" ");
            let t = t.trim_end_matches(['.', ',', ';']).trim_end();
            let antes = t.strip_suffix(cola.as_str())?.trim_end().to_string();
            Some((antes, Some(cola)))
        }
        Err(fechas::ErrorFecha::NoEntiendo(_)) => None,
    }
}

/// «aplaza la tarea X al viernes», «pasa la tarea X a mañana», «cambia la
/// fecha de X al 20/10»: otra fecha para una tarea. Sin fecha detras («pasa
/// la tarea X a la lista Obra») no es esto: al modelo.
fn aplazar(f: &Frase, ahora: Momento) -> Option<Orden> {
    let resto = f.tras(&[
        "aplaza la tarea", "aplaza", "posterga la tarea", "posterga", "pospon la tarea", "pospon", "cambia la fecha de la tarea",
        "cambia la fecha de", "pasa la tarea", "mueve la tarea", "retrasa la tarea", "retrasa", "adelanta la tarea", "adelanta",
    ])?;
    let resto = sin_conector(&resto, &["la tarea"]);
    let (tarea, fecha) = con_fecha(&resto, ahora)?;
    let fecha = fecha?;
    let tarea = tarea.trim_end_matches([',', ':']).trim().to_string();
    if tarea.is_empty() || tarea.chars().count() > 120 {
        return None;
    }
    Some(Orden::nueva("editar_tarea", json!({"tarea": tarea, "fecha": fecha})))
}

/// «marca como hecha la tarea X», «tacha X», «he terminado X», «desmarca X»,
/// «marca X como pendiente».
fn marcar(f: &Frase) -> Option<Orden> {
    let (resto, mut hecha) = if let Some(r) = f.tras(&[
        "marca como hecha la tarea", "marca como hecha", "marca como completada la tarea", "marca como completada",
        "marca como terminada", "marca la tarea", "marca", "tacha la tarea", "tacha", "he terminado la tarea", "he terminado",
        "ya termine", "termine la tarea", "termine", "ya hice", "he hecho", "completa la tarea", "completada la tarea",
    ]) {
        (r, true)
    } else {
        (f.tras(&["desmarca la tarea", "desmarca", "marca como pendiente la tarea", "marca como pendiente"])?, false)
    };
    let mut resto = sin_conector(&resto, &["la tarea", "que"]);
    let q = normalizar(&resto);
    let mut explicito = !f.q.starts_with("marca la tarea") && !(f.q.starts_with("marca ") && !f.q.starts_with("marca como"));
    for (fin, h) in [
        (" como hecha", true),
        (" como completada", true),
        (" como terminada", true),
        (" de hecha", true),
        (" hecha", true),
        (" como pendiente", false),
        (" como no hecha", false),
    ] {
        if q.ends_with(fin) {
            let n = resto.chars().count() - fin.chars().count();
            resto = resto.chars().take(n).collect::<String>().trim().to_string();
            hecha = h;
            explicito = true;
            break;
        }
    }
    // «marca la tarea X» a secas no dice si hecha o pendiente: al modelo.
    if !explicito {
        return None;
    }
    let (tarea, lista) = lista_al_final(&resto);
    let (lista, chat) = partir_chat(lista);
    if tarea.is_empty() || tarea.chars().count() > 120 {
        return None;
    }
    let mut args = json!({"tarea": tarea, "hecha": hecha});
    if let Some(l) = lista {
        args["lista"] = json!(l);
    }
    if let Some(c) = chat {
        args["chat"] = json!(c);
    }
    Some(Orden::nueva("marcar_tarea", args))
}

/// «¿que tengo pendiente?», «¿que vencio esta semana?», «mis tareas de hoy».
fn consulta(q: &str) -> Option<Orden> {
    const EMPIEZAN: &[&str] = &[
        "que tengo pendiente", "que tengo por hacer", "que tengo que hacer", "que tareas tengo", "que me falta", "que me queda",
        "mis tareas", "tareas pendientes", "mis pendientes", "lo pendiente", "que hay pendiente", "que tareas me quedan",
        "que tareas hay", "que vencio", "que vencieron", "que ha vencido", "que han vencido", "que tareas vencieron",
        "que tareas han vencido", "que tareas vencen", "que vence", "tareas vencidas", "que tengo vencido", "que tareas me vencieron", "que tareas se me vencieron", "que se me vencio", "que me vencio", "que se me paso", "que tengo atrasado",
        "tareas atrasadas", "que hice", "que he hecho", "tareas hechas", "que tareas hice", "cuantas tareas tengo",
        "dime mis tareas", "dime que tengo pendiente", "lista mis tareas",
    ];
    if !EMPIEZAN.iter().any(|e| q.starts_with(e)) {
        return None;
    }
    let estado = if q.contains("venc") || q.contains("atrasad") {
        "vencidas"
    } else if q.contains("hice") || q.contains("hecho") || q.contains("hechas") {
        "hechas"
    } else {
        "pendientes"
    };
    let periodo = if q.contains("hoy") {
        "hoy"
    } else if q.contains("semana") {
        "semana"
    } else if q.contains(" mes") {
        "mes"
    } else {
        "todo"
    };
    let largo = EMPIEZAN.iter().filter(|e| q.starts_with(*e)).map(|e| e.len()).max().unwrap_or(0);
    let mut resto = format!(" {}", &q[largo..]);
    // «en el chat Obra», «de la lista Compras»: el filtro (por su nombre; el
    // resolver lo busca sin tildes ni mayusculas). Del final hacia atras.
    let mut args = json!({"estado": estado, "periodo": periodo});
    loop {
        let marca = [
            (" en el chat ", "chat"),
            (" del chat ", "chat"),
            (" en el proyecto ", "chat"),
            (" del proyecto ", "chat"),
            (" en la lista ", "lista"),
            (" de la lista ", "lista"),
        ]
        .into_iter()
        .filter_map(|(m, campo)| resto.rfind(m).map(|i| (i, m, campo)))
        .max_by_key(|x| x.0);
        let Some((i, m, campo)) = marca else { break };
        let nombre = resto[i + m.len()..].trim().to_string();
        if nombre.is_empty() || args.get(campo).is_some() {
            return None;
        }
        args[campo] = json!(nombre);
        resto.truncate(i);
    }
    // Lo demas que diga de mas no lo entiende el camino rapido: si hay algo
    // mas que el periodo, al modelo.
    let sobra: Vec<&str> = resto
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| {
            !p.is_empty()
                && !matches!(
                    *p,
                    "hoy" | "para" | "esta" | "este" | "semana" | "mes" | "de" | "que" | "pendientes" | "pendiente" | "por" | "hacer"
                        | "tengo" | "aun" | "todavia" | "ya" | "me" | "las" | "tareas" | "en" | "la" | "el" | "vencidas" | "vencido"
                )
        })
        .collect();
    if !sobra.is_empty() {
        return None;
    }
    Some(Orden::nueva("consultar_tareas", args))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ahora() -> Momento {
        crate::tareas::pruebas::ahora()
    }

    fn orden(t: &str) -> Option<(String, Value)> {
        interpretar(t, ahora()).map(|o| (o.herramienta.to_string(), Value::Object(o.args)))
    }

    fn es(t: &str, herramienta: &str, args: Value) {
        assert_eq!(orden(t), Some((herramienta.to_string(), args)), "«{t}»");
    }

    #[test]
    fn las_ordenes_simples_salen_sin_modelo() {
        es("Apunta la tarea: llamar al ingeniero mañana.", "anadir_tarea", json!({"texto": "llamar al ingeniero", "fecha": "2026-10-15 09:00"}));
        es("crea una tarea comprar pan", "anadir_tarea", json!({"texto": "comprar pan"}));
        es("Añade la tarea revisar planos para el viernes a las 10", "anadir_tarea", json!({"texto": "revisar planos", "fecha": "2026-10-16 10:00"}));
        es("añade comprar clavos a la lista Compras", "anadir_tarea", json!({"texto": "comprar clavos", "lista": "Compras"}));
        es("Añade revisar grietas a la lista Compras del chat Casa Lima.", "anadir_tarea", json!({"texto": "revisar grietas", "lista": "Compras", "chat": "Casa Lima"}));
        es("recuérdame llamar a Juan mañana a las 5 de la tarde", "anadir_tarea", json!({"texto": "llamar a Juan", "fecha": "2026-10-15 17:00"}));
        es("por favor, apunta comprar pan y leche", "anadir_tarea", json!({"texto": "comprar pan y leche"}));
        es("Crea una lista de tareas que se llame Obra.", "lista_nueva", json!({"titulo": "Obra"}));
        es("Crea una nota nueva que diga: comprar cemento y arena.", "nota_nueva", json!({"texto": "comprar cemento y arena"}));
        es("nueva nota", "nota_nueva", json!({}));
        es("Crea un lienzo nuevo llamado Croquis.", "lienzo_nuevo", json!({"nombre": "Croquis"}));
        es("nuevo lienzo", "lienzo_nuevo", json!({}));
        es("Abre la ventana de tareas.", "abrir_ventana", json!({"cual": "tareas"}));
        es("abre la galería", "abrir_ventana", json!({"cual": "galeria"}));
        es("muéstrame mis lecciones", "abrir_ventana", json!({"cual": "lecciones"}));
        es("Haz una captura de una zona de la pantalla.", "capturar_pantalla", json!({}));
        es("captura", "capturar_pantalla", json!({}));
        es("Saca como pin la ultima captura.", "pinear_ultima_captura", json!({}));
        es("pinea la última", "pinear_ultima_captura", json!({}));
        es("Graba una nota de voz que se llame reunion.", "grabar_voz", json!({"nombre": "reunion"}));
        es("graba", "grabar_voz", json!({"nombre": "Nota de voz"}));
        es("marca como hecha la tarea comprar pan", "marcar_tarea", json!({"tarea": "comprar pan", "hecha": true}));
        es("tacha revisar encofrado", "marcar_tarea", json!({"tarea": "revisar encofrado", "hecha": true}));
        es("marca comprar pan como hecha", "marcar_tarea", json!({"tarea": "comprar pan", "hecha": true}));
        es("desmarca pedir cemento", "marcar_tarea", json!({"tarea": "pedir cemento", "hecha": false}));
        es("¿Qué tengo pendiente?", "consultar_tareas", json!({"estado": "pendientes", "periodo": "todo"}));
        es("¿qué tareas me vencieron esta semana?", "consultar_tareas", json!({"estado": "vencidas", "periodo": "semana"}));
        es("¿qué venció esta semana?", "consultar_tareas", json!({"estado": "vencidas", "periodo": "semana"}));
        es("mis tareas de hoy", "consultar_tareas", json!({"estado": "pendientes", "periodo": "hoy"}));
        es("¿Qué tareas tengo en el chat Casa Lima?", "consultar_tareas", json!({"estado": "pendientes", "periodo": "todo", "chat": "casa lima"}));
        es("¿qué tengo pendiente en la lista Compras del chat Obra?", "consultar_tareas", json!({"estado": "pendientes", "periodo": "todo", "lista": "compras", "chat": "obra"}));
        es("Aplaza la tarea llamar al banco al viernes.", "editar_tarea", json!({"tarea": "llamar al banco", "fecha": "2026-10-16 09:00"}));
        es("pasa la tarea comprar pan a mañana a las 12", "editar_tarea", json!({"tarea": "comprar pan", "fecha": "2026-10-15 12:00"}));
        // Una fecha que no existe va tal cual: el resolver dice por que no.
        es("Añade la tarea pagar alquiler para el 31 de febrero", "anadir_tarea", json!({"texto": "pagar alquiler", "fecha": "para el 31 de febrero"}));
    }

    #[test]
    fn si_no_esta_seguro_contesta_el_modelo() {
        for t in [
            // Dos ordenes en una.
            "Apunta la tarea comprar yeso y dibuja un círculo rojo",
            "crea un lienzo y luego dibuja una casa",
            "abre la galería. Después captura",
            // Lo que borra, nunca.
            "Borra la lista de tareas con codigo m-123.",
            "quita la tarea comprar pan",
            // Preguntas y cosas que no son ordenes simples.
            "¿Cómo hago una captura de pantalla?",
            "¿Dónde veo mis tareas pendientes?",
            "Hola, ¿qué tal?",
            "Dibuja un circulo rojo",
            "Busca en mis lecciones aprendidas las de encofrado.",
            r"Abre el archivo C:\Users\yo\Desktop\plano.pdf",
            // «marca la tarea X» no dice si hecha o pendiente.
            "marca la tarea comprar pan",
            // Un filtro que no entiende, o «pasa la tarea» sin fecha (es mover).
            "¿qué tengo pendiente con prisa?",
            "pasa la tarea comprar pan a la lista Compras",
            "graba la pantalla",
            "apunta en el chat que mañana no vengo",
            "crea una lista",
        ] {
            assert_eq!(orden(t), None, "«{t}» no deberia ir por el camino rapido");
        }
    }
}
