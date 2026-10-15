//! **La galeria de capturas que viaja**, del lado del PC (Android v0.107.0).
//!
//! Lo que la sincronizacion necesita saber de las capturas de este equipo
//! (`pixpin_sincro::galeria::CapturasDelAparato`): estan en la carpeta
//! `capturas/` de los datos (`galeria_capturas`), su fecha es la de
//! modificacion del fichero —por ella las agrupa la galeria por dia—, el
//! registro de caducidad es `caducidad_capturas` y lo que se quita en otro
//! aparato va a la papelera de PixPin, como el «Borrar» de la galeria.
//!
//! Las reglas (que se junta, que se trae, que se tira) son las de Android y
//! viven en `pixpin_sincro::galeria`; aqui solo se lee y se escribe.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use pixpin_sincro::galeria::{Caducidad, CapturasDelAparato, Entrada, Local};
use pixpin_sincro::protocolo::Resultado;

use crate::{caducidad_capturas, galeria_capturas};

/// Las capturas de este equipo, sobre la raiz de sus datos.
pub(crate) struct CapturasDelPc {
    raiz: PathBuf,
}

impl CapturasDelPc {
    pub(crate) fn nuevas(raiz: &Path) -> CapturasDelPc {
        CapturasDelPc {
            raiz: raiz.to_path_buf(),
        }
    }

    fn carpeta(&self) -> PathBuf {
        galeria_capturas::carpeta_en(&self.raiz)
    }
}

/// Donde se escribe una captura mientras llega: fuera de lo que lista la
/// galeria (su extension no es de captura), para que no se vea a medias.
fn ruta_llegando(carpeta: &Path, nombre: &str) -> PathBuf {
    carpeta.join(format!(".{nombre}.llegando"))
}

impl CapturasDelAparato for CapturasDelPc {
    fn raiz(&self) -> PathBuf {
        self.raiz.clone()
    }

    fn listar(&self) -> Option<Vec<Local>> {
        let carpeta = self.carpeta();
        // Sin carpeta aun no se capturo nada: «ninguna». Pero si esta y no se
        // puede leer, NO es «ninguna»: dar la lista vacia marcaria como
        // quitadas a mano todas las que habia y se irian de los demas.
        if carpeta.exists() && std::fs::read_dir(&carpeta).is_err() {
            return None;
        }
        Some(
            galeria_capturas::listar(&carpeta)
                .into_iter()
                .map(|e| {
                    let nombre = caducidad_capturas::nombre(&e.ruta);
                    Local {
                        cuando: caducidad_capturas::ms_de(e.cuando),
                        bytes: e.bytes as i64,
                        mime: pixpin_sincro::al_lienzo::mime_de(&nombre).to_string(),
                        nombre,
                    }
                })
                .collect(),
        )
    }

    fn abrir(&self, nombre: &str) -> Option<PathBuf> {
        Some(self.carpeta().join(nombre)).filter(|r| r.is_file())
    }

