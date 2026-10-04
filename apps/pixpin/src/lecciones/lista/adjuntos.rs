//! **Las fotos y notas de voz de una leccion** y su tarjeta para pinear.
//!
//! Los adjuntos son mensajes del chat que responden a la leccion (los ids
//! van en `Leccion::adjuntos`, como en el movil, `AdjuntosDeLaLeccion.kt`):
//! una foto (`IMAGEN`) o una nota de voz (`VOZ`). Aqui se buscan en el
//! cuaderno de su chat, se resuelve su fichero como lo resuelve el chat y
//! las fotos se reducen a miniatura una vez por leccion.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use pixpin_lecciones::Leccion;
use pixpin_proyecto::cuaderno::{self, Clase as ClaseMensaje};
use pixpin_render::{MotorRender, RectF};
use pixpin_store::Catalogo;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

use super::super::almacen::Entrada;
use super::super::ui::{self, v2};

/// Lado mayor de una miniatura: la celda mide 112 x 72 logicos y la
/// pantalla puede ir al 200 %.
const LADO: u32 = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clase {
    Foto,
    Voz,
}

#[derive(Clone)]
pub struct Adjunto {
    pub clase: Clase,
    pub ruta: PathBuf,
    /// La miniatura de una foto, ya en la GPU.
    pub bitmap: Option<ID2D1Bitmap1>,
    pub ancho: u32,
    pub alto: u32,
}

/// Lo ya buscado, por leccion y por la lista de sus adjuntos: si cambia (se
/// anade una foto), se busca otra vez.
#[derive(Default)]
pub struct Cache {
    por_leccion: HashMap<(String, Vec<String>), Vec<Adjunto>>,
}

impl Cache {
    /// Los adjuntos de la leccion `x` que estan en este equipo (los que aun
    /// no llegaron sincronizando no salen).
    pub fn de(&mut self, raiz: &Path, x: &Entrada, motor: Option<&MotorRender>) -> &Vec<Adjunto> {
        let clave = (x.leccion.id.clone(), x.leccion.adjuntos.clone());
        self.por_leccion.entry(clave).or_insert_with(|| buscar(raiz, x, motor))
    }

    pub fn olvidar(&mut self, id: &str) {
        self.por_leccion.retain(|(l, _), _| l != id);
    }
}

fn buscar(raiz: &Path, x: &Entrada, motor: Option<&MotorRender>) -> Vec<Adjunto> {
    if x.leccion.adjuntos.is_empty() {
        return Vec::new();
    }
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &x.ficha);
    let Ok(c) = cuaderno::Cuaderno::leer_de(&carpeta) else {
        return Vec::new();
    };
    let mut v = Vec::new();
    for id in &x.leccion.adjuntos {
        let Some(m) = c.mensajes.iter().find(|m| m.id == *id) else {
            continue;
        };
        let clase = match m.clase {
            Some(ClaseMensaje::Imagen) => Clase::Foto,
            Some(ClaseMensaje::Voz) => Clase::Voz,
            _ => continue,
        };
        let Some(ruta) = m
            .ruta
            .as_deref()
            .and_then(|r| pixpin_proyecto::vista::ruta_real(raiz, &x.ficha, r))
            .filter(|r| r.is_file())
        else {
            continue;
        };
        let (bitmap, ancho, alto) = match (clase, motor) {
            (Clase::Foto, Some(motor)) => miniatura(&ruta, motor).map_or((None, 0, 0), |(b, w, h)| (Some(b), w, h)),
            _ => (None, 0, 0),
        };
        v.push(Adjunto {
            clase,
            ruta,
            bitmap,
            ancho,
            alto,
        });
    }
    v
}

fn miniatura(ruta: &Path, motor: &MotorRender) -> Option<(ID2D1Bitmap1, u32, u32)> {
    let entera = pixpin_codec::imagen::cargar(ruta).ok()?;
    let (w, h) = (entera.ancho, entera.alto);
    if w == 0 || h == 0 {
        return None;
    }
    let mayor = w.max(h);
    let img = if mayor > LADO {
        let f = LADO as f64 / mayor as f64;
        let nw = ((w as f64 * f).round() as u32).max(1);
        let nh = ((h as f64 * f).round() as u32).max(1);
        pixpin_codec::redimensionar(entera, nw, nh).ok()?
    } else {
        entera
    };
    let b = motor.bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles).ok()?;
    Some((b, img.ancho, img.alto))
}

