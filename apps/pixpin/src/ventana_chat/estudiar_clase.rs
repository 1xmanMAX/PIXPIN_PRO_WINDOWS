//! **Estudiar una clase grabada** (25-oct-2026, el usuario: «una ventana
//! especial, siguiendo la misma interfaz de las demas ventanas, donde se vea
//! la grabacion del video con su transcripcion al lado; al otro lado las
//! capturas, los marcadores, etc., todo integrado; y que las capturas y el
//! texto se puedan desplazar de forma que uno siga al otro»).
//!
//! Una ventana normal, en su hilo, como la de las figuras:
//!
//! - **A la izquierda, lo que se ve**: el video de la clase (con su sonido)
//!   o, si no se grabo video, la captura del minuto que suena en grande y
//!   la nota de voz sonando. Debajo, la barra del tiempo con una raya por
//!   captura y una banderita por marcador (un clic cerca se pega a ellas),
//!   los mandos (atras, reproducir, adelante, la velocidad, «Marcador» y
//!   «Seguir») y la lista de marcadores.
//! - **En el centro, el texto**, con su minuto delante de cada trozo y los
//!   marcadores metidos en su sitio. Un clic lleva ahi; lo que suena se
//!   resalta.
//! - **A la derecha, las capturas** en una tira, con su minuto. Un clic
//!   lleva ahi y la ensena en grande.
//!
//! **Que una siga a la otra** (`pixpin_proyecto::clase`): con «Seguir»
//! encendido, mientras suena el texto y la tira van solos a lo que suena; y
//! al mover una con la rueda, la otra se pone en el mismo minuto. Mover una
//! a mano deja de seguir al sonido un rato, para poder leer.
//!
//! **Los marcadores** se guardan en la nota de voz, en `resto["marcadores"]`
//! (`[{ms, texto}]`), que viaja al movil como lo demas de `resto`. Las
//! banderitas del movil (`marcas`, sin nota) se ensenan con ellos.
//!
//! **Como los visores** (25-oct-2026, el usuario: «el visualizador de las
//! grabaciones deberia ser como el previsualizador, con la misma
//! interfaz»): la ventana es un pin-lector (`crate::lector_pin`), sin marco,
//! que se estira por los bordes, con la barra de los visores fuera, encima
//! (el asa para llevarla, el nombre, la luna/sol, pantalla completa, el pin
//! y cerrar). **Claro u oscuro** como los visores (`crate::oscuro`: lo
//! guardado o, si nunca se toco, el tema de Windows), y la luna/sol lo
//! cambia. Lo de dentro sigue repartido igual.
//!
//! **Acercar lo que se ve como en un lienzo** (`pixpin_ui::encuadre`): la
//! rueda encima del video o de la captura acerca hacia el raton, arrastrar
//! la lleva, el doble clic la vuelve a encajar; abajo a la derecha, −, + y
//! encajar. Al cambiar de captura o de video a captura, vuelve a encajada.
//!
//! **Mas sitio para la imagen** (27-oct-2026, como el movil 497f6aac): en
//! la esquina, el boton de pantalla entera (como el de YouTube) pone la
//! ventana a pantalla completa con la imagen ocupandola, y otra vez la
//! reduce; «Texto» (o la T) oculta o ensena la transcripcion y su sitio se
//! lo queda la imagen; la ventana se deja estrechar hasta 900 (las columnas
//! ceden y los mandos se quedan en iconos); y las miniaturas van con su
//! proporcion de verdad, sin franjas, mas pequenas. Sin el tanto por ciento.
//!
//! **Anotar encima** con el lienzo de siempre: «Anotar» abre la captura que
//! se ve en SU lienzo (`foto-<id>`, el de las fotos del chat: K11,
//! `crate::foto_anotada`), con todas las herramientas (lapiz, resaltador,
//! flecha, rectangulo, texto, goma). Lo anotado viaja con la foto como
//! cualquier foto anotada, se ve en su burbuja del chat y aqui encima de la
//! captura. Con el video, «Anotar» lo para y guarda el fotograma que se ve
//! como una captura mas de la clase, en su minuto, y la abre para anotar.
//!
//! **El clic en la barra del tiempo** cae bajo el raton: los clics se
//! cuentan desde la esquina de la ventana DE VERDAD (`clase::ms_del_raton`);
//! antes se restaba la esquina con la que se creo y, movida o estirada por
//! fuera, todo quedaba corrido. Al arrastrar, el tirador va con el raton.
//!
//! **Los temas de la reunion** (28-oct-2026, `docs/jev.md`): si la reunion
//! se grabo con temas, encima de los marcadores va la agenda con ✓ y su
//! minuto (un clic en el tema salta ahi; en la casilla, se marca o desmarca
//! a mano y su lista de tareas tambien). Lo que quedo sin tratar va
//! resaltado, con «Pasar a la próxima reunión» (sale solo al preparar la
//! siguiente) y «Añadir a Tareas» (al Inbox).
//!
//! Teclado: espacio o K reproduce y para, ←/→ cinco segundos, J/L diez,
//! ↑/↓ el trozo anterior y el siguiente, M pone un marcador (se escribe su
//! nota e Intro lo guarda), S enciende y apaga «Seguir», A anota, T
//! oculta o ensena la transcripcion, Ctrl+0
//! encaja, Ctrl+más/menos acerca y aleja, F11 pantalla completa, Escape
//! sale de ella o cierra.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_codec::imagen::ImagenRgba;
use pixpin_geom::{Punto, Rect};
use pixpin_proyecto::clase::{self, Fila, Marcador};
use pixpin_proyecto::temas as agenda;
use pixpin_proyecto::{almacen, cuaderno};
use pixpin_render::{Color, MotorRender, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma, Ubicacion};
use pixpin_ui::encuadre::{self, Encuadre};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use super::{CLARO, LienzoVisto, OSCURO, Tema, con_alfa, mi};
use crate::caja_dibujo::hex;
use crate::lector_pin::{AccionPin, PinLector};
use crate::overlay::Recursos;
use crate::ventanita::dentro;

/// Lo que mide la ventana (logicos) y sus columnas.
const ANCHO: u32 = 1360;
const ALTO: u32 = 840;
/// Lo menos que se deja estirar (logicos). Mas sitio para la imagen
/// (27-oct-2026, como el movil 497f6aac): de 1200 a 900; estrecha, la tira
/// y el texto se estrechan y, si los mandos no caben con sus rotulos, se
/// quedan en iconos (`Disposicion::compacto`).
const ANCHO_MINIMO: f32 = 900.0;
const ALTO_MINIMO: f32 = 640.0;
const MARGEN: f32 = 16.0;
/// La tira de capturas, mas estrecha que antes (236): las miniaturas, mas
/// pequenas (checklist 25).
const ANCHO_FOTOS: f32 = 210.0;
const ANCHO_FOTOS_MINIMO: f32 = 150.0;
const CABECERA: f32 = 36.0;
/// La franja de lo importante (Jev) bajo la cabecera del texto: el filtro.
const FRANJA: f32 = 32.0;
/// El color de los marcadores: ambar, que se distingue del azul de las
/// capturas en la barra del tiempo.
const AMBAR: Color = hex(0xf2a33a);
/// Lo que se deja de seguir al sonido despues de mover una lista a mano.
const A_MANO: Duration = Duration::from_secs(5);
/// Lo que dura un aviso abajo.
const AVISO_DURA: Duration = Duration::from_millis(2200);
/// Los saltos del teclado y de los botones.
const SALTO_CORTO: i64 = 5_000;
const SALTO_LARGO: i64 = 10_000;
/// Un clic en la barra del tiempo a menos de esto (logicos) de una captura
/// o un marcador se queda en el.
const PEGAR: f32 = 6.0;
/// Las miniaturas de la tira se bajan a este ancho (pixeles): sobra para
/// una columna de 236 a 200 %, y treinta capturas a pantalla entera no
/// caben en la memoria del equipo de referencia.
const ANCHO_MINIATURA: u32 = 480;
/// Dos clics en lo que se ve antes de esto: encajar.
const DOBLE_CLIC: Duration = Duration::from_millis(400);
/// Arrastrando la barra, una busqueda exacta como mucho cada tanto: el
/// video no cae en un fotograma clave lejano y el motor no se atasca.
const BUSCAR_CADA: Duration = Duration::from_millis(70);
/// Lo que se mueve el raton antes de que pulsar lo que se ve sea llevarlo.
const ARRASTRE: f32 = 4.0;

const VK_ESPACIO: u32 = 0x20;
const VK_ENTRAR: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_RETROCESO: u32 = 0x08;
const VK_IZQUIERDA: u32 = 0x25;
const VK_ARRIBA: u32 = 0x26;
const VK_DERECHA: u32 = 0x27;
const VK_ABAJO: u32 = 0x28;
const VK_0: u32 = 0x30;
const VK_A: u32 = 0x41;
const VK_J: u32 = 0x4A;
const VK_K: u32 = 0x4B;
const VK_L: u32 = 0x4C;
const VK_M: u32 = 0x4D;
const VK_S: u32 = 0x53;
const VK_T: u32 = 0x54;
const VK_MAS: u32 = 0xBB;
const VK_MENOS: u32 = 0xBD;
const VK_MAS_NUM: u32 = 0x6B;
const VK_MENOS_NUM: u32 = 0x6D;

/// La ventana abierta (su HWND), -1 mientras nace, 0 sin ventana.
static ABIERTA: AtomicIsize = AtomicIsize::new(0);

/// Que clase abrir: el proyecto, el id de la nota de voz (o de una captura
/// o del video de la clase: se busca su nota) y el minuto donde empezar.
#[derive(Debug, Clone)]
struct Pedido {
    ubicacion: Ubicacion,
    proyecto: String,
    id: String,
    ms: i64,
}

/// Otra clase pedida con la ventana ya abierta: la recoge su bucle.
static OTRA: Mutex<Option<Pedido>> = Mutex::new(None);

/// **Abre la clase en la ventana de estudiar**, o la cambia y la trae
/// delante si ya estaba abierta. `id` puede ser la nota de voz o cualquier
/// cosa de la clase (una captura, el video): se busca su nota.
pub(crate) fn abrir(idioma: Idioma, ubicacion: Ubicacion, proyecto: String, id: String, ms: i64) {
    let pedido = Pedido {
        ubicacion,
        proyecto,
        id,
        ms,
    };
    let ya = ABIERTA.load(Ordering::SeqCst);
    if ya != 0 {
        if let Ok(mut g) = OTRA.lock() {
            *g = Some(pedido);
        }
        if ya > 0 {
            VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(ya as *mut _));
            pixpin_shell::overlay::despertar(ya);
        }
        return;
    }
    ABIERTA.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new()
        .name("estudiar-clase".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            // Si el dispositivo grafico se pierde, la ventana se reabre en
            // el mismo minuto.
            let mut pedido = pedido;
            let hecho = crate::dispositivo_perdido::con_recursos("estudiar clase", |r| {
                bucle(r, &textos, &mut pedido)
            });
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la ventana de estudiar la clase");
            }
            if let Ok(mut g) = OTRA.lock() {
                *g = None;
            }
            ABIERTA.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de estudiar la clase");
        ABIERTA.store(0, Ordering::SeqCst);
    }
}

/// **Si un mensaje es de una clase grabada**: `(id de su nota de voz, minuto)`.
/// La nota misma (`grabacion: "clase"`) va al principio; una captura o el
/// video de la clase (`enGrabacion`), a su minuto.
pub(super) fn de_una_clase(m: &cuaderno::Mensaje) -> Option<(String, i64)> {
    if m.resto.get("grabacion").and_then(|v| v.as_str()) == Some("clase") {
        return Some((m.id.clone(), 0));
    }
    let en = m.resto.get("enGrabacion")?;
    let de = en.get("de")?.as_str()?.to_string();
    Some((de, en.get("ms").and_then(|v| v.as_i64()).unwrap_or(0)))
}

// --- La clase leida ------------------------------------------------------------

/// Una senal de la barra y de la lista: un marcador (con su nota) o una
/// banderita del movil (sin ella).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Senal {
    ms: i64,
    texto: String,
    banderita: bool,
    /// Un clip de la camara de la reunion presencial (27-oct-2026): su
    /// fichero y lo que dura. No se edita ni se quita como un marcador; un
    /// clic lleva a su minuto y lo abre. Los del movil (`fuente: "movil"`,
    /// la camara del movil) van por aqui igual; el `bool` dice que son
    /// suyos, para su rotulo.
    clip: Option<(PathBuf, i64, bool)>,
}

impl Senal {
    fn icono(&self) -> &'static pixpin_render::icono::Icono {
        if self.clip.is_some() {
            &mi::PLAY_ARROW
        } else {
            &mi::FLAG
        }
    }

    fn nota(&self, textos: &Catalogo) -> String {
        if let Some((_, duracion, del_movil)) = &self.clip {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("duracion", pixpin_voz::marca_de_tiempo(*duracion));
            let clave = if *del_movil {
                "estudiar-clip-movil"
            } else {
                "estudiar-clip"
            };
            return textos.t_args(clave, &args);
        }
        if self.texto.is_empty() {
            textos.t("estudiar-marcador-sin-nota")
        } else {
            self.texto.clone()
        }
    }

    /// Lo que hace un clic en ella: ir a su minuto, y si es un clip, abrirlo.
    fn zona(&self, indice: usize) -> Zona {
        if self.clip.is_some() {
            Zona::Clip(indice)
        } else {
            Zona::Ir(self.ms)
        }
    }
}

/// Todo lo de una clase, leido del cuaderno.
struct Clase {
    proyecto: String,
    id: String,
    nombre: String,
    duracion_ms: i64,
    audio: Option<PathBuf>,
    video: Option<PathBuf>,
    /// `(ms, ruta)` en orden; las que ya no estan en el disco, fuera.
    capturas: Vec<(i64, PathBuf)>,
    /// El mensaje de cada captura (el mismo orden): para su lienzo anotado.
    fotos: Vec<cuaderno::Mensaje>,
    /// De cada captura (el mismo orden): la persona que la aporto (un
    /// colaborador; lo mio, `None`) y si su hora no se pudo comprobar.
    autores: Vec<(Option<String>, bool)>,
    /// Los clips de la camara (reunion presencial) y del movil (la camara
    /// del movil), en la misma lista de la nota: `(ms, duracion, ruta, del
    /// movil)`.
    clips: Vec<(i64, i64, PathBuf, bool)>,
    /// Los trozos del texto: su minuto (si lo lleva) y lo que dice.
    lineas: Vec<(Option<i64>, String)>,
    /// Lo importante que hallo Jev (`resto["importantes"]`); `None` si aun
    /// no se miro.
    importantes: Option<Vec<clase::Importante>>,
    /// Los temas de la reunion (`resto["temas"]`), en el orden de la agenda.
    temas: Vec<agenda::Tema>,
    senales: Vec<Senal>,
    /// El texto con las senales en su minuto, y el minuto de cada fila.
    filas: Vec<Fila>,
    tiempos_filas: Vec<i64>,
}

impl Clase {
    fn leer(raiz: &Path, proyecto: &str, id: &str) -> Result<Clase> {
        let carpeta = almacen::carpeta(raiz, proyecto);
        let mensajes = cuaderno::Cuaderno::leer_de(&carpeta)
            .context("sin cuaderno")?
            .mensajes;
        let m = mensajes
            .iter()
            .find(|m| m.id == id)
            .context("la clase ya no esta en el chat")?;
        // Una captura o el video: su nota.
        let voz = match de_una_clase(m) {
            Some((de, _)) if de != m.id => mensajes
                .iter()
                .find(|v| v.id == de)
                .context("la nota de la clase ya no esta")?,
            _ => m,
        };
        let con_ruta = |id: &str| {
            let c = mensajes.iter().find(|c| c.id == id)?;
            let r = super::ruta_del_mensaje(raiz, proyecto, c).filter(|r| r.is_file())?;
            Some((c, r))
        };
        let ruta_de = |id: &str| con_ruta(id).map(|(_, r)| r);
        let (capturas, fotos): (Vec<(i64, PathBuf)>, Vec<cuaderno::Mensaje>) =
            clase::capturas_de(&voz.resto)
                .into_iter()
                .filter_map(|(ms, id)| {
                    let (m, r) = con_ruta(&id)?;
                    Some(((ms, r), m.clone()))
                })
                .unzip();
        let autores = fotos
            .iter()
            .map(|m: &cuaderno::Mensaje| {
                (
                    m.aparato
                        .as_deref()
                        .and_then(crate::sincronizar::compartidos_ui::autor_aqui),
                    pixpin_proyecto::por_su_hora::sin_comprobar(voz, &m.id),
                )
            })
            .collect();
        let video = voz
            .resto
            .get("video")
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .and_then(ruta_de);
        let clips = pixpin_proyecto::presencial::clips_de(&voz.resto)
            .into_iter()
            .filter_map(|c| {
                let (m, r) = con_ruta(&c.id)?;
                let del_movil = pixpin_proyecto::presencial::es_del_movil(&m.resto);
                Some((c.ms, c.duracion_ms, r, del_movil))
            })
            .collect();
        let audio = super::ruta_del_mensaje(raiz, proyecto, voz).filter(|r| r.is_file());
        let lineas = super::trozos_de_la_letra(voz)
            .into_iter()
            .map(|t| (t.ms, t.texto))
            .collect();
        let mut c = Clase {
            proyecto: proyecto.to_string(),
            id: voz.id.clone(),
            nombre: if voz.nombre.trim().is_empty() {
                voz.texto.clone()
            } else {
                voz.nombre.trim_end_matches(".m4a").to_string()
            },
            duracion_ms: voz.duracion_ms.max(0),
            audio,
            video,
            capturas,
            fotos,
            autores,
            clips,
            lineas,
            importantes: clase::importantes_de(&voz.resto),
            temas: agenda::temas_de(&voz.resto),
            senales: Vec::new(),
            filas: Vec::new(),
            tiempos_filas: Vec::new(),
        };
        c.poner_senales(senales_de(voz));
        Ok(c)
    }

