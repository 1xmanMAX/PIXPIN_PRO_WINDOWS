//! El gestor de pines del ejecutable: la UNICA pieza que ve a la vez el
//! almacen (pixpin-store) y las ventanas (pixpin-pin), porque ambos son L2
//! y no pueden verse entre si. D21 en codigo: todo pasa por el almacen
//! primero; el Pin es la vista.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

// Las herramientas pineadas (C3, C4, L3) y el panel de propiedades de la
// anotacion. Modulos hijos: ven los campos del gestor sin hacerlos publicos.
mod herramienta;
mod panel;
mod pizarra;
mod sacar;

/// El catalogo de los rotulos que pintan las herramientas. El pintor de un
/// pin se llama desde su `WM_PAINT`, sin el catalogo a mano; como el panel
/// del lienzo (`panel_dibujo::fijar_idioma`), se fija una vez al arrancar.
static TEXTOS: OnceLock<pixpin_store::Catalogo> = OnceLock::new();

/// Fija el idioma de lo que pintan las herramientas pineadas. Si nadie lo
/// llama, el de Windows: mejor el del sistema que uno fijo.
pub fn fijar_idioma(idioma: pixpin_store::Idioma) {
    let _ = TEXTOS.set(pixpin_store::Catalogo::nuevo(idioma));
}

fn textos() -> &'static pixpin_store::Catalogo {
    TEXTOS.get_or_init(|| {
        pixpin_store::Catalogo::nuevo(pixpin_store::resolver_idioma(
            &pixpin_shell::entorno::locale_del_sistema(),
            pixpin_store::ajustes::PreferenciaIdioma::Sistema,
        ))
    })
}

/// Cuantas paginas se extraen como maximo de un solo golpe.
///
/// Veinte llenan la pantalla sin dejarla inservible. Un PDF de doscientas
/// paginas daria doscientos pines y doscientas entradas en el almacen, y
/// cerrarlos uno a uno seria peor que no haber extraido nada.
const TOPE_PAGINAS: u32 = 20;

/// Desde donde se numeran los pines en vivo. El almacen cuenta desde 1 y no
/// llegara aqui en la vida de nadie; asi un id basta para saber de que
/// clase de pin es un pedido.
const PRIMER_ID_EN_VIVO: u64 = 1 << 60;

/// Ancho al que se dibuja una pagina extraida, en pixeles.
///
/// Fijo y generoso: la pagina extraida es un documento para leer, no una
/// miniatura. Dibujarla al tamano que tenia el pin de origen daria una
/// imagen borrosa en cuanto se agrandara.
const ANCHO_PAGINA_EXTRAIDA: u32 = 1600;

/// Lo mas pequeno que puede ser un lienzo en blanco importado.
///
/// Un dibujo de dos trazos daria un pin de treinta pixeles, imposible de
/// agarrar. Y uno vacio daria uno de cero, que ni se ve.
const LIENZO_MINIMO: u32 = 400;
/// Margen alrededor del dibujo, para que no quede pegado al borde.
const LIENZO_MARGEN: f32 = 24.0;

/// Un lienzo blanco del tamano justo para lo que hay dibujado, y cuanto hay
/// que mover el dibujo para que caiga dentro.
///
/// Se usa cuando la hoja del movil no venia sobre una pagina de PDF. El
/// Android dibuja sobre el plano; sin fondo, la anotacion quedaria en el aire
/// y no se veria donde empieza ni acaba la hoja.
///
/// # Por que se recorta por arriba a la izquierda
///
/// En un lienzo infinito, el (0,0) es un sitio cualquiera por el que se
/// empezo a dibujar, y lo dibujado puede estar a mil pixeles de el. Medir
/// desde el origen fabrica un lienzo enorme casi todo vacio: el dibujo real
/// del usuario empieza en (165, 228) y acababa dando un pin de 1202 x 2172,
/// mas alto que su pantalla. Al encogerlo para que quepa, el dibujo se veia
/// diminuto y arrinconado — lo que se noto como que «no aparece completo».
///
/// Recortar exige mover el dibujo la misma cantidad, y por eso se devuelve el
/// desplazamiento en vez de aplicarlo aqui: mover elementos no es cosa de una
/// funcion que fabrica un fondo. **Solo vale sin fondo.** Sobre una pagina de
/// PDF las coordenadas tienen que seguir cuadrando con la pagina, y ahi el
/// desplazamiento seria justo el error que se esta arreglando.
///
/// `papel` es el color del lienzo (`excalidraw::fondo`), opaco: el pin es la
/// hoja, y la hoja es de ese color.
fn lienzo_en_blanco(
    elementos: &[pixpin_motor2d::Elemento],
    papel: pixpin_motor2d::ColorRgba,
) -> (ImagenRgba, f32, f32) {
    let mut izquierda = f32::MAX;
    let mut arriba = f32::MAX;
    let mut derecha = f32::MIN;
    let mut abajo = f32::MIN;
    for e in elementos {
        let (x1, y1, x2, y2) = e.caja();
        izquierda = izquierda.min(x1);
        arriba = arriba.min(y1);
        derecha = derecha.max(x2);
        abajo = abajo.max(y2);
    }
    // Sin nada dibujado no hay esquina de la que partir: se deja el origen y
    // sale el lienzo minimo, que es lo que se quiere de una hoja en blanco.
    if izquierda > derecha {
        (izquierda, arriba, derecha, abajo) = (0.0, 0.0, 0.0, 0.0);
    }
    let (dx, dy) = (LIENZO_MARGEN - izquierda, LIENZO_MARGEN - arriba);
    let ancho = ((derecha - izquierda + 2.0 * LIENZO_MARGEN) as u32).max(LIENZO_MINIMO);
    let alto = ((abajo - arriba + 2.0 * LIENZO_MARGEN) as u32).max(LIENZO_MINIMO);
    let canal = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let pixel = [canal(papel.r), canal(papel.g), canal(papel.b), 255];
    (
        ImagenRgba {
            ancho,
            alto,
            pixeles: pixel.repeat(ancho as usize * alto as usize),
        },
        dx,
        dy,
    )
}

use anyhow::{Context, Result};
use pixpin_codec::{ImagenRgba, cargar, codificar_png};
use pixpin_geom::{DisposicionMonitores, Monitor, Punto, Rect, recolocar_en_area};
use pixpin_motor2d::Escena;
use pixpin_pin::{
    CambioPin, Contenido, CursorAnotacion, Paleta, Pin, Presentacion, TextosPin, icono_de,
    miniatura_de, presentacion_de, tamano_humano, tamano_natural,
};
use pixpin_render::MotorRender;
use pixpin_store::{Almacen, ColorGrupo, PinGuardado, TipoEntrada};
use pixpin_ui::{BotonCaja, CajaHerramientas, EventoAnotador, Herramienta, TeclaAnotador};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

/// La paleta de grupos en RGB (D35). Vive aqui porque es la traduccion
/// entre dos crates de la MISMA capa que no pueden verse: `pixpin-store`
/// sabe que color es cada grupo, `pixpin-pin` solo entiende numeros.
fn rgb_de(color: ColorGrupo) -> (f32, f32, f32) {
    match color {
        ColorGrupo::Rojo => (0.86, 0.20, 0.18),
        ColorGrupo::Naranja => (0.95, 0.45, 0.10),
        ColorGrupo::Ambar => (0.95, 0.68, 0.10),
        ColorGrupo::Verde => (0.20, 0.66, 0.33),
        ColorGrupo::Cian => (0.10, 0.66, 0.68),
        ColorGrupo::Azul => (0.16, 0.44, 0.86),
        ColorGrupo::Violeta => (0.48, 0.28, 0.78),
        ColorGrupo::Rosa => (0.87, 0.28, 0.60),
    }
}

/// La ficha de un archivo: icono real, nombre y tamano, o el aviso de que
/// la ruta ya no lleva a ninguna parte (D28).
/// La barra de un pin que se anota: la del lienzo (`dibujo::permitidas`,
/// anfitrion `Pin`), arriba en el centro del area de trabajo de su monitor.
/// En columna junto al pin, con todas las herramientas, no cabria en una
/// pantalla de 1080: la del anotador viejo tenia once botones y esta treinta.
fn caja_del_pin(area_trabajo: Rect, escala_por_cien: u32) -> CajaHerramientas {
    CajaHerramientas::barra_superior(
        area_trabajo,
        escala_por_cien,
        crate::dibujo::permitidas::botones(crate::dibujo::permitidas::Anfitrion::Pin),
    )
}

/// Donde va la ventana de la paleta del pin: la barra y, si hay un grupo
/// abierto, tambien sus hermanas.
fn rect_de_paleta(caja: &CajaHerramientas) -> Rect {
    caja.menu()
        .map_or(caja.marco, |m| caja.marco.union(m.marco))
}

/// El cursor de cada herramienta dentro del pin: cruz para dibujar, barra
/// para escribir, flecha para la mano.
fn cursor_pin_de(h: Herramienta) -> CursorAnotacion {
    match h {
        Herramienta::Mano => CursorAnotacion::Flecha,
        Herramienta::Texto => CursorAnotacion::Texto,
        _ => CursorAnotacion::Cruz,
    }
}

/// Donde esta guardado un pin ahora mismo, si el almacen lo da por abierto.
/// Se consulta justo antes de cerrarlo: al marcarlo cerrado esa posicion se
/// pierde, y sin ella no se puede devolver a su sitio.
fn posicion_guardada(almacen: &Rc<RefCell<Almacen>>, id: u64) -> Option<PinGuardado> {
    almacen
        .borrow()
        .entradas()
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| e.pin)
}

fn ficha_de(ruta: &Path, texto_no_encontrado: &str) -> Contenido {
    let nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| ruta.to_string_lossy().into_owned());
    let metadatos = std::fs::metadata(ruta).ok();
    let existe = metadatos.is_some();
    let detalle = match &metadatos {
        Some(m) if m.is_dir() => "—".to_string(),
        Some(m) => tamano_humano(m.len()),
        None => texto_no_encontrado.to_string(),
    };
    Contenido::Archivo {
        nombre,
        detalle,
        icono: icono_de(ruta),
        existe,
    }
}

pub struct Pines {
    almacen: Rc<RefCell<Almacen>>,
    d3d: ID3D11Device,
    motor: Rc<MotorRender>,
    vivos: HashMap<u64, Pin>,
    /// Los pines en vivo, con su zona de pantalla. Van aparte de `vivos`
    /// porque no tienen entrada en el almacen: no se restauran al arrancar
    /// ni se agrupan, y la mitad de los pedidos de un pin no significan nada
    /// para ellos. Sus ids salen de `siguiente_en_vivo`, lejos de los del
    /// almacen, para que la cola de pedidos no los confunda.
    en_vivo: HashMap<u64, (Pin, Rc<RefCell<pixpin_capture::RecorteVivo>>)>,
    siguiente_en_vivo: u64,
    /// Si Ctrl+2 escondio los pines en vivo. A ellos no se les cierra para
    /// ocultarlos, como a los demas: sin entrada en el almacen no habria
    /// forma de traerlos de vuelta.
    en_vivo_ocultos: bool,
    /// Ids cerrados desde los callbacks; purgar() los drena en el bucle.
    cerrados: Rc<RefCell<Vec<u64>>>,
    /// Pila de los que se han cerrado, con la posicion que tenian: cerrar
    /// marca el pin como cerrado en el almacen y con ello se pierde donde
    /// estaba, asi que hay que guardarlo ANTES para poder devolverlo a su
    /// sitio. El ultimo cerrado es el primero en volver.
    reabrir: Rc<RefCell<Vec<(u64, PinGuardado)>>>,
    /// Peticiones del menu y del teclado que el pin no puede resolver.
    /// Se atienden en `purgar`, ya fuera del callback: dentro, el almacen
    /// esta prestado y volver a pedirlo entraria en panico.
    pedidos: Rc<RefCell<Vec<(u64, CambioPin)>>>,
    /// Se lee una vez al arrancar (D33): un pin nuevo nace con el tema del
    /// momento, y los ya abiertos no cambian de color a media sesion.
    tema_claro: bool,
    /// Si el equipo sabe reconocer texto. Se pregunta UNA vez al montar el
    /// gestor y se reparte a cada pin: `disponible()` construye un motor
    /// entero para responder, y llamarlo por cada pin y cada menu se
    /// notaria.
    con_ocr: bool,
    /// Lo ultimo que dio extraer todas las paginas, para que el bucle lo
    /// avise. El gestor no tiene bandeja con la que hablar.
    paginas_extraidas: Option<(u32, u32)>,
    /// Ya traducido: `pixpin-pin` no conoce el catalogo de idiomas.
    texto_no_encontrado: String,
    /// Etiquetas del menu del pin, tambien ya traducidas.
    textos: TextosPin,
    /// El aviso antes de borrar del almacen, ya traducido.
    texto_confirmar_eliminar: String,
    /// La coletilla del video que no se pudo reproducir, ya traducida: el
    /// usuario vio un video parado y no supo por que.
    texto_sin_codec: String,
    /// La ventana del bucle principal, para darle un toque cuando un pin
    /// deja algo pendiente que solo el gestor puede atender.
    hwnd_app: windows::Win32::Foundation::HWND,
    /// La anotacion en curso, si hay un pin en modo edicion. Solo uno a la
    /// vez: anotar dos pines a la vez no significa nada y complicaria el
    /// foco del teclado sin ganar nada.
    anotacion: Option<Anotacion>,
    /// El lienzo que un pin pidio abrir (D132), a la espera de que el bucle
    /// principal lo recoja con `tomar_lienzo`.
    lienzo_pedido: Option<PedidoLienzo>,
    /// Cada cuanto pregunta un pin de video por fotogramas (D67): lo decide
    /// el nivel de rendimiento al arrancar. `None` si el dispositivo no
    /// soporta video (D66): entonces los videos se ensenan como documento.
    ritmo_video: Option<u32>,
    /// Las herramientas pineadas (mini-apps y tablas), con su documento y lo
    /// de su interfaz. El documento tambien esta en el almacen; esto es lo
    /// que se pinta y se toca.
    herramientas: HashMap<u64, herramienta::Herramienta>,
    /// Las pizarras abiertas, con su color y su pauta.
    pizarras: HashMap<u64, (u8, u8)>,
    /// Las palabras magicas (las de fabrica del movil).
    palabras: std::collections::BTreeMap<String, pixpin_pin::magia::MiniApp>,
}

/// Lo que el gestor deja preparado para abrir un pin en el lienzo. El
/// gestor no abre el editor: el editor tiene su propio bucle y su propio
/// dispositivo, y lo abre `main.rs`, que se lo devuelve con `terminar_lienzo`.
pub struct PedidoLienzo {
    pub id: u64,
    /// El `.pixpin2d` del pin.
    pub ruta: PathBuf,
    pub habia_fichero: bool,
    pub escena: Escena,
    /// La imagen del pin, o el recuadro gris si no se pudo leer.
    pub fondo: ImagenRgba,
}

