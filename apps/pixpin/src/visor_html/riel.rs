//! **El riel de los visores web y del pin de Office** (19-oct-2026): el
//! marcador, el lapiz y el marco, los mismos del PDF y del Word.
//!
//! El usuario: «el lapiz, el marcador y el frame sean universales: que
//! funcionen en todos los formatos y aparezcan en un riel lateral como en el
//! Word y el PDF; con el frame puedo imprimir rapido lo que necesito», «los
//! mismos iconos en todos los visores».
//!
//! # Por que en ventanas propias
//!
//! Un WebView2 (o la vista previa de Office) es una ventana de Windows
//! metida dentro del pin: tapa su sitio y Direct2D no puede pintar encima
//! dentro de la misma ventana. Asi que el riel va en **dos ventanas de
//! DirectComposition**, poseidas por la del visor (siempre encima de ella,
//! se esconden con ella):
//!
//! - **La del riel**: solo lo que ocupa el riel (la isla y los puntos), sin
//!   tapar la pagina. Se pinta con [`crate::lector::pintar_riel`], la misma
//!   funcion que el PDF: mismos iconos, mismo tamano, mismo encendido. Al
//!   pasar por un punto crece hacia la izquierda (el punto se adelanta, como
//!   en el PDF) y con la tira de emoticonos abierta cubre la vista entera.
//!   No se activa al pulsarla: el teclado se queda en la pagina.
//! - **La de la tinta**: la vista entera. Con el lapiz pulsado recoge el
//!   raton y la tinta es la del lienzo ([`Tinta`], la barra y el panel de
//!   siempre); suelto, deja pasar los clics a la pagina y solo ensena lo
//!   dibujado. Sin nada dibujado, ni se ve.
//!
//! # La pagina de debajo
//!
//! Con una pagina ([`Pagina`]: el visor de HTML y el lector de notas) el
//! riel le pregunta por donde va con un guion ([`guion`]): las marcas son la
//! fraccion del desplazamiento (como las del Word) y la tinta va en pixeles
//! CSS del documento, asi que se mueve con la pagina. Con el lapiz pulsado
//! la rueda sigue moviendo la pagina (el riel se lo dice a ella).
//!
//! La vista previa de Office no dice por donde va: alli no hay marcas (el
//! boton sale apagado) y la tinta va pegada a la ventana, solo mientras el
//! pin esta abierto (en otro sitio de la hoja no casaria).
//!
//! Las marcas se guardan como las del Word, en el `.pixpin-lectura` de
//! junto al documento (`pixpin_docs::lectura`), y la tinta en su
//! `.pixpin-anotado/capa.excalidraw` (`lector_tinta::ruta_de_capa`).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use pixpin_docs::lectura::{self, Marcador};
use pixpin_geom::{Punto, Rect};
use pixpin_motor2d::camara::Camara;
use pixpin_render::{Color, MotorRender, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Catalogo;
use pixpin_ui::riel_marcas::{DestinoRiel, PUNTO_ELEGIDO, Riel, SALTO};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

use crate::lector;
use crate::lector_tinta::{self, Capa, Tinta};

const VK_ESCAPE: u32 = 0x1B;
const VK_F11: u32 = 0x7A;
const VK_Z: u32 = 0x5A;
const VK_Y: u32 = 0x59;
/// Lo que se espera sin tocar la tinta antes de guardarla.
const PARA_GUARDAR: Duration = Duration::from_millis(1500);
/// Cada cuanto se pregunta a la pagina por donde va, con tinta a la vista.
const MIRAR_SITIO: Duration = Duration::from_millis(40);
/// Una pregunta sin respuesta en este tiempo se da por perdida.
const SIN_RESPUESTA: Duration = Duration::from_millis(600);
/// Pixeles CSS por muesca de rueda con el lapiz pulsado (los de Edge).
const CSS_POR_MUESCA: f32 = 100.0;

/// **Con quien se habla debajo**: una pagina que corre guiones.
pub trait Pagina {
    fn ejecutar(&self, js: &str, al_volver: Box<dyn FnOnce(String) + 'static>);
}

impl Pagina for pixpin_web::VisorHtml {
    fn ejecutar(&self, js: &str, al_volver: Box<dyn FnOnce(String) + 'static>) {
        pixpin_web::VisorHtml::ejecutar(self, js, al_volver);
    }
}

impl Pagina for pixpin_web::pagina::Pagina {
    fn ejecutar(&self, js: &str, al_volver: Box<dyn FnOnce(String) + 'static>) {
        pixpin_web::pagina::Pagina::ejecutar(self, js, al_volver);
    }
}

/// **Por donde va la pagina**, como lo cuenta el guion: la esquina de
/// arriba a la izquierda de la vista en pixeles CSS del documento (`x`,
/// `y`), lo desplazado (`arriba`) y lo que se puede desplazar (`maximo`),
/// y los pixeles de pantalla por pixel CSS (`dpr`).
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
pub struct Sitio {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub arriba: f32,
    #[serde(default)]
    pub maximo: f32,
    #[serde(default = "uno")]
    pub dpr: f32,
}

fn uno() -> f32 {
    1.0
}

impl Sitio {
    /// Lo leido, de 0 a 1 (una pagina que cabe entera esta siempre en 0).
    pub fn fraccion(&self) -> f32 {
        if self.maximo > 0.5 { (self.arriba / self.maximo).clamp(0.0, 1.0) } else { 0.0 }
    }

    /// La camara de la tinta: documento (pixeles CSS) a pixeles de la vista.
    pub fn camara(&self) -> Camara {
        Camara { x: self.x, y: self.y, zoom: self.dpr.max(0.1) }
    }
}

/// Lo que se le pide a la pagina.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Orden {
    /// Solo por donde va.
    Mirar,
    /// A esa fraccion de lo que se desplaza (una marca).
    Ir(f32),
    /// Tantos pixeles CSS mas abajo (la rueda con el lapiz pulsado).
    Bajar(f32),
}

/// **El guion que se corre en la pagina**: busca lo que se desplaza (el
/// documento o, como en el lector de notas, una caja de dentro: la que
/// tenga el centro de la vista), hace la orden y devuelve el [`Sitio`].
pub fn guion(orden: Orden) -> String {
    let (modo, valor) = match orden {
        Orden::Mirar => ("mirar", 0.0),
        Orden::Ir(f) => ("ir", f.clamp(0.0, 1.0)),
        Orden::Bajar(d) => ("bajar", d),
    };
    format!(
        r#"(function(modo,valor){{
function caja(e){{if(!e||e===document.body||e===document.documentElement)return false;var o=getComputedStyle(e).overflowY;return(o==='auto'||o==='scroll'||o==='overlay')&&e.scrollHeight>e.clientHeight+1;}}
var e=document.elementFromPoint(innerWidth/2,innerHeight/2);while(e&&!caja(e))e=e.parentElement;
var raiz=!e,s=e||document.scrollingElement||document.documentElement;
var max=raiz?Math.max(0,s.scrollHeight-innerHeight):Math.max(0,s.scrollHeight-s.clientHeight);
var w=raiz?window:s;
if(modo==='ir')w.scrollTo({{top:valor*max,behavior:'instant'}});
if(modo==='bajar')w.scrollBy({{top:valor,behavior:'instant'}});
var r;if(raiz){{r={{x:scrollX,y:scrollY,arriba:scrollY,maximo:max}};}}else{{var b=s.getBoundingClientRect();r={{x:s.scrollLeft-b.left,y:s.scrollTop-b.top,arriba:s.scrollTop,maximo:max}};}}
r.dpr=devicePixelRatio||1;return r;}})({modo:?},{valor})"#
    )
}

