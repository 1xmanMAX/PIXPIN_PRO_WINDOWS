//! **Un PowerPoint en el chat: presentarlo y meterlo en el proyecto** (D10).
//!
//! En el movil (`guardados/DiapositivasAPdf.kt`, v. 14-sep-2026) una
//! presentacion se convierte en un PDF —una pagina por diapositiva— y entra
//! **por el mismo camino que un PDF**: proyecto con una hoja por pagina,
//! anotar encima, presentar. Aqui igual, con una diferencia de medios: el
//! PDF no lo maqueta PixPin (`motor/Diapositivas.kt`, sin graficos, SmartArt
//! ni degradados), lo saca **el PowerPoint que ya tenga el equipo**
//! (`pixpin_shell::powerpoint`), que lo hace exacto y sin codigo de
//! maquetacion. Sin PowerPoint se dice, y se ofrece «Abrir con otra app»
//! (el visor de la tienda, LibreOffice, lo que haya).
//!
//! **Desde el 10-oct-2026 presentar ya no abre PowerPoint**: un `.pptx` (y
//! `.pptm`, `.ppsx`, `.potx`...) lo maqueta PixPin (`pixpin-office`) y cada
//! diapositiva se pinta en vectores a la medida de la pantalla
//! ([`presentar_nativa`], lo que pide «Presentar» en el lector de Office).
//! PowerPoint queda para lo que PixPin no lee (`.ppt`, `.odp`, o un `.pptx`
//! que no se deja) y para «Anadir al proyecto», que sigue uniendo su PDF.
//!
//! # Como se usa
//!
//! - **Tocar la burbuja** de un `.pptx` (o «Abrir aqui») lo **presenta**: la
//!   ventana sale a pantalla completa en el acto con «Preparando…», y la
//!   primera diapositiva en cuanto PowerPoint acaba.
//! - **«Anadir al proyecto»** y **«Anadir como imagenes»** hacen lo mismo que
//!   con un PDF: el PDF convertido pasa por `pdf_en_chat` sin cambiar nada
//!   de alli ([`pdf_para_unir`]).
//!
//! La conversion se guarda en la cache del almacen (fuera del proyecto: no
//! viaja al empaquetar), con la fecha y el tamano del original en la
//! clave: presentar dos veces no llama dos veces a PowerPoint.
//!
//! # La presentacion (`ui/Presentacion.kt`)
//!
//! La diapositiva entera, centrada sobre negro, y abajo la pastilla
//! «← n / total →  ✕». Pasar con el raton como con el dedo en el movil
//! (`ZonaDePasar`): clic en el tercio derecho, siguiente; en el izquierdo,
//! anterior; en el medio, esconder o ensenar la pastilla. Y el teclado de
//! siempre de PowerPoint, que es tambien lo que mandan los **mandos de
//! presentar** (flechas, AvPag/RePag, Espacio, Intro, Inicio/Fin, Esc,
//! `B`/`.` pantalla negra). Sin atajos globales: solo los de la ventana.
//!
//! # Lo que se presenta maquetado por PixPin (11-oct-2026)
//!
//! - **Notas del orador**, leidas solo al pedirlas. Con dos pantallas, la
//!   presentacion va a la otra (el proyector) y en la principal (la del
//!   portatil) sale la **vista del orador**: la de ahora, la siguiente, las
//!   notas, el tiempo que lleva y la hora. Con una sola, la `N` ensena o
//!   esconde las notas en una franja abajo.
//! - **Transiciones sencillas** (fundido, empuje, barrido, cubrir,
//!   descubrir, corte; las demas como fundido) con su duracion, y el pase
//!   solo (`advTm`). Cada diapositiva se pinta una vez a un mapa de bits de
//!   la medida de la pantalla y la transicion solo mezcla los dos ya
//!   pintados (`pixpin_office::pptx::pase::capas`), sin maquetar en cada
//!   fotograma. La `T` las apaga y las enciende.
//! - **Videos y audios**: su poster con el boton de reproducir; un clic en
//!   un audio lo suena en el reproductor flotante, y un video se abre con el
//!   reproductor del sistema (PixPin no reproduce video).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use anyhow::{Context, Result};
use pixpin_codec::imagen::ImagenRgba;
use pixpin_office::pptx::pase::{self as pp, Capa, Efecto, Medio, Transicion};
use pixpin_render::icono::material;
use pixpin_render::pagina_office::PintorOffice;
use pixpin_render::{Color, Interpolacion, MotorRender, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Catalogo;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use crate::lector::{APAGADO, CRISTAL, TEXTO, ahora_ms, con_alfa, dentro};
use crate::overlay::Recursos;

// ---------------------------------------------------------------------------
// Que es una presentacion y que se puede leer

/// Si el fichero es una presentacion (o un Keynote, que se reconoce para
/// decir que no se lee). Como `Diapositivas.esPresentacion` del movil.
pub fn es_presentacion(nombre: &str) -> bool {
    matches!(
        pixpin_docs::extension(nombre).as_str(),
        "pptx" | "pptm" | "ppsx" | "ppsm" | "potx" | "potm" | "ppt" | "pps" | "pot" | "odp" | "key"
    )
}

/// Por que no se puede presentar, si no se puede: la clave del texto.
///
/// El movil no lee el `.ppt` antiguo ni el `.odp`; aqui PowerPoint abre los
/// dos, asi que el unico que queda fuera, con PowerPoint, es Keynote.
pub fn por_que_no(nombre: &str, hay_powerpoint: bool) -> Option<&'static str> {
    if !es_presentacion(nombre) {
        return Some("diapositivas-no-es");
    }
    if pixpin_docs::extension(nombre) == "key" {
        return Some("diapositivas-keynote");
    }
    if !hay_powerpoint {
        return Some("diapositivas-sin-powerpoint");
    }
    None
}

/// Donde queda el PDF de una presentacion: en la cache del almacen, con el
/// tamano y la fecha del original en el nombre (si se cambia, se vuelve a
/// convertir; si no, se reutiliza).
pub fn pdf_en_cache(raiz: &Path, origen: &Path) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let meta = std::fs::metadata(origen).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    origen.hash(&mut h);
    meta.len().hash(&mut h);
    meta.modified().ok().hash(&mut h);
    Some(
        raiz.join("cache")
            .join("diapositivas")
            .join(format!("{:016x}.pdf", h.finish())),
    )
}

/// El PDF de la presentacion, convirtiendola si aun no lo estaba. Bloquea
/// (segundos): llamarla desde un hilo propio.
pub fn convertir(raiz: &Path, origen: &Path) -> std::result::Result<PathBuf, String> {
    let nombre = pixpin_docs::nombre(origen);
    if let Some(clave) = por_que_no(&nombre, pixpin_shell::powerpoint::hay_powerpoint()) {
        return Err(clave.to_string());
    }
    let destino = pdf_en_cache(raiz, origen).ok_or("diapositivas-no-esta")?;
    if destino.is_file() {
        return Ok(destino);
    }
    if let Some(carpeta) = destino.parent() {
        std::fs::create_dir_all(carpeta).map_err(|e| e.to_string())?;
    }
    // A un nombre de paso y luego de un tiron: un PowerPoint que se cae a
    // medias no deja un PDF roto que la cache daria por bueno.
    let temporal = destino.with_extension("tmp.pdf");
    let _ = std::fs::remove_file(&temporal);
    let empezo = std::time::Instant::now();
    pixpin_shell::powerpoint::a_pdf(origen, &temporal).map_err(|e| e.to_string())?;
    std::fs::rename(&temporal, &destino).map_err(|e| e.to_string())?;
    tracing::info!(
        ms = empezo.elapsed().as_millis() as u64,
        origen = %origen.display(),
        "presentacion convertida a PDF con PowerPoint"
    );
    Ok(destino)
}

/// **El gancho de `pdf_en_chat`**: lo que se une al proyecto. Un PDF va tal
/// cual; una presentacion, su PDF. Asi «Anadir al proyecto» y «como
/// imagenes» valen para las dos sin tocar nada mas de alli.
pub fn pdf_para_unir(raiz: &Path, ruta: &Path) -> std::result::Result<PathBuf, String> {
    if es_presentacion(&pixpin_docs::nombre(ruta)) {
        convertir(raiz, ruta)
    } else {
        Ok(ruta.to_path_buf())
    }
}

// ---------------------------------------------------------------------------
// Pasar diapositivas

/// Lo que pide una tecla o un clic en la presentacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mando {
    Siguiente,
    Anterior,
    Primera,
    Ultima,
    /// Pantalla en negro y vuelta (la `B` o el `.` de PowerPoint, que
    /// tambien tiene su boton en muchos mandos).
    Negro,
    /// Esconder o ensenar la pastilla.
    Pastilla,
    /// Ensenar o esconder las notas del orador en una franja abajo (`N`;
    /// con dos pantallas van en la vista del orador).
    Notas,
    /// Encender o apagar las transiciones (`T`).
    Transiciones,
    Salir,
}

