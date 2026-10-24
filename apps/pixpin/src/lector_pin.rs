//! **Los lectores como un pin** (lo pidio el usuario el 9-oct-2026: «que el
//! visualizador de PDF, docs y EPUB se pueda ver en formato de pin, como es
//! la idea de la app, con la barra de los pines de imagen, como el visor de
//! AutoCAD adaptado, y entre sus opciones la de pantalla completa»).
//!
//! La ventana del lector nace del tamano de un pin, sin marco: se estira por
//! sus bordes, se lleva por el asa de su barra y se queda siempre encima o
//! no (el pin, en azul cuando lo esta). **Nace sin fijar** (13-oct-2026, el
//! usuario: «los archivos que no aparezcan pineados, o sea sobre todas las
//! demas aplicaciones, por defecto»): una ventana normal en el orden Z, al
//! frente al abrirse; el pin la fija. Los pines de captura e imagen no
//! pasan por aqui y siguen siempre encima ([`FIJADO_AL_NACER`]). La barra va FUERA, encima de la
//! ventana y pegada a su lado izquierdo (o debajo si arriba no cabe), como la
//! de los pines y la del visor de planos; aparece con el raton encima y se
//! va un poco despues de salir. Lleva lo que antes iba en la pastilla
//! centrada de arriba (el nombre, el indice, escuchar, los ajustes) y, al
//! final, pantalla completa, el pin y cerrar.
//!
//! En **pantalla completa** el lector es el de siempre (su pastilla de
//! arriba, sin barra fuera), con un boton mas para volver a pin; Esc
//! tambien vuelve.
//!
//! Esto no sabe nada de lo que se lee: da la ventana, los gestos de moverla
//! y estirarla y la barra. Cada lector (`visor.rs`, `lector_pdf.rs`) le dice
//! que botones propios lleva y hace lo que se pulse.
//!
//! Dos cosas son de todos los lectores y por eso van aqui (12-oct-2026):
//! - **Luna/sol**, el modo oscuro de los visores (`crate::oscuro`): lo
//!   cambia la propia barra para toda la app, y [`PinLector::actualizar`]
//!   avisa al lector (devuelve `true`) cuando le toca repintarse, tambien
//!   si lo cambio el pin de otro documento. El que no sabe pintarse en
//!   oscuro (la vista previa de Office) lo quita con [`PinLector::sin_oscuro`].
//! - **El recuadro del tipo de archivo** junto al nombre, del color de los
//!   iconos de extension del chat y con la extension dentro
//!   ([`PinLector::con_extension`]).
//!
//! **El lienzo tambien la usa** (14-oct-2026, el usuario: «haz que los
//! canvas puedan pasar a estar en ventana en vez de pantalla completa, que
//! use la misma barra que los pines»). Lo que el lienzo pide distinto va en
//! variables, no en otra barra: su minimo al estirar es el que deja caber
//! su barra de herramientas ([`PinLector::con_minimo`]) y, como a pantalla
//! completa no tiene pastilla propia desde la que volver, la barra asoma
//! dentro, arriba a la izquierda, al llevar alli el raton
//! ([`PinLector::con_barra_en_pantalla_completa`]).

use pixpin_geom::{Punto, Rect};
use pixpin_render::icono::{Icono, material};
use pixpin_render::{Color, MotorRender, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin, VentanaOverlay};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

/// Lo que se pulso en la barra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccionPin {
    /// Un boton propio del lector (su `id`).
    Propio(u8),
    /// El nombre: cambiarlo.
    Nombre,
    PantallaCompleta,
    Fijar,
    Cerrar,
}

/// Un boton propio del lector en la barra.
#[derive(Clone, Copy)]
pub struct BotonLector {
    pub id: u8,
    pub icono: &'static Icono,
    /// Encendido (dorado): el indice abierto, la voz sonando...
    pub encendido: bool,
}

/// Medidas de la barra en pixeles logicos (las de los pines: 48 de alto,
/// botones de 40).
const ALTO: f32 = 48.0;
const BOTON: f32 = 40.0;
const ASA: f32 = 22.0;
const SEPARACION: f32 = 8.0;
/// El nombre, como mucho.
const NOMBRE_MAX: f32 = 260.0;
const TAM_NOMBRE: f32 = 13.0;
/// El recuadro del tipo de archivo, de lado (y su separacion del nombre).
const CUADRO: f32 = 24.0;
const CUADRO_AIRE: f32 = 7.0;
/// Lo que mide el borde que estira la ventana.
pub(crate) const BORDE: f32 = 7.0;
/// Lo minimo que se deja estirar.
const MINIMO: (u32, u32) = (360, 280);

const FONDO: Color = Color {
    r: 0.118,
    g: 0.118,
    b: 0.125,
    a: 0.97,
};
const TINTA: Color = Color {
    r: 0.96,
    g: 0.96,
    b: 0.97,
    a: 1.0,
};
const TENUE: Color = Color {
    r: 0.78,
    g: 0.78,
    b: 0.80,
    a: 1.0,
};
const ENCIMA: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.12,
};
const AZUL: Color = Color {
    r: 0.0,
    g: 0.376,
    b: 0.875,
    a: 1.0,
};

/// Que hay en cada sitio de la barra (x desde, x hasta, que).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Pieza {
    Asa,
    Nombre,
    Boton(AccionPin),
    /// Luna/sol: lo resuelve la barra (`crate::oscuro::alternar`).
    Oscuro,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Gesto {
    /// Llevando la ventana por el asa.
    Mover { raton: Punto, area: Rect },
    /// Estirando por los bordes (1 izq, 2 der, 4 arriba, 8 abajo).
    Estirar {
        bordes: u8,
        raton: Punto,
        area: Rect,
    },
}

struct Barra {
    ventana: VentanaOverlay,
    superficie: Superficie,
    area: Rect,
}

/// Lo que paso con un evento de la ventana del lector.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Respuesta {
    /// Era del pin (mover o estirar): el lector no hace nada mas con el.
    pub consumido: bool,
    /// La ventana cambio de sitio o de tamano.
    pub cambio: bool,
}

pub struct PinLector {
    pub pantalla_completa: bool,
    /// Siempre encima de las demas aplicaciones.
    pub fijado: bool,
    /// La ventana del lector (la duena de la barra), para saber si otra
    /// aplicacion la tapa donde esta el raton.
    dueno: Option<windows::Win32::Foundation::HWND>,
    /// Donde esta la ventana ahora.
    pub area: Rect,
    /// La de pin, para volver de pantalla completa.
    area_pin: Rect,
    /// Pixeles por pixel logico.
    e: f32,
    gesto: Option<Gesto>,
    barra: Option<Barra>,
    visible: bool,
    fuera_desde: Option<std::time::Instant>,
    encima_de: Option<Pieza>,
    /// El nombre y el atajo de los botones propios del lector, para su globo
    /// (`globo_pista`).
    pistas: Vec<(u8, String)>,
    /// Lo que se pinto la ultima vez (para no repintar si nada cambio): los
    /// botones, el nombre, lo de debajo del raton, el pin y el modo oscuro.
    pintado: Option<(Vec<(u8, bool)>, String, Option<Pieza>, bool, bool)>,
    /// La extension del archivo (`pdf`, `xlsx`...) para el recuadro de color
    /// junto al nombre; vacia, sin recuadro.
    ext: String,
    /// Si lleva el boton luna/sol (el lector sabe pintarse en oscuro).
    con_oscuro: bool,
    /// La `crate::oscuro::version` que ya se le aviso al lector.
    oscuro_visto: u32,
    /// Lo minimo que se deja estirar la ventana, en pixeles fisicos.
    minimo: (u32, u32),
    /// Si a pantalla completa la barra sigue (dentro, arriba a la
    /// izquierda, al acercar el raton) en vez de esconderse.
    barra_en_pantalla_completa: bool,
    /// Sin barra ninguna, ni asomando (el lienzo mientras se edita, 24-oct).
    sin_barra: bool,
    piezas: Vec<(f32, f32, Pieza)>,
    /// Al abrir, la barra se ve un rato aunque el raton no este encima: asi
    /// se sabe que esta (como la pastilla de antes).
    al_abrir: std::time::Instant,
}

