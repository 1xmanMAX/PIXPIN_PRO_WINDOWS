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
mod overlay;
mod pines;
mod scroll;

use anyhow::{Context, Result};
use overlay::{AccionFinal, ModoConfirmacion, Recursos, TextosBarra, ejecutar_overlay};
use pines::Pines;
use pixpin_nivel::{Nivel, Preferencia};
use pixpin_shell::{
    Bandeja, BotonGesto, Continuar, EtiquetasMenu, Evento, VentanaMensajes,
    adquirir_instancia_unica, arranque, atajos, entorno,
};
use pixpin_store::ajustes::PreferenciaNivel;
use pixpin_store::{Almacen, Catalogo, Ubicacion, ajustes, idioma, rutas};
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
    let config = ajustes::cargar(&ubicacion).context("no se pudieron leer los ajustes")?;

    // 5. Reflejar en el registro de Windows lo que digan los ajustes.
    //
    // Se aplica en cada arranque y no solo al cambiar la casilla porque el
    // usuario puede haber editado el TOML a mano, o haber copiado su fichero
    // de ajustes a otro equipo. Asi el estado real y el declarado no divergen.
    let ruta_exe = dir_exe.join("pixpinmax.exe");
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
    let bandeja = Bandeja::nueva(ventana.handle(), &textos.t("app-nombre"))
        .context("no se pudo añadir el icono de bandeja")?;

    // Tres funciones no tienen atajo salvo que el TOML se lo de (D81): la
    // region con barra (va por la bandeja o por Alt + boton), el
    // cuentagotas y la anotacion congelada.
    let peticiones: Vec<(u32, pixpin_shell::Atajo)> = [
        (atajos::ID_REGION, config.atajos.region),
        (atajos::ID_COPIAR, Some(config.atajos.copiar)),
        (atajos::ID_SCROLL, Some(config.atajos.scroll)),
        (atajos::ID_CUENTAGOTAS, config.atajos.cuentagotas),
        (atajos::ID_PIN, Some(config.atajos.pin)),
        (atajos::ID_PORTAPAPELES, Some(config.atajos.portapapeles)),
        (atajos::ID_ANOTAR, Some(config.atajos.anotar)),
        (atajos::ID_ANOTAR_CONGELADA, config.atajos.anotar_congelada),
    ]
    .into_iter()
    .filter_map(|(id, atajo)| atajo.map(|a| (id, a)))
    .collect();
    let (_registrados, fallidos) = atajos::registrar(ventana.handle(), &peticiones);
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

    let etiquetas_base = |ocultos: Vec<(u32, String)>| EtiquetasMenu {
        capturar: textos.t("bandeja-capturar"),
        ajustes: textos.t("bandeja-ajustes"),
        salir: textos.t("bandeja-salir"),
        grupos_ocultos: textos.t("grupos-ocultos"),
        ocultos,
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

    ventana.ejecutar(|evento| {
        // Todo lo que abre el overlay de captura, en un sitio: los atajos,
        // «Capturar» de la bandeja y los gestos con Alt (D81). El gesto
        // trae el punto donde ya esta pulsado el boton: el overlay arranca
        // con el arrastre en marcha desde ahi.
        let modo_overlay: Option<(ModoConfirmacion, Option<pixpin_geom::Punto>)> = match evento {
            Evento::Atajo(id) if id == atajos::ID_REGION => {
                Some((ModoConfirmacion::ConBarra, None))
            }
            Evento::Atajo(id) if id == atajos::ID_PIN => Some((ModoConfirmacion::Pinear, None)),
            Evento::Atajo(id) if id == atajos::ID_SCROLL => Some((ModoConfirmacion::Scroll, None)),
            Evento::Atajo(id) if id == atajos::ID_CUENTAGOTAS => {
                Some((ModoConfirmacion::Cuentagotas, None))
            }
            Evento::Atajo(id) if id == atajos::ID_COPIAR => {
                Some((ModoConfirmacion::DirectoAlPortapapeles, None))
            }
            Evento::MenuCapturar => Some((ModoConfirmacion::ConBarra, None)),
            Evento::Gesto {
                boton: BotonGesto::Izquierdo,
                punto,
            } => Some((ModoConfirmacion::DirectoAlPortapapeles, Some(punto))),
            Evento::Gesto {
                boton: BotonGesto::Derecho,
                punto,
            } => Some((ModoConfirmacion::Pinear, Some(punto))),
            _ => None,
        };
        let seguir = match evento {
            Evento::MenuSalir => {
                tracing::info!("salida pedida por el usuario");
                Continuar::No
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
                tracing::info!(?modo, ?inicio, "abrir captura");
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
                        p.pinear(&imagen, region, escala_del_monitor(region))?;
                        tracing::info!(abiertos = p.abiertos(), "pin creado");
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
                            p.pinear_imagen_centrada(&imagen, &m)
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
            Evento::Atajo(id) => {
                tracing::info!(id, "atajo pulsado, todavia sin accion");
                Continuar::Si
            }
            // Ya atendidos por la guarda `modo_overlay` de arriba; quedan
            // solo para que el match sea exhaustivo.
            Evento::MenuCapturar | Evento::Gesto { .. } => Continuar::Si,
            Evento::MenuAjustes => {
                tracing::info!("ajustes pedidos desde el menu");
                Continuar::Si
            }
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
        AccionFinal::Copiar(imagen) => {
            pixpin_codec::copiar_imagen(&imagen).context("no se pudo copiar al portapapeles")?;
            Ok(None)
        }
        AccionFinal::Guardar(imagen) => {
            let ruta = ruta_captura_libre(ubicacion)?;
            pixpin_codec::guardar(&imagen, &ruta, pixpin_codec::FormatoImagen::Png)?;
            Ok(Some(ruta))
        }
        AccionFinal::Pinear { .. } | AccionFinal::Scroll { .. } => {
            // El bucle intercepta Pinear y Scroll antes de llamar aqui (el
            // gestor vive alli); llegar seria un error de cableado.
            tracing::warn!("Pinear o Scroll llego a ejecutar_accion; se ignora");
            Ok(None)
        }
        AccionFinal::GuardarComo(imagen) => {
            match pixpin_shell::guardar::pedir_ruta_guardado(hwnd, "captura.png") {
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
