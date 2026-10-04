//! **Apuntar una leccion en dos toques** (`LeccionActivity.kt`).
//!
//! Solo pide **lo que aprendiste**; mientras se escribe, todo lo demas se
//! propone solo (`Etiquetador`): etiquetas, area, si es un error o un
//! acierto, las causas. Se acepta tal cual o se quita con un toque. Si ya
//! habia una leccion parecida, lo ofrece: lo que toca entonces no es otra
//! leccion, es apuntar que **volvio a pasar**.
//!
//! Lo mismo, campo por campo y en el mismo orden que la hoja del movil: el
//! microfono (dictar de corrido y repartir), el aviso «se parece a una que
//! ya tienes», tipo y gravedad, area, etiquetas con ✨, el detalle plegado
//! (que paso, por que, causas de un toque, la proxima vez, palabras para
//! encontrarla), el proyecto al crearla o «me volvio a pasar» y las
//! relacionadas al editarla, y Guardar.
//!
//! Lo que cambia por ser Windows: es una ventana y no una hoja que sube
//! desde abajo, asi que «tocar fuera guarda» no existe. Lo equivalente al
//! gesto de atras del movil (que guarda) es Escape o cerrar la ventana; la
//! «×» descarta, como en el movil. Intro en «¿Que aprendiste?» guarda
//! (Mayusculas+Intro parte el renglon), Tab pasa a la caja siguiente.

#![forbid(unsafe_code)]

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_geom::Rect;
use pixpin_lecciones::buscador::{self, Indice, Resultado};
use pixpin_lecciones::etiquetador::{self, Aprendido, Propuesta};
use pixpin_lecciones::leccion::{self, AREAS, CAUSAS, TIPO_ACIERTO, TIPO_ERROR, TIPO_LECCION};
use pixpin_lecciones::{Leccion, Repaso, dictado};
use pixpin_render::icono::material as mi;
use pixpin_render::{Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Idioma, Ubicacion};

use super::almacen::{self, Donde, Entrada};
use super::dictar::{Dictado, Salida};
use super::ui::{self, Campo, PUESTO, VK_ENTRAR, VK_ESCAPE, VK_TAB};
use crate::caja_dibujo::hex;
use crate::overlay::Recursos;
use crate::ventanita::{APAGADO, Botones, CRISTAL, FONDO, ROJO, TEXTO, centrado};

/// Que ficha abrir.
#[derive(Debug, Clone)]
pub enum Que {
    /// Una nueva. `texto` la empieza (lo de un mensaje, lo de un pedido);
    /// `de_mensaje` y `ficha` dicen de donde sale y en que chat ira;
    /// `dictar` abre el microfono ya; `fotos` van con ella al guardarla
    /// (las imagenes pegadas en Flow Launcher, pedido `leccion_nueva`).
    Nueva {
        texto: Option<String>,
        de_mensaje: Option<String>,
        ficha: Option<String>,
        dictar: bool,
        fotos: Vec<almacen::Foto>,
    },
    /// Una que ya existe, por su id.
    Editar { id: String },
}

#[derive(Debug, Clone)]
pub struct Pedido {
    pub ubicacion: Ubicacion,
    pub idioma: Idioma,
    /// El codigo de este equipo, el del sello.
    pub aparato: String,
    pub que: Que,
}