/// El resultado del guion (JSON) a [`Sitio`]; `None` si no se entiende.
pub fn sitio_de(json: &str) -> Option<Sitio> {
    serde_json::from_str::<Sitio>(json).ok().filter(|s| s.x.is_finite() && s.y.is_finite() && s.dpr.is_finite())
}

/// Como se abre el riel.
pub struct Opciones {
    /// El documento: de el cuelgan las marcas y la tinta guardadas. `None`:
    /// solo mientras este abierto.
    pub documento: Option<PathBuf>,
    /// Si lo de debajo dice por donde va (una pagina): sin eso no hay
    /// marcas y la tinta no se guarda.
    pub con_pagina: bool,
    /// El nombre para el dialogo de imprimir.
    pub titulo: String,
}

/// Lo que el visor tiene que hacer despues de un evento del riel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pide {
    Nada,
    /// Devolverle el teclado a la pagina (se dejo de anotar).
    Teclado,
    /// Una tecla que es del visor (Esc sin nada que soltar no: eso deja de
    /// anotar; F11).
    Tecla(u32),
}

/// Una de las dos ventanas.
struct Capita {
    ventana: VentanaOverlay,
    superficie: Superficie,
    /// Donde esta, en pixeles de pantalla.
    area: Rect,
    visible: bool,
}

