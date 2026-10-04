//! **Lo que la nota mete del chat y de fuera** (H12, 1-oct): documentos,
//! mensajes del chat, hojas enlazadas y audios con su transcripcion. Aqui
//! lo que es de PixPin (que dice cada renglon, los mensajes del proyecto,
//! el reproductor, Whisper, abrir); como van en el `.md` y como se pintan,
//! en `pixpin_notas::incrustados`.
//!
//! # Que se copia y que se enlaza
//!
//! - **De fuera** (el menu «Documento» o «Audio», soltar o pegar ficheros):
//!   se **copia** junto a la nota, como las fotos (`adjuntos::adjuntar`):
//!   lo que esta en la nota es de la nota y viaja con ella (`Adjuntos.kt`
//!   del movil: «se copian, no se enlazan»).
//! - **Del chat**: se **enlaza**, no se copia. Un mensaje va como
//!   `[texto](pixpin:mensaje=<proyecto>/<codigo>)` y se pinta como su
//!   burbuja (con su hora y su chapa, y lo que diga el mensaje ahora: si se
//!   edita, la nota lo ve). Una nota de voz va como su audio
//!   (`![Nota de voz](pixpin:files/…)`, el fichero del chat) con su
//!   transcripcion debajo, como la deja el movil al transcribir.
//!
//! # Lo que se lee del disco
//!
//! El editor pregunta por cada renglon al pintar y luego cada dos segundos
//! (para ver si un mensaje cambio o se borro): los mensajes de cada
//! proyecto se guardan leidos mientras su cuaderno no cambie de fecha, que
//! un chat de anos son megas.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

use pixpin_docs::md_imagen;
use pixpin_notas::incrustados::{
    self as inc, Archivo, Audio, Burbuja, Contenido, EstadoAudio, Ficha, MensajeElegible,
    OrdenAudio, RotulosIncrustados, Transcribiendo,
};
use pixpin_proyecto::almacen::{self, Ficha as FichaProyecto, Indice};
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_store::{Catalogo, Ubicacion};

use super::{Destino, adjuntos, paginas_vivas};

/// Cuantos mensajes salen en el menu «Del chat»: los ultimos. Mas seria
/// un menu que no cabe en la pantalla.
const EN_EL_MENU: usize = 40;

/// Los textos de los incrustados, del catalogo.
pub fn rotulos(t: &Catalogo) -> RotulosIncrustados {
    // El avance lo pone el pintor: se le deja `{pct}` en su sitio.
    let mut pct = fluent_bundle::FluentArgs::new();
    pct.set("pct", "{pct}");
    RotulosIncrustados {
        documento: t.t("nota-md-documento"),
        del_chat: t.t("nota-md-del-chat"),
        audio: t.t("nota-md-audio"),
        abrir: t.t("nota-md-abrir"),
        quitar: t.t("nota-md-quitar"),
        ir_al_mensaje: t.t("nota-md-ir-al-mensaje"),
        borrado: t.t("nota-md-mensaje-borrado"),
        falta: t.t("nota-md-no-esta"),
        pasar_a_texto: t.t("nota-md-pasar-a-texto"),
        pasando: t.t_args("nota-md-pasando-a-texto", &pct),
        sin_mensajes: t.t("nota-md-sin-mensajes"),
    }
}

// ---------------------------------------------------------------------------
// Buscar mensajes

/// El proyecto de una nota y su ficha, si es de uno.
fn proyecto_de(raiz: &Path, destino: &Destino) -> Option<FichaProyecto> {
    let p = paginas_vivas::proyecto_de(destino)?;
    paginas_vivas::ficha_de(&Indice::leer(raiz), p)
}

/// Los mensajes de un proyecto, leidos una vez mientras su cuaderno no
/// cambie.
#[derive(Default)]
pub struct Leidos {
    por_proyecto: HashMap<String, (Option<SystemTime>, Vec<Mensaje>)>,
}

