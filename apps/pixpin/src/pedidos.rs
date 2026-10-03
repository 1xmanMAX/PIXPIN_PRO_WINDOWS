//! **Los pedidos de otros programas** (`docs/protocolo-pedidos.md`): abrir
//! un proyecto, escribir en un chat, crear un lienzo, grabar, anadir una
//! tarea... desde el plugin de Flow Launcher o un script del usuario.
//!
//! La ventana de mensajes (`pixpin_shell::ventana`) ya contesto al que
//! mandaba (1, 2 o 3) y dejo el JSON en la cola; aqui se interpreta y se
//! hace, en el bucle principal. Lo que salga mal ya no se le puede contestar
//! al otro programa: se dice con el globo de PixPin y queda en el registro.
//!
//! **Todo pasa por las mismas funciones que usa el chat** (`escribir_nota`,
//! `escribir_miniapp`, `escribir_lienzo`, `ir_a`, `notas_md::abrir`...): un
//! mensaje que llega por aqui tiene que ser identico a uno escrito a mano,
//! con su sello (hora, numero, aparato), para que el movil lo vea como
//! nacido en el PC. Y escribir es cosa de la aplicacion, nunca del otro
//! programa: los cuadernos los protege un cerrojo que solo existe dentro de
//! este proceso (`cuaderno::cerrojo`).
//!
//! Las tareas son las de la mini-app `tareas`, copiada de Android
//! (`mini/Tareas.kt`): se edita su documento con `pixpin_proyecto::mini` y se
//! reescribe su mensaje con `cuaderno::reemplazar`, como hace el panel del
//! chat al marcar una casilla.

use std::path::{Path, PathBuf};

use pixpin_proyecto::{almacen, cuaderno, mini};
use pixpin_store::{Catalogo, Ubicacion};
use serde::Deserialize;

/// Un pedido ya leido. Lo que no se conoce del JSON se ignora (la version
/// incluida, que ya miro la ventana): un campo nuevo no rompe a nadie.
///
/// `proyecto` es el `id` de la carpeta del proyecto; ausente o `null` es
/// «Mensajes guardados». `codigo` es el `id` del mensaje en su cuaderno
/// (tambien vale su codigo unico, que es lo que usa el universo).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "accion", rename_all = "snake_case")]
pub enum Pedido {
    Abrir {
        que: Que,
    },
    Chat {
        texto: String,
        proyecto: Option<String>,
    },
    NotaNueva {
        texto: Option<String>,
        proyecto: Option<String>,
    },
    LienzoNuevo {
        nombre: Option<String>,
        proyecto: Option<String>,
    },
    Grabar {
        nombre: String,
        proyecto: Option<String>,
    },
    ListaNueva {
        titulo: String,
        proyecto: Option<String>,
    },
    AnadirTarea {
        texto: String,
        proyecto: Option<String>,
        codigo: Option<String>,
    },
    MarcarTarea {
        proyecto: Option<String>,
        codigo: String,
        indice: usize,
        hecha: bool,
    },
    /// Con llaves y sin campos, no como variante unitaria: asi se aceptan
    /// los campos de mas (`pixpin`) igual que en las demas.
    VentanaPrincipal {},
    /// Sacar a la pantalla un fichero (`ruta`) o el de un mensaje
    /// (`proyecto?`, `codigo`), como «Sacar a la pantalla» del chat.
    Pinear {
        ruta: Option<PathBuf>,
        proyecto: Option<String>,
        codigo: Option<String>,
    },
    /// Pintar los iconos de archivo de esas extensiones en
    /// `<raiz>/cache/iconos-de-extension/`, los que falten.
    Iconos {
        extensiones: Vec<String>,
    },
    /// La caja flotante a la que soltar ficheros para ese chat.
    Soltar {
        proyecto: Option<String>,
    },
}

/// Que se abre con `abrir`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "tipo", rename_all = "lowercase")]
pub enum Que {
    Proyecto {
        proyecto: Option<String>,
    },
    Mensaje {
        proyecto: Option<String>,
        codigo: String,
    },
    Hoja {
        proyecto: Option<String>,
        referencia: String,
    },
    Nota {
        proyecto: Option<String>,
        codigo: String,
    },
    Fichero {
        ruta: PathBuf,
    },
}

/// Lee un pedido. Un fallo aqui es un pedido que paso el vistazo de la
/// ventana (version y accion buenas) pero le falta un campo o lo trae de
/// otro tipo.
pub fn leer(json: &str) -> Result<Pedido, Fallo> {
    serde_json::from_str(json).map_err(|e| Fallo::Roto(e.to_string()))
}

/// Por que no se pudo hacer un pedido. Cada uno tiene su aviso: al usuario
/// le sirve saber QUE falta, no que «algo» fallo.
#[derive(Debug)]
pub enum Fallo {
    /// Al pedido le falta un campo, o lo trae mal.
    Roto(String),
    /// No hay ningun proyecto con ese id.
    SinProyecto(String),
    /// En ese proyecto no hay un mensaje con ese codigo.
    SinMensaje(String),
    /// El mensaje existe pero no es una lista de tareas.
    NoEsLista,
    /// La lista no tiene una tarea con ese numero.
    SinTarea(usize),
    /// Una tarea sin texto no dice que hay que hacer.
    TareaVacia,
    /// El fichero que se pidio abrir no esta.
    SinFichero(PathBuf),
    /// El mensaje existe pero no tiene un fichero en este equipo (una nota
    /// de texto, o un adjunto que aun no llego del movil).
    MensajeSinFichero(String),
    /// No se pudo leer o escribir el disco.
    Disco(std::io::Error),
}

