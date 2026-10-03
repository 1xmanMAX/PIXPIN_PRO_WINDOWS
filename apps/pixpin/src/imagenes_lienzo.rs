//! Las imagenes pegadas DENTRO del lienzo, una por `Figura::Imagen`.
//!
//! No es lo mismo que `fondo_lienzo`: aquel es UNA imagen fija que no pasa
//! por la escena (D133); esta es un almacen de varias, cada una referida por
//! el `id_objeto` que lleva su `Figura::Imagen`. El motor modela la figura
//! pero no sabe de bitmaps ni de GPU (a proposito: `pixpin-motor2d` no
//! depende de Direct2D), asi que el puente entre el `id_objeto` y los
//! pixeles tiene que vivir en quien pinta.
//!
//! Vive en `apps/pixpin` y no en `pixpin-render` porque necesita las dos
//! orillas a la vez —`ImagenRgba` de `pixpin-codec` y el bitmap de
//! `pixpin-render`— y `pixpin-render` no depende de `pixpin-codec`. Es
//! exactamente la misma razon por la que `fondo_lienzo` esta aqui, y este
//! modulo reutiliza sus dos reglas ya probadas: `lado_de_subida` (D139) y
//! `modo_nitidez` (D141).
//!
//! **Alcance: la sesion, y lo pegado sale con la escena.** El almacen vive
//! mientras el lienzo esta abierto. Antes los pixeles de lo pegado morian con
//! el: la figura se guardaba (o ni eso, en una hoja de proyecto) y al reabrir
//! la imagen habia desaparecido («si pego una imagen en el canvas no se
//! guarda», 2-oct-2026). Ahora cada imagen nacida en la sesion se marca como
//! **nueva** y sale del editor con [`ImagenesLienzo::tomar_nuevas`], y quien
//! guarda la escribe donde la lee al abrir: en una hoja de proyecto, a
//! `imagenes/<fileId>` con su entrada en `files`, como el movil
//! ([`guardar_pegadas_en_hoja`]); en un `.pixpin2d`, a la carpeta de al lado
//! ([`guardar_pegadas_junto_a`]).

use std::collections::HashMap;

use pixpin_codec::ImagenRgba;
use pixpin_motor2d::{ColorRgba, Elemento, Escena, EstiloRelleno, EstiloTrazo, Figura};
use pixpin_render::{MotorRender, Pintor, RectF};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use crate::fondo_lienzo::{lado_de_subida, modo_nitidez};

/// Cuanto de la parte visible puede llegar a ocupar una imagen recien
/// pegada. No se pega a tamano natural y punto: una captura de 4K en un
/// lienzo al 50 % se saldria de la pantalla por los cuatro lados y el
/// usuario no sabria ni que ha pegado.
const FRACCION_VISIBLE: f32 = 0.8;

/// Una imagen del documento: sus pixeles y, si ya se subio, su bitmap.
///
/// Se guardan los pixeles ademas del bitmap por lo mismo que en
/// `FondoLienzo`: con el dispositivo perdido hay que volver a subirlo.
struct ImagenDelLienzo {
    imagen: ImagenRgba,
    bitmap: Option<ID2D1Bitmap1>,
    /// Subir fallo: no se reintenta en cada fotograma hasta `soltar`.
    fallo: bool,
    /// Nacio en esta sesion (pegada, metida desde un fichero, la foto de una
    /// zona): no esta en disco y hay que escribirla al guardar.
    nueva: bool,
}

/// Las imagenes del documento mientras el lienzo esta abierto.
pub struct ImagenesLienzo {
    /// Lo que admite esta GPU (D139). Se pregunta una vez al motor.
    lado_maximo: u32,
    imagenes: HashMap<u64, ImagenDelLienzo>,
    siguiente_id: u64,
}

impl ImagenesLienzo {
    pub fn nuevo(lado_maximo: u32) -> Self {
        Self {
            lado_maximo,
            imagenes: HashMap::new(),
            // El cero queda libre a proposito: una `Figura::Imagen` con
            // `id_objeto: 0` es la que no se resolvio (un documento
            // reabierto), y conviene que nunca coincida con una real.
            siguiente_id: 1,
        }
    }

