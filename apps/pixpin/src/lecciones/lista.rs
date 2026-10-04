//! **Las lecciones aprendidas**: tarjetas, buscador y repaso
//! (`LeccionesActivity.kt`).
//!
//! Lo que hay en esta ventana responde a por que fracasan las bases de
//! lecciones: nadie las consulta porque **no aparecen cuando hacen falta**, y
//! se apunta lo mismo una y otra vez sin que cambie nada. Por eso, ademas de
//! la lista y del buscador (sin acentos, con erratas, por concepto):
//!
//! - **Para repasar hoy**: de una en una, primero intentando recordar que
//!   harias —recordar fija mucho mas que releer— y espaciadas cada vez mas
//!   si se recuerdan («Lo recordaba» / «Lo olvidé»).
//! - **Lista de comprobacion**: todo lo de «la proxima vez…» como casillas,
//!   para mirarla **antes** de empezar algo. Las casillas no se guardan.
//! - Las que **se repiten** se marcan (🔁×n) y suben.
//! - Con un proyecto, primero las suyas y las que hablan de lo mismo que su
//!   nombre.
//!
//! Abajo, «Dictar» y «Nueva», como la barra del movil. Una tarjeta abre su
//! ficha para verla o cambiarla; al pasar el raton sale la papelera, que
//! pide un segundo clic (en el movil se borra desde la ficha; aqui tambien).
//!
//! Una sola ventana: pedirla otra vez la trae delante con el proyecto o la
//! busqueda nuevos. Se pone al dia sola cuando se guarda una leccion en otra
//! ventana o llega algo sincronizando.

#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_geom::Rect;
use pixpin_lecciones::buscador::{self, Indice};
use pixpin_lecciones::leccion::{AREAS, TIPO_ACIERTO};
use pixpin_lecciones::{Leccion, Repaso};
use pixpin_render::icono::material as mi;
use pixpin_render::{Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Idioma, Ubicacion};

use super::almacen::{self, Donde, Entrada};
use super::dictar::{Dictado, Salida};
use super::ficha;
use super::ui::{self, Campo, NARANJA, PUESTO, VK_ESCAPE, color_de, con_alfa};
use crate::caja_dibujo::hex;
use crate::overlay::Recursos;
use crate::ventanita::{APAGADO, Botones, CRISTAL, FONDO, ROJO, TEXTO, centrado, dentro};

#[derive(Debug, Clone)]
pub struct Pedido {
    pub ubicacion: Ubicacion,
    pub idioma: Idioma,
    pub aparato: String,
    /// La ficha de un proyecto: se ven primero las suyas.
    pub proyecto: Option<String>,
    pub consulta: Option<String>,
}

/// La ventana abierta (su HWND), -1 mientras nace, 0 sin ventana.
static ABIERTA: AtomicIsize = AtomicIsize::new(0);
/// Lo pedido mientras ya estaba abierta: lo recoge su bucle.
static PENDIENTE: Mutex<Option<Pedido>> = Mutex::new(None);

