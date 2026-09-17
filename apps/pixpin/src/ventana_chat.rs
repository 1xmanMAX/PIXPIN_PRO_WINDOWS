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

use crate::caja_dibujo::hex;
use crate::overlay::Recursos;

/// Tamano con el que nace, en pixeles logicos (el de Telegram en escritorio).
const ANCHO_LOGICO: u32 = 1024;
const ALTO_LOGICO: u32 = 768;
const VK_ESCAPE: u32 = 0x1B;
const VK_RETROCESO: u32 = 0x08;
const VK_ENTRAR: u32 = 0x0D;
const VK_V: u32 = 0x56;

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
    texto_otro: Color,
    hora_mia: Color,
    hora_otra: Color,
    separador_dia: Color,
    texto_separador: Color,
}

const CLARO: Tema = Tema {
    // titleBg, titleButtonBgOver y titleButtonCloseBgOver del tema claro
    // por omision de Telegram («day-blue»).
    barra: hex(0xf1f1f1),
    boton_sobre: hex(0xe5e5e5),
    cerrar_sobre: hex(0xe81123),
    lista: hex(0xffffff),
    chat: hex(0xf1f1f1),
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
    burbuja_mia: hex(0xeffdde),
    burbuja_otra: hex(0xffffff),
    texto_mio: hex(0x000000),
    texto_otro: hex(0x000000),
    hora_mia: hex(0x6db566),
    hora_otra: hex(0xa0acb6),
    separador_dia: hex(0x6b8f5c),
    texto_separador: hex(0xffffff),
};

