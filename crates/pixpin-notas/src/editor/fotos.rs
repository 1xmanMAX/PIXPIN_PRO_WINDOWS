//! **Las fotos de la nota, a mano**: pegarlas (una captura, fotos copiadas
//! en el Explorador), soltarlas encima, cambiarles el ancho con su asa o
//! con el menu, verlas en grande, quitarlas; y las **hojas de los
//! proyectos**: meterlas como pagina viva o como enlace, abrirlas, y
//! volver a leer su imagen cuando la aplicacion la repinta.
//!
//! Lo que es de la nota (que renglon, que ancho) se cuenta con
//! `pixpin_docs::md_imagen`; lo que es de PixPin (que hojas hay, pintarlas,
//! abrirlas) lo pone la aplicacion en `integracion::Integracion`.

use std::path::PathBuf;

use pixpin_docs::md_imagen::{self, Foto};
use windows::Win32::UI::Controls::NMHDR;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::Shell::{DragAcceptFiles, HDROP};

use super::*;
use crate::integracion::{self, Viva};

/// El temporizador de las paginas vivas y cada cuanto: mirar si una hoja
/// cambio es comparar unas fechas de fichero, y pintarla lo hace la
/// aplicacion en otro hilo.
pub(super) const T_VIVAS: usize = 3;
const MS_VIVAS: u32 = 2_000;
/// El toque de la aplicacion: «mira las paginas vivas y lo mandado ya».
pub(super) const WM_VIVAS: u32 = integracion::WM_VIVAS;

/// Los anchos del menu, en pixeles a 96 ppp (la columna mide 720).
const PEQUENA: u32 = 240;
const MEDIANA: u32 = 360;
const GRANDE: u32 = 540;

thread_local! {
    /// Ficheros soltados encima, y donde (la letra), hasta que el bucle los
    /// atienda.
    static SOLTADOS: RefCell<Vec<(Vec<PathBuf>, Option<usize>)>> = const { RefCell::new(Vec::new()) };
    /// El ancho (a 96 ppp) que se escribira al soltar el asa: el ultimo que
    /// se enseno, no el del punto donde se suelta.
    static ANCHO: Cell<Option<u32>> = const { Cell::new(None) };
}

/// Lo que medira en pantalla la foto `i` de `imagenes::PUESTAS` con el
/// ancho `ancho96` puesto: las mismas cuentas que al leerla del fichero
/// (`caja_maxima` y `encajar_con`), asi el arrastre ensena lo que quedara.
fn tamano_con_ancho(e: &Estado, i: usize, ancho96: u32) -> Option<(i32, i32)> {
    let original = imagenes::PUESTAS.with(|p| p.borrow().get(i).map(|pu| pu.foto.original))?;
    let max = imagenes::caja_maxima(Some(ancho96), columna_px(e), e.ppp as f32 / 96.0);
    let (an, al) = imagenes::encajar_con(original.0, original.1, max, true);
    (an > 0 && al > 0).then_some((an, al))
}

// ---------------------------------------------------------------------------
// Medir y colocar

/// Las fotos que se pueden ensenar (renglon, ruta, foto ya leida a su
/// tamano) y los renglones de foto que no.
pub(super) type ConFoto = Vec<(usize, String, Rc<imagenes::Foto>)>;

/// La columna, o lo que de la ventana si es mas estrecha (a 96 ppp): una
/// foto no puede salirse del papel.
pub(super) fn columna_px(e: &Estado) -> i32 {
    let mut dentro = RECT::default();
    enviar(e.edit, 0x00B2, 0, &mut dentro as *mut _ as isize);
    // Sin el hueco de las tablas anchas, que no se ve (`tabla_ancha`).
    let visible = (dentro.right - dentro.left - crate::tabla_ancha::hueco()) * 96 / e.ppp.max(1);
    if visible > 0 { tabla_rtf::COLUMNA_PX.min(visible) } else { tabla_rtf::COLUMNA_PX }
}

