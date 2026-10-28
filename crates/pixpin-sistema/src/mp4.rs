//! **Si un MP4 (o `.m4a`) se puede abrir**, mirando solo sus cajas de arriba
//! (28-oct-2026, grabar sin perder nada).
//!
//! Lo que queda de una grabacion cortada a mitad es un fichero a medias, y
//! lo que se puede hacer con el depende de como quedo:
//!
//! - **Cerrado** (`moov` y sin `moof`): el escritor llego a `Finalize`. Vale
//!   tal cual; basta con ponerlo en su nombre.
//! - **Fragmentado** (`moof`): un MP4 fragmentado, que se puede abrir aunque
//!   no se cerrara. Sin su indice de fragmentos (`mfra`/`sidx`) algunos
//!   reproductores saltan mal por el, y conviene rehacerlo (copiarlo sin
//!   volver a codificar a un MP4 normal) si se puede.
//! - **Sin indice** (`ftyp`/`mdat` pero ni `moov` ni `moof`): un MP4 normal
//!   cortado. No lo abre nadie; se conserva hasta que el usuario lo descarte.
//! - **Nada**: ni siquiera la cabecera.
//!
//! Se leen solo las cabeceras de ocho bytes (o dieciseis) de cada caja: un
//! video de una hora se mira en microsegundos.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Como quedo un MP4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoMp4 {
    /// No existe, esta vacio o no es un MP4.
    Nada,
    /// Cortado sin indice: no se puede abrir.
    SinIndice,
    /// Fragmentado: se puede abrir. `con_indice`: lleva `mfra` o `sidx`.
    Fragmentado { con_indice: bool },
    /// Cerrado bien.
    Cerrado,
}

impl EstadoMp4 {
    /// Si se puede abrir tal cual.
    pub fn se_abre(self) -> bool {
        matches!(self, EstadoMp4::Cerrado | EstadoMp4::Fragmentado { .. })
    }
}

/// Las cajas de arriba de un MP4, en orden, con su tipo (cuatro letras).
/// Para en la primera que no tiene cabecera entera.
pub fn cajas(ruta: &Path) -> std::io::Result<Vec<[u8; 4]>> {
    let mut f = File::open(ruta)?;
    let largo = f.metadata()?.len();
    let mut v = Vec::new();
    let mut pos = 0u64;
    // Un tope por si el fichero es basura que parece cajas.
    while pos + 8 <= largo && v.len() < 100_000 {
        f.seek(SeekFrom::Start(pos))?;
        let mut h = [0u8; 8];
        f.read_exact(&mut h)?;
        let tipo = [h[4], h[5], h[6], h[7]];
        if !tipo.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
            break;
        }
        let corto = u64::from(u32::from_be_bytes([h[0], h[1], h[2], h[3]]));
        let tam = match corto {
            // Hasta el final del fichero.
            0 => largo - pos,
            // El tamano va en los ocho bytes siguientes.
            1 => {
                let mut g = [0u8; 8];
                if f.read_exact(&mut g).is_err() {
                    break;
                }
                u64::from_be_bytes(g)
            }
            t => t,
        };
        if tam < 8 {
            break;
        }
        v.push(tipo);
        pos = match pos.checked_add(tam) {
            Some(p) => p,
            None => break,
        };
    }
    Ok(v)
}