/// Un pin en modo anotacion: su dibujo y la mano que dibuja.
///
/// Con las herramientas del lienzo (2026-09-24): el `Gesto` del motor y la
/// `Mano` comun (`dibujo::mano`), anfitrion `Pin`. Antes era la maquina vieja
/// `Anotador` de `pixpin-ui`, con once herramientas y un elemento en curso
/// aparte; ahora lo que se dibuja va a la escena desde el primer punto, como
/// en el lienzo, y se deshace igual.
struct Anotacion {
    id: u64,
    escena: Escena,
    gesto: pixpin_motor2d::gesto::Gesto,
    mano: crate::dibujo::mano::Mano,
    /// La barra de herramientas del lienzo y la ventana que la muestra
    /// (D58). La paleta muere con la anotacion: su `Drop` la destruye.
    caja: CajaHerramientas,
    paleta: Paleta,
    /// El panel de propiedades (color, grosor...), en otra ventanita junto
    /// al pin. Muere con la anotacion, como la paleta.
    panel: Paleta,
    /// El area de trabajo del monitor del pin: el panel no se sale de ella.
    trabajo: Rect,
    escala_por_cien: u32,
    /// Pixeles de la ventana del pin por pixel del contenido original: el
    /// grosor de los tiradores y el radio de agarre se miden en pantalla.
    zoom: f32,
    /// Donde estaba el raton la ultima vez, en coordenadas del contenido.
    ultimo_cursor: Punto,
    /// El mismo punto en pixeles del dibujo, para ensenar el pincel ahi.
    cursor_documento: pixpin_motor2d::Punto2,
    /// Tras girar la rueda se ve un circulo del tamano del pincel en el
    /// cursor, hasta que se empieza a dibujar: sin el, el grosor nuevo no
    /// se veia hasta el siguiente trazo y parecia que la rueda no hacia nada.
    ver_grosor: bool,
    /// El grafito de la escena ya cocido: se repinta con cada movimiento del
    /// raton, y lo quieto no se vuelve a cocer.
    grafitos: RefCell<pixpin_motor2d::tinta::grafito::MapasSueltos>,
}

/// **Lo que se pinta de una escena en un pin**, con el grafito aparte y ya
/// cocido (`tinta::grafito::ordenes_con_grafito`). A una casilla por unidad,
/// que el pin se ve al tamano del original; con tope de memoria, pasado el
/// cual lo que queda sale liso.
fn anotaciones_de(
    escena: &Escena,
    guardados: &mut pixpin_motor2d::tinta::grafito::MapasSueltos,
) -> (
    Vec<pixpin_motor2d::Orden>,
    Vec<pixpin_motor2d::tinta::grafito::GrafitoSuelto>,
) {
    const TOPE_DEL_GRAFITO_EN_UN_PIN: usize = 64 * 1024 * 1024;
    pixpin_motor2d::tinta::grafito::ordenes_con_grafito_guardando(
        escena,
        1.0,
        TOPE_DEL_GRAFITO_EN_UN_PIN,
        guardados,
    )
}

/// El grosor de la tinta que sigue a una muesca de rueda sobre el pin: los
/// tres del lienzo (teclas 1, 2 y 3), hacia arriba o hacia abajo.
fn grosor_con_rueda(ahora: f32, delta: i32) -> f32 {
    use pixpin_motor2d::tinta::{GROSOR_FINO, GROSOR_GRUESO, GROSOR_MEDIO};
    let escalones = [GROSOR_FINO, GROSOR_MEDIO, GROSOR_GRUESO];
    let i = escalones
        .iter()
        .position(|g| (*g - ahora).abs() < 1e-3)
        .unwrap_or(1);
    let j = if delta > 0 {
        (i + 1).min(escalones.len() - 1)
    } else if delta < 0 {
        i.saturating_sub(1)
    } else {
        i
    };
    escalones[j]
}

impl Anotacion {
    /// Las ordenes de dibujo de la escena (el trazo en curso ya esta en
    /// ella), lo que va encima (marco, tiradores, lazo...) y el grafito de la
    /// escena, aparte.
    fn ordenes(
        &self,
    ) -> (
        Vec<pixpin_motor2d::Orden>,
        Vec<pixpin_motor2d::tinta::grafito::GrafitoSuelto>,
    ) {
        let (mut v, grafitos) = anotaciones_de(&self.escena, &mut self.grafitos.borrow_mut());
        if self.ver_grosor {
            // El radio del trazo a ese grosor: el lapiz de Excalidraw mide
            // unas 4 veces su `strokeWidth` de ancho.
            let radio = (self.gesto.grosor_tinta * 2.0).max(1.0);
            let c = self.cursor_documento;
            let puntos: Vec<pixpin_motor2d::Punto2> = (0..=32)
                .map(|k| {
                    let t = std::f32::consts::TAU * k as f32 / 32.0;
                    pixpin_motor2d::Punto2::nuevo(c.x + radio * t.cos(), c.y + radio * t.sin())
                })
                .collect();
            v.push(pixpin_motor2d::Orden::Polilinea {
                puntos,
                color: pixpin_motor2d::ColorRgba {
                    r: 0.2,
                    g: 0.2,
                    b: 0.2,
                    a: 0.8,
                },
                grosor: 1.5,
                estilo: pixpin_motor2d::EstiloTrazo::Solido,
            });
        }
        // Lo de encima va el ultimo, como en el lienzo: se ve que hay algo
        // elegido aunque quede debajo de otro trazo.
        v.extend(crate::dibujo::pintar::ordenes_encima(
            &self.gesto,
            &self.escena,
            self.zoom,
        ));
        (v, grafitos)
    }
}

impl Pines {
    #[allow(clippy::too_many_arguments)] // lo que el gestor recibe una vez y no cambia
    pub fn nuevos(
        raiz: &Path,
        d3d: ID3D11Device,
        motor: Rc<MotorRender>,
        texto_no_encontrado: String,
        textos: TextosPin,
        texto_confirmar_eliminar: String,
        texto_sin_codec: String,
        hwnd_app: windows::Win32::Foundation::HWND,
        ritmo_video: Option<u32>,
    ) -> Result<Pines> {
        let almacen = Almacen::abrir(raiz).context("no se pudo abrir el almacen")?;
        Ok(Pines {
            almacen: Rc::new(RefCell::new(almacen)),
            d3d,
            motor,
            vivos: HashMap::new(),
            en_vivo: HashMap::new(),
            siguiente_en_vivo: PRIMER_ID_EN_VIVO,
            en_vivo_ocultos: false,
            cerrados: Rc::new(RefCell::new(Vec::new())),
            reabrir: Rc::new(RefCell::new(Vec::new())),
            pedidos: Rc::new(RefCell::new(Vec::new())),
            tema_claro: pixpin_shell::entorno::tema_claro(),
            con_ocr: pixpin_ocr::disponible(),
            paginas_extraidas: None,
            texto_no_encontrado,
            textos,
            texto_confirmar_eliminar,
            texto_sin_codec,
            hwnd_app,
            anotacion: None,
            lienzo_pedido: None,
            ritmo_video,
            herramientas: HashMap::new(),
            pizarras: HashMap::new(),
            palabras: pixpin_pin::magia::por_defecto(),
        })
    }

    fn guardado_desde(region: Rect, escala: u32, zoom_por_cien: u32) -> PinGuardado {
        PinGuardado {
            x: region.x,
            y: region.y,
            ancho: region.ancho,
            alto: region.alto,
            escala_por_cien: escala,
            zoom_por_cien,
            // Sin girar: es lo que vale para un pin recien creado. Los que
            // ya estan en pantalla conservan el suyo con `con_giro_de`.
            giro: 0,
            volteo_h: false,
            volteo_v: false,
            pasante: false,
            gris: false,
            invertido: false,
            brillo: 0,
        }
    }

    /// Lo que el pin dice de si mismo, tal cual, para el almacen.
    fn guardado_de(c: pixpin_pin::Colocacion, escala: u32) -> PinGuardado {
        PinGuardado {
            x: c.rect.x,
            y: c.rect.y,
            ancho: c.rect.ancho,
            alto: c.rect.alto,
            escala_por_cien: escala,
            zoom_por_cien: c.zoom_por_cien,
            giro: c.giro,
            volteo_h: c.volteo_h,
            volteo_v: c.volteo_v,
            pasante: c.pasante,
            gris: c.gris,
            invertido: c.invertido,
            brillo: c.brillo,
        }
    }

    fn crear_ventana(
        &mut self,
        id: u64,
        contenido: Contenido,
        region: Rect,
        escala: u32,
    ) -> Result<()> {
        let almacen = Rc::clone(&self.almacen);
        let cerrados = Rc::clone(&self.cerrados);
        let reabrir = Rc::clone(&self.reabrir);
        let pedidos = Rc::clone(&self.pedidos);
        let hwnd_app = self.hwnd_app;
        let pin = Pin::nuevo(
            &self.d3d,
            Rc::clone(&self.motor),
            contenido,
            region,
            escala,
            self.tema_claro,
            self.ritmo_video.unwrap_or(16),
            Box::new(move |cambio| {
                let resultado = match cambio {
                    CambioPin::Movido(c) | CambioPin::Redimensionado(c) => almacen
                        .borrow_mut()
                        .actualizar_pin(id, Some(Pines::guardado_de(c, escala))),
                    CambioPin::Cerrado => {
                        cerrados.borrow_mut().push(id);
                        // Apuntar DONDE estaba antes de marcarlo cerrado:
                        // marcarlo borra esa posicion del almacen, y sin
                        // ella «restaurar el ultimo cerrado» no sabria
                        // adonde devolverlo.
                        if let Some(g) = posicion_guardada(&almacen, id) {
                            reabrir.borrow_mut().push((id, g));
                        }
                        pixpin_shell::despertar(hwnd_app);
                        almacen.borrow_mut().actualizar_pin(id, None)
                    }
                    // El pin no sabe hacer nada de esto: no conoce ni el
                    // portapapeles ni el almacen ni su propia entrada. Se
                    // apuntan y el bucle los atiende, ya fuera del prestamo.
                    otro => {
                        pedidos.borrow_mut().push((id, otro));
                        // Y se le da un toque al bucle: sin esto la peticion
                        // se quedaba en la cola hasta que el usuario pulsara
                        // un atajo, porque el WndProc del pin no produce
                        // ningun evento de la ventana principal. Lo encontro
                        // la prueba de extremo a extremo del menu.
                        pixpin_shell::despertar(hwnd_app);
                        Ok(())
                    }
                };
                if let Err(e) = resultado {
                    // Perder una posicion no puede tumbar el pin: se
                    // registra y se sigue (el contenido ya esta a salvo).
                    tracing::warn!(?e, id, "no se pudo persistir el cambio del pin");
                }
            }),
        )
        .context("no se pudo crear la ventana del pin")?;
        pin.poner_textos(self.textos.clone());
        // Se pregunta una sola vez y se reparte: montar el motor de
        // reconocimiento cuesta, y hacerlo con el menu a medio abrir se
        // notaria.
        pin.poner_ocr(self.con_ocr);
        // La ruta del fichero de origen, si lo hay. Es lo que se arrastra
        // a otra aplicacion en una ficha o un documento: el contenido solo
        // guarda lo que se PINTA (el icono, la vista previa), y sin esto
        // arrastrar una ficha no haria nada.
        pin.poner_ruta(
            self.almacen
                .borrow()
                .entradas()
                .iter()
                .find(|e| e.id == id)
                .and_then(|e| e.ruta.clone()),
        );
        // Un pin restaurado nace ya con el color de su grupo: pintarlo negro
        // y retenirlo despues daria un parpadeo al arrancar.
        if let Some(g) = self.almacen.borrow().grupo_de(id) {
            pin.poner_color(Some(rgb_de(g.color)));
        }
        // Y con la vista que tuviera: girado, volteado, y dejando pasar
        // el clic si asi se quedo. Sin esto, los tres campos se guardaban
        // en el indice y no los leia nadie: un pin girado volvia derecho
        // tras reiniciar, que es peor que no haberlo guardado, porque el
        // fichero promete algo que no se cumple.
        if let Some(g) = posicion_guardada(&self.almacen, id) {
            if g.giro != 0 || g.volteo_h || g.volteo_v {
                pin.poner_giro(g.giro, g.volteo_h, g.volteo_v);
            }
            if g.pasante {
                pin.poner_pasante(true);
            }
            if g.gris || g.invertido || g.brillo != 0 {
                pin.poner_filtros(g.gris, g.invertido, g.brillo);
            }
        }
        self.vivos.insert(id, pin);
        // Y con lo que tuviera dibujado encima. Sin esto el pin volvia
        // limpio tras reiniciar y la anotacion parecia perdida, aunque su
        // fichero siguiera ahi: lo encontro la prueba de extremo a extremo.
        self.recargar_anotaciones(id);
        // Y si es una herramienta o una pizarra, con lo suyo (`sacar`).
        self.vestir(id);
        Ok(())
    }

