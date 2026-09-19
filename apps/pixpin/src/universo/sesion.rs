//! La sesion del universo dentro del editor (D215).
//!
//! El editor de siempre pone la ventana, la camara, las anotaciones y sus
//! herramientas; la sesion se pone delante y atiende lo que es del universo
//! (astros, lineas, nebulosas, Ctrl+clic). Lo que no atiende lo devuelve con
//! `Respuesta::Pasa` y el editor hace lo suyo: asi dibujar encima de las
//! galaxias es exactamente dibujar en un lienzo.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use pixpin_geom::{Punto, Rect};
use pixpin_motor2d::{Camara, ColorRgba, Elemento, Escena, EstiloTrazo, Figura, Punto2};
use pixpin_render::{MotorRender, Pintor, RectF};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Catalogo;
use pixpin_ui::universo::{
    AccionInspector, BarraUniverso, COLORES_PLANETA, FichaInspector, Fila, HERRAMIENTAS_UNIVERSO,
    Inspector, Paneles, SelectorEmojis, minimapa_a_mundo, mundo_a_minimapa, tramos_de_ruta,
};
use pixpin_universo::buscar::{Hallazgo, IndiceBusqueda, TOPE_RESULTADOS};
use pixpin_universo::ficha::FichaLuna;
use pixpin_universo::{
    Arrastre, Clase, Encuadre, HerramientaUniverso, IdAstro, Nivel, RADIO_PLANETA_L,
    RADIO_PLANETA_M, RADIO_PLANETA_S, RejillaAstros, TipoConexion, Universo, Visto,
    ZOOM_MINIMO_UNIVERSO, nebulosa,
};

use super::Pedido;
use super::abrir::{self, Apertura, Motivo};
use super::cargador::Cargador;
use super::estrellas::Estrellas;
use super::pintar::{self, Contexto, PALETA};
use crate::miniaturas::Miniaturas;

/// Lo que dura el vuelo de la camara al enfocar (D225).
const VUELO: Duration = Duration::from_millis(250);
/// El destello rojo de «aqui no se puede».
const DESTELLO: Duration = Duration::from_millis(300);
/// El pulso azul que senala un archivo pedido desde el chat.
const PULSO: Duration = Duration::from_millis(600);
/// Se guarda 2 s despues del ultimo cambio: un arrastre son cien cambios y
/// una sola escritura.
const GUARDAR_TRAS: Duration = Duration::from_secs(2);
/// Dos clics mas juntos que esto, y mas cerca que `CERCA_DOBLE_CLIC`, son
/// un doble clic.
const DOBLE_CLIC: Duration = Duration::from_millis(400);
const CERCA_DOBLE_CLIC: i32 = 4;
/// Cada cuanto se mira si un cuaderno cargado cambio en el disco (D243).
const REVISAR_CUADERNOS: Duration = Duration::from_secs(2);
/// Lo que dura un aviso en pantalla.
const AVISO: Duration = Duration::from_secs(2);
/// Cuantas anotaciones dentro de un planeta se borran sin preguntar.
const BORRAR_SIN_PREGUNTAR: usize = 5;
/// El lado de un emoji recien puesto, en pixeles logicos de pantalla.
const LADO_EMOJI: f32 = 64.0;
/// Una galaxia por debajo de esto ya no ensena nada de dentro: su cuaderno
/// se puede soltar.
const RADIO_LEJANO: f32 = 6.0;

/// Lo que le dice la sesion al editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Respuesta {
    /// No es del universo: que lo haga el editor.
    Pasa,
    /// Atendido. `repintar` si cambio algo que se ve.
    Consumido {
        repintar: bool,
    },
    Cerrar,
    /// Hay que cerrar el universo y abrir esta hoja (D224). La sesion la
    /// deja tambien en `hoja_pedida`, que es lo que lee quien llama.
    AbrirHoja {
        proyecto: String,
        referencia: String,
    },
}

/// Lo que la sesion necesita saber del editor en cada evento.
#[derive(Debug, Clone, Copy)]
pub struct Editor {
    /// Con la mano elegida: un clic sobre un astro lo elige y lo arrastra.
    /// Con otra herramienta del editor, el clic es para dibujar encima.
    pub mano: bool,
    /// Escribiendo un texto: las teclas son del texto.
    pub escribiendo: bool,
    pub escala_por_cien: u32,
    /// La ventana del editor en el escritorio virtual.
    pub area: Rect,
}

struct Vuelo {
    desde: Camara,
    hasta: Camara,
    inicio: Instant,
}

pub struct Sesion {
    raiz: PathBuf,
    ruta: PathBuf,
    pub u: Universo,
    cargador: Cargador,
    rejilla: RejillaAstros,
    memoria: HashMap<IdAstro, Nivel>,
    vistos: Vec<Visto>,
    fichas: HashMap<String, FichaLuna>,
    cuentas: HashMap<String, usize>,
    cuentas_con: Option<u64>,
    nombres: HashMap<String, String>,
    pub seleccion: Vec<IdAstro>,
    pub herramienta: Option<HerramientaUniverso>,
    pub ultimo_tipo: TipoConexion,
    pub emoji_elegido: Option<String>,
    /// La herramienta Emoji sin emoji: el selector esta abierto (Tarea 15).
    pub selector_abierto: bool,
    arrastre: Option<(Arrastre, Punto2)>,
    conectando: Option<IdAstro>,
    /// Una ficha pulsada en la nebulosa: se coloca donde se suelte.
    colocando: Option<(String, String)>,
    paginas: HashMap<IdAstro, usize>,
    enfoque: Option<IdAstro>,
    vuelo: Option<Vuelo>,
    destellos: Vec<(IdAstro, Instant)>,
    pulsos: Vec<(IdAstro, Instant)>,
    guardar_en: Option<Instant>,
    visto: (u64, u64),
    estrellas: Option<Estrellas>,
    miniaturas: Miniaturas,
    indice_busqueda: Option<(u64, IndiceBusqueda)>,
    /// El texto del buscador, si esta abierto.
    pub busqueda: Option<String>,
    pedido: Option<Pedido>,
    ligero: bool,
    ultimo_clic: Option<(Instant, Punto)>,
    aviso: Option<(String, Instant, Duration)>,
    textos: Catalogo,
    /// La hoja que hay que abrir al cerrar el universo (D224).
    pub hoja_pedida: Option<(String, String)>,
    hwnd: isize,
    /// Pixeles fisicos de la ventana y escala del monitor: para encajar.
    tamano: (f32, f32),
    escala_por_cien: u32,
    revisado: Instant,
    pub minimapa: bool,
    selector: SelectorEmojis,
    /// Los resultados de la busqueda en curso, rehechos antes de pintar:
    /// pintar y pulsar tienen que ver la misma lista.
    encontrados: Vec<Hallazgo>,
}

/// Adonde van los ficheros soltados o pegados en el universo (D227).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destino {
    Proyecto(String),
    /// En el cosmos vacio o en un exoplaneta no hay un proyecto claro: se
    /// pregunta, porque el fichero tiene que entrar en algun chat.
    Preguntar,
}

/// Cuanto se separan en fila las lunas de varios ficheros soltados a la
/// vez, en unidades del mundo: mas que dos radios de luna, para que no se
/// tapen.
const SEPARACION_SOLTADOS: f32 = 120.0;

/// D227: el contenedor mas pequeno bajo el punto decide. Una galaxia, o un
/// planeta dentro de una, dan su proyecto; lo demas se pregunta.
pub fn destino_de_soltar(u: &Universo, x: f32, y: f32) -> Destino {
    let debajo = u
        .astros
        .iter()
        .filter(|a| a.es_contenedor() && a.contiene(x, y))
        .min_by(|a, b| a.radio.total_cmp(&b.radio));
    let galaxia = match debajo {
        Some(a) if matches!(a.clase, Clase::Galaxia { .. }) => Some(a),
        // Un planeta: el de una galaxia da la suya; un exoplaneta, ninguna.
        Some(a) => a.padre.and_then(|g| u.astro(g)),
        None => None,
    };
    match galaxia.and_then(|g| g.proyecto()) {
        Some(p) => Destino::Proyecto(p.to_string()),
        None => Destino::Preguntar,
    }
}

/// La clave de `Universo::resto` donde se guardan los emojis recientes.
const CLAVE_RECIENTES: &str = "emojis_recientes";

/// El elemento de un emoji suelto, con su esquina en `(x, y)` del mundo.
fn elemento_emoji(caracter: &str, x: f32, y: f32, lado: f32) -> Elemento {
    Elemento {
        id: 0,
        figura: Figura::Emoji {
            caracter: caracter.to_string(),
        },
        x,
        y,
        ancho: lado,
        alto: lado,
        angulo: 0.0,
        trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
        estilo_relleno: Default::default(),
        relleno: None,
        grosor: 1.0,
        estilo: EstiloTrazo::Solido,
        rugosidad: 0.0,
        opacidad: 1.0,
        semilla: 1,
        version: 0,
        borrado: false,
        grupos: Vec::new(),
        bloqueado: false,
        enlace: None,
        redondo: false,
    }
}

fn rectf(r: Rect) -> RectF {
    RectF {
        x: r.x as f32,
        y: r.y as f32,
        ancho: r.ancho as f32,
        alto: r.alto as f32,
    }
}

/// La caja que abarca unas cajas, o `None` si no hay ninguna.
fn union(cajas: impl Iterator<Item = (f32, f32, f32, f32)>) -> Option<(f32, f32, f32, f32)> {
    cajas.reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
}

/// La caja agrandada un `factor` alrededor de su centro.
fn agrandar(c: (f32, f32, f32, f32), factor: f32) -> (f32, f32, f32, f32) {
    let (cx, cy) = ((c.0 + c.2) / 2.0, (c.1 + c.3) / 2.0);
    let (mx, my) = ((c.2 - c.0) / 2.0 * factor, (c.3 - c.1) / 2.0 * factor);
    (cx - mx, cy - my, cx + mx, cy + my)
}

/// El punto medio de un vuelo: el centro va en linea recta y el zoom en
/// proporcion, que es lo que se siente como un viaje a velocidad constante.
pub fn interpolar(desde: &Camara, hasta: &Camara, t: f32, ancho: f32, alto: f32) -> Camara {
    let t = t.clamp(0.0, 1.0);
    // Suave al llegar: acelera poco y frena mucho.
    let s = 1.0 - (1.0 - t).powi(3);
    let centro = |c: &Camara| (c.x + ancho / (2.0 * c.zoom), c.y + alto / (2.0 * c.zoom));
    let (a, b) = (centro(desde), centro(hasta));
    let zoom = (desde.zoom.ln() + (hasta.zoom.ln() - desde.zoom.ln()) * s).exp();
    let (cx, cy) = (a.0 + (b.0 - a.0) * s, a.1 + (b.1 - a.1) * s);
    Camara {
        x: cx - ancho / (2.0 * zoom),
        y: cy - alto / (2.0 * zoom),
        zoom,
    }
}

