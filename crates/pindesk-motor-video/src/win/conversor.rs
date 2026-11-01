//! **BGRA -> NV12, encogido y con bandas, en la GPU**: el procesador de
//! video de Direct3D 11 (`ID3D11VideoProcessor`).
//!
//! Es la pieza que deja la CPU fuera: el mismo bloque de la grafica que
//! escala los videos al reproducirlos convierte el color (BT.709, rango de
//! estudio 16-235, que es lo que espera cualquier reproductor de H.264) y
//! encoge la pantalla al tamano del video, todo en una pasada y sin que la
//! imagen salga de la memoria de video. En una grafica Intel de tercera
//! generacion son unos pocos milisegundos por fotograma.
//!
//! - La entrada es la textura del fotograma de la captura, **mientras el
//!   fotograma sigue retenido** (ver `captura`). Si el controlador no deja
//!   ver esa textura como entrada del procesador, se copia antes a una
//!   textura propia (una copia en la GPU, sin bajar nada).
//! - La salida es una textura NV12: la del repartidor de muestras de Media
//!   Foundation en el camino de la grafica, o una propia en el de software.
//! - Las vistas de entrada y de salida se guardan por textura: las
//!   superficies del pool y las del repartidor se reutilizan siempre, asi
//!   que crear una vista por fotograma seria trabajo tirado.

use std::mem::ManuallyDrop;

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_SUBRESOURCE_DATA,
    D3D11_TEX2D_ARRAY_VPOV, D3D11_TEX2D_VPIV, D3D11_TEX2D_VPOV, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, D3D11_VIDEO_COLOR, D3D11_VIDEO_COLOR_0, D3D11_VIDEO_COLOR_RGBA,
    D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE, D3D11_VIDEO_PROCESSOR_COLOR_SPACE,
    D3D11_VIDEO_PROCESSOR_CONTENT_DESC, D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_INPUT,
    D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_OUTPUT, D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC,
    D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0, D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC,
    D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0, D3D11_VIDEO_PROCESSOR_STREAM,
    D3D11_VIDEO_USAGE_PLAYBACK_NORMAL, D3D11_VPIV_DIMENSION_TEXTURE2D,
    D3D11_VPOV_DIMENSION_TEXTURE2D, D3D11_VPOV_DIMENSION_TEXTURE2DARRAY, ID3D11Device,
    ID3D11DeviceContext, ID3D11Multithread, ID3D11Texture2D, ID3D11VideoContext,
    ID3D11VideoContext1, ID3D11VideoDevice, ID3D11VideoProcessor, ID3D11VideoProcessorEnumerator,
    ID3D11VideoProcessorInputView, ID3D11VideoProcessorOutputView,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709, DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709,
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_NV12, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
};
use windows::core::Interface;

use super::com::{EnLaGpu, en, multihilo};
use crate::ErrorMotor;
use crate::geometria;

/// La entrada: RGB de rango completo (0-255), como pinta Windows. Todos
/// los bits a cero: uso de reproduccion, RGB completo.
const ENTRADA_RGB_COMPLETO: u32 = 0;

/// La salida: YCbCr BT.709 (bit 2) de rango de estudio 16-235 (bits 4-5 =
/// 1). Es lo que el tipo de medio del codificador declara (`escritor`), y
/// declararlo distinto de lo que se convierte da los grises lavados.
const SALIDA_709_ESTUDIO: u32 = (1 << 2) | (1 << 4);

/// Vistas guardadas como mucho. Las del pool son dos y las del repartidor
/// ocho; si hay mas es que el pool se recreo y las viejas ya no sirven.
const VISTAS_DE_ENTRADA: usize = 4;
const VISTAS_DE_SALIDA: usize = crate::cola::MUESTRAS_GPU as usize + 2;

/// Lado de la textura negra con la que se pinta un fotograma sin pantalla.
const LADO_NEGRO: u32 = 16;

