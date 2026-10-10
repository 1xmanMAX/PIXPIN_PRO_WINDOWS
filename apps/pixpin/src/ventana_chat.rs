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

use pixpin_render::icono::{Icono, material as mi};

use crate::caja_dibujo::hex;
use crate::overlay::Recursos;

/// La caja flotante a la que se sueltan ficheros para un proyecto (2-oct).
mod caja_de_soltar;
/// La descripcion de una foto: su `texto`, como en el movil (3-oct).
mod descripcion;
/// Donde se guarda cada proyecto (su carpeta elegida o la habitual).
mod donde_vive;
/// v2-menus: el cuadro de enviar fotos (ordenar arrastrando, quitar).
mod envio;
/// v2-menus: la hoja del «+» de la caja, cuadricula con buscador.
mod hoja_adjuntar;
/// Pegar desde Excel o Sheets en la hoja, y copiar hacia ellos (J2).
mod hoja_portapapeles;
/// Los archivos con el icono de su tipo, un color por extension (v0.98.4).
mod icono_de_tipo;
/// Un Excel del chat es un libro de tablas (D11).
mod libro;
/// El logo de un proyecto: una imagen en vez de las iniciales (4-oct).
mod logo;
/// Mayus+clic marca un tramo de burbujas.
mod marcar;
/// v2-menus: geometria y teclas de los menus (`Menus2.dc.html`).
mod menu_v2;
/// El microfono flotante del pedido «grabar»: graba, guarda y ofrece
/// convertir la nota en llamada secreta, sin abrir el chat (2-oct).
mod microfono_flotante;
/// Muestra en PNG de una foto con su descripcion (3-oct).
#[cfg(test)]
mod muestra_descripcion;
/// v2-menus: muestras en PNG de los menus nuevos.
#[cfg(test)]
mod muestra_menus;
/// F8: muestra en PNG del chat con una zona recien mandada.
#[cfg(test)]
mod muestra_zona;
/// Clip → «Una pagina de un proyecto».
mod paginas;
/// La vista previa de un lienzo, con el papel y la tinta del lienzo.
#[cfg(test)]
mod papel_de_la_vista;
/// La pantalla de Proyectos del movil y el interruptor Chat / Proyectos.
mod proyectos;
/// «Quien llama» en la llamada secreta de una nota de voz (B11, v0.98.6).
mod quien_llama;
/// El cursor de la caja de escribir: la rayita, lo seleccionado y que letra
/// cae bajo el raton (10-oct).
mod redaccion;
mod renombrar;
/// El reproductor flotante del pedido «reproducir»: un audio suena sin
/// abrir el chat y la ventanita se va al acabar (3-oct).
mod reproductor_flotante;
/// La tarjeta del enlace, «Recibido de …» y las fechas del buzon.
mod tarjetas;

/// Tamano con el que nace, en pixeles logicos (el de Telegram en escritorio).
const ANCHO_LOGICO: u32 = 1024;
const ALTO_LOGICO: u32 = 768;
const VK_ESCAPE: u32 = 0x1B;
const VK_RETROCESO: u32 = 0x08;
const VK_ENTRAR: u32 = 0x0D;
const VK_V: u32 = 0x56;
#[cfg(test)]
const VK_U: u32 = 0x55;
const VK_C: u32 = 0x43;
/// Las teclas de la escala de la interfaz. Las dos de cada signo porque el
/// «+» del teclado numerico y el de la fila de arriba son teclas distintas,
/// y quien agranda usa la que tiene mas a mano.
const VK_MAS: u32 = 0xBB;
const VK_MENOS: u32 = 0xBD;
const VK_MAS_NUM: u32 = 0x6B;
const VK_MENOS_NUM: u32 = 0x6D;
const VK_CERO: u32 = 0x30;
const VK_CERO_NUM: u32 = 0x60;

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
            let hecho = crate::dispositivo_perdido::con_recursos("chat", |r| {
                abrir(r, &textos, &ubicacion, lienzo, idioma)
            });
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
    anadir_hojas_del_proyecto(
        ubicacion,
        &a.ficha.id,
        &a.ficha.aparato.clone().unwrap_or_default(),
        &mut mensajes,
    );
    // Sincronizar puede haber traido hojas nuevas: la galeria tiene que
    // enterarse en la misma vuelta que el cuaderno.
    a.hojas_uid = pixpin_proyecto::almacen::uids_de_hojas(ubicacion.raiz(), &a.ficha.id);
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
    // Donde quedo la ultima vez, si cabe en algun monitor; si no, centrada.
    let mut estado = pixpin_store::estado::cargar(ubicacion);
    // Lo que manda el monitor por su DPI, y encima la escala de la interfaz
    // que el usuario haya elegido (Ctrl + «+» / «-» / «0», o el menu).
    let escala_monitor = monitor.escala_por_cien;
    let mut factor_escala =
        chat::escala_valida(estado.escala_interfaz.unwrap_or(chat::ESCALA_POR_DEFECTO));
    let mut escala = escala_monitor * factor_escala / 100;
    let e = |v: u32| v * escala / 100;

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

    // M1: con el tema Cosmos, siempre los colores de noche (los del Cosmos).
    let tema = if pixpin_shell::entorno::tema_claro() && !crate::tema_cosmos::activo() {
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
    let claro = pixpin_shell::entorno::tema_claro() && !crate::tema_cosmos::activo();
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
    // Las paginas de PDF y los «Anadir al proyecto» se hacen en otros hilos
    // (`pdf_en_chat`): que despierten a esta ventana cuando acaben.
    crate::pdf_en_chat::avisar_a(ventana.handle().0 as isize);
    crate::aligerar::avisar_a(ventana.handle().0 as isize);
    // La fecha del indice la ultima vez que se leyo, para enterarse de lo
    // que llegue por fuera (del movil, de otra ventana).
    let mut sello_indice: Option<std::time::SystemTime> = None;
    // Hay que releer el cuaderno del proyecto abierto, pero puede que todavia
    // no se pueda (ver `releer_lo_abierto`): se guarda la deuda en vez de
    // perderla, y se salda en cuanto la hoja o el lienzo se cierren.
    let mut pendiente_de_releer = false;
    // La pantalla de Proyectos del movil y el interruptor de la barra
    // (`proyectos.rs`): se abre en la que se eligio la ultima vez.
    let mut vista_proyectos = proyectos::VistaProyectos::nueva(ubicacion);

    loop {
        pixpin_shell::overlay::bombear_pendientes();
        // Delante, se apunta como lo abierto (`al_frente`) con el chat que
        // ensena: es adonde va lo que llegue de otro aparato del grupo.
        if pixpin_render::superficie::en_primer_plano(ventana.handle()) {
            crate::al_frente::poner_chat(
                ventana.handle().0 as isize,
                abierto
                    .as_ref()
                    .map(|a| (a.ficha.id.as_str(), a.ficha.nombre.as_str())),
            );
        }
        // Lo primero: que el tamano con el que se pinta y se reparten los
        // clics sea el que la ventana ocupa DE VERDAD. Windows la cambia por
        // su cuenta —otro monitor con otro DPI, Win+flecha, maximizar desde
        // la barra de tareas— y sin esto la aplicacion seguia con el tamano
        // viejo: pintaba en un trozo y buscaba los botones en otro, que es
        // como se quedo «sin poder pulsar ni mover nada».
        if let Some(real) = ventana.rect_del_sistema()
            && real != marco
        {
            let cambia_el_tamano = (real.ancho, real.alto) != (marco.ancho, marco.alto);
            marco = real;
            if cambia_el_tamano {
                let _ = superficie.redimensionar(marco.ancho, marco.alto);
                ancho_lista = chat::ancho_ajustado(ancho_lista as i32, marco.ancho, escala);
                if let Some(a) = abierto.as_mut() {
                    a.colocado.borrow_mut().ancho = 0;
                }
            }
            hay_que_pintar = true;
        }
        let mut cerrar = false;
        let disposicion = disponer(marco, escala, ancho_lista, abierto.as_ref());
        let mut nuevo_marco: Option<Rect> = None;
        // Lo del bucle que la pantalla de Proyectos puede tocar al cumplir
        // un pedido (`proyectos::cumplir`).
        macro_rules! bucle_de_proyectos {
            () => {
                proyectos::Bucle {
                    ubicacion,
                    textos,
                    idioma,
                    lienzo,
                    identidad: &identidad,
                    ventana: &ventana,
                    fichas: &fichas,
                    orden: &orden,
                    abierto: &mut abierto,
                    elegida: &mut elegida,
                    borradores: &mut borradores,
                    aviso: &mut aviso,
                    cerrar: &mut cerrar,
                }
            };
        }
        // Lo que se elige en un menu abierto, con el raton o con su tecla:
        // las dos vias pasan por aqui (v2-menus).
        macro_rules! hacer_del_menu {
            ($ancla:expr, $accion:expr) => {{
                let ancla_del_menu: Punto = $ancla;
                match $accion {
                    None => {}
                    Some(Accion::AdjArchivo) => {
                        pendientes = Pendientes::con_lo_escrito(
                            pixpin_shell::elegir::pedir_ficheros(ventana.handle()),
                            abierto.as_mut(),
                        );
                    }
                    Some(Accion::AdjImagen) => {
                        pendientes = Pendientes::con_lo_escrito(
                            pixpin_shell::elegir::pedir_imagenes(ventana.handle()),
                            abierto.as_mut(),
                        );
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
                            ancla_del_menu,
                            menu_de_fondos(textos, fondo),
                        ));
                    }
                    Some(Accion::Fondo(n)) => {
                        fondo = n;
                        guardar_fondo(ubicacion, n);
                    }
                    Some(Accion::Escalas) => {
                        menu = Some(MenuAbierto::nuevo(
                            ancla_del_menu,
                            menu_de_escalas(textos, factor_escala),
                        ));
                    }
                    Some(Accion::Escala(cuanto)) => {
                        factor_escala = chat::escala_valida(cuanto);
                        escala = escala_monitor * factor_escala / 100;
                        recordar_escala(ubicacion, &mut estado, factor_escala);
                        if let Some(a) = abierto.as_mut() {
                            a.colocado.borrow_mut().ancho = 0;
                        }
                    }
                    // ⋮ → Proyectos lleva a la pantalla de Proyectos,
                    // como `abrirLaPortada(enProyectos = true)` del
                    // movil. Es ir, no elegir: lo que recuerda el
                    // interruptor no cambia.
                    Some(Accion::Proyectos) => vista_proyectos.ver_proyectos(),
                    // «Captura» de la hoja del «+»: la de zona, por el mismo camino
                    // que el atajo general (como el pedido `capturar`). Un poco
                    // despues, para que el menu ya no salga en la foto.
                    Some(Accion::AdjCaptura) => {
                        let _ = std::thread::Builder::new()
                            .name("captura-del-chat".into())
                            .spawn(|| {
                                std::thread::sleep(std::time::Duration::from_millis(200));
                                let id = pixpin_store::comandos::Comando::CapturarRegion.id();
                                if !pixpin_shell::mensajero::pedir_ventana_principal(id) {
                                    tracing::warn!("la captura del chat no llego a la ventana");
                                }
                            });
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
                                Efecto::Aviso(t) => aviso = Some((t, std::time::Instant::now())),
                                Efecto::Menu(v) => {
                                    menu = Some(MenuAbierto::nuevo(ancla_del_menu, v))
                                }
                            }
                            a.colocado.borrow_mut().ancho = 0;
                        }
                    }
                }
            }};
        }
        // La tarjeta de Proyectos es la del proyecto que el chat tiene abierto.
        vista_proyectos.seguir(abierto.as_ref().map(|a| a.ficha.id.as_str()));

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
            // En modo Proyectos la ventana es la del chat de siempre y solo
            // cambia el panel derecho: la tarjeta del proyecto elegido en vez
            // de su conversacion (`proyectos.rs`). Lo que cae en el panel es
            // de la tarjeta; lo de la lista sigue su camino. El teclado es de
            // la tarjeta salvo mientras se escribe en el buscador o el nombre
            // de un proyecto recien creado con el «+».
            if vista_proyectos.activa() && pendientes.is_none() && menu.is_none() {
                let hay_tarjeta = abierto.is_some() && disposicion.chat.ancho > 0;
                let escribiendo = buscando || abierto.as_ref().is_some_and(|a| a.renombrando);
                match evento {
                    EventoOverlay::Rueda(delta) => {
                        let aqui = local(pixpin_shell::entorno::posicion_del_cursor());
                        if hay_tarjeta && disposicion.chat.contiene(aqui) {
                            vista_proyectos.mover(aqui);
                            if let Some(pedido) = vista_proyectos.evento(evento) {
                                hay_que_pintar |= proyectos::cumplir(
                                    pedido,
                                    &mut vista_proyectos,
                                    &mut bucle_de_proyectos!(),
                                );
                            }
                        } else {
                            // La lista, aunque detras el chat tenga abierta
                            // una hoja o un panel que se llevarian la rueda.
                            let paso = (chat::FILA * 3 / 2 * escala / 100) as i32;
                            let nuevo = disposicion.scroll_ajustado(
                                scroll - giro_de_rueda(delta, paso),
                                orden.len(),
                                escala,
                            );
                            if nuevo != scroll {
                                scroll = nuevo;
                                fila_sobre = disposicion.fila_en(aqui, scroll, orden.len(), escala);
                                hay_que_pintar = true;
                            }
                        }
                        continue;
                    }
                    EventoOverlay::RuedaHorizontal(_) if hay_tarjeta => {
                        if let Some(pedido) = vista_proyectos.evento(evento) {
                            hay_que_pintar |= proyectos::cumplir(
                                pedido,
                                &mut vista_proyectos,
                                &mut bucle_de_proyectos!(),
                            );
                        }
                        continue;
                    }
                    // A pantalla completa, Escape devuelve la ventana a su
                    // sitio, como en el chat: no la cierra.
                    EventoOverlay::Tecla { vk, .. }
                        if !escribiendo && !(vk == VK_ESCAPE && antes_de_maximizar.is_some()) =>
                    {
                        if let Some(pedido) = vista_proyectos.evento(evento) {
                            hay_que_pintar |= proyectos::cumplir(
                                pedido,
                                &mut vista_proyectos,
                                &mut bucle_de_proyectos!(),
                            );
                            continue;
                        }
                    }
                    EventoOverlay::Caracter(_) if !escribiendo => {
                        if let Some(pedido) = vista_proyectos.evento(evento) {
                            hay_que_pintar |= proyectos::cumplir(
                                pedido,
                                &mut vista_proyectos,
                                &mut bucle_de_proyectos!(),
                            );
                            continue;
                        }
                    }
                    // Lo que se suelta encima va al chat de su proyecto, que
                    // es donde se pregunta antes de meterlo.
                    EventoOverlay::FicherosSoltados if hay_tarjeta => {
                        proyectos::abrir_el_que_se_ve(
                            &mut vista_proyectos,
                            &mut bucle_de_proyectos!(),
                        );
                        hay_que_pintar = true;
                    }
                    EventoOverlay::BotonDerechoPulsado(p)
                        if hay_tarjeta && disposicion.chat.contiene(local(p)) =>
                    {
                        let pedido = vista_proyectos.pulsar_derecho(local(p));
                        hay_que_pintar |= proyectos::cumplir(
                            pedido,
                            &mut vista_proyectos,
                            &mut bucle_de_proyectos!(),
                        );
                        continue;
                    }
                    EventoOverlay::RatonMovido(p)
                        if arrastre.is_none()
                            && hay_tarjeta
                            && disposicion.chat.contiene(local(p)) =>
                    {
                        // Sobre la tarjeta no hay burbujas: solo se apunta
                        // donde esta el raton (para la rueda) y se quita el
                        // resalte de la fila de la lista que se dejo atras.
                        let l = local(p);
                        vista_proyectos.mover(l);
                        ventana.poner_cursor(
                            chat::borde_en(l, marco.ancho, marco.alto, escala)
                                .map(cursor_de)
                                .unwrap_or(FormaCursorWin::Flecha),
                        );
                        if sobre.is_some() || fila_sobre.is_some() {
                            sobre = None;
                            fila_sobre = None;
                            hay_que_pintar = true;
                        }
                        continue;
                    }
                    _ => {}
                }
            }
            match evento {
                // v2-menus: con un menu (o la hoja del «+») delante, el
                // teclado es suyo: sus teclas de una letra, las flechas,
                // Intro y Esc. Van las primeras para que nada de debajo
                // (la caja de escribir, una hoja) se quede con ellas.
                EventoOverlay::Tecla { vk, ctrl, .. } if menu.is_some() => {
                    if let Some(mut m) = menu.take() {
                        let ancla = m.ancla;
                        match m.tecla(vk, ctrl) {
                            Respuesta::Fuera => {}
                            Respuesta::Sigue => menu = Some(m),
                            Respuesta::Hacer(accion) => hacer_del_menu!(ancla, Some(accion)),
                        }
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Caracter(c) if menu.is_some() => {
                    if let Some(mut m) = menu.take() {
                        let ancla = m.ancla;
                        match m.caracter(c) {
                            Respuesta::Fuera => {}
                            Respuesta::Sigue => menu = Some(m),
                            Respuesta::Hacer(accion) => hacer_del_menu!(ancla, Some(accion)),
                        }
                    }
                    hay_que_pintar = true;
                }
                // El resalte sigue al raton, y «Más» se abre al pasar.
                EventoOverlay::RatonMovido(p) if menu.is_some() && arrastre.is_none() => {
                    let l = local(p);
                    if let Some(m) = menu.as_mut()
                        && m.mover(l, marco, escala)
                    {
                        hay_que_pintar = true;
                    }
                    ventana.poner_cursor(FormaCursorWin::Flecha);
                }
                // v2: el cuadro de enviar sigue al raton (resalte y foto
                // arrastrada) y, al soltar, la foto cae en su hueco.
                EventoOverlay::RatonMovido(p) if pendientes.is_some() && arrastre.is_none() => {
                    let l = local(p);
                    if let Some(pend) = pendientes.as_mut() {
                        let sobre = pend.cuadro(marco, escala).sitio_en(l, escala);
                        let antes = (pend.sobre, pend.arrastre);
                        pend.sobre = sobre;
                        if let Some(a) = pend.arrastre.as_mut() {
                            a.ahora = l;
                        }
                        hay_que_pintar |= antes != (pend.sobre, pend.arrastre);
                    }
                    ventana.poner_cursor(FormaCursorWin::Flecha);
                }
                EventoOverlay::BotonSoltado(p)
                    if pendientes.as_ref().is_some_and(|p| p.arrastre.is_some()) =>
                {
                    let l = local(p);
                    ventana.soltar_raton();
                    if let Some(pend) = pendientes.as_mut()
                        && let Some(mut a) = pend.arrastre.take()
                    {
                        a.ahora = l;
                        if a.se_movio(escala) {
                            let hueco = pend.cuadro(marco, escala).hueco_en(l);
                            pend.reordenar(a.desde, hueco);
                        }
                    }
                    hay_que_pintar = true;
                }
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
                    if let Some(p) = pendientes.as_mut() {
                        // v2: el cuadro de enviar (`envio.rs`): el ✕ quita,
                        // una foto se agarra para moverla, «Añadir» pide mas.
                        let d = p.cuadro(marco, escala);
                        match d.sitio_en(l, escala) {
                            Some(envio::Sitio::Enviar) => {
                                if let (Some(p), Some(a)) = (pendientes.take(), abierto.as_mut()) {
                                    let hechos =
                                        meter_ficheros(ubicacion, a, &identidad, &p.rutas, &p.pie);
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
                            Some(envio::Sitio::Quitar(n)) => {
                                p.quitar(n);
                                p.sobre = None;
                                // Sin nada que enviar, el cuadro sobra: se
                                // cancela y lo escrito vuelve a la caja.
                                if p.rutas.is_empty()
                                    && let Some(p) = pendientes.take()
                                {
                                    p.cancelar(abierto.as_mut());
                                }
                            }
                            Some(envio::Sitio::Foto(n)) => {
                                ventana.capturar_raton();
                                p.arrastre = Some(envio::Arrastre {
                                    desde: n,
                                    agarre: l,
                                    ahora: l,
                                });
                            }
                            Some(envio::Sitio::Anadir) => {
                                let mas = pixpin_shell::elegir::pedir_ficheros(ventana.handle());
                                if let Some(p) = pendientes.as_mut() {
                                    p.anadir(mas);
                                }
                            }
                            Some(envio::Sitio::Cancelar) => {
                                if let Some(p) = pendientes.take() {
                                    p.cancelar(abierto.as_mut());
                                }
                            }
                            // Pulsar fuera del cuadro cancela, como el aspa de
                            // cualquier dialogo; dentro, no hace nada.
                            None => {
                                if let Some(p) = pendientes.take() {
                                    p.cancelar(abierto.as_mut());
                                }
                            }
                            Some(envio::Sitio::Dentro) => {}
                        }
                        hay_que_pintar = true;
                    } else if let Some(b) = chat::borde_en(l, marco.ancho, marco.alto, escala) {
                        ventana.capturar_raton();
                        arrastre = Some(Arrastre::Borde(b));
                    } else if let Some(a_proyectos) = vista_proyectos.interruptor_en(l) {
                        // El interruptor Chat / Proyectos (`proyectos.rs`).
                        if a_proyectos != vista_proyectos.activa() {
                            vista_proyectos.poner(ubicacion, a_proyectos);
                            menu = None;
                        }
                        hay_que_pintar = true;
                    } else if let Some(boton) = disposicion.boton_barra_en(l, escala) {
                        match boton {
                            BotonBarra::Minimizar => ventana.minimizar(),
                            BotonBarra::Maximizar => {
                                // La pantalla ENTERA, tapando la barra de
                                // tareas: el usuario lo pidio asi, «que la
                                // app ocupe la pantalla completa, y para
                                // poder salir solo sea con el escape». El
                                // mismo boton la devuelve a su sitio, que es
                                // la otra mitad de lo que se espera de el.
                                //
                                // El monitor se busca ahora y no se usa el de
                                // arrancar: con dos pantallas, «completa» es
                                // la que tiene la ventana delante.
                                nuevo_marco = Some(match antes_de_maximizar.take() {
                                    Some(vuelta) => vuelta,
                                    None => {
                                        antes_de_maximizar = Some(marco);
                                        monitor_de_la_ventana(marco, &monitor)
                                    }
                                });
                            }
                            BotonBarra::Cerrar => cerrar = true,
                        }
                    } else if disposicion.arrastra_ventana(l, escala) {
                        ventana.capturar_raton();
                        arrastre = Some(Arrastre::Ventana(l));
                    } else if vista_proyectos.activa()
                        && menu.is_none()
                        && abierto.is_some()
                        && disposicion.chat.contiene(l)
                        && !disposicion.asa.contiene(l)
                    {
                        // En modo Proyectos el panel derecho es la tarjeta:
                        // el clic ahi es suyo entero (debajo estan los
                        // botones del chat, que no se ven). En la lista
                        // sigue su camino de siempre.
                        let pedido = vista_proyectos.pulsar(l);
                        hay_que_pintar |= proyectos::cumplir(
                            pedido,
                            &mut vista_proyectos,
                            &mut bucle_de_proyectos!(),
                        );
                    } else if disposicion.asa.contiene(l) {
                        ventana.capturar_raton();
                        arrastre = Some(Arrastre::Asa(l.x - ancho_lista as i32));
                    } else if let Some(mut m) = menu.take() {
                        // Con un menu desplegado, el primer clic es suyo: o
                        // elige una entrada o lo cierra. Que ademas hiciera
                        // lo que hubiera debajo seria dispararle al usuario
                        // por querer salir del menu. Un clic en su relleno
                        // o en «Más» lo deja abierto.
                        let ancla = m.ancla;
                        match m.pulsar(l, marco, escala) {
                            Respuesta::Fuera => {}
                            Respuesta::Sigue => menu = Some(m),
                            Respuesta::Hacer(accion) => hacer_del_menu!(ancla, Some(accion)),
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
                                Zona::Menu => {
                                    Efecto::Menu(menu_de_cabecera(textos, a.ficha.es_guardados()))
                                }
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
                                Zona::Enlace(i) => abrir_enlace(a, i, textos),
                                Zona::Ir(j) => ejecutar(Accion::Ir(j), a, &cx),
                                Zona::Lecciones => ejecutar(Accion::Lecciones, a, &cx),
                                Zona::CerrarRespuesta => {
                                    a.respondiendo = None;
                                    Efecto::Nada
                                }
                                Zona::SelCerrar => {
                                    a.marcados.clear();
                                    Efecto::Nada
                                }
                                // La barra del reproductor. Ninguna de estas
                                // toca el disco ni el cuaderno: son ordenes al
                                // motor de sonido, que vive aparte.
                                Zona::VozTocar => {
                                    if crate::audio::estado().sonando {
                                        crate::audio::pausar();
                                    } else if let Err(e) = crate::audio::seguir() {
                                        tracing::warn!(?e, "no se pudo seguir");
                                    }
                                    Efecto::Nada
                                }
                                Zona::VozAtras => {
                                    crate::audio::atras();
                                    Efecto::Nada
                                }
                                Zona::VozAdelante => {
                                    crate::audio::adelante();
                                    Efecto::Nada
                                }
                                Zona::VozVelocidad => {
                                    crate::audio::otra_velocidad();
                                    Efecto::Nada
                                }
                                Zona::VozCerrar => {
                                    crate::audio::parar();
                                    Efecto::Nada
                                }
                                Zona::VozIrA(milesimas) => {
                                    crate::audio::ir_a(milesimas as f32 / 1000.0);
                                    Efecto::Nada
                                }
                                // El boton de texto de la nota de voz: sin
                                // texto lo pide, con texto pliega y despliega.
                                Zona::Texto(i) => match a.mensajes.get(i).map(|m| {
                                    (m.id.clone(), crate::voz::transcripcion_de(m).is_some())
                                }) {
                                    Some((id, true)) => {
                                        if !a.desplegados.remove(&id) {
                                            a.desplegados.insert(id);
                                        }
                                        Efecto::Cambio
                                    }
                                    Some((_, false)) => ejecutar(Accion::Transcribir(i), a, &cx),
                                    None => Efecto::Nada,
                                },
                                Zona::Trozo(i, ms) => match saltar_en_el_audio(a, i, ms) {
                                    Ok(()) => Efecto::Nada,
                                    Err(e) => {
                                        tracing::warn!(?e, "no se pudo saltar en el audio");
                                        Efecto::Aviso(textos.t("chat-voz-no-es-audio"))
                                    }
                                },
                                Zona::LetraVolver => {
                                    a.letra = None;
                                    Efecto::Nada
                                }
                                Zona::LetraMenos | Zona::LetraMas => {
                                    if let Some(l) = a.letra.as_mut() {
                                        l.tam = otro_tamano_de_letra(l.tam, zona == Zona::LetraMas);
                                    }
                                    Efecto::Nada
                                }
                                Zona::LetraCopiar => copiar_la_letra(a, textos),
                                Zona::LetraMarcar => poner_la_marca(ubicacion, a, textos),
                                Zona::LetraEditar => editar_la_letra(a, textos),
                                Zona::Marca(i, ms) => match saltar_en_el_audio(a, i, ms) {
                                    Ok(()) => Efecto::Nada,
                                    Err(e) => {
                                        tracing::warn!(?e, "no se pudo saltar a la banderita");
                                        Efecto::Aviso(textos.t("chat-voz-no-es-audio"))
                                    }
                                },
                                Zona::CerrarHora => {
                                    a.hora_a_mano = None;
                                    Efecto::Nada
                                }
                                Zona::CerrarQuien => {
                                    a.quien_llama = None;
                                    Efecto::Nada
                                }
                                Zona::Renombrar(i) => ejecutar(Accion::Renombrar(i), a, &cx),
                                Zona::CerrarNombre => {
                                    a.renombrando_mensaje = None;
                                    Efecto::Nada
                                }
                                Zona::SelCopiar => copiar_marcados(a, textos),
                                Zona::SelCompartir => {
                                    let indices = indices_marcados(a);
                                    compartir_mensajes(a, &indices, &cx)
                                }
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
                        // Una miniatura de la galeria abre lo que ensena, como
                        // si se hubiera pulsado su burbuja. Se mira ANTES que
                        // el resto del panel porque abre otra cosa encima y se
                        // lleva el prestamo de `abierto` entero.
                        let cual = abierto
                            .as_ref()
                            .and_then(|a| cosa_de_la_cuadricula(a, l, &disposicion, escala));
                        if let Some(indice) = cual {
                            if let Some(a) = abierto.as_mut() {
                                a.info = None;
                                a.buscando_info = false;
                                a.busqueda_info.clear();
                            }
                            tocar_la_burbuja(
                                indice,
                                ubicacion,
                                &mut abierto,
                                &fichas,
                                &mut elegida,
                                &mut borradores,
                                textos,
                                lienzo,
                                idioma,
                                &identidad,
                            );
                            hay_que_pintar = true;
                            continue;
                        }
                        // Con el panel abierto, la columna de la derecha es
                        // suya: no se pincha ni el historial ni la caja.
                        if let Some(a) = abierto.as_mut() {
                            let d = disposicion_info(&disposicion, escala);
                            // Fuera de la capa se cierra, como cualquier capa
                            // con velo: lo de debajo esta esperando, y pulsarlo
                            // es decir que ya se vio bastante.
                            if !d.panel.contiene(l) {
                                a.info = None;
                                a.buscando_info = false;
                                a.busqueda_info.clear();
                                hay_que_pintar = true;
                                continue;
                            }
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
                    } else if abierto.as_ref().is_some_and(|a| a.letra.is_some()) {
                        // Con la letra abierta, lo que se pulsa de ella ya
                        // lo atendieron sus zonas. El resto es el hueco entre
                        // parrafos (nada) o la cabecera del chat, que vuelve.
                        // Sin esta rama el clic caeria en una burbuja que
                        // no se ve.
                        if let Some(a) = abierto.as_mut()
                            && disposicion.cabecera_chat.contiene(l)
                        {
                            a.letra = None;
                        }
                        hay_que_pintar = true;
                    } else if abierto.as_ref().is_some_and(|a| a.biblioteca.is_some()) {
                        if let Some(a) = abierto.as_mut() {
                            if disposicion.cabecera_chat.contiene(l) {
                                a.biblioteca = None;
                            } else if let Some(b) = a.biblioteca.as_ref() {
                                pulsar_biblioteca(b, l, &disposicion, escala);
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
                                let e = pulsar_mini(a, l, &disposicion, escala, textos);
                                cumplir_mini(ubicacion, a, e, &disposicion, escala, textos);
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
                        let teclas = pixpin_shell::entrada::modificadores_pulsados();
                        let con_ctrl = teclas.ctrl;
                        if let Some(a) = abierto
                            .as_mut()
                            .filter(|a| con_ctrl || !a.marcados.is_empty())
                        {
                            // Mayus+clic con algo ya marcado: el tramo entero
                            // (`marcar::tramo`), como cualquier lista de Windows.
                            let tramo = if teclas.shift && !a.marcados.is_empty() {
                                let visibles = indices_visibles(a);
                                marcar::tramo(
                                    &visibles,
                                    |i| {
                                        a.mensajes
                                            .get(i)
                                            .is_some_and(|m| a.marcados.contains(&m.id))
                                    },
                                    indice,
                                )
                            } else {
                                Vec::new()
                            };
                            if !tramo.is_empty() {
                                for i in tramo {
                                    if let Some(id) = a.mensajes.get(i).map(|m| m.id.clone()) {
                                        a.marcados.insert(id);
                                    }
                                }
                            } else if let Some(id) = a.mensajes.get(indice).map(|m| m.id.clone())
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
                    } else if let Some(extra) = pixpin_ui::chat::Extra::TODOS
                        .into_iter()
                        .find(|x| disposicion.boton_extra(*x, escala).contiene(l))
                    {
                        // Lecciones, galeria de capturas y tareas, cada una
                        // en su ventana (en el sitio del universo, 3-oct).
                        match extra {
                            pixpin_ui::chat::Extra::Galeria => {
                                crate::galeria_capturas::abrir(idioma, ubicacion.clone())
                            }
                            pixpin_ui::chat::Extra::Tareas => {
                                crate::tareas::abrir(idioma, ubicacion.clone(), &identidad)
                            }
                            // Y con el las lecciones, en su pestana: un solo
                            // boton para las dos (8-oct-2026, el usuario).
                            pixpin_ui::chat::Extra::Timeline => {
                                crate::timeline::abrir(idioma, ubicacion.clone())
                            }
                        }
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
                        // v2: la hoja del «+», cuadricula con buscador.
                        // Su ancla es la esquina de arriba del boton: la
                        // hoja crece hacia arriba desde ahi, y lo que abra
                        // despues (los proyectos, las mini-apps) nace ahi.
                        menu = Some(MenuAbierto::de_hoja(
                            Punto {
                                x: clip.derecha(),
                                y: clip.y,
                            },
                            hoja_adjuntar::Hoja::nueva(textos),
                        ));
                        buscando = false;
                        hay_que_pintar = true;
                    } else if abierto.as_ref().is_some_and(|a| {
                        disposicion
                            .boton_enviar(a.alto_caja.get(), escala)
                            .contiene(l)
                    }) {
                        if let Some(a) = abierto.as_mut() {
                            if a.dictado.is_some() {
                                // Dictando, el boton es «ya he terminado»: lo
                                // dicho queda en la caja para repasarlo antes
                                // de enviarlo, no se envia solo.
                                a.dictado = None;
                            } else if a.grabando.is_some() {
                                // Ya se estaba grabando: este clic la cierra.
                                // Una nota de menos de 700 ms no se guarda y
                                // no deja fichero: fue un resbalon del raton.
                                let rotulo = match terminar_de_grabar(ubicacion, a, &identidad) {
                                    Ok(()) => {
                                        a.scroll = None;
                                        // La nota NO se pasa a texto sola: lo
                                        // pidio el usuario («solo cuando yo
                                        // quiera»). Queda el boton de la
                                        // burbuja y la entrada del menu.
                                        None
                                    }
                                    Err(pixpin_audio::ErrorAudio::DemasiadoCorta { .. }) => {
                                        Some(textos.t("chat-voz-muy-corta"))
                                    }
                                    Err(e) => {
                                        tracing::warn!(?e, "no se pudo cerrar la nota de voz");
                                        Some(textos.t("chat-voz-fallo"))
                                    }
                                };
                                if let Some(t) = rotulo {
                                    aviso = Some((t, std::time::Instant::now()));
                                }
                            } else if a.borrador.trim().is_empty() {
                                // Sin nada escrito, el boton es el microfono.
                                if let Err(e) = empezar_a_grabar(ubicacion, a) {
                                    tracing::warn!(?e, "no se pudo abrir el microfono");
                                    aviso = Some((
                                        textos.t(rotulo_de_no_grabar(&e)),
                                        std::time::Instant::now(),
                                    ));
                                }
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
                    } else if let Some(i) =
                        abierto.as_ref().and_then(|a| a.letras.borrow().indice_en(l))
                    {
                        // Un clic en lo escrito pone ahi la rayita; con
                        // Mayusculas, elige desde donde estaba; dos seguidos
                        // en la misma letra, la palabra entera. Y sin soltar
                        // se sigue eligiendo al arrastrar.
                        if let Some(a) = abierto.as_mut() {
                            let doble = a
                                .clic_en_caja
                                .is_some_and(|(cuando, j)| j == i && cuando.elapsed() <= DOBLE_CLIC);
                            if doble {
                                let r = redaccion::palabra_en(&a.borrador, i);
                                a.cursor.elegir(&a.borrador, r);
                                a.clic_en_caja = None;
                            } else {
                                let shift = pixpin_shell::entrada::modificadores_pulsados().shift;
                                a.cursor.poner(&a.borrador, i, shift);
                                a.clic_en_caja = Some((std::time::Instant::now(), i));
                                a.eligiendo_texto = true;
                                ventana.capturar_raton();
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
                        } else if let Err(e) =
                            pixpin_proyecto::ubicacion::preparar(ubicacion.raiz(), &fichas[i])
                        {
                            // Guardado en un disco que no esta: se avisa en
                            // vez de abrirlo vacio y escribir en otra parte.
                            aviso = Some((
                                donde_vive::texto_de_error(textos, &e),
                                std::time::Instant::now(),
                            ));
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
                    // Sacar la burbuja de la ventana la lleva a otro programa
                    // (el Explorador, un correo, el escritorio): el mismo
                    // `DoDragDrop` que los pines. Se distingue de comentar
                    // por DONDE esta el raton, no por cuanto se movio: dentro
                    // de la ventana el gesto sigue siendo comentar.
                    if let Some((indice, _, _)) = abierto.as_ref().and_then(|a| a.comentando)
                        && sale_de_la_ventana(l, marco.ancho, marco.alto)
                    {
                        let carga = abierto
                            .as_ref()
                            .and_then(|a| carga_fusionada(a, indice, textos));
                        if let Some(a) = abierto.as_mut() {
                            a.comentando = None;
                        }
                        // La captura se suelta ANTES: `DoDragDrop` lleva el
                        // raton el solo mientras dura el gesto.
                        ventana.soltar_raton();
                        if let Some(carga) = carga
                            && let Err(e) = pixpin_pin::arrastrar(carga)
                        {
                            tracing::warn!(?e, "no se pudo sacar la burbuja");
                            aviso = Some((textos.t("chat-no-se-pudo"), std::time::Instant::now()));
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
                    // Arrastrando con el boton pulsado en la caja de
                    // escribir: lo de en medio queda elegido, aunque el raton
                    // se salga de la caja.
                    if let Some(a) = abierto.as_mut().filter(|a| a.eligiendo_texto) {
                        let i = a.letras.borrow().indice_cerca(l);
                        if i != a.cursor.pos(&a.borrador) {
                            a.cursor.poner(&a.borrador, i, true);
                            hay_que_pintar = true;
                        }
                        ventana.poner_cursor(FormaCursorWin::Texto);
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
                                } else if abierto
                                    .as_ref()
                                    .is_some_and(|a| a.letras.borrow().indice_en(l).is_some())
                                {
                                    // Sobre lo escrito, la barra de texto:
                                    // ahi se puede pulsar para escribir.
                                    FormaCursorWin::Texto
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
                    // Clic derecho sobre el microfono: dictar. Lo que se dice
                    // va entrando en la caja de escribir mientras se habla,
                    // con el reconocedor de Windows y sin red. El izquierdo
                    // sigue siendo grabar una nota de voz, como en el movil.
                    let sobre_el_micro = abierto.as_ref().is_some_and(|a| {
                        a.grabando.is_none()
                            && disposicion
                                .boton_enviar(a.alto_caja.get(), escala)
                                .contiene(l)
                    });
                    // Clic derecho sobre una banderita de la letra: quitarla
                    // (mantener pulsado en el movil).
                    let banderita = abierto.as_ref().and_then(|a| marca_bajo(a, l));
                    if let Some((i, ms)) = banderita {
                        if let Some(a) = abierto.as_mut()
                            && let Efecto::Aviso(t) = quitar_la_marca(ubicacion, a, i, ms, textos)
                        {
                            aviso = Some((t, std::time::Instant::now()));
                        }
                        hay_que_pintar = true;
                    } else if sobre_el_micro {
                        if let Some(a) = abierto.as_mut()
                            && let Some(dicho) = alternar_dictado(a, textos, idioma)
                        {
                            aviso = Some((dicho, std::time::Instant::now()));
                        }
                        hay_que_pintar = true;
                    }
                    // Clic derecho sobre «+»: crearlo en una carpeta elegida
                    // («Guardar en…»). El clic izquierdo sigue creandolo en
                    // la zona habitual: si no se elige, va donde siempre.
                    else if disposicion.boton_nuevo(escala).contiene(l) {
                        let entradas = [
                            (1, textos.t("ubicacion-nuevo")),
                            (2, textos.t("ubicacion-nuevo-en")),
                            // H6: la papelera tambien aqui, porque con la
                            // lista vacia no hay proyecto sobre el que pulsar
                            // y «recuperar» es otra forma de tener uno.
                            (3, textos.t("proyecto-papelera")),
                        ];
                        let creado = match pixpin_shell::menu_llano(ventana.handle(), &entradas) {
                            Some(3) => {
                                crate::sincronizar::abrir_papelera(idioma, ubicacion.clone());
                                None
                            }
                            Some(1) => {
                                let cuando = pixpin_shell::entorno::ahora_utc_ms();
                                let f =
                                    pixpin_proyecto::almacen::Ficha::nueva("", cuando, &identidad);
                                let mut indice =
                                    pixpin_proyecto::almacen::Indice::leer(ubicacion.raiz());
                                indice.proyectos.push(f.clone());
                                indice.guardar(ubicacion.raiz()).ok().map(|_| (f, None))
                            }
                            Some(2) => donde_vive::nuevo_en_carpeta(
                                &ventana, textos, ubicacion, &identidad,
                            ),
                            _ => None,
                        };
                        if let Some((ficha, dicho)) = creado {
                            if let Some(a) = abierto.as_mut() {
                                cerrar_panel(ubicacion, a);
                                apagar_lienzo(ubicacion, a);
                            }
                            if let Some(a) = abierto.take() {
                                borradores.insert(a.ficha.id.clone(), a.borrador);
                            }
                            fichas.push(ficha.clone());
                            orden = filtrar(&fichas, &busqueda);
                            elegida = Some(fichas.len() - 1);
                            let mut nuevo = abrir_proyecto(ubicacion, &ficha);
                            nuevo.renombrando = true;
                            abierto = Some(nuevo);
                            scroll = 0;
                            if let Some(t) = dicho {
                                aviso = Some((t, std::time::Instant::now()));
                            }
                        }
                        buscando = false;
                        hay_que_pintar = true;
                    }
                    // Sobre la lista, el menu es el del proyecto: borrarlo, o
                    // borrar todos los marcados si este es uno de ellos.
                    else if let Some(fila) = disposicion.fila_en(l, scroll, orden.len(), escala) {
                        let id = fichas[orden[fila]].id.clone();
                        let ids: Vec<String> = if marcados.contains(&id) {
                            marcados.iter().cloned().collect()
                        } else {
                            vec![id]
                        };
                        let con_carpeta_propia = fichas[orden[fila]].ubicacion.is_some();
                        let con_logo = logo::tiene(ubicacion.raiz(), &fichas[orden[fila]].id);
                        let elegido = menu_de_proyecto(
                            &ventana,
                            textos,
                            ids.len(),
                            con_carpeta_propia,
                            con_logo,
                        );
                        // El logo: se pone o se quita y la lista lo vuelve a
                        // leer al pintar.
                        if matches!(
                            elegido,
                            Some(DelMenuProyecto::PonerLogo | DelMenuProyecto::QuitarLogo)
                        ) {
                            let id = &fichas[orden[fila]].id;
                            let hecho = if elegido == Some(DelMenuProyecto::PonerLogo) {
                                logo::elegir_y_poner(ventana.handle(), ubicacion.raiz(), id)
                                    .map(|_| ())
                            } else {
                                logo::quitar(ubicacion.raiz(), id).map_err(anyhow::Error::from)
                            };
                            if let Err(e) = hecho {
                                tracing::warn!(?e, "no se pudo cambiar el logo del proyecto");
                                aviso = Some((
                                    textos.t("proyecto-logo-error"),
                                    std::time::Instant::now(),
                                ));
                            }
                            hay_que_pintar = true;
                            continue;
                        }
                        // Donde se guarda: moverlo, traerlo o ensenar su
                        // carpeta. Solo con uno: cada proyecto va a la suya.
                        let cambio = match elegido {
                            Some(DelMenuProyecto::CambiarUbicacion) => {
                                Some(donde_vive::Cambio::AOtra)
                            }
                            Some(DelMenuProyecto::VolverAHabitual) => {
                                Some(donde_vive::Cambio::AHabitual)
                            }
                            _ => None,
                        };
                        if let Some(cambio) = cambio {
                            let id = fichas[orden[fila]].id.clone();
                            if let Some(t) = donde_vive::cambiar(
                                &ventana,
                                textos,
                                ubicacion,
                                &id,
                                cambio,
                                &mut abierto,
                                &mut borradores,
                            ) {
                                aviso = Some((t, std::time::Instant::now()));
                            }
                            // La lista se relee sola: el indice cambio.
                            hay_que_pintar = true;
                            continue;
                        }
                        if elegido == Some(DelMenuProyecto::AbrirCarpeta) {
                            if let Some(t) =
                                donde_vive::abrir_carpeta(textos, ubicacion, &fichas[orden[fila]])
                            {
                                aviso = Some((t, std::time::Instant::now()));
                            }
                            hay_que_pintar = true;
                            continue;
                        }
                        if elegido == Some(DelMenuProyecto::Renombrar) {
                            // Se abre y se le pone el nombre en modo
                            // escritura: es la misma caja que ya existia en
                            // la cabecera, y asi se ve lo que se escribe
                            // sobre el proyecto de verdad.
                            let cual = orden[fila];
                            if abierto
                                .as_ref()
                                .is_some_and(|a| a.ficha.id != fichas[cual].id)
                                && let Some(a) = abierto.as_mut()
                            {
                                cerrar_panel(ubicacion, a);
                                apagar_lienzo(ubicacion, a);
                            }
                            if abierto
                                .as_ref()
                                .is_some_and(|a| a.ficha.id != fichas[cual].id)
                                && let Some(a) = abierto.take()
                            {
                                borradores.insert(a.ficha.id.clone(), a.borrador);
                            }
                            elegida = Some(cual);
                            match abierto.as_mut() {
                                Some(a) => a.renombrando = true,
                                None => {
                                    let mut nuevo = abrir_proyecto(ubicacion, &fichas[cual]);
                                    nuevo.borrador =
                                        borradores.remove(&fichas[cual].id).unwrap_or_default();
                                    nuevo.renombrando = true;
                                    abierto = Some(nuevo);
                                }
                            }
                            marcados.clear();
                            hay_que_pintar = true;
                            continue;
                        }
                        // La hoja de compartir, con el proyecto (o los
                        // marcados) entero. Lo dibujado a medias en una
                        // burbuja se guarda antes, que si no se quedaria
                        // fuera (el `antes` de `CompartirPaginas.de`).
                        if elegido == Some(DelMenuProyecto::Compartir) {
                            if let Some(a) = abierto.as_mut().filter(|a| ids.contains(&a.ficha.id))
                            {
                                apagar_lienzo(ubicacion, a);
                            }
                            crate::compartir::ventana::abrir(
                                idioma,
                                ubicacion.clone(),
                                crate::compartir::Cosa::Proyectos {
                                    raiz: ubicacion.raiz().to_path_buf(),
                                    ids: ids.clone(),
                                },
                            );
                            continue;
                        }
                        // La papelera: lo borrado se recupera entero, en la
                        // pantalla de copias de Sincronizar.
                        if elegido == Some(DelMenuProyecto::Papelera) {
                            crate::sincronizar::abrir_papelera(idioma, ubicacion.clone());
                            continue;
                        }
                        if elegido == Some(DelMenuProyecto::Borrar) {
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
                                && a.letra.is_none()
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
                            let puesta = a.mensajes.get(i).and_then(|m| m.emoji.clone());
                            Some((menu_de_mensaje(a, i, textos), i, puesta))
                        });
                    if let Some((entradas, i, puesta)) = pulsado.filter(|(v, _, _)| !v.is_empty()) {
                        // v2: con la fila de etiquetas encima.
                        menu = Some(MenuAbierto::nuevo(l, entradas).con_etiquetas(i, puesta));
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::BotonSoltado(p) => {
                    // Elegir lo escrito arrastrando termina al soltar: lo
                    // elegido se queda elegido.
                    if let Some(a) = abierto.as_mut().filter(|a| a.eligiendo_texto) {
                        a.eligiendo_texto = false;
                        ventana.soltar_raton();
                        hay_que_pintar = true;
                        continue;
                    }
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
                                idioma,
                                &identidad,
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
                // «Quien llama» (B11, v0.98.6): mientras su barra esta
                // abierta, lo que se teclea es el nombre; Intro lo guarda y
                // vuelve a sacar las horas, Escape la deja.
                EventoOverlay::Caracter(c)
                    if pendientes.is_none()
                        && abierto.as_ref().is_some_and(|a| a.quien_llama.is_some()) =>
                {
                    if let Some((_, escrito)) =
                        abierto.as_mut().and_then(|a| a.quien_llama.as_mut())
                    {
                        quien_llama::teclear(escrito, c);
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk, .. }
                    if pendientes.is_none()
                        && menu.is_none()
                        && matches!(vk, VK_RETROCESO | VK_ESCAPE | VK_ENTRAR)
                        && abierto.as_ref().is_some_and(|a| a.quien_llama.is_some()) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        match vk {
                            VK_RETROCESO => {
                                if let Some((_, escrito)) = a.quien_llama.as_mut() {
                                    escrito.pop();
                                }
                            }
                            VK_ESCAPE => a.quien_llama = None,
                            _ => menu = quien_llama::al_pulsar_intro(a, ubicacion.raiz(), textos),
                        }
                    }
                    hay_que_pintar = true;
                }
                // «Elegir la hora…»: mientras la barra esta abierta, lo que
                // se teclea es la hora. Solo cifras y separadores, y cinco
                // como mucho (`18:30`): una letra ahi no significa nada.
                EventoOverlay::Caracter(c)
                    if pendientes.is_none()
                        && abierto.as_ref().is_some_and(|a| a.hora_a_mano.is_some()) =>
                {
                    if let Some((_, escrito)) =
                        abierto.as_mut().and_then(|a| a.hora_a_mano.as_mut())
                        && (c.is_ascii_digit() || c == ':' || c == '.')
                        && escrito.chars().count() < 5
                    {
                        escrito.push(c);
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk, .. }
                    if pendientes.is_none()
                        && menu.is_none()
                        && matches!(vk, VK_RETROCESO | VK_ESCAPE | VK_ENTRAR)
                        && abierto.as_ref().is_some_and(|a| a.hora_a_mano.is_some()) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        match vk {
                            VK_RETROCESO => {
                                if let Some((_, escrito)) = a.hora_a_mano.as_mut() {
                                    escrito.pop();
                                }
                            }
                            VK_ESCAPE => a.hora_a_mano = None,
                            _ => {
                                let ahora = pixpin_shell::entorno::ahora_local_ms();
                                let pedido = a.hora_a_mano.as_ref().and_then(|(id, escrito)| {
                                    let i = a.mensajes.iter().position(|m| m.id == *id)?;
                                    Some((i, crate::recordatorios::hora_escrita(escrito, ahora)))
                                });
                                match pedido {
                                    // Se queda abierta para corregirla: lo
                                    // tecleado sigue ahi.
                                    Some((_, None)) => {
                                        aviso = Some((
                                            textos.t("chat-recordar-hora-mal"),
                                            std::time::Instant::now(),
                                        ));
                                    }
                                    Some((i, Some(cuando))) => {
                                        a.hora_a_mano = None;
                                        let cx = Contexto {
                                            ubicacion,
                                            identidad: &identidad,
                                            textos,
                                            lienzo,
                                            ventana: &ventana,
                                            fichas: &fichas,
                                            idioma,
                                        };
                                        if let Efecto::Aviso(t) =
                                            ejecutar(Accion::RecordarEn(i, cuando), a, &cx)
                                        {
                                            aviso = Some((t, std::time::Instant::now()));
                                        }
                                        a.colocado.borrow_mut().ancho = 0;
                                    }
                                    // El mensaje ya no esta (lo borro una
                                    // sincronizacion): no hay a que ponerle hora.
                                    None => a.hora_a_mano = None,
                                }
                            }
                        }
                    }
                    hay_que_pintar = true;
                }
                // La letra: la rueda la recorre, Escape vuelve a la
                // conversacion y Ctrl+C se lleva el texto entero.
                EventoOverlay::Rueda(delta)
                    if abierto.as_ref().is_some_and(|a| a.letra.is_some()) =>
                {
                    if let Some(a) = abierto.as_mut()
                        && let caja = caja_de_la_letra(a, &disposicion, escala).texto
                        && let Some(l) = a.letra.as_mut()
                    {
                        let tope =
                            pixpin_ui::chat::tope_de_la_letra(caja, &l.altos.borrow(), escala);
                        let paso = (chat::FILA * 3 / 2 * escala / 100) as i32;
                        l.scroll = (l.scroll - giro_de_rueda(delta, paso)).clamp(0, tope);
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk, .. }
                    if vk == VK_ESCAPE
                        && menu.is_none()
                        && abierto.as_ref().is_some_and(|a| a.letra.is_some()) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        a.letra = None;
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk, ctrl, .. }
                    if vk == VK_C
                        && ctrl
                        && abierto.as_ref().is_some_and(|a| a.letra.is_some()) =>
                {
                    if let Some(a) = abierto.as_ref()
                        && let Efecto::Aviso(t) = copiar_la_letra(a, textos)
                    {
                        aviso = Some((t, std::time::Instant::now()));
                    }
                    hay_que_pintar = true;
                }
                // Lo que se teclea con la letra delante no puede ir a parar
                // a la caja de escribir, que esta escondida detras.
                EventoOverlay::Caracter(_)
                    if abierto.as_ref().is_some_and(|a| a.letra.is_some()) => {}
                EventoOverlay::Rueda(delta)
                    if abierto.as_ref().is_some_and(|a| a.biblioteca.is_some()) =>
                {
                    if let Some(b) = abierto.as_mut().and_then(|a| a.biblioteca.as_mut()) {
                        let t = pixpin_ui::mini::Disposicion::calcular(
                            hueco_hoja(&disposicion),
                            escala,
                            reparto_de_biblioteca(),
                        );
                        let paso = 2 * (pixpin_ui::mini::FILA_ALTO * escala / 100) as i32;
                        b.scroll = (b.scroll - giro_de_rueda(delta, paso))
                            .clamp(0, t.tope_scroll(b.filas.len(), escala));
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk, .. }
                    if vk == VK_ESCAPE
                        && abierto.as_ref().is_some_and(|a| a.biblioteca.is_some()) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        a.biblioteca = None;
                    }
                    hay_que_pintar = true;
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
                            let paso = 2 * (pixpin_ui::mini::FILA_ALTO * escala / 100) as i32;
                            m.scroll = (m.scroll - giro_de_rueda(delta, paso))
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
                        let paso = 2 * (pixpin_ui::tabla::FILA_ALTO * escala / 100) as i32;
                        let (columnas, filas) = h.tabla.tamano();
                        // Una de mas por cada lado: si la hoja acabara justo en
                        // la ultima celda escrita, no habria donde anadir.
                        let (x, y) = t.sujetar(
                            h.scroll_x,
                            h.scroll_y - giro_de_rueda(delta, paso),
                            columnas + 1,
                            filas + 1,
                            escala,
                        );
                        h.scroll_x = x;
                        h.scroll_y = y;
                    }
                    hay_que_pintar = true;
                }
                // La galeria y las demas pestanas del panel. No lo tenian: la
                // rueda no hacia nada ahi, asi que solo se veia la primera
                // pantalla de cosas y parecia que faltaban («en la galeria no
                // puedo hacer scroll, no muestra todos los archivos»).
                EventoOverlay::Rueda(delta)
                    if abierto.as_ref().is_some_and(|a| a.info.is_some()) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        let d = disposicion_info(&disposicion, escala);
                        let paso = (chat::FILA * 3 / 2 * escala / 100) as i32;
                        // `alto_info` lo deja puesto el pintado de la vuelta
                        // anterior: es lo que mide todo el contenido, y con
                        // el se sabe hasta donde se puede bajar.
                        let tope = (a.alto_info.get() as i32 - d.contenido.alto as i32).max(0);
                        a.scroll_info = (a.scroll_info - giro_de_rueda(delta, paso)).clamp(0, tope);
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
                        let paso = (chat::FILA * 3 / 2 * escala / 100) as i32;
                        let ahora_en = a.scroll.unwrap_or(tope);
                        let nuevo = pixpin_ui::historial::scroll_ajustado(
                            area,
                            a.alto.get(),
                            ahora_en - giro_de_rueda(delta, paso),
                        );
                        // Volver al final se guarda como «pegado»: si llegan
                        // mensajes nuevos, se siguen viendo sin tocar nada.
                        a.scroll = if nuevo >= tope { None } else { Some(nuevo) };
                        hay_que_pintar = true;
                        continue;
                    }
                    // Tres filas por muesca, como Telegram y como el ajuste
                    // de Windows por omision.
                    let paso = (chat::FILA * 3 / 2 * escala / 100) as i32;
                    let nuevo = disposicion.scroll_ajustado(
                        scroll - giro_de_rueda(delta, paso),
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
                // La escala de la interfaz, como en Telegram. Va de las
                // primeras y sin mirar que hay abierto: quien no ve bien lo
                // que tiene delante tiene que poder agrandarlo ahi mismo, no
                // despues de cerrar el lienzo o la hoja.
                EventoOverlay::Tecla { vk, ctrl, .. }
                    if ctrl
                        && matches!(
                            vk,
                            VK_MAS | VK_MENOS | VK_MAS_NUM | VK_MENOS_NUM | VK_CERO | VK_CERO_NUM
                        ) =>
                {
                    let pedido = match vk {
                        VK_MAS | VK_MAS_NUM => chat::escala_siguiente(factor_escala),
                        VK_MENOS | VK_MENOS_NUM => chat::escala_anterior(factor_escala),
                        _ => chat::ESCALA_POR_DEFECTO,
                    };
                    if pedido != factor_escala {
                        factor_escala = pedido;
                        escala = escala_monitor * factor_escala / 100;
                        recordar_escala(ubicacion, &mut estado, factor_escala);
                        // Lo medido con la escala vieja ya no vale: las
                        // burbujas se vuelven a colocar al pintar.
                        if let Some(a) = abierto.as_mut() {
                            a.colocado.borrow_mut().ancho = 0;
                        }
                        hay_que_pintar = true;
                    }
                    // Siempre se dice en que quedo, aunque no haya cambiado:
                    // sin aviso, pulsar en el tope parece que no funciona.
                    aviso = Some((
                        aviso_de_escala(textos, factor_escala),
                        std::time::Instant::now(),
                    ));
                }
                // Ctrl+F abre el buscador: el de la conversacion si hay una
                // abierta y, si no, el de la lista de proyectos. Es lo que en
                // el movil hace mantener pulsado el titulo (A9), que aqui
                // tambien esta en el boton derecho sobre el. Con la hoja o una
                // mini-app delante no: el teclado es suyo.
                EventoOverlay::Tecla { vk, ctrl, .. }
                    if vk == 0x46
                        && ctrl
                        && pendientes.is_none()
                        && abierto
                            .as_ref()
                            .is_none_or(|a| a.mini.is_none() && a.hoja.is_none()) =>
                {
                    match abierto.as_mut() {
                        Some(a) => {
                            a.busqueda.get_or_insert_with(String::new);
                            a.info = None;
                            a.scroll = None;
                        }
                        None => buscando = true,
                    }
                    hay_que_pintar = true;
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
                                // La misma barra escribe el nombre o la
                                // descripcion de una foto (`describiendo`).
                                if let Some((i, escrito)) = a.renombrando_mensaje.take() {
                                    if std::mem::take(&mut a.describiendo) {
                                        guardar_descripcion(ubicacion, a, i, &escrito);
                                    } else {
                                        guardar_nombre_de_mensaje(ubicacion, a, i, escrito.trim());
                                    }
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
                    if let Some(p) = pendientes.take() {
                        p.cancelar(abierto.as_mut());
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk, .. } if pendientes.is_some() && vk == VK_ENTRAR => {
                    if let (Some(p), Some(a)) = (pendientes.take(), abierto.as_mut()) {
                        let hechos = meter_ficheros(ubicacion, a, &identidad, &p.rutas, &p.pie);
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
                    // Las teclas de letra y los atajos de una tecla (Espacio,
                    // `+`, `-`, `v`) llegan por aqui; `mini_panel::caracter`
                    // decide si escriben en la caja o hacen algo.
                    if let Some(a) = abierto.as_mut()
                        && let Some(m) = a.mini.as_mut()
                    {
                        let e =
                            crate::mini_panel::caracter(&m.cual, &m.documento, &mut m.teclado, c);
                        if cumplir_mini(ubicacion, a, e, &disposicion, escala, textos) {
                            hay_que_pintar = true;
                        }
                    }
                }
                EventoOverlay::Tecla {
                    vk,
                    shift,
                    ctrl,
                    alt,
                    ..
                } if abierto.as_ref().is_some_and(|a| a.mini.is_some()) => {
                    // Flechas, F2, Supr, Intro y Escape. Escape deshace de
                    // dentro afuera (lo escrito, la fila marcada) y solo al
                    // final cierra: arrepentirse nunca cuesta el panel.
                    if let Some(a) = abierto.as_mut()
                        && let Some(m) = a.mini.as_mut()
                    {
                        let e = crate::mini_panel::tecla(
                            &m.cual,
                            &m.documento,
                            &moneda_del_catalogo(textos),
                            &mut m.teclado,
                            crate::mini_panel::Tecla {
                                vk,
                                shift,
                                ctrl,
                                alt,
                            },
                        );
                        if cumplir_mini(ubicacion, a, e, &disposicion, escala, textos) {
                            hay_que_pintar = true;
                        }
                    }
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
                EventoOverlay::Tecla {
                    vk, shift, ctrl, ..
                } if abierto.as_ref().is_some_and(|a| a.hoja.is_some()) => {
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
                            // Pegar desde Excel o Sheets y copiar hacia ellos
                            // (J2, `hoja_portapapeles.rs`).
                            0x56 if ctrl => hoja_portapapeles::pegar(h),
                            0x43 if ctrl => hoja_portapapeles::copiar(h),
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
                // A pantalla completa, Escape devuelve la ventana a su sitio.
                // Va DESPUES del panel y de la lupa a proposito: con algo
                // abierto encima, Escape cierra eso primero, que es lo que
                // se espera de el en cualquier programa.
                EventoOverlay::Tecla { vk, .. }
                    if vk == VK_ESCAPE && antes_de_maximizar.is_some() =>
                {
                    nuevo_marco = antes_de_maximizar.take();
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
                            let mut b = [0u8; 4];
                            a.cursor.escribir(&mut a.borrador, c.encode_utf8(&mut b));
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
                        // Uno que ya preguntaba devuelve su pie antes de
                        // que el nuevo se lleve lo de la caja.
                        if let Some(p) = pendientes.take() {
                            p.cancelar(abierto.as_mut());
                        }
                        pendientes = Pendientes::con_lo_escrito(rutas, abierto.as_mut());
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
                EventoOverlay::Tecla { vk, ctrl, .. } if vk == VK_RETROCESO => {
                    if let Some(a) = abierto.as_mut().filter(|a| !a.borrador.is_empty()) {
                        a.cursor.borrar_atras(&mut a.borrador, ctrl);
                        a.colocado.borrow_mut().ancho = 0;
                        hay_que_pintar = true;
                    }
                }
                // La rayita de la caja de escribir: flechas, Inicio y Fin
                // (con Mayusculas seleccionan, con Ctrl van por palabras),
                // Suprimir, y Ctrl+A, Ctrl+C y Ctrl+X sobre lo escrito.
                EventoOverlay::Tecla { vk, shift, ctrl, .. }
                    if abierto.as_ref().is_some_and(|a| !a.borrador.is_empty())
                        && (matches!(
                            vk,
                            VK_IZQUIERDA
                                | VK_DERECHA
                                | VK_ARRIBA
                                | VK_ABAJO
                                | VK_INICIO
                                | VK_FIN
                                | VK_SUPR
                        ) || (ctrl && vk == VK_A)
                            || (ctrl
                                && (vk == VK_C || vk == VK_X)
                                && abierto
                                    .as_ref()
                                    .is_some_and(|a| a.cursor.seleccion(&a.borrador).is_some()))) =>
                {
                    if let Some(a) = abierto.as_mut() {
                        let t = &mut a.borrador;
                        let c = &mut a.cursor;
                        match vk {
                            VK_IZQUIERDA => c.izquierda(t, shift, ctrl),
                            VK_DERECHA => c.derecha(t, shift, ctrl),
                            VK_INICIO => c.inicio(t, shift, ctrl),
                            VK_FIN => c.fin(t, shift, ctrl),
                            VK_SUPR => c.borrar_delante(t, ctrl),
                            VK_A => c.todo(t),
                            VK_ARRIBA | VK_ABAJO => {
                                // Un renglon arriba o abajo a la misma
                                // altura: se pregunta a las cajas pintadas
                                // que letra hay ahi, como si se pulsara.
                                let l = a.letras.borrow();
                                if l.texto == *t {
                                    let (x, y, alto) = redaccion::sitio_del_cursor(
                                        &l.texto,
                                        &l.cajas,
                                        c.pos(t),
                                        0.0,
                                    );
                                    let destino = if vk == VK_ARRIBA {
                                        y - alto / 2.0
                                    } else {
                                        y + alto * 1.5
                                    };
                                    // Por encima del primer renglon o debajo
                                    // del ultimo, al principio o al final.
                                    let fondo = l
                                        .cajas
                                        .iter()
                                        .map(|(_, b)| b.y + b.alto)
                                        .fold(0.0, f32::max);
                                    let i = if destino < 0.0 {
                                        0
                                    } else if destino >= fondo && !l.texto.ends_with('\n') {
                                        l.texto.len()
                                    } else {
                                        redaccion::indice_en_punto(&l.texto, &l.cajas, x, destino)
                                    };
                                    c.poner(t, i, shift);
                                }
                            }
                            _ => {
                                // Ctrl+C o Ctrl+X con algo elegido.
                                if let Some(s) = c.seleccionado(t) {
                                    if let Err(e) = pixpin_codec::portapapeles::copiar_texto(s) {
                                        tracing::warn!(?e, "no se pudo copiar lo escrito");
                                    } else if vk == VK_X {
                                        c.borrar_atras(t, false);
                                    }
                                }
                            }
                        }
                        a.colocado.borrow_mut().ancho = 0;
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk, shift, .. } if vk == VK_ENTRAR => {
                    if let Some(a) = abierto.as_mut() {
                        if shift {
                            // Mayusculas y entrar: un renglon mas, como en
                            // Telegram. Entrar solo, se envia.
                            a.cursor.escribir(&mut a.borrador, "\n");
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
                        // Con algo seleccionado, Escape solo suelta la
                        // seleccion, como en cualquier caja de texto.
                        if a.cursor.seleccion(&a.borrador).is_some() {
                            let pos = a.cursor.pos(&a.borrador);
                            a.cursor.poner(&a.borrador, pos, false);
                        } else {
                            a.borrador.clear();
                            a.cursor.soltar();
                        }
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
            // Se viene a ver ese chat: la pantalla de Proyectos se aparta.
            vista_proyectos.ver_chat();
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

        // Lo que llega de los hilos de los PDF: paginas ya pintadas para sus
        // vistas y «Anadir al proyecto» que terminaron (o como van).
        let (repintar, dicho) = recoger_lo_de_los_pdf(ubicacion, abierto.as_mut(), textos);
        if let Some(t) = dicho {
            aviso = Some((t, std::time::Instant::now()));
        }
        hay_que_pintar |= repintar;
        // La pantalla de Proyectos: lo leido en su hilo, lo que falta pedir y
        // un nombre recien escrito. Sin ella delante no pide nada.
        // Con el proyecto que haya quedado abierto tras los eventos.
        vista_proyectos.seguir(abierto.as_ref().map(|a| a.ficha.id.as_str()));
        let (repintar, pedido) = vista_proyectos.latido(ubicacion, &fichas);
        hay_que_pintar |= repintar;
        if let Some(pedido) = pedido {
            hay_que_pintar |=
                proyectos::cumplir(pedido, &mut vista_proyectos, &mut bucle_de_proyectos!());
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
                // Por `disponer` y no calculando aparte: con la ventana
                // estrecha, lo que se ve depende de si hay proyecto abierto,
                // y una cuenta suelta daria un tope de otra lista.
                let d = disponer(marco, escala, ancho_lista, abierto.as_ref());
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
            let mut faltan_fondos = false;
            if let Some(a) = abierto.as_ref() {
                if let Some(seccion) = a.info {
                    let d = disposicion_info(&disposicion, escala);
                    let rutas = fotos_a_la_vista(a, seccion, &d, escala);
                    if !rutas.is_empty() {
                        miniaturas.asegurar(&rutas, &motor);
                    }
                    // Las paginas de PDF de la galeria se pintan de las
                    // vistas previas; las que no quepan en este fotograma,
                    // en el siguiente.
                    let fondos = fondos_a_la_vista(a, seccion, &d, escala);
                    faltan_fondos = !fondos.is_empty() && previas.asegurar(&fondos, &motor);
                }
            }
            // Y las del cuadro de confirmar, que son pocas y se ven todas.
            if let Some(p) = pendientes.as_ref() {
                let rutas: Vec<std::path::PathBuf> = p.rutas.iter().take(48).cloned().collect();
                miniaturas.asegurar(&rutas, &motor);
            }
            // Y las imagenes de las tareas del panel de la mini-app.
            if let Some(m) = abierto.as_ref().and_then(|a| a.mini.as_ref()) {
                let rutas = m.imagenes_a_cargar();
                if !rutas.is_empty() {
                    faltan_fondos |= previas.asegurar(&rutas, &motor);
                }
            }
            // Y las de la pantalla de Proyectos: sus miniaturas y la portada.
            if vista_proyectos.activa() {
                faltan_fondos |= vista_proyectos.preparar(&mut previas, &motor);
            }
            // Los logos de las filas que se ven y el del abierto (su
            // cabecera): leidos una vez, luego solo se pintan.
            {
                let (primera, cuantas) = disposicion.visibles(scroll, orden.len(), escala);
                let filas = orden
                    .iter()
                    .skip(primera)
                    .take(cuantas)
                    .filter_map(|i| fichas.get(*i))
                    .map(|f| f.id.as_str());
                let del_abierto = abierto.as_ref().map(|a| a.ficha.id.as_str());
                logo::preparar(ubicacion.raiz(), filas.chain(del_abierto), &motor);
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
                // Con Proyectos delante, el chat no se pinta: no se ve.
                let abierto_ref = abierto.as_ref().filter(|_| !vista_proyectos.activa());
                let pendientes_ref = pendientes.as_ref();
                let menu_ref = menu.as_ref();
                let aviso_ref = aviso.as_ref().map(|(t, _)| t.as_str());
                let papel = papel_de(fondo, claro);
                // M1: el cielo del tema Cosmos se hornea fuera del fotograma.
                let (cw, ch) = crate::tema_cosmos::tamano_de(&disposicion);
                crate::tema_cosmos::preparar(&motor, cw, ch);
                let resultado = motor.dibujar(&destino, |p: &Pintor| {
                    pintar(p, &disposicion, tema, papel, escala, textos, sobre, &lista);
                    // En modo Proyectos, la tarjeta del proyecto elegido en el
                    // panel derecho, donde iria su chat. Sin ninguno elegido
                    // queda el «Elige un proyecto» del chat.
                    if vista_proyectos.activa()
                        && let Some(a) = abierto.as_ref()
                        && let Some(f) = fichas.iter().find(|f| f.id == a.ficha.id)
                    {
                        let c = Pinta {
                            tema,
                            escala,
                            textos,
                            ahora,
                            miniaturas: &miniaturas,
                            previas: &previas,
                        };
                        // El nombre, el que se esta escribiendo si acaba de
                        // nacer con el «+» de la lista.
                        proyectos::pintar(
                            p,
                            &vista_proyectos,
                            &disposicion,
                            &c,
                            f,
                            &a.ficha.nombre,
                            a.renombrando,
                            claro,
                            ubicacion.raiz(),
                        );
                    }
                    proyectos::pintar_interruptor(
                        p,
                        &vista_proyectos,
                        &disposicion,
                        tema,
                        escala,
                        textos,
                    );
                    let mut alto_caja = 0;
                    if let Some(a) = abierto_ref {
                        // Lo que se puede pulsar se apunta de nuevo en cada
                        // fotograma: lo que ya no se ve no se puede pulsar.
                        a.zonas.borrow_mut().clear();
                        // Y la caja de escribir igual: si este fotograma no
                        // la pinta (grabando, una pantalla encima), sus
                        // letras no se pueden pulsar.
                        a.letras.borrow_mut().zona = None;
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
                            // La letra de una nota de voz, igual que la
                            // biblioteca: la cabecera se ve pero sus
                            // pastillas no se apuntan (pulsarla vuelve), y
                            // las zonas de la letra van despues.
                            None if a.letra.is_some() => {
                                pintar_cabecera(p, &disposicion, &c, a);
                                a.zonas.borrow_mut().clear();
                                pintar_letra(p, &disposicion, &c, a);
                            }
                            // Con una hoja abierta, la conversacion deja su sitio:
                            // una tabla necesita todo el ancho y el alto que
                            // haya, y el historial vuelve al cerrarla.
                            None if a.biblioteca.is_some() => {
                                if let Some(b) = a.biblioteca.as_ref() {
                                    pintar_biblioteca(p, &disposicion, &c, b);
                                }
                                pintar_cabecera(p, &disposicion, &c, a);
                                a.zonas.borrow_mut().clear();
                            }
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
                            envio::pintar(p, &c, pend, marco, &a.ficha.nombre);
                        }
                        // La hora a mano, en su ventanita encima de todo: en la
                        // barra de abajo la tapaban otras cosas (8-oct-2026).
                        if a.hora_a_mano.is_some() {
                            pintar_hora_a_mano(p, &c, a, marco);
                        }
                    }
                    // El menu, el ultimo: se despliega encima de todo.
                    if let Some(m) = menu_ref {
                        let c = Pinta {
                            tema,
                            escala,
                            textos,
                            ahora,
                            miniaturas: &miniaturas,
                            previas: &previas,
                        };
                        pintar_menu_abierto(p, &c, m, marco);
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
                    CACHE_GRAFITO.with_borrow_mut(|c| c.vaciar());
                    logo::soltar();
                    crate::tema_cosmos::soltar();
                    vista_proyectos.soltar();
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
            hay_que_pintar |= faltan_fondos;
            // Lo que la pantalla de Proyectos pinto sin imagen todavia: otra vuelta.
            hay_que_pintar |= vista_proyectos.falta_algo(&previas);
            // Un destino que esperaba a que se midieran las burbujas: ya
            // estan, otra vuelta para llevar la vista alli.
            // Solo con el historial a la vista: con la hoja o el panel
            // delante no se mide nada, y esto daria vueltas sin parar.
            if abierto.as_ref().is_some_and(|a| {
                a.ir_a.is_some()
                    && a.hoja.is_none()
                    && a.mini.is_none()
                    && a.info.is_none()
                    && a.letra.is_none()
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
        // El latido del reproductor: recoge la duracion en cuanto Media
        // Foundation la sabe, mueve la posicion y rebobina al acabar. Va aqui
        // y no en un hilo porque el motor es del hilo que lo creo, que es
        // este. Cada `MS_ENTRE_LATIDOS` (250 ms), como en el movil.
        let hay_audio = crate::audio::hay_algo();
        if hay_audio && crate::audio::latido() {
            hay_que_pintar = true;
        }
        // Despues del latido, que es quien averigua la duracion: el salto a
        // un minuto que esperaba por ella, y la letra siguiendo al audio.
        if hay_audio
            && let Some(a) = abierto.as_mut()
            && latido_de_la_letra(a, escala, &disposicion)
        {
            hay_que_pintar = true;
        }
        let hasta_el_reproductor = hay_audio.then_some(pixpin_audio::MS_ENTRE_LATIDOS as u32);
        // Grabando se repinta mas a menudo: el nivel del microfono tiene que
        // moverse con la voz o no dice si esta cogiendo algo.
        let hasta_el_nivel = abierto
            .as_ref()
            .and_then(|a| a.grabando.as_ref())
            .map(|_| 100u32);
        // La transcripcion, igual: el hilo deja el texto cuando le toca y
        // nadie va a mover el raton para enterarse. Se mira aqui —una vez
        // por vuelta— y no al pintar, porque escribe en el cuaderno.
        if let Some(a) = abierto.as_mut() {
            let (cambio, fallo) = latido_del_dictado(a, textos);
            if cambio {
                hay_que_pintar = true;
            }
            if let Some(dicho) = fallo {
                aviso = Some((dicho, std::time::Instant::now()));
            }
            let dicho = latido_de_transcribir(ubicacion, a, textos)
                // Y la lectura del telepronter, que llega por otro canal
                // pero acaba igual: escrita en el cuaderno.
                .or_else(|| latido_del_telepronter(ubicacion, a, &identidad, textos))
                // Y las ventanas de la voz (letra, Pronunciar, conversacion).
                .or_else(|| latido_de_la_voz(ubicacion, a, &identidad, textos, idioma));
            if let Some(dicho) = dicho {
                aviso = Some((dicho, std::time::Instant::now()));
                hay_que_pintar = true;
            }
            // El libro de Excel que se lee en su hilo (D11, `libro.rs`):
            // al llegar se abre su tabla, y hay que pintarla.
            if let Some(llegado) = libro::latido(ubicacion, a, textos) {
                hay_que_pintar = true;
                if let Some(dicho) = llegado {
                    aviso = Some((dicho, std::time::Instant::now()));
                }
            }
        }
        // Cuatro repintados por segundo bastan para una barra que tarda
        // diez: mas es gastar procesador que le hace falta al reconocedor.
        let hasta_la_transcripcion = abierto
            .as_ref()
            .and_then(|a| a.transcribiendo.as_ref())
            .map(|_| 250u32);
        // Dictando, lo provisional tiene que ir apareciendo mientras se
        // habla: una decima de segundo es lo que tarda SAPI en dar la
        // siguiente hipotesis.
        let hasta_el_dictado = abierto
            .as_ref()
            .and_then(|a| a.dictado.as_ref())
            .map(|_| 100u32);
        // Y con el telepronter abierto hay que volver a mirar su canal: la
        // lectura llega de otro hilo y aqui no entra ningun evento por ella.
        let hasta_la_lectura = abierto
            .as_ref()
            .filter(|a| a.leyendo.is_some() || a.voz.espera())
            .map(|_| 400u32);
        // Un libro de Excel leyendose: se mira cada 100 ms si ya llego.
        let hasta_el_libro = libro::leyendo().then_some(100u32);
        let dormir = [
            hasta_el_aviso,
            hasta_el_resalte,
            hasta_el_latido,
            hasta_el_reproductor,
            hasta_el_nivel,
            hasta_la_transcripcion,
            hasta_el_dictado,
            hasta_la_lectura,
            hasta_el_libro,
        ]
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
    /// v2: la foto que se arrastra para cambiarla de sitio.
    arrastre: Option<envio::Arrastre>,
    /// v2: lo que hay bajo el raton (el ✕, Añadir, los botones).
    sobre: Option<envio::Sitio>,
    /// Lo que miden Cancelar y Enviar, medido al pintar.
    botones: std::cell::Cell<(u32, u32)>,
}

impl Pendientes {
    /// El cuadro ya colocado, igual que al pintarlo.
    fn cuadro(&self, marco: Rect, escala: u32) -> envio::Cuadro {
        envio::colocar(
            Rect {
                x: 0,
                y: 0,
                ancho: marco.ancho,
                alto: marco.alto,
            },
            self.rutas.len(),
            self.botones.get(),
            escala,
        )
    }

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
            arrastre: None,
            sobre: None,
            botones: std::cell::Cell::new((120, 130)),
        })
    }

    /// Como [`Pendientes::de`], y **lo escrito en la caja pasa a ser el
    /// pie**, como en Telegram: antes el cuadro salia con el pie vacio, la
    /// foto se iba sola y lo escrito se quedaba atras, que el usuario leia
    /// como texto perdido. Si no hay cuadro, la caja no se toca; al cancelar
    /// vuelve a ella (`descripcion::pie_de_vuelta`).
    fn con_lo_escrito(
        rutas: Vec<std::path::PathBuf>,
        a: Option<&mut Abierto>,
    ) -> Option<Pendientes> {
        let mut p = Pendientes::de(rutas)?;
        if let Some(a) = a {
            p.pie = std::mem::take(&mut a.borrador).trim().to_string();
            a.colocado.borrow_mut().ancho = 0;
        }
        Some(p)
    }

    /// Cancelar el cuadro: el pie vuelve a la caja de escribir.
    fn cancelar(self, a: Option<&mut Abierto>) {
        if let Some(a) = a {
            descripcion::pie_de_vuelta(&mut a.borrador, self.pie);
            a.colocado.borrow_mut().ancho = 0;
        }
    }
}

/// Lo que el pintado de la caja de escribir deja apuntado para el raton: la
/// caja de cada letra (de la MISMA disposicion con la que se pinta) y donde
/// quedo el texto en la ventana.
#[derive(Default)]
struct LetrasPintadas {
    /// El texto, la letra y el ancho con que se pidieron las cajas: si no
    /// cambia nada, no se vuelven a pedir en cada fotograma.
    texto: String,
    tam: f32,
    ancho: f32,
    cajas: Vec<(usize, RectF)>,
    /// La esquina del texto en la ventana.
    origen: (f32, f32),
    /// Donde se puede pulsar para poner la rayita: el campo de escribir.
    /// Vacio mientras se dicta (lo que se ve no es aun lo escrito).
    zona: Option<RectF>,
}

impl LetrasPintadas {
    /// La letra bajo `l` (coordenadas de la ventana), si cae en la caja.
    fn indice_en(&self, l: Punto) -> Option<usize> {
        redaccion::indice_en(
            &self.texto,
            &self.cajas,
            self.origen,
            self.zona?,
            l.x as f32,
            l.y as f32,
        )
    }

    /// Como `indice_en`, pero fuera de la caja tambien: arrastrando para
    /// seleccionar, salirse por arriba o por un lado sigue eligiendo.
    fn indice_cerca(&self, l: Punto) -> usize {
        redaccion::indice_en_punto(
            &self.texto,
            &self.cajas,
            l.x as f32 - self.origen.0,
            l.y as f32 - self.origen.1,
        )
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
    /// La rayita y lo seleccionado dentro de `borrador`.
    cursor: redaccion::Cursor,
    /// La caja de cada letra de lo escrito tal como se pinto, para saber que
    /// letra hay bajo el raton. La apunta el pintado (`pintar_redaccion`).
    letras: std::cell::RefCell<LetrasPintadas>,
    /// Se esta seleccionando lo escrito con el boton pulsado.
    eligiendo_texto: bool,
    /// El ultimo clic en la caja de escribir y donde cayo: el segundo, si
    /// llega pronto y en la misma letra, elige la palabra.
    clic_en_caja: Option<(std::time::Instant, usize)>,
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
    /// Los codigos unicos de las hojas del `proyecto.json`. Es lo que la
    /// galeria ensena, ni mas ni menos: las mismas que lista la pantalla de
    /// proyectos del movil.
    hojas_uid: std::collections::BTreeSet<String>,
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
    /// Que esa misma barra escribe la descripcion de la foto y no su nombre
    /// («Añadir descripción», `descripcion`).
    describiendo: bool,
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
    /// La biblioteca de audio, si esta abierta. Ocupa el mismo sitio que las
    /// otras dos, y por eso las tres se cierran por `cerrar_panel`.
    biblioteca: Option<BibliotecaAbierta>,
    /// La nota de voz que se esta grabando, si hay alguna.
    grabando: Option<Grabando>,
    /// La nota de voz que se esta pasando a texto, si hay alguna.
    ///
    /// **Una sola a la vez**, y a proposito: cada transcripcion carga un
    /// modelo de Vosk de 40 MB y pone un hilo a moler: dos a la par tardan
    /// mas que las dos seguidas y nadie mira dos barras de avance. Va en
    /// `Abierto` y no en el bucle porque cambiar de conversacion tiene que
    /// cortarla (`EnMarcha` cancela al tirarse).
    transcribiendo: Option<crate::voz::EnMarcha>,
    /// El dictado en curso (clic derecho en el microfono), si hay uno.
    dictado: Option<Dictando>,
    /// El telepronter abierto, si hay uno: por aqui llega la lectura cuando
    /// el usuario termina de leer. Se guarda el extremo del canal y no un
    /// manejador de la ventana porque **la ventana vive en su propio hilo**
    /// y el chat no manda sobre ella: solo espera lo que salga.
    leyendo: Option<std::sync::mpsc::Receiver<crate::teleprompter::Lectura>>,
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
    /// Las notas de voz con el texto desplegado, por id. Es el `desplegado`
    /// que el movil guarda en cada burbuja (`remember`): no se escribe en el
    /// cuaderno, porque es como se esta mirando ahora y no un dato de la
    /// nota, y se olvida al cambiar de conversacion.
    desplegados: std::collections::BTreeSet<String>,
    /// La pantalla de la letra, si esta abierta (`LetraActivity`).
    letra: Option<LetraAbierta>,
    /// Lo que la voz tiene a medias en otras ventanas: la caja de la
    /// letra, Pronunciar y la conversacion por turnos.
    voz: crate::voz_en_chat::Pendiente,
    /// Un salto al audio que espera a que Media Foundation sepa cuanto dura:
    /// la ruta y el milisegundo. Pulsar un minuto de una nota que no estaba
    /// cargada la carga, y hasta el primer latido no se puede ir a ningun
    /// sitio (`reloj::destino_de_fraccion` sin duracion no hace nada).
    salto_pendiente: Option<(std::path::PathBuf, i64)>,
    /// «Elegir la hora…»: el mensaje (por id) y lo tecleado hasta ahora. Se
    /// teclea en una barra encima de la caja de escribir; Intro la pone y
    /// Escape la deja.
    hora_a_mano: Option<(String, String)>,
    /// «Quien llama» (B11, v0.98.6): la nota de voz (por id) y el nombre
    /// tecleado. Intro lo guarda y vuelve a sacar las horas.
    quien_llama: Option<(String, String)>,
}

/// La pantalla de la letra abierta: de que nota es y como se esta leyendo.
struct LetraAbierta {
    /// El mensaje, por id: una sincronizacion puede meter mensajes en medio
    /// mientras se lee, y la posicion de antes apuntaria a otra nota.
    id: String,
    /// El tamano de la letra, en puntos logicos (de 14 a 40).
    tam: u32,
    scroll: i32,
    /// Lo que mide cada parrafo con la fuente, apuntado al pintar. La rueda
    /// y «seguir al audio» lo necesitan fuera del fotograma.
    altos: std::cell::RefCell<Vec<u32>>,
    /// El ultimo parrafo al que se llevo la vista siguiendo al audio. Solo
    /// se mueve la vista cuando CAMBIA: si no, el usuario no podria subir
    /// con la rueda mientras suena.
    seguido: Option<usize>,
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
    /// Lo que vale cada celda, calculado de una vez y con memoria
    /// (`formula::evaluar_todo`) al pintar, y olvidado al cambiar una celda.
    /// Celda a celda, una columna de saldos de un Excel importado daba
    /// `#¡CICLO!` pasada la fila 128 y se recalculaba en cada fotograma.
    valores:
        std::cell::OnceCell<std::collections::BTreeMap<String, pixpin_proyecto::formula::Valor>>,
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
    // M1: con el tema Cosmos, el cielo detras de la lista de proyectos (el
    // chat pone encima su papel, como en el movil). Solo en la lista: lo
    // demas lo tapa el papel y pintarlo seria trabajo tirado.
    let (cw, ch) = crate::tema_cosmos::tamano_de(d);
    crate::tema_cosmos::pintar(p, cw, ch, e, rf(d.lista));
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
    crate::tema_cosmos::rellenar_o_cielo(p, rf(d.lista), tema.lista);
    crate::tema_cosmos::rellenar_o_cielo(p, rf(d.cabecera_lista), tema.cabecera);
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

    // El boton de proyecto nuevo NO se pinta aqui: va al final de las filas
    // (`pintar_boton_nuevo`, al cierre de `pintar_filas`). Aqui quedaba
    // DEBAJO de ellas, y la fila bajo el raton lo tapaba, que es lo que el
    // usuario vio: «cuando pongo el mouse sobre un chat, este los superpone
    // y oculta el boton».

    // Los botones de sincronizar y del universo (D210): redondos, marron
    // oscuro y con el icono crema, los colores de los botones redondos del
    // chat del movil. Los dos iguales porque los dos llevan a una pantalla
    // de TODOS los proyectos.
    for (boton, icono) in [
        (d.boton_sincro(escala), &SINCRO),
        (
            d.boton_extra(pixpin_ui::chat::Extra::Tareas, escala),
            &mi::CHECKLIST,
        ),
        (
            d.boton_extra(pixpin_ui::chat::Extra::Galeria, escala),
            &mi::PHOTO_LIBRARY,
        ),
        (
            d.boton_extra(pixpin_ui::chat::Extra::Timeline, escala),
            &mi::TIMELINE,
        ),
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
    // que parece un fallo. Si lo que la vacia es la busqueda, se dice eso.
    if lista.orden.is_empty() {
        centrar_texto(
            &textos.t(clave_de_lista_vacia(lista.busqueda)),
            d.filas,
            None,
            13.0 * e,
            tema.apagado,
        );
    } else {
        pintar_filas(p, d, tema, escala, lista);
    }
    // Y encima de ellas, el boton de proyecto nuevo: flota sobre la lista, y
    // una fila resaltada no puede taparlo.
    pintar_boton_nuevo(p, d, tema, escala);

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

/// Que dice la lista cuando no ensena ninguna fila: con la busqueda puesta
/// (la misma regla que `filtrar`: espacios solos no filtran), que no hay
/// nada con eso; sin ella, que aun no hay proyectos. Decir «Aun no hay
/// proyectos» con proyectos ocultos por el filtro era falso.
fn clave_de_lista_vacia(busqueda: &str) -> &'static str {
    if busqueda.trim().is_empty() {
        "chat-sin-proyectos"
    } else {
        "chat-buscar-nada"
    }
}

/// El boton redondo de proyecto nuevo, flotando en la esquina de la lista.
///
/// Se pinta DESPUES de las filas, siempre: es lo unico que lo mantiene a la
/// vista cuando el raton pasa por la fila que tiene debajo.
fn pintar_boton_nuevo(p: &Pintor, d: &Disposicion, tema: &Tema, escala: u32) {
    let e = escala as f32 / 100.0;
    let nuevo = d.boton_nuevo(escala);
    if nuevo.ancho == 0 {
        return;
    }
    p.rellenar_redondeado(rf(nuevo), nuevo.alto as f32 / 2.0, tema.enviar);
    // Una cruz a mano: dos lineas y un icono aqui seria un recurso de mas,
    // como el triangulo de enviar.
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

        // El avatar: su logo si le puso uno, o un circulo de su color con
        // las iniciales.
        let a = rf(partes.avatar);
        if !logo::pintar(p, &ficha.id, a) {
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
        }
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
        // Guardado en un disco que ahora no esta (el USB fuera): eso manda
        // sobre la ultima linea, que es lo que explica por que no se abre.
        // Solo mira el disco para los que tienen carpeta propia.
        let resumen = if !pixpin_proyecto::ubicacion::disponible(ficha) {
            lista.textos.t("ubicacion-no-disponible")
        } else if ficha.resumen.is_empty() {
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
    } else if let Some(aviso) = aviso_de_lecciones(a) {
        // Sin fijado, la franja es del aviso de lecciones: «💡 N lecciones:»
        // y la primera. Tocarla abre las de este proyecto.
        let barra = d.fijado(escala);
        p.rellenar(rf(barra), tema.cabecera);
        let x = barra.x as f32 + chat::FIJADO_MARGEN_X as f32 * e;
        let ancho = (barra.derecha() as f32 - chat::FIJADO_MARGEN_X as f32 * e - x).max(0.0);
        let rotulo = format!(
            "💡 {} {}",
            aviso.cuantas,
            if aviso.cuantas == 1 {
                "lección"
            } else {
                "lecciones"
            }
        );
        p.texto_color(
            &rotulo,
            x,
            barra.y as f32 + 7.0 * e,
            chat::CONTADOR_TAM * e,
            tema.enviar,
        );
        p.texto_linea(
            &aviso.linea,
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
        a.zonas.borrow_mut().push((barra, Zona::Lecciones));
    }

    // La fila de chips de etiqueta, solo mientras se busca: cuelga de la
    // cabecera (o del fijado) y le roba alto al historial. Se pinta ANTES
    // que las burbujas porque su alto es el que decide donde empiezan.
    let chips = chips_de(a);
    if !chips.is_empty() {
        let fila = d.chips(chips.len(), hay_barra(a), escala);
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
            // Sin punto en la esquina: el punto va solo al lado del nombre
            // (el usuario, 3-oct).
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

        // «Recibido de …», en el color de la hora.
        if let Some((texto, alto)) = &piezas.recibido {
            p.texto_linea(
                texto,
                dentro.x as f32,
                y,
                RECIBIDO_TAM * e,
                dentro.ancho as f32 + 1.0,
                color_hora,
            );
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
                    p.rellenar_redondeado(hoja, 6.0 * e, papel_de_vista(l, tema.papel));
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
                    // Sin punto en la esquina: el punto va solo al lado del
                    // nombre (el usuario, 3-oct).
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
            // Un archivo, con el icono de su tipo (v0.98.4); lo demas, el
            // boton redondo de siempre.
            if let Some(nombre) = &f.archivo {
                icono_de_tipo::pintar(p, nombre, circulo);
            } else {
                p.rellenar_redondeado(circulo, circulo.ancho / 2.0, tema.circulo);
                icono_centrado(p, f.icono, fila.circulo, 24.0 * e, tema.circulo_icono);
            }
            punto_de_proyecto(p, rf(fila.punto), &a.raiz, m);
            // Una nota de voz ensena su ONDA en el sitio del nombre, dibujada
            // de los picos que el movil anoto al grabarla. Sin picos no se
            // inventa ninguna: una onda de mentira mentiria sobre lo que se
            // dijo, y se queda la fila de siempre con el nombre y el tiempo.
            if !f.onda.is_empty() {
                // Si esta nota es la que suena, la onda avanza con ella.
                let avance = a
                    .mensajes
                    .get(i)
                    .and_then(|m| ruta_del_mensaje(&a.raiz, &a.ficha.id, m))
                    .filter(|r| crate::audio::cargado().as_deref() == Some(r.as_path()))
                    .map(|_| crate::audio::estado().fraccion());
                // Mientras se pasa a texto, el renglon de la duracion cuenta
                // por donde va. Ahi y no sobre la onda: la onda es lo que
                // dice DE QUE nota se trata, y pisarla con una barra de
                // avance dejaria la burbuja sin senas mientras mas falta
                // hacen (hay varias notas seguidas y todas se parecen).
                let detalle = match a
                    .transcribiendo
                    .as_ref()
                    .filter(|t| a.mensajes.get(i).is_some_and(|m| m.id == t.id()))
                {
                    Some(t) => {
                        let mut args = fluent_bundle::FluentArgs::new();
                        args.set("pct", (t.avance() * 100.0).round() as i64);
                        // La primera vez, la barra es la del modelo bajando.
                        let clave = if t.bajando() {
                            "chat-voz-bajando-whisper"
                        } else {
                            "chat-transcribiendo"
                        };
                        textos.t_args(clave, &args)
                    }
                    None => f.detalle.clone(),
                };
                pintar_onda(
                    p, &f.onda, fila.texto, tema, &detalle, color_hora, e, avance,
                );
            } else {
                let nombre = match &a.renombrando_mensaje {
                    Some((n, escrito)) if *n == i && !a.describiendo => format!("{escrito}|"),
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
                // **El lapiz al lado del nombre** (4-oct-2026 en el movil):
                // pulsarlo cambia el nombre, para que nadie tenga que
                // adivinar que se puede. Solo si cabe en el renglon.
                if a.renombrando_mensaje.is_none() && renombrar::se_puede(m) {
                    let (wn, alto_n) = p.medir_texto(&nombre, FICHA_NOMBRE_TAM * e);
                    let lado = 15.0 * e;
                    let x = fila.texto.x as f32 + wn + 4.0 * e;
                    if x + lado <= (fila.texto.x + fila.texto.ancho as i32) as f32 + 1.0 {
                        let sitio = RectF {
                            x,
                            y: ty + (alto_n - lado) / 2.0,
                            ancho: lado,
                            alto: lado,
                        };
                        p.icono(&mi::EDIT, sitio, tema.enviar);
                        let toque = 6.0 * e;
                        a.zonas.borrow_mut().push((
                            Rect {
                                x: (sitio.x - toque) as i32,
                                y: (sitio.y - toque) as i32,
                                ancho: (lado + 2.0 * toque) as u32,
                                alto: (lado + 2.0 * toque) as u32,
                            },
                            Zona::Renombrar(i),
                        ));
                    }
                }
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
            // icono en ese mismo color. En una nota de voz es la de pasar a
            // texto (`BotonDeTexto`), que ocupa el MISMO sitio y mide lo
            // mismo que la de «abrir fuera»: son la misma familia de botones.
            let abrir = rf(fila.abrir);
            match boton_de_texto(a, m) {
                None => {
                    p.rellenar_redondeado(
                        abrir,
                        chat::FILA_ABRIR_RADIO as f32 * e,
                        con_alfa(color_hora, 0.156),
                    );
                    icono_centrado(p, &mi::OPEN_IN_NEW, fila.abrir, 16.0 * e, color_hora);
                    a.zonas.borrow_mut().push((fila.abrir, Zona::Abrir(i)));
                }
                Some(BotonDeTexto::Oculto) => {}
                Some(boton) => {
                    // Mientras muele, la pastilla se queda con su icono y
                    // sin poder pulsarse: quien avisa del avance es el
                    // renglon de la duracion.
                    let trabajando = boton == BotonDeTexto::Trabajando;
                    let tinta = if trabajando {
                        con_alfa(color_hora, 0.5)
                    } else {
                        color_hora
                    };
                    p.rellenar_redondeado(
                        abrir,
                        chat::FILA_ABRIR_RADIO as f32 * e,
                        con_alfa(color_hora, 0.156),
                    );
                    match boton {
                        BotonDeTexto::Plegar | BotonDeTexto::Desplegar => {
                            pintar_flechita(p, abrir, boton == BotonDeTexto::Plegar, tinta, e)
                        }
                        _ => icono_centrado(p, &mi::SUBTITLES, fila.abrir, 16.0 * e, tinta),
                    }
                    if !trabajando {
                        a.zonas.borrow_mut().push((fila.abrir, Zona::Texto(i)));
                    }
                }
            }
            y += chat::FILA_CIRCULO as f32 * e + hueco;
        }

        // El texto de una nota de voz, pegado bajo su fila.
        if let Some(v) = &piezas.voz {
            let arriba = if piezas.fila.is_some() {
                y - hueco + 6.0 * e
            } else {
                y
            };
            pintar_texto_de_voz(p, a, i, v, dentro.x as f32, arriba, color_texto, e);
            y = arriba + v.alto + hueco;
        }

        if let Some(v) = &piezas.viene {
            // Su aire de arriba hace de hueco con lo anterior, si lo hay.
            let encima = if y > dentro.y as f32 { y - hueco } else { y };
            let caja = pintar_viene_de(p, tema, textos, v, burbuja.x as f32, encima, e);
            a.zonas.borrow_mut().push((caja, Zona::VieneDe(i)));
            y += VIENE_ALTO * e + 2.0 * VIENE_AIRE_Y * e;
        }

        // La tarjeta del enlace (`TarjetaDeEnlace`): la barra del acento a la
        // izquierda, el icono del enlace, el dominio en negrita de acento y
        // de que va debajo. Pulsarla abre la direccion fuera.
        if let Some((en, alto)) = &piezas.enlace {
            let caja = RectF {
                x: dentro.x as f32,
                y,
                ancho: dentro.ancho as f32,
                alto: *alto,
            };
            p.rellenar_redondeado(caja, 4.0 * e, con_alfa(tema.enviar, 0.10));
            p.rellenar(
                RectF {
                    ancho: 3.0 * e,
                    ..caja
                },
                tema.enviar,
            );
            let lado = 18.0 * e;
            p.icono(
                &mi::LINK,
                RectF {
                    x: caja.x + 13.0 * e,
                    y: caja.y + (caja.alto - lado) / 2.0,
                    ancho: lado,
                    alto: lado,
                },
                tema.enviar,
            );
            let tx = caja.x + ENLACE_IZQUIERDA * e;
            let tw = (caja.ancho - (ENLACE_IZQUIERDA + ENLACE_DERECHA) * e).max(0.0);
            let ty = caja.y + ENLACE_AIRE_Y * e;
            // En el color del acento y no en negrita: `texto_linea` corta con
            // puntos suspensivos a una linea, y una negrita medida como
            // normal se saldria de la tarjeta.
            p.texto_linea(&en.donde, tx, ty, ENLACE_TAM * e, tw, tema.enviar);
            if !en.de_que.is_empty() {
                let (_, hd) = p.medir_texto(&en.donde, ENLACE_TAM * e);
                p.texto_linea(&en.de_que, tx, ty + hd, ENLACE_TAM * e, tw, color_texto);
            }
            a.zonas.borrow_mut().push((
                Rect {
                    x: caja.x as i32,
                    y: caja.y as i32,
                    ancho: caja.ancho as u32,
                    alto: caja.alto as u32,
                },
                Zona::Enlace(i),
            ));
            y += alto + hueco;
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
        // Lo del buzon: la franja roja a la izquierda dice «esto es temporal»
        // antes de leer nada, y el pie dice cuando llego y cuando se va, la
        // de irse en rojo porque es la unica que obliga a hacer algo.
        if let Some((llego, se_va)) = &piezas.buzon {
            let rojo = hex(0xe5534b);
            p.rellenar_redondeado(
                RectF {
                    x: burbuja.x as f32 - 6.0 * e,
                    y: burbuja.y as f32 + 4.0 * e,
                    ancho: 3.0 * e,
                    alto: (burbuja.alto as f32 - 8.0 * e).max(0.0),
                },
                2.0 * e,
                con_alfa(rojo, 0.5),
            );
            let tam = HORA_TAM * e;
            let antes = format!("{llego} → ");
            p.texto(&antes, x, y_hora, tam, color_hora);
            x += p.medir_texto(&antes, tam).0;
            p.texto(se_va, x, y_hora, tam, rojo);
            x += p.medir_texto(se_va, tam).0 + 8.0 * e;
        }
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
        // El despertador va ANTES de la chincheta, como en el movil, y dice
        // solo que hay hora puesta: la hora en si se lee en el menu, que es
        // donde se cambia.
        if crate::recordatorios::hora_de(m).is_some() {
            let lado = HORA_TAM * e;
            p.icono(
                &mi::ALARM,
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
        // **El punto rojo del movil**: se unio al proyecto y su hoja ya no
        // esta. Dice que en el menu hay «Volver a anadir al proyecto».
        if crate::pdf_en_chat::sin_su_hoja(
            &pixpin_proyecto::almacen::carpeta(&a.raiz, &a.ficha.id),
            m,
        ) {
            let lado = 8.0 * e;
            p.rellenar_redondeado(
                RectF {
                    x: burbuja.derecha() as f32 - lado - 4.0 * e,
                    y: burbuja.y as f32 + 4.0 * e,
                    ancho: lado,
                    alto: lado,
                },
                lado / 2.0,
                hex(0xE53935),
            );
        }
        pintar_flecha_de_comentar(p, tema, burbuja, corrida, escala, e);
    }
    p.soltar_recorte();
}

/// La onda de una nota de voz, en el sitio del nombre de su fila: las barras
/// arriba y la duracion debajo, como en el movil.
///
/// Se pinta barra a barra y no de un trazado cacheado como en el movil: son
/// cincuenta rectangulos por nota de voz a la vista, que no se notan al lado
/// de una foto, y asi el borde de avance cae donde de verdad va la pista en
/// vez de saltar de tres en tres.
///
/// `avance` es por donde va lo que suena, de 0 a 1, o `None` si esta nota no
/// es la que esta sonando. Lo ya oido se pinta entero y lo que queda, apagado.
#[allow(clippy::too_many_arguments)] // las barras, donde van, los dos colores y la escala
fn pintar_onda(
    p: &Pintor,
    onda: &[f32],
    sitio: Rect,
    tema: &Tema,
    duracion: &str,
    color_duracion: Color,
    e: f32,
    avance: Option<f32>,
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
        // Una barra cuenta como oida cuando su CENTRO ya paso: con el borde
        // izquierdo, la primera saldria encendida antes de empezar a sonar.
        let oida = avance.is_none_or(|f| {
            let cuantas = onda.len().max(1) as f32;
            (n as f32 + 0.5) / cuantas <= f.clamp(0.0, 1.0)
        });
        p.rellenar_redondeado(
            RectF {
                x,
                y: eje - medio,
                ancho: grueso,
                alto: medio * 2.0,
            },
            grueso / 2.0,
            if oida {
                tema.enviar
            } else {
                con_alfa(tema.enviar, 0.35)
            },
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
    idioma: pixpin_store::Idioma,
    identidad: &str,
) {
    // Una lista de tareas se abre en la ventana de Tareas, en esa lista: la
    // misma que el boton de Tareas (el usuario, 8-oct-2026: «que la interfaz
    // sea unificada»). Antes se abria un panel propio dentro del chat.
    if let Some(a) = abierto.as_ref()
        && let Some(m) = a.mensajes.get(indice)
        && m.clase == Some(pixpin_proyecto::cuaderno::Clase::MiniApp)
        && m.miniapp.as_deref() == Some(pixpin_proyecto::mini::TAREAS)
    {
        crate::tareas::abrir_en(
            idioma,
            ubicacion.clone(),
            identidad,
            format!("{}/{}", a.ficha.id, m.id),
        );
        return;
    }
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
    // Una hoja nota del proyecto (las de la galeria) se abre en el editor de
    // notas, como al tocarla en Proyectos del movil (H12). Una nota suelta
    // del chat no: esa se abre desde su menu.
    let hoja_nota = abierto.as_ref().and_then(|a| {
        let m = a.mensajes.get(indice)?;
        (m.clase == Some(pixpin_proyecto::cuaderno::Clase::Nota)
            && m.uid.as_ref().is_some_and(|u| a.hojas_uid.contains(u)))
        .then(|| crate::notas_md::Destino::Mensaje {
            proyecto: a.ficha.id.clone(),
            codigo: m.codigo_unico(),
        })
    });
    if let Some(destino) = hoja_nota {
        crate::notas_md::abrir(idioma, ubicacion.clone(), destino);
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
            matches!(
                m.clase,
                Some(pixpin_proyecto::cuaderno::Clase::Dibujo)
                    | Some(pixpin_proyecto::cuaderno::Clase::Pagina)
            ) && m.referencia.as_deref().is_some_and(|r| !r.is_empty())
                // Una pagina del documento sin dibujo todavia se abre igual
                // que en el movil: en el lienzo, con la pagina de fondo, y
                // estrenando su dibujo (`pdf_en_chat::asegurar_dibujo`). No
                // en el lector de PDF, que no deja dibujar encima.
                || crate::pdf_en_chat::es_pagina_sin_dibujo(m)
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
    // Un Excel, un .ods o un CSV se abren como tablas (D11, `libro.rs`).
    if let Some(a) = abierto.as_mut()
        && libro::abrir_libro(ubicacion, a, indice, textos)
    {
        return;
    }
    match abierto.as_mut() {
        Some(a) if es_tabla => abrir_hoja(a, indice),
        // Una foto se abre en el editor 2D, con la foto de fondo: es donde
        // estan las herramientas. Dibujar en la propia burbuja sigue estando,
        // en el menu del boton derecho.
        Some(a) if es_foto => {
            // Una foto se abre en SU lienzo, como en el movil: el que trajo
            // de alli o el que se estrena aqui (K11, `foto_anotada`).
            if crate::foto_anotada::abrir_en_su_lienzo(
                ubicacion.raiz(),
                &a.ficha.id,
                &mut a.mensajes,
                indice,
                lienzo,
            ) {
                if let Some(m) = a.mensajes.get(indice).cloned() {
                    a.vistas[indice] = leer_vista(ubicacion, &a.ficha.id, &m);
                }
                a.colocado.borrow_mut().ancho = 0;
            } else if let Some(ruta) = a
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
            let mut guardadas = abrir_dibujo(ubicacion, a, indice, lienzo);
            // La pagina que estreno su dibujo al abrirse lo apunta tambien en
            // su mensaje: la proxima vez se abre ese, y su vista ensena lo
            // dibujado encima de la pagina.
            if let Some(m) = a.mensajes.get_mut(indice)
                && crate::pdf_en_chat::es_pagina_sin_dibujo(m)
                && let Some(d) =
                    crate::pdf_en_chat::dibujo_de_la_hoja(ubicacion.raiz(), &a.ficha.id, m)
            {
                m.referencia = Some(d);
                guardadas.push(indice);
            }
            for i in guardadas {
                if let Some(m) = a.mensajes.get(i).cloned() {
                    a.vistas[i] = leer_vista(ubicacion, &a.ficha.id, &m);
                }
            }
            a.colocado.borrow_mut().ancho = 0;
        }
        Some(a) => abrir_mensaje(ubicacion, a, indice, textos, idioma),
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
        // Un PowerPoint entra como su PDF (D10): ver `diapositivas`.
        Some(Clase::Archivo) => {
            m.nombre.to_lowercase().ends_with(".pdf")
                || crate::diapositivas::es_presentacion(&m.nombre)
        }
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
    // Un PDF no es UNA hoja: es el documento del proyecto y una hoja por
    // pagina (`hojasDelPdf` del movil). Va en su hilo, que puede tardar.
    if m.clase == Some(pixpin_proyecto::cuaderno::Clase::Archivo) {
        return unir_pdf(ubicacion, a, i, textos, crate::pdf_en_chat::Trabajo::Unir);
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

/// Empieza a meter el PDF del mensaje `i` en el proyecto, en su hilo: como
/// documento con sus paginas («Anadir al proyecto») o como fotos («Anadir
/// como imagenes»). Lo que devuelve es solo el aviso de que empezo.
fn unir_pdf(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    i: usize,
    textos: &Catalogo,
    trabajo: crate::pdf_en_chat::Trabajo,
) -> Efecto {
    let Some(m) = a.mensajes.get(i) else {
        return Efecto::Nada;
    };
    let Some(pdf) = ruta_del_mensaje(ubicacion.raiz(), &a.ficha.id, m).filter(|r| r.is_file())
    else {
        return Efecto::Aviso(textos.t("chat-sin-archivo"));
    };
    if crate::pdf_en_chat::ocupado(&a.ficha.id, &m.id) {
        return Efecto::Aviso(textos.t("unir-pdf-ya-va"));
    }
    let nombre = std::path::Path::new(&m.nombre)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| m.nombre.clone());
    if !crate::pdf_en_chat::empezar(trabajo, ubicacion.raiz(), &a.ficha, &m.id, &pdf, &nombre) {
        return Efecto::Aviso(textos.t("unir-pdf-no"));
    }
    Efecto::Aviso(textos.t("unir-pdf-empezado"))
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
    // Un PDF se une en su hilo: sus hojas aun no estan al volver de aqui, y
    // el aviso de como acabo lo da `recoger_lo_de_los_pdf`.
    if a.mensajes
        .get(i)
        .is_some_and(|m| m.clase == Some(pixpin_proyecto::cuaderno::Clase::Archivo))
    {
        return unir_al_proyecto(ubicacion, a, i, textos);
    }
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

/// Si el raton, en coordenadas de la ventana, ya esta fuera de ella: es
/// cuando agarrar una burbuja deja de ser comentarla y pasa a sacarla.
fn sale_de_la_ventana(l: Punto, ancho: u32, alto: u32) -> bool {
    l.x < 0 || l.y < 0 || l.x >= ancho as i32 || l.y >= alto as i32
}

/// Lo que se lleva la burbuja de verdad: una foto con algo dibujado encima
/// (aqui o en el movil) viaja FUSIONADA, en un PNG hecho como lo hace la
/// hoja de compartir; lo demas, como [`carga_de_la_burbuja`].
fn carga_fusionada(a: &Abierto, i: usize, t: &Catalogo) -> Option<pixpin_pin::Carga> {
    let m = a.mensajes.get(i)?;
    crate::compartir::foto_fusionada(&a.raiz, &a.ficha.id, m, t)
        .map(pixpin_pin::Carga::Fichero)
        .or_else(|| carga_de_la_burbuja(a, i))
}

/// Que se lleva la burbuja al sacarla de la ventana: su fichero si lo tiene
/// en este equipo y, si no, lo que dice. Una burbuja sin nada (un mensaje
/// vacio) no se arrastra: soltar una nada en otro programa confunde.
fn carga_de_la_burbuja(a: &Abierto, i: usize) -> Option<pixpin_pin::Carga> {
    let m = a.mensajes.get(i)?;
    if let Some(ruta) = ruta_del_mensaje(&a.raiz, &a.ficha.id, m).filter(|r| r.is_file()) {
        return Some(pixpin_pin::Carga::Fichero(ruta));
    }
    let texto = m.resumen();
    (!texto.trim().is_empty()).then(|| pixpin_pin::Carga::Texto(texto.trim().to_string()))
}

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
    // Las paginas de un PDF unido (y sus fotos) son del proyecto: en el chat
    // ya esta su mensaje, y en la galeria se ven todas.
    if crate::pdf_en_chat::solo_galeria(m) {
        return false;
    }
    // Las lecciones no se mezclan con el chat (el movil, 4-oct): viajan como
    // mensajes, pero se ven en su ventana (`crate::lecciones`).
    if crate::lecciones::almacen::es_de_leccion(m) {
        return false;
    }
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

/// **El aviso de lecciones** del chat de un proyecto (`AvisoDeLecciones` del
/// movil): las que le tocan, en la franja de debajo de la cabecera. En la
/// conversacion general no sale, como alli.
fn aviso_de_lecciones(a: &Abierto) -> Option<crate::lecciones::Aviso> {
    if a.ficha.es_guardados() {
        return None;
    }
    crate::lecciones::aviso(&a.raiz, &a.ficha.id, &a.ficha.nombre)
}

/// Si hay franja bajo la cabecera: la del fijado o, sin el, la del aviso de
/// lecciones. Las dos van en el mismo sitio.
fn hay_barra(a: &Abierto) -> bool {
    a.fijado.is_some() || aviso_de_lecciones(a).is_some()
}

/// El area de las burbujas, ya descontada la fila de chips si la hay.
fn area_del_historial(d: &Disposicion, a: &Abierto, escala: u32) -> Rect {
    d.historial(a.alto_caja.get(), hay_barra(a), chips_de(a).len(), escala)
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
        // El lapiz de cambiar el nombre va detras del nombre (`renombrar`).
        let lapiz = if renombrar::se_puede(m) {
            (4.0 + 15.0) * e
        } else {
            0.0
        };
        let ancho_texto = if onda.is_empty() {
            (wn + lapiz).max(wd)
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
            archivo: icono_de_tipo::nombre_para_el_icono(m),
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
        // Vivo se dibuja sobre la foto ENTERA y sola (ver `pintar_foto`).
        tamano_de_vista(
            v,
            ancho_max,
            e,
            if vivo { VIVO_FACTOR } else { 1.0 },
            vivo || solo_la_foto(m),
        )
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
            voz: None,
            recibido: None,
            enlace: None,
            buzon: None,
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
    // «Recibido de …», debajo de la cita y encima de lo demas, como en el
    // movil: dice de que aparato vino antes de ensenar que vino.
    let recibido = tarjetas::recibido_de(m).map(|de| {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("de", de.to_string());
        let texto = textos.t_args("chat-recibido-de", &args);
        let (w, h) = p.medir_texto(&texto, RECIBIDO_TAM * e);
        sumar(&mut ancho, &mut alto, w, h);
        (texto, h)
    });
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
    // El texto de una nota de voz va pegado a su fila, debajo de la onda,
    // con el aire de `padding(top = 6.dp)` y no con el hueco de siempre.
    let voz = medir_texto_de_voz(p, m, a.desplegados.contains(&m.id), ancho_max, textos, e);
    if let Some(v) = &voz {
        let antes = alto;
        sumar(&mut ancho, &mut alto, v.ancho, v.alto);
        if antes > 0.0 {
            alto += 6.0 * e - hueco;
        }
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
    // La tarjeta del enlace, ENCIMA del texto como en cualquier mensajeria:
    // lo que se toca es la tarjeta, y al final obligaria a leer la nota
    // entera para llegar a ella. Solo en las notas (`Clase.NOTA`).
    let enlace = matches!(m.clase, None | Some(pixpin_proyecto::cuaderno::Clase::Nota))
        .then(|| tarjetas::primer_enlace(&m.texto))
        .flatten()
        .map(|en| {
            let (wd, hd) = p.medir_texto(&en.donde, ENLACE_TAM * e);
            let (wq, hq) = if en.de_que.is_empty() {
                (0.0, 0.0)
            } else {
                p.medir_texto(&en.de_que, ENLACE_TAM * e)
            };
            let alto_tarjeta = hd + hq + 2.0 * ENLACE_AIRE_Y * e;
            let w = (ENLACE_IZQUIERDA + ENLACE_DERECHA) * e + wd.max(wq);
            sumar(&mut ancho, &mut alto, w, alto_tarjeta);
            (en, alto_tarjeta)
        });
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
    // En el buzon, delante, cuando llego y cuando se va («24 sep → 1 oct»):
    // quien no lo sepa dara por guardado algo que va a desaparecer.
    let buzon = m.en_buzon.then(|| {
        (
            fecha_corta(textos, m.cuando),
            fecha_corta(textos, tarjetas::se_va_el(m.cuando)),
        )
    });
    if let Some((llego, se_va)) = &buzon {
        renglon += p.medir_texto(&format!("{llego} → {se_va}"), HORA_TAM * e).0 + 8.0 * e;
    }
    if let Some(codigo) = chapa_de_codigo(m) {
        renglon += p.medir_texto(&codigo, CHAPA_TAM * e).0 + 12.0 * e;
    }
    if let Some(emoji) = m.emoji.as_deref().filter(|s| !s.is_empty()) {
        renglon += p.medir_texto(emoji, HORA_TAM * e).0 + 3.0 * e;
    }
    // El despertador y la chincheta ocupan lo mismo. Sumarlos aqui es lo que
    // evita que el renglon de abajo se coma la ultima palabra del texto: en
    // el movil ese mismo olvido dejaba el hueco corto.
    if crate::recordatorios::hora_de(m).is_some() {
        renglon += (HORA_TAM + 3.0) * e;
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
        voz,
        recibido,
        enlace,
        buzon,
    };
    (entrada, piezas)
}

/// La fecha corta del buzon, «24 sep» (`cortaDe`, patron «d MMM»): sin ano,
/// porque en el buzon nada dura mas de una semana. El orden lo pone el
/// idioma (`chat-fecha-corta`).
pub(crate) fn fecha_corta(textos: &Catalogo, cuando_utc: i64) -> String {
    let (_, mes, dia) = pixpin_ui::chat::partes_fecha(pixpin_shell::entorno::a_local(cuando_utc));
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("dia", dia);
    args.set(
        "mes",
        tarjetas::mes_corto(&textos.t(&format!("chat-mes-{mes}"))),
    );
    textos.t_args("chat-fecha-corta", &args)
}

/// Abre fuera el enlace de la tarjeta de una nota (`abrirEnlace`), con el
/// navegador que tenga puesto Windows. Si no se puede, se dice en vez de no
/// hacer nada. Solo `http`/`https`: es lo unico que `primer_enlace` saca, y
/// pasarle a `ShellExecuteW` otra cosa podria ejecutar un programa.
fn abrir_enlace(a: &Abierto, i: usize, textos: &Catalogo) -> Efecto {
    let Some(en) = a
        .mensajes
        .get(i)
        .and_then(|m| tarjetas::primer_enlace(&m.texto))
    else {
        return Efecto::Nada;
    };
    let url = en.url.to_ascii_lowercase();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Efecto::Nada;
    }
    match pixpin_shell::abrir::abrir(std::path::Path::new(&en.url)) {
        Ok(()) => Efecto::Nada,
        Err(e) => {
            tracing::warn!(?e, "no se pudo abrir el enlace");
            Efecto::Aviso(textos.t("chat-enlace-no-abre"))
        }
    }
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
        // Compartir solo si alguno lleva fichero: se mira que APUNTE a uno,
        // no el disco, porque esto se pinta en cada fotograma; si al final
        // no esta, lo dice el clic.
        let hay_fichero = a.mensajes.iter().any(|m| {
            a.marcados.contains(&m.id) && m.ruta.as_deref().is_some_and(|r| !r.is_empty())
        });
        if hay_fichero {
            botones.push((&mi::IOS_SHARE, Zona::SelCompartir));
        }
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
    // La lupa y los tres puntos, en una sola pastilla.
    pildora(p, tema, rf(pil.derecha), e);
    // La lupa enciende y apaga, como en el movil: el mismo boton, con aspa
    // mientras se busca.
    let icono_lupa: &Icono = if a.busqueda.is_some() {
        &mi::CLOSE
    } else {
        &mi::SEARCH
    };
    icono_centrado(p, icono_lupa, pil.buscar, 24.0 * e, tema.pildora_texto);
    icono_centrado(p, &mi::MORE_VERT, pil.menu, 24.0 * e, tema.pildora_texto);
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
    let (se_ven, bytes) = crate::pdf_en_chat::cuenta_de_la_cabecera(&a.mensajes);
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("n", se_ven);
    let mut sub = textos.t_args("chat-cosas", &args);
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
    // Con logo puesto, el logo ocupa el disco de la carpeta: es el mismo
    // proyecto que en la lista y tiene que reconocerse igual.
    if !logo::pintar(p, &a.ficha.id, circulo) {
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
    }
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

/// El punto de si vive tambien en los proyectos: verde si alguna hoja de un
/// proyecto es suya, rojo si solo esta en el chat (`PuntoDeProyecto`). Lo
/// decide `punto_de_proyecto::esta`, no el chat en el que se ve: mandar algo
/// al chat de un proyecto no lo une. Con un halo blanco para que se lea sobre
/// la burbuja y sobre una foto. Una nota o una voz no llevan punto.
fn punto_de_proyecto(
    p: &Pintor,
    r: RectF,
    raiz: &std::path::Path,
    m: &pixpin_proyecto::cuaderno::Mensaje,
) {
    if !crate::punto_de_proyecto::lleva_punto(m) {
        return;
    }
    let en_un_proyecto = crate::punto_de_proyecto::esta(raiz, m);
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
    let (
        Some(Some(Ojeada::Foto {
            doc,
            dibujo,
            trazos,
        })),
        Some(m),
    ) = (a.vistas.get(i), a.mensajes.get(i))
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
                    grafitos: Vec::new(),
                    caja: (0.0, 0.0, dw, dh),
                    fondo: None,
                    papel: None,
                    giro_del_fondo: 0.0,
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
        // Quieta: la foto y, encima, lo que se haya dibujado en ella, en el
        // encuadre que pida el mensaje: la foto sola (lo normal) o la foto
        // con lo dibujado fuera de ella. El encuadre sale de la misma
        // funcion que midio la burbuja (`tamano_de_vista`).
        None => {
            let (x0, y0, x1, y1) = encuadre_de_foto(*doc, *trazos, solo_la_foto(m));
            let (ew, eh) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
            let encaje = (hoja.ancho / ew).min(hoja.alto / eh);
            let dest = RectF {
                x: hoja.x + (hoja.ancho - ew * encaje) / 2.0,
                y: hoja.y + (hoja.alto - eh * encaje) / 2.0,
                ancho: ew * encaje,
                alto: eh * encaje,
            };
            let (dw, dh) = *doc;
            let foto_en = RectF {
                x: dest.x - x0 * encaje,
                y: dest.y - y0 * encaje,
                ancho: dw * encaje,
                alto: dh * encaje,
            };
            // Con el dibujo entero, alrededor de la foto hay margen dibujado:
            // se le da papel, como el movil, o los trazos flotarian sobre
            // el color de la burbuja.
            if foto_en != dest {
                p.rellenar(dest, tema.papel);
            }
            if let Some((b, _, _)) = foto {
                p.bitmap_con(b, foto_en, None, pixpin_render::Interpolacion::Lineal);
            }
            if !dibujo.is_empty() {
                pintar_lienzo(
                    p,
                    &LienzoVisto {
                        ordenes: dibujo.clone(),
                        grafitos: Vec::new(),
                        caja: (x0, y0, x1, y1),
                        fondo: None,
                        papel: None,
                        giro_del_fondo: 0.0,
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
/// Suma a `mensajes` las hojas del `proyecto.json` que el cuaderno no tiene,
/// SOLO para verlas. No se escriben en ningun sitio.
///
/// Un proyecto que llega sincronizado trae el cuaderno del movil, y el movil
/// no anota un mensaje por cada pagina de un PDF: son hojas del proyecto. Sin
/// esto, el chat de aqui ensenaba menos cosas que la app del telefono, que es
/// lo que el usuario reporto: «si hay PDFs ahi tienen que aparecer todas sus
/// hojas y asi como en Android».
///
/// Escribirlas en el cuaderno seria peor: volverian al movil como mensajes
/// nuevos en la siguiente vuelta (por eso `completar_hojas` se salta los
/// proyectos sincronizados).
fn anadir_hojas_del_proyecto(
    ubicacion: &Ubicacion,
    id: &str,
    aparato: &str,
    mensajes: &mut Vec<pixpin_proyecto::cuaderno::Mensaje>,
) {
    // Las paginas de un PDF (y lo que salio de un mensaje de este chat)
    // solo van a la tarjeta de Proyectos: del PDF, el chat lleva su mensaje.
    crate::pdf_en_chat::para_el_chat(ubicacion.raiz(), id, aparato, mensajes);
}

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
    anadir_hojas_del_proyecto(ubicacion, &ficha.id, &aparato, &mut mensajes);
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
        cursor: Default::default(),
        letras: Default::default(),
        eligiendo_texto: false,
        clic_en_caja: None,
        info: None,
        scroll_info: 0,
        busqueda_info: String::new(),
        renombrando: false,
        buscando_info: false,
        alto_info: std::cell::Cell::new(0),
        hojas_uid: pixpin_proyecto::almacen::uids_de_hojas(ubicacion.raiz(), &ficha.id),
        anchos_pestanas: std::cell::RefCell::new(Vec::new()),
        respondiendo: None,
        renombrando_mensaje: None,
        describiendo: false,
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
        biblioteca: None,
        grabando: None,
        transcribiendo: None,
        dictado: None,
        leyendo: None,
        vivo: None,
        busqueda: None,
        por_etiqueta: None,
        comentando: None,
        barriendo: false,
        desplegados: Default::default(),
        letra: None,
        voz: Default::default(),
        salto_pendiente: None,
        hora_a_mano: None,
        quien_llama: None,
    }
}

/// Guarda lo escrito como una nota del cuaderno y lo mete en el historial.
///
/// El orden importa: primero al disco y solo si eso sale bien se ensena. Al
/// reves, un fallo de escritura dejaria en pantalla un mensaje que no
/// existe, y el usuario creeria que lo tiene guardado.
fn guardar_nota(ubicacion: &Ubicacion, a: &mut Abierto, aparato: &str) -> std::io::Result<()> {
    let texto = a.borrador.trim().to_string();
    // Sin repetir milisegundo: el id de la nota es su hora (ver
    // `descripcion::siguiente_hora`).
    let cuando = descripcion::hora_sin_repetir();
    // El numero sigue al mayor que ya hay, que es lo que hace el codigo de
    // chat (`47·K7Q2`) unico dentro de la conversacion.
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    // Si se estaba contestando a algo, la nota queda colgada de ello: es lo
    // que convierte una lista de cosas en una conversacion.
    let mensaje = escribir_nota(
        ubicacion.raiz(),
        &a.ficha.id,
        aparato,
        &texto,
        cuando,
        numero,
        a.respondiendo.clone(),
    )?;

    a.respondiendo = None;
    a.vistas.push(None);
    a.mensajes.push(mensaje);
    a.borrador.clear();
    a.cursor.soltar();
    // La ficha de la lista sube al momento: es la misma conversacion.
    a.ficha.tocado = cuando;
    a.ficha.resumen = texto;
    subir_en_la_lista(
        ubicacion.raiz(),
        &a.ficha.id,
        a.ficha.tocado,
        &a.ficha.resumen,
    )
}

/// Lo de [`guardar_nota`] que toca el disco, sin la conversacion abierta:
/// la nota sellada y anadida al cuaderno. Lo usan tambien los pedidos de
/// otros programas (`pedidos.rs`), para que un texto mandado desde Flow
/// Launcher nazca igual que uno escrito en la caja.
pub(crate) fn escribir_nota(
    raiz: &std::path::Path,
    proyecto: &str,
    aparato: &str,
    texto: &str,
    cuando: i64,
    numero: i64,
    responde_a: Option<String>,
) -> std::io::Result<pixpin_proyecto::cuaderno::Mensaje> {
    use pixpin_proyecto::cuaderno;
    let mut mensaje = cuaderno::Mensaje::nota(
        texto,
        &cuaderno::Sello {
            cuando,
            numero,
            aparato: aparato.to_string(),
            proyecto: proyecto.to_string(),
        },
    );
    mensaje.responde_a = responde_a;
    cuaderno::anadir(&pixpin_proyecto::almacen::carpeta(raiz, proyecto), &mensaje)?;
    Ok(mensaje)
}

/// Sube un proyecto en la lista: su hora y la linea que ensena debajo del
/// nombre, como al escribir en el.
pub(crate) fn subir_en_la_lista(
    raiz: &std::path::Path,
    proyecto: &str,
    cuando: i64,
    resumen: &str,
) -> std::io::Result<()> {
    let mut indice = pixpin_proyecto::almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == proyecto) {
        f.tocado = cuando;
        f.resumen = resumen.to_string();
        indice.guardar(raiz)?;
    }
    Ok(())
}

/// El numero del siguiente mensaje de un proyecto, leido de su cuaderno: el
/// mayor que hay mas uno, como hace el chat con los que tiene a la vista.
/// Un proyecto sin nada escrito todavia empieza por el uno.
pub(crate) fn siguiente_numero_en(raiz: &std::path::Path, proyecto: &str) -> std::io::Result<i64> {
    use pixpin_proyecto::{almacen, cuaderno};
    match cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, proyecto)) {
        Ok(c) => Ok(c.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(1),
        Err(e) => Err(e),
    }
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
            // Donde este la rayita, y con los saltos de Windows
            // (`\r\n`) hechos uno: la rayita no puede caer entre los dos.
            a.cursor
                .escribir(&mut a.borrador, &t.replace("\r\n", "\n"));
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

    // **Lo escrito en la caja va con la foto, como su descripcion**: antes
    // la foto se iba sola y el texto se quedaba atras, y el usuario lo
    // contaba como perdido (3-oct). Solo en el primero, como el pie del
    // cuadro de confirmar; si no entra ninguno, vuelve a la caja.
    let mut pie = std::mem::take(&mut a.borrador).trim().to_string();
    a.cursor.soltar();
    let mut hechos = 0;
    for (nombre, bytes) in ficheros {
        if let Err(e) = adjuntar(ubicacion, a, aparato, &nombre, &bytes, &pie) {
            tracing::warn!(?e, nombre, "fichero que no se pudo guardar");
            continue;
        }
        pie.clear();
        hechos += 1;
    }
    descripcion::pie_de_vuelta(&mut a.borrador, pie);
    Ok(hechos)
}

/// Copia un fichero al proyecto y lo deja como mensaje del cuaderno, con
/// `pie` como su descripcion (`descripcion`). Con pie se lleva tambien a
/// quien se estaba contestando, que antes colgaba de la nota del pie.
fn adjuntar(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    aparato: &str,
    nombre: &str,
    bytes: &[u8],
    pie: &str,
) -> std::io::Result<()> {
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let pie = pie.trim();
    let responde_a = if pie.is_empty() {
        None
    } else {
        a.respondiendo.clone()
    };
    // Primero al cuaderno y solo despues a la pantalla, como al escribir.
    let mensaje = adjuntar_con_pie(
        ubicacion.raiz(),
        &a.ficha.id,
        aparato,
        nombre,
        bytes,
        numero,
        pie,
        responde_a,
    )?;
    if !pie.is_empty() {
        a.respondiendo = None;
    }
    // Su ojeada AHORA, no al reabrir el proyecto: una foto recien adjuntada
    // salia como una fila con el nombre del fichero («pegado-…png») en vez
    // de verse, y solo aparecia al volver a entrar.
    a.vistas.push(leer_vista(ubicacion, &a.ficha.id, &mensaje));
    a.ficha.tocado = mensaje.cuando;
    a.ficha.resumen = mensaje.resumen();
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
    adjuntar_con_pie(raiz, proyecto, aparato, nombre, bytes, numero, "", None)
}

/// [`adjuntar_en_proyecto`] con su descripcion y a que contesta, puestos
/// ANTES de escribirlo: un mensaje, una linea en el cuaderno, como lo deja
/// el movil (`texto` del IMAGEN, ver `descripcion`).
#[allow(clippy::too_many_arguments)]
fn adjuntar_con_pie(
    raiz: &std::path::Path,
    proyecto: &str,
    aparato: &str,
    nombre: &str,
    bytes: &[u8],
    numero: i64,
    pie: &str,
    responde_a: Option<String>,
) -> std::io::Result<pixpin_proyecto::cuaderno::Mensaje> {
    use pixpin_proyecto::{almacen, cuaderno};
    let ruta = almacen::guardar_adjunto(raiz, proyecto, nombre, bytes)?;
    // Sin repetir milisegundo: dos ficheros soltados juntos salian con el
    // mismo id (ver `descripcion::siguiente_hora`).
    let cuando = descripcion::hora_sin_repetir();
    let mut mensaje = cuaderno::Mensaje::adjunto(
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
    mensaje.texto = pie.trim().to_string();
    mensaje.responde_a = responde_a;
    cuaderno::anadir(&almacen::carpeta(raiz, proyecto), &mensaje)?;
    // Un PDF se queda tal cual y se aligera DESPUES, en su cola: soltarlo no
    // espera a nadie (ver `crate::aligerar`).
    if crate::pdf_en_chat::es_pdf(&mensaje) {
        crate::aligerar::al_entrar(
            raiz,
            proyecto,
            &mensaje.id,
            &almacen::carpeta(raiz, proyecto).join(&ruta),
        );
    }
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == proyecto) {
        f.tocado = cuando;
        f.resumen = mensaje.resumen();
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

/// Un tamano de fichero como se le ensena a una persona.
///
/// Se reparte de mil en mil, no de 1024 en 1024: es lo que dice el
/// Explorador de Windows para el mismo fichero, y discrepar con el sistema
/// operativo en el numero que el usuario acaba de ver es peor que ser exacto.
#[cfg_attr(not(test), allow(dead_code))]
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
    let ruta = pixpin_proyecto::vista::ruta_real(raiz, proyecto, relativa)?;
    // Un PDF que el movil guardo como `.bin`: con la extension de su nombre.
    Some(pixpin_proyecto::vista::con_la_extension_del_nombre(raiz, ruta, &m.nombre))
}

/// **El fichero de un mensaje que esta en el disco**: el de su `ruta` o, en
/// un lienzo que llego de la lista de un proyecto sin `ruta` y solo con su
/// codigo (`referencia`), su dibujo en `lienzos/` (el usuario, 1-oct:
/// «algunos canvas me dicen que el mensaje no tiene archivo»). Es lo que
/// sacan a la pantalla, abren o envian las acciones del chat, y el pedido
/// «pinear».
pub(crate) fn fichero_del_mensaje(
    raiz: &std::path::Path,
    proyecto: &str,
    m: &pixpin_proyecto::cuaderno::Mensaje,
) -> Option<std::path::PathBuf> {
    ruta_del_mensaje(raiz, proyecto, m)
        .filter(|r| r.is_file())
        .or_else(|| {
            let id = m.referencia.as_deref().filter(|r| {
                !r.is_empty() && m.clase == Some(pixpin_proyecto::cuaderno::Clase::Dibujo)
            })?;
            Some(pixpin_proyecto::almacen::lienzo(raiz, proyecto, id)).filter(|r| r.is_file())
        })
}

/// **Los iconos de archivo del chat, en PNG, para quien los pida** (el pedido
/// «iconos» del plugin de Flow Launcher): en su hilo, porque pintar fuera de
/// pantalla espera a la GPU, y sin aviso, porque nadie lo esta mirando.
pub(crate) fn pintar_iconos_de_extension(raiz: std::path::PathBuf, extensiones: Vec<String>) {
    let lanzado = std::thread::Builder::new()
        .name("iconos-de-extension".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            match icono_de_tipo::pintar_los_que_faltan(&raiz, &extensiones) {
                Ok(0) => {}
                Ok(n) => tracing::info!(n, "iconos de extension pintados"),
                Err(e) => tracing::warn!(?e, "no se pudieron pintar los iconos de extension"),
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de los iconos");
    }
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
    a_la_vista_del_panel(a, seccion, d, escala)
        .into_iter()
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

/// Los fondos de los lienzos que se ven en el panel: las paginas de un PDF
/// unido y las hojas dibujadas sobre una. Se pintan de las vistas previas
/// (`pintar_lienzo`), y desde que esas paginas ya no salen en el chat nadie
/// mas las pedia: la galeria las ensenaba en blanco.
fn fondos_a_la_vista(
    a: &Abierto,
    seccion: pixpin_proyecto::cuaderno::Seccion,
    d: &pixpin_ui::info::Disposicion,
    escala: u32,
) -> Vec<std::path::PathBuf> {
    a_la_vista_del_panel(a, seccion, d, escala)
        .into_iter()
        .filter_map(|n| match a.vistas.get(n) {
            Some(Some(Ojeada::Lienzo(l))) => l.fondo.as_ref().map(|(r, _)| r.clone()),
            _ => None,
        })
        .collect()
}

/// Que mensajes se ven ahora en el panel, por su posicion y en orden.
fn a_la_vista_del_panel(
    a: &Abierto,
    seccion: pixpin_proyecto::cuaderno::Seccion,
    d: &pixpin_ui::info::Disposicion,
    escala: u32,
) -> Vec<usize> {
    let suyos = indices_de_la_seccion(a, seccion);
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
    suyos.into_iter().skip(primera).take(cuantas).collect()
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
    let nombre = clave_del_nombre(cual)
        .map(|c| textos.t(c))
        .unwrap_or_default();
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let mensaje = escribir_miniapp(
        ubicacion.raiz(),
        &a.ficha.id,
        aparato,
        cual,
        &nombre,
        &moneda_del_catalogo(textos),
        numero,
    )?;
    let cuando = mensaje.cuando;
    // Estas no tienen ojeada: su burbuja es la fila con el icono, el nombre y
    // el resumen. Una vista previa de «0 de 0» no dice mas que el resumen.
    a.vistas.push(None);
    a.mensajes.push(mensaje);
    a.ficha.tocado = cuando;
    a.ficha.resumen = nombre;
    subir_en_la_lista(
        ubicacion.raiz(),
        &a.ficha.id,
        a.ficha.tocado,
        &a.ficha.resumen,
    )
}

/// Lo de [`crear_miniapp`] que toca el disco, sin la conversacion abierta:
/// el documento nuevo, su mensaje sellado y anadido al cuaderno. `nombre`
/// es tambien el titulo del documento. Lo usan los pedidos (`pedidos.rs`)
/// para crear una lista de tareas desde fuera igual que desde el clip.
pub(crate) fn escribir_miniapp(
    raiz: &std::path::Path,
    proyecto: &str,
    aparato: &str,
    cual: &str,
    nombre: &str,
    moneda: &pixpin_proyecto::mini::gastos::Moneda,
    numero: i64,
) -> std::io::Result<pixpin_proyecto::cuaderno::Mensaje> {
    use pixpin_proyecto::{almacen, cuaderno};
    let documento =
        pixpin_proyecto::mini::documento_nuevo(cual, nombre, moneda).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("no se conoce la mini-app «{cual}»"),
            )
        })?;
    let mensaje = cuaderno::Mensaje::miniapp(
        cual,
        nombre,
        &documento,
        &cuaderno::Sello {
            cuando: pixpin_shell::entorno::ahora_utc_ms(),
            numero,
            aparato: aparato.to_string(),
            proyecto: proyecto.to_string(),
        },
    );
    cuaderno::anadir(&almacen::carpeta(raiz, proyecto), &mensaje)?;
    Ok(mensaje)
}

/// Crea un lienzo vacio en el proyecto y lo anuncia en el cuaderno.
///
/// El mensaje lleva `referencia` (el id, que es lo que lee el movil) Y
/// `ruta` (relativa, para poder abrirlo desde aqui): Android usa la primera
/// y este equipo la segunda, y ninguna de las dos sobra.
fn crear_lienzo(ubicacion: &Ubicacion, a: &mut Abierto, aparato: &str) -> std::io::Result<()> {
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let mensaje = escribir_lienzo(ubicacion.raiz(), &a.ficha.id, aparato, numero, None)?;
    let (cuando, nombre) = (mensaje.cuando, mensaje.nombre.clone());

    // Un lienzo recien creado esta vacio, y `leer_vista` devuelve `None` a
    // proposito para los vacios: la burbuja ensena su nombre hasta que se
    // dibuje algo.
    a.vistas.push(leer_vista(ubicacion, &a.ficha.id, &mensaje));
    a.mensajes.push(mensaje);
    a.ficha.tocado = cuando;
    a.ficha.resumen = nombre;
    subir_en_la_lista(
        ubicacion.raiz(),
        &a.ficha.id,
        a.ficha.tocado,
        &a.ficha.resumen,
    )
}

/// Lo de [`crear_lienzo`] que toca el disco, sin la conversacion abierta: el
/// `.excalidraw` vacio y su mensaje `DIBUJO` en el cuaderno.
///
/// `nombre` es el que se le pone a la fila, como al renombrarla en el chat
/// (`guardar_nombre_de_mensaje`): sin el, la fila se llama como su fichero.
/// Lo usan los pedidos (`pedidos.rs`), que pueden traerlo puesto.
pub(crate) fn escribir_lienzo(
    raiz: &std::path::Path,
    proyecto: &str,
    aparato: &str,
    numero: i64,
    nombre: Option<&str>,
) -> std::io::Result<pixpin_proyecto::cuaderno::Mensaje> {
    use pixpin_proyecto::{almacen, cuaderno};
    let id = pixpin_proyecto::codigos::nuevo();
    let ruta = almacen::lienzo(raiz, proyecto, &id);
    if let Some(padre) = ruta.parent() {
        std::fs::create_dir_all(padre)?;
    }
    let json = pixpin_motor2d::excalidraw::escribir(&pixpin_motor2d::excalidraw::Lienzo::vacio());
    std::fs::write(&ruta, &json)?;

    let fichero = format!("{id}.excalidraw");
    let mut mensaje = cuaderno::Mensaje::adjunto(
        cuaderno::Clase::Dibujo,
        &fichero,
        &format!("lienzos/{fichero}"),
        json.len() as i64,
        &cuaderno::Sello {
            cuando: pixpin_shell::entorno::ahora_utc_ms(),
            numero,
            aparato: aparato.to_string(),
            proyecto: proyecto.to_string(),
        },
    );
    mensaje.referencia = Some(id);
    // El mismo tope que al renombrar a mano.
    if let Some(n) = nombre.map(str::trim).filter(|n| !n.is_empty()) {
        mensaje.nombre = n.chars().take(80).collect();
    }
    cuaderno::anadir(&almacen::carpeta(raiz, proyecto), &mensaje)?;
    Ok(mensaje)
}

/// Copia unos ficheros al proyecto y los deja como mensajes. Devuelve
/// cuantos entraron: uno que falle no puede llevarse los demas.
fn meter_ficheros(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    aparato: &str,
    rutas: &[std::path::PathBuf],
    pie: &str,
) -> usize {
    let mut hechos = 0;
    // **El pie es la descripcion del primero**, su `texto`, como lo guarda y
    // lo ensena PixPin Android: debajo de la foto, dentro de su burbuja
    // (`descripcion`). Antes iba DESPUES como nota aparte, separada de su
    // foto, y si caia en el mismo milisegundo, con su mismo id (el id es la
    // hora): lo que empareja por id se quedaba con uno de los dos.
    let mut pie = pie.trim().to_string();
    // Un PDF entra como cualquier fichero: UN mensaje, con su primera pagina
    // de vista previa, y sin extraer nada. Antes se abria como proyecto
    // propio pintando sus paginas aqui mismo, y un documento largo dejaba la
    // ventana parada; extraer es «Anadir al proyecto», a mano y en su hilo
    // (`pdf_en_chat`), como en el movil.
    for ruta in rutas {
        let nombre = ruta
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "archivo".into());
        match std::fs::read(ruta) {
            Ok(bytes) => match adjuntar(ubicacion, a, aparato, &nombre, &bytes, &pie) {
                Ok(()) => {
                    pie.clear();
                    hechos += 1;
                }
                Err(e) => tracing::warn!(?e, nombre, "no se pudo guardar el adjunto"),
            },
            Err(e) => tracing::warn!(?e, ruta = %ruta.display(), "no se pudo leer"),
        }
    }
    // Si no entro ninguno, lo escrito no se pierde: vuelve a la caja.
    descripcion::pie_de_vuelta(&mut a.borrador, pie);
    hechos
}

/// Un lienzo ya leido y listo para pintar en su burbuja.
struct LienzoVisto {
    ordenes: Vec<pixpin_motor2d::Orden>,
    /// El grafito del dibujo, cocido aparte y a la finura de una vista
    /// previa (`tinta::grafito::ordenes_con_grafito`). Sin esto un dibujo de
    /// grafito del movil se veia en la burbuja con la raya lisa.
    grafitos: Vec<pixpin_motor2d::tinta::grafito::GrafitoSuelto>,
    /// La caja que ocupa el dibujo, en sus propias coordenadas.
    caja: (f32, f32, f32, f32),
    /// Lo que va DEBAJO del dibujo y donde: la pagina del PDF sobre la que
    /// se dibujo, o la foto de la hoja. Sin esto, una hoja de un plano se
    /// ensena como cuatro rayas flotando en blanco.
    fondo: Option<(std::path::PathBuf, (f32, f32, f32, f32))>,
    /// El giro de ese fondo alrededor del centro de su caja (el `angle` de
    /// la foto en el movil). Cero para una pagina del PDF.
    giro_del_fondo: f32,
    /// El papel del lienzo (`backgroundColor` / `viewBackgroundColor`),
    /// **siempre el suyo, tambien el blanco**: la vista previa tiene que
    /// verse como el lienzo, mismos colores (`DrawExport.aBitmap` del movil
    /// pinta `scene.backgroundColor`). Antes el blanco de fabrica se cambiaba
    /// por el papel del tema del chat, que en oscuro es un gris. `None` solo
    /// en lo que no es un lienzo (una pagina de PDF sola): el del tema.
    papel: Option<Color>,
}

/// El papel con que se ensena un lienzo en el chat: el suyo, o `tema` si
/// no es un lienzo (ver `LienzoVisto::papel`).
fn papel_de_vista(vista: &LienzoVisto, tema: Color) -> Color {
    vista.papel.unwrap_or(tema)
}

/// Lo que va en `LienzoVisto::papel` para este lienzo: el papel con que lo
/// abre el editor (`excalidraw::fondo`).
fn papel_del_lienzo(lienzo: &pixpin_motor2d::excalidraw::Lienzo) -> Option<Color> {
    let c = pixpin_motor2d::excalidraw::fondo(lienzo);
    Some(Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: 1.0,
    })
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
        let escena = pixpin_motor2d::cargar(&dibujo_de_foto(&ruta)).ok();
        let mut dibujo = escena
            .as_ref()
            // Con la tinta como la pinta el lienzo sobre su papel.
            .map(|e| {
                crate::dibujo::tema::ordenes_como_en_el_lienzo(
                    pixpin_motor2d::ordenes_de_escena(e),
                    e.fondo,
                )
            })
            .unwrap_or_default();
        let mut trazos = escena.as_ref().and_then(|e| e.caja());
        // Y lo dibujado en el movil, que vive en su propio lienzo
        // (`foto_anotada`): sin esto de una foto anotada alli solo llegaba
        // la foto.
        if let Some(d) = crate::foto_anotada::dibujo_de_la_foto(
            ubicacion.raiz(),
            proyecto,
            m,
            (w as f32, h as f32),
        ) {
            dibujo.extend(d.ordenes);
            trazos = match (trazos, d.trazos) {
                (Some(a), Some(b)) => {
                    Some((a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
                }
                (a, b) => a.or(b),
            };
        }
        return Some(Ojeada::Foto {
            doc: (w as f32, h as f32),
            dibujo,
            trazos,
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
    // Un PDF del chat ensena su primera pagina encima de su fila, como en el
    // movil. La pinta el hilo de `pdf_en_chat`: hasta que llega, la fila sola.
    if m.clase == Some(Clase::Archivo) && crate::pdf_en_chat::es_pdf(m) {
        let pdf = ruta_del_mensaje(ubicacion.raiz(), proyecto, m).filter(|r| r.is_file())?;
        let (ruta, w, h) = crate::pdf_en_chat::pagina_pintada(
            ubicacion.raiz(),
            &pdf,
            0,
            crate::pdf_en_chat::ANCHO_VISTA,
        )?;
        return Some(Ojeada::Lienzo(LienzoVisto {
            ordenes: Vec::new(),
            grafitos: Vec::new(),
            caja: (0.0, 0.0, w, h),
            fondo: Some((ruta, (0.0, 0.0, w, h))),
            papel: None,
            giro_del_fondo: 0.0,
        }));
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
    //
    // Y una pagina que ya estreno su dibujo (se abrio aqui o en el movil, y
    // su hoja lo dice en el `proyecto.json`) se ensena con el encima, como
    // `HojaDelProyecto` del movil: pagina + dibujo. Si su `.excalidraw` aun
    // no esta en este equipo, la pagina sola.
    let de_su_hoja = crate::pdf_en_chat::es_pagina_sin_dibujo(m)
        .then(|| crate::pdf_en_chat::dibujo_de_la_hoja(ubicacion.raiz(), proyecto, m))
        .flatten()
        .filter(|d| pixpin_proyecto::almacen::lienzo(ubicacion.raiz(), proyecto, d).is_file());
    if m.clase == Some(Clase::Pagina)
        && m.referencia.as_deref().is_none_or(|r| r.is_empty())
        && de_su_hoja.is_none()
    {
        // Las paginas que se extraian antes como PNG llevan su imagen en
        // `ruta`: esas se siguen viendo de ella, sin tocar el PDF.
        let propia = ruta_del_mensaje(ubicacion.raiz(), proyecto, m)
            .filter(|r| {
                r.is_file() && !r.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
            })
            .and_then(|r| {
                let (w, h) = pixpin_codec::imagen::medidas(&r).ok()?;
                Some((r, w as f32, h as f32))
            });
        let (ruta, w, h) = match propia {
            Some(p) => p,
            None => pagina_del_pdf(ubicacion.raiz(), proyecto, m.pagina?)?,
        };
        return Some(Ojeada::Lienzo(LienzoVisto {
            ordenes: Vec::new(),
            grafitos: Vec::new(),
            caja: (0.0, 0.0, w, h),
            fondo: Some((ruta, (0.0, 0.0, w, h))),
            papel: None,
            giro_del_fondo: 0.0,
        }));
    }
    // `referencia` es el id del dibujo, no un fichero: asi lo escribe
    // Android, y por eso no se usa `ruta`.
    let id = match de_su_hoja.as_deref() {
        Some(d) => d,
        None => m.referencia.as_deref().filter(|r| !r.is_empty())?,
    };
    let ruta = pixpin_proyecto::almacen::lienzo(ubicacion.raiz(), proyecto, id);
    let texto = std::fs::read_to_string(&ruta)
        .inspect_err(|e| tracing::warn!(?e, ruta = %ruta.display(), "lienzo que no se pudo leer"))
        .ok()?;
    let lienzo = pixpin_motor2d::excalidraw::leer(&texto)
        .inspect_err(|e| tracing::warn!(?e, "lienzo que no se entiende"))
        .ok()?;
    // Como la abre el editor (`a_escena`): con su papel, que decide la tinta.
    let mut escena = pixpin_motor2d::excalidraw::a_escena(&lienzo);
    // El grafito, cocido con la tinta que se ve en el lienzo (ver abajo).
    crate::dibujo::tema::grafito_como_en_el_lienzo(&mut escena);
    // El fondo de la hoja: la pagina del PDF si se dibujo sobre una, y si no
    // la foto que trae el propio dibujo (`imagenes/<id>` del paquete).
    // Con su giro: una foto girada en el movil se ensenaba derecha.
    let (fondo, giro_del_fondo) = match m.pagina {
        Some(pagina) => (
            pagina_del_pdf(ubicacion.raiz(), proyecto, pagina)
                .map(|(r, w, h)| (r, (0.0, 0.0, w, h))),
            0.0,
        ),
        None => {
            let ficheros = pixpin_motor2d::excalidraw::ficheros(&lienzo);
            escena
                .visibles()
                .find_map(|e| match e.figura {
                    pixpin_motor2d::Figura::Imagen { id_objeto } => ficheros
                        .iter()
                        .find(|(id, _)| *id == id_objeto)
                        .and_then(|(_, rel)| {
                            pixpin_proyecto::vista::ruta_real(ubicacion.raiz(), proyecto, rel)
                        })
                        .map(|r| ((r, e.caja()), e.angulo)),
                    _ => None,
                })
                .filter(|((r, _), _)| r.is_file())
                .map_or((None, 0.0), |(f, giro)| (Some(f), giro))
        }
    };
    let caja = match &fondo {
        // Con fondo manda su caja (girada, si lo esta): un trazo que se salga
        // de la hoja no puede encoger el plano entero para caber con el.
        Some((_, c)) => crate::foto_anotada::caja_girada(*c, giro_del_fondo),
        None => escena.caja()?,
    };
    // El grafito, cocido una vez aqui y no en cada fotograma. A la finura de
    // la vista mas grande en la que sale (la hoja abierta, unos mil pixeles):
    // una casilla por unidad en un plano de cuatro mil pesaria dieciseis veces
    // mas para verse igual. Con tope por dibujo; lo que no quepa, liso.
    const LADO_DEL_GRAFITO_EN_VISTA: f32 = 1024.0;
    const TOPE_DEL_GRAFITO_EN_VISTA: usize = 8 * 1024 * 1024;
    let lado = (caja.2 - caja.0).max(caja.3 - caja.1).max(1.0);
    let (ordenes, grafitos) = pixpin_motor2d::tinta::grafito::ordenes_con_grafito(
        &escena,
        (LADO_DEL_GRAFITO_EN_VISTA / lado).min(1.0),
        TOPE_DEL_GRAFITO_EN_VISTA,
    );
    // **La tinta como en el lienzo**: sobre papel de noche el editor la pinta
    // adaptada (`dibujo::tema`); la vista previa, igual. Sin esto la tinta
    // negra guardada salia negra sobre el papel negro y no se veia nada.
    let ordenes = crate::dibujo::tema::ordenes_como_en_el_lienzo(ordenes, escena.fondo);
    if ordenes.is_empty() && grafitos.is_empty() && fondo.is_none() {
        return None;
    }
    Some(Ojeada::Lienzo(LienzoVisto {
        ordenes,
        grafitos,
        caja,
        fondo,
        giro_del_fondo,
        papel: papel_del_lienzo(&lienzo),
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
        /// La caja de lo dibujado (esquinas, en coordenadas de la foto), o
        /// `None` sin dibujo. Se mide al leer y no al pintar: es la que decide
        /// el encuadre de «ver el dibujo entero» (`encuadre_de_foto`).
        trazos: Option<(f32, f32, f32, f32)>,
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
        let centro = (caja.x + caja.ancho / 2.0, caja.y + caja.alto / 2.0);
        p.girado(centro, vista.giro_del_fondo, |p| {
            p.bitmap_con(b, caja, None, pixpin_render::Interpolacion::Lineal);
        });
    }
    // El grafito, cada mapa justo antes de la orden que le toca, llevado a la
    // vista con la misma escala que los puntos.
    let mut grafitos = vista.grafitos.iter().peekable();
    let mut pintar_grafitos_hasta = |k: usize| {
        while let Some(g) = grafitos.next_if(|g| g.antes_de <= k) {
            let m = &g.mapa;
            let (gx, gy, gw, gh) = m.caja();
            let (cx, cy) = mover(&m.centro);
            let mapa = pixpin_render::MapaGrafito {
                rgba: &m.rgba,
                ancho: m.ancho,
                alto: m.alto,
                caja: (
                    (gx - x0) * escala + dx,
                    (gy - y0) * escala + dy,
                    gw * escala,
                    gh * escala,
                ),
                huella: m.huella,
                angulo: m.angulo,
                centro: (cx, cy),
                id: m.id,
                generacion: m.generacion,
                sucio: None,
            };
            CACHE_GRAFITO.with_borrow_mut(|c| p.grafito(Some(c), &mapa, g.opacidad, escala));
        }
    };
    for (k, orden) in vista.ordenes.iter().enumerate() {
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
                // El grosor escala con el dibujo; si no, un trazo grueso en
                // una vista pequena lo taparia entero.
                p.polilinea(&v, (grosor * escala).max(0.75), color(*c));
            }
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
                (x - x0) * escala + dx,
                (y - y0) * escala + dy,
                (tam * escala).max(4.0),
                // El suelto no se parte, tampoco en miniatura.
                if *ancho_max >= pixpin_motor2d::texto::SIN_PARTIR {
                    *ancho_max
                } else {
                    ancho_max * escala
                },
                &crate::dibujo::pintar::letra_de(familia, *negrita, *cursiva),
                color(*c),
            ),
            // El numero de una cota, girado y con halo como en el lienzo.
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
            } => p.girado(
                ((centro.x - x0) * escala + dx, (centro.y - y0) * escala + dy),
                *angulo,
                |p| {
                    p.texto_con_halo(
                        texto,
                        (x - x0) * escala + dx,
                        (y - y0) * escala + dy,
                        (tam * escala).max(4.0),
                        &crate::dibujo::pintar::letra_de(familia, false, false),
                        color(*c),
                        color(*halo),
                        grosor_halo * escala,
                    )
                },
            ),
            // El velo es de la capa viva y las imagenes incrustadas todavia
            // no tienen almacen: en una vista previa no se echan de menos.
            Orden::Velo { .. } | Orden::Imagen { .. } => {}
        }
    }
    // Lo que va despues de la ultima orden.
    pintar_grafitos_hasta(usize::MAX);
    p.soltar_recorte();
}

thread_local! {
    /// **Los bitmaps del grafito de las vistas previas**, por mapa. Del hilo
    /// del chat, que es el que pinta; se vacian con las miniaturas cuando se
    /// pierde el dispositivo (sus bitmaps ya no valdrian). Sin guardarlos, un
    /// dibujo con veinte trazos de grafito subiria veinte mapas en cada
    /// fotograma de desplazar la conversacion.
    static CACHE_GRAFITO: std::cell::RefCell<pixpin_render::CacheGrafito> =
        std::cell::RefCell::new(pixpin_render::CacheGrafito::nueva());
}

/// Abre lo que hay detras de un mensaje: su fichero, con la aplicacion que
/// le toque.
///
/// Un mensaje sin fichero (una nota) no hace nada al pincharlo, que es mejor
/// que abrir algo que el usuario no pidio. Los dibujos todavia no abren el
/// editor: escribir de vuelta el `.excalidraw` sin perder lo que el movil
/// mete y aqui no se entiende es un trabajo aparte, y a medias seria peor.
fn abrir_mensaje(
    ubicacion: &Ubicacion,
    a: &Abierto,
    indice: usize,
    textos: &Catalogo,
    idioma: pixpin_store::Idioma,
) {
    let Some(m) = a.mensajes.get(indice) else {
        return;
    };
    // Una hoja que ES una pagina del documento no llega aqui: se abre en el
    // lienzo con la pagina de fondo (`tocar_la_burbuja`), como en el movil.
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
    // Un `.bin` del movil que por dentro es un PDF o una foto se abre como
    // lo que es (`recibir::ruta_para_abrir`).
    let ruta = crate::recibir::ruta_para_abrir(&ruta);
    // Una nota de voz **suena aqui**, no se manda al reproductor de Windows:
    // es lo que hace el movil al tocar la burbuja, y lo que permite pausarla,
    // adelantarla y cambiarle la velocidad desde la barra de abajo.
    if m.clase == Some(pixpin_proyecto::cuaderno::Clase::Voz) {
        let titulo = crate::biblioteca_audio::titulo_de_audio(m);
        if let Err(e) = crate::audio::alternar(&ruta, &titulo) {
            // Un fichero que Media Foundation no sabe abrir no se queda en
            // silencio: se manda a Windows, que a lo mejor si sabe.
            tracing::warn!(?e, ruta = %ruta.display(), "no se pudo reproducir aqui");
            if let Err(e) = pixpin_shell::abrir::abrir(&ruta) {
                tracing::warn!(?e, "y tampoco se pudo abrir fuera");
            }
        }
        return;
    }
    // Un `.html` se abre DENTRO de PixPin, en el visor propio: su boton de
    // guardar escribe encima de este mismo fichero, mientras que en el
    // navegador descargaria una copia y el del chat se quedaria igual.
    if crate::visor_html::se_abre(&m.nombre) || crate::visor_html::se_abre(relativa) {
        match crate::visor_html::abrir(textos, &ruta) {
            Ok(guardado) => {
                if guardado {
                    tracing::info!(ruta = %ruta.display(), "html guardado desde el visor");
                }
                return;
            }
            // Sin runtime de WebView2 o si algo falla, el navegador de
            // siempre: mejor abrirlo fuera que no abrirlo.
            Err(e) => tracing::warn!(?e, "sin visor de HTML propio; se abre fuera"),
        }
    }
    // Un Word, un libro o un PDF se leen DENTRO, como al tocar la burbuja en
    // el movil: sin esperar a otra aplicacion y con los marcadores y lo
    // anotado a mano.
    let nombre = pixpin_docs::nombre(&ruta);
    // Un Markdown se abre en el editor de notas de PixPin, no en el programa
    // que Windows tenga para `.md` (en muchos equipos, el navegador, que lo
    // ensena como texto plano y no deja editarlo).
    if crate::notas_md::es_markdown(&ruta) {
        crate::notas_md::abrir(
            idioma,
            ubicacion.clone(),
            crate::notas_md::Destino::Fichero { ruta },
        );
        return;
    }
    if crate::lector::se_lee_al_tocar_burbuja(&ruta)
        && crate::lector::abrir_en_su_lector(idioma, ubicacion, &ruta, &nombre)
    {
        return;
    }
    if let Err(e) = pixpin_shell::abrir::abrir(&ruta) {
        tracing::warn!(?e, ruta = %ruta.display(), "no se pudo abrir");
    }
}

/// Las secciones del panel, en el orden en que se ensenan.
const SECCIONES: [pixpin_proyecto::cuaderno::Seccion; 8] =
    pixpin_proyecto::cuaderno::Seccion::TODAS;

/// El area COMPLETA del monitor donde esta la ventana (barra de tareas
/// incluida). `respaldo` es el monitor de arrancar, por si no se pueden
/// enumerar ahora.
fn monitor_de_la_ventana(marco: Rect, respaldo: &pixpin_geom::Monitor) -> Rect {
    let Ok(d) = pixpin_capture::enumerar_monitores() else {
        return respaldo.area;
    };
    d.monitores()
        .iter()
        // El que mas trozo de ventana tiene, no el primero que la toque: una
        // ventana a caballo entre dos pantallas pertenece a la que ocupa.
        .max_by_key(|m| {
            m.area
                .interseccion(marco)
                .map(|r| r.ancho as u64 * r.alto as u64)
                .unwrap_or(0)
        })
        .map(|m| m.area)
        .unwrap_or(respaldo.area)
}

/// Que mensajes ensena una seccion del panel, por su posicion.
///
/// La galeria no se puede decidir solo con el cuaderno: son las HOJAS del
/// `proyecto.json`, y eso solo se sabe aqui, donde esta `hojas_uid`. Las
/// demas secciones son las de siempre.
fn indices_de_la_seccion(a: &Abierto, seccion: pixpin_proyecto::cuaderno::Seccion) -> Vec<usize> {
    if seccion != pixpin_proyecto::cuaderno::Seccion::Galeria {
        // Sin las lecciones, como el historial (`se_ve`).
        let mut v = pixpin_proyecto::cuaderno::indices_de_seccion(&a.mensajes, seccion);
        v.retain(|&n| !crate::lecciones::almacen::es_de_leccion(&a.mensajes[n]));
        return v;
    }
    a.mensajes
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            !m.en_buzon
                && m.uid
                    .as_deref()
                    .is_some_and(|uid| a.hojas_uid.contains(uid))
        })
        .map(|(n, _)| n)
        .collect()
}

/// Donde cae el panel del proyecto, para PINTARLO y para repartir sus clics.
///
/// Una sola cuenta y no dos. Hubo dos: el pintado lo colocaba como capa
/// flotante centrada y el raton lo buscaba en la columna de la derecha, que
/// es donde vivia antes. Las pestanas se veian pero no respondian, porque su
/// rectangulo de verdad estaba en otro sitio. El usuario lo dijo asi: «los
/// otros botones como fotos archivos voz y mas no se pueden dar click».
fn disposicion_info(d: &Disposicion, escala: u32) -> pixpin_ui::info::Disposicion {
    let ventana = Rect {
        x: 0,
        y: 0,
        ancho: d.barra.ancho,
        alto: d.chat.abajo().max(d.lista.abajo()).max(0) as u32,
    };
    let (caja, _completa) = pixpin_ui::info::capa_en(ventana, escala);
    pixpin_ui::info::Disposicion::capa(caja, escala)
}

/// Que cosa del proyecto hay bajo `l`, si el panel ensena una cuadricula.
///
/// Devuelve la posicion en `a.mensajes`, que es con la que trabaja todo lo
/// demas. `None` si el panel esta en una seccion de filas, si el punto cae
/// fuera del contenido o si ahi no hay ninguna celda.
///
/// Recorre las MISMAS celdas visibles que el pintado y con la misma cuenta:
/// si cada uno hiciera la suya, un dia el clic abriria la de al lado.
fn cosa_de_la_cuadricula(a: &Abierto, l: Punto, d: &Disposicion, escala: u32) -> Option<usize> {
    let seccion = a.info?;
    if !seccion.es_cuadricula() {
        return None;
    }
    let i = disposicion_info(d, escala);
    if !i.contenido.contiene(l) {
        return None;
    }
    let mut suyos = indices_de_la_seccion(a, seccion);
    if !a.busqueda_info.trim().is_empty() {
        suyos.retain(|n| {
            let m = &a.mensajes[*n];
            pixpin_ui::resaltado::hay_coincidencia(&m.resumen(), &a.busqueda_info)
                || pixpin_ui::resaltado::hay_coincidencia(&m.nombre, &a.busqueda_info)
        });
    }
    let r = pixpin_ui::info::rejilla(i.contenido.ancho, escala);
    let (primera, cuantas) = r.visibles(i.contenido, suyos.len(), a.scroll_info, escala);
    (primera..primera + cuantas).find_map(|n| {
        let celda = r.celda(n, i.contenido, a.scroll_info, escala);
        celda.contiene(l).then(|| suyos.get(n).copied()).flatten()
    })
}

/// La tarjeta de una cosa de la galeria que no tiene vista previa que
/// dibujar: hoy, un PDF anadido al proyecto.
///
/// Icono grande centrado y, debajo, su peso. El nombre NO va aqui: lo pone
/// la chapa de extension en todas las celdas por igual, y repetirlo dejaria
/// los PDF con el nombre dos veces.
fn pintar_tarjeta_galeria(
    p: &Pintor,
    c: &Pinta,
    m: &pixpin_proyecto::cuaderno::Mensaje,
    celda: Rect,
) {
    use pixpin_proyecto::cuaderno::Clase;
    let (tema, e) = (c.tema, c.escala as f32 / 100.0);
    // Un fichero con extension se ensena como en Telegram: la chapa de color
    // con «PDF», «HTML»… Es lo unico que dice QUE es sin abrirlo, y sustituye
    // al recuadro oscuro que parecia una miniatura rota.
    let ext = chat::extension_corta(&m.nombre);
    if !ext.is_empty() {
        pintar_chapa_extension(p, c, &ext, &m.nombre, celda);
        return;
    }
    p.rellenar_redondeado(rf(celda), 4.0 * e, tema.burbuja_otra);
    // El mismo icono que lleva su fila en el chat: quien reconoce ahi una
    // nota de voz por el triangulo la tiene que reconocer aqui igual.
    let icono: &Icono = match m.clase.as_ref() {
        Some(Clase::Pagina) => &mi::MENU_BOOK,
        Some(Clase::Dibujo) => &mi::DRAW,
        Some(Clase::Voz) => &mi::PLAY_ARROW,
        Some(Clase::Proyecto) => &mi::FOLDER,
        Some(Clase::MiniApp) => icono_de_miniapp(m.miniapp.as_deref().unwrap_or_default()),
        // Una nota no es un fichero: lo que tiene que ensenar es su texto, y
        // el icono solo estorbaria. Se deja sin el.
        Some(Clase::Nota) | None => return pintar_nota_galeria(p, c, m, celda),
        _ => &mi::DESCRIPTION,
    };
    // Un tercio del lado: bastante para reconocerlo de lejos y con sitio de
    // sobra para el peso debajo y el pie del nombre.
    let lado = (celda.ancho.min(celda.alto) as f32 / 3.0).max(12.0 * e);
    p.icono(
        icono,
        RectF {
            x: celda.x as f32 + (celda.ancho as f32 - lado) / 2.0,
            y: celda.y as f32 + celda.alto as f32 / 2.0 - lado * 0.8,
            ancho: lado,
            alto: lado,
        },
        tema.apagado,
    );
    if m.bytes > 0 {
        let peso = chat::tamano_corto(m.bytes as u64);
        let (w, _) = p.medir_texto(&peso, chat::CONTADOR_TAM * e);
        p.texto(
            &peso,
            celda.x as f32 + (celda.ancho as f32 - w) / 2.0,
            celda.y as f32 + celda.alto as f32 / 2.0 + lado * 0.35,
            chat::CONTADOR_TAM * e,
            tema.apagado,
        );
    }
}

/// La chapa de un fichero sin vista previa: un rectangulo de color con su
/// extension, como en Telegram.
///
/// El color es el de su familia en la tabla del movil (v0.98.4, el mismo que
/// el icono de su fila en el chat): todos los PDF rojos, los Word azules,
/// los planos amarillos con la letra oscura. De un vistazo se distingue un
/// grupo de otro sin leer nada.
fn pintar_chapa_extension(p: &Pintor, c: &Pinta, ext: &str, nombre: &str, caja: Rect) {
    let e = c.escala as f32 / 100.0;
    let (fondo, letra) = icono_de_tipo::colores(nombre);
    p.rellenar_redondeado(rf(caja), 4.0 * e, fondo);
    // La letra, a un quinto del lado: cabe «XLSX» en la celda mas pequena y
    // se sigue leyendo en la fila de un archivo, que es mucho mas baja.
    let tam = (caja.ancho.min(caja.alto) as f32 / 5.0).clamp(9.0 * e, 34.0 * e);
    let (w, h) = p.medir_texto(ext, tam);
    p.texto(
        ext,
        caja.x as f32 + (caja.ancho as f32 - w) / 2.0,
        caja.y as f32 + (caja.alto as f32 - h) / 2.0,
        tam,
        letra,
    );
}

/// Una nota en la galeria: su texto, como un papelito pegado.
///
/// Sin icono y sin pie: en una nota el texto ES la cosa, y un nombre debajo
/// repetiria las mismas palabras.
fn pintar_nota_galeria(p: &Pintor, c: &Pinta, m: &pixpin_proyecto::cuaderno::Mensaje, celda: Rect) {
    let (tema, e) = (c.tema, c.escala as f32 / 100.0);
    p.rellenar_redondeado(rf(celda), 4.0 * e, tema.burbuja_otra);
    let margen = 8.0 * e;
    p.parrafo(
        &m.resumen(),
        celda.x as f32 + margen,
        celda.y as f32 + margen,
        chat::CONTADOR_TAM * e,
        (celda.ancho as f32 - margen * 2.0).max(0.0),
        &[],
        tema.texto,
    );
}

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

    // La MISMA cuenta que reparte los clics: si aqui se colocara aparte,
    // volverian a verse botones que no responden.
    let i = disposicion_info(d, escala);
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
    let mut suyos = indices_de_la_seccion(a, seccion);
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
            // En la galeria cada cosa lleva su nombre debajo: ahi conviven
            // lienzos, paginas y PDF, y un mosaico mudo obliga a abrirlos uno
            // a uno para saber cual es cual.
            let galeria = seccion == pixpin_proyecto::cuaderno::Seccion::Galeria;
            // Una foto de verdad, si ya esta leida.
            let foto =
                ruta_del_mensaje(&a.raiz, &a.ficha.id, m).and_then(|ruta| c.miniaturas.ya(&ruta));
            if let Some((b, w, h)) = foto {
                p.empujar_recorte(rf(celda));
                crate::miniaturas::pintar_recortado(p, b, rf(celda), w, h);
                p.soltar_recorte();
            } else {
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
                        let papel = match vista {
                            Ojeada::Lienzo(l) => papel_de_vista(l, tema.papel),
                            _ => tema.papel,
                        };
                        p.rellenar_redondeado(rf(celda), 4.0 * e, papel);
                        p.empujar_recorte(rf(celda));
                        let dentro = encoger(rf(celda), 4.0 * e);
                        match vista {
                            Ojeada::Lienzo(l) => pintar_lienzo(p, l, dentro, Some(c.previas)),
                            Ojeada::Tabla(t) => pintar_ojeada_tabla(p, tema, escala, t, dentro),
                            Ojeada::Foto { .. } => {}
                        }
                        p.soltar_recorte();
                    }
                    // Un PDF no tiene vista previa que dibujar: su tarjeta es
                    // el icono grande en medio, como la portada de un libro
                    // cerrado. El nombre lo pone el pie, abajo.
                    None if galeria => pintar_tarjeta_galeria(p, c, m, celda),
                    // Lo que no tiene miniatura: la misma tarjeta que en la
                    // galeria (chapa de extension, o icono de su clase). Antes
                    // salia un recuadro oscuro con el nombre encima, que es lo
                    // que el usuario veia como «miniatura negra».
                    None => pintar_tarjeta_galeria(p, c, m, celda),
                }
            }
            // El nombre NO va debajo: el usuario lo pidio quitar («que las
            // cosas de galeria no aparezca el nombre del archivo»). Lo que
            // hay dentro de la celda —el dibujo, la foto o la chapa con la
            // extension— ya dice lo que es.
            let _ = galeria;
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
                // Sin miniatura, la chapa con su extension (o el icono de su
                // clase): el recuadro liso de antes parecia una foto que no
                // habia cargado.
                None if m.ruta.is_some() || !m.nombre.is_empty() => {
                    pintar_tarjeta_galeria(p, c, m, f.miniatura)
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
    fn la_lista_vacia_por_la_busqueda_dice_que_no_hay_nada_y_no_que_no_hay_proyectos() {
        assert_eq!(clave_de_lista_vacia("zzz"), "chat-buscar-nada");
        // Casos negativos: sin buscar (o solo espacios, que no filtran) si
        // es que no hay ningun proyecto.
        assert_eq!(clave_de_lista_vacia(""), "chat-sin-proyectos");
        assert_eq!(clave_de_lista_vacia("   "), "chat-sin-proyectos");
    }

    #[test]
    fn la_vista_previa_ensena_el_papel_del_lienzo_tambien_el_blanco_y_el_del_tema_solo_sin_lienzo()
    {
        let leer = |json: &str| pixpin_motor2d::excalidraw::leer(json).unwrap();
        let crema = leer(
            r##"{"type":"excalidraw","elements":[],"appState":{"viewBackgroundColor":"#fdf6e3"}}"##,
        );
        let c = papel_del_lienzo(&crema).expect("el papel del lienzo se ensena");
        assert!((c.r - 253.0 / 255.0).abs() < 1e-3 && (c.b - 227.0 / 255.0).abs() < 1e-3);
        // Sin papel, con el blanco de fabrica o con uno que no se entiende:
        // blanco, como lo abre el editor, y no el gris del tema oscuro.
        for json in [
            r#"{"type":"excalidraw","elements":[]}"#,
            r##"{"type":"excalidraw","elements":[],"appState":{"viewBackgroundColor":"#ffffff"}}"##,
            r#"{"type":"excalidraw","elements":[],"appState":{"viewBackgroundColor":"transparent"}}"#,
        ] {
            let c = papel_del_lienzo(&leer(json)).expect("siempre el suyo");
            assert!(c.r > 0.999 && c.g > 0.999 && c.b > 0.999, "{json}: {c:?}");
        }
        // Caso negativo: lo que no es un lienzo (una pagina de PDF sola) va
        // sobre el papel del tema.
        let vista = LienzoVisto {
            ordenes: Vec::new(),
            grafitos: Vec::new(),
            caja: (0.0, 0.0, 1.0, 1.0),
            fondo: None,
            papel: None,
            giro_del_fondo: 0.0,
        };
        assert_eq!(papel_de_vista(&vista, TEMA_OSCURO_PAPEL), TEMA_OSCURO_PAPEL);
    }

    const TEMA_OSCURO_PAPEL: Color = Color {
        r: 0.91,
        g: 0.91,
        b: 0.91,
        a: 1.0,
    };

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
    // F5: las marcas de este lienzo viven junto a su dibujo.
    let _marcas = crate::ventana_editor::marcas::junto_a(&dibujo);
    // Las imagenes pegadas en este lienzo viven en la carpeta de al lado.
    let fotos = crate::imagenes_lienzo::fotos_junto_a(&dibujo, &escena);
    let mut pegadas = Vec::new();
    let resultado = crate::ventana_editor::abrir_con_pegadas(
        escena,
        opciones.enganche,
        opciones.nivel,
        opciones.medir_fotogramas,
        Some(fondo),
        &fotos,
        &mut pegadas,
    );
    if let Ok((escena, _)) = &resultado
        && let Err(e) = crate::imagenes_lienzo::guardar_pegadas_junto_a(&dibujo, escena, &pegadas)
    {
        tracing::error!(?e, ruta = %dibujo.display(), "no se pudieron guardar las imagenes pegadas");
    }
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
/// Dos clics en la misma letra de la caja de escribir antes de esto eligen
/// la palabra (lo mismo que la lista de proyectos).
const DOBLE_CLIC: std::time::Duration = std::time::Duration::from_millis(500);
const VK_FIN: u32 = 0x23;
const VK_INICIO: u32 = 0x24;
const VK_A: u32 = 0x41;
const VK_X: u32 = 0x58;

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
            self.valores = Default::default();
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
                valores: Default::default(),
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
    // La biblioteca no guarda nada: es una vista sobre los cuadernos. La
    // letra tampoco: se lee, no se escribe.
    a.biblioteca = None;
    a.letra = None;
    // Una grabacion a medias se tira, con su fichero. Se llega aqui al
    // volver, al cambiar de proyecto y al cerrar la ventana, que son
    // justamente los momentos en que nadie va a pulsar «parar»: guardarla a
    // escondidas dejaria una nota que el usuario no sabe que existe.
    if let Some(g) = a.grabando.take() {
        g.grabadora.cancelar();
        let _ = std::fs::remove_file(&g.temporal);
    }
    // El microfono no se queda abierto oyendo para una caja que ya no esta.
    a.dictado = None;
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
    // Los numeros con el separador decimal del usuario: en es-ES un total
    // sale «19,5», como lo que se escribe a mano.
    let decimal = pixpin_shell::entorno::separador_decimal();
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
                h.valores
                    .get_or_init(|| formula::evaluar_todo(&h.tabla))
                    .get(&pixpin_proyecto::tabla::ref_a(r))
                    .map(|v| v.mostrar(decimal))
                    .unwrap_or_default()
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
            valores: Default::default(),
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
    /// La caja de abajo (lo que se teclea y si es una linea nueva, el nombre
    /// o una fila que se corrige) y la fila marcada con las flechas.
    teclado: crate::mini_panel::Teclado,
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
    /// Donde quedo pintado el nombre del documento en la cabecera del
    /// panel. Pulsarlo lo corrige, como el `TextField` del titulo del movil
    /// (`MiniActivity.kt:136-148`); el resto de la cabecera es «volver».
    /// Se apunta al pintar porque su ancho depende de lo que mide «volver».
    titulo: std::cell::Cell<Option<Rect>>,
    /// Donde buscar las imagenes de las tareas (`![img 01](pixpin:files/…)`):
    /// la raiz y el chat del mensaje.
    raiz: std::path::PathBuf,
    proyecto: String,
    /// Cada enlace de imagen ya resuelto a su fichero (o `None`, aun no
    /// llego). Resolver lee el indice: una vez por panel abierto, no por
    /// fotograma.
    rutas_de_imagenes:
        std::cell::RefCell<std::collections::HashMap<String, Option<std::path::PathBuf>>>,
    /// Donde quedo pintada cada miniatura de una tarea, y su fichero: un clic
    /// la saca a la pantalla.
    imagenes_pintadas: std::cell::RefCell<Vec<(Rect, Option<std::path::PathBuf>)>>,
}

/// Una nota de voz grabandose ahora mismo.
///
/// Se pulsa para empezar y se vuelve a pulsar para terminar, no se mantiene
/// pulsado: en el movil se sujeta con el pulgar porque el dedo ya esta en la
/// pantalla, pero aqui tener el boton del raton apretado medio minuto deja la
/// mano agarrotada y no se puede hacer nada mas mientras tanto.
struct Grabando {
    grabadora: pixpin_audio::Grabadora,
    /// Donde escribe la grabadora. Al terminar, el fichero se mete en el
    /// proyecto por el mismo camino que cualquier adjunto y este se borra.
    temporal: std::path::PathBuf,
}

/// La clave del aviso que explica por que no se pudo empezar a grabar.
fn rotulo_de_no_grabar(e: &pixpin_audio::ErrorAudio) -> &'static str {
    match e {
        pixpin_audio::ErrorAudio::SinMicrofono => "chat-voz-sin-microfono",
        pixpin_audio::ErrorAudio::SinCodificadorAac => "chat-voz-sin-codec",
        _ => "chat-voz-fallo",
    }
}

/// **Graba una nota de voz desde fuera del chat** (el pedido «grabar» del
/// plugin de Flow Launcher): un microfono flotante, siempre encima, que se
/// para con un clic y la guarda con `nombre` en `proyecto`. Ver
/// [`microfono_flotante`].
pub(crate) fn grabar_flotante(
    textos: &Catalogo,
    raiz: std::path::PathBuf,
    proyecto: String,
    aparato: String,
    nombre: String,
) {
    microfono_flotante::lanzar(
        microfono_flotante::Pedido {
            raiz,
            proyecto,
            aparato,
            nombre,
        },
        microfono_flotante::Rotulos::de(textos),
    );
}

/// **Hace sonar un audio en el reproductor flotante** (pedido «reproducir»
/// del plugin de Flow Launcher). El del chat se para antes: dos audios a la
/// vez no se entienden. Ver [`reproductor_flotante`].
pub(crate) fn reproducir_flotante(textos: &Catalogo, ruta: std::path::PathBuf, titulo: String) {
    crate::audio::parar();
    reproductor_flotante::lanzar(
        reproductor_flotante::Pista { ruta, titulo },
        textos.t("chat-no-se-pudo"),
    );
}

/// Como [`reproducir_flotante`] con varios audios (abiertos juntos desde el
/// Explorador): suena el primero y los demas despues, en la misma ventana.
/// Cada uno es (ruta, titulo).
pub(crate) fn reproducir_flotante_en_fila(
    textos: &Catalogo,
    audios: Vec<(std::path::PathBuf, String)>,
) {
    if audios.is_empty() {
        return;
    }
    crate::audio::parar();
    reproductor_flotante::lanzar_fila(
        audios
            .into_iter()
            .map(|(ruta, titulo)| reproductor_flotante::Pista { ruta, titulo })
            .collect(),
        textos.t("chat-no-se-pudo"),
    );
}

/// **La caja de soltar de un proyecto** (pedido «soltar» y «Añadir
/// arrastrando…»): un recuadro flotante que mete en ese chat todo lo que se
/// le suelta, sin abrirlo. Ver [`caja_de_soltar`].
pub(crate) fn caja_de_soltar(
    textos: &Catalogo,
    raiz: std::path::PathBuf,
    proyecto: String,
    nombre: String,
    aparato: String,
) {
    let rotulos = caja_de_soltar::Rotulos::de(textos, &nombre);
    caja_de_soltar::lanzar(
        caja_de_soltar::Pedido {
            raiz,
            proyecto,
            nombre,
            aparato,
        },
        rotulos,
    );
}

/// Donde escribe la grabadora mientras graba en `proyecto`.
///
/// El temporal vive en la carpeta del proyecto y no en la del sistema: si
/// la aplicacion se cierra a mitad, lo que quedo esta al lado de su
/// conversacion y no perdido en `%TEMP%`.
fn temporal_de_la_voz(raiz: &std::path::Path, proyecto: &str) -> std::path::PathBuf {
    pixpin_proyecto::almacen::carpeta(raiz, proyecto).join(format!(
        "voz-{}.m4a.parcial",
        pixpin_shell::entorno::ahora_utc_ms()
    ))
}

/// Empieza a grabar, o dice por que no se puede.
///
/// El fallo se cuenta **en el acto** y no despues: que no haya microfono, o
/// que el permiso este quitado en los ajustes de Windows, es un caso normal,
/// y un boton rojo que no graba nada es peor que un aviso.
fn empezar_a_grabar(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
) -> Result<(), pixpin_audio::ErrorAudio> {
    let temporal = temporal_de_la_voz(ubicacion.raiz(), &a.ficha.id);
    let grabadora = pixpin_audio::Grabadora::empezar(&temporal)?;
    a.grabando = Some(Grabando {
        grabadora,
        temporal,
    });
    Ok(())
}

/// Para de grabar y deja la nota de voz en la conversacion.
fn terminar_de_grabar(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    aparato: &str,
) -> Result<(), pixpin_audio::ErrorAudio> {
    let Some(g) = a.grabando.take() else {
        return Ok(());
    };
    let temporal = g.temporal.clone();
    let grabacion = g.grabadora.parar()?;
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let hecho = guardar_la_voz(
        ubicacion.raiz(),
        &a.ficha.id,
        aparato,
        None,
        &grabacion,
        numero,
    );
    let _ = std::fs::remove_file(&temporal);
    let mensaje = hecho?;
    a.vistas.push(None);
    a.ficha.tocado = mensaje.cuando;
    a.ficha.resumen = mensaje.nombre.clone();
    a.mensajes.push(mensaje);
    a.colocado.borrow_mut().ancho = 0;
    Ok(())
}

/// **Mete en el proyecto una nota de voz recien grabada**: la del boton del
/// chat y la del microfono flotante, por el mismo camino, para que las dos
/// salgan identicas (sello, numero, nombre, `duracionMs` y picos).
///
/// El mensaje lleva `duracionMs` y los `picos` con el contrato del movil
/// —enteros crudos de 0 a 32767, como mucho 256—, que es lo que hace que la
/// onda se vea igual alli. Los picos van en `resto` porque el `Mensaje` de
/// aqui no declara ese campo, igual que al leerlos. El temporal de la
/// grabadora lo borra quien llama.
fn guardar_la_voz(
    raiz: &std::path::Path,
    proyecto: &str,
    aparato: &str,
    nombre: Option<&str>,
    grabacion: &pixpin_audio::Grabacion,
    numero: i64,
) -> Result<pixpin_proyecto::cuaderno::Mensaje, pixpin_audio::ErrorAudio> {
    let bytes =
        std::fs::read(&grabacion.ruta).map_err(|_| pixpin_audio::ErrorAudio::NoEsAudio {
            ruta: grabacion.ruta.display().to_string(),
        })?;
    let nombre = nombre_de_la_voz(nombre, pixpin_shell::entorno::ahora_utc_ms());
    let mut mensaje = adjuntar_en_proyecto(raiz, proyecto, aparato, &nombre, &bytes, numero)
        .map_err(|_| pixpin_audio::ErrorAudio::NoEsAudio {
            ruta: nombre.clone(),
        })?;
    mensaje.duracion_ms = grabacion.duracion_ms;
    mensaje
        .resto
        .insert("picos".into(), serde_json::json!(grabacion.picos));
    // Se reescribe la linea que acaba de escribir `adjuntar_en_proyecto`: es
    // una sola pasada mas y evita duplicar ahi el camino de la voz.
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, proyecto);
    if let Err(e) = pixpin_proyecto::cuaderno::reemplazar(&carpeta, &mensaje) {
        tracing::warn!(?e, "la nota quedo sin duracion ni picos");
    }
    Ok(mensaje)
}

/// Como se llama el fichero de una nota de voz nueva.
///
/// Con nombre puesto, ese nombre con su `.m4a`: asi se llaman igual la fila
/// del chat y el fichero que viaja al movil, y se encuentra buscando por
/// cualquiera de los dos. El `.m4a` se le pone siempre porque la clase del
/// mensaje (`clase_de_nombre`) y el reproductor van por la extension. Lo que
/// Windows no admite en un nombre lo quita despues `guardar_adjunto`. Sin
/// nombre, o con uno en blanco, la hora, como hasta ahora.
fn nombre_de_la_voz(nombre: Option<&str>, ahora: i64) -> String {
    match nombre.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => {
            let n: String = n.chars().take(80).collect();
            // Si ya lo trae, no se dobla.
            if n.to_lowercase().ends_with(".m4a") {
                n
            } else {
                format!("{n}.m4a")
            }
        }
        None => format!("voz_{ahora}.m4a"),
    }
}

/// **Arranca «Pasar a texto» sobre una nota de voz**, o dice que falta.
///
/// Lo que falta se dice **con nombre y enlace**, no con un «todavia no»: el
/// reconocedor y los modelos son descargas ajenas que PixPin no baja sola
/// (ver `pixpin_voz::idiomas`), asi que el aviso tiene que ser una
/// instruccion —este fichero, en esta carpeta— y no una disculpa.
fn empezar_a_transcribir(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    i: usize,
    textos: &Catalogo,
    idioma: pixpin_store::Idioma,
) -> Efecto {
    // Una a la vez: ver el campo `transcribiendo`. Se avisa en vez de
    // encolarla porque encolar sin ensenar la cola es perderla.
    if a.transcribiendo.as_ref().is_some_and(|t| !t.acabada()) {
        return Efecto::Aviso(textos.t("chat-transcribir-en-marcha"));
    }
    // Con Whisper (el modelo se baja solo) o el reconocedor de Windows no
    // falta nada; sin ninguno, se dice que hay que poner para Vosk.
    let whisper = crate::voz::whisper_posible(ubicacion, idioma);
    if !whisper
        && !crate::voz::windows_sabe(idioma)
        && let Some(aviso) = falta_para_transcribir(ubicacion, textos, idioma)
    {
        return Efecto::Aviso(aviso);
    }
    let Some(m) = a.mensajes.get(i) else {
        return Efecto::Nada;
    };
    let Some(ruta) = ruta_del_mensaje(&a.raiz, &a.ficha.id, m).filter(|r| r.is_file()) else {
        // El adjunto no esta en el disco: la nota llego por sincronizacion y
        // su fichero todavia no, o alguien lo borro por fuera.
        return Efecto::Aviso(textos.t("chat-voz-no-es-audio"));
    };
    // La primera vez con Whisper hay que bajar el modelo: se avisa de que
    // pesa y de que es solo esta vez, y el mismo hilo lo baja y transcribe
    // (la burbuja cuenta los dos con su barra).
    let bajar = whisper && crate::voz::falta_el_modelo(ubicacion, idioma);
    a.transcribiendo = Some(crate::voz::pasar_a_texto(
        &m.id,
        &ruta,
        ubicacion,
        idioma,
        crate::voz::turnos_de(m),
        None,
    ));
    if bajar {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("megas", pixpin_voz::whisper::MEGAS as i64);
        return Efecto::Aviso(textos.t_args("chat-voz-bajar-whisper", &args));
    }
    Efecto::Cambio
}

/// Lo que hay que instalar antes de poder transcribir, ya redactado; `None`
/// si no falta nada.
///
/// Se mira **antes** de lanzar el hilo: cargar un modelo que no existe para
/// contarlo diez segundos despues no le sirve a nadie.
pub(crate) fn falta_para_transcribir(
    ubicacion: &Ubicacion,
    textos: &Catalogo,
    idioma: pixpin_store::Idioma,
) -> Option<String> {
    use pixpin_voz::Disponibilidad;
    let donde = pixpin_voz::carpeta_de_modelos(ubicacion.raiz())
        .display()
        .to_string();
    match crate::voz::disponibilidad(ubicacion, idioma) {
        Disponibilidad::Listo(_) => None,
        Disponibilidad::SinMotor => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("motor", pixpin_voz::idiomas::NOMBRE_DEL_MOTOR);
            args.set("donde", donde);
            Some(textos.t_args("chat-voz-sin-motor", &args))
        }
        Disponibilidad::SinModelo { modelo, enlace } => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("modelo", modelo);
            args.set("enlace", enlace);
            args.set("donde", donde);
            Some(textos.t_args("chat-voz-sin-modelo", &args))
        }
        Disponibilidad::IdiomaSinModelo => Some(textos.t("chat-voz-idioma-sin-modelo")),
    }
}

/// Por que no salio la transcripcion, dicho en el idioma del usuario.
///
/// Se traduce **del enum** y no del `Display` de `ErrorVoz`: aquellas frases
/// estan en castellano sin tildes y son para el registro. Anadir una
/// variante alli deja de compilar esto, que es justo lo que hay que saber.
pub(crate) fn razon_de_voz(e: &pixpin_voz::ErrorVoz, textos: &Catalogo) -> String {
    use pixpin_voz::ErrorVoz as E;
    let arg = |clave, nombre, valor: String| {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set(nombre, valor);
        textos.t_args(clave, &args)
    };
    match e {
        E::SinMotor { donde } => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("motor", pixpin_voz::idiomas::NOMBRE_DEL_MOTOR);
            args.set("donde", donde.clone());
            textos.t_args("chat-voz-sin-motor", &args)
        }
        E::SinModelo { modelo, donde, .. } => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("modelo", modelo.clone());
            args.set(
                "enlace",
                format!("{}{modelo}.zip", pixpin_voz::idiomas::DESCARGAS),
            );
            args.set("donde", donde.clone());
            textos.t_args("chat-voz-sin-modelo", &args)
        }
        E::IdiomaSinModelo { .. } => textos.t("chat-voz-idioma-sin-modelo"),
        E::RutaImposible { donde } => arg("chat-voz-ruta-imposible", "donde", donde.clone()),
        E::ModeloIlegible { donde } => arg("chat-voz-modelo-ilegible", "donde", donde.clone()),
        E::NoEsAudio { .. } => textos.t("chat-voz-no-es-audio"),
        E::AudioVacio => textos.t("chat-voz-audio-vacio"),
        E::NoSeEntiendeNada => textos.t("chat-voz-no-se-entiende"),
        E::Cancelada => textos.t("chat-transcribir-cancelada"),
        E::SinReconocedorDeWindows { .. } => textos.t("chat-voz-sin-reconocedor"),
        E::SinMicrofono => textos.t("chat-voz-sin-microfono"),
        E::Windows { .. } => textos.t("chat-voz-fallo"),
        E::SinModeloWhisper { megas, .. } => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("megas", *megas as i64);
            textos.t_args("chat-voz-sin-modelo-whisper", &args)
        }
        E::SinOnnxRuntime { donde } => arg("chat-voz-sin-onnx", "donde", donde.clone()),
        E::Descarga { que, .. } => arg("chat-voz-descarga", "que", que.clone()),
        E::Onnx { detalle, .. } => arg("chat-voz-onnx", "detalle", detalle.clone()),
    }
}

/// Un dictado en marcha: el hilo que oye y lo que va entendiendo de la
/// frase en curso, que se pinta detras de lo ya escrito y se sustituye en
/// cada hipotesis.
struct Dictando {
    oido: pixpin_voz::Dictado,
    provisional: String,
    /// Si ya esta oyendo. Montar el dictado tarda de medio segundo a uno:
    /// hasta entonces la caja dice «preparando», no «te escucho», para que
    /// nadie hable a un microfono que todavia no oye.
    oyendo: bool,
}

/// **Empieza o termina de dictar** en la caja de escribir. Devuelve el aviso
/// que haya que ensenar.
fn alternar_dictado(
    a: &mut Abierto,
    textos: &Catalogo,
    idioma: pixpin_store::Idioma,
) -> Option<String> {
    if a.dictado.take().is_some() {
        return None;
    }
    if !crate::voz::windows_sabe(idioma) {
        return Some(textos.t("chat-voz-sin-reconocedor"));
    }
    a.dictado = Some(Dictando {
        oido: pixpin_voz::Dictado::empezar(crate::voz::idioma_de_voz(idioma)),
        provisional: String::new(),
        oyendo: false,
    });
    // Sin aviso: la propia caja dice en que esta.
    None
}

/// Recoge lo que haya dicho el dictado desde la ultima vuelta. Devuelve si
/// hay que repintar y, si fallo, por que.
fn latido_del_dictado(a: &mut Abierto, textos: &Catalogo) -> (bool, Option<String>) {
    let Some(d) = a.dictado.as_mut() else {
        return (false, None);
    };
    let mut cambio = false;
    let mut fallo = None;
    for aviso in d.oido.recoger() {
        match aviso {
            pixpin_voz::AvisoDeDictado::Oyendo => {
                d.oyendo = true;
                cambio = true;
            }
            pixpin_voz::AvisoDeDictado::Provisional(t) => {
                d.provisional = t;
                cambio = true;
            }
            pixpin_voz::AvisoDeDictado::Frase(t) => {
                a.borrador = juntar_dictado(&a.borrador, &t);
                d.provisional.clear();
                cambio = true;
            }
            pixpin_voz::AvisoDeDictado::Fallo(razon) => {
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("razon", razon);
                fallo = Some(textos.t_args("chat-dictado-fallo", &args));
            }
        }
    }
    if fallo.is_some() || d.oido.terminado() {
        a.dictado = None;
        cambio = true;
    }
    if cambio {
        // La caja puede crecer una linea: se recoloca.
        a.colocado.borrow_mut().ancho = 0;
    }
    (cambio, fallo)
}

/// Pega una frase dictada detras de lo escrito, con un espacio si hace
/// falta y la primera letra en mayuscula si empieza frase.
fn juntar_dictado(escrito: &str, frase: &str) -> String {
    let frase = frase.trim();
    if frase.is_empty() {
        return escrito.to_string();
    }
    let final_de_frase = escrito
        .trim_end()
        .chars()
        .last()
        .is_none_or(|c| matches!(c, '.' | '?' | '!' | '\n'));
    let mut letras = frase.chars();
    let primera = letras.next().unwrap_or_default();
    let frase: String = if final_de_frase {
        primera.to_uppercase().chain(letras).collect()
    } else {
        primera.to_lowercase().chain(letras).collect()
    };
    if escrito.is_empty() || escrito.ends_with([' ', '\n']) {
        format!("{escrito}{frase}")
    } else {
        format!("{escrito} {frase}")
    }
}

/// **Recoge la transcripcion cuando acaba** y la deja escrita en el cuaderno.
///
/// Se llama en cada vuelta del bucle y **no bloquea**: `recoger` solo mira
/// si el hilo ya dejo algo. Devuelve el aviso que hay que ensenar, o `None`
/// si todavia esta trabajando (o si no hay ninguna).
///
/// El texto se busca por **id** y no por la posicion que tenia al empezar:
/// una sincronizacion puede meter mensajes en medio mientras el hilo muele,
/// y el numero de antes apuntaria a otra burbuja.
fn latido_de_transcribir(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    textos: &Catalogo,
) -> Option<String> {
    let t = a.transcribiendo.as_mut()?;
    let salida = t.recoger()?;
    let id = t.id().to_string();
    a.transcribiendo = None;
    let hecha = match salida {
        Ok(hecha) => hecha,
        Err(e) => {
            tracing::info!(?e, "no salio la transcripcion");
            return Some(razon_de_voz(&e, textos));
        }
    };
    let Some(i) = a.mensajes.iter().position(|m| m.id == id) else {
        return Some(textos.t("chat-transcribir-sin-mensaje"));
    };
    let mut m = a.mensajes[i].clone();
    crate::voz::aplicar(&mut m, &hecha);
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    match pixpin_proyecto::cuaderno::reemplazar(&carpeta, &m) {
        Ok(_) => {
            a.mensajes[i] = m;
            // La burbuja crece: ahora ensena el texto bajo la onda.
            a.colocado.borrow_mut().ancho = 0;
            Some(textos.t("chat-transcribir-hecha"))
        }
        Err(e) => {
            tracing::warn!(?e, "la transcripcion no se pudo guardar");
            Some(textos.t("chat-no-se-pudo"))
        }
    }
}

/// El texto de un mensaje para leerlo en el telepronter: el de una nota
/// escrita, o el contenido de su `.txt`/`.md`.
///
/// Se lee del disco **al elegirlo** y no al abrir el menu: leer una docena
/// de ficheros para pintar una docena de rotulos seria ir al disco para
/// nada, porque solo uno se va a leer en voz alta.
fn texto_para_leer(a: &Abierto, i: usize) -> Option<String> {
    let m = a.mensajes.get(i)?;
    if m.ruta.is_none() {
        let texto = m.texto.trim();
        return (!texto.is_empty()).then(|| texto.to_string());
    }
    if !es_texto_leible(&m.nombre) {
        return None;
    }
    let ruta = ruta_del_mensaje(&a.raiz, &a.ficha.id, m)?;
    // Con `from_utf8_lossy` y no exigiendo UTF-8: un `.txt` viejo de Windows
    // viene en la pagina de codigos de siempre, y negarse a leerlo en voz
    // alta por una tilde mal codificada no le sirve a nadie.
    let bytes = std::fs::read(&ruta).ok()?;
    let texto = String::from_utf8_lossy(&bytes).trim().to_string();
    (!texto.is_empty()).then_some(texto)
}

/// **Abre el telepronter con ese texto**, en su hilo.
///
/// El `.m4a` se graba en `<datos>/voz/` (ver `voz::carpeta_de_audios`) y no
/// dentro del proyecto: el telepronter no sabe a que conversacion pertenece
/// —ni tiene por que—, y quien recoge la lectura ya lo mete donde va.
fn abrir_telepronter(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    texto: String,
    textos: &Catalogo,
    idioma: pixpin_store::Idioma,
) -> Efecto {
    if a.leyendo.is_some() {
        return Efecto::Aviso(textos.t("chat-telepronter-abierto"));
    }
    if texto.trim().is_empty() {
        return Efecto::Aviso(textos.t("chat-telepronter-sin-texto"));
    }
    let destino = crate::voz::carpeta_de_audios(ubicacion);
    if let Err(e) = std::fs::create_dir_all(&destino) {
        tracing::warn!(?e, "no se pudo preparar la carpeta de los audios");
        return Efecto::Aviso(textos.t("chat-no-se-pudo"));
    }
    a.leyendo = Some(crate::teleprompter::lanzar(idioma, texto, destino));
    Efecto::Nada
}

/// **Abre Pronunciar** (B9) con lo del chat que sirve de guia. Las tomas que
/// se guarden llegan por `a.voz` y entran como notas de voz.
fn abrir_pronunciar(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    textos: &Catalogo,
    idioma: pixpin_store::Idioma,
) -> Efecto {
    if a.voz.pronunciando() {
        return Efecto::Aviso(textos.t("pronunciar-abierto"));
    }
    let (raiz, id) = (a.raiz.clone(), a.ficha.id.clone());
    let guias = crate::pronunciar::guias_del_chat(&a.mensajes, |m| ruta_del_mensaje(&raiz, &id, m));
    let destino = crate::voz::carpeta_de_audios(ubicacion);
    a.voz
        .pronunciar(crate::pronunciar::lanzar(idioma, guias, destino));
    Efecto::Nada
}

/// **Abre la conversacion por turnos** (B10). La grabacion llega por
/// `a.voz` y entra como nota de voz con sus `turnos`.
fn abrir_conversacion(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    textos: &Catalogo,
    idioma: pixpin_store::Idioma,
) -> Efecto {
    if a.voz.conversando() {
        return Efecto::Aviso(textos.t("conversacion-abierta"));
    }
    let destino = crate::voz::carpeta_de_audios(ubicacion);
    a.voz
        .conversar(crate::conversacion::lanzar(idioma, destino));
    Efecto::Nada
}

/// **Recoge la lectura del telepronter** y la mete como nota de voz.
///
/// Se llama en cada vuelta del bucle y no bloquea. Devuelve el aviso que hay
/// que ensenar, o `None` mientras no haya nada (o si no hay telepronter).
fn latido_del_telepronter(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    aparato: &str,
    textos: &Catalogo,
) -> Option<String> {
    use std::sync::mpsc::TryRecvError;
    let lectura = match a.leyendo.as_ref()?.try_recv() {
        Ok(l) => l,
        Err(TryRecvError::Empty) => return None,
        // Se cerro sin grabar nada: ni aviso ni nota, que es lo que el
        // usuario acaba de pedir al darle a la flecha.
        Err(TryRecvError::Disconnected) => {
            a.leyendo = None;
            return None;
        }
    };
    a.leyendo = None;
    match meter_la_lectura(ubicacion, a, aparato, &lectura) {
        Ok(()) => {
            // Como el movil (`LetraActivity.abrir`): la lectura se abre en la
            // letra, para seguirla por minutos mientras suena.
            let _ = abrir_la_letra(a, a.mensajes.len().saturating_sub(1));
            Some(textos.t("chat-telepronter-hecha"))
        }
        Err(e) => {
            tracing::warn!(?e, "la lectura no se pudo meter en la conversacion");
            Some(textos.t("chat-no-se-pudo"))
        }
    }
}

/// Copia el `.m4a` de la lectura al proyecto y escribe su mensaje.
///
/// Se copia al proyecto —y no se deja donde lo grabo el telepronter— porque
/// es lo que hace que la nota **viaje por la sincronizacion**: lo que se
/// manda al movil son los adjuntos del proyecto. Es el mismo camino que
/// `terminar_de_grabar`.
fn meter_la_lectura(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    aparato: &str,
    lectura: &crate::teleprompter::Lectura,
) -> std::io::Result<()> {
    let bytes = std::fs::read(&lectura.ruta)?;
    let nombre = lectura
        .ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("lectura-{}.m4a", pixpin_shell::entorno::ahora_utc_ms()));
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let mut mensaje = adjuntar_en_proyecto(
        ubicacion.raiz(),
        &a.ficha.id,
        aparato,
        &nombre,
        &bytes,
        numero,
    )?;
    // `aplicar` pone las senas de donde grabo el telepronter; el fichero ya
    // esta copiado dentro del proyecto, asi que las buenas son las del
    // adjunto y se vuelven a poner encima. Lo demas —duracion, picos, el
    // texto con sus minutos y su estado— solo lo sabe la lectura.
    let (ruta, nombre_adjunto) = (mensaje.ruta.clone(), mensaje.nombre.clone());
    crate::teleprompter::aplicar(&mut mensaje, lectura);
    mensaje.ruta = ruta;
    mensaje.nombre = nombre_adjunto;
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    if let Err(e) = pixpin_proyecto::cuaderno::reemplazar(&carpeta, &mensaje) {
        tracing::warn!(?e, "la lectura quedo sin duracion, picos ni texto");
    }
    // Ya esta en el proyecto: el original de `<datos>/voz/` sobra, y
    // dejarlo seria guardar cada lectura dos veces.
    let _ = std::fs::remove_file(&lectura.ruta);
    a.vistas.push(None);
    a.ficha.tocado = mensaje.cuando;
    a.ficha.resumen = nombre;
    a.mensajes.push(mensaje);
    a.colocado.borrow_mut().ancho = 0;
    a.scroll = None;
    Ok(())
}

/// **«Aligerar el PDF» del menu del mensaje**: pdfsqueeze con el nivel de
/// Ajustes (y el aligerar propio de antes como plan B), en la cola de
/// `crate::aligerar`, delante de todo.
///
/// Antes se hacia aqui mismo, en el hilo de la ventana: con un escaneo
/// grande la ventana se quedaba parada segundos. Ahora se pide y se vuelve;
/// la burbuja dice «Aligerando… n %» y el aviso de como acabo, con el peso
/// nuevo ya puesto en el cuaderno, lo da `recoger_lo_aligerado`. **Nunca a
/// peor**: si no se gana lo bastante, el PDF se queda como estaba.
fn aligerar_el_pdf(ubicacion: &Ubicacion, a: &mut Abierto, i: usize, textos: &Catalogo) -> Efecto {
    let Some((ruta, id)) = a.mensajes.get(i).and_then(|m| {
        ruta_del_mensaje(&a.raiz, &a.ficha.id, m)
            .filter(|r| r.is_file())
            .map(|r| (r, m.id.clone()))
    }) else {
        return Efecto::Nada;
    };
    if !crate::aligerar::a_mano(ubicacion.raiz(), &a.ficha.id, &id, &ruta)
        && !crate::aligerar::pendiente(&ruta)
    {
        return Efecto::Aviso(textos.t("chat-no-se-pudo"));
    }
    a.colocado.borrow_mut().ancho = 0;
    Efecto::Aviso(textos.t("aligerar-empezado"))
}

// --- «Letra o texto» ----------------------------------------------------------
//
// La pantalla de `LetraActivity.kt`: el texto de una nota de voz en grande,
// un parrafo por trozo con su minuto encima, y la barra del reproductor
// debajo. Pulsar un parrafo lleva el audio a su minuto; mientras suena, el
// parrafo que se oye se resalta y la vista lo sigue. En el sitio del
// historial, como la biblioteca, y no en una ventana aparte: es de esta
// conversacion y se vuelve a ella con la flecha, la cabecera o Escape.
//
// Las banderitas (`marcas`) y el lapiz de editar la letra (B7): la bandera
// deja una donde va el audio, y el lapiz abre la caja de texto de Windows.

/// Abre la letra de esa nota y deja su audio cargado, sin sonar: los mandos
/// a mano desde el primer momento, como el movil (`LaunchedEffect(m.ruta)`).
fn abrir_la_letra(a: &mut Abierto, i: usize) -> Efecto {
    let Some(m) = a.mensajes.get(i) else {
        return Efecto::Nada;
    };
    if let Some(ruta) = ruta_del_mensaje(&a.raiz, &a.ficha.id, m).filter(|r| r.is_file())
        && crate::audio::cargado().as_deref() != Some(ruta.as_path())
        && let Err(e) =
            crate::audio::cargar(&ruta, &crate::biblioteca_audio::titulo_de_audio(m), false)
    {
        // Sin audio la letra se lee igual; solo no salta.
        tracing::warn!(?e, ruta = %ruta.display(), "la letra se abre sin su audio");
    }
    a.letra = Some(LetraAbierta {
        id: m.id.clone(),
        tam: pixpin_ui::chat::LETRA_TAM,
        scroll: 0,
        altos: std::cell::RefCell::new(Vec::new()),
        seguido: None,
    });
    Efecto::Cambio
}

/// Los parrafos que ensena la letra de una nota: sus trozos sin las marcas
/// de Markdown y sin los que se quedan vacios al quitarlas.
///
/// Una sola funcion para el pintado y para «seguir al audio»: si cada uno
/// contara los parrafos a su manera, un trozo que solo era una imagen
/// descuadraria cual se resalta y a cual se lleva la vista.
fn trozos_de_la_letra(m: &pixpin_proyecto::cuaderno::Mensaje) -> Vec<crate::voz::Trozo> {
    crate::voz::transcripcion_de(m)
        .map(crate::voz::trozos)
        .unwrap_or_default()
        .into_iter()
        .map(|t| crate::voz::Trozo {
            texto: crate::voz::sin_marcas(&t.texto),
            ..t
        })
        .filter(|t| !t.texto.is_empty())
        .collect()
}

/// El siguiente tamano de letra, de dos en dos y sin salirse de los topes
/// (`if (tamano > 14) tamano -= 2`, `if (tamano < 40) tamano += 2`).
fn otro_tamano_de_letra(tam: u32, mayor: bool) -> u32 {
    use pixpin_ui::chat::{LETRA_TAM_MAX, LETRA_TAM_MIN};
    if mayor {
        (tam + 2).min(LETRA_TAM_MAX)
    } else {
        tam.saturating_sub(2).max(LETRA_TAM_MIN)
    }
}

/// Lleva el audio de esa nota a ese milisegundo y lo deja sonando
/// (`saltarEnElAudio`). Si la nota no estaba cargada, la carga, arranca y
/// deja el salto apuntado para cuando se sepa cuanto dura: antes no hay a
/// donde saltar.
fn saltar_en_el_audio(a: &mut Abierto, i: usize, ms: i64) -> Result<(), pixpin_audio::ErrorAudio> {
    let Some(m) = a.mensajes.get(i) else {
        return Ok(());
    };
    // Sin el fichero en este equipo (llego la nota y el audio todavia no)
    // no hay a donde saltar, y se dice en vez de no hacer nada.
    let Some(ruta) = ruta_del_mensaje(&a.raiz, &a.ficha.id, m).filter(|r| r.is_file()) else {
        return Err(pixpin_audio::ErrorAudio::NoEsAudio {
            ruta: m.ruta.clone().unwrap_or_default(),
        });
    };
    if crate::audio::cargado().as_deref() != Some(ruta.as_path()) {
        crate::audio::cargar(&ruta, &crate::biblioteca_audio::titulo_de_audio(m), true)?;
        a.salto_pendiente = Some((ruta, ms));
        return Ok(());
    }
    if !ir_al_milisegundo(ms) {
        a.salto_pendiente = Some((ruta, ms));
    }
    if !crate::audio::estado().sonando {
        crate::audio::seguir()?;
    }
    Ok(())
}

/// Lleva lo cargado a `ms`, si ya se sabe cuanto dura. Devuelve si pudo.
///
/// Con un salto relativo y no con `ir_a`: aquel pide una fraccion, y pasar
/// de milisegundos a fraccion y de vuelta pierde el minuto exacto que dice
/// el parrafo.
fn ir_al_milisegundo(ms: i64) -> bool {
    let estado = crate::audio::estado();
    if estado.duracion_ms <= 0 {
        return false;
    }
    crate::audio::saltar(ms - estado.posicion_ms);
    true
}

/// Lo que hace la letra en cada vuelta del bucle, sin eventos de por medio:
/// saldar el salto que esperaba a la duracion y seguir al parrafo que suena.
/// Devuelve si hay que repintar.
fn latido_de_la_letra(a: &mut Abierto, escala: u32, d: &Disposicion) -> bool {
    let mut cambio = false;
    if let Some((ruta, ms)) = a.salto_pendiente.clone() {
        if crate::audio::cargado().as_deref() != Some(ruta.as_path()) {
            // Se cargo otra cosa entre medias: ese salto ya no es de nadie.
            a.salto_pendiente = None;
        } else if ir_al_milisegundo(ms) {
            a.salto_pendiente = None;
            cambio = true;
        }
    }
    let Some(l) = a.letra.as_ref() else {
        return cambio;
    };
    let Some(m) = a.mensajes.iter().find(|m| m.id == l.id) else {
        return cambio;
    };
    let suena = ruta_del_mensaje(&a.raiz, &a.ficha.id, m)
        .is_some_and(|r| crate::audio::cargado().as_deref() == Some(r.as_path()));
    if !suena {
        return cambio;
    }
    let trozos = trozos_de_la_letra(m);
    let Some(n) = crate::voz::por_donde_va(&trozos, crate::audio::estado().posicion_ms) else {
        return cambio;
    };
    if l.seguido == Some(n) {
        return cambio;
    }
    let caja = caja_de_la_letra(a, d, escala).texto;
    let altos = l.altos.borrow().clone();
    let nuevo = pixpin_ui::chat::scroll_para_ver(caja, &altos, n, l.scroll, escala);
    if let Some(l) = a.letra.as_mut() {
        l.scroll = nuevo;
        l.seguido = Some(n);
    }
    true
}

/// Ctrl+C y el boton de copiar: la transcripcion entera, con sus minutos.
///
/// Con los minutos y no sin ellos porque es el formato con el que se guarda
/// y viaja (`[1:23] ...`): pegada en otra nota del movil, sigue sabiendo
/// saltar; y leida por una persona, el minuto dice donde esta cada cosa.
fn copiar_la_letra(a: &Abierto, textos: &Catalogo) -> Efecto {
    let Some(texto) = a
        .letra
        .as_ref()
        .and_then(|l| a.mensajes.iter().find(|m| m.id == l.id))
        .and_then(crate::voz::transcripcion_de)
    else {
        return Efecto::Aviso(textos.t("chat-letra-vacia"));
    };
    match pixpin_codec::portapapeles::copiar_texto(texto) {
        Ok(()) => Efecto::Aviso(textos.t("chat-letra-copiada")),
        Err(e) => {
            tracing::warn!(?e, "no se pudo copiar la letra");
            Efecto::Aviso(textos.t("chat-no-se-pudo"))
        }
    }
}

/// Las cajas de la letra de la nota abierta, con la fila de banderitas si
/// tiene alguna. Una sola funcion para pintar, para la rueda y para seguir
/// al audio: si cada uno midiera a su manera, el texto de uno quedaria
/// debajo de la fila del otro.
fn caja_de_la_letra(a: &Abierto, d: &Disposicion, escala: u32) -> pixpin_ui::chat::Letra {
    let con_marcas = a
        .letra
        .as_ref()
        .and_then(|l| a.mensajes.iter().find(|m| m.id == l.id))
        .is_some_and(|m| !crate::voz::marcas_de(m).is_empty());
    pixpin_ui::chat::letra_con(hueco_hoja(d), crate::audio::hay_algo(), con_marcas, escala)
}

/// La posicion en `a.mensajes` de la nota cuya letra esta abierta.
fn indice_de_la_letra(a: &Abierto) -> Option<usize> {
    let id = &a.letra.as_ref()?.id;
    a.mensajes.iter().position(|m| &m.id == id)
}

/// Guarda en el cuaderno un mensaje cambiado y lo deja en la pantalla.
fn reescribir_mensaje(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    i: usize,
    m: pixpin_proyecto::cuaderno::Mensaje,
    textos: &Catalogo,
) -> Efecto {
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    match pixpin_proyecto::cuaderno::reemplazar(&carpeta, &m) {
        Ok(_) => {
            a.mensajes[i] = m;
            a.colocado.borrow_mut().ancho = 0;
            Efecto::Cambio
        }
        Err(e) => {
            tracing::warn!(?e, "no se pudo guardar el cambio de la nota");
            Efecto::Aviso(textos.t("chat-no-se-pudo"))
        }
    }
}

/// **La banderita** (B7): una marca donde va el audio, o al principio si no
/// suena esta nota (`if (esteAudio) estado.posicionMs else 0`). Sin audio no
/// hay a donde volver, y el boton no hace nada, como el `enabled` del movil.
fn poner_la_marca(ubicacion: &Ubicacion, a: &mut Abierto, textos: &Catalogo) -> Efecto {
    let Some(i) = indice_de_la_letra(a) else {
        return Efecto::Nada;
    };
    let mut m = a.mensajes[i].clone();
    if m.ruta.is_none() {
        return Efecto::Nada;
    }
    let suena = ruta_del_mensaje(&a.raiz, &a.ficha.id, &m)
        .is_some_and(|r| crate::audio::cargado().as_deref() == Some(r.as_path()));
    let ms = if suena {
        crate::audio::estado().posicion_ms
    } else {
        0
    };
    if !crate::voz::poner_marca(&mut m, ms) {
        return Efecto::Nada;
    }
    reescribir_mensaje(ubicacion, a, i, m, textos)
}

/// Quitar una banderita (clic derecho sobre ella).
fn quitar_la_marca(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    i: usize,
    ms: i64,
    textos: &Catalogo,
) -> Efecto {
    let Some(mut m) = a.mensajes.get(i).cloned() else {
        return Efecto::Nada;
    };
    if !crate::voz::quitar_marca(&mut m, ms) {
        return Efecto::Nada;
    }
    reescribir_mensaje(ubicacion, a, i, m, textos)
}

/// La banderita que hay bajo el raton, si la hay: `(mensaje, milisegundo)`.
fn marca_bajo(a: &Abierto, l: Punto) -> Option<(usize, i64)> {
    a.letra.as_ref()?;
    a.zonas.borrow().iter().rev().find_map(|(r, z)| match z {
        Zona::Marca(i, ms) if r.contiene(l) => Some((*i, *ms)),
        _ => None,
    })
}

/// **El lapiz** (B7): la letra en la caja de texto de Windows, para
/// corregirla o pegar la de una cancion. Lo guardado vuelve al mensaje en
/// [`latido_de_la_voz`].
fn editar_la_letra(a: &mut Abierto, textos: &Catalogo) -> Efecto {
    if a.voz.editando_letra() {
        return Efecto::Aviso(textos.t("chat-letra-editando"));
    }
    let Some(i) = indice_de_la_letra(a) else {
        return Efecto::Nada;
    };
    let m = &a.mensajes[i];
    let caja = pixpin_shell::caja_de_texto::abrir(pixpin_shell::caja_de_texto::Pedido {
        titulo: textos.t("chat-letra-editar"),
        texto: m.transcripcion.clone().unwrap_or_default(),
        guardar: textos.t("chat-letra-guardar"),
        cancelar: textos.t("chat-cancelar-caja"),
    });
    a.voz.editar_letra(m.id.clone(), caja);
    Efecto::Nada
}

/// **Lo que llega de las ventanas de la voz**: la letra corregida, una toma
/// de Pronunciar o una conversacion grabada. Una cosa por vuelta; devuelve
/// el aviso que hay que ensenar.
fn latido_de_la_voz(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    aparato: &str,
    textos: &Catalogo,
    idioma: pixpin_store::Idioma,
) -> Option<String> {
    if let Some((id, texto)) = a.voz.letra() {
        let i = a.mensajes.iter().position(|m| m.id == id)?;
        let mut m = a.mensajes[i].clone();
        if !crate::voz::escribir_letra(&mut m, &texto) {
            return None;
        }
        return match reescribir_mensaje(ubicacion, a, i, m, textos) {
            Efecto::Aviso(t) => Some(t),
            _ => Some(textos.t("chat-letra-guardada")),
        };
    }
    if let Some(toma) = a.voz.toma() {
        let practicado = toma.idioma.clone();
        return Some(
            match meter_voz(ubicacion, a, aparato, &toma.ruta, |m| {
                crate::pronunciar::aplicar(m, &toma)
            }) {
                Ok(i) => {
                    arrancar_transcripcion(ubicacion, a, i, idioma, practicado);
                    textos.t("pronunciar-guardado")
                }
                Err(e) => {
                    tracing::warn!(?e, "la toma no se pudo meter en la conversacion");
                    textos.t("chat-no-se-pudo")
                }
            },
        );
    }
    if let Some(g) = a.voz.conversacion() {
        return Some(
            match meter_voz(ubicacion, a, aparato, &g.ruta, |m| {
                crate::conversacion::aplicar(m, &g)
            }) {
                Ok(i) => {
                    arrancar_transcripcion(ubicacion, a, i, idioma, None);
                    textos.t("conversacion-transcribiendo")
                }
                Err(e) => {
                    tracing::warn!(?e, "la conversacion no se pudo meter en el chat");
                    textos.t("chat-no-se-pudo")
                }
            },
        );
    }
    None
}

/// Pasa a texto la nota `i` recien metida, si no hay otra en marcha (una a
/// la vez; si la hay, queda para «Pasar a texto» a mano).
fn arrancar_transcripcion(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    i: usize,
    idioma: pixpin_store::Idioma,
    practicado: Option<String>,
) {
    if a.transcribiendo.as_ref().is_some_and(|t| !t.acabada()) {
        return;
    }
    if !crate::voz::whisper_posible(ubicacion, idioma) && !crate::voz::windows_sabe(idioma) {
        return;
    }
    let Some(m) = a.mensajes.get(i) else {
        return;
    };
    let Some(ruta) = ruta_del_mensaje(&a.raiz, &a.ficha.id, m).filter(|r| r.is_file()) else {
        return;
    };
    a.transcribiendo = Some(crate::voz::pasar_a_texto(
        &m.id,
        &ruta,
        ubicacion,
        idioma,
        crate::voz::turnos_de(m),
        practicado,
    ));
}

/// **Mete un audio grabado aqui como nota de voz** de esta conversacion: lo
/// copia al proyecto (para que viaje por la sincronizacion, como
/// `meter_la_lectura`), le deja poner sus campos a `rellenar` y lo guarda.
/// Devuelve su posicion.
fn meter_voz(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    aparato: &str,
    ruta: &std::path::Path,
    rellenar: impl FnOnce(&mut pixpin_proyecto::cuaderno::Mensaje),
) -> std::io::Result<usize> {
    let bytes = std::fs::read(ruta)?;
    let nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("voz-{}.m4a", pixpin_shell::entorno::ahora_utc_ms()));
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let mut mensaje = adjuntar_en_proyecto(
        ubicacion.raiz(),
        &a.ficha.id,
        aparato,
        &nombre,
        &bytes,
        numero,
    )?;
    let (ruta_adjunto, nombre_adjunto) = (mensaje.ruta.clone(), mensaje.nombre.clone());
    rellenar(&mut mensaje);
    mensaje.ruta = ruta_adjunto;
    mensaje.nombre = nombre_adjunto;
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    if let Err(e) = pixpin_proyecto::cuaderno::reemplazar(&carpeta, &mensaje) {
        tracing::warn!(?e, "la nota quedo sin duracion, picos ni turnos");
    }
    // Ya esta dentro del proyecto: el original sobra.
    let _ = std::fs::remove_file(ruta);
    a.vistas.push(None);
    a.ficha.tocado = mensaje.cuando;
    a.ficha.resumen = nombre;
    a.mensajes.push(mensaje);
    a.colocado.borrow_mut().ancho = 0;
    a.scroll = None;
    Ok(a.mensajes.len() - 1)
}

/// La pantalla de la letra, en el sitio del historial. Apunta sus zonas: la
/// cabecera, cada parrafo con minuto y la barra del reproductor.
fn pintar_letra(p: &Pintor, d: &Disposicion, c: &Pinta, a: &Abierto) {
    use pixpin_ui::chat as ui;
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let Some(l) = a.letra.as_ref() else {
        return;
    };
    let t = caja_de_la_letra(a, d, escala);
    p.rellenar(rf(t.panel), tema.papel);
    let m = a.mensajes.iter().enumerate().find(|(_, m)| m.id == l.id);
    // La banderita solo con audio (`enabled = m?.ruta != null`).
    let con_audio = m.is_some_and(|(_, m)| m.ruta.is_some());
    icono_centrado(
        p,
        &mi::FLAG,
        t.marcar,
        18.0 * e,
        if con_audio { tema.texto } else { tema.apagado },
    );
    icono_centrado(p, &mi::EDIT, t.editar, 18.0 * e, tema.texto);
    {
        let mut z = a.zonas.borrow_mut();
        if con_audio {
            z.push((t.marcar, Zona::LetraMarcar));
        }
        z.push((t.editar, Zona::LetraEditar));
    }
    // Las banderitas, como fichas: «⚑ 1:23», un clic vuelve ahi.
    if let Some((i, m)) = m
        && t.marcas.alto > 0
    {
        let marcas = crate::voz::marcas_de(m);
        let tam = 13.0 * e;
        let relleno = 10.0 * e;
        let rotulos: Vec<String> = marcas
            .iter()
            .map(|ms| format!("⚑ {}", pixpin_voz::marca_de_tiempo(*ms)))
            .collect();
        let anchos: Vec<u32> = rotulos
            .iter()
            .map(|r| (p.medir_texto(r, tam).0 + 2.0 * relleno).ceil() as u32)
            .collect();
        let cajas = ui::fichas_de_marcas(t.marcas, &anchos, escala);
        for ((caja, rotulo), ms) in cajas.iter().zip(&rotulos).zip(&marcas) {
            p.rellenar_redondeado(
                rf(*caja),
                caja.alto as f32 / 2.0,
                con_alfa(tema.enviar, 0.22),
            );
            let (_, h) = p.medir_texto(rotulo, tam);
            p.texto(
                rotulo,
                caja.x as f32 + relleno,
                caja.y as f32 + (caja.alto as f32 - h) / 2.0,
                tam,
                tema.texto,
            );
            a.zonas.borrow_mut().push((*caja, Zona::Marca(i, *ms)));
        }
    }

    // La cabecera: volver, el titulo, letra menor y mayor, y copiar.
    p.rellenar(rf(t.cabecera), tema.cabecera);
    p.rellenar(
        RectF {
            x: t.cabecera.x as f32,
            y: t.cabecera.abajo() as f32 - (1.0 * e).max(1.0),
            ancho: t.cabecera.ancho as f32,
            alto: (1.0 * e).max(1.0),
        },
        tema.separador,
    );
    icono_centrado(p, &mi::ARROW_BACK, t.volver, 20.0 * e, tema.texto);
    icono_centrado(p, &mi::CONTENT_COPY, t.copiar, 18.0 * e, tema.texto);
    for (r, rotulo, tam) in [(t.menos, "A", 12.0), (t.mas, "A", 18.0)] {
        let (w, h) = p.medir_texto(rotulo, tam * e);
        p.texto(
            rotulo,
            r.x as f32 + (r.ancho as f32 - w) / 2.0,
            r.y as f32 + (r.alto as f32 - h) / 2.0,
            tam * e,
            tema.texto,
        );
    }
    let titulo = m
        .map(|(_, m)| crate::biblioteca_audio::titulo_de_audio(m))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| textos.t("chat-letra"));
    let tam_titulo = 15.0 * e;
    let (_, alto_titulo) = p.medir_texto("Ag", tam_titulo);
    p.texto_linea(
        &titulo,
        t.titulo.x as f32,
        t.titulo.y as f32 + (t.titulo.alto as f32 - alto_titulo) / 2.0,
        tam_titulo,
        t.titulo.ancho as f32,
        tema.texto,
    );
    {
        let mut z = a.zonas.borrow_mut();
        z.push((t.cabecera, Zona::LetraVolver));
        z.push((t.volver, Zona::LetraVolver));
        z.push((t.menos, Zona::LetraMenos));
        z.push((t.mas, Zona::LetraMas));
        z.push((t.copiar, Zona::LetraCopiar));
    }

    // Los parrafos. Sin texto se dice, en el centro, en vez de una pantalla
    // en blanco que parece rota (`letra_vacia`).
    let trozos = m.map(|(_, m)| trozos_de_la_letra(m)).unwrap_or_default();
    if trozos.is_empty() {
        let vacio = textos.t("chat-letra-vacia");
        let tam = 14.0 * e;
        let (w, h) = p.medir_texto(&vacio, tam);
        p.texto(
            &vacio,
            t.texto.x as f32 + (t.texto.ancho as f32 - w) / 2.0,
            t.texto.y as f32 + (t.texto.alto as f32 - h) / 2.0,
            tam,
            tema.apagado,
        );
    } else {
        let tam = l.tam as f32 * e;
        let tam_minuto = tam * 0.6;
        let ancho = ui::ancho_de_la_letra(t.texto, escala) as f32;
        let (_, alto_minuto) = p.medir_texto("0:00", tam_minuto);
        // Se mide todo en cada fotograma: son unas decenas de parrafos y
        // DirectWrite guarda las disposiciones, y asi cambiar el tamano o
        // el ancho de la ventana no deja nada viejo.
        let altos: Vec<u32> = trozos
            .iter()
            .map(|tr| {
                let (_, h) = p.medir_texto_ajustado(&tr.texto, tam, ancho.max(1.0));
                let minuto = if tr.ms.is_some() { alto_minuto } else { 0.0 };
                (h + minuto).ceil() as u32
            })
            .collect();
        let cajas = ui::parrafos_de_la_letra(t.texto, &altos, l.scroll, escala);
        *l.altos.borrow_mut() = altos;
        let actual = m.and_then(|(_, m)| {
            let ruta = ruta_del_mensaje(&a.raiz, &a.ficha.id, m)?;
            (crate::audio::cargado().as_deref() == Some(ruta.as_path()))
                .then(|| crate::voz::por_donde_va(&trozos, crate::audio::estado().posicion_ms))
                .flatten()
        });
        let x = t.texto.x as f32 + ui::LETRA_MARGEN_X as f32 * e;
        let aire = ui::LETRA_AIRE_Y as f32 * e;
        p.empujar_recorte(rf(t.texto));
        for (n, (tr, caja)) in trozos.iter().zip(&cajas).enumerate() {
            if caja.abajo() < t.texto.y || caja.y > t.texto.abajo() {
                continue;
            }
            if actual == Some(n) {
                p.rellenar(rf(*caja), con_alfa(tema.enviar, 0.10));
            }
            let mut y = caja.y as f32 + aire;
            if let Some(ms) = tr.ms {
                p.texto(
                    &pixpin_voz::marca_de_tiempo(ms),
                    x,
                    y,
                    tam_minuto,
                    tema.enviar,
                );
                y += alto_minuto;
                if let Some((i, _)) = m {
                    a.zonas.borrow_mut().push((*caja, Zona::Trozo(i, ms)));
                }
            }
            p.texto_ajustado(&tr.texto, x, y, tam, ancho + 1.0, tema.texto_papel);
        }
        p.soltar_recorte();
    }

    // La barra del reproductor, abajo, como en el movil.
    if t.barra.alto > 0 {
        pintar_barra_del_reproductor(p, t.barra, c, a);
    }
}

/// La biblioteca de audio abierta: todas las notas de voz y toda la musica,
/// de todas las conversaciones.
///
/// **No es un almacen**, igual que en el movil
/// (`BibliotecaDeAudioActivity.kt:47-56`): es una vista sobre los cuadernos.
/// Se lee entera al abrirla y no se guarda nada al cerrarla.
struct BibliotecaAbierta {
    filas: Vec<crate::biblioteca_audio::Fila>,
    /// La ruta de verdad de cada fila, ya resuelta. Se resuelve al abrir y
    /// no al pintar porque la del mensaje es relativa a la carpeta de SU
    /// conversacion, que no tiene por que ser la que esta abierta, y quien
    /// pinta no tiene la `Ubicacion` a mano. `None` es «el fichero no esta en
    /// este equipo»: la fila se ve, pero no suena.
    rutas: Vec<Option<std::path::PathBuf>>,
    /// Lo que se pinta, con las cabeceras de grupo ya intercaladas.
    lineas: Vec<LineaDeBiblioteca>,
    scroll: i32,
}

/// Una linea de la biblioteca: o el rotulo de un grupo, o un audio.
enum LineaDeBiblioteca {
    /// La clave del rotulo del grupo.
    Grupo(&'static str),
    Audio(usize),
}

/// El reparto que usa la biblioteca: una lista y nada mas.
fn reparto_de_biblioteca() -> pixpin_ui::mini::Reparto {
    pixpin_ui::mini::Reparto {
        con_tablero: false,
        filas_de_botones: 0,
        con_lista: true,
        con_anadir: false,
        con_avance: false,
    }
}

/// Lee los cuadernos de todas las conversaciones y arma la lista.
///
/// Se leen todos y no solo el abierto porque la biblioteca del movil es de
/// la aplicacion entera: una nota grabada en la obra y otra en «Mensajes
/// guardados» salen en la misma lista.
fn abrir_biblioteca(
    ubicacion: &Ubicacion,
    fichas: &[pixpin_proyecto::almacen::Ficha],
) -> BibliotecaAbierta {
    use crate::biblioteca_audio::{Grupo, del_grupo};
    let mut sueltas = Vec::new();
    for ficha in fichas {
        let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &ficha.id);
        let Ok(cuaderno) = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta) else {
            continue;
        };
        for fila in crate::biblioteca_audio::biblioteca(&cuaderno.mensajes) {
            let ruta = pixpin_proyecto::vista::ruta_real(ubicacion.raiz(), &ficha.id, &fila.ruta)
                .filter(|r| r.is_file());
            sueltas.push((fila, ruta));
        }
    }
    // `biblioteca` ya ordena dentro de cada conversacion; aqui hay varias, y
    // el orden de las dos manda: la musica primero y lo mas nuevo arriba.
    sueltas.sort_by(|(a, _), (b, _)| {
        let grupo = (a.grupo == Grupo::Notas).cmp(&(b.grupo == Grupo::Notas));
        grupo.then(b.cuando.cmp(&a.cuando))
    });
    let (filas, rutas): (Vec<_>, Vec<_>) = sueltas.into_iter().unzip();

    // Los dos rotulos solo salen cuando hay de las dos cosas: con un solo
    // grupo, la cabecera no diria nada que la lista no diga ya.
    let musica = del_grupo(&filas, Grupo::Musica).len();
    let con_grupos = musica > 0 && musica < filas.len();
    let mut lineas = Vec::new();
    let mut anterior: Option<Grupo> = None;
    for (n, f) in filas.iter().enumerate() {
        if con_grupos && anterior != Some(f.grupo) {
            lineas.push(LineaDeBiblioteca::Grupo(match f.grupo {
                Grupo::Musica => "chat-biblioteca-musica",
                Grupo::Notas => "chat-biblioteca-notas",
            }));
            anterior = Some(f.grupo);
        }
        lineas.push(LineaDeBiblioteca::Audio(n));
    }
    BibliotecaAbierta {
        filas,
        rutas,
        lineas,
        scroll: 0,
    }
}

/// La biblioteca en el sitio del historial.
fn pintar_biblioteca(p: &Pintor, d: &Disposicion, c: &Pinta, b: &BibliotecaAbierta) {
    use pixpin_ui::mini as ui;
    let (tema, escala) = (c.tema, c.escala);
    let e = escala as f32 / 100.0;
    let t = ui::Disposicion::calcular(hueco_hoja(d), escala, reparto_de_biblioteca());
    p.rellenar(rf(t.panel), tema.papel);

    let tam_titulo = ui::TITULO_TAM * e;
    let margen = ui::MARGEN as f32 * e;
    let (_, alto_titulo) = p.medir_texto("Ag", tam_titulo);
    let y_titulo = t.cabecera.y as f32 + (t.cabecera.alto as f32 - alto_titulo) / 2.0;
    let volver = c.textos.t("hoja-volver");
    let (ancho_volver, _) = p.medir_texto(&volver, tam_titulo);
    p.texto(
        &c.textos.t("chat-biblioteca-audio"),
        t.cabecera.x as f32 + margen,
        y_titulo,
        tam_titulo,
        tema.texto_papel,
    );
    p.texto(
        &volver,
        t.cabecera.derecha() as f32 - ancho_volver - margen,
        y_titulo,
        tam_titulo,
        tema.apagado,
    );

    let tam = ui::FILA_TAM * e;
    if b.lineas.is_empty() {
        let vacio = c.textos.t("chat-biblioteca-vacia");
        let (ancho, _) = p.medir_texto(&vacio, tam);
        p.texto(
            &vacio,
            t.lista.x as f32 + (t.lista.ancho as f32 - ancho) / 2.0,
            t.lista.y as f32 + 40.0 * e,
            tam,
            tema.apagado,
        );
        return;
    }

    p.empujar_recorte(rf(t.lista));
    for (n, linea) in b.lineas.iter().enumerate() {
        let caja = t.fila(n, b.scroll, escala);
        // Lo que queda fuera no se mide siquiera: una biblioteca de mil notas
        // no puede costar mil medidas de texto por fotograma.
        if caja.abajo() < t.lista.y || caja.y > t.lista.abajo() {
            continue;
        }
        let caja_f = rf(caja);
        let (_, alto) = p.medir_texto("Ag", tam);
        let y = caja_f.y + (caja_f.alto - alto) / 2.0;
        let derecha = caja_f.x + caja_f.ancho;
        match linea {
            LineaDeBiblioteca::Grupo(clave) => {
                p.texto(&c.textos.t(clave), caja_f.x, y, tam, tema.apagado);
                continue;
            }
            LineaDeBiblioteca::Audio(i) => {
                let f = &b.filas[*i];
                let aqui = b.rutas[*i].as_deref();
                let suena = aqui.is_some_and(crate::audio::suena);
                let lado = 18.0 * e;
                p.icono(
                    &mi::PLAY_ARROW,
                    RectF {
                        x: caja_f.x,
                        y: caja_f.y + (caja_f.alto - lado) / 2.0,
                        ancho: lado,
                        alto: lado,
                    },
                    if suena { tema.enviar } else { tema.apagado },
                );
                let duracion = pixpin_audio::duracion_legible(f.duracion_ms);
                let (ancho_d, _) = p.medir_texto(&duracion, tam);
                p.texto(&duracion, derecha - ancho_d, y, tam, tema.apagado);
                // En color cuando ya tiene letra o transcripcion, como el
                // movil (`BibliotecaDeAudioActivity.kt:138`); apagado cuando
                // el fichero no esta en este equipo, porque no va a sonar.
                let color = match (aqui.is_some(), f.tiene_letra) {
                    (false, _) => tema.apagado,
                    (true, true) => tema.enviar,
                    (true, false) => tema.texto_papel,
                };
                let x = caja_f.x + 26.0 * e;
                p.texto_linea(
                    &f.titulo,
                    x,
                    y,
                    tam,
                    (derecha - ancho_d - 12.0 * e - x).max(0.0),
                    color,
                );
            }
        }
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

/// Atiende un clic en la biblioteca: pulsar una fila la reproduce.
fn pulsar_biblioteca(b: &BibliotecaAbierta, l: Punto, d: &Disposicion, escala: u32) {
    let t = pixpin_ui::mini::Disposicion::calcular(hueco_hoja(d), escala, reparto_de_biblioteca());
    let Some(n) = t.cual_fila(l, b.lineas.len(), b.scroll, escala) else {
        return;
    };
    // Un rotulo de grupo no es un audio: pulsarlo no hace nada.
    let LineaDeBiblioteca::Audio(i) = b.lineas[n] else {
        return;
    };
    let Some(ruta) = b.rutas[i].as_deref() else {
        return;
    };
    if let Err(e) = crate::audio::alternar(ruta, &b.filas[i].titulo) {
        tracing::warn!(?e, ruta = %ruta.display(), "no se pudo reproducir");
    }
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
        // Con lo del teclado encima: la fila marcada y la caja que se
        // corrige. Lo mide y lo pinta todo el mundo con esta misma vista.
        crate::mini_panel::vista_con(
            &self.cual,
            &self.documento,
            &moneda_del_catalogo(textos),
            pixpin_shell::entorno::ahora_utc_ms(),
            &self.teclado,
        )
    }

    /// El fichero de la imagen `enlace` de una tarea, si esta en este equipo.
    fn ruta_de_imagen(&self, enlace: &str) -> Option<std::path::PathBuf> {
        self.rutas_de_imagenes
            .borrow_mut()
            .entry(enlace.to_string())
            .or_insert_with(|| crate::tareas::ruta_de_imagen(&self.raiz, &self.proyecto, enlace))
            .clone()
    }

    /// Las imagenes de las tareas que hay que tener cargadas para pintar.
    fn imagenes_a_cargar(&self) -> Vec<std::path::PathBuf> {
        if self.cual != pixpin_proyecto::mini::TAREAS {
            return Vec::new();
        }
        pixpin_proyecto::mini::leer_tareas(&self.documento)
            .iter()
            .flat_map(|t| {
                pixpin_proyecto::mini::imagenes_de(pixpin_proyecto::mini::partir(&t.texto).0).1
            })
            .filter_map(|e| self.ruta_de_imagen(&e))
            .collect()
    }

    /// Hace la orden y deja el documento nuevo. Devuelve si cambio algo.
    fn hacer(&mut self, orden: &crate::mini_panel::Orden, textos: &Catalogo) -> bool {
        use crate::mini_panel::Orden;
        // Girar no toca el documento; lo que cambia es a quien le toco. Para
        // quitarlo de la lista esta su propia aspa, que ya esta ahi: un
        // segundo boton para lo mismo solo esconderia el primero.
        if let Orden::Girar(azar) = orden {
            // Deja marcado a quien le toco, para que Supr lo saque.
            self.elegido = crate::mini_panel::girar(&self.documento, &mut self.teclado, *azar);
            return true;
        }
        // Esconder las hechas cambia como se mira, no el documento: nada
        // que guardar.
        if crate::mini_panel::solo_de_vista(orden, &mut self.teclado) {
            return false;
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
        teclado: Default::default(),
        scroll: 0,
        tocada: false,
        elegido: None,
        titulo: std::cell::Cell::new(None),
        raiz: a.raiz.clone(),
        proyecto: a.ficha.id.clone(),
        rutas_de_imagenes: Default::default(),
        imagenes_pintadas: Default::default(),
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
    // El temporizador y la alarma avisan a su hora: se pone, cambia o quita
    // el aviso con lo que acaba de guardarse.
    crate::mini_panel::avisar(&carpeta, &mensaje);
    // El resumen de la burbuja cambio: hay que volver a colocar.
    a.colocado.borrow_mut().ancho = 0;
    Ok(())
}

/// El boton que cae bajo el raton, si alguno y si se puede pulsar.
///
/// Se resuelve aqui y no en el bucle para que las dos mitades —lo que se
/// dibuja y lo que responde— salgan de la misma cuenta: un boton pintado en
/// un sitio y pulsable en otro es el fallo clasico de estas pantallas. La
/// lista la resuelve `mini_panel::toque_en_lista`, que es la misma del pin.
fn boton_mini(
    v: &crate::mini_panel::Vista,
    d: &pixpin_ui::mini::Disposicion,
    l: Punto,
    escala: u32,
) -> Option<crate::mini_panel::Orden> {
    for (fila, botones) in v.filas_de_botones.iter().enumerate() {
        if let Some(n) = d.cual_boton(l, fila as u32, botones.len(), escala)
            && botones[n].activo
        {
            return Some(botones[n].orden.clone());
        }
    }
    None
}

/// Atiende un clic dentro del panel: dice que hacer, y lo hace
/// [`cumplir_mini`], el mismo que las teclas. Asi un clic **guarda en el
/// acto** igual que una tecla (`MiniActivity.kt:102-110`): antes el raton
/// solo guardaba al cerrar, y un temporizador arrancado con un clic no
/// llegaba a avisar si PixPin se cerraba con el panel abierto.
fn pulsar_mini(
    a: &mut Abierto,
    l: Punto,
    d: &Disposicion,
    escala: u32,
    textos: &Catalogo,
) -> crate::mini_panel::Efecto {
    use crate::mini_panel::Efecto;
    let Some(m) = a.mini.as_mut() else {
        return Efecto::Nada;
    };
    let Some(v) = m.vista(textos) else {
        return Efecto::Nada;
    };
    let t = pixpin_ui::mini::Disposicion::calcular(hueco_hoja(d), escala, v.reparto);
    // La cabecera del panel: el nombre se corrige pulsandolo (el `TextField`
    // del titulo del movil) y el resto es «volver».
    if t.cabecera.contiene(l) {
        if m.titulo.get().is_some_and(|r| r.contiene(l)) {
            crate::mini_panel::empezar_a_corregir(
                &m.cual,
                &m.documento,
                &moneda_del_catalogo(textos),
                &mut m.teclado,
                None,
            );
            return Efecto::Repintar;
        }
        return Efecto::Cerrar;
    }
    // El azar de la ruleta lo pone `cumplir_mini`, igual que con la tecla.
    if let Some(orden) = boton_mini(&v, &t, l, escala) {
        return Efecto::Hacer(orden);
    }
    // La miniatura de una imagen de tarea: a la pantalla, como «Sacar a la
    // pantalla» (el fichero va a la ventana principal, que tiene los pines).
    let imagen = m
        .imagenes_pintadas
        .borrow()
        .iter()
        .find(|(r, _)| r.contiene(l) && t.lista.contiene(l))
        .map(|(_, ruta)| ruta.clone());
    if let Some(ruta) = imagen {
        match ruta {
            Some(r) if !pixpin_shell::mensajero::enviar_ficheros(std::slice::from_ref(&r)) => {
                tracing::warn!(ruta = %r.display(), "no contesta la ventana principal");
            }
            Some(_) => {}
            None => tracing::info!("imagen de tarea que aun no esta en este equipo"),
        }
        return Efecto::Nada;
    }
    // La lista: la casilla tacha, el aspa borra, el lapiz corrige, las
    // flechas mueven y el resto de la fila la elige para el teclado.
    match crate::mini_panel::toque_en_lista(&v, &t, m.scroll, escala, l) {
        Some(toque) => crate::mini_panel::cumplir_toque(
            &m.cual,
            &m.documento,
            &moneda_del_catalogo(textos),
            &v,
            &mut m.teclado,
            toque,
        ),
        None => Efecto::Nada,
    }
}

/// **Cumple lo que pidio una tecla** dentro del panel de la mini-app
/// (`mini_panel::tecla` y `mini_panel::caracter` deciden; aqui se hace).
/// Devuelve si hay que repintar.
///
/// Guarda a cada cambio, como el movil (`MiniActivity.kt:102-110`): antes
/// solo se guardaba al cerrar, y un temporizador puesto con el teclado no
/// llegaba a avisar si la aplicacion se cerraba con el panel abierto.
fn cumplir_mini(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    efecto: crate::mini_panel::Efecto,
    d: &Disposicion,
    escala: u32,
    textos: &Catalogo,
) -> bool {
    use crate::mini_panel::{Efecto, Orden};
    let orden = match efecto {
        Efecto::Nada => return false,
        Efecto::Repintar => None,
        Efecto::Cerrar => {
            cerrar_panel(ubicacion, a);
            return true;
        }
        // El azar lo pone quien cumple, igual que el boton (`pulsar_mini`).
        Efecto::Hacer(Orden::Girar(_)) => {
            let mut azar =
                pixpin_motor2d::azar::Azar::nuevo(pixpin_shell::entorno::ahora_utc_ms() as u32);
            Some(Orden::Girar(azar.siguiente() as f64))
        }
        Efecto::Hacer(o) => Some(o),
    };
    // `guardar_mini` pide el chat y el panel a la vez: se saca el panel.
    let Some(mut m) = a.mini.take() else {
        return true;
    };
    let mut al_final = false;
    if let Some(o) = &orden {
        al_final = matches!(o, Orden::Anadir(_));
        if m.hacer(o, textos)
            && m.tocada
            && let Err(e) = guardar_mini(ubicacion, a, &m)
        {
            tracing::warn!(?e, "no se pudo guardar la mini-app; se reintenta al cerrar");
        }
    }
    if let Some(v) = m.vista(textos) {
        let t = pixpin_ui::mini::Disposicion::calcular(hueco_hoja(d), escala, v.reparto);
        m.scroll = if al_final {
            // Al final de la lista, que es donde acaba de caer lo anadido.
            t.tope_scroll(v.lista.len(), escala)
        } else {
            crate::mini_panel::scroll_para_ver(
                &t,
                m.teclado.marcada,
                v.lista.len(),
                m.scroll,
                escala,
            )
        };
    }
    a.mini = Some(m);
    true
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
    m.imagenes_pintadas.borrow_mut().clear();

    // La cabecera: el nombre del documento y por donde se sale. Sin la caja
    // de escribir ni el historial delante, nada mas dice como volver.
    let tam_titulo = ui::TITULO_TAM * e;
    let titulo = pixpin_proyecto::mini::titulo(&m.documento);
    let (_, alto_titulo) = p.medir_texto("Ag", tam_titulo);
    let y_titulo = t.cabecera.y as f32 + (t.cabecera.alto as f32 - alto_titulo) / 2.0;
    let margen = ui::MARGEN as f32 * e;
    let volver = c.textos.t("hoja-volver");
    let (ancho_volver, _) = p.medir_texto(&volver, tam_titulo);
    // Lo que se pulsa para corregir el nombre (`pulsar_mini`): el hueco del
    // titulo entero, no solo sus letras, que un nombre corto seria un blanco
    // de dos letras.
    m.titulo.set(Some(Rect {
        x: t.cabecera.x,
        y: t.cabecera.y,
        ancho: (t.cabecera.ancho as f32 - margen - ancho_volver - 12.0 * e).max(0.0) as u32,
        alto: t.cabecera.alto,
    }));
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

    // La linea de avance de las tareas: «3 de 7 hechas» y su barrita. Es la
    // cuenta de la burbuja, aqui dentro para no tener que cerrar para verla.
    if t.avance.alto > 0
        && let Some((hechas, de)) = v.avance
    {
        let tam = ui::BOTON_TAM * e;
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("hechas", hechas as i64);
        args.set("de", de as i64);
        let rotulo = c.textos.t_args("mini-avance", &args);
        let caja = rf(t.avance);
        let (ancho, alto) = p.medir_texto(&rotulo, tam);
        p.texto(
            &rotulo,
            caja.x + margen,
            caja.y + (caja.alto - alto) / 2.0,
            tam,
            tema.apagado,
        );
        let x0 = caja.x + margen + ancho + 12.0 * e;
        let x1 = caja.x + caja.ancho - margen;
        if x1 > x0 {
            let barra = RectF {
                x: x0,
                y: caja.y + caja.alto / 2.0 - 2.0 * e,
                ancho: x1 - x0,
                alto: 4.0 * e,
            };
            p.rellenar_redondeado(barra, 2.0 * e, tema.pildora_borde);
            if de > 0 && hechas > 0 {
                let lleno = RectF {
                    ancho: barra.ancho * hechas as f32 / de as f32,
                    ..barra
                };
                p.rellenar_redondeado(lleno, 2.0 * e, tema.enviar);
            }
        }
    }

    // La lista, lo unico que se desplaza.
    if t.lista.alto > 0 {
        use crate::mini_panel::ToqueDeFila;
        let tam = ui::FILA_TAM * e;
        let tam_edad = ui::BOTON_TAM * e;
        let cuantas = v.lista.len();
        let (_, alto) = p.medir_texto("Ag", tam);
        p.empujar_recorte(rf(t.lista));
        for (n, f) in v.lista.iter().enumerate() {
            let caja = t.fila(n, m.scroll, escala);
            // Lo que queda fuera no se mide siquiera: una lista de mil
            // nombres no puede costar mil medidas de texto por fotograma.
            if caja.abajo() < t.lista.y || caja.y > t.lista.abajo() {
                continue;
            }
            let caja_f = rf(caja);
            // La fila elegida (flechas o clic): la que tacha Espacio, corrige
            // F2, borra Supr y la unica con subir y bajar.
            if f.marcada {
                p.rellenar_redondeado(caja_f, 6.0 * e, tema.pildora_borde);
            }
            let y = caja_f.y + (caja_f.alto - alto) / 2.0;
            let color = if f.hecha {
                tema.apagado
            } else {
                tema.texto_papel
            };
            // Los iconos de la derecha, del aspa hacia dentro, con la misma
            // cuenta que el clic (`mini_panel::toque_en_lista`).
            let mut derecha = caja_f.x + caja_f.ancho;
            let iconos = crate::mini_panel::iconos_de_fila(f, n, cuantas);
            let usados = iconos
                .iter()
                .rposition(Option::is_some)
                .map_or(0, |k| k + 1);
            for (k, icono) in iconos.iter().enumerate().take(usados) {
                let r = rf(t.icono_de_fila(caja, k as u32, escala));
                derecha = derecha.min(r.x);
                let glifo: &'static Icono = match icono {
                    Some(ToqueDeFila::Borrar(_)) => &mi::CLOSE,
                    Some(ToqueDeFila::Corregir(_)) => &mi::EDIT,
                    Some(ToqueDeFila::Subir(_)) => &mi::KEYBOARD_ARROW_UP,
                    Some(ToqueDeFila::Bajar(_)) => &mi::KEYBOARD_ARROW_DOWN,
                    _ => continue,
                };
                p.icono(glifo, encoger(r, 7.0 * e), tema.apagado);
            }
            // Cuantos dias lleva, nunca la fecha (lo pidio el usuario):
            // pequeno y gris, que es un dato de contexto y no la tarea.
            if let Some(dias) = f.edad {
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("dias", i64::from(dias));
                let edad = c.textos.t_args("mini-tarea-edad", &args);
                let (ancho_e, alto_e) = p.medir_texto(&edad, tam_edad);
                p.texto(
                    &edad,
                    derecha - ancho_e - 6.0 * e,
                    caja_f.y + (caja_f.alto - alto_e) / 2.0,
                    tam_edad,
                    tema.apagado,
                );
                derecha -= ancho_e + 12.0 * e;
            }
            if !f.detalle.is_empty() {
                let (ancho_d, _) = p.medir_texto(&f.detalle, tam);
                p.texto(&f.detalle, derecha - ancho_d - 6.0 * e, y, tam, color);
                derecha -= ancho_d + 12.0 * e;
            }
            // La casilla de una tarea, con su marca: lo unico que tacha.
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
                let casilla = rf(t.casilla(caja, escala));
                x = casilla.x + casilla.ancho;
            }
            // Las imagenes de la tarea, en pequeno detras del texto: el texto
            // les cede su sitio.
            let lado_mini = 28.0 * e;
            let paso = lado_mini + 4.0 * e;
            let ancho_minis = if f.imagenes.is_empty() {
                0.0
            } else {
                f.imagenes.len() as f32 * paso + 6.0 * e
            };
            let hueco = (derecha - x - ancho_minis).max(0.0);
            p.texto_linea(&f.texto, x, y, tam, hueco, color);
            let ancho_t = if f.texto.is_empty() {
                0.0
            } else {
                p.medir_texto(&f.texto, tam).0.min(hueco)
            };
            // Lo hecho se tacha, como en el movil (`TextDecoration.LineThrough`):
            // ver lo tachado dice lo que ya no hay que volver a pensar.
            if f.hecha && ancho_t > 0.0 {
                p.rellenar(
                    RectF {
                        x,
                        y: y + alto * 0.55,
                        ancho: ancho_t,
                        alto: (1.0 * e).max(1.0),
                    },
                    tema.apagado,
                );
            }
            let mut mx = x + ancho_t + if ancho_t > 0.0 { 8.0 * e } else { 0.0 };
            for enlace in &f.imagenes {
                let r = RectF {
                    x: mx,
                    y: caja_f.y + (caja_f.alto - lado_mini) / 2.0,
                    ancho: lado_mini,
                    alto: lado_mini,
                };
                let ruta = m.ruta_de_imagen(enlace);
                match ruta.as_deref().and_then(|r| c.previas.ya(r)) {
                    Some((b, iw, ih)) => crate::miniaturas::pintar_recortado(p, b, r, iw, ih),
                    // Aun no llego del movil: su hueco con el dibujo de una
                    // imagen, para que se sepa que hay una.
                    None => {
                        p.rellenar_redondeado(r, 4.0 * e, tema.pildora_borde);
                        p.icono(&mi::IMAGE, encoger(r, 5.0 * e), tema.apagado);
                    }
                }
                m.imagenes_pintadas.borrow_mut().push((
                    Rect {
                        x: r.x as i32,
                        y: r.y as i32,
                        ancho: r.ancho as u32,
                        alto: r.alto as u32,
                    },
                    ruta,
                ));
                mx += paso;
            }
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
        let (texto, color) = if m.teclado.borrador.is_empty() {
            (
                v.guia.map(|g| c.textos.t(g)).unwrap_or_default(),
                tema.apagado,
            )
        } else {
            // El cursor va pegado a lo escrito: no hay seleccion ni flechas
            // en esta caja, asi que una barra fija basta y no parpadea.
            (format!("{}|", m.teclado.borrador), tema.texto)
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

/// Lo que se eligio en el menu del clic derecho sobre un proyecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DelMenuProyecto {
    /// Se eligio «Borrar» Y se dijo que si a la pregunta.
    Borrar,
    /// Cambiarle el nombre a mano.
    Renombrar,
    /// Ver lo borrado y recuperarlo (`sincronizar::abrir_papelera`).
    Papelera,
    /// Llevarlo a una carpeta elegida (`donde_vive`).
    CambiarUbicacion,
    /// Traerlo de su carpeta elegida a la zona habitual.
    VolverAHabitual,
    /// Ensenar su carpeta en el Explorador.
    AbrirCarpeta,
    /// La hoja de compartir con el proyecto entero (o los marcados).
    Compartir,
    /// Elegir una imagen para su avatar (`logo`).
    PonerLogo,
    /// Volver al circulo con iniciales.
    QuitarLogo,
}

/// El menu del clic derecho sobre un proyecto de la lista.
///
/// «Cambiar nombre» solo con UNO marcado: con varios no hay un nombre que
/// escribir. El usuario lo pidio aqui —«para nombrar un proyecto que se haga
/// dandole clic derecho al proyecto»— porque el nombre que se ve en la lista
/// puede venir puesto por el movil o ser el codigo del proyecto, y hasta
/// ahora solo se podia cambiar entrando dentro.
///
/// Lo de la carpeta, tambien solo con uno: cada proyecto va a la suya.
/// «Volver a la ubicacion habitual» solo sale si tiene una propia.
///
/// El logo, tambien con uno solo, junto al nombre: es lo otro que se ve de
/// el en la lista. «Quitar» solo si tiene uno puesto.
fn menu_de_proyecto(
    ventana: &VentanaOverlay,
    textos: &Catalogo,
    cuantos: usize,
    con_carpeta_propia: bool,
    con_logo: bool,
) -> Option<DelMenuProyecto> {
    const BORRAR: u32 = 1;
    const RENOMBRAR: u32 = 2;
    const PAPELERA: u32 = 3;
    const CAMBIAR_UBICACION: u32 = 4;
    const VOLVER_A_HABITUAL: u32 = 5;
    const ABRIR_CARPETA: u32 = 6;
    const COMPARTIR: u32 = 7;
    const PONER_LOGO: u32 = 8;
    const QUITAR_LOGO: u32 = 9;
    let rotulo = if cuantos > 1 {
        format!("{} ({cuantos})", textos.t("proyecto-borrar-varios"))
    } else {
        textos.t("proyecto-borrar")
    };
    // Compartir, lo primero: es lo que mas se hace con un proyecto desde
    // la lista, y con varios marcados van todos (la caja de exportar unica
    // del movil).
    let mut entradas = vec![(COMPARTIR, textos.t("compartir-proyecto"))];
    if cuantos == 1 {
        entradas.push((RENOMBRAR, textos.t("proyecto-renombrar")));
        entradas.push((PONER_LOGO, textos.t("proyecto-logo-poner")));
        if con_logo {
            entradas.push((QUITAR_LOGO, textos.t("proyecto-logo-quitar")));
        }
        entradas.push((CAMBIAR_UBICACION, textos.t("ubicacion-cambiar")));
        if con_carpeta_propia {
            entradas.push((VOLVER_A_HABITUAL, textos.t("ubicacion-habitual")));
        }
        entradas.push((ABRIR_CARPETA, textos.t("ubicacion-abrir")));
    }
    entradas.push((BORRAR, rotulo.clone()));
    // Junto a «Borrar», que es donde se busca como deshacerlo.
    entradas.push((PAPELERA, textos.t("proyecto-papelera")));
    match pixpin_shell::menu_llano(ventana.handle(), &entradas) {
        Some(RENOMBRAR) => Some(DelMenuProyecto::Renombrar),
        Some(PONER_LOGO) => Some(DelMenuProyecto::PonerLogo),
        Some(QUITAR_LOGO) => Some(DelMenuProyecto::QuitarLogo),
        Some(PAPELERA) => Some(DelMenuProyecto::Papelera),
        Some(CAMBIAR_UBICACION) => Some(DelMenuProyecto::CambiarUbicacion),
        Some(VOLVER_A_HABITUAL) => Some(DelMenuProyecto::VolverAHabitual),
        Some(ABRIR_CARPETA) => Some(DelMenuProyecto::AbrirCarpeta),
        Some(COMPARTIR) => Some(DelMenuProyecto::Compartir),
        Some(BORRAR) => pixpin_shell::confirmar_destructivo(
            ventana.handle(),
            &rotulo,
            &textos.t("proyecto-borrar-aviso"),
        )
        .then_some(DelMenuProyecto::Borrar),
        _ => None,
    }
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
    // K11: lo dibujado en la burbuja pasa al lienzo de la foto, el que viaja
    // al movil (el `.pixpin2d` no viaja).
    crate::foto_anotada::asegurar_lienzo(ubicacion.raiz(), &a.ficha.id, &mut a.mensajes, v.indice);
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

/// Recoge lo que dejaron los hilos de los PDF (`pdf_en_chat`). Devuelve si
/// hay que repintar y, si toca, que decir en el aviso.
///
/// - **Paginas pintadas**: las burbujas que esperaban su pagina (la vista de
///   un PDF, una hoja del documento) se vuelven a leer. Solo esas: releer un
///   lienzo con cientos de trazos por nada seria el tiron que se evita.
/// - **Trabajos terminados**: el mensaje queda `unido` (como `unir` del
///   movil) y el proyecto se relee para que la galeria ensene las hojas.
/// - **Trabajos en marcha**: el aviso dice por que pagina van.
///
/// Lo que llega de la cola de aligerar PDF (`crate::aligerar`): el peso
/// nuevo de cada mensaje, al cuaderno y a la pantalla, y el aviso de como
/// acabo. Al entrar solo se avisa si se gano algo (como el movil); pedido a
/// mano, tambien si no. Un PDF cifrado que entra se queda como esta sin
/// decir nada: no es un error del usuario.
fn recoger_lo_aligerado(
    ubicacion: &Ubicacion,
    mut abierto: Option<&mut Abierto>,
    textos: &Catalogo,
) -> (bool, Option<String>) {
    use crate::aligerar::Desenlace;
    thread_local! {
        static VISTOS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }
    let mut repintar = false;
    let mut dicho = None;
    // El porcentaje de la burbuja es parte de lo medido: si cambio, se
    // vuelve a medir. Solo entonces, no en cada vuelta del bucle.
    let cambios = crate::aligerar::cambios();
    if VISTOS.replace(cambios) != cambios {
        repintar = true;
        if let Some(a) = abierto.as_deref_mut() {
            a.colocado.borrow_mut().ancho = 0;
        }
    }
    for t in crate::aligerar::terminados() {
        tracing::debug!(ruta = %t.ruta.display(), desenlace = ?t.desenlace, "PDF aligerado, recogido");
        match &t.desenlace {
            Desenlace::Aligerado { antes, despues } => {
                if let (Some(ficha), Some(id)) = (&t.ficha, &t.mensaje) {
                    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), ficha);
                    let cuaderno =
                        pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta).unwrap_or_default();
                    if let Some(mut m) = cuaderno.mensajes.into_iter().find(|m| m.id == *id) {
                        // El peso del mensaje tambien, o la fila seguiria
                        // diciendo los megas de antes.
                        m.bytes = *despues as i64;
                        if let Err(e) = pixpin_proyecto::cuaderno::reemplazar(&carpeta, &m) {
                            tracing::warn!(
                                ?e,
                                "el PDF bajo de peso pero el cuaderno dice el de antes"
                            );
                        }
                    }
                    if let Some(a) = abierto.as_deref_mut()
                        && a.ficha.id == *ficha
                        && let Some(m) = a.mensajes.iter_mut().find(|m| m.id == *id)
                    {
                        m.bytes = *despues as i64;
                        a.colocado.borrow_mut().ancho = 0;
                    }
                }
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("antes", pixpin_ui::chat::tamano_corto(*antes));
                args.set("despues", pixpin_ui::chat::tamano_corto(*despues));
                dicho = Some(textos.t_args("pdf-aligerado", &args));
            }
            Desenlace::SinMejora | Desenlace::YaEstaba if t.a_mano => {
                dicho = Some(textos.t("aligerar-sin-mejora"));
            }
            Desenlace::Cifrado if t.a_mano => dicho = Some(textos.t("pdf-aligerar-cifrado")),
            Desenlace::Fallo(_) if t.a_mano => dicho = Some(textos.t("pdf-aligerar-no-se-lee")),
            _ => {}
        }
        repintar = true;
    }
    (repintar, dicho)
}

fn recoger_lo_de_los_pdf(
    ubicacion: &Ubicacion,
    mut abierto: Option<&mut Abierto>,
    textos: &Catalogo,
) -> (bool, Option<String>) {
    use pixpin_proyecto::cuaderno::Clase;
    let (mut repintar, mut dicho) = recoger_lo_aligerado(ubicacion, abierto.as_deref_mut(), textos);
    if crate::pdf_en_chat::hay_paginas_nuevas()
        && let Some(a) = abierto.as_deref_mut()
    {
        for i in 0..a.mensajes.len() {
            let m = &a.mensajes[i];
            let espera = m.pagina.is_some()
                || (m.clase == Some(Clase::Archivo) && crate::pdf_en_chat::es_pdf(m));
            let sin_fondo = match a.vistas.get(i) {
                Some(Some(Ojeada::Lienzo(l))) => l.fondo.is_none(),
                Some(None) => true,
                _ => false,
            };
            if espera && sin_fondo {
                let m = m.clone();
                a.vistas[i] = leer_vista(ubicacion, &a.ficha.id, &m);
                repintar = true;
            }
        }
        if repintar {
            a.colocado.borrow_mut().ancho = 0;
        }
    }
    // Las fusiones de paginas acabadas (E9): la hoja nueva ya esta en el
    // proyecto; se relee para que salga en el chat y se dice.
    for t in crate::fusionar_paginas::terminadas() {
        repintar = true;
        dicho = Some(match &t.resultado {
            Ok(nombre) => {
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("nombre", nombre.as_str());
                textos.t_args("fusionar-paginas-hecho", &args)
            }
            Err(_) => textos.t("fusionar-paginas-no"),
        });
        if let Some(a) = abierto.as_deref_mut()
            && a.ficha.id == t.ficha
            && !releer_lo_abierto(ubicacion, a)
        {
            a.hojas_uid = pixpin_proyecto::almacen::uids_de_hojas(ubicacion.raiz(), &a.ficha.id);
        }
    }
    for t in crate::pdf_en_chat::terminados() {
        repintar = true;
        let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &t.ficha);
        let unido = match &t.resultado {
            Ok(u) => u,
            Err(e) => {
                // Un PowerPoint sin PowerPoint (o un Keynote) dice por que.
                dicho = Some(if e.starts_with("diapositivas-") {
                    textos.t(e)
                } else {
                    textos.t("unir-pdf-no")
                });
                continue;
            }
        };
        // El mensaje se marca en el cuaderno para que el menu no vuelva a
        // ofrecer unirlo (y para el punto rojo si luego se quita la hoja).
        let cuaderno = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta).unwrap_or_default();
        if let Some(mut m) = cuaderno.mensajes.into_iter().find(|m| m.id == t.mensaje) {
            m.resto
                .insert("unido".into(), serde_json::Value::Bool(true));
            if let Err(e) = pixpin_proyecto::cuaderno::reemplazar(&carpeta, &m) {
                tracing::warn!(?e, "no se pudo marcar el PDF como unido");
            }
        }
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("hojas", unido.hojas);
        args.set("paginas", unido.paginas);
        let clave = match unido.como {
            crate::pdf_en_chat::Como::Documento | crate::pdf_en_chat::Como::Pegadas => {
                "unir-pdf-hecho"
            }
            crate::pdf_en_chat::Como::Pintadas => "unir-pdf-pintadas",
            crate::pdf_en_chat::Como::Fotos => "unir-pdf-fotos",
        };
        dicho = Some(textos.t_args(clave, &args));
        if let Some(a) = abierto.as_deref_mut()
            && a.ficha.id == t.ficha
            && !releer_lo_abierto(ubicacion, a)
        {
            // Con una hoja abierta no se relee (ver `releer_lo_abierto`): al
            // menos la galeria sabe ya de las hojas nuevas.
            a.hojas_uid = pixpin_proyecto::almacen::uids_de_hojas(ubicacion.raiz(), &a.ficha.id);
        }
    }
    if dicho.is_none()
        && let Some((trabajo, hechas, total)) = crate::pdf_en_chat::progreso()
        && total > 0
    {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("hechas", hechas + 1);
        args.set("total", total);
        let clave = match trabajo {
            crate::pdf_en_chat::Trabajo::Unir => "unir-pdf-pintando",
            crate::pdf_en_chat::Trabajo::ComoImagenes => "unir-pdf-imagenes-progreso",
        };
        dicho = Some(textos.t_args(clave, &args));
        // El numero cambia: hay que ensenarlo.
        repintar = true;
    }
    (repintar, dicho)
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
    // Pulsar una zona vinculada abre su lienzo y cerrarlo vuelve a este, como
    // el movil; el camino (y a que hoja lleva cada enlace) esta en
    // `salto_por_enlace`, probado sin ventanas.
    crate::salto_por_enlace::recorrer_hojas(raiz, proyecto, mensajes, indice, |todos, i| {
        abrir_una_hoja(raiz, proyecto, todos, i, opciones)
    })
}

/// Lo que hace falta para abrir una hoja en el lienzo, ya leido.
struct HojaPreparada {
    ruta: std::path::PathBuf,
    lienzo: pixpin_motor2d::excalidraw::Lienzo,
    escena: pixpin_motor2d::Escena,
    fotos: Vec<(u64, std::path::PathBuf)>,
    fondo: Option<crate::fondo_lienzo::Fuente>,
}

/// **Todo lo de abrir una hoja que no es la ventana**: su dibujo (creandolo
/// si es una pagina que aun no tenia, como el movil), sus fotos y su fondo.
///
/// Corre en el hilo de la interfaz, asi que **no abre el PDF**: la pagina
/// la pinta el hilo del fondo (`fondo_lienzo`), y aqui solo se lee la vista
/// previa que ya pinto la galeria. Antes se abria el documento y se pintaba
/// la pagina aqui mismo, y con un PDF largo el clic se quedaba parado.
fn preparar_hoja(
    raiz: &std::path::Path,
    proyecto: &str,
    m: &pixpin_proyecto::cuaderno::Mensaje,
    ahora: i64,
) -> Option<HojaPreparada> {
    // Una pagina sin dibujo lo estrena aqui (`Proyectos.conDibujo`): la
    // hoja existia desde que se unio el PDF, y ahora tiene donde dibujar.
    let id = match m.referencia.as_deref().filter(|r| !r.is_empty()) {
        Some(r) => r.to_string(),
        None if crate::pdf_en_chat::es_pagina_sin_dibujo(m) => {
            crate::pdf_en_chat::asegurar_dibujo(raiz, proyecto, m, ahora)?
        }
        None => return None,
    };
    let ruta = pixpin_proyecto::almacen::lienzo(raiz, proyecto, &id);
    let texto = std::fs::read_to_string(&ruta)
        .inspect_err(|e| tracing::warn!(?e, ruta = %ruta.display(), "no se pudo leer el dibujo"))
        .ok()?;
    let lienzo = pixpin_motor2d::excalidraw::leer(&texto)
        .inspect_err(|e| tracing::warn!(?e, "dibujo que no se entiende"))
        .ok()?;
    // `a_escena` y no un bucle a mano: es la que fija el orden con el que
    // `con_escena` sabe luego que elemento es cual.
    let escena = pixpin_motor2d::excalidraw::a_escena(&lienzo);
    // Las fotos de la hoja viven en `imagenes/<id>` del proyecto, no dentro
    // del JSON: el movil deja ahi una ruta en vez de un `dataURL` para que un
    // plano de quince megas no se convierta en veinte de base64.
    let fotos: Vec<(u64, std::path::PathBuf)> = pixpin_motor2d::excalidraw::ficheros(&lienzo)
        .into_iter()
        .filter_map(|(id, rel)| {
            Some((id, pixpin_proyecto::vista::ruta_real(raiz, proyecto, &rel)?))
        })
        .collect();
    // Y si la hoja se dibujo sobre una pagina del PDF, esa pagina ES el
    // fondo: sin ella se ven los trazos flotando sobre el blanco. Mide lo que
    // en el movil (`ANCHO_PAPEL_PDF`), asi lo dibujado alli cae aqui en su
    // sitio.
    let fondo = m
        .pagina
        .and_then(|pagina| crate::pdf_en_chat::fondo_de_pagina(raiz, proyecto, pagina));
    tracing::info!(
        elementos = escena.cuantos_visibles(),
        ajenos = lienzo.cuantos_ajenos(),
        fotos = fotos.len(),
        pagina = ?m.pagina,
        ruta = %ruta.display(),
        "dibujo abierto"
    );
    Some(HojaPreparada {
        ruta,
        lienzo,
        escena,
        fotos,
        fondo,
    })
}

/// Abre una hoja. Devuelve si se guardo algo y a cual hay que saltar, si se
/// pulso un enlace.
fn abrir_una_hoja(
    raiz: &std::path::Path,
    proyecto: &str,
    mensajes: &[pixpin_proyecto::cuaderno::Mensaje],
    indice: usize,
    opciones: OpcionesLienzo,
) -> (bool, Option<String>) {
    let Some(m) = mensajes.get(indice) else {
        return (false, None);
    };
    let Some(HojaPreparada {
        ruta,
        lienzo,
        escena,
        fotos,
        fondo,
    }) = preparar_hoja(raiz, proyecto, m, pixpin_shell::entorno::ahora_utc_ms())
    else {
        return (false, None);
    };
    // F5: las marcas de esta hoja viven junto a su `.excalidraw`.
    let _marcas = crate::ventana_editor::marcas::junto_a(&ruta);
    // F8: el editor sabe de que hoja del proyecto es, para mandar la Zona al chat.
    let _zona = crate::zona_al_chat::en_hoja(crate::zona_al_chat::HojaAbierta::de(
        raiz, proyecto, &ruta, m,
    ));
    // Cuenta como abierto para los grupos de ventanas (H9) mientras se dibuja.
    let _grupo = crate::grupos_ventanas::apuntar(crate::grupos_ventanas::Clase::Lienzo {
        proyecto: proyecto.to_string(),
        referencia: m
            .referencia
            .clone()
            .filter(|r| !r.is_empty())
            .unwrap_or_else(|| m.codigo_unico()),
    });
    let mut pegadas = Vec::new();
    let resultado = crate::ventana_editor::abrir_sobre_con_pegadas(
        escena,
        opciones.enganche,
        opciones.nivel,
        opciones.medir_fotogramas,
        fondo,
        &fotos,
        &mut pegadas,
    );
    let (escena, destino) = match resultado {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(?e, "no se pudo abrir el lienzo");
            return (false, None);
        }
    };
    // Las imagenes pegadas, a `imagenes/` del proyecto y apuntadas en
    // `files`, como las del movil: sin esto la figura no se escribia y al
    // reabrir la hoja la imagen habia desaparecido.
    let lienzo = match crate::imagenes_lienzo::guardar_pegadas_en_hoja(
        &pixpin_proyecto::almacen::carpeta(raiz, proyecto),
        &lienzo,
        &escena,
        &pegadas,
        pixpin_shell::entorno::ahora_utc_ms(),
    ) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(?e, "no se pudieron guardar las imagenes pegadas en la hoja");
            lienzo
        }
    };
    let guardada = guardar_hoja_dibujada(&ruta, &lienzo, &escena);
    // Adonde lleva el enlace pulsado, si se pulso: lo resuelve `abrir_hojas`.
    (guardada, destino)
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

/// **Para las pruebas de otros modulos** (`anotador_al_chat`): abre una hoja
/// como `abrir_una_hoja` pero sin ventana (la escena y las fotos que recibe
/// el editor), deja que `editar` haga lo que haria el usuario y la guarda
/// como al cerrar. Devuelve la escena tal como se abrio, sus fotos y si se
/// guardo algo.
#[cfg(test)]
#[allow(clippy::type_complexity)]
pub(crate) fn hoja_abierta_y_guardada(
    raiz: &std::path::Path,
    proyecto: &str,
    m: &pixpin_proyecto::cuaderno::Mensaje,
    editar: impl FnOnce(&mut pixpin_motor2d::Escena),
) -> Option<(pixpin_motor2d::Escena, Vec<(u64, std::path::PathBuf)>, bool)> {
    let h = preparar_hoja(raiz, proyecto, m, 0)?;
    let abierta = h.escena.clone();
    let mut escena = h.escena;
    editar(&mut escena);
    let guardada = guardar_hoja_dibujada(&h.ruta, &h.lienzo, &escena);
    Some((abierta, h.fotos, guardada))
}

#[cfg(test)]
mod pruebas_imagen_pegada {
    //! «Si pego una imagen en el canvas no se guarda: al cerrarlo, la imagen
    //! desaparece al volver a abrirlo» (2-oct-2026). El camino entero de una
    //! hoja de proyecto, sin ventana: abrir, pegar, guardar como al cerrar
    //! (`abrir_una_hoja`) y volver a abrir.
    use super::{guardar_hoja_dibujada, preparar_hoja};
    use crate::imagenes_lienzo::{self as img, ImagenesLienzo};
    use pixpin_codec::ImagenRgba;
    use pixpin_motor2d::Figura;
    use pixpin_proyecto::almacen;

    fn raiz(etiqueta: &str) -> std::path::PathBuf {
        let r =
            std::env::temp_dir().join(format!("pixpin-pegada-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    /// Una imagen de colores distintos por pixel: si vuelve otra, se nota.
    fn foto(ancho: u32, alto: u32, tono: u8) -> ImagenRgba {
        let mut pixeles = Vec::with_capacity((ancho * alto * 4) as usize);
        for y in 0..alto {
            for x in 0..ancho {
                pixeles.extend_from_slice(&[(x * 7) as u8 ^ tono, (y * 11) as u8, tono, 255]);
            }
        }
        ImagenRgba {
            ancho,
            alto,
            pixeles,
        }
    }

    #[test]
    fn una_imagen_pegada_y_un_fichero_pegado_siguen_ahi_al_reabrir_la_hoja() {
        let r = raiz("hoja");
        let ruta = almacen::lienzo(&r, "p1", "h1");
        std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
        std::fs::write(
            &ruta,
            r#"{"type":"excalidraw","version":2,"elements":[],"files":{}}"#,
        )
        .unwrap();
        let m = pixpin_proyecto::cuaderno::Mensaje {
            referencia: Some("h1".into()),
            ..Default::default()
        };
        let h = preparar_hoja(&r, "p1", &m, 0).expect("la hoja se abre");
        let mut escena = h.escena;

        // Los dos caminos del Ctrl+V: pixeles del portapapeles, y un fichero
        // copiado en el Explorador (llega como ruta).
        let mapa = foto(40, 30, 0x55);
        let copiado = r.join("copiado.png");
        pixpin_codec::imagen::guardar(
            &foto(24, 16, 0xa0),
            &copiado,
            pixpin_codec::FormatoImagen::Png,
        )
        .unwrap();
        let del_fichero = match img::decidir_pegado(
            false,
            Some(pixpin_codec::ContenidoPortapapeles::Rutas(vec![
                r.join("notas.txt"),
                copiado.clone(),
            ])),
        ) {
            img::Pegado::Imagen(i) => i,
            otro => panic!("un .png copiado se pega como imagen: {otro:?}"),
        };
        let mut almacen_sesion = ImagenesLienzo::nuevo(4096);
        let a = almacen_sesion.guardar(mapa.clone()).unwrap();
        let b = almacen_sesion.guardar(del_fichero.clone()).unwrap();
        // Y una tercera que se pega y se borra: esa no se escribe.
        let c = almacen_sesion.guardar(foto(8, 8, 1)).unwrap();
        escena.anadir(img::elemento_imagen(a, 10.0, 20.0, 40.0, 30.0));
        escena.anadir(img::elemento_imagen(b, 100.0, 20.0, 24.0, 16.0));
        let borrada = escena.anadir(img::elemento_imagen(c, 0.0, 0.0, 8.0, 8.0));
        escena.borrar_apuntando(borrada);
        let pegadas = almacen_sesion.tomar_nuevas();
        assert_eq!(pegadas.len(), 3);

        // Lo que hace `abrir_una_hoja` al cerrar el editor.
        let lienzo = img::guardar_pegadas_en_hoja(
            &almacen::carpeta(&r, "p1"),
            &h.lienzo,
            &escena,
            &pegadas,
            5,
        )
        .unwrap();
        assert!(
            guardar_hoja_dibujada(&ruta, &lienzo, &escena),
            "habia algo que guardar"
        );

        // Reabrir: las dos figuras, con sus fotos, y nada de la borrada.
        let h2 = preparar_hoja(&r, "p1", &m, 0).expect("se reabre");
        let ids: Vec<u64> = h2
            .escena
            .visibles()
            .filter_map(|e| match e.figura {
                Figura::Imagen { id_objeto } => Some(id_objeto),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec![a, b], "las dos imagenes siguen en la hoja");
        assert_eq!(
            h2.fotos.len(),
            2,
            "y las dos tienen su fichero: {:?}",
            h2.fotos
        );
        for (id, original) in [(a, &mapa), (b, &del_fichero)] {
            let (_, ruta_foto) = h2.fotos.iter().find(|(i, _)| *i == id).expect("su foto");
            assert_eq!(
                ruta_foto,
                &almacen::carpeta(&r, "p1")
                    .join("imagenes")
                    .join(img::id_de_fichero(id))
            );
            let leida = pixpin_codec::imagen::cargar(ruta_foto).unwrap();
            assert_eq!((leida.ancho, leida.alto), (original.ancho, original.alto));
            assert_eq!(leida.pixeles, original.pixeles, "los mismos pixeles");
            // Y el editor la encuentra con el id de su figura.
            let mut al_abrir = ImagenesLienzo::nuevo(4096);
            assert!(al_abrir.guardar_con_id(id, leida));
        }
        assert!(
            !almacen::carpeta(&r, "p1")
                .join("imagenes")
                .join(img::id_de_fichero(c))
                .exists(),
            "la pegada y borrada no se escribe"
        );
        // Abrir y cerrar sin tocar nada no reescribe la hoja.
        assert!(!guardar_hoja_dibujada(&ruta, &h2.lienzo, &h2.escena));
        let _ = std::fs::remove_dir_all(&r);
    }
}

/// La vista de una pagina del documento del proyecto, o `None` mientras se
/// pinta. Devuelve la ruta del PNG y lo que mide la pagina en unidades de
/// lienzo (`ANCHO_PAPEL_PDF`, 1400 de ancho, el del movil), que es donde
/// caen los trazos dibujados encima.
///
/// **Aqui no se pinta nada**: se pide a `pdf_en_chat`, que la pinta en su
/// hilo a tamano de vista previa y avisa al llegar. Pintarla aqui, a 1600 px
/// y una por burbuja, era lo que paraba la ventana al abrir un proyecto con
/// un PDF largo.
///
/// Un proyecto de antes con sus `archivos/pagina-NN.png` ya pintadas las
/// sigue usando tal cual: no se repinta lo que ya esta.
fn pagina_del_pdf(
    raiz: &std::path::Path,
    proyecto: &str,
    pagina: u32,
) -> Option<(std::path::PathBuf, f32, f32)> {
    let vieja = pixpin_proyecto::almacen::carpeta(raiz, proyecto)
        .join("archivos")
        .join(format!("pagina-{:02}.png", pagina + 1));
    if vieja.is_file() {
        let (w, h) = pixpin_codec::imagen::medidas(&vieja).ok()?;
        // Esas se abren en el lienzo tal cual, un pixel por unidad
        // (`pdf_en_chat::fondo_de_pagina`): la vista tiene que medir igual.
        let unidades = w as f32;
        return Some((vieja, unidades, unidades * h as f32 / w.max(1) as f32));
    }
    let pdf = crate::pdf_en_chat::documento_de(raiz, proyecto)?;
    crate::pdf_en_chat::pagina_pintada(raiz, &pdf, pagina, crate::pdf_en_chat::ANCHO_VISTA)
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
    /// Sacar a la pantalla: la pastilla de la fila o la esquina de la foto.
    Abrir(usize),
    /// La tarjeta «Viene de»: lleva al lienzo de origen.
    VieneDe(usize),
    /// La tarjeta del enlace de una nota: lo abre fuera.
    Enlace(usize),
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
    /// Compartir lo elegido que tenga fichero, en el panel de Windows.
    SelCompartir,
    // La barra del reproductor, encima de la caja de escribir.
    VozTocar,
    VozAtras,
    VozAdelante,
    VozVelocidad,
    VozCerrar,
    /// Llevar la pista a esa fraccion, de 0 a 1. En milesimas y no en `f32`
    /// porque `Zona` se compara por igualdad y un flotante no se compara.
    VozIrA(u32),
    /// El boton de texto de una nota de voz, junto a la onda
    /// (`BotonDeTexto`): sin texto lo pide; con texto lo despliega y lo
    /// pliega.
    Texto(usize),
    /// Un trozo de la transcripcion de esa nota: lleva el audio a ese
    /// milisegundo y lo deja sonando. Lo usan la burbuja desplegada y la
    /// pantalla de la letra.
    Trozo(usize, i64),
    // La cabecera de la pantalla de la letra.
    LetraVolver,
    LetraMenos,
    LetraMas,
    LetraCopiar,
    /// La banderita y el lapiz de la letra (B7).
    LetraMarcar,
    LetraEditar,
    /// Una banderita de esa nota: clic, el audio a ese milisegundo; clic
    /// derecho, se quita (mantener pulsado en el movil).
    Marca(usize, i64),
    /// El aspa de la barra de teclear la hora de un recordatorio.
    CerrarHora,
    /// El aspa de la barra de teclear quien llama.
    CerrarQuien,
    /// El aspa de la barra de cambiar el nombre de un archivo.
    CerrarNombre,
    /// El lapiz al lado del nombre de un archivo.
    Renombrar(usize),
    /// La franja del aviso de lecciones: abre las de este proyecto.
    Lecciones,
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
    /// El nombre por el que se elige el icono de su tipo, si es un archivo
    /// (`icono_de_tipo`); `None` lleva el boton redondo.
    archivo: Option<String>,
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

/// Si la burbuja ya lleva su boton de sacar a la pantalla: la esquina de la
/// foto o la pastilla de la fila (`tieneBotonDePinear`,
/// `MensajesActivity.kt:5689`). Una nota, una nota de voz —cuya pastilla es
/// la de pasar a texto— y una mini-app no lo llevan, y a ellas el menu se
/// lo ofrece. Una clase que aqui no se conoce tampoco: sin fila no hay
/// pastilla (`fila_de`), y quitarle la entrada la dejaria sin camino.
#[cfg_attr(not(test), allow(dead_code))]
fn tiene_boton_de_pinear(m: &pixpin_proyecto::cuaderno::Mensaje) -> bool {
    use pixpin_proyecto::cuaderno::Clase;
    !matches!(
        m.clase,
        None | Some(Clase::Nota) | Some(Clase::Voz) | Some(Clase::MiniApp) | Some(Clase::Otra(_))
    )
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
    /// El texto de una nota de voz bajo su onda, si hay que ensenarlo.
    voz: Option<TextoDeVoz>,
    /// «Recibido de …», ya compuesto, y lo alto que se pinta.
    recibido: Option<(String, f32)>,
    /// La tarjeta del primer enlace de una nota, y lo alta que es.
    enlace: Option<(tarjetas::Enlace, f32)>,
    /// El pie del buzon: «24 sep → 1 oct», las dos fechas por separado
    /// porque la de irse va en rojo.
    buzon: Option<(String, String)>,
}

/// La letra de «Recibido de …» y de la tarjeta del enlace: 11 y 13, las del
/// movil (`fontSize = 11.sp`, `13.sp`).
const RECIBIDO_TAM: f32 = 11.0;
const ENLACE_TAM: f32 = 13.0;
/// El relleno de la tarjeta del enlace (`padding(start = 10, top = 6, …)`),
/// su barra de 3 y el icono de 18 con 8 de aire.
const ENLACE_AIRE_Y: f32 = 6.0;
const ENLACE_IZQUIERDA: f32 = 3.0 + 10.0 + 18.0 + 8.0;
const ENLACE_DERECHA: f32 = 8.0;

/// El texto de una nota de voz bajo su fila, ya medido (`LaTranscripcion`
/// del movil): un punto del color de como salio y, al lado, lo que se dijo.
///
/// Plegado es un solo parrafo con todo seguido y cortado a dos renglones:
/// deja saber de que iba sin abrirlo. Desplegado es un parrafo por trozo, y
/// pulsar uno lleva el audio a su minuto.
struct TextoDeVoz {
    /// Cada parrafo: su minuto (si lo tiene), lo que dice y lo alto que se
    /// pinta. En el plegado el alto ya viene cortado a dos renglones.
    parrafos: Vec<(Option<i64>, String, f32)>,
    /// El color del punto (y del texto, si no salio nada).
    color: Color,
    /// «No se pudo pasar a texto»: una linea en rojo y nada que pulsar.
    fallo: bool,
    desplegado: bool,
    ancho: f32,
    alto: f32,
}

/// Como mucho lo que ocupa el texto de una nota de voz (`widthIn(max =
/// 320.dp)`): mas ancho, los renglones se leen mal.
const VOZ_TEXTO_ANCHO: f32 = 320.0;
/// La letra del texto de una nota de voz (13 sp).
const VOZ_TEXTO_TAM: f32 = 13.0;
/// El punto de color y su hueco hasta el texto (8 dp y 7 dp).
const VOZ_PUNTO: f32 = 8.0;
const VOZ_PUNTO_HUECO: f32 = 7.0;
/// Lo apagado que va lo que todavia no se ha dicho mientras suena
/// (`ALFA_DE_LO_NO_DICHO`).
const VOZ_NO_DICHO: f32 = 0.45;

/// Los colores del punto, los del movil (`VERDE_DEL_TEXTO` y demas).
fn color_del_estado(estado: Option<pixpin_voz::EstadoDelTexto>) -> Color {
    use pixpin_voz::EstadoDelTexto as E;
    match estado {
        // Sin estado pero con texto es una nota de una version vieja: el
        // texto esta, y no hay razon para pintarlo como sospechoso.
        None | Some(E::Bien) | Some(E::Letra) => hex(0x43A047),
        Some(E::Aviso) => hex(0xFB8C00),
        Some(E::Mal) => hex(0xE53935),
    }
}

/// Mide el texto de una nota de voz, o `None` si no hay nada que ensenar:
/// no es una nota de voz, o todavia no se ha pasado a texto (entonces lo
/// que hay es el boton de la fila, que lo pide).
fn medir_texto_de_voz(
    p: &Pintor,
    m: &pixpin_proyecto::cuaderno::Mensaje,
    desplegado: bool,
    ancho_max: f32,
    textos: &Catalogo,
    e: f32,
) -> Option<TextoDeVoz> {
    if m.clase != Some(pixpin_proyecto::cuaderno::Clase::Voz) {
        return None;
    }
    let estado = crate::voz::estado_del_texto(m);
    let tam = VOZ_TEXTO_TAM * e;
    let ancho = ancho_max.min(VOZ_TEXTO_ANCHO * e);
    let ancho_texto = (ancho - (VOZ_PUNTO + VOZ_PUNTO_HUECO) * e).max(1.0);
    let Some(texto) = crate::voz::transcripcion_de(m) else {
        // Sin texto pero con estado, el reconocedor ya lo intento y no
        // salio nada: se dice, porque si no el boton parece no haber hecho
        // nada. Una letra vacia no es un fallo: es musica sin letra aun.
        if estado.is_none() || estado == Some(pixpin_voz::EstadoDelTexto::Letra) {
            return None;
        }
        let aviso = textos.t("chat-transcripcion-no");
        let (w, h) = p.medir_texto_ajustado(&aviso, tam, ancho_texto);
        return Some(TextoDeVoz {
            parrafos: vec![(None, aviso, h)],
            color: color_del_estado(Some(pixpin_voz::EstadoDelTexto::Mal)),
            fallo: true,
            desplegado: false,
            ancho: w + (VOZ_PUNTO + VOZ_PUNTO_HUECO) * e,
            alto: h,
        });
    };
    let trozos = crate::voz::trozos(texto);
    let parrafos: Vec<(Option<i64>, String)> = if desplegado {
        trozos.into_iter().map(|t| (t.ms, t.texto)).collect()
    } else {
        let todo = trozos
            .iter()
            .map(|t| t.texto.replace('\n', " "))
            .collect::<Vec<_>>()
            .join(" ");
        vec![(None, todo)]
    };
    // Dos renglones como mucho al plegar: lo que mide una linea sale de la
    // misma fuente, y el resto se corta al pintar con un recorte.
    let (_, renglon) = p.medir_texto("Ag", tam);
    let (mut ancho_usado, mut alto) = (0.0f32, 0.0f32);
    let mut medidos = Vec::with_capacity(parrafos.len());
    for (ms, t) in parrafos {
        let (w, h) = p.medir_texto_ajustado(&t, tam, ancho_texto);
        let h = if desplegado { h } else { h.min(2.0 * renglon) };
        ancho_usado = ancho_usado.max(w);
        // Un pelo de aire entre parrafos (`padding(vertical = 1.dp)`).
        alto += h + if desplegado { 2.0 * e } else { 0.0 };
        medidos.push((ms, t, h));
    }
    Some(TextoDeVoz {
        parrafos: medidos,
        color: color_del_estado(estado),
        fallo: false,
        desplegado,
        ancho: ancho_usado + (VOZ_PUNTO + VOZ_PUNTO_HUECO) * e,
        alto,
    })
}

/// Lo que es la pastilla de la fila de una nota de voz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BotonDeTexto {
    /// Sin texto todavia: pedirlo.
    Pedir,
    /// Se esta pasando a texto ahora mismo.
    Trabajando,
    /// Con texto, plegado: desplegarlo.
    Desplegar,
    /// Con texto, desplegado: plegarlo.
    Plegar,
    /// Ni texto ni audio con el que sacarlo: no hay boton (el movil tampoco
    /// lo pone, `BotonDeTexto` :4283).
    Oculto,
}

/// Que boton lleva la fila de este mensaje en el sitio de «abrir fuera», o
/// `None` si no es una nota de voz y lleva el de siempre.
///
/// El audio se mira por la RUTA apuntada y no en el disco (`m.ruta != null`
/// en el movil): mirar el disco en cada fotograma por cada nota a la vista
/// seria ir al disco para pintar. Si el fichero no esta, lo dice el clic.
fn boton_de_texto(a: &Abierto, m: &pixpin_proyecto::cuaderno::Mensaje) -> Option<BotonDeTexto> {
    if m.clase != Some(pixpin_proyecto::cuaderno::Clase::Voz) {
        return None;
    }
    let moliendo = a
        .transcribiendo
        .as_ref()
        .is_some_and(|t| !t.acabada() && t.id() == m.id);
    Some(estado_del_boton_de_texto(
        moliendo,
        crate::voz::transcripcion_de(m).is_some(),
        a.desplegados.contains(&m.id),
        m.ruta.as_deref().is_some_and(|r| !r.is_empty()),
    ))
}

/// La decision del boton de texto, sin pantalla, para poder probarla: lo
/// que manda primero es si esta moliendo, luego si ya hay texto.
fn estado_del_boton_de_texto(
    moliendo: bool,
    hay_texto: bool,
    desplegado: bool,
    hay_audio: bool,
) -> BotonDeTexto {
    match (moliendo, hay_texto, desplegado) {
        (true, _, _) => BotonDeTexto::Trabajando,
        (false, true, true) => BotonDeTexto::Plegar,
        (false, true, false) => BotonDeTexto::Desplegar,
        (false, false, _) if hay_audio => BotonDeTexto::Pedir,
        (false, false, _) => BotonDeTexto::Oculto,
    }
}

/// Una flechita hacia abajo (desplegar) o hacia arriba (plegar), centrada
/// en `caja`. Son dos trazos: el `expand_more` de Material no esta entre los
/// iconos del pintor, y dos lineas lo dicen igual.
fn pintar_flechita(p: &Pintor, caja: RectF, arriba: bool, color: Color, e: f32) {
    let (cx, cy) = (caja.x + caja.ancho / 2.0, caja.y + caja.alto / 2.0);
    let (media, alto) = (4.5 * e, 2.5 * e);
    let (punta, alas) = if arriba {
        (cy - alto, cy + alto)
    } else {
        (cy + alto, cy - alto)
    };
    let grosor = (1.8 * e).max(1.0);
    p.linea((cx - media, alas), (cx, punta), grosor, color);
    p.linea((cx, punta), (cx + media, alas), grosor, color);
}

/// Pinta el texto de una nota de voz ya medido, en `x, y`.
///
/// Desplegado, cada parrafo con minuto se puede pulsar y lleva el audio
/// ahi; y si esta nota es la que suena, lo ya dicho va en tinta y lo que
/// falta, apagado (`ALFA_DE_LO_NO_DICHO`).
#[allow(clippy::too_many_arguments)] // la nota, donde, el color y la escala
fn pintar_texto_de_voz(
    p: &Pintor,
    a: &Abierto,
    i: usize,
    v: &TextoDeVoz,
    x: f32,
    y: f32,
    tinta: Color,
    e: f32,
) {
    let tam = VOZ_TEXTO_TAM * e;
    let (_, renglon) = p.medir_texto("Ag", tam);
    // El punto, centrado en el primer renglon (`padding(top = 5.dp)`).
    let lado = VOZ_PUNTO * e;
    p.rellenar_redondeado(
        RectF {
            x,
            y: y + (renglon - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        lado / 2.0,
        v.color,
    );
    let tx = x + (VOZ_PUNTO + VOZ_PUNTO_HUECO) * e;
    let ancho_texto = (v.ancho - (VOZ_PUNTO + VOZ_PUNTO_HUECO) * e).max(1.0);
    if v.fallo {
        if let Some((_, aviso, _)) = v.parrafos.first() {
            p.texto_ajustado(aviso, tx, y, tam, ancho_texto + 1.0, v.color);
        }
        return;
    }
    // Por donde va, solo si esta nota es la que esta cargada.
    let por_donde = if v.desplegado {
        a.mensajes
            .get(i)
            .and_then(|m| ruta_del_mensaje(&a.raiz, &a.ficha.id, m))
            .filter(|r| crate::audio::cargado().as_deref() == Some(r.as_path()))
            .and_then(|_| {
                let trozos: Vec<crate::voz::Trozo> = v
                    .parrafos
                    .iter()
                    .map(|(ms, t, _)| crate::voz::Trozo {
                        ms: *ms,
                        texto: t.clone(),
                    })
                    .collect();
                crate::voz::por_donde_va(&trozos, crate::audio::estado().posicion_ms)
            })
    } else {
        None
    };
    let mut py = y;
    for (n, (ms, texto, alto)) in v.parrafos.iter().enumerate() {
        let caja = RectF {
            x: tx,
            y: py,
            ancho: ancho_texto + 1.0,
            alto: *alto,
        };
        let dicho = por_donde.is_none_or(|donde| n <= donde);
        let color = if dicho {
            tinta
        } else {
            con_alfa(tinta, VOZ_NO_DICHO)
        };
        // El plegado se corta a dos renglones con un recorte: el pintor no
        // sabe poner puntos suspensivos a un parrafo de varias lineas.
        p.empujar_recorte(caja);
        p.texto_ajustado(texto, tx, py, tam, ancho_texto + 1.0, color);
        p.soltar_recorte();
        if let Some(ms) = ms {
            a.zonas.borrow_mut().push((
                Rect {
                    x: caja.x.floor() as i32,
                    y: caja.y.floor() as i32,
                    ancho: caja.ancho.ceil() as u32,
                    alto: caja.alto.ceil().max(1.0) as u32,
                },
                Zona::Trozo(i, *ms),
            ));
        }
        py += alto + if v.desplegado { 2.0 * e } else { 0.0 };
    }
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
            let tiempo = if s == 0 {
                String::new()
            } else {
                format!("{}:{:02}", s / 60, s % 60)
            };
            // **Su nombre, a la vista** (4-oct-2026 en el movil): el que se
            // le puso, delante del tiempo. El de serie no dice nada.
            match renombrar::nombre_propio_de_voz(m) {
                Some(n) if tiempo.is_empty() => n,
                Some(n) => format!("{n} · {tiempo}"),
                None => tiempo,
            }
        }
        // Un PDF que se esta aligerando lo dice en vez de su peso, que va a
        // cambiar: «Aligerando… 40 %».
        Clase::Archivo if crate::aligerar::progreso_de_mensaje(&m.id).is_some() => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set(
                "por",
                crate::aligerar::progreso_de_mensaje(&m.id).unwrap_or(0),
            );
            textos.t_args("aligerar-progreso", &args)
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
pub(crate) fn chapa_de_codigo(m: &pixpin_proyecto::cuaderno::Mensaje) -> Option<String> {
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

/// Si de esta foto anotada se ensena solo la foto (`soloLaFoto` del movil,
/// `guardados/Mensajes.kt`). **Por defecto si**: dibujando fuera del borde,
/// encuadrarlo todo encoge la foto y la rodea de blanco, y parece otra. Un
/// mensaje sin el campo —casi todos— es por tanto «solo la foto».
fn solo_la_foto(m: &pixpin_proyecto::cuaderno::Mensaje) -> bool {
    m.resto
        .get("soloLaFoto")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

/// Que parte se ensena de una foto anotada, en esquinas y en coordenadas de
/// la foto: la foto sola, o la foto unida a lo dibujado (que puede salirse
/// por cualquier lado, incluso a negativos). Sin trazos es la foto siempre.
fn encuadre_de_foto(
    doc: (f32, f32),
    trazos: Option<(f32, f32, f32, f32)>,
    solo_la_foto: bool,
) -> (f32, f32, f32, f32) {
    let foto: (f32, f32, f32, f32) = (0.0, 0.0, doc.0, doc.1);
    match trazos {
        Some((a, b, c, d)) if !solo_la_foto => {
            (foto.0.min(a), foto.1.min(b), foto.2.max(c), foto.3.max(d))
        }
        _ => foto,
    }
}

/// Si lo dibujado se sale de la foto. Solo entonces tiene sentido ofrecer
/// «ver el dibujo entero»: si no, las dos vistas son iguales y la entrada
/// pareceria no hacer nada (lo que el movil sufrio con el recorte).
fn dibujo_se_sale(doc: (f32, f32), trazos: Option<(f32, f32, f32, f32)>) -> bool {
    encuadre_de_foto(doc, trazos, false) != encuadre_de_foto(doc, trazos, true)
}

/// Lo que mide la vista de un mensaje dentro de su burbuja.
///
/// Una foto toma SU proporcion: encajarla en una caja fija dejaba bandas a
/// los lados —el «marco blanco» que el usuario pidio quitar—. Cabe en el
/// ancho de siempre y, como mucho, en vez y media del alto, que es lo que
/// deja ver entera una captura vertical del movil sin comerse el historial.
/// Lo demas (un dibujo, una tabla) conserva su caja fija de papel.
fn tamano_de_vista(
    vista: &Ojeada,
    ancho_max: f32,
    e: f32,
    factor: f32,
    solo_la_foto: bool,
) -> (f32, f32) {
    use pixpin_ui::historial as h;
    let ancho = (h::VISTA_ANCHO as f32 * e * factor).min(ancho_max);
    let alto = h::VISTA_ALTO as f32 * e * factor;
    let Ojeada::Foto { doc, trazos, .. } = vista else {
        return (ancho, alto);
    };
    // La proporcion es la del ENCUADRE, que con «ver el dibujo entero» es
    // mas que la foto: la misma funcion la usa `pintar_foto`, asi que lo
    // medido y lo pintado no pueden discrepar.
    let (x0, y0, x1, y1) = encuadre_de_foto(*doc, *trazos, solo_la_foto);
    let (dw, dh) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    let escala = (ancho / dw).min(alto * 1.5 / dh);
    ((dw * escala).max(1.0), (dh * escala).max(1.0))
}

#[cfg(test)]
mod pruebas_vista {
    use super::{Ojeada, dibujo_se_sale, encuadre_de_foto, solo_la_foto, tamano_de_vista};

    fn foto(w: f32, h: f32) -> Ojeada {
        Ojeada::Foto {
            doc: (w, h),
            dibujo: Vec::new(),
            trazos: None,
        }
    }

    /// Una foto de 100x100 con una flecha que sale 50 por la derecha.
    fn foto_con_flecha_fuera() -> Ojeada {
        Ojeada::Foto {
            doc: (100.0, 100.0),
            dibujo: Vec::new(),
            trazos: Some((20.0, 20.0, 150.0, 60.0)),
        }
    }

    #[test]
    fn un_mensaje_sin_el_campo_ensena_solo_la_foto_como_el_movil() {
        let mut m = pixpin_proyecto::cuaderno::Mensaje::default();
        assert!(solo_la_foto(&m));
        m.resto
            .insert("soloLaFoto".into(), serde_json::Value::Bool(false));
        assert!(!solo_la_foto(&m));
        // Un valor que no es booleano no cuenta: vuelve a lo de por defecto.
        m.resto.insert("soloLaFoto".into(), "no".into());
        assert!(solo_la_foto(&m));
    }

    #[test]
    fn el_dibujo_entero_abarca_la_foto_y_lo_que_se_sale_por_cualquier_lado() {
        assert_eq!(
            encuadre_de_foto((100.0, 100.0), Some((-30.0, 10.0, 90.0, 140.0)), false),
            (-30.0, 0.0, 100.0, 140.0)
        );
    }

    #[test]
    fn solo_la_foto_recorta_a_la_foto_aunque_se_salga_el_dibujo() {
        assert_eq!(
            encuadre_de_foto((100.0, 100.0), Some((-30.0, 10.0, 90.0, 140.0)), true),
            (0.0, 0.0, 100.0, 100.0)
        );
    }

    #[test]
    fn un_dibujo_dentro_de_la_foto_no_ofrece_ver_el_dibujo_entero() {
        assert!(!dibujo_se_sale(
            (100.0, 100.0),
            Some((10.0, 10.0, 90.0, 90.0))
        ));
        assert!(!dibujo_se_sale((100.0, 100.0), None));
        assert!(dibujo_se_sale(
            (100.0, 100.0),
            Some((10.0, 10.0, 120.0, 90.0))
        ));
    }

    #[test]
    fn ver_el_dibujo_entero_ensancha_la_burbuja_con_su_proporcion() {
        let v = foto_con_flecha_fuera();
        let (sw, sh) = tamano_de_vista(&v, 400.0, 1.0, 1.0, true);
        assert!((sw / sh - 1.0).abs() < 0.01, "sola es cuadrada: {sw}x{sh}");
        let (ew, eh) = tamano_de_vista(&v, 400.0, 1.0, 1.0, false);
        assert!((ew / eh - 1.5).abs() < 0.01, "entera es 150x100: {ew}x{eh}");
    }

    #[test]
    fn la_vista_de_una_foto_tiene_su_misma_proporcion() {
        // Sin bandas: si la caja no tuviera la proporcion de la foto, por los
        // lados asomaria el fondo, que es el marco blanco que se quito.
        for (w, h) in [(1920.0, 1080.0), (1080.0, 2340.0), (500.0, 500.0)] {
            let (vw, vh) = tamano_de_vista(&foto(w, h), 400.0, 1.0, 1.0, true);
            assert!(
                ((vw / vh) - (w / h)).abs() < 0.01,
                "{w}x{h} salio {vw}x{vh}"
            );
            assert!(vw <= 260.5, "no pasa del ancho de la vista: {vw}");
        }
    }

    #[test]
    fn una_foto_sin_tamano_no_divide_por_cero() {
        let (vw, vh) = tamano_de_vista(&foto(0.0, 0.0), 400.0, 1.0, 1.0, true);
        assert!(vw.is_finite() && vh.is_finite() && vw >= 1.0 && vh >= 1.0);
    }
}

// --- La caja de escribir, la barra de responder y el aviso ------------

/// La caja de escribir: la isla flotante del movil, con el campo en
/// pastilla, el clip dentro y el microfono (o enviar) fuera.
/// La barra del reproductor, encima de la caja de escribir.
///
/// Es `BarraDelReproductor.kt:46-106`. Lo que se ensena sale del estado del
/// reproductor y no de la burbuja que se pulso: la nota sigue sonando
/// aunque se baje por la conversacion o se cambie de proyecto, que es la
/// razon de que el reproductor sea uno para toda la aplicacion.
fn pintar_barra_del_reproductor(p: &Pintor, barra: Rect, c: &Pinta, a: &Abierto) {
    use pixpin_ui::reproductor as ui;
    let (tema, escala) = (c.tema, c.escala);
    let e = escala as f32 / 100.0;
    if barra.ancho == 0 || barra.alto == 0 {
        return;
    }
    let estado = crate::audio::estado();
    let t = ui::Disposicion::calcular(barra, escala);
    let caja = RectF {
        alto: (barra.alto as f32 - 6.0 * e).max(1.0),
        ..rf(barra)
    };
    p.rellenar_redondeado(caja, 16.0 * e, tema.campo);

    // **Primero los tramos de avance y luego los botones**: el buscador de
    // zonas se queda con la ULTIMA que contiene el punto, asi que lo que se
    // apunta despues tapa a lo de antes. Al reves, pulsar pausa saltaria al
    // minuto donde cayo el raton.
    //
    // En diez tramos y no pixel a pixel porque `Zona` se guarda por valor:
    // diez bastan para ir al trozo que se quiere de una nota de voz.
    for n in 0..10u32 {
        let ancho = t.barra.ancho / 10;
        a.zonas.borrow_mut().push((
            Rect {
                x: t.barra.x + (n * ancho) as i32,
                y: t.barra.y,
                ancho,
                alto: t.barra.alto,
            },
            // El centro del tramo, en milesimas.
            Zona::VozIrA(n * 100 + 50),
        ));
    }

    let icono_boton = 20.0 * e;
    // Los mismos tres iconos que el movil: `replay_10` y `forward_10` llevan
    // el «10» dibujado dentro, asi que dicen CUANTO se salta sin gastar el
    // hueco del titulo en un rotulo; y la pausa es el `pause` de Material,
    // no dos barras a mano que nunca caen igual que el resto.
    for (icono, r, zona) in [
        (&mi::REPLAY_10, t.atras, Zona::VozAtras),
        (&mi::FORWARD_10, t.adelante, Zona::VozAdelante),
        (&mi::CLOSE, t.cerrar, Zona::VozCerrar),
    ] {
        icono_centrado(p, icono, r, icono_boton, tema.texto);
        a.zonas.borrow_mut().push((r, zona));
    }
    let tocar = if estado.sonando {
        &mi::PAUSE
    } else {
        &mi::PLAY_ARROW
    };
    icono_centrado(p, tocar, t.tocar, icono_boton, tema.texto);
    a.zonas.borrow_mut().push((t.tocar, Zona::VozTocar));

    // La velocidad se escribe, no se dibuja: «1,5×» dice mas que cualquier
    // icono, y es lo que hace el movil (`velocidadLegible` :113-114).
    let vel = pixpin_audio::velocidad_legible(estado.velocidad);
    let tam = ui::TEXTO_TAM * e;
    let (ancho_vel, alto_vel) = p.medir_texto(&vel, tam);
    p.texto(
        &vel,
        t.velocidad.x as f32 + (t.velocidad.ancho as f32 - ancho_vel) / 2.0,
        t.velocidad.y as f32 + (t.velocidad.alto as f32 - alto_vel) / 2.0,
        tam,
        tema.enviar,
    );
    a.zonas.borrow_mut().push((t.velocidad, Zona::VozVelocidad));

    // El titulo y el tiempo. El tiempo a la derecha del hueco, que es donde
    // no lo tapa un titulo largo.
    if t.texto.ancho > 0 {
        let tiempo = format!(
            "{} / {}",
            pixpin_audio::duracion_legible(estado.posicion_ms),
            pixpin_audio::duracion_legible(estado.duracion_ms)
        );
        let (ancho_tiempo, alto_t) = p.medir_texto(&tiempo, tam);
        let y = t.texto.y as f32 + (t.texto.alto as f32 - alto_t) / 2.0;
        p.texto(
            &tiempo,
            t.texto.derecha() as f32 - ancho_tiempo,
            y,
            tam,
            tema.campo_apagado,
        );
        p.texto_linea(
            &estado.titulo,
            t.texto.x as f32,
            y,
            tam,
            (t.texto.ancho as f32 - ancho_tiempo - 10.0 * e).max(0.0),
            tema.texto,
        );
    }

    // La linea de avance. Sin duracion todavia no se pinta: una barra llena
    // al empezar seria mentira mientras Media Foundation la averigua.
    if estado.duracion_ms > 0 {
        p.rellenar_redondeado(rf(t.avance(estado.fraccion())), 1.5 * e, tema.enviar);
    }
}

/// **La ventanita de la hora a mano** del recordatorio: una tarjeta en el
/// centro, sobre un velo, encima de todo lo del chat. Antes era una barra
/// sobre la caja de escribir, y otras cosas de la interfaz la tapaban y no
/// dejaban escribir (el usuario, 8-oct-2026). Las teclas son las de antes:
/// se escribe, Intro la pone y Esc la quita; la ✕ tambien.
fn pintar_hora_a_mano(p: &Pintor, c: &Pinta, a: &Abierto, marco: Rect) {
    let Some((_, escrito)) = &a.hora_a_mano else {
        return;
    };
    let (tema, textos) = (c.tema, c.textos);
    let e = c.escala as f32 / 100.0;
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: h,
        },
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.45,
        },
    );
    let ancho = (420.0 * e).min(w - 32.0 * e).max(1.0);
    let alto = 168.0 * e;
    let caja = RectF {
        x: (w - ancho) / 2.0,
        y: ((h - alto) / 2.0).max(8.0 * e),
        ancho,
        alto,
    };
    p.rellenar_redondeado(caja, 18.0 * e, tema.campo);
    // La cabecera: el reloj, el titulo y la ✕.
    let icono = 22.0 * e;
    p.icono(
        &mi::ALARM,
        RectF {
            x: caja.x + 18.0 * e,
            y: caja.y + 18.0 * e,
            ancho: icono,
            alto: icono,
        },
        tema.enviar,
    );
    let titulo = textos.t("chat-recordar-hora");
    p.texto_linea(
        &titulo,
        caja.x + 50.0 * e,
        caja.y + 18.0 * e,
        16.0 * e,
        ancho - 110.0 * e,
        tema.texto,
    );
    let cerrar = Rect {
        x: (caja.x + ancho - 52.0 * e) as i32,
        y: (caja.y + 6.0 * e) as i32,
        ancho: (44.0 * e) as u32,
        alto: (44.0 * e) as u32,
    };
    icono_centrado(p, &mi::CLOSE, cerrar, 20.0 * e, tema.campo_apagado);
    a.zonas.borrow_mut().push((cerrar, Zona::CerrarHora));
    // La caja con lo tecleado y su cursor.
    let campo = RectF {
        x: caja.x + 18.0 * e,
        y: caja.y + 60.0 * e,
        ancho: ancho - 36.0 * e,
        alto: 44.0 * e,
    };
    p.rellenar_redondeado(campo, 10.0 * e, tema.chat);
    p.trazar(campo, 1.5 * e, tema.enviar);
    let rotulo = format!("{} {escrito}|", textos.t("chat-recordar-hora-a-las"));
    let (_, alto_r) = p.medir_texto(&rotulo, 15.0 * e);
    p.texto_linea(
        &rotulo,
        campo.x + 14.0 * e,
        campo.y + (campo.alto - alto_r) / 2.0,
        15.0 * e,
        campo.ancho - 28.0 * e,
        tema.texto,
    );
    // Debajo, a que hora queda de verdad: «09:00» tecleado a las diez es
    // MANANA, y eso se ve antes de pulsar Intro, no despues.
    let ahora = pixpin_shell::entorno::ahora_local_ms();
    let pista = match crate::recordatorios::hora_escrita(escrito, ahora) {
        Some(cuando) => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("cuando", crate::recordatorios::cuando_legible(cuando, ahora));
            textos.t_args("chat-recordar-hora-queda", &args)
        }
        None => textos.t("chat-recordar-hora-teclas"),
    };
    p.texto_linea(
        &pista,
        campo.x + 2.0 * e,
        campo.y + campo.alto + 14.0 * e,
        13.0 * e,
        campo.ancho,
        tema.campo_apagado,
    );
}

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
    // La barra del reproductor manda sobre las otras dos: mientras algo
    // suena es lo unico que puede pararlo, y esconderla por estar buscando o
    // contestando dejaria una voz sonando sin boton de pausa a la vista.
    //
    // Menos a la de teclear la hora de un recordatorio, que manda sobre
    // todas: se abrio a proposito hace un segundo y lo que se teclea va ahi.
    // Igual la de «Quien llama» (v0.98.6), que se abre desde ese mismo menu.
    // Cambiando el nombre de un archivo (`renombrar`): lo tecleado, aqui y a
    // la vista, porque una foto o un audio no tienen renglon de nombre donde
    // escribirlo. Manda sobre todas: se abrio a proposito desde el menu.
    if let Some((_, escrito)) = &a.renombrando_mensaje {
        let barra = d.encima_de_la_isla(alto_texto, escala);
        let caja = RectF {
            alto: (barra.alto as f32 - 6.0 * e).max(1.0),
            ..rf(barra)
        };
        p.rellenar_redondeado(caja, 16.0 * e, tema.campo);
        let icono = 18.0 * e;
        // La misma barra para la descripcion de una foto, con su icono y
        // su rotulo (`describiendo`).
        let (dibujo, barra_clave, teclas_clave) = if a.describiendo {
            (
                &mi::SUBTITLES,
                "chat-describir-barra",
                "chat-describir-teclas",
            )
        } else {
            (&mi::EDIT, "chat-renombrar-barra", "chat-renombrar-teclas")
        };
        p.icono(
            dibujo,
            RectF {
                x: caja.x + 12.0 * e,
                y: caja.y + (caja.alto - icono) / 2.0,
                ancho: icono,
                alto: icono,
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
        a.zonas.borrow_mut().push((cerrar, Zona::CerrarNombre));
        let pista = textos.t(teclas_clave);
        let tam = 12.0 * e;
        let (w_pista, alto_r) = p.medir_texto(&pista, tam);
        let y = caja.y + (caja.alto - alto_r) / 2.0;
        let x_pista = (cerrar.x as f32 - w_pista - 6.0 * e).max(caja.x);
        p.texto(&pista, x_pista, y, tam, tema.campo_apagado);
        let x = caja.x + 40.0 * e;
        let cabe = (x_pista - x - 8.0 * e).max(0.0);
        // Un renglon: los saltos de una descripcion se ven como espacios, y
        // si no cabe se ensena el FINAL, que es donde se esta escribiendo.
        let rotulo_de = |s: &str| format!("{} {s}|", textos.t(barra_clave));
        let seguido = escrito.replace('\n', " ");
        // Se empieza por los ultimos 200 signos: medir uno a uno un texto
        // largo en cada fotograma seria tirar el rato.
        let mut desde = seguido.char_indices().rev().nth(200).map_or(0, |(i, _)| i);
        while desde < seguido.len()
            && p.medir_texto(&rotulo_de(&seguido[desde..]), 13.0 * e).0 > cabe
        {
            desde += seguido[desde..].chars().next().map_or(1, char::len_utf8);
        }
        let rotulo = if desde == 0 {
            rotulo_de(&seguido)
        } else {
            rotulo_de(&format!("…{}", &seguido[desde..]))
        };
        p.texto_linea(&rotulo, x, y, 13.0 * e, cabe, tema.texto);
    } else if let Some((_, escrito)) = &a.quien_llama {
        let barra = d.encima_de_la_isla(alto_texto, escala);
        let cerrar = quien_llama::pintar_barra(p, barra, tema, textos, escrito, e);
        a.zonas.borrow_mut().push((cerrar, Zona::CerrarQuien));
    } else if crate::audio::hay_algo() {
        pintar_barra_del_reproductor(p, d.encima_de_la_isla(alto_texto, escala), c, a);
    } else if let Some(cuantos) = resultados_de_la_busqueda(a) {
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

    // Grabando, la caja de escribir deja su sitio al nivel del microfono y
    // al cronometro: mientras se graba no se escribe, y hace falta ver que
    // el microfono de verdad esta cogiendo voz y no silencio.
    let icono = 24.0 * e;
    if let Some(g) = a.grabando.as_ref() {
        let boton = d.boton_enviar(alto_texto, escala);
        icono_centrado(p, &mi::STOP, boton, icono, hex(0xe5534b));
        let tam = chat::REDACCION_TAM * e;
        let zona = d.texto_redaccion(alto_texto, escala);
        let llevado = pixpin_audio::duracion_legible(g.grabadora.llevado_ms());
        let (ancho_t, alto_t) = p.medir_texto(&llevado, tam);
        let y = zona.y as f32;
        p.texto(&llevado, zona.x as f32, y, tam, hex(0xe5534b));
        // El nivel, en barras que suben y bajan con la voz. Sobre el maximo
        // de la escala del movil (32767) para que una nota grabada aqui y
        // otra alli se vean con la misma altura.
        let x0 = zona.x as f32 + ancho_t + 12.0 * e;
        let nivel = (g.grabadora.nivel() as f32 / pixpin_audio::PICO_MAXIMO as f32).clamp(0.0, 1.0);
        let paso = pixpin_ui::chat::ONDA_PASO as f32 * e;
        let grueso = (pixpin_ui::chat::ONDA_GRUESO as f32 * e).max(1.0);
        let eje = y + alto_t / 2.0;
        let cuantas = (((zona.ancho as f32 - ancho_t - 12.0 * e) / paso).floor() as i32).max(0);
        for n in 0..cuantas {
            // Las de en medio mas altas: un nivel plano de lado a lado
            // parece un dibujo y no una voz.
            let centro = 1.0 - ((n as f32 / cuantas.max(1) as f32) - 0.5).abs() * 2.0;
            let medio = (alto_t * 0.5 * nivel * centro).max(0.5);
            p.rellenar_redondeado(
                RectF {
                    x: x0 + n as f32 * paso,
                    y: eje - medio,
                    ancho: grueso,
                    alto: medio * 2.0,
                },
                grueso / 2.0,
                tema.enviar,
            );
        }
        return;
    }

    // Un sitio, dos caras: el microfono, o enviar si hay algo escrito. Y el
    // clip se retira al escribir, como en el movil.
    let hay_texto = !a.borrador.trim().is_empty();
    if a.dictado.is_some() {
        // Dictando, el boton es el microfono encendido: pulsarlo termina.
        icono_centrado(
            p,
            &mi::MIC,
            d.boton_enviar(alto_texto, escala),
            icono,
            hex(0xe5534b),
        );
    } else if hay_texto {
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
    // Lo que el dictado va entendiendo se ensena detras de lo escrito; al
    // cerrarse la frase pasa al borrador de verdad (`latido_del_dictado`).
    let provisional = a
        .dictado
        .as_ref()
        .map(|d| d.provisional.as_str())
        .filter(|t| !t.is_empty());
    let con_lo_oido;
    let borrador: &str = match provisional {
        Some(t) => {
            con_lo_oido = juntar_dictado(&a.borrador, t);
            &con_lo_oido
        }
        None => &a.borrador,
    };
    // Las cajas de las letras, para el raton. Solo con lo escrito de verdad
    // a la vista: con el dictado delante, lo que se ve aun no es el borrador
    // y una posicion en el no valdria para escribir.
    {
        let mut l = a.letras.borrow_mut();
        let campo = d.campo(alto_texto, escala);
        l.zona = (provisional.is_none()).then(|| rf(campo));
        l.origen = (x, y);
        if l.texto != a.borrador || l.tam != tam || l.ancho != ancho {
            l.cajas = redaccion::cajas_de_letras(&a.borrador, |desde, largo| {
                p.cajas_de_trozo(&a.borrador, tam, ancho, &[], desde, largo)
                    .first()
                    .copied()
            });
            l.texto = a.borrador.clone();
            l.tam = tam;
            l.ancho = ancho;
        }
    }
    if borrador.is_empty() {
        let invitacion = if let Some(d) = a.dictado.as_ref() {
            textos.t(if d.oyendo {
                "chat-dictado-oyendo"
            } else {
                "chat-dictado-preparando"
            })
        } else {
            textos.t("chat-escribe")
        };
        p.texto(&invitacion, x, y, tam, tema.campo_apagado);
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
    if provisional.is_some() {
        p.parrafo(borrador, x, y, tam, ancho, &[], tema.texto);
        // Dictando, el cursor va al final de lo escrito y de lo oido.
        let (_, alto_todo) = p.medir_texto_ajustado(borrador, tam, ancho);
        let ultima = borrador.rsplit('\n').next().unwrap_or("");
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
        return;
    }
    let letras = a.letras.borrow();
    // Lo seleccionado, debajo del texto: la caja de cada letra elegida.
    if let Some(r) = a.cursor.seleccion(&a.borrador) {
        for (_, b) in letras.cajas.iter().filter(|(i, _)| r.contains(i)) {
            p.rellenar(
                RectF {
                    x: x + b.x,
                    y: y + b.y,
                    // El salto de renglon no tiene ancho; se le da un poco
                    // para que se vea que tambien va elegido.
                    ancho: b.ancho.max(4.0 * e),
                    alto: b.alto,
                },
                con_alfa(tema.enviar, 0.35),
            );
        }
    }
    p.parrafo(borrador, x, y, tam, ancho, &[], tema.texto);
    // La rayita donde diga el cursor, con la caja que le da DirectWrite a
    // su letra.
    let (cx, cy, alto_rayita) = redaccion::sitio_del_cursor(
        &letras.texto,
        &letras.cajas,
        a.cursor.pos(&a.borrador),
        tam * 1.3,
    );
    p.rellenar(
        RectF {
            x: x + cx.min(ancho),
            y: y + cy,
            ancho: (1.0 * e).max(1.0),
            alto: alto_rayita,
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
    /// v2: la fila «Más» del menu de un mensaje, que abre su submenu.
    Mas,
    /// v2: «Captura» de la hoja del «+»: la de zona, por el camino del atajo.
    AdjCaptura,
    AdjArchivo,
    AdjImagen,
    AdjLienzo,
    AdjTabla,
    AdjDelMovil,
    AdjProyectos,
    AdjProyecto(String),
    /// «Una pagina de un proyecto»: primero el proyecto, luego su hoja
    /// (proyecto, hoja). Ver `paginas`.
    AdjPagina,
    AdjPaginaDe(String),
    AdjPaginaHoja(String, String),
    /// Los ajustes de PixPin, los mismos de la bandeja.
    Ajustes,
    /// Una de las siete del movil, por su palabra de `Mensaje.miniapp`. La
    /// palabra es el dato guardado y no se renombra (`MiniApps.kt:94-99`).
    AdjMini(&'static str),
    /// Todas las notas de voz y toda la musica, de todas las conversaciones.
    Biblioteca,
    /// Bajar las fotos de dentro de un PDF adjunto, sustituyendolo.
    Aligerar(usize),
    /// Las paginas del PDF elegidas, juntas en un lienzo nuevo (E9,
    /// `FusionarPaginas` del movil).
    FusionarPaginas,
    // Un mensaje, por su posicion.
    Responder(usize),
    Copiar(usize),
    Fijar(usize),
    Compartir(usize),
    /// De una foto anotada, pasar de «solo la foto» a «el dibujo entero» o
    /// al reves (`soloLaFoto` del movil).
    AlternarRecorte(usize),
    AbrirCon(usize),
    Renombrar(usize),
    /// «Añadir descripción» / «Editar descripción» de una foto: la barra de
    /// renombrar, escribiendo su `texto` (`descripcion`).
    Describir(usize),
    /// Exportar o imprimir la hoja de un lienzo (G2, F11), con la caja
    /// de exportar o el dialogo de imprimir de Windows.
    Exportar(usize),
    Imprimir(usize),
    /// Meter la hoja como pagina viva en una nota (H12, `paginas_vivas`).
    InsertarEnNota(usize),
    Pinear(usize),
    /// Sacar a la pantalla la lista de tareas de ese mensaje, viva: lo que
    /// se tacha en el pin se guarda en el mensaje (`herramienta::Vinculo`).
    PinearLista(usize),
    Rescatar(usize),
    Etiquetar(usize, Option<String>),
    Hilo(usize),
    /// Ofrecer las horas a las que avisar de este mensaje.
    Recordar(usize),
    /// Ponerle la hora elegida, **en milisegundos de hora local**: es lo que
    /// el usuario leyo en el menu, y a UTC se pasa justo antes de guardarlo.
    RecordarEn(usize, i64),
    /// «Elegir la hora…»: abrir la barra donde se teclea.
    RecordarAMano(usize),
    /// «Quien llama: …» de una nota de voz: abrir la barra donde se teclea.
    QuienLlama(usize),
    /// Quitarle la hora que tenia.
    Olvidar(usize),
    Ir(usize),
    Reenviar(Vec<usize>),
    ReenviarA(Vec<usize>, String),
    Elegir(usize),
    /// Meter el mensaje en las hojas del proyecto, y devolverle la que se le
    /// quito (`UnirAlProyecto` del movil).
    Unir(usize),
    Devolver(usize),
    /// Las paginas de un PDF como fotos clavadas en el proyecto, en su hilo.
    ComoImagenes(usize),
    /// Pasar a texto la nota de voz de esa burbuja, con Vosk y en su hilo.
    Transcribir(usize),
    /// «Letra o texto»: el texto de esa nota en grande, en el sitio del
    /// historial, con sus minutos pulsables (`LetraActivity`).
    Letra(usize),
    /// Pronunciar (B9): hablar, oirse y repetir con una guia delante.
    Pronunciar,
    /// La conversacion por turnos (B10).
    Conversacion,
    /// Abrir el telepronter: antes hay que elegir QUE se lee.
    Telepronter,
    /// Leer lo que hay escrito en la caja de abajo.
    TelepronterBorrador,
    /// Leer ese mensaje: una nota escrita, o un `.txt`/`.md` adjunto.
    TelepronterMensaje(usize),
    Borrar(Vec<usize>),
    FotoEnLienzo(usize),
    FotoAqui(usize),
    // La cabecera.
    Fondos,
    Fondo(usize),
    /// El submenu de la escala de la interfaz, y cada escalon.
    Escalas,
    Escala(u32),
    Proyectos,
    /// Lo que en Windows aun no existe: la clave del aviso.
    Aviso(&'static str),
    /// Una nota Markdown nueva en el proyecto, en su editor (H12).
    AdjNotaMd,
    /// Abrir esa nota (o ese `.md`) en el editor de notas (H12).
    EditarNota(usize),
    /// El menu de los grupos de ventanas guardados (H9).
    GruposVentanas,
    /// «Añadir arrastrando…»: la caja de soltar de este proyecto (2-oct).
    CajaDeSoltar,
    /// Las lecciones aprendidas, con las de este proyecto primero (3-oct).
    Lecciones,
    /// «Hacer leccion» de ese mensaje: la ficha empieza con su texto.
    HacerLeccion(usize),
}

/// Una linea de menu: icono, nombre, su tecla y lo que hace. Borrar va en
/// rojo. `separada` pone una raya encima; `en_mas` la manda al submenu
/// «Más» (v2, `Menus2.dc.html`).
struct EntradaMenu {
    icono: Option<&'static Icono>,
    texto: String,
    peligro: bool,
    accion: Accion,
    tecla: Option<menu_v2::Tecla>,
    separada: bool,
    en_mas: bool,
}

fn entrada(icono: Option<&'static Icono>, texto: String, accion: Accion) -> EntradaMenu {
    EntradaMenu {
        icono,
        texto,
        peligro: false,
        accion,
        tecla: None,
        separada: false,
        en_mas: false,
    }
}

impl EntradaMenu {
    fn con_tecla(mut self, t: menu_v2::Tecla) -> EntradaMenu {
        self.tecla = Some(t);
        self
    }

    fn separada(mut self) -> EntradaMenu {
        self.separada = true;
        self
    }

    fn en_mas(mut self) -> EntradaMenu {
        self.en_mas = true;
        self
    }
}

/// Lo que pasa al tocar un menu abierto (raton o teclado).
#[derive(Debug, Clone, PartialEq)]
enum Respuesta {
    /// Se cierra sin hacer nada (clic fuera, Esc).
    Fuera,
    /// Sigue abierto (clic en su relleno, flechas, abrir «Más»...).
    Sigue,
    Hacer(Accion),
}

/// Un menu desplegado. Mide sus rotulos al pintarse, y el raton usa esas
/// medidas: el ancho lo manda el mas largo y solo se sabe con la fuente.
///
/// Tambien es la hoja del «+» de la caja (`hoja`): asi todo lo que mira si
/// hay un menu delante la ve igual, y lo que se elige en ella pasa por el
/// mismo camino que una entrada del menu de antes.
struct MenuAbierto {
    ancla: Punto,
    entradas: Vec<EntradaMenu>,
    /// El submenu «Más», si lo hay.
    mas: Vec<EntradaMenu>,
    /// La fila de etiquetas de encima: el mensaje y la que ya tiene.
    etiquetas: Option<(usize, Option<String>)>,
    hoja: Option<hoja_adjuntar::Hoja>,
    necesita: std::cell::RefCell<Vec<f32>>,
    necesita_mas: std::cell::RefCell<Vec<f32>>,
    /// Bajo el raton.
    sobre: Option<menu_v2::Sitio>,
    /// Elegido con las flechas: lo que hace Intro, en azul.
    elegido: Option<menu_v2::Sitio>,
    mas_abierto: bool,
}

impl MenuAbierto {
    fn nuevo(ancla: Punto, entradas: Vec<EntradaMenu>) -> MenuAbierto {
        let (mas, entradas): (Vec<EntradaMenu>, Vec<EntradaMenu>) =
            entradas.into_iter().partition(|e| e.en_mas);
        // Una fila «Más» sin nada dentro seria un boton muerto.
        let entradas: Vec<EntradaMenu> = if mas.is_empty() {
            entradas
                .into_iter()
                .filter(|e| e.accion != Accion::Mas)
                .collect()
        } else {
            entradas
        };
        MenuAbierto {
            ancla,
            necesita: std::cell::RefCell::new(vec![0.0; entradas.len()]),
            necesita_mas: std::cell::RefCell::new(vec![0.0; mas.len()]),
            entradas,
            mas,
            etiquetas: None,
            hoja: None,
            sobre: None,
            elegido: None,
            mas_abierto: false,
        }
    }

    /// La hoja del «+» de la caja, que crece hacia arriba desde `ancla`.
    fn de_hoja(ancla: Punto, hoja: hoja_adjuntar::Hoja) -> MenuAbierto {
        let mut m = MenuAbierto::nuevo(ancla, Vec::new());
        m.hoja = Some(hoja);
        m
    }

    /// Con la fila de etiquetas encima, para el mensaje `i`.
    fn con_etiquetas(mut self, i: usize, puesta: Option<String>) -> MenuAbierto {
        self.etiquetas = Some((i, puesta));
        self
    }

    fn limite(marco: Rect) -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: marco.ancho,
            alto: marco.alto,
        }
    }

    /// Donde cae, calculado igual al pintar y al pulsar.
    fn colocado(&self, marco: Rect, escala: u32) -> menu_v2::Colocado {
        let separado: Vec<bool> = self.entradas.iter().map(|e| e.separada).collect();
        let emojis = if self.etiquetas.is_some() {
            chat::ETIQUETAS.len()
        } else {
            0
        };
        menu_v2::colocar(
            self.ancla,
            MenuAbierto::limite(marco),
            &self.necesita.borrow(),
            &separado,
            emojis,
            escala,
        )
    }

    /// El submenu «Más», si esta abierto.
    fn colocado_mas(&self, marco: Rect, escala: u32) -> Option<menu_v2::Colocado> {
        if !self.mas_abierto || self.mas.is_empty() {
            return None;
        }
        let madre = self.colocado(marco, escala);
        let n = self.entradas.iter().position(|e| e.accion == Accion::Mas)?;
        let separado: Vec<bool> = self.mas.iter().map(|e| e.separada).collect();
        Some(menu_v2::colocar_al_lado(
            madre.caja,
            *madre.filas.get(n)?,
            MenuAbierto::limite(marco),
            &self.necesita_mas.borrow(),
            &separado,
            escala,
        ))
    }

    fn sitio_en(&self, p: Punto, marco: Rect, escala: u32) -> Option<menu_v2::Sitio> {
        use menu_v2::Sitio;
        if let Some(n) = self.colocado_mas(marco, escala).and_then(|c| c.fila_en(p)) {
            return Some(Sitio::Mas(n));
        }
        let c = self.colocado(marco, escala);
        if let Some(n) = c.emoji_en(p) {
            return Some(Sitio::Emoji(n));
        }
        c.fila_en(p).map(Sitio::Principal)
    }

    fn dentro(&self, p: Punto, marco: Rect, escala: u32) -> bool {
        self.colocado(marco, escala).contiene(p)
            || self
                .colocado_mas(marco, escala)
                .is_some_and(|c| c.contiene(p))
    }

    /// Lo que hace un sitio del menu. Una etiqueta ya puesta, pulsada otra
    /// vez, se quita: es lo que se espera de una casilla.
    fn accion_de(&self, s: menu_v2::Sitio) -> Option<Accion> {
        use menu_v2::Sitio;
        match s {
            Sitio::Principal(n) => self.entradas.get(n).map(|e| e.accion.clone()),
            Sitio::Mas(n) => self.mas.get(n).map(|e| e.accion.clone()),
            Sitio::Emoji(n) => {
                let (i, puesta) = self.etiquetas.as_ref()?;
                let emoji = (*chat::ETIQUETAS.get(n)?).to_string();
                Some(if puesta.as_deref() == Some(emoji.as_str()) {
                    Accion::Etiquetar(*i, None)
                } else {
                    Accion::Etiquetar(*i, Some(emoji))
                })
            }
        }
    }

    /// Hacer lo de un sitio: «Más» abre o cierra su submenu y deja el menu
    /// abierto; lo demas se hace.
    fn hacer(&mut self, s: menu_v2::Sitio) -> Respuesta {
        match self.accion_de(s) {
            Some(Accion::Mas) => {
                self.mas_abierto = !self.mas_abierto;
                self.elegido = Some(s);
                Respuesta::Sigue
            }
            Some(a) => Respuesta::Hacer(a),
            None => Respuesta::Sigue,
        }
    }

    /// Un clic con el menu delante.
    fn pulsar(&mut self, p: Punto, marco: Rect, escala: u32) -> Respuesta {
        if let Some(h) = self.hoja.as_mut() {
            return h.pulsar(p, self.ancla, marco, escala);
        }
        match self.sitio_en(p, marco, escala) {
            Some(s) => self.hacer(s),
            None if self.dentro(p, marco, escala) => Respuesta::Sigue,
            None => Respuesta::Fuera,
        }
    }

    /// El raton se mueve: el resalte sigue al raton y «Más» se abre al
    /// pasar por encima, como un submenu de Windows. Devuelve si cambio algo.
    fn mover(&mut self, p: Punto, marco: Rect, escala: u32) -> bool {
        if let Some(h) = self.hoja.as_mut() {
            return h.mover(p, self.ancla, marco, escala);
        }
        use menu_v2::Sitio;
        let s = self.sitio_en(p, marco, escala);
        let antes = (self.sobre, self.mas_abierto);
        self.sobre = s;
        if let Some(Sitio::Principal(n)) = s {
            self.mas_abierto = self
                .entradas
                .get(n)
                .is_some_and(|e| e.accion == Accion::Mas);
        }
        antes != (self.sobre, self.mas_abierto)
    }

    /// Una tecla pulsada con el menu abierto. Todas son suyas mientras
    /// esta delante: que una flecha moviera la conversacion de debajo seria
    /// sorprender a quien esta eligiendo.
    fn tecla(&mut self, vk: u32, ctrl: bool) -> Respuesta {
        if let Some(h) = self.hoja.as_mut() {
            return h.tecla(vk);
        }
        use menu_v2::Sitio;
        if vk == VK_ESCAPE {
            if self.mas_abierto && matches!(self.elegido, Some(Sitio::Mas(_))) {
                self.mas_abierto = false;
                self.elegido = self.posicion_de_mas().map(Sitio::Principal);
                return Respuesta::Sigue;
            }
            return Respuesta::Fuera;
        }
        if vk == VK_ENTRAR {
            return match self.elegido {
                Some(s) => self.hacer(s),
                None => Respuesta::Sigue,
            };
        }
        if vk == VK_ABAJO || vk == VK_ARRIBA {
            let abajo = vk == VK_ABAJO;
            self.elegido = match self.elegido {
                Some(Sitio::Mas(n)) if self.mas_abierto => {
                    menu_v2::siguiente(Some(n), self.mas.len(), abajo).map(Sitio::Mas)
                }
                Some(Sitio::Principal(n)) => {
                    menu_v2::siguiente(Some(n), self.entradas.len(), abajo).map(Sitio::Principal)
                }
                _ => menu_v2::siguiente(None, self.entradas.len(), abajo).map(Sitio::Principal),
            };
            return Respuesta::Sigue;
        }
        if vk == VK_DERECHA {
            if let Some(Sitio::Principal(n)) = self.elegido
                && self
                    .entradas
                    .get(n)
                    .is_some_and(|e| e.accion == Accion::Mas)
            {
                self.mas_abierto = true;
                self.elegido = (!self.mas.is_empty()).then_some(Sitio::Mas(0));
            }
            return Respuesta::Sigue;
        }
        if vk == VK_IZQUIERDA {
            if matches!(self.elegido, Some(Sitio::Mas(_))) {
                self.mas_abierto = false;
                self.elegido = self.posicion_de_mas().map(Sitio::Principal);
            }
            return Respuesta::Sigue;
        }
        // Las teclas con chapita: Ctrl+C, Supr, F2... de las dos listas.
        let de_tecla = |l: &[EntradaMenu]| {
            l.iter()
                .find(|e| e.tecla.is_some_and(|t| t.coincide_tecla(vk, ctrl)))
                .map(|e| e.accion.clone())
        };
        match de_tecla(&self.entradas).or_else(|| de_tecla(&self.mas)) {
            Some(a) => Respuesta::Hacer(a),
            None => Respuesta::Sigue,
        }
    }

    /// Un caracter escrito con el menu abierto: las letras de una tecla
    /// («R» responder, «P» pinear...). Lo demas no se escribe en ningun
    /// sitio: el menu esta delante de la caja.
    fn caracter(&mut self, c: char) -> Respuesta {
        if let Some(h) = self.hoja.as_mut() {
            return h.caracter(c);
        }
        let de_letra = |l: &[EntradaMenu]| {
            l.iter()
                .find(|e| e.tecla.is_some_and(|t| t.coincide_caracter(c)))
                .map(|e| e.accion.clone())
        };
        match de_letra(&self.entradas).or_else(|| de_letra(&self.mas)) {
            Some(Accion::Mas) => {
                self.mas_abierto = !self.mas_abierto;
                Respuesta::Sigue
            }
            Some(a) => Respuesta::Hacer(a),
            None => Respuesta::Sigue,
        }
    }

    fn posicion_de_mas(&self) -> Option<usize> {
        self.entradas.iter().position(|e| e.accion == Accion::Mas)
    }
}

/// Las chapitas de una tecla, en el idioma de la ventana.
fn chapas_de(t: menu_v2::Tecla, textos: &Catalogo) -> Vec<String> {
    t.chapas(
        &textos.t("v2menus-tecla-supr"),
        &textos.t("v2menus-tecla-clic"),
    )
}

/// Lo ancho que son unas chapitas puestas una detras de otra.
fn ancho_chapas(p: &Pintor, chapas: &[String], e: f32) -> f32 {
    if chapas.is_empty() {
        return 0.0;
    }
    let tam = menu_v2::TAM_CHAPA * e;
    chapas
        .iter()
        .map(|c| (p.medir_texto(c, tam).0 + 12.0 * e).max(menu_v2::CHAPA_MIN as f32 * e))
        .sum::<f32>()
        + menu_v2::CHAPA_HUECO as f32 * e * (chapas.len() - 1) as f32
}

/// Pinta unas chapitas con su borde derecho en `derecha`, centradas en
/// la fila. Sobre la fila elegida (azul) van en blanco.
fn pintar_chapas(
    p: &Pintor,
    tema: &Tema,
    chapas: &[String],
    derecha: f32,
    fila: Rect,
    e: f32,
    sobre_azul: bool,
) {
    let tam = menu_v2::TAM_CHAPA * e;
    let alto = menu_v2::CHAPA_ALTO as f32 * e;
    let y = fila.y as f32 + (fila.alto as f32 - alto) / 2.0;
    let mut x = derecha - ancho_chapas(p, chapas, e);
    for c in chapas {
        let (w, h) = p.medir_texto(c, tam);
        let ancho = (w + 12.0 * e).max(menu_v2::CHAPA_MIN as f32 * e);
        let caja = RectF { x, y, ancho, alto };
        if sobre_azul {
            p.rellenar_redondeado(caja, 5.0 * e, con_alfa(hex(0xffffff), 0.18));
        } else {
            p.rellenar_redondeado(caja, 5.0 * e, con_alfa(tema.texto, 0.10));
            p.rellenar_redondeado(encoger(caja, 1.0), 4.0 * e, tema.cabecera);
            p.rellenar_redondeado(encoger(caja, 1.0), 4.0 * e, con_alfa(tema.texto, 0.06));
        }
        let color = if sobre_azul {
            hex(0xffffff)
        } else {
            tema.apagado
        };
        p.texto_linea(
            c,
            x + (ancho - w) / 2.0,
            y + (alto - h) / 2.0,
            tam,
            ancho,
            color,
        );
        x += ancho + menu_v2::CHAPA_HUECO as f32 * e;
    }
}

/// La flecha de un submenu («›»), con su punta en `derecha`.
fn pintar_flecha_sub(p: &Pintor, derecha: f32, fila: Rect, e: f32, color: Color) {
    let cy = fila.y as f32 + fila.alto as f32 / 2.0;
    let (w, h) = (4.0 * e, 5.0 * e);
    let x = derecha - 4.0 * e;
    p.polilinea(&[(x - w, cy - h), (x, cy), (x - w, cy + h)], 1.8 * e, color);
}

/// Los tres puntos de «Más».
fn pintar_tres_puntos(p: &Pintor, caja: Rect, color: Color, e: f32) {
    let cy = caja.y as f32 + caja.alto as f32 / 2.0;
    let cx = caja.x as f32 + caja.ancho as f32 / 2.0;
    for dx in [-6.5f32, 0.0, 6.5] {
        p.circulo((cx + dx * e, cy), 1.9 * e, color);
    }
}

/// El azul de lo elegido (`#0060DF` de la maqueta) y el rojo de Borrar.
const AZUL_ELEGIDO: Color = hex(0x0060df);
const ROJO_MENU: Color = hex(0xe5534b);

/// Una lista de entradas en su recuadro ya colocado.
#[allow(clippy::too_many_arguments)]
fn pintar_lista_menu(
    p: &Pintor,
    tema: &Tema,
    textos: &Catalogo,
    escala: u32,
    lista: &[EntradaMenu],
    c: &menu_v2::Colocado,
    sobre: Option<usize>,
    azul: &dyn Fn(usize) -> bool,
) {
    let e = escala as f32 / 100.0;
    let tam = menu_v2::TAM * e;
    let caja = rf(c.caja);
    let radio = menu_v2::RADIO as f32 * e;
    // La sombra, un poco por debajo: el menu flota sobre la conversacion.
    p.rellenar_redondeado(
        RectF {
            y: caja.y + 6.0 * e,
            ..encoger(caja, -2.0 * e)
        },
        radio + 2.0 * e,
        con_alfa(hex(0x000000), 0.22),
    );
    p.rellenar_redondeado(caja, radio, tema.pildora_borde);
    p.rellenar_redondeado(encoger(caja, 1.0), (radio - 1.0).max(0.0), tema.cabecera);
    for y in &c.separadores {
        p.rellenar(
            RectF {
                x: caja.x + 12.0 * e,
                y: *y as f32,
                ancho: (caja.ancho - 24.0 * e).max(0.0),
                alto: e.max(1.0),
            },
            tema.separador,
        );
    }
    for (n, entrada) in lista.iter().enumerate() {
        let Some(fila) = c.filas.get(n).copied() else {
            continue;
        };
        let es_azul = azul(n);
        if es_azul {
            p.rellenar_redondeado(rf(fila), menu_v2::RADIO_FILA as f32 * e, AZUL_ELEGIDO);
        } else if sobre == Some(n) {
            p.rellenar_redondeado(
                rf(fila),
                menu_v2::RADIO_FILA as f32 * e,
                con_alfa(tema.texto, 0.08),
            );
        }
        let (color, color_icono) = if es_azul {
            (hex(0xffffff), hex(0xffffff))
        } else if entrada.peligro {
            (ROJO_MENU, ROJO_MENU)
        } else {
            (tema.texto, tema.apagado)
        };
        let lado = menu_v2::ICONO as f32 * e;
        let icono = RectF {
            x: fila.x as f32 + menu_v2::ICONO_X as f32 * e,
            y: fila.y as f32 + (fila.alto as f32 - lado) / 2.0,
            ancho: lado,
            alto: lado,
        };
        if entrada.accion == Accion::Mas {
            pintar_tres_puntos(
                p,
                Rect {
                    x: icono.x as i32,
                    y: icono.y as i32,
                    ancho: lado as u32,
                    alto: lado as u32,
                },
                color_icono,
                e,
            );
        } else if let Some(i) = entrada.icono {
            p.icono(i, icono, color_icono);
        }
        let derecha = fila.derecha() as f32 - menu_v2::DERECHA as f32 * e;
        let chapas = entrada
            .tecla
            .map(|t| chapas_de(t, textos))
            .unwrap_or_default();
        let ancho_ch = ancho_chapas(p, &chapas, e);
        if entrada.tecla == Some(menu_v2::Tecla::Sub) {
            pintar_flecha_sub(p, derecha, fila, e, color_icono);
        } else {
            pintar_chapas(p, tema, &chapas, derecha, fila, e, es_azul);
        }
        let (_, alto) = p.medir_texto(&entrada.texto, tam);
        let x = fila.x as f32 + menu_v2::TEXTO_X as f32 * e;
        let reserva = if entrada.tecla == Some(menu_v2::Tecla::Sub) {
            14.0 * e
        } else if ancho_ch > 0.0 {
            ancho_ch + menu_v2::HUECO_CHAPA as f32 * e
        } else {
            0.0
        };
        p.texto_linea(
            &entrada.texto,
            x,
            fila.y as f32 + (fila.alto as f32 - alto) / 2.0,
            tam,
            (derecha - reserva - x).max(0.0),
            color,
        );
    }
}

/// Lo que necesita cada fila: su rotulo y, si tiene, sus chapitas.
fn medir_lista(p: &Pintor, textos: &Catalogo, lista: &[EntradaMenu], e: f32) -> Vec<f32> {
    let tam = menu_v2::TAM * e;
    lista
        .iter()
        .map(|n| {
            let texto = p.medir_texto(&n.texto, tam).0;
            let extra = match n.tecla {
                Some(menu_v2::Tecla::Sub) => 14.0 * e + menu_v2::HUECO_CHAPA as f32 * e,
                Some(t) => {
                    ancho_chapas(p, &chapas_de(t, textos), e) + menu_v2::HUECO_CHAPA as f32 * e
                }
                None => 0.0,
            };
            texto + extra
        })
        .collect()
}

fn pintar_menu_abierto(p: &Pintor, c: &Pinta, m: &MenuAbierto, marco: Rect) {
    use menu_v2::Sitio;
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    if let Some(h) = m.hoja.as_ref() {
        hoja_adjuntar::pintar(p, c, h, m.ancla, marco);
        return;
    }
    let e = escala as f32 / 100.0;
    *m.necesita.borrow_mut() = medir_lista(p, textos, &m.entradas, e);
    *m.necesita_mas.borrow_mut() = medir_lista(p, textos, &m.mas, e);
    let colocado = m.colocado(marco, escala);
    // La fila de etiquetas, circulos sueltos encima del menu.
    if let Some((_, puesta)) = m.etiquetas.as_ref() {
        for (n, r) in colocado.emojis.iter().enumerate() {
            let emoji = chat::ETIQUETAS[n];
            let centro = (
                r.x as f32 + r.ancho as f32 / 2.0,
                r.y as f32 + r.alto as f32 / 2.0,
            );
            let radio = r.ancho as f32 / 2.0;
            let elegida = puesta.as_deref() == Some(emoji);
            let encima = matches!(m.sobre, Some(Sitio::Emoji(k)) if k == n)
                || matches!(m.elegido, Some(Sitio::Emoji(k)) if k == n);
            p.circulo(centro, radio, tema.pildora_borde);
            p.circulo(
                centro,
                radio - 1.0,
                if elegida {
                    con_alfa(AZUL_ELEGIDO, 0.85)
                } else if encima {
                    tema.boton_sobre
                } else {
                    tema.cabecera
                },
            );
            let tam = 18.0 * e;
            let (w, h) = p.medir_texto(emoji, tam);
            p.texto_linea(
                emoji,
                centro.0 - w / 2.0,
                centro.1 - h / 2.0,
                tam,
                w + 2.0,
                tema.texto,
            );
        }
    }
    let sobre = match m.sobre {
        Some(Sitio::Principal(n)) => Some(n),
        _ => None,
    };
    let mas = m.posicion_de_mas();
    let elegido = m.elegido;
    let mas_abierto = m.mas_abierto;
    pintar_lista_menu(
        p,
        tema,
        textos,
        escala,
        &m.entradas,
        &colocado,
        sobre,
        &|n| elegido == Some(Sitio::Principal(n)) || (mas_abierto && mas == Some(n)),
    );
    if let Some(sub) = m.colocado_mas(marco, escala) {
        let sobre = match m.sobre {
            Some(Sitio::Mas(n)) => Some(n),
            _ => None,
        };
        pintar_lista_menu(p, tema, textos, escala, &m.mas, &sub, sobre, &|n| {
            elegido == Some(Sitio::Mas(n))
        });
    }
}

/// «Pagina N», con el numero que se ve en el proyecto (desde 1).
fn rotulo_de_pagina(textos: &Catalogo, n: u32) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("n", n);
    textos.t_args("chat-pagina-n", &args)
}

/// **Que texto se lee en el telepronter.**
///
/// El movil llega al telepronter desde un texto que ya esta abierto, asi que
/// no tiene que preguntar. Aqui se entra por el clip, sin nada elegido, y
/// hay que ofrecer de donde sacarlo: lo escrito en la caja, las notas de la
/// conversacion y los `.txt`/`.md` adjuntos. Los mas nuevos primero, que son
/// los que se buscan.
fn menu_del_telepronter(a: &Abierto, textos: &Catalogo) -> Vec<EntradaMenu> {
    /// Cuantos se ofrecen. Un menu de cien lineas no se lee: se cierra.
    const CUANTOS: usize = 12;
    let mut v = Vec::new();
    if !a.borrador.trim().is_empty() {
        v.push(entrada(
            Some(&mi::DESCRIPTION),
            textos.t("chat-telepronter-borrador"),
            Accion::TelepronterBorrador,
        ));
    }
    for (i, m) in a.mensajes.iter().enumerate().rev() {
        if v.len() >= CUANTOS {
            break;
        }
        let Some(rotulo) = rotulo_para_leer(m) else {
            continue;
        };
        v.push(entrada(
            Some(&mi::SUBTITLES),
            rotulo,
            Accion::TelepronterMensaje(i),
        ));
    }
    if v.is_empty() {
        v.push(entrada(
            None,
            textos.t("chat-telepronter-sin-texto"),
            Accion::Aviso("chat-telepronter-sin-texto"),
        ));
    }
    v
}

/// Como se llama en el menu del telepronter un mensaje que se puede leer, o
/// `None` si ese mensaje no lleva texto ninguno.
///
/// Una nota escrita se ensena por su primera linea recortada —el nombre de
/// una nota es su principio— y un adjunto por su nombre de fichero.
fn rotulo_para_leer(m: &pixpin_proyecto::cuaderno::Mensaje) -> Option<String> {
    /// Lo que cabe en una linea de menu sin estirarlo a media pantalla.
    const CORTE: usize = 48;
    if m.ruta.is_none() {
        let linea = m.texto.lines().find(|l| !l.trim().is_empty())?.trim();
        let mut corto: String = linea.chars().take(CORTE).collect();
        if linea.chars().count() > CORTE {
            corto.push('…');
        }
        return Some(corto);
    }
    es_texto_leible(&m.nombre).then(|| m.nombre.clone())
}

/// Si ese nombre de fichero es de los que el telepronter sabe leer.
///
/// Solo texto plano: un `.docx` o un PDF no son una cadena de caracteres y
/// abrirlos aqui pintaria su ZIP en la pantalla.
fn es_texto_leible(nombre: &str) -> bool {
    let n = nombre.to_lowercase();
    n.ends_with(".txt") || n.ends_with(".md")
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

/// Los tres puntos de la cabecera, en el orden del movil. La biblioteca y
/// las conversaciones solo salen en «Mensajes guardados», que es la general.
///
/// Ni las lecciones ni la galeria de capturas: estan en los botones de la
/// lista de proyectos, como en el movil desde el 4-oct-2026.
fn menu_de_cabecera(textos: &Catalogo, en_guardados: bool) -> Vec<EntradaMenu> {
    let mut v = Vec::new();
    if en_guardados {
        v.push(entrada(
            Some(&mi::LIBRARY_MUSIC),
            textos.t("chat-biblioteca-audio"),
            Accion::Biblioteca,
        ));
        v.push(entrada(
            None,
            textos.t("chat-conversaciones"),
            Accion::Proyectos,
        ));
    }
    v.push(entrada(None, textos.t("chat-proyectos"), Accion::Proyectos));
    // Una caja flotante a la que soltar ficheros para este proyecto, con el
    // chat cerrado (el usuario, 2-oct).
    v.push(entrada(
        Some(&mi::ATTACH_FILE),
        textos.t("chat-anadir-arrastrando"),
        Accion::CajaDeSoltar,
    ));
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
    // La escala de la interfaz, justo encima de los ajustes: es un ajuste de
    // la ventana, no del proyecto, y es donde Telegram la pone.
    // Sin icono, como los ajustes: de los Material portados ninguno dice
    // «escala», y uno que diga otra cosa confunde mas que la falta de uno.
    v.push(entrada(None, textos.t("chat-escala"), Accion::Escalas));
    // Los grupos de ventanas (H9): en el movil salen en el menu de inicio.
    v.push(entrada(
        None,
        textos.t("chat-grupos-ventanas"),
        Accion::GruposVentanas,
    ));
    v.push(entrada(None, textos.t("chat-ajustes"), Accion::Ajustes));
    v
}

/// Los escalones de la escala de la interfaz, con el puesto marcado.
fn menu_de_escalas(textos: &Catalogo, actual: u32) -> Vec<EntradaMenu> {
    chat::ESCALAS
        .iter()
        .map(|cuanto| {
            let icono: &'static Icono = if *cuanto == actual {
                &mi::CHECK_BOX
            } else {
                &mi::CHECK_BOX_OUTLINE_BLANK
            };
            let mut rotulo = format!("{cuanto} %");
            if *cuanto == chat::ESCALA_POR_DEFECTO {
                // La del monitor lleva su nombre al lado: es a la que se
                // vuelve con Ctrl+0 y conviene reconocerla entre las demas.
                rotulo.push_str(&format!("  ({})", textos.t("chat-escala-normal")));
            }
            entrada(Some(icono), rotulo, Accion::Escala(*cuanto))
        })
        .collect()
}

/// Lo que se ensena al cambiar la escala con el teclado.
fn aviso_de_escala(textos: &Catalogo, cuanto: u32) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("cuanto", cuanto.to_string());
    textos.t_args("chat-escala-puesta", &args)
}

/// Apunta la escala elegida para la proxima vez. Un fallo al escribir no
/// puede estropear el cambio: la escala ya esta puesta, y lo unico que se
/// pierde es recordarla.
fn recordar_escala(ubicacion: &Ubicacion, estado: &mut pixpin_store::estado::Estado, cuanto: u32) {
    if estado.escala_interfaz == Some(cuanto) {
        return;
    }
    estado.escala_interfaz = Some(cuanto);
    if let Err(e) = pixpin_store::estado::guardar(ubicacion, estado) {
        tracing::warn!(?e, "no se pudo recordar la escala de la interfaz");
    }
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

/// Las horas a las que se puede pedir el aviso, como el dialogo del movil
/// (`DialogoDeRecordatorio`): unos cuantos atajos y nada de teclear.
///
/// Se ofrecen horas hechas y no un reloj donde escribir porque el movil hace
/// exactamente esto y porque escribir aqui pediria una ventana aparte: el
/// menu no tiene donde teclear. Lo que no cabe en cinco atajos se apunta a la
/// hora mas cercana y se corrige, que es un toque mas.
///
/// Y al final «Elegir la hora…» (v0.72), para lo que no cae en un atajo. En
/// el movil es un reloj de agujas; aqui es teclear `18:30` en una barra
/// encima de la caja de escribir, que con un teclado delante es lo rapido.
fn menu_de_recordatorio(i: usize, textos: &Catalogo) -> Vec<EntradaMenu> {
    let mut v: Vec<EntradaMenu> =
        crate::recordatorios::atajos(pixpin_shell::entorno::ahora_local_ms())
            .into_iter()
            .map(|(clave, cuando)| {
                entrada(
                    Some(&mi::ALARM),
                    textos.t(clave),
                    Accion::RecordarEn(i, cuando),
                )
            })
            .collect();
    v.push(entrada(
        Some(&mi::EDIT),
        textos.t("chat-recordar-hora"),
        Accion::RecordarAMano(i),
    ));
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

/// El menu de un mensaje: el de mantener pulsado en el movil, ordenado como
/// la v2 (`Menus2.dc.html`, a): lo diario arriba con su tecla, lo raro en
/// «Más» y Borrar en rojo y apartado. Solo salen las que valen para ESTE
/// mensaje, igual que en el movil. La etiqueta no va aqui: va en la fila de
/// emojis de encima (`MenuAbierto::con_etiquetas`).
fn menu_de_mensaje(a: &Abierto, i: usize, textos: &Catalogo) -> Vec<EntradaMenu> {
    use menu_v2::Tecla;
    use pixpin_proyecto::cuaderno::Clase;
    let Some(m) = a.mensajes.get(i) else {
        return Vec::new();
    };
    let ruta = ruta_del_mensaje(&a.raiz, &a.ficha.id, m).filter(|r| r.is_file());
    let mut v = Vec::new();

    // ── Lo diario ──
    v.push(
        entrada(
            Some(&mi::REPLY),
            textos.t("v2menus-responder"),
            Accion::Responder(i),
        )
        .con_tecla(Tecla::Letra('r')),
    );
    if !m.texto.trim().is_empty() && m.clase != Some(Clase::MiniApp) {
        v.push(
            entrada(
                Some(&mi::CONTENT_COPY),
                textos.t("chat-copiar"),
                Accion::Copiar(i),
            )
            .con_tecla(Tecla::Ctrl('c')),
        );
    }
    // Pinear: el pin se hace de un fichero, y una nota escrita no lo tiene
    // (ofrecerlo para contestar «no hay archivo» seria peor que no
    // ofrecerlo). La v2 lo pone aqui tambien para fotos y archivos, aunque
    // su burbuja lleve la pastilla: es lo que mas se hace con una foto.
    if ruta.is_some() {
        v.push(
            entrada(
                Some(&mi::PUSH_PIN),
                textos.t("chat-pinear"),
                Accion::Pinear(i),
            )
            .con_tecla(Tecla::Letra('p')),
        );
    }
    // Una lista de tareas no tiene fichero, pero si se pinea: el pin es la
    // misma lista, viva, y lo que se tacha alli se guarda en el mensaje.
    if m.clase == Some(Clase::MiniApp)
        && m.miniapp.as_deref() == Some(pixpin_proyecto::mini::TAREAS)
    {
        v.push(
            entrada(
                Some(&mi::PUSH_PIN),
                textos.t("chat-pinear"),
                Accion::PinearLista(i),
            )
            .con_tecla(Tecla::Letra('p')),
        );
    }
    // Lo que solo tiene Windows con una foto: dibujar en la propia burbuja
    // o en el lienzo grande.
    if matches!(a.vistas.get(i), Some(Some(Ojeada::Foto { .. }))) {
        v.push(
            entrada(
                Some(&mi::DRAW),
                textos.t("v2menus-anotar-encima"),
                Accion::FotoAqui(i),
            )
            .con_tecla(Tecla::Letra('a')),
        );
        v.push(entrada(
            Some(&mi::EDIT),
            textos.t("menu-foto-lienzo"),
            Accion::FotoEnLienzo(i),
        ));
    }
    // Su descripcion, el texto bajo la foto (`descripcion`).
    if descripcion::se_puede(m) {
        let clave = if m.texto.trim().is_empty() {
            "chat-describir"
        } else {
            "chat-describir-editar"
        };
        v.push(
            entrada(Some(&mi::SUBTITLES), textos.t(clave), Accion::Describir(i))
                .con_tecla(Tecla::Letra('e')),
        );
    }
    // Una nota escrita o un `.md`, en el editor de notas (H12).
    if crate::notas_md::se_edita(m, ruta.as_deref()) {
        v.push(
            entrada(
                Some(&mi::EDIT),
                textos.t("chat-editar-nota"),
                Accion::EditarNota(i),
            )
            .con_tecla(Tecla::Letra('n')),
        );
    }
    if m.clase == Some(Clase::Voz) {
        // Con texto es VOLVER a pasarlo (`MensajesActivity.kt:2620-2626`).
        // Sin texto se ofrece tambien: la nota ya no se pasa a texto sola
        // (lo pidio el usuario) y el menu es donde se busca. Solo con el
        // audio en este equipo: sin el, no hay nada que escuchar.
        if crate::voz::transcripcion_de(m).is_some() {
            v.push(
                entrada(
                    Some(&mi::SUBTITLES),
                    textos.t("chat-transcribir-otra-vez"),
                    Accion::Transcribir(i),
                )
                .con_tecla(Tecla::Letra('t')),
            );
        } else if ruta.is_some() {
            v.push(
                entrada(
                    Some(&mi::SUBTITLES),
                    textos.t("chat-transcribir"),
                    Accion::Transcribir(i),
                )
                .con_tecla(Tecla::Letra('t')),
            );
        }
    }
    // Con dos o mas paginas del PDF elegidas (esta entre ellas): juntarlas
    // en un lienzo. Solo entonces, como la caja de lo marcado del movil.
    if a.marcados.contains(&m.id) && peticion_de_fusion(a).is_some() {
        v.push(entrada(
            Some(&mi::LIBRARY_ADD),
            textos.t("fusionar-paginas"),
            Accion::FusionarPaginas,
        ));
    }

    // ── Mandarlo a otro sitio y acordarse ──
    v.push(
        entrada(
            Some(&mi::FORWARD),
            textos.t("chat-reenviar"),
            Accion::Reenviar(vec![i]),
        )
        .con_tecla(Tecla::Letra('f'))
        .separada(),
    );
    v.push(entrada(
        Some(&mi::IOS_SHARE),
        textos.t("chat-compartir"),
        Accion::Compartir(i),
    ));
    // La hora a la que avisar. Con una puesta, la entrada pasa a quitarla en
    // vez de volver a preguntar, como en el movil (`guardados_recordar_quitar`).
    // En una nota de voz, recordar es **llamarse** (B11).
    let recordado = crate::recordatorios::hora_de(m).is_some();
    let es_voz = m.clase == Some(Clase::Voz);
    let recordar = entrada(
        Some(if es_voz && !recordado {
            &mi::CALL
        } else {
            &mi::ALARM
        }),
        textos.t(if recordado {
            "chat-recordatorio-quitar"
        } else if es_voz {
            "chat-llamada-secreta"
        } else {
            "chat-recordar"
        }),
        if recordado {
            Accion::Olvidar(i)
        } else {
            Accion::Recordar(i)
        },
    );
    // Recordar abre las horas: lleva la flecha de submenu.
    v.push(if recordado {
        recordar
    } else {
        recordar.con_tecla(Tecla::Sub)
    });
    // **De un mensaje, una leccion** (`MensajesActivity.kt`, 3-oct). No de un
    // archivo (8-oct-2026, el usuario: «las lecciones no aceptan archivos»):
    // de su texto, su nota de voz o su foto.
    if !crate::lecciones::almacen::es_leccion(m) && m.clase != Some(Clase::Archivo) {
        v.push(
            entrada(
                Some(&mi::LIGHTBULB),
                textos.t("chat-hacer-leccion"),
                Accion::HacerLeccion(i),
            )
            .con_tecla(Tecla::Letra('l')),
        );
    }

    // ── «Más»: lo que se usa poco ──
    v.push(
        entrada(None, textos.t("v2menus-mas"), Accion::Mas)
            .con_tecla(Tecla::Sub)
            .separada(),
    );
    // Cambiar el nombre de cualquier archivo, audios incluidos.
    if renombrar::se_puede(m) {
        v.push(
            entrada(
                Some(&mi::EDIT),
                textos.t("chat-renombrar"),
                Accion::Renombrar(i),
            )
            .con_tecla(Tecla::F2)
            .en_mas(),
        );
    }
    v.push(
        entrada(
            Some(&mi::FLAG),
            textos.t(if m.fijado {
                "chat-soltar"
            } else {
                "chat-fijar"
            }),
            Accion::Fijar(i),
        )
        .en_mas(),
    );
    if let Some(ruta) = &ruta {
        // Sin «Abrir aqui» (8-oct-2026, el usuario): es lo que ya hace pulsar
        // la burbuja. Ni «Enviar por Wi-Fi»: vive en Compartir.
        v.push(
            entrada(
                Some(&mi::LAUNCH),
                textos.t("chat-abrir-con"),
                Accion::AbrirCon(i),
            )
            .en_mas(),
        );
        if es_pdf(ruta) {
            // Sale para cualquier PDF: saber si se puede aligerar exige
            // leerlo entero, y hacerlo al ABRIR un menu lo dejaria parado.
            v.push(
                entrada(
                    Some(&mi::COMPRESS),
                    textos.t("chat-aligerar"),
                    Accion::Aligerar(i),
                )
                .en_mas(),
            );
        }
    }
    // Unir al proyecto, o devolverle la hoja que se le quito. Se mira en el
    // disco al ABRIR el menu, como en el movil. En «Mensajes guardados» no
    // se puede: alli habria que elegir proyecto, que es «Reenviar».
    if !a.ficha.es_guardados() && se_puede_unir(m) {
        if !esta_en_las_hojas(a, i) {
            let (clave, accion) = if ya_esta_unido(m) {
                ("chat-devolver", Accion::Devolver(i))
            } else {
                ("chat-unir", Accion::Unir(i))
            };
            v.push(entrada(Some(&mi::LIBRARY_ADD), textos.t(clave), accion).en_mas());
        }
        // La otra salida del movil para un PDF: cada pagina como una foto
        // clavada en su lienzo.
        if m.clase == Some(Clase::Archivo) {
            v.push(
                entrada(
                    Some(&mi::IMAGE),
                    textos.t("unir-pdf-como-imagenes"),
                    Accion::ComoImagenes(i),
                )
                .en_mas(),
            );
        }
    } else if a.ficha.es_guardados() {
        v.push(
            entrada(
                Some(&mi::LIBRARY_ADD),
                textos.t("chat-unir"),
                Accion::Aviso("chat-no-hay-unir"),
            )
            .en_mas(),
        );
    }
    // Una hoja como pagina viva en una nota del proyecto (H12).
    if crate::notas_md::paginas_vivas::se_puede_insertar(m) {
        v.push(
            entrada(
                Some(&mi::DESCRIPTION),
                textos.t("chat-insertar-en-nota"),
                Accion::InsertarEnNota(i),
            )
            .en_mas(),
        );
    }
    if m.clase == Some(Clase::Dibujo) && m.referencia.is_some() {
        v.push(
            entrada(
                Some(&mi::IOS_SHARE),
                textos.t("exportar-chat"),
                Accion::Exportar(i),
            )
            .en_mas(),
        );
        v.push(entrada(None, textos.t("imprimir-chat"), Accion::Imprimir(i)).en_mas());
    }
    if m.en_buzon {
        v.push(
            entrada(
                Some(&mi::BOOKMARK_BORDER),
                textos.t("chat-rescatar"),
                Accion::Rescatar(i),
            )
            .en_mas(),
        );
    }
    // «Ver el dibujo entero» / «Ver solo la foto» (`alternarRecorte`): solo
    // si lo dibujado se sale de la foto; si no, las dos vistas son la misma.
    if let Some(Some(Ojeada::Foto { doc, trazos, .. })) = a.vistas.get(i)
        && dibujo_se_sale(*doc, *trazos)
    {
        v.push(
            entrada(
                Some(&mi::CROP),
                textos.t(if solo_la_foto(m) {
                    "chat-ver-dibujo-entero"
                } else {
                    "chat-solo-la-foto"
                }),
                Accion::AlternarRecorte(i),
            )
            .en_mas(),
        );
    }
    if a.mensajes
        .iter()
        .any(|o| o.responde_a.as_deref() == Some(m.id.as_str()))
    {
        v.push(entrada(Some(&mi::FORUM), textos.t("chat-ver-hilo"), Accion::Hilo(i)).en_mas());
    }
    // La letra, como en el movil, para toda nota con su audio detras.
    if es_voz && ruta.is_some() {
        v.push(entrada(Some(&mi::LYRICS), textos.t("chat-letra"), Accion::Letra(i)).en_mas());
    }
    v.push(
        entrada(
            Some(&mi::CHECK_BOX),
            textos.t("v2menus-elegir-varios"),
            Accion::Elegir(i),
        )
        .con_tecla(Tecla::CtrlClic)
        .separada()
        .en_mas(),
    );

    // ── Borrar, en rojo y aparte ──
    let mut borrar = entrada(
        Some(&mi::DELETE),
        textos.t("chat-borrar"),
        Accion::Borrar(vec![i]),
    )
    .con_tecla(Tecla::Supr)
    .separada();
    borrar.peligro = true;
    v.push(borrar);
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
    // Un lienzo que llego de la lista de un proyecto no lleva `ruta`, solo
    // su codigo (`referencia`): su dibujo esta en `lienzos/` (el usuario,
    // 1-oct: «algunos canvas me dicen que el mensaje no tiene archivo»).
    let ruta_de =
        |a: &Abierto, i: usize| fichero_del_mensaje(&a.raiz, &a.ficha.id, a.mensajes.get(i)?);
    match accion {
        Accion::Aviso(clave) => Efecto::Aviso(cx.textos.t(clave)),
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
        // Una nota de voz es una llamada: primero, quien llama (v0.98.6).
        Accion::Recordar(i) => Efecto::Menu(quien_llama::menu_de_las_horas(
            a.mensajes.get(i),
            i,
            &carpeta,
            cx.textos,
        )),
        Accion::QuienLlama(i) => {
            a.hora_a_mano = None;
            a.quien_llama = a
                .mensajes
                .get(i)
                .map(|m| (m.id.clone(), quien_llama::nombre_actual(&carpeta, m)));
            Efecto::Cambio
        }
        Accion::RecordarAMano(i) => {
            a.quien_llama = None;
            a.hora_a_mano = a.mensajes.get(i).map(|m| (m.id.clone(), String::new()));
            Efecto::Cambio
        }
        // La hora viaja en el mensaje, como en el movil, asi que ponerla es
        // reescribirlo; al vigia se le dice aparte porque vive en el hilo
        // principal y no lee cuadernos.
        Accion::RecordarEn(i, cuando_local) => {
            let cuando = crate::recordatorios::de_local_a_utc(cuando_local);
            let hecho = reescribir(a, i, &|m| crate::recordatorios::poner_hora(m, cuando));
            if !matches!(hecho, Efecto::Cambio) {
                return hecho;
            }
            if let Some(m) = a.mensajes.get(i) {
                crate::recordatorios::programar(&carpeta, &m.id, &m.resumen(), cuando);
                quien_llama::al_poner_la_hora(raiz, &carpeta, m);
            }
            let mut args = fluent_bundle::FluentArgs::new();
            args.set(
                "cuando",
                crate::recordatorios::cuando_legible(
                    cuando_local,
                    pixpin_shell::entorno::ahora_local_ms(),
                ),
            );
            Efecto::Aviso(cx.textos.t_args("chat-recordatorio-puesto", &args))
        }
        Accion::Olvidar(i) => {
            let hecho = reescribir(a, i, &|m| crate::recordatorios::quitar_hora(m));
            if !matches!(hecho, Efecto::Cambio) {
                return hecho;
            }
            if let Some(m) = a.mensajes.get(i) {
                crate::recordatorios::cancelar(&m.id);
            }
            Efecto::Aviso(cx.textos.t("chat-recordatorio-quitado"))
        }
        // Se escribe el valor siempre, tambien el `true` de por defecto: asi
        // el otro aparato recibe la decision aunque la suya fuera otra.
        // No hay que volver a leer la vista: el encuadre sale del mensaje.
        Accion::AlternarRecorte(i) => reescribir(a, i, &|m| {
            let nuevo = !solo_la_foto(m);
            m.resto
                .insert("soloLaFoto".into(), serde_json::Value::Bool(nuevo));
        }),
        // Con ficheros detras, el panel Compartir de Windows (el del
        // Explorador), que es la hoja de compartir del movil. Si el mensaje
        // esta entre los elegidos, se comparten todos los elegidos: es lo que
        // se espera al pulsar «Compartir» sobre uno de un grupo marcado.
        Accion::Compartir(i) => {
            let indices = if a
                .mensajes
                .get(i)
                .is_some_and(|m| a.marcados.contains(&m.id))
            {
                indices_marcados(a)
            } else {
                vec![i]
            };
            compartir_mensajes(a, &indices, cx)
        }
        // El cuadro «Abrir con» de Windows, el del Explorador: elegir CON QUE
        // se abre, que es lo que dice la entrada. Abrirlo con lo de siempre
        // ya lo hace pulsar la burbuja.
        Accion::AbrirCon(i) => match ruta_de(a, i) {
            Some(ruta) => {
                match pixpin_shell::abrir_con_otra::abrir_con_otra(cx.ventana.handle(), &ruta) {
                    Ok(_) => Efecto::Nada,
                    Err(e) => fallo(&e),
                }
            }
            None => Efecto::Aviso(cx.textos.t("chat-sin-archivo")),
        },
        // Sacar a la pantalla: el fichero va a la ventana principal, que es
        // quien tiene los pines (imagen, video o ficha, segun lo que sea).
        Accion::PinearLista(i) => {
            let Some(m) = a.mensajes.get(i) else {
                return Efecto::Nada;
            };
            let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &a.ficha.id);
            let ruta = crate::pines::herramienta::ruta_de_lista(&carpeta, &m.id);
            if pixpin_shell::mensajero::enviar_ficheros(std::slice::from_ref(&ruta)) {
                Efecto::Aviso(cx.textos.t("chat-pineado"))
            } else {
                fallo(&"no contesta la ventana principal")
            }
        }
        Accion::Pinear(i) => match ruta_de(a, i) {
            Some(ruta) if pixpin_shell::mensajero::enviar_ficheros(std::slice::from_ref(&ruta)) => {
                Efecto::Aviso(cx.textos.t("chat-pineado"))
            }
            Some(_) => fallo(&"no contesta la ventana principal"),
            None => Efecto::Aviso(cx.textos.t("chat-sin-archivo")),
        },
        // Bloquean el hilo del chat mientras dura el dialogo, como abrir el
        // lienzo: es un cuadro modal de Windows sobre esta ventana.
        Accion::Exportar(i) | Accion::Imprimir(i) => {
            let Some(m) = a.mensajes.get(i) else {
                return Efecto::Nada;
            };
            let peticion = if matches!(accion, Accion::Exportar(_)) {
                crate::ventana_editor::exportar::Peticion::Exportar
            } else {
                crate::ventana_editor::exportar::Peticion::Imprimir
            };
            let nombre = nombre_de_la_fila(m, cx.textos);
            match crate::ventana_editor::exportar::desde_el_chat(
                peticion,
                cx.ventana.handle(),
                cx.textos,
                raiz,
                &a.ficha.id,
                m,
                &nombre,
            ) {
                Some(aviso) => Efecto::Aviso(aviso),
                None => Efecto::Cambio,
            }
        }
        Accion::InsertarEnNota(i) => {
            if let Some(m) = a.mensajes.get(i) {
                crate::notas_md::paginas_vivas::insertar_en_nota(cx.idioma, cx.ubicacion, &a.ficha.id, m);
            }
            Efecto::Nada
        }
        Accion::Renombrar(i) => {
            if let Some(m) = a.mensajes.get(i) {
                // Un audio se escribe sin su «.m4a», como en el movil: al
                // guardar se le vuelve a poner (`renombrar::con_su_extension`).
                let escrito = match m.nombre.trim() {
                    n if m.clase != Some(pixpin_proyecto::cuaderno::Clase::Dibujo) && !n.is_empty() => {
                        n.strip_suffix(".m4a").unwrap_or(n).to_string()
                    }
                    _ => nombre_de_la_fila(m, cx.textos),
                };
                a.renombrando_mensaje = Some((i, escrito));
                a.describiendo = false;
            }
            Efecto::Cambio
        }
        Accion::Describir(i) => {
            // Empieza con la que tiene, para retocarla y no reescribirla.
            if let Some(m) = a.mensajes.get(i) {
                a.renombrando_mensaje = Some((i, m.texto.trim().to_string()));
                a.describiendo = true;
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
        Accion::ComoImagenes(i) => unir_pdf(
            cx.ubicacion,
            a,
            i,
            cx.textos,
            crate::pdf_en_chat::Trabajo::ComoImagenes,
        ),
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
            // Como al pulsarla: en su lienzo, estrenandolo si hace falta.
            if crate::foto_anotada::abrir_en_su_lienzo(cx.ubicacion.raiz(), &a.ficha.id, &mut a.mensajes, i, cx.lienzo) {
                if let Some(m) = a.mensajes.get(i).cloned() {
                    a.vistas[i] = leer_vista(cx.ubicacion, &a.ficha.id, &m);
                }
            } else if let Some(ruta) = ruta_de(a, i) {
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
        // Clip → «Una pagina de un proyecto»: el proyecto, su hoja y
        // adjuntarla (`paginas`). Solo se ofrecen los que tienen hojas que
        // ensenar: uno sin ninguna daria un segundo menu vacio.
        Accion::AdjPagina => {
            let v: Vec<EntradaMenu> = menu_de_proyectos(cx.fichas, "", Accion::AdjPaginaDe)
                .into_iter()
                .filter(|e| match &e.accion {
                    Accion::AdjPaginaDe(id) => paginas::leer(raiz, id)
                        .is_some_and(|p| !paginas::elegibles(&p).is_empty()),
                    _ => false,
                })
                .collect();
            if v.is_empty() {
                Efecto::Aviso(cx.textos.t("chat-sin-paginas"))
            } else {
                Efecto::Menu(v)
            }
        }
        Accion::AdjPaginaDe(id) => {
            let hojas = paginas::leer(raiz, &id)
                .map(|p| paginas::elegibles(&p))
                .unwrap_or_default();
            if hojas.is_empty() {
                return Efecto::Aviso(cx.textos.t("chat-sin-paginas"));
            }
            Efecto::Menu(
                hojas
                    .into_iter()
                    .map(|h| {
                        let icono: &'static Icono = if h.pagina.is_some() {
                            &mi::MENU_BOOK
                        } else {
                            &mi::DRAW
                        };
                        entrada(
                            Some(icono),
                            rotulo_de_pagina(cx.textos, h.n),
                            Accion::AdjPaginaHoja(id.clone(), h.hoja),
                        )
                    })
                    .collect(),
            )
        }
        Accion::AdjPaginaHoja(desde, hoja) => {
            let sello = cuaderno::Sello {
                cuando: pixpin_shell::entorno::ahora_utc_ms(),
                numero: a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1,
                aparato: cx.identidad.to_string(),
                proyecto: a.ficha.id.clone(),
            };
            let rotulo = |n| rotulo_de_pagina(cx.textos, n);
            match paginas::adjuntar(raiz, &desde, &a.ficha.id, &hoja, rotulo, &sello) {
                Ok(m) => {
                    a.vistas.push(leer_vista(cx.ubicacion, &a.ficha.id, &m));
                    a.mensajes.push(m);
                    a.scroll = None;
                    Efecto::Cambio
                }
                Err(e) => fallo(&e),
            }
        }
        // Los ajustes viven en el hilo principal (vuelven a registrar los
        // atajos al cerrarse): se le pide con el mismo `WM_COMMAND` que manda
        // la bandeja. Si no hay nadie escuchando, se dice por donde ir.
        Accion::Ajustes => {
            if pixpin_shell::mensajero::pedir_ventana_principal(
                pixpin_store::comandos::Comando::AbrirAjustes.id(),
            ) {
                Efecto::Nada
            } else {
                Efecto::Aviso(cx.textos.t("chat-ajustes-bandeja"))
            }
        }
        // El editor de notas vive en su hilo, como el lector: el chat sigue
        // usable detras, y lo guardado le llega por `refrescar` (H12).
        Accion::AdjNotaMd => {
            let destino = crate::notas_md::Destino::Nueva {
                proyecto: a.ficha.id.clone(),
            };
            crate::notas_md::abrir(cx.idioma, cx.ubicacion.clone(), destino);
            Efecto::Nada
        }
        Accion::EditarNota(i) => {
            let Some(m) = a.mensajes.get(i) else {
                return Efecto::Nada;
            };
            let destino = if m.clase == Some(cuaderno::Clase::Nota) {
                crate::notas_md::Destino::Mensaje {
                    proyecto: a.ficha.id.clone(),
                    codigo: m.codigo_unico(),
                }
            } else {
                match ruta_de(a, i) {
                    Some(ruta) => crate::notas_md::Destino::Fichero { ruta },
                    None => return Efecto::Aviso(cx.textos.t("chat-sin-archivo")),
                }
            };
            crate::notas_md::abrir(cx.idioma, cx.ubicacion.clone(), destino);
            Efecto::Nada
        }
        Accion::CajaDeSoltar => {
            caja_de_soltar(
                cx.textos,
                raiz.to_path_buf(),
                a.ficha.id.clone(),
                a.ficha.nombre.clone(),
                cx.identidad.to_string(),
            );
            Efecto::Nada
        }
        Accion::Lecciones => {
            // En la general, todas; en un proyecto, las suyas primero.
            let proyecto = (!a.ficha.es_guardados()).then(|| a.ficha.id.clone());
            let _ = proyecto;
            crate::lecciones::lista(cx.ubicacion.clone(), cx.idioma);
            Efecto::Nada
        }
        Accion::HacerLeccion(i) => {
            // Lo que paso suele estar ya en el chat: la leccion sale con su
            // texto (o lo que se dijo, o su nombre) y enlazada a el.
            if let Some(m) = a.mensajes.get(i) {
                let texto = m
                    .transcripcion
                    .as_deref()
                    .map(crate::voz::sin_marcas)
                    .filter(|t| !t.trim().is_empty())
                    .or_else(|| Some(m.texto.clone()).filter(|t| !t.trim().is_empty()))
                    .or_else(|| Some(m.nombre.clone()).filter(|t| !t.trim().is_empty()));
                crate::lecciones::nueva(
                    cx.ubicacion.clone(),
                    cx.idioma,
                    cx.identidad,
                    texto,
                    Some(m.id.clone()),
                    Some(a.ficha.id.clone()),
                );
            }
            Efecto::Nada
        }
        Accion::GruposVentanas => {
            let l = crate::grupos_ventanas::Lanzador {
                idioma: cx.idioma,
                ubicacion: cx.ubicacion.clone(),
                opciones: cx.lienzo,
            };
            match crate::grupos_ventanas::menu(Some(cx.ventana.handle().0 as isize), &l, cx.textos) {
                Some(aviso) => Efecto::Aviso(aviso),
                None => Efecto::Nada,
            }
        }
        Accion::Aligerar(i) => aligerar_el_pdf(cx.ubicacion, a, i, cx.textos),
        Accion::FusionarPaginas => {
            if peticion_de_fusion(a).is_some_and(|p| crate::fusionar_paginas::empezar(&a.raiz, p)) {
                a.marcados.clear();
                Efecto::Aviso(cx.textos.t("fusionar-paginas-empieza"))
            } else {
                Efecto::Aviso(cx.textos.t("fusionar-paginas-no"))
            }
        }
        Accion::Transcribir(i) => empezar_a_transcribir(cx.ubicacion, a, i, cx.textos, cx.idioma),
        Accion::Letra(i) => abrir_la_letra(a, i),
        Accion::Telepronter => Efecto::Menu(menu_del_telepronter(a, cx.textos)),
        Accion::Pronunciar => abrir_pronunciar(cx.ubicacion, a, cx.textos, cx.idioma),
        Accion::Conversacion => abrir_conversacion(cx.ubicacion, a, cx.textos, cx.idioma),
        Accion::TelepronterBorrador => {
            let texto = a.borrador.clone();
            abrir_telepronter(cx.ubicacion, a, texto, cx.textos, cx.idioma)
        }
        Accion::TelepronterMensaje(i) => match texto_para_leer(a, i) {
            Some(texto) => abrir_telepronter(cx.ubicacion, a, texto, cx.textos, cx.idioma),
            None => Efecto::Aviso(cx.textos.t("chat-telepronter-sin-texto")),
        },
        Accion::Biblioteca => {
            a.biblioteca = Some(abrir_biblioteca(cx.ubicacion, cx.fichas));
            Efecto::Cambio
        }
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
        | Accion::Mas
        | Accion::AdjCaptura
        | Accion::AdjImagen
        | Accion::AdjLienzo
        | Accion::AdjTabla
        | Accion::AdjDelMovil
        | Accion::Fondos
        | Accion::Fondo(_)
        // La escala es de la VENTANA, no del proyecto: la atiende el bucle,
        // que es quien tiene la variable y quien la recuerda.
        | Accion::Escalas
        | Accion::Escala(_)
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
    let _cerrojo = pixpin_proyecto::cuaderno::cerrojo();
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
        // Si se estaba aligerando, se para antes de borrarlo: si no, el
        // aligerado podria volver a ponerlo en su sitio justo despues.
        let Some(ruta) = ruta_del_mensaje(&a.raiz, &a.ficha.id, m).filter(|_| con_fichero) else {
            continue;
        };
        crate::aligerar::cancelar(&ruta);
        if ruta.is_file()
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
mod pruebas_voz {
    use super::*;
    use pixpin_proyecto::cuaderno::Mensaje;
    use pixpin_store::Idioma;

    fn nota(texto: &str) -> Mensaje {
        Mensaje {
            texto: texto.into(),
            ..Default::default()
        }
    }

    fn adjunto(nombre: &str) -> Mensaje {
        Mensaje {
            nombre: nombre.into(),
            ruta: Some(format!("adjuntos/{nombre}")),
            ..Default::default()
        }
    }

    #[test]
    fn el_telepronter_solo_lee_texto_plano() {
        assert!(es_texto_leible("guion.txt"));
        assert!(es_texto_leible("GUION.TXT"));
        assert!(es_texto_leible("apuntes.md"));
        // Casos negativos: lo que parece texto pero no lo es. Pintar el ZIP
        // de un .docx en la pantalla de leer seria peor que no ofrecerlo.
        assert!(!es_texto_leible("contrato.docx"));
        assert!(!es_texto_leible("plano.pdf"));
        assert!(!es_texto_leible("voz_1.m4a"));
        assert!(!es_texto_leible("txt"));
    }

    #[test]
    fn una_nota_se_ofrece_por_su_primera_linea_y_no_por_las_vacias() {
        assert_eq!(
            rotulo_para_leer(&nota("\n\n  hay que picar la pared  \ny luego")).as_deref(),
            Some("hay que picar la pared")
        );
    }

    #[test]
    fn una_nota_larga_se_recorta_con_puntos_suspensivos() {
        let largo = "a".repeat(80);
        let rotulo = rotulo_para_leer(&nota(&largo)).expect("una nota larga se ofrece");
        assert!(rotulo.ends_with('…'), "{rotulo}");
        assert_eq!(rotulo.chars().count(), 49, "48 caracteres y los puntos");
    }

    #[test]
    fn lo_que_no_lleva_texto_no_se_ofrece_para_leer() {
        // Casos negativos: una nota en blanco, una en solo espacios y un
        // adjunto que no es texto. Ofrecer cualquiera de los tres abriria
        // una pantalla negra sin nada que leer.
        assert_eq!(rotulo_para_leer(&nota("")), None);
        assert_eq!(rotulo_para_leer(&nota("   \n  ")), None);
        assert_eq!(rotulo_para_leer(&adjunto("foto.jpg")), None);
        assert_eq!(
            rotulo_para_leer(&adjunto("guion.txt")).as_deref(),
            Some("guion.txt")
        );
    }

    /// Una carpeta de datos vacia: ni `libvosk.dll` ni modelos, que es como
    /// esta cualquier PixPin recien instalado.
    fn sin_nada() -> Ubicacion {
        Ubicacion::Portable {
            raiz: std::env::temp_dir().join(format!(
                "pixpin-chat-voz-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            )),
        }
    }

    #[test]
    fn sin_el_reconocedor_se_dice_que_fichero_falta_y_donde_va() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let dicho = falta_para_transcribir(&sin_nada(), &textos, Idioma::Espanol)
            .expect("sin libvosk.dll no se puede transcribir");
        assert!(dicho.contains("libvosk.dll"), "{dicho}");
        assert!(dicho.contains("vosk"), "falta la carpeta: {dicho}");
        assert!(
            !dicho.starts_with("chat-"),
            "el aviso salio como su clave: {dicho}"
        );
    }

    #[test]
    fn cuando_falta_el_modelo_el_aviso_trae_su_nombre_y_su_enlace() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let dicho = razon_de_voz(
            &pixpin_voz::ErrorVoz::SinModelo {
                idioma: "es".into(),
                modelo: "vosk-model-small-es-0.42".into(),
                donde: "C:/datos/vosk".into(),
            },
            &textos,
        );
        assert!(dicho.contains("vosk-model-small-es-0.42"), "{dicho}");
        assert!(
            dicho.contains("https://alphacephei.com/vosk/models/"),
            "{dicho}"
        );
    }

    #[test]
    fn lo_dictado_se_pega_detras_con_un_espacio_y_sigue_la_frase() {
        assert_eq!(juntar_dictado("", "Hola que tal"), "Hola que tal");
        assert_eq!(
            juntar_dictado("llamar a", "Pedro mañana"),
            "llamar a pedro mañana"
        );
        assert_eq!(juntar_dictado("Listo.", "vale"), "Listo. Vale");
        assert_eq!(juntar_dictado("uno ", "Dos"), "uno dos");
        assert_eq!(juntar_dictado("nada", "   "), "nada");
    }

    #[test]
    fn ningun_aviso_de_voz_sale_como_su_propia_clave() {
        use pixpin_voz::ErrorVoz as E;
        for idioma in [Idioma::Espanol, Idioma::Ingles] {
            let textos = Catalogo::nuevo(idioma);
            let todos = [
                E::SinMotor {
                    donde: "C:/datos/vosk".into(),
                },
                E::SinModelo {
                    idioma: "es".into(),
                    modelo: "m".into(),
                    donde: "C:/datos/vosk".into(),
                },
                E::IdiomaSinModelo {
                    idioma: "ja".into(),
                },
                E::RutaImposible {
                    donde: "C:/ñ".into(),
                },
                E::ModeloIlegible {
                    donde: "C:/datos/vosk/m".into(),
                },
                E::NoEsAudio {
                    ruta: "voz_1.m4a".into(),
                },
                E::AudioVacio,
                E::NoSeEntiendeNada,
                E::Cancelada,
                E::SinReconocedorDeWindows {
                    idioma: "ja".into(),
                },
                E::SinMicrofono,
                E::SinModeloWhisper {
                    donde: "C:/datos/whisper/base".into(),
                    megas: 160,
                },
                E::SinOnnxRuntime {
                    donde: "C:/datos/whisper".into(),
                },
                E::Descarga {
                    que: "base-encoder.int8.onnx".into(),
                    detalle: "sin red".into(),
                },
                E::Onnx {
                    paso: "abrir el modelo",
                    detalle: "roto".into(),
                },
            ];
            for e in &todos {
                let dicho = razon_de_voz(e, &textos);
                assert!(!dicho.starts_with("chat-"), "falta una clave: {dicho}");
            }
        }
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
        // Un PowerPoint entra como su PDF (D10).
        assert!(se_puede_unir(&mensaje(Clase::Archivo, "Charla.pptx", "")));
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
    // Estrecha, se ve UNA cosa: la lista, o el proyecto que se abrio desde
    // ella. Es lo que hace Telegram al encoger la ventana, y lo que el
    // usuario echaba en falta: «si le doy click a un chat, entre al chat;
    // ahora no lo hace». Con `Ambas` la lista se quedaba siempre delante y
    // el proyecto abierto no tenia donde pintarse.
    //
    // Ancha no cambia nada: `vista` solo manda cuando no caben dos columnas.
    let vista = if abierto.is_some() {
        Vista::SoloChat
    } else {
        Vista::SoloLista
    };
    let mut d = Disposicion::calcular(marco.ancho, marco.alto, escala, ancho_lista, vista);
    let hay_barra = abierto.is_some_and(|a| {
        // Y la del reproductor, que sale con solo haber algo cargado aunque
        // no se este ni buscando ni contestando.
        (a.respondiendo.is_some()
            || a.renombrando_mensaje.is_some()
            || resultados_de_la_busqueda(a).is_some()
            || crate::audio::hay_algo())
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

/// **Compartir** esos mensajes: la hoja de compartir, la misma de toda la
/// aplicacion (`compartir.rs`), con lo que sea cada uno —un lienzo, una
/// foto, un PDF, una nota, una tabla— y los formatos que le tocan. Se abre
/// en su propio hilo: el chat sigue a lo suyo mientras se elige.
fn compartir_mensajes(a: &mut Abierto, indices: &[usize], cx: &Contexto) -> Efecto {
    // Lo dibujado a medias en una burbuja, a disco antes: la hoja lee de
    // alli y si no se quedaria fuera.
    apagar_lienzo(cx.ubicacion, a);
    let mensajes: Vec<pixpin_proyecto::cuaderno::Mensaje> = indices
        .iter()
        .filter_map(|&i| a.mensajes.get(i))
        .cloned()
        .collect();
    if mensajes.is_empty() {
        return Efecto::Nada;
    }
    crate::compartir::ventana::abrir(
        cx.idioma,
        cx.ubicacion.clone(),
        crate::compartir::Cosa::Mensajes {
            raiz: a.raiz.clone(),
            proyecto: a.ficha.id.clone(),
            titulo: a.ficha.nombre.clone(),
            mensajes,
        },
    );
    Efecto::Nada
}

/// Los mensajes elegidos, por su posicion y del mas viejo al mas nuevo.
fn indices_marcados(a: &Abierto) -> Vec<usize> {
    a.mensajes
        .iter()
        .enumerate()
        .filter(|(_, m)| a.marcados.contains(&m.id))
        .map(|(i, _)| i)
        .collect()
}

/// Si lo elegido son dos o mas paginas del PDF del proyecto, lo que hace
/// falta para fusionarlas (ver `fusionar_paginas`).
fn peticion_de_fusion(a: &Abierto) -> Option<crate::fusionar_paginas::Peticion> {
    let elegidos: Vec<&pixpin_proyecto::cuaderno::Mensaje> = a
        .mensajes
        .iter()
        .filter(|m| a.marcados.contains(&m.id))
        .collect();
    if elegidos.len() < 2 {
        return None;
    }
    crate::fusionar_paginas::de(
        crate::pdf_en_chat::documento_de(&a.raiz, &a.ficha.id),
        &a.ficha.id,
        &elegidos,
    )
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
/// Guarda `texto` como la descripcion del mensaje `i` (`descripcion`):
/// primero el cuaderno y, si eso sale bien, la pantalla. Vacio la quita.
fn guardar_descripcion(ubicacion: &Ubicacion, a: &mut Abierto, i: usize, texto: &str) {
    let Some(m) = a
        .mensajes
        .get(i)
        .and_then(|m| descripcion::con_descripcion(m, texto))
    else {
        return;
    };
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    match pixpin_proyecto::cuaderno::reemplazar(&carpeta, &m) {
        Ok(_) => a.mensajes[i] = m,
        Err(e) => tracing::warn!(?e, "no se pudo guardar la descripcion"),
    }
}

fn guardar_nombre_de_mensaje(ubicacion: &Ubicacion, a: &mut Abierto, i: usize, nombre: &str) {
    if nombre.is_empty() {
        return;
    }
    let Some(mut m) = a.mensajes.get(i).cloned() else {
        return;
    };
    // Un lienzo, como siempre; cualquier otro archivo conserva su extension
    // y su fichero en disco (`renombrar`).
    let nuevo = if m.clase == Some(pixpin_proyecto::cuaderno::Clase::Dibujo) {
        nombre.chars().take(80).collect()
    } else {
        renombrar::con_su_extension(&m.nombre, nombre)
    };
    if nuevo.is_empty() || nuevo == m.nombre {
        return;
    }
    m.nombre = nuevo;
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
    fn una_nota_de_voz_con_nombre_se_llama_asi_y_sigue_siendo_audio() {
        use super::nombre_de_la_voz;
        use pixpin_proyecto::cuaderno::{Clase, clase_de_nombre};
        assert_eq!(
            nombre_de_la_voz(Some("  reunion lunes "), 5),
            "reunion lunes.m4a"
        );
        assert_eq!(nombre_de_la_voz(Some("idea.M4A"), 5), "idea.M4A");
        assert_eq!(
            clase_de_nombre(&nombre_de_la_voz(Some("plan"), 5)),
            Clase::Voz
        );
        // Casos negativos: sin nombre, o en blanco, la hora de siempre.
        assert_eq!(nombre_de_la_voz(None, 42), "voz_42.m4a");
        assert_eq!(nombre_de_la_voz(Some("   "), 42), "voz_42.m4a");
    }

    #[test]
    fn un_lienzo_nuevo_lleva_el_nombre_pedido_y_su_dibujo_en_el_disco() {
        let raiz =
            std::env::temp_dir().join(format!("pixpin-lienzo-pedido-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let m = super::escribir_lienzo(&raiz, "p1", "PC01", 7, Some("Plano cocina")).unwrap();
        assert_eq!(m.nombre, "Plano cocina");
        assert_eq!(m.numero, 7);
        let id = m.referencia.clone().unwrap();
        assert!(pixpin_proyecto::almacen::lienzo(&raiz, "p1", &id).is_file());
        // Sin nombre, la fila se llama como su fichero, como desde el clip.
        let sin = super::escribir_lienzo(&raiz, "p1", "PC01", 8, Some("  ")).unwrap();
        assert!(sin.nombre.ends_with(".excalidraw"));
        assert_eq!(super::siguiente_numero_en(&raiz, "p1").unwrap(), 9);
        let _ = std::fs::remove_dir_all(&raiz);
    }

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

/// §2.2 de la lista del 22-sep: el menu sin lo que la burbuja ya tiene, y el
/// boton de texto de la nota de voz.
#[cfg(test)]
mod pruebas_menu_sin_repetir {
    use super::*;
    use pixpin_proyecto::cuaderno::{Clase, Mensaje};

    /// Un proyecto de verdad en una carpeta temporal, con esos mensajes y
    /// un fichero detras de cada uno que lleve `ruta`.
    fn abierto_con(nombre: &str, mensajes: Vec<Mensaje>) -> (Abierto, std::path::PathBuf) {
        let raiz = std::env::temp_dir().join(format!(
            "pixpin-menu-sin-repetir-{nombre}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        let u = Ubicacion::Portable { raiz: raiz.clone() };
        let ficha = pixpin_proyecto::almacen::Ficha::nueva("obra", 0, "PC");
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id);
        std::fs::create_dir_all(&carpeta).unwrap();
        for m in &mensajes {
            if let Some(r) = &m.ruta {
                std::fs::write(carpeta.join(r), b"x").unwrap();
            }
        }
        let mut a = abrir_proyecto(&u, &ficha);
        a.vistas = mensajes.iter().map(|_| None).collect();
        a.mensajes = mensajes;
        (a, raiz)
    }

    fn mensaje(clase: Clase, ruta: Option<&str>) -> Mensaje {
        Mensaje {
            id: "m1".into(),
            clase: Some(clase),
            texto: "algo".into(),
            nombre: ruta.unwrap_or_default().into(),
            ruta: ruta.map(str::to_string),
            ..Mensaje::default()
        }
    }

    fn acciones(a: &Abierto) -> Vec<Accion> {
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        menu_de_mensaje(a, 0, &textos)
            .into_iter()
            .map(|e| e.accion)
            .collect()
    }

    #[test]
    fn una_nota_de_voz_sin_texto_ofrece_pasarla_a_texto_tambien_en_el_menu() {
        let (a, raiz) = abierto_con("voz", vec![mensaje(Clase::Voz, Some("voz-1.m4a"))]);
        let v = acciones(&a);
        assert!(
            v.contains(&Accion::Transcribir(0)),
            "ya no se pasa sola al grabarla: el menu tiene que ofrecerlo"
        );
        assert!(
            v.contains(&Accion::Letra(0)),
            "la letra se ofrece con audio"
        );
        assert!(
            v.contains(&Accion::Pinear(0)),
            "su pastilla es la de texto: pinear queda en el menu"
        );
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn con_texto_el_menu_ofrece_volver_a_pasarla() {
        let mut m = mensaje(Clase::Voz, Some("voz-2.m4a"));
        m.transcripcion = Some("[0:00] hola".into());
        let (a, raiz) = abierto_con("voz-texto", vec![m]);
        assert!(acciones(&a).contains(&Accion::Transcribir(0)));
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn compartir_da_solo_los_ficheros_que_estan_en_este_equipo() {
        let (mut a, raiz) = abierto_con(
            "compartir",
            vec![
                mensaje(Clase::Imagen, Some("foto.png")),
                mensaje(Clase::Nota, None),
            ],
        );
        a.mensajes
            .push(mensaje(Clase::Archivo, Some("no-esta.pdf")));
        let rutas = crate::compartir::originales(&a.raiz, &a.ficha.id, &a.mensajes);
        assert_eq!(rutas.len(), 1, "{rutas:?}");
        assert!(rutas[0].ends_with("foto.png"));
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn una_nota_de_texto_no_tiene_ficheros_que_compartir() {
        let (a, raiz) = abierto_con("compartir-texto", vec![mensaje(Clase::Nota, None)]);
        assert!(crate::compartir::originales(&a.raiz, &a.ficha.id, &a.mensajes).is_empty());
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn sacar_una_burbuja_lleva_su_fichero_y_si_no_lo_que_dice() {
        let (mut a, raiz) = abierto_con("sacar", vec![mensaje(Clase::Imagen, Some("f.png"))]);
        assert!(matches!(
            carga_de_la_burbuja(&a, 0),
            Some(pixpin_pin::Carga::Fichero(r)) if r.ends_with("f.png")
        ));
        a.mensajes.push(mensaje(Clase::Nota, None));
        assert!(matches!(
            carga_de_la_burbuja(&a, 1),
            Some(pixpin_pin::Carga::Texto(t)) if t == "algo"
        ));
        let mut vacio = mensaje(Clase::Nota, None);
        vacio.texto = "   ".into();
        a.mensajes.push(vacio);
        assert!(
            carga_de_la_burbuja(&a, 2).is_none(),
            "una nada no se arrastra"
        );
        assert!(carga_de_la_burbuja(&a, 9).is_none());
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn la_burbuja_sale_de_la_ventana_solo_al_cruzar_su_borde() {
        assert!(!sale_de_la_ventana(Punto { x: 0, y: 0 }, 800, 600));
        assert!(!sale_de_la_ventana(Punto { x: 799, y: 599 }, 800, 600));
        assert!(sale_de_la_ventana(Punto { x: 800, y: 10 }, 800, 600));
        assert!(sale_de_la_ventana(Punto { x: -1, y: 10 }, 800, 600));
        assert!(sale_de_la_ventana(Punto { x: 10, y: 600 }, 800, 600));
    }

    #[test]
    fn una_foto_con_dibujo_fuera_alterna_entre_entero_y_solo_la_foto() {
        let (mut a, raiz) = abierto_con("recorte", vec![mensaje(Clase::Imagen, Some("r.png"))]);
        a.vistas = vec![Some(Ojeada::Foto {
            doc: (100.0, 100.0),
            dibujo: Vec::new(),
            trazos: Some((50.0, 50.0, 180.0, 90.0)),
        })];
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let entrada = |a: &Abierto| {
            menu_de_mensaje(a, 0, &textos)
                .into_iter()
                .find(|e| e.accion == Accion::AlternarRecorte(0))
                .map(|e| e.texto)
        };
        assert_eq!(entrada(&a).as_deref(), Some("Ver el dibujo entero"));
        a.mensajes[0]
            .resto
            .insert("soloLaFoto".into(), serde_json::Value::Bool(false));
        assert_eq!(entrada(&a).as_deref(), Some("Ver solo la foto"));
        // Con el dibujo dentro de la foto las dos vistas son iguales: nada.
        a.vistas = vec![Some(Ojeada::Foto {
            doc: (100.0, 100.0),
            dibujo: Vec::new(),
            trazos: Some((10.0, 10.0, 90.0, 90.0)),
        })];
        assert_eq!(entrada(&a), None);
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn una_nota_de_voz_sin_su_audio_aqui_no_ofrece_pasarla_a_texto() {
        // El fichero no se crea: la nota apunta a un audio de otro aparato.
        let (a, raiz) = abierto_con("voz-sin-audio", vec![]);
        let mut a = a;
        a.mensajes = vec![mensaje(Clase::Voz, Some("voz-ausente.m4a"))];
        a.vistas = vec![None];
        assert!(!acciones(&a).contains(&Accion::Transcribir(0)));
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn una_foto_o_un_archivo_se_pinean_tambien_desde_el_menu() {
        // v2 (`Menus2.dc.html`): «Pinear · P» es de lo diario, tambien con
        // la pastilla en la burbuja: es lo que mas se hace con una foto.
        for (clase, nombre) in [(Clase::Imagen, "foto"), (Clase::Archivo, "pdf")] {
            let (a, raiz) = abierto_con(nombre, vec![mensaje(clase, Some("cosa.bin"))]);
            let v = acciones(&a);
            assert!(
                v.contains(&Accion::Pinear(0)),
                "{nombre}: Pinear en el menu"
            );
            assert!(
                !v.contains(&Accion::Letra(0)),
                "{nombre}: la letra es de la voz"
            );
            let _ = std::fs::remove_dir_all(raiz);
        }
    }

    #[test]
    fn una_nota_escrita_sin_fichero_no_ofrece_un_pin_que_no_se_puede_hacer() {
        let (a, raiz) = abierto_con("nota", vec![mensaje(Clase::Nota, None)]);
        assert!(!acciones(&a).contains(&Accion::Pinear(0)));
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn que_burbujas_llevan_su_propio_boton_de_sacar() {
        let con = |c: Option<Clase>| {
            tiene_boton_de_pinear(&Mensaje {
                clase: c,
                ..Mensaje::default()
            })
        };
        assert!(con(Some(Clase::Imagen)));
        assert!(con(Some(Clase::Archivo)));
        assert!(con(Some(Clase::Dibujo)));
        assert!(!con(Some(Clase::Voz)));
        assert!(!con(Some(Clase::Nota)));
        assert!(!con(Some(Clase::MiniApp)));
        assert!(!con(None), "una nota vieja sin clase es una nota");
        assert!(
            !con(Some(Clase::Otra("CROQUIS".into()))),
            "sin fila no hay pastilla"
        );
    }

    #[test]
    fn el_boton_de_texto_pide_despliega_pliega_o_espera() {
        use BotonDeTexto as B;
        let boton = estado_del_boton_de_texto;
        assert_eq!(boton(false, false, false, true), B::Pedir);
        assert_eq!(boton(false, true, false, true), B::Desplegar);
        assert_eq!(boton(false, true, true, true), B::Plegar);
        // Moliendo manda sobre todo: no se puede pedir dos veces.
        assert_eq!(boton(true, true, true, true), B::Trabajando);
        // Sin texto y sin audio no hay nada que pedir.
        assert_eq!(boton(false, false, false, false), B::Oculto);
        // Con texto, aunque el audio falte, se puede leer.
        assert_eq!(boton(false, true, false, false), B::Desplegar);
    }

    #[test]
    fn la_letra_crece_y_mengua_de_dos_en_dos_sin_pasarse() {
        use pixpin_ui::chat::{LETRA_TAM, LETRA_TAM_MAX, LETRA_TAM_MIN};
        assert_eq!(otro_tamano_de_letra(LETRA_TAM, true), LETRA_TAM + 2);
        assert_eq!(otro_tamano_de_letra(LETRA_TAM, false), LETRA_TAM - 2);
        assert_eq!(otro_tamano_de_letra(LETRA_TAM_MAX, true), LETRA_TAM_MAX);
        assert_eq!(otro_tamano_de_letra(LETRA_TAM_MIN, false), LETRA_TAM_MIN);
    }

    #[test]
    fn elegir_la_hora_es_la_ultima_del_menu_de_recordar() {
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let v = menu_de_recordatorio(3, &textos);
        assert_eq!(
            v.last().map(|e| e.accion.clone()),
            Some(Accion::RecordarAMano(3))
        );
        assert!(
            v[..v.len() - 1]
                .iter()
                .all(|e| matches!(e.accion, Accion::RecordarEn(3, _))),
            "los atajos siguen delante"
        );
    }

    // ── v2-menus (`Menus2.dc.html`) ──

    fn menu_de(a: &Abierto) -> MenuAbierto {
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        MenuAbierto::nuevo(Punto { x: 50, y: 50 }, menu_de_mensaje(a, 0, &textos))
            .con_etiquetas(0, None)
    }

    #[test]
    fn v2_lo_diario_arriba_mas_aparte_y_borrar_el_ultimo_en_rojo() {
        let (a, raiz) = abierto_con("v2-orden", vec![mensaje(Clase::Imagen, Some("foto.png"))]);
        let m = menu_de(&a);
        let principal: Vec<&Accion> = m.entradas.iter().map(|e| &e.accion).collect();
        assert_eq!(principal[0], &Accion::Responder(0), "Responder, lo primero");
        let borrar = m.entradas.last().unwrap();
        assert_eq!(borrar.accion, Accion::Borrar(vec![0]));
        assert!(
            borrar.peligro && borrar.separada,
            "Borrar en rojo y apartado"
        );
        let n_mas = m.posicion_de_mas().expect("hay «Más»");
        assert_eq!(n_mas, m.entradas.len() - 2, "«Más» justo encima de Borrar");
        // Lo raro va al submenu: renombrar, fijar, abrir con.
        let mas: Vec<&Accion> = m.mas.iter().map(|e| &e.accion).collect();
        for a in [
            Accion::Renombrar(0),
            Accion::Fijar(0),
            Accion::AbrirCon(0),
        ] {
            assert!(mas.contains(&&a), "{a:?} en «Más»");
            assert!(!principal.contains(&&a), "{a:?} no arriba");
        }
        // Caso negativo: la etiqueta ya no es una entrada, es la fila.
        assert!(
            !principal
                .iter()
                .chain(mas.iter())
                .any(|a| matches!(a, Accion::Etiquetar(..)))
        );
        assert!(m.etiquetas.is_some());
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn v2_ninguna_tecla_se_repite_en_ningun_mensaje() {
        for (n, clase, ruta) in [
            (0, Clase::Imagen, Some("foto.png")),
            (1, Clase::Archivo, Some("libro.pdf")),
            (2, Clase::Voz, Some("voz-1.m4a")),
            (3, Clase::Nota, None),
            (4, Clase::Dibujo, None),
        ] {
            let (a, raiz) = abierto_con(
                &format!("v2-teclas-{n}"),
                vec![mensaje(clase.clone(), ruta)],
            );
            let m = menu_de(&a);
            let teclas: Vec<menu_v2::Tecla> = m
                .entradas
                .iter()
                .chain(m.mas.iter())
                .filter_map(|e| e.tecla)
                .collect();
            assert!(!menu_v2::hay_repetidas(&teclas), "{clase:?}: {teclas:?}");
            let _ = std::fs::remove_dir_all(raiz);
        }
    }

    #[test]
    fn v2_las_teclas_de_una_letra_hacen_su_entrada() {
        let (a, raiz) = abierto_con("v2-letras", vec![mensaje(Clase::Imagen, Some("foto.png"))]);
        let mut m = menu_de(&a);
        assert_eq!(m.caracter('r'), Respuesta::Hacer(Accion::Responder(0)));
        assert_eq!(m.caracter('P'), Respuesta::Hacer(Accion::Pinear(0)));
        assert_eq!(m.tecla(VK_C, true), Respuesta::Hacer(Accion::Copiar(0)));
        assert_eq!(
            m.tecla(VK_SUPR, false),
            Respuesta::Hacer(Accion::Borrar(vec![0]))
        );
        // F2 es de «Más», pero vale sin abrirlo.
        assert_eq!(m.tecla(0x71, false), Respuesta::Hacer(Accion::Renombrar(0)));
        // Casos negativos: una letra sin entrada no hace nada ni cierra, y
        // la C sola no es copiar.
        assert_eq!(m.caracter('z'), Respuesta::Sigue);
        assert_eq!(m.tecla(VK_C, false), Respuesta::Sigue);
        assert_eq!(m.tecla(VK_ESCAPE, false), Respuesta::Fuera);
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn v2_las_flechas_eligen_y_mas_se_abre_a_la_derecha() {
        use menu_v2::Sitio;
        let (a, raiz) = abierto_con("v2-flechas", vec![mensaje(Clase::Imagen, Some("foto.png"))]);
        let mut m = menu_de(&a);
        assert_eq!(m.tecla(VK_ABAJO, false), Respuesta::Sigue);
        assert_eq!(m.elegido, Some(Sitio::Principal(0)));
        assert_eq!(
            m.tecla(VK_ENTRAR, false),
            Respuesta::Hacer(Accion::Responder(0))
        );
        // Hasta «Más» y a la derecha: se abre y se elige su primera.
        let n = m.posicion_de_mas().unwrap();
        m.elegido = Some(Sitio::Principal(n));
        m.tecla(VK_DERECHA, false);
        assert!(m.mas_abierto);
        assert_eq!(m.elegido, Some(Sitio::Mas(0)));
        // Esc dentro de «Más» solo lo cierra; el siguiente cierra el menu.
        assert_eq!(m.tecla(VK_ESCAPE, false), Respuesta::Sigue);
        assert!(!m.mas_abierto);
        assert_eq!(m.tecla(VK_ESCAPE, false), Respuesta::Fuera);
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn v2_un_clic_en_mas_lo_abre_sin_cerrar_el_menu_y_las_etiquetas_se_ponen() {
        let (a, raiz) = abierto_con("v2-clic", vec![mensaje(Clase::Imagen, Some("foto.png"))]);
        let mut m = menu_de(&a);
        let marco = Rect {
            x: 0,
            y: 0,
            ancho: 1200,
            alto: 900,
        };
        let c = m.colocado(marco, 100);
        let fila = c.filas[m.posicion_de_mas().unwrap()];
        assert_eq!(
            m.pulsar(
                Punto {
                    x: fila.x + 5,
                    y: fila.y + 5
                },
                marco,
                100
            ),
            Respuesta::Sigue
        );
        assert!(m.mas_abierto);
        let sub = m.colocado_mas(marco, 100).expect("submenu a la vista");
        assert!(sub.caja.x > c.caja.x);
        // Una etiqueta de la fila de arriba se pone; puesta, se quita.
        let e = c.emojis[0];
        let dentro = Punto {
            x: e.x + 5,
            y: e.y + 5,
        };
        let estrella = pixpin_ui::chat::ETIQUETAS[0].to_string();
        assert_eq!(
            m.pulsar(dentro, marco, 100),
            Respuesta::Hacer(Accion::Etiquetar(0, Some(estrella.clone())))
        );
        let mut m2 = menu_de(&a).con_etiquetas(0, Some(estrella));
        assert_eq!(
            m2.pulsar(dentro, marco, 100),
            Respuesta::Hacer(Accion::Etiquetar(0, None))
        );
        // Caso negativo: fuera de todo, se cierra.
        assert_eq!(
            m.pulsar(Punto { x: 1190, y: 890 }, marco, 100),
            Respuesta::Fuera
        );
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn v2_un_menu_sin_nada_raro_no_ensena_mas_vacio() {
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let m = MenuAbierto::nuevo(
            Punto { x: 0, y: 0 },
            vec![
                entrada(None, "uno".into(), Accion::Responder(0)),
                entrada(None, textos.t("v2menus-mas"), Accion::Mas),
            ],
        );
        assert_eq!(m.entradas.len(), 1);
        assert!(m.posicion_de_mas().is_none());
    }
}

#[cfg(test)]
mod pruebas_ir_al_chat {
    use super::*;

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
    fn los_tres_puntos_ya_no_llevan_lecciones_ni_galeria_ni_universo() {
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        for en_guardados in [false, true] {
            let v = menu_de_cabecera(&textos, en_guardados);
            assert!(
                !v.iter().any(|e| matches!(e.accion, Accion::Lecciones)),
                "estan en los botones de la lista, como en el movil"
            );
            // Caso negativo: lo demas sigue, con los ajustes al final.
            assert_eq!(v.last().map(|e| e.accion.clone()), Some(Accion::Ajustes));
        }
    }
}

/// **Cuanto mueve un aviso de la rueda**, en pixeles, sabiendo que `por_muesca`
/// es lo que mueve una muesca entera (el `delta` de Windows vale 120 por muesca).
///
/// Proporcional a lo que se giro, y no «una muesca por aviso»: un touchpad o
/// una rueda fina mandan muchos avisos pequenos (10, 30…) y cada uno movia
/// tres filas enteras; un roce se llevaba media pantalla. Lo reporto el
/// usuario el 2026-09-23: «con solo un poco de scroll se mueve demasiado».
/// Un aviso minimo mueve al menos un pixel, para que nunca se quede quieto.
fn giro_de_rueda(delta: i32, por_muesca: i32) -> i32 {
    let v = delta * por_muesca / 120;
    if v == 0 && delta != 0 {
        delta.signum()
    } else {
        v
    }
}

#[cfg(test)]
mod pruebas_rueda {
    use super::giro_de_rueda;

    #[test]
    fn una_muesca_mueve_lo_de_una_muesca_y_en_su_sentido() {
        assert_eq!(giro_de_rueda(120, 93), 93);
        assert_eq!(giro_de_rueda(-120, 93), -93);
    }

    #[test]
    fn un_roce_de_touchpad_mueve_poco_y_no_una_muesca_entera() {
        assert_eq!(giro_de_rueda(12, 93), 9);
        assert!(giro_de_rueda(30, 93) < 93 / 2);
    }

    #[test]
    fn el_aviso_mas_pequeno_mueve_al_menos_un_pixel() {
        assert_eq!(giro_de_rueda(1, 93), 1);
        assert_eq!(giro_de_rueda(-1, 93), -1);
        assert_eq!(giro_de_rueda(0, 93), 0);
    }
}

/// Un PDF en el chat: entra como un mensaje y nada mas (`pdf_en_chat`).
#[cfg(test)]
mod pruebas_pdf_en_el_chat {
    use super::*;
    use pixpin_proyecto::cuaderno::Clase;

    fn proyecto(etiqueta: &str) -> (Ubicacion, Abierto, std::path::PathBuf) {
        let raiz = std::env::temp_dir().join(format!(
            "pixpin-pdf-en-el-chat-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        let u = Ubicacion::Portable { raiz: raiz.clone() };
        let ficha = pixpin_proyecto::almacen::Ficha::nueva("obra", 0, "PC");
        std::fs::create_dir_all(pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id)).unwrap();
        let a = abrir_proyecto(&u, &ficha);
        (u, a, raiz)
    }

    fn pdf_de(raiz: &std::path::Path, paginas: usize) -> std::path::PathBuf {
        let img = pixpin_codec::imagen::ImagenRgba {
            ancho: 20,
            alto: 30,
            pixeles: [200, 200, 200, 255].repeat(600),
        };
        let ruta = raiz.join("entrante.pdf");
        let imgs = vec![img; paginas];
        std::fs::write(&ruta, pixpin_pdf::union::de_imagenes(&imgs).unwrap()).unwrap();
        ruta
    }

    #[test]
    fn soltar_un_pdf_deja_un_solo_mensaje_y_no_extrae_ninguna_pagina() {
        let (u, mut a, raiz) = proyecto("soltar");
        let pdf = pdf_de(&raiz, 100);
        let t = std::time::Instant::now();
        let hechos = meter_ficheros(&u, &mut a, "PC", std::slice::from_ref(&pdf), "");
        let tardo = t.elapsed();
        assert_eq!(hechos, 1);
        assert_eq!(a.mensajes.len(), 1, "un mensaje, no uno por pagina");
        assert_eq!(a.mensajes[0].clase, Some(Clase::Archivo));
        assert!(
            !a.mensajes.iter().any(|m| m.clase == Some(Clase::Pagina)),
            "ninguna pagina como mensaje"
        );
        let archivos = pixpin_proyecto::almacen::carpeta(u.raiz(), &a.ficha.id).join("archivos");
        let ficheros: Vec<_> = std::fs::read_dir(archivos).unwrap().flatten().collect();
        assert_eq!(ficheros.len(), 1, "solo el PDF, ninguna imagen extraida");
        // El hilo de la ventana no se para: pintar va en su hilo.
        assert!(
            tardo.as_millis() < 50,
            "soltar un PDF de 100 paginas tardo {tardo:?}"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_menu_de_un_pdf_ofrece_anadir_al_proyecto_y_como_imagenes() {
        let (u, mut a, raiz) = proyecto("menu");
        let pdf = pdf_de(&raiz, 2);
        meter_ficheros(&u, &mut a, "PC", std::slice::from_ref(&pdf), "");
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let acciones: Vec<Accion> = menu_de_mensaje(&a, 0, &textos)
            .into_iter()
            .map(|e| e.accion)
            .collect();
        assert!(acciones.iter().any(|x| matches!(x, Accion::Unir(0))));
        assert!(
            acciones
                .iter()
                .any(|x| matches!(x, Accion::ComoImagenes(0)))
        );
        // Caso negativo: una nota no se ofrece como imagenes.
        a.mensajes[0].clase = Some(Clase::Nota);
        let acciones: Vec<Accion> = menu_de_mensaje(&a, 0, &textos)
            .into_iter()
            .map(|e| e.accion)
            .collect();
        assert!(
            !acciones
                .iter()
                .any(|x| matches!(x, Accion::ComoImagenes(_)))
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }
}

/// Abrir una hoja-pagina de un proyecto: el lienzo con la pagina de fondo,
/// como en el movil (`abrirPaginaDePdf`), y con su dibujo en el mismo sitio
/// a los dos lados.
#[cfg(test)]
mod pruebas_hoja_pagina {
    use super::*;
    use pixpin_proyecto::cuaderno::Clase;

    fn raiz(etiqueta: &str) -> (std::path::PathBuf, pixpin_proyecto::almacen::Ficha) {
        let raiz = std::env::temp_dir().join(format!(
            "pixpin-hojapagina-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&raiz);
        let ficha = pixpin_proyecto::almacen::Ficha::nueva("Obra", 5, "PC01");
        let mut indice = pixpin_proyecto::almacen::Indice::default();
        indice.proyectos.push(ficha.clone());
        indice.guardar(&raiz).unwrap();
        std::fs::create_dir_all(pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id)).unwrap();
        (raiz, ficha)
    }

    /// Un PDF de `paginas` hojas de 20x30 (proporcion 1,5).
    fn pdf(ruta: &std::path::Path, paginas: usize) {
        let imgs: Vec<pixpin_codec::imagen::ImagenRgba> = (0..paginas)
            .map(|i| pixpin_codec::imagen::ImagenRgba {
                ancho: 20,
                alto: 30,
                pixeles: [(i % 250) as u8, 90, 200, 255].repeat(600),
            })
            .collect();
        std::fs::write(ruta, pixpin_pdf::union::de_imagenes(&imgs).unwrap()).unwrap();
    }

    /// Las hojas del proyecto como las ensena el chat.
    fn hojas(raiz: &std::path::Path, ficha: &str) -> Vec<pixpin_proyecto::cuaderno::Mensaje> {
        pixpin_proyecto::almacen::hojas_para_ensenar(raiz, ficha, "PC01")
    }

    fn proyecto_json(raiz: &std::path::Path, ficha: &str) -> serde_json::Value {
        let t = std::fs::read_to_string(
            pixpin_proyecto::almacen::carpeta(raiz, ficha).join("proyecto.json"),
        )
        .unwrap();
        serde_json::from_str(&t).unwrap()
    }

    #[test]
    fn soltar_un_pdf_no_crea_hojas_ni_imagenes_y_anadirlo_no_mete_paginas_en_el_chat() {
        let (raiz, ficha) = raiz("soltar");
        let u = Ubicacion::Portable { raiz: raiz.clone() };
        let mut a = abrir_proyecto(&u, &ficha);
        let entrante = raiz.join("plano.pdf");
        pdf(&entrante, 12);
        assert_eq!(
            meter_ficheros(&u, &mut a, "PC01", std::slice::from_ref(&entrante), ""),
            1
        );
        // Soltar: ni proyecto.json con hojas, ni imagenes, ni paginas.
        assert!(hojas(&raiz, &ficha.id).is_empty(), "soltar no crea hojas");
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id);
        assert!(!carpeta.join("lienzos").exists(), "ni dibujos");
        assert!(!carpeta.join("imagenes").exists(), "ni imagenes");
        // «Anadir al proyecto»: doce hojas-pagina en el proyecto...
        let id = a.mensajes[0].id.clone();
        crate::pdf_en_chat::unir(
            &raiz,
            &ficha,
            &carpeta.join(a.mensajes[0].ruta.clone().unwrap()),
            &id,
            "plano",
            1000,
            &|_, _| {},
        )
        .unwrap();
        let de_ensenar = hojas(&raiz, &ficha.id);
        assert_eq!(de_ensenar.len(), 12);
        assert!(
            de_ensenar
                .iter()
                .all(|m| m.clase == Some(Clase::Pagina) && m.referencia.is_none())
        );
        // ...que solo van a la galeria: el chat sigue con su mensaje del PDF.
        let a = abrir_proyecto(&u, &ficha);
        let en_el_chat = a.mensajes.iter().filter(|m| se_ve(&a, m)).count();
        assert_eq!(en_el_chat, 1, "las paginas no ensucian el chat");
        assert!(
            !carpeta.join("imagenes").exists(),
            "anadir tampoco extrae imagenes"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn abrir_una_pagina_crea_su_dibujo_una_sola_vez_y_la_segunda_lo_reutiliza() {
        let (raiz, ficha) = raiz("crear");
        let entrante = raiz.join("plano.pdf");
        pdf(&entrante, 3);
        crate::pdf_en_chat::unir(&raiz, &ficha, &entrante, "m1", "plano", 1000, &|_, _| {})
            .unwrap();
        let m = hojas(&raiz, &ficha.id)[1].clone();
        assert!(crate::pdf_en_chat::es_pagina_sin_dibujo(&m));
        let primera = preparar_hoja(&raiz, &ficha.id, &m, 1_789_000_000_000).unwrap();
        // El id con el formato del movil y el fichero donde lo busca la
        // sincronizacion (`lienzos/<dibujo>.excalidraw`).
        let p = proyecto_json(&raiz, &ficha.id);
        let hoja = &p["hojas"][1];
        assert_eq!(
            hoja["dibujo"], "dib-1789000000000",
            "el campo del movil: `dibujo`"
        );
        assert_eq!(hoja["pagina"], 1);
        assert_eq!(
            hoja["deMensaje"], "m1",
            "lo que ya tenia la hoja no se pierde"
        );
        assert!(
            p["hojas"][0].get("dibujo").is_none(),
            "las otras paginas no se tocan"
        );
        assert_eq!(
            primera.ruta,
            pixpin_proyecto::almacen::lienzo(&raiz, &ficha.id, "dib-1789000000000")
        );
        assert!(
            pixpin_motor2d::excalidraw::leer(&std::fs::read_to_string(&primera.ruta).unwrap())
                .is_ok()
        );
        assert!(
            matches!(primera.fondo, Some(crate::fondo_lienzo::Fuente::Pdf(ref f)) if f.pagina == 1)
        );
        // Segunda apertura, con el mensaje de antes (sin referencia en
        // memoria): la misma hoja, el mismo dibujo, y no se crea otro.
        let segunda = preparar_hoja(&raiz, &ficha.id, &m, 1_789_000_000_999).unwrap();
        assert_eq!(segunda.ruta, primera.ruta);
        let lienzos =
            std::fs::read_dir(pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id).join("lienzos"))
                .unwrap()
                .count();
        assert_eq!(lienzos, 1, "una sola vez");
        // Y al releer el proyecto, la hoja ya sale como dibujo sobre su
        // pagina: la galeria ensena pagina + dibujo.
        let releida = hojas(&raiz, &ficha.id)[1].clone();
        assert_eq!(releida.referencia.as_deref(), Some("dib-1789000000000"));
        assert_eq!(releida.pagina, Some(1));
        assert_eq!(
            crate::pdf_en_chat::dibujo_de_la_hoja(&raiz, &ficha.id, &m).as_deref(),
            Some("dib-1789000000000")
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn una_hoja_sin_pdf_origen_ni_proyecto_no_revienta() {
        let (raiz, ficha) = raiz("sinpdf");
        let mut m = pixpin_proyecto::cuaderno::Mensaje {
            id: "x".into(),
            clase: Some(Clase::Pagina),
            pagina: Some(4),
            ..Default::default()
        };
        // Sin proyecto.json: no hay donde apuntar el dibujo, no se abre.
        assert!(preparar_hoja(&raiz, &ficha.id, &m, 1).is_none());
        assert!(crate::pdf_en_chat::fondo_de_pagina(&raiz, &ficha.id, 4).is_none());
        // Con proyecto.json pero sin documento: se abre el dibujo sin fondo.
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id);
        std::fs::write(
            carpeta.join("proyecto.json"),
            r#"{"id":"p","nombre":"Obra","hojas":[{"id":"h4","nombre":"","pagina":4}]}"#,
        )
        .unwrap();
        let h = preparar_hoja(&raiz, &ficha.id, &m, 5).unwrap();
        assert!(h.fondo.is_none());
        // Caso negativo: una nota no es una pagina y no estrena dibujo.
        m.clase = Some(Clase::Nota);
        assert!(!crate::pdf_en_chat::es_pagina_sin_dibujo(&m));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// Un proyecto como lo trae el movil: `documento.pdf`, la hoja de la
    /// pagina 0 con su dibujo (`hoja-<proyecto>-0`, el id que pone
    /// `DrawEditorActivity`) y un rectangulo en la esquina de abajo a la
    /// derecha de la pagina, en SUS coordenadas: 1400 de ancho y, con papel
    /// de 20x30, 2100 de alto.
    fn del_movil(etiqueta: &str) -> (std::path::PathBuf, pixpin_proyecto::almacen::Ficha) {
        let (raiz, ficha) = raiz(etiqueta);
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id);
        pdf(&carpeta.join("documento.pdf"), 3);
        std::fs::write(
            carpeta.join("proyecto.json"),
            r#"{"id":"pr-1","nombre":"Obra","hojas":[
                {"id":"h-1-0","nombre":"","pagina":0,"dibujo":"hoja-pr-1-0","uid":"U0"},
                {"id":"h-1-1","nombre":"","pagina":1,"uid":"U1"}],"tocado":1}"#,
        )
        .unwrap();
        std::fs::create_dir_all(carpeta.join("lienzos")).unwrap();
        let rect = serde_json::json!({
            "type": "excalidraw", "version": 2, "source": "pixpin-android",
            "elements": [{
                "id": "esquina", "type": "rectangle",
                "x": 1300.0, "y": 2000.0, "width": 100.0, "height": 100.0,
                "angle": 0, "strokeColor": "#1e1e1e", "backgroundColor": "transparent",
                "fillStyle": "solid", "strokeWidth": 2, "strokeStyle": "solid",
                "roughness": 1, "opacity": 100, "groupIds": [], "seed": 7,
                "version": 3, "versionNonce": 9, "isDeleted": false,
                "boundElements": null, "locked": false, "extraDelMovil": "se conserva"
            }],
            "appState": {"viewBackgroundColor": "#ffffff"},
            "files": {}
        });
        std::fs::write(
            carpeta.join("lienzos/hoja-pr-1-0.excalidraw"),
            rect.to_string(),
        )
        .unwrap();
        (raiz, ficha)
    }

    #[test]
    fn lo_dibujado_en_el_movil_cae_en_la_misma_esquina_de_la_pagina_en_el_pc() {
        let (raiz, ficha) = del_movil("alinear");
        let m = hojas(&raiz, &ficha.id)
            .into_iter()
            .find(|m| m.pagina == Some(0))
            .unwrap();
        assert_eq!(m.referencia.as_deref(), Some("hoja-pr-1-0"));
        let h = preparar_hoja(&raiz, &ficha.id, &m, 1).unwrap();
        let Some(fuente) = h.fondo.clone() else {
            panic!("la pagina tiene que ir de fondo");
        };
        let mut fondo = crate::fondo_lienzo::FondoLienzo::nuevo(fuente, 4096);
        let hasta = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while std::time::Instant::now() < hasta && fondo.imagen().is_none_or(|i| i.ancho != 1400) {
            fondo.hay_novedades();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        // La pagina mide en el PC lo que en el movil: 1400 x 2100 en (0,0).
        assert_eq!(fondo.ancho(), 1400.0);
        assert!((fondo.alto() - 2100.0).abs() < 2.0, "alto {}", fondo.alto());
        // Y el rectangulo del movil cierra justo en su esquina.
        let caja = h.escena.caja().unwrap();
        assert!((caja.2 - fondo.ancho()).abs() < 3.0, "derecha {caja:?}");
        assert!((caja.3 - fondo.alto()).abs() < 3.0, "abajo {caja:?}");
        // La vista de la galeria usa las mismas medidas (con la pagina ya
        // pintada en su cache).
        let doc = pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id).join("documento.pdf");
        let mut vista = None;
        while vista.is_none() && std::time::Instant::now() < hasta {
            vista =
                crate::pdf_en_chat::pagina_pintada(&raiz, &doc, 0, crate::pdf_en_chat::ANCHO_VISTA);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let (_, w, alto) = vista.unwrap();
        assert_eq!(w, 1400.0);
        assert!((alto - 2100.0).abs() < 2.0);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn ida_y_vuelta_con_un_proyecto_del_movil_no_mueve_ni_pierde_nada() {
        let (raiz, ficha) = del_movil("idavuelta");
        let m = hojas(&raiz, &ficha.id)
            .into_iter()
            .find(|m| m.pagina == Some(0))
            .unwrap();
        let h = preparar_hoja(&raiz, &ficha.id, &m, 1).unwrap();
        // En el PC se dibuja algo al lado de lo del movil.
        let mut escena = h.escena.clone();
        let mut nuevo = escena.elementos[0].clone();
        nuevo.x = 100.0;
        nuevo.y = 100.0;
        escena.anadir(nuevo);
        assert!(guardar_hoja_dibujada(&h.ruta, &h.lienzo, &escena));
        // De vuelta al movil: lo suyo sigue donde estaba y con lo que el PC
        // no entiende; lo del PC, en sus coordenadas de pagina.
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&h.ruta).unwrap()).unwrap();
        let els = v["elements"].as_array().unwrap();
        let suyo = els.iter().find(|e| e["id"] == "esquina").unwrap();
        assert_eq!(suyo["x"].as_f64(), Some(1300.0));
        assert_eq!(suyo["y"].as_f64(), Some(2000.0));
        assert_eq!(suyo["extraDelMovil"], "se conserva");
        assert!(
            els.iter()
                .any(|e| e["x"].as_f64() == Some(100.0) && e["y"].as_f64() == Some(100.0))
        );
        // Y la hoja sigue apuntando al mismo dibujo y pagina.
        let p = proyecto_json(&raiz, &ficha.id);
        assert_eq!(p["hojas"][0]["dibujo"], "hoja-pr-1-0");
        assert_eq!(p["hojas"][0]["pagina"], 0);
        // La pagina 1, que el movil no habia abierto, estrena dibujo aqui y
        // queda apuntada con el campo que el movil lee.
        let m1 = hojas(&raiz, &ficha.id)
            .into_iter()
            .find(|m| m.pagina == Some(1))
            .unwrap();
        preparar_hoja(&raiz, &ficha.id, &m1, 1_789_000_000_000).unwrap();
        let p = proyecto_json(&raiz, &ficha.id);
        assert_eq!(p["hojas"][1]["dibujo"], "dib-1789000000000");
        assert_eq!(p["hojas"][1]["uid"], "U1", "su codigo unico no cambia");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn abrir_una_pagina_de_un_pdf_de_cien_no_para_el_hilo_de_la_ventana() {
        let (raiz, ficha) = raiz("cien");
        let entrante = raiz.join("cien.pdf");
        pdf(&entrante, 100);
        crate::pdf_en_chat::unir(&raiz, &ficha, &entrante, "m1", "cien", 1000, &|_, _| {}).unwrap();
        let m = hojas(&raiz, &ficha.id)[73].clone();
        // Lo que corre en el hilo de la ventana al pulsar la pagina: estrenar
        // su dibujo, leerlo y preparar el fondo, mas crear el fondo del
        // lienzo. Nada de abrir el PDF ni pintar la pagina.
        let medir = |m: &pixpin_proyecto::cuaderno::Mensaje, ahora: i64| {
            let t = std::time::Instant::now();
            let h = preparar_hoja(&raiz, &ficha.id, m, ahora).unwrap();
            let fondo = crate::fondo_lienzo::FondoLienzo::nuevo(h.fondo.unwrap(), 4096);
            (t.elapsed(), fondo)
        };
        let (primera, fondo) = medir(&m, 2);
        assert!(primera.as_millis() < 50, "primera apertura: {primera:?}");
        drop(fondo);
        // Con la vista previa ya pintada (lo normal: la galeria la ensenaba)
        // se lee ese PNG, y tampoco pasa de 50 ms.
        let doc = crate::pdf_en_chat::documento_de(&raiz, &ficha.id).unwrap();
        let hasta = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while crate::pdf_en_chat::pagina_pintada(&raiz, &doc, 73, crate::pdf_en_chat::ANCHO_VISTA)
            .is_none()
            && std::time::Instant::now() < hasta
        {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let (segunda, fondo) = medir(&m, 3);
        assert!(segunda.as_millis() < 50, "con vista previa: {segunda:?}");
        // Y ya con la proporcion de la hoja (la de la cabecera de la vista
        // previa), no con la supuesta: 20x30 de papel son 1400 x 2100.
        assert!((fondo.alto() - 2100.0).abs() < 2.0, "alto {}", fondo.alto());
        let _ = std::fs::remove_dir_all(&raiz);
    }
}

/// La descripcion de una foto (`descripcion`): lo escrito con una foto va
/// con ella, en su `texto`, y se puede cambiar despues.
#[cfg(test)]
mod pruebas_descripcion {
    use super::*;
    use pixpin_proyecto::cuaderno::Clase;

    fn proyecto(etiqueta: &str) -> (Ubicacion, Abierto, std::path::PathBuf) {
        let raiz = std::env::temp_dir().join(format!(
            "pixpin-descripcion-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        let u = Ubicacion::Portable { raiz: raiz.clone() };
        let ficha = pixpin_proyecto::almacen::Ficha::nueva("obra", 0, "PC");
        std::fs::create_dir_all(pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id)).unwrap();
        let a = abrir_proyecto(&u, &ficha);
        (u, a, raiz)
    }

    fn foto(raiz: &std::path::Path, nombre: &str) -> std::path::PathBuf {
        let img = pixpin_codec::imagen::ImagenRgba {
            ancho: 4,
            alto: 3,
            pixeles: [90, 140, 200, 255].repeat(12),
        };
        let ruta = raiz.join(nombre);
        std::fs::write(&ruta, pixpin_codec::codificar_png(&img).unwrap()).unwrap();
        ruta
    }

    fn del_disco(u: &Ubicacion, a: &Abierto) -> Vec<pixpin_proyecto::cuaderno::Mensaje> {
        pixpin_proyecto::cuaderno::Cuaderno::leer_de(&pixpin_proyecto::almacen::carpeta(
            u.raiz(),
            &a.ficha.id,
        ))
        .unwrap()
        .mensajes
    }

    #[test]
    fn el_texto_enviado_con_fotos_queda_como_descripcion_de_la_primera() {
        let (u, mut a, raiz) = proyecto("enviar");
        let rutas = vec![foto(&raiz, "a.png"), foto(&raiz, "b.png")];
        assert_eq!(
            meter_ficheros(&u, &mut a, "PC", &rutas, "  La pared norte  "),
            2
        );
        let guardados = del_disco(&u, &a);
        assert_eq!(guardados.len(), 2, "dos fotos y ninguna nota aparte");
        assert!(guardados.iter().all(|m| m.clase == Some(Clase::Imagen)));
        assert_eq!(guardados[0].texto, "La pared norte");
        assert!(
            guardados[1].texto.is_empty(),
            "el pie va en la primera, no en todas"
        );
        assert_ne!(guardados[0].id, guardados[1].id, "cada mensaje su id");
        // Y se ensena debajo de la foto, que es lo que pinta la burbuja.
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        assert_eq!(texto_de(&a.mensajes[0], true, &textos), "La pared norte");
        assert!(a.borrador.is_empty());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn caso_negativo_si_no_entra_ninguna_foto_el_texto_vuelve_a_la_caja() {
        let (u, mut a, raiz) = proyecto("ninguna");
        let rutas = vec![raiz.join("no-esta.png")];
        assert_eq!(meter_ficheros(&u, &mut a, "PC", &rutas, "no lo pierdas"), 0);
        assert_eq!(a.borrador, "no lo pierdas");
        assert!(a.mensajes.is_empty());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_cuadro_de_confirmar_se_lleva_lo_escrito_y_cancelar_lo_devuelve() {
        let (_u, mut a, raiz) = proyecto("cuadro");
        a.borrador = "mira esto\n".into();
        let p = Pendientes::con_lo_escrito(vec![foto(&raiz, "a.png")], Some(&mut a)).unwrap();
        assert_eq!(p.pie, "mira esto");
        assert!(a.borrador.is_empty());
        p.cancelar(Some(&mut a));
        assert_eq!(a.borrador, "mira esto");
        // Caso negativo: sin ficheros no hay cuadro y la caja no se toca.
        assert!(Pendientes::con_lo_escrito(Vec::new(), Some(&mut a)).is_none());
        assert_eq!(a.borrador, "mira esto");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_descripcion_se_cambia_despues_y_queda_en_el_cuaderno() {
        let (u, mut a, raiz) = proyecto("editar");
        meter_ficheros(&u, &mut a, "PC", &[foto(&raiz, "a.png")], "");
        assert!(a.mensajes[0].texto.is_empty());
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let acciones: Vec<Accion> = menu_de_mensaje(&a, 0, &textos)
            .into_iter()
            .map(|e| e.accion)
            .collect();
        assert!(acciones.contains(&Accion::Describir(0)));
        guardar_descripcion(&u, &mut a, 0, "  Fachada  ");
        assert_eq!(a.mensajes[0].texto, "Fachada");
        assert_eq!(del_disco(&u, &a)[0].texto, "Fachada");
        let reabierto = abrir_proyecto(&u, &a.ficha);
        assert_eq!(reabierto.mensajes[0].texto, "Fachada");
        // Vacia la quita.
        guardar_descripcion(&u, &mut a, 0, "");
        assert!(del_disco(&u, &a)[0].texto.is_empty());
        // Caso negativo: a una nota no se le ofrece descripcion.
        a.borrador = "una nota".into();
        guardar_nota(&u, &mut a, "PC").unwrap();
        let acciones: Vec<Accion> = menu_de_mensaje(&a, 1, &textos)
            .into_iter()
            .map(|e| e.accion)
            .collect();
        assert!(!acciones.iter().any(|x| matches!(x, Accion::Describir(_))));
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
