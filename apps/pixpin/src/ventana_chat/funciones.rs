//! **Funciones** (25-oct-2026, el usuario: «arriba donde dice Chat ·
//! Figuras, que Figuras sea como Funciones, donde solo aparezcan unas pocas
//! funciones fuertes … y quitarlas de los chats»).
//!
//! La segunda parte del interruptor de la barra (`interruptor.rs`). Elegida,
//! la columna de la lista de chats —o, en una ventana estrecha con un chat
//! abierto, la columna entera— ensena una rejilla de mosaicos grandes con su
//! icono, su titulo y una linea de descripcion: Grabar reunion, Nuevo
//! lienzo, Tareas, Timeline, Horario, Capturas y Figuras. Las cuentas (donde
//! cae cada mosaico, las flechas) son de `pixpin_ui::funciones`; aqui se
//! pinta, se apunta donde quedo y se hace lo pulsado.
//!
//! Teclado: al elegir la pestana el teclado es de la rejilla hasta que se
//! pulse fuera; las flechas, Inicio y Fin recorren, Intro o Espacio hacen el
//! elegido y Escape vuelve al chat. Lo mismo en el movil y el Mac:
//! `docs/ui-funciones-y-chats.md`.

use std::cell::RefCell;

use pixpin_geom::{Punto, Rect};
use pixpin_render::icono::Icono;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_store::{Catalogo, Ubicacion};
use pixpin_ui::chat::Disposicion;
use pixpin_ui::funciones::{self as ui, Funcion, Paso};

use super::interruptor::Parte;
use super::{Abierto, OpcionesLienzo, Tema, con_alfa, hex, mi, rf};

const VK_TAB: u32 = 0x09;
const VK_ENTRAR: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_ESPACIO: u32 = 0x20;
const VK_FIN: u32 = 0x23;
const VK_INICIO: u32 = 0x24;
const VK_IZQUIERDA: u32 = 0x25;
const VK_ARRIBA: u32 = 0x26;
const VK_DERECHA: u32 = 0x27;
const VK_ABAJO: u32 = 0x28;

/// El icono de cada funcion.
fn icono(f: Funcion) -> &'static Icono {
    match f {
        Funcion::Grabar => &mi::MIC,
        Funcion::Lienzo => &mi::DRAW,
        Funcion::Tareas => &mi::CHECKLIST,
        Funcion::Timeline => &mi::TIMELINE,
        Funcion::Horario => &mi::CALENDAR_MONTH,
        Funcion::Galeria => &mi::PHOTO_LIBRARY,
        Funcion::Figuras => &mi::LAYERS,
    }
}

/// El color del circulo de su icono: el mismo de dia y de noche, con el
/// icono en blanco encima (se lee igual sobre los dos papeles).
fn acento(f: Funcion) -> Color {
    match f {
        Funcion::Grabar => hex(0xE5484D),
        Funcion::Lienzo => hex(0x8E4EC6),
        Funcion::Tareas => hex(0x1F7A3A),
        Funcion::Timeline => hex(0x0090FF),
        Funcion::Horario => hex(0xF76B15),
        Funcion::Galeria => hex(0x12A594),
        Funcion::Figuras => hex(0x3E63DD),
    }
}

/// Lo que contesta la rejilla a una tecla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Respuesta {
    /// La tecla no es suya: que siga su camino.
    NoEsMia,
    /// Se movio (o no hizo nada que se vea fuera).
    Sigue,
    Hacer(Funcion),
    /// Escape: de vuelta al chat.
    Volver,
}

/// La pestana de Funciones: si se ve, quien tiene el teclado y donde quedo
/// pintada.
#[derive(Default)]
pub(super) struct Funciones {
    abierta: bool,
    /// El teclado es de la rejilla: se eligio la pestana o se pulso en ella,
    /// y no se ha pulsado fuera despues.
    foco: bool,
    /// El mosaico bajo el raton.
    sobre: Option<usize>,
    /// El elegido con el teclado.
    elegida: Option<usize>,
    /// La zona y la rejilla del ultimo fotograma (sin pintar, ninguna).
    pintada: RefCell<Option<(Rect, ui::Rejilla)>>,
}