    /// Vuelve a pintar en el pin lo que haya en su fichero de anotacion.
    /// Un fichero ilegible se registra y no impide que el pin exista: el
    /// contenido original es lo importante.
    fn recargar_anotaciones(&self, id: u64) {
        let Some(ruta) = self.ruta_anotacion(id) else {
            return;
        };
        match pixpin_motor2d::cargar(&ruta) {
            Ok(escena) if escena.cuantos_visibles() > 0 => {
                if let Some(pin) = self.vivos.get(&id) {
                    let (ordenes, grafitos) = anotaciones_de(&escena, &mut Default::default());
                    pin.poner_anotaciones_con_grafito(
                        ordenes,
                        grafitos,
                        pixpin_motor2d::mosaico::cajas_tapadas(&escena.elementos),
                    );
                }
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(?e, id, "anotacion ilegible; el pin sale sin ella"),
        }
    }

    /// Un pin que ensena `zona` en directo. Nace al lado de la zona, 1:1, y
    /// no toca el almacen: lo que se ve no se guarda hasta congelarlo.
    ///
    /// Primero la ventana y despues la captura, porque la captura avisa a
    /// ESA ventana de cada fotograma. Si la captura no se puede abrir, el
    /// pin se destruye con el error: un pin en vivo sin zona seria un
    /// recuadro vacio sin explicacion.
    pub fn pinear_en_vivo(
        &mut self,
        dispositivo: &pixpin_capture::Dispositivo,
        zona: Rect,
        tope: std::time::Duration,
    ) -> Result<u64> {
        let disposicion = pixpin_capture::enumerar_monitores().context("sin monitores")?;
        let encuadre = pixpin_capture::encuadrar(zona, disposicion.monitores())
            .context("la zona no cae en ningun monitor")?;
        let monitor = disposicion
            .monitores()
            .iter()
            .find(|m| m.id == encuadre.id_monitor)
            .context("el monitor de la zona desaparecio")?;
        let escala = monitor.escala_por_cien;
        let sitio = crate::pin_vivo::sitio_junto_a(encuadre.zona, monitor.area_trabajo);

        let id = self.siguiente_en_vivo;
        self.siguiente_en_vivo += 1;
        let cerrados = Rc::clone(&self.cerrados);
        let pedidos = Rc::clone(&self.pedidos);
        let hwnd_app = self.hwnd_app;
        let pin = Pin::nuevo(
            &self.d3d,
            Rc::clone(&self.motor),
            Contenido::Vivo {
                ancho: encuadre.zona.ancho,
                alto: encuadre.zona.alto,
            },
            sitio,
            escala,
            self.tema_claro,
            self.ritmo_video.unwrap_or(16),
            Box::new(move |cambio| match cambio {
                // Moverlo o agrandarlo no se guarda: no hay entrada.
                CambioPin::Movido(_) | CambioPin::Redimensionado(_) => {}
                CambioPin::Cerrado => {
                    cerrados.borrow_mut().push(id);
                    pixpin_shell::despertar(hwnd_app);
                }
                otro => {
                    pedidos.borrow_mut().push((id, otro));
                    pixpin_shell::despertar(hwnd_app);
                }
            }),
        )
        .context("no se pudo crear la ventana del pin en vivo")?;
        pin.poner_textos(self.textos.clone());

        let recorte = pixpin_capture::RecorteVivo::nuevo(
            dispositivo,
            encuadre,
            tope,
            Some((pin.hwnd().0 as isize, pixpin_pin::MSG_FOTOGRAMA_VIVO)),
        )
        .context("no se pudo abrir la captura en vivo de la zona")?;
        let recorte = Rc::new(RefCell::new(recorte));
        pin.poner_fuente_viva(Box::new(crate::pin_vivo::FuenteCompartida(Rc::clone(
            &recorte,
        ))));
        tracing::info!(id, ?zona, ?encuadre, ?sitio, "pin en vivo creado");
        self.en_vivo.insert(id, (pin, recorte));
        Ok(id)
    }

    /// Lo que un pin en vivo pidio. Solo tres cosas tienen sentido sin
    /// entrada en el almacen; el resto se descarta en silencio, porque el
    /// menu del pin en vivo ni siquiera las ofrece.
    fn atender_en_vivo(&mut self, id: u64, cambio: CambioPin) -> Result<()> {
        match cambio {
            CambioPin::CopiarPedido => {
                let (_, recorte) = self.en_vivo.get(&id).context("el pin en vivo ya no esta")?;
                let img = recorte
                    .borrow()
                    .imagen()
                    .context("no se pudo leer el fotograma en vivo")?;
                pixpin_codec::copiar_imagen(&img).context("no se pudo copiar la imagen")?;
                Ok(())
            }
            // Congelar = lo que se ve pasa a ser un pin de imagen de los de
            // siempre, en el mismo sitio y tamano, y el pin en vivo se va.
            CambioPin::CongelarPedido => {
                let (pin, recorte) = self
                    .en_vivo
                    .remove(&id)
                    .context("el pin en vivo ya no esta")?;
                let img = recorte
                    .borrow()
                    .imagen()
                    .context("no se pudo leer el fotograma en vivo")?;
                let rect = pin.rect_contenido();
                let escala = pin.escala_por_cien();
                // Soltar antes el en vivo: cierra su captura y su ventana, y
                // el pin congelado no nace tapado por el.
                drop(pin);
                drop(recorte);
                let nuevo = self.pinear(&img, rect, escala)?;
                tracing::info!(id, nuevo, "pin en vivo congelado");
                Ok(())
            }
            CambioPin::EliminarPedido => {
                self.en_vivo.remove(&id);
                Ok(())
            }
            // Manejar a distancia: el pin dice QUE pixel de la zona se pulso
            // y aqui se sabe DONDE esta la zona en la pantalla.
            CambioPin::ClicRemoto { x, y } => {
                if let Some(p) = self.punto_de_la_zona(id, x, y)? {
                    pixpin_shell::entrada::clic_en(p);
                    tracing::info!(id, ?p, "clic a distancia");
                }
                Ok(())
            }
            CambioPin::ArrastreRemoto { x0, y0, x1, y1 } => {
                // Basta con que el punto de PULSAR este libre: es donde el
                // arrastre agarra. El de soltar puede pasar por debajo del pin.
                if let Some(desde) = self.punto_de_la_zona(id, x0, y0)? {
                    let zona = self
                        .en_vivo
                        .get(&id)
                        .context("el pin en vivo ya no esta")?
                        .1
                        .borrow()
                        .encuadre()
                        .zona;
                    let hasta = pixpin_geom::Punto {
                        x: zona.x + x1,
                        y: zona.y + y1,
                    };
                    pixpin_shell::entrada::arrastrar_a_distancia(desde, hasta);
                    tracing::info!(id, ?desde, ?hasta, "arrastre a distancia");
                }
                Ok(())
            }
            CambioPin::RuedaRemota { x, y, delta } => {
                if let Some(p) = self.punto_de_la_zona(id, x, y)? {
                    pixpin_shell::entrada::rueda_a_distancia(p, delta);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// El punto de PANTALLA del pixel `(x, y)` de la zona del pin en vivo
    /// `id`. `None` si ahi hay ahora una ventana nuestra: con el pin encima
    /// de su propia zona, el clic caeria en el pin, que lo reenviaria otra
    /// vez. Mejor no hacer nada que entrar en ese bucle.
    fn punto_de_la_zona(&self, id: u64, x: i32, y: i32) -> Result<Option<pixpin_geom::Punto>> {
        let (_, recorte) = self.en_vivo.get(&id).context("el pin en vivo ya no esta")?;
        let zona = recorte.borrow().encuadre().zona;
        let p = pixpin_geom::Punto {
            x: zona.x + x,
            y: zona.y + y,
        };
        if pixpin_shell::entrada::punto_es_nuestro(p) {
            tracing::info!(id, ?p, "clic a distancia descartado: el punto lo tapa PixPin");
            return Ok(None);
        }
        Ok(Some(p))
    }

    /// D26: el recorte queda flotando 1:1 exactamente donde estaba.
    /// Devuelve el id del pin nuevo: quien captura y anota lo necesita para
    /// entrar a anotar en ese mismo, no en «el ultimo que haya».
    pub fn pinear(&mut self, imagen: &ImagenRgba, region: Rect, escala: u32) -> Result<u64> {
        let png = codificar_png(imagen).context("no se pudo codificar el pin")?;
        let id = self
            .almacen
            .borrow_mut()
            .guardar_imagen(
                &png,
                "recorte",
                Some(Pines::guardado_desde(region, escala, 100)),
            )
            .context("no se pudo guardar en el almacen")?;
        self.crear_ventana(id, Contenido::Imagen(imagen.clone()), region, escala)?;
        Ok(id)
    }

    /// Entra en modo anotacion en un pin concreto. Publico para «capturar y
    /// anotar», que encadena las dos cosas sin que el usuario haga nada.
    pub fn anotar_pin(&mut self, id: u64) -> Result<()> {
        self.entrar_a_anotar(id)
    }

    /// Una nota del portapapeles: nace centrada en el monitor pedido (D32).
    pub fn pinear_nota(&mut self, texto: &str, monitor: &Monitor) -> Result<()> {
        let contenido = Contenido::Nota {
            texto: texto.to_string(),
        };
        let region = self.region_centrada(&contenido, monitor);
        let id = self
            .almacen
            .borrow_mut()
            .guardar_nota(
                texto,
                "portapapeles",
                Some(Pines::guardado_desde(region, monitor.escala_por_cien, 100)),
            )
            .context("no se pudo guardar la nota")?;
        self.crear_ventana(id, contenido, region, monitor.escala_por_cien)
    }

    /// Como se ensena un archivo por referencia (D62/D65): video si la
    /// extension lo dice y el dispositivo puede reproducirlo; si no,
    /// documento cuando la Shell tiene miniatura, y ficha en ultimo caso.
    /// `tamano_guardado`: al restaurar, el rect del pin ya esta en el indice y
    /// no hace falta pedir la miniatura del video para saber su proporcion
    /// (ahorra cientos de ms por video al arrancar).
    fn contenido_de_archivo(&self, ruta: &Path, tamano_guardado: bool) -> Contenido {
        let nombre = ruta
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| ruta.to_string_lossy().into_owned());
        let es_video = presentacion_de(ruta) == Presentacion::Video
            && self.ritmo_video.is_some()
            && ruta.is_file();
        if es_video {
            // La proporcion de la miniatura decide el tamano al nacer; el
            // tamano nativo llega con los metadatos y solo afecta al 100 %.
            // Sin miniatura, el provisional (D71).
            let (ancho, alto) = if tamano_guardado {
                None
            } else {
                miniatura_de(ruta, 512)
            }
            .map(|m| {
                let base = 960.0;
                let f = base / m.ancho.max(1) as f32;
                (
                    (m.ancho as f32 * f).round() as u32,
                    (m.alto as f32 * f).round() as u32,
                )
            })
            .unwrap_or((0, 0));
            return Contenido::Video {
                nombre,
                ruta: ruta.to_path_buf(),
                ancho,
                alto,
            };
        }
        match miniatura_de(ruta, 1024) {
            Some(vista) => Contenido::Documento { nombre, vista },
            None => ficha_de(ruta, &self.texto_no_encontrado),
        }
    }

    /// Media Foundation no pudo con el video (D72): el pin se vuelve a
    /// crear como documento o ficha, en el mismo sitio y con el mismo id.
    fn degradar_video(&mut self, id: u64) -> Result<()> {
        let Some(pin) = self.vivos.remove(&id) else {
            return Ok(());
        };
        let region = pin.rect_contenido();
        let escala = pin.escala_por_cien();
        drop(pin);
        let ruta = {
            let a = self.almacen.borrow();
            a.entradas()
                .iter()
                .find(|e| e.id == id)
                .and_then(|e| e.ruta.clone())
                .context("el video no referencia ningun fichero")?
        };
        tracing::warn!(id, ruta = %ruta.display(), "video no reproducible; se ensena como documento o ficha");
        let contenido = match miniatura_de(&ruta, 1024) {
            // El nombre lleva la coletilla «sin codec de video»: un video
            // parado sin explicacion parece un fallo del programa.
            Some(vista) => Contenido::Documento {
                nombre: format!(
                    "{} · {}",
                    ruta.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    self.texto_sin_codec
                ),
                vista,
            },
            None => {
                let mut ficha = ficha_de(&ruta, &self.texto_no_encontrado);
                if let Contenido::Archivo { detalle, .. } = &mut ficha {
                    *detalle = format!("{detalle} · {}", self.texto_sin_codec);
                }
                ficha
            }
        };
        // Conserva el ancho del pin; el alto se adapta al contenido nuevo.
        let motor = Rc::clone(&self.motor);
        let (nw, nh) = tamano_natural(&contenido, escala, &|t, tam, max, tramos| {
            motor.medir_parrafo(t, tam, max, tramos)
        });
        let alto = if contenido.solo_ancho() {
            // La ficha tiene alto fijo: el contenido manda, no el ancho.
            nh
        } else if nw > 0 {
            ((region.ancho as f32) * (nh as f32 / nw as f32))
                .round()
                .max(1.0) as u32
        } else {
            region.alto
        };
        let nueva = Rect {
            x: region.x,
            y: region.y,
            ancho: region.ancho,
            alto,
        };
        self.crear_ventana(id, contenido, nueva, escala)
    }

    /// La ficha de un archivo o carpeta, por referencia (D28).
    pub fn pinear_archivo(&mut self, ruta: &Path, monitor: &Monitor) -> Result<()> {
        let t0 = std::time::Instant::now();
        let contenido = self.contenido_de_archivo(ruta, false);
        let tipo = match &contenido {
            Contenido::Video { .. } => "video",
            Contenido::Documento { .. } => "documento",
            _ => "ficha",
        };
        let region = self.region_centrada(&contenido, monitor);
        let id = self
            .almacen
            .borrow_mut()
            .guardar_archivo(
                ruta,
                Some(Pines::guardado_desde(region, monitor.escala_por_cien, 100)),
            )
            .context("no se pudo guardar la referencia")?;
        let hecho = self.crear_ventana(id, contenido, region, monitor.escala_por_cien);
        tracing::info!(
            id,
            tipo,
            ms = t0.elapsed().as_millis() as u64,
            "archivo pineado"
        );
        hecho
    }

    /// Una imagen del portapapeles: no viene de ninguna region de pantalla,
    /// asi que nace centrada y, si no cabe, al 80 % del area de trabajo.
    /// Devuelve el id del pin: quien importa una hoja de un proyecto lo
    /// necesita para colgarle despues su dibujo.
    pub fn pinear_imagen_centrada(
        &mut self,
        imagen: &ImagenRgba,
        monitor: &Monitor,
    ) -> Result<u64> {
        let contenido = Contenido::Imagen(imagen.clone());
        let region = self.region_centrada(&contenido, monitor);
        let png = codificar_png(imagen).context("no se pudo codificar la imagen")?;
        let id = self
            .almacen
            .borrow_mut()
            .guardar_imagen(
                &png,
                "portapapeles",
                Some(Pines::guardado_desde(region, monitor.escala_por_cien, 100)),
            )
            .context("no se pudo guardar en el almacen")?;
        self.crear_ventana(id, contenido, region, monitor.escala_por_cien)?;
        Ok(id)
    }

    /// Donde nace un pin que no viene de un recorte: centrado en el monitor
    /// del cursor, encogido al 80 % del area de trabajo si no cabe.
    fn region_centrada(&self, contenido: &Contenido, monitor: &Monitor) -> Rect {
        let motor = Rc::clone(&self.motor);
        let (mut w, mut h) = tamano_natural(
            contenido,
            monitor.escala_por_cien,
            &|t, tam, max, tramos| motor.medir_parrafo(t, tam, max, tramos),
        );

        let tope_w = (monitor.area_trabajo.ancho as f32 * 0.8) as u32;
        let tope_h = (monitor.area_trabajo.alto as f32 * 0.8) as u32;
        if w > tope_w || h > tope_h {
            // Se encoge proporcionalmente: deformar una captura para que
            // quepa seria peor que enseñarla mas pequena.
            let f = (tope_w as f32 / w as f32).min(tope_h as f32 / h as f32);
            w = ((w as f32 * f) as u32).max(1);
            h = ((h as f32 * f) as u32).max(1);
        }

        // En cascada, no exactamente centrados: pegar tres archivos a la vez
        // ponia los tres pines en el mismo pixel y parecian uno solo. El
        // escalon se reinicia cada ocho para no bajar sin fin.
        let escalon = (self.vivos.len() % 8) as i32 * (28 * monitor.escala_por_cien / 100) as i32;

        let rect = Rect {
            x: monitor.area_trabajo.x
                + (monitor.area_trabajo.ancho as i32 - w as i32) / 2
                + escalon,
            y: monitor.area_trabajo.y + (monitor.area_trabajo.alto as i32 - h as i32) / 2 + escalon,
            ancho: w,
            alto: h,
        };
        recolocar_en_area(rect, monitor.area_trabajo)
    }

    /// Restaura los pines abiertos del almacen. Los fallos individuales se
    /// registran y no tumban el resto; devuelve cuantos volvieron.
    pub fn restaurar(&mut self, disposicion: &DisposicionMonitores) -> usize {
        struct Pendiente {
            id: u64,
            guardado: PinGuardado,
            tipo: TipoEntrada,
            objeto: PathBuf,
            ruta: Option<PathBuf>,
            /// De donde vino: dice si una nota es una herramienta
            /// (`mini:tareas`) o una imagen una pizarra.
            origen: String,
        }

        let pendientes: Vec<Pendiente> = {
            let a = self.almacen.borrow();
            let ocultos: Vec<u32> = a
                .grupos()
                .iter()
                .filter(|g| g.oculto)
                .map(|g| g.id)
                .collect();
            a.entradas()
                .iter()
                // Un grupo oculto NO vuelve solo: ni al arrancar ni al
                // restaurar otro. Conserva su `pin` para saber donde
                // devolverlo cuando el usuario lo pida desde la bandeja.
                .filter(|e| !e.grupo.is_some_and(|g| ocultos.contains(&g)))
                // Ni se duplica uno que ya esta en pantalla: `restaurar`
                // tambien se usa al mostrar un grupo, con otros ya abiertos.
                .filter(|e| !self.vivos.contains_key(&e.id))
                .filter_map(|e| {
                    e.pin.map(|p| Pendiente {
                        id: e.id,
                        guardado: p,
                        tipo: e.tipo,
                        objeto: a.ruta_objeto(e),
                        ruta: e.ruta.clone(),
                        origen: e.origen.clone(),
                    })
                })
                .collect()
        };
        let mut restaurados = 0;
        for p in pendientes {
            let (id, guardado) = (p.id, p.guardado);
            let contenido = match p.tipo {
                TipoEntrada::Imagen => match cargar(&p.objeto) {
                    Ok(i) => {
                        // Una pizarra vuelve con su fondo en el menu.
                        self.reconocer_al_restaurar(id, &p.origen, None);
                        Contenido::Imagen(i)
                    }
                    Err(e) => {
                        tracing::warn!(?e, id, "pin sin objeto legible; queda solo en el almacen");
                        continue;
                    }
                },
                TipoEntrada::Nota => match std::fs::read_to_string(&p.objeto) {
                    // Una herramienta es una nota con su origen: vuelve como
                    // herramienta, con su documento.
                    Ok(texto) => match self.reconocer_al_restaurar(id, &p.origen, Some(&texto)) {
                        Some(c) => c,
                        None => Contenido::Nota { texto },
                    },
                    Err(e) => {
                        tracing::warn!(?e, id, "nota sin fichero legible; queda en el almacen");
                        continue;
                    }
                },
                // La referencia rota se restaura igual y se MUESTRA como "no
                // encontrado" (D28): esconderla perderia el rastro de algo
                // que el usuario dejo pineado a proposito.
                TipoEntrada::Archivo => match &p.ruta {
                    Some(r) => self.contenido_de_archivo(r, true),
                    None => {
                        tracing::warn!(id, "entrada de archivo sin ruta; se ignora");
                        continue;
                    }
                },
            };
            let rect = Rect {
                x: guardado.x,
                y: guardado.y,
                ancho: guardado.ancho,
                alto: guardado.alto,
            };
            // Si el monitor de origen ya no existe (o el pin quedo fuera),
            // se desliza al area de trabajo mas razonable (spec 5.2).
            let (rect, escala) = match disposicion
                .monitores()
                .iter()
                .find(|m| m.area.interseccion(rect).is_some())
            {
                Some(m) => (recolocar_en_area(rect, m.area_trabajo), m.escala_por_cien),
                None => match disposicion.principal() {
                    Some(p) => (recolocar_en_area(rect, p.area_trabajo), p.escala_por_cien),
                    None => (rect, guardado.escala_por_cien),
                },
            };
            match self.crear_ventana(id, contenido, rect, escala) {
                Ok(()) => {
                    restaurados += 1;
                    // El zoom del texto de una nota vuelve con ella.
                    if guardado.zoom_por_cien != 100 {
                        if let Some(pin) = self.vivos.get(&id) {
                            pin.poner_zoom_por_cien(guardado.zoom_por_cien);
                        }
                    }
                }
                Err(e) => tracing::warn!(?e, id, "no se pudo restaurar el pin"),
            }
        }
        restaurados
    }

    /// Saca de la lista los pines que se cerraron desde su propio WndProc.
    /// Llamar desde el bucle principal; barato (dos punteros si esta vacia).
    pub fn purgar(&mut self) {
        let cerrados: Vec<u64> = self.cerrados.borrow_mut().drain(..).collect();
        for id in cerrados {
            self.vivos.remove(&id);
            self.herramientas.remove(&id);
            self.pizarras.remove(&id);
            // Soltar el pin en vivo cierra tambien su captura (Drop).
            if let Some((_, recorte)) = self.en_vivo.remove(&id) {
                tracing::info!(
                    id,
                    aceptados = recorte.borrow().aceptados(),
                    "pin en vivo cerrado"
                );
            }
        }
        let pedidos: Vec<(u64, CambioPin)> = self.pedidos.borrow_mut().drain(..).collect();
        // Los punteros de la anotacion se procesan sin pintar y se pinta UNA
        // vez por tanda (I1): un solo WM_MOUSEMOVE trae hasta 63 puntos
        // recuperados, y repintar el pin con cada uno hacia el repintado
        // lento, que a su vez fusionaba mas puntos: un circulo vicioso.
        let mut sucio = false;
        for paso in planificar_pedidos(&pedidos) {
            match paso {
                PasoPedido::Atender(n) => {
                    let (id, cambio) = pedidos[n];
                    if let Err(e) = self.atender(id, cambio) {
                        tracing::warn!(?e, id, ?cambio, "no se pudo atender la peticion del pin");
                    }
                }
                PasoPedido::Anotar(n) => {
                    let (id, cambio) = pedidos[n];
                    let Some(evento) = evento_de_puntero(cambio) else {
                        continue;
                    };
                    // Al soltar puede haber cambiado lo elegido, y con ello
                    // lo que ofrece el panel. Solo entonces, no con cada
                    // movimiento: mientras se traza no hay nada nuevo que
                    // ajustar y repintarlo costaria un fotograma por punto.
                    let solto = matches!(cambio, CambioPin::PunteroSoltado(_));
                    match self.procesar_anotacion(id, evento) {
                        Ok(pide) => sucio |= pide,
                        Err(e) => {
                            tracing::warn!(?e, id, ?cambio, "no se pudo anotar en el pin")
                        }
                    }
                    if solto {
                        self.repintar_panel();
                    }
                }
                PasoPedido::Repintar(id) => {
                    if std::mem::take(&mut sucio) {
                        self.repintar_anotacion(id);
                    }
                }
            }
        }
    }

    /// Lo que el pin pidio y no podia hacer solo.
    fn atender(&mut self, id: u64, cambio: CambioPin) -> Result<()> {
        if self.en_vivo.contains_key(&id) {
            return self.atender_en_vivo(id, cambio);
        }
        // Lo de las herramientas, las pizarras y el panel (`sacar`).
        if self.atender_herramienta(id, cambio)? {
            return Ok(());
        }
        match cambio {
            // Solo lo pide un pin en vivo, y ese ya salio por arriba.
            CambioPin::CongelarPedido => Ok(()),
            CambioPin::CopiarPedido => self.copiar(id),
            CambioPin::TextoPedido => self.copiar_texto(id),
            CambioPin::ReconocerPedido => self.reconocer_texto(id).map(|_| ()),
            CambioPin::PaginaPedida(salto) => self.cambiar_pagina(id, salto),
            CambioPin::ExtraerPaginaPedida => self.extraer_pagina(id),
            CambioPin::ExtraerTodasPedida => self.extraer_todas(id),
            CambioPin::GrupoPedido(indice) => {
                let color = match indice {
                    None => None,
                    Some(i) => Some(
                        ColorGrupo::por_indice(i).context("indice de color fuera de la paleta")?,
                    ),
                };
                self.poner_grupo(id, color)
            }
            CambioPin::OcultarGrupoPedido => self.ocultar_grupo_de(id),
            CambioPin::EliminarPedido => self.eliminar(id),
            CambioPin::AbrirPedido => self.abrir(id, false),
            CambioPin::AbrirUbicacionPedido => self.abrir(id, true),
            CambioPin::GuardarComoPedido => self.guardar_como(id),
            CambioPin::AnotarPedido => self.entrar_a_anotar(id),
            // `purgar` los lleva por `procesar_anotacion` en tandas; esto
            // queda para quien atienda un pedido suelto.
            CambioPin::PunteroPulsado(_)
            | CambioPin::PunteroMovido(_)
            | CambioPin::PunteroSoltado(_)
            | CambioPin::MuestraPuntero { .. } => match evento_de_puntero(cambio) {
                Some(evento) => self.anotar(id, evento),
                None => Ok(()),
            },
            CambioPin::RuedaGirada { delta, cursor } => {
                // Anotando, la rueda cambia el grosor; si no, hace zoom del
                // pin, que es lo que pidio el usuario (D55).
                if self.anotacion.as_ref().is_some_and(|a| a.id == id) {
                    self.anotar(id, EventoAnotador::Rueda(delta))?;
                    // El grosor nuevo se ve tambien en el panel.
                    self.repintar_panel();
                    Ok(())
                } else {
                    self.zoom(id, delta, cursor)
                }
            }
            CambioPin::EscapeAnotando => {
                self.anotar(id, EventoAnotador::Tecla(TeclaAnotador::Escape))
            }
            CambioPin::CaracterAnotando(c) => self.anotar(id, EventoAnotador::Caracter(c)),
            CambioPin::EnterAnotando => {
                self.anotar(id, EventoAnotador::Tecla(TeclaAnotador::Enter))
            }
            CambioPin::RetrocesoAnotando => {
                self.anotar(id, EventoAnotador::Tecla(TeclaAnotador::Retroceso))
            }
            CambioPin::PaletaPulsada(p) => self.paleta_pulsada(id, p),
            CambioPin::VideoFallido => self.degradar_video(id),
            CambioPin::AbrirLienzoPedido => self.pedir_lienzo(id),
            // Movido, Redimensionado y Cerrado los resuelve el callback.
            _ => Ok(()),
        }
    }

    /// Donde vive el dibujo de un pin: junto a su objeto, mismo nombre y
    /// otra extension (D48). El objeto original nunca se toca.
    fn ruta_anotacion(&self, id: u64) -> Option<PathBuf> {
        let a = self.almacen.borrow();
        let e = a.entradas().iter().find(|e| e.id == id)?;
        if e.objeto.is_empty() {
            // Una ficha de archivo no tiene objeto propio que anotar.
            return None;
        }
        Some(a.ruta_objeto(e).with_extension(pixpin_motor2d::EXTENSION))
    }

    /// Doble clic: entra en modo anotacion cargando lo que ya hubiera.
    fn entrar_a_anotar(&mut self, id: u64) -> Result<()> {
        let t0 = std::time::Instant::now();
        // Salir del anterior guardando: dos pines anotandose a la vez no
        // significa nada y enredaria el foco del teclado.
        self.salir_de_anotar()?;

        let ruta = self
            .ruta_anotacion(id)
            .context("este pin no tiene contenido que anotar")?;
        let escena = pixpin_motor2d::cargar(&ruta).context("no se pudo leer la anotacion")?;
        // Las herramientas del lienzo, anfitrion `Pin` (`dibujo::permitidas`):
        // las mismas teclas y la misma barra, menos lo que el pin no pinta y
        // lo apagado en los ajustes.
        let mut gesto = pixpin_motor2d::gesto::Gesto::nuevo();
        crate::dibujo::permitidas::asegurar(crate::dibujo::permitidas::Anfitrion::Pin, &mut gesto);
        let mano = crate::dibujo::mano::Mano::nueva(crate::dibujo::permitidas::Anfitrion::Pin);

        let pin = self.vivos.get(&id).context("el pin no esta en pantalla")?;
        // La barra va en el monitor del pin: en otro se saldria de la
        // pantalla.
        let contenido = pin.rect_contenido();
        let disposicion = pixpin_capture::enumerar_monitores().context("sin monitores")?;
        let monitor = disposicion
            .monitores()
            .iter()
            .find(|m| {
                m.area.contiene(Punto {
                    x: contenido.x,
                    y: contenido.y,
                })
            })
            .or_else(|| disposicion.principal())
            .copied()
            .context("sin monitor para la paleta")?;
        let caja = caja_del_pin(monitor.area_trabajo, monitor.escala_por_cien);
        let pedidos = Rc::clone(&self.pedidos);
        let hwnd_app = self.hwnd_app;
        let paleta = Paleta::nueva(
            &self.d3d,
            Rc::clone(&self.motor),
            caja.marco,
            Box::new(move |p| {
                // Mismo camino que el menu del pin: se apunta y se despierta
                // al bucle, que lo atiende fuera del prestamo.
                pedidos.borrow_mut().push((id, CambioPin::PaletaPulsada(p)));
                pixpin_shell::despertar(hwnd_app);
            }),
        )
        .context("no se pudo crear la paleta del pin")?;
        let panel = self.panel_nuevo(id)?;
        let pin = self.vivos.get(&id).context("el pin no esta en pantalla")?;

        pin.poner_modo_anotacion(true);
        pin.poner_cursor_anotacion(cursor_pin_de(gesto.herramienta));
        let zoom = pin.escala_contenido().0.max(1e-3);
        let mut guardados = pixpin_motor2d::tinta::grafito::MapasSueltos::default();
        let (ordenes, grafitos) = anotaciones_de(&escena, &mut guardados);
        pin.poner_anotaciones_con_grafito(
            ordenes,
            grafitos,
            pixpin_motor2d::mosaico::cajas_tapadas(&escena.elementos),
        );
        self.anotacion = Some(Anotacion {
            id,
            escena,
            gesto,
            mano,
            caja,
            paleta,
            panel,
            trabajo: monitor.area_trabajo,
            escala_por_cien: monitor.escala_por_cien,
            zoom,
            ultimo_cursor: Punto { x: 0, y: 0 },
            cursor_documento: pixpin_motor2d::Punto2::nuevo(0.0, 0.0),
            ver_grosor: false,
            grafitos: RefCell::new(guardados),
        });
        self.repintar_paleta();
        tracing::info!(id, ms = t0.elapsed().as_millis() as u64, "modo anotacion");
        Ok(())
    }

    /// Vuelve a pintar la paleta con la herramienta activa resaltada. El
    /// pintor captura COPIAS: la paleta lo reusa en cada `WM_PAINT`.
    ///
    /// Con las hermanas de un grupo abiertas la ventana crece hasta
    /// abarcarlas (una ventana no pinta fuera de si misma, que es por lo que
    /// el movil las saca del `DrawToolbar`), y vuelve a la barra al cerrarlas.
    fn repintar_paleta(&self) {
        let Some(a) = &self.anotacion else {
            return;
        };
        let caja = a.caja.con_desplegado(a.mano.desplegado);
        let activa = a.gesto.herramienta;
        let escala = a.escala_por_cien;
        let rect = rect_de_paleta(&caja);
        let origen = (rect.x as f32, rect.y as f32);
        a.paleta.poner_pintor(Box::new(move |p| {
            // La barra del lienzo, en el `(0, 0)` de la ventana de la paleta.
            p.desplazar(-origen.0, -origen.1);
            crate::caja_dibujo::pintar_barra(p, &caja, activa, escala, None, |b| match b {
                BotonCaja::Elegir(h) => crate::dibujo::teclas::tecla_de(h),
                _ => None,
            });
            p.desplazar(0.0, 0.0);
        }));
        a.paleta.recolocar(Some(rect));
        // El panel sigue a la herramienta: su color y su grosor.
        self.repintar_panel();
    }

    /// Un clic en la paleta, en coordenadas de la paleta (D58).
    fn paleta_pulsada(&mut self, id: u64, p: Punto) -> Result<()> {
        let Some(a) = self.anotacion.as_mut().filter(|a| a.id == id) else {
            return Ok(());
        };
        let caja = a.caja.con_desplegado(a.mano.desplegado);
        let origen = rect_de_paleta(&caja);
        let global = Punto {
            x: p.x + origen.x,
            y: p.y + origen.y,
        };
        // Cualquier clic cierra las hermanas, como en el lienzo
        // (`dibujo::mano`); pulsar un grupo decide despues si se abre.
        let habia = a.mano.desplegado.take();
        let Some(boton) = caja.boton_en(global) else {
            if habia.is_some() {
                self.repintar_paleta();
            }
            return Ok(());
        };
        let boton = match boton {
            BotonCaja::Grupo(g) => {
                let r = crate::dibujo::grupos::pulsar(&caja, g, a.gesto.herramienta, habia);
                a.mano.desplegado = r.desplegado;
                match r.elegir {
                    Some(h) => BotonCaja::Elegir(h),
                    None => {
                        self.repintar_paleta();
                        return Ok(());
                    }
                }
            }
            otro => otro,
        };
        match boton {
            BotonCaja::Elegir(h) => {
                self.anotar(id, EventoAnotador::CambiarHerramienta(h))?;
                self.repintar_paleta();
                // El cursor sigue a la herramienta (lo pidio el usuario).
                if let Some(pin) = self.vivos.get(&id) {
                    pin.poner_cursor_anotacion(cursor_pin_de(h));
                }
            }
            BotonCaja::Deshacer => {
                self.anotar(id, EventoAnotador::Tecla(TeclaAnotador::Deshacer))?
            }
            BotonCaja::Rehacer => self.anotar(id, EventoAnotador::Tecla(TeclaAnotador::Rehacer))?,
            // Los colores y grosores estan en el panel de al lado (`panel`),
            // que ya esta a la vista; la barra del pin no tiene boton propio.
            BotonCaja::Color => {}
            BotonCaja::Salir => self.salir_de_anotar()?,
            // La imagen y las figuras son del lienzo (`permitidas`): en la
            // barra del pin no salen.
            BotonCaja::Imagen
            | BotonCaja::Figuras
            | BotonCaja::Imprimir
            | BotonCaja::Compartir
            | BotonCaja::Atravesar
            | BotonCaja::Grupo(_) => {}
        }
        if habia.is_some() && !matches!(boton, BotonCaja::Elegir(_) | BotonCaja::Salir) {
            self.repintar_paleta();
        }
        Ok(())
    }

    /// Sale del modo anotacion guardando el dibujo.
    pub fn salir_de_anotar(&mut self) -> Result<()> {
        let Some(a) = self.anotacion.take() else {
            return Ok(());
        };
        if let Some(pin) = self.vivos.get(&a.id) {
            pin.poner_modo_anotacion(false);
        }
        let Some(ruta) = self.ruta_anotacion(a.id) else {
            return Ok(());
        };
        pixpin_motor2d::guardar(&ruta, &a.escena).context("no se pudo guardar la anotacion")?;
        tracing::info!(
            id = a.id,
            elementos = a.escena.cuantos_visibles(),
            "anotacion guardada"
        );
        Ok(())
    }

    /// Prepara el lienzo de un pin (D132). Queda en `lienzo_pedido` hasta
    /// que el bucle principal lo recoja con `tomar_lienzo`.
    fn pedir_lienzo(&mut self, id: u64) -> Result<()> {
        if self.lienzo_pedido.is_some() {
            return Ok(());
        }
        // D134: si se esta anotando ESTE pin, primero se guarda y se sale;
        // si no, el lienzo abriria el fichero de antes de la anotacion.
        if self.anotacion.as_ref().is_some_and(|a| a.id == id) {
            self.salir_de_anotar()?;
        }
        let ruta = self
            .ruta_anotacion(id)
            .context("este pin no tiene contenido que abrir en el lienzo")?;
        let (escena, habia_fichero) = match escena_para_lienzo(&ruta) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(
                    ?e,
                    id,
                    ruta = %ruta.display(),
                    "dibujo del pin corrupto; el lienzo no se abre para no pisarlo"
                );
                return Ok(());
            }
        };
        let (ruta_imagen, guardado) = {
            let a = self.almacen.borrow();
            let e = a
                .entradas()
                .iter()
                .find(|e| e.id == id)
                .context("el pin no esta en el almacen")?;
            (a.ruta_objeto(e), e.pin)
        };
        let fondo = match cargar(&ruta_imagen) {
            Ok(imagen) => imagen,
            Err(e) => {
                let (ancho, alto) = tamano_de_reserva(guardado);
                tracing::warn!(
                    ?e,
                    id,
                    ancho,
                    alto,
                    "imagen del pin ilegible; el lienzo abre sobre un recuadro gris"
                );
                crate::fondo_lienzo::recuadro_gris(ancho, alto)
            }
        };
        self.lienzo_pedido = Some(PedidoLienzo {
            id,
            ruta,
            habia_fichero,
            escena,
            fondo,
        });
        Ok(())
    }

    /// Lo que `purgar` dejo preparado para abrir en el lienzo, si hay algo.
    pub fn tomar_lienzo(&mut self) -> Option<PedidoLienzo> {
        self.lienzo_pedido.take()
    }

    /// Lo que devuelve el editor al cerrar (D146): guardar si toca y
    /// repintar el pin con lo guardado. Un fallo al guardar deja el fichero
    /// y el pin como estaban.
    pub fn terminar_lienzo(
        &mut self,
        id: u64,
        ruta: &Path,
        habia_fichero: bool,
        resultado: Result<Escena>,
    ) {
        let escena = match resultado {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(?e, id, "no se pudo abrir el lienzo");
                return;
            }
        };
        match guardar_lienzo(ruta, &escena, habia_fichero) {
            Err(e) => {
                tracing::error!(
                    ?e,
                    id,
                    "no se pudo guardar el lienzo; el pin sigue con lo de antes"
                )
            }
            Ok(guardado) => {
                tracing::info!(
                    id,
                    guardado,
                    elementos = escena.cuantos_visibles(),
                    "lienzo cerrado"
                );
                // Directamente con la escena y no con `recargar_anotaciones`:
                // esa no toca el pin si el fichero quedo vacio, y lo borrado
                // en el lienzo seguiria viendose en el pin.
                if let Some(pin) = self.vivos.get(&id) {
                    let (ordenes, grafitos) = anotaciones_de(&escena, &mut Default::default());
                    pin.poner_anotaciones_con_grafito(
                        ordenes,
                        grafitos,
                        pixpin_motor2d::mosaico::cajas_tapadas(&escena.elementos),
                    );
                }
            }
        }
    }

    /// Un evento del puntero o del teclado mientras se anota, y su repintado.
    fn anotar(&mut self, id: u64, evento: EventoAnotador) -> Result<()> {
        if self.procesar_anotacion(id, evento)? {
            self.repintar_anotacion(id);
        }
        Ok(())
    }

    /// Lleva un evento por las herramientas del lienzo (`dibujo::mano`) y
    /// aplica su efecto a la escena, SIN pintar. Devuelve si el pin necesita
    /// repintarse: `purgar` junta asi una tanda de puntos en un solo
    /// repintado (I1).
    ///
    /// `EventoAnotador` sigue siendo el sobre en el que llegan los pedidos
    /// del pin (lo que ya decia `planificar_pedidos`); lo que cambio es quien
    /// los atiende.
    fn procesar_anotacion(&mut self, id: u64, evento: EventoAnotador) -> Result<bool> {
        use crate::dibujo::mano::Vista;
        use pixpin_shell::overlay::EventoOverlay as E;
        let escala = self
            .vivos
            .get(&id)
            .map_or((1.0, 1.0), |pin| pin.escala_contenido());
        // Pulsar en el pin (dibujar) o Escape cierran las hermanas de un
        // grupo de la barra, como en el lienzo; Escape no hace nada mas.
        let cerrar = matches!(
            evento,
            EventoAnotador::Pulsar(_) | EventoAnotador::Tecla(TeclaAnotador::Escape)
        );
        if cerrar
            && let Some(a) = self.anotacion.as_mut().filter(|a| a.id == id)
            && a.mano.desplegado.take().is_some()
        {
            self.repintar_paleta();
            if matches!(evento, EventoAnotador::Tecla(TeclaAnotador::Escape)) {
                return Ok(false);
            }
        }
        let Some(a) = self.anotacion.as_mut().filter(|a| a.id == id) else {
            return Ok(false);
        };
        if let EventoAnotador::Mover(p)
        | EventoAnotador::Pulsar(p)
        | EventoAnotador::Muestra { p, .. } = &evento
        {
            a.ultimo_cursor = Punto {
                x: p.x as i32,
                y: p.y as i32,
            };
        }
        match &evento {
            EventoAnotador::Rueda(_) => a.ver_grosor = true,
            EventoAnotador::Pulsar(_) => a.ver_grosor = false,
            _ => {}
        }
        // El dibujo va en pixeles del contenido ORIGINAL; el raton, en los de
        // la ventana. La camara es esa escala: con el pin agrandado o reducido
        // la tinta cae bajo el puntero (lo reporto el usuario).
        a.zoom = escala.0.max(1e-3);
        let en_el_documento = a_documento(evento.clone(), escala);
        if let EventoAnotador::Mover(p)
        | EventoAnotador::Pulsar(p)
        | EventoAnotador::Muestra { p, .. } = en_el_documento
        {
            a.cursor_documento = p;
        }
        let camara = pixpin_motor2d::camara::Camara {
            x: 0.0,
            y: 0.0,
            zoom: a.zoom,
        };
        // El contenido del pin, en sus coordenadas: el raton llega asi.
        let area = Rect {
            x: 0,
            y: 0,
            ancho: u32::MAX / 4,
            alto: u32::MAX / 4,
        };
        let vista = Vista {
            camara: &camara,
            area,
            interfaz: area,
            escala_por_cien: a.escala_por_cien,
        };
        let punto = |p: pixpin_motor2d::Punto2| Punto {
            x: p.x.round() as i32,
            y: p.y.round() as i32,
        };
        const VK_ESCAPE: u32 = 0x1B;
        const VK_ENTRAR: u32 = 0x0D;
        const VK_RETROCESO: u32 = 0x08;
        /// Ctrl+Z y Ctrl+Y llegan al pin como su caracter de control.
        const CTRL_Z: char = '\u{1a}';
        const CTRL_Y: char = '\u{19}';
        let tecla = |vk: u32| E::Tecla {
            vk,
            shift: false,
            ctrl: false,
            alt: false,
        };
        let ev = match evento {
            EventoAnotador::Pulsar(p) => Some(E::BotonPulsado(punto(p))),
            EventoAnotador::Mover(p) => Some(E::RatonMovido(punto(p))),
            EventoAnotador::Soltar(p) => Some(E::BotonSoltado(punto(p))),
            EventoAnotador::Muestra { presion, .. } => {
                // Un punto fino del lapiz: derecho al motor, con su presion y
                // su subpixel. La goma, el lazo y el cuentagotas lo siguen por
                // el `Mover` que le sigue.
                let por_la_mano = matches!(
                    a.gesto.herramienta,
                    Herramienta::Borrador | Herramienta::Lazo | Herramienta::CopiarEstilo
                );
                if let (false, EventoAnotador::Muestra { p, .. }) = (por_la_mano, en_el_documento) {
                    let g = crate::dibujo::teclas::con_modificadores(
                        pixpin_motor2d::gesto::EventoGesto::Mover {
                            p,
                            shift: false,
                            alt: false,
                            presion,
                        },
                    );
                    a.mano.al_motor(g, &mut a.gesto, &mut a.escena, &camara);
                    return Ok(true);
                }
                None
            }
            EventoAnotador::Rueda(delta) => {
                a.gesto.grosor_tinta = grosor_con_rueda(a.gesto.grosor_tinta, delta);
                return Ok(true);
            }
            EventoAnotador::Caracter(CTRL_Z) => {
                a.escena.deshacer();
                return Ok(true);
            }
            EventoAnotador::Caracter(CTRL_Y) => {
                a.escena.rehacer();
                return Ok(true);
            }
            EventoAnotador::Caracter(c) => Some(E::Caracter(c)),
            EventoAnotador::Tecla(TeclaAnotador::Escape) => {
                // Escape suelta lo que haya a medias (el texto, lo elegido,
                // un gesto); sin nada, sale guardando, como siempre.
                let algo = a.gesto.esta_escribiendo()
                    || !a.gesto.seleccion.esta_vacia()
                    || !a.gesto.en_reposo()
                    || a.gesto.lazo.is_some();
                if !algo {
                    self.salir_de_anotar()?;
                    return Ok(false);
                }
                Some(tecla(VK_ESCAPE))
            }
            EventoAnotador::Tecla(TeclaAnotador::Enter) => Some(tecla(VK_ENTRAR)),
            EventoAnotador::Tecla(TeclaAnotador::Retroceso) => Some(tecla(VK_RETROCESO)),
            EventoAnotador::Tecla(TeclaAnotador::Deshacer) => {
                a.escena.deshacer();
                return Ok(true);
            }
            EventoAnotador::Tecla(TeclaAnotador::Rehacer) => {
                a.escena.rehacer();
                return Ok(true);
            }
            EventoAnotador::CambiarHerramienta(h) => {
                if crate::dibujo::permitidas::permitida(crate::dibujo::permitidas::Anfitrion::Pin, h)
                {
                    crate::dibujo::teclas::elegir_herramienta(&mut a.gesto, h);
                }
                return Ok(true);
            }
            _ => None,
        };
        let Some(ev) = ev else {
            return Ok(false);
        };
        let hecho = a
            .mano
            .interfaz(&ev, &mut a.gesto, &mut a.escena, None, false, vista);
        let hecho = if hecho.consumido {
            hecho
        } else {
            a.mano
                .herramienta(&ev, &mut a.gesto, &mut a.escena, None, vista, 0.0, 0.0)
        };
        let cursor = hecho.paso.as_ref().map(|p| p.r.cursor);
        let repintar = hecho.consumido || hecho.repinte.algo();
        let herramienta = a.gesto.herramienta;
        // Una letra pudo cambiar de herramienta: la paleta y el cursor la
        // siguen.
        let letra = matches!(ev, E::Caracter(_));
        if letra {
            self.repintar_paleta();
        }
        if (cursor.is_some() || letra)
            && let Some(pin) = self.vivos.get(&id)
        {
            pin.poner_cursor_anotacion(cursor_pin_de(herramienta));
        }
        Ok(repintar)
    }

    /// Pinta en el pin lo que la anotacion tiene ahora.
    fn repintar_anotacion(&self, id: u64) {
        let Some(a) = self.anotacion.as_ref().filter(|a| a.id == id) else {
            return;
        };
        let (ordenes, grafitos) = a.ordenes();
        // Lo que tapa un mosaico, aparte de las ordenes: una `Orden` ya no
        // dice de que figura salio.
        let tapadas = pixpin_motor2d::mosaico::cajas_tapadas(&a.escena.elementos);
        if let Some(pin) = self.vivos.get(&id) {
            // La lupa del anotador viejo no esta entre las herramientas del
            // pin (`permitidas`): se quita por si quedo puesta.
            pin.poner_lupa(None);
            pin.poner_anotaciones_con_grafito(ordenes, grafitos, tapadas);
        }
    }

    /// Rueda sobre un pin que no se esta anotando: agranda o encoge (D55).
    fn zoom(&mut self, id: u64, delta: i32, cursor: Punto) -> Result<()> {
        let Some(pin) = self.vivos.get(&id) else {
            return Ok(());
        };
        // La ficha no se redimensiona, tampoco con la rueda.
        if !pin.redimensionable() {
            return Ok(());
        }
        // Desde el DESTINO en curso, no desde el fotograma intermedio: una
        // rueda que sigue girando encadena pasos y la animacion los sigue
        // sin saltos (el usuario los veia «de salto en salto»).
        let r = pin.rect_objetivo();
        if r.ancho == 0 || r.alto == 0 {
            return Ok(());
        }
        // Proporcional al giro: una rueda fina (tactil) da deltas pequenos
        // y pasos pequenos; una muesca entera, el 10 %.
        let paso = 1.1f32.powf(delta as f32 / 120.0);
        // Anclado en el CURSOR, no en el centro: lo que el usuario esta
        // mirando se queda bajo el puntero mientras crece todo lo demas. Con
        // el centro, el detalle se le escapaba de debajo del raton.
        // El tope es el mismo que el del zoom por arrastre: la ventana se
        // recorta al escritorio, asi que un pin enorme no cuesta memoria.
        let nuevo = pixpin_pin::escalar_anclado(
            r,
            paso,
            cursor,
            pixpin_pin::MINIMO_LOGICO,
            pixpin_pin::MAXIMO_FISICO,
        );
        let zoom = pin.zoom_objetivo_por_cien(nuevo);
        pin.escalar_persiguiendo(nuevo);
        self.almacen
            .borrow_mut()
            // Con la escala REAL del pin: guardar 100 en un monitor al 150 %
            // hacia que el pin volviera 1,5 veces mas grande tras reiniciar.
            .actualizar_pin(
                id,
                Some(Pines::guardado_desde(nuevo, pin.escala_por_cien(), zoom)),
            )
            .ok();
        Ok(())
    }

    /// Oculta en bloque el grupo del pin: cierra sus ventanas pero DEJA sus
    /// `pin` en el indice, que es lo que permite devolverlos a su sitio
    /// exacto al mostrarlos (D24).
    fn ocultar_grupo_de(&mut self, id: u64) -> Result<()> {
        let grupo = self
            .almacen
            .borrow()
            .grupo_de(id)
            .context("el pin no tiene grupo que ocultar")?;
        self.almacen
            .borrow_mut()
            .poner_grupo_oculto(grupo.id, true)
            .context("no se pudo marcar el grupo como oculto")?;

        let del_grupo: Vec<u64> = self
            .almacen
            .borrow()
            .entradas()
            .iter()
            .filter(|e| e.grupo == Some(grupo.id))
            .map(|e| e.id)
            .collect();
        for otro in del_grupo {
            self.vivos.remove(&otro);
        }
        Ok(())
    }

    /// Los grupos ocultos con su etiqueta ya montada, para el menu de la
    /// bandeja: es la unica via de vuelta de unos pines que ya no se ven.
    pub fn grupos_ocultos(&self, textos: &pixpin_store::Catalogo) -> Vec<(u32, String)> {
        let a = self.almacen.borrow();
        a.grupos()
            .iter()
            .filter(|g| g.oculto)
            .map(|g| {
                let cuantos = a
                    .entradas()
                    .iter()
                    .filter(|e| e.grupo == Some(g.id))
                    .count();
                let color = textos.t(match g.color {
                    ColorGrupo::Rojo => "pin-color-rojo",
                    ColorGrupo::Naranja => "pin-color-naranja",
                    ColorGrupo::Ambar => "pin-color-ambar",
                    ColorGrupo::Verde => "pin-color-verde",
                    ColorGrupo::Cian => "pin-color-cian",
                    ColorGrupo::Azul => "pin-color-azul",
                    ColorGrupo::Violeta => "pin-color-violeta",
                    ColorGrupo::Rosa => "pin-color-rosa",
                });
                (g.id, format!("● {color} ({cuantos})"))
            })
            .collect()
    }

    /// Devuelve a la pantalla los pines de un grupo oculto, cada uno donde
    /// estaba. Devuelve cuantos volvieron.
    pub fn mostrar_grupo(&mut self, id_grupo: u32, disposicion: &DisposicionMonitores) -> usize {
        if let Err(e) = self
            .almacen
            .borrow_mut()
            .poner_grupo_oculto(id_grupo, false)
        {
            tracing::warn!(?e, id_grupo, "no se pudo desmarcar el grupo");
            return 0;
        }
        self.restaurar(disposicion)
    }

    /// La unica accion destructiva (menu 4.3): borra la entrada y su objeto.
    /// Pregunta antes, con el «No» por defecto.
    fn eliminar(&mut self, id: u64) -> Result<()> {
        let hwnd = self
            .vivos
            .get(&id)
            .map(|p| p.hwnd())
            .context("el pin ya no esta abierto")?;
        if !pixpin_shell::confirmar_destructivo(
            hwnd,
            &self.textos.eliminar,
            &self.texto_confirmar_eliminar,
        ) {
            return Ok(());
        }
        self.almacen
            .borrow_mut()
            .eliminar(id)
            .context("no se pudo eliminar del almacen")?;
        self.vivos.remove(&id);
        self.herramientas.remove(&id);
        self.pizarras.remove(&id);
        Ok(())
    }

    /// Abre el archivo referenciado, o el Explorador con el seleccionado.
    fn abrir(&self, id: u64, ubicacion: bool) -> Result<()> {
        let ruta = {
            let a = self.almacen.borrow();
            a.entradas()
                .iter()
                .find(|e| e.id == id)
                .and_then(|e| e.ruta.clone())
                .context("esta entrada no referencia ningun fichero")?
        };
        if ubicacion {
            pixpin_shell::abrir_ubicacion(&ruta).context("no se pudo abrir la ubicacion")
        } else {
            pixpin_shell::abrir(&ruta).context("no se pudo abrir el fichero")
        }
    }

    /// Guarda una copia del contenido donde el usuario diga.
    fn guardar_como(&self, id: u64) -> Result<()> {
        let (tipo, objeto) = {
            let a = self.almacen.borrow();
            let e = a
                .entradas()
                .iter()
                .find(|e| e.id == id)
                .context("la entrada ya no esta en el almacen")?;
            (e.tipo, a.ruta_objeto(e))
        };
        let sugerido = match tipo {
            TipoEntrada::Nota => "nota.txt",
            _ => "captura.png",
        };
        let hwnd = self
            .vivos
            .get(&id)
            .map(|p| p.hwnd())
            .context("el pin ya no esta abierto")?;
        // Cancelar no es un fallo: el usuario cambio de idea.
        if let Some(destino) = pixpin_shell::guardar::pedir_ruta_guardado(
            hwnd,
            sugerido,
            pixpin_shell::guardar::Formatos::Imagen,
        ) {
            std::fs::copy(&objeto, &destino).context("no se pudo escribir el fichero")?;
            tracing::info!(?destino, "contenido del pin guardado");
        }
        Ok(())
    }

    /// `Ctrl+C` sobre un pin: imagen como mapa de bits, nota como texto,
    /// archivo como su ruta (spec 4.2). Se lee del ALMACEN, no de la
    /// ventana: el almacen es la verdad (D21).
    /// Lee el texto de la imagen del pin y se lo entrega para que se pueda
    /// seleccionar con el raton (P4.4).
    ///
    /// Se guarda SIEMPRE, aunque no haya salido texto: una lista vacia
    /// significa «se miro y no hay», y sin distinguirlo de «no se ha
    /// mirado» una imagen sin letras se reconoceria otra vez en cada
    /// pasada del raton.
    fn reconocer_texto(&self, id: u64) -> Result<Vec<pixpin_ocr::Linea>> {
        let Some(pin) = self.vivos.get(&id) else {
            return Ok(Vec::new());
        };
        let objeto = {
            let a = self.almacen.borrow();
            let Some(e) = a.entradas().iter().find(|e| e.id == id) else {
                return Ok(Vec::new());
            };
            if e.tipo != TipoEntrada::Imagen {
                return Ok(Vec::new());
            }
            a.ruta_objeto(e)
        };
        // Un fallo aqui no puede tumbar nada: el pin sigue siendo un pin
        // aunque no se le pueda leer el texto. Se anota y se guarda una
        // lista vacia para no volver a intentarlo en cada movimiento.
        let lineas = match cargar(&objeto) {
            Err(e) => {
                tracing::warn!(?e, id, "no se pudo leer la imagen para reconocer");
                Vec::new()
            }
            Ok(img) => {
                let empezado = std::time::Instant::now();
                match pixpin_ocr::reconocer(img.ancho, img.alto, &img.pixeles) {
                    Err(e) => {
                        tracing::warn!(?e, id, "no se pudo reconocer el texto del pin");
                        Vec::new()
                    }
                    Ok(lineas) => {
                        tracing::info!(
                            id,
                            renglones = lineas.len(),
                            ms = empezado.elapsed().as_millis() as u64,
                            "texto del pin reconocido"
                        );
                        lineas
                    }
                }
            }
        };
        let renglones: Vec<_> = lineas
            .iter()
            .map(|l| pixpin_geom::seleccion_texto::Renglon {
                palabras: l
                    .palabras
                    .iter()
                    .map(|p| pixpin_geom::seleccion_texto::Palabra {
                        caja: p.caja,
                        texto: p.texto.clone(),
                    })
                    .collect(),
            })
            .filter(|r| !r.palabras.is_empty())
            .collect();
        pin.poner_texto_reconocido(renglones);
        Ok(lineas)
    }

    /// Abre un `.pixpin` del movil: una hoja, un pin.
    ///
    /// Se eligio esto y no una ventana de proyecto con su lista de hojas
    /// porque reusa TODO lo que ya hay — los pines, las anotaciones, las
    /// notas — en vez de estrenar una interfaz entera. Un proyecto abierto
    /// se ve como lo que es: sus hojas encima de la mesa.
    ///
    /// Devuelve cuantas hojas salieron y cuantas quedaron fuera.
    pub fn abrir_paquete(
        &mut self,
        ruta: &Path,
        monitor: &Monitor,
        ubicacion: &pixpin_store::Ubicacion,
    ) -> Result<(usize, usize)> {
        let paquete = pixpin_proyecto::Paquete::abrir(ruta)
            .with_context(|| format!("no se pudo abrir {}", ruta.display()))?;
        // El PDF a un temporal: el lector de PDF trabaja sobre disco, y el
        // paquete lo tiene en memoria. Se escribe una vez para todas las
        // hojas, no una por hoja.
        let pdf = paquete.entrada("documento.pdf").and_then(|bytes| {
            let destino = std::env::temp_dir()
                .join("PixPin")
                .join(format!("{}.pdf", paquete.proyecto.id));
            std::fs::create_dir_all(destino.parent()?).ok()?;
            std::fs::write(&destino, bytes).ok()?;
            pixpin_pdf::Documento::abrir(&destino).ok()
        });

        let total = paquete.proyecto.hojas.len();
        let mut hechas = 0;
        for hoja in &paquete.proyecto.hojas {
            // Una hoja que falle no puede llevarse las demas: se anota y
            // se sigue, que es mejor que perder el proyecto entero por una.
            match self.abrir_hoja(&paquete, hoja, pdf.as_ref(), monitor) {
                Ok(true) => hechas += 1,
                Ok(false) => {}
                Err(e) => tracing::warn!(?e, hoja = %hoja.nombre, "hoja que no se pudo abrir"),
            }
        }
        tracing::info!(
            proyecto = %paquete.proyecto.nombre,
            hechas,
            total,
            "proyecto abierto"
        );

        // Y queda en la lista de la ventana de chat. Si ya estaba (los tres
        // codigos iguales) se pone al dia; si no, entra como uno nuevo, que
        // es lo que hace PixPin Android al recibir. Que esto falle no puede
        // deshacer un proyecto que ya esta abierto en pantalla.
        let raiz = ubicacion.raiz();
        let mut indice = pixpin_proyecto::almacen::Indice::leer(raiz);
        let (que, id) = indice.recibir(pixpin_proyecto::almacen::Ficha::de_proyecto(
            &paquete.proyecto,
            Some(ruta),
        ));
        match indice.guardar(raiz) {
            Ok(()) => tracing::info!(?que, %id, "proyecto en la lista"),
            Err(e) => tracing::warn!(?e, "no se pudo apuntar el proyecto en la lista"),
        }
        Ok((hechas, total))
    }

    /// Una hoja. Devuelve si salio algo en pantalla.
    fn abrir_hoja(
        &mut self,
        paquete: &pixpin_proyecto::Paquete,
        hoja: &pixpin_proyecto::Hoja,
        pdf: Option<&pixpin_pdf::Documento>,
        monitor: &Monitor,
    ) -> Result<bool> {
        // Una nota es una nota: el pin que ya tenemos le viene exacto.
        if let Some(texto) = paquete.nota_de(hoja) {
            if hoja.dibujo.is_none() {
                self.pinear_nota(&texto, monitor)?;
                return Ok(true);
            }
        }
        // Un croquis del espacio todavia no se sabe dibujar aqui. Se avisa
        // en vez de saltarselo en silencio: el usuario tiene que poder ver
        // que esa hoja existe y que su contenido sigue dentro del fichero.
        if hoja.dibujo.is_none() {
            if let Some(croquis) = &hoja.croquis {
                let aviso = format!(
                    "{}

Croquis del espacio ({croquis}).
Todavia no se puede ver aqui; sigue dentro del proyecto.",
                    hoja.nombre
                );
                self.pinear_nota(&aviso, monitor)?;
                return Ok(true);
            }
            return Ok(false);
        }

        let Some(lienzo) = paquete.lienzo_de(hoja) else {
            return Ok(false);
        };
        let lienzo = lienzo?;
        let elementos = lienzo.elementos();

        // El fondo: la pagina del PDF sobre la que se dibujo, si la hay.
        // Si no, un lienzo en blanco del tamano del dibujo — el Android
        // dibuja sobre el plano, y sin el la anotacion queda en el aire.
        let fondo = match (hoja.pagina, pdf) {
            (Some(pagina), Some(doc)) => doc.renderizar(pagina, ANCHO_PAGINA_EXTRAIDA).ok(),
            _ => None,
        };
        // Sobre una pagina de PDF el dibujo ya esta en las coordenadas de la
        // pagina y no se toca. Sin fondo, el lienzo se recorta a lo dibujado
        // y hay que mover el dibujo lo mismo que se recorto.
        let (imagen, dx, dy) = match fondo {
            Some(i) => (i, 0.0, 0.0),
            // Del papel del lienzo (su `viewBackgroundColor`): un dibujo
            // hecho sobre «Crema» en el movil es crema tambien en el pin.
            None => lienzo_en_blanco(&elementos, pixpin_motor2d::excalidraw::fondo(&lienzo)),
        };
        let id = self.pinear_imagen_centrada(&imagen, monitor)?;

        // Y el dibujo encima, por el mismo camino que una anotacion hecha
        // aqui: se escribe su fichero y se recarga. Reusar la via en vez de
        // meter los elementos a mano es lo que hace que una hoja importada
        // se pueda seguir editando, deshacer incluido.
        if !elementos.is_empty() {
            if let Some(destino) = self.ruta_anotacion(id) {
                let mut escena = pixpin_motor2d::Escena::nueva();
                for mut e in elementos {
                    e.mover(dx, dy);
                    escena.anadir(e);
                }
                if let Err(e) = pixpin_motor2d::guardar(&destino, &escena) {
                    tracing::warn!(?e, id, "no se pudo guardar el dibujo de la hoja");
                } else {
                    self.recargar_anotaciones(id);
                }
            }
        }
        Ok(true)
    }

    /// La ruta del fichero de una entrada, si lo tiene.
    fn ruta_de(&self, id: u64) -> Option<std::path::PathBuf> {
        self.almacen
            .borrow()
            .entradas()
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.ruta.clone())
    }

