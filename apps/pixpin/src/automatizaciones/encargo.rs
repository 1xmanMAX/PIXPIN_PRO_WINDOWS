//! **Un encargo al asistente sin abrir el chat**: el pedido `asistente` de
//! Flow Launcher (`p ia <pedido>`, «yo le pido algo y lo hace
//! directamente») y los pasos «pedido al asistente» de las automatizaciones.
//!
//! Hace lo mismo que el chat «Asistente» cuando se le escribe, pero en un
//! hilo de fondo y sin ventana:
//!
//! 1. la pregunta (con sus fotos) se escribe en el chat «Asistente», como si
//!    se hubiera tecleado alli (`pedidos::chat_con_imagenes`, el camino del
//!    pedido `chat`), y nace la burbuja de la respuesta (`aparato = "IA"`);
//! 2. el turno corre con el motor de Ajustes (local, Claude o DeepSeek) y el
//!    mismo ejecutor del chat (`EjecutorDelChat`): las acciones de la app
//!    van por el protocolo de pedidos y lo que borra **pregunta antes** (en una
//!    tarjeta de su burbuja con «Sí»/«No», `Permiso`). Lo que dibuja, si no hay un lienzo abierto, va al
//!    lienzo del asistente en su chat, por el mismo camino que usa el chat
//!    cerrado (`ventana_chat::sin_ventana`);
//! 3. la respuesta se guarda en su nota y se ensena en el aviso flotante del
//!    asistente, con «Abrir» (el chat en esa respuesta).
//!
//! `proyecto` es el chat sobre el que se pide (`p ia … @obra`): se le dice al
//! modelo y lo que cree sin chat (una tarea, una nota) va alli.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use pixpin_ia::agente::{self, Ejecutor, Evento, Final};
use pixpin_ia::herramientas::Llamada;
use pixpin_ia::motor::{Motor, Seguir};
use pixpin_proyecto::{almacen, cuaderno};
use pixpin_store::Ubicacion;

use crate::asistente::acciones::{Confirmar, Mandar};
use crate::asistente::chat::{self as ia_chat, EnVivo};
use crate::asistente::ejecutor::EjecutorDelChat;
use crate::asistente::turno::{Adjunto, Aviso};

use super::Tipo;

/// Donde y quien.
#[derive(Clone)]
pub struct Entorno {
    pub ubicacion: Ubicacion,
    /// El codigo de este equipo (el del sello).
    pub aparato: String,
}

/// Lo que se le pide.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Encargo {
    pub texto: String,
    /// El chat sobre el que se pide (su id).
    pub proyecto: Option<String>,
    pub imagenes: Vec<PathBuf>,
    /// Quien lo pide, si no es el usuario a mano: el nombre de la
    /// automatizacion (va delante de la pregunta en el chat).
    pub origen: Option<String>,
    /// Desde una automatizacion de este disparador: lo que la haria saltar
    /// otra vez no se hace (`automatizaciones::bloqueado`).
    pub bloquear: Option<Tipo>,
}

/// Como acabo.
#[derive(Debug, Clone, PartialEq)]
pub struct Respuesta {
    /// El chat «Asistente».
    pub proyecto: String,
    /// El codigo de la nota de la respuesta (para «Abrir»).
    pub codigo: String,
    /// Lo que dijo, tal como quedo en la nota.
    pub texto: String,
    pub bien: bool,
}

/// Cada cuanto se escribe en el disco lo que va contestando (si el chat
/// esta abierto lo ve crecer).
const GUARDAR_CADA: Duration = Duration::from_secs(2);

