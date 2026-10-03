//! **Los incrustados dentro de la ventana** (H12, 1-oct): documentos,
//! mensajes del chat, hojas enlazadas y audios con su transcripcion. Lo
//! que son y como se escriben esta en `crate::incrustados`; aqui, meterlos
//! (menus `+` y `/`, soltar y pegar ficheros, «Del chat»), pintarlos
//! encima de su renglon por el mismo camino que las fotos, el raton
//! (abrir, tocar, la barra, la velocidad, pasar a texto, las marcas de
//! tiempo) y el latido del reproductor.
//!
//! # Nacen pintados
//!
//! Un renglon incrustado se mete con el documento congelado y se le pone
//! el formato antes de soltarlo ([`insertar_pintado`]): nunca se ve su
//! Markdown, ni un instante.
//!
//! # Las marcas de tiempo de la transcripcion
//!
//! `[1:23] lo que se dijo` es texto de la nota (el movil lo lee igual): el
//! `[` y el `]` van escondidos y el minuto se pinta como una chapa del
//! color del acento; un clic en el salta el audio de encima a ese punto, y
//! el parrafo que suena lleva fondo, como la «Letra» del chat.

use std::collections::HashMap;

use super::*;
use crate::incrustados::pintar::{self as pin, ComoSuena, Zonas};
use crate::incrustados::{self as inc, Clase, EstadoAudio, Ficha, OrdenAudio};

/// El latido del reproductor y de la transcripcion: cuatro veces por
/// segundo basta para que el tiempo y la barra avancen sin saltos que se
/// noten, y no despierta a la maquina mas de la cuenta.
pub(super) const T_AUDIO: usize = 11;
const MS_AUDIO: u32 = 250;

pub(super) const C_DOCUMENTO: u16 = 130;
pub(super) const C_DEL_CHAT: u16 = 131;
pub(super) const C_AUDIO: u16 = 132;
const C_ABRIR: u16 = 133;
const C_QUITAR: u16 = 134;

/// Lo pintado de un incrustado y con que se pinto: si algo cambia, se
/// vuelve a pintar.
#[derive(Clone, PartialEq)]
struct Clave {
    ficha: Ficha,
    suena: ComoSuena,
    columna: i32,
    papel: Rgb,
    oscuro: bool,
    ppp: i32,
}

#[derive(Default)]
struct Memoria {
    /// Lo que dijo la aplicacion de cada ruta (`None`: no sabe que es).
    fichas: HashMap<String, Option<Ficha>>,
    pintados: HashMap<String, (Clave, Rc<imagenes::Foto>, Zonas)>,
    /// Como iba el reproductor en el ultimo latido.
    audio: Option<EstadoAudio>,
    transcribiendo: Option<inc::Transcribiendo>,
    /// El parrafo resaltado (el que suena).
    resaltado: Option<usize>,
    /// El renglon del incrustado del ultimo clic derecho.
    del_menu: Option<usize>,
    latiendo: bool,
}

thread_local! {
    static MEMORIA: RefCell<Memoria> = RefCell::new(Memoria::default());
}

/// Se olvida de todo (al cerrar la nota, y entre pruebas).
pub(super) fn olvidar() {
    MEMORIA.with(|m| *m.borrow_mut() = Memoria::default());
}

/// Lo que dice la aplicacion del renglon, con el nombre que lleva en la
/// nota (el texto del enlace, el que tambien lee el movil): ese manda sobre
/// el del fichero.
fn ficha_de(e: &mut Estado, ruta: &str, nombre: &str) -> Option<Ficha> {
    let f = match MEMORIA.with(|m| m.borrow().fichas.get(ruta).cloned()) {
        Some(f) => f,
        None => {
            let f = e.integracion.medios.as_mut().and_then(|m| m.ficha(ruta));
            MEMORIA.with(|m| m.borrow_mut().fichas.insert(ruta.to_string(), f.clone()));
            f
        }
    };
    Some(con_nombre(f?, nombre))
}