    /// Cambia de pagina en un PDF pineado, o solo cuenta cuantas tiene.
    ///
    /// `salto` de cero significa «solo dime cuantas hay»: es lo que pide
    /// el pin la primera vez que se abre su menu, para saber si ensenar
    /// las entradas de pagina.
    fn cambiar_pagina(&mut self, id: u64, salto: i32) -> Result<()> {
        let Some(pin) = self.vivos.get(&id) else {
            return Ok(());
        };
        let Some(ruta) = self.ruta_de(id) else {
            return Ok(());
        };
        let documento = match pixpin_pdf::Documento::abrir(&ruta) {
            Ok(d) => d,
            Err(e) => {
                // Un PDF cifrado o roto no puede tumbar el pin: se sigue
                // ensenando su miniatura, que es lo que ya habia.
                tracing::warn!(?e, id, "no se pudo abrir el PDF");
                return Ok(());
            }
        };
        let cuantas = documento.paginas();
        // Sin dar la vuelta: pasada la ultima no se empieza por la
        // primera. En un documento, el final es el final.
        let destino =
            (pin.pagina() as i64 + salto as i64).clamp(0, cuantas.saturating_sub(1) as i64) as u32;
        // El ancho al que se dibuja: el que tiene el pin ahora, para que
        // la pagina nueva se vea igual de nitida que la de antes sin
        // gastar en pixeles que no se van a ver.
        let ancho = pin.rect_contenido().ancho.max(1);
        match documento.renderizar(destino, ancho) {
            Ok(vista) => pin.poner_pagina(destino, cuantas, vista),
            Err(e) => tracing::warn!(?e, id, pagina = destino, "no se pudo dibujar la pagina"),
        }
        Ok(())
    }