    /// Mete una imagen y devuelve el `id_objeto` con el que referirla.
    ///
    /// Devuelve `None` si la imagen esta vacia o no se pudo reducir a lo que
    /// admite la GPU: es preferible no crear el elemento a crear uno que
    /// nunca se va a ver.
    pub fn guardar(&mut self, imagen: ImagenRgba) -> Option<u64> {
        if imagen.ancho == 0 || imagen.alto == 0 {
            return None;
        }
        let imagen = match lado_de_subida(imagen.ancho, imagen.alto, self.lado_maximo) {
            None => imagen,
            Some((w, h)) => {
                tracing::info!(
                    original_ancho = imagen.ancho,
                    original_alto = imagen.alto,
                    ancho = w,
                    alto = h,
                    "imagen pegada reducida para la GPU"
                );
                pixpin_codec::redimensionar(imagen, w, h)
                    .map_err(|e| tracing::warn!(?e, "no se pudo reducir la imagen pegada"))
                    .ok()?
            }
        };
        let id = self.id_nuevo(&imagen);
        self.imagenes.insert(
            id,
            ImagenDelLienzo {
                imagen,
                bitmap: None,
                fallo: false,
                nueva: true,
            },
        );
        Some(id)
    }

    /// **Un id que sirva tambien fuera de la sesion.** El `id_objeto` acaba
    /// siendo el `fileId` del fichero (`pc<hex>`, ver [`id_de_fichero`]), y
    /// la carpeta `imagenes/` es de todo el proyecto: un 1, 2, 3 de sesion
    /// pisaria la foto pegada ayer en otra hoja. Sale de los pixeles, la hora
    /// y un contador, y nunca es cero ni uno que ya este.
    fn id_nuevo(&mut self, imagen: &ImagenRgba) -> u64 {
        let ahora = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ ahora;
        let mezclar = |h: &mut u64, b: u64| {
            *h ^= b;
            *h = h.wrapping_mul(0x0000_0100_0000_01b3);
        };
        mezclar(&mut h, imagen.ancho as u64);
        mezclar(&mut h, imagen.alto as u64);
        // Una muestra de los pixeles basta para que dos imagenes distintas
        // en el mismo instante no coincidan; leerlos todos no aporta nada.
        let paso = (imagen.pixeles.len() / 4096).max(1);
        for b in imagen.pixeles.iter().step_by(paso) {
            mezclar(&mut h, *b as u64);
        }
        loop {
            mezclar(&mut h, self.siguiente_id);
            self.siguiente_id += 1;
            // Por debajo de 2^48: el `pc<hex>` queda en doce cifras.
            let id = h & 0x0000_ffff_ffff_ffff;
            if id != 0 && !self.imagenes.contains_key(&id) {
                return id;
            }
        }
    }

    /// Vuelve a meter una imagen nueva que ya paso por [`Self::guardar`] (y
    /// por su reduccion): la de la vuelta anterior del mismo lienzo al
    /// alternar F11, que cierra la ventana y la abre otra vez.
    pub fn reponer_nueva(&mut self, id: u64, imagen: ImagenRgba) {
        if id == 0 || imagen.ancho == 0 || imagen.alto == 0 {
            return;
        }
        self.imagenes.insert(
            id,
            ImagenDelLienzo {
                imagen,
                bitmap: None,
                fallo: false,
                nueva: true,
            },
        );
    }

    /// **Lo nacido en esta sesion**, que no esta en disco: se saca del
    /// almacen (que se suelta al cerrar de todos modos) para que quien
    /// guarda la escena escriba tambien sus pixeles.
    pub fn tomar_nuevas(&mut self) -> Vec<(u64, ImagenRgba)> {
        let ids: Vec<u64> = self
            .imagenes
            .iter()
            .filter(|(_, i)| i.nueva)
            .map(|(id, _)| *id)
            .collect();
        ids.into_iter()
            .filter_map(|id| self.imagenes.remove(&id).map(|i| (id, i.imagen)))
            .collect()
    }

