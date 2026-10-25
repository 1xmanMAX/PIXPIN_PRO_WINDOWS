//! **El ritmo variable**: cuando se codifica un fotograma.
//!
//! Windows.Graphics.Capture solo entrega un fotograma cuando algo cambio en
//! la pantalla. El motor se queda con el ultimo que llego (sin copiarlo) y
//! decide aqui si toca codificarlo:
//!
//! - **Hay uno nuevo y paso el minimo** (1/fps del perfil): se codifica.
//!   Si la pantalla cambia mas deprisa que el tope, los de en medio se
//!   juntan en el ultimo: es el tope, no una perdida.
//! - **No hay nada nuevo pero paso un segundo** ([`LATIDO`]): se repite el
//!   ultimo. Un fotograma repetido cuesta unos pocos bytes, y sin el los
//!   reproductores calculan mal la duracion y los saltos.
//! - Si no, se espera; [`espera`] dice cuanto, para que el hilo duerma justo
//!   eso y no de vueltas.
//!
//! Con una diapositiva quieta salen un fotograma por segundo en vez de
//! quince o treinta: es la mayor parte del ahorro de disco y de CPU.

use crate::reloj::UNIDADES_POR_SEGUNDO;

/// Lo que se espera como mucho sin escribir ningun fotograma.
pub const LATIDO: i64 = UNIDADES_POR_SEGUNDO;

/// Lo que se espera al primer fotograma de la pantalla antes de empezar con
/// uno negro (una ventana minimizada, una pantalla que no da imagen).
pub const ESPERA_PRIMERO: i64 = 5 * UNIDADES_POR_SEGUNDO;

/// Lo que se le deja al minimo entre fotogramas: un 5 % menos. La pantalla
/// da sus fotogramas al ritmo del monitor (cada 16,7 ms a 60 Hz) y, sin
/// holgura, un tope de 66,7 ms (15 fps) caeria siempre en el quinto
/// fotograma del monitor (83 ms, 12 fps).
pub fn minimo_entre(por_segundo: u32) -> i64 {
    UNIDADES_POR_SEGUNDO / i64::from(por_segundo.max(1)) * 95 / 100
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Codificar el fotograma nuevo.
    Nuevo,
    /// Repetir el ultimo (o uno negro si aun no hay ninguno).
    Repetir,
    /// Nada todavia.
    Esperar,
}

/// **Que hacer a la hora `ahora`**, habiendo empezado a la hora `inicio`,
/// con el ultimo fotograma escrito a la hora `ultimo` (o ninguno), y
/// sabiendo si hay uno nuevo esperando.
pub fn decidir(
    ahora: i64,
    inicio: i64,
    ultimo: Option<i64>,
    hay_nuevo: bool,
    minimo: i64,
) -> Decision {
    match ultimo {
        None if hay_nuevo => Decision::Nuevo,
        None if ahora - inicio >= ESPERA_PRIMERO => Decision::Repetir,
        None => Decision::Esperar,
        Some(u) => {
            let pasado = ahora - u;
            if hay_nuevo && pasado >= minimo {
                Decision::Nuevo
            } else if pasado >= LATIDO {
                Decision::Repetir
            } else {
                Decision::Esperar
            }
        }
    }
}

/// **Lo que se puede dormir** hasta que [`decidir`] cambie de opinion, en
/// 100 ns: hasta que pase el minimo si hay uno nuevo, o hasta el latido.
/// Llega un fotograma nuevo antes y el hilo se despierta solo.
pub fn espera(ahora: i64, inicio: i64, ultimo: Option<i64>, hay_nuevo: bool, minimo: i64) -> i64 {
    let hasta = match ultimo {
        None if hay_nuevo => ahora,
        None => inicio + ESPERA_PRIMERO,
        Some(u) if hay_nuevo => u + minimo,
        Some(u) => u + LATIDO,
    };
    (hasta - ahora).max(0)
}

/// **Fotogramas por segundo de verdad**, para ensenarlos: los escritos en
/// la ultima ventana de un segundo.
#[derive(Debug, Clone, Default)]
pub struct Medidor {
    desde: Option<i64>,
    en_ventana: u32,
    ultimo: f32,
}

