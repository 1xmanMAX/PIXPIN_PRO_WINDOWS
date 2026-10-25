//! **La ventana de una carpeta de paginas** (26-oct-2026, el usuario:
//! «dentro del chat se puede crear una carpeta con nombre, como si fuera un
//! PDF; al abrirla se ven todas sus paginas en orden: Dia 1, Dia 2, Dia 3…»).
//!
//! Se abre al pulsar la tarjeta de la carpeta en el chat. Una por carpeta,
//! cada una en su hilo, como la de las figuras:
//!
//! - **Arriba**, el nombre y cuantas paginas lleva, y los dos botones:
//!   «Nueva pagina» (la plantilla de la ultima otra vez, vacia, con el
//!   titulo siguiente) y «Exportar PDF» (una pagina por cada una, con el
//!   dialogo de guardar de siempre).
//! - **Debajo, las paginas en orden**, cada una con su miniatura, su numero y
//!   su titulo. Se abren con un clic, se cambian de sitio arrastrandolas (o
//!   con sus flechas, o Ctrl+flechas), se renombran (lapiz o F2) y se quitan
//!   de la carpeta (aspa o Supr): el lienzo sigue en el chat.
//!
//! **La carpeta de una grabacion** (25-oct-2026) se ve igual, adaptada: la
//! nota y el video con su icono y lo que duran, las capturas con su foto y
//! su minuto en la esquina, en el orden de la grabacion (no se arrastran:
//! el orden es el del tiempo). Detras, **lo adjuntado a la clase**, con otro
//! color y la chapa «Adjunto · no es de la grabacion». Arriba, «Estudiar»
//! (la clase en su ventana), «Adjuntar a la clase» y «Exportar PDF» (las
//! capturas y el texto de la nota). Pulsar una pieza de la grabacion abre
//! la clase en su minuto.
//!
//! La carpeta vive en el cuaderno (`pixpin_proyecto::carpeta_de_paginas`):
//! todo lo que se hace aqui se escribe alli en el momento, con el cerrojo
//! del cuaderno, y se avisa al chat para que relea. Lo que cambie por fuera
//! (un lienzo que se guardo, una pagina anadida desde el chat) se ve aqui
//! al mirar el disco cada dos segundos.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use pixpin_geom::{Punto, Rect};
use pixpin_proyecto::carpeta_de_paginas::{self as carpetas, Carpeta};
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje};
use pixpin_render::icono::Icono;
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma, Ubicacion};

use super::{
    CLARO, LienzoVisto, OSCURO, Ojeada, OpcionesLienzo, Tema, con_alfa, mi, pintar_lienzo, rf,
};
use crate::overlay::Recursos;

/// El color de lo adjuntado a una clase (no es de lo grabado): un ambar
/// que se lee igual en claro y en oscuro.
const COLOR_ADJUNTO: Color = Color {
    r: 0.80,
    g: 0.45,
    b: 0.05,
    a: 1.0,
};

/// Lo que mide cada pagina de la rejilla (logicos).
const CELDA_ANCHO: u32 = 196;
const CELDA_DIBUJO: u32 = 252;
const CELDA_TITULO: u32 = 28;
const CELDA_BOTONES: u32 = 30;
const HUECO: u32 = 18;
const MARGEN: u32 = 24;
const BARRA: u32 = 56;
const PISTA: u32 = 30;
const BOTON_ALTO: u32 = 36;
/// Lo que hay que mover el raton con el boton pulsado para que sea
/// arrastrar y no un clic.
const ARRASTRE_MINIMO: i32 = 6;
/// Cada cuanto se mira si lo guardado cambio por fuera.
const MIRAR_CADA: std::time::Duration = std::time::Duration::from_secs(2);
const AVISO: std::time::Duration = std::time::Duration::from_millis(3000);

const VK_RETROCESO: u32 = 0x08;
const VK_ENTRAR: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_IZQUIERDA: u32 = 0x25;
const VK_ARRIBA: u32 = 0x26;
const VK_DERECHA: u32 = 0x27;
const VK_ABAJO: u32 = 0x28;
const VK_SUPR: u32 = 0x2E;
const VK_F2: u32 = 0x71;

/// Las ventanas abiertas, por carpeta (`proyecto/id`): su HWND, o -1
/// mientras nace.
static ABIERTAS: Mutex<Vec<(String, isize)>> = Mutex::new(Vec::new());

/// Lo que hace falta para abrir una carpeta desde el chat.
#[derive(Clone)]
pub(super) struct Abrir {
    pub idioma: Idioma,
    pub ubicacion: Ubicacion,
    pub proyecto: String,
    /// El `id` del mensaje de la carpeta.
    pub id: String,
    /// El codigo de este equipo, para sellar las paginas nuevas.
    pub identidad: String,
    pub lienzo: OpcionesLienzo,
}

/// **Abre la ventana de una carpeta**, o trae delante la que ya estaba.
pub(super) fn abrir(que: Abrir) {
    let clave = format!("{}/{}", que.proyecto, que.id);
    {
        let Ok(mut v) = ABIERTAS.lock() else { return };
        match v.iter().find(|(k, _)| *k == clave).map(|(_, h)| *h) {
            Some(h) if h > 0 => {
                VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(h as *mut _));
                return;
            }
            Some(_) => return,
            None => v.push((clave.clone(), -1)),
        }
    }
    let soltar = {
        let clave = clave.clone();
        move || {
            if let Ok(mut v) = ABIERTAS.lock() {
                v.retain(|(k, _)| *k != clave);
            }
        }
    };
    let lanzado = std::thread::Builder::new().name("carpeta".into()).spawn({
        let soltar = soltar.clone();
        move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(que.idioma);
            let mut v = Vista::nueva(que, &textos);
            v.recargar();
            let hecho = crate::dispositivo_perdido::con_recursos("carpeta", |r| {
                bucle(r, &textos, &mut v, &clave)
            });
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la ventana de la carpeta");
            }
            soltar();
        }
    });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de la carpeta");
        soltar();
    }
}

// --- Lo de cada pagina ---------------------------------------------------------

/// Una pagina a la vista, ya leida.
struct PaginaVista {
    /// El mensaje que es la pagina.
    mensaje: Mensaje,
    /// Su titulo en la carpeta, o el nombre del mensaje si no lleva.
    titulo: String,
    dibujo: Option<LienzoVisto>,
    /// Lo que se ensena si no hay dibujo.
    icono: &'static Icono,
    /// En una grabacion: que se adjunto a la clase y no es de lo grabado.
    adjunto: bool,
    /// Lo que va en su esquina: el numero de la pagina o, en una grabacion,
    /// su minuto («⏱ 1:05») o lo que dura la nota.
    esquina: String,
}

/// La miniatura de una pagina: un lienzo con lo dibujado, una foto con lo
/// anotado encima, una pagina de PDF. Lo demas (una nota, un archivo), sin
/// miniatura: su icono.
fn dibujo_de(ubicacion: &Ubicacion, proyecto: &str, m: &Mensaje) -> Option<LienzoVisto> {
    match super::leer_vista(ubicacion, proyecto, m)? {
        Ojeada::Lienzo(l) => Some(l),
        Ojeada::Foto { doc, dibujo, .. } => {
            let ruta = super::ruta_del_mensaje(ubicacion.raiz(), proyecto, m)?;
            Some(LienzoVisto {
                ordenes: dibujo,
                grafitos: Vec::new(),
                caja: (0.0, 0.0, doc.0, doc.1),
                fondo: Some((ruta, (0.0, 0.0, doc.0, doc.1))),
                giro_del_fondo: 0.0,
                papel: None,
            })
        }
        _ => None,
    }
}

fn icono_de(m: &Mensaje) -> &'static Icono {
    if cuaderno::es_video(m) {
        return &mi::VIDEOCAM;
    }
    match m.clase {
        Some(Clase::Dibujo) => &mi::DRAW,
        Some(Clase::Pagina) => &mi::MENU_BOOK,
        Some(Clase::Imagen) => &mi::IMAGE,
        Some(Clase::Voz) => &mi::MIC,
        Some(Clase::Nota) | None => &mi::EDIT_NOTE,
        _ => &mi::DESCRIPTION,
    }
}

// --- El estado -------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zona {
    Barra,
    Cerrar,
    NuevaPagina,
    Exportar,
    Estudiar,
    Adjuntar,
    Pagina(usize),
    Antes(usize),
    Despues(usize),
    Renombrar(usize),
    Quitar(usize),
}

/// Lo que va por otro hilo y vuelve: el PDF que se esta escribiendo.
type Resultado = Arc<Mutex<Option<String>>>;

struct Vista {
    que: Abrir,
    raiz: PathBuf,
    dir: PathBuf,
    nombre: String,
    carpeta: Carpeta,
    /// Si es la carpeta de una grabacion (ver la cabecera).
    grabacion: bool,
    /// El chat entero, como se leyo (las paginas apuntan aqui).
    mensajes: Vec<Mensaje>,
    paginas: Vec<PaginaVista>,
    /// Si el mensaje de la carpeta ya no esta (se borro en el chat).
    perdida: bool,
    elegida: Option<usize>,
    /// La pagina cuyo titulo se esta escribiendo, y lo escrito.
    renombrando: Option<(usize, String)>,
    /// Donde se pulso (pagina y punto) y si ya se esta arrastrando.
    pulsada: Option<(usize, Punto)>,
    arrastrando: bool,
    scroll: i32,
    raton: Punto,
    aviso: Option<(String, std::time::Instant)>,
    exportando: Option<Resultado>,
    zonas: RefCell<Vec<(Rect, Zona)>>,
    alto_total: Cell<i32>,
    columnas: Cell<usize>,
    /// Lo que se miro del disco la ultima vez: si cambia, se relee.
    huella: Vec<Option<std::time::SystemTime>>,
    textos_pagina: String,
    /// Los textos, para el nombre de lo que no lleva titulo.
    catalogo: Catalogo,
}

