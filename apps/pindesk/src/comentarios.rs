//! **El comentario avanzado en los visores** (3-nov-2026): la pegatina de
//! comentario que se pega en cualquier punto, igual en todos.
//!
//! El usuario: «un emoticono de comentario, cuatro tipos (tres de urgencia
//! y uno de comentario normal), que pego donde sea como una pegatina; una
//! vez puesto funciona como atajo a una foto, un texto o un elemento como
//! una figura que cree, como un MINICHAT: mando una imagen y se abre en el
//! lienzo, y tambien puedo anotar sobre su comentario».
//!
//! # Un solo componente
//!
//! Cada visor guarda un [`Pegador`] de su documento y hace tres cosas:
//!
//! 1. Le pasa los eventos de su ventana antes que a lo suyo
//!    ([`Pegador::evento`]), con un [`Contexto`] que dice como van las
//!    anclas de su documento a la pantalla y vuelta ([`Ubicar`]).
//! 2. Lo pinta encima de lo suyo ([`Pegador::pintar_pegatinas`] con lo
//!    que se mueve con el documento, [`Pegador::pintar_interfaz`] con lo
//!    que va fijo: el boton, la tira, el menu y la lista).
//! 3. Atiende lo que pide ([`Pedido`]): ir a un sitio de su documento.
//!
//! Lo demas es igual en todos: el boton de comentar abajo a la derecha con
//! las abiertas; la tira con los cuatro tipos y la lista; elegir un tipo y
//! pulsar en el documento la pega y abre su minichat ([`minichat`]);
//! arrastrar la mueve; clic derecho, su menu (tipo, resolver, crear tarea,
//! quitar); la lista «Comentarios» por urgencia y fecha, con ir a cada una.
//!
//! Lo de dentro: el modelo en `pindesk_comentarios`, las cajas en
//! `pindesk_ui::pegatinas`, donde se guarda en [`sitio`] y el lienzo de las
//! entradas en [`lienzo`]. El formato, en `docs/comentarios.md`.

pub(crate) mod lienzo;
pub(crate) mod minichat;
pub(crate) mod pintar;
pub(crate) mod sitio;

use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime};

use pindesk_comentarios::lista::{self, Fila};
use pindesk_comentarios::{Adjunto, Ancla, Pegatinas, Quien, Tipo, fichero, fusion};
use pindesk_geom::Rect;
use pindesk_render::Pintor;
use pindesk_shell::overlay::{EventoOverlay, FormaCursorWin};
use pindesk_store::Catalogo;
use pindesk_ui::pegatinas::{
    self as ui, Arrastre, Boton, DestinoBoton, EnPantalla, Lista, Menu, Opcion,
};

pub(crate) use sitio::Sitio;

const VK_ESCAPE: u32 = 0x1B;

/// Cada cuanto se mira si el fichero cambio por fuera (la sincronizacion).
const MIRAR_CADA: Duration = Duration::from_millis(1500);

/// Los textos (el idioma del sistema, como los demas visores).
pub(crate) fn textos() -> &'static Catalogo {
    static TEXTOS: OnceLock<Catalogo> = OnceLock::new();
    TEXTOS.get_or_init(|| {
        Catalogo::nuevo(pindesk_store::resolver_idioma(
            &pindesk_shell::entorno::locale_del_sistema(),
            pindesk_store::ajustes::PreferenciaIdioma::Sistema,
        ))
    })
}

/// Quien escribe en este equipo: su nombre y su codigo de aparato.
pub(crate) fn quien(raiz: Option<&Path>) -> Quien {
    let raiz = raiz
        .map(Path::to_path_buf)
        .or_else(crate::asistente::documentos::raiz);
    match raiz.and_then(|r| pindesk_proyecto::identidad::Identidad::leer_o_crear(&r, "PC").ok()) {
        Some(i) => Quien {
            autor: i.yo.nombre,
            aparato: i.yo.id,
        },
        None => Quien {
            autor: "PC".into(),
            aparato: "pc".into(),
        },
    }
}

pub(crate) fn ahora() -> i64 {
    pindesk_shell::entorno::ahora_utc_ms()
}