impl Capita {
    fn nueva(motor: &MotorRender, d3d: &ID3D11Device, dueno: HWND, area: Rect) -> Option<Capita> {
        let ventana = VentanaOverlay::nueva(area).map_err(|e| tracing::warn!(?e, "riel: sin ventana")).ok()?;
        ventana.poner_dueno(dueno);
        ventana.poner_siempre_encima(false);
        let superficie = Superficie::nueva(motor, d3d, ventana.handle(), area.ancho.max(1), area.alto.max(1))
            .map_err(|e| tracing::warn!(?e, "riel: sin superficie"))
            .ok()?;
        Some(Capita { ventana, superficie, area, visible: false })
    }

    /// A su sitio; `true` si cambio de tamano (hay que repintar).
    fn colocar(&mut self, area: Rect) -> bool {
        if area == self.area {
            return false;
        }
        let tamano = area.ancho != self.area.ancho || area.alto != self.area.alto;
        self.area = area;
        self.ventana.mover(area);
        if tamano {
            let _ = self.superficie.redimensionar(area.ancho.max(1), area.alto.max(1));
        }
        true
    }

    fn mostrar(&mut self, si: bool) {
        if si != self.visible {
            self.visible = si;
            if si { self.ventana.mostrar() } else { self.ventana.ocultar() }
        }
    }
}

/// **El riel de un visor** con su tinta encima.
pub struct RielDeLaVista {
    motor: Rc<MotorRender>,
    riel: Capita,
    tinta_v: Capita,
    /// La ventana del visor (la del marco para imprimir).
    visor: isize,
    /// La vista (la pagina) en pixeles de pantalla.
    vista: Rect,
    escala_por_cien: u32,
    opciones: Opciones,
    marcas: Vec<Marcador>,
    poniendo: bool,
    /// El lapiz pulsado.
    pub anotando: bool,
    encima: DestinoRiel,
    /// Si el riel esta crecido (el raton sobre sus puntos).
    crecido: bool,
    tinta: Tinta,
    capa: Capa,
    ultimo_trazo: Instant,
    trazando: bool,
    /// Lo ultimo que dijo la pagina.
    sitio: Sitio,
    buzon: Rc<RefCell<Vec<Sitio>>>,
    preguntado: Option<Instant>,
    /// La ultima vez que se pregunto por seguirla (no mas de una cada
    /// [`MIRAR_SITIO`]: cada respuesta despierta al bucle, y sin tope se
    /// preguntaria sin parar).
    mirado: Instant,
    /// La marca que espera a saber el sitio para ponerse (el emoticono).
    marca_pendiente: Option<String>,
    /// La tinta sobre papel oscuro (la luna/sol).
    oscuro: bool,
    /// «Elige un emoticono», bajo la tira.
    elige: String,
    fijado: bool,
    repintar_riel: bool,
    repintar_tinta: bool,
}

impl RielDeLaVista {
    /// El riel sobre `vista` (pixeles de pantalla) del visor `dueno`.
    /// `None` sin GPU o sin ventanas: el visor sigue sin riel.
    pub fn nuevo(motor: Rc<MotorRender>, d3d: &ID3D11Device, dueno: &VentanaOverlay, vista: Rect, escala_por_cien: u32, opciones: Opciones, textos: &Catalogo) -> Option<RielDeLaVista> {
        let tinta_v = Capita::nueva(&motor, d3d, dueno.handle(), vista)?;
        tinta_v.ventana.pedir_entrada_fina();
        // El riel, poseido por la de la tinta: siempre por encima de ella.
        let riel = Capita::nueva(&motor, d3d, tinta_v.ventana.handle(), vista)?;
        riel.ventana.poner_sin_activar();
        let (marcas, capa) = match (&opciones.documento, opciones.con_pagina) {
            (Some(d), true) => (lectura::leer(d).marcadores, Capa::leer(&lector_tinta::ruta_de_capa(d))),
            _ => (Vec::new(), Capa::default()),
        };
        let e = escala_por_cien as f32 / 100.0;
        let mut r = RielDeLaVista {
            motor,
            riel,
            tinta_v,
            visor: dueno.handle().0 as isize,
            vista,
            escala_por_cien,
            opciones,
            marcas: ordenadas(marcas),
            poniendo: false,
            anotando: false,
            encima: DestinoRiel::Fuera,
            crecido: false,
            tinta: Tinta::nueva(),
            capa,
            ultimo_trazo: Instant::now(),
            trazando: false,
            sitio: Sitio { x: 0.0, y: 0.0, arriba: 0.0, maximo: 0.0, dpr: e },
            buzon: Rc::new(RefCell::new(Vec::new())),
            preguntado: None,
            mirado: Instant::now(),
            marca_pendiente: None,
            oscuro: false,
            elige: textos.t("marca-elige"),
            fijado: false,
            repintar_riel: true,
            repintar_tinta: true,
        };
        r.recolocar();
        r.riel.mostrar(true);
        // Para las fotos de prueba (solo en debug): nacer con el lapiz pulsado.
        #[cfg(debug_assertions)]
        if std::env::var_os("PIXPIN_RIEL_LAPIZ").is_some() {
            r.alternar_anotar();
        }
        Some(r)
    }

