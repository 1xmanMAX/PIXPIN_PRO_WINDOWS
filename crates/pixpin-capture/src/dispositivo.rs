//! El dispositivo Direct3D 11 sobre el que ocurre toda la captura.
//!
//! Windows.Graphics.Capture es una API de WinRT y entrega sus fotogramas como
//! `IDirect3DSurface`, mientras que el recorte, el dibujo y la codificacion
//! necesitan el `ID3D11Device` de Win32. Este modulo crea uno y expone las dos
//! caras del mismo dispositivo, que es lo que permite que la imagen **no baje
//! nunca a la CPU** salvo cuando el usuario guarda o copia.
//!
//! Si las dos caras fuesen dispositivos distintos, la textura entregada por la
//! captura no seria utilizable por nuestro contexto, y el fallo apareceria
//! mucho mas tarde y en otro sitio.

use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_SDK_VERSION,
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice;
use windows::core::Interface;

#[derive(Debug, thiserror::Error)]
pub enum ErrorCaptura {
    #[error("no se pudo crear un dispositivo Direct3D 11: {0}")]
    SinDispositivo(#[source] windows::core::Error),
    #[error("error de Windows durante la captura: {0}")]
    Windows(#[from] windows::core::Error),
    #[error("no llego ningun fotograma antes del tiempo limite")]
    SinFotograma,
    #[error("el duplicador perdio el acceso a la pantalla; hay que recrearlo")]
    AccesoPerdido,
    /// Otro proceso tiene tomada la Duplicacion de Escritorio de esa salida
    /// —es un recurso exclusivo, y lo cogen los programas de escritorio
    /// remoto y los grabadores—. No es un fallo nuestro: se cae a WGC.
    #[error("otro programa tiene tomada la duplicacion de esta pantalla")]
    DuplicacionOcupada,
    #[error("no existe ningun monitor con el identificador {0}")]
    MonitorDesconocido(u32),
    #[error("la region {region:?} no cabe en la instantanea {disponible:?}")]
    RegionFuera {
        region: pixpin_geom::Rect,
        disponible: pixpin_geom::Rect,
    },
}

pub struct Dispositivo {
    d3d: ID3D11Device,
    contexto: ID3D11DeviceContext,
    winrt: IDirect3DDevice,
    /// Si se creo con `VIDEO_SUPPORT` (D66): sin el, Media Foundation no
    /// puede compartir el dispositivo y los videos se ensenan como
    /// documento.
    soporta_video: bool,
}

impl Dispositivo {
    /// Crea el dispositivo, prefiriendo la GPU y cayendo a WARP.
    ///
    /// WARP es el rasterizador por software de Microsoft. Existe la maquina
    /// sin GPU utilizable —sesiones remotas, maquinas virtuales, controladores
    /// a medio instalar— y en ellas es preferible capturar despacio a no
    /// capturar. `BGRA_SUPPORT` es obligatorio: sin el, Direct2D no puede
    /// dibujar sobre estas texturas en S1-B2.
    pub fn nuevo() -> Result<Self, ErrorCaptura> {
        // Primero con soporte de video (D66); si el driver lo rechaza, sin
        // el: capturar es mas importante que reproducir, y un driver raro
        // no puede dejar la aplicacion sin capturas.
        let intentos = [
            (D3D_DRIVER_TYPE_HARDWARE, true),
            (D3D_DRIVER_TYPE_HARDWARE, false),
            (D3D_DRIVER_TYPE_WARP, true),
            (D3D_DRIVER_TYPE_WARP, false),
        ];
        for (tipo, con_video) in intentos {
            let mut d3d: Option<ID3D11Device> = None;
            let mut contexto: Option<ID3D11DeviceContext> = None;
            let flags = if con_video {
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT
            } else {
                D3D11_CREATE_DEVICE_BGRA_SUPPORT
            };

            // SAFETY: los tres punteros de salida son variables locales
            // inicializadas a None, que es lo que la API espera. El resto de
            // parametros son constantes documentadas. La funcion no retiene
            // ninguna referencia a nuestras variables tras devolver.
            let resultado = unsafe {
                D3D11CreateDevice(
                    None,
                    tipo,
                    // El modulo de rasterizador software solo aplica a
                    // D3D_DRIVER_TYPE_SOFTWARE; para HARDWARE y WARP va nulo.
                    HMODULE::default(),
                    flags,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut d3d),
                    None,
                    Some(&mut contexto),
                )
            };

            if resultado.is_err() {
                continue;
            }

            let (Some(d3d), Some(contexto)) = (d3d, contexto) else {
                continue;
            };

            // Media Foundation decodifica en sus propios hilos sobre este
            // mismo dispositivo: sin proteccion multihilo, dos llamadas
            // simultaneas corromperian el contexto inmediato.
            if con_video {
                if let Ok(multi) = d3d.cast::<ID3D11Multithread>() {
                    // SAFETY: interfaz valida del dispositivo recien creado.
                    unsafe {
                        let _ = multi.SetMultithreadProtected(true);
                    }
                }
            }

            let dxgi: IDXGIDevice = d3d.cast()?;
            // SAFETY: `dxgi` es una interfaz valida obtenida por `cast` del
            // dispositivo recien creado, que sigue vivo. La funcion devuelve
            // una interfaz nueva con su propia cuenta de referencias.
            let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi)? };
            let winrt: IDirect3DDevice = inspectable.cast()?;

            return Ok(Self {
                d3d,
                contexto,
                winrt,
                soporta_video: con_video,
            });
        }

