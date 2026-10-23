//! **La ventana «Horario»**: el calendario semanal donde se mete el horario
//! arrastrando (18-oct-2026).
//!
//! El usuario: «una interfaz estilo calendario semanal en la que pueda
//! ingresar mi horario: en el lado lateral puedo ingresar mis asignaturas y
//! pueda jalarlas sobre el calendario semanal y soltarlas».
//!
//! - A la izquierda, la **lista de asignaturas**: la caja «Nueva
//!   asignatura» (con el aula detras si se quiere: `Geometría @A-12`), el
//!   circulo de color de cada una (pulsarlo pasa al siguiente color), su aula
//!   y la papelera al pasar el raton. Pulsar una ensena debajo sus tareas
//!   pendientes (cada asignatura es una lista de tareas, ver
//!   [`super::asignaturas`]); doble clic, su aula.
//! - A la derecha, la **rejilla de la semana**: lunes a sabado (el domingo
//!   con su boton de la barra), de 7:00 a 22:00 en franjas de 15 min, con
//!   el dia de hoy y la hora de ahora marcados.
//! - **Arrastrar** una asignatura a la rejilla crea una clase de 1 h donde
//!   se suelta; una clase se mueve arrastrandola y se alarga o se acorta
//!   tirando de su borde de abajo. Clic derecho o Supr la borran (con
//!   «Deshacer»); doble clic abre su ficha: aula, nota para la voz y los
//!   minutos de antes del aviso.
//!
//! Todo se escribe al momento en `horario.md` ([`super::semana::reescribir`])
//! y se relee la agenda: las llamadas salen de lo mismo que se ve. Si el
//! horario cambia fuera (editado como texto), la ventana lo relee.
//!
//! La interfaz es la v2 de Tareas y Timeline: su cabecera
//! (`crate::cabecera`), sus botones (`v2`, `lecciones::ui::boton_v2`), sus
//! cajas (`lecciones::ui::Campo`) y su aviso con «Deshacer» (`v2::aviso`).
//! Lo que se decide sin pintar vive en [`super::semana`].

#![forbid(unsafe_code)]

use std::path::Path;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use pixpin_geom::Rect;
use pixpin_ia::fechas::{self, Momento};
use pixpin_render::icono::material as mi;
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma, Ubicacion};

use super::semana::{self, Rejilla, Semana};
use crate::caja_dibujo::hex;
use crate::lecciones::ui;
use crate::overlay::Recursos;
use crate::tareas::{Fila, Lista};
use crate::v2::aviso::Aviso;
use crate::v2::color::{blanco, con_alfa, negro};
use crate::v2::geom::{centrado as cuadrado, encoger};
use crate::v2::{self as d, ACENTO, FONDO, GRIS, TEXTO};
use crate::ventanita::{Botones, centrado, dentro};

const VK_TAB: u32 = 0x09;
const VK_ENTRAR: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_SUPRIMIR: u32 = 0x2E;
const VK_Z: u32 = 0x5A;

/// Dos clics en lo mismo antes de esto son un doble clic (el de Tareas).
const DOBLE_CLIC: Duration = Duration::from_millis(500);

/// El tamano de la ventana, en pixeles logicos.
const ANCHO_VENTANA: u32 = 1180;
const ALTO_VENTANA: u32 = 780;

/// Medidas, en pixeles logicos.
const LATERAL: f32 = 280.0;
const FILA: f32 = 44.0;
const COLUMNA_HORAS: f32 = 48.0;
const CABECERA_DIAS: f32 = 36.0;
/// Lo que mide la zona del borde de abajo de una clase que la alarga.
const BORDE: f32 = 7.0;
/// Cuantas tareas pendientes se ensenan de la asignatura elegida.
const TAREAS_A_LA_VISTA: usize = 8;

/// La rayita roja de la hora de ahora (la del calendario de siempre).
const AHORA: Color = hex(0xFF453A);

/// La ventana abierta (su HWND), -1 mientras nace, 0 sin ventana.
static ABIERTA: AtomicIsize = AtomicIsize::new(0);

/// Abre la ventana del horario, o la trae delante. Vuelve enseguida.
pub fn abrir(idioma: Idioma, ubicacion: Ubicacion, aparato: String) {
    let pedida = Instant::now();
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
        .name("horario".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            let hecho = crate::dispositivo_perdido::con_recursos("horario", |r| {
                bucle(r, &textos, idioma, &ubicacion, &aparato, pedida)
            });
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la ventana del horario");
            }
            ABIERTA.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del horario");
        ABIERTA.store(0, Ordering::SeqCst);
    }
}

// --------------------------------------------------------------- el estado

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CampoId {
    Nueva,
    Aula,
    Nota,
    Avisar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Accion {
    /// Lo que no hace nada: el fondo, la caja del aviso, el velo.
    #[default]
    Fondo,
    Mover,
    Cerrar,
    /// El boton «Domingo» de la barra: ensenar o no el domingo.
    Domingo,
    /// «Como texto»: el `horario.md` en el editor de notas.
    ComoTexto,
    Campo(CampoId),
    /// El «+» de la caja de nueva asignatura (o Intro en ella).
    Anadir,
    /// La fila de la asignatura `.0`: elegirla, o el principio de un
    /// arrastre a la rejilla.
    Asignatura(usize),
    /// El circulo de color de la asignatura `.0`.
    Color(usize),
    BorrarAsignatura(usize),
    /// La clase `.0` de la rejilla: elegirla o moverla.
    Bloque(usize),
    /// El borde de abajo de la clase `.0`: alargarla o acortarla.
    Borde(usize),
    /// Lo vacio de la rejilla.
    Rejilla,
    /// La casilla de la tarea pendiente `.0` de la asignatura elegida.
    Tarea(usize),
    /// El evento `.0` de Google Calendar de la semana: abrirlo en Google.
    EventoGoogle(usize),
    AbrirTareas,
    Guardar,
    Cancelar,
    BorrarBloque,
    Deshacer,
}

/// Lo que se edita en la ficha que se abre con doble clic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edicion {
    Bloque(usize),
    Asignatura(usize),
}

/// Lo que se lleva con el raton pulsado.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Agarre {
    /// Una asignatura de la lista; hasta que el raton se aleja es un clic.
    Asignatura {
        i: usize,
        desde: (f32, f32),
        arrastrando: bool,
    },
    /// Una clase: `off` son los minutos entre su principio y donde se pulso.
    Bloque {
        i: usize,
        desde: (f32, f32),
        off: i64,
        arrastrando: bool,
    },
    Borde {
        i: usize,
    },
}

struct Estado {
    semana: Semana,
    /// La antelacion de todo el horario (lo que avisa una clase sin la suya).
    antelacion: u32,
    /// La huella de `horario.md` tras la ultima lectura o escritura nuestra.
    firma: Option<(SystemTime, u64)>,
    domingo: bool,
    elegida: Option<usize>,
    bloque: Option<usize>,
    nueva: ui::Campo,
    aula: ui::Campo,
    nota: ui::Campo,
    avisar: ui::Campo,
    foco: Option<CampoId>,
    edicion: Option<Edicion>,
    agarre: Option<Agarre>,
    botones: Botones<Accion>,
    aviso: Option<Aviso<Accion>>,
    /// La semana de antes del ultimo cambio, mientras su aviso ofrece
    /// «Deshacer».
    deshacer: Option<Semana>,
    moviendo: Option<(pixpin_geom::Punto, Rect)>,
    ultimo_clic: Option<(Accion, Instant)>,
    /// Las listas del chat «Asignaturas» y la huella de su cuaderno.
    listas: Vec<Lista>,
    chat: Option<String>,
    firma_chat: Option<(SystemTime, u64)>,
    /// Las tareas pendientes de la asignatura elegida, como se pintaron.
    pendientes: Vec<(Lista, Fila)>,
    /// Donde quedo la rejilla en el ultimo fotograma.
    rejilla: Option<Rejilla>,
    /// Lunes de esta semana (dias desde 1970) y la hora local de ahora.
    ahora_ms: i64,
    /// La primera asignatura que se ve en la lista (la rueda la mueve, 18-oct:
    /// con mas de una docena, las de abajo no se alcanzaban).
    primera: usize,
    /// Los eventos de Google Calendar que se pintaron (18-oct), para abrir
    /// el pulsado.
    google: Vec<pixpin_ia::conexiones::calendario::Evento>,
}

fn firma_de(ruta: &Path) -> Option<(SystemTime, u64)> {
    std::fs::metadata(ruta)
        .ok()
        .map(|m| (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()))
}