    /// Pone los marcadores y las banderitas, con los clips de la camara
    /// metidos en su minuto (los que traiga `senales` se cambian por los de
    /// la clase).
    fn poner_senales(&mut self, mut senales: Vec<Senal>) {
        senales.retain(|s| s.clip.is_none());
        senales.extend(self.clips.iter().map(|(ms, duracion, ruta, movil)| Senal {
            ms: *ms,
            texto: String::new(),
            banderita: false,
            clip: Some((ruta.clone(), *duracion, *movil)),
        }));
        senales.sort_by_key(|s| s.ms);
        self.senales = senales;
        let lineas: Vec<Option<i64>> = self.lineas.iter().map(|(ms, _)| *ms).collect();
        let ms: Vec<i64> = self.senales.iter().map(|s| s.ms).collect();
        self.filas = clase::mezclar(&lineas, &ms);
        self.tiempos_filas = clase::tiempos_de_filas(&self.filas, &lineas, &ms);
    }

    /// Si va la franja de lo importante: con algo hallado, o mientras Jev
    /// lo busca.
    fn con_franja(&self) -> bool {
        self.importantes.as_ref().is_some_and(|v| !v.is_empty())
            || (self.importantes.is_none() && crate::jev::reunion::en_marcha(&self.id))
    }

    /// Las tareas halladas que aun no estan en Tareas.
    fn tareas_por_apuntar(&self) -> usize {
        self.importantes.as_ref().map_or(0, |v| {
            v.iter()
                .filter(|i| i.tipo == clase::TipoImportante::Tarea && !i.en_tareas)
                .count()
        })
    }

    fn tiempos_capturas(&self) -> Vec<i64> {
        self.capturas.iter().map(|(ms, _)| *ms).collect()
    }
}

/// Los marcadores y las banderitas de una nota, en orden.
fn senales_de(m: &cuaderno::Mensaje) -> Vec<Senal> {
    let mut v: Vec<Senal> = clase::marcadores_de(&m.resto)
        .into_iter()
        .map(|Marcador { ms, texto }| Senal {
            ms,
            texto,
            banderita: false,
            clip: None,
        })
        .collect();
    for ms in crate::voz::marcas_de(m) {
        if !v.iter().any(|s| s.ms == ms) {
            v.push(Senal {
                ms,
                texto: String::new(),
                banderita: true,
                clip: None,
            });
        }
    }
    v.sort_by_key(|s| s.ms);
    v
}

/// Cambia la nota en el cuaderno (relee: el chat o el movil pudieron
/// tocarla) y avisa al chat. Devuelve como quedo.
fn cambiar_la_nota(
    raiz: &Path,
    proyecto: &str,
    id: &str,
    cambio: impl FnOnce(&mut cuaderno::Mensaje) -> bool,
) -> Result<cuaderno::Mensaje> {
    let carpeta = almacen::carpeta(raiz, proyecto);
    let m = cuaderno::cambiar(&carpeta, id, cambio)?.context("la nota ya no esta")?;
    super::refrescar();
    Ok(m)
}

// --- Lo que suena ----------------------------------------------------------------

/// El video (con su sonido) o la nota de voz, con las mismas ordenes.
enum Medio {
    Video {
        r: pixpin_pin::Reproductor,
        bitmap: Option<ID2D1Bitmap1>,
        /// A donde ir en cuanto se sepa el tamano (la carga es asincrona).
        ir: Option<i64>,
        velocidad: f32,
    },
    Audio {
        s: pixpin_audio::salida::Salida,
        ir: Option<i64>,
        velocidad: f32,
        sonando: bool,
    },
    Nada,
}

impl Medio {
    fn abrir(
        d3d: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
        c: &Clase,
        ms: i64,
    ) -> Medio {
        if let Some(v) = &c.video {
            match pixpin_pin::Reproductor::para_ver(d3d, v) {
                Ok(r) => {
                    return Medio::Video {
                        r,
                        bitmap: None,
                        ir: Some(ms),
                        velocidad: 1.0,
                    };
                }
                Err(e) => tracing::warn!(?e, "el video de la clase no se pudo abrir; va el audio"),
            }
        }
        Medio::audio(c, ms)
    }

    fn audio(c: &Clase, ms: i64) -> Medio {
        match c.audio.as_deref().map(pixpin_audio::salida::Salida::abrir) {
            Some(Ok(s)) => Medio::Audio {
                s,
                ir: Some(ms),
                velocidad: 1.0,
                sonando: false,
            },
            Some(Err(e)) => {
                tracing::warn!(?e, "la nota de la clase no se pudo abrir");
                Medio::Nada
            }
            None => Medio::Nada,
        }
    }

    fn es_video(&self) -> bool {
        matches!(self, Medio::Video { .. })
    }

    fn fallo(&self) -> bool {
        match self {
            Medio::Video { r, .. } => r.fallo(),
            Medio::Audio { s, .. } => s.fallo(),
            Medio::Nada => false,
        }
    }

    fn posicion_ms(&self) -> i64 {
        match self {
            Medio::Video { r, ir, .. } => {
                ir.unwrap_or_else(|| (r.posicion().0 * 1000.0).round() as i64)
            }
            Medio::Audio { s, ir, .. } => ir.unwrap_or_else(|| s.posicion_ms()),
            Medio::Nada => 0,
        }
    }

    fn duracion_ms(&self) -> i64 {
        match self {
            Medio::Video { r, .. } => r.posicion().1.map_or(0, |d| (d * 1000.0).round() as i64),
            Medio::Audio { s, .. } => s.duracion_ms(),
            Medio::Nada => 0,
        }
    }

    fn sonando(&self) -> bool {
        match self {
            Medio::Video { r, .. } => r.reproduciendo() && !r.acabado(),
            Medio::Audio { sonando, .. } => *sonando,
            Medio::Nada => false,
        }
    }

    fn velocidad(&self) -> f32 {
        match self {
            Medio::Video { velocidad, .. } | Medio::Audio { velocidad, .. } => *velocidad,
            Medio::Nada => 1.0,
        }
    }

    fn alternar(&mut self) {
        match self {
            Medio::Video { r, .. } => {
                // Al final, otra vez desde el principio: buscar arranca
                // solo si el motor no se quedo en pausa.
                if r.acabado() {
                    r.buscar(0.0, false);
                    if !r.reproduciendo() {
                        r.alternar_pausa();
                    }
                } else {
                    r.alternar_pausa();
                }
            }
            Medio::Audio {
                s,
                velocidad,
                sonando,
                ..
            } => {
                if *sonando {
                    s.pausar();
                    *sonando = false;
                } else {
                    if s.lista() && s.duracion_ms() > 0 && s.posicion_ms() >= s.duracion_ms() {
                        s.ir_a_ms(0);
                    }
                    match s.tocar(*velocidad) {
                        Ok(()) => *sonando = true,
                        Err(e) => tracing::warn!(?e, "la nota de la clase no arranca"),
                    }
                }
            }
            Medio::Nada => {}
        }
    }

    /// Va a `ms`, al fotograma exacto. Tambien arrastrando la barra (cada
    /// `BUSCAR_CADA`): el salto aproximado cae en el fotograma clave mas
    /// cercano, y en una grabacion de pantalla pueden estar a segundos.
    fn ir_a(&mut self, ms: i64) {
        let ms = ms.max(0);
        match self {
            Medio::Video { r, ir, .. } => {
                if ir.is_some() {
                    *ir = Some(ms);
                } else {
                    r.buscar(ms as f64 / 1000.0, false);
                }
            }
            Medio::Audio { s, ir, .. } => {
                if s.lista() {
                    s.ir_a_ms(ms);
                    *ir = None;
                } else {
                    *ir = Some(ms);
                }
            }
            Medio::Nada => {}
        }
    }

    fn otra_velocidad(&mut self) {
        match self {
            Medio::Video { r, velocidad, .. } => {
                *velocidad = pixpin_audio::reloj::siguiente_velocidad(*velocidad);
                r.poner_velocidad(*velocidad as f64);
            }
            Medio::Audio { s, velocidad, .. } => {
                *velocidad = pixpin_audio::reloj::siguiente_velocidad(*velocidad);
                s.poner_velocidad(*velocidad);
            }
            Medio::Nada => {}
        }
    }

    /// Lo de cada vuelta: el fotograma nuevo del video, el salto pendiente
    /// en cuanto se puede y el final de la nota. Devuelve si hay que
    /// repintar.
    fn latido(&mut self, motor: &MotorRender) -> bool {
        match self {
            Medio::Video { r, bitmap, ir, .. } => {
                if let Some(ms) = *ir
                    && r.dimensiones().is_some()
                {
                    r.buscar(ms as f64 / 1000.0, false);
                    *ir = None;
                }
                // Clonar la interfaz suelta el prestamo del reproductor.
                let textura = r.tick().cloned();
                match textura {
                    Some(t) => {
                        if bitmap.is_none() {
                            *bitmap = motor.bitmap_desde_textura(&t).ok();
                        }
                        true
                    }
                    None => false,
                }
            }
            Medio::Audio { s, ir, sonando, .. } => {
                if let Some(ms) = *ir
                    && s.lista()
                {
                    s.ir_a_ms(ms);
                    *ir = None;
                }
                if s.termino() {
                    s.pausar();
                    *sonando = false;
                    return true;
                }
                *sonando
            }
            Medio::Nada => false,
        }
    }
}

// --- Las imagenes, en su hilo ---------------------------------------------------

/// Lo que vuelve del hilo de las imagenes, con lo que mide la captura de
/// verdad (lo anotado va en sus pixeles).
enum Cargada {
    Miniatura(usize, ImagenRgba, (u32, u32)),
    Grande(usize, ImagenRgba, (u32, u32)),
}

/// Lee las capturas fuera del hilo de la ventana: las miniaturas de todas,
/// una detras de otra, y la grande que se pida (esa primero).
///
/// **La entera al acercar** (27-oct-2026, «a resolucion COMPLETA, para que
/// se vean mas detalles»): la grande se lee a 2560 como mucho (sobra para
/// verla encajada); al acercarla mas alla de eso se pide `(i, true)` y
/// llega a su tamano de verdad (hasta lo que admita la grafica).
struct Cargador {
    pedir: Sender<(usize, bool)>,
    llegan: Receiver<Cargada>,
}

impl Cargador {
    fn lanzar(rutas: Vec<PathBuf>, lado_maximo: u32, lado_entero: u32) -> Option<Cargador> {
        let (pedir, pedidas) = channel::<(usize, bool)>();
        let (envia, llegan) = channel::<Cargada>();
        let lanzado = std::thread::Builder::new()
            .name("estudiar-imagenes".into())
            .spawn(move || {
                let grande = |(i, entera): (usize, bool)| -> Option<Cargada> {
                    let img = pixpin_codec::imagen::cargar(rutas.get(i)?).ok()?;
                    let medida = (img.ancho, img.alto);
                    let lado = if entera {
                        lado_entero.max(lado_maximo)
                    } else {
                        lado_maximo
                    };
                    Some(Cargada::Grande(i, reducir(img, lado), medida))
                };
                for (i, ruta) in rutas.iter().enumerate() {
                    // Lo pedido va antes que la tira: es lo que se esta
                    // mirando.
                    while let Ok(g) = pedidas.try_recv() {
                        if let Some(c) = grande(g)
                            && envia.send(c).is_err()
                        {
                            return;
                        }
                    }
                    if let Ok(img) = pixpin_codec::imagen::cargar(ruta) {
                        let ancho = img.ancho.min(ANCHO_MINIATURA);
                        let medida = (img.ancho, img.alto);
                        let m = reducir(img, ancho.max(1));
                        if envia.send(Cargada::Miniatura(i, m, medida)).is_err() {
                            return;
                        }
                    }
                }
                for g in pedidas {
                    if let Some(c) = grande(g)
                        && envia.send(c).is_err()
                    {
                        return;
                    }
                }
            });
        match lanzado {
            Ok(_) => Some(Cargador { pedir, llegan }),
            Err(e) => {
                tracing::warn!(?e, "no se pudo lanzar el hilo de las imagenes de la clase");
                None
            }
        }
    }
}

/// Baja una imagen para que su lado mayor no pase de `lado` (o su ancho, si
/// es una miniatura), sin deformarla.
fn reducir(img: ImagenRgba, lado: u32) -> ImagenRgba {
    let mayor = img.ancho.max(img.alto);
    if mayor <= lado || mayor == 0 {
        return img;
    }
    let k = lado as f32 / mayor as f32;
    let (w, h) = (
        ((img.ancho as f32 * k).round() as u32).max(1),
        ((img.alto as f32 * k).round() as u32).max(1),
    );
    pixpin_codec::imagen::redimensionar(img, w, h).unwrap_or_else(|e| {
        tracing::debug!(?e, "una captura de la clase no se pudo reducir");
        ImagenRgba {
            ancho: 1,
            alto: 1,
            pixeles: vec![0; 4],
        }
    })
}

/// Un bitmap con su tamano.
type Imagen = (ID2D1Bitmap1, u32, u32);

/// Un `RectF` como caja del encuadre, y al reves.
fn caja_de(r: RectF) -> encuadre::Caja {
    (r.x, r.y, r.ancho, r.alto)
}

fn rect(c: encuadre::Caja) -> RectF {
    RectF {
        x: c.0,
        y: c.1,
        ancho: c.2,
        alto: c.3,
    }
}

fn subir(motor: &MotorRender, img: &ImagenRgba) -> Option<Imagen> {
    motor
        .bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles)
        .ok()
        .map(|b| (b, img.ancho, img.alto))
}

// --- La ventana ------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Zona {
    Tocar,
    Atras,
    Adelante,
    Velocidad,
    Marcar,
    Seguir,
    /// Cambiar entre el video y la captura del minuto (con video).
    CambiarVista,
    /// La barra del tiempo entera.
    Tiempo,
    /// Lo que se ve: un clic reproduce o para.
    Medio,
    /// Un trozo del texto o una senal: ir a ese milisegundo.
    Ir(i64),
    /// Quitar la senal de ese milisegundo.
    Quitar(i64),
    /// Un clip de la camara (su senal): ir a su minuto y abrirlo.
    Clip(usize),
    /// Una captura de la tira: ir a su minuto y ensenarla.
    Captura(usize),
    /// Las listas, para la rueda.
    ListaTexto,
    ListaFotos,
    /// Los mandos del encuadre de lo que se ve.
    ZoomMenos,
    ZoomMas,
    Encajar,
    /// Anotar lo que se ve (la captura, o el fotograma del video).
    Anotar,
    /// Ensenar u ocultar la columna de la transcripcion.
    Transcripcion,
    /// La imagen a pantalla entera, o reducirla (en su esquina).
    PantallaEntera,
    /// Una chapita del filtro de lo importante (`None`: todo).
    Filtro(Option<clase::TipoImportante>),
    /// Apuntar en Tareas las tareas halladas.
    ATareas,
    /// La casilla de un tema de la reunion: marcarlo o desmarcarlo a mano.
    Tema(usize),
    /// Lo que quedo sin tratar: a la proxima reunion, o al Inbox de Tareas.
    TemasProxima,
    TemasATareas,
    /// «Repasar en voz alta» (`super::repaso_oral`).
    Repasar,
}

/// Que se esta viendo en grande: al cambiar, el encuadre vuelve a encajado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Visto {
    Video,
    Captura(usize),
}

/// Donde va cada cosa, en pixeles de la ventana.
#[derive(Debug, Clone, Copy)]
struct Disposicion {
    medio: RectF,
    tiempo: RectF,
    mandos: RectF,
    senales: RectF,
    cabecera_texto: RectF,
    texto: RectF,
    cabecera_fotos: RectF,
    fotos: RectF,
    /// Lo que mide a lo ancho la imagen de una fila de la tira (lo alto va
    /// con su proporcion, ver [`alto_de_miniatura`]).
    ancho_miniatura: f32,
    /// Los mandos, solo con su icono: no caben con sus rotulos.
    compacto: bool,
    /// La escala con que se repartio (1.0 = 96 ppp).
    escala: f32,
}

/// **Como se reparte la ventana**: con la transcripcion o sin ella (su
/// boton, o la T) y con la imagen a pantalla entera (el boton de su
/// esquina, como el de YouTube).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reparto {
    texto: bool,
    entera: bool,
    /// La franja del filtro de lo importante bajo la cabecera del texto.
    franja: bool,
}

impl Default for Reparto {
    fn default() -> Reparto {
        Reparto {
            texto: true,
            entera: false,
            franja: false,
        }
    }
}

