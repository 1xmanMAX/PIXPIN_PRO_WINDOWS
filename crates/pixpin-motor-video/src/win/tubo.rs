//! **Los caminos** de la cadena de caidas (`crate::cadena`), con la misma
//! cara para el motor: un fotograma nuevo, uno repetido, un bloque de
//! sonido, cerrar y —si la grafica se pierde— volver a montarse en otro
//! dispositivo sin cerrar el MP4 ([`Tubo::recrear`]).
//!
//! - [`Grafica`]: el procesador de video escribe en una textura NV12 del
//!   repartidor de muestras de Media Foundation y esa misma textura va al
//!   codificador de la grafica. Nada pasa por la CPU.
//! - [`Software`]: el procesador de video escribe en una textura NV12
//!   propia, que baja por una de escenificacion (1,5 bytes por pixel, no 4)
//!   al codificador de Microsoft o, en el camino
//!   `cadena::Camino::GraficaEnMemoria`, al de la grafica que haya.
//! - [`Cpu`]: lo de antes (`pixpin-record`): la pantalla baja en BGRA, se
//!   encoge en la CPU y se codifica por software.
//!
//! El backend de Windows de [`crate::plataforma::Tuberia`] es
//! [`EnLaGrafica`]: un tubo con el dispositivo en el que vive.

use std::path::{Path, PathBuf};

use pixpin_capture::Dispositivo;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_VIDEO_ENCODER, D3D11_CPU_ACCESS_READ, D3D11_MAP_READ,
    D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
    ID3D11DeviceContext, ID3D11Multithread, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC};
use windows::Win32::Media::MediaFoundation::{
    IMF2DBuffer, IMFDXGIBuffer, IMFDXGIDeviceManager, IMFMediaBuffer, IMFSample,
    IMFVideoSampleAllocatorEx, MF_E_SAMPLEALLOCATOR_EMPTY, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE,
    MF_MT_SUBTYPE, MF_SA_D3D11_BINDFLAGS, MF_SA_D3D11_USAGE, MFCreateAttributes,
    MFCreateDXGIDeviceManager, MFCreateMediaType, MFCreateVideoSampleAllocatorEx,
    MFMediaType_Video, MFVideoFormat_NV12,
};
use windows::core::Interface;

use super::captura::Retenido;
use super::com::{EnLaGpu, en, multihilo};
use super::conversor::Conversor;
use super::escritor::{Escritor, Plan, buffer_de, cerrar_buffer, muestra_de};
use crate::archivo::{self, TrasFalloAlCerrar};
use crate::audio::{FormatoAudio, a_mono};
use crate::cola::{MUESTRAS_GPU, MUESTRAS_GPU_INICIALES, admitir_en_software};
use crate::lienzo::Lienzo;
use crate::perfil::{Ajustes, bits_por_segundo};
pub(crate) use crate::plataforma::Escrito;
use crate::ritmo::ocupa_el_sitio_del_repetido;
use crate::{Camino, Codificador, ErrorMotor, Formato};

/// Lo que comparten los caminos para abrirse.
pub(crate) struct Pedido<'a> {
    pub destino: &'a Path,
    pub formato: Formato,
    pub medidas: (u32, u32),
    pub ajustes: Ajustes,
    pub audio: Option<FormatoAudio>,
    pub fragmentado: bool,
    /// Lo minimo entre dos fotogramas (`ritmo::minimo_entre`).
    pub minimo: i64,
}

/// Como quedo el MP4 al cerrarlo.
pub(crate) struct Cerrado {
    pub ruta: PathBuf,
    pub fragmentado: bool,
    /// El cierre fallo pero el fichero se pudo conservar (fragmentado).
    pub fallo: Option<String>,
}

/// El `.parte` mientras no se cierre bien: soltarlo lo borra.
struct Parte {
    parcial: PathBuf,
    destino: PathBuf,
    vivo: bool,
}

impl Parte {
    fn de(destino: &Path) -> Parte {
        if let Some(padre) = destino.parent()
            && !padre.as_os_str().is_empty()
        {
            let _ = std::fs::create_dir_all(padre);
        }
        Parte {
            parcial: archivo::a_medias(destino),
            destino: destino.to_path_buf(),
            vivo: true,
        }
    }

    /// El MP4 ya cerrado y valido: a su nombre. Si no se puede, el `.parte`
    /// se queda con su contenido (ver `archivo::poner_en_su_sitio`).
    fn poner(mut self) -> Result<PathBuf, ErrorMotor> {
        self.vivo = false;
        archivo::poner_en_su_sitio(&self.parcial, &self.destino)
    }

