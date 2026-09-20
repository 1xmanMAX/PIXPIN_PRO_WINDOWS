//! Los picos de la onda, **con el contrato del movil al numero**.
//!
//! Esto es lo unico de todo el crate que sale del ordenador y viaja: el
//! campo `picos` de un mensaje de voz (`guardados/Mensajes.kt:136`) es un
//! array JSON de **enteros crudos de 0 a 32767**, como mucho **256**, uno
//! cada **50 ms**, y el movil lo lee tal cual. Si el PC escribiera flotantes
//! normalizados a `[0,1]`, kotlinx los leeria como ceros y la onda
//! desapareceria en el telefono sin que nadie viera un error.
//!
//! La normalizacion es al **pintar**, contra el maximo de esa misma nota, y
//! eso el PC ya lo hace en `pixpin-ui/src/chat.rs` (`barras_de_onda`). Aqui
//! no se normaliza nada.
//!
//! Todo el modulo es aritmetica: el submuestreo —que es donde de verdad se
//! rompen las cosas: listas vacias, divisiones entre cero, el tope— se
//! comprueba sin microfono.

/// Cada cuanto se anota un pico, en milisegundos.
///
/// `Voz.MS_ENTRE_PICOS` (`pin/Voz.kt:122`). Veinte veces por segundo: menos
/// deja la onda a trozos —una silaba entera cabe en un hueco— y mas solo
/// engorda la linea del JSONL para dibujar las mismas 50 barras.
pub const MS_ENTRE_PICOS: u64 = 50;

/// Cuantos picos se guardan por nota, como mucho.
///
/// `PICOS_GUARDADOS` (`guardados/Onda.kt:96`). Un pico cada 50 ms son 1200
/// numeros por minuto; sin tope, una nota de diez minutos engordaria su
/// linea de JSON a decenas de kilobytes para dibujar 50 barras.
pub const PICOS_GUARDADOS: usize = 256;

/// Lo mas alto que puede valer un pico.
///
/// Es el rango de `MediaRecorder.getMaxAmplitude()` (`Voz.kt:88`), que a su
/// vez es el de una muestra de 16 bits. El PC captura PCM de 16 bits, asi
/// que el rango sale solo; se acota igual por si la mezcla en coma flotante
/// entrega una muestra por encima de 1.0.
pub const PICO_MAXIMO: i32 = 32767;

/// Lo menos que puede durar una nota para que valga la pena guardarla.
///
/// `Voz.MINIMO_MS` (`Voz.kt:138`). Por debajo de esto lo que hubo fue un
/// resbalon en el boton, no una nota.
pub const MINIMO_MS: i64 = 700;

/// Un pico dentro de su rango. Los negativos —que no deberian existir— son
/// cero, y lo que se pase del tope se planta en el tope.
pub fn acotar_pico(pico: i32) -> i32 {
    pico.clamp(0, PICO_MAXIMO)
}

/// El pico de un bloque de muestras: la mas alta en valor absoluto.
///
/// Se mira el **maximo** y no la media porque lo que distingue una nota de
/// otra en la onda son los golpes de voz, y una media de 50 ms de silencio
/// con una consonante dentro los aplana. Es tambien lo que hace
/// `getMaxAmplitude()`, que devuelve el mayor desde la llamada anterior.
///
/// `i16::MIN` no tiene opuesto positivo, asi que se toma el valor absoluto
/// sin signo y se acota: de otro modo un silencio digital saturado abortaria
/// la grabacion en una resta.
pub fn pico_de_bloque(muestras: &[i16]) -> i32 {
    let mut maximo: u16 = 0;
    for m in muestras {
        maximo = maximo.max(m.unsigned_abs());
    }
    acotar_pico(maximo as i32)
}

/// Junta los picos de dos en dos, quedandose con la mitad.
///
/// `aMitad` (`Onda.kt:147-158`). Promediando, no descartando: descartar la
/// mitad se come los golpes de voz y aplana la onda. Si sobra uno al final
/// se queda tal cual, que es mejor que perder el final de la nota.
pub fn a_mitad(picos: &[i32]) -> Vec<i32> {
    if picos.len() < 2 {
        return picos.to_vec();
    }
    let mut salida = Vec::with_capacity(picos.len().div_ceil(2));
    let mut i = 0;
    while i < picos.len() {
        let valor = if i + 1 < picos.len() {
            (picos[i] + picos[i + 1]) / 2
        } else {
            picos[i]
        };
        salida.push(valor);
        i += 2;
    }
    salida
}