fn disponer(ancho: f32, alto: f32, e: f32, reparto: Reparto) -> Disposicion {
    let m = MARGEN * e;
    // Sin barra dentro: la de los visores va fuera, encima de la ventana.
    let arriba = m;
    let abajo = alto - m;
    // Estrecha, la tira y el texto ceden sitio.
    let logico = ancho / e;
    let ancho_fotos = if reparto.entera {
        0.0
    } else {
        (logico * 0.17).clamp(ANCHO_FOTOS_MINIMO, ANCHO_FOTOS) * e
    };
    let x_fotos = ancho - m - ancho_fotos;
    let ancho_texto = if reparto.texto && !reparto.entera {
        (ancho * 0.30).clamp(240.0 * e, 460.0 * e)
    } else {
        0.0
    };
    // Sin columna, sin su hueco.
    let x_texto = if ancho_texto > 0.0 {
        x_fotos - m - ancho_texto
    } else {
        x_fotos
    };
    let derecha = if ancho_fotos > 0.0 {
        x_texto - m
    } else {
        ancho - m
    };
    let ancho_medio = (derecha - m).max(100.0 * e);
    let alto_columna = abajo - arriba;
    // A pantalla entera, la imagen llega hasta los mandos (sin marcadores).
    let bajo_el_medio = if reparto.entera { 92.0 } else { 230.0 } * e;
    let alto_medio = if reparto.entera {
        (alto_columna - bajo_el_medio).max(120.0 * e)
    } else {
        (ancho_medio * 9.0 / 16.0)
            .min(alto_columna - bajo_el_medio)
            .max(120.0 * e)
    };
    let medio = RectF {
        x: m,
        y: arriba,
        ancho: ancho_medio,
        alto: alto_medio,
    };
    let tiempo = RectF {
        x: m,
        y: medio.y + medio.alto + 8.0 * e,
        ancho: ancho_medio,
        alto: 28.0 * e,
    };
    let mandos = RectF {
        x: m,
        y: tiempo.y + tiempo.alto + 4.0 * e,
        ancho: ancho_medio,
        alto: 44.0 * e,
    };
    let senales = RectF {
        x: m,
        y: mandos.y + mandos.alto + 12.0 * e,
        ancho: ancho_medio,
        alto: (abajo - (mandos.y + mandos.alto + 12.0 * e)).max(0.0),
    };
    let cab = |x: f32, w: f32, alto: f32| RectF {
        x,
        y: arriba,
        ancho: w,
        alto: alto * e,
    };
    let lista = |x: f32, w: f32, alto: f32| RectF {
        x,
        y: arriba + alto * e,
        ancho: w,
        alto: (alto_columna - alto * e).max(0.0),
    };
    // Con lo importante hallado, la cabecera del texto lleva debajo la
    // franja de su filtro.
    let cab_texto = CABECERA + if reparto.franja { FRANJA } else { 0.0 };
    Disposicion {
        medio,
        tiempo,
        mandos,
        senales: if reparto.entera {
            RectF {
                alto: 0.0,
                ..senales
            }
        } else {
            senales
        },
        cabecera_texto: cab(x_texto, ancho_texto, cab_texto),
        texto: lista(x_texto, ancho_texto, cab_texto),
        cabecera_fotos: cab(x_fotos, ancho_fotos, CABECERA),
        fotos: lista(x_fotos, ancho_fotos, CABECERA),
        ancho_miniatura: (ancho_fotos - 16.0 * e).max(0.0),
        compacto: ancho_medio < 640.0 * e,
        escala: e,
    }
}

/// **Lo alto de la imagen de una miniatura**, con su proporcion de verdad
/// (checklist 25: «miniaturas de capturas con su proporcion real»): a lo
/// ancho de la tira y, una captura de pie, como mucho tan alta como ancha
/// (el alto es el tope y lo que varia es el ancho). Sin medidas aun, 16:9.
fn alto_de_miniatura(ancho: f32, medidas: Option<(u32, u32)>) -> f32 {
    match medidas {
        Some((w, h)) if w > 0 && h > 0 => (ancho * h as f32 / w as f32).min(ancho),
        _ => ancho * 9.0 / 16.0,
    }
}

/// Lo que mide una fila de la tira con una imagen de `alto_imagen`.
fn alto_de_fila(alto_imagen: f32, e: f32) -> f32 {
    8.0 * e + alto_imagen + 24.0 * e
}

/// Todo lo de la ventana abierta.
struct Vista {
    raiz: PathBuf,
    clase: Clase,
    medio: Medio,
    seguir: bool,
    /// Ensenar la captura del minuto aunque haya video.
    ver_capturas: bool,
    /// Como se reparte: con la transcripcion o sin ella, y la imagen a
    /// pantalla entera (ver [`Reparto`]).
    reparto: Reparto,
    /// Pulso el boton de pantalla entera (lo hace el bucle, que tiene la
    /// ventana).
    pedir_entera: bool,
    scroll_texto: f32,
    scroll_fotos: f32,
    /// El filtro de lo importante: solo los renglones de ese tipo.
    filtro: Option<clase::TipoImportante>,
    /// Hasta cuando no se sigue al sonido: se movio una lista a mano.
    a_mano_hasta: Option<Instant>,
    /// Ir a lo que suena en la siguiente vuelta, aunque este parado (tras
    /// un salto).
    ir_ya: bool,
    /// El marcador que se esta escribiendo: su minuto y su nota.
    escribiendo: Option<(i64, String)>,
    /// La M que abrio el marcador llega tambien como caracter: se traga.
    tragar: Option<char>,
    arrastrando_tiempo: bool,
    /// Arrastrando la barra: el milisegundo bajo el raton, que es lo que se
    /// ensena (`clase::posicion_a_la_vista`), y la ultima busqueda.
    arrastre_ms: Option<i64>,
    ultima_busqueda: Instant,
    /// Donde se pulso la barra (x de pantalla): soltar ahi mismo es un
    /// clic, y se queda en la captura o el marcador al que se pego.
    pulsado_x: i32,
    raton: (f32, f32),
    zonas: Vec<(RectF, Zona)>,
    disp: Disposicion,
    /// Lo alto de cada fila del texto, para el ancho con que se midio.
    altos_texto: RefCell<(u32, Vec<f32>)>,
    /// Como se ve lo de la izquierda (acercado, movido) y que es.
    encuadre: Encuadre,
    visto: Option<Visto>,
    /// Pulsado en lo que se ve: donde iba el raton y si ya se movio (es
    /// llevarlo, no reproducir).
    llevando: Option<((f32, f32), bool)>,
    /// El ultimo clic en lo que se ve, para el doble clic.
    clic_medio: Option<Instant>,
    /// Pulso «Anotar» (lo hace el bucle, que tiene el motor).
    anotar: bool,
    /// Lo anotado encima de la captura que se ve: cual, la fecha de su
    /// lienzo y sus ordenes en pixeles de la captura.
    dibujo: Option<(usize, Option<std::time::SystemTime>, LienzoVisto)>,
    miniaturas: Vec<Option<Imagen>>,
    /// Lo que mide cada captura de verdad (lo anotado va en sus pixeles).
    medidas: Vec<Option<(u32, u32)>>,
    grande: Option<(usize, Imagen)>,
    pedida: Option<usize>,
    /// La captura de la que ya se pidio la imagen entera (al acercarla).
    pedida_entera: Option<usize>,
    cargador: Option<Cargador>,
    aviso: Option<(String, Instant)>,
    /// La ultima vez que se miro si el cuaderno cambio, y como estaba.
    mirado: Instant,
    leido: Option<std::time::SystemTime>,
}

impl Vista {
    fn nueva(recursos: &Recursos, raiz: PathBuf, clase: Clase, ms: i64) -> Vista {
        let medio = Medio::abrir(&recursos.d3d(), &clase, ms);
        let entero = recursos.motor().lado_maximo_bitmap();
        let cargador = Cargador::lanzar(
            clase.capturas.iter().map(|(_, r)| r.clone()).collect(),
            entero.min(2560),
            entero,
        );
        let leido = mtime(&raiz, &clase.proyecto);
        Vista {
            raiz,
            miniaturas: (0..clase.capturas.len()).map(|_| None).collect(),
            medidas: (0..clase.capturas.len()).map(|_| None).collect(),
            clase,
            medio,
            seguir: true,
            ver_capturas: false,
            reparto: Reparto::default(),
            pedir_entera: false,
            scroll_texto: 0.0,
            scroll_fotos: 0.0,
            filtro: None,
            a_mano_hasta: None,
            ir_ya: true,
            escribiendo: None,
            tragar: None,
            arrastrando_tiempo: false,
            arrastre_ms: None,
            ultima_busqueda: Instant::now(),
            pulsado_x: 0,
            raton: (-1.0, -1.0),
            zonas: Vec::new(),
            disp: disponer(ANCHO as f32, ALTO as f32, 1.0, Reparto::default()),
            altos_texto: RefCell::new((0, Vec::new())),
            encuadre: Encuadre::default(),
            visto: None,
            llevando: None,
            clic_medio: None,
            anotar: false,
            dibujo: None,
            grande: None,
            pedida: None,
            pedida_entera: None,
            cargador,
            aviso: None,
            mirado: Instant::now(),
            leido,
        }
    }

    fn duracion(&self) -> i64 {
        let d = self.medio.duracion_ms();
        if d > 0 { d } else { self.clase.duracion_ms }
    }

    fn avisar(&mut self, t: String) {
        self.aviso = Some((t, Instant::now()));
    }

    /// Ir a `ms`, y que el texto y la tira vayan con el.
    fn ir_a(&mut self, ms: i64) {
        self.medio.ir_a(clase::saltar(ms, 0, self.duracion()));
        self.a_mano_hasta = None;
        self.ir_ya = true;
    }

    fn saltar(&mut self, delta: i64) {
        let ms = clase::saltar(self.medio.posicion_ms(), delta, self.duracion());
        self.ir_a(ms);
    }

    /// El trozo anterior o el siguiente al que suena.
    fn otro_trozo(&mut self, adelante: bool) {
        let ms = self.medio.posicion_ms();
        let tiempos: Vec<i64> = self.clase.lineas.iter().filter_map(|(t, _)| *t).collect();
        let destino = if adelante {
            tiempos.iter().find(|t| **t > ms + 200).copied()
        } else {
            // El de antes del que suena: si se acaba de empezar uno, el
            // anterior; si no, el principio de este.
            let actual = tiempos.iter().rposition(|t| *t <= ms);
            match actual {
                Some(i) if ms - tiempos[i] < 1_500 && i > 0 => Some(tiempos[i - 1]),
                Some(i) => Some(tiempos[i]),
                None => None,
            }
        };
        if let Some(t) = destino {
            self.ir_a(t);
        }
    }

    fn alto_vista_texto(&self) -> f32 {
        self.disp.texto.alto
    }

    /// Lo alto de cada fila de la tira: cada captura con su proporcion.
    fn altos_fotos(&self) -> Vec<f32> {
        let e = self.disp.escala;
        (0..self.clase.capturas.len())
            .map(|i| {
                let medidas = self
                    .miniaturas
                    .get(i)
                    .and_then(|m| m.as_ref().map(|(_, w, h)| (*w, *h)))
                    .or_else(|| self.medidas.get(i).copied().flatten());
                alto_de_fila(alto_de_miniatura(self.disp.ancho_miniatura, medidas), e)
            })
            .collect()
    }

    /// Movio una lista con la rueda: la otra la sigue (con «Seguir») y se
    /// deja de seguir al sonido un rato.
    fn rueda(&mut self, en_texto: bool, delta: i32, e: f32) {
        let paso = delta as f32 / 120.0 * 72.0 * e;
        let altos_texto = self.altos_texto.borrow().1.clone();
        let altos_fotos = self.altos_fotos();
        let (vt, vf) = (self.alto_vista_texto(), self.disp.fotos.alto);
        if en_texto {
            self.scroll_texto =
                (self.scroll_texto - paso).clamp(0.0, clase::tope(&altos_texto, vt));
        } else {
            self.scroll_fotos =
                (self.scroll_fotos - paso).clamp(0.0, clase::tope(&altos_fotos, vf));
        }
        self.a_mano_hasta = Some(Instant::now() + A_MANO);
        if !self.seguir {
            return;
        }
        let tf = self.clase.tiempos_capturas();
        if en_texto {
            if let Some(s) = clase::seguir(
                &self.clase.tiempos_filas,
                &altos_texto,
                vt,
                self.scroll_texto,
                &tf,
                &altos_fotos,
                vf,
            ) {
                self.scroll_fotos = s;
            }
        } else if let Some(s) = clase::seguir(
            &tf,
            &altos_fotos,
            vf,
            self.scroll_fotos,
            &self.clase.tiempos_filas,
            &altos_texto,
            vt,
        ) {
            self.scroll_texto = s;
        }
    }

    /// Lo de cada vuelta con «Seguir»: el texto y la tira se deslizan hasta
    /// lo que suena (mientras suena, o justo despues de un salto).
    fn seguir_al_sonido(&mut self) -> bool {
        if self.a_mano_hasta.is_some_and(|h| Instant::now() < h) {
            return false;
        }
        self.a_mano_hasta = None;
        if !self.seguir {
            self.ir_ya = false;
            return false;
        }
        if !self.ir_ya && !self.medio.sonando() {
            return false;
        }
        let ms = self.medio.posicion_ms();
        let altos_texto = self.altos_texto.borrow().1.clone();
        let altos_fotos = self.altos_fotos();
        let dt = clase::scroll_para(
            &self.clase.tiempos_filas,
            &altos_texto,
            self.alto_vista_texto(),
            ms,
            false,
        );
        let df = clase::scroll_para(
            &self.clase.tiempos_capturas(),
            &altos_fotos,
            self.disp.fotos.alto,
            ms,
            false,
        );
        let (t, f) = (
            clase::acercar(self.scroll_texto, dt, 0.25),
            clase::acercar(self.scroll_fotos, df, 0.25),
        );
        let cambio = t != self.scroll_texto || f != self.scroll_fotos;
        self.scroll_texto = t;
        self.scroll_fotos = f;
        if !cambio {
            self.ir_ya = false;
        }
        cambio
    }

    fn empezar_marcador(&mut self) {
        let ms = self.medio.posicion_ms();
        // Al escribir, el sonido se para: se esta tomando una nota.
        if self.medio.sonando() {
            self.medio.alternar();
        }
        let ya = self
            .clase
            .senales
            .iter()
            .find(|s| s.ms == ms && !s.banderita && s.clip.is_none())
            .map(|s| s.texto.clone())
            .unwrap_or_default();
        self.escribiendo = Some((ms, ya));
    }

    fn guardar_marcador(&mut self, textos: &Catalogo) {
        let Some((ms, texto)) = self.escribiendo.take() else {
            return;
        };
        let hecho = cambiar_la_nota(&self.raiz, &self.clase.proyecto, &self.clase.id, |m| {
            clase::poner_marcador(&mut m.resto, ms, &texto);
            true
        });
        match hecho {
            Ok(m) => {
                self.clase.poner_senales(senales_de(&m));
                self.leido = mtime(&self.raiz, &self.clase.proyecto);
                self.avisar(textos.t("estudiar-marcador-guardado"));
            }
            Err(e) => {
                tracing::warn!(?e, "el marcador de la clase no se guardo");
                self.avisar(textos.t("estudiar-no-se-guardo"));
            }
        }
    }

    fn quitar_senal(&mut self, ms: i64, textos: &Catalogo) {
        // Un clip no se quita desde aqui: es de lo grabado.
        if self
            .clase
            .senales
            .iter()
            .all(|s| s.ms != ms || s.clip.is_some())
        {
            return;
        }
        let banderita = self.clase.senales.iter().any(|s| s.ms == ms && s.banderita);
        let hecho = cambiar_la_nota(&self.raiz, &self.clase.proyecto, &self.clase.id, |m| {
            if banderita {
                crate::voz::quitar_marca(m, ms)
            } else {
                clase::quitar_marcador(&mut m.resto, ms)
            }
        });
        match hecho {
            Ok(m) => {
                self.clase.poner_senales(senales_de(&m));
                self.leido = mtime(&self.raiz, &self.clase.proyecto);
            }
            Err(e) => {
                tracing::warn!(?e, "la senal de la clase no se quito");
                self.avisar(textos.t("estudiar-no-se-guardo"));
            }
        }
    }

    /// Si el cuaderno cambio por fuera (llego el texto, un marcador del
    /// movil), se relee lo que no suena: el texto, las senales y las
    /// capturas nuevas.
    fn releer_si_cambio(&mut self, motor: &MotorRender) -> bool {
        if self.mirado.elapsed() < Duration::from_millis(1500) || self.escribiendo.is_some() {
            return false;
        }
        self.mirado = Instant::now();
        let ahora = mtime(&self.raiz, &self.clase.proyecto);
        if ahora == self.leido {
            return false;
        }
        self.leido = ahora;
        let Ok(nueva) = Clase::leer(&self.raiz, &self.clase.proyecto, &self.clase.id) else {
            return false;
        };
        let otras_capturas = nueva.capturas != self.clase.capturas;
        self.clase.lineas = nueva.lineas;
        self.clase.importantes = nueva.importantes;
        self.clase.temas = nueva.temas;
        self.clase.nombre = nueva.nombre;
        self.clase.clips = nueva.clips;
        self.clase.poner_senales(nueva.senales);
        self.clase.fotos = nueva.fotos;
        if otras_capturas {
            self.clase.capturas = nueva.capturas;
            self.miniaturas = (0..self.clase.capturas.len()).map(|_| None).collect();
            self.medidas = (0..self.clase.capturas.len()).map(|_| None).collect();
            self.dibujo = None;
            self.grande = None;
            self.pedida = None;
            self.cargador = Cargador::lanzar(
                self.clase.capturas.iter().map(|(_, r)| r.clone()).collect(),
                motor.lado_maximo_bitmap().min(2560),
                motor.lado_maximo_bitmap(),
            );
            self.pedida_entera = None;
        }
        *self.altos_texto.borrow_mut() = (0, Vec::new());
        true
    }

