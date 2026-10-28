//! **Recuperar una grabacion cortada** al volver a abrir PixPin (28-oct-2026,
//! grabar sin perder nada).
//!
//! Si quedo algun diario de grabacion (`pixpin_proyecto::grabacion_a_medias`)
//! es que una clase o reunion no llego a guardarse: PixPin murio, se fue la
//! luz, Windows se reinicio o el disco fallo al guardar. Al arrancar se mira
//! (una lectura de carpeta; sin diarios no se lanza ni un hilo) y, por cada
//! uno, en su propio hilo:
//!
//! > **Se cortó una grabación** (Clase 28-10 10h05, 47 min) en «Fisica».
//! > Sí: Recuperar · No: Descartar · Cancelar: preguntar luego
//!
//! - **Recuperar**: la nota (la ya cerrada, el `.parte` si llego a
//!   cerrarse, o rehecha del respaldo WAV), el video (rehecho a MP4 normal
//!   si quedo fragmentado), las capturas y fotos del diario, los clips de la
//!   camara y lo que llego del movil, cada uno en su minuto, entran en el
//!   chat por el mismo camino que «Terminar» ([`super::guardar`]): una
//!   carpeta con todo dentro. Despues se pasa a texto de fondo.
//! - **Descartar** se vuelve a preguntar (con «No» por defecto) y solo
//!   entonces se borra lo grabado. Es lo unico que lo borra.
//! - **Cancelar**: nada; se pregunta otra vez al proximo arranque.
//!
//! Si recuperar falla, el diario y lo grabado se quedan como estaban.

use std::path::Path;

use pixpin_motor_video::archivo::recuperar_video;
use pixpin_proyecto::grabacion_a_medias::{
    self as a_medias, Diario, Nota, Plan, Video, con_sufijo, hallar, pendientes, plan,
};
use pixpin_shell::dialogo::{Eleccion, confirmar_al_frente, elegir_al_frente};
use pixpin_store::{Catalogo, Idioma, Ubicacion};

use super::camara::ClipHecho;
use super::{Captura, Estado, Fase, Modo, Pedido, empezar_el_texto, guardar, texto_de_fondo};

/// Donde van el nombre, los minutos y el chat en los textos.
const NOMBRE: &str = "\u{1}";
const MINUTOS: &str = "\u{2}";
const CHAT: &str = "\u{3}";

/// Los textos de la recuperacion, traducidos en el hilo principal.
#[derive(Debug, Clone)]
pub(crate) struct TextosRecuperar {
    titulo: String,
    pregunta: String,
    perdida: String,
    descartar: String,
    fallo: String,
}

impl TextosRecuperar {
    pub(crate) fn de(t: &Catalogo) -> TextosRecuperar {
        let con = |clave: &str| {
            let mut a = fluent_bundle::FluentArgs::new();
            a.set("nombre", NOMBRE);
            a.set("minutos", MINUTOS);
            a.set("chat", CHAT);
            t.t_args(clave, &a)
        };
        TextosRecuperar {
            titulo: t.t("clase-recuperar-titulo"),
            pregunta: con("clase-recuperar-pregunta"),
            perdida: con("clase-recuperar-sin-nada"),
            descartar: con("clase-recuperar-descartar"),
            fallo: con("clase-recuperar-fallo"),
        }
    }

    fn rellenar(&self, plantilla: &str, d: &Diario, p: &Plan) -> String {
        plantilla
            .replace(NOMBRE, &d.nombre)
            .replace(MINUTOS, &p.minutos().to_string())
            .replace(CHAT, &d.nombre_proyecto)
    }
}

