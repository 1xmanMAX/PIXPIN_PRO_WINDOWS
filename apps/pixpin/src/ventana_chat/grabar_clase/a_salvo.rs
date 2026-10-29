//! **Todo a salvo mientras se graba** (28-oct-2026, el usuario: «todo tiene
//! que ser rapido y fiable al grabar: si se perdiera algo seria malo, se
//! perderia una reunion»).
//!
//! El enganche de la barra con el diario de la grabacion
//! (`pixpin_proyecto::grabacion_a_medias`): lo que se hace grabando se
//! apunta en el disco **en el acto**, para que un corte antes de
//! «Terminar» no se lleve nada.
//!
//! - **Al empezar**: el diario, con el chat, el modo, el nombre, donde va la
//!   nota y el video, y la hora de pared ([`Estado::abrir_diario`]).
//! - **Los temas de la reunion**, al empezar y cada vez que se marca uno
//!   ([`Estado::apuntar_temas`]): la clase recuperada los lleva.
//! - **Cada captura o foto**: su fichero en la carpeta del diario y su
//!   minuto en el diario; en memoria se queda solo la ruta
//!   ([`Estado::anadir_captura`]). Cada clip, al empezar y al cerrarse.
//! - **Cada pocos segundos** ([`Estado::latir`]): el minuto de la nota con
//!   su hora (de ahi salen las pausas y los huecos de una suspension), lo
//!   que se anuncia al movil y **el espacio del disco**: con menos de 2 GB se
//!   avisa; con menos de 500 MB se cierra el video con lo grabado y la nota
//!   sigue, que pesa cien veces menos.
//! - **Al guardar bien**, el diario se borra ([`Estado::cerrar_diario`]); si
//!   algo no entro, se queda para recuperarlo al volver a abrir PixPin
//!   ([`super::recuperar`]). **Tirar** (✕ dos veces) es lo unico que lo
//!   borra con lo grabado.
//!
//! Un fallo al escribir el diario nunca para la grabacion: se dice en el
//! registro y se sigue (la nota y su respaldo van por su lado).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use pixpin_proyecto::grabacion_a_medias::{self as a_medias, Diario, Espacio};

use super::{Captura, Estado, Fase, Pedido, Rotulos, con, fecha_del_nombre, video};

/// Cada cuanto se apunta el latido en el disco.
const LATIDO: Duration = Duration::from_secs(5);
/// Cada cuanto se mira el espacio del disco.
const MIRAR_DISCO: Duration = Duration::from_secs(20);

/// La marca del diario de la grabacion en marcha: la recuperacion no la
/// toca aunque se abra a la vez.
static EN_MARCHA: Mutex<Option<String>> = Mutex::new(None);

/// Si el diario de `marca` es el de la grabacion que esta en marcha.
pub(super) fn en_marcha(marca: &str) -> bool {
    EN_MARCHA
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_deref()
        == Some(marca)
}

fn marcar(marca: Option<String>) {
    *EN_MARCHA.lock().unwrap_or_else(|e| e.into_inner()) = marca;
}

/// Lo que lleva la barra del diario.
#[derive(Default)]
pub(super) struct ASalvo {
    pub diario: Option<Diario>,
    dir: PathBuf,
    escrito: Option<Instant>,
    disco_mirado: Option<Instant>,
    espacio: Option<Espacio>,
}

impl ASalvo {
    fn guardar(&mut self) {
        if let Some(d) = &self.diario {
            match d.guardar(&self.dir) {
                Ok(()) => self.escrito = Some(Instant::now()),
                Err(e) => tracing::warn!(?e, "el diario de la grabacion no se escribio"),
            }
        }
    }
}

