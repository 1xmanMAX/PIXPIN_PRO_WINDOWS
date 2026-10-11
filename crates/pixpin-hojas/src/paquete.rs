//! El ZIP del libro: leer partes enteras, leerlas de tiron y sus relaciones.

use std::io::{Cursor, Read};
use std::sync::Arc;

use zip::{CompressionMethod, ZipArchive};

use crate::ErrorHojas;
use crate::xml;

/// Tope de lo que se descomprime de una parte leida entera: una bomba de
/// ZIP no llena la memoria. La hoja grande no pasa por aqui (va de tiron).
const TOPE_PARTE: u64 = 512 * 1024 * 1024;

/// El paquete abierto. Clonarlo es barato: comparte los bytes y el indice.
#[derive(Clone)]
pub struct Paquete {
    zip: ZipArchive<Cursor<Arc<[u8]>>>,
    datos: Arc<[u8]>,
}

/// Una relacion de una parte (`_rels/*.rels`), con el destino ya resuelto a
/// una ruta dentro del ZIP (sin la barra inicial) salvo si es externa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relacion {
    pub id: String,
    /// El ultimo trozo del tipo: `worksheet`, `sharedStrings`, `image`...
    pub tipo: String,
    pub destino: String,
    pub externa: bool,
}

/// Un trozo de los bytes del fichero que se lee como `Read` y es dueno de lo
/// que lee (vive en otro hilo si hace falta).
struct Trozo {
    datos: Arc<[u8]>,
    pos: usize,
    fin: usize,
}