impl Leidos {
    pub fn de(&mut self, raiz: &Path, f: &FichaProyecto) -> &[Mensaje] {
        let carpeta = almacen::carpeta(raiz, &f.id);
        let fecha = ["guardados.jsonl", "proyecto.json"]
            .iter()
            .filter_map(|n| {
                std::fs::metadata(carpeta.join(n))
                    .and_then(|m| m.modified())
                    .ok()
            })
            .max();
        let viejo = self
            .por_proyecto
            .get(&f.id)
            .is_none_or(|(antes, _)| *antes != fecha);
        if viejo {
            self.por_proyecto
                .insert(f.id.clone(), (fecha, paginas_vivas::mensajes_de(raiz, f)));
        }
        self.por_proyecto
            .get(&f.id)
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[])
    }
}

/// La ruta del fichero de un mensaje en este equipo, si esta.
fn fichero_del_mensaje(raiz: &Path, proyecto: &str, m: &Mensaje) -> Option<PathBuf> {
    let r = m.ruta.as_deref().filter(|r| !r.is_empty())?;
    pixpin_proyecto::vista::ruta_real(raiz, proyecto, r).filter(|p| p.is_file())
}

/// La ruta del fichero de un mensaje como se escribe en una nota: la
/// portatil (`pixpin:files/…`), que entienden el PC y el movil; la del
/// mensaje si no hay otra.
fn ruta_para_la_nota(raiz: &Path, proyecto: &str, m: &Mensaje) -> Option<String> {
    let r = m.ruta.as_deref().filter(|r| !r.is_empty())?;
    if r.starts_with("pixpin:files/") {
        return Some(r.to_string());
    }
    let real = pixpin_proyecto::vista::ruta_real(raiz, proyecto, r)?;
    Some(pixpin_proyecto::vista::portatil_de_ruta(raiz, &real).unwrap_or_else(|| r.to_string()))
}

/// El texto de un mensaje en una linea o pocas, sin marcas de Markdown.
fn texto_de(m: &Mensaje) -> String {
    let t = if m.texto.trim().is_empty() {
        m.nombre.clone()
    } else {
        m.texto.clone()
    };
    crate::voz::sin_marcas(&t)
}

/// «1.2 MB · PDF», como la fila de un archivo del chat.
fn detalle(bytes: u64, nombre: &str) -> String {
    let ext = pixpin_ui::chat::extension_corta(nombre);
    match (bytes, ext.is_empty()) {
        (0, true) => String::new(),
        (0, false) => ext,
        (b, true) => pixpin_ui::chat::tamano_corto(b),
        (b, false) => format!("{} · {ext}", pixpin_ui::chat::tamano_corto(b)),
    }
}

/// La clase corta de una hoja, como la tira de Proyectos.
fn clase_corta(m: &Mensaje) -> String {
    match m.clase {
        Some(Clase::Pagina) => "PDF".into(),
        Some(Clase::Nota) => "MD".into(),
        Some(Clase::MiniApp) => "TABLA".into(),
        Some(Clase::Imagen) => "FOTO".into(),
        _ => "2D".into(),
    }
}