/// Abre la lista, o la trae delante con lo pedido si ya estaba.
pub fn abrir(pedido: Pedido) {
    let ya = ABIERTA.load(Ordering::SeqCst);
    if ya > 0 {
        *PENDIENTE.lock().unwrap_or_else(|e| e.into_inner()) = Some(pedido);
        VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(ya as *mut _));
        pixpin_shell::overlay::despertar(ya);
        return;
    }
    if ya < 0 {
        return;
    }
    ABIERTA.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new()
        .name("lecciones".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let hecho = Recursos::nuevos().and_then(|r| bucle(&r, pedido));
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la lista de lecciones");
            }
            ABIERTA.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de las lecciones");
        ABIERTA.store(0, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filtro {
    Errores,
    Repetidas,
    Graves,
    Aciertos,
}

const FILTROS: [(Filtro, &str); 4] = [
    (Filtro::Errores, "⚠️ Errores"),
    (Filtro::Repetidas, "🔁 Repetidas"),
    (Filtro::Graves, "❗ Graves"),
    (Filtro::Aciertos, "✅ Aciertos"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Mover,
    Cerrar,
    Lista,
    BorrarConsulta,
    DictarBusqueda,
    QuitarProyecto,
    Todas,
    Area(usize),
    Filtro(usize),
    Abrir(usize),
    Borrar(usize),
    Destapar,
    Recordaba,
    Olvide,
    Marcar(usize),
    Dictar,
    Nueva,
    Fondo,
}

struct Estado {
    pedido: Pedido,
    todas: Vec<Entrada>,
    indices: Vec<Indice>,
    firma: Vec<Option<std::time::SystemTime>>,
    cambios: u64,
    consulta: Campo,
    area: Option<String>,
    filtro: Option<Filtro>,
    proyecto: Option<String>,
    nombres: HashMap<String, String>,
    en_lista: bool,
    visibles: Vec<usize>,
    mirado: Option<(String, Option<String>, Option<Filtro>, Option<String>, usize)>,
    hoy: Vec<Leccion>,
    destapada: bool,
    marcadas: HashSet<String>,
    confirmar: Option<usize>,
    scroll: f32,
    alto_contenido: f32,
    botones: Botones<Accion>,
    aviso: Option<(String, Instant)>,
    dictado: Dictado,
    moviendo: Option<(pixpin_geom::Punto, Rect)>,
    /// Lo que mide cada tarjeta, por id, fecha y ancho: medir texto cuesta.
    altos: HashMap<(String, i64, u32), f32>,
    /// Donde empieza el contenido (bajo las fichas), del ultimo fotograma.
    arriba: f32,
    /// Si el ultimo fotograma movio `arriba` y hay que pintar otra vez.
    repintar: bool,
}

impl Estado {
    fn recargar(&mut self) {
        let raiz = self.pedido.ubicacion.raiz().to_path_buf();
        self.todas = almacen::listar(&raiz);
        self.indices = self.todas.iter().map(|e| Indice::nuevo(e.leccion.clone())).collect();
        self.firma = almacen::firma(&raiz, &self.todas);
        self.cambios = almacen::cambios();
        self.nombres = pixpin_proyecto::almacen::Indice::leer(&raiz)
            .proyectos
            .into_iter()
            .map(|f| (f.id, f.nombre))
            .collect();
        self.hoy = Repaso::de_hoy(
            &self.todas.iter().map(|e| e.leccion.clone()).collect::<Vec<_>>(),
            pixpin_shell::entorno::ahora_utc_ms(),
            5,
        );
        self.destapada = false;
        self.confirmar = None;
        self.mirado = None;
        self.filtrar();
    }

    fn areas(&self) -> Vec<String> {
        let mut v: Vec<String> = AREAS.iter().map(|a| a.to_string()).collect();
        for e in &self.todas {
            if !e.leccion.area.trim().is_empty() && !v.contains(&e.leccion.area) {
                v.push(e.leccion.area.clone());
            }
        }
        v
    }

    /// Lo que se ve con el filtro y la busqueda puestos.
    fn filtrar(&mut self) {
        let clave = (
            self.consulta.texto.clone(),
            self.area.clone(),
            self.filtro,
            self.proyecto.clone(),
            self.todas.len(),
        );
        if self.mirado.as_ref() == Some(&clave) {
            return;
        }
        let mut base: Vec<usize> = (0..self.todas.len())
            .filter(|i| {
                let l = &self.todas[*i].leccion;
                self.area.as_ref().is_none_or(|a| l.area == *a)
                    && match self.filtro {
                        None => true,
                        Some(Filtro::Errores) => l.es_error(),
                        Some(Filtro::Repetidas) => !l.repeticiones.is_empty(),
                        Some(Filtro::Graves) => l.gravedad >= 3,
                        Some(Filtro::Aciertos) => l.tipo == TIPO_ACIERTO,
                    }
            })
            .collect();
        if let Some(p) = &self.proyecto {
            // Las del proyecto y las que hablan de lo mismo que su nombre.
            let nombre = self.nombres.get(p).cloned().unwrap_or_default();
            let ix: Vec<Indice> = base.iter().map(|i| self.indices[*i].clone()).collect();
            let cerca: HashSet<String> = buscador::para_el_contexto(&ix, &nombre, 10).into_iter().map(|l| l.id).collect();
            let mut v: Vec<usize> = base.iter().copied().filter(|i| self.todas[*i].ficha == *p).collect();
            for i in base {
                if cerca.contains(&self.todas[i].leccion.id) && !v.contains(&i) {
                    v.push(i);
                }
            }
            base = v;
        }
        if !self.consulta.texto.trim().is_empty() {
            let ix: Vec<Indice> = base.iter().map(|i| self.indices[*i].clone()).collect();
            let por_id: HashMap<String, usize> = base.iter().map(|i| (self.todas[*i].leccion.id.clone(), *i)).collect();
            base = buscador::buscar(&ix, &self.consulta.texto)
                .into_iter()
                .filter_map(|r| por_id.get(&r.leccion.id).copied())
                .collect();
        }
        self.visibles = base;
        self.scroll = 0.0;
        self.mirado = Some(clave);
    }

    fn sin_filtros(&self) -> bool {
        self.consulta.texto.trim().is_empty() && self.area.is_none() && self.filtro.is_none() && self.proyecto.is_none()
    }

    fn aplicar(&mut self, p: Pedido) {
        self.proyecto = p.proyecto.clone();
        if let Some(c) = &p.consulta {
            self.consulta.poner(c);
        }
        self.pedido = p;
        self.mirado = None;
        self.filtrar();
    }

    fn ficha(&self, que: ficha::Que) -> ficha::Pedido {
        ficha::Pedido {
            ubicacion: self.pedido.ubicacion.clone(),
            idioma: self.pedido.idioma,
            aparato: self.pedido.aparato.clone(),
            que,
        }
    }
}

fn bucle(recursos: &Recursos, pedido: Pedido) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = pixpin_shell::pantalla_de::monitor_de_la_ventana_activa()
        .and_then(|r| monitores.monitores().iter().find(|m| m.area == r))
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let mut marco = centrado(monitor.area_trabajo, 1000, 760, monitor.escala_por_cien);
    let mut ventana = VentanaOverlay::nueva_normal(marco, "Lecciones").context("no se pudo abrir la lista de lecciones")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(&motor, &recursos.d3d(), ventana.handle(), marco.ancho, marco.alto)
        .context("sin superficie para las lecciones")?;
    ventana.mostrar();
    ventana.enfocar();
    let hwnd = ventana.handle().0 as isize;
    ABIERTA.store(hwnd, Ordering::SeqCst);
    almacen::apuntar_ventana(hwnd);

    let consulta = Campo::con(pedido.consulta.as_deref().unwrap_or(""));
    let mut e = Estado {
        proyecto: pedido.proyecto.clone(),
        pedido,
        todas: Vec::new(),
        indices: Vec::new(),
        firma: Vec::new(),
        cambios: 0,
        consulta,
        area: None,
        filtro: None,
        nombres: HashMap::new(),
        en_lista: false,
        visibles: Vec::new(),
        mirado: None,
        hoy: Vec::new(),
        destapada: false,
        marcadas: HashSet::new(),
        confirmar: None,
        scroll: 0.0,
        alto_contenido: 0.0,
        botones: Botones::default(),
        aviso: None,
        dictado: Dictado::Nada,
        moviendo: None,
        altos: HashMap::new(),
        arriba: 0.0,
        repintar: false,
    };
    e.recargar();
    tracing::info!(lecciones = e.todas.len(), "lista de lecciones abierta");
    let mut mirado = Instant::now();
    let mut vivo = true;
    let mut pintar = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        for (h, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if h != ventana.handle() {
                continue;
            }
            pintar = true;
            let local = |p: pixpin_geom::Punto, m: Rect| ((p.x - m.x) as f32, (p.y - m.y) as f32);
            match ev {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    if let Some((desde, origen)) = e.moviendo {
                        marco = Rect {
                            x: origen.x + (p.x - desde.x),
                            y: origen.y + (p.y - desde.y),
                            ..origen
                        };
                        ventana.mover(marco);
                    }
                    e.botones.raton = local(p, marco);
                }
                EventoOverlay::BotonPulsado(p) => {
                    e.botones.raton = local(p, marco);
                    match e.botones.bajo_el_raton() {
                        Some(Accion::Mover) => {
                            e.moviendo = Some((p, marco));
                            ventana.capturar_raton();
                        }
                        Some(a) => vivo = hacer(&mut e, a),
                        None => {}
                    }
                }
                EventoOverlay::BotonSoltado(_) => {
                    if e.moviendo.take().is_some() {
                        ventana.soltar_raton();
                    }
                }
                EventoOverlay::Rueda(m) => e.scroll -= m as f32 * 70.0 * escala,
                // Lo que se escribe va siempre al buscador.
                EventoOverlay::Caracter(c) => {
                    if e.consulta.letra(c) {
                        e.filtrar();
                    }
                }
                EventoOverlay::Tecla { vk: VK_ESCAPE, .. } => {
                    if e.consulta.texto.is_empty() {
                        vivo = false;
                    } else {
                        e.consulta.poner("");
                        e.filtrar();
                    }
                }
                EventoOverlay::Tecla { vk, ctrl, shift, .. } => {
                    if e.consulta.tecla(vk, ctrl, shift, false) {
                        e.filtrar();
                    }
                }
                _ => {}
            }
        }
        if !vivo {
            break;
        }
        if let Some(p) = PENDIENTE.lock().unwrap_or_else(|x| x.into_inner()).take() {
            e.aplicar(p);
            pintar = true;
        }
        // Lo guardado en otra ventana, al momento; lo que llega sincronizando,
        // mirando las fechas cada dos segundos.
        if almacen::cambios() != e.cambios {
            e.recargar();
            pintar = true;
        } else if mirado.elapsed() >= Duration::from_secs(2) {
            mirado = Instant::now();
            if almacen::firma(e.pedido.ubicacion.raiz(), &e.todas) != e.firma {
                e.recargar();
                pintar = true;
            }
        }
        match e.dictado.avanzar(&e.pedido.ubicacion.clone(), e.pedido.idioma) {
            Salida::Nada => {}
            Salida::Texto(t) => {
                e.consulta.poner(&t);
                e.filtrar();
                pintar = true;
            }
            Salida::Fallo(m) => {
                e.aviso = Some((m, Instant::now()));
                pintar = true;
            }
        }
        if e.dictado.activo() {
            pintar = true;
        }
        if e.aviso.as_ref().is_some_and(|(_, t)| t.elapsed() > Duration::from_millis(3_000)) {
            e.aviso = None;
            pintar = true;
        }
        if pintar {
            // Dos como mucho: el segundo si las fichas acabaron en otro sitio.
            for _ in 0..2 {
                e.repintar = false;
                if let Ok(d) = superficie.empezar(&motor) {
                    let _ = motor.dibujar(&d, |p: &Pintor| pintar_todo(&mut e, p, marco, escala));
                    let _ = superficie.presentar();
                }
                if !e.repintar {
                    break;
                }
            }
            pintar = false;
        }
        let espera = if e.dictado.activo() { 120 } else if e.aviso.is_some() { 250 } else { 1_000 };
        pixpin_shell::overlay::esperar_eventos(Some(espera));
    }
    almacen::quitar_ventana(hwnd);
    tracing::info!("lista de lecciones cerrada");
    Ok(())
}

