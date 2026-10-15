//! **Las imagenes del Word en modo texto**, pintadas en su sitio y que se
//! sacan como pin con un clic.
//!
//! # Por que asi
//!
//! El movil las ensena dentro del texto (`DocxAHtml`: `<img>` en su `<p>`,
//! `img{max-width:100%;height:auto}`), y el usuario las echaba de menos: el
//! lector solo dejaba su hueco con un recuadro. El hueco ya tenia el tamano
//! bueno (`maqueta_movil::colocar_imagen`, medido por la cabecera), asi que
//! pintar la imagen dentro **no mueve nada**: el documento mide lo mismo y
//! la tinta anotada sigue cayendo en la misma palabra, en el PC y en el
//! movil.
//!
//! Abrir sigue siendo instantaneo: nada se descodifica al abrir ni al medir.
//! Se descodifica **solo lo que se ve**, en un hilo aparte (una foto de
//! doce megapixeles no congela la lectura), **reducida** a lo que puede
//! llegar a ocupar en pantalla y en una cache **con tope de memoria**
//! (`CachePresupuesto`): un Word con cien fotos no se queda con cien fotos
//! en la GPU. Lo que se va de la cache se vuelve a pedir si se vuelve a ver.
//!
//! Para descodificar (y para sacarla como pin) la imagen se escribe una vez
//! en `<datos>/cache/imagenes-doc/<huella>.<ext>`: asi la lee el mismo
//! `pixpin_codec::imagen::cargar` que las fotos (con WIC de reserva para el
//! GIF o el TIFF) y el pin recibe un fichero de verdad, por el mismo camino
//! que «Sacar a la pantalla» del chat (`mensajero::enviar_ficheros` a la
//! ventana principal, que es quien tiene los pines).

use super::Colocado;
use pixpin_codec::imagen::ImagenRgba;
use pixpin_docs::documento::Imagen;
use pixpin_render::MotorRender;
use pixpin_render::cache_presupuesto::CachePresupuesto;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

/// Lo que pueden ocupar a la vez las imagenes descodificadas (ancho x alto
/// x 4). Mucho menos que el de las paginas de Office: aqui van reducidas a
/// la columna.
pub(crate) const PRESUPUESTO: usize = 96 * 1024 * 1024;
/// El lado mas largo de una imagen descodificada, pase lo que pase.
pub(crate) const LADO_TOPE: u32 = 2048;

/// Lo que pide el lector para pintar una imagen.
pub(crate) enum Vista {
    /// Lista para pintar.
    Lista(ID2D1Bitmap1),
    /// Pedida: todavia no ha llegado.
    Cargando,
    /// No se pudo leer.
    Rota,
}

struct Trabajo {
    n: usize,
    datos: Vec<u8>,
    archivo: PathBuf,
    lado: u32,
}

type Llegada = (usize, Option<ImagenRgba>);

pub(crate) struct Imagenes {
    carpeta: PathBuf,
    cache: CachePresupuesto<usize, ID2D1Bitmap1>,
    /// Pedidas al hilo y aun sin contestar: no se piden dos veces.
    pedidas: HashSet<usize>,
    /// Las que no se dejaron leer: no se vuelven a intentar.
    rotas: HashSet<usize>,
    /// El hilo que descodifica, que nace con la primera que se ve.
    hilo: Option<(mpsc::Sender<Trabajo>, mpsc::Receiver<Llegada>)>,
}

impl Imagenes {
    /// Con la carpeta donde se escriben (`<datos>/cache/imagenes-doc`).
    pub(crate) fn nueva(carpeta: PathBuf) -> Self {
        Imagenes {
            carpeta,
            cache: CachePresupuesto::nueva(PRESUPUESTO),
            pedidas: HashSet::new(),
            rotas: HashSet::new(),
            hilo: None,
        }
    }

    pub(crate) fn de_la_aplicacion(raiz: &Path) -> Self {
        Self::nueva(raiz.join("cache").join("imagenes-doc"))
    }

    /// Empieza un fotograma: lo pintado en el anterior ya se puede soltar.
    pub(crate) fn nuevo_fotograma(&mut self) {
        self.cache.nuevo_fotograma();
    }

    /// La imagen `n` para pintarla ahora. Si no esta, se pide (una vez) al
    /// hilo, reducida a `lado` pixeles por su lado largo.
    pub(crate) fn ver(&mut self, n: usize, img: &Imagen, lado: u32) -> Vista {
        if self.rotas.contains(&n) {
            return Vista::Rota;
        }
        if let Some(b) = self.cache.obtener(&n) {
            return Vista::Lista(b);
        }
        if self.pedidas.insert(n) {
            let trabajo = Trabajo {
                n,
                datos: img.datos.clone(),
                archivo: self.archivo_de(img),
                lado: lado.clamp(1, LADO_TOPE),
            };
            let enviado = self.hilo().is_some_and(|(tx, _)| tx.send(trabajo).is_ok());
            if !enviado {
                self.pedidas.remove(&n);
                self.rotas.insert(n);
                return Vista::Rota;
            }
        }
        Vista::Cargando
    }

