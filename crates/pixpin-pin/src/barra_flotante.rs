//! La ventana de la barra del pin (rediseno v2): la de `barra.rs`, en
//! pantalla.
//!
//! Una sola para todos los pines (solo hay un raton): se crea la primera vez
//! y despues se esconde y se vuelve a ensenar junto al pin que tenga el
//! raton encima. No se activa nunca, como la paleta: el teclado se queda en
//! el pin.
//!
//! No sabe nada del pin: pinta los `DatosBarra` que le dan y cada clic lo
//! devuelve como `EventoBarra` a `ventana::accion_de_barra`, que es quien
//! sabe que hacer con el.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Once;

use pixpin_geom::Rect;
use pixpin_render::{Color, MotorRender, Pintor, RectF, Superficie};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Gdi::ValidateRect;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::barra::{self, AccionBarra, Barra, Pieza, Propio, Rotulo, TipoBarra};
use crate::menu::TextosPin;

/// Lo que ensena la barra de un video.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DatosVideo {
    pub reproduciendo: bool,
    pub t: f64,
    pub duracion: Option<f64>,
    pub volumen: f64,
    pub silenciado: bool,
}

/// Todo lo que la barra pinta de un pin.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DatosBarra {
    pub tipo: TipoBarra,
    pub zoom_por_cien: u32,
    /// La pagina que se ve (desde 0) y cuantas hay.
    pub pagina: Option<(u32, u32)>,
    pub video: Option<DatosVideo>,
    /// Si el pin en vivo esta en modo clic: su boton se ve encendido.
    pub remoto: bool,
    /// Si el pin en vivo esta en pausa (quieto): su chapa se apaga y el
    /// boton ofrece reanudar.
    pub en_pausa: bool,
}

/// Lo que la barra devuelve al pin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum EventoBarra {
    Pulsado {
        accion: AccionBarra,
        /// Debajo del boton, en pantalla: donde se abre el menu de «Mas».
        bajo: (i32, i32),
    },
    Saltar {
        fraccion: f64,
        aproximado: bool,
    },
    Volumen(i32),
}

struct Interno {
    motor: Rc<MotorRender>,
    superficie: Superficie,
    dueno: HWND,
    escala: f32,
    disposicion: Barra,
    datos: DatosBarra,
    textos: TextosPin,
    tema_claro: bool,
    encima: Option<AccionBarra>,
    saltando: bool,
    /// Esquina de la barra, en pantalla.
    origen: (i32, i32),
}

static REGISTRO: Once = Once::new();

thread_local! {
    static VENTANA: Cell<Option<HWND>> = const { Cell::new(None) };
}

fn interno_de<'a>(hwnd: HWND) -> Option<&'a mut Interno> {
    // SAFETY: el puntero lo pone `mostrar` y solo WM_NCDESTROY lo retira;
    // entre ambos es un Box valido. Todo ocurre en el hilo de interfaz.
    unsafe {
        let crudo = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Interno;
        crudo.as_mut()
    }
}

fn ventana() -> Option<HWND> {
    VENTANA.with(|v| v.get())
}

fn registrar_clase() {
    // SAFETY: registro unico (Once) de una clase con WndProc propio.
    unsafe {
        let clase = WNDCLASSW {
            lpfnWndProc: Some(procedimiento),
            hInstance: GetModuleHandleW(None).expect("modulo propio").into(),
            lpszClassName: w!("PixPinBarraPin"),
            ..Default::default()
        };
        RegisterClassW(&clase);
    }
}

/// El tamano de la ventana: la barra, y debajo el aviso si hay un boton
/// bajo el raton. Crece y encoge con el aviso para no tapar el pin cuando no
/// hace falta.
fn tamano(i: &Interno) -> (u32, u32) {
    let aviso = if i
        .encima
        .is_some_and(|a| texto_de(&i.textos, a, &i.datos).is_some())
    {
        barra::AVISO_LOGICO * i.escala + 4.0 * i.escala
    } else {
        0.0
    };
    // El aviso puede ser mas ancho que una barra corta.
    let ancho = i
        .disposicion
        .ancho
        .max(if aviso > 0.0 { 260.0 * i.escala } else { 0.0 });
    (
        ancho.ceil() as u32,
        (i.disposicion.alto + aviso).ceil() as u32,
    )
}