impl From<std::io::Error> for Fallo {
    fn from(e: std::io::Error) -> Self {
        Fallo::Disco(e)
    }
}

impl Fallo {
    /// Lo que va al registro: el globo dice que paso, el registro con que
    /// (que id, que ruta, que dijo el disco), que es lo que hace falta para
    /// arreglar el script que lo mando.
    fn detalle(&self) -> String {
        match self {
            Fallo::Roto(e) => format!("pedido mal formado: {e}"),
            Fallo::SinProyecto(id) => format!("no hay proyecto «{id}»"),
            Fallo::SinMensaje(codigo) => format!("no hay mensaje «{codigo}»"),
            Fallo::NoEsLista => "el mensaje no es una lista de tareas".into(),
            Fallo::SinTarea(n) => format!("no hay tarea {n}"),
            Fallo::TareaVacia => "tarea sin texto".into(),
            Fallo::SinFichero(r) => format!("no existe {}", r.display()),
            Fallo::MensajeSinFichero(codigo) => format!("el mensaje «{codigo}» no tiene fichero"),
            Fallo::Disco(e) => format!("disco: {e}"),
        }
    }

    /// El aviso para el globo, ya traducido.
    fn aviso(&self, t: &Catalogo) -> String {
        let mut args = fluent_bundle::FluentArgs::new();
        let clave = match self {
            Fallo::Roto(_) => "pedido-roto",
            Fallo::SinProyecto(_) => "pedido-sin-proyecto",
            Fallo::SinMensaje(_) => "pedido-sin-mensaje",
            Fallo::NoEsLista => "pedido-no-es-lista",
            Fallo::SinTarea(n) => {
                args.set("indice", *n as i64);
                "pedido-sin-tarea"
            }
            Fallo::TareaVacia => "pedido-tarea-vacia",
            Fallo::SinFichero(_) => "pedido-sin-fichero",
            // El mismo aviso que el chat al pinear un mensaje sin fichero.
            Fallo::MensajeSinFichero(_) => "chat-sin-archivo",
            Fallo::Disco(_) => "pedido-fallo-disco",
        };
        t.t_args(clave, &args)
    }
}

/// Lo que el pedido necesita de la aplicacion. Va en una estructura porque
/// son las variables del `main` y no tiene sentido pasarlas una a una.
pub struct Contexto<'a> {
    pub idioma: pixpin_store::Idioma,
    pub ubicacion: &'a Ubicacion,
    pub opciones: crate::ventana_chat::OpcionesLienzo,
    /// El codigo de este equipo (`K7Q2`), el del sello.
    pub aparato: &'a str,
    pub textos: &'a Catalogo,
}

/// Lo que queda por hacer en el `main` despues de un pedido.
#[derive(Debug, Default)]
pub struct Hecho {
    /// Lo que hay que decir en el globo, si hay algo que decir. Lo que abre
    /// una ventana no dice nada: la ventana ya es la respuesta.
    pub aviso: Option<String>,
    /// Ficheros que abrir como «Abrir con PixPin»: eso lo hace el `main`,
    /// que es quien tiene los pines y el lector.
    pub ficheros: Vec<PathBuf>,
}

/// Interpreta y hace un pedido. Nunca falla hacia fuera: lo que sale mal se
/// apunta en el registro y se convierte en el aviso del globo.
pub fn atender(json: &str, cx: &Contexto) -> Hecho {
    // Lo que abre una ventana la trae delante aunque nazca minimizada o
    // PixPin no tenga el foco (`colocacion::al_frente_lo_nuevo`). El
    // microfono de «grabar» no: es una ventanita de herramientas, siempre
    // encima, que se enfoca ella sola al nacer (y esto la ignoraria y podria
    // llevarse por delante otra que se abriera en esos segundos).
    let abre = leer(json).is_ok_and(|p| {
        matches!(
            p,
            Pedido::Abrir { .. }
                | Pedido::NotaNueva { .. }
                | Pedido::LienzoNuevo { .. }
                | Pedido::Pinear { .. }
        )
    });
    if abre {
        pixpin_shell::colocacion::al_frente_lo_nuevo(
            pixpin_shell::colocacion::ventanas_del_proceso(),
            std::time::Duration::from_secs(4),
        );
    }
    let hecho = leer(json).and_then(|p| hacer(p, cx));
    match hecho {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(detalle = %e.detalle(), %json, "pedido que no se pudo hacer");
            Hecho {
                aviso: Some(e.aviso(cx.textos)),
                ficheros: Vec::new(),
            }
        }
    }
}

