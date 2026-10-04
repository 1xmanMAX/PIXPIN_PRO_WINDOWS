//! El visor de HTML de PixPin: puerto de `ui/VisorHtmlActivity.kt` de
//! Android.
//!
//! Abre un `.html` del chat DENTRO de PixPin, a pantalla completa y sin
//! cromo —ni barra de direcciones, ni pestanas, ni botones—, y sale con
//! Escape. El usuario lo pidio asi: «implementar el nuestro pero sin bordes,
//! uno ligero y facil de usar».
//!
//! # Lo que resuelve
//!
//! Estos HTML los genera PixPin y traen su propio boton de guardar. Abiertos
//! en un navegador, ese boton DESCARGA una copia a la carpeta de descargas y
//! el fichero del chat se queda como estaba: «no quiero que se descargue
//! sino que se guarde directamente en el mismo chat, no como nuevo sino como
//! el mismo archivo».
//!
//! Aqui se guarda encima del mismo fichero, por dos caminos que trae
//! `pixpin-web`: el puente `PixPinVisor.guardar(nombre, base64)` —el mismo
//! nombre que en Android, asi que las paginas ya saben llamarlo— y, para las
//! que no lo conocen, la descarga interceptada.
//!
//! Este modulo no lleva `unsafe`: la ventana y el WebView2 los manejan
//! `pixpin-shell` y `pixpin-web`.

#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use pixpin_geom::Rect;
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Catalogo;
use std::path::Path;
use std::rc::Rc;

const VK_ESCAPE: u32 = 0x1B;

/// Si este fichero se abre con el visor propio.
pub fn se_abre(nombre: &str) -> bool {
    Path::new(nombre)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
}

/// Abre `ruta` en el visor y no vuelve hasta que se cierra.
///
/// Devuelve si el fichero se guardo alguna vez: quien llama lo usa para
/// releer el chat, que asi ensena lo nuevo sin que el usuario toque nada.
///
/// Sin el runtime de WebView2 no hay visor: devuelve error y quien llama
/// abre el html con el navegador, como hasta ahora. No se instala nada a
/// espaldas del usuario.
pub fn abrir(textos: &Catalogo, ruta: &Path) -> Result<bool> {
    if !pixpin_web::VisorHtml::hay_runtime() {
        anyhow::bail!("este equipo no tiene el runtime de WebView2");
    }
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .principal()
        .context("sin monitor principal")?
        .to_owned();
    // El area de TRABAJO y no el monitor entero: sin barra de titulo propia,
    // la barra de tareas es lo unico que queda para cambiar de ventana.
    let marco: Rect = monitor.area_trabajo;

    // Llana y no «normal»: la normal lleva `WS_EX_NOREDIRECTIONBITMAP`, que
    // es lo que quiere Direct2D y lo que ESTORBA a un WebView2 metido dentro.
    // Con ella el desplazamiento de la pagina iba a tirones.
    let ventana = VentanaOverlay::nueva_llana(marco, &textos.t("visor-html-titulo"))
        .context("no se pudo abrir la ventana del visor de HTML")?;
    ventana.mostrar();
    ventana.enfocar();

    // Lo guardado viaja del aviso del WebView2 a este bucle por una celda
    // compartida: los dos corren en el MISMO hilo (el WebView2 entrega sus
    // avisos en el bucle de mensajes), asi que basta con `Rc<RefCell<…>>`.
    let recogido: Rc<std::cell::RefCell<Vec<Vec<u8>>>> =
        Rc::new(std::cell::RefCell::new(Vec::new()));
    let buzon = Rc::clone(&recogido);
    // Escape pulsado DENTRO de la pagina: el WebView2 se queda el teclado, y
    // sin este aviso el visor solo se cerraria con Alt+F4.
    let cerrado = Rc::new(std::cell::Cell::new(false));
    let aviso_de_cierre = Rc::clone(&cerrado);
    let visor = pixpin_web::VisorHtml::nuevo(
        ventana.handle(),
        Rect {
            x: 0,
            y: 0,
            ..marco
        },
        ruta,
        Box::new(move |bytes| buzon.borrow_mut().push(bytes)),
        Box::new(move || aviso_de_cierre.set(true)),
    )
    .map_err(|e| anyhow::anyhow!("no se pudo abrir el visor de HTML: {e}"))?;

    let mut guardados = 0usize;
    let mut vivo = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        if cerrado.get() {
            vivo = false;
        }
        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match evento {
                // Escape cierra, como en el visor de documentos. Mientras se
                // escribe DENTRO de la pagina el teclado es del WebView2 y
                // Escape no llega aqui; entonces se cierra con la X de la
                // barra de tareas o con Alt+F4.
                EventoOverlay::Tecla { vk, .. } if vk == VK_ESCAPE => vivo = false,
                EventoOverlay::Cerrar => vivo = false,
                // La ventana nace del tamano del area de trabajo y no se
                // redimensiona: no hay evento de tamano en `EventoOverlay`, y
                // un visor a pantalla completa no lo necesita.
                _ => {}
            }
        }
        // Lo que la pagina haya mandado guardar, al fichero del chat. Se
        // escribe aqui y no dentro del aviso del WebView2 para no tocar el
        // disco desde su reentrada.
        let pendientes: Vec<Vec<u8>> = recogido.borrow_mut().drain(..).collect();
        for bytes in pendientes {
            match escribir_encima(ruta, &bytes) {
                Ok(()) => {
                    guardados += 1;
                    tracing::info!(
                        ruta = %ruta.display(),
                        bytes = bytes.len(),
                        "html guardado desde el visor"
                    );
                }
                Err(e) => tracing::warn!(?e, "no se pudo guardar el html"),
            }
        }
        // A dormir hasta que llegue algo, en vez de un `sleep` fijo: con 8 ms
        // de siesta cada vuelta, cada movimiento de rueda y cada pintado del
        // WebView2 esperaban su turno y la pagina se movia a tirones. Asi
        // vuelve en cuanto hay un mensaje, y en reposo no cuesta CPU. El tope
        // es la red para recoger lo que la pagina mande guardar.
        pixpin_shell::overlay::esperar_eventos(Some(100));
    }
    drop(visor);
    Ok(guardados > 0)
}