const VK_RETROCESO: u32 = 0x08;
const VK_INTRO: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_ESPACIO: u32 = 0x20;
const VK_PRIOR: u32 = 0x21;
const VK_NEXT: u32 = 0x22;
const VK_END: u32 = 0x23;
const VK_HOME: u32 = 0x24;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;
const VK_B: u32 = 0x42;
const VK_N: u32 = 0x4E;
const VK_P: u32 = 0x50;
const VK_T: u32 = 0x54;
const VK_PUNTO: u32 = 0xBE;

/// Las teclas de PowerPoint en pase de diapositivas, que son las que mandan
/// los mandos de presentar (casi todos: AvPag/RePag; otros, las flechas).
/// La `N` (que en PowerPoint tambien pasa) aqui ensena las notas, y la `T`
/// apaga o enciende las transiciones.
pub fn mando_de_tecla(vk: u32, shift: bool) -> Option<Mando> {
    Some(match vk {
        VK_RIGHT | VK_DOWN | VK_NEXT | VK_INTRO => Mando::Siguiente,
        VK_ESPACIO if shift => Mando::Anterior,
        VK_ESPACIO => Mando::Siguiente,
        VK_LEFT | VK_UP | VK_PRIOR | VK_RETROCESO | VK_P => Mando::Anterior,
        VK_HOME => Mando::Primera,
        VK_END => Mando::Ultima,
        VK_B | VK_PUNTO => Mando::Negro,
        VK_N => Mando::Notas,
        VK_T => Mando::Transiciones,
        VK_ESCAPE => Mando::Salir,
        _ => return None,
    })
}

/// Un clic sin arrastrar: por tercios, como `ZonaDePasar` del movil.
pub fn mando_de_clic(x: f32, ancho: f32) -> Mando {
    let tercio = ancho / 3.0;
    if x < tercio {
        Mando::Anterior
    } else if x > tercio * 2.0 {
        Mando::Siguiente
    } else {
        Mando::Pastilla
    }
}

/// Por donde va el pase. `total` es 0 mientras no se sabe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pase {
    pub actual: usize,
    pub total: usize,
    pub negro: bool,
    pub pastilla: bool,
}

impl Pase {
    /// Aplica un mando. `false` si hay que salir.
    ///
    /// No da la vuelta: pasar de la ultima se queda en la ultima, como la
    /// pastilla del movil, que apaga «siguiente» al final. Con la pantalla
    /// en negro, cualquier mando de pasar primero la enciende (PowerPoint).
    pub fn aplicar(&mut self, m: Mando) -> bool {
        let ultima = self.total.saturating_sub(1);
        if self.negro
            && !matches!(
                m,
                Mando::Salir | Mando::Pastilla | Mando::Notas | Mando::Transiciones
            )
        {
            self.negro = false;
            if m == Mando::Negro {
                return true;
            }
        }
        match m {
            Mando::Siguiente => self.actual = (self.actual + 1).min(ultima),
            Mando::Anterior => self.actual = self.actual.saturating_sub(1),
            Mando::Primera => self.actual = 0,
            Mando::Ultima => self.actual = ultima,
            Mando::Negro => self.negro = true,
            Mando::Pastilla => self.pastilla = !self.pastilla,
            // Las lleva el bucle: no mueven el pase.
            Mando::Notas | Mando::Transiciones => {}
            Mando::Salir => return false,
        }
        true
    }

    pub fn puede_anterior(&self) -> bool {
        self.actual > 0
    }

    pub fn puede_siguiente(&self) -> bool {
        self.total == 0 || self.actual + 1 < self.total
    }
}

/// La caja donde cabe entera una diapositiva de `pagina` en `pantalla`,
/// centrada.
pub fn encajar(pagina: (f32, f32), pantalla: (f32, f32)) -> RectF {
    let (pw, ph) = (pagina.0.max(1.0), pagina.1.max(1.0));
    let s = (pantalla.0 / pw).min(pantalla.1 / ph);
    let (w, h) = (pw * s, ph * s);
    RectF {
        x: (pantalla.0 - w) / 2.0,
        y: (pantalla.1 - h) / 2.0,
        ancho: w,
        alto: h,
    }
}

/// Como [`encajar`], a pixeles enteros: la diapositiva se pinta a un mapa
/// de bits de ese tamano y se copia sin reescalar (nitida).
fn encajar_entera(pagina: (f32, f32), pantalla: (f32, f32)) -> RectF {
    let c = encajar(pagina, pantalla);
    RectF {
        x: c.x.round(),
        y: c.y.round(),
        ancho: c.ancho.round().max(1.0),
        alto: c.alto.round().max(1.0),
    }
}

// ---------------------------------------------------------------------------
// Dos pantallas: la presentacion y la vista del orador

/// Que pantalla es de que: `(la de la presentacion, la del orador)`. Con
/// una sola, la presentacion va en ella y no hay vista del orador. Con dos
/// o mas, el orador se queda la principal (la del portatil, la que tiene
/// delante) y la presentacion va a la otra (el proyector), como PowerPoint.
pub fn repartir(
    monitores: &[pixpin_geom::Monitor],
) -> Option<(pixpin_geom::Monitor, Option<pixpin_geom::Monitor>)> {
    let principal = monitores
        .iter()
        .find(|m| m.principal)
        .or(monitores.first())?;
    Some(match monitores.iter().find(|m| m.id != principal.id) {
        Some(otro) => (*otro, Some(*principal)),
        None => (*principal, None),
    })
}

/// El tiempo que lleva la presentacion: `05:07`, o `1:02:03` pasada la
/// hora.
pub fn cronometro(ms: u64) -> String {
    let s = ms / 1000;
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// La hora del reloj (`14:05`) desde los ms locales de `ahora_local_ms`.
pub fn hora(local_ms: i64) -> String {
    let minutos = local_ms.div_euclid(60_000).rem_euclid(24 * 60);
    format!("{:02}:{:02}", minutos / 60, minutos % 60)
}

/// Donde va cada cosa en la vista del orador: la diapositiva de ahora
/// grande a la izquierda; a la derecha, la siguiente arriba y las notas
/// debajo; arriba, la cuenta, el tiempo y la hora.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZonasOrador {
    pub actual: RectF,
    pub siguiente: RectF,
    pub notas: RectF,
    /// Donde empieza (en alto) la fila de arriba.
    pub cabecera: f32,
    /// Lo que mide un rotulo («Siguiente», «Notas») encima de su zona.
    pub rotulo: f32,
}

pub fn zonas_orador(ancho: f32, alto: f32, e: f32, pagina: (f32, f32)) -> ZonasOrador {
    let m = 24.0 * e;
    let cab = 56.0 * e;
    let rotulo = 30.0 * e;
    let y0 = m + cab;
    let h = (alto - y0 - m).max(1.0);
    let util = (ancho - 3.0 * m).max(2.0);
    let (wl, wr) = (util * 0.6, util * 0.4);
    let x1 = 2.0 * m + wl;
    let a = encajar(pagina, (wl, h));
    let actual = RectF {
        x: m + a.x,
        y: y0 + a.y,
        ancho: a.ancho,
        alto: a.alto,
    };
    let s = encajar(pagina, (wr, (h * 0.45 - rotulo).max(1.0)));
    let siguiente = RectF {
        x: x1 + s.x,
        y: y0 + rotulo,
        ancho: s.ancho,
        alto: s.alto,
    };
    let ny = siguiente.y + siguiente.alto + m + rotulo;
    let notas = RectF {
        x: x1,
        y: ny,
        ancho: wr,
        alto: (alto - m - ny).max(0.0),
    };
    ZonasOrador {
        actual,
        siguiente,
        notas,
        cabecera: m,
        rotulo,
    }
}

/// La ventana del orador, en la pantalla del portatil.
struct Orador {
    ventana: VentanaOverlay,
    superficie: Superficie,
    ancho: f32,
    alto: f32,
    e: f32,
}

impl Orador {
    fn nuevo(
        recursos: &Recursos,
        motor: &MotorRender,
        monitor: &pixpin_geom::Monitor,
        nombre: &str,
    ) -> Result<Orador> {
        let area = monitor.area;
        let ventana =
            VentanaOverlay::nueva_normal(area, nombre).context("sin ventana del orador")?;
        let superficie = Superficie::nueva(
            motor,
            &recursos.d3d(),
            ventana.handle(),
            area.ancho,
            area.alto,
        )
        .context("sin superficie del orador")?;
        ventana.mostrar();
        Ok(Orador {
            ventana,
            superficie,
            ancho: area.ancho as f32,
            alto: area.alto as f32,
            e: monitor.escala(),
        })
    }
}

// ---------------------------------------------------------------------------
// Abrir