    /// **Tras un `Finalize`**: si fue bien, a su nombre; si fallo, lo que
    /// diga `archivo::tras_fallo_al_cerrar` (un fragmentado se conserva, un
    /// MP4 normal sin indice se borra al soltarse).
    fn cerrar(
        self,
        hecho: Result<(), ErrorMotor>,
        fragmentado: bool,
    ) -> Result<Cerrado, ErrorMotor> {
        match hecho {
            Ok(()) => self.poner().map(|ruta| Cerrado {
                ruta,
                fragmentado,
                fallo: None,
            }),
            Err(e) => {
                let bytes = archivo::bytes(&self.parcial);
                match archivo::tras_fallo_al_cerrar(fragmentado, bytes) {
                    TrasFalloAlCerrar::Conservar => {
                        tracing::warn!(?e, bytes, "el cierre fallo; el MP4 fragmentado se queda");
                        self.poner().map(|ruta| Cerrado {
                            ruta,
                            fragmentado,
                            fallo: Some(e.to_string()),
                        })
                    }
                    TrasFalloAlCerrar::Borrar => Err(e),
                }
            }
        }
    }
}

impl Drop for Parte {
    fn drop(&mut self) {
        if self.vivo {
            let _ = std::fs::remove_file(&self.parcial);
        }
    }
}

pub(crate) enum Tubo {
    Grafica(Grafica),
    Software(Software),
    Cpu(Cpu),
}

impl Tubo {
    pub fn camino(&self) -> Camino {
        match self {
            Tubo::Grafica(_) => Camino::Grafica,
            Tubo::Software(s) if s.con_grafica => Camino::GraficaEnMemoria,
            Tubo::Software(_) => Camino::Software,
            Tubo::Cpu(_) => Camino::Cpu,
        }
    }

    pub fn codificador(&self) -> Codificador {
        self.camino().codificador()
    }

    pub fn formato(&self) -> Formato {
        match self {
            Tubo::Grafica(g) => g.formato,
            Tubo::Software(_) | Tubo::Cpu(_) => Formato::H264,
        }
    }

    pub fn medidas(&self) -> (u32, u32) {
        match self {
            Tubo::Grafica(g) => g.medidas,
            Tubo::Software(s) => s.medidas,
            Tubo::Cpu(c) => c.medidas,
        }
    }

    /// Donde se esta escribiendo.
    pub fn parcial(&self) -> &Path {
        match self {
            Tubo::Grafica(g) => &g.parte.parcial,
            Tubo::Software(s) => &s.parte.parcial,
            Tubo::Cpu(c) => &c.parcial,
        }
    }

    pub fn escritos(&self) -> u64 {
        match self {
            Tubo::Grafica(g) => g.escritos,
            Tubo::Software(s) => s.escritos,
            Tubo::Cpu(c) => c.escritos,
        }
    }

    /// Un fotograma de la pantalla, mientras su fotograma de la captura
    /// sigue retenido.
    pub fn fotograma(
        &mut self,
        disp: &Dispositivo,
        origen: &ID3D11Texture2D,
        contenido: (u32, u32),
        inicio: i64,
        duracion: i64,
    ) -> Result<Escrito, ErrorMotor> {
        match self {
            Tubo::Grafica(g) => g.fotograma(origen, contenido, inicio),
            Tubo::Software(s) => s.fotograma(origen, contenido, inicio),
            Tubo::Cpu(c) => c.fotograma(disp, origen, contenido, inicio, duracion),
        }
    }

    /// El ultimo otra vez (o uno negro si aun no hay ninguno).
    pub fn repetir(&mut self, inicio: i64, duracion: i64) -> Result<Escrito, ErrorMotor> {
        match self {
            Tubo::Grafica(g) => g.repetir(inicio),
            Tubo::Software(s) => s.repetir(inicio),
            Tubo::Cpu(c) => c.repetir(inicio, duracion),
        }
    }

    /// **El ultimo fotograma, hasta `fin`**: el que esperaba su duracion se
    /// da hasta ahi, para que el video dure lo mismo que el sonido.
    /// `duracion` es la de un fotograma al ritmo maximo, el minimo.
    pub fn vaciar(&mut self, fin: i64, duracion: i64) -> Result<(), ErrorMotor> {
        match self {
            Tubo::Grafica(g) => g.vaciar(fin, duracion),
            Tubo::Software(s) => s.vaciar(fin, duracion),
            Tubo::Cpu(c) => c.vaciar(fin, duracion),
        }
    }

    pub fn audio(&mut self, muestras: &[i16], posicion: u64) -> Result<(), ErrorMotor> {
        match self {
            Tubo::Grafica(g) => g.escritor.audio(muestras, posicion),
            Tubo::Software(s) => s.escritor.audio(muestras, posicion),
            Tubo::Cpu(c) => c.audio(muestras),
        }
    }

