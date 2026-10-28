//! **El respaldo crudo de una grabacion** (28-oct-2026, el usuario: «todo
//! tiene que ser rapido y fiable al grabar: si se perdiera algo seria malo,
//! se perderia una reunion»).
//!
//! El `.m4a` que escribe Media Foundation solo se puede abrir despues de
//! `Finalize`, que escribe su indice (`moov`) al final. Si la aplicacion
//! muere, se va la luz o Windows se cuelga a mitad de una reunion de dos
//! horas, lo que queda es un `.m4a.parte` que no abre nadie. Por eso, junto
//! al `.m4a`, se escribe **el mismo PCM en un WAV**: un formato que no tiene
//! indice que cerrar, donde cada muestra escrita ya vale.
//!
//! - **Por que WAV y no un MP4 fragmentado**: el AAC fragmentado se puede
//!   abrir a medias, pero lo lee mal el reproductor de Android y el
//!   transcriptor (`IMFSourceReader`) salta mal en el; y es el mismo escritor
//!   de Media Foundation el que tendria que sobrevivir al corte. El WAV lo
//!   escribe este codigo, sin sistema por medio, y se puede leer con un
//!   `read` cualquiera. Al recuperar se vuelve a codificar a `.m4a`
//!   ([`crate::rehacer_m4a`]), el formato de siempre: el movil y
//!   `pixpin-voz` no se enteran.
//! - **Lo que cuesta**: mono a 48 000 Hz son 96 KB/s, unos 345 MB por hora,
//!   en disco (en memoria, solo el bufer de 256 KB). Se borra al cerrar bien
//!   la nota; mientras se graba, `grabar_clase` vigila que quepa.
//! - **Cuanto se puede perder**: lo que el sistema no haya llevado al disco.
//!   Cada [`SEGUNDOS_POR_VOLCADO`] segundos de sonido se vacia el bufer y se
//!   pide `sync_data`, asi que un corte de luz pierde como mucho esos
//!   segundos; un cierre de golpe de la aplicacion, solo lo del bufer (el
//!   sistema ya tiene lo volcado).
//! - **Si escribir falla** (disco lleno, la memoria USB que se quita), el
//!   respaldo se calla y lo dice una vez ([`Respaldo::fallo`]); la nota sigue
//!   grabandose igual. Un respaldo nunca para una grabacion.
//!
//! La cabecera se escribe al empezar con los tamanos «sin fin»
//! (`0xFFFF_FFFF`, lo que hacen las grabadoras que escriben en directo) y
//! se pone la buena al cerrar; al leer uno cortado ([`leer`]) el tamano sale
//! del fichero, no de la cabecera.
//!
//! Es puro (`std::fs`): se prueba en Linux.

use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Lo que ocupa la cabecera que se escribe aqui.
pub const BYTES_CABECERA: u64 = 44;

/// Cada cuantos segundos de sonido se lleva al disco de verdad.
pub const SEGUNDOS_POR_VOLCADO: u64 = 5;

/// El bufer en memoria (unos 2,7 s de mono a 48 kHz).
const BUFER: usize = 256 * 1024;

/// Lo que se pone de tamano mientras no se sabe.
const SIN_FIN: u32 = 0xFFFF_FFFF;

/// `<destino>.respaldo.wav`: al lado del destino y con su nombre entero,
/// como el `.parte`.
pub fn ruta_del_respaldo(destino: &Path) -> PathBuf {
    let mut s = destino.as_os_str().to_os_string();
    s.push(".respaldo.wav");
    PathBuf::from(s)
}

/// La cabecera RIFF/WAVE de PCM de 16 bits.
pub fn cabecera(muestreo: u32, canales: u16, bytes_de_datos: u32) -> [u8; 44] {
    let canales = canales.max(1);
    let alineado = 2 * canales;
    let riff = if bytes_de_datos == SIN_FIN {
        SIN_FIN
    } else {
        bytes_de_datos.saturating_add(36)
    };
    let mut c = [0u8; 44];
    c[0..4].copy_from_slice(b"RIFF");
    c[4..8].copy_from_slice(&riff.to_le_bytes());
    c[8..12].copy_from_slice(b"WAVE");
    c[12..16].copy_from_slice(b"fmt ");
    c[16..20].copy_from_slice(&16u32.to_le_bytes());
    c[20..22].copy_from_slice(&1u16.to_le_bytes());
    c[22..24].copy_from_slice(&canales.to_le_bytes());
    c[24..28].copy_from_slice(&muestreo.to_le_bytes());
    c[28..32].copy_from_slice(&(muestreo * u32::from(alineado)).to_le_bytes());
    c[32..34].copy_from_slice(&alineado.to_le_bytes());
    c[34..36].copy_from_slice(&16u16.to_le_bytes());
    c[36..40].copy_from_slice(b"data");
    c[40..44].copy_from_slice(&bytes_de_datos.to_le_bytes());
    c
}