    /// Si `h` es una de sus ventanas (sus eventos van a [`Self::evento`]).
    pub fn es_suya(&self, h: HWND) -> bool {
        h == self.riel.ventana.handle() || h == self.tinta_v.ventana.handle()
    }

    fn e(&self) -> f32 {
        self.escala_por_cien as f32 / 100.0
    }

    /// El riel colocado en la vista (coordenadas de la vista).
    fn colocado(&self) -> Riel {
        lector::riel(self.vista.ancho as f32, self.vista.alto as f32, self.escala_por_cien, self.marcas.len(), self.poniendo)
    }

    /// **Lo que ocupa la ventana del riel**, en coordenadas de la vista: la
    /// isla y los puntos; crecido, tambien a la izquierda de los puntos (el
    /// que tiene el raton se adelanta y lleva su atajo); con la tira, toda
    /// la vista.
    fn area_del_riel(&self, riel: &Riel) -> Rect {
        if riel.tira.is_some() {
            return Rect { x: 0, y: 0, ancho: self.vista.ancho, alto: self.vista.alto };
        }
        let e = self.e();
        let aire = (6.0 * e) as i32;
        let mut x0 = riel.isla.x - aire;
        if self.crecido && !riel.puntos.is_empty() {
            x0 -= ((SALTO + PUNTO_ELEGIDO / 2) as f32 * e + 70.0 * e) as i32;
        }
        let abajo = riel.isla.abajo().max(riel.columna.abajo()) + aire;
        let x0 = x0.max(0);
        // Por la derecha, sin aire: alli esta la barra de desplazamiento.
        let x1 = (riel.isla.x + riel.isla.ancho as i32).min(self.vista.ancho as i32);
        let y0 = (riel.isla.y - aire).max(0);
        let y1 = abajo.min(self.vista.alto as i32);
        Rect { x: x0, y: y0, ancho: (x1 - x0).max(1) as u32, alto: (y1 - y0).max(1) as u32 }
    }

    fn recolocar(&mut self) {
        let riel = self.colocado();
        let a = self.area_del_riel(&riel);
        let en_pantalla = Rect { x: self.vista.x + a.x, y: self.vista.y + a.y, ..a };
        if self.riel.colocar(en_pantalla) {
            self.repintar_riel = true;
        }
        if self.tinta_v.colocar(self.vista) {
            self.repintar_tinta = true;
        }
    }

    /// **La vista se movio o cambio** (el pin se arrastro, se estiro o fue a
    /// pantalla completa), el pin se fijo o cambio la luna/sol.
    pub fn seguir(&mut self, vista: Rect, escala_por_cien: u32, fijado: bool, oscuro: bool) {
        if vista != self.vista || escala_por_cien != self.escala_por_cien {
            self.vista = vista;
            self.escala_por_cien = escala_por_cien;
            self.repintar_riel = true;
            self.repintar_tinta = true;
            self.recolocar();
        }
        if fijado != self.fijado {
            self.fijado = fijado;
            self.tinta_v.ventana.poner_siempre_encima(fijado);
            self.riel.ventana.poner_siempre_encima(fijado);
        }
        if oscuro != self.oscuro {
            self.oscuro = oscuro;
            self.repintar_tinta = true;
        }
    }

    /// El punto del raton (pantalla) en coordenadas de la vista.
    fn local(&self, p: Punto) -> Punto {
        Punto { x: p.x - self.vista.x, y: p.y - self.vista.y }
    }

    fn pedir(&mut self, pagina: Option<&dyn Pagina>, orden: Orden) {
        let Some(pg) = pagina.filter(|_| self.opciones.con_pagina) else { return };
        let buzon = Rc::clone(&self.buzon);
        self.preguntado = Some(Instant::now());
        pg.ejecutar(
            &guion(orden),
            Box::new(move |json| {
                if let Some(s) = sitio_de(&json) {
                    buzon.borrow_mut().push(s);
                }
            }),
        );
    }

    /// **Un evento de una de sus ventanas.**
    pub fn evento(&mut self, h: HWND, ev: &EventoOverlay, pagina: Option<&dyn Pagina>, textos: &Catalogo) -> Pide {
        if h == self.riel.ventana.handle() {
            return self.evento_del_riel(ev, pagina, textos);
        }
        self.evento_de_la_tinta(ev, pagina)
    }

