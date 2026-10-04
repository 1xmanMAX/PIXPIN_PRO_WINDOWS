//! **Comentar en la nota** (H12, 30-sep-2026), como en Google Docs o en el
//! editor de documentos de Claude: se elige un trozo, «Comentar» (el boton
//! de la barra, el clic derecho o Ctrl+Alt+M), y el trozo queda resaltado y
//! el comentario sale en el panel de la derecha **a la altura de su frase**.
//! Se responde (hilo), se edita y se borra con su «⋯», y se resuelve con
//! la marca: el resuelto se va del texto y del panel, y el filtro de la
//! cabecera del panel lo vuelve a ensenar. Un clic en un comentario lleva a
//! su texto; el cursor en un texto comentado elige su comentario. El globo
//! de la cabecera abre y cierra el panel y dice cuantos quedan abiertos.
//!
//! Los comentarios **no van dentro del Markdown** (ver
//! `pixpin_docs::md_comentarios`): los lee y los escribe la aplicacion en un
//! fichero hermano por las dos funciones de [`DeComentarios`]. Al guardar se
//! juntan con lo que haya en disco (la sincronizacion pudo traer respuestas
//! del movil mientras la nota estaba abierta) en vez de pisarlo.
//!
//! El resaltado son colores de fondo del `RichEdit`, puestos con el
//! deshacer suspendido como el resto del formato: Ctrl+Z deshace letras, no
//! comentarios.

use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::RichEdit::*;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, w};

use pixpin_docs::md_comentarios::{self as md, Ancla, Quien};
use pixpin_docs::md_tabla;

use super::{EM_POSFROMCHAR, Estado, elegir, enviar, leer, markdown, pintar, recolocar, seleccion};
use crate::panel_comentarios::{self as panel, Accion, Compositor, Entrada, Tarjeta};
use crate::tema::bgr;

/// Lo que da la aplicacion: quien escribe y donde estan los comentarios.
#[derive(Default)]
pub struct DeComentarios {
    /// El nombre de este aparato (se ensena junto a cada comentario).
    pub autor: String,
    /// Su codigo de aparato.
    pub aparato: String,
    /// El texto del fichero de comentarios: vacio si no hay; `None` si esta
    /// pero no se lee (entonces no se escribe encima).
    pub leer: Option<Box<dyn Fn() -> Option<String>>>,
    /// Escribe el fichero; `false` si no pudo (o la nota aun no tiene sitio).
    pub guardar: Option<Box<dyn FnMut(&str) -> bool>>,
}

/// Lo que se esta escribiendo.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Borrador {
    Nuevo { ancla: Ancla, rango: (usize, usize) },
    Responder(String),
    Editar(String),
}

/// El estado de los comentarios dentro del editor.
pub(super) struct Comentarios {
    pub(super) datos: md::Comentarios,
    /// Lo leido o lo ultimo guardado: con que se junta lo de disco.
    base: md::Comentarios,
    /// El fichero estaba pero no se entendia: no se escribe encima.
    roto: bool,
    quien: Quien,
    leer: Option<Box<dyn Fn() -> Option<String>>>,
    guardar: Option<Box<dyn FnMut(&str) -> bool>>,
    /// Donde cae cada hilo en el texto del control (en el orden de `datos`).
    pub(super) rangos: Vec<Option<(usize, usize)>>,
    pub(super) ver_resueltos: bool,
    pub(super) activo: Option<String>,
    pub(super) borrador: Option<Borrador>,
    pub(super) compositor: Option<HWND>,
    /// Por donde iba la nota la ultima vez que se coloco el panel.
    desplazado: POINT,
    /// La ventana de las tarjetas (ver `carril`), creada al abrirlas.
    carril: Option<HWND>,
}

fn ahora() -> i64 {
    pixpin_shell::entorno::ahora_utc_ms()
}

/// La ventana de las tarjetas (encima de la nota en el cajon).
mod carril;