impl Medidor {
    /// Se escribio un fotograma a la hora `ahora`.
    pub fn uno(&mut self, ahora: i64) {
        self.cerrar(ahora);
        self.en_ventana += 1;
    }

    /// Lo medido en la ultima ventana cerrada.
    pub fn por_segundo(&mut self, ahora: i64) -> f32 {
        self.cerrar(ahora);
        self.ultimo
    }

    fn cerrar(&mut self, ahora: i64) {
        let desde = *self.desde.get_or_insert(ahora);
        let pasado = ahora - desde;
        if pasado >= UNIDADES_POR_SEGUNDO {
            self.ultimo = if pasado >= 2 * UNIDADES_POR_SEGUNDO {
                // Mas de una ventana entera sin nada: cero, no lo de antes.
                0.0
            } else {
                self.en_ventana as f32 * UNIDADES_POR_SEGUNDO as f32 / pasado as f32
            };
            self.en_ventana = 0;
            self.desde = Some(ahora);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const S: i64 = UNIDADES_POR_SEGUNDO;
    const MS: i64 = S / 1000;

    #[test]
    fn el_minimo_deja_holgura_para_el_monitor() {
        // 15 fps: 63,3 ms, por debajo de los 66,7 del monitor a 60 Hz x 4.
        let m = minimo_entre(15);
        assert!(m < 4 * 166_667 && m > 3 * 166_667, "{m}");
        assert_eq!(minimo_entre(0), minimo_entre(1));
    }

    #[test]
    fn el_primero_se_codifica_en_cuanto_llega() {
        let m = minimo_entre(15);
        assert_eq!(decidir(0, 0, None, true, m), Decision::Nuevo);
        assert_eq!(decidir(MS, 0, None, false, m), Decision::Esperar);
        // Caso negativo: sin imagen en cinco segundos, se empieza en negro
        // en vez de esperar para siempre.
        assert_eq!(
            decidir(ESPERA_PRIMERO, 0, None, false, m),
            Decision::Repetir
        );
    }

    #[test]
    fn la_pantalla_que_cambia_deprisa_respeta_el_tope() {
        let m = minimo_entre(10);
        assert_eq!(decidir(50 * MS, 0, Some(0), true, m), Decision::Esperar);
        assert_eq!(decidir(96 * MS, 0, Some(0), true, m), Decision::Nuevo);
    }

    #[test]
    fn la_pantalla_quieta_da_un_fotograma_por_segundo() {
        let m = minimo_entre(30);
        assert_eq!(decidir(500 * MS, 0, Some(0), false, m), Decision::Esperar);
        assert_eq!(decidir(S, 0, Some(0), false, m), Decision::Repetir);
        // Caso negativo: un fotograma nuevo no se queda esperando al latido.
        assert_eq!(decidir(40 * MS, 0, Some(0), true, m), Decision::Nuevo);
    }

    #[test]
    fn la_espera_llega_justo_a_lo_siguiente() {
        let m = minimo_entre(15);
        assert_eq!(espera(10 * MS, 0, Some(0), true, m), m - 10 * MS);
        assert_eq!(espera(10 * MS, 0, Some(0), false, m), LATIDO - 10 * MS);
        assert_eq!(espera(0, 0, None, false, m), ESPERA_PRIMERO);
        // Caso negativo: nunca negativa, aunque ya toque.
        assert_eq!(espera(10 * S, 0, Some(0), false, m), 0);
        assert_eq!(espera(10 * S, 0, None, true, m), 0);
    }

    #[test]
    fn el_medidor_cuenta_lo_del_ultimo_segundo() {
        let mut med = Medidor::default();
        for i in 0..15 {
            med.uno(i * S / 15);
        }
        // Al cerrar el segundo: quince.
        let fps = med.por_segundo(S);
        assert!((fps - 15.0).abs() < 0.01, "{fps}");
        // Caso negativo: con la pantalla quieta mucho rato, cero, no lo de
        // antes.
        assert_eq!(med.por_segundo(5 * S), 0.0);
    }
}