/// Lo que hace un clic. Devuelve si la ventana sigue.
fn hacer(e: &mut Estado, a: Accion) -> bool {
    let raiz = e.pedido.ubicacion.raiz().to_path_buf();
    if !matches!(a, Accion::Borrar(_)) {
        e.confirmar = None;
    }
    match a {
        Accion::Mover | Accion::Fondo => {}
        Accion::Cerrar => return false,
        Accion::Lista => e.en_lista = !e.en_lista,
        Accion::BorrarConsulta => e.consulta.poner(""),
        Accion::DictarBusqueda => {
            let (u, i) = (e.pedido.ubicacion.clone(), e.pedido.idioma);
            if let Some(m) = e.dictado.pulsar(&u, i) {
                e.aviso = Some((m, Instant::now()));
            }
        }
        Accion::QuitarProyecto => e.proyecto = None,
        Accion::Todas => {
            e.area = None;
            e.filtro = None;
        }
        Accion::Area(i) => {
            let a = e.areas().get(i).cloned();
            e.area = if e.area == a { None } else { a };
        }
        Accion::Filtro(i) => {
            let f = Some(FILTROS[i].0);
            e.filtro = if e.filtro == f { None } else { f };
        }
        Accion::Abrir(i) => {
            if let Some(x) = e.todas.get(i) {
                ficha::abrir(e.ficha(ficha::Que::Editar { id: x.leccion.id.clone() }));
            }
        }
        Accion::Borrar(i) => {
            if e.confirmar == Some(i) {
                if let Some(x) = e.todas.get(i).cloned() {
                    match almacen::borrar(&raiz, &x) {
                        Ok(()) => e.aviso = Some(("Lección borrada".into(), Instant::now())),
                        Err(err) => tracing::warn!(?err, "no se pudo borrar la leccion"),
                    }
                }
                e.confirmar = None;
            } else {
                e.confirmar = Some(i);
            }
        }
        Accion::Destapar => e.destapada = true,
        Accion::Recordaba | Accion::Olvide => {
            if let Some(l) = e.hoy.first().cloned()
                && let Some(x) = e.todas.iter().find(|x| x.leccion.id == l.id).cloned()
            {
                let ahora = pixpin_shell::entorno::ahora_utc_ms();
                let nueva = if a == Accion::Recordaba { Repaso::recordada(&l, ahora) } else { Repaso::olvidada(&l, ahora) };
                if let Err(err) = almacen::guardar(&raiz, &nueva, &Donde::de(&x), &e.pedido.aparato) {
                    tracing::warn!(?err, "no se pudo guardar el repaso");
                }
            }
        }
        Accion::Marcar(i) => {
            if let Some(x) = e.todas.get(i) {
                let id = x.leccion.id.clone();
                if !e.marcadas.remove(&id) {
                    e.marcadas.insert(id);
                }
            }
        }
        Accion::Dictar | Accion::Nueva => {
            let que = ficha::Que::Nueva {
                texto: None,
                de_mensaje: None,
                ficha: e.proyecto.clone(),
                dictar: a == Accion::Dictar,
                fotos: Vec::new(),
            };
            ficha::abrir(e.ficha(que));
        }
    }
    e.filtrar();
    true
}