/// El acumulador de picos mientras se graba, **acotado**.
///
/// `class Picos` (`Onda.kt:169-192`). Al llegar al tope se junta lo guardado
/// de dos en dos y se pasa a mirar uno de cada dos: asi una nota de treinta
/// segundos y una de diez minutos ocupan lo mismo y las dos conservan su
/// forma entera, en vez de guardar el principio y cortar el resto.
#[derive(Debug, Clone)]
pub struct Picos {
    lista: Vec<i32>,
    /// De cuantos picos se anota uno. Se dobla cada vez que se reduce.
    paso: usize,
    vistos: usize,
    tope: usize,
}

impl Default for Picos {
    fn default() -> Self {
        Picos::con_tope(PICOS_GUARDADOS)
    }
}

impl Picos {
    pub fn nuevo() -> Picos {
        Picos::default()
    }

    /// Un acumulador con otro tope. Existe para poder probar el doblado del
    /// paso con cuatro numeros en vez de con 256.
    ///
    /// Un tope de cero o uno dejaria el acumulador reduciendo en cada pico y
    /// sin salir nunca, asi que el minimo real es dos.
    pub fn con_tope(tope: usize) -> Picos {
        Picos {
            lista: Vec::new(),
            paso: 1,
            vistos: 0,
            tope: tope.max(2),
        }
    }

    /// Anota un pico del microfono.
    pub fn anota(&mut self, pico: i32) {
        self.vistos += 1;
        if self.vistos % self.paso != 0 {
            return;
        }
        self.lista.push(acotar_pico(pico));
        if self.lista.len() >= self.tope {
            self.lista = a_mitad(&self.lista);
            self.paso *= 2;
            self.vistos = 0;
        }
    }

    /// Lo acumulado, para guardarlo en el mensaje.
    pub fn lista(&self) -> Vec<i32> {
        self.lista.clone()
    }

    pub fn cuantos(&self) -> usize {
        self.lista.len()
    }

    pub fn vacio(&self) -> bool {
        self.lista.is_empty()
    }
}

/// Reparte las muestras que van llegando en picos de [`MS_ENTRE_PICOS`].
///
/// La tarjeta no entrega bloques de 50 ms: entrega lo que le cabe, que
/// cambia de un paquete a otro y de un aparato a otro. Esta pieza lleva la
/// cuenta en **muestras**, no en relojes, para que la onda de una nota no
/// dependa de lo cargado que estuviera el ordenador al grabarla.
#[derive(Debug, Clone)]
pub struct Reparto {
    /// Cuantas muestras hay en un pico. Nunca cero.
    por_pico: usize,
    llevadas: usize,
    maximo: i32,
    picos: Picos,
}

impl Reparto {
    /// `muestreo` en hercios del flujo mono que se le va a dar.
    pub fn nuevo(muestreo: u32) -> Reparto {
        let por_pico = (muestreo as u64 * MS_ENTRE_PICOS / 1000).max(1) as usize;
        Reparto {
            por_pico,
            llevadas: 0,
            maximo: 0,
            picos: Picos::nuevo(),
        }
    }

    /// Mete un bloque de muestras mono de 16 bits.
    pub fn empuja(&mut self, muestras: &[i16]) {
        for m in muestras {
            self.maximo = self.maximo.max(m.unsigned_abs() as i32);
            self.llevadas += 1;
            if self.llevadas >= self.por_pico {
                self.picos.anota(self.maximo);
                self.maximo = 0;
                self.llevadas = 0;
            }
        }
    }

