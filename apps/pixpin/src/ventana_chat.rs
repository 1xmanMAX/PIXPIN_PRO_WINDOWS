//! La ventana de chat: proyectos a la izquierda, su contenido a la derecha.
//!
//! La disposicion la calcula `pixpin_ui::chat`, que es pura y esta probada;
//! aqui van la ventana, el raton y los colores. Las medidas y los colores
//! salen de `docs/investigacion/2026-09-15-telegram-desktop-estructura.md`
//! (analisis de Telegram Desktop). **Telegram Desktop es GPL-3.0: de ahi
//! solo se toman medidas, colores y tecnicas; ni una linea de su codigo ni
//! sus recursos.**
//!
//! La ventana no usa el marco del sistema (nace sin el, como los overlays):
//! la barra de titulo, los botones y los bordes de redimension son propios,
//! como en Telegram. A cambio, se puede pintar entera con Direct2D y queda
//! igual en tema claro y oscuro.
//!
//! Pasos hechos: las dos columnas, el asa, la barra de titulo (mover,
//! minimizar, maximizar y cerrar), redimensionar por los bordes, recordar
//! donde quedo y la lista de proyectos (avatar, nombre, hojas, hora y
//! contador), que pinta solo las filas que se ven, y el historial del
//! proyecto elegido en burbujas, y escribir notas en la caja de abajo
//! (borrador por proyecto, Entrar envia, Mayusculas+Entrar hace renglon).
//! Soltar ficheros e imagenes llega despues.

use anyhow::{Context, Result};
use pixpin_geom::{Punto, Rect};
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin, VentanaOverlay};
use pixpin_store::{Catalogo, Ubicacion};
use pixpin_ui::chat::{self, Borde, BotonBarra, Disposicion, Vista};
use pixpin_ui::menu;

use pixpin_render::icono::{Icono, material as mi};

use crate::caja_dibujo::hex;
use crate::overlay::Recursos;

/// Tamano con el que nace, en pixeles logicos (el de Telegram en escritorio).
const ANCHO_LOGICO: u32 = 1024;
const ALTO_LOGICO: u32 = 768;
const VK_ESCAPE: u32 = 0x1B;
const VK_RETROCESO: u32 = 0x08;
const VK_ENTRAR: u32 = 0x0D;
const VK_V: u32 = 0x56;
const VK_U: u32 = 0x55;

/// Lo que abre Ctrl+U (D214): la galaxia del proyecto abierto o, sin
/// ninguno, el cosmos entero (lo mismo que el boton de la lista, D210).
fn pedido_de_ctrl_u(abierto: Option<&str>) -> crate::universo::Pedido {
    match abierto {
        Some(id) => crate::universo::Pedido::Galaxia(id.to_string()),
        None => crate::universo::Pedido::Cosmos,
    }
}
/// El lado mayor de la vista previa de una foto, guardada: el doble del ancho
/// de la burbuja al 100 %, para que al 200 % no se vea pastosa.
const PREVIA_LADO: u32 = 2 * pixpin_ui::historial::VISTA_ANCHO;
/// Cuanto crece la burbuja del lienzo que esta vivo. En el tamano de la
/// ojeada (260x180) no se puede dibujar; al doble, si.
const VIVO_FACTOR: f32 = 2.0;

/// Los colores de un tema.
struct Tema {
    barra: Color,
    boton_sobre: Color,
    cerrar_sobre: Color,
    lista: Color,
    chat: Color,
    cabecera: Color,
    separador: Color,
    texto: Color,
    apagado: Color,
    /// La fila bajo el raton y la fila elegida.
    fila_sobre: Color,
    fila_elegida: Color,
    /// El texto de la fila elegida, que va sobre color fuerte.
    texto_elegido: Color,
    /// La pildora de pendientes.
    contador: Color,
    /// El fondo del buscador de la cabecera.
    buscador: Color,
    /// El azul del boton de enviar (`historySendIconFg`).
    enviar: Color,
    /// El fondo sobre el que se ensena un lienzo: una hoja de papel.
    papel: Color,
    /// Lo que se escribe ENCIMA del papel. No vale `texto`: en el tema
    /// oscuro ese es casi blanco, y sobre una hoja clara no se veria.
    texto_papel: Color,
    /// La pildora de la pestana activa del panel de informacion, y su texto
    /// (`lightButtonBgOver` y `lightButtonFg` de Telegram).
    pestana_activa: Color,
    texto_pestana_activa: Color,
    /// El velo de detras de una capa (`layerBg`): negro a la mitad.
    velo: Color,
    /// Las burbujas del historial, su texto, su hora y la pildora que
    /// separa los dias.
    burbuja_mia: Color,
    burbuja_otra: Color,
    texto_mio: Color,
    hora_mia: Color,
    separador_dia: Color,
    texto_separador: Color,
    /// Lo que sigue es del chat del movil, medido en sus capturas (tema de
    /// noche del usuario, 2026-09-18): el boton redondo de la fila y su
    /// icono (`primaryContainer` y `onPrimaryContainer`), que es tambien el
    /// color de la tarjeta «Viene de».
    circulo: Color,
    circulo_icono: Color,
    viene_etiqueta: Color,
    /// Las pastillas de la cabecera: fondo, filete, texto y subtitulo.
    pildora: Color,
    pildora_borde: Color,
    pildora_texto: Color,
    pildora_sub: Color,
    /// El disco de la carpeta del titulo y su icono.
    carpeta: Color,
    carpeta_icono: Color,
    /// La isla de escribir, su filete, el campo y lo apagado de dentro.
    isla: Color,
    isla_borde: Color,
    campo: Color,
    campo_apagado: Color,
    /// Lo que va encima de la burbuja sin ser texto: la chapa del codigo
    /// (`ColoresDelChat.filete`).
    filete: Color,
}

/// Los papeles del chat entre los que elegir (`FondosDelChat.TODOS` del
/// movil): los dos extremos de dia y los dos de noche, y su nombre.
const FONDOS: [(u32, u32, u32, u32, &str); 6] = [
    (0x86bfb2, 0x8ab8d2, 0x151e27, 0x10161d, "chat-fondo-mar"),
    (0xcdb893, 0xc9a98b, 0x241e16, 0x1b1711, "chat-fondo-arena"),
    (0xb3a7ce, 0xa9a2c9, 0x1e1a2a, 0x171422, "chat-fondo-brezo"),
    (0x93be93, 0x8fb9a4, 0x14201a, 0x101913, "chat-fondo-bosque"),
    (0xd3a79b, 0xc79fa8, 0x251a19, 0x1d1413, "chat-fondo-ocaso"),
    (0xaab4bb, 0xa3adb6, 0x1a1d20, 0x141719, "chat-fondo-pizarra"),
];

/// Con transparencia: `hex` no la lleva.
const fn con_alfa(c: Color, a: f32) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a,
    }
}

const CLARO: Tema = Tema {
    // titleBg, titleButtonBgOver y titleButtonCloseBgOver del tema claro
    // por omision de Telegram («day-blue»).
    barra: hex(0xf1f1f1),
    boton_sobre: hex(0xe5e5e5),
    cerrar_sobre: hex(0xe81123),
    lista: hex(0xffffff),
    chat: hex(0x86bfb2),
    cabecera: hex(0xffffff),
    separador: hex(0xe0e0e0),
    // dialogsNameFg, dialogsTextFg, dialogsBgActive y dialogsUnreadBg.
    texto: hex(0x222222),
    apagado: hex(0x999999),
    fila_sobre: hex(0xf1f1f1),
    fila_elegida: hex(0x419fd9),
    texto_elegido: hex(0xffffff),
    contador: hex(0x40a7e3),
    // filterInputInactiveBg.
    buscador: hex(0xf1f1f1),
    enviar: hex(0x40a7e3),
    papel: hex(0xffffff),
    texto_papel: hex(0x111111),
    pestana_activa: hex(0xe3f1fa),
    texto_pestana_activa: hex(0x168acd),
    velo: Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.498,
    },
    // msgOutBg, msgInBg y sus colores de texto y hora.
    // Los del chat de PixPin Android (`guardados/ColoresDelChat.kt`), que a
    // su vez saca de Telegram y ajusta el contraste: la burbuja `#EFFFDE`,
    // su tinta `#101B24` y su hora `#3F7A30` —un verde mas hondo que el de
    // Telegram, que en letra pequena se quedaba en 2,46:1—.
    burbuja_mia: hex(0xefffde),
    burbuja_otra: hex(0xffffff),
    texto_mio: hex(0x101b24),
    hora_mia: hex(0x3f7a30),
    separador_dia: hex(0xe1e3f3),
    texto_separador: hex(0x42465e),
    // De dia no hay captura: los mismos papeles con los tonos claros del
    // mismo tema (los contenedores claros de Material).
    circulo: hex(0xffdea6),
    circulo_icono: hex(0x3e2d00),
    viene_etiqueta: hex(0x6b5a2e),
    pildora: hex(0xffffff),
    pildora_borde: con_alfa(hex(0x101b24), 0.10),
    pildora_texto: hex(0x1b1b21),
    pildora_sub: hex(0x5d5f71),
    carpeta: hex(0x7a5900),
    carpeta_icono: hex(0xffffff),
    isla: hex(0xffffff),
    isla_borde: con_alfa(hex(0x101b24), 0.10),
    campo: hex(0xeceef8),
    campo_apagado: hex(0x5d5f71),
    filete: con_alfa(hex(0x101b24), 0.10),
};

const OSCURO: Tema = Tema {
    // titleBgActive, titleButtonBgOver y titleButtonCloseBgOver del tema
    // «night» de Telegram.
    barra: hex(0x242f3d),
    boton_sobre: hex(0x2c3847),
    cerrar_sobre: hex(0xe92539),
    lista: hex(0x17212b),
    chat: hex(0x151e27),
    cabecera: hex(0x17212b),
    separador: hex(0x101921),
    // dialogsNameFg, dialogsTextFg y dialogsUnreadBg.
    texto: hex(0xf5f5f5),
    apagado: hex(0x7f91a4),
    fila_sobre: hex(0x202b36),
    fila_elegida: hex(0x2b5278),
    texto_elegido: hex(0xffffff),
    contador: hex(0x4082bc),
    // filterInputInactiveBg.
    buscador: hex(0x242f3d),
    enviar: hex(0x5288c1),
    // En oscuro tampoco se pinta negro sobre negro: el lienzo lleva su
    // hoja clara, solo un poco apagada para no deslumbrar.
    papel: hex(0xe8e8e8),
    texto_papel: hex(0x111111),
    pestana_activa: hex(0x1d2a39),
    texto_pestana_activa: hex(0x6ab2f2),
    velo: Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.498,
    },
    // msgOutBg, msgInBg y sus colores de texto y hora.
    // Los mismos, de noche: burbuja `#3E618A`, tinta `#FAFAFA` y hora
    // `#A8CCE8`.
    burbuja_mia: hex(0x3e618a),
    burbuja_otra: hex(0x243447),
    texto_mio: hex(0xfafafa),
    hora_mia: hex(0xa8cce8),
    // La pastilla del dia del movil, medida en su captura: fondo `#242B55`
    // y letra lavanda.
    separador_dia: hex(0x242b55),
    texto_separador: hex(0xc5c9ea),
    circulo: hex(0x493a11),
    circulo_icono: hex(0xfee8b7),
    viene_etiqueta: hex(0xd0be90),
    pildora: hex(0x161b31),
    pildora_borde: hex(0x44495c),
    pildora_texto: hex(0xe8eaf6),
    pildora_sub: hex(0xb4b9d7),
    carpeta: hex(0xffd179),
    carpeta_icono: hex(0x2b1d00),
    isla: hex(0x14192d),
    isla_borde: hex(0x3a4060),
    campo: hex(0x242b55),
    campo_apagado: hex(0xb4b9d7),
    // Blanco al 20 %: sobre la burbuja `#3E618A` da el `#6481A1` medido.
    filete: con_alfa(hex(0xffffff), 0.20),
};

fn rf(r: Rect) -> RectF {
    RectF {
        x: r.x as f32,
        y: r.y as f32,
        ancho: r.ancho as f32,
        alto: r.alto as f32,
    }
}

/// El mismo rectangulo, metido `cuanto` hacia dentro por los cuatro lados.
/// Nunca al reves: encogerlo mas de lo que mide lo deja en nada, no en un
/// rectangulo del reves.
fn encoger(r: RectF, cuanto: f32) -> RectF {
    RectF {
        x: r.x + cuanto,
        y: r.y + cuanto,
        ancho: (r.ancho - 2.0 * cuanto).max(0.0),
        alto: (r.alto - 2.0 * cuanto).max(0.0),
    }
}

fn cursor_de(borde: Borde) -> FormaCursorWin {
    match borde {
        Borde::Izquierda | Borde::Derecha => FormaCursorWin::RedimEO,
        Borde::Arriba | Borde::Abajo => FormaCursorWin::RedimNS,
        Borde::ArribaIzquierda | Borde::AbajoDerecha => FormaCursorWin::RedimNoSe,
        Borde::ArribaDerecha | Borde::AbajoIzquierda => FormaCursorWin::RedimNeSo,
    }
}

/// Lo que se esta arrastrando ahora mismo.
enum Arrastre {
    /// El asa entre columnas; se guarda por donde se agarro.
    Asa(i32),
    /// La barra de titulo; se guarda el punto de agarre dentro de la ventana.
    Ventana(Punto),
    Borde(Borde),
}

/// Abre la ventana y no vuelve hasta que se cierra.
/// La ventana de chat abierta, si la hay. Es un `HWND` como entero para poder
/// vivir en un estatico que miran dos hilos.
static ABIERTA: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Abre la ventana de chat en SU PROPIO HILO, o trae al frente la que ya
/// este abierta (D141).
///
/// En su propio hilo porque es la interfaz principal y pasa horas abierta.
/// Cuando corria en el hilo principal, su bucle se quedaba con todos los
/// mensajes: los atajos globales y los gestos con Alt se encolaban y nadie
/// los atendia hasta cerrarla, y el gancho de raton se tragaba el clic, asi
/// que Alt + arrastrar no hacia nada. Con un hilo aparte el principal sigue
/// bombeando. Todo lo que la ventana necesita nace dentro del hilo: su
/// dispositivo de dibujo, sus textos y su apartamento COM (los dialogos de
/// abrir ficheros lo exigen en STA).
pub fn lanzar(idioma: pixpin_store::Idioma, ubicacion: Ubicacion, lienzo: OpcionesLienzo) {
    // El idioma viaja hasta dentro: el chat lo necesita para abrir otras
    // ventanas suyas (traer del movil) en sus propios hilos.

    use std::sync::atomic::Ordering;
    let ya = ABIERTA.load(Ordering::SeqCst);
    if ya > 0 {
        VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(ya as *mut _));
        return;
    }
    if ya < 0 {
        // Naciendo todavia: la ventana aparecera sola.
        return;
    }
    // Se marca antes de que exista la ventana: dos peticiones seguidas no
    // pueden abrir dos chats. El hilo pone el HWND de verdad al crearla.
    ABIERTA.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new()
        .name("chat".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            let hecho =
                Recursos::nuevos().and_then(|r| abrir(&r, &textos, &ubicacion, lienzo, idioma));
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir el chat de proyectos");
            }
            ABIERTA.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del chat");
        ABIERTA.store(0, Ordering::SeqCst);
    }
}

/// Adonde hay que llevar el chat: un proyecto y, si lo hay, un mensaje por
/// su codigo unico. Lo deja el universo (Ctrl+clic en una galaxia o en una
/// nota) y lo recoge el hilo del chat al despertar o al nacer.
static IR_A: std::sync::Mutex<Option<(String, Option<String>)>> = std::sync::Mutex::new(None);

/// Lo que dura resaltado el mensaje al que se llega desde el universo.
const RESALTE: std::time::Duration = std::time::Duration::from_millis(1500);

/// Lleva el chat a un proyecto y, si se da, a un mensaje suyo. Abre el chat
/// si estaba cerrado; si no, lo trae al frente y lo despierta.
///
/// Solo cabe un destino: si se piden dos seguidos gana el ultimo, que es el
/// que el usuario acaba de pulsar.
pub(crate) fn ir_a(
    idioma: pixpin_store::Idioma,
    ubicacion: Ubicacion,
    opciones: OpcionesLienzo,
    proyecto: String,
    codigo: Option<String>,
) {
    if let Ok(mut g) = IR_A.lock() {
        *g = Some((proyecto, codigo));
    }
    let ya = ABIERTA.load(std::sync::atomic::Ordering::SeqCst);
    if ya > 0 {
        VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(ya as *mut _));
        pixpin_shell::overlay::despertar(ya);
    } else {
        // Cerrado o naciendo: el bucle lo recoge en su primera vuelta.
        lanzar(idioma, ubicacion, opciones);
    }
}

fn tomar_ir_a() -> Option<(String, Option<String>)> {
    IR_A.lock().ok().and_then(|mut g| g.take())
}

/// Alguien cambio por fuera lo que el chat tiene a la vista: hay que releer.
///
/// Lo pone la sincronizacion con el movil, que escribe mensajes, proyectos y
/// ficheros DEBAJO de la ventana abierta. Es un booleano y no una lista de lo
/// que cambio a proposito: releer el cuaderno de UN proyecto cuesta leer un
/// fichero, y afinar mas obligaria a que quien avisa supiera que hay abierto.
static REFRESCAR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Avisa al chat de que lo guardado cambio por fuera (una sincronizacion).
///
/// No hace falta que la ventana este abierta: si no lo esta, no hay nada que
/// releer y la marca se queda puesta sin molestar a nadie. Si lo esta, se la
/// despierta para que lo haga en su siguiente vuelta y no dentro de un rato.
/// La llaman los dos sitios por los que entra lo del movil: la vuelta de
/// `sincronizar.rs` (de los dos lados: la que se pide aqui y la que pide el
/// otro aparato) y el `.pixpin` que se recibe en `recibir.rs`.
pub(crate) fn refrescar() {
    REFRESCAR.store(true, std::sync::atomic::Ordering::SeqCst);
    let ya = ABIERTA.load(std::sync::atomic::Ordering::SeqCst);
    if ya > 0 {
        pixpin_shell::overlay::despertar(ya);
    }
}

fn tomar_refrescar() -> bool {
    REFRESCAR.swap(false, std::sync::atomic::Ordering::SeqCst)
}

/// Relee el cuaderno del proyecto abierto SIN perder lo que el usuario tenga
/// a medias: el borrador escrito, la seleccion, a quien contesta, la
/// busqueda, el panel y la posicion del historial.
///
/// Lo que va por POSICION (la hoja abierta, el lienzo vivo, a donde se iba,
/// lo resaltado, el nombre que se estaba escribiendo) se vuelve a buscar por
/// el `id` del mensaje: una sincronizacion puede meter mensajes en medio, y
/// quedarse con el numero de antes apuntaria a otro.
///
/// **Con una hoja o un lienzo vivo abiertos no relee**: lo que el usuario
/// esta escribiendo manda, y pisarlo con lo que acaba de llegar seria perder
/// trabajo suyo. Devuelve `false` y quien llama vuelve a intentarlo cuando
/// se cierren.
fn releer_lo_abierto(ubicacion: &Ubicacion, a: &mut Abierto) -> bool {
    if a.hoja.is_some() || a.mini.is_some() || a.vivo.is_some() {
        return false;
    }
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    let Ok(cuaderno) = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta) else {
        return true;
    };
    // Quien iba por posicion se apunta por id ANTES de cambiar la lista.
    let id_de = |i: Option<usize>| i.and_then(|i| a.mensajes.get(i)).map(|m| m.id.clone());
    let id_ir_a = id_de(a.ir_a);
    let id_resaltado = id_de(a.resaltado.map(|(i, _)| i));
    let renombrando = a
        .renombrando_mensaje
        .as_ref()
        .and_then(|(i, escrito)| Some((a.mensajes.get(*i)?.id.clone(), escrito.clone())));

    let mut mensajes = cuaderno.mensajes;
    mensajes.sort_by_key(|m| m.cuando);
    a.vistas = mensajes
        .iter()
        .map(|m| leer_vista(ubicacion, &a.ficha.id, m))
        .collect();
    a.fijado = mensajes.iter().rposition(|m| m.fijado);
    a.rotas = cuaderno.lineas_rotas;
    a.mensajes = mensajes;

    let donde = |id: &str| a.mensajes.iter().position(|m| m.id == id);
    a.ir_a = id_ir_a.as_deref().and_then(donde);
    a.resaltado = id_resaltado
        .as_deref()
        .and_then(donde)
        .map(|i| (i, std::time::Instant::now()));
    a.renombrando_mensaje = renombrando.and_then(|(id, escrito)| Some((donde(&id)?, escrito)));
    // Lo marcado y a quien se contesta van por id desde siempre: lo unico que
    // hay que hacer es soltar lo que ya no existe, que si no la barra de
    // arriba contaria mensajes borrados.
    a.marcados.retain(|id| donde(id).is_some());
    if a.respondiendo
        .as_deref()
        .is_some_and(|id| donde(id).is_none())
    {
        a.respondiendo = None;
    }
    // Y a medir de nuevo: el historial tiene otras burbujas.
    a.colocado.borrow_mut().ancho = 0;
    true
}

/// Donde esta un mensaje por su codigo unico, que es lo que el universo
/// sabe de el.
pub(crate) fn indice_de_codigo(
    mensajes: &[pixpin_proyecto::cuaderno::Mensaje],
    codigo: &str,
) -> Option<usize> {
    mensajes.iter().position(|m| m.codigo_unico() == codigo)
}

pub fn abrir(
    recursos: &Recursos,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    lienzo: OpcionesLienzo,
    idioma: pixpin_store::Idioma,
) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .principal()
        .context("sin monitor principal")?
        .to_owned();
    let escala = monitor.escala_por_cien;
    let e = |v: u32| v * escala / 100;

    // Donde quedo la ultima vez, si cabe en algun monitor; si no, centrada.
    let mut estado = pixpin_store::estado::cargar(ubicacion);
    let marco_inicial = estado
        .chat_ventana
        .map(|[x, y, w, h]| Rect {
            x,
            y,
            ancho: w.max(chat::ANCHO_MINIMO_VENTANA as i32) as u32,
            alto: h.max(chat::ALTO_MINIMO_VENTANA as i32) as u32,
        })
        .filter(|r| {
            monitores
                .monitores()
                .iter()
                .any(|m| m.area.interseccion(*r).is_some())
        });
    let mut marco = marco_inicial.unwrap_or_else(|| {
        let (ancho, alto) = (e(ANCHO_LOGICO), e(ALTO_LOGICO));
        Rect {
            x: monitor.area.x + (monitor.area.ancho as i32 - ancho as i32) / 2,
            y: monitor.area.y + (monitor.area.alto as i32 - alto as i32) / 2,
            ancho,
            alto,
        }
    });

    // Una ventana normal (D141): se tapa al cambiar de programa y tiene su
    // boton en la barra de tareas, como cualquier aplicacion.
    let mut ventana = VentanaOverlay::nueva_normal(marco, &textos.t("app-nombre"))
        .context("no se pudo abrir la ventana de chat")?;
    ABIERTA.store(
        ventana.handle().0 as isize,
        std::sync::atomic::Ordering::SeqCst,
    );
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para el chat")?;
    ventana.mostrar();
    ventana.enfocar();
    // Se pueden soltar ficheros encima para meterlos en el proyecto.
    ventana.aceptar_ficheros(true);

    let tema = if pixpin_shell::entorno::tema_claro() {
        &CLARO
    } else {
        &OSCURO
    };
    // Los proyectos, ya ordenados como se ensenan. La lista se lee entera
    // una vez: es un indice pequeno, y lo caro (abrir cada `.pixpin`) no se
    // hace hasta que se elige uno.
    // El codigo de este equipo va en cada nota que se escriba aqui: es lo
    // que hace que el movil sepa de donde vino.
    let identidad = pixpin_proyecto::identidad::Identidad::leer_o_crear(ubicacion.raiz(), "PC")
        .map(|i| i.yo.codigo())
        .unwrap_or_default();
    let ahora = pixpin_shell::entorno::ahora_local_ms();
    // «Mensajes guardados» existe siempre, tambien en un almacen recien
    // estrenado: es adonde va lo suelto, y el usuario lo busco y no estaba.
    if let Err(e) = pixpin_proyecto::almacen::asegurar_guardados(
        ubicacion.raiz(),
        pixpin_shell::entorno::ahora_utc_ms(),
        &identidad,
    ) {
        tracing::warn!(?e, "no se pudo crear «Mensajes guardados»");
    }
    let indice = pixpin_proyecto::almacen::Indice::leer(ubicacion.raiz());
    let mut fichas: Vec<pixpin_proyecto::almacen::Ficha> =
        indice.ordenadas().into_iter().cloned().collect();
    // Lo escrito y sin enviar de cada proyecto, para que cambiar de
    // conversacion y volver no se lo lleve por delante.
    let mut borradores: std::collections::HashMap<String, String> = Default::default();
    // Lo escrito en el buscador de la lista.
    let mut busqueda = String::new();
    let mut buscando = false;
    // Que fichas se ensenan y en que orden. Con el buscador vacio son
    // todas; al escribir, solo las que coinciden. `elegida` sigue siendo un
    // indice de `fichas`, para que filtrar no cambie de proyecto abierto.
    let mut orden: Vec<usize> = (0..fichas.len()).collect();

    let mut ancho_lista = chat::ancho_inicial(marco.ancho, escala);
    let mut arrastre: Option<Arrastre> = None;
    let mut sobre: Option<BotonBarra> = None;
    let mut scroll: i32 = 0;
    let mut fila_sobre: Option<usize> = None;
    let mut elegida: Option<usize> = None;
    // Los proyectos marcados con Ctrl+clic, por `id`: por indice se
    // descolocarian al filtrar o al releer la lista.
    let mut marcados: std::collections::BTreeSet<String> = Default::default();
    let mut abierto: Option<Abierto> = None;
    // El menu desplegado, si hay uno: el del clip, el de los tres puntos o el
    // de un mensaje. Vive fuera de `abierto` porque cambiar de conversacion
    // tiene que cerrarlo.
    let mut menu: Option<MenuAbierto> = None;
    // El aviso de abajo y cuando salio, como el `Toast` del movil.
    let mut aviso: Option<(String, std::time::Instant)> = None;
    // El papel del chat, el que se eligio la ultima vez.
    let mut fondo = fondo_guardado(ubicacion);
    let claro = pixpin_shell::entorno::tema_claro();
    // Las fotos ya leidas y reducidas, para el panel de informacion. Sobrevive
    // a cambiar de proyecto a proposito: van por ruta, y volver al anterior no
    // tiene por que releerlas del disco.
    let mut miniaturas = crate::miniaturas::Miniaturas::nuevo();
    // Las de las burbujas, mas grandes: la vista previa mide 260 px.
    let mut previas = crate::miniaturas::Miniaturas::con_lado(PREVIA_LADO);
    // Los ficheros que esperan un si o un no. Meter algo en el cuaderno no se
    // deshace, asi que se pregunta antes.
    let mut pendientes: Option<Pendientes> = None;
    // Antes de maximizar, para poder volver.
    let mut antes_de_maximizar: Option<Rect> = None;
    let mut hay_que_pintar = true;
    // Los PDF que hayan entrado en esta vuelta: cada uno abre un proyecto
    // propio, y eso lo decide el bucle, que es quien lleva la lista.
    let mut pdfs_pendientes: Vec<std::path::PathBuf> = Vec::new();
    // La fecha del indice la ultima vez que se leyo, para enterarse de lo
    // que llegue por fuera (del movil, de otra ventana).
    let mut sello_indice: Option<std::time::SystemTime> = None;
    // Hay que releer el cuaderno del proyecto abierto, pero puede que todavia
    // no se pueda (ver `releer_lo_abierto`): se guarda la deuda en vez de
    // perderla, y se salda en cuanto la hoja o el lienzo se cierren.
    let mut pendiente_de_releer = false;

    loop {
        pixpin_shell::overlay::bombear_pendientes();
        let mut cerrar = false;
        let disposicion = disponer(marco, escala, ancho_lista, abierto.as_ref());
        let mut nuevo_marco: Option<Rect> = None;

        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            // Los eventos llegan en coordenadas del escritorio; la ventana
            // trabaja en las suyas.
            let local = |p: Punto| Punto {
                x: p.x - marco.x,
                y: p.y - marco.y,
            };
            match evento {
                EventoOverlay::BotonPulsado(p) => {
                    let l = local(p);
                    // El lienzo vivo manda dentro de su burbuja: ahi el clic
                    // es un trazo, no una pulsacion de la conversacion.
                    let en_el_lienzo = abierto
                        .as_mut()
                        .and_then(|a| a.vivo.as_mut())
                        .and_then(|v| {
                            let q = v.punto(l)?;
                            v.gesto.evento(
                                pixpin_motor2d::gesto::EventoGesto::Pulsar {
                                    p: q,
                                    shift: false,
                                    alt: false,
                                    presion: None,
                                },
                                &mut v.escena,
                                1.0,
                            );
                            v.tocado = true;
                            Some(())
                        })
                        .is_some();
                    if en_el_lienzo {
                        hay_que_pintar = true;
                        continue;
                    }
                    // Fuera de su burbuja, el lienzo se apaga y se guarda; el
                    // clic sigue su camino normal (puede encender otro).
                    if let Some(a) = abierto.as_mut().filter(|a| a.vivo.is_some()) {
                        apagar_lienzo(ubicacion, a);
                        a.colocado.borrow_mut().ancho = 0;
                        hay_que_pintar = true;
                    }
                    // Los bordes primero: son unos pocos pixeles y si otra
                    // cosa se los quedara no se podria redimensionar.
                    // El cuadro de confirmar es modal: mientras esta, se lleva
                    // el clic entero. Ni siquiera los bordes, porque
                    // redimensionar la ventana lo movería debajo del raton.
                    if let Some(p) = pendientes.as_ref() {
                        let d = pixpin_ui::confirmar::colocar(
                            Rect {
                                x: 0,
                                y: 0,
                                ancho: marco.ancho,
                                alto: marco.alto,
                            },
                            p.rutas.len(),
                            escala,
                        );
                        match d.boton_en(l) {
                            Some(pixpin_ui::confirmar::Boton::Aceptar) => {
                                if let (Some(p), Some(a)) = (pendientes.take(), abierto.as_mut()) {
                                    let (hechos, suyos) =
                                        meter_ficheros(ubicacion, a, &identidad, &p.rutas, &p.pie);
                                    pdfs_pendientes = suyos;
                                    if hechos > 0 {
                                        a.scroll = None;
                                        if let Some(i) = elegida {
                                            fichas[i].tocado = a.ficha.tocado;
                                            fichas[i].resumen = a.ficha.resumen.clone();
                                        }
                                    }
                                    a.colocado.borrow_mut().ancho = 0;
                                }
                            }
                            Some(pixpin_ui::confirmar::Boton::Cancelar) => {
                                pendientes = None;
                            }
                            // Pulsar fuera del cuadro cancela, como el aspa de
                            // cualquier dialogo; dentro, no hace nada.
                            None if !d.caja.contiene(l) => pendientes = None,
                            None => {}
                        }
                        hay_que_pintar = true;
                    } else if let Some(b) = chat::borde_en(l, marco.ancho, marco.alto, escala) {
                        ventana.capturar_raton();
                        arrastre = Some(Arrastre::Borde(b));
                    } else if let Some(boton) = disposicion.boton_barra_en(l, escala) {
                        match boton {
                            BotonBarra::Minimizar => ventana.minimizar(),
                            BotonBarra::Maximizar => {
                                let area = monitor.area_trabajo;
                                nuevo_marco = Some(match antes_de_maximizar.take() {
                                    Some(vuelta) => vuelta,
                                    None => {
                                        antes_de_maximizar = Some(marco);
                                        area
                                    }
                                });
                            }
                            BotonBarra::Cerrar => cerrar = true,
                        }
                    } else if disposicion.arrastra_ventana(l, escala) {
                        ventana.capturar_raton();
                        arrastre = Some(Arrastre::Ventana(l));
                    } else if disposicion.asa.contiene(l) {
                        ventana.capturar_raton();
                        arrastre = Some(Arrastre::Asa(l.x - ancho_lista as i32));
                    } else if let Some(m) = menu.take() {
                        // Con un menu desplegado, el primer clic es suyo: o
                        // elige una entrada o lo cierra. Que ademas hiciera
                        // lo que hubiera debajo seria dispararle al usuario
                        // por querer salir del menu.
                        let accion = m
                            .colocado(marco, escala)
                            .fila_en(l, escala)
                            .and_then(|n| m.entradas.get(n))
                            .map(|n| n.accion.clone());
                        match accion {
                            None => {}
                            Some(Accion::AdjArchivo) => {
                                pendientes = Pendientes::de(pixpin_shell::elegir::pedir_ficheros(
                                    ventana.handle(),
                                ));
                            }
                            Some(Accion::AdjImagen) => {
                                pendientes = Pendientes::de(pixpin_shell::elegir::pedir_imagenes(
                                    ventana.handle(),
                                ));
                            }
                            // Traer del movil abre su propio cartel, en otro
                            // hilo: mientras se espera al movil, el chat
                            // sigue usandose.
                            Some(Accion::AdjDelMovil) => {
                                crate::recibir::lanzar(idioma, ubicacion.clone());
                            }
                            // Un lienzo o una tabla nacen vacios: no hay nada
                            // que ensenar en un cuadro de confirmar.
                            Some(n @ (Accion::AdjLienzo | Accion::AdjTabla)) => {
                                if let Some(a) = abierto.as_mut() {
                                    let hecho = if n == Accion::AdjTabla {
                                        crear_tabla(ubicacion, a, &identidad, textos)
                                    } else {
                                        crear_lienzo(ubicacion, a, &identidad)
                                    };
                                    match hecho {
                                        Ok(()) => {
                                            a.scroll = None;
                                            if let Some(i) = elegida {
                                                fichas[i].tocado = a.ficha.tocado;
                                                fichas[i].resumen = a.ficha.resumen.clone();
                                            }
                                        }
                                        Err(e) => tracing::warn!(?e, "no se pudo crear"),
                                    }
                                    a.colocado.borrow_mut().ancho = 0;
                                }
                            }
                            Some(Accion::Fondos) => {
                                menu = Some(MenuAbierto::nuevo(
                                    m.ancla,
                                    menu_de_fondos(textos, fondo),
                                ));
                            }
                            Some(Accion::Fondo(n)) => {
                                fondo = n;
                                guardar_fondo(ubicacion, n);
                            }
                            // En Windows los proyectos son la lista de la
                            // izquierda: se lleva ahi, al buscador.
                            Some(Accion::Proyectos) => {
                                buscando = true;
                                aviso = Some((
                                    textos.t("chat-proyectos-lista"),
                                    std::time::Instant::now(),
                                ));
                            }
                            Some(otra) => {
                                if let Some(a) = abierto.as_mut() {
                                    let cx = Contexto {
                                        ubicacion,
                                        identidad: &identidad,
                                        textos,
                                        lienzo,
                                        ventana: &ventana,
                                        fichas: &fichas,
                                        idioma,
                                    };
                                    match ejecutar(otra, a, &cx) {
                                        Efecto::Nada | Efecto::Cambio => {}
                                        Efecto::Aviso(t) => {
                                            aviso = Some((t, std::time::Instant::now()))
                                        }
                                        Efecto::Menu(v) => {
                                            menu = Some(MenuAbierto::nuevo(m.ancla, v))
                                        }
                                    }
                                    a.colocado.borrow_mut().ancho = 0;
                                }
                            }
                        }
                        hay_que_pintar = true;
                    } else if let Some((r, zona)) = abierto.as_ref().and_then(|a| {
                        a.zonas
                            .borrow()
                            .iter()
                            .rev()
                            .find(|(r, _)| r.contiene(l))
                            .copied()
                    }) {
                        // Lo que se pulsa dentro de la conversacion: la
                        // cabecera, las pastillas de las burbujas, la tarjeta
                        // «Viene de», las citas y la barra de la seleccion.
                        let mut volver = false;
                        if let Some(a) = abierto.as_mut() {
                            let cx = Contexto {
                                ubicacion,
                                identidad: &identidad,
                                textos,
                                lienzo,
                                ventana: &ventana,
                                fichas: &fichas,
                                idioma,
                            };
                            let efecto = match zona {
                                Zona::Volver => {
                                    volver = true;
                                    Efecto::Nada
                                }
                                // El titulo abre lo que guarda el proyecto por
                                // secciones: en el movil son las fichas que
                                // se despliegan al tocarlo.
                                Zona::Titulo => {
                                    a.info = Some(SECCIONES[0]);
                                    a.scroll_info = 0;
                                    Efecto::Nada
                                }
                                // La lupa busca DENTRO de la conversacion y
                                // filtra las burbujas, como en el movil. El
                                // mismo boton la enciende y la apaga; al
                                // apagarla se limpia, que dejar una busqueda
                                // escondida haria parecer el chat vacio.
                                Zona::Buscar => {
                                    a.busqueda = match a.busqueda {
                                        None => Some(String::new()),
                                        Some(_) => None,
                                    };
                                    // Y con la lupa se va el chip de
                                    // etiqueta: sin la fila a la vista, un
                                    // filtro de emoji puesto dejaria la
                                    // conversacion medio vacia sin decir por
                                    // que (el movil se lo deja puesto).
                                    a.por_etiqueta = None;
                                    a.scroll = None;
                                    Efecto::Nada
                                }
                                Zona::EnContexto => {
                                    // Se quita el filtro y se salta al primer
                                    // resultado: quien buscaba no se queda sin
                                    // saber donde estaba lo que encontro.
                                    let primero = indices_visibles(a).first().copied();
                                    a.busqueda = None;
                                    a.por_etiqueta = None;
                                    if let Some(j) = primero {
                                        a.ir_a = Some(j);
                                        a.resaltado = Some((j, std::time::Instant::now()));
                                    }
                                    Efecto::Nada
                                }
                                Zona::Menu => Efecto::Menu(menu_de_cabecera(
                                    textos,
                                    a.ficha.es_guardados(),
                                    &a.ficha.id,
                                )),
                                Zona::Universo => ejecutar(
                                    Accion::Universo(crate::universo::Pedido::Galaxia(
                                        a.ficha.id.clone(),
                                    )),
                                    a,
                                    &cx,
                                ),
                                // El chip filtra por su emoji; pulsado otra
                                // vez, lo quita. Es un interruptor y no una
                                // eleccion de uno entre varios: con solo el
                                // chip no habria por donde volver a verlo
                                // todo sin apagar la lupa entera.
                                Zona::Chip(i) => {
                                    let cual = chips_de(a).get(i).cloned();
                                    a.por_etiqueta = match (&a.por_etiqueta, cual) {
                                        (Some(puesto), Some(nuevo)) if *puesto == nuevo => None,
                                        (_, cual) => cual,
                                    };
                                    // Arriba del todo: lo que se ve ahora es
                                    // otra conversacion, y dejar el scroll de
                                    // antes la deja por la mitad.
                                    a.scroll = None;
                                    Efecto::Nada
                                }
                                Zona::Abrir(i) => ejecutar(Accion::Pinear(i), a, &cx),
                                Zona::VieneDe(i) => {
                                    abrir_el_origen(ubicacion, a, i, lienzo, textos)
                                }
                                Zona::Ir(j) => ejecutar(Accion::Ir(j), a, &cx),
                                Zona::CerrarRespuesta => {
                                    a.respondiendo = None;
                                    Efecto::Nada
                                }
                                Zona::SelCerrar => {
                                    a.marcados.clear();
                                    Efecto::Nada
                                }
                                Zona::SelCopiar => copiar_marcados(a, textos),
                                Zona::SelFijar => fijar_marcados(ubicacion, a, textos),
                                Zona::SelReenviar => {
                                    ejecutar(Accion::Reenviar(indices_marcados(a)), a, &cx)
                                }
                                Zona::SelBorrar => {
                                    let hecho =
                                        ejecutar(Accion::Borrar(indices_marcados(a)), a, &cx);
                                    // Si se dijo que no, la seleccion sigue.
                                    if matches!(hecho, Efecto::Cambio) {
                                        a.marcados.clear();
                                    }
                                    hecho
                                }
                            };
                            match efecto {
                                Efecto::Nada | Efecto::Cambio => {}
                                Efecto::Aviso(t) => aviso = Some((t, std::time::Instant::now())),
                                // Los menus de la cabecera cuelgan de su boton,
                                // alineados por la derecha.
                                Efecto::Menu(v) => {
                                    menu = Some(MenuAbierto::nuevo(
                                        Punto {
                                            x: r.derecha(),
                                            y: r.abajo(),
                                        },
                                        v,
                                    ))
                                }
                            }
                            a.colocado.borrow_mut().ancho = 0;
                        }
                        if volver {
                            // Volver es salir del proyecto, como en el movil:
                            // lo que estuviera a medias se guarda antes.
                            if let Some(a) = abierto.as_mut() {
                                cerrar_panel(ubicacion, a);
                                apagar_lienzo(ubicacion, a);
                            }
                            if let Some(a) = abierto.take() {
                                borradores.insert(a.ficha.id.clone(), a.borrador);
                            }
                            elegida = None;
                        }
                        hay_que_pintar = true;
                    } else if abierto.as_ref().is_some_and(|a| a.info.is_some()) {
                        // Con el panel abierto, la columna de la derecha es
                        // suya: no se pincha ni el historial ni la caja.
                        if let Some(a) = abierto.as_mut() {
                            let d = pixpin_ui::info::Disposicion::calcular(
                                disposicion.chat,
                                disposicion.una_columna,
                                escala,
                            );
                            if d.volver.contiene(l) {
                                a.info = None;
                                a.buscando_info = false;
                                a.busqueda_info.clear();
                            } else if d.buscar(escala).contiene(l) {
                                // La lupa enciende y apaga. Al apagarla se
                                // limpia: dejar una busqueda escondida haria
                                // que la seccion pareciera vacia sin motivo.
                                a.buscando_info = !a.buscando_info;
                                if !a.buscando_info {
                                    a.busqueda_info.clear();
                                }
                                a.scroll_info = 0;
                            } else if let Some(i) = {
                                let anchos = a.anchos_pestanas.borrow().clone();
                                d.pestana_en(l, &d.pestanas(&anchos, escala))
                            } {
                                a.info = Some(SECCIONES[i]);
                                a.scroll_info = 0;
                            } else if !a.buscando_info && d.cabecera.contiene(l) {
                                // Pulsar el nombre del proyecto lo pone a
                                // escribir. Va el ultimo de la cabecera: el
                                // aspa y la lupa estan ahi dentro y mandan
                                // ellas. Se cierra el panel porque el nombre
                                // se escribe en la cabecera de la
                                // conversacion, que es donde se ve de verdad.
                                a.info = None;
                                a.renombrando = true;
                            }
                            hay_que_pintar = true;
                        }
                    } else if abierto.as_ref().is_some_and(|a| a.mini.is_some()) {
                        // Con una mini-app abierta, el sitio del historial es
                        // suyo: el clic es de un boton, de una fila o de la
                        // cabecera, que es por donde se vuelve.
                        if let Some(a) = abierto.as_mut() {
                            if disposicion.cabecera_chat.contiene(l) {
                                cerrar_panel(ubicacion, a);
                            } else {
                                pulsar_mini(a, l, &disposicion, escala, textos);
                            }
                            hay_que_pintar = true;
                        }
                    } else if abierto.as_ref().is_some_and(|a| a.hoja.is_some()) {
                        // Con la hoja abierta, la conversacion no esta debajo:
                        // el clic es de la hoja o de su cabecera, que es por
                        // donde se vuelve.
                        if let Some(a) = abierto.as_mut() {
                            if disposicion.cabecera_chat.contiene(l) {
                                cerrar_panel(ubicacion, a);
                            } else {
                                let t = disposicion_hoja(&disposicion, escala);
                                if let Some(h) = a.hoja.as_mut() {
                                    if let Some((columna, fila)) =
                                        t.celda_en(l, h.scroll_x, h.scroll_y, escala)
                                    {
                                        // Pulsar otra celda deja escrito lo que
                                        // se estaba tecleando, como en Excel.
                                        h.confirmar();
                                        h.sel = pixpin_proyecto::tabla::Ref { columna, fila };
                                    }
                                }
                            }
                        }
                        hay_que_pintar = true;
                    } else if abierto.is_some() && disposicion.cabecera_chat.contiene(l) {
                        // El papel de entre las pastillas no hace nada: la
                        // cabecera son sus tres pastillas, como en el movil.
                        hay_que_pintar = true;
                    } else if let Some((_, indice)) = abierto.as_ref().and_then(|a| {
                        let area = area_del_historial(&disposicion, a, escala);
                        let c = a.colocado.borrow();
                        let scroll = a.scroll.unwrap_or_else(|| {
                            pixpin_ui::historial::scroll_maximo(area, a.alto.get())
                        });
                        let cual = pixpin_ui::historial::mensaje_en(area, &c.puestos, scroll, l)?;
                        // Buscando no estan todas colocadas: hay que traducir
                        // de «la burbuja numero N» a «el mensaje numero M».
                        c.mensaje(cual).map(|i| (a, i))
                    }) {
                        // Eligiendo varios, pulsar marca y desmarca; Ctrl+clic
                        // hace lo mismo sin entrar antes por el menu, que es
                        // como se eligen varios en un escritorio.
                        let con_ctrl = pixpin_shell::entrada::modificadores_pulsados().ctrl;
                        if let Some(a) = abierto
                            .as_mut()
                            .filter(|a| con_ctrl || !a.marcados.is_empty())
                        {
                            if let Some(id) = a.mensajes.get(indice).map(|m| m.id.clone())
                                && !a.marcados.remove(&id)
                            {
                                a.marcados.insert(id);
                            }
                            // Y sin soltar se puede seguir barriendo hacia
                            // arriba o hacia abajo para marcar varias de un
                            // tiron, como en el movil.
                            a.barriendo = true;
                            ventana.capturar_raton();
                            hay_que_pintar = true;
                            continue;
                        }
                        // Sin seleccion, pulsar una burbuja puede ser DOS
                        // cosas: un toque, que la abre, o un arrastre a la
                        // derecha, que la comenta. Se apunta el gesto y se
                        // decide al soltar; abrir aqui mismo dejaria el
                        // fichero abierto en cuanto se rozara el arrastre.
                        if let Some(a) = abierto.as_mut() {
                            a.comentando = Some((indice, l.x, 0));
                        }
                        ventana.capturar_raton();
                        hay_que_pintar = true;
                    } else if disposicion.boton_nuevo(escala).contiene(l) {
                        // Proyecto nuevo: se crea, se pone el primero y se
                        // abre con el nombre ya listo para escribirlo. Nace
                        // sin nombre a proposito: teclear es mas rapido que
                        // borrar «Proyecto 3» para poner el de verdad.
                        let cuando = pixpin_shell::entorno::ahora_utc_ms();
                        let mut indice = pixpin_proyecto::almacen::Indice::leer(ubicacion.raiz());
                        let ficha = pixpin_proyecto::almacen::Ficha::nueva("", cuando, &identidad);
                        indice.proyectos.push(ficha.clone());
                        match indice.guardar(ubicacion.raiz()) {
                            Ok(()) => {
                                fichas.push(ficha.clone());
                                orden = filtrar(&fichas, &busqueda);
                                elegida = Some(fichas.len() - 1);
                                let mut nuevo = abrir_proyecto(ubicacion, &ficha);
                                nuevo.renombrando = true;
                                abierto = Some(nuevo);
                                scroll = 0;
                            }
                            Err(e) => tracing::warn!(?e, "no se pudo crear el proyecto"),
                        }
                        buscando = false;
                    } else if disposicion.boton_sincro(escala).contiene(l) {
                        // La pantalla de Sincronizar del movil: tus aparatos,
                        // y desde ella Recibir y Enviar.
                        crate::sincronizar::lanzar(idioma, ubicacion.clone());
                        buscando = false;
                        hay_que_pintar = true;
                    } else if disposicion.boton_universo(escala).contiene(l) {
                        // D210: todo el cosmos, en su propio hilo.
                        crate::universo::lanzar(
                            idioma,
                            ubicacion.clone(),
                            lienzo,
                            crate::universo::Pedido::Cosmos,
                        );
                        buscando = false;
                        hay_que_pintar = true;
                    } else if disposicion.buscador(escala).contiene(l) {
                        buscando = true;
                        hay_que_pintar = true;
                    } else if let Some(clip) = abierto
                        .as_ref()
                        .filter(|a| a.borrador.trim().is_empty())
                        .map(|a| disposicion.boton_adjuntar(a.alto_caja.get(), escala))
                        .filter(|r| r.contiene(l))
                    {
                        // El clip despliega su menu; lo que se adjunta se
                        // decide ahi, no aqui. Se retira al escribir, como en
                        // el movil, y entonces no se puede pulsar.
                        menu = Some(MenuAbierto::nuevo(
                            Punto {
                                x: clip.derecha(),
                                y: clip.y,
                            },
                            menu_del_clip(textos),
                        ));
                        buscando = false;
                        hay_que_pintar = true;
                    } else if abierto.as_ref().is_some_and(|a| {
                        disposicion
                            .boton_enviar(a.alto_caja.get(), escala)
                            .contiene(l)
                    }) {
                        if let Some(a) = abierto.as_mut() {
                            if a.borrador.trim().is_empty() {
                                // Sin nada escrito es el microfono, y grabar
                                // todavia no existe aqui: se dice.
                                aviso =
                                    Some((textos.t("chat-no-hay-voz"), std::time::Instant::now()));
                            } else {
                                match guardar_nota(ubicacion, a, &identidad) {
                                    Ok(()) => {
                                        a.scroll = None;
                                        if let Some(i) = elegida {
                                            fichas[i].tocado = a.ficha.tocado;
                                            fichas[i].resumen = a.ficha.resumen.clone();
                                        }
                                    }
                                    Err(e) => tracing::warn!(?e, "no se pudo guardar la nota"),
                                }
                                a.colocado.borrow_mut().ancho = 0;
                            }
                        }
                        buscando = false;
                        hay_que_pintar = true;
                    } else if let Some(fila) = disposicion.fila_en(l, scroll, orden.len(), escala) {
                        let i = orden[fila];
                        buscando = false;
                        // Ctrl+clic marca y no abre: es como se eligen varios
                        // para borrarlos de una vez.
                        if pixpin_shell::entrada::modificadores_pulsados().ctrl {
                            let id = fichas[i].id.clone();
                            if !marcados.remove(&id) {
                                marcados.insert(id);
                            }
                        } else if elegida != Some(i) {
                            marcados.clear();
                            // Lo escrito y sin enviar se guarda antes de
                            // cambiar; volver a este proyecto lo devuelve.
                            // Y la hoja abierta se guarda: cambiar de proyecto no
                            // puede llevarse por delante lo que se escribio.
                            if let Some(a) = abierto.as_mut() {
                                cerrar_panel(ubicacion, a);
                                apagar_lienzo(ubicacion, a);
                            }
                            if let Some(a) = abierto.take() {
                                borradores.insert(a.ficha.id.clone(), a.borrador);
                            }
                            elegida = Some(i);
                            let mut nuevo = abrir_proyecto(ubicacion, &fichas[i]);
                            nuevo.borrador = borradores.remove(&fichas[i].id).unwrap_or_default();
                            abierto = Some(nuevo);
                        }
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::RatonMovido(p) => {
                    let l = local(p);
                    // Mientras el trazo esta en marcha, el raton es del lienzo
                    // vivo aunque se salga de su burbuja: soltar fuera no puede
                    // dejar el trazo a medias.
                    let trazando = abierto
                        .as_mut()
                        .and_then(|a| a.vivo.as_mut())
                        .filter(|v| !v.gesto.en_reposo())
                        .map(|v| {
                            let d = v.destino.get();
                            let escala = v.escala().max(0.0001);
                            let q = pixpin_motor2d::Punto2::nuevo(
                                (l.x as f32 - d.x) / escala,
                                (l.y as f32 - d.y) / escala,
                            );
                            v.gesto.evento(
                                pixpin_motor2d::gesto::EventoGesto::Mover {
                                    p: q,
                                    shift: false,
                                    alt: false,
                                    presion: None,
                                },
                                &mut v.escena,
                                1.0,
                            );
                        })
                        .is_some();
                    if trazando {
                        hay_que_pintar = true;
                        continue;
                    }
                    // Barriendo con el boton pulsado se marca lo que va
                    // pasando por debajo. **Se SUMA, no se alterna**: por
                    // encima de una ya marcada, alternar la desmarcaria al
                    // pasar y la volveria a marcar al volver, y el gesto de
                    // barrer tiene que anadir.
                    if abierto.as_ref().is_some_and(|a| a.barriendo) {
                        let bajo_el_raton = abierto.as_ref().and_then(|a| {
                            let area = area_del_historial(&disposicion, a, escala);
                            let c = a.colocado.borrow();
                            let scroll = a.scroll.unwrap_or_else(|| {
                                pixpin_ui::historial::scroll_maximo(area, a.alto.get())
                            });
                            let cual =
                                pixpin_ui::historial::mensaje_en(area, &c.puestos, scroll, l)?;
                            let i = c.mensaje(cual)?;
                            a.mensajes.get(i).map(|m| m.id.clone())
                        });
                        if let (Some(a), Some(id)) = (abierto.as_mut(), bajo_el_raton) {
                            a.marcados.insert(id);
                        }
                        hay_que_pintar = true;
                        continue;
                    }
                    // Arrastrando una burbuja a la derecha para comentarla:
                    // sigue al raton hasta el tope y ahi se planta.
                    if let Some((_, agarre, _)) = abierto.as_ref().and_then(|a| a.comentando) {
                        let cuanto = pixpin_ui::chat::corrimiento_de_comentar(l.x - agarre, escala);
                        if let Some((_, _, corrida)) =
                            abierto.as_mut().and_then(|a| a.comentando.as_mut())
                        {
                            *corrida = cuanto;
                        }
                        hay_que_pintar = true;
                        continue;
                    }
                    match &arrastre {
                        Some(Arrastre::Asa(agarre)) => {
                            let nuevo = chat::ancho_ajustado(l.x - agarre, marco.ancho, escala);
                            if nuevo != ancho_lista {
                                ancho_lista = nuevo;
                                hay_que_pintar = true;
                            }
                        }
                        Some(Arrastre::Ventana(agarre)) => {
                            // Mover no cambia el tamano: no hay que rehacer
                            // la superficie ni repintar.
                            antes_de_maximizar = None;
                            nuevo_marco = Some(Rect {
                                x: p.x - agarre.x,
                                y: p.y - agarre.y,
                                ..marco
                            });
                        }
                        Some(Arrastre::Borde(b)) => {
                            nuevo_marco = Some(chat::redimensionar(marco, *b, p, escala));
                        }
                        None => {
                            sobre = disposicion.boton_barra_en(l, escala);
                            fila_sobre = disposicion.fila_en(l, scroll, orden.len(), escala);
                            let cursor = chat::borde_en(l, marco.ancho, marco.alto, escala)
                                .map(cursor_de)
                                .unwrap_or(if disposicion.asa.contiene(l) {
                                    FormaCursorWin::RedimEO
                                } else {
                                    FormaCursorWin::Flecha
                                });
                            ventana.poner_cursor(cursor);
                            hay_que_pintar = true;
                        }
                    }
                }
                // El clic derecho sobre una foto abre su menu. Va antes que nada
                // mas: con el menu delante, el clic ya esta atendido.
                EventoOverlay::BotonDerechoPulsado(p) => {
                    let l = local(p);
                    // Sobre la lista, el menu es el del proyecto: borrarlo, o
                    // borrar todos los marcados si este es uno de ellos.
                    if let Some(fila) = disposicion.fila_en(l, scroll, orden.len(), escala) {
                        let id = fichas[orden[fila]].id.clone();
                        let ids: Vec<String> = if marcados.contains(&id) {
                            marcados.iter().cloned().collect()
                        } else {
                            vec![id]
                        };
                        if menu_de_proyecto(&ventana, textos, ids.len()) {
                            if let Some(a) = abierto.as_mut().filter(|a| ids.contains(&a.ficha.id))
                            {
                                // Lo que estuviera a medias se guarda antes:
                                // va a la papelera, y de alli se puede volver.
                                cerrar_panel(ubicacion, a);
                                apagar_lienzo(ubicacion, a);
                            }
                            if abierto.as_ref().is_some_and(|a| ids.contains(&a.ficha.id)) {
                                abierto = None;
                                elegida = None;
                            }
                            let cuando = pixpin_shell::entorno::ahora_utc_ms();
                            match pixpin_proyecto::almacen::borrar_proyectos(
                                ubicacion.raiz(),
                                &ids,
                                cuando,
                            ) {
                                Ok((quitados, sin_mover)) => {
                                    tracing::info!(quitados, "proyectos a la papelera");
                                    for ruta in sin_mover {
                                        tracing::warn!(ruta = %ruta.display(), "carpeta que no se pudo llevar a la papelera");
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(?e, "no se pudieron borrar los proyectos")
                                }
                            }
                            for id in &ids {
                                borradores.remove(id);
                            }
                            marcados.clear();
                            // La lista se relee sola: el indice cambio de fecha.
                        }
                        hay_que_pintar = true;
                        continue;
                    }
                    // Mantener pulsado el titulo abre el buscador, como en el
                    // movil («es el blanco mas grande de la pantalla y llegar
                    // a la lupa de la esquina obliga a cruzarla entera»). En
                    // un escritorio, mantener pulsado es el boton derecho.
                    let en_el_titulo = abierto.as_ref().is_some_and(|a| {
                        a.zonas
                            .borrow()
                            .iter()
                            .rev()
                            .any(|(r, z)| *z == Zona::Titulo && r.contiene(l))
                    });
                    if en_el_titulo {
                        if let Some(a) = abierto.as_mut() {
                            a.busqueda = Some(String::new());
                            a.info = None;
                            a.scroll = None;
                        }
                        hay_que_pintar = true;
                        continue;
                    }
                    // Sobre una burbuja, el menu del mensaje: el de mantener
                    // pulsado del movil, naciendo donde se pulso. Con varios
                    // elegidos manda la barra de arriba, no este menu.
                    let pulsado = abierto
                        .as_ref()
                        .filter(|a| {
                            a.hoja.is_none()
                                && a.mini.is_none()
                                && a.info.is_none()
                                && a.marcados.is_empty()
                        })
                        .and_then(|a| {
                            let area = area_del_historial(&disposicion, a, escala);
                            let c = a.colocado.borrow();
                            let scroll = a.scroll.unwrap_or_else(|| {
                                pixpin_ui::historial::scroll_maximo(area, a.alto.get())
                            });
                            let cual =
                                pixpin_ui::historial::mensaje_en(area, &c.puestos, scroll, l)?;
                            let i = c.mensaje(cual)?;
                            Some(menu_de_mensaje(a, i, textos))
                        });
                    if let Some(entradas) = pulsado.filter(|v| !v.is_empty()) {
                        menu = Some(MenuAbierto::nuevo(l, entradas));
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::BotonSoltado(p) => {
                    // Barrer para marcar termina al levantar el dedo: lo
                    // marcado ya esta puesto, no hay nada que confirmar.
                    if let Some(a) = abierto.as_mut().filter(|a| a.barriendo) {
                        a.barriendo = false;
                        ventana.soltar_raton();
                        hay_que_pintar = true;
                        continue;
                    }
                    // Y el gesto de la burbuja se resuelve aqui: un toque la
                    // abre, un arrastre pasado el gatillo la comenta, y uno
                    // que se quedo corto no hace nada y la burbuja vuelve.
                    if let Some((indice, _, corrida)) =
                        abierto.as_mut().and_then(|a| a.comentando.take())
                    {
                        ventana.soltar_raton();
                        if corrida < ARRASTRE_MINIMO {
                            tocar_la_burbuja(
                                indice,
                                ubicacion,
                                &mut abierto,
                                &fichas,
                                &mut elegida,
                                &mut borradores,
                                textos,
                                lienzo,
                            );
                            buscando = false;
                        } else if pixpin_ui::chat::comenta_al_soltar(corrida, escala)
                            && let Some(a) = abierto.as_mut()
                        {
                            a.respondiendo = a.mensajes.get(indice).map(|m| m.id.clone());
                        }
                        hay_que_pintar = true;
                        continue;
                    }
                    if let Some(v) = abierto
                        .as_mut()
                        .and_then(|a| a.vivo.as_mut())
                        .filter(|v| !v.gesto.en_reposo())
                    {
                        let l = local(p);
                        let d = v.destino.get();
                        let escala = v.escala().max(0.0001);
                        let q = pixpin_motor2d::Punto2::nuevo(
                            (l.x as f32 - d.x) / escala,
                            (l.y as f32 - d.y) / escala,
                        );
                        v.gesto.evento(
                            pixpin_motor2d::gesto::EventoGesto::Soltar { p: q },
                            &mut v.escena,
                            1.0,
                        );
                        hay_que_pintar = true;
                    }
                    if arrastre.is_some() {
                        ventana.soltar_raton();
                    }
                    arrastre = None;
                }
                EventoOverlay::Rueda(delta)
                    if abierto.as_ref().is_some_and(|a| a.mini.is_some()) =>
                {
                    // Solo se desplaza la lista: el numero grande y los
                    // botones se quedan quietos, que es lo que no se puede
                    // perder de vista al bajar por las vueltas.
                    if let Some(a) = abierto.as_mut() {
                        let reparto = a
                            .mini
                            .as_ref()
                            .and_then(|m| m.vista(textos))
                            .map(|v| (v.reparto, v.lista.len()));
                        if let Some((reparto, cuantas)) = reparto
                            && let Some(m) = a.mini.as_mut()
                        {
                            let t = pixpin_ui::mini::Disposicion::calcular(
                                hueco_hoja(&disposicion),
                                escala,
                                reparto,
                            );
                            let paso = 3 * (pixpin_ui::mini::FILA_ALTO * escala / 100) as i32;
                            m.scroll = (m.scroll - delta.signum() * paso)
                                .clamp(0, t.tope_scroll(cuantas, escala));
                        }
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Rueda(delta)
                    if abierto.as_ref().is_some_and(|a| a.hoja.is_some()) =>
                {
                    let t = disposicion_hoja(&disposicion, escala);
                    if let Some(h) = abierto.as_mut().and_then(|a| a.hoja.as_mut()) {
                        let paso = 3 * (pixpin_ui::tabla::FILA_ALTO * escala / 100) as i32;
                        let (columnas, filas) = h.tabla.tamano();
                        // Una de mas por cada lado: si la hoja acabara justo en
                        // la ultima celda escrita, no habria donde anadir.
                        let (x, y) = t.sujetar(
                            h.scroll_x,
                            h.scroll_y - delta.signum() * paso,
                            columnas + 1,
                            filas + 1,
                            escala,
                        );
                        h.scroll_x = x;
                        h.scroll_y = y;
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Rueda(delta) => {
                    // La rueda va a la columna donde esta el raton, no a la
                    // que se pincho la ultima vez: es lo que espera la mano.
                    let aqui = local(pixpin_shell::entorno::posicion_del_cursor());
                    if let Some(a) = abierto.as_mut().filter(|_| disposicion.chat.contiene(aqui)) {
                        let area = area_del_historial(&disposicion, a, escala);
                        let tope = pixpin_ui::historial::scroll_maximo(area, a.alto.get());
                        let paso = 3 * (chat::FILA * escala / 100) as i32;
                        let ahora_en = a.scroll.unwrap_or(tope);
                        let nuevo = pixpin_ui::historial::scroll_ajustado(
                            area,
                            a.alto.get(),
                            ahora_en - delta.signum() * paso,
                        );
                        // Volver al final se guarda como «pegado»: si llegan
                        // mensajes nuevos, se siguen viendo sin tocar nada.
                        a.scroll = if nuevo >= tope { None } else { Some(nuevo) };
                        hay_que_pintar = true;
                        continue;
                    }
                    // Tres filas por muesca, como Telegram y como el ajuste
                    // de Windows por omision.
                    let paso = 3 * (chat::FILA * escala / 100) as i32;
                    let nuevo = disposicion.scroll_ajustado(
                        scroll - delta.signum() * paso,
                        orden.len(),
                        escala,
                    );
                    if nuevo != scroll {
                        scroll = nuevo;
                        // Lo que hay bajo el raton cambia aunque el raton no
                        // se mueva.
                        let p = local(pixpin_shell::entorno::posicion_del_cursor());
                        fila_sobre = disposicion.fila_en(p, scroll, orden.len(), escala);
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::Pintar => hay_que_pintar = true,
                EventoOverlay::Cerrar => cerrar = true,
                // Escapar cierra primero lo que este desplegado encima.
                EventoOverlay::Tecla { vk, .. } if menu.is_some() && vk == VK_ESCAPE => {
                    menu = None;
                    hay_que_pintar = true;
                }
                // D214: Ctrl+U abre el universo en la galaxia del proyecto
                // abierto, o el cosmos entero sin ninguno. Va antes que lo
                // que escribe: Ctrl+U no es una letra en ninguna caja. Solo
                // el cuadro de confirmar lo para, porque es modal.
                EventoOverlay::Tecla { vk, ctrl, .. }
                    if vk == VK_U && ctrl && pendientes.is_none() =>
                {
                    crate::universo::lanzar(
                        idioma,
                        ubicacion.clone(),
                        lienzo,
                        pedido_de_ctrl_u(abierto.as_ref().map(|a| a.ficha.id.as_str())),
                    );
                }
                // El nombre de un lienzo, escribiendose en su fila: manda
                // sobre la nota, como el del proyecto.
                EventoOverlay::Caracter(c)
                    if pendientes.is_none()
                        && abierto
                            .as_ref()
                            .is_some_and(|a| a.renombrando_mensaje.is_some()) =>
                {
                    if c >= ' '
                        && let Some((_, escrito)) = abierto
                            .as_mut()
                            .and_then(|a| a.renombrando_mensaje.as_mut())
                    {
                        escrito.push(c);
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::Tecla { vk, .. }
                    if pendientes.is_none()
                        && abierto
                            .as_ref()
                            .is_some_and(|a| a.renombrando_mensaje.is_some())
                        && (vk == VK_RETROCESO || vk == VK_ENTRAR || vk == VK_ESCAPE) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        match vk {
                            VK_RETROCESO => {
                                if let Some((_, escrito)) = a.renombrando_mensaje.as_mut() {
                                    escrito.pop();
                                }
                            }
                            // Escapar deja el nombre como estaba.
                            VK_ESCAPE => a.renombrando_mensaje = None,
                            _ => {
                                if let Some((i, escrito)) = a.renombrando_mensaje.take() {
                                    guardar_nombre_de_mensaje(ubicacion, a, i, escrito.trim());
                                }
                            }
                        }
                        a.colocado.borrow_mut().ancho = 0;
                    }
                    hay_que_pintar = true;
                }
                // Escapar suelta la seleccion y luego la respuesta a medias,
                // antes que cerrar nada: es lo que hace atras en el movil.
                EventoOverlay::Tecla { vk, .. }
                    if vk == VK_ESCAPE
                        && abierto.as_ref().is_some_and(|a| {
                            !a.marcados.is_empty() || a.respondiendo.is_some()
                        }) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        if a.marcados.is_empty() {
                            a.respondiendo = None;
                        } else {
                            a.marcados.clear();
                        }
                    }
                    hay_que_pintar = true;
                }
                // Escribir en la caja de abajo. Solo llega si hay un
                // proyecto abierto: sin conversacion no hay donde guardarlo.
                // Con el buscador enfocado, lo que se teclea va ahi.
                // Poniendole nombre al proyecto, lo que se teclea es el
                // nombre. Manda sobre escribir una nota: un proyecto recien
                // creado no tiene nombre, y eso es lo primero que resolver.
                EventoOverlay::Caracter(c)
                    if pendientes.is_none() && abierto.as_ref().is_some_and(|a| a.renombrando) =>
                {
                    if c >= ' ' {
                        if let Some(a) = abierto.as_mut() {
                            a.ficha.nombre.push(c);
                            hay_que_pintar = true;
                        }
                    }
                }
                EventoOverlay::Tecla { vk, .. }
                    if vk == VK_RETROCESO
                        && pendientes.is_none()
                        && abierto.as_ref().is_some_and(|a| a.renombrando) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        a.ficha.nombre.pop();
                        hay_que_pintar = true;
                    }
                }
                // Entrar o escapar dejan de escribir el nombre. Los dos
                // guardan: escapar aqui no puede «deshacer», porque el
                // proyecto ya existe y quedaria sin nombre para siempre.
                EventoOverlay::Tecla { vk, .. }
                    if (vk == VK_ENTRAR || vk == VK_ESCAPE)
                        && pendientes.is_none()
                        && abierto.as_ref().is_some_and(|a| a.renombrando) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        a.renombrando = false;
                        guardar_nombre(ubicacion, a);
                        if let Some(i) = elegida {
                            fichas[i].nombre = a.ficha.nombre.clone();
                            orden = filtrar(&fichas, &busqueda);
                        }
                    }
                    hay_que_pintar = true;
                }
                // El cuadro de confirmar es modal tambien para el teclado: lo
                // que se escribe es el pie, escapar cancela y entrar acepta.
                EventoOverlay::Caracter(c) if pendientes.is_some() => {
                    if c >= ' ' {
                        if let Some(p) = pendientes.as_mut() {
                            p.pie.push(c);
                            hay_que_pintar = true;
                        }
                    }
                }
                EventoOverlay::Tecla { vk, .. } if pendientes.is_some() && vk == VK_RETROCESO => {
                    if let Some(p) = pendientes.as_mut() {
                        p.pie.pop();
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::Tecla { vk, .. } if pendientes.is_some() && vk == VK_ESCAPE => {
                    pendientes = None;
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk, .. } if pendientes.is_some() && vk == VK_ENTRAR => {
                    if let (Some(p), Some(a)) = (pendientes.take(), abierto.as_mut()) {
                        let (hechos, suyos) =
                            meter_ficheros(ubicacion, a, &identidad, &p.rutas, &p.pie);
                        pdfs_pendientes = suyos;
                        if hechos > 0 {
                            a.scroll = None;
                            if let Some(i) = elegida {
                                fichas[i].tocado = a.ficha.tocado;
                                fichas[i].resumen = a.ficha.resumen.clone();
                            }
                        }
                        a.colocado.borrow_mut().ancho = 0;
                    }
                    hay_que_pintar = true;
                }
                // La lupa del panel manda sobre todo lo demas: mientras esta
                // encendida, lo que se teclea es lo que se busca ahi dentro,
                // no una nota ni el buscador de la lista.
                // La hoja de calculo se lleva el teclado entero mientras esta
                // abierta: escribir en una celda es escribir, y cualquier
                // otra cosa que se colara aqui iria a parar al borrador.
                // Una mini-app abierta se lleva el teclado igual que la hoja:
                // lo que se escriba es de su caja de anadir, no del borrador
                // de la conversacion que esta detras.
                EventoOverlay::Caracter(c)
                    if abierto.as_ref().is_some_and(|a| a.mini.is_some()) =>
                {
                    if c >= ' '
                        && let Some(m) = abierto.as_mut().and_then(|a| a.mini.as_mut())
                    {
                        m.borrador.push(c);
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::Tecla { vk, .. }
                    if abierto.as_ref().is_some_and(|a| a.mini.is_some()) =>
                {
                    // Escapar con algo escrito lo borra y solo el segundo
                    // cierra: es la unica forma de arrepentirse de lo tecleado
                    // sin perder el panel.
                    let vaciar = vk == VK_ESCAPE
                        && abierto
                            .as_ref()
                            .and_then(|a| a.mini.as_ref())
                            .is_some_and(|m| !m.borrador.is_empty());
                    if vk == VK_ESCAPE && !vaciar {
                        if let Some(a) = abierto.as_mut() {
                            cerrar_panel(ubicacion, a);
                        }
                    } else if let Some(a) = abierto.as_mut() {
                        let textos_ref = textos;
                        if let Some(m) = a.mini.as_mut() {
                            match vk {
                                VK_ESCAPE => m.borrador.clear(),
                                VK_RETROCESO => {
                                    m.borrador.pop();
                                }
                                VK_ENTRAR if !m.borrador.trim().is_empty() => {
                                    let texto = std::mem::take(&mut m.borrador);
                                    m.hacer(&crate::mini_panel::Orden::Anadir(texto), textos_ref);
                                    // Al final de la lista, que es donde acaba
                                    // de caer lo anadido.
                                    if let Some(v) = m.vista(textos_ref) {
                                        let t = pixpin_ui::mini::Disposicion::calcular(
                                            hueco_hoja(&disposicion),
                                            escala,
                                            v.reparto,
                                        );
                                        m.scroll = t.tope_scroll(v.lista.len(), escala);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Caracter(c)
                    if abierto.as_ref().is_some_and(|a| a.hoja.is_some()) =>
                {
                    if c >= ' '
                        && let Some(h) = abierto.as_mut().and_then(|a| a.hoja.as_mut())
                    {
                        // Teclear sobre una celda empieza de cero, sin lo que
                        // hubiera: es lo que hace cualquier hoja de calculo.
                        h.edicion.get_or_insert_with(String::new).push(c);
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::Tecla { vk, shift, .. }
                    if abierto.as_ref().is_some_and(|a| a.hoja.is_some()) =>
                {
                    let t = disposicion_hoja(&disposicion, escala);
                    let cerrar_la_hoja = vk == VK_ESCAPE
                        && abierto
                            .as_ref()
                            .and_then(|a| a.hoja.as_ref())
                            .is_some_and(|h| h.edicion.is_none());
                    if cerrar_la_hoja {
                        if let Some(a) = abierto.as_mut() {
                            cerrar_panel(ubicacion, a);
                        }
                    } else if let Some(h) = abierto.as_mut().and_then(|a| a.hoja.as_mut()) {
                        match vk {
                            // Escapar con algo tecleado deja la celda como
                            // estaba: es la unica forma de arrepentirse.
                            VK_ESCAPE => h.edicion = None,
                            VK_ENTRAR => h.mover(0, 1, &t, escala),
                            VK_TAB if shift => h.mover(-1, 0, &t, escala),
                            VK_TAB => h.mover(1, 0, &t, escala),
                            VK_IZQUIERDA => h.mover(-1, 0, &t, escala),
                            VK_DERECHA => h.mover(1, 0, &t, escala),
                            VK_ARRIBA => h.mover(0, -1, &t, escala),
                            VK_ABAJO => h.mover(0, 1, &t, escala),
                            VK_RETROCESO => match h.edicion.as_mut() {
                                Some(texto) => {
                                    texto.pop();
                                }
                                // Sin nada tecleado, retroceso vacia la celda:
                                // es lo que la mano espera, y Suprimir hace lo
                                // mismo para quien venga de otra hoja.
                                None => h.edicion = Some(String::new()),
                            },
                            VK_SUPR => {
                                h.edicion = Some(String::new());
                                h.confirmar();
                            }
                            _ => {}
                        }
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Caracter(c) if abierto.as_ref().is_some_and(|a| a.buscando_info) => {
                    if c >= ' ' {
                        if let Some(a) = abierto.as_mut() {
                            a.busqueda_info.push(c);
                            a.scroll_info = 0;
                            hay_que_pintar = true;
                        }
                    }
                }
                EventoOverlay::Tecla { vk, .. }
                    if vk == VK_RETROCESO && abierto.as_ref().is_some_and(|a| a.buscando_info) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        a.busqueda_info.pop();
                        a.scroll_info = 0;
                        hay_que_pintar = true;
                    }
                }
                // Escapar apaga la lupa antes que cerrar el panel, y el panel
                // antes que la ventana.
                EventoOverlay::Tecla { vk, .. }
                    if vk == VK_ESCAPE && abierto.as_ref().is_some_and(|a| a.buscando_info) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        a.buscando_info = false;
                        a.busqueda_info.clear();
                        a.scroll_info = 0;
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk, .. }
                    if vk == VK_ESCAPE && abierto.as_ref().is_some_and(|a| a.info.is_some()) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        a.info = None;
                    }
                    hay_que_pintar = true;
                }
                // Lo que se teclea con la lupa de la conversacion encendida es
                // para ella y no para la caja de escribir. Va ANTES que el
                // borrador, que si no lo escrito acabaria en un mensaje.
                EventoOverlay::Caracter(c)
                    if abierto.as_ref().is_some_and(|a| a.busqueda.is_some()) =>
                {
                    if c >= ' '
                        && let Some(q) = abierto.as_mut().and_then(|a| a.busqueda.as_mut())
                    {
                        q.push(c);
                        if let Some(a) = abierto.as_mut() {
                            a.scroll = None;
                        }
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk, .. }
                    if vk == VK_RETROCESO
                        && abierto.as_ref().is_some_and(|a| a.busqueda.is_some()) =>
                {
                    if let Some(q) = abierto.as_mut().and_then(|a| a.busqueda.as_mut()) {
                        q.pop();
                    }
                    if let Some(a) = abierto.as_mut() {
                        a.scroll = None;
                    }
                    hay_que_pintar = true;
                }
                // Escapar apaga la lupa de la conversacion antes que nada mas.
                EventoOverlay::Tecla { vk, .. }
                    if vk == VK_ESCAPE
                        && abierto.as_ref().is_some_and(|a| a.busqueda.is_some()) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        a.busqueda = None;
                        a.por_etiqueta = None;
                        a.scroll = None;
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Caracter(c) if buscando => {
                    if c >= ' ' {
                        busqueda.push(c);
                        orden = filtrar(&fichas, &busqueda);
                        scroll = 0;
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::Tecla { vk, .. } if buscando && vk == VK_RETROCESO => {
                    busqueda.pop();
                    orden = filtrar(&fichas, &busqueda);
                    scroll = 0;
                    hay_que_pintar = true;
                }
                // Escapar del buscador lo limpia y suelta el foco, antes que
                // cerrar la ventana entera.
                EventoOverlay::Tecla { vk, .. } if buscando && vk == VK_ESCAPE => {
                    buscando = false;
                    busqueda.clear();
                    orden = filtrar(&fichas, &busqueda);
                    hay_que_pintar = true;
                }
                EventoOverlay::Caracter(c) if abierto.is_some() => {
                    // WM_CHAR trae tambien los mandos (retroceso, enter);
                    // esos se atienden por tecla, no como letra.
                    if c >= ' ' || c == '\n' {
                        if let Some(a) = abierto.as_mut() {
                            a.borrador.push(c);
                            a.colocado.borrow_mut().ancho = 0;
                            hay_que_pintar = true;
                        }
                    }
                }
                EventoOverlay::FicherosSoltados => {
                    // Las rutas se recogen SIEMPRE, haya proyecto abierto o
                    // no: si no, se quedarian ahi y aparecerian en el
                    // siguiente que se abra, que seria peor que perderlas.
                    let rutas = pixpin_shell::overlay::ficheros_soltados();
                    if abierto.is_some() {
                        pendientes = Pendientes::de(rutas);
                        menu = None;
                        hay_que_pintar = true;
                    } else {
                        tracing::info!(
                            cuantos = rutas.len(),
                            "ficheros soltados sin proyecto abierto"
                        );
                    }
                }
                EventoOverlay::Tecla { vk, ctrl, .. } if vk == VK_V && ctrl => {
                    if let Some(a) = abierto.as_mut() {
                        match pegar(ubicacion, a, &identidad) {
                            Ok(cuantos) if cuantos > 0 => {
                                a.scroll = None;
                                if let Some(i) = elegida {
                                    fichas[i].tocado = a.ficha.tocado;
                                    fichas[i].resumen = a.ficha.resumen.clone();
                                }
                            }
                            Ok(_) => {}
                            Err(e) => tracing::warn!(?e, "no se pudo pegar en el chat"),
                        }
                        a.colocado.borrow_mut().ancho = 0;
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::Tecla { vk, shift, .. } if vk == VK_RETROCESO => {
                    if let Some(a) = abierto.as_mut().filter(|a| !a.borrador.is_empty()) {
                        a.borrador.pop();
                        a.colocado.borrow_mut().ancho = 0;
                        hay_que_pintar = true;
                    }
                    let _ = shift;
                }
                EventoOverlay::Tecla { vk, shift, .. } if vk == VK_ENTRAR => {
                    if let Some(a) = abierto.as_mut() {
                        if shift {
                            // Mayusculas y entrar: un renglon mas, como en
                            // Telegram. Entrar solo, se envia.
                            a.borrador.push('\n');
                        } else if !a.borrador.trim().is_empty() {
                            match guardar_nota(ubicacion, a, &identidad) {
                                Ok(()) => {
                                    // Vuelve al final: lo que acabas de
                                    // escribir tiene que verse.
                                    a.scroll = None;
                                    if let Some(i) = elegida {
                                        fichas[i].tocado = a.ficha.tocado;
                                        fichas[i].resumen = a.ficha.resumen.clone();
                                    }
                                }
                                Err(e) => tracing::warn!(?e, "no se pudo guardar la nota"),
                            }
                        }
                        a.colocado.borrow_mut().ancho = 0;
                        hay_que_pintar = true;
                    }
                }
                // Escapar con algo escrito no cierra: se perderia. Primero
                // limpia, y el segundo escape ya cierra.
                EventoOverlay::Tecla { vk, .. }
                    if vk == VK_ESCAPE
                        && abierto.as_ref().is_some_and(|a| !a.borrador.is_empty()) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        a.borrador.clear();
                        a.colocado.borrow_mut().ancho = 0;
                    }
                    hay_que_pintar = true;
                }
                // Escape apaga primero el lienzo vivo (y lo guarda); solo
                // sin ninguno encendido cierra la ventana.
                EventoOverlay::Tecla { vk, .. }
                    if vk == VK_ESCAPE && abierto.as_ref().is_some_and(|a| a.vivo.is_some()) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        apagar_lienzo(ubicacion, a);
                        a.colocado.borrow_mut().ancho = 0;
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk, .. } if vk == VK_ESCAPE => cerrar = true,
                _ => {}
            }
        }
        // Si la lista de proyectos cambio por fuera —algo que llego del
        // movil, por ejemplo— se relee. Se mira la fecha del indice, que es
        // una consulta al sistema de ficheros y solo al despertar.
        if let Ok(ahora_sello) = std::fs::metadata(ubicacion.raiz().join("proyectos/indice.json"))
            .and_then(|m| m.modified())
            && sello_indice != Some(ahora_sello)
        {
            if sello_indice.is_some() {
                let elegida_id = elegida.and_then(|i| fichas.get(i)).map(|f| f.id.clone());
                fichas = pixpin_proyecto::almacen::Indice::leer(ubicacion.raiz())
                    .ordenadas()
                    .into_iter()
                    .cloned()
                    .collect();
                orden = filtrar(&fichas, &busqueda);
                // El proyecto abierto sigue siendo el mismo aunque haya
                // cambiado de sitio en la lista.
                elegida = elegida_id.and_then(|id| fichas.iter().position(|f| f.id == id));
                hay_que_pintar = true;
            }
            sello_indice = Some(ahora_sello);
            // Si el indice cambio, el cuaderno del proyecto abierto pudo
            // cambiar con el: lo que llega del movil entra por los dos sitios.
            pendiente_de_releer = true;
        }

        // Lo que llego por fuera (una sincronizacion con el movil) se relee
        // sin tirar lo que el usuario tenga a medias. Con una hoja o un
        // lienzo abiertos se espera: el que esta escribiendo manda.
        if tomar_refrescar() {
            pendiente_de_releer = true;
        }
        if pendiente_de_releer
            && let Some(a) = abierto.as_mut()
            && releer_lo_abierto(ubicacion, a)
        {
            pendiente_de_releer = false;
            hay_que_pintar = true;
        }
        if pendiente_de_releer && abierto.is_none() {
            // Sin proyecto abierto no hay cuaderno que releer; la lista ya se
            // puso al dia sola unas lineas mas arriba.
            pendiente_de_releer = false;
        }

        // Lo que pidio el universo (Ctrl+clic en una galaxia o una nota):
        // abrir ese proyecto como si se pulsara su fila y, si hay mensaje,
        // llevar la vista a el y resaltarlo. Se mira en cada vuelta, que es
        // un candado sin nadie esperando: asi vale al despertar y al nacer.
        if let Some((proyecto, codigo)) = tomar_ir_a() {
            match fichas.iter().position(|f| f.id == proyecto) {
                Some(i) => {
                    if elegida != Some(i) || abierto.is_none() {
                        if let Some(a) = abierto.as_mut() {
                            cerrar_panel(ubicacion, a);
                            apagar_lienzo(ubicacion, a);
                        }
                        if let Some(a) = abierto.take() {
                            borradores.insert(a.ficha.id.clone(), a.borrador);
                        }
                        marcados.clear();
                        elegida = Some(i);
                        let mut nuevo = abrir_proyecto(ubicacion, &fichas[i]);
                        nuevo.borrador = borradores.remove(&fichas[i].id).unwrap_or_default();
                        abierto = Some(nuevo);
                    }
                    if let Some(a) = abierto.as_mut() {
                        // Lo que tapara la conversacion se quita: se viene a
                        // ver ese mensaje. La hoja se guarda al cerrarse.
                        a.info = None;
                        cerrar_panel(ubicacion, a);
                        if let Some(j) = codigo.and_then(|c| indice_de_codigo(&a.mensajes, &c)) {
                            a.ir_a = Some(j);
                            a.resaltado = Some((j, std::time::Instant::now()));
                        }
                    }
                    menu = None;
                    hay_que_pintar = true;
                }
                None => {
                    tracing::warn!(%proyecto, "el universo pidio un proyecto que no esta en la lista")
                }
            }
        }
        // El resalte se apaga solo, como el aviso.
        if let Some(a) = abierto.as_mut()
            && a.resaltado
                .is_some_and(|(_, desde)| desde.elapsed() >= RESALTE)
        {
            a.resaltado = None;
            hay_que_pintar = true;
        }

        // Un PDF que haya entrado abre su propio proyecto, con una hoja por
        // pagina: cada chat ES un proyecto, y un documento entero no es un
        // adjunto suelto de otra conversacion.
        for ruta in std::mem::take(&mut pdfs_pendientes) {
            match proyecto_de_pdf(ubicacion, &identidad, &ruta) {
                Ok((ficha, hechas, total)) => {
                    if hechas < total {
                        tracing::info!(hechas, total, "el PDF tiene mas paginas que el tope");
                    }
                    if let Some(a) = abierto.as_mut() {
                        cerrar_panel(ubicacion, a);
                        apagar_lienzo(ubicacion, a);
                    }
                    fichas.push(ficha.clone());
                    orden = filtrar(&fichas, &busqueda);
                    elegida = Some(fichas.len() - 1);
                    abierto = Some(abrir_proyecto(ubicacion, &ficha));
                    scroll = 0;
                    hay_que_pintar = true;
                }
                Err(e) => {
                    tracing::warn!(?e, ruta = %ruta.display(), "no se pudo abrir el PDF")
                }
            }
        }

        if cerrar {
            // Cerrar la ventana con una hoja o un lienzo abiertos los guarda:
            // es lo mismo que cerrarlos con Escape, y perderlos aqui seria una
            // trampa.
            if let Some(a) = abierto.as_mut() {
                cerrar_panel(ubicacion, a);
                apagar_lienzo(ubicacion, a);
            }
            break;
        }

        if let Some(r) = nuevo_marco.filter(|r| *r != marco) {
            let cambia_el_tamano = (r.ancho, r.alto) != (marco.ancho, marco.alto);
            marco = r;
            ventana.mover(marco);
            if cambia_el_tamano {
                let _ = superficie.redimensionar(marco.ancho, marco.alto);
                ancho_lista = chat::ancho_ajustado(ancho_lista as i32, marco.ancho, escala);
                // Al hacerse mas alta caben mas filas: si estaba abajo del
                // todo, quedaria hueco en blanco bajo la ultima.
                let d = Disposicion::calcular(
                    marco.ancho,
                    marco.alto,
                    escala,
                    ancho_lista,
                    Vista::Ambas,
                );
                scroll = d.scroll_ajustado(scroll, orden.len(), escala);
                hay_que_pintar = true;
            }
        }

        // Llevar la vista a un mensaje (una cita, el fijado, un comentario):
        // un poco por encima de el, para que se vea de donde viene.
        // Un proyecto recien abierto aun no esta medido (se mide al pintar):
        // entonces se espera a la vuelta siguiente, en vez de perder el
        // destino.
        if let Some(a) = abierto.as_mut()
            && a.colocado.borrow().puestos.len() == cuantas_se_ven(a)
            && let Some(j) = a.ir_a.take()
        {
            let area = area_del_historial(&disposicion, a, escala);
            // Con una busqueda puesta, el mensaje al que se iba puede estar
            // filtrado y no tener sitio: entonces no se salta a ninguna parte.
            let arriba = {
                let c = a.colocado.borrow();
                c.puesto_de(j)
                    .and_then(|n| c.puestos.get(n).map(|p| p.arriba()))
            };
            if let Some(arriba) = arriba {
                let tope = pixpin_ui::historial::scroll_maximo(area, a.alto.get());
                let quiero = pixpin_ui::historial::scroll_ajustado(
                    area,
                    a.alto.get(),
                    arriba - (24 * escala / 100) as i32,
                );
                a.scroll = if quiero >= tope { None } else { Some(quiero) };
            }
            hay_que_pintar = true;
        }
        // El aviso se va solo, como un Toast.
        if aviso
            .as_ref()
            .is_some_and(|(_, desde)| desde.elapsed().as_millis() as u64 >= AVISO_MS)
        {
            aviso = None;
            hay_que_pintar = true;
        }

        if hay_que_pintar {
            hay_que_pintar = false;
            // La barra de responder cuenta como parte de la caja de escribir:
            // se vuelve a repartir con lo que haya cambiado en esta vuelta.
            let disposicion = disponer(marco, escala, ancho_lista, abierto.as_ref());
            // Las fotos del historial que se ven, igual que las del panel.
            if let Some(a) = abierto.as_ref().filter(|a| a.info.is_none()) {
                let rutas = fotos_del_historial(a, &disposicion, escala);
                if !rutas.is_empty() {
                    previas.asegurar(&rutas, &motor);
                }
            }
            // Las fotos que se van a ver, leidas y subidas ANTES de empezar
            // el fotograma: crear recursos de dibujo a medias no se puede, y
            // leer del disco dentro del fotograma se notaria.
            if let Some(a) = abierto.as_ref() {
                if let Some(seccion) = a.info {
                    let d = pixpin_ui::info::Disposicion::calcular(
                        disposicion.chat,
                        disposicion.una_columna,
                        escala,
                    );
                    let rutas = fotos_a_la_vista(a, seccion, &d, escala);
                    if !rutas.is_empty() {
                        miniaturas.asegurar(&rutas, &motor);
                    }
                }
            }
            // Y las del cuadro de confirmar, que son pocas y se ven todas.
            if let Some(p) = pendientes.as_ref() {
                let rutas: Vec<std::path::PathBuf> = p
                    .rutas
                    .iter()
                    .take(pixpin_ui::confirmar::FILAS_MAXIMAS)
                    .cloned()
                    .collect();
                miniaturas.asegurar(&rutas, &motor);
            }
            if let Ok(destino) = superficie.empezar(&motor) {
                let lista = Lista {
                    fichas: &fichas,
                    orden: &orden,
                    scroll,
                    sobre: fila_sobre,
                    elegida,
                    ahora,
                    textos,
                    busqueda: &busqueda,
                    marcados: &marcados,
                };
                let abierto_ref = abierto.as_ref();
                let pendientes_ref = pendientes.as_ref();
                let menu_ref = menu.as_ref();
                let aviso_ref = aviso.as_ref().map(|(t, _)| t.as_str());
                let papel = papel_de(fondo, claro);
                let resultado = motor.dibujar(&destino, |p: &Pintor| {
                    pintar(p, &disposicion, tema, papel, escala, textos, sobre, &lista);
                    let mut alto_caja = 0;
                    if let Some(a) = abierto_ref {
                        // Lo que se puede pulsar se apunta de nuevo en cada
                        // fotograma: lo que ya no se ve no se puede pulsar.
                        a.zonas.borrow_mut().clear();
                        // Medir lo escrito decide lo alta que es la caja y,
                        // con ello, donde acaba el historial. Se hace una
                        // vez y lo usan los dos.
                        let ef = escala as f32 / 100.0;
                        let tam = chat::REDACCION_TAM * ef;
                        let ancho = disposicion.texto_redaccion(0, escala).ancho as f32;
                        let alto_texto = if a.borrador.is_empty() {
                            (tam * 1.3).ceil() as u32
                        } else {
                            p.medir_texto_ajustado(&a.borrador, tam, ancho.max(1.0))
                                .1
                                .ceil() as u32
                        };
                        a.alto_caja.set(alto_texto);
                        alto_caja = alto_texto;
                        let c = Pinta {
                            tema,
                            escala,
                            textos,
                            ahora,
                            miniaturas: &miniaturas,
                            previas: &previas,
                        };
                        match a.info {
                            // Con la informacion abierta, la columna de la
                            // derecha es suya entera.
                            Some(seccion) => pintar_info(p, &disposicion, &c, a, seccion),
                            // Con una hoja abierta, la conversacion deja su sitio:
                            // una tabla necesita todo el ancho y el alto que
                            // haya, y el historial vuelve al cerrarla.
                            // Y con una mini-app, lo mismo: su panel ocupa el
                            // sitio del historial hasta que se cierre.
                            None if a.mini.is_some() => {
                                if let Some(m) = a.mini.as_ref() {
                                    pintar_mini(p, &disposicion, &c, m);
                                }
                                pintar_cabecera(p, &disposicion, &c, a);
                                a.zonas.borrow_mut().clear();
                            }
                            None if a.hoja.is_some() => {
                                if let Some(h) = a.hoja.as_ref() {
                                    pintar_hoja(p, &disposicion, &c, h);
                                }
                                // La cabecera sigue a la vista, que es donde se
                                // lee de que proyecto es; pulsarla cierra la
                                // hoja, asi que sus pastillas no se apuntan.
                                pintar_cabecera(p, &disposicion, &c, a);
                                a.zonas.borrow_mut().clear();
                            }
                            None => {
                                pintar_historial(p, &disposicion, &c, a);
                                pintar_redaccion(p, &disposicion, &c, a, alto_texto);
                            }
                        }
                        // Encima de todo, panel incluido: esta esperando una
                        // respuesta y nada debe distraer de ella.
                        if let Some(pend) = pendientes_ref {
                            pintar_confirmar(p, &c, pend, marco);
                        }
                    }
                    // El menu, el ultimo: se despliega encima de todo.
                    if let Some(m) = menu_ref {
                        pintar_menu_abierto(p, tema, escala, m, marco);
                    }
                    if let Some(t) = aviso_ref {
                        pintar_aviso(p, &disposicion, escala, alto_caja, t);
                    }
                });
                if resultado.is_err() {
                    // Dispositivo perdido: los bitmaps de las miniaturas eran
                    // del viejo y D2D los rechazaria. Los pixeles se quedan,
                    // asi que volver a subirlos no toca el disco.
                    miniaturas.soltar();
                    previas.soltar();
                }
                let _ = superficie.presentar();
            }
            // La colocacion de las burbujas se hace al pintar, asi que las
            // fotos que se ven solo se conocen DESPUES. Si falta alguna, otra
            // vuelta: sin ella no saldrian hasta mover el raton.
            if let Some(a) = abierto.as_ref().filter(|a| a.info.is_none()) {
                hay_que_pintar = fotos_del_historial(a, &disposicion, escala)
                    .iter()
                    .any(|r| previas.pendiente(r));
            }
            // Un destino que esperaba a que se midieran las burbujas: ya
            // estan, otra vuelta para llevar la vista alli.
            // Solo con el historial a la vista: con la hoja o el panel
            // delante no se mide nada, y esto daria vueltas sin parar.
            if abierto.as_ref().is_some_and(|a| {
                a.ir_a.is_some() && a.hoja.is_none() && a.mini.is_none() && a.info.is_none()
            }) {
                hay_que_pintar = true;
            }
        }
        // Sin nada que hacer, el hilo duerme: la ventana abierta en reposo no
        // cuesta CPU. Con un aviso a la vista, solo hasta que toque quitarlo.
        let hasta_el_aviso = aviso.as_ref().map(|(_, desde)| {
            AVISO_MS.saturating_sub(desde.elapsed().as_millis() as u64) as u32 + 1
        });
        // Y hasta que se apague el resalte, si hay uno.
        let hasta_el_resalte = abierto
            .as_ref()
            .and_then(|a| a.resaltado)
            .map(|(_, desde)| RESALTE.saturating_sub(desde.elapsed()).as_millis() as u32 + 1);
        // Y hasta el siguiente repintado de una mini-app que se mueve sola:
        // un cronometro en marcha tiene que ir contando sin que nadie toque
        // nada. **No escribe en disco**, igual que en el movil
        // (`MiniActivity.kt:356-362`): lo que cambia es el numero calculado.
        let hasta_el_latido = abierto
            .as_ref()
            .and_then(|a| a.mini.as_ref())
            .and_then(|m| m.vista(textos))
            .and_then(|v| v.late_cada_ms)
            .map(|ms| ms as u32);
        let dormir = [hasta_el_aviso, hasta_el_resalte, hasta_el_latido]
            .into_iter()
            .flatten()
            .min();
        pixpin_shell::overlay::esperar_eventos(if hay_que_pintar { Some(0) } else { dormir });
    }

    // Donde quedo, para la proxima vez.
    estado.chat_ventana = Some([marco.x, marco.y, marco.ancho as i32, marco.alto as i32]);
    if let Err(e) = pixpin_store::estado::guardar(ubicacion, &estado) {
        tracing::warn!(?e, "no se pudo recordar donde quedo la ventana de chat");
    }
    ventana.ocultar();
    Ok(())
}

/// Los ficheros que esperan un si o un no en el cuadro de confirmar.
struct Pendientes {
    rutas: Vec<std::path::PathBuf>,
    /// Lo que se escriba acompanara al primero, como el pie de una foto en
    /// Android. Va al primero y no a todos porque repetir el mismo texto en
    /// veinte mensajes no ayuda a nadie.
    pie: String,
    /// El tamano de cada uno, ya leido. Se mira una vez al abrir el cuadro y
    /// no en cada fotograma: preguntarle al disco sesenta veces por segundo
    /// por algo que no cambia es tirar el rato.
    tamanos: Vec<u64>,
}

impl Pendientes {
    /// `None` si no hay ninguna ruta: un cuadro que pregunta por nada no se
    /// ensena, se descarta.
    fn de(rutas: Vec<std::path::PathBuf>) -> Option<Pendientes> {
        if rutas.is_empty() {
            return None;
        }
        let tamanos = rutas
            .iter()
            .map(|r| std::fs::metadata(r).map(|m| m.len()).unwrap_or(0))
            .collect();
        Some(Pendientes {
            rutas,
            pie: String::new(),
            tamanos,
        })
    }
}

/// El proyecto abierto en la columna de la derecha.
struct Abierto {
    ficha: pixpin_proyecto::almacen::Ficha,
    /// Donde viven los proyectos. Se guarda aqui porque quien pinta tiene que
    /// resolver la ruta de una foto para buscar su miniatura, y hasta el
    /// pintado no llega la `Ubicacion`.
    raiz: std::path::PathBuf,
    mensajes: Vec<pixpin_proyecto::cuaderno::Mensaje>,
    /// Lineas del cuaderno que no se entendieron. Se ensenan: si faltan
    /// mensajes, el usuario tiene que enterarse.
    rotas: usize,
    /// El lienzo de cada mensaje que sea un dibujo, si se pudo leer. Va en
    /// paralelo a `mensajes` y se lee UNA vez al abrir el proyecto: leer y
    /// traducir un excalidraw en cada fotograma seria tirar el rato.
    vistas: Vec<Option<Ojeada>>,
    /// Cual de los mensajes esta fijado, si hay alguno. En Android es el
    /// campo `fijado`; aqui se ensena en una barra bajo la cabecera.
    fijado: Option<usize>,
    /// Lo escrito y todavia sin enviar.
    borrador: String,
    /// Lo alto que mide ese texto ya medido con la fuente. Lo apunta el
    /// pintado; la rueda lo necesita para saber donde acaba el historial.
    alto_caja: std::cell::Cell<u32>,
    /// La pantalla de informacion esta abierta encima de la conversacion,
    /// con su seccion y su desplazamiento propios.
    info: Option<pixpin_proyecto::cuaderno::Seccion>,
    scroll_info: i32,
    /// Lo que se busca DENTRO del proyecto, desde la lupa del panel. Vacio
    /// es «no se esta buscando»: no hace falta un booleano aparte, porque
    /// una busqueda en blanco no filtra nada (ver `resaltado`).
    busqueda_info: String,
    /// Se esta escribiendo el nombre del proyecto en la cabecera. Un
    /// proyecto recien creado nace asi: teclear es mas rapido que borrar
    /// un nombre puesto por la aplicacion para poner el de verdad.
    renombrando: bool,
    /// La lupa esta encendida. Va aparte de `busqueda_info` porque al
    /// pulsarla la caja aparece vacia, y sin esto no habria donde escribir.
    buscando_info: bool,
    /// Lo que se busca DENTRO DE LA CONVERSACION, con la lupa de la cabecera.
    ///
    /// Tres estados, como en el movil (`var consulta: String?`): `None` es la
    /// lupa apagada, `Some("")` es la caja abierta y vacia —que no esconde
    /// ninguna burbuja— y `Some(texto)` es buscando. Con un booleano aparte
    /// habria que mantener dos cosas a la vez y se descuadran.
    busqueda: Option<String>,
    /// La etiqueta por la que se filtra, de los chips que salen bajo la
    /// cabecera mientras se busca (`porEtiqueta` del movil).
    ///
    /// **Se limpia al apagar la lupa**, que es lo que el movil NO hace: alli
    /// `consulta = null` deja `porEtiqueta` puesto y la conversacion se
    /// queda medio vacia sin que nada diga por que. Un filtro que no se ve
    /// no existe para quien mira.
    por_etiqueta: Option<String>,
    /// Lo que ocupa la seccion abierta, que solo se sabe al colocarla.
    alto_info: std::cell::Cell<u32>,
    /// Lo ancho que mide el rotulo de cada pestana. Lo apunta el pintado,
    /// que es quien tiene la fuente; el raton lo necesita para saber en cual
    /// se pulso.
    anchos_pestanas: std::cell::RefCell<Vec<f32>>,
    /// A que mensaje se esta contestando (su id). Aparte de `marcados`:
    /// uno elige sobre que actuar y esto deja un mensaje colgado mientras se
    /// escribe la respuesta, como en el movil.
    respondiendo: Option<String>,
    /// El mensaje al que se le cambia el nombre, y lo escrito hasta ahora.
    renombrando_mensaje: Option<(usize, String)>,
    /// Los mensajes elegidos para actuar sobre varios, por id: por posicion
    /// se descolocarian al borrar.
    marcados: std::collections::BTreeSet<String>,
    /// Lo que se puede pulsar, tal como quedo en el ultimo fotograma.
    zonas: std::cell::RefCell<Vec<(Rect, Zona)>>,
    /// A que mensaje hay que llevar la vista en cuanto se sepa donde cae.
    ir_a: Option<usize>,
    /// El mensaje que se resalta y desde cuando: al llegar desde el
    /// universo, durante `RESALTE`.
    resaltado: Option<(usize, std::time::Instant)>,
    /// Desde arriba. `None` es «pegado al final», que es como se abre y
    /// como se queda hasta que el usuario sube.
    scroll: Option<i32>,
    /// Lo que ocupa todo el historial, que solo se sabe al medirlo (hace
    /// falta la fuente). Lo apunta el pintado para que la rueda sepa su tope.
    alto: std::cell::Cell<u32>,
    /// La colocacion ya medida, y para que ancho se midio. Medir el texto de
    /// mil mensajes en cada fotograma seria tirar el rato: solo se rehace si
    /// cambia el ancho de la columna.
    colocado: std::cell::RefCell<Colocado>,
    /// La hoja de calculo que se esta editando, si hay alguna. Se edita
    /// DENTRO de la conversacion, en el sitio del historial: una ventana
    /// aparte por cada tabla llenaria el escritorio, y la hoja es del
    /// proyecto, no de la aplicacion.
    hoja: Option<HojaAbierta>,
    /// La mini-aplicacion abierta, si hay alguna. Ocupa el mismo sitio que
    /// la hoja de calculo y por el mismo motivo: es del proyecto, no de la
    /// aplicacion.
    mini: Option<MiniAbierta>,
    /// El lienzo que esta vivo dentro de su burbuja, si hay alguno.
    vivo: Option<LienzoVivo>,
    /// La burbuja que se esta arrastrando a la derecha para comentarla: cual
    /// es, donde se agarro y cuanto lleva corrida.
    ///
    /// El gesto se sigue aunque el raton se salga de la burbuja: soltar fuera
    /// no puede dejarla a medio camino y clavada.
    comentando: Option<(usize, i32, i32)>,
    /// Se esta barriendo el historial con el boton pulsado para marcar varias
    /// burbujas de un tiron. Solo existe con la seleccion ya abierta.
    barriendo: bool,
}

/// Una hoja de calculo abierta para escribir en ella.
struct HojaAbierta {
    /// Que mensaje del historial es. La tabla ES su texto (no hay fichero),
    /// asi que guardar es reescribir ese mensaje.
    indice: usize,
    tabla: pixpin_proyecto::tabla::Tabla,
    /// La celda elegida. Siempre hay una: una hoja sin celda elegida no
    /// sabe donde escribir la primera tecla.
    sel: pixpin_proyecto::tabla::Ref,
    /// Lo que se esta tecleando en la celda. `None` es «elegida pero no se
    /// esta escribiendo», que es cuando las flechas andan por la hoja.
    edicion: Option<String>,
    scroll_x: i32,
    scroll_y: i32,
    /// Si cambio algo desde que se abrio. Sin esto, abrir una tabla y
    /// cerrarla la reescribiria en el disco para nada.
    tocada: bool,
}

#[derive(Default)]
struct Colocado {
    ancho: u32,
    /// Que mensajes se colocaron, por su posicion en `Abierto::mensajes`.
    ///
    /// Buscando NO se colocan todos: el movil FILTRA la conversacion en vez
    /// de saltar de un resultado a otro, asi que `puestos` y `piezas` van en
    /// paralelo a ESTO y no a los mensajes. Quien pulse una burbuja tiene que
    /// traducir por aqui, o acabara abriendo la que no es.
    visibles: Vec<usize>,
    /// Con que busqueda se filtro, para no rehacerlo en cada fotograma.
    busqueda: String,
    /// Y con que chip de etiqueta. Va en la llave del cache igual que la
    /// busqueda: sin esto, pulsar un chip no rehace la colocacion y las
    /// burbujas se quedan como estaban.
    por_etiqueta: Option<String>,
    puestos: Vec<pixpin_ui::historial::Puesto>,
    /// De que se compone cada burbuja, ya medido, en el mismo orden.
    piezas: Vec<Piezas>,
}

impl Colocado {
    /// Que mensaje es la burbuja numero `cual` de las colocadas.
    fn mensaje(&self, cual: usize) -> Option<usize> {
        self.visibles.get(cual).copied()
    }

    /// Y al reves: donde quedo colocado el mensaje `indice`, si se ve. Con
    /// una busqueda puesta puede no verse, y entonces no hay a donde ir.
    fn puesto_de(&self, indice: usize) -> Option<usize> {
        self.visibles.iter().position(|i| *i == indice)
    }
}

/// Lo que hace falta para pintar la lista.
struct Lista<'a> {
    fichas: &'a [pixpin_proyecto::almacen::Ficha],
    /// Que fichas se ensenan, por indice en `fichas`.
    orden: &'a [usize],
    scroll: i32,
    sobre: Option<usize>,
    elegida: Option<usize>,
    /// La hora local de ahora, para decidir si una fecha es de hoy.
    ahora: i64,
    textos: &'a Catalogo,
    /// Lo que hay escrito en el buscador.
    busqueda: &'a str,
    /// Los proyectos marcados para borrar, por `id`.
    marcados: &'a std::collections::BTreeSet<String>,
}

/// Los colores de los avatares, los mismos siete de Telegram. Cual toca sale
/// del codigo unico del proyecto, asi que un proyecto tiene siempre el suyo,
/// aqui y en el telefono.
const COLORES_AVATAR: [u32; 7] = [
    0xe17076, 0xfaa774, 0xa695e7, 0x7bc862, 0x6ec9cb, 0x65aadd, 0xee7aae,
];

/// Las letras del avatar: la inicial de las dos primeras palabras.
fn iniciales(nombre: &str) -> String {
    nombre
        .split_whitespace()
        .take(2)
        .filter_map(|p| p.chars().next())
        .flat_map(|c| c.to_uppercase())
        .collect()
}

/// El universo pinta cada galaxia con este mismo color (D217).
pub(crate) fn color_avatar(codigo: &str) -> Color {
    let suma: u32 = codigo.bytes().map(u32::from).sum();
    hex(COLORES_AVATAR[suma as usize % COLORES_AVATAR.len()])
}

#[allow(clippy::too_many_arguments)]
fn pintar(
    p: &Pintor,
    d: &Disposicion,
    tema: &Tema,
    papel: (Color, Color),
    escala: u32,
    textos: &Catalogo,
    sobre: Option<BotonBarra>,
    lista: &Lista,
) {
    let e = escala as f32 / 100.0;
    p.limpiar(papel.0);
    // El papel del chat, en degradado como en el movil. Por bandas y no con
    // un pincel de degradado porque el pintor no tiene uno: a cien bandas no
    // se distingue una de otra y cuesta lo que cien rellenos.
    if d.chat.ancho > 0 && d.chat.alto > 0 {
        const BANDAS: u32 = 100;
        let alto = (d.chat.alto as f32 / BANDAS as f32).max(1.0);
        for n in 0..BANDAS {
            let t = n as f32 / (BANDAS - 1) as f32;
            let mezcla = |a: f32, b: f32| a + (b - a) * t;
            p.rellenar(
                RectF {
                    x: d.chat.x as f32,
                    y: d.chat.y as f32 + n as f32 * alto,
                    ancho: d.chat.ancho as f32,
                    // Un pelo mas para que el redondeo no deje rayas entre
                    // banda y banda.
                    alto: alto + 1.0,
                },
                Color {
                    r: mezcla(papel.0.r, papel.1.r),
                    g: mezcla(papel.0.g, papel.1.g),
                    b: mezcla(papel.0.b, papel.1.b),
                    a: 1.0,
                },
            );
        }
    }
    p.rellenar(rf(d.barra), tema.barra);
    p.rellenar(rf(d.lista), tema.lista);
    p.rellenar(rf(d.cabecera_lista), tema.cabecera);
    // La cabecera del proyecto NO se rellena: en el movil no es una barra
    // sino tres pastillas flotando sobre el papel (`CabeceraFlotante`).

    // Las lineas que separan: 1 px logico, como Telegram.
    let linea = (1.0 * e).max(1.0);
    for (r, vertical) in [(d.barra, false), (d.cabecera_lista, false), (d.lista, true)] {
        if r.ancho == 0 {
            continue;
        }
        let borde = if vertical {
            RectF {
                x: r.derecha() as f32 - linea,
                y: r.y as f32,
                ancho: linea,
                alto: r.alto as f32,
            }
        } else {
            RectF {
                x: r.x as f32,
                y: r.abajo() as f32 - linea,
                ancho: r.ancho as f32,
                alto: linea,
            }
        };
        p.rellenar(borde, tema.separador);
    }

    // El nombre del programa a la izquierda de la barra.
    let centrar_texto =
        |texto: &str, zona: Rect, izquierda: Option<f32>, tam: f32, color: Color| {
            if zona.ancho == 0 {
                return;
            }
            let (w, h) = p.medir_texto(texto, tam);
            let x = match izquierda {
                Some(m) => zona.x as f32 + m,
                None => zona.x as f32 + (zona.ancho as f32 - w) / 2.0,
            };
            p.texto(
                texto,
                x,
                zona.y as f32 + (zona.alto as f32 - h) / 2.0,
                tam,
                color,
            );
        };
    centrar_texto(
        &textos.t("app-nombre"),
        d.barra,
        Some(12.0 * e),
        chat::TITULO_TAM * e,
        tema.texto,
    );

    // Los tres botones, con su resaltado al pasar el raton.
    for (boton, r) in d.botones_barra(escala) {
        if sobre == Some(boton) {
            p.rellenar(
                rf(r),
                if boton == BotonBarra::Cerrar {
                    tema.cerrar_sobre
                } else {
                    tema.boton_sobre
                },
            );
        }
        let color = if sobre == Some(BotonBarra::Cerrar) && boton == BotonBarra::Cerrar {
            Color::BLANCO
        } else {
            tema.texto
        };
        // Los simbolos de Windows: raya, cuadro y aspa, dibujados a mano
        // porque son tres lineas y un icono aqui seria un recurso de mas.
        let (cx, cy) = (
            r.x as f32 + r.ancho as f32 / 2.0,
            r.y as f32 + r.alto as f32 / 2.0,
        );
        // Los glifos de Telegram en Windows, medidos: la raya de minimizar
        // 12x3, el cuadro de maximizar 12x12 y el aspa de cerrar 10x10.
        let lado = match boton {
            BotonBarra::Cerrar => 10.0 * e,
            _ => 12.0 * e,
        };
        let grosor = (1.0 * e).max(1.0);
        match boton {
            BotonBarra::Minimizar => p.rellenar(
                RectF {
                    x: cx - lado / 2.0,
                    y: cy - (3.0 * e).max(1.0) / 2.0,
                    ancho: lado,
                    alto: (3.0 * e).max(1.0),
                },
                color,
            ),
            BotonBarra::Maximizar => p.trazar(
                RectF {
                    x: cx - lado / 2.0,
                    y: cy - lado / 2.0,
                    ancho: lado,
                    alto: lado,
                },
                grosor,
                color,
            ),
            BotonBarra::Cerrar => {
                let m = lado / 2.0;
                p.linea((cx - m, cy - m), (cx + m, cy + m), grosor, color);
                p.linea((cx - m, cy + m), (cx + m, cy - m), grosor, color);
            }
        }
    }

    // El boton de proyecto nuevo, flotando sobre la lista. Se pinta despues
    // de las filas para que quede encima de la que pase por debajo.
    let nuevo = d.boton_nuevo(escala);
    if nuevo.ancho > 0 {
        p.rellenar_redondeado(rf(nuevo), nuevo.alto as f32 / 2.0, tema.enviar);
        // Una cruz a mano: dos lineas y un icono aqui seria un recurso de
        // mas, como el triangulo de enviar.
        let brazo = nuevo.ancho as f32 * 0.22;
        let (cx, cy) = (
            nuevo.x as f32 + nuevo.ancho as f32 / 2.0,
            nuevo.y as f32 + nuevo.alto as f32 / 2.0,
        );
        let grosor = (2.0 * e).max(1.0);
        p.linea(
            (cx - brazo, cy),
            (cx + brazo, cy),
            grosor,
            tema.texto_elegido,
        );
        p.linea(
            (cx, cy - brazo),
            (cx, cy + brazo),
            grosor,
            tema.texto_elegido,
        );
    }

    // Los botones de sincronizar y del universo (D210): redondos, marron
    // oscuro y con el icono crema, los colores de los botones redondos del
    // chat del movil. Los dos iguales porque los dos llevan a una pantalla
    // de TODOS los proyectos.
    for (boton, icono) in [
        (d.boton_sincro(escala), &SINCRO),
        (d.boton_universo(escala), &mi::PUBLIC),
    ] {
        if boton.ancho == 0 {
            continue;
        }
        p.rellenar_redondeado(
            rf(boton),
            boton.alto as f32 / 2.0,
            Color {
                r: 0.290,
                g: 0.227,
                b: 0.071,
                a: 1.0,
            },
        );
        let lado = boton.ancho as f32 * 0.56;
        p.icono(
            icono,
            RectF {
                x: boton.x as f32 + (boton.ancho as f32 - lado) / 2.0,
                y: boton.y as f32 + (boton.alto as f32 - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            Color {
                r: 0.961,
                g: 0.902,
                b: 0.722,
                a: 1.0,
            },
        );
    }

    // El buscador, en el sitio del titulo: es lo que hay en Telegram y lo
    // que hace falta en cuanto pasas de diez proyectos.
    let caja = d.buscador(escala);
    if caja.ancho > 0 {
        p.rellenar_redondeado(rf(caja), chat::BUSCADOR_RADIO as f32 * e, tema.buscador);
        let (texto_busqueda, color) = if lista.busqueda.is_empty() {
            (textos.t("chat-buscar"), tema.apagado)
        } else {
            (lista.busqueda.to_string(), tema.texto)
        };
        let (_, alto_texto) = p.medir_texto(&texto_busqueda, chat::BUSCADOR_TAM * e);
        p.texto_linea(
            &texto_busqueda,
            caja.x as f32 + chat::BUSCADOR_TEXTO_X as f32 * e,
            caja.y as f32 + (caja.alto as f32 - alto_texto) / 2.0,
            chat::BUSCADOR_TAM * e,
            (caja.ancho as f32 - 2.0 * chat::BUSCADOR_TEXTO_X as f32 * e).max(0.0),
            color,
        );
    }

    // Sin proyectos todavia: se dice, en vez de dejar la columna en blanco
    // que parece un fallo.
    if lista.orden.is_empty() {
        centrar_texto(
            &textos.t("chat-sin-proyectos"),
            d.filas,
            None,
            13.0 * e,
            tema.apagado,
        );
    } else {
        pintar_filas(p, d, tema, escala, lista);
    }

    // Sin proyecto elegido, la columna de la derecha dice que hay que
    // elegir uno; con uno elegido la pinta `pintar_historial`.
    if lista.elegida.is_none() {
        centrar_texto(
            &textos.t("chat-elige-proyecto"),
            d.chat,
            None,
            13.0 * e,
            tema.apagado,
        );
    }
}

/// Pinta **solo las filas que se ven**: una lista de mil proyectos cuesta lo
/// mismo que una de diez.
fn pintar_filas(p: &Pintor, d: &Disposicion, tema: &Tema, escala: u32, lista: &Lista) {
    let e = escala as f32 / 100.0;
    if d.filas.ancho == 0 || d.filas.alto == 0 {
        return;
    }
    // Recorte al area de filas: la primera y la ultima suelen salirse, y sin
    // esto pintarian encima de la cabecera.
    p.empujar_recorte(rf(d.filas));
    let (primera, cuantas) = d.visibles(lista.scroll, lista.orden.len(), escala);
    for i in primera..primera + cuantas {
        let ficha = &lista.fichas[lista.orden[i]];
        let r = d.fila(i, lista.scroll, escala);
        // Se compara contra el indice del PROYECTO, no contra el numero de
        // fila: al filtrar, la fila 0 ya no es el primer proyecto.
        let elegida = lista.elegida == Some(lista.orden[i]);
        let marcada = lista.marcados.contains(&ficha.id);
        if marcada {
            // El mismo rojo apagado del boton de borrar: marcar es el paso
            // previo a borrar, y tiene que verse que no es «abierto».
            p.rellenar(
                rf(r),
                Color {
                    r: 0.75,
                    g: 0.22,
                    b: 0.20,
                    a: 0.35,
                },
            );
        } else if elegida {
            p.rellenar(rf(r), tema.fila_elegida);
        } else if lista.sobre == Some(i) {
            p.rellenar(rf(r), tema.fila_sobre);
        }
        let partes = pixpin_ui::chat::partes_fila(r, d.plegada, escala);

        // El avatar: un circulo de su color con las iniciales.
        let a = rf(partes.avatar);
        p.rellenar_redondeado(a, a.ancho / 2.0, color_avatar(&ficha.codigo_unico()));
        let letras = iniciales(&ficha.nombre);
        let tam = a.alto * 0.4;
        let (w, h) = p.medir_texto(&letras, tam);
        p.texto(
            &letras,
            a.x + (a.ancho - w) / 2.0,
            a.y + (a.alto - h) / 2.0,
            tam,
            Color::BLANCO,
        );
        if partes.ancho_texto == 0 {
            continue;
        }

        let (nombre_color, resumen_color) = if elegida {
            (tema.texto_elegido, tema.texto_elegido)
        } else {
            (tema.texto, tema.apagado)
        };
        // La hora primero: dice cuanto sitio le queda al nombre.
        let hora = pixpin_ui::chat::etiqueta_hora(
            pixpin_shell::entorno::a_local(ficha.tocado),
            lista.ahora,
        );
        let tam_hora = chat::CONTADOR_TAM * e;
        let mut hueco_nombre = partes.ancho_texto as f32;
        if !hora.is_empty() {
            let (w, h) = p.medir_texto(&hora, tam_hora);
            p.texto(
                &hora,
                partes.derecha as f32 - w,
                partes.nombre.y as f32 + (chat::TEXTO_TAM * e - h) / 2.0,
                tam_hora,
                if elegida {
                    tema.texto_elegido
                } else {
                    tema.apagado
                },
            );
            hueco_nombre -= w + chat::HORA_HUECO as f32 * e;
        }
        p.texto_linea(
            &ficha.nombre,
            partes.nombre.x as f32,
            partes.nombre.y as f32,
            chat::TEXTO_TAM * e,
            hueco_nombre.max(0.0),
            nombre_color,
        );

        // Y la ultima linea, dejando sitio al contador si lo hay.
        let mut hueco_resumen = partes.ancho_texto as f32;
        if ficha.sin_leer > 0 {
            // El numero entero, sin «99+»: Telegram ensancha la pildora y
            // ensena los pendientes que hay, que es el dato que importa.
            let texto = ficha.sin_leer.to_string();
            let alto = chat::CONTADOR_ALTO as f32 * e;
            let (w, h) = p.medir_texto(&texto, chat::CONTADOR_TAM * e);
            // Cinco de relleno a cada lado, y nunca mas estrecha que alta:
            // con un solo digito sale un circulo.
            let ancho = (w + 2.0 * chat::CONTADOR_RELLENO as f32 * e).max(alto);
            let caja = RectF {
                x: partes.derecha as f32 - ancho,
                y: partes.resumen.y as f32,
                ancho,
                alto,
            };
            p.rellenar_redondeado(
                caja,
                alto / 2.0,
                if elegida {
                    tema.texto_elegido
                } else {
                    tema.contador
                },
            );
            p.texto(
                &texto,
                caja.x + (ancho - w) / 2.0,
                caja.y + (alto - h) / 2.0,
                chat::CONTADOR_TAM * e,
                if elegida {
                    tema.fila_elegida
                } else {
                    Color::BLANCO
                },
            );
            hueco_resumen -= ancho + chat::HORA_HUECO as f32 * e;
        }
        // Mientras no haya mensajes, la ultima linea dice lo que tiene
        // dentro; el texto se compone aqui porque aqui esta el idioma.
        let resumen = if ficha.resumen.is_empty() {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("cuantas", ficha.hojas);
            lista.textos.t_args("chat-hojas", &args)
        } else {
            ficha.resumen.clone()
        };
        p.texto_linea(
            &resumen,
            partes.resumen.x as f32,
            partes.resumen.y as f32,
            chat::TEXTO_TAM * e,
            hueco_resumen.max(0.0),
            resumen_color,
        );
    }
    p.soltar_recorte();
}

/// Que dice la etiqueta de un mensaje que no es solo texto.
fn clase_de(m: &pixpin_proyecto::cuaderno::Mensaje, textos: &Catalogo) -> Option<String> {
    use pixpin_proyecto::cuaderno::Clase;
    let clave = match m.clase.as_ref()? {
        // Una nota es solo su texto: ponerle «Nota» encima no anade nada.
        Clase::Nota => return None,
        Clase::Imagen => "chat-clase-imagen",
        Clase::Archivo => "chat-clase-archivo",
        Clase::Voz => "chat-clase-voz",
        Clase::Dibujo => "chat-clase-dibujo",
        Clase::Pagina => "chat-clase-pagina",
        Clase::Proyecto => "chat-clase-proyecto",
        Clase::MiniApp => "chat-clase-miniapp",
        // Una clase que no conocemos se ensena con su propia palabra: es
        // mas honrado que callarla o fingir que es una nota.
        Clase::Otra(palabra) => return Some(palabra.clone()),
    };
    Some(textos.t(clave))
}

/// La columna de la derecha: cabecera del proyecto y sus mensajes.
/// Lo que hace falta para pintar, junto: el tema, la escala, los textos y
/// la hora. Van juntos porque siempre viajan juntos.
struct Pinta<'a> {
    tema: &'a Tema,
    escala: u32,
    textos: &'a Catalogo,
    ahora: i64,
    /// Las fotos ya leidas. Solo se consultan: lo que falte por cargar se
    /// preparo antes de empezar el fotograma.
    miniaturas: &'a crate::miniaturas::Miniaturas,
    /// Las mismas fotos, mas grandes, para la vista previa de las burbujas.
    previas: &'a crate::miniaturas::Miniaturas,
}

fn pintar_historial(p: &Pintor, d: &Disposicion, c: &Pinta, a: &Abierto) {
    let (tema, escala, textos, ahora) = (c.tema, c.escala, c.textos, c.ahora);
    // Aparte, porque mas abajo `c` pasa a ser la colocacion.
    let previas = c.previas;
    use pixpin_ui::historial as h;
    let e = escala as f32 / 100.0;
    if d.chat.ancho == 0 {
        return;
    }

    // La cabecera: tres pastillas flotando, como en el movil.
    pintar_cabecera(p, d, c, a);

    // La barra del mensaje fijado, si lo hay: una rayita de color, el
    // rotulo y la linea del mensaje, como en Telegram y en Android. Tocarla
    // lleva al mensaje (`BarraDeFijados`).
    if let Some((indice_fijado, fijado)) = a.fijado.and_then(|i| Some((i, a.mensajes.get(i)?))) {
        let barra = d.fijado(escala);
        p.rellenar(rf(barra), tema.cabecera);
        p.rellenar(
            RectF {
                x: barra.x as f32 + chat::FIJADO_MARGEN_X as f32 * e,
                y: barra.y as f32 + 8.0 * e,
                ancho: (chat::FIJADO_RAYA as f32 * e).max(1.0),
                alto: barra.alto as f32 - 16.0 * e,
            },
            tema.enviar,
        );
        let x = barra.x as f32 + (chat::FIJADO_MARGEN_X + 10) as f32 * e;
        let ancho = (barra.derecha() as f32 - chat::FIJADO_MARGEN_X as f32 * e - x).max(0.0);
        p.texto_linea(
            &textos.t("chat-fijado"),
            x,
            barra.y as f32 + 7.0 * e,
            chat::CONTADOR_TAM * e,
            ancho,
            tema.enviar,
        );
        p.texto_linea(
            &fijado.resumen(),
            x,
            barra.y as f32 + 25.0 * e,
            chat::CABECERA_TAM * e,
            ancho,
            tema.texto,
        );
        p.rellenar(
            RectF {
                x: barra.x as f32,
                y: barra.abajo() as f32 - (1.0 * e).max(1.0),
                ancho: barra.ancho as f32,
                alto: (1.0 * e).max(1.0),
            },
            tema.separador,
        );
        a.zonas.borrow_mut().push((barra, Zona::Ir(indice_fijado)));
    }

    // La fila de chips de etiqueta, solo mientras se busca: cuelga de la
    // cabecera (o del fijado) y le roba alto al historial. Se pinta ANTES
    // que las burbujas porque su alto es el que decide donde empiezan.
    let chips = chips_de(a);
    if !chips.is_empty() {
        let fila = d.chips(chips.len(), a.fijado.is_some(), escala);
        p.rellenar(rf(fila), tema.cabecera);
        p.rellenar(
            RectF {
                x: fila.x as f32,
                y: fila.abajo() as f32 - (1.0 * e).max(1.0),
                ancho: fila.ancho as f32,
                alto: (1.0 * e).max(1.0),
            },
            tema.separador,
        );
        for (i, (caja, emoji)) in chat::fila_de_chips(fila, chips.len(), escala)
            .into_iter()
            .zip(&chips)
            .enumerate()
        {
            let puesto = a.por_etiqueta.as_deref() == Some(emoji.as_str());
            let fondo = if puesto { tema.enviar } else { tema.pildora };
            p.rellenar_redondeado(rf(caja), caja.alto as f32 / 2.0, fondo);
            // El emoji, centrado: es todo lo que lleva el chip, asi que si
            // se descentra se nota mas que en un rotulo.
            let tam = 17.0 * e;
            let (w, alto) = p.medir_texto(emoji, tam);
            p.texto(
                emoji,
                caja.x as f32 + (caja.ancho as f32 - w) / 2.0,
                caja.y as f32 + (caja.alto as f32 - alto) / 2.0,
                tam,
                if puesto {
                    tema.texto_elegido
                } else {
                    tema.pildora_texto
                },
            );
            a.zonas.borrow_mut().push((caja, Zona::Chip(i)));
        }
    }

    // El area de los mensajes: entre la cabecera (o el fijado) y la caja.
    let area = area_del_historial(d, a, escala);
    if a.mensajes.is_empty() {
        let vacio = textos.t("chat-sin-mensajes");
        let (w, alto) = p.medir_texto(&vacio, 13.0 * e);
        p.texto(
            &vacio,
            area.x as f32 + (area.ancho as f32 - w) / 2.0,
            area.y as f32 + (area.alto as f32 - alto) / 2.0,
            13.0 * e,
            tema.apagado,
        );
        return;
    }

    // Medir el texto necesita la fuente, asi que se hace aqui; pero solo
    // cuando cambia el ancho, no en cada fotograma.
    let ancho_contenido = h::ancho_contenido(area, escala);
    let aguja = a.busqueda.clone().unwrap_or_default();
    {
        let mut c = a.colocado.borrow_mut();
        if c.ancho != ancho_contenido
            || c.puestos.len() != cuantas_se_ven(a)
            || c.busqueda != aguja
            || c.por_etiqueta != a.por_etiqueta
        {
            // Buscando, solo se miden y se colocan las que casan: el movil
            // FILTRA la conversacion («quien busca "factura enero" quiere las
            // cinco juntas para compararlas»), no salta de una a otra.
            let visibles = indices_visibles(a);
            let mut entradas = Vec::with_capacity(visibles.len());
            let mut todas = Vec::with_capacity(visibles.len());
            for indice in &visibles {
                let m = &a.mensajes[*indice];
                let (entrada, piezas) = medir_mensaje(
                    p,
                    a,
                    *indice,
                    m,
                    ancho_contenido,
                    Medir {
                        textos,
                        e,
                        ahora,
                        aguja: &aguja,
                    },
                );
                entradas.push(entrada);
                todas.push(piezas);
            }
            let (puestos, alto) = h::colocar(area, &entradas, escala);
            a.alto.set(alto);
            *c = Colocado {
                ancho: ancho_contenido,
                visibles,
                busqueda: aguja.clone(),
                por_etiqueta: a.por_etiqueta.clone(),
                puestos,
                piezas: todas,
            };
        }
    }

    let c = a.colocado.borrow();
    if c.visibles.is_empty() {
        // Buscando sin resultados es otra pantalla, no la de bienvenida: ahi
        // ya se sabe que es esto, lo que falta es decir que no hay nada.
        let vacio = textos.t("chat-buscar-nada");
        let (w, alto) = p.medir_texto(&vacio, 13.0 * e);
        p.texto(
            &vacio,
            area.x as f32 + (area.ancho as f32 - w) / 2.0,
            area.y as f32 + (area.alto as f32 - alto) / 2.0,
            13.0 * e,
            tema.apagado,
        );
        return;
    }
    // Sin desplazamiento propio, pegado al final: lo ultimo es lo que importa.
    let scroll = a
        .scroll
        .unwrap_or_else(|| h::scroll_maximo(area, a.alto.get()));
    p.empujar_recorte(rf(area));
    let (primero, cuantos) = h::visibles(area, &c.puestos, scroll);
    let en_guardados = a.ficha.es_guardados();
    for cual in primero..primero + cuantos {
        let puesto = c.puestos[cual];
        let piezas = &c.piezas[cual];
        let i = c.visibles[cual];
        let m = &a.mensajes[i];
        // Lo que se ha corrido esta burbuja al arrastrarla a la derecha para
        // comentarla. Se corre TODO lo suyo, no solo el fondo.
        let corrida = match a.comentando {
            Some((j, _, cuanto)) if j == i => cuanto,
            _ => 0,
        };
        let mover = |r: Rect| Rect {
            x: r.x + corrida,
            y: r.y + area.y - scroll,
            ..r
        };

        if let Some(sep) = puesto.separador {
            // La pildora del dia NO se corre con la burbuja: es del dia, no
            // del mensaje, y verla escaparse al arrastrar seria raro.
            let sep = Rect {
                y: sep.y + area.y - scroll,
                ..sep
            };
            let fecha = fecha_larga(pixpin_shell::entorno::a_local(m.cuando), ahora, textos);
            if !fecha.is_empty() {
                // La pastilla del dia del movil (`SeparadorDeDia`): radio 11,
                // 8 de aire a los lados y 3 arriba y abajo, letra de 14.
                let tam = h::SEPARADOR_TAM * e;
                let (w, alto_texto) = p.medir_texto(&fecha, tam);
                let alto = h::SEPARADOR_PILDORA as f32 * e;
                let ancho = w + 2.0 * h::SEPARADOR_RELLENO_X as f32 * e;
                let caja = RectF {
                    x: sep.x as f32 + (sep.ancho as f32 - ancho) / 2.0,
                    y: sep.abajo() as f32 - 2.0 * e - alto,
                    ancho,
                    alto,
                };
                p.rellenar_redondeado(caja, 11.0 * e, tema.separador_dia);
                p.texto(
                    &fecha,
                    caja.x + h::SEPARADOR_RELLENO_X as f32 * e,
                    caja.y + (alto - alto_texto) / 2.0,
                    tam,
                    tema.texto_separador,
                );
            }
        }

        let burbuja = mover(puesto.burbuja);
        // Lo elegido se tine de lado a lado, no solo la burbuja: se ve de un
        // vistazo cuantos van. Y la casilla dice CUAL (`Burbuja` del movil).
        if a.marcados.contains(&m.id) {
            p.rellenar(
                RectF {
                    x: area.x as f32,
                    y: burbuja.y as f32 - 1.0 * e,
                    ancho: area.ancho as f32,
                    alto: burbuja.alto as f32 + 2.0 * e,
                },
                con_alfa(tema.enviar, 0.16),
            );
        }
        // El mensaje al que se llego desde el universo, tenido un momento
        // del color de la fila elegida: entre cien burbujas, dice cual era.
        if a.resaltado
            .is_some_and(|(j, desde)| j == i && desde.elapsed() < RESALTE)
        {
            p.rellenar(
                RectF {
                    x: area.x as f32,
                    y: burbuja.y as f32 - 1.0 * e,
                    ancho: area.ancho as f32,
                    alto: burbuja.alto as f32 + 2.0 * e,
                },
                con_alfa(tema.fila_elegida, 0.35),
            );
        }
        if !a.marcados.is_empty() {
            let lado = (h::MARGEN as f32 - 2.0) * e;
            let icono = if a.marcados.contains(&m.id) {
                &mi::CHECK_BOX
            } else {
                &mi::CHECK_BOX_OUTLINE_BLANK
            };
            p.icono(
                icono,
                RectF {
                    x: area.x as f32 + 1.0 * e,
                    y: burbuja.abajo() as f32 - lado - 4.0 * e,
                    ancho: lado,
                    alto: lado,
                },
                tema.enviar,
            );
        }

        // Todas del mismo azul, el de la burbuja del movil, y con el contorno
        // del color de su lienzo si viene de uno (`colorDelDibujo`).
        let (color, color_texto, color_hora) = (tema.burbuja_mia, tema.texto_mio, tema.hora_mia);
        let radio = RADIO_BURBUJA * e;
        let grosor = GROSOR_BORDE * e;
        let dentro_del_borde = match piezas.borde {
            Some(borde) => {
                p.rellenar_redondeado(rf(burbuja), radio, borde);
                let dentro = encoger(rf(burbuja), grosor);
                p.rellenar_redondeado(dentro, (radio - grosor).max(0.0), color);
                dentro
            }
            None => {
                p.rellenar_redondeado(rf(burbuja), radio, color);
                rf(burbuja)
            }
        };

        if piezas.foto_sola {
            // Una foto sola se come la burbuja (`shouldDrawTimeOnMedia`): sin
            // relleno, redondeada con ella y la hora encima, en su pastilla.
            let (_, alto_vista) = piezas.vista.unwrap_or((0.0, 0.0));
            let foto = RectF {
                alto: (alto_vista - if piezas.borde.is_some() { grosor } else { 0.0 })
                    .min(dentro_del_borde.alto)
                    .max(1.0),
                ..dentro_del_borde
            };
            let recortada = p.empujar_recorte_redondeado(foto, (radio - grosor).max(0.0));
            pintar_foto(p, a, i, foto, previas, tema, e);
            if recortada {
                p.soltar_recorte_redondeado();
            }
            pintar_hora_sobre_foto(p, m, foto, ahora, e);
            let zona = boton_de_esquina(foto, e);
            pintar_boton_de_esquina(p, zona);
            a.zonas.borrow_mut().push((zona, Zona::Abrir(i)));
            punto_de_proyecto(
                p,
                RectF {
                    x: foto.x + 8.0 * e,
                    y: foto.y + 8.0 * e,
                    ancho: chat::FILA_PUNTO as f32 * e,
                    alto: chat::FILA_PUNTO as f32 * e,
                },
                !en_guardados,
            );
            if let Some(v) = &piezas.viene {
                let caja =
                    pintar_viene_de(p, tema, textos, v, burbuja.x as f32, foto.y + foto.alto, e);
                a.zonas.borrow_mut().push((caja, Zona::VieneDe(i)));
            }
            continue;
        }

        let dentro = Rect {
            y: burbuja.y + (h::RELLENO_Y as f32 * e) as i32,
            ..mover(puesto.dentro(escala))
        };
        let hueco = h::RELLENO_Y as f32 * e;
        let mut y = dentro.y as f32;

        // A que contesta: la banda con la barra de color de Telegram, del
        // ancho de la burbuja. Tocarla lleva al original.
        if let Some(j) = piezas.cita {
            let alto = CITA_ALTO * e;
            let banda = RectF {
                x: dentro.x as f32,
                y,
                ancho: dentro.ancho as f32,
                alto,
            };
            pintar_cita(p, tema, &a.mensajes[j], banda, color_texto, e);
            a.zonas.borrow_mut().push((
                Rect {
                    x: banda.x as i32,
                    y: banda.y as i32,
                    ancho: banda.ancho as u32,
                    alto: banda.alto as u32,
                },
                Zona::Ir(j),
            ));
            y += alto + hueco;
        }

        // Lo que se ve: el lienzo con lo dibujado, la tabla, o una foto con
        // pie. Sobre papel oscuro los lienzos, como sus miniaturas del movil.
        if let (Some(Some(vista)), Some((ancho_vista, alto_vista))) =
            (a.vistas.get(i), piezas.vista)
        {
            let hoja = RectF {
                x: dentro.x as f32,
                y,
                ancho: ancho_vista,
                alto: alto_vista,
            };
            match vista {
                Ojeada::Lienzo(l) => {
                    p.rellenar_redondeado(hoja, 6.0 * e, tema.papel);
                    pintar_lienzo(p, l, hoja, Some(previas));
                    // El numero, encima de la hoja: adjuntando varias paginas
                    // del mismo plano las miniaturas se parecen entre si y el
                    // pie hay que leerlo; en la esquina se ve de un golpe.
                    if m.clase == Some(pixpin_proyecto::cuaderno::Clase::Pagina)
                        && let Some(pagina) = m.pagina
                    {
                        let mut args = fluent_bundle::FluentArgs::new();
                        args.set("n", pagina + 1);
                        let rotulo = textos.t_args("chat-pagina-n", &args);
                        let (w, h) = p.medir_texto(&rotulo, HORA_TAM * e);
                        let chapa = RectF {
                            x: hoja.x + 6.0 * e,
                            y: hoja.y + 6.0 * e,
                            ancho: w + 12.0 * e,
                            alto: h + 4.0 * e,
                        };
                        p.rellenar_redondeado(chapa, 8.0 * e, con_alfa(hex(0x000000), 0.6));
                        p.texto(
                            &rotulo,
                            chapa.x + 6.0 * e,
                            chapa.y + 2.0 * e,
                            HORA_TAM * e,
                            hex(0xffffff),
                        );
                    }
                }
                Ojeada::Tabla(t) => {
                    p.rellenar_redondeado(hoja, 6.0 * e, tema.papel);
                    pintar_ojeada_tabla(p, tema, escala, t, hoja);
                }
                Ojeada::Foto { .. } => {
                    let recortada = p.empujar_recorte_redondeado(hoja, 4.0 * e);
                    pintar_foto(p, a, i, hoja, previas, tema, e);
                    if recortada {
                        p.soltar_recorte_redondeado();
                    }
                    let zona = boton_de_esquina(hoja, e);
                    pintar_boton_de_esquina(p, zona);
                    a.zonas.borrow_mut().push((zona, Zona::Abrir(i)));
                    punto_de_proyecto(
                        p,
                        RectF {
                            x: hoja.x + 8.0 * e,
                            y: hoja.y + 8.0 * e,
                            ancho: chat::FILA_PUNTO as f32 * e,
                            alto: chat::FILA_PUNTO as f32 * e,
                        },
                        !en_guardados,
                    );
                }
            }
            y += alto_vista + hueco;
        }

        // La fila: el boton redondo de 44, el punto verde, el nombre y la
        // pastilla de sacarlo a la pantalla (`FilaDeArchivo`).
        if let Some(f) = &piezas.fila {
            let fila = chat::fila_archivo(
                dentro.x,
                y as i32,
                f.ancho_texto.ceil() as u32,
                dentro.ancho,
                escala,
            );
            let circulo = rf(fila.circulo);
            p.rellenar_redondeado(circulo, circulo.ancho / 2.0, tema.circulo);
            icono_centrado(p, f.icono, fila.circulo, 24.0 * e, tema.circulo_icono);
            punto_de_proyecto(p, rf(fila.punto), !en_guardados);
            // Una nota de voz ensena su ONDA en el sitio del nombre, dibujada
            // de los picos que el movil anoto al grabarla. Sin picos no se
            // inventa ninguna: una onda de mentira mentiria sobre lo que se
            // dijo, y se queda la fila de siempre con el nombre y el tiempo.
            if !f.onda.is_empty() {
                pintar_onda(p, &f.onda, fila.texto, tema, &f.detalle, color_hora, e);
            } else {
                let nombre = match &a.renombrando_mensaje {
                    Some((n, escrito)) if *n == i => format!("{escrito}|"),
                    _ => f.nombre.clone(),
                };
                let (_, alto_nombre) = p.medir_texto(&nombre, FICHA_NOMBRE_TAM * e);
                let (_, alto_detalle) = if f.detalle.is_empty() {
                    (0.0, 0.0)
                } else {
                    p.medir_texto(&f.detalle, FICHA_DETALLE_TAM * e)
                };
                let alto_todo = alto_nombre + alto_detalle;
                let mut ty = fila.circulo.y as f32 + (fila.circulo.alto as f32 - alto_todo) / 2.0;
                p.texto_linea(
                    &nombre,
                    fila.texto.x as f32,
                    ty,
                    FICHA_NOMBRE_TAM * e,
                    fila.texto.ancho as f32 + 1.0,
                    color_texto,
                );
                ty += alto_nombre;
                if !f.detalle.is_empty() {
                    p.texto_linea(
                        &f.detalle,
                        fila.texto.x as f32,
                        ty,
                        FICHA_DETALLE_TAM * e,
                        fila.texto.ancho as f32 + 1.0,
                        con_alfa(color_hora, 0.8),
                    );
                }
            }
            // La pastilla: el color de la hora al 15,6 %, esquinas de 8 y el
            // icono de «abrir fuera» en ese mismo color.
            let abrir = rf(fila.abrir);
            p.rellenar_redondeado(
                abrir,
                chat::FILA_ABRIR_RADIO as f32 * e,
                con_alfa(color_hora, 0.156),
            );
            icono_centrado(p, &mi::OPEN_IN_NEW, fila.abrir, 16.0 * e, color_hora);
            a.zonas.borrow_mut().push((fila.abrir, Zona::Abrir(i)));
            y += chat::FILA_CIRCULO as f32 * e + hueco;
        }

        if let Some(v) = &piezas.viene {
            // Su aire de arriba hace de hueco con lo anterior, si lo hay.
            let encima = if y > dentro.y as f32 { y - hueco } else { y };
            let caja = pintar_viene_de(p, tema, textos, v, burbuja.x as f32, encima, e);
            a.zonas.borrow_mut().push((caja, Zona::VieneDe(i)));
            y += VIENE_ALTO * e + 2.0 * VIENE_AIRE_Y * e;
        }

        if !piezas.texto.is_empty() {
            // Buscando, lo encontrado va en negrita dentro de la propia
            // burbuja. El movil lo tine de fondo; aqui el pintor no sabe
            // pintar fondos por tramo, y la negrita es lo que ya se usa en el
            // panel de informacion: lo mismo en los dos sitios.
            p.parrafo(
                &piezas.texto,
                dentro.x as f32,
                y,
                h::TEXTO_TAM * e,
                dentro.ancho as f32,
                &negritas(&piezas.texto, &aguja),
                color_texto,
            );
        }

        // Abajo a la IZQUIERDA: la chapa del codigo, la etiqueta, la
        // chincheta si esta fijado y la hora, en ese orden, como el movil.
        let hora = pixpin_ui::chat::etiqueta_hora(pixpin_shell::entorno::a_local(m.cuando), ahora);
        let (_, alto_hora) = p.medir_texto(&hora, HORA_TAM * e);
        let y_hora =
            burbuja.abajo() as f32 - (h::RELLENO_Y - h::HORA_INVADE_Y) as f32 * e - alto_hora;
        let mut x = burbuja.x as f32 + h::RELLENO_X as f32 * e;
        if let Some(codigo) = chapa_de_codigo(m) {
            let tam = CHAPA_TAM * e;
            let (wc, hc) = p.medir_texto(&codigo, tam);
            let caja = RectF {
                x,
                y: y_hora + (alto_hora - hc) / 2.0 - 1.0 * e,
                ancho: wc + 8.0 * e,
                alto: hc + 2.0 * e,
            };
            p.rellenar_redondeado(caja, 6.0 * e, tema.filete);
            p.texto(&codigo, caja.x + 4.0 * e, caja.y + 1.0 * e, tam, color_hora);
            x += caja.ancho + 4.0 * e;
        }
        if let Some(emoji) = m.emoji.as_deref().filter(|s| !s.is_empty()) {
            let (w, _) = p.medir_texto(emoji, HORA_TAM * e);
            p.texto(emoji, x, y_hora, HORA_TAM * e, color_texto);
            x += w + 3.0 * e;
        }
        if m.fijado {
            let lado = HORA_TAM * e;
            p.icono(
                &mi::PUSH_PIN,
                RectF {
                    x,
                    y: y_hora + (alto_hora - lado) / 2.0,
                    ancho: lado,
                    alto: lado,
                },
                tema.enviar,
            );
            x += lado + 3.0 * e;
        }
        p.texto(&hora, x, y_hora, HORA_TAM * e, color_hora);
        pintar_flecha_de_comentar(p, tema, burbuja, corrida, escala, e);
    }
    p.soltar_recorte();
}

/// La onda de una nota de voz, en el sitio del nombre de su fila: las barras
/// arriba y la duracion debajo, como en el movil.
///
/// Se pinta barra a barra y no de un trazado cacheado como en el movil: aqui
/// no se reproduce nada, asi que no hay un borde de avance que pudiera saltar
/// de tres en tres; y cincuenta rectangulos por nota de voz a la vista no se
/// notan al lado de una foto.
#[allow(clippy::too_many_arguments)] // las barras, donde van, los dos colores y la escala
fn pintar_onda(
    p: &Pintor,
    onda: &[f32],
    sitio: Rect,
    tema: &Tema,
    duracion: &str,
    color_duracion: Color,
    e: f32,
) {
    let paso = pixpin_ui::chat::ONDA_PASO as f32 * e;
    let grueso = (pixpin_ui::chat::ONDA_GRUESO as f32 * e).max(1.0);
    let alto_max = pixpin_ui::chat::ONDA_ALTO as f32 * e;
    let (_, alto_duracion) = p.medir_texto(duracion, FICHA_DETALLE_TAM * e);
    // La onda y la duracion, juntas y centradas en la fila de 44.
    let alto_todo = alto_max + alto_duracion;
    let arriba = sitio.y as f32 + (sitio.alto as f32 - alto_todo) / 2.0;
    let eje = arriba + alto_max / 2.0;
    let tope = sitio.derecha() as f32;
    for (n, valor) in onda.iter().enumerate() {
        let x = sitio.x as f32 + n as f32 * paso;
        if x + grueso > tope {
            break;
        }
        let medio = (alto_max * valor.clamp(0.0, 1.0) / 2.0).max(0.5);
        p.rellenar_redondeado(
            RectF {
                x,
                y: eje - medio,
                ancho: grueso,
                alto: medio * 2.0,
            },
            grueso / 2.0,
            tema.enviar,
        );
    }
    if !duracion.is_empty() {
        p.texto_linea(
            duracion,
            sitio.x as f32,
            arriba + alto_max,
            FICHA_DETALLE_TAM * e,
            sitio.ancho as f32 + 1.0,
            con_alfa(color_duracion, 0.8),
        );
    }
}

/// La flecha que asoma a la IZQUIERDA de la burbuja mientras se arrastra a la
/// derecha para comentarla, cada vez mas opaca hasta el gatillo.
///
/// En el movil este aviso es una vibracion; en un ordenador no hay nada que
/// vibre, asi que lo dice la flecha: entera = soltar comenta.
fn pintar_flecha_de_comentar(
    p: &Pintor,
    tema: &Tema,
    burbuja: Rect,
    corrida: i32,
    escala: u32,
    e: f32,
) {
    if corrida <= 1 {
        return;
    }
    let lado = 20.0 * e;
    let opacidad = pixpin_ui::chat::opacidad_de_comentar(corrida, escala);
    p.icono(
        &mi::REPLY,
        RectF {
            x: burbuja.x as f32 - lado - 8.0 * e,
            y: burbuja.y as f32 + (burbuja.alto as f32 - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        con_alfa(tema.enviar, opacidad),
    );
}

/// Lo que hace un toque en una burbuja: abrir lo que lleve dentro.
///
/// Vive aqui fuera y no dentro del bucle porque ya no se hace al PULSAR sino
/// al SOLTAR: pulsar solo apunta el gesto, que puede acabar siendo un
/// arrastre a la derecha para comentar.
#[allow(clippy::too_many_arguments)] // es el contexto del bucle, que va entero
fn tocar_la_burbuja(
    indice: usize,
    ubicacion: &Ubicacion,
    abierto: &mut Option<Abierto>,
    fichas: &[pixpin_proyecto::almacen::Ficha],
    elegida: &mut Option<usize>,
    borradores: &mut std::collections::HashMap<String, String>,
    textos: &Catalogo,
    lienzo: OpcionesLienzo,
) {
    // Un proyecto adjunto es la puerta a ese proyecto: pulsarlo lo abre.
    let a_otro = abierto
        .as_ref()
        .and_then(|a| a.mensajes.get(indice))
        .filter(|m| m.clase == Some(pixpin_proyecto::cuaderno::Clase::Proyecto))
        .and_then(|m| m.referencia.clone())
        .and_then(|id| fichas.iter().position(|f| f.id == id));
    if let Some(i) = a_otro {
        if let Some(a) = abierto.as_mut() {
            cerrar_panel(ubicacion, a);
            apagar_lienzo(ubicacion, a);
        }
        if let Some(a) = abierto.take() {
            borradores.insert(a.ficha.id.clone(), a.borrador);
        }
        *elegida = Some(i);
        let mut nuevo = abrir_proyecto(ubicacion, &fichas[i]);
        nuevo.borrador = borradores.remove(&fichas[i].id).unwrap_or_default();
        *abierto = Some(nuevo);
        return;
    }
    // Una tabla se abre para escribir en ella, en el sitio del historial; lo
    // demas, con su aplicacion.
    let vista = |f: fn(&Option<Ojeada>) -> bool| {
        abierto
            .as_ref()
            .and_then(|a| a.vistas.get(indice))
            .is_some_and(f)
    };
    let es_tabla = vista(|v| matches!(v, Some(Ojeada::Tabla(_))));
    let es_foto = vista(|v| matches!(v, Some(Ojeada::Foto { .. })));
    // Un dibujo del movil trae cientos de trazos: en la burbuja no se lee
    // ninguno, asi que se abre grande.
    //
    // Se mira la CLASE del mensaje y no su ojeada: un lienzo recien creado
    // esta vacio, no tiene ojeada, y por eso no se abria nunca -ni se podia
    // empezar a dibujar en el-.
    let es_dibujo = abierto
        .as_ref()
        .and_then(|a| a.mensajes.get(indice))
        .is_some_and(|m| {
            m.clase == Some(pixpin_proyecto::cuaderno::Clase::Dibujo)
                && m.referencia.as_deref().is_some_and(|r| !r.is_empty())
        });
    // Una mini-app se abre en su panel, en el sitio del historial. Si no la
    // conocemos, `abrir_mini` dice que no y se sigue por el camino de
    // siempre: un documento de una version futura se abre con su aplicacion
    // o no se abre, pero nunca en un panel en blanco.
    let es_mini = abierto
        .as_ref()
        .and_then(|a| a.mensajes.get(indice))
        .is_some_and(|m| {
            m.clase == Some(pixpin_proyecto::cuaderno::Clase::MiniApp)
                && m.miniapp.as_deref() != Some(pixpin_proyecto::tabla::MINIAPP)
        });
    if es_mini
        && let Some(a) = abierto.as_mut()
        && abrir_mini(a, indice, textos)
    {
        return;
    }
    match abierto.as_mut() {
        Some(a) if es_tabla => abrir_hoja(a, indice),
        // Una foto se abre en el editor 2D, con la foto de fondo: es donde
        // estan las herramientas. Dibujar en la propia burbuja sigue estando,
        // en el menu del boton derecho.
        Some(a) if es_foto => {
            if let Some(ruta) = a
                .mensajes
                .get(indice)
                .and_then(|m| ruta_del_mensaje(&a.raiz, &a.ficha.id, m))
            {
                abrir_foto_en_lienzo(&ruta, lienzo);
                // Al volver, la burbuja tiene que ensenar lo que se dibujo.
                if let Some(m) = a.mensajes.get(indice).cloned() {
                    a.vistas[indice] = leer_vista(ubicacion, &a.ficha.id, &m);
                }
                a.colocado.borrow_mut().ancho = 0;
            }
        }
        Some(a) if es_dibujo => {
            for i in abrir_dibujo(ubicacion, a, indice, lienzo) {
                if let Some(m) = a.mensajes.get(i).cloned() {
                    a.vistas[i] = leer_vista(ubicacion, &a.ficha.id, &m);
                }
            }
            a.colocado.borrow_mut().ancho = 0;
        }
        Some(a) => abrir_mensaje(ubicacion, a, indice),
        None => {}
    }
}

// --- Unir al proyecto y volver a anadir -----------------------------------
//
// En el movil, lo que se guardo en la conversacion de una obra acaba siendo
// parte de la obra: una foto es una hoja, una nota es una nota y el PDF del
// cliente es el documento. `UnirAlProyecto.unir` crea una `Hoja` en el
// `proyecto.json` con `deMensaje` apuntando al mensaje, y le pone `unido` al
// mensaje para no ofrecerlo dos veces.
//
// Aqui hay una diferencia que AHORRA trabajo: el movil COPIA el fichero,
// porque su chat y sus proyectos viven en sitios distintos y borrar el
// mensaje dejaria al proyecto apuntando a lo que ya no esta. En Windows el
// chat de un proyecto y el proyecto SON la misma carpeta (`proyectos/<id>/`),
// asi que la hoja apunta a lo que ya esta ahi y no se copia nada.

/// Si un mensaje puede convertirse en una hoja del proyecto.
///
/// Las mismas clases que el movil: fotos, dibujos, notas y documentos. Una
/// nota de voz o una tabla no son una hoja de un plano.
fn se_puede_unir(m: &pixpin_proyecto::cuaderno::Mensaje) -> bool {
    use pixpin_proyecto::cuaderno::Clase;
    match m.clase.as_ref() {
        Some(Clase::Imagen) | Some(Clase::Dibujo) | Some(Clase::Pagina) => true,
        Some(Clase::Nota) => !m.texto.trim().is_empty(),
        Some(Clase::Archivo) => m.nombre.to_lowercase().ends_with(".pdf"),
        _ => false,
    }
}

/// Si el mensaje dice ya ser una hoja de su proyecto (`unido` del movil).
///
/// Vive en `resto` porque el `Mensaje` de aqui no declara ese campo: lo que
/// el movil anade se guarda tal cual y se devuelve tal cual.
fn ya_esta_unido(m: &pixpin_proyecto::cuaderno::Mensaje) -> bool {
    m.resto.get("unido").and_then(|v| v.as_bool()) == Some(true)
}

/// El `proyecto.json` de un proyecto, si lo tiene. Uno nacido aqui no lo
/// tiene todavia, y eso no es un error: se empieza uno vacio.
fn leer_proyecto_json(carpeta: &std::path::Path) -> pixpin_proyecto::Proyecto {
    std::fs::read_to_string(carpeta.join("proyecto.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Lo escribe entero, primero al lado y luego de un tiron: un corte a mitad
/// de escribir no puede dejar el proyecto sin sus hojas.
fn guardar_proyecto_json(
    carpeta: &std::path::Path,
    p: &pixpin_proyecto::Proyecto,
) -> std::io::Result<()> {
    let texto = serde_json::to_string(p).map_err(std::io::Error::other)?;
    std::fs::create_dir_all(carpeta)?;
    let temporal = carpeta.join("proyecto.json.tmp");
    std::fs::write(&temporal, texto)?;
    std::fs::rename(&temporal, carpeta.join("proyecto.json"))
}

/// La hoja que le corresponde a un mensaje, con su vinculo `deMensaje`.
fn hoja_de_mensaje(
    m: &pixpin_proyecto::cuaderno::Mensaje,
    textos: &Catalogo,
) -> pixpin_proyecto::Hoja {
    use pixpin_proyecto::cuaderno::Clase;
    let mut hoja = pixpin_proyecto::Hoja {
        // El id de la hoja es el del mensaje: son la misma cosa, y asi
        // reconocerla luego no depende de buscar por `deMensaje`.
        id: m.id.clone(),
        nombre: nombre_de_la_fila(m, textos),
        dibujo: m.referencia.clone().filter(|r| !r.is_empty()),
        pagina: m.pagina,
        ..Default::default()
    };
    if m.clase == Some(Clase::Nota) {
        hoja.nota = Some(m.texto.clone());
        hoja.dibujo = None;
    }
    hoja.uid = Some(m.codigo_unico());
    hoja.resto
        .insert("deMensaje".into(), serde_json::Value::String(m.id.clone()));
    hoja
}

/// Mete el mensaje en las hojas del proyecto de su propia conversacion.
///
/// No hay cuadro de eleccion, como en el movil dentro del chat de un
/// proyecto: el destino es el proyecto que se esta mirando.
fn unir_al_proyecto(ubicacion: &Ubicacion, a: &mut Abierto, i: usize, textos: &Catalogo) -> Efecto {
    let Some(m) = a.mensajes.get(i).cloned() else {
        return Efecto::Nada;
    };
    if !se_puede_unir(&m) {
        return Efecto::Aviso(textos.t("chat-unir-nada"));
    }
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    let mut proyecto = leer_proyecto_json(&carpeta);
    if proyecto.id.is_empty() {
        proyecto.id = a.ficha.id.clone();
        proyecto.nombre = a.ficha.nombre.clone();
    }
    let hoja = hoja_de_mensaje(&m, textos);
    // Volver a unir lo mismo no da la hoja dos veces: se pone al dia la que
    // hay. Es lo que hace «Volver a anadir» cuando la hoja sigue estando.
    match proyecto.hojas.iter_mut().find(|h| h.id == hoja.id) {
        Some(vieja) => *vieja = hoja,
        None => proyecto.hojas.push(hoja),
    }
    proyecto.tocado = pixpin_shell::entorno::ahora_utc_ms();
    // Y deja de estar entre las borradas, si alguien la quito antes.
    if let Some(borradas) = proyecto
        .resto
        .get_mut("borradas")
        .and_then(|v| v.as_object_mut())
    {
        borradas.remove(&m.id);
    }
    if let Err(e) = guardar_proyecto_json(&carpeta, &proyecto) {
        tracing::warn!(?e, "no se pudo guardar el proyecto al unir");
        return Efecto::Aviso(textos.t("chat-devolver-no"));
    }
    // El mensaje se marca para que el menu no vuelva a ofrecer unirlo.
    let mut puesto = m.clone();
    puesto
        .resto
        .insert("unido".into(), serde_json::Value::Bool(true));
    match pixpin_proyecto::cuaderno::reemplazar(&carpeta, &puesto) {
        Ok(_) => a.mensajes[i] = puesto,
        Err(e) => tracing::warn!(?e, "no se pudo marcar el mensaje como unido"),
    }
    a.colocado.borrow_mut().ancho = 0;
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("nombre", a.ficha.nombre.clone());
    Efecto::Aviso(textos.t_args("chat-unido-a", &args))
}

/// Devuelve al proyecto la hoja que se quito de el, con lo que se hizo
/// mientras estuvo fuera. Es unir otra vez, con otro aviso: el mensaje ya
/// dice `unido`, y lo que falta es la hoja.
fn devolver_al_proyecto(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    i: usize,
    textos: &Catalogo,
) -> Efecto {
    match unir_al_proyecto(ubicacion, a, i, textos) {
        Efecto::Aviso(_) if esta_en_las_hojas(a, i) => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("nombre", a.ficha.nombre.clone());
            Efecto::Aviso(textos.t_args("chat-devuelto", &args))
        }
        Efecto::Aviso(_) => Efecto::Aviso(textos.t("chat-devolver-no")),
        otro => otro,
    }
}

/// Si el mensaje esta AHORA MISMO entre las hojas del proyecto.
///
/// Se mira en el disco y no en algo guardado: la hoja puede haberla quitado
/// el movil en la ultima sincronizacion, y fiarse de `unido` diria que si
/// cuando ya no esta. Es lo que el movil llama `sePuedeDevolver`.
fn esta_en_las_hojas(a: &Abierto, i: usize) -> bool {
    let Some(m) = a.mensajes.get(i) else {
        return false;
    };
    let carpeta = pixpin_proyecto::almacen::carpeta(&a.raiz, &a.ficha.id);
    leer_proyecto_json(&carpeta)
        .hojas
        .iter()
        .any(|h| h.id == m.id || h.resto.get("deMensaje").and_then(|v| v.as_str()) == Some(&m.id))
}

/// Lo que hay que moverse para que el gesto deje de ser un toque y pase a
/// ser un arrastre. Sin este margen, un temblor de la mano al pulsar dejaria
/// la burbuja sin abrir.
const ARRASTRE_MINIMO: i32 = 4;

/// Cuantas burbujas se ven con la busqueda que haya puesta.
fn cuantas_se_ven(a: &Abierto) -> usize {
    a.mensajes.iter().filter(|m| se_ve(a, m)).count()
}

/// Que mensajes se ven, por su posicion. Sin busqueda ni chip, todos.
///
/// Se busca sobre el RESUMEN y no sobre `texto` a secas: en una nota de voz
/// el texto suele estar vacio y lo que se busca es lo que se dijo, que vive
/// en la transcripcion (ver `Mensaje::resumen`).
fn indices_visibles(a: &Abierto) -> Vec<usize> {
    a.mensajes
        .iter()
        .enumerate()
        .filter(|(_, m)| se_ve(a, m))
        .map(|(i, _)| i)
        .collect()
}

/// Si una burbuja pasa los dos filtros: la palabra de la lupa y el chip de
/// etiqueta. Los dos a la vez, como en el movil (`porEmoji(buscados, …)`):
/// primero lo escrito y luego el emoji, que es lo que espera quien pulsa un
/// chip teniendo ya media palabra escrita.
fn se_ve(a: &Abierto, m: &pixpin_proyecto::cuaderno::Mensaje) -> bool {
    if let Some(q) = a.busqueda.as_deref().filter(|q| !q.trim().is_empty())
        && !pixpin_ui::chat::casa_la_busqueda(&m.resumen(), &m.nombre, q)
    {
        return false;
    }
    match a.por_etiqueta.as_deref() {
        None => true,
        Some(e) => m.emoji.as_deref() == Some(e),
    }
}

/// Los chips de etiqueta que toca ensenar: solo con la lupa encendida, y
/// solo los que de verdad filtran algo.
fn chips_de(a: &Abierto) -> Vec<String> {
    if a.busqueda.is_none() {
        return Vec::new();
    }
    pixpin_ui::chat::emojis_usados(a.mensajes.iter().filter_map(|m| m.emoji.as_deref()))
}

/// El area de las burbujas, ya descontada la fila de chips si la hay.
fn area_del_historial(d: &Disposicion, a: &Abierto, escala: u32) -> Rect {
    d.historial(
        a.alto_caja.get(),
        a.fijado.is_some(),
        chips_de(a).len(),
        escala,
    )
}

/// Lo que hace falta para medir un mensaje: los textos, la escala y la hora
/// de ahora (la de hoy se escribe como hora y la vieja como fecha, y miden
/// distinto).
#[derive(Clone, Copy)]
struct Medir<'a> {
    textos: &'a Catalogo,
    e: f32,
    ahora: i64,
    /// Lo que se esta buscando, si se busca. Hace falta al MEDIR y no solo al
    /// pintar: lo encontrado va en negrita, la negrita ocupa mas, y midiendo
    /// sin ella la ultima linea se saldria de la burbuja.
    aguja: &'a str,
}

/// Mide un mensaje y decide de que piezas se compone, como la `Burbuja` del
/// movil: la cita, lo que se ve, la fila del archivo, «Viene de», el texto y
/// el renglon de la hora.
fn medir_mensaje(
    p: &Pintor,
    a: &Abierto,
    indice: usize,
    m: &pixpin_proyecto::cuaderno::Mensaje,
    ancho_contenido: u32,
    cx: Medir,
) -> (pixpin_ui::historial::Entrada, Piezas) {
    use pixpin_ui::historial as h;
    let (textos, e, ahora) = (cx.textos, cx.e, cx.ahora);
    let ancho_max = ancho_contenido as f32;
    let hueco = h::RELLENO_Y as f32 * e;
    let vista = a.vistas.get(indice).and_then(|v| v.as_ref());
    let es_foto = matches!(vista, Some(Ojeada::Foto { .. }));
    let fila = fila_de(m, es_foto, textos).map(|(icono, nombre, detalle)| {
        let (wn, _) = p.medir_texto(&nombre, FICHA_NOMBRE_TAM * e);
        let (wd, _) = p.medir_texto(&detalle, FICHA_DETALLE_TAM * e);
        let onda = if m.clase == Some(pixpin_proyecto::cuaderno::Clase::Voz) {
            pixpin_ui::chat::barras_de_onda(&picos_de_voz(m))
        } else {
            Vec::new()
        };
        // Con onda el sitio lo pide ella: la onda arriba y la duracion
        // debajo, como en el movil. Sin ella manda el texto, como en
        // cualquier otra fila.
        let ancho_texto = if onda.is_empty() {
            wn.max(wd)
        } else {
            let ancho_onda =
                pixpin_ui::chat::ancho_de_la_onda(onda.len(), (e * 100.0).round() as u32) as f32;
            ancho_onda.max(wd)
        };
        FilaDeArchivo {
            icono,
            nombre,
            detalle,
            ancho_texto,
            onda,
        }
    });
    let texto = texto_de(m, vista.is_some() || fila.is_some(), textos);
    let cita = m
        .responde_a
        .as_deref()
        .filter(|r| !r.is_empty())
        .and_then(|r| a.mensajes.iter().position(|o| o.id == r));
    let viene = viene_de(m).map(|(texto, _)| {
        let (wt, _) = p.medir_texto(&texto, VIENE_TEXTO_TAM * e);
        let (we, _) = p.medir_texto(&textos.t("chat-viene-de"), VIENE_ETIQUETA_TAM * e);
        let ancho = (2.0 * VIENE_AIRE_X + 8.0 + 18.0 + 6.0 + 6.0 + 16.0 + 10.0) * e + wt.max(we);
        Viene {
            texto,
            ancho: ancho.min(ancho_max),
        }
    });
    let foto_sola = es_foto && texto.is_empty() && cita.is_none();
    let vista_medida = vista.map(|v| {
        let vivo = a.vivo.as_ref().is_some_and(|v| v.indice == indice);
        tamano_de_vista(v, ancho_max, e, if vivo { VIVO_FACTOR } else { 1.0 })
    });
    let borde = m
        .referencia
        .as_deref()
        .and_then(pixpin_ui::chat::color_del_borde)
        .map(hex);

    let (mut ancho, mut alto) = (0.0f32, 0.0f32);
    // Una pieza debajo de la otra, con el hueco de siempre entre las dos.
    let sumar = |ancho: &mut f32, alto: &mut f32, w: f32, alto_pieza: f32| {
        *ancho = ancho.max(w.min(ancho_max));
        if *alto > 0.0 {
            *alto += hueco;
        }
        *alto += alto_pieza;
    };
    if foto_sola {
        let (w, alto_vista) = vista_medida.unwrap_or((0.0, 0.0));
        let extra = viene
            .as_ref()
            .map_or(0.0, |_| VIENE_ALTO * e + 2.0 * VIENE_AIRE_Y * e);
        let relleno_x = 2.0 * h::RELLENO_X as f32 * e;
        let relleno_y = 2.0 * h::RELLENO_Y as f32 * e;
        let w = w.max(viene.as_ref().map_or(0.0, |v| v.ancho));
        let entrada = h::Entrada {
            ancho: (w - relleno_x).max(0.0).ceil() as u32,
            alto: (alto_vista + extra - relleno_y).max(0.0).ceil() as u32,
            mio: false,
            cuando: m.cuando,
            dia: pixpin_shell::entorno::a_local(m.cuando).div_euclid(86_400_000),
        };
        let piezas = Piezas {
            foto_sola,
            cita,
            vista: vista_medida,
            fila: None,
            viene,
            texto,
            borde,
        };
        return (entrada, piezas);
    }
    if cita.is_some() {
        sumar(
            &mut ancho,
            &mut alto,
            (240.0 * e).min(ancho_max),
            CITA_ALTO * e,
        );
    }
    if let Some((w, alto_vista)) = vista_medida {
        sumar(&mut ancho, &mut alto, w, alto_vista);
    }
    if let Some(f) = &fila {
        let w = pixpin_ui::chat::ancho_fila_archivo(
            f.ancho_texto.ceil() as u32,
            (e * 100.0).round() as u32,
        );
        sumar(
            &mut ancho,
            &mut alto,
            w as f32,
            pixpin_ui::chat::FILA_CIRCULO as f32 * e,
        );
    }
    if let Some(v) = &viene {
        // Su aire de arriba ya hace de hueco con lo anterior.
        let antes = alto;
        sumar(
            &mut ancho,
            &mut alto,
            v.ancho,
            VIENE_ALTO * e + 2.0 * VIENE_AIRE_Y * e,
        );
        if antes > 0.0 {
            alto -= hueco;
        }
    }
    if !texto.is_empty() {
        let tramos = negritas(&texto, cx.aguja);
        let (w, alto_texto) = if tramos.is_empty() {
            p.medir_texto_ajustado(&texto, h::TEXTO_TAM * e, ancho_max)
        } else {
            p.medir_parrafo(&texto, h::TEXTO_TAM * e, ancho_max, &tramos)
        };
        sumar(&mut ancho, &mut alto, w, alto_texto);
    }
    // El renglon de abajo: la chapa, la etiqueta, la chincheta y la hora.
    let hora = pixpin_ui::chat::etiqueta_hora(pixpin_shell::entorno::a_local(m.cuando), ahora);
    let (ancho_hora, alto_hora) = p.medir_texto(&hora, HORA_TAM * e);
    let mut renglon = ancho_hora;
    if let Some(codigo) = chapa_de_codigo(m) {
        renglon += p.medir_texto(&codigo, CHAPA_TAM * e).0 + 12.0 * e;
    }
    if let Some(emoji) = m.emoji.as_deref().filter(|s| !s.is_empty()) {
        renglon += p.medir_texto(emoji, HORA_TAM * e).0 + 3.0 * e;
    }
    if m.fijado {
        renglon += (HORA_TAM + 3.0) * e;
    }
    ancho = ancho.max(renglon.min(ancho_max));
    alto += alto_hora + 4.0 * e;
    let entrada = h::Entrada {
        alto: alto.ceil() as u32,
        ancho: ancho.ceil() as u32,
        // Todas a la izquierda, como en el movil: es un cuaderno propio, no
        // una conversacion entre dos, y alternar lados no decia nada.
        mio: false,
        cuando: m.cuando,
        dia: pixpin_shell::entorno::a_local(m.cuando).div_euclid(86_400_000),
    };
    let piezas = Piezas {
        foto_sola,
        cita,
        vista: vista_medida,
        fila,
        viene,
        texto,
        borde,
    };
    (entrada, piezas)
}

/// La cabecera del proyecto, como la del movil: el circulo de volver, la
/// pastilla del titulo (carpeta amarilla, nombre y «N cosas · tamano») y la
/// de la lupa y los tres puntos. Con mensajes elegidos, la barra de la
/// seleccion en su lugar (`TopAppBar` en modo seleccion).
fn pintar_cabecera(p: &Pintor, d: &Disposicion, c: &Pinta, a: &Abierto) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let cab = d.cabecera_chat;
    if cab.ancho == 0 || cab.alto == 0 {
        return;
    }
    let mut zonas = a.zonas.borrow_mut();
    if !a.marcados.is_empty() {
        // La barra se SUSTITUYE, no se anade otra: mientras se eligen cosas
        // el titulo y la lupa no sirven de nada.
        p.rellenar(rf(cab), tema.cabecera);
        let lado = (chat::PILDORA * escala / 100).min(cab.alto);
        let y = cab.y + (cab.alto as i32 - lado as i32) / 2;
        let cerrar = Rect {
            x: cab.x + (6.0 * e) as i32,
            y,
            ancho: lado,
            alto: lado,
        };
        icono_centrado(p, &mi::CLOSE, cerrar, 24.0 * e, tema.texto);
        zonas.push((cerrar, Zona::SelCerrar));
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("n", a.marcados.len());
        let titulo = textos.t_args("chat-elegidos", &args);
        let (_, alto_titulo) = p.medir_texto(&titulo, 16.0 * e);
        p.texto(
            &titulo,
            cerrar.derecha() as f32 + 12.0 * e,
            cab.y as f32 + (cab.alto as f32 - alto_titulo) / 2.0,
            16.0 * e,
            tema.texto,
        );
        // Solo lo que es de bloque, de derecha a izquierda: borrar el ultimo,
        // lejos de lo demas. Copiar solo si hay texto que copiar.
        let hay_texto = a.mensajes.iter().any(|m| {
            a.marcados.contains(&m.id)
                && m.clase != Some(pixpin_proyecto::cuaderno::Clase::MiniApp)
                && !m.texto.trim().is_empty()
        });
        let mut botones: Vec<(&'static Icono, Zona)> = vec![
            (&mi::DELETE, Zona::SelBorrar),
            (&mi::SEND, Zona::SelReenviar),
            (&mi::PUSH_PIN, Zona::SelFijar),
        ];
        if hay_texto {
            botones.push((&mi::CONTENT_COPY, Zona::SelCopiar));
        }
        let mut x = cab.derecha() - (6.0 * e) as i32;
        for (icono, zona) in botones {
            x -= lado as i32;
            let r = Rect {
                x,
                y,
                ancho: lado,
                alto: lado,
            };
            icono_centrado(p, icono, r, 24.0 * e, tema.texto);
            zonas.push((r, zona));
        }
        return;
    }

    let pil = d.pildoras(escala);
    // Volver: un circulo de 46 con la flecha.
    pildora(p, tema, rf(pil.volver), e);
    icono_centrado(p, &mi::ARROW_BACK, pil.volver, 24.0 * e, tema.pildora_texto);
    zonas.push((pil.volver, Zona::Volver));
    // El universo, la lupa y los tres puntos, en una sola pastilla.
    pildora(p, tema, rf(pil.derecha), e);
    icono_centrado(p, &mi::PUBLIC, pil.universo, 24.0 * e, tema.pildora_texto);
    // La lupa enciende y apaga, como en el movil: el mismo boton, con aspa
    // mientras se busca.
    let icono_lupa: &Icono = if a.busqueda.is_some() {
        &mi::CLOSE
    } else {
        &mi::SEARCH
    };
    icono_centrado(p, icono_lupa, pil.buscar, 24.0 * e, tema.pildora_texto);
    icono_centrado(p, &mi::MORE_VERT, pil.menu, 24.0 * e, tema.pildora_texto);
    zonas.push((pil.universo, Zona::Universo));
    zonas.push((pil.buscar, Zona::Buscar));
    zonas.push((pil.menu, Zona::Menu));

    // Buscando, la caja de buscar SUSTITUYE al titulo dentro de la pastilla
    // del centro, como en el movil: una caja debajo empujaria la conversacion
    // hacia abajo cada vez que se abre la lupa.
    if let Some(aguja) = a.busqueda.as_deref() {
        let sitio = Rect {
            y: cab.y + (2.0 * e) as i32,
            alto: cab.alto.saturating_sub((4.0 * e) as u32),
            ..pil.centro
        };
        // Se queda con TODO el sitio del centro: se escribe dentro, y una
        // pastilla medida al texto iria creciendo letra a letra.
        if sitio.ancho == 0 {
            return;
        }
        pildora(p, tema, rf(sitio), e);
        let lupa = 18.0 * e;
        let x0 = sitio.x as f32 + 14.0 * e;
        p.icono(
            &mi::SEARCH,
            RectF {
                x: x0,
                y: sitio.y as f32 + (sitio.alto as f32 - lupa) / 2.0,
                ancho: lupa,
                alto: lupa,
            },
            tema.pildora_sub,
        );
        let (escrito, color) = if aguja.is_empty() {
            (textos.t("chat-buscar-aqui"), tema.pildora_sub)
        } else {
            (format!("{aguja}|"), tema.pildora_texto)
        };
        let tam = 15.0 * e;
        let (_, alto_escrito) = p.medir_texto(&escrito, tam);
        let tx = x0 + lupa + 8.0 * e;
        p.texto_linea(
            &escrito,
            tx,
            sitio.y as f32 + (sitio.alto as f32 - alto_escrito) / 2.0,
            tam,
            (sitio.derecha() as f32 - 14.0 * e - tx).max(0.0),
            color,
        );
        return;
    }

    // El titulo: disco amarillo con la carpeta (el marcador en «Mensajes
    // guardados»), el nombre con su flechita y debajo cuantas cosas guarda y
    // cuanto ocupan, que es lo que se pregunta de un cajon de documentos.
    let (nombre, color_nombre) = if a.renombrando && a.ficha.nombre.is_empty() {
        (textos.t("chat-nombre-nuevo"), tema.pildora_sub)
    } else if a.renombrando {
        (format!("{}|", a.ficha.nombre), tema.pildora_texto)
    } else {
        (a.ficha.nombre.clone(), tema.pildora_texto)
    };
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("n", a.mensajes.len());
    let mut sub = textos.t_args("chat-cosas", &args);
    let bytes: i64 = a.mensajes.iter().map(|m| m.bytes.max(0)).sum();
    if bytes > 0 {
        sub = format!("{sub} · {}", pixpin_ui::chat::tamano_corto(bytes as u64));
    }
    if a.rotas > 0 {
        // Lo que no se pudo leer se dice, no se calla.
        sub.push_str(&format!(" · {} ?", a.rotas));
    }
    let (tam_nombre, tam_sub) = (15.0 * e, 12.0 * e);
    let disco = 28.0 * e;
    let flecha = 18.0 * e;
    let (w_nombre, alto_nombre) = p.medir_texto(&nombre, tam_nombre);
    let (w_sub, _) = p.medir_texto(&sub, tam_sub);
    let contenido = (disco + 10.0 * e + w_nombre + 2.0 * e + flecha).max(w_sub);
    let sitio = Rect {
        y: cab.y + (2.0 * e) as i32,
        alto: cab.alto.saturating_sub((4.0 * e) as u32),
        ..pil.centro
    };
    let caja = pixpin_ui::chat::pildora_centro(sitio, contenido.ceil() as u32, escala);
    if caja.ancho == 0 {
        return;
    }
    pildora(p, tema, rf(caja), e);
    let x0 = caja.x as f32 + 14.0 * e;
    let y0 = caja.y as f32 + 4.0 * e;
    let circulo = RectF {
        x: x0,
        y: y0,
        ancho: disco,
        alto: disco,
    };
    p.rellenar_redondeado(circulo, disco / 2.0, tema.carpeta);
    let icono: &Icono = if a.ficha.es_guardados() {
        &mi::BOOKMARK_BORDER
    } else {
        &mi::FOLDER
    };
    let lado = 17.0 * e;
    p.icono(
        icono,
        RectF {
            x: x0 + (disco - lado) / 2.0,
            y: y0 + (disco - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        tema.carpeta_icono,
    );
    let tx = x0 + disco + 10.0 * e;
    let ancho_nombre = (caja.derecha() as f32 - 14.0 * e - tx - flecha - 2.0 * e).max(0.0);
    p.texto_linea(
        &nombre,
        tx,
        y0 + (disco - alto_nombre) / 2.0,
        tam_nombre,
        ancho_nombre,
        color_nombre,
    );
    p.icono(
        &mi::KEYBOARD_ARROW_DOWN,
        RectF {
            x: tx + w_nombre.min(ancho_nombre) + 2.0 * e,
            y: y0 + (disco - flecha) / 2.0,
            ancho: flecha,
            alto: flecha,
        },
        tema.pildora_texto,
    );
    p.texto_linea(
        &sub,
        x0,
        y0 + disco,
        tam_sub,
        (caja.ancho as f32 - 28.0 * e).max(0.0),
        tema.pildora_sub,
    );
    zonas.push((caja, Zona::Titulo));
}

/// Una pastilla de la cabecera: opaca y con su filete, sin sombra, como la
/// del movil cuando no desenfoca (`fondoDeLaPildora`).
fn pildora(p: &Pintor, tema: &Tema, r: RectF, e: f32) {
    let radio = r.alto.min(r.ancho) / 2.0;
    let filete = (0.55 * e).max(1.0);
    p.rellenar_redondeado(r, radio, tema.pildora_borde);
    p.rellenar_redondeado(encoger(r, filete), (radio - filete).max(0.0), tema.pildora);
}

/// Un icono de `lado` centrado en `caja`.
fn icono_centrado(p: &Pintor, icono: &Icono, caja: Rect, lado: f32, color: Color) {
    p.icono(
        icono,
        RectF {
            x: caja.x as f32 + (caja.ancho as f32 - lado) / 2.0,
            y: caja.y as f32 + (caja.alto as f32 - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        color,
    );
}

/// El punto de si vive en un proyecto: verde, o rojo si no (el chat de
/// «Mensajes guardados» es el general, fuera de los proyectos), con un halo
/// blanco para que se lea sobre la burbuja y sobre una foto
/// (`PuntoDeProyecto`).
fn punto_de_proyecto(p: &Pintor, r: RectF, en_un_proyecto: bool) {
    p.rellenar_redondeado(r, r.ancho / 2.0, Color::BLANCO);
    let dentro = encoger(r, r.ancho / 6.0);
    p.rellenar_redondeado(
        dentro,
        dentro.ancho / 2.0,
        if en_un_proyecto {
            hex(0x2e9e4f)
        } else {
            hex(0xd24b3e)
        },
    );
}

/// El boton de sacar a la pantalla en la esquina de una foto: arriba a la
/// derecha, a 6 del borde, un disco de 30 (`AtajoEnLaEsquina`).
fn boton_de_esquina(foto: RectF, e: f32) -> Rect {
    let lado = 30.0 * e;
    Rect {
        x: (foto.x + foto.ancho - 6.0 * e - lado) as i32,
        y: (foto.y + 6.0 * e) as i32,
        ancho: lado as u32,
        alto: lado as u32,
    }
}

/// Oscuro y traslucido con el icono en blanco: es lo que se lee sobre
/// cualquier foto, negra, blanca o un plano lleno de lineas.
fn pintar_boton_de_esquina(p: &Pintor, r: Rect) {
    let caja = rf(r);
    p.rellenar_redondeado(caja, caja.ancho / 2.0, con_alfa(hex(0x000000), 0.42));
    icono_centrado(p, &mi::OPEN_IN_NEW, r, caja.ancho * 0.57, Color::BLANCO);
}

/// La chapa y la hora encima de una foto sola: una pastilla negra al 60 %,
/// radio 8, a 6 del borde de abajo a la derecha (`Miniatura`).
fn pintar_hora_sobre_foto(
    p: &Pintor,
    m: &pixpin_proyecto::cuaderno::Mensaje,
    foto: RectF,
    ahora: i64,
    e: f32,
) {
    let hora = pixpin_ui::chat::etiqueta_hora(pixpin_shell::entorno::a_local(m.cuando), ahora);
    let tam = HORA_TAM * e;
    let codigo = chapa_de_codigo(m);
    let emoji = m.emoji.clone().filter(|s| !s.is_empty());
    let (w_hora, alto) = p.medir_texto(&hora, tam);
    let w_codigo = codigo
        .as_ref()
        .map_or(0.0, |c| p.medir_texto(c, tam).0 + 4.0 * e);
    let w_emoji = emoji
        .as_ref()
        .map_or(0.0, |s| p.medir_texto(s, tam).0 + 3.0 * e);
    let w_pin = if m.fijado { tam + 3.0 * e } else { 0.0 };
    let ancho = w_emoji + w_pin + w_codigo + w_hora + 12.0 * e;
    let caja = RectF {
        x: foto.x + foto.ancho - 6.0 * e - ancho,
        y: foto.y + foto.alto - 6.0 * e - (alto + 4.0 * e),
        ancho,
        alto: alto + 4.0 * e,
    };
    p.rellenar_redondeado(caja, 8.0 * e, con_alfa(hex(0x000000), 0.6));
    let mut x = caja.x + 6.0 * e;
    let y = caja.y + 2.0 * e;
    if let Some(s) = &emoji {
        p.texto(s, x, y, tam, Color::BLANCO);
        x += w_emoji;
    }
    if m.fijado {
        p.icono(
            &mi::PUSH_PIN,
            RectF {
                x,
                y: y + (alto - tam) / 2.0,
                ancho: tam,
                alto: tam,
            },
            Color::BLANCO,
        );
        x += w_pin;
    }
    if let Some(c) = &codigo {
        p.texto(c, x, y, tam, con_alfa(Color::BLANCO, 0.85));
        x += w_codigo;
    }
    p.texto(&hora, x, y, tam, Color::BLANCO);
}

/// La tarjeta «Viene de»: marron, con el enlace a la izquierda, el rotulo
/// pequeno, de donde viene en grande y la flecha de ir (`vieneDe` del
/// movil). Devuelve donde quedo, para poder pulsarla.
fn pintar_viene_de(
    p: &Pintor,
    tema: &Tema,
    textos: &Catalogo,
    v: &Viene,
    x_burbuja: f32,
    y_encima: f32,
    e: f32,
) -> Rect {
    let caja = RectF {
        x: x_burbuja + VIENE_AIRE_X * e,
        y: y_encima + VIENE_AIRE_Y * e,
        ancho: (v.ancho - 2.0 * VIENE_AIRE_X * e).max(1.0),
        alto: VIENE_ALTO * e,
    };
    p.rellenar_redondeado(caja, 12.0 * e, tema.circulo);
    let icono = 18.0 * e;
    p.icono(
        &mi::LINK,
        RectF {
            x: caja.x + 8.0 * e,
            y: caja.y + (caja.alto - icono) / 2.0,
            ancho: icono,
            alto: icono,
        },
        tema.circulo_icono,
    );
    let flecha = 16.0 * e;
    p.icono(
        &mi::ARROW_FORWARD,
        RectF {
            x: caja.x + caja.ancho - 10.0 * e - flecha,
            y: caja.y + (caja.alto - flecha) / 2.0,
            ancho: flecha,
            alto: flecha,
        },
        tema.circulo_icono,
    );
    let tx = caja.x + (8.0 + 18.0 + 6.0) * e;
    let ancho_texto = (caja.ancho - (8.0 + 18.0 + 6.0 + 6.0 + 16.0 + 10.0) * e).max(0.0);
    let etiqueta = textos.t("chat-viene-de");
    let (_, alto_etiqueta) = p.medir_texto(&etiqueta, VIENE_ETIQUETA_TAM * e);
    let (_, alto_texto) = p.medir_texto(&v.texto, VIENE_TEXTO_TAM * e);
    let y = caja.y + (caja.alto - alto_etiqueta - alto_texto) / 2.0;
    p.texto_linea(
        &etiqueta,
        tx,
        y,
        VIENE_ETIQUETA_TAM * e,
        ancho_texto,
        tema.viene_etiqueta,
    );
    p.texto_linea(
        &v.texto,
        tx,
        y + alto_etiqueta,
        VIENE_TEXTO_TAM * e,
        ancho_texto,
        tema.circulo_icono,
    );
    Rect {
        x: caja.x as i32,
        y: caja.y as i32,
        ancho: caja.ancho as u32,
        alto: caja.alto as u32,
    }
}

/// La cita de una respuesta: barra de 3 del color de acento y su fondo al
/// 10 %, con el numero, el nombre y el resumen del citado (`ReplyMessageLine`).
fn pintar_cita(
    p: &Pintor,
    tema: &Tema,
    citado: &pixpin_proyecto::cuaderno::Mensaje,
    banda: RectF,
    color_texto: Color,
    e: f32,
) {
    p.rellenar_redondeado(banda, 4.0 * e, con_alfa(tema.enviar, 0.10));
    p.rellenar(
        RectF {
            ancho: 3.0 * e,
            ..banda
        },
        tema.enviar,
    );
    let x = banda.x + 11.0 * e;
    let ancho = (banda.ancho - 19.0 * e).max(0.0);
    let arriba = if citado.numero > 0 {
        format!("#{} {}", citado.numero, citado.nombre.trim())
    } else {
        citado.nombre.trim().to_string()
    };
    let (_, alto_arriba) = p.medir_texto(&arriba, 13.0 * e);
    p.texto_linea(&arriba, x, banda.y + 5.0 * e, 13.0 * e, ancho, tema.enviar);
    p.texto_linea(
        &citado.resumen(),
        x,
        banda.y + 5.0 * e + alto_arriba,
        13.0 * e,
        ancho,
        color_texto,
    );
}

/// Pinta una foto del proyecto en `hoja`: la quieta, con lo dibujado
/// encima, o la del lienzo vivo.
fn pintar_foto(
    p: &Pintor,
    a: &Abierto,
    i: usize,
    hoja: RectF,
    previas: &crate::miniaturas::Miniaturas,
    tema: &Tema,
    e: f32,
) {
    let (Some(Some(Ojeada::Foto { doc, dibujo })), Some(m)) = (a.vistas.get(i), a.mensajes.get(i))
    else {
        return;
    };
    let foto = ruta_del_mensaje(&a.raiz, &a.ficha.id, m).and_then(|ruta| previas.ya(&ruta));
    match a.vivo.as_ref().filter(|v| v.indice == i) {
        // El lienzo vivo: la foto ENTERA (no recortada, o se dibujaria sobre
        // un trozo que no se ve) y encima lo dibujado. Su caja en pantalla se
        // apunta aqui, que es donde se sabe, y con ella se traduce el raton.
        Some(v) => {
            let (dw, dh) = v.doc;
            let escala = (hoja.ancho / dw.max(1.0)).min(hoja.alto / dh.max(1.0));
            let dest = RectF {
                x: hoja.x + (hoja.ancho - dw * escala) / 2.0,
                y: hoja.y + (hoja.alto - dh * escala) / 2.0,
                ancho: dw * escala,
                alto: dh * escala,
            };
            v.destino.set(dest);
            if let Some((b, _, _)) = foto {
                p.bitmap_con(b, dest, None, pixpin_render::Interpolacion::Lineal);
            }
            pintar_lienzo(
                p,
                &LienzoVisto {
                    ordenes: pixpin_motor2d::ordenes_de_escena(&v.escena),
                    caja: (0.0, 0.0, dw, dh),
                    fondo: None,
                },
                dest,
                None,
            );
            // Un borde que diga cual esta vivo: sin el, dos fotos seguidas se
            // ven igual y no se sabe en cual va a caer el trazo.
            let g = (2.0 * e).max(2.0);
            for lado in [
                RectF { alto: g, ..dest },
                RectF {
                    y: dest.y + dest.alto - g,
                    alto: g,
                    ..dest
                },
                RectF { ancho: g, ..dest },
                RectF {
                    x: dest.x + dest.ancho - g,
                    ancho: g,
                    ..dest
                },
            ] {
                p.rellenar(lado, tema.enviar);
            }
        }
        // Quieta: la foto y, encima, lo que se haya dibujado en ella. Entera
        // y no recortada, porque los trazos van en coordenadas de la foto.
        None => {
            let (dw, dh) = *doc;
            let encaje = (hoja.ancho / dw.max(1.0)).min(hoja.alto / dh.max(1.0));
            let dest = RectF {
                x: hoja.x + (hoja.ancho - dw * encaje) / 2.0,
                y: hoja.y + (hoja.alto - dh * encaje) / 2.0,
                ancho: dw * encaje,
                alto: dh * encaje,
            };
            if let Some((b, _, _)) = foto {
                p.bitmap_con(b, dest, None, pixpin_render::Interpolacion::Lineal);
            }
            if !dibujo.is_empty() {
                pintar_lienzo(
                    p,
                    &LienzoVisto {
                        ordenes: dibujo.clone(),
                        caja: (0.0, 0.0, dw, dh),
                        fondo: None,
                    },
                    dest,
                    None,
                );
            }
        }
    }
}

/// Abre un proyecto: lee su cuaderno de disco.
///
/// Que no haya cuaderno no es un fallo: un proyecto recien llegado del movil
/// todavia no tiene ninguno. Se ensena vacio y ya esta.
fn abrir_proyecto(ubicacion: &Ubicacion, ficha: &pixpin_proyecto::almacen::Ficha) -> Abierto {
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &ficha.id);
    // Un proyecto que entro con una version anterior puede haberse quedado
    // sin las paginas del PDF que no llevaban dibujo: se completan aqui, que
    // es cuando se abre, en vez de obligar a pedirselo otra vez al movil.
    let aparato = ficha.aparato.clone().unwrap_or_default();
    match pixpin_proyecto::almacen::completar_hojas(ubicacion.raiz(), &ficha.id, &aparato) {
        Ok(0) => {}
        Ok(hechas) => tracing::info!(hechas, proyecto = %ficha.nombre, "hojas que faltaban"),
        Err(e) => tracing::warn!(?e, "no se pudieron completar las hojas"),
    }
    let cuaderno = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta).unwrap_or_default();
    if cuaderno.lineas_rotas > 0 {
        tracing::warn!(
            proyecto = %ficha.nombre,
            rotas = cuaderno.lineas_rotas,
            "lineas del cuaderno que no se entendieron"
        );
    }
    let mut mensajes = cuaderno.mensajes;
    // Lo mas viejo arriba: el fichero se escribe anadiendo, pero un cuaderno
    // que viajo entre aparatos puede venir con las lineas mezcladas.
    mensajes.sort_by_key(|m| m.cuando);
    // El ultimo fijado manda, como en Android: fijar otro sustituye al
    // anterior en la barra.
    let fijado = mensajes.iter().rposition(|m| m.fijado);
    let vistas = mensajes
        .iter()
        .map(|m| leer_vista(ubicacion, &ficha.id, m))
        .collect();
    Abierto {
        ficha: ficha.clone(),
        raiz: ubicacion.raiz().to_path_buf(),
        mensajes,
        vistas,
        rotas: cuaderno.lineas_rotas,
        fijado,
        borrador: String::new(),
        info: None,
        scroll_info: 0,
        busqueda_info: String::new(),
        renombrando: false,
        buscando_info: false,
        alto_info: std::cell::Cell::new(0),
        anchos_pestanas: std::cell::RefCell::new(Vec::new()),
        respondiendo: None,
        renombrando_mensaje: None,
        marcados: Default::default(),
        zonas: std::cell::RefCell::new(Vec::new()),
        ir_a: None,
        resaltado: None,
        alto_caja: std::cell::Cell::new(0),
        scroll: None,
        alto: std::cell::Cell::new(0),
        colocado: std::cell::RefCell::new(Colocado::default()),
        hoja: None,
        mini: None,
        vivo: None,
        busqueda: None,
        por_etiqueta: None,
        comentando: None,
        barriendo: false,
    }
}

/// Guarda lo escrito como una nota del cuaderno y lo mete en el historial.
///
/// El orden importa: primero al disco y solo si eso sale bien se ensena. Al
/// reves, un fallo de escritura dejaria en pantalla un mensaje que no
/// existe, y el usuario creeria que lo tiene guardado.
fn guardar_nota(ubicacion: &Ubicacion, a: &mut Abierto, aparato: &str) -> std::io::Result<()> {
    use pixpin_proyecto::cuaderno;
    let texto = a.borrador.trim().to_string();
    let cuando = pixpin_shell::entorno::ahora_utc_ms();
    // El numero sigue al mayor que ya hay, que es lo que hace el codigo de
    // chat (`47·K7Q2`) unico dentro de la conversacion.
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let mut mensaje = cuaderno::Mensaje::nota(
        &texto,
        &cuaderno::Sello {
            cuando,
            numero,
            aparato: aparato.to_string(),
            proyecto: a.ficha.id.clone(),
        },
    );
    // Si se estaba contestando a algo, la nota queda colgada de ello: es lo
    // que convierte una lista de cosas en una conversacion.
    mensaje.responde_a = a.respondiendo.clone();
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    cuaderno::anadir(&carpeta, &mensaje)?;

    a.respondiendo = None;
    a.vistas.push(None);
    a.mensajes.push(mensaje);
    a.borrador.clear();
    // La ficha de la lista sube al momento: es la misma conversacion.
    a.ficha.tocado = cuando;
    a.ficha.resumen = texto;
    let mut indice = pixpin_proyecto::almacen::Indice::leer(ubicacion.raiz());
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == a.ficha.id) {
        f.tocado = a.ficha.tocado;
        f.resumen = a.ficha.resumen.clone();
        indice.guardar(ubicacion.raiz())?;
    }
    Ok(())
}

/// Mete en el proyecto lo que haya en el portapapeles. Devuelve cuantos
/// mensajes salieron de ahi (cero si solo era texto, que va al borrador).
///
/// Los ficheros se COPIAN dentro del proyecto. Apuntar al original seria mas
/// barato y estaria mal: el original se mueve, se renombra o se borra, y un
/// proyecto que viaja al movil no puede llevar rutas del escritorio de nadie.
fn pegar(ubicacion: &Ubicacion, a: &mut Abierto, aparato: &str) -> std::io::Result<usize> {
    use pixpin_codec::ContenidoPortapapeles as Que;
    let Some(que) = pixpin_codec::portapapeles::leer() else {
        return Ok(0);
    };
    let ficheros: Vec<(String, Vec<u8>)> = match que {
        Que::Texto(t) => {
            // El texto va a la caja, no al cuaderno: pegar no es enviar, y
            // asi se puede retocar antes.
            a.borrador.push_str(&t);
            return Ok(0);
        }
        Que::Imagen(imagen) => {
            let bytes = pixpin_codec::codificar_png(&imagen).map_err(std::io::Error::other)?;
            // Una imagen pegada no tiene nombre; se le pone la hora, que es
            // lo unico verdadero que se sabe de ella.
            let cuando = pixpin_shell::entorno::ahora_utc_ms();
            vec![(format!("pegado-{cuando}.png"), bytes)]
        }
        Que::Rutas(rutas) => leer_ficheros(&rutas),
    };

    let mut hechos = 0;
    for (nombre, bytes) in ficheros {
        if let Err(e) = adjuntar(ubicacion, a, aparato, &nombre, &bytes) {
            tracing::warn!(?e, nombre, "fichero que no se pudo guardar");
            continue;
        }
        hechos += 1;
    }
    Ok(hechos)
}

/// Copia un fichero al proyecto y lo deja como mensaje del cuaderno.
fn adjuntar(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    aparato: &str,
    nombre: &str,
    bytes: &[u8],
) -> std::io::Result<()> {
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    // Primero al cuaderno y solo despues a la pantalla, como al escribir.
    let mensaje = adjuntar_en_proyecto(
        ubicacion.raiz(),
        &a.ficha.id,
        aparato,
        nombre,
        bytes,
        numero,
    )?;
    // Su ojeada AHORA, no al reabrir el proyecto: una foto recien adjuntada
    // salia como una fila con el nombre del fichero («pegado-…png») en vez
    // de verse, y solo aparecia al volver a entrar.
    a.vistas.push(leer_vista(ubicacion, &a.ficha.id, &mensaje));
    a.ficha.tocado = mensaje.cuando;
    a.ficha.resumen = nombre.to_string();
    a.mensajes.push(mensaje);
    Ok(())
}

/// Lo de `adjuntar` sin la conversacion abierta: copia el fichero al
/// proyecto, lo apunta en su cuaderno y sube el proyecto en la lista.
/// Devuelve el mensaje nuevo. Es lo que usa el universo al soltar o pegar
/// (D227, D244): un fichero entra al chat por un solo sitio.
pub(crate) fn adjuntar_en_proyecto(
    raiz: &std::path::Path,
    proyecto: &str,
    aparato: &str,
    nombre: &str,
    bytes: &[u8],
    numero: i64,
) -> std::io::Result<pixpin_proyecto::cuaderno::Mensaje> {
    use pixpin_proyecto::{almacen, cuaderno};
    let ruta = almacen::guardar_adjunto(raiz, proyecto, nombre, bytes)?;
    let cuando = pixpin_shell::entorno::ahora_utc_ms();
    let mensaje = cuaderno::Mensaje::adjunto(
        cuaderno::clase_de_nombre(nombre),
        nombre,
        &ruta,
        bytes.len() as i64,
        &cuaderno::Sello {
            cuando,
            numero,
            aparato: aparato.to_string(),
            proyecto: proyecto.to_string(),
        },
    );
    cuaderno::anadir(&almacen::carpeta(raiz, proyecto), &mensaje)?;
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == proyecto) {
        f.tocado = cuando;
        f.resumen = nombre.to_string();
        indice.guardar(raiz)?;
    }
    Ok(mensaje)
}

/// Mete en un proyecto varios ficheros ya leidos, con numeros seguidos a
/// partir del ultimo de su cuaderno. Uno que falle se apunta en el registro
/// y no se lleva los demas; lo que devuelve son los que entraron.
pub(crate) fn meter_en_proyecto(
    raiz: &std::path::Path,
    proyecto: &str,
    ficheros: &[(String, Vec<u8>)],
    aparato: &str,
) -> std::io::Result<Vec<pixpin_proyecto::cuaderno::Mensaje>> {
    use pixpin_proyecto::{almacen, cuaderno};
    let mut numero = match cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, proyecto)) {
        Ok(c) => c.siguiente_numero(Some(proyecto)),
        // Un proyecto sin nada escrito todavia empieza por el uno.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 1,
        Err(e) => return Err(e),
    };
    let mut hechos = Vec::new();
    for (nombre, bytes) in ficheros {
        match adjuntar_en_proyecto(raiz, proyecto, aparato, nombre, bytes, numero) {
            Ok(m) => {
                hechos.push(m);
                numero += 1;
            }
            Err(e) => tracing::warn!(?e, nombre, "fichero que no se pudo meter en el proyecto"),
        }
    }
    Ok(hechos)
}

/// Lee de disco los ficheros que se sueltan o se pegan, con su nombre. Uno
/// que no se pueda leer se salta: no puede llevarse los demas.
pub(crate) fn leer_ficheros(rutas: &[std::path::PathBuf]) -> Vec<(String, Vec<u8>)> {
    rutas
        .iter()
        .filter_map(|r| {
            let nombre = r.file_name()?.to_string_lossy().to_string();
            match std::fs::read(r) {
                Ok(bytes) => Some((nombre, bytes)),
                Err(e) => {
                    tracing::warn!(?e, ruta = %r.display(), "fichero que no se pudo leer");
                    None
                }
            }
        })
        .collect()
}

/// La fecha del separador de dias, como la escribe PixPin Android: «Hoy»,
/// «Ayer», «15 de septiembre», y con el ano si es de otro.
///
/// Telegram Desktop no usa «Hoy» ni «Ayer», pero esta app es el puerto de
/// la de Android y ahi si se usan; manda el original.
fn fecha_larga(cuando_ms: i64, ahora_ms: i64, textos: &Catalogo) -> String {
    if cuando_ms <= 0 {
        return String::new();
    }
    const DIA: i64 = 86_400_000;
    let cuantos = ahora_ms.div_euclid(DIA) - cuando_ms.div_euclid(DIA);
    if cuantos == 0 {
        return textos.t("chat-hoy");
    }
    if cuantos == 1 {
        return textos.t("chat-ayer");
    }
    let (ano, mes, dia) = pixpin_ui::chat::partes_fecha(cuando_ms);
    let (ano_ahora, _, _) = pixpin_ui::chat::partes_fecha(ahora_ms);
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("dia", dia);
    args.set("mes", textos.t(&format!("chat-mes-{mes}")));
    if ano == ano_ahora {
        return textos.t_args("chat-fecha", &args);
    }
    args.set("ano", ano);
    textos.t_args("chat-fecha-con-ano", &args)
}

/// Que proyectos pasan el filtro del buscador, por su indice.
///
/// Se compara sin distinguir mayusculas y buscando la palabra en cualquier
/// sitio del nombre o de la ultima linea: escribir «playa» tiene que
/// encontrar «Casa de playa», no solo lo que empieza por ahi.
fn filtrar(fichas: &[pixpin_proyecto::almacen::Ficha], busqueda: &str) -> Vec<usize> {
    let aguja = busqueda.trim().to_lowercase();
    if aguja.is_empty() {
        return (0..fichas.len()).collect();
    }
    fichas
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            f.nombre.to_lowercase().contains(&aguja) || f.resumen.to_lowercase().contains(&aguja)
        })
        .map(|(i, _)| i)
        .collect()
}

/// Copia unos ficheros al proyecto y los deja como mensajes. Devuelve
/// cuantos entraron: uno que falle no puede llevarse los demas.
/// El cuadro que pregunta antes de meter ficheros en el proyecto.
fn pintar_confirmar(p: &Pintor, c: &Pinta, pend: &Pendientes, marco: Rect) {
    use pixpin_ui::confirmar as cf;
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let ventana = Rect {
        x: 0,
        y: 0,
        ancho: marco.ancho,
        alto: marco.alto,
    };
    // El velo dice que lo de debajo esta esperando una respuesta.
    p.rellenar(rf(ventana), tema.velo);
    let d = cf::colocar(ventana, pend.rutas.len(), escala);
    p.rellenar_redondeado(rf(d.caja), cf::RADIO as f32 * e, tema.lista);
    p.empujar_recorte(rf(d.caja));

    let mut args = fluent_bundle::FluentArgs::new();
    args.set("cuantos", pend.rutas.len());
    let (_, alto_titulo) = p.medir_texto("X", cf::TITULO_TAM * e);
    p.texto_linea(
        &textos.t_args("confirmar-titulo", &args),
        d.cabecera.x as f32 + cf::TITULO_X as f32 * e,
        d.cabecera.y as f32 + (d.cabecera.alto as f32 - alto_titulo) / 2.0,
        cf::TITULO_TAM * e,
        (d.cabecera.ancho as f32 - 2.0 * cf::TITULO_X as f32 * e).max(0.0),
        tema.texto,
    );

    for (n, ruta) in pend.rutas.iter().enumerate().take(d.filas) {
        let fila = d.fila(n, escala);
        let (x, ancho) = d.texto(fila, escala);
        let mini = d.miniatura(fila, escala);
        // La foto de verdad si ya se leyo; si no, un recuadro. Aqui no se
        // pide cargarla: las del cuadro se preparan antes del fotograma.
        match c.miniaturas.ya(ruta) {
            Some((b, w, h)) => {
                p.empujar_recorte(rf(mini));
                crate::miniaturas::pintar_recortado(p, b, rf(mini), w, h);
                p.soltar_recorte();
            }
            None => p.rellenar_redondeado(rf(mini), cf::MINIATURA_RADIO as f32 * e, tema.chat),
        }
        let nombre = ruta
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        p.texto_linea(
            &nombre,
            x as f32,
            fila.y as f32 + cf::NOMBRE_Y as f32 * e,
            cf::NOMBRE_TAM * e,
            ancho as f32,
            tema.texto,
        );
        p.texto_linea(
            &en_bytes(pend.tamanos.get(n).copied().unwrap_or(0)),
            x as f32,
            fila.y as f32 + cf::TAMANO_Y as f32 * e,
            cf::TAMANO_TAM * e,
            ancho as f32,
            tema.apagado,
        );
    }
    if let Some(linea) = d.linea_resto(escala) {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("cuantos", d.resto);
        let (_, alto) = p.medir_texto("X", cf::TAMANO_TAM * e);
        p.texto_linea(
            &textos.t_args("confirmar-resto", &args),
            linea.x as f32 + cf::NOMBRE_X as f32 * e,
            linea.y as f32 + (linea.alto as f32 - alto) / 2.0,
            cf::TAMANO_TAM * e,
            (linea.ancho as f32 - cf::NOMBRE_X as f32 * e).max(0.0),
            tema.apagado,
        );
    }

    // El pie, con su texto en gris mientras esta vacio.
    let caja_pie = Rect {
        x: d.pie.x + (cf::PIE_X as f32 * e) as i32,
        y: d.pie.y,
        ancho: d
            .pie
            .ancho
            .saturating_sub((2.0 * cf::PIE_X as f32 * e) as u32),
        alto: d.pie.alto,
    };
    let (texto_pie, color_pie) = if pend.pie.is_empty() {
        (textos.t("confirmar-pie"), tema.apagado)
    } else {
        (pend.pie.clone(), tema.texto)
    };
    let (_, alto_pie) = p.medir_texto(&texto_pie, cf::PIE_TAM * e);
    p.texto_linea(
        &texto_pie,
        caja_pie.x as f32,
        caja_pie.y as f32 + (caja_pie.alto as f32 - alto_pie) / 2.0,
        cf::PIE_TAM * e,
        caja_pie.ancho as f32,
        color_pie,
    );
    // Una raya debajo dice que ahi se escribe, sin gastar una caja entera.
    p.rellenar(
        RectF {
            x: caja_pie.x as f32,
            y: caja_pie.abajo() as f32 - (1.0 * e).max(1.0),
            ancho: caja_pie.ancho as f32,
            alto: (1.0 * e).max(1.0),
        },
        tema.separador,
    );

    for (boton, clave, fuerte) in [
        (d.cancelar, "confirmar-cancelar", false),
        (d.aceptar, "confirmar-aceptar", true),
    ] {
        if fuerte {
            p.rellenar_redondeado(rf(boton), cf::BOTON_RADIO as f32 * e, tema.enviar);
        }
        let rotulo = textos.t(clave);
        let (w, h) = p.medir_texto(&rotulo, cf::BOTON_TAM * e);
        p.texto(
            &rotulo,
            boton.x as f32 + (boton.ancho as f32 - w) / 2.0,
            boton.y as f32 + (boton.alto as f32 - h) / 2.0,
            cf::BOTON_TAM * e,
            if fuerte {
                tema.texto_elegido
            } else {
                tema.texto
            },
        );
    }
    p.soltar_recorte();
}

/// Un tamano de fichero como se le ensena a una persona.
///
/// Se reparte de mil en mil, no de 1024 en 1024: es lo que dice el
/// Explorador de Windows para el mismo fichero, y discrepar con el sistema
/// operativo en el numero que el usuario acaba de ver es peor que ser exacto.
fn en_bytes(bytes: u64) -> String {
    const UNIDADES: [&str; 4] = ["B", "kB", "MB", "GB"];
    let mut valor = bytes as f64;
    let mut cual = 0;
    while valor >= 1000.0 && cual + 1 < UNIDADES.len() {
        valor /= 1000.0;
        cual += 1;
    }
    if cual == 0 {
        format!("{bytes} {}", UNIDADES[0])
    } else {
        format!("{valor:.1} {}", UNIDADES[cual])
    }
}

/// Los tramos de `texto` que hay que poner en negrita por coincidir con lo
/// que se busca. Vacio si no se busca nada, que es lo normal.
///
/// Traduce de indices de byte —como los cuenta `resaltado`— a unidades
/// UTF-16, que es como los cuenta DirectWrite. Confundirlos no se nota hasta
/// la primera tilde.
fn negritas(texto: &str, aguja: &str) -> Vec<pixpin_render::Tramo> {
    if aguja.trim().is_empty() {
        return Vec::new();
    }
    pixpin_ui::resaltado::coincidencias(texto, aguja)
        .into_iter()
        .filter_map(|t| {
            let (inicio, longitud) = t.en_utf16(texto);
            (longitud > 0).then_some(pixpin_render::Tramo {
                inicio,
                longitud,
                estilo: pixpin_render::EstiloTexto {
                    negrita: true,
                    ..Default::default()
                },
            })
        })
        .collect()
}

/// La ruta del fichero de un mensaje dentro del proyecto, si lo tiene y si
/// esta en este equipo.
///
/// `ruta` es relativa a la carpeta del proyecto. Una que venga del movil
/// puede ser absoluta y de otro aparato: esa no se resuelve aqui, porque
/// apuntaria a un disco que no es este.
fn ruta_del_mensaje(
    raiz: &std::path::Path,
    proyecto: &str,
    m: &pixpin_proyecto::cuaderno::Mensaje,
) -> Option<std::path::PathBuf> {
    let relativa = m.ruta.as_deref().filter(|r| !r.is_empty())?;
    // `pixpin:files/…` es lo que llego sincronizando con el movil: la vista
    // de sincronizar sabe donde lo guardo (`vista::ruta_real`).
    pixpin_proyecto::vista::ruta_real(raiz, proyecto, relativa)
}

/// Las fotos de las burbujas que se ven ahora mismo en el historial.
///
/// Usa la colocacion del ultimo fotograma: la de este se calcula al pintar.
/// Antes del primero no hay ninguna, y la vuelta de despues las pide.
fn fotos_del_historial(a: &Abierto, d: &Disposicion, escala: u32) -> Vec<std::path::PathBuf> {
    use pixpin_ui::historial as h;
    let area = area_del_historial(d, a, escala);
    let c = a.colocado.borrow();
    let scroll = a
        .scroll
        .unwrap_or_else(|| h::scroll_maximo(area, a.alto.get()));
    let (primero, cuantos) = h::visibles(area, &c.puestos, scroll);
    (primero..primero + cuantos)
        .filter_map(|cual| c.mensaje(cual))
        .filter_map(|i| match a.vistas.get(i) {
            Some(Some(Ojeada::Foto { .. })) => {
                ruta_del_mensaje(&a.raiz, &a.ficha.id, a.mensajes.get(i)?)
            }
            // Una hoja de un plano tambien tiene foto: su pagina del PDF o
            // la imagen que trae dentro. Sin pedirla aqui, la burbuja se
            // quedaria con los trazos flotando sobre el blanco.
            Some(Some(Ojeada::Lienzo(l))) => l.fondo.as_ref().map(|(r, _)| r.clone()),
            _ => None,
        })
        .collect()
}

/// Las fotos que se ven ahora mismo en el panel, en cuadricula o en lista.
///
/// Solo las visibles, y en el orden en que se ven: con trescientas fotos, lo
/// que importa es que salgan primero las que el usuario esta mirando.
fn fotos_a_la_vista(
    a: &Abierto,
    seccion: pixpin_proyecto::cuaderno::Seccion,
    d: &pixpin_ui::info::Disposicion,
    escala: u32,
) -> Vec<std::path::PathBuf> {
    use pixpin_proyecto::cuaderno::Clase;
    let suyos = pixpin_proyecto::cuaderno::indices_de_seccion(&a.mensajes, seccion);
    let (primera, cuantas) = if seccion.es_cuadricula() {
        let r = pixpin_ui::info::rejilla(d.contenido.ancho, escala);
        r.visibles(d.contenido, suyos.len(), a.scroll_info, escala)
    } else {
        // La misma cuenta que al pintar las filas, incluida la fila de mas
        // por arriba y por abajo que asoma al desplazar.
        let alto = (pixpin_ui::info::ARCHIVO_ALTO * escala / 100).max(1);
        (
            (a.scroll_info / alto as i32).max(0) as usize,
            (d.contenido.alto / alto) as usize + 2,
        )
    };
    suyos
        .into_iter()
        .skip(primera)
        .take(cuantas)
        .filter_map(|n| {
            let m = a.mensajes.get(n)?;
            // Solo las fotos: un dibujo ya se pinta de su lienzo, y de un
            // archivo no hay nada que descomprimir.
            (m.clase == Some(Clase::Imagen))
                .then(|| ruta_del_mensaje(&a.raiz, &a.ficha.id, m))
                .flatten()
        })
        .collect()
}

/// Apunta en el indice el nombre que se acaba de escribir.
///
/// Un nombre en blanco no se guarda como tal: se deja el que ya hubiera, o
/// se pone uno cualquiera. Una lista con una fila sin nombre no se puede
/// usar —no hay donde pulsar con seguridad ni que buscar—, y el usuario
/// puede cambiarlo cuando quiera.
fn guardar_nombre(ubicacion: &Ubicacion, a: &mut Abierto) {
    if a.ficha.nombre.trim().is_empty() {
        a.ficha.nombre = "Proyecto".into();
    }
    let raiz = ubicacion.raiz();
    let mut indice = pixpin_proyecto::almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == a.ficha.id) {
        f.nombre = a.ficha.nombre.clone();
        if let Err(e) = indice.guardar(raiz) {
            tracing::warn!(?e, "no se pudo guardar el nombre del proyecto");
        }
    }
}

/// Crea una hoja de calculo vacia en la conversacion.
///
/// A diferencia del lienzo, NO hay fichero: el documento entero va en el
/// texto del mensaje, que es como Android guarda sus mini-aplicaciones. Ver
/// la cabecera de `pixpin_proyecto::tabla`.
fn crear_tabla(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    aparato: &str,
    textos: &Catalogo,
) -> std::io::Result<()> {
    use pixpin_proyecto::{almacen, cuaderno};
    let nombre = textos.t("tabla-nueva");
    let tabla = pixpin_proyecto::tabla::Tabla {
        nombre: nombre.clone(),
        ..Default::default()
    };
    let documento = tabla
        .escribir()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let cuando = pixpin_shell::entorno::ahora_utc_ms();
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let mensaje = cuaderno::Mensaje::miniapp(
        pixpin_proyecto::tabla::MINIAPP,
        &nombre,
        &documento,
        &cuaderno::Sello {
            cuando,
            numero,
            aparato: aparato.to_string(),
            proyecto: a.ficha.id.clone(),
        },
    );
    let raiz = ubicacion.raiz();
    cuaderno::anadir(&almacen::carpeta(raiz, &a.ficha.id), &mensaje)?;
    a.vistas.push(leer_vista(ubicacion, &a.ficha.id, &mensaje));
    a.mensajes.push(mensaje);
    a.ficha.tocado = cuando;
    a.ficha.resumen = nombre;
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == a.ficha.id) {
        f.tocado = a.ficha.tocado;
        f.resumen = a.ficha.resumen.clone();
        indice.guardar(raiz)?;
    }
    Ok(())
}

/// Crea una de las siete mini-apps del movil en la conversacion.
///
/// El documento nace con `mini::documento_nuevo`, que lo escribe **letra por
/// letra como lo escribe el movil**, y el mensaje con `cuaderno::Mensaje`,
/// que pone los campos que espera Kotlin: escribirlo a mano cambiaria el
/// resumen de sincronizacion y el movil veria el mensaje como modificado en
/// cada vuelta.
fn crear_miniapp(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    aparato: &str,
    cual: &str,
    textos: &Catalogo,
) -> std::io::Result<()> {
    use pixpin_proyecto::{almacen, cuaderno};
    let nombre = clave_del_nombre(cual)
        .map(|c| textos.t(c))
        .unwrap_or_default();
    let documento =
        pixpin_proyecto::mini::documento_nuevo(cual, &nombre, &moneda_del_catalogo(textos))
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("no se conoce la mini-app «{cual}»"),
                )
            })?;

    let cuando = pixpin_shell::entorno::ahora_utc_ms();
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let mensaje = cuaderno::Mensaje::miniapp(
        cual,
        &nombre,
        &documento,
        &cuaderno::Sello {
            cuando,
            numero,
            aparato: aparato.to_string(),
            proyecto: a.ficha.id.clone(),
        },
    );
    let raiz = ubicacion.raiz();
    cuaderno::anadir(&almacen::carpeta(raiz, &a.ficha.id), &mensaje)?;
    // Estas no tienen ojeada: su burbuja es la fila con el icono, el nombre y
    // el resumen. Una vista previa de «0 de 0» no dice mas que el resumen.
    a.vistas.push(None);
    a.mensajes.push(mensaje);
    a.ficha.tocado = cuando;
    a.ficha.resumen = nombre;
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == a.ficha.id) {
        f.tocado = a.ficha.tocado;
        f.resumen = a.ficha.resumen.clone();
        indice.guardar(raiz)?;
    }
    Ok(())
}

/// Crea un lienzo vacio en el proyecto y lo anuncia en el cuaderno.
///
/// El mensaje lleva `referencia` (el id, que es lo que lee el movil) Y
/// `ruta` (relativa, para poder abrirlo desde aqui): Android usa la primera
/// y este equipo la segunda, y ninguna de las dos sobra.
fn crear_lienzo(ubicacion: &Ubicacion, a: &mut Abierto, aparato: &str) -> std::io::Result<()> {
    use pixpin_proyecto::{almacen, cuaderno};
    let raiz = ubicacion.raiz();
    let id = pixpin_proyecto::codigos::nuevo();
    let ruta = almacen::lienzo(raiz, &a.ficha.id, &id);
    if let Some(padre) = ruta.parent() {
        std::fs::create_dir_all(padre)?;
    }
    let json = pixpin_motor2d::excalidraw::escribir(&pixpin_motor2d::excalidraw::Lienzo::vacio());
    std::fs::write(&ruta, &json)?;

    let cuando = pixpin_shell::entorno::ahora_utc_ms();
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let nombre = format!("{id}.excalidraw");
    let mut mensaje = cuaderno::Mensaje::adjunto(
        cuaderno::Clase::Dibujo,
        &nombre,
        &format!("lienzos/{nombre}"),
        json.len() as i64,
        &cuaderno::Sello {
            cuando,
            numero,
            aparato: aparato.to_string(),
            proyecto: a.ficha.id.clone(),
        },
    );
    mensaje.referencia = Some(id);
    cuaderno::anadir(&almacen::carpeta(raiz, &a.ficha.id), &mensaje)?;

    // Un lienzo recien creado esta vacio, y `leer_vista` devuelve `None` a
    // proposito para los vacios: la burbuja ensena su nombre hasta que se
    // dibuje algo.
    a.vistas.push(leer_vista(ubicacion, &a.ficha.id, &mensaje));
    a.mensajes.push(mensaje);
    a.ficha.tocado = cuando;
    a.ficha.resumen = nombre;
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == a.ficha.id) {
        f.tocado = a.ficha.tocado;
        f.resumen = a.ficha.resumen.clone();
        indice.guardar(raiz)?;
    }
    Ok(())
}

fn meter_ficheros(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    aparato: &str,
    rutas: &[std::path::PathBuf],
    pie: &str,
) -> (usize, Vec<std::path::PathBuf>) {
    let mut hechos = 0;
    // Los PDF no se adjuntan a esta conversacion: cada uno es un proyecto
    // propio con una hoja por pagina. Se devuelven para que los abra quien
    // lleva la lista de proyectos, que aqui no se ve.
    let mut pdfs = Vec::new();
    for ruta in rutas {
        if es_pdf(ruta) {
            pdfs.push(ruta.clone());
            continue;
        }
        let nombre = ruta
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "archivo".into());
        match std::fs::read(ruta) {
            Ok(bytes) => match adjuntar(ubicacion, a, aparato, &nombre, &bytes) {
                Ok(()) => hechos += 1,
                Err(e) => tracing::warn!(?e, nombre, "no se pudo guardar el adjunto"),
            },
            Err(e) => tracing::warn!(?e, ruta = %ruta.display(), "no se pudo leer"),
        }
    }
    // El pie va DESPUES y como nota aparte, no dentro del mensaje del
    // fichero: asi PixPin Android lo ensena como lo que es, un comentario, y
    // se puede fijar o buscar por su cuenta.
    if hechos > 0 && !pie.trim().is_empty() {
        let antes = std::mem::replace(&mut a.borrador, pie.trim().to_string());
        if let Err(e) = guardar_nota(ubicacion, a, aparato) {
            tracing::warn!(?e, "no se pudo guardar el pie");
        }
        a.borrador = antes;
    }
    (hechos, pdfs)
}

/// Un lienzo ya leido y listo para pintar en su burbuja.
struct LienzoVisto {
    ordenes: Vec<pixpin_motor2d::Orden>,
    /// La caja que ocupa el dibujo, en sus propias coordenadas.
    caja: (f32, f32, f32, f32),
    /// Lo que va DEBAJO del dibujo y donde: la pagina del PDF sobre la que
    /// se dibujo, o la foto de la hoja. Sin esto, una hoja de un plano se
    /// ensena como cuatro rayas flotando en blanco.
    fondo: Option<(std::path::PathBuf, (f32, f32, f32, f32))>,
}

/// Lee el lienzo de un mensaje de clase DIBUJO.
///
/// Devuelve `None` si el mensaje no apunta a ninguno, si su fichero no esta
/// o si el dibujo esta vacio: una burbuja con un recuadro en blanco es peor
/// que una que diga «Dibujo» y su nombre.
fn leer_vista(
    ubicacion: &Ubicacion,
    proyecto: &str,
    m: &pixpin_proyecto::cuaderno::Mensaje,
) -> Option<Ojeada> {
    use pixpin_proyecto::cuaderno::Clase;
    // Una foto ensena su vista previa si su fichero esta en este equipo;
    // una que llego del movil sin el fichero se queda en texto.
    if m.clase == Some(Clase::Imagen) {
        let ruta = ruta_del_mensaje(ubicacion.raiz(), proyecto, m).filter(|r| r.is_file())?;
        // Solo la cabecera: descomprimir cada foto del proyecto al abrirlo
        // seria medio segundo por cada una.
        let (w, h) = pixpin_codec::imagen::medidas(&ruta)
            .inspect_err(
                |e| tracing::info!(?e, ruta = %ruta.display(), "foto que no se pudo medir"),
            )
            .ok()?;
        // Lo dibujado encima, si lo hay: una foto anotada tiene que verse
        // anotada tambien cuando su burbuja esta quieta.
        let dibujo = pixpin_motor2d::cargar(&dibujo_de_foto(&ruta))
            .map(|e| pixpin_motor2d::ordenes_de_escena(&e))
            .unwrap_or_default();
        return Some(Ojeada::Foto {
            doc: (w as f32, h as f32),
            dibujo,
        });
    }
    // Una tabla no tiene fichero: su documento es el propio texto del
    // mensaje, asi que se lee sin tocar el disco.
    if m.clase == Some(Clase::MiniApp)
        && m.miniapp.as_deref() == Some(pixpin_proyecto::tabla::MINIAPP)
    {
        let tabla = pixpin_proyecto::tabla::Tabla::leer(&m.texto)
            .inspect_err(|e| tracing::warn!(?e, "tabla que no se entiende"))
            .ok()?;
        // Una tabla sin nada escrito no ensena rejilla: seria un recuadro
        // vacio que no dice mas que su nombre.
        if tabla.celdas.is_empty() {
            return None;
        }
        return Some(Ojeada::Tabla(Box::new(tabla)));
    }
    if !matches!(m.clase, Some(Clase::Dibujo) | Some(Clase::Pagina)) {
        return None;
    }
    // **Una pagina adjunta se ve, no se lee.** Como fila decia «documento.pdf
    // · pag. 7», que es justo lo que uno no recuerda: lo que se recuerda es
    // lo que habia senalado en ella. Se ensena la hoja con lo anotado encima.
    //
    // Una pagina sin anotaciones no tiene lienzo que leer, y aun asi tiene
    // que verse: su vista es el fondo solo.
    let carpeta_pagina = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), proyecto);
    if m.clase == Some(Clase::Pagina) && m.referencia.as_deref().is_none_or(|r| r.is_empty()) {
        let pagina = m.pagina?;
        let (ruta, w, h) = pagina_del_pdf(&carpeta_pagina, pagina)?;
        return Some(Ojeada::Lienzo(LienzoVisto {
            ordenes: Vec::new(),
            caja: (0.0, 0.0, w, h),
            fondo: Some((ruta, (0.0, 0.0, w, h))),
        }));
    }
    // `referencia` es el id del dibujo, no un fichero: asi lo escribe
    // Android, y por eso no se usa `ruta`.
    let id = m.referencia.as_deref().filter(|r| !r.is_empty())?;
    let ruta = pixpin_proyecto::almacen::lienzo(ubicacion.raiz(), proyecto, id);
    let texto = std::fs::read_to_string(&ruta)
        .inspect_err(|e| tracing::warn!(?e, ruta = %ruta.display(), "lienzo que no se pudo leer"))
        .ok()?;
    let lienzo = pixpin_motor2d::excalidraw::leer(&texto)
        .inspect_err(|e| tracing::warn!(?e, "lienzo que no se entiende"))
        .ok()?;
    let mut escena = pixpin_motor2d::Escena::nueva();
    for e in lienzo.elementos() {
        escena.anadir(e);
    }
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), proyecto);
    // El fondo de la hoja: la pagina del PDF si se dibujo sobre una, y si no
    // la foto que trae el propio dibujo (`imagenes/<id>` del paquete).
    let fondo = match m.pagina {
        Some(pagina) => pagina_del_pdf(&carpeta, pagina).map(|(r, w, h)| (r, (0.0, 0.0, w, h))),
        None => {
            let ficheros = pixpin_motor2d::excalidraw::ficheros(&lienzo);
            escena
                .elementos
                .iter()
                .find_map(|e| match e.figura {
                    pixpin_motor2d::Figura::Imagen { id_objeto } => ficheros
                        .iter()
                        .find(|(id, _)| *id == id_objeto)
                        .and_then(|(_, rel)| {
                            pixpin_proyecto::vista::ruta_real(ubicacion.raiz(), proyecto, rel)
                        })
                        .map(|r| (r, e.caja())),
                    _ => None,
                })
                .filter(|(r, _)| r.is_file())
        }
    };
    let caja = match fondo {
        // Con fondo manda su caja: un trazo que se salga de la hoja no puede
        // encoger el plano entero para caber con el.
        Some((_, c)) => c,
        None => escena.caja()?,
    };
    let ordenes = pixpin_motor2d::ordenes_de_escena(&escena);
    if ordenes.is_empty() && fondo.is_none() {
        return None;
    }
    Some(Ojeada::Lienzo(LienzoVisto {
        ordenes,
        caja,
        fondo,
    }))
}

/// Lo que se ensena dentro de una burbuja ademas del texto.
///
/// Se lee UNA vez al abrir el proyecto y se guarda en paralelo a los
/// mensajes: releer y traducir un excalidraw en cada fotograma seria tirar el
/// rato, y con una tabla pasa lo mismo aunque no haya fichero.
enum Ojeada {
    Lienzo(LienzoVisto),
    /// En caja porque una `Tabla` es mucho mas grande que un `LienzoVisto`, y
    /// sin ella todas las entradas del vector pagarian ese tamano.
    Tabla(Box<pixpin_proyecto::tabla::Tabla>),
    /// Una foto del proyecto: cuanto mide y lo que se haya dibujado encima.
    ///
    /// Sus pixeles no estan aqui: salen de las vistas previas, que se cargan
    /// poco a poco. El tamano si, porque los trazos van en coordenadas de la
    /// foto y sin el no se sabe donde caen.
    Foto {
        doc: (f32, f32),
        dibujo: Vec<pixpin_motor2d::Orden>,
    },
}

/// Una ojeada a una tabla dentro de su burbuja: las primeras celdas y ya.
///
/// Se pintan a tamano de verdad y se corta lo que no cabe, en vez de encoger
/// la hoja entera para que quepa: una tabla encogida no se lee, y la burbuja
/// no es para leerla sino para reconocerla. Quien quiera verla, la abre.
fn pintar_ojeada_tabla(
    p: &Pintor,
    tema: &Tema,
    escala: u32,
    t: &pixpin_proyecto::tabla::Tabla,
    destino: RectF,
) {
    use pixpin_ui::tabla as ui;
    let e = escala as f32 / 100.0;
    let (columnas, filas) = t.tamano();
    if columnas == 0 || filas == 0 {
        return;
    }
    p.empujar_recorte(destino);
    let ancho = ui::COLUMNA_ANCHO as f32 * e;
    let alto = ui::FILA_ALTO as f32 * e;
    let linea = (1.0 * e).max(1.0);
    // Una de mas por cada lado: la ultima queda cortada a proposito, que es
    // lo que dice «sigue».
    let cuantas_x = (destino.ancho / ancho).ceil() as u32 + 1;
    let cuantas_y = (destino.alto / alto).ceil() as u32 + 1;
    for fila in 0..filas.min(cuantas_y) {
        for columna in 0..columnas.min(cuantas_x) {
            let celda = RectF {
                x: destino.x + columna as f32 * ancho,
                y: destino.y + fila as f32 * alto,
                ancho,
                alto,
            };
            p.trazar(celda, linea, tema.separador);
            let contenido = t.celda(pixpin_proyecto::tabla::Ref { columna, fila });
            if contenido.is_empty() {
                continue;
            }
            let (_, alto_texto) = p.medir_texto(contenido, ui::CELDA_TAM * e);
            p.texto_linea(
                contenido,
                celda.x + ui::CELDA_RELLENO as f32 * e,
                celda.y + (alto - alto_texto) / 2.0,
                ui::CELDA_TAM * e,
                (ancho - 2.0 * ui::CELDA_RELLENO as f32 * e).max(0.0),
                tema.texto_papel,
            );
        }
    }
    p.soltar_recorte();
}

/// Pinta un lienzo dentro de `destino`, entero y sin deformarlo.
fn pintar_lienzo(
    p: &Pintor,
    vista: &LienzoVisto,
    destino: RectF,
    previas: Option<&crate::miniaturas::Miniaturas>,
) {
    use pixpin_motor2d::Orden;
    let (x0, y0, x1, y1) = vista.caja;
    let (ancho, alto) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    // La misma escala en los dos ejes, y centrado: deformar un plano para
    // que llene la caja lo hace ilegible.
    let escala = (destino.ancho / ancho).min(destino.alto / alto);
    let dx = destino.x + (destino.ancho - ancho * escala) / 2.0;
    let dy = destino.y + (destino.alto - alto * escala) / 2.0;
    let mover = |q: &pixpin_motor2d::Punto2| ((q.x - x0) * escala + dx, (q.y - y0) * escala + dy);
    let color = |c: pixpin_motor2d::ColorRgba| Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    };

    p.empujar_recorte(destino);
    // Primero el fondo —la pagina del plano o la foto—, y encima el dibujo:
    // es el orden en el que se hizo en el movil.
    if let Some((ruta, (fx0, fy0, fx1, fy1))) = &vista.fondo
        && let Some((b, w, alto)) = previas.and_then(|c| c.ya(ruta))
    {
        let caja = RectF {
            x: (fx0 - x0) * escala + dx,
            y: (fy0 - y0) * escala + dy,
            ancho: (fx1 - fx0) * escala,
            alto: (fy1 - fy0) * escala,
        };
        let _ = (w, alto);
        p.bitmap_con(b, caja, None, pixpin_render::Interpolacion::Lineal);
    }
    for orden in &vista.ordenes {
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
                // El grosor escala con el dibujo; si no, un trazo grueso en
                // una vista pequena lo taparia entero.
                p.polilinea(&v, (grosor * escala).max(0.75), color(*c));
            }
            Orden::Texto {
                texto,
                x,
                y,
                tam,
                color: c,
                ancho_max,
                ..
            } => p.texto_ajustado(
                texto,
                (x - x0) * escala + dx,
                (y - y0) * escala + dy,
                (tam * escala).max(4.0),
                ancho_max * escala,
                color(*c),
            ),
            // El velo es de la capa viva y las imagenes incrustadas todavia
            // no tienen almacen: en una vista previa no se echan de menos.
            Orden::Velo { .. } | Orden::Imagen { .. } => {}
        }
    }
    p.soltar_recorte();
}

/// Abre lo que hay detras de un mensaje: su fichero, con la aplicacion que
/// le toque.
///
/// Un mensaje sin fichero (una nota) no hace nada al pincharlo, que es mejor
/// que abrir algo que el usuario no pidio. Los dibujos todavia no abren el
/// editor: escribir de vuelta el `.excalidraw` sin perder lo que el movil
/// mete y aqui no se entiende es un trabajo aparte, y a medias seria peor.
fn abrir_mensaje(ubicacion: &Ubicacion, a: &Abierto, indice: usize) {
    let Some(m) = a.mensajes.get(indice) else {
        return;
    };
    let Some(relativa) = m.ruta.as_deref().filter(|r| !r.is_empty()) else {
        return;
    };
    // La ruta del mensaje es relativa a la carpeta del proyecto. Una que
    // venga del movil sera absoluta y de otro aparato: entonces no hay nada
    // que abrir aqui, y decirlo es mejor que abrir cualquier cosa.
    let Some(ruta) = pixpin_proyecto::vista::ruta_real(ubicacion.raiz(), &a.ficha.id, relativa)
    else {
        return;
    };
    if !ruta.is_file() {
        tracing::info!(
            ruta = %ruta.display(),
            "el fichero de ese mensaje no esta en este equipo"
        );
        return;
    }
    if let Err(e) = pixpin_shell::abrir::abrir(&ruta) {
        tracing::warn!(?e, ruta = %ruta.display(), "no se pudo abrir");
    }
}

/// Las secciones del panel, en el orden en que se ensenan.
const SECCIONES: [pixpin_proyecto::cuaderno::Seccion; 7] =
    pixpin_proyecto::cuaderno::Seccion::TODAS;

/// La pantalla de informacion del proyecto: ficha, pestanas y contenido.
///
/// Ocupa la columna de la derecha entera. En Telegram seria una tercera
/// columna cuando la ventana es muy ancha; aqui la ventana no suele serlo
/// tanto, y tapar la conversacion es lo que hace el propio Telegram en
/// cuanto no le caben tres columnas.
fn pintar_info(
    p: &Pintor,
    d: &Disposicion,
    c: &Pinta,
    a: &Abierto,
    seccion: pixpin_proyecto::cuaderno::Seccion,
) {
    use pixpin_ui::info;
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    // El velo oscurece TODA la ventana, no solo la conversacion: es lo que
    // dice que lo de debajo esta esperando, y lo que hace que el recuadro se
    // lea como una capa y no como otra columna mas.
    let ventana = Rect {
        x: 0,
        y: 0,
        ancho: d.barra.ancho,
        alto: d.chat.abajo().max(d.lista.abajo()).max(0) as u32,
    };
    p.rellenar(rf(ventana), tema.velo);

    let (caja, _completa) = info::capa_en(ventana, escala);
    let i = info::Disposicion::capa(caja, escala);
    p.rellenar_redondeado(rf(i.panel), info::CAPA_RADIO as f32 * e, tema.lista);

    // La cabecera: el nombre del proyecto y, debajo, lo que tiene dentro. No
    // hay flecha de volver: en una capa se cierra con el aspa.
    let lupa = i.buscar(escala);
    let ancho_titulo = (lupa.x - i.cabecera.x) as f32 - info::CAPA_TITULO_X as f32 * e;
    if a.buscando_info {
        // Con la lupa encendida, la caja de buscar ocupa el sitio del
        // titulo: mientras se busca, el nombre del proyecto no dice nada que
        // no se sepa ya.
        let b = i.caja_buscar(escala);
        p.rellenar_redondeado(rf(b), b.alto as f32 / 2.0, tema.buscador);
        let (texto, color) = if a.busqueda_info.is_empty() {
            (textos.t("chat-buscar"), tema.apagado)
        } else {
            (a.busqueda_info.clone(), tema.texto)
        };
        let (_, alto_texto) = p.medir_texto(&texto, info::BUSCAR_TAM * e);
        p.texto_linea(
            &texto,
            b.x as f32 + info::BUSCAR_TEXTO_X as f32 * e,
            b.y as f32 + (b.alto as f32 - alto_texto) / 2.0,
            info::BUSCAR_TAM * e,
            (b.ancho as f32 - 2.0 * info::BUSCAR_TEXTO_X as f32 * e).max(0.0),
            color,
        );
    } else {
        p.texto_linea(
            &a.ficha.nombre,
            i.cabecera.x as f32 + info::CAPA_TITULO_X as f32 * e,
            i.cabecera.y as f32 + info::CAPA_TITULO_Y as f32 * e,
            info::CAPA_TITULO_TAM * e,
            ancho_titulo.max(0.0),
            tema.texto,
        );
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("cuantas", a.ficha.hojas);
        // Las hojas y, detras, el codigo unico del proyecto: es el que hay
        // que mirar para saber si lo que hay aqui y lo que hay en el movil
        // son la misma cosa, y sin verlo no se puede comprobar una
        // sincronizacion. En el movil sale igual.
        let subtitulo = format!(
            "{}  ·  {}",
            textos.t_args("chat-hojas", &args),
            a.ficha.codigo_unico()
        );
        p.texto_linea(
            &subtitulo,
            i.cabecera.x as f32 + info::CAPA_TITULO_X as f32 * e,
            i.cabecera.y as f32 + info::CAPA_SUBTITULO_Y as f32 * e,
            info::CAPA_SUBTITULO_TAM * e,
            ancho_titulo.max(0.0),
            tema.apagado,
        );
    }

    // El aspa de cerrar, pegada al borde derecho, y la lupa a su izquierda.
    let grosor = (2.0 * e).max(1.0);
    let centro = |r: Rect| {
        (
            r.x as f32 + r.ancho as f32 / 2.0,
            r.y as f32 + r.alto as f32 / 2.0,
        )
    };
    let (cx, cy) = centro(i.volver);
    let brazo = 5.0 * e;
    p.linea(
        (cx - brazo, cy - brazo),
        (cx + brazo, cy + brazo),
        grosor,
        tema.texto,
    );
    p.linea(
        (cx - brazo, cy + brazo),
        (cx + brazo, cy - brazo),
        grosor,
        tema.texto,
    );
    let (lx, ly) = centro(lupa);
    let lado = 16.0 * e;
    p.icono(
        &pixpin_render::iconos_excalidraw::SEARCH_ICON,
        RectF {
            x: lx - lado / 2.0,
            y: ly - lado / 2.0,
            ancho: lado,
            alto: lado,
        },
        tema.texto,
    );

    // La tira de pestanas. Se miden aqui, que es donde esta la fuente, y se
    // apuntan para que el raton sepa luego en cual se pulso.
    let rotulos: Vec<String> = SECCIONES.iter().map(|s| textos.t(s.clave())).collect();
    let anchos: Vec<f32> = rotulos
        .iter()
        .map(|r| p.medir_texto(r, info::PESTANA_TAM * e).0)
        .collect();
    *a.anchos_pestanas.borrow_mut() = anchos.clone();
    let pestanas = i.pestanas(&anchos, escala);
    p.rellenar_redondeado(rf(i.isla), i.isla.alto as f32 / 2.0, tema.cabecera);
    p.empujar_recorte(rf(i.isla));
    for (n, r) in pestanas.iter().enumerate() {
        let activa = SECCIONES[n] == seccion;
        if activa {
            let pildora = i.pildora(*r, escala);
            p.rellenar_redondeado(rf(pildora), pildora.alto as f32 / 2.0, tema.pestana_activa);
        }
        let (w, h) = p.medir_texto(&rotulos[n], info::PESTANA_TAM * e);
        p.texto(
            &rotulos[n],
            r.x as f32 + (r.ancho as f32 - w) / 2.0,
            r.y as f32 + (r.alto as f32 - h) / 2.0,
            info::PESTANA_TAM * e,
            if activa {
                tema.texto_pestana_activa
            } else {
                tema.apagado
            },
        );
    }
    p.soltar_recorte();

    // Y el contenido de la seccion.
    // Por indice y no por referencia: con el numero se llega tambien a
    // `a.vistas`, que es donde esta el lienzo ya leido de cada dibujo.
    let mut suyos = pixpin_proyecto::cuaderno::indices_de_seccion(&a.mensajes, seccion);
    if !a.busqueda_info.trim().is_empty() {
        suyos.retain(|n| {
            let m = &a.mensajes[*n];
            pixpin_ui::resaltado::hay_coincidencia(&m.resumen(), &a.busqueda_info)
                || pixpin_ui::resaltado::hay_coincidencia(&m.nombre, &a.busqueda_info)
        });
    }
    if suyos.is_empty() {
        let vacio = textos.t("chat-sin-mensajes");
        let (w, h) = p.medir_texto(&vacio, 13.0 * e);
        p.texto(
            &vacio,
            i.contenido.x as f32 + (i.contenido.ancho as f32 - w) / 2.0,
            i.contenido.y as f32 + (i.contenido.alto as f32 - h) / 2.0,
            13.0 * e,
            tema.apagado,
        );
        a.alto_info.set(0);
        return;
    }

    p.empujar_recorte(rf(i.contenido));
    if seccion.es_cuadricula() {
        let r = info::rejilla(i.contenido.ancho, escala);
        a.alto_info.set(r.alto_total(suyos.len(), escala));
        let (primera, cuantas) = r.visibles(i.contenido, suyos.len(), a.scroll_info, escala);
        for (n, indice) in suyos
            .iter()
            .copied()
            .enumerate()
            .skip(primera)
            .take(cuantas)
        {
            let celda = r.celda(n, i.contenido, a.scroll_info, escala);
            let m = &a.mensajes[indice];
            // Una foto de verdad, si ya esta leida.
            let foto =
                ruta_del_mensaje(&a.raiz, &a.ficha.id, m).and_then(|ruta| c.miniaturas.ya(&ruta));
            if let Some((b, w, h)) = foto {
                p.empujar_recorte(rf(celda));
                crate::miniaturas::pintar_recortado(p, b, rf(celda), w, h);
                p.soltar_recorte();
                continue;
            }
            // Una foto que aun no esta leida sale como las demas cosas sin
            // miniatura: con su nombre, no con un papel en blanco.
            let vista = a
                .vistas
                .get(indice)
                .and_then(|v| v.as_ref())
                .filter(|v| !matches!(v, Ojeada::Foto { .. }));
            match vista {
                // Un dibujo se ensena dibujado. Sobre papel blanco y no
                // sobre el gris de la celda: los trazos vienen de un lienzo
                // claro y sobre gris se pierden, igual que en las burbujas.
                Some(vista) => {
                    p.rellenar_redondeado(rf(celda), 4.0 * e, tema.papel);
                    p.empujar_recorte(rf(celda));
                    let dentro = encoger(rf(celda), 4.0 * e);
                    match vista {
                        Ojeada::Lienzo(l) => pintar_lienzo(p, l, dentro, Some(c.previas)),
                        Ojeada::Tabla(t) => pintar_ojeada_tabla(p, tema, escala, t, dentro),
                        Ojeada::Foto { .. } => {}
                    }
                    p.soltar_recorte();
                }
                // Lo que no tiene miniatura ensena su nombre: es mejor que
                // un cuadro vacio que no dice de que es.
                None => {
                    p.rellenar_redondeado(rf(celda), 4.0 * e, tema.burbuja_otra);
                    p.texto_linea(
                        &m.resumen(),
                        celda.x as f32 + 6.0 * e,
                        celda.y as f32 + 6.0 * e,
                        chat::CONTADOR_TAM * e,
                        celda.ancho as f32 - 12.0 * e,
                        tema.apagado,
                    );
                }
            }
        }
    } else {
        let alto = info::ARCHIVO_ALTO * escala / 100;
        a.alto_info.set(alto * suyos.len() as u32);
        let primera = (a.scroll_info / alto.max(1) as i32).max(0) as usize;
        let caben = (i.contenido.alto / alto.max(1)) as usize + 2;
        for (n, indice) in suyos.iter().copied().enumerate().skip(primera).take(caben) {
            let m = &a.mensajes[indice];
            let fila = Rect {
                x: i.contenido.x,
                y: i.contenido.y + (n as u32 * alto) as i32 - a.scroll_info,
                ancho: i.contenido.ancho,
                alto,
            };
            let f = info::fila_archivo(fila, escala);
            // La miniatura solo si hay fichero: una nota no tiene ninguno, y
            // un recuadro vacio al lado de un texto parece algo que no cargo.
            let foto = ruta_del_mensaje(&a.raiz, &a.ficha.id, m).and_then(|r| c.miniaturas.ya(&r));
            match foto {
                Some((b, w, h)) => {
                    p.empujar_recorte(rf(f.miniatura));
                    crate::miniaturas::pintar_recortado(p, b, rf(f.miniatura), w, h);
                    p.soltar_recorte();
                }
                None if m.ruta.is_some() || !m.nombre.is_empty() => {
                    p.rellenar_redondeado(rf(f.miniatura), 6.0 * e, tema.burbuja_otra)
                }
                // Una nota no tiene fichero, y un recuadro vacio al lado de
                // un texto parece algo que no cargo.
                None => {}
            }
            // Lo encontrado va en negrita. Un fondo de color seria mas
            // parecido a Telegram, pero para eso hace falta preguntarle a
            // DirectWrite donde cae cada trozo ya partido en lineas; la
            // negrita dice lo mismo y se ve en una sola pasada.
            let nombre = m.resumen();
            p.parrafo(
                &nombre,
                f.nombre.x as f32,
                f.nombre.y as f32,
                info::ARCHIVO_NOMBRE_TAM * e,
                f.ancho_texto as f32,
                &negritas(&nombre, &a.busqueda_info),
                tema.texto,
            );
            if let Some(etiqueta) = clase_de(m, textos) {
                p.texto_linea(
                    &etiqueta,
                    f.estado.x as f32,
                    f.estado.y as f32,
                    info::ARCHIVO_ESTADO_TAM * e,
                    f.ancho_texto as f32,
                    tema.apagado,
                );
            }
            p.texto_linea(
                &pixpin_ui::chat::etiqueta_hora(pixpin_shell::entorno::a_local(m.cuando), c.ahora),
                f.fecha.x as f32,
                f.fecha.y as f32,
                info::ARCHIVO_ESTADO_TAM * e,
                f.ancho_texto as f32,
                tema.apagado,
            );
        }
    }
    p.soltar_recorte();
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_tamano_se_ensena_como_lo_diria_el_explorador() {
        // De mil en mil, no de 1024: el usuario acaba de ver este numero en
        // el Explorador y discrepar con el es peor que ser exacto.
        assert_eq!(en_bytes(0), "0 B");
        assert_eq!(en_bytes(999), "999 B");
        assert_eq!(en_bytes(1_000), "1.0 kB");
        assert_eq!(en_bytes(2_500_000), "2.5 MB");
        assert_eq!(en_bytes(3_000_000_000), "3.0 GB");
    }

    #[test]
    fn un_tamano_enorme_no_se_queda_sin_unidad() {
        // Caso negativo: pasado el ultimo escalon se sigue en gigas en vez
        // de inventarse una unidad o dar la vuelta.
        assert!(
            en_bytes(u64::MAX).ends_with(" GB"),
            "{}",
            en_bytes(u64::MAX)
        );
    }

    #[test]
    fn sin_ficheros_no_se_pregunta_nada() {
        // Caso negativo: un cuadro que pregunta por cero ficheros no se
        // ensena, se descarta.
        assert!(Pendientes::de(Vec::new()).is_none());
        // Y con uno si, aunque no exista: su tamano sale cero y se dira al
        // intentar leerlo, que es donde de verdad se sabe.
        let p = Pendientes::de(vec![std::path::PathBuf::from("no-esta.png")])
            .expect("una ruta es una ruta");
        assert_eq!(p.rutas.len(), 1);
        assert_eq!(p.tamanos, [0]);
        assert!(p.pie.is_empty());
    }
}

/// Lo que el editor necesita saber de los ajustes de la aplicacion. Viaja
/// hasta el hilo del chat porque ahi no hay `Ajustes`: solo lo que el
/// lienzo usa.
#[derive(Debug, Clone, Copy)]
pub struct OpcionesLienzo {
    pub enganche: pixpin_motor2d::enganche::Ajustes,
    pub nivel: pixpin_nivel::Nivel,
    pub medir_fotogramas: bool,
}

/// Donde se guarda lo dibujado sobre una foto: a su lado, con el mismo
/// nombre y `.pixpin2d` detras (`fachada.jpg.pixpin2d`). La foto no se toca
/// nunca (D48), y el nombre entero evita que `a.jpg` y `a.png` compartan
/// dibujo.
fn dibujo_de_foto(foto: &std::path::Path) -> std::path::PathBuf {
    let mut s = foto.as_os_str().to_owned();
    s.push(".pixpin2d");
    std::path::PathBuf::from(s)
}

/// Abre el editor con la foto de fondo, centrada, y guarda lo dibujado al
/// cerrar. Bloquea el hilo del chat mientras dura, que es lo que se quiere:
/// el editor tapa la pantalla y el chat no tiene nada que hacer.
fn abrir_foto_en_lienzo(foto: &std::path::Path, opciones: OpcionesLienzo) {
    let fondo = match pixpin_codec::cargar(foto) {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!(?e, ruta = %foto.display(), "foto que no se pudo leer para el lienzo");
            return;
        }
    };
    let dibujo = dibujo_de_foto(foto);
    let (escena, habia) = match crate::pines::escena_para_lienzo(&dibujo) {
        Ok(v) => v,
        Err(e) => {
            // Abrir en blanco y guardar al cerrar pisaria lo que hubiera.
            tracing::error!(?e, ruta = %dibujo.display(), "dibujo corrupto; no se abre el lienzo");
            return;
        }
    };
    let resultado = crate::ventana_editor::abrir(
        escena,
        opciones.enganche,
        opciones.nivel,
        opciones.medir_fotogramas,
        Some(fondo),
        &[],
    );
    match resultado {
        Ok((escena, _)) => match crate::pines::guardar_lienzo(&dibujo, &escena, habia) {
            Ok(guardado) => {
                tracing::info!(guardado, ruta = %dibujo.display(), "lienzo de foto cerrado")
            }
            Err(e) => tracing::error!(?e, ruta = %dibujo.display(), "no se pudo guardar el lienzo"),
        },
        Err(e) => tracing::warn!(?e, "no se pudo abrir el lienzo de la foto"),
    }
}

#[cfg(test)]
mod pruebas_foto {
    use super::dibujo_de_foto;
    use std::path::Path;

    #[test]
    fn el_dibujo_de_una_foto_va_a_su_lado_con_el_nombre_entero() {
        let d = dibujo_de_foto(Path::new(r"C:\p\imagenes\fachada.jpg"));
        assert_eq!(d, Path::new(r"C:\p\imagenes\fachada.jpg.pixpin2d"));
    }

    #[test]
    fn las_flechas_de_sincronizar_son_un_trazado_que_se_entiende() {
        let tramos = pixpin_render::trayecto_svg::analizar(super::SINCRO.trazos[0].d).unwrap();
        assert!(tramos.len() > 6);
    }

    #[test]
    fn dos_fotos_con_el_mismo_nombre_y_otra_extension_no_comparten_dibujo() {
        assert_ne!(
            dibujo_de_foto(Path::new("a.jpg")),
            dibujo_de_foto(Path::new("a.png"))
        );
    }
}

/// Las dos flechas en redondo de sincronizar: «refresh» de Tabler Icons (MIT,
/// ver THIRD-PARTY-NOTICES.md), como el clip.
const SINCRO: pixpin_render::icono::Icono = pixpin_render::icono::Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[pixpin_render::icono::TrazoIcono {
        d: "M20 11a8.1 8.1 0 0 0 -15.5 -2m-.5 -4v4h4M4 13a8.1 8.1 0 0 0 15.5 2m.5 4v-4h-4",
        relleno: pixpin_render::icono::Pintura::Nada,
        trazo: pixpin_render::icono::Pintura::Actual,
        grosor: 2.0,
        extremo_redondo: true,
        union_redonda: true,
        opacidad: 1.0,
        par_impar: false,
        matriz: None,
        mascara: None,
    }],
};

// --- La hoja de calculo, dentro de la conversacion ---------------------

const VK_TAB: u32 = 0x09;
const VK_SUPR: u32 = 0x2E;
const VK_IZQUIERDA: u32 = 0x25;
const VK_ARRIBA: u32 = 0x26;
const VK_DERECHA: u32 = 0x27;
const VK_ABAJO: u32 = 0x28;

/// El hueco de la hoja: la conversacion menos su cabecera. La cabecera se
/// queda porque es donde se lee de que proyecto es y por donde se vuelve.
fn hueco_hoja(d: &Disposicion) -> Rect {
    let arriba = d.cabecera_chat.abajo();
    Rect {
        x: d.chat.x,
        y: arriba,
        ancho: d.chat.ancho,
        alto: (d.chat.abajo() - arriba).max(0) as u32,
    }
}

fn disposicion_hoja(d: &Disposicion, escala: u32) -> pixpin_ui::tabla::Disposicion {
    pixpin_ui::tabla::Disposicion::calcular(hueco_hoja(d), escala)
}

impl HojaAbierta {
    /// Deja escrito en la tabla lo que se estaba tecleando.
    ///
    /// Escribir la misma cadena que ya estaba NO cuenta como tocarla: entrar
    /// en una celda y salir sin cambiar nada no puede marcar el proyecto como
    /// modificado ni reescribir el cuaderno.
    fn confirmar(&mut self) {
        let Some(texto) = self.edicion.take() else {
            return;
        };
        let texto = texto.trim().to_string();
        if self.tabla.celda(self.sel) != texto {
            self.tabla.poner(self.sel, &texto);
            self.tocada = true;
        }
    }

    /// Mueve la celda elegida y deja que la hoja la siga. Confirma antes: las
    /// flechas y el tabulador salen de la celda, como en cualquier hoja.
    fn mover(&mut self, dx: i32, dy: i32, d: &pixpin_ui::tabla::Disposicion, escala: u32) {
        self.confirmar();
        let columna = (self.sel.columna as i64 + dx as i64).max(0) as u32;
        let fila = (self.sel.fila as i64 + dy as i64).max(0) as u32;
        self.sel = pixpin_proyecto::tabla::Ref { columna, fila };
        let (x, y) = d.seguir(columna, fila, self.scroll_x, self.scroll_y, escala);
        self.scroll_x = x;
        self.scroll_y = y;
    }

    /// Lo que se ensena en la barra de formulas: lo que se teclea, o lo
    /// guardado tal cual. En la barra va la FORMULA, no su resultado: es
    /// donde se mira lo que de verdad hay escrito.
    fn en_la_barra(&self) -> &str {
        match &self.edicion {
            Some(t) => t,
            None => self.tabla.celda(self.sel),
        }
    }
}

/// Abre la tabla de un mensaje para escribir en ella. No hace nada si ese
/// mensaje no es una tabla o si su texto no se entiende: abrirla vacia y
/// guardarla al cerrar pisaria lo que hubiera.
fn abrir_hoja(a: &mut Abierto, indice: usize) {
    let Some(m) = a.mensajes.get(indice) else {
        return;
    };
    match pixpin_proyecto::tabla::Tabla::leer(&m.texto) {
        Ok(tabla) => {
            a.hoja = Some(HojaAbierta {
                indice,
                tabla,
                sel: pixpin_proyecto::tabla::Ref {
                    columna: 0,
                    fila: 0,
                },
                edicion: None,
                scroll_x: 0,
                scroll_y: 0,
                tocada: false,
            })
        }
        Err(e) => tracing::warn!(?e, "tabla que no se entiende; no se abre para no pisarla"),
    }
}

/// Cierra lo que ocupe el sitio del historial —la hoja de calculo o una
/// mini-aplicacion— guardandolo si se toco.
///
/// Uno solo para los dos porque el sitio es uno solo: quien cambia de
/// proyecto, cierra la ventana o pulsa «volver» no tiene que acordarse de
/// cual de las dos cosas habia abierta.
fn cerrar_panel(ubicacion: &Ubicacion, a: &mut Abierto) {
    cerrar_hoja(ubicacion, a);
    cerrar_mini(ubicacion, a);
}

/// Cierra la hoja, guardandola si se toco.
fn cerrar_hoja(ubicacion: &Ubicacion, a: &mut Abierto) {
    let Some(mut h) = a.hoja.take() else {
        return;
    };
    h.confirmar();
    if !h.tocada {
        return;
    }
    if let Err(e) = guardar_hoja(ubicacion, a, &mut h) {
        tracing::error!(
            ?e,
            "no se pudo guardar la tabla; el mensaje sigue como estaba"
        );
    }
}

/// Escribe la tabla en su mensaje del cuaderno y refresca lo que se ve.
fn guardar_hoja(ubicacion: &Ubicacion, a: &mut Abierto, h: &mut HojaAbierta) -> Result<()> {
    h.tabla.tocado = pixpin_shell::entorno::ahora_utc_ms();
    let texto = h
        .tabla
        .escribir()
        .context("tabla que no se pudo escribir")?;
    let Some(m) = a.mensajes.get_mut(h.indice) else {
        return Ok(());
    };
    m.texto = texto;
    let mensaje = m.clone();
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    if !pixpin_proyecto::cuaderno::reemplazar(&carpeta, &mensaje)? {
        // No estaba: mejor anadirlo que perderlo.
        pixpin_proyecto::cuaderno::anadir(&carpeta, &mensaje)?;
    }
    a.vistas[h.indice] = Some(Ojeada::Tabla(Box::new(h.tabla.clone())));
    // La ojeada de la burbuja cambio de tamano: hay que volver a colocar.
    a.colocado.borrow_mut().ancho = 0;
    Ok(())
}

/// La hoja de calculo en el sitio del historial.
///
/// En la celda va el RESULTADO (una formula ensena su numero) y en la barra
/// de arriba, lo escrito. Es lo que hace cualquier hoja, y sin ello una hoja
/// llena de `=SUMA(...)` no se podria leer de un vistazo.
fn pintar_hoja(p: &Pintor, d: &Disposicion, c: &Pinta, h: &HojaAbierta) {
    use pixpin_proyecto::formula;
    use pixpin_proyecto::tabla::{Ref, ref_a};
    use pixpin_ui::tabla as ui;
    let (tema, escala) = (c.tema, c.escala);
    let e = escala as f32 / 100.0;
    let t = disposicion_hoja(d, escala);
    let linea = (1.0 * e).max(1.0);

    p.rellenar(rf(t.hoja), tema.papel);
    // La barra de formulas y las cabeceras, en el gris de la cabecera del
    // chat: son marco, no dato.
    p.rellenar(rf(t.barra_formulas), tema.cabecera);
    p.rellenar(rf(t.esquina), tema.cabecera);
    p.rellenar(rf(t.cabecera_columnas), tema.cabecera);
    p.rellenar(rf(t.cabecera_filas), tema.cabecera);

    // El nombre de la celda elegida y lo que tiene escrito.
    let tam = ui::BARRA_FORMULAS_TAM * e;
    let (_, alto_texto) = p.medir_texto("A1", tam);
    let y = t.barra_formulas.y as f32 + (t.barra_formulas.alto as f32 - alto_texto) / 2.0;
    let x = t.barra_formulas.x as f32 + ui::BARRA_FORMULAS_X as f32 * e;
    p.texto(&ref_a(h.sel), x, y, tam, tema.apagado);
    // Por donde se vuelve, a la derecha del todo: sin la caja de escribir
    // ni el historial delante, nada mas dice como salir de la hoja.
    let volver = c.textos.t("hoja-volver");
    let (ancho_volver, _) = p.medir_texto(&volver, tam);
    p.texto(
        &volver,
        t.barra_formulas.derecha() as f32 - ancho_volver - ui::BARRA_FORMULAS_X as f32 * e,
        y,
        tam,
        tema.apagado,
    );
    let x_texto = x + 46.0 * e;
    p.texto_linea(
        h.en_la_barra(),
        x_texto,
        y,
        tam,
        (t.barra_formulas.derecha() as f32 - x_texto - ancho_volver - 20.0 * e).max(0.0),
        tema.texto_papel,
    );

    let (columnas, filas) = h.tabla.tamano();
    let (primera_columna, cuantas_columnas, primera_fila, cuantas_filas) =
        t.visibles(h.scroll_x, h.scroll_y, escala);

    p.empujar_recorte(rf(t.celdas));
    for fila in primera_fila..primera_fila + cuantas_filas {
        for columna in primera_columna..primera_columna + cuantas_columnas {
            let r = Ref { columna, fila };
            let caja = t.celda(columna, fila, h.scroll_x, h.scroll_y, escala);
            // Las lineas de la rejilla: solo dos por celda, la de la derecha
            // y la de abajo, que son las que comparten con sus vecinas.
            p.rellenar(
                RectF {
                    x: caja.derecha() as f32 - linea,
                    y: caja.y as f32,
                    ancho: linea,
                    alto: caja.alto as f32,
                },
                tema.separador,
            );
            p.rellenar(
                RectF {
                    x: caja.x as f32,
                    y: caja.abajo() as f32 - linea,
                    ancho: caja.ancho as f32,
                    alto: linea,
                },
                tema.separador,
            );
            let escribiendo = h.edicion.is_some() && r == h.sel;
            let texto = if escribiendo {
                h.en_la_barra().to_string()
            } else {
                let guardado = h.tabla.celda(r);
                if guardado.is_empty() {
                    continue;
                }
                formula::evaluar(&h.tabla, r).to_string()
            };
            if texto.is_empty() {
                continue;
            }
            let (tx, ancho) = t.texto_celda(caja, escala);
            let tam = ui::CELDA_TAM * e;
            let (_, alto_texto) = p.medir_texto(&texto, tam);
            p.texto_linea(
                &texto,
                tx as f32,
                caja.y as f32 + (caja.alto as f32 - alto_texto) / 2.0,
                tam,
                ancho as f32,
                tema.texto_papel,
            );
        }
    }

    // El recuadro de la celda elegida, encima de la rejilla y de dos pixeles:
    // con uno se confunde con las lineas de al lado.
    let caja = t.celda(h.sel.columna, h.sel.fila, h.scroll_x, h.scroll_y, escala);
    let grueso = (2.0 * e).max(2.0);
    for lado in [
        RectF {
            x: caja.x as f32,
            y: caja.y as f32,
            ancho: caja.ancho as f32,
            alto: grueso,
        },
        RectF {
            x: caja.x as f32,
            y: caja.abajo() as f32 - grueso,
            ancho: caja.ancho as f32,
            alto: grueso,
        },
        RectF {
            x: caja.x as f32,
            y: caja.y as f32,
            ancho: grueso,
            alto: caja.alto as f32,
        },
        RectF {
            x: caja.derecha() as f32 - grueso,
            y: caja.y as f32,
            ancho: grueso,
            alto: caja.alto as f32,
        },
    ] {
        p.rellenar(lado, tema.enviar);
    }
    p.soltar_recorte();

    // Los rotulos: las letras arriba y los numeros a la izquierda. Se pintan
    // DESPUES de las celdas para que el desplazamiento no las tape.
    let tam = ui::CABECERA_TAM * e;
    p.empujar_recorte(rf(t.cabecera_columnas));
    for columna in primera_columna..primera_columna + cuantas_columnas {
        let caja = t.cabecera_de_columna(columna, h.scroll_x, escala);
        let rotulo = ref_a(Ref { columna, fila: 0 });
        let rotulo = rotulo.trim_end_matches('1');
        let (w, alto_texto) = p.medir_texto(rotulo, tam);
        let color = if columna == h.sel.columna {
            tema.texto_papel
        } else {
            tema.apagado
        };
        p.texto(
            rotulo,
            caja.x as f32 + (caja.ancho as f32 - w) / 2.0,
            caja.y as f32 + (caja.alto as f32 - alto_texto) / 2.0,
            tam,
            color,
        );
    }
    p.soltar_recorte();
    p.empujar_recorte(rf(t.cabecera_filas));
    for fila in primera_fila..primera_fila + cuantas_filas {
        let caja = t.cabecera_de_fila(fila, h.scroll_y, escala);
        let rotulo = (fila + 1).to_string();
        let (w, alto_texto) = p.medir_texto(&rotulo, tam);
        let color = if fila == h.sel.fila {
            tema.texto_papel
        } else {
            tema.apagado
        };
        p.texto(
            &rotulo,
            caja.x as f32 + (caja.ancho as f32 - w) / 2.0,
            caja.y as f32 + (caja.alto as f32 - alto_texto) / 2.0,
            tam,
            color,
        );
    }
    p.soltar_recorte();
    let _ = (columnas, filas);
}

#[cfg(test)]
mod pruebas_hoja {
    use super::HojaAbierta;
    use pixpin_geom::Rect;
    use pixpin_proyecto::tabla::{Ref, Tabla};

    fn hoja() -> HojaAbierta {
        HojaAbierta {
            indice: 0,
            tabla: Tabla::default(),
            sel: Ref {
                columna: 0,
                fila: 0,
            },
            edicion: None,
            scroll_x: 0,
            scroll_y: 0,
            tocada: false,
        }
    }

    fn rejilla() -> pixpin_ui::tabla::Disposicion {
        pixpin_ui::tabla::Disposicion::calcular(
            Rect {
                x: 0,
                y: 0,
                ancho: 600,
                alto: 400,
            },
            100,
        )
    }

    #[test]
    fn escribir_en_una_celda_la_deja_escrita_y_marca_la_tabla() {
        let mut h = hoja();
        h.edicion = Some("  12  ".into());
        h.confirmar();
        assert_eq!(h.tabla.celda(h.sel), "12", "se guarda sin los espacios");
        assert!(h.tocada);
    }

    #[test]
    fn volver_a_escribir_lo_mismo_no_marca_la_tabla() {
        // Si contara como cambio, entrar en una celda y salir reescribiria el
        // cuaderno entero y el proyecto subiria de sitio en la lista.
        let mut h = hoja();
        h.tabla.poner(h.sel, "12");
        h.edicion = Some("12".into());
        h.confirmar();
        assert!(!h.tocada);
    }

    #[test]
    fn una_celda_vaciada_desaparece_del_mapa() {
        // Una cadena vacia guardada viajaria por la red como un cambio, y la
        // tabla creceria con celdas que no tienen nada.
        let mut h = hoja();
        h.tabla.poner(h.sel, "12");
        h.edicion = Some(String::new());
        h.confirmar();
        assert!(h.tabla.celdas.is_empty());
        assert!(h.tocada);
    }

    #[test]
    fn moverse_confirma_lo_tecleado_y_no_se_sale_de_la_hoja() {
        let mut h = hoja();
        h.edicion = Some("7".into());
        h.mover(0, 1, &rejilla(), 100);
        assert_eq!(
            h.tabla.celda(Ref {
                columna: 0,
                fila: 0
            }),
            "7"
        );
        assert_eq!(
            h.sel,
            Ref {
                columna: 0,
                fila: 1
            }
        );

        // Caso negativo: arriba y a la izquierda del todo no hay nada, y una
        // resta sin sujetar daria la vuelta al `u32`.
        h.mover(-1, -1, &rejilla(), 100);
        assert_eq!(
            h.sel,
            Ref {
                columna: 0,
                fila: 0
            }
        );
        h.mover(-1, -1, &rejilla(), 100);
        assert_eq!(
            h.sel,
            Ref {
                columna: 0,
                fila: 0
            }
        );
    }

    #[test]
    fn la_barra_ensena_la_formula_y_no_su_resultado() {
        let mut h = hoja();
        h.tabla.poner(h.sel, "=1+1");
        assert_eq!(h.en_la_barra(), "=1+1");
        h.edicion = Some("=2+2".into());
        assert_eq!(h.en_la_barra(), "=2+2", "y lo que se teclea manda");
    }
}

// --- El panel de una mini-aplicacion ----------------------------------

/// Una mini-aplicacion abierta para tocarla, en el sitio del historial.
///
/// Es la misma decision que con la hoja de calculo: **dentro de la
/// conversacion y no en una ventana aparte**. Una tarea marcada, un gasto
/// apuntado o un cronometro en marcha son del proyecto, no de la aplicacion,
/// y siete ventanas sueltas llenarian el escritorio.
struct MiniAbierta {
    /// Que mensaje del historial es. El documento ES su texto, asi que
    /// guardar es reescribir ese mensaje.
    indice: usize,
    /// La palabra de `Mensaje.miniapp`: es lo que decide que panel sale.
    cual: String,
    documento: String,
    /// Lo que se esta tecleando en la caja de abajo.
    borrador: String,
    scroll: i32,
    /// Si cambio algo. Sin esto, abrir una lista y cerrarla la reescribiria
    /// en el disco para nada, y el resumen de sincronizacion cambiaria con
    /// ella.
    tocada: bool,
    /// A quien le toco en el ultimo sorteo de la ruleta.
    ///
    /// **No se guarda en el documento**, igual que en el movil
    /// (`mini/Contador.kt:68-91` no tiene donde ponerlo): vive mientras el
    /// panel esta abierto y se pierde al cerrarlo.
    elegido: Option<String>,
}

/// La moneda con la que nace una hoja de gastos.
///
/// Sale del catalogo y no del sistema porque el catalogo ya es el idioma
/// elegido: un PixPin en castellano apunta en euros aunque Windows este en
/// ingles. En cuanto el documento se guarda deja de importar, porque la
/// moneda viaja dentro de el (`gastos::Libro`).
fn moneda_del_catalogo(textos: &Catalogo) -> pixpin_proyecto::mini::gastos::Moneda {
    use pixpin_proyecto::mini::gastos::Moneda;
    Moneda::de_codigo(textos.t("mini-moneda").trim()).unwrap_or_else(Moneda::euro)
}

impl MiniAbierta {
    /// Lo que hay que pintar ahora mismo. Se recalcula del documento en cada
    /// fotograma a proposito: el documento es la unica verdad, y un estado
    /// aparte se desincronizaria en cuanto algo lo tocara por otro camino.
    ///
    /// La hora se pregunta aqui dentro y **en UTC**, no se hereda de `Pinta`:
    /// alli es la hora local y se calcula una sola vez al abrir la ventana,
    /// que es justo lo contrario de lo que necesita un cronometro. Los
    /// documentos guardan `System.currentTimeMillis()` del movil, que es UTC.
    fn vista(&self, textos: &Catalogo) -> Option<crate::mini_panel::Vista> {
        crate::mini_panel::vista(
            &self.cual,
            &self.documento,
            &moneda_del_catalogo(textos),
            pixpin_shell::entorno::ahora_utc_ms(),
        )
    }

    /// Hace la orden y deja el documento nuevo. Devuelve si cambio algo.
    fn hacer(&mut self, orden: &crate::mini_panel::Orden, textos: &Catalogo) -> bool {
        use crate::mini_panel::Orden;
        // Girar no toca el documento; lo que cambia es a quien le toco. Para
        // quitarlo de la lista esta su propia aspa, que ya esta ahi: un
        // segundo boton para lo mismo solo esconderia el primero.
        if let Orden::Girar(azar) = orden {
            self.elegido = crate::mini_panel::sorteo(&self.documento, *azar).and_then(|n| {
                pixpin_proyecto::mini::ruleta::leer(&self.documento)
                    .get(n)
                    .cloned()
            });
            return true;
        }
        let nuevo = crate::mini_panel::aplicar(
            &self.cual,
            &self.documento,
            &moneda_del_catalogo(textos),
            orden,
            pixpin_shell::entorno::ahora_utc_ms(),
        );
        if nuevo == self.documento {
            return false;
        }
        self.documento = nuevo;
        self.tocada = true;
        true
    }
}

/// Abre la mini-app de un mensaje. No hace nada si no la conocemos: una de
/// una version futura se queda en la lista como texto, que es lo que es.
fn abrir_mini(a: &mut Abierto, indice: usize, textos: &Catalogo) -> bool {
    let Some(m) = a.mensajes.get(indice) else {
        return false;
    };
    let Some(cual) = m.miniapp.as_deref() else {
        return false;
    };
    if crate::mini_panel::vista(cual, &m.texto, &moneda_del_catalogo(textos), 0).is_none() {
        return false;
    }
    a.mini = Some(MiniAbierta {
        indice,
        cual: cual.to_string(),
        documento: m.texto.clone(),
        borrador: String::new(),
        scroll: 0,
        tocada: false,
        elegido: None,
    });
    true
}

/// Cierra el panel, guardandolo si se toco.
fn cerrar_mini(ubicacion: &Ubicacion, a: &mut Abierto) {
    let Some(m) = a.mini.take() else {
        return;
    };
    if !m.tocada {
        return;
    }
    if let Err(e) = guardar_mini(ubicacion, a, &m) {
        tracing::error!(?e, "no se pudo guardar la mini-app; sigue como estaba");
    }
}

/// Escribe el documento en su mensaje del cuaderno.
///
/// Por `cuaderno::reemplazar`, que **copia tal cual las lineas que no
/// entiende**: marcar una casilla no puede llevarse por delante lo que
/// escribio una version mas nueva del movil.
fn guardar_mini(ubicacion: &Ubicacion, a: &mut Abierto, m: &MiniAbierta) -> Result<()> {
    let Some(mensaje) = a.mensajes.get_mut(m.indice) else {
        return Ok(());
    };
    mensaje.texto = m.documento.clone();
    let mensaje = mensaje.clone();
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    if !pixpin_proyecto::cuaderno::reemplazar(&carpeta, &mensaje)? {
        // No estaba: mejor anadirlo que perderlo.
        pixpin_proyecto::cuaderno::anadir(&carpeta, &mensaje)?;
    }
    // El resumen de la burbuja cambio: hay que volver a colocar.
    a.colocado.borrow_mut().ancho = 0;
    Ok(())
}

/// Donde cayo el raton dentro del panel.
enum ZonaMini {
    Boton(crate::mini_panel::Orden),
    Marcar(usize),
    Borrar(usize),
}

/// Que se pulso, calculado igual al pintar y al hacer clic.
///
/// Se resuelve aqui y no en el bucle para que las dos mitades —lo que se
/// dibuja y lo que responde— salgan de la misma cuenta: un boton pintado en
/// un sitio y pulsable en otro es el fallo clasico de estas pantallas.
fn zona_mini(
    v: &crate::mini_panel::Vista,
    d: &pixpin_ui::mini::Disposicion,
    m: &MiniAbierta,
    l: Punto,
    escala: u32,
) -> Option<ZonaMini> {
    for (fila, botones) in v.filas_de_botones.iter().enumerate() {
        if let Some(n) = d.cual_boton(l, fila as u32, botones.len(), escala)
            && botones[n].activo
        {
            return Some(ZonaMini::Boton(botones[n].orden.clone()));
        }
    }
    let n = d.cual_fila(l, v.lista.len(), m.scroll, escala)?;
    let caja = d.fila(n, m.scroll, escala);
    if v.lista[n].se_borra && d.aspa(caja, escala).contiene(l) {
        return Some(ZonaMini::Borrar(n));
    }
    v.lista[n].se_marca.then_some(ZonaMini::Marcar(n))
}

/// Atiende un clic dentro del panel.
fn pulsar_mini(a: &mut Abierto, l: Punto, d: &Disposicion, escala: u32, textos: &Catalogo) {
    use crate::mini_panel::Orden;
    let Some(m) = a.mini.as_mut() else {
        return;
    };
    let Some(v) = m.vista(textos) else {
        return;
    };
    let t = pixpin_ui::mini::Disposicion::calcular(hueco_hoja(d), escala, v.reparto);
    let Some(zona) = zona_mini(&v, &t, m, l, escala) else {
        return;
    };
    let orden = match zona {
        ZonaMini::Marcar(n) => Orden::Alternar(n),
        ZonaMini::Borrar(n) => Orden::Quitar(n),
        // El azar se siembra con el reloj y se inyecta: el sorteo en si es
        // puro y se comprueba en `mini_panel`, que es donde vive la regla.
        ZonaMini::Boton(Orden::Girar(_)) => {
            let mut azar =
                pixpin_motor2d::azar::Azar::nuevo(pixpin_shell::entorno::ahora_utc_ms() as u32);
            Orden::Girar(azar.siguiente() as f64)
        }
        ZonaMini::Boton(otra) => otra,
    };
    m.hacer(&orden, textos);
}

/// El panel de la mini-app en el sitio del historial.
fn pintar_mini(p: &Pintor, d: &Disposicion, c: &Pinta, m: &MiniAbierta) {
    use crate::mini_panel::Rotulo;
    use pixpin_ui::mini as ui;
    let (tema, escala) = (c.tema, c.escala);
    let e = escala as f32 / 100.0;
    let Some(v) = m.vista(c.textos) else {
        return;
    };
    let t = pixpin_ui::mini::Disposicion::calcular(hueco_hoja(d), escala, v.reparto);
    p.rellenar(rf(t.panel), tema.papel);

    // La cabecera: el nombre del documento y por donde se sale. Sin la caja
    // de escribir ni el historial delante, nada mas dice como volver.
    let tam_titulo = ui::TITULO_TAM * e;
    let titulo = pixpin_proyecto::mini::titulo(&m.documento);
    let (_, alto_titulo) = p.medir_texto("Ag", tam_titulo);
    let y_titulo = t.cabecera.y as f32 + (t.cabecera.alto as f32 - alto_titulo) / 2.0;
    let margen = ui::MARGEN as f32 * e;
    let volver = c.textos.t("hoja-volver");
    let (ancho_volver, _) = p.medir_texto(&volver, tam_titulo);
    p.texto_linea(
        &titulo,
        t.cabecera.x as f32 + margen,
        y_titulo,
        tam_titulo,
        (t.cabecera.ancho as f32 - margen * 2.0 - ancho_volver - 12.0 * e).max(0.0),
        tema.texto_papel,
    );
    p.texto(
        &volver,
        t.cabecera.derecha() as f32 - ancho_volver - margen,
        y_titulo,
        tam_titulo,
        tema.apagado,
    );

    // El tablero: el numero grande. Es lo que se lee desde lejos, y por eso
    // es lo unico que no cede sitio cuando la ventana es baja.
    if t.tablero.alto > 0 {
        let texto = match (v.tablero.is_empty(), m.elegido.as_deref()) {
            // La ruleta no tiene numero: su tablero es a quien le ha tocado.
            (true, Some(quien)) => quien.to_string(),
            (true, None) => String::new(),
            _ => v.tablero.clone(),
        };
        if !texto.is_empty() {
            let tam = ui::TABLERO_TAM * e;
            let (ancho, alto) = p.medir_texto(&texto, tam);
            p.texto(
                &texto,
                t.tablero.x as f32 + (t.tablero.ancho as f32 - ancho) / 2.0,
                t.tablero.y as f32 + (t.tablero.alto as f32 - alto) / 2.0,
                tam,
                if v.alerta {
                    hex(0xe5534b)
                } else {
                    tema.texto_papel
                },
            );
        }
    }

    // Los botones.
    let tam_boton = ui::BOTON_TAM * e;
    for (fila, botones) in v.filas_de_botones.iter().enumerate() {
        for (n, b) in botones.iter().enumerate() {
            let caja = rf(t.boton(fila as u32, n, botones.len(), escala));
            let radio = 8.0 * e;
            if b.principal && b.activo {
                p.rellenar_redondeado(caja, radio, tema.enviar);
            } else {
                p.rellenar_redondeado(caja, radio, tema.pildora_borde);
                p.rellenar_redondeado(encoger(caja, 1.0), (radio - 1.0).max(0.0), tema.papel);
            }
            let rotulo = match &b.rotulo {
                Rotulo::Clave(k) => c.textos.t(k),
                Rotulo::Tal(s) => s.clone(),
            };
            let (ancho, alto) = p.medir_texto(&rotulo, tam_boton);
            let color = match (b.activo, b.principal) {
                (false, _) => tema.apagado,
                (true, true) => tema.papel,
                (true, false) => tema.texto_papel,
            };
            p.texto_linea(
                &rotulo,
                caja.x + (caja.ancho - ancho).max(0.0) / 2.0,
                caja.y + (caja.alto - alto) / 2.0,
                tam_boton,
                caja.ancho,
                color,
            );
        }
    }

    // La lista, lo unico que se desplaza.
    if t.lista.alto > 0 {
        let tam = ui::FILA_TAM * e;
        p.empujar_recorte(rf(t.lista));
        for (n, f) in v.lista.iter().enumerate() {
            let caja = t.fila(n, m.scroll, escala);
            // Lo que queda fuera no se mide siquiera: una lista de mil
            // nombres no puede costar mil medidas de texto por fotograma.
            if caja.abajo() < t.lista.y || caja.y > t.lista.abajo() {
                continue;
            }
            let caja_f = rf(caja);
            let (_, alto) = p.medir_texto("Ag", tam);
            let y = caja_f.y + (caja_f.alto - alto) / 2.0;
            let color = if f.hecha {
                tema.apagado
            } else {
                tema.texto_papel
            };
            let mut derecha = caja_f.x + caja_f.ancho;
            if f.se_borra {
                let aspa = rf(t.aspa(caja, escala));
                p.icono(&mi::CLOSE, encoger(aspa, 7.0 * e), tema.apagado);
                derecha = aspa.x;
            }
            if !f.detalle.is_empty() {
                let (ancho_d, _) = p.medir_texto(&f.detalle, tam);
                p.texto(&f.detalle, derecha - ancho_d - 6.0 * e, y, tam, color);
                derecha -= ancho_d + 12.0 * e;
            }
            // La casilla de una tarea, con su marca. El tachado del movil no
            // se imita con una raya: se apaga el color, que es lo que este
            // pintor sabe hacer y se lee igual de rapido.
            let mut x = caja_f.x;
            if f.se_marca {
                let icono: &'static Icono = if f.hecha {
                    &mi::CHECK_BOX
                } else {
                    &mi::CHECK_BOX_OUTLINE_BLANK
                };
                p.icono(
                    icono,
                    RectF {
                        x,
                        y: caja_f.y + (caja_f.alto - 18.0 * e) / 2.0,
                        ancho: 18.0 * e,
                        alto: 18.0 * e,
                    },
                    if f.hecha { tema.enviar } else { tema.apagado },
                );
                x += 26.0 * e;
            }
            p.texto_linea(&f.texto, x, y, tam, (derecha - x).max(0.0), color);
            p.rellenar(
                RectF {
                    x: caja_f.x,
                    y: caja_f.y + caja_f.alto - 1.0,
                    ancho: caja_f.ancho,
                    alto: 1.0,
                },
                tema.separador,
            );
        }
        p.soltar_recorte();
    }

    // La caja de escribir, abajo.
    if t.anadir.alto > 0 {
        let tam = ui::ANADIR_TAM * e;
        let caja = rf(t.anadir);
        p.rellenar(caja, tema.cabecera);
        let (_, alto) = p.medir_texto("Ag", tam);
        let y = caja.y + (caja.alto - alto) / 2.0;
        let (texto, color) = if m.borrador.is_empty() {
            (
                v.guia.map(|g| c.textos.t(g)).unwrap_or_default(),
                tema.apagado,
            )
        } else {
            // El cursor va pegado a lo escrito: no hay seleccion ni flechas
            // en esta caja, asi que una barra fija basta y no parpadea.
            (format!("{}|", m.borrador), tema.texto)
        };
        p.texto_linea(
            &texto,
            caja.x + margen,
            y,
            tam,
            (caja.ancho - margen * 2.0).max(0.0),
            color,
        );
    }
}

/// El menu del clic derecho sobre un proyecto de la lista. `true` si hay que
/// borrar: se eligio «Borrar» Y se dijo que si a la pregunta.
fn menu_de_proyecto(ventana: &VentanaOverlay, textos: &Catalogo, cuantos: usize) -> bool {
    const BORRAR: u32 = 1;
    let rotulo = if cuantos > 1 {
        format!("{} ({cuantos})", textos.t("proyecto-borrar-varios"))
    } else {
        textos.t("proyecto-borrar")
    };
    if pixpin_shell::menu_llano(ventana.handle(), &[(BORRAR, rotulo.clone())]) != Some(BORRAR) {
        return false;
    }
    pixpin_shell::confirmar_destructivo(
        ventana.handle(),
        &rotulo,
        &textos.t("proyecto-borrar-aviso"),
    )
}

// --- El lienzo vivo dentro de la burbuja ------------------------------

/// El lienzo que esta VIVO ahora mismo: el que se pulso.
///
/// Los demas se quedan quietos en su burbuja —ya pintados, sin gastar nada—
/// y solo este recibe el raton. Es lo que pidio el usuario: «todos esos
/// canvas ya estan abiertos en modo bajo consumo, en estatico; solo en el
/// que estoy escribiendo es el que esta trabajando». Por eso no se abre
/// ninguna ventana al pulsar.
struct LienzoVivo {
    /// Que mensaje del historial es.
    indice: usize,
    escena: pixpin_motor2d::Escena,
    gesto: pixpin_motor2d::gesto::Gesto,
    /// Donde se guarda lo dibujado: el `.pixpin2d` de al lado de la foto. La
    /// foto no se toca nunca (D48).
    dibujo: std::path::PathBuf,
    /// Si ese fichero ya existia: decide si una escena vacia se guarda (D146).
    habia: bool,
    /// El tamano del documento, que son los pixeles de la foto. Se usa el
    /// tamano ORIGINAL y no el de la vista previa para que lo dibujado aqui
    /// caiga donde toca al abrir la misma foto en el editor grande.
    doc: (f32, f32),
    /// La caja en pantalla del ultimo pintado, para traducir el raton. La
    /// apunta quien pinta, que es el unico que sabe donde acabo la burbuja.
    destino: std::cell::Cell<RectF>,
    tocado: bool,
}

impl LienzoVivo {
    /// El punto de la pantalla en coordenadas del documento. `None` si cae
    /// fuera de la burbuja: ahi el clic es para salir, no para dibujar.
    fn punto(&self, l: Punto) -> Option<pixpin_motor2d::Punto2> {
        let d = self.destino.get();
        let escala = self.escala();
        if escala <= 0.0 {
            return None;
        }
        let (x, y) = (l.x as f32, l.y as f32);
        if x < d.x || x > d.x + d.ancho || y < d.y || y > d.y + d.alto {
            return None;
        }
        Some(pixpin_motor2d::Punto2::nuevo(
            (x - d.x) / escala,
            (y - d.y) / escala,
        ))
    }

    /// Cuanto mide en pantalla un pixel del documento.
    fn escala(&self) -> f32 {
        let d = self.destino.get();
        let (w, h) = self.doc;
        if w <= 0.0 || h <= 0.0 {
            return 0.0;
        }
        (d.ancho / w).min(d.alto / h)
    }
}

/// Enciende el lienzo de un mensaje con foto. Devuelve si se pudo.
///
/// Lee la foto entera una vez, solo para saber cuanto mide: las coordenadas
/// de lo que se dibuje tienen que ser las suyas, o el mismo dibujo se veria
/// corrido al abrirlo en el editor grande.
fn encender_lienzo(a: &mut Abierto, indice: usize) -> bool {
    if a.vivo.as_ref().is_some_and(|v| v.indice == indice) {
        return true;
    }
    if !matches!(a.vistas.get(indice), Some(Some(Ojeada::Foto { .. }))) {
        return false;
    }
    let Some(m) = a.mensajes.get(indice) else {
        return false;
    };
    let Some(foto) = ruta_del_mensaje(&a.raiz, &a.ficha.id, m) else {
        return false;
    };
    // El tamano ya lo midio la ojeada al abrir el proyecto.
    let Some(Some(Ojeada::Foto { doc, .. })) = a.vistas.get(indice) else {
        return false;
    };
    let doc = *doc;
    let dibujo = dibujo_de_foto(&foto);
    let (escena, habia) = match crate::pines::escena_para_lienzo(&dibujo) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(?e, ruta = %dibujo.display(), "dibujo corrupto; no se enciende");
            return false;
        }
    };
    a.vivo = Some(LienzoVivo {
        indice,
        escena,
        gesto: pixpin_motor2d::gesto::Gesto::nuevo(),
        dibujo,
        habia,
        doc,
        destino: std::cell::Cell::new(RectF {
            x: 0.0,
            y: 0.0,
            ancho: 0.0,
            alto: 0.0,
        }),
        tocado: false,
    });
    true
}

/// Apaga el lienzo vivo y guarda lo dibujado si cambio algo.
fn apagar_lienzo(ubicacion: &Ubicacion, a: &mut Abierto) {
    let Some(v) = a.vivo.take() else {
        return;
    };
    if !v.tocado {
        return;
    }
    match crate::pines::guardar_lienzo(&v.dibujo, &v.escena, v.habia) {
        Ok(guardado) => tracing::info!(
            guardado,
            elementos = v.escena.cuantos_visibles(),
            "lienzo de la burbuja guardado"
        ),
        Err(e) => tracing::error!(?e, ruta = %v.dibujo.display(), "no se pudo guardar el lienzo"),
    }
    // La burbuja vuelve a quedarse quieta, pero con lo dibujado: se rehace su
    // ojeada, o el trazo desapareceria al apagar el lienzo.
    if let Some(m) = a.mensajes.get(v.indice).cloned() {
        a.vistas[v.indice] = leer_vista(ubicacion, &a.ficha.id, &m);
    }
}

#[cfg(test)]
mod pruebas_lienzo_vivo {
    use super::*;

    fn vivo(destino: RectF, doc: (f32, f32)) -> LienzoVivo {
        LienzoVivo {
            indice: 0,
            escena: pixpin_motor2d::Escena::nueva(),
            gesto: pixpin_motor2d::gesto::Gesto::nuevo(),
            dibujo: std::path::PathBuf::from("x.pixpin2d"),
            habia: false,
            doc,
            destino: std::cell::Cell::new(destino),
            tocado: false,
        }
    }

    #[test]
    fn el_raton_se_traduce_a_coordenadas_de_la_foto() {
        // Una foto de 1000x500 vista en una caja de 500x250: la mitad de
        // grande. El centro de la caja es el centro de la foto.
        let v = vivo(
            RectF {
                x: 100.0,
                y: 40.0,
                ancho: 500.0,
                alto: 250.0,
            },
            (1000.0, 500.0),
        );
        assert_eq!(v.escala(), 0.5);
        let q = v.punto(Punto { x: 350, y: 165 }).expect("cae dentro");
        assert_eq!((q.x, q.y), (500.0, 250.0));
    }

    #[test]
    fn fuera_de_la_burbuja_no_hay_trazo() {
        // El clic de al lado es para apagar el lienzo, no para dibujar en el
        // borde: sin esto, pulsar en la conversacion pintaria una raya.
        let v = vivo(
            RectF {
                x: 100.0,
                y: 40.0,
                ancho: 500.0,
                alto: 250.0,
            },
            (1000.0, 500.0),
        );
        assert!(
            v.punto(Punto { x: 90, y: 165 }).is_none(),
            "por la izquierda"
        );
        assert!(v.punto(Punto { x: 350, y: 400 }).is_none(), "por abajo");
    }

    #[test]
    fn una_foto_sin_tamano_no_divide_por_cero() {
        let v = vivo(
            RectF {
                x: 0.0,
                y: 0.0,
                ancho: 100.0,
                alto: 100.0,
            },
            (0.0, 0.0),
        );
        assert_eq!(v.escala(), 0.0);
        assert!(v.punto(Punto { x: 10, y: 10 }).is_none());
    }
}

/// Cuantas paginas se extraen como mucho de un PDF.
///
/// El mismo tope que los pines: un PDF de doscientas paginas dejaria el
/// proyecto con doscientas imagenes de 1600 px y varios cientos de megas.
/// Se hacen las primeras y se dice cuantas quedaron fuera.
const TOPE_PAGINAS_PDF: u32 = 20;
/// A que ancho se dibuja cada pagina. Generoso: una pagina extraida es un
/// documento para leer, no una miniatura.
const ANCHO_PAGINA: u32 = 1600;

/// Un PDF entra como PROYECTO propio, con una hoja por pagina.
///
/// Es lo que pidio el usuario y lo que hace el movil: cada chat ES un
/// proyecto, y un PDF es un documento entero, no un adjunto suelto de otra
/// conversacion. Devuelve la ficha del proyecto nuevo y cuantas paginas se
/// extrajeron de cuantas.
///
/// El PDF original se guarda tambien, como primer mensaje: las paginas son
/// imagenes y no se puede volver de ellas al documento.
fn proyecto_de_pdf(
    ubicacion: &Ubicacion,
    aparato: &str,
    pdf: &std::path::Path,
) -> anyhow::Result<(pixpin_proyecto::almacen::Ficha, u32, u32)> {
    use pixpin_proyecto::{almacen, cuaderno};
    let raiz = ubicacion.raiz();
    let nombre_fichero = pdf
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "documento.pdf".into());
    // El proyecto se llama como el PDF sin su extension: es el nombre que el
    // usuario reconoce en la lista.
    let titulo = pdf
        .file_stem()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| nombre_fichero.clone());

    let cuando = pixpin_shell::entorno::ahora_utc_ms();
    let identidad = pixpin_proyecto::identidad::Identidad::leer_o_crear(raiz, "PC")
        .map(|i| i.yo.codigo())
        .unwrap_or_else(|_| aparato.to_string());
    let ficha = almacen::Ficha::nueva(&titulo, cuando, &identidad);
    let mut indice = almacen::Indice::leer(raiz);
    indice.proyectos.push(ficha.clone());
    indice.guardar(raiz)?;

    let carpeta = almacen::carpeta(raiz, &ficha.id);
    let mut numero = 1;
    // El documento entero primero: sin el, de las paginas no se vuelve.
    let bytes = std::fs::read(pdf)?;
    let relativa = almacen::guardar_adjunto(raiz, &ficha.id, &nombre_fichero, &bytes)?;
    let mensaje = cuaderno::Mensaje::adjunto(
        cuaderno::Clase::Archivo,
        &nombre_fichero,
        &relativa,
        bytes.len() as i64,
        &cuaderno::Sello {
            cuando,
            numero,
            aparato: identidad.clone(),
            proyecto: ficha.id.clone(),
        },
    );
    cuaderno::anadir(&carpeta, &mensaje)?;

    let documento = pixpin_pdf::Documento::abrir(pdf)?;
    let total = documento.paginas();
    let cuantas = total.min(TOPE_PAGINAS_PDF);
    let mut hechas = 0;
    for pagina in 0..cuantas {
        // Una pagina que falle no puede llevarse las demas: se anota y se
        // sigue, como en los pines.
        let hecho = (|| -> anyhow::Result<()> {
            let imagen = documento.renderizar(pagina, ANCHO_PAGINA)?;
            let png = pixpin_codec::codificar_png(&imagen)?;
            let nombre = format!("{titulo} - pagina {:02}.png", pagina + 1);
            let relativa = almacen::guardar_adjunto(raiz, &ficha.id, &nombre, &png)?;
            numero += 1;
            let mut m = cuaderno::Mensaje::adjunto(
                cuaderno::Clase::Pagina,
                &nombre,
                &relativa,
                png.len() as i64,
                &cuaderno::Sello {
                    cuando: cuando + numero,
                    numero,
                    aparato: identidad.clone(),
                    proyecto: ficha.id.clone(),
                },
            );
            // Que pagina es, como en Android: de la imagen sola no se sabe.
            m.pagina = Some(pagina);
            cuaderno::anadir(&carpeta, &m)?;
            Ok(())
        })();
        match hecho {
            Ok(()) => hechas += 1,
            Err(e) => tracing::warn!(?e, pagina, "no se pudo extraer la pagina"),
        }
    }
    tracing::info!(
        proyecto = %ficha.id,
        %titulo,
        hechas,
        total,
        "PDF abierto como proyecto"
    );
    Ok((ficha, hechas, total))
}

/// Si una ruta es un PDF. Por la extension y sin distinguir mayusculas, que
/// es como lo escribe cada programa a su manera.
fn es_pdf(ruta: &std::path::Path) -> bool {
    ruta.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

#[cfg(test)]
mod pruebas_pdf {
    use super::es_pdf;
    use std::path::Path;

    #[test]
    fn se_reconoce_un_pdf_se_escriba_como_se_escriba() {
        assert!(es_pdf(Path::new("plano.pdf")));
        assert!(es_pdf(Path::new("PLANO.PDF")));
        assert!(es_pdf(Path::new(r"C:\obra\Plano General.Pdf")));
        // Caso negativo: lo que solo lo lleva en el nombre no lo es.
        assert!(!es_pdf(Path::new("plano.pdf.png")));
        assert!(!es_pdf(Path::new("pdf")));
    }
}

/// Abre un dibujo del proyecto en el lienzo grande, para verlo entero.
///
/// Guarda lo que se haga al cerrar, y tambien al saltar a otra hoja por un
/// enlace. Devuelve las hojas que se guardaron, para que quien llama rehaga
/// sus burbujas: sin eso el chat seguiria ensenando el dibujo de antes.
fn abrir_dibujo(
    ubicacion: &Ubicacion,
    a: &Abierto,
    indice: usize,
    opciones: OpcionesLienzo,
) -> Vec<usize> {
    abrir_hojas(ubicacion.raiz(), &a.ficha.id, &a.mensajes, indice, opciones)
}

/// Lo de `abrir_dibujo` sin el proyecto abierto en el chat: el universo abre
/// asi las hojas desde su Ctrl+clic, con los mensajes recien leidos del
/// cuaderno. `indice` es el mensaje de la hoja dentro de `mensajes`.
pub(crate) fn abrir_hojas(
    raiz: &std::path::Path,
    proyecto: &str,
    mensajes: &[pixpin_proyecto::cuaderno::Mensaje],
    indice: usize,
    opciones: OpcionesLienzo,
) -> Vec<usize> {
    let mut guardadas = Vec::new();
    // Pulsar una «zona» de una pagina lleva a su hoja, y desde ella se puede
    // saltar a otra: por eso es un bucle y no una llamada. El tope es por si
    // dos hojas se enlazan entre si; sin el, el ir y venir no acabaria.
    let mut indice = indice;
    for _ in 0..SALTOS_MAXIMOS {
        let (guardada, siguiente) = abrir_una_hoja(raiz, proyecto, mensajes, indice, opciones);
        if guardada {
            guardadas.push(indice);
        }
        match siguiente {
            Some(s) => indice = s,
            None => break,
        }
    }
    guardadas
}

/// Cuantas hojas encadenadas se abren antes de parar.
const SALTOS_MAXIMOS: usize = 32;

/// Abre una hoja. Devuelve si se guardo algo y a cual hay que saltar, si se
/// pulso un enlace.
fn abrir_una_hoja(
    raiz: &std::path::Path,
    proyecto: &str,
    mensajes: &[pixpin_proyecto::cuaderno::Mensaje],
    indice: usize,
    opciones: OpcionesLienzo,
) -> (bool, Option<usize>) {
    let Some(m) = mensajes.get(indice) else {
        return (false, None);
    };
    let Some(id) = m.referencia.as_deref().filter(|r| !r.is_empty()) else {
        return (false, None);
    };
    let ruta = pixpin_proyecto::almacen::lienzo(raiz, proyecto, id);
    let texto = match std::fs::read_to_string(&ruta) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(?e, ruta = %ruta.display(), "no se pudo leer el dibujo");
            return (false, None);
        }
    };
    let lienzo = match pixpin_motor2d::excalidraw::leer(&texto) {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(?e, "dibujo que no se entiende");
            return (false, None);
        }
    };
    // `a_escena` y no un bucle a mano: es la que fija el orden con el que
    // `con_escena` sabe luego que elemento es cual.
    let escena = pixpin_motor2d::excalidraw::a_escena(&lienzo);
    // Las fotos de la hoja viven en `imagenes/<id>` del proyecto, no dentro
    // del JSON: el movil deja ahi una ruta en vez de un `dataURL` para que un
    // plano de quince megas no se convierta en veinte de base64.
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, proyecto);
    let fotos: Vec<(u64, std::path::PathBuf)> = pixpin_motor2d::excalidraw::ficheros(&lienzo)
        .into_iter()
        .filter_map(|(id, rel)| {
            Some((id, pixpin_proyecto::vista::ruta_real(raiz, proyecto, &rel)?))
        })
        .collect();
    // Y si la hoja se dibujo sobre una pagina del PDF, esa pagina ES el
    // fondo: sin ella se ven los trazos flotando sobre el blanco.
    let fondo = m.pagina.and_then(|pagina| {
        let pdf = carpeta.join("documento.pdf");
        let hecho =
            pixpin_pdf::Documento::abrir(&pdf).and_then(|d| d.renderizar(pagina, ANCHO_PAGINA));
        match hecho {
            Ok(img) => Some(img),
            Err(e) => {
                tracing::warn!(?e, pagina, ruta = %pdf.display(), "no se pudo dibujar la pagina");
                None
            }
        }
    });
    tracing::info!(
        elementos = escena.cuantos_visibles(),
        ajenos = lienzo.cuantos_ajenos(),
        fotos = fotos.len(),
        pagina = ?m.pagina,
        ruta = %ruta.display(),
        "dibujo abierto"
    );
    let resultado = crate::ventana_editor::abrir(
        escena,
        opciones.enganche,
        opciones.nivel,
        opciones.medir_fotogramas,
        fondo,
        &fotos,
    );
    let (escena, destino) = match resultado {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(?e, "no se pudo abrir el lienzo");
            return (false, None);
        }
    };
    let guardada = guardar_hoja_dibujada(&ruta, &lienzo, &escena);
    // Se pulso un recuadro con enlace: la hoja a la que lleva es la que tiene
    // ese dibujo como referencia.
    let siguiente = destino.and_then(|destino| {
        let s = mensajes
            .iter()
            .position(|m| m.referencia.as_deref() == Some(destino.as_str()));
        if s.is_none() {
            tracing::warn!(%destino, "el enlace no lleva a ninguna hoja de este proyecto");
        }
        s
    });
    (guardada, siguiente)
}

/// Escribe de vuelta el `.excalidraw` con lo que se hizo en el editor.
/// `true` si habia algo que guardar y se guardo.
///
/// Lo que el movil mete y aqui no se entiende no se pierde: `con_escena`
/// deja lo ajeno donde estaba y `escribir` devuelve tal cual lo que no se
/// toco. Y si no se toco nada, no se escribe: abrir una hoja para mirarla no
/// puede cambiarle la fecha al fichero, que es lo que mira la sincronizacion.
fn guardar_hoja_dibujada(
    ruta: &std::path::Path,
    lienzo: &pixpin_motor2d::excalidraw::Lienzo,
    escena: &pixpin_motor2d::Escena,
) -> bool {
    use pixpin_motor2d::excalidraw::{con_escena, escribir};
    let antes = escribir(lienzo);
    let despues = escribir(&con_escena(lienzo, escena));
    if antes == despues {
        return false;
    }
    // A un fichero de al lado y luego se cambia el nombre: si la luz se va a
    // media escritura, queda el dibujo viejo entero y no medio nuevo.
    let temporal = ruta.with_extension("excalidraw.tmp");
    let hecho = std::fs::write(&temporal, despues).and_then(|()| std::fs::rename(&temporal, ruta));
    match hecho {
        Ok(()) => {
            tracing::info!(ruta = %ruta.display(), "dibujo guardado");
            true
        }
        Err(e) => {
            tracing::error!(?e, ruta = %ruta.display(), "no se pudo guardar el dibujo");
            false
        }
    }
}

/// La pagina de un PDF del proyecto, dibujada a fichero la primera vez.
///
/// Se guarda en `archivos/pagina-NN.png` y se reutiliza: dibujar una pagina
/// de un PDF de quince megas cuesta cientos de milisegundos, y la burbuja se
/// repinta muchas veces. Devuelve la ruta y lo que mide.
fn pagina_del_pdf(
    carpeta: &std::path::Path,
    pagina: u32,
) -> Option<(std::path::PathBuf, f32, f32)> {
    let destino = carpeta
        .join("archivos")
        .join(format!("pagina-{:02}.png", pagina + 1));
    if destino.is_file() {
        let (w, h) = pixpin_codec::imagen::medidas(&destino).ok()?;
        return Some((destino, w as f32, h as f32));
    }
    let pdf = carpeta.join("documento.pdf");
    if !pdf.is_file() {
        return None;
    }
    let imagen = pixpin_pdf::Documento::abrir(&pdf)
        .and_then(|d| d.renderizar(pagina, ANCHO_PAGINA))
        .inspect_err(|e| tracing::warn!(?e, pagina, "no se pudo dibujar la pagina del PDF"))
        .ok()?;
    let png = pixpin_codec::codificar_png(&imagen).ok()?;
    if let Some(padre) = destino.parent() {
        std::fs::create_dir_all(padre).ok()?;
    }
    std::fs::write(&destino, png).ok()?;
    Some((destino, imagen.ancho as f32, imagen.alto as f32))
}

// --- Las piezas de una burbuja, como en el movil -----------------------

/// El radio de la burbuja (el de Telegram, `RADIO_BURBUJA` del movil) y el
/// grueso del contorno del color de su lienzo.
const RADIO_BURBUJA: f32 = 17.0;
const GROSOR_BORDE: f32 = 2.0;
/// La letra de la hora: 11, como `TAMANO_DE_LA_HORA`.
const HORA_TAM: f32 = 11.0;
/// Lo alto de la banda de la cita: dos renglones de 13 y su aire.
const CITA_ALTO: f32 = 44.0;
/// La tarjeta «Viene de»: su alto, su aire alrededor y sus letras.
const VIENE_ALTO: f32 = 42.0;
const VIENE_AIRE_X: f32 = 8.0;
const VIENE_AIRE_Y: f32 = 6.0;
const VIENE_TEXTO_TAM: f32 = 13.0;
const VIENE_ETIQUETA_TAM: f32 = 10.0;
/// El nombre y el detalle de la fila de un archivo.
const FICHA_NOMBRE_TAM: f32 = 15.0;
const FICHA_DETALLE_TAM: f32 = 13.0;

/// Lo que se puede pulsar dentro de la conversacion, tal como quedo en el
/// ultimo fotograma. Lo apunta el pintado, que es quien sabe donde cae cada
/// cosa: si el raton lo recalculara por su cuenta acabarian sin coincidir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zona {
    Volver,
    Titulo,
    Buscar,
    Menu,
    /// Ver este proyecto en el universo (D211).
    Universo,
    /// Sacar a la pantalla: la pastilla de la fila o la esquina de la foto.
    Abrir(usize),
    /// La tarjeta «Viene de»: lleva al lienzo de origen.
    VieneDe(usize),
    /// Ir a un mensaje: la cita de una respuesta o la barra del fijado.
    Ir(usize),
    /// «Ver en contexto»: quita el filtro y lleva al primer resultado, que es
    /// como se sale de una busqueda sin perder el sitio.
    EnContexto,
    /// Un chip de etiqueta de la fila de buscar, por su sitio en `chips_de`:
    /// filtra por ese emoji, y volver a pulsarlo quita el filtro.
    Chip(usize),
    CerrarRespuesta,
    SelCerrar,
    SelCopiar,
    SelFijar,
    SelReenviar,
    SelBorrar,
}

/// La fila de un archivo ya medida.
struct FilaDeArchivo {
    icono: &'static Icono,
    nombre: String,
    detalle: String,
    ancho_texto: f32,
    /// La onda de una nota de voz, si el mensaje trae los picos del
    /// microfono. Ocupa el sitio del nombre: en una nota de voz el nombre es
    /// «voz_1758..m4a», que no le dice nada a nadie, y la onda si.
    onda: Vec<f32>,
}

/// Los picos del microfono que el movil anoto al grabar la nota.
///
/// Viven en `resto` y no en un campo propio porque el `Mensaje` de aqui no
/// los declara: `resto` guarda tal cual lo que el movil anade, y eso es
/// justo lo que hace falta para no perderlos al reescribir el cuaderno.
fn picos_de_voz(m: &pixpin_proyecto::cuaderno::Mensaje) -> Vec<i64> {
    m.resto
        .get("picos")
        .and_then(|v| v.as_array())
        .map(|v| v.iter().filter_map(|n| n.as_i64()).collect())
        .unwrap_or_default()
}

/// La tarjeta «Viene de» ya medida.
struct Viene {
    texto: String,
    ancho: f32,
}

/// De que se compone la burbuja de un mensaje, medido una vez.
#[derive(Default)]
struct Piezas {
    /// Una foto sin texto ni cita: se come la burbuja.
    foto_sola: bool,
    /// El mensaje al que contesta, por su posicion.
    cita: Option<usize>,
    vista: Option<(f32, f32)>,
    fila: Option<FilaDeArchivo>,
    viene: Option<Viene>,
    texto: String,
    borde: Option<Color>,
}

/// La fila de un mensaje: su icono, su nombre y el detalle, o `None` si no
/// lleva (una nota, o una foto que ya se ve).
///
/// Los iconos son los del movil (`FilaDeArchivo`): documento para un
/// archivo, el lapiz para un lienzo, la tabla, el libro para una pagina y
/// la carpeta para un proyecto.
fn fila_de(
    m: &pixpin_proyecto::cuaderno::Mensaje,
    foto_a_la_vista: bool,
    textos: &Catalogo,
) -> Option<(&'static Icono, String, String)> {
    use pixpin_proyecto::cuaderno::Clase;
    let clase = m.clase.as_ref()?;
    let icono: &'static Icono = match clase {
        Clase::Imagen if foto_a_la_vista => return None,
        Clase::Imagen | Clase::Archivo => &mi::DESCRIPTION,
        Clase::Dibujo => &mi::DRAW,
        Clase::Pagina => &mi::MENU_BOOK,
        Clase::Proyecto => &mi::FOLDER,
        Clase::Voz => &mi::PLAY_ARROW,
        // Cualquier mini-app que conozcamos, no solo la tabla: una lista de
        // tareas llegada del movil salia hasta ahora como texto suelto.
        Clase::MiniApp => icono_de_miniapp(m.miniapp.as_deref().unwrap_or_default()),
        _ => return None,
    };
    let nombre = nombre_de_la_fila(m, textos);
    let detalle = match clase {
        // La mini-app ensena su RESUMEN —«3 de 7», «60,50 €», la hora de la
        // alarma—, que es lo que el movil pone en la burbuja
        // (`MiniApps.kt:79-89`). Sin el, la burbuja solo repetia su nombre.
        Clase::MiniApp => pixpin_proyecto::mini::resumen(
            m.miniapp.as_deref().unwrap_or_default(),
            &m.texto,
            &moneda_del_catalogo(textos),
        )
        .map(|r| r.texto)
        .unwrap_or_default(),
        // Un lienzo o un proyecto dicen solo su nombre, como en el movil: su
        // peso no es lo que se pregunta de ellos.
        Clase::Dibujo | Clase::Proyecto => String::new(),
        Clase::Voz => {
            let s = (m.duracion_ms.max(0) / 1000) as u64;
            if s == 0 {
                String::new()
            } else {
                format!("{}:{:02}", s / 60, s % 60)
            }
        }
        _ => {
            // Tamano y extension en mayusculas: «949 kB PDF».
            let mut partes = Vec::new();
            if m.bytes > 0 {
                partes.push(pixpin_ui::chat::tamano_corto(m.bytes as u64));
            }
            let extension = extension_de(&nombre);
            if !extension.is_empty() {
                partes.push(extension.to_uppercase());
            }
            if let Some(p) = m.pagina {
                partes.push(format!("pág. {}", p + 1));
            }
            partes.join(" ")
        }
    };
    Some((icono, nombre, detalle))
}

/// El nombre que se ensena en la fila. Un lienzo creado aqui se guarda como
/// `<id>.excalidraw`, que no le dice nada a nadie: se llama «Lienzo», que es
/// como lo llama el movil.
fn nombre_de_la_fila(m: &pixpin_proyecto::cuaderno::Mensaje, textos: &Catalogo) -> String {
    use pixpin_proyecto::cuaderno::Clase;
    let nombre = m.nombre.trim();
    match m.clase.as_ref() {
        Some(Clase::Dibujo) => {
            let de_serie = nombre
                .strip_suffix(".excalidraw")
                .is_some_and(|base| Some(base) == m.referencia.as_deref());
            if nombre.is_empty() || de_serie {
                textos.t("chat-lienzo")
            } else {
                nombre.to_string()
            }
        }
        // Sin nombre se llama como su mini-app, no «Tabla»: una lista de
        // tareas del movil se rotulaba «Tabla» solo por ser de clase MINIAPP.
        Some(Clase::MiniApp) if nombre.is_empty() => {
            match clave_del_nombre(m.miniapp.as_deref().unwrap_or_default()) {
                Some(clave) => textos.t(clave),
                // Una mini-app de una version futura: su propia palabra, que
                // es mas honrado que llamarla como la que no es.
                None => m.miniapp.clone().unwrap_or_else(|| textos.t("chat-tabla")),
            }
        }
        Some(Clase::Voz) if nombre.is_empty() => textos.t("chat-clase-voz"),
        _ if nombre.is_empty() => {
            let resumen = m.resumen();
            if resumen.trim().is_empty() {
                textos.t("chat-clase-archivo")
            } else {
                resumen
            }
        }
        _ => nombre.to_string(),
    }
}

/// El texto que va en la burbuja, debajo de lo demas.
///
/// Una nota es su texto. Lo que ya se ve o lleva fila solo lleva su pie, si
/// se escribio uno; una mini-aplicacion nunca, porque su texto es el
/// documento entero. Lo que no se reconoce se ensena con su etiqueta: es
/// mas honrado que callarlo o fingir que es una nota.
fn texto_de(m: &pixpin_proyecto::cuaderno::Mensaje, con_pieza: bool, textos: &Catalogo) -> String {
    use pixpin_proyecto::cuaderno::Clase;
    match m.clase.as_ref() {
        Some(Clase::Nota) | None => m.texto.trim().to_string(),
        Some(Clase::MiniApp) if con_pieza => String::new(),
        _ if con_pieza => m.texto.trim().to_string(),
        _ => {
            let texto = m.resumen();
            match clase_de(m, textos) {
                Some(etiqueta) if texto.is_empty() => etiqueta,
                Some(etiqueta) => format!("{etiqueta}\n{texto}"),
                None => texto,
            }
        }
    }
}

/// De donde viene un mensaje, si lo dice (`vieneDe` del movil): el texto que
/// se ensena y el lienzo al que lleva. Lo escribe el movil al sacar una zona
/// de un lienzo; aqui viaja en lo que no se entiende del mensaje.
fn viene_de(m: &pixpin_proyecto::cuaderno::Mensaje) -> Option<(String, Option<String>)> {
    let v = m.resto.get("vieneDe")?.as_object()?;
    let texto = v.get("texto")?.as_str()?.trim().to_string();
    if texto.is_empty() {
        return None;
    }
    let dibujo = v
        .get("dibujo")
        .and_then(|d| d.as_str())
        .filter(|d| !d.is_empty())
        .map(str::to_string);
    Some((texto, dibujo))
}

/// La extension en minusculas, o vacio si el nombre no lleva ninguna.
/// Se corta a cuatro signos: lo que pasa de ahi no es una extension, es un
/// nombre con puntos.
pub(crate) fn extension_de(nombre: &str) -> String {
    nombre
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .filter(|e| !e.is_empty() && e.len() <= 4 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_default()
}

/// El tamano de la letra de la chapa del codigo: 10, como en el movil.
const CHAPA_TAM: f32 = 10.0;

/// El codigo de chat de un mensaje, como lo ensena el movil: `#47·K7Q2`.
/// `None` si el mensaje todavia no tiene numero.
fn chapa_de_codigo(m: &pixpin_proyecto::cuaderno::Mensaje) -> Option<String> {
    if m.numero <= 0 {
        return None;
    }
    // El codigo del aparato son CUATRO signos (`K7Q2`). Unos mensajes se
    // guardaron con el identificador entero del equipo en su sitio —un
    // UUID de treinta y seis—, y la chapa salia mas larga que el mensaje.
    // De ese identificador sale el codigo corto por el mismo camino que en
    // el movil, asi que se ensena el corto sin tocar lo guardado.
    let sufijo = match (m.aparato.as_deref(), m.letra.as_deref()) {
        (Some(a), _) if a.chars().count() > 6 => {
            format!("·{}", pixpin_proyecto::codigos::de_aparato(a))
        }
        (Some(a), _) => format!("·{a}"),
        (None, Some(l)) => l.to_string(),
        (None, None) => String::new(),
    };
    Some(format!("#{}{sufijo}", m.numero))
}

#[cfg(test)]
mod pruebas_ficha {
    use super::extension_de;

    #[test]
    fn la_extension_sale_en_minusculas_y_solo_si_lo_es() {
        assert_eq!(extension_de("Plano General.PDF"), "pdf");
        assert_eq!(extension_de("kyyj8l.apk"), "apk");
        // Casos negativos: un nombre con puntos no es una extension, y uno
        // sin punto tampoco. La chapa saldria con media frase dentro.
        assert_eq!(extension_de("informe.v3.definitivo"), "");
        assert_eq!(extension_de("sin-extension"), "");
        assert_eq!(extension_de("raro.p d"), "");
    }
}

#[cfg(test)]
mod pruebas_chapa {
    use super::chapa_de_codigo;
    use pixpin_proyecto::cuaderno::Mensaje;

    fn mensaje(numero: i64, aparato: Option<&str>) -> Mensaje {
        Mensaje {
            numero,
            aparato: aparato.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn la_chapa_es_corta_aunque_lo_guardado_sea_largo() {
        // El fallo que vio el usuario: mensajes guardados con el UUID del
        // equipo en vez de su codigo daban una chapa de cuarenta signos,
        // mas ancha que el propio mensaje.
        let larga = chapa_de_codigo(&mensaje(2, Some("78134539-e22b-42f6-8dab-6d35318e7c35")))
            .expect("con numero hay chapa");
        assert!(larga.starts_with("#2·"), "{larga}");
        assert_eq!(larga.chars().count(), "#2·".chars().count() + 4, "{larga}");
        // La que ya es corta se queda como esta.
        assert_eq!(
            chapa_de_codigo(&mensaje(5, Some("6ARJ"))).as_deref(),
            Some("#5·6ARJ")
        );
    }

    #[test]
    fn sin_numero_no_hay_chapa() {
        assert_eq!(chapa_de_codigo(&mensaje(0, Some("6ARJ"))), None);
    }
}

/// Lo que mide la vista de un mensaje dentro de su burbuja.
///
/// Una foto toma SU proporcion: encajarla en una caja fija dejaba bandas a
/// los lados —el «marco blanco» que el usuario pidio quitar—. Cabe en el
/// ancho de siempre y, como mucho, en vez y media del alto, que es lo que
/// deja ver entera una captura vertical del movil sin comerse el historial.
/// Lo demas (un dibujo, una tabla) conserva su caja fija de papel.
fn tamano_de_vista(vista: &Ojeada, ancho_max: f32, e: f32, factor: f32) -> (f32, f32) {
    use pixpin_ui::historial as h;
    let ancho = (h::VISTA_ANCHO as f32 * e * factor).min(ancho_max);
    let alto = h::VISTA_ALTO as f32 * e * factor;
    let Ojeada::Foto { doc, .. } = vista else {
        return (ancho, alto);
    };
    let (dw, dh) = (doc.0.max(1.0), doc.1.max(1.0));
    let escala = (ancho / dw).min(alto * 1.5 / dh);
    ((dw * escala).max(1.0), (dh * escala).max(1.0))
}

#[cfg(test)]
mod pruebas_vista {
    use super::{Ojeada, tamano_de_vista};

    fn foto(w: f32, h: f32) -> Ojeada {
        Ojeada::Foto {
            doc: (w, h),
            dibujo: Vec::new(),
        }
    }

    #[test]
    fn la_vista_de_una_foto_tiene_su_misma_proporcion() {
        // Sin bandas: si la caja no tuviera la proporcion de la foto, por los
        // lados asomaria el fondo, que es el marco blanco que se quito.
        for (w, h) in [(1920.0, 1080.0), (1080.0, 2340.0), (500.0, 500.0)] {
            let (vw, vh) = tamano_de_vista(&foto(w, h), 400.0, 1.0, 1.0);
            assert!(
                ((vw / vh) - (w / h)).abs() < 0.01,
                "{w}x{h} salio {vw}x{vh}"
            );
            assert!(vw <= 260.5, "no pasa del ancho de la vista: {vw}");
        }
    }

    #[test]
    fn una_foto_sin_tamano_no_divide_por_cero() {
        let (vw, vh) = tamano_de_vista(&foto(0.0, 0.0), 400.0, 1.0, 1.0);
        assert!(vw.is_finite() && vh.is_finite() && vw >= 1.0 && vh >= 1.0);
    }
}

// --- La caja de escribir, la barra de responder y el aviso ------------

/// La caja de escribir: la isla flotante del movil, con el campo en
/// pastilla, el clip dentro y el microfono (o enviar) fuera.
fn pintar_redaccion(p: &Pintor, d: &Disposicion, c: &Pinta, a: &Abierto, alto_texto: u32) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let isla = d.isla(alto_texto, escala);
    if isla.ancho == 0 || isla.alto == 0 {
        return;
    }
    // Buscando, la barra de encima de la isla dice cuantos resultados hay y
    // ofrece verlos en su sitio (`guardados_resultados` y
    // «Ver en contexto»): sin el recuento, una busqueda que filtra veinte
    // burbujas parece toda la conversacion.
    //
    // Y manda sobre la de responder, que no desaparece: la respuesta que
    // estuviera a medias sigue colgada y vuelve a verse al cerrar la lupa.
    // Dos barras encima de la isla se comerian el historial.
    if let Some(cuantos) = resultados_de_la_busqueda(a) {
        let barra = d.encima_de_la_isla(alto_texto, escala);
        let caja = RectF {
            alto: (barra.alto as f32 - 6.0 * e).max(1.0),
            ..rf(barra)
        };
        p.rellenar_redondeado(caja, 16.0 * e, tema.campo);
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("n", cuantos);
        let rotulo = textos.t_args("chat-buscar-resultados", &args);
        let (_, alto_r) = p.medir_texto(&rotulo, 12.0 * e);
        let contexto = textos.t("chat-buscar-en-contexto");
        let (w_contexto, _) = p.medir_texto(&contexto, 12.0 * e);
        let boton = Rect {
            x: (caja.x + caja.ancho - w_contexto - 24.0 * e) as i32,
            y: caja.y as i32,
            ancho: (w_contexto + 24.0 * e) as u32,
            alto: caja.alto as u32,
        };
        p.texto_linea(
            &rotulo,
            caja.x + 14.0 * e,
            caja.y + (caja.alto - alto_r) / 2.0,
            12.0 * e,
            (boton.x as f32 - caja.x - 18.0 * e).max(0.0),
            tema.texto,
        );
        p.texto(
            &contexto,
            boton.x as f32 + 12.0 * e,
            caja.y + (caja.alto - alto_r) / 2.0,
            12.0 * e,
            tema.enviar,
        );
        a.zonas.borrow_mut().push((boton, Zona::EnContexto));
    }
    // A quien se contesta, encima del campo y con su aspa: sin esta barra
    // uno escribe sin saber si el mensaje sigue enganchado.
    else if let Some(r) = a
        .respondiendo
        .as_deref()
        .and_then(|id| a.mensajes.iter().find(|m| m.id == id))
    {
        let barra = d.encima_de_la_isla(alto_texto, escala);
        let caja = RectF {
            alto: (barra.alto as f32 - 6.0 * e).max(1.0),
            ..rf(barra)
        };
        p.rellenar_redondeado(caja, 16.0 * e, tema.campo);
        let icono = 18.0 * e;
        p.icono(
            &mi::REPLY,
            RectF {
                x: caja.x + 12.0 * e,
                y: caja.y + (caja.alto - icono) / 2.0,
                ancho: icono,
                alto: icono,
            },
            tema.enviar,
        );
        p.rellenar(
            RectF {
                x: caja.x + 40.0 * e,
                y: caja.y + 6.0 * e,
                ancho: 3.0 * e,
                alto: (caja.alto - 12.0 * e).max(1.0),
            },
            tema.enviar,
        );
        let cerrar = Rect {
            x: (caja.x + caja.ancho - caja.alto) as i32,
            y: caja.y as i32,
            ancho: caja.alto as u32,
            alto: caja.alto as u32,
        };
        icono_centrado(p, &mi::CLOSE, cerrar, 20.0 * e, tema.campo_apagado);
        a.zonas.borrow_mut().push((cerrar, Zona::CerrarRespuesta));
        let resumen = match chapa_de_codigo(r) {
            Some(codigo) => format!("{codigo}  {}", r.resumen()),
            None => r.resumen(),
        };
        let (_, alto_r) = p.medir_texto(&resumen, 12.0 * e);
        let x = caja.x + 51.0 * e;
        p.texto_linea(
            &resumen,
            x,
            caja.y + (caja.alto - alto_r) / 2.0,
            12.0 * e,
            (cerrar.x as f32 - x).max(0.0),
            tema.texto,
        );
    }

    // La isla: radio 22 y un filete que la despega del papel.
    let r_isla = rf(isla);
    let radio = (chat::ISLA_RADIO as f32 * e).min(r_isla.alto / 2.0);
    let filete = (1.0 * e).max(1.0);
    p.rellenar_redondeado(r_isla, radio, tema.isla_borde);
    p.rellenar_redondeado(
        encoger(r_isla, filete),
        (radio - filete).max(0.0),
        tema.isla,
    );
    let campo = rf(d.campo(alto_texto, escala));
    p.rellenar_redondeado(
        campo,
        (chat::ISLA_RADIO as f32 * e).min(campo.alto / 2.0),
        tema.campo,
    );

    // Un sitio, dos caras: el microfono, o enviar si hay algo escrito. Y el
    // clip se retira al escribir, como en el movil.
    let icono = 24.0 * e;
    let hay_texto = !a.borrador.trim().is_empty();
    if hay_texto {
        icono_centrado(
            p,
            &mi::SEND,
            d.boton_enviar(alto_texto, escala),
            icono,
            tema.enviar,
        );
    } else {
        icono_centrado(
            p,
            &mi::MIC,
            d.boton_enviar(alto_texto, escala),
            icono,
            tema.campo_apagado,
        );
        icono_centrado(
            p,
            &mi::ATTACH_FILE,
            d.boton_adjuntar(alto_texto, escala),
            icono,
            tema.campo_apagado,
        );
    }

    let zona = d.texto_redaccion(alto_texto, escala);
    let (x, y, ancho) = (zona.x as f32, zona.y as f32, zona.ancho as f32);
    let tam = chat::REDACCION_TAM * e;
    if a.borrador.is_empty() {
        p.texto(&textos.t("chat-escribe"), x, y, tam, tema.campo_apagado);
        // El cursor, quieto y sin parpadeo: parpadear obligaria a despertar
        // el hilo dos veces por segundo con la ventana en reposo.
        p.rellenar(
            RectF {
                x: x - 2.0 * e,
                y,
                ancho: (1.0 * e).max(1.0),
                alto: tam * 1.3,
            },
            tema.texto,
        );
        return;
    }
    p.empujar_recorte(rf(zona));
    p.parrafo(&a.borrador, x, y, tam, ancho, &[], tema.texto);
    // El cursor va al final de lo escrito.
    let (_, alto_todo) = p.medir_texto_ajustado(&a.borrador, tam, ancho);
    let ultima = a.borrador.rsplit('\n').next().unwrap_or("");
    let (ancho_ultima, _) = p.medir_texto(ultima, tam);
    p.rellenar(
        RectF {
            x: x + ancho_ultima.min(ancho),
            y: y + alto_todo - tam * 1.3,
            ancho: (1.0 * e).max(1.0),
            alto: tam * 1.3,
        },
        tema.texto,
    );
    p.soltar_recorte();
}

/// Lo que dura un aviso a la vista: lo de un `Toast` corto de Android.
const AVISO_MS: u64 = 2500;

/// El aviso de abajo, como el `Toast` del movil: una pastilla oscura encima
/// de la caja de escribir. Dice lo que paso, o lo que en Windows aun no hay.
fn pintar_aviso(p: &Pintor, d: &Disposicion, escala: u32, alto_caja: u32, texto: &str) {
    let e = escala as f32 / 100.0;
    let columna = if d.chat.ancho > 0 { d.chat } else { d.lista };
    let tam = 13.0 * e;
    let tope = (columna.ancho as f32 - 48.0 * e).max(1.0);
    let (w, alto) = p.medir_texto_ajustado(texto, tam, tope);
    let caja = RectF {
        x: columna.x as f32 + (columna.ancho as f32 - w - 32.0 * e) / 2.0,
        y: d.redaccion(alto_caja, escala).y as f32 - alto - 20.0 * e - 12.0 * e,
        ancho: w + 32.0 * e,
        alto: alto + 20.0 * e,
    };
    p.rellenar_redondeado(
        caja,
        caja.alto.min(40.0 * e) / 2.0,
        con_alfa(hex(0x2b2b2f), 0.95),
    );
    p.parrafo(
        texto,
        caja.x + 16.0 * e,
        caja.y + 10.0 * e,
        tam,
        w + 1.0,
        &[],
        Color::BLANCO,
    );
}

// --- Los menus, con los iconos del movil ------------------------------

/// Lo que hace una entrada de menu. Todo sobre el cuaderno de verdad; lo que
/// en Windows no existe todavia se dice con un aviso, no se finge.
#[derive(Debug, Clone, PartialEq)]
enum Accion {
    // El clip.
    AdjArchivo,
    AdjImagen,
    AdjLienzo,
    AdjTabla,
    AdjDelMovil,
    AdjProyectos,
    AdjProyecto(String),
    MiniApps,
    /// Una de las siete del movil, por su palabra de `Mensaje.miniapp`. La
    /// palabra es el dato guardado y no se renombra (`MiniApps.kt:94-99`).
    AdjMini(&'static str),
    // Un mensaje, por su posicion.
    Responder(usize),
    Copiar(usize),
    Fijar(usize),
    Compartir(usize),
    AbrirCon(usize),
    /// Abrirlo en el visor de PixPin, sin salir a otra aplicacion.
    AbrirAqui(usize),
    Renombrar(usize),
    Pinear(usize),
    Rescatar(usize),
    Etiquetas(usize),
    Etiquetar(usize, Option<String>),
    Hilo(usize),
    Ir(usize),
    Reenviar(Vec<usize>),
    ReenviarA(Vec<usize>, String),
    Elegir(usize),
    /// Meter el mensaje en las hojas del proyecto, y devolverle la que se le
    /// quito (`UnirAlProyecto` del movil).
    Unir(usize),
    Devolver(usize),
    Borrar(Vec<usize>),
    FotoEnLienzo(usize),
    FotoAqui(usize),
    // La cabecera.
    Fondos,
    Fondo(usize),
    Proyectos,
    /// Abrir el universo con esto a la vista (D211, D212).
    Universo(crate::universo::Pedido),
    /// Lo que en Windows aun no existe: la clave del aviso.
    Aviso(&'static str),
}

/// Una linea de menu: icono, nombre y lo que hace. Borrar va en rojo.
struct EntradaMenu {
    icono: Option<&'static Icono>,
    texto: String,
    peligro: bool,
    accion: Accion,
}

fn entrada(icono: Option<&'static Icono>, texto: String, accion: Accion) -> EntradaMenu {
    EntradaMenu {
        icono,
        texto,
        peligro: false,
        accion,
    }
}

/// Un menu desplegado. Mide sus rotulos al pintarse, y el raton usa esas
/// medidas: el ancho lo manda el mas largo y solo se sabe con la fuente.
struct MenuAbierto {
    ancla: Punto,
    entradas: Vec<EntradaMenu>,
    anchos: std::cell::RefCell<Vec<f32>>,
}

impl MenuAbierto {
    fn nuevo(ancla: Punto, entradas: Vec<EntradaMenu>) -> MenuAbierto {
        MenuAbierto {
            ancla,
            anchos: std::cell::RefCell::new(vec![0.0; entradas.len()]),
            entradas,
        }
    }

    /// Donde cae, calculado igual al pintar y al pulsar.
    fn colocado(&self, marco: Rect, escala: u32) -> menu::Menu {
        chat::colocar_menu(
            self.ancla,
            Rect {
                x: 0,
                y: 0,
                ancho: marco.ancho,
                alto: marco.alto,
            },
            &self.anchos.borrow(),
            (menu::TEXTO_TAM * escala as f32 / 100.0).ceil() as u32,
            escala,
        )
    }
}

fn pintar_menu_abierto(p: &Pintor, tema: &Tema, escala: u32, m: &MenuAbierto, marco: Rect) {
    let e = escala as f32 / 100.0;
    let tam = menu::TEXTO_TAM * e;
    *m.anchos.borrow_mut() = m
        .entradas
        .iter()
        .map(|n| p.medir_texto(&n.texto, tam).0)
        .collect();
    let colocado = m.colocado(marco, escala);
    let caja = rf(colocado.caja);
    let radio = menu::RADIO as f32 * e;
    p.rellenar_redondeado(caja, radio, tema.pildora_borde);
    p.rellenar_redondeado(encoger(caja, 1.0), (radio - 1.0).max(0.0), tema.cabecera);
    let rojo = hex(0xe5534b);
    for (n, entrada) in m.entradas.iter().enumerate() {
        let fila = colocado.fila(n, escala);
        let color = if entrada.peligro { rojo } else { tema.texto };
        if let Some(icono) = entrada.icono {
            p.icono(
                icono,
                rf(colocado.icono(fila, escala)),
                if entrada.peligro { rojo } else { tema.apagado },
            );
        }
        let (_, alto) = p.medir_texto(&entrada.texto, tam);
        let x = colocado.texto(fila, escala) as f32;
        p.texto_linea(
            &entrada.texto,
            x,
            fila.y as f32 + (fila.alto as f32 - alto) / 2.0,
            tam,
            (colocado.caja.derecha() as f32 - x - menu::RELLENO_DERECHA as f32 * e).max(0.0),
            color,
        );
    }
}

/// El clip, con lo de la hoja de adjuntar del movil y en su orden (archivo,
/// pagina, proyecto, conversacion, teleprompter, pronunciar, mini-app); y
/// detras lo que solo tiene Windows: foto, lienzo nuevo y traer del movil.
fn menu_del_clip(textos: &Catalogo) -> Vec<EntradaMenu> {
    vec![
        entrada(
            Some(&mi::DESCRIPTION),
            textos.t("chat-adj-archivo"),
            Accion::AdjArchivo,
        ),
        entrada(
            Some(&mi::MENU_BOOK),
            textos.t("chat-adj-pagina"),
            Accion::Aviso("chat-no-hay-pagina"),
        ),
        entrada(
            Some(&mi::FOLDER),
            textos.t("chat-adj-proyecto"),
            Accion::AdjProyectos,
        ),
        entrada(
            Some(&mi::RECORD_VOICE_OVER),
            textos.t("chat-adj-conversacion"),
            Accion::Aviso("chat-no-hay-conversacion"),
        ),
        entrada(
            Some(&mi::SUBTITLES),
            textos.t("chat-adj-telepronter"),
            Accion::Aviso("chat-no-hay-telepronter"),
        ),
        entrada(
            Some(&mi::HEARING),
            textos.t("chat-adj-pronunciar"),
            Accion::Aviso("chat-no-hay-pronunciar"),
        ),
        entrada(
            Some(&mi::CHECKLIST),
            textos.t("chat-adj-miniapp"),
            Accion::MiniApps,
        ),
        entrada(
            Some(&mi::IMAGE),
            textos.t("adjuntar-imagen"),
            Accion::AdjImagen,
        ),
        entrada(
            Some(&mi::DRAW),
            textos.t("adjuntar-lienzo"),
            Accion::AdjLienzo,
        ),
        entrada(
            Some(&mi::WIFI),
            textos.t("adjuntar-del-movil"),
            Accion::AdjDelMovil,
        ),
    ]
}

/// La palabra de `Mensaje.miniapp` de una mini-app, y la clave de su nombre.
///
/// Las siete del movil mas la hoja de calculo, que solo existe aqui. Ninguna
/// palabra se traduce: es el dato que viaja por la sincronizacion, y
/// renombrarla dejaria sin dueno a todo lo ya escrito (`MiniApps.kt:94-99`).
fn clave_del_nombre(cual: &str) -> Option<&'static str> {
    use pixpin_proyecto::mini;
    Some(match cual {
        mini::TAREAS => "mini-tareas",
        mini::GASTOS => "mini-gastos",
        mini::CRONOMETRO => "mini-cronometro",
        mini::TEMPORIZADOR => "mini-temporizador",
        mini::CONTADOR => "mini-contador",
        mini::RULETA => "mini-ruleta",
        mini::ALARMA => "mini-alarma",
        _ if cual == pixpin_proyecto::tabla::MINIAPP => "chat-tabla",
        _ => return None,
    })
}

/// El icono de una mini-app en la fila de su burbuja.
///
/// Los tres relojes —cronometro, temporizador y alarma— comparten el mismo a
/// proposito: son lo mismo visto de tres formas, y un icono inventado para
/// cada uno diria menos que el que ya se entiende.
fn icono_de_miniapp(cual: &str) -> &'static Icono {
    use pixpin_proyecto::mini;
    match cual {
        mini::GASTOS => &mi::LIST,
        mini::CRONOMETRO | mini::TEMPORIZADOR | mini::ALARMA => &mi::ALARM,
        _ if cual == pixpin_proyecto::tabla::MINIAPP => &mi::TABLE_CHART,
        // Tareas, contador y ruleta: la lista con casillas, que es el icono
        // con el que el movil rotula la hoja de mini-apps entera.
        _ => &mi::CHECKLIST,
    }
}

/// El menu del clip que ofrece las mini-apps: las siete del movil y, la
/// ultima, la hoja de calculo, que solo existe en este equipo.
fn menu_de_miniapps(textos: &Catalogo) -> Vec<EntradaMenu> {
    let mut v: Vec<EntradaMenu> = pixpin_proyecto::mini::TODAS
        .iter()
        .filter_map(|cual| {
            Some(entrada(
                Some(icono_de_miniapp(cual)),
                textos.t(clave_del_nombre(cual)?),
                Accion::AdjMini(cual),
            ))
        })
        .collect();
    v.push(entrada(
        Some(&mi::TABLE_CHART),
        textos.t("chat-tabla"),
        Accion::AdjTabla,
    ));
    v
}

/// Los tres puntos de la cabecera, en el orden del movil. La biblioteca y
/// las conversaciones solo salen en «Mensajes guardados», que es la general.
///
/// «Ver en el universo» (D211) es del escritorio y va el primero: tambien
/// tiene su boton en la pastilla, y aqui es donde se busca con el nombre
/// escrito.
fn menu_de_cabecera(textos: &Catalogo, en_guardados: bool, proyecto: &str) -> Vec<EntradaMenu> {
    let mut v = vec![entrada(
        Some(&mi::PUBLIC),
        textos.t("universo-ver-proyecto"),
        Accion::Universo(crate::universo::Pedido::Galaxia(proyecto.to_string())),
    )];
    if en_guardados {
        v.push(entrada(
            Some(&mi::LIBRARY_MUSIC),
            textos.t("chat-biblioteca-audio"),
            Accion::Aviso("chat-no-hay-biblioteca"),
        ));
        v.push(entrada(
            None,
            textos.t("chat-conversaciones"),
            Accion::Proyectos,
        ));
    }
    v.push(entrada(None, textos.t("chat-proyectos"), Accion::Proyectos));
    v.push(entrada(
        None,
        textos.t("chat-comenzar"),
        Accion::Aviso("chat-ya-en-marcha"),
    ));
    v.push(entrada(
        Some(&mi::PALETTE),
        textos.t("chat-fondo"),
        Accion::Fondos,
    ));
    v.push(entrada(
        None,
        textos.t("chat-ajustes"),
        Accion::Aviso("chat-ajustes-bandeja"),
    ));
    v
}

/// Los papeles del chat, con el elegido marcado.
fn menu_de_fondos(textos: &Catalogo, actual: usize) -> Vec<EntradaMenu> {
    FONDOS
        .iter()
        .enumerate()
        .map(|(n, f)| {
            let icono: &'static Icono = if n == actual {
                &mi::CHECK_BOX
            } else {
                &mi::CHECK_BOX_OUTLINE_BLANK
            };
            entrada(Some(icono), textos.t(f.4), Accion::Fondo(n))
        })
        .collect()
}

/// Las etiquetas del movil y, si ya tiene una, quitarla.
fn menu_de_etiquetas(textos: &Catalogo, i: usize, puesta: Option<&str>) -> Vec<EntradaMenu> {
    let mut v: Vec<EntradaMenu> = pixpin_ui::chat::ETIQUETAS
        .iter()
        .map(|e| {
            entrada(
                None,
                (*e).to_string(),
                Accion::Etiquetar(i, Some((*e).to_string())),
            )
        })
        .collect();
    if puesta.is_some() {
        v.push(entrada(
            Some(&mi::CLOSE),
            textos.t("chat-quitar-etiqueta"),
            Accion::Etiquetar(i, None),
        ));
    }
    v
}

/// Los demas proyectos, para mandarles algo: todos menos el abierto.
fn menu_de_proyectos(
    fichas: &[pixpin_proyecto::almacen::Ficha],
    excepto: &str,
    hacer: impl Fn(String) -> Accion,
) -> Vec<EntradaMenu> {
    fichas
        .iter()
        .filter(|f| f.id != excepto)
        .map(|f| {
            let icono: &'static Icono = if f.es_guardados() {
                &mi::BOOKMARK_BORDER
            } else {
                &mi::FOLDER
            };
            entrada(Some(icono), f.nombre.clone(), hacer(f.id.clone()))
        })
        .collect()
}

/// El menu de un mensaje: el de mantener pulsado en el movil, con Borrar el
/// ultimo y en rojo (`Burbuja`, `DelMenu`). Solo salen las que valen para
/// ESTE mensaje, igual que alli.
fn menu_de_mensaje(a: &Abierto, i: usize, textos: &Catalogo) -> Vec<EntradaMenu> {
    use pixpin_proyecto::cuaderno::Clase;
    let Some(m) = a.mensajes.get(i) else {
        return Vec::new();
    };
    let mut v = vec![entrada(
        Some(&mi::REPLY),
        textos.t("chat-comentar"),
        Accion::Responder(i),
    )];
    if !m.texto.trim().is_empty() && m.clase != Some(Clase::MiniApp) {
        v.push(entrada(
            Some(&mi::CONTENT_COPY),
            textos.t("chat-copiar"),
            Accion::Copiar(i),
        ));
    }
    v.push(entrada(
        None,
        textos.t(if m.fijado {
            "chat-soltar"
        } else {
            "chat-fijar"
        }),
        Accion::Fijar(i),
    ));
    v.push(entrada(
        Some(&mi::IOS_SHARE),
        textos.t("chat-compartir"),
        Accion::Compartir(i),
    ));
    let ruta = ruta_del_mensaje(&a.raiz, &a.ficha.id, m).filter(|r| r.is_file());
    if let Some(ruta) = &ruta {
        // «Abrir aqui» va DELANTE de «Abrir con otra app» y solo cuando el
        // visor sabe leer eso: ofrecerse para un `.zip` abriria una ventana
        // en blanco, que es peor que mandarlo a Windows.
        if ruta
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(crate::visor::se_abre)
        {
            v.push(entrada(
                Some(&mi::MENU_BOOK),
                textos.t("chat-abrir-aqui"),
                Accion::AbrirAqui(i),
            ));
        }
        v.push(entrada(
            Some(&mi::LAUNCH),
            textos.t("chat-abrir-con"),
            Accion::AbrirCon(i),
        ));
        if es_pdf(ruta) {
            v.push(entrada(
                Some(&mi::COMPRESS),
                textos.t("chat-aligerar"),
                Accion::Aviso("chat-no-hay-aligerar"),
            ));
        }
    }
    if m.clase == Some(Clase::Dibujo) && m.referencia.is_some() {
        v.push(entrada(
            Some(&mi::EDIT),
            textos.t("chat-renombrar"),
            Accion::Renombrar(i),
        ));
    }
    v.push(entrada(
        Some(&mi::OPEN_IN_NEW),
        textos.t("chat-pinear"),
        Accion::Pinear(i),
    ));
    // Lo que solo tiene Windows con una foto: el lienzo grande o dibujar en
    // la propia burbuja.
    if matches!(a.vistas.get(i), Some(Some(Ojeada::Foto { .. }))) {
        v.push(entrada(
            Some(&mi::DRAW),
            textos.t("menu-foto-lienzo"),
            Accion::FotoEnLienzo(i),
        ));
        v.push(entrada(
            Some(&mi::EDIT),
            textos.t("menu-foto-aqui"),
            Accion::FotoAqui(i),
        ));
    }
    // D212: solo lo que es una luna del universo (archivos, fotos, dibujos;
    // no las notas ni lo del buzon), que es lo que alli se puede resaltar.
    if crate::universo::fichas::es_luna(m) {
        v.push(entrada(
            Some(&mi::PUBLIC),
            textos.t("menu-foto-universo"),
            Accion::Universo(crate::universo::Pedido::Luna {
                proyecto: a.ficha.id.clone(),
                codigo: m.codigo_unico(),
            }),
        ));
    }
    v.push(entrada(
        Some(&mi::ALARM),
        textos.t("chat-recordar"),
        Accion::Aviso("chat-no-hay-recordatorios"),
    ));
    if m.en_buzon {
        v.push(entrada(
            Some(&mi::BOOKMARK_BORDER),
            textos.t("chat-rescatar"),
            Accion::Rescatar(i),
        ));
    }
    if m.clase == Some(Clase::Imagen) && m.referencia.is_some() {
        v.push(entrada(
            Some(&mi::CROP),
            textos.t("chat-solo-la-foto"),
            Accion::Aviso("chat-no-hay-recorte"),
        ));
    }
    if a.mensajes
        .iter()
        .any(|o| o.responde_a.as_deref() == Some(m.id.as_str()))
    {
        v.push(entrada(
            Some(&mi::FORUM),
            textos.t("chat-ver-hilo"),
            Accion::Hilo(i),
        ));
    }
    v.push(entrada(
        Some(&mi::EMOJI_EMOTIONS),
        textos.t("chat-etiquetar"),
        Accion::Etiquetas(i),
    ));
    v.push(entrada(
        Some(&mi::FORWARD),
        textos.t("chat-reenviar"),
        Accion::Reenviar(vec![i]),
    ));
    // Unir al proyecto, o devolverle la hoja que se le quito. Se mira en el
    // disco al ABRIR el menu, como en el movil: la hoja puede haberla quitado
    // la ultima sincronizacion, y fiarse de `unido` diria que sigue ahi.
    //
    // En «Mensajes guardados» no se ofrece: alli habria que elegir a que
    // proyecto va y copiarle el fichero, que es lo que ya hace «Reenviar».
    if !a.ficha.es_guardados() && se_puede_unir(m) {
        if !esta_en_las_hojas(a, i) {
            let (clave, accion) = if ya_esta_unido(m) {
                ("chat-devolver", Accion::Devolver(i))
            } else {
                ("chat-unir", Accion::Unir(i))
            };
            v.push(entrada(Some(&mi::LIBRARY_ADD), textos.t(clave), accion));
        }
    } else if a.ficha.es_guardados() {
        v.push(entrada(
            Some(&mi::LIBRARY_ADD),
            textos.t("chat-unir"),
            Accion::Aviso("chat-no-hay-unir"),
        ));
    }
    if m.clase == Some(Clase::Voz) {
        v.push(entrada(
            Some(&mi::SUBTITLES),
            textos.t("chat-transcribir"),
            Accion::Aviso("chat-no-hay-transcripcion"),
        ));
        v.push(entrada(
            Some(&mi::LYRICS),
            textos.t("chat-letra"),
            Accion::Aviso("chat-no-hay-letra"),
        ));
    }
    v.push(entrada(
        Some(&mi::CHECK_BOX),
        textos.t("chat-elegir"),
        Accion::Elegir(i),
    ));
    v.push(EntradaMenu {
        icono: Some(&mi::DELETE),
        texto: textos.t("chat-borrar"),
        peligro: true,
        accion: Accion::Borrar(vec![i]),
    });
    v
}

/// Lo que tiene que hacer el bucle despues de una accion.
enum Efecto {
    Nada,
    Aviso(String),
    /// Otro menu en el mismo sitio (una etiqueta, un proyecto...).
    Menu(Vec<EntradaMenu>),
    /// Cambio el cuaderno: hay que volver a medir las burbujas.
    Cambio,
}

/// Lo que una accion necesita del bucle.
struct Contexto<'a> {
    ubicacion: &'a Ubicacion,
    identidad: &'a str,
    textos: &'a Catalogo,
    lienzo: OpcionesLienzo,
    ventana: &'a VentanaOverlay,
    fichas: &'a [pixpin_proyecto::almacen::Ficha],
    /// Para abrir otras ventanas en sus hilos (el universo).
    idioma: pixpin_store::Idioma,
}

/// Hace una accion sobre el proyecto abierto. Lo del clip que abre dialogos
/// o crea proyectos lo atiende el bucle, que es quien lleva la lista.
fn ejecutar(accion: Accion, a: &mut Abierto, cx: &Contexto) -> Efecto {
    use pixpin_proyecto::{almacen, cuaderno};
    let raiz = cx.ubicacion.raiz();
    let carpeta = almacen::carpeta(raiz, &a.ficha.id);
    let fallo = |e: &dyn std::fmt::Debug| {
        tracing::warn!(?e, "accion del chat que no salio");
        Efecto::Aviso(cx.textos.t("chat-no-se-pudo"))
    };
    // Cambia un mensaje en el disco y, si sale, en la pantalla.
    let reescribir = |a: &mut Abierto, i: usize, cambio: &dyn Fn(&mut cuaderno::Mensaje)| {
        let Some(mut m) = a.mensajes.get(i).cloned() else {
            return Efecto::Nada;
        };
        cambio(&mut m);
        match cuaderno::reemplazar(&carpeta, &m) {
            Ok(_) => {
                a.mensajes[i] = m;
                a.fijado = a.mensajes.iter().rposition(|m| m.fijado);
                Efecto::Cambio
            }
            Err(e) => fallo(&e),
        }
    };
    let ruta_de = |a: &Abierto, i: usize| {
        a.mensajes
            .get(i)
            .and_then(|m| ruta_del_mensaje(&a.raiz, &a.ficha.id, m))
            .filter(|r| r.is_file())
    };
    match accion {
        Accion::Aviso(clave) => Efecto::Aviso(cx.textos.t(clave)),
        // En su propio hilo: el chat sigue abierto y usable detras (D215).
        Accion::Universo(pedido) => {
            crate::universo::lanzar(cx.idioma, cx.ubicacion.clone(), cx.lienzo, pedido);
            Efecto::Nada
        }
        Accion::Responder(i) => {
            a.respondiendo = a.mensajes.get(i).map(|m| m.id.clone());
            Efecto::Nada
        }
        Accion::Copiar(i) => match a
            .mensajes
            .get(i)
            .map(|m| pixpin_codec::portapapeles::copiar_texto(m.texto.trim()))
        {
            Some(Ok(())) => Efecto::Aviso(cx.textos.t("chat-copiado")),
            Some(Err(e)) => fallo(&e),
            None => Efecto::Nada,
        },
        Accion::Fijar(i) => reescribir(a, i, &|m| m.fijado = !m.fijado),
        Accion::Rescatar(i) => reescribir(a, i, &|m| m.en_buzon = false),
        Accion::Etiquetar(i, emoji) => reescribir(a, i, &|m| m.emoji = emoji.clone()),
        Accion::Etiquetas(i) => {
            let puesta = a.mensajes.get(i).and_then(|m| m.emoji.as_deref());
            Efecto::Menu(menu_de_etiquetas(cx.textos, i, puesta))
        }
        // En Windows compartir es dejarlo en el portapapeles: el fichero o
        // el texto, listo para pegar en cualquier sitio.
        Accion::Compartir(i) => {
            let hecho = match ruta_de(a, i) {
                Some(ruta) => pixpin_codec::portapapeles::copiar_ficheros(&[ruta]),
                None => match a.mensajes.get(i) {
                    Some(m) => pixpin_codec::portapapeles::copiar_texto(&m.resumen()),
                    None => return Efecto::Nada,
                },
            };
            match hecho {
                Ok(()) => Efecto::Aviso(cx.textos.t("chat-compartido")),
                Err(e) => fallo(&e),
            }
        }
        // En su propio hilo, como el universo: el chat sigue abierto detras
        // y se puede seguir escribiendo mientras se lee el documento.
        Accion::AbrirAqui(i) => match ruta_de(a, i) {
            Some(ruta) => {
                crate::visor::lanzar(cx.idioma, cx.ubicacion.clone(), &ruta);
                Efecto::Nada
            }
            None => Efecto::Aviso(cx.textos.t("chat-sin-archivo")),
        },
        Accion::AbrirCon(i) => match ruta_de(a, i) {
            Some(ruta) => match pixpin_shell::abrir::abrir(&ruta) {
                Ok(()) => Efecto::Nada,
                Err(e) => fallo(&e),
            },
            None => Efecto::Aviso(cx.textos.t("chat-sin-archivo")),
        },
        // Sacar a la pantalla: el fichero va a la ventana principal, que es
        // quien tiene los pines (imagen, video o ficha, segun lo que sea).
        Accion::Pinear(i) => match ruta_de(a, i) {
            Some(ruta) if pixpin_shell::mensajero::enviar_ficheros(std::slice::from_ref(&ruta)) => {
                Efecto::Aviso(cx.textos.t("chat-pineado"))
            }
            Some(_) => fallo(&"no contesta la ventana principal"),
            None => Efecto::Aviso(cx.textos.t("chat-sin-archivo")),
        },
        Accion::Renombrar(i) => {
            if let Some(m) = a.mensajes.get(i) {
                a.renombrando_mensaje = Some((i, nombre_de_la_fila(m, cx.textos)));
            }
            Efecto::Cambio
        }
        Accion::Hilo(i) => {
            let Some(id) = a.mensajes.get(i).map(|m| m.id.clone()) else {
                return Efecto::Nada;
            };
            Efecto::Menu(
                a.mensajes
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| o.responde_a.as_deref() == Some(id.as_str()))
                    .map(|(j, o)| entrada(Some(&mi::REPLY), o.resumen(), Accion::Ir(j)))
                    .collect(),
            )
        }
        Accion::Ir(j) => {
            a.ir_a = Some(j);
            // Con el destello, como al llegar desde el universo: entre cien
            // burbujas parecidas, saltar sin destello deja al usuario
            // buscando cual de ellas era la que pidio.
            a.resaltado = Some((j, std::time::Instant::now()));
            Efecto::Nada
        }
        Accion::Elegir(i) => {
            if let Some(m) = a.mensajes.get(i) {
                a.marcados.insert(m.id.clone());
            }
            Efecto::Nada
        }
        Accion::Unir(i) => unir_al_proyecto(cx.ubicacion, a, i, cx.textos),
        Accion::Devolver(i) => devolver_al_proyecto(cx.ubicacion, a, i, cx.textos),
        Accion::Reenviar(indices) => {
            let v = menu_de_proyectos(cx.fichas, &a.ficha.id, |id| {
                Accion::ReenviarA(indices.clone(), id)
            });
            if v.is_empty() {
                Efecto::Aviso(cx.textos.t("chat-sin-proyectos-otros"))
            } else {
                Efecto::Menu(v)
            }
        }
        Accion::ReenviarA(indices, destino) => {
            match reenviar(cx.ubicacion, a, &indices, &destino, cx.identidad) {
                Ok(_) => {
                    a.marcados.clear();
                    Efecto::Aviso(cx.textos.t("chat-reenviado"))
                }
                Err(e) => fallo(&e),
            }
        }
        Accion::Borrar(indices) => {
            if !pixpin_shell::confirmar_destructivo(
                cx.ventana.handle(),
                &cx.textos.t("chat-borrar"),
                &cx.textos.t("chat-borrar-aviso"),
            ) {
                return Efecto::Nada;
            }
            match borrar_mensajes(cx.ubicacion, a, &indices) {
                Ok(_) => Efecto::Cambio,
                Err(e) => fallo(&e),
            }
        }
        Accion::FotoEnLienzo(i) => {
            if let Some(ruta) = ruta_de(a, i) {
                abrir_foto_en_lienzo(&ruta, cx.lienzo);
                // Al volver, la burbuja tiene que ensenar lo que se dibujo.
                if let Some(m) = a.mensajes.get(i).cloned() {
                    a.vistas[i] = leer_vista(cx.ubicacion, &a.ficha.id, &m);
                }
            }
            Efecto::Cambio
        }
        Accion::FotoAqui(i) => {
            encender_lienzo(a, i);
            Efecto::Cambio
        }
        Accion::AdjProyectos => {
            let v = menu_de_proyectos(cx.fichas, &a.ficha.id, Accion::AdjProyecto);
            if v.is_empty() {
                Efecto::Aviso(cx.textos.t("chat-sin-proyectos-otros"))
            } else {
                Efecto::Menu(v)
            }
        }
        Accion::AdjProyecto(id) => {
            let Some(ficha) = cx.fichas.iter().find(|f| f.id == id) else {
                return Efecto::Nada;
            };
            let cuando = pixpin_shell::entorno::ahora_utc_ms();
            let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
            // Un proyecto adjunto no se copia: es la puerta a lo que ya
            // existe (`guardados_adj_proyecto`). Pulsarlo lo abre.
            let mut m = cuaderno::Mensaje::adjunto(
                cuaderno::Clase::Proyecto,
                &ficha.nombre,
                "",
                0,
                &cuaderno::Sello {
                    cuando,
                    numero,
                    aparato: cx.identidad.to_string(),
                    proyecto: a.ficha.id.clone(),
                },
            );
            m.ruta = None;
            m.referencia = Some(id);
            match cuaderno::anadir(&carpeta, &m) {
                Ok(()) => {
                    a.vistas.push(None);
                    a.mensajes.push(m);
                    a.scroll = None;
                    Efecto::Cambio
                }
                Err(e) => fallo(&e),
            }
        }
        Accion::MiniApps => Efecto::Menu(menu_de_miniapps(cx.textos)),
        Accion::AdjMini(cual) => {
            match crear_miniapp(cx.ubicacion, a, cx.identidad, cual, cx.textos) {
                Ok(()) => {
                    a.scroll = None;
                    Efecto::Cambio
                }
                Err(e) => fallo(&e),
            }
        }
        // Lo demas lo atiende el bucle.
        Accion::AdjArchivo
        | Accion::AdjImagen
        | Accion::AdjLienzo
        | Accion::AdjTabla
        | Accion::AdjDelMovil
        | Accion::Fondos
        | Accion::Fondo(_)
        | Accion::Proyectos => Efecto::Nada,
    }
}

/// Quita del cuaderno los mensajes cuyo id este en `ids`, dejando el resto
/// del fichero como estaba. Devuelve cuantos quito.
///
/// Las lineas que no se entienden se copian tal cual, como en
/// `cuaderno::reemplazar`: borrar no puede ser la forma de perder lo que
/// escribio una version mas nueva del movil. Se escribe a un temporal y se
/// renombra, para que un corte a mitad deje el cuaderno anterior entero.
fn quitar_del_cuaderno(
    carpeta: &std::path::Path,
    ids: &std::collections::BTreeSet<String>,
) -> std::io::Result<usize> {
    let fichero = carpeta.join("guardados.jsonl");
    let texto = std::fs::read_to_string(&fichero)?;
    let mut salida = String::with_capacity(texto.len());
    let mut quitados = 0;
    for linea in texto.lines() {
        let suya = serde_json::from_str::<pixpin_proyecto::cuaderno::Mensaje>(linea)
            .is_ok_and(|m| ids.contains(&m.id));
        if suya {
            quitados += 1;
            continue;
        }
        salida.push_str(linea);
        salida.push('\n');
    }
    if quitados == 0 {
        return Ok(0);
    }
    let temporal = fichero.with_extension("jsonl.tmp");
    std::fs::write(&temporal, salida)?;
    std::fs::rename(&temporal, &fichero)?;
    Ok(quitados)
}

/// Borra mensajes de la conversacion, y el fichero adjunto de los que lo
/// llevan, como el movil (`borrarAdjunto`). Un lienzo o una pagina NO se
/// borran: son hojas del proyecto y otras cosas pueden apuntar a ellas.
fn borrar_mensajes(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    indices: &[usize],
) -> std::io::Result<usize> {
    use pixpin_proyecto::cuaderno::Clase;
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    let ids: std::collections::BTreeSet<String> = indices
        .iter()
        .filter_map(|&i| a.mensajes.get(i).map(|m| m.id.clone()))
        .collect();
    // El lienzo vivo se guarda antes: su posicion cambia al quitar otros.
    if a.vivo.is_some() {
        apagar_lienzo(ubicacion, a);
    }
    let quitados = quitar_del_cuaderno(&carpeta, &ids)?;
    // La marca de cada uno, como la deja el movil al borrar: sin ella, la
    // siguiente vuelta de sincronizar los traeria otra vez del otro aparato.
    // Con la hora del reloj (UTC) y no la local: se compara con la del movil.
    let idos: Vec<_> = a
        .mensajes
        .iter()
        .filter(|m| ids.contains(&m.id))
        .cloned()
        .collect();
    let ahora_utc = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    if let Err(e) =
        pixpin_proyecto::vista::anotar_borrados(ubicacion.raiz(), &a.ficha.id, &idos, ahora_utc)
    {
        tracing::warn!(?e, "no se pudo apuntar lo borrado para sincronizar");
    }
    for m in a.mensajes.iter().filter(|m| ids.contains(&m.id)) {
        let con_fichero = matches!(m.clase, Some(Clase::Archivo | Clase::Imagen | Clase::Voz));
        if let Some(ruta) = ruta_del_mensaje(&a.raiz, &a.ficha.id, m).filter(|_| con_fichero)
            && ruta.is_file()
            && let Err(e) = std::fs::remove_file(&ruta)
        {
            tracing::warn!(?e, ruta = %ruta.display(), "adjunto que no se pudo borrar");
        }
    }
    // En paralelo: los mensajes y lo que se lee de cada uno.
    let mut n = 0;
    a.vistas.retain(|_| {
        let queda = !ids.contains(&a.mensajes[n].id);
        n += 1;
        queda
    });
    a.mensajes.retain(|m| !ids.contains(&m.id));
    a.fijado = a.mensajes.iter().rposition(|m| m.fijado);
    a.marcados.retain(|id| !ids.contains(id));
    if a.respondiendo.as_ref().is_some_and(|id| ids.contains(id)) {
        a.respondiendo = None;
    }
    a.renombrando_mensaje = None;
    Ok(quitados)
}

/// Manda copias de unos mensajes a otro proyecto (`reenviarA` del movil):
/// cada una es un mensaje nuevo alli, con su numero y su codigo, y su
/// fichero copiado, porque cada proyecto viaja con lo suyo.
fn reenviar(
    ubicacion: &Ubicacion,
    a: &Abierto,
    indices: &[usize],
    destino: &str,
    aparato: &str,
) -> std::io::Result<usize> {
    use pixpin_proyecto::{almacen, cuaderno};
    let raiz = ubicacion.raiz();
    let carpeta = almacen::carpeta(raiz, destino);
    let alli = cuaderno::Cuaderno::leer_de(&carpeta).unwrap_or_default();
    let mut numero = alli.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let mut ordenados: Vec<usize> = indices.to_vec();
    ordenados.sort_by_key(|&i| a.mensajes.get(i).map(|m| m.cuando));
    let mut hechos = 0;
    let mut ultimo = String::new();
    for (k, &i) in ordenados.iter().enumerate() {
        let Some(m) = a.mensajes.get(i) else {
            continue;
        };
        let cuando = ahora + k as i64;
        let mut copia = m.clone();
        copia.id = format!("{cuando}");
        copia.cuando = cuando;
        copia.numero = numero;
        copia.uid = Some(pixpin_proyecto::codigos::nuevo());
        copia.aparato = Some(aparato.to_string());
        copia.letra = None;
        copia.origen = m.uid.clone().or_else(|| Some(m.id.clone()));
        copia.proyecto = Some(destino.to_string());
        copia.fijado = false;
        copia.en_buzon = false;
        copia.responde_a = None;
        match ruta_del_mensaje(raiz, &a.ficha.id, m) {
            Some(ruta) if ruta.is_file() => {
                let bytes = std::fs::read(&ruta)?;
                let nombre = if m.nombre.trim().is_empty() {
                    ruta.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "archivo".into())
                } else {
                    m.nombre.clone()
                };
                copia.ruta = Some(almacen::guardar_adjunto(raiz, destino, &nombre, &bytes)?);
            }
            // Lo que no esta en este equipo no se puede llevar: la copia va
            // sin fichero, como en el movil.
            Some(_) => copia.ruta = None,
            None => {}
        }
        // Un lienzo lleva su dibujo: sin el, alli seria una hoja vacia.
        if m.clase == Some(cuaderno::Clase::Dibujo)
            && let Some(id) = m.referencia.as_deref().filter(|r| !r.is_empty())
        {
            let origen = almacen::lienzo(raiz, &a.ficha.id, id);
            let destino_lienzo = almacen::lienzo(raiz, destino, id);
            if origen.is_file() && !destino_lienzo.exists() {
                if let Some(padre) = destino_lienzo.parent() {
                    std::fs::create_dir_all(padre)?;
                }
                std::fs::copy(&origen, &destino_lienzo)?;
            }
        }
        cuaderno::anadir(&carpeta, &copia)?;
        ultimo = copia.resumen();
        numero += 1;
        hechos += 1;
    }
    if hechos > 0 {
        let mut indice = almacen::Indice::leer(raiz);
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == destino) {
            f.tocado = ahora;
            f.resumen = ultimo;
            indice.guardar(raiz)?;
        }
    }
    Ok(hechos)
}

/// El papel elegido, si se eligio alguno: se guarda al lado del almacen,
/// como el ajuste `fondoDelChat` del movil.
fn fondo_guardado(ubicacion: &Ubicacion) -> usize {
    std::fs::read_to_string(ubicacion.raiz().join("fondo-del-chat.txt"))
        .ok()
        .and_then(|t| {
            let clave = t.trim().to_string();
            FONDOS.iter().position(|f| f.4 == clave)
        })
        .unwrap_or(0)
}

fn guardar_fondo(ubicacion: &Ubicacion, cual: usize) {
    if let Some(f) = FONDOS.get(cual)
        && let Err(e) = std::fs::write(ubicacion.raiz().join("fondo-del-chat.txt"), f.4)
    {
        tracing::warn!(?e, "no se pudo guardar el fondo del chat");
    }
}

/// Los dos extremos del papel elegido, en el tema de ahora.
fn papel_de(cual: usize, claro: bool) -> (Color, Color) {
    let f = FONDOS.get(cual).unwrap_or(&FONDOS[0]);
    if claro {
        (hex(f.0), hex(f.1))
    } else {
        (hex(f.2), hex(f.3))
    }
}

#[cfg(test)]
mod pruebas_unir {
    use super::*;
    use pixpin_proyecto::cuaderno::{Clase, Mensaje};

    fn mensaje(clase: Clase, nombre: &str, texto: &str) -> Mensaje {
        Mensaje {
            id: "m1".into(),
            clase: Some(clase),
            nombre: nombre.into(),
            texto: texto.into(),
            ..Default::default()
        }
    }

    #[test]
    fn solo_se_unen_fotos_dibujos_notas_y_pdf() {
        assert!(se_puede_unir(&mensaje(Clase::Imagen, "obra.jpg", "")));
        assert!(se_puede_unir(&mensaje(Clase::Dibujo, "Lienzo", "")));
        assert!(se_puede_unir(&mensaje(Clase::Pagina, "Plano", "")));
        assert!(se_puede_unir(&mensaje(Clase::Nota, "", "hay que picar")));
        assert!(se_puede_unir(&mensaje(Clase::Archivo, "Contrato.PDF", "")));
        // Casos negativos, los que el movil deja fuera: una nota de voz, una
        // tabla, un acceso a otro proyecto y un archivo que no es un PDF. Y
        // una nota en blanco no es una hoja, es nada.
        assert!(!se_puede_unir(&mensaje(Clase::Voz, "voz_1.m4a", "")));
        assert!(!se_puede_unir(&mensaje(Clase::MiniApp, "Tabla", "")));
        assert!(!se_puede_unir(&mensaje(Clase::Proyecto, "Casa", "")));
        assert!(!se_puede_unir(&mensaje(Clase::Archivo, "notas.txt", "")));
        assert!(!se_puede_unir(&mensaje(Clase::Nota, "", "   ")));
    }

    #[test]
    fn la_hoja_apunta_al_mensaje_del_que_salio() {
        let textos = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let mut m = mensaje(Clase::Dibujo, "Planta baja", "");
        m.referencia = Some("dib-7".into());
        let h = hoja_de_mensaje(&m, &textos);
        assert_eq!(h.id, "m1");
        assert_eq!(h.dibujo.as_deref(), Some("dib-7"));
        assert_eq!(
            h.resto.get("deMensaje").and_then(|v| v.as_str()),
            Some("m1"),
            "sin el vinculo, el proyecto no sabe de donde vino la hoja"
        );
        // Una nota lleva su texto y NO un dibujo: los dos a la vez darian una
        // hoja que es dos cosas.
        let h = hoja_de_mensaje(&mensaje(Clase::Nota, "", "hay que picar"), &textos);
        assert_eq!(h.nota.as_deref(), Some("hay que picar"));
        assert!(h.dibujo.is_none());
    }

    #[test]
    fn unir_dos_veces_lo_mismo_no_deja_la_hoja_repetida() {
        let c = std::env::temp_dir().join(format!("pixpin-unir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&c);
        std::fs::create_dir_all(&c).unwrap();
        let mut p = pixpin_proyecto::Proyecto {
            id: "p1".into(),
            nombre: "Casa".into(),
            ..Default::default()
        };
        let textos = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let m = mensaje(Clase::Nota, "", "hay que picar");
        for _ in 0..2 {
            let hoja = hoja_de_mensaje(&m, &textos);
            match p.hojas.iter_mut().find(|h| h.id == hoja.id) {
                Some(vieja) => *vieja = hoja,
                None => p.hojas.push(hoja),
            }
        }
        assert_eq!(p.hojas.len(), 1);
        // Y lo escrito se vuelve a leer igual, con su vinculo dentro.
        guardar_proyecto_json(&c, &p).unwrap();
        let vuelta = leer_proyecto_json(&c);
        assert_eq!(vuelta.hojas.len(), 1);
        assert_eq!(
            vuelta.hojas[0]
                .resto
                .get("deMensaje")
                .and_then(|v| v.as_str()),
            Some("m1")
        );
        // Caso negativo: una carpeta sin `proyecto.json` no es un error, es un
        // proyecto sin hojas todavia.
        let vacia = c.join("otra");
        std::fs::create_dir_all(&vacia).unwrap();
        assert!(leer_proyecto_json(&vacia).hojas.is_empty());
        let _ = std::fs::remove_dir_all(&c);
    }
}

#[cfg(test)]
mod pruebas_cuaderno {
    use super::quitar_del_cuaderno;

    fn carpeta(nombre: &str) -> std::path::PathBuf {
        let c = std::env::temp_dir().join(format!("pixpin-chat-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&c);
        std::fs::create_dir_all(&c).unwrap();
        c
    }

    #[test]
    fn borrar_quita_solo_esos_mensajes_y_deja_intacto_lo_que_no_entiende() {
        let c = carpeta("borrar");
        let texto = "{\"id\":\"1\",\"cuando\":1,\"texto\":\"uno\"}\n\
                     esto no es json\n\
                     {\"id\":\"2\",\"cuando\":2,\"texto\":\"dos\"}\n";
        std::fs::write(c.join("guardados.jsonl"), texto).unwrap();
        let ids = ["1".to_string()].into_iter().collect();
        assert_eq!(quitar_del_cuaderno(&c, &ids).unwrap(), 1);
        let queda = std::fs::read_to_string(c.join("guardados.jsonl")).unwrap();
        assert!(!queda.contains("\"uno\""));
        assert!(queda.contains("esto no es json"), "lo raro se queda");
        assert!(queda.contains("\"dos\""));
        let _ = std::fs::remove_dir_all(&c);
    }

    #[test]
    fn borrar_algo_que_no_esta_no_reescribe_el_cuaderno() {
        let c = carpeta("nada");
        let texto = "{\"id\":\"1\",\"cuando\":1,\"texto\":\"uno\"}\n";
        std::fs::write(c.join("guardados.jsonl"), texto).unwrap();
        let ids = ["9".to_string()].into_iter().collect();
        assert_eq!(quitar_del_cuaderno(&c, &ids).unwrap(), 0);
        assert_eq!(
            std::fs::read_to_string(c.join("guardados.jsonl")).unwrap(),
            texto
        );
        let _ = std::fs::remove_dir_all(&c);
    }
}

// --- Lo que atiende el bucle con el proyecto abierto ------------------

/// Reparte la ventana, con el sitio de la barra de responder si se esta
/// contestando a algo.
fn disponer(marco: Rect, escala: u32, ancho_lista: u32, abierto: Option<&Abierto>) -> Disposicion {
    let mut d = Disposicion::calcular(marco.ancho, marco.alto, escala, ancho_lista, Vista::Ambas);
    let hay_barra = abierto.is_some_and(|a| {
        (a.respondiendo.is_some() || resultados_de_la_busqueda(a).is_some())
            && a.info.is_none()
            && a.hoja.is_none()
            && a.mini.is_none()
    });
    if hay_barra {
        d.encima_de_la_isla = BARRA_DE_RESPONDER * escala / 100;
    }
    d
}

/// Cuantos resultados tiene la busqueda de la conversacion, o `None` si no se
/// esta buscando de verdad.
///
/// Sin nada escrito no hay recuento: la caja recien abierta ensena la
/// conversacion entera, y decir «312 resultados» ahi no informa de nada.
/// Tampoco con cero: para eso ya esta el «Nada con eso» del historial.
fn resultados_de_la_busqueda(a: &Abierto) -> Option<usize> {
    a.busqueda.as_deref().filter(|q| !q.trim().is_empty())?;
    match cuantas_se_ven(a) {
        0 => None,
        n => Some(n),
    }
}

/// Lo que mide la barra de «a quien se contesta», con su aire de abajo.
const BARRA_DE_RESPONDER: u32 = 46;

/// Los mensajes elegidos, por su posicion y del mas viejo al mas nuevo.
fn indices_marcados(a: &Abierto) -> Vec<usize> {
    a.mensajes
        .iter()
        .enumerate()
        .filter(|(_, m)| a.marcados.contains(&m.id))
        .map(|(i, _)| i)
        .collect()
}

/// Copia el texto de lo elegido, con una linea en blanco entre uno y otro
/// para que lo pegado se lea como lo que era (`ChatActivity.java:3699`).
fn copiar_marcados(a: &mut Abierto, textos: &Catalogo) -> Efecto {
    let junto = indices_marcados(a)
        .into_iter()
        .filter_map(|i| a.mensajes.get(i))
        .filter(|m| m.clase != Some(pixpin_proyecto::cuaderno::Clase::MiniApp))
        .map(|m| m.texto.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    a.marcados.clear();
    match pixpin_codec::portapapeles::copiar_texto(&junto) {
        Ok(()) => Efecto::Aviso(textos.t("chat-copiado")),
        Err(e) => {
            tracing::warn!(?e, "no se pudo copiar");
            Efecto::Aviso(textos.t("chat-no-se-pudo"))
        }
    }
}

/// Fija lo elegido; si ya estaba todo fijado, lo suelta. Con una seleccion
/// mezclada, «fijar» es lo que se espera (`fijarTodos` del movil).
fn fijar_marcados(ubicacion: &Ubicacion, a: &mut Abierto, textos: &Catalogo) -> Efecto {
    let indices = indices_marcados(a);
    let fijar = indices.iter().any(|&i| !a.mensajes[i].fijado);
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    let mut fallos = 0;
    for i in indices {
        let mut m = a.mensajes[i].clone();
        if m.fijado == fijar {
            continue;
        }
        m.fijado = fijar;
        match pixpin_proyecto::cuaderno::reemplazar(&carpeta, &m) {
            Ok(_) => a.mensajes[i] = m,
            Err(e) => {
                tracing::warn!(?e, "no se pudo fijar");
                fallos += 1;
            }
        }
    }
    a.fijado = a.mensajes.iter().rposition(|m| m.fijado);
    a.marcados.clear();
    if fallos > 0 {
        Efecto::Aviso(textos.t("chat-no-se-pudo"))
    } else {
        Efecto::Cambio
    }
}

/// Guarda el nombre nuevo de un mensaje (un lienzo). En blanco no se toca:
/// una fila sin nombre no se puede buscar ni nombrar.
fn guardar_nombre_de_mensaje(ubicacion: &Ubicacion, a: &mut Abierto, i: usize, nombre: &str) {
    if nombre.is_empty() {
        return;
    }
    let Some(mut m) = a.mensajes.get(i).cloned() else {
        return;
    };
    m.nombre = nombre.chars().take(80).collect();
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    match pixpin_proyecto::cuaderno::reemplazar(&carpeta, &m) {
        Ok(_) => a.mensajes[i] = m,
        Err(e) => tracing::warn!(?e, "no se pudo cambiar el nombre"),
    }
}

/// Abre el lienzo del que viene una zona (`abrirElOrigen` del movil): la
/// hoja de ESTE proyecto cuyo dibujo es el de «Viene de». Si no esta aqui,
/// se dice en vez de no hacer nada.
fn abrir_el_origen(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    i: usize,
    lienzo: OpcionesLienzo,
    textos: &Catalogo,
) -> Efecto {
    let dibujo = a
        .mensajes
        .get(i)
        .and_then(viene_de)
        .and_then(|(_, dibujo)| dibujo);
    let hoja = dibujo.and_then(|d| {
        a.mensajes.iter().position(|m| {
            m.clase == Some(pixpin_proyecto::cuaderno::Clase::Dibujo)
                && m.referencia.as_deref() == Some(d.as_str())
        })
    });
    let Some(j) = hoja else {
        return Efecto::Aviso(textos.t("chat-sin-origen"));
    };
    for k in abrir_dibujo(ubicacion, a, j, lienzo) {
        if let Some(m) = a.mensajes.get(k).cloned() {
            a.vistas[k] = leer_vista(ubicacion, &a.ficha.id, &m);
        }
    }
    Efecto::Cambio
}

#[cfg(test)]
mod pruebas_piezas {
    use super::{extension_de, viene_de};
    use pixpin_proyecto::cuaderno::Mensaje;

    #[test]
    fn viene_de_se_lee_de_lo_que_escribe_el_movil() {
        let m: Mensaje = serde_json::from_str(
            r#"{"id":"1","cuando":1,"vieneDe":{"texto":"PDF «GE_Sem16» → página 5","dibujo":"ABC","pagina":4}}"#,
        )
        .unwrap();
        let (texto, dibujo) = viene_de(&m).expect("lo trae");
        assert_eq!(texto, "PDF «GE_Sem16» → página 5");
        assert_eq!(dibujo.as_deref(), Some("ABC"));
    }

    #[test]
    fn sin_viene_de_o_vacio_no_hay_tarjeta() {
        let m: Mensaje = serde_json::from_str(r#"{"id":"1","cuando":1}"#).unwrap();
        assert!(viene_de(&m).is_none());
        // Caso negativo: una tarjeta sin texto no diria nada.
        let m: Mensaje =
            serde_json::from_str(r#"{"id":"1","cuando":1,"vieneDe":{"texto":"  "}}"#).unwrap();
        assert!(viene_de(&m).is_none());
    }

    #[test]
    fn la_extension_de_la_fila_va_en_minusculas() {
        assert_eq!(extension_de("GE_Sem16.pdf"), "pdf");
        assert_eq!(extension_de("Lienzo"), "");
    }
}

#[cfg(test)]
mod pruebas_universo {
    use super::*;
    use crate::universo::Pedido;

    #[test]
    fn ctrl_u_con_un_proyecto_abierto_va_a_su_galaxia_y_sin_ninguno_al_cosmos() {
        assert_eq!(pedido_de_ctrl_u(Some("p1")), Pedido::Galaxia("p1".into()));
        assert_eq!(pedido_de_ctrl_u(None), Pedido::Cosmos);
    }

    #[test]
    fn meter_dos_ficheros_en_un_proyecto_deja_dos_mensajes_con_su_ruta_y_su_clase() {
        use pixpin_proyecto::{almacen, cuaderno};
        let raiz = std::env::temp_dir().join(format!("pixpin-meter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let ficheros = vec![
            ("plano.pdf".to_string(), b"%PDF".to_vec()),
            ("foto.png".to_string(), vec![1, 2, 3]),
        ];
        let hechos = meter_en_proyecto(&raiz, "p1", &ficheros, "K7Q2").unwrap();
        assert_eq!(hechos.len(), 2);
        assert_eq!(hechos[0].ruta.as_deref(), Some("archivos/plano.pdf"));
        assert_eq!(
            hechos[0].clase,
            Some(cuaderno::clase_de_nombre("plano.pdf"))
        );
        assert_eq!(hechos[1].clase, Some(cuaderno::Clase::Imagen));
        assert_eq!(hechos[1].numero, hechos[0].numero + 1, "numeros seguidos");
        let leido = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&raiz, "p1")).unwrap();
        assert_eq!(leido.mensajes.len(), 2);
        // Caso negativo: el mismo nombre otra vez no pisa el primero, y el
        // numero sigue donde iba.
        let otra = meter_en_proyecto(&raiz, "p1", &ficheros[..1], "K7Q2").unwrap();
        assert_eq!(otra[0].ruta.as_deref(), Some("archivos/plano (1).pdf"));
        assert_eq!(otra[0].numero, hechos[1].numero + 1);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_mensaje_se_encuentra_por_su_codigo_unico_y_uno_que_no_esta_no() {
        use pixpin_proyecto::cuaderno::Mensaje;
        let a = Mensaje {
            id: "1".into(),
            uid: Some("A".into()),
            ..Default::default()
        };
        let b = Mensaje {
            id: "2".into(),
            uid: Some("B".into()),
            ..Default::default()
        };
        let v = vec![a, b.clone()];
        assert_eq!(indice_de_codigo(&v, &b.codigo_unico()), Some(1));
        assert_eq!(indice_de_codigo(&v, "m:nada"), None);
    }

    #[test]
    fn ir_al_chat_deja_un_solo_destino_y_gana_el_ultimo() {
        tomar_ir_a();
        if let Ok(mut g) = IR_A.lock() {
            *g = Some(("p1".into(), None));
            *g = Some(("p2".into(), Some("m:x".into())));
        }
        assert_eq!(tomar_ir_a(), Some(("p2".into(), Some("m:x".into()))));
        // Caso negativo: recogido una vez, no se repite.
        assert_eq!(tomar_ir_a(), None);
    }

    #[test]
    fn los_tres_puntos_ofrecen_ver_el_proyecto_en_el_universo() {
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        for en_guardados in [false, true] {
            let v = menu_de_cabecera(&textos, en_guardados, "p1");
            let n = v
                .iter()
                .filter(|e| e.accion == Accion::Universo(Pedido::Galaxia("p1".into())))
                .count();
            assert_eq!(n, 1, "una sola vez, en guardados={en_guardados}");
        }
        // Caso negativo: ninguna entrada lleva a otro proyecto ni al cosmos.
        assert!(
            !menu_de_cabecera(&textos, false, "p1")
                .iter()
                .any(|e| matches!(
                    &e.accion,
                    Accion::Universo(Pedido::Cosmos | Pedido::Luna { .. })
                ))
        );
    }
}