const CABECERA: f32 = 64.0;
const BARRA: f32 = 80.0;

fn pintar_todo(e: &mut Estado, p: &Pintor, marco: Rect, s: f32) {
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    p.limpiar(FONDO);
    e.botones.vaciar();
    e.botones.zona(RectF { x: 0.0, y: 0.0, ancho: w, alto: h }, Accion::Fondo);
    let m = 16.0 * s;
    let ancho = w - 2.0 * m;

    // El contenido, desplazable, ANTES que lo de arriba: asi la cabecera, el
    // buscador y las fichas se apuntan despues y ganan a una tarjeta que
    // asome por debajo. Empieza donde acabaron las fichas en el fotograma
    // anterior; si este acaban en otro sitio, se pide otro.
    let y = e.arriba;
    let abajo = h - BARRA * s;
    let area = RectF { x: 0.0, y, ancho: w, alto: (abajo - y).max(0.0) };
    let visible = area.alto;
    e.scroll = e.scroll.clamp(0.0, (e.alto_contenido - visible).max(0.0));
    let mut total = 0.0;
    p.con_recorte(area, |p| {
        let y0 = y - e.scroll;
        let fin = if e.todas.is_empty() {
            pintar_vacia(p, m, y0, ancho, s)
        } else if e.en_lista {
            pintar_lista(e, p, m, y0, ancho, area, s)
        } else {
            pintar_tarjetas(e, p, m, y0, ancho, area, s)
        };
        total = fin - y0 + 20.0 * s;
    });
    e.alto_contenido = total;

    // Lo de arriba, tapando lo que se desplazo bajo ello.
    p.rellenar(RectF { x: 0.0, y: 0.0, ancho: w, alto: e.arriba }, FONDO);

    // Cabecera.
    let cab = RectF { x: 0.0, y: 0.0, ancho: w, alto: CABECERA * s };
    e.botones.zona(cab, Accion::Mover);
    p.texto("Lecciones", m, 10.0 * s, 20.0 * s, TEXTO);
    let repetidas = e.todas.iter().filter(|x| !x.leccion.repeticiones.is_empty()).count();
    let sub = if repetidas > 0 {
        format!("{} aprendidas · {repetidas} se repitieron", e.todas.len())
    } else {
        format!("{} aprendidas", e.todas.len())
    };
    p.texto(&sub, m, 38.0 * s, 12.5 * s, APAGADO);
    let lado = 40.0 * s;
    let cerrar = RectF { x: w - m - lado, y: 12.0 * s, ancho: lado, alto: lado };
    e.botones.boton(p, cerrar, Accion::Cerrar, "", None, s);
    p.icono(&mi::CLOSE, ui::encoger(cerrar, 9.0 * s), TEXTO);
    let lista = RectF { x: cerrar.x - lado - 8.0 * s, ..cerrar };
    e.botones.boton(p, lista, Accion::Lista, "", if e.en_lista { Some(ui::PUESTO_FONDO) } else { None }, s);
    p.icono(&mi::CHECKLIST, ui::encoger(lista, 9.0 * s), if e.en_lista { PUESTO } else { TEXTO });

    // Buscador.
    let mut y = CABECERA * s;
    let caja = RectF { x: m, y, ancho, alto: 48.0 * s };
    p.rellenar_redondeado(caja, 24.0 * s, CRISTAL);
    p.icono(&mi::SEARCH, RectF { x: m + 14.0 * s, y: y + 12.0 * s, ancho: 24.0 * s, alto: 24.0 * s }, APAGADO);
    let tx = m + 48.0 * s;
    let tam = 15.0 * s;
    // El texto sin caja propia: el fondo es el de la pastilla. Siempre con
    // el foco: lo que se escribe en esta ventana va al buscador.
    let campo = RectF { x: tx, y: y + 2.0 * s, ancho: ancho - 150.0 * s, alto: 44.0 * s };
    p.con_recorte(campo, |p| {
        e.consulta.pintar_texto(p, tx, y + 13.0 * s, 10_000.0, tam, true, "Buscar: palabra, tema, situación…", s);
    });
    let mut bx = m + ancho - 44.0 * s;
    let mic = RectF { x: bx, y: y + 4.0 * s, ancho: 40.0 * s, alto: 40.0 * s };
    let grabando = matches!(e.dictado, Dictado::Grabando(_));
    if grabando {
        p.circulo((mic.x + 20.0 * s, mic.y + 20.0 * s), 18.0 * s, ROJO);
    }
    p.icono(&mi::MIC, ui::encoger(mic, 9.0 * s), TEXTO);
    e.botones.zona(mic, Accion::DictarBusqueda);
    if !e.consulta.texto.is_empty() {
        bx -= 42.0 * s;
        let x = RectF { x: bx, y: y + 4.0 * s, ancho: 40.0 * s, alto: 40.0 * s };
        p.icono(&mi::CLOSE, ui::encoger(x, 10.0 * s), APAGADO);
        e.botones.zona(x, Accion::BorrarConsulta);
    }
    y += 56.0 * s;
    if let Some(estado) = e.dictado.estado() {
        p.texto_color(&estado, m, y, 13.0 * s, if grabando { ROJO } else { APAGADO });
        y += 22.0 * s;
    }

    // Areas y filtros.
    let mut fichas: Vec<(String, Accion, bool, bool)> = Vec::new();
    let mut puntos = Vec::new();
    if let Some(pr) = &e.proyecto {
        let nombre = e.nombres.get(pr).cloned().unwrap_or_else(|| pr.clone());
        fichas.push((format!("📁 {nombre}"), Accion::QuitarProyecto, true, true));
        puntos.push(None);
    }
    fichas.push(("Todas".into(), Accion::Todas, e.area.is_none() && e.filtro.is_none(), false));
    puntos.push(None);
    for (i, a) in e.areas().iter().enumerate() {
        fichas.push((a.clone(), Accion::Area(i), e.area.as_deref() == Some(a.as_str()), false));
        puntos.push(Some(color_de(a)));
    }
    for (i, (f, n)) in FILTROS.iter().enumerate() {
        fichas.push((n.to_string(), Accion::Filtro(i), e.filtro == Some(*f), false));
    }
    y = ui::fila_con_puntos(p, &mut e.botones, m, y, ancho, &fichas, &puntos, 14.0 * s, s) + 12.0 * s;
    if (y - e.arriba).abs() > 0.5 {
        e.arriba = y;
        e.repintar = true;
    }
    let abajo = h - BARRA * s;

    // Apuntar: lo que mas se hace, abajo y a mano.
    let barra = RectF { x: 0.0, y: abajo, ancho: w, alto: h - abajo };
    p.rellenar(barra, FONDO);
    let bw = 150.0 * s;
    let bh = 48.0 * s;
    let by = abajo + (BARRA * s - bh) / 2.0;
    let dictar = RectF { x: w / 2.0 - bw - 6.0 * s, y: by, ancho: bw, alto: bh };
    let nueva = RectF { x: w / 2.0 + 6.0 * s, y: by, ancho: bw, alto: bh };
    ui::boton(p, &mut e.botones, dictar, Accion::Dictar, "🎙️ Dictar", hex(0x34343c), TEXTO, 15.0 * s, s);
    ui::boton(p, &mut e.botones, nueva, Accion::Nueva, "＋ Nueva", PUESTO, hex(0x1a1a1a), 15.0 * s, s);

    if let Some((t, _)) = &e.aviso {
        ui::aviso(p, t, w, h, s);
    }
}

