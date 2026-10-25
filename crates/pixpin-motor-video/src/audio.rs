//! **El contrato con el motor de audio**.
//!
//! El sonido lo graba otro motor (hoy `pixpin_audio::Grabadora`; despues el
//! motor de audio nuevo). Este crate no depende de ninguno: el contrato es
//! esto, y cualquiera que lo cumpla se enchufa.
//!
//! - El sonido llega en **bloques de PCM de 16 bits intercalados**, a un
//!   muestreo fijo (44 100 o 48 000, lo que admite el AAC de Windows) y con
//!   uno o dos canales ([`FormatoAudio`]).
//! - Cada bloque trae su **posicion**: las muestras por canal desde el
//!   principio. En pausa no llegan bloques y la posicion no avanza: es el
//!   reloj de la grabacion (ver [`crate::reloj`]).
//! - Quien lo graba lo empuja con `MotorVideo::poner_audio`, o lo deja
//!   como una [`FuenteAudio`] para que alguien lo vaya sacando.
//!
//! [`CopiaMono`] es el adaptador de lo de hoy: la copia de la nota de voz
//! (`pixpin_audio::Copia`, un canal de bloques mono sin posicion), contando
//! las muestras para ponerles la suya.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

/// Bytes por segundo del AAC: 96 kbps, los mismos que la nota de voz. El
/// codificador de Windows solo admite 12 000, 16 000, 20 000 o 24 000; para
/// una reunion la voz no gana nada por encima.
pub const BYTES_POR_SEGUNDO_AAC: u32 = 12_000;

/// Como viene el sonido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatoAudio {
    /// Muestras por segundo: 44 100 o 48 000.
    pub muestreo: u32,
    /// Uno o dos.
    pub canales: u16,
}

impl FormatoAudio {
    pub fn mono(muestreo: u32) -> FormatoAudio {
        FormatoAudio {
            muestreo,
            canales: 1,
        }
    }

    /// Si el AAC de Windows lo admite.
    pub fn validar(&self) -> Result<(), &'static str> {
        if !pixpin_record::escala::muestreo_aac_valido(self.muestreo) {
            return Err("el AAC solo admite 44 100 o 48 000 muestras por segundo");
        }
        if !(1..=2).contains(&self.canales) {
            return Err("el sonido ha de ser mono o estereo");
        }
        Ok(())
    }

    /// Cuantas muestras por canal son `intercaladas` muestras sueltas.
    pub fn por_canal(&self, intercaladas: usize) -> u64 {
        (intercaladas / usize::from(self.canales.max(1))) as u64
    }

    /// Bytes de cada marco (todas las muestras de un instante).
    pub fn bytes_por_marco(&self) -> u32 {
        2 * u32::from(self.canales.max(1))
    }
}

/// Lo que da una [`FuenteAudio`] al pedirle sonido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Siguiente {
    /// Un bloque intercalado que empieza en `posicion` (por canal).
    Bloque { muestras: Vec<i16>, posicion: u64 },
    /// Nada en el tiempo que se espero (pausa, o todavia no).
    Nada,
    /// Ya no va a llegar mas: la grabacion termino.
    Fin,
}

/// **Un motor de audio visto desde el video**: de donde sale el sonido.
pub trait FuenteAudio: Send {
    fn formato(&self) -> FormatoAudio;
    /// Espera como mucho `espera` al siguiente bloque.
    fn siguiente(&mut self, espera: Duration) -> Siguiente;
}

/// **La copia de la nota de voz como fuente**: bloques mono a `muestreo`
/// por un canal que se cierra al acabar (`pixpin_audio::Copia`). La
/// posicion de cada bloque es lo recibido hasta entonces: en pausa no llega
/// nada, asi que contar es el mismo reloj que el de la nota.
pub struct CopiaMono {
    rx: Receiver<Vec<i16>>,
    muestreo: u32,
    llevadas: u64,
}

impl CopiaMono {
    pub fn nueva(rx: Receiver<Vec<i16>>, muestreo: u32) -> CopiaMono {
        CopiaMono {
            rx,
            muestreo,
            llevadas: 0,
        }
    }
}

impl FuenteAudio for CopiaMono {
    fn formato(&self) -> FormatoAudio {
        FormatoAudio::mono(self.muestreo)
    }

    fn siguiente(&mut self, espera: Duration) -> Siguiente {
        match self.rx.recv_timeout(espera) {
            Ok(muestras) => {
                let posicion = self.llevadas;
                self.llevadas += muestras.len() as u64;
                Siguiente::Bloque { muestras, posicion }
            }
            Err(RecvTimeoutError::Timeout) => Siguiente::Nada,
            Err(RecvTimeoutError::Disconnected) => Siguiente::Fin,
        }
    }
}

/// **La pista de sonido nunca va hacia atras**: lo que ya se escribio no se
/// vuelve a escribir. Si el motor de audio entrega un bloque que pisa el
/// anterior (un reinicio de su dispositivo, un bloque repetido), el MP4
/// tendria dos muestras en el mismo instante y hay escritores que lo
/// rechazan y reproductores que se atascan. Se recorta lo que pisa; un
/// hueco (se salto sonido) se deja como esta: lo rellena el reproductor.
/// Lo mismo que hace `windows-capture` al fechar el sonido con su propia
/// cuenta en vez de la hora que le dan (`VideoEncoder::send_audio_buffer`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pista {
    /// Donde acaba lo escrito, en muestras por canal.
    fin: u64,
}

