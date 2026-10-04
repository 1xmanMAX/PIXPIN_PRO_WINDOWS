//! PixPin Max — punto de entrada.
//!
//! PixPin es marca de DepthPixel. Este proyecto es una implementacion
//! personal e independiente.
//!
//! El orden de arranque no es arbitrario:
//!
//! 1. Instancia unica primero, para no hacer trabajo que habra que deshacer.
//! 2. Rutas, porque el registro a fichero necesita saber donde escribir.
//! 3. Registro a fichero, antes de leer los ajustes: con
//!    `#![windows_subsystem = "windows"]` no hay stderr, asi que si el
//!    usuario edito `pixpinmax.toml` a mano y se equivoco, el fallo tiene
//!    que quedar escrito en algun sitio en vez de perderse en silencio.
//! 4. Ajustes.
//! 5. Arranque con Windows, que solo depende de los ajustes.
//! 6. Idioma, antes de crear nada que muestre texto.
//! 7. Ventana, bandeja y atajos.
//! 8. Bucle, que duerme hasta que pasa algo.
//!
//! Todo el cuerpo vive en [`arrancar`], que devuelve `Result`. `main` solo
//! llama a `arrancar` y, si falla, ademas de dejar constancia en el
//! registro (si ya se pudo abrir) muestra un cuadro de error: es el unico
//! canal que le queda a esta aplicacion sin consola para avisar al usuario
//! de un fallo de arranque que ocurrio antes de tener bandeja con la que
//! decirlo. El catalogo de idiomas puede no estar cargado todavia cuando
//! esto pasa, asi que el mensaje puede salir sin traducir; es aceptable,
//! mejor eso que un fallo mudo.
//!
//! El guardia del registro (`WorkerGuard`) lo posee `main`, no `arrancar`:
//! `arrancar` recibe un `&mut Option<WorkerGuard>` y lo rellena en cuanto
//! sabe donde escribir. Si el guardia viviera dentro de `arrancar`, se
//! soltaria (cerrando el hilo de escritura no bloqueante) al salir con
//! `Err`, *antes* de que `main` pudiera registrar el error -- ese fue
//! exactamente el fallo que la re-revision encontro ejecutando el
//! programa con un `pixpinmax.toml` corrupto: el `tracing::error!` de
//! `main` se ejecutaba, pero el escritor ya estaba cerrado y la linea
//! nunca llegaba al fichero.

// Sin consola: es una aplicacion de bandeja, no una herramienta de linea de
// comandos. Sin esto se abriria una ventana negra al arrancar.
#![windows_subsystem = "windows"]
// Este ejecutable no esta en la lista de crates auditados para `unsafe` del
// documento maestro (esa es `pixpin-shell`, la unica que habla con Win32 de
// forma revisada). Sin este `forbid`, añadir una dependencia como `windows`
// aqui -- como paso por error en la revision anterior, para un unico
// `MessageBoxW` que ya se movio a `pixpin-shell::dialogo` -- crearia
// `unsafe` sin auditar por omision, no por decision, y ningun test de capas
// lo detectaria porque esas pruebas ignoran los crates externos. Este
// atributo es la unica guarda que lo habria detectado.
#![forbid(unsafe_code)]

mod abrir_hoja;
mod abrir_imagen;
mod aligerar;
mod anotado_del_adjunto;
mod anotador_al_chat;
mod audio;
mod biblioteca_audio;
mod buscador;
mod buscar_todo;
mod caducidad_capturas;
mod caja_dibujo;
mod capa;
mod captura2;
mod centro_bandeja;
mod cielo;
mod compartir;
mod conversacion;
mod cuenta_atras;
mod diapositivas;
mod dibujo;
mod dispositivo_perdido;
mod editor;
mod fondo_lienzo;
mod foto_anotada;
mod fusionar_paginas;
mod galeria_capturas;
mod gif;
mod grabador;
mod grupos_ventanas;
mod imagenes_lienzo;
mod lecciones;
mod lector;
mod lector_pdf;
mod lector_pdf_proyecto;
mod lector_tinta;
mod leer_en_voz;
mod llamada;
mod marco_de_la_tinta;
mod medir_fotogramas;
mod mini_panel;
mod miniaturas;
mod navegacion;
mod notas_md;
mod overlay;
mod panel_dibujo;
mod pdf_del_proyecto;
mod pdf_en_chat;
mod pedidos;
mod pegar_en_flow;
mod pila_capturas;
mod pin_vivo;
mod pines;
mod pronunciar;
mod punto_de_proyecto;
mod recibir;
mod recordatorios;
mod renombrar_doc;
mod reproductor;
mod salto_por_enlace;
mod scroll;
mod sincronizar;
mod tareas;
mod teleprompter;
mod tema_cosmos;
mod turno_pesado;
mod ventana_ajustes;
mod ventana_chat;
mod ventana_editor;
mod ventanita;
mod visor;
mod visor_html;
mod voz;
mod voz_en_chat;
mod zona_al_chat;

use anyhow::{Context, Result};
use overlay::{AccionFinal, ModoConfirmacion, Recursos, TextosBarra, ejecutar_overlay};
use pines::Pines;
use pixpin_nivel::{Nivel, Preferencia};
use pixpin_shell::{
    Bandeja, BotonGesto, Continuar, EtiquetasMenu, Evento, VentanaMensajes,
    adquirir_instancia_unica, arranque, atajos, entorno,
};
use pixpin_store::ajustes::PreferenciaNivel;
use pixpin_store::{Almacen, Catalogo, Ubicacion, ajustes, comandos, idioma, rutas};
use pixpin_ui::FormatoColorLupa;

/// El identificador del «Editor» de la bandeja (tarea 11, el hito).
///
/// Ni pasa por `comandos::CATALOGO` (no tiene atajo, ni traduccion todavia:
/// es lo minimo para poder probar el editor a mano) ni cae en el rango de
/// las regiones guardadas (que empieza en `pixpin_store::regiones::PRIMER_ID`,
/// 1000): un hueco propio evita chocar con cualquiera de los dos.
///
/// **Este comentario se olvidaba de un tercero, y salio caro.** Los grupos
/// ocultos ocupaban «todo lo que pase de 200» sin tope, asi que el 900 caia
/// dentro y cada clic en «Editor» se convertia en «muestra el grupo 700».
/// No fallaba nada y no avisaba nadie: el editor era inalcanzable desde la
/// bandeja y su brazo en el bucle nunca llego a ejecutarse. Ahora el tramo
/// de los grupos tiene tope y la guarda de abajo lo comprueba al compilar.
const ID_VENTANA_EDITOR: u32 = 900;

/// Que el identificador del editor quede fuera del tramo de los grupos
/// ocultos no puede depender de que alguien se acuerde: si algun dia se
/// mueve cualquiera de los dos numeros, esto no compila. Una guarda en
/// tiempo de compilacion, no una prueba, porque el coste es cero y el aviso
/// llega antes.
const _: () = assert!(
    ID_VENTANA_EDITOR >= pixpin_shell::ventana::ID_MENU_GRUPO_TOPE,
    "el identificador del Editor cae dentro del tramo de los grupos ocultos: \
     el menu lo convertiria en MostrarGrupo y el editor no se abriria"
);

// El 901 era el del universo, que se quito (3-oct-2026, como en el movil).
/// «Abrir documento…»: el visor de Word, libros y paginas (tanda 2).
const ID_ABRIR_DOCUMENTO: u32 = 902;
/// «Grupos de ventanas…» (H9): guardar y reabrir lienzos, lectores y notas
/// con su sitio. Sin atajo: en el movil tambien es una entrada de menu.
const ID_GRUPOS_VENTANAS: u32 = 903;
/// «Galeria de capturas…» (2-oct): todas las capturas guardadas.
const ID_GALERIA_CAPTURAS: u32 = 904;
/// «Lecciones…» y «Nueva leccion…» (3-oct): las lecciones aprendidas, con
/// acceso facil desde fuera del chat, como el atajo del icono en el movil.
const ID_LECCIONES: u32 = 905;
const ID_LECCION_NUEVA: u32 = 906;
/// «Tareas…» (3-oct): las listas de tareas de todos los chats juntas.
const ID_TAREAS: u32 = 907;

const _: () = assert!(
    ID_ABRIR_DOCUMENTO >= pixpin_shell::ventana::ID_MENU_GRUPO_TOPE,
    "los identificadores de la bandeja caen dentro del tramo de los grupos ocultos"
);

/// Las entradas de la bandeja, en su orden, menos «Salir» (que va aparte).
///
/// Sale del catalogo de comandos, no de una lista escrita a mano: anadir
/// una funcion es anadir su fila. Debajo del chat van las lecciones, la
/// galeria de capturas y las tareas, que en el movil estan junto a
/// Proyectos donde estaba el sistema solar (3-oct-2026), y Sincronizar
/// detras: es donde van los chats a otros aparatos, y en el catalogo esta al final
/// solo para no correr los numeros de los demas.
fn acciones_de_bandeja(t: impl Fn(&str) -> String) -> Vec<(u32, String)> {
    use comandos::Comando;
    let mut v = Vec::new();
    // «Salir» sale aparte, al final y tras una raya: no debe pulsarse por
    // inercia al buscar otra cosa.
    for d in comandos::CATALOGO.iter().filter(|d| {
        d.en_bandeja && d.comando != Comando::Salir && d.comando != Comando::Sincronizar
    }) {
        v.push((d.comando.id(), t(d.clave_titulo)));
        if d.comando == Comando::AbrirChat {
            v.push((ID_LECCIONES, t("bandeja-lecciones")));
            v.push((ID_LECCION_NUEVA, t("bandeja-leccion-nueva")));
            v.push((ID_GALERIA_CAPTURAS, t("bandeja-galeria-capturas")));
            v.push((ID_TAREAS, t("bandeja-tareas")));
            let s = Comando::Sincronizar.descriptor();
            if s.en_bandeja {
                v.push((s.comando.id(), t(s.clave_titulo)));
            }
        }
    }
    // El editor avanzado (tarea 11): sin catalogo ni traduccion todavia, es
    // lo minimo para abrirlo desde la bandeja y probarlo a mano.
    v.push((ID_ABRIR_DOCUMENTO, t("bandeja-abrir-documento")));
    v.push((ID_GRUPOS_VENTANAS, t("bandeja-grupos-ventanas")));
    v.push((ID_VENTANA_EDITOR, "Editor".to_string()));
    v
}

fn main() -> Result<()> {
    // Con panic = "abort" y sin consola, un panico moria MUDO: ni log ni
    // dialogo (costo una sesion de depuracion a ciegas). El hook escribe al
    // registro antes del abort; tracing puede no estar inicializado aun, y
    // entonces simplemente no hace nada, que ya es lo que habia.
    std::panic::set_hook(Box::new(|info| {
        tracing::error!(%info, "panico fatal");
        // Darle al escritor no bloqueante un instante para volcar la linea
        // antes de que abort() se lleve el proceso por delante.
        std::thread::sleep(std::time::Duration::from_millis(300));
    }));

    // La caja de los textos del dibujo, medida con DirectWrite y la misma
    // letra con que se pintan (el motor es puro y sin esto mide a ojo).
    pixpin_motor2d::texto::instalar_medidor(dibujo::pintar::medir_para_el_motor);

    // Vive aqui, no dentro de `arrancar`, precisamente para que sobreviva a
    // un `Err`: ver el comentario de modulo de mas arriba.
    let mut guardia_registro: Option<tracing_appender::non_blocking::WorkerGuard> = None;

    if let Err(error) = arrancar(&mut guardia_registro) {
        // `guardia_registro` sigue vivo aqui (es local a `main`, y `arrancar`
        // solo tiene un prestamo), asi que si el registro a fichero ya se
        // pudo abrir esto queda escrito de verdad. Si el fallo ocurrio antes
        // de eso (p. ej. al comprobar la instancia unica, que es antes del
        // paso 3), `guardia_registro` sigue en `None` y `tracing::error!`
        // sin un subscriber inicializado simplemente no hace nada, no entra
        // en panico.
        tracing::error!(?error, "PixPin Max no pudo arrancar");
        pixpin_shell::mostrar_error_fatal("PixPin Max", &format!("{error:#}"));
        return Err(error);
    }
    Ok(())
}

