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
use pixpin_render::{MotorRender, Pintor};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Catalogo;
use pixpin_universo::buscar::{Hallazgo, IndiceBusqueda, TOPE_RESULTADOS};
use pixpin_universo::ficha::FichaLuna;
use pixpin_universo::{
    Arrastre, Clase, Encuadre, HerramientaUniverso, IdAstro, Nivel, RADIO_PLANETA_M, RejillaAstros,
    TipoConexion, Universo, Visto, ZOOM_MINIMO_UNIVERSO, nebulosa,
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
}

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
            EventoOverlay::BotonPulsado(p) => self.pulsar(p, camara, escena, ed),
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
            _ => Respuesta::Pasa,
        }
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
    pub fn pintar_delante(&self, p: &Pintor, _efectiva: &Camara, ancho: f32, alto: f32) {
        let e = self.escala();
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
