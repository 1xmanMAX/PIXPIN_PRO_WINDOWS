//! **La Salida** (6-oct-2026): una ventanita flotante con lo que se acaba
//! de exportar o compartir, lista para ARRASTRARLA a otro sitio.
//!
//! El usuario: «si comparto algun documento o archivo, que primero aparezca
//! en pantalla para que pueda facilmente jalarlo a otro lugar; y si exporto,
//! aparezca como una ventana para compartir y tambien aparezca el archivo y
//! pueda jalarlo».
//!
//! - Siempre encima y abajo a la derecha del monitor del raton, sin boton en
//!   la barra de tareas y **sin robar el foco** al aparecer: el usuario
//!   estaba haciendo algo y lo sigue haciendo. Al pulsarla si lo toma, que es
//!   lo que hace falta para que `Esc` la cierre.
//! - Una tarjeta por fichero (miniatura, nombre, tipo y peso). **Arrastrar
//!   una tarjeta** con el boton izquierdo la lleva a otra aplicacion por el
//!   mismo `DoDragDrop` que los pines (`pixpin_pin::arrastrar`); con mas de
//!   uno, una franja los lleva todos juntos.
//! - Abajo, sobre la tarjeta elegida: Copiar (el principal, en azul), Abrir,
//!   Mostrar en la carpeta y Pinear. Hubo un Compartir con el panel de
//!   Windows; el usuario (8-oct-2026) lo cambio por Copiar: lo que se hace
//!   aqui es arrastrar o copiar.
//! - **Una sola a la vez**: lo que llega mientras esta abierta se suma
//!   arriba. Soltar con exito NO la cierra: se puede querer soltar el mismo
//!   fichero en otro sitio.

#![forbid(unsafe_code)]

mod disposicion;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_geom::{Punto, Rect};
use pixpin_render::icono::Icono;
use pixpin_render::icono::material as mi;
use pixpin_render::{Color, MotorRender, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use crate::overlay::Recursos;
use crate::v2;
use crate::v2::color::blanco;
use crate::ventanita::Botones;
use disposicion::{Boton, Disposicion, Zona};

const VK_ESCAPE: u32 = 0x1B;
const VK_RETURN: u32 = 0x0D;
const VK_ARRIBA: u32 = 0x26;
const VK_ABAJO: u32 = 0x28;
const VK_C: u32 = 0x43;
/// Lo que dura un aviso en la cabecera («Copiado», «Soltado»).
const AVISO: Duration = Duration::from_millis(2_500);
/// El lado al que se pide la miniatura: la tarjeta la pinta a 48 logicos, y
/// a 200 % son 96.
const LADO_MINIATURA: u32 = 96;

// --- Una sola a la vez -------------------------------------------------------

/// Lo que espera a la ventana: los ficheros que llegaron y el titulo. `hwnd`
/// es 0 mientras nace (lo que llegue entonces lo recoge al empezar).
struct Pendiente {
    hwnd: isize,
    ficheros: Vec<PathBuf>,
    titulo: Option<String>,
}

static ABIERTA: Mutex<Option<Pendiente>> = Mutex::new(None);

fn con_abierta<T>(f: impl FnOnce(&mut Option<Pendiente>) -> T) -> T {
    let mut g = ABIERTA.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut g)
}

/// Lo que ha llegado desde la ultima vez, para la ventana.
fn tomar_pendiente() -> (Vec<PathBuf>, Option<String>) {
    con_abierta(|a| match a.as_mut() {
        Some(p) => (std::mem::take(&mut p.ficheros), p.titulo.take()),
        None => (Vec::new(), None),
    })
}

/// Solo lo que se puede arrastrar: ficheros que existen y con ruta entera
/// (`construir_hdrop` rechaza las relativas, y una tarjeta que no se deja
/// arrastrar seria un boton muerto).
fn validos(ficheros: Vec<PathBuf>) -> Vec<PathBuf> {
    ficheros
        .into_iter()
        .filter(|r| r.is_absolute() && r.is_file())
        .collect()
}

/// **Saca la Salida con `ficheros`**, o se los suma a la que ya esta.
/// `titulo` es lo que se acaba de hacer («Exportado», «Listo para
/// compartir»), ya traducido por quien llama.
pub(crate) fn mostrar(ficheros: Vec<PathBuf>, titulo: String) {
    let ficheros = validos(ficheros);
    if ficheros.is_empty() {
        return;
    }
    // En las pruebas no se abre nada en el escritorio del usuario: se apunta
    // lo pedido para que la prueba compruebe que se pidio.
    // (Con `PIXPIN_SALIDA_DE_VERDAD` puesta, una prueba `#[ignore]` puede
    // abrirla a proposito.)
    #[cfg(test)]
    if std::env::var_os("PIXPIN_SALIDA_DE_VERDAD").is_none() {
        pedidas_en_pruebas()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend(ficheros);
        return;
    }
    abrir_o_sumar(ficheros, titulo);
}

