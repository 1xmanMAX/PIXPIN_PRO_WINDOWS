//! La imagen del pin como fondo fijo del lienzo (D132-D143).
//!
//! No es un elemento de la escena (D133): no pasa por `Escena` ni por la
//! rejilla, asi que el gesto no puede seleccionarla, moverla ni borrarla.
//! Vive en el mundo en (0,0)-(ancho, alto), las coordenadas en las que ya
//! estan las anotaciones del pin.
//!
//! **Una pagina de PDF como fondo** ([`Fuente::Pdf`]) es lo mismo con dos
//! diferencias, las dos copiadas del movil (`DrawEditorActivity.abrirPaginaDePdf`
//! y `MosaicoDePdf`):
//!
//! - **Mide [`ANCHO_PAPEL_PDF`] unidades de ancho**, no lo que midan sus
//!   pixeles: es el `PdfDoc.PAGE_WIDTH` con el que el movil coloca la pagina
//!   en (0,0). Con otro numero, lo que se dibuja en el movil caeria corrido
//!   en el PC y al reves.
//! - **Se pinta en otro hilo y se afina al acercarse.** Abrir un PDF de cien
//!   paginas y pintar una son decenas de milisegundos que el hilo de la
//!   ventana no puede pagar; y una sola imagen de la pagina se ve borrosa en
//!   cuanto se amplia. Asi que el lienzo abre con la vista previa que ya
//!   tenia la galeria (o en blanco), el hilo manda la pagina entera y, al
//!   acercarse, **teselas** de la zona visible a la densidad del aumento.
//!   Mientras llegan se ve lo anterior estirado.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};

use pixpin_codec::ImagenRgba;
use pixpin_motor2d::camara::Camara;
use pixpin_render::{Interpolacion, MotorRender, Pintor, RectF};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

/// **Lo que mide de ancho una pagina de PDF en el lienzo**, en unidades del
/// dibujo: `PdfDoc.PAGE_WIDTH` del movil, que la pinta a ese ancho y la pone
/// en (0,0) (`abrirPaginaDePdf` → `encajarLaPagina`). Todo lo dibujado sobre
/// una pagina esta en estas coordenadas: el PC tiene que usar el mismo
/// numero o los trazos no caen donde se hicieron.
pub const ANCHO_PAPEL_PDF: f32 = 1400.0;

/// Lado de cada tesela, en pixeles (`MosaicoDePdf.LADO_EN_PIXELES`).
pub const LADO_TESELA: u32 = 512;

/// Lo mas fino que se afina, en pixeles por unidad del dibujo
/// (`MosaicoDePdf.LO_MAS_FINO`). Pasado esto se estira: a dieciseis ya se
/// lee cualquier cota de un plano.
pub const DENSIDAD_MAXIMA: u32 = 16;

/// **Cuanta memoria pueden ocupar las teselas**, en bytes de pixeles. Es el
/// `PRESUPUESTO` del movil: sesenta y cuatro teselas de 512, mas que de sobra
/// para una pantalla entera a cualquier aumento. Lo que pase se suelta,
/// empezando por lo que hace mas que no se pinta.
pub const TOPE_TESELAS_BYTES: usize = 64 * 1024 * 1024;

/// De donde sale el fondo del lienzo.
#[derive(Clone, Debug)]
pub enum Fuente {
    /// Una imagen ya en memoria: la de un pin o una foto.
    Imagen(ImagenRgba),
    /// Una pagina de un PDF, que se pinta en su hilo y se afina al acercarse.
    Pdf(PaginaPdf),
}

impl From<ImagenRgba> for Fuente {
    fn from(i: ImagenRgba) -> Self {
        Fuente::Imagen(i)
    }
}

/// Una pagina de PDF como fondo.
#[derive(Clone, Debug)]
pub struct PaginaPdf {
    pub pdf: PathBuf,
    /// Desde 0, como `Hoja.pagina`.
    pub pagina: u32,
    /// Lo que se ve mientras el hilo pinta la de verdad: el PNG de la vista
    /// previa que ya pinto la galeria. Va la ruta y no los pixeles porque
    /// descomprimirlo tambien lo hace el hilo: en el de la ventana eran cien
    /// milisegundos (medido, en depuracion).
    pub provisional: Option<PathBuf>,
    /// Alto/ancho de la hoja, si se sabe (la cabecera de ese PNG): da el
    /// alto desde el primer fotograma. Sin ella se supone un A4 vertical
    /// hasta que llegue la pagina.
    pub proporcion: Option<f32>,
}

/// Proporcion alto/ancho que se supone sin vista previa: un A4 vertical,
/// que es lo mas corriente. Se corrige en cuanto llega la pagina.
const PROPORCION_SUPUESTA: f32 = 1.414;

/// Una tesela: su densidad (pixeles por unidad) y su columna y fila.
pub type ClaveTesela = (u32, u32, u32);

/// **Cuantos pixeles por unidad pide este aumento**: la potencia de dos que
/// llega a `escala` (pixeles fisicos por unidad del mundo), hasta
/// [`DENSIDAD_MAXIMA`]. Uno es la pagina entera, que ya esta; por potencias
/// de dos para que un paso de rueda no pida otra tanda de teselas.
pub fn densidad_para(escala: f32) -> u32 {
    if !escala.is_finite() || escala <= 1.0 {
        return 1;
    }
    (escala.ceil() as u32)
        .next_power_of_two()
        .min(DENSIDAD_MAXIMA)
}

/// La caja del mundo de una tesela, recortada al papel. `None` si cae fuera.
pub fn caja_de_tesela(
    (densidad, tx, ty): ClaveTesela,
    ancho: f32,
    alto: f32,
) -> Option<(f32, f32, f32, f32)> {
    let lado = LADO_TESELA as f32 / densidad.max(1) as f32;
    let (x0, y0) = (tx as f32 * lado, ty as f32 * lado);
    if x0 >= ancho || y0 >= alto {
        return None;
    }
    Some((x0, y0, (x0 + lado).min(ancho), (y0 + lado).min(alto)))
}

