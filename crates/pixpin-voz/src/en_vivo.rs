//! **Escuchar en vivo mientras se graba una reunion** (28-oct-2026, los
//! temas de la reunion, `docs/jev.md`; en vivo de verdad el 29-oct-2026, el
//! usuario: «en cuanto termino de decir un tema, que se marque»): lo que
//! oye la grabacion se pasa a texto **frase a frase**, de fondo, para que
//! Jev vaya marcando los temas de la agenda que ya se trataron a los pocos
//! segundos de hablarlos.
//!
//! Lo puro, sin sonido ni modelo, para probarlo en milisegundos:
//!
//! - [`Frasero`]: junta los bloques del PCM de la grabacion (los de su
//!   suscripcion, `pixpin_motor_audio::pcm`: i16 intercalado, con su
//!   posicion) y **corta una frase en cuanto se calla** quien habla
//!   ([`Medida::silencio_ms`] sin voz despues de algo dicho), o en el punto
//!   mas callado de sus ultimos [`BUSCAR_CORTE_MS`] si alguien habla sin
//!   parar ([`Medida::maximo_ms`]). Cada frase va en mono y a 16 kHz, con
//!   su numero, el milisegundo de la grabacion en que empieza, el de
//!   cuando se dejo de hablar y cuanto silencio llevaba ya al cortarse (lo
//!   que mide el retraso «se dejo de hablar → ✓»). Lo callado del
//!   principio no se guarda (salvo un poco antes de la voz). En pausa no
//!   llega nada y el reloj no corre, como el de la nota. Un hueco (bloques
//!   que se le tiraron a esta suscripcion por ir tarde) se rellena con
//!   silencio si es corto; si es largo, la frase se cierra ahi.
//!
//!   Dos medidas: [`PARA_WINDOWS`] (el reconocedor de Windows muele una
//!   frase de 3 s en unas decimas: frases cortas) y [`PARA_WHISPER`]
//!   (Whisper cuesta casi lo mismo con 2 que con 30 s, porque su encoder
//!   oye siempre 30: trozos de 5 s o mas, cortados igual en un silencio; y
//!   sin solaparse, para que Jev no lea dos veces lo mismo).
//! - [`Cola`]: **acotada, y tira lo mas viejo**. Si el reconocedor va por
//!   detras, se pierde texto en vivo, nunca grabacion: el hilo de la mezcla
//!   no espera a nadie (la suscripcion ya es asi) y la transcripcion entera
//!   de despues sigue siendo la de siempre.
//! - [`Freno`]: **si una frase se pasa a texto o se salta**. Se salta lo
//!   que no tiene voz, todo mientras el procesador va cargado (≥ 85 %) y,
//!   si la ultima tardo mas del 80 % de lo que dura, la siguiente (para
//!   que el reconocedor no viva pegado al procesador).

use std::collections::VecDeque;
use std::time::Duration;

use crate::HERCIOS;

/// Donde se busca el corte de una frase que no para: en sus ultimos 2 s.
pub const BUSCAR_CORTE_MS: i64 = 2_000;
/// Un hueco de hasta esto se rellena con silencio.
pub const HUECO_MAXIMO_MS: i64 = 1_000;
/// Lo callado que se guarda antes de la primera voz de una frase (para no
/// comerse el principio de la primera palabra).
pub const ANTES_DE_LA_VOZ_MS: i64 = 300;
/// Lo callado que se deja despues de la ultima voz (el final de la ultima
/// palabra).
pub const DESPUES_DE_LA_VOZ_MS: i64 = 200;
/// Un silencio de esto despues de algo dicho cierra la frase aunque aun no
/// llegue a [`Medida::largo_minimo_ms`] (se acabo el turno: es justo cuando
/// mas importa no esperar). Despues de una tos, la olvida.
pub const SILENCIO_LARGO_MS: i64 = 1_500;
/// Cuantas frases esperan como mucho a pasar a texto.
pub const COLA: usize = 3;
/// Procesador por encima de esto (0-1): no se pasa a texto.
pub const CPU_MAXIMA: f32 = 0.85;
/// Si una frase tardo mas que esta parte de lo que dura, se descansa una.
pub const TARDANZA_MAXIMA: f32 = 0.8;
/// Energia (RMS, en i16) por encima de la cual un trozo de 20 ms tiene voz.
const RMS_VOZ: f64 = 300.0;
/// Que parte de una frase tiene que tener voz para mirarla.
const PARTE_CON_VOZ: f64 = 0.1;