impl Estado {
    fn nuevo() -> Estado {
        Estado {
            semana: Semana::default(),
            antelacion: super::ANTELACION,
            firma: None,
            domingo: false,
            primera: 0,
            google: Vec::new(),
            elegida: None,
            bloque: None,
            nueva: ui::Campo::default(),
            aula: ui::Campo::default(),
            nota: ui::Campo::default(),
            avisar: ui::Campo::default(),
            foco: None,
            edicion: None,
            agarre: None,
            botones: Botones::default(),
            aviso: None,
            deshacer: None,
            moviendo: None,
            ultimo_clic: None,
            listas: Vec::new(),
            chat: None,
            firma_chat: None,
            pendientes: Vec::new(),
            rejilla: None,
            ahora_ms: pixpin_shell::entorno::ahora_local_ms(),
        }
    }

    /// Lee `horario.md` (vacio si aun no esta).
    fn cargar(&mut self, raiz: &Path) {
        let ruta = super::ruta(raiz);
        let texto = std::fs::read_to_string(&ruta).unwrap_or_default();
        self.cargar_de(&texto);
        self.firma = firma_de(&ruta);
    }

    fn cargar_de(&mut self, texto: &str) {
        let h = super::leer(texto, Momento::de_ms_locales(self.ahora_ms));
        self.semana = Semana::de_horario(&h);
        self.antelacion = h.antelacion;
        self.domingo |= self.semana.bloques.iter().any(|b| b.dia == 6);
        // Releido, los numeros pueden ser otros.
        self.bloque = None;
        self.edicion = None;
        self.elegida = self.elegida.filter(|&i| i < self.semana.asignaturas.len());
    }

    /// Las listas de tareas de las asignaturas.
    fn cargar_tareas(&mut self, raiz: &Path) {
        self.chat = super::asignaturas::ficha(raiz).map(|f| f.id);
        self.firma_chat = super::asignaturas::firma(raiz, self.chat.as_deref());
        self.listas = super::asignaturas::listas(raiz);
    }

    /// Escribe la semana en `horario.md` y relee la agenda de avisos.
    fn guardar(&mut self, raiz: &Path, textos: &Catalogo) {
        let ruta = super::ruta(raiz);
        let original =
            std::fs::read_to_string(&ruta).unwrap_or_else(|_| super::PLANTILLA.to_string());
        let nuevo = semana::reescribir(&original, &self.semana);
        if nuevo != original
            && let Err(e) = std::fs::write(&ruta, &nuevo)
        {
            tracing::warn!(?e, "horario: no se pudo guardar");
            self.decir(textos.t("horario-no-guardado"));
            return;
        }
        self.firma = firma_de(&ruta);
        crate::recordatorios::releer(raiz);
    }

    /// Un cambio de la semana: se guarda y se puede deshacer.
    fn cambiar(&mut self, nueva: Semana, raiz: &Path, textos: &Catalogo) {
        if nueva == self.semana {
            return;
        }
        self.deshacer = Some(std::mem::replace(&mut self.semana, nueva));
        self.guardar(raiz, textos);
    }

    fn decir(&mut self, t: String) {
        self.aviso = Some(Aviso::nuevo(t));
    }

    fn decir_con_deshacer(&mut self, t: String, textos: &Catalogo) {
        self.aviso = Some(Aviso::con_deshacer(t, textos.t("horario-deshacer"), Accion::Deshacer));
    }

    fn campo_mut(&mut self, c: CampoId) -> &mut ui::Campo {
        match c {
            CampoId::Nueva => &mut self.nueva,
            CampoId::Aula => &mut self.aula,
            CampoId::Nota => &mut self.nota,
            CampoId::Avisar => &mut self.avisar,
        }
    }

    /// **La semana como queda si se suelta ahora** lo que se arrastra.
    fn previa(&self) -> Option<Semana> {
        let r = self.rejilla?;
        let raton = self.botones.raton;
        let mut s = self.semana.clone();
        match self.agarre? {
            Agarre::Asignatura {
                i,
                arrastrando: true,
                ..
            } => {
                let (dia, minuto) = r.en(raton)?;
                s.soltar(i, dia, minuto)?;
            }
            Agarre::Bloque {
                i,
                off,
                arrastrando: true,
                ..
            } => s.mover(i, r.dia_cercano(raton.0), r.minuto_en(raton.1) - off),
            Agarre::Borde { i } => s.alargar(i, r.minuto_en(raton.1).max(0) as u32),
            _ => return None,
        }
        Some(s)
    }

    /// Abre la ficha de una clase o de una asignatura con lo que tiene.
    fn editar(&mut self, ed: Edicion) {
        let (aula, nota, avisar) = match ed {
            Edicion::Bloque(i) => {
                let Some(b) = self.semana.bloques.get(i) else {
                    return;
                };
                (
                    b.aula.clone().unwrap_or_default(),
                    b.notas.join("\n"),
                    b.antelacion.map(|n| n.to_string()).unwrap_or_default(),
                )
            }
            Edicion::Asignatura(i) => {
                let Some(a) = self.semana.asignaturas.get(i) else {
                    return;
                };
                (a.aula.clone().unwrap_or_default(), String::new(), String::new())
            }
        };
        self.aula.poner(&aula);
        self.nota.poner(&nota);
        self.avisar.poner(&avisar);
        self.edicion = Some(ed);
        self.foco = Some(CampoId::Aula);
    }

    /// Lo escrito en la ficha, a la semana.
    fn guardar_edicion(&mut self, raiz: &Path, textos: &Catalogo) {
        let Some(ed) = self.edicion.take() else {
            return;
        };
        self.foco = None;
        let aula = Some(self.aula.texto().trim().to_string()).filter(|a| !a.is_empty());
        let mut s = self.semana.clone();
        match ed {
            Edicion::Bloque(i) => {
                let Some(b) = s.bloques.get_mut(i) else {
                    return;
                };
                b.aula = aula;
                b.notas = self
                    .nota
                    .texto()
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect();
                b.antelacion = self
                    .avisar
                    .texto()
                    .trim()
                    .trim_end_matches(|c: char| !c.is_ascii_digit())
                    .parse::<u32>()
                    .ok()
                    .map(|n| n.min(24 * 60));
            }
            Edicion::Asignatura(i) => s.poner_aula_de_asignatura(i, aula),
        }
        self.cambiar(s, raiz, textos);
    }

    fn borrar_bloque(&mut self, i: usize, raiz: &Path, textos: &Catalogo) {
        let mut s = self.semana.clone();
        if s.borrar(i).is_some() {
            self.bloque = None;
            self.edicion = None;
            self.foco = None;
            self.cambiar(s, raiz, textos);
            self.decir_con_deshacer(textos.t("horario-borrada"), textos);
        }
    }

    /// Las tareas pendientes de la asignatura elegida.
    fn pendientes_de_la_elegida(&self) -> Vec<(Lista, Fila)> {
        let Some(a) = self.elegida.and_then(|i| self.semana.asignaturas.get(i)) else {
            return Vec::new();
        };
        let Some(l) = super::asignaturas::de(&self.listas, &a.nombre) else {
            return Vec::new();
        };
        let (arriba, _) = l.a_la_vista(&|_| false);
        arriba
            .into_iter()
            .filter(|f| !f.hecha)
            .map(|f| (l.clone(), f.clone()))
            .collect()
    }
}

// --------------------------------------------------------------- el bucle