/// **Presenta** esa presentacion (o dice por que no), en su propio hilo.
/// Un `.pptx` y los suyos los maqueta PixPin (sin abrir PowerPoint); los
/// demas (`.ppt`, `.odp`) van por PowerPoint, como antes.
pub fn lanzar(idioma: pixpin_store::Idioma, ubicacion: pixpin_store::Ubicacion, ruta: &Path) {
    if pixpin_office::tipo_de(&pixpin_docs::nombre(ruta)) == Some(pixpin_office::Tipo::PowerPoint) {
        presentar_nativa(idioma, ubicacion, ruta, 0);
    } else {
        lanzar_con_powerpoint(idioma, ubicacion, ruta);
    }
}

/// **Presenta un PowerPoint maquetado por PixPin** (`pixpin-office`) desde
/// la diapositiva `desde`: lo que pide «Presentar» en el lector de Office.
/// Si no se deja leer, por PowerPoint como antes.
pub fn presentar_nativa(
    idioma: pixpin_store::Idioma,
    ubicacion: pixpin_store::Ubicacion,
    ruta: &Path,
    desde: usize,
) {
    let ruta = ruta.to_path_buf();
    let lanzado = std::thread::Builder::new().name("diapositivas".into()).spawn(move || {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let empezo = std::time::Instant::now();
        let mut documento = match pixpin_office::Documento::abrir(&ruta) {
            Ok(d) => Some(d),
            Err(err) => {
                tracing::warn!(%err, ruta = %ruta.display(), "presentacion que PixPin no lee; se pasa por PowerPoint");
                lanzar_con_powerpoint(idioma, ubicacion, &ruta);
                return;
            }
        };
        tracing::info!(ms = empezo.elapsed().as_millis() as u64, "presentacion leida (sin PowerPoint)");
        let textos = Catalogo::nuevo(idioma);
        let hecho = crate::dispositivo_perdido::con_recursos("diapositivas", |r| {
            let d = match documento.take() {
                Some(d) => d,
                None => pixpin_office::Documento::abrir(&ruta).map_err(|e| anyhow::anyhow!("{e}"))?,
            };
            presentar(r, &textos, ubicacion.raiz(), &ruta, Some((d, desde)))
        });
        if let Err(e) = hecho {
            tracing::warn!(?e, ruta = %ruta.display(), "no se pudo presentar");
        }
    });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de las diapositivas");
    }
}