    /// **Vuelve a montar lo que vive en la grafica** sobre `disp`, un
    /// dispositivo nuevo tras perderse el anterior, **sin cerrar el MP4**:
    /// el escritor (y con el lo ya escrito) sigue. Lo que se pierde es el
    /// fotograma que esperaba en la GPU, que se cambia por uno negro en su
    /// mismo instante para que no quede un hueco.
    pub fn recrear(&mut self, disp: &Dispositivo) -> Result<(), ErrorMotor> {
        match self {
            Tubo::Grafica(g) => g.recrear(disp),
            Tubo::Software(s) => s.recrear(disp),
            Tubo::Cpu(c) => {
                // La bajada guarda texturas del dispositivo viejo.
                c.bajada = pixpin_capture::Bajada::default();
                Ok(())
            }
        }
    }

    /// **Cierra el MP4** y lo pone en su nombre.
    pub fn terminar(self) -> Result<Cerrado, ErrorMotor> {
        match self {
            Tubo::Grafica(g) => g.terminar(),
            Tubo::Software(s) => s.terminar(),
            Tubo::Cpu(c) => c.terminar().map(|ruta| Cerrado {
                ruta,
                fragmentado: false,
                fallo: None,
            }),
        }
    }
}

/// **El tubo visto como [`crate::plataforma::Tuberia`]**: con el
/// dispositivo en el que vive, que el camino de la CPU necesita para bajar
/// la pantalla.
pub(crate) struct EnLaGrafica<'a> {
    pub tubo: &'a mut Tubo,
    pub disp: &'a Dispositivo,
}

impl crate::plataforma::Tuberia for EnLaGrafica<'_> {
    type Retenido = Retenido;

    fn fotograma(
        &mut self,
        r: &Retenido,
        inicio: i64,
        duracion: i64,
    ) -> Result<Escrito, ErrorMotor> {
        match r.textura() {
            Ok(tex) => self
                .tubo
                .fotograma(self.disp, &tex, r.contenido, inicio, duracion),
            // Un fotograma sin textura (raro: la superficie de un pool
            // recien recreado) se salta, no corta el video.
            Err(e) => {
                tracing::debug!(?e, "un fotograma sin textura");
                Ok(Escrito::Caido)
            }
        }
    }

    fn repetir(&mut self, inicio: i64, duracion: i64) -> Result<Escrito, ErrorMotor> {
        self.tubo.repetir(inicio, duracion)
    }

    fn audio(&mut self, muestras: &[i16], posicion: u64) -> Result<(), ErrorMotor> {
        self.tubo.audio(muestras, posicion)
    }

    fn vaciar(&mut self, fin: i64, duracion: i64) -> Result<(), ErrorMotor> {
        self.tubo.vaciar(fin, duracion)
    }

    fn escritos(&self) -> u64 {
        self.tubo.escritos()
    }
}

// --- La grafica ----------------------------------------------------------

/// Una textura NV12 del repartidor, con la muestra que la envuelve.
struct Ranura {
    muestra: IMFSample,
    textura: ID3D11Texture2D,
    sub: u32,
}

/// **El repartidor de muestras** de Media Foundation sobre la GPU: da
/// muestras que envuelven texturas NV12 (`MFCreateDXGISurfaceBuffer` por
/// dentro) y las recupera cuando el codificador las suelta. Es la cola con
/// tope del camino de la grafica: si estan todas dentro, no hay muestra y
/// el fotograma se salta.
struct Lote {
    repartidor: IMFVideoSampleAllocatorEx,
}