fn hacer(p: Pedido, cx: &Contexto) -> Result<Hecho, Fallo> {
    let raiz = cx.ubicacion.raiz();
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let t = cx.textos;
    let aviso = |clave: &str, pares: &[(&str, String)]| {
        let mut args = fluent_bundle::FluentArgs::new();
        for (k, v) in pares {
            args.set(*k, v.clone());
        }
        Hecho {
            aviso: Some(t.t_args(clave, &args)),
            ficheros: Vec::new(),
        }
    };
    let ir_al_chat = |proyecto: String, codigo: Option<String>| {
        crate::ventana_chat::ir_a(
            cx.idioma,
            cx.ubicacion.clone(),
            cx.opciones,
            proyecto,
            codigo,
        )
    };
    match p {
        Pedido::Abrir { que } => match que {
            Que::Proyecto { proyecto } => {
                let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
                ir_al_chat(f.id, None);
            }
            Que::Mensaje { proyecto, codigo } => {
                let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
                let m = mensaje_de(raiz, &f.id, &codigo)?;
                // El chat busca por el codigo unico, no por el id.
                ir_al_chat(f.id, Some(m.codigo_unico()));
            }
            Que::Hoja {
                proyecto,
                referencia,
            } => {
                let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
                abrir_hoja(cx, f.id, referencia);
            }
            Que::Nota { proyecto, codigo } => {
                let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
                // Una nota del movil puede ser solo una hoja de
                // `proyecto.json`, sin mensaje en el cuaderno: entonces se
                // le pasa el codigo tal cual, que el editor sabe buscarla.
                let codigo = match mensaje_de(raiz, &f.id, &codigo) {
                    Ok(m) => m.codigo_unico(),
                    Err(Fallo::SinMensaje(_)) => codigo,
                    Err(e) => return Err(e),
                };
                crate::notas_md::abrir(
                    cx.idioma,
                    cx.ubicacion.clone(),
                    crate::notas_md::Destino::Mensaje {
                        proyecto: f.id,
                        codigo,
                    },
                );
            }
            Que::Fichero { ruta } => {
                if !ruta.is_file() {
                    return Err(Fallo::SinFichero(ruta));
                }
                return Ok(Hecho {
                    aviso: None,
                    ficheros: vec![ruta],
                });
            }
        },
        Pedido::Chat { texto, proyecto } => {
            let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
            escribir_en_el_chat(raiz, &f.id, cx.aparato, &texto, ahora)?;
            crate::ventana_chat::refrescar();
            return Ok(aviso("pedido-escrito", &[("proyecto", f.nombre)]));
        }
        Pedido::NotaNueva { texto, proyecto } => {
            let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
            // El editor no sabe nacer con texto: con texto, la nota se
            // escribe antes en el chat, como si se hubiera guardado, y se
            // abre la que ya existe. Sin texto, la nota nueva de siempre,
            // que no deja nada en el chat si se cierra sin escribir.
            let destino = match texto.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
                Some(texto) => {
                    let m = escribir_en_el_chat(raiz, &f.id, cx.aparato, texto, ahora)?;
                    crate::ventana_chat::refrescar();
                    crate::notas_md::Destino::Mensaje {
                        proyecto: f.id,
                        codigo: m.codigo_unico(),
                    }
                }
                None => crate::notas_md::Destino::Nueva { proyecto: f.id },
            };
            crate::notas_md::abrir(cx.idioma, cx.ubicacion.clone(), destino);
        }
        Pedido::LienzoNuevo { nombre, proyecto } => {
            let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
            let numero = crate::ventana_chat::siguiente_numero_en(raiz, &f.id)?;
            let m = crate::ventana_chat::escribir_lienzo(
                raiz,
                &f.id,
                cx.aparato,
                numero,
                nombre.as_deref(),
            )?;
            crate::ventana_chat::subir_en_la_lista(raiz, &f.id, m.cuando, &m.nombre)?;
            crate::ventana_chat::refrescar();
            if let Some(referencia) = m.referencia.clone() {
                abrir_hoja(cx, f.id, referencia);
            }
        }
        // Sin chat: el microfono flotante es la respuesta (y el que dice que
        // se esta grabando).
        Pedido::Grabar { nombre, proyecto } => {
            let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
            crate::ventana_chat::grabar_flotante(
                t,
                raiz.to_path_buf(),
                f.id,
                cx.aparato.to_string(),
                nombre,
            );
        }
        Pedido::Pinear {
            ruta,
            proyecto,
            codigo,
        } => {
            let ruta = fichero_a_pinear(raiz, ruta, proyecto.as_deref(), codigo, cx.aparato, ahora)?;
            // Al `main`, como el chat (`Accion::Pinear` se lo manda por
            // `enviar_ficheros`): es quien tiene los pines.
            return Ok(Hecho {
                aviso: None,
                ficheros: vec![ruta],
            });
        }
        Pedido::Iconos { extensiones } => {
            crate::ventana_chat::pintar_iconos_de_extension(raiz.to_path_buf(), extensiones);
        }
        // Como el microfono: la caja se trae delante ella sola.
        Pedido::Soltar { proyecto } => {
            let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
            crate::ventana_chat::caja_de_soltar(
                t,
                raiz.to_path_buf(),
                f.id,
                f.nombre,
                cx.aparato.to_string(),
            );
        }
        Pedido::ListaNueva { titulo, proyecto } => {
            let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
            let m = nueva_lista(raiz, &f.id, cx.aparato, &titulo)?;
            crate::ventana_chat::refrescar();
            return Ok(aviso(
                "pedido-lista-hecha",
                &[("lista", m.nombre), ("proyecto", f.nombre)],
            ));
        }
        Pedido::AnadirTarea {
            texto,
            proyecto,
            codigo,
        } => {
            let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
            let lista = anadir_tarea(
                raiz,
                &f.id,
                cx.aparato,
                codigo.as_deref(),
                &texto,
                &t.t("pedido-tareas"),
            )?;
            crate::ventana_chat::refrescar();
            return Ok(aviso(
                "pedido-tarea-anadida",
                &[
                    ("tarea", mini::saneado(&texto)),
                    ("lista", nombre_de_lista(&lista)),
                ],
            ));
        }
        Pedido::MarcarTarea {
            proyecto,
            codigo,
            indice,
            hecha,
        } => {
            let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
            let tarea = marcar_tarea(raiz, &f.id, &codigo, indice, hecha)?;
            crate::ventana_chat::refrescar();
            let clave = if hecha {
                "pedido-tarea-hecha"
            } else {
                "pedido-tarea-pendiente"
            };
            return Ok(aviso(clave, &[("tarea", tarea.texto)]));
        }
        Pedido::VentanaPrincipal {} => {
            crate::ventana_chat::lanzar(cx.idioma, cx.ubicacion.clone(), cx.opciones);
        }
    }
    Ok(Hecho::default())
}