/// **Si un visor de archivos nace «siempre encima»: no.** Un documento se
/// abre para leerlo junto a lo demas, como cualquier ventana; quien lo
/// quiera encima pulsa el pin. (Los pines de captura e imagen de
/// `pixpin-pin` si nacen encima: son notas pegadas a la pantalla.)
pub const FIJADO_AL_NACER: bool = false;

/// Si la ventana que se ve bajo el raton (`bajo`) es el lector o su barra.
fn es_mia(
    bajo: windows::Win32::Foundation::HWND,
    lector: windows::Win32::Foundation::HWND,
    barra: Option<windows::Win32::Foundation::HWND>,
) -> bool {
    !bajo.is_invalid() && (bajo == lector || Some(bajo) == barra)
}

/// El sitio de pin al abrir: en el monitor del raton, centrado, del 55 % de
/// ancho y el 86 % de alto de su area de trabajo (una hoja se lee de alto).
pub fn area_inicial() -> (Rect, Rect) {
    let (monitor, trabajo) =
        pixpin_shell::overlay::monitor_de(pixpin_shell::overlay::raton_en_pantalla());
    let w = ((trabajo.ancho as f32 * 0.55) as u32).max(MINIMO.0);
    let h = ((trabajo.alto as f32 * 0.86) as u32).max(MINIMO.1);
    let area = Rect {
        x: trabajo.x + (trabajo.ancho as i32 - w as i32) / 2,
        y: trabajo.y + (trabajo.alto as i32 - h as i32) / 2 + (trabajo.alto as f32 * 0.02) as i32,
        ancho: w,
        alto: h,
    };
    (area, monitor)
}

fn dentro(r: &Rect, p: Punto) -> bool {
    p.x >= r.x && p.y >= r.y && p.x < r.x + r.ancho as i32 && p.y < r.y + r.alto as i32
}

/// Donde va la barra: encima de la ventana si cabe, si no debajo, si no
/// dentro; pegada a su izquierda (como la de los pines y la del plano).
pub fn colocar(ventana: Rect, trabajo: Rect, ancho: u32, alto: u32, sep: i32) -> (i32, i32) {
    let max_x = (trabajo.x + trabajo.ancho as i32 - ancho as i32).max(trabajo.x);
    let x = ventana.x.clamp(trabajo.x, max_x);
    let arriba = ventana.y - sep - alto as i32;
    if arriba >= trabajo.y {
        return (x, arriba);
    }
    let abajo = ventana.y + ventana.alto as i32 + sep;
    if abajo + alto as i32 <= trabajo.y + trabajo.alto as i32 {
        return (x, abajo);
    }
    (x + sep, ventana.y.max(trabajo.y) + sep)
}

/// **Donde va lo que se ve** (una vista de Windows o una pagina web metida
/// dentro de la ventana del pin) en su zona de cliente: todo menos el borde
/// por donde se estira (`borde`, 0 a pantalla completa). Lo usan el pin de
/// Office y los visores de HTML y de Markdown.
pub fn area_de_la_vista(ancho: u32, alto: u32, borde: i32) -> Rect {
    let b = borde.max(0);
    Rect {
        x: b,
        y: b,
        ancho: (ancho as i32 - 2 * b).max(1) as u32,
        alto: (alto as i32 - 2 * b).max(1) as u32,
    }
}

const VK_ESCAPE: u32 = 0x1B;
const VK_F11: u32 = 0x7A;

/// **Lo que pide una tecla** en un visor como pin: `Some(true)` cerrar,
/// `Some(false)` pantalla completa (o volver de ella), `None` nada. F11
/// alterna; Esc sale de pantalla completa y, si no lo esta, cierra.
pub fn tecla(vk: u32, pantalla_completa: bool) -> Option<bool> {
    match vk {
        VK_F11 => Some(false),
        VK_ESCAPE if pantalla_completa => Some(false),
        VK_ESCAPE => Some(true),
        _ => None,
    }
}

/// La ventana estirada desde `area` por `bordes` lo que se movio el raton,
/// sin bajar de `minimo`.
pub fn estirada(area: Rect, bordes: u8, dx: i32, dy: i32, minimo: (u32, u32)) -> Rect {
    let (mx, my) = (minimo.0 as i32, minimo.1 as i32);
    let (mut x0, mut y0) = (area.x, area.y);
    let (mut x1, mut y1) = (area.x + area.ancho as i32, area.y + area.alto as i32);
    if bordes & 1 != 0 {
        x0 = (x0 + dx).min(x1 - mx);
    }
    if bordes & 2 != 0 {
        x1 = (x1 + dx).max(x0 + mx);
    }
    if bordes & 4 != 0 {
        y0 = (y0 + dy).min(y1 - my);
    }
    if bordes & 8 != 0 {
        y1 = (y1 + dy).max(y0 + my);
    }
    Rect {
        x: x0,
        y: y0,
        ancho: (x1 - x0) as u32,
        alto: (y1 - y0) as u32,
    }
}

impl PinLector {
    pub fn nuevo(area: Rect, e: f32) -> PinLector {
        // Para probar a mano que cerrar un visor con el globo hecho no tira
        // la app (20-oct-2026): el globo sale al abrir, sin mover el raton.
        if std::env::var_os("PIXPIN_PRUEBA_GLOBO").is_some() {
            crate::globo_pista::mostrar(
                "prueba",
                Rect {
                    x: area.x + 40,
                    y: area.y,
                    ancho: 40,
                    alto: 30,
                },
                e,
            );
            tracing::info!("globo de prueba puesto (PIXPIN_PRUEBA_GLOBO)");
        }
        PinLector {
            pantalla_completa: false,
            fijado: FIJADO_AL_NACER,
            dueno: None,
            area,
            area_pin: area,
            e,
            gesto: None,
            barra: None,
            visible: false,
            fuera_desde: None,
            encima_de: None,
            pistas: Vec::new(),
            pintado: None,
            piezas: Vec::new(),
            al_abrir: std::time::Instant::now() + std::time::Duration::from_secs(4),
            ext: String::new(),
            con_oscuro: true,
            oscuro_visto: crate::oscuro::version(),
            minimo: MINIMO,
            barra_en_pantalla_completa: false,
            sin_barra: false,
        }
    }

    /// Lo minimo que se deja estirar (pixeles fisicos). El area de ahora y
    /// la de pin crecen hasta el si eran menores.
    pub fn con_minimo(mut self, ancho: u32, alto: u32) -> PinLector {
        self.minimo = (ancho.max(1), alto.max(1));
        let crecer = |r: Rect| Rect {
            ancho: r.ancho.max(ancho),
            alto: r.alto.max(alto),
            ..r
        };
        self.area_pin = crecer(self.area_pin);
        if !self.pantalla_completa {
            self.area = crecer(self.area);
        }
        self
    }

    /// Con la barra tambien a pantalla completa: dentro de la ventana,
    /// arriba a la izquierda, y solo al llevar el raton alli (como la de un
    /// navegador a pantalla completa). Para quien no tiene otro sitio desde
    /// el que volver a pin (el lienzo).
    pub fn con_barra_en_pantalla_completa(mut self) -> PinLector {
        self.barra_en_pantalla_completa = true;
        self
    }