impl Lote {
    fn nuevo(gestor: &IMFDXGIDeviceManager, (w, h): (u32, u32)) -> Result<Lote, ErrorMotor> {
        let mut crudo = std::ptr::null_mut();
        // SAFETY: se pide la interfaz por su IID y la funcion deja en
        // `crudo` un puntero con una referencia propia, que pasa a ser de
        // `repartidor`.
        let repartidor = unsafe {
            MFCreateVideoSampleAllocatorEx(&IMFVideoSampleAllocatorEx::IID, &mut crudo)
                .map_err(en("crear el repartidor de muestras"))?;
            IMFVideoSampleAllocatorEx::from_raw(crudo)
        };
        // SAFETY: tipo propio con claves constantes; el gestor esta vivo.
        let tipo = unsafe {
            repartidor
                .SetDirectXManager(gestor)
                .map_err(en("darle la GPU al repartidor"))?;
            let t = MFCreateMediaType().map_err(en("crear el tipo del repartidor"))?;
            t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .and_then(|()| t.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12))
                .and_then(|()| t.SetUINT64(&MF_MT_FRAME_SIZE, (u64::from(w) << 32) | u64::from(h)))
                .map_err(en("describir las texturas del repartidor"))?;
            t
        };
        // Destino del procesador de video (render target) y, si el
        // controlador lo admite, marcadas para el codificador: algunos
        // (Intel) las leen sin copia solo asi.
        let mut ultimo = None;
        for usos in [
            D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_VIDEO_ENCODER.0,
            D3D11_BIND_RENDER_TARGET.0,
        ] {
            let mut a = None;
            // SAFETY: `a` es local; luego solo recibe claves constantes.
            let hecho = unsafe {
                MFCreateAttributes(&mut a, 2)
                    .and_then(|()| {
                        let a = a.as_ref().ok_or_else(windows::core::Error::empty)?;
                        a.SetUINT32(&MF_SA_D3D11_BINDFLAGS, usos as u32)?;
                        a.SetUINT32(&MF_SA_D3D11_USAGE, D3D11_USAGE_DEFAULT.0 as u32)
                    })
                    .and_then(|()| {
                        repartidor.InitializeSampleAllocatorEx(
                            MUESTRAS_GPU_INICIALES,
                            MUESTRAS_GPU,
                            a.as_ref(),
                            &tipo,
                        )
                    })
            };
            match hecho {
                Ok(()) => return Ok(Lote { repartidor }),
                Err(e) => ultimo = Some(e),
            }
        }
        Err(en("preparar las texturas del repartidor")(
            ultimo.unwrap_or_else(windows::core::Error::empty),
        ))
    }

    /// Una muestra libre, o `None` si estan todas en el codificador.
    fn pedir(&self) -> Result<Option<Ranura>, ErrorMotor> {
        // SAFETY: repartidor vivo e inicializado.
        let muestra = match unsafe { self.repartidor.AllocateSample() } {
            Ok(m) => m,
            Err(e) if e.code() == MF_E_SAMPLEALLOCATOR_EMPTY => return Ok(None),
            Err(e) => return Err(en("pedir una muestra al repartidor")(e)),
        };
        // SAFETY: la muestra del repartidor tiene un buffer DXGI; se pide su
        // textura por IID y el puntero devuelto trae su propia referencia.
        unsafe {
            let buffer: IMFMediaBuffer = muestra
                .GetBufferByIndex(0)
                .map_err(en("leer el buffer de una muestra"))?;
            let dxgi: IMFDXGIBuffer = buffer.cast().map_err(en("ver el buffer como DXGI"))?;
            let mut crudo = std::ptr::null_mut();
            dxgi.GetResource(&ID3D11Texture2D::IID, &mut crudo)
                .map_err(en("sacar la textura de una muestra"))?;
            let textura = ID3D11Texture2D::from_raw(crudo);
            let sub = dxgi
                .GetSubresourceIndex()
                .map_err(en("leer el subrecurso de una muestra"))?;
            // Hay codificadores que miran la longitud aunque lean la
            // textura: se le pone la del NV12 entero.
            if let Ok(b2) = buffer.cast::<IMF2DBuffer>()
                && let Ok(n) = b2.GetContiguousLength()
            {
                let _ = buffer.SetCurrentLength(n);
            }
            Ok(Some(Ranura {
                muestra,
                textura,
                sub,
            }))
        }
    }
}

impl Drop for Lote {
    fn drop(&mut self) {
        // SAFETY: repartidor vivo; deja de dar muestras y suelta las
        // texturas que tenga libres (las que siguen fuera se sueltan al
        // soltarse sus muestras).
        unsafe {
            let _ = self.repartidor.UninitializeSampleAllocator();
        }
    }
}

/// El fotograma que espera su duracion (el retraso de un fotograma).
struct Pendiente<T> {
    dato: T,
    inicio: i64,
    /// Si es el latido (un repetido) y no una imagen nueva.
    repetido: bool,
}

pub(crate) struct Grafica {
    // En este orden se sueltan: el escritor (y con el el codificador, que
    // devuelve sus muestras), la muestra pendiente, el repartidor, el gestor
    // y el procesador. El `.parte` al final, con el fichero ya cerrado.
    escritor: Escritor,
    /// El ultimo fotograma convertido, **aun sin dar**: se da cuando llega
    /// el siguiente y ya se sabe cuanto dura (ver [`Grafica::escribir`]).
    /// Mientras se tiene, el repartidor no la recicla, y es de donde sale
    /// el fotograma repetido.
    pendiente: Option<Pendiente<Ranura>>,
    lote: Lote,
    gestor: IMFDXGIDeviceManager,
    /// El testigo del gestor, para darle otro dispositivo si se pierde.
    testigo: u32,
    conv: Conversor,
    parte: Parte,
    formato: Formato,
    medidas: (u32, u32),
    minimo: i64,
    escritos: u64,
}

impl Grafica {
    /// Monta el camino de la grafica. Si no puede, devuelve el procesador
    /// para el siguiente camino.
    pub fn abrir(
        disp: &Dispositivo,
        conv: Conversor,
        p: &Pedido,
    ) -> Result<Grafica, Box<(Conversor, ErrorMotor)>> {
        let parte = Parte::de(p.destino);
        match montar_grafica(disp, &parte.parcial, p) {
            Ok((gestor, testigo, lote, escritor)) => Ok(Grafica {
                escritor,
                pendiente: None,
                lote,
                gestor,
                testigo,
                conv,
                parte,
                formato: p.formato,
                medidas: p.medidas,
                minimo: p.minimo,
                escritos: 0,
            }),
            Err(e) => Err(Box::new((conv, e))),
        }
    }

