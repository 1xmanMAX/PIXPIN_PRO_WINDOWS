//! Forma rapida: un trazo a mano que se queda quieto se convierte en figura.
//!
//! Es lo que hacen QuickShape de Procreate y «Dibujar a forma» de
//! Excalidraw: se dibuja un circulo torcido, se deja el lapiz quieto sin
//! soltar, y aparece un circulo limpio que se sigue ajustando al arrastrar.
//!
//! El reconocimiento es puro y barato: corre UNA vez, cuando el trazo se
//! para, sobre sus puntos. Tres formas, de mas a menos segura:
//! - **Linea**: todos los puntos cerca de la cuerda.
//! - **Rectangulo**: el trazo simplificado tiene esquinas de casi 90 grados y
//!   lados derechos. Vale una «L» abierta (subir y luego ir en horizontal),
//!   que da el rectangulo que esa esquina insinua, y vale cerrado.
//! - **Elipse**: cerrado (o casi) y los puntos cerca de la elipse inscrita
//!   en su caja.
//!
//! Si no es ninguna, el trazo se queda como esta.

use crate::vector::{Punto2, distancia_a_segmento};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormaReconocida {
    Linea {
        a: Punto2,
        b: Punto2,
    },
    /// Caja (x0, y0, x1, y1).
    Rectangulo {
        caja: (f32, f32, f32, f32),
    },
    Elipse {
        caja: (f32, f32, f32, f32),
    },
}

