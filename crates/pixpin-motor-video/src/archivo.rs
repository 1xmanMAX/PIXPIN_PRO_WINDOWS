//! **El fichero a medias**: mientras se graba, el MP4 se escribe en
//! `<destino>.parte` y solo al cerrarlo bien se pone en su nombre, como la
//! nota de voz y el video de antes (`pixpin-record::mp4_vivo`). Un MP4 sin su
//! indice (`moov`) no lo abre nadie, y no puede quedar con el nombre bueno
//! para que alguien lo enlace en el chat.
//!
//! Si la aplicacion se cierra de golpe queda el `.parte`; quien graba lo
//! barre la siguiente vez (`grabar_clase::video::barrer_restos`).

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