fn arrancar(
    guardia_registro: &mut Option<tracing_appender::non_blocking::WorkerGuard>,
) -> Result<()> {
    // 1. Una sola copia a la vez.
    //
    // Los dos casos de error se tratan distinto a proposito. Que ya haya otra
    // instancia no es un fallo: el usuario pulso el icono dos veces. Que
    // `CreateMutexW` falle de verdad si lo es, y confundirlos manda a quien
    // depure a buscar una segunda copia que no existe.
    let _instancia = match adquirir_instancia_unica() {
        Ok(i) => i,
        Err(pixpin_shell::instancia::ErrorInstanciaUnica::YaHayOtraInstancia) => {
            // Si nos dieron ficheros («Abrir con», doble clic, arrastrar al
            // icono), se los pasamos a la copia que ya corre antes de
            // irnos. Sin esto, abrir una imagen con PixPin no haria NADA:
            // esta copia se iria en silencio llevandose la ruta, y el
            // usuario veria que su fichero no se abre.
            let rutas = pixpin_shell::mensajero::rutas_de_los_argumentos();
            if !rutas.is_empty() {
                pixpin_shell::mensajero::enviar_ficheros(&rutas);
            } else {
                // Sin ficheros, abrir PixPin otra vez es pedir SU ventana: el
                // chat, que es la pantalla principal. Antes esta copia se iba
                // en silencio y al usuario le parecia que el icono no hacia
                // nada.
                pixpin_shell::mensajero::pedir_ventana_principal(
                    pixpin_store::comandos::Comando::AbrirChat.id(),
                );
            }
            // Todavia no se han leido los ajustes, asi que no hay catalogo con
            // el que traducir un dialogo. Salir en silencio es lo correcto.
            return Ok(());
        }
        Err(e) => {
            return Err(anyhow::Error::new(e).context("no se pudo comprobar la instancia unica"));
        }
    };

    // 2. Donde vivimos.
    let dir_exe = entorno::directorio_del_ejecutable().context("no se pudo localizar el .exe")?;
    let appdata = entorno::appdata().context("no se pudo localizar APPDATA")?;
    let ubicacion = rutas::resolver(&dir_exe, &appdata);

    // 3. Registro a fichero, antes de leer los ajustes: si el TOML esta mal
    // escrito, el fallo de mas abajo queda documentado en vez de perderse.
    // Se guarda en el `Option` que paso `main`, no en una variable local de
    // esta funcion, para que siga vivo aunque `arrancar` devuelva `Err` mas
    // abajo (ver el comentario de modulo).
    *guardia_registro = Some(iniciar_registro(&ubicacion));
    tracing::info!(
        portable = ubicacion.es_portable(),
        raiz = ?ubicacion.raiz(),
        "PixPin Max arrancando"
    );
    // La identidad de este equipo, en el formato de PixPin Android
    // (`sincro/identidad.json`): de ella sale el codigo de aparato que llevan
    // los tres codigos de todo lo que nazca aqui. Sin ella la app funciona;
    // solo se registra el fallo.
    let nombre_equipo = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PixPin Max".into());
    // El codigo de este equipo: lo lleva cada cosa que nace aqui, y con el
    // se reconoce lo que vuelve del movil.
    let identidad_equipo = match pixpin_proyecto::identidad::Identidad::leer_o_crear(
        ubicacion.raiz(),
        &nombre_equipo,
    ) {
        Ok(i) => {
            tracing::info!(aparato = %i.yo.codigo(), nombre = %i.yo.nombre, "identidad del equipo");
            i.yo.codigo()
        }
        Err(e) => {
            tracing::warn!(?e, "no se pudo leer ni crear la identidad del equipo");
            String::new()
        }
    };

    // 4. Que nos han configurado.
    let mut config = ajustes::cargar(&ubicacion).context("no se pudieron leer los ajustes")?;

    // Las capturas se van solas a los `[capturas] dias_caducidad` dias salvo
    // las conservadas: se barre ahora y cada hora, en su hilo
    // (`caducidad_capturas`).
    caducidad_capturas::fijar_dias(config.capturas.dias_caducidad);
    caducidad_capturas::vigilar(ubicacion.raiz().to_path_buf());
    // Como se siente el lapiz (`[tinta]`). Va por aqui y no como parametro de
    // `ventana_editor::abrir` porque el editor se abre desde cinco sitios
    // distintos (la bandeja, el «abrir con», el chat, las lecciones) y ninguno
    // de ellos tiene por que enterarse de un ajuste que solo mira el bucle de
    // dibujo. Se fija una vez, antes de que exista el primer editor, y no
    // vuelve a cambiar mientras el programa viva.
    ventana_editor::fijar_ajustes_tinta(config.tinta);
    // M1: el tema Cosmos lo mira el hilo del chat al abrirse.
    tema_cosmos::fijar(config.tema_cosmos);
    // Las herramientas de dibujo que el usuario apago en los ajustes: fuera
    // de la barra y de sus teclas en el lienzo, el lector, los pines y el
    // anotador de pantalla (`dibujo::permitidas`).
    dibujo::permitidas::fijar(config.herramientas.clone());
    // Aligerar los PDF que entran al chat: el nivel lo lee el trabajador de
    // `aligerar` cada vez, asi que cambiarlo en Ajustes vale sin reiniciar.
    aligerar::configurar(&config.pdf);

    // 5. Reflejar en el registro de Windows lo que digan los ajustes.
    //
    // Se aplica en cada arranque y no solo al cambiar la casilla porque el
    // usuario puede haber editado el TOML a mano, o haber copiado su fichero
    // de ajustes a otro equipo. Asi el estado real y el declarado no divergen.
    let ruta_exe = dir_exe.join("pixpinmax.exe");
    // Salir en «Abrir con» para imagenes y videos. Se hace en cada
    // arranque porque es idempotente y porque el ejecutable puede haberse
    // movido: en modo portable la carpeta entera cambia de sitio, y una
    // orden que apunta a donde ya no esta el .exe es peor que no salir en
    // la lista.
    //
    // Solo se OFRECE, no se queda con las extensiones: eso ultimo es de lo
    // que mas molesta de un programa, y ademas Windows lo deshace y avisa.
    // Y como app de imagenes y videos en Configuracion → Aplicaciones
    // predeterminadas (`asociaciones`): ahi el usuario la hace la de siempre
    // con un boton, y todo se abre como pin.
    if config.abrir_con {
        if let Err(e) = pixpin_shell::asociaciones::registrar(&ruta_exe) {
            tracing::warn!(?e, "no se pudo registrar PixPin para imagenes y videos");
        }
    } else {
        pixpin_shell::asociaciones::quitar();
    }
    let inscripcion = if config.abrir_con {
        pixpin_shell::abrir_con::inscribir(&ruta_exe)
    } else {
        // Apagarlo BORRA lo escrito, no solo deja de escribir: si no, el
        // rastro se quedaria ahi para siempre y el interruptor no serviria
        // de nada a quien va en modo portable.
        pixpin_shell::abrir_con::desinscribir(&ruta_exe)
    };
    if let Err(e) = inscripcion {
        tracing::warn!(
            ?e,
            activo = config.abrir_con,
            "no se pudo tocar «Abrir con»"
        );
    }
    match arranque::establecer(
        config.arranque_con_windows,
        ubicacion.es_portable(),
        &ruta_exe,
    ) {
        Ok(()) => {}
        Err(arranque::ErrorArranque::ModoPortable) => {
            // No es un fallo: es la regla del modo portable funcionando. Se
            // deja constancia para que nadie piense que la casilla esta rota.
            tracing::info!(
                "arranque con Windows ignorado: en modo portable no se toca el registro"
            );
        }
        Err(e) => tracing::warn!(?e, "no se pudo aplicar el arranque con Windows"),
    }

    // 5b. Nivel de rendimiento (D13-D19): se decide UNA vez, se registra con
    // sus razones y viaja por parametro. Sin globals. En S1-B1 la captura no
    // cambia con el nivel —los bytes son sagrados y los efectos no existen
    // aun—, pero la decision y su registro son lo que permitira diagnosticar
    // un "me va lento" sin adivinar. El primer consumidor real del
    // presupuesto sera el overlay de S1-B2.
    let hechos = pixpin_shell::hechos::recolectar();
    let preferencia = match config.rendimiento.nivel {
        PreferenciaNivel::Auto => Preferencia::Auto,
        PreferenciaNivel::Completo => Preferencia::Forzado(Nivel::Completo),
        PreferenciaNivel::Ligero => Preferencia::Forzado(Nivel::Ligero),
    };
    let decision = pixpin_nivel::decidir(&hechos, preferencia);
    // El ritmo del temporizador de los pines de video (D67): sin tope real
    // en Completo (16 ms ~ 60 Hz), 30 fps en Ligero.
    let ritmo_video: u32 = match decision.nivel {
        Nivel::Completo => 16,
        Nivel::Ligero => 33,
    };
    tracing::info!(?hechos, ?decision, "nivel de rendimiento decidido");
    // Aligerar PDF en modo cuidadoso (un hilo, esperar a la memoria) en los
    // equipos de 4 GB o en nivel Ligero: ver `aligerar::modo_cuidadoso`.
    aligerar::fijar_equipo(hechos.ram_fisica_bytes, decision.nivel == Nivel::Ligero);
    // Whisper, con la misma regla: sin arena y de uno en uno en modo cuidadoso.
    turno_pesado::fijar_equipo(hechos.ram_fisica_bytes, decision.nivel == Nivel::Ligero);

    // 6. Idioma, antes de crear nada con texto.
    let lengua = idioma::resolver_idioma(&entorno::locale_del_sistema(), config.idioma);
    let textos = Catalogo::nuevo(lengua);
    // El panel de propiedades del lienzo vive en el hilo del editor y no
    // recibe el catalogo: se le dice aqui el idioma elegido, o usaria el de
    // Windows aunque el usuario haya puesto otro en los ajustes.
    panel_dibujo::fijar_idioma(lengua);
    // Lo mismo para lo que pintan las herramientas pineadas (C3).
    pines::fijar_idioma(lengua);

    // 7. Ventana invisible, icono de bandeja y atajos.
    let ventana = VentanaMensajes::nueva().context("no se pudo crear la ventana de mensajes")?;
    let mut bandeja = Bandeja::nueva(ventana.handle(), &textos.t("app-nombre"))
        .context("no se pudo añadir el icono de bandeja")?;

    // Los atajos salen del registro de comandos: la tabla `[comandos]` del
    // TOML manda, y por debajo se sigue leyendo la tabla vieja `[atajos]`
    // para no romper el fichero de nadie. Tres funciones nacen sin atajo
    // (D81): la region con barra, el cuentagotas y la anotacion congelada.
    let (enlaces, avisos) = comandos::Enlaces::de_ajustes(&config);
    for aviso in &avisos {
        tracing::warn!(%aviso, "entrada de [comandos] que no se entiende; se ignora");
    }
    if let Some((a, b)) = enlaces.choque() {
        // Dos comandos con el mismo atajo: el segundo no llegaria a
        // dispararse nunca, y sin aviso parece que el programa lo ignora.
        tracing::warn!(?a, ?b, "dos comandos comparten atajo; solo actuara uno");
    }
    let mut peticiones = enlaces.registrables();
    // Y las regiones guardadas (P2.3), con sus identificadores propios
    // muy por encima de los de los comandos para que no se pisen.
    let (de_regiones, avisos_regiones) = pixpin_store::regiones::registrables(&config.regiones);
    for aviso in &avisos_regiones {
        tracing::warn!(%aviso, "region guardada que no se puede usar");
    }
    peticiones.extend(de_regiones);
    // En un `Option` porque silenciar los atajos es soltar el guardia:
    // `UnregisterHotKey` es lo unico que devuelve de verdad la
    // combinacion al sistema, para que el programa de delante la reciba.
    // Desviarlos a una bandera dejaria el atajo tomado y el otro programa
    // seguiria sin verlo.
    let (registrados, fallidos) = atajos::registrar(ventana.handle(), &peticiones);
    let mut registrados = Some(registrados);
    tracing::info!(
        pedidos = peticiones.len(),
        fallidos = fallidos.len(),
        "atajos globales registrados"
    );

    // Los gestos con Alt (D81): Alt + izquierdo copia, Alt + derecho pinea.
    let gancho = match pixpin_shell::GanchoRaton::instalar(ventana.handle()) {
        Ok(g) => Some(g),
        Err(e) => {
            tracing::warn!(?e, "sin gancho de raton: los gestos con Alt no funcionaran");
            None
        }
    };
    // Ctrl+V de una imagen en Flow Launcher la pega como `[img 01]` en la
    // tarea que se esta escribiendo (el plugin no ve las teclas).
    let _pegar_en_flow = pegar_en_flow::instalar(ubicacion.raiz().to_path_buf());
    for (id, atajo) in &fallidos {
        // Se registra el problema pero no se aborta: otra aplicacion puede
        // tener ese atajo y el resto de PixPin Max sigue siendo util.
        tracing::warn!(id, %atajo, "no se pudo registrar el atajo; otra aplicacion lo tiene");
    }

    // El menu de la bandeja sale del catalogo de comandos, no de una lista
    // escrita a mano: anadir una funcion es anadir su fila.
    let etiquetas_base = |ocultos: Vec<(u32, String)>| {
        let entrada = |d: &comandos::Descriptor| (d.comando.id(), textos.t(d.clave_titulo));
        EtiquetasMenu {
            acciones: acciones_de_bandeja(|clave| textos.t(clave)),
            aparte: comandos::CATALOGO
                .iter()
                .find(|d| d.en_bandeja && d.comando == comandos::Comando::Salir)
                .map(entrada),
            grupos_ocultos: textos.t("grupos-ocultos"),
            ocultos,
        }
    };

    // 7b. Precalentamiento diferido (5.3 del diseno de rendimiento): cargar
    // los DLL del driver es la parte cara del primer atajo del dia; se paga
    // ya, en un hilo aparte, creando y soltando un dispositivo. No se
    // retiene: compartir un dispositivo D3D11 entre hilos pide una
    // disciplina que traera el overlay de S1-B2, y el beneficio de hoy esta
    // en calentar el driver, no en guardar el objeto. Sin SetThreadPriority:
    // este ejecutable es forbid(unsafe_code) y bajar la prioridad de un
    // trabajo de ~150 ms no justifica abrir un agujero en pixpin-shell.
    // 7c. La presencia de sincronizar: este equipo se deja encontrar por el
    // grupo mientras PixPin viva, como `Presencia` en el movil, y no solo
    // mientras esta abierta la ventana de Sincronizar. Antes, al cerrarla el
    // anuncio mDNS se apagaba y al volver a abrirla tardaba uno o dos minutos
    // en llegar al movil; eso es lo que el usuario veia como «tengo que
    // esperar para que me aparezca». Se puede apagar con `[sincro] presencia
    // = false`.
    sincronizar::presencia::instalar(ubicacion.raiz().to_path_buf(), config.sincro.presencia);
    // 7d. El marco de la tinta de lo ya anotado (K21): una vez por almacen,
    // en un hilo aparte; no toca ninguna tinta.
    marco_de_la_tinta::al_arrancar(ubicacion.raiz().to_path_buf());

    std::thread::spawn(|| match pixpin_capture::Dispositivo::nuevo() {
        Ok(_) => tracing::debug!("dispositivo D3D11 precalentado"),
        Err(e) => {
            tracing::debug!(
                ?e,
                "precalentamiento fallido; el primer atajo pagara el camino lento"
            );
        }
    });

    // 8. A dormir hasta que pase algo. Los recursos caros del overlay
    // (dispositivo, motor, duplicadores) se crean en el primer atajo y
    // viven entre capturas: son la diferencia entre 200 ms y menos de 50.
    let mut recursos_overlay: Option<Recursos> = None;
    let mut pines: Option<Pines> = None;
    // La ultima region que el usuario confirmo, para poder repetirla sin
    // volver a dibujarla. Se pierde al cerrar: es una comodidad de la
    // sesion, no un ajuste que merezca ir al disco.
    let mut ultima_region: Option<pixpin_geom::Rect> = None;
    let hwnd = ventana.handle();
    // Si la GPU se pierde (driver, TDR, suspension), el primer fallo
    // despierta este bucle, que lo rehace todo (`dispositivo_perdido`).
    dispositivo_perdido::instalar_aviso(hwnd);
    // La pila de capturas: toda captura que va al portapapeles pasa por
    // ella. Nace sin ventana y sin temporizador; con `apilar_segundos = 0`
    // se limita a copiar, como antes de existir.
    let mut pila = pila_capturas::PilaCapturas::nueva(&config.capturas, hwnd);

    // 8b. Restauracion al arrancar (spec S2 5.2): el coste de crear los
    // recursos solo se paga si el almacen tiene pines abiertos, y la
    // bandeja ya esta visible, asi que el presupuesto de arranque no se
    // toca. Un almacen ilegible no impide arrancar: se registra y se sigue.
    match Almacen::abrir(ubicacion.raiz()) {
        Ok(a) if a.entradas().iter().any(|e| e.pin.is_some()) => {
            drop(a); // Pines::nuevos abre el suyo; no dos indices vivos.
            let t = std::time::Instant::now();
            let restaurado = preparar_pines(
                &mut recursos_overlay,
                &mut pines,
                &ubicacion,
                &textos,
                hwnd,
                ritmo_video,
            )
            .and_then(|p| {
                let d =
                    pixpin_capture::enumerar_monitores().context("sin monitores para restaurar")?;
                Ok(p.restaurar(&d))
            });
            match restaurado {
                Ok(restaurados) => tracing::info!(
                    restaurados,
                    ms = t.elapsed().as_millis() as u64,
                    "pines restaurados al arrancar"
                ),
                Err(e) => tracing::warn!(?e, "no se pudieron restaurar los pines"),
            }
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(?e, "no se pudo abrir el almacen al arrancar"),
    }

    // 8c. Los ficheros de la linea de mandatos, si esta es la PRIMERA copia
    // («Abrir con» sin PixPin corriendo).
    //
    // Se mandan por el mismo camino que usa una segunda copia: un
    // WM_COPYDATA a nuestra propia ventana. Reusar la via en vez de
    // duplicar el pineado aqui es lo que garantiza que abrir un fichero se
    // comporte IGUAL este PixPin ya abierto o no; con dos caminos, uno de
    // los dos se queda atras en cuanto se cambie algo.
    let del_arranque = pixpin_shell::mensajero::rutas_de_los_argumentos();
    if !del_arranque.is_empty() {
        tracing::info!(
            cuantos = del_arranque.len(),
            "ficheros en la linea de mandatos"
        );
        pixpin_shell::mensajero::enviar_ficheros(&del_arranque);
        // Y un toque a la cola. `enviar_ficheros` usa SendMessage, que entra
        // DIRECTO al procedimiento de ventana sin pasar por la cola: el
        // evento queda apuntado, pero el bucle todavia no ha empezado y al
        // empezar se duerme en GetMessage esperando algo que ya paso. Sin
        // esto, el fichero solo se abria cuando el usuario tocaba cualquier
        // otra cosa.
        pixpin_shell::despertar(ventana.handle());
    }

    // El chat de proyectos es la interfaz principal (D141): se abre al
    // arrancar, como la ventana de cualquier aplicacion. Cerrarlo deja
    // PixPin en la bandeja, con los gestos y el atajo vivos.
    let opciones_lienzo = ventana_chat::OpcionesLienzo {
        enganche: config.enganche,
        nivel: decision.nivel,
        medir_fotogramas: config.rendimiento.medir_fotogramas,
    };
    // Las hojas que se abren desde una nota (paginas vivas, enlaces), con
    // los mismos ajustes del lienzo.
    notas_md::paginas_vivas::poner_opciones_del_lienzo(opciones_lienzo);
    ventana_chat::lanzar(lengua, ubicacion.clone(), opciones_lienzo);

    // 8e. Los recordatorios. Su hilo duerme hasta que vence el proximo y solo
    // da un toque a ESTA ventana: el pin y el globo los saca el bucle de
    // abajo, que es quien tiene la bandeja y los pines. Se arranca despues
    // del chat porque lo que vencio con el programa cerrado sale de golpe, y
    // asi el primer pin nace con la ventana ya en pie.
    recordatorios::vigilar(ubicacion.raiz(), hwnd.0 as isize);

    // Lo que ensenan los interruptores del panel de la bandeja (v2): lo
    // que dijeron los dos ultimos «alternar» de los pines.
    let mut pines_ocultos = false;
    let mut pines_pasantes = false;

    ventana.ejecutar(|evento| {
        // Antes de nada: si la GPU se perdio, todo lo de abajo (capturar,
        // pinear) fallaria sobre el dispositivo muerto. Mirarlo es una
        // consulta barata; rehacerlo, solo cuando de verdad se perdio.
        dispositivo_perdido::rehacer_si_se_perdio(
            &mut recursos_overlay,
            &mut pines,
            &mut pila,
            &textos,
        );
        // Todo lo que abre el overlay de captura, en un sitio: los atajos,
        // «Capturar» de la bandeja y los gestos con Alt (D81). El gesto
        // trae el punto donde ya esta pulsado el boton: el overlay arranca
        // con el arrastre en marcha desde ahi.
        // Un atajo y una entrada del menu son la misma cosa vista por dos
        // vias: las dos traen el identificador del comando.
        let comando = match evento {
            Evento::Atajo(id) | Evento::Menu(id) => comandos::Comando::desde_id(id),
            _ => None,
        };
        // Una region guardada, si el identificador cae en su espacio.
        let region_guardada = match evento {
            Evento::Atajo(id) | Evento::Menu(id) => pixpin_store::regiones::desde_id(id)
                .and_then(|i| config.regiones.get(i))
                .filter(|r| r.es_util()),
            _ => None,
        };
        // La lista de programas a ignorar (P1.8). Solo frena los ATAJOS y
        // los gestos: lo que se elige a mano en el menu de la bandeja se
        // hace siempre, porque ahi el usuario ya esta mirando a PixPin y
        // no puede querer decir otra cosa.
        if !config.ignorar_programas.is_empty()
            && !matches!(evento, Evento::Menu(_))
            && let Some(programa) = pixpin_shell::primer_plano::programa_delante()
            && pixpin_shell::primer_plano::esta_en_la_lista(&programa, &config.ignorar_programas)
        {
            tracing::debug!(%programa, "atajo ignorado: el programa esta en la lista");
            return Continuar::Si;
        }
        if comando == Some(comandos::Comando::SilenciarAtajos) {
            match registrados.take() {
                Some(guardia) => {
                    // Soltarlo es lo que los desregistra: el `Drop` del
                    // guardia llama a UnregisterHotKey uno por uno.
                    drop(guardia);
                    let _ = bandeja.poner_titulo(&textos.t("bandeja-silenciada"));
                    let _ = bandeja.avisar(
                        &textos.t("aviso-atajos-silenciados"),
                        &textos.t("aviso-atajos-silenciados-detalle"),
                    );
                    tracing::info!("atajos globales silenciados");
                }
                None => {
                    let (guardia, fallidos) = atajos::registrar(ventana.handle(), &peticiones);
                    let cuantos = peticiones.len() - fallidos.len();
                    registrados = Some(guardia);
                    let _ = bandeja.poner_titulo(&textos.t("app-nombre"));
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("cuantos", cuantos.to_string());
                    let _ = bandeja.avisar(
                        &textos.t("aviso-atajos-activos"),
                        &textos.t_args("aviso-atajos-activos-detalle", &args),
                    );
                    tracing::info!(cuantos, "atajos globales devueltos");
                }
            }
            return Continuar::Si;
        }
        let modo_overlay: Option<(ModoConfirmacion, Option<pixpin_geom::Punto>)> = match evento {
            Evento::Gesto {
                boton: BotonGesto::Izquierdo,
                punto,
            } => Some((ModoConfirmacion::DirectoAlPortapapeles, Some(punto))),
            Evento::Gesto {
                boton: BotonGesto::Derecho,
                punto,
            } => Some((ModoConfirmacion::Pinear, Some(punto))),
            Evento::Gesto {
                boton: BotonGesto::Central,
                punto,
            } => Some((ModoConfirmacion::PinEnVivo, Some(punto))),
            _ => match comando {
                Some(comandos::Comando::CapturarRegion) => Some((ModoConfirmacion::ConBarra, None)),
                // El retardo abre la MISMA captura; lo unico distinto es
                // que antes se cuenta. La cuenta corre mas abajo, ya con
                // los recursos a mano.
                Some(comandos::Comando::CapturarConRetardo) => {
                    Some((ModoConfirmacion::ConBarra, None))
                }
                Some(comandos::Comando::Pinear) => Some((ModoConfirmacion::Pinear, None)),
                Some(comandos::Comando::PinearEnVivo) => Some((ModoConfirmacion::PinEnVivo, None)),
                Some(comandos::Comando::CapturarConScroll) => {
                    Some((ModoConfirmacion::Scroll, None))
                }
                Some(comandos::Comando::Cuentagotas) => Some((ModoConfirmacion::Cuentagotas, None)),
                Some(comandos::Comando::CopiarTexto) => Some((ModoConfirmacion::Texto, None)),
                Some(comandos::Comando::GrabarGif) => Some((ModoConfirmacion::Gif, None)),
                // Capturar y anotar abre la misma captura que pinear; lo
                // que cambia es lo que pasa despues, ya con el pin hecho.
                Some(comandos::Comando::CapturarYAnotar) => Some((ModoConfirmacion::Pinear, None)),
                Some(comandos::Comando::CapturarYCopiar) => {
                    Some((ModoConfirmacion::DirectoAlPortapapeles, None))
                }
                _ => None,
            },
        };
        let seguir = match evento {
            _ if comando == Some(comandos::Comando::Salir) => {
                tracing::info!("salida pedida por el usuario");
                // El PDF que se estuviera aligerando se deja como estaba.
                aligerar::cancelar_todo();
                Continuar::No
            }
            // Los comandos de pines: los tres necesitan la disposicion de
            // monitores para devolver cada uno a su sitio.
            _ if matches!(
                comando,
                Some(
                    comandos::Comando::AlternarPasoDeClics
                        | comandos::Comando::AlternarPines
                        | comandos::Comando::RestaurarUltimoPin
                        | comandos::Comando::CerrarTodosLosPines
                )
            ) =>
            {
                match pines.as_mut() {
                    None => tracing::info!("todavia no hay ningun pin"),
                    Some(p) => match comando {
                        Some(comandos::Comando::CerrarTodosLosPines) => {
                            tracing::info!(cuantos = p.cerrar_todos(), "pines cerrados");
                        }
                        Some(comandos::Comando::AlternarPasoDeClics) => {
                            let (pasantes, cuantos) = p.alternar_paso_de_clics();
                            pines_pasantes = pasantes;
                            tracing::info!(pasantes, cuantos, "paso de clics de los pines");
                        }
                        _ => match pixpin_capture::enumerar_monitores() {
                            Err(e) => tracing::warn!(?e, "sin monitores"),
                            Ok(d) if comando == Some(comandos::Comando::AlternarPines) => {
                                let (ocultados, cuantos) = p.alternar_todos(&d);
                                pines_ocultos = ocultados;
                                tracing::info!(ocultados, cuantos, "pines ocultados o mostrados");
                            }
                            Ok(d) => {
                                let hecho = p.restaurar_ultimo_cerrado(&d);
                                tracing::info!(hecho, "devolver el ultimo pin cerrado");
                            }
                        },
                    },
                }
                Continuar::Si
            }
            // Repetir el ultimo recorte sin overlay ni preguntas: para
            // capturar la misma zona una y otra vez, que es lo que se hace
            // al seguir un proceso que va cambiando en el mismo sitio.
            _ if comando == Some(comandos::Comando::CapturarUltimaRegion) => {
                match ultima_region {
                    None => tracing::info!("todavia no hay ninguna region que repetir"),
                    Some(region) => {
                        let hecho = (match &mut recursos_overlay {
                            Some(r) => Ok(r),
                            nada => Recursos::nuevos().map(|r| nada.insert(r)),
                        })
                        .and_then(|r| {
                            let d = pixpin_capture::enumerar_monitores()?;
                            let m = d
                                .monitores()
                                .iter()
                                .find(|m| m.area.interseccion(region).is_some())
                                .or_else(|| d.principal())
                                .context("sin monitor para la region")?
                                .to_owned();
                            let imagen = scroll::capturar(r, &m, region)?;
                            pila.entrar(imagen, region, r, &ubicacion, &textos)
                        });
                        match hecho {
                            Ok(()) => tracing::info!(?region, "ultima region repetida y copiada"),
                            Err(e) => tracing::warn!(?e, "no se pudo repetir la region"),
                        }
                    }
                }
                Continuar::Si
            }
            // Una region guardada captura y copia directamente, sin
            // overlay: la zona ya esta decidida, y volver a preguntarla
            // seria justo lo que esta funcion viene a ahorrar.
            _ if region_guardada.is_some() => {
                let r = region_guardada.expect("comprobado en la guarda");
                let region = pixpin_geom::Rect {
                    x: r.x,
                    y: r.y,
                    ancho: r.ancho,
                    alto: r.alto,
                };
                let hecho = (match &mut recursos_overlay {
                    Some(rec) => Ok(rec),
                    nada => Recursos::nuevos().map(|rec| nada.insert(rec)),
                })
                .and_then(|rec| {
                    let d = pixpin_capture::enumerar_monitores()?;
                    let m = d
                        .monitores()
                        .iter()
                        .find(|m| m.area.interseccion(region).is_some())
                        .or_else(|| d.principal())
                        .context("la region guardada no cae en ningun monitor")?
                        .to_owned();
                    let imagen = scroll::capturar(rec, &m, region)?;
                    pila.entrar(imagen, region, rec, &ubicacion, &textos)
                });
                match hecho {
                    Ok(()) => {
                        tracing::info!(nombre = %r.nombre, ?region, "region guardada copiada")
                    }
                    Err(e) => {
                        tracing::warn!(?e, nombre = %r.nombre, "no se pudo capturar la region")
                    }
                }
                Continuar::Si
            }
            _ if comando == Some(comandos::Comando::VentanaEncima) => {
                match pixpin_shell::alternar_ventana_bajo_el_cursor() {
                    pixpin_shell::Fijada::Cambiada { encima, titulo } => {
                        tracing::info!(encima, %titulo, "ventana fijada encima o bajada")
                    }
                    pixpin_shell::Fijada::SinVentana => {
                        tracing::info!("bajo el cursor no hay ventana que fijar")
                    }
                }
                Continuar::Si
            }
            _ if comando == Some(comandos::Comando::RecibirDelMovil) => {
                // En su propio hilo, como el chat: esperar al movil no puede
                // dejar sordos los atajos ni los gestos.
                recibir::lanzar(lengua, ubicacion.clone());
                Continuar::Si
            }
            _ if comando == Some(comandos::Comando::Buscar) => {
                // En su propio hilo, como la galeria; si ya esta abierto
                // lo trae delante (lo vigila `buscar_todo::abrir`).
                buscar_todo::abrir(
                    lengua,
                    ubicacion.clone(),
                    buscar_todo::Opciones::de(&config, hwnd.0 as isize),
                );
                Continuar::Si
            }
            _ if comando == Some(comandos::Comando::Sincronizar) => {
                // Su propio hilo, como el chat; si ya esta abierta no abre
                // otra (lo vigila `sincronizar::lanzar`).
                sincronizar::lanzar(lengua, ubicacion.clone());
                Continuar::Si
            }
            _ if comando == Some(comandos::Comando::AbrirChat) => {
                // En su propio hilo (D141): el principal sigue atendiendo
                // atajos y gestos mientras el chat esta abierto.
                ventana_chat::lanzar(lengua, ubicacion.clone(), opciones_lienzo);
                Continuar::Si
            }
            _ if comando == Some(comandos::Comando::AbrirAjustes) => {
                let recursos = match &mut recursos_overlay {
                    Some(r) => Ok(&*r),
                    nada => Recursos::nuevos().map(|r| &*nada.insert(r)),
                };
                let abierta =
                    recursos.and_then(|r| ventana_ajustes::abrir(r, &config, &textos, &ubicacion));
                match abierta {
                    Err(e) => tracing::warn!(?e, "no se pudo abrir la ventana de ajustes"),
                    Ok(None) => tracing::info!("ajustes cerrados sin cambios"),
                    Ok(Some(nuevos)) => {
                        // Los atajos se vuelven a registrar EN VIVO: es lo
                        // que evita el «reinicia para aplicar», y lo que
                        // hace que grabar un atajo en la ventana valga de
                        // algo al salir de ella. Lo que no se puede aplicar
                        // sin reiniciar (el idioma, el nivel de rendimiento)
                        // queda guardado y entra en el siguiente arranque.
                        config = nuevos;
                        // M1: vale desde el siguiente chat que se abra.
                        tema_cosmos::fijar(config.tema_cosmos);
                        aligerar::configurar(&config.pdf);
                        // Las herramientas apagadas valen desde el siguiente
                        // lienzo, lector, pin o anotador que se abra.
                        dibujo::permitidas::fijar(config.herramientas.clone());
                        let (enlaces, _) = comandos::Enlaces::de_ajustes(&config);
                        peticiones = enlaces.registrables();
                        let (de_regiones, _) =
                            pixpin_store::regiones::registrables(&config.regiones);
                        peticiones.extend(de_regiones);
                        // Soltar el guardia viejo ANTES de registrar el nuevo:
                        // si no, las combinaciones que no cambiaron seguirian
                        // tomadas y el registro nuevo fallaria en ellas.
                        drop(registrados.take());
                        let (guardia, fallidos) = atajos::registrar(ventana.handle(), &peticiones);
                        registrados = Some(guardia);
                        tracing::info!(
                            pedidos = peticiones.len(),
                            fallidos = fallidos.len(),
                            "ajustes aplicados y atajos registrados de nuevo"
                        );
                    }
                }
                Continuar::Si
            }
            Evento::AbrirFicheros(rutas) => {
                abrir_ficheros(
                    rutas,
                    lengua,
                    &ubicacion,
                    &identidad_equipo,
                    &mut recursos_overlay,
                    &mut pines,
                    &textos,
                    hwnd,
                    ritmo_video,
                );
                Continuar::Si
            }
            // Un pedido de otro programa (`docs/protocolo-pedidos.md`): la
            // ventana ya le contesto; aqui se hace. Lo que no sale se dice con
            // el globo, que es lo unico que el usuario ve cuando lo pidio desde
            // un lanzador con PixPin escondida en la bandeja.
            Evento::Pedido(json) => {
                let hecho = pedidos::atender(
                    &json,
                    &pedidos::Contexto {
                        idioma: lengua,
                        ubicacion: &ubicacion,
                        opciones: opciones_lienzo,
                        aparato: &identidad_equipo,
                        textos: &textos,
                    },
                );
                if let Some(aviso) = hecho.aviso {
                    let _ = bandeja.avisar(&textos.t("app-nombre"), &aviso);
                }
                if !hecho.ficheros.is_empty() {
                    abrir_ficheros(
                        hecho.ficheros,
                        lengua,
                        &ubicacion,
                        &identidad_equipo,
                        &mut recursos_overlay,
                        &mut pines,
                        &textos,
                        hwnd,
                        ritmo_video,
                    );
                }
                Continuar::Si
            }
            Evento::Menu(id) if id == ID_ABRIR_DOCUMENTO => {
                // El visor recibe una ruta y no sabe de donde sale; desde la
                // bandeja la elige el usuario. Lo que no sepa abrir se salta:
                // mas vale no abrir nada que abrir una ventana en blanco.
                for ruta in pixpin_shell::elegir::pedir_ficheros(windows::Win32::Foundation::HWND(
                    std::ptr::null_mut(),
                ))
                .iter()
                {
                    // Word, libro, pagina o PDF: cada uno a su lector.
                    lector::abrir_en_su_lector(
                        lengua,
                        &ubicacion,
                        ruta,
                        &pixpin_docs::nombre(ruta),
                    );
                }
                Continuar::Si
            }
            Evento::Menu(id) if id == ID_LECCIONES => {
                lecciones::lista(ubicacion.clone(), lengua, &identidad_equipo, None, None);
                Continuar::Si
            }
            Evento::Menu(id) if id == ID_LECCION_NUEVA => {
                lecciones::nueva(
                    ubicacion.clone(),
                    lengua,
                    &identidad_equipo,
                    None,
                    None,
                    None,
                );
                Continuar::Si
            }
            Evento::Menu(id) if id == ID_GALERIA_CAPTURAS => {
                galeria_capturas::abrir(lengua, ubicacion.clone());
                Continuar::Si
            }
            Evento::Menu(id) if id == ID_TAREAS => {
                tareas::abrir(lengua, ubicacion.clone(), &identidad_equipo);
                Continuar::Si
            }
            Evento::Menu(id) if id == ID_GRUPOS_VENTANAS => {
                let lanzador = grupos_ventanas::Lanzador {
                    idioma: lengua,
                    ubicacion: ubicacion.clone(),
                    opciones: opciones_lienzo,
                };
                if let Some(aviso) = grupos_ventanas::menu(None, &lanzador, &textos) {
                    let _ = bandeja.avisar(&textos.t("app-nombre"), &aviso);
                }
                Continuar::Si
            }
            Evento::Menu(id) if id == ID_VENTANA_EDITOR => {
                match ventana_editor::abrir(
                    pixpin_motor2d::Escena::nueva(),
                    config.enganche,
                    decision.nivel,
                    config.rendimiento.medir_fotogramas,
                    // D137: la bandeja abre el editor en blanco, sin pin.
                    None,
                    &[],
                ) {
                    Ok((escena, _)) => {
                        tracing::info!(elementos = escena.cuantos_visibles(), "editor cerrado")
                    }
                    Err(e) => tracing::warn!(?e, "no se pudo abrir el editor"),
                }
                Continuar::Si
            }
            // Clic izquierdo: el panel de la bandeja (v2). Lo que se pulse en
            // el vuelve aqui como `Evento::Menu`, con los numeros del menu.
            Evento::PanelBandeja => {
                centro_bandeja::abrir(lengua, &ubicacion, &config);
                Continuar::Si
            }
            Evento::IconoPulsado => {
                // La lista de grupos ocultos se monta AL ABRIR el menu, no
                // al arrancar: si no, ocultar un grupo no aparecería hasta
                // reiniciar.
                let ocultos = pines
                    .as_ref()
                    .map(|p| p.grupos_ocultos(&textos))
                    .unwrap_or_default();
                if let Err(e) = bandeja.mostrar_menu(hwnd, &etiquetas_base(ocultos)) {
                    tracing::warn!(?e, "no se pudo mostrar el menu de bandeja");
                }
                Continuar::Si
            }
            _ if modo_overlay.is_some() => {
                let (modo, inicio) = modo_overlay.expect("comprobado en la guarda");
                // Cuando lo pidio la mano: la pulsacion del gesto (la vio el
                // gancho) o el mensaje del atajo. El overlay mide desde aqui.
                let mut desde_ms = if inicio.is_some() {
                    pixpin_shell::gestos::hora_de_la_pulsacion()
                } else {
                    pixpin_shell::gestos::hora_del_mensaje()
                };
                // La cuenta atras va AQUI y no antes de decidir el modo:
                // hace falta tener los recursos de dibujo montados para
                // ensenar el cartel, y montarlos es lo primero que hace
                // esta rama de todas formas.
                if comando == Some(comandos::Comando::CapturarConRetardo) {
                    let recursos = match &mut recursos_overlay {
                        Some(r) => Ok(&*r),
                        nada => Recursos::nuevos().map(|r| &*nada.insert(r)),
                    };
                    match recursos {
                        Err(e) => tracing::warn!(?e, "sin recursos para la cuenta atras"),
                        Ok(r) => {
                            if !cuenta_atras::esperar(r, config.retardo_captura_s, &textos) {
                                return Continuar::Si;
                            }
                        }
                    }
                    desde_ms = pixpin_shell::gestos::reloj_ms();
                }
                let anotar_al_pinear = comando == Some(comandos::Comando::CapturarYAnotar);
                tracing::info!(?modo, ?inicio, anotar_al_pinear, "abrir captura");
                let etiquetas_barra = TextosBarra {
                    todo: textos.t("barra-todo"),
                };
                // La captura v2: rotulos, la ultima zona para «Repetir» y
                // donde estan los proyectos para «Al chat».
                let contexto_captura = captura2::Contexto {
                    textos: captura2::Textos::de(&textos),
                    ultima_region,
                    raiz: Some(ubicacion.raiz().to_path_buf()),
                    aparato: identidad_equipo.clone(),
                    gestos: gancho.is_some(),
                };
                let formato = match config.formato_color {
                    ajustes::FormatoColor::Hex => FormatoColorLupa::Hex,
                    ajustes::FormatoColor::Rgb => FormatoColorLupa::Rgb,
                    ajustes::FormatoColor::Hsl => FormatoColorLupa::Hsl,
                };
                // Con el overlay delante, un Alt+clic es del overlay: el
                // gancho se aparta mientras tanto.
                if let Some(g) = &gancho {
                    g.suspender(true);
                }
                let accion = match &mut recursos_overlay {
                    Some(r) => Ok(r),
                    nada => Recursos::nuevos().map(|r| nada.insert(r)),
                }
                .and_then(|r| {
                    ejecutar_overlay(
                        r,
                        decision.nivel,
                        modo,
                        &etiquetas_barra,
                        formato,
                        inicio,
                        desde_ms,
                        contexto_captura,
                    )
                });
                if let Some(g) = &gancho {
                    g.suspender(false);
                }
                let resultado = accion.and_then(|accion| match accion {
                    // La captura con scroll (D75/D77): el overlay ya esta
                    // oculto; ahora se recorre la region y la pagina cosida
                    // va al portapapeles y se queda como pin.
                    AccionFinal::Scroll { region } => {
                        let imagen = {
                            let r = recursos_overlay
                                .as_mut()
                                .context("sin recursos para la captura con scroll")?;
                            scroll::ejecutar_scroll(r, region)?
                        };
                        let Some(imagen) = imagen else {
                            tracing::info!("captura con scroll sin resultado");
                            return Ok(None);
                        };
                        if let Err(e) = pixpin_codec::copiar_imagen(&imagen) {
                            tracing::warn!(?e, "la pagina cosida no se pudo copiar");
                        }
                        let p = preparar_pines(
                            &mut recursos_overlay,
                            &mut pines,
                            &ubicacion,
                            &textos,
                            hwnd,
                            ritmo_video,
                        )?;
                        let d = pixpin_capture::enumerar_monitores()?;
                        let m = d.principal().context("sin monitor")?.to_owned();
                        p.pinear_imagen_centrada(&imagen, &m)?;
                        tracing::info!(alto = imagen.alto, "pagina cosida pineada");
                        Ok(None)
                    }
                    // Grabar en GIF (P5): como el scroll, el overlay ya esta
                    // oculto y ahora se captura la region una y otra vez.
                    AccionFinal::Gif { region } => {
                        let grabado = {
                            let r = recursos_overlay
                                .as_mut()
                                .context("sin recursos para grabar")?;
                            gif::ejecutar_sesion(
                                r,
                                region,
                                Some(comandos::Comando::GrabarGif.id()),
                                // Manda lo ultimo que se eligio en la
                                // barra; los ajustes ponen el punto de
                                // partida la primera vez.
                                pixpin_store::estado::cargar(&ubicacion)
                                    .gif_por_segundo
                                    .unwrap_or(config.gif.por_segundo),
                                std::time::Duration::from_secs(config.gif.retardo_s as u64),
                                &textos,
                            )?
                        };
                        let Some(g) = grabado else {
                            tracing::info!("grabacion sin fotogramas aprovechables");
                            return Ok(None);
                        };
                        // El ritmo elegido se recuerda para la proxima, y
                        // va a `estado.toml` y no a los ajustes: guardar
                        // los ajustes reescribe el fichero entero y se
                        // llevaria por delante los comentarios que el
                        // usuario tiene ahi explicando cada linea.
                        let mut estado = pixpin_store::estado::cargar(&ubicacion);
                        if estado.gif_por_segundo != Some(g.por_segundo) {
                            estado.gif_por_segundo = Some(g.por_segundo);
                            if let Err(e) = pixpin_store::estado::guardar(&ubicacion, &estado) {
                                tracing::warn!(?e, "no se pudo recordar el ritmo");
                            }
                        }
                        // El editor: se ve lo grabado antes de decidir que
                        // hacer con ello. Media grabacion sale mal a la
                        // primera, y guardarlas sin mirar llena la carpeta
                        // de ficheros que hay que borrar despues.
                        let (salida, formato) = {
                            let d = pixpin_capture::enumerar_monitores()?;
                            let m = d
                                .monitores()
                                .iter()
                                .find(|m| m.area.interseccion(region).is_some())
                                .or_else(|| d.principal())
                                .context("sin monitor para el editor")?
                                .to_owned();
                            let r = recursos_overlay
                                .as_ref()
                                .context("sin recursos para el editor")?;
                            editor::abrir(r, &g, &m, &textos)?
                        };
                        if salida == reproductor::Salida::Descartar {
                            tracing::info!("grabacion descartada");
                            return Ok(None);
                        }
                        // La ruta se decide ANTES de codificar: el MP4 se
                        // escribe directamente a fichero, asi que no hay un
                        // monton de bytes que ensenar antes de saber donde
                        // van. Y si se cancela el dialogo, se ahorra la
                        // codificacion entera.
                        let ruta = match salida {
                            reproductor::Salida::Guardar => {
                                match pixpin_shell::guardar::pedir_ruta_guardado(
                                    hwnd,
                                    &format!("grabacion.{}", formato.extension()),
                                    match formato {
                                        reproductor::Formato::Gif => {
                                            pixpin_shell::guardar::Formatos::Gif
                                        }
                                        reproductor::Formato::Mp4 => {
                                            pixpin_shell::guardar::Formatos::Mp4
                                        }
                                    },
                                ) {
                                    Some(r) => r,
                                    // Cancelar el dialogo cancela el guardado
                                    // entero. Dejarlo caer a la carpeta de
                                    // capturas seria escribir un fichero que
                                    // se acaba de decir que no.
                                    None => return Ok(None),
                                }
                            }
                            _ => {
                                ruta_captura_libre(&ubicacion)?.with_extension(formato.extension())
                            }
                        };
                        let pesa = match formato {
                            reproductor::Formato::Gif => {
                                let bytes = pixpin_codec::codificar_gif(
                                    &g.fotogramas,
                                    pixpin_codec::OpcionesGif {
                                        centesimas_por_fotograma: g.centesimas_por_fotograma(),
                                        bucle: true,
                                    },
                                )
                                .context("no se pudo codificar el GIF")?;
                                std::fs::write(&ruta, &bytes).with_context(|| {
                                    format!("no se pudo escribir {}", ruta.display())
                                })?;
                                bytes.len()
                            }
                            reproductor::Formato::Mp4 => {
                                pixpin_record::codificar_mp4(
                                    &g.fotogramas,
                                    pixpin_record::OpcionesMp4 {
                                        por_segundo: g.por_segundo,
                                        bitrate: None,
                                    },
                                    &ruta,
                                )
                                .context("no se pudo codificar el MP4")?;
                                std::fs::metadata(&ruta)
                                    .map(|m| m.len() as usize)
                                    .unwrap_or(0)
                            }
                        };
                        // Copiar deja el FICHERO en el portapapeles, no la
                        // imagen: el portapapeles de Windows solo guarda un
                        // fotograma, asi que pegar la imagen daria una foto
                        // quieta y pareceria que el GIF salio roto. Como
                        // fichero se pega entero y sigue moviendose.
                        if salida == reproductor::Salida::Copiar {
                            if let Err(e) =
                                pixpin_codec::copiar_ficheros(std::slice::from_ref(&ruta))
                            {
                                tracing::warn!(?e, "el GIF no se pudo copiar");
                            }
                        }
                        tracing::info!(
                            fotogramas = g.fotogramas.len(),
                            kb = pesa / 1024,
                            fin = ?g.fin,
                            ?salida,
                            ?formato,
                            "GIF guardado"
                        );
                        Ok(Some(ruta))
                    }
                    AccionFinal::PinEnVivo { region } => {
                        let p = preparar_pines(
                            &mut recursos_overlay,
                            &mut pines,
                            &ubicacion,
                            &textos,
                            hwnd,
                            ritmo_video,
                        )?;
                        let r = recursos_overlay
                            .as_ref()
                            .context("sin recursos para la captura en vivo")?;
                        // El mismo tope que el modo vivo del overlay (D14):
                        // en Ligero, 30 fps bastan para seguir una zona y no
                        // roban la GPU compartida.
                        let tope = match decision.nivel {
                            Nivel::Completo => std::time::Duration::ZERO,
                            Nivel::Ligero => std::time::Duration::from_millis(33),
                        };
                        p.pinear_en_vivo(r.dispositivo(), region, tope)?;
                        Ok(None)
                    }
                    AccionFinal::Pinear { imagen, region } => {
                        // El gestor consume la accion aqui, no en
                        // ejecutar_accion: el pin nace 1:1 en la region del
                        // recorte (D26), con la escala de su monitor.
                        let p = preparar_pines(
                            &mut recursos_overlay,
                            &mut pines,
                            &ubicacion,
                            &textos,
                            hwnd,
                            ritmo_video,
                        )?;
                        ultima_region = Some(region);
                        let nuevo = p.pinear(&imagen, region, escala_del_monitor(region))?;
                        tracing::info!(abiertos = p.abiertos(), "pin creado");
                        // «Capturar y anotar» encadena las dos cosas: el pin
                        // nace ya con la paleta abierta y el lapiz listo,
                        // que es lo que se quiere al senalar algo deprisa.
                        if anotar_al_pinear {
                            p.anotar_pin(nuevo)?;
                        }
                        Ok(None)
                    }
                    // Al chat de un proyecto, elegido en la captura (v2).
                    AccionFinal::AlChat {
                        imagen,
                        region,
                        proyecto,
                        comentario,
                    } => {
                        ultima_region = Some(region);
                        let nombre = captura2::al_chat::mandar(
                            ubicacion.raiz(),
                            &proyecto,
                            &identidad_equipo,
                            &imagen,
                            &comentario,
                        )?;
                        let mut args = fluent_bundle::FluentArgs::new();
                        args.set("proyecto", nombre);
                        let _ = bandeja.avisar(
                            &textos.t("app-nombre"),
                            &textos.t_args("captura2-enviada", &args),
                        );
                        Ok(None)
                    }
                    // Copiar pasa por la pila de capturas, que necesita los
                    // recursos de dibujo para su icono: por eso se atiende
                    // aqui y no en `ejecutar_accion`.
                    AccionFinal::Copiar { imagen, region } => {
                        ultima_region = Some(region);
                        let r = recursos_overlay
                            .as_ref()
                            .context("sin recursos para la pila de capturas")?;
                        pila.entrar(imagen, region, r, &ubicacion, &textos)?;
                        Ok(None)
                    }
                    otra => ejecutar_accion(otra, &ubicacion, hwnd),
                });
                match resultado {
                    Ok(Some(ruta)) => {
                        let mut args = fluent_bundle::FluentArgs::new();
                        args.set("ruta", ruta.display().to_string());
                        tracing::info!("{}", textos.t_args("captura-guardada", &args));
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let mut args = fluent_bundle::FluentArgs::new();
                        args.set("motivo", e.to_string());
                        tracing::warn!("{}", textos.t_args("captura-fallo", &args));
                    }
                }
                Continuar::Si
            }
            // Desde la bandeja (lo normal: de fabrica no tienen atajo, D81) o
            // desde el atajo que el usuario se ponga en el TOML, o con
            // Alt + doble clic central (el cuarto gesto, 2026-09-28), que abre
            // al instante el vivo: nada que capturar antes de ensenarlo.
            _ if matches!(
                comando,
                Some(comandos::Comando::Anotar | comandos::Comando::AnotarCongelada)
            ) || matches!(
                evento,
                Evento::Gesto {
                    boton: BotonGesto::DobleCentral,
                    ..
                }
            ) =>
            {
                let modo = if comando != Some(comandos::Comando::AnotarCongelada) {
                    capa::ModoCapa::Viva
                } else {
                    capa::ModoCapa::Congelada
                };
                let listo = match &mut recursos_overlay {
                    Some(r) => Ok(r),
                    nada => Recursos::nuevos().map(|r| nada.insert(r)),
                };
                if let Some(g) = &gancho {
                    g.suspender(true);
                }
                let capa_hecha = listo.and_then(|r| {
                    capa::ejecutar_capa(
                        r,
                        modo,
                        decision.nivel,
                        config.enganche,
                        Some(ubicacion.raiz().to_path_buf()),
                        hwnd.0 as isize,
                    )
                });
                if let Some(g) = &gancho {
                    g.suspender(false);
                }
                match capa_hecha {
                    // D54: cerrar sin avisar tirando cinco minutos de
                    // anotaciones es el peor fallo posible aqui. Se pregunta
                    // con la capa ya cerrada, para que el cuadro no salga en
                    // la captura.
                    Ok(Some(_))
                        if !pixpin_shell::preguntar(
                            hwnd,
                            &textos.t("capa-guardar-titulo"),
                            &textos.t("capa-guardar-pregunta"),
                        ) =>
                    {
                        tracing::info!("anotacion de pantalla descartada por el usuario");
                    }
                    Ok(Some(imagen)) => {
                        let hecho = preparar_pines(
                            &mut recursos_overlay,
                            &mut pines,
                            &ubicacion,
                            &textos,
                            hwnd,
                            ritmo_video,
                        )
                        .and_then(|p| {
                            let d = pixpin_capture::enumerar_monitores()?;
                            let m = d.principal().context("sin monitor")?.to_owned();
                            p.pinear_imagen_centrada(&imagen, &m).map(|_| ())
                        });
                        match hecho {
                            Ok(()) => tracing::info!("anotacion de pantalla pineada"),
                            Err(e) => tracing::warn!(?e, "no se pudo pinear la anotacion"),
                        }
                    }
                    Ok(None) => tracing::info!("capa viva cerrada sin dibujo"),
                    Err(e) => tracing::warn!(?e, "no se pudo abrir la capa viva"),
                }
                Continuar::Si
            }
            // Pinear lo seleccionado en el Explorador (P1.6). NO pasa por
            // el portapapeles: usarlo obligaria a copiar y luego dejarlo
            // pisado, y el usuario perderia lo que tuviera copiado sin que
            // nadie se lo hubiera preguntado.
            _ if comando == Some(comandos::Comando::PinearSeleccion) => {
                let rutas = pixpin_shell::seleccion_del_explorador();
                if rutas.is_empty() {
                    tracing::info!("delante no hay un Explorador con nada seleccionado");
                    return Continuar::Si;
                }
                let hecho = preparar_pines(
                    &mut recursos_overlay,
                    &mut pines,
                    &ubicacion,
                    &textos,
                    hwnd,
                    ritmo_video,
                )
                .and_then(|p| {
                    pinear_portapapeles(p, pixpin_codec::ContenidoPortapapeles::Rutas(rutas))
                });
                match hecho {
                    Ok(cuantos) => tracing::info!(cuantos, "pineada la seleccion del Explorador"),
                    Err(e) => tracing::warn!(?e, "no se pudo pinear la seleccion"),
                }
                Continuar::Si
            }
            Evento::Atajo(id) if id == atajos::ID_PORTAPAPELES => {
                // Pinear el portapapeles NO abre overlay: aparece un pin
                // centrado en el monitor del cursor y sin robar el foco
                // (4.4), asi que no interrumpe donde estabas escribiendo.
                match pixpin_codec::leer() {
                    None => tracing::info!("portapapeles vacio o con un formato ajeno"),
                    Some(contenido) => {
                        let hecho = preparar_pines(
                            &mut recursos_overlay,
                            &mut pines,
                            &ubicacion,
                            &textos,
                            hwnd,
                            ritmo_video,
                        )
                        .and_then(|p| pinear_portapapeles(p, contenido));
                        match hecho {
                            Ok(cuantos) => tracing::info!(cuantos, "pineado del portapapeles"),
                            Err(e) => tracing::warn!(?e, "no se pudo pinear el portapapeles"),
                        }
                    }
                }
                Continuar::Si
            }
            // Todo comando del catalogo se atiende arriba; si algo llega
            // hasta aqui es que se anadio una fila y se olvido la accion.
            Evento::Atajo(id) | Evento::Menu(id) => {
                tracing::warn!(id, ?comando, "comando sin accion");
                Continuar::Si
            }
            // Ya atendido por la guarda `modo_overlay` de arriba; queda
            // solo para que el match sea exhaustivo.
            Evento::Gesto { .. } => Continuar::Si,
            // Un pin dejo algo pendiente; el trabajo esta tras el match, en
            // `purgar`. Aqui no hay nada que hacer salvo haber girado.
            Evento::Despertar => Continuar::Si,
            Evento::MostrarGrupo(id_grupo) => {
                match pixpin_capture::enumerar_monitores() {
                    Ok(d) => {
                        let vueltos = pines
                            .as_mut()
                            .map(|p| p.mostrar_grupo(id_grupo, &d))
                            .unwrap_or(0);
                        tracing::info!(id_grupo, vueltos, "grupo mostrado de nuevo");
                    }
                    Err(e) => tracing::warn!(?e, "sin monitores para mostrar el grupo"),
                }
                Continuar::Si
            }
        };
        // Un pin cerrado desde su propio WndProc solo apunta su id; aqui
        // se saca de la lista. Barato: nada que hacer si no cerro ninguno.
        if let Some(p) = &mut pines {
            p.purgar();
            // D132/D144: el lienzo de un pin se abre aqui, fuera del gestor,
            // porque el editor tiene su propio bucle. Mientras dura, lo que
            // pidan otros pines espera en su cola; al cerrar se atiende con
            // otro `purgar` (que puede traer otro lienzo, de ahi el `while`).
            while let Some(pedido) = p.tomar_lienzo() {
                let crate::pines::PedidoLienzo {
                    id,
                    ruta,
                    habia_fichero,
                    escena,
                    fondo,
                } = pedido;
                // F5: las marcas de este lienzo viven junto a su dibujo.
                let _marcas = ventana_editor::marcas::junto_a(&ruta);
                // Las imagenes pegadas en el lienzo del pin, en la carpeta
                // de al lado del dibujo (`imagenes_lienzo`).
                let fotos = imagenes_lienzo::fotos_junto_a(&ruta, &escena);
                let mut pegadas = Vec::new();
                let resultado = ventana_editor::abrir_con_pegadas(
                    escena,
                    config.enganche,
                    decision.nivel,
                    config.rendimiento.medir_fotogramas,
                    Some(fondo),
                    &fotos,
                    &mut pegadas,
                );
                if let Ok((escena, _)) = &resultado
                    && let Err(e) =
                        imagenes_lienzo::guardar_pegadas_junto_a(&ruta, escena, &pegadas)
                {
                    tracing::error!(?e, "no se pudieron guardar las imagenes pegadas del pin");
                }
                p.terminar_lienzo(id, &ruta, habia_fichero, resultado.map(|(e, _)| e));
                p.purgar();
            }
            // Extraer paginas puede haber dejado algunas fuera por el
            // tope. Se avisa AQUI y no en el gestor porque la bandeja vive
            // en este bucle, y callarselo dejaria al usuario contando
            // pines para averiguar que falta la mitad.
            if let Some((hechas, total)) = p.tomar_paginas_extraidas() {
                if hechas < total {
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("hechas", hechas.to_string());
                    args.set("total", total.to_string());
                    let _ = bandeja.avisar(
                        &textos.t("aviso-paginas-extraidas"),
                        &textos.t_args("aviso-paginas-extraidas-detalle", &args),
                    );
                }
            }
        }
        // Lo que se haya pulsado en el panel de la pila de capturas. Como
        // `purgar`: la ventanita solo apunta el pedido y da un toque.
        pila.atender(&textos, &mut bandeja);
        // Lo que haya vencido mientras tanto. Va AQUI, tras el match y no
        // dentro de `Evento::Despertar`, por lo mismo que `purgar`: el toque
        // del vigia y otro evento cualquiera pueden llegar juntos, y entonces
        // el bucle da una sola vuelta. Mirar una lista vacia no cuesta nada.
        atender_recordatorios(
            &mut recursos_overlay,
            &mut pines,
            &ubicacion,
            &textos,
            hwnd,
            ritmo_video,
        );
        // Los interruptores del panel de la bandeja, tal como quedaron.
        centro_bandeja::publicar(centro_bandeja::Interruptores {
            a_la_vista: if pines_ocultos {
                0
            } else {
                pines.as_ref().map_or(0, |p| p.abiertos())
            },
            ocultos: pines_ocultos,
            pasantes: pines_pasantes,
            silenciados: registrados.is_none(),
        });
        seguir
    });

    tracing::info!("PixPin Max terminado limpiamente");
    Ok(())
}

/// Saca a la pantalla lo que se le pidio recordar y le quita la hora.
///
/// **El pin es lo que de verdad avisa**, y el globo va de propina: Windows
/// silencia los globos con la concentracion puesta, en modo presentacion o si
/// el usuario apago las notificaciones, y un recordatorio que solo sale por
/// ahi es un recordatorio que se pierde. Es la misma decision del movil
/// (`pin/RecordatorioReceiver.kt`: «no una notificacion... el pin vuelve a la
/// pantalla»).
///
/// Y acto seguido se le quita la hora al mensaje: si no, la conversacion queda
/// con una alarma fantasma que ya sono y volveria a sonar en el proximo
/// arranque. El movil hace lo mismo poniendo `recuerdaEn = null`.
fn atender_recordatorios(
    recursos: &mut Option<Recursos>,
    pines: &mut Option<Pines>,
    ubicacion: &Ubicacion,
    textos: &Catalogo,
    hwnd: windows::Win32::Foundation::HWND,
    ritmo_video: u32,
) {
    let perdidas = llamada::tomar_perdidas();
    let volver = llamada::tomar_volver();
    let vencidos = recordatorios::tomar_vencidos();
    if vencidos.is_empty() && perdidas.is_empty() && volver.is_empty() {
        return;
    }
    // «Volver a llamar en» (B11, v0.98.6): la misma alarma a otra hora y el
    // aviso de cuando, como el `Toast` del movil.
    for v in volver {
        match recordatorios::volver_a_poner(&v.carpeta, &v.mensaje, v.cuando_utc_ms) {
            Ok(true) => {
                let local = pixpin_shell::entorno::a_local(v.cuando_utc_ms);
                let mut args = fluent_bundle::FluentArgs::new();
                args.set(
                    "hora",
                    recordatorios::cuando_legible(local, pixpin_shell::entorno::ahora_local_ms()),
                );
                let mut aviso = pixpin_shell::aviso::Aviso::sobre_la_bandeja(hwnd);
                if let Err(e) = aviso.mostrar(
                    &textos.t("llamada-titulo"),
                    &textos.t_args("llamada-vuelve-a-las", &args),
                ) {
                    tracing::warn!(?e, "no se pudo avisar de cuando vuelve la llamada");
                }
                ventana_chat::refrescar();
            }
            Ok(false) => tracing::warn!(mensaje = %v.mensaje, "la nota de la llamada ya no esta"),
            Err(e) => tracing::warn!(?e, "no se pudo volver a poner la llamada"),
        }
    }
    // El monitor se busca UNA vez para todos: enumerar monitores es una
    // llamada al sistema y dos recordatorios para el mismo minuto son
    // normales (los de «lo que vencio con el programa cerrado» salen juntos).
    let monitor = pixpin_capture::enumerar_monitores().ok().and_then(|d| {
        d.monitor_en(pixpin_shell::posicion_del_cursor())
            .or_else(|| d.principal())
            .map(|m| m.to_owned())
    });
    let mut aviso = pixpin_shell::aviso::Aviso::sobre_la_bandeja(hwnd);
    let mut hubo = false;
    // Las llamadas secretas que nadie contesto (B11): su pin, como el
    // `pinTexto("Llamada perdida · …")` del movil. El globo ya salio.
    if let Some(monitor) = &monitor {
        for texto in perdidas {
            let pineado = preparar_pines(recursos, pines, ubicacion, textos, hwnd, ritmo_video)
                .and_then(|p| p.pinear_nota(&texto, monitor));
            if let Err(e) = pineado {
                tracing::warn!(?e, "no se pudo sacar el pin de la llamada perdida");
            }
        }
    }
    for v in vencidos {
        let r = v.recordatorio;
        tracing::info!(id = %r.id, "vencio un recordatorio");
        // **Una nota de voz con hora es una llamada secreta** (B11,
        // `RecordatorioReceiver`): suena como una llamada y el recado se
        // oye por la salida de llamadas. Lo demas, un pin como siempre.
        let secreta = v
            .carpeta
            .as_deref()
            .and_then(|c| llamada::de_un_recordatorio(c, &r.id));
        let es_secreta = secreta.is_some();
        if let Some(l) = secreta {
            llamada::lanzar(l, rotulos_de_la_llamada(textos), hwnd.0 as isize);
        } else if let Some(monitor) = &monitor {
            let pineado = preparar_pines(recursos, pines, ubicacion, textos, hwnd, ritmo_video)
                .and_then(|p| p.pinear_nota(&r.texto, monitor));
            if let Err(e) = pineado {
                tracing::warn!(?e, "no se pudo sacar el pin del recordatorio");
            }
        } else {
            tracing::warn!("sin monitor donde sacar el pin del recordatorio");
        }
        // La secreta no dice su recado en un globo: eso es lo secreto.
        if !es_secreta
            && let Err(e) = aviso.mostrar(&textos.t("chat-recordatorio-titulo"), &r.texto)
        {
            tracing::warn!(?e, "no se pudo avisar del recordatorio");
        }
        match &v.carpeta {
            Some(carpeta) => {
                if let Err(e) = recordatorios::olvidar(carpeta, &r.id) {
                    tracing::warn!(?e, "no se pudo quitar la hora ya sonada");
                } else {
                    hubo = true;
                }
            }
            // El proyecto se borro entre que se puso la hora y que llego: no
            // hay cuaderno que tocar, pero el aviso ya salio, que es lo suyo.
            None => tracing::warn!(id = %r.id, "recordatorio sin carpeta conocida"),
        }
    }
    // El chat tiene el reloj de esa burbuja pintado: sin esto seguiria ahi
    // hasta que el usuario entrara y saliera del proyecto.
    if hubo {
        ventana_chat::refrescar();
    }
}

/// Los rotulos de la llamada secreta, traducidos aqui: la ventanita vive en
/// su hilo y el `Catalogo` no cruza hilos.
fn rotulos_de_la_llamada(textos: &Catalogo) -> llamada::Rotulos {
    llamada::Rotulos {
        entrante: textos.t("llamada-entrante"),
        contestar: textos.t("llamada-contestar"),
        colgar: textos.t("llamada-colgar"),
        altavoz: textos.t("llamada-altavoz"),
        llamada: textos.t("llamada-titulo"),
        perdida: textos.t("llamada-perdida"),
        volver_en: textos.t("llamada-volver-en"),
    }
}

/// Abre ficheros como «Abrir con PixPin»: cada uno en lo suyo (una nota en
/// su editor, un documento en su lector, un `.pixpin` en la lista de
/// proyectos y como pines, lo demas como pin).
///
/// Aparte del bucle porque llegan por dos sitios —otra copia del programa
/// (`Evento::AbrirFicheros`) y un pedido `abrir` de tipo `fichero`— y los
/// dos tienen que hacer exactamente lo mismo.
#[allow(clippy::too_many_arguments)]
fn abrir_ficheros(
    rutas: Vec<std::path::PathBuf>,
    lengua: pixpin_store::Idioma,
    ubicacion: &Ubicacion,
    identidad_equipo: &str,
    recursos_overlay: &mut Option<Recursos>,
    pines: &mut Option<Pines>,
    textos: &Catalogo,
    hwnd: windows::Win32::Foundation::HWND,
    ritmo_video: u32,
) {
    // Un Word, un libro o un PDF abiertos desde el Explorador
    // («Abrir con», doble clic) se LEEN, en su lector: es lo que
    // se pidio al abrirlos. Como pin solo eran una ficha con su
    // icono, que no se puede leer.
    // Un Markdown, en el editor de notas (no en el navegador).
    let (notas, rutas): (Vec<_>, Vec<_>) = rutas
        .into_iter()
        .partition(|r| destino_de_fichero(r) == DestinoDeFichero::Nota);
    for ruta in notas {
        notas_md::abrir(
            lengua,
            ubicacion.clone(),
            notas_md::Destino::Fichero { ruta },
        );
    }
    let (a_leer, rutas): (Vec<_>, Vec<_>) = rutas
        .into_iter()
        .partition(|r| destino_de_fichero(r) == DestinoDeFichero::Lector);
    for ruta in &a_leer {
        lector::abrir_en_su_lector(lengua, ubicacion, ruta, &pixpin_docs::nombre(ruta));
    }
    // Un audio suena en el reproductor flotante, sin abrir la app (como el
    // Enter sobre un audio en Flow Launcher). Varios a la vez: en fila, uno
    // tras otro en la misma ventanita.
    let (audios, rutas): (Vec<_>, Vec<_>) = rutas
        .into_iter()
        .partition(|r| destino_de_fichero(r) == DestinoDeFichero::Audio);
    if !audios.is_empty() {
        tracing::info!(cuantos = audios.len(), "audios al reproductor flotante");
        ventana_chat::reproducir_flotante_en_fila(
            textos,
            audios
                .into_iter()
                .map(|r| {
                    let titulo = titulo_del_audio(&r);
                    (r, titulo)
                })
                .collect(),
        );
    }
    // Cada ruta cae en el pin que le toque por su extension:
    // imagen, video o ficha de archivo. Eso ya lo decide el
    // gestor, que es quien conoce los tipos.
    // La lista de tareas de un mensaje del chat («Sacar a la pantalla»):
    // no es un fichero, es la ruta que dice de que mensaje es.
    let (listas, rutas): (Vec<_>, Vec<_>) = rutas
        .into_iter()
        .partition(|r| destino_de_fichero(r) == DestinoDeFichero::ListaDelChat);
    // Un `.pixpin` no es un fichero que pinear: es un proyecto
    // entero, y cada hoja suya sale como su propio pin.
    let (proyectos, sueltos): (Vec<_>, Vec<_>) = rutas
        .into_iter()
        .partition(|r| destino_de_fichero(r) == DestinoDeFichero::Proyecto);
    // Las fotos que solo lee Windows con una extension de la tienda (HEIC,
    // AVIF, RAW...): se leen aqui para que, si falta esa extension, salga
    // un pin que DIGA cual instalar, en vez de la ficha muda del archivo.
    let (de_la_tienda, sueltos): (Vec<_>, Vec<_>) = sueltos
        .into_iter()
        .partition(|r| pixpin_codec::wic::extension_de_la_tienda(r).is_some());
    // Y ademas entra en la LISTA de proyectos, que es donde el
    // usuario lo busca: un `.pixpin` es una conversacion entera.
    // Los pines de sus hojas siguen saliendo, que es lo de antes.
    for ruta in &proyectos {
        let hecho = pixpin_proyecto::Paquete::abrir(ruta)
            .map_err(|e| e.to_string())
            .and_then(|p| {
                pixpin_proyecto::almacen::importar_paquete(ubicacion.raiz(), &p, identidad_equipo)
                    .map_err(|e| e.to_string())
            });
        match hecho {
            Ok(f) => tracing::info!(id = %f.id, nombre = %f.nombre, "proyecto en la lista"),
            Err(e) => tracing::warn!(%e, ruta = %ruta.display(), "no se pudo importar"),
        }
    }
    // Solo audios, notas o documentos: nada que pinear, y los pines (que
    // crean el dispositivo de la GPU la primera vez) ni se tocan.
    if listas.is_empty() && proyectos.is_empty() && de_la_tienda.is_empty() && sueltos.is_empty() {
        return;
    }
    let hecho = preparar_pines(
        recursos_overlay,
        pines,
        ubicacion,
        textos,
        hwnd,
        ritmo_video,
    )
    .and_then(|p| {
        let d = pixpin_capture::enumerar_monitores()?;
        let m = d.principal().context("sin monitor")?.to_owned();
        let mut cuantos = 0;
        for v in listas
            .iter()
            .filter_map(|r| pines::herramienta::vinculo_de_ruta(r))
        {
            match p.pinear_lista_del_chat(v, &m) {
                Ok(_) => cuantos += 1,
                Err(e) => tracing::warn!(?e, "no se pudo sacar la lista a la pantalla"),
            }
        }
        for proyecto in &proyectos {
            // Un proyecto que falle no puede llevarse los
            // demas ficheros que venian con el.
            match p.abrir_paquete(proyecto, &m, ubicacion) {
                Ok((hechas, _)) => cuantos += hechas,
                Err(e) => {
                    tracing::warn!(?e, ruta = %proyecto.display(), "proyecto que no se pudo abrir")
                }
            }
        }
        for foto in &de_la_tienda {
            // HEIC, AVIF, RAW...: leidas a la medida del pin, como las demas
            // fotos. Si Windows no puede (falta la extension), el camino de
            // siempre, que dice cual instalar.
            if p.pinear_foto(foto, &m).is_ok() {
                cuantos += 1;
                continue;
            }
            match pixpin_codec::cargar(foto) {
                Ok(img) => match p.pinear_imagen_centrada(&img, &m) {
                    Ok(_) => cuantos += 1,
                    Err(e) => {
                        tracing::warn!(?e, ruta = %foto.display(), "no se pudo pinear la foto")
                    }
                },
                Err(e) => {
                    tracing::warn!(%e, ruta = %foto.display(), "foto que Windows no sabe leer");
                    let texto = match e.extension_que_falta() {
                        Some(tienda) => {
                            let mut args = fluent_bundle::FluentArgs::new();
                            args.set("nombre", pixpin_docs::nombre(foto));
                            args.set("extension", tienda);
                            textos.t_args("abrir-falta-extension", &args)
                        }
                        None => e.to_string(),
                    };
                    if p.pinear_nota(&texto, &m).is_ok() {
                        cuantos += 1;
                    }
                }
            }
        }
        if !sueltos.is_empty() {
            cuantos += pinear_portapapeles(p, pixpin_codec::ContenidoPortapapeles::Rutas(sueltos))?;
        }
        Ok(cuantos)
    });
    match hecho {
        Ok(cuantos) => tracing::info!(cuantos, "ficheros abiertos como pines"),
        Err(e) => tracing::warn!(?e, "no se pudieron abrir los ficheros"),
    }
}

/// A donde va un fichero abierto «con PixPin» (doble clic, «Abrir con»,
/// pedido `abrir`). Puro, por la ruta: [`abrir_ficheros`] lo reparte asi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DestinoDeFichero {
    /// Markdown: al editor de notas.
    Nota,
    /// PDF, Word, libro: a su lector.
    Lector,
    /// Un audio: al reproductor flotante.
    Audio,
    /// La lista de tareas de un mensaje del chat («Sacar a la pantalla»).
    ListaDelChat,
    /// Un `.pixpin`: a la lista de proyectos y sus hojas como pines.
    Proyecto,
    /// Lo demas (fotos, videos, cualquier archivo): un pin.
    Pin,
}