impl Read for Trozo {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = buf.len().min(self.fin.saturating_sub(self.pos));
        buf[..n].copy_from_slice(&self.datos[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// El lado que lee de [`Paquete::lector_en_hilo`].
struct EnHilo {
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
    actual: Vec<u8>,
    pos: usize,
}

impl Read for EnHilo {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while self.pos >= self.actual.len() {
            match self.rx.recv() {
                Ok(v) => {
                    self.actual = v;
                    self.pos = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let n = buf.len().min(self.actual.len() - self.pos);
        buf[..n].copy_from_slice(&self.actual[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

impl Paquete {
    pub fn nuevo(datos: Arc<[u8]>) -> Result<Self, ErrorHojas> {
        let zip = ZipArchive::new(Cursor::new(datos.clone())).map_err(|_| ErrorHojas::NoEsZip)?;
        Ok(Self { zip, datos })
    }

    fn indice(&self, ruta: &str) -> Option<usize> {
        let ruta = ruta.trim_start_matches('/');
        if let Some(i) = self.zip.index_for_name(ruta) {
            return Some(i);
        }
        // Hay generadores que cambian mayusculas o usan barras invertidas.
        let buscada = ruta.replace('\\', "/").to_ascii_lowercase();
        let nombre = self.zip.file_names().find(|n| n.replace('\\', "/").to_ascii_lowercase() == buscada)?;
        self.zip.index_for_name(nombre)
    }

    pub fn existe(&self, ruta: &str) -> bool {
        self.indice(ruta).is_some()
    }

    /// La parte entera descomprimida, o `None` si no esta o no se puede leer.
    pub fn leer(&self, ruta: &str) -> Option<Vec<u8>> {
        let i = self.indice(ruta)?;
        let mut zip = self.zip.clone();
        let f = zip.by_index(i).ok()?;
        let mut v = Vec::with_capacity(f.size().min(64 * 1024 * 1024) as usize);
        f.take(TOPE_PARTE).read_to_end(&mut v).ok()?;
        Some(v)
    }

    /// La parte para leer de tiron mientras se descomprime: dueno de lo
    /// suyo, `Send`, sin cargar la parte entera.
    pub fn lector(&self, ruta: &str) -> Option<Box<dyn Read + Send>> {
        let i = self.indice(ruta)?;
        let mut zip = self.zip.clone();
        let f = zip.by_index_raw(i).ok()?;
        if f.encrypted() {
            return None;
        }
        let inicio = usize::try_from(f.data_start()).ok()?;
        let fin = inicio.checked_add(usize::try_from(f.compressed_size()).ok()?)?;
        if fin > self.datos.len() {
            return None;
        }
        let metodo = f.compression();
        drop(f);
        let trozo = Trozo { datos: self.datos.clone(), pos: inicio, fin };
        match metodo {
            CompressionMethod::Stored => Some(Box::new(trozo)),
            CompressionMethod::Deflated => Some(Box::new(flate2::read::DeflateDecoder::new(trozo))),
            _ => None,
        }
    }

    /// Cuanto ocupa comprimida una parte.
    pub fn comprimido(&self, ruta: &str) -> Option<u64> {
        let i = self.indice(ruta)?;
        let mut zip = self.zip.clone();
        let f = zip.by_index_raw(i).ok()?;
        Some(f.compressed_size())
    }

    /// Como [`Paquete::lector`], pero descomprimiendo en otro hilo mientras
    /// quien lee va procesando: con una hoja grande, descomprimir y leer el
    /// XML cuestan parecido, y asi se solapan. El hilo acaba solo al llegar
    /// al final o cuando se suelta el lector.
    pub fn lector_en_hilo(&self, ruta: &str) -> Option<Box<dyn Read + Send>> {
        let mut fuente = self.lector(ruta)?;
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(8);
        std::thread::Builder::new()
            .name("pixpin-hojas-inflar".into())
            .spawn(move || {
                loop {
                    let mut v = vec![0u8; 1 << 20];
                    let mut n = 0;
                    while n < v.len() {
                        match fuente.read(&mut v[n..]) {
                            Ok(0) => break,
                            Ok(k) => n += k,
                            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                            Err(_) => break,
                        }
                    }
                    if n == 0 {
                        break;
                    }
                    v.truncate(n);
                    let corto = n < (1 << 20);
                    if tx.send(v).is_err() || corto {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Box::new(EnHilo { rx, actual: Vec::new(), pos: 0 }))
    }

    /// Las relaciones de una parte (`xl/workbook.xml` -> `xl/_rels/workbook.xml.rels`).
    pub fn relaciones(&self, parte: &str) -> Vec<Relacion> {
        let parte = parte.trim_start_matches('/');
        let (dir, nombre) = match parte.rfind('/') {
            Some(i) => (&parte[..i + 1], &parte[i + 1..]),
            None => ("", parte),
        };
        let ruta = format!("{dir}_rels/{nombre}.rels");
        let Some(bytes) = self.leer(&ruta) else {
            return Vec::new();
        };
        let raiz = xml::arbol(&bytes);
        let mut v = Vec::new();
        let mut rels = Vec::new();
        raiz.todos("Relationship", &mut rels);
        for r in rels {
            let externa = r.attr("TargetMode").is_some_and(|m| m.eq_ignore_ascii_case("External"));
            let destino = r.attr("Target").unwrap_or_default();
            v.push(Relacion {
                id: r.attr("Id").unwrap_or_default().to_string(),
                tipo: r.attr("Type").unwrap_or_default().rsplit('/').next().unwrap_or_default().to_string(),
                destino: if externa { destino.to_string() } else { resolver(parte, destino) },
                externa,
            });
        }
        v
    }
}

/// Resuelve `destino` relativo a la carpeta de la parte `base`.
pub fn resolver(base: &str, destino: &str) -> String {
    let destino = destino.replace('\\', "/");
    let mut trozos: Vec<&str> = Vec::new();
    if !destino.starts_with('/') {
        if let Some(i) = base.rfind('/') {
            trozos.extend(base[..i].split('/').filter(|s| !s.is_empty()));
        }
    }
    for t in destino.split('/') {
        match t {
            "" | "." => {}
            ".." => {
                trozos.pop();
            }
            t => trozos.push(t),
        }
    }
    trozos.join("/")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn resuelve_rutas_relativas_y_absolutas() {
        assert_eq!(resolver("xl/workbook.xml", "worksheets/sheet1.xml"), "xl/worksheets/sheet1.xml");
        assert_eq!(resolver("xl/workbook.xml", "/xl/worksheets/sheet1.xml"), "xl/worksheets/sheet1.xml");
        assert_eq!(resolver("xl/drawings/drawing1.xml", "../media/image1.png"), "xl/media/image1.png");
        assert_eq!(resolver("xl/a.xml", "../../../../x"), "x");
    }
}