fn con_nombre(f: Ficha, nombre: &str) -> Ficha {
    let nombre = nombre.trim();
    if nombre.is_empty() {
        return f;
    }
    match f {
        Ficha::Archivo(a) => Ficha::Archivo(inc::Archivo {
            nombre: nombre.to_string(),
            ..a
        }),
        Ficha::Audio(a) => Ficha::Audio(inc::Audio {
            nombre: nombre.to_string(),
            ..a
        }),
        Ficha::Borrado(_) => Ficha::Borrado(nombre.to_string()),
        otra => otra,
    }
}

/// Los colores del chat sobre el papel de la nota.
fn colores(e: &Estado) -> pin::Colores {
    pin::Colores::del_chat(e.estilos.tema.oscuro, e.estilos.tema.papel)
}

/// Como suena el audio de `ruta` ahora, para su reproductor.
fn como_suena(ruta: &str, con_letra: bool) -> ComoSuena {
    MEMORIA.with(|m| {
        let m = m.borrow();
        ComoSuena {
            estado: m.audio.clone().filter(|a| a.ruta == ruta),
            con_letra,
            pasando: m.transcribiendo.as_ref().filter(|t| t.ruta == ruta && t.hecho.is_none()).map(|t| t.avance),
        }
    })
}

/// El incrustado pintado (de la memoria si nada cambio).
fn pintado(e: &mut Estado, ruta: &str, ficha: Ficha, suena: ComoSuena) -> Option<(Rc<imagenes::Foto>, Zonas)> {
    let clave = Clave {
        ficha,
        suena,
        columna: fotos::columna_px(e) * e.ppp / 96,
        papel: e.estilos.tema.papel,
        oscuro: e.estilos.tema.oscuro,
        ppp: e.ppp,
    };
    if let Some((c, f, z)) = MEMORIA.with(|m| m.borrow().pintados.get(ruta).cloned())
        && c == clave
    {
        return Some((f, z));
    }
    let esc = e.ppp as f32 / 96.0;
    let p = pin::pintar(&clave.ficha, &clave.suena, clave.columna, &colores(e), &e.rotulos.incrustados, esc)?;
    let foto = Rc::new(imagenes::Foto::de_mapa(p.mapa, p.an, p.al));
    MEMORIA.with(|m| m.borrow_mut().pintados.insert(ruta.to_string(), (clave, foto.clone(), p.zonas)));
    Some((foto, p.zonas))
}

/// **Anade los incrustados a lo que se pinta encima** (`fotos::medir`): cada
/// renglon con su tarjeta, burbuja o reproductor, en orden de renglon.
pub(super) fn medir(e: &mut Estado, texto: &str, con: &mut fotos::ConFoto) {
    if e.integracion.medios.is_none() {
        return;
    }
    let letras = inc::letras(texto);
    for r in inc::renglones(texto) {
        let Some(ficha) = ficha_de(e, &r.ruta, &r.nombre) else {
            continue;
        };
        let con_letra = letras.iter().any(|l| l.linea == r.linea && !l.parrafos.is_empty());
        let suena = if r.clase == Clase::Audio { como_suena(&r.ruta, con_letra) } else { ComoSuena::default() };
        if let Some((foto, _)) = pintado(e, &r.ruta, ficha, suena) {
            con.push((r.linea, r.ruta, foto));
        }
    }
    con.sort_by_key(|(n, _, _)| *n);
}

fn esconder(e: &Estado, desde: usize, hasta: usize) {
    let mut f = formato();
    f.Base.dwMask = CFM_HIDDEN;
    f.Base.dwEffects = CFE_HIDDEN;
    elegir(e.edit, desde, hasta);
    poner_formato(e.edit, &f);
}