fn destino_de_fichero(ruta: &std::path::Path) -> DestinoDeFichero {
    if notas_md::es_markdown(ruta) {
        DestinoDeFichero::Nota
    } else if lector::se_lee_al_tocar(&pixpin_docs::nombre(ruta)) {
        DestinoDeFichero::Lector
    } else if pixpin_shell::asociaciones::es_audio(ruta) {
        DestinoDeFichero::Audio
    } else if pines::herramienta::vinculo_de_ruta(ruta).is_some() {
        DestinoDeFichero::ListaDelChat
    } else if ruta
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pixpin"))
    {
        DestinoDeFichero::Proyecto
    } else {
        DestinoDeFichero::Pin
    }
}

/// Lo que se lee en la barra del reproductor: el nombre sin la extension.
fn titulo_del_audio(ruta: &std::path::Path) -> String {
    ruta.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| pixpin_docs::nombre(ruta))
}

#[cfg(test)]
mod pruebas_abrir_ficheros {
    use super::*;
    use std::path::Path;

    #[test]
    fn abrir_cada_fichero_va_a_lo_suyo() {
        let d = |r: &str| destino_de_fichero(Path::new(r));
        assert_eq!(d(r"C:\musica\Cancion.MP3"), DestinoDeFichero::Audio);
        for a in [
            "a.m4a", "b.wav", "c.flac", "d.ogg", "e.opus", "f.aac", "g.wma", "h.amr", "i.3gp",
        ] {
            assert_eq!(d(a), DestinoDeFichero::Audio, "{a}");
        }
        assert_eq!(d("notas.md"), DestinoDeFichero::Nota);
        assert_eq!(d("libro.pdf"), DestinoDeFichero::Lector);
        assert_eq!(d("proyecto.PIXPIN"), DestinoDeFichero::Proyecto);
        for f in ["foto.png", "IMG_0001.HEIC", "b.avif", "clip.mp4", "c.tif"] {
            assert_eq!(d(f), DestinoDeFichero::Pin, "{f}");
        }
    }