/// **Presenta por PowerPoint**: el camino de antes (convertir a PDF con el
/// PowerPoint del equipo), para lo que PixPin no maqueta.
pub fn lanzar_con_powerpoint(
    idioma: pixpin_store::Idioma,
    ubicacion: pixpin_store::Ubicacion,
    ruta: &Path,
) {
    let ruta = ruta.to_path_buf();
    let lanzado = std::thread::Builder::new()
        .name("diapositivas".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            let nombre = pixpin_docs::nombre(&ruta);
            if let Some(clave) = por_que_no(&nombre, pixpin_shell::powerpoint::hay_powerpoint()) {
                ofrecer_otra_app(&textos, clave, &ruta);
                return;
            }
            let hecho = crate::dispositivo_perdido::con_recursos("diapositivas", |r| {
                presentar(r, &textos, ubicacion.raiz(), &ruta, None)
            });
            if let Err(e) = hecho {
                tracing::warn!(?e, ruta = %ruta.display(), "no se pudo presentar");
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de las diapositivas");
    }
}

/// Sin PowerPoint (o con un Keynote): se dice por que y se ofrece el
/// «Abrir con» de Windows, que es lo que queda.
fn ofrecer_otra_app(textos: &Catalogo, clave: &str, ruta: &Path) {
    let pregunta = format!(
        "{}\n\n{}",
        textos.t(clave),
        textos.t("diapositivas-abrir-con-otra")
    );
    let nula = windows::Win32::Foundation::HWND::default();
    if pixpin_shell::preguntar(nula, &textos.t("diapositivas-titulo"), &pregunta) {
        crate::abrir_con::abrir_con_o_avisar(nula, ruta, textos);
    }
}

enum Llega {
    Listo(Vec<(f32, f32)>),
    Hoja(usize, ImagenRgba),
    Fallo(String),
}

/// El hilo que convierte y luego dibuja las diapositivas que se le piden.
fn hilo(
    raiz: PathBuf,
    origen: PathBuf,
    pedidos: mpsc::Receiver<(usize, u32)>,
    salida: mpsc::Sender<Llega>,
    ventana: isize,
) {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let avisar = |l: Llega| {
        let _ = salida.send(l);
        pixpin_shell::overlay::despertar(ventana);
    };
    let pdf = match convertir(&raiz, &origen) {
        Ok(p) => p,
        Err(e) => return avisar(Llega::Fallo(e)),
    };
    let documento = match pixpin_pdf::Documento::abrir(&pdf) {
        Ok(d) => d,
        Err(e) => return avisar(Llega::Fallo(e.to_string())),
    };
    avisar(Llega::Listo(documento.medidas()));
    while let Ok(mut pedido) = pedidos.recv() {
        // Solo el ultimo: pasar cinco seguidas no dibuja las cinco.
        while let Ok(otro) = pedidos.try_recv() {
            pedido = otro;
        }
        let (i, ancho) = pedido;
        match documento.renderizar(i as u32, ancho.clamp(1, pixpin_pdf::ANCHO_MAXIMO)) {
            Ok(img) => avisar(Llega::Hoja(i, img)),
            Err(e) => tracing::warn!(?e, hoja = i, "diapositiva que no se pudo dibujar"),
        }
    }
}

/// La pastilla se va sola tras esto, como la del lector.
const MS_DE_LA_PASTILLA: u64 = 3000;
/// Lo que se queda el aviso de una tecla («Transiciones desactivadas»).
const MS_DEL_AVISO: u64 = 1600;

/// Las diapositivas maquetadas por PixPin: el documento, su pintor (del
/// motor de esta ventana), las ya maquetadas y lo que se lee de cada una
/// solo cuando hace falta (notas, transicion, videos).
struct Nativas {
    documento: pixpin_office::Documento,
    pintor: PintorOffice,
    recursos: std::sync::Arc<pixpin_office::Recursos>,
    hechas: HashMap<usize, std::sync::Arc<pixpin_office::Pagina>>,
    notas: HashMap<usize, Option<std::sync::Arc<str>>>,
    transiciones: HashMap<usize, Option<Transicion>>,
    medios: HashMap<usize, Vec<Medio>>,
}

impl Nativas {
    fn nuevas(documento: pixpin_office::Documento, pintor: PintorOffice) -> Nativas {
        let recursos = documento.recursos();
        Nativas {
            documento,
            pintor,
            recursos,
            hechas: HashMap::new(),
            notas: HashMap::new(),
            transiciones: HashMap::new(),
            medios: HashMap::new(),
        }
    }

    fn diapositiva(&mut self, i: usize) -> Option<std::sync::Arc<pixpin_office::Pagina>> {
        if let Some(p) = self.hechas.get(&i) {
            return Some(p.clone());
        }
        let p = self.documento.pagina(i, &self.pintor.letras)?;
        self.hechas.insert(i, p.clone());
        Some(p)
    }

    fn presentacion(&mut self) -> Option<&mut pixpin_office::pptx::Presentacion> {
        match &mut self.documento {
            pixpin_office::Documento::PowerPoint(p) => Some(p),
            _ => None,
        }
    }

    /// Las notas de la `i`, leidas la primera vez que se piden.
    fn notas(&mut self, i: usize) -> Option<std::sync::Arc<str>> {
        if let Some(n) = self.notas.get(&i) {
            return n.clone();
        }
        let n: Option<std::sync::Arc<str>> =
            self.presentacion().and_then(|p| p.notas(i)).map(Into::into);
        self.notas.insert(i, n.clone());
        n
    }

    fn transicion(&mut self, i: usize) -> Option<Transicion> {
        if let Some(t) = self.transiciones.get(&i) {
            return *t;
        }
        let t = self.presentacion().and_then(|p| p.transicion(i));
        self.transiciones.insert(i, t);
        t
    }

    fn medios(&mut self, i: usize) -> &[Medio] {
        if !self.medios.contains_key(&i) {
            let m = self.presentacion().map(|p| p.medios(i)).unwrap_or_default();
            self.medios.insert(i, m);
        }
        &self.medios[&i]
    }

    /// La diapositiva `i` pintada en vectores a un mapa de bits de `caja`
    /// (a la medida de la pantalla): lo que se copia en cada fotograma y lo
    /// que mezclan las transiciones, sin volver a maquetar.
    fn foto(
        &mut self,
        motor: &MotorRender,
        i: usize,
        caja: RectF,
        total: usize,
    ) -> Option<ID2D1Bitmap1> {
        let d = self.diapositiva(i)?;
        let b = motor
            .mapa_de_dibujo(caja.ancho as u32, caja.alto as u32)
            .ok()?;
        let escala = caja.ancho / d.ancho.max(1.0);
        let (pintor, recursos) = (&self.pintor, &self.recursos);
        motor
            .dibujar(&b, |p| {
                p.limpiar(Color::BLANCO);
                pintor.pintar(p, &d, recursos, escala, (0.0, 0.0), total as u32);
            })
            .ok()?;
        Some(b)
    }
}

impl Drop for Nativas {
    fn drop(&mut self) {
        self.pintor.olvidar_imagenes();
    }
}

/// Una transicion que se esta viendo: la diapositiva de antes (ya pintada)
/// y desde cuando.
struct EnCurso {
    vieja: ID2D1Bitmap1,
    efecto: Efecto,
    ms: u32,
    empezo: u64,
}

/// Si al pasar de `antes` a `ahora` toca transicion: solo hacia delante y
/// de una en una (atras o saltando, de golpe, como PowerPoint), con las
/// transiciones encendidas y si dura algo.
pub fn toca_transicion(
    antes: usize,
    ahora: usize,
    encendidas: bool,
    t: Option<Transicion>,
) -> bool {
    encendidas && ahora == antes + 1 && t.is_some_and(|t| t.ms > 0)
}

/// Un video o un audio de la diapositiva: se saca a un fichero temporal y
/// el audio suena en el reproductor flotante de la app; el video se abre
/// con el reproductor del sistema (PixPin no reproduce video).
fn reproducir_medio(textos: &Catalogo, origen: &Path, n: &mut Nativas, m: &Medio) {
    use std::hash::{Hash, Hasher};
    let Some(parte) = m.parte.as_deref() else {
        tracing::info!("video enlazado fuera de la presentacion: no se reproduce");
        return;
    };
    let nombre: String = parte
        .rsplit('/')
        .next()
        .unwrap_or("medio")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    origen.hash(&mut h);
    std::fs::metadata(origen)
        .ok()
        .and_then(|m| m.modified().ok())
        .hash(&mut h);
    parte.hash(&mut h);
    let carpeta = std::env::temp_dir().join("pixpin-medios");
    let ruta = carpeta.join(format!("{:016x}-{nombre}", h.finish()));
    if !ruta.is_file() {
        let Some(bytes) = n.presentacion().and_then(|p| p.bytes_de_parte(parte)) else {
            tracing::warn!(parte, "medio que no se pudo sacar de la presentacion");
            return;
        };
        let temporal = ruta.with_extension("tmp");
        let hecho = std::fs::create_dir_all(&carpeta)
            .and_then(|_| std::fs::write(&temporal, &bytes))
            .and_then(|_| std::fs::rename(&temporal, &ruta));
        if let Err(e) = hecho {
            tracing::warn!(?e, "no se pudo guardar el medio");
            return;
        }
    }
    if m.audio {
        crate::ventana_chat::reproducir_flotante(textos, ruta, nombre);
    } else if let Err(e) = pixpin_shell::abrir::abrir(&ruta) {
        tracing::warn!(?e, "no se pudo abrir el video");
    }
}

/// La presentacion a pantalla completa. Con `nativa` (el documento y desde
/// que diapositiva) las pinta PixPin en vectores; sin ella, las saca
/// PowerPoint a un PDF en su hilo.
fn presentar(
    recursos: &Recursos,
    textos: &Catalogo,
    raiz: &Path,
    ruta: &Path,
    nativa: Option<(pixpin_office::Documento, usize)>,
) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let (monitor, del_orador) = repartir(monitores.monitores()).context("sin monitores")?;
    // Notas solo hay en lo que maqueta PixPin: por PowerPoint, una pantalla.
    let del_orador = del_orador.filter(|_| nativa.is_some());
    let area = monitor.area;
    let e = monitor.escala_por_cien as f32 / 100.0;
    let (ancho, alto) = (area.ancho as f32, area.alto as f32);
    let nombre = pixpin_docs::sin_extension(&pixpin_docs::nombre(ruta));
    let ventana =
        VentanaOverlay::nueva_normal(area, &nombre).context("sin ventana para presentar")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        area.ancho,
        area.alto,
    )
    .context("sin superficie para presentar")?;
    ventana.mostrar();
    // Con dos pantallas, el orador en la del portatil (y el teclado alli).
    let mut orador = del_orador.and_then(|m| match Orador::nuevo(recursos, &motor, &m, &nombre) {
        Ok(o) => Some(o),
        Err(err) => {
            tracing::warn!(?err, "sin vista del orador; se presenta en una pantalla");
            None
        }
    });
    match &orador {
        Some(o) => o.ventana.enfocar(),
        None => ventana.enfocar(),
    }

    let (tx, rx_pedidos) = mpsc::channel::<(usize, u32)>();
    let (tx_llega, rx) = mpsc::channel::<Llega>();
    let hwnd = ventana.handle().0 as isize;
    let (raiz_h, ruta_h) = (raiz.to_path_buf(), ruta.to_path_buf());
    let mut pase = Pase {
        pastilla: true,
        ..Default::default()
    };
    let mut medidas: Vec<(f32, f32)> = Vec::new();
    // Sin PowerPoint: el documento ya leido, su pintor y cuantas hay.
    let mut nativas = match nativa {
        Some((documento, desde)) => {
            let pintor =
                PintorOffice::nuevo(&motor).context("sin letras para pintar las diapositivas")?;
            let mut n = Nativas::nuevas(documento, pintor);
            pase.total = n.documento.total();
            let tam = n
                .diapositiva(desde.min(pase.total.saturating_sub(1)))
                .map(|p| (p.ancho, p.alto))
                .unwrap_or((960.0, 540.0));
            medidas = vec![tam; pase.total];
            pase.actual = desde.min(pase.total.saturating_sub(1));
            drop((rx_pedidos, tx_llega));
            Some(n)
        }
        None => {
            std::thread::Builder::new()
                .name("diapositivas-hojas".into())
                .spawn(move || hilo(raiz_h, ruta_h, rx_pedidos, tx_llega, hwnd))
                .context("sin hilo para las diapositivas")?;
            None
        }
    };
    // Las diapositivas ya pintadas a la medida de la pantalla (las de
    // PowerPoint llegan del hilo; las de PixPin se pintan aqui).
    let mut pintadas: HashMap<usize, ID2D1Bitmap1> = HashMap::new();
    let mut pedida: Option<usize> = None;
    let mut fallo: Option<String> = None;
    let mut pastilla_hasta = ahora_ms() + MS_DE_LA_PASTILLA;
    let mut botones: Vec<(RectF, Mando)> = Vec::new();
    let mut raton = (0.0f32, 0.0f32);
    let mut hay_que_pintar = true;
    let mut pintar_orador = true;
    let mut vivo = true;
    // Lo de las transiciones, las notas y el reloj.
    let mut en_curso: Option<EnCurso> = None;
    let mut transiciones = true;
    let mut notas_abajo = false;
    let mut aviso: Option<(String, u64)> = None;
    let empezo = ahora_ms();
    let mut vista = pase.actual;
    let mut visible_desde = empezo;
    let mut segundo_pintado = u64::MAX;
    let orador_hwnd = orador.as_ref().map(|o| o.ventana.handle());
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        while let Ok(l) = rx.try_recv() {
            hay_que_pintar = true;
            match l {
                Llega::Listo(m) => {
                    pase.total = m.len();
                    medidas = m;
                }
                Llega::Hoja(i, img) => {
                    match motor.bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles) {
                        Ok(b) => {
                            pintadas.insert(i, b);
                        }
                        Err(err) => {
                            tracing::warn!(?err, hoja = i, "no se pudo subir la diapositiva")
                        }
                    }
                }
                Llega::Fallo(clave) => fallo = Some(clave),
            }
        }
        for (h, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            let del_orador = Some(h) == orador_hwnd;
            if h != ventana.handle() && !del_orador {
                continue;
            }
            hay_que_pintar = true;
            let mando = match evento {
                EventoOverlay::Cerrar => Some(Mando::Salir),
                EventoOverlay::Tecla { vk, shift, .. } => mando_de_tecla(vk, shift),
                EventoOverlay::Rueda(d) if d < 0 => Some(Mando::Siguiente),
                EventoOverlay::Rueda(d) if d > 0 => Some(Mando::Anterior),
                EventoOverlay::BotonDerechoPulsado(_) if !del_orador => Some(Mando::Anterior),
                EventoOverlay::RatonMovido(p) if !del_orador => {
                    let nuevo = ((p.x - area.x) as f32, (p.y - area.y) as f32);
                    // Mover el raton ensena la pastilla un rato, como en
                    // cualquier reproductor.
                    if (nuevo.0 - raton.0).abs() + (nuevo.1 - raton.1).abs() > 4.0 {
                        pastilla_hasta = ahora_ms() + MS_DE_LA_PASTILLA;
                    }
                    raton = nuevo;
                    None
                }
                EventoOverlay::BotonPulsado(p) if !del_orador => {
                    raton = ((p.x - area.x) as f32, (p.y - area.y) as f32);
                    let sobre_boton = botones
                        .iter()
                        .find(|(r, _)| dentro(r, raton.0, raton.1))
                        .map(|(_, m)| *m);
                    // Un clic en un video o un audio lo reproduce.
                    let medio = match (&mut nativas, sobre_boton, medidas.get(pase.actual)) {
                        (Some(n), None, Some(m)) if !pase.negro => {
                            let caja = encajar_entera(*m, (ancho, alto));
                            let k = caja.ancho / m.0.max(1.0);
                            let (px, py) = ((raton.0 - caja.x) / k, (raton.1 - caja.y) / k);
                            n.medios(pase.actual)
                                .iter()
                                .find(|md| md.rect.contiene(px, py))
                                .cloned()
                        }
                        _ => None,
                    };
                    if let (Some(md), Some(n)) = (medio, nativas.as_mut()) {
                        reproducir_medio(textos, ruta, n, &md);
                        None
                    } else {
                        Some(sobre_boton.unwrap_or_else(|| mando_de_clic(raton.0, ancho)))
                    }
                }
                _ => None,
            };
            if let Some(m) = mando {
                match m {
                    Mando::Pastilla => {
                        // En el medio: esconder o ensenar ya, sin esperar.
                        let vista = pase.pastilla && pastilla_hasta > ahora_ms();
                        pase.pastilla = !vista;
                        pastilla_hasta = if pase.pastilla { u64::MAX } else { 0 };
                        continue;
                    }
                    Mando::Notas => {
                        // Con la vista del orador las notas ya estan alli.
                        if orador.is_none() {
                            notas_abajo = !notas_abajo;
                        }
                        continue;
                    }
                    Mando::Transiciones => {
                        transiciones = !transiciones;
                        let clave = if transiciones {
                            "diapositivas-transiciones-si"
                        } else {
                            "diapositivas-transiciones-no"
                        };
                        aviso = Some((textos.t(clave), ahora_ms() + MS_DEL_AVISO));
                        pintar_orador = true;
                        continue;
                    }
                    _ => {}
                }
                if !pase.aplicar(m) {
                    vivo = false;
                }
            }
        }
        if !vivo {
            break;
        }
        let mut ahora = ahora_ms();
        // Paso sola si la diapositiva lo pide (`advTm`), contando desde que
        // acabo de entrar.
        let mut tope: u64 = 100;
        if en_curso.is_none()
            && !pase.negro
            && pase.puede_siguiente()
            && pase.total > 0
            && let Some(n) = nativas.as_mut()
            && let Some(avance) = n.transicion(pase.actual).and_then(|t| t.avance_ms)
        {
            let cuando = visible_desde + avance as u64;
            if ahora >= cuando {
                pase.aplicar(Mando::Siguiente);
                hay_que_pintar = true;
            } else {
                tope = tope.min(cuando - ahora);
            }
        }
        // Cambio de diapositiva: ¿con transicion?
        if pase.actual != vista {
            let antes = vista;
            vista = pase.actual;
            visible_desde = ahora;
            en_curso = None;
            pintar_orador = true;
            if let Some(n) = nativas.as_mut() {
                let t = n.transicion(pase.actual);
                if !pase.negro
                    && toca_transicion(antes, pase.actual, transiciones, t)
                    && let (Some(t), Some(vieja)) = (t, pintadas.get(&antes).cloned())
                {
                    en_curso = Some(EnCurso {
                        vieja,
                        efecto: t.efecto,
                        ms: t.ms,
                        empezo: ahora,
                    });
                    visible_desde = ahora + t.ms as u64;
                }
            }
        }
        // La de ahora pintada antes del fotograma (fuera de el).
        if let Some(n) = nativas.as_mut()
            && pase.total > 0
            && !pintadas.contains_key(&pase.actual)
            && let Some(m) = medidas.get(pase.actual)
            && let Some(b) = n.foto(
                &motor,
                pase.actual,
                encajar_entera(*m, (ancho, alto)),
                pase.total,
            )
        {
            pintadas.insert(pase.actual, b);
            hay_que_pintar = true;
            ahora = ahora_ms();
        }
        if pastilla_hasta != u64::MAX && pastilla_hasta >= ahora && pastilla_hasta < ahora + 200 {
            hay_que_pintar = true;
        }
        if let Some((_, hasta)) = &aviso {
            if *hasta <= ahora {
                aviso = None;
                hay_que_pintar = true;
            } else {
                tope = tope.min(hasta - ahora);
            }
        }
        // Se pide la que se mira; al llegar, la de al lado, para que pasar
        // sea instantaneo. Las lejanas se sueltan: cien diapositivas a
        // pantalla completa serian cientos de megas de video.
        if pase.total > 0 && nativas.is_none() {
            let quiero = if !pintadas.contains_key(&pase.actual) {
                Some(pase.actual)
            } else if pase.actual + 1 < pase.total && !pintadas.contains_key(&(pase.actual + 1)) {
                Some(pase.actual + 1)
            } else if pase.actual > 0 && !pintadas.contains_key(&(pase.actual - 1)) {
                Some(pase.actual - 1)
            } else {
                None
            };
            if let Some(i) = quiero
                && pedida != Some(i)
                && let Some(m) = medidas.get(i)
            {
                let caja = encajar(*m, (ancho, alto));
                let _ = tx.send((i, caja.ancho.round() as u32));
                pedida = Some(i);
            }
            pintadas.retain(|i, _| i.abs_diff(pase.actual) <= 2);
        } else if pase.total > 0 {
            // Las de PixPin: la de antes y la de despues, nada mas.
            pintadas.retain(|i, _| i.abs_diff(pase.actual) <= 1);
        }
        // El fotograma de una transicion: la mezcla de los dos mapas de
        // bits, al ritmo de la pantalla.
        let capas_ahora = en_curso.as_ref().map(|c| {
            let t = pp::progreso(ahora.saturating_sub(c.empezo), c.ms);
            (t, pp::capas(c.efecto, t))
        });
        if capas_ahora.is_some() {
            hay_que_pintar = true;
        }
        if hay_que_pintar {
            hay_que_pintar = false;
            let mostrar_pastilla = pase.pastilla && pastilla_hasta > ahora;
            let notas = if notas_abajo && orador.is_none() {
                Some(nativas.as_mut().and_then(|n| n.notas(pase.actual)))
            } else {
                None
            };
            let mezcla = match (&en_curso, &capas_ahora) {
                (Some(c), Some((_, capas))) => Some((&c.vieja, capas.as_slice())),
                _ => None,
            };
            if let Ok(destino) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&destino, |p: &Pintor| {
                    botones = pintar(
                        p,
                        &pase,
                        &medidas,
                        &pintadas,
                        mezcla,
                        fallo.as_deref(),
                        mostrar_pastilla,
                        notas.as_ref().map(|n| n.as_deref()),
                        aviso.as_ref().map(|(t, _)| t.as_str()),
                        (ancho, alto),
                        e,
                        textos,
                    );
                });
                let _ = if mezcla.is_some() {
                    superficie.presentar_sincronizado(None)
                } else {
                    superficie.presentar()
                };
            }
            if let Some((t, _)) = capas_ahora
                && t >= 1.0
            {
                // Acabada: la vieja se suelta y el siguiente fotograma es
                // la nueva sola.
                en_curso = None;
                hay_que_pintar = true;
            }
        }
        // La vista del orador: al cambiar de diapositiva y cada segundo (el
        // reloj); nunca en cada fotograma de una transicion.
        if let Some(o) = orador.as_mut() {
            let segundo = (ahora - empezo) / 1000;
            if pintar_orador || segundo != segundo_pintado {
                pintar_orador = false;
                segundo_pintado = segundo;
                let notas = nativas.as_mut().and_then(|n| n.notas(pase.actual));
                if let Ok(destino) = o.superficie.empezar(&motor) {
                    let _ = motor.dibujar(&destino, |p: &Pintor| {
                        pintar_vista_orador(
                            p,
                            (o.ancho, o.alto),
                            o.e,
                            &pase,
                            medidas.get(pase.actual).copied().unwrap_or((960.0, 540.0)),
                            &pintadas,
                            notas.as_deref(),
                            ahora - empezo,
                            &hora(pixpin_shell::entorno::ahora_local_ms()),
                            transiciones,
                            textos,
                        );
                    });
                    let _ = o.superficie.presentar();
                }
            }
            tope = tope.min(1000 - (ahora - empezo) % 1000);
        }
        if en_curso.is_some() {
            // Sin esperar: el ritmo lo marca la sincronia con la pantalla.
            pixpin_shell::overlay::esperar_eventos(Some(0));
            continue;
        }
        // Quieto: la siguiente se pinta ya, para que pasar (y su
        // transicion) sea instantaneo y el orador la vea.
        if let Some(n) = nativas.as_mut()
            && pase.actual + 1 < pase.total
            && !pintadas.contains_key(&(pase.actual + 1))
            && let Some(m) = medidas.get(pase.actual + 1)
        {
            if let Some(b) = n.foto(
                &motor,
                pase.actual + 1,
                encajar_entera(*m, (ancho, alto)),
                pase.total,
            ) {
                pintadas.insert(pase.actual + 1, b);
                pintar_orador = true;
            }
            continue;
        }
        pixpin_shell::overlay::esperar_eventos(Some(tope.clamp(1, 100) as u32));
    }
    drop(tx);
    Ok(())
}