/// **La burbuja de un mensaje**, como la pinta el chat: su hora, su chapa y
/// lo que lleva. `miniatura` es la de la hoja, si ya se pinto.
pub fn burbuja_de(
    raiz: &Path,
    proyecto: &str,
    m: &Mensaje,
    miniatura: Option<PathBuf>,
    t: &Catalogo,
) -> Burbuja {
    let ahora = pixpin_shell::entorno::ahora_local_ms();
    let hora = pixpin_ui::chat::etiqueta_hora(pixpin_shell::entorno::a_local(m.cuando), ahora);
    let fichero = fichero_del_mensaje(raiz, proyecto, m);
    let nombre = if m.nombre.trim().is_empty() {
        m.ruta
            .as_deref()
            .and_then(|r| r.rsplit(['/', '\\']).next())
            .unwrap_or("")
            .to_string()
    } else {
        m.nombre.trim().to_string()
    };
    let contenido = match m.clase {
        Some(Clase::Voz) => Contenido::Voz {
            duracion: inc::marca_de_tiempo(m.duracion_ms),
            texto: crate::voz::transcripcion_de(m)
                .map(|x| {
                    crate::voz::trozos(x)
                        .into_iter()
                        .map(|t| t.texto)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default(),
        },
        Some(Clase::Imagen) if fichero.is_some() => Contenido::Foto {
            ruta: fichero.clone().unwrap_or_default(),
            pie: m.texto.trim().to_string(),
        },
        Some(Clase::Dibujo) | Some(Clase::Pagina) | Some(Clase::MiniApp) => Contenido::Hoja {
            nombre: paginas_vivas::buscar(raiz, Some(proyecto), &m.codigo_unico())
                .map(|h| h.nombre)
                .unwrap_or_else(|| nombre.clone()),
            miniatura,
            clase: clase_corta(m),
        },
        _ if m.ruta.as_deref().is_some_and(|r| !r.is_empty()) => {
            let bytes = fichero
                .as_ref()
                .and_then(|f| std::fs::metadata(f).ok())
                .map(|x| x.len())
                .unwrap_or(m.bytes.max(0) as u64);
            Contenido::Archivo(Archivo {
                nombre: nombre.clone(),
                detalle: if fichero.is_some() {
                    detalle(bytes, &nombre)
                } else {
                    t.t("nota-md-no-esta")
                },
                falta: fichero.is_none(),
            })
        }
        _ => Contenido::Texto(texto_de(m)),
    };
    Burbuja {
        hora,
        codigo: crate::ventana_chat::chapa_de_codigo(m),
        contenido,
    }
}

/// Lo que se escribe en la nota para el mensaje `m` del proyecto `f`: su
/// audio con la transcripcion si es una nota de voz con su fichero, o el
/// enlace que se pinta como su burbuja.
pub fn bloque_de(raiz: &Path, f: &FichaProyecto, m: &Mensaje, t: &Catalogo) -> String {
    if m.clase == Some(Clase::Voz)
        && fichero_del_mensaje(raiz, &f.id, m).is_some()
        && let Some(ruta) = ruta_para_la_nota(raiz, &f.id, m)
    {
        let nombre = match crate::biblioteca_audio::titulo_de_audio(m) {
            n if n.trim().is_empty() => t.t("nota-md-nota-de-voz"),
            n => n,
        };
        return inc::bloque_de_audio(&nombre, &ruta, crate::voz::transcripcion_de(m));
    }
    let texto = match m.resumen() {
        r if r.trim().is_empty() => crate::ventana_chat::chapa_de_codigo(m).unwrap_or_default(),
        r => crate::voz::sin_marcas(&r),
    };
    inc::renglon_de_mensaje(
        &texto,
        &paginas_vivas::codigo_de_proyecto(f),
        &m.codigo_unico(),
    )
}

/// Si un mensaje se puede meter en una nota como incrustado (todos los del
/// chat menos los que aun estan en el buzon, que se van solos).
pub fn se_puede_enlazar(m: &Mensaje) -> bool {
    !m.en_buzon && !m.id.is_empty()
}

// ---------------------------------------------------------------------------
// Abrir

/// Que se hace al abrir un incrustado (pura salvo por leer el disco).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Apertura {
    /// El mensaje, en su chat.
    Mensaje {
        proyecto: String,
        codigo: String,
    },
    /// Un documento de este equipo, en su lector de PixPin o su programa.
    Fichero(PathBuf),
    Nada,
    /// No es un incrustado: lo abre quien abre las fotos y las hojas.
    NoEsMio,
}

pub fn que_abre(raiz: &Path, destino: &Destino, ruta: &str) -> Apertura {
    if let Some((p, c)) = inc::mensaje_del_enlace(ruta) {
        return match paginas_vivas::ficha_de(&Indice::leer(raiz), &p) {
            Some(f) => Apertura::Mensaje {
                proyecto: f.id,
                codigo: c,
            },
            None => Apertura::Nada,
        };
    }
    if md_imagen::hoja_del_enlace(ruta).is_some()
        || md_imagen::hoja_de_viva(ruta).is_some()
        || md_imagen::es_foto(ruta)
    {
        return Apertura::NoEsMio;
    }
    match adjuntos::resolver(raiz, destino, ruta).filter(|r| r.is_file()) {
        Some(r) => Apertura::Fichero(r),
        None => Apertura::Nada,
    }
}

/// Abre un documento como lo abre el chat al tocar su burbuja: un Markdown
/// en el editor de notas, un PDF, Word, libro o presentacion en su lector
/// de PixPin, y lo demas con lo que diga Windows.
fn abrir_fichero(idioma: pixpin_store::Idioma, ubicacion: &Ubicacion, r: &Path) {
    let nombre = pixpin_docs::nombre(r);
    if super::es_markdown(r) {
        super::abrir(
            idioma,
            ubicacion.clone(),
            Destino::Fichero {
                ruta: r.to_path_buf(),
            },
        );
        return;
    }
    if crate::lector::se_lee_al_tocar(&nombre)
        && crate::lector::abrir_en_su_lector(idioma, ubicacion, r, &nombre)
    {
        return;
    }
    if let Err(e) = pixpin_shell::abrir(r) {
        tracing::warn!(?e, ruta = %r.display(), "no se pudo abrir el documento de la nota");
    }
}

/// Abre un incrustado. `false` si no es uno (lo abre `paginas_vivas`).
pub fn abrir(
    idioma: pixpin_store::Idioma,
    ubicacion: &Ubicacion,
    destino: &Destino,
    ruta: &str,
) -> bool {
    match que_abre(ubicacion.raiz(), destino, ruta) {
        Apertura::NoEsMio => false,
        Apertura::Nada => {
            tracing::info!(%ruta, "el incrustado de la nota ya no esta");
            true
        }
        Apertura::Fichero(r) => {
            abrir_fichero(idioma, ubicacion, &r);
            true
        }
        Apertura::Mensaje { proyecto, codigo } => {
            crate::ventana_chat::ir_a(
                idioma,
                ubicacion.clone(),
                paginas_vivas::opciones_del_lienzo(),
                proyecto,
                Some(codigo),
            );
            true
        }
    }
}

// ---------------------------------------------------------------------------
// Lo que se le da al editor

/// Un salto del reproductor pendiente de que se sepa cuanto dura el audio.
#[derive(Debug, Clone, Copy)]
enum Pendiente {
    Ms(i64),
    Fraccion(f32),
}

/// **Los medios de una nota abierta**: lo de arriba sobre la nota `actual`
/// (que cambia de nueva a su mensaje al guardarse), con el reproductor de
/// este hilo (`audio`, uno por hilo) y Whisper.
pub struct MediosDeLaNota {
    raiz: PathBuf,
    ubicacion: Ubicacion,
    idioma: pixpin_store::Idioma,
    textos: Catalogo,
    actual: Rc<RefCell<Destino>>,
    leidos: Leidos,
    /// La ruta (del Markdown) de lo cargado en el reproductor.
    cargado: Option<String>,
    pendiente: Option<Pendiente>,
    transcribiendo: Option<(String, crate::voz::EnMarcha)>,
    /// Las hojas cuya miniatura ya se pidio pintar (una vez).
    pidiendo: Rc<RefCell<HashSet<String>>>,
}

impl MediosDeLaNota {
    pub fn nuevo(
        idioma: pixpin_store::Idioma,
        ubicacion: &Ubicacion,
        actual: &Rc<RefCell<Destino>>,
    ) -> MediosDeLaNota {
        MediosDeLaNota {
            raiz: ubicacion.raiz().to_path_buf(),
            ubicacion: ubicacion.clone(),
            idioma,
            textos: Catalogo::nuevo(idioma),
            actual: actual.clone(),
            leidos: Leidos::default(),
            cargado: None,
            pendiente: None,
            transcribiendo: None,
            pidiendo: Rc::default(),
        }
    }

    fn resolver(&self, ruta: &str) -> Option<PathBuf> {
        adjuntos::resolver(&self.raiz, &self.actual.borrow(), ruta).filter(|r| r.is_file())
    }

    /// La miniatura de una hoja: la pagina viva de la nota si ya esta; si
    /// no, se pinta una vez en otro hilo (con lo de las paginas vivas) y la
    /// proxima vez que se pregunte ya estara.
    fn miniatura(&self, h: &paginas_vivas::HojaDeProyecto) -> Option<PathBuf> {
        if h.mensaje.clase == Some(Clase::Imagen) {
            return fichero_del_mensaje(&self.raiz, &h.proyecto, &h.mensaje);
        }
        let destino = self.actual.borrow().clone();
        let (png, _) =
            adjuntos::sitio(&self.raiz, &destino, &md_imagen::fichero_de_viva(&h.codigo)).ok()?;
        if png.is_file() {
            return Some(png);
        }
        if self.pidiendo.borrow_mut().insert(h.codigo.clone()) {
            let (raiz, h, idioma) = (self.raiz.clone(), h.clone(), self.idioma);
            let lanzado = std::thread::Builder::new()
                .name("miniatura-de-nota".into())
                .spawn(move || {
                    let _com = pixpin_shell::ComDelHilo::iniciar();
                    let t = Catalogo::nuevo(idioma);
                    if let Err(e) = paginas_vivas::pintar(&raiz, &h, &t)
                        .and_then(|img| paginas_vivas::escribir_png(&png, &img))
                    {
                        tracing::warn!(?e, "no se pudo pintar la miniatura de la hoja enlazada");
                    }
                });
            if let Err(e) = lanzado {
                tracing::warn!(?e, "no se pudo lanzar la miniatura");
            }
        }
        None
    }

    fn ficha_de_mensaje(&mut self, p: &str, c: &str) -> Ficha {
        let Some(f) = paginas_vivas::ficha_de(&Indice::leer(&self.raiz), p) else {
            return Ficha::Borrado(String::new());
        };
        let raiz = self.raiz.clone();
        let Some(m) = self
            .leidos
            .de(&raiz, &f)
            .iter()
            .find(|m| m.codigo_unico() == c)
            .cloned()
        else {
            return Ficha::Borrado(String::new());
        };
        let miniatura = matches!(
            m.clase,
            Some(Clase::Dibujo) | Some(Clase::Pagina) | Some(Clase::MiniApp)
        )
        .then(|| paginas_vivas::buscar(&raiz, Some(&f.id), c))
        .flatten()
        .and_then(|h| self.miniatura(&h));
        Ficha::Burbuja(burbuja_de(&raiz, &f.id, &m, miniatura, &self.textos))
    }

    fn ficha_de_hoja(&mut self, p: &str, c: &str) -> Ficha {
        let Some(h) = paginas_vivas::buscar(&self.raiz, Some(p), c) else {
            return Ficha::Borrado(String::new());
        };
        let miniatura = self.miniatura(&h);
        let mut b = burbuja_de(
            &self.raiz,
            &h.proyecto,
            &h.mensaje,
            miniatura.clone(),
            &self.textos,
        );
        // Una hoja se ensena siempre como hoja (una foto o una nota del
        // proyecto tambien), con su nombre de la tarjeta de Proyectos.
        b.contenido = Contenido::Hoja {
            nombre: h.nombre.clone(),
            miniatura,
            clase: clase_corta(&h.mensaje),
        };
        Ficha::Burbuja(b)
    }

    fn ficha_de_fichero(&mut self, ruta: &str) -> Ficha {
        let nombre_md = ruta.rsplit(['/', '\\']).next().unwrap_or(ruta);
        // Sin la hora que le pone `adjuntos::adjuntar` delante.
        let nombre = match nombre_md.split_once('-') {
            Some((hora, resto))
                if !resto.is_empty() && hora.chars().all(|c| c.is_ascii_digit()) =>
            {
                resto
            }
            _ => nombre_md,
        }
        .to_string();
        let fichero = self.resolver(ruta);
        if inc::es_audio(ruta) {
            return Ficha::Audio(Audio {
                nombre,
                duracion_ms: fichero.as_ref().map_or(0, |f| self.duracion_de(f)),
                falta: fichero.is_none(),
            });
        }
        Ficha::Archivo(match fichero {
            Some(f) => Archivo {
                detalle: detalle(std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0), &nombre),
                nombre,
                falta: false,
            },
            None => Archivo {
                nombre,
                detalle: self.textos.t("nota-md-no-esta"),
                falta: true,
            },
        })
    }

    /// Lo que dura un audio, si es el de una nota de voz del proyecto de la
    /// nota (el mensaje lo apunta al grabarse); si no, se sabra al tocarlo.
    fn duracion_de(&mut self, fichero: &Path) -> i64 {
        let Some(f) = proyecto_de(&self.raiz, &self.actual.borrow()) else {
            return 0;
        };
        let raiz = self.raiz.clone();
        self.leidos
            .de(&raiz, &f)
            .iter()
            .filter(|m| m.clase == Some(Clase::Voz) && m.duracion_ms > 0)
            .find(|m| fichero_del_mensaje(&raiz, &f.id, m).as_deref() == Some(fichero))
            .map_or(0, |m| m.duracion_ms)
    }

    fn titulo(&self, ruta: &str) -> String {
        ruta.rsplit(['/', '\\']).next().unwrap_or(ruta).to_string()
    }

    /// Lleva lo cargado a `ms` si ya se sabe cuanto dura (salto relativo,
    /// como `ir_al_milisegundo` del chat: el minuto exacto del parrafo).
    fn ir_a_ms(ms: i64) -> bool {
        let e = crate::audio::estado();
        if e.duracion_ms <= 0 {
            return false;
        }
        crate::audio::saltar(ms - e.posicion_ms);
        true
    }

    fn estado(&self) -> Option<EstadoAudio> {
        let ruta = self.cargado.clone()?;
        let real = self.resolver(&ruta)?;
        if crate::audio::cargado().as_deref() != Some(real.as_path()) {
            return None;
        }
        let e = crate::audio::estado();
        Some(EstadoAudio {
            ruta,
            sonando: e.sonando,
            posicion_ms: e.posicion_ms,
            duracion_ms: e.duracion_ms,
            velocidad: e.velocidad,
        })
    }

    /// Carga `ruta` si no es la que suena, y arranca.
    fn cargar(&mut self, ruta: &str) -> bool {
        let Some(real) = self.resolver(ruta) else {
            return false;
        };
        if crate::audio::cargado().as_deref() != Some(real.as_path()) {
            if let Err(e) = crate::audio::cargar(&real, &self.titulo(ruta), true) {
                tracing::warn!(?e, ruta = %real.display(), "no se pudo tocar el audio de la nota");
                return false;
            }
            self.cargado = Some(ruta.to_string());
            self.pendiente = None;
        } else if !crate::audio::estado().sonando
            && let Err(e) = crate::audio::seguir()
        {
            tracing::warn!(?e, "no se pudo seguir el audio de la nota");
        }
        true
    }
}

