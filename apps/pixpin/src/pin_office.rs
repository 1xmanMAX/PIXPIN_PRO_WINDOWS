//! **El pin de Office** (10-oct-2026): un Word, un Excel, un PowerPoint, un
//! Visio o un correo `.msg` abiertos casi al instante, como un pin, con la
//! vista previa que Office registra para el panel del Explorador
//! (`pixpin_shell::vista_previa`). Se ve exactamente como en Office, sin
//! convertir nada.
//!
//! La ventana es la de los lectores como pin (`lector_pin`): del tamano de
//! un pin, siempre encima, la barra fuera con el asa, el nombre, pantalla
//! completa (F11, y Esc para volver), el pin y cerrar; se estira por los
//! bordes. Es una ventana «llana» (sin `WS_EX_NOREDIRECTIONBITMAP`): la
//! vista de Office es una ventana de Windows metida dentro, y bajo una
//! ventana de DirectComposition saldria negra.
//!
//! Sale en seguida con «Abriendo…» y la vista llega cuando Office la tiene
//! (la primera vez Office tiene que arrancar). No le quita el foco a nadie:
//! abierto con el espacio en el Explorador, el teclado sigue en el
//! Explorador y otro espacio lo cierra.
//!
//! En la barra, segun lo que sea:
//! - **Abrir con su programa** (siempre): Word, Excel... y el pin se cierra.
//! - **Abrir en el lector de PixPin** (Word): el lector propio, con marcas,
//!   tinta y lectura en voz alta (aqui no se anota: la vista es de Office).
//! - **Presentar** (PowerPoint): el modo de presentar de siempre.
//! - El nombre: «Abrir con…» (elegir otro programa).
//!
//! Este modulo no lleva `unsafe`: lo de Windows esta en `pixpin-shell`.

#![forbid(unsafe_code)]

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use pixpin_geom::Rect;
use pixpin_render::icono::material;
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_shell::vista_previa::{self, Carteles, Estado, VistaPrevia};
use pixpin_store::{Catalogo, Ubicacion};

use crate::overlay::Recursos;
use crate::lector_pin::{AccionPin, BotonLector, PinLector};

const VK_ESCAPE: u32 = 0x1B;
const VK_F11: u32 = 0x7A;

/// Lo que puede abrir el pin de Office, si el equipo tiene previsualizador
/// para ello: Word, Excel, PowerPoint, Visio, OpenDocument y los correos de
/// Outlook. Un `.csv` no: es de su hoja de calculo (o del Bloc de notas),
/// como en el chat.
pub const EXTENSIONES: [&str; 20] = [
    "doc", "docx", "docm", "dot", "dotx", "rtf", "odt", // Word
    "xls", "xlsx", "xlsm", "xlsb", "ods", // Excel
    "ppt", "pptx", "pptm", "pps", "ppsx", "odp", // PowerPoint
    "vsdx", "msg", // Visio y Outlook
];

/// Los de Visio que no caben arriba (los viejos y las plantillas).
const MAS_VISIO: [&str; 3] = ["vsd", "vsdm", "vssx"];

/// Si la extension (sin punto, en minusculas) es de las del pin de Office.
pub fn es_de_office(ext: &str) -> bool {
    EXTENSIONES.contains(&ext) || MAS_VISIO.contains(&ext)
}

/// **La decision, sin Windows** (para probarla): va al pin de Office si es
/// de Office, el equipo tiene previsualizador, el hilo de las vistas no esta
/// atascado y —siendo un Word que viene de internet— no hay que mandarlo al
/// lector propio (Windows no deja previsualizar lo de internet; un Word se
/// lee igual en PixPin, lo demas sale en el pin con el aviso y «abrir con su
/// programa»).
pub fn va_al_pin(ext: &str, hay_previsualizador: bool, atascado: bool, de_internet: bool) -> bool {
    if !es_de_office(ext) || !hay_previsualizador || atascado {
        return false;
    }
    !(de_internet && tiene_lector_propio(ext))
}

/// Los de Office que PixPin tambien sabe leer en su lector (Word).
pub fn tiene_lector_propio(ext: &str) -> bool {
    matches!(ext, "docx" | "docm")
}