/// **Las teselas que hacen falta para ver `vista` a este aumento**, de la
/// del centro hacia fuera (lo que se mira llega primero), y su densidad.
/// Ninguna si basta la pagina entera o si la vista no toca el papel. Nunca
/// mas de las que caben en [`TOPE_TESELAS_BYTES`]: una vista enorme no puede
/// pedir mas de lo que luego se deja guardar.
pub fn teselas_para(
    escala: f32,
    vista: (f32, f32, f32, f32),
    ancho: f32,
    alto: f32,
) -> (u32, Vec<ClaveTesela>) {
    let d = densidad_para(escala);
    if d == 1 || !se_ve(vista, ancho, alto) {
        return (d, Vec::new());
    }
    let lado = LADO_TESELA as f32 / d as f32;
    let x0 = (vista.0.max(0.0) / lado).floor() as u32;
    let y0 = (vista.1.max(0.0) / lado).floor() as u32;
    let x1 = ((vista.2.min(ancho) / lado).ceil() as u32).max(x0 + 1);
    let y1 = ((vista.3.min(alto) / lado).ceil() as u32).max(y0 + 1);
    let (cx, cy) = ((vista.0 + vista.2) / 2.0, (vista.1 + vista.3) / 2.0);
    let mut v: Vec<ClaveTesela> = (y0..y1)
        .flat_map(|ty| (x0..x1).map(move |tx| (d, tx, ty)))
        .filter(|c| caja_de_tesela(*c, ancho, alto).is_some())
        .collect();
    v.sort_by(|a, b| {
        let dist = |c: &ClaveTesela| {
            let (x0, y0, x1, y1) = caja_de_tesela(*c, ancho, alto).unwrap_or_default();
            ((x0 + x1) / 2.0 - cx).powi(2) + ((y0 + y1) / 2.0 - cy).powi(2)
        };
        dist(a).total_cmp(&dist(b))
    });
    let cabe = TOPE_TESELAS_BYTES / (LADO_TESELA as usize * LADO_TESELA as usize * 4);
    v.truncate(cabe);
    (d, v)
}

/// **Que teselas soltar** para quedar bajo `tope` bytes: las que hace mas que
/// no se pintan (`usada` mas bajo). Cada entrada es (clave, bytes, usada).
pub fn a_soltar(teselas: &[(ClaveTesela, usize, u64)], tope: usize) -> Vec<ClaveTesela> {
    let mut total: usize = teselas.iter().map(|t| t.1).sum();
    if total <= tope {
        return Vec::new();
    }
    let mut orden: Vec<&(ClaveTesela, usize, u64)> = teselas.iter().collect();
    orden.sort_by_key(|t| t.2);
    let mut fuera = Vec::new();
    for t in orden {
        if total <= tope {
            break;
        }
        total -= t.1;
        fuera.push(t.0);
    }
    fuera
}

/// Margen alrededor de la imagen al abrir, en pixeles logicos (D135).
pub const MARGEN_ENCUADRE: f32 = 48.0;

/// La camara (logica) con la que se abre el lienzo: la imagen centrada, a
/// zoom 1 si cabe con margen y, si no, al zoom que la hace caber (D135).
pub fn encuadre_inicial(
    ancho: f32,
    alto: f32,
    area_ancho_px: f32,
    area_alto_px: f32,
    escala_por_cien: u32,
) -> Camara {
    let escala = crate::navegacion::escala_de(escala_por_cien);
    let (w, h) = (area_ancho_px / escala, area_alto_px / escala);
    let cabe = ancho + 2.0 * MARGEN_ENCUADRE <= w && alto + 2.0 * MARGEN_ENCUADRE <= h;
    if cabe {
        return Camara {
            x: ancho / 2.0 - w / 2.0,
            y: alto / 2.0 - h / 2.0,
            zoom: 1.0,
        };
    }
    Camara::encajar((0.0, 0.0, ancho, alto), w, h, MARGEN_ENCUADRE)
}

/// **La camara con la que se abre un lienzo con dibujo**: todo lo dibujado
/// a la vista y centrado, con el mismo criterio que la imagen de fondo (zoom
/// 1 si cabe con margen; si no, el que lo hace caber). Abrir en el origen
/// dejaba al usuario mirando un trozo vacio cuando el dibujo estaba en otra
/// parte. `caja` es `(x0, y0, x1, y1)` en el mundo.
pub fn encuadre_de_contenido(
    caja: (f32, f32, f32, f32),
    area_ancho_px: f32,
    area_alto_px: f32,
    escala_por_cien: u32,
) -> Camara {
    let (x0, y0, x1, y1) = caja;
    let mut c = encuadre_inicial(
        (x1 - x0).max(1.0),
        (y1 - y0).max(1.0),
        area_ancho_px,
        area_alto_px,
        escala_por_cien,
    );
    // `encuadre_inicial` encuadra una caja con esquina en el origen: se
    // corre hasta donde de verdad esta el dibujo.
    c.x += x0;
    c.y += y0;
    c
}

/// Como se muestrea la imagen segun cuantos pixeles fisicos ocupa cada
/// pixel suyo (D141): al 100 % exacto o muy ampliada, pixeles tal cual (una
/// captura se lee nitida); reducida, cubica (sin dientes); entre medias,
/// lineal.
pub fn modo_nitidez(zoom_efectivo: f32) -> Interpolacion {
    if zoom_efectivo == 1.0 || zoom_efectivo >= 3.0 {
        Interpolacion::Vecino
    } else if zoom_efectivo < 1.0 {
        Interpolacion::Cubica
    } else {
        Interpolacion::Lineal
    }
}

/// El tamano al que hay que subir una imagen para que su lado mayor no pase
/// de `maximo`, o `None` si ya cabe (D139).
pub fn lado_de_subida(ancho: u32, alto: u32, maximo: u32) -> Option<(u32, u32)> {
    let mayor = ancho.max(alto);
    if maximo == 0 || mayor <= maximo {
        return None;
    }
    let f = maximo as f64 / mayor as f64;
    Some((
        ((ancho as f64 * f).round() as u32).clamp(1, maximo),
        ((alto as f64 * f).round() as u32).clamp(1, maximo),
    ))
}

/// Si la imagen (0,0)-(ancho, alto) toca la caja del mundo que se ve (D142).
pub fn se_ve(vista: (f32, f32, f32, f32), ancho: f32, alto: f32) -> bool {
    vista.0 <= ancho && vista.2 >= 0.0 && vista.1 <= alto && vista.3 >= 0.0
}

/// El recuadro gris que sustituye a una imagen que no se pudo leer. Nunca de
/// cero pixeles: un bitmap vacio no se puede subir. Lo usa `Pines` cuando la
/// imagen de un pin no se puede leer al abrirlo en el lienzo.
pub fn recuadro_gris(ancho: u32, alto: u32) -> ImagenRgba {
    let (ancho, alto) = (ancho.max(1), alto.max(1));
    ImagenRgba {
        ancho,
        alto,
        pixeles: [200u8, 200, 200, 255].repeat(ancho as usize * alto as usize),
    }
}

