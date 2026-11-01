//! **Bajar Vosk para escuchar en vivo** (1-nov-2026, `vosk_vivo`): la
//! biblioteca (`vosk-win64-0.3.45.zip` de las versiones de `vosk-api` en
//! GitHub: `libvosk.dll` y las tres DLL de MinGW que pide) y el modelo
//! pequeno del idioma (de alphacephei.com, los mismos que el movil), a
//! `<datos>/vosk/`, donde ya los buscan `idiomas::buscar_motor` y
//! `idiomas::buscar_modelo`.
//!
//! `idiomas` dice que aqui no se baja nada sin que se pida: sigue siendo
//! asi. Solo se baja **al elegir Vosk en Ajustes › Voz** (como Qwen3-ASR),
//! con lo que pesa dicho antes, y cada zip se comprueba con su sha256 antes
//! de abrirlo. Solo los idiomas cuyo zip se comprobo aqui (castellano e
//! ingles): los demas siguen con el reconocedor de Windows o Whisper.
//!
//! Desempaquetar es puro (se prueba en Linux con un zip hecho en la
//! prueba); bajar es de Windows (WinHTTP, `whisper::bajar`).

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::ErrorVoz;
use crate::idiomas;

/// Un zip que se baja, con lo que pesa y su huella.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paquete {
    pub url: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

/// La biblioteca, para Windows de 64 bits (Apache-2.0).
pub const MOTOR: Paquete = Paquete {
    url: "https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-win64-0.3.45.zip",
    bytes: 14_882_445,
    sha256: "f1dcc9cca460630f81ea8f71794f69c80bed6556d2a4e6237b5785e1d2dff34b",
};

/// Las DLL del zip de la biblioteca que hacen falta (las demas, fuera).
pub const DLL: [&str; 4] = [
    "libvosk.dll",
    "libstdc++-6.dll",
    "libgcc_s_seh-1.dll",
    "libwinpthread-1.dll",
];

/// Los modelos que se bajan solos para escuchar en vivo, por lengua: los de
/// `idiomas::IDIOMAS` cuyo zip se comprobo (1-nov-2026).
pub const MODELOS: [(&str, Paquete); 2] = [
    (
        "es",
        Paquete {
            url: "https://alphacephei.com/vosk/models/vosk-model-small-es-0.42.zip",
            bytes: 39_817_833,
            sha256: "09b239888f633ef2f0b4e09736e3d9936acfd810bc65d53fad45261762c6511f",
        },
    ),
    (
        "en",
        Paquete {
            url: "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip",
            bytes: 41_205_931,
            sha256: "30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498",
        },
    ),
];

/// El zip del modelo de un idioma («es-ES»), si se baja solo.
pub fn modelo_para(idioma: &str) -> Option<Paquete> {
    let l = idiomas::lengua_de(idioma);
    MODELOS.iter().find(|(x, _)| *x == l).map(|(_, p)| *p)
}

/// Lo que pesa bajar lo que falta para ese idioma (0: nada que bajar o no
/// se puede).
pub fn falta_bajar(raiz: &Path, idioma: &str) -> u64 {
    let Some(m) = modelo_para(idioma) else {
        return 0;
    };
    let motor = if idiomas::buscar_motor(raiz, None).is_some() {
        0
    } else {
        MOTOR.bytes
    };
    let modelo = if idiomas::buscar_modelo(raiz, idioma).is_some() {
        0
    } else {
        m.bytes
    };
    motor + modelo
}

/// Si esta todo para escuchar en vivo con Vosk en ese idioma.
pub fn listo(raiz: &Path, idioma: &str) -> bool {
    idiomas::buscar_motor(raiz, None).is_some() && idiomas::buscar_modelo(raiz, idioma).is_some()
}

/// Si hay algo de Vosk bajado (para ofrecer borrarlo).
pub fn algo_bajado(raiz: &Path) -> bool {
    std::fs::read_dir(idiomas::carpeta_de_modelos(raiz))
        .map(|mut d| d.next().is_some())
        .unwrap_or(false)
}

/// Borra lo bajado (la carpeta `vosk` entera).
pub fn borrar(raiz: &Path) -> std::io::Result<()> {
    let c = idiomas::carpeta_de_modelos(raiz);
    match std::fs::remove_dir_all(&c) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

fn fallo(que: &str, e: impl std::fmt::Display) -> ErrorVoz {
    ErrorVoz::Descarga {
        que: que.to_string(),
        detalle: e.to_string(),
    }
}

/// **Saca las DLL** de [`DLL`] del zip de la biblioteca a `destino`, sin
/// carpetas. Las demas (el `.lib`, la cabecera) no.
pub fn desempaquetar_motor(zip: &Path, destino: &Path) -> Result<(), ErrorVoz> {
    let f = std::fs::File::open(zip).map_err(|e| fallo("libvosk", e))?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| fallo("libvosk", e))?;
    std::fs::create_dir_all(destino).map_err(|e| fallo("libvosk", e))?;
    let mut sacadas = 0;
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| fallo("libvosk", e))?;
        let nombre = e
            .enclosed_name()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
        let Some(nombre) = nombre.filter(|n| DLL.iter().any(|d| d.eq_ignore_ascii_case(n))) else {
            continue;
        };
        let mut v = Vec::new();
        e.read_to_end(&mut v).map_err(|e| fallo(&nombre, e))?;
        escribir(&destino.join(&nombre), &v).map_err(|e| fallo(&nombre, e))?;
        sacadas += 1;
    }
    if sacadas < DLL.len() {
        return Err(fallo("libvosk", "al zip le faltan DLL"));
    }
    Ok(())
}

