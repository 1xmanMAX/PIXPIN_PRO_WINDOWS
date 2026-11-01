//! El estado de lo que suena, y las cuentas que lo mueven.
//!
//! Es el `Reproductor.Estado` del movil (`guardados/Reproductor.kt:23-30`)
//! traido tal cual, pero **sin tocar la tarjeta de sonido**: aqui solo hay
//! numeros. Asi lo dificil de verdad —que la velocidad cicle en el orden
//! bueno, que saltar al final no se pase, que `1:02:09` no salga como
//! `62:09`— se comprueba sin altavoces, sin fichero y sin Windows.
//!
//! Quien manda de verdad sobre la posicion es la tarjeta: `salida.rs` la
//! pregunta y la escribe aqui. Este modulo decide **a donde** hay que ir, no
//! lleva la cuenta por su cuenta.

use std::path::PathBuf;

/// Cuanto se salta con los botones de adelante y atras.
///
/// `Reproductor.SALTO_MS` (`Reproductor.kt:35`). Los botones del movil se
/// llaman `Replay10`/`Forward10` justo por esto.
pub const SALTO_MS: i64 = 10_000;

/// Las velocidades por las que pasa el boton, en orden y en ciclo.
///
/// `Reproductor.VELOCIDADES` (`Reproductor.kt:38`). **Son cinco**, y la
/// quinta es mas lenta que la primera a proposito: quien va acelerando llega
/// a `2` y, un toque mas, se planta en `0.75` para entender una palabra que
/// no cogio. Si se escribieran de menor a mayor habria que dar cuatro toques
/// para volver de `2` a `1`.
pub const VELOCIDADES: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 0.75];

/// Cada cuanto se pregunta a la tarjeta por donde va.
///
/// El latido del movil (`Reproductor.kt:48`). Cuatro veces por segundo basta
/// para que la barra no vaya a saltos y no cuesta nada.
pub const MS_ENTRE_LATIDOS: u64 = 250;

/// Lo que se sabe de lo que suena. `ruta` vacia: no hay nada cargado y la
/// barra no se pinta (`BarraDelReproductor.kt:48`).
#[derive(Debug, Clone, PartialEq)]
pub struct Estado {
    pub ruta: Option<PathBuf>,
    pub titulo: String,
    pub sonando: bool,
    pub posicion_ms: i64,
    pub duracion_ms: i64,
    pub velocidad: f32,
}

impl Default for Estado {
    fn default() -> Self {
        Estado {
            ruta: None,
            titulo: String::new(),
            sonando: false,
            posicion_ms: 0,
            duracion_ms: 0,
            // Uno, no cero: un estado recien nacido con velocidad cero
            // dejaria la primera nota muda sin que nadie lo hubiera pedido.
            velocidad: 1.0,
        }
    }
}

impl Estado {
    /// Un estado vacio que **conserva la velocidad**.
    ///
    /// Es lo que hace `parar()` en el movil (`Reproductor.kt:105`): quien
    /// escucha a `1.5` todas las notas no quiere volver a ponerlo cada vez.
    pub fn vacio_con_velocidad(velocidad: f32) -> Estado {
        Estado {
            velocidad,
            ..Estado::default()
        }
    }

    /// Por donde va, de 0 a 1. `Reproductor.fraccion()` (:138-141).
    pub fn fraccion(&self) -> f32 {
        fraccion(self.posicion_ms, self.duracion_ms)
    }
}

/// Por donde va, de 0 a 1.
///
/// Con duracion desconocida o cero devuelve 0 en vez de dividir entre cero:
/// una nota cuyos metadatos aun no han llegado pinta la barra al principio,
/// que es donde esta.
pub fn fraccion(posicion_ms: i64, duracion_ms: i64) -> f32 {
    if duracion_ms <= 0 {
        return 0.0;
    }
    (posicion_ms as f32 / duracion_ms as f32).clamp(0.0, 1.0)
}

/// La siguiente velocidad del ciclo.
///
/// Una velocidad que no este en la lista —un ajuste viejo, un numero
/// escrito a mano— devuelve la primera, igual que en el movil, donde
/// `indexOf` da `-1` y `(-1 + 1) % 5` cae en el indice 0
/// (`Reproductor.kt:128-135`). Se compara con holgura porque `1.25f` leido
/// de un fichero y `1.25f` escrito en el codigo pueden no ser el mismo bit.
pub fn siguiente_velocidad(actual: f32) -> f32 {
    let indice = VELOCIDADES
        .iter()
        .position(|v| (v - actual).abs() < 1e-4)
        .map(|i| (i + 1) % VELOCIDADES.len())
        .unwrap_or(0);
    VELOCIDADES[indice]
}

/// A donde lleva saltar `ms` milisegundos (negativo para atras).
///
/// `Reproductor.saltar` (:109-115). Se acota a `[0, duracion]`, y con
/// duracion negativa —que no deberia darse, pero llega de un `resto` ajeno—
/// el tope es cero y no un numero al reves, que dejaria el rango vacio.
pub fn destino_al_saltar(posicion_ms: i64, ms: i64, duracion_ms: i64) -> i64 {
    let tope = duracion_ms.max(0);
    posicion_ms.saturating_add(ms).clamp(0, tope)
}

/// A donde lleva pinchar en la fraccion `f` de la barra, de 0 a 1.
///
/// `None` cuando todavia no se sabe cuanto dura: pinchar en la mitad de una
/// barra sin duracion no significa nada, y mandar a cero seria rebobinar sin
/// que nadie lo pidiera (`Reproductor.irA` :118-125).
pub fn destino_de_fraccion(f: f32, duracion_ms: i64) -> Option<i64> {
    if duracion_ms <= 0 {
        return None;
    }
    let f = if f.is_nan() { 0.0 } else { f.clamp(0.0, 1.0) };
    Some((duracion_ms as f64 * f as f64) as i64)
}