/// Abre la ficha en su propio hilo y vuelve enseguida. Puede haber varias
/// a la vez, como en el movil (cada leccion en su tarea).
pub fn abrir(pedido: Pedido) {
    let lanzado = std::thread::Builder::new()
        .name("leccion".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let hecho = Recursos::nuevos().and_then(|r| bucle(&r, &pedido));
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la ficha de la leccion");
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de la leccion");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CampoId {
    Titulo = 0,
    Paso = 1,
    PorQue = 2,
    Proxima = 3,
    Referencias = 4,
    Etiqueta = 5,
}

const ORDEN: [CampoId; 6] = [
    CampoId::Titulo,
    CampoId::Paso,
    CampoId::PorQue,
    CampoId::Proxima,
    CampoId::Referencias,
    CampoId::Etiqueta,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Mover,
    Descartar,
    Guardar,
    Borrar,
    Foco(CampoId),
    Tipo(usize),
    Gravedad(i64),
    Area(usize),
    QuitarEtiqueta(usize),
    QuitarAuto(usize),
    Causa(usize),
    Detalle,
    Proyecto,
    ElegirProyecto(usize),
    PasoOtraVez,
    VolvioAPasar,
    Relacionada(usize),
    Dictar,
}

const TIPOS: [(&str, &str); 3] = [
    (TIPO_LECCION, "💡 Lección"),
    (TIPO_ERROR, "⚠️ Error"),
    (TIPO_ACIERTO, "✅ Acierto"),
];
const GRAVEDADES: [(i64, &str); 3] = [(1, "Leve"), (2, "Importante"), (3, "Grave")];

struct Estado {
    existente: Option<Entrada>,
    de_mensaje: Option<String>,
    campos: [Campo; 6],
    foco: Option<CampoId>,
    /// `None` = lo propuesto.
    tipo: Option<String>,
    /// `None` = lo propuesto; `Some("")` = sin area, puesto a mano.
    area: Option<String>,
    gravedad: i64,
    etiquetas: Vec<String>,
    quitadas: Vec<String>,
    causas: Vec<String>,
    causas_tocadas: bool,
    ficha: String,
    detalle: bool,
    propuesta: Propuesta,
    parecidas: Vec<Resultado>,
    mirado: Option<String>,
    todas: Vec<Entrada>,
    indices: Vec<Indice>,
    aprendido: Aprendido,
    proyectos: Vec<(String, String)>,
    eligiendo: bool,
    areas: Vec<String>,
    relacionadas: Vec<Leccion>,
    scroll: f32,
    alto_contenido: f32,
    botones: Botones<Accion>,
    aviso: Option<(String, Instant)>,
    dictado: Dictado,
    moviendo: Option<(pixpin_geom::Punto, Rect)>,
    /// Las fotos que iran con la leccion nueva al guardarla.
    fotos: Vec<almacen::Foto>,
}

impl Estado {
    fn campo(&self, c: CampoId) -> &Campo {
        &self.campos[c as usize]
    }

    fn campo_mut(&mut self, c: CampoId) -> &mut Campo {
        &mut self.campos[c as usize]
    }

    fn titulo(&self) -> &str {
        &self.campo(CampoId::Titulo).texto
    }

    /// Rellena con lo que habia (editar).
    fn rellenar_de(&mut self, e: &Entrada) {
        let l = &e.leccion;
        self.campo_mut(CampoId::Titulo).poner(&l.titulo);
        self.campo_mut(CampoId::Paso).poner(&l.que_paso);
        self.campo_mut(CampoId::PorQue).poner(&l.por_que);
        self.campo_mut(CampoId::Proxima).poner(&l.proxima);
        self.campo_mut(CampoId::Referencias)
            .poner(&l.referencias.join(", "));
        self.tipo = Some(l.tipo.clone());
        self.area = (!l.area.trim().is_empty()).then(|| l.area.clone());
        self.gravedad = l.gravedad;
        self.etiquetas = l.etiquetas.clone();
        self.quitadas = l.quitadas.clone();
        self.causas = l.causas.clone();
        self.causas_tocadas = true;
        self.ficha = e.ficha.clone();
        self.detalle = !l.que_paso.trim().is_empty()
            || !l.por_que.trim().is_empty()
            || !l.proxima.trim().is_empty();
    }

    /// Lo dicho o recibido, repartido en sus campos (`poner` del movil).
    fn poner(&mut self, dicho: &str) {
        let limpio = etiquetador::sin_etiquetas(dicho);
        for e in etiquetador::escritas(dicho) {
            if !self.etiquetas.contains(&e) {
                self.etiquetas.push(e);
            }
        }
        let c = dictado::repartir(&limpio);
        self.campo_mut(CampoId::Titulo).poner(&c.titulo);
        if !c.que_paso.trim().is_empty() {
            self.campo_mut(CampoId::Paso).poner(&c.que_paso);
        }
        if !c.por_que.trim().is_empty() {
            self.campo_mut(CampoId::PorQue).poner(&c.por_que);
        }
        if !c.proxima.trim().is_empty() && c.proxima != c.titulo {
            self.campo_mut(CampoId::Proxima).poner(&c.proxima);
        }
        if ORDEN[1..4]
            .iter()
            .any(|c| !self.campo(*c).texto.trim().is_empty())
        {
            self.detalle = true;
        }
    }

    fn texto_entero(&self) -> String {
        ORDEN[..5]
            .iter()
            .map(|c| self.campo(*c).texto.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Las propuestas, si cambio lo escrito.
    fn recalcular(&mut self) {
        let t = self.texto_entero();
        if self.mirado.as_deref() == Some(t.as_str()) {
            return;
        }
        self.propuesta = etiquetador::proponer(&t, &self.aprendido, &self.quitadas);
        self.parecidas = if self.existente.is_some() {
            Vec::new()
        } else {
            let texto = format!("{} {}", self.titulo(), self.campo(CampoId::Paso).texto);
            buscador::parecidas_por_defecto(&self.indices, &texto)
        };
        self.mirado = Some(t);
    }

    fn auto(&self) -> Vec<String> {
        self.propuesta
            .etiquetas
            .iter()
            .filter(|e| !self.etiquetas.contains(e) && !self.quitadas.contains(e))
            .cloned()
            .collect()
    }

    fn tipo_final(&self) -> String {
        self.tipo
            .clone()
            .or_else(|| self.propuesta.tipo.clone())
            .unwrap_or_else(|| TIPO_LECCION.into())
    }

    fn area_final(&self) -> Option<String> {
        self.area
            .clone()
            .or_else(|| self.propuesta.area.clone())
            .filter(|a| !a.is_empty())
    }

    fn causas_final(&self) -> Vec<String> {
        if self.causas_tocadas {
            return self.causas.clone();
        }
        let mut v = self.causas.clone();
        for c in &self.propuesta.causas {
            if !v.contains(c) {
                v.push(c.clone());
            }
        }
        v
    }

    fn poner_etiqueta(&mut self) {
        let e = self
            .campo(CampoId::Etiqueta)
            .texto
            .trim_matches([' ', ',', '#'])
            .to_lowercase();
        if !e.is_empty() && !self.etiquetas.contains(&e) {
            self.etiquetas.push(e.clone());
        }
        self.quitadas.retain(|q| *q != e);
        self.campo_mut(CampoId::Etiqueta).poner("");
        self.mirado = None;
    }

    /// La leccion con lo escrito, lista para guardar.
    fn leccion(&mut self, ahora: i64) -> Leccion {
        self.recalcular();
        let base = match &self.existente {
            Some(e) => e.leccion.clone(),
            None => Leccion {
                de_mensaje: self.de_mensaje.clone(),
                ..Leccion::nueva(&leccion::nuevo_id(ahora), ahora, "")
            },
        };
        Leccion {
            tocada: ahora,
            titulo: self.titulo().trim().to_string(),
            que_paso: self.campo(CampoId::Paso).texto.trim().to_string(),
            por_que: self.campo(CampoId::PorQue).texto.trim().to_string(),
            proxima: self.campo(CampoId::Proxima).texto.trim().to_string(),
            tipo: self.tipo_final(),
            area: self.area_final().unwrap_or_default(),
            gravedad: self.gravedad,
            etiquetas: self.etiquetas.clone(),
            etiquetas_auto: self.auto(),
            quitadas: self.quitadas.clone(),
            referencias: self
                .campo(CampoId::Referencias)
                .texto
                .split([',', ';'])
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            causas: self.causas_final(),
            ..base
        }
    }

    fn avisar(&mut self, texto: &str) {
        self.aviso = Some((texto.to_string(), Instant::now()));
    }
}

/// Lo que pasa al cerrar.
enum Fin {
    Seguir,
    /// Guardar si hay algo escrito, y cerrar (Escape, Alt+F4: el «atras»).
    GuardarYSalir,
    Salir,
}

fn bucle(recursos: &Recursos, pedido: &Pedido) -> Result<()> {
    let raiz = pedido.ubicacion.raiz().to_path_buf();
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let todas = almacen::listar(&raiz);
    let lecciones: Vec<Leccion> = todas.iter().map(|e| e.leccion.clone()).collect();
    let indices: Vec<Indice> = lecciones.iter().cloned().map(Indice::nuevo).collect();
    let indice = pixpin_proyecto::almacen::Indice::leer(&raiz);
    let mut proyectos: Vec<(String, String, i64, bool)> = indice
        .proyectos
        .iter()
        .map(|f| (f.id.clone(), f.nombre.clone(), f.tocado, f.es_guardados()))
        .collect();
    proyectos.sort_by(|a, b| b.3.cmp(&a.3).then(b.2.cmp(&a.2)));
    let general = match proyectos.iter().find(|p| p.3) {
        Some(p) => p.0.clone(),
        None => pixpin_proyecto::almacen::asegurar_guardados(&raiz, ahora, &pedido.aparato)?.id,
    };
    let mut areas: Vec<String> = AREAS.iter().map(|a| a.to_string()).collect();
    for l in &lecciones {
        if !l.area.trim().is_empty() && !areas.contains(&l.area) {
            areas.push(l.area.clone());
        }
    }
    let mut e = Estado {
        existente: None,
        de_mensaje: None,
        campos: Default::default(),
        foco: Some(CampoId::Titulo),
        tipo: None,
        area: None,
        gravedad: 1,
        etiquetas: Vec::new(),
        quitadas: Vec::new(),
        causas: Vec::new(),
        causas_tocadas: false,
        ficha: general.clone(),
        detalle: false,
        propuesta: Propuesta::default(),
        parecidas: Vec::new(),
        mirado: None,
        aprendido: etiquetador::aprender(&lecciones),
        todas,
        indices,
        proyectos: proyectos
            .iter()
            .map(|p| {
                (
                    p.0.clone(),
                    if p.3 {
                        "Sin proyecto (chat general)".to_string()
                    } else {
                        p.1.clone()
                    },
                )
            })
            .collect(),
        eligiendo: false,
        areas,
        relacionadas: Vec::new(),
        scroll: 0.0,
        alto_contenido: 0.0,
        botones: Botones::default(),
        aviso: None,
        dictado: Dictado::Nada,
        moviendo: None,
        fotos: Vec::new(),
    };
    let mut dictar_ya = false;
    match &pedido.que {
        Que::Editar { id } => {
            let Some(ent) = e.todas.iter().find(|x| x.leccion.id == *id).cloned() else {
                tracing::warn!(id, "la leccion que se pidio abrir no esta");
                return Ok(());
            };
            e.rellenar_de(&ent);
            e.relacionadas = buscador::relacionadas(&e.indices, &ent.leccion, 4);
            e.existente = Some(ent);
            e.foco = None;
        }
        Que::Nueva {
            texto,
            de_mensaje,
            ficha,
            dictar,
            fotos,
        } => {
            e.de_mensaje = de_mensaje.clone();
            e.fotos = fotos.clone();
            if let Some(f) = ficha
                .as_ref()
                .filter(|f| e.proyectos.iter().any(|p| p.0 == **f))
            {
                e.ficha = f.clone();
            }
            if let Some(t) = texto.as_deref().filter(|t| !t.trim().is_empty()) {
                e.poner(t);
            }
            dictar_ya = *dictar;
        }
    }
    e.recalcular();

    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .monitor_en(pixpin_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let mut marco = centrado(monitor.area_trabajo, 600, 780, monitor.escala_por_cien);
    let titulo_ventana = if e.existente.is_some() {
        "Lección"
    } else {
        "Nueva lección"
    };
    let mut ventana =
        VentanaOverlay::nueva_normal(marco, titulo_ventana).context("no se pudo abrir la ficha")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para la ficha")?;
    ventana.mostrar();
    ventana.traer_encima();
    ventana.enfocar();
    let hwnd = ventana.handle().0 as isize;
    almacen::apuntar_ventana(hwnd);
    if dictar_ya && let Some(a) = e.dictado.pulsar(&pedido.ubicacion, pedido.idioma) {
        e.avisar(&a);
    }

    let mut fin = Fin::Seguir;
    let mut pintar = true;
    while matches!(fin, Fin::Seguir) {
        pixpin_shell::overlay::bombear_pendientes();
        for (h, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if h != ventana.handle() {
                continue;
            }
            pintar = true;
            let local = |p: pixpin_geom::Punto, m: Rect| ((p.x - m.x) as f32, (p.y - m.y) as f32);
            match ev {
                EventoOverlay::Cerrar => fin = Fin::GuardarYSalir,
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
                        Some(a) => fin = hacer(&mut e, a, pedido),
                        None => e.foco = None,
                    }
                }
                EventoOverlay::BotonSoltado(_) => {
                    if e.moviendo.take().is_some() {
                        ventana.soltar_raton();
                    }
                }
                EventoOverlay::Rueda(m) => {
                    e.scroll -= m as f32 * 60.0 * escala;
                }
                EventoOverlay::Caracter(c) => {
                    if let Some(f) = e.foco {
                        if f == CampoId::Etiqueta && (c == ' ' || c == ',') {
                            e.poner_etiqueta();
                        } else if e.campo_mut(f).letra(c) {
                            e.recalcular();
                        }
                    }
                }
                EventoOverlay::Tecla {
                    vk, shift, ctrl, ..
                } => match (vk, e.foco) {
                    (VK_ESCAPE, _) => fin = Fin::GuardarYSalir,
                    (VK_ENTRAR, _) if ctrl => fin = hacer(&mut e, Accion::Guardar, pedido),
                    (VK_ENTRAR, Some(CampoId::Etiqueta)) => e.poner_etiqueta(),
                    (VK_ENTRAR, Some(CampoId::Titulo)) if !shift => {
                        fin = hacer(&mut e, Accion::Guardar, pedido)
                    }
                    (VK_TAB, f) => {
                        let visibles: Vec<CampoId> = ORDEN
                            .iter()
                            .copied()
                            .filter(|c| {
                                e.detalle
                                    || !matches!(
                                        c,
                                        CampoId::Paso
                                            | CampoId::PorQue
                                            | CampoId::Proxima
                                            | CampoId::Referencias
                                    )
                            })
                            .collect();
                        let i = f.and_then(|f| visibles.iter().position(|c| *c == f));
                        let n = visibles.len();
                        let j = match i {
                            None => 0,
                            Some(i) if shift => (i + n - 1) % n,
                            Some(i) => (i + 1) % n,
                        };
                        e.foco = Some(visibles[j]);
                    }
                    (_, Some(f)) => {
                        let multilinea = !matches!(f, CampoId::Etiqueta | CampoId::Referencias);
                        if e.campo_mut(f).tecla(vk, ctrl, shift, multilinea) {
                            e.recalcular();
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        if let Fin::GuardarYSalir = fin {
            if !e.titulo().trim().is_empty() {
                guardar_y_salir(&mut e, pedido);
            }
            break;
        }
        if let Fin::Salir = fin {
            break;
        }

        match e.dictado.avanzar(&pedido.ubicacion, pedido.idioma) {
            Salida::Nada => {}
            Salida::Texto(dicho) => {
                let t = e.titulo().trim().to_string();
                if t.is_empty() {
                    e.poner(&dicho);
                } else {
                    e.poner(&format!("{t}. {dicho}"));
                }
                e.recalcular();
                pintar = true;
            }
            Salida::Fallo(m) => {
                e.avisar(&m);
                pintar = true;
            }
        }
        if e.aviso
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() > Duration::from_millis(3_000))
        {
            e.aviso = None;
            pintar = true;
        }
        if e.dictado.activo() {
            pintar = true;
        }

        if pintar {
            let h = marco.alto as f32;
            let visible = h - (CABECERA + BARRA) * escala;
            e.scroll = e.scroll.clamp(0.0, (e.alto_contenido - visible).max(0.0));
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| pintar_todo(&mut e, p, marco, escala));
                let _ = superficie.presentar();
            }
            pintar = false;
        }
        let espera = if e.dictado.activo() {
            120
        } else if e.aviso.is_some() {
            250
        } else {
            1_000
        };
        pixpin_shell::overlay::esperar_eventos(Some(espera));
    }
    almacen::quitar_ventana(hwnd);
    Ok(())
}

fn guardar_y_salir(e: &mut Estado, pedido: &Pedido) {
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let l = e.leccion(ahora);
    let donde = match &e.existente {
        Some(x) => Donde::de(x),
        None => Donde::Nueva {
            ficha: e.ficha.clone(),
        },
    };
    // Las fotos solo van con una nueva (en una que ya existe no se ponen).
    let fotos: &[almacen::Foto] = if e.existente.is_none() { &e.fotos } else { &[] };
    match almacen::guardar_con_fotos(pedido.ubicacion.raiz(), &l, &donde, &pedido.aparato, fotos) {
        Ok(_) => tracing::info!(id = %l.id, "leccion guardada"),
        Err(err) => tracing::warn!(?err, "no se pudo guardar la leccion"),
    }
}

/// Lo que hace un clic. Devuelve si hay que cerrar.
fn hacer(e: &mut Estado, a: Accion, pedido: &Pedido) -> Fin {
    let raiz = pedido.ubicacion.raiz();
    match a {
        Accion::Mover => {}
        Accion::Descartar => return Fin::Salir,
        Accion::Guardar => {
            if !e.titulo().trim().is_empty() {
                guardar_y_salir(e, pedido);
                return Fin::Salir;
            }
            e.foco = Some(CampoId::Titulo);
        }
        Accion::Borrar => {
            if let Some(x) = &e.existente {
                if let Err(err) = almacen::borrar(raiz, x) {
                    tracing::warn!(?err, "no se pudo borrar la leccion");
                }
                return Fin::Salir;
            }
        }
        Accion::Foco(c) => e.foco = Some(c),
        Accion::Tipo(i) => e.tipo = Some(TIPOS[i].0.to_string()),
        Accion::Gravedad(g) => e.gravedad = g,
        Accion::Area(i) => {
            if let Some(a) = e.areas.get(i).cloned() {
                e.area = Some(if e.area_final().as_deref() == Some(a.as_str()) {
                    String::new()
                } else {
                    a
                });
            }
        }
        Accion::QuitarEtiqueta(i) => {
            if i < e.etiquetas.len() {
                e.etiquetas.remove(i);
                e.mirado = None;
            }
        }
        Accion::QuitarAuto(i) => {
            if let Some(x) = e.auto().get(i).cloned() {
                e.quitadas.push(x);
                e.mirado = None;
            }
        }
        Accion::Causa(i) => {
            if !e.causas_tocadas {
                e.causas = e.causas_final();
                e.causas_tocadas = true;
            }
            let c = CAUSAS[i].to_string();
            if let Some(n) = e.causas.iter().position(|x| *x == c) {
                e.causas.remove(n);
            } else {
                e.causas.push(c);
            }
        }
        Accion::Detalle => e.detalle = !e.detalle,
        Accion::Proyecto => e.eligiendo = !e.eligiendo,
        Accion::ElegirProyecto(i) => {
            if let Some(p) = e.proyectos.get(i) {
                e.ficha = p.0.clone();
            }
            e.eligiendo = false;
        }
        Accion::PasoOtraVez => {
            if let Some(r) = e.parecidas.first()
                && let Some(x) = e
                    .todas
                    .iter()
                    .find(|x| x.leccion.id == r.leccion.id)
                    .cloned()
            {
                return volvio_a_pasar(&x, pedido);
            }
        }
        Accion::VolvioAPasar => {
            if let Some(x) = e.existente.clone() {
                return volvio_a_pasar(&x, pedido);
            }
        }
        Accion::Relacionada(i) => {
            if let Some(l) = e.relacionadas.get(i) {
                abrir(Pedido {
                    que: Que::Editar { id: l.id.clone() },
                    ..pedido.clone()
                });
            }
        }
        Accion::Dictar => {
            if let Some(m) = e.dictado.pulsar(&pedido.ubicacion, pedido.idioma) {
                e.avisar(&m);
            }
        }
    }
    e.recalcular();
    Fin::Seguir
}

/// **Me volvio a pasar**: se apunta en la que ya estaba y se cierra.
fn volvio_a_pasar(x: &Entrada, pedido: &Pedido) -> Fin {
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    if let Err(err) = almacen::guardar(
        pedido.ubicacion.raiz(),
        &Repaso::repetida(&x.leccion, ahora),
        &Donde::de(x),
        &pedido.aparato,
    ) {
        tracing::warn!(?err, "no se pudo apuntar que volvio a pasar");
    }
    Fin::Salir
}

const CABECERA: f32 = 56.0;
const BARRA: f32 = 76.0;

fn rotulo(p: &Pintor, texto: &str, x: f32, y: f32, e: f32) -> f32 {
    p.texto(texto, x, y, 12.5 * e, APAGADO);
    y + 20.0 * e
}

fn pintar_todo(e: &mut Estado, p: &Pintor, marco: Rect, s: f32) {
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    p.limpiar(FONDO);
    e.botones.vaciar();
    let m = 18.0 * s;
    let ancho = w - 2.0 * m;
    let arriba = CABECERA * s;
    let abajo = h - BARRA * s;
    let contenido = RectF {
        x: 0.0,
        y: arriba,
        ancho: w,
        alto: abajo - arriba,
    };

    let mut alto_total = 0.0;
    p.con_recorte(contenido, |p| {
        let y0 = arriba + 10.0 * s - e.scroll;
        let y = pintar_contenido(e, p, m, y0, ancho, s);
        alto_total = y - y0 + 20.0 * s;
    });
    e.alto_contenido = alto_total;

    // La cabecera, por encima de lo que se desplazo bajo ella.
    let cab = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: arriba,
    };
    p.rellenar(cab, FONDO);
    e.botones.zona(cab, Accion::Mover);
    let titulo = if e.existente.is_some() {
        "💡 Lección"
    } else {
        "💡 Nueva lección"
    };
    p.texto_color(titulo, m, 15.0 * s, 20.0 * s, TEXTO);
    let lado = 36.0 * s;
    let cerrar = RectF {
        x: w - m - lado,
        y: (arriba - lado) / 2.0,
        ancho: lado,
        alto: lado,
    };
    e.botones.boton(p, cerrar, Accion::Descartar, "", None, s);
    p.icono(&mi::CLOSE, ui::encoger(cerrar, 8.0 * s), TEXTO);
    if e.existente.is_some() {
        let borrar = RectF {
            x: cerrar.x - lado - 8.0 * s,
            ..cerrar
        };
        e.botones.boton(p, borrar, Accion::Borrar, "", None, s);
        p.icono(&mi::DELETE, ui::encoger(borrar, 8.0 * s), ROJO);
    }

    // Guardar, siempre a mano.
    let barra = RectF {
        x: 0.0,
        y: abajo,
        ancho: w,
        alto: h - abajo,
    };
    p.rellenar(barra, FONDO);
    // Tapa tambien para el raton lo que se desplazo bajo la barra.
    e.botones.zona(barra, Accion::Mover);
    let boton = RectF {
        x: m,
        y: abajo + 12.0 * s,
        ancho,
        alto: 52.0 * s,
    };
    let vale = !e.titulo().trim().is_empty();
    ui::boton(
        p,
        &mut e.botones,
        boton,
        Accion::Guardar,
        "Guardar",
        if vale { PUESTO } else { hex(0x3a3a42) },
        if vale { hex(0x1a1a1a) } else { APAGADO },
        17.0 * s,
        s,
    );

    if let Some((t, _)) = &e.aviso {
        ui::aviso(p, t, w, h, s);
    }
}

/// El cuerpo de la ficha, de arriba abajo. Devuelve la `y` del final.
fn pintar_contenido(e: &mut Estado, p: &Pintor, x: f32, mut y: f32, ancho: f32, s: f32) -> f32 {
    let tam = 15.0 * s;
    let hueco = 12.0 * s;

    // Lo unico obligatorio, con el microfono dentro.
    let reserva = 44.0 * s;
    let t = e.campo(CampoId::Titulo);
    let alto = t.alto(p, ancho, 17.0 * s, 2, reserva, s);
    let caja = RectF { x, y, ancho, alto };
    t.pintar(
        p,
        caja,
        17.0 * s,
        e.foco == Some(CampoId::Titulo),
        "¿Qué aprendiste? Escríbelo o díctalo de corrido",
        reserva,
        s,
    );
    e.botones.zona(caja, Accion::Foco(CampoId::Titulo));
    let lado = 36.0 * s;
    let mic = RectF {
        x: x + ancho - lado - 6.0 * s,
        y: y + 6.0 * s,
        ancho: lado,
        alto: lado,
    };
    let grabando = matches!(e.dictado, Dictado::Grabando(_));
    p.circulo(
        (mic.x + lado / 2.0, mic.y + lado / 2.0),
        lado / 2.0,
        if grabando { ROJO } else { hex(0x3a3a44) },
    );
    p.icono(&mi::MIC, ui::encoger(mic, 8.0 * s), TEXTO);
    e.botones.zona(mic, Accion::Dictar);
    y += alto + 6.0 * s;
    if let Some(estado) = e.dictado.estado() {
        p.texto_color(
            &estado,
            x,
            y,
            13.0 * s,
            if grabando { ROJO } else { APAGADO },
        );
        y += 22.0 * s;
    }
    // Las imagenes pegadas en Flow Launcher: van con ella al guardar.
    if e.existente.is_none() && !e.fotos.is_empty() {
        let n = e.fotos.len();
        let t = format!(
            "📎 {n} {} · se guarda{} con la lección",
            if n == 1 { "foto" } else { "fotos" },
            if n == 1 { "" } else { "n" }
        );
        p.texto_color(&t, x, y, 13.0 * s, APAGADO);
        y += 22.0 * s;
    }
    y += hueco - 6.0 * s;

    // ¿Ya la tenias?
    if let Some(r) = e.parecidas.first() {
        let titulo = format!("«{}»", ui::corto(&r.leccion.titulo, 90));
        let ancho_texto = ancho - 190.0 * s;
        let (_, th) = p.medir_texto_ajustado(&titulo, 14.0 * s, ancho_texto);
        let alto = (th + 40.0 * s).max(64.0 * s);
        let caja = RectF { x, y, ancho, alto };
        p.rellenar_redondeado(caja, 14.0 * s, hex(0x2f3a2c));
        p.texto(
            "Se parece a una que ya tienes",
            x + 12.0 * s,
            y + 8.0 * s,
            12.5 * s,
            hex(0xb5d3a8),
        );
        p.texto_ajustado(
            &titulo,
            x + 12.0 * s,
            y + 28.0 * s,
            14.0 * s,
            ancho_texto,
            TEXTO,
        );
        let b = RectF {
            x: x + ancho - 170.0 * s,
            y: y + (alto - 40.0 * s) / 2.0,
            ancho: 160.0 * s,
            alto: 40.0 * s,
        };
        ui::boton(
            p,
            &mut e.botones,
            b,
            Accion::PasoOtraVez,
            "🔁 Pasó otra vez",
            hex(0x445a3e),
            TEXTO,
            14.0 * s,
            s,
        );
        y += alto + hueco;
    }

    // Tipo y gravedad: lo propuesto viene marcado.
    let tipo = e.tipo_final();
    let mut fichas: Vec<(String, Accion, bool, bool)> = TIPOS
        .iter()
        .enumerate()
        .map(|(i, (t, n))| (n.to_string(), Accion::Tipo(i), tipo == *t, false))
        .collect();
    fichas.extend(
        GRAVEDADES
            .iter()
            .map(|(g, n)| (n.to_string(), Accion::Gravedad(*g), e.gravedad == *g, false)),
    );
    y = ui::fila_de_fichas(p, &mut e.botones, x, y, ancho, &fichas, tam * 0.93, s) + hueco;

    // Area.
    y = rotulo(p, "Área", x, y, s);
    let area_final = e.area_final();
    let fichas: Vec<(String, Accion, bool, bool)> = e
        .areas
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let propuesta = e.area.is_none() && e.propuesta.area.as_deref() == Some(a.as_str());
            (
                if propuesta {
                    format!("✨ {a}")
                } else {
                    a.clone()
                },
                Accion::Area(i),
                area_final.as_deref() == Some(a.as_str()),
                false,
            )
        })
        .collect();
    y = ui::fila_de_fichas(p, &mut e.botones, x, y, ancho, &fichas, tam * 0.93, s) + hueco;

    // Etiquetas: las puestas y las propuestas (✨), que se quitan con un toque.
    y = rotulo(p, "Etiquetas", x, y, s);
    let mut fichas: Vec<(String, Accion, bool, bool)> = e
        .etiquetas
        .iter()
        .enumerate()
        .map(|(i, t)| (format!("#{t}"), Accion::QuitarEtiqueta(i), true, true))
        .collect();
    fichas.extend(
        e.auto()
            .iter()
            .enumerate()
            .map(|(i, t)| (format!("✨ #{t}"), Accion::QuitarAuto(i), false, true)),
    );
    if !fichas.is_empty() {
        y = ui::fila_de_fichas(p, &mut e.botones, x, y, ancho, &fichas, tam * 0.93, s) + 8.0 * s;
    }
    let c = e.campo(CampoId::Etiqueta);
    let ancho_e = 220.0 * s;
    let alto = c.alto(p, ancho_e, tam, 1, 0.0, s);
    let caja = RectF {
        x,
        y,
        ancho: ancho_e,
        alto,
    };
    c.pintar(
        p,
        caja,
        tam,
        e.foco == Some(CampoId::Etiqueta),
        "+ etiqueta",
        0.0,
        s,
    );
    e.botones.zona(caja, Accion::Foco(CampoId::Etiqueta));
    y += alto + hueco;

    // Lo demas, plegado: no estorba a quien solo quiere apuntar la frase.
    let rot = if e.detalle {
        "▴  Menos detalle"
    } else {
        "▾  Qué pasó, por qué y qué haré distinto"
    };
    let (rw, _) = p.medir_texto(rot, tam);
    let b = RectF {
        x,
        y,
        ancho: rw + 20.0 * s,
        alto: 34.0 * s,
    };
    ui::boton(
        p,
        &mut e.botones,
        b,
        Accion::Detalle,
        rot,
        FONDO,
        PUESTO,
        tam,
        s,
    );
    y += 34.0 * s + hueco;
    if e.detalle {
        for (id, pista) in [
            (CampoId::Paso, "Qué pasó"),
            (CampoId::PorQue, "Por qué pasó"),
        ] {
            y = campo_con_rotulo(e, p, id, pista, x, y, ancho, s) + hueco;
        }
        y = rotulo(p, "Causas (un toque)", x, y, s);
        let elegidas = e.causas_final();
        let fichas: Vec<(String, Accion, bool, bool)> = CAUSAS
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let propuesta = !e.causas_tocadas
                    && e.propuesta.causas.iter().any(|x| x == c)
                    && !e.causas.iter().any(|x| x == c);
                (
                    if propuesta {
                        format!("✨ {c}")
                    } else {
                        c.to_string()
                    },
                    Accion::Causa(i),
                    elegidas.iter().any(|x| x == c),
                    false,
                )
            })
            .collect();
        y = ui::fila_de_fichas(p, &mut e.botones, x, y, ancho, &fichas, tam * 0.93, s) + hueco;
        y = campo_con_rotulo(
            e,
            p,
            CampoId::Proxima,
            "La próxima vez… (si pasa X, haré Y)",
            x,
            y,
            ancho,
            s,
        ) + hueco;
        y = campo_con_rotulo(
            e,
            p,
            CampoId::Referencias,
            "Palabras para encontrarla (separadas por comas)",
            x,
            y,
            ancho,
            s,
        ) + hueco;
    }

    if e.existente.is_none() {
        // El proyecto, al crearla: de el depende en que chat sale.
        let nombre = e
            .proyectos
            .iter()
            .find(|p| p.0 == e.ficha)
            .map_or("Sin proyecto (chat general)".to_string(), |p| p.1.clone());
        let fichas = vec![(format!("📁 {nombre}"), Accion::Proyecto, false, false)];
        y = ui::fila_de_fichas(p, &mut e.botones, x, y, ancho, &fichas, tam * 0.93, s) + 6.0 * s;
        if e.eligiendo {
            let fila = 34.0 * s;
            let caja = RectF {
                x,
                y,
                ancho,
                alto: fila * e.proyectos.len().min(12) as f32 + 8.0 * s,
            };
            p.rellenar_redondeado(caja, 10.0 * s, CRISTAL);
            let mut yy = y + 4.0 * s;
            for (i, (id, n)) in e.proyectos.iter().enumerate().take(12) {
                let r = RectF {
                    x: x + 4.0 * s,
                    y: yy,
                    ancho: ancho - 8.0 * s,
                    alto: fila,
                };
                if crate::ventanita::dentro(r, e.botones.raton) {
                    p.rellenar_redondeado(r, 8.0 * s, hex(0x36363e));
                }
                p.texto_linea(
                    n,
                    r.x + 10.0 * s,
                    r.y + 8.0 * s,
                    tam * 0.93,
                    r.ancho - 20.0 * s,
                    if *id == e.ficha { PUESTO } else { TEXTO },
                );
                e.botones.zona(r, Accion::ElegirProyecto(i));
                yy += fila;
            }
            y = caja.y + caja.alto + hueco;
        } else {
            y += hueco;
        }
    } else if let Some(x_e) = &e.existente {
        // Lo que hace que no se repita: apuntar que paso otra vez.
        let veces = x_e.leccion.veces_que_paso();
        let rot = if x_e.leccion.repeticiones.is_empty() {
            "🔁 Me volvió a pasar".to_string()
        } else {
            format!("🔁 Me volvió a pasar (van {veces})")
        };
        let b = RectF {
            x,
            y,
            ancho,
            alto: 44.0 * s,
        };
        p.rellenar_redondeado(b, 10.0 * s, hex(0x4a4a52));
        ui::boton(
            p,
            &mut e.botones,
            ui::encoger(b, 1.0 * s),
            Accion::VolvioAPasar,
            &rot,
            FONDO,
            TEXTO,
            tam,
            s,
        );
        y += 44.0 * s + hueco;
        if !e.relacionadas.is_empty() {
            y = rotulo(p, "Relacionadas", x, y, s);
            for (i, l) in e.relacionadas.iter().enumerate() {
                let r = RectF {
                    x,
                    y,
                    ancho,
                    alto: 34.0 * s,
                };
                if crate::ventanita::dentro(r, e.botones.raton) {
                    p.rellenar_redondeado(r, 10.0 * s, CRISTAL);
                }
                p.texto_linea(
                    &format!("💡 {}", l.titulo),
                    x + 8.0 * s,
                    y + 8.0 * s,
                    tam * 0.93,
                    ancho - 16.0 * s,
                    TEXTO,
                );
                e.botones.zona(r, Accion::Relacionada(i));
                y += 36.0 * s;
            }
            y += hueco;
        }
    }
    y
}

