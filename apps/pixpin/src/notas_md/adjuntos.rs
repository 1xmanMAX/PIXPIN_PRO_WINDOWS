//! **Las fotos de una nota**: donde se guardan y como se nombran en el
//! Markdown para que viajen con ella, como en el movil.
//!
//! El movil **copia** la foto en `files/notas/<hora>-<nombre>` (no la enlaza:
//! el `content://` del selector caduca) y escribe su ruta en la nota
//! (`Adjuntos.importar`); al sincronizar, la ruta sale como
//! `pixpin:files/notas/…` y el fichero viaja con la nota, porque la
//! sincronizacion lleva todo lo que un mensaje o un proyecto nombra con
//! `pixpin:files/` (`alcance_de`).
//!
//! Aqui igual: la foto se copia a `notas/<hora>-<nombre>` **dentro de la
//! carpeta del proyecto** y en el Markdown va su ruta portatil
//! (`pixpin:files/guardados/pc/<chat>/notas/…`, `vista::portatil_de_ruta`),
//! que el movil sabe traer y abrir. Un `.md` suelto guarda sus fotos al
//! lado, en `adjuntos/`, con ruta relativa: asi la carpeta se puede mover
//! entera.

use std::path::{Path, PathBuf};

use pixpin_proyecto::{almacen, vista};

use super::Destino;

/// El nombre de la copia: sin las letras que romperian el `![](…)` o una
/// ruta (`Adjuntos.nombreDe`), sin espacios (un lector de Markdown corta
/// ahi la ruta) y con 80 letras como mucho.
pub fn nombre_limpio(nombre: &str) -> String {
    let limpio: String = nombre
        .chars()
        .map(|c| {
            if "/\\:|()[] ".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .take(80)
        .collect();
    if limpio.trim_matches('_').is_empty() {
        "imagen".into()
    } else {
        limpio
    }
}

/// Donde esta en este equipo la foto que la nota nombra con `ruta`.
pub fn resolver(raiz: &Path, destino: &Destino, ruta: &str) -> Option<PathBuf> {
    let ruta = ruta.trim();
    // Una ruta de este equipo escrita a mano tambien vale.
    if Path::new(ruta).is_absolute() && !ruta.starts_with('/') {
        return Some(PathBuf::from(ruta)).filter(|p| p.is_file());
    }
    match destino {
        Destino::Mensaje { proyecto, .. } | Destino::Nueva { proyecto } => {
            vista::ruta_real(raiz, proyecto, ruta)
        }
        Destino::Fichero { ruta: md } => {
            if ruta.contains(':') || ruta.starts_with('/') || ruta.contains("..") {
                return None;
            }
            md.parent().map(|d| d.join(ruta.replace('/', "\\")))
        }
    }
}

/// Donde va un fichero `fichero` de la nota (una foto, una pagina viva) y
/// la ruta con que se escribe en el Markdown. Crea la carpeta.
pub fn sitio(raiz: &Path, destino: &Destino, fichero: &str) -> std::io::Result<(PathBuf, String)> {
    match destino {
        Destino::Mensaje { proyecto, .. } | Destino::Nueva { proyecto } => {
            let carpeta = almacen::carpeta(raiz, proyecto).join("notas");
            std::fs::create_dir_all(&carpeta)?;
            // Portatil si el proyecto esta en el indice (lo normal); si no,
            // relativa a su carpeta, que `ruta_real` tambien entiende. Se
            // saca de la carpeta, que ya existe: el fichero puede no estar
            // aun (una pagina viva se pinta despues).
            let ruta = vista::portatil_de_ruta(raiz, &carpeta)
                .map(|c| format!("{c}/{fichero}"))
                .unwrap_or_else(|| format!("notas/{fichero}"));
            Ok((carpeta.join(fichero), ruta))
        }
        Destino::Fichero { ruta } => {
            let carpeta = ruta.parent().unwrap_or(Path::new(".")).join("adjuntos");
            std::fs::create_dir_all(&carpeta)?;
            Ok((carpeta.join(fichero), format!("adjuntos/{fichero}")))
        }
    }
}

/// Copia la foto `origen` junto a la nota y da la ruta que se escribe en
/// el Markdown.
pub fn adjuntar(
    raiz: &Path,
    destino: &Destino,
    origen: &Path,
    ahora: i64,
) -> std::io::Result<String> {
    let nombre = nombre_limpio(
        &origen
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
    );
    let (copia, ruta) = sitio(raiz, destino, &format!("{ahora}-{nombre}"))?;
    std::fs::copy(origen, &copia)?;
    Ok(ruta)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    struct Carpeta(PathBuf);
    impl Drop for Carpeta {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn carpeta(nombre: &str) -> Carpeta {
        let d =
            std::env::temp_dir().join(format!("pixpin-notas-adj-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Carpeta(d)
    }

    #[test]
    fn el_nombre_de_la_copia_no_rompe_el_markdown() {
        assert_eq!(nombre_limpio("foto (1) [obra].jpg"), "foto__1___obra_.jpg");
        assert_eq!(nombre_limpio("C:\\x/y.png"), "C__x_y.png");
        assert_eq!(nombre_limpio("()"), "imagen");
        assert_eq!(nombre_limpio(&"a".repeat(200)).chars().count(), 80);
    }

    #[test]
    fn una_foto_de_un_proyecto_se_copia_a_sus_notas_y_se_vuelve_a_encontrar() {
        let c = carpeta("proyecto");
        let origen = c.0.join("planta baja.png");
        std::fs::write(&origen, b"png").unwrap();
        let d = Destino::Nueva {
            proyecto: "p1".into(),
        };
        let ruta = adjuntar(&c.0, &d, &origen, 1234).unwrap();
        // Sin indice no hay chat: relativa a la carpeta del proyecto.
        assert_eq!(ruta, "notas/1234-planta_baja.png");
        let copia = almacen::carpeta(&c.0, "p1")
            .join("notas")
            .join("1234-planta_baja.png");
        assert!(copia.is_file());
        assert_eq!(resolver(&c.0, &d, &ruta), Some(copia));
    }

    #[test]
    fn un_md_suelto_guarda_sus_fotos_al_lado() {
        let c = carpeta("fichero");
        let origen = c.0.join("x.jpg");
        std::fs::write(&origen, b"jpg").unwrap();
        let d = Destino::Fichero {
            ruta: c.0.join("apuntes.md"),
        };
        let ruta = adjuntar(&c.0, &d, &origen, 7).unwrap();
        assert_eq!(ruta, "adjuntos/7-x.jpg");
        assert_eq!(
            resolver(&c.0, &d, &ruta),
            Some(c.0.join("adjuntos").join("7-x.jpg"))
        );
    }

    #[test]
    fn una_ruta_que_se_sale_o_de_otro_aparato_no_se_resuelve() {
        let c = carpeta("fuera");
        let d = Destino::Fichero {
            ruta: c.0.join("apuntes.md"),
        };
        assert_eq!(resolver(&c.0, &d, "../secreto.png"), None);
        assert_eq!(
            resolver(
                &c.0,
                &d,
                "/data/user/0/com.forge.pixpin/files/notas/1-a.png"
            ),
            None
        );
        assert_eq!(resolver(&c.0, &d, "C:\\no\\existe.png"), None);
    }
}
