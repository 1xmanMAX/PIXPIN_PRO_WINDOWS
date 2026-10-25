//! **La foto del chat, pintada**: lo que se ve en el circulo de cada chat en
//! vez de (o detras de) sus iniciales.
//!
//! Empezo el 4-oct como el logo de imagen («el logo de los proyectos esta un
//! poco feo asi que dame la opcion de ponerle de logo una imagen»), un
//! `logo.png` solo del PC. Desde el 25-oct es la foto del chat
//! (`pixpin_proyecto::avatar`): una imagen, un emoji sobre un degradado, o
//! las iniciales sobre el color elegido, y **viaja** con el proyecto. El
//! `logo.png` de antes sigue saliendo (y viajando) mientras no se cambie.
//! Lo que se elige esta en `foto_del_chat.rs`.
//!
//! Dos reglas que no son evidentes:
//!
//! 1. **La imagen se guarda ya cuadrada y pequena** (256 de lado). El avatar
//!    mide 46 px; guardar la foto de 12 MP para ensenarla asi seria leer
//!    megas en cada arranque.
//! 2. **El circulo va en los pixeles**, no en el dibujo: horneando el alfa
//!    del circulo al cargarla, se pinta como cualquier bitmap, con su borde
//!    suave.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use pixpin_codec::ImagenRgba;
use pixpin_proyecto::almacen::Ficha;
use pixpin_proyecto::avatar::{self, Avatar};
use pixpin_render::{Color, Interpolacion, MotorRender, Pintor, RectF};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use super::{color_avatar, hex, iniciales};

/// Cada cuanto se vuelve a mirar si llego la imagen de una foto que aun no
/// esta (la sincronizacion trae el proyecto y sus ficheros por separado).
const REINTENTO: Duration = Duration::from_secs(3);

/// El cuadrado `(x, y, lado)` de una imagen, recortado. `None` si esta vacia,
/// sus pixeles no cuadran con su tamano o el cuadrado se sale.
pub(super) fn recortar(imagen: &ImagenRgba, (x, y, lado): (u32, u32, u32)) -> Option<ImagenRgba> {
    if imagen.ancho == 0
        || imagen.alto == 0
        || lado == 0
        || imagen.pixeles.len() != imagen.bytes_esperados()
        || x + lado > imagen.ancho
        || y + lado > imagen.alto
    {
        return None;
    }
    let (fila, trozo) = (imagen.ancho as usize * 4, lado as usize * 4);
    let mut pixeles = Vec::with_capacity(trozo * lado as usize);
    for f in y..y + lado {
        let inicio = f as usize * fila + x as usize * 4;
        pixeles.extend_from_slice(&imagen.pixeles[inicio..inicio + trozo]);
    }
    Some(ImagenRgba {
        ancho: lado,
        alto: lado,
        pixeles,
    })
}

/// La imagen recortada a su cuadrado central.
fn recortar_cuadrado(imagen: &ImagenRgba) -> Option<ImagenRgba> {
    let r = pixpin_ui::foto_del_chat::Recorte::centrado(imagen.ancho, imagen.alto);
    recortar(imagen, r.en_pixeles(imagen.ancho, imagen.alto))
}

/// Cuanto del pixel `(x, y)` cae dentro del circulo inscrito en un cuadrado
/// de `lado`: 1 dentro, 0 fuera y lo de en medio en el borde, que es lo que
/// lo deja suave en vez de dentado.
fn cobertura(lado: u32, x: u32, y: u32) -> f32 {
    let r = lado as f32 / 2.0;
    let (dx, dy) = (x as f32 + 0.5 - r, y as f32 + 0.5 - r);
    (r - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0)
}

/// Hornea el circulo en el alfa de unos pixeles RGBA cuadrados.
fn en_circulo(pixeles: &mut [u8], lado: u32) {
    for (i, p) in pixeles.chunks_exact_mut(4).enumerate() {
        let (x, y) = (i as u32 % lado, i as u32 / lado);
        p[3] = (p[3] as f32 * cobertura(lado, x, y)).round() as u8;
    }
}

/// **Guarda como foto del chat** el cuadrado ya recortado `cuadrada`:
/// reducida a 256 si es mayor, en PNG, en la carpeta del proyecto y apuntada
/// en su ficha. Entero o nada.
pub(super) fn guardar_imagen(
    raiz: &Path,
    id: &str,
    cuadrada: ImagenRgba,
    cuando: i64,
) -> anyhow::Result<()> {
    // Una pequena NO se agranda: solo gastaria disco y se veria igual.
    let cuadrada = if cuadrada.ancho > avatar::LADO {
        pixpin_codec::redimensionar(cuadrada, avatar::LADO, avatar::LADO)?
    } else {
        cuadrada
    };
    let png = pixpin_codec::codificar_png(&cuadrada)?;
    let rel = avatar::guardar_imagen(raiz, id, &png, cuando)?;
    avatar::poner(raiz, id, &Avatar::imagen(&rel, cuando))?;
    olvidar(id);
    Ok(())
}

