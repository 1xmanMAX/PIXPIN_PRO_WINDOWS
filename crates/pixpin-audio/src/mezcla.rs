//! De lo que entrega la tarjeta a lo que come el codificador.
//!
//! WASAPI en modo compartido no negocia: da lo que tenga puesto la mesa de
//! mezclas de Windows, que casi siempre es **coma flotante de 32 bits,
//! estereo, a 48 000 Hz**, pero puede ser otra cosa en un microfono USB o
//! en unos cascos con manos libres. El codificador AAC del sistema, en
//! cambio, solo acepta **PCM de 16 bits, mono o estereo, a 44 100 o 48 000
//! Hz**. Entre uno y otro hay que bajar a mono, pasar a enteros y, muy de
//! vez en cuando, remuestrear.
//!
//! Todo esto es aritmetica pura y esta aqui aparte por eso: una mezcla mal
//! hecha no da un error, da una nota de voz que suena a lata, y eso solo se
//! pilla con numeros.

/// Los dos unicos muestreos que traga el codificador AAC de Windows.
///
/// Documentado por Microsoft para el «AAC Encoder» (el MFT que usa el sink
/// writer): entrada PCM de 16 bits, 1 o 2 canales, y **44 100 o 48 000**
/// muestras por segundo. Los 22 050 Hz del movil (`Voz.kt:140`) no estan, y
/// esa es la unica diferencia de formato que no se puede evitar.
pub const MUESTREOS_QUE_ADMITE_AAC: [u32; 2] = [44_100, 48_000];

/// A que muestreo se graba, sabiendo a cual esta la tarjeta.
///
/// Se prefiere **no remuestrear**: si la tarjeta ya esta en uno de los dos
/// que admite el codificador, se usa ese y las muestras pasan intactas. En
/// cualquier otro caso se sube a 48 000, que es el de la inmensa mayoria de
/// los equipos y el que menos artefactos deja al interpolar.
pub fn muestreo_de_grabacion(el_de_la_tarjeta: u32) -> u32 {
    if MUESTREOS_QUE_ADMITE_AAC.contains(&el_de_la_tarjeta) {
        el_de_la_tarjeta
    } else {
        48_000
    }
}

/// Cuanto duran `muestras` muestras mono, en milisegundos.
///
/// Se redondea hacia abajo, que es lo que hace `MediaPlayer.duration`: una
/// nota de 6 999 ms se rotula `0:06` en los dos aparatos.
pub fn duracion_ms(muestras: usize, muestreo: u32) -> i64 {
    if muestreo == 0 {
        return 0;
    }
    (muestras as i64 * 1000) / muestreo as i64
}

/// Una muestra en coma flotante pasada a entero de 16 bits.
///
/// La mesa de Windows entrega valores centrados en cero que **pueden pasar
/// de 1.0** cuando algo va saturado. Sin acotar, el `as i16` de Rust satura
/// en vez de dar la vuelta —no es como en C—, pero el valor ya seria basura:
/// se acota antes para que un pico saturado quede en el tope limpio.
pub fn a_entero(v: f32) -> i16 {
    let v = if v.is_nan() { 0.0 } else { v.clamp(-1.0, 1.0) };
    (v * i16::MAX as f32).round() as i16
}

/// Baja a mono un bloque intercalado de muestras en coma flotante.
///
/// Se **promedian** los canales en vez de quedarse con el izquierdo: hay
/// microfonos que dejan un canal callado, y quedarse con ese da una nota
/// muda que solo se descubre al escucharla.
pub fn a_mono_desde_f32(intercaladas: &[f32], canales: u16) -> Vec<i16> {
    let canales = canales.max(1) as usize;
    intercaladas
        .chunks_exact(canales)
        .map(|marco| a_entero(marco.iter().sum::<f32>() / canales as f32))
        .collect()
}

/// Lo mismo, con muestras que ya son enteros de 16 bits.
///
/// La suma se hace en `i32` porque dos canales al tope se salen de un `i16`
/// antes de dividir.
pub fn a_mono_desde_i16(intercaladas: &[i16], canales: u16) -> Vec<i16> {
    let canales = canales.max(1) as usize;
    intercaladas
        .chunks_exact(canales)
        .map(|marco| {
            let suma: i32 = marco.iter().map(|m| *m as i32).sum();
            (suma / canales as i32) as i16
        })
        .collect()
}

