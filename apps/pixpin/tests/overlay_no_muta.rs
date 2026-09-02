//! El tercer intocable de D14: mostrar la captura no la muta.
//!
//! Guardar tras pasar por el overlay debe producir EXACTAMENTE los mismos
//! bytes que guardar sin overlay. Si el render escribiera en la textura
//! capturada (un SetTarget equivocado bastaria), el PNG cambiaria y este
//! test lo caza comparando byte a byte.
//!
//! Es un test de integracion (binario aparte): el forbid(unsafe_code) de
//! main.rs no aplica aqui, pero cada bloque unsafe lleva su SAFETY igual
//! que en los crates auditados.

use pixpin_capture::{Dispositivo, a_imagen, capturar_monitor, enumerar_monitores};
use pixpin_geom::Rect;
use pixpin_render::MotorRender;
use windows::Win32::Graphics::Direct2D::D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};

/// Textura destino del mismo corte que las del overlay. Quince lineas
/// duplicadas del descriptor a proposito: exportar un helper de test desde
/// pixpin-render seria API publica para un solo consumidor.
fn textura_destino(d3d: &windows::Win32::Graphics::Direct3D11::ID3D11Device) -> ID3D11Texture2D {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: 800,
        Height: 600,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
        ..Default::default()
    };
    let mut t = None;
    // SAFETY: desc inicializada; t es la variable de salida local.
    unsafe { d3d.CreateTexture2D(&desc, None, Some(&mut t)) }.expect("textura destino");
    t.expect("textura destino")
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
fn dibujar_la_instantanea_no_cambia_sus_bytes() {
    let d = Dispositivo::nuevo().unwrap();
    let m = enumerar_monitores().unwrap();
    let principal = *m.principal().unwrap();
    let inst = capturar_monitor(&d, principal.id, principal.area).unwrap();
    let region = Rect {
        x: principal.area.x + 40,
        y: principal.area.y + 40,
        ancho: 200,
        alto: 150,
    };

    // Antes de dibujar nada.
    let antes = a_imagen(&d, &inst.recortar(&d, region).unwrap()).unwrap();

    // Envolver la textura como fondo y dibujarla entera sobre un destino
    // aparte, como hace el overlay en cada Pintar.
    let motor = MotorRender::nuevo(d.d3d()).unwrap();
    let fondo = motor.bitmap_desde_textura(inst.textura()).unwrap();
    let destino_tex = textura_destino(d.d3d());
    let destino = motor.destino_desde_textura(&destino_tex).unwrap();
    let c = motor.contexto();
    // SAFETY: protocolo SetTarget/BeginDraw/EndDraw sobre objetos vivos.
    unsafe {
        c.SetTarget(&destino);
        c.BeginDraw();
        c.DrawBitmap(
            &fondo,
            None,
            1.0,
            D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
            None,
            None,
        );
        c.EndDraw(None, None).unwrap();
        c.SetTarget(None);
    }

    // Despues de dibujar: los mismos bytes, o el overlay esta mutando.
    let despues = a_imagen(&d, &inst.recortar(&d, region).unwrap()).unwrap();
    assert_eq!(
        antes.pixeles, despues.pixeles,
        "el render muto la textura capturada"
    );
}