/// **Hace el encargo** y no vuelve hasta acabar (llamarlo en un hilo).
/// `permiso` dice como se pregunta antes de lo que borra (en la burbuja, o
/// fijo en las pruebas) y `mandar` adonde van los pedidos: la PixPin que
/// corre, o los de las pruebas.
pub fn hacer(
    ent: &Entorno,
    motor: Arc<dyn Motor>,
    e: &Encargo,
    permiso: Permiso,
    mandar: Mandar,
) -> Result<Respuesta, String> {
    let raiz = ent.ubicacion.raiz().to_path_buf();
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let (asistente, carpeta) = super::chat_del_asistente(&raiz, &ent.aparato, ahora).map_err(|x| x.to_string())?;
    // El chat sobre el que se pide, por su nombre para el modelo.
    let contexto = match e.proyecto.as_deref().filter(|p| !p.is_empty()) {
        Some(id) => {
            let indice = almacen::Indice::leer(&raiz);
            let f = indice
                .proyectos
                .iter()
                .find(|f| f.id == id)
                .ok_or_else(|| format!("no hay ningún chat «{id}»"))?;
            Some((f.id.clone(), f.nombre.clone()))
        }
        None => None,
    };
    if let Some(r) = e.imagenes.iter().find(|r| !r.is_file()) {
        return Err(format!("no existe la imagen {}", r.display()));
    }

    // La conversacion de antes (lo que el modelo ya sabe), antes de escribir.
    let anteriores = match cuaderno::Cuaderno::leer_de(&carpeta) {
        Ok(c) => c.mensajes,
        Err(x) if x.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(x) => return Err(x.to_string()),
    };
    let conversacion = ia_chat::conversacion(&anteriores);

    // 1. La pregunta, en el chat «Asistente».
    let limpio = sin_fichas(&e.texto);
    let visible = match &e.origen {
        Some(o) if !limpio.is_empty() => format!("⚙️ {o}: {limpio}"),
        Some(o) => format!("⚙️ {o}"),
        None => limpio.clone(),
    };
    let visible = match &contexto {
        Some((_, nombre)) => format!("{visible} @{nombre}"),
        None => visible,
    };
    crate::pedidos::chat_con_imagenes(&raiz, &asistente, &ent.aparato, &visible, &e.imagenes, &[], ahora)
        .map_err(|x| x.detalle())?;
    let mut pregunta = if limpio.is_empty() {
        ia_chat::PREGUNTA_SOLO_FOTO.to_string()
    } else {
        limpio
    };
    if let Some((_, nombre)) = &contexto {
        pregunta = format!("{pregunta}\n\n(Lo pido sobre el proyecto «{nombre}»: lo que crees va a su chat.)");
    }
    let foto = e.imagenes.first().and_then(|r| crate::asistente::foto::preparar_fichero(r).ok());
    let historial = crate::asistente::turno::historial_para_el_modelo(
        &conversacion,
        &pregunta,
        foto.as_ref().map(|(b, m)| (b.clone(), *m)),
    );
    let adjuntos: Vec<Adjunto> = e
        .imagenes
        .iter()
        .map(|r| Adjunto {
            nombre: r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            ruta: r.clone(),
        })
        .collect();

    // 2. La burbuja de la respuesta.
    let numero = crate::ventana_chat::siguiente_numero_en(&raiz, &asistente).map_err(|x| x.to_string())?;
    let mut vivo0 = EnVivo::default();
    vivo0.estado = Some(crate::asistente::rapido::ESTADO_INICIAL.into());
    let nota = crate::ventana_chat::escribir_nota(
        &raiz,
        &asistente,
        ia_chat::APARATO,
        &vivo0.mostrar(),
        pixpin_shell::entorno::ahora_utc_ms().max(ahora + 1),
        numero,
        None,
    )
    .map_err(|x| x.to_string())?;
    crate::ventana_chat::refrescar();

    // 3. El turno, aqui mismo.
    let herramientas = if motor.catalogo_completo() {
        pixpin_ia::herramientas::catalogo()
    } else {
        crate::asistente::acciones::herramientas_para(&historial, &adjuntos, foto.is_some())
    };
    let (tx, rx) = channel();
    let parar = Arc::new(AtomicBool::new(false));
    // Lo que se ve en la burbuja, compartido con la pregunta de permiso (que
    // corre dentro del ejecutor, mientras el turno espera).
    let vivo = Arc::new(Mutex::new(vivo0));
    let confirmar = confirmar_en_la_nota(permiso, vivo.clone(), carpeta.clone(), asistente.clone(), nota.id.clone(), parar.clone());
    let mandar = envolver(mandar, contexto.as_ref().map(|(id, _)| id.clone()), e.bloquear);
    let mut ejecutor = SinVentana {
        dentro: EjecutorDelChat::nuevo(tx, asistente.clone(), adjuntos.clone(), parar).con_acciones(confirmar, mandar)
            .con_raiz(Some(ent.ubicacion.raiz().to_path_buf())),
        ubicacion: ent.ubicacion.clone(),
        proyecto: asistente.clone(),
        adjuntos,
        lienzo: None,
        semilla: 0,
    };
    let mut historial = historial;
    let mut guardado = Instant::now();
    let resultado: Result<Final, String> = {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        // El camino rapido, el mismo que el chat (`asistente::rapido`): una
        // orden simple se hace en milisegundos sin tocar el modelo.
        let opciones = agente::Opciones {
            rapido: Some(crate::asistente::rapido::ahora()),
            ..Default::default()
        };
        agente::turno_con(motor.as_ref(), &mut historial, &herramientas, &mut ejecutor, &opciones, &mut |ev: Evento| {
            let mut vivo = vivo.lock().unwrap_or_else(|x| x.into_inner());
            vivo.aplicar(&ev);
            if guardado.elapsed() >= GUARDAR_CADA {
                guardado = Instant::now();
                let t = vivo.mostrar();
                let _ = cuaderno::cambiar(&carpeta, &nota.id, |m| {
                    m.texto = t.clone();
                    true
                });
                crate::ventana_chat::refrescar();
            }
            Seguir::Si
        })
        .map_err(|x| {
            tracing::warn!(error = %x, "asistente sin ventana: el turno no pudo acabar");
            agente::para_el_usuario(&x)
        })
    };
    let mut vivo = vivo.lock().unwrap_or_else(|x| x.into_inner());
    vivo.acabar(&resultado);
    let texto = vivo.mostrar();
    let lienzo = ejecutor.lienzo.clone();
    cuaderno::cambiar(&carpeta, &nota.id, |m| {
        m.texto = texto.clone();
        if let Some(r) = &lienzo {
            m.resto
                .insert(ia_chat::LIENZO_DE_LA_RESPUESTA.into(), serde_json::Value::String(r.clone()));
        }
        true
    })
    .map_err(|x| x.to_string())?;
    // El texto entero de un OCR: al chat, como hace el chat.
    textos_leidos(&ent.ubicacion, &asistente, &rx);
    let resumen: String = texto.chars().take(120).collect();
    let _ = crate::ventana_chat::subir_en_la_lista(&raiz, &asistente, pixpin_shell::entorno::ahora_utc_ms(), &resumen);
    crate::ventana_chat::refrescar();
    Ok(Respuesta {
        proyecto: asistente,
        codigo: nota.codigo_unico(),
        texto,
        bien: resultado.is_ok() && vivo.fallos.is_empty(),
    })
}