/// Lo que las pruebas pidieron mostrar.
#[cfg(test)]
pub(crate) fn pedidas_en_pruebas() -> &'static Mutex<Vec<PathBuf>> {
    static PEDIDAS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
    &PEDIDAS
}

fn abrir_o_sumar(ficheros: Vec<PathBuf>, titulo: String) {
    let nueva = con_abierta(|a| match a.as_mut() {
        Some(p) => {
            p.ficheros.extend(ficheros.iter().cloned());
            p.titulo = Some(titulo.clone());
            if p.hwnd != 0 {
                pixpin_shell::overlay::despertar(p.hwnd);
            }
            false
        }
        None => {
            *a = Some(Pendiente {
                hwnd: 0,
                ficheros: ficheros.clone(),
                titulo: Some(titulo.clone()),
            });
            true
        }
    });
    if !nueva {
        return;
    }
    let lanzado = std::thread::Builder::new()
        .name("salida".into())
        .spawn(|| {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma_actual());
            let mut estado = Estado::default();
            loop {
                if let Err(e) = crate::dispositivo_perdido::con_recursos("salida", |r| {
                    flotar(r, &textos, &mut estado)
                }) {
                    tracing::warn!(?e, "no se pudo abrir la Salida");
                }
                // Lo que llego mientras se cerraba no se pierde: se vuelve a
                // abrir con ello.
                let seguir = con_abierta(|a| match a.as_mut() {
                    Some(p) if !p.ficheros.is_empty() => {
                        p.hwnd = 0;
                        true
                    }
                    _ => {
                        *a = None;
                        false
                    }
                });
                if !seguir {
                    break;
                }
                estado = Estado::default();
            }
        });
    if let Err(e) = lanzado {
        con_abierta(|a| *a = None);
        tracing::warn!(?e, "no se pudo lanzar el hilo de la Salida");
    }
}

/// El idioma de la aplicacion, leido de sus ajustes: quien abre la Salida
/// (el editor, el lector) no siempre lo tiene a mano.
fn idioma_actual() -> Idioma {
    let dir_exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let appdata = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default();
    crate::compartir::idioma_de(&pixpin_store::resolver(&dir_exe, &appdata))
}

// --- Lo que se ensena ---------------------------------------------------------

/// Un fichero de la Salida, con lo que se pinta ya leido.
struct Ficha {
    ruta: PathBuf,
    nombre: String,
    /// «PDF · 1,2 MB».
    detalle: String,
    /// La extension en mayusculas, para la casilla sin miniatura.
    tipo: String,
    /// La miniatura, si el fichero la tiene (ver [`miniatura_de`]).
    imagen: Option<ImagenRgba>,
    bitmap: Option<ID2D1Bitmap1>,
}

impl Ficha {
    fn de(ruta: PathBuf, coma: char) -> Ficha {
        let bytes = std::fs::metadata(&ruta).map(|m| m.len()).unwrap_or(0);
        Ficha {
            nombre: pixpin_docs::nombre(&ruta),
            detalle: detalle_de(&ruta, bytes, coma),
            tipo: ruta
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_uppercase())
                .unwrap_or_default(),
            imagen: miniatura_de(&ruta),
            ruta,
            bitmap: None,
        }
    }
}

/// La miniatura de un fichero, si la tiene.
///
/// - Foto o video: la de la Shell, la misma del Explorador.
/// - PDF: su primera pagina, pintada aqui.
/// - Lo demas (una web, un Word): ninguna; la casilla dice su tipo. Ni la
///   «miniatura» de la Shell ni su icono valen: sin un visor propio
///   instalado los dos llegan como el icono del navegador sobre negro.
fn miniatura_de(ruta: &Path) -> Option<ImagenRgba> {
    let ext = ruta
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if es_imagen(ruta) || matches!(ext.as_str(), "mp4" | "mov" | "mkv" | "webm") {
        return pixpin_pin::miniatura_de(ruta, LADO_MINIATURA);
    }
    if ext == "pdf" {
        return pixpin_pdf::Documento::abrir(ruta)
            .and_then(|d| d.renderizar(0, LADO_MINIATURA))
            .inspect_err(|e| tracing::debug!(?e, "PDF sin miniatura en la Salida"))
            .ok();
    }
    None
}

/// El tipo y el peso de un fichero: «PDF · 1,2 MB». Sin extension, solo el
/// peso.
fn detalle_de(ruta: &Path, bytes: u64, coma: char) -> String {
    let peso = pixpin_ui::hoja_compartir::peso_legible(bytes, coma);
    match ruta.extension().and_then(|e| e.to_str()) {
        Some(e) if !e.is_empty() => format!("{} · {peso}", e.to_ascii_uppercase()),
        _ => peso,
    }
}