    fn fotograma(
        &mut self,
        origen: &ID3D11Texture2D,
        contenido: (u32, u32),
        inicio: i64,
    ) -> Result<Escrito, ErrorMotor> {
        let Some(r) = self.lote.pedir()? else {
            return Ok(Escrito::Caido);
        };
        self.conv.convertir(origen, contenido, &r.textura, r.sub)?;
        self.escribir(r, inicio, false)
    }

    fn repetir(&mut self, inicio: i64) -> Result<Escrito, ErrorMotor> {
        let Some(r) = self.lote.pedir()? else {
            return Ok(Escrito::Caido);
        };
        // Una muestra no se puede dar dos veces (el repartidor la recicla
        // cuando el codificador la suelta): se copia en la GPU a otra.
        match &self.pendiente {
            Some(p) => self
                .conv
                .copiar_nv12(&p.dato.textura, p.dato.sub, &r.textura, r.sub),
            None => self.conv.negro(&r.textura, r.sub)?,
        }
        self.escribir(r, inicio, true)
    }

    /// **Con un fotograma de retraso**: el que esperaba se da ahora, con la
    /// duracion de verdad (hasta este), y este se queda esperando. Con ritmo
    /// variable un fotograma puede durar de 1/30 s a un segundo, y el MP4
    /// guarda la duracion de cada muestra: darla bien es lo que hace que el
    /// video no se adelante al sonido en ningun reproductor.
    ///
    /// Si lo que esperaba era un repetido y este llega pegado, este ocupa
    /// su sitio (`ritmo::ocupa_el_sitio_del_repetido`).
    fn escribir(&mut self, r: Ranura, inicio: i64, repetido: bool) -> Result<Escrito, ErrorMotor> {
        if let Some(p) = self.pendiente.take() {
            if !repetido && ocupa_el_sitio_del_repetido(p.repetido, p.inicio, inicio, self.minimo) {
                // El repetido vuelve al repartidor sin darse.
                self.pendiente = Some(Pendiente {
                    dato: r,
                    inicio: p.inicio,
                    repetido: false,
                });
                return Ok(Escrito::Hecho);
            }
            self.dar(&p.dato, p.inicio, inicio - p.inicio)?;
        }
        self.pendiente = Some(Pendiente {
            dato: r,
            inicio,
            repetido,
        });
        self.escritos += 1;
        Ok(Escrito::Hecho)
    }

    fn dar(&self, r: &Ranura, inicio: i64, duracion: i64) -> Result<(), ErrorMotor> {
        // SAFETY: muestra viva del repartidor.
        unsafe {
            r.muestra
                .SetSampleTime(inicio)
                .and_then(|()| r.muestra.SetSampleDuration(duracion.max(1)))
                .map_err(en("fechar un fotograma"))?;
        }
        self.escritor.video(&r.muestra)
    }

    /// Da el que esperaba, hasta `fin`.
    fn vaciar(&mut self, fin: i64, minima: i64) -> Result<(), ErrorMotor> {
        match self.pendiente.take() {
            Some(p) => self.dar(&p.dato, p.inicio, (fin - p.inicio).max(minima)),
            None => Ok(()),
        }
    }

    /// Lo que vive en la grafica, otra vez, sobre `disp`. El gestor es el
    /// mismo (el escritor y su codificador lo tienen): se le da el
    /// dispositivo nuevo con `ResetDevice`, que es como Media Foundation
    /// espera que se le cambie (el codificador ve
    /// `MF_E_DXGI_NEW_VIDEO_DEVICE` y vuelve a abrirlo).
    fn recrear(&mut self, disp: &Dispositivo) -> Result<(), ErrorMotor> {
        let perdido = self.pendiente.take().map(|p| p.inicio);
        let conv = self.conv.rehacer(disp.d3d(), disp.contexto())?;
        // SAFETY: dispositivo nuevo y vivo, con soporte de video; el
        // testigo es el que dio la creacion del gestor.
        unsafe { self.gestor.ResetDevice(disp.d3d(), self.testigo) }
            .map_err(en("darle el dispositivo nuevo al gestor"))?;
        self.lote = Lote::nuevo(&self.gestor, self.medidas)?;
        self.conv = conv;
        if let Some(t) = perdido
            && let Some(r) = self.lote.pedir()?
        {
            self.conv.negro(&r.textura, r.sub)?;
            self.pendiente = Some(Pendiente {
                dato: r,
                inicio: t,
                repetido: true,
            });
        }
        Ok(())
    }

    fn terminar(self) -> Result<Cerrado, ErrorMotor> {
        let Grafica {
            escritor,
            pendiente,
            lote,
            gestor,
            conv,
            parte,
            ..
        } = self;
        let fragmentado = escritor.fragmentado;
        let hecho = escritor.terminar();
        drop(pendiente);
        drop(lote);
        drop(gestor);
        drop(conv);
        parte.cerrar(hecho, fragmentado)
    }
}

