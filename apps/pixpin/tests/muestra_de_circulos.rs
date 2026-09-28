//! **El circulo a mano, pintado de verdad y guardado en PNG, antes y despues.**
//!
//! El usuario vio que con el trazo a mano medio o alto el circulo salia
//! poligonal: se notaban las esquinas. Las ordenes del motor no lo dicen a
//! simple vista (son puntos), asi que se pinta con el mismo pintor del editor
//! y se guarda para mirarlo. El «antes» son los vertices de siempre
//! (`formas::vertices_de_elipse`, misma semilla) unidos con rectas, que es lo
//! que se pintaba; el «despues», `formas::elipse`, que pasa por esos mismos
//! vertices con la spline de rough.js.
//!
//! `cargo test -p pixpin --test muestra_de_circulos -- --ignored --nocapture
//! --test-threads=1`. Deja los PNG en `target/muestras-de-circulos`.

use pixpin_motor2d::azar::Azar;
use pixpin_motor2d::formas;
use pixpin_motor2d::vector::Punto2;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_render::{Color, MotorRender, Pintor};

const LADO: u32 = 240;
/// Cuanto se amplia el trozo de borde en su muestra, pixel a pixel.
const LUPA: u32 = 6;

const BLANCO: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};
const TINTA: Color = Color {
    r: 0.12,
    g: 0.12,
    b: 0.14,
    a: 1.0,
};

fn carpeta() -> std::path::PathBuf {
    let d =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-de-circulos");
    std::fs::create_dir_all(&d).expect("crear la carpeta de muestras");
    d
}

fn guardar(nombre: &str, ancho: u32, alto: u32, pixeles: Vec<u8>) {
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
        ancho,
        alto,
        pixeles,
    })
    .expect("codificar");
    let ruta = carpeta().join(format!("{nombre}.png"));
    std::fs::write(&ruta, png).expect("guardar");
    println!("{nombre}: {}", ruta.display());
}

fn pintar(p: &Pintor, vueltas: &[Vec<Punto2>], grosor: f32) {
    p.limpiar(BLANCO);
    for v in vueltas {
        let v: Vec<(f32, f32)> = v.iter().map(|q| (q.x, q.y)).collect();
        p.polilinea(&v, grosor, TINTA);
    }
}

/// El cuarto de arriba a la derecha del borde (80 x 80 desde (140, 20))
/// ampliado `LUPA` veces: es donde se ven las esquinas.
fn borde_ampliado(rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    let (x0, y0, lado) = (140u32, 20u32, 80u32);
    let n = lado * LUPA;
    let mut v = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let (sx, sy) = (x0 + x / LUPA, y0 + y / LUPA);
            let i = ((sy * LADO + sx) * 4) as usize;
            v.extend_from_slice(&rgba[i..i + 4]);
        }
    }
    (n, n, v)
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn el_circulo_a_mano_antes_y_despues_de_alisarlo() {
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), LADO, LADO).expect("fuera de pantalla");
    let leer = |vueltas: &[Vec<Punto2>], grosor: f32| {
        motor
            .dibujar(&fuera.destino, |p| pintar(p, vueltas, grosor))
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        fuera.leer_rgba().expect("leer").2
    };
    for rugosidad in [0.0f32, 1.0, 2.0] {
        for grosor in [2.0f32, 4.0] {
            let (x, y, lado, semilla) = (20.0, 20.0, 200.0, 4);
            let antes =
                formas::vertices_de_elipse(x, y, lado, lado, rugosidad, &mut Azar::nuevo(semilla));
            let despues = formas::elipse(x, y, lado, lado, rugosidad, &mut Azar::nuevo(semilla));
            for (cuando, vueltas) in [("antes", antes), ("despues", despues)] {
                let rgba = leer(&vueltas, grosor);
                let nombre = format!("circulo-r{rugosidad}-g{grosor}-{cuando}");
                let (w, h, v) = borde_ampliado(&rgba);
                guardar(&format!("{nombre}-lupa"), w, h, v);
                guardar(&nombre, LADO, LADO, rgba);
            }
        }
    }
}
