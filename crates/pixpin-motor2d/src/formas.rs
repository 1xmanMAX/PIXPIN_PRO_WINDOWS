//! Rectangulos, elipses, lineas y flechas dibujados "a mano".
//!
//! Una linea recta perfecta no parece dibujada. El truco que usan todas las
//! herramientas de este estilo es doble:
//!
//! 1. **Desviar los extremos** un poco, con el azar reproducible del
//!    elemento, y curvar el trazo por el medio con una Bezier cubica cuyos
//!    puntos de control tambien estan desviados.
//! 2. **Trazar cada linea DOS veces**, con desvios distintos. Dos pasadas casi
//!    iguales es exactamente lo que hace una mano al repasar un boceto, y es
//!    lo que separa "linea temblorosa" de "linea dibujada".
//!
//! El azar viene del elemento (su semilla), asi que el mismo rectangulo se
//! dibuja igual siempre: es lo que permite guardar un documento y reabrirlo
//! sin que cambie de aspecto.

use crate::azar::Azar;
use crate::vector::Punto2;

/// Cuantos tramos tiene cada linea "a mano": suficientes para que la curva se
/// vea suave sin llenar la geometria de puntos.
const TRAMOS: usize = 12;

/// Una linea "a mano" de `a` a `b`: dos pasadas, cada una con su desvio.
///
/// `rugosidad` 0 devuelve la linea recta exacta (util para reglas y para el
/// resaltador); 1 es el aspecto normal; 2, muy marcado.
pub fn linea(a: Punto2, b: Punto2, rugosidad: f32, azar: &mut Azar) -> Vec<Vec<Punto2>> {
    if rugosidad <= 0.0 {
        return vec![vec![a, b]];
    }
    // El desvio crece con la longitud pero se estanca: en una linea de mil
    // pixeles, un temblor proporcional se veria como un garabato.
    let largo = a.distancia(b);
    let desvio = (largo / 40.0).clamp(1.0, 4.0) * rugosidad;

    (0..2)
        .map(|pasada| {
            // La segunda pasada desvia algo menos: al repasar, la mano ya
            // sabe por donde va.
            let d = if pasada == 0 { desvio } else { desvio * 0.7 };
            let inicio = Punto2::nuevo(a.x + azar.desvio(d), a.y + azar.desvio(d));
            let fin = Punto2::nuevo(b.x + azar.desvio(d), b.y + azar.desvio(d));
            // Dos puntos de control desviados: la panza de la linea.
            let c1 = a
                .hacia(b, 0.33)
                .sumar(Punto2::nuevo(azar.desvio(d * 2.0), azar.desvio(d * 2.0)));
            let c2 = a
                .hacia(b, 0.66)
                .sumar(Punto2::nuevo(azar.desvio(d * 2.0), azar.desvio(d * 2.0)));
            bezier(inicio, c1, c2, fin)
        })
        .collect()
}

/// Bezier cubica evaluada en `TRAMOS` tramos.
fn bezier(p0: Punto2, c1: Punto2, c2: Punto2, p3: Punto2) -> Vec<Punto2> {
    (0..=TRAMOS)
        .map(|i| {
            let t = i as f32 / TRAMOS as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            Punto2::nuevo(
                p0.x * a + c1.x * b + c2.x * c + p3.x * d,
                p0.y * a + c1.y * b + c2.y * c + p3.y * d,
            )
        })
        .collect()
}

/// Los cuatro lados de un rectangulo, cada uno con sus dos pasadas.
pub fn rectangulo(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    rugosidad: f32,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    let e = [
        Punto2::nuevo(x, y),
        Punto2::nuevo(x + ancho, y),
        Punto2::nuevo(x + ancho, y + alto),
        Punto2::nuevo(x, y + alto),
    ];
    let mut salida = Vec::with_capacity(8);
    for i in 0..4 {
        salida.extend(linea(e[i], e[(i + 1) % 4], rugosidad, azar));
    }
    salida
}

