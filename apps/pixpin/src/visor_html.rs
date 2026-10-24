//! El visor de HTML de PixPin: puerto de `ui/VisorHtmlActivity.kt` de
//! Android.
//!
//! Abre un `.html` del chat DENTRO de PixPin, sin cromo de navegador —ni
//! barra de direcciones, ni pestanas—. El usuario lo pidio asi: «implementar
//! el nuestro pero sin bordes, uno ligero y facil de usar».
//!
//! **Como un pin, con la barra de los demas visores** (17-oct-2026, el
//! usuario: «modificar el visor de MD y de HTML para que tenga una UI igual a
//! la de los demas visores en formato de pin … de forma que la UI sea
//! unificada entre todos los visores»). Antes ocupaba el area de trabajo
//! entera, sin barra, y solo se cerraba con Escape; ahora es la ventana de
//! los lectores como pin (`lector_pin`), la misma del PDF, el Word, el Excel
//! o el pin de Office: del tamano de un pin, se estira por los bordes, la
//! barra va fuera con el asa, el nombre con el recuadro del tipo, la
//! luna/sol, pantalla completa (F11; Esc vuelve), el pin y cerrar. Lo propio
//! de una pagina va en sus botones ([`botones`]): **recargar**, **exportar**
//! (el componente comun) y **abrir en el navegador**. El nombre, como en el
//! pin de Office, es «Abrir con…».
//!
//! Va en su propio hilo ([`lanzar`]): antes corria en el del chat y lo
//! dejaba quieto mientras la pagina estaba abierta.
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

pub mod capa_web;
pub mod riel;

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use anyhow::{Context, Result};
use pixpin_render::icono::material;
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Catalogo;

use crate::lector_pin::{AccionPin, BotonLector, PinLector};

/// Si este fichero se abre con el visor propio.
pub fn se_abre(nombre: &str) -> bool {
    Path::new(nombre)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
}

/// Los botones propios de la barra.
pub const RECARGAR: u8 = 0;
pub const EXPORTAR: u8 = 1;
pub const NAVEGADOR: u8 = 2;

/// **Lo que lleva la barra de una pagina**, entre el nombre y la luna/sol:
/// recargar (lo que otro programa cambio en el fichero), exportar (HTML, PDF,
/// el original, `.pixpin`) y abrir en el navegador. Sin «Copiar»: Ctrl+C
/// copia dentro de la pagina.
pub fn botones() -> Vec<BotonLector> {
    vec![
        BotonLector {
            id: RECARGAR,
            icono: &material::REPLAY,
            encendido: false,
        },
        BotonLector {
            id: EXPORTAR,
            icono: &material::IOS_SHARE,
            encendido: false,
        },
        BotonLector {
            id: NAVEGADOR,
            icono: &material::PUBLIC,
            encendido: false,
        },
    ]
}

/// **El globo de cada boton propio**: su nombre y su tecla.
pub fn pistas(textos: &Catalogo) -> Vec<(u8, String)> {
    vec![
        (RECARGAR, textos.t("web-recargar")),
        (EXPORTAR, textos.t("web-exportar")),
        (NAVEGADOR, textos.t("web-navegador")),
    ]
}

/// El fondo mientras la pagina carga: el de una pagina sin estilo (blanco)
/// o, en oscuro, el de las de PixPin.
pub fn fondo(oscuro: bool) -> (u8, u8, u8) {
    if oscuro {
        (0x1e, 0x1e, 0x20)
    } else {
        (0xff, 0xff, 0xff)
    }
}

/// El titulo de la ventana (la barra de tareas, Alt+Tab).
pub fn titulo_de_ventana(nombre_fichero: &str) -> String {
    format!("{nombre_fichero} — PixPin")
}