/// El aire de encima del renglon `linea`, para una foto de `alto` pixeles
/// de pantalla: el formato del parrafo, sin tocar el texto ni el deshacer.
pub(super) fn hueco_de_foto(e: &Estado, linea: usize, alto: i32) {
    let texto = leer(e.edit);
    let Some(l) = md_vivo::lineas(&texto).get(linea).copied() else {
        return;
    };
    let aire = imagenes::AIRE_PX * e.ppp / 96;
    let mut p = parrafo_base(&e.estilos);
    p.Base.dwMask = PFM_SPACEBEFORE;
    p.dySpaceBefore = (alto + 2 * aire) * 1440 / e.ppp;
    let mascara = enviar(e.edit, EM_GETEVENTMASK, 0, 0);
    enviar(e.edit, EM_SETEVENTMASK, 0, ENM_NONE as isize);
    // SAFETY: el documento es el del propio control, vivo mientras el.
    if let Some(d) = &e.doc {
        unsafe {
            let _ = d.Undo(tomSuspend.0);
        }
    }
    let sel = seleccion(e.edit);
    elegir(e.edit, l.desde, l.hasta);
    poner_parrafo(e.edit, &p);
    elegir(e.edit, sel.0, sel.1);
    if let Some(d) = &e.doc {
        // SAFETY: como arriba.
        unsafe {
            let _ = d.Undo(tomResume.0);
        }
    }
    enviar(e.edit, EM_SETEVENTMASK, 0, mascara);
}

pub(super) fn medir(e: &mut Estado, texto: &str) -> (ConFoto, Vec<usize>) {
    let esc = e.ppp as f32 / 96.0;
    let columna = columna_px(e);
    let (mut con, mut sin) = (Vec::new(), Vec::new());
    for (n, f) in md_imagen::fotos(texto) {
        let max = imagenes::caja_maxima(f.ancho, columna, esc);
        match e.fotos.foto(&f.ruta, &*e.resolver, max, f.ancho.is_some()) {
            Some(x) => con.push((n, f.ruta, x)),
            None => sin.push(n),
        }
    }
    // Los documentos, mensajes y audios se ponen encima igual (`incrustados`).
    super::incrustados::medir(e, texto, &mut con);
    (con, sin)
}

/// Deja las fotos listas para pintarse encima. `todo`: tras un cambio del
/// texto (si no, solo se movio el cursor y basta con decir cual tiene el
/// borde).
pub(super) fn poner_puestas(e: &Estado, ls: &[md_vivo::Linea], con: &ConFoto, activa: usize, todo: bool) {
    let t = &e.estilos.tema;
    imagenes::COLORES.with(|c| {
        c.set(imagenes::Colores {
            acento: color(t.acento),
            aviso_fondo: color(t.boton_fondo),
            aviso_texto: color(t.boton_texto),
        })
    });
    imagenes::PUESTAS.with(|p| {
        let mut p = p.borrow_mut();
        if !todo {
            for pu in p.iter_mut() {
                pu.activa = pu.linea == activa;
            }
            return;
        }
        *p = con
            .iter()
            .filter_map(|(n, ruta, f)| {
                ls.get(*n).map(|l| imagenes::Puesta {
                    pos: l.desde,
                    linea: *n,
                    ruta: ruta.clone(),
                    foto: f.clone(),
                    activa: *n == activa,
                    aviso: e.sin_hoja.contains(ruta).then(|| e.rotulos.fotos.hoja_borrada.clone()),
                    caja: Cell::new(RECT::default()),
                })
            })
            .collect();
    });
}

// ---------------------------------------------------------------------------
// Meter fotos

/// Un aviso al usuario; en las pruebas (ventana oculta) solo al registro,
/// que un cuadro de dialogo las pararia.
fn avisar(e: &Estado, texto: &str) {
    if e.oculto {
        tracing::info!(texto, "aviso de la nota");
    } else {
        pixpin_shell::exportar::informar(e.marco, &e.rotulos.sufijo, texto);
    }
}