/// **Saca el modelo** del zip (una carpeta `vosk-model-…` con `am/`,
/// `conf/`…) dentro de `destino`. Primero a una carpeta `.parte` y luego se
/// le cambia el nombre: un modelo a medias nunca parece entero. Devuelve la
/// carpeta.
pub fn desempaquetar_modelo(zip: &Path, destino: &Path) -> Result<PathBuf, ErrorVoz> {
    let f = std::fs::File::open(zip).map_err(|e| fallo("modelo", e))?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| fallo("modelo", e))?;
    let mut raiz_del_zip: Option<String> = None;
    let parte = destino.join(".modelo.parte");
    let _ = std::fs::remove_dir_all(&parte);
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| fallo("modelo", e))?;
        // Fuera de la carpeta (`..`, rutas absolutas): no se saca.
        let Some(ruta) = e.enclosed_name() else {
            continue;
        };
        let mut partes = ruta.components();
        let Some(primera) = partes.next() else {
            continue;
        };
        let primera = primera.as_os_str().to_string_lossy().into_owned();
        if !primera.starts_with("vosk-model") {
            continue;
        }
        raiz_del_zip.get_or_insert(primera);
        let dentro: PathBuf = partes.collect();
        if dentro.as_os_str().is_empty() {
            continue;
        }
        let p = parte.join(&dentro);
        if e.is_dir() {
            std::fs::create_dir_all(&p).map_err(|e| fallo("modelo", e))?;
            continue;
        }
        let mut v = Vec::new();
        e.read_to_end(&mut v).map_err(|e| fallo("modelo", e))?;
        escribir(&p, &v).map_err(|e| fallo("modelo", e))?;
    }
    let Some(nombre) = raiz_del_zip.filter(|_| idiomas::parece_modelo(&parte)) else {
        let _ = std::fs::remove_dir_all(&parte);
        return Err(fallo("modelo", "el zip no trae un modelo de Vosk"));
    };
    let final_ = destino.join(nombre);
    let _ = std::fs::remove_dir_all(&final_);
    std::fs::rename(&parte, &final_).map_err(|e| fallo("modelo", e))?;
    Ok(final_)
}

fn escribir(p: &Path, v: &[u8]) -> std::io::Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(p, v)
}

/// Comprueba la huella de un zip bajado.
#[cfg(windows)]
fn comprobar(zip: &Path, p: &Paquete) -> Result<(), ErrorVoz> {
    let sha = crate::qwen3::sha256_de(zip).map_err(|e| fallo(p.url, e))?;
    if sha != p.sha256 {
        let _ = std::fs::remove_file(zip);
        return Err(fallo(p.url, format!("el sha256 no cuadra ({sha})")));
    }
    Ok(())
}

