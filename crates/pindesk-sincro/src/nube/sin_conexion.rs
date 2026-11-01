//! **«Sin conexión» en un documento de un chat de la nube** (1-nov-2026).
//!
//! En un visor de un chat compartido por la nube, arriba, «En línea / Sin
//! conexión». Sin conexion, lo que se anota se queda en este aparato; al
//! volver a «En línea» se sube. Si se sale del documento sin conexion, lo
//! anotado sin conexion se descarta (asi no choca con lo que hicieron los
//! demas mientras tanto).
//!
//! Como va, sin que los visores sepan nada de la nube:
//!
//! - **Al pasar a sin conexion** ([SinConexion::retener]) se copia aparte
//!   lo anotado del documento tal como estaba (`anot-<uid>…`, en
//!   `sincro/nube/sin-conexion/`). Desde ahi la vuelta de la nube **no sube**
//!   esos ficheros ([Retencion::retenido]).
//! - **Lo que llega de los demas se sigue viendo**: se escribe en el fichero
//!   de verdad como siempre (el visor lo relee y lo junta debajo de lo suyo,
//!   `lector_pdf::al_dia`) **y tambien en la copia** ([Retencion::copia]):
//!   la copia es siempre «lo de la nube sin lo mio sin conexion».
//! - **Volver a en linea** ([SinConexion::volver]): se tira la copia y la
//!   vuelta siguiente sube lo anotado como delta contra lo que la nube ya
//!   tiene (solo los trazos nuevos).
//! - **Descartar** ([SinConexion::descartar]): la copia vuelve a su sitio y
//!   lo que nacio sin conexion se borra.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::anotado;
use crate::disco::{Disco, escribir_atomico, limpio};

/// Lo que la vuelta de la nube pregunta de lo retenido.
pub trait Retencion {
    /// Si `rel` de `chat` esta sin conexion: no se sube.
    fn retenido(&self, chat: &str, rel: &str) -> bool;
    /// La copia de `rel` (lo de la nube sin lo de aqui), si esta retenido.
    fn copia(&self, chat: &str, rel: &str) -> Option<PathBuf>;
}

/// Nada retenido (las pruebas, o sin visor abierto).
pub struct NadaRetenido;

impl Retencion for NadaRetenido {
    fn retenido(&self, _: &str, _: &str) -> bool {
        false
    }
    fn copia(&self, _: &str, _: &str) -> Option<PathBuf> {
        None
    }
}

/// Un documento sin conexion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Retenido {
    pub chat: String,
    /// El codigo del adjunto (`anot-<uid>…`) o la referencia de un lienzo.
    pub clave: String,
    pub desde: i64,
    /// Lo anotado que habia al pasar a sin conexion (rutas portatiles).
    pub habia: Vec<String>,
}

/// Si el fichero `rel` es de lo anotado con `clave`: `anot-<clave>…`, o el
/// lienzo `pins/draw/<clave>.excalidraw.gz` y lo de su lado.
pub fn pertenece(clave: &str, rel: &str) -> bool {
    let Some(nombre) = rel
        .strip_prefix(anotado::CARPETA)
        .and_then(|x| x.strip_prefix('/'))
        .filter(|n| !n.contains('/'))
    else {
        return false;
    };
    match anotado::uid_del_nombre(nombre) {
        Some(u) => u == clave,
        None => nombre.starts_with(&format!("{clave}.")),
    }
}

/// Los documentos sin conexion de un aparato.
pub struct SinConexion {
    sincro: PathBuf,
}

impl SinConexion {
    pub fn nuevo(sincro: &Path) -> SinConexion {
        SinConexion {
            sincro: sincro.to_path_buf(),
        }
    }

    fn indice(&self) -> PathBuf {
        super::estado::carpeta(&self.sincro).join("sin-conexion.json")
    }

    fn carpeta_de(&self, chat: &str, clave: &str) -> PathBuf {
        super::estado::carpeta(&self.sincro)
            .join("sin-conexion")
            .join(limpio(chat))
            .join(limpio(clave))
    }

    fn copia_de(&self, chat: &str, clave: &str, rel: &str) -> PathBuf {
        self.carpeta_de(chat, clave)
            .join(limpio(rel.rsplit('/').next().unwrap_or(rel)))
    }