    #[test]
    fn abrir_caso_negativo_un_video_o_algo_sin_extension_no_va_al_reproductor() {
        let d = |r: &str| destino_de_fichero(Path::new(r));
        assert_ne!(d("pelicula.mp4"), DestinoDeFichero::Audio);
        assert_ne!(
            d("mp3"),
            DestinoDeFichero::Audio,
            "sin punto no es extension"
        );
        assert_ne!(d("cancion.mp3.txt"), DestinoDeFichero::Audio);
    }

    #[test]
    fn abrir_un_audio_lo_titula_sin_extension() {
        assert_eq!(
            titulo_del_audio(Path::new(r"C:\x\Nota de voz 3.m4a")),
            "Nota de voz 3"
        );
        // Caso negativo: un punto en medio no se come el resto del nombre.
        assert_eq!(titulo_del_audio(Path::new("v1.2 final.mp3")), "v1.2 final");
    }

    /// Un WAV de `ms` milisegundos, mono de 16 bits, con un tono casi mudo
    /// (no molesta a quien este trabajando al lado).
    fn wav(ruta: &Path, ms: u32) {
        let hz = 22_050u32;
        let n = hz * ms / 1000;
        let mut datos = Vec::with_capacity(n as usize * 2);
        for i in 0..n {
            let v = ((i as f32 * 440.0 * std::f32::consts::TAU / hz as f32).sin() * 300.0) as i16;
            datos.extend_from_slice(&v.to_le_bytes());
        }
        let mut f = Vec::new();
        f.extend_from_slice(b"RIFF");
        f.extend_from_slice(&(36 + datos.len() as u32).to_le_bytes());
        f.extend_from_slice(b"WAVEfmt ");
        f.extend_from_slice(&16u32.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes()); // PCM
        f.extend_from_slice(&1u16.to_le_bytes()); // mono
        f.extend_from_slice(&hz.to_le_bytes());
        f.extend_from_slice(&(hz * 2).to_le_bytes());
        f.extend_from_slice(&2u16.to_le_bytes());
        f.extend_from_slice(&16u16.to_le_bytes());
        f.extend_from_slice(b"data");
        f.extend_from_slice(&(datos.len() as u32).to_le_bytes());
        f.extend_from_slice(&datos);
        std::fs::write(ruta, f).unwrap();
    }