impl Comentarios {
    /// Lee los comentarios de la nota. Las tarjetas nacen escondidas: solo
    /// salen con el boton de la cabecera (el usuario, 1-oct: «solo se
    /// muestren cuando le doy al boton de comentarios»), que dice cuantos hay.
    pub(super) fn nuevo(d: DeComentarios) -> Comentarios {
        let (datos, roto) = match d.leer.as_ref().map(|f| f()) {
            None => (md::Comentarios::default(), false),
            Some(None) => {
                tracing::warn!("los comentarios de la nota estan pero no se leen; no se tocaran");
                (md::Comentarios::default(), true)
            }
            Some(Some(t)) => match md::leer(&t) {
                Ok(c) => (c, false),
                Err(err) => {
                    tracing::warn!(%err, "los comentarios de la nota no se entienden; no se tocaran");
                    (md::Comentarios::default(), true)
                }
            },
        };
        panel::ABIERTO.with(|a| a.set(false));
        panel::CONTADOR.with(|c| c.set(datos.abiertos()));
        Comentarios {
            base: datos.clone(),
            datos,
            roto,
            quien: Quien {
                autor: d.autor,
                aparato: d.aparato,
            },
            leer: d.leer,
            guardar: d.guardar,
            rangos: Vec::new(),
            ver_resueltos: false,
            activo: None,
            borrador: None,
            compositor: None,
            desplazado: POINT {
                x: i32::MIN,
                y: i32::MIN,
            },
            carril: None,
        }
    }
}

// ---------------------------------------------------------------------------
// El resaltado