fn montar_grafica(
    disp: &Dispositivo,
    parcial: &Path,
    p: &Pedido,
) -> Result<(IMFDXGIDeviceManager, u32, Lote, Escritor), ErrorMotor> {
    let mut testigo = 0u32;
    let mut gestor = None;
    // SAFETY: salidas locales; el gestor se queda con su referencia.
    unsafe { MFCreateDXGIDeviceManager(&mut testigo, &mut gestor) }
        .map_err(en("crear el gestor del dispositivo"))?;
    let gestor = gestor.ok_or(ErrorMotor::Sistema {
        paso: "crear el gestor del dispositivo",
        detalle: "sin gestor".into(),
        codigo: 0,
    })?;
    // SAFETY: dispositivo vivo, creado con soporte de video y protegido
    // para varios hilos (`motor`); el testigo es el que dio la creacion.
    unsafe { gestor.ResetDevice(disp.d3d(), testigo) }
        .map_err(en("darle el dispositivo al gestor"))?;
    let lote = Lote::nuevo(&gestor, p.medidas)?;
    let escritor = Escritor::abrir(
        parcial,
        &Plan {
            hardware: true,
            formato: p.formato,
            gestor: Some(&gestor),
            medidas: p.medidas,
            ajustes: p.ajustes,
            audio: p.audio,
            fragmentado: p.fragmentado,
        },
    )?;
    Ok((gestor, testigo, lote, escritor))
}

// --- Software (y la grafica desde memoria) -------------------------------

pub(crate) struct Software {
    escritor: Escritor,
    /// El ultimo NV12 bajado, aun sin dar (el mismo retraso de un fotograma
    /// que en [`Grafica::escribir`]). Repetirlo es otra muestra con el mismo
    /// buffer: el codificador solo lo lee. Esta en memoria: sobrevive a que
    /// se pierda la grafica.
    pendiente: Option<Pendiente<IMFMediaBuffer>>,
    conv: Conversor,
    destino_gpu: ID3D11Texture2D,
    escenario: ID3D11Texture2D,
    contexto: ID3D11DeviceContext,
    multi: Option<ID3D11Multithread>,
    parte: Parte,
    medidas: (u32, u32),
    minimo: i64,
    escritos: u64,
    /// El NV12 en memoria va al codificador de la grafica, no al de
    /// Microsoft (`Camino::GraficaEnMemoria`).
    con_grafica: bool,
}

impl Software {
    pub fn abrir(
        disp: &Dispositivo,
        conv: Conversor,
        p: &Pedido,
        con_grafica: bool,
    ) -> Result<Software, Box<(Conversor, ErrorMotor)>> {
        let parte = Parte::de(p.destino);
        let montado = (|| {
            let destino_gpu = textura_nv12(disp, p.medidas, false)?;
            let escenario = textura_nv12(disp, p.medidas, true)?;
            let escritor = Escritor::abrir(
                &parte.parcial,
                &Plan {
                    hardware: con_grafica,
                    formato: Formato::H264,
                    gestor: None,
                    medidas: p.medidas,
                    ajustes: p.ajustes,
                    audio: p.audio,
                    fragmentado: p.fragmentado,
                },
            )?;
            Ok::<_, ErrorMotor>((destino_gpu, escenario, escritor))
        })();
        match montado {
            Ok((destino_gpu, escenario, escritor)) => Ok(Software {
                escritor,
                pendiente: None,
                conv,
                destino_gpu,
                escenario,
                contexto: disp.contexto().clone(),
                multi: multihilo(disp.d3d()),
                parte,
                medidas: p.medidas,
                minimo: p.minimo,
                escritos: 0,
                con_grafica,
            }),
            Err(e) => Err(Box::new((conv, e))),
        }
    }

    fn fotograma(
        &mut self,
        origen: &ID3D11Texture2D,
        contenido: (u32, u32),
        inicio: i64,
    ) -> Result<Escrito, ErrorMotor> {
        if !admitir_en_software(self.escritos, self.escritor.en_cola()) {
            return Ok(Escrito::Caido);
        }
        let destino = self.destino_gpu.clone();
        self.conv.convertir(origen, contenido, &destino, 0)?;
        let b = self.bajar()?;
        self.escribir(b, inicio, false)
    }

    fn repetir(&mut self, inicio: i64) -> Result<Escrito, ErrorMotor> {
        if !admitir_en_software(self.escritos, self.escritor.en_cola()) {
            return Ok(Escrito::Caido);
        }
        let b = match &self.pendiente {
            Some(p) => p.dato.clone(),
            None => {
                let destino = self.destino_gpu.clone();
                self.conv.negro(&destino, 0)?;
                self.bajar()?
            }
        };
        self.escribir(b, inicio, true)
    }

