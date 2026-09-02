//! La ventana del pin: PixPinPin, autocontenida tras GWLP_USERDATA.
//!
//! El pin vive en el bucle principal de la app (VentanaMensajes::ejecutar
//! bombea todos los mensajes del hilo), asi que su WndProc ejecuta los
//! efectos ahi mismo: mover con SetWindowPos, redimensionar recreando la
//! superficie, cerrar destruyendo. El ejecutable se entera por el callback
//! CambioPin (unica via: pixpin-pin no puede tocar el almacen, misma capa).
//!
//! Este WndProc JAMAS llama a PostQuitMessage — tercera vez que la mina de
//! S1-A esta a punto de pisarse, tercera vez que el comentario lo impide.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Once;

use pixpin_geom::{Punto, Rect};
use pixpin_render::{Color, MotorRender, RectF, Superficie};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Gdi::ValidateRect;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_DOWN, VK_ESCAPE, VK_LEFT, VK_RIGHT,
    VK_SHIFT, VK_UP,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_BACK, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::contenido::{Contenido, DOCUMENTO_FRANJA_LOGICA, NOTA_MARGEN_LOGICO, NOTA_TEXTO_LOGICO};
use crate::estado::{EfectoPin, EstadoPin, EventoPin, MINIMO_LOGICO};

/// Margen transparente alrededor del contenido: ahi vive la sombra (D30).
pub const MARGEN_SOMBRA_LOGICO: u32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CambioPin {
    Movido(Rect),
    Redimensionado(Rect),
    Cerrado,
    /// `Ctrl+C` sobre el pin enfocado (spec 4.2). Que significa copiar
    /// depende del tipo, y eso lo sabe el gestor, no la ventana.
    CopiarPedido,
    /// Menu: guardar el contenido en un fichero elegido por el usuario.
    GuardarComoPedido,
    /// Doble clic sobre una ficha, o menu: abrir con la app predeterminada.
    AbrirPedido,
    /// Menu de una ficha: abrir el Explorador con el fichero seleccionado.
    AbrirUbicacionPedido,
    /// Menu: `None` quita el grupo; `Some(i)` es el indice 0-7 en la paleta.
    GrupoPedido(Option<u8>),
    /// Menu: ocultar en bloque el grupo de este pin (D24).
    OcultarGrupoPedido,
    /// Menu: la unica accion destructiva; el gestor pide confirmacion.
    EliminarPedido,
    /// Doble clic sobre imagen o nota: entrar a anotar (D47).
    AnotarPedido,
    /// En modo anotacion, el raton se reenvia tal cual en coordenadas del
    /// CONTENIDO (el margen de sombra ya descontado). El pin no sabe
    /// dibujar: quien lleva la maquina de anotar es el gestor, que vive en
    /// una capa que si puede ver `pixpin-ui`.
    PunteroPulsado(Punto),
    PunteroMovido(Punto),
    PunteroSoltado(Punto),
    /// Rueda del raton. Positivo hacia arriba (D55).
    RuedaGirada(i32),
    /// Escape mientras se anota: lo interpreta la maquina, no la ventana.
    EscapeAnotando,
    /// Un caracter escrito mientras se anota, ya compuesto (WM_CHAR, IME
    /// incluido) (D57).
    CaracterAnotando(char),
    /// Enter mientras se anota: confirma el texto en curso.
    EnterAnotando,
    /// Retroceso mientras se anota: borra el ultimo caracter.
    RetrocesoAnotando,
    /// Un clic en la paleta flotante del pin, en coordenadas de la paleta
    /// (D58). Lo produce la paleta, no esta ventana, pero viaja por la
    /// misma cola del gestor.
    PaletaPulsada(Punto),
}

/// La lupa dentro del pin (D52): que trozo del contenido se amplia y donde
/// se dibuja, las dos en coordenadas del contenido. La aritmetica la hace el
/// gestor (la `Lupa` de `pixpin-ui` es L3); el pin solo copia pixeles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LupaPin {
    pub fuente: Rect,
    pub destino: Rect,
}

/// Identificador del temporizador que agrupa el guardado tras una rafaga de
/// flechas, y su retardo (spec 5.2: 300 ms tras el ultimo cambio).
const ID_TEMPORIZADOR_GUARDADO: usize = 1;
const RETARDO_GUARDADO_MS: u32 = 300;
/// Distancia a la que el borde del area de trabajo atrae al pin (px logicos).
const IMAN_LOGICO: i32 = 8;

fn es_flecha(vk: u32) -> bool {
    [VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN]
        .iter()
        .any(|t| t.0 as u32 == vk)
}

fn tecla_pulsada(vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    // SAFETY: consulta pura del estado de teclado; sin precondiciones.
    unsafe { (GetKeyState(vk.0 as i32) as u16 & 0x8000) != 0 }
}