/// Remuestreador lineal **con memoria entre bloques**.
///
/// Los bloques llegan de la tarjeta de uno en uno y no caen en fronteras
/// redondas. Un remuestreador que empezara de cero en cada bloque metria un
/// chasquido en cada costura —veinte por segundo—, que es exactamente el
/// ruido que la gente describe como «se oye a robot». Por eso guarda la
/// ultima muestra del bloque anterior y la fase con la que se quedo.
///
/// Es interpolacion lineal, no un filtro: para voz a 22-48 kHz sobra, y un
/// filtro de verdad seria media biblioteca por un caso que casi no ocurre.
///
/// **La fase se lleva con enteros**, no con un `f64` que se va sumando. Con
/// un flotante, 22 050/48 000 no es un numero exacto y el error se acumula:
/// una nota de diez minutos son medio millon de sumas, y al final la
/// posicion de una muestra puede caer al otro lado de un entero. El sintoma
/// no es ruido, es peor: la misma grabacion sale distinta segun en cuantos
/// trozos la haya entregado la tarjeta, y eso no hay forma de comprobarlo
/// despues. Con `entero + resto/a` la cuenta es exacta y da igual como se
/// trocee.
#[derive(Debug, Clone)]
pub struct Remuestreador {
    de: u32,
    a: u32,
    /// Parte entera de donde cae la proxima muestra de salida, contada en
    /// muestras de entrada desde la ultima del bloque anterior.
    entero: i64,
    /// La parte de detras de la coma, como `resto / a`. Siempre menor que
    /// `a`.
    resto: u32,
    anterior: i16,
}

impl Remuestreador {
    pub fn nuevo(de: u32, a: u32) -> Remuestreador {
        Remuestreador {
            de: de.max(1),
            a: a.max(1),
            // Uno y no cero: en la primera llamada el indice 0 es la
            // muestra fantasma de arranque, no sonido de verdad.
            entero: 1,
            resto: 0,
            anterior: 0,
        }
    }

    /// Si no hay nada que hacer. Lo pregunta quien graba para saltarse la
    /// copia entera cuando la tarjeta ya esta al muestreo bueno.
    pub fn transparente(&self) -> bool {
        self.de == self.a
    }

