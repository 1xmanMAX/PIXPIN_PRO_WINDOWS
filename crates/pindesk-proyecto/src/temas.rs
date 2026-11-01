//! **Los temas de una reunion** (28-oct-2026, el usuario: «una funcion de
//! TAREAS para la reunion: la persona crea una lista de TEMAS a tratar en la
//! reunion; durante la reunion el telefono (o el PC) se queda escuchando y
//! transcribiendo; eso se manda a JEV y Jev marca que temas ya se trataron»).
//!
//! Lo puro, sin ventana ni red, para probarlo en milisegundos y para que el
//! movil haga las mismas cuentas:
//!
//! - **Leer los temas** de lo escrito o pegado ([`de_texto`]: uno por linea,
//!   sin vinetas ni numeros) o de una lista de tareas del chat
//!   ([`de_documento`]).
//! - **El formato** en la nota de voz, `resto["temas"]`, una lista en el
//!   orden de la agenda:
//!
//!   ```json
//!   [{"id": "t1", "texto": "Presupuesto 2027", "hecho": true,
//!     "ms": 750000, "p": 0.86, "porJev": true, "aMano": false},
//!    {"id": "t2", "texto": "Viaje a Lima", "hecho": false,
//!     "porJev": false, "aMano": false}]
//!   ```
//!
//!   `id` es estable (`t1`, `t2`… en el orden en que se apuntaron; uno
//!   quitado no se reutiliza), `hecho` si ya se trato, `ms` el minuto de la
//!   grabacion en que se trato (Jev: el principio del tramo; a mano
//!   grabando: el minuto del clic; sin el, no se sabe), `p` la probabilidad
//!   que dio Jev (0-1, dos decimales, solo si lo marco el), `porJev` si lo
//!   marco Jev y `aMano` si **la persona lo toco** (marcado o desmarcado).
//!   Sin temas, la clave se quita: la nota sigue byte a byte como antes.
//!   `resto["temasLista"] = {"proyecto", "id"}` es la lista de tareas del
//!   chat que lleva los mismos temas (se ven en la ventana Tareas).
//! - **Lo hecho a mano manda** ([`a_mano`], [`por_jev`]): un tema tocado a
//!   mano no lo vuelve a tocar Jev, ni para marcarlo ni para desmarcarlo.
//!   Jev solo marca (nunca desmarca) y solo lo que sigue pendiente.
//! - **La lista de tareas** con los temas ([`a_documento`]) y las marcas
//!   que vuelven de ella ([`con_la_lista`], [`marcas_para_la_lista`]).
//!
//! Android lee y escribe lo mismo (ver `docs/jev.md`, «Temas de la
//! reunion»).

use serde_json::{Map, Value};

use crate::mini;

/// La clave de los temas en el `resto` de la nota de voz.
pub const TEMAS: &str = "temas";
/// Y la de la lista de tareas que los lleva.
pub const TEMAS_LISTA: &str = "temasLista";
/// Cuantos temas como mucho: una agenda de mas de cuarenta no es una agenda.
pub const MAXIMO: usize = 40;
/// Lo largo de un tema, en caracteres.
pub const LARGO_MAXIMO: usize = 200;

/// Un tema de la agenda.
#[derive(Debug, Clone, PartialEq)]
pub struct Tema {
    pub id: String,
    pub texto: String,
    pub hecho: bool,
    /// El milisegundo de la grabacion en que se trato.
    pub ms: Option<i64>,
    /// La probabilidad que dio Jev, si lo marco el.
    pub p: Option<f64>,
    /// Lo marco Jev.
    pub por_jev: bool,
    /// Lo toco la persona: Jev ya no lo cambia.
    pub a_mano: bool,
}

impl Tema {
    pub fn nuevo(id: &str, texto: &str) -> Tema {
        Tema {
            id: id.to_string(),
            texto: texto.to_string(),
            hecho: false,
            ms: None,
            p: None,
            por_jev: false,
            a_mano: false,
        }
    }

    /// Si Jev aun tiene que mirar este tema: pendiente y sin tocar a mano.
    pub fn para_jev(&self) -> bool {
        !self.hecho && !self.a_mano
    }
}