    /// Recoge lo que el hilo ya descodifico y lo sube a la GPU. `true` si
    /// llego algo (hay que repintar).
    pub(crate) fn recibir(&mut self, motor: &MotorRender) -> bool {
        let Some((_, rx)) = &self.hilo else {
            return false;
        };
        let llegadas: Vec<Llegada> = rx.try_iter().collect();
        let hubo = !llegadas.is_empty();
        for (n, imagen) in llegadas {
            self.pedidas.remove(&n);
            let bitmap = imagen.and_then(|i| {
                motor
                    .bitmap_desde_pixeles_premultiplicado(i.ancho, i.alto, &i.pixeles)
                    .ok()
                    .map(|b| (b, i.ancho as usize * i.alto as usize * 4))
            });
            match bitmap {
                Some((b, bytes)) => self.cache.insertar(n, b, bytes),
                None => {
                    self.rotas.insert(n);
                }
            }
        }
        hubo
    }

    /// Si la imagen `n` se sabe ya que no se deja leer.
    pub(crate) fn rota(&self, n: usize) -> bool {
        self.rotas.contains(&n)
    }

    /// El fichero de esa imagen en la cache de la aplicacion.
    pub(crate) fn archivo_de(&self, img: &Imagen) -> PathBuf {
        self.carpeta.join(nombre_de_archivo(img))
    }

    /// **Saca la imagen a la pantalla como pin**: su fichero (escrito si
    /// aun no lo estaba) a la ventana principal, como «Sacar a la pantalla»
    /// del chat. `false` si no se pudo escribir o nadie contesto.
    pub(crate) fn sacar_como_pin(&self, img: &Imagen) -> bool {
        let archivo = self.archivo_de(img);
        if let Err(e) = escribir_si_falta(&archivo, &img.datos) {
            tracing::warn!(?e, "visor: no se pudo escribir la imagen para el pin");
            return false;
        }
        pixpin_shell::mensajero::enviar_ficheros(std::slice::from_ref(&archivo))
    }

    fn hilo(&mut self) -> Option<&(mpsc::Sender<Trabajo>, mpsc::Receiver<Llegada>)> {
        if self.hilo.is_none() {
            let (tx, rx_trabajo) = mpsc::channel::<Trabajo>();
            let (tx_llegada, rx) = mpsc::channel::<Llegada>();
            let lanzado = std::thread::Builder::new()
                .name("visor-imagenes".into())
                .spawn(move || {
                    // Se acaba solo al cerrarse el lector (cae el emisor).
                    for t in rx_trabajo {
                        let imagen = descodificar(&t);
                        if tx_llegada.send((t.n, imagen)).is_err() {
                            break;
                        }
                    }
                });
            if let Err(e) = lanzado {
                tracing::warn!(?e, "visor: sin hilo para las imagenes");
                return None;
            }
            self.hilo = Some((tx, rx));
        }
        self.hilo.as_ref()
    }
}

/// En el hilo: al disco si no estaba, leida como una foto y reducida.
fn descodificar(t: &Trabajo) -> Option<ImagenRgba> {
    if let Err(e) = escribir_si_falta(&t.archivo, &t.datos) {
        tracing::warn!(?e, "visor: no se pudo escribir la imagen del documento");
        return None;
    }
    let imagen = match pixpin_codec::imagen::cargar(&t.archivo) {
        Ok(i) => i,
        Err(e) => {
            tracing::info!(?e, "visor: imagen del documento que no se deja leer");
            return None;
        }
    };
    let (w, h) = medida_reducida(imagen.ancho, imagen.alto, t.lado);
    if (w, h) == (imagen.ancho, imagen.alto) {
        return Some(imagen);
    }
    pixpin_codec::imagen::redimensionar(imagen, w, h).ok()
}

fn escribir_si_falta(archivo: &Path, datos: &[u8]) -> std::io::Result<()> {
    if archivo.is_file() {
        return Ok(());
    }
    if let Some(c) = archivo.parent() {
        std::fs::create_dir_all(c)?;
    }
    // A un temporal y luego el nombre: un pin o el hilo nunca leen media
    // imagen.
    let temporal = archivo.with_extension("parte");
    std::fs::write(&temporal, datos)?;
    std::fs::rename(&temporal, archivo)
}

/// Lo que mide reducida para que su lado largo no pase de `lado` (nunca
/// agrandada, nunca de cero).
pub(crate) fn medida_reducida(ancho: u32, alto: u32, lado: u32) -> (u32, u32) {
    let largo = ancho.max(alto);
    if largo <= lado || largo == 0 {
        return (ancho, alto);
    }
    let k = lado as f64 / largo as f64;
    let a = |v: u32| ((v as f64 * k).round() as u32).max(1);
    (a(ancho), a(alto))
}

/// `<huella>.<ext>`: la misma imagen da siempre el mismo fichero (no se
/// escribe dos veces) y su extension es la de su tipo, que es lo que mira
/// el pin para saber que es una imagen.
pub(crate) fn nombre_de_archivo(img: &Imagen) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    img.datos.hash(&mut h);
    let ext = match img.mime.as_str() {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        "image/webp" => "webp",
        "image/tiff" => "tif",
        _ => "png",
    };
    format!("{:016x}.{ext}", h.finish())
}

/// **La imagen que hay en ese punto** del documento (en sus unidades), si
/// la hay y se puede sacar como pin. Una imagen rota no cuenta: su hueco no
/// tiene `imagen`.
pub(crate) fn imagen_en(colocados: &[Colocado], x: f32, y: f32) -> Option<usize> {
    // Por orden de altura: solo se miran los que empiezan por encima.
    let hasta = colocados.partition_point(|c| c.y <= y);
    colocados[..hasta].iter().rev().find_map(|c| {
        let n = c.imagen?;
        let dentro = x >= c.sangria && x < c.sangria + c.ancho && y >= c.y && y < c.y + c.alto;
        dentro.then_some(n)
    })
}

#[cfg(test)]
mod pruebas;