    fn escribir(
        &mut self,
        b: IMFMediaBuffer,
        inicio: i64,
        repetido: bool,
    ) -> Result<Escrito, ErrorMotor> {
        if let Some(p) = self.pendiente.take() {
            if !repetido && ocupa_el_sitio_del_repetido(p.repetido, p.inicio, inicio, self.minimo) {
                self.pendiente = Some(Pendiente {
                    dato: b,
                    inicio: p.inicio,
                    repetido: false,
                });
                return Ok(Escrito::Hecho);
            }
            let m = muestra_de(&p.dato, p.inicio, (inicio - p.inicio).max(1))?;
            self.escritor.video(&m)?;
        }
        self.pendiente = Some(Pendiente {
            dato: b,
            inicio,
            repetido,
        });
        self.escritos += 1;
        Ok(Escrito::Hecho)
    }

    /// Da el que esperaba, hasta `fin`.
    fn vaciar(&mut self, fin: i64, minima: i64) -> Result<(), ErrorMotor> {
        match self.pendiente.take() {
            Some(p) => {
                let m = muestra_de(&p.dato, p.inicio, (fin - p.inicio).max(minima))?;
                self.escritor.video(&m)
            }
            None => Ok(()),
        }
    }

    /// Lo que vive en la grafica, otra vez, sobre `disp`. El escritor y el
    /// fotograma que espera estan en memoria y siguen.
    fn recrear(&mut self, disp: &Dispositivo) -> Result<(), ErrorMotor> {
        self.conv = self.conv.rehacer(disp.d3d(), disp.contexto())?;
        self.destino_gpu = textura_nv12(disp, self.medidas, false)?;
        self.escenario = textura_nv12(disp, self.medidas, true)?;
        self.contexto = disp.contexto().clone();
        self.multi = multihilo(disp.d3d());
        Ok(())
    }

    /// Baja el NV12 de la GPU a un buffer de Media Foundation, de arriba
    /// abajo y sin relleno de fila: el plano Y y detras el UV a media
    /// altura.
    fn bajar(&self) -> Result<IMFMediaBuffer, ErrorMotor> {
        let (w, h) = (self.medidas.0 as usize, self.medidas.1 as usize);
        let bytes = w * h * 3 / 2;
        // El buffer antes que el mapeo: entre `Map` y `Unmap` no puede haber
        // ningun `?`.
        let (buffer, destino) = buffer_de(bytes)?;
        let _gpu = EnLaGpu::entrar(self.multi.as_ref());
        let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: las dos texturas son NV12 del mismo tamano (`abrir`); la
        // de escenificacion tiene lectura de CPU.
        let mapeado = unsafe {
            self.contexto
                .CopyResource(&self.escenario, &self.destino_gpu);
            self.contexto
                .Map(&self.escenario, 0, D3D11_MAP_READ, 0, Some(&mut mapa))
        };
        if let Err(e) = mapeado {
            let _ = cerrar_buffer(&buffer, 0);
            return Err(en("bajar el NV12")(e));
        }
        let paso = mapa.RowPitch as usize;
        // SAFETY: mientras dure el mapeo `pData` tiene el plano Y (`h` filas
        // de `paso` bytes) y detras, en un NV12 de D3D11, el plano UV (`h/2`
        // filas del mismo paso). Se copian solo los `w` bytes utiles de
        // cada fila a `destino`, que tiene `w*h*3/2` bytes reservados.
        unsafe {
            let base = mapa.pData as *const u8;
            for fila in 0..h {
                std::ptr::copy_nonoverlapping(base.add(fila * paso), destino.add(fila * w), w);
            }
            let uv = base.add(paso * h);
            let destino_uv = destino.add(w * h);
            for fila in 0..h / 2 {
                std::ptr::copy_nonoverlapping(uv.add(fila * paso), destino_uv.add(fila * w), w);
            }
            self.contexto.Unmap(&self.escenario, 0);
        }
        cerrar_buffer(&buffer, bytes)?;
        Ok(buffer)
    }

    fn terminar(self) -> Result<Cerrado, ErrorMotor> {
        let Software {
            escritor,
            pendiente,
            conv,
            parte,
            ..
        } = self;
        let fragmentado = escritor.fragmentado;
        let hecho = escritor.terminar();
        drop(pendiente);
        drop(conv);
        parte.cerrar(hecho, fragmentado)
    }
}

/// Una textura NV12 del tamano del video: destino del procesador, o de
/// escenificacion para leerla desde la CPU.
fn textura_nv12(
    disp: &Dispositivo,
    (w, h): (u32, u32),
    para_leer: bool,
) -> Result<ID3D11Texture2D, ErrorMotor> {
    let d = D3D11_TEXTURE2D_DESC {
        Width: w,
        Height: h,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_NV12,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: if para_leer {
            D3D11_USAGE_STAGING
        } else {
            D3D11_USAGE_DEFAULT
        },
        BindFlags: if para_leer {
            0
        } else {
            D3D11_BIND_RENDER_TARGET.0 as u32
        },
        CPUAccessFlags: if para_leer {
            D3D11_CPU_ACCESS_READ.0 as u32
        } else {
            0
        },
        MiscFlags: 0,
    };
    let mut t = None;
    // SAFETY: descripcion completa con lados pares (NV12); `t` es local.
    unsafe { disp.d3d().CreateTexture2D(&d, None, Some(&mut t)) }
        .map_err(en("crear una textura NV12"))?;
    t.ok_or(ErrorMotor::Sistema {
        paso: "crear una textura NV12",
        detalle: "sin textura".into(),
        codigo: 0,
    })
}