    /// Cierra el ultimo pico a medias y devuelve la lista para el mensaje.
    ///
    /// El resto que no llego a 50 ms se anota igual: tirarlo dejaria la
    /// ultima barra de la onda en el suelo aunque ahi hubiera voz.
    pub fn terminar(mut self) -> Vec<i32> {
        if self.llevadas > 0 {
            self.picos.anota(self.maximo);
        }
        self.picos.lista()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_pico_negativo_se_toma_como_cero_y_uno_que_se_pasa_del_tope_se_planta() {
        assert_eq!(acotar_pico(-1), 0);
        assert_eq!(acotar_pico(-40_000), 0);
        assert_eq!(acotar_pico(99_999), PICO_MAXIMO);
        assert_eq!(acotar_pico(1_234), 1_234);
    }

    #[test]
    fn el_acumulador_nunca_guarda_un_pico_fuera_de_cero_a_32767() {
        let mut p = Picos::nuevo();
        for v in [-5, 0, 32_767, 999_999, i32::MAX, i32::MIN] {
            p.anota(v);
        }
        assert!(
            p.lista().iter().all(|v| (0..=PICO_MAXIMO).contains(v)),
            "salio {:?}",
            p.lista()
        );
    }

    #[test]
    fn el_pico_de_un_bloque_es_el_mas_alto_en_valor_absoluto() {
        assert_eq!(pico_de_bloque(&[0, -300, 120]), 300);
        assert_eq!(pico_de_bloque(&[]), 0);
    }

    #[test]
    fn el_pico_de_un_bloque_saturado_al_negativo_no_desborda() {
        // `i16::MIN.abs()` no cabe en un i16: sin `unsigned_abs` esto
        // abortaria en depuracion y daria basura en release.
        assert_eq!(pico_de_bloque(&[i16::MIN]), PICO_MAXIMO);
    }

    #[test]
    fn juntar_de_dos_en_dos_promedia_y_conserva_el_impar_del_final() {
        assert_eq!(a_mitad(&[10, 20, 30, 40]), vec![15, 35]);
        assert_eq!(a_mitad(&[10, 20, 30]), vec![15, 30]);
        assert_eq!(a_mitad(&[7]), vec![7]);
        assert_eq!(a_mitad(&[]), Vec::<i32>::new());
    }

    #[test]
    fn una_nota_larguisima_no_pasa_nunca_del_tope_de_picos() {
        let mut p = Picos::nuevo();
        // Diez minutos a veinte picos por segundo: 12 000 picos.
        for i in 0..12_000 {
            p.anota(i % PICO_MAXIMO);
        }
        assert!(
            p.cuantos() <= PICOS_GUARDADOS,
            "guardo {} picos",
            p.cuantos()
        );
        assert!(p.cuantos() > PICOS_GUARDADOS / 2, "se quedo casi vacio");
    }

    #[test]
    fn al_llegar_al_tope_se_reduce_a_la_mitad_y_se_dobla_el_paso() {
        let mut p = Picos::con_tope(4);
        for v in [100, 200, 300, 400] {
            p.anota(v);
        }
        // Cuatro picos llenan el tope y quedan dos promediados.
        assert_eq!(p.lista(), vec![150, 350]);
        // Con el paso a dos, hace falta un pico mas para anotar el
        // siguiente: del par se queda el segundo, como en el movil.
        p.anota(500);
        assert_eq!(p.lista(), vec![150, 350]);
        p.anota(600);
        assert_eq!(p.lista(), vec![150, 350, 600]);
    }

    #[test]
    fn un_audio_de_cero_segundos_no_da_ni_un_pico() {
        let r = Reparto::nuevo(48_000);
        assert!(r.terminar().is_empty());
    }

    #[test]
    fn el_reparto_da_un_pico_por_cada_cincuenta_milisegundos() {
        let mut r = Reparto::nuevo(48_000);
        // Un segundo justo: veinte picos.
        r.empuja(&vec![1_000i16; 48_000]);
        assert_eq!(r.terminar().len(), 20);
    }

    #[test]
    fn el_ultimo_trozo_a_medias_se_guarda_igual() {
        let mut r = Reparto::nuevo(48_000);
        // 50 ms justos mas 10 ms: dos picos, el segundo a medias.
        r.empuja(&vec![500i16; 2_400 + 480]);
        let picos = r.terminar();
        assert_eq!(picos, vec![500, 500]);
    }

    #[test]
    fn un_muestreo_absurdo_no_divide_entre_cero() {
        let mut r = Reparto::nuevo(0);
        r.empuja(&[1, 2, 3]);
        assert_eq!(r.terminar().len(), 3);
    }
}
