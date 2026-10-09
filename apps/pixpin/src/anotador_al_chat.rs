//! **Guardar lo anotado sobre la pantalla en «Mensajes guardados».**
//!
//! Lo pidio el usuario el 2026-09-28: «que se tome captura de pantalla al
//! fondo y las tintas de encima se mantengan editables y se plasmen dentro de
//! un canvas; esto se envia directamente al chat a Mensajes guardados,
//! aparezca como si hubiera subido una imagen y hubiera anotado sobre ella».
//!
//! Por eso no se inventa un formato: es **una foto del chat con su lienzo**,
//! lo mismo que deja el movil al anotar una foto (K11,
//! `pixpin_proyecto::lienzo_de_la_foto`):
//!
//! 1. la captura de la pantalla SIN la tinta ni la barra entra en «Mensajes
//!    guardados» como un mensaje IMAGEN corriente (`meter_en_proyecto`, que
//!    usa `cuaderno::anadir`, con su cerrojo);
//! 2. se le crea su lienzo `foto-<id>` con la captura como primer elemento
//!    (bloqueada, a la medida con que el movil la carga) y la tinta encima
//!    como elementos sueltos y editables, llevada de pixeles de la captura a
//!    unidades del lienzo (`lienzo_de_la_foto::con_dibujo`), y se apunta la
//!    `referencia` del mensaje.
//!
//! La burbuja ensena la foto con la tinta (`foto_anotada::dibujo_de_la_foto`)
//! y al pulsarla se abre ese lienzo y se sigue editando. Y viaja al movil
//! como cualquier foto anotada alli.
//!
//! **Se guarda solo al salir** (2026-09-29: «el anotar sobre la pantalla ...
//! tiene que guardarse automaticamente en el canvas dentro de Mensajes
//! guardados»): Escape, Salir o cerrar con tinta nueva lo llevan al chat sin
//! preguntar ([`EnElChat`], `pantalla::guardar_en_el_chat`). El boton de la
//! pastilla se queda como «guardar ahora»; lo dibujado despues pone al dia
//! EL MISMO lienzo ([`poner_al_dia`]), no manda otro mensaje. Sin tinta no
//! se guarda nada.
//!
//! **Varios monitores**: una sola captura del escritorio virtual entero, no
//! una por monitor. La ventana del anotador cubre el escritorio virtual y la
//! tinta vive en sus pixeles (camara quieta a 1:1, `pantalla::camara_quieta`),
//! asi que una flecha que cruza de un monitor a otro cae entera en UNA foto y
//! en las mismas coordenadas, sin partirla ni recolocarla. Lo que ningun
//! monitor cubre (un escritorio en L) sale negro, como en la captura
//! congelada de siempre (`pantalla::unir_fotos`).

use std::io;
use std::path::{Path, PathBuf};

use pixpin_motor2d::Elemento;
use pixpin_proyecto::almacen;
#[cfg(test)]
use pixpin_proyecto::cuaderno;

/// El nombre del fichero de la captura en el chat. Uno fijo: el almacen le
/// pone `(1)`, `(2)`... si ya hay otro (`almacen::guardar_adjunto`).
pub const NOMBRE_DE_LA_CAPTURA: &str = "pantalla-anotada.png";

/// Lo que se guarda: la captura de debajo y la tinta de encima, en pixeles
/// de la captura.
pub struct Sesion {
    pub foto: pixpin_codec::ImagenRgba,
    pub tinta: Vec<Elemento>,
    /// Las imagenes pegadas durante la edicion: van DENTRO de la foto
    /// ([`componer`]), porque el lienzo de la foto solo lleva la foto como
    /// fichero. Antes no se guardaban (el usuario, 8-oct-2026).
    pub pegadas: Vec<Pegada>,
}

/// Una imagen pegada en el anotador: el elemento (sitio, tamano, giro) y
/// sus pixeles.
#[derive(Clone)]
pub struct Pegada {
    pub elemento: Elemento,
    pub imagen: pixpin_codec::ImagenRgba,
}

