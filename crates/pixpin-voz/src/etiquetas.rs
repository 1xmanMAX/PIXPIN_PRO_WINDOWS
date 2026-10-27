//! **Las etiquetas de una cancion**: titulo, artista, album y la portada que
//! trae dentro (27-oct-2026, «Musica con su letra»).
//!
//! Es lo que la tarjeta de musica del chat ensena en vez de la onda, como
//! `TarjetaDeMusica.kt` del movil (que lo saca de `MediaMetadataRetriever`).
//! Aqui se leen las cabeceras a mano, sin Media Foundation: son tres
//! formatos sencillos, se prueban en Linux y no hace falta arrancar ningun
//! motor para pintar una burbuja. Vive junto a `letra_de_cancion` y no en
//! `pixpin-audio`, que solo compila en Windows.
//!
//! - **ID3v2** (`.mp3`, y cualquier fichero que empiece por `ID3`): v2.2,
//!   v2.3 y v2.4; `TIT2`/`TPE1`/`TALB` y la imagen `APIC` (`PIC` en v2.2).
//! - **MPEG-4** (`.m4a`, `.mp4`, `.aac` en contenedor): `moov/udta/meta/ilst`
//!   con `©nam`, `©ART`, `©alb` y `covr`.
//! - **FLAC**: el bloque `VORBIS_COMMENT` (`TITLE`, `ARTIST`, `ALBUM`) y el
//!   bloque `PICTURE`.
//!
//! Lo que no se reconoce da [`Etiquetas::default`]: la tarjeta cae al
//! nombre del fichero y a la nota en un degradado, nunca a un fallo.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Mas de esto no se lee de una caja `moov` ni de una etiqueta ID3: una
/// portada de 16 MB ya es una foto, no una portada, y leerla para pintar
/// 44 px no tiene sentido.
const TOPE: u64 = 24 * 1024 * 1024;

/// Lo que dice de si misma una cancion.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Etiquetas {
    pub titulo: Option<String>,
    pub artista: Option<String>,
    pub album: Option<String>,
    pub portada: Option<Portada>,
}

/// Una imagen metida en el fichero, tal cual (JPEG o PNG casi siempre).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Portada {
    pub datos: Vec<u8>,
}

impl Portada {
    /// La extension que le toca por sus primeros bytes: lo que dice el
    /// fichero sobre su tipo miente a menudo («image/jpg», vacio…).
    pub fn extension(&self) -> &'static str {
        match self.datos.as_slice() {
            [0x89, b'P', b'N', b'G', ..] => "png",
            [b'G', b'I', b'F', ..] => "gif",
            [b'B', b'M', ..] => "bmp",
            [
                b'R',
                b'I',
                b'F',
                b'F',
                _,
                _,
                _,
                _,
                b'W',
                b'E',
                b'B',
                b'P',
                ..,
            ] => "webp",
            _ => "jpg",
        }
    }
}

/// Las etiquetas del fichero en `ruta`. Sin poder abrirlo, ninguna.
pub fn leer(ruta: &Path) -> Etiquetas {
    std::fs::File::open(ruta)
        .and_then(|f| leer_de(std::io::BufReader::new(f)))
        .unwrap_or_default()
}

/// Las etiquetas de lo que venga en `r`, mirando sus primeros bytes.
pub fn leer_de<R: Read + Seek>(mut r: R) -> std::io::Result<Etiquetas> {
    let mut cabeza = [0u8; 12];
    let leidos = leer_lo_que_haya(&mut r, &mut cabeza)?;
    let cabeza = &cabeza[..leidos];
    r.seek(SeekFrom::Start(0))?;
    Ok(if cabeza.starts_with(b"ID3") {
        id3(&mut r)?
    } else if cabeza.starts_with(b"fLaC") {
        flac(&mut r)?
    } else if cabeza.get(4..8) == Some(b"ftyp") {
        mp4(&mut r)?
    } else {
        Etiquetas::default()
    })
}