    /// **Sin barra mientras dure** (`true`): ni sale con el raton ni asoma
    /// en su rincon. El lienzo la quita al editar: a pantalla completa, sin
    /// mas botones que los de dibujar (24-oct-2026).
    pub fn quitar_barra(&mut self, si: bool) {
        self.sin_barra = si;
    }

    /// **Nace a pantalla completa** en `monitor`, recordando su sitio de pin
    /// (el `area` con que se creo) para cuando se vuelva. Va antes de crear
    /// la ventana, que nace ya con [`PinLector::area`].
    pub fn en_pantalla_completa(mut self, monitor: Rect) -> PinLector {
        self.area_pin = self.area;
        self.area = monitor;
        self.pantalla_completa = true;
        self
    }

    /// El sitio de pin: el de ahora o, a pantalla completa, al que se
    /// vuelve. Es lo que se recuerda para la siguiente vez.
    pub fn area_pin(&self) -> Rect {
        if self.pantalla_completa {
            self.area_pin
        } else {
            self.area
        }
    }

    /// Otra escala (cambio el DPI del monitor).
    pub fn poner_escala(&mut self, e: f32) {
        if e > 0.0 && (e - self.e).abs() > f32::EPSILON {
            self.e = e;
            self.pintado = None;
        }
    }

    /// Con el recuadro de color del tipo de archivo junto al nombre
    /// (`ext` sin punto, en cualquier caja: «PDF», «xlsx»).
    pub fn con_extension(mut self, ext: &str) -> PinLector {
        self.ext = ext.trim_start_matches('.').to_lowercase();
        self
    }

    /// Sin el boton luna/sol: para el lector que no sabe pintarse en oscuro
    /// (la vista previa de Office es una ventana de Office).
    pub fn sin_oscuro(mut self) -> PinLector {
        self.con_oscuro = false;
        self
    }

    /// Si este lector va en oscuro ahora.
    pub fn oscuro(&self) -> bool {
        self.con_oscuro && crate::oscuro::activo()
    }

    /// **Donde va la vista** metida en la ventana ahora (zona de cliente):
    /// toda menos el borde de estirar, que a pantalla completa no hay.
    pub fn area_de_la_vista(&self) -> Rect {
        let borde = if self.pantalla_completa {
            0
        } else {
            (BORDE * self.e).round() as i32
        };
        area_de_la_vista(self.area.ancho, self.area.alto, borde)
    }

    /// Crea la barra (escondida), duena la ventana del lector: va siempre
    /// por encima de ella.
    pub fn crear_barra(&mut self, motor: &MotorRender, d3d: &ID3D11Device, dueno: &VentanaOverlay) {
        let area = Rect {
            x: self.area.x,
            y: self.area.y,
            ancho: 200,
            alto: (ALTO * self.e).ceil() as u32,
        };
        let Ok(ventana) = VentanaOverlay::nueva(area) else {
            return;
        };
        ventana.poner_sin_activar();
        ventana.poner_dueno(dueno.handle());
        self.dueno = Some(dueno.handle());
        // La barra nace TOPMOST (es un overlay); con el lector sin fijar,
        // como el: por encima de su duena y no de las demas aplicaciones.
        if !self.fijado {
            ventana.poner_siempre_encima(false);
        }
        let Ok(superficie) = Superficie::nueva(motor, d3d, ventana.handle(), area.ancho, area.alto)
        else {
            return;
        };
        self.barra = Some(Barra {
            ventana,
            superficie,
            area,
        });
    }

    /// La ventana de la barra, para repartir los eventos.
    pub fn es_de_la_barra(&self, hwnd: windows::Win32::Foundation::HWND) -> bool {
        self.barra
            .as_ref()
            .is_some_and(|b| b.ventana.handle() == hwnd)
    }

    /// Los bordes de la ventana bajo `p` (vacio en pantalla completa).
    fn bordes_en(&self, p: Punto) -> u8 {
        if self.pantalla_completa || !dentro(&self.area, p) {
            return 0;
        }
        let b = (BORDE * self.e) as i32;
        let r = &self.area;
        let mut k = 0;
        if p.x < r.x + b {
            k |= 1;
        }
        if p.x >= r.x + r.ancho as i32 - b {
            k |= 2;
        }
        if p.y < r.y + b {
            k |= 4;
        }
        if p.y >= r.y + r.alto as i32 - b {
            k |= 8;
        }
        k
    }

    fn poner_area(&mut self, area: Rect, ventana: &mut VentanaOverlay) {
        if area != self.area {
            self.area = area;
            ventana.mover(area);
            if !self.pantalla_completa {
                self.area_pin = area;
            }
        }
    }

    /// **Si otro movio o estiro la ventana** (AltSnap, Win+flecha, un cambio
    /// de monitor), el pin se entera aqui: Windows no avisa con un evento
    /// del overlay y la barra se quedaba donde estaba (10-oct-2026, el
    /// usuario: «se achica normal pero su barra superior se mantiene donde
    /// estaba … cuando lo redimensiono usando AltSnap»). Se mira lo que la
    /// ventana ocupa de verdad y, si no es lo que el pin cree, se toma eso.
    /// `true` si cambio. Mientras el propio pin mueve o estira, manda el.
    pub fn seguir_ventana(&mut self, ventana: &mut VentanaOverlay) -> bool {
        if self.gesto.is_some() {
            return false;
        }
        let Some(real) = ventana.rect_del_sistema() else {
            return false;
        };
        if real == self.area {
            return false;
        }
        self.area = real;
        // `mover` a donde ya esta no la mueve: solo deja al dia lo que la
        // ventana cree que ocupa.
        ventana.mover(real);
        if !self.pantalla_completa {
            self.area_pin = real;
        }
        true
    }

    /// **Un evento de la ventana del lector**: estirar por los bordes.
    pub fn evento_ventana(
        &mut self,
        ev: &EventoOverlay,
        ventana: &mut VentanaOverlay,
    ) -> Respuesta {
        match *ev {
            EventoOverlay::BotonPulsado(p) => {
                let bordes = self.bordes_en(p);
                if bordes != 0 {
                    self.gesto = Some(Gesto::Estirar {
                        bordes,
                        raton: p,
                        area: self.area,
                    });
                    ventana.capturar_raton();
                    return Respuesta {
                        consumido: true,
                        cambio: false,
                    };
                }
            }
            EventoOverlay::RatonMovido(p) => {
                if let Some(Gesto::Estirar {
                    bordes,
                    raton,
                    area,
                }) = self.gesto
                {
                    let nueva = estirada(area, bordes, p.x - raton.x, p.y - raton.y, self.minimo);
                    let cambio = nueva != self.area;
                    self.poner_area(nueva, ventana);
                    return Respuesta {
                        consumido: true,
                        cambio,
                    };
                }
                let forma = match self.bordes_en(p) {
                    0 => None,
                    1 | 2 => Some(FormaCursorWin::RedimEO),
                    4 | 8 => Some(FormaCursorWin::RedimNS),
                    5 | 10 => Some(FormaCursorWin::RedimNoSe),
                    _ => Some(FormaCursorWin::RedimNeSo),
                };
                if let Some(f) = forma {
                    ventana.poner_cursor(f);
                    return Respuesta {
                        consumido: true,
                        cambio: false,
                    };
                }
            }
            EventoOverlay::BotonSoltado(_) if matches!(self.gesto, Some(Gesto::Estirar { .. })) => {
                self.gesto = None;
                ventana.soltar_raton();
                return Respuesta {
                    consumido: true,
                    cambio: false,
                };
            }
            _ => {}
        }
        Respuesta::default()
    }

