//! **El fichero a medias**: mientras se graba, el MP4 se escribe en
//! `<destino>.parte` y solo al cerrarlo bien se pone en su nombre, como la
//! nota de voz y el video de antes (`pixpin-record::mp4_vivo`). Un MP4 sin su
//! indice (`moov`) no lo abre nadie, y no puede quedar con el nombre bueno
//! para que alguien lo enlace en el chat.
//!
//! Si la aplicacion se cierra de golpe queda el `.parte`. Si era
//! fragmentado se puede abrir y se rehace a MP4 normal
//! ([`recuperar_video`]); si no, no lo abre nadie, pero **no se borra** sin
//! que el usuario lo pida (la recuperacion de `grabar_clase` lo ofrece).

use std::path::{Path, PathBuf};

use crate::ErrorMotor;

/// `<destino>.parte`, al lado del destino y con su nombre entero.
pub fn a_medias(destino: &Path) -> PathBuf {
    let mut s = destino.as_os_str().to_os_string();
    s.push(".parte");
    PathBuf::from(s)
}

/// **Pone el MP4 ya cerrado en su nombre bueno**. Si no se deja renombrar
/// (otro volumen, un antivirus que lo tiene abierto), se copia; y si ni
/// eso, se queda en `.parte` **con su contenido**, sin borrarlo: el video
/// vale y es la reunion entera.
pub fn poner_en_su_sitio(parcial: &Path, destino: &Path) -> Result<PathBuf, ErrorMotor> {
    if std::fs::rename(parcial, destino).is_ok() {
        return Ok(destino.to_path_buf());
    }
    if let Err(fuente) = std::fs::copy(parcial, destino) {
        let _ = std::fs::remove_file(destino);
        return Err(ErrorMotor::Renombrar {
            ruta: destino.to_path_buf(),
            fuente,
        });
    }
    let _ = std::fs::remove_file(parcial);
    Ok(destino.to_path_buf())
}

/// Que hacer con el `.parte` cuando el cierre (`Finalize`) falla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrasFalloAlCerrar {
    /// Se pone en su nombre: aunque el cierre fallara, se puede abrir (un
    /// MP4 fragmentado lleva su indice por trozos).
    Conservar,
    /// Se borra: un MP4 normal sin su indice no lo abre nadie, y dejarlo con
    /// el nombre bueno haria creer que hay video.
    Borrar,
}

/// **La politica del cierre fallido**: solo se queda lo que se puede abrir,
/// y solo si tiene algo dentro.
pub fn tras_fallo_al_cerrar(fragmentado: bool, bytes: u64) -> TrasFalloAlCerrar {
    // Una cabecera de MP4 fragmentado sin ningun fragmento ocupa menos de un
    // kilobyte; por debajo no hay video.
    if fragmentado && bytes > 1024 {
        TrasFalloAlCerrar::Conservar
    } else {
        TrasFalloAlCerrar::Borrar
    }
}

/// Como salio [`recuperar_video`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recuperado {
    /// Estaba cerrado: solo se puso en su nombre.
    Cerrado,
    /// Era fragmentado y se rehizo a MP4 normal.
    Rehecho,
    /// Era fragmentado y no se pudo rehacer: va tal cual (se abre, pero
    /// puede saltar mal).
    Fragmentado,
}

/// **Pone en `destino` el video de `parcial`** (lo que dejo una grabacion
/// cortada, o un MP4 fragmentado recien cerrado), lo mejor que se pueda:
///
/// - cerrado (con su `moov`): renombrado;
/// - fragmentado: rehecho a MP4 normal con `rehacer` (sin volver a
///   codificar), y si no se puede, tal cual;
/// - sin indice o vacio: `None`, y **no se toca**.
///
/// Lo rehecho se escribe a un `.parte` propio y solo se pone en su nombre
/// entero; el original se borra solo cuando el destino ya esta.
pub fn recuperar_video(
    parcial: &Path,
    destino: &Path,
    rehacer: impl FnOnce(&Path, &Path) -> Result<u64, ErrorMotor>,
) -> Option<Recuperado> {
    use pixpin_sistema::mp4::{EstadoMp4, mirar};
    match mirar(parcial) {
        EstadoMp4::Cerrado => poner_en_su_sitio(parcial, destino)
            .ok()
            .map(|_| Recuperado::Cerrado),
        EstadoMp4::Fragmentado { .. } => {
            let mut s = destino.as_os_str().to_os_string();
            s.push(".rehecho.parte");
            let rehecho = PathBuf::from(s);
            let hecho = rehacer(parcial, &rehecho);
            if hecho.is_ok()
                && mirar(&rehecho) == EstadoMp4::Cerrado
                && poner_en_su_sitio(&rehecho, destino).is_ok()
            {
                let _ = std::fs::remove_file(parcial);
                return Some(Recuperado::Rehecho);
            }
            tracing::info!(?hecho, "el video fragmentado se queda tal cual");
            let _ = std::fs::remove_file(&rehecho);
            poner_en_su_sitio(parcial, destino)
                .ok()
                .map(|_| Recuperado::Fragmentado)
        }
        EstadoMp4::SinIndice | EstadoMp4::Nada => None,
    }
}

