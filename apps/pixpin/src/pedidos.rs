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
    /// `imagenes`: como en `AnadirTarea`; van al chat como fotos.
    /// `archivos`: rutas absolutas de ficheros que van detras como adjuntos
    /// (como soltarlos en el chat); en `texto`, `[archivo 01]`… marcan cada
    /// uno y se quitan.
    Chat {
        texto: String,
        proyecto: Option<String>,
        #[serde(default)]
        imagenes: Vec<PathBuf>,
        #[serde(default)]
        archivos: Vec<PathBuf>,
    },
    /// `imagenes`: como en `AnadirTarea`; cada `[img NN]` queda como su
    /// imagen dentro de la nota.
    NotaNueva {
        texto: Option<String>,
        proyecto: Option<String>,
        #[serde(default)]
        imagenes: Vec<PathBuf>,
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
    /// `imagenes`: rutas absolutas de imagenes que van con la tarea. En
    /// `texto`, `[img 01]`, `[img 02]`… (desde 1, en el orden de la lista)
    /// marcan donde va cada una; la que no tenga ficha va al final.
    AnadirTarea {
        texto: String,
        proyecto: Option<String>,
        codigo: Option<String>,
        #[serde(default)]
        imagenes: Vec<PathBuf>,
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
    /// La ficha de una leccion nueva (`LeccionActivity.nueva`), empezada con
    /// `texto` si viene, para el chat de `proyecto`. `imagenes`: como en
    /// `AnadirTarea`; son las fotos de la leccion (se guardan con ella al
    /// pulsar Guardar) y sus `[img NN]` salen del texto.
    LeccionNueva {
        texto: Option<String>,
        proyecto: Option<String>,
        #[serde(default)]
        imagenes: Vec<PathBuf>,
    },
    /// La lista de lecciones; con `proyecto`, las suyas primero, y con
    /// `consulta`, ya buscando.
    Lecciones {
        proyecto: Option<String>,
        consulta: Option<String>,
    },
    /// Hacer sonar un audio en el reproductor flotante, sin abrir el chat
    /// (Intro sobre un audio en Flow Launcher). `titulo` es lo que se lee
    /// en la barra; sin el, el nombre del fichero.
    Reproducir {
        ruta: PathBuf,
        titulo: Option<String>,
    },
    /// Pasar la tarea `indice` de la lista `codigo` (del chat `proyecto`) al
    /// final de la lista `a_codigo` (del chat `a_proyecto`), como «Mover a…»
    /// de la ventana de tareas.
    MoverTarea {
        proyecto: Option<String>,
        codigo: String,
        indice: usize,
        a_proyecto: Option<String>,
        a_codigo: String,
    },
    /// Abrir una de las ventanas de la bandeja.
    Ventana {
        cual: Cual,
    },
    /// Capturar como el atajo general. Sin `modo`, una zona.
    Capturar {
        modo: Option<ModoCaptura>,
    },
    /// Sacar a la pantalla la captura mas reciente.
    PinearUltima {},
    /// «Conservar» de la galeria: a «Mensajes guardados» y que no caduque.
    ConservarCaptura {
        ruta: PathBuf,
    },
    /// «Borrar» de la galeria: a la papelera de PixPin.
    BorrarCaptura {
        ruta: PathBuf,
    },
    /// Poner una imagen en el portapapeles, como imagen (no como fichero).
    CopiarImagen {
        ruta: PathBuf,
    },
}

/// Que ventana abre `ventana`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cual {
    Tareas,
    Galeria,
    Lecciones,
}