    /// Mete una imagen con un `id_objeto` que YA existe: el que lleva la
    /// figura de un dibujo que viene del movil.
    ///
    /// Ahi el identificador no lo elegimos nosotros —sale del `fileId` del
    /// fichero— y tiene que coincidir con el de la figura, o la imagen no se
    /// encuentra al pintar y la hoja sale en blanco con sus trazos flotando.
    /// Si ese id ya estaba, no se vuelve a leer: la misma foto puede salir en
    /// varias hojas del mismo proyecto.
    pub fn guardar_con_id(&mut self, id: u64, imagen: ImagenRgba) -> bool {
        if id == 0 || imagen.ancho == 0 || imagen.alto == 0 || self.imagenes.contains_key(&id) {
            return false;
        }
        let imagen = match lado_de_subida(imagen.ancho, imagen.alto, self.lado_maximo) {
            None => imagen,
            Some((w, h)) => match pixpin_codec::redimensionar(imagen, w, h) {
                Ok(i) => i,
                Err(e) => {
                    tracing::warn!(?e, "no se pudo reducir la imagen del proyecto");
                    return false;
                }
            },
        };
        self.imagenes.insert(
            id,
            ImagenDelLienzo {
                imagen,
                bitmap: None,
                fallo: false,
                nueva: false,
            },
        );
        true
    }

    /// El tamano en pixeles de la imagen `id`, tal como se guardo.
    pub fn tamano(&self, id: u64) -> Option<(u32, u32)> {
        self.imagenes
            .get(&id)
            .map(|i| (i.imagen.ancho, i.imagen.alto))
    }

    /// Los pixeles de la imagen `id`, tal como se guardaron: para exportar
    /// el lienzo a un fichero, que no pasa por la GPU de la ventana.
    pub fn rgba(&self, id: u64) -> Option<&ImagenRgba> {
        self.imagenes.get(&id).map(|i| &i.imagen)
    }

    /// Cuantas hay guardadas. Solo lo miran las pruebas, para comprobar que
    /// lo que no se puede subir tampoco se guarda.
    #[cfg(test)]
    pub fn cuantas(&self) -> usize {
        self.imagenes.len()
    }

    /// Sube a la GPU lo que aun no lo esta. Se llama una vez por fotograma,
    /// antes de pintar: subir dentro del `dibujar` seria crear recursos con
    /// el `BeginDraw` abierto.
    pub fn asegurar(&mut self, motor: &MotorRender) {
        for (id, i) in self.imagenes.iter_mut() {
            if i.bitmap.is_some() || i.fallo {
                continue;
            }
            match motor.bitmap_desde_pixeles_premultiplicado(
                i.imagen.ancho,
                i.imagen.alto,
                &i.imagen.pixeles,
            ) {
                Ok(b) => i.bitmap = Some(b),
                Err(e) => {
                    i.fallo = true;
                    tracing::warn!(?e, id, "no se pudo subir una imagen del lienzo");
                }
            }
        }
    }

    /// Pinta la imagen `id` en `destino`, con la vista del mundo ya puesta,
    /// ensenando **solo un trozo** de ella si trae `recorte` (`crop`).
    /// Sin bitmap (todavia sin subir, o un documento reabierto sin pixeles)
    /// no pinta nada: mejor un hueco que un recuadro falso.
    ///
    /// El recorte de Excalidraw no encoge la imagen: la caja del elemento
    /// sigue siendo la misma y lo que cambia es que parte del original se
    /// estira dentro de ella. Ninguna herramienta de Android lo crea —solo
    /// llega de un `.excalidraw` importado de la web— pero el movil **si lo
    /// pinta** (`Renderer.kt:2983`), asi que una imagen recortada abierta
    /// aqui sin esto ensena lo que el recorte habia quitado.
    ///
    /// Con un recorte que no se entiende se pinta la imagen entera, no nada:
    /// ensenar de mas es feo, pero no ensenar la imagen es perderla.
    pub fn pintar_recortada(
        &self,
        p: &Pintor<'_>,
        id: u64,
        destino: RectF,
        zoom_efectivo: f32,
        opacidad: f32,
        recorte: Option<&Recorte>,
    ) {
        let Some(i) = self.imagenes.get(&id) else {
            return;
        };
        let Some(b) = i.bitmap.as_ref() else {
            return;
        };
        // La fuente va en pixeles del bitmap SUBIDO, que puede ser mas
        // pequeno que el original (D139, `lado_de_subida`): por eso el
        // recorte se guarda contra `naturalWidth`/`naturalHeight` y aqui se
        // traduce al tamano que de verdad tiene el bitmap.
        let fuente = recorte.and_then(|r| r.fuente_en(i.imagen.ancho, i.imagen.alto));
        p.bitmap_translucido(b, destino, fuente, modo_nitidez(zoom_efectivo), opacidad);
    }

