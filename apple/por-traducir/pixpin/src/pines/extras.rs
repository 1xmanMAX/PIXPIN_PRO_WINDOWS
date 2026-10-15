//! Lo que un pin lleva puesto y no es su sitio: la pegatina de la esquina y
//! la pagina por la que se lee una nota (`PinState.emoji` y
//! `widget.notaPagina` del movil).
//!
//! Va en un fichero aparte del indice del almacen (`almacen/pines-extra.json`)
//! y no en `PinGuardado`: el indice es de `pixpin-store` y su posicion es
//! `Copy`, y esto es cosa del gestor de pines. Si el fichero falta o esta
//! roto, los pines salen sin pegatina y seguidos: nada se pierde de lo
//! importante. Los pines no viajan al movil, asi que el formato es solo
//! nuestro; el emoji, eso si, es el mismo texto que guarda el movil.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Lo de un pin.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Extra {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pegatina: Option<String>,
    /// La pagina de la nota (desde 0); sin ella, seguida.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagina: Option<u32>,
}

impl Extra {
    fn vacio(&self) -> bool {
        self.pegatina.is_none() && self.pagina.is_none()
    }
}

/// Todos, por id de pin, y donde se guardan.
#[derive(Debug, Default)]
pub struct Extras {
    ruta: Option<PathBuf>,
    pines: BTreeMap<u64, Extra>,
}

impl Extras {
    /// Lee `<raiz>/almacen/pines-extra.json`. Sin fichero, o roto, vacio.
    pub fn abrir(raiz: &Path) -> Extras {
        let ruta = raiz.join("almacen").join("pines-extra.json");
        let pines = std::fs::read_to_string(&ruta)
            .ok()
            .and_then(|t| match serde_json::from_str(&t) {
                Ok(p) => Some(p),
                Err(e) => {
                    tracing::warn!(?e, "pines-extra.json ilegible; los pines salen sin extras");
                    None
                }
            })
            .unwrap_or_default();
        Extras {
            ruta: Some(ruta),
            pines,
        }
    }

    pub fn de(&self, id: u64) -> Extra {
        self.pines.get(&id).cloned().unwrap_or_default()
    }

    /// Cambia lo de un pin y lo guarda.
    pub fn cambiar(&mut self, id: u64, f: impl FnOnce(&mut Extra)) {
        let mut e = self.de(id);
        f(&mut e);
        if e.vacio() {
            self.pines.remove(&id);
        } else {
            self.pines.insert(id, e);
        }
        self.guardar();
    }

    /// Olvida un pin eliminado.
    pub fn quitar(&mut self, id: u64) {
        if self.pines.remove(&id).is_some() {
            self.guardar();
        }
    }

    fn guardar(&self) {
        let Some(ruta) = &self.ruta else {
            return;
        };
        let resultado = serde_json::to_string_pretty(&self.pines)
            .map_err(anyhow::Error::from)
            .and_then(|t| {
                let temporal = ruta.with_extension("tmp");
                std::fs::write(&temporal, t)?;
                std::fs::rename(&temporal, ruta)?;
                Ok(())
            });
        if let Err(e) = resultado {
            tracing::warn!(?e, "no se pudo guardar pines-extra.json");
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn raiz(etiqueta: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-extras-{etiqueta}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("almacen")).unwrap();
        d
    }

    #[test]
    fn la_pegatina_y_la_pagina_sobreviven_a_reiniciar() {
        let r = raiz("vuelta");
        let mut e = Extras::abrir(&r);
        e.cambiar(7, |x| x.pegatina = Some("🔥".into()));
        e.cambiar(9, |x| x.pagina = Some(3));
        let otra = Extras::abrir(&r);
        assert_eq!(otra.de(7).pegatina.as_deref(), Some("🔥"));
        assert_eq!(otra.de(9).pagina, Some(3));
        // Caso negativo: un pin sin nada no tiene nada.
        assert_eq!(otra.de(8), Extra::default());
    }

    #[test]
    fn quitar_todo_deja_el_fichero_sin_el_pin() {
        let r = raiz("quitar");
        let mut e = Extras::abrir(&r);
        e.cambiar(1, |x| x.pegatina = Some("⭐".into()));
        e.cambiar(1, |x| x.pegatina = None);
        e.cambiar(2, |x| x.pagina = Some(0));
        e.quitar(2);
        let texto = std::fs::read_to_string(r.join("almacen/pines-extra.json")).unwrap();
        assert_eq!(texto.trim(), "{}");
    }

    #[test]
    fn un_fichero_roto_no_impide_abrir() {
        let r = raiz("roto");
        std::fs::write(r.join("almacen/pines-extra.json"), "{esto no es json").unwrap();
        let mut e = Extras::abrir(&r);
        assert_eq!(e.de(1), Extra::default());
        // Y se puede volver a guardar encima.
        e.cambiar(1, |x| x.pagina = Some(1));
        assert_eq!(Extras::abrir(&r).de(1).pagina, Some(1));
    }
}
