//! **El reloj del video**: en que instante del MP4 va cada fotograma.
//!
//! Hay un solo reloj para las dos pistas. Con sonido, **manda el sonido**:
//! su posicion en muestras es el tiempo del MP4, como en la nota de voz, y
//! los fotogramas se fechan contra ella. Asi no se separan aunque el equipo
//! se atasque, y la pausa (no llegan muestras) para el video sola. Sin
//! sonido, el reloj es el del sistema (`QueryPerformanceCounter`) sin contar
//! lo que se estuvo en pausa.
//!
//! Todo va en unidades de 100 ns, que es lo que cuentan Media Foundation y
//! el `SystemRelativeTime` de cada fotograma de Windows.Graphics.Capture
//! (que es el mismo contador del sistema, ya pasado a esa unidad).
//!
//! Como se fecha un fotograma con sonido: cada bloque de sonido que llega
//! deja un **ancla** (el instante del MP4 en que acaba ese bloque, y la hora
//! del sistema a la que llego). Un fotograma capturado a la hora `q` va en
//! `ancla.tiempo + (q - ancla.hora)`, sin adelantarse mas de
//! [`ADELANTO_MAXIMO`] al sonido: si el sonido se para (pausa, un tiron),
//! el video se para con el en vez de seguir corriendo solo.

/// Media Foundation cuenta el tiempo en unidades de 100 nanosegundos.
pub const UNIDADES_POR_SEGUNDO: i64 = 10_000_000;

/// Lo que un fotograma puede ir por delante del ultimo sonido recibido. Los
/// bloques llegan cada 10-20 ms; un cuarto de segundo es holgura para un
/// tiron sin dejar que el video se escape en una pausa.
pub const ADELANTO_MAXIMO: i64 = UNIDADES_POR_SEGUNDO / 4;

/// Lo que un fotograma puede ir por detras del ultimo ancla (se capturo
/// antes de que llegara el ultimo bloque). Mas atras ya no es de este rato.
pub const RETRASO_MAXIMO: i64 = UNIDADES_POR_SEGUNDO;

/// Lo minimo entre dos fotogramas del MP4: un milisegundo. Dos con el mismo
/// instante no los acepta el contenedor.
pub const SEPARACION_MINIMA: i64 = UNIDADES_POR_SEGUNDO / 1000;

/// El instante, en 100 ns, al que se llega tras `muestras` muestras (por
/// canal) a `muestreo` por segundo. Exacto: se cuenta desde el cero, no
/// sumando duraciones redondeadas.
pub fn tiempo_de_muestras(muestras: u64, muestreo: u32) -> i64 {
    pixpin_record::escala::tiempo_de_muestras(muestras, muestreo)
}

