//! Cuanto cuesta cada fotograma del editor, para medirlo en el equipo del
//! usuario (D129).
//!
//! La spec tiene tres sospechas de por que el lienzo va lento (vsync, un
//! `WM_PAINT` que no llega, recalcular el trazo entero) y cada una deja una
//! huella distinta en estos numeros. Se decide con ellos, no a ojo.
//!
//! **Por clase de fotograma** (2026-09-23). El usuario dijo «al principio va
//! rapido y al cabo de un minuto dibujar va lento». Una sola media de
//! «pintar» mezclaba cuatro fotogramas que no se parecen en nada —el del
//! trazo en su capa (cuesta lo que mide el trazo), el de soltar el lapiz
//! (lo que mide el trazo nuevo), el parcial sobre la capa congelada y el de
//! la escena entera (lo que haya a la vista)— y lo que crece con el dibujo
//! quedaba diluido entre los que no crecen. Ahora cada clase lleva su media
//! y su maximo, y la linea dice ademas cuantos elementos se recorrieron, de
//! cuantos puntos era el trazo vivo y cuantas geometrias se teselaron: con
//! eso se ve en su equipo QUE fase crece y con QUE.
//!
//! Sin memoria dinamica: son sumas y maximos en campos fijos, y el registro
//! es una linea de `tracing` cada 60 fotogramas.

use std::time::Duration;

/// Cada cuantos fotogramas se escribe una linea.
pub const FOTOGRAMAS_POR_LINEA: u32 = 60;

/// Que clase de fotograma fue. Cada una cuesta por una razon distinta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Clase {
    /// La escena entera: recorre todo lo visible. Es el que crece con el
    /// dibujo, y el que no deberia salir mientras se dibuja.
    #[default]
    Escena,
    /// Solo una zona, sobre la capa congelada (arrastrar, trazar sin B2).
    Zona,
    /// B2: el trazo en curso en su capa. Crece con los puntos del trazo.
    Tinta,
    /// Soltar el lapiz: solo el trazo nuevo, sobre lo que ya se veia.
    Horneado,
}

const CLASES: usize = 4;

impl Clase {
    fn indice(self) -> usize {
        match self {
            Clase::Escena => 0,
            Clase::Zona => 1,
            Clase::Tinta => 2,
            Clase::Horneado => 3,
        }
    }
}

/// Lo que costo un fotograma que si se pinto.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pintado {
    /// Desde empezar hasta justo antes de presentar.
    pub pintar: Duration,
    /// `presentar_sincronizado`: con vsync, aqui se nota la espera al refresco.
    pub presentar: Duration,
    pub clase: Clase,
    /// Cuantos elementos se recorrieron para pintarlo (los candidatos de la
    /// rejilla en `Escena`/`Zona`; 1 en `Tinta` y `Horneado`).
    pub visibles: u32,
    /// Cuantos puntos tenia el trazo que se pinto en su capa (`Tinta`), o el
    /// que se horneo al soltar. 0 si no habia trazo.
    pub puntos_trazo: u32,
    /// Cuantas geometrias nuevas se teselaron para la cache (resta de
    /// `CacheTinta::realizadas` antes y despues).
    pub teseladas: u32,
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