/// **Como van las anclas de un visor** a su ventana y vuelta. Todo en
/// pixeles de la ventana.
pub(crate) trait Ubicar {
    /// Donde cae el ancla en la ventana; `None` si no se ve o no es de
    /// este visor.
    fn a_pantalla(&self, a: &Ancla) -> Option<(f32, f32)>;
    /// El ancla del punto de la ventana; `None` si ahi no hay documento.
    fn de_pantalla(&self, p: (f32, f32)) -> Option<Ancla>;
}

/// Lo que el componente necesita saber del visor en cada evento.
pub(crate) struct Contexto<'a> {
    /// La ventana en el escritorio (los eventos llegan en coordenadas del
    /// escritorio).
    pub area: Rect,
    /// La escala de la pantalla (1,0 = 96 ppp).
    pub e: f32,
    /// Lo que ocupa abajo lo del propio visor.
    pub reserva_abajo: f32,
    pub ubicar: &'a dyn Ubicar,
    /// El visor esta a medias con algo (un trazo): el raton es suyo.
    pub ocupado: bool,
    /// La ventana del visor: el minichat es suyo.
    pub dueno: isize,
    /// Lo que el boton de atajo del minichat puede adjuntar ahora (el
    /// elemento elegido en el lienzo).
    pub atajo: Option<Adjunto>,
}

/// Que se esta haciendo con el boton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Modo {
    Nada,
    /// La tira abierta.
    Tira,
    /// Un tipo elegido: el siguiente clic en el documento la pega.
    Plantando(Tipo),
}

/// Lo que el visor tiene que hacer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Pedido {
    /// Llevar la vista hasta ese sitio (de la lista).
    IrA(Box<Ancla>),
    /// Ir a un elemento de un lienzo (un atajo del minichat): la
    /// referencia del lienzo y donde estaba el elemento.
    Elemento {
        lienzo: String,
        punto: Option<[f64; 2]>,
    },
}

/// Lo que hizo el componente con un evento.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Respuesta {
    /// El evento era suyo: no sigue hacia el visor.
    pub consumido: bool,
    pub repintar: bool,
    pub cursor: Option<FormaCursorWin>,
    pub pedido: Option<Pedido>,
}

impl Respuesta {
    fn suya(repintar: bool) -> Respuesta {
        Respuesta {
            consumido: true,
            repintar,
            ..Respuesta::default()
        }
    }
}

/// **Las pegatinas de un documento abierto en un visor.**
pub(crate) struct Pegador {
    pub pegatinas: Pegatinas,
    pub sitio: Option<Sitio>,
    /// El texto del fichero tal como se leyo o escribio la ultima vez.
    leido: String,
    /// La fecha del fichero entonces (para ver si la cambio otro).
    fecha: Option<SystemTime>,
    mirado: Instant,
    /// Si el fichero estaba roto: no se escribe encima.
    roto: bool,
    pub modo: Modo,
    /// La lista abierta y desde que fila se ve.
    lista: Option<usize>,
    menu: Option<Menu>,
    arrastre: Option<Arrastre>,
    encima: Option<String>,
    encima_boton: Option<DestinoBoton>,
    raton: (f32, f32),
    quien: Quien,
}

fn fecha_de(f: &Path) -> Option<SystemTime> {
    std::fs::metadata(f).and_then(|m| m.modified()).ok()
}

impl Pegador {
    /// Sin fichero: las pegatinas valen mientras dura la ventana.
    pub(crate) fn sin_fichero() -> Pegador {
        Pegador {
            pegatinas: Pegatinas::default(),
            sitio: None,
            leido: String::new(),
            fecha: None,
            mirado: Instant::now(),
            roto: false,
            modo: Modo::Nada,
            lista: None,
            menu: None,
            arrastre: None,
            encima: None,
            encima_boton: None,
            raton: (-1.0, -1.0),
            quien: quien(None),
        }
    }

    pub(crate) fn en(sitio: Sitio) -> Pegador {
        let mut p = Pegador {
            quien: quien(sitio.raiz()),
            sitio: Some(sitio),
            ..Pegador::sin_fichero()
        };
        p.releer();
        p
    }

    /// Las de un documento de un visor.
    pub(crate) fn del_documento(doc: &Path) -> Pegador {
        Pegador::en(Sitio::del_documento(doc))
    }

    /// Las de un lienzo.
    pub(crate) fn del_lienzo(dibujo: &Path) -> Pegador {
        Pegador::en(Sitio::del_lienzo(dibujo))
    }

