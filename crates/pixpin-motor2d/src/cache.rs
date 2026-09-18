//! No volver a calcular el garabato de lo que no ha cambiado.
//!
//! `pintado::ordenes()` regenera la geometria de **cada elemento en cada
//! fotograma**. Un dibujo de trabajo son ocho mil elementos: ocho mil
//! generaciones de ruido, sesenta veces por segundo, para un dibujo que no
//! ha cambiado. El campo `version` de `Elemento` existe justo para esto —lo
//! dice su propio comentario— y no lo usaba nadie.
//!
//! # Por que la clave lleva el nivel y no el zoom
//!
//! La geometria depende del aumento: `ordenes_a_distancia` adelgaza los
//! trazos que a esa distancia no se ven. Si la clave llevara el zoom en
//! bruto, mover la rueda un grado invalidaria los ocho mil elementos y la
//! cache no serviria de nada justo cuando mas falta hace.
//!
//! Por eso la clave lleva la **octava**: `log2(zoom)` redondeado hacia
//! abajo. Entre los topes de la camara —5 % y 3.000 %— son diez valores, y
//! solo cambia al doblar o partir por la mitad el aumento. Encuadrar no lo
//! cambia nunca, asi que arrastrar el lienzo no invalida nada.

use std::collections::HashMap;

use crate::elemento::Elemento;
use crate::pintado::{Orden, ordenes_a_distancia};
#[cfg(test)]
use crate::vector::Punto2;

/// El nivel del 5 % de aumento, el tope de alejarse de la camara.
pub const NIVEL_MINIMO: i8 = -5;
/// El nivel del 3.000 %, el tope de acercarse.
pub const NIVEL_MAXIMO: i8 = 4;

/// La octava del aumento: `log2(zoom)` redondeado hacia abajo, sujeto a los
/// topes de la camara.
///
/// Sujetarlo no es paranoia: `log2(0)` es menos infinito y `log2` de un
/// negativo es NaN, y convertir cualquiera de los dos a `i8` es
/// comportamiento que no queremos ni mirar.
pub fn nivel_de_detalle(zoom: f32) -> i8 {
    if !zoom.is_finite() {
        return if zoom > 0.0 {
            NIVEL_MAXIMO
        } else {
            NIVEL_MINIMO
        };
    }
    if zoom <= 0.0 {
        return NIVEL_MINIMO;
    }
    let n = zoom.log2().floor();
    if n < NIVEL_MINIMO as f32 {
        NIVEL_MINIMO
    } else if n > NIVEL_MAXIMO as f32 {
        NIVEL_MAXIMO
    } else {
        n as i8
    }
}

#[derive(Debug, Clone)]
struct Entrada {
    version: u32,
    nivel: i8,
    ordenes: Vec<Orden>,
}

/// Guarda la geometria ya calculada de cada elemento, indexada por su
/// version y su nivel de detalle, para no volver a calcularla si ninguno de
/// los dos cambio.
///
/// **No tiene techo de memoria.** Guarda la geometria de los elementos que
/// se han pintado, que son los que caben en pantalla mas los que se hayan
/// visto al pasar; con ocho mil elementos y unas pocas ordenes por elemento
/// es del orden de megas, no decenas. Si la medicion dice otra cosa, el
/// arreglo es tirar las entradas de los elementos que llevan N fotogramas
/// sin pintarse. No hacerlo antes de medirlo.
#[derive(Debug, Default)]
pub struct Cache {
    mapa: HashMap<u64, Entrada>,
    aciertos: u64,
    fallos: u64,
}

impl Cache {
    pub fn nueva() -> Self {
        Self::default()
    }

    /// La geometria del elemento a ese aumento, calculandola solo si hace
    /// falta.
    pub fn ordenes(&mut self, e: &Elemento, zoom: f32) -> &[Orden] {
        let nivel = nivel_de_detalle(zoom);
        let vale = self
            .mapa
            .get(&e.id)
            .is_some_and(|x| x.version == e.version && x.nivel == nivel);

        if vale {
            self.aciertos += 1;
        } else {
            self.fallos += 1;
            let ordenes = ordenes_a_distancia(e, zoom);
            self.mapa.insert(
                e.id,
                Entrada {
                    version: e.version,
                    nivel,
                    ordenes,
                },
            );
        }
        // El `unwrap` es seguro: o valia, o se acaba de meter.
        &self.mapa.get(&e.id).expect("recien puesto").ordenes
    }

    /// Se llama al borrar de verdad un elemento (`Escena::compactar`).
    pub fn olvidar(&mut self, id: u64) {
        self.mapa.remove(&id);
    }

    /// Se llama al abrir otro documento.
    pub fn vaciar(&mut self) {
        self.mapa.clear();
    }

    pub fn cuantos(&self) -> usize {
        self.mapa.len()
    }

    /// Cuantas veces valio lo cacheado. Para las pruebas y la medicion.
    pub fn aciertos(&self) -> u64 {
        self.aciertos
    }

    /// Cuantas veces hubo que calcular. Para las pruebas y la medicion.
    pub fn fallos(&self) -> u64 {
        self.fallos
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::azar::Azar;
    use crate::camara::{ZOOM_MAXIMO, ZOOM_MINIMO};
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};
    use crate::pintado::ordenes_a_distancia;