/// Abre una hoja en el lienzo, en su propio hilo como la abre un grupo de
/// ventanas (`grupos_ventanas::lanzar`): el lienzo pasa minutos abierto y el
/// bucle principal tiene que seguir atendiendo atajos y gestos. Al cerrarlo,
/// la burbuja del chat tiene que ensenar lo dibujado.
fn abrir_hoja(cx: &Contexto, proyecto: String, referencia: String) {
    let raiz = cx.ubicacion.raiz().to_path_buf();
    let opciones = cx.opciones;
    let lanzado = std::thread::Builder::new()
        .name("lienzo-del-pedido".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            crate::universo::abrir::abrir_hoja(&raiz, &proyecto, &referencia, opciones);
            crate::ventana_chat::refrescar();
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del lienzo del pedido");
    }
}

/// La ficha del proyecto pedido. Sin proyecto (o vacio), «Mensajes
/// guardados», que se crea si todavia no existe: es adonde va lo suelto.
fn ficha_de(
    raiz: &Path,
    proyecto: Option<&str>,
    aparato: &str,
    ahora: i64,
) -> Result<almacen::Ficha, Fallo> {
    match proyecto.map(str::trim).filter(|p| !p.is_empty()) {
        None => Ok(almacen::asegurar_guardados(raiz, ahora, aparato)?),
        Some(id) => almacen::Indice::leer_si_esta(raiz)?
            .proyectos
            .into_iter()
            .find(|f| f.id == id)
            .ok_or_else(|| Fallo::SinProyecto(id.to_string())),
    }
}

/// Un mensaje del proyecto por su `id` o por su codigo unico.
fn mensaje_de(raiz: &Path, proyecto: &str, codigo: &str) -> Result<cuaderno::Mensaje, Fallo> {
    let c = match cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, proyecto)) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => return Err(e.into()),
    };
    c.mensajes
        .into_iter()
        .find(|m| m.id == codigo || m.codigo_unico() == codigo)
        .ok_or_else(|| Fallo::SinMensaje(codigo.to_string()))
}

/// El fichero que saca a la pantalla un «pinear»: la `ruta` tal cual, o el
/// del mensaje `codigo` resuelto como lo resuelve el chat
/// (`ventana_chat::fichero_del_mensaje`, con el dibujo de un lienzo que solo
/// trae su `referencia`).
fn fichero_a_pinear(
    raiz: &Path,
    ruta: Option<PathBuf>,
    proyecto: Option<&str>,
    codigo: Option<String>,
    aparato: &str,
    ahora: i64,
) -> Result<PathBuf, Fallo> {
    if let Some(ruta) = ruta.filter(|r| !r.as_os_str().is_empty()) {
        return if ruta.is_file() {
            Ok(ruta)
        } else {
            Err(Fallo::SinFichero(ruta))
        };
    }
    let Some(codigo) = codigo.filter(|c| !c.trim().is_empty()) else {
        return Err(Fallo::Roto("pinear sin ruta ni codigo".into()));
    };
    let f = ficha_de(raiz, proyecto, aparato, ahora)?;
    let m = mensaje_de(raiz, &f.id, &codigo)?;
    crate::ventana_chat::fichero_del_mensaje(raiz, &f.id, &m).ok_or(Fallo::MensajeSinFichero(codigo))
}

/// Una nota de texto nueva en el chat, sellada como las de la caja, y el
/// proyecto subido en la lista.
fn escribir_en_el_chat(
    raiz: &Path,
    proyecto: &str,
    aparato: &str,
    texto: &str,
    ahora: i64,
) -> Result<cuaderno::Mensaje, Fallo> {
    // Como la caja: lo que se envia va sin los blancos de los bordes, y un
    // envio vacio no es un mensaje.
    let texto = texto.trim();
    if texto.is_empty() {
        return Err(Fallo::Roto("texto vacio".into()));
    }
    let numero = crate::ventana_chat::siguiente_numero_en(raiz, proyecto)?;
    let m =
        crate::ventana_chat::escribir_nota(raiz, proyecto, aparato, texto, ahora, numero, None)?;
    crate::ventana_chat::subir_en_la_lista(raiz, proyecto, ahora, texto)?;
    Ok(m)
}