impl Vista {
    fn nueva(que: Abrir, textos: &Catalogo) -> Vista {
        let que_idioma = que.idioma;
        let raiz = que.ubicacion.raiz().to_path_buf();
        let dir = pixpin_proyecto::almacen::carpeta(&raiz, &que.proyecto);
        Vista {
            que,
            raiz,
            dir,
            nombre: String::new(),
            carpeta: Carpeta::default(),
            grabacion: false,
            mensajes: Vec::new(),
            paginas: Vec::new(),
            perdida: false,
            elegida: None,
            renombrando: None,
            pulsada: None,
            arrastrando: false,
            scroll: 0,
            raton: Punto { x: -1, y: -1 },
            aviso: None,
            exportando: None,
            zonas: RefCell::new(Vec::new()),
            alto_total: Cell::new(0),
            columnas: Cell::new(1),
            huella: Vec::new(),
            textos_pagina: textos.t("carpeta-pagina"),
            catalogo: Catalogo::nuevo(que_idioma),
        }
    }

    /// Lo que se mira para saber si hay que releer: el cuaderno y el lienzo
    /// de cada pagina (un lienzo guardado cambia su miniatura, no el chat).
    fn huella_actual(&self) -> Vec<Option<std::time::SystemTime>> {
        let fecha = |r: &std::path::Path| std::fs::metadata(r).and_then(|m| m.modified()).ok();
        let mut v = vec![fecha(&self.dir.join("guardados.jsonl"))];
        for p in &self.paginas {
            if let Some(r) = p.mensaje.referencia.as_deref().filter(|r| !r.is_empty()) {
                v.push(fecha(&pixpin_proyecto::almacen::lienzo(
                    &self.raiz,
                    &self.que.proyecto,
                    r,
                )));
            }
        }
        v
    }

    /// **Lee la carpeta y sus paginas del disco.** Lo que se borro en el
    /// chat se poda de la carpeta en el momento.
    fn recargar(&mut self) {
        let elegida = self
            .elegida
            .and_then(|i| self.paginas.get(i))
            .map(|p| p.mensaje.id.clone());
        let mut mensajes = match cuaderno::Cuaderno::leer_de(&self.dir) {
            Ok(c) => c.mensajes,
            Err(e) => {
                tracing::warn!(?e, "no se pudo leer el cuaderno de la carpeta");
                return;
            }
        };
        mensajes.sort_by_key(|m| m.cuando);
        let Some(m) = mensajes.iter().find(|m| m.id == self.que.id) else {
            self.perdida = true;
            self.paginas.clear();
            self.mensajes = mensajes;
            self.huella = self.huella_actual();
            return;
        };
        self.perdida = false;
        self.nombre = super::nombre_de_la_fila(m, &self.catalogo);
        let Some(carpeta) = Carpeta::de_mensaje(m) else {
            tracing::warn!(id = %self.que.id, "carpeta que no se entiende; no se toca");
            self.paginas.clear();
            self.mensajes = mensajes;
            return;
        };
        // Lo borrado se quita ya: asi las posiciones de aqui y las de la
        // carpeta guardada son las mismas.
        if carpeta.vivas(&mensajes).len() != carpeta.len() {
            match carpetas::cambiar_en_disco(&self.dir, &self.que.id, |_| false) {
                Ok(Some(c)) => self.carpeta = c,
                _ => self.carpeta = carpeta,
            }
        } else {
            self.carpeta = carpeta;
        }
        let ubicacion = self.que.ubicacion.clone();
        self.grabacion = self.carpeta.es_grabacion();
        let grabacion = self.grabacion;
        let mut vivas = self.carpeta.vivas(&mensajes);
        // En una grabacion, lo grabado delante y lo adjuntado detras.
        if grabacion {
            vivas.sort_by_key(|(_, p)| p.adjunto);
        }
        self.paginas = vivas
            .into_iter()
            .enumerate()
            .map(|(k, (i, p))| {
                let m = mensajes[i].clone();
                let titulo = if p.titulo.trim().is_empty() {
                    super::nombre_de_la_fila(&m, &self.catalogo)
                } else {
                    p.titulo.clone()
                };
                let esquina = match (grabacion, p.adjunto, &m.clase) {
                    (true, false, Some(Clase::Voz)) => {
                        super::grabar_clase::minuto(m.duracion_ms.max(0))
                    }
                    (true, false, _) if !cuaderno::es_video(&m) => {
                        format!("⏱ {}", super::grabar_clase::minuto(p.ms.unwrap_or(0)))
                    }
                    (true, _, _) => String::new(),
                    (false, _, _) => (k + 1).to_string(),
                };
                PaginaVista {
                    dibujo: dibujo_de(&ubicacion, &self.que.proyecto, &m),
                    icono: icono_de(&m),
                    titulo,
                    adjunto: grabacion && p.adjunto,
                    esquina,
                    mensaje: m,
                }
            })
            .collect();
        self.mensajes = mensajes;
        self.elegida = elegida
            .and_then(|id| self.paginas.iter().position(|p| p.mensaje.id == id))
            .or(if self.paginas.is_empty() {
                None
            } else {
                Some(0)
            });
        self.huella = self.huella_actual();
    }

    fn avisar(&mut self, texto: String) {
        self.aviso = Some((texto, std::time::Instant::now()));
    }

    /// **Cambia la carpeta en el disco** y relee. `cambio` recibe la carpeta
    /// ya podada (las mismas posiciones que se ven aqui).
    fn cambiar(&mut self, cambio: impl FnOnce(&mut Carpeta) -> bool) -> bool {
        let hecho = match carpetas::cambiar_en_disco(&self.dir, &self.que.id, cambio) {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(e) => {
                tracing::warn!(?e, "no se pudo guardar la carpeta");
                false
            }
        };
        self.recargar();
        super::refrescar();
        hecho
    }

    fn id_de(&self, k: usize) -> Option<String> {
        self.paginas.get(k).map(|p| p.mensaje.id.clone())
    }

    /// Lleva la pagina `desde` al sitio de `hasta`.
    fn mover(&mut self, desde: usize, hasta: usize) {
        let (Some(a), Some(b)) = (self.id_de(desde), self.id_de(hasta)) else {
            return;
        };
        let movida = self.cambiar(|c| match (c.posicion(&a), c.posicion(&b)) {
            (Some(i), Some(j)) => c.mover(i, j),
            _ => false,
        });
        if movida {
            self.elegida = self.paginas.iter().position(|p| p.mensaje.id == a);
        }
    }

    fn quitar(&mut self, k: usize) {
        if let Some(id) = self.id_de(k) {
            self.cambiar(|c| c.quitar(&id));
        }
    }

    fn renombrar(&mut self, k: usize, titulo: &str) {
        if let Some(id) = self.id_de(k) {
            let titulo = titulo.to_string();
            self.cambiar(|c| c.posicion(&id).is_some_and(|i| c.renombrar(i, &titulo)));
        }
    }

    /// **Abre una pagina**: un lienzo o una pagina del PDF en el lienzo, una
    /// foto en su lienzo; lo demas, en el chat, en su sitio.
    fn abrir_pagina(&mut self, k: usize) {
        let Some(m) = self.paginas.get(k).map(|p| p.mensaje.clone()) else {
            return;
        };
        let Some(j) = self.mensajes.iter().position(|x| x.id == m.id) else {
            return;
        };
        // Lo de una grabacion se estudia en su minuto, con el texto al lado.
        if self.grabacion
            && !self.paginas[k].adjunto
            && let Some((id, ms)) = super::estudiar_clase::de_una_clase(&m)
        {
            super::estudiar_clase::abrir(
                self.que.idioma,
                self.que.ubicacion.clone(),
                self.que.proyecto.clone(),
                id,
                ms,
            );
            return;
        }
        let (raiz, proyecto, opciones) = (
            self.raiz.clone(),
            self.que.proyecto.clone(),
            self.que.lienzo,
        );
        let es_hoja = matches!(m.clase, Some(Clase::Dibujo) | Some(Clase::Pagina))
            && m.referencia.as_deref().is_some_and(|r| !r.is_empty())
            || crate::pdf_en_chat::es_pagina_sin_dibujo(&m);
        if es_hoja {
            let mensajes = self.mensajes.clone();
            super::lienzo_aparte(super::clave_de_hoja(&proyecto, &mensajes, j), move || {
                super::abrir_hojas(&raiz, &proyecto, &mensajes, j, opciones);
            });
            return;
        }
        if m.clase == Some(Clase::Imagen) {
            let mut mensajes = self.mensajes.clone();
            crate::foto_anotada::asegurar_lienzo(&raiz, &proyecto, &mut mensajes, j);
            let plana = super::ruta_del_mensaje(&raiz, &proyecto, &m);
            super::lienzo_aparte(super::clave_de_hoja(&proyecto, &mensajes, j), move || {
                if !crate::foto_anotada::abrir_si_tiene_lienzo(
                    &raiz, &proyecto, &mensajes, j, opciones,
                ) && let Some(ruta) = plana
                {
                    super::abrir_foto_en_lienzo(&ruta, opciones);
                }
            });
            return;
        }
        super::ir_a(
            self.que.idioma,
            self.que.ubicacion.clone(),
            opciones,
            proyecto,
            Some(m.codigo_unico()),
        );
    }

    /// **«Nueva pagina»**: un lienzo nuevo en el chat con la plantilla de la
    /// ultima pagina, vacia, y el titulo siguiente; sin plantilla, en blanco.
    /// Se abre enseguida, que es lo que se quiere: llenarla.
    fn nueva_pagina(&mut self, textos: &Catalogo) {
        let hecha = pagina_siguiente(
            &self.raiz,
            &self.que.proyecto,
            &self.que.identidad,
            &self.que.id,
            &self.textos_pagina,
        );
        let id = match hecha {
            Ok(n) => n.mensaje.id,
            Err(e) => {
                tracing::warn!(?e, "no se pudo crear la pagina nueva");
                self.avisar(textos.t("chat-no-se-pudo"));
                return;
            }
        };
        self.recargar();
        super::refrescar();
        if let Some(k) = self.paginas.iter().position(|p| p.mensaje.id == id) {
            self.elegida = Some(k);
            // A la vista: la nueva es la ultima.
            self.scroll = self.alto_total.get().max(0) + 10_000;
            self.abrir_pagina(k);
        }
    }