    /// Saca la pagina que se ve como pin de imagen propio.
    fn extraer_pagina(&mut self, id: u64) -> Result<()> {
        let Some(pin) = self.vivos.get(&id) else {
            return Ok(());
        };
        let pagina = pin.pagina();
        let ruta = self.ruta_de(id).context("el pin no viene de un fichero")?;
        let d = pixpin_capture::enumerar_monitores()?;
        let monitor = d.principal().context("sin monitor")?.to_owned();
        let documento = pixpin_pdf::Documento::abrir(&ruta)?;
        self.extraer_una(&documento, pagina, &monitor)
    }

    /// Saca TODAS las paginas, una por pin.
    ///
    /// Con tope: un PDF de doscientas paginas dejaria la pantalla
    /// inservible y el almacen lleno. Se hacen las primeras y se dice
    /// cuantas quedaron fuera, que es mejor que reventar el escritorio o
    /// que recortar en silencio.
    fn extraer_todas(&mut self, id: u64) -> Result<()> {
        let ruta = self.ruta_de(id).context("el pin no viene de un fichero")?;
        let d = pixpin_capture::enumerar_monitores()?;
        let monitor = d.principal().context("sin monitor")?.to_owned();
        let documento = pixpin_pdf::Documento::abrir(&ruta)?;
        let total = documento.paginas();
        let cuantas = total.min(TOPE_PAGINAS);
        let mut hechas = 0;
        for pagina in 0..cuantas {
            // Una pagina que falle no puede parar las demas: se anota y se
            // sigue, que es mejor que perder las veinte siguientes por una.
            match self.extraer_una(&documento, pagina, &monitor) {
                Ok(()) => hechas += 1,
                Err(e) => tracing::warn!(?e, pagina, "no se pudo extraer la pagina"),
            }
        }
        tracing::info!(hechas, total, "paginas extraidas como pines");
        self.paginas_extraidas = Some((hechas, total));
        Ok(())
    }

