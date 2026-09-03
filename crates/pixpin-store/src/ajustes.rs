//! Los ajustes de la aplicacion, en TOML.
//!
//! Dos reglas gobiernan el formato, y las dos existen para que el fichero se
//! pueda editar a mano sin miedo:
//!
//! - **Todo campo que falte se rellena con su valor por defecto.** Un usuario
//!   que solo quiere cambiar un atajo escribe dos lineas, no el fichero entero.
//! - **Las claves desconocidas se ignoran.** Un fichero escrito por una version
//!   mas nueva no impide arrancar a una mas vieja.

use std::fs;
use std::path::PathBuf;

use pixpin_shell::Atajo;
use serde::{Deserialize, Serialize};

use crate::rutas::Ubicacion;

#[derive(Debug, thiserror::Error)]
pub enum ErrorAjustes {
    #[error("no se pudo leer {ruta}: {fuente}")]
    Lectura {
        ruta: PathBuf,
        #[source]
        fuente: std::io::Error,
    },
    #[error("no se pudo escribir {ruta}: {fuente}")]
    Escritura {
        ruta: PathBuf,
        #[source]
        fuente: std::io::Error,
    },
    #[error("el fichero de ajustes tiene un error: {0}")]
    Formato(#[from] toml::de::Error),
    #[error("no se pudieron serializar los ajustes: {0}")]
    Serializacion(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PreferenciaIdioma {
    /// Se toma del idioma de Windows.
    #[default]
    Sistema,
    #[serde(rename = "es")]
    Espanol,
    #[serde(rename = "en")]
    Ingles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PreferenciaNivel {
    /// La aplicacion mide el equipo al arrancar y decide.
    #[default]
    Auto,
    Completo,
    Ligero,
}

/// Seccion `[rendimiento]` del fichero de ajustes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Rendimiento {
    /// `auto` deja decidir a los hechos; `completo` y `ligero` fuerzan el
    /// nivel. Forzar `ligero` en una maquina potente es legitimo y util:
    /// es como se prueba la ruta ligera sin tener hardware flojo delante.
    pub nivel: PreferenciaNivel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FormatoColor {
    #[default]
    Hex,
    Rgb,
    Hsl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Atajos {
    /// Capturar region y mostrar la barra de resultado. Sin atajo por
    /// defecto (D81): el usuario no lo quiso; queda en la bandeja y en el
    /// TOML para quien lo quiera.
    pub region: Option<Atajo>,
    /// Capturar region y copiar directo al portapapeles, sin confirmacion.
    pub copiar: Atajo,
    /// Captura larga con scroll.
    pub scroll: Atajo,
    /// Cuentagotas global. Sin atajo por defecto (D81).
    pub cuentagotas: Option<Atajo>,
    /// Recortar y dejar flotando como pin (S2).
    pub pin: Atajo,
    /// Pinear el contenido del portapapeles (S2-B).
    pub portapapeles: Atajo,
    /// Anotar sobre la pantalla con la capa viva (S3-C).
    pub anotar: Atajo,
    /// Anotar sobre una captura estatica de la pantalla (S3-C, D56). Sin
    /// atajo por defecto (D81).
    pub anotar_congelada: Option<Atajo>,
}

impl Default for Atajos {
    fn default() -> Self {
        // `expect` es correcto aqui: si una constante del propio codigo no
        // parsea, es un fallo de programacion y debe verse en el primer test.
        Self {
            region: None,
            copiar: "Ctrl+Alt+C".parse().expect("atajo por defecto valido"),
            scroll: "Ctrl+Alt+S".parse().expect("atajo por defecto valido"),
            cuentagotas: None,
            pin: "Ctrl+Alt+F".parse().expect("atajo por defecto valido"),
            portapapeles: "Ctrl+Alt+V".parse().expect("atajo por defecto valido"),
            anotar: "Ctrl+Alt+A".parse().expect("atajo por defecto valido"),
            anotar_congelada: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ajustes {
    pub idioma: PreferenciaIdioma,
    /// La tabla vieja, de cuando cada atajo era un campo. Se sigue leyendo
    /// para no romper el fichero de nadie, pero lo que manda es `comandos`.
    pub atajos: Atajos,
    /// Nombre de comando -> atajo, la tabla nueva (ver `comandos.rs`). Lo
    /// que no se nombra conserva su atajo por defecto; una cadena vacia deja
    /// el comando sin atajo.
    pub comandos: std::collections::BTreeMap<String, String>,
    /// Si es `None` se usa la carpeta Imagenes del usuario.
    pub carpeta_capturas: Option<PathBuf>,
    pub formato_color: FormatoColor,
    pub rendimiento: Rendimiento,
    pub arranque_con_windows: bool,
    /// Tope de altura de la captura con scroll. Sin el, una pagina infinita
    /// capturaria hasta agotar la memoria.
    pub limite_scroll_px: u32,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            idioma: PreferenciaIdioma::default(),
            atajos: Atajos::default(),
            comandos: std::collections::BTreeMap::new(),
            carpeta_capturas: None,
            formato_color: FormatoColor::default(),
            rendimiento: Rendimiento::default(),
            arranque_con_windows: false,
            limite_scroll_px: 30_000,
        }
    }
}

/// Lee los ajustes. Si el fichero no existe, devuelve los valores por defecto.
pub fn cargar(ubicacion: &Ubicacion) -> Result<Ajustes, ErrorAjustes> {
    let ruta = ubicacion.fichero_ajustes();
    let texto = match fs::read_to_string(&ruta) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Ajustes::default()),
        Err(fuente) => return Err(ErrorAjustes::Lectura { ruta, fuente }),
    };
    Ok(toml::from_str(&texto)?)
}

/// Escribe los ajustes, creando el directorio si hace falta.
pub fn guardar(ubicacion: &Ubicacion, ajustes: &Ajustes) -> Result<(), ErrorAjustes> {
    let ruta = ubicacion.fichero_ajustes();
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre).map_err(|fuente| ErrorAjustes::Escritura {
            ruta: padre.to_path_buf(),
            fuente,
        })?;
    }
    let texto = toml::to_string_pretty(ajustes)?;
    fs::write(&ruta, texto).map_err(|fuente| ErrorAjustes::Escritura { ruta, fuente })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    fn temporal(etiqueta: &str) -> Ubicacion {
        let dir = std::env::temp_dir().join(format!("pixpin-ajustes-{etiqueta}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Ubicacion::Instalado { raiz: dir }
    }

    #[test]
    fn los_valores_por_defecto_son_los_del_diseno() {
        let a = Ajustes::default();
        // Sin atajo por defecto (D81): el usuario los quito; van por gesto
        // (Alt + boton) y por la bandeja.
        assert_eq!(a.atajos.region, None);
        assert_eq!(a.atajos.cuentagotas, None);
        assert_eq!(a.atajos.anotar_congelada, None);
        assert_eq!(a.atajos.copiar.to_string(), "Ctrl+Alt+C");
        assert_eq!(a.atajos.scroll.to_string(), "Ctrl+Alt+S");
        assert_eq!(a.atajos.pin.to_string(), "Ctrl+Alt+F");
        assert_eq!(a.atajos.portapapeles.to_string(), "Ctrl+Alt+V");
        assert_eq!(a.atajos.anotar.to_string(), "Ctrl+Alt+A");
        assert_eq!(a.idioma, PreferenciaIdioma::Sistema);
        assert_eq!(a.formato_color, FormatoColor::Hex);
        assert!(!a.arranque_con_windows);
    }

    #[test]
    fn sobrevive_la_ida_y_vuelta_por_toml() {
        let original = Ajustes {
            idioma: PreferenciaIdioma::Ingles,
            arranque_con_windows: true,
            atajos: Atajos {
                region: Some("Ctrl+Shift+F1".parse().unwrap()),
                ..Default::default()
            },
            ..Default::default()
        };

        let texto = toml::to_string_pretty(&original).unwrap();
        let vuelta: Ajustes = toml::from_str(&texto).unwrap();

        assert_eq!(original, vuelta);
    }

    #[test]
    fn si_no_hay_fichero_se_usan_los_valores_por_defecto() {
        let u = temporal("sin-fichero");
        let a = cargar(&u).unwrap();
        assert_eq!(a, Ajustes::default());
    }

    #[test]
    fn un_fichero_a_medias_completa_con_los_valores_por_defecto() {
        // Es el caso real de un usuario que edita el TOML a mano y solo
        // escribe lo que quiere cambiar. No debe romper nada.
        let u = temporal("parcial");
        fs::write(u.fichero_ajustes(), "arranque_con_windows = true\n").unwrap();

        let a = cargar(&u).unwrap();

        assert!(a.arranque_con_windows);
        assert_eq!(a.atajos.copiar.to_string(), "Ctrl+Alt+C");
    }

    #[test]
    fn las_claves_desconocidas_se_ignoran() {
        // Compatibilidad hacia atras: un fichero escrito por una version mas
        // nueva no debe impedir que arranque una version mas vieja.
        let u = temporal("desconocidas");
        fs::write(
            u.fichero_ajustes(),
            "arranque_con_windows = true\nfuncion_del_futuro = 42\n",
        )
        .unwrap();

        let a = cargar(&u).unwrap();

        assert!(a.arranque_con_windows);
    }

    #[test]
    fn un_atajo_invalido_da_error_con_mensaje_util() {
        let u = temporal("atajo-malo");
        fs::write(u.fichero_ajustes(), "[atajos]\nregion = \"NoEsUnAtajo\"\n").unwrap();

        let e = cargar(&u).unwrap_err();

        assert!(
            e.to_string().contains("NoEsUnAtajo"),
            "el error debe decir que valor concreto esta mal, dijo: {e}"
        );
    }

    #[test]
    fn el_nivel_de_rendimiento_se_lee_y_por_defecto_es_auto() {
        let ajustes: Ajustes = toml::from_str("[rendimiento]\nnivel = \"ligero\"").unwrap();
        assert_eq!(ajustes.rendimiento.nivel, PreferenciaNivel::Ligero);
        // Un fichero sin la seccion conserva el valor por defecto: la regla
        // de "todo campo que falte se rellena" tambien vale para secciones.
        let vacios: Ajustes = toml::from_str("").unwrap();
        assert_eq!(vacios.rendimiento.nivel, PreferenciaNivel::Auto);
    }

    #[test]
    fn un_nivel_desconocido_da_error_en_vez_de_adivinar() {
        // Caso negativo: "turbo" no existe. Adivinar seria peor que fallar,
        // porque el usuario cree haber forzado algo que no esta pasando.
        let resultado = toml::from_str::<Ajustes>("[rendimiento]\nnivel = \"turbo\"");
        assert!(resultado.is_err());
    }

    #[test]
    fn guardar_crea_el_directorio_si_no_existe() {
        let dir = std::env::temp_dir().join("pixpin-ajustes-crear/anidado");
        let _ = fs::remove_dir_all(dir.parent().unwrap());
        let u = Ubicacion::Instalado { raiz: dir.clone() };

        guardar(&u, &Ajustes::default()).unwrap();

        assert!(u.fichero_ajustes().is_file());
        assert_eq!(cargar(&u).unwrap(), Ajustes::default());
    }
}
