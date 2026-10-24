//! **La ventana de las figuras** (24-oct-2026, el usuario: «la
//! visualizacion de las figuras … que sea como una galeria donde yo pueda
//! ver las figuras asi realizadas agrupadas, de forma que sea todo intuitivo
//! y muy rapido de ver»; 25-oct: «las figuras deberian aparecer como una
//! ventana separada, y deberia haber como un previsualizador de las figuras
//! y de todo lo que tiene»).
//!
//! Se abre con la tercera parte del interruptor de la barra del chat
//! (Chat · Proyectos · **Figuras**) y desde el menu de la bandeja. Una sola a
//! la vez, en su hilo, como la galeria de capturas:
//!
//! - **A la izquierda, los grupos** con su cuenta: «Todas», «Mis figuras» y
//!   las categorias (la categoria va en el nombre, «Diario · Gratitud», ver
//!   `figuras_de_serie::categoria_de`).
//! - **En el centro, cada grupo con su titulo** y sus figuras en rejilla,
//!   dibujadas de verdad (las mismas ordenes que la vista previa de un lienzo
//!   en el chat), con su nombre. Arriba, un buscador: se escribe y filtra.
//! - **A la derecha, el previsualizador** de la elegida: en grande, con su
//!   grupo, lo que mide, de que piezas esta hecha y los textos que lleva. Las
//!   del usuario se pueden borrar (dos clics).
//!
//! Teclado: escribir busca, las flechas recorren, Escape borra lo buscado y,
//! sin nada buscado, cierra.

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicIsize, Ordering};

use anyhow::{Context, Result};
use pixpin_geom::{Punto, Rect};
use pixpin_motor2d::biblioteca::{self, FiguraGuardada};
use pixpin_motor2d::figuras_de_serie::{CATEGORIAS, categoria_de, es_de_serie};
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma};

use super::{CLARO, LienzoVisto, OSCURO, Tema, con_alfa, mi, pintar_lienzo, rf};
use crate::overlay::Recursos;

/// Lo que mide una ficha de la rejilla (logicos): el dibujo y su nombre.
const CELDA_ANCHO: u32 = 156;
const CELDA_DIBUJO: u32 = 112;
const CELDA_NOMBRE: u32 = 30;
const HUECO: u32 = 12;
const MARGEN: u32 = 18;
const BARRA: u32 = 48;
const GRUPOS: u32 = 210;
const FILA_GRUPO: u32 = 36;
const PREVIA: u32 = 380;
const ALTO_TITULO: u32 = 32;
/// El grupo de lo que no lleva categoria: lo del usuario.
pub(super) const MIS_FIGURAS: &str = "";

const VK_ESCAPE: u32 = 0x1B;
const VK_RETROCESO: u32 = 0x08;
const VK_IZQUIERDA: u32 = 0x25;
const VK_ARRIBA: u32 = 0x26;
const VK_DERECHA: u32 = 0x27;
const VK_ABAJO: u32 = 0x28;

/// La ventana abierta (su HWND), -1 mientras nace, 0 sin ventana.
static ABIERTA: AtomicIsize = AtomicIsize::new(0);

/// **Abre la ventana de las figuras**, o la trae delante si ya estaba.
pub(crate) fn abrir(idioma: Idioma) {
    let ya = ABIERTA.load(Ordering::SeqCst);
    if ya > 0 {
        VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(ya as *mut _));
        return;
    }
    if ya < 0 {
        return;
    }
    ABIERTA.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new()
        .name("figuras".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            let mut g = Galeria::default();
            g.leer();
            let hecho =
                crate::dispositivo_perdido::con_recursos("figuras", |r| bucle(r, &textos, &mut g));
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la ventana de las figuras");
            }
            ABIERTA.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de las figuras");
        ABIERTA.store(0, Ordering::SeqCst);
    }
}

// --- Lo de cada figura -------------------------------------------------------

/// De que esta hecha una figura, para el previsualizador.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Piezas {
    pub total: usize,
    pub trazos: usize,
    pub lineas: usize,
    pub formas: usize,
    pub textos: usize,
    pub otras: usize,
}

/// Una figura, ya lista para pintar.
struct Vista {
    id: String,
    /// El nombre sin la categoria delante.
    nombre: String,
    grupo: String,
    de_serie: bool,
    tamano: (f32, f32),
    piezas: Piezas,
    /// Los textos que lleva, en orden de lectura (arriba abajo).
    letras: Vec<String>,
    dibujo: Option<LienzoVisto>,
}

