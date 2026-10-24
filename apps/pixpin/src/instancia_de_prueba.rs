//! **Una PixPin de prueba, aparte de la del usuario** (solo para
//! desarrollar).
//!
//! ```text
//! set PIXPIN_INSTANCIA=paginas
//! pixpinmax.exe "C:\ruta\documento.docx" "C:\ruta\charla.pptx"
//! ```
//!
//! Con `PIXPIN_INSTANCIA` puesta, esta copia **no** le pasa los ficheros a la
//! PixPin que ya corre: coge su propia instancia unica (otro nombre de mutex,
//! `Local\PixPinMax-instancia-prueba-<nombre>`) y sus propios datos (en
//! `%TEMP%\pixpin-instancia-<nombre>`, en modo portable: nada al registro de
//! Windows), y abre los ficheros de la linea de mandatos en sus lectores por
//! el mismo camino que «Abrir con PixPin» (`lector::abrir_en_su_lector`).
//!
//! Y nada mas, a proposito: ni bandeja, ni atajos globales, ni el gancho del
//! raton, ni el del espacio en el Explorador, ni sincronizar, ni el chat. Dos
//! PixPin con ganchos globales a la vez contestarian las dos a cada gesto del
//! usuario. Se cierra sola cuando se cierra la ultima de sus ventanas.
//!
//! Sin la variable (lo normal), nada de esto existe: el arranque es el de
//! siempre.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_store::{Ubicacion, idioma};

/// El nombre de la instancia de prueba, si se pidio.
pub fn pedida() -> Option<String> {
    std::env::var("PIXPIN_INSTANCIA")
        .ok()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
}

/// Arranca la PixPin de prueba y vuelve cuando se cierran sus ventanas.
pub fn arrancar(
    nombre: &str,
    guardia_registro: &mut Option<tracing_appender::non_blocking::WorkerGuard>,
) -> Result<()> {
    let _instancia = pixpin_shell::instancia::adquirir_instancia_de_prueba(nombre)
        .context("ya corre una PixPin de prueba con ese nombre")?;
    let limpio: String = nombre
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let raiz = std::env::temp_dir().join(format!("pixpin-instancia-{limpio}"));
    std::fs::create_dir_all(&raiz).context("sin carpeta para la PixPin de prueba")?;
    let ubicacion = Ubicacion::Portable { raiz };
    *guardia_registro = Some(crate::iniciar_registro(&ubicacion));
    // La luna/sol de los visores, como en la app (`estado.toml` de esta copia).
    crate::oscuro::iniciar(&ubicacion);
    let lengua = idioma::resolver_idioma(
        &pixpin_shell::entorno::locale_del_sistema(),
        Default::default(),
    );
    let rutas = pixpin_shell::mensajero::rutas_de_los_argumentos();
    tracing::info!(nombre, raiz = ?ubicacion.raiz(), ficheros = rutas.len(), "PixPin de prueba (PIXPIN_INSTANCIA)");
    for ruta in &rutas {
        // Un lienzo (`.excalidraw`): el editor, en su hilo, como desde el chat.
        if ruta
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("excalidraw"))
        {
            let ruta = ruta.clone();
            std::thread::spawn(move || {
                let _com = pixpin_shell::ComDelHilo::iniciar();
                let Some(lienzo) = std::fs::read_to_string(&ruta)
                    .ok()
                    .and_then(|j| pixpin_motor2d::excalidraw::leer(&j).ok())
                else {
                    tracing::warn!(ruta = %ruta.display(), "PixPin de prueba: lienzo ilegible");
                    return;
                };
                let escena = pixpin_motor2d::excalidraw::a_escena(&lienzo);
                let _ = crate::ventana_editor::abrir(
                    escena,
                    Default::default(),
                    pixpin_nivel::Nivel::Completo,
                    false,
                    None,
                    &[],
                );
            });
            continue;
        }
        // Un Office viejo: el pin de Office (y de ahi, convertido, el visor
        // propio), como en la app.
        if crate::pin_office::se_abre(ruta) {
            crate::pin_office::lanzar(lengua, ubicacion.clone(), ruta);
            continue;
        }
        if !crate::lector::abrir_en_su_lector(lengua, &ubicacion, ruta, &pixpin_docs::nombre(ruta))
        {
            tracing::warn!(ruta = %ruta.display(), "PixPin de prueba: no hay lector para esto");
        }
    }
    // Vive mientras tenga ventanas (y espera un poco a que salga la primera).
    // Sin ventanas un rato seguido: una ventana que se rehace (dispositivo
    // grafico perdido) desaparece un momento y no cuenta.
    let mut sin_ventanas = Instant::now();
    let mut vista = false;
    loop {
        std::thread::sleep(Duration::from_millis(300));
        let vivas = !pixpin_shell::espacio_explorador::ventanas_propias().is_empty();
        vista |= vivas;
        if vivas {
            sin_ventanas = Instant::now();
        }
        let espera = if vista {
            Duration::from_secs(5)
        } else {
            Duration::from_secs(60)
        };
        if sin_ventanas.elapsed() > espera {
            break;
        }
    }
    tracing::info!("PixPin de prueba: sin ventanas, se acaba");
    Ok(())
}