/// La imagen de fondo mientras el lienzo esta abierto.
///
/// Guarda los pixeles (ya reducidos si hizo falta) ademas del bitmap: con el
/// dispositivo perdido hay que volver a subirlo, igual que la cache de tinta
/// se rehace. Todo se suelta al cerrar el lienzo (D143).
pub struct FondoLienzo {
    /// Tamano en el mundo: el de la imagen original, o el del papel de una
    /// pagina de PDF ([`ANCHO_PAPEL_PDF`] de ancho). Es independiente de los
    /// pixeles que haya subidos: lo dibujado encima no se mueve cuando llega
    /// una resolucion distinta.
    ancho: f32,
    alto: f32,
    imagen: ImagenRgba,
    bitmap: Option<ID2D1Bitmap1>,
    /// Subir fallo: no se reintenta en cada fotograma hasta `soltar`.
    fallo: bool,
    /// El tope de la GPU, para reducir lo que llegue del hilo del PDF.
    lado_maximo: u32,
    /// Solo con una pagina de PDF: su hilo y sus teselas.
    nitidez: Option<Nitidez>,
}

/// Lo que se ve de una tesela que ya llego.
struct Tesela {
    caja: (f32, f32, f32, f32),
    /// Los pixeles hasta que se suben; luego se sueltan (la GPU ya los tiene).
    imagen: Option<ImagenRgba>,
    bitmap: Option<ID2D1Bitmap1>,
    bytes: usize,
    /// El ultimo fotograma en que se pinto, para soltar las mas viejas.
    usada: Cell<u64>,
}

/// Lo que va del hilo del PDF a la ventana.
enum Llegada {
    /// La vista previa, mientras llega la pagina.
    Provisional(ImagenRgba),
    /// La pagina entera, a un pixel por unidad.
    Base(ImagenRgba),
    Tesela(ClaveTesela, ImagenRgba),
}

/// Lo que comparten la ventana y el hilo del PDF.
struct Compartido {
    /// Las teselas que faltan, en el orden en que se quieren. La ventana la
    /// reescribe entera cuando cambia lo que se mira: lo que se dejo de ver
    /// no se pinta.
    pedido: Mutex<Vec<ClaveTesela>>,
    cv: Condvar,
    /// Las que la ventana ya tiene (o estan de camino): el hilo no las
    /// repite. La ventana quita las que suelta.
    tenidas: Mutex<HashSet<ClaveTesela>>,
    fin: AtomicBool,
    novedades: AtomicBool,
    ventana: AtomicIsize,
}

impl Compartido {
    fn avisar(&self) {
        self.novedades.store(true, Ordering::Release);
        let h = self.ventana.load(Ordering::Relaxed);
        if h != 0 {
            pixpin_shell::overlay::despertar(h);
        }
    }
}

struct Nitidez {
    compartido: Arc<Compartido>,
    llegadas: mpsc::Receiver<Llegada>,
    teselas: HashMap<ClaveTesela, Tesela>,
    /// El ultimo pedido mandado: si no cambia, no se toca el candado.
    ultimo: RefCell<Vec<ClaveTesela>>,
    reloj: Cell<u64>,
    /// Ya llego la pagina entera de verdad (no la provisional).
    base: bool,
}

impl Drop for Nitidez {
    fn drop(&mut self) {
        // El hilo se va solo en cuanto lo ve; no se le espera: cerrar el
        // lienzo no puede quedarse colgado de una pagina a medio pintar.
        self.compartido.fin.store(true, Ordering::Release);
        self.compartido.cv.notify_all();
    }
}

/// Como va el fondo a este aumento: lo que devuelve
/// [`FondoLienzo::para_escala`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estado {
    /// Pixeles por unidad que pide el aumento (1 = basta la pagina entera).
    pub densidad: u32,
    /// Teselas de esa densidad que ya estan para la vista.
    pub listas: usize,
    /// Las que se han pedido y aun no llegan.
    pub faltan: usize,
}

impl FondoLienzo {
    pub fn nuevo(fuente: impl Into<Fuente>, lado_maximo: u32) -> Self {
        match fuente.into() {
            Fuente::Imagen(imagen) => Self::de_imagen(imagen, lado_maximo),
            Fuente::Pdf(p) => Self::de_pdf(p, lado_maximo),
        }
    }

    fn de_imagen(imagen: ImagenRgba, lado_maximo: u32) -> Self {
        let (ancho, alto) = (imagen.ancho as f32, imagen.alto as f32);
        Self {
            ancho,
            alto,
            imagen: reducir(imagen, lado_maximo),
            bitmap: None,
            fallo: false,
            lado_maximo,
            nitidez: None,
        }
    }

    /// Una pagina de PDF. **No abre el PDF aqui**: lo abre su hilo, que
    /// manda la pagina entera y luego las teselas que se pidan. Aqui solo se
    /// pone la vista previa, si la hay, para no abrir en blanco.
    fn de_pdf(p: PaginaPdf, lado_maximo: u32) -> Self {
        let ancho = ANCHO_PAPEL_PDF;
        let proporcion = p
            .proporcion
            .filter(|r| r.is_finite() && *r > 0.0)
            .unwrap_or(PROPORCION_SUPUESTA);
        let alto = ancho * proporcion;
        let imagen = ImagenRgba {
            ancho: 0,
            alto: 0,
            pixeles: Vec::new(),
        };
        let compartido = Arc::new(Compartido {
            pedido: Mutex::new(Vec::new()),
            cv: Condvar::new(),
            tenidas: Mutex::new(HashSet::new()),
            fin: AtomicBool::new(false),
            novedades: AtomicBool::new(false),
            ventana: AtomicIsize::new(0),
        });
        let (tx, rx) = mpsc::channel();
        let c = Arc::clone(&compartido);
        let lanzado = std::thread::Builder::new()
            .name("fondo-pdf".into())
            .spawn(move || hilo_del_pdf(p, c, tx));
        if let Err(e) = lanzado {
            tracing::warn!(?e, "sin hilo para la pagina del PDF; se queda la vista previa");
        }
        Self {
            ancho,
            alto,
            imagen,
            bitmap: None,
            fallo: false,
            lado_maximo,
            nitidez: Some(Nitidez {
                compartido,
                llegadas: rx,
                teselas: HashMap::new(),
                ultimo: RefCell::new(Vec::new()),
                reloj: Cell::new(0),
                base: false,
            }),
        }
    }