    /// Las imagenes que llegaron de su hilo, y pedir la grande que toca.
    fn imagenes(&mut self, motor: &MotorRender) -> bool {
        let mut cambio = false;
        if let Some(c) = &self.cargador {
            for llegada in c.llegan.try_iter() {
                match llegada {
                    Cargada::Miniatura(i, img, medida) => {
                        if let Some(sitio) = self.miniaturas.get_mut(i) {
                            *sitio = subir(motor, &img);
                        }
                        if let Some(sitio) = self.medidas.get_mut(i) {
                            *sitio = Some(medida);
                        }
                    }
                    Cargada::Grande(i, img, medida) => {
                        if let Some(sitio) = self.medidas.get_mut(i) {
                            *sitio = Some(medida);
                        }
                        if let Some(b) = subir(motor, &img) {
                            self.grande = Some((i, b));
                        }
                        if self.pedida == Some(i) {
                            self.pedida = None;
                        }
                    }
                }
                cambio = true;
            }
        }
        if let Some(i) = self.captura_a_la_vista()
            && self.grande.as_ref().is_none_or(|(g, _)| *g != i)
            && self.pedida != Some(i)
            && let Some(c) = &self.cargador
            && c.pedir.send((i, false)).is_ok()
        {
            self.pedida = Some(i);
            self.pedida_entera = None;
        }
        // Acercada mas alla de lo cargado: la entera, una vez por captura.
        if let Some(i) = self.captura_a_la_vista()
            && let Some((g, (_, w, h))) = &self.grande
            && *g == i
            && self.pedida_entera != Some(i)
            && let Some(Some((de_verdad, _))) = self.medidas.get(i)
        {
            let pintada = self
                .encuadre
                .destino(caja_de(self.disp.medio), *w as f32, *h as f32);
            if pixpin_proyecto::presencial::hace_falta_la_entera(pintada.2, *w, *de_verdad)
                && let Some(c) = &self.cargador
                && c.pedir.send((i, true)).is_ok()
            {
                self.pedida_entera = Some(i);
            }
        }
        cambio
    }

    /// La captura que se ensena en grande, si se ensena alguna.
    fn captura_a_la_vista(&self) -> Option<usize> {
        if self.medio.es_video() && !self.ver_capturas {
            return None;
        }
        clase::ultimo_antes(&self.clase.tiempos_capturas(), self.medio.posicion_ms())
    }

    /// Lo que se ve en grande ahora y lo que mide su imagen (la que se
    /// pinta: la grande, o su miniatura mientras llega).
    fn lo_visto(&mut self) -> Option<(Visto, u32, u32)> {
        if let Some(i) = self.captura_a_la_vista() {
            let img = match &self.grande {
                Some((g, img)) if *g == i => Some(img),
                _ => self.miniaturas.get(i).and_then(|m| m.as_ref()),
            };
            return img.map(|(_, w, h)| (Visto::Captura(i), *w, *h));
        }
        match &mut self.medio {
            Medio::Video {
                bitmap: Some(_), r, ..
            } => r.dimensiones().map(|(w, h)| (Visto::Video, w, h)),
            _ => None,
        }
    }

    /// Si cambio lo que se ve (otra captura, del video a la captura), el
    /// encuadre vuelve a encajado: acercar una no es acercar la otra.
    fn al_dia_lo_visto(&mut self) {
        let ahora = self.lo_visto().map(|(v, _, _)| v);
        if ahora.is_some() && ahora != self.visto {
            self.visto = ahora;
            self.encuadre.encajar();
            self.llevando = None;
        }
    }

    /// Acerca (o aleja) lo que se ve hacia `punto`.
    fn acercar(&mut self, factor: f32, punto: Option<(f32, f32)>) {
        let Some((_, w, h)) = self.lo_visto() else {
            return;
        };
        let c = caja_de(self.disp.medio);
        let punto = punto.unwrap_or((c.0 + c.2 / 2.0, c.1 + c.3 / 2.0));
        self.encuadre.acercar(c, w as f32, h as f32, factor, punto);
    }

    /// **Lo anotado encima de la captura que se ve**, al dia: se relee si
    /// su lienzo cambio (se anoto en el lienzo, llego del movil). Devuelve
    /// si hay que repintar.
    fn releer_dibujo(&mut self) -> bool {
        let Some(i) = self.captura_a_la_vista() else {
            return std::mem::take(&mut self.dibujo).is_some();
        };
        let (Some(m), Some(Some((w, h)))) = (self.clase.fotos.get(i), self.medidas.get(i)) else {
            return false;
        };
        let lienzo = crate::foto_anotada::lienzo_de_la_foto(&self.raiz, &self.clase.proyecto, m);
        let fecha = lienzo
            .as_ref()
            .and_then(|(_, r)| std::fs::metadata(r).and_then(|d| d.modified()).ok());
        if self
            .dibujo
            .as_ref()
            .is_some_and(|(j, f, _)| *j == i && *f == fecha)
        {
            return false;
        }
        let ordenes = crate::foto_anotada::dibujo_de_la_foto(
            &self.raiz,
            &self.clase.proyecto,
            m,
            (*w as f32, *h as f32),
        )
        .map(|d| d.ordenes)
        .unwrap_or_default();
        self.dibujo = Some((
            i,
            fecha,
            LienzoVisto {
                ordenes,
                grafitos: Vec::new(),
                caja: (0.0, 0.0, *w as f32, *h as f32),
                fondo: None,
                papel: None,
                giro_del_fondo: 0.0,
            },
        ));
        true
    }

    fn zona_en(&self, p: (f32, f32)) -> Option<Zona> {
        self.zonas
            .iter()
            .rev()
            .find(|(r, _)| dentro(*r, p))
            .map(|(_, z)| *z)
    }
}

fn mtime(raiz: &Path, proyecto: &str) -> Option<std::time::SystemTime> {
    std::fs::metadata(almacen::carpeta(raiz, proyecto).join("guardados.jsonl"))
        .and_then(|m| m.modified())
        .ok()
}

/// La escala del monitor donde esta `area` (su centro).
fn escala_de(area: Rect) -> f32 {
    let centro = Punto {
        x: area.x + area.ancho as i32 / 2,
        y: area.y + area.alto as i32 / 2,
    };
    pixpin_capture::enumerar_monitores()
        .ok()
        .and_then(|d| {
            d.monitor_en(centro)
                .or_else(|| d.principal())
                .map(|m| m.escala_por_cien)
        })
        .map_or(1.0, |e| e as f32 / 100.0)
}

fn bucle(recursos: &Recursos, textos: &Catalogo, pedido: &mut Pedido) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .monitor_en(pixpin_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien;
    let mut e = escala as f32 / 100.0;
    let raiz = pedido.ubicacion.raiz().to_path_buf();
    let clase = Clase::leer(&raiz, &pedido.proyecto, &pedido.id)?;
    let mut v = Vista::nueva(recursos, raiz, clase, pedido.ms);
    // Una clase pasada a texto antes de Jev (o con la clave puesta despues):
    // se busca lo importante ahora, en su hilo (sin clave, nada).
    crate::jev::reunion::lanzar(&pedido.ubicacion, &v.clase.proyecto, &v.clase.id, false);
    // Un visor como los demas: sin marco, se estira por los bordes y lleva
    // su barra fuera, encima (`lector_pin`).
    let mut pin = PinLector::nuevo(
        crate::ventanita::centrado(monitor.area_trabajo, ANCHO, ALTO, escala),
        e,
    )
    .con_minimo(
        ((ANCHO_MINIMO * e) as u32).min(monitor.area_trabajo.ancho),
        ((ALTO_MINIMO * e) as u32).min(monitor.area_trabajo.alto),
    );
    let mut area = pin.area;
    let mut ventana = VentanaOverlay::nueva_normal(area, &textos.t("estudiar-titulo"))
        .context("no se pudo abrir la ventana de estudiar la clase")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        area.ancho,
        area.alto,
    )
    .context("sin superficie para estudiar la clase")?;
    ventana.mostrar();
    ventana.enfocar();
    pin.al_nacer(&ventana);
    pin.crear_barra(&motor, &recursos.d3d(), &ventana);
    ABIERTA.store(ventana.handle().0 as isize, Ordering::SeqCst);
    tracing::info!(
        capturas = v.clase.capturas.len(),
        trozos = v.clase.lineas.len(),
        con_video = v.medio.es_video(),
        "ventana de estudiar la clase abierta"
    );
    v.disp = disponer(area.ancho as f32, area.alto as f32, e, v.reparto);

    let mut vivo = true;
    let mut pintar = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        // Lo que la ventana ocupa DE VERDAD, antes de repartir ningun clic:
        // movida o estirada por fuera (AltSnap, Win+flecha, otro monitor),
        // la esquina vieja corria todos los clics esa diferencia.
        pin.seguir_ventana(&mut ventana);
        if pin.area != area {
            if (pin.area.ancho, pin.area.alto) != (area.ancho, area.alto) {
                let _ = superficie.redimensionar(pin.area.ancho, pin.area.alto);
                *v.altos_texto.borrow_mut() = (0, Vec::new());
            }
            area = pin.area;
            v.disp = disponer(area.ancho as f32, area.alto as f32, e, v.reparto);
            pintar = true;
        }
        // Otra clase pedida con la ventana abierta.
        let otra = OTRA.lock().ok().and_then(|mut g| g.take());
        if let Some(o) = otra {
            match Clase::leer(&v.raiz, &o.proyecto, &o.id) {
                Ok(c) => {
                    let (disp, reparto) = (v.disp, v.reparto);
                    v = Vista::nueva(recursos, v.raiz.clone(), c, o.ms);
                    v.disp = disp;
                    v.reparto = reparto;
                    crate::jev::reunion::lanzar(
                        &o.ubicacion,
                        &v.clase.proyecto,
                        &v.clase.id,
                        false,
                    );
                    *pedido = o;
                }
                Err(e) => tracing::warn!(?e, "la otra clase no se pudo leer"),
            }
            pintar = true;
        }
        for (hw, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            // La barra de los visores: llevar la ventana, la luna/sol,
            // pantalla completa, el pin y cerrar.
            if pin.es_de_la_barra(hw) {
                let (accion, _) = pin.evento_barra(&ev, &mut ventana);
                match accion {
                    Some(AccionPin::PantallaCompleta) => {
                        pin.alternar_pantalla_completa(&mut ventana)
                    }
                    Some(AccionPin::Fijar) => pin.alternar_fijado(&ventana),
                    Some(AccionPin::Cerrar) => vivo = false,
                    _ => {}
                }
                pintar = true;
                continue;
            }
            if hw != ventana.handle() {
                continue;
            }
            // Estirar por los bordes es del pin.
            if pin.evento_ventana(&ev, &mut ventana).consumido {
                pintar = true;
                continue;
            }
            // Siempre desde la esquina de ahora (el pin la sigue).
            let esquina = (pin.area.x, pin.area.y);
            let local = |p: Punto| encuadre::a_la_ventana((p.x, p.y), esquina);
            let ms_bajo = |p: Punto, v: &Vista| {
                let t = v.disp.tiempo;
                clase::ms_del_raton(p.x, esquina.0, t.x, t.ancho, v.duracion())
            };
            match ev {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::CambioDpi => {
                    let nueva = escala_de(pin.area);
                    if (nueva - e).abs() > f32::EPSILON {
                        e = nueva;
                        pin.poner_escala(e);
                        v.disp = disponer(area.ancho as f32, area.alto as f32, e, v.reparto);
                        *v.altos_texto.borrow_mut() = (0, Vec::new());
                    }
                    pintar = true;
                }
                EventoOverlay::RatonMovido(p) => {
                    let l = local(p);
                    if v.arrastrando_tiempo && (p.x - v.pulsado_x).abs() > 2 {
                        let ms = ms_bajo(p, &v);
                        v.arrastre_ms = Some(ms);
                        if v.ultima_busqueda.elapsed() >= BUSCAR_CADA {
                            v.ultima_busqueda = Instant::now();
                            v.ir_a(ms);
                        }
                        pintar = true;
                    }
                    if let Some((antes, movido)) = v.llevando {
                        let (dx, dy) = (l.0 - antes.0, l.1 - antes.1);
                        if movido || dx.abs().max(dy.abs()) >= ARRASTRE * e {
                            if let Some((_, w, h)) = v.lo_visto() {
                                let c = caja_de(v.disp.medio);
                                v.encuadre.mover(c, w as f32, h as f32, dx, dy);
                            }
                            v.llevando = Some((l, true));
                            pintar = true;
                        }
                    }
                    if v.zona_en(l) != v.zona_en(v.raton) {
                        pintar = true;
                    }
                    v.raton = l;
                }
                EventoOverlay::BotonPulsado(p) => {
                    let l = local(p);
                    v.raton = l;
                    pintar = true;
                    match v.zona_en(l) {
                        Some(Zona::Tiempo) => {
                            let t = v.disp.tiempo;
                            let senales: Vec<i64> = v
                                .clase
                                .capturas
                                .iter()
                                .map(|(ms, _)| *ms)
                                .chain(v.clase.senales.iter().map(|s| s.ms))
                                .collect();
                            let ms = clase::pegar_a_senal(
                                l.0 - t.x,
                                t.ancho,
                                v.duracion(),
                                &senales,
                                PEGAR * e,
                            );
                            v.ir_a(ms);
                            v.arrastre_ms = Some(ms);
                            v.pulsado_x = p.x;
                            v.ultima_busqueda = Instant::now();
                            v.arrastrando_tiempo = true;
                            ventana.capturar_raton();
                        }
                        // Lo que se ve: llevarlo si se arrastra; un clic
                        // reproduce o para, y dos seguidos lo encajan.
                        Some(Zona::Medio) => {
                            v.llevando = Some((l, false));
                            ventana.capturar_raton();
                        }
                        Some(z) => pulsar(&mut v, z, textos, &mut vivo),
                        None => {}
                    }
                }
                EventoOverlay::BotonSoltado(p) => {
                    if let Some((_, movido)) = v.llevando.take() {
                        ventana.soltar_raton();
                        if !movido {
                            let doble = v.clic_medio.is_some_and(|t| t.elapsed() < DOBLE_CLIC);
                            if doble {
                                // El primer clic ya reprodujo o paro: se
                                // deshace, el doble clic solo encaja.
                                v.clic_medio = None;
                                v.medio.alternar();
                                v.encuadre.encajar();
                            } else {
                                v.clic_medio = Some(Instant::now());
                                v.medio.alternar();
                            }
                        }
                        pintar = true;
                    }
                    if v.arrastrando_tiempo {
                        v.arrastrando_tiempo = false;
                        ventana.soltar_raton();
                        // Al soltar, el fotograma exacto, si se arrastro;
                        // un clic quieto se queda donde se pego.
                        let ms = match v.arrastre_ms {
                            Some(ms) if (p.x - v.pulsado_x).abs() <= 2 => ms,
                            _ => ms_bajo(p, &v),
                        };
                        v.ir_a(ms);
                        v.arrastre_ms = None;
                        pintar = true;
                    }
                }
                EventoOverlay::BotonDerechoPulsado(p) => {
                    // Clic derecho en una senal: quitarla (como mantener
                    // pulsada una banderita en el movil).
                    let l = local(p);
                    if let Some(Zona::Ir(ms)) = v.zona_en(l)
                        && v.clase.senales.iter().any(|s| s.ms == ms)
                    {
                        v.quitar_senal(ms, textos);
                        pintar = true;
                    }
                }
                EventoOverlay::Rueda(delta) => match v.zona_en(v.raton) {
                    Some(Zona::ListaFotos | Zona::Captura(_)) => {
                        v.rueda(false, delta, e);
                        pintar = true;
                    }
                    // Encima de lo que se ve, la rueda acerca hacia el raton.
                    Some(_) if dentro(v.disp.medio, v.raton) => {
                        if let Some((_, w, h)) = v.lo_visto() {
                            let c = caja_de(v.disp.medio);
                            let raton = v.raton;
                            v.encuadre.rueda(c, w as f32, h as f32, delta, raton);
                            pintar = true;
                        }
                    }
                    Some(_) if dentro(v.disp.texto, v.raton) => {
                        v.rueda(true, delta, e);
                        pintar = true;
                    }
                    _ => {}
                },
                EventoOverlay::Tecla { vk, ctrl, .. } => {
                    pintar = true;
                    // F11 y Escape, como en los visores: Escape sale de
                    // pantalla completa y, si no, cierra.
                    let del_visor = if v.escribiendo.is_some() || ctrl {
                        None
                    } else if vk == VK_ESCAPE && !v.encuadre.encajado() {
                        // Acercado, Escape primero lo encaja.
                        v.encuadre.encajar();
                        continue;
                    } else {
                        crate::lector_pin::tecla(vk, pin.pantalla_completa)
                    };
                    match del_visor {
                        Some(true) => vivo = false,
                        Some(false) => pin.alternar_pantalla_completa(&mut ventana),
                        None => tecla(&mut v, vk, ctrl, textos, &mut vivo),
                    }
                }
                EventoOverlay::Caracter(c) => {
                    if v.tragar.take().is_some_and(|t| t.eq_ignore_ascii_case(&c)) {
                        continue;
                    }
                    if let Some((_, texto)) = v.escribiendo.as_mut()
                        && !c.is_control()
                        && texto.chars().count() < clase::NOTA_MAXIMA
                    {
                        texto.push(c);
                        pintar = true;
                    }
                }
                _ => {}
            }
        }
        if !vivo {
            break;
        }
        // **La imagen a pantalla entera** (el boton de su esquina): la
        // ventana a pantalla completa y la imagen ocupandola; otra vez,
        // reducir. Saliendo de pantalla completa por la barra, F11 o Esc,
        // la imagen vuelve a su sitio.
        if std::mem::take(&mut v.pedir_entera) {
            v.reparto.entera = !v.reparto.entera;
            if v.reparto.entera != pin.pantalla_completa {
                pin.alternar_pantalla_completa(&mut ventana);
            }
            pintar = true;
        }
        if v.reparto.entera && !pin.pantalla_completa {
            v.reparto.entera = false;
            pintar = true;
        }
        // «Anotar»: la captura que se ve, o el fotograma del video.
        if std::mem::take(&mut v.anotar) {
            anotar_lo_visto(&mut v, &motor, &recursos.d3d(), textos);
            pintar = true;
        }
        // El video que no se pudo decodificar: se oye la nota.
        if v.medio.es_video() && v.medio.fallo() {
            tracing::warn!("el video de la clase no se puede ver aqui; va el audio");
            let ms = v.medio.posicion_ms();
            v.medio = Medio::audio(&v.clase, ms);
            v.avisar(textos.t("estudiar-sin-video"));
            pintar = true;
        }
        if v.medio.latido(&motor) {
            pintar = true;
        }
        if v.imagenes(&motor) {
            pintar = true;
        }
        let toca_mirar = v.mirado.elapsed() >= Duration::from_millis(1500);
        if v.releer_si_cambio(&motor) {
            pintar = true;
        }
        v.al_dia_lo_visto();
        // Lo anotado de la captura que se ve: al cambiar de captura y, de
        // vez en cuando, por si se anoto en su lienzo o llego del movil.
        let otra_captura = match v.captura_a_la_vista() {
            Some(i) => v.dibujo.as_ref().is_none_or(|(j, _, _)| *j != i),
            None => v.dibujo.is_some(),
        };
        if (toca_mirar || otra_captura) && v.releer_dibujo() {
            pintar = true;
        }
        if v.seguir_al_sonido() {
            pintar = true;
        }
        if v.aviso
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() >= AVISO_DURA)
        {
            v.aviso = None;
            pintar = true;
        }
        // La barra de los visores; si su luna/sol (o la de otro visor)
        // cambio el modo, se repinta.
        let nombre = v.clase.nombre.clone();
        if pin.actualizar(&motor, &nombre, &[]) {
            pintar = true;
        }
        if pintar {
            pintar = false;
            let tema = if crate::oscuro::activo() {
                &OSCURO
            } else {
                &CLARO
            };
            v.reparto.franja = v.clase.con_franja();
            v.disp = disponer(area.ancho as f32, area.alto as f32, e, v.reparto);
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| {
                    pintar_todo(
                        p,
                        &mut v,
                        area.ancho as f32,
                        area.alto as f32,
                        tema,
                        e,
                        textos,
                    )
                });
                let _ = superficie.presentar();
            }
        }
        pedido.ms = v.medio.posicion_ms();
        let latido = if v.medio.sonando() || v.ir_ya || v.arrastrando_tiempo || v.llevando.is_some()
        {
            16
        } else if v.pedida.is_some() || v.aviso.is_some() || pin.quiere_reloj() {
            50
        } else {
            400
        };
        pixpin_shell::overlay::esperar_eventos(Some(latido));
    }
    ventana.ocultar();
    tracing::info!("ventana de estudiar la clase cerrada");
    Ok(())
}