/// Que captura hace `capturar`. Solo la de zona, por ahora: la del atajo
/// general (`Comando::CapturarRegion`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModoCaptura {
    Zona,
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
    /// La tarea a mover ya no estaba donde se vio: no se toco nada.
    TareaCambio,
    /// La carpeta de capturas esta vacia.
    SinCapturas,
    /// Se pidio tocar un fichero que no es una captura de la carpeta.
    FueraDeCapturas(PathBuf),
    /// Se pidio copiar como imagen algo que no lo es.
    NoEsImagen(PathBuf),
    /// La imagen no se pudo leer o el portapapeles no la tomo.
    Imagen(String),
    /// Una imagen que tenia que ir con una tarea no esta en el disco.
    SinImagen(PathBuf),
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
            Fallo::TareaCambio => "la tarea ya no estaba donde se vio".into(),
            Fallo::SinCapturas => "no hay capturas".into(),
            Fallo::FueraDeCapturas(r) => {
                format!("fuera de la carpeta de capturas: {}", r.display())
            }
            Fallo::NoEsImagen(r) => format!("no es una imagen: {}", r.display()),
            Fallo::Imagen(e) => format!("imagen: {e}"),
            Fallo::SinImagen(r) => format!("no existe la imagen {}", r.display()),
        }
    }

    /// El aviso para el globo, ya traducido.
    pub(crate) fn aviso(&self, t: &Catalogo) -> String {
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
            Fallo::TareaCambio => "pedido-tarea-cambio",
            Fallo::SinCapturas => "pedido-sin-capturas",
            Fallo::FueraDeCapturas(_) => "pedido-fuera-de-capturas",
            Fallo::NoEsImagen(_) => "pedido-no-es-imagen",
            Fallo::Imagen(motivo) => {
                args.set("motivo", motivo.clone());
                "pedido-imagen-no-se-pudo"
            }
            Fallo::SinImagen(r) => {
                args.set("ruta", r.display().to_string());
                "pedido-sin-imagen"
            }
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
                | Pedido::PinearUltima { .. }
                | Pedido::Ventana { .. }
                | Pedido::LeccionNueva { .. }
                | Pedido::Lecciones { .. }
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
                // Una leccion abre su ficha, no el chat (no se ve en el).
                if let Some(id) = crate::lecciones::almacen::es_leccion(&m)
                    .then(|| {
                        m.ruta
                            .as_deref()
                            .and_then(|r| crate::lecciones::id_del_archivo(Path::new(r)))
                    })
                    .flatten()
                {
                    crate::lecciones::editar(cx.ubicacion.clone(), cx.idioma, cx.aparato, &id);
                    return Ok(Hecho::default());
                }
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
                // Una leccion abre su ficha, no un visor de archivos
                // (`MensajesActivity.kt`).
                if let Some(id) = crate::lecciones::id_del_archivo(&ruta) {
                    crate::lecciones::editar(cx.ubicacion.clone(), cx.idioma, cx.aparato, &id);
                    return Ok(Hecho::default());
                }
                return Ok(Hecho {
                    aviso: None,
                    ficheros: vec![ruta],
                });
            }
        },
        Pedido::Chat {
            texto,
            proyecto,
            imagenes,
            archivos,
        } => {
            let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
            chat_con_imagenes(raiz, &f.id, cx.aparato, &texto, &imagenes, &archivos, ahora)?;
            crate::ventana_chat::refrescar();
            return Ok(aviso("pedido-escrito", &[("proyecto", f.nombre)]));
        }
        Pedido::NotaNueva {
            texto,
            proyecto,
            imagenes,
        } => {
            let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
            let numeradas: Vec<(u32, PathBuf)> = imagenes
                .iter()
                .enumerate()
                .map(|(i, r)| (i as u32 + 1, r.clone()))
                .collect();
            let texto = match texto {
                Some(t) if !numeradas.is_empty() => Some(crate::tareas::con_imagenes_guardadas(
                    raiz, &f.id, &t, &numeradas,
                )?),
                t => t,
            };
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
            let ruta =
                fichero_a_pinear(raiz, ruta, proyecto.as_deref(), codigo, cx.aparato, ahora)?;
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
        // Como el microfono: la ventanita sale sola y se va al acabar.
        Pedido::Reproducir { ruta, titulo } => {
            if !ruta.is_file() {
                return Err(Fallo::SinFichero(ruta));
            }
            let titulo = titulo
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .or_else(|| ruta.file_stem().map(|s| s.to_string_lossy().to_string()))
                .unwrap_or_default();
            crate::ventana_chat::reproducir_flotante(t, ruta, titulo);
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
        // Las lecciones: su ventana es la respuesta.
        Pedido::LeccionNueva {
            texto,
            proyecto,
            imagenes,
        } => {
            let f = ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?;
            // Se leen ya: el borrador de Flow se vacia solo, y la ficha puede
            // quedarse abierta un buen rato antes de guardar.
            let fotos = leer_imagenes(&imagenes, "leccion", ahora)?;
            let texto = texto
                .map(|t| sin_fichas_de_imagen(&t, imagenes.len()))
                .filter(|t| !t.is_empty());
            crate::lecciones::nueva_con_fotos(
                cx.ubicacion.clone(),
                cx.idioma,
                cx.aparato,
                texto,
                None,
                Some(f.id),
                fotos,
            );
        }
        Pedido::Lecciones { proyecto, consulta } => {
            let ficha = match proyecto.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
                Some(_) => Some(ficha_de(raiz, proyecto.as_deref(), cx.aparato, ahora)?.id),
                None => None,
            };
            crate::lecciones::lista(cx.ubicacion.clone(), cx.idioma, cx.aparato, ficha, consulta);
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
            imagenes,
        } => {
            let lista = anadir_pedida(
                raiz,
                proyecto.as_deref(),
                codigo.as_deref(),
                cx.aparato,
                &texto,
                &imagenes,
                &t.t("pedido-tareas"),
                ahora,
            )?;
            crate::ventana_chat::refrescar();
            return Ok(aviso(
                "pedido-tarea-anadida",
                &[
                    ("tarea", lo_que_se_lee(&texto, imagenes.len())),
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
        Pedido::MoverTarea {
            proyecto,
            codigo,
            indice,
            a_proyecto,
            a_codigo,
        } => {
            let (tarea, lista) = mover_tarea(
                raiz,
                cx.aparato,
                ahora,
                (proyecto.as_deref(), &codigo, indice),
                (a_proyecto.as_deref(), &a_codigo),
            )?;
            crate::ventana_chat::refrescar();
            return Ok(aviso(
                "pedido-tarea-movida",
                &[("tarea", tarea), ("lista", lista)],
            ));
        }
        Pedido::VentanaPrincipal {} => {
            crate::ventana_chat::lanzar(cx.idioma, cx.ubicacion.clone(), cx.opciones);
        }
        // Las mismas entradas que la bandeja: su ventana es la respuesta.
        Pedido::Ventana { cual } => match cual {
            Cual::Tareas => crate::tareas::abrir(cx.idioma, cx.ubicacion.clone(), cx.aparato),
            Cual::Galeria => crate::galeria_capturas::abrir(cx.idioma, cx.ubicacion.clone()),
            Cual::Lecciones => {
                crate::lecciones::lista(cx.ubicacion.clone(), cx.idioma, cx.aparato, None, None)
            }
        },
        Pedido::Capturar { modo } => capturar(modo.unwrap_or(ModoCaptura::Zona)),
        Pedido::PinearUltima {} => {
            let ruta = ultima_captura(&crate::galeria_capturas::carpeta(cx.ubicacion))?;
            // Como `pinear` con una ruta: al `main`, que tiene los pines.
            return Ok(Hecho {
                aviso: None,
                ficheros: vec![ruta],
            });
        }
        Pedido::ConservarCaptura { ruta } => {
            let ruta = captura_de(raiz, &ruta)?;
            crate::galeria_capturas::conservar(raiz, &ruta)?;
            return Ok(aviso("galeria-conservada-aviso", &[]));
        }
        Pedido::BorrarCaptura { ruta } => {
            let ruta = captura_de(raiz, &ruta)?;
            crate::galeria_capturas::borrar(raiz, &ruta)?;
            return Ok(aviso("galeria-borrada", &[]));
        }
        Pedido::CopiarImagen { ruta } => {
            imagen_a_copiar(&ruta)?;
            pixpin_codec::imagen::cargar(&ruta)
                .map_err(|e| Fallo::Imagen(e.to_string()))
                .and_then(|img| {
                    pixpin_codec::copiar_imagen(&img).map_err(|e| Fallo::Imagen(e.to_string()))
                })?;
            return Ok(aviso("galeria-copiada", &[]));
        }
    }
    Ok(Hecho::default())
}

/// Lo que espera el lanzador antes de que salga la captura: su propia
/// ventana se esconde DESPUES de mandar el pedido, y sin esta espera
/// saldria en la foto.
const ESPERA_ANTES_DE_CAPTURAR: std::time::Duration = std::time::Duration::from_millis(300);

/// Empieza la captura por el mismo camino que el atajo general y la bandeja:
/// el `WM_COMMAND` del comando a la ventana del bucle, que la abre como si
/// se hubiera elegido en el menu. Asi no hay un segundo camino a la captura
/// que se quede atras. Desde un hilo aparte para no dormir el bucle.
fn capturar(modo: ModoCaptura) {
    let comando = match modo {
        ModoCaptura::Zona => pixpin_store::comandos::Comando::CapturarRegion,
    };
    let lanzado = std::thread::Builder::new()
        .name("captura-del-pedido".into())
        .spawn(move || {
            std::thread::sleep(ESPERA_ANTES_DE_CAPTURAR);
            if !pixpin_shell::mensajero::pedir_ventana_principal(comando.id()) {
                tracing::warn!("la captura del pedido no llego a la ventana");
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de la captura del pedido");
    }
}

/// La captura mas reciente de la carpeta, la que ensena la galeria la
/// primera.
fn ultima_captura(carpeta: &Path) -> Result<PathBuf, Fallo> {
    crate::galeria_capturas::listar(carpeta)
        .into_iter()
        .next()
        .map(|e| e.ruta)
        .ok_or(Fallo::SinCapturas)
}

/// Una ruta que viene de fuera para conservar o borrar: tiene que ser una
/// captura que este directamente en la carpeta de capturas. Lo demas no se
/// toca, aunque exista: un pedido mal hecho no puede llevarse a la papelera
/// un fichero cualquiera del disco.
fn captura_de(raiz: &Path, ruta: &Path) -> Result<PathBuf, Fallo> {
    if !ruta.is_file() {
        return Err(Fallo::SinFichero(ruta.to_path_buf()));
    }
    let carpeta = crate::galeria_capturas::carpeta_en(raiz);
    // Comparadas ya resueltas: `..`, mayusculas de la unidad o un enlace no
    // cuelan un fichero de fuera.
    let dentro = match (ruta.canonicalize(), carpeta.canonicalize()) {
        (Ok(r), Ok(c)) => {
            r.parent() == Some(c.as_path()) && crate::galeria_capturas::es_captura(&r)
        }
        _ => false,
    };
    if dentro {
        Ok(ruta.to_path_buf())
    } else {
        Err(Fallo::FueraDeCapturas(ruta.to_path_buf()))
    }
}

/// Las extensiones que `copiar_imagen` acepta. El GIF lo rechaza despues
/// el codec (no trae su lector): se dice con el aviso de «no se pudo».
const IMAGENES_A_COPIAR: [&str; 6] = ["png", "jpg", "jpeg", "bmp", "gif", "webp"];

/// Si la ruta se puede copiar como imagen: que este y que sea una imagen
/// por su extension. Leerla es cosa del codec, despues.
fn imagen_a_copiar(ruta: &Path) -> Result<(), Fallo> {
    let es_imagen = ruta
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGENES_A_COPIAR.iter().any(|x| x.eq_ignore_ascii_case(e)));
    if !es_imagen {
        return Err(Fallo::NoEsImagen(ruta.to_path_buf()));
    }
    if !ruta.is_file() {
        return Err(Fallo::SinFichero(ruta.to_path_buf()));
    }
    Ok(())
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
            crate::abrir_hoja::abrir_hoja(&raiz, &proyecto, &referencia, opciones);
            crate::ventana_chat::refrescar();
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del lienzo del pedido");
    }
}

/// La ficha del proyecto pedido. Sin proyecto (o vacio), «Mensajes
/// guardados», que se crea si todavia no existe: es adonde va lo suelto.
pub(crate) fn ficha_de(
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
pub(crate) fn mensaje_de(
    raiz: &Path,
    proyecto: &str,
    codigo: &str,
) -> Result<cuaderno::Mensaje, Fallo> {
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
    crate::ventana_chat::fichero_del_mensaje(raiz, &f.id, &m)
        .ok_or(Fallo::MensajeSinFichero(codigo))
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

/// Un mensaje del pedido `chat`. Sin `imagenes`, la nota de texto de
/// siempre. Con ellas (pegadas como `[img NN]` en el lanzador), el texto sin
/// sus fichas va primero y cada imagen detras como una foto del chat, como
/// si se hubiera soltado en la ventana (un BMP, como PNG). Antes de escribir
/// nada se comprueba que esten todas. Con `archivos` (pegados como
/// `[archivo NN]`), cada fichero va detras de las fotos como un adjunto con
/// su nombre, igual que al soltarlo en el chat (`meter_en_proyecto`).
fn chat_con_imagenes(
    raiz: &Path,
    proyecto: &str,
    aparato: &str,
    texto: &str,
    imagenes: &[PathBuf],
    archivos: &[PathBuf],
    ahora: i64,
) -> Result<(), Fallo> {
    if imagenes.is_empty() && archivos.is_empty() {
        escribir_en_el_chat(raiz, proyecto, aparato, texto, ahora)?;
        return Ok(());
    }
    let mut adjuntos = leer_imagenes(imagenes, "imagen", ahora)?;
    adjuntos.extend(leer_archivos(archivos)?);
    let limpio =
        sin_fichas_de_archivo(&sin_fichas_de_imagen(texto, imagenes.len()), archivos.len());
    if !limpio.is_empty() {
        escribir_en_el_chat(raiz, proyecto, aparato, &limpio, ahora)?;
    }
    crate::ventana_chat::meter_en_proyecto(raiz, proyecto, &adjuntos, aparato)?;
    Ok(())
}

/// Los ficheros `archivos` de un pedido `chat`, leidos con su nombre. Todos
/// o ninguno: uno que no este (o sea una carpeta) es un fallo del pedido
/// entero, antes de escribir nada.
fn leer_archivos(archivos: &[PathBuf]) -> Result<Vec<(String, Vec<u8>)>, Fallo> {
    if let Some(r) = archivos.iter().find(|r| !r.is_file()) {
        return Err(Fallo::SinFichero(r.clone()));
    }
    archivos
        .iter()
        .map(|r| {
            let nombre = r
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .ok_or_else(|| Fallo::SinFichero(r.clone()))?;
            Ok((nombre, std::fs::read(r)?))
        })
        .collect()
}

/// El texto sin las fichas `[archivo 01]`…`[archivo NN]` de sus `n`
/// ficheros, con los blancos de mas fuera.
fn sin_fichas_de_archivo(texto: &str, n: usize) -> String {
    let mut limpio = texto.to_string();
    for i in 1..=n as u32 {
        limpio = limpio.replace(&pixpin_lanzador::imagenes::ficha_de_archivo(i), " ");
    }
    limpio.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Las imagenes de un pedido, leidas y con nombre (`<prefijo>-<ms>-NN.ext`),
/// listas para meterlas en un chat. Un BMP pasa a PNG. Todas o ninguna: una
/// que falte o no sea imagen es un fallo del pedido entero.
fn leer_imagenes(
    imagenes: &[PathBuf],
    prefijo: &str,
    ahora: i64,
) -> Result<Vec<(String, Vec<u8>)>, Fallo> {
    let mut fotos = Vec::with_capacity(imagenes.len());
    for (i, r) in imagenes.iter().enumerate() {
        if !r.is_file() {
            return Err(Fallo::SinImagen(r.clone()));
        }
        if !crate::tareas::es_imagen(r) {
            return Err(Fallo::NoEsImagen(r.clone()));
        }
        let ext = r
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_ascii_lowercase();
        let (ext, bytes) = if ext == "bmp" {
            let img = pixpin_codec::imagen::cargar(r).map_err(|e| Fallo::Imagen(e.to_string()))?;
            let png = pixpin_codec::imagen::codificar_png(&img)
                .map_err(|e| Fallo::Imagen(e.to_string()))?;
            ("png".to_string(), png)
        } else {
            (ext, std::fs::read(r)?)
        };
        fotos.push((format!("{prefijo}-{ahora}-{:02}.{ext}", i + 1), bytes));
    }
    Ok(fotos)
}

/// El texto sin las fichas `[img 01]`…`[img NN]` de sus `n` imagenes (que
/// van aparte, como fotos), con los blancos de mas fuera.
fn sin_fichas_de_imagen(texto: &str, n: usize) -> String {
    let mut limpio = texto.to_string();
    for i in 1..=n as u32 {
        limpio = limpio.replace(&mini::ficha_de_imagen(i), " ");
    }
    limpio.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Si un mensaje es una lista de tareas.
pub(crate) fn es_lista(m: &cuaderno::Mensaje) -> bool {
    m.clase == Some(cuaderno::Clase::MiniApp) && m.miniapp.as_deref() == Some(mini::TAREAS)
}

/// Como se llama una lista: su titulo, o el nombre del mensaje si el
/// documento no tiene.
pub(crate) fn nombre_de_lista(m: &cuaderno::Mensaje) -> String {
    match mini::titulo(&m.texto) {
        t if t.is_empty() => m.nombre.clone(),
        t => t,
    }
}

/// Una lista de tareas nueva y vacia, con ese titulo, como la del clip.
pub(crate) fn nueva_lista(
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
pub(crate) fn anadir_tarea(
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
    let hoy = mini::Fecha::de_ms_locales(
        pixpin_shell::entorno::ahora_utc_ms() + pixpin_shell::entorno::desfase_local_ms(),
    );
    m.texto = mini::anadir_el(&m.texto, texto, hoy);
    reescribir(raiz, proyecto, &m)?;
    Ok(m)
}

/// La tarea de un pedido `anadir_tarea`: sin chat ni lista, al **Inbox** de
/// «Mensajes guardados» (`tareas::apuntar`, lo mismo que la caja de la
/// ventana de tareas), que es donde cae todo lo apuntado «asi nomas»; con
/// chat o lista, como siempre.
///
/// Con `imagenes`, cada una se copia al chat de la lista y su ficha
/// `[img NN]` (desde 1, en el orden de la lista) se cambia por el enlace
/// (`tareas::con_imagenes_guardadas`). Antes de copiar nada se comprueba
/// que esten todas y, si se pidio una lista concreta, que lo sea: un pedido
/// que no se hace no deja ficheros sueltos en el chat.
#[allow(clippy::too_many_arguments)] // lo que trae el pedido, mas raiz, aparato y hora
fn anadir_pedida(
    raiz: &Path,
    proyecto: Option<&str>,
    codigo: Option<&str>,
    aparato: &str,
    texto: &str,
    imagenes: &[PathBuf],
    titulo_nueva: &str,
    ahora: i64,
) -> Result<cuaderno::Mensaje, Fallo> {
    let numeradas: Vec<(u32, PathBuf)> = imagenes
        .iter()
        .enumerate()
        .map(|(i, r)| (i as u32 + 1, r.clone()))
        .collect();
    for r in imagenes {
        if !r.is_file() {
            return Err(Fallo::SinImagen(r.clone()));
        }
        if !crate::tareas::es_imagen(r) {
            return Err(Fallo::NoEsImagen(r.clone()));
        }
    }
    let hay = |x: Option<&str>| x.is_some_and(|x| !x.trim().is_empty());
    if !hay(proyecto) && !hay(codigo) {
        return crate::tareas::apuntar_con(raiz, aparato, texto, &numeradas);
    }
    let f = ficha_de(raiz, proyecto, aparato, ahora)?;
    if mini::saneado(texto).is_empty() && imagenes.is_empty() {
        return Err(Fallo::TareaVacia);
    }
    if let Some(c) = codigo.filter(|c| !c.trim().is_empty())
        && !es_lista(&mensaje_de(raiz, &f.id, c.trim())?)
    {
        return Err(Fallo::NoEsLista);
    }
    let texto = crate::tareas::con_imagenes_guardadas(raiz, &f.id, texto, &numeradas)?;
    anadir_tarea(raiz, &f.id, aparato, codigo, &texto, titulo_nueva)
}

/// Lo que dice el globo de una tarea pedida: su texto sin las fichas de
/// sus `cuantas` imagenes, o, si no lleva mas que imagenes, la primera
/// ficha (que algo hay que decir).
fn lo_que_se_lee(texto: &str, cuantas: usize) -> String {
    let mut s = mini::saneado(texto);
    for n in 1..=cuantas as u32 {
        s = s.replace(&mini::ficha_de_imagen(n), " ");
    }
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.is_empty() && cuantas > 0 {
        mini::ficha_de_imagen(1)
    } else {
        s
    }
}

/// Una lista de tareas como la ve la ventana de tareas, por su chat y su
/// codigo.
fn lista_pedida(
    raiz: &Path,
    proyecto: Option<&str>,
    codigo: &str,
    aparato: &str,
    ahora: i64,
) -> Result<crate::tareas::Lista, Fallo> {
    let f = ficha_de(raiz, proyecto, aparato, ahora)?;
    let m = mensaje_de(raiz, &f.id, codigo)?;
    crate::tareas::lista_de(&f, &m).ok_or(Fallo::NoEsLista)
}

/// Pasa la tarea `indice` de una lista (`desde`: chat y codigo) al final de
/// otra (`hasta`), con `tareas::mover`, el «Mover a…» de la ventana de
/// tareas. Devuelve el texto de la tarea (sin su fecha) y como se llama la
/// lista adonde fue. Si la tarea cambio entre leerla y moverla no se toca
/// nada: [`Fallo::TareaCambio`].
fn mover_tarea(
    raiz: &Path,
    aparato: &str,
    ahora: i64,
    desde: (Option<&str>, &str, usize),
    hasta: (Option<&str>, &str),
) -> Result<(String, String), Fallo> {
    let (proyecto, codigo, indice) = desde;
    let origen = lista_pedida(raiz, proyecto, codigo, aparato, ahora)?;
    let destino = lista_pedida(raiz, hasta.0, hasta.1, aparato, ahora)?;
    let Some(fila) = origen.filas.get(indice) else {
        return Err(Fallo::SinTarea(indice));
    };
    if !crate::tareas::mover(raiz, &origen, fila, &destino)? {
        return Err(Fallo::TareaCambio);
    }
    Ok((fila.texto.clone(), destino.titulo))
}

/// Pone la tarea numero `indice` (desde 0, en el orden del documento) como
/// hecha o pendiente. **Pone, no alterna**: un pedido repetido —un doble
/// Intro en el lanzador— no puede deshacer lo que hizo el primero. Devuelve
/// la tarea como quedo.
pub(crate) fn marcar_tarea(
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

/// Cuantas veces se reescribio un mensaje desde que arranco la app. Tachar
/// una tarea deja el cuaderno del mismo tamano, y hay discos que no mueven
/// la fecha a tiempo (paso en la CI): con esto la ventana de tareas se entera
/// siempre de lo escrito aqui. Lo llegado de fuera lo dice la fecha.
static REESCRITOS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Ver [`REESCRITOS`].
pub(crate) fn reescritos() -> u64 {
    REESCRITOS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Escribe el documento cambiado en su mensaje, como `guardar_mini` del
/// chat: `cuaderno::reemplazar` toma el cerrojo de los cuadernos y copia tal
/// cual lo que no entiende. Si el mensaje desaparecio entre leerlo y
/// escribirlo (lo borro el chat, o una sincronizacion), se dice.
pub(crate) fn reescribir(raiz: &Path, proyecto: &str, m: &cuaderno::Mensaje) -> Result<(), Fallo> {
    if cuaderno::reemplazar(&almacen::carpeta(raiz, proyecto), m)? {
        REESCRITOS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
    const EJEMPLOS: [(&str, &str); 22] = [
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
        ("pinear", r#"{"pixpin":1,"accion":"pinear","codigo":"1"}"#),
        (
            "iconos",
            r#"{"pixpin":1,"accion":"iconos","extensiones":["pdf","docx"]}"#,
        ),
        ("soltar", r#"{"pixpin":1,"accion":"soltar"}"#),
        (
            "reproducir",
            r#"{"pixpin":1,"accion":"reproducir","ruta":"C:\\a\\voz.m4a"}"#,
        ),
        (
            "leccion_nueva",
            r#"{"pixpin":1,"accion":"leccion_nueva","texto":"Revisar la escala [img 01]","imagenes":["C:\\a\\escala.png"]}"#,
        ),
        ("lecciones", r#"{"pixpin":1,"accion":"lecciones"}"#),
        (
            "mover_tarea",
            r#"{"pixpin":1,"accion":"mover_tarea","codigo":"1","indice":0,"a_codigo":"2"}"#,
        ),
        (
            "ventana",
            r#"{"pixpin":1,"accion":"ventana","cual":"tareas"}"#,
        ),
        ("capturar", r#"{"pixpin":1,"accion":"capturar"}"#),
        ("pinear_ultima", r#"{"pixpin":1,"accion":"pinear_ultima"}"#),
        (
            "conservar_captura",
            r#"{"pixpin":1,"accion":"conservar_captura","ruta":"C:\\c\\captura-0001.png"}"#,
        ),
        (
            "borrar_captura",
            r#"{"pixpin":1,"accion":"borrar_captura","ruta":"C:\\c\\captura-0001.png"}"#,
        ),
        (
            "copiar_imagen",
            r#"{"pixpin":1,"accion":"copiar_imagen","ruta":"C:\\a\\foto.jpg"}"#,
        ),
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
                proyecto: None,
                imagenes: vec![],
                archivos: vec![]
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

    #[test]
    fn un_chat_con_imagenes_manda_el_texto_y_cada_imagen_como_foto() {
        let (raiz, f) = almacen_de_prueba("chat-imagenes");
        let png = raiz.join("pegada.png");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\nfalsa").unwrap();
        chat_con_imagenes(
            &raiz,
            &f.id,
            "PC01",
            "mira esto [img 01]",
            std::slice::from_ref(&png),
            &[],
            5,
        )
        .unwrap();
        let v = cuaderno_de(&raiz, &f.id);
        assert_eq!(v.len(), 2, "{v:#?}");
        assert_eq!(v[0].texto, "mira esto", "el texto, sin la ficha");
        assert_eq!(v[1].clase, Some(cuaderno::Clase::Imagen));
        let ruta = v[1].ruta.clone().unwrap();
        assert!(
            almacen::carpeta(&raiz, &f.id).join(&ruta).is_file(),
            "{ruta}"
        );
        // Caso negativo: una imagen que no esta no deja nada escrito.
        let antes = cuaderno_de(&raiz, &f.id).len();
        assert!(matches!(
            chat_con_imagenes(
                &raiz,
                &f.id,
                "PC01",
                "otra [img 01]",
                &[raiz.join("no.png")],
                &[],
                6
            ),
            Err(Fallo::SinImagen(_))
        ));
        assert_eq!(cuaderno_de(&raiz, &f.id).len(), antes);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_chat_con_archivos_los_adjunta_como_al_soltarlos() {
        let p = leer(
            r#"{"pixpin":1,"accion":"chat","texto":"plano [archivo 01]","archivos":["C:\\a\\plano.pdf"]}"#,
        )
        .unwrap();
        assert!(matches!(
            &p,
            Pedido::Chat { archivos, imagenes, .. }
                if archivos == &[PathBuf::from(r"C:\a\plano.pdf")] && imagenes.is_empty()
        ));
        let (raiz, f) = almacen_de_prueba("chat-archivos");
        let fuera = raiz.join("escritorio");
        std::fs::create_dir_all(fuera.join("carpeta")).unwrap();
        let pdf = fuera.join("plano.pdf");
        let png = fuera.join("foto.png");
        std::fs::write(&pdf, b"%PDF-1.4 falso").unwrap();
        std::fs::write(&png, b"\x89PNG\r\n\x1a\nfalsa").unwrap();
        chat_con_imagenes(
            &raiz,
            &f.id,
            "PC01",
            "va [archivo 01] y la foto [img 01]",
            std::slice::from_ref(&png),
            std::slice::from_ref(&pdf),
            5,
        )
        .unwrap();
        let v = cuaderno_de(&raiz, &f.id);
        assert_eq!(v.len(), 3, "{v:#?}");
        assert_eq!(v[0].texto, "va y la foto", "el texto, sin las fichas");
        assert_eq!(v[1].clase, Some(cuaderno::Clase::Imagen));
        let ruta = v[2].ruta.clone().unwrap();
        assert!(ruta.ends_with("plano.pdf"), "{ruta}");
        assert_eq!(
            std::fs::read(almacen::carpeta(&raiz, &f.id).join(&ruta)).unwrap(),
            b"%PDF-1.4 falso"
        );
        // Caso negativo: un fichero que no esta, o una carpeta, no dejan
        // nada escrito (ni el texto ni los demas).
        let antes = cuaderno_de(&raiz, &f.id).len();
        for malo in [fuera.join("no.docx"), fuera.join("carpeta")] {
            assert!(matches!(
                chat_con_imagenes(
                    &raiz,
                    &f.id,
                    "PC01",
                    "otra [archivo 01] [archivo 02]",
                    &[],
                    &[pdf.clone(), malo],
                    6
                ),
                Err(Fallo::SinFichero(_))
            ));
        }
        assert_eq!(cuaderno_de(&raiz, &f.id).len(), antes);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn una_leccion_con_imagenes_las_lee_ya_y_quita_sus_fichas_del_texto() {
        let p = leer(
            r#"{"pixpin":1,"accion":"leccion_nueva","texto":"Mirar el plano [img 01] antes","imagenes":["C:\\a\\uno.png"]}"#,
        )
        .unwrap();
        assert_eq!(
            p,
            Pedido::LeccionNueva {
                texto: Some("Mirar el plano [img 01] antes".into()),
                proyecto: None,
                imagenes: vec![PathBuf::from(r"C:\a\uno.png")]
            }
        );
        // Sin `imagenes` sigue valiendo, como antes.
        assert!(matches!(
            leer(r#"{"pixpin":1,"accion":"leccion_nueva"}"#),
            Ok(Pedido::LeccionNueva { imagenes, .. }) if imagenes.is_empty()
        ));
        assert_eq!(
            sin_fichas_de_imagen("Mirar el plano [img 01] antes [img 02]", 2),
            "Mirar el plano antes"
        );
        // Una ficha de mas (sin su imagen) se queda: no es de ninguna foto.
        assert_eq!(sin_fichas_de_imagen("a [img 02]", 1), "a [img 02]");
        let (raiz, _) = almacen_de_prueba("leccion-imagenes");
        let png = raiz.join("pegada.png");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\nfalsa").unwrap();
        let fotos = leer_imagenes(&[png.clone(), png], "leccion", 7).unwrap();
        assert_eq!(fotos.len(), 2);
        assert_eq!(fotos[0].0, "leccion-7-01.png");
        assert_eq!(fotos[1].0, "leccion-7-02.png");
        assert!(fotos[0].1.starts_with(b"\x89PNG"));
        // Caso negativo: una que falta o que no es imagen tumba el pedido.
        assert!(matches!(
            leer_imagenes(&[raiz.join("no.png")], "leccion", 7),
            Err(Fallo::SinImagen(_))
        ));
        let txt = raiz.join("nota.txt");
        std::fs::write(&txt, b"hola").unwrap();
        assert!(matches!(
            leer_imagenes(&[txt], "leccion", 7),
            Err(Fallo::NoEsImagen(_))
        ));
        let _ = std::fs::remove_dir_all(&raiz);
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

    /// El documento sin la fecha de creacion de cada tarea (`➕ AAAA-MM-DD`,
    /// la de hoy, que `anadir_tarea` pone): lo que se comprueba es lo demas.
    fn sin_fechas(documento: &str) -> String {
        documento
            .split('\n')
            .map(|l| mini::partir(l).0)
            .collect::<Vec<_>>()
            .join("\n")
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
        assert_eq!(sin_fechas(&l2.texto), "# Tareas\n\n- [ ] pan\n- [ ] leche");
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
        assert_eq!(sin_fechas(&de(&compra.id)), "# Compra\n\n- [ ] pan");
        assert_eq!(sin_fechas(&de(&obra.id)), "# Obra\n\n- [ ] yeso");
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
        assert_eq!((mini::partir(&t.texto).0, t.hecha), ("leche", true));
        // Dos veces lo mismo: sigue hecha (alternar la habria desmarcado).
        marcar_tarea(&raiz, &f.id, &l.id, 1, true).unwrap();
        let doc = mensaje_de(&raiz, &f.id, &l.id).unwrap().texto;
        assert_eq!(sin_fechas(&doc), "# Tareas\n\n- [ ] pan\n- [x] leche");
        marcar_tarea(&raiz, &f.id, &l.id, 1, false).unwrap();
        let doc = mensaje_de(&raiz, &f.id, &l.id).unwrap().texto;
        assert_eq!(sin_fechas(&doc), "# Tareas\n\n- [ ] pan\n- [ ] leche");
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
        let adjunto = crate::ventana_chat::adjuntar_en_proyecto(
            &raiz,
            &f.id,
            "PC01",
            "plano.pdf",
            b"%PDF",
            1,
        )
        .unwrap();
        let ruta = fichero_a_pinear(
            &raiz,
            None,
            Some(&f.id),
            Some(adjunto.id.clone()),
            "PC01",
            1,
        )
        .unwrap();
        assert!(
            ruta.is_file() && ruta.starts_with(&carpeta),
            "{}",
            ruta.display()
        );
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

    #[test]
    fn anadir_sin_chat_ni_lista_va_al_inbox_de_mensajes_guardados() {
        let (raiz, f) = almacen_de_prueba("inbox");
        let l = anadir_pedida(
            &raiz,
            None,
            None,
            "PC01",
            "llamar al fontanero",
            &[],
            "Tareas",
            1,
        )
        .unwrap();
        let l2 = anadir_pedida(&raiz, Some("  "), None, "PC01", "pan", &[], "Tareas", 2).unwrap();
        assert_eq!(l.id, l2.id, "las dos al mismo Inbox");
        assert_eq!(nombre_de_lista(&l2), crate::tareas::INBOX);
        let g = ficha_de(&raiz, None, "PC01", 3).unwrap();
        assert_eq!(
            sin_fechas(&mensaje_de(&raiz, &g.id, &l.id).unwrap().texto),
            "# Inbox\n\n- [ ] llamar al fontanero\n- [ ] pan"
        );
        // Caso negativo: con un chat no va al Inbox, sino a la lista de ese
        // chat, como antes.
        let c = anadir_pedida(&raiz, Some(&f.id), None, "PC01", "yeso", &[], "Tareas", 4).unwrap();
        assert_eq!(nombre_de_lista(&c), "Tareas");
        assert_eq!(cuaderno_de(&raiz, &f.id).len(), 1);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn anadir_tarea_con_imagenes_las_copia_al_chat_de_la_lista() {
        // El campo se lee; sin el, la lista vacia (los pedidos de antes).
        let p = leer(r#"{"pixpin":1,"accion":"anadir_tarea","texto":"yeso [img 01]","imagenes":["C:\\a\\x.png"]}"#).unwrap();
        assert!(
            matches!(&p, Pedido::AnadirTarea { imagenes, .. } if imagenes == &[PathBuf::from("C:\\a\\x.png")])
        );
        let p = leer(r#"{"pixpin":1,"accion":"anadir_tarea","texto":"pan"}"#).unwrap();
        assert!(matches!(&p, Pedido::AnadirTarea { imagenes, .. } if imagenes.is_empty()));

        let (raiz, f) = almacen_de_prueba("tarea-imagenes");
        let fuera = raiz.join("fuera");
        std::fs::create_dir_all(&fuera).unwrap();
        let foto = |n: &str| {
            let r = fuera.join(n);
            let img = pixpin_codec::ImagenRgba {
                ancho: 1,
                alto: 1,
                pixeles: vec![9; 4],
            };
            pixpin_codec::guardar(&img, &r, pixpin_codec::FormatoImagen::Png).unwrap();
            r
        };
        let (a, b) = (foto("a.png"), foto("b b.png"));
        let l = anadir_pedida(
            &raiz,
            Some(&f.id),
            None,
            "PC01",
            "[img 02] yeso",
            &[a.clone(), b],
            "Tareas",
            1,
        )
        .unwrap();
        let t = &mini::leer_tareas(&l.texto)[0];
        let (visible, creada) = mini::partir(&t.texto);
        assert!(creada.is_some(), "la fecha sigue al final: {}", t.texto);
        let (lee, enlaces) = mini::imagenes_de(visible);
        assert_eq!(lee, "yeso");
        assert_eq!(enlaces.len(), 2);
        assert!(
            visible.starts_with("![img 02]("),
            "la 2 en su ficha: {visible}"
        );
        assert!(
            visible.contains("yeso ![img 01]("),
            "la 1, sin ficha, al final: {visible}"
        );
        for e in &enlaces {
            let r = crate::tareas::ruta_de_imagen(&raiz, &f.id, e).unwrap();
            assert!(r.starts_with(almacen::carpeta(&raiz, &f.id).join("archivos")));
        }
        assert_eq!(lo_que_se_lee("[img 02] yeso", 2), "yeso");
        assert_eq!(lo_que_se_lee("[img 01]", 1), "[img 01]");

        // Casos negativos: una imagen que no esta, o algo que no es una
        // imagen, no apuntan nada ni dejan ficheros copiados.
        let antes = std::fs::read_dir(almacen::carpeta(&raiz, &f.id).join("archivos"))
            .unwrap()
            .count();
        let falta = anadir_pedida(
            &raiz,
            Some(&f.id),
            None,
            "PC01",
            "x",
            &[a.clone(), fuera.join("no.png")],
            "Tareas",
            2,
        );
        assert!(matches!(falta, Err(Fallo::SinImagen(_))), "{falta:?}");
        let txt = fuera.join("t.txt");
        std::fs::write(&txt, "x").unwrap();
        assert!(matches!(
            anadir_pedida(&raiz, Some(&f.id), None, "PC01", "x", &[txt], "Tareas", 3),
            Err(Fallo::NoEsImagen(_))
        ));
        // Una lista pedida que no es lista tampoco copia nada.
        let nota =
            crate::ventana_chat::escribir_nota(&raiz, &f.id, "PC01", "nota", 4, 50, None).unwrap();
        assert!(matches!(
            anadir_pedida(
                &raiz,
                Some(&f.id),
                Some(&nota.id),
                "PC01",
                "x",
                &[a],
                "Tareas",
                5
            ),
            Err(Fallo::NoEsLista)
        ));
        assert_eq!(
            std::fs::read_dir(almacen::carpeta(&raiz, &f.id).join("archivos"))
                .unwrap()
                .count(),
            antes
        );
        assert_eq!(
            mini::leer_tareas(&mensaje_de(&raiz, &f.id, &l.id).unwrap().texto).len(),
            1
        );
        // El aviso dice que imagen falta.
        let t = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        assert!(
            Fallo::SinImagen(PathBuf::from("C:\\x\\foto.png"))
                .aviso(&t)
                .contains("foto.png")
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn mover_pasa_la_tarea_de_una_lista_a_otra_y_dice_adonde() {
        let (raiz, f) = almacen_de_prueba("mover");
        let compra = nueva_lista(&raiz, &f.id, "PC01", "Compra").unwrap();
        let obra = nueva_lista(&raiz, &f.id, "PC01", "Obra").unwrap();
        anadir_tarea(&raiz, &f.id, "PC01", Some(&compra.id), "pan", "Tareas").unwrap();
        anadir_tarea(&raiz, &f.id, "PC01", Some(&compra.id), "yeso", "Tareas").unwrap();
        let (tarea, lista) = mover_tarea(
            &raiz,
            "PC01",
            1,
            (Some(&f.id), &compra.id, 1),
            (Some(&f.id), &obra.id),
        )
        .unwrap();
        assert_eq!((tarea.as_str(), lista.as_str()), ("yeso", "Obra"));
        let doc = |id: &str| sin_fechas(&mensaje_de(&raiz, &f.id, id).unwrap().texto);
        assert_eq!(doc(&compra.id), "# Compra\n\n- [ ] pan");
        assert_eq!(doc(&obra.id), "# Obra\n\n- [ ] yeso");
        // Al Inbox de «Mensajes guardados», sin `a_proyecto`.
        let inbox = crate::tareas::apuntar(&raiz, "PC01", "otra").unwrap();
        mover_tarea(
            &raiz,
            "PC01",
            1,
            (Some(&f.id), &compra.id, 0),
            (None, &inbox.id),
        )
        .unwrap();
        let g = ficha_de(&raiz, None, "PC01", 1).unwrap();
        assert_eq!(
            sin_fechas(&mensaje_de(&raiz, &g.id, &inbox.id).unwrap().texto),
            "# Inbox\n\n- [ ] otra\n- [ ] pan"
        );
        // Casos negativos: una tarea que no esta, o un destino que no es una
        // lista, no tocan nada.
        let antes = doc(&obra.id);
        assert!(matches!(
            mover_tarea(
                &raiz,
                "PC01",
                1,
                (Some(&f.id), &obra.id, 5),
                (Some(&f.id), &compra.id)
            ),
            Err(Fallo::SinTarea(5))
        ));
        let nota = escribir_en_el_chat(&raiz, &f.id, "PC01", "una nota", 9).unwrap();
        assert!(matches!(
            mover_tarea(
                &raiz,
                "PC01",
                1,
                (Some(&f.id), &obra.id, 0),
                (Some(&f.id), &nota.id)
            ),
            Err(Fallo::NoEsLista)
        ));
        assert_eq!(doc(&obra.id), antes);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn ventana_capturar_y_los_de_capturas_se_leen() {
        assert_eq!(
            leer(r#"{"pixpin":1,"accion":"ventana","cual":"galeria"}"#).unwrap(),
            Pedido::Ventana {
                cual: Cual::Galeria
            }
        );
        assert_eq!(
            leer(r#"{"pixpin":1,"accion":"capturar","modo":"zona"}"#).unwrap(),
            Pedido::Capturar {
                modo: Some(ModoCaptura::Zona)
            }
        );
        assert_eq!(
            leer(r#"{"pixpin":1,"accion":"capturar"}"#).unwrap(),
            Pedido::Capturar { modo: None }
        );
        assert_eq!(
            leer(r#"{"pixpin":1,"accion":"mover_tarea","proyecto":"p","codigo":"1","indice":2,"a_codigo":"9"}"#)
                .unwrap(),
            Pedido::MoverTarea {
                proyecto: Some("p".into()),
                codigo: "1".into(),
                indice: 2,
                a_proyecto: None,
                a_codigo: "9".into()
            }
        );
        // Casos negativos: una ventana o un modo que no existen, y los que
        // les falta lo que tocan.
        for json in [
            r#"{"pixpin":1,"accion":"ventana","cual":"universo"}"#,
            r#"{"pixpin":1,"accion":"ventana"}"#,
            r#"{"pixpin":1,"accion":"capturar","modo":"pantalla"}"#,
            r#"{"pixpin":1,"accion":"mover_tarea","codigo":"1","indice":0}"#,
            r#"{"pixpin":1,"accion":"conservar_captura"}"#,
            r#"{"pixpin":1,"accion":"borrar_captura"}"#,
            r#"{"pixpin":1,"accion":"copiar_imagen"}"#,
        ] {
            assert!(matches!(leer(json), Err(Fallo::Roto(_))), "{json}");
        }
    }

    /// Una carpeta de capturas de prueba, con sus ficheros de mas viejo a
    /// mas nuevo.
    fn capturas_de_prueba(raiz: &Path, nombres: &[&str]) -> Vec<PathBuf> {
        let carpeta = crate::galeria_capturas::carpeta_en(raiz);
        std::fs::create_dir_all(&carpeta).unwrap();
        let base = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        nombres
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let r = carpeta.join(n);
                std::fs::write(&r, b"x").unwrap();
                std::fs::File::options()
                    .write(true)
                    .open(&r)
                    .unwrap()
                    .set_modified(base + std::time::Duration::from_secs(60 * i as u64))
                    .unwrap();
                r
            })
            .collect()
    }

    #[test]
    fn pinear_ultima_saca_la_captura_mas_nueva() {
        let (raiz, _) = almacen_de_prueba("ultima");
        let r = capturas_de_prueba(&raiz, &["captura-0002.png", "captura-0001.png"]);
        let carpeta = crate::galeria_capturas::carpeta_en(&raiz);
        // Por fecha, no por nombre: la 0001 se escribio despues.
        assert_eq!(ultima_captura(&carpeta).unwrap(), r[1]);
        // Caso negativo: sin capturas (ni carpeta) se dice, no se pinea nada.
        assert!(matches!(
            ultima_captura(&raiz.join("no-esta")),
            Err(Fallo::SinCapturas)
        ));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn conservar_o_borrar_solo_tocan_capturas_de_su_carpeta() {
        let (raiz, _) = almacen_de_prueba("capturas");
        let r = capturas_de_prueba(&raiz, &["captura-0001.png", "notas.txt"]);
        assert_eq!(captura_de(&raiz, &r[0]).unwrap(), r[0]);
        // Casos negativos: un fichero de fuera, uno que no es captura aunque
        // este dentro, uno colado con `..` y uno que no esta.
        let fuera = raiz.join("captura-0009.png");
        std::fs::write(&fuera, b"x").unwrap();
        let colado = crate::galeria_capturas::carpeta_en(&raiz)
            .join("..")
            .join("captura-0009.png");
        for ruta in [&fuera, &r[1], &colado] {
            assert!(
                matches!(captura_de(&raiz, ruta), Err(Fallo::FueraDeCapturas(_))),
                "{}",
                ruta.display()
            );
        }
        assert!(matches!(
            captura_de(&raiz, &raiz.join("capturas").join("captura-0404.png")),
            Err(Fallo::SinFichero(_))
        ));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn conservar_la_manda_a_guardados_y_borrarla_la_olvida() {
        let (raiz, _) = almacen_de_prueba("conservar");
        let r = capturas_de_prueba(&raiz, &["captura-0001.png"]);
        let nombre = crate::caducidad_capturas::nombre(&r[0]);
        let reg = crate::galeria_capturas::conservar(&raiz, &r[0]).unwrap();
        assert!(reg.conservadas.contains(&nombre));
        let g = ficha_de(&raiz, None, "PC01", 1).unwrap();
        assert_eq!(
            cuaderno_de(&raiz, &g.id).len(),
            1,
            "entra en Mensajes guardados"
        );
        let reg = crate::galeria_capturas::borrar(&raiz, &r[0])
            .unwrap()
            .unwrap();
        assert!(!reg.conservadas.contains(&nombre), "su nombre queda libre");
        assert!(!r[0].exists());
        assert!(
            raiz.join("papelera")
                .join("capturas")
                .join("captura-0001.png")
                .is_file()
        );
        // Caso negativo: una que no estaba conservada no cambia el registro.
        let r2 = capturas_de_prueba(&raiz, &["captura-0002.png"]);
        assert!(
            crate::galeria_capturas::borrar(&raiz, &r2[0])
                .unwrap()
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn copiar_imagen_solo_acepta_imagenes_que_esten() {
        let (raiz, _) = almacen_de_prueba("copiar");
        let foto = raiz.join("foto.JPG");
        std::fs::write(&foto, b"x").unwrap();
        assert!(imagen_a_copiar(&foto).is_ok());
        // Casos negativos: algo que no es una imagen, y una que no esta.
        let texto = raiz.join("notas.txt");
        std::fs::write(&texto, b"x").unwrap();
        assert!(matches!(imagen_a_copiar(&texto), Err(Fallo::NoEsImagen(_))));
        assert!(matches!(
            imagen_a_copiar(&raiz.join("no.png")),
            Err(Fallo::SinFichero(_))
        ));
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