/// **Baja lo que falte** para escuchar en vivo con Vosk en `idioma`: la
/// biblioteca y el modelo. `avance` va de 0 a 1. Tarda (unos 55 MB): en un
/// hilo de fondo.
#[cfg(windows)]
pub fn bajar(
    raiz: &Path,
    idioma: &str,
    avance: &mut dyn FnMut(f32),
    cancelado: &std::sync::atomic::AtomicBool,
) -> Result<(), ErrorVoz> {
    let modelo = modelo_para(idioma).ok_or_else(|| ErrorVoz::IdiomaSinModelo {
        idioma: idioma.to_string(),
    })?;
    let carpeta = idiomas::carpeta_de_modelos(raiz);
    std::fs::create_dir_all(&carpeta).map_err(|e| fallo("vosk", e))?;
    let mut que: Vec<(Paquete, bool)> = Vec::new();
    if idiomas::buscar_motor(raiz, None).is_none() {
        que.push((MOTOR, true));
    }
    if idiomas::buscar_modelo(raiz, idioma).is_none() {
        que.push((modelo, false));
    }
    let total: u64 = que.iter().map(|(p, _)| p.bytes).sum::<u64>().max(1);
    let mut hecho = 0u64;
    for (p, es_motor) in que {
        let zip = carpeta.join(if es_motor {
            ".motor.zip"
        } else {
            ".modelo.zip"
        });
        let antes = hecho;
        crate::whisper::bajar::bajar(
            p.url,
            &zip,
            &mut |leidos, _| {
                avance(((antes + leidos.min(p.bytes)) as f32 / total as f32).min(0.99))
            },
            cancelado,
        )?;
        comprobar(&zip, &p)?;
        let r = if es_motor {
            desempaquetar_motor(&zip, &carpeta)
        } else {
            desempaquetar_modelo(&zip, &carpeta).map(|_| ())
        };
        let _ = std::fs::remove_file(&zip);
        r?;
        hecho += p.bytes;
    }
    avance(1.0);
    tracing::info!(carpeta = %carpeta.display(), "Vosk bajado para escuchar en vivo");
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::io::Write;

    struct Carpeta(PathBuf);
    impl Drop for Carpeta {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn carpeta(nombre: &str) -> Carpeta {
        let p = std::env::temp_dir().join(format!(
            "pixpin-vosk-instalar-{nombre}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Carpeta(p)
    }

    fn zip_con(p: &Path, ficheros: &[(&str, &[u8])]) {
        let f = std::fs::File::create(p).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let o = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (n, d) in ficheros {
            z.start_file(*n, o).unwrap();
            z.write_all(d).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn el_modelo_sale_entero_en_su_carpeta_y_lo_de_fuera_no() {
        let c = carpeta("modelo");
        let zip = c.0.join("m.zip");
        zip_con(
            &zip,
            &[
                ("vosk-model-small-es-0.42/am/final.mdl", b"am"),
                ("vosk-model-small-es-0.42/conf/model.conf", b"--x"),
                ("vosk-model-small-es-0.42/graph/HCLr.fst", b"g"),
                ("../fuera.txt", b"no"),
                ("otra/cosa.txt", b"no"),
            ],
        );
        let destino = c.0.join("vosk");
        let m = desempaquetar_modelo(&zip, &destino).unwrap();
        assert_eq!(m, destino.join("vosk-model-small-es-0.42"));
        assert_eq!(std::fs::read(m.join("conf/model.conf")).unwrap(), b"--x");
        assert!(!c.0.join("fuera.txt").exists());
        assert!(!destino.join("otra").exists());
        assert!(!destino.join(".modelo.parte").exists());
        assert_eq!(
            idiomas::buscar_modelo(&c.0, "es-ES").as_deref(),
            Some(m.as_path())
        );
    }

    #[test]
    fn un_zip_que_no_es_un_modelo_no_deja_nada() {
        let c = carpeta("no-modelo");
        let zip = c.0.join("m.zip");
        zip_con(&zip, &[("vosk-model-x/LEEME", b"hola")]);
        let destino = c.0.join("vosk");
        assert!(desempaquetar_modelo(&zip, &destino).is_err());
        assert!(!destino.join("vosk-model-x").exists());
        assert!(!destino.join(".modelo.parte").exists());
    }

    #[test]
    fn de_la_biblioteca_solo_las_cuatro_dll() {
        let c = carpeta("motor");
        let zip = c.0.join("w.zip");
        let mut f: Vec<(&str, &[u8])> = vec![
            ("vosk-win64-0.3.45/libvosk.lib", b"lib"),
            ("vosk-win64-0.3.45/vosk_api.h", b"h"),
        ];
        let nombres: Vec<String> = DLL
            .iter()
            .map(|d| format!("vosk-win64-0.3.45/{d}"))
            .collect();
        for n in &nombres {
            f.push((n.as_str(), b"dll"));
        }
        zip_con(&zip, &f);
        let destino = c.0.join("vosk");
        desempaquetar_motor(&zip, &destino).unwrap();
        for d in DLL {
            assert!(destino.join(d).is_file(), "{d}");
        }
        assert!(!destino.join("libvosk.lib").exists());
        assert!(idiomas::buscar_motor(&c.0, None).is_some());
        // Caso negativo: un zip sin todas las DLL es un error.
        let malo = c.0.join("malo.zip");
        zip_con(&malo, &[("x/libvosk.dll", b"dll")]);
        assert!(desempaquetar_motor(&malo, &c.0.join("otro")).is_err());
    }

    #[test]
    fn solo_se_baja_solo_lo_comprobado_y_se_dice_lo_que_falta() {
        let c = carpeta("falta");
        assert!(modelo_para("es-MX").is_some());
        assert!(modelo_para("en-US").is_some());
        assert_eq!(modelo_para("pt-BR"), None);
        assert_eq!(falta_bajar(&c.0, "es-ES"), MOTOR.bytes + MODELOS[0].1.bytes);
        assert_eq!(falta_bajar(&c.0, "ja-JP"), 0);
        assert!(!listo(&c.0, "es-ES"));
        assert!(!algo_bajado(&c.0));
        borrar(&c.0).unwrap();
    }
}