/// **Como quedo** el MP4 de `ruta`.
pub fn mirar(ruta: &Path) -> EstadoMp4 {
    let Ok(v) = cajas(ruta) else {
        return EstadoMp4::Nada;
    };
    let hay = |t: &[u8; 4]| v.iter().any(|c| c == t);
    if hay(b"moof") {
        EstadoMp4::Fragmentado {
            con_indice: hay(b"mfra") || hay(b"sidx"),
        }
    } else if hay(b"moov") {
        EstadoMp4::Cerrado
    } else if hay(b"ftyp") || hay(b"mdat") {
        EstadoMp4::SinIndice
    } else {
        EstadoMp4::Nada
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn caja(tipo: &[u8; 4], cuerpo: usize) -> Vec<u8> {
        let mut c = ((cuerpo + 8) as u32).to_be_bytes().to_vec();
        c.extend_from_slice(tipo);
        c.resize(cuerpo + 8, 0);
        c
    }

    fn fichero(nombre: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("pixpin-mp4-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let r = dir.join(nombre);
        std::fs::write(&r, bytes).unwrap();
        r
    }

    #[test]
    fn un_mp4_cerrado_lleva_su_moov() {
        let b = [caja(b"ftyp", 16), caja(b"mdat", 1000), caja(b"moov", 200)].concat();
        let r = fichero("cerrado.mp4", &b);
        assert_eq!(cajas(&r).unwrap(), vec![*b"ftyp", *b"mdat", *b"moov"]);
        assert_eq!(mirar(&r), EstadoMp4::Cerrado);
        assert!(mirar(&r).se_abre());
    }

    #[test]
    fn un_mp4_cortado_a_mitad_del_mdat_no_se_abre() {
        // El `mdat` dice mas de lo que hay: la aplicacion murio escribiendo.
        let mut b = [caja(b"ftyp", 16), caja(b"mdat", 5000)].concat();
        b.truncate(2000);
        let r = fichero("cortado.mp4", &b);
        assert_eq!(mirar(&r), EstadoMp4::SinIndice);
        assert!(!mirar(&r).se_abre());
        // Un mdat «hasta el final» (tamano cero) tampoco trae indice.
        let mut b = caja(b"ftyp", 16);
        b.extend_from_slice(&[0, 0, 0, 0]);
        b.extend_from_slice(b"mdat");
        b.resize(b.len() + 300, 0);
        let r = fichero("cero.mp4", &b);
        assert_eq!(mirar(&r), EstadoMp4::SinIndice);
    }

    #[test]
    fn un_mp4_fragmentado_se_abre_aunque_se_cortara() {
        let mut b = [
            caja(b"ftyp", 16),
            caja(b"moov", 100),
            caja(b"moof", 50),
            caja(b"mdat", 400),
            caja(b"moof", 50),
            caja(b"mdat", 400),
        ]
        .concat();
        b.truncate(b.len() - 100);
        let r = fichero("frag.mp4", &b);
        assert_eq!(mirar(&r), EstadoMp4::Fragmentado { con_indice: false });
        assert!(mirar(&r).se_abre());
        let entero = [
            caja(b"ftyp", 16),
            caja(b"moov", 100),
            caja(b"moof", 50),
            caja(b"mdat", 400),
            caja(b"mfra", 30),
        ]
        .concat();
        let r = fichero("frag-mfra.mp4", &entero);
        assert_eq!(mirar(&r), EstadoMp4::Fragmentado { con_indice: true });
    }

    #[test]
    fn el_tamano_largo_de_64_bits_se_entiende() {
        let mut b = caja(b"ftyp", 16);
        // `mdat` con tamano 1 y el de verdad en 64 bits: 16 + 100.
        b.extend_from_slice(&1u32.to_be_bytes());
        b.extend_from_slice(b"mdat");
        b.extend_from_slice(&116u64.to_be_bytes());
        b.resize(b.len() + 100, 0);
        b.extend(caja(b"moov", 20));
        let r = fichero("largo.mp4", &b);
        assert_eq!(cajas(&r).unwrap(), vec![*b"ftyp", *b"mdat", *b"moov"]);
        assert_eq!(mirar(&r), EstadoMp4::Cerrado);
    }

    #[test]
    fn lo_que_no_es_un_mp4_es_nada() {
        assert_eq!(mirar(Path::new("/no/esta/aqui.mp4")), EstadoMp4::Nada);
        let r = fichero("vacio.mp4", b"");
        assert_eq!(mirar(&r), EstadoMp4::Nada);
        let r = fichero("texto.mp4", b"\x00\x01hola que tal esto no es nada");
        assert_eq!(mirar(&r), EstadoMp4::Nada);
    }
}