/// La diapositiva en su caja: sola, o mezclada con la de antes durante una
/// transicion (las dos ya pintadas: aqui solo se copian).
fn pintar_diapositiva(
    p: &Pintor,
    caja: RectF,
    nueva: Option<&ID2D1Bitmap1>,
    mezcla: Option<(&ID2D1Bitmap1, &[Capa])>,
) {
    let Some((vieja, capas)) = mezcla else {
        if let Some(b) = nueva {
            p.bitmap(b, caja, None, false);
        }
        return;
    };
    p.con_recorte(caja, |p| {
        for c in capas {
            let destino = RectF {
                x: caja.x + c.destino[0] * caja.ancho,
                y: caja.y + c.destino[1] * caja.alto,
                ancho: (c.destino[2] - c.destino[0]) * caja.ancho,
                alto: (c.destino[3] - c.destino[1]) * caja.alto,
            };
            let fuente = match c.fuente {
                pp::Fuente::Vieja => Some(vieja),
                pp::Fuente::Nueva => nueva,
                pp::Fuente::Negro => None,
            };
            match fuente {
                // Los dos mapas de bits miden lo que la caja.
                Some(b) => {
                    let origen = RectF {
                        x: c.origen[0] * caja.ancho,
                        y: c.origen[1] * caja.alto,
                        ancho: (c.origen[2] - c.origen[0]) * caja.ancho,
                        alto: (c.origen[3] - c.origen[1]) * caja.alto,
                    };
                    p.bitmap_translucido(b, destino, Some(origen), Interpolacion::Lineal, c.alfa);
                }
                None => p.rellenar(destino, con_alfa(Color::NEGRO, c.alfa)),
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn pintar(
    p: &Pintor,
    pase: &Pase,
    medidas: &[(f32, f32)],
    pintadas: &HashMap<usize, ID2D1Bitmap1>,
    mezcla: Option<(&ID2D1Bitmap1, &[Capa])>,
    fallo: Option<&str>,
    pastilla: bool,
    notas: Option<Option<&str>>,
    aviso: Option<&str>,
    (ancho, alto): (f32, f32),
    e: f32,
    textos: &Catalogo,
) -> Vec<(RectF, Mando)> {
    let mut botones = Vec::new();
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho,
            alto,
        },
        Color::NEGRO,
    );
    let centrado = |texto: &str, color: Color| {
        let tam = 18.0 * e;
        let (w, _) = p.medir_texto(texto, tam);
        p.texto(texto, (ancho - w) / 2.0, alto / 2.0, tam, color);
    };
    if let Some(clave) = fallo {
        // Una clave de texto (sin PowerPoint...) o el error de PowerPoint.
        let dicho = if clave.starts_with("diapositivas-") {
            textos.t(clave)
        } else {
            format!("{} — {clave}", textos.t("diapositivas-no-se-pudo"))
        };
        centrado(&dicho, TEXTO);
        return botones;
    }
    if pase.total == 0 {
        centrado(&textos.t("diapositivas-preparando"), APAGADO);
        return botones;
    }
    if pase.negro {
        return botones;
    }
    if let Some(m) = medidas.get(pase.actual) {
        pintar_diapositiva(
            p,
            encajar_entera(*m, (ancho, alto)),
            pintadas.get(&pase.actual),
            mezcla,
        );
    }
    // Las notas abajo (una pantalla, tecla N).
    let mut suelo = alto;
    if let Some(notas) = notas {
        let tam = 20.0 * e;
        let margen = 24.0 * e;
        let texto = notas
            .map(str::to_string)
            .unwrap_or_else(|| textos.t("diapositivas-sin-notas"));
        let (_, h) = p.medir_texto_ajustado(&texto, tam, ancho - 2.0 * margen);
        let franja_alto = (h + 2.0 * margen).min(alto * 0.33);
        let franja = RectF {
            x: 0.0,
            y: alto - franja_alto,
            ancho,
            alto: franja_alto,
        };
        p.rellenar(franja, con_alfa(CRISTAL, 0.92));
        let color = if notas.is_some() {
            Color::BLANCO
        } else {
            APAGADO
        };
        p.con_recorte(franja, |p| {
            p.texto_ajustado(
                &texto,
                margen,
                franja.y + margen,
                tam,
                ancho - 2.0 * margen,
                color,
            )
        });
        suelo = franja.y;
    }
    if let Some(t) = aviso {
        let tam = 16.0 * e;
        let (w, h) = p.medir_texto(t, tam);
        let caja = RectF {
            x: (ancho - w) / 2.0 - 16.0 * e,
            y: 32.0 * e,
            ancho: w + 32.0 * e,
            alto: h + 16.0 * e,
        };
        p.rellenar_redondeado(caja, caja.alto / 2.0, con_alfa(CRISTAL, 0.9));
        p.texto(t, caja.x + 16.0 * e, caja.y + 8.0 * e, tam, Color::BLANCO);
    }
    if !pastilla {
        return botones;
    }
    // La pastilla del movil: anterior, «n / total», siguiente y salir.
    let lado = 44.0 * e;
    let tam = 15.0 * e;
    let cuenta = format!("{} / {}", pase.actual + 1, pase.total);
    let (w, h) = p.medir_texto(&cuenta, tam);
    let total = lado * 3.0 + w + 30.0 * e;
    let caja = RectF {
        x: (ancho - total) / 2.0,
        y: suelo - lado - 28.0 * e,
        ancho: total,
        alto: lado,
    };
    p.rellenar_redondeado(caja, lado / 2.0, con_alfa(CRISTAL, 0.86));
    let mut x = caja.x + 6.0 * e;
    let mut boton = |icono: &pixpin_render::icono::Icono, activo: bool, mando: Mando, x: f32| {
        let zona = RectF {
            x,
            y: caja.y,
            ancho: lado,
            alto: lado,
        };
        let l = 22.0 * e;
        let color = if activo {
            Color::BLANCO
        } else {
            con_alfa(Color::BLANCO, 0.35)
        };
        p.icono(
            icono,
            RectF {
                x: x + (lado - l) / 2.0,
                y: caja.y + (lado - l) / 2.0,
                ancho: l,
                alto: l,
            },
            color,
        );
        botones.push((zona, mando));
    };
    boton(
        &material::ARROW_BACK,
        pase.puede_anterior(),
        Mando::Anterior,
        x,
    );
    x += lado;
    p.texto(
        &cuenta,
        x + 4.0 * e,
        caja.y + (lado - h) / 2.0,
        tam,
        Color::BLANCO,
    );
    x += w + 8.0 * e;
    boton(
        &material::ARROW_FORWARD,
        pase.puede_siguiente(),
        Mando::Siguiente,
        x,
    );
    x += lado + 10.0 * e;
    boton(&material::CLOSE, true, Mando::Salir, x);
    botones
}

/// La vista del orador: la de ahora, la siguiente, las notas, el tiempo que
/// lleva y la hora. Sencilla y sin botones: se maneja con el teclado (o el
/// mando) como la presentacion.
#[allow(clippy::too_many_arguments)]
fn pintar_vista_orador(
    p: &Pintor,
    (ancho, alto): (f32, f32),
    e: f32,
    pase: &Pase,
    pagina: (f32, f32),
    pintadas: &HashMap<usize, ID2D1Bitmap1>,
    notas: Option<&str>,
    transcurrido_ms: u64,
    reloj: &str,
    transiciones: bool,
    textos: &Catalogo,
) {
    const FONDO: Color = Color {
        r: 0.09,
        g: 0.09,
        b: 0.106,
        a: 1.0,
    };
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho,
            alto,
        },
        FONDO,
    );
    let z = zonas_orador(ancho, alto, e, pagina);
    let m = 24.0 * e;
    // Arriba: «3 / 12», el tiempo (grande, en medio) y la hora.
    let tam = 18.0 * e;
    let cuenta = format!("{} / {}", pase.actual + 1, pase.total.max(1));
    p.texto(&cuenta, m, z.cabecera + 8.0 * e, tam, APAGADO);
    let tiempo = cronometro(transcurrido_ms);
    let grande = 30.0 * e;
    let (w, _) = p.medir_texto(&tiempo, grande);
    p.texto(
        &tiempo,
        (ancho - w) / 2.0,
        z.cabecera,
        grande,
        Color::BLANCO,
    );
    let (w, _) = p.medir_texto(reloj, tam);
    p.texto(reloj, ancho - m - w, z.cabecera + 8.0 * e, tam, APAGADO);
    if !transiciones {
        let t = textos.t("diapositivas-transiciones-no");
        let chico = 13.0 * e;
        let (w, _) = p.medir_texto(&t, chico);
        p.texto(&t, ancho - m - w, z.cabecera + 34.0 * e, chico, APAGADO);
    }
    // La de ahora.
    if pase.negro {
        p.rellenar(z.actual, Color::NEGRO);
    } else if let Some(b) = pintadas.get(&pase.actual) {
        p.bitmap(b, z.actual, None, false);
    } else {
        p.rellenar(z.actual, CRISTAL);
    }
    // La siguiente (la misma imagen que ya esta pintada, en pequeno).
    let rotulo = 15.0 * e;
    p.texto(
        &textos.t("diapositivas-siguiente"),
        z.siguiente.x,
        z.siguiente.y - z.rotulo + 4.0 * e,
        rotulo,
        APAGADO,
    );
    if pase.actual + 1 < pase.total {
        match pintadas.get(&(pase.actual + 1)) {
            Some(b) => p.bitmap(b, z.siguiente, None, false),
            None => p.rellenar(z.siguiente, CRISTAL),
        }
    } else {
        p.rellenar(z.siguiente, CRISTAL);
        let t = textos.t("diapositivas-fin");
        let (w, h) = p.medir_texto(&t, rotulo);
        p.texto(
            &t,
            z.siguiente.x + (z.siguiente.ancho - w) / 2.0,
            z.siguiente.y + (z.siguiente.alto - h) / 2.0,
            rotulo,
            APAGADO,
        );
    }
    // Las notas.
    p.texto(
        &textos.t("diapositivas-notas"),
        z.notas.x,
        z.notas.y - z.rotulo + 4.0 * e,
        rotulo,
        APAGADO,
    );
    if z.notas.alto > 0.0 {
        let (texto, color) = match notas {
            Some(n) => (n.to_string(), TEXTO),
            None => (textos.t("diapositivas-sin-notas"), APAGADO),
        };
        p.con_recorte(z.notas, |p| {
            p.texto_ajustado(&texto, z.notas.x, z.notas.y, 22.0 * e, z.notas.ancho, color)
        });
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_presentaciones_se_reconocen_por_su_extension() {
        for si in [
            "charla.pptx",
            "CHARLA.PPTX",
            "vieja.ppt",
            "pase.ppsx",
            "libre.odp",
            "mac.key",
        ] {
            assert!(es_presentacion(si), "{si}");
        }
        for no in ["informe.pdf", "carta.docx", "hoja.xlsx", "pptx", "sin"] {
            assert!(!es_presentacion(no), "{no}");
        }
    }

    #[test]
    fn con_powerpoint_se_abre_todo_menos_keynote_y_sin_el_se_dice() {
        assert_eq!(por_que_no("a.pptx", true), None);
        // El movil no lee `.ppt` ni `.odp`; PowerPoint si.
        assert_eq!(por_que_no("a.ppt", true), None);
        assert_eq!(por_que_no("a.odp", true), None);
        assert_eq!(por_que_no("a.key", true), Some("diapositivas-keynote"));
        assert_eq!(
            por_que_no("a.pptx", false),
            Some("diapositivas-sin-powerpoint")
        );
        assert_eq!(por_que_no("a.pdf", true), Some("diapositivas-no-es"));
    }

    #[test]
    fn las_teclas_de_powerpoint_y_de_los_mandos_pasan_diapositivas() {
        for vk in [VK_RIGHT, VK_DOWN, VK_NEXT, VK_ESPACIO, VK_INTRO] {
            assert_eq!(mando_de_tecla(vk, false), Some(Mando::Siguiente), "{vk:#x}");
        }
        for vk in [VK_LEFT, VK_UP, VK_PRIOR, VK_RETROCESO, VK_P] {
            assert_eq!(mando_de_tecla(vk, false), Some(Mando::Anterior), "{vk:#x}");
        }
        assert_eq!(mando_de_tecla(VK_ESPACIO, true), Some(Mando::Anterior));
        assert_eq!(mando_de_tecla(VK_HOME, false), Some(Mando::Primera));
        assert_eq!(mando_de_tecla(VK_END, false), Some(Mando::Ultima));
        assert_eq!(mando_de_tecla(VK_ESCAPE, false), Some(Mando::Salir));
        assert_eq!(mando_de_tecla(VK_B, false), Some(Mando::Negro));
        // La N ensena las notas y la T apaga las transiciones.
        assert_eq!(mando_de_tecla(VK_N, false), Some(Mando::Notas));
        assert_eq!(mando_de_tecla(VK_T, false), Some(Mando::Transiciones));
        // Una letra cualquiera no hace nada: no hay atajos sorpresa.
        assert_eq!(mando_de_tecla(0x51, false), None);
    }

    #[test]
    fn un_clic_pasa_por_tercios_como_el_dedo_en_el_movil() {
        assert_eq!(mando_de_clic(10.0, 900.0), Mando::Anterior);
        assert_eq!(mando_de_clic(450.0, 900.0), Mando::Pastilla);
        assert_eq!(mando_de_clic(890.0, 900.0), Mando::Siguiente);
    }

    #[test]
    fn el_pase_no_da_la_vuelta_ni_se_sale_de_la_presentacion() {
        let mut p = Pase {
            total: 3,
            ..Default::default()
        };
        assert!(!p.puede_anterior());
        assert!(p.aplicar(Mando::Anterior));
        assert_eq!(p.actual, 0, "antes de la primera no hay nada");
        p.aplicar(Mando::Siguiente);
        p.aplicar(Mando::Siguiente);
        p.aplicar(Mando::Siguiente);
        assert_eq!(p.actual, 2, "tras la ultima tampoco");
        assert!(!p.puede_siguiente());
        p.aplicar(Mando::Primera);
        assert_eq!(p.actual, 0);
        p.aplicar(Mando::Ultima);
        assert_eq!(p.actual, 2);
        assert!(!p.aplicar(Mando::Salir), "Esc sale");
    }

    #[test]
    fn en_negro_el_primer_mando_solo_enciende_la_pantalla() {
        let mut p = Pase {
            total: 5,
            actual: 1,
            ..Default::default()
        };
        p.aplicar(Mando::Negro);
        assert!(p.negro);
        p.aplicar(Mando::Siguiente);
        assert!(!p.negro);
        assert_eq!(p.actual, 2, "como PowerPoint: sale del negro y pasa");
        p.aplicar(Mando::Negro);
        p.aplicar(Mando::Negro);
        assert!(!p.negro, "la B otra vez vuelve");
        assert_eq!(p.actual, 2);
    }

    #[test]
    fn sin_saber_cuantas_hay_no_se_pasa_de_la_primera() {
        let mut p = Pase::default();
        p.aplicar(Mando::Siguiente);
        assert_eq!(p.actual, 0);
    }

    #[test]
    fn una_diapositiva_apaisada_se_encaja_centrada_con_franjas() {
        // 16:9 en una pantalla 4:3: franjas arriba y abajo.
        let c = encajar((960.0, 540.0), (1024.0, 768.0));
        assert!((c.ancho - 1024.0).abs() < 0.01);
        assert!((c.alto - 576.0).abs() < 0.01);
        assert!((c.y - 96.0).abs() < 0.01);
        assert_eq!(c.x, 0.0);
        // Una medida rota no divide entre cero.
        let c = encajar((0.0, 0.0), (800.0, 600.0));
        assert!(c.ancho.is_finite() && c.alto.is_finite());
    }

    #[test]
    fn la_cache_cambia_si_cambia_el_original_y_un_pdf_se_une_tal_cual() {
        let dir = std::env::temp_dir().join(format!("pixpin-diapos-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pptx = dir.join("charla.pptx");
        std::fs::write(&pptx, b"uno").unwrap();
        let a = pdf_en_cache(&dir, &pptx).unwrap();
        assert_eq!(
            a,
            pdf_en_cache(&dir, &pptx).unwrap(),
            "la misma clave dos veces"
        );
        assert!(
            a.starts_with(dir.join("cache")),
            "fuera del proyecto: {a:?}"
        );
        std::fs::write(&pptx, b"otro mas largo").unwrap();
        assert_ne!(
            a,
            pdf_en_cache(&dir, &pptx).unwrap(),
            "cambiado, otra conversion"
        );
        // Sin fichero no hay clave.
        assert!(pdf_en_cache(&dir, &dir.join("no.pptx")).is_none());
        // Un PDF no pasa por PowerPoint.
        let pdf = dir.join("plano.pdf");
        assert_eq!(pdf_para_unir(&dir, &pdf).unwrap(), pdf);
        // Un Keynote no se convierte: se dice por que.
        let key = dir.join("mac.key");
        std::fs::write(&key, b"x").unwrap();
        assert_eq!(
            pdf_para_unir(&dir, &key).unwrap_err(),
            "diapositivas-keynote"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn monitor(id: u32, x: i32, principal: bool) -> pixpin_geom::Monitor {
        let area = pixpin_geom::Rect {
            x,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        pixpin_geom::Monitor {
            id,
            area,
            area_trabajo: area,
            escala_por_cien: 100,
            principal,
        }
    }

    #[test]
    fn con_dos_pantallas_el_orador_se_queda_la_principal() {
        let (p, o) = repartir(&[monitor(1, 0, true)]).unwrap();
        assert_eq!((p.id, o), (1, None), "una sola: sin vista del orador");
        let (p, o) = repartir(&[monitor(1, 0, true), monitor(2, 1920, false)]).unwrap();
        assert_eq!((p.id, o.map(|m| m.id)), (2, Some(1)));
        // Da igual el orden en que lleguen.
        let (p, o) = repartir(&[monitor(2, -1920, false), monitor(1, 0, true)]).unwrap();
        assert_eq!((p.id, o.map(|m| m.id)), (2, Some(1)));
        assert!(repartir(&[]).is_none());
    }

    #[test]
    fn el_cronometro_y_el_reloj() {
        assert_eq!(cronometro(0), "00:00");
        assert_eq!(cronometro(65_999), "01:05");
        assert_eq!(cronometro(3_723_000), "1:02:03");
        assert_eq!(hora((14 * 60 + 5) * 60_000 + 59_000), "14:05");
        assert_eq!(hora(3 * 86_400_000 + 60_000), "00:01", "de otro dia");
        assert_eq!(hora(-60_000), "23:59", "antes de 1970 no se rompe");
    }

    #[test]
    fn la_vista_del_orador_cabe_y_no_se_pisa() {
        let dentro_de = |r: RectF, w: f32, h: f32| {
            r.x >= 0.0 && r.y >= 0.0 && r.x + r.ancho <= w + 0.01 && r.y + r.alto <= h + 0.01
        };
        let se_pisan = |a: RectF, b: RectF| {
            a.x < b.x + b.ancho && b.x < a.x + a.ancho && a.y < b.y + b.alto && b.y < a.y + a.alto
        };
        for (w, h, e, pagina) in [
            (1366.0, 768.0, 1.0, (960.0, 540.0)),
            (1920.0, 1080.0, 1.5, (720.0, 540.0)),
            (2560.0, 1600.0, 2.0, (960.0, 540.0)),
        ] {
            let z = zonas_orador(w, h, e, pagina);
            for r in [z.actual, z.siguiente, z.notas] {
                assert!(dentro_de(r, w, h), "{w}x{h}: {r:?}");
            }
            assert!(
                !se_pisan(z.actual, z.siguiente)
                    && !se_pisan(z.actual, z.notas)
                    && !se_pisan(z.siguiente, z.notas),
                "{z:?}"
            );
            assert!(
                z.actual.ancho > z.siguiente.ancho,
                "la de ahora es la grande"
            );
            assert!(
                ((z.actual.ancho / z.actual.alto) - pagina.0 / pagina.1).abs() < 0.01,
                "sin deformar"
            );
            assert!(z.notas.alto > h * 0.2, "sitio para las notas: {z:?}");
        }
        // Una pantalla diminuta no da medidas negativas.
        let z = zonas_orador(50.0, 40.0, 2.0, (960.0, 540.0));
        assert!(z.notas.alto >= 0.0 && z.actual.ancho.is_finite());
    }

    #[test]
    fn la_transicion_solo_al_avanzar_de_una_en_una() {
        let t = Some(Transicion {
            efecto: Efecto::Fundido { por_negro: false },
            ms: 700,
            avance_ms: None,
            avance_clic: true,
        });
        assert!(toca_transicion(2, 3, true, t));
        assert!(!toca_transicion(3, 2, true, t), "hacia atras, de golpe");
        assert!(!toca_transicion(0, 5, true, t), "saltando (Fin), de golpe");
        assert!(!toca_transicion(2, 3, false, t), "apagadas con la T");
        assert!(!toca_transicion(2, 3, true, None), "sin transicion");
        let corte = Some(Transicion {
            efecto: Efecto::Corte { por_negro: false },
            ms: 0,
            avance_ms: None,
            avance_clic: true,
        });
        assert!(!toca_transicion(2, 3, true, corte), "un corte no anima");
    }

    #[test]
    fn las_notas_y_las_transiciones_no_mueven_el_pase_ni_encienden_el_negro() {
        let mut p = Pase {
            total: 4,
            actual: 1,
            ..Default::default()
        };
        p.aplicar(Mando::Negro);
        assert!(p.aplicar(Mando::Notas));
        assert!(p.aplicar(Mando::Transiciones));
        assert!(p.negro, "sigue en negro");
        assert_eq!(p.actual, 1);
    }

    /// **La transicion a mitad y la vista del orador**, pintadas fuera de
    /// pantalla con una presentacion publica de muestra, en PNG para
    /// mirarlas. Necesita GPU:
    /// `cargo test -p pixpin --bin pixpinmax muestra_de_presentar -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU y una presentacion de muestra; genera PNG"]
    fn muestra_de_presentar() {
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let ruta = std::env::var_os("PIXPIN_PPTX")
            .map(PathBuf::from)
            .unwrap_or_else(|| "E:/pixpin-office-muestras/pptx/poi-charla-48-diapos.pptx".into());
        if !ruta.is_file() {
            println!("sin muestra en {}", ruta.display());
            return;
        }
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let documento = pixpin_office::Documento::abrir(&ruta).expect("abre");
        let mut n = Nativas::nuevas(documento, PintorOffice::nuevo(&motor).expect("letras"));
        let total = n.documento.total();
        for i in 0..total.min(6) {
            println!(
                "diapositiva {}: {:?}; notas: {:?}",
                i + 1,
                n.transicion(i),
                n.notas(i).map(|t| t.chars().take(40).collect::<String>())
            );
        }
        let pagina = n
            .diapositiva(0)
            .map(|p| (p.ancho, p.alto))
            .expect("primera");
        let (ancho, alto) = (1280.0, 720.0);
        let caja = encajar_entera(pagina, (ancho, alto));
        let empezo = std::time::Instant::now();
        let vieja = n.foto(&motor, 0, caja, total).expect("foto 1");
        println!(
            "una diapositiva a mapa de bits: {} ms",
            empezo.elapsed().as_millis()
        );
        let mut pintadas = HashMap::new();
        pintadas.insert(0, vieja.clone());
        pintadas.insert(1, n.foto(&motor, 1, caja, total).expect("foto 2"));
        pintadas.insert(2, n.foto(&motor, 2, caja, total).expect("foto 3"));
        pintadas.insert(3, n.foto(&motor, 3, caja, total).expect("foto 4"));
        let medidas = vec![pagina; total];
        let pase = Pase {
            actual: 1,
            total,
            ..Default::default()
        };
        let guardar = |nombre: &str, fuera: &FueraDePantalla| {
            fuera.esperar_gpu().expect("esperar");
            let (ancho, alto, pixeles) = fuera.leer_rgba().expect("leer");
            let png = pixpin_codec::imagen::codificar_png(&ImagenRgba {
                ancho,
                alto,
                pixeles,
            })
            .expect("png");
            let salida = carpeta.join(nombre);
            std::fs::write(&salida, png).expect("guardar");
            println!("{}", salida.display());
        };
        let fuera =
            FueraDePantalla::nuevo(&motor, d.d3d(), ancho as u32, alto as u32).expect("superficie");
        let efectos = [
            ("empuje", Efecto::Empuje(pp::Sentido::IZQUIERDA)),
            ("barrido", Efecto::Barrido(pp::Sentido::IZQUIERDA)),
            ("fundido", Efecto::Fundido { por_negro: false }),
            ("cubrir", Efecto::Cubrir(pp::Sentido { dx: 0, dy: -1 })),
        ];
        for (nombre, efecto) in efectos {
            let capas = pp::capas(efecto, 0.5);
            motor
                .dibujar(&fuera.destino, |p| {
                    pintar(
                        p,
                        &pase,
                        &medidas,
                        &pintadas,
                        Some((&vieja, &capas)),
                        None,
                        false,
                        None,
                        None,
                        (ancho, alto),
                        1.0,
                        &textos,
                    );
                })
                .expect("pintar");
            guardar(&format!("presentar-transicion-{nombre}.png"), &fuera);
        }
        // Una pantalla: las notas abajo (N), la pastilla y el aviso de la T.
        // La 3, que tiene notas.
        let pase = Pase { actual: 2, ..pase };
        let notas = n.notas(2);
        let aviso = textos.t("diapositivas-transiciones-no");
        motor
            .dibujar(&fuera.destino, |p| {
                pintar(
                    p,
                    &pase,
                    &medidas,
                    &pintadas,
                    None,
                    None,
                    true,
                    Some(notas.as_deref()),
                    Some(&aviso),
                    (ancho, alto),
                    1.0,
                    &textos,
                );
            })
            .expect("pintar");
        guardar("presentar-notas-abajo.png", &fuera);
        // Dos pantallas: la vista del orador en la del portatil.
        let orador = FueraDePantalla::nuevo(&motor, d.d3d(), 1366, 768).expect("superficie");
        motor
            .dibujar(&orador.destino, |p| {
                pintar_vista_orador(
                    p,
                    (1366.0, 768.0),
                    1.0,
                    &pase,
                    pagina,
                    &pintadas,
                    notas.as_deref(),
                    754_000,
                    "14:05",
                    true,
                    &textos,
                );
            })
            .expect("pintar");
        guardar("presentar-vista-orador.png", &orador);
    }
}
