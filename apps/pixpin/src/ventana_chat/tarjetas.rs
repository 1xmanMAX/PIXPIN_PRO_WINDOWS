//! **Lo que una burbuja dice ademas de su texto**: la tarjeta de un enlace,
//! de quien llego y cuando se va del buzon.
//!
//! Copia de `guardados/Enlaces.kt` y de los trozos de `MensajesActivity.kt`
//! que pintan «Recibido de …» y el pie de fechas del buzon. Va aparte de
//! `ventana_chat.rs` porque todo esto es texto que entra y texto que sale:
//! se comprueba en `cargo test` sin ventana ni pintor, y los casos raros
//! (el punto final de la frase, el parentesis que envuelve la direccion)
//! fallan en silencio si no se comprueban.
//!
//! ## Por que la tarjeta del enlace no sale a la red
//!
//! El movil lo decidio asi y aqui se respeta igual (`Enlaces.kt:6-17`): la
//! tarjeta se hace con lo que **la propia direccion** ya dice —de donde es y
//! de que va—. Traer el titulo o la imagen de la pagina obligaria a visitar
//! cada direccion guardada, y una aplicacion que guarda documentos privados
//! y no habla con nadie es una promesa que no se rompe para adornar.

use pixpin_proyecto::cuaderno::Mensaje;

/// Un enlace como se ensena en su tarjeta (`Enlace` del movil).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enlace {
    /// La direccion completa, la que se abre.
    pub url: String,
    /// De donde es: `github.com`. Lo primero que se lee y lo que da confianza.
    pub donde: String,
    /// De que va: el camino con las barras aireadas (`forge / pixpin`).
    pub de_que: String,
}

/// Lo que la puntuacion suele dejar pegado al final sin ser de la direccion.
const PEGADOS_AL_FINAL: &str = ".,;:!?)]}\"'»";

/// El primer enlace del texto, si lo hay (`primerEnlace`).
///
/// El primero y no todos: la tarjeta es una, y con dos enlaces en una nota lo
/// que previsualiza cualquier mensajeria es el primero.
pub fn primer_enlace(texto: &str) -> Option<Enlace> {
    let crudo = buscar(texto, &["https://", "http://"]).or_else(|| buscar(texto, &["www."]))?;
    let crudo = recortar_lo_pegado(crudo);
    if crudo.is_empty() {
        return None;
    }
    let sin_protocolo = crudo.split_once("://").map_or(crudo, |(_, r)| r);
    let hasta = sin_protocolo
        .find(['/', '?'])
        .unwrap_or(sin_protocolo.len());
    let servidor = sin_protocolo[..hasta].to_lowercase();
    // `https://localhost` no es un sitio que se reconozca por su nombre.
    if !servidor.contains('.') {
        return None;
    }
    // El «www.» se ensena quitado: no distingue nada y roba cuatro letras de
    // la linea donde se reconoce el dominio. La direccion que se abre si lo
    // conserva.
    let donde = servidor
        .strip_prefix("www.")
        .unwrap_or(&servidor)
        .to_string();
    // Se corta por lo que mide el servidor y no quitando su texto: el movil
    // quitaba el servidor ya en minusculas y con «Ejemplo.COM/Cosa» dejaba
    // el dominio metido en el camino.
    let camino = sin_protocolo[hasta..].trim_matches('/');
    let camino = camino.split('?').next().unwrap_or("");
    let de_que = camino
        .split('/')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
    let url = if crudo.contains("://") {
        crudo.to_string()
    } else {
        format!("https://{crudo}")
    };
    Some(Enlace { url, donde, de_que })
}

