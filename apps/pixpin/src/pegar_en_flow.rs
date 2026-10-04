//! **Ctrl+V de una imagen en Flow Launcher**: la parte de la app.
//!
//! El gancho (`pixpin_shell::pegar_en_flow`) se traga el Ctrl+V cuando Flow
//! esta delante y el portapapeles tiene una imagen sin texto; aqui, en su
//! hilo de trabajo, se guarda la imagen en el borrador del plugin
//! (`pixpin_lanzador::imagenes`, el mismo codigo que lo lee) y se dice que
//! ficha escribir. Una imagen, un nombre: si ya esta en el borrador no se
//! escribe nada.

use pixpin_lanzador::imagenes::{self, Pegado, Portapapeles};
use std::path::{Path, PathBuf};

/// Lo que se escribe en Flow tras un Ctrl+V tragado: ` [img 02] `, o nada
/// si esa imagen ya esta (o ya no hay imagen).
pub fn ficha_a_escribir(raiz: &Path, pp: &Portapapeles, ahora: i64) -> Option<String> {
    match imagenes::pegar_en_borrador(raiz, pp, None, ahora) {
        Pegado::Nueva(n, ruta) => {
            tracing::info!(ficha = n, ruta = %ruta.display(), "pegar en Flow: imagen guardada");
            Some(format!(" {} ", imagenes::ficha(n)))
        }
        Pegado::Repetida(n) => {
            tracing::info!(ficha = n, "pegar en Flow: esa imagen ya esta en la tarea; no se escribe nada");
            None
        }
        Pegado::Nada => {
            tracing::warn!("pegar en Flow: no se pudo leer la imagen del portapapeles");
            None
        }
    }
}

/// Instala el gancho. `raiz` es la carpeta de datos de PixPin (la que lee
/// el plugin).
pub fn instalar(raiz: PathBuf) -> Option<pixpin_shell::pegar_en_flow::GanchoPegarEnFlow> {
    let al_pegar =
        move || ficha_a_escribir(&raiz, &imagenes::WINDOWS, pixpin_shell::entorno::ahora_utc_ms());
    match pixpin_shell::pegar_en_flow::GanchoPegarEnFlow::instalar(al_pegar) {
        Ok(g) => {
            tracing::info!("gancho de teclado activo: Ctrl+V de una imagen en Flow Launcher la pega como [img NN]");
            Some(g)
        }
        Err(e) => {
            tracing::warn!(?e, "sin gancho de teclado: Ctrl+V de una imagen en Flow no hara nada");
            None
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_lanzador::imagenes::Copiada;

    fn hay() -> Option<u32> {
        Some(41)
    }
    fn foto() -> Option<Copiada> {
        Some(Copiada { extension: "png".into(), bytes: b"\x89PNG foto de la app".to_vec() })
    }
    fn otra() -> Option<Copiada> {
        Some(Copiada { extension: "bmp".into(), bytes: b"BM otra".to_vec() })
    }
    fn si() -> bool {
        true
    }
    const FOTO: Portapapeles = Portapapeles { imagen: hay, leer: foto, pega_la_app: si };
    const OTRA: Portapapeles = Portapapeles { imagen: hay, leer: otra, pega_la_app: si };

    fn raiz(nombre: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("pixpin-pegar-en-flow-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        r
    }

    #[test]
    fn la_app_escribe_la_ficha_y_el_plugin_la_encuentra() {
        let r = raiz("ida-y-vuelta");
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        assert_eq!(ficha_a_escribir(&r, &FOTO, ahora).as_deref(), Some(" [img 01] "));
        assert_eq!(ficha_a_escribir(&r, &OTRA, ahora).as_deref(), Some(" [img 02] "));
        // Lo que escribio la app es lo que lee el plugin: cada ficha, su
        // fichero, en el orden de las fichas.
        let b = imagenes::leer_borrador(&r, ahora).expect("el borrador del plugin");
        let a = imagenes::resolver("t pan [img 02] y [img 01]", Some(&b));
        assert_eq!(a.texto, "t pan [img 02] y [img 01]");
        assert_eq!(a.rutas.len(), 2);
        assert!(a.rutas[0].ends_with(".png") && a.rutas[1].ends_with(".bmp"), "{:?}", a.rutas);
        assert_eq!(std::fs::read(&a.rutas[0]).unwrap(), b"\x89PNG foto de la app");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn caso_negativo_la_misma_imagen_no_se_escribe_dos_veces() {
        let r = raiz("repetida");
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        assert!(ficha_a_escribir(&r, &FOTO, ahora).is_some());
        assert_eq!(ficha_a_escribir(&r, &FOTO, ahora), None, "una imagen, un nombre");
        let b = imagenes::leer_borrador(&r, ahora).unwrap();
        assert_eq!(b.imagenes.len(), 1);
        // Sin imagen, tampoco.
        assert_eq!(ficha_a_escribir(&r, &imagenes::SIN_PORTAPAPELES, ahora), None);
        let _ = std::fs::remove_dir_all(&r);
    }
}
