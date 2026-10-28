//! **El movil como camara de la reunion que se graba aqui** (27-oct-2026,
//! el usuario: «agregar una funcion especial para conectar con Android, para
//! que el MOVIL tome las fotos y se envien directamente y se adjunten aqui»).
//!
//! El cable es `pixpin_sincro::camara` y lo que se hace con cada foto,
//! `pixpin_proyecto::camara_del_movil`; aqui esta lo de este equipo:
//!
//! - **El enganche con «Grabar clase»** ([`latido`]): la barra lo llama a
//!   cada vuelta con el minuto de la grabadora y si esta en pausa. Al
//!   empezar, pausar, seguir o acabar, se avisa a los moviles del grupo
//!   ([`avisar_a_los_moviles`]); el movil, ademas, pregunta solo cada pocos
//!   segundos (`camara_remota`), que es lo que de verdad le llega si no
//!   escucha. Devuelve el aviso de la barra al llegar una foto («Foto del
//!   móvil · 12:34»).
//! - **Al guardarse la clase** ([`guardada`]): lo que llego grabando entra
//!   en su minuto. Lo que llega despues entra solo y se avisa en el globo.
//! - **Quien atiende al movil** ([`RecepcionPc`]): la da `al_frente` a cada
//!   conexion del grupo; refresca el chat al entrar algo.
//! - **El interruptor** de la barra ([`alternar`], [`indicador`]).
//! - **Por su hora** (28-oct-2026, `super::por_su_hora`): el reloj de la
//!   grabacion se apunta en disco al empezar, pausar, seguir, acabar y cada
//!   [`APUNTAR_CADA`]; al guardarse entra en la nota y se repasan los chats
//!   (lo que llego sin conexion entra en su minuto).
//! - **La grabacion del movil** ([`RecepcionPc::anuncio_ajeno`]): cuando el
//!   movil graba y lo dice, sale el boton flotante para mandarle capturas
//!   (`super::aporte`).

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use pixpin_proyecto::camara_del_movil::{Camaras, Destino, Recepcion, minuto};
use pixpin_sincro::al_lienzo::{Conexion, Suelto};
use pixpin_sincro::camara::{self, Anuncio, Camara};
use pixpin_store::{Catalogo, Idioma, Ubicacion};

use super::al_movil;

static CAMARAS: OnceLock<Arc<Mutex<Camaras>>> = OnceLock::new();

/// Cada cuanto se apunta el reloj de la grabacion en disco: si el programa
/// se cae, la ventana de la grabacion acaba como mucho esto antes.
const APUNTAR_CADA: std::time::Duration = std::time::Duration::from_secs(15);

/// Lo de la barra: si ya se empezo a anunciar esta grabacion.
static BARRA: Mutex<Option<Barra>> = Mutex::new(None);

struct Barra {
    idioma: Idioma,
    textos: Catalogo,
    /// Si hay moviles en el grupo (leido al empezar): sin ellos, la barra no
    /// ensena nada de esto.
    hay_moviles: bool,
    /// El rotulo de la barra, y de que (encendida, cuantas) salio: se
    /// rehace solo cuando cambia, no a cada fotograma.
    rotulo: Option<((bool, usize), String)>,
    /// Cuando se apunto el reloj en disco por ultima vez.
    apuntado: std::time::Instant,
}

/// Las camaras de este equipo, creadas la primera vez.
pub(crate) fn camaras(raiz: &Path) -> Arc<Mutex<Camaras>> {
    CAMARAS
        .get_or_init(|| {
            let nombre = super::leer_identidad(raiz)
                .map(|i| i.yo.nombre)
                .ok()
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(super::nombre_del_equipo);
            let mut c = Camaras::nuevas(super::carpeta_sincro(raiz).join("camara-movil"), &nombre);
            // Por su hora: el reloj lleva el codigo de este equipo, y lo que
            // no sabe de que grabacion es se busca en las guardadas.
            c.codigo = crate::letras_que_viajan::aparato(raiz);
            c.almacen = Some((
                raiz.to_path_buf(),
                crate::ventana_chat::descripcion::hora_sin_repetir,
            ));
            // HEVC solo si este equipo lo sabe ver: si no, el movil graba
            // los clips en H.264, que Windows trae de serie.
            if pixpin_pin::video::sabe_hevc() {
                c.codecs.push(camara::HEVC.into());
            }
            Arc::new(Mutex::new(c))
        })
        .clone()
}