/// **Pura**: el trozo de la foto que merece guardarse: la union de los
/// monitores (en pixeles de la foto) que tocan algo dibujado o pegado.
///
/// La foto es el escritorio virtual entero, todos los monitores en fila; el
/// movil reduce cualquier foto hasta que su lado largo no pasa de 2048 para
/// dibujar encima (`lienzo_de_la_foto::LADO_MAXIMO`), y tres monitores en
/// fila quedaban a un cuarto: el texto llegaba ilegible (el usuario,
/// 8-oct-2026). Con solo el monitor donde se dibujo, llega a su tamano.
/// `None`: entera (nada dibujado, o tocan todos).
pub fn monitores_con_dibujo(
    monitores: &[pixpin_geom::Rect],
    cajas: &[(f32, f32, f32, f32)],
    foto: (u32, u32),
) -> Option<pixpin_geom::Rect> {
    let toca = |m: &pixpin_geom::Rect| {
        cajas.iter().any(|&(x0, y0, x1, y1)| {
            x0 < m.derecha() as f32 && x1 > m.x as f32 && y0 < m.abajo() as f32 && y1 > m.y as f32
        })
    };
    let union = monitores
        .iter()
        .filter(|m| toca(m))
        .copied()
        .reduce(|a, b| a.union(b))?;
    // Dentro de la foto.
    let x0 = union.x.max(0);
    let y0 = union.y.max(0);
    let x1 = union.derecha().min(foto.0 as i32);
    let y1 = union.abajo().min(foto.1 as i32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let r = pixpin_geom::Rect {
        x: x0,
        y: y0,
        ancho: (x1 - x0) as u32,
        alto: (y1 - y0) as u32,
    };
    (r.ancho < foto.0 || r.alto < foto.1).then_some(r)
}

/// El trozo `r` de `img`.
pub fn recortar(img: &pixpin_codec::ImagenRgba, r: pixpin_geom::Rect) -> pixpin_codec::ImagenRgba {
    let mut pixeles = Vec::with_capacity((r.ancho * r.alto * 4) as usize);
    for y in r.y..r.y + r.alto as i32 {
        let i = ((y as u32 * img.ancho + r.x as u32) * 4) as usize;
        pixeles.extend_from_slice(&img.pixeles[i..i + (r.ancho * 4) as usize]);
    }
    pixpin_codec::ImagenRgba {
        ancho: r.ancho,
        alto: r.alto,
        pixeles,
    }
}

/// Corre la tinta y las imagenes pegadas `(-dx, -dy)`: del escritorio al
/// trozo guardado.
fn correr(tinta: &mut [Elemento], pegadas: &mut [Pegada], dx: f32, dy: f32) {
    if dx == 0.0 && dy == 0.0 {
        return;
    }
    for e in tinta.iter_mut() {
        e.mover(-dx, -dy);
    }
    for p in pegadas.iter_mut() {
        p.elemento.mover(-dx, -dy);
    }
}

/// **Pura**: la foto con las imagenes pegadas encima, cada una en su caja
/// (en pixeles de la foto, que son los del anotador), con su giro, su
/// recorte y su opacidad, en el orden en que estan.
pub fn componer(foto: &pixpin_codec::ImagenRgba, pegadas: &[Pegada]) -> pixpin_codec::ImagenRgba {
    let mut out = foto.clone();
    let (fw, fh) = (foto.ancho as i64, foto.alto as i64);
    for p in pegadas {
        let e = &p.elemento;
        let img = &p.imagen;
        if img.ancho == 0 || img.alto == 0 || e.ancho.abs() < 1.0 || e.alto.abs() < 1.0 {
            continue;
        }
        // El trozo de la imagen que se ve (el recorte), en sus pixeles.
        let (sx0, sy0, sx1, sy1) = e
            .extras
            .recorte
            .and_then(|r| r.trozo_en(img.ancho as f32, img.alto as f32))
            .unwrap_or((0.0, 0.0, img.ancho as f32, img.alto as f32));
        let (x, y, w, h) = (
            e.x.min(e.x + e.ancho),
            e.y.min(e.y + e.alto),
            e.ancho.abs(),
            e.alto.abs(),
        );
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let (sen, cos) = e.angulo.sin_cos();
        // La caja girada, para no recorrer la foto entera.
        let r = ((w * w + h * h).sqrt() / 2.0).ceil();
        let (bx0, by0) = (
            ((cx - r).floor() as i64).max(0),
            ((cy - r).floor() as i64).max(0),
        );
        let (bx1, by1) = (
            ((cx + r).ceil() as i64).min(fw),
            ((cy + r).ceil() as i64).min(fh),
        );
        let opacidad = e.opacidad.clamp(0.0, 1.0);
        for py in by0..by1 {
            for px in bx0..bx1 {
                // Del pixel de la foto al de la imagen: deshacer el giro.
                let (dx, dy) = (px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
                let (lx, ly) = (
                    dx * cos + dy * sen + w / 2.0,
                    -dx * sen + dy * cos + h / 2.0,
                );
                if lx < 0.0 || ly < 0.0 || lx >= w || ly >= h {
                    continue;
                }
                let ix = (sx0 + lx / w * (sx1 - sx0))
                    .floor()
                    .clamp(0.0, img.ancho as f32 - 1.0) as usize;
                let iy = (sy0 + ly / h * (sy1 - sy0))
                    .floor()
                    .clamp(0.0, img.alto as f32 - 1.0) as usize;
                let s = (iy * img.ancho as usize + ix) * 4;
                let d = (py as usize * foto.ancho as usize + px as usize) * 4;
                let (Some(src), true) = (img.pixeles.get(s..s + 4), d + 4 <= out.pixeles.len())
                else {
                    continue;
                };
                let a = src[3] as f32 / 255.0 * opacidad;
                if a <= 0.0 {
                    continue;
                }
                let da = out.pixeles[d + 3] as f32 / 255.0;
                let oa = a + da * (1.0 - a);
                for c in 0..3 {
                    let v = (src[c] as f32 * a + out.pixeles[d + c] as f32 * da * (1.0 - a))
                        / oa.max(1e-6);
                    out.pixeles[d + c] = v.round().clamp(0.0, 255.0) as u8;
                }
                out.pixeles[d + 3] = (oa * 255.0).round() as u8;
            }
        }
    }
    out
}

/// Donde quedo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guardado {
    /// El id de «Mensajes guardados» en el indice.
    pub proyecto: String,
    /// El id del mensaje IMAGEN.
    pub mensaje: String,
    /// El id del lienzo de la foto (`foto-<id del mensaje>`).
    pub lienzo: String,
    /// Cuantos trazos quedaron encima.
    pub trazos: usize,
    /// El fichero de la foto, por si hay que rehacerla (cambiaron las
    /// imagenes pegadas).
    pub foto: PathBuf,
}

/// **Guarda la sesion** en «Mensajes guardados» (creandolo si no esta).
pub fn guardar(raiz: &Path, aparato: &str, sesion: &Sesion, ahora: i64) -> io::Result<Guardado> {
    let ficha = almacen::asegurar_guardados(raiz, ahora, aparato)?;
    let compuesta = componer(&sesion.foto, &sesion.pegadas);
    let png = pixpin_codec::codificar_png(&compuesta).map_err(io::Error::other)?;
    let mensajes = crate::ventana_chat::meter_en_proyecto(
        raiz,
        &ficha.id,
        &[(NOMBRE_DE_LA_CAPTURA.to_string(), png)],
        aparato,
    )?;
    let m = mensajes
        .into_iter()
        .next()
        .ok_or_else(|| io::Error::other("la captura no entro en el chat"))?;
    let ruta = m
        .ruta
        .as_deref()
        .ok_or_else(|| io::Error::other("el mensaje de la captura no tiene fichero"))?;
    let foto = almacen::carpeta(raiz, &ficha.id).join(ruta);
    let hecho = pixpin_proyecto::lienzo_de_la_foto::con_dibujo(
        raiz,
        &ficha.id,
        &m,
        &foto,
        (sesion.foto.ancho, sesion.foto.alto),
        &sesion.tinta,
        ahora,
    )?;
    Ok(Guardado {
        proyecto: ficha.id,
        mensaje: m.id,
        lienzo: hecho.id,
        trazos: hecho.adoptados,
        foto,
    })
}

/// Los textos del globo, ya traducidos (el hilo no tiene el catalogo).
#[derive(Clone)]
pub struct Globo {
    pub titulo: String,
    pub hecho: String,
    pub fallo: String,
}

/// Lo que toca hacer con lo anotado, al pulsar Guardar o al salir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paso {
    /// Nada que guardar: sin tinta y sin guardar antes, o sin cambios desde
    /// lo ultimo guardado.
    Nada,
    /// La primera vez con tinta en esta sesion: captura de fondo, mensaje
    /// IMAGEN y su lienzo (`guardar`).
    Nuevo,
    /// Ya se guardo en esta sesion y se siguio dibujando (o se limpio): se
    /// pone al dia EL MISMO lienzo, sin otro mensaje (`poner_al_dia`).
    AlDia,
}