/// Un respaldo escribiendose.
pub struct Respaldo {
    escritor: Option<BufWriter<File>>,
    ruta: PathBuf,
    muestreo: u32,
    canales: u16,
    /// Marcos escritos.
    marcos: u64,
    /// Marcos desde el ultimo volcado al disco.
    sin_volcar: u64,
    fallo: Option<io::ErrorKind>,
}

impl Respaldo {
    /// Crea (o trunca) `ruta` y escribe la cabecera.
    pub fn crear(ruta: &Path, muestreo: u32, canales: u16) -> io::Result<Respaldo> {
        let mut f = BufWriter::with_capacity(BUFER, File::create(ruta)?);
        f.write_all(&cabecera(muestreo, canales, SIN_FIN))?;
        f.flush()?;
        Ok(Respaldo {
            escritor: Some(f),
            ruta: ruta.to_path_buf(),
            muestreo,
            canales: canales.max(1),
            marcos: 0,
            sin_volcar: 0,
            fallo: None,
        })
    }

    /// Donde se escribe.
    pub fn ruta(&self) -> &Path {
        &self.ruta
    }

    /// Marcos escritos hasta ahora.
    pub fn marcos(&self) -> u64 {
        self.marcos
    }

    /// Por que dejo de escribir, si dejo.
    pub fn fallo(&self) -> Option<io::ErrorKind> {
        self.fallo
    }

    /// Un bloque intercalado de 16 bits, detras de lo anterior. Si falla, el
    /// respaldo se calla (las siguientes no hacen nada) y devuelve el fallo
    /// esa vez.
    pub fn escribir(&mut self, muestras: &[i16]) -> io::Result<()> {
        let Some(f) = self.escritor.as_mut() else {
            return Ok(());
        };
        let mut bytes = Vec::with_capacity(muestras.len() * 2);
        for m in muestras {
            bytes.extend_from_slice(&m.to_le_bytes());
        }
        let marcos = (muestras.len() / self.canales as usize) as u64;
        let hecho = f.write_all(&bytes).and_then(|()| {
            self.sin_volcar += marcos;
            if self.sin_volcar >= u64::from(self.muestreo) * SEGUNDOS_POR_VOLCADO {
                self.sin_volcar = 0;
                f.flush()?;
                // Hasta el disco: un corte de luz no se lleva lo de antes.
                f.get_ref().sync_data()?;
            }
            Ok(())
        });
        match hecho {
            Ok(()) => {
                self.marcos += marcos;
                Ok(())
            }
            Err(e) => {
                self.fallo = Some(e.kind());
                // Lo que ya este en el disco se queda; no se intenta mas.
                self.escritor = None;
                Err(e)
            }
        }
    }

    /// Vacia el bufer al disco (sin cerrar).
    pub fn volcar(&mut self) -> io::Result<()> {
        if let Some(f) = self.escritor.as_mut() {
            f.flush()?;
            f.get_ref().sync_data()?;
            self.sin_volcar = 0;
        }
        Ok(())
    }

    /// Cierra con la cabecera buena. Devuelve los marcos escritos.
    pub fn cerrar(mut self) -> io::Result<u64> {
        let Some(mut f) = self.escritor.take() else {
            return Ok(self.marcos);
        };
        f.flush()?;
        let mut f = f.into_inner().map_err(|e| e.into_error())?;
        let datos = self.marcos * 2 * u64::from(self.canales);
        let datos = u32::try_from(datos).unwrap_or(SIN_FIN - 36);
        f.seek(SeekFrom::Start(0))?;
        f.write_all(&cabecera(self.muestreo, self.canales, datos))?;
        f.sync_data()?;
        Ok(self.marcos)
    }
}

/// Lo que dice un WAV (cerrado o cortado).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wav {
    pub muestreo: u32,
    pub canales: u16,
    /// Donde empiezan las muestras.
    pub inicio: u64,
    /// Marcos que hay de verdad en el fichero (un marco a medias no cuenta).
    pub marcos: u64,
}