fn nombre_de(r: &std::path::Path) -> String {
    r.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// Copia las fotos junto a la nota y mete un renglon por cada una en el
/// sitio del cursor. Lo que no es una foto se deja. Devuelve cuantas entraron.
pub(super) fn meter(e: &mut Estado, rutas: &[PathBuf]) -> usize {
    let (mut hechas, mut fallo) = (0, false);
    for r in rutas.iter().filter(|r| md_imagen::es_foto(&nombre_de(r))) {
        match (e.adjuntar)(r) {
            Some(md) => {
                let alt = r.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let renglon = md_imagen::escribir(&Foto {
                    alt,
                    ancho: None,
                    ruta: md,
                });
                // El renglon nace escondido y con su hueco: congelado, se
                // pinta la foto en el mismo fotograma que entra (ver `congelar`).
                congelar::congelado(e, congelar::Pintado::Entero, |e| insertar_renglon(e, &renglon));
                hechas += 1;
            }
            None => fallo = true,
        }
    }
    if fallo {
        let t = e.rotulos.imagen_no_copiada.clone();
        avisar(e, &t);
    }
    hechas
}

/// Si en el portapapeles hay texto: entonces se pega texto aunque traiga
/// tambien una imagen (Word y los navegadores copian el texto con un
/// dibujo de el, y pegar ese dibujo en vez del texto seria un susto).
fn hay_texto_en_el_portapapeles() -> bool {
    // SAFETY: consulta sin abrir el portapapeles.
    unsafe { windows::Win32::System::DataExchange::IsClipboardFormatAvailable(CF_UNICODETEXT as u32).is_ok() }
}

/// Ctrl+V con una imagen (una captura de PixPin, una de Windows) o con
/// fotos copiadas en el Explorador: entran como fotos de la nota. `false`
/// si lo que hay es otra cosa, que se pega como texto.
pub(super) fn pegar(e: &mut Estado) -> bool {
    use pixpin_codec::portapapeles::ContenidoPortapapeles as C;
    match pixpin_codec::portapapeles::leer() {
        Some(C::Rutas(v)) => {
            // Con aplicacion detras, los documentos y audios copiados en el
            // Explorador tambien entran (`incrustados`); sin ella, solo fotos.
            if e.integracion.medios.is_some() {
                return super::incrustados::meter_documentos(e, &v) > 0;
            }
            let fotos: Vec<PathBuf> = v.into_iter().filter(|r| md_imagen::es_foto(&nombre_de(r))).collect();
            !fotos.is_empty() && meter(e, &fotos) > 0
        }
        Some(C::Imagen(img)) if !hay_texto_en_el_portapapeles() => pegar_imagen(e, &img),
        _ => false,
    }
}

/// Mete una imagen (pegada) como foto: se escribe en un PNG temporal y
/// entra como cualquier otra, copiada junto a la nota.
pub(super) fn pegar_imagen(e: &mut Estado, img: &pixpin_codec::ImagenRgba) -> bool {
    let nombre = match e.rotulos.fotos.captura.trim() {
        "" => "captura".to_string(),
        n => n.to_string(),
    };
    let Some(r) = integracion::png_temporal(img, &nombre) else {
        return false;
    };
    let hechas = meter(e, std::slice::from_ref(&r));
    if let Some(d) = r.parent() {
        let _ = std::fs::remove_dir_all(d);
    }
    hechas > 0
}

/// Mete lo soltado encima, en la letra donde se solto si se sabe.
pub(super) fn soltar(e: &mut Estado, rutas: Vec<PathBuf>, donde: Option<usize>) -> usize {
    if rutas.is_empty() {
        return 0;
    }
    if e.integracion.medios.is_some() {
        // Documentos y audios tambien, como su tarjeta (`incrustados`).
        if let Some(p) = donde {
            elegir(e.edit, p, p);
        }
        return super::incrustados::meter_documentos(e, &rutas);
    }
    if !rutas.iter().any(|r| md_imagen::es_foto(&nombre_de(r))) {
        let t = e.rotulos.fotos.solo_imagenes.clone();
        avisar(e, &t);
        return 0;
    }
    if let Some(p) = donde {
        elegir(e.edit, p, p);
    }
    meter(e, &rutas)
}

pub(super) fn soltar_lo_soltado(e: &mut Estado) {
    let todo: Vec<_> = SOLTADOS.with(|s| std::mem::take(&mut *s.borrow_mut()));
    for (rutas, donde) in todo {
        soltar(e, rutas, donde);
    }
}

/// Un `WM_DROPFILES` que llego al propio control.
pub(super) fn soltar_hdrop(e: &mut Estado, w: usize, liberar: bool) {
    let rutas = integracion::rutas_de_hdrop(HDROP(w as *mut _), liberar);
    soltar(e, rutas, None);
}

/// Lo que el marco recibe para las fotos: el `EN_DROPFILES` del control,
/// un `WM_DROPFILES` en la cabecera o la barra, y el toque de la
/// aplicacion. Se apunta y lo atiende el bucle.
pub(super) fn mensaje_del_marco(m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_NOTIFY => {
            let cab = l.0 as *const NMHDR;
            // SAFETY: Windows pasa en WM_NOTIFY un NMHDR valido; con
            // EN_DROPFILES es la cabecera de un ENDROPFILES.
            unsafe {
                if !cab.is_null() && (*cab).code == EN_DROPFILES {
                    let d = &*(l.0 as *const ENDROPFILES);
                    // El arrastre es del control: se lee y no se suelta.
                    let rutas = integracion::rutas_de_hdrop(HDROP(d.hDrop.0), false);
                    SOLTADOS.with(|s| s.borrow_mut().push((rutas, Some(d.cp.max(0) as usize))));
                    apuntar(Orden::Soltar);
                }
            }
            // Cero: el control no mete nada por su cuenta.
            LRESULT(0)
        }
        WM_DROPFILES => {
            let rutas = integracion::rutas_de_hdrop(HDROP(w.0 as *mut _), true);
            SOLTADOS.with(|s| s.borrow_mut().push((rutas, None)));
            apuntar(Orden::Soltar);
            LRESULT(0)
        }
        _ => {
            apuntar(Orden::Vivas);
            LRESULT(0)
        }
    }
}