pub(super) fn piezas_de(f: &FiguraGuardada) -> Piezas {
    use pixpin_motor2d::Figura as F;
    let mut p = Piezas::default();
    for e in f.elementos.iter().filter(|e| !e.borrado) {
        p.total += 1;
        match &e.figura {
            F::Lapiz { .. } | F::Resaltador { .. } => p.trazos += 1,
            F::Linea { .. } | F::Flecha { .. } | F::Arco { .. } | F::Cota { .. } => p.lineas += 1,
            F::Rectangulo | F::Rombo | F::Elipse | F::Region { .. } => p.formas += 1,
            F::Texto { .. } => p.textos += 1,
            _ => p.otras += 1,
        }
    }
    p
}

pub(super) fn letras_de(f: &FiguraGuardada) -> Vec<String> {
    let mut v: Vec<(f32, f32, String)> = f
        .elementos
        .iter()
        .filter(|e| !e.borrado)
        .filter_map(|e| match &e.figura {
            pixpin_motor2d::Figura::Texto { texto, .. } if !texto.trim().is_empty() => {
                Some((e.y, e.x, texto.trim().replace('\n', " ")))
            }
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    v.into_iter().map(|(_, _, t)| t).collect()
}

/// El orden de los grupos: lo del usuario, y las categorias de serie en el
/// orden del menu del lienzo; una categoria inventada por el usuario, detras.
fn orden_del_grupo(g: &str) -> usize {
    if g == MIS_FIGURAS {
        0
    } else {
        CATEGORIAS
            .iter()
            .position(|c| *c == g)
            .map_or(CATEGORIAS.len() + 1, |i| i + 1)
    }
}

fn vista_de(f: &FiguraGuardada) -> Vista {
    let (grupo, nombre) = match categoria_de(&f.nombre) {
        Some((c, n)) => (c.to_string(), n.to_string()),
        None => (MIS_FIGURAS.to_string(), f.nombre.clone()),
    };
    Vista {
        id: f.id.clone(),
        nombre,
        grupo,
        de_serie: es_de_serie(f),
        tamano: f.tamano(),
        piezas: piezas_de(f),
        letras: letras_de(f),
        dibujo: dibujo_de(f),
    }
}

/// La figura como la vista previa de un lienzo: sus ordenes y su caja.
fn dibujo_de(f: &FiguraGuardada) -> Option<LienzoVisto> {
    let mut escena = pixpin_motor2d::escena::Escena::nueva();
    for e in &f.elementos {
        escena.anadir(e.clone());
    }
    let caja = escena.caja()?;
    let lado = (caja.2 - caja.0).max(caja.3 - caja.1).max(1.0);
    let (ordenes, grafitos) = pixpin_motor2d::tinta::grafito::ordenes_con_grafito(
        &escena,
        (640.0 / lado).min(1.0),
        1 << 21,
    );
    let ordenes = crate::dibujo::tema::ordenes_como_en_el_lienzo(ordenes, escena.fondo);
    Some(LienzoVisto {
        ordenes,
        grafitos,
        caja,
        fondo: None,
        giro_del_fondo: 0.0,
        papel: None,
    })
}

/// El nombre de un grupo para ensenarlo.
fn rotulo_del_grupo(g: &str, textos: &Catalogo) -> String {
    if g == MIS_FIGURAS {
        textos.t("figuras-mias")
    } else {
        g.to_string()
    }
}

/// Si la figura casa con lo buscado (sin mayusculas ni tildes, en su
/// nombre, su grupo o sus textos).
fn casa(f: &Vista, consulta: &str) -> bool {
    let q = plegar(consulta.trim());
    q.is_empty()
        || plegar(&f.nombre).contains(&q)
        || plegar(&f.grupo).contains(&q)
        || f.letras.iter().any(|l| plegar(l).contains(&q))
}

pub(super) fn plegar(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            otra => otra,
        })
        .collect()
}

// --- El estado ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Zona {
    Grupo(Option<String>),
    Figura(usize),
    Borrar,
    Cerrar,
    Barra,
}

struct Galeria {
    figuras: Vec<Vista>,
    /// El grupo elegido a la izquierda; `None`, todas.
    filtro: Option<String>,
    consulta: String,
    scroll: i32,
    elegida: Option<usize>,
    borrar_pedido: bool,
    raton: Punto,
    zonas: RefCell<Vec<(Rect, Zona)>>,
    alto_total: Cell<i32>,
    /// Las que se ven, en el orden en que se pintan (para las flechas).
    visibles: RefCell<Vec<usize>>,
    columnas: Cell<usize>,
}

impl Default for Galeria {
    fn default() -> Self {
        Galeria {
            figuras: Vec::new(),
            filtro: None,
            consulta: String::new(),
            scroll: 0,
            elegida: None,
            borrar_pedido: false,
            raton: Punto { x: -1, y: -1 },
            zonas: RefCell::new(Vec::new()),
            alto_total: Cell::new(0),
            visibles: RefCell::new(Vec::new()),
            columnas: Cell::new(1),
        }
    }
}