    /// **De verdad, en el escritorio**: abre dos audios, un GIF y (si se da
    /// en `PIXPIN_PROBAR_HEIC`) una foto HEIC como lo haria el Explorador.
    /// Salen el reproductor flotante (que se va solo al acabar los dos
    /// audios) y los pines, que se cierran al acabar la prueba.
    /// `cargo test -p pixpin abrir_de_verdad -- --ignored --nocapture`.
    #[test]
    #[ignore = "abre ventanas de verdad en el escritorio"]
    fn abrir_de_verdad_audios_y_fotos() {
        let dir = std::env::temp_dir().join("pixpin-abrir-de-verdad");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("Primero.wav"), dir.join("Segundo.wav"));
        wav(&a, 2500);
        wav(&b, 2500);
        let rojo = pixpin_codec::ImagenRgba {
            ancho: 160,
            alto: 90,
            pixeles: [220u8, 40, 40, 255].repeat(160 * 90),
        };
        let gif = dir.join("rojo.gif");
        std::fs::write(
            &gif,
            pixpin_codec::codificar_gif(&[rojo], Default::default()).unwrap(),
        )
        .unwrap();
        let mut rutas = vec![a, b, gif];
        match std::env::var("PIXPIN_PROBAR_HEIC") {
            Ok(heic) => rutas.push(heic.into()),
            // Sin foto de verdad, un «HEIC» que Windows no entiende: sale el
            // pin que dice que extension instalar.
            Err(_) => {
                let falso = dir.join("IMG_prueba.heic");
                std::fs::write(&falso, b"no es una foto").unwrap();
                rutas.push(falso);
            }
        }
        let ubicacion = rutas::resolver(&dir, &dir.join("appdata"));
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let mut recursos = None;
        let mut pines = None;
        abrir_ficheros(
            rutas,
            pixpin_store::Idioma::Espanol,
            &ubicacion,
            "prueba",
            &mut recursos,
            &mut pines,
            &textos,
            windows::Win32::Foundation::HWND::default(),
            33,
        );
        println!("pines abiertos: {}", pines.is_some());
        // Tiempo para mirar (y hacer una captura); los pines se quedan
        // mientras, y bombear mensajes los deja pintarse.
        let hasta = std::time::Instant::now() + std::time::Duration::from_secs(9);
        while std::time::Instant::now() < hasta {
            pixpin_shell::overlay::bombear_pendientes();
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        drop(pines);
    }
}