impl inc::Medios for MediosDeLaNota {
    fn ficha(&mut self, ruta: &str) -> Option<Ficha> {
        if let Some((p, c)) = inc::mensaje_del_enlace(ruta) {
            return Some(self.ficha_de_mensaje(&p, &c));
        }
        if let Some((p, c)) = md_imagen::hoja_del_enlace(ruta) {
            return Some(self.ficha_de_hoja(&p, &c));
        }
        Some(self.ficha_de_fichero(ruta))
    }

    fn mensajes(&mut self) -> Vec<MensajeElegible> {
        let destino = self.actual.borrow().clone();
        let Some(f) = proyecto_de(&self.raiz, &destino) else {
            return Vec::new();
        };
        let propia = match &destino {
            Destino::Mensaje { codigo, .. } => Some(codigo.clone()),
            _ => None,
        };
        let raiz = self.raiz.clone();
        let ahora = pixpin_shell::entorno::ahora_local_ms();
        let mut v: Vec<&Mensaje> = self
            .leidos
            .de(&raiz, &f)
            .iter()
            .filter(|m| se_puede_enlazar(m))
            .collect();
        v.sort_by_key(|m| std::cmp::Reverse(m.cuando));
        v.into_iter()
            .filter(|m| Some(m.codigo_unico()) != propia)
            .take(EN_EL_MENU)
            .map(|m| {
                let hora =
                    pixpin_ui::chat::etiqueta_hora(pixpin_shell::entorno::a_local(m.cuando), ahora);
                let codigo = crate::ventana_chat::chapa_de_codigo(m).unwrap_or_default();
                let que: String = match m.clase {
                    Some(Clase::Voz) => {
                        format!("\u{1f3a4} {}", crate::biblioteca_audio::titulo_de_audio(m))
                    }
                    _ => texto_de(m)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(60)
                        .collect(),
                };
                MensajeElegible {
                    clave: m.codigo_unico(),
                    // El menu de Windows corta en `&`; el tabulador alinea
                    // la hora a la derecha.
                    rotulo: format!("{}\t{hora} {codigo}", que.replace('&', "&&")),
                }
            })
            .collect()
    }