fn pintar_vacia(p: &Pintor, x: f32, y: f32, ancho: f32, s: f32) -> f32 {
    let mut y = y + 60.0 * s;
    let (bw, _) = p.medir_texto("💡", 56.0 * s);
    p.texto_color("💡", x + (ancho - bw) / 2.0, y, 56.0 * s, TEXTO);
    y += 90.0 * s;
    let t = "Apunta lo que aprendes para no repetir errores";
    let (tw, _) = p.medir_texto(t, 17.0 * s);
    p.texto(t, x + (ancho - tw) / 2.0, y, 17.0 * s, TEXTO);
    y += 36.0 * s;
    let texto = "Pulsa Dictar y cuéntalo de corrido: «pasó que…, porque…, la próxima vez…». PixPin lo reparte, le pone etiquetas y te avisa si ya te pasó antes.";
    let anch = ancho.min(560.0 * s);
    let (_, th) = p.medir_texto_ajustado(texto, 14.0 * s, anch);
    p.texto_ajustado(texto, x + (ancho - anch) / 2.0, y, 14.0 * s, anch, APAGADO);
    y + th
}

/// **Repasar sin releer**: se ensena lo que paso (o la etiqueta) y se
/// intenta recordar que haria uno antes de destaparlo.
fn pintar_repaso(e: &mut Estado, p: &Pintor, x: f32, y: f32, ancho: f32, s: f32) -> f32 {
    let Some(l) = e.hoy.first().cloned() else {
        return y;
    };
    let pad = 14.0 * s;
    let interior = ancho - 2.0 * pad;
    let pista = if !l.que_paso.trim().is_empty() {
        l.que_paso.clone()
    } else {
        let t = l.todas_las_etiquetas().iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" ");
        if t.is_empty() { l.area.clone() } else { t }
    };
    let pregunta = if pista.trim().is_empty() { "Recuerda esta lección".to_string() } else { format!("Cuando {pista}…") };
    let respuesta = if l.proxima.trim().is_empty() { l.titulo.clone() } else { l.proxima.clone() };
    let (_, hp) = p.medir_texto_ajustado(&pregunta, 16.0 * s, interior);
    let (_, hr) = p.medir_texto_ajustado(&respuesta, 17.0 * s, interior);
    let alto = pad + 22.0 * s + hp + 12.0 * s + if e.destapada { hr + 10.0 * s } else { 0.0 } + 40.0 * s + pad;
    let caja = RectF { x, y, ancho, alto };
    p.rellenar_redondeado(caja, 18.0 * s, CRISTAL);
    let mut yy = y + pad;
    p.texto_color(&format!("🧠 Para repasar hoy · {}", e.hoy.len()), x + pad, yy, 13.5 * s, PUESTO);
    yy += 22.0 * s;
    p.texto_ajustado(&pregunta, x + pad, yy, 16.0 * s, interior, TEXTO);
    yy += hp + 12.0 * s;
    if !e.destapada {
        let b = RectF { x: x + pad, y: yy, ancho: 260.0 * s, alto: 40.0 * s };
        ui::boton(p, &mut e.botones, b, Accion::Destapar, "¿Qué harías? Ver respuesta", hex(0x3a3a44), TEXTO, 14.0 * s, s);
    } else {
        p.texto_ajustado(&respuesta, x + pad, yy, 17.0 * s, interior, PUESTO);
        yy += hr + 10.0 * s;
        let b1 = RectF { x: x + pad, y: yy, ancho: 160.0 * s, alto: 40.0 * s };
        let b2 = RectF { x: b1.x + 170.0 * s, ..b1 };
        ui::boton(p, &mut e.botones, b1, Accion::Recordaba, "Lo recordaba", PUESTO, hex(0x1a1a1a), 14.0 * s, s);
        ui::boton(p, &mut e.botones, b2, Accion::Olvide, "Lo olvidé", hex(0x3a3a44), TEXTO, 14.0 * s, s);
    }
    y + alto
}