/// Recursos y Pines comparten creacion perezosa: los pines necesitan el
/// dispositivo y el motor que viven en los recursos del overlay.
fn preparar_pines<'a>(
    recursos: &mut Option<Recursos>,
    pines: &'a mut Option<Pines>,
    ubicacion: &Ubicacion,
    textos: &Catalogo,
    hwnd_app: windows::Win32::Foundation::HWND,
    ritmo_video_ms: u32,
) -> Result<&'a mut Pines> {
    if pines.is_none() {
        let r = match recursos {
            Some(r) => r,
            nada => nada.insert(Recursos::nuevos()?),
        };
        *pines = Some(Pines::nuevos(
            ubicacion.raiz(),
            r.d3d(),
            r.motor(),
            textos.t("pin-no-encontrado"),
            textos_del_pin(textos),
            textos.t("pin-eliminar-confirmar"),
            textos.t("pin-sin-codec"),
            hwnd_app,
            // Sin soporte de video en el dispositivo (D66) no hay reproductor:
            // los videos se ensenan como documento.
            r.dispositivo().soporta_video().then_some(ritmo_video_ms),
        )?);
    }
    Ok(pines.as_mut().expect("recien comprobado o creado"))
}

/// Lee el texto de una imagen y lo devuelve ya puesto en orden.
///
/// Las lineas llegan del sistema SIN orden de lectura: agruparlas en
/// parrafos y columnas es lo que hace que el texto pegado se parezca al
/// que se veia, en vez de a una lista de renglones sueltos.
///
/// Esta a comun porque lo usan las dos vias que leen texto: una zona de la
/// pantalla y un pin ya hecho. Dos copias de esto acabarian dando
/// resultados distintos para la misma imagen.
pub fn texto_de_imagen(imagen: &pixpin_codec::ImagenRgba) -> Result<String> {
    let lineas = pixpin_ocr::reconocer(imagen.ancho, imagen.alto, &imagen.pixeles)
        .context("no se pudo reconocer el texto")?;
    Ok(texto_de_lineas(lineas))
}