#[allow(clippy::too_many_arguments)] // recursos, textos, idioma, datos, aparato y la hora del pedido
fn bucle(
    recursos: &Recursos,
    textos: &Catalogo,
    idioma: Idioma,
    ubicacion: &Ubicacion,
    aparato: &str,
    pedida: Instant,
) -> Result<()> {
    let raiz = ubicacion.raiz().to_path_buf();
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = pixpin_shell::pantalla_de::monitor_de_la_ventana_activa()
        .and_then(|r| monitores.monitores().iter().find(|m| m.area == r))
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let mut marco = centrado(
        monitor.area_trabajo,
        ANCHO_VENTANA,
        ALTO_VENTANA,
        monitor.escala_por_cien,
    );
    let mut ventana = VentanaOverlay::nueva_normal(marco, &textos.t("horario-titulo"))
        .context("no se pudo abrir la ventana del horario")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para el horario")?;
    ventana.mostrar();
    ventana.enfocar();
    ABIERTA.store(ventana.handle().0 as isize, Ordering::SeqCst);
    let hwnd = ventana.handle();

    let mut e = Estado::nuevo();
    e.cargar(&raiz);
    if e.semana.asignaturas.is_empty() {
        e.foco = Some(CampoId::Nueva);
    }
    let mut primera = true;
    let mut tareas_leidas = false;
    let mut mirado = Instant::now();
    let mut minuto = e.ahora_ms / 60_000;
    let mut vivo = true;
    let mut pintar = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        for (h, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if h != hwnd {
                continue;
            }
            pintar = true;
            let local = |p: pixpin_geom::Punto, m: Rect| ((p.x - m.x) as f32, (p.y - m.y) as f32);
            match ev {
                EventoOverlay::Cerrar => vivo = false,
                // La rueda sobre la lista de asignaturas: una por muesca.
                EventoOverlay::Rueda(m) if e.rejilla.is_some_and(|r| e.botones.raton.0 < r.x) => {
                    let pasos = (m as f32 / 120.0).round() as i64;
                    let max = e.semana.asignaturas.len().saturating_sub(1) as i64;
                    e.primera = (e.primera as i64 - pasos).clamp(0, max) as usize;
                }
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
                    match e.agarre.as_mut() {
                        Some(Agarre::Asignatura {
                            desde, arrastrando, ..
                        })
                        | Some(Agarre::Bloque {
                            desde, arrastrando, ..
                        }) if !*arrastrando
                            && crate::tareas::kanban::es_arrastre(*desde, e.botones.raton, escala) =>
                        {
                            *arrastrando = true;
                        }
                        _ => {}
                    }
                }
                EventoOverlay::BotonPulsado(p) => {
                    e.botones.raton = local(p, marco);
                    let Some(a) = e.botones.bajo_el_raton() else {
                        continue;
                    };
                    let doble = e
                        .ultimo_clic
                        .is_some_and(|(b, t)| b == a && t.elapsed() < DOBLE_CLIC);
                    e.ultimo_clic = (!doble).then(|| (a, Instant::now()));
                    match a {
                        Accion::Mover => {
                            e.moviendo = Some((p, marco));
                            ventana.capturar_raton();
                        }
                        Accion::Asignatura(i) if doble => e.editar(Edicion::Asignatura(i)),
                        Accion::Bloque(i) if doble => {
                            e.bloque = Some(i);
                            e.editar(Edicion::Bloque(i));
                        }
                        Accion::Asignatura(i) => {
                            e.agarre = Some(Agarre::Asignatura {
                                i,
                                desde: e.botones.raton,
                                arrastrando: false,
                            });
                            ventana.capturar_raton();
                        }
                        Accion::Bloque(i) => {
                            let off = match (e.rejilla, e.semana.bloques.get(i)) {
                                (Some(r), Some(b)) => r.minuto_en(e.botones.raton.1) - i64::from(b.inicio),
                                _ => 0,
                            };
                            e.bloque = Some(i);
                            e.foco = None;
                            e.agarre = Some(Agarre::Bloque {
                                i,
                                desde: e.botones.raton,
                                off,
                                arrastrando: false,
                            });
                            ventana.capturar_raton();
                        }
                        Accion::Borde(i) => {
                            e.bloque = Some(i);
                            e.agarre = Some(Agarre::Borde { i });
                            ventana.capturar_raton();
                        }
                        a => vivo = hacer(&mut e, a, textos, idioma, ubicacion, aparato),
                    }
                }
                EventoOverlay::BotonDerechoPulsado(p) => {
                    e.botones.raton = local(p, marco);
                    if let Some(Accion::Bloque(i) | Accion::Borde(i)) = e.botones.bajo_el_raton()
                        && e.edicion.is_none()
                    {
                        e.borrar_bloque(i, &raiz, textos);
                    }
                }
                EventoOverlay::BotonSoltado(p) => {
                    e.botones.raton = local(p, marco);
                    if e.moviendo.take().is_some() {
                        ventana.soltar_raton();
                    }
                    if let Some(ag) = e.agarre {
                        ventana.soltar_raton();
                        soltar(&mut e, ag, &raiz, textos);
                    }
                }
                EventoOverlay::Caracter(c) => {
                    if let Some(f) = e.foco {
                        e.campo_mut(f).letra(c);
                    }
                }
                EventoOverlay::Tecla {
                    vk, shift, ctrl, ..
                } => vivo = tecla(&mut e, vk, shift, ctrl, textos, idioma, ubicacion, aparato),
                _ => {}
            }
        }
        if !vivo {
            break;
        }

        // Cada segundo: el horario cambiado fuera, las tareas, el minuto.
        if mirado.elapsed() >= Duration::from_secs(1) {
            mirado = Instant::now();
            e.ahora_ms = pixpin_shell::entorno::ahora_local_ms();
            if e.ahora_ms / 60_000 != minuto {
                minuto = e.ahora_ms / 60_000;
                pintar = true;
            }
            if e.agarre.is_none() && firma_de(&super::ruta(&raiz)) != e.firma {
                tracing::info!("horario: cambio fuera de la ventana, se relee");
                e.cargar(&raiz);
                pintar = true;
            }
            if tareas_leidas && super::asignaturas::firma(&raiz, e.chat.as_deref()) != e.firma_chat {
                e.cargar_tareas(&raiz);
                pintar = true;
            }
        }
        if let Some(a) = &e.aviso
            && a.caducado()
        {
            e.aviso = None;
            e.deshacer = None;
            pintar = true;
        }

        if pintar {
            if let Ok(dc) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&dc, |p: &Pintor| pintar_todo(&mut e, p, marco, escala, textos));
                let _ = superficie.presentar();
            }
            pintar = false;
            if primera {
                primera = false;
                tracing::info!(
                    ms = pedida.elapsed().as_millis() as u64,
                    asignaturas = e.semana.asignaturas.len(),
                    clases = e.semana.bloques.len(),
                    "horario: ventana a la vista"
                );
            }
        }
        // Las tareas, despues de lo primero visible: leer el chat no retrasa
        // abrir.
        if !tareas_leidas {
            tareas_leidas = true;
            e.cargar_tareas(&raiz);
            pintar = true;
            continue;
        }
        let espera = if e.agarre.is_some() {
            16
        } else if e.aviso.is_some() {
            250
        } else {
            1_000
        };
        pixpin_shell::overlay::esperar_eventos(Some(espera));
    }
    Ok(())
}

/// Lo que pasa al soltar el raton con algo agarrado.
fn soltar(e: &mut Estado, ag: Agarre, raiz: &Path, textos: &Catalogo) {
    let previa = e.previa();
    e.agarre = None;
    match ag {
        Agarre::Asignatura {
            i,
            arrastrando: false,
            ..
        } => {
            // Un clic: elegirla (y otro, soltarla).
            e.elegida = if e.elegida == Some(i) { None } else { Some(i) };
            e.foco = None;
        }
        Agarre::Bloque {
            arrastrando: false, ..
        } => {}
        Agarre::Asignatura { .. } => {
            if let Some(s) = previa {
                e.bloque = Some(s.bloques.len() - 1);
                e.cambiar(s, raiz, textos);
            }
        }
        Agarre::Bloque { .. } | Agarre::Borde { .. } => {
            if let Some(s) = previa {
                e.cambiar(s, raiz, textos);
            }
        }
    }
}

/// Una tecla. Devuelve si la ventana sigue.
#[allow(clippy::too_many_arguments)] // estado, tecla, modificadores y lo de la ventana
fn tecla(
    e: &mut Estado,
    vk: u32,
    shift: bool,
    ctrl: bool,
    textos: &Catalogo,
    idioma: Idioma,
    ubicacion: &Ubicacion,
    aparato: &str,
) -> bool {
    let raiz = ubicacion.raiz();
    if vk == VK_Z && ctrl && e.foco.is_none() {
        return hacer(e, Accion::Deshacer, textos, idioma, ubicacion, aparato);
    }
    if vk == VK_ESCAPE {
        if e.edicion.is_some() {
            e.edicion = None;
            e.foco = None;
        } else if e.foco.is_some() {
            e.foco = None;
        } else if e.bloque.is_some() || e.elegida.is_some() {
            e.bloque = None;
            e.elegida = None;
        } else {
            return false;
        }
        return true;
    }
    match e.foco {
        Some(CampoId::Nueva) if vk == VK_ENTRAR => {
            return hacer(e, Accion::Anadir, textos, idioma, ubicacion, aparato);
        }
        Some(CampoId::Nota) if vk == VK_ENTRAR && !ctrl => {
            e.nota.tecla(vk, ctrl, shift, true);
        }
        Some(_) if vk == VK_ENTRAR && e.edicion.is_some() => e.guardar_edicion(raiz, textos),
        Some(f) if vk == VK_TAB && e.edicion.is_some() => {
            const ORDEN: [CampoId; 3] = [CampoId::Aula, CampoId::Nota, CampoId::Avisar];
            let solo_aula = matches!(e.edicion, Some(Edicion::Asignatura(_)));
            let n = if solo_aula { 1 } else { 3 };
            let k = ORDEN.iter().position(|c| *c == f).unwrap_or(0);
            e.foco = Some(ORDEN[if shift { (k + n - 1) % n } else { (k + 1) % n }]);
        }
        Some(f) => {
            let multilinea = f == CampoId::Nota;
            e.campo_mut(f).tecla(vk, ctrl, shift, multilinea);
        }
        None if vk == VK_SUPRIMIR => {
            if let Some(i) = e.bloque {
                e.borrar_bloque(i, raiz, textos);
            }
        }
        None if vk == VK_ENTRAR => {
            if let Some(i) = e.bloque {
                e.editar(Edicion::Bloque(i));
            }
        }
        None => {}
    }
    true
}