/// **Anota lo que se ve**: la captura en su lienzo y, con el video, el
/// fotograma de ahora guardado antes como una captura mas de la clase.
fn anotar_lo_visto(
    v: &mut Vista,
    motor: &MotorRender,
    d3d: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
    textos: &Catalogo,
) {
    let id = match v.captura_a_la_vista() {
        Some(i) => v.clase.fotos.get(i).map(|m| m.id.clone()),
        None if v.medio.es_video() => {
            // Se para: se anota lo que se esta viendo, no lo que viene.
            if v.medio.sonando() {
                v.medio.alternar();
            }
            match guardar_fotograma(v, motor, d3d) {
                Ok(id) => {
                    v.avisar(textos.t("estudiar-fotograma-guardado"));
                    // Que se vea la captura nueva (se relee en la siguiente
                    // vuelta) y no el video.
                    v.ver_capturas = true;
                    v.leido = None;
                    v.mirado = Instant::now()
                        .checked_sub(Duration::from_secs(2))
                        .unwrap_or_else(Instant::now);
                    Some(id)
                }
                Err(e) => {
                    tracing::warn!(?e, "el fotograma de la clase no se guardo");
                    v.avisar(textos.t("estudiar-no-se-guardo"));
                    return;
                }
            }
        }
        None => None,
    };
    let Some(id) = id else {
        v.avisar(textos.t("estudiar-nada-que-anotar"));
        return;
    };
    if let Err(e) = anotar_foto(&v.raiz, &v.clase.proyecto, &id) {
        tracing::warn!(?e, "no se pudo abrir la captura para anotar");
        v.avisar(textos.t("estudiar-no-se-guardo"));
    }
}

/// **Abre la foto `id` del chat en SU lienzo**, como al pulsarla en el
/// chat: el `foto-<id>` que viaja con ella (K11, `crate::foto_anotada`),
/// con todas las herramientas del lienzo. Va en su hilo
/// (`lienzo_aparte`) y la ventana de estudiar sigue sonando.
fn anotar_foto(raiz: &Path, proyecto: &str, id: &str) -> Result<()> {
    let carpeta = almacen::carpeta(raiz, proyecto);
    let mut mensajes = cuaderno::Cuaderno::leer_de(&carpeta)
        .context("sin cuaderno")?
        .mensajes;
    let indice = mensajes
        .iter()
        .position(|m| m.id == id)
        .context("la captura ya no esta en el chat")?;
    crate::foto_anotada::asegurar_lienzo(raiz, proyecto, &mut mensajes, indice);
    let plana = mensajes
        .get(indice)
        .and_then(|m| super::ruta_del_mensaje(raiz, proyecto, m));
    let opciones = crate::notas_md::paginas_vivas::opciones_del_lienzo();
    let clave = super::clave_de_hoja(proyecto, &mensajes, indice);
    let (raiz, proyecto) = (raiz.to_path_buf(), proyecto.to_string());
    super::lienzo_aparte(clave, move || {
        if !crate::foto_anotada::abrir_si_tiene_lienzo(
            &raiz, &proyecto, &mensajes, indice, opciones,
        ) && let Some(ruta) = plana
        {
            super::abrir_foto_en_lienzo(&ruta, opciones);
        }
    });
    Ok(())
}

/// **Guarda el fotograma que se ve** como una captura mas de la clase: una
/// imagen en el chat, enlazada a la nota en su minuto (`enGrabacion`) y en
/// la lista de capturas de la nota (`clase::poner_captura`). Devuelve su id.
fn guardar_fotograma(
    v: &mut Vista,
    motor: &MotorRender,
    d3d: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
) -> Result<String> {
    let ms = v.medio.posicion_ms();
    let Medio::Video {
        bitmap: Some(b), r, ..
    } = &mut v.medio
    else {
        anyhow::bail!("sin fotograma");
    };
    let (w, h) = r.dimensiones().context("el video aun no tiene tamano")?;
    let lienzo = pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(motor, d3d, w, h)
        .context("sin lienzo para el fotograma")?;
    motor
        .dibujar(&lienzo.destino, |p| {
            p.limpiar(Color::NEGRO);
            p.bitmap(
                b,
                RectF {
                    x: 0.0,
                    y: 0.0,
                    ancho: w as f32,
                    alto: h as f32,
                },
                None,
                false,
            );
        })
        .context("no se pudo pintar el fotograma")?;
    let (ancho, alto, mut pixeles) = lienzo.leer_rgba().context("no se pudo leer el fotograma")?;
    // Un video no tiene transparencia.
    for a in pixeles.iter_mut().skip(3).step_by(4) {
        *a = 255;
    }
    let png = pixpin_codec::imagen::codificar_png(&ImagenRgba {
        ancho,
        alto,
        pixeles,
    })
    .context("no se pudo codificar el fotograma")?;
    let (raiz, proyecto) = (v.raiz.clone(), v.clase.proyecto.clone());
    let numero = super::siguiente_numero_en(&raiz, &proyecto)?;
    let aparato = crate::letras_que_viajan::aparato(&raiz);
    let fichero = format!("clase-{}-{ms}.png", pixpin_shell::entorno::ahora_utc_ms());
    let pie = format!("⏱ {} · {}", pixpin_voz::marca_de_tiempo(ms), v.clase.nombre);
    let mut m = super::adjuntar_con_pie(
        &raiz, &proyecto, &aparato, &fichero, &png, numero, &pie, None,
    )?;
    m.resto.insert(
        "enGrabacion".into(),
        serde_json::json!({ "de": v.clase.id, "ms": ms }),
    );
    cuaderno::reemplazar(&almacen::carpeta(&raiz, &proyecto), &m)?;
    let id = m.id.clone();
    cambiar_la_nota(&raiz, &proyecto, &v.clase.id, |n| {
        clase::poner_captura(&mut n.resto, ms, &id);
        true
    })?;
    tracing::info!(ms, "fotograma de la clase guardado como captura");
    Ok(m.id)
}

fn pulsar(v: &mut Vista, z: Zona, textos: &Catalogo, vivo: &mut bool) {
    match z {
        Zona::Tocar | Zona::Medio => v.medio.alternar(),
        Zona::Atras => v.saltar(-SALTO_LARGO),
        Zona::Adelante => v.saltar(SALTO_LARGO),
        Zona::Velocidad => v.medio.otra_velocidad(),
        Zona::Marcar => {
            if v.escribiendo.is_some() {
                v.guardar_marcador(textos);
            } else {
                v.empezar_marcador();
            }
        }
        Zona::Seguir => {
            v.seguir = !v.seguir;
            if v.seguir {
                v.a_mano_hasta = None;
                v.ir_ya = true;
            }
        }
        Zona::CambiarVista => v.ver_capturas = !v.ver_capturas,
        Zona::Ir(ms) => v.ir_a(ms),
        Zona::Quitar(ms) => v.quitar_senal(ms, textos),
        Zona::Clip(i) => {
            if let Some(s) = v.clase.senales.get(i).cloned() {
                v.ir_a(s.ms);
                if let Some((ruta, _, _)) = &s.clip {
                    // Un clip HEVC sin «HEVC Video Extensions»: se dice que
                    // falta y se abre igual (el fichero no se pierde).
                    if pixpin_pin::video::examinar(ruta)
                        .and_then(|f| f.extension_que_falta())
                        .is_some()
                    {
                        v.avisar(textos.t("estudiar-sin-hevc"));
                    }
                    if let Err(e) = pixpin_shell::abrir(ruta) {
                        tracing::warn!(?e, "no se pudo abrir el clip de la clase");
                    }
                }
            }
        }
        Zona::Captura(i) => {
            if let Some((ms, _)) = v.clase.capturas.get(i) {
                let ms = *ms;
                v.ver_capturas = true;
                v.ir_a(ms);
            }
        }
        Zona::ZoomMenos => v.acercar(1.0 / encuadre::PASO, None),
        Zona::ZoomMas => v.acercar(encuadre::PASO, None),
        Zona::Encajar => v.encuadre.encajar(),
        Zona::Anotar => v.anotar = true,
        Zona::Transcripcion => v.reparto.texto = !v.reparto.texto,
        Zona::PantallaEntera => v.pedir_entera = true,
        Zona::Filtro(f) => {
            v.filtro = if v.filtro == f { None } else { f };
            v.scroll_texto = 0.0;
            *v.altos_texto.borrow_mut() = (0, Vec::new());
        }
        Zona::ATareas => {
            match crate::jev::reunion::a_tareas(&v.raiz, &v.clase.proyecto, &v.clase.id) {
                Ok(0) => {}
                Ok(n) => {
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("n", n as i64);
                    v.avisar(textos.t_args("clase-importante-anadidas", &args));
                    // Se relee ya: la chapita «+N» se va.
                    v.leido = None;
                    v.mirado = Instant::now() - Duration::from_secs(2);
                }
                Err(e) => {
                    tracing::warn!("jev: no se apuntaron las tareas de la clase: {e}");
                    v.avisar(textos.t("chat-no-se-pudo"));
                }
            }
        }
        Zona::Tema(i) => {
            if let Some(t) = v.clase.temas.get(i).cloned() {
                match crate::jev::temas::cambiar_a_mano(
                    &v.raiz,
                    &v.clase.proyecto,
                    &v.clase.id,
                    &t.id,
                    !t.hecho,
                ) {
                    Ok(_) => {
                        agenda::a_mano(&mut v.clase.temas, &t.id, !t.hecho, None);
                    }
                    Err(e) => {
                        tracing::warn!("temas: no se pudo marcar a mano: {e}");
                        v.avisar(textos.t("chat-no-se-pudo"));
                    }
                }
            }
        }
        Zona::TemasProxima => {
            let sin: Vec<String> = agenda::sin_tratar(&v.clase.temas)
                .into_iter()
                .map(|t| t.texto.clone())
                .collect();
            match crate::jev::temas::guardar_para_la_proxima(&v.raiz, &sin) {
                Ok(n) => {
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("n", n as i64);
                    v.avisar(textos.t_args("clase-temas-pasados", &args));
                }
                Err(e) => {
                    tracing::warn!(?e, "temas: no se guardaron para la proxima reunion");
                    v.avisar(textos.t("chat-no-se-pudo"));
                }
            }
        }
        Zona::TemasATareas => {
            let sin: Vec<String> = agenda::sin_tratar(&v.clase.temas)
                .into_iter()
                .map(|t| t.texto.clone())
                .collect();
            let n = crate::jev::temas::a_tareas(&v.raiz, &sin, &v.clase.nombre);
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("n", n as i64);
            v.avisar(textos.t_args("clase-temas-anadidos", &args));
        }
        Zona::Repasar => super::repaso_oral::abrir(&v.raiz, &v.clase.proyecto, &v.clase.id),
        Zona::Tiempo | Zona::ListaTexto | Zona::ListaFotos => {}
    }
    let _ = vivo;
}

fn tecla(v: &mut Vista, vk: u32, ctrl: bool, textos: &Catalogo, vivo: &mut bool) {
    // Escribiendo la nota de un marcador, las letras son letras.
    if v.escribiendo.is_some() {
        match vk {
            VK_ENTRAR => v.guardar_marcador(textos),
            VK_ESCAPE => v.escribiendo = None,
            VK_RETROCESO => {
                if let Some((_, texto)) = v.escribiendo.as_mut() {
                    texto.pop();
                }
            }
            _ => {}
        }
        return;
    }
    if ctrl {
        // El encuadre, como en el lienzo: Ctrl+0 encaja, Ctrl+mas/menos.
        match vk {
            VK_0 => v.encuadre.encajar(),
            VK_MAS | VK_MAS_NUM => v.acercar(encuadre::PASO, None),
            VK_MENOS | VK_MENOS_NUM => v.acercar(1.0 / encuadre::PASO, None),
            _ => {}
        }
        return;
    }
    match vk {
        VK_ESPACIO | VK_K => v.medio.alternar(),
        VK_IZQUIERDA => v.saltar(-SALTO_CORTO),
        VK_DERECHA => v.saltar(SALTO_CORTO),
        VK_J => v.saltar(-SALTO_LARGO),
        VK_L => v.saltar(SALTO_LARGO),
        VK_ARRIBA => v.otro_trozo(false),
        VK_ABAJO => v.otro_trozo(true),
        VK_M => {
            v.tragar = Some('m');
            v.empezar_marcador();
        }
        VK_S => {
            v.tragar = Some('s');
            pulsar(v, Zona::Seguir, textos, vivo);
        }
        VK_A => {
            v.tragar = Some('a');
            v.anotar = true;
        }
        VK_T => {
            v.tragar = Some('t');
            pulsar(v, Zona::Transcripcion, textos, vivo);
        }
        VK_ESCAPE => *vivo = false,
        _ => {}
    }
    // Las letras de los atajos tambien llegan como caracter: sin marcador
    // abierto no hacen nada, pero la de la M no puede acabar en su nota.
    if !matches!(vk, VK_M | VK_S | VK_A | VK_T) {
        v.tragar = None;
    }
}

// --- Pintar ------------------------------------------------------------------------