/// Una elipse "a mano": dos vueltas completas ligeramente distintas.
///
/// No se cierra en el mismo punto donde empieza a proposito: una elipse
/// dibujada a mano casi nunca cierra exacta, y ese pequeño exceso es lo que
/// la hace creible.
pub fn elipse(
    x: f32,
    y: f32,
    ancho: f32,
    alto: f32,
    rugosidad: f32,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    let cx = x + ancho / 2.0;
    let cy = y + alto / 2.0;
    let rx = ancho / 2.0;
    let ry = alto / 2.0;

    if rugosidad <= 0.0 {
        let pasos = 48;
        return vec![
            (0..=pasos)
                .map(|i| {
                    let t = std::f32::consts::TAU * i as f32 / pasos as f32;
                    Punto2::nuevo(cx + rx * t.cos(), cy + ry * t.sin())
                })
                .collect(),
        ];
    }

    let desvio = ((rx + ry) / 40.0).clamp(1.0, 4.0) * rugosidad;
    let pasos = 32;
    (0..2)
        .map(|pasada| {
            let d = if pasada == 0 { desvio } else { desvio * 0.7 };
            // La segunda vuelta arranca con un pequeño desfase y se pasa un
            // poco del cierre: asi las dos pasadas no quedan superpuestas.
            let desfase = azar.desvio(0.4) + pasada as f32 * 0.3;
            let vuelta = std::f32::consts::TAU + if pasada == 1 { 0.35 } else { 0.0 };
            (0..=pasos)
                .map(|i| {
                    let t = desfase + vuelta * i as f32 / pasos as f32;
                    Punto2::nuevo(
                        cx + rx * t.cos() + azar.desvio(d),
                        cy + ry * t.sin() + azar.desvio(d),
                    )
                })
                .collect()
        })
        .collect()
}