/// Si se pega como imagen ademas de como fichero.
fn es_imagen(ruta: &Path) -> bool {
    matches!(
        ruta.extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp")
    )
}

#[derive(Default)]
struct Estado {
    fichas: Vec<Ficha>,
    titulo: String,
    elegida: usize,
    aviso: Option<(String, bool, Instant)>,
    raton: (f32, f32),
}

impl Estado {
    /// Mete lo que llego (arriba) y elige lo nuevo. Devuelve si cambio.
    fn recibir(&mut self, ficheros: Vec<PathBuf>, titulo: Option<String>, coma: char) -> bool {
        // Un titulo nuevo tambien es un cambio: hay que repintar la cabecera.
        let titulo_cambia = match titulo {
            Some(t) if t != self.titulo => {
                self.titulo = t;
                true
            }
            _ => false,
        };
        let mut rutas: Vec<PathBuf> = self.fichas.iter().map(|f| f.ruta.clone()).collect();
        if !disposicion::juntar(&mut rutas, ficheros) {
            return titulo_cambia;
        }
        let mut viejas: Vec<Option<Ficha>> = std::mem::take(&mut self.fichas)
            .into_iter()
            .map(Some)
            .collect();
        self.fichas = rutas
            .into_iter()
            .map(|r| {
                // Lo que ya estaba conserva su miniatura: leerla otra vez de
                // la Shell es lo que mas tarda.
                viejas
                    .iter_mut()
                    .find(|v| v.as_ref().is_some_and(|f| f.ruta == r))
                    .and_then(Option::take)
                    .unwrap_or_else(|| Ficha::de(r, coma))
            })
            .collect();
        self.elegida = 0;
        true
    }

    fn avisar(&mut self, texto: String, bien: bool) {
        self.aviso = Some((texto, bien, Instant::now()));
    }

    fn elegida(&self) -> Option<&Ficha> {
        self.fichas.get(self.elegida).or_else(|| self.fichas.first())
    }

    /// Sube a la GPU las miniaturas que falten. Fuera del dibujo.
    fn subir(&mut self, motor: &MotorRender) {
        for f in &mut self.fichas {
            if f.bitmap.is_none()
                && let Some(img) = &f.imagen
            {
                f.bitmap = motor
                    .bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles)
                    .inspect_err(|e| tracing::warn!(?e, "miniatura de la Salida"))
                    .ok();
            }
        }
    }

    /// Suelta los bitmaps (el motor se va a rehacer).
    fn soltar_bitmaps(&mut self) {
        for f in &mut self.fichas {
            f.bitmap = None;
        }
    }
}

// --- La ventana ---------------------------------------------------------------

fn escala_de(monitor_escala: u32) -> f32 {
    monitor_escala as f32 / 100.0
}

fn medidas(d: &Disposicion) -> (u32, u32) {
    (d.ancho.ceil() as u32, d.alto.ceil() as u32)
}