fn u16s(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Pone el fondo de un tramo sin moverle la seleccion al control.
fn fondo(e: &Estado, (a, b): (usize, usize), color: u32) {
    if let Some(d) = &e.doc {
        // SAFETY: rango del documento vivo del control.
        unsafe {
            if let Ok(r) = d.Range(a as i32, b as i32)
                && let Ok(f) = r.GetFont()
            {
                let _ = f.SetBackColor(bgr(color) as i32);
            }
        }
    }
}

/// **Resalta lo comentado**. Lo llama `pintar` con el deshacer suspendido;
/// `entero` (tras un cambio del texto) vuelve a buscar cada ancla.
pub(super) fn resaltar(e: &mut Estado, texto: &str, entero: bool) {
    if entero || e.comentarios.rangos.len() != e.comentarios.datos.comentarios.len() {
        e.comentarios.rangos = e.comentarios.datos.reanclar(&u16s(texto));
    }
    poner_colores(e);
    // Las tarjetas se colocan al descongelar (`pintar`, `congelar`):
    // congelado, el control no ha vuelto a medir y daria sitios viejos.
}

fn poner_colores(e: &Estado) {
    let co = panel::colores(&e.estilos.tema);
    let c = &e.comentarios;
    for (h, r) in c.datos.comentarios.iter().zip(&c.rangos) {
        if let Some(r) = r
            && !h.resuelto
        {
            let activo = c.activo.as_deref() == Some(h.id.as_str());
            fondo(
                e,
                *r,
                if activo {
                    co.resaltado_activo
                } else {
                    co.resaltado
                },
            );
        }
    }
    if let Some(Borrador::Nuevo { rango, .. }) = &c.borrador {
        fondo(e, *rango, co.resaltado_activo);
    }
}

/// Cambia los colores sin repintar todo (al elegir otro comentario).
fn recolorear(e: &Estado) {
    let mascara = enviar(e.edit, EM_GETEVENTMASK, 0, 0);
    enviar(e.edit, EM_SETEVENTMASK, 0, ENM_NONE as isize);
    if let Some(d) = &e.doc {
        // SAFETY: documento vivo del control; se reanuda abajo.
        unsafe {
            let _ = d.Undo(tomSuspend.0);
        }
    }
    poner_colores(e);
    if let Some(d) = &e.doc {
        // SAFETY: como arriba.
        unsafe {
            let _ = d.Undo(tomResume.0);
        }
    }
    enviar(e.edit, EM_SETEVENTMASK, 0, mascara);
}

// ---------------------------------------------------------------------------
// El panel

/// Lo alto en la ventana de la letra `pos` del control.
fn alto_de(e: &Estado, pos: usize) -> i32 {
    let mut p = POINT::default();
    enviar(
        e.edit,
        EM_POSFROMCHAR,
        &mut p as *mut _ as usize,
        pos as isize,
    );
    let cuerpo = super::VISTA
        .with(|v| v.borrow().as_ref().map(|v| v.disp.cuerpo))
        .unwrap_or_default();
    cuerpo.y + p.y
}

fn fecha(e: &Estado, cuando: i64, editado: Option<i64>) -> String {
    let meses: Vec<String> = e
        .rotulos
        .meses
        .split_whitespace()
        .map(String::from)
        .collect();
    let desfase = pixpin_shell::entorno::ahora_local_ms() - ahora();
    let f = panel::fecha_corta(cuando, desfase, ahora(), &meses);
    match editado {
        Some(_) => format!("{f} · {}", e.rotulos.comentarios.editado),
        None => f,
    }
}

/// Las tarjetas de ahora, en el orden del texto.
fn tarjetas(e: &Estado) -> (Vec<Tarjeta>, usize) {
    let c = &e.comentarios;
    let mut con_sitio: Vec<(usize, Tarjeta)> = Vec::new();
    let mut sin_sitio: Vec<Tarjeta> = Vec::new();
    let mut resueltos = 0;
    for (i, h) in c.datos.comentarios.iter().enumerate() {
        if h.resuelto {
            resueltos += 1;
            if !c.ver_resueltos {
                continue;
            }
        }
        let rango = c.rangos.get(i).copied().flatten().filter(|_| !h.resuelto);
        let compositor = match &c.borrador {
            Some(Borrador::Responder(id)) if *id == h.id => Some(Compositor::Responder),
            Some(Borrador::Editar(id))
                if *id == h.id || h.respuestas.iter().any(|r| r.id == *id) =>
            {
                Some(Compositor::Editar(id.clone()))
            }
            _ => None,
        };
        let mut entradas = vec![Entrada {
            id: h.id.clone(),
            autor: h.autor.clone(),
            aparato: h.aparato.clone(),
            fecha: fecha(e, h.cuando, h.editado),
            texto: h.texto.clone(),
        }];
        entradas.extend(h.respuestas.iter().map(|r| Entrada {
            id: r.id.clone(),
            autor: r.autor.clone(),
            aparato: r.aparato.clone(),
            fecha: fecha(e, r.cuando, r.editado),
            texto: r.texto.clone(),
        }));
        let t = Tarjeta {
            id: h.id.clone(),
            ancla_y: rango.map(|(a, _)| alto_de(e, a)),
            cita: h.ancla.cita.clone(),
            resuelto: h.resuelto,
            activa: c.activo.as_deref() == Some(h.id.as_str()),
            entradas,
            compositor,
            autor_nuevo: (c.quien.autor.clone(), c.quien.aparato.clone()),
        };
        match rango {
            Some((a, _)) => con_sitio.push((a, t)),
            None => sin_sitio.push(t),
        }
    }
    if let Some(Borrador::Nuevo { ancla, rango }) = &c.borrador {
        con_sitio.push((
            rango.0,
            Tarjeta {
                id: String::new(),
                ancla_y: Some(alto_de(e, rango.0)),
                cita: ancla.cita.clone(),
                resuelto: false,
                activa: true,
                entradas: Vec::new(),
                compositor: Some(Compositor::Nuevo),
                autor_nuevo: (c.quien.autor.clone(), c.quien.aparato.clone()),
            },
        ));
    }
    con_sitio.sort_by_key(|(a, _)| *a);
    sin_sitio.extend(con_sitio.into_iter().map(|(_, t)| t));
    (sin_sitio, resueltos)
}

/// **Coloca el panel** con lo de ahora y lo manda pintar.
pub(super) fn componer(e: &mut Estado) {
    panel::CONTADOR.with(|c| c.set(e.comentarios.datos.abiertos()));
    let caja = panel::CAJA.with(|c| c.get());
    let Some(caja) = caja.filter(|_| panel::ABIERTO.with(|a| a.get())) else {
        panel::VISTA.with(|v| *v.borrow_mut() = None);
        if let Some(h) = e.comentarios.compositor {
            // SAFETY: control propio.
            unsafe {
                let _ = ShowWindow(h, SW_HIDE);
            }
        }
        if let Some(h) = e.comentarios.carril {
            carril::esconder(h);
        }
        // SAFETY: ventana propia; el globo de la cabecera cambia.
        unsafe {
            let _ = InvalidateRect(Some(e.marco), None, false);
        }
        return;
    };
    let (lista, resueltos) = tarjetas(e);
    let datos = panel::Datos {
        tarjetas: lista,
        resueltos,
        ver_resueltos: e.comentarios.ver_resueltos,
        rotulos: &e.rotulos.comentarios,
        tema: e.estilos.tema,
        escala: e.pintor.escala,
        cajon: panel::CAJON.with(|c| c.get()),
    };
    // SAFETY: el DC de la pantalla se pide y se devuelve aqui, para medir.
    let vista = unsafe {
        let dc = GetDC(None);
        let v = panel::disponer(
            &datos,
            caja,
            &panel::MedirGdi {
                hdc: dc,
                pintor: &e.pintor,
            },
        );
        ReleaseDC(None, dc);
        v
    };
    let sitio = vista.compositor_visible();
    panel::VISTA.with(|v| *v.borrow_mut() = Some(vista));
    // La ventana de las tarjetas, encima de la nota (en el cajon la tapa).
    if e.comentarios.carril.is_none() {
        e.comentarios.carril = carril::crear(e.marco);
    }
    if let Some(h) = e.comentarios.carril {
        carril::colocar(h, caja);
    }
    if let Some(h) = e.comentarios.compositor {
        // SAFETY: control propio, dentro del marco, encima de las tarjetas.
        unsafe {
            match sitio {
                Some(c) => {
                    let _ = SetWindowPos(
                        h,
                        Some(HWND_TOP),
                        c.x,
                        c.y,
                        c.an,
                        c.al,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                }
                None => {
                    let _ = ShowWindow(h, SW_HIDE);
                }
            }
        }
    }
    enviar(
        e.edit,
        EM_GETSCROLLPOS,
        0,
        &mut e.comentarios.desplazado as *mut _ as isize,
    );
    // SAFETY: ventana propia; el globo de la cabecera tambien cambia.
    unsafe {
        let _ = InvalidateRect(Some(e.marco), None, false);
    }
}

/// Abre o cierra el panel: el papel se estrecha o se ensancha.
pub(super) fn abrir_panel(e: &mut Estado, si: bool) {
    if !si {
        cancelar(e);
    }
    panel::ABIERTO.with(|a| a.set(si));
    recolocar(e.marco);
    super::colocar_edit(e.marco);
    super::apuntar(super::Orden::Tamano);
    componer(e);
}

pub(super) fn alternar_panel(e: &mut Estado) {
    let abierto = panel::ABIERTO.with(|a| a.get());
    abrir_panel(e, !abierto);
}

// ---------------------------------------------------------------------------
// Escribir

/// El cuadro de escribir: un `EDIT` de Windows (IME, corrector, pegar) en
/// la tarjeta. Intro envia, Mayus+Intro parte el renglon, Esc cancela.
fn abrir_compositor(e: &mut Estado, texto: &str) {
    if let Some(h) = e.comentarios.compositor.take() {
        // SAFETY: control propio.
        unsafe {
            let _ = DestroyWindow(h);
        }
    }
    // SAFETY: control hijo propio; se destruye en `cerrar_compositor`.
    unsafe {
        let Ok(h) = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EDIT"),
            &HSTRING::from(texto.replace('\n', "\r\n")),
            WS_CHILD | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL) as u32),
            0,
            0,
            10,
            10,
            Some(e.marco),
            None,
            None,
            None,
        ) else {
            return;
        };
        SendMessageW(
            h,
            WM_SETFONT,
            Some(WPARAM(e.pintor.letra.0 as usize)),
            Some(LPARAM(1)),
        );
        let n = texto.encode_utf16().count() + texto.matches('\n').count();
        SendMessageW(h, 0x00B1, Some(WPARAM(n)), Some(LPARAM(n as isize)));
        e.comentarios.compositor = Some(h);
    }
    componer(e);
    if let Some(h) = e.comentarios.compositor
        && !e.oculto
    {
        // SAFETY: control propio.
        unsafe {
            let _ = SetFocus(Some(h));
        }
    }
}