fn recolocar_ventana(hwnd: HWND, i: &Interno) {
    let (ancho, alto) = tamano(i);
    if let Err(e) = i.superficie.redimensionar(ancho, alto) {
        tracing::warn!(?e, "no se pudo redimensionar la barra del pin");
        return;
    }
    // SAFETY: mover y ensenar una ventana propia sin activarla.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            i.origen.0,
            i.origen.1,
            ancho as i32,
            alto as i32,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
    pintar(i);
}

/// Ensena la barra del pin `dueno`, cuyo contenido ocupa `contenido` en un
/// monitor de area de trabajo `trabajo`.
#[allow(clippy::too_many_arguments)] // todo lo que la barra necesita al aparecer
pub(crate) fn mostrar(
    d3d: &ID3D11Device,
    motor: &Rc<MotorRender>,
    dueno: HWND,
    contenido: Rect,
    trabajo: Rect,
    escala_por_cien: u32,
    datos: DatosBarra,
    textos: &TextosPin,
    tema_claro: bool,
) {
    let escala = escala_por_cien as f32 / 100.0;
    let disposicion = barra::disponer(datos.tipo, escala);
    let sep = (barra::SEPARACION_LOGICA * escala) as i32;
    let (x, y, _) = barra::colocar(
        contenido,
        trabajo,
        disposicion.ancho.ceil() as u32,
        disposicion.alto.ceil() as u32,
        sep,
    );
    let hwnd = match ventana() {
        Some(h) => h,
        None => {
            REGISTRO.call_once(registrar_clase);
            // SAFETY: clase registrada; estilos documentados; se crea una
            // vez por hilo y no se destruye: se esconde.
            let h = unsafe {
                CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    w!("PixPinBarraPin"),
                    w!(""),
                    WS_POPUP,
                    x,
                    y,
                    disposicion.ancho.ceil() as i32,
                    disposicion.alto.ceil() as i32,
                    None,
                    None,
                    GetModuleHandleW(None).ok().map(|m| m.into()),
                    None,
                )
            };
            let Ok(h) = h else {
                tracing::warn!("no se pudo crear la barra del pin");
                return;
            };
            let superficie = match Superficie::nueva(
                motor,
                d3d,
                h,
                disposicion.ancho.ceil() as u32,
                disposicion.alto.ceil() as u32,
            ) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(?e, "sin superficie para la barra del pin");
                    // SAFETY: ventana propia recien creada.
                    unsafe {
                        let _ = DestroyWindow(h);
                    }
                    return;
                }
            };
            let interno = Box::new(Interno {
                motor: Rc::clone(motor),
                superficie,
                dueno,
                escala,
                disposicion: disposicion.clone(),
                datos: datos.clone(),
                textos: textos.clone(),
                tema_claro,
                encima: None,
                saltando: false,
                origen: (x, y),
            });
            // SAFETY: el Box se cede al USERDATA y se recupera una vez en
            // WM_NCDESTROY.
            unsafe {
                SetWindowLongPtrW(h, GWLP_USERDATA, Box::into_raw(interno) as isize);
            }
            VENTANA.with(|v| v.set(Some(h)));
            h
        }
    };
    let Some(i) = interno_de(hwnd) else { return };
    if i.dueno != dueno {
        i.encima = None;
        i.saltando = false;
    }
    i.dueno = dueno;
    i.escala = escala;
    i.disposicion = disposicion;
    i.datos = datos;
    i.textos = textos.clone();
    i.tema_claro = tema_claro;
    i.origen = (x, y);
    recolocar_ventana(hwnd, i);
}