    fn releer(&mut self) {
        let Some(s) = &self.sitio else {
            return;
        };
        let texto = std::fs::read_to_string(&s.fichero).unwrap_or_default();
        match fichero::leer(&texto) {
            Ok(p) => {
                self.pegatinas = p;
                self.roto = false;
            }
            Err(e) => {
                tracing::warn!(%e, ruta = %s.fichero.display(), "fichero de comentarios roto: no se escribe encima");
                self.roto = true;
            }
        }
        self.leido = texto;
        self.fecha = fecha_de(&s.fichero);
    }

    /// **Si el fichero cambio por fuera** (llego de otro aparato, o lo
    /// cambio otro visor), lo junta con lo de aqui. Barato: mira la fecha
    /// como mucho cada segundo y medio. Devuelve si hay que repintar.
    pub(crate) fn al_dia(&mut self) -> bool {
        if self.mirado.elapsed() < MIRAR_CADA || self.arrastre.is_some() {
            return false;
        }
        self.mirado = Instant::now();
        let Some(s) = &self.sitio else {
            return false;
        };
        if fecha_de(&s.fichero) == self.fecha {
            return false;
        }
        let mio = self.pegatinas.clone();
        self.releer();
        self.pegatinas = fusion::juntar(&self.pegatinas, &mio);
        true
    }

    /// **Guarda**, juntando con lo que haya en el disco (por si llego algo
    /// mientras): nunca pisa lo de otro.
    pub(crate) fn guardar(&mut self) {
        let Some(s) = &self.sitio else {
            return;
        };
        if self.roto {
            return;
        }
        let en_disco = std::fs::read_to_string(&s.fichero).unwrap_or_default();
        let Ok(de_disco) = fichero::leer(&en_disco) else {
            self.roto = true;
            return;
        };
        let junta = fusion::juntar(&de_disco, &self.pegatinas);
        match fichero::escribir(&en_disco, &junta) {
            Ok(t) => {
                if let Some(p) = s.fichero.parent() {
                    let _ = std::fs::create_dir_all(p);
                }
                if let Err(e) = pindesk_proyecto::anotado::escribir(&s.fichero, &t) {
                    tracing::warn!(?e, ruta = %s.fichero.display(), "no se pudieron guardar los comentarios");
                }
                self.leido = t;
                self.fecha = fecha_de(&s.fichero);
            }
            Err(e) => tracing::warn!(%e, "no se pudieron escribir los comentarios"),
        }
        self.pegatinas = junta;
    }

    /// Las que se ven, donde caen (la que se arrastra, donde va).
    pub(crate) fn en_pantalla(&self, ubicar: &dyn Ubicar) -> Vec<EnPantalla> {
        self.pegatinas
            .vivas()
            .filter_map(|c| {
                let ancla = match &self.arrastre {
                    Some(a) if a.id == c.id && a.movido => a.ancla(),
                    _ => ubicar.a_pantalla(&c.ancla)?,
                };
                Some(EnPantalla {
                    id: c.id.clone(),
                    tipo: c.tipo,
                    ancla,
                    resuelto: c.resuelto,
                    entradas: c.entradas().count(),
                })
            })
            .collect()
    }

    fn boton(&self, cx: &Contexto) -> Boton {
        Boton::colocar(
            cx.area.ancho as f32,
            cx.area.alto as f32,
            cx.e,
            self.modo != Modo::Nada,
            cx.reserva_abajo,
        )
    }

    fn filas(&self) -> Vec<Fila> {
        lista::filas(&self.pegatinas)
    }

    fn colocar_lista(&self, desde: usize, cx: &Contexto) -> Lista {
        Lista::colocar(
            self.filas().len(),
            desde,
            &self.boton(cx).boton,
            (cx.area.ancho as f32, cx.area.alto as f32),
            cx.e,
        )
    }

    /// Si hay algo abierto (tira, menu, lista, plantando): Escape es suyo.
    pub(crate) fn ocupado(&self) -> bool {
        self.modo != Modo::Nada
            || self.menu.is_some()
            || self.lista.is_some()
            || self.arrastre.is_some()
    }