/// Que el control y el marco acepten ficheros soltados.
pub(super) fn preparar(e: &Estado) {
    // SAFETY: ventanas propias.
    unsafe {
        DragAcceptFiles(e.edit, true);
        DragAcceptFiles(e.marco, true);
    }
}

// ---------------------------------------------------------------------------
// Las hojas de los proyectos

/// Donde sale el selector: bajo el cursor de escribir.
fn punto_del_cursor(e: &Estado) -> POINT {
    let (_, b) = seleccion(e.edit);
    let mut p = POINT::default();
    enviar(e.edit, EM_POSFROMCHAR, &mut p as *mut _ as usize, b as isize);
    p.y += RENGLON_PX * e.ppp / 96;
    // SAFETY: conversion de coordenadas de una ventana propia.
    unsafe {
        let _ = ClientToScreen(e.edit, &mut p);
    }
    p
}

/// «Pagina de un proyecto» y «Enlace a una hoja»: el selector de hojas y,
/// con la elegida, su renglon.
pub(super) fn hoja_de_un_proyecto(e: &mut Estado, como_enlace: bool) {
    let Some(hojas) = e.integracion.hojas.as_mut() else {
        return;
    };
    let grupos = hojas();
    if integracion::claves_del_selector(&grupos).is_empty() {
        let t = e.rotulos.fotos.sin_hojas.clone();
        avisar(e, &t);
        return;
    }
    if e.oculto {
        return;
    }
    let punto = punto_del_cursor(e);
    if let Some(clave) = integracion::elegir_hoja(e.marco, &grupos, &e.rotulos.fotos, punto) {
        meter_hoja(e, &clave, como_enlace);
    }
}

/// Mete la hoja `clave`: su pagina viva en un renglon propio, o el enlace
/// en el sitio del cursor (con lo elegido como texto, si hay algo elegido).
pub(super) fn meter_hoja(e: &mut Estado, clave: &str, como_enlace: bool) -> bool {
    let Some(f) = e.integracion.insertar_hoja.as_mut() else {
        return false;
    };
    let Some(renglon) = f(clave, como_enlace) else {
        return false;
    };
    if como_enlace {
        let (a, b) = seleccion(e.edit);
        let texto = leer(e.edit);
        let u: Vec<u16> = texto.encode_utf16().collect();
        let elegido = String::from_utf16_lossy(&u[a.min(u.len())..b.min(u.len())]);
        let puesto = match renglon.split_once("](") {
            Some((_, resto)) if !elegido.trim().is_empty() && !elegido.contains('\n') => {
                format!("[{}]({resto}", elegido.replace(['[', ']'], ""))
            }
            _ => renglon,
        };
        congelar::congelado(e, congelar::Pintado::Entero, |e| {
            enviar(e.edit, EM_REPLACESEL, 1, ancho_nulo(&puesto).as_ptr() as isize)
        });
    } else {
        congelar::congelado(e, congelar::Pintado::Entero, |e| insertar_renglon(e, &renglon));
        // Que la aplicacion se ponga a pintarla ya.
        apuntar(Orden::Vivas);
    }
    true
}