impl Estado {
    /// **Abre el diario** al pulsar «Empezar».
    pub(super) fn abrir_diario(&mut self, pedido: &Pedido) {
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        let (proyecto, nombre_proyecto) = pedido
            .proyectos
            .get(self.proyecto)
            .cloned()
            .unwrap_or_default();
        let nombre = con(
            &self.plantilla,
            &fecha_del_nombre(pixpin_shell::entorno::ahora_local_ms()),
        );
        let mut d = Diario::nuevo(ahora, &proyecto, &nombre, &self.temporal);
        d.nombre_proyecto = nombre_proyecto;
        // Ya no hay modos (29-oct-2026): se apunta como empezo, para quien
        // lea el diario.
        d.modo = self.modo_del_diario().palabra().to_string();
        d.microfono = self.microfono;
        d.equipo = self.equipo;
        if self.pantalla_al_empezar {
            d.video = Some(video::temporal_del_video(&self.temporal));
        }
        d.temas = self.temas_en_json();
        d.temas_lista = self.temas_lista.clone();
        self.a_salvo = ASalvo {
            dir: d.carpeta(pedido.ubicacion.raiz()),
            diario: Some(d),
            ..ASalvo::default()
        };
        marcar(self.a_salvo.diario.as_ref().map(|d| d.marca.clone()));
        self.a_salvo.guardar();
        // Antes de nada, cuanto cabe: sin sitio para el video, ni se empieza.
        // El aviso sale en el primer latido, ya con la barra.
        self.mirar_disco(None);
        self.a_salvo.disco_mirado = None;
    }

    /// **Una captura o foto, al disco en el acto** y a la lista. En memoria
    /// se queda la ruta, no los bytes (una reunion presencial son decenas de
    /// fotos de 12 megapixeles). Si no se puede escribir, se queda en
    /// memoria como antes.
    pub(super) fn anadir_captura(&mut self, mut c: Captura) {
        let s = &mut self.a_salvo;
        if let Some(d) = s.diario.as_mut() {
            match d.apuntar_captura(&s.dir, c.ms, &c.bytes, c.de_camara, c.extension()) {
                Ok(ruta) => {
                    c.bytes = Vec::new();
                    c.fichero = Some(ruta);
                    s.guardar();
                }
                Err(e) => tracing::warn!(?e, ms = c.ms, "una captura no se pudo poner a salvo"),
            }
        }
        self.capturas.push(c);
    }

    /// Los temas como van a la nota (`Null` sin temas).
    fn temas_en_json(&self) -> serde_json::Value {
        let mut r = serde_json::Map::new();
        pixpin_proyecto::temas::poner_temas(&mut r, &self.temas);
        r.remove(pixpin_proyecto::temas::TEMAS)
            .unwrap_or(serde_json::Value::Null)
    }

    /// **Los temas, al diario** (se marco uno, a mano o Jev).
    pub(super) fn apuntar_temas(&mut self) {
        let temas = self.temas_en_json();
        if let Some(d) = self.a_salvo.diario.as_mut() {
            d.temas = temas;
            self.a_salvo.guardar();
        }
    }

    /// Un clip empezo en `ms`, en `ruta`.
    pub(super) fn apuntar_clip_empezado(&mut self, ms: i64, ruta: &Path) {
        if let Some(d) = self.a_salvo.diario.as_mut() {
            d.empezar_clip(ms, ruta);
            self.a_salvo.guardar();
        }
    }

    /// Un trozo del video de la pantalla empezo en `ms`, en `ruta`
    /// (`super::pantalla`).
    pub(super) fn apuntar_trozo_empezado(&mut self, ms: i64, ruta: &Path) {
        if let Some(d) = self.a_salvo.diario.as_mut() {
            d.empezar_trozo_de_pantalla(ms, ruta);
            self.a_salvo.guardar();
        }
    }

    /// Un trozo de la pantalla se cerro y dura `duracion_ms`.
    pub(super) fn apuntar_trozo_cerrado(&mut self, ruta: &Path, duracion_ms: i64) {
        if let Some(d) = self.a_salvo.diario.as_mut() {
            d.cerrar_clip(ruta, duracion_ms);
            self.a_salvo.guardar();
        }
    }

    /// Un clip se cerro.
    pub(super) fn apuntar_clip(&mut self, c: &super::camara::ClipHecho) {
        if let Some(d) = self.a_salvo.diario.as_mut() {
            d.cerrar_clip(&c.ruta, c.duracion_ms);
            self.a_salvo.guardar();
        }
    }