/// La punta de una flecha: dos lineas cortas desde el extremo.
///
/// Se abre 25 grados a cada lado y mide una fraccion del tramo final, con un
/// tope: una flecha larguisima no puede tener una punta de doscientos pixeles.
pub fn punta_flecha(
    desde: Punto2,
    hasta: Punto2,
    rugosidad: f32,
    azar: &mut Azar,
) -> Vec<Vec<Punto2>> {
    let largo = (desde.distancia(hasta) * 0.3).clamp(8.0, 40.0);
    let direccion = desde.restar(hasta).unitario();
    let apertura = 25.0f32.to_radians();

    let mut salida = Vec::with_capacity(4);
    for signo in [1.0f32, -1.0] {
        let (s, c) = (apertura * signo).sin_cos();
        let girado = Punto2::nuevo(
            direccion.x * c - direccion.y * s,
            direccion.x * s + direccion.y * c,
        );
        salida.extend(linea(
            hasta,
            hasta.proyectar(girado, largo),
            rugosidad,
            azar,
        ));
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn caja(trazos: &[Vec<Punto2>]) -> (f32, f32, f32, f32) {
        let todos: Vec<Punto2> = trazos.iter().flatten().copied().collect();
        (
            todos.iter().map(|p| p.x).fold(f32::MAX, f32::min),
            todos.iter().map(|p| p.y).fold(f32::MAX, f32::min),
            todos.iter().map(|p| p.x).fold(f32::MIN, f32::max),
            todos.iter().map(|p| p.y).fold(f32::MIN, f32::max),
        )
    }

    #[test]
    fn sin_rugosidad_la_linea_es_exactamente_recta() {
        // El modo regla: dos puntos, los pedidos, sin un pixel de desvio.
        let mut a = Azar::nuevo(1);
        let t = linea(
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(100.0, 0.0),
            0.0,
            &mut a,
        );
        assert_eq!(t.len(), 1, "sin rugosidad basta una pasada");
        assert_eq!(
            t[0],
            vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)]
        );
    }

    #[test]
    fn con_rugosidad_hay_dos_pasadas_y_ninguna_es_recta() {
        let mut a = Azar::nuevo(42);
        let t = linea(
            Punto2::nuevo(0.0, 50.0),
            Punto2::nuevo(200.0, 50.0),
            1.0,
            &mut a,
        );
        assert_eq!(t.len(), 2, "una mano repasa la linea");
        for pasada in &t {
            let torcida = pasada.iter().any(|p| (p.y - 50.0).abs() > 0.3);
            assert!(torcida, "la pasada salio recta: no parece dibujada");
        }
        // Y las dos pasadas son DISTINTAS: si fueran iguales se veria una
        // sola linea y el efecto se pierde.
        assert_ne!(t[0], t[1]);
    }

    #[test]
    fn la_misma_semilla_dibuja_exactamente_la_misma_forma() {
        // La puerta de la fase: reabrir el documento no puede cambiar nada.
        let hacer = || {
            let mut a = Azar::nuevo(777);
            rectangulo(10.0, 20.0, 100.0, 60.0, 1.0, &mut a)
        };
        assert_eq!(hacer(), hacer());
    }

    #[test]
    fn semillas_distintas_dibujan_formas_distintas() {
        // Caso negativo: si la semilla no influyera, todos los rectangulos
        // del documento saldrian calcados y el efecto "a mano" desapareceria.
        let hacer = |s| {
            let mut a = Azar::nuevo(s);
            rectangulo(10.0, 20.0, 100.0, 60.0, 1.0, &mut a)
        };
        assert_ne!(hacer(777), hacer(778));
    }

    #[test]
    fn el_rectangulo_no_se_aleja_de_su_caja() {
        // Puede temblar, pero no puede irse: si el desvio creciera sin tope,
        // un rectangulo grande se saldria del elemento y del pin.
        let mut a = Azar::nuevo(5);
        let t = rectangulo(100.0, 100.0, 400.0, 300.0, 2.0, &mut a);
        let (x0, y0, x1, y1) = caja(&t);
        assert!(
            x0 > 100.0 - 20.0 && y0 > 100.0 - 20.0,
            "se sale por arriba: {x0},{y0}"
        );
        assert!(
            x1 < 500.0 + 20.0 && y1 < 400.0 + 20.0,
            "se sale por abajo: {x1},{y1}"
        );
    }

    #[test]
    fn la_elipse_cubre_su_caja_por_los_cuatro_lados() {
        let mut a = Azar::nuevo(9);
        let t = elipse(0.0, 0.0, 200.0, 100.0, 1.0, &mut a);
        let (x0, y0, x1, y1) = caja(&t);
        assert!(x0 < 10.0 && x1 > 190.0, "no llega a los lados: {x0}..{x1}");
        assert!(
            y0 < 10.0 && y1 > 90.0,
            "no llega arriba y abajo: {y0}..{y1}"
        );
    }

    #[test]
    fn la_punta_de_la_flecha_esta_en_el_extremo_y_apunta_hacia_atras() {
        let mut a = Azar::nuevo(11);
        let desde = Punto2::nuevo(0.0, 0.0);
        let hasta = Punto2::nuevo(100.0, 0.0);
        let t = punta_flecha(desde, hasta, 0.0, &mut a);
        assert_eq!(t.len(), 2, "la punta son dos lineas");
        for pasada in &t {
            // Cada linea empieza en la punta...
            assert!(pasada[0].distancia(hasta) < 0.001);
            // ...y termina hacia atras, nunca mas alla del extremo.
            assert!(
                pasada.last().unwrap().x < hasta.x,
                "la punta apunta al reves"
            );
        }
    }

    #[test]
    fn una_forma_sin_tamano_no_entra_en_panico() {
        let mut a = Azar::nuevo(1);
        let _ = rectangulo(10.0, 10.0, 0.0, 0.0, 1.0, &mut a);
        let e = elipse(10.0, 10.0, 0.0, 0.0, 1.0, &mut a);
        for p in e.iter().flatten() {
            assert!(p.x.is_finite() && p.y.is_finite());
        }
    }
}
