//! **Realzar la voz al grabar**: que una nota dicha alto y claro no salga
//! con un hilo de voz (el usuario, 2-oct: «lo capta muy bajo de volumen»).
//!
//! ## Por que hace falta aqui y no en el movil
//!
//! El movil graba con `MediaRecorder.AudioSource.MIC` (`pin/Voz.kt`) y no
//! anade nada encima: ni `AutomaticGainControl`, ni `NoiseSuppressor`, ni
//! ganancia propia. No le hace falta porque el fabricante ya afina la
//! ganancia del microfono del telefono para voz a un palmo de la boca. En el
//! PC no hay nadie que haga eso: se graba del aparato de captura por defecto
//! de la consola (`eCapture, eConsole`, modo compartido) **tal como lo
//! entrega la mesa de Windows**, y un microfono de portatil a medio metro, o
//! unos cascos con el nivel de Windows al 50 %, dan picos de -30 a -40 dBFS.
//! Al oirlo despues al volumen de siempre, la nota «no se oye».
//!
//! ## Que se hace, y en que orden
//!
//! Sobre las muestras mono de 16 bits, **antes** de los picos y del
//! codificador AAC (asi la onda del chat dice lo mismo que suena):
//!
//! 1. **Paso alto a 80 Hz** (un biquad de Butterworth): quita el retumbe de
//!    la mesa, el zumbido grave y la continua. Sin esto, la continua de
//!    algunos microfonos USB se comeria la mitad del margen de la ganancia.
//! 2. **Control automatico de ganancia**: una envolvente con ataque de 10 ms
//!    y suelta de 300 ms, y la ganancia que lleva esos picos a -3 dBFS,
//!    entre +0 y +24 dB. **Nunca baja de 0 dB**: lo que ya entra alto se
//!    deja como esta, y de eso se encarga el limitador.
//! 3. **Puerta de ruido**: por debajo de -45 dBFS no es voz, es el
//!    ventilador. Ahi la ganancia no sube (eso seria «bombear» el silencio
//!    hasta oirlo como un soplido); vuelve despacio hacia 0 dB, para que una
//!    pausa corta entre frases no deje la siguiente palabra sin ganancia.
//! 4. **Limitador suave**: por encima de -1,4 dBFS se dobla con una tangente
//!    hiperbolica hacia el techo sin llegar nunca a el. Una consonante que
//!    pilla al control a destiempo (el ataque tarda 10 ms) se redondea en
//!    vez de recortarse en seco, que es lo que suena a «roto».
//!
//! ## Lo que no se hace: normalizar al guardar
//!
//! El `.m4a` se escribe **mientras se graba**, trozo a trozo, por el
//! codificador AAC del sistema. Normalizar al final obligaria a descodificar
//! y volver a codificar la nota entera al pararla —segundos de espera en
//! una de diez minutos, y una segunda perdida de calidad—. No hace falta:
//! con el control apuntando a -3 dBFS y el limitador por encima, los picos
//! de la voz ya quedan donde los dejaria esa normalizacion.
//!
//! Todo es aritmetica pura sobre un `&mut [i16]`, **sin reservar memoria**:
//! corre en el hilo que vacia la tarjeta, cada pocos milisegundos, y no
//! cambia ni el muestreo ni el formato de lo que entra.

/// La frecuencia de corte del paso alto, en Hz.
pub const CORTE_HZ: f32 = 80.0;
/// El ataque de la envolvente, en segundos.
pub const ATAQUE_S: f32 = 0.010;
/// La suelta de la envolvente, en segundos.
pub const SUELTA_S: f32 = 0.300;
/// A donde se llevan los picos de la voz: -3 dBFS.
pub const OBJETIVO: f32 = 0.707_946;
/// Lo mas que se sube: +24 dB.
pub const GANANCIA_MAXIMA: f32 = 15.848_932;
/// Lo menos: 0 dB. Bajar lo que entra alto es cosa del limitador.
pub const GANANCIA_MINIMA: f32 = 1.0;
/// Por debajo de esto (-45 dBFS) no es voz y la ganancia no sube.
pub const PISO: f32 = 0.005_623;
/// Desde donde dobla el limitador: -1,4 dBFS.
pub const UMBRAL_LIMITADOR: f32 = 0.85;
/// Lo que tarda la ganancia en volver hacia 0 dB en silencio, en segundos.
const VUELTA_EN_SILENCIO_S: f32 = 1.5;

