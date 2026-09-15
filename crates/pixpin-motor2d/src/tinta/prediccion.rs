//! Prediccion del puntero para la tinta en curso.
//!
//! Aunque se pinte en cuanto llega el punto, entre leer el raton y verlo en
//! pantalla pasan un refresco y la composicion de DWM: medido en el equipo
//! del usuario, la punta iba unos 30 ms detras del cursor. Presentar antes
//! no lo arreglo (misma cifra con la cola de fotogramas en 1), porque ese
//! retraso es el propio ciclo de la pantalla.
//!
//! Lo que hacen Windows Ink y los navegadores (`getPredictedEvents`) es
//! pintar la punta donde ESTARA el puntero cuando el fotograma se vea: la
//! velocidad reciente por el horizonte. Solo se pinta; el trazo guardado es
//! el real.
//!
//! Las dos protecciones que evitan que la punta «se adelante» de mas:
//! - un tope de distancia, y
//! - apagarla cuando el movimiento es lento o acaba de torcer, que es
//!   justo cuando extrapolar en linea recta se equivoca y se veria.

use std::collections::VecDeque;

use crate::vector::Punto2;

/// Ventana de muestras con la que se mide la velocidad.
const VENTANA_MS: f64 = 50.0;
/// Por debajo de esta velocidad (pixeles de pantalla por segundo) no se
/// predice: al frenar para terminar un trazo, una punta adelantada se ve
/// como un pelo que sobra.
const VELOCIDAD_MINIMA: f32 = 60.0;
/// Si la direccion de los ultimos puntos se separa de la de la ventana mas
/// de esto (coseno), el trazo esta girando y se predice menos.
const COSENO_GIRO: f32 = 0.7;

#[derive(Debug, Default)]
pub struct Predictor {
    muestras: VecDeque<(Punto2, f64)>,
}

impl Predictor {
    pub fn nuevo() -> Self {
        Self::default()
    }

    /// Olvida el trazo anterior: un trazo nuevo no hereda velocidad.
    pub fn reiniciar(&mut self) {
        self.muestras.clear();
    }

    /// Anota un punto del trazo en el instante `ms` (reloj monotono).
    pub fn anotar(&mut self, p: Punto2, ms: f64) {
        self.muestras.push_back((p, ms));
        while let Some(&(_, t)) = self.muestras.front() {
            if ms - t > VENTANA_MS && self.muestras.len() > 2 {
                self.muestras.pop_front();
            } else {
                break;
            }
        }
    }