    /// **Un evento de la barra**: llevar la ventana por el asa, o lo que se
    /// pulse. `Respuesta.cambio` si la ventana se movio.
    pub fn evento_barra(
        &mut self,
        ev: &EventoOverlay,
        ventana: &mut VentanaOverlay,
    ) -> (Option<AccionPin>, Respuesta) {
        let Some(b) = &self.barra else {
            return (None, Respuesta::default());
        };
        let origen = (b.area.x, b.area.y);
        let local = |p: Punto| ((p.x - origen.0) as f32, (p.y - origen.1) as f32);
        match *ev {
            EventoOverlay::RatonMovido(p) => {
                if let Some(Gesto::Mover { raton, area }) = self.gesto {
                    let nueva = Rect {
                        x: area.x + p.x - raton.x,
                        y: area.y + p.y - raton.y,
                        ..area
                    };
                    let cambio = nueva != self.area;
                    self.poner_area(nueva, ventana);
                    return (
                        None,
                        Respuesta {
                            consumido: true,
                            cambio,
                        },
                    );
                }
                let (x, _) = local(p);
                let pieza = self.pieza_en(x);
                if pieza != self.encima_de {
                    self.encima_de = pieza;
                    self.globo(pieza);
                }
                self.fuera_desde = None;
            }
            EventoOverlay::BotonPulsado(p) => {
                let (x, _) = local(p);
                match self.pieza_en(x) {
                    Some(Pieza::Asa) if !self.pantalla_completa => {
                        self.gesto = Some(Gesto::Mover {
                            raton: p,
                            area: self.area,
                        });
                        if let Some(b) = &self.barra {
                            b.ventana.capturar_raton();
                        }
                    }
                    Some(Pieza::Nombre) => return (Some(AccionPin::Nombre), Respuesta::default()),
                    Some(Pieza::Oscuro) => {
                        // Para toda la app; este lector se entera en su
                        // `actualizar`, como los demas abiertos.
                        crate::oscuro::alternar();
                        self.pintado = None;
                        return (
                            None,
                            Respuesta {
                                consumido: true,
                                cambio: false,
                            },
                        );
                    }
                    Some(Pieza::Boton(a)) => return (Some(a), Respuesta::default()),
                    _ => {}
                }
            }
            EventoOverlay::BotonSoltado(_) => {
                if matches!(self.gesto, Some(Gesto::Mover { .. })) {
                    self.gesto = None;
                    if let Some(b) = &self.barra {
                        b.ventana.soltar_raton();
                    }
                }
            }
            _ => {}
        }
        (None, Respuesta::default())
    }

    /// **El nombre y el atajo de cada boton propio** (los del lector), para
    /// el globo que sale al dejar el raton encima.
    pub fn poner_pistas(&mut self, pistas: Vec<(u8, String)>) {
        self.pistas = pistas;
    }

    /// El globo de `pieza` (o ninguno), bajo su sitio en la barra.
    fn globo(&self, pieza: Option<Pieza>) {
        let t = crate::ventana_editor::exportar::textos();
        let texto = match pieza {
            Some(Pieza::Asa) => Some(t.t("barra-mover")),
            Some(Pieza::Oscuro) => Some(t.t("barra-oscuro")),
            Some(Pieza::Boton(AccionPin::PantallaCompleta)) => Some(t.t("barra-pantalla")),
            Some(Pieza::Boton(AccionPin::Fijar)) => Some(t.t("barra-fijar")),
            Some(Pieza::Boton(AccionPin::Cerrar)) => Some(t.t("barra-cerrar")),
            Some(Pieza::Boton(AccionPin::Propio(id))) => self
                .pistas
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, s)| s.clone()),
            _ => None,
        };
        let (Some(texto), Some(b), Some(pieza)) = (texto, &self.barra, pieza) else {
            crate::globo_pista::ocultar();
            return;
        };
        let Some((a, z, _)) = self.piezas.iter().find(|(_, _, p)| *p == pieza) else {
            return;
        };
        let boton = Rect {
            x: b.area.x + *a as i32,
            y: b.area.y,
            ancho: (z - a).max(1.0) as u32,
            alto: b.area.alto,
        };
        crate::globo_pista::mostrar(&texto, boton, self.e);
    }

    fn pieza_en(&self, x: f32) -> Option<Pieza> {
        self.piezas
            .iter()
            .find(|(a, b, _)| x >= *a && x < *b)
            .map(|(_, _, p)| *p)
    }

    /// Pantalla completa, o de vuelta a pin.
    pub fn alternar_pantalla_completa(&mut self, ventana: &mut VentanaOverlay) {
        if self.pantalla_completa {
            self.pantalla_completa = false;
            let a = self.area_pin;
            self.poner_area(a, ventana);
        } else {
            self.area_pin = self.area;
            let centro = Punto {
                x: self.area.x + self.area.ancho as i32 / 2,
                y: self.area.y + self.area.alto as i32 / 2,
            };
            let (monitor, _) = pixpin_shell::overlay::monitor_de(centro);
            self.pantalla_completa = true;
            self.poner_area(monitor, ventana);
            self.esconder_barra();
        }
        ventana.enfocar();
    }

    pub fn alternar_fijado(&mut self, ventana: &VentanaOverlay) {
        self.fijado = !self.fijado;
        ventana.poner_siempre_encima(self.fijado);
        // Windows lleva las poseidas con su duena al cambiarla, pero no
        // cuesta nada decirselo a la barra tambien.
        if let Some(b) = &self.barra {
            b.ventana.poner_siempre_encima(self.fijado);
        }
        self.pintado = None;
    }

    /// **El orden Z al abrir** (llamarlo una vez, con la ventana ya creada):
    /// sin fijar ([`FIJADO_AL_NACER`]), al frente de las aplicaciones
    /// normales pero sin «siempre encima» y sin robar el foco; fijado,
    /// siempre encima.
    pub fn al_nacer(&self, ventana: &VentanaOverlay) {
        if self.fijado {
            ventana.poner_siempre_encima(true);
        } else {
            ventana.al_frente_sin_fijar();
        }
    }

    fn esconder_barra(&mut self) {
        if self.visible
            && let Some(b) = &self.barra
        {
            b.ventana.ocultar();
        }
        self.visible = false;
        self.encima_de = None;
        crate::globo_pista::ocultar();
    }

    /// Si hay que seguir despertando al bucle (la barra puede irse sola).
    pub fn quiere_reloj(&self) -> bool {
        self.visible || self.gesto.is_some() || std::time::Instant::now() < self.al_abrir
    }

    /// **La barra, al dia**: sale con el raton encima de la ventana o de
    /// ella, se va un poco despues de salir de las dos, sigue a la ventana y
    /// se repinta si cambio algo.
    ///
    /// Devuelve `true` si el lector tiene que repintarse porque cambio el
    /// modo oscuro (con su luna/sol o con la de otro lector abierto).
    pub fn actualizar(
        &mut self,
        motor: &MotorRender,
        nombre: &str,
        propios: &[BotonLector],
    ) -> bool {
        let v = crate::oscuro::version();
        let cambio = self.con_oscuro && v != self.oscuro_visto;
        self.oscuro_visto = v;
        if cambio {
            self.pintado = None;
        }
        self.actualizar_barra(motor, nombre, propios);
        cambio
    }

    /// El ancho de la pieza del nombre: el recuadro (si hay extension) y el
    /// texto, como mucho `NOMBRE_MAX`.
    fn ancho_nombre(nombre: &str, ext: &str, e: f32) -> f32 {
        let cuadro = if ext.is_empty() {
            0.0
        } else {
            (CUADRO + CUADRO_AIRE) * e
        };
        let texto = if nombre.is_empty() {
            0.0
        } else {
            (nombre.chars().count() as f32 * TAM_NOMBRE * 0.56 * e).min(NOMBRE_MAX * e)
        };
        if cuadro + texto == 0.0 {
            0.0
        } else {
            cuadro + texto + 16.0 * e
        }
    }

    fn actualizar_barra(&mut self, motor: &MotorRender, nombre: &str, propios: &[BotonLector]) {
        if self.sin_barra
            || (self.pantalla_completa && !self.barra_en_pantalla_completa)
            || self.barra.is_none()
        {
            self.esconder_barra();
            return;
        }
        // Las piezas y lo que mide: no dependen del raton.
        let e = self.e;
        let (piezas, ancho) = piezas_de(nombre, &self.ext, propios, self.con_oscuro, e);
        let alto = (ALTO * e).ceil() as u32;
        let sep = (SEPARACION * e) as i32;
        let raton = pixpin_shell::overlay::raton_en_pantalla();
        let en_la_barra = self
            .barra
            .as_ref()
            .is_some_and(|b| self.visible && dentro(&b.area, raton));
        // Sin fijar, otra aplicacion puede taparla: el raton sobre esa otra
        // no cuenta como encima del lector (si no, la barra saldria sobre
        // ella).
        let tapada = !self.fijado
            && self.dueno.is_some_and(|d| {
                !es_mia(
                    pixpin_shell::overlay::ventana_bajo(raton),
                    d,
                    self.barra.as_ref().map(|b| b.ventana.handle()),
                )
            });
        // A pantalla completa la barra va dentro y no sale con el raton en
        // cualquier sitio (taparia lo que se esta haciendo): solo en su
        // rincon, arriba a la izquierda.
        let donde_asoma = if self.pantalla_completa {
            rincon_de_la_barra(self.area, ancho, alto, sep)
        } else {
            self.area
        };
        let quiere = (dentro(&donde_asoma, raton) && !tapada)
            || en_la_barra
            || self.gesto.is_some()
            || std::time::Instant::now() < self.al_abrir;
        if quiere {
            self.fuera_desde = None;
        } else if self.visible && self.fuera_desde.is_none() {
            self.fuera_desde = Some(std::time::Instant::now());
        }
        let mostrar = quiere
            || self
                .fuera_desde
                .is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(600));
        if !mostrar {
            self.fuera_desde = None;
            self.esconder_barra();
            return;
        }
        if !en_la_barra && self.encima_de.is_some() && self.gesto.is_none() {
            self.encima_de = None;
        }
        let estado: Vec<(u8, bool)> = propios.iter().map(|b| (b.id, b.encendido)).collect();
        let oscuro = self.oscuro();
        let quieto = self.pintado.as_ref().is_some_and(|(s, n, h, f, o)| {
            *s == estado && n == nombre && *h == self.encima_de && *f == self.fijado && *o == oscuro
        });
        self.piezas = piezas;
        let (_, trabajo) = pixpin_shell::overlay::monitor_de(Punto {
            x: self.area.x + 20,
            y: self.area.y + 20,
        });
        let (bx, by) = colocar(self.area, trabajo, ancho, alto, sep);
        let area = Rect {
            x: bx,
            y: by,
            ancho,
            alto,
        };
        let aspecto = Aspecto {
            encima_de: self.encima_de,
            fijado: self.fijado,
            oscuro,
            pantalla_completa: self.pantalla_completa,
        };
        let Some(b) = &mut self.barra else { return };
        let redimensionada = b.area.ancho != ancho || b.area.alto != alto;
        // Se movio: tambien al pasar a pantalla completa (va dentro) y al
        // volver (fuera), que es cuando cambia el icono de pantalla completa.
        let movida = b.area != area;
        if movida {
            b.area = area;
            b.ventana.mover(area);
        }
        if redimensionada {
            let _ = b.superficie.redimensionar(ancho, alto);
        }
        if !self.visible {
            b.ventana.mostrar();
            if self.fijado {
                b.ventana.traer_encima();
            } else {
                b.ventana.poner_siempre_encima(false);
            }
            self.visible = true;
        }
        if quieto && !redimensionada && !movida {
            return;
        }
        self.pintado = Some((
            estado,
            nombre.to_string(),
            self.encima_de,
            self.fijado,
            oscuro,
        ));
        let (piezas, ext) = (&self.piezas, &self.ext);
        let Ok(destino) = b.superficie.empezar(motor) else {
            return;
        };
        let _ = motor.dibujar(&destino, |p| {
            p.limpiar_transparente();
            pintar_barra(
                p,
                (ancho as f32, alto as f32),
                e,
                piezas,
                nombre,
                ext,
                propios,
                aspecto,
            );
        });
        let _ = b.superficie.presentar();
    }
}