    /// Dibuja una pagina y la pinea como imagen.
    fn extraer_una(
        &mut self,
        documento: &pixpin_pdf::Documento,
        pagina: u32,
        monitor: &Monitor,
    ) -> Result<()> {
        // A un ancho generoso y fijo, no al del pin de origen: la pagina
        // extraida es un documento para leer, y sale de una miniatura si
        // se dibuja al tamano de la que estaba en pantalla.
        let imagen = documento.renderizar(pagina, ANCHO_PAGINA_EXTRAIDA)?;
        self.pinear_imagen_centrada(&imagen, monitor).map(|_| ())
    }

    /// Lee el texto de la imagen del pin y lo copia (P4.2).
    ///
    /// Se lee del ALMACEN y no de lo que se ve en pantalla: el pin puede
    /// estar reducido, girado o con algo dibujado encima, y reconocer eso
    /// daria peor texto que reconocer el original, que es lo que se
    /// guardo tal cual se capturo.
    fn copiar_texto(&self, id: u64) -> Result<()> {
        // Se reconoce UNA vez y se usa dos: se copia el texto y ademas se
        // le deja al pin para que se pueda marcar con el raton sin volver
        // a esperar. Reconocer dos veces la misma imagen seria pagar el
        // tiron dos veces por lo mismo.
        let texto = crate::texto_de_lineas(self.reconocer_texto(id)?);
        if texto.trim().is_empty() {
            tracing::info!(id, "no se leyo texto en el pin");
            return Ok(());
        }
        pixpin_codec::copiar_texto(&texto).context("no se pudo copiar el texto")?;
        tracing::info!(id, largo = texto.len(), "texto del pin copiado");
        Ok(())
    }