// --- Pintar ----------------------------------------------------------------

/// Un circulo con el degradado `(arriba, abajo)` de arriba abajo.
pub(super) fn pintar_fondo(p: &Pintor, caja: RectF, (arriba, abajo): (u32, u32)) {
    if arriba == abajo || !p.empujar_recorte_redondeado(caja, caja.ancho / 2.0) {
        p.rellenar_redondeado(caja, caja.ancho / 2.0, hex(arriba));
        return;
    }
    let cx = caja.x + caja.ancho / 2.0;
    p.rect_degradado(
        caja,
        (cx, caja.y),
        (cx, caja.y + caja.alto),
        hex(arriba),
        hex(abajo),
    );
    p.soltar_recorte_redondeado();
}

/// Un emoji grande, en color y centrado en el circulo.
pub(super) fn pintar_emoji(p: &Pintor, caja: RectF, emoji: &str) {
    let tam = caja.alto * 0.52;
    let (w, h) = p.medir_texto(emoji, tam);
    p.texto_color(
        emoji,
        caja.x + (caja.ancho - w) / 2.0,
        caja.y + (caja.alto - h) / 2.0,
        tam,
        Color::BLANCO,
    );
}

/// Las iniciales en blanco, centradas.
pub(super) fn pintar_letras(p: &Pintor, caja: RectF, nombre: &str) {
    let letras = iniciales(nombre);
    let tam = caja.alto * 0.4;
    let (w, h) = p.medir_texto(&letras, tam);
    p.texto(
        &letras,
        caja.x + (caja.ancho - w) / 2.0,
        caja.y + (caja.alto - h) / 2.0,
        tam,
        Color::BLANCO,
    );
}

/// **Una foto del chat entera**: la imagen si la hay (`imagen`, ya en
/// circulo), o el emoji o las iniciales sobre su fondo. Sin fondo elegido,
/// el color de siempre del chat (`codigo`).
pub(super) fn pintar_avatar(
    p: &Pintor,
    caja: RectF,
    a: Option<&Avatar>,
    imagen: Option<&ID2D1Bitmap1>,
    nombre: &str,
    codigo: &str,
) {
    if let Some(b) = imagen {
        // Cubica: se reduce de 256 a 46, y la lineal dejaria dientes.
        p.bitmap_con(b, caja, None, Interpolacion::Cubica);
        return;
    }
    match a.and_then(Avatar::fondo) {
        Some(f) => pintar_fondo(p, caja, f),
        None => p.rellenar_redondeado(caja, caja.ancho / 2.0, color_avatar(codigo)),
    }
    match a.and_then(Avatar::el_emoji) {
        Some(e) => pintar_emoji(p, caja, e),
        None => pintar_letras(p, caja, nombre),
    }
}

// --- La cache, del hilo del chat ----------------------------------------

/// Una imagen leida: sus pixeles (ya en circulo) y, si ya se subio, su
/// bitmap. Los pixeles se quedan para volver a subirla si se pierde el
/// dispositivo.
pub(super) struct Foto {
    pixeles: Vec<u8>,
    lado: u32,
    bitmap: Option<ID2D1Bitmap1>,
}

enum Imagen {
    Hecha(Foto),
    /// No esta (o no se lee): se vuelve a mirar pasado un rato, por si la
    /// trae la sincronizacion.
    Falta(Instant),
}

/// Lo que se sabe de la foto de un proyecto.
struct Entrada {
    /// El JSON de la ficha del que salio: si cambia, se rehace.
    valor: Option<serde_json::Value>,
    avatar: Option<Avatar>,
    /// Donde esta su imagen y como va, si es de imagen.
    imagen: Option<(PathBuf, Imagen)>,
}

thread_local! {
    /// Por `id` de proyecto. Del hilo del chat, que es el que pinta, como
    /// la cache del grafito. El editor de la foto corre en otro hilo: lo que
    /// cambia lo apunta en el indice, la lista lo relee y aqui se nota en
    /// que su JSON ya no es el mismo.
    static FOTOS: RefCell<HashMap<String, Entrada>> = RefCell::new(HashMap::new());
}