/// Como se ve la barra ahora: lo que hay bajo el raton y los interruptores.
#[derive(Debug, Clone, Copy, Default)]
struct Aspecto {
    encima_de: Option<Pieza>,
    fijado: bool,
    oscuro: bool,
    pantalla_completa: bool,
}

/// **Las piezas de la barra** de izquierda a derecha (x desde, x hasta,
/// que) y su ancho total en pixeles: el asa, el nombre con su recuadro, los
/// botones propios, la luna/sol si la lleva y, al final, pantalla completa,
/// el pin y cerrar.
fn piezas_de(
    nombre: &str,
    ext: &str,
    propios: &[BotonLector],
    con_oscuro: bool,
    e: f32,
) -> (Vec<(f32, f32, Pieza)>, u32) {
    let mut piezas = Vec::new();
    let mut x = 4.0 * e;
    piezas.push((x, x + ASA * e, Pieza::Asa));
    x += ASA * e + 2.0 * e;
    let ancho_nombre = PinLector::ancho_nombre(nombre, ext, e);
    if ancho_nombre > 0.0 {
        piezas.push((x, x + ancho_nombre, Pieza::Nombre));
        x += ancho_nombre + 2.0 * e;
    }
    for b in propios {
        piezas.push((x, x + BOTON * e, Pieza::Boton(AccionPin::Propio(b.id))));
        x += BOTON * e + 2.0 * e;
    }
    x += 6.0 * e;
    if con_oscuro {
        piezas.push((x, x + BOTON * e, Pieza::Oscuro));
        x += BOTON * e + 2.0 * e;
    }
    for a in [
        AccionPin::PantallaCompleta,
        AccionPin::Fijar,
        AccionPin::Cerrar,
    ] {
        piezas.push((x, x + BOTON * e, Pieza::Boton(a)));
        x += BOTON * e + 2.0 * e;
    }
    (piezas, (x + 2.0 * e).ceil() as u32)
}

/// Donde asoma la barra a pantalla completa: su rincon de arriba a la
/// izquierda de la ventana, con el aire de su separacion alrededor.
fn rincon_de_la_barra(ventana: Rect, ancho: u32, alto: u32, sep: i32) -> Rect {
    let aire = sep.max(0) as u32 * 2;
    Rect {
        x: ventana.x,
        y: ventana.y,
        ancho: (ancho + aire).min(ventana.ancho),
        alto: (alto + aire).min(ventana.alto),
    }
}

