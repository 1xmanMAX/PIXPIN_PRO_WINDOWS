//! **Los ojos del asistente: el lienzo pintado para el modelo** (15-oct-2026).
//!
//! Lo pide `ver_lienzo` y la revision del turno (`pixpin_ia::ojos`). Se
//! pinta con **el mismo motor del lienzo** —`dibujo::pintar::pintar_escena`
//! sobre Direct2D, lo mismo que ve el usuario en el editor, no un pintor
//! aparte— en una textura sin ventana ([`FueraDePantalla`]), y encima una
//! rejilla ligera numerada en las coordenadas del modelo y la etiqueta `#id`
//! de cada elemento, para que pueda referirse a lo que ve.
//!
//! ## Latencia
//!
//! - Quien tiene la escena (el editor abierto o el chat) solo **copia** lo
//!   que cae en la zona y las imagenes que usa ([`encargo_de`]): microsegundos
//!   en su hilo. Pintar no ocurre en el hilo de ninguna ventana.
//! - Pinta un **hilo propio** que guarda el dispositivo de la GPU entre una
//!   vista y otra ([`pintar`]): crearlo cuesta lo que una vista entera, y se
//!   puede adelantar al empezar el turno ([`precalentar`]). Medido en este
//!   PC: ver `muestras_de_los_ojos` y el banco (`ojos/banco.rs`).

use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use pixpin_codec::ImagenRgba;
use pixpin_ia::edicion::Contexto;
use pixpin_ia::ojos::{self, Mirada, Vista};
use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::Figura;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_render::{Color, MotorRender, RectF};

/// Los pixeles de una imagen del lienzo: ya en memoria (el editor abierto)
/// o en su fichero (un lienzo cerrado).
#[derive(Debug, Clone, PartialEq)]
pub enum Pixeles {
    Rgba(ImagenRgba),
    Ruta(PathBuf),
}

/// **Lo que hay que pintar**, ya copiado de quien tiene la escena.
#[derive(Debug, Clone, PartialEq)]
pub struct Encargo {
    /// Solo lo que cae en la zona, con sus ids de siempre.
    pub escena: Escena,
    pub imagenes: Vec<(u64, Pixeles)>,
    /// Lo que se mira, en coordenadas de la escena.
    pub caja: (f32, f32, f32, f32),
    /// Desde donde mide el modelo (para numerar la rejilla).
    pub marco: (f32, f32),
    /// Una hoja de documento debajo (un PDF anotado) y donde va.
    pub fondo: Option<(ImagenRgba, (f32, f32, f32, f32))>,
    pub marcas: bool,
}

/// **Copia lo que hace falta para pintar una mirada**: los elementos que
/// caen en su caja (con un poco de holgura por los trazos) y las imagenes
/// que usan. `pixeles(id)` da los de cada imagen.
pub fn encargo_de(
    escena: &Escena,
    m: &Mirada,
    ctx: &Contexto,
    pixeles: impl Fn(u64) -> Option<Pixeles>,
) -> Result<Encargo, String> {
    let caja = ojos::caja_de(escena, m, ctx)?;
    Ok(copiar(escena, caja, ctx.marco, m.marcas, pixeles))
}

/// La copia de la zona `caja` de una escena.
pub fn copiar(
    escena: &Escena,
    caja: (f32, f32, f32, f32),
    marco: (f32, f32),
    marcas: bool,
    pixeles: impl Fn(u64) -> Option<Pixeles>,
) -> Encargo {
    let mut copia = Escena::nueva();
    copia.fondo = escena.fondo;
    copia.escala = escena.escala.clone();
    copia.referencias_visibles = escena.referencias_visibles;
    copia.siguiente_id = escena.siguiente_id;
    let mut imagenes = Vec::new();
    for e in escena.a_la_vista() {
        let (x0, y0, x1, y1) = e.caja();
        let h = e.grosor * 2.0 + 4.0;
        if x1 + h < caja.0 || x0 - h > caja.2 || y1 + h < caja.1 || y0 - h > caja.3 {
            continue;
        }
        if let Figura::Imagen { id_objeto } = e.figura
            && !imagenes.iter().any(|(id, _)| *id == id_objeto)
            && let Some(p) = pixeles(id_objeto)
        {
            imagenes.push((id_objeto, p));
        }
        copia.elementos.push(e.clone());
    }
    Encargo { escena: copia, imagenes, caja, marco, fondo: None, marcas }
}

// ---------------------------------------------------------------------------
// El hilo que pinta

enum Orden {
    Pintar(Box<Encargo>, Sender<Result<Vista, String>>),
    Calentar,
}