fn texto_del_compositor(e: &Estado) -> String {
    let Some(h) = e.comentarios.compositor else {
        return String::new();
    };
    // SAFETY: control propio; el bufer es local.
    unsafe {
        let n = GetWindowTextLengthW(h).max(0) as usize;
        let mut b = vec![0u16; n + 1];
        let copiados = GetWindowTextW(h, &mut b).max(0) as usize;
        String::from_utf16_lossy(&b[..copiados]).replace("\r\n", "\n")
    }
}

/// Deja de escribir sin guardar nada.
pub(super) fn cancelar(e: &mut Estado) {
    let era_nuevo = matches!(e.comentarios.borrador, Some(Borrador::Nuevo { .. }));
    e.comentarios.borrador = None;
    if let Some(h) = e.comentarios.compositor.take() {
        // SAFETY: control propio; el foco vuelve a la nota.
        unsafe {
            let _ = DestroyWindow(h);
            let _ = SetFocus(Some(e.edit));
        }
    }
    if era_nuevo {
        // El resaltado del trozo que se iba a comentar se va.
        pintar(e, None);
    } else {
        componer(e);
    }
}

/// **Comentar lo elegido** (o la palabra del cursor si no hay nada
/// elegido). Abre el panel y el cuadro de escribir junto a su frase.
pub(super) fn comentar(e: &mut Estado) {
    let texto = leer(e.edit);
    let u = u16s(&texto);
    let (mut a, mut b) = seleccion(e.edit);
    if a == b {
        match md::palabra_en(&u, a) {
            Some(r) => (a, b) = r,
            None => return,
        }
    }
    // Un trozo de tabla se corta en la primera raya de celda: una cita que
    // cruza celdas no se encontraria en el Markdown.
    let corte = u[a..b.min(u.len())].iter().position(|&c| {
        [md_tabla::FILA_ABRE, md_tabla::FILA_CIERRA, md_tabla::CELDA]
            .contains(&char::from_u32(c as u32).unwrap_or(' '))
    });
    if let Some(k) = corte {
        b = a + k;
    }
    let Some(ancla) = md::ancla_de(&u, a, b) else {
        return;
    };
    if e.comentarios.compositor.is_some() {
        cancelar(e);
    }
    let rango = (ancla.pos, ancla.pos + ancla.cita.encode_utf16().count());
    e.comentarios.activo = None;
    e.comentarios.borrador = Some(Borrador::Nuevo { ancla, rango });
    if !panel::ABIERTO.with(|x| x.get()) {
        panel::ABIERTO.with(|x| x.set(true));
        recolocar(e.marco);
        super::colocar_edit(e.marco);
        super::apuntar(super::Orden::Tamano);
    }
    recolorear(e);
    abrir_compositor(e, "");
}