    fn copiar(&self, id: u64) -> Result<()> {
        let (tipo, objeto, ruta) = {
            let a = self.almacen.borrow();
            let e = a
                .entradas()
                .iter()
                .find(|e| e.id == id)
                .context("la entrada ya no esta en el almacen")?;
            (e.tipo, a.ruta_objeto(e), e.ruta.clone())
        };
        match tipo {
            TipoEntrada::Imagen => {
                let img = cargar(&objeto).context("no se pudo leer la imagen del almacen")?;
                pixpin_codec::copiar_imagen(&img).context("no se pudo copiar la imagen")?;
            }
            TipoEntrada::Nota => {
                let texto = std::fs::read_to_string(&objeto).context("no se pudo leer la nota")?;
                pixpin_codec::copiar_texto(&texto).context("no se pudo copiar la nota")?;
            }
            // La ruta como texto: pegarla en una terminal o en un dialogo de
            // abrir es exactamente lo que se espera.
            TipoEntrada::Archivo => {
                let r = ruta.context("la entrada de archivo no tiene ruta")?;
                pixpin_codec::copiar_texto(&r.to_string_lossy())
                    .context("no se pudo copiar la ruta")?;
            }
        }
        Ok(())
    }

    /// Asigna (o quita) el grupo de un pin y retiñe su sombra al momento.
    pub fn poner_grupo(&mut self, id: u64, color: Option<ColorGrupo>) -> Result<()> {
        self.almacen
            .borrow_mut()
            .poner_grupo(id, color)
            .context("no se pudo guardar el grupo")?;
        if let Some(pin) = self.vivos.get(&id) {
            pin.poner_color(color.map(rgb_de));
        }
        Ok(())
    }

    /// Cierra todos los pines de la pantalla, dejandolos en el almacen y
    /// apuntados para poder devolverlos uno a uno. Devuelve cuantos cerro.
    pub fn cerrar_todos(&mut self) -> usize {
        let ids: Vec<u64> = self.vivos.keys().copied().collect();
        for id in &ids {
            if let Some(g) = posicion_guardada(&self.almacen, *id) {
                self.reabrir.borrow_mut().push((*id, g));
            }
            if let Err(e) = self.almacen.borrow_mut().actualizar_pin(*id, None) {
                tracing::warn!(?e, id = *id, "no se pudo marcar el pin como cerrado");
            }
            self.vivos.remove(id);
            self.herramientas.remove(id);
            self.pizarras.remove(id);
        }
        // Los en vivo no tienen sitio que recordar: se cierran sin mas.
        let en_vivo = self.en_vivo.len();
        self.en_vivo.clear();
        ids.len() + en_vivo
    }

    /// Quita los pines de la pantalla SIN cerrarlos: el almacen los sigue
    /// dando por abiertos, asi que `mostrar_todos` los devuelve enteros.
    /// Es la diferencia con `cerrar_todos`, que si los cierra.
    pub fn ocultar_todos(&mut self) -> usize {
        let cuantos = self.vivos.len() + self.en_vivo.len();
        self.vivos.clear();
        // Vuelven del almacen al mostrarlos, con su documento guardado.
        self.herramientas.clear();
        self.pizarras.clear();
        for (pin, _) in self.en_vivo.values() {
            pin.esconder(true);
        }
        self.en_vivo_ocultos = !self.en_vivo.is_empty();
        cuantos
    }

    /// Devuelve a la pantalla todo lo que el almacen da por abierto, y los
    /// pines en vivo escondidos. Los grupos ocultos siguen ocultos: para eso
    /// estan.
    pub fn mostrar_todos(&mut self, disposicion: &DisposicionMonitores) -> usize {
        for (pin, _) in self.en_vivo.values() {
            pin.esconder(false);
        }
        let en_vivo = if self.en_vivo_ocultos {
            self.en_vivo.len()
        } else {
            0
        };
        self.en_vivo_ocultos = false;
        self.restaurar(disposicion) + en_vivo
    }

    /// Un solo comando para las dos cosas, como en el original (Ctrl+2): si
    /// hay algo en pantalla lo esconde, y si no, lo saca. Devuelve si
    /// escondio y cuantos pines movio.
    pub fn alternar_todos(&mut self, disposicion: &DisposicionMonitores) -> (bool, usize) {
        let en_vivo_a_la_vista = !self.en_vivo.is_empty() && !self.en_vivo_ocultos;
        if self.vivos.is_empty() && !en_vivo_a_la_vista {
            (false, self.mostrar_todos(disposicion))
        } else {
            (true, self.ocultar_todos())
        }
    }

    /// Se lleva el resultado de la ultima extraccion de paginas, para que
    /// el bucle lo avise por la bandeja. El gestor no tiene bandeja.
    pub fn tomar_paginas_extraidas(&mut self) -> Option<(u32, u32)> {
        self.paginas_extraidas.take()
    }

    /// Que TODOS los pines dejen pasar el clic, o que vuelvan a
    /// recogerlo. Devuelve si quedaron pasantes y cuantos cambiaron.
    ///
    /// Es la unica via de vuelta y por eso alterna en bloque: un pin
    /// pasante no puede abrir su propio menu del clic derecho, porque el
    /// clic pasa de largo. Si esto solo pudiera activarse, un descuido
    /// dejaria pines intocables para siempre.
    ///
    /// Manda la mayoria: con unos cuantos pasantes y otros no, los pone
    /// todos a lo contrario de lo que haya de mas. Asi el comando siempre
    /// hace algo visible, en vez de dejar la pantalla igual.
    pub fn alternar_paso_de_clics(&mut self) -> (bool, usize) {
        let pasantes = self.vivos.values().filter(|p| p.es_pasante()).count();
        let hacia = pasantes * 2 <= self.vivos.len();
        let mut cambiados = 0;
        for (id, pin) in &self.vivos {
            if pin.es_pasante() == hacia {
                continue;
            }
            pin.poner_pasante(hacia);
            cambiados += 1;
            // Se guarda uno a uno: el pin no avisa de esto por su cuenta,
            // porque el cambio no viene de su ventana sino de aqui.
            if let Some(mut g) = posicion_guardada(&self.almacen, *id) {
                g.pasante = hacia;
                if let Err(e) = self.almacen.borrow_mut().actualizar_pin(*id, Some(g)) {
                    tracing::warn!(?e, id = *id, "no se pudo guardar el paso de clics");
                }
            }
        }
        (hacia, cambiados)
    }

    /// Devuelve el ultimo pin cerrado a donde estaba. `false` si no queda
    /// ninguno por devolver o si su entrada ya no existe en el almacen.
    pub fn restaurar_ultimo_cerrado(&mut self, disposicion: &DisposicionMonitores) -> bool {
        loop {
            // El `pop` va en su propia sentencia para soltar el prestamo de
            // la pila antes de tocar el almacen y las ventanas: encadenarlo
            // en el `while let` lo mantendria vivo todo el cuerpo.
            let siguiente = self.reabrir.borrow_mut().pop();
            let Some((id, guardado)) = siguiente else {
                return false;
            };
            // Puede haberse eliminado del almacen despues de cerrarlo: eso
            // es definitivo y no se deshace, asi que se pasa al siguiente.
            let existe = self
                .almacen
                .borrow()
                .entradas()
                .iter()
                .any(|e| e.id == id && e.pin.is_none());
            if !existe {
                continue;
            }
            if let Err(e) = self.almacen.borrow_mut().actualizar_pin(id, Some(guardado)) {
                tracing::warn!(?e, id, "no se pudo devolver el pin al almacen");
                continue;
            }
            self.restaurar(disposicion);
            return self.vivos.contains_key(&id);
        }
    }

    pub fn abiertos(&self) -> usize {
        self.vivos.len() + self.en_vivo.len()
    }
}

/// Del punto entero del pin al punto en coma flotante del motor.
fn a_punto2(p: pixpin_geom::Punto) -> pixpin_motor2d::Punto2 {
    pixpin_motor2d::Punto2::nuevo(p.x as f32, p.y as f32)
}

/// El evento de anotacion de un pedido del puntero; `None` si no lo es.
/// Pasa los puntos de un evento del anotador de pixeles de la ventana del
/// pin a pixeles del contenido original, dividiendo por su escala.
fn a_documento(evento: EventoAnotador, (fx, fy): (f32, f32)) -> EventoAnotador {
    let d = |p: pixpin_motor2d::Punto2| {
        pixpin_motor2d::Punto2::nuevo(p.x / fx.max(1e-6), p.y / fy.max(1e-6))
    };
    match evento {
        EventoAnotador::Pulsar(p) => EventoAnotador::Pulsar(d(p)),
        EventoAnotador::Mover(p) => EventoAnotador::Mover(d(p)),
        EventoAnotador::Soltar(p) => EventoAnotador::Soltar(d(p)),
        EventoAnotador::Muestra { p, presion } => EventoAnotador::Muestra { p: d(p), presion },
        otro => otro,
    }
}

fn evento_de_puntero(cambio: CambioPin) -> Option<EventoAnotador> {
    match cambio {
        CambioPin::PunteroPulsado(p) => Some(EventoAnotador::Pulsar(a_punto2(p))),
        CambioPin::PunteroMovido(p) => Some(EventoAnotador::Mover(a_punto2(p))),
        CambioPin::PunteroSoltado(p) => Some(EventoAnotador::Soltar(a_punto2(p))),
        CambioPin::MuestraPuntero { x, y, presion } => Some(EventoAnotador::Muestra {
            p: pixpin_motor2d::Punto2::nuevo(x, y),
            presion,
        }),
        _ => None,
    }
}

/// Un paso al drenar la cola de pedidos de los pines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PasoPedido {
    /// Atender el pedido `n` como siempre (menu, rueda, teclas...).
    Atender(usize),
    /// Llevar el pedido `n` del puntero por la maquina de anotar, sin pintar.
    Anotar(usize),
    /// Pintar una vez lo anotado desde el ultimo repintado de ese pin.
    Repintar(u64),
}

/// El orden en que se drena la cola: los pedidos del puntero seguidos de un
/// mismo pin se anotan sin pintar y se pintan una sola vez al final de la
/// tanda (I1). Cualquier otro pedido, o un pin distinto, cierra la tanda
/// ANTES de atenderse, para que nada vea el pin sin su ultimo dibujo.
///
/// Pura: la cuenta de repintados se prueba sin ventanas.
fn planificar_pedidos(pedidos: &[(u64, CambioPin)]) -> Vec<PasoPedido> {
    let mut pasos = Vec::with_capacity(pedidos.len() + 1);
    let mut tanda: Option<u64> = None;
    for (n, (id, cambio)) in pedidos.iter().enumerate() {
        let es_puntero = evento_de_puntero(*cambio).is_some();
        if let Some(abierta) = tanda {
            if !es_puntero || abierta != *id {
                pasos.push(PasoPedido::Repintar(abierta));
                tanda = None;
            }
        }
        if es_puntero {
            pasos.push(PasoPedido::Anotar(n));
            tanda = Some(*id);
        } else {
            pasos.push(PasoPedido::Atender(n));
        }
    }
    if let Some(abierta) = tanda {
        pasos.push(PasoPedido::Repintar(abierta));
    }
    pasos
}

/// El tamano del recuadro gris cuando la imagen del pin no se puede leer:
/// el del pin guardado si se conoce, si no 800 x 600 (tabla de errores).
const RESERVA_LIENZO: (u32, u32) = (800, 600);

fn tamano_de_reserva(guardado: Option<PinGuardado>) -> (u32, u32) {
    match guardado {
        Some(g) if g.ancho > 0 && g.alto > 0 => (g.ancho, g.alto),
        _ => RESERVA_LIENZO,
    }
}

/// La escena con la que se abre el lienzo y si ya habia fichero. Un fichero
/// corrupto es un error: abrir el lienzo con una escena vacia y guardarla al
/// cerrar pisaria lo que el usuario tenia (tabla de errores).
pub(crate) fn escena_para_lienzo(
    ruta: &Path,
) -> Result<(Escena, bool), pixpin_motor2d::ErrorFormato> {
    let habia = ruta.is_file();
    let escena = pixpin_motor2d::cargar(ruta)?;
    Ok((escena, habia))
}

/// D146: una escena vacia sin fichero previo no crea uno; con fichero previo
/// se guarda aunque quede vacia, o lo borrado volveria a salir en el pin.
fn hay_que_guardar_lienzo(escena: &Escena, habia_fichero: bool) -> bool {
    habia_fichero || escena.cuantos_visibles() > 0
}