const OSCURO: Tema = Tema {
    // titleBgActive, titleButtonBgOver y titleButtonCloseBgOver del tema
    // «night» de Telegram.
    barra: hex(0x242f3d),
    boton_sobre: hex(0x2c3847),
    cerrar_sobre: hex(0xe92539),
    lista: hex(0x17212b),
    chat: hex(0x0e1621),
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
    burbuja_mia: hex(0x2b5278),
    burbuja_otra: hex(0x182533),
    texto_mio: hex(0xe4ecf2),
    texto_otro: hex(0xf5f5f5),
    hora_mia: hex(0x7da8d3),
    hora_otra: hex(0x6d7f8f),
    // msgServiceBg del tema oscuro, ya mezclado sobre el fondo del chat:
    // el original es semitransparente y se recalcula con el fondo de
    // pantalla, que aqui no existe.
    separador_dia: hex(0x1d2a38),
    texto_separador: hex(0xffffff),
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
pub fn abrir(recursos: &Recursos, textos: &Catalogo, ubicacion: &Ubicacion) -> Result<()> {
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

    let mut ventana =
        VentanaOverlay::nueva(marco).context("no se pudo abrir la ventana de chat")?;
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
    let indice = pixpin_proyecto::almacen::Indice::leer(ubicacion.raiz());
    let mut fichas: Vec<pixpin_proyecto::almacen::Ficha> =
        indice.ordenadas().into_iter().cloned().collect();
    let ahora = pixpin_shell::entorno::ahora_local_ms();
    // El codigo de este equipo va en cada nota que se escriba aqui: es lo
    // que hace que el movil sepa de donde vino.
    let identidad = pixpin_proyecto::identidad::Identidad::leer_o_crear(ubicacion.raiz(), "PC")
        .map(|i| i.yo.codigo())
        .unwrap_or_default();
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
    let mut abierto: Option<Abierto> = None;
    // El menu del clip, cuando esta desplegado. Vive fuera de `abierto`
    // porque cambiar de conversacion tiene que cerrarlo.
    let mut menu_adjuntar = false;
    // Las fotos ya leidas y reducidas, para el panel de informacion. Sobrevive
    // a cambiar de proyecto a proposito: van por ruta, y volver al anterior no
    // tiene por que releerlas del disco.
    let mut miniaturas = crate::miniaturas::Miniaturas::nuevo();
    // Los ficheros que esperan un si o un no. Meter algo en el cuaderno no se
    // deshace, asi que se pregunta antes.
    let mut pendientes: Option<Pendientes> = None;
    // Antes de maximizar, para poder volver.
    let mut antes_de_maximizar: Option<Rect> = None;
    let mut hay_que_pintar = true;

    loop {
        pixpin_shell::overlay::bombear_pendientes();
        let mut cerrar = false;
        let disposicion =
            Disposicion::calcular(marco.ancho, marco.alto, escala, ancho_lista, Vista::Ambas);
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
                    } else if menu_adjuntar {
                        // Con el menu desplegado, el primer clic es suyo: o
                        // elige una entrada o lo cierra. Que ademas hiciera
                        // lo que hubiera debajo seria dispararle al usuario
                        // por querer salir del menu.
                        let elegido = abierto.as_ref().and_then(|a| {
                            menu_del_clip(&disposicion, a, marco, escala).fila_en(l, escala)
                        });
                        menu_adjuntar = false;
                        if let Some(n) = elegido.and_then(|n| menu::Entrada::TODAS.get(n).copied())
                        {
                            let rutas = match n {
                                menu::Entrada::Imagen => {
                                    pixpin_shell::elegir::pedir_imagenes(ventana.handle())
                                }
                                menu::Entrada::Archivo => {
                                    pixpin_shell::elegir::pedir_ficheros(ventana.handle())
                                }
                                menu::Entrada::Lienzo | menu::Entrada::Tabla => Vec::new(),
                            };
                            match n {
                                // Un lienzo o una tabla nacen vacios: no hay
                                // nada que ensenar en un cuadro de confirmar,
                                // y pedirlo dos veces seria un estorbo.
                                menu::Entrada::Lienzo | menu::Entrada::Tabla => {
                                    if let Some(a) = abierto.as_mut() {
                                        let hecho = if n == menu::Entrada::Tabla {
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
                                _ => pendientes = Pendientes::de(rutas),
                            }
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
                            }
                            hay_que_pintar = true;
                        }
                    } else if abierto.is_some() && disposicion.cabecera_chat.contiene(l) {
                        // Pulsar el nombre abre la informacion del proyecto,
                        // como en Telegram.
                        if let Some(a) = abierto.as_mut() {
                            a.info = Some(SECCIONES[0]);
                            a.scroll_info = 0;
                        }
                        hay_que_pintar = true;
                    } else if let Some((a, indice)) = abierto.as_ref().and_then(|a| {
                        let area =
                            disposicion.historial(a.alto_caja.get(), a.fijado.is_some(), escala);
                        let c = a.colocado.borrow();
                        let scroll = a.scroll.unwrap_or_else(|| {
                            pixpin_ui::historial::scroll_maximo(area, a.alto.get())
                        });
                        pixpin_ui::historial::mensaje_en(area, &c.puestos, scroll, l)
                            .map(|i| (a, i))
                    }) {
                        abrir_mensaje(ubicacion, a, indice);
                        buscando = false;
                    } else if disposicion.buscador(escala).contiene(l) {
                        buscando = true;
                        hay_que_pintar = true;
                    } else if abierto.as_ref().is_some_and(|a| {
                        disposicion
                            .boton_adjuntar(a.alto_caja.get(), escala)
                            .contiene(l)
                    }) {
                        // El clip despliega su menu; lo que se adjunta se
                        // decide ahi, no aqui.
                        menu_adjuntar = !menu_adjuntar;
                        buscando = false;
                        hay_que_pintar = true;
                    } else if abierto.as_ref().is_some_and(|a| {
                        disposicion
                            .boton_enviar(a.alto_caja.get(), escala)
                            .contiene(l)
                    }) {
                        if let Some(a) = abierto.as_mut() {
                            if !a.borrador.trim().is_empty() {
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
                        if elegida != Some(i) {
                            // Lo escrito y sin enviar se guarda antes de
                            // cambiar; volver a este proyecto lo devuelve.
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
                EventoOverlay::BotonSoltado(_) => {
                    if arrastre.is_some() {
                        ventana.soltar_raton();
                    }
                    arrastre = None;
                }
                EventoOverlay::Rueda(delta) => {
                    // La rueda va a la columna donde esta el raton, no a la
                    // que se pincho la ultima vez: es lo que espera la mano.
                    let aqui = local(pixpin_shell::entorno::posicion_del_cursor());
                    if let Some(a) = abierto.as_mut().filter(|_| disposicion.chat.contiene(aqui)) {
                        let area =
                            disposicion.historial(a.alto_caja.get(), a.fijado.is_some(), escala);
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
                // Escribir en la caja de abajo. Solo llega si hay un
                // proyecto abierto: sin conversacion no hay donde guardarlo.
                // Con el buscador enfocado, lo que se teclea va ahi.
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
                // Escapar cierra primero lo que este desplegado encima: el
                // menu antes que el buscador, y los dos antes que la ventana.
                EventoOverlay::Tecla { vk, .. } if menu_adjuntar && vk == VK_ESCAPE => {
                    menu_adjuntar = false;
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
                        menu_adjuntar = false;
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
                EventoOverlay::Tecla { vk, .. } if vk == VK_ESCAPE => cerrar = true,
                _ => {}
            }
        }
        if cerrar {
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

        if hay_que_pintar {
            hay_que_pintar = false;
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
                };
                let abierto_ref = abierto.as_ref();
                let pendientes_ref = pendientes.as_ref();
                let resultado = motor.dibujar(&destino, |p: &Pintor| {
                    pintar(p, &disposicion, tema, escala, textos, sobre, &lista);
                    if let Some(a) = abierto_ref {
                        // Medir lo escrito decide lo alta que es la caja y,
                        // con ello, donde acaba el historial. Se hace una
                        // vez y lo usan los dos.
                        let ef = escala as f32 / 100.0;
                        let tam = chat::REDACCION_TAM * ef;
                        let ancho = (disposicion.chat.ancho as f32
                            - 2.0 * chat::REDACCION_RELLENO_X as f32 * ef)
                            .max(0.0);
                        let alto_texto = if a.borrador.is_empty() {
                            tam.ceil() as u32
                        } else {
                            p.medir_texto_ajustado(&a.borrador, tam, ancho).1.ceil() as u32
                        };
                        a.alto_caja.set(alto_texto);
                        let c = Pinta {
                            tema,
                            escala,
                            textos,
                            ahora,
                            miniaturas: &miniaturas,
                        };
                        match a.info {
                            // Con la informacion abierta, la columna de la
                            // derecha es suya entera.
                            Some(seccion) => pintar_info(p, &disposicion, &c, a, seccion),
                            None => {
                                pintar_historial(p, &disposicion, &c, a, alto_texto);
                                pintar_redaccion(p, &disposicion, &c, a, alto_texto);
                                // El ultimo, porque se despliega encima de
                                // todo. Mide sus rotulos aunque este
                                // cerrado: el raton los necesita para saber
                                // donde cae el menu en cuanto se abra.
                                pintar_menu(p, &disposicion, &c, a, marco, menu_adjuntar);
                            }
                        }
                        // Encima de todo, panel incluido: esta esperando una
                        // respuesta y nada debe distraer de ella.
                        if let Some(pend) = pendientes_ref {
                            pintar_confirmar(p, &c, pend, marco);
                        }
                    }
                });
                if resultado.is_err() {
                    // Dispositivo perdido: los bitmaps de las miniaturas eran
                    // del viejo y D2D los rechazaria. Los pixeles se quedan,
                    // asi que volver a subirlos no toca el disco.
                    miniaturas.soltar();
                }
                let _ = superficie.presentar();
            }
        }
        // Sin nada que hacer, el hilo duerme: la ventana abierta en reposo no
        // cuesta CPU.
        pixpin_shell::overlay::esperar_eventos(None);
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
    /// La lupa esta encendida. Va aparte de `busqueda_info` porque al
    /// pulsarla la caja aparece vacia, y sin esto no habria donde escribir.
    buscando_info: bool,
    /// Lo que ocupa la seccion abierta, que solo se sabe al colocarla.
    alto_info: std::cell::Cell<u32>,
    /// Lo ancho que mide el rotulo de cada pestana. Lo apunta el pintado,
    /// que es quien tiene la fuente; el raton lo necesita para saber en cual
    /// se pulso.
    anchos_pestanas: std::cell::RefCell<Vec<f32>>,
    /// Y lo mismo para los rotulos del menu del clip: el ancho del menu lo
    /// manda el mas largo, y solo se sabe con la fuente delante.
    anchos_menu: std::cell::RefCell<Vec<f32>>,
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
}

#[derive(Default)]
struct Colocado {
    ancho: u32,
    puestos: Vec<pixpin_ui::historial::Puesto>,
    /// El texto ya compuesto de cada mensaje, en el mismo orden.
    lineas: Vec<String>,
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

fn color_avatar(codigo: &str) -> Color {
    let suma: u32 = codigo.bytes().map(u32::from).sum();
    hex(COLORES_AVATAR[suma as usize % COLORES_AVATAR.len()])
}

fn pintar(
    p: &Pintor,
    d: &Disposicion,
    tema: &Tema,
    escala: u32,
    textos: &Catalogo,
    sobre: Option<BotonBarra>,
    lista: &Lista,
) {
    let e = escala as f32 / 100.0;
    p.limpiar(tema.chat);
    p.rellenar(rf(d.barra), tema.barra);
    p.rellenar(rf(d.lista), tema.lista);
    p.rellenar(rf(d.cabecera_lista), tema.cabecera);
    p.rellenar(rf(d.cabecera_chat), tema.cabecera);

    // Las lineas que separan: 1 px logico, como Telegram.
    let linea = (1.0 * e).max(1.0);
    for (r, vertical) in [
        (d.barra, false),
        (d.cabecera_lista, false),
        (d.cabecera_chat, false),
        (d.lista, true),
    ] {
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
        if elegida {
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
        let hora = pixpin_ui::chat::etiqueta_hora(ficha.tocado, lista.ahora);
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
}

fn pintar_historial(p: &Pintor, d: &Disposicion, c: &Pinta, a: &Abierto, alto_caja: u32) {
    let (tema, escala, textos, ahora) = (c.tema, c.escala, c.textos, c.ahora);
    use pixpin_ui::historial as h;
    let e = escala as f32 / 100.0;
    if d.chat.ancho == 0 {
        return;
    }

    // La cabecera: el avatar del proyecto, su nombre y lo que tiene dentro.
    let cab = d.cabecera_chat;
    // Las medidas de la cabecera de Telegram: avatar de 42 en (19, 6).
    let avatar = chat::CABECERA_AVATAR as f32 * e;
    let ax = cab.x as f32 + chat::CABECERA_AVATAR_X as f32 * e;
    let ay = cab.y as f32 + chat::CABECERA_AVATAR_Y as f32 * e;
    let caja = RectF {
        x: ax,
        y: ay,
        ancho: avatar,
        alto: avatar,
    };
    p.rellenar_redondeado(caja, avatar / 2.0, color_avatar(&a.ficha.codigo_unico()));
    let letras = iniciales(&a.ficha.nombre);
    let (w, alto_letras) = p.medir_texto(&letras, avatar * 0.4);
    p.texto(
        &letras,
        ax + (avatar - w) / 2.0,
        ay + (avatar - alto_letras) / 2.0,
        avatar * 0.4,
        Color::BLANCO,
    );
    let texto_x = cab.x as f32 + chat::CABECERA_TEXTO_X as f32 * e;
    let ancho_nombre =
        (cab.derecha() as f32 - chat::CABECERA_MARGEN_DERECHO as f32 * e - texto_x).max(0.0);
    p.texto_linea(
        &a.ficha.nombre,
        texto_x,
        cab.y as f32 + chat::CABECERA_NOMBRE_Y as f32 * e,
        chat::CABECERA_TAM * e,
        ancho_nombre,
        tema.texto,
    );
    let mut abajo = {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("cuantas", a.ficha.hojas);
        textos.t_args("chat-hojas", &args)
    };
    if a.rotas > 0 {
        // Lo que no se pudo leer se dice, no se calla.
        abajo.push_str(&format!("  ·  {} ?", a.rotas));
    }
    // La linea de estado se apoya abajo, a 8 del borde, como en Telegram.
    let (_, alto_estado) = p.medir_texto(&abajo, chat::CABECERA_TAM * e);
    p.texto_linea(
        &abajo,
        texto_x,
        cab.abajo() as f32 - chat::CABECERA_NOMBRE_Y as f32 * e - alto_estado,
        chat::CABECERA_TAM * e,
        ancho_nombre,
        tema.apagado,
    );

    // La barra del mensaje fijado, si lo hay: una rayita de color, el
    // rotulo y la linea del mensaje, como en Telegram y en Android.
    if let Some(fijado) = a.fijado.and_then(|i| a.mensajes.get(i)) {
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
        // La linea que la separa del historial.
        p.rellenar(
            RectF {
                x: barra.x as f32,
                y: barra.abajo() as f32 - (1.0 * e).max(1.0),
                ancho: barra.ancho as f32,
                alto: (1.0 * e).max(1.0),
            },
            tema.separador,
        );
    }

    // El area de los mensajes: entre la cabecera (o el fijado) y la caja.
    let area = d.historial(alto_caja, a.fijado.is_some(), escala);
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
    {
        let mut c = a.colocado.borrow_mut();
        if c.ancho != ancho_contenido || c.puestos.len() != a.mensajes.len() {
            let mut entradas = Vec::with_capacity(a.mensajes.len());
            let mut lineas = Vec::with_capacity(a.mensajes.len());
            for (indice, m) in a.mensajes.iter().enumerate() {
                let mut texto = m.resumen();
                if let Some(etiqueta) = clase_de(m, textos) {
                    texto = if texto.is_empty() {
                        etiqueta
                    } else {
                        format!("{etiqueta}\n{texto}")
                    };
                }
                // La hora va al final de la ultima linea, con su hueco. Si
                // ahi no cabe, baja a una linea propia y la burbuja crece;
                // es lo que hace Telegram, y evita que se monte encima.
                let tam = h::TEXTO_TAM * e;
                let (ancho, alto) = p.medir_texto_ajustado(&texto, tam, ancho_contenido as f32);
                let hora = pixpin_ui::chat::etiqueta_hora(m.cuando, ahora);
                let (ancho_hora, alto_hora) = p.medir_texto(&hora, h::HORA_TAM * e);
                let reserva = ancho_hora + h::HORA_HUECO as f32 * e;
                // Si el texto cabe igual en una caja mas estrecha, ninguna
                // de sus lineas llega al borde y la hora tiene sitio.
                let estrecho = (ancho_contenido as f32 - reserva).max(1.0);
                let cabe_al_lado = p.medir_texto_ajustado(&texto, tam, estrecho).1 <= alto + 0.5;
                let (mut ancho, mut alto) = if cabe_al_lado {
                    ((ancho + reserva).min(ancho_contenido as f32), alto)
                } else {
                    (ancho, alto + alto_hora)
                };
                // Un dibujo ensena su lienzo: la vista previa manda sobre
                // el texto, que queda como pie.
                if a.vistas.get(indice).is_some_and(|v| v.is_some()) {
                    let vista_ancho = (h::VISTA_ANCHO as f32 * e).min(ancho_contenido as f32);
                    ancho = ancho.max(vista_ancho);
                    alto += h::VISTA_ALTO as f32 * e + h::RELLENO_Y as f32 * e;
                }
                entradas.push(h::Entrada {
                    alto: alto.ceil() as u32,
                    ancho: ancho.ceil() as u32,
                    // Lo que nacio en otro aparato se ensena a la izquierda.
                    mio: m.origen.is_none(),
                    cuando: m.cuando,
                    dia: m.cuando.div_euclid(86_400_000),
                });
                lineas.push(texto);
            }
            let (puestos, alto) = h::colocar(area, &entradas, escala);
            a.alto.set(alto);
            *c = Colocado {
                ancho: ancho_contenido,
                puestos,
                lineas,
            };
        }
    }

    let c = a.colocado.borrow();
    // Sin desplazamiento propio, pegado al final: lo ultimo es lo que importa.
    let scroll = a
        .scroll
        .unwrap_or_else(|| h::scroll_maximo(area, a.alto.get()));
    p.empujar_recorte(rf(area));
    let (primero, cuantos) = h::visibles(area, &c.puestos, scroll);
    for i in primero..primero + cuantos {
        let puesto = c.puestos[i];
        let m = &a.mensajes[i];
        let mover = |r: Rect| Rect {
            y: r.y + area.y - scroll,
            ..r
        };

        if let Some(sep) = puesto.separador {
            let sep = mover(sep);
            let fecha = fecha_larga(m.cuando, ahora, textos);
            if !fecha.is_empty() {
                let tam = h::SEPARADOR_TAM * e;
                let (w, alto_texto) = p.medir_texto(&fecha, tam);
                let alto = h::SEPARADOR_PILDORA as f32 * e;
                let ancho = w + 2.0 * h::SEPARADOR_RELLENO_X as f32 * e;
                let caja = RectF {
                    x: sep.x as f32 + (sep.ancho as f32 - ancho) / 2.0,
                    // La pildora va pegada abajo de su hueco: encima lleva
                    // 10 de aire y debajo 2.
                    y: sep.abajo() as f32 - 2.0 * e - alto,
                    ancho,
                    alto,
                };
                p.rellenar_redondeado(caja, alto / 2.0, tema.separador_dia);
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
        let mio = m.origen.is_none();
        let (color, color_texto, color_hora) = if mio {
            (tema.burbuja_mia, tema.texto_mio, tema.hora_mia)
        } else {
            (tema.burbuja_otra, tema.texto_otro, tema.hora_otra)
        };
        p.rellenar_redondeado(rf(burbuja), h::RADIO as f32 * e, color);
        let dentro = Rect {
            y: burbuja.y + (h::RELLENO_Y as f32 * e) as i32,
            ..mover(puesto.dentro(escala))
        };
        // Si es un dibujo, primero el lienzo y el texto debajo, como pie.
        let mut texto_y = dentro.y as f32;
        if let Some(Some(vista)) = a.vistas.get(i) {
            let alto_vista = h::VISTA_ALTO as f32 * e;
            let hoja = RectF {
                x: dentro.x as f32,
                y: texto_y,
                ancho: dentro.ancho as f32,
                alto: alto_vista,
            };
            // Fondo de papel: tanto un dibujo como una tabla se hacen sobre
            // blanco, y su trazo oscuro sobre la burbuja azul no se leeria.
            p.rellenar_redondeado(hoja, 6.0 * e, tema.papel);
            match vista {
                Ojeada::Lienzo(l) => pintar_lienzo(p, l, hoja),
                Ojeada::Tabla(t) => pintar_ojeada_tabla(p, tema, escala, t, hoja),
            }
            texto_y += alto_vista + h::RELLENO_Y as f32 * e;
        }
        p.parrafo(
            &c.lineas[i],
            dentro.x as f32,
            texto_y,
            h::TEXTO_TAM * e,
            dentro.ancho as f32,
            &[],
            color_texto,
        );
        // La hora, abajo a la derecha, invadiendo un poco el relleno de la
        // burbuja como hace Telegram: 2 por la derecha y 5 por abajo.
        let hora = pixpin_ui::chat::etiqueta_hora(m.cuando, ahora);
        if !hora.is_empty() {
            let (w, alto) = p.medir_texto(&hora, h::HORA_TAM * e);
            p.texto(
                &hora,
                burbuja.derecha() as f32 - (h::RELLENO_X - h::HORA_INVADE_X) as f32 * e - w,
                burbuja.abajo() as f32 - (h::RELLENO_Y - h::HORA_INVADE_Y) as f32 * e - alto,
                h::HORA_TAM * e,
                color_hora,
            );
        }
    }
    p.soltar_recorte();
}

/// Abre un proyecto: lee su cuaderno de disco.
///
/// Que no haya cuaderno no es un fallo: un proyecto recien llegado del movil
/// todavia no tiene ninguno. Se ensena vacio y ya esta.
fn abrir_proyecto(ubicacion: &Ubicacion, ficha: &pixpin_proyecto::almacen::Ficha) -> Abierto {
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &ficha.id);
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
        buscando_info: false,
        alto_info: std::cell::Cell::new(0),
        anchos_pestanas: std::cell::RefCell::new(Vec::new()),
        anchos_menu: std::cell::RefCell::new(Vec::new()),
        alto_caja: std::cell::Cell::new(0),
        scroll: None,
        alto: std::cell::Cell::new(0),
        colocado: std::cell::RefCell::new(Colocado::default()),
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
    let cuando = pixpin_shell::entorno::ahora_local_ms();
    // El numero sigue al mayor que ya hay, que es lo que hace el codigo de
    // chat (`47·K7Q2`) unico dentro de la conversacion.
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let mensaje = cuaderno::Mensaje::nota(
        &texto,
        &cuaderno::Sello {
            cuando,
            numero,
            aparato: aparato.to_string(),
            proyecto: a.ficha.id.clone(),
        },
    );
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    cuaderno::anadir(&carpeta, &mensaje)?;

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

/// La caja de escribir, abajo de la columna del proyecto.
fn pintar_redaccion(p: &Pintor, d: &Disposicion, c: &Pinta, a: &Abierto, alto_texto: u32) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let caja = d.redaccion(alto_texto, escala);
    if caja.ancho == 0 || caja.alto == 0 {
        return;
    }
    p.rellenar(rf(caja), tema.cabecera);
    // Una linea arriba la separa del historial, como la cabecera.
    p.rellenar(
        RectF {
            x: caja.x as f32,
            y: caja.y as f32,
            ancho: caja.ancho as f32,
            alto: (1.0 * e).max(1.0),
        },
        tema.separador,
    );

    // Los dos botones, como en Telegram: adjuntar a la izquierda y enviar a
    // la derecha, apoyados abajo.
    let icono_lado = 20.0 * e;
    let centrado = |r: Rect| RectF {
        x: r.x as f32 + (r.ancho as f32 - icono_lado) / 2.0,
        y: r.y as f32 + (r.alto as f32 - icono_lado) / 2.0,
        ancho: icono_lado,
        alto: icono_lado,
    };
    let adjuntar = d.boton_adjuntar(alto_texto, escala);
    p.icono(
        &pixpin_render::iconos_excalidraw::FILE,
        centrado(adjuntar),
        tema.apagado,
    );
    // El de enviar solo se ensena si hay algo que enviar.
    if !a.borrador.trim().is_empty() {
        let enviar = d.boton_enviar(alto_texto, escala);
        let c = centrado(enviar);
        // Un triangulo a mano: son tres lineas y un icono aqui seria un
        // recurso de mas.
        let grosor = (2.0 * e).max(1.0);
        let (x0, y0) = (c.x, c.y);
        let m = icono_lado / 2.0;
        p.linea((x0, y0), (x0 + icono_lado, y0 + m), grosor, tema.enviar);
        p.linea(
            (x0 + icono_lado, y0 + m),
            (x0, y0 + icono_lado),
            grosor,
            tema.enviar,
        );
        p.linea((x0, y0), (x0, y0 + icono_lado), grosor, tema.enviar);
    }

    let zona = d.texto_redaccion(alto_texto, escala);
    let x = zona.x as f32 + chat::REDACCION_RELLENO_X as f32 * e;
    let y = caja.y as f32 + chat::REDACCION_RELLENO_Y as f32 * e;
    let ancho = (zona.ancho as f32 - 2.0 * chat::REDACCION_RELLENO_X as f32 * e).max(0.0);
    let tam = chat::REDACCION_TAM * e;
    if a.borrador.is_empty() {
        p.texto(&textos.t("chat-escribe"), x, y, tam, tema.apagado);
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
    p.empujar_recorte(rf(caja));
    p.parrafo(&a.borrador, x, y, tam, ancho, &[], tema.texto);
    // El cursor va al final de lo escrito.
    let (ancho_ultima, alto_todo) = p.medir_texto_ajustado(&a.borrador, tam, ancho);
    let _ = ancho_ultima;
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
            let cuando = pixpin_shell::entorno::ahora_local_ms();
            vec![(format!("pegado-{cuando}.png"), bytes)]
        }
        Que::Rutas(rutas) => rutas
            .iter()
            .filter_map(|r| {
                let nombre = r.file_name()?.to_string_lossy().to_string();
                // Uno que no se pueda leer no puede llevarse los demas.
                match std::fs::read(r) {
                    Ok(bytes) => Some((nombre, bytes)),
                    Err(e) => {
                        tracing::warn!(?e, ruta = %r.display(), "fichero que no se pudo leer");
                        None
                    }
                }
            })
            .collect(),
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
    use pixpin_proyecto::{almacen, cuaderno};
    let raiz = ubicacion.raiz();
    let ruta = almacen::guardar_adjunto(raiz, &a.ficha.id, nombre, bytes)?;
    let cuando = pixpin_shell::entorno::ahora_local_ms();
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let mensaje = cuaderno::Mensaje::adjunto(
        cuaderno::clase_de_nombre(nombre),
        nombre,
        &ruta,
        bytes.len() as i64,
        &cuaderno::Sello {
            cuando,
            numero,
            aparato: aparato.to_string(),
            proyecto: a.ficha.id.clone(),
        },
    );
    // Primero al cuaderno y solo despues a la pantalla, como al escribir.
    cuaderno::anadir(&almacen::carpeta(raiz, &a.ficha.id), &mensaje)?;
    a.mensajes.push(mensaje);
    a.vistas.push(None);
    a.ficha.tocado = cuando;
    a.ficha.resumen = nombre.to_string();
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == a.ficha.id) {
        f.tocado = a.ficha.tocado;
        f.resumen = a.ficha.resumen.clone();
        indice.guardar(raiz)?;
    }
    Ok(())
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
    if std::path::Path::new(relativa).is_absolute() {
        return None;
    }
    Some(pixpin_proyecto::almacen::carpeta(raiz, proyecto).join(relativa))
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

/// El menu del clip, encima de todo lo demas.
///
/// Mide sus rotulos SIEMPRE, este abierto o cerrado, y los apunta: el ancho
/// del menu depende del mas largo, y el raton tiene que poder colocarlo
/// igual que el pintado desde el primer clic, sin esperar a un fotograma.
fn pintar_menu(p: &Pintor, d: &Disposicion, c: &Pinta, a: &Abierto, marco: Rect, abierto: bool) {
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let rotulos: Vec<String> = menu::Entrada::TODAS
        .iter()
        .map(|n| textos.t(n.clave()))
        .collect();
    let anchos: Vec<f32> = rotulos
        .iter()
        .map(|r| p.medir_texto(r, menu::TEXTO_TAM * e).0)
        .collect();
    *a.anchos_menu.borrow_mut() = anchos;
    if !abierto {
        return;
    }
    let m = menu_del_clip(d, a, marco, escala);
    p.rellenar_redondeado(rf(m.caja), menu::RADIO as f32 * e, tema.cabecera);
    for (n, rotulo) in rotulos.iter().enumerate() {
        let fila = m.fila(n, escala);
        let icono = match menu::Entrada::TODAS[n] {
            menu::Entrada::Imagen => &pixpin_render::iconos_excalidraw::IMAGE_ICON,
            menu::Entrada::Archivo => &pixpin_render::iconos_excalidraw::FILE,
            menu::Entrada::Lienzo => &pixpin_render::iconos_excalidraw::FREEDRAW_ICON,
            menu::Entrada::Tabla => &pixpin_render::iconos_excalidraw::GRID_ICON,
        };
        p.icono(icono, rf(m.icono(fila, escala)), tema.apagado);
        let (_, alto_texto) = p.medir_texto(rotulo, menu::TEXTO_TAM * e);
        let x = m.texto(fila, escala) as f32;
        p.texto_linea(
            rotulo,
            x,
            fila.y as f32 + (fila.alto as f32 - alto_texto) / 2.0,
            menu::TEXTO_TAM * e,
            (m.caja.derecha() as f32 - x - menu::RELLENO_DERECHA as f32 * e).max(0.0),
            tema.texto,
        );
    }
}

/// Donde cae el menu del clip, con los rotulos que midio el pintado.
///
/// Se calcula igual al pintar y al pulsar, en vez de guardarse: un menu
/// guardado y una ventana que cambia de tamano dejarian de coincidir, y se
/// pulsaria una entrada distinta de la que se ve.
fn menu_del_clip(d: &Disposicion, a: &Abierto, marco: Rect, escala: u32) -> menu::Menu {
    let anchos = a.anchos_menu.borrow();
    menu::desplegar(
        d.boton_adjuntar(a.alto_caja.get(), escala),
        Rect {
            x: 0,
            y: 0,
            ancho: marco.ancho,
            alto: marco.alto,
        },
        &anchos,
        // La letra del menu, no la de la caja de escribir: si fuera esa, las
        // filas crecerian al escribir un mensaje largo.
        (menu::TEXTO_TAM * escala as f32 / 100.0).ceil() as u32,
        escala,
    )
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

    let cuando = pixpin_shell::entorno::ahora_local_ms();
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

    let cuando = pixpin_shell::entorno::ahora_local_ms();
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
) -> usize {
    let mut hechos = 0;
    for ruta in rutas {
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
    hechos
}

/// Un lienzo ya leido y listo para pintar en su burbuja.
struct LienzoVisto {
    ordenes: Vec<pixpin_motor2d::Orden>,
    /// La caja que ocupa el dibujo, en sus propias coordenadas.
    caja: (f32, f32, f32, f32),
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
    if m.clase != Some(Clase::Dibujo) {
        return None;
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
    let caja = escena.caja()?;
    let ordenes = pixpin_motor2d::ordenes_de_escena(&escena);
    if ordenes.is_empty() {
        return None;
    }
    Some(Ojeada::Lienzo(LienzoVisto { ordenes, caja }))
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
fn pintar_lienzo(p: &Pintor, vista: &LienzoVisto, destino: RectF) {
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
    let ruta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id).join(relativa);
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
        p.texto_linea(
            &textos.t_args("chat-hojas", &args),
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
            match a.vistas.get(indice).and_then(|v| v.as_ref()) {
                // Un dibujo se ensena dibujado. Sobre papel blanco y no
                // sobre el gris de la celda: los trazos vienen de un lienzo
                // claro y sobre gris se pierden, igual que en las burbujas.
                Some(vista) => {
                    p.rellenar_redondeado(rf(celda), 4.0 * e, tema.papel);
                    p.empujar_recorte(rf(celda));
                    let dentro = encoger(rf(celda), 4.0 * e);
                    match vista {
                        Ojeada::Lienzo(l) => pintar_lienzo(p, l, dentro),
                        Ojeada::Tabla(t) => pintar_ojeada_tabla(p, tema, escala, t, dentro),
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
                &pixpin_ui::chat::etiqueta_hora(m.cuando, c.ahora),
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
