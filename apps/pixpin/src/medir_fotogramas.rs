//! Cuanto cuesta cada fotograma del editor, para medirlo en el equipo del
//! usuario (D129).
//!
//! La spec tiene tres sospechas de por que el lienzo va lento (vsync, un
//! `WM_PAINT` que no llega, recalcular el trazo entero) y cada una deja una
//! huella distinta en estos numeros. Se decide con ellos, no a ojo.
//!
//! Sin memoria dinamica: son sumas y maximos en campos fijos, y el registro
//! es una linea de `tracing` cada 60 fotogramas.

use std::time::Duration;

/// Cada cuantos fotogramas se escribe una linea.
pub const FOTOGRAMAS_POR_LINEA: u32 = 60;

/// Lo que costo un fotograma que si se pinto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pintado {
    /// Desde empezar hasta justo antes de presentar.
    pub pintar: Duration,
    /// `presentar_sincronizado`: con vsync, aqui se nota la espera al refresco.
    pub presentar: Duration,
}

/// Una vuelta del bucle del editor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vuelta {
    /// `RatonMovido` y `Muestra` recibidos en la vuelta.
    pub puntos: u32,
    /// Bombear mensajes y pasar la cola al gesto.
    pub vaciar: Duration,
    /// `None` si en esta vuelta no se pinto.
    pub pintado: Option<Pintado>,
    /// Dentro de `esperar_eventos`.
    pub esperar: Duration,
}

/// Media y maximo de una magnitud en una linea.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Campo {
    pub media: f32,
    pub maximo: f32,
}

/// Una linea del registro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resumen {
    pub fotogramas: u32,
    pub puntos: Campo,
    pub vaciar_ms: Campo,
    pub pintar_ms: Campo,
    pub presentar_ms: Campo,
    pub esperar_ms: Campo,
}