/// **Pura**: la regla de cuando se guarda (pedida el 2026-09-29: «tiene que
/// guardarse automaticamente en el canvas dentro de Mensajes guardados»).
/// `guardada` es la tinta tal como se mando la ultima vez en esta sesion.
pub fn que_hacer(guardada: Option<&[Elemento]>, ahora: &[Elemento]) -> Paso {
    match guardada {
        None if ahora.is_empty() => Paso::Nada,
        None => Paso::Nuevo,
        Some(antes) if antes == ahora => Paso::Nada,
        Some(_) => Paso::AlDia,
    }
}

/// **Pone al dia** el lienzo de una pantalla ya guardada con la tinta de
/// ahora (en pixeles de la captura, que mide `medidas`). Ni captura nueva ni
/// mensaje nuevo: la foto de fondo es la de la primera vez, que es sobre la
/// que se dibujo.
pub fn poner_al_dia(
    raiz: &Path,
    g: &Guardado,
    medidas: (u32, u32),
    tinta: &[Elemento],
) -> io::Result<Guardado> {
    let trazos = pixpin_proyecto::lienzo_de_la_foto::cambiar_dibujo(
        raiz,
        &g.proyecto,
        &g.lienzo,
        medidas,
        tinta,
    )?;
    Ok(Guardado {
        trazos,
        ..g.clone()
    })
}

/// **El guardado de UNA sesion del anotador** en «Mensajes guardados».
///
/// La primera vez con tinta crea la foto y su lienzo; las siguientes ponen
/// al dia ese mismo lienzo. Todo en hilos, porque comprimir la captura del
/// escritorio entero a PNG tarda (cientos de milisegundos a 3840x1080 en el
/// portatil) y el anotador tiene que seguir al ritmo del raton; y al salir,
/// la ventana se cierra sin esperar. Cada hilo espera al anterior: el que
/// pone al dia necesita saber donde quedo el primero.
#[derive(Default)]
pub struct EnElChat {
    /// La tinta tal como se mando la ultima vez.
    tinta: Option<Vec<Elemento>>,
    /// Las imagenes pegadas tal como se mandaron (sus elementos).
    pegadas: Vec<Elemento>,
    /// La captura de fondo sin nada encima: si cambian las imagenes
    /// pegadas, la foto se rehace desde ella.
    limpia: Option<std::sync::Arc<pixpin_codec::ImagenRgba>>,
    /// Lo que mide la captura de fondo (la tinta va en sus pixeles).
    medidas: (u32, u32),
    /// Donde empieza, dentro del escritorio, el trozo que se guardo (ver
    /// [`monitores_con_dibujo`]): la tinta se corre esto antes de guardarla.
    corrimiento: (f32, f32),
    /// El ultimo guardado lanzado; devuelve donde quedo.
    hilo: Option<std::thread::JoinHandle<Option<Guardado>>>,
}

impl EnElChat {
    pub fn paso(&self, tinta: &[Elemento], pegadas: &[Elemento]) -> Paso {
        match (&self.tinta, que_hacer(self.tinta.as_deref(), tinta)) {
            // Solo imagenes pegadas, sin tinta: tambien se guarda.
            (None, Paso::Nada) if !pegadas.is_empty() => Paso::Nuevo,
            (Some(_), Paso::Nada) if self.pegadas != pegadas => Paso::AlDia,
            (_, p) => p,
        }
    }

    /// La primera vez: la captura de debajo con la tinta, a un mensaje nuevo.
    /// `recorte`: el trozo de la foto que se guarda (los monitores con algo
    /// dibujado), en sus pixeles; `None`, entera.
    pub fn guardar_nuevo(
        &mut self,
        raiz: PathBuf,
        sesion: Sesion,
        recorte: Option<pixpin_geom::Rect>,
        hwnd: isize,
        globo: Globo,
    ) {
        // Lo que se compara la proxima vez va SIN correr: es lo que se ve.
        self.tinta = Some(sesion.tinta.clone());
        self.pegadas = sesion.pegadas.iter().map(|p| p.elemento.clone()).collect();
        let mut sesion = sesion;
        if let Some(r) = recorte {
            sesion.foto = recortar(&sesion.foto, r);
            self.corrimiento = (r.x as f32, r.y as f32);
        }
        let (dx, dy) = self.corrimiento;
        correr(&mut sesion.tinta, &mut sesion.pegadas, dx, dy);
        self.medidas = (sesion.foto.ancho, sesion.foto.alto);
        self.limpia = Some(std::sync::Arc::new(sesion.foto.clone()));
        let previo = self.hilo.take();
        self.hilo = lanzar(move || {
            // Uno anterior que fallo no cuenta: este es el primero bueno.
            if let Some(h) = previo {
                let _ = h.join();
            }
            let t0 = std::time::Instant::now();
            let aparato = pixpin_proyecto::identidad::Identidad::leer_o_crear(&raiz, "PC")
                .map(|i| i.yo.codigo())
                .unwrap_or_default();
            let hecho = guardar(
                &raiz,
                &aparato,
                &sesion,
                pixpin_shell::entorno::ahora_utc_ms(),
            );
            avisar(
                hecho,
                t0,
                (sesion.foto.ancho, sesion.foto.alto),
                hwnd,
                &globo,
            )
        });
    }

    /// Las siguientes: el mismo lienzo con la tinta de ahora.
    pub fn poner_al_dia(
        &mut self,
        raiz: PathBuf,
        tinta: Vec<Elemento>,
        pegadas: Vec<Pegada>,
        hwnd: isize,
        globo: Globo,
    ) {
        self.tinta = Some(tinta.clone());
        let firma: Vec<Elemento> = pegadas.iter().map(|p| p.elemento.clone()).collect();
        // Si cambiaron las imagenes pegadas, la foto se rehace desde la limpia.
        let rehacer = (firma != self.pegadas)
            .then(|| self.limpia.clone())
            .flatten();
        self.pegadas = firma;
        let (mut tinta, mut pegadas) = (tinta, pegadas);
        correr(
            &mut tinta,
            &mut pegadas,
            self.corrimiento.0,
            self.corrimiento.1,
        );
        let medidas = self.medidas;
        let previo = self.hilo.take();
        self.hilo = lanzar(move || {
            let t0 = std::time::Instant::now();
            let Some(g) = previo.and_then(|h| h.join().ok().flatten()) else {
                tracing::warn!(
                    "la pantalla anotada no llego a guardarse: no hay lienzo que poner al dia"
                );
                return avisar(
                    Err(io::Error::other("sin guardado previo")),
                    t0,
                    medidas,
                    hwnd,
                    &globo,
                );
            };
            if let Some(limpia) = &rehacer {
                let hecho = pixpin_codec::codificar_png(&componer(limpia, &pegadas))
                    .map_err(io::Error::other)
                    .and_then(|png| std::fs::write(&g.foto, png));
                if let Err(e) = hecho {
                    tracing::warn!(?e, "no se pudo rehacer la foto con las imagenes pegadas");
                }
            }
            avisar(
                poner_al_dia(&raiz, &g, medidas, &tinta),
                t0,
                medidas,
                hwnd,
                &globo,
            )
        });
    }