const TOPE_TITULO: usize = 5;
const TOPE_PROXIMA: usize = 3;

/// Lo que mide una tarjeta (y su cache).
fn alto_de_tarjeta(e: &mut Estado, p: &Pintor, i: usize, ancho: f32, s: f32) -> f32 {
    let l = &e.todas[i].leccion;
    let clave = (l.id.clone(), l.tocada, ancho as u32);
    if let Some(h) = e.altos.get(&clave) {
        return *h;
    }
    let interior = ancho - 24.0 * s;
    let (_, linea_t) = p.medir_texto("Ag", 15.0 * s);
    let (_, ht) = p.medir_texto_ajustado(&l.titulo, 15.0 * s, interior);
    let mut h = 5.0 * s + 12.0 * s + 20.0 * s + 6.0 * s + ht.min(linea_t * TOPE_TITULO as f32);
    if !l.proxima.trim().is_empty() && l.proxima != l.titulo {
        let (_, linea_p) = p.medir_texto("Ag", 13.0 * s);
        let (_, hp) = p.medir_texto_ajustado(&format!("→ {}", l.proxima), 13.0 * s, interior);
        h += 6.0 * s + hp.min(linea_p * TOPE_PROXIMA as f32);
    }
    if !l.todas_las_etiquetas().is_empty() {
        h += 6.0 * s + 18.0 * s;
    }
    h += 12.0 * s;
    e.altos.insert(clave, h);
    h
}

/// Las tarjetas en columnas que se rellenan por la mas corta, como la
/// rejilla escalonada del movil. Solo se pinta lo que cae a la vista.
fn pintar_tarjetas(e: &mut Estado, p: &Pintor, x: f32, y: f32, ancho: f32, vista: RectF, s: f32) -> f32 {
    let mut y = y;
    if !e.hoy.is_empty() && e.sin_filtros() {
        y = pintar_repaso(e, p, x, y, ancho, s) + 12.0 * s;
    }
    if e.visibles.is_empty() {
        let t = if e.consulta.texto.trim().is_empty() {
            "Nada con este filtro.".to_string()
        } else {
            format!("Nada con «{}». Prueba con otra palabra o apúntala ahora.", e.consulta.texto.trim())
        };
        p.texto_ajustado(&t, x + 8.0 * s, y + 8.0 * s, 14.0 * s, ancho - 16.0 * s, APAGADO);
        return y + 40.0 * s;
    }
    let hueco = 10.0 * s;
    let columnas = (((ancho + hueco) / (230.0 * s + hueco)) as usize).max(1);
    let cw = (ancho - hueco * (columnas - 1) as f32) / columnas as f32;
    let mut alturas = vec![y; columnas];
    let visibles = e.visibles.clone();
    for i in visibles {
        let h = alto_de_tarjeta(e, p, i, cw, s);
        let (col, cy) = alturas
            .iter()
            .copied()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or((0, y));
        let caja = RectF { x: x + col as f32 * (cw + hueco), y: cy, ancho: cw, alto: h };
        if caja.y + caja.alto >= vista.y && caja.y <= vista.y + vista.alto {
            pintar_tarjeta(e, p, i, caja, s);
        }
        alturas[col] = cy + h + hueco;
    }
    alturas.into_iter().fold(y, f32::max)
}