/// Un tema limpio: en una linea, sin vineta, casilla ni numero delante, y
/// recortado. Vacio si no queda nada.
pub fn limpio(linea: &str) -> String {
    let mut t = linea.trim();
    // Las vinetas que no son de Markdown pero llegan al pegar de Word o de
    // un correo.
    while let Some(r) = t.strip_prefix(['•', '·', '–', '—', '▪', '◦', '○', '●']) {
        t = r.trim_start();
    }
    let t = mini::saneado(t);
    let (t, _) = mini::partir(&t);
    let t = t.trim().trim_end_matches([':', ';', ',']).trim();
    if t.chars().count() > LARGO_MAXIMO {
        let mut r: String = t.chars().take(LARGO_MAXIMO).collect();
        r.push('…');
        r
    } else {
        t.to_string()
    }
}

/// La forma de comparar dos temas: sin mayusculas ni blancos de mas.
fn clave(texto: &str) -> String {
    texto
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// **Los temas de lo escrito o pegado**: uno por linea, sin las vinetas, las
/// casillas (`- [ ]`) ni los numeros (`1.`, `2)`) de delante. Se saltan las
/// lineas vacias, los titulos de Markdown (`# Agenda`) y los repetidos; como
/// mucho [`MAXIMO`].
pub fn de_texto(texto: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for linea in texto.lines() {
        if !mini::titulo(linea.trim()).is_empty() {
            continue;
        }
        let t = limpio(linea);
        if t.is_empty() || v.iter().any(|x| clave(x) == clave(&t)) {
            continue;
        }
        v.push(t);
        if v.len() == MAXIMO {
            break;
        }
    }
    v
}

/// **Los temas de una lista de tareas** del chat (el documento de la
/// mini-app `tareas`): su texto sin la fecha ni las imagenes, y si ya
/// estaba hecha.
pub fn de_documento(documento: &str) -> Vec<(String, bool)> {
    let mut v: Vec<(String, bool)> = Vec::new();
    for t in mini::leer_tareas(documento) {
        let (visible, _) = mini::partir(&t.texto);
        let (sin_imagenes, _) = mini::imagenes_de(visible);
        let texto = limpio(&sin_imagenes);
        if texto.is_empty() || v.iter().any(|(x, _)| clave(x) == clave(&texto)) {
            continue;
        }
        v.push((texto, t.hecha));
        if v.len() == MAXIMO {
            break;
        }
    }
    v
}

/// El siguiente `id` libre: `t` y uno mas que el mayor que haya.
fn siguiente_id(temas: &[Tema]) -> String {
    let mayor = temas
        .iter()
        .filter_map(|t| t.id.strip_prefix('t')?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("t{}", mayor + 1)
}

/// **Apunta un tema** al final. Devuelve su `id`, o `None` si esta vacio,
/// repetido o ya hay [`MAXIMO`].
pub fn anadir(temas: &mut Vec<Tema>, texto: &str) -> Option<String> {
    let t = limpio(texto);
    if t.is_empty() || temas.len() >= MAXIMO || temas.iter().any(|x| clave(&x.texto) == clave(&t)) {
        return None;
    }
    let id = siguiente_id(temas);
    temas.push(Tema::nuevo(&id, &t));
    Some(id)
}

/// Apunta varios (lo pegado, una lista): devuelve cuantos entraron.
pub fn anadir_varios(temas: &mut Vec<Tema>, textos: &[String]) -> usize {
    textos.iter().filter(|t| anadir(temas, t).is_some()).count()
}

/// Quita el tema `id`. Devuelve si estaba.
pub fn quitar(temas: &mut Vec<Tema>, id: &str) -> bool {
    let antes = temas.len();
    temas.retain(|t| t.id != id);
    temas.len() != antes
}

/// **La persona marca o desmarca** el tema `id` (en la barra o al
/// estudiar). Manda siempre: queda `aMano` y Jev ya no lo toca. Al marcar
/// se apunta `ms` si se sabe; al desmarcar se olvidan el minuto y la `p`.
/// Devuelve si cambio algo.
pub fn a_mano(temas: &mut [Tema], id: &str, hecho: bool, ms: Option<i64>) -> bool {
    let Some(t) = temas.iter_mut().find(|t| t.id == id) else {
        return false;
    };
    let antes = t.clone();
    t.a_mano = true;
    t.por_jev = false;
    t.p = None;
    t.hecho = hecho;
    t.ms = if hecho {
        ms.map(|m| m.max(0)).or(t.ms)
    } else {
        None
    };
    *t != antes
}

/// **Jev marca el tema `id` como tratado** en `ms` con probabilidad `p`.
/// Solo si sigue pendiente y nadie lo toco a mano. Devuelve si lo marco.
pub fn por_jev(temas: &mut [Tema], id: &str, ms: i64, p: f64) -> bool {
    match temas.iter_mut().find(|t| t.id == id) {
        Some(t) if t.para_jev() => {
            t.hecho = true;
            t.por_jev = true;
            t.ms = Some(ms.max(0));
            t.p = Some(p.clamp(0.0, 1.0));
            true
        }
        _ => false,
    }
}

/// Los que Jev aun tiene que mirar, en orden.
pub fn para_jev(temas: &[Tema]) -> Vec<&Tema> {
    temas.iter().filter(|t| t.para_jev()).collect()
}

/// `(tratados, total)`.
pub fn cuenta(temas: &[Tema]) -> (usize, usize) {
    (temas.iter().filter(|t| t.hecho).count(), temas.len())
}

/// Los que quedaron sin tratar, en orden: lo que se ofrece pasar a la
/// proxima reunion.
pub fn sin_tratar(temas: &[Tema]) -> Vec<&Tema> {
    temas.iter().filter(|t| !t.hecho).collect()
}

/// Dos decimales, como `p` de las partes importantes.
fn dos_decimales(p: f64) -> f64 {
    (p.clamp(0.0, 1.0) * 100.0).round() / 100.0
}

/// **Los temas de una nota**, en el orden de la agenda. Vacio si no tiene;
/// lo que no se entiende (sin `id` o sin texto) se salta.
pub fn temas_de(resto: &Map<String, Value>) -> Vec<Tema> {
    let Some(a) = resto.get(TEMAS).and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut v: Vec<Tema> = Vec::new();
    for x in a {
        let (Some(id), Some(texto)) = (
            x.get("id").and_then(Value::as_str),
            x.get("texto").and_then(Value::as_str),
        ) else {
            continue;
        };
        if id.trim().is_empty() || texto.trim().is_empty() || v.iter().any(|t| t.id == id) {
            continue;
        }
        let si = |k: &str| x.get(k).and_then(Value::as_bool) == Some(true);
        v.push(Tema {
            id: id.to_string(),
            texto: texto.trim().to_string(),
            hecho: si("hecho"),
            ms: x.get("ms").and_then(Value::as_i64).map(|m| m.max(0)),
            p: x.get("p")
                .and_then(Value::as_f64)
                .map(|p| p.clamp(0.0, 1.0)),
            por_jev: si("porJev"),
            a_mano: si("aMano"),
        });
    }
    v
}

/// **Escribe los temas** en la nota. Sin temas, la clave se quita.
pub fn poner_temas(resto: &mut Map<String, Value>, temas: &[Tema]) {
    if temas.is_empty() {
        resto.remove(TEMAS);
        return;
    }
    let lista = temas
        .iter()
        .map(|t| {
            let mut o = Map::new();
            o.insert("id".into(), Value::from(t.id.as_str()));
            o.insert("texto".into(), Value::from(t.texto.as_str()));
            o.insert("hecho".into(), Value::from(t.hecho));
            if let Some(ms) = t.ms {
                o.insert("ms".into(), Value::from(ms.max(0)));
            }
            if let Some(p) = t.p {
                o.insert("p".into(), Value::from(dos_decimales(p)));
            }
            o.insert("porJev".into(), Value::from(t.por_jev));
            o.insert("aMano".into(), Value::from(t.a_mano));
            Value::Object(o)
        })
        .collect();
    resto.insert(TEMAS.into(), Value::Array(lista));
}

/// La lista de tareas enlazada, `(proyecto, id del mensaje)`.
pub fn lista_de(resto: &Map<String, Value>) -> Option<(String, String)> {
    let o = resto.get(TEMAS_LISTA)?;
    Some((
        o.get("proyecto")?.as_str()?.to_string(),
        o.get("id")?.as_str()?.to_string(),
    ))
}

/// Enlaza la lista de tareas que lleva los temas.
pub fn poner_lista(resto: &mut Map<String, Value>, proyecto: &str, id: &str) {
    resto.insert(
        TEMAS_LISTA.into(),
        serde_json::json!({ "proyecto": proyecto, "id": id }),
    );
}

/// **El documento de la lista de tareas** con los temas: `# titulo` y una
/// casilla por tema (`- [x]` lo tratado), el formato de la mini-app
/// `tareas`, que es lo que lee la ventana Tareas y el movil.
pub fn a_documento(titulo: &str, temas: &[Tema]) -> String {
    let tareas: Vec<mini::Tarea> = temas
        .iter()
        .map(|t| mini::Tarea {
            texto: t.texto.clone(),
            hecha: t.hecho,
        })
        .collect();
    mini::escribir_tareas(titulo, &tareas)
}

/// **Lo marcado en la lista de tareas mientras tanto** (en la ventana
/// Tareas o en el movil) entra en los temas como hecho a mano: es la
/// persona quien lo marco. Lo desmarcado alli no desmarca un tema que marco
/// Jev (solo se lee lo marcado: la lista puede ir por detras). Devuelve
/// cuantos cambiaron.
pub fn con_la_lista(temas: &mut [Tema], documento: &str) -> usize {
    let hechas: Vec<String> = de_documento(documento)
        .into_iter()
        .filter(|(_, h)| *h)
        .map(|(t, _)| clave(&t))
        .collect();
    let mut n = 0;
    for t in temas.iter_mut() {
        if !t.hecho && hechas.contains(&clave(&t.texto)) {
            t.hecho = true;
            t.a_mano = true;
            t.por_jev = false;
            t.p = None;
            n += 1;
        }
    }
    n
}

/// **Que casillas de la lista hay que cambiar** para que diga lo mismo que
/// los temas: `(numero de la tarea en el documento, hecha)`. Solo se marcan
/// (lo tratado); una tarea marcada en la lista sigue marcada.
pub fn marcas_para_la_lista(temas: &[Tema], documento: &str) -> Vec<(usize, bool)> {
    let hechos: Vec<String> = temas
        .iter()
        .filter(|t| t.hecho)
        .map(|t| clave(&t.texto))
        .collect();
    mini::leer_tareas(documento)
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.hecha)
        .filter(|(_, t)| {
            let (visible, _) = mini::partir(&t.texto);
            let (sin_imagenes, _) = mini::imagenes_de(visible);
            hechos.contains(&clave(&limpio(&sin_imagenes)))
        })
        .map(|(i, _)| (i, true))
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lo_pegado_sale_un_tema_por_linea_sin_vinetas_ni_repetidos() {
        let pegado = "# Agenda del lunes\n\n1. Presupuesto 2027\n2) Viaje a Lima:\n\
                      - [ ] Nuevo becario\n• presupuesto   2027\n   \n* Varios\n";
        assert_eq!(
            de_texto(pegado),
            vec![
                "Presupuesto 2027",
                "Viaje a Lima",
                "Nuevo becario",
                "Varios"
            ]
        );
        // Un tema larguisimo se recorta, sin partir un caracter.
        let largo = "ñ".repeat(500);
        let v = de_texto(&largo);
        assert_eq!(v[0].chars().count(), LARGO_MAXIMO + 1);
        // Mas de MAXIMO, se corta.
        let muchos: String = (0..60).map(|i| format!("tema {i}\n")).collect();
        assert_eq!(de_texto(&muchos).len(), MAXIMO);
        assert!(de_texto("").is_empty());
    }

    #[test]
    fn de_una_lista_de_tareas_sin_fechas_y_con_lo_hecho() {
        let doc =
            "# Reunion\n\n- [ ] Presupuesto 2027 \u{2795} 2026-10-27\n- [x] Viaje a Lima\n- [ ] \n";
        assert_eq!(
            de_documento(doc),
            vec![
                ("Presupuesto 2027".to_string(), false),
                ("Viaje a Lima".to_string(), true)
            ]
        );
    }

    #[test]
    fn los_ids_son_estables_y_no_se_reutilizan() {
        let mut v = Vec::new();
        assert_eq!(anadir(&mut v, "Uno"), Some("t1".into()));
        assert_eq!(anadir(&mut v, "Dos"), Some("t2".into()));
        assert_eq!(anadir(&mut v, "  uno "), None, "repetido");
        assert_eq!(anadir(&mut v, "   "), None, "vacio");
        assert!(quitar(&mut v, "t2"));
        assert!(!quitar(&mut v, "t2"));
        assert_eq!(anadir(&mut v, "Tres"), Some("t2".into()));
        assert!(quitar(&mut v, "t1"));
        assert_eq!(anadir(&mut v, "Cuatro"), Some("t3".into()));
        assert_eq!(
            anadir_varios(&mut v, &["Cinco".into(), "Tres".into(), "Seis".into()]),
            2
        );
    }

    #[test]
    fn lo_hecho_a_mano_manda_sobre_jev() {
        let mut v = vec![Tema::nuevo("t1", "A"), Tema::nuevo("t2", "B")];
        // Jev marca lo pendiente.
        assert!(por_jev(&mut v, "t1", 60_000, 0.86));
        assert!(v[0].hecho && v[0].por_jev && v[0].ms == Some(60_000));
        // Y no lo vuelve a marcar (ni le cambia el minuto).
        assert!(!por_jev(&mut v, "t1", 90_000, 0.99));
        assert_eq!(v[0].ms, Some(60_000));
        // La persona lo desmarca: Jev ya no lo toca.
        assert!(a_mano(&mut v, "t1", false, None));
        assert!(!v[0].hecho && v[0].a_mano && !v[0].por_jev && v[0].p.is_none());
        assert!(!por_jev(&mut v, "t1", 120_000, 0.95));
        assert!(!v[0].hecho);
        // Marcado a mano antes que Jev: Jev no lo cambia.
        assert!(a_mano(&mut v, "t2", true, Some(30_000)));
        assert!(!por_jev(&mut v, "t2", 40_000, 0.9));
        assert_eq!((v[1].ms, v[1].por_jev), (Some(30_000), false));
        assert!(para_jev(&v).is_empty());
        assert_eq!(cuenta(&v), (1, 2));
        assert_eq!(sin_tratar(&v).len(), 1);
        // Un id que no esta no hace nada.
        assert!(!a_mano(&mut v, "t9", true, None));
        assert!(!por_jev(&mut v, "t9", 0, 1.0));
    }

    #[test]
    fn van_y_vuelven_por_el_resto_y_sin_temas_la_clave_se_quita() {
        let mut v = vec![Tema::nuevo("t1", "Presupuesto"), Tema::nuevo("t2", "Viaje")];
        por_jev(&mut v, "t1", 750_000, 0.8649);
        let mut r = Map::new();
        poner_temas(&mut r, &v);
        let leidos = temas_de(&r);
        assert_eq!(leidos[0].p, Some(0.86));
        assert_eq!(leidos[0].ms, Some(750_000));
        assert_eq!(leidos[1], v[1]);
        let json = serde_json::to_string(&r[TEMAS]).unwrap();
        assert!(json.contains("\"porJev\":true") && json.contains("\"aMano\":false"));
        poner_temas(&mut r, &[]);
        assert!(r.is_empty());
        // Lo que no se entiende se salta.
        let mut r = Map::new();
        r.insert(
            TEMAS.into(),
            serde_json::json!([{"id": "t1"}, {"texto": "x"}, {"id": "t2", "texto": "ok", "hecho": true}]),
        );
        let v = temas_de(&r);
        assert_eq!(v.len(), 1);
        assert!(v[0].hecho && !v[0].a_mano);
        poner_lista(&mut r, "p1", "m7");
        assert_eq!(lista_de(&r), Some(("p1".into(), "m7".into())));
    }

    #[test]
    fn la_lista_de_tareas_dice_lo_mismo_que_los_temas() {
        let mut v = vec![
            Tema::nuevo("t1", "Presupuesto"),
            Tema::nuevo("t2", "Viaje"),
            Tema::nuevo("t3", "Varios"),
        ];
        let doc = a_documento("Temas · Reunion", &v);
        assert_eq!(
            doc,
            "# Temas · Reunion\n\n- [ ] Presupuesto\n- [ ] Viaje\n- [ ] Varios"
        );
        assert_eq!(de_documento(&doc).len(), 3);
        // En la ventana Tareas marcaron «Viaje»: entra como hecho a mano.
        let marcada = mini::alternar(&doc, 1);
        assert_eq!(con_la_lista(&mut v, &marcada), 1);
        assert!(v[1].hecho && v[1].a_mano);
        // Jev trato el primero: la lista se marca alli (la que ya lo estaba
        // no se toca).
        por_jev(&mut v, "t1", 0, 0.9);
        assert_eq!(marcas_para_la_lista(&v, &marcada), vec![(0, true)]);
    }
}