    pub fn ancho(&self) -> f32 {
        self.ancho
    }

    pub fn alto(&self) -> f32 {
        self.alto
    }

    /// Los pixeles del papel, para exportarlo con lo dibujado encima. `None`
    /// si no se pudo reducir y el lienzo va sin fondo.
    pub fn imagen(&self) -> Option<&ImagenRgba> {
        (self.imagen.ancho > 0 && self.imagen.alto > 0).then_some(&self.imagen)
    }

    /// Cuantos pixeles se suben de verdad. Solo lo miran las pruebas, para
    /// comprobar la reduccion a la GPU sin abrir una ventana.
    #[cfg(test)]
    pub fn tamano_subido(&self) -> (u32, u32) {
        (self.imagen.ancho, self.imagen.alto)
    }

    /// A que ventana despertar cuando el hilo del PDF traiga algo. Sin esto
    /// la resolucion nueva esperaria a que se moviera el raton.
    pub fn avisar_a(&self, hwnd: isize) {
        if let Some(n) = &self.nitidez {
            n.compartido.ventana.store(hwnd, Ordering::Relaxed);
        }
    }

    /// **Si llego una resolucion nueva** desde la ultima vez. Recoge lo que
    /// trajo el hilo (sin subir nada: eso es de `asegurar`). Quien pinta con
    /// una capa congelada tiene que rehacerla al oir que si: la pagina de
    /// antes esta cocida dentro.
    pub fn hay_novedades(&mut self) -> bool {
        let Some(n) = &self.nitidez else {
            return false;
        };
        let avisado = n.compartido.novedades.swap(false, Ordering::AcqRel);
        self.recoger() || avisado
    }

    /// Mete en su sitio lo que haya llegado. `true` si habia algo.
    fn recoger(&mut self) -> bool {
        let Some(n) = self.nitidez.as_mut() else {
            return false;
        };
        let mut algo = false;
        while let Ok(l) = n.llegadas.try_recv() {
            algo = true;
            match l {
                // Solo si la de verdad no llego antes (no deberia: va
                // primero), y sin tocar el alto, que ya salio de su cabecera.
                Llegada::Provisional(img) => {
                    if !n.base {
                        self.imagen = reducir(img, self.lado_maximo);
                        self.bitmap = None;
                        self.fallo = false;
                    }
                }
                Llegada::Base(img) => {
                    // La proporcion de verdad manda sobre la supuesta; el
                    // ancho no se toca, que es el que fija lo dibujado.
                    self.alto = self.ancho * img.alto as f32 / img.ancho.max(1) as f32;
                    self.imagen = reducir(img, self.lado_maximo);
                    self.bitmap = None;
                    self.fallo = false;
                    n.base = true;
                }
                Llegada::Tesela(clave, img) => {
                    let Some(caja) = caja_de_tesela(clave, self.ancho, self.alto) else {
                        continue;
                    };
                    let bytes = img.pixeles.len();
                    n.teselas.insert(
                        clave,
                        Tesela {
                            caja,
                            imagen: Some(img),
                            bitmap: None,
                            bytes,
                            usada: Cell::new(n.reloj.get()),
                        },
                    );
                }
            }
        }
        algo
    }

    /// **La resolucion que pide este aumento, y como va.** Pide al hilo las
    /// teselas de `vista` que falten (solo si cambio lo que falta: se llama
    /// en cada fotograma). `escala` son pixeles fisicos por unidad del mundo
    /// (el zoom de la camara efectiva). Sin PDF, siempre densidad 1.
    pub fn para_escala(&self, escala: f32, vista: (f32, f32, f32, f32)) -> Estado {
        let Some(n) = &self.nitidez else {
            return Estado {
                densidad: 1,
                listas: 0,
                faltan: 0,
            };
        };
        let (densidad, quiero) = teselas_para(escala, vista, self.ancho, self.alto);
        let faltan: Vec<ClaveTesela> = quiero
            .iter()
            .copied()
            .filter(|c| !n.teselas.contains_key(c))
            .collect();
        if *n.ultimo.borrow() != faltan {
            if let Ok(mut p) = n.compartido.pedido.lock() {
                p.clone_from(&faltan);
            }
            n.compartido.cv.notify_all();
            *n.ultimo.borrow_mut() = faltan.clone();
        }
        Estado {
            densidad,
            listas: quiero.len() - faltan.len(),
            faltan: faltan.len(),
        }
    }

    /// Sube el bitmap si aun no esta (una vez por lienzo, o tras `soltar`),
    /// y las teselas que hayan llegado; suelta las que pasen del tope.
    pub fn asegurar(&mut self, motor: &MotorRender) {
        self.recoger();
        if let Some(n) = self.nitidez.as_mut() {
            for t in n.teselas.values_mut() {
                if t.bitmap.is_some() {
                    continue;
                }
                let Some(img) = t.imagen.take() else { continue };
                match motor.bitmap_desde_pixeles_premultiplicado(img.ancho, img.alto, &img.pixeles)
                {
                    Ok(b) => t.bitmap = Some(b),
                    Err(e) => tracing::warn!(?e, "no se pudo subir una tesela del PDF"),
                }
            }
            n.teselas
                .retain(|_, t| t.bitmap.is_some() || t.imagen.is_some());
            let lista: Vec<(ClaveTesela, usize, u64)> = n
                .teselas
                .iter()
                .map(|(c, t)| (*c, t.bytes, t.usada.get()))
                .collect();
            let fuera = a_soltar(&lista, TOPE_TESELAS_BYTES);
            if !fuera.is_empty() {
                for c in &fuera {
                    n.teselas.remove(c);
                }
                if let Ok(mut t) = n.compartido.tenidas.lock() {
                    for c in &fuera {
                        t.remove(c);
                    }
                }
                // Lo soltado puede volver a hacer falta: el pedido se rehace.
                n.ultimo.borrow_mut().clear();
            }
        }
        if self.bitmap.is_some() || self.fallo || self.imagen.ancho == 0 || self.imagen.alto == 0 {
            return;
        }
        match motor.bitmap_desde_pixeles_premultiplicado(
            self.imagen.ancho,
            self.imagen.alto,
            &self.imagen.pixeles,
        ) {
            Ok(b) => self.bitmap = Some(b),
            Err(e) => {
                self.fallo = true;
                tracing::warn!(?e, "no se pudo subir la imagen del lienzo");
            }
        }
    }