/// **Pinta la barra** en `p` (de `tam` pixeles), sin limpiar antes.
#[allow(clippy::too_many_arguments)]
fn pintar_barra(
    p: &pixpin_render::Pintor,
    tam: (f32, f32),
    e: f32,
    piezas: &[(f32, f32, Pieza)],
    nombre: &str,
    ext: &str,
    propios: &[BotonLector],
    aspecto: Aspecto,
) {
    let (w, h) = tam;
    p.rellenar_redondeado(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: h,
        },
        10.0 * e,
        FONDO,
    );
    let lado = BOTON * e;
    let y0 = (h - lado) / 2.0;
    let icono = 20.0 * e;
    for (a, z, pieza) in piezas {
        let caja = RectF {
            x: *a,
            y: y0,
            ancho: z - a,
            alto: lado,
        };
        // Pulsado = relleno azul con el icono blanco, igual para fijar y para
        // los propios encendidos (el lapiz al editar): un solo aspecto en
        // todos los visores (19-oct-2026).
        let activo = match pieza {
            Pieza::Boton(AccionPin::Fijar) => aspecto.fijado,
            Pieza::Boton(AccionPin::Propio(id)) => {
                propios.iter().any(|b| b.id == *id && b.encendido)
            }
            _ => false,
        };
        if activo {
            p.rellenar_redondeado(caja, 8.0 * e, AZUL);
        } else if aspecto.encima_de == Some(*pieza) && *pieza != Pieza::Asa {
            p.rellenar_redondeado(caja, 8.0 * e, ENCIMA);
        }
        let centrado = RectF {
            x: caja.x + (caja.ancho - icono) / 2.0,
            y: caja.y + (lado - icono) / 2.0,
            ancho: icono,
            alto: icono,
        };
        match pieza {
            Pieza::Asa => p.icono(&material::DRAG_INDICATOR, centrado, TENUE),
            Pieza::Nombre => {
                let mut x = caja.x + 8.0 * e;
                if !ext.is_empty() {
                    cuadro_de_tipo(
                        p,
                        ext,
                        RectF {
                            x,
                            y: caja.y + (lado - CUADRO * e) / 2.0,
                            ancho: CUADRO * e,
                            alto: CUADRO * e,
                        },
                        e,
                    );
                    x += (CUADRO + CUADRO_AIRE) * e;
                }
                let (_, th) = p.medir_texto("Ag", TAM_NOMBRE * e);
                p.texto_linea(
                    nombre,
                    x,
                    caja.y + (lado - th) / 2.0,
                    TAM_NOMBRE * e,
                    caja.x + caja.ancho - 8.0 * e - x,
                    TINTA,
                );
            }
            // La luna para pasar a oscuro; en oscuro, el sol para volver.
            Pieza::Oscuro => p.icono(
                if aspecto.oscuro {
                    &material::LIGHT_MODE
                } else {
                    &material::DARK_MODE
                },
                centrado,
                TINTA,
            ),
            Pieza::Boton(AccionPin::Propio(id)) => {
                if let Some(b) = propios.iter().find(|b| b.id == *id) {
                    p.icono(
                        b.icono,
                        centrado,
                        if b.encendido {
                            Color {
                                r: 1.0,
                                g: 1.0,
                                b: 1.0,
                                a: 1.0,
                            }
                        } else {
                            TINTA
                        },
                    );
                }
            }
            // A pantalla completa (si la barra sigue alli), el de salir de ella.
            Pieza::Boton(AccionPin::PantallaCompleta) => p.icono(
                if aspecto.pantalla_completa {
                    &material::FULLSCREEN_EXIT
                } else {
                    &material::FULLSCREEN
                },
                centrado,
                TINTA,
            ),
            Pieza::Boton(AccionPin::Fijar) => p.icono(
                &material::PUSH_PIN,
                centrado,
                if activo {
                    Color {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    }
                } else {
                    TINTA
                },
            ),
            Pieza::Boton(AccionPin::Cerrar) => p.icono(&material::CLOSE, centrado, TINTA),
            Pieza::Boton(AccionPin::Nombre) => {}
        }
    }
}

/// **La barra pintada en otro sitio**, para las muestras de quien la usa
/// (el lienzo en ventana): sin botones propios, en `origen`. Devuelve lo
/// que mide.
#[cfg(test)]
pub(crate) fn pintar_barra_de_muestra(
    p: &pixpin_render::Pintor,
    origen: (f32, f32),
    e: f32,
    nombre: &str,
    ext: &str,
    con_oscuro: bool,
    pantalla_completa: bool,
) -> (u32, u32) {
    let (piezas, ancho) = piezas_de(nombre, ext, &[], con_oscuro, e);
    let alto = (ALTO * e).ceil() as u32;
    p.desplazar(origen.0, origen.1);
    pintar_barra(
        p,
        (ancho as f32, alto as f32),
        e,
        &piezas,
        nombre,
        ext,
        &[],
        Aspecto {
            pantalla_completa,
            ..Aspecto::default()
        },
    );
    p.desplazar(0.0, 0.0);
    (ancho, alto)
}

/// **La disposicion de la barra** de izquierda a derecha, por nombres, para
/// probar sin ventana lo que lleva cada visor: `asa`, `nombre`,
/// `propio:<id>`, `oscuro`, `pantalla`, `fijar` y `cerrar`.
#[cfg(test)]
pub(crate) fn disposicion(
    nombre: &str,
    ext: &str,
    propios: &[BotonLector],
    con_oscuro: bool,
) -> Vec<String> {
    piezas_de(nombre, ext, propios, con_oscuro, 1.0)
        .0
        .into_iter()
        .map(|(_, _, p)| match p {
            Pieza::Asa => "asa".to_string(),
            Pieza::Nombre => "nombre".to_string(),
            Pieza::Oscuro => "oscuro".to_string(),
            Pieza::Boton(AccionPin::Propio(id)) => format!("propio:{id}"),
            Pieza::Boton(AccionPin::PantallaCompleta) => "pantalla".to_string(),
            Pieza::Boton(AccionPin::Fijar) => "fijar".to_string(),
            Pieza::Boton(AccionPin::Cerrar) => "cerrar".to_string(),
            Pieza::Boton(AccionPin::Nombre) => "nombre".to_string(),
        })
        .collect()
}

/// Lo que mide la barra (ancho, alto) en pixeles, para colocar la muestra.
#[cfg(test)]
pub(crate) fn medida_de_la_barra(e: f32, nombre: &str, ext: &str, con_oscuro: bool) -> (u32, u32) {
    (
        piezas_de(nombre, ext, &[], con_oscuro, e).1,
        (ALTO * e).ceil() as u32,
    )
}

/// El color del recuadro del tipo de archivo: el de los iconos de extension
/// del chat (Excel verde, Word azul, PDF rojo...).
pub fn color_de_tipo(ext: &str) -> Color {
    let (r, g, b) = pixpin_ui::chat::color_de_extension(ext);
    Color { r, g, b, a: 1.0 }
}

/// **El recuadro del tipo de archivo**: un cuadradito redondeado del color
/// de su extension con la extension en pequeno dentro, en blanco o en
/// oscuro segun lo pida su color, como el icono de archivo del chat.
fn cuadro_de_tipo(p: &pixpin_render::Pintor, ext: &str, r: RectF, e: f32) {
    p.rellenar_redondeado(r, 5.0 * e, color_de_tipo(ext));
    let nombre = format!("x.{ext}");
    let rotulo = pixpin_ui::color_de_extension::rotulo(&nombre);
    if rotulo.is_empty() {
        return;
    }
    let tinta = if pixpin_ui::color_de_extension::de(&nombre).texto_oscuro {
        Color {
            r: 0x3a as f32 / 255.0,
            g: 0x2b as f32 / 255.0,
            b: 0.0,
            a: 1.0,
        }
    } else {
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }
    };
    // La letra mas grande que deja la extension dentro, con un margen.
    let mut tam = 9.5 * e;
    let (mut w, mut h) = p.medir_texto(&rotulo, tam);
    let cabe = r.ancho - 4.0 * e;
    if w > cabe && w > 0.0 {
        tam *= cabe / w;
        (w, h) = p.medir_texto(&rotulo, tam);
    }
    p.texto_linea(
        &rotulo,
        r.x + (r.ancho - w) / 2.0,
        r.y + (r.alto - h) / 2.0,
        tam,
        r.ancho,
        tinta,
    );
}

