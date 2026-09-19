//! Moverse por el universo como por un mapa, no como por un dibujo.
//!
//! En un lienzo, arrastrar en el vacio es elegir con la marquesina y la
//! rueda desplaza (Excalidraw). En el universo lo que mas se hace es MIRAR:
//! arrastrar el vacio lleva el cielo, la rueda acerca y aleja donde esta el
//! raton, suave, y un arrastre rapido sigue un poco al soltarlo. Todo esto
//! solo con el universo detras: el editor lo pregunta aqui y un lienzo
//! normal no pasa nunca por este modulo.
//!
//! Puro: sin ventana ni reloj propio. Quien lo usa le da los tiempos.

use pixpin_geom::Punto;
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::vector::Punto2;

/// Cuanto se come la persecucion del zoom por segundo, como en el zoom de
/// los pines (`pixpin_pin::zoom`): `1 - exp(-VELOCIDAD * dt)`. Con 18, un
/// fotograma de 60 Hz recorre el 26 % de lo que falta y en 150 ms se ha
/// llegado: suave, sin sentirse lento.
pub const VELOCIDAD_ZOOM: f32 = 18.0;
/// Cuanto acerca una muesca de la rueda (`WHEEL_DELTA` = 120).
pub const FACTOR_MUESCA: f32 = 1.25;
/// Lo que tarda la inercia en caer a un tercio (constante de tiempo).
pub const INERCIA_TAU: f32 = 0.12;
/// Por debajo de esta velocidad al soltar (pixeles logicos por segundo) no
/// hay inercia: un arrastre lento se queda donde se solto.
pub const INERCIA_MINIMA: f32 = 400.0;
/// La inercia se apaga por debajo de esto.
const INERCIA_PARADA: f32 = 20.0;
/// Solo cuentan los movimientos de los ultimos milisegundos para medir la
/// velocidad al soltar: si la mano se paro antes de soltar, no hay inercia.
const VENTANA_VELOCIDAD_MS: f64 = 80.0;
/// Lo que se mueve la camara con una flecha, en pixeles logicos.
pub const PASO_FLECHA: f32 = 120.0;
/// Pasos de tiempo que se aceptan: uno mayor (la ventana estuvo parada)
/// daria un salto.
const DT_MAXIMO: f32 = 0.05;

pub const VK_IZQUIERDA: u32 = 0x25;
pub const VK_ARRIBA: u32 = 0x26;
pub const VK_DERECHA: u32 = 0x27;
pub const VK_ABAJO: u32 = 0x28;

/// El factor de zoom de un giro de rueda: proporcional, igual a cualquier
/// distancia (0,002 o 30), y fraccionario para los paneles tactiles que
/// mandan giros pequenos.
pub fn factor_de_rueda(delta: i32) -> f32 {
    FACTOR_MUESCA.powf(delta as f32 / 120.0)
}

/// Que hace una flecha: desplazar la camara (pixeles logicos, convenio de
/// `Camara::desplazar`: el cielo sigue a la flecha al reves, como al
/// arrastrarlo, asi que la flecha derecha lleva la vista a la derecha).
pub fn desplazamiento_de_flecha(vk: u32) -> Option<(f32, f32)> {
    match vk {
        VK_IZQUIERDA => Some((PASO_FLECHA, 0.0)),
        VK_DERECHA => Some((-PASO_FLECHA, 0.0)),
        VK_ARRIBA => Some((0.0, PASO_FLECHA)),
        VK_ABAJO => Some((0.0, -PASO_FLECHA)),
        _ => None,
    }
}

/// `+` acerca y `-` aleja, como una muesca de rueda en el centro. `=` es el
/// `+` sin mayusculas en los teclados ingleses.
pub fn delta_de_caracter(c: char) -> Option<i32> {
    match c {
        '+' | '=' => Some(120),
        '-' | '_' => Some(-120),
        _ => None,
    }
}