/// El primer trozo sin espacios que empieza por alguno de `inicios` al
/// principio de una palabra, sin mirar mayusculas (el `\b` de la expresion
/// regular del movil, que con `IGNORE_CASE` acepta `HTTPS://`).
fn buscar<'a>(texto: &'a str, inicios: &[&str]) -> Option<&'a str> {
    let minusculas = texto.to_ascii_lowercase();
    let mut mejor: Option<usize> = None;
    for inicio in inicios {
        let mut desde = 0;
        while let Some(n) = minusculas[desde..].find(inicio) {
            let i = desde + n;
            let antes = texto[..i].chars().next_back();
            if antes.is_none_or(|c| !(c.is_alphanumeric() || c == '_')) {
                mejor = Some(mejor.map_or(i, |m| m.min(i)));
                break;
            }
            desde = i + inicio.len();
        }
    }
    let i = mejor?;
    let resto = &texto[i..];
    let fin = resto.find(char::is_whitespace).unwrap_or(resto.len());
    Some(&resto[..fin])
}

/// Quita la puntuacion pegada al final (`recortarLoPegado`).
///
/// «Mira esto: ejemplo.com/cosa.» abriria `cosa.`, que no existe. Los
/// parentesis se cuentan: si la direccion lleva uno abierto dentro (los
/// enlaces de enciclopedia lo hacen), el que cierra **si es suyo**.
fn recortar_lo_pegado(crudo: &str) -> &str {
    let mut fin = crudo.len();
    while let Some(c) = crudo[..fin].chars().next_back() {
        if !PEGADOS_AL_FINAL.contains(c) {
            break;
        }
        let hasta = &crudo[..fin];
        let abiertos = hasta.matches('(').count() as i64;
        let cerrados = hasta.matches(')').count() as i64;
        if c == ')' && abiertos > cerrados - 1 {
            break;
        }
        fin -= c.len_utf8();
    }
    &crudo[..fin]
}