pub(super) fn es_enlace_a_hoja(url: &str) -> bool {
    md_imagen::hoja_del_enlace(url).is_some()
}

/// Abre una foto en grande, una hoja por su pagina viva o un enlace a una
/// hoja: lo decide la aplicacion. Sin ella, una foto con lo de Windows.
pub(super) fn abrir(e: &mut Estado, ruta: &str) {
    if let Some(a) = e.integracion.abrir.as_mut() {
        a(ruta);
        return;
    }
    if let Some(r) = (e.resolver)(ruta)
        && let Err(err) = pixpin_shell::abrir(&r)
    {
        tracing::warn!(?err, "no se pudo abrir la foto");
    }
}

/// Mira las paginas vivas de la nota (lo que la aplicacion repinto o se
/// quedo sin hoja) y mete lo que se le mando desde fuera.
pub(super) fn vigilar(e: &mut Estado) {
    if let Some(p) = e.integracion.pendientes.as_mut() {
        let nuevos = p();
        if !nuevos.is_empty() {
            congelar::congelado(e, congelar::Pintado::Entero, |e| {
                for r in nuevos {
                    insertar_renglon(e, &r);
                }
            });
        }
    }
    let texto = leer(e.edit);
    let vivas: Vec<String> = md_imagen::fotos(&texto)
        .into_iter()
        .filter(|(_, f)| md_imagen::hoja_de_viva(&f.ruta).is_some())
        .map(|(_, f)| f.ruta)
        .collect();
    let Some(v) = e.integracion.vigilar.as_mut() else {
        return;
    };
    if vivas.is_empty() {
        return;
    }
    let cambios = v(&vivas);
    if cambios.is_empty() {
        return;
    }
    for (ruta, c) in cambios {
        match c {
            Viva::Renovada => {
                e.fotos.olvidar(&ruta);
                e.sin_hoja.remove(&ruta);
            }
            Viva::SinHoja => {
                e.sin_hoja.insert(ruta);
            }
        }
    }
    pintar(e, None);
    imagenes::repintar(e.edit);
}

/// Pone en marcha la vigilancia si la aplicacion la da, y mira ya.
pub(super) fn arrancar(e: &mut Estado) {
    if e.integracion.vigilar.is_none() && e.integracion.pendientes.is_none() {
        return;
    }
    // SAFETY: temporizador de una ventana propia.
    unsafe {
        SetTimer(Some(e.marco), T_VIVAS, MS_VIVAS, None);
    }
    vigilar(e);
}

// ---------------------------------------------------------------------------
// El raton y el menu de una foto

fn punto(l: LPARAM) -> (i32, i32) {
    ((l.0 & 0xffff) as i16 as i32, ((l.0 >> 16) & 0xffff) as i16 as i32)
}

/// El renglon `n` del texto del control.
fn renglon(e: &Estado, n: usize) -> Option<(md_vivo::Linea, String)> {
    let texto = leer(e.edit);
    let ls = md_vivo::lineas(&texto);
    let l = *ls.get(n)?;
    let u: Vec<u16> = texto.encode_utf16().collect();
    Some((l, String::from_utf16_lossy(&u[l.desde..l.hasta.min(u.len())])))
}

/// Cambia el renglon `n` por `nuevo`, como un paso de deshacer, y deja el
/// cursor al final.
fn cambiar_renglon(e: &Estado, n: usize, nuevo: &str) {
    let Some((l, _)) = renglon(e, n) else { return };
    elegir(e.edit, l.desde, l.hasta);
    enviar(e.edit, EM_REPLACESEL, 1, ancho_nulo(nuevo).as_ptr() as isize);
    let fin = l.desde + nuevo.encode_utf16().count();
    elegir(e.edit, fin, fin);
}

/// Pone el ancho (pixeles a 96 ppp; `None`, el de la columna) de la foto
/// del renglon `n`.
pub(super) fn poner_ancho(e: &Estado, n: usize, ancho: Option<u32>) -> bool {
    let Some((_, r)) = renglon(e, n) else { return false };
    match md_imagen::con_ancho(&r, 0, ancho) {
        Some(nuevo) if nuevo != r => {
            cambiar_renglon(e, n, &nuevo);
            true
        }
        _ => false,
    }
}

