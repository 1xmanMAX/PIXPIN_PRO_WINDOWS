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
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_BACK, VK_RETURN, VK_SPACE};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::contenido::{
    Contenido, DOCUMENTO_FRANJA_LOGICA, FICHA_DETALLE_LOGICO, FICHA_ICONO_LOGICO,
    FICHA_MARGEN_LOGICO, FICHA_NOMBRE_LOGICO, NOTA_MARGEN_LOGICO, NOTA_TEXTO_LOGICO,
};
use crate::estado::{EfectoPin, EstadoPin, EventoPin, MINIMO_LOGICO};
use crate::video::Reproductor;

/// Margen transparente alrededor del contenido: ahi vive la sombra (D30).
pub const MARGEN_SOMBRA_LOGICO: u32 = 24;

/// Todo lo que hay que guardar de un pin para devolverlo tal cual: donde
/// esta, a que tamano, con que zoom de texto y como esta girado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colocacion {
    pub rect: Rect,
    /// 100 salvo en las notas, donde la rueda cambia el tamano del texto.
    pub zoom_por_cien: u32,
    /// Cuartos de vuelta a la derecha, de 0 a 3.
    pub giro: u8,
    pub volteo_h: bool,
    pub volteo_v: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CambioPin {
    /// Lo que el gestor persiste cuando el pin se mueve o cambia de tamano.
    Movido(Colocacion),
    Redimensionado(Colocacion),
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
    /// La rueda, con el punto de pantalla donde estaba el cursor: el zoom
    /// se ancla ahi, no en el centro del pin.
    RuedaGirada {
        delta: i32,
        cursor: Punto,
    },
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
    /// Media Foundation no pudo con el video (D72): el gestor vuelve a
    /// crear el pin como documento o ficha.
    VideoFallido,
}

/// La lupa dentro del pin (D52): que trozo del contenido se amplia y donde
/// se dibuja, las dos en coordenadas del contenido. La aritmetica la hace el
/// gestor (la `Lupa` de `pixpin-ui` es L3); el pin solo copia pixeles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LupaPin {
    pub fuente: Rect,
    pub destino: Rect,
}

/// El cursor mientras se anota, segun la herramienta. Lo decide el gestor
/// (que conoce la herramienta); la ventana solo lo ensena. Sin esto el
/// cursor se quedaba en las cuatro flechas de mover, que es lo que el
/// usuario vio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorAnotacion {
    #[default]
    Cruz,
    Texto,
    Flecha,
}

/// Identificador del temporizador que agrupa el guardado tras una rafaga de
/// flechas, y su retardo (spec 5.2: 300 ms tras el ultimo cambio).
const ID_TEMPORIZADOR_GUARDADO: usize = 1;
const RETARDO_GUARDADO_MS: u32 = 300;
/// El temporizador que pregunta por fotogramas nuevos a un pin de video
/// (D67). Solo corre mientras el video se reproduce.
const ID_TEMPORIZADOR_VIDEO: usize = 2;
/// El zoom animado de la rueda: un tick por fotograma hasta llegar.
const ID_TEMPORIZADOR_ZOOM: usize = 3;
/// Un disparo tras el ultimo cambio de un gesto continuo (Ctrl + arrastrar),
/// para dibujar nitido sin esperar a que el usuario suelte el boton.
const ID_TEMPORIZADOR_REPOSO: usize = 4;
const RITMO_ZOOM_MS: u32 = 16;
/// Lo que dura ir de un tamano al siguiente. Corto: la rueda encadena
/// muescas y una persecucion lenta iria por detras de la mano.
///
/// Cuanto se espera, tras el ultimo cambio, para redibujar nitido. Mientras
/// se interactua basta con estirar la textura, que es gratis; el dibujo de
/// verdad se paga UNA vez, al parar.
const REPOSO_ZOOM_MS: u32 = 150;

/// Un zoom en curso: la persecucion del destino y lo que hace falta para
/// llevar con ella el zoom del texto de una nota.
struct ZoomEnCurso {
    control: crate::zoom::ControlZoom,
    /// Cuando fue el ultimo paso, para saber cuanto tiempo ha pasado de
    /// verdad. La persecucion es independiente de los fotogramas justamente
    /// porque mide el tiempo en vez de contar ticks.
    ultimo_paso: std::time::Instant,
    /// El zoom del texto y el ancho con los que empezo: el texto de una nota
    /// crece en la misma proporcion que la caja, asi que basta una regla de
    /// tres desde estos dos.
    zoom_texto_inicial: f32,
    ancho_inicial: u32,
}

impl ZoomEnCurso {
    /// El zoom de texto que corresponde a un ancho dado.
    fn zoom_texto_en(&self, ancho: u32) -> f32 {
        self.zoom_texto_inicial * ancho as f32 / self.ancho_inicial.max(1) as f32
    }
}
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
    #[error("no se pudo abrir el reproductor de video: {0}")]
    Video(#[source] windows::core::Error),
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

/// El escritorio virtual (todos los monitores), en pixeles fisicos.
fn escritorio_virtual() -> Rect {
    // SAFETY: consultas puras de metricas del sistema.
    unsafe {
        Rect {
            x: GetSystemMetrics(SM_XVIRTUALSCREEN),
            y: GetSystemMetrics(SM_YVIRTUALSCREEN),
            ancho: GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1) as u32,
            alto: GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1) as u32,
        }
    }
}

/// La ventana REAL de un contenido: `rect_ventana` recortado al escritorio.
///
/// Un pin puede ser mas grande que la pantalla (agrandado con la rueda o
/// con Ctrl + arrastrar). La ventana no lo acompaña: se queda con la parte
/// visible y el contenido se pinta desplazado dentro de ella. Asi la
/// superficie de dibujo nunca supera el escritorio, que es donde estaba el
/// fallo que vio el usuario: la ventana crecia y el contenido dejaba de
/// pintarse (la superficie no se podia crear tan grande) y parecia que el
/// pin se desplazaba hacia arriba a la izquierda escondiendo la imagen.
pub fn ventana_visible(contenido: Rect, escala_por_cien: u32) -> Rect {
    recortar_al_escritorio(
        rect_ventana(contenido, escala_por_cien),
        escritorio_virtual(),
    )
}

/// Como encajar el pin en la pantalla, con las teclas 1, 2 y 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vista {
    /// Un pixel de la imagen, un pixel de la pantalla. Es la unica escala en
    /// la que la captura se ve tal cual se hizo, sin filtro que la suavice.
    Original,
    /// La mayor que cabe entera en el monitor.
    Ajustar,
    /// La menor que cubre el monitor: llena, recortando lo que sobra.
    Rellenar,
}

/// El area de trabajo del monitor donde esta la ventana (sin la barra de
/// tareas), en pixeles fisicos.
fn area_de_trabajo(hwnd: HWND) -> Rect {
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: MonitorFromWindow siempre devuelve un monitor con
    // DEFAULTTONEAREST; GetMonitorInfoW escribe en la estructura local, que
    // lleva su cbSize puesto.
    let ok = unsafe {
        let m = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        GetMonitorInfoW(m, &mut info).as_bool()
    };
    if !ok {
        return escritorio_virtual();
    }
    let r = info.rcWork;
    Rect {
        x: r.left,
        y: r.top,
        ancho: (r.right - r.left).max(1) as u32,
        alto: (r.bottom - r.top).max(1) as u32,
    }
}