    /// **«Estudiar»**: la clase de esta grabacion en su ventana.
    fn estudiar(&self) {
        if self.carpeta.voz.is_empty() {
            return;
        }
        super::estudiar_clase::abrir(
            self.que.idioma,
            self.que.ubicacion.clone(),
            self.que.proyecto.clone(),
            self.carpeta.voz.clone(),
            0,
        );
    }

    /// **«Adjuntar a la clase»**: los ficheros elegidos entran en el chat
    /// (como si se soltaran en el) y en la carpeta como adjuntos, aparte de
    /// lo grabado. No salen sueltos en el chat: estan en la carpeta.
    fn adjuntar(&mut self, ventana: &VentanaOverlay, textos: &Catalogo) {
        let elegidos = pixpin_shell::elegir::pedir_ficheros(ventana.handle());
        if elegidos.is_empty() {
            return;
        }
        let mut puestos = Vec::new();
        for ruta in &elegidos {
            match adjuntar_a_la_clase(
                &self.raiz,
                &self.que.proyecto,
                &self.que.identidad,
                &self.que.id,
                ruta,
            ) {
                Ok(nombre) => puestos.push(nombre),
                Err(e) => {
                    tracing::warn!(?e, ruta = %ruta.display(), "no se pudo adjuntar a la clase")
                }
            }
        }
        self.recargar();
        super::refrescar();
        if puestos.is_empty() {
            self.avisar(textos.t("chat-no-se-pudo"));
        } else {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("nombre", puestos.join(", "));
            self.avisar(textos.t_args("carpeta-adjuntado", &args));
        }
    }

    /// **«Exportar PDF»**: pregunta donde con el dialogo de guardar y lo
    /// escribe en otro hilo (un cuaderno de treinta paginas tarda).
    fn exportar(&mut self, ventana: &VentanaOverlay, textos: &Catalogo) {
        if self.exportando.is_some() {
            return;
        }
        let paginas: Vec<(Mensaje, String)> = self
            .paginas
            .iter()
            .map(|p| (p.mensaje.clone(), p.titulo.clone()))
            .collect();
        if paginas.is_empty() {
            self.avisar(textos.t("carpeta-no-se-exporto"));
            return;
        }
        let nombre = crate::compartir::carpeta::nombre_de_fichero(&self.nombre);
        let Some(destino) =
            pixpin_shell::guardar::pedir_ruta_para(ventana.handle(), &nombre, "PDF", "pdf")
        else {
            return;
        };
        self.avisar(textos.t("carpeta-exportando"));
        let resultado: Resultado = Arc::new(Mutex::new(None));
        self.exportando = Some(resultado.clone());
        let (raiz, proyecto, titulo, idioma) = (
            self.raiz.clone(),
            self.que.proyecto.clone(),
            self.nombre.clone(),
            self.que.idioma,
        );
        let hwnd = ventana.handle().0 as isize;
        let lanzado = std::thread::Builder::new()
            .name("carpeta-pdf".into())
            .spawn(move || {
                let _com = pixpin_shell::ComDelHilo::iniciar();
                let t = Catalogo::nuevo(idioma);
                let hecho = crate::compartir::carpeta::pdf(&raiz, &proyecto, &titulo, &paginas, &t)
                    .and_then(|bytes| {
                        std::fs::write(&destino, bytes)
                            .with_context(|| format!("no se pudo escribir {}", destino.display()))
                    });
                let aviso = match hecho {
                    Ok(()) => {
                        tracing::info!(ruta = %destino.display(), "carpeta exportada a PDF");
                        let mut args = fluent_bundle::FluentArgs::new();
                        args.set(
                            "nombre",
                            destino
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                        );
                        t.t_args("carpeta-exportado", &args)
                    }
                    Err(e) => {
                        tracing::warn!(?e, "no se pudo exportar la carpeta");
                        t.t("carpeta-no-se-exporto")
                    }
                };
                if let Ok(mut r) = resultado.lock() {
                    *r = Some(aviso);
                }
                pixpin_shell::overlay::despertar(hwnd);
            });
        if let Err(e) = lanzado {
            tracing::warn!(?e, "no se pudo lanzar el hilo del PDF");
            self.exportando = None;
        }
    }

    /// Lo que volvio del hilo del PDF, si ya volvio.
    fn recoger_exportado(&mut self) -> bool {
        let Some(r) = &self.exportando else {
            return false;
        };
        let listo = r.lock().ok().and_then(|mut g| g.take());
        match listo {
            Some(aviso) => {
                self.exportando = None;
                self.avisar(aviso);
                true
            }
            None => false,
        }
    }

    fn zona_en(&self, l: Punto) -> Option<Zona> {
        self.zonas
            .borrow()
            .iter()
            .rev()
            .find(|(r, _)| r.contiene(l))
            .map(|(_, z)| *z)
    }

    fn empezar_a_renombrar(&mut self, k: usize) {
        if let Some(p) = self.paginas.get(k) {
            self.renombrando = Some((k, p.titulo.clone()));
            self.elegida = Some(k);
        }
    }

    fn terminar_de_renombrar(&mut self, guardar: bool) {
        if let Some((k, escrito)) = self.renombrando.take()
            && guardar
            && !escrito.trim().is_empty()
        {
            self.renombrar(k, &escrito);
        }
    }

    fn mover_eleccion(&mut self, paso: i32) {
        if self.paginas.is_empty() {
            return;
        }
        let actual = self.elegida.unwrap_or(0) as i32;
        let nueva = (actual + paso).clamp(0, self.paginas.len() as i32 - 1);
        self.elegida = Some(nueva as usize);
    }
}

// --- La ventana ----------------------------------------------------------------

fn bucle(recursos: &Recursos, textos: &Catalogo, v: &mut Vista, clave: &str) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .monitor_en(pixpin_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien;
    let mut marco =
        crate::ventanita::centrado(monitor.area_trabajo, 1120, 820, monitor.escala_por_cien);
    let titulo = if v.nombre.is_empty() {
        textos.t("carpeta-nueva")
    } else {
        v.nombre.clone()
    };
    let mut ventana = VentanaOverlay::nueva_normal(marco, &titulo)
        .context("no se pudo abrir la ventana de la carpeta")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para la carpeta")?;
    ventana.mostrar();
    ventana.enfocar();
    if let Ok(mut a) = ABIERTAS.lock() {
        for (k, h) in a.iter_mut() {
            if k == clave {
                *h = ventana.handle().0 as isize;
            }
        }
    }
    tracing::info!(paginas = v.paginas.len(), "ventana de la carpeta abierta");

    let mut previas = crate::miniaturas::Miniaturas::con_lado(2 * CELDA_DIBUJO);
    let mut moviendo: Option<(Punto, Rect)> = None;
    let mut vivo = true;
    let mut pintar = true;
    let mut mirado = std::time::Instant::now();
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        for (hw, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hw != ventana.handle() {
                continue;
            }
            let local = |p: Punto, m: Rect| Punto {
                x: p.x - m.x,
                y: p.y - m.y,
            };
            let pintar_antes = pintar;
            // Casi todo cambia lo que se ve; mover el raton, solo si pasa a
            // otra cosa (pintar las miniaturas en cada pixel seria tirarlo).
            pintar = true;
            match ev {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    if let Some((desde, origen)) = moviendo {
                        marco = Rect {
                            x: origen.x + (p.x - desde.x),
                            y: origen.y + (p.y - desde.y),
                            ..origen
                        };
                        ventana.mover(marco);
                    }
                    let l = local(p, marco);
                    if let Some((_, desde)) = v.pulsada
                        && !v.grabacion
                        && ((l.x - desde.x).abs() > ARRASTRE_MINIMO
                            || (l.y - desde.y).abs() > ARRASTRE_MINIMO)
                    {
                        v.arrastrando = true;
                    }
                    let otra_cosa =
                        moviendo.is_some() || v.arrastrando || v.zona_en(l) != v.zona_en(v.raton);
                    v.raton = l;
                    if !otra_cosa {
                        pintar = pintar_antes;
                    }
                }
                EventoOverlay::BotonPulsado(p) => {
                    let l = local(p, marco);
                    v.raton = l;
                    let zona = v.zona_en(l);
                    // Pulsar fuera del titulo que se escribe lo guarda.
                    let en_la_misma = matches!(
                        (zona, &v.renombrando),
                        (Some(Zona::Pagina(k)), Some((r, _))) if k == *r
                    );
                    if !en_la_misma {
                        v.terminar_de_renombrar(true);
                    }
                    match zona {
                        Some(Zona::Barra) => {
                            moviendo = Some((p, marco));
                            ventana.capturar_raton();
                        }
                        Some(Zona::Cerrar) => vivo = false,
                        Some(Zona::NuevaPagina) => v.nueva_pagina(textos),
                        Some(Zona::Exportar) => v.exportar(&ventana, textos),
                        Some(Zona::Estudiar) => v.estudiar(),
                        Some(Zona::Adjuntar) => v.adjuntar(&ventana, textos),
                        Some(Zona::Pagina(k)) => {
                            v.elegida = Some(k);
                            v.pulsada = Some((k, l));
                            v.arrastrando = false;
                            ventana.capturar_raton();
                        }
                        Some(Zona::Antes(k)) if k > 0 => v.mover(k, k - 1),
                        Some(Zona::Despues(k)) => v.mover(k, k + 1),
                        Some(Zona::Renombrar(k)) => v.empezar_a_renombrar(k),
                        Some(Zona::Quitar(k)) => v.quitar(k),
                        _ => {}
                    }
                }
                EventoOverlay::BotonSoltado(p) => {
                    if moviendo.take().is_some() {
                        ventana.soltar_raton();
                    }
                    if let Some((k, _)) = v.pulsada.take() {
                        ventana.soltar_raton();
                        let l = local(p, marco);
                        let destino = match v.zona_en(l) {
                            Some(
                                Zona::Pagina(j)
                                | Zona::Antes(j)
                                | Zona::Despues(j)
                                | Zona::Renombrar(j)
                                | Zona::Quitar(j),
                            ) => Some(j),
                            _ => None,
                        };
                        if v.arrastrando {
                            if let Some(j) = destino.filter(|j| *j != k) {
                                v.mover(k, j);
                            }
                        } else if destino == Some(k) && v.renombrando.is_none() {
                            v.abrir_pagina(k);
                        }
                        v.arrastrando = false;
                    }
                }
                EventoOverlay::Rueda(delta) => {
                    let paso = (CELDA_DIBUJO * escala / 100) as i32;
                    v.scroll =
                        (v.scroll - delta * paso / 120 / 2).clamp(0, v.alto_total.get().max(0));
                }
                EventoOverlay::Tecla { vk, ctrl, .. } => {
                    if let Some((_, escrito)) = v.renombrando.as_mut() {
                        match vk {
                            VK_ENTRAR => v.terminar_de_renombrar(true),
                            VK_ESCAPE => v.terminar_de_renombrar(false),
                            VK_RETROCESO => {
                                escrito.pop();
                            }
                            _ => {}
                        }
                        continue;
                    }
                    let elegida = v.elegida;
                    match (vk, elegida) {
                        (VK_ESCAPE, _) => vivo = false,
                        (VK_IZQUIERDA | VK_ARRIBA, Some(k)) if ctrl && k > 0 && !v.grabacion => {
                            v.mover(k, k - 1)
                        }
                        (VK_DERECHA | VK_ABAJO, Some(k)) if ctrl && !v.grabacion => {
                            v.mover(k, k + 1)
                        }
                        (VK_IZQUIERDA, _) => v.mover_eleccion(-1),
                        (VK_DERECHA, _) => v.mover_eleccion(1),
                        (VK_ARRIBA, _) => v.mover_eleccion(-(v.columnas.get() as i32)),
                        (VK_ABAJO, _) => v.mover_eleccion(v.columnas.get() as i32),
                        (VK_ENTRAR, Some(k)) => v.abrir_pagina(k),
                        (VK_F2, Some(k)) => v.empezar_a_renombrar(k),
                        (VK_SUPR, Some(k)) => v.quitar(k),
                        _ => {}
                    }
                }
                EventoOverlay::Caracter(c) if !c.is_control() => {
                    if let Some((_, escrito)) = v.renombrando.as_mut()
                        && escrito.chars().count() < carpetas::TOPE_TITULO
                    {
                        escrito.push(c);
                    }
                }
                // Pintar, despertar, cambiar de pantalla: se repinta y ya.
                _ => {}
            }
        }
        if !vivo {
            break;
        }
        if v.recoger_exportado() {
            pintar = true;
        }
        // Lo guardado cambio por fuera (un lienzo cerrado, una pagina anadida
        // desde el chat): se relee. No mientras se escribe un titulo.
        if mirado.elapsed() >= MIRAR_CADA && v.renombrando.is_none() && v.pulsada.is_none() {
            mirado = std::time::Instant::now();
            if v.huella_actual() != v.huella {
                v.recargar();
                pintar = true;
            }
        }
        if let Some((_, desde)) = &v.aviso
            && desde.elapsed() >= AVISO
        {
            v.aviso = None;
            pintar = true;
        }
        if pintar {
            pintar = false;
            let tema = if pixpin_shell::entorno::tema_claro() {
                &CLARO
            } else {
                &OSCURO
            };
            let fondos: Vec<PathBuf> = v
                .paginas
                .iter()
                .filter_map(|p| p.dibujo.as_ref()?.fondo.as_ref().map(|(r, _)| r.clone()))
                .collect();
            if previas.asegurar(&fondos, &motor) {
                // Quedan miniaturas por leer: otra vuelta enseguida.
                pintar = true;
            }
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| {
                    pintar_todo(
                        p,
                        v,
                        marco.ancho,
                        marco.alto,
                        tema,
                        escala,
                        textos,
                        &previas,
                    )
                });
                let _ = superficie.presentar();
            }
        }
        let espera = if pintar || v.exportando.is_some() || v.aviso.is_some() {
            100
        } else {
            1_000
        };
        pixpin_shell::overlay::esperar_eventos(Some(espera));
    }
    ventana.ocultar();
    tracing::info!("ventana de la carpeta cerrada");
    Ok(())
}

