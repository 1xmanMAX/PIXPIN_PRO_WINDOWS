//! **Compartir como enlace** (8-oct-2026): subir lo hecho a un servicio de
//! archivos temporales y pasar solo su direccion.
//!
//! Es `motor/SubirArchivo.kt` del movil, con los mismos servicios (el
//! usuario: «mantener los mismos servicios»), y con la duracion mas larga
//! que dan («que este el tiempo mas alto disponible»):
//!
//! - **litterbox**, «se borra en 3 dias», hasta 1 GB. Sirve el archivo con su
//!   tipo, asi que un `.html` se abre **como pagina**.
//! - **temp.sh**, de reserva: tambien 3 dias, hasta 4 GB; su enlace abre una
//!   pagina con un boton de descarga.
//!
//! Lo mismo que alli: es lo unico que manda algo fuera del equipo, y solo al
//! pulsar «Enlace». **Cualquiera con el enlace puede abrirlo** mientras dure.
//! Si el primero falla se prueba el otro (`con_reserva`).

use std::path::Path;

/// Un sitio donde dejar el archivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Servicio {
    pub id: &'static str,
    /// Lo que se le dice al usuario de cuanto dura.
    pub caduca: &'static str,
    /// Lo mas grande que admite, en MB.
    pub tope_mb: u64,
    /// Si su enlace abre el archivo con su tipo (y no una pagina de descarga).
    pub abre_el_archivo: bool,
    url: &'static str,
    campo: &'static str,
    extras: &'static [(&'static str, &'static str)],
}

/// Los servicios, en el orden en que se prueban: el de la duracion mas
/// larga de litterbox primero, y temp.sh de reserva.
pub const SERVICIOS: [Servicio; 2] = [
    Servicio {
        id: "litter72h",
        caduca: "3 días",
        tope_mb: 1000,
        abre_el_archivo: true,
        url: "https://litterbox.catbox.moe/resources/internals/api.php",
        campo: "fileToUpload",
        extras: &[("reqtype", "fileupload"), ("time", "72h")],
    },
    Servicio {
        id: "tempsh",
        caduca: "3 días",
        tope_mb: 4000,
        abre_el_archivo: false,
        url: "https://temp.sh/upload",
        campo: "file",
        extras: &[],
    },
];

/// Si el archivo cabe en ese servicio.
pub fn cabe(s: &Servicio, bytes: u64) -> bool {
    bytes <= s.tope_mb * 1_000_000
}

/// El enlace que hay en la respuesta: unos contestan la direccion pelada y
/// otros un JSON con `"link"`. Como `enlaceDe` del movil.
pub fn enlace_de(cuerpo: &str) -> Option<String> {
    let limpio = cuerpo.trim();
    if let Some(l) = limpio
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("https://") && !l.contains(' ') && l.len() < 300)
    {
        return Some(l.to_string());
    }
    let marca = "\"link\":";
    let i = limpio.find(marca)?;
    let desde = limpio[i + marca.len()..].find('"')? + i + marca.len();
    let hasta = limpio[desde + 1..].find('"')? + desde + 1;
    let url = limpio[desde + 1..hasta].replace("\\/", "/");
    url.starts_with("https://").then_some(url)
}

/// Lo que contesto el servicio, recortado, para decir por que no salio.
pub fn motivo_de(cuerpo: &str) -> String {
    let mut sin_etiquetas = String::new();
    let mut dentro = false;
    for c in cuerpo.chars() {
        match c {
            '<' => dentro = true,
            '>' => {
                dentro = false;
                sin_etiquetas.push(' ');
            }
            _ if !dentro => sin_etiquetas.push(c),
            _ => {}
        }
    }
    let texto = sin_etiquetas.split_whitespace().collect::<Vec<_>>().join(" ");
    if texto.is_empty() {
        "el servicio no contestó nada".into()
    } else {
        texto.chars().take(120).collect()
    }
}

/// Un nombre de fichero que no rompa la cabecera, con su extension.
pub fn sano(nombre: &str) -> String {
    let mut s = String::new();
    let mut guion = false;
    for c in nombre.chars() {
        if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\r' | '\n') {
            if !guion {
                s.push('-');
            }
            guion = true;
        } else {
            s.push(c);
            guion = false;
        }
    }
    let s = s.trim().to_string();
    if s.is_empty() { "archivo".into() } else { s }
}