/// Guarda lo del lienzo si toca. Devuelve si escribio. `guardar` escribe a
/// un temporal y renombra: si falla, el fichero anterior queda intacto.
pub(crate) fn guardar_lienzo(
    ruta: &Path,
    escena: &Escena,
    habia_fichero: bool,
) -> Result<bool, pixpin_motor2d::ErrorFormato> {
    if !hay_que_guardar_lienzo(escena, habia_fichero) {
        return Ok(false);
    }
    pixpin_motor2d::guardar(ruta, escena)?;
    Ok(true)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_barra_del_pin_es_la_del_lienzo_sin_lo_apagado_ni_lo_que_el_pin_no_pinta() {
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        crate::dibujo::permitidas::fijar(pixpin_store::herramientas::Herramientas {
            apagadas: vec!["flecha-codos".into()],
        });
        let caja = caja_del_pin(area, 100);
        // Lo que se puede coger: lo suelto y lo de dentro de cada grupo.
        let mut b = caja.botones().to_vec();
        for g in pixpin_ui::GrupoBarra::TODOS {
            b.extend(caja.miembros(g));
        }
        // Mas que las once del anotador viejo: las del lienzo.
        for h in [
            Herramienta::Lazo,
            Herramienta::Rombo,
            Herramienta::Grafito,
            Herramienta::FlechaLibre,
            Herramienta::Marco,
        ] {
            assert!(b.contains(&BotonCaja::Elegir(h)), "{h:?}");
        }
        // Casos negativos: la apagada y la que el pin no sabe hacer; y el
        // grupo de medir, sin ninguna que el pin sepa, ni sale.
        assert!(!b.contains(&BotonCaja::Elegir(Herramienta::FlechaCodos)));
        assert!(!b.contains(&BotonCaja::Elegir(Herramienta::Lupa)));
        assert!(!b.contains(&BotonCaja::Elegir(Herramienta::Escalar)));
        assert!(!caja.botones().contains(&BotonCaja::Grupo(pixpin_ui::GrupoBarra::Medir)));
        assert!(!caja.botones().contains(&BotonCaja::Grupo(pixpin_ui::GrupoBarra::Sacar)));
        // Cabe en la pantalla: es una barra, no una columna de treinta.
        assert!(caja.marco.alto < 100 && caja.marco.ancho <= area.ancho);
        // Con un grupo abierto, la ventana de la paleta abarca sus hermanas;
        // cerrado, solo la barra.
        assert_eq!(rect_de_paleta(&caja), caja.marco);
        let abierta = caja.con_desplegado(Some(pixpin_ui::GrupoBarra::Formas));
        let r = rect_de_paleta(&abierta);
        let m = abierta.menu().expect("abierto").marco;
        assert!(r.abajo() >= m.abajo() && r.arriba() <= caja.marco.arriba(), "{r:?}");
        crate::dibujo::permitidas::fijar(Default::default());
    }

    #[test]
    fn la_rueda_sobre_el_pin_pasa_por_los_tres_grosores_del_lienzo() {
        use pixpin_motor2d::tinta::{GROSOR_FINO, GROSOR_GRUESO, GROSOR_MEDIO};
        assert_eq!(grosor_con_rueda(GROSOR_MEDIO, 120), GROSOR_GRUESO);
        assert_eq!(grosor_con_rueda(GROSOR_MEDIO, -120), GROSOR_FINO);
        // En los extremos no se sale.
        assert_eq!(grosor_con_rueda(GROSOR_GRUESO, 120), GROSOR_GRUESO);
        assert_eq!(grosor_con_rueda(GROSOR_FINO, -120), GROSOR_FINO);
    }
    use pixpin_motor2d::gesto::{EventoGesto, Gesto};
    use pixpin_motor2d::{ColorRgba, Elemento, EstiloTrazo, Figura, Punto2};

    #[test]
    fn el_raton_se_divide_por_la_escala_del_pin_antes_de_dibujar() {
        use pixpin_motor2d::Punto2;
        // Pin al 200 %: el punto (100, 60) de la ventana es el (50, 30) del
        // original, que es donde se guarda y desde donde se pinta por 2.
        let e = a_documento(
            EventoAnotador::Mover(Punto2::nuevo(100.0, 60.0)),
            (2.0, 2.0),
        );
        assert_eq!(e, EventoAnotador::Mover(Punto2::nuevo(50.0, 30.0)));
        let m = a_documento(
            EventoAnotador::Muestra {
                p: Punto2::nuevo(30.0, 30.0),
                presion: Some(0.5),
            },
            (0.5, 1.5),
        );
        assert_eq!(
            m,
            EventoAnotador::Muestra {
                p: Punto2::nuevo(60.0, 20.0),
                presion: Some(0.5)
            }
        );
        // Caso negativo: lo que no es un punto no se toca.
        assert_eq!(
            a_documento(EventoAnotador::Rueda(120), (2.0, 2.0)),
            EventoAnotador::Rueda(120)
        );
    }

    #[test]
    fn una_tanda_de_muestras_y_su_movimiento_se_pinta_una_sola_vez() {
        // Lo que deja un WM_MOUSEMOVE rapido: 63 puntos recuperados y el
        // movimiento. Antes eran 64 repintados del pin; ahora 64 anotaciones
        // y un repintado al final.
        let mut pedidos: Vec<(u64, CambioPin)> = (0..63)
            .map(|k| {
                (
                    7,
                    CambioPin::MuestraPuntero {
                        x: k as f32,
                        y: 0.0,
                        presion: None,
                    },
                )
            })
            .collect();
        pedidos.push((7, CambioPin::PunteroMovido(Punto { x: 63, y: 0 })));
        let pasos = planificar_pedidos(&pedidos);
        let anotar = pasos
            .iter()
            .filter(|p| matches!(p, PasoPedido::Anotar(_)))
            .count();
        assert_eq!(anotar, 64);
        let repintados: Vec<&PasoPedido> = pasos
            .iter()
            .filter(|p| matches!(p, PasoPedido::Repintar(_)))
            .collect();
        assert_eq!(repintados, vec![&PasoPedido::Repintar(7)]);
        assert_eq!(pasos.last(), Some(&PasoPedido::Repintar(7)));
    }

    #[test]
    fn otro_pedido_o_otro_pin_cierra_la_tanda_antes_de_atenderse() {
        // Caso negativo: un pedido que no es del puntero no puede ver el pin
        // sin el dibujo que lleva delante en la cola.
        let m = |x: f32| CambioPin::MuestraPuntero {
            x,
            y: 0.0,
            presion: None,
        };
        let pedidos = [
            (1, m(0.0)),
            (1, m(1.0)),
            (1, CambioPin::EscapeAnotando),
            (1, m(2.0)),
            (2, m(3.0)),
        ];
        assert_eq!(
            planificar_pedidos(&pedidos),
            vec![
                PasoPedido::Anotar(0),
                PasoPedido::Anotar(1),
                PasoPedido::Repintar(1),
                PasoPedido::Atender(2),
                PasoPedido::Anotar(3),
                PasoPedido::Repintar(1),
                PasoPedido::Anotar(4),
                PasoPedido::Repintar(2),
            ]
        );
    }

    fn trazo(x1: f32, y1: f32, x2: f32, y2: f32) -> Elemento {
        Elemento {
            id: 0,
            estilo_relleno: Default::default(),
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(x1, y1), Punto2::nuevo(x2, y2)],
            },
            x: x1,
            y: y1,
            ancho: x2 - x1,
            alto: y2 - y1,
            angulo: 0.0,
            trazo: ColorRgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            relleno: None,
            grosor: 0.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 0.0,
            opacidad: 1.0,
            semilla: 1,
            version: 1,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    #[test]
    fn el_lienzo_se_recorta_a_lo_dibujado_y_no_al_origen() {
        // El caso del usuario: su dibujo empieza en (165, 228) y acaba en
        // (1178, 2148). Midiendo desde el origen salia un pin de 1202 x 2172
        // —mas alto que su pantalla—, casi todo en blanco, y al encogerlo
        // para que cupiera el dibujo se veia diminuto y arrinconado.
        let (imagen, dx, dy) = lienzo_en_blanco(&[trazo(165.0, 228.0, 1178.0, 2148.0)], BLANCO);
        let m = LIENZO_MARGEN as u32;
        assert_eq!(
            imagen.ancho,
            1013 + 2 * m,
            "sobra lienzo en blanco a un lado"
        );
        assert_eq!(imagen.alto, 1920 + 2 * m, "sobra lienzo en blanco arriba");
        // Y el desplazamiento tiene que dejar el dibujo justo tras el margen.
        assert!((165.0 + dx - LIENZO_MARGEN).abs() < 0.01, "dx {dx}");
        assert!((228.0 + dy - LIENZO_MARGEN).abs() < 0.01, "dy {dy}");
    }

    #[test]
    fn lo_dibujado_cabe_entero_dentro_del_lienzo_recortado() {
        // Lo que de verdad importa: despues de mover, ni un punto se sale.
        // Si esto falla, al usuario se le pierde parte del dibujo.
        let mut e = trazo(165.0, 228.0, 1178.0, 2148.0);
        let (imagen, dx, dy) = lienzo_en_blanco(std::slice::from_ref(&e), BLANCO);
        e.mover(dx, dy);
        let (x1, y1, x2, y2) = e.caja();
        assert!(x1 >= 0.0 && y1 >= 0.0, "se sale por arriba: {x1} {y1}");
        assert!(
            x2 <= imagen.ancho as f32 && y2 <= imagen.alto as f32,
            "se sale por abajo: {x2} {y2} en {}x{}",
            imagen.ancho,
            imagen.alto
        );
    }

    const BLANCO: pixpin_motor2d::ColorRgba = pixpin_motor2d::escena::FONDO_DE_FABRICA;

    #[test]
    fn la_hoja_del_pin_es_del_papel_del_lienzo() {
        // Una hoja del movil sobre «Crema» (`#fdf6e3`) se pinea crema.
        let json = r##"{"type":"excalidraw","elements":[],
            "appState":{"viewBackgroundColor":"#fdf6e3"}}"##;
        let lienzo = pixpin_motor2d::excalidraw::leer(json).unwrap();
        let papel = pixpin_motor2d::excalidraw::fondo(&lienzo);
        let (imagen, _, _) = lienzo_en_blanco(&[], papel);
        assert!(imagen.pixeles.chunks_exact(4).all(|p| p == [253, 246, 227, 255]));
        // Caso negativo: sin papel dicho, blanco como siempre.
        let (imagen, _, _) = lienzo_en_blanco(&[], BLANCO);
        assert!(imagen.pixeles.iter().all(|&v| v == 255));
    }

    #[test]
    fn una_hoja_en_blanco_da_el_lienzo_minimo_sin_moverse() {
        // Caso negativo: sin nada dibujado no hay esquina de la que partir, y
        // un lienzo de cero no se veria.
        let (imagen, dx, dy) = lienzo_en_blanco(&[], BLANCO);
        assert_eq!(imagen.ancho, LIENZO_MINIMO);
        assert_eq!(imagen.alto, LIENZO_MINIMO);
        assert!((dx - LIENZO_MARGEN).abs() < 0.01 && (dy - LIENZO_MARGEN).abs() < 0.01);
    }

    #[test]
    fn un_dibujo_en_coordenadas_negativas_tambien_entra() {
        // En un lienzo infinito el (0,0) es un sitio cualquiera, y se dibuja
        // igual a su izquierda. Midiendo desde el origen esto daba un lienzo
        // minimo con el dibujo entero fuera: invisible.
        let mut e = trazo(-900.0, -700.0, -400.0, -200.0);
        let (imagen, dx, dy) = lienzo_en_blanco(std::slice::from_ref(&e), BLANCO);
        e.mover(dx, dy);
        let (x1, y1, x2, y2) = e.caja();
        assert!(x1 >= 0.0 && y1 >= 0.0, "quedo fuera: {x1} {y1}");
        assert!(x2 <= imagen.ancho as f32 && y2 <= imagen.alto as f32);
    }

    fn dir_de_prueba(etiqueta: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-lienzo-{etiqueta}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn una_escena_vacia_sin_fichero_previo_no_crea_fichero() {
        let ruta = dir_de_prueba("vacia-sin-previo").join("objeto.pixpin2d");
        let (escena, habia) = escena_para_lienzo(&ruta).unwrap();
        assert!(!habia);
        assert!(!guardar_lienzo(&ruta, &escena, habia).unwrap());
        assert!(
            !ruta.exists(),
            "abrir y cerrar sin dibujar no deja ficheros"
        );
    }

    #[test]
    fn con_fichero_previo_se_guarda_aunque_quede_vacia() {
        // Borrar todo en el lienzo tiene que borrarlo tambien del pin.
        let ruta = dir_de_prueba("vacia-con-previo").join("objeto.pixpin2d");
        let mut antes = Escena::nueva();
        antes.anadir(trazo(1.0, 2.0, 30.0, 40.0));
        pixpin_motor2d::guardar(&ruta, &antes).unwrap();
        let (_, habia) = escena_para_lienzo(&ruta).unwrap();
        assert!(habia);
        assert!(guardar_lienzo(&ruta, &Escena::nueva(), habia).unwrap());
        assert_eq!(pixpin_motor2d::cargar(&ruta).unwrap().cuantos_visibles(), 0);
    }

    #[test]
    fn un_fichero_corrupto_no_abre_el_lienzo_ni_se_sobrescribe() {
        let ruta = dir_de_prueba("corrupto").join("objeto.pixpin2d");
        std::fs::write(&ruta, "{ esto no es un dibujo").unwrap();
        assert!(escena_para_lienzo(&ruta).is_err());
        assert_eq!(
            std::fs::read_to_string(&ruta).unwrap(),
            "{ esto no es un dibujo"
        );
    }

    #[test]
    fn si_guardar_falla_el_fichero_anterior_queda_intacto() {
        let d = dir_de_prueba("guardar-falla");
        let ruta = d.join("objeto.pixpin2d");
        let mut antes = Escena::nueva();
        antes.anadir(trazo(1.0, 2.0, 30.0, 40.0));
        pixpin_motor2d::guardar(&ruta, &antes).unwrap();
        let original = std::fs::read_to_string(&ruta).unwrap();
        // El temporal de `guardar` no se puede escribir: hay un directorio
        // con su nombre.
        std::fs::create_dir_all(ruta.with_extension("pixpin2d.tmp")).unwrap();
        let mut nueva = Escena::nueva();
        nueva.anadir(trazo(5.0, 5.0, 9.0, 9.0));
        assert!(guardar_lienzo(&ruta, &nueva, true).is_err());
        assert_eq!(std::fs::read_to_string(&ruta).unwrap(), original);
    }

    #[test]
    fn lo_dibujado_en_el_lienzo_vuelve_en_las_mismas_coordenadas() {
        // Ida y vuelta (spec §6): la escena del pin, un trazo nuevo con el
        // gesto (fuera de la imagen incluso, D147), guardar y recargar.
        let ruta = dir_de_prueba("ida-y-vuelta").join("objeto.pixpin2d");
        let mut previa = Escena::nueva();
        previa.anadir(trazo(10.0, 10.0, 50.0, 50.0));
        pixpin_motor2d::guardar(&ruta, &previa).unwrap();

        let (mut escena, habia) = escena_para_lienzo(&ruta).unwrap();
        let mut gesto = Gesto::nuevo();
        gesto.evento(
            EventoGesto::Pulsar {
                p: Punto2::nuevo(-50.0, 10.0),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        gesto.evento(
            EventoGesto::Mover {
                p: Punto2::nuevo(-20.0, 40.0),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        gesto.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(-20.0, 40.0),
            },
            &mut escena,
            1.0,
        );
        escena.compactar();
        assert_eq!(escena.cuantos_visibles(), 2);
        assert!(guardar_lienzo(&ruta, &escena, habia).unwrap());

        let vuelta = pixpin_motor2d::cargar(&ruta).unwrap();
        let antes: Vec<Elemento> = escena.visibles().cloned().collect();
        let despues: Vec<Elemento> = vuelta.visibles().cloned().collect();
        assert_eq!(antes, despues);
    }

    #[test]
    fn sin_imagen_legible_el_recuadro_mide_lo_del_pin_o_800_por_600() {
        let g = Pines::guardado_desde(
            Rect {
                x: 0,
                y: 0,
                ancho: 320,
                alto: 200,
            },
            100,
            100,
        );
        assert_eq!(tamano_de_reserva(Some(g)), (320, 200));
        assert_eq!(tamano_de_reserva(None), (800, 600));
        let cero = PinGuardado { ancho: 0, ..g };
        assert_eq!(tamano_de_reserva(Some(cero)), (800, 600));
    }
}