        // `from_thread` recoge el ultimo error del hilo, el equivalente
        // actual del antiguo `from_win32` que el plan esbozaba.
        Err(ErrorCaptura::SinDispositivo(
            windows::core::Error::from_thread(),
        ))
    }

    pub fn d3d(&self) -> &ID3D11Device {
        &self.d3d
    }

    /// Si Media Foundation puede compartir este dispositivo (D66).
    pub fn soporta_video(&self) -> bool {
        self.soporta_video
    }

    pub fn contexto(&self) -> &ID3D11DeviceContext {
        &self.contexto
    }

    /// La misma GPU, vista desde WinRT. Es lo que espera `GraphicsCaptureItem`.
    pub fn winrt(&self) -> &IDirect3DDevice {
        &self.winrt
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use windows::core::Interface;

    /// Necesita una GPU real, asi que no puede correr en la CI de GitHub.
    /// Ejecutalo en local con `cargo test -- --ignored`.
    #[test]
    #[ignore = "necesita una GPU real; ejecutar con --ignored"]
    fn se_crea_un_dispositivo_y_sus_tres_vistas_son_coherentes() {
        let d = Dispositivo::nuevo().expect("deberia poder crearse un dispositivo D3D11");

        // Las tres vistas deben apuntar al mismo dispositivo subyacente. Si
        // `winrt()` devolviera un dispositivo distinto del de `d3d()`, la
        // textura que entregue la captura no seria utilizable por nuestro
        // contexto y el fallo apareceria mucho mas tarde, al copiar.
        let ctx = d.contexto();
        // SAFETY: `GetDevice` devuelve una interfaz nueva, contada por
        // referencia, al dispositivo que creo este contexto. `ctx` sigue vivo
        // durante toda la llamada porque lo mantiene `d`.
        let desde_contexto =
            unsafe { ctx.GetDevice() }.expect("el contexto siempre tiene dispositivo");

        assert_eq!(
            desde_contexto.as_raw(),
            d.d3d().as_raw(),
            "el contexto debe pertenecer al mismo dispositivo"
        );
        // D66: en una GPU real y en WARP el soporte de video se concede; si
        // aqui saliera false, los videos caerian a documento sin motivo.
        assert!(
            d.soporta_video(),
            "el dispositivo deberia tener VIDEO_SUPPORT"
        );
    }
}
