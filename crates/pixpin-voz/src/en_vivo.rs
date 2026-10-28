//! **Escuchar en vivo mientras se graba una reunion** (28-oct-2026, los
//! temas de la reunion, `docs/jev.md`): lo que oye la grabacion se pasa a
//! texto por tramos de unos 15 s, de fondo, para que Jev vaya marcando los
//! temas de la agenda que ya se trataron.
//!
//! Lo puro, sin sonido ni modelo, para probarlo en milisegundos:
//!
//! - [`Ventanero`]: junta los bloques del PCM de la grabacion (los de su
//!   suscripcion, `pixpin_motor_audio::pcm`: i16 intercalado, con su
//!   posicion) en tramos de [`TRAMO_MS`], cortados en el punto mas callado
//!   de sus ultimos [`BUSCAR_CORTE_MS`] (para no partir una palabra), en
//!   mono y a 16 kHz, cada uno con su numero y el milisegundo de la
//!   grabacion en que empieza. En pausa no llega nada y el reloj no corre,
//!   como el de la nota. Un hueco (bloques que se le tiraron a esta
//!   suscripcion por ir tarde) se rellena con silencio si es corto; si es
//!   largo, el tramo se cierra ahi.
//! - [`Cola`]: **acotada, y tira lo mas viejo**. Si el reconocedor va por
//!   detras, se pierde texto en vivo, nunca grabacion: el hilo de la mezcla
//!   no espera a nadie (la suscripcion ya es asi) y la transcripcion entera
//!   de despues sigue siendo la de siempre.
//! - [`Freno`]: **si un tramo se pasa a texto o se salta**. Se salta lo
//!   que no tiene voz, todo mientras el procesador va cargado (≥ 85 %) y,
//!   si el ultimo tramo tardo mas del 80 % de lo que dura, el siguiente
//!   (para que el reconocedor no viva pegado al procesador).

use std::collections::VecDeque;
use std::time::Duration;

use crate::HERCIOS;

/// Lo que dura un tramo (como las tandas de Whisper, [`crate::cortes`]).
pub const TRAMO_MS: i64 = 15_000;
/// Donde se busca el corte: en los ultimos 3 s del tramo.
pub const BUSCAR_CORTE_MS: i64 = 3_000;
/// Un tramo que se cierra por un hueco o al final tiene que durar esto para
/// mandarse (lo mas corto no dice nada).
pub const TRAMO_MINIMO_MS: i64 = 3_000;
/// Un hueco de hasta esto se rellena con silencio.
pub const HUECO_MAXIMO_MS: i64 = 1_000;
/// Cuantos tramos esperan como mucho a pasar a texto.
pub const COLA: usize = 2;
/// Procesador por encima de esto (0-1): no se pasa a texto.
pub const CPU_MAXIMA: f32 = 0.85;
/// Si un tramo tardo mas que esta parte de lo que dura, se descansa uno.
pub const TARDANZA_MAXIMA: f32 = 0.8;
/// Energia (RMS, en i16) por encima de la cual un trozo de 20 ms tiene voz.
const RMS_VOZ: f64 = 300.0;
/// Que parte del tramo tiene que tener voz para mirarlo.
const PARTE_CON_VOZ: f64 = 0.1;

/// Un tramo listo para pasar a texto: mono, 16 kHz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tramo {
    /// Su numero, desde 0 y seguidos.
    pub n: u64,
    /// El milisegundo de la grabacion en que empieza.
    pub ms: i64,
    pub muestras: Vec<i16>,
}

impl Tramo {
    /// Lo que dura.
    pub fn duracion(&self) -> Duration {
        Duration::from_millis(self.muestras.len() as u64 * 1000 / u64::from(HERCIOS))
    }
}

/// **Junta el PCM de la grabacion en tramos.**
#[derive(Debug, Default)]
pub struct Ventanero {
    /// Mono, al muestreo de la grabacion.
    buf: Vec<i16>,
    hz: u32,
    /// La posicion (marcos desde el principio) de `buf[0]`.
    inicio: u64,
    /// Donde deberia empezar el siguiente bloque.
    esperado: Option<u64>,
    n: u64,
}

fn ms_de(marcos: u64, hz: u32) -> i64 {
    (marcos * 1000 / u64::from(hz.max(1))) as i64
}

fn marcos_de(ms: i64, hz: u32) -> usize {
    (ms.max(0) as u64 * u64::from(hz) / 1000) as usize
}

impl Ventanero {
    pub fn nuevo() -> Ventanero {
        Ventanero::default()
    }