impl Drop for PinLector {
    fn drop(&mut self) {
        self.esconder_barra();
        crate::globo_pista::soltar();
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_barra_va_encima_pegada_a_la_izquierda_y_si_no_cabe_debajo() {
        let t = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        let v = Rect {
            x: 100,
            y: 300,
            ancho: 800,
            alto: 500,
        };
        assert_eq!(colocar(v, t, 400, 48, 8), (100, 244));
        let v = Rect {
            x: 100,
            y: 20,
            ancho: 800,
            alto: 500,
        };
        assert_eq!(colocar(v, t, 400, 48, 8), (100, 528));
        // Caso negativo: sin sitio fuera, dentro y arriba.
        let v = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        assert_eq!(colocar(v, t, 400, 48, 8), (8, 8));
    }

    #[test]
    fn el_recuadro_del_tipo_lleva_el_color_del_chat_y_hace_sitio() {
        // Excel verde, Word azul, PDF rojo: los del chat.
        let (x, w, pdf) = (
            color_de_tipo("xlsx"),
            color_de_tipo("DOCX"),
            color_de_tipo("pdf"),
        );
        assert!(x.g > x.r && x.g > x.b, "excel {x:?}");
        assert!(w.b > w.r && w.b > w.g, "word {w:?}");
        assert!(pdf.r > pdf.g && pdf.r > pdf.b, "pdf {pdf:?}");
        let (r, g, b) = pixpin_ui::chat::color_de_extension("xlsx");
        assert_eq!((x.r, x.g, x.b), (r, g, b));
        // El nombre hace sitio al recuadro.
        let con = PinLector::ancho_nombre("Tesis", "pdf", 1.0);
        let sin = PinLector::ancho_nombre("Tesis", "", 1.0);
        assert!((con - sin - (CUADRO + CUADRO_AIRE)).abs() < 1e-3);
        // Caso negativo: sin nombre ni extension, no hay pieza.
        assert_eq!(PinLector::ancho_nombre("", "", 1.0), 0.0);
        // Con solo la extension, el recuadro se ve.
        assert!(PinLector::ancho_nombre("", "pdf", 1.0) > CUADRO);
    }

    #[test]
    fn la_extension_se_guarda_sin_punto_y_en_minusculas() {
        let a = Rect {
            x: 0,
            y: 0,
            ancho: 800,
            alto: 600,
        };
        let p = PinLector::nuevo(a, 1.0).con_extension(".XLSX");
        assert_eq!(p.ext, "xlsx");
        assert!(p.con_oscuro);
        // Caso negativo: la vista previa de Office no lleva luna/sol.
        let q = PinLector::nuevo(a, 1.0).sin_oscuro();
        assert!(!q.con_oscuro && !q.oscuro());
    }

    #[test]
    fn un_visor_de_archivos_nace_sin_fijar() {
        let a = Rect {
            x: 0,
            y: 0,
            ancho: 800,
            alto: 600,
        };
        const { assert!(!FIJADO_AL_NACER) };
        let p = PinLector::nuevo(a, 1.0);
        assert!(!p.fijado, "nace como una ventana mas");
        // Tambien el de Office (sin luna/sol) y con extension.
        assert!(
            !PinLector::nuevo(a, 1.0)
                .sin_oscuro()
                .con_extension("xls")
                .fijado
        );
    }

    #[test]
    fn el_raton_sobre_otra_aplicacion_no_saca_la_barra() {
        use windows::Win32::Foundation::HWND;
        let (lector, barra, otra) = (HWND(1 as _), HWND(2 as _), HWND(3 as _));
        assert!(es_mia(lector, lector, Some(barra)));
        assert!(es_mia(barra, lector, Some(barra)));
        // Casos negativos: otra aplicacion encima, o nada bajo el raton.
        assert!(!es_mia(otra, lector, Some(barra)));
        assert!(!es_mia(HWND::default(), lector, None));
    }

    /// Lo que hace el lector al nacer, sobre una ventana de verdad pero sin
    /// ensenarla (ni foco ni nada en la pantalla del usuario).
    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn al_nacer_no_queda_siempre_encima_y_el_pin_lo_alterna() {
        let a = Rect {
            x: -3000,
            y: -3000,
            ancho: 300,
            alto: 200,
        };
        for v in [
            VentanaOverlay::nueva_normal(a, "prueba").unwrap(),
            VentanaOverlay::nueva_llana(a, "prueba").unwrap(),
        ] {
            let mut p = PinLector::nuevo(a, 1.0);
            p.al_nacer(&v);
            assert!(!v.siempre_encima(), "nace sin fijar");
            p.alternar_fijado(&v);
            assert!(p.fijado && v.siempre_encima(), "el pin la fija");
            p.alternar_fijado(&v);
            assert!(!p.fijado && !v.siempre_encima(), "y la suelta");
            // Caso negativo: uno que si nace fijado queda encima.
            p.fijado = true;
            p.al_nacer(&v);
            assert!(v.siempre_encima());
        }
    }

    #[test]
    fn la_vista_deja_el_borde_para_estirar() {
        assert_eq!(
            area_de_la_vista(800, 600, 7),
            Rect {
                x: 7,
                y: 7,
                ancho: 786,
                alto: 586
            }
        );
        // En pantalla completa, sin borde.
        assert_eq!(
            area_de_la_vista(1920, 1080, 0),
            Rect {
                x: 0,
                y: 0,
                ancho: 1920,
                alto: 1080
            }
        );
        // Caso negativo: una ventana diminuta no da tamanos negativos.
        assert_eq!(
            area_de_la_vista(10, 10, 7),
            Rect {
                x: 7,
                y: 7,
                ancho: 1,
                alto: 1
            }
        );
        // La del pin: con su borde a su escala, y sin el a pantalla completa.
        let p = PinLector::nuevo(
            Rect {
                x: 0,
                y: 0,
                ancho: 800,
                alto: 600,
            },
            2.0,
        );
        assert_eq!(
            p.area_de_la_vista(),
            Rect {
                x: 14,
                y: 14,
                ancho: 772,
                alto: 572
            }
        );
        let monitor = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        let q = PinLector::nuevo(
            Rect {
                x: 0,
                y: 0,
                ancho: 800,
                alto: 600,
            },
            2.0,
        )
        .en_pantalla_completa(monitor);
        assert_eq!(
            q.area_de_la_vista(),
            Rect {
                x: 0,
                y: 0,
                ancho: 1920,
                alto: 1080
            }
        );
    }

    #[test]
    fn esc_cierra_o_sale_de_pantalla_completa_y_f11_alterna() {
        assert_eq!(tecla(VK_ESCAPE, false), Some(true));
        assert_eq!(tecla(VK_ESCAPE, true), Some(false));
        assert_eq!(tecla(VK_F11, false), Some(false));
        assert_eq!(tecla(VK_F11, true), Some(false));
        // Caso negativo: otra tecla no hace nada.
        assert_eq!(tecla(0x41, false), None);
    }

    #[test]
    fn la_barra_comun_rodea_los_botones_de_cada_visor() {
        let b = |id| BotonLector {
            id,
            icono: &material::SEARCH,
            encendido: false,
        };
        assert_eq!(
            disposicion("Plan", "md", &[b(0), b(4)], true),
            [
                "asa", "nombre", "propio:0", "propio:4", "oscuro", "pantalla", "fijar", "cerrar"
            ]
        );
        // Caso negativo: sin luna/sol (Office) no hay `oscuro`, y sin
        // nombre ni extension no hay pieza de nombre.
        assert_eq!(
            disposicion("", "", &[], false),
            ["asa", "pantalla", "fijar", "cerrar"]
        );
    }

    #[test]
    fn estirar_por_un_borde_no_pasa_del_minimo() {
        let a = Rect {
            x: 100,
            y: 100,
            ancho: 800,
            alto: 600,
        };
        // Por la derecha y abajo, crece.
        assert_eq!(
            estirada(a, 2 | 8, 50, 40, MINIMO),
            Rect {
                x: 100,
                y: 100,
                ancho: 850,
                alto: 640
            }
        );
        // Por la izquierda, se mueve el borde y no el otro.
        assert_eq!(
            estirada(a, 1, 30, 0, MINIMO),
            Rect {
                x: 130,
                y: 100,
                ancho: 770,
                alto: 600
            }
        );
        // Caso negativo: encoger de mas se queda en el minimo.
        let r = estirada(a, 2, -2000, 0, MINIMO);
        assert_eq!((r.x, r.ancho), (100, MINIMO.0));
    }

    #[test]
    fn el_minimo_propio_manda_al_estirar_y_al_nacer() {
        // El lienzo pide mas minimo que un lector: su barra tiene que caber.
        let a = Rect {
            x: 100,
            y: 100,
            ancho: 500,
            alto: 300,
        };
        let p = PinLector::nuevo(a, 1.0).con_minimo(640, 420);
        assert_eq!(
            (p.area.ancho, p.area.alto),
            (640, 420),
            "nace ya con el minimo"
        );
        assert_eq!(p.minimo, (640, 420));
        // Caso negativo: por la izquierda y arriba tampoco se pasa (los
        // bordes de enfrente no se mueven).
        let r = estirada(p.area, 1 | 4, 5000, 5000, p.minimo);
        assert_eq!(
            r,
            Rect {
                x: 100,
                y: 100,
                ancho: 640,
                alto: 420
            }
        );
        // Un lector sin minimo propio sigue con el de siempre.
        assert_eq!(PinLector::nuevo(a, 1.0).minimo, MINIMO);
    }

    #[test]
    fn nacer_a_pantalla_completa_recuerda_el_sitio_de_pin() {
        let pin = Rect {
            x: 300,
            y: 200,
            ancho: 900,
            alto: 700,
        };
        let monitor = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        let p = PinLector::nuevo(pin, 1.0).en_pantalla_completa(monitor);
        assert!(p.pantalla_completa);
        assert_eq!(p.area, monitor);
        assert_eq!(p.area_pin(), pin, "al volver, el sitio de antes");
        // Caso negativo: a pantalla completa no se estira por los bordes.
        assert_eq!(p.bordes_en(Punto { x: 1, y: 1 }), 0);
        // En pin, el sitio de pin es el de ahora.
        let q = PinLector::nuevo(pin, 1.0);
        assert_eq!(q.area_pin(), pin);
        assert_ne!(q.bordes_en(Punto { x: 301, y: 201 }), 0);
    }

    #[test]
    fn a_pantalla_completa_la_barra_asoma_solo_en_su_rincon() {
        let monitor = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        let (piezas, ancho) = piezas_de("Plano", "pixpin", &[], false, 1.0);
        // Sin luna/sol: asa, nombre y los tres de siempre.
        assert_eq!(piezas.len(), 5);
        assert!(!piezas.iter().any(|(_, _, p)| *p == Pieza::Oscuro));
        let r = rincon_de_la_barra(monitor, ancho, 48, 8);
        assert!(dentro(&r, Punto { x: 10, y: 10 }));
        // Caso negativo: el medio de la pantalla no la saca.
        assert!(!dentro(&r, Punto { x: 960, y: 540 }));
        // Ni se sale de una ventana mas chica que ella.
        let chica = Rect {
            x: 0,
            y: 0,
            ancho: 100,
            alto: 40,
        };
        let r = rincon_de_la_barra(chica, ancho, 48, 8);
        assert!(r.ancho <= 100 && r.alto <= 40);
        // Por defecto los lectores no la llevan a pantalla completa.
        let a = Rect {
            x: 0,
            y: 0,
            ancho: 800,
            alto: 600,
        };
        assert!(!PinLector::nuevo(a, 1.0).barra_en_pantalla_completa);
        assert!(
            PinLector::nuevo(a, 1.0)
                .con_barra_en_pantalla_completa()
                .barra_en_pantalla_completa
        );
    }
}

/// El nombre para la barra, sin el numero con que Android guarda los
/// adjuntos delante («1791239986208_Tesis» → «Tesis»).
pub fn nombre_limpio(nombre: &str) -> String {
    match nombre.split_once('_') {
        Some((n, resto))
            if n.len() >= 10 && n.bytes().all(|b| b.is_ascii_digit()) && !resto.is_empty() =>
        {
            resto.to_string()
        }
        _ => nombre.to_string(),
    }
}

#[cfg(test)]
mod pruebas_nombre {
    #[test]
    fn el_numero_de_guardado_no_se_ve() {
        assert_eq!(
            super::nombre_limpio("1791239986208_Tesis_Max_v4"),
            "Tesis_Max_v4"
        );
        // Caso negativo: un nombre con numeros cortos se queda igual.
        assert_eq!(super::nombre_limpio("2024_informe"), "2024_informe");
    }
}

#[cfg(test)]
mod muestras {
    use super::*;

