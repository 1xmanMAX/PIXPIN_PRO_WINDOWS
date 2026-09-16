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

    let mut ancho_lista = chat::ancho_inicial(marco.ancho, escala);
    let mut arrastre: Option<Arrastre> = None;
    let mut sobre: Option<BotonBarra> = None;
    let mut scroll: i32 = 0;
    let mut fila_sobre: Option<usize> = None;
    let mut elegida: Option<usize> = None;
    let mut abierto: Option<Abierto> = None;
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
                    if let Some(b) = chat::borde_en(l, marco.ancho, marco.alto, escala) {
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
                    } else if let Some(i) = disposicion.fila_en(l, scroll, fichas.len(), escala) {
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
                            fila_sobre = disposicion.fila_en(l, scroll, fichas.len(), escala);
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
                        let area = disposicion.historial(a.alto_caja.get(), escala);
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
                        fichas.len(),
                        escala,
                    );
                    if nuevo != scroll {
                        scroll = nuevo;
                        // Lo que hay bajo el raton cambia aunque el raton no
                        // se mueva.
                        let p = local(pixpin_shell::entorno::posicion_del_cursor());
                        fila_sobre = disposicion.fila_en(p, scroll, fichas.len(), escala);
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::Pintar => hay_que_pintar = true,
                EventoOverlay::Cerrar => cerrar = true,
                // Escribir en la caja de abajo. Solo llega si hay un
                // proyecto abierto: sin conversacion no hay donde guardarlo.
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
                    match abierto.as_mut() {
                        Some(a) => {
                            let mut hechos = 0;
                            for ruta in &rutas {
                                match std::fs::read(ruta) {
                                    Ok(bytes) => {
                                        let nombre = ruta
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_else(|| "archivo".into());
                                        // Uno que falle no puede llevarse los
                                        // demas que venian con el.
                                        match adjuntar(ubicacion, a, &identidad, &nombre, &bytes) {
                                            Ok(()) => hechos += 1,
                                            Err(e) => {
                                                tracing::warn!(?e, nombre, "no se pudo guardar")
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(?e, ruta = %ruta.display(), "no se pudo leer")
                                    }
                                }
                            }
                            if hechos > 0 {
                                a.scroll = None;
                                if let Some(i) = elegida {
                                    fichas[i].tocado = a.ficha.tocado;
                                    fichas[i].resumen = a.ficha.resumen.clone();
                                }
                            }
                            a.colocado.borrow_mut().ancho = 0;
                            hay_que_pintar = true;
                        }
                        None => tracing::info!(
                            cuantos = rutas.len(),
                            "ficheros soltados sin proyecto abierto"
                        ),
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
                scroll = d.scroll_ajustado(scroll, fichas.len(), escala);
                hay_que_pintar = true;
            }
        }

        if hay_que_pintar {
            hay_que_pintar = false;
            if let Ok(destino) = superficie.empezar(&motor) {
                let lista = Lista {
                    fichas: &fichas,
                    scroll,
                    sobre: fila_sobre,
                    elegida,
                    ahora,
                    textos,
                };
                let abierto_ref = abierto.as_ref();
                let _ = motor.dibujar(&destino, |p: &Pintor| {
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
                        };
                        pintar_historial(p, &disposicion, &c, a, alto_texto);
                        pintar_redaccion(p, &disposicion, &c, a, alto_texto);
                    }
                });
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

/// El proyecto abierto en la columna de la derecha.
struct Abierto {
    ficha: pixpin_proyecto::almacen::Ficha,
    mensajes: Vec<pixpin_proyecto::cuaderno::Mensaje>,
    /// Lineas del cuaderno que no se entendieron. Se ensenan: si faltan
    /// mensajes, el usuario tiene que enterarse.
    rotas: usize,
    /// Lo escrito y todavia sin enviar.
    borrador: String,
    /// Lo alto que mide ese texto ya medido con la fuente. Lo apunta el
    /// pintado; la rueda lo necesita para saber donde acaba el historial.
    alto_caja: std::cell::Cell<u32>,
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
    scroll: i32,
    sobre: Option<usize>,
    elegida: Option<usize>,
    /// La hora local de ahora, para decidir si una fecha es de hoy.
    ahora: i64,
    textos: &'a Catalogo,
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

    // El titulo de la lista.
    centrar_texto(
        &textos.t("chat-titulo"),
        d.cabecera_lista,
        Some(16.0 * e),
        15.0 * e,
        tema.texto,
    );

    // Sin proyectos todavia: se dice, en vez de dejar la columna en blanco
    // que parece un fallo.
    if lista.fichas.is_empty() {
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
    let (primera, cuantas) = d.visibles(lista.scroll, lista.fichas.len(), escala);
    for i in primera..primera + cuantas {
        let ficha = &lista.fichas[i];
        let r = d.fila(i, lista.scroll, escala);
        let elegida = lista.elegida == Some(i);
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

    // El area de los mensajes: entre la cabecera y la caja de escribir.
    let area = d.historial(alto_caja, escala);
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
            for m in &a.mensajes {
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
                let (ancho, alto) = if cabe_al_lado {
                    ((ancho + reserva).min(ancho_contenido as f32), alto)
                } else {
                    (ancho, alto + alto_hora)
                };
                entradas.push(h::Entrada {
                    alto: alto.ceil() as u32,
                    ancho: ancho.ceil() as u32,
                    // Lo que nacio en otro aparato se ensena a la izquierda.
                    mio: m.origen.is_none(),
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
        p.parrafo(
            &c.lineas[i],
            dentro.x as f32,
            dentro.y as f32,
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
    Abierto {
        ficha: ficha.clone(),
        mensajes,
        rotas: cuaderno.lineas_rotas,
        borrador: String::new(),
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

    let x = caja.x as f32 + chat::REDACCION_RELLENO_X as f32 * e;
    let y = caja.y as f32 + chat::REDACCION_RELLENO_Y as f32 * e;
    let ancho = (caja.ancho as f32 - 2.0 * chat::REDACCION_RELLENO_X as f32 * e).max(0.0);
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

/// La fecha del separador de dias, como la escribe Telegram: «15 de
/// septiembre», con el ano solo si no es este.
///
/// A proposito NO dice «Hoy» ni «Ayer»: Telegram Desktop tampoco, y una
/// pildora que pone «Hoy» entre dos mensajes de hace un rato no separa nada.
fn fecha_larga(cuando_ms: i64, ahora_ms: i64, textos: &Catalogo) -> String {
    if cuando_ms <= 0 {
        return String::new();
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