    fn insertar_mensaje(&mut self, clave: &str) -> Option<String> {
        let f = proyecto_de(&self.raiz, &self.actual.borrow())?;
        let raiz = self.raiz.clone();
        let m = self
            .leidos
            .de(&raiz, &f)
            .iter()
            .find(|m| m.codigo_unico() == clave)
            .cloned()?;
        Some(bloque_de(&raiz, &f, &m, &self.textos))
    }

    fn audio(&mut self, orden: OrdenAudio) -> Option<EstadoAudio> {
        match orden {
            OrdenAudio::Mirar => {
                crate::audio::latido();
                match self.pendiente {
                    Some(Pendiente::Ms(ms)) if Self::ir_a_ms(ms) => self.pendiente = None,
                    Some(Pendiente::Fraccion(f)) if crate::audio::estado().duracion_ms > 0 => {
                        crate::audio::ir_a(f);
                        self.pendiente = None;
                    }
                    _ => {}
                }
            }
            OrdenAudio::Alternar(r) => {
                if let Some(real) = self.resolver(&r) {
                    if let Err(e) = crate::audio::alternar(&real, &self.titulo(&r)) {
                        tracing::warn!(?e, "no se pudo tocar el audio de la nota");
                    }
                    self.cargado = Some(r);
                }
            }
            OrdenAudio::Saltar(r, ms) => {
                let ya = self.cargado.as_deref() == Some(r.as_str()) && self.estado().is_some();
                if self.cargar(&r) && !(ya && Self::ir_a_ms(ms)) {
                    self.pendiente = Some(Pendiente::Ms(ms));
                }
            }
            OrdenAudio::IrA(r, f) => {
                if self.cargar(&r) && crate::audio::estado().duracion_ms > 0 {
                    crate::audio::ir_a(f);
                } else {
                    self.pendiente = Some(Pendiente::Fraccion(f));
                }
            }
            OrdenAudio::Velocidad => {
                crate::audio::otra_velocidad();
            }
        }
        self.estado()
    }