/// Repinta con datos nuevos si la barra es de `dueno` y se ve.
pub(crate) fn actualizar(dueno: HWND, datos: DatosBarra) {
    let Some(h) = ventana() else { return };
    let Some(i) = interno_de(h) else { return };
    if i.dueno != dueno || !visible(h) || i.datos == datos {
        return;
    }
    let mismo_tipo = i.datos.tipo == datos.tipo;
    i.datos = datos;
    if mismo_tipo {
        pintar(i);
    } else {
        i.disposicion = barra::disponer(i.datos.tipo, i.escala);
        recolocar_ventana(h, i);
    }
}

/// La vuelve a poner junto al pin, que se ha movido o cambiado de tamano.
pub(crate) fn recolocar(dueno: HWND, contenido: Rect, trabajo: Rect) {
    let Some(h) = ventana() else { return };
    let Some(i) = interno_de(h) else { return };
    if i.dueno != dueno || !visible(h) {
        return;
    }
    let sep = (barra::SEPARACION_LOGICA * i.escala) as i32;
    let (x, y, _) = barra::colocar(
        contenido,
        trabajo,
        i.disposicion.ancho.ceil() as u32,
        i.disposicion.alto.ceil() as u32,
        sep,
    );
    if (x, y) != i.origen {
        i.origen = (x, y);
        recolocar_ventana(h, i);
    }
}

fn visible(h: HWND) -> bool {
    // SAFETY: consulta pura sobre una ventana propia.
    unsafe { IsWindowVisible(h) }.as_bool()
}

/// La destruye (si existe): su superficie es del dispositivo grafico, y al
/// perderse este la barra renace con el nuevo en el siguiente `mostrar`.
pub(crate) fn soltar() {
    if let Some(h) = ventana() {
        // SAFETY: ventana propia de este hilo; WM_NCDESTROY suelta su
        // estado y borra `VENTANA`.
        unsafe {
            let _ = DestroyWindow(h);
        }
        VENTANA.with(|v| v.set(None));
    }
}

/// Si la barra se ve ahora para este pin.
pub(crate) fn es_de(dueno: HWND) -> bool {
    ventana().is_some_and(|h| visible(h) && interno_de(h).is_some_and(|i| i.dueno == dueno))
}

/// La esconde, si es de `dueno`.
pub(crate) fn esconder(dueno: HWND) {
    let Some(h) = ventana() else { return };
    if interno_de(h).is_some_and(|i| i.dueno == dueno) {
        if let Some(i) = interno_de(h) {
            i.encima = None;
            i.saltando = false;
        }
        // SAFETY: esconder una ventana propia.
        unsafe {
            let _ = ShowWindow(h, SW_HIDE);
        }
    }
}