    /// El recuadro del tipo y la luna/sol como salen en la barra, a PNG:
    /// `cargo test -p pixpin --bin pixpinmax muestra_de_la_barra_del_lector -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU; genera un PNG para mirarlo"]
    fn muestra_de_la_barra_del_lector() {
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let e = 1.5;
        let exts = ["pdf", "docx", "xlsx", "pptx", "epub", "html"];
        let (w, h) = ((exts.len() as f32 * 190.0 * e) as u32, (ALTO * e) as u32);
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), w, h).expect("superficie");
        motor
            .dibujar(&fuera.destino, |p| {
                p.limpiar(Color {
                    r: 0.5,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                });
                for (k, ext) in exts.iter().enumerate() {
                    let x = k as f32 * 190.0 * e;
                    p.rellenar_redondeado(
                        RectF {
                            x: x + 2.0,
                            y: 0.0,
                            ancho: 186.0 * e,
                            alto: h as f32,
                        },
                        10.0 * e,
                        FONDO,
                    );
                    let lado = BOTON * e;
                    let y0 = (h as f32 - lado) / 2.0;
                    cuadro_de_tipo(
                        p,
                        ext,
                        RectF {
                            x: x + 8.0 * e,
                            y: y0 + (lado - CUADRO * e) / 2.0,
                            ancho: CUADRO * e,
                            alto: CUADRO * e,
                        },
                        e,
                    );
                    let (_, th) = p.medir_texto("Ag", TAM_NOMBRE * e);
                    p.texto_linea(
                        "Tesis",
                        x + (8.0 + CUADRO + CUADRO_AIRE) * e,
                        y0 + (lado - th) / 2.0,
                        TAM_NOMBRE * e,
                        80.0 * e,
                        TINTA,
                    );
                    let icono = 20.0 * e;
                    let c = RectF {
                        x: x + 140.0 * e,
                        y: (h as f32 - icono) / 2.0,
                        ancho: icono,
                        alto: icono,
                    };
                    p.icono(
                        if k % 2 == 0 {
                            &material::DARK_MODE
                        } else {
                            &material::LIGHT_MODE
                        },
                        c,
                        TINTA,
                    );
                }
            })
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        let (ancho, alto, pixeles) = fuera.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
            ancho,
            alto,
            pixeles,
        })
        .expect("png");
        let salida = carpeta.join("barra-del-lector.png");
        std::fs::write(&salida, png).expect("guardar");
        println!("{}", salida.display());
    }
}