/// **Abre `ruta` en el visor, en su propio hilo**: quien lo pide (el chat)
/// sigue a lo suyo. Sin el runtime de WebView2 no hay visor y se abre con el
/// navegador, como hasta ahora (no se instala nada a espaldas del usuario);
/// tambien si el visor no llega a abrirse. Si la pagina se guardo, el chat se
/// relee y ensena lo nuevo sin que el usuario toque nada.
pub fn lanzar(idioma: pixpin_store::Idioma, ruta: &Path) {
    if !pixpin_web::VisorHtml::hay_runtime() {
        tracing::info!("sin el runtime de WebView2: el html, al navegador");
        abrir_fuera(ruta);
        return;
    }
    let ruta = ruta.to_path_buf();
    let lanzado = std::thread::Builder::new()
        .name("visor-html".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            match abrir(&textos, &ruta) {
                Ok(true) => {
                    tracing::info!(ruta = %ruta.display(), "html guardado desde el visor");
                    crate::ventana_chat::refrescar();
                }
                Ok(false) => {}
                Err(e) => {
                    tracing::warn!(?e, "sin visor de HTML propio; se abre fuera");
                    abrir_fuera(&ruta);
                }
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del visor de HTML");
    }
}

/// Con el navegador (lo que Windows tenga para `.html`).
fn abrir_fuera(ruta: &Path) {
    if let Err(e) = pixpin_shell::abrir::abrir(ruta) {
        tracing::warn!(?e, ruta = %ruta.display(), "no se pudo abrir el html fuera");
    }
}

/// Abre `ruta` en el visor y no vuelve hasta que se cierra.
///
/// Devuelve si el fichero se guardo alguna vez. `Err` si no se pudo abrir
/// (quien llama lo abre en el navegador).
pub fn abrir(textos: &Catalogo, ruta: &Path) -> Result<bool> {
    let inicio = Instant::now();
    let nombre_fichero = pixpin_docs::nombre(ruta);
    let nombre = crate::lector_pin::nombre_limpio(&pixpin_docs::sin_extension(&nombre_fichero));
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .principal()
        .context("sin monitor principal")?
        .to_owned();
    let e = monitor.escala_por_cien as f32 / 100.0;
    let (area_pin, _) = crate::lector_pin::area_inicial();
    let mut pin =
        PinLector::nuevo(area_pin, e).con_extension(&pixpin_docs::extension(&nombre_fichero));
    // Llana y no «normal»: la normal lleva `WS_EX_NOREDIRECTIONBITMAP`, que
    // es lo que quiere Direct2D y lo que ESTORBA a un WebView2 metido dentro.
    // Con ella el desplazamiento de la pagina iba a tirones.
    let mut ventana = VentanaOverlay::nueva_llana(pin.area, &titulo_de_ventana(&nombre_fichero))
        .context("no se pudo abrir la ventana del visor de HTML")?;
    pin.al_nacer(&ventana);
    ventana.mostrar();
    ventana.enfocar();
    let hwnd = ventana.handle().0 as isize;
    pixpin_web::pintar_fondo(hwnd, fondo(pin.oscuro()));
    tracing::info!(
        ms = inicio.elapsed().as_millis() as u64,
        "visor de HTML: a la vista"
    );

    // La barra: sin GPU no la hay, pero la pagina se ve igual (y Esc cierra).
    let recursos = crate::overlay::Recursos::nuevos()
        .map_err(|e| tracing::warn!(?e, "visor de HTML sin barra"))
        .ok();
    let motor = recursos.as_ref().map(|r| r.motor());
    if let (Some(r), Some(m)) = (&recursos, &motor) {
        pin.crear_barra(m, &r.d3d(), &ventana);
    }
    let propios = botones();
    pin.poner_pistas(pistas(textos));
    // Pruebas (solo en debug): siempre encima, para fotografiar la pantalla
    // sin que otra ventana lo tape.
    #[cfg(debug_assertions)]
    if std::env::var_os("PIXPIN_PRUEBA_ENCIMA").is_some() {
        pin.alternar_fijado(&ventana);
    }
    if let Some(m) = &motor {
        pin.actualizar(m, &nombre, &propios);
    }

    // Lo guardado y las teclas viajan de los avisos del WebView2 a este
    // bucle por celdas compartidas: los dos corren en el MISMO hilo (el
    // WebView2 entrega sus avisos en el bucle de mensajes), asi que basta
    // con `Rc<RefCell<…>>`.
    let recogido: Rc<RefCell<Vec<Vec<u8>>>> = Rc::new(RefCell::new(Vec::new()));
    let buzon = Rc::clone(&recogido);
    // Esc y F11 pulsados DENTRO de la pagina: el WebView2 se queda el
    // teclado, y sin este aviso no llegarian.
    let pulsadas: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
    let buzon_teclas = Rc::clone(&pulsadas);
    let visor = pixpin_web::VisorHtml::nuevo(
        ventana.handle(),
        pin.area_de_la_vista(),
        ruta,
        Box::new(move |bytes| buzon.borrow_mut().push(bytes)),
        Box::new(move |vk| buzon_teclas.borrow_mut().push(vk)),
    )
    .map_err(|e| anyhow::anyhow!("no se pudo abrir el visor de HTML: {e}"))?;
    // El modo oscuro de los visores (luna/sol, `crate::oscuro`): el de la
    // barra, que tambien avisa si cambia con el abierto.
    visor.poner_oscuro(pin.oscuro());
    tracing::info!(
        ms = inicio.elapsed().as_millis() as u64,
        "visor de HTML: pagina pedida"
    );

    // **El riel de los visores** (el marcador, el lapiz y el marco), en sus
    // ventanas por encima de la pagina: las marcas y la tinta, junto al html.
    let escala = monitor.escala_por_cien;
    let mut riel = match (&recursos, &motor) {
        (Some(r), Some(m)) => riel::RielDeLaVista::nuevo(
            Rc::clone(m),
            &r.d3d(),
            &ventana,
            riel::en_pantalla(pin.area, pin.area_de_la_vista()),
            escala,
            riel::Opciones {
                documento: Some(ruta.to_path_buf()),
                con_pagina: true,
                titulo: nombre.clone(),
                menu: false,
                maqueta: false,
            },
            textos,
        ),
        _ => None,
    };

    let mut area = pin.area;
    let mut guardados = 0usize;
    let mut vivo = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        let mut teclas: Vec<u32> = pulsadas.borrow_mut().drain(..).collect();
        // Lo que mando la pagina al riel: por donde va al desplazarse y las
        // teclas de los visores (I, M, A, Ctrl+M) pulsadas en ella.
        for m in visor.tomar_mensajes() {
            if let Some(r) = riel.as_mut() {
                match r.mensaje(&m, Some(&visor)) {
                    Some(riel::Pide::Teclado) => visor.enfocar(),
                    Some(riel::Pide::Tecla(vk)) => teclas.push(vk),
                    _ => {}
                }
            }
        }
        for (h, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if let Some(r) = riel.as_mut()
                && r.es_suya(h)
            {
                match r.evento(h, &evento, Some(&visor), textos) {
                    riel::Pide::Teclado => visor.enfocar(),
                    riel::Pide::Tecla(vk) => teclas.push(vk),
                    riel::Pide::Nada => {}
                }
                continue;
            }
            if pin.es_de_la_barra(h) {
                let (accion, _) = pin.evento_barra(&evento, &mut ventana);
                match accion {
                    Some(AccionPin::Propio(RECARGAR)) => visor.recargar(),
                    Some(AccionPin::Propio(EXPORTAR)) => exportar(ruta),
                    // Se abre fuera y el pin sigue: es una mirada en el
                    // navegador, no un cambio de visor.
                    Some(AccionPin::Propio(NAVEGADOR)) => abrir_fuera(ruta),
                    Some(AccionPin::Propio(_)) => {}
                    // «Abrir con…», como el pin de Office: elegida la app,
                    // el pin se cierra.
                    Some(AccionPin::Nombre) => {
                        if crate::abrir_con::abrir_con_o_avisar(ventana.handle(), ruta, textos) {
                            vivo = false;
                        }
                    }
                    Some(AccionPin::PantallaCompleta) => {
                        pin.alternar_pantalla_completa(&mut ventana)
                    }
                    Some(AccionPin::Fijar) => pin.alternar_fijado(&ventana),
                    Some(AccionPin::Cerrar) => vivo = false,
                    None => {}
                }
                continue;
            }
            if h != ventana.handle() {
                continue;
            }
            if pin.evento_ventana(&evento, &mut ventana).consumido {
                continue;
            }
            match evento {
                EventoOverlay::Cerrar => vivo = false,
                // Ctrl+E: el «Exportar» comun, como el boton.
                EventoOverlay::Tecla {
                    vk: 0x45,
                    ctrl: true,
                    alt: false,
                    ..
                } => exportar(ruta),
                // Ctrl+P con el teclado en la ventana: los marcos.
                EventoOverlay::Tecla {
                    vk: 0x50,
                    ctrl: true,
                    alt: false,
                    ..
                } if riel.as_ref().is_some_and(|r| r.hay_marcos()) => {
                    if let Some(r) = riel.as_mut() {
                        r.imprimir_marcos(Some(&visor));
                    }
                }
                // El teclado lo tenia la ventana (se toco su borde): la
                // tecla cuenta y el siguiente va ya a la pagina. Las de los
                // visores (I, A, M, Ctrl+M), al riel.
                EventoOverlay::Tecla {
                    vk,
                    ctrl,
                    alt: false,
                    ..
                } if riel.as_ref().is_some() => {
                    match riel.as_mut().and_then(|r| r.tecla(vk, ctrl, Some(&visor))) {
                        Some(riel::Pide::Tecla(v)) => teclas.push(v),
                        Some(riel::Pide::Teclado) => visor.enfocar(),
                        Some(riel::Pide::Nada) => {}
                        None => {
                            teclas.push(vk);
                            visor.enfocar();
                        }
                    }
                }
                EventoOverlay::Tecla { vk, .. } => {
                    teclas.push(vk);
                    visor.enfocar();
                }
                EventoOverlay::Pintar | EventoOverlay::CambioDpi => {
                    pixpin_web::pintar_fondo(hwnd, fondo(pin.oscuro()))
                }
                _ => {}
            }
        }
        // Esc y F11, vengan de la ventana o de la pagina: como en los demas
        // visores (`lector_pin::tecla`).
        for vk in teclas {
            // Ctrl+P con marcos, dentro de la pagina: imprimirlos en grupo.
            if vk == pixpin_web::CTRL_P {
                if let Some(r) = riel.as_mut() {
                    r.imprimir_marcos(Some(&visor));
                }
                continue;
            }
            // Esc con la tira de los marcadores abierta: solo la cierra.
            if vk == 0x1B && riel.as_mut().is_some_and(|r| r.soltar_esc()) {
                continue;
            }
            match crate::lector_pin::tecla(vk, pin.pantalla_completa) {
                Some(true) => vivo = false,
                Some(false) => pin.alternar_pantalla_completa(&mut ventana),
                None => {}
            }
        }
        if !vivo {
            break;
        }
        // Movida o estirada desde fuera (AltSnap, Win+flecha): el pin la sigue.
        pin.seguir_ventana(&mut ventana);
        if pin.area != area {
            area = pin.area;
            visor.redimensionar(pin.area_de_la_vista());
            pixpin_web::pintar_fondo(hwnd, fondo(pin.oscuro()));
        }
        if let Some(m) = &motor
            && pin.actualizar(m, &nombre, &propios)
        {
            visor.poner_oscuro(pin.oscuro());
            pixpin_web::pintar_fondo(hwnd, fondo(pin.oscuro()));
        }
        if let Some(r) = riel.as_mut() {
            r.seguir(
                riel::en_pantalla(pin.area, pin.area_de_la_vista()),
                escala,
                pin.fijado,
                pin.oscuro(),
            );
            r.tic(Some(&visor));
            // Con marcos, Ctrl+P es suyo (y no la impresion de Edge).
            visor.tomar_ctrl_p(r.hay_marcos());
        }
        // Lo que la pagina haya mandado guardar, al fichero del chat. Se
        // escribe aqui y no dentro del aviso del WebView2 para no tocar el
        // disco desde su reentrada.
        let pendientes: Vec<Vec<u8>> = recogido.borrow_mut().drain(..).collect();
        for bytes in pendientes {
            match escribir_encima(ruta, &bytes) {
                Ok(()) => {
                    guardados += 1;
                    tracing::info!(ruta = %ruta.display(), bytes = bytes.len(), "html guardado desde el visor");
                }
                Err(e) => tracing::warn!(?e, "no se pudo guardar el html"),
            }
        }
        // A dormir hasta que llegue algo, en vez de un `sleep` fijo: con 8 ms
        // de siesta cada vuelta, cada movimiento de rueda y cada pintado del
        // WebView2 esperaban su turno y la pagina se movia a tirones. Asi
        // vuelve en cuanto hay un mensaje, y en reposo no cuesta CPU. El tope
        // es la red para recoger lo que la pagina mande guardar y para que
        // la barra se vaya sola.
        let tope = if pin.quiere_reloj() { 50 } else { 100 };
        let tope = riel
            .as_ref()
            .and_then(|r| r.tope_ms())
            .map_or(tope, |t| t.min(tope));
        pixpin_shell::overlay::esperar_eventos(Some(tope));
    }
    // Cerrar: se esconde ya (que se note al instante) y luego se suelta todo.
    if let Some(r) = riel.take() {
        r.cerrar();
    }
    drop(pin);
    ventana.ocultar();
    drop(visor);
    drop(ventana);
    Ok(guardados > 0)
}