fn leer_lo_que_haya<R: Read>(r: &mut R, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

fn leer_exacto<R: Read>(r: &mut R, largo: u64) -> std::io::Result<Vec<u8>> {
    if largo > TOPE {
        return Err(std::io::Error::other("demasiado grande"));
    }
    let mut v = vec![0u8; largo as usize];
    r.read_exact(&mut v)?;
    Ok(v)
}

fn sin_vacio(t: String) -> Option<String> {
    let t = t.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    (!t.is_empty()).then(|| t.to_string())
}

// --- ID3v2 -------------------------------------------------------------------

/// Un entero «synchsafe» de ID3: 7 bits por byte.
fn synchsafe(b: &[u8]) -> u32 {
    b.iter().fold(0u32, |n, &x| (n << 7) | u32::from(x & 0x7f))
}

/// Quita la «desincronizacion» de ID3: cada `FF 00` vuelve a ser `FF`.
fn resincronizar(datos: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(datos.len());
    let mut i = 0;
    while i < datos.len() {
        v.push(datos[i]);
        if datos[i] == 0xff && datos.get(i + 1) == Some(&0) {
            i += 1;
        }
        i += 1;
    }
    v
}

fn id3<R: Read>(r: &mut R) -> std::io::Result<Etiquetas> {
    let mut cabecera = [0u8; 10];
    r.read_exact(&mut cabecera)?;
    let version = cabecera[3];
    let banderas = cabecera[5];
    let largo = synchsafe(&cabecera[6..10]) as u64;
    let mut cuerpo = leer_exacto(r, largo)?;
    // En v2.2 y v2.3 la desincronizacion es de la etiqueta entera; en v2.4,
    // de cada marco (su bandera propia).
    if banderas & 0x80 != 0 && version < 4 {
        cuerpo = resincronizar(&cuerpo);
    }
    Ok(marcos_id3(&cuerpo, version, banderas))
}

/// Recorre los marcos de una etiqueta ID3v2 ya leida.
pub fn marcos_id3(cuerpo: &[u8], version: u8, banderas: u8) -> Etiquetas {
    let mut e = Etiquetas::default();
    let mut i = 0usize;
    // La cabecera extendida, si la hay, se salta.
    if banderas & 0x40 != 0 && version >= 3 && cuerpo.len() >= 4 {
        let extra = if version == 4 {
            synchsafe(&cuerpo[..4]) as usize
        } else {
            u32::from_be_bytes([cuerpo[0], cuerpo[1], cuerpo[2], cuerpo[3]]) as usize + 4
        };
        i = extra.min(cuerpo.len());
    }
    let (largo_id, largo_cabecera) = if version == 2 { (3, 6) } else { (4, 10) };
    let mut portada_buena = false;
    while i + largo_cabecera <= cuerpo.len() {
        let id = &cuerpo[i..i + largo_id];
        if id[0] == 0 {
            break; // el relleno
        }
        let largo = match version {
            2 => u32::from_be_bytes([0, cuerpo[i + 3], cuerpo[i + 4], cuerpo[i + 5]]),
            3 => u32::from_be_bytes([cuerpo[i + 4], cuerpo[i + 5], cuerpo[i + 6], cuerpo[i + 7]]),
            _ => synchsafe(&cuerpo[i + 4..i + 8]),
        } as usize;
        let inicio = i + largo_cabecera;
        let Some(fin) = inicio.checked_add(largo).filter(|f| *f <= cuerpo.len()) else {
            break;
        };
        let mut datos = std::borrow::Cow::Borrowed(&cuerpo[inicio..fin]);
        if version == 4 {
            let b = cuerpo[i + 9];
            if b & 0x02 != 0 {
                datos = std::borrow::Cow::Owned(resincronizar(&datos));
            }
            if b & 0x01 != 0 && datos.len() >= 4 {
                datos = std::borrow::Cow::Owned(datos[4..].to_vec());
            }
        }
        match id {
            b"TIT2" | b"TT2" => e.titulo = e.titulo.or_else(|| texto_id3(&datos)),
            b"TPE1" | b"TP1" => e.artista = e.artista.or_else(|| texto_id3(&datos)),
            b"TALB" | b"TAL" => e.album = e.album.or_else(|| texto_id3(&datos)),
            b"APIC" | b"PIC" if !portada_buena => {
                if let Some((tipo, imagen)) = imagen_id3(&datos, version == 2) {
                    // La portada de delante (tipo 3) gana a cualquier otra.
                    portada_buena = tipo == 3;
                    if portada_buena || e.portada.is_none() {
                        e.portada = Some(Portada { datos: imagen });
                    }
                }
            }
            _ => {}
        }
        i = fin;
    }
    e
}

/// Un texto de ID3: su primer byte dice la codificacion. Con varios valores
/// separados por un nulo, el primero.
fn texto_id3(datos: &[u8]) -> Option<String> {
    let (&cod, resto) = datos.split_first()?;
    let (t, _) = cadena_id3(resto, cod);
    sin_vacio(t)
}

/// Una cadena terminada en nulo con la codificacion `cod`, y lo que queda
/// detras del nulo.
fn cadena_id3(datos: &[u8], cod: u8) -> (String, &[u8]) {
    match cod {
        1 | 2 => {
            // UTF-16, con BOM (1) o big-endian sin el (2). El nulo son dos
            // bytes alineados.
            let mut fin = datos.len();
            let mut k = 0;
            while k + 1 < datos.len() {
                if datos[k] == 0 && datos[k + 1] == 0 {
                    fin = k;
                    break;
                }
                k += 2;
            }
            let cadena = &datos[..fin];
            let resto = &datos[(fin + 2).min(datos.len())..];
            let (cadena, mut be) = match cadena {
                [0xff, 0xfe, r @ ..] => (r, false),
                [0xfe, 0xff, r @ ..] => (r, true),
                r => (r, true),
            };
            if cod == 2 {
                be = true;
            }
            let unidades: Vec<u16> = cadena
                .chunks_exact(2)
                .map(|c| {
                    if be {
                        u16::from_be_bytes([c[0], c[1]])
                    } else {
                        u16::from_le_bytes([c[0], c[1]])
                    }
                })
                .collect();
            (String::from_utf16_lossy(&unidades), resto)
        }
        _ => {
            let fin = datos.iter().position(|&b| b == 0).unwrap_or(datos.len());
            let cadena = &datos[..fin];
            let resto = &datos[(fin + 1).min(datos.len())..];
            let t = if cod == 3 {
                String::from_utf8_lossy(cadena).into_owned()
            } else {
                // ISO-8859-1: cada byte es su punto de codigo.
                cadena.iter().map(|&b| char::from(b)).collect()
            };
            (t, resto)
        }
    }
}

/// `APIC`: codificacion, tipo MIME (o el formato de tres letras de `PIC`),
/// tipo de imagen, descripcion y la imagen. Devuelve `(tipo, imagen)`.
fn imagen_id3(datos: &[u8], v22: bool) -> Option<(u8, Vec<u8>)> {
    let (&cod, resto) = datos.split_first()?;
    let resto = if v22 {
        resto.get(3..)?
    } else {
        let fin = resto.iter().position(|&b| b == 0)?;
        &resto[fin + 1..]
    };
    let (&tipo, resto) = resto.split_first()?;
    let (_, imagen) = cadena_id3(resto, cod);
    (!imagen.is_empty()).then(|| (tipo, imagen.to_vec()))
}

// --- MPEG-4 ------------------------------------------------------------------

fn mp4<R: Read + Seek>(r: &mut R) -> std::io::Result<Etiquetas> {
    // Las cajas de arriba se recorren con saltos: el `mdat` (el sonido) puede
    // ir delante del `moov` y pesar muchos megas que no hace falta leer.
    let fin = r.seek(SeekFrom::End(0))?;
    let mut pos = r.seek(SeekFrom::Start(0))?;
    while pos + 8 <= fin {
        let mut cab = [0u8; 8];
        r.read_exact(&mut cab)?;
        let mut largo = u64::from(u32::from_be_bytes([cab[0], cab[1], cab[2], cab[3]]));
        let mut cabecera = 8;
        if largo == 1 {
            let mut grande = [0u8; 8];
            r.read_exact(&mut grande)?;
            largo = u64::from_be_bytes(grande);
            cabecera = 16;
        } else if largo == 0 {
            largo = fin - pos;
        }
        if largo < cabecera {
            break;
        }
        if &cab[4..8] == b"moov" {
            let moov = leer_exacto(r, largo - cabecera)?;
            return Ok(moov_mp4(&moov));
        }
        pos = r.seek(SeekFrom::Start(pos + largo))?;
    }
    Ok(Etiquetas::default())
}

/// Las cajas hijas de `datos`: `(tipo, contenido)`.
fn cajas(datos: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut v = Vec::new();
    let mut i = 0usize;
    while i + 8 <= datos.len() {
        let largo = u32::from_be_bytes([datos[i], datos[i + 1], datos[i + 2], datos[i + 3]]);
        let tipo = [datos[i + 4], datos[i + 5], datos[i + 6], datos[i + 7]];
        let (largo, cabecera) = match largo {
            0 => (datos.len() - i, 8),
            1 if i + 16 <= datos.len() => {
                let mut b = [0u8; 8];
                b.copy_from_slice(&datos[i + 8..i + 16]);
                (
                    usize::try_from(u64::from_be_bytes(b)).unwrap_or(usize::MAX),
                    16,
                )
            }
            n => (n as usize, 8),
        };
        let Some(fin) = i.checked_add(largo).filter(|f| *f <= datos.len()) else {
            break;
        };
        if largo < cabecera {
            break;
        }
        v.push((tipo, &datos[i + cabecera..fin]));
        i = fin;
    }
    v
}

fn hija<'a>(datos: &'a [u8], tipo: &[u8; 4]) -> Option<&'a [u8]> {
    cajas(datos)
        .into_iter()
        .find(|(t, _)| t == tipo)
        .map(|(_, c)| c)
}

