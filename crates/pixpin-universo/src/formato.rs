//! `universo.json` (D240): un fichero, escrito a un temporal y renombrado.

use std::fs;
use std::path::{Path, PathBuf};

use pixpin_motor2d::Escena;
use serde::Serialize;

use crate::universo::Universo;

pub const FICHERO: &str = "universo.json";

pub fn ruta(raiz: &Path) -> PathBuf {
    raiz.join("universo").join(FICHERO)
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorUniverso {
    #[error("E/S en {1}: {0}")]
    Io(std::io::Error, PathBuf),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug)]
pub struct Cargado {
    pub universo: Universo,
    /// Si el fichero no se entendia, adonde se aparto.
    pub roto: Option<PathBuf>,
}

pub fn cargar(ruta: &Path, ahora_ms: i64) -> Result<Cargado, ErrorUniverso> {
    let texto = match fs::read_to_string(ruta) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Cargado {
                universo: Universo::nuevo(),
                roto: None,
            });
        }
        Err(e) => return Err(ErrorUniverso::Io(e, ruta.to_path_buf())),
    };
    match serde_json::from_str::<Universo>(&texto) {
        Ok(mut u) => {
            u.limpiar_conexiones();
            Ok(Cargado {
                universo: u,
                roto: None,
            })
        }
        Err(_) => {
            // Nunca se pisa en silencio: se aparta y se empieza de cero.
            let roto = ruta.with_file_name(format!("{FICHERO}.roto-{ahora_ms}"));
            fs::rename(ruta, &roto).map_err(|e| ErrorUniverso::Io(e, roto.clone()))?;
            Ok(Cargado {
                universo: Universo::nuevo(),
                roto: Some(roto),
            })
        }
    }
}

/// Lo que se escribe. Va aparte de `Universo` porque la escena vive en el
/// editor mientras el universo esta abierto y hay que compactar una copia.
#[derive(Serialize)]
struct Guardable<'a> {
    version: u32,
    siguiente_id: u64,
    astros: &'a [crate::astro::Astro],
    conexiones: &'a [crate::astro::Conexion],
    anotaciones: Escena,
    encuadre: crate::universo::Encuadre,
    #[serde(flatten)]
    resto: &'a serde_json::Map<String, serde_json::Value>,
}

pub fn guardar(ruta: &Path, u: &Universo, anotaciones: &Escena) -> Result<(), ErrorUniverso> {
    let mut escena = anotaciones.clone();
    escena.compactar();
    let g = Guardable {
        version: u.version,
        siguiente_id: u.siguiente_id,
        astros: &u.astros,
        conexiones: &u.conexiones,
        anotaciones: escena,
        encuadre: u.encuadre,
        resto: &u.resto,
    };
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre).map_err(|e| ErrorUniverso::Io(e, padre.to_path_buf()))?;
    }
    // Sin sangrado: 5.000 lunas son unos 750 KB en vez de 1,5 MB.
    let texto = serde_json::to_string(&g)?;
    let temporal = ruta.with_extension("json.tmp");
    fs::write(&temporal, texto).map_err(|e| ErrorUniverso::Io(e, temporal.clone()))?;
    fs::rename(&temporal, ruta).map_err(|e| ErrorUniverso::Io(e, ruta.to_path_buf()))?;
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::astro::{Astro, Conexion, IdAstro};

    // `Elemento` no deriva `Default`: se escribe entero.
    fn rect_de_prueba() -> pixpin_motor2d::Elemento {
        use pixpin_motor2d::{ColorRgba, Elemento, EstiloTrazo, Figura};
        Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x: 0.0,
            y: 0.0,
            ancho: 5.0,
            alto: 5.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    fn tmp(nombre: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("pixpin-universo-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join(FICHERO)
    }

    #[test]
    fn un_fichero_que_no_existe_es_un_universo_vacio_y_no_se_crea() {
        let r = tmp("nada");
        let c = cargar(&r, 0).unwrap();
        assert!(c.universo.astros.is_empty());
        assert!(c.roto.is_none());
        assert!(!r.exists());
    }

    #[test]
    fn ida_y_vuelta_con_astros_conexiones_anotaciones_y_lo_desconocido() {
        let r = tmp("ida");
        let mut u = Universo::nuevo();
        u.astros.push(Astro::planeta(IdAstro(1), 1.0, 2.0, 250.0));
        u.astros.push(Astro::planeta(IdAstro(2), 900.0, 2.0, 250.0));
        u.conexiones
            .push(Conexion::nueva(3, IdAstro(1), IdAstro(2)));
        u.resto.insert("futuro".into(), serde_json::json!(42));
        let mut e = Escena::nueva();
        e.anadir(rect_de_prueba());
        guardar(&r, &u, &e).unwrap();
        let c = cargar(&r, 0).unwrap();
        assert_eq!(c.universo.astros, u.astros);
        assert_eq!(c.universo.conexiones, u.conexiones);
        assert_eq!(c.universo.anotaciones.elementos.len(), 1);
        assert_eq!(c.universo.resto.get("futuro"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn un_fichero_corrupto_se_aparta_con_fecha_y_no_se_pisa() {
        let r = tmp("roto");
        std::fs::write(&r, "{esto no es json").unwrap();
        let c = cargar(&r, 1234).unwrap();
        let roto = c.roto.unwrap();
        assert!(roto.ends_with("universo.json.roto-1234"));
        assert_eq!(std::fs::read_to_string(&roto).unwrap(), "{esto no es json");
        assert!(!r.exists());
    }

    #[test]
    fn al_cargar_se_quitan_las_lineas_a_ninguna_parte() {
        let r = tmp("lineas");
        let mut u = Universo::nuevo();
        u.astros.push(Astro::planeta(IdAstro(1), 0.0, 0.0, 250.0));
        u.conexiones
            .push(Conexion::nueva(3, IdAstro(1), IdAstro(77)));
        guardar(&r, &u, &Escena::nueva()).unwrap();
        assert!(cargar(&r, 0).unwrap().universo.conexiones.is_empty());
    }
}