pub(crate) struct Conversor {
    // Las vistas antes que el procesador y el enumerador: se sueltan en el
    // orden en que estan declarados los campos.
    entradas: Vec<(ID3D11Texture2D, ID3D11VideoProcessorInputView)>,
    salidas: Vec<(ID3D11Texture2D, u32, ID3D11VideoProcessorOutputView)>,
    intermedia: Option<(ID3D11Texture2D, u32, u32)>,
    negra: Option<ID3D11Texture2D>,
    procesador: ID3D11VideoProcessor,
    enumerador: ID3D11VideoProcessorEnumerator,
    video_ctx: ID3D11VideoContext,
    video: ID3D11VideoDevice,
    contexto: ID3D11DeviceContext,
    d3d: ID3D11Device,
    multi: Option<ID3D11Multithread>,
    /// El tamano de entrada para el que se creo el procesador.
    entrada: (u32, u32),
    salida: (u32, u32),
    por_segundo: u32,
    /// El controlador no deja usar la textura del pool como entrada: se
    /// copia antes a `intermedia`.
    copiar: bool,
    /// El tamano de la ultima textura de entrada, para tirar las vistas
    /// viejas cuando cambia.
    ultima_entrada: (u32, u32),
}

impl Conversor {
    /// Un procesador para entradas de hasta `entrada` y salida `salida`.
    pub fn nuevo(
        d3d: &ID3D11Device,
        contexto: &ID3D11DeviceContext,
        entrada: (u32, u32),
        salida: (u32, u32),
        por_segundo: u32,
    ) -> Result<Conversor, ErrorMotor> {
        let video: ID3D11VideoDevice =
            d3d.cast().map_err(en("ver el dispositivo como de video"))?;
        let video_ctx: ID3D11VideoContext = contexto
            .cast()
            .map_err(en("ver el contexto como de video"))?;
        let (enumerador, procesador) =
            crear_procesador(&video, &video_ctx, entrada, salida, por_segundo)?;
        Ok(Conversor {
            entradas: Vec::new(),
            salidas: Vec::new(),
            intermedia: None,
            negra: None,
            procesador,
            enumerador,
            video_ctx,
            video,
            contexto: contexto.clone(),
            d3d: d3d.clone(),
            multi: multihilo(d3d),
            entrada,
            salida,
            por_segundo,
            copiar: false,
            ultima_entrada: (0, 0),
        })
    }

    /// **El mismo procesador en otro dispositivo**: tras perderse la
    /// grafica (`crate::recuperacion`), con las mismas medidas.
    pub fn rehacer(
        &self,
        d3d: &ID3D11Device,
        contexto: &ID3D11DeviceContext,
    ) -> Result<Conversor, ErrorMotor> {
        Conversor::nuevo(d3d, contexto, self.entrada, self.salida, self.por_segundo)
    }

    /// Cambia el tamano de salida: el reintento de la grafica a un tamano
    /// mas pequeno (`cadena::reintento_mas_pequeno`) o con el tope de su
    /// codificador (`cadena::peculiaridades`).
    pub fn con_salida(&mut self, salida: (u32, u32)) -> Result<(), ErrorMotor> {
        if salida == self.salida {
            return Ok(());
        }
        let (enumerador, procesador) = crear_procesador(
            &self.video,
            &self.video_ctx,
            self.entrada,
            salida,
            self.por_segundo,
        )?;
        self.entradas.clear();
        self.salidas.clear();
        self.enumerador = enumerador;
        self.procesador = procesador;
        self.salida = salida;
        Ok(())
    }