    /// Pinta la imagen con la vista del mundo ya puesta. `vista` es la caja
    /// del mundo visible y `zoom_efectivo` el de la camara efectiva.
    ///
    /// Con una pagina de PDF, encima de la pagina entera van las teselas
    /// que haya de la zona, de la mas basta a la mas fina: mientras llega la
    /// resolucion nueva se ve la anterior estirada, nunca un hueco. Y de paso
    /// se piden las que falten ([`FondoLienzo::para_escala`]).
    pub fn pintar(&self, p: &Pintor<'_>, vista: (f32, f32, f32, f32), zoom_efectivo: f32) {
        if !se_ve(vista, self.ancho, self.alto) {
            return;
        }
        if let Some(b) = &self.bitmap {
            // Cuantos pixeles de la imagen caen en cada unidad del mundo: en
            // un pin es uno; en la vista previa de un PDF, menos. La nitidez
            // se elige por lo que de verdad se estira cada pixel.
            let pixeles_por_unidad = self.imagen.ancho as f32 / self.ancho.max(1.0);
            p.bitmap_con(
                b,
                RectF {
                    x: 0.0,
                    y: 0.0,
                    ancho: self.ancho,
                    alto: self.alto,
                },
                None,
                modo_nitidez(zoom_efectivo / pixeles_por_unidad.max(f32::EPSILON)),
            );
        }
        if self.nitidez.is_none() {
            return;
        }
        let estado = self.para_escala(zoom_efectivo, vista);
        self.pintar_teselas(p, vista, estado.densidad);
    }

    /// Encima de la pagina entera, las teselas que ya hay de `vista` hasta
    /// `densidad`, de la mas basta a la mas fina.
    fn pintar_teselas(&self, p: &Pintor<'_>, vista: (f32, f32, f32, f32), densidad: u32) {
        let Some(n) = &self.nitidez else {
            return;
        };
        let reloj = n.reloj.get() + 1;
        n.reloj.set(reloj);
        let mut a_pintar: Vec<(&ClaveTesela, &Tesela)> = n
            .teselas
            .iter()
            .filter(|(c, t)| {
                c.0 <= densidad
                    && t.bitmap.is_some()
                    && t.caja.0 <= vista.2
                    && t.caja.2 >= vista.0
                    && t.caja.1 <= vista.3
                    && t.caja.3 >= vista.1
            })
            .collect();
        a_pintar.sort_by_key(|(c, _)| c.0);
        for (_, t) in a_pintar {
            let Some(b) = &t.bitmap else { continue };
            t.usada.set(reloj);
            p.bitmap_con(
                b,
                RectF {
                    x: t.caja.0,
                    y: t.caja.1,
                    ancho: t.caja.2 - t.caja.0,
                    alto: t.caja.3 - t.caja.1,
                },
                None,
                Interpolacion::Lineal,
            );
        }
    }

    /// **El papel dentro de una lupa**: la pagina y las teselas de lo mirado
    /// (`vista`) a `escala` pixeles por unidad, muestreado siempre lineal
    /// (el `imagePaint` de `Renderer.drawLupa`): lo de dentro de una lupa no
    /// puede pasar de liso a pixelado segun el zoom de la camara.
    ///
    /// Las teselas que falten se piden **ademas** de las de la vista
    /// ([`FondoLienzo::pedir_tambien`]): con `para_escala` el pedido de la
    /// lupa y el de la pantalla se pisaban en cada fotograma y ninguno
    /// llegaba a afinarse. Devuelve cuantas faltan.
    pub fn pintar_en_lupa(&self, p: &Pintor<'_>, vista: (f32, f32, f32, f32), escala: f32) -> usize {
        if !se_ve(vista, self.ancho, self.alto) {
            return 0;
        }
        if let Some(b) = &self.bitmap {
            p.bitmap_con(
                b,
                RectF {
                    x: 0.0,
                    y: 0.0,
                    ancho: self.ancho,
                    alto: self.alto,
                },
                None,
                Interpolacion::Lineal,
            );
        }
        if self.nitidez.is_none() {
            return 0;
        }
        let faltan = self.pedir_tambien(escala, vista);
        self.pintar_teselas(p, vista, densidad_para(escala));
        faltan
    }

    /// Pide las teselas de `vista` a `escala` que falten **sin quitar** las
    /// que ya estaban pedidas (las de la pantalla). Cuantas faltan.
    pub fn pedir_tambien(&self, escala: f32, vista: (f32, f32, f32, f32)) -> usize {
        let Some(n) = &self.nitidez else {
            return 0;
        };
        let (_, quiero) = teselas_para(escala, vista, self.ancho, self.alto);
        let faltan: Vec<ClaveTesela> = quiero
            .into_iter()
            .filter(|c| !n.teselas.contains_key(c))
            .collect();
        if !faltan.is_empty() {
            if let Ok(mut p) = n.compartido.pedido.lock() {
                for c in &faltan {
                    if !p.contains(c) {
                        p.push(*c);
                    }
                }
            }
            n.compartido.cv.notify_all();
        }
        faltan.len()
    }

    /// **Una huella de lo que hay subido**: cambia cuando llega la pagina,
    /// sube una tesela o se suelta alguna. Quien guarda algo pintado con el
    /// papel dentro (la lupa) sabe asi cuando rehacerlo.
    pub fn firma(&self) -> u64 {
        // Cada tesela subida suma su clave revuelta; con XOR no importa el
        // orden del mapa, y una que se va y otra que llega no se anulan.
        let teselas = self.nitidez.as_ref().map_or(0, |n| {
            n.teselas
                .iter()
                .filter(|(_, t)| t.bitmap.is_some())
                .fold(0u64, |h, (c, _)| {
                    h ^ ((c.0 as u64) << 42 | (c.1 as u64) << 21 | c.2 as u64)
                        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                })
        });
        teselas
            ^ ((self.bitmap.is_some() as u64) << 33)
            ^ ((self.imagen.ancho as u64) << 16)
            ^ self.alto.to_bits() as u64
    }

    /// Olvida el bitmap (dispositivo perdido): el siguiente `asegurar` lo
    /// vuelve a subir desde los pixeles guardados. Las teselas no guardan
    /// pixeles: se sueltan y se vuelven a pedir.
    pub fn soltar(&mut self) {
        self.bitmap = None;
        self.fallo = false;
        if let Some(n) = self.nitidez.as_mut() {
            n.teselas.clear();
            n.ultimo.borrow_mut().clear();
            if let Ok(mut t) = n.compartido.tenidas.lock() {
                t.clear();
            }
        }
    }

