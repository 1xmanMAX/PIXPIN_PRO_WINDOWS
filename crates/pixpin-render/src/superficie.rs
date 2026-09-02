//! Una superficie de composicion por ventana de overlay.
//!
//! `WS_EX_NOREDIRECTIONBITMAP` quita a la ventana su superficie GDI: lo
//! unico que se ve es lo que este swapchain presenta a traves de
//! DirectComposition. Es el camino sin parpadeo y sin copias del escritorio
//! moderno, el mismo que usa QuickView — la tecnica, no el codigo.
//!
//! El present es A DEMANDA: se presenta cuando algo cambio, nunca en un
//! bucle de fotogramas. De ahi sale el 0% de CPU en reposo del overlay.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGIFactory2, IDXGISwapChain1,
};
use windows::core::Interface;

use crate::motor::{ErrorRender, MotorRender};

pub struct Superficie {
    _dcomp: IDCompositionDevice,
    _objetivo: IDCompositionTarget,
    _visual: IDCompositionVisual,
    swapchain: IDXGISwapChain1,
}

impl Superficie {
    /// La ventana del llamante debe sobrevivir a la Superficie (obligacion
    /// del llamante: en el overlay, la ventana posee a su Superficie).
    pub fn nueva(
        _motor: &MotorRender,
        d3d: &ID3D11Device,
        hwnd: HWND,
        ancho: u32,
        alto: u32,
    ) -> Result<Self, ErrorRender> {
        let dxgi: IDXGIDevice = d3d.cast().map_err(|_| ErrorRender::SinDxgi)?;
        // SAFETY: el adaptador y la factoria se obtienen del dispositivo del
        // llamante, vivo durante toda la llamada.
        let fabrica: IDXGIFactory2 = unsafe { dxgi.GetAdapter()?.GetParent()? };

        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: ancho,
            Height: alto,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            // Dos buffers y flip: el minimo que permite componer sin copia.
            BufferCount: 2,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            Scaling: DXGI_SCALING_STRETCH,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
            ..Default::default()
        };
        // SAFETY: swapchain DE COMPOSICION (sin HWND propio): es el unico
        // tipo valido para NOREDIRECTIONBITMAP; parametros documentados.
        let swapchain = unsafe { fabrica.CreateSwapChainForComposition(&dxgi, &desc, None)? };

        // SAFETY: dispositivo de composicion sobre el mismo DXGI; el target
        // toma la ventana del llamante (ver la obligacion en el doc de
        // `nueva`).
        let dcomp: IDCompositionDevice = unsafe { DCompositionCreateDevice(&dxgi)? };
        // SAFETY: los objetos se acaban de crear y estan vivos; Commit
        // publica el arbol de composicion.
        let (objetivo, visual) = unsafe {
            let objetivo = dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = dcomp.CreateVisual()?;
            visual.SetContent(&swapchain)?;
            objetivo.SetRoot(&visual)?;
            dcomp.Commit()?;
            (objetivo, visual)
        };

        Ok(Self {
            _dcomp: dcomp,
            _objetivo: objetivo,
            _visual: visual,
            swapchain,
        })
    }

    /// El backbuffer ACTUAL envuelto como destino D2D. Hay que volver a
    /// llamarlo en cada fotograma: con flip, el backbuffer rota en cada
    /// present y el bitmap anterior queda apuntando al buffer equivocado.
    pub fn empezar(&self, motor: &MotorRender) -> Result<ID2D1Bitmap1, ErrorRender> {
        // SAFETY: el indice 0 es siempre el backbuffer escribible actual.
        let textura: ID3D11Texture2D = unsafe { self.swapchain.GetBuffer(0)? };
        motor.destino_backbuffer(&textura)
    }

    pub fn presentar(&self) -> Result<(), ErrorRender> {
        // SAFETY: present sin espera de vsync (0,0): el bucle es dirigido
        // por eventos y no debe bloquear el hilo de interfaz.
        unsafe { self.swapchain.Present(0, Default::default()).ok()? };
        Ok(())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::motor::{Color, MotorRender};
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CW_USEDEFAULT, CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_POPUP,
    };
    use windows::core::w;

    fn d3d() -> ID3D11Device {
        let mut d = None;
        // SAFETY: igual que en motor.rs: salidas locales, constantes documentadas.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d),
                None,
                None,
            )
            .expect("GPU real");
        }
        d.unwrap()
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn se_crea_se_dibuja_y_se_presenta_sin_error() {
        // Una ventana minima descartable. La clase "STATIC" del sistema
        // evita registrar una propia solo para el test.
        // SAFETY: CreateWindowExW con la clase del sistema no exige mas que
        // un modulo valido; la ventana se destruye al final del test.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("prueba"),
                WS_POPUP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                256,
                256,
                None,
                None,
                Some(GetModuleHandleW(None).unwrap().into()),
                None,
            )
            .expect("ventana de prueba")
        };

        let d3d = d3d();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let superficie = Superficie::nueva(&motor, &d3d, hwnd, 256, 256)
            .expect("deberia crearse la superficie de composicion");

        let destino = superficie.empezar(&motor).unwrap();
        let c = motor.contexto();
        // SAFETY: protocolo SetTarget/BeginDraw/EndDraw sobre objetos vivos.
        unsafe {
            c.SetTarget(&destino);
            c.BeginDraw();
            c.Clear(Some(&Color::ACENTO.a_d2d()));
            c.EndDraw(None, None).unwrap();
            c.SetTarget(None);
        }
        superficie.presentar().expect("present deberia funcionar");

        // Dos fotogramas seguidos: el swapchain rota los buffers y el
        // segundo present fallaria si empezar() no re-envolviera el
        // backbuffer actual. Es el caso negativo del diseno de un solo uso.
        let destino2 = superficie.empezar(&motor).unwrap();
        // SAFETY: igual que arriba.
        unsafe {
            c.SetTarget(&destino2);
            c.BeginDraw();
            c.Clear(Some(&Color::NEGRO.a_d2d()));
            c.EndDraw(None, None).unwrap();
            c.SetTarget(None);
        }
        superficie.presentar().expect("el segundo present tambien");

        drop(superficie);
        // SAFETY: la ventana la creo este test y nadie mas la usa.
        unsafe { DestroyWindow(hwnd).unwrap() };
    }
}