/// **Exportar la pagina**: ella misma (HTML), su PDF impreso por Edge, el
/// original y el `.pixpin`, con el componente comun.
fn exportar(ruta: &Path) {
    let Some(u) = crate::oscuro::ubicacion() else {
        return;
    };
    let titulo = pixpin_docs::sin_extension(&pixpin_docs::nombre(ruta));
    let mut x = crate::compartir::exportable::Exportable::nuevo(titulo.clone());
    x.raiz = Some(u.raiz().to_path_buf());
    x.original = Some(ruta.to_path_buf());
    let base = crate::compartir::nombre_de_fichero(&titulo);
    let (r1, b1) = (ruta.to_path_buf(), base.clone());
    x.propios.push(crate::compartir::exportable::Propio::nuevo(
        crate::compartir::WEB,
        "compartir-web",
        move |c| {
            let r = c.join(format!("{b1}.html"));
            std::fs::copy(&r1, &r)?;
            Ok(vec![r])
        },
    ));
    let r2 = ruta.to_path_buf();
    x.propios.push(crate::compartir::exportable::Propio::nuevo(
        crate::compartir::PDF,
        "compartir-pdf",
        move |c| {
            let r = c.join(format!("{base}.pdf"));
            let html = std::fs::read_to_string(&r2)?;
            crate::timeline::exportar::a_pdf(&html, &r)
                .map_err(|e| anyhow::anyhow!("no se pudo imprimir el PDF: {e}"))?;
            Ok(vec![r])
        },
    ));
    crate::compartir::ventana::abrir(
        crate::compartir::idioma_de(&u),
        u,
        crate::compartir::Cosa::Visor(x),
    );
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
    fn la_barra_de_una_pagina_es_la_comun_con_recargar_exportar_y_navegador() {
        let ids: Vec<u8> = botones().iter().map(|b| b.id).collect();
        assert_eq!(ids, vec![RECARGAR, EXPORTAR, NAVEGADOR]);
        // La disposicion entera: la de todos los visores, con luna/sol.
        assert_eq!(
            crate::lector_pin::disposicion("informe", "html", &botones(), true),
            [
                "asa", "nombre", "propio:0", "propio:1", "propio:2", "oscuro", "pantalla", "fijar",
                "cerrar"
            ]
        );
        // Casos negativos: nada encendido al abrir, y ni editar ni copiar
        // (la pagina se edita y se copia dentro).
        assert!(botones().iter().all(|b| !b.encendido));
        assert!(
            !botones()
                .iter()
                .any(|b| std::ptr::eq(b.icono, &material::EDIT)
                    || std::ptr::eq(b.icono, &material::CONTENT_COPY))
        );
    }

    #[test]
    fn cada_boton_propio_tiene_su_globo_en_los_dos_idiomas() {
        for idioma in [pixpin_store::Idioma::Espanol, pixpin_store::Idioma::Ingles] {
            let t = Catalogo::nuevo(idioma);
            let p = pistas(&t);
            // Uno por boton, con su texto de verdad (no la clave sin traducir).
            assert_eq!(
                p.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                botones().iter().map(|b| b.id).collect::<Vec<_>>()
            );
            assert!(
                p.iter()
                    .all(|(_, s)| !s.is_empty() && !s.starts_with("web-")),
                "{p:?}"
            );
        }
        // Y el de exportar lleva su tecla.
        assert!(
            pistas(&Catalogo::nuevo(pixpin_store::Idioma::Espanol))[1]
                .1
                .contains("Ctrl+E")
        );
    }

    #[test]
    fn el_titulo_lleva_el_fichero_y_el_fondo_sigue_al_tema() {
        assert_eq!(titulo_de_ventana("informe.html"), "informe.html — PixPin");
        assert_ne!(fondo(true), fondo(false));
        // Caso negativo: en claro no se pinta oscuro (una pagina sin estilo
        // es blanca y saldria un fogonazo).
        assert_eq!(fondo(false), (0xff, 0xff, 0xff));
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

#[cfg(test)]
mod latencia {
    /// **Latencia hasta lo primero visible** del visor de HTML y del de
    /// Markdown, como los abre PixPin. Abre ventanas de verdad unos segundos:
    /// solo a mano,
    /// `cargo test -p pixpin --bin pixpinmax latencia_de_los_visores_web -- --ignored --nocapture --test-threads=1`.
    #[test]
    #[ignore = "abre ventanas; mide la latencia a mano"]
    fn latencia_de_los_visores_web() {
        let _ = tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .try_init();
        use std::time::Duration;
        let d = std::env::temp_dir().join(format!("pixpin-latencia-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let idioma = pixpin_store::Idioma::Espanol;
        // (Como la app: el navegador ya en marcha antes de abrir nada.)
        if std::env::var_os("PIXPIN_PRECALENTAR").is_some() {
            pixpin_web::precalentar();
            std::thread::sleep(Duration::from_secs(3));
        }
        for vuelta in 0..3 {
            let html = d.join(format!("informe{vuelta}.html"));
            std::fs::write(
                &html,
                "<!doctype html><html><body><h1>Informe</h1><p>Hola</p></body></html>",
            )
            .unwrap();
            let r = html.clone();
            std::thread::spawn(move || {
                let _com = pixpin_shell::ComDelHilo::iniciar();
                let textos = pixpin_store::Catalogo::nuevo(idioma);
                let _ = super::abrir(&textos, &r);
            });
            let t = pixpin_web::esperar_visible(
                &super::titulo_de_ventana(&format!("informe{vuelta}.html")),
                Duration::from_secs(5),
            );
            eprintln!("vuelta {vuelta}: html visible en {t:?}");
            std::thread::sleep(Duration::from_millis(2500));
            let md = d.join(format!("prueba{vuelta}.md"));
            std::fs::write(&md, "# Prueba\n\nHola **mundo**\n\n- [ ] una\n").unwrap();
            let ub = pixpin_store::Ubicacion::Portable { raiz: d.clone() };
            crate::notas_md::abrir(idioma, ub, crate::notas_md::Destino::Fichero { ruta: md });
            let t = pixpin_web::esperar_visible(
                &format!("prueba{vuelta}.md — PixPin"),
                Duration::from_secs(10),
            );
            eprintln!("vuelta {vuelta}: md visible en {t:?}");
            std::thread::sleep(Duration::from_millis(2500));
        }
    }

    /// La ventana del pin de ahora y su barra encima, a PNG.
    fn foto(nombre: &str) {
        let (area, _) = crate::lector_pin::area_inicial();
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
        let ms = pixpin_capture::enumerar_monitores().expect("monitores");
        let m = *ms.principal().expect("monitor");
        let e = m.escala_por_cien as f32 / 100.0;
        let arriba = ((60.0 * e) as i32).min(area.y - m.area.y).max(0);
        let region = pixpin_geom::Rect {
            x: area.x,
            y: area.y - arriba,
            ancho: area.ancho,
            alto: area.alto + arriba as u32,
        };
        let region = region.interseccion(m.area).expect("en su monitor");
        let foto = pixpin_capture::capturar_monitor(&d, m.id, m.area).expect("captura");
        let img = pixpin_capture::a_imagen(&d, &foto.recortar(&d, region).expect("recorte"))
            .expect("imagen");
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let salida = carpeta.join(format!("visor-{nombre}.png"));
        std::fs::write(
            &salida,
            pixpin_codec::imagen::codificar_png(&img).expect("png"),
        )
        .expect("guardar");
        println!("{}", salida.display());
    }

    /// **Como se ven** el visor de Markdown y el de HTML como pin, en claro
    /// y en oscuro (la luna/sol no se guarda: en las pruebas no hay datos).
    /// Abre ventanas y fotografia su sitio en la pantalla: solo a mano,
    /// `PIXPIN_MUESTRAS=<carpeta> cargo test -p pixpin --bin pixpinmax foto_de_los_visores_web -- --ignored --nocapture`.
    #[test]
    #[ignore = "abre ventanas y fotografia la pantalla; a mano"]
    fn foto_de_los_visores_web() {
        let _ = tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .try_init();
        use std::time::Duration;
        let d = std::env::temp_dir().join(format!("pixpin-foto-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let idioma = pixpin_store::Idioma::Espanol;
        let html = "<!doctype html><html><head><style>@media (prefers-color-scheme: dark){body{background:#1e1e20;color:#eee}}</style></head><body><h1>Informe de obra</h1><p>Avance del 62 %.</p></body></html>";
        let md = "# Visita de obra\n\nLa losa ya esta **hormigonada**.\n\n- [x] encofrado\n- [ ] curado\n\n> Revisar juntas el lunes.\n";
        for (k, oscuro) in [false, true].into_iter().enumerate() {
            if oscuro != crate::oscuro::activo() {
                crate::oscuro::alternar();
            }
            let tema = if oscuro { "oscuro" } else { "claro" };
            let ruta = d.join(format!("Visita{k}.md"));
            std::fs::write(&ruta, md).unwrap();
            crate::notas_md::abrir(
                idioma,
                pixpin_store::Ubicacion::Portable { raiz: d.clone() },
                crate::notas_md::Destino::Fichero { ruta },
            );
            std::thread::sleep(Duration::from_millis(2500));
            foto(&format!("md-{tema}"));
            let r = d.join(format!("Informe{k}.html"));
            std::fs::write(&r, html).unwrap();
            std::thread::spawn(move || {
                let _com = pixpin_shell::ComDelHilo::iniciar();
                let _ = super::abrir(&pixpin_store::Catalogo::nuevo(idioma), &r);
            });
            std::thread::sleep(Duration::from_millis(2500));
            foto(&format!("html-{tema}"));
        }
    }

    /// **El riel sobre una pagina**, para fotografiarlo desde fuera: abre
    /// `PIXPIN_RIEL_MUESTRA` (un `.html` en el visor, un `.md` en el lector de
    /// notas) y espera medio minuto; quien lo lanzo fotografia sus ventanas y
    /// lo cierra. Con `PIXPIN_RIEL_LAPIZ`, el lapiz nace pulsado. A mano:
    /// `PIXPIN_RIEL_MUESTRA=<fichero> cargo test -p pixpin --bin pixpinmax muestra_del_riel_web -- --ignored`.
    #[test]
    #[ignore = "abre ventanas; para las fotos del riel, a mano"]
    fn muestra_del_riel_web() {
        let _ = tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .try_init();
        let Some(ruta) = std::env::var_os("PIXPIN_RIEL_MUESTRA").map(std::path::PathBuf::from)
        else {
            return;
        };
        let idioma = pixpin_store::Idioma::Espanol;
        // Con `PIXPIN_RIEL_OSCURO`, en oscuro (para ver que se imprime en claro).
        if std::env::var_os("PIXPIN_RIEL_OSCURO").is_some() != crate::oscuro::activo() {
            crate::oscuro::alternar();
        }
        if super::se_abre(&pixpin_docs::nombre(&ruta)) {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let _ = super::abrir(&pixpin_store::Catalogo::nuevo(idioma), &ruta);
        } else {
            let raiz = ruta.parent().expect("carpeta").to_path_buf();
            crate::notas_md::abrir(
                idioma,
                pixpin_store::Ubicacion::Portable { raiz },
                crate::notas_md::Destino::Fichero { ruta },
            );
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
    }
}