impl Galeria {
    fn leer(&mut self) {
        let elegida = self
            .elegida
            .and_then(|i| self.figuras.get(i))
            .map(|f| f.id.clone());
        let figuras = biblioteca::cargar(&crate::biblioteca_figuras::ruta());
        self.figuras = figuras.iter().map(vista_de).collect();
        // Las del usuario primero y despues las categorias en su orden.
        self.figuras.sort_by_key(|f| orden_del_grupo(&f.grupo));
        if let Some(g) = &self.filtro
            && !self.figuras.iter().any(|f| &f.grupo == g)
        {
            self.filtro = None;
        }
        self.elegida = elegida
            .and_then(|id| self.figuras.iter().position(|f| f.id == id))
            .or(if self.figuras.is_empty() {
                None
            } else {
                Some(0)
            });
    }

    /// Los grupos que hay, en orden, con cuantas figuras tiene cada uno.
    fn grupos(&self) -> Vec<(String, usize)> {
        let mut v: Vec<(String, usize)> = Vec::new();
        for f in &self.figuras {
            match v.iter_mut().find(|(g, _)| *g == f.grupo) {
                Some((_, n)) => *n += 1,
                None => v.push((f.grupo.clone(), 1)),
            }
        }
        v
    }

    fn zona_en(&self, l: Punto) -> Option<Zona> {
        self.zonas
            .borrow()
            .iter()
            .rev()
            .find(|(r, _)| r.contiene(l))
            .map(|(_, z)| z.clone())
    }

    fn pulsar(&mut self, l: Punto, vivo: &mut bool) -> Option<Zona> {
        let zona = self.zona_en(l);
        if zona != Some(Zona::Borrar) {
            self.borrar_pedido = false;
        }
        match &zona {
            Some(Zona::Grupo(g)) => {
                self.filtro = g.clone();
                self.scroll = 0;
            }
            Some(Zona::Figura(i)) => self.elegida = Some(*i),
            Some(Zona::Borrar) => {
                if self.borrar_pedido {
                    self.borrar();
                } else {
                    self.borrar_pedido = true;
                }
            }
            Some(Zona::Cerrar) => *vivo = false,
            _ => {}
        }
        zona
    }

    /// Las flechas: a la de al lado o a la de arriba/abajo en la rejilla.
    fn mover_eleccion(&mut self, dx: i32, dy: i32) {
        let v = self.visibles.borrow().clone();
        if v.is_empty() {
            return;
        }
        let actual = self
            .elegida
            .and_then(|e| v.iter().position(|i| *i == e))
            .unwrap_or(0) as i32;
        let paso = dx + dy * self.columnas.get().max(1) as i32;
        let nueva = (actual + paso).clamp(0, v.len() as i32 - 1);
        self.elegida = Some(v[nueva as usize]);
    }

    /// Quita de la biblioteca la elegida (solo las del usuario).
    fn borrar(&mut self) {
        self.borrar_pedido = false;
        let Some(f) = self
            .elegida
            .and_then(|i| self.figuras.get(i))
            .filter(|f| !f.de_serie)
        else {
            return;
        };
        let ruta = crate::biblioteca_figuras::ruta();
        let mut todas = biblioteca::cargar(&ruta);
        let antes = todas.len();
        todas.retain(|t| t.id != f.id);
        if todas.len() != antes && crate::biblioteca_figuras::guardar(&ruta, &todas) {
            tracing::info!(nombre = %f.nombre, "figura borrada desde la ventana de figuras");
        }
        self.elegida = None;
        self.leer();
    }
}

// --- La ventana ----------------------------------------------------------------