/// El contenido de una caja `moov` ya leida.
pub fn moov_mp4(moov: &[u8]) -> Etiquetas {
    let mut e = Etiquetas::default();
    let Some(meta) = hija(moov, b"udta").and_then(|u| hija(u, b"meta")) else {
        return e;
    };
    // `meta` es una «full box» (version y banderas delante) en MPEG-4, y no
    // lo es en el QuickTime de antes: se distingue por donde cae el `hdlr`.
    let meta = if meta.get(4..8) == Some(b"hdlr") {
        meta
    } else {
        meta.get(4..).unwrap_or_default()
    };
    let Some(ilst) = hija(meta, b"ilst") else {
        return e;
    };
    for (tipo, item) in cajas(ilst) {
        // Cada elemento lleva su `data`: tipo (4), idioma (4) y el valor.
        let Some(valor) = hija(item, b"data").and_then(|d| d.get(8..)) else {
            continue;
        };
        let texto = || sin_vacio(String::from_utf8_lossy(valor).into_owned());
        match &tipo {
            b"\xa9nam" => e.titulo = texto(),
            b"\xa9ART" => e.artista = texto(),
            b"aART" if e.artista.is_none() => e.artista = texto(),
            b"\xa9alb" => e.album = texto(),
            b"covr" if e.portada.is_none() && !valor.is_empty() => {
                e.portada = Some(Portada {
                    datos: valor.to_vec(),
                })
            }
            _ => {}
        }
    }
    e
}