fn flotar(recursos: &Recursos, textos: &Catalogo, e: &mut Estado) -> Result<()> {
    let coma = textos
        .t("compartir-coma-decimal")
        .chars()
        .next()
        .unwrap_or(',');
    let (llegan, titulo) = tomar_pendiente();
    e.soltar_bitmaps();
    e.recibir(llegan, titulo, coma);
    if e.fichas.is_empty() {
        return Ok(());
    }
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .monitor_en(pixpin_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let area = monitor.area_trabajo;
    let s = escala_de(monitor.escala_por_cien);
    let mut d = disposicion::disponer(e.fichas.len(), s);
    let (w, h) = medidas(&d);
    let mut marco = disposicion::marco_inicial(area, w, h, monitor.escala_por_cien);
    let mut ventana = VentanaOverlay::nueva(marco).context("no se pudo abrir la Salida")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(&motor, &recursos.d3d(), ventana.handle(), w, h)
        .context("sin superficie para la Salida")?;
    // Sin `enfocar`: aparece sin quitarle el teclado a nadie.
    ventana.mostrar();
    ventana.traer_encima();
    let hwnd = ventana.handle().0 as isize;
    con_abierta(|a| {
        if let Some(p) = a.as_mut() {
            p.hwnd = hwnd;
        }
    });

    let mut botones: Botones<Zona> = Botones::default();
    // Nace sin el raton encima: nada resaltado hasta que se mueva.
    e.raton = (-1.0, -1.0);
    // Donde se pulso (pantalla), el marco de entonces y que habia debajo.
    let mut pulsado: Option<(Punto, Rect, Zona)> = None;
    let mut hay_que_pintar = true;
    let mut vivo = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        let mut arrastre: Option<pixpin_pin::Carga> = None;
        let mut pulsar: Option<Zona> = None;
        for (hw, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hw != ventana.handle() {
                continue;
            }
            hay_que_pintar = true;
            let local = |p: Punto, m: Rect| ((p.x - m.x) as f32, (p.y - m.y) as f32);
            match ev {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::Tecla { vk, ctrl, .. } => match (vk, ctrl) {
                    (VK_ESCAPE, _) => vivo = false,
                    (VK_C, true) => pulsar = Some(Zona::Boton(Boton::Copiar)),
                    (VK_RETURN, _) => pulsar = Some(Zona::Boton(Boton::Abrir)),
                    (VK_ARRIBA, _) => e.elegida = e.elegida.saturating_sub(1),
                    (VK_ABAJO, _) => {
                        e.elegida = (e.elegida + 1).min(e.fichas.len().saturating_sub(1))
                    }
                    _ => {}
                },
                EventoOverlay::RatonMovido(p) => {
                    e.raton = local(p, marco);
                    if let Some((desde, origen, zona)) = pulsado {
                        let movido =
                            disposicion::supera_umbral((desde.x, desde.y), (p.x, p.y), s);
                        match zona {
                            Zona::Cabecera if movido => {
                                marco = Rect {
                                    x: origen.x + p.x - desde.x,
                                    y: origen.y + p.y - desde.y,
                                    ..origen
                                };
                                ventana.mover(marco);
                                e.raton = local(p, marco);
                            }
                            Zona::Tarjeta(i) if movido => {
                                arrastre = e
                                    .fichas
                                    .get(i)
                                    .map(|f| pixpin_pin::Carga::Fichero(f.ruta.clone()));
                                e.elegida = i;
                                pulsado = None;
                            }
                            Zona::Todos if movido => {
                                arrastre = Some(pixpin_pin::Carga::Ficheros(
                                    e.fichas.iter().map(|f| f.ruta.clone()).collect(),
                                ));
                                pulsado = None;
                            }
                            _ => {}
                        }
                    }
                }
                EventoOverlay::BotonPulsado(p) => {
                    e.raton = local(p, marco);
                    let zona = disposicion::zona_en(&d, e.raton);
                    if let Zona::Tarjeta(i) = zona {
                        e.elegida = i;
                    }
                    pulsado = Some((p, marco, zona));
                }
                EventoOverlay::BotonSoltado(p) => {
                    e.raton = local(p, marco);
                    if let Some((_, _, zona)) = pulsado.take()
                        && disposicion::zona_en(&d, e.raton) == zona
                    {
                        pulsar = Some(zona);
                    }
                }
                _ => {}
            }
        }
        if let Some(z) = pulsar {
            match z {
                Zona::Cerrar => vivo = false,
                Zona::Boton(b) => hacer(b, e, textos),
                // Pulsar sin arrastrar: lo que hace es decir como se usa.
                Zona::Todos => e.avisar(textos.t("salida-arrastra-todos"), true),
                _ => {}
            }
        }
        if !vivo {
            break;
        }
        if let Some(carga) = arrastre {
            // La captura del raton, fuera: `DoDragDrop` lleva el raton el solo.
            ventana.soltar_raton();
            match pixpin_pin::arrastrar(carga) {
                Ok(pixpin_pin::ResultadoArrastre::Soltado) => {
                    e.avisar(textos.t("salida-soltado"), true)
                }
                Ok(pixpin_pin::ResultadoArrastre::Cancelado) => {}
                Err(err) => {
                    tracing::warn!(?err, "no se pudo arrastrar desde la Salida");
                    e.avisar(textos.t("salida-no-se-pudo"), false);
                }
            }
            hay_que_pintar = true;
        }

        // Lo que llego mientras tanto, arriba; la ventana crece hacia arriba.
        let (llegan, titulo) = tomar_pendiente();
        if (!llegan.is_empty() || titulo.is_some()) && e.recibir(llegan, titulo, coma) {
            d = disposicion::disponer(e.fichas.len(), s);
            let (w, h) = medidas(&d);
            marco = disposicion::con_alto(Rect { ancho: w, ..marco }, h, area);
            ventana.mover(marco);
            if let Err(err) = superficie.redimensionar(w, h) {
                tracing::warn!(?err, "la Salida no pudo crecer");
            }
            ventana.traer_encima();
            hay_que_pintar = true;
        }
        if e.aviso.as_ref().is_some_and(|(_, _, t)| t.elapsed() > AVISO) {
            e.aviso = None;
            hay_que_pintar = true;
        }
        // La mano de arrastrar encima de lo que se arrastra.
        let sobre = disposicion::zona_en(&d, e.raton);
        ventana.poner_cursor(match sobre {
            Zona::Tarjeta(_) | Zona::Todos | Zona::Cabecera => FormaCursorWin::Mover,
            _ => FormaCursorWin::Flecha,
        });
        if hay_que_pintar {
            hay_que_pintar = false;
            e.subir(&motor);
            if let Ok(destino) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&destino, |p: &Pintor| {
                    // Las esquinas redondeadas dejan ver lo de debajo.
                    p.limpiar_transparente();
                    pintar(p, &d, e, &mut botones, textos, s)
                });
                let _ = superficie.presentar();
            }
        }
        pixpin_shell::overlay::esperar_eventos(Some(if e.aviso.is_some() { 200 } else { 1000 }));
    }
    ventana.ocultar();
    Ok(())
}