/// **Si este fichero se abre en el pin de Office** en este equipo, ahora.
pub fn se_abre(ruta: &Path) -> bool {
    let ext = vista_previa::extension_de(ruta);
    if !es_de_office(&ext) {
        return false;
    }
    let hay = vista_previa::manejador_de_extension(&ext).is_some();
    // La marca de internet solo se mira si cambia algo (un Word).
    let de_internet = hay && tiene_lector_propio(&ext) && vista_previa::bloqueado_por_internet(ruta);
    va_al_pin(&ext, hay, vista_previa::atascado(), de_internet)
}

/// Abre `ruta` en un pin de Office, en su propio hilo.
pub fn lanzar(idioma: pixpin_store::Idioma, ubicacion: Ubicacion, ruta: &Path) {
    let ruta = ruta.to_path_buf();
    // Que Office vaya arrancando mientras se crea la ventana.
    vista_previa::calentar(&ruta);
    let lanzado = std::thread::Builder::new().name("pin-office".into()).spawn(move || {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let textos = Catalogo::nuevo(idioma);
        let hecho = crate::dispositivo_perdido::con_recursos("pin-office", |r| abrir(r, &textos, &ubicacion, &ruta, idioma));
        if let Err(e) = hecho {
            tracing::warn!(?e, ruta = %ruta.display(), "no se pudo abrir el pin de Office");
        }
    });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del pin de Office");
    }
}

/// Los botones propios de la barra.
const ABRIR_CON_SU_PROGRAMA: u8 = 0;
const LECTOR_PROPIO: u8 = 1;
const PRESENTAR: u8 = 2;

fn botones(nombre: &str) -> Vec<BotonLector> {
    let mut v = Vec::new();
    if crate::visor::se_abre(nombre) && tiene_lector_propio(&pixpin_docs::extension(nombre)) {
        v.push(BotonLector { id: LECTOR_PROPIO, icono: &material::MENU_BOOK, encendido: false });
    }
    if crate::diapositivas::es_presentacion(nombre) {
        v.push(BotonLector { id: PRESENTAR, icono: &material::PLAY_ARROW, encendido: false });
    }
    v.push(BotonLector { id: ABRIR_CON_SU_PROGRAMA, icono: &material::OPEN_IN_NEW, encendido: false });
    v
}

/// Donde va la vista dentro de la ventana: todo menos el borde por donde
/// se estira (en pantalla completa no hay borde).
pub fn area_de_la_vista(ancho: u32, alto: u32, borde: i32) -> Rect {
    let b = borde.max(0);
    Rect { x: b, y: b, ancho: (ancho as i32 - 2 * b).max(1) as u32, alto: (alto as i32 - 2 * b).max(1) as u32 }
}

/// Lo que pide una tecla: `Some(true)` cerrar, `Some(false)` pantalla
/// completa (o volver), `None` nada.
pub fn tecla(vk: u32, pantalla_completa: bool) -> Option<bool> {
    match vk {
        VK_F11 => Some(false),
        VK_ESCAPE if pantalla_completa => Some(false),
        VK_ESCAPE => Some(true),
        _ => None,
    }
}