/// Olvida lo leido de un proyecto: se vuelve a leer al pintarlo.
pub(super) fn olvidar(id: &str) {
    FOTOS.with_borrow_mut(|m| m.remove(id));
}

/// Suelta los bitmaps y conserva los pixeles: el dispositivo se perdio.
pub(super) fn soltar() {
    FOTOS.with_borrow_mut(|m| {
        for e in m.values_mut() {
            if let Some((_, Imagen::Hecha(f))) = &mut e.imagen {
                f.bitmap = None;
            }
        }
    });
}

/// Lee y sube las fotos de `ids` (que estan en `fichas`) que falten. Se
/// llama ANTES del fotograma: crear bitmaps a medio pintar no se puede.
pub(super) fn preparar<'a>(
    raiz: &Path,
    fichas: &[Ficha],
    ids: impl IntoIterator<Item = &'a str>,
    motor: &MotorRender,
) {
    FOTOS.with_borrow_mut(|m| {
        for id in ids {
            let Some(f) = fichas.iter().find(|f| f.id == id) else {
                continue;
            };
            let valor = avatar::valor_de(f);
            // Casi siempre ya esta y no cambio: sin crear cadenas ni mirar
            // el disco en cada fotograma.
            let al_dia = m.get(id).is_some_and(|e| e.valor.as_ref() == valor);
            if !al_dia {
                let a = avatar::de_ficha_o_logo(raiz, f);
                // «Ya tocaba mirar»: se lee en esta misma vuelta.
                let ya = Instant::now()
                    .checked_sub(REINTENTO)
                    .unwrap_or_else(Instant::now);
                let imagen = a
                    .as_ref()
                    .and_then(|a| avatar::ruta_de_la_imagen(raiz, id, a))
                    .map(|r| (r, Imagen::Falta(ya)));
                m.insert(
                    id.to_string(),
                    Entrada {
                        valor: valor.cloned(),
                        avatar: a,
                        imagen,
                    },
                );
            }
            let Some(Entrada {
                imagen: Some((ruta, estado)),
                ..
            }) = m.get_mut(id)
            else {
                continue;
            };
            if let Imagen::Falta(desde) = estado
                && desde.elapsed() >= REINTENTO
            {
                *estado = match leer(ruta) {
                    Some(f) => Imagen::Hecha(f),
                    None => Imagen::Falta(Instant::now()),
                };
            }
            if let Imagen::Hecha(l) = estado
                && l.bitmap.is_none()
            {
                match motor.bitmap_desde_pixeles_premultiplicado(l.lado, l.lado, &l.pixeles) {
                    Ok(b) => l.bitmap = Some(b),
                    Err(e) => tracing::warn!(?e, id, "no se pudo subir la foto del chat"),
                }
            }
        }
    });
}

/// La imagen del disco, cuadrada y en circulo; `None` si no hay o no se lee.
pub(super) fn leer(ruta: &Path) -> Option<Foto> {
    if !ruta.is_file() {
        return None;
    }
    let imagen = pixpin_codec::cargar(ruta)
        .inspect_err(|e| tracing::info!(?e, ruta = %ruta.display(), "foto que no se lee"))
        .ok()?;
    // Se guarda ya cuadrada, pero pudo llegar otra de otro aparato.
    let mut cuadrada = recortar_cuadrado(&imagen)?;
    if cuadrada.ancho > avatar::LADO {
        cuadrada = pixpin_codec::redimensionar(cuadrada, avatar::LADO, avatar::LADO).ok()?;
    }
    en_circulo(&mut cuadrada.pixeles, cuadrada.ancho);
    Some(Foto {
        lado: cuadrada.ancho,
        pixeles: cuadrada.pixeles,
        bitmap: None,
    })
}

impl Foto {
    /// Sube la imagen para pintarla fuera de la cache (el editor).
    pub(super) fn subir(&self, motor: &MotorRender) -> Option<ID2D1Bitmap1> {
        motor
            .bitmap_desde_pixeles_premultiplicado(self.lado, self.lado, &self.pixeles)
            .ok()
    }
}