    fn trazo(semilla: u32) -> Elemento {
        let mut azar = Azar::nuevo(semilla);
        let puntos: Vec<Punto2> = (0..40)
            .map(|i| Punto2::nuevo(i as f32 * 3.0, azar.siguiente() * 50.0))
            .collect();
        Elemento {
            id: semilla as u64,
            figura: Figura::Lapiz {
                puntos,
                presiones: Vec::new(),
                opciones: None,
            },
            x: 0.0,
            y: 0.0,
            ancho: 120.0,
            alto: 50.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 3.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
        }
    }

    #[test]
    fn lo_cacheado_es_igual_a_lo_recien_generado() {
        // La prueba que hace la cache digna de confianza. Si esto falla,
        // el dibujo cambia de aspecto segun si venia de la cache o no, que
        // es peor que no tener cache.
        let mut cache = Cache::nueva();
        for semilla in 1..60u32 {
            let e = trazo(semilla);
            for zoom in [0.1, 0.5, 1.0, 2.0, 7.0] {
                let esperado = ordenes_a_distancia(&e, zoom);
                assert_eq!(
                    cache.ordenes(&e, zoom),
                    esperado.as_slice(),
                    "semilla {semilla} al {zoom}"
                );
                // Y la segunda vez, que ya viene de la cache, tambien.
                assert_eq!(cache.ordenes(&e, zoom), esperado.as_slice(), "cacheado");
            }
        }
    }

    #[test]
    fn encuadrar_no_invalida_nada() {
        // Mover el lienzo no cambia el zoom, asi que la cache entera vale.
        // Es lo que hace que arrastrar el lienzo con ocho mil elementos no
        // cueste nada.
        let mut cache = Cache::nueva();
        let e = trazo(1);
        cache.ordenes(&e, 1.0);
        let fallos = cache.fallos();

        for _ in 0..100 {
            cache.ordenes(&e, 1.0);
        }
        assert_eq!(cache.fallos(), fallos, "ni un fallo mas");
        assert_eq!(cache.aciertos(), 100);
    }

    #[test]
    fn mover_la_rueda_un_poco_no_tira_la_cache() {
        // El motivo de que el nivel sea la octava y no el zoom en bruto: si
        // fuera el zoom, un grado de rueda tiraria los ocho mil elementos.
        let mut cache = Cache::nueva();
        let e = trazo(1);
        cache.ordenes(&e, 1.0);
        let fallos = cache.fallos();

        cache.ordenes(&e, 1.3);
        cache.ordenes(&e, 1.9);
        assert_eq!(
            cache.fallos(),
            fallos,
            "1,0 / 1,3 / 1,9 son la misma octava"
        );
    }

    #[test]
    fn cruzar_la_octava_si_la_tira() {
        let mut cache = Cache::nueva();
        let e = trazo(1);
        cache.ordenes(&e, 1.9);
        let fallos = cache.fallos();

        cache.ordenes(&e, 2.1);
        assert_eq!(cache.fallos(), fallos + 1, "2,0 empieza otra octava");
    }

    #[test]
    fn tocar_el_elemento_invalida_su_entrada() {
        let mut cache = Cache::nueva();
        let mut e = trazo(1);
        cache.ordenes(&e, 1.0);
        let fallos = cache.fallos();

        e.tocar();
        cache.ordenes(&e, 1.0);
        assert_eq!(cache.fallos(), fallos + 1, "la version subio");
    }

    #[test]
    fn el_nivel_de_detalle_es_la_octava_del_aumento() {
        assert_eq!(nivel_de_detalle(1.0), 0);
        assert_eq!(nivel_de_detalle(1.9), 0);
        assert_eq!(nivel_de_detalle(2.0), 1);
        assert_eq!(nivel_de_detalle(4.0), 2);
        assert_eq!(nivel_de_detalle(0.5), -1);
        assert_eq!(nivel_de_detalle(0.25), -2);
    }

    #[test]
    fn hay_diez_niveles_entre_los_topes_de_la_camara() {
        // Del 5 % al 3.000 %, que son los topes que ya tiene la camara.
        assert_eq!(nivel_de_detalle(ZOOM_MINIMO), NIVEL_MINIMO);
        assert_eq!(nivel_de_detalle(ZOOM_MAXIMO), NIVEL_MAXIMO);
        assert_eq!((NIVEL_MAXIMO - NIVEL_MINIMO + 1), 10);
    }

    #[test]
    fn un_zoom_absurdo_no_produce_un_nivel_absurdo() {
        // log2(0) es menos infinito, y un i8 no lo aguanta. Que no reviente
        // ni produzca un nivel de mil.
        assert_eq!(nivel_de_detalle(0.0), NIVEL_MINIMO);
        assert_eq!(nivel_de_detalle(-3.0), NIVEL_MINIMO);
        assert_eq!(nivel_de_detalle(f32::INFINITY), NIVEL_MAXIMO);
        assert_eq!(nivel_de_detalle(f32::NAN), NIVEL_MINIMO);
    }

    #[test]
    fn olvidar_saca_el_elemento_y_vaciar_los_saca_todos() {
        let mut cache = Cache::nueva();
        let a = trazo(1);
        let b = trazo(2);
        cache.ordenes(&a, 1.0);
        cache.ordenes(&b, 1.0);
        assert_eq!(cache.cuantos(), 2);

        cache.olvidar(a.id);
        assert_eq!(cache.cuantos(), 1);
        cache.vaciar();
        assert_eq!(cache.cuantos(), 0);
    }
}