fn con<T>(raiz: &Path, f: impl FnOnce(&mut Camaras) -> T) -> T {
    let c = camaras(raiz);
    let mut g = c.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut g)
}

/// Lo que dice la barra de «Grabar clase» a cada vuelta.
pub(crate) struct Latido<'a> {
    pub ubicacion: &'a Ubicacion,
    pub idioma: Idioma,
    /// «Clase 27-10 10h05».
    pub titulo: &'a str,
    /// Grabando: el minuto de la grabadora y si esta en pausa.
    pub grabando: Option<(i64, bool)>,
}

/// **El enganche con la barra.** Devuelve el aviso de lo que acaba de
/// llegar del movil, si llego algo.
pub(crate) fn latido(l: Latido<'_>) -> Option<String> {
    let raiz = l.ubicacion.raiz();
    let ahora = super::ahora_ms();
    let mut barra = BARRA.lock().unwrap_or_else(|e| e.into_inner());
    let avisar = match (l.grabando, barra.is_some()) {
        (Some((ms, _)), false) => {
            *barra = Some(Barra {
                idioma: l.idioma,
                textos: Catalogo::nuevo(l.idioma),
                hay_moviles: !al_movil::aparatos_de(raiz).is_empty(),
                rotulo: None,
                apuntado: std::time::Instant::now(),
            });
            let id = format!("g-{ahora}");
            con(raiz, |c| {
                c.empezar(&id, l.titulo, ahora, ms);
                apuntar(c, ahora);
            });
            true
        }
        (Some((ms, pausa)), true) => {
            let cada = barra
                .as_mut()
                .is_some_and(|b| b.apuntado.elapsed() >= APUNTAR_CADA);
            let cambio = con(raiz, |c| {
                let cambio = c.seguir_a(ahora, ms, pausa);
                if cambio || cada {
                    apuntar(c, ahora);
                }
                cambio
            });
            if (cambio || cada)
                && let Some(b) = barra.as_mut()
            {
                b.apuntado = std::time::Instant::now();
            }
            cambio
        }
        (None, true) => {
            *barra = None;
            con(raiz, |c| c.acabar(ahora));
            true
        }
        // Sin grabar y sin nada de antes: ni se crean las camaras.
        (None, false) if CAMARAS.get().is_none() => return None,
        (None, false) => false,
    };
    if avisar {
        avisar_a_los_moviles(raiz);
    }
    let nuevas = con(raiz, Camaras::tomar_nuevas);
    let ultima = nuevas.last()?;
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("minuto", minuto(ultima.ms));
    let clave = if ultima.clip {
        "clase-clip-movil"
    } else {
        "clase-foto-movil"
    };
    Some(match barra.as_ref() {
        Some(b) => b.textos.t_args(clave, &args),
        None => Catalogo::nuevo(l.idioma).t_args(clave, &args),
    })
}

/// **La clase se guardo**: lo que llego del movil grabando entra ya, en su
/// minuto. `voz` y `carpeta`, los mensajes de la nota y de su carpeta (si
/// entraron).
pub(crate) fn guardada(
    raiz: &Path,
    proyecto: &str,
    voz: Option<&str>,
    carpeta: Option<&str>,
    duracion_ms: i64,
) {
    let destino = Destino {
        raiz: raiz.to_path_buf(),
        proyecto: proyecto.to_string(),
        voz: voz.map(str::to_string),
        carpeta: carpeta.map(str::to_string),
        aparato: crate::letras_que_viajan::aparato(raiz),
        hora: crate::ventana_chat::descripcion::hora_sin_repetir,
    };
    let ahora = super::ahora_ms();
    // Por si la barra no llego a ver el final (se guardo al cerrarla).
    if let Ok(mut b) = BARRA.lock()
        && b.take().is_some()
    {
        con(raiz, |c| c.acabar(ahora));
        avisar_a_los_moviles(raiz);
    }
    let hechas = con(raiz, |c| c.guardada(destino, Some(duracion_ms), ahora));
    if hechas > 0 {
        tracing::info!(hechas, "fotos del movil en la clase");
        crate::ventana_chat::refrescar();
    }
    // Con el reloj ya en la nota: lo que llego sin conexion mientras se
    // grababa (por la sincronizacion) entra ahora en su minuto.
    super::por_su_hora::repasar(raiz);
}

