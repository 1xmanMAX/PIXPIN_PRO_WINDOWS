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
//! minimizar, maximizar y cerrar), redimensionar por los bordes y recordar
//! donde quedo. La lista de proyectos y el historial llegan despues.

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
}

const CLARO: Tema = Tema {
    barra: hex(0xffffff),
    boton_sobre: hex(0xe6e6e6),
    cerrar_sobre: hex(0xe81123),
    lista: hex(0xffffff),
    chat: hex(0xf1f1f1),
    cabecera: hex(0xffffff),
    separador: hex(0xe0e0e0),
    texto: hex(0x000000),
    apagado: hex(0x999999),
};

const OSCURO: Tema = Tema {
    barra: hex(0x17212b),
    boton_sobre: hex(0x232e3a),
    cerrar_sobre: hex(0xe81123),
    lista: hex(0x17212b),
    chat: hex(0x0e1621),
    cabecera: hex(0x17212b),
    separador: hex(0x101921),
    texto: hex(0xffffff),
    apagado: hex(0x7d8b99),
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

    let tema = if pixpin_shell::entorno::tema_claro() {
        &CLARO
    } else {
        &OSCURO
    };
    let mut ancho_lista = chat::ancho_inicial(marco.ancho, escala);
    let mut arrastre: Option<Arrastre> = None;
    let mut sobre: Option<BotonBarra> = None;
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
                EventoOverlay::Pintar => hay_que_pintar = true,
                EventoOverlay::Cerrar => cerrar = true,
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
                hay_que_pintar = true;
            }
        }

        if hay_que_pintar {
            hay_que_pintar = false;
            if let Ok(destino) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&destino, |p: &Pintor| {
                    pintar(p, &disposicion, tema, escala, textos, sobre);
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

fn pintar(
    p: &Pintor,
    d: &Disposicion,
    tema: &Tema,
    escala: u32,
    textos: &Catalogo,
    sobre: Option<BotonBarra>,
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
        13.0 * e,
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
        let lado = 10.0 * e;
        let grosor = (1.0 * e).max(1.0);
        match boton {
            BotonBarra::Minimizar => p.rellenar(
                RectF {
                    x: cx - lado / 2.0,
                    y: cy,
                    ancho: lado,
                    alto: grosor,
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

    // Sin proyectos todavia: se dice, en vez de dejar dos columnas en blanco
    // que parecen un fallo.
    centrar_texto(
        &textos.t("chat-sin-proyectos"),
        d.filas,
        None,
        13.0 * e,
        tema.apagado,
    );
    centrar_texto(
        &textos.t("chat-elige-proyecto"),
        d.chat,
        None,
        13.0 * e,
        tema.apagado,
    );
}
