//! **La muestra de los materiales de tinta, pintada y guardada en un PNG.**
//!
//! Un grano no se comprueba con un `assert_eq`: se mira. Esto pinta el mismo
//! trazo con cada material —la pluma de siempre, las de raya, las porosas— y
//! deja el resultado en un PNG que se puede abrir. Lo que si se comprueba a
//! maquina es lo que una prueba puede decir de una imagen sin verla: que las
//! porosas **no** tapan lo mismo que la pluma, que las encendidas si —porque
//! el PC todavia no tiene llave de luz—, y que ningun material deja el lienzo
//! en blanco.
//!
//! Necesita GPU y sesion de escritorio, como las demas puertas de pintado:
//! `cargo test -p pixpin-render --test muestra_de_tintas -- --ignored
//! --nocapture --test-threads=1`.

use std::time::Instant;

use windows::Win32::Graphics::Direct3D11::ID3D11Device;

use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};
use pixpin_motor2d::pintado;
use pixpin_motor2d::tinta::{self, MATERIALES, MaterialTinta};
use pixpin_motor2d::vector::Punto2;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_render::{CacheGrano, Color, MotorRender};

const ANCHO: u32 = 900;
const ALTO: u32 = 150;

const BLANCO: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

/// Un motor y un dispositivo D3D11 de hardware. Las mismas lineas que
/// `puerta_tinta.rs`: una prueba de integracion no ve los ayudantes
/// `#[cfg(test)]` del crate, asi que se repiten.
fn motor_y_dispositivo() -> (MotorRender, ID3D11Device) {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
    };
    let mut d3d = None;
    // SAFETY: salidas locales; sin adaptador concreto ni capas de depuracion.
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut d3d),
            None,
            None,
        )
        .expect("sin D3D11 hardware");
    }
    let d3d = d3d.expect("dispositivo");
    let motor = MotorRender::nuevo(&d3d).expect("motor");
    (motor, d3d)
}

/// Un trazo ondulado de lado a lado, como el que se hace a pulso.
fn trazo_de_muestra(material: MaterialTinta) -> Elemento {
    let puntos: Vec<Punto2> = (0..=160)
        .map(|i| {
            let t = i as f32 / 160.0;
            let x = 30.0 + t * (ANCHO as f32 - 60.0);
            let y = ALTO as f32 / 2.0 + (t * std::f32::consts::TAU * 1.5).sin() * 28.0;
            Punto2::nuevo(x, y)
        })
        .collect();
    Elemento {
        id: 1,
        figura: Figura::Lapiz {
            puntos,
            presiones: Vec::new(),
            opciones: Some(tinta::OpcionesTinta::default()),
        },
        // Grueso de verdad: con una raya fina no se ve ni el grano ni la
        // diferencia entre uno y otro, que es lo que la muestra viene a
        // ensenar.
        grosor: tinta::GROSOR_GRUESO * 3.0,
        trazo: ColorRgba::opaco(0.12, 0.12, 0.14),
        material,
        ..Default::default()
    }
}

fn a_color(c: ColorRgba) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

/// Pinta `e` sobre blanco y devuelve los pixeles RGBA.
///
/// Es, en pequeno, lo que tendra que hacer el editor: la orden de tinta pinta
/// el cuerpo y, justo detras, el grano recortado a la MISMA silueta.
fn pintar(
    fuera: &FueraDePantalla,
    motor: &MotorRender,
    e: &Elemento,
    cache: &mut CacheGrano,
) -> Vec<u8> {
    motor
        .dibujar(&fuera.destino, |p| {
            p.limpiar(BLANCO);
            for orden in pintado::ordenes(e) {
                let pintado::Orden::Tinta { contorno, color } = orden else {
                    continue;
                };
                let puntos: Vec<(f32, f32)> = contorno.iter().map(|q| (q.x, q.y)).collect();
                p.tinta(&puntos, a_color(color));
                if let Some(g) = pintado::grano_de(e) {
                    let tela = tinta::tejido(g.material);
                    p.grano(
                        cache,
                        (e.id, e.version),
                        &puntos,
                        &tela,
                        tinta::material::LADO_DEL_MOSAICO,
                        g.material as u32,
                        a_color(g.color),
                        g.paso,
                        g.inclinada,
                    );
                }
            }
        })
        .expect("pintar");
    fuera.esperar_gpu().expect("esperar");
    let (_, _, rgba) = fuera.leer_rgba().expect("leer");
    rgba
}