/// Lo escrito pasa a ser un comentario, una respuesta o el texto nuevo.
pub(super) fn enviar_borrador(e: &mut Estado) {
    let texto = texto_del_compositor(e);
    if texto.trim().is_empty() {
        return;
    }
    let quien = e.comentarios.quien.clone();
    let c = &mut e.comentarios;
    match c.borrador.take() {
        Some(Borrador::Nuevo { ancla, .. }) => {
            if let Some(id) = c.datos.nuevo(ancla, &quien, ahora(), &texto) {
                c.activo = Some(id);
            }
        }
        Some(Borrador::Responder(id)) => {
            c.datos.responder(&id, &quien, ahora(), &texto);
            c.activo = Some(id);
        }
        Some(Borrador::Editar(id)) => {
            c.datos.editar(&id, &texto, ahora());
        }
        None => {}
    }
    if let Some(h) = c.compositor.take() {
        // SAFETY: control propio; el foco vuelve a la nota.
        unsafe {
            let _ = DestroyWindow(h);
            let _ = SetFocus(Some(e.edit));
        }
    }
    cambiaron(e);
}

/// Tras cambiar los comentarios: repintar los resaltados y guardar.
fn cambiaron(e: &mut Estado) {
    pintar(e, None);
    guardar_ya(e);
}

