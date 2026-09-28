//! El puente que la pagina ve como `window.PixPinVisor`.
//!
//! El nombre y la firma `guardar(nombre, base64)` vienen de PixPin Android:
//! los HTML que genera el chat ya los llaman asi, y cambiarlos aqui obligaria
//! a tocar todos los ficheros que el usuario ya tiene guardados. No se tocan.
//!
//! Del JSON que vuelve solo interesan dos campos, asi que se leen a mano en
//! vez de traer `serde`: en una capa L1 cada dependencia se paga en tiempo de
//! compilacion, y el valor que importa (`datos`) es base64, o sea que no
//! puede contener ni comillas ni barras invertidas que compliquen el parseo.

/// Script que se inyecta antes de cada documento.
///
/// Va en `AddScriptToExecuteOnDocumentCreated` y no en un `ExecuteScript` al
/// terminar de navegar porque la pagina puede llamar a `PixPinVisor.guardar`
/// desde el propio `<head>`, antes de que ningun evento de navegacion haya
/// llegado.
pub const SCRIPT: &str = r#"window.PixPinVisor = { guardar: function(nombre, base64) { window.chrome.webview.postMessage(JSON.stringify({tipo:"guardar", nombre:nombre, datos:base64})); } };"#;

/// Lee el valor de texto de un campo del JSON plano que manda el puente.
///
/// No es un analizador de JSON de verdad: solo busca `"<campo>"`, se salta los
/// espacios y los dos puntos, y lee la cadena entrecomillada siguiente. Con el
/// mensaje que produce [`SCRIPT`] basta, y cualquier otra cosa devuelve
/// `None`, que es exactamente lo que hay que hacer con un mensaje que no se
/// entiende.
pub fn campo_de_texto(json: &str, campo: &str) -> Option<String> {
    let aguja = format!("\"{campo}\"");
    let mut resto = &json[json.find(&aguja)? + aguja.len()..];

    resto = resto.trim_start();
    resto = resto.strip_prefix(':')?.trim_start();
    let mut chars = resto.chars();
    if chars.next()? != '"' {
        return None;
    }

    let mut valor = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(valor),
            '\\' => match chars.next()? {
                'n' => valor.push('\n'),
                'r' => valor.push('\r'),
                't' => valor.push('\t'),
                'b' => valor.push('\u{8}'),
                'f' => valor.push('\u{c}'),
                'u' => {
                    let mut hex = String::new();
                    for _ in 0..4 {
                        hex.push(chars.next()?);
                    }
                    let punto = u32::from_str_radix(&hex, 16).ok()?;
                    valor.push(char::from_u32(punto)?);
                }
                otro => valor.push(otro),
            },
            otro => valor.push(otro),
        }
    }
    // Se acabo la cadena sin comilla de cierre: el mensaje llego truncado.
    None
}

/// Si el mensaje es una peticion de guardado del puente.
pub fn es_peticion_de_guardado(json: &str) -> bool {
    campo_de_texto(json, "tipo").as_deref() == Some("guardar")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const MENSAJE: &str = r#"{"tipo":"guardar","nombre":"nota.html","datos":"aG9sYQ=="}"#;

    #[test]
    fn saca_los_tres_campos_del_mensaje_del_puente() {
        assert_eq!(campo_de_texto(MENSAJE, "tipo").unwrap(), "guardar");
        assert_eq!(campo_de_texto(MENSAJE, "nombre").unwrap(), "nota.html");
        assert_eq!(campo_de_texto(MENSAJE, "datos").unwrap(), "aG9sYQ==");
    }

    #[test]
    fn aguanta_los_espacios_que_mete_json_stringify_con_sangrado() {
        let con_espacios = "{ \"tipo\" : \"guardar\" , \"datos\" : \"QQ==\" }";
        assert_eq!(campo_de_texto(con_espacios, "datos").unwrap(), "QQ==");
    }

    #[test]
    fn deshace_los_escapes_del_nombre() {
        let json = r#"{"nombre":"mi \"nota\" ñ.html"}"#;
        assert_eq!(
            campo_de_texto(json, "nombre").unwrap(),
            "mi \"nota\" ñ.html"
        );
    }

    #[test]
    fn un_campo_que_no_esta_devuelve_nada() {
        assert!(campo_de_texto(MENSAJE, "otro").is_none());
    }

    #[test]
    fn un_mensaje_truncado_devuelve_nada() {
        assert!(campo_de_texto(r#"{"datos":"aG9s"#, "datos").is_none());
    }

    #[test]
    fn reconoce_la_peticion_de_guardado_y_descarta_las_demas() {
        assert!(es_peticion_de_guardado(MENSAJE));
        assert!(!es_peticion_de_guardado(r#"{"tipo":"hola"}"#));
        assert!(!es_peticion_de_guardado("no soy json"));
    }

    #[test]
    fn el_script_declara_el_nombre_y_el_metodo_que_espera_android() {
        assert!(SCRIPT.contains("window.PixPinVisor"));
        assert!(SCRIPT.contains("guardar: function(nombre, base64)"));
        assert!(SCRIPT.contains("window.chrome.webview.postMessage"));
    }
}