    fn evento_del_riel(&mut self, ev: &EventoOverlay, pagina: Option<&dyn Pagina>, textos: &Catalogo) -> Pide {
        match *ev {
            EventoOverlay::RatonMovido(p) => {
                self.raton_en(p);
                Pide::Nada
            }
            EventoOverlay::BotonPulsado(p) => {
                let destino = self.colocado().destino(self.local(p));
                self.pulsar(destino, pagina, textos)
            }
            // El derecho sobre un punto quita esa marca, como en el PDF.
            EventoOverlay::BotonDerechoPulsado(p) => {
                if let DestinoRiel::Marca(i) = self.colocado().destino(self.local(p))
                    && i < self.marcas.len()
                {
                    self.marcas.remove(i);
                    self.guardar_marcas();
                    self.recolocar();
                    self.repintar_riel = true;
                }
                Pide::Nada
            }
            EventoOverlay::Tecla { vk: VK_ESCAPE, .. } if self.poniendo => {
                self.poniendo = false;
                self.recolocar();
                self.repintar_riel = true;
                Pide::Nada
            }
            EventoOverlay::Pintar | EventoOverlay::CambioDpi => {
                self.repintar_riel = true;
                Pide::Nada
            }
            _ => Pide::Nada,
        }
    }

    /// El raton paso por `p` (pantalla): lo de debajo se resalta y el riel
    /// crece sobre sus puntos.
    fn raton_en(&mut self, p: Punto) {
        let riel = self.colocado();
        let local = self.local(p);
        let encima = riel.destino(local);
        if encima != self.encima {
            self.encima = encima;
            self.repintar_riel = true;
        }
        let a = self.area_del_riel(&riel);
        let dentro = local.x >= a.x && local.y >= a.y && local.x < a.x + a.ancho as i32 && local.y < a.y + a.alto as i32;
        let sobre_los_puntos = matches!(encima, DestinoRiel::Marca(_));
        let crecido = if self.crecido { dentro } else { sobre_los_puntos };
        if crecido != self.crecido {
            self.crecido = crecido;
            self.recolocar();
            self.repintar_riel = true;
        }
    }

    fn pulsar(&mut self, destino: DestinoRiel, pagina: Option<&dyn Pagina>, textos: &Catalogo) -> Pide {
        let mut pide = Pide::Nada;
        match destino {
            DestinoRiel::PonerMarca if self.opciones.con_pagina => {
                self.poniendo = !self.poniendo;
            }
            DestinoRiel::PonerMarca => {}
            DestinoRiel::Hojita => pide = self.alternar_anotar(),
            DestinoRiel::Imprimir => {
                self.poniendo = false;
                self.recolocar();
                crate::marco_imprimir::de_la_ventana(Some(self.visor), &self.opciones.titulo, textos);
            }
            DestinoRiel::Marca(i) => {
                if let Some(m) = self.marcas.get(i) {
                    let f = m.fraccion;
                    self.pedir(pagina, Orden::Ir(f));
                }
            }
            DestinoRiel::Emoji(i) => {
                self.poniendo = false;
                self.marca_pendiente = Some(lector::emoji_de_la_tira(i).to_string());
                self.pedir(pagina, Orden::Mirar);
            }
            DestinoRiel::CerrarTira => self.poniendo = false,
            // Con la tira abierta, un clic fuera de ella la cierra.
            DestinoRiel::Hueco | DestinoRiel::Fuera => self.poniendo = false,
        }
        self.recolocar();
        self.repintar_riel = true;
        pide
    }

    /// **El lapiz**: la ventana de la tinta recoge el raton (y el teclado)
    /// o los suelta a la pagina, guardando lo dibujado.
    pub fn alternar_anotar(&mut self) -> Pide {
        self.anotando = !self.anotando;
        self.poniendo = false;
        self.repintar_riel = true;
        self.repintar_tinta = true;
        if self.anotando {
            self.tinta_v.ventana.poner_pasante(false);
            self.tinta_v.mostrar(true);
            self.riel.ventana.traer_encima();
            self.tinta_v.ventana.enfocar();
            Pide::Nada
        } else {
            self.tinta_v.ventana.soltar_raton();
            self.trazando = false;
            self.guardar_tinta();
            self.tinta_v.ventana.poner_pasante(true);
            Pide::Teclado
        }
    }