    /// **Atiende un evento** de la ventana del visor. Lo que no es suyo sale
    /// con `consumido: false` y sigue su camino.
    pub(crate) fn evento(&mut self, ev: &EventoOverlay, cx: &Contexto) -> Respuesta {
        let local = |p: &pindesk_geom::Punto| ((p.x - cx.area.x) as f32, (p.y - cx.area.y) as f32);
        let e = cx.e;
        match ev {
            EventoOverlay::Tecla { vk: VK_ESCAPE, .. } if self.ocupado() => {
                self.cerrar_todo();
                Respuesta::suya(true)
            }
            EventoOverlay::BotonPulsado(p) => {
                let q = local(p);
                self.raton = q;
                // El menu abierto: una opcion, o fuera lo cierra.
                if let Some(m) = self.menu.take() {
                    if let Some(o) = m.opcion_en(q) {
                        return self.opcion(&m.id, o, cx);
                    }
                    return Respuesta::suya(true);
                }
                if let Some(desde) = self.lista {
                    let l = self.colocar_lista(desde, cx);
                    if l.cerrar.dentro(q) {
                        self.lista = None;
                        return Respuesta::suya(true);
                    }
                    if let Some(i) = l.fila_en(q) {
                        let filas = self.filas();
                        if let Some(c) = filas.get(i).and_then(|f| self.pegatinas.get(&f.id)) {
                            return Respuesta {
                                pedido: Some(Pedido::IrA(Box::new(c.ancla.clone()))),
                                ..Respuesta::suya(true)
                            };
                        }
                    }
                    if l.caja.dentro(q) {
                        return Respuesta::suya(false);
                    }
                }
                match self.boton(cx).destino(q) {
                    Some(DestinoBoton::Comentar) => {
                        self.modo = if self.modo == Modo::Nada {
                            Modo::Tira
                        } else {
                            Modo::Nada
                        };
                        return Respuesta::suya(true);
                    }
                    Some(DestinoBoton::Tipo(t)) => {
                        self.modo = Modo::Plantando(t);
                        return Respuesta {
                            cursor: Some(FormaCursorWin::Cruz),
                            ..Respuesta::suya(true)
                        };
                    }
                    Some(DestinoBoton::Lista) => {
                        self.lista = if self.lista.is_some() { None } else { Some(0) };
                        return Respuesta::suya(true);
                    }
                    Some(DestinoBoton::Hueco) => return Respuesta::suya(false),
                    None => {}
                }
                if let Modo::Plantando(t) = self.modo {
                    let Some(ancla) = cx.ubicar.de_pantalla(q) else {
                        // Fuera del documento no se pega; tampoco es suyo
                        // dibujar ahi mientras se planta.
                        return Respuesta::suya(false);
                    };
                    let id = self.pegatinas.poner(t, ancla, &self.quien, ahora());
                    self.modo = Modo::Nada;
                    self.guardar();
                    let pedido = self.abrir_chat(&id, cx);
                    return Respuesta {
                        pedido,
                        ..Respuesta::suya(true)
                    };
                }
                let lista = self.en_pantalla(cx.ubicar);
                if let Some(i) = ui::bajo(&lista, q, e) {
                    let x = &lista[i];
                    self.arrastre = Some(Arrastre::nuevo(&x.id, q, x.ancla));
                    return Respuesta {
                        cursor: Some(FormaCursorWin::Mover),
                        ..Respuesta::suya(false)
                    };
                }
                Respuesta::default()
            }
            EventoOverlay::BotonDerechoPulsado(p) => {
                let q = local(p);
                self.raton = q;
                let lista = self.en_pantalla(cx.ubicar);
                if let Some(i) = ui::bajo(&lista, q, e)
                    && let Some(c) = self.pegatinas.get(&lista[i].id)
                {
                    self.menu = Some(Menu::colocar(
                        &c.id,
                        &ui::opciones(c),
                        q,
                        (cx.area.ancho as f32, cx.area.alto as f32),
                        e,
                    ));
                    return Respuesta::suya(true);
                }
                if self.menu.take().is_some() {
                    return Respuesta::suya(true);
                }
                if self.boton(cx).destino(q).is_some()
                    || self
                        .lista
                        .is_some_and(|d| self.colocar_lista(d, cx).caja.dentro(q))
                {
                    return Respuesta::suya(false);
                }
                Respuesta::default()
            }
            EventoOverlay::RatonMovido(p) => {
                let q = local(p);
                self.raton = q;
                if let Some(a) = &mut self.arrastre {
                    let antes = a.movido;
                    let ahora = a.mover(q, e);
                    return Respuesta {
                        consumido: true,
                        repintar: ahora || antes,
                        cursor: Some(FormaCursorWin::Mover),
                        pedido: None,
                    };
                }
                if cx.ocupado {
                    return Respuesta::default();
                }
                let mut repintar = false;
                let b = self.boton(cx).destino(q);
                if b != self.encima_boton {
                    self.encima_boton = b;
                    repintar = true;
                }
                if self.menu.is_some() || self.lista.is_some() {
                    repintar = true;
                }
                let lista = self.en_pantalla(cx.ubicar);
                let encima = ui::bajo(&lista, q, e).map(|i| lista[i].id.clone());
                if encima != self.encima {
                    self.encima = encima;
                    repintar = true;
                }
                let sobre_lo_suyo = b.is_some()
                    || self.menu.as_ref().is_some_and(|m| m.caja.dentro(q))
                    || self
                        .lista
                        .is_some_and(|d| self.colocar_lista(d, cx).caja.dentro(q));
                let cursor = if sobre_lo_suyo {
                    Some(FormaCursorWin::Flecha)
                } else if self.encima.is_some() {
                    // La mano (`IDC_HAND`): se puede pulsar y coger.
                    Some(FormaCursorWin::Giro)
                } else if matches!(self.modo, Modo::Plantando(_)) {
                    Some(FormaCursorWin::Cruz)
                } else {
                    None
                };
                Respuesta {
                    consumido: sobre_lo_suyo || self.encima.is_some(),
                    repintar,
                    cursor,
                    pedido: None,
                }
            }
            EventoOverlay::BotonSoltado(p) => {
                let Some(a) = self.arrastre.take() else {
                    return Respuesta::default();
                };
                let q = local(p);
                if a.movido {
                    let mut a = a;
                    a.mover(q, e);
                    if let Some(ancla) = cx.ubicar.de_pantalla(a.ancla()) {
                        self.pegatinas.mover(&a.id, ancla, ahora());
                        self.guardar();
                    }
                    return Respuesta::suya(true);
                }
                // Un clic: su minichat.
                let pedido = self.abrir_chat(&a.id, cx);
                Respuesta {
                    pedido,
                    ..Respuesta::suya(true)
                }
            }
            EventoOverlay::Rueda(d) => {
                let Some(desde) = self.lista else {
                    return Respuesta::default();
                };
                let l = self.colocar_lista(desde, cx);
                if !l.caja.dentro(self.raton) {
                    return Respuesta::default();
                }
                let n = self.filas().len();
                let tope = n.saturating_sub(l.caben);
                let nuevo = if *d > 0 {
                    desde.saturating_sub(1)
                } else {
                    (desde + 1).min(tope)
                };
                self.lista = Some(nuevo);
                Respuesta::suya(nuevo != desde)
            }
            _ => Respuesta::default(),
        }
    }