/// Pega el rect al borde del area de trabajo del monitor donde esta la
/// ventana, si queda a tiro (spec 4.1).
fn con_iman(hwnd: HWND, rect: Rect, escala_por_cien: u32) -> Rect {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` es propia con su cbSize correcto; el handle es de una
    // ventana viva y MonitorFromWindow nunca falla (cae al mas cercano).
    let ok = unsafe {
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        GetMonitorInfoW(mon, &mut info).as_bool()
    };
    if !ok {
        return rect;
    }
    let trabajo = Rect {
        x: info.rcWork.left,
        y: info.rcWork.top,
        ancho: (info.rcWork.right - info.rcWork.left).max(0) as u32,
        alto: (info.rcWork.bottom - info.rcWork.top).max(0) as u32,
    };
    let umbral = (IMAN_LOGICO * escala_por_cien as i32 / 100).max(1);
    pixpin_geom::iman_de_bordes(rect, trabajo, umbral)
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorPin {
    #[error("no se pudo crear la ventana del pin: {0}")]
    Creacion(#[source] windows::core::Error),
    #[error("no se pudo preparar el dibujo del pin: {0}")]
    Dibujo(#[from] pixpin_render::ErrorRender),
}

/// Rect de VENTANA para un contenido: el margen de sombra a cada lado.
pub fn rect_ventana(contenido: Rect, escala_por_cien: u32) -> Rect {
    let m = (MARGEN_SOMBRA_LOGICO * escala_por_cien / 100) as i32;
    Rect {
        x: contenido.x - m,
        y: contenido.y - m,
        ancho: contenido.ancho + 2 * m as u32,
        alto: contenido.alto + 2 * m as u32,
    }
}

/// La inversa exacta de `rect_ventana`.
pub fn contenido_desde_ventana(ventana: Rect, escala_por_cien: u32) -> Rect {
    let m = (MARGEN_SOMBRA_LOGICO * escala_por_cien / 100) as i32;
    Rect {
        x: ventana.x + m,
        y: ventana.y + m,
        ancho: ventana.ancho.saturating_sub(2 * m as u32),
        alto: ventana.alto.saturating_sub(2 * m as u32),
    }
}

/// Todo lo que el WndProc necesita, colgado de GWLP_USERDATA.
struct PinInterno {
    estado: EstadoPin,
    escala_por_cien: u32,
    motor: Rc<MotorRender>,
    d3d: ID3D11Device,
    superficie: Superficie,
    /// (ancho, alto) nativos de la imagen: el 100% del doble clic. La nota y
    /// la ficha no tienen tamano nativo de pixeles, y ahi vale el actual.
    imagen_nativa: (u32, u32),
    /// El bitmap solo existe si hay imagen que dibujar (imagen o icono de
    /// ficha); la nota se pinta entera con texto.
    bitmap: Option<ID2D1Bitmap1>,
    contenido: Contenido,
    tema_claro: bool,
    /// RGB del grupo, que tiñe la sombra (D24/D30). `None` = sin grupo,
    /// sombra negra. El pin recibe el color ya resuelto: `ColorGrupo` vive
    /// en `pixpin-store`, que es su misma capa y no puede ver.
    color_sombra: Option<(f32, f32, f32)>,
    /// La sombra del enfocado es mas intensa: asi se sabe a quien cerrara
    /// `Esc` sin necesidad de un solo borde (D30).
    enfocado: bool,
    /// Etiquetas del menu, ya traducidas. Sin ellas no hay menu: es
    /// preferible no ofrecerlo a ofrecerlo en el idioma equivocado.
    textos: Option<crate::menu::TextosPin>,
    /// Lo que hay dibujado encima, ya convertido a ordenes por el motor 2D.
    /// El pin solo las pinta: quien las produce es el gestor (S3-B).
    anotaciones: Vec<pixpin_motor2d::Orden>,
    /// En modo anotacion el pin NO se mueve ni se redimensiona: arrastrar
    /// dibuja. Sin un modo explicito, el gesto seria ambiguo (D47).
    anotando: bool,
    /// La lupa, mientras la herramienta activa sea la lupa. No es un
    /// elemento: no se guarda (D52).
    lupa: Option<LupaPin>,
    al_cambiar: Box<dyn Fn(CambioPin)>,
}

pub struct Pin {
    hwnd: HWND,
}

static REGISTRO: Once = Once::new();

thread_local! {
    /// La mitad alta de un par subrogado UTF-16 a la espera de su mitad
    /// baja: WM_CHAR entrega un emoji en dos mensajes.
    static MITAD_ALTA: Cell<Option<u16>> = const { Cell::new(None) };
}

impl Pin {
    /// Crea el pin visible (sin robar el foco: spec 4.4) con su contenido ya
    /// pintado. `rect_contenido` en pixeles fisicos del escritorio virtual.
    pub fn nuevo(
        d3d: &ID3D11Device,
        motor: Rc<MotorRender>,
        contenido: Contenido,
        rect_contenido: Rect,
        escala_por_cien: u32,
        tema_claro: bool,
        al_cambiar: Box<dyn Fn(CambioPin)>,
    ) -> Result<Pin, ErrorPin> {
        REGISTRO.call_once(registrar_clase);
        let ventana = rect_ventana(rect_contenido, escala_por_cien);
        // SAFETY: la clase quedo registrada en call_once; estilos constantes
        // documentados; modulo propio.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW,
                w!("PixPinPin"),
                w!(""),
                WS_POPUP,
                ventana.x,
                ventana.y,
                ventana.ancho as i32,
                ventana.alto as i32,
                None,
                None,
                Some(GetModuleHandleW(None).map_err(ErrorPin::Creacion)?.into()),
                None,
            )
            .map_err(ErrorPin::Creacion)?
        };

        let superficie = Superficie::nueva(&motor, d3d, hwnd, ventana.ancho, ventana.alto)?;
        // La imagen del pin, o el icono de la ficha: los dos son bitmaps. La
        // nota no tiene ninguno y se pinta entera con texto.
        let fuente_bitmap = match &contenido {
            Contenido::Imagen(img) => Some(img),
            Contenido::Archivo { icono, .. } => icono.as_ref(),
            Contenido::Documento { vista, .. } => Some(vista),
            // El video no tiene bitmap fijo: lo trae cada fotograma.
            Contenido::Nota { .. } | Contenido::Video { .. } => None,
        };
        let bitmap = match fuente_bitmap {
            Some(img) => Some(motor.bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles)?),
            None => None,
        };
        let imagen_nativa = match &contenido {
            Contenido::Imagen(img) => (img.ancho, img.alto),
            Contenido::Documento { vista, .. } => (vista.ancho, vista.alto),
            Contenido::Video { ancho, alto, .. } if *ancho > 0 && *alto > 0 => (*ancho, *alto),
            // Sin tamano nativo de pixeles: el "100 %" de una nota o una
            // ficha es el tamano con el que nacio.
            _ => (rect_contenido.ancho, rect_contenido.alto),
        };

        let estado = if contenido.solo_ancho() {
            EstadoPin::nuevo_solo_ancho(rect_contenido, escala_por_cien)
        } else {
            EstadoPin::nuevo(rect_contenido, escala_por_cien)
        };

        let interno = Box::new(PinInterno {
            estado,
            escala_por_cien,
            motor,
            d3d: d3d.clone(),
            superficie,
            imagen_nativa,
            bitmap,
            contenido,
            tema_claro,
            color_sombra: None,
            enfocado: false,
            textos: None,
            anotaciones: Vec::new(),
            anotando: false,
            lupa: None,
            al_cambiar,
        });
        // SAFETY: la ventana es propia y viva; el Box se cede al USERDATA y
        // se recupera exactamente una vez en WM_NCDESTROY.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(interno) as isize);
        }

        let pin = Pin { hwnd };
        pin.repintar();
        // SAFETY: mostrar sin activar (spec 4.4: pinear no roba el foco).
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        Ok(pin)
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn rect_contenido(&self) -> Rect {
        match interno_de(self.hwnd) {
            Some(i) => i.estado.rect(),
            None => Rect {
                x: 0,
                y: 0,
                ancho: 0,
                alto: 0,
            },
        }
    }

    fn repintar(&self) {
        if let Some(i) = interno_de(self.hwnd) {
            pintar(i);
        }
    }

    /// Tiñe la sombra con el color del grupo, o la devuelve a negra con
    /// `None`. El gestor traduce `ColorGrupo` a RGB: este crate no puede
    /// ver `pixpin-store` (misma capa).
    /// Coloca el pin en un rect nuevo (la rueda hace zoom con esto).
    pub fn poner_rect(&self, contenido: Rect) {
        if let Some(i) = interno_de(self.hwnd) {
            i.estado.poner_rect(contenido);
        }
        aplicar(self.hwnd, EfectoPin::Redimensionar(contenido));
    }

    /// Entra o sale del modo anotacion (D47). Mientras se anota, el pin no
    /// se mueve ni se redimensiona: arrastrar dibuja.
    pub fn poner_modo_anotacion(&self, anotando: bool) {
        if let Some(i) = interno_de(self.hwnd) {
            i.anotando = anotando;
            pintar(i);
        }
    }

    pub fn anotando(&self) -> bool {
        interno_de(self.hwnd).is_some_and(|i| i.anotando)
    }

    /// La escala con la que nacio el pin (la de su monitor).
    pub fn escala_por_cien(&self) -> u32 {
        interno_de(self.hwnd).map_or(100, |i| i.escala_por_cien)
    }

    /// Coloca la ventana de composicion del IME donde se escribe (D57).
    /// `p` esta en coordenadas del contenido; se suma el margen de sombra.
    pub fn poner_posicion_ime(&self, p: Punto) {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::Input::Ime::{
            CFS_POINT, COMPOSITIONFORM, ImmGetContext, ImmReleaseContext, ImmSetCompositionWindow,
        };
        let Some(i) = interno_de(self.hwnd) else {
            return;
        };
        let m = (MARGEN_SOMBRA_LOGICO * i.escala_por_cien / 100) as i32;
        // SAFETY: contexto del IME de una ventana propia, tomado y devuelto
        // en la misma llamada; la estructura es local y valida.
        unsafe {
            let ctx = ImmGetContext(self.hwnd);
            if ctx.is_invalid() {
                return;
            }
            let forma = COMPOSITIONFORM {
                dwStyle: CFS_POINT,
                ptCurrentPos: POINT {
                    x: p.x + m,
                    y: p.y + m,
                },
                ..Default::default()
            };
            let _ = ImmSetCompositionWindow(ctx, &forma);
            let _ = ImmReleaseContext(self.hwnd, ctx);
        }
    }

    /// Cambia lo que hay dibujado encima y repinta. Las ordenes vienen del
    /// motor 2D, que es quien sabe convertir elementos en geometria.
    pub fn poner_anotaciones(&self, ordenes: Vec<pixpin_motor2d::Orden>) {
        if let Some(i) = interno_de(self.hwnd) {
            i.anotaciones = ordenes;
            pintar(i);
        }
    }

    /// Pone o quita la lupa (D52). Solo repinta si algo cambio: la lupa se
    /// actualiza con cada movimiento del raton y repintar en balde cuesta.
    pub fn poner_lupa(&self, lupa: Option<LupaPin>) {
        if let Some(i) = interno_de(self.hwnd) {
            if i.lupa != lupa {
                i.lupa = lupa;
                pintar(i);
            }
        }
    }

    /// Entrega las etiquetas del menu, ya traducidas. Hasta que llegan, el
    /// clic derecho no abre nada: mejor mudo que en otro idioma.
    pub fn poner_textos(&self, textos: crate::menu::TextosPin) {
        if let Some(i) = interno_de(self.hwnd) {
            i.textos = Some(textos);
        }
    }

    pub fn poner_color(&self, color: Option<(f32, f32, f32)>) {
        // Los pines viven en el hilo de interfaz, el mismo desde el que se
        // llama esto, asi que se toca el interno directamente: mandar un
        // mensaje solo anadiria un salto sin ganar nada.
        if let Some(i) = interno_de(self.hwnd) {
            i.color_sombra = color;
            pintar(i);
        }
    }
}

impl Drop for Pin {
    fn drop(&mut self) {
        // SAFETY: destruir una ventana propia desde su hilo; si el WndProc ya
        // la destruyo (Esc), DestroyWindow falla y se ignora.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// El interno colgado del USERDATA, si la ventana sigue viva.
fn interno_de<'a>(hwnd: HWND) -> Option<&'a mut PinInterno> {
    // SAFETY: el puntero lo puso Pin::nuevo y solo WM_NCDESTROY lo retira;
    // entre ambos es un Box valido. Todo ocurre en el hilo de interfaz.
    unsafe {
        let crudo = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PinInterno;
        crudo.as_mut()
    }
}

fn registrar_clase() {
    // SAFETY: registro unico (Once); CS_DBLCLKS para recibir WM_LBUTTONDBLCLK.
    unsafe {
        let clase = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(procedimiento_pin),
            hInstance: GetModuleHandleW(None).expect("modulo propio").into(),
            lpszClassName: w!("PixPinPin"),
            ..Default::default()
        };
        RegisterClassW(&clase);
    }
}

/// Dibuja el fotograma completo del pin: sombra + imagen. Cero cromo (D23).
fn pintar(i: &PinInterno) {
    let Ok(destino) = i.superficie.empezar(&i.motor) else {
        return;
    };
    let escala = i.escala_por_cien as f32 / 100.0;
    let m = MARGEN_SOMBRA_LOGICO as f32 * escala;
    let contenido = i.estado.rect();
    let (w, h) = (contenido.ancho as f32, contenido.alto as f32);
    let radio = 8.0 * escala;

    let _ = i.motor.dibujar(&destino, |p| {
        p.limpiar_transparente();
        // Sombra difusa: seis aros redondeados concentricos de alfa
        // decreciente, desplazados hacia abajo. Sin desenfoque real y
        // suficiente para el look de recorte elevado (D30). El cache por
        // bitmap de la spec queda para S2-B.
        let desplome = 2.0 * escala;
        // Sin grupo la sombra es negra; con grupo toma su color (D24). El
        // pin enfocado la lleva mas intensa y algo mas amplia.
        let (sr, sg, sb) = i.color_sombra.unwrap_or((0.0, 0.0, 0.0));
        let refuerzo = if i.enfocado { 1.7 } else { 1.0 };
        for (paso, alfa) in [0.10f32, 0.08, 0.06, 0.045, 0.03, 0.02].iter().enumerate() {
            let crece = (paso as f32 + 1.0) * 2.0 * escala * if i.enfocado { 1.25 } else { 1.0 };
            p.rellenar_redondeado(
                RectF {
                    x: m - crece,
                    y: m - crece + desplome,
                    ancho: w + 2.0 * crece,
                    alto: h + 2.0 * crece,
                },
                radio + crece,
                Color {
                    r: sr,
                    g: sg,
                    b: sb,
                    a: (*alfa * refuerzo).min(1.0),
                },
            );
        }
        // La tarjeta: blanca o negra segun el tema del sistema (D30). Bajo
        // una imagen opaca no se ve, pero es el lienzo de la nota y la
        // ficha, y el fondo de una imagen con transparencia.
        let lienzo = if i.tema_claro {
            Color::BLANCO
        } else {
            Color {
                r: 0.11,
                g: 0.11,
                b: 0.12,
                a: 1.0,
            }
        };
        let tinta = if i.tema_claro {
            Color {
                r: 0.10,
                g: 0.10,
                b: 0.11,
                a: 1.0,
            }
        } else {
            Color {
                r: 0.93,
                g: 0.93,
                b: 0.94,
                a: 1.0,
            }
        };
        let tinta_tenue = Color { a: 0.55, ..tinta };
        let caja = RectF {
            x: m,
            y: m,
            ancho: w,
            alto: h,
        };
        p.rellenar_redondeado(caja, radio, lienzo);

        match &i.contenido {
            // La imagen va sin recorte redondeado (simplificacion consciente
            // heredada de S2-A): el redondeo se aprecia en la sombra.
            // El video es una imagen en movimiento: el bitmap es el ultimo
            // fotograma, o nada hasta que llegue el primero (D63).
            Contenido::Imagen(_) | Contenido::Video { .. } => {
                if let Some(b) = &i.bitmap {
                    p.bitmap(b, caja, None, false);
                }
            }

            // La miniatura arriba y el nombre en su franja debajo (D71).
            Contenido::Documento { nombre, .. } => {
                let franja = DOCUMENTO_FRANJA_LOGICA as f32 * escala;
                let alto_vista = (h - franja).max(1.0);
                if let Some(b) = &i.bitmap {
                    p.bitmap(
                        b,
                        RectF {
                            x: caja.x,
                            y: caja.y,
                            ancho: caja.ancho,
                            alto: alto_vista,
                        },
                        None,
                        false,
                    );
                }
                let margen = 8.0 * escala;
                p.texto_ajustado(
                    nombre,
                    m + margen,
                    m + alto_vista + 6.0 * escala,
                    13.0 * escala,
                    (w - 2.0 * margen).max(1.0),
                    tinta,
                );
            }

            Contenido::Nota { texto } => {
                let margen = NOTA_MARGEN_LOGICO * escala;
                p.texto_ajustado(
                    texto,
                    m + margen,
                    m + margen,
                    NOTA_TEXTO_LOGICO * escala,
                    (w - 2.0 * margen).max(1.0),
                    tinta,
                );
            }

            Contenido::Archivo {
                nombre,
                detalle,
                existe,
                ..
            } => {
                let margen = 12.0 * escala;
                let lado = 32.0 * escala;
                if let Some(b) = &i.bitmap {
                    p.bitmap(
                        b,
                        RectF {
                            x: m + margen,
                            y: m + (h - lado) / 2.0,
                            ancho: lado,
                            alto: lado,
                        },
                        None,
                        false,
                    );
                }
                let x_texto = m + margen + lado + margen;
                let ancho_texto = (w - (x_texto - m) - margen).max(1.0);
                p.texto_ajustado(
                    nombre,
                    x_texto,
                    m + h / 2.0 - 17.0 * escala,
                    14.0 * escala,
                    ancho_texto,
                    tinta,
                );
                // La referencia rota se MUESTRA (D28): el detalle lo dice, y
                // en rojo para que no haya que leerlo dos veces.
                let color_detalle = if *existe {
                    tinta_tenue
                } else {
                    Color {
                        r: 0.86,
                        g: 0.20,
                        b: 0.18,
                        a: 1.0,
                    }
                };
                p.texto_ajustado(
                    detalle,
                    x_texto,
                    m + h / 2.0 + 2.0 * escala,
                    12.0 * escala,
                    ancho_texto,
                    color_detalle,
                );
            }
        }

        // Las anotaciones van ENCIMA de todo lo demas: son una capa, y el
        // contenido original nunca se toca (D48). Se dibujan en coordenadas
        // del contenido, asi que hay que sumarles el margen de la sombra.
        pintar_anotaciones(p, i, m);

        // La lupa amplia el bitmap NATIVO del pin: si el pin esta escalado,
        // la fuente en pixeles del contenido se convierte a pixeles de la
        // imagen, y la lupa ensena detalle real, no pixeles ya estirados.
        if let (Some(l), Some(b)) = (&i.lupa, &i.bitmap) {
            let (nw, nh) = i.imagen_nativa;
            let fx = nw as f32 / w.max(1.0);
            let fy = nh as f32 / h.max(1.0);
            let fuente = RectF {
                x: l.fuente.x as f32 * fx,
                y: l.fuente.y as f32 * fy,
                ancho: l.fuente.ancho as f32 * fx,
                alto: l.fuente.alto as f32 * fy,
            };
            let destino = RectF {
                x: l.destino.x as f32 + m,
                y: l.destino.y as f32 + m,
                ancho: l.destino.ancho as f32,
                alto: l.destino.alto as f32,
            };
            p.bitmap(b, destino, Some(fuente), true);
            p.trazar(destino, 2.0 * escala, Color::ACENTO);
        }
    });
    let _ = i.superficie.presentar();
}

/// Pinta las ordenes de dibujo del motor 2D sobre el contenido del pin.
///
/// El motor produce geometria y el pintor la pinta: es la separacion que
/// mantiene al motor puro y probable sin GPU.
fn pintar_anotaciones(p: &pixpin_render::Pintor, i: &PinInterno, margen: f32) {
    use pixpin_motor2d::Orden;

    // El origen del documento de anotacion es la esquina del CONTENIDO, no
    // la de la ventana: asi las anotaciones acompañan al pin al moverlo sin
    // recalcular ni un punto.
    let mover = |q: &pixpin_motor2d::Punto2| (q.x + margen, q.y + margen);
    let color = |c: pixpin_motor2d::ColorRgba| Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    };

    for orden in &i.anotaciones {
        match orden {
            Orden::Poligono { puntos, color: c } | Orden::Relleno { puntos, color: c } => {
                let v: Vec<(f32, f32)> = puntos.iter().map(mover).collect();
                p.poligono(&v, color(*c));
            }
            Orden::Polilinea {
                puntos,
                color: c,
                grosor,
                ..
            } => {
                let v: Vec<(f32, f32)> = puntos.iter().map(mover).collect();
                p.polilinea(&v, *grosor, color(*c));
            }
            Orden::Texto {
                texto,
                x,
                y,
                tam,
                color: c,
                ancho_max,
                ..
            } => p.texto_ajustado(texto, x + margen, y + margen, *tam, *ancho_max, color(*c)),
            // El velo del foco (D51) cubre el CONTENIDO del pin, no la
            // ventana entera: la sombra queda fuera del oscurecido.
            Orden::Velo { hueco, color: c } => {
                let r = i.estado.rect();
                let marco = RectF {
                    x: margen,
                    y: margen,
                    ancho: r.ancho as f32,
                    alto: r.alto as f32,
                };
                let v: Vec<(f32, f32)> = hueco.iter().map(mover).collect();
                p.velo(marco, &v, color(*c));
            }
            // Las imagenes incrustadas quedan para S6 (D61), cuando haya un
            // almacen de bitmaps por anotacion de donde sacarlas.
            Orden::Imagen { .. } => {}
        }
    }
}

/// Aplica un efecto de la maquina pura sobre la ventana real.
fn aplicar(hwnd: HWND, efecto: EfectoPin) {
    let Some(i) = interno_de(hwnd) else { return };
    match efecto {
        EfectoPin::Nada => {}
        EfectoPin::Mover(contenido) => {
            let v = rect_ventana(contenido, i.escala_por_cien);
            // SAFETY: SetWindowPos sobre ventana propia; sin redibujar: la
            // composicion mueve el visual entero.
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    v.x,
                    v.y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
        EfectoPin::Redimensionar(contenido) => {
            let v = rect_ventana(contenido, i.escala_por_cien);
            // SAFETY: igual que arriba, con tamano.
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    v.x,
                    v.y,
                    v.ancho as i32,
                    v.alto as i32,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            // La superficie es del tamano de la ventana: se recrea.
            if let Ok(s) = Superficie::nueva(&i.motor, &i.d3d, hwnd, v.ancho, v.alto) {
                i.superficie = s;
            }
            pintar(i);
        }
        EfectoPin::AlternarTamano => {
            let actual = i.estado.rect();
            let (nw, nh) = i.imagen_nativa;
            let al_natural = actual.ancho == nw && actual.alto == nh;
            let nuevo = if al_natural {
                // Ajustado: 80% del area del monitor bajo el pin no esta a
                // mano sin enumerar; media del nativo, con el minimo del
                // estado. Suficiente para S2-A y determinista.
                let minimo = MINIMO_LOGICO * i.escala_por_cien / 100;
                Rect {
                    x: actual.x,
                    y: actual.y,
                    ancho: (nw / 2).max(minimo),
                    alto: (nh / 2).max(minimo),
                }
            } else {
                Rect {
                    x: actual.x,
                    y: actual.y,
                    ancho: nw,
                    alto: nh,
                }
            };
            i.estado.poner_rect(nuevo);
            aplicar(hwnd, EfectoPin::Redimensionar(nuevo));
            if let Some(i2) = interno_de(hwnd) {
                (i2.al_cambiar)(CambioPin::Redimensionado(nuevo));
            }
        }
        EfectoPin::GestoTerminado(contenido) => {
            // El iman actua AL SOLTAR, no durante el arrastre: pegarse a
            // media pasada peleaba con el raton y se sentia como un tiron.
            let pegado = con_iman(hwnd, contenido, i.escala_por_cien);
            if pegado != contenido {
                i.estado.poner_rect(pegado);
                aplicar(hwnd, EfectoPin::Mover(pegado));
            }
            (i.al_cambiar)(CambioPin::Movido(pegado));
        }
        EfectoPin::Cerrar => {
            (i.al_cambiar)(CambioPin::Cerrado);
            // SAFETY: destruye la ventana propia; WM_NCDESTROY libera el Box.
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

extern "system" fn procedimiento_pin(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Cliente -> escritorio virtual, como en el overlay.
    let punto = |lparam: LPARAM| {
        let mut r = RECT::default();
        // SAFETY: GetWindowRect sobre la ventana del propio WndProc.
        unsafe {
            let _ = GetWindowRect(hwnd, &mut r);
        }
        Punto {
            x: r.left + (lparam.0 & 0xFFFF) as i16 as i32,
            y: r.top + ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
        }
    };

    /// Coordenadas dentro del CONTENIDO, con el margen de sombra ya
    /// descontado: es el sistema en el que vive el documento de anotacion,
    /// y por eso las anotaciones acompañan al pin al moverlo sin recalcular.
    fn punto_contenido(i: &PinInterno, lparam: LPARAM) -> Punto {
        let m = (MARGEN_SOMBRA_LOGICO * i.escala_por_cien / 100) as i32;
        Punto {
            x: (lparam.0 & 0xFFFF) as i16 as i32 - m,
            y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32 - m,
        }
    }

    match mensaje {
        WM_LBUTTONDOWN => {
            // SAFETY: captura para no perder el arrastre al salir del borde.
            unsafe { SetCapture(hwnd) };
            // Clic tambien enfoca: es lo que arma el Esc de D23.
            // SAFETY: foco sobre ventana propia.
            unsafe {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(hwnd));
            }
            if let Some(i) = interno_de(hwnd) {
                if i.anotando {
                    (i.al_cambiar)(CambioPin::PunteroPulsado(punto_contenido(i, lparam)));
                } else {
                    let e = i.estado.procesar(EventoPin::BotonPulsado(punto(lparam)));
                    aplicar(hwnd, e);
                }
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            if let Some(i) = interno_de(hwnd) {
                // La palabra alta del wparam trae el giro, con signo.
                let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as i32;
                (i.al_cambiar)(CambioPin::RuedaGirada(delta));
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if let Some(i) = interno_de(hwnd) {
                if i.anotando {
                    (i.al_cambiar)(CambioPin::PunteroMovido(punto_contenido(i, lparam)));
                } else {
                    let e = i.estado.procesar(EventoPin::RatonMovido(punto(lparam)));
                    aplicar(hwnd, e);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            // SAFETY: libera la captura tomada en el pulsado.
            unsafe {
                let _ = ReleaseCapture();
            }
            if let Some(i) = interno_de(hwnd) {
                if i.anotando {
                    (i.al_cambiar)(CambioPin::PunteroSoltado(punto_contenido(i, lparam)));
                } else {
                    let e = i.estado.procesar(EventoPin::BotonSoltado);
                    aplicar(hwnd, e);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONDBLCLK => {
            if let Some(i) = interno_de(hwnd) {
                // En una ficha, doble clic ABRE el archivo (spec 4.1). En
                // imagen y nota entra a ANOTAR (D47): alternar tamano pasa
                // al menu, donde ya estaba "Tamano original".
                if matches!(
                    i.contenido,
                    Contenido::Archivo { .. } | Contenido::Documento { .. }
                ) {
                    (i.al_cambiar)(CambioPin::AbrirPedido);
                } else if matches!(i.contenido, Contenido::Video { .. }) {
                    // El doble clic en un video reproduce o pausa (D68/D70);
                    // llega con el reproductor.
                } else if !i.anotando {
                    (i.al_cambiar)(CambioPin::AnotarPedido);
                }
            }
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            if let Some(i) = interno_de(hwnd) {
                let Some(t) = i.textos.clone() else {
                    return LRESULT(0);
                };
                match crate::menu::mostrar(hwnd, &i.contenido, i.color_sombra.is_some(), false, &t)
                {
                    None => {}
                    // Las dos que puede resolver la propia ventana se
                    // resuelven aqui: pedirselas al gestor solo daria un
                    // rodeo para volver al mismo sitio.
                    Some(crate::menu::CMD_TAMANO_ORIGINAL) => {
                        aplicar(hwnd, EfectoPin::AlternarTamano)
                    }
                    Some(crate::menu::CMD_CERRAR) => aplicar(hwnd, EfectoPin::Cerrar),
                    Some(cmd) => {
                        let cambio = match cmd {
                            crate::menu::CMD_COPIAR => Some(CambioPin::CopiarPedido),
                            crate::menu::CMD_GUARDAR_COMO => Some(CambioPin::GuardarComoPedido),
                            crate::menu::CMD_ABRIR_UBICACION => {
                                Some(CambioPin::AbrirUbicacionPedido)
                            }
                            crate::menu::CMD_OCULTAR_GRUPO => Some(CambioPin::OcultarGrupoPedido),
                            crate::menu::CMD_ELIMINAR => Some(CambioPin::EliminarPedido),
                            crate::menu::CMD_SIN_GRUPO => Some(CambioPin::GrupoPedido(None)),
                            c if (crate::menu::CMD_COLOR_BASE..crate::menu::CMD_COLOR_BASE + 8)
                                .contains(&c) =>
                            {
                                Some(CambioPin::GrupoPedido(Some(
                                    (c - crate::menu::CMD_COLOR_BASE) as u8,
                                )))
                            }
                            _ => None,
                        };
                        if let Some(c) = cambio {
                            (i.al_cambiar)(c);
                        }
                    }
                }
            }
            LRESULT(0)
        }
        WM_SETFOCUS | WM_KILLFOCUS => {
            if let Some(i) = interno_de(hwnd) {
                i.enfocado = mensaje == WM_SETFOCUS;
                pintar(i);
            }
            LRESULT(0)
        }
        WM_CHAR => {
            // Solo anotando: fuera de ese modo el pin no escribe nada. WM_CHAR
            // trae unidades UTF-16 y un emoji llega en dos; el IME entrega
            // por aqui el texto ya compuesto (D57).
            if let Some(i) = interno_de(hwnd) {
                if i.anotando {
                    let unidad = wparam.0 as u16;
                    let caracter = MITAD_ALTA.with(|alta| {
                        if (0xD800..0xDC00).contains(&unidad) {
                            alta.set(Some(unidad));
                            None
                        } else if (0xDC00..0xE000).contains(&unidad) {
                            let a = alta.take()?;
                            char::decode_utf16([a, unidad]).next()?.ok()
                        } else {
                            alta.set(None);
                            char::from_u32(unidad as u32)
                        }
                    });
                    if let Some(c) = caracter {
                        (i.al_cambiar)(CambioPin::CaracterAnotando(c));
                    }
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN
            if wparam.0 as u32 == VK_RETURN.0 as u32 || wparam.0 as u32 == VK_BACK.0 as u32 =>
        {
            if let Some(i) = interno_de(hwnd) {
                if i.anotando {
                    (i.al_cambiar)(if wparam.0 as u32 == VK_RETURN.0 as u32 {
                        CambioPin::EnterAnotando
                    } else {
                        CambioPin::RetrocesoAnotando
                    });
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN if wparam.0 as u32 == VK_ESCAPE.0 as u32 => {
            if let Some(i) = interno_de(hwnd) {
                if i.anotando {
                    // Anotando, Escape es de la maquina de anotar: el primero
                    // abandona el trazo y el segundo sale del modo. Cerrar el
                    // pin aqui tiraria el dibujo.
                    (i.al_cambiar)(CambioPin::EscapeAnotando);
                } else {
                    let e = i.estado.procesar(EventoPin::Escape);
                    aplicar(hwnd, e);
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN if es_flecha(wparam.0 as u32) => {
            // Las flechas mueven ya y agrupan la persistencia: una rafaga
            // de veinte pulsaciones no puede escribir veinte veces el
            // indice (D34). El temporizador emite un solo Movido al parar.
            if let Some(i) = interno_de(hwnd) {
                let paso = if tecla_pulsada(VK_SHIFT) { 10 } else { 1 };
                let paso = (paso * i.escala_por_cien / 100).max(1) as i32;
                let r = i.estado.rect();
                let (dx, dy) = match wparam.0 as u32 {
                    x if x == VK_LEFT.0 as u32 => (-paso, 0),
                    x if x == VK_RIGHT.0 as u32 => (paso, 0),
                    x if x == VK_UP.0 as u32 => (0, -paso),
                    _ => (0, paso),
                };
                let nuevo = Rect {
                    x: r.x + dx,
                    y: r.y + dy,
                    ..r
                };
                i.estado.poner_rect(nuevo);
                aplicar(hwnd, EfectoPin::Mover(nuevo));
                // SAFETY: temporizador sobre ventana propia; se mata en el
                // WM_TIMER de mas abajo.
                unsafe {
                    SetTimer(
                        Some(hwnd),
                        ID_TEMPORIZADOR_GUARDADO,
                        RETARDO_GUARDADO_MS,
                        None,
                    );
                }
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == ID_TEMPORIZADOR_GUARDADO => {
            // SAFETY: mata el temporizador propio armado arriba.
            unsafe {
                let _ = KillTimer(Some(hwnd), ID_TEMPORIZADOR_GUARDADO);
            }
            if let Some(i) = interno_de(hwnd) {
                let pegado = con_iman(hwnd, i.estado.rect(), i.escala_por_cien);
                if pegado != i.estado.rect() {
                    i.estado.poner_rect(pegado);
                    aplicar(hwnd, EfectoPin::Mover(pegado));
                }
                (i.al_cambiar)(CambioPin::Movido(i.estado.rect()));
            }
            LRESULT(0)
        }
        WM_KEYDOWN if wparam.0 as u32 == b'C' as u32 && tecla_pulsada(VK_CONTROL) => {
            if let Some(i) = interno_de(hwnd) {
                (i.al_cambiar)(CambioPin::CopiarPedido);
            }
            LRESULT(0)
        }
        WM_SETCURSOR => {
            let diagonal = interno_de(hwnd)
                .map(|i| {
                    // La posicion del cursor en pantalla, preguntada aqui:
                    // WM_SETCURSOR no trae coordenadas utiles.
                    let mut p = windows::Win32::Foundation::POINT::default();
                    // SAFETY: GetCursorPos escribe en la variable local.
                    unsafe {
                        let _ = GetCursorPos(&mut p);
                    }
                    i.estado.sobre_esquina(Punto { x: p.x, y: p.y })
                })
                .unwrap_or(false);
            let id = if diagonal { IDC_SIZENWSE } else { IDC_SIZEALL };
            // SAFETY: cursores del sistema, llamadas sin precondiciones.
            unsafe {
                if let Ok(c) = LoadCursorW(None, id) {
                    SetCursor(Some(c));
                }
            }
            LRESULT(1)
        }
        WM_PAINT => {
            // SAFETY: ValidateRect marca pintado; el dibujo es del swapchain.
            unsafe {
                let _ = ValidateRect(Some(hwnd), None);
            }
            if let Some(i) = interno_de(hwnd) {
                pintar(i);
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            // SAFETY: recupera el Box cedido en Pin::nuevo exactamente una
            // vez y deja el USERDATA a cero antes de soltarlo.
            unsafe {
                let crudo = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut PinInterno;
                if !crudo.is_null() {
                    drop(Box::from_raw(crudo));
                }
            }
            LRESULT(0)
        }
        // NUNCA PostQuitMessage: cerrar un pin no apaga la aplicacion.
        _ => {
            // SAFETY: delegacion estandar.
            unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_codec::ImagenRgba;
    use pixpin_geom::Rect;
    use pixpin_render::MotorRender;
    use std::cell::RefCell;
    use std::rc::Rc;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    };

    fn d3d() -> ID3D11Device {
        let mut d = None;
        // SAFETY: salidas locales, constantes documentadas (patron de
        // pixpin-render).
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

    fn imagen_2x2() -> ImagenRgba {
        ImagenRgba {
            ancho: 2,
            alto: 2,
            pixeles: vec![255; 16],
        }
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn el_pin_se_crea_visible_y_al_destruirse_no_mata_nada() {
        let d3d = d3d();
        let motor = Rc::new(MotorRender::nuevo(&d3d).unwrap());
        let cambios: Rc<RefCell<Vec<CambioPin>>> = Rc::new(RefCell::new(Vec::new()));
        let c = Rc::clone(&cambios);
        let pin = Pin::nuevo(
            &d3d,
            Rc::clone(&motor),
            Contenido::Imagen(imagen_2x2()),
            Rect {
                x: 100,
                y: 100,
                ancho: 200,
                alto: 150,
            },
            100,
            true,
            Box::new(move |cambio| c.borrow_mut().push(cambio)),
        )
        .expect("el pin deberia crearse");

        // SAFETY: IsWindowVisible es consulta pura sobre handle vivo.
        let visible = unsafe { IsWindowVisible(pin.hwnd()).as_bool() };
        assert!(visible, "el pin nace visible (sin robar el foco)");
        assert_eq!(
            pin.rect_contenido(),
            Rect {
                x: 100,
                y: 100,
                ancho: 200,
                alto: 150
            }
        );

        // Dos pines: destruir uno no toca al otro (la mina de S1-A).
        let c2 = Rc::clone(&cambios);
        let pin2 = Pin::nuevo(
            &d3d,
            motor,
            Contenido::Imagen(imagen_2x2()),
            Rect {
                x: 400,
                y: 100,
                ancho: 200,
                alto: 150,
            },
            100,
            true,
            Box::new(move |cambio| c2.borrow_mut().push(cambio)),
        )
        .unwrap();
        let hwnd2 = pin2.hwnd();
        drop(pin);
        // SAFETY: IsWindow consulta pura.
        let vivo2 = unsafe { IsWindow(Some(hwnd2)).as_bool() };
        assert!(vivo2, "destruir un pin no puede llevarse a los demas");
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn una_nota_y_una_ficha_se_crean_y_se_destruyen_limpio() {
        // Los dos tipos sin imagen: la nota no tiene bitmap y la ficha
        // dibuja icono mas dos textos. Si alguno reventara al pintar, se
        // veria aqui y no en el escritorio del usuario.
        let d3d = d3d();
        let motor = Rc::new(MotorRender::nuevo(&d3d).unwrap());
        let sitio = |x| Rect {
            x,
            y: 700,
            ancho: 280,
            alto: 72,
        };

        let nota = Pin::nuevo(
            &d3d,
            Rc::clone(&motor),
            Contenido::Nota {
                texto: "una nota con acentos: canción, ñandú".into(),
            },
            sitio(100),
            100,
            true,
            Box::new(|_| {}),
        )
        .expect("la nota deberia crearse");

        let ficha = Pin::nuevo(
            &d3d,
            motor,
            Contenido::Archivo {
                nombre: "informe.pdf".into(),
                detalle: "no encontrado".into(),
                icono: crate::icono::icono_de(std::path::Path::new(r"Z:\no\existe\informe.pdf")),
                existe: false,
            },
            sitio(500),
            100,
            false,
            Box::new(|_| {}),
        )
        .expect("la ficha deberia crearse");

        // SAFETY: consultas puras sobre handles vivos.
        unsafe {
            assert!(IsWindowVisible(nota.hwnd()).as_bool());
            assert!(IsWindowVisible(ficha.hwnd()).as_bool());
        }
        let h = ficha.hwnd();
        drop(nota);
        // SAFETY: IsWindow es consulta pura sobre un handle propio.
        let sigue_viva = unsafe { IsWindow(Some(h)).as_bool() };
        assert!(sigue_viva, "cerrar la nota no puede llevarse la ficha");
    }

    #[test]
    fn la_ventana_es_mayor_que_el_contenido_por_el_margen() {
        // Puro: la conversion contenido <-> ventana con margen de sombra.
        let contenido = Rect {
            x: 100,
            y: 100,
            ancho: 200,
            alto: 150,
        };
        let v = rect_ventana(contenido, 150);
        let margen = (MARGEN_SOMBRA_LOGICO * 150 / 100) as i32;
        assert_eq!(v.x, 100 - margen);
        assert_eq!(v.ancho, 200 + 2 * margen as u32);
        assert_eq!(
            contenido_desde_ventana(v, 150),
            contenido,
            "ida y vuelta exacta"
        );
    }
}
