//! Las miniaturas de las fotos del proyecto, para el panel de informacion.
//!
//! No es lo mismo que `imagenes_lienzo`: aquel guarda por `id_objeto` lo que
//! se pega DENTRO de un dibujo, y la imagen ya la trae quien pega. Esto lee
//! del disco, por ruta, lo que ya esta guardado en el proyecto.
//!
//! Dos reglas que no son evidentes:
//!
//! 1. **Se lee poco a poco.** Un proyecto con trescientas fotos no puede
//!    parar la ventana medio segundo al abrir la pestana. Por eso solo se
//!    cargan unas pocas por fotograma; las que faltan salen como un recuadro
//!    y aparecen en los siguientes, que es lo que hace cualquier galeria.
//!
//! 2. **Se guarda reducida, no entera.** Una celda mide 82 px; guardar en
//!    memoria el JPEG de 12 MP seria tirar cien megas por foto. Se reduce al
//!    cargarla y lo grande se suelta enseguida.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pixpin_render::{Interpolacion, MotorRender, Pintor, RectF};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

/// Lo que mide el lado mayor de una miniatura ya guardada.
///
/// El doble de la celda mas grande que se pinta (82 px al 100 %), para que
/// al 200 % de escala siga sin verse pastosa. Mas que eso es memoria que no
/// se llega a ver.
const LADO: u32 = 164;

/// Cuantas se cargan como mucho en un fotograma. Cargar una foto es leer del
/// disco y descomprimir; cuatro caben de sobra en 16 ms y el resto llega en
/// los siguientes.
const POR_FOTOGRAMA: usize = 4;

/// Una miniatura: sus pixeles reducidos y, si ya se subio, su bitmap.
///
/// Se guardan los pixeles ademas del bitmap porque con el dispositivo
/// perdido hay que volver a subirlo, y volver a leer del disco seria peor.
struct Miniatura {
    pixeles: Vec<u8>,
    ancho: u32,
    alto: u32,
    bitmap: Option<ID2D1Bitmap1>,
}

/// Lo que se sabe de una ruta.
enum Estado {
    /// Cargada y lista (o a falta de subirla).
    Hecha(Miniatura),
    /// No se pudo leer, o no era una imagen. No se reintenta: si el fichero
    /// no esta, no va a estar en el fotograma siguiente, y reintentarlo seria
    /// leer del disco sesenta veces por segundo para nada.
    Imposible,
}

/// Las miniaturas ya cargadas, por ruta.
pub struct Miniaturas {
    por_ruta: HashMap<PathBuf, Estado>,
    /// El lado mayor con el que se guardan. El panel usa `LADO`; la vista
    /// previa de las burbujas, que es mas grande, el suyo.
    lado: u32,
}

impl Miniaturas {
    pub fn nuevo() -> Self {
        Self::con_lado(LADO)
    }

    /// Con otro tamano de guardado: la vista previa de una foto en su
    /// burbuja mide 260 px y con 164 se veria pastosa.
    pub fn con_lado(lado: u32) -> Self {
        Self {
            por_ruta: HashMap::new(),
            lado,
        }
    }

    /// Carga y sube lo que haga falta de `rutas`, hasta el cupo del
    /// fotograma. Se llama ANTES de pintar, nunca dentro: crear recursos de
    /// dibujo a medio fotograma es justo lo que no se puede hacer.
    ///
    /// Devuelve si quedaron rutas sin leer por el cupo: quien llama tiene que
    /// pedir otro fotograma, o las que faltan no saldrian hasta que el
    /// usuario moviera el raton.
    pub fn asegurar(&mut self, rutas: &[PathBuf], motor: &MotorRender) -> bool {
        let mut cupo = POR_FOTOGRAMA;
        let mut faltan = false;
        for ruta in rutas {
            if !self.por_ruta.contains_key(ruta) {
                if cupo == 0 {
                    // Las que faltan llegan en los fotogramas siguientes;
                    // mientras, su celda ensena el nombre.
                    faltan = true;
                    break;
                }
                cupo -= 1;
                let estado = match cargar_reducida(ruta, self.lado) {
                    Some(m) => Estado::Hecha(m),
                    None => Estado::Imposible,
                };
                self.por_ruta.insert(ruta.clone(), estado);
            }
            let Some(Estado::Hecha(m)) = self.por_ruta.get_mut(ruta) else {
                continue;
            };
            if m.bitmap.is_none() {
                match motor.bitmap_desde_pixeles_premultiplicado(m.ancho, m.alto, &m.pixeles) {
                    Ok(b) => m.bitmap = Some(b),
                    // Se queda sin bitmap y se reintentara: a diferencia de
                    // un fichero que no esta, subir puede fallar por algo
                    // pasajero —el dispositivo perdiendose— y arreglarse.
                    Err(e) => {
                        tracing::warn!(?e, ruta = %ruta.display(), "no se pudo subir la miniatura")
                    }
                }
            }
        }
        faltan
    }