/// El coeficiente de un suavizado de una sola pole con constante de tiempo
/// `segundos` a `muestreo` muestras por segundo.
fn coeficiente(segundos: f32, muestreo: u32) -> f32 {
    let n = segundos * muestreo.max(1) as f32;
    if n <= 0.0 {
        return 1.0;
    }
    1.0 - (-1.0 / n).exp()
}

/// El limitador suave: lineal hasta [`UMBRAL_LIMITADOR`] y, por encima, una
/// curva que se acerca a 1 sin tocarlo. Simetrico.
pub fn limitar(x: f32) -> f32 {
    let a = x.abs();
    if a <= UMBRAL_LIMITADOR {
        return x;
    }
    let margen = 1.0 - UMBRAL_LIMITADOR;
    let doblado = UMBRAL_LIMITADOR + margen * ((a - UMBRAL_LIMITADOR) / margen).tanh();
    // Por si la tangente llega a 1,0 en coma flotante con una entrada
    // enorme: el techo se queda un pelo por debajo.
    doblado.min(0.999_9).copysign(x)
}

/// El realce de una grabacion: el estado de los filtros entre un trozo y el
/// siguiente. Uno por grabacion, al muestreo con que se escribe.
#[derive(Debug, Clone)]
pub struct Realce {
    // El biquad del paso alto, ya normalizado (a0 = 1), en forma directa I.
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
    ataque: f32,
    suelta: f32,
    vuelta: f32,
    envolvente: f32,
    ganancia: f32,
}

impl Realce {
    pub fn nuevo(muestreo: u32) -> Realce {
        let fs = muestreo.max(1) as f32;
        // El paso alto de Butterworth del «Audio EQ Cookbook» (Q = 1/raiz 2).
        let w0 = 2.0 * std::f32::consts::PI * (CORTE_HZ / fs).min(0.49);
        let (seno, coseno) = w0.sin_cos();
        let alfa = seno / (2.0 * std::f32::consts::FRAC_1_SQRT_2);
        let a0 = 1.0 + alfa;
        Realce {
            b0: (1.0 + coseno) / 2.0 / a0,
            b1: -(1.0 + coseno) / a0,
            b2: (1.0 + coseno) / 2.0 / a0,
            a1: -2.0 * coseno / a0,
            a2: (1.0 - alfa) / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
            ataque: coeficiente(ATAQUE_S, muestreo),
            suelta: coeficiente(SUELTA_S, muestreo),
            vuelta: coeficiente(VUELTA_EN_SILENCIO_S, muestreo),
            envolvente: 0.0,
            ganancia: GANANCIA_MINIMA,
        }
    }

    /// La ganancia que lleva ahora, en veces (1,0 = 0 dB).
    pub fn ganancia(&self) -> f32 {
        self.ganancia
    }

    /// Realza un trozo de muestras mono **en su sitio**.
    pub fn procesar(&mut self, muestras: &mut [i16]) {
        for s in muestras.iter_mut() {
            let x = *s as f32 / 32768.0;
            // 1. Paso alto.
            let mut y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
                - self.a1 * self.y1
                - self.a2 * self.y2;
            // Una cola que se apaga hacia cero acaba en numeros desnormales,
            // que en x86 son cien veces mas lentos: por debajo de lo audible
            // se corta a cero.
            if y.abs() < 1.0e-12 {
                y = 0.0;
            }
            self.x2 = self.x1;
            self.x1 = x;
            self.y2 = self.y1;
            self.y1 = y;

            // 2. La envolvente: sube deprisa, baja despacio.
            let a = y.abs();
            let k = if a > self.envolvente {
                self.ataque
            } else {
                self.suelta
            };
            self.envolvente += (a - self.envolvente) * k;

            // 3. La ganancia que toca, con la puerta de ruido.
            if self.envolvente < PISO {
                // Silencio: no se sube; se vuelve despacio a 0 dB.
                self.ganancia += (GANANCIA_MINIMA - self.ganancia) * self.vuelta;
            } else {
                let quiere = (OBJETIVO / self.envolvente).clamp(GANANCIA_MINIMA, GANANCIA_MAXIMA);
                // Bajar es urgente (lo que viene se saldria); subir, no.
                let k = if quiere < self.ganancia {
                    self.ataque
                } else {
                    self.suelta
                };
                self.ganancia += (quiere - self.ganancia) * k;
            }

            // 4. El limitador.
            let fuera = limitar(y * self.ganancia);
            *s = (fuera * 32767.0).round().clamp(-32767.0, 32767.0) as i16;
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const FS: u32 = 48_000;

    fn seno(amplitud: f32, hz: f32, segundos: f32) -> Vec<i16> {
        let n = (segundos * FS as f32) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / FS as f32;
                (amplitud * 32767.0 * (2.0 * std::f32::consts::PI * hz * t).sin()).round() as i16
            })
            .collect()
    }