/// **El formato de los incrustados**, dentro del pintado de la nota: el
/// renglon de cada uno, escondido entero salvo con el cursor (entonces
/// asoma su nombre); y las marcas de tiempo como chapas, con el parrafo que
/// suena resaltado.
pub(super) fn formatear(e: &Estado, texto: &str, ls: &[md_vivo::Linea], con: &fotos::ConFoto, activa: usize, entra: &dyn Fn(usize) -> bool) {
    if e.integracion.medios.is_none() {
        return;
    }
    let pintados: Vec<usize> = inc::renglones(texto)
        .into_iter()
        .filter(|r| con.iter().any(|(n, ruta, _)| *n == r.linea && *ruta == r.ruta))
        .map(|r| r.linea)
        .collect();
    for n in pintados {
        if n != activa
            && entra(n)
            && let Some(l) = ls.get(n)
            && l.hasta > l.desde
        {
            esconder(e, l.desde, l.hasta);
            // Y el renglon, con su salto, casi sin alto: lo que se ve es la
            // tarjeta, que ya lleva su aire; un renglon vacio debajo de cada
            // una las separaba de mas.
            let mut f = formato();
            f.Base.dwMask = CFM_SIZE;
            f.Base.yHeight = 40;
            elegir(e.edit, l.desde, l.hasta + 1);
            poner_formato(e.edit, &f);
        }
    }
    let t = &e.estilos.tema;
    let resaltado = MEMORIA.with(|m| m.borrow().resaltado);
    let u: Vec<u16> = texto.encode_utf16().collect();
    for letra in inc::letras(texto) {
        for (n, _) in letra.parrafos {
            if !entra(n) {
                continue;
            }
            let Some(l) = ls.get(n) else { continue };
            let renglon = String::from_utf16_lossy(&u[l.desde..l.hasta.min(u.len())]);
            let Some(m) = inc::marca(&renglon) else { continue };
            let (abre, cierra) = (l.desde + m.abre, l.desde + m.cierra);
            esconder(e, abre, abre + 1);
            esconder(e, cierra, cierra + 1);
            let mut f = formato();
            f.Base.dwMask = CFM_COLOR | CFM_BOLD | CFM_BACKCOLOR;
            f.Base.dwEffects = CFE_BOLD;
            f.Base.crTextColor = color(t.acento);
            f.crBackColor = color(pin::mezcla(t.acento, t.papel, 0.16));
            elegir(e.edit, abre + 1, cierra);
            poner_formato(e.edit, &f);
            if resaltado == Some(n) && cierra + 1 < l.hasta {
                let mut f = formato();
                f.Base.dwMask = CFM_BACKCOLOR;
                f.crBackColor = color(pin::mezcla(t.acento, t.papel, 0.22));
                elegir(e.edit, cierra + 1, l.hasta);
                poner_formato(e.edit, &f);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Meter

/// Un aviso al usuario; en las pruebas (ventana oculta) solo al registro.
fn avisar(e: &Estado, texto: &str) {
    if texto.trim().is_empty() {
        return;
    }
    if e.oculto {
        tracing::info!(texto, "aviso de la nota");
    } else {
        pixpin_shell::exportar::informar(e.marco, &e.rotulos.sufijo, texto);
    }
}

/// **Mete un renglon que nace pintado**: con el documento congelado se
/// escribe y se le pone el formato (escondido, con su hueco y su tarjeta),
/// y solo entonces se suelta. Nunca se ve el Markdown.
pub(super) fn insertar_pintado(e: &mut Estado, renglon: &str) {
    congelar::congelado(e, congelar::Pintado::Entero, |e| insertar_renglon(e, renglon));
    imagenes::repintar(e.edit);
}

fn nombre_de(r: &Path) -> String {
    r.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// **Mete documentos y audios de fuera**: las fotos van como fotos; lo
/// demas se copia junto a la nota (para que viaje con ella) y entra como
/// su tarjeta o su reproductor. Devuelve cuantos entraron.
pub(super) fn meter_documentos(e: &mut Estado, rutas: &[PathBuf]) -> usize {
    let (fotos_, otros): (Vec<PathBuf>, Vec<PathBuf>) =
        rutas.iter().cloned().partition(|r| pixpin_docs::md_imagen::es_foto(&nombre_de(r)));
    let mut hechas = if fotos_.is_empty() { 0 } else { fotos::meter(e, &fotos_) };
    let mut fallo = false;
    for r in otros.iter().filter(|r| r.is_file()) {
        match (e.adjuntar)(r) {
            Some(md) => {
                insertar_pintado(e, &inc::renglon_de_archivo(&nombre_de(r), &md));
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

/// El primer id de los mensajes en el menu «Del chat».
const ID_MENSAJE: u32 = 0x5000;

/// **«Del chat»**: los mensajes del proyecto en un menu y el elegido, como
/// su burbuja (o su audio con la transcripcion).
fn del_chat(e: &mut Estado) {
    let Some(medios) = e.integracion.medios.as_mut() else {
        return;
    };
    let lista = medios.mensajes();
    if lista.is_empty() {
        let t = e.rotulos.incrustados.sin_mensajes.clone();
        avisar(e, &t);
        return;
    }
    if e.oculto {
        return;
    }
    let (_, b) = seleccion(e.edit);
    let mut p = POINT::default();
    enviar(e.edit, EM_POSFROMCHAR, &mut p as *mut _ as usize, b as isize);
    p.y += RENGLON_PX * e.ppp / 96;
    // SAFETY: el menu se crea y se destruye aqui; las cadenas viven durante
    // cada llamada; ventanas propias.
    let elegido = unsafe {
        let _ = ClientToScreen(e.edit, &mut p);
        let Ok(menu) = CreatePopupMenu() else { return };
        for (i, m) in lista.iter().enumerate() {
            let _ = AppendMenuW(menu, MF_STRING, ID_MENSAJE as usize + i, &HSTRING::from(m.rotulo.as_str()));
        }
        let r = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, p.x, p.y, None, e.marco, None);
        let _ = DestroyMenu(menu);
        r.0 as u32
    };
    if let Some(m) = elegido.checked_sub(ID_MENSAJE).and_then(|i| lista.get(i as usize)) {
        meter_mensaje(e, &m.clave.clone());
    }
}

/// Mete el mensaje `clave` del chat. `false` si la aplicacion no lo da.
pub(super) fn meter_mensaje(e: &mut Estado, clave: &str) -> bool {
    let Some(bloque) = e.integracion.medios.as_mut().and_then(|m| m.insertar_mensaje(clave)) else {
        return false;
    };
    insertar_pintado(e, &bloque);
    true
}

pub(super) fn es_suyo(c: u16) -> bool {
    (C_DOCUMENTO..=C_QUITAR).contains(&c)
}

pub(super) fn comando(e: &mut Estado, c: u16) {
    match c {
        C_DOCUMENTO | C_AUDIO => {
            let rutas = pixpin_shell::elegir::pedir_ficheros(e.marco);
            meter_documentos(e, &rutas);
        }
        C_DEL_CHAT => del_chat(e),
        C_ABRIR => {
            if let Some(n) = MEMORIA.with(|m| m.borrow_mut().del_menu.take())
                && let Some(r) = incrustado_en(e, n)
            {
                abrir(e, &r);
            }
        }
        C_QUITAR => {
            if let Some(n) = MEMORIA.with(|m| m.borrow_mut().del_menu.take()) {
                quitar(e, n);
            }
        }
        _ => {}
    }
}

/// Las entradas de los menus `+`: documento, del chat y audio (si hay
/// aplicacion que los de).
pub(super) fn entradas_del_mas(e: &Estado) -> Vec<Entrada> {
    if e.integracion.medios.is_none() {
        return Vec::new();
    }
    let r = &e.rotulos.incrustados;
    vec![
        entrada(C_DOCUMENTO, Dibujo::Icono(Icono::Documento), &r.documento, ""),
        entrada(C_DEL_CHAT, Dibujo::Icono(Icono::Chat), &r.del_chat, ""),
        entrada(C_AUDIO, Dibujo::Icono(Icono::Audio), &r.audio, ""),
    ]
}

// ---------------------------------------------------------------------------
// Abrir, quitar y el menu

/// La ruta del incrustado del renglon `n`, si lo es.
fn incrustado_en(e: &Estado, n: usize) -> Option<String> {
    let texto = leer(e.edit);
    inc::renglones(&texto).into_iter().find(|r| r.linea == n).map(|r| r.ruta)
}

fn abrir(e: &mut Estado, ruta: &str) {
    fotos::abrir(e, ruta);
}

/// Quita el renglon `n` (con su salto) si es un incrustado.
fn quitar(e: &Estado, n: usize) -> bool {
    if incrustado_en(e, n).is_none() {
        return false;
    }
    let texto = leer(e.edit);
    let ls = md_vivo::lineas(&texto);
    let Some(l) = ls.get(n).copied() else { return false };
    let total = texto.encode_utf16().count();
    let (a, b) = if l.hasta < total { (l.desde, l.hasta + 1) } else { (l.desde.saturating_sub(1), l.hasta) };
    elegir(e.edit, a, b);
    enviar(e.edit, EM_REPLACESEL, 1, ancho_nulo("").as_ptr() as isize);
    true
}

/// Lo del incrustado bajo el raton (o en el renglon del cursor) para el
/// menu del clic derecho.
pub(super) fn entradas_del_menu(e: &mut Estado) -> Vec<Option<(u16, String)>> {
    let mut p = POINT::default();
    // SAFETY: posicion del raton y conversion para una ventana propia.
    unsafe {
        let _ = GetCursorPos(&mut p);
        let _ = ScreenToClient(e.edit, &mut p);
    }
    let bajo = imagenes::tocar(p.x, p.y).and_then(|(i, _)| imagenes::puesta(i)).map(|(n, ..)| n);
    let n = bajo.or_else(|| {
        let texto = leer(e.edit);
        Some(md_vivo::linea_de(&md_vivo::lineas(&texto), seleccion(e.edit).1))
    });
    let Some(n) = n.filter(|n| incrustado_en(e, *n).is_some()) else {
        return Vec::new();
    };
    MEMORIA.with(|m| m.borrow_mut().del_menu = Some(n));
    let r = &e.rotulos.incrustados;
    let es_mensaje = incrustado_en(e, n).is_some_and(|ruta| inc::mensaje_del_enlace(&ruta).is_some());
    vec![
        Some((C_ABRIR, if es_mensaje { r.ir_al_mensaje.clone() } else { r.abrir.clone() })),
        Some((C_QUITAR, r.quitar.clone())),
        None,
    ]
}

// ---------------------------------------------------------------------------
// El raton

fn punto(l: LPARAM) -> (i32, i32) {
    ((l.0 & 0xffff) as i16 as i32, ((l.0 >> 16) & 0xffff) as i16 as i32)
}

/// Pide algo al reproductor y deja el latido en marcha.
fn al_audio(e: &mut Estado, orden: OrdenAudio) {
    let Some(m) = e.integracion.medios.as_mut() else { return };
    let estado = m.audio(orden);
    MEMORIA.with(|me| me.borrow_mut().audio = estado);
    latir(e, true);
    refrescar_audio(e);
}

fn latir(e: &Estado, si: bool) {
    let antes = MEMORIA.with(|m| std::mem::replace(&mut m.borrow_mut().latiendo, si));
    if antes == si || e.oculto {
        return;
    }
    // SAFETY: temporizador de una ventana propia.
    unsafe {
        if si {
            SetTimer(Some(e.marco), T_AUDIO, MS_AUDIO, None);
        } else {
            let _ = KillTimer(Some(e.marco), T_AUDIO);
        }
    }
}

/// El renglon de la letra donde cae `pos`, si es la marca de tiempo de un
/// parrafo de la transcripcion de un audio: su audio y su milisegundo.
pub(super) fn marca_en(texto: &str, pos: usize) -> Option<(String, i64)> {
    let ls = md_vivo::lineas(texto);
    let n = md_vivo::linea_de(&ls, pos);
    let l = ls.get(n)?;
    let u: Vec<u16> = texto.encode_utf16().collect();
    let renglon = String::from_utf16_lossy(&u[l.desde..l.hasta.min(u.len())]);
    let m = inc::marca(&renglon)?;
    if pos < l.desde + m.abre || pos > l.desde + m.cierra + 1 {
        return None;
    }
    let letra = inc::letras(texto).into_iter().find(|x| x.parrafos.iter().any(|(p, _)| *p == n))?;
    Some((letra.ruta, m.ms))
}

/// **El raton sobre los incrustados**: tocar, la barra, la velocidad,
/// pasar a texto, abrir con un clic en la tarjeta, y las marcas de tiempo.
/// `true` si se atendio aqui.
pub(super) fn raton(e: &mut Estado, m: &MSG) -> bool {
    if e.integracion.medios.is_none() || !matches!(m.message, WM_LBUTTONDOWN | WM_LBUTTONDBLCLK) {
        return false;
    }
    let (x, y) = punto(m.lParam);
    let sobre = imagenes::tocar(x, y).and_then(|(i, _)| imagenes::puesta(i));
    let Some((linea, ruta, caja, _)) = sobre.filter(|(_, ruta, ..)| MEMORIA.with(|me| me.borrow().pintados.contains_key(ruta)))
    else {
        // Un clic en una marca de tiempo salta el audio.
        if m.message == WM_LBUTTONDOWN && !pulsada(VK_SHIFT.0) {
            let texto = leer(e.edit);
            let pos = letra_en(e.edit, m.lParam);
            if let Some((ruta, ms)) = marca_en(&texto, pos) {
                al_audio(e, OrdenAudio::Saltar(ruta, ms));
                return true;
            }
        }
        return false;
    };
    cerrar_menu(e);
    let zonas = MEMORIA.with(|me| me.borrow().pintados.get(&ruta).map(|(_, _, z)| *z)).unwrap_or_default();
    let (rx, ry) = (x - caja.left, y - caja.top);
    let elegir_renglon = |e: &Estado| {
        let texto = leer(e.edit);
        if let Some(l) = md_vivo::lineas(&texto).get(linea) {
            elegir(e.edit, l.hasta, l.hasta);
        }
    };
    if m.message == WM_LBUTTONDBLCLK {
        if !inc::es_audio(&ruta) {
            abrir(e, &ruta);
        }
        return true;
    }
    let en = |z: Option<pin::Caja>| z.is_some_and(|c| c.dentro(rx, ry));
    if en(zonas.tocar) {
        al_audio(e, OrdenAudio::Alternar(ruta));
    } else if en(zonas.barra) {
        let b = zonas.barra.unwrap_or_default();
        let f = ((rx - b.x) as f32 / b.an.max(1) as f32).clamp(0.0, 1.0);
        let cargado = MEMORIA.with(|me| me.borrow().audio.as_ref().is_some_and(|a| a.ruta == ruta && a.duracion_ms > 0));
        if !cargado {
            al_audio(e, OrdenAudio::Alternar(ruta.clone()));
        }
        al_audio(e, OrdenAudio::IrA(ruta, f));
    } else if en(zonas.velocidad) {
        al_audio(e, OrdenAudio::Velocidad);
    } else if en(zonas.texto) {
        pasar_a_texto(e, &ruta);
    } else if zonas.tarjeta.dentro(rx, ry) && !inc::es_audio(&ruta) {
        // Un clic en la tarjeta abre lo que es, como en el chat; el
        // renglon queda elegido para poder quitarlo con el teclado.
        elegir_renglon(e);
        abrir(e, &ruta);
    } else {
        elegir_renglon(e);
    }
    true
}

fn pasar_a_texto(e: &mut Estado, ruta: &str) {
    let Some(m) = e.integracion.medios.as_mut() else { return };
    match m.pasar_a_texto(ruta) {
        Ok(()) => {
            MEMORIA.with(|me| {
                me.borrow_mut().transcribiendo = Some(inc::Transcribiendo {
                    ruta: ruta.to_string(),
                    ..Default::default()
                })
            });
            latir(e, true);
            refrescar_audio(e);
        }
        Err(aviso) => avisar(e, &aviso),
    }
}

// ---------------------------------------------------------------------------
// El latido

/// Vuelve a pintar el reproductor de cada audio (solo cambia el que suena
/// o se transcribe: los demas salen de la memoria) y lo pone encima.
fn refrescar_audio(e: &mut Estado) {
    let texto = leer(e.edit);
    let letras = inc::letras(&texto);
    let mut cambiadas = Vec::new();
    let renglones = inc::renglones(&texto);
    for l in &letras {
        let nombre = renglones.iter().find(|r| r.linea == l.linea).map(|r| r.nombre.clone()).unwrap_or_default();
        let Some(ficha) = ficha_de(e, &l.ruta, &nombre) else { continue };
        let suena = como_suena(&l.ruta, !l.parrafos.is_empty());
        if let Some((foto, _)) = pintado(e, &l.ruta, ficha, suena) {
            cambiadas.push((l.ruta.clone(), foto));
        }
    }
    let mut otro_alto = false;
    imagenes::PUESTAS.with(|p| {
        for pu in p.borrow_mut().iter_mut() {
            if let Some((_, f)) = cambiadas.iter().find(|(r, _)| *r == pu.ruta)
                && !Rc::ptr_eq(&pu.foto, f)
            {
                otro_alto |= pu.foto.alto != f.alto;
                pu.foto = f.clone();
            }
        }
    });
    // El parrafo que suena, resaltado.
    let ahora = MEMORIA.with(|m| {
        let m = m.borrow();
        let a = m.audio.as_ref().filter(|a| a.sonando || a.posicion_ms > 0)?;
        let l = letras.iter().find(|l| l.ruta == a.ruta)?;
        inc::parrafo_que_suena(&l.parrafos, a.posicion_ms)
    });
    let antes = MEMORIA.with(|m| std::mem::replace(&mut m.borrow_mut().resaltado, ahora));
    if otro_alto {
        // La pastilla de pasar a texto aparecio o se fue: otro hueco.
        pintar(e, None);
    } else if antes != ahora {
        let ls: Vec<usize> = [antes, ahora].into_iter().flatten().collect();
        pintar(e, Some(&ls));
    }
    imagenes::repintar(e.edit);
}

/// **El latido**: como va el reproductor y la transcripcion. Cuando acaba
/// una transcripcion, su texto entra debajo de su audio (si aun no tiene).
pub(super) fn latido(e: &mut Estado) {
    let Some(m) = e.integracion.medios.as_mut() else {
        latir(e, false);
        return;
    };
    let audio = m.audio(OrdenAudio::Mirar);
    let tr = m.transcribiendo();
    let (cambio, sigue) = MEMORIA.with(|me| {
        let mut me = me.borrow_mut();
        let cambio = me.audio != audio || me.transcribiendo != tr;
        let sigue = audio.as_ref().is_some_and(|a| a.sonando) || tr.as_ref().is_some_and(|t| t.hecho.is_none());
        me.audio = audio;
        me.transcribiendo = tr.clone();
        (cambio, sigue)
    });
    if let Some(t) = tr.filter(|t| t.hecho.is_some()) {
        MEMORIA.with(|me| me.borrow_mut().transcribiendo = None);
        match t.hecho {
            Some(Ok(letra)) => {
                poner_letra(e, &t.ruta, &letra);
            }
            Some(Err(aviso)) => avisar(e, &aviso),
            None => {}
        }
    }
    if cambio {
        refrescar_audio(e);
    }
    if !sigue {
        latir(e, false);
    }
}

/// Mete la transcripcion `letra` debajo del audio `ruta`, si ese audio
/// sigue en la nota y aun no la lleva.
pub(super) fn poner_letra(e: &mut Estado, ruta: &str, letra: &str) -> bool {
    let texto = leer(e.edit);
    let Some(l) = inc::letras(&texto).into_iter().find(|l| l.ruta == ruta && l.parrafos.is_empty()) else {
        return false;
    };
    let bloque = inc::bloque_de_audio("x", "x", Some(letra));
    let Some((_, cuerpo)) = bloque.split_once("\n\n") else {
        return false;
    };
    let ls = md_vivo::lineas(&texto);
    let Some(hasta) = ls.get(l.linea).map(|x| x.hasta) else { return false };
    let puesto = format!("\r\r{}", cuerpo.replace('\n', "\r"));
    congelar::congelado(e, congelar::Pintado::Entero, |e| {
        let sel = seleccion(e.edit);
        elegir(e.edit, hasta, hasta);
        enviar(e.edit, EM_REPLACESEL, 1, ancho_nulo(&puesto).as_ptr() as isize);
        elegir(e.edit, sel.0, sel.1);
    });
    imagenes::repintar(e.edit);
    true
}

/// Cada poco (con las paginas vivas): si lo que la aplicacion dice de un
/// incrustado cambio (el mensaje se edito o se borro, el fichero llego),
/// se vuelve a pintar.
pub(super) fn vigilar(e: &mut Estado) {
    let rutas: Vec<String> = MEMORIA.with(|m| m.borrow().fichas.keys().cloned().collect());
    let Some(medios) = e.integracion.medios.as_mut() else { return };
    let mut cambio = false;
    for r in rutas {
        let nueva = medios.ficha(&r);
        MEMORIA.with(|m| {
            let mut m = m.borrow_mut();
            if m.fichas.get(&r) != Some(&nueva) {
                m.fichas.insert(r.clone(), nueva);
                cambio = true;
            }
        });
    }
    if cambio {
        pintar(e, None);
        imagenes::repintar(e.edit);
    }
}

#[cfg(test)]
mod pruebas;
