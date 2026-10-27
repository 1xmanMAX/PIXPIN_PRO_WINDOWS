//! **La camara de la reunion presencial, fuera del hilo de la barra**
//! (27-oct-2026).
//!
//! Como el [`super::fotografo`]: la barra encarga y sigue pintando; la
//! camara (abrirla tarda un segundo, una foto de 12 megapixeles otro) vive
//! en su hilo, en el apartamento multihilo que pide `MediaCapture`, y lo
//! hecho vuelve por un canal con el minuto en que se pulso.
//!
//! - **Abrir** se encarga al empezar a grabar: la primera foto ya no espera
//!   a la camara. La foto se pone a la mayor resolucion que tenga
//!   ([`presencial::mejor_modo`]).
//! - **Foto**: un JPEG tal cual sale, sin reducir.
//! - **Clip**: el flujo de video se pone a 1080p30 como mucho
//!   ([`presencial::modo_para_clip`]), se graba a un MP4 (HEVC si la grafica
//!   lo codifica, si no H.264) y al pararlo se devuelve la foto a lo mas
//!   grande: en muchas camaras los dos flujos son el mismo.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use pixpin_capture::camara::{self, Camara, Flujo, InfoCamara, ModoCamara, OpcionesClip};
use pixpin_proyecto::presencial::{self, FalloDeCamara, ModoDeCamara};

enum Encargo {
    Abrir { id: String, con_sonido: bool },
    Foto(i64),
    EmpezarClip { ms: i64, ruta: PathBuf },
    PararClip,
}

/// Un clip ya cerrado: cuando empezo en la nota, cuanto dura y donde esta.
#[derive(Debug, Clone)]
pub(super) struct ClipHecho {
    pub ms: i64,
    pub duracion_ms: i64,
    pub ruta: PathBuf,
}

/// Lo que vuelve del hilo de la camara.
pub(super) enum Vuelta {
    /// La camara abierta, con el tamano de sus fotos; o por que no.
    Abierta(Result<(u32, u32), FalloDeCamara>),
    /// Una foto en JPEG, con el minuto en que se pulso.
    Foto(i64, Result<Vec<u8>, FalloDeCamara>),
    /// El clip empezo (y si va en HEVC), o por que no.
    ClipEmpezado(Result<bool, FalloDeCamara>),
    /// El clip se cerro (o se cerro mal: `None`, y su fichero se borra).
    ClipParado(Option<ClipHecho>),
}

#[derive(Default)]
pub(super) struct Camarografo {
    canal: Option<(Sender<Encargo>, Receiver<Vuelta>)>,
    /// Fotos encargadas que aun no han vuelto.
    fotos: usize,
    /// Grabando un clip: desde cuando (para el contador de la barra).
    clip: Option<Instant>,
}

impl Camarografo {
    /// Abre la camara `id` (con el microfono, para el sonido de los clips).
    pub(super) fn abrir(&mut self, id: &str) -> bool {
        self.encargar(Encargo::Abrir {
            id: id.to_string(),
            con_sonido: true,
        })
    }

    /// Encarga una foto en el milisegundo `ms`.
    pub(super) fn foto(&mut self, ms: i64) -> bool {
        let hecho = self.encargar(Encargo::Foto(ms));
        if hecho {
            self.fotos += 1;
        }
        hecho
    }

    /// Empieza un clip en `ruta`, que empieza en el milisegundo `ms`.
    pub(super) fn empezar_clip(&mut self, ms: i64, ruta: PathBuf) -> bool {
        if self.clip.is_some() {
            return true;
        }
        let hecho = self.encargar(Encargo::EmpezarClip { ms, ruta });
        if hecho {
            self.clip = Some(Instant::now());
        }
        hecho
    }

    /// Para el clip que se este grabando.
    pub(super) fn parar_clip(&mut self) {
        if self.clip.take().is_some() {
            let _ = self.encargar(Encargo::PararClip);
        }
    }

    /// Lo que lleva el clip, si se graba uno.
    pub(super) fn clip_llevado(&self) -> Option<Duration> {
        self.clip.map(|d| d.elapsed())
    }

    /// Cuantas fotos faltan por volver.
    pub(super) fn pendientes(&self) -> usize {
        self.fotos
    }

    /// Lo que ya volvio, sin esperar.
    pub(super) fn recoger(&mut self) -> Vec<Vuelta> {
        let Some((_, rx)) = &self.canal else {
            return Vec::new();
        };
        let v: Vec<Vuelta> = rx.try_iter().collect();
        self.contar(&v);
        v
    }