/// Lo de una clase de fotograma en una linea.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PorClase {
    pub cuantos: u32,
    pub pintar_ms: Campo,
    pub visibles: Campo,
    pub puntos_trazo: Campo,
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
    pub escena: PorClase,
    pub zona: PorClase,
    pub tinta: PorClase,
    pub horneado: PorClase,
    /// Geometrias teseladas en toda la linea.
    pub teseladas: u32,
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
            teseladas = self.teseladas,
            escena_n = self.escena.cuantos,
            escena_ms = self.escena.pintar_ms.media,
            escena_ms_max = self.escena.pintar_ms.maximo,
            escena_visibles = self.escena.visibles.media,
            escena_visibles_max = self.escena.visibles.maximo,
            zona_n = self.zona.cuantos,
            zona_ms = self.zona.pintar_ms.media,
            zona_ms_max = self.zona.pintar_ms.maximo,
            tinta_n = self.tinta.cuantos,
            tinta_ms = self.tinta.pintar_ms.media,
            tinta_ms_max = self.tinta.pintar_ms.maximo,
            tinta_puntos = self.tinta.puntos_trazo.media,
            tinta_puntos_max = self.tinta.puntos_trazo.maximo,
            soltar_n = self.horneado.cuantos,
            soltar_ms = self.horneado.pintar_ms.media,
            soltar_ms_max = self.horneado.pintar_ms.maximo,
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
        if n == 0 {
            return Campo::default();
        }
        Campo {
            media: (self.total / n as f64) as f32,
            maximo: self.maximo,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct SumaClase {
    cuantos: u32,
    pintar: Suma,
    visibles: Suma,
    puntos_trazo: Suma,
}

impl SumaClase {
    fn resumen(&self) -> PorClase {
        PorClase {
            cuantos: self.cuantos,
            pintar_ms: self.pintar.campo(self.cuantos),
            visibles: self.visibles.campo(self.cuantos),
            puntos_trazo: self.puntos_trazo.campo(self.cuantos),
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
    clases: [SumaClase; CLASES],
    teseladas: u32,
}

impl MedidorFotogramas {
    /// Se enciende con `[rendimiento] medir_fotogramas` del TOML o con la
    /// variable de entorno `PIXPIN_MEDIR_FOTOGRAMAS`.
    ///
    /// La variable existe para poder pedirle una medida al usuario sin
    /// tocarle el fichero de ajustes: se arranca una vez con ella puesta, se
    /// miran las lineas «fotogramas del editor» del registro y se cierra. Un
    /// ajuste que hay que poner y luego acordarse de quitar se queda puesto.
    pub fn nuevo(activo: bool) -> Self {
        let activo = activo || std::env::var_os("PIXPIN_MEDIR_FOTOGRAMAS").is_some();
        Self {
            activo,
            ..Default::default()
        }
    }

    /// Si esta encendido: quien llama se ahorra contar lo que nadie leera.
    pub fn activo(&self) -> bool {
        self.activo
    }

    /// Apunta una vuelta. Devuelve la linea cuando se completan
    /// `FOTOGRAMAS_POR_LINEA` fotogramas pintados; entonces empieza de cero.
    ///
    /// Las vueltas que no pintan acumulan sus puntos, su vaciado y su espera
    /// en el fotograma siguiente: con un raton de 1000 Hz hay muchas, y
    /// contarlas como fotogramas esconderia justo la sospecha S2.
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
        let c = &mut self.clases[pintado.clase.indice()];
        c.cuantos += 1;
        c.pintar.sumar(ms(pintado.pintar));
        c.visibles.sumar(pintado.visibles as f32);
        c.puntos_trazo.sumar(pintado.puntos_trazo as f32);
        self.teseladas = self.teseladas.saturating_add(pintado.teseladas);
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
            escena: self.clases[Clase::Escena.indice()].resumen(),
            zona: self.clases[Clase::Zona.indice()].resumen(),
            tinta: self.clases[Clase::Tinta.indice()].resumen(),
            horneado: self.clases[Clase::Horneado.indice()].resumen(),
            teseladas: self.teseladas,
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
                ..Default::default()
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

    #[test]
    fn cada_clase_de_fotograma_lleva_su_media_sus_visibles_y_su_trazo() {
        // Lo que tiene que dejar ver en el equipo del usuario QUE crece: el
        // fotograma de la escena entera con lo visible, el del trazo vivo
        // con sus puntos. Mezclados en una sola media no se distinguian.
        let mut m = MedidorFotogramas::nuevo(true);
        let con = |clase, pintar_ms, visibles, puntos_trazo| Vuelta {
            puntos: 1,
            vaciar: Duration::ZERO,
            pintado: Some(Pintado {
                pintar: Duration::from_millis(pintar_ms),
                presentar: Duration::ZERO,
                clase,
                visibles,
                puntos_trazo,
                teseladas: 2,
            }),
            esperar: Duration::ZERO,
        };
        for _ in 0..50 {
            assert_eq!(m.anotar(con(Clase::Tinta, 1, 1, 400)), None);
        }
        for _ in 0..9 {
            assert_eq!(m.anotar(con(Clase::Escena, 30, 300, 0)), None);
        }
        let r = m
            .anotar(con(Clase::Horneado, 2, 1, 900))
            .expect("sesenta fotogramas");
        assert_eq!(r.tinta.cuantos, 50);
        assert!((r.tinta.pintar_ms.media - 1.0).abs() < 1e-3);
        assert!((r.tinta.puntos_trazo.media - 400.0).abs() < 1e-3);
        assert_eq!(r.escena.cuantos, 9);
        assert!((r.escena.pintar_ms.media - 30.0).abs() < 1e-3);
        assert!((r.escena.visibles.maximo - 300.0).abs() < 1e-3);
        assert_eq!(r.horneado.cuantos, 1);
        assert!((r.horneado.puntos_trazo.maximo - 900.0).abs() < 1e-3);
        assert_eq!(r.teseladas, 120);
        // Caso negativo: la clase que no salio da ceros, no un NaN de 0/0
        // que en el registro se leeria como un fotograma infinito.
        assert_eq!(r.zona.cuantos, 0);
        assert_eq!(r.zona.pintar_ms, Campo::default());
    }
}