    fn cerrar_todo(&mut self) {
        self.modo = Modo::Nada;
        self.menu = None;
        self.lista = None;
        self.arrastre = None;
    }

    fn opcion(&mut self, id: &str, o: Opcion, cx: &Contexto) -> Respuesta {
        let cuando = ahora();
        let cambio = match o {
            Opcion::Abrir => {
                let pedido = self.abrir_chat(id, cx);
                return Respuesta {
                    pedido,
                    ..Respuesta::suya(true)
                };
            }
            Opcion::Tipo(t) => self.pegatinas.cambiar_tipo(id, t, cuando),
            Opcion::Resolver => self.pegatinas.resolver(id, true, cuando),
            Opcion::Reabrir => self.pegatinas.resolver(id, false, cuando),
            Opcion::CrearTarea => self.crear_tarea(id),
            Opcion::Quitar => self.pegatinas.quitar(id, cuando),
        };
        if cambio {
            self.guardar();
        }
        Respuesta::suya(true)
    }

    /// **«Crear tarea»** de una pegatina de urgencia: una tarea en el Inbox
    /// con su emoticono, lo que dice y de que documento es, y la pegatina
    /// queda apuntando a ella.
    pub(crate) fn crear_tarea(&mut self, id: &str) -> bool {
        let Some(c) = self.pegatinas.get(id).cloned() else {
            return false;
        };
        if !lista::se_puede_crear_tarea(&c) {
            return false;
        }
        let raiz = self
            .sitio
            .as_ref()
            .and_then(|s| s.raiz().map(Path::to_path_buf))
            .or_else(crate::asistente::documentos::raiz);
        let Some(raiz) = raiz else {
            tracing::warn!("sin datos donde apuntar la tarea del comentario");
            return false;
        };
        let doc = self
            .sitio
            .as_ref()
            .map(|s| s.nombre.clone())
            .unwrap_or_default();
        let texto = lista::texto_de_tarea(&c, &doc, &textos().t(c.tipo.clave()));
        match crate::tareas::apuntar_con(&raiz, &self.quien.aparato, &texto, &[]) {
            Ok(m) => {
                crate::tareas::refrescar();
                self.pegatinas
                    .vincular_tarea(id, &m.codigo_unico(), ahora())
            }
            Err(e) => {
                tracing::warn!(?e, "no se pudo crear la tarea del comentario");
                false
            }
        }
    }