fn pintar_tarjeta(e: &mut Estado, p: &Pintor, i: usize, caja: RectF, s: f32) {
    let l = e.todas[i].leccion.clone();
    let encima = dentro(caja, e.botones.raton);
    p.rellenar_redondeado(caja, 16.0 * s, if encima { hex(0x2e2e36) } else { CRISTAL });
    p.con_recorte(RectF { alto: 5.0 * s, ..caja }, |p| {
        p.rellenar_redondeado(RectF { alto: 20.0 * s, ..caja }, 16.0 * s, color_de(&l.area));
    });
    e.botones.zona(caja, Accion::Abrir(i));
    let pad = 12.0 * s;
    let interior = caja.ancho - 2.0 * pad;
    let mut y = caja.y + 5.0 * s + pad;
    p.texto_color(l.icono(), caja.x + pad, y, 14.0 * s, TEXTO);
    let area = if l.area.trim().is_empty() { "Sin área".to_string() } else { l.area.clone() };
    p.texto_linea(&area, caja.x + pad + 24.0 * s, y + 1.0 * s, 12.0 * s, interior - 90.0 * s, APAGADO);
    let mut dx = caja.x + caja.ancho - pad;
    if !l.repeticiones.is_empty() {
        let t = format!("🔁×{}", l.veces_que_paso());
        let (tw, _) = p.medir_texto(&t, 13.0 * s);
        dx -= tw;
        p.texto_color(&t, dx, y, 13.0 * s, NARANJA);
    }
    if !l.adjuntos.is_empty() {
        let t = format!("📎{}", l.adjuntos.len());
        let (tw, _) = p.medir_texto(&t, 13.0 * s);
        dx -= tw + 6.0 * s;
        p.texto_color(&t, dx, y, 13.0 * s, APAGADO);
    }
    y += 26.0 * s;
    let (_, linea_t) = p.medir_texto("Ag", 15.0 * s);
    let (_, ht) = p.medir_texto_ajustado(&l.titulo, 15.0 * s, interior);
    let ht = ht.min(linea_t * TOPE_TITULO as f32);
    p.con_recorte(RectF { x: caja.x, y, ancho: caja.ancho, alto: ht }, |p| {
        p.texto_ajustado(&l.titulo, caja.x + pad, y, 15.0 * s, interior, TEXTO);
    });
    y += ht;
    if !l.proxima.trim().is_empty() && l.proxima != l.titulo {
        y += 6.0 * s;
        let t = format!("→ {}", l.proxima);
        let (_, linea_p) = p.medir_texto("Ag", 13.0 * s);
        let (_, hp) = p.medir_texto_ajustado(&t, 13.0 * s, interior);
        let hp = hp.min(linea_p * TOPE_PROXIMA as f32);
        p.con_recorte(RectF { x: caja.x, y, ancho: caja.ancho, alto: hp }, |p| {
            p.texto_ajustado(&t, caja.x + pad, y, 13.0 * s, interior, con_alfa(TEXTO, 0.8));
        });
        y += hp;
    }
    let etiquetas = l.todas_las_etiquetas();
    if !etiquetas.is_empty() {
        y += 6.0 * s;
        let t = etiquetas.iter().take(4).map(|t| format!("#{t}")).collect::<Vec<_>>().join("  ");
        p.texto_linea(&t, caja.x + pad, y, 12.0 * s, interior, PUESTO);
    }
    // Borrar, al pasar el raton: pide un segundo clic.
    if encima || e.confirmar == Some(i) {
        let confirma = e.confirmar == Some(i);
        let b = if confirma {
            RectF { x: caja.x + caja.ancho - 110.0 * s, y: caja.y + caja.alto - 40.0 * s, ancho: 100.0 * s, alto: 32.0 * s }
        } else {
            RectF { x: caja.x + caja.ancho - 40.0 * s, y: caja.y + caja.alto - 40.0 * s, ancho: 32.0 * s, alto: 32.0 * s }
        };
        if confirma {
            ui::boton(p, &mut e.botones, b, Accion::Borrar(i), "¿Borrar?", ROJO, TEXTO, 13.0 * s, s);
        } else {
            p.rellenar_redondeado(b, 8.0 * s, hex(0x3a3a44));
            p.icono(&mi::DELETE, ui::encoger(b, 6.0 * s), ROJO);
            e.botones.zona(b, Accion::Borrar(i));
        }
    }
}