/// **Cuando se corta una frase.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Medida {
    /// La voz (trozos de 20 ms con voz) que tiene que tener para mandarse:
    /// menos es una tos o un golpe.
    pub voz_minima_ms: i64,
    /// Lo que tiene que durar, como poco, para cortarse en un silencio
    /// corto (si el silencio llega a [`SILENCIO_LARGO_MS`], da igual).
    pub largo_minimo_ms: i64,
    /// El silencio despues de la voz que cierra la frase.
    pub silencio_ms: i64,
    /// Lo mas que dura: entonces se corta en lo mas callado del final.
    pub maximo_ms: i64,
}

/// Para el reconocedor de Windows (SAPI): frases de verdad, cortadas en
/// cuanto se calla medio segundo.
pub const PARA_WINDOWS: Medida = Medida {
    voz_minima_ms: 500,
    largo_minimo_ms: 0,
    silencio_ms: 450,
    maximo_ms: 8_000,
};

/// Para Whisper: trozos de 5 s o mas (cada llamada cuesta casi lo mismo),
/// y si no, en cuanto se acaba el turno.
pub const PARA_WHISPER: Medida = Medida {
    voz_minima_ms: 800,
    largo_minimo_ms: 5_000,
    silencio_ms: 500,
    maximo_ms: 12_000,
};

/// Una frase lista para pasar a texto: mono, 16 kHz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frase {
    /// Su numero, desde 0 y seguidos.
    pub n: u64,
    /// El milisegundo de la grabacion en que empieza.
    pub ms: i64,
    /// El milisegundo de la grabacion en que se dejo de hablar (o el del
    /// corte, si se corto hablando).
    pub fin_voz_ms: i64,
    /// Lo que ya se habia oido despues de [`Frase::fin_voz_ms`] cuando se
    /// corto: restado al momento en que sale, da cuando se dejo de hablar.
    pub callado_al_final_ms: i64,
    pub muestras: Vec<i16>,
}

impl Frase {
    /// Lo que dura.
    pub fn duracion(&self) -> Duration {
        Duration::from_millis(self.muestras.len() as u64 * 1000 / u64::from(HERCIOS))
    }
}

/// **Corta el PCM de la grabacion en frases** (ver la cabecera).
#[derive(Debug)]
pub struct Frasero {
    medida: Medida,
    /// Mono, al muestreo de la grabacion.
    buf: Vec<i16>,
    hz: u32,
    /// La posicion (marcos desde el principio) de `buf[0]`.
    inicio: u64,
    /// Donde deberia empezar el siguiente bloque.
    esperado: Option<u64>,
    n: u64,
    /// Hasta donde de `buf` se midio la voz (de 20 en 20 ms).
    mirado: usize,
    /// Los marcos con voz de `buf[..mirado]`.
    con_voz: usize,
    /// Donde acaba el ultimo trozo con voz de `buf[..mirado]`.
    ultima_voz: Option<usize>,
}

fn ms_de(marcos: u64, hz: u32) -> i64 {
    (marcos * 1000 / u64::from(hz.max(1))) as i64
}

fn marcos_de(ms: i64, hz: u32) -> usize {
    (ms.max(0) as u64 * u64::from(hz) / 1000) as usize
}

impl Frasero {
    pub fn nuevo(medida: Medida) -> Frasero {
        Frasero {
            medida,
            buf: Vec::new(),
            hz: 0,
            inicio: 0,
            esperado: None,
            n: 0,
            mirado: 0,
            con_voz: 0,
            ultima_voz: None,
        }
    }

    /// Otra medida desde ahora (el reconocedor cambio: de Windows a
    /// Whisper).
    pub fn poner_medida(&mut self, medida: Medida) {
        self.medida = medida;
    }

