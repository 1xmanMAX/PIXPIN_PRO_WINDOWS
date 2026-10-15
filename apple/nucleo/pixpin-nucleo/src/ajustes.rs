//! **Los ajustes que valen en los dos sistemas**, en el mismo
//! `pixpinmax.toml` que Windows (`pixpin_store::ajustes`), en la raiz de la
//! carpeta de datos.
//!
//! Solo cruzan las claves que significan lo mismo en Mac y en Windows: los
//! dias que aguanta una captura (`[capturas] dias_caducidad`) y la
//! sincronizacion sola (`[sincro] automatica`, `presencia`). Lo que es de
//! cada aparato (el atajo de capturar, el tema, abrir al iniciar sesion)
//! vive en Swift (UserDefaults, `SMAppService`): un atajo de Carbon no es un
//! atajo de Windows y no tiene sentido que viaje.
//!
//! Se guarda con `guardar_conservando`: el fichero conserva sus
//! comentarios, su orden y las claves de Windows que el Mac no toca. Si el
//! fichero que hay no se entiende, **no se guarda** (se dice): reescribirlo
//! con los valores de fabrica le borraria al usuario todo lo de Windows.

use pixpin_store::{Ubicacion, ajustes};

use crate::{ErrorNucleo, Nucleo};

/// Lo de `pixpinmax.toml` que se toca desde el Mac.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AjustesMac {
    /// Dias que aguanta una captura antes de irse sola (0 = nunca).
    pub dias_caducidad: u32,
    /// Sincronizar solo con los aparatos del grupo que contesten.
    pub sincro_automatica: bool,
    /// Dejarse encontrar en la red local (escuchar y anunciarse).
    pub sincro_presencia: bool,
}

impl From<&ajustes::Ajustes> for AjustesMac {
    fn from(a: &ajustes::Ajustes) -> Self {
        AjustesMac {
            dias_caducidad: a.capturas.dias_caducidad,
            sincro_automatica: a.sincro.automatica,
            sincro_presencia: a.sincro.presencia,
        }
    }
}

impl From<ajustes::ErrorAjustes> for ErrorNucleo {
    fn from(e: ajustes::ErrorAjustes) -> Self {
        ErrorNucleo::Ajustes {
            detalle: e.to_string(),
        }
    }
}

/// El tope de dias: un ano y pico. Mas no tiene sentido y un numero enorme
/// solo puede ser un error de escritura.
pub const DIAS_MAXIMOS: u32 = 400;

#[uniffi::export]
impl Nucleo {
    /// Los ajustes de `pixpinmax.toml` (los de fabrica si no hay fichero).
    pub fn ajustes(&self) -> Result<AjustesMac, ErrorNucleo> {
        Ok(AjustesMac::from(&ajustes::cargar(&self.ubicacion())?))
    }

    /// Guarda los ajustes del Mac en `pixpinmax.toml` sin tocar lo demas.
    pub fn guardar_ajustes(&self, nuevos: AjustesMac) -> Result<(), ErrorNucleo> {
        if nuevos.dias_caducidad > DIAS_MAXIMOS {
            return Err(ErrorNucleo::Argumento {
                detalle: format!(
                    "dias de caducidad fuera de rango: {}",
                    nuevos.dias_caducidad
                ),
            });
        }
        let u = self.ubicacion();
        // Si el fichero no se entiende, aqui sale el error y no se escribe.
        let mut a = ajustes::cargar(&u)?;
        a.capturas.dias_caducidad = nuevos.dias_caducidad;
        a.sincro.automatica = nuevos.sincro_automatica;
        a.sincro.presencia = nuevos.sincro_presencia;
        ajustes::guardar_conservando(&u, &a)?;
        Ok(())
    }

    /// La ruta de `pixpinmax.toml` (para ensenarla).
    pub fn ruta_ajustes(&self) -> String {
        self.ubicacion().fichero_ajustes().display().to_string()
    }
}

impl Nucleo {
    pub(crate) fn ubicacion(&self) -> Ubicacion {
        Ubicacion::Instalado {
            raiz: self.ruta().to_path_buf(),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(n: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("pixpin-nucleo-ajustes-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn sin_fichero_salen_los_de_fabrica_y_se_guardan() {
        let d = temporal("fabrica");
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        let a = n.ajustes().unwrap();
        assert_eq!(a.dias_caducidad, 7);
        assert!(a.sincro_automatica);
        n.guardar_ajustes(AjustesMac {
            dias_caducidad: 30,
            sincro_automatica: false,
            sincro_presencia: true,
        })
        .unwrap();
        let b = n.ajustes().unwrap();
        assert_eq!(b.dias_caducidad, 30);
        assert!(!b.sincro_automatica);
        assert!(n.ruta_ajustes().ends_with("pixpinmax.toml"));
    }

    #[test]
    fn se_conservan_los_comentarios_y_lo_de_windows() {
        let d = temporal("conservar");
        let toml = "# mis ajustes\narranque_con_windows = false\nclave_rara = 5\n\n[capturas]\n# una semana\ndias_caducidad = 7\n";
        std::fs::write(d.join("pixpinmax.toml"), toml).unwrap();
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        n.guardar_ajustes(AjustesMac {
            dias_caducidad: 14,
            sincro_automatica: true,
            sincro_presencia: false,
        })
        .unwrap();
        let texto = std::fs::read_to_string(d.join("pixpinmax.toml")).unwrap();
        assert!(texto.contains("# mis ajustes"));
        assert!(texto.contains("# una semana"));
        assert!(texto.contains("clave_rara = 5"));
        assert!(texto.contains("arranque_con_windows = false"));
        assert!(texto.contains("dias_caducidad = 14"));
        assert!(!n.ajustes().unwrap().sincro_presencia);
    }

    #[test]
    fn un_fichero_roto_no_se_pisa() {
        let d = temporal("roto");
        std::fs::write(d.join("pixpinmax.toml"), "esto = = no es toml").unwrap();
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        assert!(matches!(n.ajustes(), Err(ErrorNucleo::Ajustes { .. })));
        let e = n.guardar_ajustes(AjustesMac {
            dias_caducidad: 1,
            sincro_automatica: true,
            sincro_presencia: true,
        });
        assert!(matches!(e, Err(ErrorNucleo::Ajustes { .. })));
        assert_eq!(
            std::fs::read_to_string(d.join("pixpinmax.toml")).unwrap(),
            "esto = = no es toml"
        );
        assert!(matches!(
            n.guardar_ajustes(AjustesMac {
                dias_caducidad: 9999,
                sincro_automatica: true,
                sincro_presencia: true
            }),
            Err(ErrorNucleo::Argumento { .. })
        ));
    }
}