    /// Espera a que acabe lo lanzado y dice donde quedo (para las pruebas y
    /// para quien quiera saberlo; el anotador no espera).
    #[cfg(test)]
    pub fn esperar(&mut self) -> Option<Guardado> {
        let g = self.hilo.take()?.join().ok().flatten();
        // Lo sabido sigue valiendo para la siguiente puesta al dia.
        if let Some(g) = &g {
            let copia = g.clone();
            self.hilo = lanzar(move || Some(copia));
        }
        g
    }
}

fn lanzar(
    f: impl FnOnce() -> Option<Guardado> + Send + 'static,
) -> Option<std::thread::JoinHandle<Option<Guardado>>> {
    std::thread::Builder::new()
        .name("pixpin-anotador-al-chat".into())
        .spawn(f)
        .inspect_err(|e| tracing::warn!(?e, "no se pudo lanzar el guardado de la pantalla anotada"))
        .ok()
}

/// Deja constancia, despierta al chat y saca el globo (`hwnd` es la ventana
/// de mensajes de `main`, duena del icono de la bandeja; 0 = sin globo).
fn avisar(
    hecho: io::Result<Guardado>,
    t0: std::time::Instant,
    (ancho, alto): (u32, u32),
    hwnd: isize,
    globo: &Globo,
) -> Option<Guardado> {
    let (texto, g) = match hecho {
        Ok(g) => {
            tracing::info!(
                proyecto = %g.proyecto,
                mensaje = %g.mensaje,
                lienzo = %g.lienzo,
                trazos = g.trazos,
                ancho,
                alto,
                ms = t0.elapsed().as_millis() as u64,
                "pantalla anotada guardada en Mensajes guardados"
            );
            crate::ventana_chat::refrescar();
            (&globo.hecho, Some(g))
        }
        Err(e) => {
            tracing::warn!(?e, "no se pudo guardar la pantalla anotada");
            (&globo.fallo, None)
        }
    };
    if hwnd != 0 {
        let hwnd = windows::Win32::Foundation::HWND(hwnd as *mut _);
        let _ = pixpin_shell::aviso::Aviso::sobre_la_bandeja(hwnd).mostrar(&globo.titulo, texto);
    }
    g
}

/// **Pura**: la tinta que se guarda. Lo que se ve (lo borrado o deshecho no)
/// y sin imagenes pegadas: el lienzo de la foto solo lleva la foto como
/// fichero, y una imagen suelta sin fichero que viaje se perderia al abrirlo.
pub fn tinta_de(escena: &pixpin_motor2d::Escena) -> Vec<Elemento> {
    escena
        .visibles()
        .filter(|e| !matches!(e.figura, pixpin_motor2d::Figura::Imagen { .. }))
        .cloned()
        .collect()
}