impl Funciones {
    /// La parte del interruptor que va puesta.
    pub(super) fn parte(&self) -> Parte {
        if self.abierta {
            Parte::Funciones
        } else {
            Parte::Chat
        }
    }

    /// Lo que hace pulsar `parte` en el interruptor.
    pub(super) fn elegir(&mut self, parte: Parte) {
        self.abierta = parte == Parte::Funciones;
        self.foco = self.abierta;
        self.sobre = None;
        if !self.abierta {
            self.elegida = None;
            self.pintada.replace(None);
        }
    }

    pub(super) fn tiene_el_teclado(&self) -> bool {
        self.abierta && self.foco
    }

    /// Si el punto cae en la zona de la rejilla pintada.
    pub(super) fn contiene(&self, l: Punto) -> bool {
        self.abierta
            && self
                .pintada
                .borrow()
                .as_ref()
                .is_some_and(|(zona, _)| zona.contiene(l))
    }

    /// La funcion del mosaico bajo el punto.
    pub(super) fn en(&self, l: Punto) -> Option<Funcion> {
        if !self.abierta {
            return None;
        }
        let i = self.pintada.borrow().as_ref()?.1.en(l)?;
        Funcion::TODAS.get(i).copied()
    }

    /// El raton se movio: el resalte le sigue. Dice si hay que repintar.
    pub(super) fn mover_raton(&mut self, l: Punto) -> bool {
        if !self.abierta {
            return false;
        }
        let ahora = self.pintada.borrow().as_ref().and_then(|(_, g)| g.en(l));
        let cambia = ahora != self.sobre;
        self.sobre = ahora;
        cambia
    }

    /// Se pulso en `l`: dentro de la rejilla el teclado es suyo, fuera deja
    /// de serlo (se esta escribiendo en otra cosa).
    pub(super) fn pulsado_en(&mut self, l: Punto) {
        self.foco = self.contiene(l);
        if let Some(i) = self.pintada.borrow().as_ref().and_then(|(_, g)| g.en(l)) {
            self.elegida = Some(i);
        }
    }

    /// Si la tecla es de las que se queda la rejilla (con el teclado suyo).
    pub(super) fn tecla_es_suya(&self, vk: u32) -> bool {
        self.tiene_el_teclado()
            && matches!(
                vk,
                VK_TAB
                    | VK_ENTRAR
                    | VK_ESCAPE
                    | VK_ESPACIO
                    | VK_FIN
                    | VK_INICIO
                    | VK_IZQUIERDA
                    | VK_ARRIBA
                    | VK_DERECHA
                    | VK_ABAJO
            )
    }

    /// Una tecla con el teclado en la rejilla.
    pub(super) fn tecla(&mut self, vk: u32) -> Respuesta {
        if !self.tiene_el_teclado() {
            return Respuesta::NoEsMia;
        }
        let cuantos = Funcion::TODAS.len();
        let columnas = self
            .pintada
            .borrow()
            .as_ref()
            .map_or(1, |(_, g)| g.columnas);
        let paso = match vk {
            VK_IZQUIERDA => Some(Paso::Izquierda),
            VK_DERECHA => Some(Paso::Derecha),
            VK_ARRIBA => Some(Paso::Arriba),
            VK_ABAJO => Some(Paso::Abajo),
            VK_INICIO => Some(Paso::Inicio),
            VK_FIN => Some(Paso::Fin),
            _ => None,
        };
        if let Some(paso) = paso {
            self.elegida = ui::mover(self.elegida, paso, columnas, cuantos);
            return Respuesta::Sigue;
        }
        match vk {
            // Tab da la vuelta, como en cualquier grupo de botones.
            VK_TAB => {
                self.elegida = Some(self.elegida.map_or(0, |i| (i + 1) % cuantos));
                Respuesta::Sigue
            }
            VK_ENTRAR | VK_ESPACIO => match self.elegida {
                Some(i) => Funcion::TODAS
                    .get(i)
                    .map_or(Respuesta::Sigue, |f| Respuesta::Hacer(*f)),
                None => {
                    self.elegida = Some(0);
                    Respuesta::Sigue
                }
            },
            VK_ESCAPE => Respuesta::Volver,
            _ => Respuesta::NoEsMia,
        }
    }