    /// **Mete un bloque** (`posicion` en marcos, i16 intercalado con
    /// `canales`). Devuelve los tramos que se cerraron (casi siempre ninguno
    /// o uno).
    pub fn meter(
        &mut self,
        posicion: u64,
        muestreo: u32,
        canales: u16,
        muestras: &[i16],
    ) -> Vec<Tramo> {
        let mut salida = Vec::new();
        if muestreo == 0 || muestras.is_empty() {
            return salida;
        }
        if self.hz != muestreo {
            // No cambia en una grabacion; si cambiara, se empieza de cero.
            salida.extend(self.cerrar());
            self.hz = muestreo;
            self.buf.clear();
            self.esperado = None;
        }
        match self.esperado {
            None => {
                self.inicio = posicion;
                self.buf.clear();
            }
            Some(e) if posicion > e => {
                let hueco = posicion - e;
                if ms_de(hueco, self.hz) <= HUECO_MAXIMO_MS {
                    self.buf.extend(std::iter::repeat_n(0, hueco as usize));
                } else {
                    salida.extend(self.cerrar());
                    self.inicio = posicion;
                }
            }
            // Un bloque que se solapa con lo ya visto: lo nuevo, si algo.
            Some(e) if posicion < e => {
                let saltar = ((e - posicion) as usize) * usize::from(canales.max(1));
                if saltar >= muestras.len() {
                    return salida;
                }
                return self.meter(e, muestreo, canales, &muestras[saltar..]);
            }
            Some(_) => {}
        }
        let c = usize::from(canales.max(1));
        self.buf.extend(muestras.chunks(c).map(|m| {
            let s: i32 = m.iter().map(|x| i32::from(*x)).sum();
            (s / m.len() as i32) as i16
        }));
        self.esperado = Some(posicion + (muestras.len() / c) as u64);
        let tramo = marcos_de(TRAMO_MS, self.hz);
        while self.buf.len() >= tramo {
            let desde = tramo - marcos_de(BUSCAR_CORTE_MS, self.hz);
            let corte = punto_callado(&self.buf, desde, tramo, self.hz);
            salida.push(self.sacar(corte));
        }
        salida
    }

    /// Saca los primeros `marcos` de `buf` como un tramo.
    fn sacar(&mut self, marcos: usize) -> Tramo {
        let marcos = marcos.min(self.buf.len());
        let t = Tramo {
            n: self.n,
            ms: ms_de(self.inicio, self.hz),
            muestras: a_16k(&self.buf[..marcos], self.hz),
        };
        self.n += 1;
        self.buf.drain(..marcos);
        self.inicio += marcos as u64;
        t
    }

    /// Cierra lo que haya (un hueco largo, el final), si da para un tramo.
    fn cerrar(&mut self) -> Option<Tramo> {
        if self.hz == 0 || ms_de(self.buf.len() as u64, self.hz) < TRAMO_MINIMO_MS {
            self.buf.clear();
            return None;
        }
        let n = self.buf.len();
        Some(self.sacar(n))
    }

    /// **Al acabar la grabacion**: el ultimo trozo, si da para un tramo.
    pub fn acabar(&mut self) -> Option<Tramo> {
        let t = self.cerrar();
        self.esperado = None;
        t
    }
}

/// **El punto mas callado** de `pcm[desde..hasta]`, medido cada 20 ms: el
/// principio del trozo de menos energia. Sin sitio, `hasta`.
pub fn punto_callado(pcm: &[i16], desde: usize, hasta: usize, hz: u32) -> usize {
    let hasta = hasta.min(pcm.len());
    let paso = (hz as usize / 50).max(1);
    let mut mejor = (hasta, f64::MAX);
    let mut i = desde;
    while i + paso <= hasta {
        let e = rms(&pcm[i..i + paso]);
        if e < mejor.1 {
            mejor = (i, e);
        }
        i += paso;
    }
    mejor.0
}

fn rms(v: &[i16]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let s: f64 = v.iter().map(|x| f64::from(*x) * f64::from(*x)).sum();
    (s / v.len() as f64).sqrt()
}

/// **De `hz` a 16 kHz**, mono: cada muestra de salida es la media de las de
/// entrada que le tocan (un filtro de caja: basta para la voz y evita el
/// aliasing de quedarse con una de cada tres).
pub fn a_16k(pcm: &[i16], hz: u32) -> Vec<i16> {
    if hz == HERCIOS || hz == 0 {
        return pcm.to_vec();
    }
    let paso = f64::from(hz) / f64::from(HERCIOS);
    let n = (pcm.len() as f64 / paso).floor() as usize;
    (0..n)
        .map(|k| {
            let a = (k as f64 * paso).floor() as usize;
            let b = (((k + 1) as f64 * paso).floor() as usize)
                .clamp(a + 1, pcm.len())
                .max(a + 1);
            let trozo = &pcm[a.min(pcm.len() - 1)..b.min(pcm.len())];
            let s: i64 = trozo.iter().map(|x| i64::from(*x)).sum();
            (s / trozo.len().max(1) as i64) as i16
        })
        .collect()
}

