//! Captura de un solo disparo con Windows.Graphics.Capture.
//!
//! La sesion se abre, se toma **un** fotograma y se cierra. El overlay de
//! S1-B2 congela la pantalla con esto; el modo en vivo reabrira una sesion
//! continua sobre las mismas piezas.
//!
//! El fotograma llega en un hilo del pool de WinRT, asi que se usa un canal
//! para traerlo al hilo llamante. La imagen **no baja a la CPU**: lo que
//! cruza el canal es la textura de la GPU.

use std::sync::mpsc;
use std::time::Duration;

use pixpin_geom::Rect;
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::core::Interface;

use crate::dispositivo::{Dispositivo, ErrorCaptura};
use crate::monitores::handle_de_monitor;

/// Cuanto se espera al primer fotograma antes de rendirse.
///
/// Generoso a proposito: en el primer arranque, o con la GPU ocupada, WinRT
/// puede tardar. Rendirse antes daria una captura vacia sin explicacion.
const ESPERA_FOTOGRAMA: Duration = Duration::from_secs(3);

/// Una imagen capturada, viviendo en la memoria de la GPU.
pub struct Instantanea {
    textura: ID3D11Texture2D,
    /// Donde estaba esta imagen en el escritorio virtual.
    area: Rect,
}

impl Instantanea {
    pub fn textura(&self) -> &ID3D11Texture2D {
        &self.textura
    }

    pub fn area(&self) -> Rect {
        self.area
    }

    /// Recorta una region, **en la GPU**, sin pasar por memoria de sistema.
    ///
    /// `region` va en coordenadas del escritorio virtual, igual que
    /// [`Self::area`].
    pub fn recortar(
        &self,
        dispositivo: &Dispositivo,
        region: Rect,
    ) -> Result<Instantanea, ErrorCaptura> {
        // Comprobar la contencion aqui es importante: `CopySubresourceRegion`
        // con una caja fuera de rango no falla ruidosamente, deja basura.
        let Some(recorte) = region.interseccion(self.area) else {
            return Err(ErrorCaptura::RegionFuera {
                region,
                disponible: self.area,
            });
        };
        if recorte != region {
            return Err(ErrorCaptura::RegionFuera {
                region,
                disponible: self.area,
            });
        }

        let destino = crear_textura(dispositivo, recorte.ancho, recorte.alto)?;

        // Coordenadas relativas al origen de esta instantanea.
        let dx = (region.x - self.area.x) as u32;
        let dy = (region.y - self.area.y) as u32;
        let caja = D3D11_BOX {
            left: dx,
            top: dy,
            front: 0,
            right: dx + region.ancho,
            bottom: dy + region.alto,
            back: 1,
        };

        // SAFETY: origen y destino pertenecen al mismo dispositivo, tienen el
        // mismo formato, y `caja` esta contenida en el origen — lo garantiza
        // la comprobacion de interseccion de arriba, y el destino se creo con
        // exactamente el tamano de la caja.
        unsafe {
            dispositivo.contexto().CopySubresourceRegion(
                &destino,
                0,
                0,
                0,
                0,
                &self.textura,
                0,
                Some(&caja),
            );
        }

        Ok(Instantanea {
            textura: destino,
            area: region,
        })
    }
}