/// **La lista de comprobacion**: lo que hare distinto, como casillas, para
/// mirarla antes de empezar.
fn pintar_lista(e: &mut Estado, p: &Pintor, x: f32, y: f32, ancho: f32, vista: RectF, s: f32) -> f32 {
    let mut filas: Vec<usize> = e
        .visibles
        .iter()
        .copied()
        .filter(|i| {
            let l = &e.todas[*i].leccion;
            l.en_lista && (!l.proxima.trim().is_empty() || l.es_error())
        })
        .collect();
    filas.sort_by(|a, b| {
        let (la, lb) = (&e.todas[*a].leccion, &e.todas[*b].leccion);
        lb.gravedad.cmp(&la.gravedad).then(lb.repeticiones.len().cmp(&la.repeticiones.len()))
    });
    let hechas = filas.iter().filter(|i| e.marcadas.contains(&e.todas[**i].leccion.id)).count();
    let mut y = y;
    p.texto(&format!("Antes de empezar, repasa: {hechas} de {}", filas.len()), x + 4.0 * s, y + 4.0 * s, 15.0 * s, TEXTO);
    y += 34.0 * s;
    if filas.is_empty() {
        p.texto_ajustado(
            "Aún no hay nada de «la próxima vez…». Se rellena en «Qué pasó, por qué y qué haré distinto».",
            x + 8.0 * s,
            y,
            14.0 * s,
            ancho - 16.0 * s,
            APAGADO,
        );
        return y + 40.0 * s;
    }
    let alto = 58.0 * s;
    for i in filas {
        let caja = RectF { x, y, ancho, alto };
        if caja.y + alto >= vista.y && caja.y <= vista.y + vista.alto {
            let l = e.todas[i].leccion.clone();
            let marcada = e.marcadas.contains(&l.id);
            p.rellenar_redondeado(caja, 14.0 * s, if dentro(caja, e.botones.raton) { hex(0x2e2e36) } else { CRISTAL });
            let icono = if marcada { &mi::CHECK_BOX } else { &mi::CHECK_BOX_OUTLINE_BLANK };
            p.icono(icono, RectF { x: x + 12.0 * s, y: y + 15.0 * s, ancho: 28.0 * s, alto: 28.0 * s }, if marcada { PUESTO } else { APAGADO });
            let tx = x + 52.0 * s;
            let interior = ancho - 52.0 * s - 70.0 * s;
            let principal = if l.proxima.trim().is_empty() { &l.titulo } else { &l.proxima };
            p.texto_linea(principal, tx, y + 9.0 * s, 15.0 * s, interior, if marcada { APAGADO } else { TEXTO });
            p.texto_linea(&format!("{} {}", l.icono(), l.titulo), tx, y + 33.0 * s, 12.0 * s, interior, APAGADO);
            if !l.repeticiones.is_empty() {
                let t = format!("🔁×{}", l.veces_que_paso());
                let (tw, _) = p.medir_texto(&t, 13.0 * s);
                p.texto_color(&t, x + ancho - tw - 14.0 * s, y + 19.0 * s, 13.0 * s, NARANJA);
            }
            e.botones.zona(caja, Accion::Marcar(i));
        }
        y += alto + 6.0 * s;
    }
    y
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_lecciones::leccion::TIPO_ERROR;

    fn entrada(l: Leccion) -> Entrada {
        Entrada {
            leccion: l,
            mensaje: Default::default(),
            ficha: "p".into(),
            nombre_chat: "Obra".into(),
            general: false,
            archivo: std::path::PathBuf::from("x.leccion"),
        }
    }

    fn estado(todas: Vec<Leccion>) -> Estado {
        let todas: Vec<Entrada> = todas.into_iter().map(entrada).collect();
        let indices = todas.iter().map(|e| Indice::nuevo(e.leccion.clone())).collect();
        let hoy = Repaso::de_hoy(&todas.iter().map(|e| e.leccion.clone()).collect::<Vec<_>>(), 10 * pixpin_lecciones::leccion::DIA, 5);
        let mut e = Estado {
            pedido: Pedido {
                ubicacion: Ubicacion::Portable { raiz: std::env::temp_dir() },
                idioma: Idioma::Espanol,
                aparato: "PC01".into(),
                proyecto: None,
                consulta: None,
            },
            todas,
            indices,
            firma: Vec::new(),
            cambios: 0,
            consulta: Campo::default(),
            area: None,
            filtro: None,
            proyecto: None,
            nombres: HashMap::new(),
            en_lista: false,
            visibles: Vec::new(),
            mirado: None,
            hoy,
            destapada: false,
            marcadas: HashSet::new(),
            confirmar: None,
            scroll: 0.0,
            alto_contenido: 0.0,
            botones: Botones::default(),
            aviso: None,
            dictado: Dictado::Nada,
            moviendo: None,
            altos: HashMap::new(),
            arriba: 0.0,
            repintar: false,
        };
        e.filtrar();
        e
    }

    fn ejemplos() -> Vec<Leccion> {
        vec![
            Leccion {
                tipo: TIPO_ERROR.into(),
                area: "Construcción".into(),
                gravedad: 3,
                que_paso: "Se vació la losa sin revisar el encofrado".into(),
                proxima: "Revisar los puntales antes de cada vaciado".into(),
                etiquetas: vec!["obra".into()],
                etiquetas_auto: vec!["concreto".into(), "encofrado".into()],
                repeticiones: vec![5, 6],
                ..Leccion::nueva("a", 1, "No vaciar sin revisar el encofrado")
            },
            Leccion {
                area: "Trabajo".into(),
                proxima: "Si el cliente cambia algo, pedirlo por escrito".into(),
                ..Leccion::nueva("b", 2, "Pedir todo por escrito al cliente")
            },
            Leccion {
                area: "Estudio".into(),
                ..Leccion::nueva("c", 3, "Dormir antes del examen final")
            },
        ]
    }

    #[test]
    fn buscar_y_filtrar_dejan_solo_lo_que_toca() {
        let mut e = estado(ejemplos());
        assert_eq!(e.visibles.len(), 3);
        e.consulta.poner("escrito");
        e.filtrar();
        assert_eq!(e.visibles, vec![1]);
        e.consulta.poner("");
        e.filtro = Some(Filtro::Repetidas);
        e.filtrar();
        assert_eq!(e.visibles, vec![0]);
        e.filtro = None;
        e.area = Some("Estudio".into());
        e.filtrar();
        assert_eq!(e.visibles, vec![2]);
    }

    #[test]
    #[ignore = "necesita GPU; ejecutar con --ignored y mirar el PNG"]
    fn muestra_de_la_lista() {
        let mut e = estado(ejemplos());
        let marco = Rect { x: 0, y: 0, ancho: 1000, alto: 760 };
        // El primero coloca las fichas; el segundo es el que se mira.
        crate::ventanita::muestra("lecciones-lista", 1000, 760, |p, _| pintar_todo(&mut e, p, marco, 1.0));
        crate::ventanita::muestra("lecciones-lista", 1000, 760, |p, _| pintar_todo(&mut e, p, marco, 1.0));
        e.en_lista = true;
        crate::ventanita::muestra("lecciones-comprobacion", 1000, 760, |p, _| pintar_todo(&mut e, p, marco, 1.0));
    }
}
