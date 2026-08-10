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

use anyhow::{Context, Result};
use pixpin_shell::{
    Bandeja, Continuar, EtiquetasMenu, Evento, VentanaMensajes, adquirir_instancia_unica, arranque,
    atajos, entorno,
};
use pixpin_store::{Catalogo, Ubicacion, ajustes, idioma, rutas};

fn main() -> Result<()> {
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

    // 8. A dormir hasta que pase algo.
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
        Evento::Atajo(id) => {
            // S1-B conecta esto con la captura. Por ahora solo se registra,
            // que ya permite comprobar de verdad que los atajos funcionan.
            tracing::info!(id, "atajo pulsado");
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