/// **Un boton de abajo**, sobre la tarjeta elegida.
fn hacer(b: Boton, e: &mut Estado, textos: &Catalogo) {
    let Some(ruta) = e.elegida().map(|f| f.ruta.clone()) else {
        return;
    };
    let una = std::slice::from_ref(&ruta);
    match b {
        Boton::Copiar => {
            let hecho = if es_imagen(&ruta) {
                match pixpin_codec::cargar(&ruta) {
                    Ok(img) => pixpin_codec::portapapeles::copiar_imagen_y_ficheros(&img, una),
                    Err(_) => pixpin_codec::portapapeles::copiar_ficheros(una),
                }
            } else {
                pixpin_codec::portapapeles::copiar_ficheros(una)
            };
            match hecho {
                Ok(()) => e.avisar(textos.t("salida-copiado"), true),
                Err(err) => {
                    tracing::warn!(?err, "no se pudo copiar desde la Salida");
                    e.avisar(textos.t("salida-no-se-pudo"), false);
                }
            }
        }
        Boton::Abrir => {
            if let Err(err) = pixpin_shell::abrir(&ruta) {
                tracing::warn!(?err, "no se pudo abrir desde la Salida");
                e.avisar(textos.t("salida-no-se-pudo"), false);
            }
        }
        Boton::Carpeta => {
            if let Err(err) = pixpin_shell::abrir_ubicacion(&ruta) {
                tracing::warn!(?err, "no se pudo mostrar en la carpeta");
                e.avisar(textos.t("salida-no-se-pudo"), false);
            }
        }
        Boton::Pinear => {
            // Por el mismo camino que «Abrir con PixPin»: la ventana
            // principal, que es la que tiene los pines.
            if pixpin_shell::mensajero::enviar_ficheros(una) {
                e.avisar(textos.t("salida-pineado"), true);
            } else {
                e.avisar(textos.t("salida-no-se-pudo"), false);
            }
        }
    }
}

// --- Pintar -------------------------------------------------------------------

fn icono_de(b: Boton) -> &'static Icono {
    match b {
        Boton::Copiar => &mi::CONTENT_COPY,
        Boton::Abrir => &mi::OPEN_IN_NEW,
        Boton::Carpeta => &mi::FOLDER,
        Boton::Pinear => &mi::PUSH_PIN,
    }
}

/// La pista de un boton de icono, con su atajo si lo tiene.
fn pista_de(b: Boton, textos: &Catalogo) -> String {
    match b {
        Boton::Copiar => format!("{} · Ctrl C", textos.t("salida-copiar")),
        Boton::Abrir => format!("{} · Enter", textos.t("salida-abrir")),
        Boton::Carpeta => textos.t("salida-carpeta"),
        Boton::Pinear => textos.t("salida-pinear"),
    }
}

/// Los puntitos de agarre (2 x 3): lo que dice «esto se arrastra» sin
/// palabras, como en cualquier lista que se reordena.
pub(crate) fn agarre(p: &Pintor, cx: f32, cy: f32, color: Color, s: f32) {
    let (dx, dy, r) = (5.0 * s, 5.0 * s, 1.6 * s);
    for fila in -1..=1 {
        for col in [-0.5f32, 0.5] {
            p.circulo((cx + col * dx, cy + fila as f32 * dy), r, color);
        }
    }
}