/// **Cuanta tinta hay en el papel**: lo oscuro que sale el lienzo entero.
///
/// Y no cuantos pixeles se tocaron, que fue el primer intento y no sirve: un
/// trazo poroso ocupa el MISMO sitio que uno liso —la silueta es la misma—,
/// solo que por dentro deja ver el papel. Contando pixeles tocados los diez
/// materiales dan casi el mismo numero; contando lo oscuro, se separan.
fn tinta_derramada(rgba: &[u8]) -> u64 {
    rgba.chunks_exact(4).map(|p| (255 - p[0]) as u64).sum()
}

/// Donde dejar los PNG de muestra.
fn carpeta_de_muestras() -> std::path::PathBuf {
    let d =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-de-tintas");
    std::fs::create_dir_all(&d).expect("crear la carpeta de muestras");
    d
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn cada_material_pinta_el_mismo_trazo_de_otra_manera() {
    let (motor, d3d) = motor_y_dispositivo();
    let fuera = FueraDePantalla::nuevo(&motor, &d3d, ANCHO, ALTO).expect("fuera de pantalla");
    let mut cache = CacheGrano::nueva();
    let carpeta = carpeta_de_muestras();

    let mut huellas: Vec<(MaterialTinta, u64)> = Vec::new();
    for m in MATERIALES {
        let e = trazo_de_muestra(m);
        let rgba = pintar(&fuera, &motor, &e, &mut cache);
        let con_tinta = tinta_derramada(&rgba);
        assert!(
            con_tinta > 0,
            "{m:?} dejo el lienzo en blanco: no se pinto nada"
        );
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
            ancho: ANCHO,
            alto: ALTO,
            pixeles: rgba,
        })
        .expect("codificar");
        let ruta = carpeta.join(format!("tinta-{}.png", m.palabra()));
        std::fs::write(&ruta, png).expect("guardar");
        println!(
            "{:>8}: {con_tinta:>9} de tinta derramada -> {}",
            m.palabra(),
            ruta.display()
        );
        huellas.push((m, con_tinta));
    }

    let de = |m: MaterialTinta| {
        huellas
            .iter()
            .find(|(k, _)| *k == m)
            .map(|(_, v)| *v)
            .unwrap()
    };
    // Las porosas dejan ver el papel por dentro: si taparan lo mismo que la
    // pluma, el grano no se estaria viendo y las tres se leerian igual que la
    // lisa, que es justo el fallo que el movil arreglo el 16-sep.
    for m in [
        MaterialTinta::Tiza,
        MaterialTinta::Lapiz2b,
        MaterialTinta::Seco,
    ] {
        assert!(
            de(m) < de(MaterialTinta::Lisa),
            "{m:?} derrama {} y la pluma {}: no se distingue",
            de(m),
            de(MaterialTinta::Lisa)
        );
    }
    // Caso negativo: las encendidas, que aqui no tienen llave de paso, se
    // pintan como la pluma —mismo ancho, mismo contorno, misma huella— en vez
    // de inventarse un halo que el movil tampoco hace.
    for m in [MaterialTinta::Luz, MaterialTinta::Hdr] {
        assert_eq!(
            de(m),
            de(MaterialTinta::Lisa),
            "{m:?} tendria que verse como la pluma mientras el PC no tenga llave de luz"
        );
    }
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn una_tela_se_teje_una_sola_vez_por_material_y_color() {
    // Es lo que hace que el grano cueste un relleno y no cien rayas: la tela
    // sube a la GPU la primera vez y se reaprovecha en todos los fotogramas.
    let (motor, d3d) = motor_y_dispositivo();
    let fuera = FueraDePantalla::nuevo(&motor, &d3d, ANCHO, ALTO).expect("fuera de pantalla");
    let mut cache = CacheGrano::nueva();
    let e = trazo_de_muestra(MaterialTinta::Tiza);
    for _ in 0..5 {
        pintar(&fuera, &motor, &e, &mut cache);
    }
    assert_eq!(cache.cuantas(), 1, "se tejio mas de una vez");

    // Otro material es otra tela; el mismo, no.
    pintar(
        &fuera,
        &motor,
        &trazo_de_muestra(MaterialTinta::Rayado),
        &mut cache,
    );
    assert_eq!(cache.cuantas(), 2);

    // Y una tinta lisa no teje nada: no hay tela que estampar.
    pintar(
        &fuera,
        &motor,
        &trazo_de_muestra(MaterialTinta::Lisa),
        &mut cache,
    );
    assert_eq!(cache.cuantas(), 2, "la pluma no teje");
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn el_grano_no_vuelve_lento_el_lienzo() {
    // La puerta de rendimiento del encargo: una tinta con textura no puede
    // volver el lienzo lento. Se comparan mil trazos lisos contra mil trazos
    // de tiza, que es el caso peor —cuerpo mas grano, dos rellenos por
    // trazo—, y el segundo tiene que seguir cabiendo en un fotograma de
    // 60 Hz igual que el primero.
    let (motor, d3d) = motor_y_dispositivo();
    let fuera = FueraDePantalla::nuevo(&motor, &d3d, 1920, 1080).expect("fuera de pantalla");
    let mut cache = CacheGrano::nueva();
    let contornos: Vec<Vec<(f32, f32)>> = (0..1_000)
        .map(|i: usize| {
            let (bx, by) = ((i % 40) as f32 * 45.0, (i / 40) as f32 * 40.0);
            (0..60)
                .map(|k| {
                    let a = k as f32 / 60.0 * std::f32::consts::TAU;
                    (bx + 20.0 + a.cos() * 15.0, by + 20.0 + a.sin() * 8.0)
                })
                .collect()
        })
        .collect();
    let negro = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let tela = tinta::tejido(MaterialTinta::Tiza);

    let medir = |con_grano: bool, cache: &mut CacheGrano| {
        let fotograma = |cache: &mut CacheGrano| {
            motor
                .dibujar(&fuera.destino, |p| {
                    for c in &contornos {
                        p.tinta(c, negro);
                        if con_grano {
                            p.grano(
                                cache,
                                (0, 0),
                                c,
                                &tela,
                                tinta::material::LADO_DEL_MOSAICO,
                                MaterialTinta::Tiza as u32,
                                negro,
                                6.0,
                                false,
                            );
                        }
                    }
                })
                .unwrap();
        };
        fotograma(cache); // en frio: sube la tela
        let mut mejor = std::time::Duration::MAX;
        for _ in 0..10 {
            let t = Instant::now();
            fotograma(cache);
            mejor = mejor.min(t.elapsed());
        }
        mejor
    };

    let liso = medir(false, &mut cache);
    let con_grano = medir(true, &mut cache);
    println!("mil trazos lisos: {liso:?} | mil trazos de tiza: {con_grano:?}");
    // **El tope es relativo y no absoluto**, y eso es a proposito: mil trazos
    // sin cache de teselado ya se salen de un fotograma de 60 Hz **sin
    // grano** —para eso existe `tinta_cacheada` y la capa estatica—, asi que
    // un tope en milisegundos mediria la maquina y no el grano. Lo que el
    // grano tiene que cumplir es que el trazo cueste **un relleno mas**, no
    // cien rayas: como mucho el doble, que es exactamente pintar dos veces la
    // misma silueta.
    assert!(
        con_grano.as_micros() <= liso.as_micros() * 5 / 2,
        "el grano multiplico el fotograma por mas de dos y medio: liso {liso:?}, con grano {con_grano:?}"
    );
}