// --- FLAC --------------------------------------------------------------------

fn flac<R: Read>(r: &mut R) -> std::io::Result<Etiquetas> {
    let mut marca = [0u8; 4];
    r.read_exact(&mut marca)?;
    let mut e = Etiquetas::default();
    let mut portada_buena = false;
    loop {
        let mut cab = [0u8; 4];
        r.read_exact(&mut cab)?;
        let ultimo = cab[0] & 0x80 != 0;
        let tipo = cab[0] & 0x7f;
        let largo = u64::from(u32::from_be_bytes([0, cab[1], cab[2], cab[3]]));
        let bloque = leer_exacto(r, largo)?;
        match tipo {
            4 => comentarios_vorbis(&bloque, &mut e),
            6 if !portada_buena => {
                if let Some((t, imagen)) = imagen_flac(&bloque) {
                    portada_buena = t == 3;
                    if portada_buena || e.portada.is_none() {
                        e.portada = Some(Portada { datos: imagen });
                    }
                }
            }
            _ => {}
        }
        if ultimo {
            break;
        }
    }
    Ok(e)
}

fn u32_le(d: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(i..i + 4)?.try_into().ok()?))
}

fn u32_be(d: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(i..i + 4)?.try_into().ok()?))
}

/// Los comentarios Vorbis (`TITLE=...`), en little-endian.
pub fn comentarios_vorbis(d: &[u8], e: &mut Etiquetas) {
    let Some(vendedor) = u32_le(d, 0) else {
        return;
    };
    let mut i = 4 + vendedor as usize;
    let Some(cuantos) = u32_le(d, i) else {
        return;
    };
    i += 4;
    for _ in 0..cuantos {
        let Some(largo) = u32_le(d, i) else {
            return;
        };
        let Some(c) = d.get(i + 4..i + 4 + largo as usize) else {
            return;
        };
        i += 4 + largo as usize;
        let c = String::from_utf8_lossy(c);
        let Some((clave, valor)) = c.split_once('=') else {
            continue;
        };
        let valor = sin_vacio(valor.to_string());
        match clave.to_ascii_uppercase().as_str() {
            "TITLE" if e.titulo.is_none() => e.titulo = valor,
            "ARTIST" if e.artista.is_none() => e.artista = valor,
            "ALBUM" if e.album.is_none() => e.album = valor,
            _ => {}
        }
    }
}