fn bucle(recursos: &Recursos, textos: &Catalogo, g: &mut Galeria) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .monitor_en(pixpin_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien;
    let mut marco =
        crate::ventanita::centrado(monitor.area_trabajo, 1320, 840, monitor.escala_por_cien);
    let mut ventana = VentanaOverlay::nueva_normal(marco, &textos.t("figuras-titulo"))
        .context("no se pudo abrir la ventana de las figuras")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para las figuras")?;
    ventana.mostrar();
    ventana.enfocar();
    ABIERTA.store(ventana.handle().0 as isize, Ordering::SeqCst);
    tracing::info!(figuras = g.figuras.len(), "ventana de las figuras abierta");

    let mut moviendo: Option<(Punto, Rect)> = None;
    let mut vivo = true;
    let mut pintar = true;
    let mut leida = std::fs::metadata(crate::biblioteca_figuras::ruta())
        .and_then(|m| m.modified())
        .ok();
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
                    if g.zona_en(l) != g.zona_en(g.raton) {
                        pintar = true;
                    }
                    g.raton = l;
                }
                EventoOverlay::BotonPulsado(p) => {
                    let l = local(p, marco);
                    g.raton = l;
                    if g.pulsar(l, &mut vivo) == Some(Zona::Barra) {
                        moviendo = Some((p, marco));
                        ventana.capturar_raton();
                    }
                    pintar = true;
                }
                EventoOverlay::BotonSoltado(_) => {
                    if moviendo.take().is_some() {
                        ventana.soltar_raton();
                    }
                }
                EventoOverlay::Rueda(delta) => {
                    let paso = (CELDA_DIBUJO * escala / 100) as i32;
                    let nuevo = (g.scroll - delta * paso / 120 * 6 / 10)
                        .clamp(0, g.alto_total.get().max(0));
                    if nuevo != g.scroll {
                        g.scroll = nuevo;
                        pintar = true;
                    }
                }
                EventoOverlay::Tecla { vk, .. } => {
                    pintar = true;
                    match vk {
                        VK_ESCAPE if !g.consulta.is_empty() => g.consulta.clear(),
                        VK_ESCAPE => vivo = false,
                        VK_RETROCESO => {
                            g.consulta.pop();
                        }
                        VK_IZQUIERDA => g.mover_eleccion(-1, 0),
                        VK_DERECHA => g.mover_eleccion(1, 0),
                        VK_ARRIBA => g.mover_eleccion(0, -1),
                        VK_ABAJO => g.mover_eleccion(0, 1),
                        _ => pintar = false,
                    }
                }
                EventoOverlay::Caracter(c) if !c.is_control() => {
                    if g.consulta.chars().count() < 40 {
                        g.consulta.push(c);
                        g.scroll = 0;
                    }
                    pintar = true;
                }
                _ => {}
            }
        }
        if !vivo {
            break;
        }
        // La biblioteca cambio por fuera (una figura guardada en un lienzo,
        // llegada del movil): se relee.
        if mirado.elapsed() >= std::time::Duration::from_secs(2) {
            mirado = std::time::Instant::now();
            let ahora = std::fs::metadata(crate::biblioteca_figuras::ruta())
                .and_then(|m| m.modified())
                .ok();
            if ahora != leida {
                leida = ahora;
                g.leer();
                pintar = true;
            }
        }
        if pintar {
            pintar = false;
            let tema = if pixpin_shell::entorno::tema_claro() {
                &CLARO
            } else {
                &OSCURO
            };
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| {
                    pintar_todo(p, g, marco.ancho, marco.alto, tema, escala, textos)
                });
                let _ = superficie.presentar();
            }
        }
        pixpin_shell::overlay::esperar_eventos(Some(1_000));
    }
    ventana.ocultar();
    tracing::info!("ventana de las figuras cerrada");
    Ok(())
}