    /// Pinta la pestana encima de la lista (o de la columna, estrecha). Sin
    /// la pestana puesta no pinta nada y olvida donde estaba.
    pub(super) fn pintar(
        &self,
        p: &Pintor,
        d: &Disposicion,
        tema: &Tema,
        escala: u32,
        textos: &Catalogo,
    ) {
        let zona = zona_de(d);
        if !self.abierta || zona.ancho == 0 || zona.alto == 0 {
            self.pintada.replace(None);
            return;
        }
        let e = escala as f32 / 100.0;
        let g = ui::rejilla(zona, Funcion::TODAS.len(), escala);
        p.con_recorte(rf(zona), |p| {
            p.rellenar(rf(zona), tema.lista);
            if g.titulo.ancho > 0 {
                let t = textos.t("funciones-titulo");
                let tam = ui::TITULO_TAM * e;
                let (_, h) = p.medir_texto(&t, tam);
                p.texto(
                    &t,
                    g.titulo.x as f32,
                    g.titulo.y as f32 + (g.titulo.alto as f32 - h) / 2.0,
                    tam,
                    tema.texto,
                );
            }
            for (i, (f, r)) in Funcion::TODAS.iter().zip(&g.mosaicos).enumerate() {
                let elegido = self.foco && self.elegida == Some(i);
                pintar_mosaico(
                    p,
                    *f,
                    *r,
                    &g,
                    Estado {
                        elegido,
                        sobre: self.sobre == Some(i),
                    },
                    tema,
                    escala,
                    textos,
                );
            }
        });
        self.pintada.replace(Some((zona, g)));
    }
}

/// Donde va la pestana: la columna de la lista; si no se ve (ventana
/// estrecha con un chat abierto), toda la de debajo de la barra.
fn zona_de(d: &Disposicion) -> Rect {
    if d.lista.ancho > 0 {
        return d.lista;
    }
    let y = d.barra.abajo();
    Rect {
        x: d.chat.x,
        y,
        ancho: d.chat.ancho,
        alto: (d.chat.abajo() - y).max(0) as u32,
    }
}

#[derive(Clone, Copy)]
struct Estado {
    elegido: bool,
    sobre: bool,
}

#[allow(clippy::too_many_arguments)]
fn pintar_mosaico(
    p: &Pintor,
    f: Funcion,
    r: Rect,
    g: &ui::Rejilla,
    estado: Estado,
    tema: &Tema,
    escala: u32,
    textos: &Catalogo,
) {
    let e = escala as f32 / 100.0;
    let caja = rf(r);
    let (fondo, texto, detalle) = if estado.elegido {
        (
            tema.fila_elegida,
            tema.texto_elegido,
            con_alfa(tema.texto_elegido, 0.8),
        )
    } else if estado.sobre {
        (tema.fila_sobre, tema.texto, tema.apagado)
    } else {
        (tema.buscador, tema.texto, tema.apagado)
    };
    p.rellenar_redondeado(caja, ui::RADIO * e, fondo);
    let lado = ui::ICONO_LADO as f32 * e;
    let relleno = ui::RELLENO as f32 * e;
    // El circulo de color con el icono en blanco.
    let circulo = |cx: f32, cy: f32| {
        p.circulo((cx, cy), lado / 2.0, acento(f));
        let l = lado * 0.58;
        p.icono(
            icono(f),
            RectF {
                x: cx - l / 2.0,
                y: cy - l / 2.0,
                ancho: l,
                alto: l,
            },
            Color::BLANCO,
        );
    };
    if g.solo_iconos {
        circulo(caja.x + caja.ancho / 2.0, caja.y + caja.alto / 2.0);
        return;
    }
    let titulo = textos.t(f.clave_titulo());
    let ancho_texto = (caja.ancho - 2.0 * relleno).max(0.0);
    if g.con_detalle {
        // De arriba abajo: el icono, el titulo y su linea.
        circulo(caja.x + relleno + lado / 2.0, caja.y + relleno + lado / 2.0);
        let tam = ui::TAM_TITULO * e;
        let y = caja.y + relleno + lado + 8.0 * e;
        p.texto_linea(&titulo, caja.x + relleno, y, tam, ancho_texto, texto);
        let (_, h) = p.medir_texto(&titulo, tam);
        p.texto_linea(
            &textos.t(f.clave_detalle()),
            caja.x + relleno,
            y + h + 2.0 * e,
            ui::TAM_DETALLE * e,
            ancho_texto,
            detalle,
        );
    } else {
        // Bajito: el icono a la izquierda y el titulo a su lado.
        let cy = caja.y + caja.alto / 2.0;
        circulo(caja.x + relleno + lado / 2.0, cy);
        let tam = ui::TAM_TITULO * e;
        let (_, h) = p.medir_texto(&titulo, tam);
        let x = caja.x + relleno + lado + 10.0 * e;
        p.texto_linea(
            &titulo,
            x,
            cy - h / 2.0,
            tam,
            (caja.x + caja.ancho - relleno - x).max(0.0),
            texto,
        );
    }
}