fn abrir(recursos: &Recursos, textos: &Catalogo, ubicacion: &Ubicacion, ruta: &Path, idioma: pixpin_store::Idioma) -> Result<()> {
    let nombre_fichero = pixpin_docs::nombre(ruta);
    let nombre = crate::lector_pin::nombre_limpio(&pixpin_docs::sin_extension(&nombre_fichero));
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores.principal().context("sin monitor principal")?.to_owned();
    let e = monitor.escala_por_cien as f32 / 100.0;
    let (area_pin, _) = crate::lector_pin::area_inicial();
    let mut pin = PinLector::nuevo(area_pin, e);
    let mut area = pin.area;
    let mut ventana = VentanaOverlay::nueva_llana(area, &format!("{nombre} — {}", textos.t("office-titulo"))).context("no se pudo abrir la ventana del pin de Office")?;
    // Como un pin: siempre encima, y SIN quitarle el foco a nadie.
    ventana.poner_siempre_encima(true);
    ventana.mostrar();
    let motor = recursos.motor();
    pin.crear_barra(&motor, &recursos.d3d(), &ventana);
    let borde = |pin: &PinLector| if pin.pantalla_completa { 0 } else { (crate::lector_pin::BORDE * e).round() as i32 };
    let dentro = area_de_la_vista(area.ancho, area.alto, borde(&pin));

    let de_internet = vista_previa::bloqueado_por_internet(ruta);
    let vista = if de_internet {
        tracing::info!(ruta = %ruta.display(), "pin de Office: viene de internet, sin vista previa");
        VistaPrevia::solo_cartel(ventana.handle(), dentro, &textos.t("office-internet"))
    } else {
        let carteles = Carteles { abriendo: textos.t("office-abriendo"), tarda: textos.t("office-tarda"), fallo: textos.t("office-fallo") };
        VistaPrevia::abrir(ventana.handle(), ruta, dentro, carteles)
    };
    let Some(vista) = vista else {
        // Ni previsualizador ni hilo: lo de siempre, con su programa.
        drop(pin);
        drop(ventana);
        anyhow::bail!("sin vista previa para {}", ruta.display());
    };
    vista_previa::pintar_fondo(ventana.handle());
    let ext = vista_previa::extension_de(ruta);
    let propios = botones(&nombre_fichero);
    let mut contado = false;
    let mut avisado_tarda = false;
    let mut vivo = true;
    // Lo que se hace al cerrar, despues de quitar la vista.
    let mut despues: Option<u8> = None;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        let mut teclas: Vec<u32> = vista.tomar_teclas();
        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if pin.es_de_la_barra(hwnd) {
                let (accion, _) = pin.evento_barra(&evento, &mut ventana);
                match accion {
                    Some(AccionPin::Propio(ABRIR_CON_SU_PROGRAMA)) => {
                        despues = Some(ABRIR_CON_SU_PROGRAMA);
                        vivo = false;
                    }
                    Some(AccionPin::Propio(id @ (LECTOR_PROPIO | PRESENTAR))) => {
                        despues = Some(id);
                        vivo = false;
                    }
                    Some(AccionPin::Propio(_)) => {}
                    Some(AccionPin::Nombre) => match pixpin_shell::abrir_con_otra::abrir_con_otra(ventana.handle(), ruta) {
                        Ok(true) => vivo = false,
                        Ok(false) => {}
                        Err(e) => tracing::warn!(?e, "pin de Office: «Abrir con…» no se pudo"),
                    },
                    Some(AccionPin::PantallaCompleta) => pin.alternar_pantalla_completa(&mut ventana),
                    Some(AccionPin::Fijar) => pin.alternar_fijado(&ventana),
                    Some(AccionPin::Cerrar) => vivo = false,
                    None => {}
                }
                continue;
            }
            if hwnd != ventana.handle() {
                continue;
            }
            if pin.evento_ventana(&evento, &mut ventana).consumido {
                continue;
            }
            match evento {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::Tecla { vk, .. } => teclas.push(vk),
                EventoOverlay::Pintar | EventoOverlay::CambioDpi => vista_previa::pintar_fondo(ventana.handle()),
                _ => {}
            }
        }
        // Las teclas, vengan de la ventana o del previsualizador (Esc y F11
        // con el foco dentro de la vista de Office).
        for vk in teclas {
            match tecla(vk, pin.pantalla_completa) {
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
            vista.mover(area_de_la_vista(area.ancho, area.alto, borde(&pin)));
            vista_previa::pintar_fondo(ventana.handle());
        }
        // Lo que tardo, al registro (una vez).
        match vista.estado() {
            Estado::Lista { ms, en_frio } if !contado => {
                contado = true;
                tracing::info!(%ext, ms, en_frio, "pin de Office: listo");
            }
            Estado::Fallo(motivo) if !contado => {
                contado = true;
                tracing::warn!(%ext, %motivo, "pin de Office: Office no pudo; queda «abrir con su programa»");
            }
            Estado::Abriendo if !avisado_tarda && vista.edad() > vista_previa::TARDA => {
                avisado_tarda = true;
                tracing::warn!(%ext, ms = vista.edad().as_millis() as u64, "pin de Office: tarda");
            }
            _ => {}
        }
        pin.actualizar(&motor, &nombre, &propios);
        let tope = if pin.quiere_reloj() { 50 } else { 250 };
        pixpin_shell::overlay::esperar_eventos(Some(tope));
    }

    // Cerrar: se esconde ya (que se note al instante) y luego se quita la
    // vista. Si Office esta colgado y el hilo no contesta, la ventana se
    // queda escondida en vez de destruirse: destruirla esperaria al hilo.
    drop(pin);
    ventana.ocultar();
    let cerrada = vista.cerrar(Duration::from_millis(1500));
    if cerrada {
        drop(ventana);
    } else {
        tracing::warn!("pin de Office: la vista no se quito a tiempo; la ventana queda escondida");
        std::mem::forget(ventana);
    }
    match despues {
        Some(ABRIR_CON_SU_PROGRAMA) => {
            if let Err(e) = pixpin_shell::abrir(ruta) {
                tracing::warn!(?e, "pin de Office: no se pudo abrir con su programa");
            }
        }
        Some(LECTOR_PROPIO) => crate::visor::lanzar(idioma, ubicacion.clone(), ruta),
        Some(PRESENTAR) => crate::diapositivas::lanzar(idioma, ubicacion.clone(), ruta),
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn van_al_pin_los_de_office_con_previsualizador() {
        for ext in ["xlsx", "xls", "xlsb", "doc", "docx", "rtf", "odt", "ods", "odp", "vsd", "vsdx", "msg", "pptx", "ppt"] {
            assert!(va_al_pin(ext, true, false, false), "{ext}");
        }
    }

    #[test]
    fn caso_negativo_sin_previsualizador_o_atascado_o_no_es_de_office() {
        // Sin previsualizador (un equipo sin Office): a lo de siempre.
        assert!(!va_al_pin("xlsx", false, false, false));
        assert!(!va_al_pin("docx", false, false, false));
        // El hilo de las vistas atascado (Office colgado): tampoco.
        assert!(!va_al_pin("pptx", true, true, false));
        // Lo que no es de Office, aunque Windows tenga vista previa para ello.
        for ext in ["pdf", "png", "csv", "txt", "zip", "", "docx.txt"] {
            assert!(!va_al_pin(ext, true, false, false), "{ext}");
        }
    }

    #[test]
    fn un_word_de_internet_va_al_lector_propio_y_un_excel_al_pin_con_aviso() {
        assert!(!va_al_pin("docx", true, false, true), "el Word de internet se lee en PixPin");
        assert!(va_al_pin("xlsx", true, false, true), "el Excel sale en el pin, con el aviso");
        assert!(va_al_pin("doc", true, false, true), "un .doc no lo lee PixPin: pin con aviso");
    }

    #[test]
    fn la_vista_deja_el_borde_para_estirar() {
        assert_eq!(area_de_la_vista(800, 600, 7), Rect { x: 7, y: 7, ancho: 786, alto: 586 });
        // En pantalla completa, sin borde.
        assert_eq!(area_de_la_vista(1920, 1080, 0), Rect { x: 0, y: 0, ancho: 1920, alto: 1080 });
        // Caso negativo: una ventana diminuta no da tamanos negativos.
        assert_eq!(area_de_la_vista(10, 10, 7), Rect { x: 7, y: 7, ancho: 1, alto: 1 });
    }

    #[test]
    fn esc_cierra_o_sale_de_pantalla_completa_y_f11_alterna() {
        assert_eq!(tecla(VK_ESCAPE, false), Some(true));
        assert_eq!(tecla(VK_ESCAPE, true), Some(false));
        assert_eq!(tecla(VK_F11, false), Some(false));
        // Caso negativo: otra tecla no hace nada.
        assert_eq!(tecla(0x41, false), None);
    }

    #[test]
    fn la_barra_lleva_lo_que_toca_a_cada_uno() {
        let ids = |n: &str| botones(n).iter().map(|b| b.id).collect::<Vec<_>>();
        assert_eq!(ids("Informe.docx"), vec![LECTOR_PROPIO, ABRIR_CON_SU_PROGRAMA]);
        assert_eq!(ids("Clase.pptx"), vec![PRESENTAR, ABRIR_CON_SU_PROGRAMA]);
        // Caso negativo: un Excel no se lee ni se presenta.
        assert_eq!(ids("Gastos.xlsx"), vec![ABRIR_CON_SU_PROGRAMA]);
    }
}
