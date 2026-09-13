//! Pintar la tinta como Excalidraw: contorno cerrado, curvas cuadraticas
//! por puntos medios y relleno *winding*.
//!
//! `pasos_de_tinta` es `getSvgPathFromStroke` de Excalidraw
//! (`packages/element/src/shape.ts` @afa3a65, MIT) sin el recorte a dos
//! decimales, que solo existe para acortar el SVG. Es pura: se prueba sin
//! GPU contra el oraculo.
//!
//! Por que *winding* y no la regla por defecto de Direct2D (alternada): el
//! contorno de perfect-freehand se cruza consigo mismo en bucles, retrocesos
//! y en los arcos de las esquinas. Con la alternada cada cruce abre un
//! agujero; Canvas2D rellena con *nonzero*, que es *winding*. `Pintor::velo`
//! SI necesita la alternada para su hueco, por eso esto es una geometria
//! aparte y no un cambio en `Pintor::geometria`.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PasoTrayecto {
    Mover((f32, f32)),
    Cuadratica {
        control: (f32, f32),
        fin: (f32, f32),
    },
    Linea((f32, f32)),
    Cerrar,
}

/// `M p0 Q p0 m01 p1 m12 ... pN mN0 L p0 Z`.
pub fn pasos_de_tinta(contorno: &[(f32, f32)]) -> Vec<PasoTrayecto> {
    let Some(&primero) = contorno.first() else {
        return Vec::new();
    };
    let medio = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let max = contorno.len() - 1;
    let mut pasos = Vec::with_capacity(contorno.len() + 3);
    pasos.push(PasoTrayecto::Mover(primero));
    for (i, &p) in contorno.iter().enumerate() {
        let siguiente = if i == max { primero } else { contorno[i + 1] };
        pasos.push(PasoTrayecto::Cuadratica {
            control: p,
            fin: medio(p, siguiente),
        });
    }
    pasos.push(PasoTrayecto::Linea(primero));
    pasos.push(PasoTrayecto::Cerrar);
    pasos
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_contorno_vacio_no_da_ningun_paso() {
        assert!(pasos_de_tinta(&[]).is_empty());
    }

    #[test]
    fn cada_vertice_es_el_control_de_una_cuadratica_que_acaba_en_el_punto_medio() {
        let c = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];
        assert_eq!(
            pasos_de_tinta(&c),
            vec![
                PasoTrayecto::Mover((0.0, 0.0)),
                PasoTrayecto::Cuadratica {
                    control: (0.0, 0.0),
                    fin: (5.0, 0.0)
                },
                PasoTrayecto::Cuadratica {
                    control: (10.0, 0.0),
                    fin: (10.0, 5.0)
                },
                PasoTrayecto::Cuadratica {
                    control: (10.0, 10.0),
                    fin: (5.0, 5.0)
                },
                PasoTrayecto::Linea((0.0, 0.0)),
                PasoTrayecto::Cerrar,
            ]
        );
    }

    /// Los numeros del SVG de Excalidraw, en orden, sin las letras.
    fn numeros_svg(svg: &str) -> Vec<f32> {
        svg.split_whitespace()
            .filter(|t| !t.chars().all(|c| c.is_ascii_alphabetic()))
            .flat_map(|t| {
                t.split(',')
                    .map(|n| n.parse::<f32>().expect("numero"))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn numeros_pasos(p: &[PasoTrayecto]) -> Vec<f32> {
        p.iter()
            .flat_map(|paso| match *paso {
                PasoTrayecto::Mover(a) | PasoTrayecto::Linea(a) => vec![a.0, a.1],
                PasoTrayecto::Cuadratica { control, fin } => {
                    vec![control.0, control.1, fin.0, fin.1]
                }
                PasoTrayecto::Cerrar => vec![],
            })
            .collect()
    }

    #[test]
    fn el_trayecto_coincide_con_get_svg_path_from_stroke_de_excalidraw() {
        // NOTA (ruling del controlador): se compara contra `svg_crudo`, no
        // `svg`. El regex de truncado de Excalidraw a dos decimales corrompe
        // numeros en notacion exponencial (1.2e-16 pasa a "1.22"), lo que
        // daria un rojo falso. `svg_crudo` puede traer esa notacion
        // ("1.2246467991473532e-16"); f32::parse la acepta sin problema.
        let ruta = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../pixpin-motor2d/tests/oraculo/tinta.json"
        );
        let texto = std::fs::read_to_string(ruta).expect("falta el oraculo (Tarea 1)");
        let v: serde_json::Value = serde_json::from_str(&texto).unwrap();
        for caso in v["casos"].as_array().unwrap() {
            let contorno: Vec<(f32, f32)> = caso["contorno"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| (p[0].as_f64().unwrap() as f32, p[1].as_f64().unwrap() as f32))
                .collect();
            let esperado = numeros_svg(caso["svg_crudo"].as_str().unwrap());
            let obtenido = numeros_pasos(&pasos_de_tinta(&contorno));
            assert_eq!(esperado.len(), obtenido.len(), "{}", caso["nombre"]);
            for (e, o) in esperado.iter().zip(&obtenido) {
                // Coordenadas de pixel: la conversion f64->f32 se queda muy
                // por debajo de esta tolerancia.
                assert!((e - o).abs() <= 0.001, "{}: {e} contra {o}", caso["nombre"]);
            }
        }
    }
}