fn pintar_todo(
    p: &Pintor,
    v: &mut Vista,
    ancho: f32,
    alto: f32,
    tema: &Tema,
    e: f32,
    textos: &Catalogo,
) {
    v.zonas.clear();
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho,
            alto,
        },
        tema.chat,
    );
    // Sin barra dentro: el nombre, el asa y cerrar van en la de los
    // visores, fuera (`lector_pin`).
    // Arrastrando la barra, el tirador y lo que suena van con el raton.
    let posicion = clase::posicion_a_la_vista(v.medio.posicion_ms(), v.arrastre_ms);
    pintar_medio(p, v, e, textos);
    pintar_tiempo(p, v, tema, e, posicion);
    pintar_mandos(p, v, tema, e, textos, posicion);
    // A pantalla entera, solo la imagen y sus mandos; sin la transcripcion,
    // su columna se la queda la imagen.
    if !v.reparto.entera {
        pintar_senales(p, v, tema, e, textos, posicion);
        if v.reparto.texto {
            pintar_texto(p, v, tema, e, textos, posicion);
        }
        pintar_fotos(p, v, tema, e, textos, posicion);
    }

    if let Some((texto, _)) = &v.aviso {
        let tam = 14.0 * e;
        let (w, h) = p.medir_texto(texto, tam);
        let pastilla = RectF {
            x: (ancho - w - 36.0 * e) / 2.0,
            y: alto - h - 40.0 * e,
            ancho: w + 36.0 * e,
            alto: h + 18.0 * e,
        };
        p.rellenar_redondeado(pastilla, pastilla.alto / 2.0, hex(0x248a3d));
        p.texto(
            texto,
            pastilla.x + 18.0 * e,
            pastilla.y + 9.0 * e,
            tam,
            Color::BLANCO,
        );
    }
}

/// Encaja `w × h` dentro de `caja` sin deformarlo, centrado.
fn encajar(caja: RectF, w: u32, h: u32) -> RectF {
    if w == 0 || h == 0 {
        return caja;
    }
    let k = (caja.ancho / w as f32).min(caja.alto / h as f32);
    let (dw, dh) = (w as f32 * k, h as f32 * k);
    RectF {
        x: caja.x + (caja.ancho - dw) / 2.0,
        y: caja.y + (caja.alto - dh) / 2.0,
        ancho: dw,
        alto: dh,
    }
}

/// Lo que se ve: el fotograma del video, o la captura del minuto en grande
/// (la grande si ya llego, si no su miniatura) con lo anotado encima, o el
/// aviso de solo audio. Va con su encuadre: acercado y movido como en un
/// lienzo, Direct2D la escala al pintarla.
fn pintar_medio(p: &Pintor, v: &mut Vista, e: f32, textos: &Catalogo) {
    let caja = v.disp.medio;
    p.rellenar_redondeado(caja, 10.0 * e, hex(0x0b0f14));
    let recortado = p.empujar_recorte_redondeado(caja, 10.0 * e);
    // Lo que mide lo que se pinta (sin nada, sin mandos encima).
    let mut visto: Option<(u32, u32)> = None;
    let encuadre = v.encuadre;
    if let Some(i) = v.captura_a_la_vista() {
        let img = match &v.grande {
            Some((g, img)) if *g == i => Some(img),
            _ => v.miniaturas.get(i).and_then(|m| m.as_ref()),
        };
        if let Some((b, w, h)) = img {
            let d = rect(encuadre.destino(caja_de(caja), *w as f32, *h as f32));
            p.bitmap_con(b, d, None, interpolacion(d.ancho, *w));
            // Lo anotado en su lienzo, encima y con el mismo encuadre.
            if let Some((j, _, dibujo)) = &v.dibujo
                && *j == i
                && !dibujo.ordenes.is_empty()
            {
                super::pintar_lienzo(p, dibujo, d, None);
            }
            visto = Some((*w, *h));
        }
    } else if let Medio::Video {
        bitmap: Some(b), r, ..
    } = &mut v.medio
        && let Some((w, h)) = r.dimensiones()
    {
        let d = rect(encuadre.destino(caja_de(caja), w as f32, h as f32));
        p.bitmap_con(b, d, None, interpolacion(d.ancho, w));
        visto = Some((w, h));
    }
    if recortado {
        p.soltar_recorte_redondeado();
    }
    if visto.is_none() {
        let (icono, texto) = match (&v.medio, v.clase.capturas.is_empty()) {
            (Medio::Nada, _) => (&mi::MIC, textos.t("estudiar-sin-sonido")),
            (Medio::Video { .. }, _) => (&mi::VIDEOCAM, textos.t("estudiar-cargando")),
            (_, true) => (&mi::MIC, textos.t("estudiar-solo-audio")),
            (_, false) => (&mi::IMAGE, textos.t("estudiar-cargando")),
        };
        let lado = 40.0 * e;
        p.icono(
            icono,
            RectF {
                x: caja.x + (caja.ancho - lado) / 2.0,
                y: caja.y + caja.alto / 2.0 - lado,
                ancho: lado,
                alto: lado,
            },
            con_alfa(Color::BLANCO, 0.6),
        );
        let (w, _) = p.medir_texto(&texto, 14.0 * e);
        p.texto(
            &texto,
            caja.x + (caja.ancho - w) / 2.0,
            caja.y + caja.alto / 2.0 + 8.0 * e,
            14.0 * e,
            con_alfa(Color::BLANCO, 0.7),
        );
    }
    v.zonas.push((caja, Zona::Medio));
    // Con video y capturas, una ficha arriba a la derecha cambia entre los
    // dos: «Ver la captura» / «Ver el vídeo».
    if v.medio.es_video() && !v.clase.capturas.is_empty() {
        let (icono, rotulo) = if v.ver_capturas {
            (&mi::VIDEOCAM, textos.t("estudiar-ver-video"))
        } else {
            (&mi::IMAGE, textos.t("estudiar-ver-captura"))
        };
        let w = ancho_de_ficha(p, Some(icono), &rotulo, e);
        let ficha = RectF {
            x: caja.x + caja.ancho - w - 10.0 * e,
            y: caja.y + 10.0 * e,
            ancho: w,
            alto: 28.0 * e,
        };
        pintar_ficha(p, v, ficha, Some(icono), &rotulo, Zona::CambiarVista, e);
    }
    if visto.is_none() {
        return;
    }
    // Arriba a la izquierda, «Anotar»: la captura en su lienzo o el
    // fotograma del video.
    let anotar = textos.t("estudiar-anotar");
    let ficha = RectF {
        x: caja.x + 10.0 * e,
        y: caja.y + 10.0 * e,
        ancho: ancho_de_ficha(p, Some(&mi::DRAW), &anotar, e),
        alto: 28.0 * e,
    };
    pintar_ficha(p, v, ficha, Some(&mi::DRAW), &anotar, Zona::Anotar, e);
    let lado = 28.0 * e;
    // En la esquina, como en YouTube: la imagen a pantalla entera, o
    // reducirla (checklist 25).
    let entera = RectF {
        x: caja.x + caja.ancho - lado - 10.0 * e,
        y: caja.y + caja.alto - lado - 10.0 * e,
        ancho: lado,
        alto: lado,
    };
    let fondo = if dentro(entera, v.raton) { 0.85 } else { 0.65 };
    p.rellenar_redondeado(entera, lado / 2.0, con_alfa(hex(0x000000), fondo));
    let i = 18.0 * e;
    p.icono(
        if v.reparto.entera {
            &mi::FULLSCREEN_EXIT
        } else {
            &mi::FULLSCREEN
        },
        RectF {
            x: entera.x + (lado - i) / 2.0,
            y: entera.y + (lado - i) / 2.0,
            ancho: i,
            alto: i,
        },
        Color::BLANCO,
    );
    v.zonas.push((entera, Zona::PantallaEntera));
    // A su izquierda, el encuadre: −, + y encajar (sin el tanto por
    // ciento, checklist 25).
    let total = lado * 3.0 + 4.0 * e;
    let pastilla = RectF {
        x: entera.x - total - 8.0 * e,
        y: entera.y,
        ancho: total,
        alto: lado,
    };
    p.rellenar_redondeado(pastilla, lado / 2.0, con_alfa(hex(0x000000), 0.65));
    let mut x = pastilla.x + 2.0 * e;
    let mut sitio = |ancho: f32| {
        let r = RectF {
            x,
            y: pastilla.y,
            ancho,
            alto: lado,
        };
        x += ancho;
        r
    };
    let (menos, mas, encajar) = (sitio(lado), sitio(lado), sitio(lado));
    for (r, z) in [
        (menos, Zona::ZoomMenos),
        (mas, Zona::ZoomMas),
        (encajar, Zona::Encajar),
    ] {
        if dentro(r, v.raton) {
            p.rellenar_redondeado(r, lado / 2.0, con_alfa(Color::BLANCO, 0.16));
        }
        v.zonas.push((r, z));
    }
    let centrado = |p: &Pintor, t: &str, r: RectF, tam: f32| {
        let (tw, th) = p.medir_texto(t, tam);
        p.texto(
            t,
            r.x + (r.ancho - tw) / 2.0,
            r.y + (r.alto - th) / 2.0,
            tam,
            Color::BLANCO,
        );
    };
    centrado(p, "−", menos, 16.0 * e);
    centrado(p, "+", mas, 16.0 * e);
    let i = 16.0 * e;
    p.icono(
        &mi::CROP_FREE,
        RectF {
            x: encajar.x + (lado - i) / 2.0,
            y: encajar.y + (lado - i) / 2.0,
            ancho: i,
            alto: i,
        },
        if v.encuadre.encajado() {
            con_alfa(Color::BLANCO, 0.5)
        } else {
            Color::BLANCO
        },
    );
}

/// Lo que mide una ficha encima de lo que se ve: icono y rotulo.
fn ancho_de_ficha(
    p: &Pintor,
    icono: Option<&pixpin_render::icono::Icono>,
    rotulo: &str,
    e: f32,
) -> f32 {
    let icono = if icono.is_some() { 22.0 * e } else { 0.0 };
    p.medir_texto(rotulo, 12.5 * e).0 + icono + 20.0 * e
}

/// Una ficha encima de lo que se ve (cristal oscuro: va sobre el video, en
/// claro y en oscuro), apuntada para el raton.
fn pintar_ficha(
    p: &Pintor,
    v: &mut Vista,
    ficha: RectF,
    icono: Option<&pixpin_render::icono::Icono>,
    rotulo: &str,
    zona: Zona,
    e: f32,
) {
    let tam = 12.5 * e;
    let (_, th) = p.medir_texto(rotulo, tam);
    let fondo = if dentro(ficha, v.raton) { 0.85 } else { 0.65 };
    p.rellenar_redondeado(ficha, ficha.alto / 2.0, con_alfa(hex(0x000000), fondo));
    let mut x = ficha.x + 10.0 * e;
    if let Some(icono) = icono {
        p.icono(
            icono,
            RectF {
                x,
                y: ficha.y + (ficha.alto - 16.0 * e) / 2.0,
                ancho: 16.0 * e,
                alto: 16.0 * e,
            },
            Color::BLANCO,
        );
        x += 22.0 * e;
    }
    p.texto(
        rotulo,
        x,
        ficha.y + (ficha.alto - th) / 2.0,
        tam,
        Color::BLANCO,
    );
    v.zonas.push((ficha, zona));
}

/// Como se escala lo que se ve: suave al acercar y al reducir; a partir del
/// triple, pixeles tal cual (se acerca para ver la letra pequena).
fn interpolacion(ancho_pintado: f32, ancho_imagen: u32) -> pixpin_render::Interpolacion {
    let k = ancho_pintado / ancho_imagen.max(1) as f32;
    if k >= 3.0 {
        pixpin_render::Interpolacion::Vecino
    } else {
        pixpin_render::Interpolacion::Lineal
    }
}

/// La barra del tiempo: lo oido, una raya por captura, una banderita por
/// senal y el tirador.
fn pintar_tiempo(p: &Pintor, v: &mut Vista, tema: &Tema, e: f32, posicion: i64) {
    let caja = v.disp.tiempo;
    let duracion = v.duracion();
    let pista = RectF {
        x: caja.x,
        y: caja.y + caja.alto / 2.0 - 3.0 * e,
        ancho: caja.ancho,
        alto: 6.0 * e,
    };
    p.rellenar_redondeado(pista, 3.0 * e, con_alfa(tema.apagado, 0.35));
    let x_de = |ms: i64| caja.x + clase::x_en_la_barra(ms, caja.ancho, duracion);
    let xp = x_de(posicion);
    p.rellenar_redondeado(
        RectF {
            ancho: (xp - caja.x).max(0.0),
            ..pista
        },
        3.0 * e,
        tema.enviar,
    );
    for (ms, _) in &v.clase.capturas {
        let x = x_de(*ms);
        p.rellenar(
            RectF {
                x: x - 1.0 * e,
                y: caja.y + 4.0 * e,
                ancho: 2.0 * e,
                alto: caja.alto - 8.0 * e,
            },
            con_alfa(tema.texto, 0.7),
        );
    }
    for s in &v.clase.senales {
        let x = x_de(s.ms);
        p.rellenar(
            RectF {
                x: x - 1.0 * e,
                y: caja.y,
                ancho: 2.0 * e,
                alto: caja.alto / 2.0,
            },
            AMBAR,
        );
        p.circulo((x, caja.y + 3.0 * e), 4.0 * e, AMBAR);
    }
    let encima = dentro(caja, v.raton) || v.arrastrando_tiempo;
    p.circulo(
        (xp, caja.y + caja.alto / 2.0),
        (if encima { 8.0 } else { 6.0 }) * e,
        tema.enviar,
    );
    v.zonas.push((caja, Zona::Tiempo));
}