// --- Pintar ----------------------------------------------------------------------

/// Un boton de la barra: su caja, con el rotulo y su icono.
#[allow(clippy::too_many_arguments)]
fn boton(
    p: &Pintor,
    caja: Rect,
    icono: &Icono,
    rotulo: &str,
    fuerte: bool,
    encima: bool,
    tema: &Tema,
    e: f32,
) {
    let fondo = if fuerte { tema.enviar } else { tema.buscador };
    p.rellenar_redondeado(
        rf(caja),
        caja.alto as f32 / 2.0,
        if encima { con_alfa(fondo, 0.85) } else { fondo },
    );
    let tinta = if fuerte { Color::BLANCO } else { tema.texto };
    p.icono(
        icono,
        RectF {
            x: caja.x as f32 + 12.0 * e,
            y: caja.y as f32 + (caja.alto as f32 - 18.0 * e) / 2.0,
            ancho: 18.0 * e,
            alto: 18.0 * e,
        },
        tinta,
    );
    let (_, th) = p.medir_texto(rotulo, 13.5 * e);
    p.texto(
        rotulo,
        caja.x as f32 + 38.0 * e,
        caja.y as f32 + (caja.alto as f32 - th) / 2.0,
        13.5 * e,
        tinta,
    );
}

#[allow(clippy::too_many_arguments)]
fn pintar_todo(
    p: &Pintor,
    v: &Vista,
    ancho: u32,
    alto: u32,
    tema: &Tema,
    escala: u32,
    textos: &Catalogo,
    previas: &crate::miniaturas::Miniaturas,
) {
    let e = escala as f32 / 100.0;
    let ei = |x: u32| x * escala / 100;
    let mut zonas = v.zonas.borrow_mut();
    zonas.clear();
    let todo = Rect {
        x: 0,
        y: 0,
        ancho,
        alto,
    };
    p.rellenar(rf(todo), tema.chat);

    // La barra: el nombre y cuantas lleva, los dos botones y cerrar.
    let barra = Rect {
        alto: ei(BARRA),
        ..todo
    };
    p.rellenar(rf(barra), tema.barra);
    zonas.push((barra, Zona::Barra));
    p.icono(
        if v.grabacion {
            &mi::MIC
        } else {
            &mi::MENU_BOOK
        },
        RectF {
            x: ei(MARGEN) as f32,
            y: (barra.alto as f32 - 24.0 * e) / 2.0,
            ancho: 24.0 * e,
            alto: 24.0 * e,
        },
        tema.texto,
    );
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("n", v.paginas.len() as i64);
    let cuenta = if v.grabacion {
        detalle_de_grabacion(&v.carpeta, textos)
    } else {
        textos.t_args("carpeta-paginas", &args)
    };
    // Los botones de la barra: en una grabacion, estudiar (si hay nota),
    // adjuntar y exportar; en un cuaderno, una pagina nueva y exportar.
    let mut botones: Vec<(&Icono, String, bool, Zona)> = Vec::new();
    if v.grabacion {
        if !v.carpeta.voz.is_empty() {
            botones.push((
                &mi::MENU_BOOK,
                textos.t("carpeta-estudiar"),
                true,
                Zona::Estudiar,
            ));
        }
        botones.push((
            &mi::ATTACH_FILE,
            textos.t("carpeta-adjuntar"),
            false,
            Zona::Adjuntar,
        ));
    } else {
        botones.push((
            &mi::ADD,
            textos.t("carpeta-nueva-pagina"),
            true,
            Zona::NuevaPagina,
        ));
    }
    let rotulo_exportar = if v.exportando.is_some() {
        textos.t("carpeta-exportando")
    } else {
        textos.t("carpeta-exportar-pdf")
    };
    botones.push((&mi::IOS_SHARE, rotulo_exportar, false, Zona::Exportar));
    let n_botones = botones.len() as u32;
    let titulo = if v.perdida {
        textos.t("carpeta-no-esta")
    } else {
        v.nombre.clone()
    };
    let (_, th) = p.medir_texto(&titulo, 17.0 * e);
    let x_titulo = ei(MARGEN) as f32 + 34.0 * e;
    let botones_x = ancho as i32 - ei(BARRA) as i32 - ei(n_botones * (160 + 10)) as i32;
    p.texto_linea(
        &titulo,
        x_titulo,
        (barra.alto as f32 - th) / 2.0 - 7.0 * e,
        17.0 * e,
        (botones_x as f32 - x_titulo - 12.0 * e).max(0.0),
        tema.texto,
    );
    p.texto(
        &cuenta,
        x_titulo,
        (barra.alto as f32 - th) / 2.0 + 13.0 * e,
        12.0 * e,
        tema.apagado,
    );
    if !v.perdida {
        let alto_boton = ei(BOTON_ALTO);
        let y_boton = ((barra.alto - alto_boton) / 2) as i32;
        let mut caja = Rect {
            x: botones_x,
            y: y_boton,
            ancho: ei(160),
            alto: alto_boton,
        };
        for (icono, rotulo, fuerte, zona) in &botones {
            boton(
                p,
                caja,
                icono,
                rotulo,
                *fuerte,
                caja.contiene(v.raton),
                tema,
                e,
            );
            zonas.push((caja, *zona));
            caja.x = caja.derecha() + ei(10) as i32;
        }
    }
    let cerrar = Rect {
        x: ancho as i32 - ei(BARRA) as i32,
        y: 0,
        ancho: ei(BARRA),
        alto: ei(BARRA),
    };
    if cerrar.contiene(v.raton) {
        p.rellenar(rf(cerrar), tema.cerrar_sobre);
    }
    p.icono(
        &mi::CLOSE,
        RectF {
            x: cerrar.x as f32 + (cerrar.ancho as f32 - 20.0 * e) / 2.0,
            y: (cerrar.alto as f32 - 20.0 * e) / 2.0,
            ancho: 20.0 * e,
            alto: 20.0 * e,
        },
        tema.texto,
    );
    zonas.push((cerrar, Zona::Cerrar));

    // La pista, o lo que se esta escribiendo, o el aviso.
    let pista = Rect {
        x: 0,
        y: barra.abajo(),
        ancho,
        alto: ei(PISTA),
    };
    let (linea, color) = match (&v.aviso, &v.renombrando) {
        (Some((t, _)), _) => (t.clone(), tema.texto),
        (None, Some(_)) => (textos.t("carpeta-escribe-titulo"), tema.apagado),
        (None, None) if v.grabacion => (textos.t("carpeta-grabacion-pista"), tema.apagado),
        (None, None) => (textos.t("carpeta-pista"), tema.apagado),
    };
    let (_, lh) = p.medir_texto(&linea, 12.5 * e);
    p.texto_linea(
        &linea,
        ei(MARGEN) as f32,
        pista.y as f32 + (pista.alto as f32 - lh) / 2.0,
        12.5 * e,
        ancho as f32 - 2.0 * ei(MARGEN) as f32,
        color,
    );

    // La rejilla de las paginas, en orden.
    let rejilla = Rect {
        x: 0,
        y: pista.abajo(),
        ancho,
        alto: alto.saturating_sub(barra.alto + pista.alto),
    };
    if v.paginas.is_empty() {
        let vacio = if v.perdida {
            textos.t("carpeta-no-esta")
        } else {
            textos.t("carpeta-vacia")
        };
        let ancho_texto = (rejilla.ancho as f32 - 120.0 * e).min(560.0 * e);
        let (w, _) = p.medir_texto_ajustado(&vacio, 15.0 * e, ancho_texto);
        p.texto_ajustado(
            &vacio,
            (rejilla.ancho as f32 - w) / 2.0,
            rejilla.y as f32 + 90.0 * e,
            15.0 * e,
            ancho_texto,
            tema.apagado,
        );
        v.alto_total.set(0);
        return;
    }
    let margen = ei(MARGEN);
    let paso_x = ei(CELDA_ANCHO + HUECO);
    let columnas = ((rejilla.ancho.saturating_sub(2 * margen) + ei(HUECO)) / paso_x.max(1)).max(1);
    v.columnas.set(columnas as usize);
    let alto_celda = ei(CELDA_DIBUJO + CELDA_TITULO + CELDA_BOTONES);
    let paso_y = alto_celda + ei(HUECO);
    // Centrada: con pocas paginas no se quedan pegadas a la izquierda.
    let usado = columnas * paso_x - ei(HUECO);
    let x0 = rejilla.x + ((rejilla.ancho.saturating_sub(usado)) / 2).max(margen) as i32;
    let filas = (v.paginas.len() as u32).div_ceil(columnas);
    let total = filas * paso_y + 2 * margen;
    v.alto_total.set(total as i32 - rejilla.alto as i32);
    let scroll = v.scroll.clamp(0, v.alto_total.get().max(0));
    // A donde caeria lo que se arrastra.
    let arrastrada = v.pulsada.filter(|_| v.arrastrando).map(|(k, _)| k);
    p.empujar_recorte(rf(rejilla));
    for (k, pg) in v.paginas.iter().enumerate() {
        let (col, fila) = (k as u32 % columnas, k as u32 / columnas);
        let celda = Rect {
            x: x0 + (col * paso_x) as i32,
            y: rejilla.y + margen as i32 + (fila * paso_y) as i32 - scroll,
            ancho: ei(CELDA_ANCHO),
            alto: alto_celda,
        };
        if celda.abajo() < rejilla.y || celda.y > rejilla.abajo() {
            continue;
        }
        let renombrando = v
            .renombrando
            .as_ref()
            .filter(|(r, _)| *r == k)
            .map(|(_, t)| t);
        let destino = arrastrada.is_some_and(|a| a != k) && celda.contiene(v.raton);
        pintar_pagina(
            p,
            pg,
            k,
            celda,
            Celda {
                elegida: v.elegida == Some(k),
                encima: celda.contiene(v.raton),
                arrastrada: arrastrada == Some(k),
                destino,
                renombrando,
                ultima: k + 1 == v.paginas.len(),
                raton: v.raton,
                sin_mover: v.grabacion,
                insignia: pg.adjunto.then(|| textos.t("carpeta-adjunto-insignia")),
            },
            tema,
            e,
            previas,
            &mut zonas,
        );
    }
    p.soltar_recorte();
}