/// Lo que hace un clic. Devuelve si la ventana sigue.
fn hacer(
    e: &mut Estado,
    a: Accion,
    textos: &Catalogo,
    idioma: Idioma,
    ubicacion: &Ubicacion,
    aparato: &str,
) -> bool {
    let raiz = ubicacion.raiz();
    match a {
        Accion::Fondo => {
            if e.edicion.is_none() {
                e.foco = None;
            }
        }
        Accion::Mover | Accion::Asignatura(_) | Accion::Bloque(_) | Accion::Borde(_) => {}
        Accion::Cerrar => return false,
        Accion::Domingo => e.domingo = !e.domingo,
        Accion::ComoTexto => super::abrir_nota(idioma, ubicacion.clone()),
        Accion::Campo(c) => e.foco = Some(c),
        Accion::Anadir => {
            let escrito = e.nueva.texto().to_string();
            let mut s = e.semana.clone();
            match s.anadir_asignatura(&escrito) {
                Ok(Some(i)) => {
                    let nombre = s.asignaturas[i].nombre.clone();
                    e.cambiar(s, raiz, textos);
                    e.nueva.poner("");
                    e.elegida = Some(i);
                    // Su grupo de tareas, ya: asi sale en el desplegable de
                    // Tareas y en el chat.
                    match super::asignaturas::asegurar_lista(raiz, aparato, &nombre) {
                        Ok(_) => {
                            crate::ventana_chat::refrescar();
                            crate::tareas::refrescar();
                            e.cargar_tareas(raiz);
                        }
                        Err(err) => tracing::warn!(?err, "horario: sin lista para la asignatura"),
                    }
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("asignatura", nombre);
                    e.decir(textos.t_args("horario-arrastra", &args));
                }
                Ok(None) => {}
                Err(i) => {
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("asignatura", e.semana.asignaturas[i].nombre.clone());
                    e.decir(textos.t_args("horario-repetida", &args));
                    e.elegida = Some(i);
                }
            }
        }
        Accion::Color(i) => {
            let mut s = e.semana.clone();
            s.cambiar_color(i);
            e.cambiar(s, raiz, textos);
        }
        Accion::BorrarAsignatura(i) => {
            let mut s = e.semana.clone();
            if let Some(a) = s.borrar_asignatura(i) {
                e.elegida = None;
                e.bloque = None;
                e.cambiar(s, raiz, textos);
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("asignatura", a.nombre);
                e.decir_con_deshacer(textos.t_args("horario-asignatura-borrada", &args), textos);
            }
        }
        Accion::Rejilla => {
            e.bloque = None;
            e.foco = None;
        }
        Accion::Tarea(k) => {
            if let Some((l, f)) = e.pendientes.get(k).cloned() {
                match crate::tareas::marcar(raiz, &l, &f, true) {
                    Ok(_) => {
                        crate::ventana_chat::refrescar();
                        crate::tareas::refrescar();
                    }
                    Err(err) => tracing::warn!(?err, "horario: no se pudo marcar la tarea"),
                }
                e.cargar_tareas(raiz);
            }
        }
        Accion::EventoGoogle(i) => {
            if let Some(ev) = e.google.get(i).filter(|ev| !ev.enlace.is_empty()) {
                let _ = pixpin_shell::abrir(std::path::Path::new(&ev.enlace));
            }
        }
        Accion::AbrirTareas => {
            let nombre = e
                .elegida
                .and_then(|i| e.semana.asignaturas.get(i))
                .map(|a| a.nombre.clone());
            let clave = nombre.and_then(|n| super::asignaturas::de(&e.listas, &n).map(Lista::clave));
            match clave {
                Some(k) => crate::tareas::abrir_en(idioma, ubicacion.clone(), aparato, k),
                None => crate::tareas::abrir(idioma, ubicacion.clone(), aparato),
            }
        }
        Accion::Guardar => e.guardar_edicion(raiz, textos),
        Accion::Cancelar => {
            e.edicion = None;
            e.foco = None;
        }
        Accion::BorrarBloque => {
            if let Some(Edicion::Bloque(i)) = e.edicion {
                e.borrar_bloque(i, raiz, textos);
            }
        }
        Accion::Deshacer => {
            if let Some(s) = e.deshacer.take() {
                e.semana = s;
                e.bloque = None;
                e.elegida = e.elegida.filter(|&i| i < e.semana.asignaturas.len());
                e.guardar(raiz, textos);
                e.aviso = None;
            }
        }
    }
    true
}

// --------------------------------------------------------------- pintar

/// El color de una asignatura, para pintarlo.
fn color(rgb: u32) -> Color {
    hex(rgb)
}

/// Lo que mide de alto lo de arriba de la lateral y de la rejilla.
fn arriba(s: f32) -> f32 {
    crate::cabecera::ALTO * s + d::HUECO * s
}

/// Donde queda la rejilla en una ventana de `w` x `h` (pixeles), con
/// `dias` dias.
fn rejilla_en(w: f32, h: f32, s: f32, dias: u8) -> Rejilla {
    let x0 = d::MARGEN * s + LATERAL * s + d::MARGEN * s + COLUMNA_HORAS * s;
    let y0 = arriba(s) + CABECERA_DIAS * s;
    let ancho = (w - d::MARGEN * s - x0).max(60.0);
    let alto = (h - d::MARGEN * s - y0).max(60.0);
    Rejilla {
        x: x0,
        y: y0,
        ancho_dia: ancho / f32::from(dias),
        alto_franja: alto / semana::FRANJAS as f32,
        dias,
    }
}

fn pintar_todo(e: &mut Estado, p: &Pintor, marco: Rect, s: f32, textos: &Catalogo) {
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    p.limpiar(FONDO);
    e.botones.vaciar();
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: h,
    };
    e.botones.zona(todo, Accion::Fondo);
    let r = rejilla_en(w, h, s, if e.domingo { 7 } else { 6 });
    e.rejilla = Some(r);
    let sem = e.previa().unwrap_or_else(|| e.semana.clone());

    pintar_rejilla(e, p, &sem, r, s);
    pintar_lateral(e, p, h, s, textos);

    // La barra de arriba comun, con el domingo y «Como texto».
    let titulo = textos.t("horario-titulo");
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("asignaturas", e.semana.asignaturas.len());
    args.set("clases", e.semana.bloques.len());
    let subtitulo = textos.t_args("horario-subtitulo", &args);
    let domingo = textos.t("horario-domingo");
    let como_texto = textos.t("horario-como-texto");
    let cab = crate::cabecera::Cabecera {
        titulo: &titulo,
        subtitulo: &subtitulo,
        buscador: None,
        botones: vec![
            crate::cabecera::Boton {
                icono: Some(&mi::CALENDAR_MONTH),
                rotulo: &domingo,
                chapa: None,
                activo: e.domingo,
                accion: Accion::Domingo,
            },
            crate::cabecera::Boton {
                icono: Some(&mi::EDIT),
                rotulo: &como_texto,
                chapa: None,
                activo: false,
                accion: Accion::ComoTexto,
            },
        ],
        mover: Accion::Mover,
        cerrar: Accion::Cerrar,
    };
    crate::cabecera::pintar(p, &mut e.botones, w, s, &cab);

    // Lo que se arrastra desde la lista, fuera de la rejilla: su chapa.
    if let Some(Agarre::Asignatura {
        i,
        arrastrando: true,
        ..
    }) = e.agarre
        && r.en(e.botones.raton).is_none()
        && let Some(a) = e.semana.asignaturas.get(i)
    {
        let tam = d::LETRA_SECUNDARIO * s;
        let (tw, th) = p.medir_texto(&a.nombre, tam);
        let caja = RectF {
            x: e.botones.raton.0 + 10.0 * s,
            y: e.botones.raton.1 - (th + 12.0 * s) / 2.0,
            ancho: tw + 24.0 * s,
            alto: th + 12.0 * s,
        };
        p.rellenar_redondeado(caja, d::RADIO_BOTON * s, color(e.semana.color_de(&a.nombre)));
        p.texto(&a.nombre, caja.x + 12.0 * s, caja.y + 6.0 * s, tam, Color::BLANCO);
    }

    if e.edicion.is_some() {
        pintar_ficha(e, p, w, h, s, textos);
    }
    if let Some(a) = &e.aviso {
        crate::v2::aviso::pintar(a, p, &mut e.botones, todo, s);
    }
}