/// **Al arrancar PixPin**: si quedaron grabaciones cortadas, pregunta por
/// cada una que hacer (en su hilo: el arranque no espera).
pub(crate) fn al_arrancar(
    ubicacion: Ubicacion,
    idioma: Idioma,
    aparato: String,
    textos: TextosRecuperar,
) {
    if !a_medias::carpeta_de_diarios(ubicacion.raiz()).is_dir() {
        return;
    }
    let lanzado = std::thread::Builder::new()
        .name("recuperar-grabaciones".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            for (dir, d) in pendientes(ubicacion.raiz()) {
                if super::a_salvo::en_marcha(&d.marca) {
                    continue;
                }
                let pedido = Pedido {
                    ubicacion: ubicacion.clone(),
                    idioma,
                    aparato: aparato.clone(),
                    proyectos: vec![(d.proyecto.clone(), d.nombre_proyecto.clone())],
                };
                preguntar(&pedido, &textos, &dir, &d);
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de recuperar grabaciones");
    }
}

/// Lo que dura el respaldo WAV de `ruta`, si lo hay.
fn respaldo_ms(ruta: &Path) -> Option<i64> {
    pixpin_audio::respaldo::leer(ruta)
        .ok()
        .map(|w| w.duracion_ms())
}

fn preguntar(pedido: &Pedido, textos: &TextosRecuperar, dir: &Path, d: &Diario) {
    let h = hallar(dir, d, respaldo_ms);
    let p = plan(d, &h);
    tracing::info!(marca = %d.marca, ?p, "grabacion cortada");
    if !p.algo() && p.video != Video::Perdido {
        // No llego a grabarse nada (se corto al abrir el microfono): no hay
        // nada que preguntar ni que perder.
        a_medias::descartar(dir, d);
        return;
    }
    let eleccion = if p.algo() {
        elegir_al_frente(&textos.titulo, &textos.rellenar(&textos.pregunta, d, &p))
    } else {
        // Solo queda un video que no se puede abrir: se dice y se pregunta
        // si borrarlo.
        if confirmar_al_frente(&textos.titulo, &textos.rellenar(&textos.perdida, d, &p)) {
            Eleccion::No
        } else {
            Eleccion::Luego
        }
    };
    match eleccion {
        Eleccion::Si => {
            if let Err(e) = recuperar_una(pedido, dir, d, &p) {
                tracing::warn!(?e, marca = %d.marca, "la grabacion no se pudo recuperar");
                pixpin_shell::dialogo::avisar_al_frente(
                    &textos.titulo,
                    &textos.rellenar(&textos.fallo, d, &p),
                );
            }
        }
        Eleccion::No => {
            if !p.algo()
                || confirmar_al_frente(&textos.titulo, &textos.rellenar(&textos.descartar, d, &p))
            {
                tracing::info!(marca = %d.marca, "grabacion cortada descartada");
                if let Some(id) = &d.movil {
                    crate::sincronizar::camara::descartada(pedido.ubicacion.raiz(), id);
                }
                a_medias::descartar(dir, d);
            }
        }
        Eleccion::Luego => {}
    }
}

/// **Rehace la clase** del diario `d` con el plan `p` y la mete en el chat.
fn recuperar_una(pedido: &Pedido, dir: &Path, d: &Diario, p: &Plan) -> anyhow::Result<()> {
    let raiz = pedido.ubicacion.raiz();
    // La nota.
    let grabacion = match p.nota {
        Nota::Lista => Some(pixpin_audio::Grabacion {
            ruta: d.audio.clone(),
            duracion_ms: p.duracion_ms,
            picos: Vec::new(),
        }),
        Nota::DelParte | Nota::DelRespaldo => match pixpin_audio::recuperar(&d.audio) {
            Ok(mut g) => {
                if g.duracion_ms <= 0 {
                    g.duracion_ms = p.duracion_ms;
                }
                Some(g)
            }
            Err(e) => {
                tracing::warn!(?e, "la nota de la grabacion cortada no se pudo rehacer");
                None
            }
        },
        Nota::Ninguna => None,
    };
    // El video.
    let video = match (p.video, &d.video) {
        (Video::Listo, Some(v)) => Some(v.clone()),
        (Video::DelParte, Some(v)) => {
            let como =
                recuperar_video(&con_sufijo(v, ".parte"), v, pixpin_motor_video::rehacer_mp4);
            tracing::info!(?como, "video de la grabacion cortada");
            como.map(|_| v.clone())
        }
        _ => None,
    };
    // Lo capturado, en su minuto.
    let mut estado = Estado::vacio(pedido, d.nombre.clone());
    estado.modo = Modo::de(&d.modo);
    estado.temporal = d.audio.clone();
    estado.capturas = p
        .capturas
        .iter()
        .filter_map(|&i| d.capturas.get(i))
        .map(|c| Captura {
            ms: c.ms,
            bytes: Vec::new(),
            de_camara: c.de_camara,
            fichero: Some(dir.join(&c.fichero)),
        })
        .collect();
    estado.clips = p
        .clips
        .iter()
        .filter_map(|&i| d.clips.get(i))
        .map(|c| ClipHecho {
            ms: c.ms,
            duracion_ms: c.duracion_ms.unwrap_or(0),
            ruta: c.ruta.clone(),
        })
        .collect();
    let hay_movil = d.movil.as_ref().is_some_and(|id| {
        !pixpin_proyecto::camara_del_movil::cola_en_disco(
            &crate::sincronizar::camara::carpeta_de_la_cola(raiz),
            id,
        )
        .is_empty()
    });
    anyhow::ensure!(
        grabacion.is_some()
            || video.is_some()
            || !estado.capturas.is_empty()
            || !estado.clips.is_empty()
            || hay_movil,
        "no quedo nada que se pueda abrir"
    );
    let voz = guardar(pedido, &mut estado, grabacion.as_ref(), video.as_deref())?;
    let duracion = grabacion.as_ref().map_or(p.duracion_ms, |g| g.duracion_ms);
    if let Some(id) = &d.movil {
        crate::sincronizar::camara::recuperada(
            raiz,
            id,
            &d.proyecto,
            voz.as_ref().map(|v| v.id.as_str()),
            estado.carpeta_id.as_deref(),
            &d.nombre,
            duracion,
        );
    }
    // Su reloj de pared en la nota: lo que llegue tarde (fotos sin
    // conexion, de colaboradores, la cola del movil) entra por su hora.
    if let Some(v) = &voz {
        crate::sincronizar::camara::reloj_recuperado(raiz, d, &v.id);
    }
    // Ya esta todo en el chat: la nota temporal (copiada) y el diario con
    // sus capturas sobran. Lo que no se pudo abrir (un video sin indice, un
    // clip sin cerrar) no se borra: se queda en la carpeta del chat.
    if voz.is_some() {
        let _ = std::fs::remove_file(&d.audio);
    }
    let _ = std::fs::remove_dir_all(dir);
    tracing::info!(
        marca = %d.marca,
        con_voz = voz.is_some(),
        capturas = estado.guardadas,
        clips = estado.clips_guardados,
        "grabacion cortada recuperada"
    );
    // Y a texto, de fondo, como al terminar.
    if let Some(voz) = &voz
        && let Some(Fase::Texto { id, en_marcha }) = empezar_el_texto(pedido, &d.proyecto, voz)
    {
        texto_de_fondo(pedido.clone(), d.proyecto.clone(), id, en_marcha);
    }
    Ok(())
}