/// Como esta una pagina al pintarla.
struct Celda<'a> {
    elegida: bool,
    encima: bool,
    arrastrada: bool,
    destino: bool,
    renombrando: Option<&'a String>,
    ultima: bool,
    raton: Punto,
    /// En una grabacion no se cambia el orden: es el del tiempo.
    sin_mover: bool,
    /// Lo adjuntado a una clase: la chapa que lo dice.
    insignia: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn pintar_pagina(
    p: &Pintor,
    pg: &PaginaVista,
    k: usize,
    celda: Rect,
    c: Celda<'_>,
    tema: &Tema,
    e: f32,
    previas: &crate::miniaturas::Miniaturas,
    zonas: &mut Vec<(Rect, Zona)>,
) {
    let dibujo = RectF {
        x: celda.x as f32,
        y: celda.y as f32,
        ancho: celda.ancho as f32,
        alto: CELDA_DIBUJO as f32 * e,
    };
    if c.elegida || c.encima || c.destino {
        p.rellenar_redondeado(
            RectF {
                x: dibujo.x - 6.0 * e,
                y: dibujo.y - 6.0 * e,
                ancho: dibujo.ancho + 12.0 * e,
                alto: celda.alto as f32 + 8.0 * e,
            },
            12.0 * e,
            if c.elegida || c.destino {
                tema.fila_elegida
            } else {
                tema.fila_sobre
            },
        );
    }
    let papel = pg
        .dibujo
        .as_ref()
        .map_or(tema.papel, |d| super::papel_de_vista(d, tema.papel));
    p.rellenar_redondeado(
        dibujo,
        8.0 * e,
        if c.arrastrada {
            con_alfa(papel, 0.45)
        } else {
            papel
        },
    );
    // Lo adjuntado a una clase, con otro color: no es de lo grabado.
    if c.insignia.is_some() {
        p.rellenar_redondeado(dibujo, 8.0 * e, con_alfa(COLOR_ADJUNTO, 0.18));
        p.trazar(dibujo, 2.0 * e, COLOR_ADJUNTO);
    }
    match &pg.dibujo {
        Some(d) if !c.arrastrada => pintar_lienzo(
            p,
            d,
            RectF {
                x: dibujo.x + 8.0 * e,
                y: dibujo.y + 8.0 * e,
                ancho: dibujo.ancho - 16.0 * e,
                alto: dibujo.alto - 16.0 * e,
            },
            Some(previas),
        ),
        Some(_) => {}
        None => p.icono(
            pg.icono,
            RectF {
                x: dibujo.x + (dibujo.ancho - 48.0 * e) / 2.0,
                y: dibujo.y + (dibujo.alto - 48.0 * e) / 2.0,
                ancho: 48.0 * e,
                alto: 48.0 * e,
            },
            tema.texto_papel,
        ),
    }
    // En su esquina: el numero de la pagina, o su minuto en la grabacion.
    let numero = &pg.esquina;
    if !numero.is_empty() {
        let (nw, nh) = p.medir_texto(numero, 12.0 * e);
        let chapa = RectF {
            x: dibujo.x + 8.0 * e,
            y: dibujo.y + 8.0 * e,
            ancho: (nw + 14.0 * e).max(24.0 * e),
            alto: nh + 6.0 * e,
        };
        p.rellenar_redondeado(chapa, chapa.alto / 2.0, con_alfa(Color::NEGRO, 0.6));
        p.texto(
            numero,
            chapa.x + (chapa.ancho - nw) / 2.0,
            chapa.y + 3.0 * e,
            12.0 * e,
            Color::BLANCO,
        );
    }
    // «Adjunto · no es de la grabacion», abajo, sobre su color.
    if let Some(insignia) = &c.insignia {
        let (iw, ih) = p.medir_texto(insignia, 11.5 * e);
        let ancho_max = dibujo.ancho - 16.0 * e;
        let chapa = RectF {
            x: dibujo.x + 8.0 * e,
            y: dibujo.y + dibujo.alto - ih - 14.0 * e,
            ancho: (iw + 14.0 * e).min(ancho_max),
            alto: ih + 6.0 * e,
        };
        p.rellenar_redondeado(chapa, chapa.alto / 2.0, COLOR_ADJUNTO);
        p.texto_linea(
            insignia,
            chapa.x + 7.0 * e,
            chapa.y + 3.0 * e,
            11.5 * e,
            chapa.ancho - 14.0 * e,
            Color::BLANCO,
        );
    }
    zonas.push((
        Rect {
            x: celda.x,
            y: celda.y,
            ancho: celda.ancho,
            alto: (CELDA_DIBUJO as f32 * e) as u32,
        },
        Zona::Pagina(k),
    ));

    // El titulo, o lo que se esta escribiendo con su cursor.
    let y_titulo = dibujo.y + dibujo.alto + 6.0 * e;
    let tinta = if c.elegida {
        tema.texto_elegido
    } else {
        tema.texto
    };
    match c.renombrando {
        Some(escrito) => {
            let caja = RectF {
                x: dibujo.x,
                y: y_titulo - 3.0 * e,
                ancho: dibujo.ancho,
                alto: 24.0 * e,
            };
            p.rellenar_redondeado(caja, 6.0 * e, tema.campo);
            p.texto_linea(
                &format!("{escrito}▏"),
                caja.x + 6.0 * e,
                y_titulo,
                13.5 * e,
                caja.ancho - 12.0 * e,
                tema.texto,
            );
        }
        None => p.texto_linea(
            &pg.titulo,
            dibujo.x + 2.0 * e,
            y_titulo,
            13.5 * e,
            dibujo.ancho - 4.0 * e,
            tinta,
        ),
    }

    // Los botones: antes, despues, renombrar y quitar.
    let lado = (26.0 * e) as u32;
    let y_botones = celda.y + ((CELDA_DIBUJO + CELDA_TITULO) as f32 * e) as i32;
    let botones: [(&Icono, Zona, bool); 4] = [
        (&mi::CHEVRON_LEFT, Zona::Antes(k), k > 0 && !c.sin_mover),
        (
            &mi::CHEVRON_RIGHT,
            Zona::Despues(k),
            !c.ultima && !c.sin_mover,
        ),
        (&mi::EDIT, Zona::Renombrar(k), true),
        (&mi::CLOSE, Zona::Quitar(k), true),
    ];
    for (n, (icono, zona, activo)) in botones.into_iter().enumerate() {
        let caja = Rect {
            x: celda.x + (n as u32 * (lado + (6.0 * e) as u32)) as i32,
            y: y_botones,
            ancho: lado,
            alto: lado,
        };
        let sobre = caja.contiene(c.raton);
        if sobre && activo {
            p.rellenar_redondeado(rf(caja), lado as f32 / 2.0, tema.boton_sobre);
        }
        let color = if !activo {
            con_alfa(tema.apagado, 0.35)
        } else if c.encima || c.elegida {
            tinta
        } else {
            tema.apagado
        };
        p.icono(
            icono,
            RectF {
                x: caja.x as f32 + 4.0 * e,
                y: caja.y as f32 + 4.0 * e,
                ancho: caja.ancho as f32 - 8.0 * e,
                alto: caja.alto as f32 - 8.0 * e,
            },
            color,
        );
        if activo {
            zonas.push((caja, zona));
        }
    }
}