/// El rect que le toca al pin para una vista dada, centrado en su monitor.
///
/// `Ajustar` y `Rellenar` conservan la proporcion NATIVA de la imagen, no la
/// que tenga el pin ahora: si no, encadenar dos ajustes iria deformando el
/// resultado poco a poco.
fn rect_para_vista(hwnd: HWND, i: &PinInterno, vista: Vista) -> Rect {
    let area = area_de_trabajo(hwnd);
    let (nw, nh) = i.imagen_nativa;
    let (nw, nh) = (nw.max(1), nh.max(1));

    let (ancho, alto) = match vista {
        Vista::Original => (nw, nh),
        Vista::Ajustar | Vista::Rellenar => {
            let fx = area.ancho as f32 / nw as f32;
            let fy = area.alto as f32 / nh as f32;
            // Caber entero es el menor de los dos factores; cubrir, el mayor.
            let f = if vista == Vista::Ajustar {
                fx.min(fy)
            } else {
                fx.max(fy)
            };
            (
                ((nw as f32 * f).round() as u32).max(1),
                ((nh as f32 * f).round() as u32).max(1),
            )
        }
    };
    // Centrado en el area de trabajo: al rellenar se sale por los cuatro
    // lados por igual, que es lo que se espera.
    Rect {
        x: area.x + (area.ancho as i32 - ancho as i32) / 2,
        y: area.y + (area.alto as i32 - alto as i32) / 2,
        ancho,
        alto,
    }
}

/// El rectangulo mas pequeno que contiene a los dos. Con el se prepara la
/// ventana antes de un zoom, para que quepa el recorrido entero.
fn envolvente(a: Rect, b: Rect) -> Rect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let derecha = (a.x + a.ancho as i32).max(b.x + b.ancho as i32);
    let abajo = (a.y + a.alto as i32).max(b.y + b.alto as i32);
    Rect {
        x,
        y,
        ancho: (derecha - x).max(1) as u32,
        alto: (abajo - y).max(1) as u32,
    }
}

/// Parte pura de `ventana_visible`: recorta `ideal` al escritorio. Si no
/// se tocan (un pin arrastrado fuera del todo), conserva la posicion y
/// solo limita el tamano.
pub fn recortar_al_escritorio(ideal: Rect, escritorio: Rect) -> Rect {
    match ideal.interseccion(escritorio) {
        Some(r) if r.ancho > 0 && r.alto > 0 => r,
        _ => Rect {
            x: ideal.x,
            y: ideal.y,
            ancho: ideal.ancho.min(escritorio.ancho),
            alto: ideal.alto.min(escritorio.alto),
        },
    }
}

/// Donde empieza el contenido dentro de la ventana real, en pixeles de
/// ventana. Sin recorte es el margen de sombra; con recorte, menos lo que
/// quedo fuera. Se pregunta a la ventana de verdad para no llevar un
/// segundo estado que pueda desincronizarse.
fn origen_contenido(hwnd: HWND, contenido: Rect) -> (i32, i32) {
    let mut r = RECT::default();
    // SAFETY: GetWindowRect sobre ventana propia.
    unsafe {
        let _ = GetWindowRect(hwnd, &mut r);
    }
    (contenido.x - r.left, contenido.y - r.top)
}