impl Resumen {
    /// Escribe la linea. Aparte de `anotar` para que las pruebas no dependan
    /// de un suscriptor de `tracing`.
    pub fn registrar(&self) {
        tracing::info!(
            fotogramas = self.fotogramas,
            puntos_media = self.puntos.media,
            puntos_max = self.puntos.maximo,
            vaciar_ms = self.vaciar_ms.media,
            vaciar_ms_max = self.vaciar_ms.maximo,
            pintar_ms = self.pintar_ms.media,
            pintar_ms_max = self.pintar_ms.maximo,
            presentar_ms = self.presentar_ms.media,
            presentar_ms_max = self.presentar_ms.maximo,
            esperar_ms = self.esperar_ms.media,
            esperar_ms_max = self.esperar_ms.maximo,
            "fotogramas del editor"
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Suma {
    total: f64,
    maximo: f32,
}

impl Suma {
    fn sumar(&mut self, v: f32) {
        self.total += v as f64;
        self.maximo = self.maximo.max(v);
    }

    fn campo(&self, n: u32) -> Campo {
        Campo {
            media: (self.total / n as f64) as f32,
            maximo: self.maximo,
        }
    }
}

fn ms(d: Duration) -> f32 {
    d.as_secs_f32() * 1000.0
}

/// El acumulador. Apagado, `anotar` vuelve enseguida y no guarda nada.
#[derive(Debug, Clone, Default)]
pub struct MedidorFotogramas {
    activo: bool,
    fotogramas: u32,
    // Lo que llevan las vueltas del fotograma que aun no se ha pintado.
    puntos_en_curso: u32,
    vaciar_en_curso: f32,
    esperar_en_curso: f32,
    puntos: Suma,
    vaciar: Suma,
    pintar: Suma,
    presentar: Suma,
    esperar: Suma,
}

impl MedidorFotogramas {
    pub fn nuevo(activo: bool) -> Self {
        Self {
            activo,
            ..Default::default()
        }
    }

    /// Apunta una vuelta. Devuelve la linea cuando se completan
    /// `FOTOGRAMAS_POR_LINEA` fotogramas pintados; entonces empieza de cero.
    ///
    /// Las vueltas que no pintan acumulan sus puntos, su vaciado y su espera
    /// en el fotograma siguiente: con un raton de 1000 Hz hay muchas, y
    /// contarlas como fotogramas escondería justo la sospecha S2.
    pub fn anotar(&mut self, v: Vuelta) -> Option<Resumen> {
        if !self.activo {
            return None;
        }
        self.puntos_en_curso = self.puntos_en_curso.saturating_add(v.puntos);
        self.vaciar_en_curso += ms(v.vaciar);
        self.esperar_en_curso += ms(v.esperar);
        let pintado = v.pintado?;
        self.puntos.sumar(self.puntos_en_curso as f32);
        self.vaciar.sumar(self.vaciar_en_curso);
        self.esperar.sumar(self.esperar_en_curso);
        self.pintar.sumar(ms(pintado.pintar));
        self.presentar.sumar(ms(pintado.presentar));
        self.puntos_en_curso = 0;
        self.vaciar_en_curso = 0.0;
        self.esperar_en_curso = 0.0;
        self.fotogramas += 1;
        if self.fotogramas < FOTOGRAMAS_POR_LINEA {
            return None;
        }
        let n = self.fotogramas;
        let r = Resumen {
            fotogramas: n,
            puntos: self.puntos.campo(n),
            vaciar_ms: self.vaciar.campo(n),
            pintar_ms: self.pintar.campo(n),
            presentar_ms: self.presentar.campo(n),
            esperar_ms: self.esperar.campo(n),
        };
        *self = Self::nuevo(true);
        Some(r)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn vuelta(puntos: u32, pintar_ms: u64) -> Vuelta {
        Vuelta {
            puntos,
            vaciar: Duration::from_millis(1),
            pintado: Some(Pintado {
                pintar: Duration::from_millis(pintar_ms),
                presentar: Duration::from_millis(10),
            }),
            esperar: Duration::from_millis(3),
        }
    }

    #[test]
    fn apagado_no_registra_nada_por_muchos_fotogramas_que_pasen() {
        let mut m = MedidorFotogramas::nuevo(false);
        for i in 0..600 {
            assert_eq!(m.anotar(vuelta(5, 2)), None, "vuelta {i}");
        }
    }

    #[test]
    fn encendido_da_una_linea_cada_sesenta_fotogramas_con_medias_y_maximos() {
        let mut m = MedidorFotogramas::nuevo(true);
        for i in 0..59 {
            let pintar = if i == 7 { 8 } else { 2 };
            assert_eq!(m.anotar(vuelta(5, pintar)), None, "fotograma {i}");
        }
        let r = m
            .anotar(vuelta(5, 2))
            .expect("el fotograma 60 cierra la linea");
        assert_eq!(r.fotogramas, FOTOGRAMAS_POR_LINEA);
        assert!((r.puntos.media - 5.0).abs() < 1e-3);
        assert_eq!(r.puntos.maximo, 5.0);
        // (59 x 2 + 8) / 60 = 2,1
        assert!((r.pintar_ms.media - 2.1).abs() < 1e-2, "{:?}", r.pintar_ms);
        assert!((r.pintar_ms.maximo - 8.0).abs() < 1e-3);
        assert!((r.presentar_ms.media - 10.0).abs() < 1e-3);
        assert!((r.vaciar_ms.media - 1.0).abs() < 1e-3);
        assert!((r.esperar_ms.media - 3.0).abs() < 1e-3);
        // Y vuelve a empezar de cero: la segunda linea no arrastra el maximo.
        for _ in 0..59 {
            assert_eq!(m.anotar(vuelta(1, 1)), None);
        }
        let r2 = m.anotar(vuelta(1, 1)).expect("segunda linea");
        assert!((r2.pintar_ms.maximo - 1.0).abs() < 1e-3);
    }

    #[test]
    fn las_vueltas_sin_fotograma_suman_al_siguiente_fotograma() {
        // Con un raton de 1000 Hz hay vueltas que vacian la cola y no pintan:
        // sus puntos y su espera son del fotograma que por fin se pinta.
        let mut m = MedidorFotogramas::nuevo(true);
        let sin_pintar = Vuelta {
            puntos: 7,
            vaciar: Duration::from_millis(1),
            pintado: None,
            esperar: Duration::from_millis(3),
        };
        assert_eq!(m.anotar(sin_pintar), None);
        assert_eq!(m.anotar(sin_pintar), None);
        for _ in 0..59 {
            let _ = m.anotar(vuelta(0, 2));
        }
        let r = m.anotar(vuelta(0, 2)).expect("sesenta fotogramas");
        assert_eq!(r.puntos.maximo, 14.0);
        assert!((r.esperar_ms.maximo - 9.0).abs() < 1e-3);
    }
}