/// **Adjunta un fichero a una clase grabada**: entra en el chat como si se
/// soltara (`adjuntar_con_pie`) y en la carpeta `carpeta` como adjunto, con
/// su nombre. Devuelve ese nombre.
fn adjuntar_a_la_clase(
    raiz: &std::path::Path,
    proyecto: &str,
    aparato: &str,
    carpeta: &str,
    ruta: &std::path::Path,
) -> Result<String> {
    let nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .context("sin nombre")?;
    let bytes =
        std::fs::read(ruta).with_context(|| format!("no se pudo leer {}", ruta.display()))?;
    let numero = super::siguiente_numero_en(raiz, proyecto)?;
    let m = super::adjuntar_con_pie(raiz, proyecto, aparato, &nombre, &bytes, numero, "", None)?;
    let dir = pixpin_proyecto::almacen::carpeta(raiz, proyecto);
    let titulo = pixpin_docs::sin_extension(&nombre);
    carpetas::cambiar_en_disco(&dir, carpeta, |c| c.adjuntar(&m.id, &titulo))?
        .context("la carpeta ya no esta")?;
    Ok(nombre)
}

/// Las paginas que se ensenan en la tarjeta de la carpeta del chat: los
/// `id` de las primeras, en orden. De una grabacion, sus capturas (la
/// primera es la foto de su tarjeta).
pub(super) fn ids_de_la_tarjeta(m: &Mensaje) -> Option<Vec<String>> {
    let c = Carpeta::de_mensaje(m)?;
    if c.es_grabacion() {
        return Some(c.capturas().map(|p| p.id.clone()).collect());
    }
    Some(c.paginas.iter().map(|p| p.id.clone()).collect())
}

// --- Lo de dentro no sale suelto en el chat ----------------------------------------

/// La marca de lo que esta dentro de una carpeta (ver
/// [`marcar_lo_de_dentro`]). Con guion bajo, como `_soloGaleria`: no es del
/// formato, se rehace cada vez que se lee el chat.
pub(crate) const DENTRO: &str = "_enCarpeta";

/// **Marca lo que esta dentro de alguna carpeta** (de paginas o de una
/// grabacion), que el chat no ensena suelto: lo representa la tarjeta de
/// su carpeta (el usuario, 25-oct-2026: «las paginas de un cuaderno tienen
/// que estar solo dentro de la carpeta»). Se quita de lo que ya no esta en
/// ninguna, que asi vuelve a verse.
pub(crate) fn marcar_lo_de_dentro(mensajes: &mut [Mensaje]) {
    let dentro = carpetas::dentro_de_carpetas(mensajes);
    for m in mensajes.iter_mut() {
        if dentro.contains(&m.id) {
            m.resto.insert(DENTRO.into(), serde_json::Value::Bool(true));
        } else {
            m.resto.remove(DENTRO);
        }
    }
}

/// Si el mensaje esta dentro de una carpeta (ver [`marcar_lo_de_dentro`]).
pub(crate) fn esta_dentro(m: &Mensaje) -> bool {
    m.resto.get(DENTRO).and_then(|v| v.as_bool()) == Some(true)
}

/// **La clase de la carpeta de una grabacion**: `(id de su nota, 0)`, lo
/// que pide «Estudiar la clase». `None` si no es una grabacion o no tiene
/// nota (solo entraron el video y las capturas).
pub(super) fn clase_de_la_grabacion(m: &Mensaje) -> Option<(String, i64)> {
    grabacion_de(m)
        .filter(|c| !c.voz.is_empty())
        .map(|c| (c.voz, 0))
}

/// La carpeta de un mensaje si es la de una grabacion.
pub(super) fn grabacion_de(m: &Mensaje) -> Option<Carpeta> {
    Carpeta::de_mensaje(m).filter(Carpeta::es_grabacion)
}

/// **Lo que lleva la carpeta de una grabacion**, para quien lo quiera
/// ensenar o compartir: lo grabado (la nota, el video y las capturas, en el
/// orden de la grabacion) y, aparte, lo adjuntado a la clase. Cada uno con
/// su sitio en `mensajes`; lo que ya no esta en el chat se salta.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PiezasDeGrabacion {
    pub voz: Option<usize>,
    pub video: Option<usize>,
    /// `(minuto en ms, posicion)`, por su minuto.
    pub capturas: Vec<(i64, usize)>,
    pub adjuntos: Vec<usize>,
    pub duracion_ms: i64,
}

/// [`PiezasDeGrabacion`] de la carpeta `carpeta` (su mensaje). `None` si no
/// es la de una grabacion.
// Es la puerta para compartir una clase entera (otro trabajo la usa).
#[allow(dead_code)]
pub(crate) fn piezas_de_grabacion(
    carpeta: &Mensaje,
    mensajes: &[Mensaje],
) -> Option<PiezasDeGrabacion> {
    let c = grabacion_de(carpeta)?;
    let donde = |id: &str| {
        (!id.is_empty())
            .then(|| mensajes.iter().position(|m| m.id == id))
            .flatten()
    };
    let mut capturas: Vec<(i64, usize)> = c
        .capturas()
        .filter_map(|p| Some((p.ms.unwrap_or(0), donde(&p.id)?)))
        .collect();
    capturas.sort_by_key(|(ms, _)| *ms);
    Some(PiezasDeGrabacion {
        voz: donde(&c.voz),
        video: donde(&c.video),
        capturas,
        adjuntos: c.adjuntos().filter_map(|p| donde(&p.id)).collect(),
        duracion_ms: c.duracion_ms,
    })
}

/// **El detalle de la fila de una grabacion**: «1:30:00 · 5 capturas».
pub(super) fn detalle_de_grabacion(c: &Carpeta, textos: &Catalogo) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("duracion", super::grabar_clase::minuto(c.duracion_ms));
    args.set("capturas", c.capturas().count() as i64);
    textos.t_args("carpeta-grabacion-detalle", &args)
}

/// Cuantas paginas lleva, para el detalle de su fila en el chat.
pub(super) fn cuantas(m: &Mensaje) -> usize {
    Carpeta::de_mensaje(m).map_or(0, |c| c.len())
}

/// Las carpetas de la conversacion `(indice, nombre)` para el menu
/// «Añadir a carpeta…».
pub(super) fn del_chat(mensajes: &[Mensaje], textos: &Catalogo) -> Vec<(usize, String)> {
    // Las de las grabaciones tambien: lo que entra en una es un adjunto a
    // la clase, que se ve aparte de lo grabado (ver `anadir`).
    carpetas::carpetas_de(mensajes)
        .into_iter()
        .map(|i| (i, super::nombre_de_la_fila(&mensajes[i], textos)))
        .collect()
}

/// Los nombres que ya estan en uso, para no repetir «Cuaderno» dos veces.
pub(super) fn nombre_libre(base: &str, mensajes: &[Mensaje]) -> String {
    let usados: Vec<&str> = carpetas::carpetas_de(mensajes)
        .into_iter()
        .map(|i| mensajes[i].nombre.trim())
        .collect();
    if !usados.contains(&base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|n| !usados.contains(&n.as_str()))
        .unwrap_or_else(|| base.to_string())
}

/// Lo que deja [`pagina_siguiente`].
pub(crate) struct PaginaNueva {
    /// El mensaje del lienzo nuevo, ya en el chat y en la carpeta.
    pub mensaje: Mensaje,
    pub titulo: String,
    /// Si lleva la plantilla de la ultima pagina (si no, en blanco).
    pub con_plantilla: bool,
}