/// Un boton redondeado de los mandos: icono y, si lo hay, rotulo.
#[allow(clippy::too_many_arguments)]
fn boton(
    p: &Pintor,
    v: &mut Vista,
    caja: RectF,
    zona: Zona,
    icono: &pixpin_render::icono::Icono,
    rotulo: &str,
    encendido: bool,
    tema: &Tema,
    e: f32,
) {
    let fondo = if encendido {
        tema.enviar
    } else if dentro(caja, v.raton) {
        tema.fila_sobre
    } else {
        tema.lista
    };
    let tinta = if encendido { Color::BLANCO } else { tema.texto };
    p.rellenar_redondeado(caja, 10.0 * e, fondo);
    let lado = 20.0 * e;
    let tam = 13.5 * e;
    let (tw, th) = if rotulo.is_empty() {
        (0.0, 0.0)
    } else {
        p.medir_texto(rotulo, tam)
    };
    let hueco = if rotulo.is_empty() { 0.0 } else { 6.0 * e };
    let x = caja.x + (caja.ancho - lado - hueco - tw) / 2.0;
    p.icono(
        icono,
        RectF {
            x,
            y: caja.y + (caja.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        tinta,
    );
    if !rotulo.is_empty() {
        p.texto(
            rotulo,
            x + lado + hueco,
            caja.y + (caja.alto - th) / 2.0,
            tam,
            tinta,
        );
    }
    v.zonas.push((caja, zona));
}

/// Lo que mide un boton con rotulo.
fn ancho_de(p: &Pintor, rotulo: &str, e: f32) -> f32 {
    p.medir_texto(rotulo, 13.5 * e).0 + 20.0 * e + 6.0 * e + 24.0 * e
}

fn pintar_mandos(p: &Pintor, v: &mut Vista, tema: &Tema, e: f32, textos: &Catalogo, posicion: i64) {
    let caja = v.disp.mandos;
    let alto = 40.0 * e;
    let y = caja.y + (caja.alto - alto) / 2.0;
    let mut x = caja.x;
    let mut sitio = |w: f32| {
        let r = RectF {
            x,
            y,
            ancho: w,
            alto,
        };
        x += w + 6.0 * e;
        r
    };
    let atras = sitio(44.0 * e);
    let tocar = sitio(52.0 * e);
    let adelante = sitio(44.0 * e);
    boton(p, v, atras, Zona::Atras, &mi::REPLAY_10, "", false, tema, e);
    let icono = if v.medio.sonando() {
        &mi::PAUSE
    } else {
        &mi::PLAY_ARROW
    };
    boton(p, v, tocar, Zona::Tocar, icono, "", true, tema, e);
    boton(
        p,
        v,
        adelante,
        Zona::Adelante,
        &mi::FORWARD_10,
        "",
        false,
        tema,
        e,
    );
    // 12:34 / 45:00
    let reloj = format!(
        "{} / {}",
        pixpin_voz::marca_de_tiempo(posicion),
        pixpin_voz::marca_de_tiempo(v.duracion())
    );
    let tam = 14.0 * e;
    let (w_reloj, th) = p.medir_texto(&reloj, tam);
    let x_reloj = adelante.x + adelante.ancho + 14.0 * e;
    // A la derecha: la velocidad, «Marcador», «Seguir» y «Texto» (ensenar u
    // ocultar la transcripcion; a pantalla entera no hay). Estrecha la
    // ventana, solo sus iconos (`Disposicion::compacto`).
    let rotulo = |t: String| if v.disp.compacto { String::new() } else { t };
    let velocidad = pixpin_audio::reloj::velocidad_legible(v.medio.velocidad());
    let marcar = rotulo(textos.t("estudiar-marcador"));
    let seguir = rotulo(textos.t("estudiar-seguir"));
    let ver_texto = rotulo(textos.t("estudiar-ver-texto"));
    let con_texto = !v.reparto.entera;
    let ancho_boton = |r: &str| {
        if r.is_empty() {
            44.0 * e
        } else {
            ancho_de(p, r, e)
        }
    };
    let w_vel = p.medir_texto(&velocidad, 13.5 * e).0 + 24.0 * e;
    let w_marcar = ancho_boton(&marcar);
    let w_seguir = ancho_boton(&seguir);
    let w_texto = if con_texto {
        ancho_boton(&ver_texto)
    } else {
        0.0
    };
    let mut x = caja.x + caja.ancho - w_texto;
    let c_texto = RectF {
        x,
        y,
        ancho: w_texto,
        alto,
    };
    if con_texto {
        x -= 6.0 * e;
    }
    x -= w_seguir;
    let c_seguir = RectF {
        x,
        y,
        ancho: w_seguir,
        alto,
    };
    x -= w_marcar + 6.0 * e;
    let c_marcar = RectF {
        x,
        y,
        ancho: w_marcar,
        alto,
    };
    x -= w_vel + 6.0 * e;
    let c_vel = RectF {
        x,
        y,
        ancho: w_vel,
        alto,
    };
    // El reloj, si cabe entre los de la izquierda y los de la derecha.
    if x_reloj + w_reloj + 8.0 * e <= c_vel.x {
        p.texto(&reloj, x_reloj, y + (alto - th) / 2.0, tam, tema.texto);
    }
    let fondo = if dentro(c_vel, v.raton) {
        tema.fila_sobre
    } else {
        tema.lista
    };
    p.rellenar_redondeado(c_vel, 10.0 * e, fondo);
    let (tw, th) = p.medir_texto(&velocidad, 13.5 * e);
    p.texto(
        &velocidad,
        c_vel.x + (c_vel.ancho - tw) / 2.0,
        y + (alto - th) / 2.0,
        13.5 * e,
        tema.texto,
    );
    v.zonas.push((c_vel, Zona::Velocidad));
    let escribiendo = v.escribiendo.is_some();
    boton(
        p,
        v,
        c_marcar,
        Zona::Marcar,
        &mi::BOOKMARK_ADD,
        &marcar,
        escribiendo,
        tema,
        e,
    );
    let encendido = v.seguir;
    boton(
        p,
        v,
        c_seguir,
        Zona::Seguir,
        &mi::LINK,
        &seguir,
        encendido,
        tema,
        e,
    );
    if con_texto {
        let encendido = v.reparto.texto;
        boton(
            p,
            v,
            c_texto,
            Zona::Transcripcion,
            &mi::SUBTITLES,
            &ver_texto,
            encendido,
            tema,
            e,
        );
    }
}

/// Los marcadores: el que se esta escribiendo arriba y, debajo, los que
/// hay, cada uno con su minuto.
fn pintar_senales(
    p: &Pintor,
    v: &mut Vista,
    tema: &Tema,
    e: f32,
    textos: &Catalogo,
    posicion: i64,
) {
    let caja = v.disp.senales;
    if caja.alto < 40.0 * e {
        return;
    }
    p.rellenar_redondeado(caja, 10.0 * e, tema.lista);
    let m = 12.0 * e;
    let mut y = caja.y + 10.0 * e;
    if !v.clase.temas.is_empty() {
        y = pintar_temas(p, v, tema, e, textos, y);
    }
    let titulo = format!(
        "{} · {}",
        textos.t("estudiar-marcadores"),
        v.clase.senales.len()
    );
    p.texto(&titulo, caja.x + m, y, 14.0 * e, tema.texto);
    let pista = textos.t("estudiar-pista-teclas");
    let (pw, _) = p.medir_texto(&pista, 11.5 * e);
    let tw = p.medir_texto(&titulo, 14.0 * e).0;
    if tw + pw + 3.0 * m < caja.ancho {
        p.texto(
            &pista,
            caja.x + caja.ancho - m - pw,
            y + 2.0 * e,
            11.5 * e,
            tema.apagado,
        );
    }
    y += 28.0 * e;
    p.empujar_recorte(caja);
    // El que se esta escribiendo.
    if let Some((ms, texto)) = &v.escribiendo {
        let fila = RectF {
            x: caja.x + m,
            y,
            ancho: caja.ancho - 2.0 * m,
            alto: 36.0 * e,
        };
        p.rellenar_redondeado(fila, 8.0 * e, con_alfa(AMBAR, 0.18));
        let minuto = pixpin_voz::marca_de_tiempo(*ms);
        p.texto(
            &minuto,
            fila.x + 10.0 * e,
            fila.y + 9.0 * e,
            13.0 * e,
            AMBAR,
        );
        let (escrito, color) = if texto.is_empty() {
            (textos.t("estudiar-nota-del-marcador"), tema.apagado)
        } else {
            (format!("{texto}▏"), tema.texto)
        };
        p.texto_linea(
            &escrito,
            fila.x + 64.0 * e,
            fila.y + 9.0 * e,
            13.5 * e,
            fila.ancho - 74.0 * e,
            color,
        );
        y += 42.0 * e;
    }
    if v.clase.senales.is_empty() && v.escribiendo.is_none() {
        p.texto_ajustado(
            &textos.t("estudiar-sin-marcadores"),
            caja.x + m,
            y,
            13.0 * e,
            caja.ancho - 2.0 * m,
            tema.apagado,
        );
    }
    let actual = v.clase.senales.iter().rposition(|s| s.ms <= posicion);
    let senales = v.clase.senales.clone();
    for (k, s) in senales.iter().enumerate() {
        let fila = RectF {
            x: caja.x + m,
            y,
            ancho: caja.ancho - 2.0 * m,
            alto: 30.0 * e,
        };
        if fila.y > caja.y + caja.alto {
            break;
        }
        let encima = dentro(fila, v.raton);
        if actual == Some(k) || encima {
            p.rellenar_redondeado(
                fila,
                8.0 * e,
                if encima {
                    tema.fila_sobre
                } else {
                    con_alfa(AMBAR, 0.12)
                },
            );
        }
        p.icono(
            s.icono(),
            RectF {
                x: fila.x + 8.0 * e,
                y: fila.y + 7.0 * e,
                ancho: 16.0 * e,
                alto: 16.0 * e,
            },
            AMBAR,
        );
        p.texto(
            &pixpin_voz::marca_de_tiempo(s.ms),
            fila.x + 30.0 * e,
            fila.y + 7.0 * e,
            12.5 * e,
            AMBAR,
        );
        let nota = s.nota(textos);
        p.texto_linea(
            &nota,
            fila.x + 84.0 * e,
            fila.y + 7.0 * e,
            13.0 * e,
            fila.ancho - 120.0 * e,
            if s.texto.is_empty() && s.clip.is_none() {
                tema.apagado
            } else {
                tema.texto
            },
        );
        v.zonas.push((fila, s.zona(k)));
        if encima && s.clip.is_none() {
            let x = RectF {
                x: fila.x + fila.ancho - 28.0 * e,
                y: fila.y + 3.0 * e,
                ancho: 24.0 * e,
                alto: 24.0 * e,
            };
            p.icono(
                &mi::CLOSE,
                RectF {
                    x: x.x + 4.0 * e,
                    y: x.y + 4.0 * e,
                    ancho: 16.0 * e,
                    alto: 16.0 * e,
                },
                if dentro(x, v.raton) {
                    tema.cerrar_sobre
                } else {
                    tema.apagado
                },
            );
            v.zonas.push((x, Zona::Quitar(s.ms)));
        }
        y += 32.0 * e;
    }
    p.soltar_recorte();
}

/// **La agenda de la reunion**, arriba de los marcadores: cada tema con su
/// casilla (un clic la cambia, a mano), su texto (un clic salta a su
/// minuto) y su minuto; lo que quedo sin tratar, resaltado, con los dos
/// botones para llevarlo a la proxima reunion o a Tareas. Devuelve donde
/// sigue lo de debajo.
fn pintar_temas(p: &Pintor, v: &mut Vista, tema: &Tema, e: f32, textos: &Catalogo, y0: f32) -> f32 {
    let caja = v.disp.senales;
    let m = 12.0 * e;
    let mut y = y0;
    let (hechos, total) = agenda::cuenta(&v.clase.temas);
    let titulo = format!("{} · {hechos}/{total}", textos.t("clase-temas-titulo"));
    p.texto(&titulo, caja.x + m, y, 14.0 * e, tema.texto);
    let fin_titulo = caja.x + m + p.medir_texto(&titulo, 14.0 * e).0;
    // Los dos botones de lo sin tratar, a la derecha del titulo.
    if hechos < total {
        let tam = 12.0 * e;
        let alto = 22.0 * e;
        let mut x = caja.x + caja.ancho - m;
        for (clave, zona) in [
            ("clase-temas-a-tareas", Zona::TemasATareas),
            ("clase-temas-proxima", Zona::TemasProxima),
        ] {
            let rotulo = textos.t(clave);
            let (w, h) = p.medir_texto(&rotulo, tam);
            let boton = RectF {
                x: x - w - 20.0 * e,
                y: y - 2.0 * e,
                ancho: w + 20.0 * e,
                alto,
            };
            if boton.x < fin_titulo + 8.0 * e {
                break;
            }
            let encima = dentro(boton, v.raton);
            p.rellenar_redondeado(
                boton,
                alto / 2.0,
                con_alfa(AMBAR, if encima { 0.32 } else { 0.2 }),
            );
            p.texto(
                &rotulo,
                boton.x + 10.0 * e,
                boton.y + (alto - h) / 2.0,
                tam,
                tema.texto,
            );
            v.zonas.push((boton, zona));
            x = boton.x - 6.0 * e;
        }
    }
    y += 28.0 * e;
    let verde = hex(0x22a06b);
    let temas = v.clase.temas.clone();
    for (i, t) in temas.iter().enumerate() {
        let fila = RectF {
            x: caja.x + m,
            y,
            ancho: caja.ancho - 2.0 * m,
            alto: 28.0 * e,
        };
        if fila.y + fila.alto > caja.y + caja.alto {
            break;
        }
        let encima = dentro(fila, v.raton);
        if !t.hecho {
            // Lo que quedo sin tratar, resaltado.
            p.rellenar_redondeado(fila, 8.0 * e, con_alfa(AMBAR, 0.14));
        } else if encima {
            p.rellenar_redondeado(fila, 8.0 * e, tema.fila_sobre);
        }
        let casilla = RectF {
            x: fila.x + 4.0 * e,
            y: fila.y + 4.0 * e,
            ancho: 20.0 * e,
            alto: 20.0 * e,
        };
        p.icono(
            if t.hecho {
                &mi::CHECK_BOX
            } else {
                &mi::CHECK_BOX_OUTLINE_BLANK
            },
            casilla,
            if t.hecho { verde } else { tema.apagado },
        );
        let derecha = match (t.hecho, t.ms) {
            (true, Some(ms)) => pixpin_voz::marca_de_tiempo(ms),
            (false, _) => textos.t("clase-temas-sin-tratar"),
            _ => String::new(),
        };
        let tam = 12.5 * e;
        let (dw, _) = p.medir_texto(&derecha, tam);
        if !derecha.is_empty() {
            p.texto(
                &derecha,
                fila.x + fila.ancho - dw - 8.0 * e,
                fila.y + 7.0 * e,
                tam,
                if t.hecho { verde } else { AMBAR },
            );
        }
        p.texto_linea(
            &format!("{}. {}", i + 1, t.texto),
            fila.x + 32.0 * e,
            fila.y + 6.0 * e,
            13.0 * e,
            fila.ancho - 48.0 * e - dw,
            if t.hecho { tema.apagado } else { tema.texto },
        );
        // El texto salta a su minuto; la casilla (despues: gana) se marca.
        if let Some(ms) = t.ms.filter(|_| t.hecho) {
            v.zonas.push((fila, Zona::Ir(ms)));
        }
        v.zonas.push((
            RectF {
                x: fila.x,
                y: fila.y,
                ancho: 30.0 * e,
                alto: fila.alto,
            },
            Zona::Tema(i),
        ));
        y += 30.0 * e;
    }
    y + 10.0 * e
}

/// Lo que mide la columna del minuto en el texto.
const COLUMNA_MINUTO: f32 = 52.0;
const TAM_TEXTO: f32 = 14.5;

fn pintar_texto(p: &Pintor, v: &mut Vista, tema: &Tema, e: f32, textos: &Catalogo, posicion: i64) {
    let cab = v.disp.cabecera_texto;
    let caja = v.disp.texto;
    p.rellenar_redondeado(
        RectF {
            alto: cab.alto + caja.alto,
            ..cab
        },
        10.0 * e,
        tema.lista,
    );
    let titulo = textos.t("estudiar-transcripcion");
    let (tw, th) = p.medir_texto(&titulo, 14.0 * e);
    // El titulo va en la fila de arriba; debajo, si la hay, la franja de lo
    // importante.
    let fila_titulo = RectF {
        alto: CABECERA * e,
        ..cab
    };
    p.icono(
        &mi::LYRICS,
        RectF {
            x: cab.x + 12.0 * e,
            y: cab.y + (fila_titulo.alto - 18.0 * e) / 2.0,
            ancho: 18.0 * e,
            alto: 18.0 * e,
        },
        tema.enviar,
    );
    p.texto(
        &titulo,
        cab.x + 38.0 * e,
        cab.y + (fila_titulo.alto - th) / 2.0,
        14.0 * e,
        tema.texto,
    );
    pintar_lo_importante(p, v, tema, e, textos, fila_titulo, cab.x + 38.0 * e + tw);
    v.zonas.push((caja, Zona::ListaTexto));
    if v.clase.lineas.is_empty() && v.clase.senales.is_empty() {
        p.texto_ajustado(
            &textos.t("estudiar-sin-texto"),
            caja.x + 14.0 * e,
            caja.y + 12.0 * e,
            13.5 * e,
            caja.ancho - 28.0 * e,
            tema.apagado,
        );
        return;
    }
    // Lo alto de cada fila, medido una vez por ancho (y por filtro: al
    // cambiarlo se vacia).
    let ancho_texto = caja.ancho - (COLUMNA_MINUTO + 24.0) * e;
    let sin_importantes = Vec::new();
    let importantes = v.clase.importantes.as_ref().unwrap_or(&sin_importantes);
    {
        let mut cache = v.altos_texto.borrow_mut();
        let clave = caja.ancho.round() as u32;
        if cache.0 != clave || cache.1.len() != v.clase.filas.len() {
            let altos = v
                .clase
                .filas
                .iter()
                .map(|f| match f {
                    Fila::Linea(i) => {
                        let (ms, t) = v
                            .clase
                            .lineas
                            .get(*i)
                            .map_or((None, ""), |(ms, t)| (*ms, t.as_str()));
                        if !clase::se_ve_con_filtro(importantes, ms, v.filtro) {
                            return 0.0;
                        }
                        let alto_texto = p.medir_texto_ajustado(t, TAM_TEXTO * e, ancho_texto).1;
                        // Con chapitas bajo el minuto, que quepan.
                        let minimo = if clase::tipos_en(importantes, ms).is_empty() {
                            0.0
                        } else {
                            22.0 * e
                        };
                        alto_texto.max(minimo) + 14.0 * e
                    }
                    // Filtrando, solo los renglones de ese tipo.
                    Fila::Marcador(_) if v.filtro.is_some() => 0.0,
                    Fila::Marcador(_) => 34.0 * e,
                })
                .collect();
            *cache = (clave, altos);
        }
    }
    let altos = v.altos_texto.borrow().1.clone();
    v.scroll_texto = v.scroll_texto.clamp(0.0, clase::tope(&altos, caja.alto));
    let lineas_ms: Vec<Option<i64>> = v.clase.lineas.iter().map(|(ms, _)| *ms).collect();
    let actual = clase::linea_en(&lineas_ms, posicion);
    p.empujar_recorte(caja);
    let mut y = caja.y - v.scroll_texto;
    let filas = v.clase.filas.clone();
    for (k, f) in filas.iter().enumerate() {
        let alto = altos.get(k).copied().unwrap_or(0.0);
        let fila = RectF {
            x: caja.x + 6.0 * e,
            y,
            ancho: caja.ancho - 12.0 * e,
            alto,
        };
        y += alto;
        if alto <= 0.0 || fila.y + fila.alto < caja.y || fila.y > caja.y + caja.alto {
            continue;
        }
        let encima = dentro(fila, v.raton) && dentro(caja, v.raton);
        match f {
            Fila::Linea(i) => {
                let Some((ms, texto)) = v.clase.lineas.get(*i) else {
                    continue;
                };
                let es_actual = actual == Some(*i);
                if es_actual {
                    p.rellenar_redondeado(fila, 8.0 * e, con_alfa(tema.enviar, 0.16));
                    p.rellenar_redondeado(
                        RectF {
                            ancho: 3.0 * e,
                            ..fila
                        },
                        1.5 * e,
                        tema.enviar,
                    );
                } else if encima && ms.is_some() {
                    p.rellenar_redondeado(fila, 8.0 * e, tema.fila_sobre);
                }
                if let Some(ms) = ms {
                    p.texto(
                        &pixpin_voz::marca_de_tiempo(*ms),
                        fila.x + 10.0 * e,
                        fila.y + 8.0 * e,
                        12.0 * e,
                        if es_actual { tema.enviar } else { tema.apagado },
                    );
                    // Lo importante de este renglon: un punto de su color
                    // por tipo, bajo el minuto.
                    for (k, t) in clase::tipos_en(importantes, Some(*ms)).iter().enumerate() {
                        let d = 8.0 * e;
                        p.rellenar_redondeado(
                            RectF {
                                x: fila.x + 11.0 * e + k as f32 * 11.0 * e,
                                y: fila.y + 27.0 * e,
                                ancho: d,
                                alto: d,
                            },
                            d / 2.0,
                            hex(crate::jev::reunion::color_de(*t)),
                        );
                    }
                    v.zonas.push((fila, Zona::Ir(*ms)));
                }
                // Lo que aun no se ha oido, un poco apagado.
                let futuro = ms.is_some_and(|ms| ms > posicion) && v.medio.sonando();
                p.texto_ajustado(
                    texto,
                    fila.x + COLUMNA_MINUTO * e,
                    fila.y + 7.0 * e,
                    TAM_TEXTO * e,
                    ancho_texto,
                    if futuro {
                        con_alfa(tema.texto, 0.6)
                    } else {
                        tema.texto
                    },
                );
            }
            Fila::Marcador(j) => {
                let Some(s) = v.clase.senales.get(*j) else {
                    continue;
                };
                let marca = RectF {
                    y: fila.y + 3.0 * e,
                    alto: fila.alto - 6.0 * e,
                    ..fila
                };
                p.rellenar_redondeado(
                    marca,
                    8.0 * e,
                    con_alfa(AMBAR, if encima { 0.26 } else { 0.16 }),
                );
                p.icono(
                    s.icono(),
                    RectF {
                        x: marca.x + 10.0 * e,
                        y: marca.y + (marca.alto - 16.0 * e) / 2.0,
                        ancho: 16.0 * e,
                        alto: 16.0 * e,
                    },
                    AMBAR,
                );
                let nota = s.nota(textos);
                let (_, nh) = p.medir_texto(&nota, 13.0 * e);
                p.texto_linea(
                    &format!("{} · {nota}", pixpin_voz::marca_de_tiempo(s.ms)),
                    marca.x + COLUMNA_MINUTO * e,
                    marca.y + (marca.alto - nh) / 2.0,
                    13.0 * e,
                    ancho_texto,
                    tema.texto,
                );
                v.zonas.push((marca, s.zona(*j)));
            }
        }
    }
    p.soltar_recorte();
}

/// **Lo importante que hallo Jev**, en la cabecera del texto: a la derecha
/// del titulo, «+N tareas» para apuntarlas en Tareas; debajo, la franja
/// del filtro («Todo» y una chapita de su color por tipo, con cuantas hay)
/// o, mientras se busca, «Buscando lo importante…».
fn pintar_lo_importante(
    p: &Pintor,
    v: &mut Vista,
    tema: &Tema,
    e: f32,
    textos: &Catalogo,
    fila_titulo: RectF,
    fin_titulo: f32,
) {
    let tam = 12.0 * e;
    let alto = 22.0 * e;
    // «Repasar en voz alta» (Jev), el primero por la derecha.
    let mut derecha = fila_titulo.x + fila_titulo.ancho;
    let repasar = super::repaso_oral::pintar_boton(
        p,
        e,
        tema,
        v.raton,
        fila_titulo,
        fin_titulo,
        textos,
        &v.raiz,
    );
    if let Some(r) = repasar {
        v.zonas.push((r, Zona::Repasar));
        derecha = r.x + 4.0 * e;
    }
    let por_apuntar = v.clase.tareas_por_apuntar();
    if por_apuntar > 0 {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("n", por_apuntar as i64);
        let rotulo = textos.t_args("clase-importante-tareas", &args);
        let (w, h) = p.medir_texto(&rotulo, tam);
        let boton = RectF {
            x: derecha - w - 20.0 * e - 10.0 * e,
            y: fila_titulo.y + (fila_titulo.alto - alto) / 2.0,
            ancho: w + 20.0 * e,
            alto,
        };
        if boton.x > fin_titulo + 8.0 * e {
            let azul = hex(crate::jev::reunion::color_de(clase::TipoImportante::Tarea));
            let encima = dentro(boton, v.raton);
            p.rellenar_redondeado(
                boton,
                alto / 2.0,
                con_alfa(azul, if encima { 0.32 } else { 0.2 }),
            );
            p.texto(
                &rotulo,
                boton.x + 10.0 * e,
                boton.y + (alto - h) / 2.0,
                tam,
                tema.texto,
            );
            v.zonas.push((boton, Zona::ATareas));
        }
    }
    if !v.reparto.franja {
        return;
    }
    let franja = RectF {
        x: fila_titulo.x + 10.0 * e,
        y: fila_titulo.y + fila_titulo.alto,
        ancho: fila_titulo.ancho - 20.0 * e,
        alto: FRANJA * e,
    };
    let Some(importantes) = v.clase.importantes.clone().filter(|i| !i.is_empty()) else {
        let rotulo = textos.t("clase-importante-buscando");
        let (_, h) = p.medir_texto(&rotulo, tam);
        p.texto(
            &rotulo,
            franja.x + 4.0 * e,
            franja.y + (franja.alto - h) / 2.0 - 3.0 * e,
            tam,
            tema.apagado,
        );
        return;
    };
    let mut x = franja.x;
    let y = franja.y + (franja.alto - alto) / 2.0 - 3.0 * e;
    let mut chapas: Vec<(Option<clase::TipoImportante>, String, Color)> =
        vec![(None, textos.t("clase-importante-todo"), tema.enviar)];
    for t in clase::TipoImportante::TODOS {
        let n = importantes
            .iter()
            .filter(|i| i.tipo == t)
            .map(|i| i.ms)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        if n > 0 {
            chapas.push((
                Some(t),
                format!("{} {n}", textos.t(crate::jev::reunion::clave_de(t))),
                hex(crate::jev::reunion::color_de(t)),
            ));
        }
    }
    for (filtro, rotulo, color) in chapas {
        let (w, h) = p.medir_texto(&rotulo, tam);
        let punto = if filtro.is_some() { 12.0 * e } else { 0.0 };
        let chapa = RectF {
            x,
            y,
            ancho: w + 18.0 * e + punto,
            alto,
        };
        if chapa.x + chapa.ancho > franja.x + franja.ancho {
            break;
        }
        let elegida = v.filtro == filtro;
        let encima = dentro(chapa, v.raton);
        p.rellenar_redondeado(
            chapa,
            alto / 2.0,
            if elegida {
                con_alfa(color, 0.9)
            } else {
                con_alfa(color, if encima { 0.26 } else { 0.14 })
            },
        );
        if filtro.is_some() {
            let d = 7.0 * e;
            p.rellenar_redondeado(
                RectF {
                    x: chapa.x + 9.0 * e,
                    y: chapa.y + (alto - d) / 2.0,
                    ancho: d,
                    alto: d,
                },
                d / 2.0,
                if elegida { Color::BLANCO } else { color },
            );
        }
        p.texto(
            &rotulo,
            chapa.x + 9.0 * e + punto,
            chapa.y + (alto - h) / 2.0,
            tam,
            if elegida { Color::BLANCO } else { tema.texto },
        );
        v.zonas.push((chapa, Zona::Filtro(filtro)));
        x += chapa.ancho + 6.0 * e;
    }
}

fn pintar_fotos(p: &Pintor, v: &mut Vista, tema: &Tema, e: f32, textos: &Catalogo, posicion: i64) {
    let cab = v.disp.cabecera_fotos;
    let caja = v.disp.fotos;
    p.rellenar_redondeado(
        RectF {
            alto: cab.alto + caja.alto,
            ..cab
        },
        10.0 * e,
        tema.lista,
    );
    let titulo = format!(
        "{} · {}",
        textos.t("estudiar-capturas"),
        v.clase.capturas.len()
    );
    let (_, th) = p.medir_texto(&titulo, 14.0 * e);
    p.icono(
        &mi::PHOTO_LIBRARY,
        RectF {
            x: cab.x + 12.0 * e,
            y: cab.y + (cab.alto - 18.0 * e) / 2.0,
            ancho: 18.0 * e,
            alto: 18.0 * e,
        },
        tema.enviar,
    );
    p.texto(
        &titulo,
        cab.x + 38.0 * e,
        cab.y + (cab.alto - th) / 2.0,
        14.0 * e,
        tema.texto,
    );
    v.zonas.push((caja, Zona::ListaFotos));
    if v.clase.capturas.is_empty() {
        p.texto_ajustado(
            &textos.t("estudiar-sin-capturas"),
            caja.x + 14.0 * e,
            caja.y + 12.0 * e,
            13.0 * e,
            caja.ancho - 28.0 * e,
            tema.apagado,
        );
        return;
    }
    let altos = v.altos_fotos();
    v.scroll_fotos = v.scroll_fotos.clamp(0.0, clase::tope(&altos, caja.alto));
    let actual = clase::ultimo_antes(&v.clase.tiempos_capturas(), posicion);
    p.empujar_recorte(caja);
    // Cada fila con lo alto de su captura (su proporcion de verdad).
    let mut y = caja.y - v.scroll_fotos;
    for (i, (ms, _)) in v.clase.capturas.clone().iter().enumerate() {
        let paso = altos.get(i).copied().unwrap_or(0.0);
        let arriba = y;
        y += paso;
        if arriba + paso < caja.y || arriba > caja.y + caja.alto {
            continue;
        }
        let fila = RectF {
            x: caja.x + 6.0 * e,
            y: arriba,
            ancho: caja.ancho - 12.0 * e,
            alto: paso,
        };
        let hueco = RectF {
            x: caja.x + 8.0 * e,
            y: arriba + 8.0 * e,
            ancho: caja.ancho - 16.0 * e,
            alto: paso - 32.0 * e,
        };
        // El marco es la imagen misma, sin franjas: centrada a lo ancho.
        let marco = match v.miniaturas.get(i) {
            Some(Some((_, w, h))) => encajar(hueco, *w, *h),
            _ => hueco,
        };
        let es_actual = actual == Some(i);
        let encima = dentro(fila, v.raton) && dentro(caja, v.raton);
        if es_actual {
            p.rellenar_redondeado(
                RectF {
                    x: marco.x - 3.0 * e,
                    y: marco.y - 3.0 * e,
                    ancho: marco.ancho + 6.0 * e,
                    alto: marco.alto + 6.0 * e,
                },
                9.0 * e,
                tema.enviar,
            );
        } else if encima {
            p.rellenar_redondeado(fila, 8.0 * e, tema.fila_sobre);
        }
        p.rellenar_redondeado(marco, 6.0 * e, hex(0x0b0f14));
        if let Some(Some((b, _, _))) = v.miniaturas.get(i) {
            p.bitmap(b, marco, None, false);
        }
        // De quien es (un colaborador) y si su hora no se comprobo.
        if let Some((autor, sin)) = v.clase.autores.get(i)
            && (autor.is_some() || *sin)
        {
            let aviso = sin.then(|| textos.t("clase-hora-sin-comprobar"));
            super::chapa_de_autor(p, marco, autor.as_deref(), aviso.as_deref(), e);
        }
        let rotulo = format!("⏱ {}", pixpin_voz::marca_de_tiempo(*ms));
        p.texto(
            &rotulo,
            hueco.x + 2.0 * e,
            hueco.y + hueco.alto + 5.0 * e,
            12.5 * e,
            if es_actual { tema.enviar } else { tema.apagado },
        );
        v.zonas.push((fila, Zona::Captura(i)));
    }
    p.soltar_recorte();
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_nota_de_una_clase_y_lo_suyo_abren_la_clase() {
        let mut voz = cuaderno::Mensaje {
            id: "v".into(),
            ..Default::default()
        };
        assert_eq!(de_una_clase(&voz), None, "una nota de voz cualquiera no");
        voz.resto
            .insert("grabacion".into(), serde_json::json!("clase"));
        assert_eq!(de_una_clase(&voz), Some(("v".into(), 0)));
        let mut foto = cuaderno::Mensaje {
            id: "f".into(),
            ..Default::default()
        };
        foto.resto.insert(
            "enGrabacion".into(),
            serde_json::json!({ "de": "v", "ms": 65_000 }),
        );
        assert_eq!(de_una_clase(&foto), Some(("v".into(), 65_000)));
        // Caso negativo: un enlace roto (sin `de`) no es de ninguna clase.
        foto.resto
            .insert("enGrabacion".into(), serde_json::json!({ "ms": 1 }));
        assert_eq!(de_una_clase(&foto), None);
    }

    #[test]
    fn las_senales_juntan_marcadores_y_banderitas_sin_repetir() {
        let mut m = cuaderno::Mensaje::default();
        clase::poner_marcador(&mut m.resto, 5_000, "ojo");
        crate::voz::poner_marca(&mut m, 5_000);
        crate::voz::poner_marca(&mut m, 1_000);
        let s = senales_de(&m);
        assert_eq!(s.len(), 2, "la banderita del mismo minuto no se repite");
        assert!(s[0].banderita && s[0].ms == 1_000);
        assert!(!s[1].banderita && s[1].texto == "ojo");
    }

    #[test]
    fn la_franja_de_lo_importante_baja_solo_el_texto() {
        let con = disponer(
            1360.0,
            840.0,
            1.0,
            Reparto {
                franja: true,
                ..Reparto::default()
            },
        );
        let sin = disponer(1360.0, 840.0, 1.0, Reparto::default());
        assert_eq!(con.texto.y, sin.texto.y + FRANJA);
        assert_eq!(con.cabecera_texto.alto, CABECERA + FRANJA);
        assert_eq!(con.texto.y + con.texto.alto, sin.texto.y + sin.texto.alto);
        // La tira de las capturas no se mueve.
        assert_eq!(con.fotos.y, sin.fotos.y);
    }

    #[test]
    fn las_columnas_caben_y_no_se_pisan() {
        let sin_texto = Reparto {
            texto: false,
            ..Reparto::default()
        };
        let entera = Reparto {
            entera: true,
            ..Reparto::default()
        };
        for (w, h, e) in [
            (1360.0, 840.0, 1.0),
            (2040.0, 1260.0, 1.5),
            (1100.0, 700.0, 1.0),
            // Lo menos que se deja estirar, a 100 % y a 150 %.
            (ANCHO_MINIMO, ALTO_MINIMO, 1.0),
            (ANCHO_MINIMO * 1.5, ALTO_MINIMO * 1.5, 1.5),
        ] {
            for r in [Reparto::default(), sin_texto, entera] {
                let d = disponer(w, h, e, r);
                assert!(d.medio.x + d.medio.ancho <= d.texto.x, "{w} {r:?}");
                assert!(d.texto.x + d.texto.ancho <= d.fotos.x, "{w} {r:?}");
                assert!(d.fotos.x + d.fotos.ancho <= w, "{w} {r:?}");
                assert!(d.mandos.y + d.mandos.alto <= h, "{w} {r:?}");
                // Sin barra dentro: lo de arriba empieza en el margen.
                assert_eq!(d.medio.y, MARGEN * e, "{w}");
            }
            // Sin la transcripcion, su sitio es para la imagen; a pantalla
            // entera, toda la ventana.
            let con = disponer(w, h, e, Reparto::default());
            let sin = disponer(w, h, e, sin_texto);
            let toda = disponer(w, h, e, entera);
            assert!(sin.medio.ancho > con.medio.ancho, "{w}");
            assert_eq!(sin.texto.ancho, 0.0);
            assert!(toda.medio.ancho >= sin.medio.ancho && toda.medio.alto > con.medio.alto);
            assert_eq!((toda.fotos.ancho, toda.senales.alto), (0.0, 0.0));
            assert!(con.ancho_miniatura > 0.0);
        }
        // Holgada, los mandos con sus rotulos; a lo menos que se estira, solo
        // sus iconos (unos 360 logicos), que caben.
        assert!(!disponer(1360.0, 840.0, 1.0, Reparto::default()).compacto);
        let d = disponer(ANCHO_MINIMO, ALTO_MINIMO, 1.0, Reparto::default());
        assert!(d.compacto && d.mandos.ancho >= 360.0, "{}", d.mandos.ancho);
        // Sin la transcripcion vuelven los rotulos.
        assert!(!disponer(ANCHO_MINIMO, ALTO_MINIMO, 1.0, sin_texto).compacto);
    }

    #[test]
    fn las_miniaturas_van_con_su_proporcion_y_de_pie_con_tope() {
        assert_eq!(alto_de_miniatura(160.0, Some((1600, 900))), 90.0);
        // De pie: como mucho cuadrada (varia el ancho, no el alto).
        assert_eq!(alto_de_miniatura(160.0, Some((900, 1600))), 160.0);
        // Caso negativo: sin medidas (aun no llego) o vacia, 16:9.
        assert_eq!(alto_de_miniatura(160.0, None), 90.0);
        assert_eq!(alto_de_miniatura(160.0, Some((0, 10))), 90.0);
        assert!(alto_de_fila(90.0, 1.0) > 90.0);
    }

    #[test]
    fn encajar_no_deforma() {
        let c = RectF {
            x: 0.0,
            y: 0.0,
            ancho: 160.0,
            alto: 90.0,
        };
        let r = encajar(c, 100, 100);
        assert_eq!((r.ancho, r.alto), (90.0, 90.0));
        assert_eq!(r.x, 35.0);
        // Caso negativo: una imagen vacia se queda con la caja.
        assert_eq!(encajar(c, 0, 10).ancho, 160.0);
    }
}