    fn contar(&mut self, v: &[Vuelta]) {
        for vuelta in v {
            match vuelta {
                Vuelta::Foto(..) => self.fotos = self.fotos.saturating_sub(1),
                Vuelta::ClipEmpezado(Err(_)) => self.clip = None,
                _ => {}
            }
        }
    }

    /// **Al terminar**: para el clip, espera las fotos que faltan y el
    /// cierre del clip (como mucho `tope`) y cierra la camara. Una foto del
    /// ultimo segundo de la reunion no se puede perder.
    pub(super) fn acabar(&mut self, tope: Duration) -> Vec<Vuelta> {
        self.parar_clip();
        let Some((tx, rx)) = self.canal.take() else {
            return Vec::new();
        };
        drop(tx);
        let hasta = Instant::now() + tope;
        let mut v = Vec::new();
        // Hasta que el hilo suelte su lado (lo hace al acabar los encargos).
        loop {
            let queda = hasta.saturating_duration_since(Instant::now());
            match rx.recv_timeout(queda) {
                Ok(vuelta) => v.push(vuelta),
                Err(_) => break,
            }
        }
        self.contar(&v);
        if self.fotos > 0 {
            tracing::warn!(
                faltan = self.fotos,
                "fotos de la reunion que no llegaron a tiempo"
            );
        }
        self.fotos = 0;
        self.clip = None;
        v
    }

    fn encargar(&mut self, e: Encargo) -> bool {
        if self.canal.is_none() {
            self.canal = lanzar();
        }
        let Some((tx, _)) = &self.canal else {
            return false;
        };
        if tx.send(e).is_err() {
            self.canal = None;
            return false;
        }
        true
    }
}

