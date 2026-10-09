//! **Los comentarios de la nota en el editor nuevo** (8-oct-2026): los mismos
//! que el de antes, en el mismo fichero hermano (`comentarios`, JSON que
//! viaja al movil). Las cuentas son las de `pixpin_docs::md_comentarios`
//! (el ancla es una cita con su contexto; al cambiar el texto se vuelve a
//! buscar; al guardar se junta con lo que haya llegado al disco). La pagina
//! solo los pinta y pide cambios.

use std::path::{Path, PathBuf};

use pixpin_docs::md_comentarios::{self as mc, Comentarios, Quien};
use serde_json::json;

use super::Destino;

pub struct Comentarista {
    ruta: Option<PathBuf>,
    /// Lo que habia en el disco al leer (la base de la fusion).
    base: Comentarios,
    /// Lo de aqui.
    mio: Comentarios,
    quien: Quien,
    /// El fichero no se entiende: no se escribe encima (seria perderlo).
    roto: bool,
    pendiente: bool,
}

fn utf16(t: &str) -> Vec<u16> {
    t.encode_utf16().collect()
}

impl Comentarista {
    pub fn abrir(raiz: &Path, destino: &Destino, quien: Quien) -> Self {
        let ruta = super::comentarios::ruta(raiz, destino);
        let (c, roto) = match ruta.as_ref().map(|r| pixpin_proyecto::comentarios_de_notas::leer(r)) {
            Some(Some(t)) => match mc::leer(&t) {
                Ok(c) => (c, false),
                Err(e) => {
                    tracing::warn!(%e, "los comentarios de la nota no se entienden: no se tocan");
                    (Comentarios::default(), true)
                }
            },
            _ => (Comentarios::default(), false),
        };
        Comentarista {
            ruta,
            base: c.clone(),
            mio: c,
            quien,
            roto,
            pendiente: false,
        }
    }

    /// La nota cambio de sitio (una nueva, al guardarse la primera vez).
    pub fn mudar(&mut self, raiz: &Path, destino: &Destino) {
        self.ruta = super::comentarios::ruta(raiz, destino);
        if self.pendiente {
            self.guardar();
        }
    }

    /// Los hilos con donde estan ahora en `texto` (unidades UTF-16, las de
    /// la pagina), para pintarlos.
    pub fn json(&mut self, texto: &str) -> serde_json::Value {
        let sitios = self.mio.reanclar(&utf16(texto));
        let hilos: Vec<serde_json::Value> = self
            .mio
            .comentarios
            .iter()
            .zip(sitios)
            .map(|(h, s)| {
                json!({
                    "id": h.id,
                    "desde": s.map(|s| s.0),
                    "hasta": s.map(|s| s.1),
                    "cita": h.ancla.cita,
                    "autor": h.autor,
                    "aparato": h.aparato,
                    "cuando": h.cuando,
                    "editado": h.editado,
                    "texto": h.texto,
                    "resuelto": h.resuelto,
                    "respuestas": h.respuestas.iter().map(|r| json!({
                        "id": r.id, "autor": r.autor, "aparato": r.aparato,
                        "cuando": r.cuando, "editado": r.editado, "texto": r.texto,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        json!({"tipo": "comentarios", "hilos": hilos, "abiertos": self.mio.abiertos(), "yo": self.quien.aparato})
    }

    pub fn nuevo(&mut self, texto: &str, desde: usize, hasta: usize, cuerpo: &str) -> Option<String> {
        let t = utf16(texto);
        let (desde, hasta) = (desde.min(t.len()), hasta.min(t.len()));
        let (desde, hasta) = if desde == hasta {
            mc::palabra_en(&t, desde)?
        } else {
            (desde.min(hasta), desde.max(hasta))
        };
        let ancla = mc::ancla_de(&t, desde, hasta)?;
        let id = self.mio.nuevo(ancla, &self.quien, ahora(), cuerpo)?;
        self.guardar();
        Some(id)
    }

    pub fn responder(&mut self, hilo: &str, cuerpo: &str) {
        if self.mio.responder(hilo, &self.quien, ahora(), cuerpo).is_some() {
            self.guardar();
        }
    }

    pub fn editar(&mut self, id: &str, cuerpo: &str) {
        if self.mio.editar(id, cuerpo, ahora()) {
            self.guardar();
        }
    }

    pub fn borrar(&mut self, id: &str) {
        if self.mio.borrar(id) {
            self.guardar();
        }
    }

    pub fn resolver(&mut self, hilo: &str, si: bool) {
        if self.mio.resolver(hilo, si, &self.quien, ahora()) {
            self.guardar();
        }
    }

    /// Escribe junto con lo que haya llegado al disco mientras tanto.
    fn guardar(&mut self) {
        let Some(ruta) = self.ruta.clone() else {
            // Una nota nueva: se guardan cuando tenga sitio (`mudar`).
            self.pendiente = true;
            return;
        };
        if self.roto {
            return;
        }
        let disco = pixpin_proyecto::comentarios_de_notas::leer(&ruta)
            .map(|t| mc::leer(&t))
            .unwrap_or_else(|| Ok(Comentarios::default()));
        let Ok(disco) = disco else {
            tracing::warn!("el fichero de comentarios se rompio por fuera: no se pisa");
            return;
        };
        let junto = mc::fusionar(&self.base, &self.mio, &disco);
        match pixpin_proyecto::comentarios_de_notas::escribir(&ruta, &mc::escribir(&junto)) {
            Ok(()) => {
                self.base = junto.clone();
                self.mio = junto;
                self.pendiente = false;
            }
            Err(e) => tracing::warn!(?e, "no se pudieron guardar los comentarios"),
        }
    }
}

fn ahora() -> i64 {
    pixpin_shell::entorno::ahora_utc_ms()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_comentario_se_guarda_en_su_fichero_y_vuelve_con_su_sitio() {
        let d = std::env::temp_dir().join(format!("pixpin-com-web-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let md = d.join("nota.md");
        std::fs::write(&md, "Hola mundo bonito").unwrap();
        let destino = Destino::Fichero { ruta: md };
        let quien = Quien { autor: "Max".into(), aparato: "PC1".into() };
        let mut c = Comentarista::abrir(&d, &destino, quien.clone());
        let id = c.nuevo("Hola mundo bonito", 5, 10, "¿seguro?").unwrap();
        // Otra vez desde el disco: esta, y con el texto movido se reencuentra.
        let mut c2 = Comentarista::abrir(&d, &destino, quien);
        let j = c2.json("Ya: Hola mundo bonito");
        assert_eq!(j["hilos"][0]["id"], id);
        assert_eq!(j["hilos"][0]["desde"], 9);
        assert_eq!(j["abiertos"], 1);
        c2.resolver(&id, true);
        assert_eq!(c2.json("Hola mundo bonito")["abiertos"], 0);
        // Caso negativo: un comentario vacio no se crea.
        assert!(c2.nuevo("Hola mundo bonito", 0, 4, "   ").is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