/// **Como se pide permiso** para lo que borra o envia (13-oct-2026).
pub enum Permiso {
    /// En la burbuja de la respuesta, con «Sí»/«No» (como en el chat): la
    /// ventana del chat la ve en `asistente::permiso`. Lo de verdad.
    EnLaBurbuja,
    /// Lo que diga esta funcion, sin preguntar a nadie (las pruebas).
    Fijo(Confirmar),
}

/// **La pregunta en la nota de la respuesta**: la burbuja dice que espera
/// y, contestada, deja la linea «❓ … → ✅ Sí» en el texto, en el disco.
fn confirmar_en_la_nota(
    permiso: Permiso,
    vivo: Arc<Mutex<EnVivo>>,
    carpeta: PathBuf,
    proyecto: String,
    nota: String,
    parar: Arc<AtomicBool>,
) -> Confirmar {
    let guardar = move |vivo: &Arc<Mutex<EnVivo>>, cambio: &dyn Fn(&mut EnVivo)| {
        let t = {
            let mut v = vivo.lock().unwrap_or_else(|x| x.into_inner());
            cambio(&mut v);
            v.mostrar()
        };
        (t, ())
    };
    Box::new(move |pregunta: &str| {
        let escribir = |t: String| {
            let _ = cuaderno::cambiar(&carpeta, &nota, |m| {
                m.texto = t.clone();
                true
            });
            crate::ventana_chat::refrescar();
        };
        let (t, ()) = guardar(&vivo, &|v| v.preguntar(pregunta));
        let c = match &permiso {
            Permiso::EnLaBurbuja => {
                // Primero la nota (que se vea que espera), luego la pregunta.
                escribir(t);
                let rx = crate::asistente::permiso::registrar(&proyecto, &nota, pregunta);
                crate::ventana_chat::refrescar();
                let c = crate::asistente::permiso::esperar(&rx, ia_chat::CADUCA_LA_PREGUNTA, Some(&parar));
                crate::asistente::permiso::quitar(&proyecto, &nota);
                c
            }
            Permiso::Fijo(f) => {
                if f(pregunta) {
                    ia_chat::Contestacion::Si
                } else {
                    ia_chat::Contestacion::No
                }
            }
        };
        let (t, ()) = guardar(&vivo, &|v| {
            v.contestar(c);
        });
        escribir(t);
        c == ia_chat::Contestacion::Si
    })
}
fn textos_leidos(ubicacion: &Ubicacion, proyecto: &str, rx: &Receiver<Aviso>) {
    while let Ok(a) = rx.try_recv() {
        if let Aviso::TextoLeido { de, texto } = a
            && let Err(e) = crate::ventana_chat::sin_ventana::texto_leido(ubicacion, proyecto, &de, &texto)
        {
            tracing::warn!(?e, "asistente sin ventana: no se pudo dejar el texto leido");
        }
    }
}