    /// **Convierte** lo que tiene imagen de `origen` (`contenido`, desde la
    /// esquina) en el NV12 `destino` (subrecurso `sub`), encajado con bandas.
    pub fn convertir(
        &mut self,
        origen: &ID3D11Texture2D,
        contenido: (u32, u32),
        destino: &ID3D11Texture2D,
        sub: u32,
    ) -> Result<(), ErrorMotor> {
        let desc = descripcion(origen);
        let (cw, ch) = (
            contenido.0.min(desc.Width).max(1),
            contenido.1.min(desc.Height).max(1),
        );
        if (desc.Width, desc.Height) != self.ultima_entrada {
            // El pool se recreo: las vistas viejas tienen sus texturas
            // vivas y no van a volver.
            self.entradas.clear();
            self.ultima_entrada = (desc.Width, desc.Height);
        }
        if desc.Width > self.entrada.0 || desc.Height > self.entrada.1 {
            self.recrear((
                desc.Width.max(self.entrada.0),
                desc.Height.max(self.entrada.1),
            ))?;
        }
        let vista = if self.copiar {
            self.vista_copiando(origen, &desc, (cw, ch))?
        } else {
            match self.vista_de_entrada(origen) {
                Ok(v) => v,
                Err(e) => {
                    tracing::info!(
                        ?e,
                        "la textura de la captura no vale como entrada del procesador; se copia"
                    );
                    self.copiar = true;
                    self.vista_copiando(origen, &desc, (cw, ch))?
                }
            }
        };
        let salida = self.vista_de_salida(destino, sub)?;
        self.pasar(&vista, &salida, (cw, ch))
    }

    /// Pinta un fotograma **negro** en `destino`: el del principio cuando la
    /// pantalla aun no dio imagen.
    pub fn negro(&mut self, destino: &ID3D11Texture2D, sub: u32) -> Result<(), ErrorMotor> {
        if self.negra.is_none() {
            self.negra = Some(textura_negra(&self.d3d)?);
        }
        let Some(negra) = self.negra.clone() else {
            return Err(ErrorMotor::SinFotogramas);
        };
        let vista = self.vista_de_entrada(&negra)?;
        let salida = self.vista_de_salida(destino, sub)?;
        // La textura negra encajada deja el resto en el fondo, que tambien
        // es negro.
        self.pasar(&vista, &salida, (LADO_NEGRO, LADO_NEGRO))
    }

    /// Copia un NV12 a otro en la GPU: el fotograma repetido del camino de
    /// la grafica (ver `tubo`).
    pub fn copiar_nv12(
        &self,
        origen: &ID3D11Texture2D,
        sub_origen: u32,
        destino: &ID3D11Texture2D,
        sub_destino: u32,
    ) {
        let _gpu = EnLaGpu::entrar(self.multi.as_ref());
        // SAFETY: las dos texturas son NV12 del mismo tamano (las da el
        // mismo repartidor) y del mismo dispositivo; la copia se encola en
        // el contexto inmediato, protegido para varios hilos.
        unsafe {
            self.contexto.CopySubresourceRegion(
                destino,
                sub_destino,
                0,
                0,
                0,
                origen,
                sub_origen,
                None,
            )
        };
    }