#[allow(clippy::too_many_arguments)] // estado, pintor, campo, rotulo, sitio y escala
fn campo_con_rotulo(
    e: &mut Estado,
    p: &Pintor,
    id: CampoId,
    pista: &str,
    x: f32,
    y: f32,
    ancho: f32,
    s: f32,
) -> f32 {
    let tam = 15.0 * s;
    let c = e.campo(id);
    let y = if c.texto.is_empty() {
        y
    } else {
        rotulo(p, pista, x, y, s)
    };
    let alto = c.alto(p, ancho, tam, 1, 0.0, s);
    let caja = RectF { x, y, ancho, alto };
    c.pintar(p, caja, tam, e.foco == Some(id), pista, 0.0, s);
    e.botones.zona(caja, Accion::Foco(id));
    y + alto
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn estado() -> Estado {
        Estado {
            existente: None,
            de_mensaje: None,
            campos: Default::default(),
            foco: None,
            tipo: None,
            area: None,
            gravedad: 1,
            etiquetas: Vec::new(),
            quitadas: Vec::new(),
            causas: Vec::new(),
            causas_tocadas: false,
            ficha: "g".into(),
            detalle: false,
            propuesta: Propuesta::default(),
            parecidas: Vec::new(),
            mirado: None,
            todas: Vec::new(),
            indices: Vec::new(),
            aprendido: Aprendido::default(),
            proyectos: Vec::new(),
            eligiendo: false,
            areas: AREAS.iter().map(|a| a.to_string()).collect(),
            relacionadas: Vec::new(),
            scroll: 0.0,
            alto_contenido: 0.0,
            botones: Botones::default(),
            aviso: None,
            dictado: Dictado::Nada,
            moviendo: None,
            fotos: Vec::new(),
        }
    }

    #[test]
    fn lo_dictado_se_reparte_y_lo_propuesto_va_a_la_leccion() {
        let mut e = estado();
        e.poner("pasó que no revisé el encofrado y la losa se fisuró porque había prisa, la próxima vez reviso los puntales #obra");
        assert_eq!(e.campo(CampoId::Proxima).texto, "");
        assert_eq!(e.titulo(), "Reviso los puntales");
        assert!(e.detalle);
        assert_eq!(e.etiquetas, vec!["obra"]);
        let l = e.leccion(5000);
        assert_eq!(l.tipo, TIPO_ERROR, "propuesto: error");
        assert_eq!(l.area, "Construcción");
        assert!(l.causas.contains(&"Prisa".to_string()));
        assert!(l.etiquetas_auto.contains(&"concreto".to_string()));
        assert!(
            !l.etiquetas_auto.contains(&"obra".to_string()),
            "la puesta no va tambien como automatica"
        );
        assert_eq!(l.creada, 5000);
    }

    #[test]
    #[ignore = "necesita GPU; ejecutar con --ignored y mirar el PNG"]
    fn muestra_de_la_ficha() {
        let mut e = estado();
        e.poner("pasó que no revisé el encofrado y la losa se fisuró porque había prisa, la próxima vez reviso los puntales antes del vaciado #obra");
        e.proyectos = vec![
            ("g".into(), "Sin proyecto (chat general)".into()),
            ("p".into(), "Edificio Sur".into()),
        ];
        e.foco = Some(CampoId::Titulo);
        let parecida = Leccion::nueva("x", 1, "Revisar los puntales del encofrado antes de vaciar");
        e.indices = vec![Indice::nuevo(parecida)];
        e.mirado = None;
        e.recalcular();
        let marco = Rect {
            x: 0,
            y: 0,
            ancho: 600,
            alto: 780,
        };
        crate::ventanita::muestra("leccion-ficha", 600, 780, |p, _| {
            pintar_todo(&mut e, p, marco, 1.0)
        });
    }

    #[test]
    fn quitar_una_propuesta_no_la_deja_volver_y_area_se_puede_dejar_vacia() {
        let mut e = estado();
        e.campo_mut(CampoId::Titulo).poner("La losa se fisuró");
        e.recalcular();
        let i = e.auto().iter().position(|x| x == "concreto").unwrap();
        e.quitadas.push(e.auto()[i].clone());
        e.mirado = None;
        e.recalcular();
        assert!(!e.auto().contains(&"concreto".to_string()));
        assert_eq!(e.area_final().as_deref(), Some("Construcción"));
        e.area = Some(String::new());
        assert_eq!(e.area_final(), None, "sin area, puesto a mano");
    }
}