/// Donde va la foto en `celda`, recortada para llenarla sin deformarse:
/// el trozo del bitmap que se pinta.
pub fn recorte(ancho: u32, alto: u32, celda: RectF) -> RectF {
    let (w, h) = (ancho.max(1) as f32, alto.max(1) as f32);
    let f = (celda.ancho / w).max(celda.alto / h);
    let (vw, vh) = (celda.ancho / f, celda.alto / f);
    RectF {
        x: (w - vw) / 2.0,
        y: (h - vh) / 2.0,
        ancho: vw,
        alto: vh,
    }
}

/// **La tarjeta de una leccion, en PNG**, para pinearla: titulo y los tres
/// bloques, como en la ficha. Se escribe en la carpeta temporal; la ventana
/// principal la copia a su pin.
pub fn tarjeta_png(motor: &MotorRender, d3d: &ID3D11Device, l: &Leccion, textos: &Catalogo) -> Result<PathBuf> {
    const ANCHO: f32 = 460.0;
    const PAD: f32 = 20.0;
    let interior = ANCHO - 2.0 * PAD;
    let bloques: Vec<(String, pixpin_render::Color, String)> = [
        (textos.t("lec2-que-paso"), v2::CIAN, l.que_paso.clone()),
        (textos.t("lec2-por-que"), v2::NARANJA, l.por_que.clone()),
        (textos.t("lec2-proxima"), v2::VERDE, l.proxima.clone()),
    ]
    .into_iter()
    .filter(|(_, _, t)| !t.trim().is_empty())
    .collect();
    let (_, ht) = motor.medir_texto(&l.titulo, 20.0, interior);
    let mut alto = PAD + ht + 12.0;
    for (_, _, t) in &bloques {
        alto += 20.0 + motor.medir_texto(t, 15.0, interior).1 + 14.0;
    }
    alto += PAD - 4.0;
    let (w, h) = (ANCHO.ceil() as u32, alto.ceil() as u32);
    let fuera = pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(motor, d3d, w, h).context("sin superficie")?;
    motor
        .dibujar(&fuera.destino, |p| {
            p.limpiar(v2::COLUMNA);
            p.rellenar(RectF { x: 0.0, y: 0.0, ancho: 4.0, alto }, ui::color_de_gravedad(l.gravedad));
            let mut y = PAD;
            ui::negrita(p, &l.titulo, PAD, y, 20.0, interior, v2::TEXTO);
            y += ht + 12.0;
            for (rotulo, color, t) in &bloques {
                ui::negrita(p, &rotulo.to_uppercase(), PAD, y, 11.5, interior, *color);
                y += 20.0;
                p.texto_ajustado(t, PAD, y, 15.0, interior, v2::CUERPO);
                y += p.medir_texto_ajustado(t, 15.0, interior).1 + 14.0;
            }
        })
        .context("no se pudo pintar")?;
    fuera.esperar_gpu().context("la GPU no acabo")?;
    let (_, _, pixeles) = fuera.leer_rgba().context("no se pudo leer")?;
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba { ancho: w, alto: h, pixeles })
        .context("no se pudo codificar")?;
    let carpeta = std::env::temp_dir().join("pixpin-lecciones");
    std::fs::create_dir_all(&carpeta)?;
    let ruta = carpeta.join(format!("{}.png", l.id));
    std::fs::write(&ruta, png)?;
    Ok(ruta)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_miniatura_llena_la_celda_sin_deformarse() {
        let celda = RectF { x: 10.0, y: 10.0, ancho: 112.0, alto: 72.0 };
        // Una foto alta: se corta arriba y abajo, centrada.
        let r = recorte(240, 320, celda);
        assert!((r.ancho - 240.0).abs() < 0.01);
        assert!((r.alto - 240.0 * 72.0 / 112.0).abs() < 0.01);
        assert!((r.y - (320.0 - r.alto) / 2.0).abs() < 0.01);
        // Una apaisada: se corta a los lados.
        let a = recorte(240, 100, celda);
        assert!((a.alto - 100.0).abs() < 0.01 && a.x > 0.0);
        // Caso negativo: una de tamano cero no divide entre cero.
        let z = recorte(0, 0, celda);
        assert!(z.ancho.is_finite() && z.alto.is_finite());
    }
}