impl Wav {
    /// Lo que dura, en milisegundos.
    pub fn duracion_ms(&self) -> i64 {
        if self.muestreo == 0 {
            return 0;
        }
        (u128::from(self.marcos) * 1000 / u128::from(self.muestreo)) as i64
    }
}

/// **Lee la cabecera** de un WAV de PCM de 16 bits, aunque este cortado: el
/// tamano de los datos sale de lo que mide el fichero (y de la cabecera solo
/// si es menor, que es un WAV cerrado con algo detras).
pub fn leer(ruta: &Path) -> io::Result<Wav> {
    let mut f = File::open(ruta)?;
    let largo = f.metadata()?.len();
    let mut riff = [0u8; 12];
    f.read_exact(&mut riff)?;
    if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "no es un WAV"));
    }
    let mut formato: Option<(u32, u16)> = None;
    let mut pos = 12u64;
    loop {
        let mut trozo = [0u8; 8];
        f.seek(SeekFrom::Start(pos))?;
        if f.read_exact(&mut trozo).is_err() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "WAV sin datos"));
        }
        let tam = u32::from_le_bytes([trozo[4], trozo[5], trozo[6], trozo[7]]);
        if &trozo[0..4] == b"fmt " {
            let mut fmt = [0u8; 16];
            f.read_exact(&mut fmt)?;
            let etiqueta = u16::from_le_bytes([fmt[0], fmt[1]]);
            let canales = u16::from_le_bytes([fmt[2], fmt[3]]);
            let muestreo = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
            let bits = u16::from_le_bytes([fmt[14], fmt[15]]);
            if etiqueta != 1 || bits != 16 || canales == 0 || muestreo == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "WAV que no es PCM de 16 bits",
                ));
            }
            formato = Some((muestreo, canales));
        } else if &trozo[0..4] == b"data" {
            let (muestreo, canales) = formato.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "WAV sin formato antes de los datos",
                )
            })?;
            let inicio = pos + 8;
            let en_fichero = largo.saturating_sub(inicio);
            let dichos = if tam == SIN_FIN {
                en_fichero
            } else {
                u64::from(tam).min(en_fichero)
            };
            let marcos = dichos / (2 * u64::from(canales));
            return Ok(Wav {
                muestreo,
                canales,
                inicio,
                marcos,
            });
        }
        // Los trozos van alineados a dos bytes.
        pos += 8 + u64::from(tam) + u64::from(tam & 1);
        if pos >= largo {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "WAV sin datos"));
        }
    }
}