fn caja(puntos: &[Punto2]) -> (f32, f32, f32, f32) {
    puntos.iter().fold(
        (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
        |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
    )
}

/// Ramer-Douglas-Peucker: los vertices que sobreviven con tolerancia `eps`.
fn simplificar(puntos: &[Punto2], eps: f32) -> Vec<Punto2> {
    if puntos.len() < 3 {
        return puntos.to_vec();
    }
    let (a, b) = (puntos[0], puntos[puntos.len() - 1]);
    let (mut peor, mut indice) = (0.0, 0);
    for (i, p) in puntos.iter().enumerate().take(puntos.len() - 1).skip(1) {
        let d = distancia_a_segmento(*p, a, b);
        if d > peor {
            peor = d;
            indice = i;
        }
    }
    if peor <= eps {
        return vec![a, b];
    }
    let mut izquierda = simplificar(&puntos[..=indice], eps);
    let derecha = simplificar(&puntos[indice..], eps);
    izquierda.pop();
    izquierda.extend(derecha);
    izquierda
}

/// Coseno del angulo en `b` entre `a-b` y `c-b`. 0 es un angulo recto.
fn coseno_esquina(a: Punto2, b: Punto2, c: Punto2) -> f32 {
    let (ux, uy) = (a.x - b.x, a.y - b.y);
    let (vx, vy) = (c.x - b.x, c.y - b.y);
    let n = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
    if n == 0.0 {
        return 1.0;
    }
    (ux * vx + uy * vy) / n
}

/// Si un segmento va casi en horizontal o casi en vertical (menos de ~20
/// grados de desvio): los rectangulos de Excalidraw van derechos.
fn casi_de_eje(a: Punto2, b: Punto2) -> bool {
    let (dx, dy) = ((b.x - a.x).abs(), (b.y - a.y).abs());
    dx.min(dy) <= dx.max(dy) * 0.36
}

pub fn reconocer(puntos: &[Punto2]) -> Option<FormaReconocida> {
    if puntos.len() < 4 {
        return None;
    }
    let largo: f32 = puntos.windows(2).map(|w| w[0].distancia(w[1])).sum();
    let c = caja(puntos);
    let diagonal = ((c.2 - c.0).powi(2) + (c.3 - c.1).powi(2)).sqrt();
    // Un garabato de menos de 20 unidades no es una forma: es un punto.
    if largo < 20.0 || diagonal < 12.0 {
        return None;
    }
    let (inicio, fin) = (puntos[0], puntos[puntos.len() - 1]);
    let cuerda = inicio.distancia(fin);

    // Linea: la cuerda casi es el recorrido y nadie se aparta de ella.
    if cuerda >= 0.85 * largo {
        let peor = puntos
            .iter()
            .map(|p| distancia_a_segmento(*p, inicio, fin))
            .fold(0.0, f32::max);
        if peor <= (0.06 * cuerda).max(3.0) {
            return Some(FormaReconocida::Linea { a: inicio, b: fin });
        }
    }

    let cerrado = cuerda <= 0.2 * diagonal.max(largo * 0.25);
    let vertices = simplificar(puntos, 0.07 * diagonal);

    // «L» abierta: dos lados rectos, de eje, que forman casi 90 grados.
    if !cerrado && vertices.len() == 3 {
        let (a, b, v) = (vertices[0], vertices[1], vertices[2]);
        let lados_largos = a.distancia(b) >= 0.2 * largo && b.distancia(v) >= 0.2 * largo;
        if lados_largos
            && coseno_esquina(a, b, v).abs() < 0.35
            && casi_de_eje(a, b)
            && casi_de_eje(b, v)
        {
            return Some(FormaReconocida::Rectangulo {
                caja: caja(&vertices),
            });
        }
    }

    if cerrado {
        // Rectangulo cerrado: 4 esquinas casi rectas. Se admite un vertice de
        // mas por el pequeño gancho al cerrar.
        let mut v = vertices.clone();
        if v.len() >= 2 && v[0].distancia(v[v.len() - 1]) <= 0.2 * diagonal {
            v.pop();
        }
        if (4..=5).contains(&v.len()) {
            let n = v.len();
            let rectas = (0..n)
                .filter(|&i| coseno_esquina(v[(i + n - 1) % n], v[i], v[(i + 1) % n]).abs() < 0.4)
                .count();
            let derechos = (0..n).all(|i| {
                let (a, b) = (v[i], v[(i + 1) % n]);
                a.distancia(b) < 0.08 * largo || casi_de_eje(a, b)
            });
            if rectas >= 3 && derechos {
                return Some(FormaReconocida::Rectangulo { caja: c });
            }
        }
    }

    // Elipse: cerca de la elipse inscrita en la caja, y cerrada o casi.
    if cuerda <= 0.4 * diagonal {
        let (cx, cy) = ((c.0 + c.2) / 2.0, (c.1 + c.3) / 2.0);
        let (rx, ry) = (((c.2 - c.0) / 2.0).max(1.0), ((c.3 - c.1) / 2.0).max(1.0));
        let error: f32 = puntos
            .iter()
            .map(|p| {
                let (u, w) = ((p.x - cx) / rx, (p.y - cy) / ry);
                ((u * u + w * w).sqrt() - 1.0).abs()
            })
            .sum::<f32>()
            / puntos.len() as f32;
        if error < 0.12 {
            return Some(FormaReconocida::Elipse { caja: c });
        }
    }
    None
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn circulo(cx: f32, cy: f32, r: f32, vuelta: f32, ruido: f32) -> Vec<Punto2> {
        (0..=60)
            .map(|i| {
                let t = std::f32::consts::TAU * vuelta * i as f32 / 60.0;
                // Un temblor determinista, como el de una mano.
                let d = ruido * ((i as f32 * 1.7).sin());
                Punto2::nuevo(cx + (r + d) * t.cos(), cy + (r + d) * t.sin())
            })
            .collect()
    }

    fn tramo(a: Punto2, b: Punto2, n: usize) -> Vec<Punto2> {
        (0..n)
            .map(|i| {
                let t = i as f32 / n as f32;
                Punto2::nuevo(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
            })
            .collect()
    }

    #[test]
    fn un_circulo_a_mano_es_una_elipse() {
        match reconocer(&circulo(100.0, 100.0, 50.0, 1.0, 2.5)) {
            Some(FormaReconocida::Elipse { caja }) => {
                assert!((caja.0 - 50.0).abs() < 5.0 && (caja.2 - 150.0).abs() < 5.0);
            }
            otro => panic!("esperaba elipse: {otro:?}"),
        }
    }

    #[test]
    fn un_circulo_casi_cerrado_tambien_cuenta() {
        assert!(matches!(
            reconocer(&circulo(0.0, 0.0, 40.0, 0.92, 1.0)),
            Some(FormaReconocida::Elipse { .. })
        ));
    }

    #[test]
    fn una_raya_torcida_poco_es_una_linea() {
        let mut p = tramo(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(200.0, 40.0), 30);
        for (i, q) in p.iter_mut().enumerate() {
            q.y += ((i as f32) * 0.9).sin() * 2.0;
        }
        assert!(matches!(reconocer(&p), Some(FormaReconocida::Linea { .. })));
    }

    #[test]
    fn subir_y_luego_ir_en_horizontal_da_el_rectangulo_de_esa_esquina() {
        // Desde abajo a la izquierda sube, y en la esquina va a la derecha.
        let mut p = tramo(Punto2::nuevo(0.0, 100.0), Punto2::nuevo(2.0, 0.0), 20);
        p.extend(tramo(
            Punto2::nuevo(2.0, 0.0),
            Punto2::nuevo(150.0, 3.0),
            20,
        ));
        p.push(Punto2::nuevo(150.0, 3.0));
        match reconocer(&p) {
            Some(FormaReconocida::Rectangulo { caja }) => {
                assert!(caja.0.abs() < 3.0 && caja.1.abs() < 3.0, "{caja:?}");
                assert!((caja.2 - 150.0).abs() < 3.0 && (caja.3 - 100.0).abs() < 3.0);
            }
            otro => panic!("esperaba rectangulo: {otro:?}"),
        }
    }

    #[test]
    fn un_rectangulo_cerrado_a_mano_es_un_rectangulo_y_no_una_elipse() {
        let esquinas = [
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(120.0, 2.0),
            Punto2::nuevo(118.0, 80.0),
            Punto2::nuevo(-2.0, 78.0),
            Punto2::nuevo(1.0, 3.0),
        ];
        let mut p = Vec::new();
        for w in esquinas.windows(2) {
            p.extend(tramo(w[0], w[1], 15));
        }
        p.push(esquinas[4]);
        assert!(matches!(
            reconocer(&p),
            Some(FormaReconocida::Rectangulo { .. })
        ));
    }

    #[test]
    fn un_garabato_o_una_v_no_se_convierten() {
        // Caso negativo: una V de 45 grados no es una esquina de rectangulo.
        let mut v = tramo(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(50.0, 100.0), 20);
        v.extend(tramo(
            Punto2::nuevo(50.0, 100.0),
            Punto2::nuevo(100.0, 0.0),
            20,
        ));
        v.push(Punto2::nuevo(100.0, 0.0));
        assert_eq!(reconocer(&v), None);
        // Un zigzag tampoco.
        let zig: Vec<Punto2> = (0..40)
            .map(|i| Punto2::nuevo(i as f32 * 5.0, if i % 2 == 0 { 0.0 } else { 30.0 }))
            .collect();
        assert_eq!(reconocer(&zig), None);
        // Y un toque de nada.
        assert_eq!(
            reconocer(&tramo(Punto2::nuevo(0.0, 0.0), Punto2::nuevo(3.0, 3.0), 5)),
            None
        );
    }
}