/// Apunta en disco el reloj de la grabacion en curso.
fn apuntar(c: &Camaras, ahora: i64) {
    if let Err(e) = c.apuntar_reloj(ahora) {
        tracing::debug!(?e, "no se apunto el reloj de la grabacion");
    }
}

/// **La grabacion se tiro**: lo que llego del movil, tambien.
pub(crate) fn tirada(raiz: &Path) {
    if let Ok(mut b) = BARRA.lock() {
        b.take();
    }
    con(raiz, Camaras::tirar);
    avisar_a_los_moviles(raiz);
}

/// El interruptor de la barra: enciende o apaga la camara del movil.
pub(crate) fn alternar(raiz: &Path) {
    let encendida = con(raiz, |c| {
        c.encendida = !c.encendida;
        c.encendida
    });
    tracing::info!(encendida, "camara del movil");
    avisar_a_los_moviles(raiz);
}

/// **Lo que ensena la barra** mientras se graba: «📱 Cámara del móvil · 2»
/// y si esta encendida. `None` sin grabar o sin moviles en el grupo: no hay
/// a quien pedirselo.
pub(crate) fn indicador_actual() -> Option<(String, bool)> {
    let mut barra = BARRA.lock().unwrap_or_else(|e| e.into_inner());
    let b = barra.as_mut().filter(|b| b.hay_moviles)?;
    let clave = {
        let c = CAMARAS.get()?.lock().unwrap_or_else(|e| e.into_inner());
        (c.encendida, c.recibidas())
    };
    if b.rotulo.as_ref().is_none_or(|(k, _)| *k != clave) {
        let (encendida, n) = clave;
        let texto = if !encendida {
            b.textos.t("clase-camara-movil-apagada")
        } else if n == 0 {
            b.textos.t("clase-camara-movil")
        } else {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("n", n as i64);
            b.textos.t_args("clase-camara-movil-n", &args)
        };
        b.rotulo = Some((clave, texto));
    }
    b.rotulo.as_ref().map(|((e, _), t)| (t.clone(), *e))
}