/// Si el cursor (pantalla) esta encima de la barra.
pub(crate) fn contiene(x: i32, y: i32) -> bool {
    let Some(h) = ventana() else { return false };
    if !visible(h) {
        return false;
    }
    let mut r = RECT::default();
    // SAFETY: GetWindowRect sobre una ventana propia.
    unsafe {
        let _ = GetWindowRect(h, &mut r);
    }
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

/// Si el cursor esta en el hueco entre el pin y su barra (o en cualquiera
/// de los dos): pasar del uno a la otra cruza ese hueco, y la barra no puede
/// esconderse a medio camino.
pub(crate) fn en_el_paso(pin: Rect, x: i32, y: i32) -> bool {
    let Some(h) = ventana() else { return false };
    if !visible(h) {
        return false;
    }
    let mut r = RECT::default();
    // SAFETY: GetWindowRect sobre una ventana propia.
    unsafe {
        let _ = GetWindowRect(h, &mut r);
    }
    let (l, t) = (r.left.min(pin.x), r.top.min(pin.y));
    let (d, b) = (r.right.max(pin.derecha()), r.bottom.max(pin.abajo()));
    x >= l && x < d && y >= t && y < b
}

/// Si la barra esta ocupada (arrastrando la linea de tiempo): mientras, no
/// se esconde aunque el raton se salga.
pub(crate) fn ocupada() -> bool {
    ventana().and_then(interno_de).is_some_and(|i| i.saltando)
}

/// El nombre y la tecla de un boton, para su aviso. `None` si no lleva.
fn texto_de(t: &TextosPin, a: AccionBarra, datos: &DatosBarra) -> Option<(String, Option<String>)> {
    let v = &t.v2;
    let (nombre, tecla): (&str, Option<&str>) = match a {
        AccionBarra::Alejar => (&v.alejar, Some(&v.tecla_rueda)),
        AccionBarra::Acercar => (&v.acercar, Some(&v.tecla_rueda)),
        AccionBarra::PaginaAnterior => (&t.pagina_anterior, Some(&v.tecla_re_pag)),
        AccionBarra::PaginaSiguiente => (&t.pagina_siguiente, Some(&v.tecla_av_pag)),
        AccionBarra::PinearPagina => (&v.pinear_pagina, None),
        AccionBarra::Reproducir => (
            if datos.video.is_some_and(|d| d.reproduciendo)
                || (datos.tipo.propio == Propio::Vivo && !datos.en_pausa)
            {
                &t.pausar
            } else {
                &t.reproducir
            },
            Some(&v.tecla_espacio),
        ),
        AccionBarra::Saltar => return None,
        AccionBarra::Sonido => (&t.sonido, Some("M")),
        AccionBarra::Manejar => (
            if datos.remoto {
                &t.dejar_de_manejar
            } else {
                &t.manejar
            },
            None,
        ),
        AccionBarra::Congelar => (&t.congelar, None),
        AccionBarra::Abrir => (&v.abrir, None),
        AccionBarra::Copiar => (&t.copiar, Some("Ctrl C")),
        AccionBarra::Anotar => (&v.anotar, Some("A")),
        AccionBarra::Mas => (&v.mas, None),
        AccionBarra::Cerrar => (&v.cerrar_pin, Some("Esc")),
    };
    if nombre.is_empty() {
        return None;
    }
    Some((
        nombre.to_string(),
        tecla.filter(|s| !s.is_empty()).map(str::to_string),
    ))
}

const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a,
    }
}

fn pintar(i: &Interno) {
    let Ok(destino) = i.superficie.empezar(&i.motor) else {
        return;
    };
    let _ = i.motor.dibujar(&destino, |p| {
        p.limpiar_transparente();
        pintar_barra(p, i);
    });
    let _ = i.superficie.presentar();
}

/// Tres puntos en fila: «Mas».
fn tres_puntos(p: &Pintor, r: RectF, color: Color, e: f32) {
    let cy = r.y + r.alto / 2.0;
    let cx = r.x + r.ancho / 2.0;
    for dx in [-7.0, 0.0, 7.0] {
        p.circulo((cx + dx * e, cy), 1.9 * e, color);
    }
}