/// Quita la foto del renglon `n`, con su salto.
pub(super) fn quitar(e: &Estado, n: usize) -> bool {
    let texto = leer(e.edit);
    let ls = md_vivo::lineas(&texto);
    let Some(l) = ls.get(n).copied() else { return false };
    let Some((_, r)) = renglon(e, n) else { return false };
    if md_imagen::leer(&r).is_none() {
        return false;
    }
    let total = texto.encode_utf16().count();
    let (a, b) = if l.hasta < total {
        (l.desde, l.hasta + 1)
    } else {
        (l.desde.saturating_sub(1), l.hasta)
    };
    elegir(e.edit, a, b);
    enviar(e.edit, EM_REPLACESEL, 1, ancho_nulo("").as_ptr() as isize);
    true
}

/// La ruta de la foto del renglon `n`.
fn ruta_de(e: &Estado, n: usize) -> Option<String> {
    renglon(e, n).and_then(|(_, r)| md_imagen::leer(&r)).map(|f| f.ruta)
}

/// El raton sobre las fotos: el asa cambia el ancho, un clic elige la foto
/// (borde y asa a la vista), doble clic o Ctrl+clic la abre. `true` si se
/// atendio aqui.
pub(super) fn raton(e: &mut Estado, m: &MSG) -> bool {
    let (x, y) = punto(m.lParam);
    match m.message {
        WM_LBUTTONDOWN => {
            let Some((i, asa)) = imagenes::tocar(x, y) else {
                return false;
            };
            let Some((linea, ruta, caja, _)) = imagenes::puesta(i) else {
                return false;
            };
            cerrar_menu(e);
            if asa {
                imagenes::ARRASTRE.with(|a| a.set(Some((i, caja.right - caja.left, caja.bottom - caja.top))));
                ANCHO.with(|a| a.set(None));
                // SAFETY: captura del raton para una ventana propia.
                unsafe {
                    SetCapture(e.edit);
                }
                return true;
            }
            if pulsada(VK_CONTROL.0) {
                abrir(e, &ruta);
                return true;
            }
            if let Some((l, _)) = renglon(e, linea) {
                elegir(e.edit, l.hasta, l.hasta);
            }
            true
        }
        WM_MOUSEMOVE => {
            let Some((i, ..)) = imagenes::ARRASTRE.with(|a| a.get()) else {
                return false;
            };
            let Some((linea, _, caja, _)) = imagenes::puesta(i) else {
                return false;
            };
            // La foto va centrada: crece por los dos lados a la vez, entre
            // el minimo y lo que mide la columna de verdad (en una ventana
            // estrecha, menos que la columna de siempre).
            let centro = (caja.left + caja.right) / 2;
            let px = (2 * (x - centro)).clamp(md_imagen::ANCHO_MINIMO as i32 * e.ppp / 96, columna_px(e) * e.ppp / 96);
            // Lo que se escribira al soltar (pixeles a 96 ppp) y lo que medira
            // entonces, con las mismas cuentas que al leerla: lo que se ve
            // mientras se arrastra es exactamente lo que queda (sin rebote).
            let ancho96 = ((px * 96 + e.ppp / 2) / e.ppp.max(1)).max(md_imagen::ANCHO_MINIMO as i32) as u32;
            let Some((an, al)) = tamano_con_ancho(e, i, ancho96) else {
                return true;
            };
            imagenes::ARRASTRE.with(|a| a.set(Some((i, an, al))));
            ANCHO.with(|a| a.set(Some(ancho96)));
            // El hueco crece o mengua con ella mientras se arrastra: el texto
            // de debajo se aparta y nunca queda debajo de la foto.
            hueco_de_foto(e, linea, al);
            // SAFETY: ventana propia; el repintado pinta la foto a su ancho nuevo.
            unsafe {
                let _ = InvalidateRect(Some(e.edit), None, false);
            }
            true
        }
        WM_LBUTTONUP => {
            let Some((i, ..)) = imagenes::ARRASTRE.with(|a| a.get()) else {
                return false;
            };
            // SAFETY: suelta la captura de este hilo.
            unsafe {
                let _ = ReleaseCapture();
            }
            // Antes, al soltar se volvia un momento a la foto vieja (el
            // arrastre ya no estaba y el formato llegaba 60 ms despues) y luego
            // a la nueva: «se achica y agranda de nuevo». Ahora el ancho se
            // escribe y la foto se lee a su tamano nuevo congelado, y el primer
            // fotograma ya es el de soltar.
            let ancho96 = ANCHO.with(Cell::take);
            congelar::congelado(e, congelar::Pintado::Entero, |e| {
                imagenes::ARRASTRE.with(|a| a.set(None));
                if let (Some((linea, ..)), Some(w)) = (imagenes::puesta(i), ancho96) {
                    poner_ancho(e, linea, Some(w));
                }
            });
            true
        }
        WM_LBUTTONDBLCLK => match imagenes::tocar(x, y) {
            Some((i, false)) => {
                if let Some((_, ruta, _, _)) = imagenes::puesta(i) {
                    abrir(e, &ruta);
                }
                true
            }
            _ => false,
        },
        _ => false,
    }
}