/// Lo que hace falta para hacer una funcion desde el hilo del chat.
pub(super) struct Para<'a> {
    pub idioma: pixpin_store::Idioma,
    pub ubicacion: &'a Ubicacion,
    pub aparato: &'a str,
    pub textos: &'a Catalogo,
    pub lienzo: OpcionesLienzo,
}

/// **Hace la funcion**: cada una abre su ventana en su hilo, como hacian sus
/// botones de la cabecera de la lista y la bandeja; el chat sigue a lo suyo.
pub(super) fn hacer(f: Funcion, cx: &Para, abierto: Option<&mut Abierto>) {
    let (idioma, ubicacion) = (cx.idioma, cx.ubicacion);
    match f {
        Funcion::Grabar => super::grabar_clase(cx.textos, ubicacion, idioma, cx.aparato),
        Funcion::Lienzo => nuevo_lienzo(ubicacion, abierto, cx.aparato, cx.lienzo),
        Funcion::Tareas => crate::tareas::abrir(idioma, ubicacion.clone(), cx.aparato),
        // Con las lecciones dentro, en su pestana.
        Funcion::Timeline => crate::timeline::abrir(idioma, ubicacion.clone()),
        Funcion::Horario => crate::horario::abrir(idioma, ubicacion.clone(), cx.aparato),
        Funcion::Galeria => crate::galeria_capturas::abrir(idioma, ubicacion.clone()),
        Funcion::Figuras => super::figuras::abrir(idioma),
    }
}