    fn guardar(
        &self,
        e: &Entrada,
        escribir: &mut dyn FnMut(&mut dyn Write) -> Resultado<bool>,
    ) -> Resultado<bool> {
        let carpeta = self.carpeta();
        std::fs::create_dir_all(&carpeta)?;
        let tmp = ruta_llegando(&carpeta, &e.nombre);
        let hecho = (|| -> Resultado<bool> {
            let mut f = std::fs::File::create(&tmp)?;
            if !escribir(&mut f)? {
                return Ok(false);
            }
            f.flush()?;
            // **Con la hora de cuando se hizo**, no la de ahora: la galeria
            // agrupa por dia con la fecha del fichero, y una captura de
            // hace tres dias no puede salir en «hoy».
            if e.cuando > 0 {
                f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_millis(e.cuando as u64))?;
            }
            drop(f);
            std::fs::rename(&tmp, carpeta.join(&e.nombre))?;
            Ok(true)
        })();
        if !matches!(hecho, Ok(true)) {
            let _ = std::fs::remove_file(&tmp);
        }
        hecho
    }

    fn tirar(&self, nombres: &[String]) {
        for n in nombres {
            let ruta = self.carpeta().join(n);
            match galeria_capturas::borrar(&self.raiz, &ruta) {
                Ok(_) => tracing::info!(ruta = %ruta.display(), "captura quitada en otro aparato: a la papelera"),
                Err(e) => tracing::warn!(?e, ruta = %ruta.display(), "no se pudo quitar la captura"),
            }
        }
    }

    fn dias(&self) -> i64 {
        caducidad_capturas::dias()
    }

    fn caducidad(&self, ahora: i64) -> Caducidad {
        caducidad_capturas::leer(&self.raiz, ahora)
    }

    fn cambiar_caducidad(
        &self,
        ahora: i64,
        f: &mut dyn FnMut(&mut Caducidad),
    ) -> std::io::Result<()> {
        caducidad_capturas::cambiar(&self.raiz, ahora, |r| f(r)).map(|_| ())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(etiqueta: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-galeria-pc-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn una_captura_que_llega_se_guarda_con_la_fecha_de_cuando_se_hizo() {
        let raiz = temporal("llega");
        let c = CapturasDelPc::nuevas(&raiz);
        assert_eq!(c.listar(), Some(Vec::new()), "sin carpeta: ninguna");
        let mut e = Entrada::nueva("PixPin_20261005_101500.png", 1_791_000_000_000);
        e.mime = "image/png".into();
        let entera = c
            .guardar(&e, &mut |w| {
                w.write_all(b"\x89PNG datos").unwrap();
                Ok(true)
            })
            .unwrap();
        assert!(entera);
        let l = c.listar().unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].nombre, "PixPin_20261005_101500.png");
        // La galeria la pone en su dia, no en el de hoy.
        assert_eq!(l[0].cuando, 1_791_000_000_000);
        assert_eq!(l[0].mime, "image/png");
        assert!(c.abrir("PixPin_20261005_101500.png").is_some());
        let lista = galeria_capturas::listar(&galeria_capturas::carpeta_en(&raiz));
        assert_eq!(caducidad_capturas::ms_de(lista[0].cuando), 1_791_000_000_000);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn caso_negativo_una_que_no_llego_entera_no_deja_nada() {
        let raiz = temporal("rota");
        let c = CapturasDelPc::nuevas(&raiz);
        let e = Entrada::nueva("a.png", 5);
        let entera = c
            .guardar(&e, &mut |w| {
                w.write_all(b"medio").unwrap();
                Ok(false)
            })
            .unwrap();
        assert!(!entera);
        assert!(c.listar().unwrap().is_empty());
        // Ni el fichero a medias.
        let carpeta = galeria_capturas::carpeta_en(&raiz);
        assert_eq!(std::fs::read_dir(&carpeta).unwrap().count(), 0);
        // Y si falla la red a medias, igual.
        assert!(
            c.guardar(&e, &mut |_| Err(pixpin_sincro::protocolo::ErrorSincro::Protocolo(
                "cortado".into()
            )))
            .is_err()
        );
        assert_eq!(std::fs::read_dir(&carpeta).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn lo_quitado_en_otro_va_a_la_papelera_de_pixpin_y_lo_acordado_al_registro() {
        let raiz = temporal("tirar");
        let c = CapturasDelPc::nuevas(&raiz);
        let carpeta = galeria_capturas::carpeta_en(&raiz);
        std::fs::create_dir_all(&carpeta).unwrap();
        std::fs::write(carpeta.join("a.png"), b"x").unwrap();
        c.tirar(&["a.png".to_string()]);
        assert!(!carpeta.join("a.png").exists());
        assert!(raiz.join("papelera").join("capturas").join("a.png").exists());
        c.cambiar_caducidad(7, &mut |r| {
            r.fijadas.insert("b.png".into(), 99);
        })
        .unwrap();
        assert_eq!(
            caducidad_capturas::leer(&raiz, 7).fijadas.get("b.png"),
            Some(&99)
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
