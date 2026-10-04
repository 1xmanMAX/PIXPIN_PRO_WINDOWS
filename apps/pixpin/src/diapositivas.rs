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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use anyhow::{Context, Result};
use pixpin_codec::imagen::ImagenRgba;
use pixpin_render::icono::material;
use pixpin_render::{Color, Pintor, RectF, Superficie};
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
const VK_PUNTO: u32 = 0xBE;

/// Las teclas de PowerPoint en pase de diapositivas, que son las que mandan
/// los mandos de presentar (casi todos: AvPag/RePag; otros, las flechas).
pub fn mando_de_tecla(vk: u32, shift: bool) -> Option<Mando> {
    Some(match vk {
        VK_RIGHT | VK_DOWN | VK_NEXT | VK_INTRO | VK_N => Mando::Siguiente,
        VK_ESPACIO if shift => Mando::Anterior,
        VK_ESPACIO => Mando::Siguiente,
        VK_LEFT | VK_UP | VK_PRIOR | VK_RETROCESO | VK_P => Mando::Anterior,
        VK_HOME => Mando::Primera,
        VK_END => Mando::Ultima,
        VK_B | VK_PUNTO => Mando::Negro,
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
        if self.negro && !matches!(m, Mando::Salir | Mando::Pastilla) {
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

// ---------------------------------------------------------------------------
// Abrir

/// **Presenta** esa presentacion (o dice por que no), en su propio hilo.
/// Lo llama el chat al tocar la burbuja o «Abrir aqui» (`lector.rs`).
pub fn lanzar(idioma: pixpin_store::Idioma, ubicacion: pixpin_store::Ubicacion, ruta: &Path) {
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
                presentar(r, &textos, ubicacion.raiz(), &ruta)
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
    if pixpin_shell::preguntar(nula, &textos.t("diapositivas-titulo"), &pregunta)
        && let Err(e) = pixpin_shell::abrir_con_otra::abrir_con_otra(nula, ruta)
    {
        tracing::warn!(?e, "no se pudo abrir con otra app");
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

fn presentar(recursos: &Recursos, textos: &Catalogo, raiz: &Path, ruta: &Path) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .principal()
        .context("sin monitor principal")?
        .to_owned();
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
    ventana.enfocar();

    let (tx, rx_pedidos) = mpsc::channel::<(usize, u32)>();
    let (tx_llega, rx) = mpsc::channel::<Llega>();
    let hwnd = ventana.handle().0 as isize;
    let (raiz_h, ruta_h) = (raiz.to_path_buf(), ruta.to_path_buf());
    std::thread::Builder::new()
        .name("diapositivas-hojas".into())
        .spawn(move || hilo(raiz_h, ruta_h, rx_pedidos, tx_llega, hwnd))
        .context("sin hilo para las diapositivas")?;

    let mut pase = Pase {
        pastilla: true,
        ..Default::default()
    };
    let mut medidas: Vec<(f32, f32)> = Vec::new();
    let mut pintadas: HashMap<usize, ID2D1Bitmap1> = HashMap::new();
    let mut pedida: Option<usize> = None;
    let mut fallo: Option<String> = None;
    let mut pastilla_hasta = ahora_ms() + MS_DE_LA_PASTILLA;
    let mut botones: Vec<(RectF, Mando)> = Vec::new();
    let mut raton = (0.0f32, 0.0f32);
    let mut hay_que_pintar = true;
    let mut vivo = true;
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
            if h != ventana.handle() {
                continue;
            }
            hay_que_pintar = true;
            let mando = match evento {
                EventoOverlay::Cerrar => Some(Mando::Salir),
                EventoOverlay::Tecla { vk, shift, .. } => mando_de_tecla(vk, shift),
                EventoOverlay::Rueda(d) if d < 0 => Some(Mando::Siguiente),
                EventoOverlay::Rueda(d) if d > 0 => Some(Mando::Anterior),
                EventoOverlay::BotonDerechoPulsado(_) => Some(Mando::Anterior),
                EventoOverlay::RatonMovido(p) => {
                    let nuevo = ((p.x - area.x) as f32, (p.y - area.y) as f32);
                    // Mover el raton ensena la pastilla un rato, como en
                    // cualquier reproductor.
                    if (nuevo.0 - raton.0).abs() + (nuevo.1 - raton.1).abs() > 4.0 {
                        pastilla_hasta = ahora_ms() + MS_DE_LA_PASTILLA;
                    }
                    raton = nuevo;
                    None
                }
                EventoOverlay::BotonPulsado(p) => {
                    raton = ((p.x - area.x) as f32, (p.y - area.y) as f32);
                    let sobre_boton = botones
                        .iter()
                        .find(|(r, _)| dentro(r, raton.0, raton.1))
                        .map(|(_, m)| *m);
                    Some(sobre_boton.unwrap_or_else(|| mando_de_clic(raton.0, ancho)))
                }
                _ => None,
            };
            if let Some(m) = mando {
                if m == Mando::Pastilla {
                    // En el medio: esconder o ensenar ya, sin esperar.
                    let vista = pase.pastilla && pastilla_hasta > ahora_ms();
                    pase.pastilla = !vista;
                    pastilla_hasta = if pase.pastilla { u64::MAX } else { 0 };
                    continue;
                }
                if !pase.aplicar(m) {
                    vivo = false;
                }
            }
        }
        if !vivo {
            break;
        }
        let ahora = ahora_ms();
        if pastilla_hasta != u64::MAX && pastilla_hasta >= ahora && pastilla_hasta < ahora + 200 {
            hay_que_pintar = true;
        }
        // Se pide la que se mira; al llegar, la de al lado, para que pasar
        // sea instantaneo. Las lejanas se sueltan: cien diapositivas a
        // pantalla completa serian cientos de megas de video.
        if pase.total > 0 {
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
        }
        if hay_que_pintar {
            hay_que_pintar = false;
            let mostrar_pastilla = pase.pastilla && pastilla_hasta > ahora;
            if let Ok(destino) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&destino, |p: &Pintor| {
                    botones = pintar(
                        p,
                        &pase,
                        &medidas,
                        &pintadas,
                        fallo.as_deref(),
                        mostrar_pastilla,
                        (ancho, alto),
                        e,
                        textos,
                    );
                });
                let _ = superficie.presentar();
            }
        }
        pixpin_shell::overlay::esperar_eventos(Some(100));
    }
    drop(tx);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn pintar(
    p: &Pintor,
    pase: &Pase,
    medidas: &[(f32, f32)],
    pintadas: &HashMap<usize, ID2D1Bitmap1>,
    fallo: Option<&str>,
    pastilla: bool,
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
    if let (Some(b), Some(m)) = (pintadas.get(&pase.actual), medidas.get(pase.actual)) {
        p.bitmap(b, encajar(*m, (ancho, alto)), None, false);
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
        y: alto - lado - 28.0 * e,
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
        for vk in [VK_RIGHT, VK_DOWN, VK_NEXT, VK_ESPACIO, VK_INTRO, VK_N] {
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
}
