//! Buscar sin tildes ni mayusculas, y cuanto vale cada coincidencia.
//!
//! Las reglas son las de `IndiceBusqueda` del universo: el principio del
//! nombre gana a una palabra del nombre, que gana a un trozo del nombre, que
//! gana al texto de dentro. Entre iguales decide lo mas reciente (eso lo hace
//! quien ordena, no esto).

/// Minusculas y sin tildes: «Gestión» y «gestion» son lo mismo.
pub fn normalizar(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len());
    for c in texto.chars() {
        let c = match c {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'Á' | 'À' | 'Ä' | 'Â' | 'Ã' => 'a',
            'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'Ó' | 'Ò' | 'Ö' | 'Ô' | 'Õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
            'ñ' | 'Ñ' => 'n',
            'ç' | 'Ç' => 'c',
            otro => otro,
        };
        s.extend(c.to_lowercase());
    }
    s
}

/// Puntos por coincidir en el nombre entero.
pub const IGUAL: u32 = 1100;
/// El nombre empieza por lo tecleado.
pub const PREFIJO: u32 = 1000;
/// Alguna palabra del nombre empieza por lo tecleado.
pub const PALABRA: u32 = 800;
/// Lo tecleado esta en medio del nombre.
pub const TROZO: u32 = 600;
/// Cada palabra tecleada empieza una palabra del nombre.
pub const PALABRAS: u32 = 500;
/// Cada palabra tecleada esta en el nombre.
pub const TROZOS: u32 = 400;
/// Lo tecleado esta en el texto de dentro.
pub const DENTRO: u32 = 300;
/// Cada palabra tecleada esta en el nombre o en el texto.
pub const DENTRO_TROZOS: u32 = 200;
/// Las letras estan en orden en el nombre (la parte «fuzzy»).
pub const SALTEADO: u32 = 100;

/// Cuanto vale `consulta` contra `nombre` y `dentro`, todo ya normalizado.
/// `0` es que no coincide.
pub fn puntuar(consulta: &str, nombre: &str, dentro: &str) -> u32 {
    let q = consulta.trim();
    if q.is_empty() {
        return 0;
    }
    if nombre == q {
        return IGUAL;
    }
    if nombre.starts_with(q) {
        return PREFIJO;
    }
    if empieza_palabra(nombre, q) {
        return PALABRA;
    }
    if nombre.contains(q) {
        return TROZO;
    }
    let palabras: Vec<&str> = q.split_whitespace().collect();
    if palabras.len() > 1 {
        if palabras.iter().all(|p| empieza_palabra(nombre, p)) {
            return PALABRAS;
        }
        if palabras.iter().all(|p| nombre.contains(p)) {
            return TROZOS;
        }
    }
    if !dentro.is_empty() {
        if dentro.contains(q) {
            return DENTRO;
        }
        if palabras.len() > 1 && palabras.iter().all(|p| nombre.contains(p) || dentro.contains(p)) {
            return DENTRO_TROZOS;
        }
    }
    if q.chars().count() >= 3 && !q.contains(' ') && salteado(nombre, q) {
        return SALTEADO;
    }
    0
}

/// Si alguna palabra de `texto` empieza por `q` (una palabra empieza al
/// principio o detras de algo que no es letra ni cifra).
fn empieza_palabra(texto: &str, q: &str) -> bool {
    let mut anterior: Option<char> = None;
    for (i, c) in texto.char_indices() {
        let inicio = anterior.is_none_or(|a| !a.is_alphanumeric());
        if inicio && c.is_alphanumeric() && texto[i..].starts_with(q) {
            return true;
        }
        anterior = Some(c);
    }
    false
}

/// Las letras de `q` aparecen en orden dentro de `texto`, y cada una o va
/// pegada a la anterior o empieza una palabra: «gdp» y «gestproy» encajan con
/// «gestion de proyectos», pero «tar» no con «transcripcion» (eso seria ruido).
fn salteado(texto: &str, q: &str) -> bool {
    let q: Vec<char> = q.chars().collect();
    let mut i = 0;
    let mut anterior: Option<char> = None;
    let mut pegada = false;
    for c in texto.chars() {
        let inicio = c.is_alphanumeric() && anterior.is_none_or(|a| !a.is_alphanumeric());
        anterior = Some(c);
        if i < q.len() && c == q[i] && (inicio || (pegada && i > 0)) {
            i += 1;
            pegada = true;
        } else {
            pegada = false;
        }
    }
    i == q.len()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn quita_tildes_y_mayusculas() {
        assert_eq!(normalizar("Gestión de PROYECTOS"), "gestion de proyectos");
        assert_eq!(normalizar("Añadir Canción ÜBER"), "anadir cancion uber");
    }

    #[test]
    fn el_principio_gana_a_la_palabra_y_esta_al_trozo() {
        assert_eq!(puntuar("gest", "gestion de proyectos", ""), PREFIJO);
        assert_eq!(puntuar("proy", "gestion de proyectos", ""), PALABRA);
        assert_eq!(puntuar("yect", "gestion de proyectos", ""), TROZO);
        assert_eq!(puntuar("gestion de proyectos", "gestion de proyectos", ""), IGUAL);
    }

    #[test]
    fn varias_palabras_en_cualquier_orden() {
        assert_eq!(puntuar("proy gest", "gestion de proyectos", ""), PALABRAS);
        assert_eq!(puntuar("yect stion", "gestion de proyectos", ""), TROZOS);
        assert_eq!(puntuar("pan leche", "compra", "pan y leche"), DENTRO_TROZOS);
    }

    #[test]
    fn el_texto_de_dentro_cuenta_menos() {
        assert_eq!(puntuar("leche", "compra", "pan y leche"), DENTRO);
        assert_eq!(puntuar("vino", "compra", "pan y leche"), 0);
    }

    #[test]
    fn letras_salteadas_desde_el_principio_de_una_palabra() {
        assert_eq!(puntuar("gdp", "gestion de proyectos", ""), SALTEADO);
        assert_eq!(puntuar("gestproy", "gestion de proyectos", ""), SALTEADO);
        assert_eq!(puntuar("xyz", "gestion de proyectos", ""), 0);
        assert_eq!(puntuar("tar", "lo que te falta mostrar", ""), 0);
        // Dos letras no bastan: casi todo coincidiria.
        assert_eq!(puntuar("gp", "gestion de proyectos", ""), 0);
    }

    #[test]
    fn nada_tecleado_no_coincide_con_nada() {
        assert_eq!(puntuar("  ", "algo", "algo"), 0);
    }
}