    pub fn lista(&self) -> Vec<Retenido> {
        std::fs::read_to_string(self.indice())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn guardar(&self, l: &[Retenido]) -> io::Result<()> {
        escribir_atomico(
            &self.indice(),
            &serde_json::to_vec(l).map_err(io::Error::other)?,
        )
    }

    /// Si el documento `clave` de `chat` esta sin conexion.
    pub fn esta(&self, chat: &str, clave: &str) -> bool {
        self.lista()
            .iter()
            .any(|r| r.chat == chat && r.clave == clave)
    }

    /// Lo anotado de `clave` que hay ahora en disco.
    fn ficheros<D: Disco + ?Sized>(d: &D, chat: &str, clave: &str) -> Vec<String> {
        let mut v: Vec<String> = d
            .anotado(chat)
            .into_iter()
            .filter(|r| pertenece(clave, r))
            .collect();
        for t in [".excalidraw.gz", ".marcas", ".vistas.json"] {
            let r = format!("{}/{clave}{t}", anotado::CARPETA);
            if !v.contains(&r) && d.ruta(chat, &r).is_file() {
                v.push(r);
            }
        }
        v
    }

    /// **Pasa a sin conexion**: copia aparte lo anotado tal como esta.
    /// Si ya lo estaba, nada.
    pub fn retener<D: Disco + ?Sized>(
        &self,
        d: &D,
        chat: &str,
        clave: &str,
        ahora: i64,
    ) -> io::Result<()> {
        let mut l = self.lista();
        if l.iter().any(|r| r.chat == chat && r.clave == clave) {
            return Ok(());
        }
        let habia = Self::ficheros(d, chat, clave);
        let dir = self.carpeta_de(chat, clave);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        for rel in &habia {
            std::fs::copy(d.ruta(chat, rel), self.copia_de(chat, clave, rel))?;
        }
        l.push(Retenido {
            chat: chat.to_string(),
            clave: clave.to_string(),
            desde: ahora,
            habia,
        });
        self.guardar(&l)
    }

    fn soltar(&self, chat: &str, clave: &str) -> io::Result<()> {
        let l: Vec<Retenido> = self
            .lista()
            .into_iter()
            .filter(|r| !(r.chat == chat && r.clave == clave))
            .collect();
        self.guardar(&l)?;
        let _ = std::fs::remove_dir_all(self.carpeta_de(chat, clave));
        Ok(())
    }

    /// **Vuelve a en linea**: lo anotado se queda y la vuelta lo sube.
    pub fn volver(&self, chat: &str, clave: &str) -> io::Result<()> {
        self.soltar(chat, clave)
    }

    /// **Descarta lo anotado sin conexion**: cada fichero vuelve a como lo
    /// tiene la nube (la copia, con lo que llego de los demas) y lo nacido
    /// sin conexion se borra. Dice cuantos ficheros se tocaron.
    pub fn descartar<D: Disco + ?Sized>(
        &self,
        d: &D,
        chat: &str,
        clave: &str,
    ) -> io::Result<usize> {
        let Some(r) = self
            .lista()
            .into_iter()
            .find(|r| r.chat == chat && r.clave == clave)
        else {
            return Ok(0);
        };
        let mut n = 0;
        for rel in &r.habia {
            let copia = self.copia_de(chat, clave, rel);
            if copia.is_file() {
                let destino = d.ruta(chat, rel);
                let bytes = std::fs::read(&copia)?;
                if std::fs::read(&destino).ok().as_deref() != Some(&bytes[..]) {
                    escribir_atomico(&destino, &bytes)?;
                    n += 1;
                }
            }
        }
        // Lo que llego de la nube estando sin conexion tambien esta en su
        // copia; lo que no tiene copia nacio aqui sin conexion.
        for rel in Self::ficheros(d, chat, clave) {
            if r.habia.contains(&rel) {
                continue;
            }
            let copia = self.copia_de(chat, clave, &rel);
            if copia.is_file() {
                escribir_atomico(&d.ruta(chat, &rel), &std::fs::read(&copia)?)?;
            } else {
                let _ = std::fs::remove_file(d.ruta(chat, &rel));
            }
            n += 1;
        }
        self.soltar(chat, clave)?;
        if n > 0 {
            d.avisar(crate::disco::Cambio::Archivos);
        }
        Ok(n)
    }
}

impl Retencion for SinConexion {
    fn retenido(&self, chat: &str, rel: &str) -> bool {
        self.lista()
            .iter()
            .any(|r| r.chat == chat && pertenece(&r.clave, rel))
    }

    fn copia(&self, chat: &str, rel: &str) -> Option<PathBuf> {
        let l = self.lista();
        let r = l
            .iter()
            .find(|r| r.chat == chat && pertenece(&r.clave, rel))?;
        Some(self.copia_de(chat, &r.clave, rel))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn que_es_de_cada_documento() {
        assert!(pertenece(
            "ABCDEFGHJK",
            "pins/draw/anot-ABCDEFGHJK-p0.excalidraw.gz"
        ));
        assert!(pertenece(
            "ABCDEFGHJK",
            "pins/draw/anot-ABCDEFGHJK.marcos.json"
        ));
        assert!(!pertenece(
            "ABCDEFGHJK",
            "pins/draw/anot-ZZZZZZZZZZ-p0.excalidraw.gz"
        ));
        assert!(pertenece("d1", "pins/draw/d1.excalidraw.gz"));
        assert!(pertenece("d1", "pins/draw/d1.marcas"));
        assert!(!pertenece("d1", "pins/draw/d10.excalidraw.gz"));
        assert!(!pertenece("d1", "guardados/d1.pdf"));
    }
}
