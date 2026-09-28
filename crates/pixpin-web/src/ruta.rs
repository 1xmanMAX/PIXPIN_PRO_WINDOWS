//! De una ruta de Windows a la URL `file:` que entiende WebView2.
//!
//! WebView2 no admite `Navigate("C:\\...")`: exige una URL. Y no basta con
//! cambiar las barras, porque las carpetas del usuario llevan espacios,
//! acentos y `#`, y un `#` sin escapar corta la ruta por la mitad y abre otro
//! fichero (o ninguno).

use std::path::Path;

/// Caracteres que pueden ir tal cual en la parte de ruta de una URL.
///
/// Se deja pasar `:` para la letra de unidad y `/` porque ya es el separador.
fn se_deja_tal_cual(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.' | b'~' | b'/' | b':')
}

/// URL `file:` de una ruta absoluta de Windows.
pub fn url_de_fichero(ruta: &Path) -> String {
    let texto = ruta.to_string_lossy().replace('\\', "/");
    let mut url = String::from("file:///");
    for byte in texto.bytes() {
        if se_deja_tal_cual(byte) {
            url.push(byte as char);
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    url
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn una_ruta_sencilla_se_convierte_en_url() {
        assert_eq!(
            url_de_fichero(&PathBuf::from(r"C:\temp\nota.html")),
            "file:///C:/temp/nota.html"
        );
    }

    #[test]
    fn los_espacios_van_escapados() {
        assert_eq!(
            url_de_fichero(&PathBuf::from(r"C:\Mis documentos\nota.html")),
            "file:///C:/Mis%20documentos/nota.html"
        );
    }

    #[test]
    fn la_almohadilla_va_escapada_para_que_no_corte_la_ruta() {
        let url = url_de_fichero(&PathBuf::from(r"C:\a#b\nota.html"));
        assert!(!url.contains('#'), "quedo sin escapar: {url}");
        assert_eq!(url, "file:///C:/a%23b/nota.html");
    }

    #[test]
    fn los_acentos_salen_en_utf8_escapado() {
        assert_eq!(
            url_de_fichero(&PathBuf::from(r"C:\ñ.html")),
            "file:///C:/%C3%B1.html"
        );
    }
}
