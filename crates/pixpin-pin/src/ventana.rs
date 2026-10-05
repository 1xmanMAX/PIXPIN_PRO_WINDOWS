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
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_BACK, VK_MENU, VK_RETURN, VK_SPACE};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::contenido::{
    Contenido, DOCUMENTO_FRANJA_LOGICA, FICHA_DETALLE_LOGICO, FICHA_ICONO_LOGICO,
    FICHA_MARGEN_LOGICO, FICHA_NOMBRE_LOGICO, NOTA_MARGEN_LOGICO, NOTA_TEXTO_LOGICO,
};
use crate::estado::{EfectoPin, EstadoPin, EventoPin, MINIMO_LOGICO};
use crate::video::Reproductor;
use crate::vivo::{FuenteViva, MSG_FOTOGRAMA_VIVO};

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
    /// Si deja pasar los clics a lo que hay debajo (P1.4).
    pub pasante: bool,
    /// Los filtros de imagen, para que un pin en gris vuelva en gris.
    pub gris: bool,
    pub invertido: bool,
    pub brillo: i32,
    /// La opacidad en por ciento (v2): 100 es opaco.
    pub opacidad: u8,
}

// Sin Eq: MuestraPuntero lleva f32 (subpixel), que no lo implementa.
#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// Menu: leer el texto de la imagen y copiarlo (P4.2). Lo resuelve el
    /// gestor y no la ventana: el pin no conoce ni el motor de
    /// reconocimiento ni el portapapeles.
    TextoPedido,
    /// Cambiar de pagina en un PDF. El numero es relativo: +1 o -1.
    PaginaPedida(i32),
    /// Sacar la pagina que se ve como pin propio.
    ExtraerPaginaPedida,
    /// Sacar TODAS las paginas, una por pin.
    ExtraerTodasPedida,
    /// El pin quiere que se lea su texto (P4.4). Lo pide la primera vez
    /// que el raton pasa por encima, no al nacer: reconocer cuesta
    /// milisegundos y hacerlo al crear cada pin daria un tiron justo
    /// cuando el usuario acaba de capturar.
    ReconocerPedido,
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
    /// Menu de un pin de imagen: abrirlo en el lienzo del editor (D131). Lo
    /// resuelve el gestor, que conoce el almacen y el fichero de dibujo.
    AbrirLienzoPedido,
    /// En modo anotacion, el raton se reenvia tal cual en coordenadas del
    /// CONTENIDO (el margen de sombra ya descontado). El pin no sabe
    /// dibujar: quien lleva la maquina de anotar es el gestor, que vive en
    /// una capa que si puede ver `pixpin-ui`.
    PunteroPulsado(Punto),
    PunteroMovido(Punto),
    PunteroSoltado(Punto),
    /// Un punto de la entrada fina mientras se anota (E1): uno que Windows
    /// fusiono o uno del lapiz. En coordenadas del contenido, con subpixel.
    MuestraPuntero {
        x: f32,
        y: f32,
        presion: Option<f32>,
    },
    /// Rueda del raton. Positivo hacia arriba (D55).
    /// La rueda, con el punto de pantalla donde estaba el cursor: el zoom
    /// se ancla ahi, no en el centro del pin.
    RuedaGirada {
        delta: i32,
        cursor: Punto,
    },
    /// Un clic sobre un pin en vivo que maneja su zona a distancia. El punto
    /// va en pixeles DE LA ZONA (0,0 es su esquina): la ventana sabe que
    /// pixel se ve bajo el cursor, pero no donde esta la zona en la pantalla;
    /// eso lo sabe el gestor, que es quien manda el clic de verdad.
    ClicRemoto {
        x: i32,
        y: i32,
    },
    /// Pulsar, arrastrar y soltar sobre un pin en vivo que maneja su zona:
    /// de `(x0, y0)` a `(x1, y1)`, en pixeles de la zona.
    ArrastreRemoto {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
    },
    /// La rueda sobre un pin en vivo que maneja su zona: igual que el clic.
    RuedaRemota {
        x: i32,
        y: i32,
        delta: i32,
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
    /// Un clic en el panel de propiedades que acompana a la barra mientras
    /// se anota (el color y el grosor, como en el lienzo), en coordenadas de
    /// su ventana. Tambien lo produce una `Paleta`, no esta ventana.
    PanelPulsado(Punto),
    /// Media Foundation no pudo con el video (D72): el gestor vuelve a
    /// crear el pin como documento o ficha.
    VideoFallido,
    /// Menu de un pin en vivo: dejar lo que se ve como pin de imagen.
    /// Lo resuelve el gestor, que tiene la fuente y el almacen.
    CongelarPedido,
    /// Menu de una nota: convertirla en la herramienta de ese indice de
    /// `magia::MiniApp::TODAS` (C3).
    ConvertirPedido(u8),
    /// Menu de una pizarra: su fondo nuevo, color y pauta (C4).
    PizarraPedida {
        color: u8,
        pauta: u8,
    },
    /// Un clic dentro de una herramienta, en pixeles del contenido tal como
    /// se ve ahora (el margen de sombra ya descontado). El pin no sabe que
    /// hay pintado ahi: lo sabe el gestor, que pinto la disposicion.
    ClicInterior {
        x: i32,
        y: i32,
    },
    /// La rueda sobre una herramienta: desplaza su lista.
    RuedaInterior {
        delta: i32,
    },
    /// Una tecla con la herramienta enfocada (flechas, Intro, Supr, F2...).
    TeclaInterior {
        vk: u32,
        shift: bool,
        ctrl: bool,
        alt: bool,
    },
    /// Un caracter escrito con la herramienta enfocada, ya compuesto.
    CaracterInterior(char),
    /// El latido de una herramienta que se mueve sola (cronometro,
    /// temporizador): el pin ya se repinto, y el gestor mira si vencio algo.
    Latido,
    /// Menu «Mas»: abrir el panel «Pines abiertos» (v2).
    PinesAbiertosPedido,
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
/// El temporizador de un pin de video sin reproductor: solo sirve para
/// llevarle al gestor el aviso del fallo (D72). Con reproductor, el ritmo
/// lo marca el refresco del monitor (`MSG_TICK_VIDEO`).
const ID_TEMPORIZADOR_VIDEO: usize = 2;
/// El aviso del hilo que sigue el refresco del monitor: toca preguntar al
/// reproductor si hay fotograma nuevo.
const MSG_TICK_VIDEO: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 0x52;
/// `WM_MOUSELEAVE`, que el crate de Windows no exporta con los demas
/// mensajes de ventana.
const WM_RATON_FUERA: u32 = 0x02A3;
/// El zoom animado de la rueda: un tick por fotograma hasta llegar.
const ID_TEMPORIZADOR_ZOOM: usize = 3;
/// Un disparo tras el ultimo cambio de un gesto continuo (Ctrl + arrastrar),
/// para dibujar nitido sin esperar a que el usuario suelte el boton.
const ID_TEMPORIZADOR_REPOSO: usize = 4;
/// El latido de una herramienta que cambia sola (el cronometro que corre,
/// la cuenta atras). Solo mientras el gestor lo pida: parada, no despierta
/// a nadie, como `esperaDelReloj` del movil.
const ID_TEMPORIZADOR_LATIDO: usize = 5;
/// Mientras la barra del pin se ve, mira cada poco si el raton sigue en el
/// pin o en la barra; si no, la esconde. Con `WM_MOUSELEAVE` solo no basta:
/// pasar del pin a su barra es salir del pin.
const ID_TEMPORIZADOR_BARRA: usize = 6;
const RITMO_BARRA_MS: u32 = 120;
/// La opacidad mas baja: por debajo el pin se pierde de vista.
const OPACIDAD_MINIMA: f32 = 0.2;
const RITMO_ZOOM_MS: u32 = 16;

/// Lo que pinta dentro de una herramienta. Lo cuelga el gestor y el pin lo
/// llama en cada pintado con la caja del contenido, en las coordenadas del
/// pintor de ese momento: tiene que pintar dentro de ella y no tocar el
/// desplazamiento del pintor, que es el del pin.
pub type PintorInterior = Box<dyn Fn(&pixpin_render::Pintor, RectF)>;
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
    /// **El grafito de lo anotado**, cocido aparte y con su sitio entre las
    /// ordenes (`tinta::grafito::GrafitoSuelto`). Sin esto el grafito del
    /// editor o del movil salia liso en el pin.
    grafitos: Vec<pixpin_motor2d::tinta::grafito::GrafitoSuelto>,
    /// Los bitmaps de esos mapas: el pin repinta con cada movimiento de la
    /// lupa, y resubir los mapas cada vez seria pagar megas por nada.
    cache_grafito: std::cell::RefCell<pixpin_render::CacheGrafito>,
    /// Las cajas de los mosaicos, en pixeles de la imagen original. Lo unico
    /// que las usa es la lupa, que amplia el bitmap SIN anotaciones y sin
    /// esto seria una ventana al dato tapado.
    tapadas: Vec<(f32, f32, f32, f32)>,
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
    pasante: bool,
    /// Si el equipo sabe reconocer texto. Se pregunta una vez al crear el
    /// pin y no cada vez que se abre el menu: `TryCreateFromUserProfileLanguages`
    /// monta un motor entero, y hacerlo con el menu a medio abrir se nota.
    con_ocr: bool,
    /// El texto reconocido, en pixeles de la imagen NATIVA. `None` es
    /// «todavia no se ha mirado»; un vector vacio es «se miro y no hay
    /// texto», que son cosas distintas: sin distinguirlas se volveria a
    /// reconocer una y otra vez una imagen sin letras.
    texto_ocr: Option<Vec<pixpin_geom::seleccion_texto::Renglon>>,
    /// Ya se pidio el reconocimiento y se espera respuesta.
    ocr_pedido: bool,
    /// Las palabras marcadas ahora mismo.
    seleccion_texto: Vec<pixpin_geom::seleccion_texto::Sitio>,
    /// Donde empezo el arrastre de seleccion, mientras dura.
    ancla_texto: Option<pixpin_geom::seleccion_texto::Sitio>,
    /// Gris, invertido y brillo. No tocan el original: se aplican al
    /// rehacer el bitmap, igual que el giro se aplica al pintar.
    filtros: pixpin_codec::filtros::Filtros,
    /// Cuantas paginas tiene, si es un PDF. `None` es «no es un PDF, o
    /// todavia no se ha mirado»: distinguirlo de `Some(0)` evita volver a
    /// preguntarselo al sistema en cada apertura del menu.
    pdf_paginas: Option<u32>,
    /// La pagina que se ensena ahora, desde 0.
    pdf_pagina: u32,
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
    /// Arrastre con el boton derecho para hacer zoom (P3.3): la `y` de
    /// pantalla del ultimo giro emitido, y si ya se paso del umbral.
    ///
    /// Se guarda la ultima `y` emitida y no la inicial para poder mandar
    /// el giro a trozos segun se mueve la mano. Con la inicial habria que
    /// recalcular el zoom absoluto en cada mensaje, y eso choca con la
    /// animacion, que ya lleva su propio destino.
    arrastre_derecho: Option<(i32, bool)>,
    /// El fichero al que apunta el pin, cuando apunta a uno. Es lo que se
    /// lleva `Ctrl` + arrastrar hacia otra aplicacion.
    ///
    /// El video la trae dentro de su contenido, pero la ficha y el
    /// documento no (`Contenido::Archivo` y `Contenido::Documento` solo
    /// guardan lo que se pinta): para esos la pone el gestor con
    /// `poner_ruta`, que es quien la conoce.
    ruta_origen: Option<std::path::PathBuf>,
    /// El reproductor, solo en un pin de video (D63). Si no pudo crearse,
    /// `video_fallido` avisa al gestor en el primer tick (D72).
    video: Option<Reproductor>,
    video_fallido: bool,
    /// El raton esta encima: se ensena la barra del pin (v2).
    raton_encima: bool,
    /// Donde se pulso un video fuera de sus mandos: si al soltar no se ha
    /// movido, fue un clic, y un clic reproduce o pausa.
    pulsado_video: Option<(i32, i32)>,
    /// La proporcion del video ya se ajusto a la de sus metadatos.
    proporcion_video_hecha: bool,
    /// La zona de pantalla de un pin en vivo, puesta por el gestor tras
    /// crear la ventana (la fuente necesita su HWND para avisarla).
    fuente_viva: Option<Box<dyn FuenteViva>>,
    /// En pausa, los avisos de fotograma se ignoran y queda el ultimo.
    vivo_pausado: bool,
    /// El pin en vivo maneja su zona a distancia: el clic y la rueda van a
    /// lo que se ve, no al pin. Arrastrar sigue moviendolo.
    remoto: bool,
    /// Donde estaba el cursor (pantalla) al pulsar con el modo encendido. Al
    /// soltar se compara: si apenas se movio fue un clic y se reenvia; si
    /// no, fue mover el pin y no se reenvia nada.
    pulsado_remoto: Option<(i32, i32)>,
    /// Lo que pinta el gestor dentro de una herramienta.
    pintor_interior: Option<PintorInterior>,
    /// Donde se pulso (pantalla) sobre una herramienta. Al soltar, si apenas
    /// se movio, fue un clic para lo de dentro; si no, fue mover el pin.
    pulsado_interior: Option<(i32, i32)>,
    /// El color y la pauta de una pizarra, para su submenu; `None` si el pin
    /// no es una.
    pizarra: Option<(u8, u8)>,
    /// La opacidad del pin entero, de 0,2 a 1 (v2). Mayus + rueda y el menu.
    opacidad: f32,
    /// Donde quedaria el pin pegado a la guia que se ve mientras se
    /// arrastra; se pega al soltar (`guias.rs`).
    guia: Option<Rect>,
    /// El fichero con la imagen ENTERA cuando la que se ve se leyo reducida
    /// a la medida del pin (abrir una foto). Se lee de el, una vez, si el
    /// pin se acerca mas alla de lo leido o se saca la lupa; `None` cuando
    /// ya se tiene entera.
    completa: Option<std::path::PathBuf>,
    /// Si el ultimo fotograma se pinto y se presento sin fallo. Lo mira el
    /// gestor tras rehacer el dispositivo (y la prueba de la perdida).
    pintado_bien: Cell<bool>,
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
    /// Historial del raton del pin que se esta anotando. Uno basta: solo se
    /// anota un pin a la vez.
    static HISTORIAL: std::cell::RefCell<pixpin_shell::puntero::HistorialRaton> =
        const { std::cell::RefCell::new(pixpin_shell::puntero::HistorialRaton::nuevo()) };
    /// Lo que el ultimo WM_POINTERUPDATE de este pin dejo saber sobre el
    /// lapiz (Tarea 11): igual que ENTRADA_FINA en el overlay, para que
    /// `descartar_movimiento` distinga el WM_MOUSEMOVE que el lapiz ya
    /// cubrio de uno que es lo unico que hay.
    static ESTADO_LAPIZ: Cell<pixpin_shell::puntero::EstadoLapiz> =
        const { Cell::new(pixpin_shell::puntero::EstadoLapiz::SinDatos) };
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
            // El video no tiene bitmap fijo: lo trae cada fotograma. La
            // herramienta la pinta el gestor.
            Contenido::Nota { .. }
            | Contenido::Video { .. }
            | Contenido::Vivo { .. }
            | Contenido::Herramienta { .. } => None,
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
            Contenido::Vivo { ancho, alto } => (*ancho, *alto),
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

        // El video ya sabe a que fichero apunta; la ficha y el documento no
        // lo guardan, y su ruta llega despues por `poner_ruta`.
        let ruta_origen = match &contenido {
            Contenido::Video { ruta, .. } => Some(ruta.clone()),
            _ => None,
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
            grafitos: Vec::new(),
            cache_grafito: std::cell::RefCell::new(pixpin_render::CacheGrafito::nueva()),
            tapadas: Vec::new(),
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
            pasante: false,
            con_ocr: false,
            texto_ocr: None,
            ocr_pedido: false,
            seleccion_texto: Vec::new(),
            ancla_texto: None,
            filtros: pixpin_codec::filtros::Filtros::default(),
            pdf_paginas: None,
            pdf_pagina: 0,
            vista_escala: 1.0,
            vista_dx: 0.0,
            vista_dy: 0.0,
            paneo: None,
            arrastre_derecho: None,
            hwnd,
            ruta_origen,
            video,
            video_fallido,
            raton_encima: false,
            pulsado_video: None,
            proporcion_video_hecha: false,
            fuente_viva: None,
            vivo_pausado: false,
            remoto: false,
            pulsado_remoto: None,
            pintor_interior: None,
            pulsado_interior: None,
            pizarra: None,
            opacidad: 1.0,
            guia: None,
            completa: None,
            pintado_bien: Cell::new(false),
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
        // Un video arranca su ritmo ya (D67); si el reproductor no pudo
        // crearse, el temporizador es lo que lleva el aviso al gestor.
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

    /// Si el pin deja pasar los clics a lo que hay debajo.
    pub fn es_pasante(&self) -> bool {
        interno_de(self.hwnd).map(|i| i.pasante).unwrap_or(false)
    }

    /// Hace que el pin deje pasar los clics, o que vuelva a recogerlos.
    ///
    /// `WS_EX_TRANSPARENT` solo funciona en una ventana `WS_EX_LAYERED`,
    /// asi que se ponen y se quitan juntos. Y afecta SOLO al raton: si el
    /// pin tuviera el foco seguiria recibiendo teclas, asi que se le quita
    /// tambien.
    ///
    /// Ojo con la unica via de vuelta: un pin pasante no puede abrir su
    /// propio menu del clic derecho, porque el clic ya no llega. Se sale
    /// de aqui con el comando global, que esta en el menu de la bandeja.
    pub fn poner_pasante(&self, pasante: bool) {
        poner_pasante_en(self.hwnd, pasante);
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

    /// Dice a que fichero apunta el pin, para que `Ctrl` + arrastrar pueda
    /// llevarlo a otra aplicacion.
    ///
    /// Solo hace falta en la ficha y en el documento: el video la trae en su
    /// contenido y se toma al crear el pin. Sin ella, `Ctrl` + arrastrar
    /// sobre una ficha no hace nada, que es preferible a soltar en el
    /// destino un fichero que no es.
    /// Cambia la pagina que se ensena de un PDF, con su imagen ya dibujada
    /// por el gestor.
    ///
    /// La dibuja el gestor porque el lector de PDF vive en `pixpin-pdf`,
    /// que esta en la MISMA capa que este crate y no se puede llamar de
    /// lado. El pin solo ensena lo que le den.
    pub fn poner_pagina(&self, pagina: u32, cuantas: u32, vista: pixpin_codec::ImagenRgba) {
        if let Some(i) = interno_de(self.hwnd) {
            i.pdf_paginas = Some(cuantas);
            i.pdf_pagina = pagina;
            if let Contenido::Documento { vista: v, .. } = &mut i.contenido {
                *v = vista;
            }
            i.imagen_nativa = match &i.contenido {
                Contenido::Documento { vista, .. } => (vista.ancho, vista.alto),
                _ => i.imagen_nativa,
            };
            // El texto reconocido era el de la pagina ANTERIOR: dejarlo
            // seria ofrecer seleccionar palabras que ya no estan ahi.
            i.texto_ocr = None;
            i.ocr_pedido = false;
            i.seleccion_texto.clear();
            rehacer_bitmap(i);
            pintar(i);
        }
    }

    /// Cuantas paginas tiene el PDF, si ya se sabe.
    pub fn paginas(&self) -> Option<u32> {
        interno_de(self.hwnd).and_then(|i| i.pdf_paginas)
    }

    /// La pagina que se ensena ahora.
    pub fn pagina(&self) -> u32 {
        interno_de(self.hwnd).map(|i| i.pdf_pagina).unwrap_or(0)
    }

    /// Cuelga lo que pinta el gestor dentro de una herramienta y repinta ya.
    /// Se vuelve a colgar cada vez que cambia lo de dentro: el pintor lleva
    /// COPIAS, no prestamos, porque se llama en cada `WM_PAINT`.
    pub fn poner_pintor_interior(&self, pintor: PintorInterior) {
        if let Some(i) = interno_de(self.hwnd) {
            i.pintor_interior = Some(pintor);
            pintar(i);
        }
    }

    /// Enciende (`Some(ms)`) o apaga el latido de una herramienta. Cada
    /// latido repinta el pin y avisa al gestor con `CambioPin::Latido`.
    pub fn poner_latido(&self, cada_ms: Option<u32>) {
        // SAFETY: temporizadores de la ventana propia, desde su hilo. Matar
        // uno que no existe no hace nada.
        unsafe {
            match cada_ms {
                Some(ms) => {
                    SetTimer(Some(self.hwnd), ID_TEMPORIZADOR_LATIDO, ms.max(15), None);
                }
                None => {
                    let _ = KillTimer(Some(self.hwnd), ID_TEMPORIZADOR_LATIDO);
                }
            }
        }
    }

    /// Dice que el pin es una pizarra, con su color y su pauta de ahora,
    /// para que el menu ofrezca cambiarlos.
    /// **Pasa el pin a otro dispositivo grafico** (el suyo se perdio: driver
    /// actualizado, TDR, la grafica dedicada apagada...). Rehace en la misma
    /// ventana su superficie, el bitmap de la imagen (desde los pixeles que
    /// el pin ya guarda, con sus filtros) y el reproductor si es un video,
    /// y repinta. Sitio, tamano, zoom, giro, opacidad y anotaciones no se
    /// tocan: son estado del pin, no de la GPU.
    ///
    /// La fuente de un pin en vivo vive en el gestor y la cambia el.
    pub fn cambiar_dispositivo(
        &self,
        d3d: &ID3D11Device,
        motor: Rc<MotorRender>,
        ritmo_video_ms: u32,
    ) -> Result<(), ErrorPin> {
        let Some(i) = interno_de(self.hwnd) else {
            return Ok(());
        };
        i.motor = motor;
        i.d3d = d3d.clone();
        // Lo cacheado era del motor viejo.
        i.cache_grafito.borrow_mut().vaciar();
        i.bitmap = None;
        // La barra flotante tiene su propia superficie sobre el dispositivo
        // viejo: se suelta y renace con el nuevo la proxima vez.
        crate::barra_flotante::soltar();
        i.superficie.rehacer(&i.motor, d3d)?;
        rehacer_bitmap(i);
        // El video: un reproductor nuevo sobre el dispositivo nuevo, en el
        // mismo punto y con el mismo sonido.
        if let (Some(viejo), Contenido::Video { ruta, .. }) = (i.video.take(), &i.contenido) {
            let (segundos, _) = viejo.posicion();
            let sonaba = !viejo.silenciado();
            let pausado = !viejo.reproduciendo();
            drop(viejo);
            match Reproductor::nuevo(d3d, ruta) {
                Ok(r) => {
                    r.buscar(segundos, false);
                    if sonaba {
                        r.alternar_sonido();
                    }
                    if pausado {
                        r.alternar_pausa();
                    }
                    i.video = Some(r);
                    armar_temporizador_video(self.hwnd, ritmo_video_ms);
                }
                Err(e) => {
                    tracing::warn!(?e, "el video no pudo reabrirse en el dispositivo nuevo");
                    i.video_fallido = true;
                }
            }
        }
        pintar(i);
        Ok(())
    }

    /// Si el ultimo fotograma se pinto y presento sin fallo.
    pub fn pintado_bien(&self) -> bool {
        interno_de(self.hwnd).is_some_and(|i| i.pintado_bien.get())
    }

    pub fn poner_pizarra(&self, pizarra: Option<(u8, u8)>) {
        if let Some(i) = interno_de(self.hwnd) {
            i.pizarra = pizarra;
        }
    }

    /// Cambia la imagen de un pin de imagen por otra DEL MISMO USO (el fondo
    /// nuevo de una pizarra), sin tocar lo anotado encima ni su sitio.
    pub fn poner_imagen(&self, imagen: pixpin_codec::ImagenRgba) {
        if let Some(i) = interno_de(self.hwnd) {
            if let Contenido::Imagen(img) = &mut i.contenido {
                i.tapa_la_tarjeta = imagen.es_opaca();
                i.imagen_nativa = (imagen.ancho, imagen.alto);
                *img = imagen;
                rehacer_bitmap(i);
                pintar(i);
            }
        }
    }

    /// La imagen del pin se leyo **reducida** a la medida en que se ve
    /// (abrir una foto: `pixpin_codec::vista`); la entera mide `nativa` y
    /// esta en `fichero`. Desde aqui el «100 %», las anotaciones y el texto
    /// reconocido van en pixeles de la entera, y el pin la lee el solo, una
    /// vez, si se acerca mas alla de lo leido o se saca la lupa.
    pub fn poner_resolucion_completa(&self, fichero: std::path::PathBuf, nativa: (u32, u32)) {
        if let Some(i) = interno_de(self.hwnd)
            && let Contenido::Imagen(img) = &i.contenido
            && (img.ancho, img.alto) != nativa
        {
            i.imagen_nativa = nativa;
            i.completa = Some(fichero);
        }
    }

    /// Cambia la vista previa con que nacio el pin por la imagen buena, leida
    /// en otro hilo (abrir una foto grande: `pixpin_codec::vista`). No toca
    /// sitio, zoom ni anotaciones, y nunca empeora lo que hay: si el pin ya
    /// leyo mas (se acerco y leyo la entera), se queda con eso.
    pub fn poner_imagen_leida(&self, imagen: pixpin_codec::ImagenRgba) {
        let Some(i) = interno_de(self.hwnd) else {
            return;
        };
        let Contenido::Imagen(img) = &mut i.contenido else {
            return;
        };
        if imagen.ancho as u64 * imagen.alto as u64 <= img.ancho as u64 * img.alto as u64 {
            return;
        }
        let (nw, nh) = i.imagen_nativa;
        if (imagen.ancho, imagen.alto) == (nw, nh) || (imagen.ancho, imagen.alto) == (nh, nw) {
            // Ya es la entera: no queda nada que leer despues.
            i.completa = None;
        }
        i.tapa_la_tarjeta = imagen.es_opaca();
        *img = imagen;
        rehacer_bitmap(i);
        pintar(i);
    }

    /// Los pixeles que el pin tiene leidos de su imagen (ancho, alto).
    pub fn resolucion_leida(&self) -> Option<(u32, u32)> {
        interno_de(self.hwnd).and_then(|i| match &i.contenido {
            Contenido::Imagen(img) => Some((img.ancho, img.alto)),
            _ => None,
        })
    }

    pub fn poner_ruta(&self, ruta: Option<std::path::PathBuf>) {
        if let Some(i) = interno_de(self.hwnd) {
            i.ruta_origen = ruta;
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

    /// Si el pin ensena un video (aunque su reproductor haya fallado).
    pub fn es_video(&self) -> bool {
        matches!(contenido_es_video(self.hwnd), Some(true))
    }

    /// Da o quita el sonido a un pin de video. Un video abierto a proposito
    /// (doble clic en el Explorador) se oye: es lo que se pidio; los que
    /// vuelven al arrancar Windows siguen callados (D69).
    pub fn poner_sonido(&self, con_sonido: bool) {
        if let Some(v) = interno_de(self.hwnd).and_then(|i| i.video.as_ref()) {
            v.poner_silencio(!con_sonido);
        }
    }
}

impl Pin {
    /// Cuelga la zona de pantalla de un pin en vivo. Va aparte de `nuevo`
    /// porque la fuente necesita el HWND de esta ventana para despertarla.
    ///
    /// El pin se EXCLUYE de la captura: sin eso, ponerlo encima de su propia
    /// zona lo meteria dentro de si mismo, un pasillo de espejos que ademas
    /// tapa lo que se queria ver. Efecto secundario conocido: tampoco sale
    /// en las capturas de pantalla, de PixPin ni de otros programas.
    pub fn poner_fuente_viva(&self, fuente: Box<dyn FuenteViva>) {
        // SAFETY: afinidad de una ventana propia y viva. Si el sistema no la
        // admite (anterior a Windows 10 2004) falla sin mas y el pin sigue
        // funcionando, solo que se vera a si mismo si se le pone encima.
        unsafe {
            if let Err(e) = SetWindowDisplayAffinity(self.hwnd, WDA_EXCLUDEFROMCAPTURE) {
                tracing::warn!(?e, "el pin en vivo no se pudo excluir de la captura");
            }
        }
        if let Some(i) = interno_de(self.hwnd) {
            i.fuente_viva = Some(fuente);
            i.vivo_pausado = false;
        }
    }

    /// Cuantos pixeles de la ventana mide un pixel del contenido original,
    /// en horizontal y en vertical. Las anotaciones se guardan en pixeles
    /// del original y se pintan multiplicadas por esto; el raton llega en
    /// pixeles de la ventana y hay que DIVIDIRLO por lo mismo, o la tinta
    /// sale desplazada en cuanto el pin no esta al 100 %.
    pub fn escala_contenido(&self) -> (f32, f32) {
        let Some(i) = interno_de(self.hwnd) else {
            return (1.0, 1.0);
        };
        let (nw, nh) = i.imagen_nativa;
        let r = i.estado.rect();
        if nw == 0 || nh == 0 || r.ancho == 0 || r.alto == 0 {
            return (1.0, 1.0);
        }
        (r.ancho as f32 / nw as f32, r.alto as f32 / nh as f32)
    }

    /// Quita el pin de la pantalla o lo devuelve, sin cerrarlo. Es lo que
    /// usa Ctrl+2 con los pines en vivo, que no tienen entrada en el almacen
    /// y cerrados no se podrian traer de vuelta. Oculto, un pin en vivo no
    /// copia ni pinta fotogramas (ver `MSG_FOTOGRAMA_VIVO`).
    pub fn esconder(&self, esconder: bool) {
        // SAFETY: ventana propia y viva mientras viva `self`.
        unsafe {
            let _ = ShowWindow(
                self.hwnd,
                if esconder { SW_HIDE } else { SW_SHOWNOACTIVATE },
            );
        }
    }

    /// Si es un pin en vivo y esta en pausa.
    pub fn vivo_pausado(&self) -> bool {
        interno_de(self.hwnd).is_some_and(|i| i.vivo_pausado)
    }

    /// La opacidad del pin en por ciento (v2), al restaurarlo del almacen.
    pub fn poner_opacidad(&self, por_cien: u8) {
        if let Some(i) = interno_de(self.hwnd) {
            i.opacidad = opacidad_valida(por_cien as f32 / 100.0);
            pintar(i);
        }
    }

    /// La opacidad de ahora, en por ciento.
    pub fn opacidad(&self) -> u8 {
        interno_de(self.hwnd).map_or(100, |i| (i.opacidad * 100.0).round() as u8)
    }

    /// Lo trae delante y le da el foco, para encontrarlo (panel «Pines
    /// abiertos»): la sombra del enfocado es la mas marcada.
    pub fn resaltar(&self) {
        // SAFETY: ventana propia y viva; traerla delante desde el mismo
        // hilo que tiene la ventana activa (el panel) esta permitido.
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            let _ = SetForegroundWindow(self.hwnd);
            let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(self.hwnd));
        }
    }

    /// Si se ve en pantalla (no esta escondido).
    pub fn visible(&self) -> bool {
        // SAFETY: consulta pura sobre una ventana propia.
        unsafe { IsWindowVisible(self.hwnd) }.as_bool()
    }
}

/// La opacidad dentro de lo que se deja: ni invisible ni mas que opaco.
fn opacidad_valida(o: f32) -> f32 {
    if o.is_finite() {
        o.clamp(OPACIDAD_MINIMA, 1.0)
    } else {
        1.0
    }
}

/// La opacidad tras girar la rueda con Mayus: un 10 % por muesca.
fn opacidad_tras_rueda(o: f32, delta: i32) -> f32 {
    let paso = 0.1 * delta.signum() as f32;
    (opacidad_valida(o + paso) * 100.0).round() / 100.0
}

/// Si la ventana es un pin de video. `None` si ya no existe.
fn contenido_es_video(hwnd: HWND) -> Option<bool> {
    interno_de(hwnd).map(|i| matches!(i.contenido, Contenido::Video { .. }))
}

/// Pone en marcha el ritmo del video: con reproductor, el hilo que sigue el
/// refresco del monitor; sin el, un temporizador que solo lleva el fallo.
fn armar_temporizador_video(hwnd: HWND, ritmo_ms: u32) {
    if let Some(v) = interno_de(hwnd).and_then(|i| i.video.as_mut()) {
        // En `Completo` (16 ms) cada refresco; en `Ligero` (33 ms), uno de
        // cada dos a 60 Hz: 30 fps como mucho.
        let minimo = if ritmo_ms > 20 { ritmo_ms } else { 0 };
        v.marcar_ritmo(hwnd.0 as isize, MSG_TICK_VIDEO, minimo);
        return;
    }
    // SAFETY: temporizador de ventana propia; volver a armarlo con el mismo
    // id solo cambia el intervalo.
    unsafe {
        SetTimer(Some(hwnd), ID_TEMPORIZADOR_VIDEO, ritmo_ms.max(1), None);
    }
}

// ---------------------------------------------------------------------------
// La barra del pin (v2): `barra.rs` dice que lleva, `barra_flotante.rs` la
// ensena, y aqui se decide para cada pin y se atiende cada clic.
// ---------------------------------------------------------------------------

/// Que barra lleva este pin.
fn tipo_barra(i: &PinInterno) -> crate::barra::TipoBarra {
    use crate::barra::{Propio, TipoBarra};
    let propio = match &i.contenido {
        Contenido::Imagen(_) | Contenido::Nota { .. } => Propio::Zoom,
        Contenido::Video { .. } if i.video.is_some() => Propio::Video,
        Contenido::Video { .. } => Propio::Ninguno,
        Contenido::Vivo { .. } => Propio::Vivo,
        Contenido::Documento { .. } if es_pdf(i) => Propio::Paginas {
            con_paso: i.pdf_paginas.is_some_and(|c| c > 1),
        },
        Contenido::Documento { .. } | Contenido::Archivo { .. } => Propio::Abrir,
        Contenido::Herramienta { .. } => Propio::Ninguno,
    };
    TipoBarra {
        propio,
        anotable: crate::menu::anotable(&i.contenido),
        copiable: true,
    }
}

/// Lo que la barra ensena ahora de este pin.
fn datos_barra(i: &PinInterno) -> crate::barra_flotante::DatosBarra {
    let r = i.estado.rect();
    let zoom_por_cien = match &i.contenido {
        Contenido::Nota { .. } => (i.zoom_texto * 100.0).round() as u32,
        _ if i.imagen_nativa.0 > 0 => {
            (r.ancho as f32 * 100.0 / i.imagen_nativa.0 as f32).round() as u32
        }
        _ => 100,
    };
    crate::barra_flotante::DatosBarra {
        tipo: tipo_barra(i),
        zoom_por_cien,
        pagina: i.pdf_paginas.map(|c| (i.pdf_pagina, c)),
        video: i.video.as_ref().map(|v| {
            let (t, duracion) = v.posicion();
            crate::barra_flotante::DatosVideo {
                reproduciendo: v.reproduciendo(),
                // Al segundo: la barra solo se repinta cuando cambia algo
                // que se ve, y el tiempo se ve en segundos.
                t: t.floor(),
                duracion,
                volumen: v.volumen(),
                silenciado: v.silenciado(),
            }
        }),
        remoto: i.remoto,
        en_pausa: i.vivo_pausado,
    }
}

/// Si a este pin le toca barra ahora: con textos (sin ellos no hay ni
/// menu), sin anotar, sin dejar pasar el clic, sin un gesto a medias y a la
/// vista.
fn quiere_barra(i: &PinInterno) -> bool {
    i.textos.is_some()
        && !i.anotando
        && !i.pasante
        && !i.estado.en_gesto()
        // SAFETY: consulta pura sobre la ventana propia.
        && unsafe { IsWindowVisible(i.hwnd) }.as_bool()
}

/// Ensena la barra junto a este pin y arma la vigilancia de la salida.
fn ensenar_barra(i: &PinInterno) {
    let Some(t) = &i.textos else { return };
    // Las paginas de un PDF se preguntan la primera vez que hacen falta.
    if i.pdf_paginas.is_none() && es_pdf(i) {
        (i.al_cambiar)(CambioPin::PaginaPedida(0));
    }
    crate::barra_flotante::mostrar(
        &i.d3d,
        &i.motor,
        i.hwnd,
        i.estado.rect(),
        area_de_trabajo(i.hwnd),
        i.escala_por_cien,
        datos_barra(i),
        t,
        i.tema_claro,
    );
    // SAFETY: temporizador de la ventana propia; se mata al esconderla.
    unsafe {
        SetTimer(Some(i.hwnd), ID_TEMPORIZADOR_BARRA, RITMO_BARRA_MS, None);
    }
}

/// Repinta la barra si es de este pin (cambio el zoom, la pagina, el video).
fn actualizar_barra(i: &PinInterno) {
    if crate::barra_flotante::es_de(i.hwnd) {
        crate::barra_flotante::actualizar(i.hwnd, datos_barra(i));
    }
}

/// La vuelve a poner junto al pin, que se ha movido o cambiado de tamano.
fn recolocar_barra(i: &PinInterno) {
    if crate::barra_flotante::es_de(i.hwnd) {
        crate::barra_flotante::recolocar(i.hwnd, i.estado.rect(), area_de_trabajo(i.hwnd));
        crate::barra_flotante::actualizar(i.hwnd, datos_barra(i));
    }
}

fn esconder_barra(hwnd: HWND) {
    crate::barra_flotante::esconder(hwnd);
    // SAFETY: mata el temporizador propio (si lo habia).
    unsafe {
        let _ = KillTimer(Some(hwnd), ID_TEMPORIZADOR_BARRA);
    }
}

/// Lo que se pulso en la barra de este pin.
pub(crate) fn accion_de_barra(hwnd: HWND, evento: crate::barra_flotante::EventoBarra) {
    use crate::barra::AccionBarra as A;
    use crate::barra_flotante::EventoBarra;
    let Some(i) = interno_de(hwnd) else {
        return;
    };
    match evento {
        EventoBarra::Saltar {
            fraccion,
            aproximado,
        } => {
            if let Some(v) = &i.video
                && let (_, Some(d)) = v.posicion()
            {
                v.buscar(fraccion * d, aproximado);
            }
            pintar(i);
            actualizar_barra(i);
        }
        EventoBarra::Volumen(muescas) => {
            if let Some(v) = &i.video {
                v.poner_volumen(crate::mandos_video::volumen_tras_rueda(
                    v.volumen(),
                    muescas,
                ));
            }
            actualizar_barra(i);
        }
        EventoBarra::Pulsado { accion, bajo } => match accion {
            A::Alejar | A::Acercar => {
                // El mismo camino que la rueda, anclado en el centro del pin:
                // el gestor ya sabe animar el zoom y guardarlo.
                let r = i.estado.rect();
                (i.al_cambiar)(CambioPin::RuedaGirada {
                    delta: if accion == A::Acercar { 120 } else { -120 },
                    cursor: Punto {
                        x: r.x + r.ancho as i32 / 2,
                        y: r.y + r.alto as i32 / 2,
                    },
                });
            }
            A::PaginaAnterior => (i.al_cambiar)(CambioPin::PaginaPedida(-1)),
            A::PaginaSiguiente => (i.al_cambiar)(CambioPin::PaginaPedida(1)),
            A::PinearPagina => (i.al_cambiar)(CambioPin::ExtraerPaginaPedida),
            A::Reproducir => {
                if matches!(i.contenido, Contenido::Vivo { .. }) {
                    alternar_vivo(i);
                } else {
                    alternar_video(hwnd, i);
                }
                actualizar_barra(i);
            }
            A::Saltar => {}
            A::Sonido => {
                if let Some(v) = &i.video {
                    v.alternar_sonido();
                    // Quitar el silencio con el volumen a cero no se oiria:
                    // se sube a la mitad.
                    if !v.silenciado() && v.volumen() <= 0.0 {
                        v.poner_volumen(0.5);
                    }
                }
                actualizar_barra(i);
            }
            A::Manejar => alternar_remoto(hwnd, i),
            A::Congelar => (i.al_cambiar)(CambioPin::CongelarPedido),
            A::Abrir => (i.al_cambiar)(CambioPin::AbrirPedido),
            A::Copiar => {
                // Con texto marcado, copia ESO, como Ctrl+C.
                if !copiar_seleccion(i) {
                    (i.al_cambiar)(CambioPin::CopiarPedido);
                }
            }
            A::Anotar => {
                esconder_barra(hwnd);
                (i.al_cambiar)(CambioPin::AnotarPedido);
            }
            A::Mas => abrir_menu(hwnd, Some(bajo)),
            A::Cerrar => {
                esconder_barra(hwnd);
                aplicar(hwnd, EfectoPin::Cerrar);
            }
        },
    }
}

/// Las guias frente a los demas pines visibles, para este pin en `rect`.
fn guias_para(hwnd: HWND, i: &PinInterno, rect: Rect) -> crate::guias::Ajuste {
    let mut otros: Vec<Rect> = Vec::new();
    // Los pines viven todos en este hilo: se recorren sus ventanas y se lee
    // el rect de cada una de su propio estado (la ventana puede estar
    // recortada al escritorio; el contenido no).
    unsafe extern "system" fn cada(h: HWND, lp: LPARAM) -> windows::core::BOOL {
        // SAFETY: `lp` es el puntero a la pareja de abajo, viva durante toda
        // la enumeracion, que es sincrona.
        let (yo, otros) = unsafe { &mut *(lp.0 as *mut (HWND, &mut Vec<Rect>)) };
        let mut clase = [0u16; 16];
        // SAFETY: lectura del nombre de clase a un bufer propio.
        let n = unsafe { GetClassNameW(h, &mut clase) };
        let es_pin = String::from_utf16_lossy(&clase[..n.max(0) as usize]) == "PixPinPin";
        // Solo los del MISMO hilo: su USERDATA es un `PinInterno` de verdad.
        // SAFETY: consultas puras sobre ventanas que existen ahora.
        let mismo_hilo =
            unsafe { GetWindowThreadProcessId(h, None) == GetWindowThreadProcessId(*yo, None) };
        if es_pin
            && mismo_hilo
            && h != *yo
            // SAFETY: consulta pura.
            && unsafe { IsWindowVisible(h) }.as_bool()
            && let Some(o) = interno_de(h)
        {
            otros.push(o.estado.rect());
        }
        true.into()
    }
    let mut pareja: (HWND, &mut Vec<Rect>) = (hwnd, &mut otros);
    // SAFETY: enumeracion sincrona; el puntero apunta a `pareja`, que vive
    // hasta despues de la llamada.
    unsafe {
        let _ = EnumWindows(Some(cada), LPARAM(&mut pareja as *mut _ as isize));
    }
    let e = i.escala_por_cien as i32;
    crate::guias::alinear(
        rect,
        &otros,
        crate::guias::UMBRAL_LOGICO * e / 100,
        crate::guias::SEPARACION_LOGICA * e / 100,
    )
}

/// El tick del video: si hay fotograma nuevo, se pinta. Lo llaman el ritmo
/// del monitor y, sin reproductor, el temporizador del fallo.
fn tick_video(hwnd: HWND) {
    let Some(i) = interno_de(hwnd) else {
        return;
    };
    if let Some(v) = &i.video {
        v.atendido();
    }
    let fallo = i.video_fallido || i.video.as_ref().is_some_and(|v| v.fallo());
    if fallo {
        // SAFETY: mata el temporizador propio (si lo habia).
        unsafe {
            let _ = KillTimer(Some(hwnd), ID_TEMPORIZADOR_VIDEO);
        }
        if let Some(v) = &i.video {
            tracing::warn!(motivo = ?v.motivo_fallo(), "Media Foundation no pudo con el video");
        }
        // El hilo del ritmo se para con el reproductor.
        i.video = None;
        i.video_fallido = true;
        (i.al_cambiar)(CambioPin::VideoFallido);
        return;
    }
    // Oculto (Ctrl+2) nadie lo ve: ni copiar ni pintar.
    // SAFETY: consulta pura sobre la ventana propia.
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return;
    }
    // Los metadatos dan el tamano nativo: es el 100 % del menu. La primera
    // vez, si el pin nacio con otra proporcion (sin cabecera legible ni
    // miniatura), se corrige: un video estirado parece roto.
    if let Some(dim) = i.video.as_mut().and_then(|v| v.dimensiones()) {
        i.imagen_nativa = dim;
        if !i.proporcion_video_hecha {
            i.proporcion_video_hecha = true;
            if !i.estado.es_fijo()
                && let Some(r) = crate::mandos_video::rect_con_proporcion(i.estado.rect(), dim)
            {
                i.estado.poner_rect(r);
                aplicar(hwnd, EfectoPin::Redimensionar(r));
                let Some(i) = interno_de(hwnd) else {
                    return;
                };
                (i.al_cambiar)(CambioPin::Movido(colocacion_de(i, r)));
            }
        }
    }
    let Some(i) = interno_de(hwnd) else {
        return;
    };
    // Clonar la interfaz de la textura (una cuenta de referencia) libera el
    // prestamo del reproductor antes de tocar el bitmap.
    let textura = i.video.as_mut().and_then(|v| v.tick().cloned());
    if let Some(t) = textura {
        if i.bitmap.is_none() {
            i.bitmap = i.motor.bitmap_desde_textura(&t).ok();
        }
        pintar(i);
        // El tiempo de la barra; `actualizar` no repinta si el segundo que
        // se ve no ha cambiado.
        actualizar_barra(i);
    }
    if let Some(v) = &i.video {
        v.reposar_si_parado();
    }
}

/// Pausar o reanudar un pin en vivo. En pausa la captura sigue abierta
/// —reabrirla tarda y parpadea el aviso de grabacion del sistema—, pero el
/// pin deja de copiar: se queda el ultimo fotograma, que es lo que se
/// quiere al pausar para leer algo que se mueve.
fn alternar_vivo(i: &mut PinInterno) {
    if i.fuente_viva.is_some() {
        i.vivo_pausado = !i.vivo_pausado;
        tracing::info!(pausado = i.vivo_pausado, "pin en vivo alternado");
        // Al reanudar, ponerse al dia YA: con la zona quieta la captura no
        // manda ningun aviso, y el pin seguia con la foto de cuando se
        // pauso, como si no hubiera vuelto a en vivo (el usuario: «que se
        // pueda volver de nuevo en vivo»). Lo capturado en la pausa ya esta
        // en la fuente; el aviso solo hace que se copie y se pinte.
        if !i.vivo_pausado {
            // SAFETY: mensaje propio a la ventana propia.
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                    Some(i.hwnd),
                    MSG_FOTOGRAMA_VIVO,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
        actualizar_barra(i);
    }
}

/// El pixel de la zona que se ve bajo un punto de PANTALLA, si este pin esta
/// manejando su zona a distancia. `None` si no lo esta, o si el punto cae en
/// la sombra: entonces el clic es del pin, como siempre.
fn punto_remoto(i: &PinInterno, pantalla: (i32, i32)) -> Option<(i32, i32)> {
    if !i.remoto || !matches!(i.contenido, Contenido::Vivo { .. }) {
        return None;
    }
    // `estado.rect()` es el contenido en coordenadas de pantalla, que es lo
    // mismo que usa el zoom anclado: no depende de como este recortada la
    // ventana contra el borde del escritorio.
    let r = i.estado.rect();
    crate::remoto::punto_en_la_zona(
        ((pantalla.0 - r.x) as f32, (pantalla.1 - r.y) as f32),
        (r.ancho, r.alto),
        crate::remoto::Vista {
            escala: i.vista_escala,
            dx: i.vista_dx,
            dy: i.vista_dy,
        },
        i.imagen_nativa,
    )
}

/// Reproducir o pausar (D68): el temporizador va con el estado, asi que un
/// video en pausa no cuesta un solo tick (D67).
fn alternar_video(_hwnd: HWND, i: &mut PinInterno) {
    let Some(v) = &i.video else {
        return;
    };
    // El reproductor despierta o duerme su propio ritmo.
    v.alternar_pausa();
    tracing::debug!(reproduciendo = v.reproduciendo(), "video alternado");
    // Se repinta ya: el boton y el simbolo de pausa cambian aunque no
    // llegue fotograma nuevo.
    pintar(i);
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
    ///
    /// `tapadas` son las cajas de los mosaicos, en pixeles de la imagen
    /// original, y van **aparte de las ordenes a proposito**: una `Orden` ya
    /// no dice de que figura salio, y la lupa necesita saber exactamente que
    /// trozos no puede ensenar. Ver `pintar`.
    pub fn poner_anotaciones(
        &self,
        ordenes: Vec<pixpin_motor2d::Orden>,
        tapadas: Vec<(f32, f32, f32, f32)>,
    ) {
        self.poner_anotaciones_con_grafito(ordenes, Vec::new(), tapadas);
    }

    /// Como [`Self::poner_anotaciones`], con el grafito aparte: cada mapa se
    /// pinta en su sitio entre las ordenes (`GrafitoSuelto::antes_de`). Las
    /// saca `tinta::grafito::ordenes_con_grafito`.
    pub fn poner_anotaciones_con_grafito(
        &self,
        ordenes: Vec<pixpin_motor2d::Orden>,
        grafitos: Vec<pixpin_motor2d::tinta::grafito::GrafitoSuelto>,
        tapadas: Vec<(f32, f32, f32, f32)>,
    ) {
        if let Some(i) = interno_de(self.hwnd) {
            i.anotaciones = ordenes;
            // Los bitmaps de mapas que ya no estan, fuera: cada mapa nuevo
            // trae un id nuevo y la cache no los reconoceria nunca.
            let siguen = |id: u64| grafitos.iter().any(|g| g.mapa.id == id);
            if !i.grafitos.iter().all(|g| siguen(g.mapa.id)) {
                i.cache_grafito.borrow_mut().vaciar();
            }
            i.grafitos = grafitos;
            i.tapadas = tapadas;
            pintar(i);
        }
    }

    /// Pone o quita la lupa (D52). Solo repinta si algo cambio: la lupa se
    /// actualiza con cada movimiento del raton y repintar en balde cuesta.
    pub fn poner_lupa(&self, lupa: Option<LupaPin>) {
        if let Some(i) = interno_de(self.hwnd) {
            if i.lupa != lupa {
                // La lupa ensena pixeles reales: con la foto leida reducida
                // ampliaria pixeles ya estirados (y en otra escala).
                if lupa.is_some() {
                    asegurar_resolucion(i, true);
                }
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
    /// Dice si el equipo sabe reconocer texto, para ofrecerlo o no en el
    /// menu.
    ///
    /// Lo decide la aplicacion y no este crate: el motor de reconocimiento
    /// vive en `pixpin-ocr`, que esta en la MISMA capa que este, y las
    /// capas no se llaman de lado. Ademas se pregunta una vez y no cada
    /// vez que se abre el menu, porque montar el motor se nota.
    /// Guarda el texto reconocido de la imagen (P4.4).
    ///
    /// Las cajas vienen en pixeles de la imagen NATIVA, no de lo que se ve:
    /// asi el pin puede estar a cualquier tamano y la seleccion sigue
    /// cuadrando sin volver a reconocer nada.
    ///
    /// Una lista vacia es una respuesta valida y se guarda igual: significa
    /// «se miro y no hay texto», que no es lo mismo que «no se ha mirado».
    /// Sin distinguirlo, una imagen sin letras se reconoceria una y otra
    /// vez en cada pasada del raton.
    pub fn poner_texto_reconocido(&self, renglones: Vec<pixpin_geom::seleccion_texto::Renglon>) {
        if let Some(i) = interno_de(self.hwnd) {
            i.texto_ocr = Some(renglones);
        }
    }

    /// Pone los filtros al restaurar del almacen y rehace la imagen.
    pub fn poner_filtros(&self, gris: bool, invertido: bool, brillo: i32) {
        if let Some(i) = interno_de(self.hwnd) {
            i.filtros = pixpin_codec::filtros::Filtros {
                gris,
                invertido,
                brillo,
            };
            if !i.filtros.son_neutros() {
                rehacer_bitmap(i);
                pintar(i);
            }
        }
    }

    pub fn poner_ocr(&self, hay: bool) {
        if let Some(i) = interno_de(self.hwnd) {
            i.con_ocr = hay;
        }
    }

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
    asegurar_resolucion(i, false);
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

/// Si un pin que ensena `leida` pixeles necesita mas para verse nitido a
/// `vista` (lo que ocupa en pantalla, ya contando el giro y el zoom de
/// dentro). Un 5 % de holgura: el escalado de la GPU no se nota tan cerca, y
/// leer la entera cuesta decenas de milisegundos.
fn necesita_mas_resolucion(leida: (u32, u32), vista: (f32, f32)) -> bool {
    vista.0 > leida.0 as f32 * 1.05 || vista.1 > leida.1 as f32 * 1.05
}

/// Si la imagen del pin se leyo reducida y ahora se ve mas grande que eso
/// (o `siempre`: la lupa ensena pixeles reales), lee la ENTERA de su fichero
/// y rehace el bitmap. Una sola vez: despues ya no hay fichero pendiente.
fn asegurar_resolucion(i: &mut PinInterno, siempre: bool) {
    let Some(fichero) = i.completa.clone() else {
        return;
    };
    let Contenido::Imagen(img) = &i.contenido else {
        i.completa = None;
        return;
    };
    let r = i.estado.rect();
    let zoom = i.vista_escala.max(1.0);
    // Girado un cuarto, el ancho de la imagen ocupa el alto del pin.
    let (vw, vh) = if i.giro % 2 == 1 {
        (r.alto as f32, r.ancho as f32)
    } else {
        (r.ancho as f32, r.alto as f32)
    };
    if !siempre && !necesita_mas_resolucion((img.ancho, img.alto), (vw * zoom, vh * zoom)) {
        return;
    }
    let t0 = std::time::Instant::now();
    i.completa = None;
    match pixpin_codec::cargar(&fichero) {
        Ok(entera) => {
            let (nw, nh) = i.imagen_nativa;
            let cuadra =
                (entera.ancho, entera.alto) == (nw, nh) || (entera.ancho, entera.alto) == (nh, nw);
            if !cuadra {
                tracing::warn!(
                    leida = ?(entera.ancho, entera.alto),
                    nativa = ?i.imagen_nativa,
                    "la imagen entera no mide lo esperado; el pin sigue con la reducida"
                );
                return;
            }
            i.tapa_la_tarjeta = entera.es_opaca();
            if let Contenido::Imagen(img) = &mut i.contenido {
                *img = entera;
            }
            rehacer_bitmap(i);
            tracing::info!(
                ms = t0.elapsed().as_millis() as u64,
                ancho = nw,
                alto = nh,
                "el pin leyo su imagen entera"
            );
        }
        Err(e) => tracing::warn!(?e, "no se pudo leer la imagen entera del pin"),
    }
}

/// El zoom del texto en por ciento, para persistirlo (100 fuera de las
/// notas: los demas contenidos no tienen zoom de texto).
/// Lo que se guarda de un pin en un momento dado, para un rect concreto.
/// Rehace el bitmap del pin desde la imagen ORIGINAL con los filtros
/// puestos.
///
/// Siempre desde el original y nunca encima de lo ya filtrado: cada pasada
/// redondea, y encadenandolas la imagen se degradaria hasta quedar
/// irreconocible despues de unos cuantos toques al brillo.
fn rehacer_bitmap(i: &mut PinInterno) {
    let fuente = match &i.contenido {
        Contenido::Imagen(img) => Some(img),
        Contenido::Archivo { icono, .. } => icono.as_ref(),
        Contenido::Documento { vista, .. } => Some(vista),
        Contenido::Nota { .. }
        | Contenido::Video { .. }
        | Contenido::Vivo { .. }
        | Contenido::Herramienta { .. } => None,
    };
    let Some(original) = fuente else { return };
    let filtrada = pixpin_codec::filtros::aplicar(original, i.filtros);
    match i
        .motor
        .bitmap_desde_pixeles(filtrada.ancho, filtrada.alto, &filtrada.pixeles)
    {
        Ok(b) => i.bitmap = Some(b),
        // Si falla se deja el bitmap de antes: peor es quedarse sin imagen
        // que ensenar por no haber podido aplicar un filtro.
        Err(e) => tracing::warn!(?e, "no se pudo aplicar el filtro al pin"),
    }
}

/// Si el pin viene de un fichero PDF.
///
/// Por la extension y no por el contenido: mirar dentro obligaria a abrir
/// el fichero cada vez que se abre el menu, y la extension es lo que ya
/// uso el gestor para decidir como ensenarlo.
fn es_pdf(i: &PinInterno) -> bool {
    i.ruta_origen
        .as_ref()
        .and_then(|r| r.extension())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Copia el texto marcado, si lo hay. Devuelve si copio algo.
///
/// Lo hace el pin y no el gestor porque el texto ya esta aqui: mandarlo a
/// dar la vuelta por el gestor solo para llamar al portapapeles seria un
/// rodeo, y `pixpin-codec` esta por debajo de este crate.
fn copiar_seleccion(i: &PinInterno) -> bool {
    let Some(renglones) = &i.texto_ocr else {
        return false;
    };
    if i.seleccion_texto.is_empty() {
        return false;
    }
    let texto = pixpin_geom::seleccion_texto::texto_de(renglones, &i.seleccion_texto);
    if texto.trim().is_empty() {
        return false;
    }
    match pixpin_codec::copiar_texto(&texto) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(?e, "no se pudo copiar el texto seleccionado");
            false
        }
    }
}

/// Un punto del contenido llevado a pixeles de la imagen NATIVA.
///
/// Las cajas del texto reconocido estan en pixeles de la imagen original,
/// y el pin casi nunca se ve a ese tamano. Sin esta conversion, la
/// seleccion acertaria solo con el pin al 100 % exacto.
fn a_pixeles_nativos(i: &PinInterno, p: Punto) -> Punto {
    let (nw, nh) = i.imagen_nativa;
    let r = i.estado.rect();
    if r.ancho == 0 || r.alto == 0 || nw == 0 || nh == 0 {
        return p;
    }
    Punto {
        x: (p.x as f32 * nw as f32 / r.ancho as f32).round() as i32,
        y: (p.y as f32 * nh as f32 / r.alto as f32).round() as i32,
    }
}

/// Holgura vertical al buscar texto bajo el cursor, en pixeles nativos.
///
/// Acertar el recuadro exacto de una palabra obliga a afinar el raton, y
/// para «hay texto aqui» basta con estar a su altura.
const HOLGURA_TEXTO: i32 = 3;

/// Donde esta el cursor, en coordenadas del escritorio.
///
/// Los mensajes de raton traen el punto relativo al cliente, pero el zoom
/// anclado lo necesita en pantalla â es lo mismo que ya hace WM_MOUSEWHEEL,
/// que por su cuenta viene ya en pantalla.
fn cursor_de_pantalla() -> Punto {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT::default();
    // SAFETY: GetCursorPos escribe en una estructura local; si falla, se
    // queda en (0,0), que solo significa un zoom anclado en la esquina.
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    Punto { x: p.x, y: p.y }
}

fn y_de_pantalla() -> i32 {
    cursor_de_pantalla().y
}

/// Hace que la ventana deje pasar los clics, o que vuelva a recogerlos.
///
/// Es una funcion libre y no un metodo de `Pin` porque el WndProc tiene
/// que poder llamarla: `Pin` POSEE la ventana y la destruye al soltarlo,
/// asi que fabricar uno ahi dentro mataria el pin en cuanto acabara la
/// linea.
/// Enciende o apaga el modo clic de un pin en vivo: desde el menu o desde su
/// boton de la barra (el usuario lo queria a mano, arriba).
fn alternar_remoto(hwnd: HWND, i: &mut PinInterno) {
    i.remoto = !i.remoto;
    i.pulsado_remoto = None;
    // Encenderlo REANUDA el pin. Al usuario le paso a la primera: un doble
    // clic de antes lo habia dejado en pausa, y manejando a distancia veia
    // una foto fija mientras sus clics si actuaban en la ventana. Un mando a
    // distancia sobre una imagen parada es manejar a ciegas.
    if i.remoto && i.vivo_pausado {
        i.vivo_pausado = false;
        // Y un aviso de fotograma ya: con la pantalla quieta la captura no
        // manda ninguno, y el pin se quedaria con la foto de cuando se pauso.
        // SAFETY: mensaje propio a la ventana propia.
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(hwnd),
                MSG_FOTOGRAMA_VIVO,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
    tracing::info!(remoto = i.remoto, "manejo a distancia alternado");
    pintar(i);
    actualizar_barra(i);
}

fn poner_pasante_en(hwnd: HWND, pasante: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    };
    // `WS_EX_TRANSPARENT` solo deja pasar el raton en una ventana
    // `WS_EX_LAYERED`, asi que se ponen y se quitan juntos.
    let bits = WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0;
    // SAFETY: leer y escribir el estilo extendido de una ventana propia y
    // viva. Se conserva el resto de bits.
    unsafe {
        let actual = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let nuevo = if pasante {
            actual | bits
        } else {
            actual & !bits
        };
        if nuevo != actual {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, nuevo as isize);
        }
    }
    if let Some(i) = interno_de(hwnd) {
        i.pasante = pasante;
    }
    // Un pin que deja pasar el clic no se toca: tampoco su barra.
    if pasante {
        esconder_barra(hwnd);
    }
}

fn colocacion_de(i: &PinInterno, rect: Rect) -> Colocacion {
    Colocacion {
        rect,
        zoom_por_cien: zoom_por_cien_de(i),
        giro: i.giro,
        volteo_h: i.volteo_h,
        volteo_v: i.volteo_v,
        pasante: i.pasante,
        gris: i.filtros.gris,
        invertido: i.filtros.invertido,
        brillo: i.filtros.brillo,
        opacidad: (i.opacidad * 100.0).round() as u8,
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
    i.pintado_bien.set(false);
    let destino = match i.superficie.empezar(&i.motor) {
        Ok(d) => d,
        Err(e) => {
            // Con el dispositivo perdido fallaria cada fotograma: el aviso
            // ya salio (`pixpin_render::perdida`) y el gestor lo rehace; una
            // linea por fotograma solo llenaria el registro.
            if e.es_perdida() {
                tracing::debug!(?e, "el pin no pudo empezar a pintar: dispositivo perdido");
            } else {
                tracing::warn!(?e, "el pin no pudo empezar a pintar");
            }
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

    let con_capa = i.opacidad < 0.999;
    let dibujado = i.motor.dibujar(&destino, |p| {
        p.limpiar_transparente();
        // La opacidad (v2): todo el pin, sombra incluida, en una capa
        // translucida. Opaco no se paga la capa.
        if con_capa {
            use windows::Win32::Graphics::Direct2D::{
                Common::D2D_RECT_F, D2D1_LAYER_OPTIONS1_NONE, D2D1_LAYER_PARAMETERS1,
            };
            let parametros = D2D1_LAYER_PARAMETERS1 {
                contentBounds: D2D_RECT_F {
                    left: -1.0e7,
                    top: -1.0e7,
                    right: 1.0e7,
                    bottom: 1.0e7,
                },
                opacity: i.opacidad,
                layerOptions: D2D1_LAYER_OPTIONS1_NONE,
                ..Default::default()
            };
            // SAFETY: Push emparejado con el Pop del final del fotograma.
            unsafe { i.motor.contexto().PushLayer(&parametros, None) };
        }
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
            Contenido::Imagen(_) | Contenido::Video { .. } | Contenido::Vivo { .. } => {
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
                // Recortado a la tarjeta. Una nota tiene un alto maximo, y
                // el texto que no cabe se seguia pintando por debajo: salia
                // por fuera del pin y quedaba flotando sobre el escritorio.
                // Se ve con cualquier nota larga del movil, que son de
                // miles de caracteres. Lo reporto el usuario.
                //
                // Recortar y no encoger la letra: encogerla dejaria una
                // nota de cuatro mil caracteres ilegible. Cortada se lee lo
                // que hay, y la tarjeta se estira por la esquina para ver
                // el resto.
                p.con_recorte(
                    RectF {
                        x: m,
                        y: m,
                        ancho: w,
                        alto: h,
                    },
                    |p| {
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
                    },
                );
            }

            // La herramienta la pinta el gestor, recortada a la tarjeta: una
            // lista larga no puede asomar por fuera del pin.
            Contenido::Herramienta { .. } => {
                if let Some(f) = &i.pintor_interior {
                    p.con_recorte(caja, |p| f(p, caja));
                }
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

        // Manejando a distancia, un marco de acento: con el modo encendido
        // un clic sobre el pin ACTUA en otra parte de la pantalla, y eso
        // tiene que verse antes de pulsar, no despues.
        if i.remoto {
            p.trazar(caja, 2.0 * escala, Color::ACENTO);
        }

        // Los mandos del video, encima de la imagen y solo con el raton
        // encima: el resto del tiempo el pin es solo el video.
        if let Some(v) = &i.video {
            pintar_mandos_video(p, i, v, caja, escala);
        }

        // El texto marcado, entre la imagen y las anotaciones: es una
        // seleccion sobre la imagen, asi que va encima de ella, pero lo
        // que el usuario ha dibujado manda sobre todo.
        if let Some(renglones) = &i.texto_ocr {
            if !i.seleccion_texto.is_empty() {
                let (nw, nh) = i.imagen_nativa;
                let r = i.estado.rect();
                if nw > 0 && nh > 0 {
                    let fx = r.ancho as f32 / nw as f32;
                    let fy = r.alto as f32 / nh as f32;
                    for caja in
                        pixpin_geom::seleccion_texto::recuadros_de(renglones, &i.seleccion_texto)
                    {
                        p.rellenar_redondeado(
                            pixpin_render::RectF {
                                x: m + caja.x as f32 * fx,
                                y: m + caja.y as f32 * fy,
                                ancho: caja.ancho as f32 * fx,
                                alto: caja.alto as f32 * fy,
                            },
                            2.0,
                            // Azul translucido, como cualquier seleccion de
                            // texto: tiene que dejar leer lo que hay debajo.
                            pixpin_render::Color {
                                r: 0.16,
                                g: 0.51,
                                b: 0.96,
                                a: 0.38,
                            },
                        );
                    }
                }
            }
        }

        // Las anotaciones van ENCIMA de todo lo demas: son una capa, y el
        // contenido original nunca se toca (D48). Se dibujan en coordenadas
        // del contenido, asi que hay que sumarles el margen de la sombra.
        // D147: lo dibujado en el lienzo fuera de la imagen se guarda, pero
        // el pin ensena solo su contenido: sin recorte se colaba por el
        // margen de la sombra.
        p.con_recorte(caja, |p| pintar_anotaciones(p, i, m));

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
            // **Y se vuelve a tapar lo tapado.** `b` es el bitmap ORIGINAL
            // del pin: las anotaciones se pintan encima de el, no dentro, asi
            // que el cristal ensenaba el dato que el usuario habia cubierto
            // con un mosaico. Bastaba pulsar la lupa y pasar el cursor.
            //
            // `i.tapadas` va en pixeles de la imagen original, igual que
            // `fuente` despues de la conversion de arriba.
            for (x0, y0, x1, y1) in pixpin_motor2d::mosaico::zonas_en_la_lupa(
                &i.tapadas,
                (fuente.x, fuente.y, fuente.ancho, fuente.alto),
                (destino.x, destino.y, destino.ancho, destino.alto),
            ) {
                p.rellenar(
                    RectF {
                        x: x0,
                        y: y0,
                        ancho: x1 - x0,
                        alto: y1 - y0,
                    },
                    Color {
                        r: pixpin_motor2d::mosaico::TAPA_MACIZA.r,
                        g: pixpin_motor2d::mosaico::TAPA_MACIZA.g,
                        b: pixpin_motor2d::mosaico::TAPA_MACIZA.b,
                        a: 1.0,
                    },
                );
            }
            p.trazar(destino, 2.0 * escala, Color::ACENTO);
        }
        if con_capa {
            // SAFETY: cierra el PushLayer de arriba, dentro del fotograma.
            unsafe { i.motor.contexto().PopLayer() };
        }
    });
    let presentado = i.superficie.presentar();
    i.pintado_bien.set(dibujado.is_ok() && presentado.is_ok());
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

/// En pausa, un simbolo de reproducir en el centro del video para que se
/// vea que esta parado. Los mandos (reproducir, tiempo, linea, altavoz) van
/// en la barra del pin, fuera de la imagen (v2): ya no tapan el video.
fn pintar_mandos_video(
    p: &pixpin_render::Pintor,
    i: &PinInterno,
    v: &Reproductor,
    caja: RectF,
    escala: f32,
) {
    use pixpin_render::icono::material::PLAY_ARROW;
    if v.reproduciendo() || i.bitmap.is_none() {
        return;
    }
    let velo = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.55,
    };
    let radio = (caja.ancho.min(caja.alto) * 0.12).clamp(16.0 * escala, 36.0 * escala);
    let centro = (caja.x + caja.ancho / 2.0, caja.y + caja.alto / 2.0);
    p.circulo(centro, radio, velo);
    let lado = radio * 1.2;
    p.icono(
        &PLAY_ARROW,
        RectF {
            x: centro.0 - lado / 2.0,
            y: centro.1 - lado / 2.0,
            ancho: lado,
            alto: lado,
        },
        Color::BLANCO,
    );
}

/// Pinta las ordenes de dibujo del motor 2D sobre el contenido del pin.
///
/// El motor produce geometria y el pintor la pinta: es la separacion que
/// mantiene al motor puro y probable sin GPU.
fn pintar_anotaciones(p: &pixpin_render::Pintor, i: &PinInterno, margen: f32) {
    use pixpin_motor2d::Orden;

    // El origen del documento de anotacion es la esquina del CONTENIDO, no
    // la de la ventana: asi las anotaciones acompanan al pin al moverlo sin
    // recalcular ni un punto.
    //
    // Y se ESCALAN con el pin. Las coordenadas del dibujo estan en pixeles
    // de la imagen original, que casi nunca es el tamano al que se ve.
    // Sin escalar, agrandar el pin dejaba el dibujo con las medidas de
    // antes: se iba descolocando hacia un lado y acababa fuera de la
    // ventana. Lo reporto el usuario abriendo un proyecto del movil.
    let (nw, nh) = i.imagen_nativa;
    let r = i.estado.rect();
    let (fx, fy) = if nw > 0 && nh > 0 && r.ancho > 0 && r.alto > 0 {
        (r.ancho as f32 / nw as f32, r.alto as f32 / nh as f32)
    } else {
        // Sin tamano nativo —una nota, un video— no hay nada que escalar.
        (1.0, 1.0)
    };
    let mover = |q: &pixpin_motor2d::Punto2| (q.x * fx + margen, q.y * fy + margen);
    let color = |c: pixpin_motor2d::ColorRgba| Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    };

    // El grafito, cada mapa justo antes de la orden que le toca: lo que se
    // dibujo despues va encima, como en el editor.
    let mut grafitos = i.grafitos.iter().peekable();
    let mut pintar_grafitos_hasta = |k: usize| {
        while let Some(g) = grafitos.next_if(|g| g.antes_de <= k) {
            pintar_grafito(p, &mut i.cache_grafito.borrow_mut(), g, (fx, fy), margen);
        }
    };
    for (k, orden) in i.anotaciones.iter().enumerate() {
        pintar_grafitos_hasta(k);
        match orden {
            Orden::Poligono { puntos, color: c } | Orden::Relleno { puntos, color: c } => {
                let v: Vec<(f32, f32)> = puntos.iter().map(mover).collect();
                p.poligono(&v, color(*c));
            }
            Orden::Tinta { contorno, color: c } => {
                let v: Vec<(f32, f32)> = contorno.iter().map(mover).collect();
                p.tinta(&v, color(*c));
            }
            Orden::Polilinea {
                puntos,
                color: c,
                grosor,
                ..
            } => {
                let v: Vec<(f32, f32)> = puntos.iter().map(mover).collect();
                // El grosor escala como los puntos, y por la media de las
                // dos escalas: una linea no tiene ancho horizontal y
                // vertical por separado. Sin esto, un trazo grueso en un
                // pin reducido se veria igual de gordo y taparia el dibujo.
                p.polilinea(&v, *grosor * (fx + fy) / 2.0, color(*c));
            }
            // Con su letra, como en el lienzo (`dibujo::pintar::letra_de`):
            // familia, cara e interlineado de Excalidraw. Un texto suelto no
            // se parte, tampoco encogido.
            Orden::Texto {
                texto,
                x,
                y,
                tam,
                familia,
                color: c,
                ancho_max,
                negrita,
                cursiva,
            } => p.texto_con_letra(
                texto,
                x * fx + margen,
                y * fy + margen,
                *tam * (fx + fy) / 2.0,
                if *ancho_max >= pixpin_motor2d::texto::SIN_PARTIR {
                    *ancho_max
                } else {
                    *ancho_max * fx
                },
                &pixpin_render::letras::Letra {
                    familia,
                    negrita: *negrita,
                    cursiva: *cursiva,
                    interlineado: pixpin_motor2d::texto::interlineado_de(familia),
                },
                color(*c),
            ),
            // El numero de una cota: girado con su raya y con halo, como en
            // el lienzo (`dibujo::pintar`).
            Orden::Rotulo {
                texto,
                x,
                y,
                tam,
                familia,
                color: c,
                halo,
                grosor_halo,
                centro,
                angulo,
            } => {
                let k = (fx + fy) / 2.0;
                p.girado(
                    (centro.x * fx + margen, centro.y * fy + margen),
                    *angulo,
                    |p| {
                        p.texto_con_halo(
                            texto,
                            x * fx + margen,
                            y * fy + margen,
                            *tam * k,
                            &pixpin_render::letras::Letra {
                                familia,
                                negrita: false,
                                cursiva: false,
                                interlineado: pixpin_motor2d::texto::interlineado_de(familia),
                            },
                            color(*c),
                            color(*halo),
                            *grosor_halo * k,
                        )
                    },
                );
            }
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
    // Lo que va despues de la ultima orden.
    pintar_grafitos_hasta(usize::MAX);
}

/// **Un mapa de grafito en el pin**, escalado con el como todo lo anotado.
///
/// La caja y el centro de giro se llevan con la misma escala y el mismo
/// margen que los puntos (`mover` de [`pintar_anotaciones`]); el aumento que
/// decide si las casillas se ven de canto vivo es el del pin.
fn pintar_grafito(
    p: &pixpin_render::Pintor,
    cache: &mut pixpin_render::CacheGrafito,
    g: &pixpin_motor2d::tinta::grafito::GrafitoSuelto,
    (fx, fy): (f32, f32),
    margen: f32,
) {
    let m = &g.mapa;
    let (x, y, w, h) = m.caja();
    let mapa = pixpin_render::MapaGrafito {
        rgba: &m.rgba,
        ancho: m.ancho,
        alto: m.alto,
        caja: (x * fx + margen, y * fy + margen, w * fx, h * fy),
        huella: m.huella,
        angulo: m.angulo,
        centro: (m.centro.x * fx + margen, m.centro.y * fy + margen),
        id: m.id,
        generacion: m.generacion,
        sucio: None,
    };
    p.grafito(Some(cache), &mapa, g.opacidad, (fx + fy) / 2.0);
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
            // Con las flechas el pin se mueve con la barra a la vista.
            recolocar_barra(i);
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
            // Una foto abierta reducida: si ya se ve mas grande que lo
            // leido, se lee entera antes de pintar nitido.
            asegurar_resolucion(i, false);
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
            // El zoom de la barra cambia, y su sitio si el pin crecio.
            recolocar_barra(i);
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
            // Primero la guia de otro pin que se estuviera viendo (v2), y
            // luego los bordes de la pantalla. Con Alt, ninguno de los dos.
            crate::guias::esconder();
            let sin_iman = tecla_pulsada(VK_MENU);
            let guia = i.guia.take().filter(|_| !sin_iman);
            let pegado = if sin_iman {
                contenido
            } else {
                con_iman(hwnd, guia.unwrap_or(contenido), i.escala_por_cien)
            };
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
            esconder_barra(hwnd);
            crate::guias::esconder();
            (i.al_cambiar)(CambioPin::Cerrado);
            // SAFETY: destruye la ventana propia; WM_NCDESTROY libera el Box.
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

/// Si este `WM_POINTER*` del pin que se anota es de un contacto de borrador
/// y hay que tragarlo (D107). Misma regla que el overlay: se pregunta al
/// lapiz solo al bajar, y despues manda el estado (`decidir_goma`).
fn tragar_goma(mensaje: u32, wparam: WPARAM) -> bool {
    use pixpin_shell::puntero::FaseContacto;
    let fase = match mensaje {
        WM_POINTERDOWN => FaseContacto::Baja,
        WM_POINTERUP => FaseContacto::Sube,
        _ => FaseContacto::Actualiza,
    };
    let goma = fase == FaseContacto::Baja
        && pixpin_shell::puntero::indicadores_de_lapiz(wparam)
            .is_some_and(pixpin_shell::puntero::es_goma);
    let estado = ESTADO_LAPIZ.with(|e| e.get());
    let Some(nuevo) = pixpin_shell::puntero::decidir_goma(fase, goma, estado) else {
        return false;
    };
    ESTADO_LAPIZ.with(|e| e.set(nuevo));
    // Nada del raton de antes o de durante el borrador vale para el trazo
    // siguiente.
    HISTORIAL.with(|h| h.borrow_mut().olvidar());
    true
}

/// Abre el menu del pin en `punto` (pantalla), o donde este el raton, y
/// hace lo elegido. Lo abren el clic derecho y el boton «Mas» de la barra.
fn abrir_menu(hwnd: HWND, punto: Option<(i32, i32)>) {
    esconder_barra(hwnd);
    let Some(i) = interno_de(hwnd) else {
        return;
    };
    let Some(t) = i.textos.clone() else {
        return;
    };
    let reproduciendo = i.video.as_ref().is_some_and(|v| v.reproduciendo())
        || (i.fuente_viva.is_some() && !i.vivo_pausado);
    // Cuantas paginas tiene se pregunta la PRIMERA vez que se
    // abre el menu de un PDF, no al nacer el pin: abrir el
    // documento cuesta, y la mayoria de los pines nunca ven su
    // menu.
    if i.pdf_paginas.is_none() && es_pdf(i) {
        (i.al_cambiar)(CambioPin::PaginaPedida(0));
    }
    let estado = crate::menu::EstadoMenu {
        con_grupo: i.color_sombra.is_some(),
        reproduciendo,
        pasante: i.pasante,
        con_ocr: i.con_ocr,
        paginas: i.pdf_paginas,
        pagina: i.pdf_pagina,
        remoto: i.remoto,
        pizarra: i.pizarra,
        opacidad: (i.opacidad * 100.0).round() as u8,
    };
    match crate::menu::mostrar(hwnd, &i.contenido, estado, &t, punto) {
        None => {}
        // Tambien de esta ventana y de nadie mas: es como se
        // interpretan SUS clics.
        Some(crate::menu::CMD_REMOTO) => alternar_remoto(hwnd, i),
        // Las dos que puede resolver la propia ventana se
        // resuelven aqui: pedirselas al gestor solo daria un
        // rodeo para volver al mismo sitio.
        Some(crate::menu::CMD_PASANTE) => {
            // Se resuelve aqui, como el tamano: el paso de
            // clics es un estilo de ESTA ventana, y pedirselo
            // al gestor seria un rodeo para volver al mismo
            // sitio. Se avisa del cambio para que quede
            // guardado y el pin vuelva pasante tras reiniciar.
            poner_pasante_en(hwnd, true);
            (i.al_cambiar)(CambioPin::Movido(colocacion_de(i, i.estado.rect())));
        }
        Some(crate::menu::CMD_TAMANO_ORIGINAL) => aplicar(hwnd, EfectoPin::AlternarTamano),
        Some(crate::menu::CMD_CERRAR) => aplicar(hwnd, EfectoPin::Cerrar),
        // Los nuevos del rediseno v2: los mismos que la barra.
        Some(crate::menu::CMD_ANOTAR) => (i.al_cambiar)(CambioPin::AnotarPedido),
        Some(crate::menu::CMD_ABRIR) => (i.al_cambiar)(CambioPin::AbrirPedido),
        Some(crate::menu::CMD_PINES_ABIERTOS) => (i.al_cambiar)(CambioPin::PinesAbiertosPedido),
        Some(c)
            if (crate::menu::CMD_OPACIDAD_BASE
                ..crate::menu::CMD_OPACIDAD_BASE + crate::menu::OPACIDADES.len() as u32)
                .contains(&c) =>
        {
            let o = crate::menu::OPACIDADES[(c - crate::menu::CMD_OPACIDAD_BASE) as usize];
            i.opacidad = opacidad_valida(o as f32 / 100.0);
            pintar(i);
            (i.al_cambiar)(CambioPin::Movido(colocacion_de(i, i.estado.rect())));
        }
        // Los del video tambien se resuelven aqui: el reproductor
        // vive en esta ventana (D64/D68).
        Some(crate::menu::CMD_REPRODUCIR) => {
            if matches!(i.contenido, Contenido::Vivo { .. }) {
                alternar_vivo(i);
            } else {
                alternar_video(hwnd, i);
            }
        }
        Some(crate::menu::CMD_SONIDO) => {
            if let Some(v) = &i.video {
                v.alternar_sonido();
                tracing::info!(silenciado = v.silenciado(), "sonido del video alternado");
            }
        }
        Some(cmd) => {
            let cambio = match cmd {
                crate::menu::CMD_COPIAR => Some(CambioPin::CopiarPedido),
                crate::menu::CMD_GUARDAR_COMO => Some(CambioPin::GuardarComoPedido),
                crate::menu::CMD_TEXTO => Some(CambioPin::TextoPedido),
                crate::menu::CMD_ABRIR_LIENZO => Some(CambioPin::AbrirLienzoPedido),
                crate::menu::CMD_CONGELAR => Some(CambioPin::CongelarPedido),
                crate::menu::CMD_PAGINA_SIGUIENTE => Some(CambioPin::PaginaPedida(1)),
                crate::menu::CMD_PAGINA_ANTERIOR => Some(CambioPin::PaginaPedida(-1)),
                crate::menu::CMD_EXTRAER_PAGINA => Some(CambioPin::ExtraerPaginaPedida),
                crate::menu::CMD_EXTRAER_TODAS => Some(CambioPin::ExtraerTodasPedida),
                crate::menu::CMD_ABRIR_UBICACION => Some(CambioPin::AbrirUbicacionPedido),
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
                c if (crate::menu::CMD_CONVERTIR_BASE
                    ..crate::menu::CMD_CONVERTIR_BASE
                        + crate::magia::MiniApp::TODAS.len() as u32)
                    .contains(&c) =>
                {
                    Some(CambioPin::ConvertirPedido(
                        (c - crate::menu::CMD_CONVERTIR_BASE) as u8,
                    ))
                }
                // Cambiar el color deja la pauta y al reves: son
                // dos decisiones sobre el mismo papel.
                c if (crate::menu::CMD_PIZARRA_COLOR_BASE
                    ..crate::menu::CMD_PIZARRA_COLOR_BASE
                        + crate::menu::COLORES_PIZARRA as u32)
                    .contains(&c) =>
                {
                    let (_, pauta) = i.pizarra.unwrap_or((0, 0));
                    Some(CambioPin::PizarraPedida {
                        color: (c - crate::menu::CMD_PIZARRA_COLOR_BASE) as u8,
                        pauta,
                    })
                }
                c if (crate::menu::CMD_PIZARRA_PAUTA_BASE
                    ..crate::menu::CMD_PIZARRA_PAUTA_BASE + crate::menu::PAUTAS_PIZARRA as u32)
                    .contains(&c) =>
                {
                    let (color, _) = i.pizarra.unwrap_or((0, 0));
                    Some(CambioPin::PizarraPedida {
                        color,
                        pauta: (c - crate::menu::CMD_PIZARRA_PAUTA_BASE) as u8,
                    })
                }
                _ => None,
            };
            if let Some(c) = cambio {
                (i.al_cambiar)(c);
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

    /// Como `punto_contenido`, desde coordenadas del escritorio virtual y
    /// sin redondear: las muestras traen subpixel.
    fn muestra_a_contenido(i: &PinInterno, m: pixpin_shell::puntero::Muestra) -> (f32, f32) {
        let mut r = RECT::default();
        // SAFETY: GetWindowRect sobre la ventana del propio pin.
        unsafe {
            let _ = GetWindowRect(i.hwnd, &mut r);
        }
        let (ox, oy) = origen_contenido(i.hwnd, i.estado.rect());
        (
            m.x() - r.left as f32 - ox as f32,
            m.y() - r.top as f32 - oy as f32,
        )
    }

    match mensaje {
        WM_LBUTTONDOWN => {
            // `Ctrl` + arrastrar saca el contenido del pin hacia OTRA
            // aplicacion (arrastrar y soltar saliente). Va lo PRIMERO, por
            // delante de la seleccion de texto: con `Ctrl` pulsado el
            // usuario quiere llevarse el contenido, no marcar palabras.
            //
            // La carga se monta y el prestamo de `PinInterno` se suelta
            // ANTES de arrancar el arrastre. `DoDragDrop` es modal y bombea
            // mensajes, asi que el WndProc vuelve a entrar mientras dura y
            // pediria otra vez el mismo `&mut`.
            let carga = interno_de(hwnd).and_then(|i| {
                if tecla_pulsada(VK_CONTROL) && !i.anotando {
                    // Lo que se lleva es la foto entera, no la leida a la
                    // medida del pin.
                    asegurar_resolucion(i, true);
                    crate::arrastre::carga_de(&i.contenido, i.ruta_origen.as_deref())
                } else {
                    None
                }
            });
            if let Some(carga) = carga {
                // Un fallo al montar los datos cancela el gesto y nada mas:
                // el bucle de interfaz no se puede parar por esto.
                if let Err(e) = crate::arrastre::arrastrar(carga) {
                    tracing::warn!(?e, "no se pudo arrastrar el contenido del pin");
                }
                return LRESULT(0);
            }
            // Un clic sobre el video (sin moverlo) reproduce o pausa: se
            // apunta donde se pulso y lo decide el soltar. Los mandos van
            // en la barra del pin (v2), fuera de la imagen.
            if let Some(i) = interno_de(hwnd)
                && i.video.is_some()
                && !i.anotando
                && !tecla_pulsada(VK_CONTROL)
            {
                let c = cursor_de_pantalla();
                i.pulsado_video = Some((c.x, c.y));
            }
            // La barra se esconde mientras se arrastra el pin.
            esconder_barra(hwnd);
            // Sobre una palabra reconocida, el boton izquierdo SELECCIONA
            // texto en vez de mover el pin: es lo que lo hace parecerse a
            // un documento. Fuera del texto, mover, como siempre.
            if let Some(i) = interno_de(hwnd) {
                if !i.anotando {
                    if let Some(renglones) = &i.texto_ocr {
                        let nativo = a_pixeles_nativos(i, punto_contenido(i, lparam));
                        if let Some(sitio) =
                            pixpin_geom::seleccion_texto::sitio_en(renglones, nativo, HOLGURA_TEXTO)
                        {
                            // SAFETY: captura sobre ventana propia; se
                            // suelta en WM_LBUTTONUP.
                            unsafe { SetCapture(hwnd) };
                            i.ancla_texto = Some(sitio);
                            i.seleccion_texto = vec![sitio];
                            pintar(i);
                            return LRESULT(0);
                        }
                    }
                    // Pulsar fuera del texto quita la seleccion: es lo que
                    // hace cualquier documento y evita que se quede una
                    // marca azul olvidada encima de la imagen.
                    if !i.seleccion_texto.is_empty() {
                        i.seleccion_texto.clear();
                        pintar(i);
                    }
                }
            }
            // SAFETY: captura para no perder el arrastre al salir del borde.
            unsafe { SetCapture(hwnd) };
            // Clic tambien enfoca: es lo que arma el Esc de D23.
            // SAFETY: foco sobre ventana propia.
            unsafe {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(hwnd));
            }
            if let Some(i) = interno_de(hwnd) {
                if i.anotando {
                    // El pulsado siembra el historial (I2): sin el, el primer
                    // movimiento no recupera los puntos fusionados y cada
                    // trazo rapido empieza con una cuerda recta. El pulsado
                    // que sintetiza el lapiz no: sus puntos vienen por
                    // WM_POINTERUPDATE.
                    if pixpin_shell::puntero::usa_historial_raton(
                        pixpin_shell::puntero::origen_actual(),
                    ) {
                        let p = punto(lparam);
                        // SAFETY: tiempo del mensaje que se esta atendiendo.
                        let tiempo = unsafe { GetMessageTime() } as u32;
                        HISTORIAL.with(|h| h.borrow_mut().sembrar(p.x, p.y, tiempo));
                    }
                    (i.al_cambiar)(CambioPin::PunteroPulsado(punto_contenido(i, lparam)));
                } else {
                    // Ctrl + arrastrar: zoom (arriba agranda, abajo encoge).
                    // Desde que `Ctrl` + arrastrar saca el contenido hacia
                    // fuera, aqui solo se llega cuando NO habia nada que
                    // arrastrar: una ficha o un documento a los que nadie
                    // les dijo su ruta. Se deja como red, no como gesto
                    // anunciado: la rueda sigue siendo el zoom de siempre.
                    // SAFETY: consulta pura del estado del teclado.
                    let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                    // Manejando a distancia el boton izquierdo es DE LA ZONA:
                    // se apunta donde se pulso y al soltar se reenvia como
                    // clic o como arrastre. El pin no se mueve. Lo pidio el
                    // usuario asi: «como el clic se transmite a la zona real,
                    // para mover se mantiene Control mas clic».
                    let remoto = i.remoto && matches!(i.contenido, Contenido::Vivo { .. });
                    if remoto && !ctrl {
                        let c = cursor_de_pantalla();
                        i.pulsado_remoto = Some((c.x, c.y));
                        return LRESULT(0);
                    }
                    // Y con `Ctrl`, manejando, se MUEVE (no el zoom por
                    // arrastre de abajo): es el unico gesto que le queda al
                    // pin para cambiar de sitio.
                    // Sobre una herramienta el pulsado se apunta para lo de
                    // dentro Y el pin sigue pudiendo moverse: al soltar se
                    // decide si fue un toque (pulsar un boton, tachar una
                    // tarea) o un arrastre, como en el pin del movil.
                    if i.contenido.interactivo() && !ctrl {
                        let c = cursor_de_pantalla();
                        i.pulsado_interior = Some((c.x, c.y));
                    }
                    let evento = if ctrl && !remoto {
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
        // El boton derecho hace dos cosas segun lo que pase despues: si te
        // mueves, hace zoom; si no, abre el menu al soltar. Por eso aqui
        // solo se APUNTA, y quien decide es WM_RBUTTONUP.
        WM_RBUTTONDOWN => {
            if let Some(i) = interno_de(hwnd) {
                // Ni sobre una nota que se esta escribiendo ni mientras se
                // anota: ahi el boton derecho tiene otro dueno.
                if !i.anotando {
                    // SAFETY: captura sobre ventana propia; se suelta en
                    // WM_RBUTTONUP, que siempre llega tras esto.
                    unsafe { SetCapture(hwnd) };
                    i.arrastre_derecho = Some((y_de_pantalla(), false));
                }
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE if interno_de(hwnd).is_some_and(|i| i.arrastre_derecho.is_some()) => {
            if let Some(i) = interno_de(hwnd) {
                if let Some((y_ultima, movido)) = i.arrastre_derecho {
                    let y = y_de_pantalla();
                    let dy = y - y_ultima;
                    // Hasta pasar el umbral no se toca nada: el pulso de
                    // la mano al pulsar no puede robarle el menu al clic.
                    if !movido && dy.abs() < crate::zoom::UMBRAL_ARRASTRE {
                        return LRESULT(0);
                    }
                    let delta = crate::zoom::pasos_de_arrastre(dy);
                    i.arrastre_derecho = Some((y, true));
                    if delta != 0 {
                        // Se reusa el mismo camino que la rueda: el gestor
                        // ya sabe animar el zoom anclado a un punto, y
                        // duplicarlo aqui daria dos comportamientos que se
                        // separarian con el tiempo.
                        (i.al_cambiar)(CambioPin::RuedaGirada {
                            delta,
                            cursor: cursor_de_pantalla(),
                        });
                    }
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
                // Manejando a distancia los papeles se cruzan, porque la rueda
                // sola es de la zona: `Ctrl + rueda` pasa a ser el zoom de
                // SIEMPRE (agrandar el pin), que es lo que pidio el usuario,
                // y el zoom interior se queda sin gesto mientras dure el modo.
                let remoto = i.remoto && matches!(i.contenido, Contenido::Vivo { .. });
                // Mayus + rueda: la opacidad del pin, un 10 % por muesca (v2).
                // Ctrl + rueda ya es el zoom de dentro, y no se le quita.
                if tecla_pulsada(VK_SHIFT) && !tecla_pulsada(VK_CONTROL) && !i.anotando {
                    let nueva = opacidad_tras_rueda(i.opacidad, delta);
                    if (nueva - i.opacidad).abs() > f32::EPSILON {
                        i.opacidad = nueva;
                        pintar(i);
                        // Se guarda al parar de girar, como las flechas.
                        // SAFETY: temporizador sobre ventana propia.
                        unsafe {
                            SetTimer(
                                Some(hwnd),
                                ID_TEMPORIZADOR_GUARDADO,
                                RETARDO_GUARDADO_MS,
                                None,
                            );
                        }
                    }
                    return LRESULT(0);
                }
                // Sobre una herramienta la rueda baja por su lista, que es lo
                // unico que se desplaza; con `Ctrl`, agranda el pin como
                // siempre (lo de dentro se recoloca, no hay zoom interior).
                if i.contenido.interactivo() && !i.anotando {
                    if !tecla_pulsada(VK_CONTROL) {
                        (i.al_cambiar)(CambioPin::RuedaInterior { delta });
                        return LRESULT(0);
                    }
                } else if tecla_pulsada(VK_CONTROL) && !i.anotando && !remoto {
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
                // Manejando a distancia, la rueda sola es para la zona (hacer
                // scroll en lo que se ve); con `Ctrl` sigue hasta abajo y
                // agranda el pin.
                if !tecla_pulsada(VK_CONTROL)
                    && let Some((x, y)) = punto_remoto(i, (cursor.x, cursor.y))
                {
                    (i.al_cambiar)(CambioPin::RuedaRemota { x, y, delta });
                    return LRESULT(0);
                }
                (i.al_cambiar)(CambioPin::RuedaGirada { delta, cursor });
            }
            LRESULT(0)
        }
        m if m == WM_RATON_FUERA => {
            // La salida la vigila el temporizador de la barra: pasar del pin
            // a su barra tambien es salir del pin, y no debe esconderla.
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if let Some(i) = interno_de(hwnd) {
                // Con el raton encima sale la barra del pin (v2). No con un
                // boton pulsado: arrastrando, la barra estorba.
                const BOTONES: usize = 0x0001 | 0x0002 | 0x0010;
                if (!i.raton_encima || !crate::barra_flotante::es_de(hwnd))
                    && wparam.0 & BOTONES == 0
                    && quiere_barra(i)
                {
                    i.raton_encima = true;
                    ensenar_barra(i);
                }
                // Arrastrando una seleccion de texto: se amplia y se
                // repinta, y el pin NO se mueve.
                if let Some(ancla) = i.ancla_texto {
                    if let Some(renglones) = &i.texto_ocr {
                        let nativo = a_pixeles_nativos(i, punto_contenido(i, lparam));
                        if let Some(hasta) =
                            pixpin_geom::seleccion_texto::sitio_en(renglones, nativo, HOLGURA_TEXTO)
                        {
                            i.seleccion_texto =
                                pixpin_geom::seleccion_texto::seleccion(renglones, ancla, hasta);
                            pintar(i);
                        }
                    }
                    return LRESULT(0);
                }
                if i.anotando {
                    const MK_LBUTTON: usize = 0x0001;
                    let boton = wparam.0 & MK_LBUTTON != 0;
                    // Mismas reglas que el overlay (Tareas 7 y 11):
                    // `descartar_movimiento` (pura, con su propia tabla de
                    // pruebas) dice si este WM_MOUSEMOVE sintetizado sobra
                    // porque el lapiz (WM_POINTERUPDATE) ya lo cubrio, o es
                    // su borrador y no debe reaparecer como tinta (D107).
                    let origen = pixpin_shell::puntero::origen_actual();
                    let estado = ESTADO_LAPIZ.with(|e| e.get());
                    if pixpin_shell::puntero::descartar_movimiento(boton, origen, estado) {
                        // Tirado, pero el historial se olvida (M3): si no, un
                        // SinDatos a mitad del trazo recuperaria desde un
                        // punto viejo y repetiria puntos ya entregados.
                        HISTORIAL.with(|h| h.borrow_mut().olvidar());
                        return LRESULT(0);
                    }
                    // SAFETY: tiempo del mensaje que se esta atendiendo.
                    let tiempo = unsafe { GetMessageTime() } as u32;
                    let mut muestras: Vec<(f32, f32)> = Vec::new();
                    if !boton {
                        // Se acabo el trazo (o nunca hubo boton): el
                        // siguiente no debe heredar el estado del lapiz de
                        // este, o un fallo puntual del historial del
                        // siguiente trazo se confundiria con un ConMuestras
                        // viejo y le tiraria el primer movimiento. Un
                        // contacto de borrador en curso lo cierra su
                        // WM_POINTERUP.
                        HISTORIAL.with(|h| h.borrow_mut().olvidar());
                        if estado != pixpin_shell::puntero::EstadoLapiz::ContactoDeGoma {
                            ESTADO_LAPIZ
                                .with(|e| e.set(pixpin_shell::puntero::EstadoLapiz::SinDatos));
                        }
                    } else if !pixpin_shell::puntero::usa_historial_raton(origen)
                        || pixpin_shell::puntero::olvida_historial_raton(estado)
                    {
                        // Un movimiento del lapiz no se recupera del
                        // historial del raton (M3).
                        HISTORIAL.with(|h| h.borrow_mut().olvidar());
                    } else {
                        let c = punto_contenido(i, lparam);
                        let mut r = RECT::default();
                        // SAFETY: GetWindowRect sobre la ventana propia.
                        let (x, y) = unsafe {
                            let _ = GetWindowRect(hwnd, &mut r);
                            let (ox, oy) = origen_contenido(i.hwnd, i.estado.rect());
                            (r.left + ox + c.x, r.top + oy + c.y)
                        };
                        // Todo a coordenadas del contenido ANTES de avisar:
                        // el callback no vuelve a encontrarse `i` a medias.
                        muestras = HISTORIAL
                            .with(|h| h.borrow_mut().recuperar(x, y, tiempo))
                            .into_iter()
                            .map(|m| muestra_a_contenido(i, m))
                            .collect();
                    }
                    let movido = punto_contenido(i, lparam);
                    for (x, y) in muestras {
                        (i.al_cambiar)(CambioPin::MuestraPuntero {
                            x,
                            y,
                            presion: None,
                        });
                    }
                    (i.al_cambiar)(CambioPin::PunteroMovido(movido));
                } else {
                    let e = i.estado.procesar(EventoPin::RatonMovido(punto(lparam)));
                    // Moviendo el pin entero, las guias con los demas pines
                    // (v2). Se ensenan, no se pegan: se pega al soltar. Con
                    // Alt, sin guias.
                    if let EfectoPin::Mover(r) = e
                        && i.estado.moviendo()
                    {
                        if tecla_pulsada(VK_MENU) {
                            i.guia = None;
                            crate::guias::esconder();
                        } else {
                            let a = guias_para(hwnd, i, r);
                            let grosor = (2 * i.escala_por_cien as i32 / 100).max(2);
                            crate::guias::ensenar(&a.guias, grosor);
                            i.guia = (!a.guias.is_empty()).then_some(a.rect);
                        }
                    }
                    aplicar(hwnd, e);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP if interno_de(hwnd).is_some_and(|i| i.ancla_texto.is_some()) => {
            if let Some(i) = interno_de(hwnd) {
                i.ancla_texto = None;
            }
            // SAFETY: libera la captura tomada al empezar la seleccion.
            unsafe {
                let _ = ReleaseCapture();
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
                    // Se olvida aqui, no solo en el siguiente WM_MOUSEMOVE:
                    // un simple clic (pulsar y soltar sin mover) no pasa por
                    // ahi, y el estado del lapiz de este trazo se colaria
                    // en el siguiente.
                    HISTORIAL.with(|h| h.borrow_mut().olvidar());
                    ESTADO_LAPIZ.with(|e| e.set(pixpin_shell::puntero::EstadoLapiz::SinDatos));
                    (i.al_cambiar)(CambioPin::PunteroSoltado(punto_contenido(i, lparam)));
                } else {
                    let e = i.estado.procesar(EventoPin::BotonSoltado);
                    aplicar(hwnd, e);
                }
            }
            // El clic a distancia, DESPUES de cerrar el arrastre del pin y
            // con el prestamo pedido de nuevo (`aplicar` pide el suyo).
            if let Some(i) = interno_de(hwnd)
                && let Some(pulsado) = i.pulsado_remoto.take()
            {
                let c = cursor_de_pantalla();
                // Un arrastre que empieza o acaba fuera de lo que se ve no se
                // reenvia: a medias seria pulsar sin soltar en la zona.
                if let (Some((x0, y0)), Some((x, y))) =
                    (punto_remoto(i, pulsado), punto_remoto(i, (c.x, c.y)))
                {
                    if crate::remoto::fue_clic(pulsado, (c.x, c.y)) {
                        (i.al_cambiar)(CambioPin::ClicRemoto { x, y });
                    } else {
                        (i.al_cambiar)(CambioPin::ArrastreRemoto {
                            x0,
                            y0,
                            x1: x,
                            y1: y,
                        });
                    }
                }
            }
            // El clic sobre un video (sin moverlo) reproduce o pausa. Tambien
            // despues de cerrar el arrastre: si se movio, no se toca.
            if let Some(i) = interno_de(hwnd)
                && let Some(pulsado) = i.pulsado_video.take()
            {
                let c = cursor_de_pantalla();
                if crate::remoto::fue_clic(pulsado, (c.x, c.y)) {
                    alternar_video(hwnd, i);
                }
            }
            // El toque sobre una herramienta, tambien DESPUES de cerrar el
            // arrastre del pin: si fue un arrastre, el pin ya se movio y aqui
            // no se manda nada.
            if let Some(i) = interno_de(hwnd)
                && let Some(pulsado) = i.pulsado_interior.take()
            {
                let c = cursor_de_pantalla();
                if crate::remoto::fue_clic(pulsado, (c.x, c.y)) {
                    let p = punto_contenido(i, lparam);
                    (i.al_cambiar)(CambioPin::ClicInterior { x: p.x, y: p.y });
                }
            }
            LRESULT(0)
        }
        // Las muestras buenas del lapiz (E1): tinta de verdad con presion, o
        // el borrador (D107), del mismo mensaje que WM_MOUSEMOVE sintetiza
        // despues. Solo mientras se anota: fuera de ese modo el pin no
        // dibuja nada y no hay estado de lapiz que mantener.
        // El borrador del lapiz (D107): un contacto que empieza con el se
        // traga entero, sin DefWindowProc, para que Windows no sintetice un
        // WM_LBUTTONDOWN que la anotacion convertiria en un punto.
        WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP
            if interno_de(hwnd).is_some_and(|i| i.anotando) && tragar_goma(mensaje, wparam) =>
        {
            LRESULT(0)
        }
        WM_POINTERUPDATE if interno_de(hwnd).is_some_and(|i| i.anotando) => {
            let muestras = pixpin_shell::puntero::muestras_de_lapiz(wparam);
            // Se recuerda el estado para que el WM_MOUSEMOVE que Windows
            // sintetiza a partir de este mismo mensaje pueda distinguir
            // "el lapiz ya respondio" (tinta o borrador) de "sin datos"
            // (ver descartar_movimiento).
            let estado = match &muestras {
                pixpin_shell::puntero::MuestrasLapiz::SinDatos => {
                    pixpin_shell::puntero::EstadoLapiz::SinDatos
                }
                pixpin_shell::puntero::MuestrasLapiz::Goma => {
                    pixpin_shell::puntero::EstadoLapiz::Goma
                }
                pixpin_shell::puntero::MuestrasLapiz::Muestras(_) => {
                    pixpin_shell::puntero::EstadoLapiz::ConMuestras
                }
            };
            ESTADO_LAPIZ.with(|e| e.set(estado));
            // El lapiz respondio: el ultimo punto del raton recordado ya no
            // es el ultimo entregado (M3).
            if pixpin_shell::puntero::olvida_historial_raton(estado) {
                HISTORIAL.with(|h| h.borrow_mut().olvidar());
            }
            if let (Some(i), pixpin_shell::puntero::MuestrasLapiz::Muestras(muestras)) =
                (interno_de(hwnd), muestras)
            {
                // Todo a coordenadas del contenido ANTES de avisar a nadie.
                let convertidas: Vec<(f32, f32, Option<f32>)> = muestras
                    .into_iter()
                    .map(|m| {
                        let (x, y) = muestra_a_contenido(i, m);
                        (x, y, m.presion())
                    })
                    .collect();
                for (x, y, presion) in convertidas {
                    (i.al_cambiar)(CambioPin::MuestraPuntero { x, y, presion });
                }
            }
            // Se deja pasar a DefWindowProc: asi Windows sigue sintetizando
            // los clics del lapiz, que es como se pulsa y se suelta.
            // SAFETY: reenvio estandar del mensaje recibido.
            unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
        }
        WM_LBUTTONDBLCLK => {
            if let Some(i) = interno_de(hwnd) {
                // En una ficha, doble clic ABRE el archivo (spec 4.1). En
                // imagen y nota entra a ANOTAR (D47): alternar tamano pasa
                // al menu, donde ya estaba "Tamano original".
                if i.contenido.interactivo() {
                    // En una herramienta la segunda pulsacion es otro toque:
                    // dos veces «+1» son dos, como dos toques en el movil.
                    // Anotar encima de una lista no significa nada.
                    let c = cursor_de_pantalla();
                    i.pulsado_interior = Some((c.x, c.y));
                } else if matches!(
                    i.contenido,
                    Contenido::Archivo { .. } | Contenido::Documento { .. }
                ) {
                    (i.al_cambiar)(CambioPin::AbrirPedido);
                } else if matches!(i.contenido, Contenido::Video { .. }) {
                    // El primer clic del doble ya reprodujo o pauso: aqui no
                    // se hace nada, o el doble clic se anularia a si mismo.
                    // Sin reproductor (fallo), el doble clic sigue siendo
                    // reproducir/pausar, que no hace nada.
                } else if matches!(i.contenido, Contenido::Vivo { .. }) {
                    if i.remoto {
                        // Manejando a distancia, la segunda pulsacion de un
                        // doble clic es otro clic para la zona: llegan dos
                        // seguidos al mismo punto y alli SI es un doble clic.
                        let c = cursor_de_pantalla();
                        i.pulsado_remoto = Some((c.x, c.y));
                    } else {
                        // Y en un pin en vivo, igual: es un video de la
                        // pantalla.
                        alternar_vivo(i);
                    }
                } else if !i.anotando {
                    (i.al_cambiar)(CambioPin::AnotarPedido);
                }
            }
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            if let Some(i) = interno_de(hwnd) {
                let arrastro = i.arrastre_derecho.map(|(_, m)| m).unwrap_or(false);
                if i.arrastre_derecho.take().is_some() {
                    // SAFETY: libera la captura tomada en WM_RBUTTONDOWN.
                    unsafe {
                        let _ = ReleaseCapture();
                    }
                }
                // Si hubo zoom, el boton ya hizo su trabajo: abrir ademas
                // el menu seria una sorpresa al final de cada gesto.
                if arrastro {
                    return LRESULT(0);
                }
            }
            abrir_menu(hwnd, None);
            LRESULT(0)
        }
        WM_SETFOCUS | WM_KILLFOCUS => {
            if let Some(i) = interno_de(hwnd) {
                i.enfocado = mensaje == WM_SETFOCUS;
                pintar(i);
            }
            LRESULT(0)
        }
        // Con una herramienta enfocada, el teclado es de lo de dentro: sus
        // flechas eligen fila, Intro anade, Supr borra, Esc deshace (lo
        // decide `mini_panel` en el gestor). Van ANTES que los atajos del
        // pin: una «r» escrita en la caja de una tarea no puede girar el pin.
        // `Ctrl + C` sigue siendo copiar el pin.
        WM_CHAR if interno_de(hwnd).is_some_and(|i| i.contenido.interactivo() && !i.anotando) => {
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
            // Los de control (Intro, Retroceso, Esc, Tab) ya llegaron como
            // tecla: mandarlos tambien como caracter los haria dos veces.
            if let (Some(i), Some(c)) = (interno_de(hwnd), caracter)
                && !c.is_control()
            {
                (i.al_cambiar)(CambioPin::CaracterInterior(c));
            }
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN
            if interno_de(hwnd).is_some_and(|i| i.contenido.interactivo() && !i.anotando)
                && !(tecla_pulsada(VK_CONTROL) && wparam.0 as u32 == b'C' as u32)
                && (mensaje == WM_KEYDOWN || es_flecha(wparam.0 as u32)) =>
        {
            if let Some(i) = interno_de(hwnd) {
                (i.al_cambiar)(CambioPin::TeclaInterior {
                    vk: wparam.0 as u32,
                    shift: tecla_pulsada(VK_SHIFT),
                    ctrl: tecla_pulsada(VK_CONTROL),
                    alt: tecla_pulsada(VK_MENU),
                });
            }
            LRESULT(0)
        }
        // La vigilancia de la barra: si el raton ya no esta ni en el pin ni
        // en su barra (y no se esta arrastrando la linea de tiempo), fuera.
        WM_TIMER if wparam.0 == ID_TEMPORIZADOR_BARRA => {
            let c = cursor_de_pantalla();
            let mut r = RECT::default();
            // SAFETY: GetWindowRect sobre la ventana propia.
            unsafe {
                let _ = GetWindowRect(hwnd, &mut r);
            }
            let en_pin = interno_de(hwnd).is_some_and(|i| i.estado.rect().contiene(c))
                && c.x >= r.left
                && c.x < r.right
                && c.y >= r.top
                && c.y < r.bottom;
            let sigue = en_pin
                || crate::barra_flotante::contiene(c.x, c.y)
                || interno_de(hwnd)
                    .is_some_and(|i| crate::barra_flotante::en_el_paso(i.estado.rect(), c.x, c.y))
                || crate::barra_flotante::ocupada();
            if !sigue || !crate::barra_flotante::es_de(hwnd) {
                esconder_barra(hwnd);
                if let Some(i) = interno_de(hwnd) {
                    i.raton_encima = false;
                }
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == ID_TEMPORIZADOR_LATIDO => {
            if let Some(i) = interno_de(hwnd) {
                pintar(i);
                (i.al_cambiar)(CambioPin::Latido);
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
        // Los atajos que ensenan la barra y el menu del pin (v2). De UNA
        // tecla y solo con el pin enfocado; ninguno choca con los de antes
        // (1-3, 5-8, 0, R, H, V, T, M, C, Espacio, Esc, flechas).
        //
        // A: anotar encima (lo mismo que el doble clic, en fotos y notas).
        WM_KEYDOWN
            if wparam.0 as u32 == b'A' as u32
                && !tecla_pulsada(VK_CONTROL)
                && interno_de(hwnd)
                    .is_some_and(|i| !i.anotando && crate::menu::anotable(&i.contenido)) =>
        {
            esconder_barra(hwnd);
            if let Some(i) = interno_de(hwnd) {
                (i.al_cambiar)(CambioPin::AnotarPedido);
            }
            LRESULT(0)
        }
        // Ctrl+0: tamano original. Va antes que el 0 de los filtros, que no
        // mira el Ctrl.
        WM_KEYDOWN
            if wparam.0 as u32 == 0x30
                && tecla_pulsada(VK_CONTROL)
                && interno_de(hwnd).is_some_and(|i| !i.anotando) =>
        {
            aplicar(hwnd, EfectoPin::AlternarTamano);
            LRESULT(0)
        }
        // Ctrl+T: dejar pasar el clic. La vuelta es el comando global, como
        // desde el menu.
        WM_KEYDOWN
            if wparam.0 as u32 == b'T' as u32
                && tecla_pulsada(VK_CONTROL)
                && interno_de(hwnd).is_some_and(|i| !i.anotando && !i.pasante) =>
        {
            esconder_barra(hwnd);
            poner_pasante_en(hwnd, true);
            if let Some(i) = interno_de(hwnd) {
                (i.al_cambiar)(CambioPin::Movido(colocacion_de(i, i.estado.rect())));
            }
            LRESULT(0)
        }
        // RePag y AvPag: la pagina anterior y la siguiente de un PDF.
        WM_KEYDOWN
            if matches!(wparam.0 as u32, 0x21 | 0x22)
                && interno_de(hwnd).is_some_and(|i| !i.anotando && i.pdf_paginas.is_some()) =>
        {
            if let Some(i) = interno_de(hwnd) {
                let salto = if wparam.0 as u32 == 0x21 { -1 } else { 1 };
                (i.al_cambiar)(CambioPin::PaginaPedida(salto));
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
        // Los filtros de imagen: 5 gris, 6 invertir, 7 y 8 el brillo, y 0
        // devuelve la imagen a como era. Son los numeros del original.
        //
        // Ninguno toca lo guardado: se rehace el bitmap desde el original
        // con los filtros puestos, asi que 0 restaura de verdad en vez de
        // intentar deshacer, y subir y bajar el brillo diez veces devuelve
        // la imagen exacta con la que se empezo.
        WM_KEYDOWN
            if matches!(wparam.0 as u32, 0x35 | 0x36 | 0x37 | 0x38 | 0x30)
                && interno_de(hwnd).is_some_and(|i| !i.anotando && i.bitmap.is_some()) =>
        {
            if let Some(i) = interno_de(hwnd) {
                let antes = i.filtros;
                i.filtros = match wparam.0 as u32 {
                    0x35 => pixpin_codec::filtros::Filtros {
                        gris: !antes.gris,
                        ..antes
                    },
                    0x36 => pixpin_codec::filtros::Filtros {
                        invertido: !antes.invertido,
                        ..antes
                    },
                    0x37 => antes.con_brillo(1),
                    0x38 => antes.con_brillo(-1),
                    // El 0 se lleva tambien el giro y los volteos: se llama
                    // «restaurar el original», y dejar el pin del reves
                    // despues de pedirlo seria no haberlo restaurado.
                    _ => {
                        i.giro = 0;
                        i.volteo_h = false;
                        i.volteo_v = false;
                        pixpin_codec::filtros::Filtros::default()
                    }
                };
                if i.filtros != antes || wparam.0 as u32 == 0x30 {
                    rehacer_bitmap(i);
                    pintar(i);
                    (i.al_cambiar)(CambioPin::Movido(colocacion_de(i, i.estado.rect())));
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
                } else if matches!(i.contenido, Contenido::Vivo { .. }) {
                    alternar_vivo(i);
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
        // En un video, las teclas de cualquier reproductor: M silencia,
        // izquierda/derecha saltan 5 s y arriba/abajo cambian el volumen.
        // Con Shift, las flechas mueven el pin como en los demas.
        WM_KEYDOWN
            if (es_flecha(wparam.0 as u32) && !tecla_pulsada(VK_SHIFT)
                || wparam.0 as u32 == b'M' as u32)
                && interno_de(hwnd).is_some_and(|i| i.video.is_some() && !i.anotando) =>
        {
            if let Some(i) = interno_de(hwnd)
                && let Some(v) = &i.video
            {
                use crate::mandos_video::{SALTO_FLECHA, volumen_tras_rueda};
                match wparam.0 as u32 {
                    x if x == b'M' as u32 => v.alternar_sonido(),
                    x if x == VK_LEFT.0 as u32 => v.buscar(v.posicion().0 - SALTO_FLECHA, false),
                    x if x == VK_RIGHT.0 as u32 => {
                        let (t, d) = v.posicion();
                        let destino = t + SALTO_FLECHA;
                        // Pasado el final, al final (no da la vuelta).
                        v.buscar(d.map_or(destino, |d| destino.min(d - 0.05)), false);
                    }
                    x if x == VK_UP.0 as u32 => v.poner_volumen(volumen_tras_rueda(v.volumen(), 2)),
                    _ => v.poner_volumen(volumen_tras_rueda(v.volumen(), -2)),
                }
                pintar(i);
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
        m if m == MSG_FOTOGRAMA_VIVO => {
            if let Some(i) = interno_de(hwnd) {
                // Oculto con Ctrl+2 cuenta como en pausa: nadie lo ve, y
                // copiar y pintar treinta fotogramas por segundo para nadie
                // es gastar en balde.
                // SAFETY: consulta pura sobre la ventana propia.
                if i.vivo_pausado || !unsafe { IsWindowVisible(hwnd) }.as_bool() {
                    return LRESULT(0);
                }
                // La captura manda un aviso por fotograma y no espera: si el
                // pintado va atrasado se acumulan en la cola. `tick` dice si
                // queda algo NUEVO, asi que los avisos sobrantes no copian
                // ni pintan nada.
                let textura = i.fuente_viva.as_mut().and_then(|f| f.tick());
                if let Some(t) = textura {
                    if i.bitmap.is_none() {
                        i.bitmap = i.motor.bitmap_desde_textura(&t).ok();
                    }
                    pintar(i);
                }
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == ID_TEMPORIZADOR_VIDEO => {
            tick_video(hwnd);
            LRESULT(0)
        }
        m if m == MSG_TICK_VIDEO => {
            tick_video(hwnd);
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
        // T lee el texto de la imagen, y solo entonces se puede marcar con
        // el raton.
        //
        // A peticion y NO al pasar el raton por encima, que es como estaba
        // antes: reconocer bloquea el hilo de interfaz entre 170 y 670
        // milisegundos medidos en el equipo del usuario, y como el raton
        // entra en el pin justo despues de crearlo, el tiron caia siempre
        // en el peor momento. La peor medida fue en una imagen SIN texto:
        // dos tercios de segundo de raton trabado a cambio de nada.
        WM_KEYDOWN
            if wparam.0 as u32 == b'T' as u32
                && !tecla_pulsada(VK_CONTROL)
                && interno_de(hwnd).is_some_and(|i| i.con_ocr && !i.anotando) =>
        {
            if let Some(i) = interno_de(hwnd) {
                // Solo una vez por imagen: si ya esta reconocido, volver a
                // pulsar no puede costar otro tiron.
                if i.texto_ocr.is_none() {
                    i.ocr_pedido = true;
                    (i.al_cambiar)(CambioPin::ReconocerPedido);
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN if wparam.0 as u32 == b'C' as u32 && tecla_pulsada(VK_CONTROL) => {
            if let Some(i) = interno_de(hwnd) {
                // Con texto marcado, Ctrl+C copia ESO y no la imagen: es lo
                // que espera cualquiera que acabe de seleccionar algo. Sin
                // seleccion, la imagen, como siempre.
                if copiar_seleccion(i) {
                    return LRESULT(0);
                }
                (i.al_cambiar)(CambioPin::CopiarPedido);
            }
            LRESULT(0)
        }
        // Shift+C copia TODO el texto reconocido, como en el original.
        WM_KEYDOWN
            if wparam.0 as u32 == b'C' as u32
                && tecla_pulsada(VK_SHIFT)
                && !tecla_pulsada(VK_CONTROL) =>
        {
            if let Some(i) = interno_de(hwnd) {
                if let Some(renglones) = &i.texto_ocr {
                    let todo: Vec<_> = renglones
                        .iter()
                        .enumerate()
                        .flat_map(|(r, renglon)| {
                            (0..renglon.palabras.len()).map(move |p| {
                                pixpin_geom::seleccion_texto::Sitio {
                                    renglon: r,
                                    palabra: p,
                                }
                            })
                        })
                        .collect();
                    i.seleccion_texto = todo;
                    copiar_seleccion(i);
                    pintar(i);
                }
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
                        return IDC_SIZENWSE;
                    }
                    // Barra de texto sobre lo que se puede seleccionar: es
                    // lo unico que avisa de que ahi hay texto, porque el
                    // texto reconocido no se ve.
                    if let Some(renglones) = &i.texto_ocr {
                        let (ox, oy) = origen_contenido(i.hwnd, i.estado.rect());
                        let r = i.estado.rect();
                        let dentro = Punto {
                            x: p.x - r.x - ox,
                            y: p.y - r.y - oy,
                        };
                        if pixpin_geom::seleccion_texto::hay_texto_en(
                            renglones,
                            a_pixeles_nativos(i, dentro),
                            HOLGURA_TEXTO,
                        ) {
                            return IDC_IBEAM;
                        }
                    }
                    IDC_ARROW
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
            // Un pin que se va no deja su barra ni sus guias a la vista.
            crate::barra_flotante::esconder(hwnd);
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
    fn una_foto_leida_reducida_pide_la_entera_solo_si_se_ve_mas_grande() {
        assert!(necesita_mas_resolucion((1152, 864), (2304.0, 1728.0)));
        assert!(necesita_mas_resolucion((1152, 864), (1152.0, 1000.0)));
    }

    #[test]
    fn caso_negativo_vista_a_su_medida_o_menor_no_lee_nada() {
        assert!(!necesita_mas_resolucion((1152, 864), (1152.0, 864.0)));
        assert!(!necesita_mas_resolucion((1152, 864), (600.0, 450.0)));
        // Dentro de la holgura del 5 %.
        assert!(!necesita_mas_resolucion((1000, 1000), (1040.0, 1040.0)));
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn la_lupa_hace_leer_la_foto_entera_una_vez() {
        let dir = std::env::temp_dir().join("pixpin-pin-completa");
        std::fs::create_dir_all(&dir).unwrap();
        let fichero = dir.join("entera.png");
        let entera = ImagenRgba {
            ancho: 400,
            alto: 300,
            pixeles: [90u8, 90, 200, 255].repeat(400 * 300),
        };
        pixpin_codec::guardar(&entera, &fichero, pixpin_codec::FormatoImagen::Png).unwrap();
        let d3d = d3d();
        let motor = Rc::new(MotorRender::nuevo(&d3d).unwrap());
        let reducida = ImagenRgba {
            ancho: 100,
            alto: 75,
            pixeles: [90u8, 90, 200, 255].repeat(100 * 75),
        };
        let pin = Pin::nuevo(
            &d3d,
            motor,
            Contenido::Imagen(reducida),
            Rect {
                x: 100,
                y: 100,
                ancho: 100,
                alto: 75,
            },
            100,
            true,
            16,
            Box::new(|_| {}),
        )
        .unwrap();
        pin.poner_resolucion_completa(fichero, (400, 300));
        assert_eq!(pin.resolucion_leida(), Some((100, 75)), "aun reducida");
        pin.poner_lupa(Some(LupaPin {
            fuente: Rect {
                x: 10,
                y: 10,
                ancho: 20,
                alto: 20,
            },
            destino: Rect {
                x: 0,
                y: 0,
                ancho: 60,
                alto: 60,
            },
        }));
        assert_eq!(
            pin.resolucion_leida(),
            Some((400, 300)),
            "la lupa la lee entera"
        );
        assert!(pin.pintado_bien());
    }

    /// La perdida de la GPU (4-oct-2026: `0x887A0005`, y ningun pin pintaba
    /// hasta reiniciar). Un TDR de verdad no se puede provocar en una prueba:
    /// se inyecta el fallo en el dispositivo y se comprueba que el pin deja
    /// de pintar, que se avisa una vez, y que con un dispositivo nuevo vuelve
    /// a pintar en el mismo sitio y con el mismo giro.
    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn tras_perder_el_dispositivo_el_pin_vuelve_a_pintar_en_su_sitio() {
        use pixpin_render::perdida;
        let viejo = d3d();
        let motor = Rc::new(MotorRender::nuevo(&viejo).unwrap());
        let sitio = Rect {
            x: 120,
            y: 140,
            ancho: 240,
            alto: 160,
        };
        let pin = Pin::nuevo(
            &viejo,
            motor,
            Contenido::Imagen(ImagenRgba {
                ancho: 4,
                alto: 3,
                pixeles: [10u8, 120, 220, 255].repeat(12),
            }),
            sitio,
            100,
            true,
            16,
            Box::new(|_| {}),
        )
        .expect("el pin deberia crearse");
        pin.poner_giro(1, false, false);
        assert!(pin.pintado_bien(), "con la GPU sana pinta");

        let _ = perdida::tomar_aviso();
        perdida::inyectar_perdida(&viejo, Some(perdida::DEVICE_REMOVED));
        pin.repintar();
        assert!(!pin.pintado_bien(), "con el dispositivo perdido no pinta");
        assert_eq!(perdida::motivo(&viejo), Some(perdida::DEVICE_REMOVED));
        assert!(perdida::tomar_aviso(), "la perdida se avisa");
        assert!(!perdida::tomar_aviso(), "y una sola vez");

        // Caso negativo: rehacer sobre el MISMO dispositivo perdido no
        // arregla nada (el fallo sigue ahi).
        let _ = pin.cambiar_dispositivo(&viejo, Rc::new(MotorRender::nuevo(&viejo).unwrap()), 16);
        assert!(!pin.pintado_bien());

        let nuevo = d3d();
        let motor_nuevo = Rc::new(MotorRender::nuevo(&nuevo).unwrap());
        pin.cambiar_dispositivo(&nuevo, motor_nuevo, 16)
            .expect("el pin pasa al dispositivo nuevo");
        perdida::inyectar_perdida(&viejo, None);
        assert!(
            pin.pintado_bien(),
            "con el dispositivo nuevo vuelve a pintar"
        );
        assert_eq!(pin.rect_contenido(), sitio, "en el mismo sitio");
        assert_eq!(pin.giro(), (1, false, false), "y con el mismo giro");
        // Y sigue pintando despues (no fue un fotograma suelto).
        pin.repintar();
        assert!(pin.pintado_bien());
        drop(pin);
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

    /// Un pin de video de verdad en pantalla, con su bucle de mensajes: que
    /// el ritmo siga al monitor (no al temporizador de 15,6 ms), cuantos
    /// fotogramas pinta por segundo y cuanta CPU gasta. Lo mismo mide, para
    /// comparar, lo que daba `SetTimer(16)`. Informativa: imprime.
    ///
    /// `PIXPIN_VIDEO_PRUEBA` = ruta del video. El pin sale abajo a la
    /// derecha del monitor principal unos segundos y se cierra solo.
    #[test]
    #[ignore = "necesita GPU, escritorio y PIXPIN_VIDEO_PRUEBA; --ignored --nocapture"]
    fn un_pin_de_video_va_al_ritmo_del_monitor() {
        use std::time::{Duration, Instant};
        use windows::Win32::Graphics::Direct3D11::{
            D3D11_CREATE_DEVICE_VIDEO_SUPPORT, ID3D11Multithread,
        };
        use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
        use windows::core::Interface;

        let Some(ruta) = std::env::var_os("PIXPIN_VIDEO_PRUEBA") else {
            return;
        };
        let ruta = std::path::PathBuf::from(ruta);
        let segundos: u64 = std::env::var("PIXPIN_VIDEO_SEGUNDOS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(6);

        let mut d = None;
        // SAFETY: creacion estandar con salidas locales.
        let d3d: ID3D11Device = unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d),
                None,
                None,
            )
            .expect("GPU real");
            let d: ID3D11Device = d.unwrap();
            let _ = d
                .cast::<ID3D11Multithread>()
                .unwrap()
                .SetMultithreadProtected(true);
            d
        };
        let motor = Rc::new(MotorRender::nuevo(&d3d).unwrap());
        // SAFETY: metrica del sistema, consulta pura.
        let (sw, sh) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
        let t_crear = Instant::now();
        let pin = Pin::nuevo(
            &d3d,
            motor,
            Contenido::Video {
                nombre: "prueba".into(),
                ruta,
                ancho: 0,
                alto: 0,
            },
            Rect {
                x: sw - 520,
                y: sh - 340,
                ancho: 480,
                alto: 270,
            },
            100,
            true,
            16,
            Box::new(|_| {}),
        )
        .expect("pin de video");
        let hwnd = pin.hwnd();
        pin.poner_sonido(false);

        let cpu = || {
            let (mut a, mut b, mut k, mut u) = Default::default();
            // SAFETY: salidas locales sobre el proceso propio.
            unsafe {
                let _ = GetProcessTimes(GetCurrentProcess(), &mut a, &mut b, &mut k, &mut u);
            }
            let t = |f: windows::Win32::Foundation::FILETIME| {
                ((f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64) as f64 / 1e7
            };
            t(k) + t(u)
        };
        let bombear = |hasta: Duration, avisos: &mut u64| {
            let t0 = Instant::now();
            let mut msg = MSG::default();
            while t0.elapsed() < hasta {
                // SAFETY: bucle de mensajes estandar del hilo propio.
                unsafe {
                    if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                        if msg.message == MSG_TICK_VIDEO {
                            *avisos += 1;
                        }
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    } else {
                        let _ = MsgWaitForMultipleObjects(None, false, 5, QS_ALLINPUT);
                    }
                }
            }
        };
        let fotogramas = || {
            interno_de(hwnd)
                .and_then(|i| i.video.as_ref().map(|v| v.fotogramas()))
                .unwrap_or(0)
        };

        // Hasta el primer fotograma.
        let mut avisos = 0u64;
        while fotogramas() == 0 && t_crear.elapsed() < Duration::from_secs(5) {
            bombear(Duration::from_millis(5), &mut avisos);
        }
        let primer_ms = t_crear.elapsed().as_millis();
        // Medida en regimen.
        let (f0, c0, t0) = (fotogramas(), cpu(), Instant::now());
        let mut avisos = 0u64;
        bombear(Duration::from_secs(segundos), &mut avisos);
        let dt = t0.elapsed().as_secs_f64();
        let (f1, c1) = (fotogramas(), cpu());
        println!(
            "primer_fotograma_ms={primer_ms} avisos_por_s={:.1} fotogramas_por_s={:.1} cpu_un_nucleo={:.1}% (proceso de prueba entero)",
            avisos as f64 / dt,
            (f1 - f0) as f64 / dt,
            (c1 - c0) / dt * 100.0
        );
        // Lo que daba el temporizador de antes, en la misma ventana.
        // SAFETY: temporizador propio sobre la ventana propia; se mata abajo.
        unsafe {
            SetTimer(Some(hwnd), 77, 16, None);
        }
        let t0 = Instant::now();
        let mut timers = 0u64;
        let mut msg = MSG::default();
        while t0.elapsed() < Duration::from_secs(2) {
            // SAFETY: bucle de mensajes del hilo propio; los WM_TIMER 77 se
            // cuentan y no se despachan.
            unsafe {
                if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    if msg.message == WM_TIMER && msg.wParam.0 == 77 {
                        timers += 1;
                        continue;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                } else {
                    let _ = MsgWaitForMultipleObjects(None, false, 5, QS_ALLINPUT);
                }
            }
        }
        // SAFETY: mata el temporizador de la prueba.
        unsafe {
            let _ = KillTimer(Some(hwnd), 77);
        }
        println!(
            "SetTimer(16) daba {:.1} ticks/s",
            timers as f64 / t0.elapsed().as_secs_f64()
        );
        assert!(f1 > f0, "el video deberia avanzar");
        drop(pin);
    }

    /// Lo que se ve en la pantalla en `r`, en RGBA. Se lee la pantalla y no
    /// la ventana: estas ventanas no tienen superficie GDI que imprimir.
    fn capturar_pantalla(r: Rect) -> ImagenRgba {
        use windows::Win32::Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
            DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC, SRCCOPY, SelectObject,
        };
        let (ancho, alto) = (r.ancho as i32, r.alto as i32);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: ancho,
                biHeight: -alto,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: objetos GDI creados y soltados aqui; `bits` vive hasta el
        // DeleteObject.
        let mut px = unsafe {
            let pantalla = GetDC(None);
            let hdc = CreateCompatibleDC(Some(pantalla));
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let bmp = CreateDIBSection(Some(hdc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                .expect("seccion DIB");
            let viejo = SelectObject(hdc, bmp.into());
            let _ = BitBlt(hdc, 0, 0, ancho, alto, Some(pantalla), r.x, r.y, SRCCOPY);
            let px =
                std::slice::from_raw_parts(bits as *const u8, (ancho * alto * 4) as usize).to_vec();
            SelectObject(hdc, viejo);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(hdc);
            ReleaseDC(None, pantalla);
            px
        };
        for p in px.chunks_exact_mut(4) {
            p.swap(0, 2);
            p[3] = 255;
        }
        ImagenRgba {
            ancho: r.ancho,
            alto: r.alto,
            pixeles: px,
        }
    }

    fn bombear(ms: u64) {
        let fin = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        while std::time::Instant::now() < fin {
            // SAFETY: bombear la cola del hilo de la prueba; `m` es local.
            unsafe {
                let mut m = MSG::default();
                while PeekMessageW(&mut m, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = DispatchMessageW(&m);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }

    /// **La muestra del rediseno v2**: dos pines con la barra a la vista
    /// (con el aviso de Copiar), una guia de alinear, un pin al 70 % y el
    /// panel «Pines abiertos». Deja un PNG en `PIXPIN_CAPTURA_PRUEBA` (o en
    /// el temporal) para mirarlo. No toca el raton ni el teclado: la barra
    /// se ensena llamando a su funcion.
    #[test]
    #[ignore = "necesita GPU y escritorio; --ignored y mirar el PNG"]
    fn muestra_de_la_barra_y_el_panel_v2() {
        use crate::panel_abiertos::{FilaPanel, PanelAbiertos, TipoFila};
        let d3d = d3d();
        let motor = Rc::new(MotorRender::nuevo(&d3d).unwrap());
        let textos = crate::menu::pruebas::textos();
        // Una foto con degradado, para que se note la opacidad del otro.
        let (w, h) = (480u32, 300u32);
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                px.extend_from_slice(&[
                    (47 + x * 46 / w) as u8,
                    (74 + y * 53 / h) as u8,
                    (107 + x * 59 / w) as u8,
                    255,
                ]);
            }
        }
        let foto = Pin::nuevo(
            &d3d,
            Rc::clone(&motor),
            Contenido::Imagen(ImagenRgba {
                ancho: w,
                alto: h,
                pixeles: px,
            }),
            Rect {
                x: 200,
                y: 220,
                ancho: w,
                alto: h,
            },
            100,
            false,
            16,
            Box::new(|_| {}),
        )
        .unwrap();
        foto.poner_textos(textos.clone());
        let nota = Pin::nuevo(
            &d3d,
            Rc::clone(&motor),
            Contenido::Nota {
                texto: "# Compras temu\n- Nivel láser\n- Wincha 40 m".into(),
            },
            Rect {
                x: 204,
                y: 560,
                ancho: 300,
                alto: 160,
            },
            100,
            false,
            16,
            Box::new(|_| {}),
        )
        .unwrap();
        nota.poner_textos(textos.clone());
        nota.poner_opacidad(70);
        bombear(300);

        // La barra de la foto, con el raton «sobre» Copiar.
        let i = interno_de(foto.hwnd()).unwrap();
        assert!(quiere_barra(i), "con textos y a la vista, quiere barra");
        ensenar_barra(i);
        crate::barra_flotante::poner_encima(Some(crate::barra::AccionBarra::Copiar));
        assert!(crate::barra_flotante::es_de(foto.hwnd()));
        // El raton de verdad no esta encima: sin esto la vigilancia la
        // esconderia en el primer tick.
        // SAFETY: temporizador de una ventana propia de la prueba.
        unsafe {
            let _ = KillTimer(Some(foto.hwnd()), ID_TEMPORIZADOR_BARRA);
        }
        // La guia que veria la nota al arrastrarla junto a la foto.
        let n = interno_de(nota.hwnd()).unwrap();
        let a = guias_para(nota.hwnd(), n, n.estado.rect());
        assert_eq!(a.rect.x, 200, "se pegaria a la izquierda de la foto");
        crate::guias::ensenar(&a.guias, 2);

        let panel = PanelAbiertos::nuevo(
            &d3d,
            Rc::clone(&motor),
            Rect {
                x: 760,
                y: 120,
                ancho: 316,
                alto: 640,
            },
            100,
            textos.v2.clone(),
            textos.colores.clone(),
            false,
            Box::new(|_| {}),
        )
        .unwrap();
        let fila = |id, nombre: &str, detalle: &str, tipo, grupo, oculto| FilaPanel {
            id,
            nombre: nombre.into(),
            detalle: detalle.into(),
            tipo,
            grupo,
            oculto,
        };
        panel.poner_filas(
            vec![
                fila(1, "Tesis.pdf", "PDF · 3/48", TipoFila::Pdf, Some(5), false),
                fila(
                    2,
                    "Foto · 2026-10-04",
                    "Foto",
                    TipoFila::Foto,
                    Some(5),
                    false,
                ),
                fila(
                    3,
                    "Tabla de resultados",
                    "Foto",
                    TipoFila::Foto,
                    Some(5),
                    true,
                ),
                fila(
                    4,
                    "Compras temu",
                    "Nota · opacidad 70 %",
                    TipoFila::Nota,
                    Some(1),
                    false,
                ),
                fila(
                    5,
                    "Clase grabada.mp4",
                    "Vídeo",
                    TipoFila::Video,
                    None,
                    false,
                ),
                fila(6, "Pin en vivo", "EN VIVO", TipoFila::Vivo, None, false),
            ],
            false,
        );
        bombear(700);
        // La prueba no declara DPI: Windows la escala, y la pantalla se lee
        // en pixeles de verdad. Se mide cuanto y se lee la zona escalada.
        let k = {
            use windows::Win32::Graphics::Gdi::{
                DESKTOPHORZRES, GetDC, GetDeviceCaps, HORZRES, ReleaseDC,
            };
            // SAFETY: DC de pantalla pedido y soltado aqui.
            unsafe {
                let dc = GetDC(None);
                let k = GetDeviceCaps(Some(dc), DESKTOPHORZRES) as f32
                    / GetDeviceCaps(Some(dc), HORZRES).max(1) as f32;
                ReleaseDC(None, dc);
                k
            }
        };
        let z = |v: i32| (v as f32 * k) as i32;
        let png = pixpin_codec::codificar_png(&capturar_pantalla(Rect {
            x: z(150),
            y: z(120),
            ancho: z(960) as u32,
            alto: z(660) as u32,
        }))
        .unwrap();
        let ruta = std::env::var_os("PIXPIN_CAPTURA_PRUEBA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("pixpin-v2-pines.png"));
        std::fs::write(&ruta, png).unwrap();
        println!("captura en {}", ruta.display());

        crate::guias::esconder();
        esconder_barra(foto.hwnd());
        assert!(
            !crate::barra_flotante::es_de(foto.hwnd()),
            "caso negativo: escondida"
        );
        drop(panel);
        drop(nota);
        drop(foto);
        bombear(100);
    }
}