    /// **Mete un bloque** (`posicion` en marcos, i16 intercalado con
    /// `canales`). Devuelve las frases que se cerraron (casi siempre
    /// ninguna o una).
    pub fn meter(
        &mut self,
        posicion: u64,
        muestreo: u32,
        canales: u16,
        muestras: &[i16],
    ) -> Vec<Frase> {
        let mut salida = Vec::new();
        if muestreo == 0 || muestras.is_empty() {
            return salida;
        }
        if self.hz != muestreo {
            // No cambia en una grabacion; si cambiara, se empieza de cero.
            salida.extend(self.cerrar());
            self.hz = muestreo;
            self.vaciar();
            self.esperado = None;
        }
        match self.esperado {
            None => {
                self.vaciar();
                self.inicio = posicion;
            }
            Some(e) if posicion > e => {
                let hueco = posicion - e;
                if ms_de(hueco, self.hz) <= HUECO_MAXIMO_MS {
                    self.buf.extend(std::iter::repeat_n(0, hueco as usize));
                } else {
                    salida.extend(self.cerrar());
                    self.vaciar();
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
        self.mirar(&mut salida);
        salida
    }

    /// Mide la voz de lo nuevo, trozo a trozo, y corta donde toque.
    fn mirar(&mut self, salida: &mut Vec<Frase>) {
        let hz = self.hz;
        let paso = (hz as usize / 50).max(1);
        let m = self.medida;
        while self.mirado + paso <= self.buf.len() {
            if rms(&self.buf[self.mirado..self.mirado + paso]) >= RMS_VOZ {
                if self.ultima_voz.is_none() && self.con_voz == 0 {
                    // Empieza a hablar: de lo callado de antes, solo un poco.
                    let sobra = self
                        .mirado
                        .saturating_sub(marcos_de(ANTES_DE_LA_VOZ_MS, hz));
                    self.tirar(sobra);
                }
                self.con_voz += paso;
                self.ultima_voz = Some(self.mirado + paso);
            }
            self.mirado += paso;
            let Some(fin) = self.ultima_voz else {
                // Aun nadie habla: solo se guarda un poco de silencio.
                let guardar = marcos_de(ANTES_DE_LA_VOZ_MS, hz);
                if self.mirado > guardar + marcos_de(1_000, hz) {
                    self.tirar(self.mirado - guardar);
                }
                continue;
            };
            let callado = ms_de((self.mirado - fin) as u64, hz);
            let voz = ms_de(self.con_voz as u64, hz);
            let largo = ms_de(self.mirado as u64, hz);
            if voz >= m.voz_minima_ms
                && callado >= m.silencio_ms
                && (largo >= m.largo_minimo_ms || callado >= SILENCIO_LARGO_MS)
            {
                // Se callo: la frase, con un poco de cola.
                let corte = (fin + marcos_de(DESPUES_DE_LA_VOZ_MS, hz)).min(self.mirado);
                salida.push(self.sacar(corte, fin));
            } else if voz < m.voz_minima_ms && callado >= SILENCIO_LARGO_MS {
                // Una tos y silencio: nada que mandar.
                self.ultima_voz = None;
                self.con_voz = 0;
            } else if largo >= m.maximo_ms {
                // No para de hablar: en lo mas callado del final.
                let desde = self.mirado.saturating_sub(marcos_de(BUSCAR_CORTE_MS, hz));
                let corte = punto_callado(&self.buf, desde, self.mirado, hz).max(paso);
                salida.push(self.sacar(corte, corte));
            }
        }
    }

    /// Tira los primeros `marcos` (silencio) de `buf`.
    fn tirar(&mut self, marcos: usize) {
        let marcos = marcos.min(self.buf.len());
        self.buf.drain(..marcos);
        self.inicio += marcos as u64;
        self.mirado = self.mirado.saturating_sub(marcos);
        self.ultima_voz = self.ultima_voz.map(|f| f.saturating_sub(marcos));
    }

    /// Olvida lo guardado sin cortar (la medida empieza otra vez).
    fn vaciar(&mut self) {
        self.buf.clear();
        self.mirado = 0;
        self.con_voz = 0;
        self.ultima_voz = None;
    }

    /// Saca los primeros `marcos` de `buf` como una frase que dejo de tener
    /// voz en `fin` (un marco de `buf`). Lo que queda se mide otra vez.
    fn sacar(&mut self, marcos: usize, fin: usize) -> Frase {
        let marcos = marcos.min(self.buf.len());
        let fin = fin.min(self.buf.len());
        let f = Frase {
            n: self.n,
            ms: ms_de(self.inicio, self.hz),
            fin_voz_ms: ms_de(self.inicio + fin as u64, self.hz),
            callado_al_final_ms: ms_de((self.buf.len() - fin) as u64, self.hz),
            muestras: a_16k(&self.buf[..marcos], self.hz),
        };
        self.n += 1;
        self.buf.drain(..marcos);
        self.inicio += marcos as u64;
        self.mirado = 0;
        self.con_voz = 0;
        self.ultima_voz = None;
        f
    }

    /// Cierra lo que haya (un hueco largo, el final), si tiene voz bastante.
    fn cerrar(&mut self) -> Option<Frase> {
        if self.hz == 0 || ms_de(self.con_voz as u64, self.hz) < self.medida.voz_minima_ms {
            self.vaciar();
            return None;
        }
        let n = self.buf.len();
        let fin = self.ultima_voz.unwrap_or(n);
        Some(self.sacar(n, fin))
    }

    /// **Al acabar la grabacion**: lo ultimo, si tiene voz bastante.
    pub fn acabar(&mut self) -> Option<Frase> {
        let f = self.cerrar();
        self.esperado = None;
        f
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

/// Si un audio (16 kHz) tiene voz: al menos un 10 % de trozos de 20 ms por
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

/// Que se hace con una frase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paso {
    /// A texto.
    Pasar,
    /// No tiene voz.
    SaltarSilencio,
    /// El procesador va cargado.
    SaltarCpu,
    /// La anterior tardo demasiado: esta se salta para respirar.
    SaltarRetraso,
}

/// **Si una frase se pasa a texto** (ver la cabecera).
#[derive(Debug, Clone, Default)]
pub struct Freno {
    descansar: bool,
}

impl Freno {
    /// `cpu`: lo ocupado del procesador ahora (0-1), si se sabe.
    pub fn decidir(&mut self, frase: &Frase, cpu: Option<f32>) -> Paso {
        if !tiene_voz(&frase.muestras) {
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

    /// Lo que tardo la ultima frase que se paso a texto.
    pub fn tardo(&mut self, frase: Duration, tardanza: Duration) {
        self.descansar = tardanza.as_secs_f32() > TARDANZA_MAXIMA * frase.as_secs_f32();
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

    fn callado(ms: i64, hz: u32, canales: u16) -> Vec<i16> {
        vec![0; marcos_de(ms, hz) * usize::from(canales)]
    }

    /// Mete `pcm` en bloques de 10 ms desde la posicion `desde` y devuelve
    /// lo que salio y por donde va.
    fn en_bloques(
        f: &mut Frasero,
        pcm: &[i16],
        hz: u32,
        canales: u16,
        desde: u64,
    ) -> (Vec<Frase>, u64) {
        let marcos = marcos_de(10, hz);
        let mut pos = desde;
        let mut v = Vec::new();
        for b in pcm.chunks(marcos * usize::from(canales)) {
            v.extend(f.meter(pos, hz, canales, b));
            pos += (b.len() / usize::from(canales)) as u64;
        }
        (v, pos)
    }

    #[test]
    fn corta_la_frase_en_cuanto_se_calla() {
        let hz = 48_000;
        let mut f = Frasero::nuevo(PARA_WINDOWS);
        // 2 s callado, 2 s hablando, 1 s callado, 3 s hablando, 1 s callado.
        let mut pcm = callado(2_000, hz, 2);
        pcm.extend(tono(2_000, hz, 2, 8_000.0));
        pcm.extend(callado(1_000, hz, 2));
        pcm.extend(tono(3_000, hz, 2, 8_000.0));
        pcm.extend(callado(1_000, hz, 2));
        let (v, _) = en_bloques(&mut f, &pcm, hz, 2, 0);
        assert_eq!(
            v.len(),
            2,
            "{:?}",
            v.iter().map(|x| x.ms).collect::<Vec<_>>()
        );
        // La primera empieza un poco antes de la voz (lo callado de antes no
        // va) y acaba cuando se dejo de hablar.
        assert_eq!(v[0].n, 0);
        assert!((1_690..=1_710).contains(&v[0].ms), "{}", v[0].ms);
        assert!(
            (3_990..=4_010).contains(&v[0].fin_voz_ms),
            "{}",
            v[0].fin_voz_ms
        );
        // Se corta a los 450 ms de silencio, no al final del segundo.
        assert!(
            (440..=470).contains(&v[0].callado_al_final_ms),
            "{}",
            v[0].callado_al_final_ms
        );
        let d = v[0].duracion().as_millis() as i64;
        assert!((2_450..=2_550).contains(&d), "{d}");
        // La segunda, con su minuto.
        assert_eq!(v[1].n, 1);
        assert!(
            (7_990..=8_010).contains(&v[1].fin_voz_ms),
            "{}",
            v[1].fin_voz_ms
        );
        assert!(v[1].ms > v[0].fin_voz_ms && v[1].ms < 5_000, "{}", v[1].ms);
        assert!(f.acabar().is_none(), "lo que queda es silencio");
    }

    #[test]
    fn una_tos_no_es_una_frase_y_el_silencio_no_se_guarda() {
        let hz = 16_000;
        let mut f = Frasero::nuevo(PARA_WINDOWS);
        let mut pcm = callado(30_000, hz, 1);
        pcm.extend(tono(200, hz, 1, 8_000.0));
        pcm.extend(callado(3_000, hz, 1));
        let (v, _) = en_bloques(&mut f, &pcm, hz, 1, 0);
        assert!(v.is_empty());
        assert!(f.buf.len() < marcos_de(2_000, hz), "{}", f.buf.len());
        assert!(f.acabar().is_none());
    }

    #[test]
    fn quien_no_para_de_hablar_se_corta_en_lo_mas_callado() {
        let hz = 16_000;
        let mut f = Frasero::nuevo(PARA_WINDOWS);
        // 7 s de voz, 200 ms (menos que el silencio que cierra), 5 s de voz.
        let mut pcm = tono(7_000, hz, 1, 8_000.0);
        pcm.extend(callado(200, hz, 1));
        pcm.extend(tono(5_000, hz, 1, 8_000.0));
        let (v, _) = en_bloques(&mut f, &pcm, hz, 1, 0);
        assert_eq!(v.len(), 1);
        let d = v[0].duracion().as_millis() as i64;
        assert!((7_000..7_200).contains(&d), "{d}");
        let r = f.acabar().unwrap();
        assert_eq!(r.n, 1);
        assert_eq!(r.ms, d);
    }

    #[test]
    fn para_whisper_espera_cinco_segundos_salvo_que_se_acabe_el_turno() {
        let hz = 16_000;
        let mut f = Frasero::nuevo(PARA_WHISPER);
        let mut pcm = tono(2_000, hz, 1, 8_000.0);
        pcm.extend(callado(600, hz, 1));
        pcm.extend(tono(3_000, hz, 1, 8_000.0));
        pcm.extend(callado(600, hz, 1));
        let (v, pos) = en_bloques(&mut f, &pcm, hz, 1, 0);
        // A los 2,6 s aun no (corto); a los 6,2 s si, las dos juntas.
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].ms, 0);
        assert!((5_590..=5_610).contains(&v[0].fin_voz_ms));
        // Un trozo corto y luego un silencio largo: sale igual.
        let mut pcm = tono(1_500, hz, 1, 8_000.0);
        pcm.extend(callado(2_000, hz, 1));
        let (v, _) = en_bloques(&mut f, &pcm, hz, 1, pos);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].callado_al_final_ms, SILENCIO_LARGO_MS);
    }

    #[test]
    fn un_hueco_corto_es_silencio_y_uno_largo_cierra_la_frase() {
        let hz = 16_000;
        let mut f = Frasero::nuevo(PARA_WINDOWS);
        let s3 = tono(3_000, hz, 1, 8_000.0);
        assert!(f.meter(0, hz, 1, &s3).is_empty());
        // Un hueco de 10 s: lo que habia sale como frase.
        let pos = 3_000 * 16 + 160_000;
        let v = f.meter(pos, hz, 1, &s3);
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].ms, v[0].fin_voz_ms), (0, 3_000));
        // Y lo nuevo empieza en su minuto.
        let r = f.acabar().unwrap();
        assert_eq!(r.ms, 13_000);
        // Un bloque repetido (solapado) no se cuenta dos veces.
        let mut f = Frasero::nuevo(PARA_WINDOWS);
        f.meter(0, hz, 1, &s3);
        f.meter(0, hz, 1, &s3);
        assert_eq!(f.acabar().unwrap().duracion().as_millis(), 3_000);
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
        let voz = Frase {
            n: 0,
            ms: 0,
            fin_voz_ms: 3_000,
            callado_al_final_ms: 450,
            muestras: tono(3_000, 16_000, 1, 6_000.0),
        };
        let callado = Frase {
            muestras: tono(3_000, 16_000, 1, 50.0),
            ..voz.clone()
        };
        let mut f = Freno::default();
        assert_eq!(f.decidir(&callado, Some(0.1)), Paso::SaltarSilencio);
        assert_eq!(f.decidir(&voz, Some(0.9)), Paso::SaltarCpu);
        assert_eq!(f.decidir(&voz, None), Paso::Pasar);
        // Tardo 2,6 s en 3: se descansa una, y solo una.
        f.tardo(Duration::from_secs(3), Duration::from_millis(2_600));
        assert_eq!(f.decidir(&voz, Some(0.2)), Paso::SaltarRetraso);
        assert_eq!(f.decidir(&voz, Some(0.2)), Paso::Pasar);
        f.tardo(Duration::from_secs(3), Duration::from_millis(300));
        assert_eq!(f.decidir(&voz, Some(0.2)), Paso::Pasar);
    }
}