/// **La pagina siguiente de una carpeta**, sin ventana (la usan la ventana
/// y el asistente): un lienzo nuevo en el chat con la plantilla de la ultima
/// pagina que sea un lienzo, vacia (`hoja_de_plantilla::lienzo_siguiente`),
/// o en blanco si no tiene; con el titulo siguiente («Dia 3» tras «Dia 2»;
/// sin serie, `por_defecto` y su puesto), y metido al final de la carpeta.
pub(crate) fn pagina_siguiente(
    raiz: &std::path::Path,
    proyecto: &str,
    aparato: &str,
    carpeta: &str,
    por_defecto: &str,
) -> std::io::Result<PaginaNueva> {
    let dir = pixpin_proyecto::almacen::carpeta(raiz, proyecto);
    let mensajes = cuaderno::Cuaderno::leer_de(&dir)?.mensajes;
    let mut c = mensajes
        .iter()
        .find(|m| m.id == carpeta)
        .and_then(Carpeta::de_mensaje)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no hay esa carpeta"))?;
    c.podar(|id| mensajes.iter().any(|m| m.id == id));
    let plantilla = c
        .vivas(&mensajes)
        .into_iter()
        .rev()
        .map(|(i, _)| &mensajes[i])
        .find(|m| m.clase == Some(Clase::Dibujo))
        .and_then(|m| m.referencia.clone())
        .and_then(|r| {
            let texto =
                std::fs::read_to_string(pixpin_proyecto::almacen::lienzo(raiz, proyecto, &r))
                    .ok()?;
            let lienzo = pixpin_motor2d::excalidraw::leer(&texto).ok()?;
            pixpin_motor2d::hoja_de_plantilla::lienzo_siguiente(
                &pixpin_motor2d::excalidraw::a_escena(&lienzo),
            )
        });
    let titulo = c.titulo_siguiente(por_defecto);
    let numero = super::siguiente_numero_en(raiz, proyecto)?;
    let m = super::escribir_lienzo(raiz, proyecto, aparato, numero, Some(&titulo))?;
    if let (Some(escena), Some(r)) = (&plantilla, m.referencia.as_deref()) {
        let lienzo = pixpin_motor2d::excalidraw::con_escena(
            &pixpin_motor2d::excalidraw::Lienzo::vacio(),
            escena,
        );
        std::fs::write(
            pixpin_proyecto::almacen::lienzo(raiz, proyecto, r),
            pixpin_motor2d::excalidraw::escribir(&lienzo),
        )?;
    }
    let _ = super::subir_en_la_lista(raiz, proyecto, m.cuando, &titulo);
    carpetas::cambiar_en_disco(&dir, carpeta, |c| c.anadir(&m.id, &titulo))?;
    tracing::info!(con_plantilla = plantilla.is_some(), titulo = %titulo, "pagina nueva en la carpeta");
    Ok(PaginaNueva {
        mensaje: m,
        titulo,
        con_plantilla: plantilla.is_some(),
    })
}

// --- Lo del asistente: las carpetas por su nombre ---------------------------------

/// **La carpeta del chat que se llama `nombre`**: igual sin tildes ni
/// mayusculas, o si no la que lo lleva dentro. La mas reciente si hay dos.
pub(crate) fn carpeta_por_nombre(mensajes: &[Mensaje], nombre: &str) -> Option<usize> {
    let buscado = super::figuras::plegar(nombre.trim());
    if buscado.is_empty() {
        return None;
    }
    let todas: Vec<usize> = carpetas::carpetas_de(mensajes)
        .into_iter()
        .filter(|&i| !carpetas::es_grabacion(&mensajes[i]))
        .collect();
    let nombre_de = |i: &usize| super::figuras::plegar(mensajes[*i].nombre.trim());
    todas
        .iter()
        .rev()
        .find(|i| nombre_de(i) == buscado)
        .or_else(|| todas.iter().rev().find(|i| nombre_de(i).contains(&buscado)))
        .copied()
}

/// Lo que deja [`anadir_por_nombre`].
pub(crate) struct Anadida {
    pub carpeta: String,
    pub titulo: String,
    pub paginas: usize,
    /// Si la carpeta no existia y se creo.
    pub creada: bool,
    /// Si la pagina ya estaba (entonces no se toco nada).
    pub ya_estaba: bool,
}

/// **Para el asistente**: mete el mensaje `pagina` (su `id`) en la carpeta
/// `nombre` de este chat, al final y con el titulo que le toca; si no hay
/// ninguna carpeta con ese nombre, la crea. Sin ventana: escribe el cuaderno
/// y avisa al chat.
pub(crate) fn anadir_por_nombre(
    raiz: &std::path::Path,
    proyecto: &str,
    aparato: &str,
    nombre: &str,
    pagina: &str,
    por_defecto: &str,
) -> std::io::Result<Anadida> {
    let dir = pixpin_proyecto::almacen::carpeta(raiz, proyecto);
    let mensajes = cuaderno::Cuaderno::leer_de(&dir)?.mensajes;
    let Some(m) = mensajes.iter().find(|m| m.id == pagina) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "esa pagina no esta en el chat",
        ));
    };
    let (id, creada) = match carpeta_por_nombre(&mensajes, nombre) {
        Some(i) => (mensajes[i].id.clone(), false),
        None => {
            let numero = super::siguiente_numero_en(raiz, proyecto)?;
            // El id es el milisegundo: el lienzo recien hecho puede ser del
            // mismo (lo vio la CI) y entonces la carpeta seria el lienzo.
            let mut cuando = pixpin_shell::entorno::ahora_utc_ms();
            while mensajes.iter().any(|m| m.id == cuando.to_string()) {
                cuando += 1;
            }
            let nueva = carpetas::mensaje_nuevo(
                nombre,
                &cuaderno::Sello {
                    cuando,
                    numero,
                    aparato: aparato.to_string(),
                    proyecto: proyecto.to_string(),
                },
            );
            cuaderno::anadir(&dir, &nueva)?;
            (nueva.id, true)
        }
    };
    let nombre_de_la_pagina = if m.nombre.ends_with(".excalidraw") {
        String::new()
    } else {
        pixpin_docs::sin_extension(m.nombre.trim())
    };
    let (mut titulo, mut ya_estaba) = (String::new(), false);
    let c = carpetas::cambiar_en_disco(&dir, &id, |c| {
        if let Some(i) = c.posicion(pagina) {
            ya_estaba = true;
            titulo = c.paginas[i].titulo.clone();
            return false;
        }
        titulo = c.titulo_al_anadir(&nombre_de_la_pagina, por_defecto);
        c.anadir(pagina, &titulo)
    })?
    .ok_or_else(|| std::io::Error::other("la carpeta no se entiende"))?;
    super::refrescar();
    let carpeta = cuaderno::Cuaderno::leer_de(&dir)?
        .mensajes
        .into_iter()
        .find(|m| m.id == id)
        .map_or_else(|| nombre.trim().to_string(), |m| m.nombre);
    Ok(Anadida {
        carpeta,
        titulo,
        paginas: c.len(),
        creada,
        ya_estaba,
    })
}

// --- Lo del chat: crear una carpeta y anadirle paginas ----------------------------

/// Si el mensaje se ofrece para «Añadir a carpeta…»: lo que es una pagina
/// (`carpeta_de_paginas::puede_ir`) y se ve en el chat (no las paginas del
/// PDF que solo salen en la galeria).
pub(super) fn se_ofrece(m: &Mensaje) -> bool {
    carpetas::puede_ir(m) && !crate::pdf_en_chat::solo_galeria(m)
}

/// **Una carpeta nueva al final del chat**, con `primera` dentro si se da
/// («Nueva carpeta con esta pagina»), y con el nombre a medio escribir en la
/// barra de renombrar, como un lienzo recien hecho espera que se dibuje.
pub(super) fn crear(
    ubicacion: &Ubicacion,
    a: &mut super::Abierto,
    identidad: &str,
    textos: &Catalogo,
    primera: Option<usize>,
) -> super::Efecto {
    let nombre = nombre_libre(&textos.t("carpeta-nombre-de-serie"), &a.mensajes);
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    // El id es el milisegundo: que no sea el de otro mensaje.
    let mut cuando = pixpin_shell::entorno::ahora_utc_ms();
    while a.mensajes.iter().any(|m| m.id == cuando.to_string()) {
        cuando += 1;
    }
    let mut m = carpetas::mensaje_nuevo(
        &nombre,
        &cuaderno::Sello {
            cuando,
            numero,
            aparato: identidad.to_string(),
            proyecto: a.ficha.id.clone(),
        },
    );
    if let Some(pagina) = primera
        .and_then(|i| a.mensajes.get(i))
        .filter(|p| se_ofrece(p))
    {
        let mut c = Carpeta::default();
        let titulo =
            c.titulo_al_anadir(&nombre_de_hoja(pagina, textos), &textos.t("carpeta-pagina"));
        c.anadir(&pagina.id, &titulo);
        m.texto = c.escribir();
    }
    let dir = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    if let Err(e) = cuaderno::anadir(&dir, &m) {
        tracing::warn!(?e, "no se pudo crear la carpeta");
        return super::Efecto::Aviso(textos.t("chat-no-se-pudo"));
    }
    a.vistas.push(super::leer_vista(ubicacion, &a.ficha.id, &m));
    a.ficha.tocado = m.cuando;
    a.ficha.resumen = nombre.clone();
    a.mensajes.push(m);
    marcar_lo_de_dentro(&mut a.mensajes);
    let _ = super::subir_en_la_lista(
        ubicacion.raiz(),
        &a.ficha.id,
        a.ficha.tocado,
        &a.ficha.resumen,
    );
    // El nombre, ya escrito y listo para cambiarlo: «Cuaderno» no dice nada.
    a.renombrando_mensaje = Some((
        a.mensajes.len() - 1,
        super::redaccion::CajaLinea::con_texto(&nombre),
    ));
    a.describiendo = false;
    a.scroll = None;
    super::Efecto::Cambio
}

/// El nombre con el que entra una hoja: el suyo si se lo pusieron, y si no
/// nada (entonces cuenta la serie o «Pagina N»). «Lienzo» o
/// `<id>.excalidraw` no le dicen nada a nadie en un cuaderno.
fn nombre_de_hoja(m: &Mensaje, textos: &Catalogo) -> String {
    let n = super::nombre_de_la_fila(m, textos);
    let de_serie = n == textos.t("chat-lienzo") || n.ends_with(".excalidraw");
    if de_serie {
        String::new()
    } else {
        pixpin_docs::sin_extension(&n)
    }
}

