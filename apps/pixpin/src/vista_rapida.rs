//! **Vista rapida: espacio en el Explorador** (10-oct-2026), la parte de la
//! app. El gancho (`pixpin_shell::espacio_explorador`) se queda con el
//! espacio pulsado en la lista de ficheros del Explorador o del escritorio;
//! aqui, en su hilo, se lee la seleccion y lo que PixPin sabe abrir va por el
//! mismo camino que «Abrir con PixPin» (`Evento::AbrirFicheros`): fotos,
//! videos, audios, PDF, Word, EPUB, PowerPoint, Markdown, planos DWG/DXF y
//! modelos IFC, Revit y de Civil 3D.
//!
//! Como en QuickLook: **otro espacio con lo mismo elegido lo cierra**, y un
//! espacio con otra cosa elegida cierra lo de antes y abre lo nuevo (se
//! puede ir con las flechas y el espacio). Lo que no se sabe abrir (un ZIP,
//! una carpeta) devuelve el espacio al Explorador como si nada.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Cuantos ficheros se abren de una vez como mucho (elegir una carpeta
/// entera y pulsar espacio no debe llenar la pantalla de pines).
const MAXIMO: usize = 12;

/// Lo ultimo abierto con el espacio: que ficheros y sus ventanas.
static ULTIMA: Mutex<Option<(Vec<PathBuf>, Vec<isize>)>> = Mutex::new(None);

/// Si PixPin abre este fichero con la vista rapida (por su extension y, en
/// los de Civil 3D, por lo que tienen dentro).
pub fn se_abre(ruta: &Path) -> bool {
    let ext = ruta.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    use pixpin_shell::asociaciones::{AUDIOS, IMAGENES, VIDEOS};
    IMAGENES.contains(&ext.as_str())
        || VIDEOS.contains(&ext.as_str())
        || AUDIOS.contains(&ext.as_str())
        || crate::notas_md::es_markdown(ruta)
        || crate::lector::se_lee_al_tocar_fichero(ruta)
}

/// Lo de la seleccion que se abre, en su orden y sin pasar del maximo.
pub fn que_abrir(seleccion: Vec<PathBuf>) -> Vec<PathBuf> {
    seleccion.into_iter().filter(|r| r.is_file() && se_abre(r)).take(MAXIMO).collect()
}

/// Lo que pasa al pulsar el espacio: `true` si era para PixPin.
fn al_pulsar() -> bool {
    let rutas = que_abrir(pixpin_shell::explorador::seleccion_del_explorador());
    if rutas.is_empty() {
        return false;
    }
    let Ok(mut ultima) = ULTIMA.lock() else { return false };
    // Lo abierto antes con el espacio se cierra; si era lo mismo, solo eso.
    if let Some((antes, ventanas)) = ultima.take() {
        let vivas: Vec<isize> = ventanas.into_iter().filter(|h| pixpin_shell::espacio_explorador::viva(*h)).collect();
        if !vivas.is_empty() {
            pixpin_shell::espacio_explorador::cerrar(&vivas);
            if antes == rutas {
                tracing::info!(cuantos = rutas.len(), "vista rapida: cerrada con el espacio");
                return true;
            }
        }
    }
    let antes = pixpin_shell::espacio_explorador::ventanas_propias();
    if !pixpin_shell::mensajero::enviar_ficheros(&rutas) {
        return false;
    }
    // Las ventanas nuevas, para poder cerrarlas con el siguiente espacio:
    // en cuanto estan las que tocan (o a los dos segundos).
    let t = Instant::now();
    let mut nuevas = Vec::new();
    while t.elapsed() < Duration::from_secs(2) {
        nuevas = pixpin_shell::espacio_explorador::ventanas_propias().into_iter().filter(|h| !antes.contains(h)).collect();
        if nuevas.len() >= rutas.len() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    tracing::info!(cuantos = rutas.len(), ventanas = nuevas.len(), ms = t.elapsed().as_millis() as u64, "vista rapida: abierto");
    *ultima = Some((rutas, nuevas));
    true
}

/// Pone el gancho del espacio; `activa`, el interruptor de Ajustes.
pub fn instalar(activa: bool) -> Option<pixpin_shell::espacio_explorador::GanchoEspacio> {
    pixpin_shell::espacio_explorador::activar(activa);
    match pixpin_shell::espacio_explorador::GanchoEspacio::instalar(|_| al_pulsar()) {
        Ok(g) => {
            tracing::info!(activa, "vista rapida: espacio en el Explorador listo");
            Some(g)
        }
        Err(e) => {
            tracing::warn!(?e, "vista rapida: no se pudo poner el gancho del espacio");
            None
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_abren_las_fotos_documentos_planos_y_modelos_y_lo_demas_no() {
        for n in ["a.png", "b.JPG", "c.mp4", "d.mp3", "e.pdf", "f.docx", "g.dwg", "h.ifc", "i.rvt", "j.md"] {
            assert!(se_abre(Path::new(n)), "{n}");
        }
        for n in ["a.zip", "b.exe", "c", "d.lnk"] {
            assert!(!se_abre(Path::new(n)), "{n}");
        }
    }

    #[test]
    fn de_la_seleccion_solo_ficheros_que_existen_y_sin_pasar_del_maximo() {
        let dir = std::env::temp_dir().join(format!("pixpin-vista-rapida-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut sel = Vec::new();
        for i in 0..20 {
            let r = dir.join(format!("f{i}.png"));
            let _ = std::fs::write(&r, b"x");
            sel.push(r);
        }
        sel.insert(0, dir.join("no-existe.png"));
        sel.insert(1, dir.clone());
        let a = que_abrir(sel);
        assert_eq!(a.len(), MAXIMO);
        assert!(a[0].ends_with("f0.png"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