/// Las imagenes pegadas que se ven, con sus pixeles (`imagen` los da por el
/// `id_objeto`; la que no los tenga no se puede pegar en la foto y se salta).
pub fn pegadas_de(
    escena: &pixpin_motor2d::Escena,
    imagen: impl Fn(u64) -> Option<pixpin_codec::ImagenRgba>,
) -> Vec<Pegada> {
    escena
        .visibles()
        .filter_map(|e| match e.figura {
            pixpin_motor2d::Figura::Imagen { id_objeto } => Some(Pegada {
                elemento: e.clone(),
                imagen: imagen(id_objeto)?,
            }),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::{Figura, Punto2};

    fn raiz(etiqueta: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!(
            "pixpin-anotador-chat-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    /// Una «pantalla» inventada: dos monitores de colores, lado a lado.
    fn pantalla(ancho: u32, alto: u32) -> pixpin_codec::ImagenRgba {
        let mut pixeles = Vec::with_capacity((ancho * alto * 4) as usize);
        for _ in 0..alto {
            for x in 0..ancho {
                let c = if x < ancho / 2 {
                    [40, 90, 200]
                } else {
                    [235, 235, 240]
                };
                pixeles.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        pixpin_codec::ImagenRgba {
            ancho,
            alto,
            pixeles,
        }
    }

    fn raya(puntos: &[(f32, f32)]) -> Elemento {
        Elemento {
            figura: Figura::Lapiz {
                puntos: puntos.iter().map(|&(x, y)| Punto2::nuevo(x, y)).collect(),
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            },
            trazo: pixpin_motor2d::ColorRgba::opaco(0.88, 0.19, 0.19),
            grosor: 2.0,
            ..Elemento::default()
        }
    }

    fn flecha(de: (f32, f32), a: (f32, f32)) -> Elemento {
        Elemento {
            figura: Figura::Flecha {
                // Los puntos van en coordenadas del mundo, como los deja el gesto.
                puntos: vec![Punto2::nuevo(de.0, de.1), Punto2::nuevo(a.0, a.1)],
                punta_inicio: pixpin_motor2d::formas::TipoPunta::Ninguna,
                punta_fin: pixpin_motor2d::formas::TipoPunta::Flecha,
                codos: false,
            },
            x: de.0,
            y: de.1,
            ancho: (a.0 - de.0).abs(),
            alto: (a.1 - de.1).abs(),
            trazo: pixpin_motor2d::ColorRgba::opaco(0.88, 0.19, 0.19),
            grosor: 4.0,
            ..Elemento::default()
        }
    }

    #[test]
    fn guardar_la_pantalla_anotada_deja_una_foto_en_mensajes_guardados_con_su_lienzo_editable() {
        let r = raiz("guardar");
        let sesion = Sesion {
            pegadas: Vec::new(),
            foto: pantalla(64, 18),
            tinta: vec![raya(&[(10.0, 4.0), (50.0, 14.0)])],
        };
        let g = guardar(&r, "PC01", &sesion, 1_790_000_000_000).unwrap();
        assert_eq!(g.lienzo, format!("foto-{}", g.mensaje));
        assert_eq!(g.trazos, 1);
        // En el indice, «Mensajes guardados».
        let indice = almacen::Indice::leer(&r);
        let ficha = indice
            .proyectos
            .iter()
            .find(|f| f.id == g.proyecto)
            .unwrap();
        assert!(ficha.es_guardados());
        // Un mensaje IMAGEN como una captura cualquiera, con su referencia.
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&r, &g.proyecto)).unwrap();
        assert_eq!(c.mensajes.len(), 1);
        let m = &c.mensajes[0];
        assert_eq!(m.clase, Some(cuaderno::Clase::Imagen));
        assert_eq!(m.referencia.as_deref(), Some(g.lienzo.as_str()));
        // La foto es la captura tal cual (un PNG que se lee con sus medidas).
        let foto = almacen::carpeta(&r, &g.proyecto).join(m.ruta.as_deref().unwrap());
        assert_eq!(pixpin_codec::imagen::medidas(&foto).unwrap(), (64, 18));
        // Y la burbuja la ensena con la tinta encima, en pixeles de la foto.
        let d = crate::foto_anotada::dibujo_de_la_foto(&r, &g.proyecto, m, (64.0, 18.0)).unwrap();
        // (La caja lleva el margen del trazo: se mira su centro, el de la raya.)
        let (x0, y0, x1, y1) = d.trazos.unwrap();
        let centro = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        assert!(
            (centro.0 - 30.0).abs() < 1.0 && (centro.1 - 9.0).abs() < 1.0,
            "{:?}",
            d.trazos
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn dos_sesiones_seguidas_son_dos_fotos_y_no_pisan_la_primera() {
        let r = raiz("dos");
        let s = Sesion {
            pegadas: Vec::new(),
            foto: pantalla(8, 4),
            tinta: vec![raya(&[(1.0, 1.0), (6.0, 3.0)])],
        };
        let a = guardar(&r, "PC01", &s, 1).unwrap();
        let b = guardar(&r, "PC01", &s, 2).unwrap();
        assert_eq!(a.proyecto, b.proyecto, "las dos en Mensajes guardados");
        assert_ne!(a.mensaje, b.mensaje);
        assert_ne!(a.lienzo, b.lienzo);
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&r, &a.proyecto)).unwrap();
        let rutas: Vec<_> = c.mensajes.iter().map(|m| m.ruta.clone().unwrap()).collect();
        assert_eq!(rutas.len(), 2);
        assert_ne!(
            rutas[0], rutas[1],
            "la segunda captura no pisa el fichero de la primera"
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_tinta_que_se_guarda_es_la_que_se_ve_sin_lo_borrado_ni_imagenes_sueltas() {
        let mut e = pixpin_motor2d::Escena::nueva();
        e.anadir(raya(&[(0.0, 0.0), (5.0, 5.0)]));
        let borrada = e.anadir(raya(&[(1.0, 1.0), (2.0, 2.0)]));
        e.borrar(borrada);
        e.anadir(Elemento {
            figura: Figura::Imagen { id_objeto: 7 },
            ancho: 10.0,
            alto: 10.0,
            ..Elemento::default()
        });
        let t = tinta_de(&e);
        assert_eq!(t.len(), 1);
        assert!(matches!(t[0].figura, Figura::Lapiz { .. }));
        // Caso negativo: una escena vacia no da nada.
        assert!(tinta_de(&pixpin_motor2d::Escena::nueva()).is_empty());
    }

    fn globo_mudo() -> Globo {
        Globo {
            titulo: String::new(),
            hecho: String::new(),
            fallo: String::new(),
        }
    }

    /// El mensaje de la foto en «Mensajes guardados» y su lienzo, leidos.
    fn lo_guardado(r: &Path, g: &Guardado) -> (Vec<cuaderno::Mensaje>, Vec<Elemento>) {
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(r, &g.proyecto)).unwrap();
        let texto = std::fs::read_to_string(almacen::lienzo(r, &g.proyecto, &g.lienzo)).unwrap();
        let l = pixpin_motor2d::excalidraw::leer(&texto).unwrap();
        (c.mensajes, l.elementos())
    }

    #[test]
    fn la_regla_de_guardar_primero_crea_luego_pone_al_dia_y_sin_cambios_no_hace_nada() {
        let una = vec![raya(&[(1.0, 1.0), (5.0, 5.0)])];
        let dos = vec![
            raya(&[(1.0, 1.0), (5.0, 5.0)]),
            raya(&[(9.0, 9.0), (2.0, 2.0)]),
        ];
        assert_eq!(que_hacer(None, &una), Paso::Nuevo);
        assert_eq!(que_hacer(Some(&una), &dos), Paso::AlDia);
        // Limpiar lo ya guardado tambien se lleva al lienzo.
        assert_eq!(que_hacer(Some(&una), &[]), Paso::AlDia);
        // Casos negativos: sin tinta y sin guardar antes, o sin cambios.
        assert_eq!(que_hacer(None, &[]), Paso::Nada);
        assert_eq!(que_hacer(Some(&una), &una), Paso::Nada);
    }

    #[test]
    fn salir_con_tinta_guarda_la_foto_en_mensajes_guardados_con_la_tinta_editable_encima_de_la_foto_bloqueada()
     {
        let r = raiz("salir");
        let tinta = vec![
            raya(&[(10.0, 4.0), (50.0, 14.0)]),
            flecha((5.0, 15.0), (60.0, 2.0)),
        ];
        let mut chat = EnElChat::default();
        assert_eq!(chat.paso(&tinta, &[]), Paso::Nuevo);
        chat.guardar_nuevo(
            r.clone(),
            Sesion {
                pegadas: Vec::new(),
                foto: pantalla(64, 18),
                tinta: tinta.clone(),
            },
            None,
            0,
            globo_mudo(),
        );
        let g = chat.esperar().expect("guardado");
        let (mensajes, els) = lo_guardado(&r, &g);
        assert_eq!(mensajes.len(), 1, "un mensaje IMAGEN");
        assert_eq!(mensajes[0].clase, Some(cuaderno::Clase::Imagen));
        assert_eq!(mensajes[0].referencia.as_deref(), Some(g.lienzo.as_str()));
        assert_eq!(els.len(), 3, "la foto y los dos trazos");
        assert!(
            matches!(els[0].figura, Figura::Imagen { .. }) && els[0].bloqueado,
            "la foto primero y bloqueada"
        );
        assert!(els[1..].iter().all(|e| !e.bloqueado), "la tinta, editable");
        // Salir otra vez sin haber tocado nada no guarda otra.
        assert_eq!(chat.paso(&tinta, &[]), Paso::Nada);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn salir_sin_tinta_no_guarda_nada_en_el_chat() {
        let r = raiz("salir-vacio");
        let chat = EnElChat::default();
        let e = pixpin_motor2d::Escena::nueva();
        assert_eq!(chat.paso(&tinta_de(&e), &[]), Paso::Nada);
        // Ni se crea «Mensajes guardados» por haber entrado y salido.
        assert!(almacen::Indice::leer(&r).proyectos.is_empty());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn guardar_seguir_dibujando_y_salir_pone_al_dia_el_mismo_lienzo_sin_otro_mensaje() {
        let r = raiz("al-dia");
        let mut tinta = vec![raya(&[(10.0, 4.0), (50.0, 14.0)])];
        let mut chat = EnElChat::default();
        // El boton de guardar, a mitad de la sesion.
        chat.guardar_nuevo(
            r.clone(),
            Sesion {
                pegadas: Vec::new(),
                foto: pantalla(64, 18),
                tinta: tinta.clone(),
            },
            None,
            0,
            globo_mudo(),
        );
        // Se sigue dibujando SIN esperar al primer guardado: el segundo hilo
        // espera al primero.
        tinta.push(raya(&[(2.0, 2.0), (8.0, 16.0)]));
        tinta.push(flecha((30.0, 2.0), (62.0, 16.0)));
        assert_eq!(chat.paso(&tinta, &[]), Paso::AlDia);
        chat.poner_al_dia(r.clone(), tinta.clone(), Vec::new(), 0, globo_mudo());
        let g = chat.esperar().expect("puesto al dia");
        assert_eq!(g.trazos, 3);
        let (mensajes, els) = lo_guardado(&r, &g);
        assert_eq!(mensajes.len(), 1, "el mismo mensaje, no otro");
        assert_eq!(
            els.len(),
            4,
            "la foto y los tres trazos, sin duplicar el primero"
        );
        assert!(els[0].bloqueado);
        // Y otra puesta al dia tras esperar sigue yendo al mismo sitio.
        tinta.pop();
        chat.poner_al_dia(r.clone(), tinta.clone(), Vec::new(), 0, globo_mudo());
        let g2 = chat.esperar().unwrap();
        assert_eq!((g2.mensaje.as_str(), g2.trazos), (g.mensaje.as_str(), 2));
        assert_eq!(lo_guardado(&r, &g2).1.len(), 3);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn poner_al_dia_sin_un_guardado_previo_que_llegara_no_inventa_un_mensaje() {
        let r = raiz("al-dia-sin-previo");
        let mut chat = EnElChat::default();
        chat.poner_al_dia(
            r.clone(),
            vec![raya(&[(1.0, 1.0), (2.0, 2.0)])],
            Vec::new(),
            0,
            globo_mudo(),
        );
        assert_eq!(chat.esperar(), None);
        assert!(almacen::Indice::leer(&r).proyectos.is_empty());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn abrir_la_burbuja_da_el_lienzo_con_la_foto_bloqueada_debajo_y_la_tinta_editable_que_se_guarda_al_cerrar()
     {
        let r = raiz("abrir");
        let s = Sesion {
            pegadas: Vec::new(),
            foto: pantalla(64, 18),
            tinta: vec![
                raya(&[(10.0, 4.0), (50.0, 14.0)]),
                flecha((5.0, 15.0), (60.0, 2.0)),
            ],
        };
        let g = guardar(&r, "PC01", &s, 7).unwrap();
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&r, &g.proyecto)).unwrap();
        let m = &c.mensajes[0];
        // Lo que recibe el editor al tocar la burbuja (o la tarjeta de
        // Proyectos): la escena del lienzo, no la foto plana.
        let azul = pixpin_motor2d::ColorRgba::opaco(0.1, 0.3, 0.9);
        let (abierta, fotos, guardada) =
            crate::ventana_chat::hoja_abierta_y_guardada(&r, &g.proyecto, m, |e| {
                let ids: Vec<u64> = e.visibles().map(|x| x.id).collect();
                // La goma se lleva la raya; la flecha se mueve y cambia de color.
                assert!(e.borrar_apuntando(ids[1]));
                assert!(e.mover(ids[2], 20.0, 0.0));
                e.buscar_mut(ids[2]).unwrap().trazo = azul;
            })
            .expect("se abre su lienzo");
        let els: Vec<&Elemento> = abierta.visibles().collect();
        assert_eq!(els.len(), 3);
        assert!(
            matches!(els[0].figura, Figura::Imagen { .. }),
            "la foto primero"
        );
        assert!(els[0].bloqueado, "la foto bloqueada");
        assert!(els[1..].iter().all(|e| !e.bloqueado), "la tinta editable");
        // La foto tiene su fichero, que el editor carga.
        assert_eq!(fotos.len(), 1);
        assert!(fotos[0].1.is_file(), "{}", fotos[0].1.display());
        // Y al cerrar se guarda sin aplanar nada: la foto sigue debajo.
        assert!(guardada);
        let (mensajes, despues) = lo_guardado(&r, &g);
        assert_eq!(mensajes.len(), 1);
        assert_eq!(despues.len(), 2, "la foto y la flecha");
        assert!(despues[0].bloqueado && matches!(despues[0].figura, Figura::Imagen { .. }));
        assert!(matches!(despues[1].figura, Figura::Flecha { .. }));
        assert!(
            (despues[1].trazo.b - azul.b).abs() < 0.01
                && (despues[1].trazo.r - azul.r).abs() < 0.01,
            "el color nuevo"
        );
        assert!((despues[1].x - (els[2].x + 20.0)).abs() < 1e-3);
        // Caso negativo: abrir y cerrar sin tocar no reescribe el lienzo.
        let (_, _, otra) =
            crate::ventana_chat::hoja_abierta_y_guardada(&r, &g.proyecto, m, |_| {}).unwrap();
        assert!(!otra);
        let _ = std::fs::remove_dir_all(&r);
    }

    /// **El lienzo que abre la burbuja**, pintado como lo pinta el editor:
    /// la escena que recibe (`hoja_abierta_y_guardada`), con la foto de su
    /// fichero y la tinta encima. Arriba recien guardado al salir del
    /// anotador; abajo tras editarlo (la goma se llevo el circulo, la flecha
    /// se movio y paso a azul) y volverlo a abrir. Cada elemento lleva su
    /// caja: roja la foto bloqueada, verde lo editable.
    /// `cargo test -p pixpin --bin pixpinmax muestra_del_lienzo_abierto -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_del_lienzo_abierto_de_la_pantalla_anotada() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let r = raiz("muestra-abierto");
        let (w, h) = (1600u32, 450u32);
        let s = Sesion {
            pegadas: Vec::new(),
            foto: pantalla(w, h),
            tinta: vec![
                flecha((500.0, 300.0), (1100.0, 120.0)),
                raya(
                    &(0..=64)
                        .map(|i| {
                            let a = i as f32 / 60.0 * std::f32::consts::TAU;
                            (1250.0 + 110.0 * a.cos(), 110.0 + 70.0 * a.sin())
                        })
                        .collect::<Vec<_>>(),
                ),
            ],
        };
        let mut chat = EnElChat::default();
        chat.guardar_nuevo(r.clone(), s, None, 0, globo_mudo());
        let g = chat.esperar().unwrap();
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&r, &g.proyecto)).unwrap();
        let m = c.mensajes[0].clone();
        let (antes, fotos, _) =
            crate::ventana_chat::hoja_abierta_y_guardada(&r, &g.proyecto, &m, |e| {
                let ids: Vec<u64> = e.visibles().map(|x| x.id).collect();
                e.borrar_apuntando(ids[2]);
                e.mover(ids[1], 0.0, 60.0);
                e.buscar_mut(ids[1]).unwrap().trazo =
                    pixpin_motor2d::ColorRgba::opaco(0.1, 0.35, 0.95);
            })
            .unwrap();
        let (despues, _, _) =
            crate::ventana_chat::hoja_abierta_y_guardada(&r, &g.proyecto, &m, |_| {}).unwrap();
        // La foto del lienzo mide lo del movil (1600 ya cabe): una unidad por
        // pixel. Dos paneles, uno encima del otro, a la mitad.
        let escala = 0.5;
        let (pw, ph) = ((w as f32 * escala) as u32, (h as f32 * escala) as u32);
        let (bw, bh) = (pw + 24, 2 * ph + 36);
        let disp = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(disp.d3d()).expect("motor");
        let fuera = FueraDePantalla::nuevo(&motor, disp.d3d(), bw, bh).expect("superficie");
        let mut imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(4096);
        for (id, ruta) in &fotos {
            assert!(imagenes.guardar_con_id(*id, pixpin_codec::imagen::cargar(ruta).unwrap()));
        }
        imagenes.asegurar(&motor);
        motor
            .dibujar(&fuera.destino, |p| {
                p.limpiar(pixpin_render::Color {
                    r: 0.93,
                    g: 0.94,
                    b: 0.96,
                    a: 1.0,
                });
                for (i, escena) in [&antes, &despues].into_iter().enumerate() {
                    let y0 = 12.0 + i as f32 * (ph as f32 + 12.0);
                    p.poner_vista((12.0, y0), escala, (0.0, 0.0));
                    let vista = (0.0, 0.0, w as f32, h as f32);
                    for o in pixpin_motor2d::ordenes_de_escena(escena) {
                        crate::dibujo::pintar::dibujar_orden(
                            p, &o, vista, None, &imagenes, escala, None,
                        );
                    }
                    p.desplazar(0.0, 0.0);
                    for e in escena.visibles() {
                        let Some((x0, y0e, x1, y1)) =
                            pixpin_motor2d::biblioteca::caja_de(std::slice::from_ref(e))
                        else {
                            continue;
                        };
                        let color = if e.bloqueado {
                            pixpin_render::Color {
                                r: 0.9,
                                g: 0.1,
                                b: 0.1,
                                a: 1.0,
                            }
                        } else {
                            pixpin_render::Color {
                                r: 0.1,
                                g: 0.7,
                                b: 0.2,
                                a: 1.0,
                            }
                        };
                        p.trazar(
                            pixpin_render::RectF {
                                x: 12.0 + x0 * escala,
                                y: y0 + y0e * escala,
                                ancho: (x1 - x0) * escala,
                                alto: (y1 - y0e) * escala,
                            },
                            2.0,
                            color,
                        );
                    }
                }
            })
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        let (_, _, pixeles) = fuera.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba {
            ancho: bw,
            alto: bh,
            pixeles,
        })
        .unwrap();
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let ruta = carpeta.join("pantalla-anotada-lienzo-abierto.png");
        std::fs::write(&ruta, png).unwrap();
        println!("pantalla-anotada-lienzo-abierto: {}", ruta.display());
        let _ = std::fs::remove_dir_all(&r);
    }

    /// **La burbuja del chat con la pantalla y la tinta guardadas**: se
    /// guarda de verdad (en una carpeta temporal) una «pantalla» de dos
    /// monitores con una flecha que cruza de uno a otro y un trazo, y se
    /// pinta como la pinta la burbuja: la foto y encima lo que devuelve
    /// `dibujo_de_la_foto` leyendo el lienzo guardado.
    /// `cargo test -p pixpin --bin pixpinmax muestra_de_la_pantalla_anotada -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_de_la_pantalla_anotada_en_su_burbuja() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let r = raiz("muestra");
        let (w, h) = (1600u32, 450u32);
        let sesion = Sesion {
            pegadas: Vec::new(),
            foto: pantalla(w, h),
            tinta: vec![
                flecha((500.0, 300.0), (1100.0, 120.0)),
                // Un circulo a mano alzada alrededor de algo, con los puntos
                // seguidos que da el raton.
                raya(
                    &(0..=64)
                        .map(|i| {
                            let a = i as f32 / 60.0 * std::f32::consts::TAU;
                            (1250.0 + 110.0 * a.cos(), 110.0 + 70.0 * a.sin())
                        })
                        .collect::<Vec<_>>(),
                ),
            ],
        };
        let t0 = std::time::Instant::now();
        let g = guardar(&r, "PC01", &sesion, 5).unwrap();
        println!("guardar {w}x{h}: {} ms", t0.elapsed().as_millis());
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&r, &g.proyecto)).unwrap();
        let d = crate::foto_anotada::dibujo_de_la_foto(
            &r,
            &g.proyecto,
            &c.mensajes[0],
            (w as f32, h as f32),
        )
        .unwrap();
        // Como la burbuja: a la mitad.
        let escala = 0.5;
        let (bw, bh) = (
            (w as f32 * escala) as u32 + 24,
            (h as f32 * escala) as u32 + 24,
        );
        let disp = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(disp.d3d()).expect("motor");
        let fuera = FueraDePantalla::nuevo(&motor, disp.d3d(), bw, bh).expect("superficie");
        let mut imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(4096);
        assert!(imagenes.guardar_con_id(1, sesion.foto.clone()));
        imagenes.asegurar(&motor);
        motor
            .dibujar(&fuera.destino, |p| {
                p.limpiar(pixpin_render::Color {
                    r: 0.93,
                    g: 0.94,
                    b: 0.96,
                    a: 1.0,
                });
                p.poner_vista((12.0, 12.0), escala, (0.0, 0.0));
                let vista = (0.0, 0.0, w as f32, h as f32);
                let foto = pixpin_motor2d::Orden::Imagen {
                    id_objeto: 1,
                    x: 0.0,
                    y: 0.0,
                    ancho: w as f32,
                    alto: h as f32,
                    opacidad: 1.0,
                    recorte: None,
                    angulo: 0.0,
                };
                crate::dibujo::pintar::dibujar_orden(
                    p, &foto, vista, None, &imagenes, escala, None,
                );
                for o in &d.ordenes {
                    crate::dibujo::pintar::dibujar_orden(
                        p, o, vista, None, &imagenes, escala, None,
                    );
                }
            })
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        let (_, _, pixeles) = fuera.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba {
            ancho: bw,
            alto: bh,
            pixeles,
        })
        .unwrap();
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let ruta = carpeta.join("pantalla-anotada-en-burbuja.png");
        std::fs::write(&ruta, png).unwrap();
        println!("pantalla-anotada-en-burbuja: {}", ruta.display());
        let _ = std::fs::remove_dir_all(&r);
    }

    fn lisa(ancho: u32, alto: u32, rgba: [u8; 4]) -> pixpin_codec::ImagenRgba {
        pixpin_codec::ImagenRgba {
            ancho,
            alto,
            pixeles: rgba.repeat((ancho * alto) as usize),
        }
    }

    fn pixel(img: &pixpin_codec::ImagenRgba, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * img.ancho + x) * 4) as usize;
        [
            img.pixeles[i],
            img.pixeles[i + 1],
            img.pixeles[i + 2],
            img.pixeles[i + 3],
        ]
    }

    fn pegada(x: f32, y: f32, w: f32, h: f32, img: pixpin_codec::ImagenRgba) -> Pegada {
        Pegada {
            elemento: Elemento {
                figura: Figura::Imagen { id_objeto: 1 },
                x,
                y,
                ancho: w,
                alto: h,
                ..Elemento::default()
            },
            imagen: img,
        }
    }

    #[test]
    fn la_imagen_pegada_entra_en_la_foto_en_su_caja_y_a_su_tamano() {
        let foto = lisa(40, 30, [255, 255, 255, 255]);
        // Una roja de 2x2 estirada a 10x10 en (5, 5).
        let p = pegada(5.0, 5.0, 10.0, 10.0, lisa(2, 2, [255, 0, 0, 255]));
        let c = componer(&foto, &[p]);
        assert_eq!(pixel(&c, 10, 10), [255, 0, 0, 255]);
        assert_eq!(pixel(&c, 14, 14), [255, 0, 0, 255]);
        // Caso negativo: fuera de su caja, la foto como estaba.
        assert_eq!(pixel(&c, 3, 3), [255, 255, 255, 255]);
        assert_eq!(pixel(&c, 16, 16), [255, 255, 255, 255]);
        // Medio transparente: se mezcla.
        let mut q = pegada(0.0, 0.0, 4.0, 4.0, lisa(1, 1, [0, 0, 0, 255]));
        q.elemento.opacidad = 0.5;
        let c = componer(&foto, &[q]);
        let v = pixel(&c, 1, 1)[0];
        assert!(v > 100 && v < 160, "{v}");
    }

    #[test]
    fn girada_un_cuarto_una_imagen_alargada_ocupa_lo_alto() {
        let foto = lisa(40, 40, [0, 0, 0, 255]);
        let mut p = pegada(10.0, 18.0, 20.0, 4.0, lisa(1, 1, [0, 255, 0, 255]));
        p.elemento.angulo = std::f32::consts::FRAC_PI_2;
        let c = componer(&foto, &[p]);
        assert_eq!(pixel(&c, 20, 12), [0, 255, 0, 255], "arriba del centro");
        assert_eq!(
            pixel(&c, 12, 20),
            [0, 0, 0, 255],
            "caso negativo: ya no a lo ancho"
        );
    }

    #[test]
    fn solo_con_una_imagen_pegada_tambien_se_guarda_y_cambiarla_pone_al_dia() {
        let chat = EnElChat::default();
        let p = pegada(0.0, 0.0, 4.0, 4.0, lisa(1, 1, [0, 0, 0, 255]));
        assert_eq!(chat.paso(&[], &[p.elemento.clone()]), Paso::Nuevo);
        assert_eq!(
            chat.paso(&[], &[]),
            Paso::Nada,
            "caso negativo: nada de nada"
        );
        let chat = EnElChat {
            tinta: Some(Vec::new()),
            pegadas: vec![p.elemento.clone()],
            ..Default::default()
        };
        assert_eq!(chat.paso(&[], &[p.elemento.clone()]), Paso::Nada);
        let mut movida = p.elemento.clone();
        movida.x = 9.0;
        assert_eq!(chat.paso(&[], &[movida]), Paso::AlDia);
    }

    fn monitor(x: i32, ancho: u32) -> pixpin_geom::Rect {
        pixpin_geom::Rect {
            x,
            y: 0,
            ancho,
            alto: 1080,
        }
    }

    #[test]
    fn se_guarda_solo_el_monitor_donde_se_dibujo_a_su_tamano() {
        // Tres monitores en fila, como el del usuario.
        let m = [monitor(0, 1920), monitor(1920, 1920), monitor(3840, 1920)];
        let foto = (5760, 1080);
        // Un garabato en el del medio.
        let r = monitores_con_dibujo(&m, &[(2000.0, 100.0, 2500.0, 600.0)], foto).unwrap();
        assert_eq!((r.x, r.ancho, r.alto), (1920, 1920, 1080));
        // Una flecha que cruza dos: los dos.
        let r = monitores_con_dibujo(&m, &[(1800.0, 10.0, 2100.0, 20.0)], foto).unwrap();
        assert_eq!((r.x, r.ancho), (0, 3840));
        // Casos negativos: sin nada dibujado, o con algo en los tres, entera.
        assert_eq!(monitores_con_dibujo(&m, &[], foto), None);
        assert_eq!(
            monitores_con_dibujo(&m, &[(10.0, 10.0, 5000.0, 20.0)], foto),
            None
        );
    }

    #[test]
    fn el_trozo_guardado_lleva_la_tinta_corrida_a_su_sitio() {
        let foto = lisa(8, 4, [0, 0, 0, 255]);
        let mut foto2 = foto.clone();
        // Un pixel blanco en (5, 1): en el trozo desde x = 4, queda en (1, 1).
        let i = ((8 + 5) * 4) as usize;
        foto2.pixeles[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
        let r = pixpin_geom::Rect {
            x: 4,
            y: 0,
            ancho: 4,
            alto: 4,
        };
        let t = recortar(&foto2, r);
        assert_eq!((t.ancho, t.alto), (4, 4));
        assert_eq!(pixel(&t, 1, 1), [255, 255, 255, 255]);
        let mut tinta = vec![raya(&[(5.0, 1.0), (6.0, 2.0)])];
        correr(&mut tinta, &mut [], 4.0, 0.0);
        match &tinta[0].figura {
            Figura::Lapiz { puntos, .. } | Figura::Linea { puntos } => {
                assert!((puntos[0].x - 1.0).abs() < 1e-3, "{:?}", puntos[0])
            }
            otra => panic!("{otra:?}"),
        }
    }
}