impl Sesion {
    pub fn nueva(
        raiz: PathBuf,
        u: Universo,
        nombres: HashMap<String, String>,
        nivel: pixpin_nivel::Nivel,
        textos: Catalogo,
        pedido: Option<Pedido>,
    ) -> Sesion {
        let ligero = nivel == pixpin_nivel::Nivel::Ligero;
        let mut selector = SelectorEmojis::default();
        selector.recientes = u
            .resto
            .get(CLAVE_RECIENTES)
            .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok())
            .unwrap_or_default();
        Sesion {
            ruta: pixpin_universo::formato::ruta(&raiz),
            cargador: Cargador::nuevo(raiz.clone(), None),
            raiz,
            u,
            rejilla: RejillaAstros::default(),
            memoria: HashMap::new(),
            vistos: Vec::new(),
            fichas: HashMap::new(),
            cuentas: HashMap::new(),
            cuentas_con: None,
            nombres,
            seleccion: Vec::new(),
            herramienta: None,
            ultimo_tipo: TipoConexion::Relacion,
            emoji_elegido: None,
            selector_abierto: false,
            arrastre: None,
            conectando: None,
            colocando: None,
            paginas: HashMap::new(),
            enfoque: None,
            vuelo: None,
            destellos: Vec::new(),
            pulsos: Vec::new(),
            guardar_en: None,
            visto: (0, 0),
            estrellas: None,
            // D238: menos miniaturas en Ligero.
            miniaturas: Miniaturas::con_lado(if ligero { 120 } else { 164 }),
            indice_busqueda: None,
            busqueda: None,
            pedido,
            ligero,
            ultimo_clic: None,
            aviso: None,
            textos,
            hoja_pedida: None,
            hwnd: 0,
            tamano: (1920.0, 1080.0),
            escala_por_cien: 100,
            revisado: Instant::now(),
            minimapa: true,
            selector,
            encontrados: Vec::new(),
        }
    }

    fn escala(&self) -> f32 {
        crate::navegacion::vista_efectiva(
            &Camara {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
            self.escala_por_cien,
        )
        .zoom
    }

    /// El tamano de la ventana en pixeles logicos: lo que mide la camara
    /// del usuario.
    fn tamano_logico(&self) -> (f32, f32) {
        let e = self.escala();
        (self.tamano.0 / e, self.tamano.1 / e)
    }

    pub fn avisar(&mut self, texto: String, dura: Duration) {
        self.aviso = Some((texto, Instant::now(), dura));
    }

    pub fn avisar_roto(&mut self) {
        let t = self.textos.t("universo-roto");
        self.avisar(t, Duration::from_secs(6));
    }

    /// La ventana del editor ya existe: el cargador ya puede despertarla, y
    /// `lanzar` ya puede traerla al frente.
    pub fn conectar_ventana(&mut self, ventana: &VentanaOverlay, area: Rect, escala_por_cien: u32) {
        self.hwnd = ventana.handle().0 as isize;
        self.cargador.poner_aviso(self.hwnd);
        self.tamano = (area.ancho as f32, area.alto as f32);
        self.escala_por_cien = escala_por_cien;
        super::ABIERTO.store(self.hwnd, std::sync::atomic::Ordering::SeqCst);
        // Un pedido que llego mientras nacia la ventana.
        if let Some(p) = super::tomar_pedido() {
            self.pedido = Some(p);
        }
    }

    /// La camara con la que abre: la del pedido, o la del ultimo dia.
    pub fn camara_inicial(&mut self) -> Camara {
        let guardada = Camara {
            x: self.u.encuadre.x,
            y: self.u.encuadre.y,
            zoom: self.u.encuadre.zoom.max(ZOOM_MINIMO_UNIVERSO),
        };
        let mut c = guardada;
        let ligero = std::mem::replace(&mut self.ligero, true);
        self.atender_pedido(&mut c);
        self.ligero = ligero;
        c
    }

    fn encajar(&self, caja: (f32, f32, f32, f32)) -> Camara {
        let (w, h) = self.tamano_logico();
        Camara::encajar_entre(
            caja,
            w,
            h,
            24.0,
            ZOOM_MINIMO_UNIVERSO,
            pixpin_motor2d::camara::ZOOM_MAXIMO,
        )
    }

    fn volar_a(&mut self, destino: Camara, camara: &mut Camara) {
        if self.ligero {
            *camara = destino;
            self.vuelo = None;
        } else {
            self.vuelo = Some(Vuelo {
                desde: *camara,
                hasta: destino,
                inicio: Instant::now(),
            });
        }
    }

    /// D225: la camara encaja el astro y lo de fuera se atenua.
    fn enfocar(&mut self, id: IdAstro, camara: &mut Camara) {
        let Some(a) = self.u.astro(id) else { return };
        let caja = agrandar(a.caja(), 1.1);
        self.enfoque = Some(id);
        let destino = self.encajar(caja);
        self.volar_a(destino, camara);
    }

    fn encajar_cosmos(&mut self, camara: &mut Camara) {
        let caja = union(
            self.u
                .astros
                .iter()
                .filter(|a| matches!(a.clase, Clase::Galaxia { .. }))
                .map(|a| a.caja()),
        );
        self.enfoque = None;
        if let Some(c) = caja {
            let destino = self.encajar(agrandar(c, 1.05));
            self.volar_a(destino, camara);
        }
    }

    fn galaxia_de(&self, proyecto: &str) -> Option<IdAstro> {
        self.u
            .astros
            .iter()
            .find(|a| matches!(&a.clase, Clase::Galaxia { proyecto: p } if p == proyecto))
            .map(|a| a.id)
    }

    /// Las fichas sin colocar de un proyecto, de la mas nueva a la mas
    /// vieja. Vacio si su cuaderno no esta cargado.
    fn sueltas(&self, proyecto: &str) -> Vec<&FichaLuna> {
        let Some(todas) = self.cargador.cargado(proyecto) else {
            return Vec::new();
        };
        let colocadas: HashSet<&str> = self.u.astros.iter().filter_map(|a| a.codigo()).collect();
        nebulosa::sin_colocar(todas, &colocadas)
    }

    /// Atiende el pedido pendiente (D211, D212). Se queda pendiente si hace
    /// falta un cuaderno que aun no ha llegado.
    fn atender_pedido(&mut self, camara: &mut Camara) -> bool {
        let Some(p) = self.pedido.take() else {
            return false;
        };
        match &p {
            Pedido::Cosmos => self.encajar_cosmos(camara),
            Pedido::Galaxia(proyecto) => {
                if let Some(g) = self.galaxia_de(proyecto) {
                    self.enfocar(g, camara);
                }
            }
            Pedido::Luna { proyecto, codigo } => {
                if let Some(l) = self.u.luna_de(codigo).map(|a| a.id) {
                    // La luna, con aire alrededor para ver donde esta.
                    if let Some(a) = self.u.astro(l) {
                        let destino = self.encajar(agrandar(a.caja(), 6.0));
                        self.volar_a(destino, camara);
                    }
                    self.seleccion = vec![l];
                    self.pulsos.push((l, Instant::now()));
                } else if let Some(g) = self.galaxia_de(proyecto) {
                    if self.cargador.cargado(proyecto).is_none() {
                        // Sin su cuaderno no se sabe en que pagina esta.
                        let notas = self.u.astro(g).is_some_and(|a| a.notas_del_chat);
                        self.cargador.pedir(proyecto, notas);
                        self.enfocar(g, camara);
                        self.pedido = Some(p);
                        return true;
                    }
                    let i = self
                        .sueltas(proyecto)
                        .iter()
                        .position(|f| f.codigo == *codigo);
                    if let Some(i) = i {
                        self.paginas.insert(g, i / nebulosa::POR_PAGINA);
                    }
                    self.enfocar(g, camara);
                }
            }
        }
        true
    }

    fn rehacer_fichas(&mut self) {
        self.fichas = self
            .cargador
            .todas()
            .map(|f| (f.codigo.clone(), f.clone()))
            .collect();
        self.cuentas_con = None;
        self.indice_busqueda = None;
    }

    /// Cuantas lunas tiene cada proyecto: las del cuaderno si esta cargado,
    /// y si no, las que ya estan en el cielo.
    fn rehacer_cuentas(&mut self) {
        if self.cuentas_con == Some(self.u.cambios()) {
            return;
        }
        let mut c: HashMap<String, usize> = HashMap::new();
        for a in &self.u.astros {
            if let Clase::Luna { proyecto, .. } = &a.clase
                && self.cargador.cargado(proyecto).is_none()
            {
                *c.entry(proyecto.clone()).or_default() += 1;
            }
        }
        for f in self.fichas.values() {
            *c.entry(f.proyecto.clone()).or_default() += 1;
        }
        self.cuentas = c;
        self.cuentas_con = Some(self.u.cambios());
    }

    fn a_mundo(&self, p: Punto, camara: &Camara, ed: &Editor) -> Punto2 {
        let efectiva = crate::navegacion::vista_efectiva(camara, ed.escala_por_cien);
        efectiva.a_mundo(Punto2::nuevo(
            (p.x - ed.area.x) as f32,
            (p.y - ed.area.y) as f32,
        ))
    }

    /// La ficha de la nebulosa bajo un punto del mundo.
    fn celda_en(&self, q: Punto2) -> Option<(IdAstro, FichaLuna)> {
        for v in self.vistos.iter().filter(|v| v.nivel == Nivel::Vista) {
            let Some(g) = self.u.astro(v.id) else {
                continue;
            };
            let Clase::Galaxia { proyecto } = &g.clase else {
                continue;
            };
            let Some(i) = nebulosa::luna_en(g, q.x, q.y) else {
                continue;
            };
            let sueltas = self.sueltas(proyecto);
            let tramo =
                nebulosa::pagina(sueltas.len(), self.paginas.get(&g.id).copied().unwrap_or(0));
            if let Some(f) = sueltas
                .get(tramo.start + i)
                .filter(|_| tramo.start + i < tramo.end)
            {
                return Some((g.id, (*f).clone()));
            }
        }
        None
    }

    /// La galaxia cuyo chip «+N mas» esta bajo el punto.
    fn chip_en(&self, q: Punto2) -> Option<IdAstro> {
        self.vistos
            .iter()
            .filter(|v| v.nivel == Nivel::Vista)
            .filter_map(|v| self.u.astro(v.id))
            .find(|g| {
                let (x0, y0, x1, y1) = nebulosa::chip_mas(g);
                q.x >= x0 && q.x < x1 && q.y >= y0 && q.y < y1
            })
            .map(|g| g.id)
    }

    fn astro_en(&self, q: Punto2) -> Option<IdAstro> {
        pixpin_universo::astro_en(&self.u, &self.vistos, q.x, q.y)
    }

    fn destellar(&mut self, id: Option<IdAstro>) {
        if let Some(id) = id {
            self.destellos.push((id, Instant::now()));
        }
    }

    /// Guarda ya. Lo llama el guardado diferido y el cierre.
    pub fn guardar(&mut self, escena: &Escena) -> Result<(), pixpin_universo::ErrorUniverso> {
        self.guardar_en = None;
        let hecho = pixpin_universo::formato::guardar(&self.ruta, &self.u, escena);
        match &hecho {
            Ok(()) => tracing::debug!(ruta = %self.ruta.display(), "universo guardado"),
            Err(e) => tracing::warn!(?e, "no se pudo guardar el universo"),
        }
        hecho
    }

    /// Ctrl+clic (D224).
    fn abrir_con_ctrl(&mut self, q: Punto2, todas: bool) -> Respuesta {
        let apertura = if let Some(id) = self.astro_en(q) {
            if todas {
                let con_fichero = self
                    .u
                    .hijos(id)
                    .filter_map(|l| l.codigo().and_then(|c| self.fichas.get(c)))
                    .filter(|f| f.en_equipo && f.ruta.is_some())
                    .count();
                if con_fichero > abrir::TOPE_ABRIR_VARIOS {
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("n", abrir::TOPE_ABRIR_VARIOS);
                    args.set("total", con_fichero);
                    let seguir = pixpin_shell::confirmar_destructivo(
                        windows::Win32::Foundation::HWND(self.hwnd as *mut _),
                        &self.textos.t("universo-titulo"),
                        &self.textos.t_args("universo-abrir-varios", &args),
                    );
                    if !seguir {
                        return Respuesta::Consumido { repintar: false };
                    }
                }
            }
            (
                Some(id),
                abrir::que_abre(&self.u, id, &self.fichas, &self.raiz, todas),
            )
        } else if let Some((g, f)) = self.celda_en(q) {
            (Some(g), abrir::que_abre_ficha(&f, &self.raiz))
        } else {
            return Respuesta::Consumido { repintar: false };
        };
        self.resolver_apertura(apertura)
    }

    /// Hace lo que dice la tabla D224 con lo que se pulso: `origen` es el
    /// astro que destella si no se puede.
    fn resolver_apertura(&mut self, apertura: (Option<IdAstro>, Apertura)) -> Respuesta {
        match apertura {
            (
                _,
                Apertura::Hoja {
                    proyecto,
                    referencia,
                },
            ) => {
                self.hoja_pedida = Some((proyecto.clone(), referencia.clone()));
                Respuesta::AbrirHoja {
                    proyecto,
                    referencia,
                }
            }
            (id, Apertura::Nada(m)) => {
                self.destellar(id);
                if m == Motivo::NoEstaEnEquipo {
                    let t = self.textos.t("universo-no-esta");
                    self.avisar(t, AVISO);
                }
                Respuesta::Consumido { repintar: true }
            }
            (_, a) => {
                abrir::ejecutar(&a);
                Respuesta::Consumido { repintar: false }
            }
        }
    }

    fn ir_a_hallazgo(&mut self, h: Hallazgo, camara: &mut Camara) {
        match h {
            Hallazgo::Astro(id) => {
                self.seleccion = vec![id];
                if let Some(a) = self.u.astro(id) {
                    let factor = if a.es_contenedor() { 1.1 } else { 6.0 };
                    let destino = self.encajar(agrandar(a.caja(), factor));
                    self.volar_a(destino, camara);
                }
                self.pulsos.push((id, Instant::now()));
            }
            Hallazgo::Suelta { proyecto, codigo } => {
                self.pedido = Some(Pedido::Luna { proyecto, codigo });
                self.atender_pedido(camara);
            }
        }
    }

    /// Los resultados de la busqueda en curso (D226).
    pub fn hallazgos(&mut self) -> Vec<Hallazgo> {
        let Some(q) = self.busqueda.clone() else {
            return Vec::new();
        };
        if self.indice_busqueda.as_ref().map(|(c, _)| *c) != Some(self.u.cambios()) {
            let fichas = &self.fichas;
            let nombres = &self.nombres;
            let colocadas: HashSet<&str> =
                self.u.astros.iter().filter_map(|a| a.codigo()).collect();
            let sueltas: Vec<(String, String, String)> = fichas
                .values()
                .filter(|f| !colocadas.contains(f.codigo.as_str()))
                .map(|f| (f.proyecto.clone(), f.codigo.clone(), f.busqueda.clone()))
                .collect();
            let indice = IndiceBusqueda::construir(
                &self.u,
                |a| match &a.clase {
                    Clase::Galaxia { proyecto } => {
                        nombres.get(proyecto).cloned().unwrap_or_default()
                    }
                    Clase::Luna { codigo, .. } => fichas
                        .get(codigo)
                        .map(|f| f.busqueda.clone())
                        .unwrap_or_default(),
                    Clase::Planeta => String::new(),
                },
                sueltas,
            );
            self.indice_busqueda = Some((self.u.cambios(), indice));
        }
        self.indice_busqueda
            .as_ref()
            .map(|(_, i)| i.buscar(&q, TOPE_RESULTADOS))
            .unwrap_or_default()
    }

    /// Un evento del editor. Ver el orden en el plan (Tarea 14, Step 4).
    pub fn evento(
        &mut self,
        ev: &EventoOverlay,
        camara: &mut Camara,
        escena: &mut Escena,
        ed: &Editor,
    ) -> Respuesta {
        self.escala_por_cien = ed.escala_por_cien;
        self.tamano = (ed.area.ancho as f32, ed.area.alto as f32);
        match *ev {
            EventoOverlay::Despierta => Respuesta::Consumido {
                repintar: self.al_despertar(camara),
            },
            EventoOverlay::Tecla { .. } | EventoOverlay::Caracter(_) if ed.escribiendo => {
                Respuesta::Pasa
            }
            EventoOverlay::Tecla {
                vk, ctrl, shift, ..
            } => self.tecla(vk, ctrl, shift, camara, escena),
            EventoOverlay::Caracter(c) => self.caracter(c, camara),
            EventoOverlay::BotonPulsado(p) => {
                // 1. Los paneles van antes que el cielo: son dialogos
                // encima de el.
                let local = Punto {
                    x: p.x - ed.area.x,
                    y: p.y - ed.area.y,
                };
                if let Some(r) = self.pulsar_panel(local, camara, escena) {
                    return r;
                }
                self.pulsar(p, camara, escena, ed)
            }
            EventoOverlay::RatonMovido(p) => {
                if let Some((mut a, inicio)) = self.arrastre.take() {
                    let q = self.a_mundo(p, camara, ed);
                    self.u
                        .arrastrar(&mut a, q.x - inicio.x, q.y - inicio.y, escena);
                    self.arrastre = Some((a, inicio));
                    return Respuesta::Consumido { repintar: true };
                }
                if self.conectando.is_some() || self.colocando.is_some() {
                    return Respuesta::Consumido { repintar: false };
                }
                Respuesta::Pasa
            }
            EventoOverlay::BotonSoltado(p) => self.soltar(p, camara, escena, ed),
            EventoOverlay::FicherosSoltados => {
                // Las rutas se recogen siempre: si se quedaran, saldrian en
                // la siguiente vez que se suelte algo.
                let rutas = pixpin_shell::overlay::ficheros_soltados();
                // Donde esta el raton AHORA: mientras se arrastra desde el
                // Explorador la ventana no recibe movimientos, asi que el
                // ultimo que vio no sirve.
                let q = self.a_mundo(pixpin_shell::entorno::posicion_del_cursor(), camara, ed);
                let ficheros = crate::ventana_chat::leer_ficheros(&rutas);
                Respuesta::Consumido {
                    repintar: self.meter_ficheros(&ficheros, q),
                }
            }
            _ => Respuesta::Pasa,
        }
    }

    /// Mete ficheros en el chat del proyecto que toque y pone sus lunas
    /// donde cayeron (D227). `true` si entro alguno.
    fn meter_ficheros(&mut self, ficheros: &[(String, Vec<u8>)], q: Punto2) -> bool {
        if ficheros.is_empty() {
            return false;
        }
        let proyecto = match destino_de_soltar(&self.u, q.x, q.y) {
            Destino::Proyecto(p) => p,
            Destino::Preguntar => match self.preguntar_proyecto() {
                Some(p) => p,
                None => return false,
            },
        };
        let aparato = pixpin_proyecto::identidad::Identidad::leer_o_crear(&self.raiz, "PC")
            .map(|i| i.yo.codigo())
            .unwrap_or_default();
        let hechos =
            match crate::ventana_chat::meter_en_proyecto(&self.raiz, &proyecto, ficheros, &aparato)
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(?e, %proyecto, "no se pudieron meter los ficheros");
                    return false;
                }
            };
        for (i, m) in hechos.iter().enumerate() {
            let x = q.x + i as f32 * SEPARACION_SOLTADOS;
            // Si no se puede colocar ahi (un exoplaneta dentro de otra
            // galaxia, el cosmos vacio), la luna se queda en la nebulosa:
            // el fichero ya esta en el chat y no se pierde.
            if let Err(r) = self.u.colocar_luna(&m.codigo_unico(), &proyecto, x, q.y) {
                tracing::info!(?r, "la luna nueva se queda en la nebulosa");
            }
        }
        // Sus fichas, para que las lunas nuevas tengan nombre e icono.
        let notas = self
            .galaxia_de(&proyecto)
            .and_then(|g| self.u.astro(g))
            .is_some_and(|a| a.notas_del_chat);
        self.cargador.pedir(&proyecto, notas);
        !hechos.is_empty()
    }

    /// El menu con los proyectos, por nombre, para lo que cae fuera de
    /// toda galaxia. `None` si se cierra sin elegir.
    fn preguntar_proyecto(&self) -> Option<String> {
        let mut lista: Vec<(&String, &String)> = self.nombres.iter().collect();
        lista.sort_by_key(|(_, nombre)| nombre.to_lowercase());
        let entradas: Vec<(u32, String)> = lista
            .iter()
            .enumerate()
            .map(|(i, (_, nombre))| (i as u32 + 1, (*nombre).clone()))
            .collect();
        let elegido = pixpin_shell::menu_llano(
            windows::Win32::Foundation::HWND(self.hwnd as *mut _),
            &entradas,
        )?;
        lista
            .get(elegido.checked_sub(1)? as usize)
            .map(|(id, _)| (*id).clone())
    }

    /// Ctrl+V (D244): ficheros o una imagen del portapapeles entran al chat
    /// como si se soltaran en el centro de la vista. `None` si lo que hay
    /// es otra cosa (texto): eso es del editor.
    fn pegar(&mut self, camara: &Camara) -> Option<bool> {
        use pixpin_codec::ContenidoPortapapeles as Que;
        let ficheros = match pixpin_codec::portapapeles::leer()? {
            Que::Rutas(rutas) => crate::ventana_chat::leer_ficheros(&rutas),
            // Una imagen suelta en la escena solo viviria en memoria: se
            // mete en el proyecto y nace como luna.
            Que::Imagen(imagen) => match pixpin_codec::codificar_png(&imagen) {
                Ok(bytes) => {
                    let cuando = pixpin_shell::entorno::ahora_local_ms();
                    vec![(format!("pegada-{cuando}.png"), bytes)]
                }
                Err(e) => {
                    tracing::warn!(?e, "no se pudo guardar la imagen pegada");
                    return Some(false);
                }
            },
            Que::Texto(_) => return None,
        };
        let (w, h) = self.tamano_logico();
        let zoom = camara.zoom.max(f32::EPSILON);
        let centro = Punto2::nuevo(camara.x + w / (2.0 * zoom), camara.y + h / (2.0 * zoom));
        Some(self.meter_ficheros(&ficheros, centro))
    }

    fn tecla(
        &mut self,
        vk: u32,
        ctrl: bool,
        shift: bool,
        camara: &mut Camara,
        escena: &mut Escena,
    ) -> Respuesta {
        const VK_RETROCESO: u32 = 0x08;
        const VK_ENTRAR: u32 = 0x0D;
        const VK_ESCAPE: u32 = 0x1B;
        const VK_INICIO: u32 = 0x24;
        const VK_SUPR: u32 = 0x2E;
        let hecho = Respuesta::Consumido { repintar: true };
        if let Some(b) = &mut self.busqueda {
            match vk {
                VK_RETROCESO => {
                    b.pop();
                    return hecho;
                }
                VK_ENTRAR => {
                    if let Some(h) = self.hallazgos().into_iter().next() {
                        self.ir_a_hallazgo(h, camara);
                    }
                    self.busqueda = None;
                    return hecho;
                }
                VK_ESCAPE => {
                    self.busqueda = None;
                    return hecho;
                }
                _ if !ctrl => return Respuesta::Consumido { repintar: false },
                _ => {}
            }
        }
        if self.selector_abierto && vk == VK_RETROCESO {
            self.selector.filtro.pop();
            return hecho;
        }
        match vk {
            v if ctrl && (v == b'Z' as u32 || v == b'Y' as u32) => {
                let rehacer = v == b'Y' as u32 || shift;
                let hubo = if rehacer {
                    self.u.rehacer(escena)
                } else {
                    self.u.deshacer(escena)
                };
                let u = &self.u;
                self.seleccion.retain(|id| u.astro(*id).is_some());
                Respuesta::Consumido { repintar: hubo }
            }
            v if ctrl && v == b'F' as u32 => {
                self.busqueda = Some(String::new());
                hecho
            }
            v if ctrl && v == b'V' as u32 => match self.pegar(camara) {
                Some(repintar) => Respuesta::Consumido { repintar },
                None => Respuesta::Pasa,
            },
            VK_INICIO => {
                self.encajar_cosmos(camara);
                hecho
            }
            VK_SUPR if !self.seleccion.is_empty() => {
                let anotaciones: usize = self
                    .seleccion
                    .iter()
                    .map(|id| self.u.anotaciones_dentro(*id, escena).len())
                    .sum();
                if anotaciones > BORRAR_SIN_PREGUNTAR {
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("n", anotaciones);
                    let seguir = pixpin_shell::confirmar_destructivo(
                        windows::Win32::Foundation::HWND(self.hwnd as *mut _),
                        &self.textos.t("universo-titulo"),
                        &self.textos.t_args("universo-borrar-anotaciones", &args),
                    );
                    if !seguir {
                        return Respuesta::Consumido { repintar: false };
                    }
                }
                let ids = std::mem::take(&mut self.seleccion);
                self.u.borrar(&ids, escena);
                hecho
            }
            VK_ESCAPE => {
                if self.selector_abierto {
                    self.selector_abierto = false;
                } else if self.enfoque.is_some() {
                    self.enfoque = None;
                } else if !self.seleccion.is_empty() {
                    self.seleccion.clear();
                } else if self.herramienta.is_some() {
                    self.herramienta = None;
                } else {
                    return self.pedir_cierre(escena);
                }
                hecho
            }
            _ => Respuesta::Pasa,
        }
    }

    /// Cerrar guarda antes; si no se puede guardar, pregunta (D240).
    fn pedir_cierre(&mut self, escena: &Escena) -> Respuesta {
        if self.guardar(escena).is_err() {
            let cerrar = pixpin_shell::confirmar_destructivo(
                windows::Win32::Foundation::HWND(self.hwnd as *mut _),
                &self.textos.t("universo-titulo"),
                &self.textos.t("universo-no-guardado"),
            );
            if !cerrar {
                return Respuesta::Consumido { repintar: false };
            }
        }
        Respuesta::Cerrar
    }

    fn caracter(&mut self, c: char, camara: &mut Camara) -> Respuesta {
        if c < ' ' {
            return Respuesta::Pasa;
        }
        if let Some(b) = &mut self.busqueda {
            b.push(c);
            return Respuesta::Consumido { repintar: true };
        }
        // Con el selector abierto, lo que se teclea filtra los emojis.
        if self.selector_abierto {
            self.selector.filtro.push(c);
            return Respuesta::Consumido { repintar: true };
        }
        let hecho = Respuesta::Consumido { repintar: true };
        match c.to_ascii_lowercase() {
            'p' => {
                self.herramienta = Some(HerramientaUniverso::Planeta);
                hecho
            }
            'e' => {
                self.herramienta = Some(HerramientaUniverso::Emoji);
                self.selector_abierto = self.emoji_elegido.is_none();
                hecho
            }
            'c' => {
                self.herramienta = Some(HerramientaUniverso::Conectar);
                hecho
            }
            // La V deja la del universo y la mano la pone el editor.
            'v' => {
                self.herramienta = None;
                Respuesta::Pasa
            }
            'm' => {
                self.minimapa = !self.minimapa;
                hecho
            }
            // Con astros elegidos, la F los encaja; sin ellos es del editor.
            'f' if self.encajar_seleccion(camara) => hecho,
            _ => Respuesta::Pasa,
        }
    }

    /// `F`: encaja la seleccion. Va aparte de `caracter` porque mueve la
    /// camara.
    fn encajar_seleccion(&mut self, camara: &mut Camara) -> bool {
        let caja = union(
            self.seleccion
                .iter()
                .filter_map(|id| self.u.astro(*id))
                .map(|a| a.caja()),
        );
        match caja {
            Some(c) => {
                let destino = self.encajar(agrandar(c, 1.2));
                self.volar_a(destino, camara);
                true
            }
            None => false,
        }
    }

    fn pulsar(
        &mut self,
        p: Punto,
        camara: &mut Camara,
        escena: &mut Escena,
        ed: &Editor,
    ) -> Respuesta {
        if ed.escribiendo {
            return Respuesta::Pasa;
        }
        let q = self.a_mundo(p, camara, ed);
        let mods = pixpin_shell::entrada::modificadores_pulsados();
        // Ctrl+clic abre, con cualquier herramienta: igual que un enlace en
        // el editor, es un boton y no un trazo.
        if mods.ctrl {
            return self.abrir_con_ctrl(q, mods.shift);
        }
        // El doble clic.
        let ahora = Instant::now();
        let doble = self.ultimo_clic.is_some_and(|(t, a)| {
            ahora.duration_since(t) <= DOBLE_CLIC
                && (a.x - p.x).abs() <= CERCA_DOBLE_CLIC
                && (a.y - p.y).abs() <= CERCA_DOBLE_CLIC
        });
        if doble {
            self.ultimo_clic = None;
            match self.astro_en(q) {
                Some(id) if self.u.astro(id).is_some_and(|a| a.es_contenedor()) => {
                    self.enfocar(id, camara)
                }
                Some(id) => self.seleccion = vec![id],
                None => self.enfoque = None,
            }
            return Respuesta::Consumido { repintar: true };
        }
        self.ultimo_clic = Some((ahora, p));

        match self.herramienta {
            Some(HerramientaUniverso::Planeta) => {
                match self.u.crear_planeta(q.x, q.y, RADIO_PLANETA_M) {
                    Ok(id) => self.seleccion = vec![id],
                    Err(e) => {
                        tracing::debug!(?e, "planeta rechazado");
                        let id = self.astro_en(q);
                        self.destellar(id);
                    }
                }
                return Respuesta::Consumido { repintar: true };
            }
            Some(HerramientaUniverso::Emoji) => {
                match self.emoji_elegido.clone() {
                    None => self.selector_abierto = true,
                    Some(e) => {
                        let efectiva =
                            crate::navegacion::vista_efectiva(camara, ed.escala_por_cien);
                        let lado = LADO_EMOJI * self.escala() / efectiva.zoom;
                        escena.abrir_paso();
                        escena.anadir(elemento_emoji(&e, q.x - lado / 2.0, q.y - lado / 2.0, lado));
                        escena.cerrar_paso();
                    }
                }
                return Respuesta::Consumido { repintar: true };
            }
            Some(HerramientaUniverso::Conectar) => {
                self.conectando = self.astro_en(q);
                return Respuesta::Consumido { repintar: false };
            }
            None => {}
        }
        if !ed.mano {
            return Respuesta::Pasa;
        }
        if let Some(g) = self.chip_en(q) {
            let proyecto = self
                .u
                .astro(g)
                .and_then(|a| a.proyecto().map(str::to_string));
            let total = proyecto.map(|p| self.sueltas(&p).len()).unwrap_or(0);
            let actual = self.paginas.get(&g).copied().unwrap_or(0);
            let siguiente = (actual + 1) % nebulosa::paginas(total);
            self.paginas.insert(g, siguiente);
            return Respuesta::Consumido { repintar: true };
        }
        if let Some((_, f)) = self.celda_en(q) {
            self.colocando = Some((f.codigo, f.proyecto));
            return Respuesta::Consumido { repintar: false };
        }
        match self.astro_en(q) {
            Some(id) => {
                if mods.shift {
                    if let Some(i) = self.seleccion.iter().position(|s| *s == id) {
                        self.seleccion.remove(i);
                        return Respuesta::Consumido { repintar: true };
                    }
                    self.seleccion.push(id);
                } else if !self.seleccion.contains(&id) {
                    self.seleccion = vec![id];
                }
                let a = self.u.empezar_arrastre(&self.seleccion, escena);
                self.arrastre = Some((a, q));
                Respuesta::Consumido { repintar: true }
            }
            None => {
                // En el vacio: el editor elige anotaciones con su marquesina.
                if !self.seleccion.is_empty() {
                    self.seleccion.clear();
                }
                Respuesta::Pasa
            }
        }
    }

    fn soltar(
        &mut self,
        p: Punto,
        camara: &mut Camara,
        escena: &mut Escena,
        ed: &Editor,
    ) -> Respuesta {
        let q = self.a_mundo(p, camara, ed);
        if let Some((a, _)) = self.arrastre.take() {
            let raices = self.seleccion.clone();
            match self.u.soltar(a, escena) {
                Ok(true) => raices
                    .iter()
                    .for_each(|id| self.pulsos.push((*id, Instant::now()))),
                Ok(false) => {}
                Err(e) => {
                    tracing::debug!(?e, "arrastre rechazado");
                    for id in raices {
                        self.destellar(Some(id));
                    }
                }
            }
            return Respuesta::Consumido { repintar: true };
        }
        if let Some(desde) = self.conectando.take() {
            if let Some(hasta) = self.astro_en(q) {
                self.u.conectar(desde, hasta, self.ultimo_tipo);
            }
            return Respuesta::Consumido { repintar: true };
        }
        if let Some((codigo, proyecto)) = self.colocando.take() {
            match self.u.colocar_luna(&codigo, &proyecto, q.x, q.y) {
                Ok(id) => self.seleccion = vec![id],
                Err(e) => {
                    tracing::debug!(?e, "luna rechazada");
                    let g = self.galaxia_de(&proyecto);
                    self.destellar(g);
                }
            }
            return Respuesta::Consumido { repintar: true };
        }
        Respuesta::Pasa
    }

    /// Una vez por vuelta del bucle del editor: el vuelo, los destellos y
    /// el encuadre que se guarda. Devuelve si hay que repintar.
    pub fn tick(&mut self, camara: &mut Camara) -> bool {
        let ahora = Instant::now();
        let mut repintar = false;
        if let Some(v) = &self.vuelo {
            let t = ahora.duration_since(v.inicio).as_secs_f32() / VUELO.as_secs_f32();
            let (w, h) = self.tamano_logico();
            *camara = interpolar(&v.desde, &v.hasta, t, w, h);
            if t >= 1.0 {
                *camara = v.hasta;
                self.vuelo = None;
            }
            repintar = true;
        }
        let antes = self.destellos.len() + self.pulsos.len();
        self.destellos
            .retain(|(_, t)| ahora.duration_since(*t) < DESTELLO);
        self.pulsos
            .retain(|(_, t)| ahora.duration_since(*t) < PULSO);
        repintar |= antes > 0;
        if let Some((_, desde, dura)) = &self.aviso {
            if ahora.duration_since(*desde) >= *dura {
                self.aviso = None;
                repintar = true;
            }
        }
        self.u.encuadre = Encuadre {
            x: camara.x,
            y: camara.y,
            zoom: camara.zoom,
        };
        repintar
    }

    /// Tras cada evento y cada vuelta: apuntar los trazos del editor en la
    /// pila del universo (D234) y guardar a su hora.
    pub fn tras_evento(&mut self, escena: &Escena) {
        self.u.sincronizar_trazos(escena);
        let ahora = (self.u.cambios(), escena.pasos_cerrados());
        if ahora != self.visto {
            self.visto = ahora;
            self.guardar_en = Some(Instant::now() + GUARDAR_TRAS);
        }
        if self.guardar_en.is_some_and(|t| Instant::now() >= t) {
            // Un error queda en el registro; se reintenta al siguiente cambio.
            let _ = self.guardar(escena);
        }
    }

    /// Cuanto puede dormir el editor. `None` si no hay nada pendiente: con
    /// el universo quieto, la CPU se queda en cero (D235).
    pub fn tope_ms(&self) -> Option<u32> {
        let mut tope: Option<u32> = None;
        let mut min = |ms: u32| tope = Some(tope.map_or(ms, |t| t.min(ms)));
        if self.vuelo.is_some() || !self.destellos.is_empty() || !self.pulsos.is_empty() {
            min(16);
        }
        if let Some(t) = self.guardar_en {
            min(t.saturating_duration_since(Instant::now()).as_millis() as u32 + 1);
        }
        if let Some((_, desde, dura)) = &self.aviso {
            let fin = *desde + *dura;
            min(fin.saturating_duration_since(Instant::now()).as_millis() as u32 + 1);
        }
        tope
    }

    /// `EventoOverlay::Despierta`: llego un cuaderno o un pedido.
    pub fn al_despertar(&mut self, camara: &mut Camara) -> bool {
        let llegados = self.cargador.recibir();
        let mut repintar = !llegados.is_empty();
        for c in &llegados {
            if let Some(e) = &c.error {
                tracing::warn!(proyecto = %c.proyecto, %e, "cuaderno ilegible");
            }
            if c.rotas > 0 {
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("n", c.rotas);
                let t = self.textos.t_args("universo-rotas", &args);
                self.avisar(t, AVISO);
            }
        }
        if !llegados.is_empty() {
            self.rehacer_fichas();
        }
        if let Some(p) = super::tomar_pedido() {
            self.pedido = Some(p);
        }
        repintar |= self.atender_pedido(camara);
        repintar
    }

    /// D243: vuelve a leer los cuadernos cargados que cambiaron.
    fn revisar_cuadernos(&mut self) {
        if self.revisado.elapsed() < REVISAR_CUADERNOS || self.cargador.leyendo() {
            return;
        }
        self.revisado = Instant::now();
        for p in self.cargador.cambiados() {
            let notas = self
                .galaxia_de(&p)
                .and_then(|g| self.u.astro(g))
                .is_some_and(|a| a.notas_del_chat);
            self.cargador.pedir(&p, notas);
        }
    }

    /// Antes de pintar, fuera del fotograma: que se ve, que cuadernos hacen
    /// falta y que miniaturas subir. Crear recursos de dibujo a medio
    /// fotograma no se puede.
    pub fn preparar(&mut self, motor: &MotorRender, efectiva: &Camara) {
        let e = self.escala();
        // El nivel se mide en pixeles logicos (D218): con la camara del
        // usuario, no con la efectiva.
        let logica = Camara {
            zoom: efectiva.zoom / e,
            ..*efectiva
        };
        let (w, h) = self.tamano_logico();
        self.rejilla.al_dia(&self.u);
        self.vistos =
            pixpin_universo::visibles(&self.u, &self.rejilla, &logica, w, h, &mut self.memoria);

        // D236: se leen las galaxias abiertas; las lejanas se sueltan.
        let ahora = Instant::now();
        let mut cerca: HashSet<String> = HashSet::new();
        for v in &self.vistos {
            let Some(a) = self.u.astro(v.id) else {
                continue;
            };
            let Clase::Galaxia { proyecto } = &a.clase else {
                continue;
            };
            if v.radio_px >= RADIO_LEJANO {
                cerca.insert(proyecto.clone());
            }
            if v.nivel == Nivel::Vista {
                if self.cargador.cargado(proyecto).is_some() {
                    self.cargador.tocar(proyecto, ahora);
                } else {
                    self.cargador.pedir(proyecto, a.notas_del_chat);
                }
            }
        }
        let cargados: Vec<String> = self.nombres.keys().cloned().collect();
        let lejos: HashSet<String> = cargados
            .into_iter()
            .filter(|p| !cerca.contains(p))
            .collect();
        let soltar = self.cargador.a_soltar(ahora, &lejos);
        if !soltar.is_empty() {
            for p in &soltar {
                self.cargador.soltar(p);
            }
            self.rehacer_fichas();
        }
        self.revisar_cuadernos();
        self.rehacer_cuentas();

        if self.estrellas.is_none() {
            let capas = if self.ligero { 1 } else { 3 };
            match Estrellas::nuevas(motor, 0x5eed, capas) {
                Ok(s) => self.estrellas = Some(s),
                Err(err) => tracing::warn!(?err, "sin estrellas de fondo"),
            }
        } else if let Some(s) = self.estrellas.as_mut()
            && !s.listas()
            && let Err(err) = s.volver_a_subir(motor)
        {
            tracing::warn!(?err, "no se pudieron volver a subir las estrellas");
        }
        let fotos: Vec<PathBuf> = self
            .vistos
            .iter()
            .filter(|v| v.nivel == Nivel::Vista)
            .filter_map(|v| self.u.astro(v.id))
            .filter_map(|a| a.codigo().and_then(|c| self.fichas.get(c)))
            .filter_map(|f| pintar::ruta_de_foto(&self.raiz, f))
            .collect();
        self.miniaturas.asegurar(&fotos, motor);
        self.encontrados = self.hallazgos();
    }

    /// El dispositivo de dibujo se perdio: los bitmaps eran del viejo.
    pub fn soltar_recursos(&mut self) {
        if let Some(s) = self.estrellas.as_mut() {
            s.soltar();
        }
        self.miniaturas.soltar();
    }

    /// El cielo y los astros, debajo de las anotaciones. En pixeles de
    /// pantalla: quien llama quita antes la vista del mundo.
    pub fn pintar_detras(&self, p: &Pintor, efectiva: &Camara, ancho: f32, alto: f32) {
        let ahora = Instant::now();
        let destellos: Vec<(IdAstro, f32)> = self
            .destellos
            .iter()
            .map(|(id, t)| {
                (
                    *id,
                    ahora.duration_since(*t).as_secs_f32() / DESTELLO.as_secs_f32(),
                )
            })
            .collect();
        let c = Contexto {
            u: &self.u,
            vistos: &self.vistos,
            fichas: &self.fichas,
            nombres: &self.nombres,
            cuentas: &self.cuentas,
            seleccion: &self.seleccion,
            enfocado: self.enfoque,
            destellos: &destellos,
            ligero: self.ligero,
            escala: self.escala(),
            miniaturas: &self.miniaturas,
            raiz: &self.raiz,
        };
        pintar::fondo(p, self.estrellas.as_ref(), efectiva, ancho, alto);
        pintar::conexiones(p, &c, efectiva);
        pintar::astros(p, &c, efectiva);
        let rotulo = self.textos.t("universo-nebulosa");
        pintar::nebulosas(
            p,
            &c,
            efectiva,
            &self.paginas,
            |proyecto| self.sueltas(proyecto),
            &rotulo,
            |n| {
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("n", n);
                self.textos.t_args("universo-mas", &args)
            },
        );
        // Los pulsos azules: lo que se pidio desde el chat o se acaba de
        // encontrar.
        let e = self.escala();
        for (id, t) in &self.pulsos {
            let Some(a) = self.u.astro(*id) else { continue };
            let fase = ahora.duration_since(*t).as_secs_f32() / PULSO.as_secs_f32();
            let q = efectiva.a_pantalla(Punto2::nuevo(a.x, a.y));
            let r = (a.radio * efectiva.zoom).max(10.0 * e);
            p.anillo(
                (q.x, q.y),
                r * (1.0 + 0.5 * fase),
                3.0 * e,
                pintar::con_alfa(PALETA.acento, 1.0 - fase),
            );
        }
    }

    /// Encima de las anotaciones: los avisos. Los paneles llegan en la
    /// Tarea 15.
    pub fn pintar_delante(&self, p: &Pintor, efectiva: &Camara, ancho: f32, alto: f32) {
        let e = self.escala();
        let pan = self.paneles();
        self.pintar_ruta(p, &pan);
        self.pintar_barra(p, &pan);
        if pan.minimapa.ancho > 0 {
            self.pintar_minimapa(p, &pan, efectiva, ancho, alto);
        }
        if let Some(i) = self.inspector() {
            self.pintar_inspector(p, &pan, &i);
        }
        if self.selector_abierto && self.herramienta == Some(HerramientaUniverso::Emoji) {
            self.pintar_selector(p, &pan);
        }
        if let Some((t, _, _)) = &self.aviso {
            let tam = 14.0 * e;
            let (w, _) = p.medir_texto(t, tam);
            p.texto_con_fondo(
                t,
                (ancho - w) / 2.0,
                alto - 64.0 * e,
                tam,
                PALETA.texto,
                PALETA.panel,
            );
        }
    }

    // --- Paneles (Tarea 15) ------------------------------------------------
    //
    // Todo en pixeles fisicos de la ventana, con el origen en su esquina:
    // la geometria la decide `pixpin_ui::universo`, aqui se pinta y se
    // reacciona.

    fn paneles(&self) -> Paneles {
        Paneles::calcular(
            Rect {
                x: 0,
                y: 0,
                ancho: self.tamano.0 as u32,
                alto: self.tamano.1 as u32,
            },
            self.escala_por_cien,
            !self.seleccion.is_empty(),
            self.minimapa,
        )
    }

    /// El alto de la barra de ruta: el editor pone su caja debajo.
    pub fn alto_ruta(escala_por_cien: u32) -> u32 {
        Paneles::calcular(
            Rect {
                x: 0,
                y: 0,
                ancho: 100,
                alto: 1000,
            },
            escala_por_cien,
            false,
            false,
        )
        .ruta
        .alto
    }

    fn nombre_de(&self, a: &pixpin_universo::Astro) -> String {
        match &a.clase {
            Clase::Galaxia { proyecto } => self
                .nombres
                .get(proyecto)
                .cloned()
                .unwrap_or_else(|| proyecto.clone()),
            Clase::Planeta if !a.nombre.is_empty() => a.nombre.clone(),
            Clase::Planeta if a.padre.is_none() => self.textos.t("universo-exoplaneta"),
            Clase::Planeta => self.textos.t("universo-planeta"),
            Clase::Luna { codigo, .. } => self
                .fichas
                .get(codigo)
                .map(|f| f.nombre.clone())
                .unwrap_or_else(|| self.textos.t("universo-luna")),
        }
    }

    /// «Cosmos > Galaxia > Planeta > Luna» del enfocado, o del elegido.
    fn cadena(&self) -> Vec<(Option<IdAstro>, String)> {
        let mut v = Vec::new();
        let mut actual = self.enfoque.or(self.seleccion.first().copied());
        while let Some(id) = actual {
            let Some(a) = self.u.astro(id) else { break };
            v.push((Some(id), self.nombre_de(a)));
            actual = a.padre;
        }
        v.push((None, self.textos.t("universo-cosmos")));
        v.reverse();
        v
    }

    fn tam_ruta(&self) -> f32 {
        13.0 * self.escala()
    }

    /// Un ancho de texto aproximado: el mismo al pintar y al pulsar, que es
    /// lo que importa, sin necesitar el pintor para medir.
    fn ancho_texto(texto: &str, tam: f32) -> f32 {
        texto.chars().count() as f32 * tam * 0.55
    }

    fn tramos(&self, pan: &Paneles) -> Vec<(Option<IdAstro>, String, Rect)> {
        let e = self.escala();
        let cadena = self.cadena();
        let tam = self.tam_ruta();
        let medidos: Vec<(String, f32)> = cadena
            .iter()
            .map(|(_, t)| (t.clone(), Self::ancho_texto(t, tam)))
            .collect();
        let hueco = (16.0 * e) as u32;
        let disponible = pan
            .ruta
            .ancho
            .saturating_sub(pan.buscador.ancho + 3 * hueco);
        tramos_de_ruta(disponible, &medidos)
            .into_iter()
            .map(|(i, r)| {
                let rect = Rect {
                    x: pan.ruta.x + hueco as i32 + r.x,
                    y: pan.ruta.y,
                    ancho: r.ancho,
                    alto: pan.ruta.alto,
                };
                match cadena.get(i) {
                    Some((id, t)) => (*id, t.clone(), rect),
                    None => (None, "…".to_string(), rect),
                }
            })
            .collect()
    }

    fn filas_encontrados(&self, pan: &Paneles) -> Vec<(Hallazgo, Rect)> {
        if self.busqueda.is_none() {
            return Vec::new();
        }
        let alto = (28.0 * self.escala()) as u32;
        self.encontrados
            .iter()
            .enumerate()
            .map(|(i, h)| {
                (
                    h.clone(),
                    Rect {
                        x: pan.buscador.x,
                        y: pan.ruta.y + pan.ruta.alto as i32 + (i as u32 * alto) as i32,
                        ancho: pan.buscador.ancho,
                        alto,
                    },
                )
            })
            .collect()
    }

    fn barra(&self, pan: &Paneles) -> BarraUniverso {
        BarraUniverso::colocar(pan.lienzo, self.escala_por_cien)
    }

    fn conexiones_de(&self, id: IdAstro) -> Vec<pixpin_universo::Conexion> {
        self.u
            .conexiones
            .iter()
            .filter(|c| c.toca(id))
            .cloned()
            .collect()
    }

    fn clase_inspector(&self) -> Option<FichaInspector> {
        match self.seleccion.as_slice() {
            [] => None,
            [id] => self.u.astro(*id).map(|a| match a.clase {
                Clase::Galaxia { .. } => FichaInspector::Galaxia,
                Clase::Planeta => FichaInspector::Planeta,
                Clase::Luna { .. } => FichaInspector::Luna,
            }),
            v => Some(FichaInspector::Varios(v.len())),
        }
    }

    fn inspector(&self) -> Option<Inspector> {
        let clase = self.clase_inspector()?;
        let conexiones = self
            .seleccion
            .first()
            .map(|id| self.conexiones_de(*id).len())
            .unwrap_or(0);
        Some(Inspector::colocar(
            self.paneles().inspector,
            self.escala_por_cien,
            clase,
            conexiones,
        ))
    }

    /// La caja de todas las galaxias, con aire: lo que ensena el minimapa.
    fn cosmos(&self) -> (f32, f32, f32, f32) {
        union(
            self.u
                .astros
                .iter()
                .filter(|a| matches!(a.clase, Clase::Galaxia { .. }))
                .map(|a| a.caja()),
        )
        .map(|c| agrandar(c, 1.2))
        .unwrap_or((-10_000.0, -10_000.0, 10_000.0, 10_000.0))
    }

    fn nombre_tipo(&self, t: TipoConexion) -> String {
        self.textos.t(match t {
            TipoConexion::Relacion => "universo-tipo-relacion",
            TipoConexion::Depende => "universo-tipo-depende",
            TipoConexion::Referencia => "universo-tipo-referencia",
            TipoConexion::Secuencia(_) => "universo-tipo-secuencia",
        })
    }

    /// Un clic sobre algun panel. `None` si cae en el cielo.
    fn pulsar_panel(
        &mut self,
        p: Punto,
        camara: &mut Camara,
        escena: &mut Escena,
    ) -> Option<Respuesta> {
        let hecho = Some(Respuesta::Consumido { repintar: true });
        let pan = self.paneles();
        if self.selector_abierto && self.herramienta == Some(HerramientaUniverso::Emoji) {
            let marco = self.selector.colocar(pan.lienzo, self.escala_por_cien);
            if marco.contiene(p) {
                if let Some(t) = self.selector.pestana_en(p) {
                    self.selector.pestana = t;
                } else if let Some(e) = self.selector.emoji_en(p) {
                    self.selector.usar(e);
                    self.u.resto.insert(
                        CLAVE_RECIENTES.into(),
                        serde_json::json!(self.selector.recientes),
                    );
                    self.u.marcar_cambio();
                    self.elegir_emoji(e);
                }
                return hecho;
            }
        }
        if let Some(h) = self.barra(&pan).boton_en(p) {
            if self.herramienta == Some(h) {
                self.herramienta = None;
                self.selector_abierto = false;
            } else {
                self.herramienta = Some(h);
                self.selector_abierto =
                    h == HerramientaUniverso::Emoji && self.emoji_elegido.is_none();
            }
            return hecho;
        }
        if pan.ruta.contiene(p) {
            if pan.buscador.contiene(p) {
                self.busqueda.get_or_insert_with(String::new);
            } else if let Some((id, _, _)) = self
                .tramos(&pan)
                .into_iter()
                .find(|(_, t, r)| r.contiene(p) && t != "…")
            {
                match id {
                    Some(id) => self.enfocar(id, camara),
                    None => self.encajar_cosmos(camara),
                }
            }
            return hecho;
        }
        if let Some((h, _)) = self
            .filas_encontrados(&pan)
            .into_iter()
            .find(|(_, r)| r.contiene(p))
        {
            self.busqueda = None;
            self.ir_a_hallazgo(h, camara);
            return hecho;
        }
        if pan.inspector.ancho > 0 && pan.inspector.contiene(p) {
            if let Some(a) = self.inspector().and_then(|i| i.accion_en(p)) {
                return Some(self.accion_inspector(a, camara, escena));
            }
            return Some(Respuesta::Consumido { repintar: false });
        }
        if pan.minimapa.ancho > 0 && pan.minimapa.contiene(p) {
            let (x, y) = minimapa_a_mundo(pan.minimapa, self.cosmos(), p);
            let (w, h) = self.tamano_logico();
            let destino = Camara {
                x: x - w / (2.0 * camara.zoom),
                y: y - h / (2.0 * camara.zoom),
                zoom: camara.zoom,
            };
            self.volar_a(destino, camara);
            return hecho;
        }
        None
    }

    fn accion_inspector(
        &mut self,
        a: AccionInspector,
        camara: &mut Camara,
        escena: &mut Escena,
    ) -> Respuesta {
        let hecho = Respuesta::Consumido { repintar: true };
        let Some(id) = self.seleccion.first().copied() else {
            return hecho;
        };
        let proyecto = self
            .u
            .astro(id)
            .and_then(|x| x.proyecto().map(str::to_string));
        match a {
            AccionInspector::Abrir => {
                let apertura = abrir::que_abre(&self.u, id, &self.fichas, &self.raiz, false);
                return self.resolver_apertura((Some(id), apertura));
            }
            AccionInspector::IrAlChat => {
                if let Some(p) = proyecto {
                    let codigo = self
                        .u
                        .astro(id)
                        .and_then(|x| x.codigo().map(str::to_string));
                    abrir::ejecutar(&Apertura::Chat {
                        proyecto: p,
                        codigo,
                    });
                }
            }
            AccionInspector::MostrarEnCarpeta => {
                let ficha = self
                    .u
                    .astro(id)
                    .and_then(|x| x.codigo())
                    .and_then(|c| self.fichas.get(c));
                match ficha.map(|f| abrir::que_abre_ficha(f, &self.raiz)) {
                    Some(Apertura::Fichero(r)) => {
                        if let Err(e) = pixpin_shell::abrir_ubicacion(&r) {
                            tracing::warn!(?e, "no se pudo ensenar en la carpeta");
                        }
                    }
                    _ => self.destellar(Some(id)),
                }
            }
            AccionInspector::Devolver => {
                self.u.borrar(&[id], escena);
                self.seleccion.clear();
            }
            AccionInspector::Color(i) => {
                let color = COLORES_PLANETA.get(i).copied();
                self.u.editar(id, |x| x.color = color);
            }
            AccionInspector::Tamano(t) => {
                let radio = [RADIO_PLANETA_S, RADIO_PLANETA_M, RADIO_PLANETA_L][t.min(2) as usize];
                self.u.editar(id, |x| x.radio = radio);
            }
            AccionInspector::NotasDelChat => {
                self.u.editar(id, |x| x.notas_del_chat = !x.notas_del_chat);
                // Las notas cambian que fichas hay: se vuelve a leer.
                if let Some(p) = proyecto {
                    let notas = self.u.astro(id).is_some_and(|x| x.notas_del_chat);
                    self.cargador.soltar(&p);
                    self.rehacer_fichas();
                    self.cargador.pedir(&p, notas);
                }
            }
            AccionInspector::Ordenar => self.u.ordenar_galaxia(id),
            AccionInspector::LimpiarHuerfanas => {
                self.u.limpiar_conexiones();
            }
            AccionInspector::Conexion(i) => {
                if let Some(c) = self.conexiones_de(id).get(i) {
                    let siguiente = match c.tipo {
                        TipoConexion::Relacion => TipoConexion::Depende,
                        TipoConexion::Depende => TipoConexion::Referencia,
                        TipoConexion::Referencia => TipoConexion::Secuencia(1),
                        TipoConexion::Secuencia(_) => TipoConexion::Relacion,
                    };
                    self.ultimo_tipo = siguiente;
                    self.u.editar_conexion(c.id, |k| k.tipo = siguiente);
                }
            }
            AccionInspector::AgruparNuevo => {
                let lunas: Vec<IdAstro> = self
                    .seleccion
                    .iter()
                    .copied()
                    .filter(|l| self.u.astro(*l).is_some_and(|x| !x.es_contenedor()))
                    .collect();
                match self.u.agrupar_en_planeta(&lunas) {
                    Ok(p) => self.seleccion = vec![p],
                    Err(e) => {
                        tracing::debug!(?e, "no se pudo agrupar");
                        for l in lunas {
                            self.destellar(Some(l));
                        }
                    }
                }
            }
            AccionInspector::ConectarEntreSi => {
                let ids = self.seleccion.clone();
                for par in ids.windows(2) {
                    self.u.conectar(par[0], par[1], self.ultimo_tipo);
                }
            }
        }
        let _ = camara;
        hecho
    }

    fn pintar_ruta(&self, p: &Pintor, pan: &Paneles) {
        let e = self.escala();
        p.rellenar(rectf(pan.ruta), PALETA.panel);
        p.rellenar(
            RectF {
                x: pan.ruta.x as f32,
                y: (pan.ruta.y + pan.ruta.alto as i32) as f32 - e,
                ancho: pan.ruta.ancho as f32,
                alto: e,
            },
            PALETA.borde_panel,
        );
        let tam = self.tam_ruta();
        let tramos = self.tramos(pan);
        let n = tramos.len();
        for (i, (_, t, r)) in tramos.into_iter().enumerate() {
            let y = r.y as f32 + (r.alto as f32 - tam * 1.3) / 2.0;
            let ultimo = i + 1 == n;
            let color = if ultimo {
                PALETA.texto
            } else {
                PALETA.texto_suave
            };
            p.texto(&t, r.x as f32, y, tam, color);
            if !ultimo {
                p.texto(
                    "›",
                    r.x as f32 + r.ancho as f32 + 5.0 * e,
                    y,
                    tam,
                    PALETA.texto_suave,
                );
            }
        }
        // El buscador.
        let b = rectf(pan.buscador);
        p.rellenar_redondeado(b, 6.0 * e, PALETA.borde_panel);
        let (texto, color) = match &self.busqueda {
            Some(q) => (format!("{q}|"), PALETA.texto),
            None => (self.textos.t("universo-buscar"), PALETA.texto_suave),
        };
        p.texto_linea(
            &texto,
            b.x + 8.0 * e,
            b.y + (b.alto - tam * 1.3) / 2.0,
            tam,
            b.ancho - 16.0 * e,
            color,
        );
        for (h, r) in self.filas_encontrados(pan) {
            let r = rectf(r);
            p.rellenar(r, PALETA.panel);
            let texto = match &h {
                Hallazgo::Astro(id) => self
                    .u
                    .astro(*id)
                    .map(|a| self.nombre_de(a))
                    .unwrap_or_default(),
                Hallazgo::Suelta { codigo, .. } => self
                    .fichas
                    .get(codigo)
                    .map(|f| format!("{} · {}", f.nombre, self.textos.t("universo-nebulosa")))
                    .unwrap_or_default(),
            };
            p.texto_linea(
                &texto,
                r.x + 8.0 * e,
                r.y + (r.alto - tam * 1.3) / 2.0,
                tam,
                r.ancho - 16.0 * e,
                PALETA.texto,
            );
        }
    }

    fn pintar_barra(&self, p: &Pintor, pan: &Paneles) {
        let e = self.escala();
        let barra = self.barra(pan);
        p.rellenar_redondeado(rectf(barra.marco), 8.0 * e, PALETA.panel);
        for (i, h) in HERRAMIENTAS_UNIVERSO.iter().enumerate() {
            let r = rectf(barra.rect_de(i));
            let elegida = self.herramienta == Some(*h);
            if elegida {
                p.rellenar_redondeado(r, 8.0 * e, PALETA.acento);
            }
            let icono = match h {
                HerramientaUniverso::Planeta => &pintar::PLANETA,
                HerramientaUniverso::Emoji => &pintar::CARITA,
                HerramientaUniverso::Conectar => &pintar::LINEA,
            };
            let lado = 18.0 * e;
            p.icono(
                icono,
                RectF {
                    x: r.x + (r.ancho - lado) / 2.0,
                    y: r.y + (r.alto - lado) / 2.0,
                    ancho: lado,
                    alto: lado,
                },
                PALETA.texto,
            );
        }
    }

    fn pintar_minimapa(&self, p: &Pintor, pan: &Paneles, efectiva: &Camara, ancho: f32, alto: f32) {
        let e = self.escala();
        let m = pan.minimapa;
        p.rellenar_redondeado(rectf(m), 6.0 * e, PALETA.panel);
        let cosmos = self.cosmos();
        for a in &self.u.astros {
            let Clase::Galaxia { proyecto } = &a.clase else {
                continue;
            };
            let (x, y) = mundo_a_minimapa(m, cosmos, (a.x, a.y));
            p.circulo((x, y), 3.0 * e, pintar::color_avatar(proyecto));
        }
        // La vista actual, recortada al minimapa.
        let (x0, y0, x1, y1) = efectiva.ventana(ancho, alto);
        let (a0, b0) = mundo_a_minimapa(m, cosmos, (x0, y0));
        let (a1, b1) = mundo_a_minimapa(m, cosmos, (x1, y1));
        let (mx0, my0) = (m.x as f32, m.y as f32);
        let (mx1, my1) = (mx0 + m.ancho as f32, my0 + m.alto as f32);
        let (a0, b0) = (a0.clamp(mx0, mx1), b0.clamp(my0, my1));
        let (a1, b1) = (a1.clamp(mx0, mx1), b1.clamp(my0, my1));
        p.trazar(
            RectF {
                x: a0,
                y: b0,
                ancho: (a1 - a0).max(2.0),
                alto: (b1 - b0).max(2.0),
            },
            1.5 * e,
            PALETA.acento,
        );
    }

    fn pintar_inspector(&self, p: &Pintor, pan: &Paneles, i: &Inspector) {
        let e = self.escala();
        p.rellenar(rectf(pan.inspector), PALETA.panel);
        let unico = match self.seleccion.as_slice() {
            [id] => self.u.astro(*id),
            _ => None,
        };
        let ficha = unico
            .and_then(|a| a.codigo())
            .and_then(|c| self.fichas.get(c));
        let conexiones = unico.map(|a| self.conexiones_de(a.id)).unwrap_or_default();
        for (fila, r) in &i.filas {
            let r = rectf(*r);
            match fila {
                Fila::Cabecera => {
                    let (titulo, tipo) = match unico {
                        Some(a) => (
                            self.nombre_de(a),
                            self.textos.t(match a.clase {
                                Clase::Galaxia { .. } => "universo-galaxia",
                                Clase::Planeta if a.padre.is_none() => "universo-exoplaneta",
                                Clase::Planeta => "universo-planeta",
                                Clase::Luna { .. } => "universo-luna",
                            }),
                        ),
                        None => {
                            let mut args = fluent_bundle::FluentArgs::new();
                            args.set("n", self.seleccion.len());
                            (self.textos.t_args("universo-varios", &args), String::new())
                        }
                    };
                    p.texto_linea(&titulo, r.x, r.y, 16.0 * e, r.ancho, PALETA.texto);
                    p.texto_linea(
                        &tipo,
                        r.x,
                        r.y + 24.0 * e,
                        12.0 * e,
                        r.ancho,
                        PALETA.texto_suave,
                    );
                }
                Fila::VistaPrevia => {
                    if let Some(f) = ficha {
                        let hecha = pintar::ruta_de_foto(&self.raiz, f).and_then(|ruta| {
                            self.miniaturas.ya(&ruta).map(|m| (m.0.clone(), m.1, m.2))
                        });
                        match hecha {
                            Some((b, w, h)) => crate::miniaturas::pintar_recortado(p, &b, r, w, h),
                            None => {
                                let lado = r.alto.min(r.ancho) * 0.6;
                                pintar::icono_de_luna(
                                    p,
                                    f,
                                    RectF {
                                        x: r.x + (r.ancho - lado) / 2.0,
                                        y: r.y + (r.alto - lado) / 2.0,
                                        ancho: lado,
                                        alto: lado,
                                    },
                                    pintar::opacidad_de_luna(f.en_equipo),
                                    e,
                                );
                            }
                        }
                    }
                }
                Fila::Dato(n) => {
                    let texto = match (unico, ficha, n) {
                        (_, Some(f), 0) => pintar::tamano_legible(f.bytes),
                        (_, Some(f), 1) => pintar::extension_de(&f.nombre).to_uppercase(),
                        (_, Some(f), 2) => f.codigo_chat.clone().unwrap_or_default(),
                        (_, Some(f), 3) => {
                            self.nombres.get(&f.proyecto).cloned().unwrap_or_default()
                        }
                        (Some(a), None, 0) => {
                            let mut args = fluent_bundle::FluentArgs::new();
                            args.set("n", self.u.hijos(a.id).count());
                            self.textos.t_args("universo-varios", &args)
                        }
                        (Some(a), None, 1) if matches!(a.clase, Clase::Galaxia { .. }) => {
                            if a.notas_del_chat {
                                format!("✓ {}", self.textos.t("universo-notas-chat"))
                            } else {
                                String::new()
                            }
                        }
                        _ => String::new(),
                    };
                    p.texto_linea(&texto, r.x, r.y, 12.0 * e, r.ancho, PALETA.texto_suave);
                }
                Fila::Nota => {
                    p.rellenar_redondeado(r, 6.0 * e, PALETA.borde_panel);
                    let nota = unico.map(|a| a.nota.clone()).unwrap_or_default();
                    let (t, color) = if nota.is_empty() {
                        (self.textos.t("universo-nota"), PALETA.texto_suave)
                    } else {
                        (nota, PALETA.texto)
                    };
                    p.texto_ajustado(
                        &t,
                        r.x + 6.0 * e,
                        r.y + 6.0 * e,
                        12.0 * e,
                        r.ancho - 12.0 * e,
                        color,
                    );
                }
                Fila::Conexion(n) => {
                    let Some(c) = conexiones.get(*n) else {
                        continue;
                    };
                    let otro = unico
                        .map(|a| if c.desde == a.id { c.hasta } else { c.desde })
                        .and_then(|o| self.u.astro(o))
                        .map(|o| self.nombre_de(o))
                        .unwrap_or_default();
                    p.texto_linea(
                        &format!("{} → {}", self.nombre_tipo(c.tipo), otro),
                        r.x,
                        r.y + 4.0 * e,
                        12.0 * e,
                        r.ancho,
                        PALETA.texto,
                    );
                }
                Fila::Accion(AccionInspector::Color(n)) => {
                    let color = crate::caja_dibujo::hex(COLORES_PLANETA[*n]);
                    p.rellenar_redondeado(r, 6.0 * e, color);
                    if unico.and_then(|a| a.color) == Some(COLORES_PLANETA[*n]) {
                        p.trazar(r, 2.0 * e, PALETA.texto);
                    }
                }
                Fila::Accion(AccionInspector::Tamano(t)) => {
                    p.rellenar_redondeado(r, 6.0 * e, PALETA.borde_panel);
                    let letra = ["S", "M", "L"][(*t).min(2) as usize];
                    let (w, h) = p.medir_texto(letra, 13.0 * e);
                    p.texto(
                        letra,
                        r.x + (r.ancho - w) / 2.0,
                        r.y + (r.alto - h) / 2.0,
                        13.0 * e,
                        PALETA.texto,
                    );
                }
                Fila::Accion(a) => {
                    p.rellenar_redondeado(r, 6.0 * e, PALETA.borde_panel);
                    let clave = match a {
                        AccionInspector::Abrir => "universo-abrir",
                        AccionInspector::IrAlChat => "universo-ir-chat",
                        AccionInspector::MostrarEnCarpeta => "universo-carpeta",
                        AccionInspector::Devolver => "universo-devolver",
                        AccionInspector::NotasDelChat => "universo-notas-chat",
                        AccionInspector::Ordenar => "universo-ordenar",
                        AccionInspector::LimpiarHuerfanas => "universo-limpiar",
                        AccionInspector::AgruparNuevo => "universo-agrupar",
                        AccionInspector::ConectarEntreSi => "universo-conectar",
                        _ => "",
                    };
                    let t = self.textos.t(clave);
                    let tam = 13.0 * e;
                    p.texto_linea(
                        &t,
                        r.x + 10.0 * e,
                        r.y + (r.alto - tam * 1.3) / 2.0,
                        tam,
                        r.ancho - 20.0 * e,
                        PALETA.texto,
                    );
                }
            }
        }
    }

    fn pintar_selector(&self, p: &Pintor, pan: &Paneles) {
        let e = self.escala();
        let mut s = self.selector.clone();
        let marco = s.colocar(pan.lienzo, self.escala_por_cien);
        p.rellenar_redondeado(rectf(marco), 10.0 * e, PALETA.panel);
        for (t, r) in s.pestanas() {
            let r = rectf(r);
            if t == s.pestana && s.filtro.is_empty() {
                p.rellenar_redondeado(r, 6.0 * e, PALETA.borde_panel);
            }
            let icono = match t {
                pixpin_ui::universo::Pestana::Espacio => "🪐",
                pixpin_ui::universo::Pestana::Caras => "😀",
                pixpin_ui::universo::Pestana::Objetos => "📁",
                pixpin_ui::universo::Pestana::Naturaleza => "🌳",
                pixpin_ui::universo::Pestana::Simbolos => "❤️",
                pixpin_ui::universo::Pestana::Recientes => "🕘",
            };
            let tam = 16.0 * e;
            let (w, h) = p.medir_texto(icono, tam);
            p.texto_color(
                icono,
                r.x + (r.ancho - w) / 2.0,
                r.y + (r.alto - h) / 2.0,
                tam,
                PALETA.texto,
            );
        }
        if let Some(f) = s.caja_filtro() {
            let f = rectf(f);
            p.rellenar_redondeado(f, 6.0 * e, PALETA.borde_panel);
            let (t, color) = if s.filtro.is_empty() {
                (self.textos.t("universo-buscar"), PALETA.texto_suave)
            } else {
                (format!("{}|", s.filtro), PALETA.texto)
            };
            p.texto_linea(&t, f.x + 8.0 * e, f.y + 6.0 * e, 13.0 * e, f.ancho, color);
        }
        for (emoji, r) in s.celdas() {
            let r = rectf(r);
            let tam = r.alto * 0.6;
            let (w, h) = p.medir_texto(emoji, tam);
            p.texto_color(
                emoji,
                r.x + (r.ancho - w) / 2.0,
                r.y + (r.alto - h) / 2.0,
                tam,
                PALETA.texto,
            );
        }
    }

    /// Al cerrar: el encuadre se queda y todo se guarda.
    pub fn cerrar(&mut self, escena: &Escena, camara: &Camara) {
        self.u.encuadre = Encuadre {
            x: camara.x,
            y: camara.y,
            zoom: camara.zoom,
        };
        self.arrastre = None;
        let _ = self.guardar(escena);
        self.vuelo = None;
    }

    /// El emoji que se coloca con la herramienta Emoji.
    pub fn elegir_emoji(&mut self, e: &str) {
        self.emoji_elegido = Some(e.to_string());
        self.selector_abierto = false;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_vuelo_empieza_donde_estaba_y_acaba_donde_iba() {
        let a = Camara {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        };
        let b = Camara {
            x: 5000.0,
            y: -300.0,
            zoom: 0.01,
        };
        let i = interpolar(&a, &b, 0.0, 1000.0, 800.0);
        assert!((i.x - a.x).abs() < 1e-2 && (i.zoom - a.zoom).abs() < 1e-6);
        let f = interpolar(&a, &b, 1.0, 1000.0, 800.0);
        assert!((f.x - b.x).abs() < 1.0 && (f.zoom - b.zoom).abs() < 1e-6);
        // A mitad de camino el zoom esta entre los dos, nunca fuera.
        let m = interpolar(&a, &b, 0.5, 1000.0, 800.0);
        assert!(m.zoom < a.zoom && m.zoom > b.zoom);
    }

    #[test]
    fn agrandar_una_caja_la_crece_alrededor_de_su_centro() {
        assert_eq!(
            agrandar((0.0, 0.0, 10.0, 20.0), 2.0),
            (-5.0, -10.0, 15.0, 30.0)
        );
        assert_eq!(union(std::iter::empty()), None);
    }

    fn sesion(u: Universo) -> Sesion {
        Sesion::nueva(
            std::env::temp_dir(),
            u,
            HashMap::new(),
            pixpin_nivel::Nivel::Ligero,
            Catalogo::nuevo(pixpin_store::Idioma::Espanol),
            None,
        )
    }

    #[test]
    fn pedir_una_galaxia_pone_la_camara_sobre_ella_y_una_que_no_existe_no_la_mueve() {
        let mut u = Universo::nuevo();
        u.astros.push(pixpin_universo::Astro::galaxia(
            IdAstro(1),
            "p1",
            50_000.0,
            0.0,
        ));
        let mut s = sesion(u);
        s.pedido = Some(Pedido::Galaxia("p1".into()));
        let c = s.camara_inicial();
        let (w, h) = s.tamano_logico();
        let centro = (c.x + w / (2.0 * c.zoom), c.y + h / (2.0 * c.zoom));
        assert!((centro.0 - 50_000.0).abs() < 1.0, "{centro:?}");
        assert_eq!(s.enfoque, Some(IdAstro(1)));
        // Caso negativo: un proyecto que no esta deja el encuadre guardado.
        s.pedido = Some(Pedido::Galaxia("nada".into()));
        s.u.encuadre = Encuadre {
            x: 1.0,
            y: 2.0,
            zoom: 0.5,
        };
        let c = s.camara_inicial();
        assert_eq!((c.x, c.y, c.zoom), (1.0, 2.0, 0.5));
    }

    #[test]
    fn soltar_en_una_galaxia_va_a_su_proyecto_y_en_el_vacio_o_un_exoplaneta_se_pregunta() {
        use pixpin_universo::Astro;
        let mut u = Universo::nuevo();
        u.astros.push(Astro::galaxia(IdAstro(1), "p1", 0.0, 0.0));
        let mut dentro = Astro::planeta(IdAstro(3), 500.0, 0.0, 400.0);
        dentro.padre = Some(IdAstro(1));
        u.astros.push(dentro);
        u.astros
            .push(Astro::planeta(IdAstro(2), 50_000.0, 0.0, 400.0));
        assert_eq!(
            destino_de_soltar(&u, 100.0, 0.0),
            Destino::Proyecto("p1".into())
        );
        assert_eq!(
            destino_de_soltar(&u, 500.0, 0.0),
            Destino::Proyecto("p1".into())
        );
        assert_eq!(destino_de_soltar(&u, 50_000.0, 0.0), Destino::Preguntar);
        assert_eq!(destino_de_soltar(&u, -90_000.0, 0.0), Destino::Preguntar);
    }

    #[test]
    fn soltar_dos_ficheros_en_una_galaxia_los_mete_en_el_chat_y_los_pone_en_fila() {
        use pixpin_universo::Astro;
        let raiz = std::env::temp_dir().join(format!("pixpin-soltar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let mut u = Universo::nuevo();
        // Con `nuevo_id`, como al cargar: un id puesto a mano chocaria con
        // el de la primera luna.
        let g = u.nuevo_id();
        u.astros.push(Astro::galaxia(g, "p1", 0.0, 0.0));
        let mut s = Sesion::nueva(
            raiz.clone(),
            u,
            HashMap::new(),
            pixpin_nivel::Nivel::Ligero,
            Catalogo::nuevo(pixpin_store::Idioma::Espanol),
            None,
        );
        let ficheros = vec![
            ("a.pdf".to_string(), b"%PDF".to_vec()),
            ("b.txt".to_string(), b"hola".to_vec()),
        ];
        assert!(s.meter_ficheros(&ficheros, Punto2::nuevo(100.0, 50.0)));
        let c = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&pixpin_proyecto::almacen::carpeta(
            &raiz, "p1",
        ))
        .unwrap();
        assert_eq!(c.mensajes.len(), 2, "los dos estan en el chat");
        for (i, m) in c.mensajes.iter().enumerate() {
            let l =
                s.u.luna_de(&m.codigo_unico())
                    .expect("su luna esta colocada");
            assert_eq!(l.padre, Some(g));
            assert_eq!((l.x, l.y), (100.0 + i as f32 * SEPARACION_SOLTADOS, 50.0));
        }
        // Caso negativo: nada que meter no toca nada.
        let antes = s.u.cambios();
        assert!(!s.meter_ficheros(&[], Punto2::nuevo(100.0, 50.0)));
        assert_eq!(s.u.cambios(), antes);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    fn centro(r: Rect) -> Punto {
        Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        }
    }

    #[test]
    fn la_barra_elige_la_herramienta_y_pulsarla_otra_vez_la_deja() {
        let mut s = sesion(Universo::nuevo());
        let (mut c, mut e) = (Camara::nueva(), Escena::nueva());
        let boton = s.barra(&s.paneles()).rect_de(0);
        assert!(s.pulsar_panel(centro(boton), &mut c, &mut e).is_some());
        assert_eq!(s.herramienta, Some(HerramientaUniverso::Planeta));
        s.pulsar_panel(centro(boton), &mut c, &mut e);
        assert_eq!(s.herramienta, None);
        // Caso negativo: un clic en el cielo no es de ningun panel.
        assert!(
            s.pulsar_panel(Punto { x: 900, y: 600 }, &mut c, &mut e)
                .is_none()
        );
    }

    #[test]
    fn elegir_un_emoji_lo_deja_puesto_y_lo_recuerda_en_el_universo() {
        let mut s = sesion(Universo::nuevo());
        let (mut c, mut e) = (Camara::nueva(), Escena::nueva());
        let boton = s.barra(&s.paneles()).rect_de(1);
        s.pulsar_panel(centro(boton), &mut c, &mut e);
        assert!(s.selector_abierto, "sin emoji elegido se abre el selector");
        let lienzo = s.paneles().lienzo;
        let mut sel = s.selector.clone();
        sel.colocar(lienzo, s.escala_por_cien);
        let (emoji, celda) = sel.celdas()[0];
        s.pulsar_panel(centro(celda), &mut c, &mut e);
        assert_eq!(s.emoji_elegido.as_deref(), Some(emoji));
        assert!(!s.selector_abierto);
        assert_eq!(
            s.u.resto.get(CLAVE_RECIENTES),
            Some(&serde_json::json!([emoji]))
        );
    }

    #[test]
    fn con_algo_elegido_sale_el_inspector_y_su_boton_devuelve_la_luna() {
        let mut u = Universo::nuevo();
        u.astros
            .push(pixpin_universo::Astro::galaxia(IdAstro(1), "p1", 0.0, 0.0));
        u.siguiente_id = 2;
        let l = u.colocar_luna("m:a", "p1", 0.0, 0.0).unwrap();
        let mut s = sesion(u);
        let (mut c, mut e) = (Camara::nueva(), Escena::nueva());
        assert!(s.inspector().is_none(), "sin nada elegido no hay inspector");
        s.seleccion = vec![l];
        let i = s.inspector().expect("hay inspector");
        let devolver = i
            .filas
            .iter()
            .find(|(f, _)| *f == Fila::Accion(AccionInspector::Devolver))
            .unwrap()
            .1;
        s.pulsar_panel(centro(devolver), &mut c, &mut e);
        assert!(s.u.astro(l).is_none(), "vuelve a la nebulosa");
        assert!(s.seleccion.is_empty());
    }

    #[test]
    fn sin_nada_pendiente_el_editor_puede_dormir() {
        let s = sesion(Universo::nuevo());
        assert_eq!(s.tope_ms(), None);
    }

    #[test]
    fn un_cambio_programa_el_guardado_y_se_duerme_como_mucho_hasta_entonces() {
        let mut s = sesion(Universo::nuevo());
        s.u.marcar_cambio();
        s.tras_evento(&Escena::nueva());
        let t = s.tope_ms().expect("hay un guardado pendiente");
        assert!(t <= GUARDAR_TRAS.as_millis() as u32 + 1);
    }
}