/// Si un mensaje es una lista de tareas.
fn es_lista(m: &cuaderno::Mensaje) -> bool {
    m.clase == Some(cuaderno::Clase::MiniApp) && m.miniapp.as_deref() == Some(mini::TAREAS)
}

/// Como se llama una lista: su titulo, o el nombre del mensaje si el
/// documento no tiene.
fn nombre_de_lista(m: &cuaderno::Mensaje) -> String {
    match mini::titulo(&m.texto) {
        t if t.is_empty() => m.nombre.clone(),
        t => t,
    }
}

/// Una lista de tareas nueva y vacia, con ese titulo, como la del clip.
fn nueva_lista(
    raiz: &Path,
    proyecto: &str,
    aparato: &str,
    titulo: &str,
) -> Result<cuaderno::Mensaje, Fallo> {
    let titulo = titulo.trim();
    let numero = crate::ventana_chat::siguiente_numero_en(raiz, proyecto)?;
    // La moneda solo la usan los gastos; una lista de tareas no la mira.
    let m = crate::ventana_chat::escribir_miniapp(
        raiz,
        proyecto,
        aparato,
        mini::TAREAS,
        titulo,
        &mini::gastos::Moneda::euro(),
        numero,
    )?;
    crate::ventana_chat::subir_en_la_lista(raiz, proyecto, m.cuando, titulo)?;
    Ok(m)
}

/// La lista de tareas en la que se apunta: la pedida por su codigo, o si no
/// se pidio ninguna la ultima del chat (la mas reciente), o una nueva con
/// `titulo_nueva` si el chat no tiene ninguna.
fn lista_para_anadir(
    raiz: &Path,
    proyecto: &str,
    aparato: &str,
    codigo: Option<&str>,
    titulo_nueva: &str,
) -> Result<cuaderno::Mensaje, Fallo> {
    if let Some(codigo) = codigo.map(str::trim).filter(|c| !c.is_empty()) {
        let m = mensaje_de(raiz, proyecto, codigo)?;
        return if es_lista(&m) {
            Ok(m)
        } else {
            Err(Fallo::NoEsLista)
        };
    }
    let ultima = match cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, proyecto)) {
        // Lo del buzon caduca solo: no es donde se apunta una tarea.
        Ok(c) => c
            .mensajes
            .into_iter()
            .filter(|m| es_lista(m) && !m.en_buzon)
            .max_by_key(|m| m.cuando),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    match ultima {
        Some(m) => Ok(m),
        None => nueva_lista(raiz, proyecto, aparato, titulo_nueva),
    }
}

/// Anade `- [ ] texto` al final de una lista y la reescribe en su sitio.
/// Devuelve la lista como quedo.
fn anadir_tarea(
    raiz: &Path,
    proyecto: &str,
    aparato: &str,
    codigo: Option<&str>,
    texto: &str,
    titulo_nueva: &str,
) -> Result<cuaderno::Mensaje, Fallo> {
    // Antes de buscar la lista: una tarea vacia no puede dejar creada una
    // lista «Tareas» que nadie pidio.
    if mini::saneado(texto).is_empty() {
        return Err(Fallo::TareaVacia);
    }
    let mut m = lista_para_anadir(raiz, proyecto, aparato, codigo, titulo_nueva)?;
    // Con la fecha de hoy, como al anadirla en el chat (`mini::anadir_el`).
    let hoy = mini::Fecha::de_ms_locales(pixpin_shell::entorno::ahora_utc_ms() + pixpin_shell::entorno::desfase_local_ms());
    m.texto = mini::anadir_el(&m.texto, texto, hoy);
    reescribir(raiz, proyecto, &m)?;
    Ok(m)
}

/// Pone la tarea numero `indice` (desde 0, en el orden del documento) como
/// hecha o pendiente. **Pone, no alterna**: un pedido repetido —un doble
/// Intro en el lanzador— no puede deshacer lo que hizo el primero. Devuelve
/// la tarea como quedo.
fn marcar_tarea(
    raiz: &Path,
    proyecto: &str,
    codigo: &str,
    indice: usize,
    hecha: bool,
) -> Result<mini::Tarea, Fallo> {
    let mut m = mensaje_de(raiz, proyecto, codigo)?;
    if !es_lista(&m) {
        return Err(Fallo::NoEsLista);
    }
    let tareas = mini::leer_tareas(&m.texto);
    let Some(tarea) = tareas.get(indice) else {
        return Err(Fallo::SinTarea(indice));
    };
    if tarea.hecha != hecha {
        // `alternar` es lo que hace la casilla del panel (`Tareas.kt`): el
        // documento sale reescrito igual que si se hubiera pulsado.
        m.texto = mini::alternar(&m.texto, indice);
        reescribir(raiz, proyecto, &m)?;
    }
    Ok(mini::Tarea {
        texto: tarea.texto.clone(),
        hecha,
    })
}