    /// Si ya llego la pagina de verdad (no la vista previa). Sin PDF, si.
    #[cfg(test)]
    pub fn pagina_lista(&self) -> bool {
        self.nitidez.as_ref().is_none_or(|n| n.base)
    }
}

/// La imagen reducida al tope de la GPU, si pasa de el (D139).
fn reducir(imagen: ImagenRgba, lado_maximo: u32) -> ImagenRgba {
    match lado_de_subida(imagen.ancho, imagen.alto, lado_maximo) {
        None => imagen,
        Some((w, h)) => {
            tracing::info!(
                original_ancho = imagen.ancho,
                original_alto = imagen.alto,
                ancho = w,
                alto = h,
                "imagen del lienzo reducida para la GPU"
            );
            pixpin_codec::redimensionar(imagen, w, h).unwrap_or_else(|e| {
                tracing::warn!(?e, "no se pudo reducir la imagen del lienzo; sale sin fondo");
                ImagenRgba {
                    ancho: 0,
                    alto: 0,
                    pixeles: Vec::new(),
                }
            })
        }
    }
}

/// **El hilo de una pagina de PDF.** Abre el documento una vez (lo caro),
/// manda la pagina entera y luego va pintando las teselas que se le pidan,
/// de una en una y mirando el pedido antes de cada una: si se dejo de mirar
/// esa zona, no se pinta.
fn hilo_del_pdf(p: PaginaPdf, c: Arc<Compartido>, tx: mpsc::Sender<Llegada>) {
    // Lo primero, la vista previa: es un PNG pequeno y se ve enseguida,
    // mientras se abre el PDF (lo caro).
    if let Some(png) = &p.provisional {
        match pixpin_codec::cargar(png) {
            Ok(img) => {
                if tx.send(Llegada::Provisional(img)).is_err() {
                    return;
                }
                c.avisar();
            }
            Err(e) => tracing::info!(?e, "vista previa de la pagina que no se lee"),
        }
    }
    let (pdf, pagina) = (p.pdf, p.pagina);
    // El documento y su pagina entera. Si no se dejan, **la pagina aunque el
    // documento este roto** (E7, `paginaSana` del movil): se rehace desde la
    // copia limpia del proyecto y se vuelve a intentar una vez; si ni asi, se
    // pinta la copia. El fondo no desaparece mientras exista.
    let abrir_y_pintar = |ruta: &std::path::Path| -> Result<(pixpin_pdf::Documento, ImagenRgba), String> {
        let d = pixpin_pdf::Documento::abrir(ruta).map_err(|e| e.to_string())?;
        let img = d.renderizar(pagina, ANCHO_PAPEL_PDF as u32).map_err(|e| e.to_string())?;
        Ok((d, img))
    };
    let (doc, img) = match abrir_y_pintar(&pdf) {
        Ok(v) => v,
        Err(e) => {
            let otra = crate::pdf_del_proyecto::reparar(&pdf);
            match otra.as_deref().map(abrir_y_pintar) {
                Some(Ok(v)) => v,
                _ => {
                    tracing::warn!(%e, pdf = %pdf.display(), pagina, "la pagina del lienzo no se puede pintar");
                    return;
                }
            }
        }
    };
    let proporcion = img.alto as f32 / img.ancho.max(1) as f32;
    if tx.send(Llegada::Base(img)).is_err() {
        return;
    }
    c.avisar();
    // Las mismas medidas que la ventana saca de la pagina entera: una
    // tesela de otra caja caeria corrida.
    let (ancho, alto) = (ANCHO_PAPEL_PDF, ANCHO_PAPEL_PDF * proporcion);
    loop {
        let clave = {
            let Ok(mut p) = c.pedido.lock() else { return };
            loop {
                if c.fin.load(Ordering::Acquire) {
                    return;
                }
                let tenidas = c.tenidas.lock().map(|t| t.clone()).unwrap_or_default();
                p.retain(|k| !tenidas.contains(k));
                if !p.is_empty() {
                    break p.remove(0);
                }
                p = match c.cv.wait(p) {
                    Ok(p) => p,
                    Err(_) => return,
                };
            }
        };
        let Some((x0, y0, x1, y1)) = caja_de_tesela(clave, ancho, alto) else {
            continue;
        };
        let d = clave.0 as f32;
        let (w, h) = (
            ((x1 - x0) * d).round().max(1.0) as u32,
            ((y1 - y0) * d).round().max(1.0) as u32,
        );
        let trozo = (x0 / ancho, y0 / alto, (x1 - x0) / ancho, (y1 - y0) / alto);
        // Apuntada antes de pintarla: si el pedido se rehace mientras tanto,
        // no se vuelve a pedir la que ya esta en camino.
        if let Ok(mut t) = c.tenidas.lock() {
            t.insert(clave);
        }
        match doc.renderizar_trozo(pagina, trozo, w, h) {
            Ok(img) => {
                if tx.send(Llegada::Tesela(clave, img)).is_err() {
                    return;
                }
                c.avisar();
            }
            // Se queda como tenida para no reintentarla en bucle: se vera la
            // resolucion anterior, que es lo que habia.
            Err(e) => tracing::info!(?e, ?clave, "tesela del PDF que no se pudo pintar"),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::navegacion::vista_efectiva;
    use pixpin_motor2d::vector::Punto2;

    #[test]
    fn una_imagen_que_cabe_se_abre_a_zoom_uno_y_centrada() {
        for escala in [100, 150] {
            let c = encuadre_inicial(800.0, 600.0, 1920.0, 1080.0, escala);
            assert_eq!(c.zoom, 1.0, "escala {escala}");
            let centro = vista_efectiva(&c, escala).a_pantalla(Punto2::nuevo(400.0, 300.0));
            assert!(
                (centro.x - 960.0).abs() < 0.5 && (centro.y - 540.0).abs() < 0.5,
                "escala {escala}: {centro:?}"
            );
        }
    }

    #[test]
    fn una_imagen_que_no_cabe_se_abre_al_zoom_que_la_hace_caber_con_margen() {
        let c = encuadre_inicial(4000.0, 3000.0, 1920.0, 1080.0, 100);
        assert!(c.zoom < 1.0);
        assert!(4000.0 * c.zoom + 2.0 * MARGEN_ENCUADRE <= 1920.0 + 0.5);
        assert!(3000.0 * c.zoom + 2.0 * MARGEN_ENCUADRE <= 1080.0 + 0.5);
        let centro = vista_efectiva(&c, 100).a_pantalla(Punto2::nuevo(2000.0, 1500.0));
        assert!((centro.x - 960.0).abs() < 0.5 && (centro.y - 540.0).abs() < 0.5);
    }

    #[test]
    fn un_pixel_mas_que_el_hueco_con_margen_ya_no_va_a_zoom_uno() {
        // Caso negativo del borde: 1825 + 2 x 48 = 1921 > 1920.
        assert!(encuadre_inicial(1825.0, 600.0, 1920.0, 1080.0, 100).zoom < 1.0);
        assert_eq!(
            encuadre_inicial(1824.0, 600.0, 1920.0, 1080.0, 100).zoom,
            1.0
        );
    }

    #[test]
    fn un_dibujo_lejos_del_origen_se_abre_centrado_y_entero() {
        // Pequeno: a zoom 1, con su centro en el centro de la vista.
        let c = encuadre_de_contenido((1000.0, 2000.0, 1400.0, 2300.0), 1920.0, 1080.0, 100);
        assert_eq!(c.zoom, 1.0);
        assert!((c.x + 960.0 - 1200.0).abs() < 0.01 && (c.y + 540.0 - 2150.0).abs() < 0.01);
        // Grande: se aleja hasta que cabe, y cabe entero.
        let (x0, y0, x1, y1) = (-5000.0, -300.0, 4000.0, 2000.0);
        let c = encuadre_de_contenido((x0, y0, x1, y1), 1920.0, 1080.0, 100);
        assert!(c.zoom < 1.0);
        let a = c.a_pantalla(Punto2::nuevo(x0, y0));
        let b = c.a_pantalla(Punto2::nuevo(x1, y1));
        assert!(a.x >= 0.0 && a.y >= 0.0 && b.x <= 1920.0 && b.y <= 1080.0, "{a:?} {b:?}");
        // Un punto solo (caja sin ancho) no divide por cero.
        assert!(encuadre_de_contenido((5.0, 5.0, 5.0, 5.0), 1920.0, 1080.0, 100).zoom.is_finite());
    }

    #[test]
    fn la_nitidez_depende_del_zoom_efectivo() {
        assert_eq!(modo_nitidez(1.0), Interpolacion::Vecino);
        assert_eq!(modo_nitidez(2.0), Interpolacion::Lineal);
        assert_eq!(modo_nitidez(2.999), Interpolacion::Lineal);
        assert_eq!(modo_nitidez(3.0), Interpolacion::Vecino);
        assert_eq!(modo_nitidez(0.5), Interpolacion::Cubica);
        // Caso negativo: casi 1 no es 1; a 1,0001 ya se interpola.
        assert_eq!(modo_nitidez(1.0001), Interpolacion::Lineal);
    }

    #[test]
    fn solo_se_reduce_lo_que_pasa_del_maximo_y_se_conserva_la_proporcion() {
        assert_eq!(lado_de_subida(1920, 1080, 4096), None);
        assert_eq!(lado_de_subida(4096, 10, 4096), None);
        assert_eq!(lado_de_subida(5000, 10, 4096), Some((4096, 8)));
        assert_eq!(lado_de_subida(10, 9000, 4096), Some((5, 4096)));
        // Un maximo desconocido (0) no reduce a nada.
        assert_eq!(lado_de_subida(5000, 10, 0), None);
    }

    #[test]
    fn el_fondo_reducido_sigue_midiendo_lo_que_la_imagen_original() {
        let img = ImagenRgba {
            ancho: 5000,
            alto: 10,
            pixeles: vec![255; 5000 * 10 * 4],
        };
        let f = FondoLienzo::nuevo(img, 4096);
        assert_eq!(f.tamano_subido(), (4096, 8));
        assert_eq!((f.ancho(), f.alto()), (5000.0, 10.0));
        let pequena = ImagenRgba {
            ancho: 20,
            alto: 10,
            pixeles: vec![0; 800],
        };
        assert_eq!(FondoLienzo::nuevo(pequena, 4096).tamano_subido(), (20, 10));
    }

    #[test]
    fn fuera_de_la_vista_el_fondo_no_se_pinta() {
        assert!(se_ve((-10.0, -10.0, 100.0, 100.0), 800.0, 600.0));
        assert!(se_ve((799.0, 599.0, 900.0, 700.0), 800.0, 600.0));
        assert!(!se_ve((801.0, 0.0, 1000.0, 100.0), 800.0, 600.0));
        assert!(!se_ve((-500.0, -500.0, -1.0, -1.0), 800.0, 600.0));
    }

    #[test]
    fn el_recuadro_de_reserva_es_gris_opaco_y_nunca_de_cero() {
        let r = recuadro_gris(3, 2);
        assert_eq!((r.ancho, r.alto), (3, 2));
        assert_eq!(&r.pixeles[0..4], &[200, 200, 200, 255]);
        let minimo = recuadro_gris(0, 0);
        assert_eq!((minimo.ancho, minimo.alto), (1, 1));
    }

    // --- La pagina de PDF como fondo --------------------------------------

    #[test]
    fn al_acercarse_se_pide_mas_resolucion_por_potencias_de_dos() {
        assert_eq!(densidad_para(0.3), 1);
        assert_eq!(densidad_para(1.0), 1);
        assert_eq!(densidad_para(1.25), 2);
        assert_eq!(densidad_para(3.0), 4);
        assert_eq!(densidad_para(9.0), 16);
        // Caso negativo: pasado el tope no se pide mas, se estira.
        assert_eq!(densidad_para(200.0), DENSIDAD_MAXIMA);
        assert_eq!(densidad_para(f32::NAN), 1);
    }

    #[test]
    fn solo_se_piden_las_teselas_de_lo_que_se_ve_y_la_del_centro_primero() {
        let (ancho, alto) = (ANCHO_PAPEL_PDF, 1980.0);
        // A 4 px por unidad cada tesela son 128 unidades.
        let vista = (300.0, 300.0, 556.0, 556.0);
        let (d, v) = teselas_para(4.0, vista, ancho, alto);
        assert_eq!(d, 4);
        assert!(!v.is_empty());
        for c in &v {
            let (x0, y0, x1, y1) = caja_de_tesela(*c, ancho, alto).unwrap();
            assert!(x1 >= vista.0 && x0 <= vista.2 && y1 >= vista.1 && y0 <= vista.3);
        }
        let primera = caja_de_tesela(v[0], ancho, alto).unwrap();
        assert!(primera.0 <= 428.0 && primera.2 >= 428.0, "la del centro va primera");
        // Casos negativos: a zoom 1 basta la pagina entera; fuera del papel,
        // nada; y la ultima columna se recorta al papel, no se sale.
        assert!(teselas_para(1.0, vista, ancho, alto).1.is_empty());
        assert!(teselas_para(8.0, (-900.0, -900.0, -10.0, -10.0), ancho, alto).1.is_empty());
        let (_, borde) = teselas_para(4.0, (1350.0, 0.0, 1400.0, 10.0), ancho, alto);
        let caja = caja_de_tesela(borde[0], ancho, alto).unwrap();
        assert_eq!(caja.2, ancho);
    }

    #[test]
    fn con_la_memoria_llena_se_sueltan_las_teselas_que_hace_mas_que_no_se_ven() {
        let t = vec![((2, 0, 0), 10, 5), ((2, 1, 0), 10, 1), ((2, 2, 0), 10, 9)];
        assert_eq!(a_soltar(&t, 20), vec![(2, 1, 0)]);
        assert_eq!(a_soltar(&t, 5), vec![(2, 1, 0), (2, 0, 0), (2, 2, 0)]);
        // Caso negativo: bajo el tope no se suelta nada.
        assert!(a_soltar(&t, 30).is_empty());
    }

    fn pdf_de_prueba(etiqueta: &str, paginas: usize) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-fondo-pdf-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let imgs: Vec<ImagenRgba> = (0..paginas)
            .map(|i| ImagenRgba {
                ancho: 20,
                alto: 30,
                pixeles: [(i * 7) as u8, 90, 200, 255].repeat(600),
            })
            .collect();
        let ruta = dir.join("plano.pdf");
        std::fs::write(&ruta, pixpin_pdf::union::de_imagenes(&imgs).unwrap()).unwrap();
        ruta
    }

    /// Espera a que `listo` diga que si, recogiendo lo que llegue.
    fn esperar(f: &mut FondoLienzo, mut listo: impl FnMut(&mut FondoLienzo) -> bool) -> bool {
        let fin = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while std::time::Instant::now() < fin {
            f.hay_novedades();
            if listo(f) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn una_pagina_de_pdf_abre_sin_esperar_y_se_afina_en_su_hilo_al_acercarse() {
        let pdf = pdf_de_prueba("afinar", 100);
        let t = std::time::Instant::now();
        let mut f = FondoLienzo::nuevo(
            Fuente::Pdf(PaginaPdf {
                pdf: pdf.clone(),
                pagina: 57,
                provisional: None,
                proporcion: None,
            }),
            4096,
        );
        let tardo = t.elapsed();
        // El hilo de la ventana no abre el PDF: esto es solo lanzar el hilo.
        assert!(tardo.as_millis() < 50, "crear el fondo tardo {tardo:?}");
        assert_eq!(f.ancho(), ANCHO_PAPEL_PDF, "el ancho del movil desde el principio");
        assert!(!f.pagina_lista());
        assert!(esperar(&mut f, |f| f.pagina_lista()), "la pagina no llego");
        // 20x30 de papel: el alto sale de su proporcion, el ancho no cambia.
        assert_eq!(f.ancho(), ANCHO_PAPEL_PDF);
        assert!((f.alto() - ANCHO_PAPEL_PDF * 1.5).abs() < 2.0, "alto {}", f.alto());
        assert_eq!(f.imagen().map(|i| i.ancho), Some(ANCHO_PAPEL_PDF as u32));
        // A zoom 1 basta con ella: no se pide nada mas.
        assert_eq!(f.para_escala(1.0, (0.0, 0.0, 1400.0, 900.0)).faltan, 0);
        // Al acercarse (x4) se piden las teselas de la zona visible...
        let vista = (200.0, 200.0, 400.0, 400.0);
        let e = f.para_escala(4.0, vista);
        assert_eq!(e.densidad, 4);
        assert!(e.faltan > 0);
        assert!(
            esperar(&mut f, |f| f.para_escala(4.0, vista).faltan == 0),
            "las teselas no llegaron"
        );
        // ...y llegan a su densidad: 128 unidades a 4 px son 512 px.
        let n = f.nitidez.as_ref().unwrap();
        assert!(!n.teselas.is_empty());
        for (c, t) in &n.teselas {
            let img = t.imagen.as_ref().unwrap();
            let esperado = ((t.caja.2 - t.caja.0) * c.0 as f32).round() as i64;
            assert!((img.ancho as i64 - esperado).abs() <= esperado / 20 + 2, "{c:?}: {}", img.ancho);
        }
        // Volver a mirar lo mismo no pide nada: se reutiliza.
        let otra = f.para_escala(4.0, vista);
        assert_eq!(otra.faltan, 0);
        assert!(otra.listas > 0);
        drop(f);
        let _ = std::fs::remove_dir_all(pdf.parent().unwrap());
    }

    #[test]
    fn una_pagina_con_vista_previa_abre_con_su_proporcion_y_la_ensena_mientras_llega() {
        // Aunque el PDF no este: la vista previa ya se ve y la hoja ya mide
        // lo suyo desde el primer fotograma.
        let dir = std::env::temp_dir().join(format!("pixpin-fondo-previa-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("previa.png");
        std::fs::write(&png, pixpin_codec::codificar_png(&recuadro_gris(64, 32)).unwrap()).unwrap();
        let mut f = FondoLienzo::nuevo(
            Fuente::Pdf(PaginaPdf {
                pdf: PathBuf::from("no-existe.pdf"),
                pagina: 0,
                provisional: Some(png),
                proporcion: Some(0.5),
            }),
            4096,
        );
        assert_eq!((f.ancho(), f.alto()), (ANCHO_PAPEL_PDF, ANCHO_PAPEL_PDF / 2.0));
        assert!(esperar(&mut f, |f| f.imagen().is_some()), "la vista previa no llego");
        assert_eq!(f.tamano_subido(), (64, 32));
        // Caso negativo: la provisional no cuenta como la pagina de verdad.
        assert!(!f.pagina_lista());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_pdf_que_no_esta_no_revienta_y_deja_el_lienzo_sin_fondo() {
        let mut f = FondoLienzo::nuevo(
            Fuente::Pdf(PaginaPdf {
                pdf: PathBuf::from(r"C:\no\existe\plano.pdf"),
                pagina: 3,
                provisional: None,
                proporcion: None,
            }),
            4096,
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(!f.hay_novedades());
        assert!(!f.pagina_lista());
        assert!(f.imagen().is_none());
        // Pedir resolucion sin hilo no se queda esperando.
        assert!(f.para_escala(4.0, (0.0, 0.0, 100.0, 100.0)).faltan > 0);
    }
}
