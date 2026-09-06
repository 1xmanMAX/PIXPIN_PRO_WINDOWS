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

mod caja_dibujo;
mod capa;
mod cuenta_atras;
mod editor;
mod gif;
mod grabador;
mod overlay;
mod pines;
mod reproductor;
mod scroll;
mod ventana_ajustes;

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

    // 4. Que nos han configurado.
    let mut config = ajustes::cargar(&ubicacion).context("no se pudieron leer los ajustes")?;

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

    // 6. Idioma, antes de crear nada con texto.
    let lengua = idioma::resolver_idioma(&entorno::locale_del_sistema(), config.idioma);
    let textos = Catalogo::nuevo(lengua);

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
            acciones: comandos::CATALOGO
                .iter()
                // «Salir» sale aparte, al final y tras una raya: no debe
                // pulsarse por inercia al buscar otra cosa.
                .filter(|d| d.en_bandeja && d.comando != comandos::Comando::Salir)
                .map(entrada)
                .collect(),
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

    ventana.ejecutar(|evento| {
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
            _ => match comando {
                Some(comandos::Comando::CapturarRegion) => Some((ModoConfirmacion::ConBarra, None)),
                // El retardo abre la MISMA captura; lo unico distinto es
                // que antes se cuenta. La cuenta corre mas abajo, ya con
                // los recursos a mano.
                Some(comandos::Comando::CapturarConRetardo) => {
                    Some((ModoConfirmacion::ConBarra, None))
                }
                Some(comandos::Comando::Pinear) => Some((ModoConfirmacion::Pinear, None)),
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
                            tracing::info!(pasantes, cuantos, "paso de clics de los pines");
                        }
                        _ => match pixpin_capture::enumerar_monitores() {
                            Err(e) => tracing::warn!(?e, "sin monitores"),
                            Ok(d) if comando == Some(comandos::Comando::AlternarPines) => {
                                let (ocultados, cuantos) = p.alternar_todos(&d);
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
                            pixpin_codec::copiar_imagen(&imagen)
                                .context("no se pudo copiar la captura")
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
                    pixpin_codec::copiar_imagen(&imagen).context("no se pudo copiar")
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
                // Cada ruta cae en el pin que le toque por su extension:
                // imagen, video o ficha de archivo. Eso ya lo decide el
                // gestor, que es quien conoce los tipos.
                // Un `.pixpin` no es un fichero que pinear: es un proyecto
                // entero, y cada hoja suya sale como su propio pin.
                let (proyectos, sueltos): (Vec<_>, Vec<_>) = rutas.into_iter().partition(|r| {
                    r.extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("pixpin"))
                });
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
                    let mut cuantos = 0;
                    for proyecto in &proyectos {
                        // Un proyecto que falle no puede llevarse los
                        // demas ficheros que venian con el.
                        match p.abrir_paquete(proyecto, &m) {
                            Ok((hechas, _)) => cuantos += hechas,
                            Err(e) => tracing::warn!(?e, ruta = %proyecto.display(), "proyecto que no se pudo abrir"),
                        }
                    }
                    if !sueltos.is_empty() {
                        cuantos += pinear_portapapeles(
                            p,
                            pixpin_codec::ContenidoPortapapeles::Rutas(sueltos),
                        )?;
                    }
                    Ok(cuantos)
                });
                match hecho {
                    Ok(cuantos) => tracing::info!(cuantos, "ficheros abiertos como pines"),
                    Err(e) => tracing::warn!(?e, "no se pudieron abrir los ficheros"),
                }
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
                }
                let anotar_al_pinear = comando == Some(comandos::Comando::CapturarYAnotar);
                tracing::info!(?modo, ?inicio, anotar_al_pinear, "abrir captura");
                let etiquetas_barra = TextosBarra {
                    copiar: textos.t("barra-copiar"),
                    guardar: textos.t("barra-guardar"),
                    guardar_como: textos.t("barra-guardar-como"),
                    descartar: textos.t("barra-descartar"),
                    todo: textos.t("barra-todo"),
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
                    ejecutar_overlay(r, decision.nivel, modo, &etiquetas_barra, formato, inicio)
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
            Evento::Atajo(id) if id == atajos::ID_ANOTAR || id == atajos::ID_ANOTAR_CONGELADA => {
                let modo = if id == atajos::ID_ANOTAR {
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
                let capa_hecha = listo.and_then(|r| capa::ejecutar_capa(r, modo, decision.nivel));
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
        seguir
    });

    tracing::info!("PixPin Max terminado limpiamente");
    Ok(())
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
fn textos_del_pin(textos: &Catalogo) -> pixpin_pin::TextosPin {
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
        pagina_siguiente: textos.t("pin-pagina-siguiente"),
        pagina_anterior: textos.t("pin-pagina-anterior"),
        extraer_pagina: textos.t("pin-extraer-pagina"),
        extraer_todas: textos.t("pin-extraer-todas"),
        ocultar_grupo: textos.t("pin-ocultar-grupo"),
        cerrar: textos.t("pin-cerrar"),
        eliminar: textos.t("pin-eliminar"),
        no_encontrado: textos.t("pin-no-encontrado"),
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
            pines.pinear_nota(&t, &monitor)?;
            Ok(1)
        }
        C::Rutas(rutas) => {
            let mut hechas = 0;
            for r in rutas {
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
        AccionFinal::Copiar(imagen) => {
            pixpin_codec::copiar_imagen(&imagen).context("no se pudo copiar al portapapeles")?;
            Ok(None)
        }
        AccionFinal::Guardar(imagen) => {
            let ruta = ruta_captura_libre(ubicacion)?;
            pixpin_codec::guardar(&imagen, &ruta, pixpin_codec::FormatoImagen::Png)?;
            Ok(Some(ruta))
        }
        AccionFinal::Pinear { .. } | AccionFinal::Scroll { .. } | AccionFinal::Gif { .. } => {
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