/// Lo de debajo del nombre de una asignatura: su aula y sus horas a la
/// semana («A-12 · 4 h/sem», «1,5 h/sem»).
fn subtitulo_de_asignatura(s: &Semana, a: &super::Asignatura) -> String {
    let minutos: u32 = s
        .bloques
        .iter()
        .filter(|b| b.nombre == a.nombre)
        .map(|b| b.fin.saturating_sub(b.inicio))
        .sum();
    let horas = if minutos == 0 {
        None
    } else if minutos % 60 == 0 {
        Some(format!("{} h/sem", minutos / 60))
    } else {
        Some(format!("{:.2} h/sem", minutos as f32 / 60.0).replace(".50", ",5").replace(".25", ",25").replace(".75", ",75"))
    };
    match (a.aula.as_deref(), horas) {
        (Some(aula), Some(h)) => format!("{aula} · {h}"),
        (Some(aula), None) => aula.to_string(),
        (None, Some(h)) => h,
        (None, None) => String::new(),
    }
}

/// La lista de asignaturas y, debajo, las tareas de la elegida.
fn pintar_lateral(e: &mut Estado, p: &Pintor, h: f32, s: f32, textos: &Catalogo) {
    let x = d::MARGEN * s;
    let ancho = LATERAL * s;
    let mut y = arriba(s);
    // La caja «Nueva asignatura» con su «+».
    let alto_caja = d::BOTON * s;
    let caja = RectF {
        x,
        y,
        ancho,
        alto: alto_caja,
    };
    let foco = e.foco == Some(CampoId::Nueva);
    let lado = d::BOTON * s;
    e.nueva.pintar(
        p,
        caja,
        d::LETRA_CUERPO * s,
        foco,
        &textos.t("horario-nueva"),
        lado,
        s,
    );
    e.botones.zona(caja, Accion::Campo(CampoId::Nueva));
    let mas = RectF {
        x: x + ancho - lado,
        y,
        ancho: lado,
        alto: lado,
    };
    crate::v2::boton_icono(p, &mut e.botones, mas, Accion::Anadir, &mi::ADD, false, false, s);
    y += alto_caja + d::HUECO * s;

    // Las asignaturas.
    let tam = d::LETRA_CUERPO * s;
    let tam2 = 12.0 * s;
    if e.semana.asignaturas.is_empty() {
        p.texto_ajustado(&textos.t("horario-vacio"), x + 4.0 * s, y + 8.0 * s, d::LETRA_SECUNDARIO * s, ancho - 8.0 * s, GRIS);
        y += 60.0 * s;
    }
    let arrastrada = match e.agarre {
        Some(Agarre::Asignatura {
            i,
            arrastrando: true,
            ..
        }) => Some(i),
        _ => None,
    };
    let cuantas = e.semana.asignaturas.len();
    e.primera = e.primera.min(cuantas.saturating_sub(1));
    if e.primera > 0 {
        // Las de arriba que no se ven.
        p.texto_linea(&format!("↑ {}", e.primera), x + 36.0 * s, y, 12.0 * s, 80.0 * s, GRIS);
        y += 20.0 * s;
    }
    for i in e.primera..cuantas {
        let a = e.semana.asignaturas[i].clone();
        let fila = RectF {
            x,
            y,
            ancho,
            alto: FILA * s,
        };
        let encima = dentro(fila, e.botones.raton) && e.agarre.is_none();
        let fondo = if e.elegida == Some(i) || arrastrada == Some(i) {
            Some(con_alfa(ACENTO, 0.18))
        } else if encima {
            Some(blanco(0.06))
        } else {
            None
        };
        if let Some(f) = fondo {
            p.rellenar_redondeado(fila, d::RADIO_BOTON * s, f);
        }
        e.botones.zona(fila, Accion::Asignatura(i));
        // El circulo de color: pulsarlo pasa al siguiente.
        let c = (x + 18.0 * s, y + fila.alto / 2.0);
        p.circulo(c, 7.0 * s, color(e.semana.color_de(&a.nombre)));
        e.botones.zona(cuadrado(c, 28.0 * s), Accion::Color(i));
        let tx = x + 36.0 * s;
        let derecha = if encima { 44.0 * s } else { 8.0 * s };
        let (_, th) = p.medir_texto("Ag", tam);
        let sub = subtitulo_de_asignatura(&e.semana, &a);
        if sub.is_empty() {
            p.texto_linea(&a.nombre, tx, y + (fila.alto - th) / 2.0, tam, ancho - (tx - x) - derecha, TEXTO);
        } else {
            let (_, th2) = p.medir_texto("Ag", tam2);
            let y0 = y + (fila.alto - th - th2) / 2.0;
            p.texto_linea(&a.nombre, tx, y0, tam, ancho - (tx - x) - derecha, TEXTO);
            p.texto_linea(&sub, tx, y0 + th, tam2, ancho - (tx - x) - derecha, GRIS);
        }
        if encima {
            let b = RectF {
                x: x + ancho - 40.0 * s,
                y: y + (fila.alto - 40.0 * s) / 2.0,
                ancho: 40.0 * s,
                alto: 40.0 * s,
            };
            crate::v2::boton_icono(p, &mut e.botones, b, Accion::BorrarAsignatura(i), &mi::DELETE, false, false, s);
        }
        y += FILA * s;
        if y > h - 200.0 * s && i + 1 < cuantas {
            // Lo que no cabe: cuantas quedan (la rueda las trae).
            p.texto_linea(&format!("↓ {}", cuantas - i - 1), tx, y, 12.0 * s, 80.0 * s, GRIS);
            y += 24.0 * s;
            break;
        }
    }

    // Las tareas pendientes de la elegida.
    e.pendientes = e.pendientes_de_la_elegida();
    let Some(a) = e.elegida.and_then(|i| e.semana.asignaturas.get(i)).cloned() else {
        return;
    };
    y += d::HUECO_GRANDE * s;
    p.rellenar(
        RectF {
            x,
            y,
            ancho,
            alto: 1.0 * s.max(1.0),
        },
        blanco(0.07),
    );
    y += d::HUECO_GRANDE * s;
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("asignatura", a.nombre.clone());
    let titulo = format!("{} ({})", textos.t_args("horario-tareas-de", &args), e.pendientes.len());
    p.texto_linea(&titulo, x + 4.0 * s, y, d::LETRA_SECUNDARIO * s, ancho, GRIS);
    y += 24.0 * s;
    if e.pendientes.is_empty() {
        p.texto_ajustado(&textos.t("horario-sin-tareas"), x + 4.0 * s, y, d::LETRA_SECUNDARIO * s, ancho - 8.0 * s, GRIS);
        y += 48.0 * s;
    }
    let pendientes = e.pendientes.clone();
    for (k, (_, f)) in pendientes.iter().enumerate().take(TAREAS_A_LA_VISTA) {
        if y > h - 2.0 * d::BOTON * s {
            break;
        }
        let fila = 34.0 * s;
        let casilla = RectF {
            x,
            y: y + (fila - 32.0 * s) / 2.0,
            ancho: 32.0 * s,
            alto: 32.0 * s,
        };
        let sobre = dentro(casilla, e.botones.raton);
        p.icono(
            if sobre { &mi::CHECK_BOX } else { &mi::CHECK_BOX_OUTLINE_BLANK },
            encoger(casilla, 7.0 * s),
            if sobre { ACENTO } else { GRIS },
        );
        e.botones.zona(casilla, Accion::Tarea(k));
        let (_, th) = p.medir_texto("Ag", tam);
        p.texto_linea(&f.texto, x + 36.0 * s, y + (fila - th) / 2.0, tam, ancho - 40.0 * s, d::CUERPO);
        y += fila;
    }
    y += d::HUECO * s;
    let rotulo = textos.t("horario-abrir-tareas");
    let bw = ui::ancho_de_boton(p, true, &rotulo, None, s);
    let b = RectF {
        x,
        y,
        ancho: bw.min(ancho),
        alto: d::BOTON * s,
    };
    ui::boton_v2(p, &mut e.botones, b, Accion::AbrirTareas, Some(&mi::CHECKLIST), &rotulo, None, Some(d::CAJA), TEXTO, s);
}