/// **Las camaras del equipo**, buscadas en su hilo (tarda un momento): la
/// lista llega por el canal; vacia si no hay o no se pudo mirar.
pub(super) fn buscar_camaras() -> Receiver<Vec<InfoCamara>> {
    let (tx, rx) = channel();
    let lanzado = std::thread::Builder::new()
        .name("clase-camaras".into())
        .spawn(move || {
            let _com = camara::Apartamento::iniciar();
            let v = camara::camaras().unwrap_or_else(|e| {
                tracing::info!(?e, "no se pudieron mirar las camaras");
                Vec::new()
            });
            let _ = tx.send(v);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de buscar camaras");
    }
    rx
}

/// Lo de la camara en las cuentas puras.
fn modos_puros(v: &[ModoCamara]) -> Vec<ModoDeCamara> {
    v.iter()
        .map(|m| ModoDeCamara {
            ancho: m.ancho,
            alto: m.alto,
            formato: m.formato.clone(),
            fps: m.fps,
        })
        .collect()
}

/// Pone las fotos a lo mas grande que tenga la camara. Devuelve el tamano.
fn fotos_a_lo_mas_grande(c: &Camara) -> (u32, u32) {
    let modos = c.modos(Flujo::Foto);
    match presencial::mejor_modo(&modos_puros(&modos)) {
        Some(i) => {
            if let Err(e) = c.poner_modo(Flujo::Foto, i) {
                tracing::info!(?e, "la camara se queda en su resolucion de fotos");
            }
            (modos[i].ancho, modos[i].alto)
        }
        None => (0, 0),
    }
}

fn fallo(e: &camara::ErrorCamara) -> FalloDeCamara {
    presencial::fallo_de_codigo(e.codigo)
}

fn lanzar() -> Option<(Sender<Encargo>, Receiver<Vuelta>)> {
    let (tx, encargos) = channel::<Encargo>();
    let (vueltas, rx) = channel::<Vuelta>();
    let lanzado = std::thread::Builder::new()
        .name("clase-camara".into())
        .spawn(move || {
            let _com = camara::Apartamento::iniciar();
            let mut abierta: Option<(String, Camara)> = None;
            let mut fallo_al_abrir: Option<FalloDeCamara> = None;
            let mut clip: Option<(i64, PathBuf, Instant)> = None;
            // HEVC solo con codificador en la grafica: por software pesa en
            // un equipo modesto y el H.264 lo reproduce cualquiera.
            let hevc = pixpin_motor_video::hay_hevc_por_hardware();
            let parar = |c: &mut Camara, clip: &mut Option<(i64, PathBuf, Instant)>| {
                let (ms, ruta, desde) = clip.take()?;
                let duracion_ms = desde.elapsed().as_millis() as i64;
                match c.parar_clip() {
                    Ok(()) if ruta.is_file() => Some(ClipHecho {
                        ms,
                        duracion_ms,
                        ruta,
                    }),
                    otro => {
                        tracing::warn!(?otro, "un clip de la reunion no se cerro");
                        let _ = std::fs::remove_file(&ruta);
                        None
                    }
                }
            };
            for e in encargos {
                let vuelta = match e {
                    Encargo::Abrir { id, con_sonido } => {
                        if abierta.as_ref().is_some_and(|(a, _)| *a == id) {
                            continue;
                        }
                        abierta = None;
                        match Camara::abrir(&id, con_sonido) {
                            Ok(c) => {
                                let tam = fotos_a_lo_mas_grande(&c);
                                tracing::info!(?tam, "camara de la reunion abierta");
                                abierta = Some((id, c));
                                fallo_al_abrir = None;
                                Vuelta::Abierta(Ok(tam))
                            }
                            Err(e) => {
                                tracing::warn!(?e, "no se pudo abrir la camara");
                                let f = fallo(&e);
                                fallo_al_abrir = Some(f);
                                Vuelta::Abierta(Err(f))
                            }
                        }
                    }
                    Encargo::Foto(ms) => match abierta.as_mut() {
                        Some((_, c)) => match c.foto_jpeg() {
                            Ok(jpg) => Vuelta::Foto(ms, Ok(jpg)),
                            Err(e) => {
                                tracing::warn!(?e, "la camara no hizo la foto");
                                Vuelta::Foto(ms, Err(fallo(&e)))
                            }
                        },
                        None => Vuelta::Foto(
                            ms,
                            Err(fallo_al_abrir.unwrap_or(FalloDeCamara::SinCamara)),
                        ),
                    },
                    Encargo::EmpezarClip { ms, ruta } => match abierta.as_mut() {
                        Some((_, c)) => {
                            let modos = c.modos(Flujo::Video);
                            let (mut w, mut h) = (1920, 1080);
                            if let Some(i) = presencial::modo_para_clip(&modos_puros(&modos)) {
                                if let Err(e) = c.poner_modo(Flujo::Video, i) {
                                    tracing::info!(?e, "el clip va con el modo de la camara");
                                } else {
                                    (w, h) = (modos[i].ancho, modos[i].alto);
                                }
                            }
                            let o = OpcionesClip {
                                hevc,
                                bits: presencial::bits_del_clip(w, h, hevc),
                                bits_sonido: presencial::CLIP_BITS_SONIDO,
                            };
                            match c.empezar_clip(&ruta, o) {
                                Ok(en_hevc) => {
                                    tracing::info!(w, h, en_hevc, "clip de la reunion empezado");
                                    clip = Some((ms, ruta, Instant::now()));
                                    Vuelta::ClipEmpezado(Ok(en_hevc))
                                }
                                Err(e) => {
                                    tracing::warn!(?e, "el clip no empezo");
                                    let _ = std::fs::remove_file(&ruta);
                                    let _ = fotos_a_lo_mas_grande(c);
                                    Vuelta::ClipEmpezado(Err(fallo(&e)))
                                }
                            }
                        }
                        None => Vuelta::ClipEmpezado(Err(
                            fallo_al_abrir.unwrap_or(FalloDeCamara::SinCamara)
                        )),
                    },
                    Encargo::PararClip => match abierta.as_mut() {
                        Some((_, c)) => {
                            let hecho = parar(c, &mut clip);
                            // Las fotos vuelven a lo mas grande.
                            let _ = fotos_a_lo_mas_grande(c);
                            Vuelta::ClipParado(hecho)
                        }
                        None => Vuelta::ClipParado(None),
                    },
                };
                if vueltas.send(vuelta).is_err() {
                    break;
                }
            }
            // Sin mas encargos (se termino): un clip abierto se cierra bien.
            if let Some((_, c)) = abierta.as_mut()
                && clip.is_some()
            {
                let hecho = parar(c, &mut clip);
                let _ = vueltas.send(Vuelta::ClipParado(hecho));
            }
        });
    match lanzado {
        Ok(_) => Some((tx, rx)),
        Err(e) => {
            tracing::warn!(?e, "no se pudo lanzar el hilo de la camara");
            None
        }
    }
}
