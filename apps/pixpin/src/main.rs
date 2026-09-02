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

mod overlay;

use anyhow::{Context, Result};
use overlay::{AccionFinal, ModoConfirmacion, Recursos, TextosBarra, ejecutar_overlay};
use pixpin_nivel::{Nivel, Preferencia};
use pixpin_shell::{
    Bandeja, Continuar, EtiquetasMenu, Evento, VentanaMensajes, adquirir_instancia_unica, arranque,
    atajos, entorno,
};
use pixpin_store::ajustes::PreferenciaNivel;
use pixpin_store::{Catalogo, Ubicacion, ajustes, idioma, rutas};
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
    tracing::info!(?hechos, ?decision, "nivel de rendimiento decidido");

    // 6. Idioma, antes de crear nada con texto.
    let lengua = idioma::resolver_idioma(&entorno::locale_del_sistema(), config.idioma);
    let textos = Catalogo::nuevo(lengua);

    // 7. Ventana invisible, icono de bandeja y atajos.
    let ventana = VentanaMensajes::nueva().context("no se pudo crear la ventana de mensajes")?;
    let bandeja = Bandeja::nueva(ventana.handle(), &textos.t("app-nombre"))
        .context("no se pudo añadir el icono de bandeja")?;

    let peticiones = [
        (atajos::ID_REGION, config.atajos.region),
        (atajos::ID_COPIAR, config.atajos.copiar),
        (atajos::ID_SCROLL, config.atajos.scroll),
        (atajos::ID_CUENTAGOTAS, config.atajos.cuentagotas),
        (atajos::ID_PIN, config.atajos.pin),
    ];
    let (_registrados, fallidos) = atajos::registrar(ventana.handle(), &peticiones);
    for (id, atajo) in &fallidos {
        // Se registra el problema pero no se aborta: otra aplicacion puede
        // tener ese atajo y el resto de PixPin Max sigue siendo util.
        tracing::warn!(id, %atajo, "no se pudo registrar el atajo; otra aplicacion lo tiene");
    }

    let etiquetas = EtiquetasMenu {
        capturar: textos.t("bandeja-capturar"),
        ajustes: textos.t("bandeja-ajustes"),
        salir: textos.t("bandeja-salir"),
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
    let hwnd = ventana.handle();
    ventana.ejecutar(|evento| match evento {
        Evento::MenuSalir => {
            tracing::info!("salida pedida por el usuario");
            Continuar::No
        }
        Evento::IconoPulsado => {
            if let Err(e) = bandeja.mostrar_menu(hwnd, &etiquetas) {
                tracing::warn!(?e, "no se pudo mostrar el menu de bandeja");
            }
            Continuar::Si
        }
        Evento::Atajo(id)
            if id == atajos::ID_REGION || id == atajos::ID_COPIAR || id == atajos::ID_PIN =>
        {
            let modo = if id == atajos::ID_REGION {
                ModoConfirmacion::ConBarra
            } else if id == atajos::ID_PIN {
                ModoConfirmacion::Pinear
            } else {
                ModoConfirmacion::DirectoAlPortapapeles
            };
            let etiquetas_barra = TextosBarra {
                copiar: textos.t("barra-copiar"),
                guardar: textos.t("barra-guardar"),
                guardar_como: textos.t("barra-guardar-como"),
                descartar: textos.t("barra-descartar"),
            };
            let formato = match config.formato_color {
                ajustes::FormatoColor::Hex => FormatoColorLupa::Hex,
                ajustes::FormatoColor::Rgb => FormatoColorLupa::Rgb,
                ajustes::FormatoColor::Hsl => FormatoColorLupa::Hsl,
            };
            let listo = match &mut recursos_overlay {
                Some(r) => Ok(r),
                nada => Recursos::nuevos().map(|r| nada.insert(r)),
            };
            match listo
                .and_then(|r| ejecutar_overlay(r, decision.nivel, modo, &etiquetas_barra, formato))
                .and_then(|accion| ejecutar_accion(accion, &ubicacion, hwnd))
            {
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
        Evento::Atajo(id) => {
            tracing::info!(id, "atajo pulsado, todavia sin accion");
            Continuar::Si
        }
        Evento::MenuCapturar => {
            tracing::info!("capturar pedido desde el menu");
            Continuar::Si
        }
        Evento::MenuAjustes => {
            tracing::info!("ajustes pedidos desde el menu");
            Continuar::Si
        }
    });

    tracing::info!("PixPin Max terminado limpiamente");
    Ok(())
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
        AccionFinal::Pinear { imagen, region } => {
            // El gestor de pines (tarea siguiente del plan) consume esta
            // accion antes de llegar aqui; este brazo existe para que el
            // commit del atajo compile y funcione solo.
            tracing::info!(
                ancho = imagen.ancho,
                x = region.x,
                y = region.y,
                "pinear pendiente del gestor de pines"
            );
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