    fn evento_de_la_tinta(&mut self, ev: &EventoOverlay, pagina: Option<&dyn Pagina>) -> Pide {
        if !self.anotando {
            return Pide::Nada;
        }
        match *ev {
            EventoOverlay::Pintar | EventoOverlay::CambioDpi => {
                self.repintar_tinta = true;
                return Pide::Nada;
            }
            // La rueda mueve la pagina, aunque se este anotando.
            EventoOverlay::Rueda(d) => {
                self.pedir(pagina, Orden::Bajar(-(d as f32) / 120.0 * CSS_POR_MUESCA));
                return Pide::Nada;
            }
            EventoOverlay::Tecla { vk: VK_F11, .. } => return Pide::Tecla(VK_F11),
            EventoOverlay::Tecla { vk: VK_ESCAPE, .. } if !self.tinta.quiere_escape() => return self.alternar_anotar(),
            EventoOverlay::Tecla { vk: VK_Z, ctrl: true, shift, .. } if !self.tinta.gesto.esta_escribiendo() => {
                if shift { self.capa.rehacer() } else { self.capa.deshacer() };
                self.ultimo_trazo = Instant::now();
                self.repintar_tinta = true;
                return Pide::Nada;
            }
            EventoOverlay::Tecla { vk: VK_Y, ctrl: true, .. } if !self.tinta.gesto.esta_escribiendo() => {
                self.capa.rehacer();
                self.ultimo_trazo = Instant::now();
                self.repintar_tinta = true;
                return Pide::Nada;
            }
            _ => {}
        }
        let camara = self.camara();
        let h = self.tinta.evento(ev, &mut self.capa, &camara, self.vista, self.escala_por_cien);
        if let Some(c) = h.cursor {
            self.tinta_v.ventana.poner_cursor(crate::dibujo::teclas::forma_de(c));
        }
        if h.cambio {
            self.ultimo_trazo = Instant::now();
        }
        if h.arrastra {
            self.trazando = true;
            self.tinta_v.ventana.capturar_raton();
        }
        if matches!(ev, EventoOverlay::BotonSoltado(_)) && self.trazando {
            self.trazando = false;
            self.tinta_v.ventana.soltar_raton();
        }
        self.repintar_tinta = true;
        if h.salir {
            return self.alternar_anotar();
        }
        Pide::Nada
    }

    /// La camara de la tinta: con pagina, la que dice ella; sin, la vista
    /// quieta en pixeles logicos.
    fn camara(&self) -> Camara {
        if self.opciones.con_pagina { self.sitio.camara() } else { Camara { x: 0.0, y: 0.0, zoom: self.e() } }
    }

    /// **Cada vuelta del bucle del visor**: lo que conto la pagina, si el
    /// raton se fue del riel, la forma rapida, guardar la tinta y pintar.
    pub fn tic(&mut self, pagina: Option<&dyn Pagina>) {
        let llegados: Vec<Sitio> = self.buzon.borrow_mut().drain(..).collect();
        if let Some(s) = llegados.last().copied() {
            self.preguntado = None;
            if s != self.sitio {
                self.sitio = s;
                self.repintar_tinta = true;
            }
            if let Some(emoji) = self.marca_pendiente.take() {
                self.poner_marca(s.fraccion(), &emoji);
            }
        }
        if self.preguntado.is_some_and(|t| t.elapsed() > SIN_RESPUESTA) {
            self.preguntado = None;
        }
        // Con tinta a la vista, la pagina se sigue: la tinta va con ella.
        let tinta_a_la_vista = self.anotando || !self.capa.vacia();
        if tinta_a_la_vista && self.preguntado.is_none() && self.opciones.con_pagina && self.mirado.elapsed() >= MIRAR_SITIO {
            self.mirado = Instant::now();
            self.pedir(pagina, Orden::Mirar);
        }
        // El raton se fue sin pasar por el borde (no hay WM_MOUSELEAVE).
        if self.crecido || self.encima != DestinoRiel::Fuera {
            let p = pixpin_shell::overlay::raton_en_pantalla();
            let r = self.riel.area;
            if !(p.x >= r.x && p.y >= r.y && p.x < r.x + r.ancho as i32 && p.y < r.y + r.alto as i32) {
                self.encima = DestinoRiel::Fuera;
                self.crecido = false;
                self.recolocar();
                self.repintar_riel = true;
            }
        }
        let zoom = self.camara().zoom;
        if self.anotando && self.tinta.forma_rapida(&mut self.capa, zoom) {
            self.repintar_tinta = true;
        }
        if self.capa.sucia && !self.tinta.trazando() && self.ultimo_trazo.elapsed() > PARA_GUARDAR {
            self.guardar_tinta();
        }
        self.pintar();
    }