fn pintar(
    p: &Pintor,
    d: &Disposicion,
    e: &Estado,
    botones: &mut Botones<Zona>,
    textos: &Catalogo,
    s: f32,
) {
    botones.vaciar();
    botones.raton = e.raton;
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: d.ancho,
        alto: d.alto,
    };
    let radio = v2::RADIO_FLOTANTE * s;
    p.rellenar_redondeado(todo, radio, blanco(0.14));
    p.rellenar_redondeado(v2::geom::encoger(todo, 1.0 * s), radio - 1.0 * s, v2::FONDO);

    // La cabecera: lo hecho y, debajo, como se usa (o el aviso).
    let x0 = disposicion::MARGEN * s + 4.0 * s;
    let ancho_titulo = (d.cerrar.x - x0 - 8.0 * s).max(0.0);
    let titulo = if e.titulo.is_empty() {
        textos.t("salida-titulo")
    } else {
        e.titulo.clone()
    };
    crate::lecciones::ui::negrita(
        p,
        &titulo,
        x0,
        12.0 * s,
        v2::LETRA_TITULO_TARJETA * s,
        ancho_titulo,
        v2::TEXTO,
    );
    let (linea, color) = match &e.aviso {
        Some((t, true, _)) => (t.clone(), v2::VERDE),
        Some((t, false, _)) => (t.clone(), v2::ROJO_TEXTO),
        None => (textos.t("salida-pista"), v2::GRIS),
    };
    p.texto_linea(
        &linea,
        x0,
        35.0 * s,
        v2::LETRA_SECUNDARIO * s,
        ancho_titulo,
        color,
    );
    v2::boton_icono(
        p,
        botones,
        d.cerrar,
        Zona::Cerrar,
        &mi::CLOSE,
        false,
        false,
        s,
    );

    // Arrastrar todos juntos.
    if let Some(r) = d.todos {
        let encima = v2::geom::dentro(r, e.raton);
        p.rellenar_redondeado(r, v2::RADIO_BOTON * s, blanco(if encima { 0.08 } else { 0.04 }));
        p.trazar_discontinuo(r, (1.2 * s).max(1.0), blanco(if encima { 0.4 } else { 0.22 }));
        let mut a = fluent_bundle::FluentArgs::new();
        a.set("n", e.fichas.len());
        let texto = textos.t_args("salida-todos", &a);
        let tam = v2::LETRA_SECUNDARIO * s;
        let (tw, th) = p.medir_texto(&texto, tam);
        let total = 14.0 * s + tw;
        let x = r.x + (r.ancho - total) / 2.0;
        let tinta = if encima { v2::TEXTO } else { v2::CUERPO };
        agarre(p, x + 3.0 * s, r.y + r.alto / 2.0, tinta, s);
        p.texto(&texto, x + 14.0 * s, r.y + (r.alto - th) / 2.0, tam, tinta);
        botones.zona(r, Zona::Todos);
    }

    // Las tarjetas.
    for (i, (r, f)) in d.tarjetas.iter().zip(&e.fichas).enumerate() {
        let encima = v2::geom::dentro(*r, e.raton);
        let elegida = i == e.elegida && e.fichas.len() > 1;
        v2::tarjeta::fondo(p, *r, encima, elegida, s);
        let lado = 48.0 * s;
        let mini = RectF {
            x: r.x + 8.0 * s,
            y: r.y + (r.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        };
        match (&f.bitmap, &f.imagen) {
            // Una hoja (mas alta que ancha), desde arriba: su cabecera dice
            // mas que su mitad.
            (Some(b), Some(img)) if img.alto > img.ancho => {
                p.bitmap(
                    b,
                    mini,
                    Some(RectF {
                        x: 0.0,
                        y: 0.0,
                        ancho: img.ancho as f32,
                        alto: img.ancho as f32,
                    }),
                    false,
                );
            }
            (Some(b), Some(img)) => {
                crate::miniaturas::pintar_recortado(p, b, mini, img.ancho, img.alto);
            }
            // Sin miniatura: la hoja doblada y su tipo debajo, como un
            // icono de fichero pero con los colores de la ventana.
            _ => {
                p.rellenar_redondeado(mini, 8.0 * s, v2::CAJA);
                let li = 22.0 * s;
                p.icono(
                    &mi::DESCRIPTION,
                    RectF {
                        x: mini.x + (lado - li) / 2.0,
                        y: mini.y + 6.0 * s,
                        ancho: li,
                        alto: li,
                    },
                    v2::ACENTO,
                );
                let tam = 10.5 * s;
                let (tw, _) = p.medir_texto(&f.tipo, tam);
                p.texto_linea(
                    &f.tipo,
                    mini.x + ((lado - tw) / 2.0).max(2.0 * s),
                    mini.y + 30.0 * s,
                    tam,
                    lado - 4.0 * s,
                    v2::CUERPO,
                );
            }
        }
        let tx = mini.x + lado + 12.0 * s;
        let grip_x = r.x + r.ancho - 20.0 * s;
        let tw = (grip_x - tx - 12.0 * s).max(0.0);
        p.texto_linea(
            &f.nombre,
            tx,
            r.y + 13.0 * s,
            v2::LETRA_CUERPO * s,
            tw,
            v2::TEXTO,
        );
        p.texto_linea(
            &f.detalle,
            tx,
            r.y + 35.0 * s,
            v2::LETRA_SECUNDARIO * s,
            tw,
            v2::GRIS,
        );
        agarre(
            p,
            grip_x,
            r.y + r.alto / 2.0,
            if encima { v2::TEXTO } else { blanco(0.35) },
            s,
        );
        botones.zona(*r, Zona::Tarjeta(i));
    }

    // Los botones de abajo.
    let mut pista = None;
    for (b, r) in &d.botones {
        if *b == Boton::Copiar {
            crate::lecciones::ui::boton_v2(
                p,
                botones,
                *r,
                Zona::Boton(*b),
                Some(icono_de(*b)),
                &textos.t("salida-copiar"),
                None,
                Some(v2::AZUL_LLENO),
                Color::BLANCO,
                s,
            );
        } else {
            v2::boton_icono(p, botones, *r, Zona::Boton(*b), icono_de(*b), false, false, s);
            if v2::geom::dentro(*r, e.raton) {
                pista = Some((*r, pista_de(*b, textos)));
            }
        }
    }
    // La pista encima de todo lo demas.
    if let Some((r, t)) = pista {
        v2::pista::pintar(p, r, &t, todo, s);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_detalle_dice_el_tipo_y_el_peso() {
        assert_eq!(
            detalle_de(Path::new(r"C:\x\informe.pdf"), 1_258_291, ','),
            "PDF · 1,2 MB"
        );
        assert_eq!(detalle_de(Path::new(r"C:\x\foto.png"), 812, ','), "PNG · 812 B");
        // Caso negativo: sin extension no se inventa un tipo.
        assert_eq!(detalle_de(Path::new(r"C:\x\LEEME"), 812, ','), "812 B");
    }

    #[test]
    fn solo_entran_ficheros_que_existen_con_ruta_entera() {
        let dir = std::env::temp_dir().join(format!("pixpin-salida-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.txt");
        std::fs::write(&f, b"x").unwrap();
        let v = validos(vec![
            f.clone(),
            dir.join("no-esta.txt"),
            PathBuf::from("relativo.txt"),
            dir.clone(),
        ]);
        // Caso negativo: lo que no existe, lo relativo y una carpeta fuera.
        assert_eq!(v, vec![f]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn copiar_pega_como_imagen_solo_las_imagenes() {
        assert!(es_imagen(Path::new(r"C:\x\a.PNG")));
        assert!(es_imagen(Path::new(r"C:\x\a.jpeg")));
        // Caso negativo: un PDF o una pagina web van como fichero.
        assert!(!es_imagen(Path::new(r"C:\x\a.pdf")));
        assert!(!es_imagen(Path::new(r"C:\x\a.html")));
    }

    #[test]
    fn lo_que_llega_va_arriba_elegido_y_lo_que_estaba_guarda_su_miniatura() {
        let dir = std::env::temp_dir().join(format!("pixpin-salida-r-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.txt");
        let b = dir.join("b.txt");
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"y").unwrap();
        let mut e = Estado::default();
        assert!(e.recibir(vec![a.clone()], Some("Exportado".into()), ','));
        e.fichas[0].detalle = "marcada".into();
        e.elegida = 0;
        assert!(e.recibir(vec![b.clone()], None, ','));
        assert_eq!(e.fichas.len(), 2);
        assert_eq!(e.fichas[0].ruta, b);
        assert_eq!(e.elegida, 0, "lo nuevo queda elegido");
        assert_eq!(e.fichas[1].detalle, "marcada", "no se ha vuelto a leer");
        assert_eq!(e.titulo, "Exportado");
        // Caso negativo: lo mismo otra vez, en el mismo orden, no cambia.
        assert!(!e.recibir(vec![b], None, ','));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// **Muestra de la Salida** fuera de pantalla, con una foto y un PDF de
    /// ejemplo. `PIXPIN_MUESTRAS` dice donde dejar el PNG.
    ///
    /// `cargo test -p pixpin --bin pixpinmax -- --ignored --test-threads=1 muestra_de_la_salida`
    #[test]
    #[ignore = "necesita GPU; deja un PNG para mirarlo"]
    fn muestra_de_la_salida() {
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let datos = carpeta.join("ejemplo");
        std::fs::create_dir_all(&datos).unwrap();
        // Una foto con un degradado y un PDF de una pagina.
        let (w, h) = (640u32, 420u32);
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let franja = (y as f32 / h as f32 - 0.6).abs() < 0.04;
                let v = if franja { 0.5 } else { 1.0 };
                px.extend_from_slice(&[
                    ((60 + x * 180 / w) as f32 * v) as u8,
                    ((120 + y * 100 / h) as f32 * v) as u8,
                    (200.0 * v) as u8,
                    255,
                ]);
            }
        }
        let foto = datos.join("Fachada con cotas.png");
        pixpin_codec::guardar(
            &ImagenRgba {
                ancho: w,
                alto: h,
                pixeles: px,
            },
            &foto,
            pixpin_codec::FormatoImagen::Png,
        )
        .unwrap();
        let pdf = datos.join("Presupuesto de la obra - octubre (anotado).pdf");
        // Una hoja con una cabecera azul y renglones grises, dibujados con
        // rectangulos (sin letras, que necesitarian incrustar una fuente).
        let mut hoja = String::from("0.15 0.4 0.85 rg 20 245 170 25 re f 0.6 0.6 0.6 rg\n");
        for k in 0..9 {
            let ancho = if k % 3 == 2 { 110 } else { 170 };
            hoja.push_str(&format!("20 {} {ancho} 7 re f\n", 220 - k * 18));
        }
        // Con su tabla de referencias: sin ella el lector de PDF la rechaza.
        let objetos = [
            "<</Type/Catalog/Pages 2 0 R>>".to_string(),
            "<</Type/Pages/Kids[3 0 R]/Count 1>>".to_string(),
            "<</Type/Page/Parent 2 0 R/MediaBox[0 0 210 297]/Contents 4 0 R>>".to_string(),
            format!("<</Length {}>>stream\n{hoja}endstream", hoja.len()),
        ];
        let mut pdf_bytes = String::from("%PDF-1.4\n");
        let mut sitios = Vec::new();
        for (k, o) in objetos.iter().enumerate() {
            sitios.push(pdf_bytes.len());
            pdf_bytes.push_str(&format!("{} 0 obj\n{o}\nendobj\n", k + 1));
        }
        let xref = pdf_bytes.len();
        pdf_bytes.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objetos.len() + 1));
        for s in sitios {
            pdf_bytes.push_str(&format!("{s:010} 00000 n \n"));
        }
        pdf_bytes.push_str(&format!(
            "trailer<</Size {}/Root 1 0 R>>\nstartxref\n{xref}\n%%EOF\n",
            objetos.len() + 1
        ));
        std::fs::write(&pdf, pdf_bytes).unwrap();
        let web = datos.join("Lecciones de octubre.html");
        std::fs::write(&web, "<!doctype html><title>x</title>").unwrap();

        let textos = Catalogo::nuevo(Idioma::Espanol);
        let d3d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d3d.d3d()).expect("motor");
        for (nombre, ficheros, raton, aviso) in [
            ("salida-dos", vec![pdf.clone(), foto.clone()], (-1.0, -1.0), None),
            (
                "salida-tres",
                vec![web.clone(), pdf.clone(), foto.clone()],
                (-1.0, -1.0),
                None,
            ),
            ("salida-una", vec![foto.clone()], (-1.0, -1.0), None),
        ]
        .into_iter()
        .chain([(
            "salida-encima",
            vec![pdf.clone(), foto.clone()],
            (0.0, 0.0),
            Some(("Copiado al portapapeles".to_string(), true)),
        )])
        {
            let s = 1.5;
            let mut e = Estado::default();
            e.recibir(ficheros, Some(textos.t("salida-titulo-exportado")), ',');
            let d = disposicion::disponer(e.fichas.len(), s);
            e.raton = if raton == (0.0, 0.0) {
                // Encima del boton de copiar: se ve su pista.
                let r = d.botones[1].1;
                (r.x + r.ancho / 2.0, r.y + r.alto / 2.0)
            } else {
                raton
            };
            e.aviso = aviso.map(|(t, b)| (t, b, Instant::now()));
            e.subir(&motor);
            let (w, h) = medidas(&d);
            // Sobre un fondo claro, como un escritorio: se ve el borde.
            let fuera = FueraDePantalla::nuevo(&motor, d3d.d3d(), w + 40, h + 40).unwrap();
            let mut botones = Botones::default();
            motor
                .dibujar(&fuera.destino, |p| {
                    p.limpiar(Color {
                        r: 0.82,
                        g: 0.85,
                        b: 0.9,
                        a: 1.0,
                    });
                    p.desplazar(20.0, 20.0);
                    pintar(p, &d, &e, &mut botones, &textos, s);
                    p.desplazar(0.0, 0.0);
                })
                .unwrap();
            fuera.esperar_gpu().unwrap();
            let (fw, fh, pixeles) = fuera.leer_rgba().unwrap();
            let ruta = carpeta.join(format!("{nombre}.png"));
            pixpin_codec::guardar(
                &ImagenRgba {
                    ancho: fw,
                    alto: fh,
                    pixeles,
                },
                &ruta,
                pixpin_codec::FormatoImagen::Png,
            )
            .unwrap();
            println!("{nombre}: {}", ruta.display());
        }
    }

    /// **Para mirarla a ojo**: abre la Salida de verdad con una pagina web
    /// que lleva una nota de voz dentro, y la deja 8 s en pantalla. Con
    /// `PIXPIN_SALIDA_DE_VERDAD=1`.
    #[test]
    #[ignore]
    fn muestra_la_salida_con_una_pagina_con_adjuntos() {
        let dir = std::env::temp_dir().join("pixpin-muestra-salida");
        std::fs::create_dir_all(&dir).unwrap();
        let voz = dir.join("nota de voz.m4a");
        std::fs::write(&voz, b"audio de prueba").unwrap();
        let trozo = crate::compartir::adjuntos::html(&[voz], "Adjuntos");
        let pagina = dir.join("Proyecto de prueba.html");
        std::fs::write(
            &pagina,
            crate::compartir::adjuntos::pagina_sola("Proyecto de prueba", &trozo),
        )
        .unwrap();
        mostrar(vec![pagina], "Listo para compartir".into());
        std::thread::sleep(std::time::Duration::from_secs(8));
    }
}