/// Pone en orden de lectura unas lineas YA reconocidas.
///
/// Separado de la lectura para poder reusar un reconocimiento que ya se
/// pago: reconocer la misma imagen dos veces son entre 170 y 670
/// milisegundos de raton trabado por lo mismo.
pub fn texto_de_lineas(lineas: Vec<pixpin_ocr::Linea>) -> String {
    pixpin_geom::parrafos::a_texto(&pixpin_geom::parrafos::agrupar(
        lineas
            .into_iter()
            .map(|l| pixpin_geom::parrafos::LineaTexto {
                caja: l.caja,
                texto: l.texto,
            })
            .collect(),
    ))
}

/// Las etiquetas del menu del pin, traducidas de una vez.
pub(crate) fn textos_del_pin(textos: &Catalogo) -> pixpin_pin::TextosPin {
    pixpin_pin::TextosPin {
        copiar: textos.t("pin-copiar"),
        guardar_como: textos.t("pin-guardar-como"),
        abrir_ubicacion: textos.t("pin-abrir-ubicacion"),
        tamano_original: textos.t("pin-tamano-original"),
        grupo: textos.t("pin-grupo"),
        sin_grupo: textos.t("pin-sin-grupo"),
        colores: [
            textos.t("pin-color-rojo"),
            textos.t("pin-color-naranja"),
            textos.t("pin-color-ambar"),
            textos.t("pin-color-verde"),
            textos.t("pin-color-cian"),
            textos.t("pin-color-azul"),
            textos.t("pin-color-violeta"),
            textos.t("pin-color-rosa"),
        ],
        reproducir: textos.t("pin-reproducir"),
        pausar: textos.t("pin-pausar"),
        sonido: textos.t("pin-sonido"),
        dejar_pasar_clic: textos.t("pin-dejar-pasar-clic"),
        copiar_texto: textos.t("pin-copiar-texto"),
        abrir_en_lienzo: textos.t("pin-abrir-en-lienzo"),
        congelar: textos.t("pin-congelar"),
        manejar: textos.t("pin-manejar"),
        dejar_de_manejar: textos.t("pin-dejar-de-manejar"),
        pagina_siguiente: textos.t("pin-pagina-siguiente"),
        pagina_anterior: textos.t("pin-pagina-anterior"),
        extraer_pagina: textos.t("pin-extraer-pagina"),
        extraer_todas: textos.t("pin-extraer-todas"),
        ocultar_grupo: textos.t("pin-ocultar-grupo"),
        cerrar: textos.t("pin-cerrar"),
        eliminar: textos.t("pin-eliminar"),
        no_encontrado: textos.t("pin-no-encontrado"),
        convertir_en: textos.t("pin-convertir-en"),
        herramientas: [
            textos.t("pin-herramienta-temporizador"),
            textos.t("pin-herramienta-cronometro"),
            textos.t("pin-herramienta-tareas"),
            textos.t("pin-herramienta-contador"),
            textos.t("pin-herramienta-gastos"),
            textos.t("pin-herramienta-pizarra"),
            textos.t("pin-herramienta-ruleta"),
            textos.t("pin-herramienta-lienzo"),
            textos.t("pin-herramienta-hoja"),
        ],
        fondo_pizarra: textos.t("pin-fondo-pizarra"),
        colores_pizarra: [
            textos.t("pin-pizarra-blanca"),
            textos.t("pin-pizarra-negra"),
            textos.t("pin-pizarra-azul"),
            textos.t("pin-pizarra-verde"),
        ],
        pautas_pizarra: [
            textos.t("pin-pauta-lisa"),
            textos.t("pin-pauta-cuadros"),
            textos.t("pin-pauta-rayas"),
            textos.t("pin-pauta-columnas"),
            textos.t("pin-pauta-puntos"),
        ],
        // v2-pines: la barra, el menu y el panel «Pines abiertos».
        v2: pines::textos_v2(textos),
    }
}