/// El bloque `PICTURE` de FLAC: `(tipo, imagen)`.
fn imagen_flac(d: &[u8]) -> Option<(u32, Vec<u8>)> {
    let tipo = u32_be(d, 0)?;
    let mime = u32_be(d, 4)? as usize;
    let i = 8 + mime;
    let desc = u32_be(d, i)? as usize;
    let i = i + 4 + desc + 16;
    let largo = u32_be(d, i)? as usize;
    let imagen = d.get(i + 4..i + 4 + largo)?;
    (!imagen.is_empty()).then(|| (tipo, imagen.to_vec()))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::io::Cursor;

    const JPEG: &[u8] = &[0xff, 0xd8, 0xff, 0xe0, 1, 2, 3, 0xff, 0xd9];

    fn marco_v3(id: &[u8; 4], datos: &[u8]) -> Vec<u8> {
        let mut v = id.to_vec();
        v.extend((datos.len() as u32).to_be_bytes());
        v.extend([0, 0]);
        v.extend(datos);
        v
    }

    fn etiqueta_id3(version: u8, marcos: &[u8]) -> Vec<u8> {
        let n = marcos.len() as u32;
        let mut v = b"ID3".to_vec();
        v.extend([version, 0, 0]);
        v.extend([
            ((n >> 21) & 0x7f) as u8,
            ((n >> 14) & 0x7f) as u8,
            ((n >> 7) & 0x7f) as u8,
            (n & 0x7f) as u8,
        ]);
        v.extend(marcos);
        v.extend([0xff, 0xfb, 0x90, 0x00]); // el primer cuadro de MP3
        v
    }

    #[test]
    fn un_mp3_con_id3v23_da_titulo_artista_y_portada() {
        let mut marcos = marco_v3(b"TIT2", b"\x03B\xc3\xa9same mucho\x00");
        // UTF-16 con BOM, como lo escribe el Reproductor de Windows.
        let mut artista = vec![1, 0xff, 0xfe];
        for u in "Consuelo".encode_utf16() {
            artista.extend(u.to_le_bytes());
        }
        marcos.extend(marco_v3(b"TPE1", &artista));
        let mut apic = vec![0];
        apic.extend(b"image/jpeg\x00");
        apic.push(4); // la de atras
        apic.extend(b"atras\x00");
        apic.extend(b"\x89PNG....");
        marcos.extend(marco_v3(b"APIC", &apic));
        let mut apic = vec![0];
        apic.extend(b"image/jpeg\x00");
        apic.push(3); // la de delante
        apic.extend(b"\x00");
        apic.extend(JPEG);
        marcos.extend(marco_v3(b"APIC", &apic));
        marcos.extend([0; 16]); // relleno
        let e = leer_de(Cursor::new(etiqueta_id3(3, &marcos))).unwrap();
        assert_eq!(e.titulo.as_deref(), Some("Bésame mucho"));
        assert_eq!(e.artista.as_deref(), Some("Consuelo"));
        let p = e.portada.unwrap();
        assert_eq!(p.datos, JPEG, "gana la portada de delante");
        assert_eq!(p.extension(), "jpg");
    }

    #[test]
    fn id3v22_con_pic_y_latin1() {
        let mut marcos = b"TT2".to_vec();
        marcos.extend([0, 0, 6, 0]);
        marcos.extend(b"Ca\xf1o\x00");
        marcos.extend(b"PIC");
        let mut pic = vec![0];
        pic.extend(b"PNG");
        pic.push(3);
        pic.push(0);
        pic.extend(b"\x89PNGxx");
        marcos.extend([0, 0, pic.len() as u8]);
        marcos.extend(&pic);
        let e = leer_de(Cursor::new(etiqueta_id3(2, &marcos))).unwrap();
        assert_eq!(e.titulo.as_deref(), Some("Caño"));
        assert_eq!(e.portada.unwrap().extension(), "png");
    }

    fn caja(tipo: &[u8; 4], contenido: &[u8]) -> Vec<u8> {
        let mut v = ((contenido.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend(tipo);
        v.extend(contenido);
        v
    }

    fn dato(tipo: u32, valor: &[u8]) -> Vec<u8> {
        let mut d = tipo.to_be_bytes().to_vec();
        d.extend([0, 0, 0, 0]);
        d.extend(valor);
        caja(b"data", &d)
    }

    #[test]
    fn un_m4a_con_el_moov_detras_del_sonido() {
        let mut ilst = caja(b"\xa9nam", &dato(1, "Canción".as_bytes()));
        ilst.extend(caja(b"\xa9ART", &dato(1, b"Artista")));
        ilst.extend(caja(b"covr", &dato(13, JPEG)));
        let mut meta = vec![0, 0, 0, 0];
        meta.extend(caja(b"hdlr", &[0; 25]));
        meta.extend(caja(b"ilst", &ilst));
        let moov = caja(b"moov", &caja(b"udta", &caja(b"meta", &meta)));
        let mut fichero = caja(b"ftyp", b"M4A \x00\x00\x00\x00");
        fichero.extend(caja(b"mdat", &[7; 4096]));
        fichero.extend(moov);
        let e = leer_de(Cursor::new(fichero)).unwrap();
        assert_eq!(e.titulo.as_deref(), Some("Canción"));
        assert_eq!(e.artista.as_deref(), Some("Artista"));
        assert_eq!(e.portada.unwrap().datos, JPEG);
    }

    #[test]
    fn un_flac_con_comentarios_e_imagen() {
        let mut com = 3u32.to_le_bytes().to_vec();
        com.extend(b"yo!");
        com.extend(2u32.to_le_bytes());
        for c in ["title=Uno", "ARTIST=Dos"] {
            com.extend((c.len() as u32).to_le_bytes());
            com.extend(c.as_bytes());
        }
        let mut pic = 3u32.to_be_bytes().to_vec();
        pic.extend(10u32.to_be_bytes());
        pic.extend(b"image/jpeg");
        pic.extend(0u32.to_be_bytes());
        pic.extend([0; 16]);
        pic.extend((JPEG.len() as u32).to_be_bytes());
        pic.extend(JPEG);
        let mut f = b"fLaC".to_vec();
        f.extend([0, 0, 0, 34]);
        f.extend([0; 34]); // STREAMINFO
        f.push(4);
        f.extend(&(com.len() as u32).to_be_bytes()[1..]);
        f.extend(&com);
        f.push(0x80 | 6);
        f.extend(&(pic.len() as u32).to_be_bytes()[1..]);
        f.extend(&pic);
        let e = leer_de(Cursor::new(f)).unwrap();
        assert_eq!(e.titulo.as_deref(), Some("Uno"));
        assert_eq!(e.artista.as_deref(), Some("Dos"));
        assert_eq!(e.portada.unwrap().datos, JPEG);
    }

    #[test]
    fn lo_que_no_se_conoce_o_viene_roto_no_trae_nada() {
        assert_eq!(
            leer_de(Cursor::new(b"RIFF....WAVE")).unwrap(),
            Etiquetas::default()
        );
        assert_eq!(leer_de(Cursor::new(b"")).unwrap(), Etiquetas::default());
        // Una etiqueta que dice medir mas de lo que hay.
        assert!(leer_de(Cursor::new(b"ID3\x03\x00\x00\x00\x00\x7f\x7fTIT2")).is_err());
        assert_eq!(leer(Path::new("/no/esta.mp3")), Etiquetas::default());
    }
}