/// La semana: los dias arriba, las horas a la izquierda, las clases y la
/// rayita de ahora.
fn pintar_rejilla(e: &mut Estado, p: &Pintor, sem: &Semana, r: Rejilla, s: f32) {
    let dia_hoy = e.ahora_ms.div_euclid(86_400_000);
    let semana_hoy = fechas::dia_de_la_semana(dia_hoy) as u8;
    // (En domingo, sin la columna del domingo, la semana que empieza mañana:
    // la pasada ya no sirve para nada, 18-oct.)
    let lunes = if semana_hoy >= r.dias { dia_hoy + 7 - i64::from(semana_hoy) } else { dia_hoy - i64::from(semana_hoy) };
    let ancho = r.ancho_dia * f32::from(r.dias);
    let zona = RectF {
        x: r.x,
        y: r.y,
        ancho,
        alto: r.alto(),
    };
    p.rellenar_redondeado(encoger(zona, -2.0 * s), d::RADIO_BOTON * s, con_alfa(d::TARJETA, 0.6));
    e.botones.zona(zona, Accion::Rejilla);
    // Hoy, un poco mas claro.
    if semana_hoy < r.dias {
        p.rellenar(r.caja(semana_hoy, semana::DESDE, semana::HASTA), blanco(0.03));
    }
    // Las horas: raya entera en punto, tenue a la media.
    let tam_hora = 11.5 * s;
    let mut m = semana::DESDE;
    while m <= semana::HASTA {
        let y = r.y_de(m);
        let en_punto = m % 60 == 0;
        p.linea((r.x, y), (r.x + ancho, y), 1.0, blanco(if en_punto { 0.08 } else { 0.035 }));
        if en_punto {
            let t = format!("{}:00", m / 60);
            let (tw, th) = p.medir_texto(&t, tam_hora);
            let ty = (y - th / 2.0).max(r.y - th / 2.0);
            p.texto(&t, r.x - 8.0 * s - tw, ty, tam_hora, GRIS);
        }
        m += 30;
    }
    // Los dias.
    for dia in 0..r.dias {
        let x = r.x + f32::from(dia) * r.ancho_dia;
        if dia > 0 {
            p.linea((x, r.y), (x, r.y + r.alto()), 1.0, blanco(0.06));
        }
        let (_, _, dd) = fechas::fecha_de(lunes + i64::from(dia));
        let t = format!("{} {dd}", semana::DIAS[usize::from(dia)]);
        let tam = d::LETRA_SECUNDARIO * s;
        let (tw, th) = p.medir_texto(&t, tam);
        let cx = x + r.ancho_dia / 2.0;
        let cy = r.y - CABECERA_DIAS * s / 2.0;
        if dia == semana_hoy {
            let pastilla = RectF {
                x: cx - tw / 2.0 - 10.0 * s,
                y: cy - th / 2.0 - 4.0 * s,
                ancho: tw + 20.0 * s,
                alto: th + 8.0 * s,
            };
            p.rellenar_redondeado(pastilla, pastilla.alto / 2.0, d::AZUL_LLENO);
            p.texto(&t, cx - tw / 2.0, cy - th / 2.0, tam, Color::BLANCO);
        } else {
            p.texto(&t, cx - tw / 2.0, cy - th / 2.0, tam, d::CUERPO);
        }
    }
    // **Los eventos de Google Calendar** de esta semana (18-oct), debajo de
    // las clases: translucidos, con la raya de su color a la izquierda.
    // Son de Google: no se mueven; pulsarlos los abre alli.
    let desfase = pixpin_shell::entorno::ahora_local_ms() - pixpin_shell::entorno::ahora_utc_ms();
    let lunes_ms = lunes * 86_400_000;
    let fin_semana_ms = lunes_ms + i64::from(r.dias) * 86_400_000;
    e.google = crate::calendario_google::eventos_entre(lunes_ms - desfase - 86_400_000, fin_semana_ms - desfase + 86_400_000);
    for (k, ev) in e.google.clone().iter().enumerate() {
        // En hora local (uno de todo el dia ya es una fecha).
        let (ini, fin) = if ev.todo_el_dia { (ev.inicio_ms, ev.fin_ms) } else { (ev.inicio_ms + desfase, ev.fin_ms + desfase) };
        let dia = ini.div_euclid(86_400_000) - lunes;
        if dia < 0 || dia >= i64::from(r.dias) {
            continue;
        }
        let tinte = u32::from_str_radix(ev.color.trim_start_matches('#'), 16).map(crate::caja_dibujo::hex).unwrap_or(ACENTO);
        let caja = if ev.todo_el_dia {
            // Arriba del todo de su dia, una tira.
            let c = r.caja(dia as u8, semana::DESDE, semana::HASTA);
            RectF { x: c.x + 2.0 * s, y: c.y + 2.0 * s, ancho: c.ancho - 4.0 * s, alto: 18.0 * s }
        } else {
            let m0 = (ini.rem_euclid(86_400_000) / 60_000) as u32;
            let m1 = if fin.div_euclid(86_400_000) > ini.div_euclid(86_400_000) { semana::HASTA } else { (fin.rem_euclid(86_400_000) / 60_000) as u32 };
            if m1 <= semana::DESDE || m0 >= semana::HASTA {
                continue;
            }
            encoger(r.caja(dia as u8, m0, m1.max(m0 + 15)), 1.5 * s)
        };
        p.rellenar_redondeado(caja, 6.0 * s, con_alfa(tinte, 0.30));
        p.rellenar(RectF { ancho: 3.0 * s, ..caja }, tinte);
        p.con_recorte(caja, |p| {
            let tam = 12.0 * s;
            let (_, th) = p.medir_texto("Ag", tam);
            let tx = caja.x + 8.0 * s;
            p.texto_linea(&ev.titulo, tx, caja.y + (if ev.todo_el_dia { (caja.alto - th) / 2.0 } else { 3.0 * s }), tam, caja.ancho - 12.0 * s, TEXTO);
            if !ev.todo_el_dia && caja.alto > th * 2.2 {
                let mut d = crate::calendario_google::tramo_legible(ev);
                if !ev.lugar.is_empty() {
                    d.push_str(" · ");
                    d.push_str(&ev.lugar);
                }
                p.texto_linea(&d, tx, caja.y + 3.0 * s + th, 11.0 * s, caja.ancho - 12.0 * s, GRIS);
            }
        });
        e.botones.zona(caja, Accion::EventoGoogle(k));
    }
    // Las clases.
    let movida = match e.agarre {
        Some(Agarre::Bloque {
            i,
            arrastrando: true,
            ..
        })
        | Some(Agarre::Borde { i }) => Some(i),
        Some(Agarre::Asignatura {
            arrastrando: true, ..
        }) if sem.bloques.len() > e.semana.bloques.len() => Some(sem.bloques.len() - 1),
        _ => None,
    };
    for (i, b) in sem.bloques.iter().enumerate() {
        if b.dia >= r.dias || b.fin <= semana::DESDE || b.inicio >= semana::HASTA {
            continue;
        }
        let caja = encoger(r.caja(b.dia, b.inicio, b.fin), 1.5 * s);
        let base = color(sem.color_de(&b.nombre));
        let alfa = if movida == Some(i) { 0.75 } else { 0.92 };
        p.rellenar_redondeado(caja, 6.0 * s, con_alfa(base, alfa));
        if e.bloque == Some(i) || movida == Some(i) {
            p.trazar(encoger(caja, -1.0 * s), 2.0 * s, TEXTO);
        }
        p.con_recorte(caja, |p| {
            let tam = 13.0 * s;
            let tam2 = 11.5 * s;
            let (_, th) = p.medir_texto("Ag", tam);
            let tx = caja.x + 6.0 * s;
            let mut ty = caja.y + 3.0 * s;
            let anch = caja.ancho - 10.0 * s;
            p.texto_linea(&b.nombre, tx, ty, tam, anch, Color::BLANCO);
            ty += th;
            let mut detalle = format!(
                "{}:{:02}–{}:{:02}",
                b.inicio / 60,
                b.inicio % 60,
                b.fin / 60,
                b.fin % 60
            );
            if let Some(a) = &b.aula {
                detalle.push_str(" · ");
                detalle.push_str(a);
            }
            if ty + th * 0.8 < caja.y + caja.alto {
                p.texto_linea(&detalle, tx, ty, tam2, anch, con_alfa(Color::BLANCO, 0.85));
                ty += th * 0.9;
            }
            if !b.notas.is_empty() && ty + th * 0.8 < caja.y + caja.alto {
                p.texto_linea(&format!("🔊 {}", b.notas[0]), tx, ty, tam2, anch, con_alfa(Color::BLANCO, 0.75));
            }
        });
        if i < e.semana.bloques.len() {
            e.botones.zona(caja, Accion::Bloque(i));
            let borde = RectF {
                y: caja.y + caja.alto - BORDE * s,
                alto: BORDE * s,
                ..caja
            };
            if dentro(caja, e.botones.raton) && e.agarre.is_none() {
                // La agarradera del borde: una rayita blanca en medio.
                let cx = caja.x + caja.ancho / 2.0;
                p.linea(
                    (cx - 10.0 * s, caja.y + caja.alto - 3.5 * s),
                    (cx + 10.0 * s, caja.y + caja.alto - 3.5 * s),
                    2.0 * s,
                    con_alfa(Color::BLANCO, 0.8),
                );
            }
            e.botones.zona(borde, Accion::Borde(i));
        }
    }
    // La hora de ahora.
    let minuto = (e.ahora_ms.rem_euclid(86_400_000) / 60_000) as u32;
    if semana_hoy < r.dias && (semana::DESDE..semana::HASTA).contains(&minuto) {
        let c = r.caja(semana_hoy, semana::DESDE, semana::HASTA);
        let y = r.y_de(minuto);
        p.linea((c.x, y), (c.x + c.ancho, y), 2.0 * s, AHORA);
        p.circulo((c.x, y), 4.5 * s, AHORA);
    }
}