/// Sobre el asa de la foto elegida, la flecha de cambiar el tamano.
pub(super) fn cursor_del_asa(edit: HWND) -> bool {
    let mut p = POINT::default();
    // SAFETY: posicion del raton y conversion para una ventana propia.
    unsafe {
        let _ = GetCursorPos(&mut p);
        let _ = ScreenToClient(edit, &mut p);
    }
    let encima = imagenes::ARRASTRE.with(|a| a.get()).is_some() || matches!(imagenes::tocar(p.x, p.y), Some((_, true)));
    if encima {
        // SAFETY: cursor del sistema, compartido; no se suelta.
        unsafe {
            if let Ok(c) = LoadCursorW(None, IDC_SIZENWSE) {
                SetCursor(Some(c));
            }
        }
    }
    encima
}

/// La foto del menu: la que hay bajo el raton o, con el teclado, la del
/// renglon del cursor.
fn foto_para_el_menu(e: &Estado) -> Option<usize> {
    let mut p = POINT::default();
    // SAFETY: posicion del raton y conversion para una ventana propia.
    unsafe {
        let _ = GetCursorPos(&mut p);
        let _ = ScreenToClient(e.edit, &mut p);
    }
    if let Some((i, _)) = imagenes::tocar(p.x, p.y) {
        return imagenes::puesta(i).map(|(n, ..)| n);
    }
    let texto = leer(e.edit);
    let n = md_vivo::linea_de(&md_vivo::lineas(&texto), seleccion(e.edit).1);
    ruta_de(e, n).map(|_| n)
}

/// Lo de la foto en el menu del clic derecho, si es sobre una foto.
pub(super) fn entradas_del_menu(e: &mut Estado) -> Vec<Option<(u16, String)>> {
    e.foto_del_menu = foto_para_el_menu(e);
    let Some(n) = e.foto_del_menu else {
        return Vec::new();
    };
    let r = &e.rotulos.fotos;
    let viva = ruta_de(e, n).is_some_and(|ruta| md_imagen::hoja_de_viva(&ruta).is_some());
    vec![
        Some((C_VER_FOTO, if viva { r.abrir_hoja.clone() } else { r.ver_grande.clone() })),
        None,
        Some((C_FOTO_PEQUENA, r.pequena.clone())),
        Some((C_FOTO_MEDIANA, r.mediana.clone())),
        Some((C_FOTO_GRANDE, r.grande.clone())),
        Some((C_FOTO_COLUMNA, r.columna.clone())),
        None,
        Some((C_QUITAR_FOTO, r.quitar.clone())),
        None,
    ]
}

/// Las ordenes del menu de una foto.
pub(super) fn orden_de_foto(e: &mut Estado, c: u16) {
    let Some(n) = e.foto_del_menu.take().or_else(|| foto_para_el_menu(e)) else {
        return;
    };
    match c {
        C_VER_FOTO => {
            if let Some(ruta) = ruta_de(e, n) {
                abrir(e, &ruta);
            }
        }
        C_FOTO_PEQUENA => {
            poner_ancho(e, n, Some(PEQUENA));
        }
        C_FOTO_MEDIANA => {
            poner_ancho(e, n, Some(MEDIANA));
        }
        C_FOTO_GRANDE => {
            poner_ancho(e, n, Some(GRANDE));
        }
        C_FOTO_COLUMNA => {
            poner_ancho(e, n, None);
        }
        C_QUITAR_FOTO => {
            quitar(e, n);
        }
        _ => {}
    }
}

#[cfg(test)]
mod pruebas;