    /// Olvida los bitmaps (dispositivo perdido): el siguiente `asegurar` los
    /// vuelve a subir desde los pixeles guardados.
    pub fn soltar(&mut self) {
        for i in self.imagenes.values_mut() {
            i.bitmap = None;
            i.fallo = false;
        }
    }
}

/// **El recorte de una imagen** (`crop` de Excalidraw, `Element.kt:1008`).
///
/// Los cuatro primeros campos van en pixeles del ORIGINAL, y por eso hacen
/// falta los dos ultimos: sin saber contra que tamano se midieron, un recorte
/// no se puede traducir al bitmap que se subio, que puede ser mas pequeno.
///
/// Es un tipo propio y no una `RectF` por eso mismo: una `RectF` suelta no
/// dice en que unidades esta, y aqui hay tres sistemas de coordenadas a la
/// vez —la caja del documento, el original y el bitmap subido—.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recorte {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
    /// Lo que media el original cuando se hizo el recorte.
    pub ancho_natural: f32,
    pub alto_natural: f32,
}

impl Recorte {
    /// El rectangulo de la fuente, en pixeles del bitmap de `ancho` x `alto`.
    ///
    /// `None` cuando el recorte no dice nada util —area cero, tamano natural
    /// cero— y entonces se pinta la imagen entera: un recorte roto no puede
    /// hacer desaparecer la foto.
    pub fn fuente_en(&self, ancho: u32, alto: u32) -> Option<RectF> {
        if self.ancho <= 0.0
            || self.alto <= 0.0
            || self.ancho_natural <= 0.0
            || self.alto_natural <= 0.0
            || ancho == 0
            || alto == 0
        {
            return None;
        }
        let (kx, ky) = (
            ancho as f32 / self.ancho_natural,
            alto as f32 / self.alto_natural,
        );
        // Se recorta a lo que existe: un `crop` que se sale del original
        // —los hay, de documentos editados a mano— pediria pixeles que no
        // estan y Direct2D dibujaria basura en el borde.
        let x0 = (self.x * kx).clamp(0.0, ancho as f32);
        let y0 = (self.y * ky).clamp(0.0, alto as f32);
        let x1 = ((self.x + self.ancho) * kx).clamp(0.0, ancho as f32);
        let y1 = ((self.y + self.alto) * ky).clamp(0.0, alto as f32);
        if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
            return None;
        }
        Some(RectF {
            x: x0,
            y: y0,
            ancho: x1 - x0,
            alto: y1 - y0,
        })
    }
}

/// Que hacer con un Ctrl+V en el lienzo.
#[derive(Debug, PartialEq)]
pub enum Pegado {
    /// Pegar lo que se copio DENTRO del editor.
    Elementos,
    /// Pegar una imagen que viene del portapapeles del sistema.
    Imagen(ImagenRgba),
    Nada,
}

/// Decide de donde sale el Ctrl+V.
///
/// El portapapeles interno manda sobre el del sistema, y no al reves. Ctrl+C
/// en el editor NO escribe en el portapapeles de Windows, asi que el sistema
/// casi siempre trae algo viejo —una captura de hace diez minutos—; mirarlo
/// primero convertiria «copio esta flecha y la pego» en «aparece una captura
/// de antes», que es lo que el usuario no ha pedido.
///
/// Del sistema solo se acepta una imagen: texto y rutas los pega la bandeja
/// como pines, que es otra cosa.
pub fn decidir_pegado(
    hay_elementos_copiados: bool,
    del_sistema: Option<pixpin_codec::ContenidoPortapapeles>,
) -> Pegado {
    if hay_elementos_copiados {
        return Pegado::Elementos;
    }
    match del_sistema {
        Some(pixpin_codec::ContenidoPortapapeles::Imagen(img)) if img.ancho > 0 && img.alto > 0 => {
            Pegado::Imagen(img)
        }
        // Un fichero de imagen copiado en el Explorador llega como ruta, no
        // como pixeles: se pega la primera que sea una imagen que se lee.
        Some(pixpin_codec::ContenidoPortapapeles::Rutas(rutas)) => match imagen_de_rutas(&rutas) {
            Some(img) => Pegado::Imagen(img),
            None => Pegado::Nada,
        },
        _ => Pegado::Nada,
    }
}