/// Si un tramo (16 kHz) tiene voz: al menos un 10 % de trozos de 20 ms por
/// encima de un umbral fijo (el ruido de una sala callada queda debajo).
pub fn tiene_voz(pcm: &[i16]) -> bool {
    let paso = HERCIOS as usize / 50;
    let trozos: Vec<&[i16]> = pcm.chunks(paso).filter(|c| c.len() == paso).collect();
    if trozos.is_empty() {
        return false;
    }
    let con_voz = trozos.iter().filter(|c| rms(c) >= RMS_VOZ).count();
    con_voz as f64 >= PARTE_CON_VOZ * trozos.len() as f64
}

/// **Una cola acotada que tira lo mas viejo.**
#[derive(Debug)]
pub struct Cola<T> {
    v: VecDeque<T>,
    capacidad: usize,
    tirados: u64,
}

impl<T> Cola<T> {
    pub fn nueva(capacidad: usize) -> Cola<T> {
        Cola {
            v: VecDeque::new(),
            capacidad: capacidad.max(1),
            tirados: 0,
        }
    }

    /// Mete al final; si ya estaba llena, tira el primero y lo devuelve.
    pub fn meter(&mut self, x: T) -> Option<T> {
        let tirado = if self.v.len() >= self.capacidad {
            self.tirados += 1;
            self.v.pop_front()
        } else {
            None
        };
        self.v.push_back(x);
        tirado
    }

    pub fn sacar(&mut self) -> Option<T> {
        self.v.pop_front()
    }

    pub fn len(&self) -> usize {
        self.v.len()
    }

    pub fn is_empty(&self) -> bool {
        self.v.is_empty()
    }

    /// Cuantos se tiraron por ir tarde.
    pub fn tirados(&self) -> u64 {
        self.tirados
    }
}

/// Que se hace con un tramo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paso {
    /// A texto.
    Pasar,
    /// No tiene voz.
    SaltarSilencio,
    /// El procesador va cargado.
    SaltarCpu,
    /// El anterior tardo demasiado: este se salta para respirar.
    SaltarRetraso,
}

/// **Si un tramo se pasa a texto** (ver la cabecera).
#[derive(Debug, Clone, Default)]
pub struct Freno {
    descansar: bool,
}

impl Freno {
    /// `cpu`: lo ocupado del procesador ahora (0-1), si se sabe.
    pub fn decidir(&mut self, tramo: &Tramo, cpu: Option<f32>) -> Paso {
        if !tiene_voz(&tramo.muestras) {
            return Paso::SaltarSilencio;
        }
        if cpu.is_some_and(|c| c >= CPU_MAXIMA) {
            return Paso::SaltarCpu;
        }
        if std::mem::take(&mut self.descansar) {
            return Paso::SaltarRetraso;
        }
        Paso::Pasar
    }