/// La duracion escrita como se lee: `0:07`, `1:24`, `1:02:09`.
///
/// `duracionLegible` de `pin/Voz.kt:150-162`. Con horas hay que decirlas:
/// sin esto una grabacion de dos horas se rotulaba `120:00`, que no es una
/// duracion sino una cuenta sin acabar.
pub fn duracion_legible(ms: i64) -> String {
    let total = (ms / 1000).max(0);
    let segundos = total % 60;
    let minutos = (total / 60) % 60;
    let horas = total / 3600;
    if horas > 0 {
        format!("{horas}:{minutos:02}:{segundos:02}")
    } else {
        format!("{minutos}:{segundos:02}")
    }
}

/// La velocidad escrita como se lee en espanol: `1,5×`.
///
/// `velocidadLegible` (`BarraDelReproductor.kt:113-114`), carcater a
/// caracter: coma decimal, ceros de la derecha fuera (`1,5×` y no `1,50×`)
/// y el signo de multiplicar, no una equis.
pub fn velocidad_legible(v: f32) -> String {
    if (v - v.trunc()).abs() < 1e-4 {
        return format!("{}\u{d7}", v.trunc() as i64);
    }
    let mut texto = format!("{v}").replace('.', ",");
    while texto.ends_with('0') {
        texto.pop();
    }
    format!("{texto}\u{d7}")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_velocidades_son_cinco_y_ciclan_en_el_orden_del_movil() {
        assert_eq!(VELOCIDADES.len(), 5);
        let mut v = 1.0;
        let mut vistas = Vec::new();
        for _ in 0..5 {
            v = siguiente_velocidad(v);
            vistas.push(v);
        }
        assert_eq!(vistas, vec![1.25, 1.5, 2.0, 0.75, 1.0]);
    }

    #[test]
    fn una_velocidad_que_no_esta_en_la_lista_vuelve_a_la_primera() {
        assert_eq!(siguiente_velocidad(3.0), 1.0);
        assert_eq!(siguiente_velocidad(0.0), 1.0);
        assert_eq!(siguiente_velocidad(f32::NAN), 1.0);
    }

    #[test]
    fn saltar_no_se_sale_por_ninguno_de_los_dos_extremos() {
        assert_eq!(destino_al_saltar(2_000, -SALTO_MS, 30_000), 0);
        assert_eq!(destino_al_saltar(25_000, SALTO_MS, 30_000), 30_000);
        assert_eq!(destino_al_saltar(5_000, SALTO_MS, 30_000), 15_000);
    }

    #[test]
    fn saltar_en_un_audio_de_cero_segundos_se_queda_en_cero() {
        assert_eq!(destino_al_saltar(0, SALTO_MS, 0), 0);
        // Una duracion negativa llega de un `resto` escrito por otro: el
        // tope es cero, nunca un rango al reves.
        assert_eq!(destino_al_saltar(0, SALTO_MS, -5), 0);
    }

    #[test]
    fn pinchar_en_la_barra_de_un_audio_sin_duracion_no_mueve_nada() {
        assert_eq!(destino_de_fraccion(0.5, 0), None);
        assert_eq!(destino_de_fraccion(0.5, -1), None);
    }

    #[test]
    fn pinchar_en_la_barra_lleva_al_punto_proporcional() {
        assert_eq!(destino_de_fraccion(0.0, 30_000), Some(0));
        assert_eq!(destino_de_fraccion(0.5, 30_000), Some(15_000));
        assert_eq!(destino_de_fraccion(1.0, 30_000), Some(30_000));
        // Fuera de rango y NaN se acotan: un dedo que se sale del control
        // no puede mandar a un milisegundo imposible.
        assert_eq!(destino_de_fraccion(2.0, 30_000), Some(30_000));
        assert_eq!(destino_de_fraccion(-1.0, 30_000), Some(0));
        assert_eq!(destino_de_fraccion(f32::NAN, 30_000), Some(0));
    }

    #[test]
    fn la_fraccion_de_un_audio_sin_duracion_es_cero_y_no_una_division_entre_cero() {
        assert_eq!(fraccion(1_000, 0), 0.0);
        assert_eq!(fraccion(15_000, 30_000), 0.5);
    }

    #[test]
    fn la_duracion_se_escribe_como_se_lee() {
        assert_eq!(duracion_legible(7_000), "0:07");
        assert_eq!(duracion_legible(84_000), "1:24");
        assert_eq!(duracion_legible(3_729_000), "1:02:09");
        // El redondeo mal hecho es lo que da `0:60`, que se ve enseguida.
        assert_eq!(duracion_legible(59_999), "0:59");
        assert_eq!(duracion_legible(60_000), "1:00");
        // Negativo: un `duracionMs` roto no puede pintar `-1:-1`.
        assert_eq!(duracion_legible(-5), "0:00");
    }

    #[test]
    fn la_velocidad_se_escribe_con_coma() {
        assert_eq!(velocidad_legible(1.0), "1\u{d7}");
        assert_eq!(velocidad_legible(2.0), "2\u{d7}");
        assert_eq!(velocidad_legible(1.25), "1,25\u{d7}");
        assert_eq!(velocidad_legible(1.5), "1,5\u{d7}");
        assert_eq!(velocidad_legible(0.75), "0,75\u{d7}");
    }

    #[test]
    fn parar_conserva_la_velocidad_elegida() {
        let e = Estado::vacio_con_velocidad(1.5);
        assert!(e.ruta.is_none());
        assert!(!e.sonando);
        assert_eq!(e.velocidad, 1.5);
    }
}