/// Guarda ya los comentarios (cada cambio se guarda: son pocos bytes).
fn guardar_ya(e: &mut Estado) {
    let md = markdown(e);
    guardar(e, &md);
}

/// **Guarda los comentarios** con el ancla al dia contra el Markdown que se
/// guarda, juntandolos con lo que haya ahora en disco. No escribe si nada
/// cambio, si el fichero que habia no se entendia o si la nota aun no tiene
/// sitio (una nota nueva sin guardar: se guardaran con ella).
pub(super) fn guardar(e: &mut Estado, md_texto: &str) {
    let c = &mut e.comentarios;
    if c.roto || c.guardar.is_none() {
        return;
    }
    let mut mio = c.datos.clone();
    mio.reanclar(&u16s(md_texto));
    if md::escribir(&mio) == md::escribir(&c.base) {
        return;
    }
    let en_disco = match c.leer.as_ref().map(|f| f()) {
        Some(Some(t)) => match md::leer(&t) {
            Ok(d) => d,
            Err(err) => {
                tracing::warn!(%err, "los comentarios en disco ya no se entienden; no se pisan");
                return;
            }
        },
        Some(None) => return,
        None => c.base.clone(),
    };
    let junto = md::fusionar(&c.base, &mio, &en_disco);
    let Some(guardar) = c.guardar.as_mut() else {
        return;
    };
    if guardar(&md::escribir(&junto)) {
        let llego_algo = junto != mio;
        c.base = junto.clone();
        if llego_algo {
            // Lo que llego del otro aparato se ve ya.
            c.datos = junto;
            c.rangos.clear();
            super::apuntar(super::Orden::Repintar);
        }
    }
}

// ---------------------------------------------------------------------------
// Clics y teclas

/// El clic que apunto el panel.
pub(super) fn clic(e: &mut Estado, (x, y): (i32, i32)) {
    let accion = panel::VISTA.with(|v| v.borrow().as_ref().and_then(|v| v.accion_en(x, y)));
    if let Some(a) = accion {
        hacer(e, a);
    }
}

pub(super) fn hacer(e: &mut Estado, a: Accion) {
    match a {
        Accion::CerrarPanel => abrir_panel(e, false),
        Accion::Filtro => {
            e.comentarios.ver_resueltos = !e.comentarios.ver_resueltos;
            componer(e);
        }
        Accion::Tarjeta(id) if !id.is_empty() => elegir_hilo(e, &id, true),
        Accion::Tarjeta(_) => {}
        Accion::Resolver(id) => {
            let si = !e.comentarios.datos.hilo(&id).is_some_and(|h| h.resuelto);
            let quien = e.comentarios.quien.clone();
            if e.comentarios.datos.resolver(&id, si, &quien, ahora()) {
                if si {
                    e.comentarios.activo = None;
                }
                cambiaron(e);
            }
        }
        Accion::Mas(id) => menu_mas(e, &id),
        Accion::Responder(id) => {
            e.comentarios.activo = Some(id.clone());
            e.comentarios.borrador = Some(Borrador::Responder(id));
            abrir_compositor(e, "");
        }
        Accion::Enviar => enviar_borrador(e),
        Accion::Cancelar => cancelar(e),
    }
}