/// Lo que se sabe de un clic en el lienzo con el universo detras, para
/// decidir si empieza a arrastrar el cielo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Clic {
    /// La sesion del universo lo dejo pasar: no cayo en un astro, una
    /// nebulosa ni un panel suyo.
    pub libre_de_astros: bool,
    /// Con la mano del editor (la de elegir). Con el lapiz o una forma, el
    /// clic es para dibujar.
    pub mano: bool,
    pub shift: bool,
    /// Cae sobre una anotacion, o sobre la seleccion de anotaciones (sus
    /// tiradores).
    pub sobre_anotacion: bool,
}

/// Arrastrar el vacio con la mano lleva el cielo. Shift+arrastrar sigue
/// siendo la marquesina, y sobre una anotacion se sigue eligiendo y
/// moviendo como en cualquier lienzo.
pub fn arrastra_el_cielo(c: Clic) -> bool {
    c.libre_de_astros && c.mano && !c.shift && !c.sobre_anotacion
}

/// Un arrastre del cielo en curso.
#[derive(Debug, Clone, PartialEq)]
pub struct Arrastre {
    anterior: Punto,
    /// `(ms, dx, dy)` de los ultimos movimientos, en pixeles logicos.
    muestras: Vec<(f64, f32, f32)>,
}

impl Arrastre {
    pub fn nuevo(p: Punto) -> Self {
        Self {
            anterior: p,
            muestras: Vec::new(),
        }
    }

    /// Un movimiento a `p` (pixeles fisicos) en el instante `ms`: lo que hay
    /// que desplazar la camara, en pixeles logicos.
    pub fn mover(&mut self, p: Punto, ms: f64, escala: f32) -> (f32, f32) {
        let dx = (p.x - self.anterior.x) as f32 / escala;
        let dy = (p.y - self.anterior.y) as f32 / escala;
        self.anterior = p;
        self.muestras.push((ms, dx, dy));
        self.muestras
            .retain(|(t, _, _)| ms - *t <= VENTANA_VELOCIDAD_MS);
        (dx, dy)
    }

    /// La velocidad al soltar en `ms` (pixeles logicos por segundo), si da
    /// para seguir con inercia.
    pub fn soltar(&self, ms: f64) -> Option<(f32, f32)> {
        let recientes: Vec<_> = self
            .muestras
            .iter()
            .filter(|(t, _, _)| ms - *t <= VENTANA_VELOCIDAD_MS)
            .collect();
        let primero = recientes.first()?.0;
        let (sx, sy) = recientes
            .iter()
            .fold((0.0, 0.0), |(x, y), (_, dx, dy)| (x + dx, y + dy));
        // Desde el primer movimiento de la ventana hasta soltar: si la mano
        // se quedo quieta un rato antes, la velocidad cae sola.
        let dt = ((ms - primero) as f32 / 1000.0).max(1.0 / 120.0);
        let v = (sx / dt, sy / dt);
        (v.0.hypot(v.1) >= INERCIA_MINIMA).then_some(v)
    }
}

/// Lo que sigue moviendose sin eventos: el zoom que persigue su objetivo y
/// la inercia de un arrastre soltado deprisa.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Suave {
    /// El zoom al que se va y el punto (pixeles logicos de la ventana) que
    /// se queda quieto mientras.
    zoom: Option<(f32, Punto2)>,
    /// Pixeles logicos por segundo.
    inercia: Option<(f32, f32)>,
}

impl Suave {
    pub fn activo(&self) -> bool {
        self.zoom.is_some() || self.inercia.is_some()
    }

    /// Una muesca (o un trozo de muesca) de rueda con el raton en `foco`.
    /// Se suma al objetivo que ya hubiera: tres muescas seguidas son tres
    /// pasos aunque la camara aun no haya llegado al primero.
    pub fn pedir_zoom(&mut self, actual: f32, foco: Punto2, delta: i32, minimo: f32, maximo: f32) {
        let desde = self.zoom.map_or(actual, |(z, _)| z);
        let objetivo = (desde * factor_de_rueda(delta)).clamp(minimo, maximo);
        self.zoom = Some((objetivo, foco));
    }

