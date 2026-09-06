//! Arrastrar el contenido de un pin hacia OTRA aplicacion: arrastrar y
//! soltar saliente (https://pixpin.com/docs/pin/base-use).
//!
//! El gesto es `Ctrl` + boton izquierdo desde dentro del pin. Lo que viaja
//! depende del contenido: la imagen, el texto de la nota o la ruta del
//! fichero al que apunta el pin.
//!
//! La decision que hace que esto funcione de verdad esta en el objeto de
//! datos de una IMAGEN: ofrece **dos formatos a la vez**. `CF_DIB` para los
//! editores de imagen, Word o Paint, que quieren pixeles; y `CF_HDROP` con
//! un PNG temporal para el Explorador, los chats y el correo, que solo
//! saben recibir ficheros. Ofreciendo uno solo, la mitad de los destinos
//! rechazan el arrastre y el gesto parece roto sin dar un solo error.
//!
//! Y el PNG temporal se escribe **dentro de `GetData`**, o sea solo cuando
//! alguien pide `CF_HDROP`. Si el usuario suelta en un editor de imagen ese
//! fichero no llega a existir nunca: PixPin va a equipos con pocos
//! recursos y un arrastre no tiene por que costar un fichero en disco.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

use pixpin_codec::{ErrorCodec, ImagenRgba};
use windows::Win32::Foundation::{
    DATA_S_SAMEFORMATETC, DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS,
    DV_E_FORMATETC, E_NOTIMPL, E_OUTOFMEMORY, E_POINTER, GlobalFree, HGLOBAL,
    OLE_E_ADVISENOTSUPPORTED, S_OK,
};
use windows::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFOHEADER};
use windows::Win32::System::Com::{
    DATADIR_GET, DVASPECT_CONTENT, FORMATETC, IAdviseSink, IDataObject, IDataObject_Impl,
    IEnumFORMATETC, IEnumSTATDATA, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::System::Memory::{GHND, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::{
    CF_DIB, CF_HDROP, CF_UNICODETEXT, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, DoDragDrop,
    IDropSource, IDropSource_Impl, OleInitialize,
};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::SHCreateStdEnumFmtEtc;
use windows::core::{BOOL, HRESULT, Ref, implement};

use crate::contenido::Contenido;

/// Que se arrastra. Lo decide la ventana del pin segun su contenido.
pub enum Carga {
    Imagen(ImagenRgba),
    Texto(String),
    Fichero(PathBuf),
}

/// Como termino el gesto. No es un error que el usuario cambie de idea a
/// medio arrastre, asi que cancelar no viaja por `Err`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resultado {
    /// Un destino acepto la carga.
    Soltado,
    /// Se solto en el vacio o se pulso `Esc`.
    Cancelado,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorArrastre {
    #[error("la imagen del pin no se puede arrastrar: {0}")]
    Imagen(#[source] ErrorCodec),
    #[error("no se pudo montar la lista de ficheros: {0}")]
    Ficheros(#[source] ErrorCodec),
    #[error("no hay nada que arrastrar")]
    Vacia,
    #[error("no se pudo escribir el PNG temporal {ruta}: {fuente}")]
    Temporal {
        ruta: PathBuf,
        #[source]
        fuente: std::io::Error,
    },
    #[error("este hilo no pudo entrar en OLE: {0}")]
    Ole(#[source] windows::core::Error),
    #[error("el arrastre fallo: {0}")]
    Arrastre(#[source] windows::core::Error),
}

/// Que carga le corresponde a un contenido.
///
/// `ruta` es la del fichero al que apunta el pin. La ficha y el documento
/// NO la llevan dentro (`Contenido::Archivo` y `Contenido::Documento` solo
/// guardan lo que se pinta; la ruta la conoce el gestor), asi que llega por
/// fuera; el video si la trae. Sin ruta, un pin por referencia no se puede
/// arrastrar y se devuelve `None`: mejor que el gesto no haga nada a que
/// suelte un fichero equivocado.
pub fn carga_de(contenido: &Contenido, ruta: Option<&Path>) -> Option<Carga> {
    match contenido {
        // Se clona la imagen: el objeto de datos sobrevive a esta llamada y
        // no puede prestar nada de la ventana, porque durante el arrastre
        // Windows bombea mensajes y el WndProc vuelve a entrar. Es una copia
        // por gesto, no por fotograma.
        Contenido::Imagen(img) => Some(Carga::Imagen(img.clone())),
        Contenido::Nota { texto } => Some(Carga::Texto(texto.clone())),
        Contenido::Video { ruta: r, .. } => Some(Carga::Fichero(r.clone())),
        Contenido::Archivo { .. } | Contenido::Documento { .. } => {
            ruta.map(|r| Carga::Fichero(r.to_path_buf()))
        }
    }
}

/// Arranca el arrastre y no vuelve hasta que se suelta o se cancela.
///
/// `DoDragDrop` es modal por naturaleza: bombea el raton y el teclado el
/// solo. Por eso todo lo que puede fallar se comprueba ANTES de entrar; un
/// fallo aqui devuelve `Err` y el clic se queda en nada, que es lo unico
/// aceptable cuando quien llama es el bucle de mensajes.
pub fn arrastrar(carga: Carga) -> Result<Resultado, ErrorArrastre> {
    comprobar(&carga)?;
    asegurar_ole()?;

    let datos: IDataObject = ObjetoDatos {
        carga,
        png: RefCell::new(None),
    }
    .into();
    let origen: IDropSource = Origen.into();
    let mut efecto = DROPEFFECT_NONE;

    // SAFETY: los dos objetos son nuestros y viven hasta el final de la
    // funcion, que es lo unico que `DoDragDrop` necesita porque es
    // sincrona; `efecto` es una variable local escribible.
    let hr = unsafe { DoDragDrop(&datos, &origen, DROPEFFECT_COPY, &mut efecto) };

    if hr == DRAGDROP_S_DROP {
        // Un destino puede aceptar el gesto y no quedarse nada; entonces el
        // efecto vuelve en NONE y para el usuario no paso nada.
        if efecto == DROPEFFECT_NONE {
            Ok(Resultado::Cancelado)
        } else {
            Ok(Resultado::Soltado)
        }
    } else if hr == DRAGDROP_S_CANCEL {
        Ok(Resultado::Cancelado)
    } else {
        Err(ErrorArrastre::Arrastre(windows::core::Error::from(hr)))
    }
}

/// Rechaza lo que no se puede arrastrar antes de tocar OLE.
fn comprobar(carga: &Carga) -> Result<(), ErrorArrastre> {
    match carga {
        Carga::Imagen(img) => comprobar_imagen(img).map_err(ErrorArrastre::Imagen),
        // Arrastrar una nota vacia dejaria caer una cadena vacia en el
        // destino: un renglon en blanco que el usuario no pidio.
        Carga::Texto(t) if t.is_empty() => Err(ErrorArrastre::Vacia),
        Carga::Texto(_) => Ok(()),
        // `construir_hdrop` es quien sabe de rutas relativas y de NUL
        // dentro del nombre; se le pregunta ya, no cuando el raton este a
        // medio camino.
        Carga::Fichero(r) => pixpin_codec::construir_hdrop(std::slice::from_ref(r))
            .map(|_| ())
            .map_err(ErrorArrastre::Ficheros),
    }
}

/// Medidas y buffer coherentes, y ademas dentro de lo que un DIB sabe
/// decir.
fn comprobar_imagen(imagen: &ImagenRgba) -> Result<(), ErrorCodec> {
    if imagen.ancho == 0 || imagen.alto == 0 {
        return Err(ErrorCodec::Vacia {
            ancho: imagen.ancho,
            alto: imagen.alto,
        });
    }
    // `BITMAPINFOHEADER` guarda las medidas en `i32` y el tamano en `u32`.
    // Pasado ese limite el ancho se leeria negativo (que en un DIB no
    // significa nada) o el tamano daria la vuelta, y el destino leeria
    // memoria a lo loco. Se rechaza aqui, no alli.
    let espera = imagen.bytes_esperados();
    if imagen.ancho > i32::MAX as u32 || imagen.alto > i32::MAX as u32 || espera > u32::MAX as usize
    {
        return Err(ErrorCodec::TamanoIncoherente {
            ancho: imagen.ancho,
            alto: imagen.alto,
            tiene: imagen.pixeles.len(),
            espera,
        });
    }
    if imagen.pixeles.len() != espera {
        return Err(ErrorCodec::TamanoIncoherente {
            ancho: imagen.ancho,
            alto: imagen.alto,
            tiene: imagen.pixeles.len(),
            espera,
        });
    }
    Ok(())
}

/// Monta el bloque `CF_DIB`: la `BITMAPINFOHEADER` y detras los pixeles.
///
/// Tres decisiones, y las tres son la causa clasica de un pegado torcido:
///
/// 1. **Sin `BITMAPFILEHEADER`.** Esos 14 bytes solo existen en un `.bmp`
///    en disco. `CF_DIB` empieza en la cabecera de informacion; ponerlos
///    aqui correria todo 14 bytes y el destino leeria basura.
/// 2. **Filas de ABAJO ARRIBA, con `biHeight` positivo.** Se puede pedir el
///    orden natural con un alto negativo, pero hay aplicaciones que lo
///    ignoran y pegan la imagen del reves. Ademas es exactamente lo que ya
///    publica `pixpin_codec::copiar_imagen` en el portapapeles: asi las dos
///    rutas se rompen o funcionan juntas, nunca una si y otra no.
/// 3. **BGRA, no RGBA.** Hay que intercambiar rojo y azul. Es la misma
///    trampa que documenta `pixpin_record::mp4`: sin el intercambio, los
///    cielos salen naranjas y las caras azules, y no hay ningun error.
///
/// A 32 bits por pixel el paso de fila ya es multiplo de 4, asi que no hace
/// falta relleno: por eso no aparece por ningun lado.
fn construir_dib(imagen: &ImagenRgba) -> Result<Vec<u8>, ErrorArrastre> {
    comprobar_imagen(imagen).map_err(ErrorArrastre::Imagen)?;

    let espera = imagen.bytes_esperados();
    let cabecera = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: imagen.ancho as i32,
        biHeight: imagen.alto as i32,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        biSizeImage: espera as u32,
        ..Default::default()
    };

    let mut bloque = Vec::with_capacity(size_of::<BITMAPINFOHEADER>() + espera);
    // SAFETY: `BITMAPINFOHEADER` es un POD `repr(C)` con todos sus campos
    // inicializados, asi que sus `size_of` bytes son legibles y no hay
    // relleno sin inicializar. Solo se copian; el puntero no se guarda.
    bloque.extend_from_slice(unsafe {
        std::slice::from_raw_parts(
            &cabecera as *const BITMAPINFOHEADER as *const u8,
            size_of::<BITMAPINFOHEADER>(),
        )
    });

    let paso = imagen.ancho as usize * 4;
    for fila in (0..imagen.alto as usize).rev() {
        for pixel in imagen.pixeles[fila * paso..(fila + 1) * paso].chunks_exact(4) {
            bloque.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }

    Ok(bloque)
}

/// Texto para `CF_UNICODETEXT`: UTF-16 terminado en NUL.
fn construir_texto(texto: &str) -> Vec<u8> {
    let mut carga = Vec::with_capacity(texto.len() * 2 + 2);
    // Win32 espera UTF-16 en el orden nativo, que en toda maquina Windows
    // soportada es little endian.
    for unidad in texto.encode_utf16().chain(std::iter::once(0)) {
        carga.extend_from_slice(&unidad.to_le_bytes());
    }
    carga
}

/// Escribe el PNG que se ofrece como `CF_HDROP` y devuelve su ruta.
///
/// Va a `%TEMP%\PixPin`, no al escritorio ni a la carpeta de capturas: es un
/// fichero que el usuario no pidio guardar, solo el peaje de que el
/// Explorador y los chats no saben recibir un mapa de bits.
///
/// Y **no se borra** al terminar el arrastre, a proposito. El destino se
/// queda con la RUTA, no con el contenido: el Explorador copia enseguida,
/// pero un cliente de chat puede subir el fichero segundos despues.
/// Borrarlo al volver de `DoDragDrop` seria una carrera que de vez en
/// cuando pierde el fichero, y un adjunto vacio es peor que un temporal
/// olvidado. Windows limpia `%TEMP%`, y el nombre lleva fecha y hora para
/// que dos arrastres no se pisen y para que el usuario reconozca el fichero
/// alla donde caiga.
fn escribir_png_temporal(imagen: &ImagenRgba) -> Result<PathBuf, ErrorArrastre> {
    let bytes = pixpin_codec::codificar_png(imagen).map_err(ErrorArrastre::Imagen)?;

    let carpeta = std::env::temp_dir().join("PixPin");
    std::fs::create_dir_all(&carpeta).map_err(|fuente| ErrorArrastre::Temporal {
        ruta: carpeta.clone(),
        fuente,
    })?;

    // SAFETY: `GetLocalTime` solo escribe el `SYSTEMTIME` que se le pasa.
    let t = unsafe { GetLocalTime() };
    let base = format!(
        "PixPin {:04}-{:02}-{:02} {:02}-{:02}-{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    );

    // Dos arrastres dentro del mismo segundo compartirian nombre, y
    // reescribir el fichero le cambiaria el contenido a un envio que
    // todavia no ha salido. Se busca un hueco en vez de pisar.
    let mut ruta = carpeta.join(format!("{base}.png"));
    let mut intento = 2;
    while ruta.exists() && intento < 100 {
        ruta = carpeta.join(format!("{base} ({intento}).png"));
        intento += 1;
    }

    std::fs::write(&ruta, &bytes).map_err(|fuente| ErrorArrastre::Temporal {
        ruta: ruta.clone(),
        fuente,
    })?;
    Ok(ruta)
}

/// Copia los bytes a un bloque global movible, que es lo que viaja dentro
/// de un `STGMEDIUM` de tipo `TYMED_HGLOBAL`.
fn a_hglobal(bytes: &[u8]) -> windows::core::Result<HGLOBAL> {
    // SAFETY: se pide memoria movible del tamano exacto que se va a
    // escribir, y se escriben exactamente esos bytes. En el camino de exito
    // el bloque viaja en el `STGMEDIUM` y deja de ser nuestro; si el
    // bloqueo falla seguimos siendo duenos y hay que liberarlo aqui, porque
    // si no cada arrastre fallido deja memoria global perdida para todo el
    // proceso.
    unsafe {
        let bloque = GlobalAlloc(GHND, bytes.len())?;
        let destino = GlobalLock(bloque);
        if destino.is_null() {
            let _ = GlobalFree(Some(bloque));
            return Err(windows::core::Error::from(E_OUTOFMEMORY));
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), destino as *mut u8, bytes.len());
        let _ = GlobalUnlock(bloque);
        Ok(bloque)
    }
}

/// El `FORMATETC` con el que se ofrece (y se acepta) cada formato: siempre
/// el contenido entero, en un bloque de memoria global.
fn formatetc(formato: u16) -> FORMATETC {
    FORMATETC {
        cfFormat: formato,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

/// Los datos que se ofrecen al destino. Vive solo mientras dura el gesto:
/// `DoDragDrop` es la unica que tiene una referencia a el.
#[implement(IDataObject)]
struct ObjetoDatos {
    carga: Carga,
    /// El PNG temporal, escrito la PRIMERA vez que alguien pide `CF_HDROP`
    /// y no antes. Si el usuario suelta en Paint, este `Option` se queda en
    /// `None` y no se toca el disco: esa es toda la gracia de que `GetData`
    /// haga trabajo en vez de tenerlo ya hecho.
    png: RefCell<Option<PathBuf>>,
}

impl ObjetoDatos {
    /// Los formatos que se ofrecen, **en orden de preferencia**: el primero
    /// es el que se lleva un destino que acepte varios.
    ///
    /// Para una imagen va primero `CF_DIB`, que es lo que quiere un editor
    /// o un procesador de textos: incrusta la imagen en el documento. El
    /// Explorador y los chats ignoran el DIB y buscan `CF_HDROP`, asi que
    /// no pierden nada por ir segundos.
    fn formatos(&self) -> Vec<u16> {
        match self.carga {
            Carga::Imagen(_) => vec![CF_DIB.0, CF_HDROP.0],
            Carga::Texto(_) => vec![CF_UNICODETEXT.0],
            Carga::Fichero(_) => vec![CF_HDROP.0],
        }
    }

    /// La carga util de un formato concreto, montada ahora mismo.
    fn bytes_para(&self, formato: u16) -> windows::core::Result<Vec<u8>> {
        let fallo = |e: ErrorArrastre| {
            // Un fallo al montar los datos no puede tumbar el arrastre: se
            // contesta "ese formato no lo tengo" y el destino prueba otro o
            // rechaza el soltado, que es lo que el usuario ve como "aqui no
            // se puede".
            tracing::warn!(?e, formato, "no se pudo montar el formato de arrastre");
            windows::core::Error::from(DV_E_FORMATETC)
        };
        match (&self.carga, formato) {
            (Carga::Imagen(img), f) if f == CF_DIB.0 => construir_dib(img).map_err(fallo),
            (Carga::Imagen(img), f) if f == CF_HDROP.0 => {
                let mut png = self.png.borrow_mut();
                if png.is_none() {
                    *png = Some(escribir_png_temporal(img).map_err(fallo)?);
                }
                let ruta = png.as_ref().expect("se acaba de poner");
                pixpin_codec::construir_hdrop(std::slice::from_ref(ruta))
                    .map_err(|e| fallo(ErrorArrastre::Ficheros(e)))
            }
            (Carga::Texto(t), f) if f == CF_UNICODETEXT.0 => Ok(construir_texto(t)),
            (Carga::Fichero(r), f) if f == CF_HDROP.0 => {
                pixpin_codec::construir_hdrop(std::slice::from_ref(r))
                    .map_err(|e| fallo(ErrorArrastre::Ficheros(e)))
            }
            _ => Err(windows::core::Error::from(DV_E_FORMATETC)),
        }
    }

    /// Si se puede servir lo que pide este `FORMATETC`.
    ///
    /// # Safety
    ///
    /// `pedido` debe apuntar a un `FORMATETC` valido, o ser nulo.
    unsafe fn admite(&self, pedido: *const FORMATETC) -> bool {
        // SAFETY: lo garantiza quien llama; solo se lee, y solo aqui.
        let Some(p) = (unsafe { pedido.as_ref() }) else {
            return false;
        };
        // El aspecto se comprueba porque solo se enumera `DVASPECT_CONTENT`:
        // decir que si a una miniatura y luego servirle el contenido entero
        // seria mentirle al destino.
        p.dwAspect == DVASPECT_CONTENT.0
            && p.tymed & TYMED_HGLOBAL.0 as u32 != 0
            && self.formatos().contains(&p.cfFormat)
    }
}

impl IDataObject_Impl for ObjetoDatos_Impl {
    fn GetData(&self, pformatetcin: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
        if pformatetcin.is_null() {
            return Err(windows::core::Error::from(E_POINTER));
        }
        // SAFETY: OLE entrega un `FORMATETC` valido durante la llamada y se
        // acaba de comprobar que no es nulo; solo se lee.
        if !unsafe { self.admite(pformatetcin) } {
            return Err(windows::core::Error::from(DV_E_FORMATETC));
        }
        // SAFETY: mismo puntero, ya comprobado.
        let formato = unsafe { (*pformatetcin).cfFormat };

        let bytes = self.bytes_para(formato)?;
        let bloque = a_hglobal(&bytes)?;

        Ok(STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: bloque },
            // `None` no es descuido: es QUIEN LIBERA. Con `pUnkForRelease`
            // vacio, el bloque pasa a ser del destino, que lo suelta con
            // `ReleaseStgMedium` en cuanto termina de leerlo. Si aqui se
            // pusiera un objeto, habria que saber liberarlo nosotros; y si
            // se liberara el bloque al volver de `GetData`, el destino
            // leeria memoria muerta. La alternativa (guardarlo para
            // liberarlo despues) es la fuga silenciosa clasica: nadie ve
            // los megabytes que se quedan por el camino en cada arrastre.
            pUnkForRelease: std::mem::ManuallyDrop::new(None),
        })
    }

    fn GetDataHere(
        &self,
        _pformatetc: *const FORMATETC,
        _pmedium: *mut STGMEDIUM,
    ) -> windows::core::Result<()> {
        // Solo se sirve memoria propia, nunca un bloque que ponga el
        // destino: no hay forma de saber si le cabe.
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        // SAFETY: OLE entrega un `FORMATETC` valido durante la llamada, o
        // nulo; `admite` trata el nulo como un "no".
        if unsafe { self.admite(pformatetc) } {
            S_OK
        } else {
            DV_E_FORMATETC
        }
    }

    fn GetCanonicalFormatEtc(
        &self,
        _pformatectin: *const FORMATETC,
        pformatetcout: *mut FORMATETC,
    ) -> HRESULT {
        if !pformatetcout.is_null() {
            // SAFETY: OLE entrega un `FORMATETC` escribible y se acaba de
            // comprobar que no es nulo.
            //
            // Hay que dejarlo a cero aunque la respuesta sea "no hay forma
            // canonica": quien llama liberara `ptd`, y si se deja sin tocar
            // liberara lo que hubiera en esa memoria.
            unsafe { pformatetcout.write(FORMATETC::default()) };
        }
        DATA_S_SAMEFORMATETC
    }

    fn SetData(
        &self,
        _pformatetc: *const FORMATETC,
        _pmedium: *const STGMEDIUM,
        _frelease: BOOL,
    ) -> windows::core::Result<()> {
        // Este objeto es de salida: nadie le mete datos.
        Err(windows::core::Error::from(E_NOTIMPL))
    }

    fn EnumFormatEtc(&self, dwdirection: u32) -> windows::core::Result<IEnumFORMATETC> {
        if dwdirection != DATADIR_GET.0 as u32 {
            return Err(windows::core::Error::from(E_NOTIMPL));
        }
        let formatos: Vec<FORMATETC> = self.formatos().iter().map(|f| formatetc(*f)).collect();
        // SAFETY: la Shell COPIA la lista dentro del enumerador, asi que el
        // vector local puede morir al salir de aqui. Se usa el enumerador
        // estandar en vez de escribir un `IEnumFORMATETC` a mano porque
        // clonar y rebobinar un enumerador tiene mas esquinas que la lista
        // que enumera.
        unsafe { SHCreateStdEnumFmtEtc(&formatos) }
    }

    fn DAdvise(
        &self,
        _pformatetc: *const FORMATETC,
        _advf: u32,
        _padvsink: Ref<IAdviseSink>,
    ) -> windows::core::Result<u32> {
        // Los datos no cambian mientras dura el arrastre: no hay de que
        // avisar.
        Err(windows::core::Error::from(OLE_E_ADVISENOTSUPPORTED))
    }

    fn DUnadvise(&self, _dwconnection: u32) -> windows::core::Result<()> {
        Err(windows::core::Error::from(OLE_E_ADVISENOTSUPPORTED))
    }

    fn EnumDAdvise(&self) -> windows::core::Result<IEnumSTATDATA> {
        Err(windows::core::Error::from(OLE_E_ADVISENOTSUPPORTED))
    }
}

/// El lado del que arrastra: decide cuando termina el gesto.
#[implement(IDropSource)]
struct Origen;

impl IDropSource_Impl for Origen_Impl {
    fn QueryContinueDrag(&self, fescapepressed: BOOL, grfkeystate: MODIFIERKEYS_FLAGS) -> HRESULT {
        if fescapepressed.as_bool() {
            return DRAGDROP_S_CANCEL;
        }
        // Soltar el boton con el que se empezo es lo que cierra el gesto.
        // Se mira el boton y no `Ctrl`: el usuario suelta el modificador a
        // mitad de camino casi siempre, y cancelar ahi seria desconcertante.
        if grfkeystate.0 & MK_LBUTTON.0 == 0 {
            return DRAGDROP_S_DROP;
        }
        S_OK
    }

    fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> HRESULT {
        // Los cursores del sistema: son los que el usuario ya reconoce, y
        // ademas el destino puede afinarlos por su cuenta.
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

thread_local! {
    /// Si este hilo ya entro en OLE. Es por HILO, no por proceso: el
    /// apartamento lo es.
    static OLE_LISTO: Cell<bool> = const { Cell::new(false) };
}

/// Mete el hilo en OLE, que es lo que `DoDragDrop` necesita por encima de
/// COM a secas.
///
/// `OleInitialize` entra en un apartamento de hilo unico, exactamente el
/// mismo que ya pide el resto del pin (`video.rs` e `icono.rs` llaman a
/// `CoInitializeEx` con `COINIT_APARTMENTTHREADED`), asi que la segunda
/// llamada se limita a subir la cuenta. Lo que NO se puede hacer es
/// mezclarlo con `COINIT_MULTITHREADED`, que es lo que usan `mp4.rs` y
/// `uia.rs` en sus propios hilos: alli `OleInitialize` devolveria
/// `RPC_E_CHANGED_MODE` y el arrastre se rechaza con un error en vez de
/// dejar el hilo a medio inicializar.
///
/// No se llama nunca a `OleUninitialize`: el hilo de interfaz vive lo que
/// vive la aplicacion, y bajar la cuenta al salir de cada arrastre podria
/// cerrarle el COM a quien lo abrio antes. Es la misma decision, y por el
/// mismo motivo, que ya toma `video.rs`.
fn asegurar_ole() -> Result<(), ErrorArrastre> {
    OLE_LISTO.with(|listo| {
        if listo.get() {
            return Ok(());
        }
        // SAFETY: `OleInitialize` no tiene precondiciones. El parametro
        // reservado va en nulo, que es lo unico que admite.
        unsafe { OleInitialize(None) }.map_err(ErrorArrastre::Ole)?;
        listo.set(true);
        Ok(())
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Cuadrado 2x2 con un color por esquina, en RGBA:
    ///
    /// ```text
    /// rojo   verde
    /// azul   blanco
    /// ```
    fn cuadrado() -> ImagenRgba {
        ImagenRgba {
            ancho: 2,
            alto: 2,
            pixeles: vec![
                255, 0, 0, 255, // (0,0) rojo
                0, 255, 0, 255, // (1,0) verde
                0, 0, 255, 255, // (0,1) azul
                255, 255, 255, 255, // (1,1) blanco
            ],
        }
    }

    /// Los `size_of::<BITMAPINFOHEADER>()` primeros bytes leidos campo a
    /// campo, sin usar la propia estructura: si alguien cambiara el orden
    /// de los campos, la prueba tiene que enterarse.
    fn campo_u32(dib: &[u8], desplazamiento: usize) -> u32 {
        u32::from_le_bytes(dib[desplazamiento..desplazamiento + 4].try_into().unwrap())
    }
    fn campo_i32(dib: &[u8], desplazamiento: usize) -> i32 {
        i32::from_le_bytes(dib[desplazamiento..desplazamiento + 4].try_into().unwrap())
    }
    fn campo_u16(dib: &[u8], desplazamiento: usize) -> u16 {
        u16::from_le_bytes(dib[desplazamiento..desplazamiento + 2].try_into().unwrap())
    }

    #[test]
    fn el_dib_empieza_en_la_cabecera_de_informacion_y_no_en_la_de_fichero() {
        let dib = construir_dib(&cuadrado()).expect("un 2x2 deberia valer");

        // Si alguien colara los 14 bytes de `BITMAPFILEHEADER`, el primer
        // campo dejaria de ser 40 y ademas el bloque mediria 14 de mas.
        assert_eq!(campo_u32(&dib, 0), 40, "biSize debe abrir el bloque");
        assert_eq!(
            dib.len(),
            40 + 2 * 2 * 4,
            "cabecera de informacion y pixeles, nada mas"
        );
        assert_ne!(
            &dib[0..2],
            b"BM",
            "un CF_DIB con la firma BM es un fichero .bmp mal colocado"
        );
    }

    #[test]
    fn la_cabecera_declara_un_bmp_de_32_bits_sin_comprimir() {
        let dib = construir_dib(&cuadrado()).unwrap();
        assert_eq!(campo_i32(&dib, 4), 2, "biWidth");
        assert_eq!(campo_i32(&dib, 8), 2, "biHeight");
        assert_eq!(campo_u16(&dib, 12), 1, "biPlanes");
        assert_eq!(campo_u16(&dib, 14), 32, "biBitCount");
        assert_eq!(campo_u32(&dib, 16), BI_RGB.0, "biCompression");
        assert_eq!(campo_u32(&dib, 20), 16, "biSizeImage");
    }

    #[test]
    fn el_alto_del_dib_es_positivo_porque_las_filas_van_de_abajo_arriba() {
        let img = ImagenRgba {
            ancho: 1,
            alto: 3,
            pixeles: vec![
                1, 1, 1, 255, // fila de arriba
                2, 2, 2, 255, 3, 3, 3, 255, // fila de abajo
            ],
        };
        let dib = construir_dib(&img).unwrap();

        // Un alto negativo significaria filas en orden natural. Se elige el
        // positivo, asi que la PRIMERA fila del bloque tiene que ser la
        // ULTIMA de la imagen; si alguien cambia una cosa sin la otra, todo
        // el mundo pegaria la imagen del reves.
        assert_eq!(campo_i32(&dib, 8), 3, "biHeight positivo");
        assert_eq!(dib[40], 3, "abajo del todo va primero");
        assert_eq!(dib[44], 2);
        assert_eq!(dib[48], 1, "arriba del todo va al final");
    }

    #[test]
    fn los_pixeles_del_dib_van_en_bgra_con_el_rojo_y_el_azul_cambiados() {
        let dib = construir_dib(&cuadrado()).unwrap();
        let p = &dib[40..];

        // Primera fila del bloque = ultima de la imagen: azul y blanco.
        assert_eq!(&p[0..4], &[255, 0, 0, 255], "el azul RGBA sale B=255");
        assert_eq!(&p[4..8], &[255, 255, 255, 255], "el blanco no cambia");
        // Segunda fila del bloque = primera de la imagen: rojo y verde.
        assert_eq!(
            &p[8..12],
            &[0, 0, 255, 255],
            "el rojo RGBA sale R en el 3.º"
        );
        assert_eq!(&p[12..16], &[0, 255, 0, 255], "el verde se queda en medio");
    }

    #[test]
    fn una_imagen_vacia_no_se_puede_arrastrar() {
        // Caso negativo: sin esto se publicaria una cabecera con biWidth 0 y
        // el destino reservaria un mapa de bits de nada.
        let vacia = ImagenRgba {
            ancho: 0,
            alto: 0,
            pixeles: vec![],
        };
        assert!(construir_dib(&vacia).is_err());
        assert!(comprobar(&Carga::Imagen(vacia)).is_err());
    }

    #[test]
    fn una_imagen_con_el_buffer_incoherente_no_se_puede_arrastrar() {
        // Caso negativo: construir el DIB leeria fuera del vector.
        let mala = ImagenRgba {
            ancho: 4,
            alto: 4,
            pixeles: vec![0; 3],
        };
        assert!(construir_dib(&mala).is_err());
    }

    #[test]
    fn unas_dimensiones_imposibles_no_llegan_a_la_cabecera() {
        // Caso negativo: `biWidth` es `i32`. Un ancho por encima de
        // `i32::MAX` se leeria negativo en el destino, que en un DIB no
        // significa nada, y `biSizeImage` daria la vuelta.
        let enorme = ImagenRgba {
            ancho: u32::MAX,
            alto: 2,
            pixeles: vec![],
        };
        assert!(construir_dib(&enorme).is_err());

        let alto_imposible = ImagenRgba {
            ancho: 2,
            alto: u32::MAX,
            pixeles: vec![],
        };
        assert!(construir_dib(&alto_imposible).is_err());
    }

    #[test]
    fn el_texto_viaja_en_utf16_terminado_en_nul() {
        let carga = construir_texto("añ");
        let unidades: Vec<u16> = carga
            .chunks_exact(2)
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect();
        assert_eq!(unidades, vec![0x0061, 0x00F1, 0]);
    }

    #[test]
    fn una_nota_vacia_no_se_arrastra() {
        // Caso negativo: soltaria un renglon en blanco en el destino.
        assert!(comprobar(&Carga::Texto(String::new())).is_err());
        assert!(comprobar(&Carga::Texto("hola".into())).is_ok());
    }

    #[test]
    fn un_fichero_de_ruta_relativa_no_se_arrastra() {
        // Caso negativo, y se caza ANTES de entrar en el arrastre modal: el
        // destino resolveria la ruta contra SU directorio de trabajo y
        // recibiria un fichero que no existe.
        assert!(comprobar(&Carga::Fichero(PathBuf::from("capturas/a.png"))).is_err());
        assert!(comprobar(&Carga::Fichero(PathBuf::from(r"C:\tmp\a.png"))).is_ok());
    }

    #[test]
    fn una_imagen_ofrece_mapa_de_bits_y_fichero_en_ese_orden() {
        // La decision del modulo: sin los dos formatos, la mitad de los
        // destinos rechaza el arrastre. Y el DIB va primero para que Word
        // incruste la imagen en vez de adjuntar un PNG.
        let o = ObjetoDatos {
            carga: Carga::Imagen(cuadrado()),
            png: RefCell::new(None),
        };
        assert_eq!(o.formatos(), vec![CF_DIB.0, CF_HDROP.0]);
        assert_eq!(
            ObjetoDatos {
                carga: Carga::Texto("x".into()),
                png: RefCell::new(None),
            }
            .formatos(),
            vec![CF_UNICODETEXT.0]
        );
        assert_eq!(
            ObjetoDatos {
                carga: Carga::Fichero(PathBuf::from(r"C:\tmp\a.png")),
                png: RefCell::new(None),
            }
            .formatos(),
            vec![CF_HDROP.0]
        );
    }

    #[test]
    fn pedir_el_mapa_de_bits_no_escribe_ningun_png() {
        // El nucleo de la decision de recursos: quien suelta en un editor
        // de imagen no deja un fichero en el disco. Si alguien adelantara
        // la escritura del PNG a la construccion del objeto, esta prueba lo
        // caza sin necesidad de mirar `%TEMP%`.
        let o = ObjetoDatos {
            carga: Carga::Imagen(cuadrado()),
            png: RefCell::new(None),
        };
        let _ = o.bytes_para(CF_DIB.0).expect("el DIB deberia montarse");
        assert!(
            o.png.borrow().is_none(),
            "servir CF_DIB no puede tocar el disco"
        );
    }

    #[test]
    fn una_nota_no_ofrece_ni_mapa_de_bits_ni_ficheros() {
        // Caso negativo del reparto de formatos: si una nota contestara a
        // CF_DIB, el destino recibiria un mapa de bits vacio.
        let o = ObjetoDatos {
            carga: Carga::Texto("hola".into()),
            png: RefCell::new(None),
        };
        assert!(o.bytes_para(CF_DIB.0).is_err());
        assert!(o.bytes_para(CF_HDROP.0).is_err());
        assert!(o.bytes_para(CF_UNICODETEXT.0).is_ok());
    }

    #[test]
    fn una_ficha_sin_ruta_no_se_arrastra() {
        // Caso negativo: `Contenido::Archivo` no lleva la ruta dentro. Sin
        // ella el gesto no hace nada, que es mejor que soltar un fichero
        // inventado.
        let ficha = Contenido::Archivo {
            nombre: "informe.pdf".into(),
            detalle: "1,2 MB".into(),
            icono: None,
            existe: true,
        };
        assert!(carga_de(&ficha, None).is_none());
        assert!(matches!(
            carga_de(&ficha, Some(Path::new(r"C:\tmp\informe.pdf"))),
            Some(Carga::Fichero(_))
        ));
    }

    #[test]
    fn un_video_arrastra_su_propia_ruta_sin_que_nadie_se_la_diga() {
        let video = Contenido::Video {
            nombre: "v.mp4".into(),
            ruta: PathBuf::from(r"C:\v\v.mp4"),
            ancho: 1920,
            alto: 1080,
        };
        match carga_de(&video, None) {
            Some(Carga::Fichero(r)) => assert_eq!(r, PathBuf::from(r"C:\v\v.mp4")),
            _ => panic!("un video se arrastra como fichero"),
        }
    }

    #[test]
    #[ignore = "escribe un .bmp en %TEMP% para abrirlo con un visor de fuera; ejecutar con --ignored"]
    fn el_dib_de_una_imagen_se_puede_abrir_como_bmp() {
        // El DIB es memoria, no fichero: para mirarlo con un visor hay que
        // ponerle delante los 14 bytes de `BITMAPFILEHEADER` que `CF_DIB`
        // NO lleva. Abrir el resultado es la unica comprobacion que ve a la
        // vez el orden de las filas y el intercambio de canales con los
        // ojos de otro programa, no con los nuestros.
        //
        // La imagen va 3x2 y no cuadrada a proposito: con un cuadrado, un
        // ancho y un alto intercambiados en la cabecera no se notarian.
        //
        //     rojo    verde   azul
        //     negro   blanco  gris
        let img = ImagenRgba {
            ancho: 3,
            alto: 2,
            pixeles: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, //
                0, 0, 0, 255, 255, 255, 255, 255, 128, 128, 128, 255,
            ],
        };
        let dib = construir_dib(&img).unwrap();
        let inicio: u32 = 14 + 40;
        let total: u32 = 14 + dib.len() as u32;
        let mut bmp = Vec::with_capacity(total as usize);
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&total.to_le_bytes());
        bmp.extend_from_slice(&0u16.to_le_bytes());
        bmp.extend_from_slice(&0u16.to_le_bytes());
        bmp.extend_from_slice(&inicio.to_le_bytes());
        bmp.extend_from_slice(&dib);

        let ruta = std::env::temp_dir().join("pixpin-prueba-dib.bmp");
        std::fs::write(&ruta, &bmp).expect("deberia poder escribirse");
        println!("BMP de comprobacion en {}", ruta.display());
    }

    #[test]
    #[ignore = "abre un arrastre modal que se queda esperando al raton; necesita escritorio"]
    fn arrastrar_una_imagen_devuelve_cancelado_si_se_suelta_en_el_vacio() {
        let r = arrastrar(Carga::Imagen(cuadrado())).expect("no deberia fallar al montar");
        assert_eq!(r, Resultado::Cancelado);
    }
}