/// Elige un hilo; con `saltar`, lleva la nota a su texto.
pub(super) fn elegir_hilo(e: &mut Estado, id: &str, saltar: bool) {
    e.comentarios.activo = Some(id.to_string());
    let i = e
        .comentarios
        .datos
        .comentarios
        .iter()
        .position(|h| h.id == id);
    if saltar && let Some((a, _)) = i.and_then(|i| e.comentarios.rangos.get(i).copied().flatten()) {
        // El cursor a su texto sin que el control salte; luego la nota se
        // desliza hasta dejarlo a un tercio de arriba, si no se ve bien.
        let mut antes = POINT::default();
        enviar(e.edit, EM_GETSCROLLPOS, 0, &mut antes as *mut _ as isize);
        elegir(e.edit, a, a);
        enviar(e.edit, EM_SETSCROLLPOS, 0, &antes as *const _ as isize);
        let mut p = POINT::default();
        enviar(
            e.edit,
            EM_POSFROMCHAR,
            &mut p as *mut _ as usize,
            a as isize,
        );
        let (visible, _) = crate::imagenes::medidas(e.edit);
        if p.y < visible / 8 || p.y > visible * 3 / 4 {
            super::wysiwyg::deslizar_a(e, antes.y + p.y - visible / 3);
        }
    }
    recolorear(e);
    pintar(e, None);
}

/// Editar o borrar (el «⋯»): un menu de Windows en el raton.
fn menu_mas(e: &mut Estado, id: &str) {
    const EDITAR: usize = 1;
    const BORRAR: usize = 2;
    if e.oculto {
        return;
    }
    let r = &e.rotulos.comentarios;
    // SAFETY: el menu se crea y se destruye aqui; las cadenas viven durante
    // cada llamada; la ventana es propia.
    let elegido = unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let _ = AppendMenuW(menu, MF_STRING, EDITAR, &HSTRING::from(r.editar.as_str()));
        let _ = AppendMenuW(menu, MF_STRING, BORRAR, &HSTRING::from(r.borrar.as_str()));
        let mut p = POINT::default();
        let _ = GetCursorPos(&mut p);
        let x = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            p.x,
            p.y,
            None,
            e.marco,
            None,
        );
        let _ = DestroyMenu(menu);
        x.0 as usize
    };
    match elegido {
        EDITAR => editar(e, id),
        BORRAR => {
            let es_hilo = e.comentarios.datos.hilo(id).is_some();
            if !es_hilo
                || pixpin_shell::dialogo::preguntar(
                    e.marco,
                    &r.comentarios.clone(),
                    &r.borrar_hilo.clone(),
                )
            {
                borrar(e, id);
            }
        }
        _ => {}
    }
}

/// Abre el cuadro con el texto de esa entrada para cambiarlo.
pub(super) fn editar(e: &mut Estado, id: &str) {
    let texto = e.comentarios.datos.comentarios.iter().find_map(|h| {
        if h.id == id {
            Some(h.texto.clone())
        } else {
            h.respuestas
                .iter()
                .find(|r| r.id == id)
                .map(|r| r.texto.clone())
        }
    });
    let Some(texto) = texto else {
        return;
    };
    if let Some(h) = e
        .comentarios
        .datos
        .comentarios
        .iter()
        .find(|h| h.id == id || h.respuestas.iter().any(|r| r.id == id))
    {
        e.comentarios.activo = Some(h.id.clone());
    }
    e.comentarios.borrador = Some(Borrador::Editar(id.to_string()));
    abrir_compositor(e, &texto);
}