    /// Donde estara el puntero dentro de `horizonte_ms`, sin pasar de `tope`
    /// de distancia. `a_pantalla` pasa la velocidad a pixeles de pantalla
    /// (el zoom) para decidir si es lenta. `None` si no hay datos o no
    /// conviene predecir.
    pub fn predecir(&self, horizonte_ms: f32, tope: f32, a_pantalla: f32) -> Option<Punto2> {
        let &(ultimo, t1) = self.muestras.back()?;
        let &(primero, t0) = self.muestras.front()?;
        let dt = (t1 - t0) as f32;
        // Menos de 8 ms de historia: la velocidad seria ruido.
        if dt < 8.0 {
            return None;
        }
        let vx = (ultimo.x - primero.x) / dt;
        let vy = (ultimo.y - primero.y) / dt;
        let velocidad = (vx * vx + vy * vy).sqrt();
        if velocidad * 1000.0 * a_pantalla < VELOCIDAD_MINIMA {
            return None;
        }

        // Direccion reciente: el tramo que acaba en el ultimo punto desde la
        // muestra del medio. Si tuerce respecto a la media, se reduce.
        let medio = self.muestras[self.muestras.len() / 2].0;
        let (rx, ry) = (ultimo.x - medio.x, ultimo.y - medio.y);
        let largo_r = (rx * rx + ry * ry).sqrt();
        let mut factor = 1.0;
        if largo_r > 0.0 {
            let coseno = (rx * vx + ry * vy) / (largo_r * velocidad);
            if coseno <= 0.0 {
                return None;
            }
            if coseno < COSENO_GIRO {
                factor = 0.5;
            }
        }

        let (mut dx, mut dy) = (vx * horizonte_ms * factor, vy * horizonte_ms * factor);
        let largo = (dx * dx + dy * dy).sqrt();
        if largo > tope && largo > 0.0 {
            dx *= tope / largo;
            dy *= tope / largo;
        }
        Some(Punto2::nuevo(ultimo.x + dx, ultimo.y + dy))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn recta(pred: &mut Predictor, vx: f32, vy: f32, n: usize, cada_ms: f64) {
        for i in 0..n {
            let t = i as f64 * cada_ms;
            pred.anotar(Punto2::nuevo(vx * t as f32, vy * t as f32), t);
        }
    }

    #[test]
    fn a_velocidad_constante_la_punta_va_donde_estara_el_cursor() {
        let mut p = Predictor::nuevo();
        // 1,5 px por ms = 1500 px/s, una muestra cada 4 ms.
        recta(&mut p, 1.5, 0.0, 20, 4.0);
        let ultimo = 1.5 * 76.0;
        let q = p.predecir(30.0, 1000.0, 1.0).unwrap();
        assert!((q.x - (ultimo + 45.0)).abs() < 0.5, "{q:?}");
        assert!(q.y.abs() < 1e-3);
    }

    #[test]
    fn la_prediccion_no_pasa_del_tope() {
        let mut p = Predictor::nuevo();
        recta(&mut p, 10.0, 0.0, 20, 4.0);
        let ultimo = 10.0 * 76.0;
        let q = p.predecir(30.0, 40.0, 1.0).unwrap();
        assert!((q.x - (ultimo + 40.0)).abs() < 1e-3, "{q:?}");
    }

    #[test]
    fn despacio_no_se_predice_para_no_dejar_un_pelo_al_frenar() {
        let mut p = Predictor::nuevo();
        // 0,02 px/ms = 20 px/s.
        recta(&mut p, 0.02, 0.0, 20, 4.0);
        assert_eq!(p.predecir(30.0, 40.0, 1.0), None);
    }

    #[test]
    fn sin_historia_suficiente_no_se_inventa_nada() {
        let mut p = Predictor::nuevo();
        assert_eq!(p.predecir(30.0, 40.0, 1.0), None);
        p.anotar(Punto2::nuevo(0.0, 0.0), 0.0);
        p.anotar(Punto2::nuevo(5.0, 0.0), 4.0);
        assert_eq!(p.predecir(30.0, 40.0, 1.0), None, "4 ms no bastan");
    }

    #[test]
    fn al_dar_la_vuelta_se_apaga() {
        let mut p = Predictor::nuevo();
        // Va a la derecha 36 ms y ACABA de dar la vuelta: dos muestras hacia
        // la izquierda. Seguir la media llevaria la punta a la derecha, justo
        // al lado contrario de donde va la mano.
        for i in 0..10 {
            p.anotar(Punto2::nuevo(i as f32 * 6.0, 0.0), i as f64 * 4.0);
        }
        for i in 1..3 {
            p.anotar(
                Punto2::nuevo(54.0 - i as f32 * 12.0, 0.0),
                36.0 + i as f64 * 4.0,
            );
        }
        assert_eq!(p.predecir(30.0, 40.0, 1.0), None);
    }

    #[test]
    fn las_muestras_viejas_salen_de_la_ventana() {
        let mut p = Predictor::nuevo();
        recta(&mut p, 1.0, 0.0, 100, 4.0);
        let span = p.muestras.back().unwrap().1 - p.muestras.front().unwrap().1;
        assert!(span <= VENTANA_MS + 4.0, "{span}");
    }

    #[test]
    fn reiniciar_olvida_la_velocidad_del_trazo_anterior() {
        let mut p = Predictor::nuevo();
        recta(&mut p, 1.5, 0.0, 20, 4.0);
        p.reiniciar();
        assert_eq!(p.predecir(30.0, 40.0, 1.0), None);
    }
}