fn pintar_barra(p: &Pintor, i: &Interno) {
    use pixpin_render::icono::material::{
        CLOSE, CONTENT_COPY, EDIT, OPEN_IN_NEW, PAUSE, PLAY_ARROW, PUSH_PIN, VOLUME_UP,
    };
    let e = i.escala;
    let claro = i.tema_claro;
    let fondo = if claro {
        rgba(0xF9, 0xF9, 0xFB, 0.98)
    } else {
        rgba(0x1E, 0x1E, 0x20, 0.96)
    };
    let borde = if claro {
        rgba(0, 0, 0, 0.12)
    } else {
        rgba(255, 255, 255, 0.12)
    };
    let tinta = if claro {
        rgba(0x1C, 0x1C, 0x1E, 1.0)
    } else {
        rgba(0xF5, 0xF5, 0xF7, 1.0)
    };
    let tenue = if claro {
        rgba(0x6E, 0x6E, 0x73, 1.0)
    } else {
        rgba(0xC7, 0xC7, 0xCC, 1.0)
    };
    let hover = if claro {
        rgba(0, 0, 0, 0.08)
    } else {
        rgba(255, 255, 255, 0.14)
    };
    let azul = rgba(0x00, 0x60, 0xDF, 1.0);
    let b = &i.disposicion;
    let marco = RectF {
        x: 0.5,
        y: 0.5,
        ancho: b.ancho - 1.0,
        alto: b.alto - 1.0,
    };
    p.rellenar_redondeado(marco, 12.0 * e, borde);
    p.rellenar_redondeado(
        RectF {
            x: 1.0 * e,
            y: 1.0 * e,
            ancho: b.ancho - 2.0 * e,
            alto: b.alto - 2.0 * e,
        },
        11.0 * e,
        fondo,
    );
    let icono_en = |r: RectF| RectF {
        x: r.x + (r.ancho - 18.0 * e) / 2.0,
        y: r.y + (r.alto - 18.0 * e) / 2.0,
        ancho: 18.0 * e,
        alto: 18.0 * e,
    };
    let tam = 13.0 * e;
    let texto_centrado = |texto: &str, r: RectF, color: Color| {
        let (w, h) = p.medir_texto(texto, tam);
        p.texto_linea(
            texto,
            r.x + ((r.ancho - w) / 2.0).max(0.0),
            r.y + (r.alto - h) / 2.0,
            tam,
            r.ancho,
            color,
        );
    };
    let video = i.datos.video;
    for el in &b.elementos {
        let r = el.rect;
        match el.pieza {
            Pieza::Separador => {
                p.rellenar(
                    RectF {
                        x: r.x + r.ancho / 2.0 - 0.5 * e,
                        y: r.y,
                        ancho: 1.0 * e,
                        alto: r.alto,
                    },
                    borde,
                );
            }
            Pieza::Rotulo(rotulo) => match rotulo {
                Rotulo::Zoom => texto_centrado(&format!("{} %", i.datos.zoom_por_cien), r, tinta),
                Rotulo::Pagina => {
                    let t = match i.datos.pagina {
                        Some((pag, total)) => format!("{} / {}", pag + 1, total),
                        None => "—".into(),
                    };
                    texto_centrado(&t, r, tinta)
                }
                Rotulo::Tiempo => {
                    let t = video.map_or(0.0, |v| v.t);
                    texto_centrado(&crate::mandos_video::formato_tiempo(t), r, tenue)
                }
                Rotulo::Duracion => {
                    let d = video.and_then(|v| v.duracion);
                    let t = d.map_or("—".to_string(), crate::mandos_video::formato_tiempo);
                    texto_centrado(&t, r, tenue)
                }
                Rotulo::EnVivo => {
                    let chapa = RectF {
                        x: r.x + 2.0 * e,
                        y: r.y + (r.alto - 28.0 * e) / 2.0,
                        ancho: r.ancho - 4.0 * e,
                        alto: 28.0 * e,
                    };
                    // Roja en vivo; gris en pausa, que es cuando el pin
                    // ya no sigue a su zona.
                    let fondo = if i.datos.en_pausa {
                        rgba(0x6B, 0x6B, 0x6B, 1.0)
                    } else {
                        rgba(0xC9, 0x34, 0x2B, 1.0)
                    };
                    p.rellenar_redondeado(chapa, 7.0 * e, fondo);
                    p.circulo(
                        (chapa.x + 13.0 * e, chapa.y + chapa.alto / 2.0),
                        3.5 * e,
                        Color::BLANCO,
                    );
                    let t = &i.textos.v2.en_vivo;
                    let tam = 12.0 * e;
                    let (_, h) = p.medir_texto(t, tam);
                    p.texto_linea(
                        t,
                        chapa.x + 22.0 * e,
                        chapa.y + (chapa.alto - h) / 2.0,
                        tam,
                        chapa.ancho - 24.0 * e,
                        Color::BLANCO,
                    );
                }
            },
            Pieza::Boton(a) => {
                let encima = i.encima == Some(a);
                let primario =
                    a == AccionBarra::Reproducir || (a == AccionBarra::Manejar && i.datos.remoto);
                if primario {
                    p.rellenar_redondeado(r, 8.0 * e, azul);
                } else if encima && a != AccionBarra::Saltar {
                    p.rellenar_redondeado(r, 8.0 * e, hover);
                }
                let color = if primario { Color::BLANCO } else { tinta };
                match a {
                    AccionBarra::Alejar | AccionBarra::Acercar => {
                        let (cx, cy) = (r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
                        let l = 7.0 * e;
                        p.linea((cx - l, cy), (cx + l, cy), 2.0 * e, color);
                        if a == AccionBarra::Acercar {
                            p.linea((cx, cy - l), (cx, cy + l), 2.0 * e, color);
                        }
                    }
                    AccionBarra::PaginaAnterior | AccionBarra::PaginaSiguiente => {
                        let (cx, cy) = (r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
                        let s = if a == AccionBarra::PaginaAnterior {
                            1.0
                        } else {
                            -1.0
                        };
                        p.polilinea(
                            &[
                                (cx + 3.0 * e * s, cy - 6.0 * e),
                                (cx - 3.0 * e * s, cy),
                                (cx + 3.0 * e * s, cy + 6.0 * e),
                            ],
                            2.0 * e,
                            color,
                        );
                    }
                    AccionBarra::PinearPagina => p.icono(&PUSH_PIN, icono_en(r), color),
                    AccionBarra::Reproducir => {
                        let jugando = video.is_some_and(|v| v.reproduciendo)
                            || (i.datos.tipo.propio == Propio::Vivo && !i.datos.en_pausa);
                        p.icono(
                            if jugando { &PAUSE } else { &PLAY_ARROW },
                            icono_en(r),
                            color,
                        );
                    }
                    AccionBarra::Saltar => {
                        let y = r.y + r.alto / 2.0;
                        let m = r.ancho.min(16.0) / 2.0;
                        let (x0, x1) = (r.x + m, r.x + r.ancho - m);
                        p.linea((x0, y), (x1, y), 4.0 * e, Color { a: 0.25, ..tinta });
                        if let Some(v) = video
                            && let Some(d) = v.duracion
                            && d > 0.0
                        {
                            let x = x0 + (x1 - x0) * (v.t / d).clamp(0.0, 1.0) as f32;
                            p.linea((x0, y), (x, y), 4.0 * e, tinta);
                            p.circulo((x, y), 7.0 * e, tinta);
                        }
                    }
                    AccionBarra::Sonido => {
                        let caja = icono_en(r);
                        p.icono(&VOLUME_UP, caja, color);
                        if video.is_some_and(|v| v.silenciado || v.volumen <= 0.0) {
                            p.linea(
                                (caja.x, caja.y),
                                (caja.x + caja.ancho, caja.y + caja.alto),
                                2.0 * e,
                                color,
                            );
                        }
                    }
                    AccionBarra::Manejar => {
                        // La flecha del raton, a mano: no hay icono de puntero.
                        let (x, y) = (r.x + r.ancho / 2.0 - 5.0 * e, r.y + r.alto / 2.0 - 9.0 * e);
                        let punta = [
                            (0.0, 0.0),
                            (0.0, 15.0),
                            (4.0, 11.5),
                            (7.0, 18.0),
                            (9.5, 17.0),
                            (6.5, 10.5),
                            (11.5, 10.5),
                            (0.0, 0.0),
                        ]
                        .map(|(dx, dy)| (x + dx * e, y + dy * e));
                        p.polilinea(&punta, 1.8 * e, color);
                    }
                    AccionBarra::Congelar => {
                        // Un copo: tres rayas por el centro.
                        let (cx, cy) = (r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
                        let l = 8.0 * e;
                        for ang in [90.0f32, 30.0, -30.0] {
                            let (s, c) = ang.to_radians().sin_cos();
                            p.linea(
                                (cx - c * l, cy - s * l),
                                (cx + c * l, cy + s * l),
                                2.0 * e,
                                color,
                            );
                        }
                    }
                    AccionBarra::Abrir => p.icono(&OPEN_IN_NEW, icono_en(r), color),
                    AccionBarra::Copiar => p.icono(&CONTENT_COPY, icono_en(r), color),
                    AccionBarra::Anotar => p.icono(&EDIT, icono_en(r), color),
                    AccionBarra::Mas => tres_puntos(p, r, color, e),
                    AccionBarra::Cerrar => p.icono(&CLOSE, icono_en(r), color),
                }
            }
        }
    }

    // El aviso del boton bajo el raton: su nombre y su tecla.
    if let Some(a) = i.encima
        && let Some((nombre, tecla)) = texto_de(&i.textos, a, &i.datos)
        && let Some(r) = b.rect_de(a)
    {
        let tam = 13.0 * e;
        let (wn, hn) = p.medir_texto(&nombre, tam);
        let chapa_tam = 11.0 * e;
        let wt = tecla
            .as_ref()
            .map(|t| p.medir_texto(t, chapa_tam).0 + 12.0 * e)
            .unwrap_or(0.0);
        let ancho = 20.0 * e + wn + if wt > 0.0 { 8.0 * e + wt } else { 0.0 };
        let alto = barra::AVISO_LOGICO * e - 4.0 * e;
        let (total, _) = tamano(i);
        let x = (r.x + r.ancho / 2.0 - ancho / 2.0).clamp(0.0, (total as f32 - ancho).max(0.0));
        let y = b.alto + 4.0 * e;
        let caja = RectF { x, y, ancho, alto };
        p.rellenar_redondeado(caja, 8.0 * e, rgba(0x3A, 0x3A, 0x3C, 1.0));
        p.texto_linea(
            &nombre,
            x + 10.0 * e,
            y + (alto - hn) / 2.0,
            tam,
            wn + 2.0,
            rgba(0xF5, 0xF5, 0xF7, 1.0),
        );
        if let Some(t) = tecla {
            let (_, ht) = p.medir_texto(&t, chapa_tam);
            let chapa = RectF {
                x: x + 18.0 * e + wn,
                y: y + (alto - 20.0 * e) / 2.0,
                ancho: wt,
                alto: 20.0 * e,
            };
            p.rellenar_redondeado(chapa, 5.0 * e, rgba(255, 255, 255, 0.12));
            p.texto_linea(
                &t,
                chapa.x + 6.0 * e,
                chapa.y + (chapa.alto - ht) / 2.0,
                chapa_tam,
                wt,
                rgba(0xD1, 0xD1, 0xD6, 1.0),
            );
        }
    }
}

/// Manda un evento al pin dueno. Sin prestamos vivos: el pin puede abrir un
/// menu modal, que bombea mensajes y vuelve a entrar aqui.
fn avisar(dueno: HWND, evento: EventoBarra) {
    crate::ventana::accion_de_barra(dueno, evento);
}

fn local(lparam: LPARAM) -> (f32, f32) {
    (
        (lparam.0 & 0xFFFF) as i16 as f32,
        ((lparam.0 >> 16) & 0xFFFF) as i16 as f32,
    )
}

extern "system" fn procedimiento(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    const WM_RATON_FUERA: u32 = 0x02A3;
    match mensaje {
        // Ni el clic la activa: el teclado sigue en el pin.
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_MOUSEMOVE => {
            let Some(i) = interno_de(hwnd) else {
                return LRESULT(0);
            };
            let (x, y) = local(lparam);
            if i.saltando {
                let (dueno, f) = (i.dueno, i.disposicion.fraccion(x));
                avisar(
                    dueno,
                    EventoBarra::Saltar {
                        fraccion: f,
                        aproximado: true,
                    },
                );
                return LRESULT(0);
            }
            let encima = i.disposicion.accion_en(x, y);
            if encima != i.encima {
                let antes = tamano(i);
                i.encima = encima;
                if tamano(i) != antes {
                    recolocar_ventana(hwnd, i);
                } else {
                    pintar(i);
                }
                let mut seguir = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                // SAFETY: estructura completa sobre la ventana propia.
                unsafe {
                    let _ = TrackMouseEvent(&mut seguir);
                }
            }
            LRESULT(0)
        }
        m if m == WM_RATON_FUERA => {
            if let Some(i) = interno_de(hwnd)
                && i.encima.is_some()
            {
                i.encima = None;
                recolocar_ventana(hwnd, i);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let Some(i) = interno_de(hwnd) else {
                return LRESULT(0);
            };
            let (x, y) = local(lparam);
            let Some(accion) = i.disposicion.accion_en(x, y) else {
                return LRESULT(0);
            };
            let dueno = i.dueno;
            if accion == AccionBarra::Saltar {
                i.saltando = true;
                let f = i.disposicion.fraccion(x);
                // SAFETY: captura sobre la ventana propia; se suelta al
                // soltar el boton.
                unsafe { SetCapture(hwnd) };
                avisar(
                    dueno,
                    EventoBarra::Saltar {
                        fraccion: f,
                        aproximado: true,
                    },
                );
                return LRESULT(0);
            }
            let r = i.disposicion.rect_de(accion).unwrap_or(RectF {
                x,
                y,
                ancho: 0.0,
                alto: 0.0,
            });
            let bajo = (i.origen.0 + r.x as i32, i.origen.1 + (r.y + r.alto) as i32);
            avisar(dueno, EventoBarra::Pulsado { accion, bajo });
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if let Some(i) = interno_de(hwnd)
                && i.saltando
            {
                i.saltando = false;
                let (x, _) = local(lparam);
                let (dueno, f) = (i.dueno, i.disposicion.fraccion(x));
                // SAFETY: suelta la captura tomada al pulsar.
                unsafe {
                    let _ = ReleaseCapture();
                }
                avisar(
                    dueno,
                    EventoBarra::Saltar {
                        fraccion: f,
                        aproximado: false,
                    },
                );
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            // La rueda encima del altavoz es el volumen, como en cualquier
            // reproductor. En el resto de la barra no hace nada.
            if let Some(i) = interno_de(hwnd) {
                let mut r = RECT::default();
                // SAFETY: GetWindowRect sobre la ventana propia.
                unsafe {
                    let _ = GetWindowRect(hwnd, &mut r);
                }
                let (px, py) = local(lparam);
                let (x, y) = (px - r.left as f32, py - r.top as f32);
                if i.disposicion.accion_en(x, y) == Some(AccionBarra::Sonido) {
                    let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    let dueno = i.dueno;
                    avisar(dueno, EventoBarra::Volumen(if delta > 0 { 1 } else { -1 }));
                }
            }
            LRESULT(0)
        }
        WM_PAINT => {
            // SAFETY: ValidateRect marca la ventana como pintada; el dibujo
            // real lo hace el swapchain.
            unsafe {
                let _ = ValidateRect(Some(hwnd), None);
            }
            if let Some(i) = interno_de(hwnd) {
                pintar(i);
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            VENTANA.with(|v| v.set(None));
            // SAFETY: recupera el Box cedido en `mostrar` una sola vez.
            unsafe {
                let crudo = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut Interno;
                if !crudo.is_null() {
                    drop(Box::from_raw(crudo));
                }
            }
            LRESULT(0)
        }
        // NUNCA PostQuitMessage.
        // SAFETY: delegacion estandar.
        _ => unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) },
    }
}

/// Para la prueba visual: pone el raton «encima» de un boton sin mover el
/// raton de verdad (nada de clics simulados en el escritorio).
#[cfg(test)]
pub(crate) fn poner_encima(accion: Option<AccionBarra>) {
    if let Some(h) = ventana()
        && let Some(i) = interno_de(h)
    {
        i.encima = accion;
        recolocar_ventana(h, i);
    }
}