    fn pasar_a_texto(&mut self, ruta: &str) -> Result<(), String> {
        if self
            .transcribiendo
            .as_ref()
            .is_some_and(|(_, t)| !t.acabada())
        {
            return Err(self.textos.t("chat-transcribir-en-marcha"));
        }
        let Some(real) = self.resolver(ruta) else {
            return Err(self.textos.t("chat-voz-no-es-audio"));
        };
        let whisper = crate::voz::whisper_posible(&self.ubicacion, self.idioma);
        if !whisper
            && !crate::voz::windows_sabe(self.idioma)
            && let Some(aviso) = crate::ventana_chat::falta_para_transcribir(
                &self.ubicacion,
                &self.textos,
                self.idioma,
            )
        {
            return Err(aviso);
        }
        let en =
            crate::voz::pasar_a_texto(ruta, &real, &self.ubicacion, self.idioma, Vec::new(), None);
        self.transcribiendo = Some((ruta.to_string(), en));
        Ok(())
    }

    fn transcribiendo(&mut self) -> Option<Transcribiendo> {
        let (ruta, en) = self.transcribiendo.as_mut()?;
        let ruta = ruta.clone();
        match en.recoger() {
            Some(r) => {
                let hecho = r
                    .map(|t| t.texto)
                    .map_err(|e| crate::ventana_chat::razon_de_voz(&e, &self.textos));
                self.transcribiendo = None;
                Some(Transcribiendo {
                    ruta,
                    avance: 1.0,
                    hecho: Some(hecho),
                })
            }
            None => Some(Transcribiendo {
                ruta,
                avance: en.avance(),
                hecho: None,
            }),
        }
    }
}

#[cfg(test)]
mod pruebas;