/// Captura un fotograma del monitor indicado.
pub fn capturar_monitor(
    dispositivo: &Dispositivo,
    id_monitor: u32,
    area: Rect,
) -> Result<Instantanea, ErrorCaptura> {
    let Some(handle) = handle_de_monitor(id_monitor) else {
        return Err(ErrorCaptura::MonitorDesconocido(id_monitor));
    };

    let interop: IGraphicsCaptureItemInterop =
        windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
    // SAFETY: `handle` viene de la enumeracion de monitores y sigue siendo
    // valido; `CreateForMonitor` devuelve una interfaz nueva con su propia
    // cuenta de referencias.
    let item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(handle)? };

    let tamano = item.Size()?;
    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        dispositivo.winrt(),
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        // Dos buffers: uno en uso y otro llenandose. Con uno solo, WinRT
        // descarta fotogramas; con mas, se gasta memoria de video sin ganar
        // nada en una captura de un disparo.
        2,
        SizeInt32 {
            Width: tamano.Width,
            Height: tamano.Height,
        },
    )?;

    let (envia, recibe) = mpsc::channel::<ID3D11Texture2D>();

    let manejador =
        TypedEventHandler::<Direct3D11CaptureFramePool, windows::core::IInspectable>::new(
            move |pool, _| {
                let Some(pool) = pool.as_ref() else {
                    return Ok(());
                };
                let Ok(fotograma) = pool.TryGetNextFrame() else {
                    return Ok(());
                };
                let Ok(superficie) = fotograma.Surface() else {
                    return Ok(());
                };
                if let Ok(acceso) = superficie.cast::<IDirect3DDxgiInterfaceAccess>() {
                    // SAFETY: `acceso` procede de la superficie del fotograma,
                    // que sigue viva en este ambito. `GetInterface` devuelve
                    // una interfaz nueva con su propia cuenta de referencias,
                    // asi que la textura sobrevive al envio por el canal.
                    if let Ok(textura) = unsafe { acceso.GetInterface::<ID3D11Texture2D>() } {
                        let _ = envia.send(textura);
                    }
                }
                Ok(())
            },
        );

    let token = pool.FrameArrived(&manejador)?;
    let sesion = pool.CreateCaptureSession(&item)?;

    // Sin esto Windows dibuja un borde amarillo alrededor de lo capturado en
    // las compilaciones donde la propiedad existe. Se consulta por capacidad
    // y no por numero de compilacion: detectar capacidades sobrevive a los
    // cambios de Microsoft, comparar versiones no.
    if let Ok(soportado) = windows::Foundation::Metadata::ApiInformation::IsPropertyPresent(
        &windows::core::HSTRING::from("Windows.Graphics.Capture.GraphicsCaptureSession"),
        &windows::core::HSTRING::from("IsBorderRequired"),
    ) {
        if soportado {
            let _ = sesion.SetIsBorderRequired(false);
        }
    }
    let _ = sesion.SetIsCursorCaptureEnabled(false);

    sesion.StartCapture()?;

    let resultado = recibe.recv_timeout(ESPERA_FOTOGRAMA);

    let _ = pool.RemoveFrameArrived(token);
    let _ = sesion.Close();
    let _ = pool.Close();

    let textura = resultado.map_err(|_| ErrorCaptura::SinFotograma)?;
    Ok(Instantanea { textura, area })
}