/// **Avisa a los moviles del grupo** de como va la grabacion, en un hilo:
/// a los que contesten en su ultima direccion (sin buscarlos por la red: el
/// movil pregunta solo, y esto solo adelanta el aviso). Mide de paso el
/// desfase de cada reloj.
fn avisar_a_los_moviles(raiz: &Path) {
    let raiz = raiz.to_path_buf();
    let lanzado = std::thread::Builder::new()
        .name("camara-movil".into())
        .spawn(move || {
            let Some(anuncio) = con(&raiz, |c| c.anuncio(super::ahora_ms())) else {
                return;
            };
            for otro in al_movil::aparatos_de(&raiz) {
                match avisar_a(&raiz, &otro, &anuncio) {
                    Ok(Some(desfase)) => {
                        con(&raiz, |c| c.medir(&otro.id, desfase));
                        super::por_su_hora::medido(&raiz, &otro.id, desfase);
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::debug!(movil = %otro.nombre, %e, "camara del movil: sin aviso")
                    }
                }
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el aviso a la camara del movil");
    }
}

fn avisar_a(
    raiz: &Path,
    otro: &al_movil::AparatoDelGrupo,
    anuncio: &Anuncio,
) -> anyhow::Result<Option<i64>> {
    let Some(mut c) = abrir_con(raiz, otro)? else {
        return Ok(None);
    };
    let desfase = c.anunciar_camara(anuncio, &super::ahora_ms)?;
    c.adios();
    Ok(Some(desfase))
}

/// **Abre el canal de la camara con un aparato del grupo** en su ultima
/// direccion. `None` si no tiene direccion o no sabe de la camara
/// (`camara1`).
pub(super) fn abrir_con(
    raiz: &Path,
    otro: &al_movil::AparatoDelGrupo,
) -> anyhow::Result<Option<Conexion<std::net::TcpStream>>> {
    let Some((host, puerto)) = otro.direccion.clone() else {
        return Ok(None);
    };
    let id = super::leer_identidad(raiz)?;
    let codigo = id.codigo.clone().unwrap_or_default();
    let flujo = super::conectar(&host, puerto)?;
    let nonce = super::nonce().ok_or_else(|| anyhow::anyhow!("sin azar"))?;
    let hola = pixpin_sincro::mensajes::Hola {
        yo: super::al_cable(&id.yo),
        miembros: id.miembros.iter().map(super::al_cable).collect(),
        reloj: super::ahora_ms(),
        puerto: super::presencia::puerto().map_or(0, u32::from),
        capacidades: pixpin_sincro::CAPACIDADES_CON_CAMARA
            .iter()
            .map(|c| c.to_string())
            .collect(),
        ..Default::default()
    };
    let c = Conexion::abrir(flujo, &codigo, nonce, hola)?;
    if !c.sabe_de_camara() {
        c.adios();
        return Ok(None);
    }
    Ok(Some(c))
}

/// **Quien atiende la camara del movil** en una conexion del grupo: el de
/// `pixpin_proyecto`, y al entrar algo refresca el chat y, si ya no se
/// graba (llego tarde), lo dice en el globo.
pub(crate) struct RecepcionPc {
    dentro: Recepcion,
    raiz: std::path::PathBuf,
}

impl RecepcionPc {
    pub(crate) fn nueva(raiz: &Path, de_id: &str, de_nombre: &str) -> RecepcionPc {
        RecepcionPc {
            dentro: Recepcion {
                camaras: camaras(raiz),
                de_id: de_id.to_string(),
                de_nombre: de_nombre.to_string(),
                ahora: super::ahora_ms,
            },
            raiz: raiz.to_path_buf(),
        }
    }
}

impl Camara for RecepcionPc {
    fn anuncio(&mut self) -> Option<Anuncio> {
        self.dentro.anuncio()
    }

    fn aceptar_foto(&mut self, p: &Suelto) -> Result<(), String> {
        self.dentro.aceptar_foto(p)
    }

    fn anuncio_ajeno(&mut self, de: &pixpin_sincro::mensajes::Aparato, a: &Anuncio, desfase: i64) {
        let ahora = super::ahora_ms();
        let nueva = con(&self.raiz, |c| {
            c.apuntar_ajena(&de.id, &de.nombre, a, desfase, ahora)
        });
        super::por_su_hora::medido(&self.raiz, &de.id, desfase);
        if nueva {
            tracing::info!(movil = %de.nombre, grabacion = %a.grabacion, "el movil graba: boton para mandarle capturas");
            super::aporte::lanzar(&self.raiz);
        }
    }

    fn guardar_foto(&mut self, p: &Suelto, fichero: &Path) -> Result<i64, String> {
        let ms = self.dentro.guardar_foto(p, fichero)?;
        let grabando = con(&self.raiz, |c| c.en_curso() == Some(p.grabacion.as_str()));
        if !grabando {
            // Llego tarde: entro ya en la clase guardada. La barra ya no
            // esta para decirlo.
            crate::ventana_chat::refrescar();
            let idioma = BARRA
                .lock()
                .ok()
                .and_then(|b| b.as_ref().map(|b| b.idioma))
                .unwrap_or_else(crate::salida::idioma_actual);
            let titulo = con(&self.raiz, |c| {
                c.anuncio(super::ahora_ms())
                    .map(|a| a.titulo)
                    .unwrap_or_default()
            });
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("titulo", titulo);
            args.set("minuto", minuto(ms));
            al_movil::avisar(Catalogo::nuevo(idioma).t_args("camara-movil-tarde", &args));
        }
        Ok(ms)
    }
}