/// Una lectura de `QueryPerformanceCounter` en 100 ns, sin desbordar en
/// equipos que llevan semanas encendidos (por eso no es `cuenta * 1e7 / f`).
pub fn contador_a_unidades(cuenta: i64, frecuencia: i64) -> i64 {
    if frecuencia <= 0 {
        return 0;
    }
    let enteros = cuenta / frecuencia;
    let resto = cuenta % frecuencia;
    enteros * UNIDADES_POR_SEGUNDO + resto * UNIDADES_POR_SEGUNDO / frecuencia
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ancla {
    /// El instante del MP4 en que acaba el ultimo bloque de sonido.
    tiempo: i64,
    /// La hora del sistema a la que llego.
    hora: i64,
}

/// El reloj de una grabacion. Todas las horas que recibe son del contador
/// del sistema en 100 ns ([`contador_a_unidades`]).
#[derive(Debug, Clone)]
pub struct Reloj {
    con_sonido: Option<u32>,
    inicio: i64,
    ancla: Option<Ancla>,
    pausa_desde: Option<i64>,
    pausado: i64,
    /// El instante del ultimo fotograma fechado.
    ultimo: Option<i64>,
    /// Donde acaba el sonido recibido.
    fin_sonido: i64,
}

impl Reloj {
    /// Un reloj que empieza a la hora `ahora`, con sonido a `muestreo` o
    /// sin el.
    pub fn nuevo(muestreo: Option<u32>, ahora: i64) -> Reloj {
        Reloj {
            con_sonido: muestreo,
            inicio: ahora,
            ancla: None,
            pausa_desde: None,
            pausado: 0,
            ultimo: None,
            fin_sonido: 0,
        }
    }

    /// Llego un bloque de sonido que acaba en la muestra `fin` (por canal),
    /// a la hora `ahora`.
    pub fn sonido(&mut self, fin: u64, ahora: i64) {
        let Some(muestreo) = self.con_sonido else {
            return;
        };
        let tiempo = tiempo_de_muestras(fin, muestreo);
        self.fin_sonido = self.fin_sonido.max(tiempo);
        // En pausa el sonido no deberia llegar; si llega (un bloque que ya
        // estaba en camino), cuenta para el final pero no ancla: al seguir
        // se ancla con el primero de verdad.
        if self.pausa_desde.is_none() {
            self.ancla = Some(Ancla {
                tiempo: self.fin_sonido,
                hora: ahora,
            });
        }
    }

    pub fn pausar(&mut self, ahora: i64) {
        if self.pausa_desde.is_none() {
            self.pausa_desde = Some(ahora);
            // Lo anclado antes de la pausa ya no dice nada de despues.
            self.ancla = None;
        }
    }

    pub fn seguir(&mut self, ahora: i64) {
        if let Some(desde) = self.pausa_desde.take() {
            self.pausado += (ahora - desde).max(0);
        }
    }

    pub fn en_pausa(&self) -> bool {
        self.pausa_desde.is_some()
    }

    /// **El instante del MP4 de un fotograma capturado a la hora `hora`**, o
    /// `None` si no va: en pausa, o con sonido pero sin ancla todavia (salvo
    /// el primero), o si el sonido esta parado y ya se llego a su tope.
    ///
    /// Si cae en el anterior o antes (Windows entrega el fotograma unos
    /// milisegundos despues de capturarlo, y entre medias pudo escribirse
    /// uno repetido), va [`SEPARACION_MINIMA`] despues del anterior: es la
    /// imagen mas nueva y no se puede quedar fuera.
    ///
    /// El primero va siempre en el cero: la pantalla tarda en dar su primera
    /// imagen y el sonido ya corre, y una pista de video que empieza tarde
    /// se ve como un hueco negro al principio.
    pub fn fotograma(&mut self, hora: i64) -> Option<i64> {
        if self.en_pausa() {
            return None;
        }
        let Some(ultimo) = self.ultimo else {
            self.ultimo = Some(0);
            return Some(0);
        };
        let (t, tope) = match self.con_sonido {
            Some(_) => {
                let a = self.ancla?;
                (
                    a.tiempo + (hora - a.hora).clamp(-RETRASO_MAXIMO, ADELANTO_MAXIMO),
                    a.tiempo + ADELANTO_MAXIMO,
                )
            }
            None => (hora - self.inicio - self.pausado, i64::MAX),
        };
        let t = t.max(ultimo + SEPARACION_MINIMA);
        if t > tope {
            return None;
        }
        self.ultimo = Some(t);
        Some(t)
    }

    /// El instante del ultimo fotograma fechado.
    pub fn ultimo(&self) -> Option<i64> {
        self.ultimo
    }

    /// **Donde acaba la grabacion** a la hora `ahora`: el final del sonido
    /// recibido, o lo corrido sin pausas si no hay sonido. Es donde se pone
    /// el ultimo fotograma para que el video dure lo mismo que el sonido.
    pub fn final_(&self, ahora: i64) -> i64 {
        match self.con_sonido {
            Some(_) => self.fin_sonido,
            None => {
                let en_pausa = self.pausa_desde.map_or(0, |d| (ahora - d).max(0));
                (ahora - self.inicio - self.pausado - en_pausa).max(0)
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const S: i64 = UNIDADES_POR_SEGUNDO;
    const MS: i64 = S / 1000;

    #[test]
    fn el_contador_pasa_a_100_ns_sin_desbordar() {
        assert_eq!(contador_a_unidades(10_000_000, 10_000_000), S);
        assert_eq!(contador_a_unidades(3_000_000, 3_000_000), S);
        // Medio segundo en un contador de 3 MHz.
        assert_eq!(contador_a_unidades(1_500_000, 3_000_000), S / 2);
        // Cuarenta dias encendido a 10 MHz: la cuenta ingenua (x 1e7) se
        // saldria de un i64.
        let cuarenta_dias = 40 * 86_400 * 10_000_000i64;
        assert_eq!(
            contador_a_unidades(cuarenta_dias, 10_000_000),
            40 * 86_400 * S
        );
        // Caso negativo: una frecuencia rota no divide por cero.
        assert_eq!(contador_a_unidades(123, 0), 0);
    }

    #[test]
    fn sin_sonido_el_reloj_es_el_del_sistema_sin_las_pausas() {
        let mut r = Reloj::nuevo(None, 100 * S);
        assert_eq!(
            r.fotograma(100 * S + 3 * MS),
            Some(0),
            "el primero en el cero"
        );
        assert_eq!(r.fotograma(101 * S), Some(S));
        r.pausar(102 * S);
        // Caso negativo: en pausa no se fecha nada.
        assert_eq!(r.fotograma(103 * S), None);
        r.seguir(105 * S);
        // Tres segundos de pausa que no cuentan.
        assert_eq!(r.fotograma(106 * S), Some(3 * S));
        assert_eq!(r.final_(107 * S), 4 * S);
    }

    #[test]
    fn con_sonido_manda_el_sonido() {
        let mut r = Reloj::nuevo(Some(48_000), 0);
        // Sin ancla, el primero va en el cero igual.
        assert_eq!(r.fotograma(5 * MS), Some(0));
        // Caso negativo: el segundo, sin ningun sonido aun, espera.
        assert_eq!(r.fotograma(50 * MS), None);
        // Llega un segundo de sonido a la hora 1,02 s (un poco tarde).
        r.sonido(48_000, S + 20 * MS);
        // Un fotograma capturado 100 ms despues de llegar el sonido va 100
        // ms despues del final del sonido, no a la hora del sistema.
        assert_eq!(r.fotograma(S + 120 * MS), Some(S + 100 * MS));
        assert_eq!(r.final_(0), S);
    }

    #[test]
    fn si_el_sonido_se_para_el_video_no_se_escapa() {
        let mut r = Reloj::nuevo(Some(48_000), 0);
        r.fotograma(0);
        r.sonido(48_000, S);
        // Diez segundos despues sin sonido nuevo: no pasa del adelanto.
        let t = r.fotograma(11 * S).unwrap();
        assert_eq!(t, S + ADELANTO_MAXIMO);
        // Caso negativo: los siguientes no avanzan y no se fechan.
        assert_eq!(r.fotograma(12 * S), None);
        // Vuelve el sonido y el video sigue con el.
        r.sonido(96_000, 13 * S);
        assert_eq!(r.fotograma(13 * S + 50 * MS), Some(2 * S + 50 * MS));
    }

    #[test]
    fn tras_una_pausa_con_sonido_se_espera_al_sonido_nuevo() {
        let mut r = Reloj::nuevo(Some(44_100), 0);
        r.fotograma(0);
        r.sonido(44_100, S);
        assert!(r.fotograma(S + 10 * MS).is_some());
        r.pausar(2 * S);
        // Un bloque rezagado en pausa cuenta para el final, no ancla.
        r.sonido(2 * 44_100, 2 * S + 5 * MS);
        r.seguir(10 * S);
        // Caso negativo: sin sonido nuevo tras la pausa, el ancla vieja no
        // vale (diria que van ocho segundos mas).
        assert_eq!(r.fotograma(10 * S + 10 * MS), None);
        r.sonido(3 * 44_100, 11 * S);
        assert_eq!(r.fotograma(11 * S + 40 * MS), Some(3 * S + 40 * MS));
        assert_eq!(r.final_(0), 3 * S);
    }

    #[test]
    fn dos_fotogramas_nunca_van_en_el_mismo_instante() {
        let mut r = Reloj::nuevo(None, 0);
        assert_eq!(r.fotograma(0), Some(0));
        // Demasiado cerca: justo detras del anterior, no fuera.
        assert_eq!(r.fotograma(MS / 2), Some(MS));
        assert_eq!(r.fotograma(5 * MS), Some(5 * MS));
        // Caso negativo: capturado antes que el anterior (llego tarde) no
        // va hacia atras, va detras.
        assert_eq!(r.fotograma(MS / 3), Some(6 * MS));
        assert_eq!(r.ultimo(), Some(6 * MS));
    }

    #[test]
    fn el_fotograma_capturado_antes_del_ultimo_bloque_va_algo_antes() {
        let mut r = Reloj::nuevo(Some(48_000), 0);
        r.fotograma(0);
        r.sonido(48_000 * 5, 5 * S);
        // Capturado 30 ms antes de que llegara el bloque.
        assert_eq!(r.fotograma(5 * S - 30 * MS), Some(5 * S - 30 * MS));
        // Caso negativo: uno de hace un minuto no se va un minuto atras.
        let mut r = Reloj::nuevo(Some(48_000), 0);
        r.fotograma(0);
        r.sonido(48_000 * 100, 100 * S);
        assert_eq!(r.fotograma(40 * S), Some(99 * S));
    }
}