/// Crea un pin por cada cosa del portapapeles, en el monitor del cursor.
/// Devuelve cuantos nacieron: varias rutas copiadas dan varias fichas.
fn pinear_portapapeles(
    pines: &mut Pines,
    contenido: pixpin_codec::ContenidoPortapapeles,
) -> Result<usize> {
    use pixpin_codec::ContenidoPortapapeles as C;

    let disposicion = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let cursor = pixpin_shell::posicion_del_cursor();
    let monitor = disposicion
        .monitor_en(cursor)
        .or_else(|| disposicion.principal())
        .context("sin monitor donde pinear")?
        .to_owned();

    match contenido {
        C::Imagen(img) => {
            pines.pinear_imagen_centrada(&img, &monitor)?;
            Ok(1)
        }
        C::Texto(t) => {
            // Una palabra magica sola saca su herramienta y una tabla pegada
            // sale como tabla (C3, L3); lo demas, nota.
            pines.pinear_texto(&t, &monitor)?;
            Ok(1)
        }
        C::Rutas(rutas) => {
            let mut hechas = 0;
            for r in rutas {
                // Una foto es una foto: se pinea como imagen y no como la
                // ficha de un archivo. Se decide leyendola, porque las del
                // movil no traen extension por la que guiarse. Primero el
                // camino rapido (leida a la medida del pin, copiada tal cual
                // al almacen: `pines::foto`); si Windows no sabe leerla, el
                // de siempre.
                if r.is_file() {
                    match pines.pinear_foto(&r, &monitor) {
                        Ok(_) => {
                            hechas += 1;
                            continue;
                        }
                        Err(e) => tracing::debug!(?e, ruta = ?r, "sin camino rapido de foto"),
                    }
                }
                if r.is_file()
                    && let Ok(img) = pixpin_codec::cargar(&r)
                {
                    match pines.pinear_imagen_centrada(&img, &monitor) {
                        Ok(_) => hechas += 1,
                        Err(e) => tracing::warn!(?e, ruta = ?r, "no se pudo pinear la imagen"),
                    }
                    continue;
                }
                // Una ruta que falle no puede impedir que las demas se
                // pineen: se registra y se sigue.
                match pines.pinear_archivo(&r, &monitor) {
                    Ok(()) => hechas += 1,
                    Err(e) => tracing::warn!(?e, ruta = ?r, "no se pudo pinear el archivo"),
                }
            }
            Ok(hechas)
        }
    }
}

/// La escala del monitor que contiene la region; la del principal si no
/// toca ninguno; 100 como ultimo recurso si no se pueden enumerar.
fn escala_del_monitor(region: pixpin_geom::Rect) -> u32 {
    match pixpin_capture::enumerar_monitores() {
        Ok(d) => d
            .monitores()
            .iter()
            .find(|m| m.area.interseccion(region).is_some())
            .or_else(|| d.principal())
            .map_or(100, |m| m.escala_por_cien),
        Err(_) => 100,
    }
}

/// Ejecuta la accion que el overlay decidio. Devuelve la ruta si se guardo.
fn ejecutar_accion(
    accion: AccionFinal,
    ubicacion: &Ubicacion,
    hwnd: windows::Win32::Foundation::HWND,
) -> Result<Option<std::path::PathBuf>> {
    match accion {
        AccionFinal::Nada => Ok(None),
        // Reconocer el texto del recorte y copiarlo. Las lineas llegan sin
        // orden de lectura: agruparlas en parrafos y columnas es lo que
        // hace que el texto pegado se parezca al que se veia.
        AccionFinal::Texto(imagen) => {
            let texto = texto_de_imagen(&imagen)?;
            if texto.trim().is_empty() {
                tracing::info!("no se leyo texto en la zona elegida");
                return Ok(None);
            }
            pixpin_codec::copiar_texto(&texto).context("no se pudo copiar el texto")?;
            tracing::info!(largo = texto.len(), "texto copiado");
            Ok(None)
        }
        AccionFinal::Guardar(imagen) => {
            let ruta = ruta_captura_libre(ubicacion)?;
            pixpin_codec::guardar(&imagen, &ruta, pixpin_codec::FormatoImagen::Png)?;
            Ok(Some(ruta))
        }
        AccionFinal::Copiar { .. }
        | AccionFinal::Pinear { .. }
        | AccionFinal::Scroll { .. }
        | AccionFinal::Gif { .. }
        | AccionFinal::PinEnVivo { .. }
        | AccionFinal::AlChat { .. } => {
            // El bucle los intercepta antes de llamar aqui, porque necesitan
            // el gestor o los recursos de captura; llegar seria un error de
            // cableado.
            tracing::warn!("una accion diferida llego a ejecutar_accion; se ignora");
            Ok(None)
        }
        AccionFinal::GuardarComo(imagen) => {
            match pixpin_shell::guardar::pedir_ruta_guardado(
                hwnd,
                "captura.png",
                pixpin_shell::guardar::Formatos::Imagen,
            ) {
                None => Ok(None), // cancelado: la imagen se descarta sin drama
                Some(ruta) => {
                    let formato = ruta
                        .extension()
                        .and_then(|e| e.to_str())
                        .and_then(pixpin_codec::FormatoImagen::por_extension)
                        .unwrap_or(pixpin_codec::FormatoImagen::Png);
                    pixpin_codec::guardar(&imagen, &ruta, formato)?;
                    Ok(Some(ruta))
                }
            }
        }
    }
}

/// La siguiente ruta `captura-NNNN.png` libre en la carpeta de capturas.
///
/// Nombre por contador y no por fecha: `main` no tiene reloj inyectado y
/// S1-C traera las plantillas de nombre configurables.
fn ruta_captura_libre(ubicacion: &Ubicacion) -> Result<std::path::PathBuf> {
    let carpeta = ubicacion.raiz().join("capturas");
    std::fs::create_dir_all(&carpeta)?;
    let mut n = 1u32;
    loop {
        let candidata = carpeta.join(format!("captura-{n:04}.png"));
        if !candidata.exists() {
            return Ok(candidata);
        }
        n += 1;
        if n > 9999 {
            anyhow::bail!("demasiadas capturas en {}", carpeta.display());
        }
    }
}

/// Registro rotativo diario junto a los ajustes. Nada sale del equipo.
fn iniciar_registro(ubicacion: &Ubicacion) -> tracing_appender::non_blocking::WorkerGuard {
    let dir = ubicacion.raiz().join("registros");
    let _ = std::fs::create_dir_all(&dir);
    let fichero = tracing_appender::rolling::daily(dir, "pixpinmax.log");
    let (escritor, guardia) = tracing_appender::non_blocking(fichero);
    tracing_subscriber::fmt()
        .with_writer(escritor)
        .with_ansi(false)
        .init();
    guardia
}

#[cfg(test)]
mod pruebas_bandeja {
    use super::*;

    #[test]
    fn lecciones_galeria_y_tareas_salen_justo_debajo_del_chat() {
        let v = acciones_de_bandeja(|clave| clave.to_string());
        let chat = v
            .iter()
            .position(|(id, _)| *id == comandos::Comando::AbrirChat.id())
            .expect("el chat esta en la bandeja");
        let siguen: Vec<u32> = v[chat + 1..chat + 5].iter().map(|(id, _)| *id).collect();
        assert_eq!(
            siguen,
            [
                ID_LECCIONES,
                ID_LECCION_NUEVA,
                ID_GALERIA_CAPTURAS,
                ID_TAREAS
            ]
        );
    }

    #[test]
    fn sincronizar_sale_bajo_el_chat_y_recibir_suelto_no() {
        let v = acciones_de_bandeja(|clave| clave.to_string());
        let chat = v
            .iter()
            .position(|(id, _)| *id == comandos::Comando::AbrirChat.id())
            .expect("el chat esta en la bandeja");
        // Chat, lecciones, galeria y tareas, y en seguida Sincronizar.
        assert_eq!(
            v.get(chat + 5),
            Some(&(
                comandos::Comando::Sincronizar.id(),
                "comando-sincronizar".to_string()
            ))
        );
        assert_eq!(
            v.iter()
                .filter(|(id, _)| *id == comandos::Comando::Sincronizar.id())
                .count(),
            1,
            "una sola vez, aunque en el catalogo este al final"
        );
        assert!(
            !v.iter()
                .any(|(id, _)| *id == comandos::Comando::RecibirDelMovil.id()),
            "recibir cuelga ahora de Sincronizar, como en el movil"
        );
    }

    #[test]
    fn cada_entrada_sale_una_sola_vez_y_salir_no_se_cuela() {
        let v = acciones_de_bandeja(|clave| clave.to_string());
        for id in [
            ID_LECCIONES,
            ID_LECCION_NUEVA,
            ID_GALERIA_CAPTURAS,
            ID_TAREAS,
        ] {
            assert_eq!(v.iter().filter(|(i, _)| *i == id).count(), 1, "{id}");
        }
        // Caso negativo: el universo (901) ya no esta.
        assert!(!v.iter().any(|(i, _)| *i == 901));
        assert!(!v.iter().any(|(id, _)| *id == comandos::Comando::Salir.id()));
    }
}