/// La GPU del hilo: se crea con la primera vista y se guarda.
struct Gpu {
    _dispositivo: pixpin_capture::Dispositivo,
    motor: MotorRender,
}

impl Gpu {
    fn nueva() -> Result<Gpu, String> {
        let d = pixpin_capture::Dispositivo::nuevo().map_err(|e| format!("no hay GPU para pintar la vista: {e:?}"))?;
        let motor = MotorRender::nuevo(d.d3d()).map_err(|e| format!("no se pudo preparar el pintor: {e:?}"))?;
        Ok(Gpu { _dispositivo: d, motor })
    }
}

/// Cuanto se espera una vista como mucho (con la GPU ocupada, mejor seguir
/// sin vista que colgar el turno).
pub const ESPERA_VISTA: Duration = Duration::from_secs(30);

fn cola() -> Option<Sender<Orden>> {
    static COLA: OnceLock<Mutex<Option<Sender<Orden>>>> = OnceLock::new();
    let m = COLA.get_or_init(|| {
        let (tx, rx) = channel::<Orden>();
        let lanzado = std::thread::Builder::new().name("asistente-ojos".into()).spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let mut gpu: Option<Gpu> = None;
            for orden in rx {
                let tg = Instant::now();
                if gpu.is_none() {
                    gpu = Gpu::nueva().map_err(|e| tracing::warn!(%e, "asistente: ojos sin GPU")).ok();
                }
                tracing::debug!(ms = tg.elapsed().as_millis(), "asistente: GPU de los ojos lista");
                let Orden::Pintar(enc, responder) = orden else { continue };
                let r = match gpu.as_ref() {
                    Some(g) => pintar_con(g, &enc),
                    None => Err("no hay GPU para pintar la vista".to_string()),
                };
                // Un dispositivo perdido se rehace una vez.
                let r = match r {
                    Err(e) if e.contains("dispositivo") => {
                        gpu = Gpu::nueva().ok();
                        gpu.as_ref().map_or(Err(e), |g| pintar_con(g, &enc))
                    }
                    r => r,
                };
                let _ = responder.send(r);
            }
        });
        Mutex::new(lanzado.ok().map(|_| tx))
    });
    m.lock().ok().and_then(|g| g.clone())
}

/// **Deja la GPU lista** para la primera vista (al empezar un turno que
/// puede mirar): no espera.
pub fn precalentar() {
    if let Some(tx) = cola() {
        let _ = tx.send(Orden::Calentar);
    }
}

/// Cuantas vistas se pidieron en el proceso (el banco las cuenta).
pub static PINTADAS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// **Pinta un encargo** en el hilo de los ojos y espera la imagen.
pub fn pintar(enc: Encargo) -> Result<Vista, String> {
    PINTADAS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let tx = cola().ok_or("no se pudo arrancar el pintor de la vista")?;
    let (responder, rx) = channel();
    tx.send(Orden::Pintar(Box::new(enc), responder)).map_err(|_| "el pintor de la vista no esta")?;
    rx.recv_timeout(ESPERA_VISTA).map_err(|_| "la vista no llego a tiempo".to_string())?
}

/// Un color de 0-1.
const fn color(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color { r, g, b, a }
}