    pub fn empuja(&mut self, entrada: &[i16]) -> Vec<i16> {
        if self.transparente() {
            return entrada.to_vec();
        }
        if entrada.is_empty() {
            return Vec::new();
        }
        let mut buf = Vec::with_capacity(entrada.len() + 1);
        buf.push(self.anterior);
        buf.extend_from_slice(entrada);

        let ultimo = (buf.len() - 1) as i64;
        let salto = (self.de / self.a) as i64;
        let salto_resto = self.de % self.a;
        let mut salida = Vec::with_capacity(entrada.len() * self.a as usize / self.de as usize + 2);
        // `< ultimo` y no `<= ultimo`: interpolar necesita la muestra de al
        // lado, y la ultima del bloque todavia no la tiene. Emitirla
        // clavandola contra si misma daria un valor distinto del que sale
        // cuando el siguiente bloque trae su vecina de verdad, y ese es
        // justo el chasquido en la costura. Se espera; no se pierde nada,
        // porque el bloque siguiente empieza por ella.
        while self.entero < ultimo {
            let i = self.entero as usize;
            let f = self.resto as f64 / self.a as f64;
            let a = buf[i] as f64;
            let b = buf[i + 1] as f64;
            salida.push((a + (b - a) * f).round() as i16);
            self.entero += salto;
            self.resto += salto_resto;
            if self.resto >= self.a {
                self.resto -= self.a;
                self.entero += 1;
            }
        }
        // El indice `entrada.len()` de este bloque es el indice 0 del
        // siguiente: la fase se traslada restando, no se pierde.
        self.entero -= entrada.len() as i64;
        self.anterior = *buf.last().unwrap();
        salida
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_graba_al_muestreo_de_la_tarjeta_si_el_codificador_lo_admite() {
        assert_eq!(muestreo_de_grabacion(48_000), 48_000);
        assert_eq!(muestreo_de_grabacion(44_100), 44_100);
    }

    #[test]
    fn un_muestreo_que_el_codificador_no_admite_sube_a_cuarenta_y_ocho_mil() {
        // Los 22 050 del movil son justo el caso: el AAC de Windows no los
        // acepta.
        assert_eq!(muestreo_de_grabacion(22_050), 48_000);
        assert_eq!(muestreo_de_grabacion(16_000), 48_000);
        assert_eq!(muestreo_de_grabacion(0), 48_000);
    }

    #[test]
    fn una_muestra_saturada_se_planta_en_el_tope_y_no_da_la_vuelta() {
        assert_eq!(a_entero(2.0), i16::MAX);
        assert_eq!(a_entero(-2.0), -i16::MAX);
        assert_eq!(a_entero(0.0), 0);
        assert_eq!(a_entero(f32::NAN), 0);
    }

    #[test]
    fn bajar_a_mono_promedia_los_canales_en_vez_de_quedarse_con_uno() {
        // Canal izquierdo callado: quedarse con el daria silencio.
        let estereo = [0.0, 1.0, 0.0, 1.0];
        assert_eq!(a_mono_desde_f32(&estereo, 2), vec![16_384, 16_384]);
        assert_eq!(a_mono_desde_i16(&[0, 1000, 0, 1000], 2), vec![500, 500]);
    }

    #[test]
    fn un_marco_a_medias_al_final_del_bloque_no_inventa_una_muestra() {
        // Tres valores en estereo: el tercero es medio marco y se descarta;
        // la tarjeta lo entregara entero en el paquete siguiente.
        assert_eq!(a_mono_desde_i16(&[100, 100, 900], 2), vec![100]);
    }

    #[test]
    fn cero_canales_no_divide_entre_cero() {
        assert_eq!(a_mono_desde_i16(&[100, 200], 0), vec![100, 200]);
    }

    #[test]
    fn la_duracion_sale_de_las_muestras_y_no_del_reloj() {
        assert_eq!(duracion_ms(48_000, 48_000), 1_000);
        assert_eq!(duracion_ms(0, 48_000), 0);
        assert_eq!(duracion_ms(48_000, 0), 0);
    }

    #[test]
    fn el_remuestreador_al_mismo_muestreo_no_toca_nada() {
        let mut r = Remuestreador::nuevo(48_000, 48_000);
        assert!(r.transparente());
        assert_eq!(r.empuja(&[1, 2, 3]), vec![1, 2, 3]);
    }

    #[test]
    fn remuestrear_al_doble_da_el_doble_de_muestras() {
        let mut r = Remuestreador::nuevo(24_000, 48_000);
        let entrada: Vec<i16> = (0..24_000).map(|i| (i % 1000) as i16).collect();
        let salida = r.empuja(&entrada);
        // Una muestra de margen por la fase: lo que no puede es dar la
        // mitad ni el triple.
        assert!(
            (salida.len() as i64 - 48_000).abs() <= 2,
            "salieron {}",
            salida.len()
        );
    }

    #[test]
    fn remuestrear_por_bloques_da_lo_mismo_que_de_una_vez() {
        let entrada: Vec<i16> = (0..4_000).map(|i| ((i * 7) % 2000 - 1000) as i16).collect();

        let mut de_una = Remuestreador::nuevo(22_050, 48_000);
        let entera = de_una.empuja(&entrada);

        let mut a_trozos = Remuestreador::nuevo(22_050, 48_000);
        let mut partida = Vec::new();
        for trozo in entrada.chunks(313) {
            partida.extend(a_trozos.empuja(trozo));
        }

        assert_eq!(
            entera, partida,
            "la costura entre bloques mete un chasquido"
        );
    }

    #[test]
    fn un_bloque_vacio_no_rompe_la_fase() {
        let mut r = Remuestreador::nuevo(22_050, 48_000);
        let a = r.empuja(&[100, 200, 300]);
        let vacio = r.empuja(&[]);
        assert!(vacio.is_empty());
        let b = r.empuja(&[400, 500]);

        let mut sin_vacio = Remuestreador::nuevo(22_050, 48_000);
        let c = sin_vacio.empuja(&[100, 200, 300]);
        let d = sin_vacio.empuja(&[400, 500]);
        assert_eq!((a, b), (c, d));
    }
}