    /// Si quiere que el bucle vuelva pronto (siguiendo la pagina o con un
    /// gesto a medias).
    pub fn quiere_reloj(&self) -> bool {
        self.anotando || self.crecido || self.marca_pendiente.is_some() || (!self.capa.vacia() && self.opciones.con_pagina)
    }

    /// Cuanto puede dormir el bucle como mucho.
    pub fn tope_ms(&self) -> Option<u32> {
        if !self.quiere_reloj() {
            return None;
        }
        let t = MIRAR_SITIO.as_millis() as u32;
        Some(self.tinta.mano.tope_forma_ms().map_or(t, |f| f.min(t)))
    }

    fn poner_marca(&mut self, fraccion: f32, emoji: &str) {
        let ahora = lector::ahora_ms();
        let mut id = ahora;
        while self.marcas.iter().any(|m| m.id == id) {
            id += 1;
        }
        self.marcas.push(Marcador { id, fraccion, emoji: emoji.to_string() });
        // Como en el PDF: 24 como mucho, se va la mas vieja.
        while self.marcas.len() > pixpin_motor2d::marcas::MAXIMO {
            if let Some((i, _)) = self.marcas.iter().enumerate().min_by_key(|(_, m)| m.id) {
                self.marcas.remove(i);
            }
        }
        self.marcas = ordenadas(std::mem::take(&mut self.marcas));
        self.guardar_marcas();
        self.recolocar();
        self.repintar_riel = true;
    }

    fn guardar_marcas(&self) {
        let Some(d) = self.opciones.documento.as_deref().filter(|_| self.opciones.con_pagina) else { return };
        guardar_marcas_en(d, &self.marcas);
    }

    fn guardar_tinta(&mut self) {
        let Some(d) = self.opciones.documento.as_deref().filter(|_| self.opciones.con_pagina) else {
            self.capa.sucia = false;
            return;
        };
        if let Err(e) = self.capa.guardar(&lector_tinta::ruta_de_capa(d)) {
            tracing::warn!(?e, "riel: no se pudo guardar la tinta");
        }
    }

    /// Pinta lo que haga falta.
    fn pintar(&mut self) {
        // La tinta: solo si hay algo que ensenar.
        let tinta_a_la_vista = self.anotando || !self.capa.vacia();
        if !tinta_a_la_vista {
            self.tinta_v.mostrar(false);
        }
        if tinta_a_la_vista && self.repintar_tinta {
            self.repintar_tinta = false;
            self.pintar_tinta();
            if !self.tinta_v.visible {
                self.tinta_v.ventana.poner_pasante(!self.anotando);
                self.tinta_v.mostrar(true);
                self.riel.ventana.traer_encima();
            }
        }
        if self.repintar_riel {
            self.repintar_riel = false;
            self.pintar_riel();
        }
    }

    fn pintar_tinta(&mut self) {
        let (w, h) = (self.tinta_v.area.ancho as f32, self.tinta_v.area.alto as f32);
        let camara = self.camara();
        let papel = lector_tinta::papel(if self.oscuro { pixpin_render::oscuro::PAPEL } else { Color::BLANCO });
        let Ok(destino) = self.tinta_v.superficie.empezar(&self.motor) else { return };
        let (tinta, capa, anotando, vista, escala) = (&mut self.tinta, &self.capa, self.anotando, self.vista, self.escala_por_cien);
        let _ = self.motor.dibujar(&destino, |p| {
            p.limpiar_transparente();
            // Con el lapiz, un velo muy leve: se ve que la pagina esta
            // debajo de la tinta y no se la puede tocar.
            if anotando {
                p.rellenar(RectF { x: 0.0, y: 0.0, ancho: w, alto: h }, Color { r: 0.0, g: 0.0, b: 0.0, a: 0.04 });
            }
            let z = camara.zoom;
            p.poner_vista((0.0, 0.0), z, (-camara.x * z, -camara.y * z));
            tinta.pintar_capa(p, 0, capa, camara.ventana(w, h), z, papel, anotando);
            p.desplazar(0.0, 0.0);
            if anotando {
                tinta.pintar_interfaz(p, Some(capa), vista, escala);
            }
        });
        let _ = self.tinta_v.superficie.presentar();
    }

