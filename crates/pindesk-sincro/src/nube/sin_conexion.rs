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
//!
//! **La clave** de lo retenido es el codigo de un adjunto (`anot-<uid>…`),
//! la referencia de un lienzo, o **varias partes** ([clave_con_lienzos]): el
//! PDF de un proyecto escribe sus marcadores en `anot-<unico del proyecto>…`
//! y la tinta de cada hoja en el lienzo de la hoja del proyecto
//! (`pins/draw/<dibujo>.excalidraw.gz`), asi que su clave lleva el codigo y
//! esos lienzos. Dos claves con la misma primera parte son el mismo
//! documento ([misma_clave]).

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

/// Entre las partes de una clave ([clave_con_lienzos]). No cabe en un
/// nombre de fichero de Windows ni en un codigo: ningun lienzo lo lleva.
pub const ENTRE_PARTES: char = '|';

/// **La clave de un documento cuya tinta va en lienzos** (el PDF de un
/// proyecto, `pindesk_proyecto::capas_del_pdf`): `codigo` es el de lo
/// anotado `anot-<codigo>…` (sus marcadores y espacios) y `lienzos`, el
/// dibujo de cada hoja (`pins/draw/<dibujo>.excalidraw.gz` y lo de su lado).
/// Sin repetidos ni vacios; el codigo, siempre delante.
pub fn clave_con_lienzos(codigo: &str, lienzos: &[String]) -> String {
    let mut s = codigo.to_string();
    let mut vistos: Vec<&str> = vec![codigo];
    for l in lienzos {
        let l = l.as_str();
        if l.is_empty() || l.contains(ENTRE_PARTES) || vistos.contains(&l) {
            continue;
        }
        vistos.push(l);
        s.push(ENTRE_PARTES);
        s.push_str(l);
    }
    s
}

/// Las partes de una clave (una sola si es de un adjunto o de un lienzo).
fn partes(clave: &str) -> impl Iterator<Item = &str> {
    clave.split(ENTRE_PARTES).filter(|p| !p.is_empty())
}

/// La primera parte: la que dice de que documento es.
fn primera(clave: &str) -> &str {
    clave.split(ENTRE_PARTES).next().unwrap_or(clave)
}

/// **Si dos claves son del mismo documento**: la misma primera parte. Los
/// lienzos de un PDF de un proyecto pueden cambiar mientras esta sin
/// conexion (el movil le pone dibujo a una hoja): sigue siendo el.
pub fn misma_clave(a: &str, b: &str) -> bool {
    primera(a) == primera(b)
}