// --- La CPU ---------------------------------------------------------------

pub(crate) struct Cpu {
    cod: pixpin_record::Codificador,
    bajada: pixpin_capture::Bajada,
    crudo: Vec<u8>,
    lienzo: Lienzo,
    hay_imagen: bool,
    /// El inicio del ultimo fotograma dado.
    ultimo_inicio: Option<i64>,
    canales: u16,
    parcial: PathBuf,
    medidas: (u32, u32),
    escritos: u64,
}

impl Cpu {
    pub fn abrir(p: &Pedido) -> Result<Cpu, ErrorMotor> {
        let (media, _) = bits_por_segundo(&p.ajustes, p.medidas.0, p.medidas.1);
        let cod = pixpin_record::Codificador::nuevo(
            p.destino,
            pixpin_record::OpcionesVivo {
                ancho: p.medidas.0,
                alto: p.medidas.1,
                por_segundo: p.ajustes.por_segundo,
                bitrate: Some(media),
                muestreo_audio: p.audio.map(|a| a.muestreo),
            },
        )
        .map_err(|e| ErrorMotor::SinCodificador(e.to_string()))?;
        Ok(Cpu {
            medidas: cod.medidas(),
            cod,
            bajada: pixpin_capture::Bajada::default(),
            crudo: Vec::new(),
            lienzo: Lienzo::default(),
            hay_imagen: false,
            ultimo_inicio: None,
            canales: p.audio.map_or(1, |a| a.canales),
            parcial: archivo::a_medias(p.destino),
            escritos: 0,
        })
    }

    fn fotograma(
        &mut self,
        disp: &Dispositivo,
        origen: &ID3D11Texture2D,
        contenido: (u32, u32),
        inicio: i64,
        duracion: i64,
    ) -> Result<Escrito, ErrorMotor> {
        let (w, h) = match self.bajada.bajar_bgra(disp, origen, &mut self.crudo) {
            Ok(medidas) => medidas,
            // Como antes de este motor: un fotograma que no baja se salta
            // (el anterior se queda un poco mas), no corta el video.
            Err(e) => {
                tracing::debug!(?e, "un fotograma no bajo");
                return Ok(Escrito::Caido);
            }
        };
        self.lienzo
            .pintar(&self.crudo, (w, h), contenido, self.medidas);
        self.hay_imagen = true;
        self.poner(inicio, duracion)
    }

    fn repetir(&mut self, inicio: i64, duracion: i64) -> Result<Escrito, ErrorMotor> {
        if !self.hay_imagen {
            self.lienzo.negro(self.medidas);
            self.hay_imagen = true;
        }
        self.poner(inicio, duracion)
    }

    fn poner(&mut self, inicio: i64, duracion: i64) -> Result<Escrito, ErrorMotor> {
        // `pixpin-record` salta el fotograma por su cuenta si su codificador
        // va atrasado (su propio tope).
        let entro = self
            .cod
            .poner_fotograma(&self.lienzo.pixeles, inicio, duracion)
            .map_err(|e| ErrorMotor::SinCodificador(e.to_string()))?;
        if entro {
            self.escritos += 1;
            self.ultimo_inicio = Some(inicio);
            Ok(Escrito::Hecho)
        } else {
            Ok(Escrito::Caido)
        }
    }

    /// Aqui no hay retraso de un fotograma (`pixpin-record` copia cada uno
    /// al darselo): para llegar a `fin` se repite el ultimo justo antes.
    fn vaciar(&mut self, fin: i64, duracion: i64) -> Result<(), ErrorMotor> {
        let t = fin - duracion;
        if self.ultimo_inicio.is_some_and(|u| t > u) {
            self.poner(t, duracion)?;
        }
        Ok(())
    }

    /// El codificador de `pixpin-record` es mono y sigue el orden de
    /// llegada: la posicion la lleva el.
    fn audio(&mut self, muestras: &[i16]) -> Result<(), ErrorMotor> {
        let mono = a_mono(muestras, self.canales);
        self.cod
            .poner_audio(&mono)
            .map_err(|e| ErrorMotor::SinCodificador(e.to_string()))
    }

    fn terminar(self) -> Result<PathBuf, ErrorMotor> {
        self.cod
            .terminar()
            .map_err(|e| ErrorMotor::SinCodificador(e.to_string()))
    }
}