    pub fn empujar(&mut self, velocidad: (f32, f32)) {
        self.inercia = Some(velocidad);
    }

    /// Un arrastre nuevo para lo que quedara de inercia: la mano manda.
    pub fn parar_inercia(&mut self) {
        self.inercia = None;
    }

    /// Avanza `dt` segundos. Devuelve si la camara cambio.
    pub fn avanzar(&mut self, camara: &mut Camara, dt: f32, minimo: f32, maximo: f32) -> bool {
        let dt = dt.clamp(0.0, DT_MAXIMO);
        let mut cambio = false;
        if let Some((objetivo, foco)) = self.zoom {
            let actual = camara.zoom.max(f32::EPSILON);
            let falta = (objetivo / actual).ln();
            // En escala logaritmica: de 0,01 a 0,02 se siente igual que de
            // 1 a 2.
            let paso = if falta.abs() < 0.002 {
                self.zoom = None;
                falta
            } else {
                falta * (1.0 - (-VELOCIDAD_ZOOM * dt).exp())
            };
            cambio |= camara.acercar_en_entre(foco, paso.exp(), minimo, maximo);
        }
        if let Some((vx, vy)) = self.inercia {
            camara.desplazar(vx * dt, vy * dt);
            cambio = true;
            let caida = (-dt / INERCIA_TAU).exp();
            let v = (vx * caida, vy * caida);
            self.inercia = (v.0.hypot(v.1) >= INERCIA_PARADA).then_some(v);
        }
        cambio
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn camara() -> Camara {
        Camara {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }

    #[test]
    fn arrastrar_el_vacio_con_la_mano_lleva_el_cielo() {
        let base = Clic {
            libre_de_astros: true,
            mano: true,
            shift: false,
            sobre_anotacion: false,
        };
        assert!(arrastra_el_cielo(base));
        // Casos negativos: cada uno de los que NO lo llevan.
        assert!(
            !arrastra_el_cielo(Clic {
                shift: true,
                ..base
            }),
            "Shift es la marquesina"
        );
        assert!(
            !arrastra_el_cielo(Clic {
                mano: false,
                ..base
            }),
            "con el lapiz se dibuja"
        );
        assert!(
            !arrastra_el_cielo(Clic {
                libre_de_astros: false,
                ..base
            }),
            "un astro se mueve"
        );
        assert!(
            !arrastra_el_cielo(Clic {
                sobre_anotacion: true,
                ..base
            }),
            "una anotacion se elige"
        );
    }

    #[test]
    fn la_rueda_acerca_proporcional_y_una_muesca_hacia_cada_lado_se_anulan() {
        assert!((factor_de_rueda(120) - FACTOR_MUESCA).abs() < 1e-6);
        assert!((factor_de_rueda(120) * factor_de_rueda(-120) - 1.0).abs() < 1e-6);
        // Un panel tactil manda trozos: dos medias muescas son una.
        let medias = factor_de_rueda(60) * factor_de_rueda(60);
        assert!((medias - FACTOR_MUESCA).abs() < 1e-5);
        // Caso negativo: sin giro, nada.
        assert_eq!(factor_de_rueda(0), 1.0);
    }

    #[test]
    fn el_zoom_suave_llega_a_su_objetivo_sin_mover_el_punto_bajo_el_raton() {
        let mut s = Suave::default();
        let mut c = camara();
        let foco = Punto2::nuevo(300.0, 200.0);
        let bajo_el_raton = c.a_mundo(foco);
        s.pedir_zoom(c.zoom, foco, 120, 0.002, 30.0);
        s.pedir_zoom(c.zoom, foco, 120, 0.002, 30.0);
        let mut pasos = 0;
        while s.activo() && pasos < 200 {
            s.avanzar(&mut c, 1.0 / 60.0, 0.002, 30.0);
            pasos += 1;
        }
        assert!(!s.activo(), "termina");
        assert!(pasos < 30, "en medio segundo: {pasos} fotogramas");
        assert!(
            (c.zoom - FACTOR_MUESCA * FACTOR_MUESCA).abs() < 1e-3,
            "{}",
            c.zoom
        );
        let despues = c.a_mundo(foco);
        assert!((despues.x - bajo_el_raton.x).abs() < 0.01);
        assert!((despues.y - bajo_el_raton.y).abs() < 0.01);
    }

    #[test]
    fn el_zoom_suave_no_pasa_de_los_topes() {
        let mut s = Suave::default();
        let mut c = Camara {
            zoom: 0.0021,
            ..camara()
        };
        for _ in 0..10 {
            s.pedir_zoom(c.zoom, Punto2::nuevo(0.0, 0.0), -120, 0.002, 30.0);
        }
        for _ in 0..100 {
            s.avanzar(&mut c, 1.0 / 60.0, 0.002, 30.0);
        }
        assert!(c.zoom >= 0.002 - 1e-7, "{}", c.zoom);
        // Caso negativo: sin nada pedido, avanzar no toca la camara.
        let mut quieta = camara();
        assert!(!Suave::default().avanzar(&mut quieta, 0.016, 0.002, 30.0));
        assert_eq!(quieta, camara());
    }

    #[test]
    fn un_arrastre_rapido_deja_inercia_y_uno_lento_o_parado_no() {
        let mut a = Arrastre::nuevo(Punto { x: 0, y: 0 });
        for i in 1..=5 {
            // 30 px cada 16 ms: casi 1.900 px/s.
            let d = a.mover(Punto { x: 30 * i, y: 0 }, 16.0 * i as f64, 1.0);
            assert_eq!(d, (30.0, 0.0));
        }
        let v = a.soltar(80.0).expect("hay inercia");
        assert!(v.0 > 1000.0 && v.1 == 0.0, "{v:?}");
        // Caso negativo: la misma mano, parada 200 ms antes de soltar.
        assert!(a.soltar(280.0).is_none());
        // Caso negativo: despacio.
        let mut lento = Arrastre::nuevo(Punto { x: 0, y: 0 });
        for i in 1..=5 {
            lento.mover(Punto { x: i, y: 0 }, 16.0 * i as f64, 1.0);
        }
        assert!(lento.soltar(80.0).is_none());
    }

    #[test]
    fn la_inercia_frena_sola_y_se_apaga() {
        let mut s = Suave::default();
        s.empujar((2000.0, 0.0));
        let mut c = camara();
        let mut t = 0;
        while s.activo() && t < 600 {
            s.avanzar(&mut c, 1.0 / 60.0, 0.002, 30.0);
            t += 1;
        }
        assert!(!s.activo(), "se para");
        assert!(t < 60, "en menos de un segundo: {t}");
        // Recorre algo mas que v * tau y menos que lo que haria sin frenar.
        let recorrido = -c.x;
        assert!(recorrido > 150.0 && recorrido < 400.0, "{recorrido}");
    }

    #[test]
    fn las_flechas_desplazan_y_mas_y_menos_acercan() {
        assert_eq!(
            desplazamiento_de_flecha(VK_DERECHA),
            Some((-PASO_FLECHA, 0.0))
        );
        assert_eq!(
            desplazamiento_de_flecha(VK_ARRIBA),
            Some((0.0, PASO_FLECHA))
        );
        assert_eq!(delta_de_caracter('+'), Some(120));
        assert_eq!(delta_de_caracter('-'), Some(-120));
        // Casos negativos: otra tecla u otra letra no son del mapa.
        assert_eq!(desplazamiento_de_flecha(0x41), None);
        assert_eq!(delta_de_caracter('p'), None);
    }
}