impl Pista {
    /// Para un bloque de `marcos` muestras por canal que dice empezar en
    /// `posicion`: cuantos marcos del principio hay que saltar y donde
    /// empieza lo que queda, o `None` si ya estaba todo escrito.
    pub fn admitir(&mut self, posicion: u64, marcos: u64) -> Option<(u64, u64)> {
        let fin = posicion.saturating_add(marcos);
        if marcos == 0 || fin <= self.fin {
            return None;
        }
        let salto = self.fin.saturating_sub(posicion);
        self.fin = fin;
        Some((salto, posicion + salto))
    }

    pub fn fin(&self) -> u64 {
        self.fin
    }
}

/// **Un bloque intercalado pasado a mono** (la media de los canales), para
/// el camino por la CPU, cuyo codificador solo lleva un canal. Mono ya,
/// tal cual.
pub fn a_mono(intercaladas: &[i16], canales: u16) -> Vec<i16> {
    let n = usize::from(canales.max(1));
    if n == 1 {
        return intercaladas.to_vec();
    }
    intercaladas
        .chunks_exact(n)
        .map(|marco| {
            let suma: i32 = marco.iter().map(|&m| i32::from(m)).sum();
            (suma / n as i32) as i16
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn la_pista_de_sonido_no_se_pisa() {
        let mut p = Pista::default();
        assert_eq!(p.admitir(0, 480), Some((0, 0)));
        assert_eq!(p.admitir(480, 480), Some((0, 480)));
        // Un bloque que vuelve 100 muestras atras: se recortan esas.
        assert_eq!(p.admitir(860, 480), Some((100, 960)));
        assert_eq!(p.fin(), 1340);
        // Un hueco se deja como esta.
        assert_eq!(p.admitir(2000, 10), Some((0, 2000)));
        // Caso negativo: un bloque repetido entero, o vacio, no entra.
        assert_eq!(p.admitir(2000, 10), None);
        assert_eq!(p.admitir(5000, 0), None);
        assert_eq!(p.fin(), 2010);
    }

    #[test]
    fn solo_valen_los_formatos_del_aac() {
        assert!(FormatoAudio::mono(48_000).validar().is_ok());
        assert!(
            FormatoAudio {
                muestreo: 44_100,
                canales: 2
            }
            .validar()
            .is_ok()
        );
        // Casos negativos.
        assert!(FormatoAudio::mono(16_000).validar().is_err());
        assert!(
            FormatoAudio {
                muestreo: 48_000,
                canales: 6
            }
            .validar()
            .is_err()
        );
        assert!(
            FormatoAudio {
                muestreo: 48_000,
                canales: 0
            }
            .validar()
            .is_err()
        );
    }

    #[test]
    fn las_cuentas_del_formato() {
        let estereo = FormatoAudio {
            muestreo: 48_000,
            canales: 2,
        };
        assert_eq!(estereo.por_canal(960), 480);
        assert_eq!(estereo.bytes_por_marco(), 4);
        assert_eq!(FormatoAudio::mono(48_000).por_canal(480), 480);
    }

    #[test]
    fn la_copia_mono_pone_la_posicion_contando() {
        let (tx, rx) = channel();
        let mut f = CopiaMono::nueva(rx, 48_000);
        assert_eq!(f.formato(), FormatoAudio::mono(48_000));
        tx.send(vec![1i16; 480]).unwrap();
        tx.send(vec![2i16; 240]).unwrap();
        assert_eq!(
            f.siguiente(Duration::from_millis(10)),
            Siguiente::Bloque {
                muestras: vec![1; 480],
                posicion: 0
            }
        );
        assert_eq!(
            f.siguiente(Duration::from_millis(10)),
            Siguiente::Bloque {
                muestras: vec![2; 240],
                posicion: 480
            }
        );
        // Nada en camino: espera y dice que nada.
        assert_eq!(f.siguiente(Duration::from_millis(5)), Siguiente::Nada);
        // Caso negativo: el canal cerrado es el fin, no «nada».
        drop(tx);
        assert_eq!(f.siguiente(Duration::from_millis(5)), Siguiente::Fin);
    }

    #[test]
    fn pasar_a_mono_hace_la_media() {
        assert_eq!(a_mono(&[100, 200, -50, 50], 2), vec![150, 0]);
        // Sin desbordar en los extremos.
        assert_eq!(a_mono(&[i16::MAX, i16::MAX], 2), vec![i16::MAX]);
        assert_eq!(a_mono(&[i16::MIN, i16::MIN], 2), vec![i16::MIN]);
        // Mono, tal cual; y un marco a medias se descarta.
        assert_eq!(a_mono(&[1, 2, 3], 1), vec![1, 2, 3]);
        assert_eq!(a_mono(&[1, 2, 3], 2), vec![1]);
    }
}