/// El texto sin las fichas `[img NN]` (las imagenes van aparte).
pub fn sin_fichas(texto: &str) -> String {
    pixpin_lanzador::imagenes::sin_fichas(texto)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// **Los pedidos del modelo, con el chat del encargo y la guardia de
/// bucles**: lo que crea sin decir donde (una tarea, una nota) va al chat
/// pedido, y lo que haria saltar otra vez a la automatizacion que lo pide no
/// sale (se le contesta «no se entiende» y el modelo lo dice).
pub fn envolver(dentro: Mandar, proyecto: Option<String>, bloquear: Option<Tipo>) -> Mandar {
    Box::new(move |json: &str| {
        let Ok(mut v) = serde_json::from_str::<serde_json::Value>(json) else {
            return dentro(json);
        };
        if let Some(m) = super::bloqueado(bloquear, &v) {
            tracing::info!(%m, %json, "asistente: pedido bloqueado en una automatizacion");
            return pixpin_shell::mensajero::respuesta::NO_SE_ENTIENDE;
        }
        if let Some(p) = &proyecto {
            super::con_proyecto(&mut v, p);
        }
        dentro(&v.to_string())
    })
}

/// **El ejecutor del chat, sin chat**: lo del lienzo, si no hay un editor
/// abierto que lo atienda, se mete aqui mismo en el lienzo del asistente (el
/// chat cerrado no recogeria la cola); lo demas, igual que en el chat.
struct SinVentana {
    dentro: EjecutorDelChat,
    ubicacion: Ubicacion,
    proyecto: String,
    adjuntos: Vec<Adjunto>,
    /// El lienzo donde dibujo (su referencia), para «Abrir lienzo».
    lienzo: Option<String>,
    semilla: u64,
}

impl SinVentana {
    fn semilla(&mut self) -> u64 {
        self.semilla += 1;
        (pixpin_shell::entorno::ahora_utc_ms() as u64)
            .wrapping_mul(31)
            .wrapping_add(self.semilla)
    }

    fn al_lienzo(&mut self, elementos: Vec<serde_json::Value>) -> Result<String, String> {
        if elementos.is_empty() {
            return Err("no salio ninguna figura".into());
        }
        let documento = pixpin_ia::lienzo::documento(elementos);
        pixpin_motor2d::excalidraw::leer(&documento).map_err(|_| "el lienzo no pudo leer las figuras".to_string())?;
        let (resumen, referencia) =
            crate::ventana_chat::sin_ventana::al_lienzo(&self.ubicacion, &self.proyecto, &documento)?;
        self.lienzo = Some(referencia);
        crate::ventana_chat::refrescar();
        Ok(resumen)
    }
}

impl Ejecutor for SinVentana {
    fn ejecutar(&mut self, llamada: &Llamada) -> Result<String, String> {
        if crate::asistente::al_lienzo::hay_editor() {
            return self.dentro.ejecutar(llamada);
        }
        match llamada {
            Llamada::Dibujar(_) | Llamada::Grafica(_) => {
                let s = self.semilla();
                self.al_lienzo(pixpin_ia::lienzo::elementos(llamada, (0.0, 0.0), s))
            }
            Llamada::Calcar(p) => {
                let ruta = crate::asistente::acciones::imagen_para_calcar(&self.adjuntos, p.imagen.as_deref())?
                    .ruta
                    .clone();
                let s = self.semilla();
                let els = crate::asistente::acciones::calcar_fichero(&ruta, p, s)?;
                self.al_lienzo(els)
            }
            _ => self.dentro.ejecutar(llamada),
        }
    }
}

// ---------------------------------------------------------------------------
// Desde Flow Launcher (pedido `asistente`)

/// Lo que necesita el pedido de la app.
pub struct DeLaApp {
    pub entorno: Entorno,
    pub idioma: pixpin_store::Idioma,
    pub opciones: crate::ventana_chat::OpcionesLienzo,
}

/// **El pedido `asistente`**: en su hilo, con el aviso flotante desde ya
/// («pensando…») y la respuesta en el al acabar. Vuelve en el acto.
pub fn lanzar(app: DeLaApp, e: Encargo) {
    let aviso = crate::ventana_chat::sin_ventana::avisar(crate::ventana_chat::sin_ventana::Aviso {
        titulo: titulo_de(&e),
        texto: crate::asistente::rapido::ESTADO_INICIAL.into(),
        abrir: None,
        acabado: false,
        // Si el turno pide permiso, «Abrir» lleva a la burbuja de la pregunta.
        chat: Some(crate::ventana_chat::sin_ventana::Chat {
            idioma: app.idioma,
            ubicacion: app.entorno.ubicacion.clone(),
            opciones: app.opciones,
        }),
    });
    let lanzado = std::thread::Builder::new().name("asistente-encargo".into()).spawn(move || {
        let textos = pixpin_store::Catalogo::nuevo(app.idioma);
        // Sin modelo, una orden rapida se hace igual (como en el chat).
        let motor = crate::ventana_chat::sin_ventana::motor(&app.entorno.ubicacion, &textos);
        let r = crate::asistente::rapido::motor_o_sin(motor, &e.texto).and_then(|motor| {
            hacer(
                &app.entorno,
                motor,
                &e,
                Permiso::EnLaBurbuja,
                crate::asistente::acciones::mandar_a_pixpin(),
            )
        });
        let (texto, abrir) = match r {
            // Entera: el aviso la recorta a lo que cabe y se desplaza dentro.
            Ok(r) => (respuesta_para_el_aviso(&r.texto), Some((r.proyecto, r.codigo))),
            Err(x) => (format!("⚠️ {x}"), None),
        };
        aviso.poner(texto, abrir.map(|(p, c)| (app.idioma, app.entorno.ubicacion.clone(), app.opciones, p, c)));
    });
    if let Err(x) = lanzado {
        tracing::warn!(?x, "asistente: no se pudo lanzar el hilo del encargo");
    }
}

fn titulo_de(e: &Encargo) -> String {
    let t = sin_fichas(&e.texto);
    if t.is_empty() {
        "🤖 Asistente".into()
    } else {
        format!("🤖 {}", super::recortar(&t, 48))
    }
}

/// La respuesta para el aviso flotante, entera («Hecho.» si no dijo nada):
/// el aviso ensena lo que cabe y lo demas se desplaza (14-oct-2026; antes,
/// cortada a 280 letras y 31 tareas no se veian).
pub fn respuesta_para_el_aviso(texto: &str) -> String {
    let t = texto.trim();
    if t.is_empty() { "Hecho.".into() } else { t.to_string() }
}

/// La respuesta corta (el error de un paso de una automatizacion).
pub fn resumen_de(texto: &str) -> String {
    let t = texto.trim();
    if t.is_empty() {
        return "Hecho.".into();
    }
    super::recortar(t, 280)
}