    fn pasar(
        &self,
        vista: &ID3D11VideoProcessorInputView,
        salida: &ID3D11VideoProcessorOutputView,
        (cw, ch): (u32, u32),
    ) -> Result<(), ErrorMotor> {
        let r = geometria::encaje(cw, ch, self.salida.0, self.salida.1);
        let fuente = RECT {
            left: 0,
            top: 0,
            right: cw as i32,
            bottom: ch as i32,
        };
        let destino = RECT {
            left: r.x as i32,
            top: r.y as i32,
            right: (r.x + r.ancho) as i32,
            bottom: (r.y + r.alto) as i32,
        };
        let corriente = D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: true.into(),
            pInputSurface: ManuallyDrop::new(Some(vista.clone())),
            ..Default::default()
        };
        let gpu = EnLaGpu::entrar(self.multi.as_ref());
        // SAFETY: procesador, vistas y rectangulos son de este dispositivo y
        // estan vivos durante la llamada; el rectangulo de origen no pasa
        // de la textura (`convertir` lo recorta) y el de destino no pasa
        // del video (`geometria::encaje`). La corriente lleva una
        // referencia propia a la vista, que se suelta justo despues.
        let hecho = unsafe {
            self.video_ctx.VideoProcessorSetStreamSourceRect(
                &self.procesador,
                0,
                true,
                Some(&fuente),
            );
            self.video_ctx.VideoProcessorSetStreamDestRect(
                &self.procesador,
                0,
                true,
                Some(&destino),
            );
            self.video_ctx.VideoProcessorBlt(
                &self.procesador,
                salida,
                0,
                std::slice::from_ref(&corriente),
            )
        };
        drop(gpu);
        drop(ManuallyDrop::into_inner(corriente.pInputSurface));
        hecho.map_err(en("convertir el fotograma en la GPU"))
    }

    fn recrear(&mut self, entrada: (u32, u32)) -> Result<(), ErrorMotor> {
        let (enumerador, procesador) = crear_procesador(
            &self.video,
            &self.video_ctx,
            entrada,
            self.salida,
            self.por_segundo,
        )?;
        // Las vistas se crearon con el enumerador viejo.
        self.entradas.clear();
        self.salidas.clear();
        self.enumerador = enumerador;
        self.procesador = procesador;
        self.entrada = entrada;
        Ok(())
    }

    fn vista_de_entrada(
        &mut self,
        textura: &ID3D11Texture2D,
    ) -> Result<ID3D11VideoProcessorInputView, ErrorMotor> {
        if let Some((_, v)) = self
            .entradas
            .iter()
            .find(|(t, _)| t.as_raw() == textura.as_raw())
        {
            return Ok(v.clone());
        }
        let desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
            FourCC: 0,
            ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPIV {
                    MipSlice: 0,
                    ArraySlice: 0,
                },
            },
        };
        let mut vista = None;
        // SAFETY: textura y enumerador vivos y del mismo dispositivo; la
        // descripcion es completa; `vista` es local.
        unsafe {
            self.video.CreateVideoProcessorInputView(
                textura,
                &self.enumerador,
                &desc,
                Some(&mut vista),
            )
        }
        .map_err(en("crear la vista de entrada del procesador"))?;
        let vista = vista.ok_or(ErrorMotor::Sistema {
            paso: "crear la vista de entrada del procesador",
            detalle: "sin vista".into(),
            codigo: 0,
        })?;
        if self.entradas.len() >= VISTAS_DE_ENTRADA {
            self.entradas.clear();
        }
        self.entradas.push((textura.clone(), vista.clone()));
        Ok(vista)
    }

    fn vista_copiando(
        &mut self,
        origen: &ID3D11Texture2D,
        desc: &D3D11_TEXTURE2D_DESC,
        (cw, ch): (u32, u32),
    ) -> Result<ID3D11VideoProcessorInputView, ErrorMotor> {
        let vale = self
            .intermedia
            .as_ref()
            .is_some_and(|(_, w, h)| *w >= desc.Width && *h >= desc.Height);
        if !vale {
            let d = D3D11_TEXTURE2D_DESC {
                Width: desc.Width,
                Height: desc.Height,
                MipLevels: 1,
                ArraySize: 1,
                Format: desc.Format,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut t = None;
            // SAFETY: descripcion completa; `t` es local.
            unsafe { self.d3d.CreateTexture2D(&d, None, Some(&mut t)) }
                .map_err(en("crear la textura intermedia"))?;
            let t = t.ok_or(ErrorMotor::Sistema {
                paso: "crear la textura intermedia",
                detalle: "sin textura".into(),
                codigo: 0,
            })?;
            self.intermedia = Some((t, desc.Width, desc.Height));
        }
        let Some((intermedia, _, _)) = self.intermedia.clone() else {
            return Err(ErrorMotor::SinFotogramas);
        };
        let caja = D3D11_BOX {
            left: 0,
            top: 0,
            front: 0,
            right: cw,
            bottom: ch,
            back: 1,
        };
        {
            let _gpu = EnLaGpu::entrar(self.multi.as_ref());
            // SAFETY: mismo formato y dispositivo; la caja no pasa de
            // ninguna de las dos texturas (`cw`x`ch` cabe en el origen, y la
            // intermedia es por lo menos tan grande como el).
            unsafe {
                self.contexto
                    .CopySubresourceRegion(&intermedia, 0, 0, 0, 0, origen, 0, Some(&caja))
            };
        }
        self.vista_de_entrada(&intermedia)
    }

    fn vista_de_salida(
        &mut self,
        textura: &ID3D11Texture2D,
        sub: u32,
    ) -> Result<ID3D11VideoProcessorOutputView, ErrorMotor> {
        if let Some((_, _, v)) = self
            .salidas
            .iter()
            .find(|(t, s, _)| t.as_raw() == textura.as_raw() && *s == sub)
        {
            return Ok(v.clone());
        }
        let d = descripcion(textura);
        let desc = if d.ArraySize > 1 {
            D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2DARRAY,
                Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                    Texture2DArray: D3D11_TEX2D_ARRAY_VPOV {
                        MipSlice: 0,
                        FirstArraySlice: sub,
                        ArraySize: 1,
                    },
                },
            }
        } else {
            D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
                Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                    Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 },
                },
            }
        };
        let mut vista = None;
        // SAFETY: como en `vista_de_entrada`; el subrecurso es el que dio el
        // repartidor para esta textura.
        unsafe {
            self.video.CreateVideoProcessorOutputView(
                textura,
                &self.enumerador,
                &desc,
                Some(&mut vista),
            )
        }
        .map_err(en("crear la vista de salida del procesador"))?;
        let vista = vista.ok_or(ErrorMotor::Sistema {
            paso: "crear la vista de salida del procesador",
            detalle: "sin vista".into(),
            codigo: 0,
        })?;
        if self.salidas.len() >= VISTAS_DE_SALIDA {
            self.salidas.clear();
        }
        self.salidas.push((textura.clone(), sub, vista.clone()));
        Ok(vista)
    }
}