// --- Pintar ----------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn pintar_todo(
    p: &Pintor,
    g: &Galeria,
    ancho: u32,
    alto: u32,
    tema: &Tema,
    escala: u32,
    textos: &Catalogo,
) {
    let e = escala as f32 / 100.0;
    let ei = |x: u32| x * escala / 100;
    let mut zonas = g.zonas.borrow_mut();
    zonas.clear();
    let todo = Rect {
        x: 0,
        y: 0,
        ancho,
        alto,
    };
    p.rellenar(rf(todo), tema.chat);

    // La barra: titulo, buscador y cerrar. Lo que no es boton la arrastra.
    let barra = Rect {
        alto: ei(BARRA),
        ..todo
    };
    p.rellenar(rf(barra), tema.barra);
    zonas.push((barra, Zona::Barra));
    let titulo = format!("{} · {}", textos.t("figuras-titulo"), g.figuras.len());
    let (_, th) = p.medir_texto(&titulo, 16.0 * e);
    p.texto(
        &titulo,
        ei(MARGEN) as f32,
        (barra.alto as f32 - th) / 2.0,
        16.0 * e,
        tema.texto,
    );
    let buscador = Rect {
        x: ei(GRUPOS + MARGEN) as i32,
        y: ei(9) as i32,
        ancho: ei(360),
        alto: ei(BARRA - 18),
    };
    p.rellenar_redondeado(rf(buscador), buscador.alto as f32 / 2.0, tema.buscador);
    p.icono(
        &mi::SEARCH,
        RectF {
            x: buscador.x as f32 + 10.0 * e,
            y: buscador.y as f32 + (buscador.alto as f32 - 18.0 * e) / 2.0,
            ancho: 18.0 * e,
            alto: 18.0 * e,
        },
        tema.apagado,
    );
    let (buscado, color) = if g.consulta.is_empty() {
        (textos.t("figuras-buscar"), tema.apagado)
    } else {
        (format!("{}▏", g.consulta), tema.texto)
    };
    let (_, bh) = p.medir_texto(&buscado, 13.5 * e);
    p.texto_linea(
        &buscado,
        buscador.x as f32 + 36.0 * e,
        buscador.y as f32 + (buscador.alto as f32 - bh) / 2.0,
        13.5 * e,
        buscador.ancho as f32 - 44.0 * e,
        color,
    );
    let cerrar = Rect {
        x: ancho as i32 - ei(BARRA) as i32,
        y: 0,
        ancho: ei(BARRA),
        alto: ei(BARRA),
    };
    if cerrar.contiene(g.raton) {
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

    // Los grupos, a la izquierda.
    let lista = Rect {
        x: 0,
        y: barra.abajo(),
        ancho: ei(GRUPOS),
        alto: alto.saturating_sub(barra.alto),
    };
    p.rellenar(rf(lista), tema.lista);
    let grupos = g.grupos();
    let mut filas: Vec<(Option<String>, String, usize)> =
        vec![(None, textos.t("figuras-todas"), g.figuras.len())];
    filas.extend(
        grupos
            .iter()
            .map(|(gr, n)| (Some(gr.clone()), rotulo_del_grupo(gr, textos), *n)),
    );
    // Las filas se encogen para que entren todos los grupos (hay 18 de serie).
    let paso_fila = (lista.alto.saturating_sub(ei(20)) / filas.len().max(1) as u32)
        .clamp(ei(22), ei(FILA_GRUPO));
    let mut y = lista.y + ei(10) as i32;
    for (gr, rotulo, n) in filas {
        let fila = Rect {
            x: ei(8) as i32,
            y,
            ancho: lista.ancho - ei(16),
            alto: paso_fila - ei(4),
        };
        if fila.abajo() > lista.abajo() {
            break;
        }
        let elegida = g.filtro == gr;
        if elegida || fila.contiene(g.raton) {
            p.rellenar_redondeado(
                rf(fila),
                8.0 * e,
                if elegida {
                    tema.fila_elegida
                } else {
                    tema.fila_sobre
                },
            );
        }
        let tinta = if elegida {
            tema.texto_elegido
        } else {
            tema.texto
        };
        let cuenta = n.to_string();
        let (cw, ch) = p.medir_texto(&cuenta, 12.0 * e);
        p.texto_linea(
            &rotulo,
            fila.x as f32 + 12.0 * e,
            fila.y as f32 + (fila.alto as f32 - ch) / 2.0,
            13.5 * e,
            fila.ancho as f32 - cw - 32.0 * e,
            tinta,
        );
        p.texto(
            &cuenta,
            fila.derecha() as f32 - cw - 12.0 * e,
            fila.y as f32 + (fila.alto as f32 - ch) / 2.0,
            12.0 * e,
            if elegida { tinta } else { tema.apagado },
        );
        zonas.push((fila, Zona::Grupo(gr)));
        y += paso_fila as i32;
    }

    // La rejilla, por grupos.
    let previa_ancho = ei(PREVIA);
    let rejilla = Rect {
        x: lista.derecha(),
        y: barra.abajo(),
        ancho: ancho.saturating_sub(lista.ancho + previa_ancho),
        alto: lista.alto,
    };
    let margen = ei(MARGEN) as i32;
    let ancho_util = rejilla.ancho.saturating_sub(2 * ei(MARGEN));
    let paso = ei(CELDA_ANCHO + HUECO);
    let columnas = ((ancho_util + ei(HUECO)) / paso.max(1)).max(1) as usize;
    g.columnas.set(columnas);
    let alto_celda = ei(CELDA_DIBUJO + CELDA_NOMBRE);
    let mut visibles = Vec::new();
    p.empujar_recorte(rf(rejilla));
    let mut y = rejilla.y - g.scroll + ei(6) as i32;
    for (gr, _) in grupos
        .iter()
        .filter(|(gr, _)| g.filtro.as_ref().is_none_or(|f| f == gr))
    {
        let de_este: Vec<usize> = (0..g.figuras.len())
            .filter(|i| &g.figuras[*i].grupo == gr && casa(&g.figuras[*i], &g.consulta))
            .collect();
        if de_este.is_empty() {
            continue;
        }
        y += ei(8) as i32;
        p.texto(
            &format!("{}  ({})", rotulo_del_grupo(gr, textos), de_este.len()),
            (rejilla.x + margen) as f32,
            y as f32 + 4.0 * e,
            15.0 * e,
            tema.texto,
        );
        y += ei(ALTO_TITULO) as i32;
        for (k, i) in de_este.iter().enumerate() {
            let (col, fila) = (k % columnas, k / columnas);
            let celda = Rect {
                x: rejilla.x + margen + (col as u32 * paso) as i32,
                y: y + (fila as u32 * (alto_celda + ei(HUECO))) as i32,
                ancho: ei(CELDA_ANCHO),
                alto: alto_celda,
            };
            visibles.push(*i);
            if celda.abajo() < rejilla.y || celda.y > rejilla.abajo() {
                continue;
            }
            pintar_celda(
                p,
                &g.figuras[*i],
                celda,
                tema,
                e,
                celda.contiene(g.raton),
                g.elegida == Some(*i),
            );
            if let Some(vista) = celda.interseccion(rejilla) {
                zonas.push((vista, Zona::Figura(*i)));
            }
        }
        let filas = de_este.len().div_ceil(columnas) as u32;
        y += (filas * (alto_celda + ei(HUECO))) as i32;
    }
    if visibles.is_empty() {
        let vacio = if g.figuras.is_empty() {
            textos.t("figuras-vacia")
        } else {
            textos.t("figuras-sin-resultados")
        };
        let (w, _) = p.medir_texto_ajustado(&vacio, 14.0 * e, rejilla.ancho as f32 - 40.0 * e);
        p.texto_ajustado(
            &vacio,
            rejilla.x as f32 + (rejilla.ancho as f32 - w) / 2.0,
            rejilla.y as f32 + 60.0 * e,
            14.0 * e,
            rejilla.ancho as f32 - 40.0 * e,
            tema.apagado,
        );
    }
    p.soltar_recorte();
    *g.visibles.borrow_mut() = visibles;
    g.alto_total
        .set((y + g.scroll - rejilla.y - rejilla.alto as i32 + ei(MARGEN) as i32).max(0));

    // El previsualizador, a la derecha.
    let previa = Rect {
        x: rejilla.derecha(),
        y: barra.abajo(),
        ancho: previa_ancho,
        alto: lista.alto,
    };
    p.rellenar(rf(previa), tema.lista);
    if let Some(f) = g.elegida.and_then(|i| g.figuras.get(i)) {
        pintar_previa(p, f, previa, tema, e, textos, g, &mut zonas);
    }
}

#[allow(clippy::too_many_arguments)]
fn pintar_previa(
    p: &Pintor,
    f: &Vista,
    caja: Rect,
    tema: &Tema,
    e: f32,
    textos: &Catalogo,
    g: &Galeria,
    zonas: &mut Vec<(Rect, Zona)>,
) {
    let m = 18.0 * e;
    let x = caja.x as f32 + m;
    let ancho = caja.ancho as f32 - 2.0 * m;
    let papel = RectF {
        x,
        y: caja.y as f32 + m,
        ancho,
        alto: ancho * 0.85,
    };
    p.rellenar_redondeado(papel, 12.0 * e, tema.papel);
    if let Some(d) = &f.dibujo {
        pintar_lienzo(
            p,
            d,
            RectF {
                x: papel.x + 14.0 * e,
                y: papel.y + 14.0 * e,
                ancho: papel.ancho - 28.0 * e,
                alto: papel.alto - 28.0 * e,
            },
            None,
        );
    }
    let mut y = papel.y + papel.alto + 16.0 * e;
    let (_, h) = p.medir_texto_ajustado(&f.nombre, 17.0 * e, ancho);
    p.texto_ajustado(&f.nombre, x, y, 17.0 * e, ancho, tema.texto);
    y += h + 4.0 * e;
    let grupo = rotulo_del_grupo(&f.grupo, textos);
    let origen = if f.de_serie {
        textos.t("figuras-de-serie")
    } else {
        textos.t("figuras-tuya")
    };
    p.texto_linea(
        &format!("{grupo} · {origen}"),
        x,
        y,
        12.5 * e,
        ancho,
        tema.apagado,
    );
    y += 26.0 * e;
    let datos = [
        (
            textos.t("figuras-mide"),
            format!("{:.0} × {:.0}", f.tamano.0, f.tamano.1),
        ),
        (
            textos.t("figuras-piezas"),
            resumen_de_piezas(&f.piezas, textos),
        ),
    ];
    for (rotulo, valor) in datos {
        p.texto(&rotulo, x, y, 12.5 * e, tema.apagado);
        let (_, vh) = p.medir_texto_ajustado(&valor, 12.5 * e, ancho - 90.0 * e);
        p.texto_ajustado(
            &valor,
            x + 90.0 * e,
            y,
            12.5 * e,
            ancho - 90.0 * e,
            tema.texto,
        );
        y += vh.max(16.0 * e) + 6.0 * e;
    }
    if !f.letras.is_empty() {
        y += 4.0 * e;
        p.texto(&textos.t("figuras-textos"), x, y, 12.5 * e, tema.apagado);
        y += 20.0 * e;
        let tope = caja.abajo() as f32 - 70.0 * e;
        for l in &f.letras {
            if y + 16.0 * e > tope {
                p.texto("…", x, y, 12.5 * e, tema.apagado);
                break;
            }
            p.texto_linea(&format!("• {l}"), x, y, 12.5 * e, ancho, tema.texto);
            y += 18.0 * e;
        }
    }
    if !f.de_serie {
        let boton = Rect {
            x: x as i32,
            y: (caja.abajo() as f32 - m - 38.0 * e) as i32,
            ancho: ancho as u32,
            alto: (38.0 * e) as u32,
        };
        let rojo = crate::caja_dibujo::hex(0xd9534f);
        p.rellenar_redondeado(
            rf(boton),
            10.0 * e,
            if g.borrar_pedido || boton.contiene(g.raton) {
                rojo
            } else {
                con_alfa(rojo, 0.75)
            },
        );
        let rotulo = if g.borrar_pedido {
            textos.t("figuras-borrar-seguro")
        } else {
            textos.t("figuras-borrar")
        };
        let (tw, th) = p.medir_texto(&rotulo, 14.0 * e);
        p.texto(
            &rotulo,
            boton.x as f32 + (boton.ancho as f32 - tw) / 2.0,
            boton.y as f32 + (boton.alto as f32 - th) / 2.0,
            14.0 * e,
            Color::BLANCO,
        );
        zonas.push((boton, Zona::Borrar));
    }
}

/// «23 · 8 líneas, 6 textos, 4 formas».
fn resumen_de_piezas(p: &Piezas, textos: &Catalogo) -> String {
    let mut partes = Vec::new();
    for (n, clave) in [
        (p.lineas, "figuras-lineas"),
        (p.formas, "figuras-formas"),
        (p.textos, "figuras-de-texto"),
        (p.trazos, "figuras-trazos"),
        (p.otras, "figuras-otras"),
    ] {
        if n > 0 {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("n", n as i64);
            partes.push(textos.t_args(clave, &args));
        }
    }
    if partes.is_empty() {
        p.total.to_string()
    } else {
        format!("{} · {}", p.total, partes.join(", "))
    }
}

fn pintar_celda(
    p: &Pintor,
    f: &Vista,
    celda: Rect,
    tema: &Tema,
    e: f32,
    encima: bool,
    elegida: bool,
) {
    let dibujo = RectF {
        x: celda.x as f32,
        y: celda.y as f32,
        ancho: celda.ancho as f32,
        alto: CELDA_DIBUJO as f32 * e,
    };
    if encima || elegida {
        p.rellenar_redondeado(
            RectF {
                x: dibujo.x - 5.0 * e,
                y: dibujo.y - 5.0 * e,
                ancho: dibujo.ancho + 10.0 * e,
                alto: celda.alto as f32 + 6.0 * e,
            },
            12.0 * e,
            if elegida {
                tema.fila_elegida
            } else {
                tema.fila_sobre
            },
        );
    }
    p.rellenar_redondeado(dibujo, 10.0 * e, tema.papel);
    if let Some(d) = &f.dibujo {
        pintar_lienzo(
            p,
            d,
            RectF {
                x: dibujo.x + 9.0 * e,
                y: dibujo.y + 9.0 * e,
                ancho: dibujo.ancho - 18.0 * e,
                alto: dibujo.alto - 18.0 * e,
            },
            None,
        );
    }
    p.texto_linea(
        &f.nombre,
        dibujo.x + 2.0 * e,
        dibujo.y + dibujo.alto + 7.0 * e,
        12.5 * e,
        dibujo.ancho - 4.0 * e,
        if elegida {
            tema.texto_elegido
        } else {
            tema.texto
        },
    );
}

// --- Adjuntar figuras al asistente (25-oct-2026) --------------------------------
//
// El usuario: «en la opcion de adjuntar del asistente … puedo adjuntar una
// figura para que la mejore, o decirle hazme una figura, y lo puedo anadir
// facilmente a la lista de figuras».

/// Las figuras de la biblioteca, en el orden de `biblioteca::cargar` (los
/// indices del menu de adjuntar apuntan aqui).
pub(super) fn todas() -> Vec<FiguraGuardada> {
    biblioteca::cargar(&crate::biblioteca_figuras::ruta())
}

/// Los grupos para el menu de adjuntar: `(grupo, [(indice, nombre)])`, lo del
/// usuario primero y las categorias en su orden.
pub(super) fn por_grupos(figuras: &[FiguraGuardada]) -> Vec<(String, Vec<(usize, String)>)> {
    let mut v: Vec<(String, Vec<(usize, String)>)> = Vec::new();
    for (i, f) in figuras.iter().enumerate() {
        let (g, n) = match categoria_de(&f.nombre) {
            Some((c, n)) => (c.to_string(), n.to_string()),
            None => (MIS_FIGURAS.to_string(), f.nombre.clone()),
        };
        match v.iter_mut().find(|(x, _)| *x == g) {
            Some((_, l)) => l.push((i, n)),
            None => v.push((g, vec![(i, n)])),
        }
    }
    v.sort_by_key(|(g, _)| orden_del_grupo(g));
    v
}

/// El nombre de un grupo en el menu.
pub(super) fn grupo_legible(g: &str, textos: &Catalogo) -> String {
    rotulo_del_grupo(g, textos)
}

/// **La escena de un lienzo con la figura dentro**, para adjuntarla al chat.
pub(super) fn escena_con(f: &FiguraGuardada) -> pixpin_motor2d::Escena {
    let mut escena = pixpin_motor2d::Escena::nueva();
    let (w, h) = f.tamano();
    biblioteca::estampar_en_escena(
        &mut escena,
        f,
        pixpin_motor2d::vector::Punto2::nuevo(w / 2.0, h / 2.0),
    );
    escena
}

/// **Guarda un lienzo del chat como figura** con `nombre` (la del mismo
/// nombre se sustituye). Lo que se guarda es lo que se ve en el lienzo, con
/// sus piezas tal cual.
pub(super) fn guardar_lienzo(escena: &pixpin_motor2d::Escena, nombre: &str) -> Result<(), String> {
    let piezas: Vec<pixpin_motor2d::Elemento> = escena.visibles().cloned().collect();
    if piezas.is_empty() {
        return Err("vacio".into());
    }
    let figura = FiguraGuardada {
        id: biblioteca::id_nuevo(),
        nombre: nombre.trim().chars().take(80).collect(),
        elementos: biblioteca::normalizar(&piezas),
    };
    if crate::biblioteca_figuras::anadir(&crate::biblioteca_figuras::ruta(), figura) {
        Ok(())
    } else {
        Err("no se pudo escribir".into())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_menu_de_adjuntar_agrupa_y_la_figura_entra_entera_en_su_lienzo() {
        let catalogo = pixpin_motor2d::figuras_de_serie::catalogo();
        let mut todas = vec![FiguraGuardada {
            id: "mia".into(),
            nombre: "Mi árbol".into(),
            elementos: catalogo[0].elementos.clone(),
        }];
        todas.extend(catalogo.iter().cloned());
        let g = por_grupos(&todas);
        assert_eq!(g[0].0, MIS_FIGURAS, "lo del usuario primero");
        assert_eq!(g[0].1, vec![(0, "Mi árbol".to_string())]);
        let total: usize = g.iter().map(|(_, l)| l.len()).sum();
        assert_eq!(total, todas.len(), "cada figura sale una vez");
        // La figura estampada es la misma, pieza a pieza.
        let e = escena_con(&catalogo[0]);
        assert_eq!(
            e.visibles().count(),
            catalogo[0].elementos.iter().filter(|x| !x.borrado).count()
        );
        let c = e.caja().unwrap();
        assert!(c.0 >= -0.5 && c.1 >= -0.5, "desde el origen: {c:?}");
    }

    #[test]
    fn lo_del_usuario_va_primero_y_las_categorias_en_su_orden() {
        assert_eq!(orden_del_grupo(MIS_FIGURAS), 0);
        assert!(orden_del_grupo(CATEGORIAS[0]) < orden_del_grupo(CATEGORIAS[1]));
        // Caso negativo: una categoria inventada no se cuela delante de las
        // de serie.
        assert!(
            orden_del_grupo("Mi coleccion") > orden_del_grupo(CATEGORIAS[CATEGORIAS.len() - 1])
        );
    }

    #[test]
    fn el_grupo_sale_del_nombre_y_se_busca_sin_tildes() {
        let f = FiguraGuardada {
            id: "x".into(),
            nombre: "Diario · Gratitud".into(),
            elementos: Vec::new(),
        };
        let v = vista_de(&f);
        assert_eq!(
            (v.grupo.as_str(), v.nombre.as_str()),
            ("Diario", "Gratitud")
        );
        assert!(v.dibujo.is_none(), "sin piezas no hay nada que dibujar");
        assert!(casa(&v, "GRATI") && casa(&v, "diario") && casa(&v, ""));
        assert!(!casa(&v, "árbol"), "lo que no esta no casa");
        let mia = vista_de(&FiguraGuardada {
            nombre: "Mi árbol".into(),
            ..f
        });
        assert_eq!(mia.grupo, MIS_FIGURAS);
        assert!(!mia.de_serie);
        assert!(casa(&mia, "arbol"), "sin tildes tambien");
    }

    #[test]
    fn el_previsualizador_cuenta_las_piezas_y_lee_los_textos() {
        let catalogo = pixpin_motor2d::figuras_de_serie::catalogo();
        let con_textos = catalogo
            .iter()
            .find(|f| piezas_de(f).textos > 1)
            .expect("alguna de serie lleva textos");
        let p = piezas_de(con_textos);
        assert_eq!(
            p.total,
            p.trazos + p.lineas + p.formas + p.textos + p.otras,
            "cada pieza cuenta una vez"
        );
        assert!(!letras_de(con_textos).is_empty());
        // Caso negativo: una figura sin textos no lista ninguno.
        let sin = FiguraGuardada {
            id: "y".into(),
            nombre: "Nada".into(),
            elementos: Vec::new(),
        };
        assert!(letras_de(&sin).is_empty());
        assert_eq!(piezas_de(&sin), Piezas::default());
    }
}