    /// **Abre el minichat** de una pegatina y no vuelve hasta cerrarlo.
    pub(crate) fn abrir_chat(&mut self, id: &str, cx: &Contexto) -> Option<Pedido> {
        let ancla = self
            .pegatinas
            .get(id)
            .and_then(|c| cx.ubicar.a_pantalla(&c.ancla))
            .unwrap_or((cx.area.ancho as f32 / 2.0, cx.area.alto as f32 / 2.0));
        let c = ui::centro(ancla, cx.e);
        let en_escritorio = (cx.area.x as f32 + c.0, cx.area.y as f32 + c.1);
        let pedido = minichat::abrir(self, id, en_escritorio, cx.dueno, cx.atajo.clone());
        self.encima = None;
        pedido
    }

    /// **Lo que se mueve con el documento**: las pegatinas, en pixeles de la
    /// ventana (quien pinte corrido, que corra antes el pintor).
    pub(crate) fn pintar_pegatinas(&self, p: &Pintor<'_>, ubicar: &dyn Ubicar, e: f32) {
        let lista = self.en_pantalla(ubicar);
        pintar::pegatinas(p, &lista, e, self.encima.as_deref());
    }

    /// Si alguna pegatina cae en la zona (pixeles de la ventana): quien
    /// hornea un trazo encima de lo pintado lo pregunta, como con las marcas.
    pub(crate) fn alguna_toca(
        &self,
        ubicar: &dyn Ubicar,
        zona: (f32, f32, f32, f32),
        e: f32,
    ) -> bool {
        let r = (ui::RADIO + ui::PIQUITO + 4.0) * e;
        self.en_pantalla(ubicar).iter().any(|x| {
            let c = ui::centro(x.ancla, e);
            c.0 + r >= zona.0 && c.0 - r <= zona.2 && c.1 + r >= zona.1 && x.ancla.1 - r <= zona.3
        })
    }

    /// **Lo que va fijo**: el boton, la tira, el aviso de plantar, el menu y
    /// la lista.
    pub(crate) fn pintar_interfaz(&self, p: &Pintor<'_>, cx: &Contexto, arriba: f32) {
        let e = cx.e;
        let ventana = (cx.area.ancho as f32, cx.area.alto as f32);
        let plantando = match self.modo {
            Modo::Plantando(t) => Some(t),
            _ => None,
        };
        pintar::boton(
            p,
            &self.boton(cx),
            e,
            self.pegatinas.abiertas(),
            self.modo != Modo::Nada,
            plantando,
            self.encima_boton,
        );
        if let Some(t) = plantando {
            pintar::aviso(
                p,
                &textos().t("comentario-plantando"),
                t,
                ventana,
                arriba + 8.0 * e,
                e,
            );
        }
        if let Some(desde) = self.lista {
            let l = self.colocar_lista(desde, cx);
            pintar::lista(
                p,
                &l,
                &self.filas(),
                textos(),
                e,
                self.raton,
                pindesk_shell::entorno::ahora_local_ms(),
            );
        }
        if let Some(m) = &self.menu
            && let Some(c) = self.pegatinas.get(&m.id)
        {
            pintar::menu(p, m, c, textos(), e, self.raton);
        }
    }
}