/// Escribe `bytes` sobre `ruta` sin dejarla a medias.
///
/// Fichero temporal y renombrado, como Android (`VisorHtmlActivity.kt`
/// escribe `.tmp` y hace `renameTo`): un corte a mitad de la escritura
/// dejaria el html del usuario partido, y eso es trabajo suyo.
fn escribir_encima(ruta: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temporal = ruta.with_extension("pixpin-tmp");
    std::fs::write(&temporal, bytes)?;
    // En Windows `rename` falla si el destino existe: se quita antes. Entre
    // las dos llamadas el fichero no esta, pero el contenido bueno ya esta
    // escrito en el temporal y no se pierde nada.
    let _ = std::fs::remove_file(ruta);
    std::fs::rename(&temporal, ruta)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn solo_abre_htmls() {
        assert!(se_abre("informe.html"));
        assert!(se_abre("INFORME.HTM"));
        // Caso negativo: lo demas sigue abriendose con su aplicacion.
        assert!(!se_abre("plano.pdf"));
        assert!(!se_abre("html"));
        assert!(!se_abre("pagina.html.txt"));
    }

    #[test]
    fn guardar_deja_el_fichero_entero_y_no_deja_restos() {
        let dir = std::env::temp_dir().join(format!("pixpin-visor-html-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let ruta = dir.join("pagina.html");
        std::fs::write(&ruta, b"<html>viejo</html>").unwrap();

        escribir_encima(&ruta, b"<html>nuevo y mas largo</html>").unwrap();
        assert_eq!(
            std::fs::read(&ruta).unwrap(),
            b"<html>nuevo y mas largo</html>"
        );
        // El temporal no se queda por ahi: si se quedara, la carpeta del
        // proyecto acabaria llena de `.pixpin-tmp`.
        assert!(!ruta.with_extension("pixpin-tmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
