//! El logo de un proyecto: una imagen en lugar del circulo con iniciales.
//!
//! El usuario lo pidio asi: «el logo de los proyectos esta un poco feo asi
//! que dame la opcion de ponerle de logo una imagen». Es solo del PC (el
//! movil no tiene logos de imagen), asi que no viaja: es un `logo.png` dentro
//! de la carpeta del proyecto que sincronizar no mira.
//!
//! Dos reglas que no son evidentes:
//!
//! 1. **Se guarda ya cuadrado y pequeno** (256 de lado, del centro de la
//!    foto). El avatar mide 46 px; guardar la foto de 12 MP para ensenarla
//!    asi seria leer megas en cada arranque.
//! 2. **El circulo va en los pixeles**, no en el dibujo: el pintor no tiene
//!    recorte redondo, y horneando el alfa del circulo al cargarla el logo se
//!    pinta como cualquier bitmap, con su borde suave.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pixpin_codec::ImagenRgba;
use pixpin_render::{Interpolacion, MotorRender, Pintor, RectF};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

/// El nombre del fichero dentro de la carpeta del proyecto.
const NOMBRE: &str = "logo.png";

/// El lado con el que se guarda: el avatar mas grande (46 px) al 200 % y
/// algo de sobra para que reducirlo no deje dientes.
const LADO: u32 = 256;

/// Donde vive el logo de un proyecto.
pub(super) fn ruta(raiz: &Path, id: &str) -> PathBuf {
    pixpin_proyecto::almacen::carpeta(raiz, id).join(NOMBRE)
}

/// Si el proyecto tiene logo puesto: lo pregunta el menu, una vez.
pub(super) fn tiene(raiz: &Path, id: &str) -> bool {
    ruta(raiz, id).is_file()
}

/// Pide una imagen y la deja de logo. `Ok(false)` si se cancelo el dialogo.
pub(super) fn elegir_y_poner(padre: HWND, raiz: &Path, id: &str) -> anyhow::Result<bool> {
    let Some(origen) = pixpin_shell::elegir::pedir_una_imagen(padre) else {
        return Ok(false);
    };
    poner(raiz, id, &origen)?;
    Ok(true)
}

/// Lee `origen`, toma su cuadrado central, lo reduce y lo guarda como logo.
/// Entero o nada: un logo a medio escribir se quedaria roto para siempre.
pub(super) fn poner(raiz: &Path, id: &str, origen: &Path) -> anyhow::Result<()> {
    let imagen = pixpin_codec::cargar(origen)?;
    let cuadrada = recortar_cuadrado(&imagen).ok_or_else(|| anyhow::anyhow!("imagen vacia"))?;
    // Una pequena NO se agranda: solo gastaria disco y se veria igual.
    let cuadrada = if cuadrada.ancho > LADO {
        pixpin_codec::redimensionar(cuadrada, LADO, LADO)?
    } else {
        cuadrada
    };
    let png = pixpin_codec::codificar_png(&cuadrada)?;
    pixpin_sincro::disco::escribir_atomico(&ruta(raiz, id), &png)?;
    olvidar(id);
    Ok(())
}

/// Quita el logo: vuelve el circulo con iniciales.
pub(super) fn quitar(raiz: &Path, id: &str) -> std::io::Result<()> {
    let r = ruta(raiz, id);
    let hecho = match std::fs::remove_file(&r) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        otro => otro,
    };
    olvidar(id);
    hecho
}

/// El cuadrado mas grande del centro de una imagen: `(x, y, lado)`.
fn cuadrado_central(ancho: u32, alto: u32) -> (u32, u32, u32) {
    let lado = ancho.min(alto);
    ((ancho - lado) / 2, (alto - lado) / 2, lado)
}