fn descripcion(t: &ID3D11Texture2D) -> D3D11_TEXTURE2D_DESC {
    let mut d = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: textura viva; `GetDesc` solo rellena la estructura local.
    unsafe { t.GetDesc(&mut d) };
    d
}

/// El enumerador (que sabe que formatos admite la grafica) y el procesador,
/// con el color y el fondo ya puestos.
fn crear_procesador(
    video: &ID3D11VideoDevice,
    video_ctx: &ID3D11VideoContext,
    entrada: (u32, u32),
    salida: (u32, u32),
    por_segundo: u32,
) -> Result<(ID3D11VideoProcessorEnumerator, ID3D11VideoProcessor), ErrorMotor> {
    let ritmo = DXGI_RATIONAL {
        Numerator: por_segundo.max(1),
        Denominator: 1,
    };
    let desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
        InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
        InputFrameRate: ritmo,
        InputWidth: entrada.0,
        InputHeight: entrada.1,
        OutputFrameRate: ritmo,
        OutputWidth: salida.0,
        OutputHeight: salida.1,
        Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
    };
    // SAFETY: descripcion completa; el dispositivo de video esta vivo.
    let enumerador = unsafe { video.CreateVideoProcessorEnumerator(&desc) }
        .map_err(en("crear el enumerador del procesador de video"))?;
    // SAFETY: consultas de solo lectura al enumerador recien creado.
    let (bgra, nv12) = unsafe {
        (
            enumerador
                .CheckVideoProcessorFormat(DXGI_FORMAT_B8G8R8A8_UNORM)
                .unwrap_or(0),
            enumerador
                .CheckVideoProcessorFormat(DXGI_FORMAT_NV12)
                .unwrap_or(0),
        )
    };
    if bgra & D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_INPUT.0 as u32 == 0
        || nv12 & D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_OUTPUT.0 as u32 == 0
    {
        return Err(ErrorMotor::Sistema {
            paso: "comprobar el procesador de video",
            detalle: "la grafica no convierte BGRA a NV12".into(),
            codigo: 0,
        });
    }
    // SAFETY: enumerador vivo; el indice 0 de conversion de ritmo existe
    // siempre (es «sin conversion»).
    let procesador = unsafe { video.CreateVideoProcessor(&enumerador, 0) }
        .map_err(en("crear el procesador de video"))?;
    let negro = D3D11_VIDEO_COLOR {
        Anonymous: D3D11_VIDEO_COLOR_0 {
            RGBA: D3D11_VIDEO_COLOR_RGBA {
                R: 0.0,
                G: 0.0,
                B: 0.0,
                A: 1.0,
            },
        },
    };
    let rect_salida = RECT {
        left: 0,
        top: 0,
        right: salida.0 as i32,
        bottom: salida.1 as i32,
    };
    // SAFETY: procesador recien creado en este dispositivo; todas las
    // estructuras son locales y viven durante cada llamada.
    unsafe {
        video_ctx.VideoProcessorSetStreamFrameFormat(
            &procesador,
            0,
            D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
        );
        video_ctx.VideoProcessorSetStreamColorSpace(
            &procesador,
            0,
            &D3D11_VIDEO_PROCESSOR_COLOR_SPACE {
                _bitfield: ENTRADA_RGB_COMPLETO,
            },
        );
        video_ctx.VideoProcessorSetOutputColorSpace(
            &procesador,
            &D3D11_VIDEO_PROCESSOR_COLOR_SPACE {
                _bitfield: SALIDA_709_ESTUDIO,
            },
        );
        // Con D3D11.1 (Windows 8 en adelante, casi todos los controladores)
        // el color se dice con los espacios de DXGI, que los controladores
        // respetan mejor que los bits de arriba (hay Intel antiguos que
        // ignoran el rango de estudio en la forma vieja y dan negros
        // grises). Se ponen los dos: el nuevo, si esta, manda.
        if let Ok(ctx1) = video_ctx.cast::<ID3D11VideoContext1>() {
            ctx1.VideoProcessorSetStreamColorSpace1(
                &procesador,
                0,
                DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709,
            );
            ctx1.VideoProcessorSetOutputColorSpace1(
                &procesador,
                DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709,
            );
        }
        // Sin «mejoras» del controlador (ruido, bordes): en texto hacen mas
        // mal que bien, y cuestan.
        video_ctx.VideoProcessorSetStreamAutoProcessingMode(&procesador, 0, false);
        video_ctx.VideoProcessorSetOutputBackgroundColor(&procesador, false, &negro);
        video_ctx.VideoProcessorSetOutputTargetRect(&procesador, true, Some(&rect_salida));
    }
    Ok((enumerador, procesador))
}

/// Una textura BGRA negra y opaca de [`LADO_NEGRO`] de lado.
fn textura_negra(d3d: &ID3D11Device) -> Result<ID3D11Texture2D, ErrorMotor> {
    let pixeles: Vec<u8> = [0u8, 0, 0, 255].repeat((LADO_NEGRO * LADO_NEGRO) as usize);
    let d = D3D11_TEXTURE2D_DESC {
        Width: LADO_NEGRO,
        Height: LADO_NEGRO,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let inicial = D3D11_SUBRESOURCE_DATA {
        pSysMem: pixeles.as_ptr().cast(),
        SysMemPitch: LADO_NEGRO * 4,
        SysMemSlicePitch: 0,
    };
    let mut t = None;
    // SAFETY: `pixeles` tiene `LADO_NEGRO` filas de `LADO_NEGRO * 4` bytes y
    // vive durante la llamada, que copia los datos.
    unsafe { d3d.CreateTexture2D(&d, Some(&inicial), Some(&mut t)) }
        .map_err(en("crear la textura negra"))?;
    t.ok_or(ErrorMotor::Sistema {
        paso: "crear la textura negra",
        detalle: "sin textura".into(),
        codigo: 0,
    })
}
