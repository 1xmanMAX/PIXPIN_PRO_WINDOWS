//! Descodificar base64 sin traer una libreria.
//!
//! El puente de JavaScript manda el HTML nuevo como base64 dentro de un JSON,
//! y es lo unico que este crate necesita descodificar. Una dependencia mas en
//! una capa L1 por doscientas lineas de tabla no sale a cuenta, y ademas asi
//! la parte que puede machacar el fichero del usuario se prueba entera sin
//! ventana ni navegador.

/// Valor de 6 bits de un caracter base64, o `None` si no es del alfabeto.
///
/// Acepta a la vez el alfabeto estandar (`+/`) y el de URL (`-_`): las paginas
/// que genera PixPin Android usan `android.util.Base64`, que segun la bandera
/// escupe uno u otro, y distinguirlos aqui solo daria fallos raros.
fn valor(c: u8) -> Option<u32> {
    Some(match c {
        b'A'..=b'Z' => u32::from(c - b'A'),
        b'a'..=b'z' => u32::from(c - b'a') + 26,
        b'0'..=b'9' => u32::from(c - b'0') + 52,
        b'+' | b'-' => 62,
        b'/' | b'_' => 63,
        _ => return None,
    })
}

/// Descodifica base64. Devuelve `None` si aparece un caracter que no pinta
/// nada o si la longitud no cuadra.
///
/// Los espacios, saltos de linea y el relleno `=` se saltan porque los HTML
/// grandes llegan troceados en lineas y porque `btoa` del navegador siempre
/// rellena. Un prefijo `data:...;base64,` tambien se quita: algunas paginas
/// pasan el data-URL entero en vez de solo la carga.
pub fn descodificar(texto: &str) -> Option<Vec<u8>> {
    let cuerpo = match texto.find(";base64,") {
        Some(i) if texto.starts_with("data:") => &texto[i + ";base64,".len()..],
        _ => texto,
    };

    let mut salida = Vec::with_capacity(cuerpo.len() / 4 * 3);
    let mut acumulado: u32 = 0;
    let mut bits: u32 = 0;
    for c in cuerpo.bytes() {
        if c.is_ascii_whitespace() || c == b'=' {
            continue;
        }
        let v = valor(c)?;
        acumulado = (acumulado << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            salida.push(((acumulado >> bits) & 0xFF) as u8);
        }
    }

    // Sobrar 6 bits significa un caracter suelto al final: eso no lo produce
    // ningun codificador, asi que el texto venia cortado y prefiero no
    // entregar bytes a medias a quien va a sobrescribir un fichero.
    if bits >= 6 { None } else { Some(salida) }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn descodifica_un_texto_corriente() {
        assert_eq!(descodificar("aG9sYQ==").unwrap(), b"hola");
    }

    #[test]
    fn descodifica_aunque_falte_el_relleno() {
        assert_eq!(descodificar("aG9sYQ").unwrap(), b"hola");
    }

    #[test]
    fn se_salta_los_saltos_de_linea_que_mete_android() {
        assert_eq!(descodificar("aG9s\r\nYQ==\n").unwrap(), b"hola");
    }

    #[test]
    fn entiende_el_alfabeto_de_url() {
        // 0xFB 0xFF -> "+/8=" en estandar, "-_8=" en el de URL.
        assert_eq!(descodificar("-_8=").unwrap(), descodificar("+/8=").unwrap());
    }

    #[test]
    fn quita_el_prefijo_del_data_url() {
        assert_eq!(
            descodificar("data:text/html;base64,aG9sYQ==").unwrap(),
            b"hola"
        );
    }

    #[test]
    fn rechaza_un_caracter_que_no_es_del_alfabeto() {
        assert!(descodificar("aG9s*YQ==").is_none());
    }

    #[test]
    fn rechaza_un_texto_cortado_por_la_mitad_de_un_byte() {
        // Cinco caracteres: un grupo entero mas uno suelto, que no puede
        // venir de ningun codificador.
        assert!(descodificar("aG9sY").is_none());
    }

    #[test]
    fn descodifica_un_html_entero_ida_y_vuelta() {
        let html = format!("<html><body>{}</body></html>", "x".repeat(500));
        let codificado = codificar_de_prueba(html.as_bytes());
        assert_eq!(descodificar(&codificado).unwrap(), html.as_bytes());
    }

    /// Codificador minimo, solo para las pruebas: asi el ida y vuelta no
    /// depende de que yo escriba a mano una cadena larga sin equivocarme.
    fn codificar_de_prueba(bytes: &[u8]) -> String {
        const ALFABETO: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut salida = String::new();
        for trozo in bytes.chunks(3) {
            let mut bloque = [0u8; 3];
            bloque[..trozo.len()].copy_from_slice(trozo);
            let n =
                (u32::from(bloque[0]) << 16) | (u32::from(bloque[1]) << 8) | u32::from(bloque[2]);
            for i in 0..4 {
                if i <= trozo.len() {
                    salida.push(ALFABETO[((n >> (18 - 6 * i)) & 0x3F) as usize] as char);
                } else {
                    salida.push('=');
                }
            }
        }
        salida
    }
}