/// **Nuevo lienzo**: uno en blanco en el chat abierto (sale en su
/// conversacion, como el «Lienzo» del «+») o, sin chat abierto, en
/// «Guardados»; y se abre al momento en su ventana.
fn nuevo_lienzo(
    ubicacion: &Ubicacion,
    abierto: Option<&mut Abierto>,
    aparato: &str,
    opciones: OpcionesLienzo,
) {
    let raiz = ubicacion.raiz();
    let creado = match abierto {
        Some(a) => super::crear_lienzo(ubicacion, a, aparato).map(|()| {
            a.scroll = None;
            a.colocado.borrow_mut().ancho = 0;
            (a.ficha.id.clone(), a.mensajes.last().cloned())
        }),
        None => pixpin_proyecto::almacen::asegurar_guardados(
            raiz,
            pixpin_shell::entorno::ahora_utc_ms(),
            aparato,
        )
        .and_then(|f| {
            let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &f.id);
            let numero = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta)
                .map(|c| c.mensajes.iter().map(|m| m.numero).max().unwrap_or(0))
                .unwrap_or(0)
                + 1;
            let m = super::escribir_lienzo(raiz, &f.id, aparato, numero, None)?;
            super::subir_en_la_lista(raiz, &f.id, m.cuando, &m.nombre)?;
            Ok((f.id, Some(m)))
        }),
    };
    let (proyecto, m) = match creado {
        Ok(hecho) => hecho,
        Err(e) => {
            tracing::warn!(?e, "no se pudo crear el lienzo nuevo");
            return;
        }
    };
    let Some((m, referencia)) = m.and_then(|m| {
        m.referencia
            .clone()
            .filter(|r| !r.is_empty())
            .map(|r| (m, r))
    }) else {
        return;
    };
    let raiz = raiz.to_path_buf();
    super::lienzo_aparte(format!("{proyecto}/{}", m.id), move || {
        crate::abrir_hoja::abrir_hoja(&raiz, &proyecto, &referencia, opciones);
    });
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn rejilla_pintada(f: &Funciones) {
        let zona = Rect {
            x: 0,
            y: 40,
            ancho: 320,
            alto: 700,
        };
        f.pintada
            .replace(Some((zona, ui::rejilla(zona, Funcion::TODAS.len(), 100))));
    }

    fn centro(r: Rect) -> Punto {
        Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        }
    }

    #[test]
    fn elegir_la_pestana_le_da_el_teclado_y_volver_al_chat_se_lo_quita() {
        let mut f = Funciones::default();
        assert_eq!(f.parte(), Parte::Chat);
        assert_eq!(f.tecla(VK_ABAJO), Respuesta::NoEsMia, "cerrada no es suya");
        f.elegir(Parte::Funciones);
        assert_eq!(f.parte(), Parte::Funciones);
        assert!(f.tiene_el_teclado());
        rejilla_pintada(&f);
        // La primera flecha elige el primero, Intro lo hace.
        assert_eq!(f.tecla(VK_ABAJO), Respuesta::Sigue);
        assert_eq!(f.tecla(VK_ENTRAR), Respuesta::Hacer(Funcion::Grabar));
        assert_eq!(f.tecla(VK_DERECHA), Respuesta::Sigue);
        assert_eq!(f.tecla(VK_ESPACIO), Respuesta::Hacer(Funcion::Lienzo));
        assert_eq!(f.tecla(VK_ABAJO), Respuesta::Sigue);
        assert_eq!(f.tecla(VK_ENTRAR), Respuesta::Hacer(Funcion::Timeline));
        // Una letra no es suya; Escape vuelve.
        assert_eq!(f.tecla(0x41), Respuesta::NoEsMia);
        assert_eq!(f.tecla(VK_ESCAPE), Respuesta::Volver);
        f.elegir(Parte::Chat);
        assert!(!f.tiene_el_teclado());
        assert!(
            !f.contiene(Punto { x: 10, y: 100 }),
            "sin pestana no hay rejilla"
        );
    }

    #[test]
    fn pulsar_fuera_de_la_rejilla_le_quita_el_teclado_y_dentro_se_lo_da() {
        let mut f = Funciones::default();
        f.elegir(Parte::Funciones);
        rejilla_pintada(&f);
        f.pulsado_en(Punto { x: 600, y: 300 });
        assert!(!f.tiene_el_teclado());
        assert_eq!(f.tecla(VK_ENTRAR), Respuesta::NoEsMia);
        let tercero = f.pintada.borrow().as_ref().unwrap().1.mosaicos[2];
        f.pulsado_en(centro(tercero));
        assert!(f.tiene_el_teclado());
        assert_eq!(f.en(centro(tercero)), Some(Funcion::Tareas));
        assert_eq!(f.tecla(VK_ENTRAR), Respuesta::Hacer(Funcion::Tareas));
    }

    #[test]
    fn el_resalte_sigue_al_raton_y_solo_pide_repintar_si_cambia() {
        let mut f = Funciones::default();
        f.elegir(Parte::Funciones);
        rejilla_pintada(&f);
        let primero = f.pintada.borrow().as_ref().unwrap().1.mosaicos[0];
        assert!(f.mover_raton(centro(primero)));
        assert!(
            !f.mover_raton(centro(primero)),
            "el mismo: nada que repintar"
        );
        assert!(f.mover_raton(Punto { x: 1, y: 41 }));
    }
}