/// **Recorre las muestras** de un WAV en trozos de como mucho `marcos`
/// marcos, intercaladas, y se las da a `cada`.
pub fn recorrer<E: From<io::Error>>(
    ruta: &Path,
    wav: &Wav,
    marcos: usize,
    mut cada: impl FnMut(&[i16]) -> Result<(), E>,
) -> Result<(), E> {
    let mut f = io::BufReader::new(File::open(ruta)?);
    f.seek(SeekFrom::Start(wav.inicio))?;
    let por_marco = 2 * wav.canales as usize;
    let mut quedan = wav.marcos;
    let mut bytes = vec![0u8; marcos.max(1) * por_marco];
    let mut muestras: Vec<i16> = Vec::with_capacity(marcos.max(1) * wav.canales as usize);
    while quedan > 0 {
        let n = quedan.min(marcos.max(1) as u64) as usize;
        let b = &mut bytes[..n * por_marco];
        f.read_exact(b)?;
        muestras.clear();
        muestras.extend(b.chunks_exact(2).map(|p| i16::from_le_bytes([p[0], p[1]])));
        cada(&muestras)?;
        quedan -= n as u64;
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pixpin-respaldo-{nombre}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn el_respaldo_va_al_lado_del_destino_con_su_nombre_entero() {
        assert_eq!(
            ruta_del_respaldo(Path::new("C:/p/voz-1.m4a.parcial")),
            Path::new("C:/p/voz-1.m4a.parcial.respaldo.wav")
        );
    }

    #[test]
    fn un_respaldo_cerrado_se_lee_entero_y_con_su_cabecera_buena() {
        let dir = carpeta("cerrado");
        let ruta = dir.join("a.wav");
        let mut r = Respaldo::crear(&ruta, 48_000, 1).unwrap();
        let tono: Vec<i16> = (0..48_000).map(|i| (i % 300) as i16 - 150).collect();
        r.escribir(&tono).unwrap();
        r.escribir(&tono[..24_000]).unwrap();
        assert_eq!(r.marcos(), 72_000);
        assert_eq!(r.cerrar().unwrap(), 72_000);

        let w = leer(&ruta).unwrap();
        assert_eq!(w.muestreo, 48_000);
        assert_eq!(w.canales, 1);
        assert_eq!(w.inicio, BYTES_CABECERA);
        assert_eq!(w.marcos, 72_000);
        assert_eq!(w.duracion_ms(), 1_500);
        // La cabecera dice lo que hay (la lee cualquiera).
        let bytes = std::fs::read(&ruta).unwrap();
        assert_eq!(&bytes[40..44], &(144_000u32).to_le_bytes());

        let mut leidas = Vec::new();
        recorrer::<io::Error>(&ruta, &w, 10_000, |m| {
            leidas.extend_from_slice(m);
            Ok(())
        })
        .unwrap();
        assert_eq!(leidas.len(), 72_000);
        assert_eq!(&leidas[..48_000], &tono[..]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn un_respaldo_cortado_sin_cerrar_se_lee_hasta_donde_llego() {
        // **El corte**: la aplicacion muere y nadie llama a `cerrar`. La
        // cabecera sigue diciendo «sin fin» y el ultimo marco puede quedar
        // a medias.
        let dir = carpeta("cortado");
        let ruta = dir.join("b.wav");
        let mut r = Respaldo::crear(&ruta, 48_000, 2).unwrap();
        r.escribir(&vec![7i16; 2 * 4_800]).unwrap();
        r.volcar().unwrap();
        std::mem::forget(r);
        // Un byte suelto de mas, como si se cortara a mitad de una muestra.
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&ruta)
            .unwrap();
        f.write_all(&[1, 2, 3]).unwrap();
        drop(f);

        let w = leer(&ruta).unwrap();
        assert_eq!(w.canales, 2);
        assert_eq!(w.marcos, 4_800, "el marco a medias no cuenta");
        assert_eq!(w.duracion_ms(), 100);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn el_respaldo_se_vuelca_al_disco_cada_pocos_segundos() {
        let dir = carpeta("volcado");
        let ruta = dir.join("c.wav");
        let mut r = Respaldo::crear(&ruta, 8_000, 1).unwrap();
        // Mas de un volcado (5 s a 8 kHz son 40 000 marcos), sin cerrar.
        for _ in 0..50 {
            r.escribir(&[3i16; 1_000]).unwrap();
        }
        // Lo de antes del ultimo volcado ya esta en el fichero aunque el
        // bufer no se vacie nunca.
        let w = leer(&ruta).unwrap();
        assert!(w.marcos >= 40_000, "{w:?}");
        std::mem::forget(r);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn lo_que_no_es_un_wav_de_16_bits_no_se_lee() {
        let dir = carpeta("raro");
        let ruta = dir.join("d.wav");
        std::fs::write(&ruta, b"esto no es un wav de nada").unwrap();
        assert!(leer(&ruta).is_err());
        // Caso negativo: un WAV de 8 bits.
        let mut c = cabecera(8_000, 1, 0).to_vec();
        c[34] = 8;
        std::fs::write(&ruta, &c).unwrap();
        assert!(leer(&ruta).is_err());
        // Y uno vacio pero bueno vale, con cero marcos.
        std::fs::write(&ruta, cabecera(8_000, 1, 0)).unwrap();
        assert_eq!(leer(&ruta).unwrap().marcos, 0);
        assert!(leer(&dir.join("no-esta.wav")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn un_respaldo_que_no_puede_escribir_se_calla_sin_parar_nada() {
        // `/dev/full` siempre dice «disco lleno».
        let ruta = Path::new("/dev/full");
        if !ruta.exists() {
            return;
        }
        let Ok(mut r) = Respaldo::crear(ruta, 8_000, 1) else {
            // Ya la cabecera fallo: tambien vale (quien llama sigue sin el).
            return;
        };
        let mut fallos = 0;
        for _ in 0..200 {
            if r.escribir(&[1i16; 4_000]).is_err() {
                fallos += 1;
            }
        }
        assert_eq!(fallos, 1, "el fallo se dice una vez y luego se calla");
        assert_eq!(r.fallo(), Some(io::ErrorKind::StorageFull));
    }
}