/// Si el fichero `rel` es de lo anotado con `clave`: `anot-<clave>…`, o el
/// lienzo `pins/draw/<clave>.excalidraw.gz` y lo de su lado; con varias
/// partes ([clave_con_lienzos]), de cualquiera de ellas.
pub fn pertenece(clave: &str, rel: &str) -> bool {
    let Some(nombre) = rel
        .strip_prefix(anotado::CARPETA)
        .and_then(|x| x.strip_prefix('/'))
        .filter(|n| !n.contains('/'))
    else {
        return false;
    };
    partes(clave).any(|parte| match anotado::uid_del_nombre(nombre) {
        Some(u) => u == parte,
        None => nombre.starts_with(&format!("{parte}.")),
    })
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
            .join(limpio(primera(clave)))
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
            .any(|r| r.chat == chat && misma_clave(&r.clave, clave))
    }

    /// Lo anotado de `clave` que hay ahora en disco.
    fn ficheros<D: Disco + ?Sized>(d: &D, chat: &str, clave: &str) -> Vec<String> {
        let mut v: Vec<String> = d
            .anotado(chat)
            .into_iter()
            .filter(|r| pertenece(clave, r))
            .collect();
        for parte in partes(clave) {
            for t in [".excalidraw.gz", ".marcas", ".vistas.json"] {
                let r = format!("{}/{parte}{t}", anotado::CARPETA);
                if !v.contains(&r) && d.ruta(chat, &r).is_file() {
                    v.push(r);
                }
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
        if l.iter()
            .any(|r| r.chat == chat && misma_clave(&r.clave, clave))
        {
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
            .filter(|r| !(r.chat == chat && misma_clave(&r.clave, clave)))
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
            .find(|r| r.chat == chat && misma_clave(&r.clave, clave))
        else {
            return Ok(0);
        };
        // Lo de la clave con que se retuvo (y lo de la de ahora, si el
        // documento gano un lienzo mientras tanto).
        let lienzos: Vec<String> = partes(&r.clave)
            .skip(1)
            .chain(partes(clave).skip(1))
            .map(str::to_string)
            .collect();
        let clave = &clave_con_lienzos(primera(&r.clave), &lienzos);
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

    /// **La clave de cada visor** con «En línea / Sin conexión»: el codigo
    /// del adjunto retiene todo lo que ese visor escribe de el; la
    /// referencia de un lienzo, el lienzo y lo de su lado.
    #[test]
    fn la_clave_de_cada_visor_retiene_lo_que_ese_visor_escribe() {
        let uid = "ABCDEFGHJK";
        let del_adjunto = [
            // PDF: una capa por pagina; Word y libros: la capa y la maqueta.
            "pins/draw/anot-ABCDEFGHJK-p3.excalidraw.gz",
            "pins/draw/anot-ABCDEFGHJK.excalidraw.gz",
            "pins/draw/anot-ABCDEFGHJK.maqueta",
            "pins/draw/anot-ABCDEFGHJK-texto.excalidraw.gz",
            // Hoja de calculo: una capa por hoja.
            "pins/draw/anot-ABCDEFGHJK-h0.excalidraw.gz",
            // Plano y modelo: los marcadores, los marcos y lo de cada marcador.
            "pins/draw/anot-ABCDEFGHJK.vistas.json",
            "pins/draw/anot-ABCDEFGHJK.marcos.json",
            "pins/draw/anot-ABCDEFGHJK-mm18f2a.excalidraw.gz",
            "pins/draw/anot-ABCDEFGHJK-mm18f2a.hoja",
            // Pagina web y nota (visor de HTML, lector de notas): las marcas
            // de texto, la tinta y las marcas del riel, los comentarios.
            "pins/draw/anot-ABCDEFGHJK.marcas-texto.json",
            "pins/draw/anot-ABCDEFGHJK.marcas",
            "pins/draw/anot-ABCDEFGHJK.comentarios.json",
            // Las pegatinas de comentario (3-nov): lo adjuntado y sus lienzos.
            "pins/draw/anot-ABCDEFGHJK.a9f1c2.adjunto",
            "pins/draw/anot-ABCDEFGHJK.cpc1-18f2a-0.excalidraw.gz",
        ];
        for rel in del_adjunto {
            assert!(pertenece(uid, rel), "{rel}");
            // Caso negativo: otro documento del mismo chat no se retiene.
            assert!(!pertenece("ZZZZZZZZZZ", rel), "{rel}");
        }
        // El lienzo de una hoja DIBUJO o PAGINA (o de una foto), por su
        // referencia; el de otra hoja no.
        assert!(pertenece("foto-171", "pins/draw/foto-171.excalidraw.gz"));
        assert!(pertenece("foto-171", "pins/draw/foto-171.marcas"));
        // Y sus pegatinas, con lo adjuntado y sus lienzos.
        for rel in [
            "pins/draw/foto-171.comentarios.json",
            "pins/draw/foto-171.a9f1c2.adjunto",
            "pins/draw/foto-171.cpc1-0.excalidraw.gz",
        ] {
            assert!(pertenece("foto-171", rel), "{rel}");
            assert!(!pertenece("foto-17", rel), "{rel}");
        }
        assert!(!pertenece("foto-171", "pins/draw/foto-1710.excalidraw.gz"));
        // Caso negativo: la referencia de un lienzo no se lleva los
        // `anot-…` de ningun adjunto.
        assert!(!pertenece("anot", "pins/draw/anot-ABCDEFGHJK.marcas"));
    }

    /// **El PDF de un proyecto** (`lector_pdf` con `capas_del_pdf`): sus
    /// marcadores van en `anot-<unico del proyecto>…` y la tinta de cada
    /// hoja en el lienzo de la hoja (`pdf-<huella>-p<i>` si la puso el PC,
    /// o el que le puso el movil). La clave lleva el codigo y esos lienzos,
    /// y nada mas.
    #[test]
    fn la_clave_del_pdf_de_un_proyecto_retiene_sus_hojas_y_nada_mas() {
        let lienzos = [
            "pdf-1a2b3c-p0".to_string(),
            "pdf-1a2b3c-p1".to_string(),
            "d1730000000".to_string(),
            // Repetidos y vacios no cuentan.
            "pdf-1a2b3c-p0".to_string(),
            String::new(),
        ];
        let clave = clave_con_lienzos("PRYCTABCDE", &lienzos);
        assert_eq!(clave, "PRYCTABCDE|pdf-1a2b3c-p0|pdf-1a2b3c-p1|d1730000000");
        for rel in [
            "pins/draw/anot-PRYCTABCDE.marcas",
            "pins/draw/anot-PRYCTABCDE.espacios",
            "pins/draw/anot-PRYCTABCDE.vistas.json",
            "pins/draw/pdf-1a2b3c-p0.excalidraw.gz",
            "pins/draw/pdf-1a2b3c-p1.excalidraw.gz",
            "pins/draw/pdf-1a2b3c-p1.marcas",
            "pins/draw/d1730000000.excalidraw.gz",
        ] {
            assert!(pertenece(&clave, rel), "{rel}");
        }
        for rel in [
            // Otra pagina con un numero que empieza igual.
            "pins/draw/pdf-1a2b3c-p10.excalidraw.gz",
            // El mismo numero de hoja de otro PDF.
            "pins/draw/pdf-ffff00-p0.excalidraw.gz",
            // Otra hoja del proyecto que no es de una pagina del PDF.
            "pins/draw/d17.excalidraw.gz",
            // Lo anotado de otro adjunto del mismo chat.
            "pins/draw/anot-ZZZZZZZZZZ-p0.excalidraw.gz",
            // Ni el PDF ni algo de otra carpeta.
            "guardados/plano.pdf",
            "pins/draw/sub/pdf-1a2b3c-p0.excalidraw.gz",
            "proyectos/proyecto.json",
        ] {
            assert!(!pertenece(&clave, rel), "{rel}");
        }
        // Caso negativo: la clave de un adjunto suelto no se lleva los
        // lienzos de las hojas.
        assert!(!pertenece(
            "PRYCTABCDE",
            "pins/draw/pdf-1a2b3c-p0.excalidraw.gz"
        ));
    }

    #[test]
    fn dos_claves_con_el_mismo_codigo_son_el_mismo_documento() {
        let antes = clave_con_lienzos("PRYCTABCDE", &["pdf-1a-p0".into()]);
        let ahora = clave_con_lienzos("PRYCTABCDE", &["pdf-1a-p0".into(), "d9".into()]);
        assert!(misma_clave(&antes, &ahora));
        assert!(misma_clave("PRYCTABCDE", &ahora));
        assert!(!misma_clave("ZZZZZZZZZZ", &ahora));
        // Un lienzo no es el documento cuyo codigo empieza igual.
        assert!(!misma_clave("PRYCTAB", &ahora));
        // `|` no puede colarse en un lienzo.
        assert_eq!(clave_con_lienzos("A", &["x|y".into()]), "A");
    }

    #[test]
    fn retener_el_pdf_de_un_proyecto_copia_sus_hojas_y_descartar_las_devuelve() {
        let raiz = std::env::temp_dir().join(format!(
            "pindesk-sin-conexion-proyecto-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&raiz);
        let d = crate::disco_android::DiscoAndroid::nuevo(raiz.join("files"), "a");
        let poner = |rel: &str, texto: &str| {
            let f = d.ruta("p1", rel);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, texto).unwrap();
        };
        poner("pins/draw/pdf-1a-p0.excalidraw.gz", "hoja 0");
        poner("pins/draw/anot-PRYCTABCDE.marcas", "m");
        poner("pins/draw/d17.excalidraw.gz", "otra hoja");
        let sc = SinConexion::nuevo(&d.sincro());
        let clave = clave_con_lienzos("PRYCTABCDE", &["pdf-1a-p0".into(), "pdf-1a-p1".into()]);
        sc.retener(&d, "p1", &clave, 1).unwrap();
        assert!(sc.esta("p1", "PRYCTABCDE"));
        assert!(sc.retenido("p1", "pins/draw/pdf-1a-p0.excalidraw.gz"));
        assert!(!sc.retenido("p1", "pins/draw/d17.excalidraw.gz"));
        // Sin conexion: se pinta en la hoja 0 y nace la hoja 1.
        poner("pins/draw/pdf-1a-p0.excalidraw.gz", "hoja 0 sin red");
        poner("pins/draw/pdf-1a-p1.excalidraw.gz", "hoja 1 sin red");
        poner("pins/draw/d17.excalidraw.gz", "otra hoja, en linea");
        assert_eq!(sc.descartar(&d, "p1", &clave).unwrap(), 2);
        let leer = |rel: &str| std::fs::read_to_string(d.ruta("p1", rel)).ok();
        assert_eq!(
            leer("pins/draw/pdf-1a-p0.excalidraw.gz").as_deref(),
            Some("hoja 0")
        );
        assert_eq!(leer("pins/draw/pdf-1a-p1.excalidraw.gz"), None);
        assert_eq!(
            leer("pins/draw/d17.excalidraw.gz").as_deref(),
            Some("otra hoja, en linea")
        );
        assert!(!sc.esta("p1", &clave));
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