/// **El menu de «Añadir a carpeta…»**: las carpetas del chat (en la que ya
/// esta, marcada) y una nueva con esta pagina.
pub(super) fn menu_de_anadir(
    a: &super::Abierto,
    i: usize,
    textos: &Catalogo,
) -> Vec<super::EntradaMenu> {
    let Some(m) = a.mensajes.get(i) else {
        return Vec::new();
    };
    let mut v: Vec<super::EntradaMenu> = del_chat(&a.mensajes, textos)
        .into_iter()
        .map(|(j, nombre)| {
            let c = Carpeta::de_mensaje(&a.mensajes[j]);
            let ya = c.as_ref().is_some_and(|c| c.contiene(&m.id));
            let de_clase = c.as_ref().is_some_and(Carpeta::es_grabacion);
            super::entrada(
                Some(if ya {
                    &mi::CHECK_BOX
                } else if de_clase {
                    &mi::MIC
                } else {
                    &mi::MENU_BOOK
                }),
                nombre,
                super::Accion::AnadirACarpetaDe(i, a.mensajes[j].id.clone()),
            )
        })
        .collect();
    v.push(
        super::entrada(
            Some(&mi::ADD),
            textos.t("carpeta-nueva-con"),
            super::Accion::NuevaCarpetaCon(i),
        )
        .separada(),
    );
    v
}

/// **Mete el mensaje `i` en la carpeta `carpeta`** (el `id` de su mensaje),
/// al final y con el titulo que le toca («Dia 3» si la carpeta lleva una
/// serie). Dice como entro, o que ya estaba.
pub(super) fn anadir(
    ubicacion: &Ubicacion,
    a: &mut super::Abierto,
    i: usize,
    carpeta: &str,
    textos: &Catalogo,
) -> super::Efecto {
    let Some(m) = a.mensajes.get(i).filter(|m| se_ofrece(m)).cloned() else {
        return super::Efecto::Nada;
    };
    let Some(k) = a.mensajes.iter().position(|x| x.id == carpeta) else {
        return super::Efecto::Aviso(textos.t("carpeta-no-esta"));
    };
    let nombre_carpeta = super::nombre_de_la_fila(&a.mensajes[k], textos);
    let dir = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    let nombre = nombre_de_hoja(&m, textos);
    let por_defecto = textos.t("carpeta-pagina");
    let mut titulo = String::new();
    let mut ya = false;
    let hecho = carpetas::cambiar_en_disco(&dir, carpeta, |c| {
        if c.contiene(&m.id) {
            ya = true;
            return false;
        }
        // A una grabacion entra como adjunto a la clase, con su nombre: no
        // es de lo grabado y no lleva minuto.
        if c.es_grabacion() {
            titulo = nombre.clone();
            return c.adjuntar(&m.id, &titulo);
        }
        titulo = c.titulo_al_anadir(&nombre, &por_defecto);
        c.anadir(&m.id, &titulo)
    });
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("carpeta", nombre_carpeta);
    match hecho {
        Ok(Some(c)) => {
            // La tarjeta de la carpeta, al dia sin releer todo el chat.
            a.mensajes[k].texto = c.escribir();
            if let Some(v) = a.vistas.get_mut(k) {
                *v = super::leer_vista(ubicacion, &a.ficha.id, &a.mensajes[k]);
            }
            // Y la pagina se va del chat: ahora vive en la carpeta.
            marcar_lo_de_dentro(&mut a.mensajes);
            if ya {
                return super::Efecto::Aviso(textos.t_args("carpeta-ya-estaba", &args));
            }
            args.set("titulo", titulo);
            super::Efecto::Aviso(textos.t_args("carpeta-anadida", &args))
        }
        Ok(None) => super::Efecto::Aviso(textos.t("carpeta-no-esta")),
        Err(e) => {
            tracing::warn!(?e, "no se pudo anadir a la carpeta");
            super::Efecto::Aviso(textos.t("chat-no-se-pudo"))
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::cuaderno::Sello;

    fn sello(n: i64) -> Sello {
        Sello {
            cuando: 1_000 + n,
            numero: n,
            aparato: "K7Q2".into(),
            proyecto: "p".into(),
        }
    }

    #[test]
    fn el_nombre_de_una_carpeta_nueva_no_se_repite() {
        let mut mensajes = vec![carpetas::mensaje_nuevo("Cuaderno", &sello(1))];
        assert_eq!(nombre_libre("Cuaderno", &mensajes), "Cuaderno 2");
        mensajes.push(carpetas::mensaje_nuevo("Cuaderno 2", &sello(2)));
        assert_eq!(nombre_libre("Cuaderno", &mensajes), "Cuaderno 3");
        // Caso negativo: sin carpetas con ese nombre, el de siempre.
        assert_eq!(nombre_libre("Diario", &mensajes), "Diario");
    }

    /// «Nueva pagina»: el Dia 2 nace con la plantilla del Dia 1, vacia, y
    /// entra al final de la carpeta; sin plantilla, en blanco.
    #[test]
    fn la_pagina_siguiente_repite_la_plantilla_vacia_con_el_titulo_siguiente() {
        let raiz = std::env::temp_dir().join(format!("pixpin-carpeta-sig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let dir = pixpin_proyecto::almacen::carpeta(&raiz, "p");
        std::fs::create_dir_all(&dir).unwrap();
        // El Dia 1: un lienzo con el diario de serie y algo escrito encima.
        let dia1 = super::super::escribir_lienzo(&raiz, "p", "PC", 1, Some("Día 1")).unwrap();
        let f = pixpin_motor2d::figuras_de_serie::catalogo()
            .into_iter()
            .find(|f| f.id == "serie-diario")
            .unwrap();
        let mut escena = pixpin_motor2d::Escena::nueva();
        let (w, h) = f.tamano();
        pixpin_motor2d::biblioteca::estampar_en_escena(
            &mut escena,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(w / 2.0, h / 2.0),
        );
        escena.anadir(pixpin_motor2d::Elemento {
            x: 100.0,
            y: 300.0,
            ancho: 50.0,
            alto: 20.0,
            figura: pixpin_motor2d::Figura::Rectangulo,
            ..Default::default()
        });
        let ruta =
            pixpin_proyecto::almacen::lienzo(&raiz, "p", dia1.referencia.as_deref().unwrap());
        let l = pixpin_motor2d::excalidraw::con_escena(
            &pixpin_motor2d::excalidraw::Lienzo::vacio(),
            &escena,
        );
        std::fs::write(&ruta, pixpin_motor2d::excalidraw::escribir(&l)).unwrap();
        let mut carpeta = carpetas::mensaje_nuevo("Diario", &sello(2));
        carpeta.id = "carpeta".into();
        let mut c = Carpeta::default();
        c.anadir(&dia1.id, "Día 1");
        carpeta.texto = c.escribir();
        cuaderno::anadir(&dir, &carpeta).unwrap();
        // El id de un lienzo es su hora: que el nuevo no nazca en el mismo
        // milisegundo que el Dia 1.
        std::thread::sleep(std::time::Duration::from_millis(3));

        let n = pagina_siguiente(&raiz, "p", "PC", "carpeta", "Página").unwrap();
        assert_eq!(n.titulo, "Día 2");
        assert!(n.con_plantilla);
        assert_eq!(
            n.mensaje.nombre, "Día 2",
            "el lienzo se llama como su pagina"
        );
        // El lienzo nuevo: solo la plantilla, sin lo escrito.
        let texto = std::fs::read_to_string(pixpin_proyecto::almacen::lienzo(
            &raiz,
            "p",
            n.mensaje.referencia.as_deref().unwrap(),
        ))
        .unwrap();
        let nueva = pixpin_motor2d::excalidraw::a_escena(
            &pixpin_motor2d::excalidraw::leer(&texto).unwrap(),
        );
        assert_eq!(
            nueva.cuantos_visibles(),
            f.elementos.iter().filter(|e| !e.borrado).count()
        );
        // Y la carpeta, con las dos en orden.
        let leida = cuaderno::Cuaderno::leer_de(&dir).unwrap();
        let c = leida
            .mensajes
            .iter()
            .find(|m| m.id == "carpeta")
            .and_then(Carpeta::de_mensaje)
            .unwrap();
        let titulos: Vec<&str> = c.paginas.iter().map(|p| p.titulo.as_str()).collect();
        assert_eq!(titulos, ["Día 1", "Día 2"]);

        // Caso negativo: una carpeta que no esta es un error, y no crea nada.
        let antes = cuaderno::Cuaderno::leer_de(&dir).unwrap().mensajes.len();
        assert!(pagina_siguiente(&raiz, "p", "PC", "no-esta", "Página").is_err());
        assert_eq!(
            cuaderno::Cuaderno::leer_de(&dir).unwrap().mensajes.len(),
            antes
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_tarjeta_y_el_detalle_salen_del_documento() {
        let mut m = carpetas::mensaje_nuevo("Diario", &sello(1));
        assert_eq!(cuantas(&m), 0);
        assert_eq!(ids_de_la_tarjeta(&m), Some(Vec::new()));
        let mut c = Carpeta::default();
        c.anadir("a", "Día 1");
        c.anadir("b", "Día 2");
        m.texto = c.escribir();
        assert_eq!(cuantas(&m), 2);
        assert_eq!(ids_de_la_tarjeta(&m), Some(vec!["a".into(), "b".into()]));
        // Caso negativo: una nota no es una carpeta.
        let nota = Mensaje::nota("x", &sello(2));
        assert_eq!(cuantas(&nota), 0);
        assert!(ids_de_la_tarjeta(&nota).is_none());
    }

    #[test]
    fn el_icono_de_una_pagina_sin_miniatura_dice_lo_que_es() {
        let m = |c: Clase| Mensaje {
            clase: Some(c),
            ..Default::default()
        };
        // Los iconos son `const`: se comparan por lo que dibujan, no por su
        // direccion (en Windows cada uso puede ser una copia).
        let igual = |a: &Icono, b: &Icono| {
            a.vista == b.vista
                && a.trazos.len() == b.trazos.len()
                && format!("{:?}", a.trazos) == format!("{:?}", b.trazos)
        };
        assert!(igual(icono_de(&m(Clase::Dibujo)), &mi::DRAW));
        assert!(igual(icono_de(&m(Clase::Imagen)), &mi::IMAGE));
        assert!(igual(icono_de(&m(Clase::Archivo)), &mi::DESCRIPTION));
        assert!(!igual(icono_de(&m(Clase::Dibujo)), &mi::IMAGE));
    }
}