    /// La miniatura ya lista de una ruta, con su tamano.
    ///
    /// `None` tanto si aun no ha llegado su turno como si el fichero no se
    /// pudo leer: quien pinta hace lo mismo en los dos casos —ensenar el
    /// recuadro con el nombre— y distinguirlos solo complicaria ese sitio.
    pub fn ya(&self, ruta: &Path) -> Option<(&ID2D1Bitmap1, u32, u32)> {
        match self.por_ruta.get(ruta) {
            Some(Estado::Hecha(m)) => Some((m.bitmap.as_ref()?, m.ancho, m.alto)),
            _ => None,
        }
    }

    /// Si una ruta todavia no se ha intentado leer. Lo que no se pudo leer
    /// no cuenta: volver a pedirlo no lo va a arreglar, y pedir fotogramas
    /// para nada mantendria la ventana despierta.
    pub fn pendiente(&self, ruta: &Path) -> bool {
        !self.por_ruta.contains_key(ruta)
    }

    /// Suelta los bitmaps conservando los pixeles: es lo que hay que hacer
    /// cuando se pierde el dispositivo de dibujo.
    pub fn soltar(&mut self) {
        for estado in self.por_ruta.values_mut() {
            if let Estado::Hecha(m) = estado {
                m.bitmap = None;
            }
        }
    }

    /// Cuantas rutas se conocen, buenas y malas. Solo lo miran las pruebas.
    #[cfg(test)]
    fn cuantas(&self) -> usize {
        self.por_ruta.len()
    }
}

/// Pinta un bitmap dentro de `destino` llenandolo, recortando lo que sobre.
///
/// Recorta en vez de encajarlo entero porque es una cuadricula: una foto
/// apaisada y una vertical tienen que ocupar la misma celda, y dejar bandas
/// de fondo a los lados haria que la rejilla pareciera rota. Es lo que hacen
/// Telegram y cualquier galeria.
pub fn pintar_recortado(p: &Pintor, b: &ID2D1Bitmap1, destino: RectF, ancho: u32, alto: u32) {
    let fuente = recorte_central(ancho, alto, destino.ancho, destino.alto);
    // Lineal y no cubica: la miniatura ya se guardo reducida con cuidado, y
    // lo que queda aqui es un ajuste pequeno de nada.
    p.bitmap_con(b, destino, Some(fuente), Interpolacion::Lineal);
}

/// Que trozo de una imagen de `ancho`x`alto` hay que tomar para llenar una
/// caja con esa proporcion, tomandolo del centro.
fn recorte_central(ancho: u32, alto: u32, caja_ancho: f32, caja_alto: f32) -> RectF {
    let (w, h) = (ancho as f32, alto as f32);
    let entera = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: h,
    };
    if w <= 0.0 || h <= 0.0 || caja_ancho <= 0.0 || caja_alto <= 0.0 {
        return entera;
    }
    let quiere = caja_ancho / caja_alto;
    let tiene = w / h;
    if tiene > quiere {
        // Sobra por los lados: se toma una franja vertical del centro.
        let usado = h * quiere;
        RectF {
            x: (w - usado) / 2.0,
            y: 0.0,
            ancho: usado,
            alto: h,
        }
    } else {
        // Sobra por arriba y por abajo.
        let usado = w / quiere;
        RectF {
            x: 0.0,
            y: (h - usado) / 2.0,
            ancho: w,
            alto: usado,
        }
    }
}

/// Lee una imagen del disco y la deja en tamano de miniatura.
fn cargar_reducida(ruta: &Path, lado: u32) -> Option<Miniatura> {
    let imagen = pixpin_codec::imagen::cargar(ruta)
        .inspect_err(
            |e| tracing::info!(?e, ruta = %ruta.display(), "no es una imagen que sepamos leer"),
        )
        .ok()?;
    if imagen.ancho == 0 || imagen.alto == 0 {
        return None;
    }
    let imagen = match encoger_a(imagen.ancho, imagen.alto, lado) {
        None => imagen,
        Some((w, h)) => pixpin_codec::redimensionar(imagen, w, h)
            .inspect_err(|e| tracing::warn!(?e, "no se pudo reducir la miniatura"))
            .ok()?,
    };
    Some(Miniatura {
        pixeles: imagen.pixeles,
        ancho: imagen.ancho,
        alto: imagen.alto,
        bitmap: None,
    })
}