/// Lo que ocupa ya `ruta` (cero si no se puede saber).
pub fn bytes(ruta: &Path) -> u64 {
    std::fs::metadata(ruta).map(|m| m.len()).unwrap_or(0)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pixpin-motor-video-{nombre}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn el_fichero_a_medias_va_al_lado_del_destino() {
        assert_eq!(
            a_medias(Path::new("C:/p/clase.mp4")),
            Path::new("C:/p/clase.mp4.parte")
        );
    }

    #[test]
    fn un_cierre_fallido_solo_conserva_lo_que_se_puede_abrir() {
        assert_eq!(
            tras_fallo_al_cerrar(true, 5_000_000),
            TrasFalloAlCerrar::Conservar
        );
        // Caso negativo: un MP4 normal sin indice, o uno fragmentado vacio.
        assert_eq!(
            tras_fallo_al_cerrar(false, 5_000_000),
            TrasFalloAlCerrar::Borrar
        );
        assert_eq!(tras_fallo_al_cerrar(true, 600), TrasFalloAlCerrar::Borrar);
    }

    #[test]
    fn poner_en_su_sitio_renombra() {
        let dir = carpeta("sitio");
        let destino = dir.join("v.mp4");
        let parcial = a_medias(&destino);
        std::fs::write(&parcial, b"moov").unwrap();
        assert_eq!(poner_en_su_sitio(&parcial, &destino).unwrap(), destino);
        assert!(!parcial.exists());
        assert_eq!(std::fs::read(&destino).unwrap(), b"moov");
        assert_eq!(bytes(&destino), 4);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn caja(tipo: &[u8; 4], cuerpo: usize) -> Vec<u8> {
        let mut c = ((cuerpo + 8) as u32).to_be_bytes().to_vec();
        c.extend_from_slice(tipo);
        c.resize(cuerpo + 8, 0);
        c
    }

    #[test]
    fn recuperar_un_video_cerrado_es_ponerlo_en_su_nombre() {
        let dir = carpeta("recuperar-cerrado");
        let parcial = dir.join("v.mp4.parte");
        let destino = dir.join("v.mp4");
        std::fs::write(&parcial, [caja(b"ftyp", 8), caja(b"moov", 8)].concat()).unwrap();
        let r = recuperar_video(&parcial, &destino, |_, _| panic!("no hay que rehacerlo"));
        assert_eq!(r, Some(Recuperado::Cerrado));
        assert!(destino.is_file() && !parcial.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn recuperar_un_fragmentado_lo_rehace_y_si_no_puede_lo_deja_tal_cual() {
        let dir = carpeta("recuperar-frag");
        let fragmentado = [caja(b"ftyp", 8), caja(b"moov", 8), caja(b"moof", 8)].concat();
        let parcial = dir.join("v.mp4.parte");
        let destino = dir.join("v.mp4");
        std::fs::write(&parcial, &fragmentado).unwrap();
        // Rehecho: lo que deja `rehacer` es un MP4 cerrado.
        let r = recuperar_video(&parcial, &destino, |_, a| {
            std::fs::write(
                a,
                [caja(b"ftyp", 8), caja(b"mdat", 8), caja(b"moov", 8)].concat(),
            )
            .unwrap();
            Ok(1)
        });
        assert_eq!(r, Some(Recuperado::Rehecho));
        assert_eq!(
            pixpin_sistema::mp4::mirar(&destino),
            pixpin_sistema::mp4::EstadoMp4::Cerrado
        );
        assert!(!parcial.exists());

        // Caso negativo: no se pudo rehacer (fuera de Windows, un codec
        // raro): va el fragmentado, que tambien se abre.
        std::fs::remove_file(&destino).unwrap();
        std::fs::write(&parcial, &fragmentado).unwrap();
        let r = recuperar_video(&parcial, &destino, |_, _| Err(ErrorMotor::SoloWindows));
        assert_eq!(r, Some(Recuperado::Fragmentado));
        assert_eq!(std::fs::read(&destino).unwrap(), fragmentado);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn un_video_sin_indice_no_se_toca() {
        let dir = carpeta("recuperar-sin-indice");
        let parcial = dir.join("v.mp4.parte");
        let destino = dir.join("v.mp4");
        let bytes = [caja(b"ftyp", 8), caja(b"mdat", 64)].concat();
        std::fs::write(&parcial, &bytes).unwrap();
        assert_eq!(recuperar_video(&parcial, &destino, |_, _| Ok(1)), None);
        // Ni se borra ni se mueve: es del usuario hasta que lo descarte.
        assert_eq!(std::fs::read(&parcial).unwrap(), bytes);
        assert!(!destino.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn si_no_se_puede_ni_copiar_el_parte_se_queda() {
        let dir = carpeta("sin-sitio");
        let parcial = dir.join("v.mp4.parte");
        std::fs::write(&parcial, b"moov").unwrap();
        // Caso negativo: un destino imposible (dentro de un fichero).
        let destino = parcial.join("v.mp4");
        assert!(matches!(
            poner_en_su_sitio(&parcial, &destino),
            Err(ErrorMotor::Renombrar { .. })
        ));
        // Lo grabado no se pierde.
        assert_eq!(std::fs::read(&parcial).unwrap(), b"moov");
        assert_eq!(bytes(&dir.join("no-esta")), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