/// **Pinta la foto de `f` en `caja`** si tiene una propia: su imagen (ya
/// subida), su emoji o sus iniciales sobre el fondo elegido. `false` si se
/// pinta como siempre (o su imagen aun no esta), y quien llama pinta lo
/// suyo: el circulo con iniciales en la lista, la carpeta en la cabecera.
pub(super) fn pintar(p: &Pintor, f: &Ficha, caja: RectF) -> bool {
    FOTOS.with_borrow(|m| {
        let Some(e) = m.get(&f.id) else {
            return false;
        };
        let a = e.avatar.as_ref();
        if let Some((
            _,
            Imagen::Hecha(Foto {
                bitmap: Some(b), ..
            }),
        )) = &e.imagen
        {
            pintar_avatar(p, caja, a, Some(b), &f.nombre, &f.codigo_unico());
            return true;
        }
        match a {
            Some(a) if !a.es_imagen() && !a.es_lo_de_siempre() => {
                pintar_avatar(p, caja, Some(a), None, &f.nombre, &f.codigo_unico());
                true
            }
            _ => false,
        }
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn imagen(ancho: u32, alto: u32) -> ImagenRgba {
        // Cada pixel lleva su columna en R y su fila en G: asi se ve de
        // donde salio cada uno tras recortar.
        let mut pixeles = Vec::new();
        for y in 0..alto {
            for x in 0..ancho {
                pixeles.extend_from_slice(&[x as u8, y as u8, 0, 255]);
            }
        }
        ImagenRgba {
            ancho,
            alto,
            pixeles,
        }
    }

    #[test]
    fn recortar_toma_los_pixeles_del_centro() {
        let r = recortar_cuadrado(&imagen(6, 4)).expect("no esta vacia");
        assert_eq!((r.ancho, r.alto), (4, 4));
        assert_eq!(r.pixeles.len(), 4 * 4 * 4);
        // Arriba a la izquierda es la columna 1 de la original.
        assert_eq!(&r.pixeles[0..2], &[1, 0]);
        // Abajo a la derecha, la columna 4 de la fila 3.
        assert_eq!(
            &r.pixeles[r.pixeles.len() - 4..r.pixeles.len() - 2],
            &[4, 3]
        );
    }

    #[test]
    fn recortar_un_cuadrado_elegido_y_nunca_fuera() {
        let r = recortar(&imagen(6, 4), (2, 1, 3)).unwrap();
        assert_eq!((r.ancho, r.alto), (3, 3));
        assert_eq!(&r.pixeles[0..2], &[2, 1]);
        // Caso negativo: un cuadrado que se sale no se recorta.
        assert!(recortar(&imagen(6, 4), (4, 0, 3)).is_none());
        assert!(recortar(&imagen(6, 4), (0, 0, 0)).is_none());
    }

    #[test]
    fn una_imagen_vacia_o_incoherente_no_se_recorta() {
        assert!(recortar_cuadrado(&imagen(0, 3)).is_none());
        let mut rota = imagen(3, 3);
        rota.pixeles.pop();
        assert!(recortar_cuadrado(&rota).is_none());
    }

    #[test]
    fn el_circulo_deja_las_esquinas_transparentes_y_el_centro_entero() {
        let mut p = imagen(16, 16).pixeles;
        en_circulo(&mut p, 16);
        let alfa = |x: usize, y: usize| p[(y * 16 + x) * 4 + 3];
        assert_eq!(alfa(0, 0), 0);
        assert_eq!(alfa(15, 15), 0);
        assert_eq!(alfa(8, 8), 255);
        // En el borde de arriba, en medio, el circulo toca: casi entero.
        assert!(alfa(8, 0) > 128);
        // Y el color no se toca, solo el alfa.
        assert_eq!(p[0], 0);
        assert_eq!(p[(8 * 16 + 8) * 4], 8);
    }

    #[test]
    fn guardar_deja_un_cuadrado_de_256_apuntado_en_la_ficha() {
        let raiz = std::env::temp_dir().join(format!("pixpin-foto-chat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let f = Ficha::nueva("Casa", 1, "PC01");
        pixpin_proyecto::almacen::Indice {
            proyectos: vec![f.clone()],
            ..Default::default()
        }
        .guardar(&raiz)
        .unwrap();
        let grande = recortar_cuadrado(&imagen(600, 300)).unwrap();
        guardar_imagen(&raiz, &f.id, grande, 42).expect("se guarda");
        let ficha = pixpin_proyecto::almacen::Indice::leer(&raiz)
            .buscar(&f.id)
            .cloned()
            .unwrap();
        let a = avatar::de_ficha(&ficha).unwrap();
        let ruta = avatar::ruta_de_la_imagen(&raiz, &f.id, &a).unwrap();
        let guardado = pixpin_codec::cargar(&ruta).unwrap();
        assert_eq!(
            (guardado.ancho, guardado.alto),
            (avatar::LADO, avatar::LADO)
        );
        assert!(leer(&ruta).is_some());
        // Caso negativo: lo que no es una imagen no se lee como foto.
        let falso = raiz.join("falso.png");
        std::fs::write(&falso, b"no soy una imagen").unwrap();
        assert!(leer(&falso).is_none());
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
