//! **Lo que este aparato sabe que tiene la nube de cada chat**, en disco
//! (`sincro/nube/chats/<chat>.json`): la ultima revision, el resumen de cada
//! mensaje, y de cada fichero su SHA-256 —y, de un lienzo o una tinta, su
//! original, sus deltas y la version de cada trazo—. Con eso la diferencia se
//! saca contra lo que la nube ya tiene, y no se vuelve a subir nada que ya
//! este. Y que chats van por la nube (`sincro/nube/chats.json`).

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::tipos::Delta;
use super::trazos::{Conocido, Version};
use crate::disco::{escribir_atomico, limpio};

/// La carpeta de la nube dentro de `sincro/`.
pub fn carpeta(sincro: &Path) -> PathBuf {
    sincro.join("nube")
}

/// Lo que la nube tiene de un fichero.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnLaNube {
    /// El SHA-256 de lo que se subio (de un lienzo, de su original).
    pub sha256: String,
    pub bytes: u64,
    /// Los deltas encima del original (solo lienzos y tintas, `deltas1`).
    pub deltas: Vec<Delta>,
    /// La version de cada trazo que la nube ya tiene.
    pub versiones: BTreeMap<String, Version>,
    pub imagenes: BTreeSet<String>,
    pub estado: Option<String>,
    /// El resumen de como estaba **aqui** al subirlo o bajarlo
    /// (`Disco::resumen_de_archivo`): si sigue igual, no hay nada que mirar.
    pub local: String,
}

impl EnLaNube {
    /// Lo conocido de los trazos, para [`super::trazos::calcular`].
    pub fn conocido(&self) -> Conocido {
        Conocido {
            versiones: self.versiones.clone(),
            imagenes: self.imagenes.clone(),
            estado: self.estado.clone(),
        }
    }

    pub fn poner_conocido(&mut self, k: Conocido) {
        self.versiones = k.versiones;
        self.imagenes = k.imagenes;
        self.estado = k.estado;
    }

    /// Lo que pesan sus deltas.
    pub fn bytes_de_deltas(&self) -> u64 {
        self.deltas.iter().map(|d| d.bytes).sum()
    }

    /// Si es un lienzo subido como original + deltas.
    pub fn con_trazos(&self) -> bool {
        !self.versiones.is_empty() || !self.deltas.is_empty()
    }
}

/// **Lo sabido de un chat en la nube.**
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EstadoDeChat {
    /// El espacio donde vive (el de un compartido no es el del aparato).
    pub espacio: String,
    /// La ultima revision que se bajo o se dejo (0: nunca).
    pub revision: i64,
    pub nombre: String,
    /// Codigo unico de cada mensaje → su resumen (`kotlin::resumen_de`).
    pub mensajes: BTreeMap<String, String>,
    pub proyecto: Option<String>,
    pub compartido: Option<String>,
    pub archivos: BTreeMap<String, EnLaNube>,
    /// Objetos que la nube dijo que faltan y no se pudieron subir.
    pub faltan: Vec<String>,
    /// Cuando se termino la ultima vuelta bien, y el ultimo error.
    pub al_dia: i64,
    pub error: Option<String>,
}

fn archivo(sincro: &Path, chat: &str) -> PathBuf {
    carpeta(sincro)
        .join("chats")
        .join(format!("{}.json", limpio(chat)))
}

impl EstadoDeChat {
    pub fn leer(sincro: &Path, chat: &str) -> EstadoDeChat {
        std::fs::read_to_string(archivo(sincro, chat))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn guardar(&self, sincro: &Path, chat: &str) -> io::Result<()> {
        let t = serde_json::to_vec(self).map_err(io::Error::other)?;
        escribir_atomico(&archivo(sincro, chat), &t)
    }

    pub fn olvidar(sincro: &Path, chat: &str) {
        let _ = std::fs::remove_file(archivo(sincro, chat));
    }
}

/// **Por donde va un chat compartido**: por la red local (lo de siempre) o
/// por la nube.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modo {
    #[default]
    Local,
    Nube,
}

/// Los chats que van por la nube (`sincro/nube/chats.json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatsEnLaNube {
    pub chats: BTreeSet<String>,
    /// Los que se pasaron a «Local» a mano: aunque esten en la nube, no se
    /// vuelven a poner solos.
    pub locales: BTreeSet<String>,
    /// Por donde iban las novedades.
    pub seq: i64,
}

impl ChatsEnLaNube {
    fn ruta(sincro: &Path) -> PathBuf {
        carpeta(sincro).join("chats.json")
    }

    pub fn leer(sincro: &Path) -> ChatsEnLaNube {
        std::fs::read_to_string(Self::ruta(sincro))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn guardar(&self, sincro: &Path) -> io::Result<()> {
        let t = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        escribir_atomico(&Self::ruta(sincro), &t)
    }

    pub fn modo(&self, chat: &str) -> Modo {
        if self.chats.contains(chat) {
            Modo::Nube
        } else {
            Modo::Local
        }
    }

    /// Cambia el modo de un chat (a mano: lo que diga la persona manda).
    pub fn poner(&mut self, chat: &str, modo: Modo) {
        match modo {
            Modo::Nube => {
                self.chats.insert(chat.to_string());
                self.locales.remove(chat);
            }
            Modo::Local => {
                self.chats.remove(chat);
                self.locales.insert(chat.to_string());
            }
        }
    }

    /// Un compartido que otra persona puso en la nube: entra solo, salvo
    /// que aqui se dijera «Local» a mano. Dice si entro.
    pub fn adoptar(&mut self, chat: &str) -> bool {
        if self.locales.contains(chat) || self.chats.contains(chat) {
            return false;
        }
        self.chats.insert(chat.to_string())
    }
}

/// Cambia la lista de chats de la nube de una vez.
pub fn cambiar_chats(sincro: &Path, f: impl FnOnce(&mut ChatsEnLaNube)) -> io::Result<()> {
    let mut c = ChatsEnLaNube::leer(sincro);
    let antes = c.clone();
    f(&mut c);
    if c != antes {
        c.guardar(sincro)?;
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_estado_y_los_modos_se_guardan_y_vuelven() {
        let dir = std::env::temp_dir().join(format!("pindesk-nube-estado-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut e = EstadoDeChat {
            espacio: "max".into(),
            revision: 3,
            ..EstadoDeChat::default()
        };
        e.archivos.insert(
            "pins/draw/a.excalidraw.gz".into(),
            EnLaNube {
                sha256: "x".into(),
                versiones: [("t1".to_string(), (2, 5))].into(),
                ..EnLaNube::default()
            },
        );
        e.guardar(&dir, "p/1").unwrap();
        assert_eq!(EstadoDeChat::leer(&dir, "p/1"), e);
        assert!(e.archivos.values().next().unwrap().con_trazos());

        let mut c = ChatsEnLaNube::default();
        assert_eq!(c.modo("p1"), Modo::Local);
        assert!(c.adoptar("p1"));
        assert_eq!(c.modo("p1"), Modo::Nube);
        c.poner("p1", Modo::Local);
        assert!(!c.adoptar("p1"), "lo puesto a mano en Local no vuelve solo");
        c.poner("p1", Modo::Nube);
        assert_eq!(c.modo("p1"), Modo::Nube);
        c.guardar(&dir).unwrap();
        assert_eq!(ChatsEnLaNube::leer(&dir), c);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