    fn pintar_riel(&mut self) {
        let riel = self.colocado();
        let a = self.area_del_riel(&riel);
        let emojis: Vec<&str> = self.marcas.iter().map(|m| m.emoji.as_str()).collect();
        let (encima, poniendo, anotando, con_marcas, elige) = (self.encima, self.poniendo, self.anotando, self.opciones.con_pagina, self.elige.as_str());
        let e = self.e();
        let Ok(destino) = self.riel.superficie.empezar(&self.motor) else { return };
        let _ = self.motor.dibujar(&destino, |p| {
            p.limpiar_transparente();
            p.desplazar(-(a.x as f32), -(a.y as f32));
            lector::pintar_riel(p, &riel, &emojis, encima, poniendo, anotando, elige);
            // Sin pagina que diga por donde va (la vista previa de Office)
            // no hay marcas: el boton, apagado.
            if !con_marcas {
                let r = lector::rf(riel.poner_marca);
                p.rellenar_redondeado(r, 6.0 * e, lector::con_alfa(lector::CRISTAL, 0.75));
            }
            p.desplazar(0.0, 0.0);
        });
        let _ = self.riel.superficie.presentar();
    }

    /// Al cerrar el visor: lo dibujado, al disco.
    pub fn cerrar(mut self) {
        self.guardar_tinta();
        self.riel.ventana.ocultar();
        self.tinta_v.ventana.ocultar();
    }
}

/// **La vista en pixeles de pantalla**: la ventana del pin (`ventana`) y la
/// vista dentro de su zona de cliente (`vista`, la de
/// `PinLector::area_de_la_vista`; la ventana no tiene marco).
pub fn en_pantalla(ventana: Rect, vista: Rect) -> Rect {
    Rect { x: ventana.x + vista.x, y: ventana.y + vista.y, ..vista }
}

/// Las marcas en el orden del riel: de arriba abajo.
fn ordenadas(mut v: Vec<Marcador>) -> Vec<Marcador> {
    v.sort_by(|a, b| a.fraccion.partial_cmp(&b.fraccion).unwrap_or(std::cmp::Ordering::Equal));
    v
}

/// Las marcas en el `.pixpin-lectura` del documento, sin tocar lo demas
/// que guarde alli el lector de Word.
fn guardar_marcas_en(documento: &Path, marcas: &[Marcador]) {
    let mut a = lectura::leer(documento);
    if a.marcadores == marcas {
        return;
    }
    a.marcadores = marcas.to_vec();
    if let Some(c) = documento.parent() {
        let _ = std::fs::create_dir_all(c);
    }
    if let Err(e) = lectura::escribir(documento, &a) {
        tracing::warn!(?e, "riel: no se pudieron guardar las marcas");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_sitio_de_la_pagina_se_entiende_y_da_la_fraccion() {
        let s = sitio_de(r#"{"x":0,"y":300,"arriba":300,"maximo":1200,"dpr":1.5}"#).expect("sitio");
        assert_eq!(s.fraccion(), 0.25);
        let c = s.camara();
        assert_eq!((c.x, c.y, c.zoom), (0.0, 300.0, 1.5));
        // Una pagina que cabe entera: siempre arriba.
        let corta = sitio_de(r#"{"x":0,"y":0,"arriba":0,"maximo":0,"dpr":1}"#).expect("sitio");
        assert_eq!(corta.fraccion(), 0.0);
        // Casos negativos: lo que no es un sitio no se toma por uno.
        assert!(sitio_de("null").is_none());
        assert!(sitio_de("\"hola\"").is_none());
    }

    #[test]
    fn el_guion_lleva_su_orden_y_acota_la_fraccion() {
        assert!(guion(Orden::Mirar).ends_with(r#"("mirar",0)"#));
        assert!(guion(Orden::Ir(0.5)).ends_with(r#"("ir",0.5)"#));
        assert!(guion(Orden::Bajar(-100.0)).ends_with(r#"("bajar",-100)"#));
        // Caso negativo: una fraccion fuera de 0..1 no saca la pagina de si.
        assert!(guion(Orden::Ir(3.0)).ends_with(r#"("ir",1)"#));
        assert!(guion(Orden::Ir(-1.0)).ends_with(r#"("ir",0)"#));
    }

    #[test]
    fn las_marcas_van_de_arriba_abajo_y_se_guardan_junto_al_documento() {
        let v = ordenadas(vec![
            Marcador { id: 1, fraccion: 0.8, emoji: "⭐".into() },
            Marcador { id: 2, fraccion: 0.1, emoji: "🔖".into() },
        ]);
        assert_eq!(v.iter().map(|m| m.id).collect::<Vec<_>>(), vec![2, 1]);
        let dir = std::env::temp_dir().join(format!("pixpin-riel-web-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let doc = dir.join("informe.html");
        guardar_marcas_en(&doc, &v);
        assert_eq!(lectura::leer(&doc).marcadores, v);
        // Caso negativo: sin marcas nuevas no se reescribe lo que habia.
        guardar_marcas_en(&doc, &v);
        assert_eq!(lectura::leer(&doc).marcadores.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