/// Las extensiones de imagen que se pegan desde una ruta. Por extension y
/// no probando a abrir cualquier cosa: copiar un video de dos gigas y pulsar
/// Ctrl+V en el lienzo no puede ponerse a leerlo.
const EXTENSIONES_DE_IMAGEN: [&str; 9] = ["png", "jpg", "jpeg", "bmp", "gif", "webp", "tif", "tiff", "ico"];

/// La primera imagen legible de unas rutas copiadas.
pub fn imagen_de_rutas(rutas: &[std::path::PathBuf]) -> Option<ImagenRgba> {
    rutas
        .iter()
        .filter(|r| {
            r.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| EXTENSIONES_DE_IMAGEN.contains(&e.to_ascii_lowercase().as_str()))
        })
        .find_map(|r| {
            pixpin_codec::cargar(r)
                .inspect_err(|e| tracing::warn!(?e, ruta = %r.display(), "imagen copiada ilegible"))
                .ok()
                .filter(|i| i.ancho > 0 && i.alto > 0)
        })
}

/// El `fileId` con el que se guarda la imagen `id` del almacen: el `pc<hex>`
/// de lo que nace en el escritorio, que `id_del_fichero` devuelve al mismo
/// numero al reabrir.
pub fn id_de_fichero(id: u64) -> String {
    pixpin_motor2d::enlace::id_de_texto(id)
}

/// Las imagenes nuevas que la escena sigue usando (las pegadas y luego
/// borradas no se escriben), cada una con su `fileId`.
fn pegadas_en_uso<'a>(
    escena: &Escena,
    pegadas: &'a [(u64, ImagenRgba)],
) -> Vec<(String, &'a ImagenRgba)> {
    let usadas: std::collections::HashSet<u64> = escena
        .visibles()
        .filter_map(|e| match e.figura {
            Figura::Imagen { id_objeto } => Some(id_objeto),
            _ => None,
        })
        .collect();
    pegadas
        .iter()
        .filter(|(id, _)| usadas.contains(id))
        .map(|(id, img)| (id_de_fichero(*id), img))
        .collect()
}

/// Escribe `img` en PNG en `ruta`, si no estaba ya: a un temporal y luego se
/// cambia el nombre, como el dibujo, para que un corte de luz no deje media
/// foto.
fn escribir_png_si_falta(ruta: &std::path::Path, img: &ImagenRgba) -> std::io::Result<()> {
    if ruta.is_file() {
        return Ok(());
    }
    if let Some(dir) = ruta.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let bytes = pixpin_codec::imagen::codificar_png(img)
        .map_err(|e| std::io::Error::other(format!("{e:?}")))?;
    let mut temporal = ruta.as_os_str().to_owned();
    temporal.push(".tmp");
    std::fs::write(&temporal, bytes)?;
    std::fs::rename(&temporal, ruta)
}

/// **Las imagenes pegadas en una hoja de proyecto, donde las busca el movil**:
/// cada una a `imagenes/<fileId>` de la carpeta del proyecto y apuntada en
/// `files` del lienzo con esa ruta (el `SceneFile` de alla, sin base64). Es
/// exactamente lo que lee `preparar_hoja` al abrir (`excalidraw::ficheros`).
/// Devuelve el lienzo con las entradas puestas; `con_escena` ya sabe escribir
/// la figura con su `fileId`.
pub fn guardar_pegadas_en_hoja(
    carpeta_proyecto: &std::path::Path,
    lienzo: &pixpin_motor2d::excalidraw::Lienzo,
    escena: &Escena,
    pegadas: &[(u64, ImagenRgba)],
    ahora_ms: i64,
) -> std::io::Result<pixpin_motor2d::excalidraw::Lienzo> {
    let mut salida = lienzo.clone();
    for (clave, img) in pegadas_en_uso(escena, pegadas) {
        let relativa = format!("imagenes/{clave}");
        escribir_png_si_falta(&carpeta_proyecto.join("imagenes").join(&clave), img)?;
        pixpin_motor2d::excalidraw::poner_fichero(&mut salida, &clave, "image/png", &relativa, ahora_ms);
    }
    Ok(salida)
}

/// La carpeta de las imagenes de un dibujo `.pixpin2d` (el de un pin o el de
/// una foto del chat): a su lado, con su nombre entero y `.imagenes` detras.
pub fn carpeta_junto_a(dibujo: &std::path::Path) -> std::path::PathBuf {
    let mut s = dibujo.as_os_str().to_owned();
    s.push(".imagenes");
    std::path::PathBuf::from(s)
}