/// A que tamano reducir una imagen para que su lado mayor sea `lado`.
/// `None` si ya es igual o mas pequena: una foto pequena NO se agranda, que
/// solo gastaria memoria y se veria igual de mal.
fn encoger_a(ancho: u32, alto: u32, lado: u32) -> Option<(u32, u32)> {
    let mayor = ancho.max(alto);
    if mayor <= lado || mayor == 0 {
        return None;
    }
    let factor = lado as f64 / mayor as f64;
    Some((
        ((ancho as f64 * factor).round() as u32).max(1),
        ((alto as f64 * factor).round() as u32).max(1),
    ))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_foto_grande_se_encoge_guardando_su_proporcion() {
        let (w, h) = encoger_a(4000, 3000, 164).expect("4000 pasa de 164");
        assert_eq!(w, 164);
        assert_eq!(h, 123, "tres cuartos de 164");
        // La alta se mide por su lado mayor, que es el alto.
        assert_eq!(encoger_a(1000, 2000, 164), Some((82, 164)));
    }

    #[test]
    fn una_foto_pequena_no_se_agranda() {
        // Caso negativo: agrandar no mejora nada y gasta memoria.
        assert_eq!(encoger_a(80, 60, 164), None);
        assert_eq!(encoger_a(164, 164, 164), None, "justo el tope tampoco");
        // Ni una degenerada tumba la cuenta.
        assert_eq!(encoger_a(0, 0, 164), None);
    }

    #[test]
    fn una_franja_muy_larga_no_se_queda_en_cero_pixeles() {
        // 2000x3 reducido a 164 de lado mayor da 0,246 de alto: redondear a
        // cero dejaria una imagen sin pixeles, y subirla fallaria.
        assert_eq!(encoger_a(2000, 3, 164), Some((164, 1)));
    }

    #[test]
    fn el_recorte_de_una_apaisada_toma_una_franja_del_centro() {
        // Una foto 200x100 en una celda cuadrada: sobra por los lados.
        let r = recorte_central(200, 100, 80.0, 80.0);
        assert_eq!((r.ancho, r.alto), (100.0, 100.0), "un cuadrado de lado 100");
        assert_eq!((r.x, r.y), (50.0, 0.0), "sobra lo mismo a cada lado");
    }

    #[test]
    fn el_recorte_de_una_vertical_toma_una_franja_de_enmedio() {
        let r = recorte_central(100, 200, 80.0, 80.0);
        assert_eq!((r.ancho, r.alto), (100.0, 100.0));
        assert_eq!((r.x, r.y), (0.0, 50.0));
    }

    #[test]
    fn una_foto_que_ya_encaja_se_toma_entera() {
        let r = recorte_central(120, 60, 40.0, 20.0);
        assert_eq!((r.x, r.y, r.ancho, r.alto), (0.0, 0.0, 120.0, 60.0));
        // Y una caja sin tamano no hace dividir por cero.
        let r = recorte_central(120, 60, 0.0, 0.0);
        assert_eq!((r.ancho, r.alto), (120.0, 60.0));
    }

    #[test]
    fn lo_que_no_se_puede_leer_se_apunta_como_imposible_y_no_se_reintenta() {
        let mut m = Miniaturas::nuevo();
        let ruta = Path::new("no-existe-de-verdad-0f3a.png");
        // Caso negativo: un fichero que no esta no da miniatura.
        assert!(cargar_reducida(ruta, LADO).is_none());
        m.por_ruta.insert(ruta.to_path_buf(), Estado::Imposible);
        assert_eq!(m.cuantas(), 1);
        assert!(
            matches!(m.por_ruta.get(ruta), Some(Estado::Imposible)),
            "queda apuntado, para no volver a tocar el disco"
        );
    }

    #[test]
    fn lo_que_no_esta_cargado_no_da_miniatura() {
        let mut m = Miniaturas::nuevo();
        let ruta = Path::new("ni-cargada-ni-nada.png");
        assert!(m.ya(ruta).is_none(), "sin cargar no hay nada que pintar");
        // Y una que se supo imposible tampoco la da, aunque se conozca.
        m.por_ruta.insert(ruta.to_path_buf(), Estado::Imposible);
        assert!(m.ya(ruta).is_none());
    }
}