/// **Pinta de verdad**: papel, documento debajo si lo hay, la escena con el
/// motor del lienzo, y la rejilla y las etiquetas encima.
fn pintar_con(g: &Gpu, enc: &Encargo) -> Result<Vista, String> {
    let t0 = Instant::now();
    let (x0, y0, x1, y1) = enc.caja;
    let (ancho, alto, s) = ojos::tamano(enc.caja, ojos::LADO_MAXIMO);
    let d3d = g._dispositivo.d3d();
    let fuera = FueraDePantalla::nuevo(&g.motor, d3d, ancho, alto).map_err(|e| format!("dispositivo: {e:?}"))?;
    let mut imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(4096);
    for (id, p) in &enc.imagenes {
        let img = match p {
            Pixeles::Rgba(i) => Some(i.clone()),
            Pixeles::Ruta(r) => crate::imagenes_lienzo::imagen_de_rutas(std::slice::from_ref(r)),
        };
        if let Some(i) = img {
            imagenes.guardar_con_id(*id, i);
        }
    }
    imagenes.asegurar(&g.motor);
    let t_prep = t0.elapsed().as_millis();
    let fondo = enc.fondo.as_ref().and_then(|(img, r)| {
        g.motor
            .bitmap_desde_pixeles_premultiplicado(img.ancho, img.alto, &img.pixeles)
            .ok()
            .map(|b| (b, *r))
    });
    let papel = enc.escena.fondo;
    let marcas = enc.marcas.then(|| (ojos::paso_de_rejilla(enc.caja), ojos::etiquetas(&enc.escena, enc.caja)));
    g.motor
        .dibujar(&fuera.destino, |p| {
            // Un papel transparente se ve blanco, como en el editor.
            p.limpiar(color(1.0, 1.0, 1.0, 1.0));
            if papel.a > 0.01 {
                p.limpiar(crate::dibujo::pintar::a_color(papel));
            }
            p.poner_vista((-x0 * s, -y0 * s), s, (0.0, 0.0));
            if let Some((b, (a, bb, c, d))) = &fondo {
                p.bitmap(b, RectF { x: *a, y: *bb, ancho: c - a, alto: d - bb }, None, false);
            }
            let mut cache = pixpin_motor2d::cache::Cache::nueva();
            let mut tinta = pixpin_render::CacheTinta::nueva();
            crate::dibujo::pintar::pintar_escena(p, &enc.escena, &mut cache, &mut tinta, &imagenes, enc.caja, s, |_| false);
            p.poner_vista((0.0, 0.0), 1.0, (0.0, 0.0));
            if let Some((paso, etiquetas)) = &marcas {
                pintar_marcas(p, enc, s, *paso, etiquetas, (ancho as f32, alto as f32));
            }
        })
        .map_err(|e| format!("dispositivo: {e:?}"))?;
    let t_dib = t0.elapsed().as_millis();
    fuera.esperar_gpu().map_err(|e| format!("dispositivo: {e:?}"))?;
    let t_gpu = t0.elapsed().as_millis();
    let (w, h, rgba) = fuera.leer_rgba().map_err(|e| format!("dispositivo: {e:?}"))?;
    let png = pixpin_codec::imagen::codificar_png(&ImagenRgba { ancho: w, alto: h, pixeles: rgba }).map_err(|e| format!("{e:?}"))?;
    let ms = t0.elapsed().as_millis();
    tracing::debug!(t_prep, t_dib, t_gpu, ms, "asistente: tiempos de la vista");
    let texto = ojos::leyenda(enc.caja, enc.marco, w, h, s, enc.marcas);
    tracing::info!(ms, ancho = w, alto = h, kb = png.len() / 1024, elementos = enc.escena.elementos.len(), "asistente: vista del lienzo");
    let _ = (x1, y1);
    Ok(Vista { png, ancho: w, alto: h, texto, ms })
}

/// La rejilla numerada (en las coordenadas del modelo) y las etiquetas.
fn pintar_marcas(p: &pixpin_render::Pintor<'_>, enc: &Encargo, s: f32, paso: f32, etiquetas: &[(u64, (f32, f32))], (w, h): (f32, f32)) {
    let (x0, y0, x1, y1) = enc.caja;
    let (mx, my) = enc.marco;
    let raya = color(0.45, 0.5, 0.58, 0.28);
    let numero = color(0.25, 0.28, 0.33, 1.0);
    let fondo_numero = color(1.0, 1.0, 1.0, 0.75);
    let tam = 11.0;
    let mut u = ((x0 - mx) / paso).ceil() * paso;
    while u + mx <= x1 {
        let px = (u + mx - x0) * s;
        p.linea((px, 0.0), (px, h), 1.0, raya);
        p.texto_con_fondo(&format!("{}", u.round() as i64), px + 3.0, 3.0, tam, numero, fondo_numero);
        u += paso;
    }
    let mut v = ((y0 - my) / paso).ceil() * paso;
    while v + my <= y1 {
        let py = (v + my - y0) * s;
        p.linea((0.0, py), (w, py), 1.0, raya);
        if py > 18.0 {
            p.texto_con_fondo(&format!("{}", v.round() as i64), 3.0, py + 2.0, tam, numero, fondo_numero);
        }
        v += paso;
    }
    let amarillo = color(1.0, 0.86, 0.25, 0.92);
    let negro = color(0.08, 0.08, 0.08, 1.0);
    for (id, (x, y)) in etiquetas {
        let texto = format!("#{id}");
        let (tw, th) = p.medir_texto(&texto, tam);
        // Encima de la esquina, sin salirse de la imagen.
        let px = ((x - x0) * s).clamp(2.0, (w - tw - 6.0).max(2.0));
        let py = ((y - y0) * s - th - 3.0).clamp(2.0, (h - th - 4.0).max(2.0));
        p.texto_con_fondo(&texto, px + 3.0, py + 1.0, tam, negro, amarillo);
    }
}

#[cfg(test)]
mod pruebas;

#[cfg(test)]
mod banco;
