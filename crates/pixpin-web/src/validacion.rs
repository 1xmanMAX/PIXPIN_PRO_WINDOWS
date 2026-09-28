//! El filtro que decide si unos bytes merecen sobrescribir el fichero.
//!
//! Es la traduccion literal de `VisorHtmlActivity.kt:1170-1195` de PixPin
//! Android. La razon de existir no es la elegancia: por el mismo camino de
//! guardado entra cualquier descarga que arranque la pagina (una fuente, un
//! JSON de telemetria, un error 404 servido como texto), y ahi no hay segunda
//! oportunidad: el destino es el `.html` del usuario. Mas vale dejar pasar de
//! largo un guardado que quedarse sin el original.

/// Tamano por debajo del cual no se acepta nada. Copiado de Android.
///
/// Doscientos bytes no llegan ni para un HTML vacio con su `<head>`, asi que
/// cualquier cosa mas corta es un aviso de error o una descarga equivocada.
pub const MINIMO_BYTES: usize = 200;

/// Cierre que tiene que aparecer para creerse que esto es la pagina.
const CIERRE: &str = "</html>";

/// Si estos bytes parecen el HTML completo que la pagina queria guardar.
pub fn parece_html_completo(bytes: &[u8]) -> bool {
    if bytes.len() <= MINIMO_BYTES {
        return false;
    }
    // `from_utf8_lossy` y no un `from_utf8` estricto porque un HTML guardado
    // desde el navegador puede traer trozos en otra codificacion dentro de un
    // atributo; buscar el cierre no necesita que todo el fichero sea UTF-8
    // valido. Y en minusculas porque hay plantillas que escriben `</HTML>`.
    String::from_utf8_lossy(bytes)
        .to_lowercase()
        .contains(CIERRE)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn html_valido() -> Vec<u8> {
        format!("<html><body>{}</body></html>", "a".repeat(300)).into_bytes()
    }

    #[test]
    fn un_html_entero_pasa_el_filtro() {
        assert!(parece_html_completo(&html_valido()));
    }

    #[test]
    fn un_fichero_corto_no_pasa_aunque_lleve_el_cierre() {
        assert!(!parece_html_completo(b"<html></html>"));
    }

    #[test]
    fn justo_en_el_limite_de_doscientos_bytes_no_pasa() {
        let mut bytes = b"</html>".to_vec();
        bytes.resize(MINIMO_BYTES, b' ');
        assert!(!parece_html_completo(&bytes));
    }

    #[test]
    fn un_byte_mas_que_el_limite_ya_pasa() {
        let mut bytes = b"</html>".to_vec();
        bytes.resize(MINIMO_BYTES + 1, b' ');
        assert!(parece_html_completo(&bytes));
    }

    #[test]
    fn un_fichero_largo_sin_el_cierre_no_pasa() {
        assert!(!parece_html_completo(&vec![b'a'; 5000]));
    }

    #[test]
    fn el_cierre_en_mayusculas_tambien_vale() {
        let html = format!("<HTML><BODY>{}</BODY></HTML>", "a".repeat(300));
        assert!(parece_html_completo(html.as_bytes()));
    }

    #[test]
    fn unos_bytes_que_no_son_texto_no_pasan() {
        assert!(!parece_html_completo(&vec![0xFFu8; 5000]));
    }
}