pub(super) fn borrar(e: &mut Estado, id: &str) {
    if e.comentarios.datos.borrar(id) {
        if e.comentarios.activo.as_deref() == Some(id) {
            e.comentarios.activo = None;
        }
        cambiaron(e);
    }
}

/// Una tecla en el cuadro de escribir: `true` si se atendio aqui.
pub(super) fn tecla(e: &mut Estado, m: &MSG) -> bool {
    if e.comentarios.compositor != Some(m.hwnd) || m.message != WM_KEYDOWN {
        return false;
    }
    let k = m.wParam.0 as u16;
    if k == super::VK_RETURN.0 && !super::pulsada(super::VK_SHIFT.0) {
        enviar_borrador(e);
        true
    } else if k == super::VK_ESCAPE.0 {
        cancelar(e);
        true
    } else {
        false
    }
}

/// Tras un mensaje de la nota: si se movio, el panel la sigue; si el cursor
/// cae en un texto comentado, se elige su comentario (y con un clic, se
/// abre el panel si estaba cerrado).
pub(super) fn seguir(e: &mut Estado, m: &MSG) {
    if m.hwnd == e.edit && matches!(m.message, WM_LBUTTONUP | WM_KEYUP) {
        let (_, cursor) = seleccion(e.edit);
        let c = &e.comentarios;
        let bajo = c
            .datos
            .comentarios
            .iter()
            .zip(&c.rangos)
            .find(|(h, r)| !h.resuelto && r.is_some_and(|(a, b)| a <= cursor && cursor <= b))
            .map(|(h, _)| h.id.clone());
        // Un clic en un texto comentado ensena las tarjetas (si no se veian)
        // y elige la suya; el cursor que llega con las flechas solo la elige.
        let cerrado = !panel::ABIERTO.with(|a| a.get());
        if let Some(id) = bajo
            && (c.activo.as_deref() != Some(id.as_str()) || (cerrado && m.message == WM_LBUTTONUP))
        {
            if m.message == WM_LBUTTONUP && cerrado {
                abrir_panel(e, true);
            }
            elegir_hilo(e, &id, false);
            return;
        }
    }
    if panel::ABIERTO.with(|a| a.get()) {
        let mut p = POINT::default();
        enviar(e.edit, EM_GETSCROLLPOS, 0, &mut p as *mut _ as isize);
        if p.x != e.comentarios.desplazado.x || p.y != e.comentarios.desplazado.y {
            componer(e);
        }
    }
}

/// Al cerrar la ventana.
pub(super) fn desmontar(e: &Estado) {
    for h in [e.comentarios.compositor, e.comentarios.carril]
        .into_iter()
        .flatten()
    {
        // SAFETY: ventanas propias.
        unsafe {
            let _ = DestroyWindow(h);
        }
    }
    panel::VISTA.with(|v| *v.borrow_mut() = None);
    panel::ABIERTO.with(|a| a.set(false));
    panel::CONTADOR.with(|c| c.set(0));
}

/// El cuadro de escribir en una muestra (es una ventana aparte).
pub(super) fn pintar_compositor(e: &Estado, dc: HDC) {
    let Some(h) = e.comentarios.compositor else {
        return;
    };
    let Some(c) = panel::VISTA.with(|v| v.borrow().as_ref().and_then(|v| v.compositor_visible()))
    else {
        return;
    };
    // SAFETY: DC de la muestra; el origen se devuelve como estaba.
    unsafe {
        let mut antes = POINT::default();
        let _ = SetViewportOrgEx(dc, c.x, c.y, Some(&mut antes));
        SendMessageW(
            h,
            WM_PRINT,
            Some(WPARAM(dc.0 as usize)),
            Some(LPARAM((PRF_CLIENT | PRF_ERASEBKGND) as isize)),
        );
        let _ = SetViewportOrgEx(dc, antes.x, antes.y, None);
    }
}

#[cfg(test)]
mod pruebas;