/// El tipo de un fichero por su extension, el que el servicio usara para
/// servirlo.
fn tipo_de(ruta: &Path) -> &'static str {
    match ruta
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .as_deref()
    {
        Some("html" | "htm") => "text/html",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("txt" | "md") => "text/plain",
        Some("csv") => "text/csv",
        _ => "application/octet-stream",
    }
}

/// **Sube `fichero`** y devuelve su enlace y el servicio. Prueba primero
/// litterbox y despues temp.sh, saltandose los que no lo admiten por peso;
/// con una pagina web, los que la abren van antes. Bloquea: hilo de fondo.
pub fn con_reserva(
    fichero: &Path,
    avance: &mut dyn FnMut(f32),
) -> Result<(String, Servicio), String> {
    let bytes = std::fs::metadata(fichero)
        .map_err(|e| format!("no hay nada que subir: {e}"))?
        .len();
    if bytes == 0 {
        return Err("no hay nada que subir".into());
    }
    let nombre = sano(
        &fichero
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );
    let tipo = tipo_de(fichero);
    let mut cola: Vec<Servicio> = SERVICIOS.to_vec();
    if tipo == "text/html" {
        cola.sort_by_key(|s| !s.abre_el_archivo);
    }
    let mut ultimo = "no se pudo subir".to_string();
    for s in cola.iter().filter(|s| cabe(s, bytes)) {
        match pixpin_shell::subir::subir(s.url, s.extras, s.campo, fichero, &nombre, tipo, avance) {
            Ok(r) if (200..300).contains(&r.codigo) => match enlace_de(&r.cuerpo) {
                Some(url) => {
                    tracing::info!(servicio = s.id, bytes, "subido para compartir como enlace");
                    return Ok((url, *s));
                }
                None => ultimo = motivo_de(&r.cuerpo),
            },
            Ok(r) => ultimo = format!("el servicio contestó {}", r.codigo),
            Err(e) => ultimo = e,
        }
        tracing::info!(servicio = s.id, %ultimo, "no se pudo subir; se prueba el siguiente");
    }
    if !cola.iter().any(|s| cabe(s, bytes)) {
        ultimo = format!("pesa {} MB y como mucho se admiten 4000", bytes / 1_000_000);
    }
    Err(ultimo)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_enlace_sale_de_la_direccion_pelada_o_del_json() {
        assert_eq!(
            enlace_de("https://litter.catbox.moe/abc123.html\n").as_deref(),
            Some("https://litter.catbox.moe/abc123.html")
        );
        assert_eq!(
            enlace_de(r#"{"ok":true,"link":"https:\/\/temp.sh\/x\/y.pdf"}"#).as_deref(),
            Some("https://temp.sh/x/y.pdf")
        );
        // Casos negativos: un error o un http sin cifrar no son enlace.
        assert_eq!(enlace_de("Error: file too big"), None);
        assert_eq!(enlace_de(r#"{"link":"http://x"}"#), None);
    }

    #[test]
    fn el_motivo_es_el_texto_sin_etiquetas_y_corto() {
        assert_eq!(motivo_de("<h1>413</h1> <p>Too   large</p>"), "413 Too large");
        assert_eq!(motivo_de("   "), "el servicio no contestó nada");
        assert!(motivo_de(&"x".repeat(500)).chars().count() <= 120);
    }

    #[test]
    fn el_nombre_no_rompe_la_cabecera_y_lo_que_no_cabe_se_salta() {
        assert_eq!(sano("Lista: obra/2026?.pdf"), "Lista- obra-2026-.pdf");
        assert_eq!(sano("  "), "archivo");
        let [litter, temp] = SERVICIOS;
        assert!(cabe(&litter, 900_000_000) && !cabe(&litter, 1_500_000_000));
        assert!(cabe(&temp, 1_500_000_000));
        assert_eq!(litter.caduca, "3 días");
    }

    /// Sube de verdad un texto de prueba y dice el enlace. Sale a internet:
    /// solo a mano, con `--ignored`.
    #[test]
    #[ignore = "sube un fichero de prueba a litterbox"]
    fn sube_de_verdad_un_texto_de_prueba() {
        let f = std::env::temp_dir().join("pixpin-prueba-enlace.txt");
        std::fs::write(&f, "prueba de PixPin: se borra sola en 3 días").unwrap();
        let (url, s) = con_reserva(&f, &mut |_| {}).expect("subido");
        println!("ENLACE {url} ({})", s.id);
        assert!(url.starts_with("https://"));
        let _ = std::fs::remove_file(&f);
    }
}
