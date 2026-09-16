//! La ventana de chat: proyectos a la izquierda, su contenido a la derecha.
//!
//! La disposicion la calcula `pixpin_ui::chat`, que es pura y esta probada;
//! aqui van la ventana, el raton y los colores. Las medidas y los colores
//! salen de `docs/investigacion/2026-09-15-telegram-desktop-estructura.md`
//! (analisis de Telegram Desktop). **Telegram Desktop es GPL-3.0: de ahi
//! solo se toman medidas, colores y tecnicas; ni una linea de su codigo ni
//! sus recursos.**
//!
//! Paso 1 de la ventana: las dos columnas, el asa que las reparte y los
//! estados vacios. La lista de proyectos, el historial y el campo de
//! escribir llegan en los pasos siguientes.

use anyhow::{Context, Result};
use pixpin_geom::{Punto, Rect};
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin, VentanaOverlay};
use pixpin_store::Catalogo;
use pixpin_ui::chat::{self, Disposicion, Vista};

use crate::caja_dibujo::hex;
use crate::overlay::Recursos;

/// Tamano con el que nace, en pixeles logicos (el de Telegram en escritorio).
const ANCHO_LOGICO: u32 = 1024;
const ALTO_LOGICO: u32 = 768;
/// Escape cierra la ventana.
const VK_ESCAPE: u32 = 0x1B;

/// Los colores de un tema.
struct Tema {
    lista: Color,
    chat: Color,
    cabecera: Color,
    separador: Color,
    texto: Color,
    apagado: Color,
}

const CLARO: Tema = Tema {
    lista: hex(0xffffff),
    chat: hex(0xf1f1f1),
    cabecera: hex(0xffffff),
    separador: hex(0xe0e0e0),
    texto: hex(0x000000),
    apagado: hex(0x999999),
};

const OSCURO: Tema = Tema {
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

/// Abre la ventana y no vuelve hasta que se cierra.
pub fn abrir(recursos: &Recursos, textos: &Catalogo) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .principal()
        .context("sin monitor principal")?
        .to_owned();
    let escala = monitor.escala_por_cien;
    let e = |v: u32| v * escala / 100;
    let (ancho, alto) = (e(ANCHO_LOGICO), e(ALTO_LOGICO));
    let marco = Rect {
        x: monitor.area.x + (monitor.area.ancho as i32 - ancho as i32) / 2,
        y: monitor.area.y + (monitor.area.alto as i32 - alto as i32) / 2,
        ancho,
        alto,
    };

    let ventana = VentanaOverlay::nueva(marco).context("no se pudo abrir la ventana de chat")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(&motor, &recursos.d3d(), ventana.handle(), ancho, alto)
        .context("sin superficie para el chat")?;
    ventana.mostrar();
    ventana.enfocar();

    let tema = if pixpin_shell::entorno::tema_claro() {
        &CLARO
    } else {
        &OSCURO
    };
    let mut ancho_lista = chat::ancho_inicial(ancho, escala);
    // Desde donde se agarro el asa, para que la columna no salte al cogerla
    // por un lado.
    let mut arrastrando: Option<i32> = None;
    let mut hay_que_pintar = true;

    loop {
        pixpin_shell::overlay::bombear_pendientes();
        let mut cerrar = false;
        let disposicion = Disposicion::calcular(ancho, alto, escala, ancho_lista, Vista::Ambas);

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
                    if disposicion.asa.contiene(l) {
                        arrastrando = Some(l.x - ancho_lista as i32);
                    }
                }
                EventoOverlay::RatonMovido(p) => {
                    let l = local(p);
                    if let Some(agarre) = arrastrando {
                        let nuevo = chat::ancho_ajustado(l.x - agarre, ancho, escala);
                        if nuevo != ancho_lista {
                            ancho_lista = nuevo;
                            hay_que_pintar = true;
                        }
                    }
                    // El cursor avisa de que ahi se puede arrastrar.
                    ventana.poner_cursor(if disposicion.asa.contiene(l) || arrastrando.is_some() {
                        FormaCursorWin::RedimEO
                    } else {
                        FormaCursorWin::Flecha
                    });
                }
                EventoOverlay::BotonSoltado(_) => arrastrando = None,
                EventoOverlay::Pintar => hay_que_pintar = true,
                EventoOverlay::Cerrar => cerrar = true,
                EventoOverlay::Tecla { vk, .. } if vk == VK_ESCAPE => cerrar = true,
                _ => {}
            }
        }
        if cerrar {
            break;
        }

        if hay_que_pintar {
            hay_que_pintar = false;
            if let Ok(destino) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&destino, |p: &Pintor| {
                    pintar(p, &disposicion, tema, escala, textos);
                });
                let _ = superficie.presentar();
            }
        }
        // Sin nada que hacer, el hilo duerme: la ventana abierta en reposo no
        // cuesta CPU.
        pixpin_shell::overlay::esperar_eventos(None);
    }

    ventana.ocultar();
    Ok(())
}

fn pintar(p: &Pintor, d: &Disposicion, tema: &Tema, escala: u32, textos: &Catalogo) {
    let e = escala as f32 / 100.0;
    p.limpiar(tema.chat);
    p.rellenar(rf(d.lista), tema.lista);
    p.rellenar(rf(d.cabecera_lista), tema.cabecera);
    p.rellenar(rf(d.cabecera_chat), tema.cabecera);

    // Las lineas que separan: 1 px logico, como Telegram.
    let linea = (1.0 * e).max(1.0);
    for (r, vertical) in [
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
                y: 0.0,
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

    // El titulo de la lista.
    if d.cabecera_lista.ancho > 0 {
        let tam = 15.0 * e;
        let (_, h) = p.medir_texto("Ay", tam);
        p.texto(
            &textos.t("chat-titulo"),
            d.cabecera_lista.x as f32 + 16.0 * e,
            d.cabecera_lista.y as f32 + (d.cabecera_lista.alto as f32 - h) / 2.0,
            tam,
            tema.texto,
        );
    }

    // Sin proyectos todavia: se dice, en vez de dejar dos columnas en blanco
    // que parecen un fallo.
    let centrado = |texto: &str, zona: Rect| {
        if zona.ancho == 0 {
            return;
        }
        let tam = 13.0 * e;
        let (w, h) = p.medir_texto(texto, tam);
        p.texto(
            texto,
            zona.x as f32 + (zona.ancho as f32 - w) / 2.0,
            zona.y as f32 + (zona.alto as f32 - h) / 2.0,
            tam,
            tema.apagado,
        );
    };
    centrado(&textos.t("chat-sin-proyectos"), d.filas);
    centrado(&textos.t("chat-elige-proyecto"), d.chat);
}