/// Las imagenes guardadas junto a un `.pixpin2d` que su escena usa, con el
/// `id_objeto` de su figura: lo que el editor recibe como `fotos`.
pub fn fotos_junto_a(dibujo: &std::path::Path, escena: &Escena) -> Vec<(u64, std::path::PathBuf)> {
    let carpeta = carpeta_junto_a(dibujo);
    escena
        .visibles()
        .filter_map(|e| match e.figura {
            Figura::Imagen { id_objeto } => Some(id_objeto),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<u64>>()
        .into_iter()
        .map(|id| (id, carpeta.join(id_de_fichero(id))))
        .filter(|(_, ruta)| ruta.is_file())
        .collect()
}

/// Escribe junto a un `.pixpin2d` las imagenes pegadas que su escena usa.
/// Devuelve cuantas escribio de nuevas o ya estaban.
pub fn guardar_pegadas_junto_a(
    dibujo: &std::path::Path,
    escena: &Escena,
    pegadas: &[(u64, ImagenRgba)],
) -> std::io::Result<usize> {
    let carpeta = carpeta_junto_a(dibujo);
    let en_uso = pegadas_en_uso(escena, pegadas);
    for (clave, img) in &en_uso {
        escribir_png_si_falta(&carpeta.join(clave), img)?;
    }
    Ok(en_uso.len())
}

/// El tamano con el que entra en el lienzo una imagen de `ancho` x `alto`
/// pixeles: el natural, y reducido en proporcion si no cabe en la parte
/// visible con holgura. Nunca se agranda: una imagen pequena se pega
/// pequena, como en cualquier editor.
pub fn tamano_al_pegar(ancho: u32, alto: u32, vista_ancho: f32, vista_alto: f32) -> (f32, f32) {
    let (w, h) = (ancho.max(1) as f32, alto.max(1) as f32);
    let (cabe_w, cabe_h) = (
        (vista_ancho * FRACCION_VISIBLE).max(1.0),
        (vista_alto * FRACCION_VISIBLE).max(1.0),
    );
    let factor = (cabe_w / w).min(cabe_h / h).min(1.0);
    ((w * factor).max(1.0), (h * factor).max(1.0))
}

/// La esquina superior izquierda para dejar una caja de `ancho` x `alto`
/// centrada en la caja del mundo `vista` (`x0, y0, x1, y1`).
pub fn esquina_centrada(vista: (f32, f32, f32, f32), ancho: f32, alto: f32) -> (f32, f32) {
    let (x0, y0, x1, y1) = vista;
    ((x0 + x1) / 2.0 - ancho / 2.0, (y0 + y1) / 2.0 - alto / 2.0)
}

/// El elemento que representa una imagen ya guardada en el almacen.
///
/// Sin trazo ni relleno: la imagen es su propio aspecto, y `propiedades.rs`
/// ya dice que de una imagen solo se toca la opacidad.
pub fn elemento_imagen(id_objeto: u64, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
    Elemento {
        id: 0, // lo pone `Escena::anadir`
        figura: Figura::Imagen { id_objeto },
        x,
        y,
        ancho,
        alto,
        angulo: 0.0,
        trazo: ColorRgba::opaco(0.1, 0.1, 0.1),
        relleno: None,
        estilo_relleno: EstiloRelleno::Solido,
        grosor: 0.0,
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
        material: Default::default(),
        extras: Default::default(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_codec::ContenidoPortapapeles;

    fn imagen(ancho: u32, alto: u32) -> ImagenRgba {
        ImagenRgba {
            ancho,
            alto,
            pixeles: vec![255; ancho as usize * alto as usize * 4],
        }
    }

    fn recorte(x: f32, y: f32, ancho: f32, alto: f32) -> Recorte {
        Recorte {
            x,
            y,
            ancho,
            alto,
            ancho_natural: 400.0,
            alto_natural: 200.0,
        }
    }

    #[test]
    fn un_recorte_pide_el_trozo_del_original_que_le_toca() {
        // La imagen se subio a su tamano natural: el recorte va tal cual.
        let f = recorte(100.0, 50.0, 200.0, 100.0)
            .fuente_en(400, 200)
            .expect("el recorte tiene area");
        assert_eq!((f.x, f.y, f.ancho, f.alto), (100.0, 50.0, 200.0, 100.0));
    }

    #[test]
    fn un_recorte_se_traduce_al_bitmap_reducido_que_admitio_la_gpu() {
        // El fallo que esto impide (D139): la imagen se subio a la mitad
        // porque no cabia, y el recorte —medido contra el original— pedia el
        // doble de pixeles de los que hay. Se veia el trozo equivocado.
        let f = recorte(100.0, 50.0, 200.0, 100.0)
            .fuente_en(200, 100)
            .unwrap();
        assert_eq!((f.x, f.y, f.ancho, f.alto), (50.0, 25.0, 100.0, 50.0));
    }

    #[test]
    fn un_recorte_que_se_sale_del_original_se_queda_en_el_borde() {
        // Los hay, de documentos editados a mano: pedir pixeles que no estan
        // hace que Direct2D dibuje basura en el borde.
        let f = recorte(300.0, 150.0, 400.0, 400.0)
            .fuente_en(400, 200)
            .unwrap();
        assert_eq!((f.x, f.y, f.ancho, f.alto), (300.0, 150.0, 100.0, 50.0));
    }

    #[test]
    fn un_recorte_roto_no_hace_desaparecer_la_foto() {
        // Caso negativo, y el que manda: sin area, sin tamano natural o
        // contra un bitmap vacio se devuelve `None`, que quien pinta lee como
        // «la imagen entera». Ensenar de mas es feo; no ensenarla, peor.
        assert!(recorte(0.0, 0.0, 0.0, 100.0).fuente_en(400, 200).is_none());
        assert!(recorte(0.0, 0.0, 100.0, -5.0).fuente_en(400, 200).is_none());
        assert!(recorte(0.0, 0.0, 100.0, 100.0).fuente_en(0, 200).is_none());
        let mut r = recorte(0.0, 0.0, 100.0, 100.0);
        r.ancho_natural = 0.0;
        assert!(r.fuente_en(400, 200).is_none());
        // Y un recorte del todo fuera tampoco: menos de un pixel no se pinta.
        assert!(
            recorte(500.0, 0.0, 100.0, 100.0)
                .fuente_en(400, 200)
                .is_none()
        );
    }

    #[test]
    fn una_imagen_que_cabe_se_pega_a_su_tamano_natural() {
        assert_eq!(tamano_al_pegar(400, 300, 1920.0, 1080.0), (400.0, 300.0));
    }

    #[test]
    fn una_imagen_mas_grande_que_la_vista_se_reduce_conservando_la_proporcion() {
        let (w, h) = tamano_al_pegar(4000, 2000, 1000.0, 1000.0);
        assert!((w - 800.0).abs() < 0.01, "{w}");
        assert!((h - 400.0).abs() < 0.01, "{h}");
        // Caso negativo del borde: justo en el limite no se toca.
        assert_eq!(tamano_al_pegar(800, 400, 1000.0, 1000.0), (800.0, 400.0));
    }

    #[test]
    fn una_imagen_pequena_nunca_se_agranda_para_llenar_la_vista() {
        assert_eq!(tamano_al_pegar(2, 2, 4000.0, 4000.0), (2.0, 2.0));
    }

    #[test]
    fn lo_pegado_queda_centrado_en_lo_que_se_ve() {
        let (x, y) = esquina_centrada((100.0, 200.0, 900.0, 800.0), 400.0, 300.0);
        assert_eq!((x, y), (300.0, 350.0));
        // El centro de la caja pegada es el centro de la vista.
        assert_eq!((x + 200.0, y + 150.0), (500.0, 500.0));
    }

    #[test]
    fn pegar_sin_imagen_en_el_portapapeles_no_crea_nada() {
        assert_eq!(decidir_pegado(false, None), Pegado::Nada);
        assert_eq!(
            decidir_pegado(false, Some(ContenidoPortapapeles::Texto("hola".into()))),
            Pegado::Nada
        );
        assert_eq!(
            decidir_pegado(false, Some(ContenidoPortapapeles::Rutas(vec![]))),
            Pegado::Nada
        );
        // Una imagen sin pixeles tampoco vale: no se puede subir un bitmap
        // de cero lados.
        assert_eq!(
            decidir_pegado(false, Some(ContenidoPortapapeles::Imagen(imagen(0, 0)))),
            Pegado::Nada
        );
    }

    #[test]
    fn lo_copiado_en_el_editor_manda_sobre_el_portapapeles_del_sistema() {
        let del_sistema = Some(ContenidoPortapapeles::Imagen(imagen(2, 2)));
        assert_eq!(decidir_pegado(true, del_sistema), Pegado::Elementos);
        // Sin nada copiado dentro, sí entra la imagen del sistema.
        assert!(matches!(
            decidir_pegado(false, Some(ContenidoPortapapeles::Imagen(imagen(2, 2)))),
            Pegado::Imagen(_)
        ));
    }

    #[test]
    fn cada_imagen_guardada_recibe_un_id_propio_y_distinto_de_cero() {
        let mut a = ImagenesLienzo::nuevo(4096);
        let primera = a.guardar(imagen(4, 3)).expect("cabe");
        let segunda = a.guardar(imagen(4, 3)).expect("cabe");
        assert_ne!(primera, 0);
        assert_ne!(primera, segunda);
        assert_eq!(a.cuantas(), 2);
        assert_eq!(a.tamano(primera), Some((4, 3)));
        // Caso negativo: una imagen vacia no se guarda ni gasta id.
        assert_eq!(a.guardar(imagen(0, 0)), None);
        assert_eq!(a.cuantas(), 2);
    }

    #[test]
    fn una_imagen_enorme_se_guarda_ya_reducida_a_lo_que_admite_la_gpu() {
        let mut a = ImagenesLienzo::nuevo(64);
        let id = a.guardar(imagen(256, 128)).expect("se reduce");
        assert_eq!(a.tamano(id), Some((64, 32)));
    }

    #[test]
    fn una_imagen_pegada_en_el_lienzo_de_un_pin_vuelve_al_reabrirlo() {
        // El mismo fallo en un `.pixpin2d` (pin o foto del chat): la figura
        // se guardaba, los pixeles no.
        let dir = std::env::temp_dir().join(format!("pixpin-pegada-pin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dibujo = dir.join("pin.pixpin2d");
        let mut sesion = ImagenesLienzo::nuevo(4096);
        let mut original = imagen(6, 5);
        original.pixeles[0] = 7;
        let id = sesion.guardar(original.clone()).unwrap();
        let mut escena = Escena::nueva();
        escena.anadir(elemento_imagen(id, 1.0, 2.0, 6.0, 5.0));
        let pegadas = sesion.tomar_nuevas();
        assert_eq!(guardar_pegadas_junto_a(&dibujo, &escena, &pegadas).unwrap(), 1);
        pixpin_motor2d::guardar(&dibujo, &escena).unwrap();

        let reabierta = pixpin_motor2d::cargar(&dibujo).unwrap();
        let fotos = fotos_junto_a(&dibujo, &reabierta);
        assert_eq!(fotos.len(), 1);
        assert_eq!(fotos[0].0, id);
        let leida = pixpin_codec::cargar(&fotos[0].1).unwrap();
        assert_eq!(leida.pixeles, original.pixeles);
        // Caso negativo: lo tomado ya no esta en el almacen de la sesion,
        // y las que vienen de disco nunca salen como nuevas.
        assert!(sesion.tomar_nuevas().is_empty());
        let mut otra = ImagenesLienzo::nuevo(4096);
        assert!(otra.guardar_con_id(id, leida));
        assert!(otra.tomar_nuevas().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_id_de_una_imagen_pegada_vuelve_igual_de_su_file_id() {
        let mut a = ImagenesLienzo::nuevo(4096);
        let id = a.guardar(imagen(3, 3)).unwrap();
        assert_eq!(pixpin_motor2d::enlace::id_del_fichero(&id_de_fichero(id)), id);
    }

    #[test]
    fn una_ruta_que_no_es_imagen_no_se_pega() {
        assert_eq!(
            decidir_pegado(
                false,
                Some(ContenidoPortapapeles::Rutas(vec!["video.mp4".into(), "no-esta.png".into()]))
            ),
            Pegado::Nada
        );
    }

    #[test]
    fn el_elemento_de_una_imagen_lleva_su_id_y_su_caja() {
        let e = elemento_imagen(7, 10.0, 20.0, 100.0, 50.0);
        assert_eq!(e.figura, Figura::Imagen { id_objeto: 7 });
        assert_eq!(e.caja(), (10.0, 20.0, 110.0, 70.0));
        // Sin relleno: una imagen no se pinta con fondo debajo.
        assert_eq!(e.relleno, None);
    }
}