/// Todo lo que el WndProc necesita, colgado de GWLP_USERDATA.
struct PinInterno {
    /// La propia ventana: el pintado necesita saber donde esta de verdad
    /// para desplazar el contenido cuando la ventana esta recortada.
    hwnd: HWND,
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
    /// El contenido tapa la tarjeta por completo (una captura opaca), asi
    /// que no hace falta pintarla debajo.
    tapa_la_tarjeta: bool,
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
    /// El cursor que toca mientras se anota; lo pone el gestor al cambiar de
    /// herramienta.
    cursor_anotacion: CursorAnotacion,
    /// Zoom del texto de una nota (1.0 = como nacio). Lo cambian la rueda y
    /// Ctrl + arrastrar (en proporcion); estirar la caja por la esquina no.
    zoom_texto: f32,
    /// Zoom animado en curso (la rueda): de donde a donde y desde cuando.
    zoom: Option<ZoomEnCurso>,
    /// El rect de contenido con el que se pinto la superficie por ultima
    /// vez. Mientras dura una animacion, la superficie sigue teniendo ese
    /// dibujo y el compositor lo estira; de aqui sale la transformada.
    base_pintado: Cell<Rect>,
    /// La ventana que habia al pintar esa base. Mientras se persigue un
    /// destino la ventana NO se mueve, asi que este rect y el de verdad son
    /// el mismo, y de ahi sale la transformada.
    base_ventana: Cell<Rect>,
    /// Cuartos de vuelta a la derecha, de 0 a 3, y volteos. No tocan los
    /// pixeles: son una transformada al dibujar, asi que girar y volver a
    /// girar deja la imagen exactamente como estaba.
    giro: u8,
    volteo_h: bool,
    volteo_v: bool,
    /// Zoom del CONTENIDO dentro de la ventana, que no cambia de tamano
    /// (Ctrl + rueda). 1.0 = el contenido cabe justo. Con mas, se ve un
    /// trozo mas grande y el resto se alcanza arrastrando con el boton
    /// central. Es lo que pidio el usuario para las notas: mismo tamano de
    /// caja, mas texto a la vista.
    vista_escala: f32,
    vista_dx: f32,
    vista_dy: f32,
    /// Desde donde se empezo a arrastrar con el boton central, para panear.
    paneo: Option<(i32, i32, f32, f32)>,
    /// El reproductor, solo en un pin de video (D63). Si no pudo crearse,
    /// `video_fallido` avisa al gestor en el primer tick (D72).
    video: Option<Reproductor>,
    video_fallido: bool,
    /// Cada cuanto se pregunta por un fotograma nuevo (D67): 16 ms en
    /// `Completo`, 33 en `Ligero`.
    ritmo_video_ms: u32,
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
    #[allow(clippy::too_many_arguments)] // el pin nace con todo lo que no cambia en su vida
    pub fn nuevo(
        d3d: &ID3D11Device,
        motor: Rc<MotorRender>,
        contenido: Contenido,
        rect_contenido: Rect,
        escala_por_cien: u32,
        tema_claro: bool,
        ritmo_video_ms: u32,
        al_cambiar: Box<dyn Fn(CambioPin)>,
    ) -> Result<Pin, ErrorPin> {
        REGISTRO.call_once(registrar_clase);
        let ventana = ventana_visible(rect_contenido, escala_por_cien);
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
        // Una captura opaca cubre la tarjeta entera: pintarla debajo es un
        // relleno del tamano del pin desperdiciado en cada fotograma. Se
        // pregunta una sola vez, aqui.
        let tapa_la_tarjeta = match &contenido {
            Contenido::Imagen(img) => img.es_opaca(),
            _ => false,
        };
        let imagen_nativa = match &contenido {
            Contenido::Imagen(img) => (img.ancho, img.alto),
            Contenido::Documento { vista, .. } => (vista.ancho, vista.alto),
            Contenido::Video { ancho, alto, .. } if *ancho > 0 && *alto > 0 => (*ancho, *alto),
            // Sin tamano nativo de pixeles: el "100 %" de una nota o una
            // ficha es el tamano con el que nacio.
            _ => (rect_contenido.ancho, rect_contenido.alto),
        };

        let estado = if !contenido.redimensionable() {
            EstadoPin::nuevo_fijo(rect_contenido, escala_por_cien)
        } else if contenido.redimension_libre() {
            EstadoPin::nuevo_libre(rect_contenido, escala_por_cien)
        } else if contenido.solo_ancho() {
            EstadoPin::nuevo_solo_ancho(rect_contenido, escala_por_cien)
        } else {
            EstadoPin::nuevo(rect_contenido, escala_por_cien)
        };

        // El reproductor nace con el pin (D63). Si no puede crearse, el pin
        // existe igual y el gestor se entera en el primer tick (D72): un
        // error modal aqui dejaria al usuario sin pin y sin explicacion.
        let (video, video_fallido) = match &contenido {
            Contenido::Video { ruta, .. } => match Reproductor::nuevo(d3d, ruta) {
                Ok(r) => (Some(r), false),
                Err(e) => {
                    tracing::warn!(?e, "sin reproductor; el video caera a documento");
                    (None, true)
                }
            },
            _ => (None, false),
        };

        let interno = Box::new(PinInterno {
            estado,
            escala_por_cien,
            motor,
            d3d: d3d.clone(),
            superficie,
            imagen_nativa,
            bitmap,
            tapa_la_tarjeta,
            contenido,
            tema_claro,
            color_sombra: None,
            enfocado: false,
            textos: None,
            anotaciones: Vec::new(),
            anotando: false,
            lupa: None,
            cursor_anotacion: CursorAnotacion::Cruz,
            zoom_texto: 1.0,
            zoom: None,
            base_pintado: Cell::new(rect_contenido),
            base_ventana: Cell::new(ventana),
            giro: 0,
            volteo_h: false,
            volteo_v: false,
            vista_escala: 1.0,
            vista_dx: 0.0,
            vista_dy: 0.0,
            paneo: None,
            hwnd,
            video,
            video_fallido,
            ritmo_video_ms,
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
        // Un video arranca su temporizador ya (D67); si el reproductor no
        // pudo crearse, el temporizador es lo que lleva el aviso al gestor.
        if matches!(contenido_es_video(hwnd), Some(true)) {
            armar_temporizador_video(hwnd, ritmo_video_ms);
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
    /// Coloca el pin en un rect nuevo sin tocar el zoom del texto.
    pub fn poner_rect(&self, contenido: Rect) {
        if let Some(i) = interno_de(self.hwnd) {
            i.estado.poner_rect(contenido);
        }
        aplicar(self.hwnd, EfectoPin::Redimensionar(contenido));
    }

    /// Escala el pin en proporcion a un rect nuevo, texto incluido si es
    /// una nota. Es lo que hace la rueda: el gesto Ctrl + arrastrar llega
    /// por el mismo camino desde la maquina de estado.
    pub fn escalar(&self, contenido: Rect) {
        if let Some(i) = interno_de(self.hwnd) {
            i.zoom = None;
            i.estado.poner_rect(contenido);
        }
        aplicar(self.hwnd, EfectoPin::Escalar(contenido));
    }

    /// Como `escalar`, pero persiguiendo el destino en vez de saltar a el
    /// (la rueda). Volver a llamarlo con otro destino NO reinicia nada: la
    /// persecucion sigue desde donde iba, que es lo que hace que encadenar
    /// muescas salga continuo.
    pub fn escalar_persiguiendo(&self, contenido: Rect) {
        let Some(i) = interno_de(self.hwnd) else {
            return;
        };
        // La ventana tiene que dar cabida a TODO el recorrido antes de
        // empezar: al de ahora y al de destino. Asi se agranda una vez por
        // muesca en vez de en cada fotograma, y se dibuja de verdad en ese
        // mismo instante, de modo que nunca hay un hueco transparente. Lo
        // que sobre queda transparente, que es lo que el pin va a ocupar de
        // todas formas al terminar.
        preparar_ventana_para(self.hwnd, i, contenido);
        match i.zoom.as_mut() {
            Some(z) => z.control.pedir(contenido),
            None => {
                let actual = i.estado.rect();
                let mut control = crate::zoom::ControlZoom::nuevo(actual);
                control.pedir(contenido);
                i.zoom = Some(ZoomEnCurso {
                    control,
                    ultimo_paso: std::time::Instant::now(),
                    zoom_texto_inicial: i.zoom_texto,
                    ancho_inicial: actual.ancho,
                });
            }
        }
        // SAFETY: temporizador sobre ventana propia; se mata al llegar.
        unsafe {
            SetTimer(Some(self.hwnd), ID_TEMPORIZADOR_ZOOM, RITMO_ZOOM_MS, None);
        }
    }

    /// El rect al que va el pin: el destino que persigue si hay uno, o el
    /// actual. La rueda encadena pasos desde aqui y no desde el fotograma
    /// intermedio, que iria por detras de la mano.
    pub fn rect_objetivo(&self) -> Rect {
        match interno_de(self.hwnd) {
            Some(i) => i
                .zoom
                .as_ref()
                .map(|z| z.control.objetivo())
                .unwrap_or(i.estado.rect()),
            None => self.rect_contenido(),
        }
    }

    /// Como esta girado y volteado: cuartos de vuelta a la derecha (0 a 3)
    /// y los dos volteos. Lo guarda el gestor para devolverlo asi.
    pub fn giro(&self) -> (u8, bool, bool) {
        interno_de(self.hwnd)
            .map(|i| (i.giro, i.volteo_h, i.volteo_v))
            .unwrap_or((0, false, false))
    }

    /// Lo pone al restaurar del almacen y repinta.
    pub fn poner_giro(&self, giro: u8, volteo_h: bool, volteo_v: bool) {
        if let Some(i) = interno_de(self.hwnd) {
            i.giro = giro % 4;
            i.volteo_h = volteo_h;
            i.volteo_v = volteo_v;
            pintar(i);
        }
    }

    /// Zoom del texto de una nota, en por ciento (100 = como nacio).
    pub fn zoom_por_cien(&self) -> u32 {
        interno_de(self.hwnd)
            .map(|i| (i.zoom_texto * 100.0).round().max(1.0) as u32)
            .unwrap_or(100)
    }

    /// El zoom que tendra el texto cuando el pin llegue a `hasta` desde su
    /// destino en curso: lo que el gestor guarda al girar la rueda, sin
    /// esperar a que la animacion termine.
    pub fn zoom_objetivo_por_cien(&self, hasta: Rect) -> u32 {
        let Some(i) = interno_de(self.hwnd) else {
            return 100;
        };
        if !matches!(i.contenido, Contenido::Nota { .. }) {
            return 100;
        }
        let (zoom, rect) = match i.zoom.as_ref() {
            Some(z) => {
                let objetivo = z.control.objetivo();
                (z.zoom_texto_en(objetivo.ancho), objetivo)
            }
            None => (i.zoom_texto, i.estado.rect()),
        };
        (zoom * 100.0 * hasta.ancho as f32 / rect.ancho.max(1) as f32)
            .round()
            .max(1.0) as u32
    }

    /// Fija el zoom del texto (al restaurar del almacen) y repinta.
    pub fn poner_zoom_por_cien(&self, zoom: u32) {
        if let Some(i) = interno_de(self.hwnd) {
            i.zoom_texto = (zoom.max(1) as f32) / 100.0;
            pintar(i);
        }
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

    /// Si es un pin de video y esta reproduciendose.
    pub fn reproduciendo(&self) -> bool {
        interno_de(self.hwnd).is_some_and(|i| i.video.as_ref().is_some_and(|v| v.reproduciendo()))
    }

    /// Si es un pin de video y esta silenciado (D69).
    pub fn silenciado(&self) -> bool {
        interno_de(self.hwnd).is_some_and(|i| i.video.as_ref().is_some_and(|v| v.silenciado()))
    }
}

/// Si la ventana es un pin de video. `None` si ya no existe.
fn contenido_es_video(hwnd: HWND) -> Option<bool> {
    interno_de(hwnd).map(|i| matches!(i.contenido, Contenido::Video { .. }))
}

fn armar_temporizador_video(hwnd: HWND, ritmo_ms: u32) {
    // SAFETY: temporizador de ventana propia; volver a armarlo con el mismo
    // id solo cambia el intervalo.
    unsafe {
        SetTimer(Some(hwnd), ID_TEMPORIZADOR_VIDEO, ritmo_ms.max(1), None);
    }
}

/// Reproducir o pausar (D68): el temporizador va con el estado, asi que un
/// video en pausa no cuesta un solo tick (D67).
fn alternar_video(hwnd: HWND, i: &mut PinInterno) {
    let Some(v) = &i.video else {
        return;
    };
    v.alternar_pausa();
    if v.reproduciendo() {
        armar_temporizador_video(hwnd, i.ritmo_video_ms);
    } else {
        // SAFETY: mata el temporizador propio.
        unsafe {
            let _ = KillTimer(Some(hwnd), ID_TEMPORIZADOR_VIDEO);
        }
    }
}

impl Pin {
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
        let (ox, oy) = origen_contenido(self.hwnd, i.estado.rect());
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
                    x: p.x + ox,
                    y: p.y + oy,
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

    /// El cursor mientras se anota. Ademas de guardarlo, lo aplica ya si el
    /// raton esta sobre el pin: sin eso el cambio se veria solo al mover.
    pub fn poner_cursor_anotacion(&self, cursor: CursorAnotacion) {
        if let Some(i) = interno_de(self.hwnd) {
            i.cursor_anotacion = cursor;
            // SAFETY: un mensaje sincrono a la propia ventana; Windows lo
            // ignora si el raton no esta encima.
            unsafe {
                let _ = SendMessageW(
                    self.hwnd,
                    WM_SETCURSOR,
                    Some(WPARAM(self.hwnd.0 as usize)),
                    Some(LPARAM(HTCLIENT as isize)),
                );
            }
        }
    }

    /// Si el pin acepta redimension (la ficha y la nota no).
    pub fn redimensionable(&self) -> bool {
        interno_de(self.hwnd).is_some_and(|i| !i.estado.es_fijo())
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
/// Un fotograma intermedio de la animacion de zoom, SIN redibujar nada: la
/// ventana toma su tamano nuevo y el compositor estira lo ya dibujado (la
/// tecnica que usan los visores rapidos; redibujar el contenido en cada
/// fotograma es lo que producia tirones en graficos integrados).
///
/// La superficie contiene el pin tal como estaba en `base_pintado`. El zoom
/// lleva un punto `q` de aquel pin a `rect.origen + (q - base.origen) * s`.
/// Pasando eso a coordenadas de la ventana nueva sale la transformada.
/// Deja la ventana con sitio para todo el recorrido de un zoom: el tamano
/// de ahora y el de destino a la vez. Dibuja de verdad al agrandarla, para
/// que el compositor nunca tenga que ensenar un hueco.
fn preparar_ventana_para(hwnd: HWND, i: &mut PinInterno, destino: Rect) {
    let necesaria = envolvente(
        ventana_visible(i.estado.rect(), i.escala_por_cien),
        ventana_visible(destino, i.escala_por_cien),
    );
    if necesaria == i.base_ventana.get() {
        return;
    }
    // SAFETY: SetWindowPos sobre ventana propia. Sin redibujado del sistema:
    // el dibujo lo hace `pintar` justo despues, y en el mismo turno.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            necesaria.x,
            necesaria.y,
            necesaria.ancho as i32,
            necesaria.alto as i32,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW,
        );
    }
    if let Err(e) = i.superficie.asegurar(necesaria.ancho, necesaria.alto) {
        tracing::warn!(?e, "no se pudo agrandar la superficie para el zoom");
    }
    // Este dibujo deja la base desde la que estiran los fotogramas
    // siguientes; sin el, la transformada partiria de un tamano que ya no es.
    pintar(i);
}

/// Amplia o reduce el contenido dentro de la ventana, anclando en el cursor.
/// `lparam` viene de `WM_MOUSEWHEEL`, que trae el punto en coordenadas de
/// pantalla.
fn ajustar_vista(i: &mut PinInterno, paso: f32, lparam: LPARAM) {
    let contenido = i.estado.rect();
    let (cx, cy) = (
        (lparam.0 & 0xFFFF) as i16 as f32 - contenido.x as f32,
        ((lparam.0 >> 16) & 0xFFFF) as i16 as f32 - contenido.y as f32,
    );

    let antes = i.vista_escala;
    // Menos de 1 no tiene sentido: el contenido ya cabe justo, y encoger
    // solo dejaria huecos dentro de la tarjeta.
    let ahora = (antes * paso).clamp(1.0, 40.0);
    // El punto bajo el cursor se queda donde esta: el desplazamiento se
    // corrige por la diferencia de escalas.
    i.vista_dx = cx - (cx - i.vista_dx) * ahora / antes;
    i.vista_dy = cy - (cy - i.vista_dy) * ahora / antes;
    i.vista_escala = ahora;
    limitar_vista(i);
}

/// Impide que el contenido se despegue de la tarjeta y deje un hueco: el
/// desplazamiento vive entre «pegado al borde derecho» y cero.
fn limitar_vista(i: &mut PinInterno) {
    let r = i.estado.rect();
    let sobra_x = r.ancho as f32 * (1.0 - i.vista_escala);
    let sobra_y = r.alto as f32 * (1.0 - i.vista_escala);
    i.vista_dx = i.vista_dx.clamp(sobra_x.min(0.0), 0.0);
    i.vista_dy = i.vista_dy.clamp(sobra_y.min(0.0), 0.0);
    if i.vista_escala <= 1.0 {
        i.vista_dx = 0.0;
        i.vista_dy = 0.0;
    }
}

fn estirar_hasta(_hwnd: HWND, i: &PinInterno, rect: Rect) {
    let base = i.base_pintado.get();
    if base.ancho == 0 || base.alto == 0 {
        return;
    }
    // La ventana NO se toca aqui, y ese es todo el arreglo del parpadeo:
    // `SetWindowPos` es inmediato pero el compositor confirma la nueva
    // transformada en su siguiente fotograma, asi que al agrandar quedaba un
    // instante con la ventana ya grande y el contenido todavia pequeno. Por
    // ese hueco se veia el escritorio de detras. Ahora la ventana se prepara
    // UNA vez, al fijar el destino, y aqui solo se estira dentro de ella.
    let base_ventana = i.base_ventana.get();
    let v = base_ventana;
    let sx = rect.ancho as f32 / base.ancho as f32;
    let sy = rect.alto as f32 / base.alto as f32;
    let dx = rect.x as f32 + (base_ventana.x - base.x) as f32 * sx - v.x as f32;
    let dy = rect.y as f32 + (base_ventana.y - base.y) as f32 * sy - v.y as f32;
    i.superficie.estirar(sx, sy, dx, dy);
}

/// El zoom del texto en por ciento, para persistirlo (100 fuera de las
/// notas: los demas contenidos no tienen zoom de texto).
/// Lo que se guarda de un pin en un momento dado, para un rect concreto.
fn colocacion_de(i: &PinInterno, rect: Rect) -> Colocacion {
    Colocacion {
        rect,
        zoom_por_cien: zoom_por_cien_de(i),
        giro: i.giro,
        volteo_h: i.volteo_h,
        volteo_v: i.volteo_v,
    }
}

fn zoom_por_cien_de(i: &PinInterno) -> u32 {
    if matches!(i.contenido, Contenido::Nota { .. }) {
        (i.zoom_texto * 100.0).round().max(1.0) as u32
    } else {
        100
    }
}

fn pintar(i: &PinInterno) {
    let destino = match i.superficie.empezar(&i.motor) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(?e, "el pin no pudo empezar a pintar");
            return;
        }
    };
    let escala = i.escala_por_cien as f32 / 100.0;
    let m = MARGEN_SOMBRA_LOGICO as f32 * escala;
    let contenido = i.estado.rect();
    // Se dibuja de verdad: fuera la transformada de estirado y esta pasa a
    // ser la base desde la que estiraran los siguientes fotogramas.
    i.superficie.dejar_de_estirar();
    i.base_pintado.set(contenido);
    {
        let mut r = RECT::default();
        // SAFETY: GetWindowRect sobre ventana propia.
        unsafe {
            let _ = GetWindowRect(i.hwnd, &mut r);
        }
        i.base_ventana.set(Rect {
            x: r.left,
            y: r.top,
            ancho: (r.right - r.left).max(0) as u32,
            alto: (r.bottom - r.top).max(0) as u32,
        });
    }
    let (w, h) = (contenido.ancho as f32, contenido.alto as f32);
    let radio = 8.0 * escala;
    // Con la ventana recortada al escritorio, el contenido no empieza en el
    // margen de sombra sino donde le toque: todo lo de abajo pinta como si
    // la ventana fuera entera y el desplazamiento lo corrige.
    let (ox, oy) = origen_contenido(i.hwnd, contenido);

    let _ = i.motor.dibujar(&destino, |p| {
        p.limpiar_transparente();
        p.desplazar(ox as f32 - m, oy as f32 - m);
        // Sombra difusa: seis aros redondeados concentricos de alfa
        // decreciente, desplazados hacia abajo. Sin desenfoque real y
        // suficiente para el look de recorte elevado (D30). El cache por
        // bitmap de la spec queda para S2-B.
        let desplome = 2.0 * escala;
        // Sin grupo la sombra es negra; con grupo toma su color (D24). El
        // pin enfocado la lleva mas intensa y algo mas amplia.
        let (sr, sg, sb) = i.color_sombra.unwrap_or((0.0, 0.0, 0.0));
        let refuerzo = if i.enfocado { 1.7 } else { 1.0 };
        // Los seis aros se pintan SOLO en el anillo que rodea a la tarjeta:
        // debajo de ella quedan tapados, y rellenarlos costaba seis veces el
        // area del pin en cada fotograma. Con un pin a pantalla completa eso
        // era el tiron que veia el usuario al hacer zoom. Las cuatro bandas
        // son disjuntas (si se solaparan, el alfa se sumaria dos veces) y
        // entre todas cubren lo que la tarjeta redondeada deja fuera.
        let aros = |p: &pixpin_render::Pintor| {
            for (paso, alfa) in [0.10f32, 0.08, 0.06, 0.045, 0.03, 0.02].iter().enumerate() {
                let crece =
                    (paso as f32 + 1.0) * 2.0 * escala * if i.enfocado { 1.25 } else { 1.0 };
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
        };
        // Arriba y abajo van a lo ancho y entran `radio` en la tarjeta: asi
        // cubren sus cuatro esquinas redondeadas. Los lados van entre ambas.
        let ancho_total = w + 2.0 * m;
        let alto_banda = m + radio;
        for banda in [
            RectF {
                x: 0.0,
                y: 0.0,
                ancho: ancho_total,
                alto: alto_banda,
            },
            RectF {
                x: 0.0,
                y: m + h - radio,
                ancho: ancho_total,
                alto: alto_banda,
            },
            RectF {
                x: 0.0,
                y: alto_banda,
                ancho: m,
                alto: (h - 2.0 * radio).max(0.0),
            },
            RectF {
                x: m + w,
                y: alto_banda,
                ancho: m,
                alto: (h - 2.0 * radio).max(0.0),
            },
        ] {
            if banda.ancho > 0.0 && banda.alto > 0.0 {
                p.con_recorte(banda, aros);
            }
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
        if !i.tapa_la_tarjeta {
            p.rellenar_redondeado(caja, radio, lienzo);
        }

        // El zoom con la ventana bloqueada: se amplia lo de dentro y se
        // recorta a la tarjeta, para que lo que se sale no invada la sombra.
        // La tarjeta y la sombra quedan fuera a proposito: son el marco, y
        // el marco no se amplia.
        let con_vista = i.vista_escala > 1.0;
        if con_vista {
            p.empujar_recorte(caja);
            p.poner_vista(
                (ox as f32 - m, oy as f32 - m),
                i.vista_escala,
                (i.vista_dx, i.vista_dy),
            );
        }

        match &i.contenido {
            // La imagen va sin recorte redondeado (simplificacion consciente
            // heredada de S2-A): el redondeo se aprecia en la sombra.
            // El video es una imagen en movimiento: el bitmap es el ultimo
            // fotograma, o nada hasta que llegue el primero (D63).
            Contenido::Imagen(_) | Contenido::Video { .. } => {
                if let Some(b) = &i.bitmap {
                    if i.giro == 0 && !i.volteo_h && !i.volteo_v {
                        p.bitmap(b, caja, None, false);
                    } else {
                        // Girado un cuarto, la imagen entra en la caja con
                        // los lados cambiados: se dibuja en la caja
                        // traspuesta y la transformada la coloca.
                        let destino = if i.giro % 2 == 1 {
                            RectF {
                                x: caja.x + (caja.ancho - caja.alto) / 2.0,
                                y: caja.y + (caja.alto - caja.ancho) / 2.0,
                                ancho: caja.alto,
                                alto: caja.ancho,
                            }
                        } else {
                            caja
                        };
                        let centro = (caja.x + caja.ancho / 2.0, caja.y + caja.alto / 2.0);
                        p.con_giro(
                            (ox as f32 - m, oy as f32 - m),
                            centro,
                            i.giro,
                            i.volteo_h,
                            i.volteo_v,
                            |p| p.bitmap(b, destino, None, false),
                        );
                    }
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

            // La nota es Markdown (titulos, listas, codigo...) y su texto
            // lleva el zoom de la rueda / Ctrl + arrastrar; estirar la caja
            // por la esquina solo recoloca el texto al ancho nuevo.
            Contenido::Nota { texto } => {
                let z = i.zoom_texto;
                let margen = NOTA_MARGEN_LOGICO * escala * z;
                let tam = NOTA_TEXTO_LOGICO * escala * z;
                let bloques = crate::markdown::analizar(texto);
                let ancho_texto = (w - 2.0 * margen).max(1.0);
                let d = crate::markdown::disponer(
                    &bloques,
                    ancho_texto,
                    tam,
                    &|t, tam, max, tramos| p.medir_parrafo(t, tam, max, tramos),
                );
                pintar_markdown(
                    p,
                    &bloques,
                    &d,
                    m + margen,
                    m + margen,
                    ancho_texto,
                    tam,
                    tinta,
                    tinta_tenue,
                );
            }

            Contenido::Archivo {
                nombre,
                detalle,
                existe,
                ..
            } => {
                let margen = FICHA_MARGEN_LOGICO * escala;
                let lado = FICHA_ICONO_LOGICO * escala;
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
                // Una linea con puntos suspensivos: un nombre largo no se
                // sale de la tarjeta (lo vio el usuario).
                p.texto_linea(
                    nombre,
                    x_texto,
                    m + h / 2.0 - 17.0 * escala,
                    FICHA_NOMBRE_LOGICO * escala,
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
                p.texto_linea(
                    detalle,
                    x_texto,
                    m + h / 2.0 + 2.0 * escala,
                    FICHA_DETALLE_LOGICO * escala,
                    ancho_texto,
                    color_detalle,
                );
            }
        }

        if con_vista {
            p.desplazar(ox as f32 - m, oy as f32 - m);
            p.soltar_recorte();
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

/// Pinta una nota ya dispuesta como Markdown: prefijos de lista, barra de
/// cita, fondo del codigo, reglas y los parrafos con sus tramos.
#[allow(clippy::too_many_arguments)] // geometria y colores de un solo pintado
fn pintar_markdown(
    p: &pixpin_render::Pintor,
    bloques: &[crate::markdown::Bloque],
    d: &crate::markdown::Disposicion,
    x0: f32,
    y0: f32,
    ancho_texto: f32,
    tam: f32,
    tinta: Color,
    tenue: Color,
) {
    use crate::markdown::{Tipo, tramos_de};
    let fondo_codigo = Color { a: 0.10, ..tinta };
    for c in &d.colocados {
        let b = &bloques[c.bloque];
        match b.tipo {
            Tipo::Regla => {
                p.rellenar(
                    RectF {
                        x: x0,
                        y: y0 + c.y + c.alto / 2.0,
                        ancho: ancho_texto,
                        alto: (tam * 0.08).max(1.0),
                    },
                    tenue,
                );
                continue;
            }
            Tipo::Codigo => {
                let relleno = tam * 0.4;
                p.rellenar_redondeado(
                    RectF {
                        x: x0 + c.x - tam * 0.4,
                        y: y0 + c.y - relleno,
                        ancho: (ancho_texto - c.x + tam * 0.4).max(1.0),
                        alto: c.alto + 2.0 * relleno,
                    },
                    tam * 0.3,
                    fondo_codigo,
                );
            }
            Tipo::Cita => {
                p.rellenar(
                    RectF {
                        x: x0 + c.x - tam * 0.7,
                        y: y0 + c.y,
                        ancho: tam * 0.2,
                        alto: c.alto,
                    },
                    tenue,
                );
            }
            _ => {}
        }
        if let Some(prefijo) = &c.prefijo {
            p.texto(prefijo, x0 + c.prefijo_x, y0 + c.y, c.tam, tinta);
        }
        p.parrafo(
            &b.texto,
            x0 + c.x,
            y0 + c.y,
            c.tam,
            (ancho_texto - c.x).max(1.0),
            &tramos_de(b),
            tinta,
        );
    }
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
            let v = ventana_visible(contenido, i.escala_por_cien);
            let mut actual = RECT::default();
            // SAFETY: GetWindowRect sobre ventana propia.
            unsafe {
                let _ = GetWindowRect(hwnd, &mut actual);
            }
            let mismo_tamano = (actual.right - actual.left) as u32 == v.ancho
                && (actual.bottom - actual.top) as u32 == v.alto;
            if !mismo_tamano {
                // Un pin mayor que la pantalla cambia de parte visible al
                // moverse: es una redimension de la ventana, no un traslado.
                aplicar(hwnd, EfectoPin::Redimensionar(contenido));
                return;
            }
            // SAFETY: SetWindowPos sobre ventana propia.
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
            if v != rect_ventana(contenido, i.escala_por_cien) {
                // Recortada: el contenido se desplaza dentro de la ventana
                // y hay que repintar. Sin recorte la composicion mueve el
                // visual entero y no hace falta.
                pintar(i);
            }
        }
        EfectoPin::Escalar(contenido) => {
            // En proporcion: el texto de una nota acompaña al tamano.
            let antes = i.estado.rect().ancho.max(1) as f32;
            if matches!(i.contenido, Contenido::Nota { .. }) {
                i.zoom_texto = (i.zoom_texto * contenido.ancho as f32 / antes).clamp(0.05, 50.0);
            }
            i.estado.poner_rect(contenido);
            // Escalar es proporcional, asi que estirar la textura da
            // exactamente el fotograma que tocaria dibujar, y gratis. El
            // dibujo nitido llega al soltar o, si el usuario se queda quieto
            // a media faena, cuando pare: cada cambio rearma el temporizador,
            // asi que solo dispara cuando de verdad ha dejado de moverse.
            preparar_ventana_para(hwnd, i, contenido);
            estirar_hasta(hwnd, i, contenido);
            // SAFETY: temporizador sobre ventana propia; se mata al disparar.
            unsafe {
                SetTimer(Some(hwnd), ID_TEMPORIZADOR_REPOSO, REPOSO_ZOOM_MS, None);
            }
        }
        EfectoPin::Redimensionar(contenido) => {
            let t0 = std::time::Instant::now();
            let v = ventana_visible(contenido, i.escala_por_cien);
            // SAFETY: SetWindowPos sobre ventana propia, con tamano.
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
            // La superficie cubre al menos la ventana; crece con margen
            // para que un zoom no reasigne memoria en cada fotograma, y se
            // compacta al acabar el gesto. Si no puede, se recrea.
            if let Err(e) = i.superficie.asegurar(v.ancho, v.alto) {
                tracing::warn!(?e, ancho = v.ancho, alto = v.alto, "ResizeBuffers fallo");
                match Superficie::nueva(&i.motor, &i.d3d, hwnd, v.ancho, v.alto) {
                    Ok(s) => i.superficie = s,
                    Err(e) => tracing::warn!(
                        ?e,
                        ancho = v.ancho,
                        alto = v.alto,
                        "no se pudo recrear la superficie del pin"
                    ),
                }
            }
            pintar(i);
            // Un fotograma lento se anota: es la unica pista para el lag que
            // el usuario ve en su equipo y no se reproduce aqui.
            let ms = t0.elapsed().as_millis() as u64;
            if ms > 24 {
                tracing::info!(ms, ancho = v.ancho, alto = v.alto, "redimension lenta");
            }
        }
        EfectoPin::AlternarTamano => {
            if i.estado.es_fijo() {
                return;
            }
            let actual = i.estado.rect();
            if let Contenido::Nota { .. } = i.contenido {
                // La nota vuelve a como nacio: zoom 1 y su tamano natural.
                i.zoom_texto = 1.0;
                let motor = Rc::clone(&i.motor);
                let (nw, nh) = crate::contenido::tamano_natural(
                    &i.contenido,
                    i.escala_por_cien,
                    &|t, tam, max, tramos| motor.medir_parrafo(t, tam, max, tramos),
                );
                let nuevo = Rect {
                    x: actual.x,
                    y: actual.y,
                    ancho: nw,
                    alto: nh,
                };
                i.estado.poner_rect(nuevo);
                aplicar(hwnd, EfectoPin::Redimensionar(nuevo));
                if let Some(i2) = interno_de(hwnd) {
                    (i2.al_cambiar)(CambioPin::Redimensionado(colocacion_de(i2, nuevo)));
                }
                return;
            }
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
                (i2.al_cambiar)(CambioPin::Redimensionado(colocacion_de(i2, nuevo)));
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
            // Si el gesto dejo la textura estirada (un zoom con Ctrl), al
            // soltar se dibuja de verdad: es el fotograma que se queda.
            if i.superficie.esta_estirada() {
                aplicar(hwnd, EfectoPin::Redimensionar(pegado));
            }
            // Fin de gesto: la superficie vuelve a su tamano justo si el
            // gesto la dejo muy sobrada (histeresis del zoom).
            let v = ventana_visible(pegado, i.escala_por_cien);
            if i.superficie
                .compactar(v.ancho, v.alto)
                .is_ok_and(|hecho| hecho)
            {
                // ResizeBuffers descarta el contenido: repintar.
                pintar(i);
            }
            (i.al_cambiar)(CambioPin::Movido(colocacion_de(i, pegado)));
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
        // Con la ventana recortada al escritorio el contenido no empieza en
        // el margen de sombra: se pregunta donde esta de verdad.
        let (ox, oy) = origen_contenido(i.hwnd, i.estado.rect());
        Punto {
            x: (lparam.0 & 0xFFFF) as i16 as i32 - ox,
            y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32 - oy,
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
                    // Ctrl + arrastrar: zoom (arriba agranda, abajo encoge).
                    // Lo pidio el usuario como alternativa a la rueda.
                    // SAFETY: consulta pura del estado del teclado.
                    let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                    let evento = if ctrl {
                        EventoPin::EscalarPulsado(punto(lparam))
                    } else {
                        EventoPin::BotonPulsado(punto(lparam))
                    };
                    let e = i.estado.procesar(evento);
                    aplicar(hwnd, e);
                }
            }
            LRESULT(0)
        }
        // El boton central arrastra el contenido dentro de la ventana, que
        // es la pareja natural del zoom con Ctrl. No se usa el izquierdo
        // porque ese mueve el pin entero, y no se quiere elegir entre las
        // dos cosas con un modificador mas.
        WM_MBUTTONDOWN => {
            if let Some(i) = interno_de(hwnd) {
                if i.vista_escala > 1.0 {
                    // SAFETY: captura sobre ventana propia, se suelta abajo.
                    unsafe { SetCapture(hwnd) };
                    i.paneo = Some((
                        (lparam.0 & 0xFFFF) as i16 as i32,
                        ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
                        i.vista_dx,
                        i.vista_dy,
                    ));
                }
            }
            LRESULT(0)
        }
        WM_MBUTTONUP => {
            // SAFETY: libera la captura tomada arriba.
            unsafe {
                let _ = ReleaseCapture();
            }
            if let Some(i) = interno_de(hwnd) {
                i.paneo = None;
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE if interno_de(hwnd).is_some_and(|i| i.paneo.is_some()) => {
            if let Some(i) = interno_de(hwnd) {
                if let Some((x0, y0, dx0, dy0)) = i.paneo {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    i.vista_dx = dx0 + (x - x0) as f32;
                    i.vista_dy = dy0 + (y - y0) as f32;
                    limitar_vista(i);
                    pintar(i);
                }
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            if let Some(i) = interno_de(hwnd) {
                // La palabra alta del wparam trae el giro, con signo.
                let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as i32;
                // Con Ctrl la rueda NO cambia el tamano de la ventana: mira
                // el contenido de cerca dentro de la caja que ya hay. Es lo
                // que se quiere en una nota, donde agrandar la caja no cabe.
                if tecla_pulsada(VK_CONTROL) && !i.anotando {
                    let paso = 1.15f32.powf(delta as f32 / 120.0);
                    ajustar_vista(i, paso, lparam);
                    pintar(i);
                    return LRESULT(0);
                }
                // A diferencia del resto de mensajes del raton, WM_MOUSEWHEEL
                // trae el punto en coordenadas de PANTALLA, que es justo lo
                // que necesita el zoom anclado: no hay que convertir nada.
                let cursor = Punto {
                    x: (lparam.0 & 0xFFFF) as i16 as i32,
                    y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
                };
                (i.al_cambiar)(CambioPin::RuedaGirada { delta, cursor });
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
                    // El doble clic en un video reproduce o pausa (D68/D70).
                    alternar_video(hwnd, i);
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
                let reproduciendo = i.video.as_ref().is_some_and(|v| v.reproduciendo());
                match crate::menu::mostrar(
                    hwnd,
                    &i.contenido,
                    i.color_sombra.is_some(),
                    reproduciendo,
                    &t,
                ) {
                    None => {}
                    // Las dos que puede resolver la propia ventana se
                    // resuelven aqui: pedirselas al gestor solo daria un
                    // rodeo para volver al mismo sitio.
                    Some(crate::menu::CMD_TAMANO_ORIGINAL) => {
                        aplicar(hwnd, EfectoPin::AlternarTamano)
                    }
                    Some(crate::menu::CMD_CERRAR) => aplicar(hwnd, EfectoPin::Cerrar),
                    // Los del video tambien se resuelven aqui: el reproductor
                    // vive en esta ventana (D64/D68).
                    Some(crate::menu::CMD_REPRODUCIR) => alternar_video(hwnd, i),
                    Some(crate::menu::CMD_SONIDO) => {
                        if let Some(v) = &i.video {
                            v.alternar_sonido();
                            tracing::info!(
                                silenciado = v.silenciado(),
                                "sonido del video alternado"
                            );
                        }
                    }
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
        // 1, 2 y 3: tamano original exacto, ajustar a la pantalla y
        // rellenarla. Solo fuera del modo anotacion, donde las teclas son
        // texto. Un pin que no se redimensiona (la ficha) las ignora.
        WM_KEYDOWN
            if matches!(wparam.0 as u32, 0x31..=0x33)
                && interno_de(hwnd).is_some_and(|i| !i.anotando && !i.estado.es_fijo()) =>
        {
            if let Some(i) = interno_de(hwnd) {
                // Cualquiera de las tres devuelve la vista de dentro a su
                // sitio: si no, encajar el pin dejaria el contenido corrido.
                i.vista_escala = 1.0;
                limitar_vista(i);
                let vista = match wparam.0 as u32 {
                    0x31 => Vista::Original,
                    0x32 => Vista::Ajustar,
                    _ => Vista::Rellenar,
                };
                let nuevo = rect_para_vista(hwnd, i, vista);
                i.zoom = None;
                i.estado.poner_rect(nuevo);
                aplicar(hwnd, EfectoPin::Redimensionar(nuevo));
                if let Some(i) = interno_de(hwnd) {
                    (i.al_cambiar)(CambioPin::Redimensionado(colocacion_de(i, nuevo)));
                }
            }
            LRESULT(0)
        }
        // R gira a la derecha, Shift+R a la izquierda; H y V voltean. Como
        // el giro es de 90 grados, el pin intercambia ancho y alto.
        WM_KEYDOWN
            if matches!(wparam.0 as u32, x if x == b'R' as u32 || x == b'H' as u32 || x == b'V' as u32)
                && interno_de(hwnd).is_some_and(|i| !i.anotando) =>
        {
            if let Some(i) = interno_de(hwnd) {
                match wparam.0 as u32 {
                    x if x == b'R' as u32 => {
                        let cuartos = if tecla_pulsada(VK_SHIFT) { 3 } else { 1 };
                        i.giro = (i.giro + cuartos) % 4;
                        let r = i.estado.rect();
                        // Girar un cuarto de vuelta cambia la forma de la
                        // caja: se intercambian los lados alrededor del
                        // centro, para que el pin no salte de sitio.
                        let nuevo = Rect {
                            x: r.x + (r.ancho as i32 - r.alto as i32) / 2,
                            y: r.y + (r.alto as i32 - r.ancho as i32) / 2,
                            ancho: r.alto,
                            alto: r.ancho,
                        };
                        i.estado.poner_rect(nuevo);
                        i.imagen_nativa = (i.imagen_nativa.1, i.imagen_nativa.0);
                        aplicar(hwnd, EfectoPin::Redimensionar(nuevo));
                    }
                    x if x == b'H' as u32 => {
                        i.volteo_h = !i.volteo_h;
                        pintar(i);
                    }
                    _ => {
                        i.volteo_v = !i.volteo_v;
                        pintar(i);
                    }
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN if wparam.0 as u32 == VK_SPACE.0 as u32 => {
            // Espacio en un video enfocado: reproducir o pausar (D68).
            if let Some(i) = interno_de(hwnd) {
                if matches!(i.contenido, Contenido::Video { .. }) && !i.anotando {
                    alternar_video(hwnd, i);
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
        WM_TIMER if wparam.0 == ID_TEMPORIZADOR_VIDEO => {
            if let Some(i) = interno_de(hwnd) {
                let fallo = i.video_fallido || i.video.as_ref().is_some_and(|v| v.fallo());
                if fallo {
                    // SAFETY: mata el temporizador propio.
                    unsafe {
                        let _ = KillTimer(Some(hwnd), ID_TEMPORIZADOR_VIDEO);
                    }
                    i.video_fallido = true;
                    (i.al_cambiar)(CambioPin::VideoFallido);
                    return LRESULT(0);
                }
                // Los metadatos dan el tamano nativo: es el 100 % del menu.
                if let Some(dim) = i.video.as_mut().and_then(|v| v.dimensiones()) {
                    i.imagen_nativa = dim;
                }
                // Clonar la interfaz de la textura (una cuenta de referencia)
                // libera el prestamo del reproductor antes de tocar el bitmap.
                let textura = i.video.as_mut().and_then(|v| v.tick().cloned());
                if let Some(t) = textura {
                    if i.bitmap.is_none() {
                        i.bitmap = i.motor.bitmap_desde_textura(&t).ok();
                    }
                    pintar(i);
                }
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == ID_TEMPORIZADOR_ZOOM => {
            if let Some(i) = interno_de(hwnd) {
                let Some(z) = i.zoom.as_mut() else {
                    // SAFETY: mata el temporizador propio.
                    unsafe {
                        let _ = KillTimer(Some(hwnd), ID_TEMPORIZADOR_ZOOM);
                    }
                    return LRESULT(0);
                };
                // El tiempo REAL transcurrido, no el ritmo nominal del
                // temporizador: Windows entrega los WM_TIMER cuando puede, y
                // contar ticks haria que el zoom fuera mas lento cuanto mas
                // cargado estuviera el equipo. El controlador ya acota el
                // paso por su cuenta.
                let dt = z.ultimo_paso.elapsed().as_secs_f32();
                z.ultimo_paso = std::time::Instant::now();
                let rect = z.control.paso(dt);
                let zoom_texto = z.zoom_texto_en(rect.ancho);
                let fin = z.control.terminado();

                if matches!(i.contenido, Contenido::Nota { .. }) {
                    i.zoom_texto = zoom_texto;
                }
                i.estado.poner_rect(rect);
                if fin {
                    // Ya se ha llegado: se dibuja de verdad, nitido y con la
                    // superficie a su tamano. Es el unico dibujo de todo el
                    // zoom; los fotogramas intermedios solo estiran.
                    aplicar(hwnd, EfectoPin::Redimensionar(rect));
                    if let Some(i) = interno_de(hwnd) {
                        i.zoom = None;
                        let v = ventana_visible(rect, i.escala_por_cien);
                        if i.superficie.compactar(v.ancho, v.alto).is_ok_and(|h| h) {
                            pintar(i);
                        }
                    }
                    // SAFETY: mata el temporizador propio.
                    unsafe {
                        let _ = KillTimer(Some(hwnd), ID_TEMPORIZADOR_ZOOM);
                    }
                } else {
                    estirar_hasta(hwnd, i, rect);
                }
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == ID_TEMPORIZADOR_REPOSO => {
            // SAFETY: de un disparo; se mata siempre al entrar.
            unsafe {
                let _ = KillTimer(Some(hwnd), ID_TEMPORIZADOR_REPOSO);
            }
            if let Some(i) = interno_de(hwnd) {
                // Si hay una persecucion en marcha, ella se encarga del
                // dibujo nitido al llegar: repintar aqui seria trabajo doble.
                if i.zoom.is_none() && i.superficie.esta_estirada() {
                    aplicar(hwnd, EfectoPin::Redimensionar(i.estado.rect()));
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
                (i.al_cambiar)(CambioPin::Movido(colocacion_de(i, i.estado.rect())));
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
            let id = interno_de(hwnd)
                .map(|i| {
                    if i.anotando {
                        // Anotando no se mueve ni se redimensiona: el cursor
                        // es el de la herramienta.
                        return match i.cursor_anotacion {
                            CursorAnotacion::Cruz => IDC_CROSS,
                            CursorAnotacion::Texto => IDC_IBEAM,
                            CursorAnotacion::Flecha => IDC_ARROW,
                        };
                    }
                    // La posicion del cursor en pantalla, preguntada aqui:
                    // WM_SETCURSOR no trae coordenadas utiles.
                    let mut p = windows::Win32::Foundation::POINT::default();
                    // SAFETY: GetCursorPos escribe en la variable local.
                    unsafe {
                        let _ = GetCursorPos(&mut p);
                    }
                    if i.estado.sobre_esquina(Punto { x: p.x, y: p.y }) {
                        IDC_SIZENWSE
                    } else {
                        IDC_ARROW
                    }
                })
                .unwrap_or(IDC_ARROW);
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
                // Durante una animacion de zoom lo que se ve es la textura
                // estirada por el compositor: repintar aqui tiraria por
                // tierra justo el ahorro que hace fluido el zoom. El
                // fotograma final ya repinta nitido.
                if i.zoom.is_none() {
                    pintar(i);
                }
            }
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            // Windows limita por defecto el tamano de una ventana al de la
            // pantalla. Al agrandar un pin mas alla, SetWindowPos aplicaba
            // la posicion pero recortaba el tamano: el pin "se escapaba"
            // hacia arriba y a la izquierda sin crecer, y su superficie,
            // creada con el tamano pedido, quedaba mas grande que la
            // ventana, con la sombra cortada por abajo y por la derecha.
            // SAFETY: durante WM_GETMINMAXINFO, lparam apunta a un
            // MINMAXINFO valido que el sistema espera que se rellene.
            unsafe {
                let info = lparam.0 as *mut MINMAXINFO;
                if !info.is_null() {
                    let tope = windows::Win32::Foundation::POINT {
                        x: 32_000,
                        y: 32_000,
                    };
                    (*info).ptMaxTrackSize = tope;
                    (*info).ptMaxSize = tope;
                }
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
            16,
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
            16,
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
            16,
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
            16,
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