    /// Lo que tardo el ultimo tramo que se paso a texto.
    pub fn tardo(&mut self, tramo: Duration, tardanza: Duration) {
        self.descansar = tardanza.as_secs_f32() > TARDANZA_MAXIMA * tramo.as_secs_f32();
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un seno de `ms` a `hz`, con `canales` iguales, de amplitud `a`.
    fn tono(ms: i64, hz: u32, canales: u16, a: f64) -> Vec<i16> {
        let n = marcos_de(ms, hz);
        (0..n)
            .flat_map(|i| {
                let s =
                    (a * (i as f64 * 440.0 * std::f64::consts::TAU / f64::from(hz)).sin()) as i16;
                std::iter::repeat_n(s, usize::from(canales))
            })
            .collect()
    }

    #[test]
    fn junta_tramos_de_quince_segundos_con_su_minuto() {
        let mut v = Ventanero::nuevo();
        let hz = 48_000;
        let mut tramos = Vec::new();
        // 40 s en bloques de 10 ms, estereo.
        let bloque = tono(10, hz, 2, 8_000.0);
        let marcos = (bloque.len() / 2) as u64;
        for i in 0..4_000u64 {
            tramos.extend(v.meter(i * marcos, hz, 2, &bloque));
        }
        // Un tono parejo se corta en cualquier punto de los ultimos 3 s:
        // de 12 a 15 s cada tramo, seguidos y con su minuto.
        assert!((2..=3).contains(&tramos.len()), "{}", tramos.len());
        let mut ms = 0;
        for (i, t) in tramos.iter().enumerate() {
            assert_eq!(t.n, i as u64);
            assert_eq!(t.ms, ms);
            let d = t.duracion().as_millis() as i64;
            assert!((12_000..=15_000).contains(&d), "{d}");
            ms += d;
        }
        // Al acabar, lo que queda (≥ 3 s).
        let ultimo = v.acabar().unwrap();
        assert_eq!(ultimo.n, tramos.len() as u64);
        assert_eq!(ultimo.ms, ms);
        assert!(v.acabar().is_none());
    }

    #[test]
    fn corta_en_el_silencio_y_no_en_mitad_de_una_palabra() {
        let hz = 16_000;
        let mut pcm = tono(13_000, hz, 1, 8_000.0);
        pcm.extend(vec![0i16; marcos_de(400, hz)]);
        pcm.extend(tono(5_000, hz, 1, 8_000.0));
        let mut v = Ventanero::nuevo();
        let t = v.meter(0, hz, 1, &pcm);
        assert_eq!(t.len(), 1);
        let d = t[0].duracion().as_millis() as i64;
        assert!((13_000..13_400).contains(&d), "{d}");
    }

    #[test]
    fn un_hueco_corto_es_silencio_y_uno_largo_cierra_el_tramo() {
        let hz = 16_000;
        let mut v = Ventanero::nuevo();
        let s5 = tono(5_000, hz, 1, 8_000.0);
        assert!(v.meter(0, hz, 1, &s5).is_empty());
        // Medio segundo perdido: se rellena; el reloj sigue cuadrando.
        let pos = 5_500 * 16;
        assert!(v.meter(pos, hz, 1, &s5).is_empty());
        // Un hueco de 10 s: lo que habia (10,5 s) sale como tramo.
        let t = v.meter(pos + 80_000 + 160_000, hz, 1, &s5);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].ms, 0);
        assert_eq!(t[0].duracion().as_millis(), 10_500);
        // Y lo nuevo empieza en su minuto.
        let r = v.acabar().unwrap();
        assert_eq!(r.ms, 20_500);
        // Un bloque repetido (solapado) no se cuenta dos veces.
        let mut v = Ventanero::nuevo();
        v.meter(0, hz, 1, &s5);
        v.meter(0, hz, 1, &s5);
        assert_eq!(v.acabar().unwrap().duracion().as_millis(), 5_000);
        // Lo que no llega a 3 s al final no se manda.
        let mut v = Ventanero::nuevo();
        v.meter(0, hz, 1, &tono(2_000, hz, 1, 8_000.0));
        assert!(v.acabar().is_none());
    }

    #[test]
    fn de_44100_a_16000_sin_perder_el_reloj() {
        let pcm = vec![1000i16; 44_100];
        let r = a_16k(&pcm, 44_100);
        assert_eq!(r.len(), 16_000);
        assert!(r.iter().all(|x| *x == 1000));
        let r = a_16k(&vec![0i16; 48_000], 48_000);
        assert_eq!(r.len(), 16_000);
        assert_eq!(a_16k(&[5, 6], 16_000), vec![5, 6]);
    }

    #[test]
    fn la_cola_tira_lo_mas_viejo() {
        let mut c = Cola::nueva(2);
        assert_eq!(c.meter(1), None);
        assert_eq!(c.meter(2), None);
        assert_eq!(c.meter(3), Some(1));
        assert_eq!(c.tirados(), 1);
        assert_eq!(c.sacar(), Some(2));
        assert_eq!(c.sacar(), Some(3));
        assert!(c.is_empty());
    }

    #[test]
    fn el_freno() {
        let voz = Tramo {
            n: 0,
            ms: 0,
            muestras: tono(15_000, 16_000, 1, 6_000.0),
        };
        let callado = Tramo {
            muestras: tono(15_000, 16_000, 1, 50.0),
            ..voz.clone()
        };
        let mut f = Freno::default();
        assert_eq!(f.decidir(&callado, Some(0.1)), Paso::SaltarSilencio);
        assert_eq!(f.decidir(&voz, Some(0.9)), Paso::SaltarCpu);
        assert_eq!(f.decidir(&voz, None), Paso::Pasar);
        // Tardo 13 s en 15: se descansa uno, y solo uno.
        f.tardo(Duration::from_secs(15), Duration::from_secs(13));
        assert_eq!(f.decidir(&voz, Some(0.2)), Paso::SaltarRetraso);
        assert_eq!(f.decidir(&voz, Some(0.2)), Paso::Pasar);
        f.tardo(Duration::from_secs(15), Duration::from_secs(3));
        assert_eq!(f.decidir(&voz, Some(0.2)), Paso::Pasar);
    }
}