    /// **El latido**, a cada vuelta de la barra: lo apunta cada pocos
    /// segundos (en el acto si cambio la pausa o hubo un hueco) y mira el
    /// disco.
    pub(super) fn latir(&mut self, pedido: &Pedido, r: &Rotulos) {
        let Fase::Grabando(g) = &self.fase else {
            return;
        };
        let (ms, pausa) = (g.llevado_ms(), g.en_pausa());
        let Some(d) = self.a_salvo.diario.as_mut() else {
            return;
        };
        let huecos = d.reloj.huecos.len();
        let cambio = d.latir(pixpin_shell::entorno::ahora_utc_ms(), ms, pausa);
        if d.movil.is_none() {
            d.movil = crate::sincronizar::camara::en_curso(pedido.ubicacion.raiz());
        }
        let suspendido = d
            .reloj
            .huecos
            .get(huecos)
            .and_then(|t| t.hasta.map(|h| h - t.desde));
        if cambio || self.a_salvo.escrito.is_none_or(|t| t.elapsed() >= LATIDO) {
            self.a_salvo.guardar();
        }
        if let Some(hueco_ms) = suspendido {
            // El equipo se suspendio (la tapa, la bateria): la nota siguio
            // al volver, sin ese rato. Se dice, para que nadie lo busque.
            tracing::info!(hueco_ms, "la grabacion siguio tras una suspension");
            let minutos = ((hueco_ms + 30_000) / 60_000).max(1);
            self.avisar(con(&r.suspendido, &minutos.to_string()));
        }
        if self
            .a_salvo
            .disco_mirado
            .is_none_or(|t| t.elapsed() >= MIRAR_DISCO)
        {
            self.mirar_disco(Some(r));
        }
    }

    /// **Cuanto cabe en el disco** de la nota. Con poco, se avisa una vez;
    /// casi lleno, se cierra el video (la nota sigue). `r` es `None` al
    /// empezar, cuando aun no hay barra donde avisar (se avisa al primer
    /// latido).
    fn mirar_disco(&mut self, r: Option<&Rotulos>) {
        self.a_salvo.disco_mirado = Some(Instant::now());
        let carpeta = self.temporal.parent().unwrap_or(Path::new("."));
        let ahora = a_medias::espacio(pixpin_sistema::archivos::espacio_libre(carpeta));
        if ahora == Espacio::Critico {
            // El video que va se cierra con lo grabado y no empieza otro (ni
            // el del principio, si aun se esta abriendo).
            if let Some(v) = self.video_en_marcha() {
                v.cortar();
            }
            self.sin_video_por_disco = true;
        }
        let Some(r) = r else {
            return;
        };
        let antes = self.a_salvo.espacio.replace(ahora);
        if antes != Some(ahora) {
            match ahora {
                Espacio::Poco => self.avisar(r.disco_poco.clone()),
                Espacio::Critico => self.avisar(r.disco_lleno.clone()),
                Espacio::Bien => {}
            }
        }
    }

    /// **Despues de guardar**: si entro todo, el diario sobra; si la nota no
    /// se pudo cerrar pero quedo su respaldo, el diario se queda solo con la
    /// nota (lo demas ya entro); si no se pudo guardar nada, se queda entero.
    pub(super) fn cerrar_diario(&mut self, guardado: &super::Guardado) {
        let s = std::mem::take(&mut self.a_salvo);
        marcar(None);
        let Some(mut d) = s.diario else {
            return;
        };
        d.acabar(pixpin_shell::entorno::ahora_utc_ms());
        if guardado.fallo_guardar.is_some() {
            tracing::warn!(dir = %s.dir.display(), "la clase queda en su diario para recuperarla");
            if let Err(e) = d.guardar(&s.dir) {
                tracing::warn!(?e, "el diario de la grabacion no se escribio");
            }
            return;
        }
        let corta = matches!(
            guardado.fallo_audio,
            Some(pixpin_audio::ErrorAudio::DemasiadoCorta { .. })
        );
        if guardado.fallo_audio.is_some()
            && !corta
            && pixpin_audio::hay_algo_que_recuperar(&d.audio)
        {
            d.solo_la_nota = true;
            tracing::warn!(dir = %s.dir.display(), "la nota de la clase queda para recuperarla");
            if let Err(e) = d.guardar(&s.dir) {
                tracing::warn!(?e, "el diario de la grabacion no se escribio");
            }
            return;
        }
        let _ = std::fs::remove_dir_all(&s.dir);
    }

    /// **Tirar** (lo pidio el usuario) o no llego a grabarse nada: el diario
    /// fuera.
    pub(super) fn tirar_diario(&mut self) {
        let s = std::mem::take(&mut self.a_salvo);
        marcar(None);
        if s.diario.is_some() {
            let _ = std::fs::remove_dir_all(&s.dir);
        }
    }

    /// Si hay un diario abierto (se esta grabando a salvo).
    pub(super) fn con_diario(&self) -> bool {
        self.a_salvo.diario.is_some()
    }
}
