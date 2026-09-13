//! Puerta de pintado de E1 (spec §5): un fotograma con 1.000 trazos ya
//! realizados en menos de 3 ms. Necesita GPU: `--ignored`.

use std::time::Instant;

use pixpin_render::{CacheTinta, Color, MotorRender};

fn contorno(i: usize) -> Vec<(f32, f32)> {
    let (bx, by) = ((i % 40) as f32 * 45.0, (i / 40) as f32 * 40.0);
    (0..60)
        .map(|k| {
            let a = k as f32 / 60.0 * std::f32::consts::TAU;
            (bx + 20.0 + a.cos() * 15.0, by + 20.0 + a.sin() * 8.0)
        })
        .collect()
}

/// Las mismas lineas que `motor_y_destino_de_prueba` de `src/tinta.rs`
/// (Tarea 10): una prueba de integracion no ve los ayudantes `#[cfg(test)]`
/// del crate, asi que se repiten aqui.
fn motor_y_destino(
    ancho: u32,
    alto: u32,
) -> (
    MotorRender,
    windows::Win32::Graphics::Direct2D::ID2D1Bitmap1,
) {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11CreateDevice,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
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
    let desc = D3D11_TEXTURE2D_DESC {
        Width: ancho,
        Height: alto,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
        ..Default::default()
    };
    let mut textura = None;
    // SAFETY: descripcion local valida; salida local.
    unsafe {
        d3d.CreateTexture2D(&desc, None, Some(&mut textura))
            .expect("textura")
    };
    let destino = motor
        .destino_desde_textura(&textura.expect("textura"))
        .expect("destino");
    (motor, destino)
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn mil_trazos_realizados_se_pintan_en_menos_de_tres_milisegundos() {
    let (motor, destino) = motor_y_destino(1920, 1080);
    let contornos: Vec<_> = (0..1_000).map(contorno).collect();
    let mut cache = CacheTinta::nueva();
    let negro = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let fotograma = |cache: &mut CacheTinta| {
        motor
            .dibujar(&destino, |p| {
                for (i, c) in contornos.iter().enumerate() {
                    p.tinta_cacheada(cache, (i as u64, 1, 0), c, negro);
                }
            })
            .unwrap();
    };
    fotograma(&mut cache); // realiza
    let mut mejor = std::time::Duration::MAX;
    for _ in 0..10 {
        let t = Instant::now();
        fotograma(&mut cache);
        mejor = mejor.min(t.elapsed());
    }
    assert!(mejor.as_micros() < 3_000, "1.000 trazos en {mejor:?}");
}