/// La imagen recortada a su cuadrado central. `None` si esta vacia o sus
/// pixeles no cuadran con su tamano.
fn recortar_cuadrado(imagen: &ImagenRgba) -> Option<ImagenRgba> {
    if imagen.ancho == 0 || imagen.alto == 0 || imagen.pixeles.len() != imagen.bytes_esperados() {
        return None;
    }
    let (x, y, lado) = cuadrado_central(imagen.ancho, imagen.alto);
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

// --- La cache, del hilo del chat ----------------------------------------

/// Un logo leido: sus pixeles (ya en circulo) y, si ya se subio, su bitmap.
/// Los pixeles se quedan para volver a subirlo si se pierde el dispositivo.
struct Logo {
    pixeles: Vec<u8>,
    lado: u32,
    bitmap: Option<ID2D1Bitmap1>,
}

/// Lo que se sabe del logo de un proyecto. `Sin` vale tanto para «no tiene»
/// como para «no se pudo leer»: en los dos se pinta el circulo de siempre,
/// y no se vuelve a mirar el disco en cada fotograma.
enum Estado {
    Hecho(Logo),
    Sin,
}

thread_local! {
    /// Por `id` de proyecto. Del hilo del chat, que es el que pinta, como
    /// la cache del grafito: el menu que pone o quita un logo corre en el
    /// mismo hilo y lo olvida aqui.
    static LOGOS: RefCell<HashMap<String, Estado>> = RefCell::new(HashMap::new());
}

/// Olvida lo leido de un proyecto: se vuelve a leer al pintarlo.
pub(super) fn olvidar(id: &str) {
    LOGOS.with_borrow_mut(|m| m.remove(id));
}

/// Suelta los bitmaps y conserva los pixeles: el dispositivo se perdio.
pub(super) fn soltar() {
    LOGOS.with_borrow_mut(|m| {
        for e in m.values_mut() {
            if let Estado::Hecho(l) = e {
                l.bitmap = None;
            }
        }
    });
}

/// Lee y sube los logos de `ids` que falten. Se llama ANTES del fotograma:
/// crear bitmaps a medio pintar no se puede.
pub(super) fn preparar<'a>(
    raiz: &Path,
    ids: impl IntoIterator<Item = &'a str>,
    motor: &MotorRender,
) {
    LOGOS.with_borrow_mut(|m| {
        for id in ids {
            // Sin `entry`: esto corre en cada fotograma y casi siempre ya
            // esta, asi que no se crea la cadena para nada.
            if !m.contains_key(id) {
                let estado = match leer(&ruta(raiz, id)) {
                    Some(l) => Estado::Hecho(l),
                    None => Estado::Sin,
                };
                m.insert(id.to_string(), estado);
            }
            let Some(Estado::Hecho(l)) = m.get_mut(id) else {
                continue;
            };
            if l.bitmap.is_none() {
                match motor.bitmap_desde_pixeles_premultiplicado(l.lado, l.lado, &l.pixeles) {
                    Ok(b) => l.bitmap = Some(b),
                    Err(e) => tracing::warn!(?e, id, "no se pudo subir el logo"),
                }
            }
        }
    });
}

/// El logo del disco, cuadrado y en circulo; `None` si no hay o no se lee.
fn leer(ruta: &Path) -> Option<Logo> {
    if !ruta.is_file() {
        return None;
    }
    let imagen = pixpin_codec::cargar(ruta)
        .inspect_err(|e| tracing::info!(?e, ruta = %ruta.display(), "logo que no se lee"))
        .ok()?;
    // Lo guarda `poner` ya cuadrado, pero alguien pudo dejar otro a mano.
    let mut cuadrada = recortar_cuadrado(&imagen)?;
    if cuadrada.ancho > LADO {
        cuadrada = pixpin_codec::redimensionar(cuadrada, LADO, LADO).ok()?;
    }
    en_circulo(&mut cuadrada.pixeles, cuadrada.ancho);
    Some(Logo {
        lado: cuadrada.ancho,
        pixeles: cuadrada.pixeles,
        bitmap: None,
    })
}

/// Pinta el logo de `id` en `caja` si lo hay ya subido. `false` si no, y
/// quien llama pinta el circulo de siempre.
pub(super) fn pintar(p: &Pintor, id: &str, caja: RectF) -> bool {
    LOGOS.with_borrow(|m| match m.get(id) {
        Some(Estado::Hecho(Logo {
            bitmap: Some(b), ..
        })) => {
            // Cubica: se reduce de 256 a 46, y la lineal dejaria dientes.
            p.bitmap_con(b, caja, None, Interpolacion::Cubica);
            true
        }
        _ => false,
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
    fn el_cuadrado_sale_del_centro_por_el_lado_que_sobra() {
        assert_eq!(cuadrado_central(400, 300), (50, 0, 300));
        assert_eq!(cuadrado_central(300, 400), (0, 50, 300));
        assert_eq!(cuadrado_central(256, 256), (0, 0, 256));
        // Impar: el pixel de mas se queda a la derecha.
        assert_eq!(cuadrado_central(5, 2), (1, 0, 2));
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
    fn poner_guarda_un_cuadrado_de_256_y_quitar_lo_borra() {
        let raiz = std::env::temp_dir().join(format!("pixpin-logo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        let origen = raiz.join("foto.png");
        let grande = imagen(600, 300);
        std::fs::write(&origen, pixpin_codec::codificar_png(&grande).unwrap()).unwrap();
        poner(&raiz, "p1", &origen).expect("se guarda");
        assert!(tiene(&raiz, "p1"));
        let guardado = pixpin_codec::cargar(&ruta(&raiz, "p1")).unwrap();
        assert_eq!((guardado.ancho, guardado.alto), (LADO, LADO));
        quitar(&raiz, "p1").unwrap();
        assert!(!tiene(&raiz, "p1"));
        // Quitar lo que no esta no es un error.
        quitar(&raiz, "p1").unwrap();
        // Un fichero que no es imagen se rechaza sin dejar logo.
        let falso = raiz.join("falso.png");
        std::fs::write(&falso, b"no soy una imagen").unwrap();
        assert!(poner(&raiz, "p1", &falso).is_err());
        assert!(!tiene(&raiz, "p1"));
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