fn crear_textura(
    dispositivo: &Dispositivo,
    ancho: u32,
    alto: u32,
) -> Result<ID3D11Texture2D, ErrorCaptura> {
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
        BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut textura = None;
    // SAFETY: `desc` esta completamente inicializada y `textura` es una
    // variable local que la API rellena en caso de exito.
    unsafe {
        dispositivo
            .d3d()
            .CreateTexture2D(&desc, None, Some(&mut textura))?
    };
    textura.ok_or(ErrorCaptura::SinFotograma)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::monitores::enumerar_monitores;

    /// Pone el proceso de test en PerMonitorV2, como el manifiesto de
    /// pixpinmax.exe.
    ///
    /// Sin esto, Windows virtualiza las coordenadas del binario de test: la
    /// enumeracion diria 2000 px de ancho donde la GPU captura 3000 (150% de
    /// escalado), y la comparacion fisico-contra-fisico de estos tests seria
    /// imposible. Si el contexto ya esta puesto, la llamada falla y se
    /// ignora: es idempotente en la practica.
    fn asegurar_dpi_fisico() {
        use windows::Win32::UI::HiDpi::{
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
        };
        // SAFETY: cambia el contexto DPI del propio proceso de test antes de
        // usar cualquier API dependiente de DPI; no hay mas precondiciones.
        let _ =
            unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn captura_el_monitor_principal_con_su_tamano_real() {
        asegurar_dpi_fisico();
        let disp = Dispositivo::nuevo().unwrap();
        let monitores = enumerar_monitores().unwrap();
        let principal = *monitores.principal().expect("siempre hay uno principal");

        let inst = capturar_monitor(&disp, principal.id, principal.area)
            .expect("deberia capturar el monitor principal");

        assert_eq!(inst.area(), principal.area);

        // La textura debe tener exactamente el tamano del monitor. Si no
        // coincidiera, el recorte posterior tomaria la region equivocada y
        // nadie se enteraria hasta ver la imagen guardada.
        let mut desc = Default::default();
        // SAFETY: `textura()` devuelve una interfaz viva mientras viva
        // `inst`; `GetDesc` solo rellena la estructura que se le pasa.
        unsafe { inst.textura().GetDesc(&mut desc) };
        assert_eq!(desc.Width, principal.area.ancho);
        assert_eq!(desc.Height, principal.area.alto);
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn recortar_produce_una_textura_del_tamano_pedido() {
        asegurar_dpi_fisico();
        let disp = Dispositivo::nuevo().unwrap();
        let monitores = enumerar_monitores().unwrap();
        let principal = *monitores.principal().unwrap();
        let inst = capturar_monitor(&disp, principal.id, principal.area).unwrap();

        let region = Rect {
            x: principal.area.x + 10,
            y: principal.area.y + 20,
            ancho: 100,
            alto: 50,
        };
        let recorte = inst.recortar(&disp, region).expect("deberia recortar");

        assert_eq!(recorte.area(), region);
        let mut desc = Default::default();
        // SAFETY: igual que arriba.
        unsafe { recorte.textura().GetDesc(&mut desc) };
        assert_eq!(desc.Width, 100);
        assert_eq!(desc.Height, 50);
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn recortar_fuera_de_la_instantanea_da_error() {
        // Caso negativo: sin esta comprobacion, `CopySubresourceRegion`
        // recortaria en silencio y devolveria una textura con basura.
        let disp = Dispositivo::nuevo().unwrap();
        let monitores = enumerar_monitores().unwrap();
        let principal = *monitores.principal().unwrap();
        let inst = capturar_monitor(&disp, principal.id, principal.area).unwrap();

        let fuera = Rect {
            x: principal.area.x - 5000,
            y: principal.area.y,
            ancho: 100,
            alto: 50,
        };
        assert!(inst.recortar(&disp, fuera).is_err());
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn la_bajada_a_cpu_respeta_el_relleno_de_fila() {
        // El fallo clasico aqui es copiar el buffer de golpe ignorando el
        // RowPitch, que casi nunca vale ancho*4. El sintoma es una imagen
        // inclinada. Se detecta comprobando que el numero de bytes es
        // exactamente ancho*alto*4, sin el relleno.
        use crate::mapa::a_imagen;
        asegurar_dpi_fisico();
        let disp = Dispositivo::nuevo().unwrap();
        let monitores = crate::monitores::enumerar_monitores().unwrap();
        let principal = *monitores.principal().unwrap();
        let inst = capturar_monitor(&disp, principal.id, principal.area).unwrap();
        // Un ancho deliberadamente raro para forzar relleno.
        let region = Rect {
            x: principal.area.x,
            y: principal.area.y,
            ancho: 37,
            alto: 11,
        };
        let recorte = inst.recortar(&disp, region).unwrap();

        let img = a_imagen(&disp, &recorte).unwrap();
        assert_eq!(img.ancho, 37);
        assert_eq!(img.alto, 11);
        assert_eq!(
            img.pixeles.len(),
            37 * 11 * 4,
            "quedo relleno de fila en el buffer"
        );
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn un_monitor_inexistente_da_error_en_vez_de_entrar_en_panico() {
        asegurar_dpi_fisico();
        let disp = Dispositivo::nuevo().unwrap();
        let r = Rect {
            x: 0,
            y: 0,
            ancho: 100,
            alto: 100,
        };
        assert!(capturar_monitor(&disp, 9999, r).is_err());
    }
}