    fn pico(v: &[i16]) -> i32 {
        v.iter().map(|s| (*s as i32).abs()).max().unwrap_or(0)
    }

    /// Procesa en trozos de 10 ms, como llegan de la tarjeta.
    fn realzar(mut v: Vec<i16>) -> Vec<i16> {
        let mut r = Realce::nuevo(FS);
        for trozo in v.chunks_mut(480) {
            r.procesar(trozo);
        }
        v
    }

    #[test]
    fn una_voz_baja_sale_mas_alta() {
        // -34 dBFS: lo que da un microfono de portatil a medio metro.
        let entrada = seno(0.02, 440.0, 2.0);
        let salida = realzar(entrada.clone());
        let cola = FS as usize; // el ultimo segundo, ya asentado
        let (antes, despues) = (pico(&entrada[cola..]), pico(&salida[cola..]));
        assert!(despues > 10 * antes, "{antes} -> {despues}");
        // Y no pasa de los +24 dB.
        assert!(despues as f32 <= antes as f32 * GANANCIA_MAXIMA * 1.05);
    }

    #[test]
    fn una_voz_ya_alta_no_se_recorta() {
        let entrada = seno(0.97, 440.0, 1.0);
        let salida = realzar(entrada);
        let p = pico(&salida);
        assert!(p < 32767, "llego al techo: {p}");
        // Ni un tramo plano pegado arriba: eso es un recorte.
        assert_eq!(
            salida
                .iter()
                .filter(|s| (**s as i32).abs() >= 32700)
                .count(),
            0
        );
        // Y sigue sonando alta: el limitador dobla, no apaga.
        assert!(p > (0.85 * 32767.0) as i32, "{p}");
    }

    #[test]
    fn el_silencio_sigue_en_silencio() {
        assert!(realzar(vec![0; FS as usize]).iter().all(|s| *s == 0));
        // Un ruido de fondo de -70 dBFS no se sube hasta oirse.
        let mut semilla = 12345u32;
        let ruido: Vec<i16> = (0..FS as usize)
            .map(|_| {
                semilla = semilla.wrapping_mul(1_103_515_245).wrapping_add(12345);
                ((semilla >> 16) % 21) as i16 - 10
            })
            .collect();
        let salida = realzar(ruido);
        assert!(pico(&salida) <= 30, "{}", pico(&salida));
    }

    #[test]
    fn el_paso_alto_quita_la_continua() {
        let salida = realzar(vec![8000; FS as usize]);
        let cola = &salida[FS as usize / 2..];
        let media = cola.iter().map(|s| *s as f64).sum::<f64>() / cola.len() as f64;
        assert!(media.abs() < 20.0, "media {media}");
        assert!(pico(cola) < 50, "{}", pico(cola));
    }

    #[test]
    fn el_limitador_nunca_llega_al_techo_y_es_simetrico() {
        for x in [0.0f32, 0.5, 0.85, 0.9, 1.0, 2.0, 15.0, 1.0e6] {
            let y = limitar(x);
            assert!(y < 1.0 && y >= 0.0, "{x} -> {y}");
            assert_eq!(limitar(-x), -y);
        }
        assert_eq!(limitar(0.5), 0.5, "por debajo del umbral no toca nada");
        assert!(limitar(0.9) < 0.9 && limitar(0.9) > 0.85);
    }
}