/// La ficha de una clase (o de una asignatura) encima de todo.
fn pintar_ficha(e: &mut Estado, p: &Pintor, w: f32, h: f32, s: f32, textos: &Catalogo) {
    let Some(ed) = e.edicion else {
        return;
    };
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: h,
    };
    p.rellenar(todo, negro(0.45));
    e.botones.zona(todo, Accion::Fondo);
    let (titulo, sub, solo_aula) = match ed {
        Edicion::Bloque(i) => match e.semana.bloques.get(i) {
            Some(b) => (
                b.nombre.clone(),
                format!(
                    "{} {}:{:02}–{}:{:02}",
                    semana::DIAS[usize::from(b.dia.min(6))],
                    b.inicio / 60,
                    b.inicio % 60,
                    b.fin / 60,
                    b.fin % 60
                ),
                false,
            ),
            None => return,
        },
        Edicion::Asignatura(i) => match e.semana.asignaturas.get(i) {
            Some(a) => (a.nombre.clone(), String::new(), true),
            None => return,
        },
    };
    let ancho = 440.0 * s;
    let alto = if solo_aula { 230.0 } else { 430.0 } * s;
    let caja = RectF {
        x: (w - ancho) / 2.0,
        y: (h - alto) / 2.0,
        ancho,
        alto,
    };
    p.rellenar_redondeado(encoger(caja, -1.0 * s), (d::RADIO_FLOTANTE + 1.0) * s, blanco(0.10));
    p.rellenar_redondeado(caja, d::RADIO_FLOTANTE * s, d::TARJETA);
    e.botones.zona(caja, Accion::Fondo);
    let x = caja.x + 20.0 * s;
    let dentro_ancho = ancho - 40.0 * s;
    let mut y = caja.y + 18.0 * s;
    p.texto_linea(&titulo, x, y, d::LETRA_TITULO_TARJETA * s + 2.0 * s, dentro_ancho, TEXTO);
    y += 26.0 * s;
    if !sub.is_empty() {
        p.texto_linea(&sub, x, y, d::LETRA_SECUNDARIO * s, dentro_ancho, GRIS);
        y += 22.0 * s;
    }
    y += 6.0 * s;
    let campos: &[(CampoId, &str, usize)] = if solo_aula {
        &[(CampoId::Aula, "horario-aula", 1)]
    } else {
        &[
            (CampoId::Aula, "horario-aula", 1),
            (CampoId::Nota, "horario-nota", 3),
            (CampoId::Avisar, "horario-avisar", 1),
        ]
    };
    let tam = d::LETRA_CUERPO * s;
    for &(c, rotulo, lineas) in campos {
        p.texto_linea(&textos.t(rotulo), x, y, 12.0 * s, dentro_ancho, GRIS);
        y += 18.0 * s;
        let foco = e.foco == Some(c);
        let pista = if c == CampoId::Avisar {
            let mut a = fluent_bundle::FluentArgs::new();
            a.set("min", e.antelacion);
            textos.t_args("horario-avisar-pista", &a)
        } else {
            String::new()
        };
        let campo = match c {
            CampoId::Aula => &e.aula,
            CampoId::Nota => &e.nota,
            CampoId::Avisar => &e.avisar,
            CampoId::Nueva => &e.nueva,
        };
        let alto_campo = campo.alto(p, dentro_ancho, tam, lineas, 0.0, s);
        let r = RectF {
            x,
            y,
            ancho: dentro_ancho,
            alto: alto_campo,
        };
        campo.pintar(p, r, tam, foco, &pista, 0.0, s);
        e.botones.zona(r, Accion::Campo(c));
        y += alto_campo + 12.0 * s;
    }
    // Los botones, abajo a la derecha (y Borrar a la izquierda).
    let by = caja.y + caja.alto - 20.0 * s - d::BOTON * s;
    let guardar = textos.t("horario-guardar");
    let cancelar = textos.t("horario-cancelar");
    let gw = ui::ancho_de_boton(p, false, &guardar, Some("Intro"), s);
    let cw = ui::ancho_de_boton(p, false, &cancelar, Some("Esc"), s);
    let g = RectF {
        x: caja.x + caja.ancho - 20.0 * s - gw,
        y: by,
        ancho: gw,
        alto: d::BOTON * s,
    };
    ui::boton_v2(p, &mut e.botones, g, Accion::Guardar, None, &guardar, Some("Intro"), Some(d::AZUL_LLENO), Color::BLANCO, s);
    let c = RectF {
        x: g.x - 8.0 * s - cw,
        ancho: cw,
        ..g
    };
    ui::boton_v2(p, &mut e.botones, c, Accion::Cancelar, None, &cancelar, Some("Esc"), Some(d::CAJA), TEXTO, s);
    if !solo_aula {
        let borrar = textos.t("horario-borrar");
        let bw = ui::ancho_de_boton(p, true, &borrar, None, s);
        let b = RectF {
            x,
            ancho: bw,
            ..g
        };
        ui::boton_v2(p, &mut e.botones, b, Accion::BorrarBloque, Some(&mi::DELETE), &borrar, None, None, d::ROJO_TEXTO, s);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const HORARIO: &str = "# Horario

## Asignaturas
Asignatura Geometría @A-12 #0A84FF
Asignatura Física
Asignatura Dibujo técnico @(Lab de dibujo)

## Clases
Lun Mié 8:00-10:00 Geometría @A-12
> traer el compás
Mar Jue 10:15-12:00 Física avisar 20
Vie 16:00-18:00 Dibujo técnico @(Lab de dibujo)

avisar 10 min antes
";

    /// La rejilla de la ventana tal como se pinta: lo que se suelta en una
    /// franja cae en ese dia y esa hora.
    #[test]
    fn en_la_rejilla_de_la_ventana_cada_franja_es_su_dia_y_su_hora() {
        let r = rejilla_en(1180.0, 780.0, 1.0, 6);
        assert!(r.ancho_dia > 100.0 && r.alto_franja > 9.0, "{r:?}");
        // El centro de la franja del jueves a las 10:30.
        let c = r.caja(3, 10 * 60 + 30, 10 * 60 + 45);
        let (dia, minuto) = r.en((c.x + c.ancho / 2.0, c.y + c.alto / 2.0)).unwrap();
        assert_eq!((dia, semana::redondear_abajo(minuto)), (3, 10 * 60 + 30));
        // La lateral no es rejilla.
        assert_eq!(r.en((d::MARGEN + 10.0, c.y)), None);
        // Con el domingo, los dias son mas estrechos y caben siete.
        let r7 = rejilla_en(1180.0, 780.0, 1.0, 7);
        assert!(r7.ancho_dia < r.ancho_dia);
        let dom = r7.caja(6, 9 * 60, 10 * 60);
        assert_eq!(r7.en((dom.x + 2.0, dom.y + 2.0)).map(|x| x.0), Some(6));
    }

    /// Arrastrar en la ventana (sin ventana): soltar una asignatura en la
    /// rejilla, mover la clase y alargarla, como lo hace el bucle.
    #[test]
    fn arrastrar_soltar_mover_y_alargar_como_en_la_ventana() {
        let mut e = Estado::nuevo();
        e.cargar_de(HORARIO);
        let r = rejilla_en(1180.0, 780.0, 1.0, 6);
        e.rejilla = Some(r);
        assert_eq!(e.semana.asignaturas.len(), 3);
        assert_eq!(e.semana.bloques.len(), 5, "Lun, Mié, Mar, Jue y Vie");
        // Física (1) soltada el sabado a las 9:10.
        let c = r.caja(5, 9 * 60 + 10, 9 * 60 + 25);
        e.botones.raton = (c.x + 5.0, c.y + 1.0);
        e.agarre = Some(Agarre::Asignatura {
            i: 1,
            desde: (0.0, 0.0),
            arrastrando: true,
        });
        let s = e.previa().unwrap();
        let b = s.bloques.last().unwrap();
        assert_eq!((b.dia, b.inicio, b.fin, b.nombre.as_str()), (5, 9 * 60, 10 * 60, "Física"));
        // La clase del lunes, cogida por su mitad y llevada al martes: la
        // mitad sigue bajo el raton.
        let lunes = e.semana.bloques.iter().position(|b| b.dia == 0).unwrap();
        let caja = r.caja(0, 8 * 60, 10 * 60);
        let off = r.minuto_en(caja.y + caja.alto / 2.0) - 8 * 60;
        let destino = r.caja(1, 14 * 60, 14 * 60 + 15);
        e.botones.raton = (destino.x + 10.0, destino.y + 1.0);
        e.agarre = Some(Agarre::Bloque {
            i: lunes,
            desde: (0.0, 0.0),
            off,
            arrastrando: true,
        });
        let s = e.previa().unwrap();
        let b = &s.bloques[lunes];
        assert_eq!((b.dia, b.inicio, b.fin), (1, 13 * 60, 15 * 60), "14:00 era su mitad");
        // El borde de abajo, hasta las 11:30.
        let borde = r.caja(0, 11 * 60 + 30, 11 * 60 + 30);
        e.botones.raton = (borde.x + 10.0, borde.y + 1.0);
        e.agarre = Some(Agarre::Borde { i: lunes });
        let s = e.previa().unwrap();
        assert_eq!(s.bloques[lunes].fin, 11 * 60 + 30);
        // Caso negativo: un clic sin arrastrar no cambia nada.
        e.agarre = Some(Agarre::Bloque {
            i: lunes,
            desde: (0.0, 0.0),
            off,
            arrastrando: false,
        });
        assert_eq!(e.previa(), None);
        // Ni soltar una asignatura fuera de la rejilla.
        e.botones.raton = (10.0, 300.0);
        e.agarre = Some(Agarre::Asignatura {
            i: 1,
            desde: (0.0, 0.0),
            arrastrando: true,
        });
        assert_eq!(e.previa(), None);
    }

    /// **Lo primero visible en menos de 0,5 s**: leer el horario, montar la
    /// semana y pintar la ventana entera (en una superficie fuera de
    /// pantalla, con la GPU de verdad). Deja la muestra en PNG.
    #[test]
    #[ignore = "necesita GPU; cargo test -- --ignored horario_ventana"]
    fn horario_ventana_se_pinta_en_menos_de_medio_segundo() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let marco = Rect {
            x: 0,
            y: 0,
            ancho: ANCHO_VENTANA,
            alto: ALTO_VENTANA,
        };
        // Por partes, como al abrir la ventana: el dispositivo grafico del
        // hilo (`Recursos::nuevos`, lo que pagan todas las ventanas v2), leer
        // el horario y montar la semana, y el primer fotograma. La primera
        // vez de cada cosa (DLL de D3D, DirectWrite y fuentes): es la que
        // cuenta.
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let raiz = std::env::temp_dir().join(format!("pixpin-horario-medir-{}", std::process::id()));
        std::fs::create_dir_all(&raiz).unwrap();
        std::fs::write(super::super::ruta(&raiz), HORARIO).unwrap();
        let t0 = Instant::now();
        let disp = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(disp.d3d()).expect("motor");
        let fuera = FueraDePantalla::nuevo(&motor, disp.d3d(), ANCHO_VENTANA, ALTO_VENTANA).expect("superficie");
        let t_disp = t0.elapsed();
        let t1 = Instant::now();
        let mut e = Estado::nuevo();
        e.cargar(&raiz);
        e.elegida = Some(0);
        let t_leer = t1.elapsed();
        let t2 = Instant::now();
        motor
            .dibujar(&fuera.destino, |p| pintar_todo(&mut e, p, marco, 1.0, &textos))
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        let t_pintar = t2.elapsed();
        let total = t0.elapsed().as_millis();
        println!(
            "horario: dispositivo {} ms, leer {} ms, primer fotograma {} ms, total {total} ms",
            t_disp.as_millis(),
            t_leer.as_millis(),
            t_pintar.as_millis()
        );
        let _ = std::fs::remove_dir_all(&raiz);
        assert!(
            (t_leer + t_pintar).as_millis() < 500,
            "lo propio de la ventana: {} ms",
            (t_leer + t_pintar).as_millis()
        );
        crate::ventanita::muestra("horario-semanal", ANCHO_VENTANA, ALTO_VENTANA, |p, _| {
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        // Con Google Calendar (18-oct): un evento el martes de 16:00 a 17:30
        // y uno de todo el dia el jueves, de la semana que se ve.
        {
            let desfase = pixpin_shell::entorno::ahora_local_ms() - pixpin_shell::entorno::ahora_utc_ms();
            let hoy = e.ahora_ms.div_euclid(86_400_000);
            let sem = fechas::dia_de_la_semana(hoy) as i64;
            let lunes = if sem >= 6 { hoy + 7 - sem } else { hoy - sem };
            let ev = |id: &str, t: &str, dia: i64, h0: i64, h1: i64, todo: bool| pixpin_ia::conexiones::calendario::Evento {
                id: id.into(),
                calendario: "Personal".into(),
                titulo: t.into(),
                inicio_ms: if todo { (lunes + dia) * 86_400_000 } else { (lunes + dia) * 86_400_000 + h0 * 60_000 - desfase },
                fin_ms: if todo { (lunes + dia + 1) * 86_400_000 } else { (lunes + dia) * 86_400_000 + h1 * 60_000 - desfase },
                todo_el_dia: todo,
                lugar: if todo { String::new() } else { "Aula 12".into() },
                color: if todo { "#d50000".into() } else { "#0b8043".into() },
                enlace: String::new(),
            };
            crate::calendario_google::poner_para_pruebas(vec![
                ev("a", "Reunión con el asesor", 1, 16 * 60, 17 * 60 + 30, false),
                ev("b", "Feriado", 3, 0, 0, true),
            ]);
            crate::ventanita::muestra("horario-google", ANCHO_VENTANA, ALTO_VENTANA, |p, _| pintar_todo(&mut e, p, marco, 1.0, &textos));
            assert_eq!(e.google.len(), 2, "los dos eventos de la semana, pintados");
        }
        // La ficha de una clase y un arrastre a medias, para mirarlos.
        e.editar(Edicion::Bloque(0));
        crate::ventanita::muestra("horario-ficha", ANCHO_VENTANA, ALTO_VENTANA, |p, _| {
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.edicion = None;
        e.foco = None;
        let r = e.rejilla.unwrap();
        let c = r.caja(5, 11 * 60, 11 * 60 + 15);
        e.botones.raton = (c.x + 20.0, c.y + 2.0);
        e.agarre = Some(Agarre::Asignatura {
            i: 2,
            desde: (0.0, 0.0),
            arrastrando: true,
        });
        crate::ventanita::muestra("horario-arrastre", ANCHO_VENTANA, ALTO_VENTANA, |p, _| {
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
    }
}