/// **De quien llego**, si vino de otro aparato: «Max phone · K7Q2».
///
/// El `Mensaje` de aqui no declara el campo: vive en `resto`, que guarda tal
/// cual lo que el movil anade, y asi reescribir el cuaderno no lo pierde.
pub fn recibido_de(m: &Mensaje) -> Option<&str> {
    m.resto
        .get("recibidoDe")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Cuanto aguanta algo en el buzon antes de irse solo (`DIAS_DEL_BUZON`).
pub const DIAS_DEL_BUZON: i64 = 7;

/// Cuando se va del buzon algo que llego en `cuando` (milisegundos).
pub fn se_va_el(cuando: i64) -> i64 {
    cuando + DIAS_DEL_BUZON * 86_400_000
}

/// El mes corto de la fecha del buzon («sep», «Sep»), sacado del nombre
/// entero del catalogo: asi no hace falta una segunda lista de doce meses
/// por idioma, y en el buzon nada dura mas de una semana, asi que con el mes
/// corto y el dia basta (`cortaDe`, patron «d MMM»).
pub fn mes_corto(nombre: &str) -> String {
    nombre.trim().chars().take(3).collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn e(texto: &str) -> Enlace {
        primer_enlace(texto).expect("tenia que encontrar un enlace")
    }

    #[test]
    fn una_direccion_suelta_se_reconoce() {
        let x = e("https://github.com/forge/pixpin");
        assert_eq!(x.donde, "github.com");
        assert_eq!(x.de_que, "forge / pixpin");
        assert_eq!(x.url, "https://github.com/forge/pixpin");
    }

    #[test]
    fn dentro_de_una_frase_tambien() {
        let x = e("mira esto https://ejemplo.com/cosa que está bien");
        assert_eq!(x.donde, "ejemplo.com");
        assert_eq!(x.de_que, "cosa");
    }

    #[test]
    fn la_puntuacion_del_final_no_entra_en_la_direccion() {
        assert_eq!(
            e("mira https://ejemplo.com/cosa.").url,
            "https://ejemplo.com/cosa"
        );
        assert_eq!(e("https://ejemplo.com, y luego").url, "https://ejemplo.com");
        assert_eq!(e("https://ejemplo.com: mira").url, "https://ejemplo.com");
    }

    #[test]
    fn el_parentesis_de_la_frase_se_queda_fuera_y_el_de_dentro_no() {
        assert_eq!(
            e("lo puse (https://ejemplo.com/x) ahí").url,
            "https://ejemplo.com/x"
        );
        assert_eq!(
            e("https://es.wikipedia.org/wiki/Pin_(sujeción))").url,
            "https://es.wikipedia.org/wiki/Pin_(sujeción)"
        );
    }

    #[test]
    fn sin_protocolo_pero_con_www_vale_y_el_www_no_se_ensena() {
        let x = e("entra en www.ejemplo.com/algo");
        assert_eq!(x.donde, "ejemplo.com");
        assert_eq!(x.url, "https://www.ejemplo.com/algo");
    }

    #[test]
    fn el_primero_es_el_que_manda() {
        assert_eq!(e("https://uno.com y https://dos.com").donde, "uno.com");
        // Aunque el de `www.` vaya antes, uno con protocolo gana, como en el
        // movil (primero se busca con protocolo y solo si no hay, sin el).
        assert_eq!(e("www.uno.com y https://dos.com").donde, "dos.com");
    }

    #[test]
    fn un_texto_sin_enlaces_no_inventa_ninguno() {
        assert_eq!(primer_enlace("esto no lleva ninguna dirección"), None);
        assert_eq!(primer_enlace(""), None);
        assert_eq!(primer_enlace("https://localhost"), None);
        // Pegado a una palabra no es principio de enlace.
        assert_eq!(primer_enlace("xhttps://ejemplo.com"), None);
        assert_eq!(primer_enlace("nowww.ejemplo.com"), None);
    }

    #[test]
    fn el_dominio_se_lee_en_minusculas_y_el_camino_sin_el() {
        let x = e("HTTPS://Ejemplo.COM/Cosa");
        assert_eq!(x.donde, "ejemplo.com");
        assert_eq!(x.de_que, "Cosa");
    }

    #[test]
    fn la_parte_de_las_preguntas_no_se_ensena_pero_se_abre() {
        let x = e("https://ejemplo.com/buscar?q=algo&x=1");
        assert_eq!(x.de_que, "buscar");
        assert_eq!(x.url, "https://ejemplo.com/buscar?q=algo&x=1");
    }

    #[test]
    fn una_direccion_sin_camino_se_queda_sin_la_segunda_linea() {
        let x = e("https://ejemplo.com");
        assert_eq!(x.donde, "ejemplo.com");
        assert_eq!(x.de_que, "");
    }

    #[test]
    fn recibido_de_sale_de_lo_que_escribe_el_movil() {
        let m: Mensaje =
            serde_json::from_str(r#"{"id":"1","recibidoDe":"Max phone · K7Q2"}"#).unwrap();
        assert_eq!(recibido_de(&m), Some("Max phone · K7Q2"));
        // Casos negativos: sin el campo, vacio o de otro tipo no dice nada.
        let sin: Mensaje = serde_json::from_str(r#"{"id":"1"}"#).unwrap();
        assert_eq!(recibido_de(&sin), None);
        let vacio: Mensaje = serde_json::from_str(r#"{"id":"1","recibidoDe":"  "}"#).unwrap();
        assert_eq!(recibido_de(&vacio), None);
        let raro: Mensaje = serde_json::from_str(r#"{"id":"1","recibidoDe":3}"#).unwrap();
        assert_eq!(recibido_de(&raro), None);
    }

    #[test]
    fn lo_del_buzon_se_va_a_los_siete_dias() {
        assert_eq!(se_va_el(0), 7 * 86_400_000);
        assert_ne!(se_va_el(0), 6 * 86_400_000);
    }

    #[test]
    fn el_mes_corto_son_sus_tres_primeras_letras() {
        assert_eq!(mes_corto("septiembre"), "sep");
        assert_eq!(mes_corto("September"), "Sep");
        // Sin partir una letra con tilde por la mitad.
        assert_eq!(mes_corto("ñandú"), "ñan");
        assert_eq!(mes_corto(""), "");
    }
}