/// Escribe el documento cambiado en su mensaje, como `guardar_mini` del
/// chat: `cuaderno::reemplazar` toma el cerrojo de los cuadernos y copia tal
/// cual lo que no entiende. Si el mensaje desaparecio entre leerlo y
/// escribirlo (lo borro el chat, o una sincronizacion), se dice.
fn reescribir(raiz: &Path, proyecto: &str, m: &cuaderno::Mensaje) -> Result<(), Fallo> {
    if cuaderno::reemplazar(&almacen::carpeta(raiz, proyecto), m)? {
        Ok(())
    } else {
        Err(Fallo::SinMensaje(m.id.clone()))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un ejemplo de cada accion, con lo minimo que pide la tabla del
    /// protocolo.
    const EJEMPLOS: [(&str, &str); 12] = [
        (
            "abrir",
            r#"{"pixpin":1,"accion":"abrir","que":{"tipo":"proyecto"}}"#,
        ),
        ("chat", r#"{"pixpin":1,"accion":"chat","texto":"hola"}"#),
        ("nota_nueva", r#"{"pixpin":1,"accion":"nota_nueva"}"#),
        ("lienzo_nuevo", r#"{"pixpin":1,"accion":"lienzo_nuevo"}"#),
        (
            "grabar",
            r#"{"pixpin":1,"accion":"grabar","nombre":"reunion"}"#,
        ),
        (
            "lista_nueva",
            r#"{"pixpin":1,"accion":"lista_nueva","titulo":"Compra"}"#,
        ),
        (
            "anadir_tarea",
            r#"{"pixpin":1,"accion":"anadir_tarea","texto":"pan"}"#,
        ),
        (
            "marcar_tarea",
            r#"{"pixpin":1,"accion":"marcar_tarea","codigo":"1","indice":0,"hecha":true}"#,
        ),
        (
            "ventana_principal",
            r#"{"pixpin":1,"accion":"ventana_principal"}"#,
        ),
        (
            "pinear",
            r#"{"pixpin":1,"accion":"pinear","codigo":"1"}"#,
        ),
        (
            "iconos",
            r#"{"pixpin":1,"accion":"iconos","extensiones":["pdf","docx"]}"#,
        ),
        ("soltar", r#"{"pixpin":1,"accion":"soltar"}"#),
    ];

    #[test]
    fn la_aplicacion_entiende_todas_las_acciones_que_acepta_la_ventana() {
        // Si alguien anade una accion a `mensajero::ACCIONES` y no aqui, la
        // ventana contestaria «aceptado» a algo que luego no se hace.
        for accion in pixpin_shell::mensajero::ACCIONES {
            let (_, json) = EJEMPLOS
                .iter()
                .find(|(a, _)| *a == accion)
                .unwrap_or_else(|| panic!("falta un ejemplo de «{accion}»"));
            assert_eq!(
                pixpin_shell::mensajero::validar_pedido(json),
                pixpin_shell::mensajero::respuesta::ACEPTADO
            );
            assert!(leer(json).is_ok(), "{accion}: {:?}", leer(json));
        }
    }

    #[test]
    fn sin_proyecto_o_con_null_es_mensajes_guardados() {
        let a = leer(r#"{"pixpin":1,"accion":"chat","texto":"x","proyecto":null}"#).unwrap();
        let b = leer(r#"{"pixpin":1,"accion":"chat","texto":"x"}"#).unwrap();
        assert_eq!(a, b);
        assert_eq!(
            a,
            Pedido::Chat {
                texto: "x".into(),
                proyecto: None
            }
        );
    }

    #[test]
    fn abrir_lee_cada_tipo_y_los_campos_de_mas_se_ignoran() {
        let p = leer(
            r#"{"pixpin":1,"accion":"abrir","extra":true,
                "que":{"tipo":"hoja","proyecto":"pr-1","referencia":"dib-2","otro":1}}"#,
        )
        .unwrap();
        assert_eq!(
            p,
            Pedido::Abrir {
                que: Que::Hoja {
                    proyecto: Some("pr-1".into()),
                    referencia: "dib-2".into()
                }
            }
        );
        let f = leer(
            r#"{"pixpin":1,"accion":"abrir","que":{"tipo":"fichero","ruta":"C:\\a b\\ñ.pdf"}}"#,
        )
        .unwrap();
        assert_eq!(
            f,
            Pedido::Abrir {
                que: Que::Fichero {
                    ruta: PathBuf::from(r"C:\a b\ñ.pdf")
                }
            }
        );
    }

    #[test]
    fn a_un_pedido_que_le_falta_algo_no_se_le_inventa() {
        // Casos negativos: cada uno pasa el vistazo de la ventana (version y
        // accion buenas) pero no se puede hacer.
        for json in [
            r#"{"pixpin":1,"accion":"chat"}"#,
            r#"{"pixpin":1,"accion":"grabar"}"#,
            r#"{"pixpin":1,"accion":"marcar_tarea","codigo":"1","indice":-1,"hecha":true}"#,
            r#"{"pixpin":1,"accion":"marcar_tarea","codigo":"1","indice":0}"#,
            r#"{"pixpin":1,"accion":"abrir","que":{"tipo":"universo"}}"#,
            r#"{"pixpin":1,"accion":"abrir","que":{"tipo":"mensaje","proyecto":"p"}}"#,
        ] {
            assert!(matches!(leer(json), Err(Fallo::Roto(_))), "{json}");
        }
    }

    /// Un almacen de prueba con un proyecto «Casa».
    fn almacen_de_prueba(nombre: &str) -> (PathBuf, almacen::Ficha) {
        let raiz =
            std::env::temp_dir().join(format!("pixpin-pedidos-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let f = almacen::Ficha::nueva("Casa", 1, "PC01");
        let indice = almacen::Indice {
            proyectos: vec![f.clone()],
            ..Default::default()
        };
        indice.guardar(&raiz).unwrap();
        (raiz, f)
    }

    fn cuaderno_de(raiz: &Path, proyecto: &str) -> Vec<cuaderno::Mensaje> {
        cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, proyecto))
            .unwrap()
            .mensajes
    }

    #[test]
    fn un_texto_al_chat_lleva_su_sello_y_sube_el_proyecto() {
        let (raiz, f) = almacen_de_prueba("chat");
        let m = escribir_en_el_chat(&raiz, &f.id, "PC01", "  comprar pan  ", 1000).unwrap();
        let m2 = escribir_en_el_chat(&raiz, &f.id, "PC01", "y leche", 2000).unwrap();
        assert_eq!(m.texto, "comprar pan");
        assert_eq!((m.numero, m2.numero), (1, 2));
        assert_eq!(m.aparato.as_deref(), Some("PC01"));
        assert_eq!(m.proyecto.as_deref(), Some(f.id.as_str()));
        assert!(m.uid.is_some());
        assert_eq!(cuaderno_de(&raiz, &f.id).len(), 2);
        let i = almacen::Indice::leer(&raiz);
        let subida = i.buscar(&f.id).unwrap();
        assert_eq!((subida.tocado, subida.resumen.as_str()), (2000, "y leche"));
        // Caso negativo: un envio en blanco no es un mensaje.
        assert!(escribir_en_el_chat(&raiz, &f.id, "PC01", "   ", 3000).is_err());
        assert_eq!(cuaderno_de(&raiz, &f.id).len(), 2);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn sin_proyecto_va_a_mensajes_guardados_y_uno_que_no_existe_se_dice() {
        let (raiz, f) = almacen_de_prueba("guardados");
        let g = ficha_de(&raiz, None, "PC01", 5).unwrap();
        assert!(g.es_guardados());
        // Es siempre el mismo, no uno nuevo cada vez.
        assert_eq!(ficha_de(&raiz, Some(""), "PC01", 6).unwrap().id, g.id);
        assert_eq!(ficha_de(&raiz, Some(&f.id), "PC01", 7).unwrap().id, f.id);
        assert!(matches!(
            ficha_de(&raiz, Some("no-existe"), "PC01", 8),
            Err(Fallo::SinProyecto(_))
        ));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn anadir_sin_lista_crea_tareas_y_la_siguiente_va_a_la_misma() {
        let (raiz, f) = almacen_de_prueba("anadir");
        let l1 = anadir_tarea(&raiz, &f.id, "PC01", None, "pan", "Tareas").unwrap();
        let l2 = anadir_tarea(&raiz, &f.id, "PC01", None, "leche", "Tareas").unwrap();
        assert_eq!(l1.id, l2.id);
        assert_eq!(l2.texto, "# Tareas\n\n- [ ] pan\n- [ ] leche");
        let guardados = cuaderno_de(&raiz, &f.id);
        assert_eq!(guardados.len(), 1);
        assert_eq!(guardados[0].texto, l2.texto);
        assert_eq!(guardados[0].miniapp.as_deref(), Some(mini::TAREAS));
        // Caso negativo: una tarea vacia no deja nada, ni una lista nueva.
        let (raiz2, f2) = almacen_de_prueba("anadir-vacia");
        assert!(matches!(
            anadir_tarea(&raiz2, &f2.id, "PC01", None, "  ", "Tareas"),
            Err(Fallo::TareaVacia)
        ));
        assert!(cuaderno::Cuaderno::leer_de(&almacen::carpeta(&raiz2, &f2.id)).is_err());
        let _ = std::fs::remove_dir_all(&raiz);
        let _ = std::fs::remove_dir_all(&raiz2);
    }

    #[test]
    fn anadir_con_codigo_va_a_esa_lista_y_no_a_la_ultima() {
        let (raiz, f) = almacen_de_prueba("codigo");
        let compra = nueva_lista(&raiz, &f.id, "PC01", "Compra").unwrap();
        // La segunda nace despues: es «la ultima».
        std::thread::sleep(std::time::Duration::from_millis(5));
        let obra = nueva_lista(&raiz, &f.id, "PC01", "Obra").unwrap();
        anadir_tarea(&raiz, &f.id, "PC01", Some(&compra.id), "pan", "Tareas").unwrap();
        anadir_tarea(&raiz, &f.id, "PC01", None, "yeso", "Tareas").unwrap();
        let c = cuaderno_de(&raiz, &f.id);
        let de = |id: &str| c.iter().find(|m| m.id == id).unwrap().texto.clone();
        assert_eq!(de(&compra.id), "# Compra\n\n- [ ] pan");
        assert_eq!(de(&obra.id), "# Obra\n\n- [ ] yeso");
        // Por su codigo unico tambien se encuentra.
        anadir_tarea(
            &raiz,
            &f.id,
            "PC01",
            Some(&compra.codigo_unico()),
            "sal",
            "Tareas",
        )
        .unwrap();
        assert_eq!(
            nombre_de_lista(&mensaje_de(&raiz, &f.id, &compra.id).unwrap()),
            "Compra"
        );
        // Caso negativo: un mensaje que no es lista no se convierte en una.
        let nota = escribir_en_el_chat(&raiz, &f.id, "PC01", "una nota", 9).unwrap();
        assert!(matches!(
            anadir_tarea(&raiz, &f.id, "PC01", Some(&nota.id), "x", "Tareas"),
            Err(Fallo::NoEsLista)
        ));
        assert!(matches!(
            anadir_tarea(&raiz, &f.id, "PC01", Some("no-esta"), "x", "Tareas"),
            Err(Fallo::SinMensaje(_))
        ));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn marcar_pone_el_estado_pedido_y_repetirlo_no_lo_deshace() {
        let (raiz, f) = almacen_de_prueba("marcar");
        let l = anadir_tarea(&raiz, &f.id, "PC01", None, "pan", "Tareas").unwrap();
        anadir_tarea(&raiz, &f.id, "PC01", None, "leche", "Tareas").unwrap();
        let t = marcar_tarea(&raiz, &f.id, &l.id, 1, true).unwrap();
        assert_eq!(
            t,
            mini::Tarea {
                texto: "leche".into(),
                hecha: true
            }
        );
        // Dos veces lo mismo: sigue hecha (alternar la habria desmarcado).
        marcar_tarea(&raiz, &f.id, &l.id, 1, true).unwrap();
        let doc = mensaje_de(&raiz, &f.id, &l.id).unwrap().texto;
        assert_eq!(doc, "# Tareas\n\n- [ ] pan\n- [x] leche");
        marcar_tarea(&raiz, &f.id, &l.id, 1, false).unwrap();
        let doc = mensaje_de(&raiz, &f.id, &l.id).unwrap().texto;
        assert_eq!(doc, "# Tareas\n\n- [ ] pan\n- [ ] leche");
        // Caso negativo: una tarea que no existe no toca el documento.
        assert!(matches!(
            marcar_tarea(&raiz, &f.id, &l.id, 2, true),
            Err(Fallo::SinTarea(2))
        ));
        assert_eq!(mensaje_de(&raiz, &f.id, &l.id).unwrap().texto, doc);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn pinear_encuentra_el_fichero_como_el_chat() {
        let (raiz, f) = almacen_de_prueba("pinear");
        let carpeta = almacen::carpeta(&raiz, &f.id);
        // Por ruta, tal cual; una que no esta se dice.
        let suelto = raiz.join("suelto.png");
        std::fs::write(&suelto, b"x").unwrap();
        assert_eq!(
            fichero_a_pinear(&raiz, Some(suelto.clone()), None, None, "PC01", 1).unwrap(),
            suelto
        );
        assert!(matches!(
            fichero_a_pinear(&raiz, Some(raiz.join("no.png")), None, None, "PC01", 1),
            Err(Fallo::SinFichero(_))
        ));
        // Un adjunto por su codigo.
        let adjunto =
            crate::ventana_chat::adjuntar_en_proyecto(&raiz, &f.id, "PC01", "plano.pdf", b"%PDF", 1)
                .unwrap();
        let ruta = fichero_a_pinear(&raiz, None, Some(&f.id), Some(adjunto.id.clone()), "PC01", 1)
            .unwrap();
        assert!(ruta.is_file() && ruta.starts_with(&carpeta), "{}", ruta.display());
        // Un lienzo de la lista, sin `ruta` y solo con su `referencia`.
        let lienzo = cuaderno::Mensaje {
            id: "lz".into(),
            clase: Some(cuaderno::Clase::Dibujo),
            referencia: Some("dib-7".into()),
            ..Default::default()
        };
        cuaderno::anadir(&carpeta, &lienzo).unwrap();
        let dibujo = almacen::lienzo(&raiz, &f.id, "dib-7");
        std::fs::create_dir_all(dibujo.parent().unwrap()).unwrap();
        std::fs::write(&dibujo, b"{}").unwrap();
        assert_eq!(
            fichero_a_pinear(&raiz, None, Some(&f.id), Some("lz".into()), "PC01", 1).unwrap(),
            dibujo
        );
        // Casos negativos: una nota no tiene fichero, y sin nada no hay que.
        let nota = escribir_en_el_chat(&raiz, &f.id, "PC01", "solo texto", 2).unwrap();
        assert!(matches!(
            fichero_a_pinear(&raiz, None, Some(&f.id), Some(nota.id), "PC01", 1),
            Err(Fallo::MensajeSinFichero(_))
        ));
        assert!(matches!(
            fichero_a_pinear(&raiz, None, None, None, "PC01", 1),
            Err(Fallo::Roto(_))
        ));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn pinear_e_iconos_se_leen() {
        assert_eq!(
            leer(r#"{"pixpin":1,"accion":"pinear","ruta":"C:\\a.png"}"#).unwrap(),
            Pedido::Pinear {
                ruta: Some(PathBuf::from(r"C:\a.png")),
                proyecto: None,
                codigo: None
            }
        );
        assert_eq!(
            leer(r#"{"pixpin":1,"accion":"iconos","extensiones":["pdf"]}"#).unwrap(),
            Pedido::Iconos {
                extensiones: vec!["pdf".into()]
            }
        );
        // Caso negativo: «iconos» sin la lista no es un pedido.
        assert!(matches!(
            leer(r#"{"pixpin":1,"accion":"iconos"}"#),
            Err(Fallo::Roto(_))
        ));
    }
}
