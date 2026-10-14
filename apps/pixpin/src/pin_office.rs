//! **El pin de Office** (10-oct-2026): los formatos VIEJOS de Office y los
//! que no son OOXML —un `.doc`, un `.xls`, un `.ppt`, un Visio, un correo
//! `.msg`, un OpenDocument— abiertos como un pin con la vista previa que
//! Office registra para el panel del Explorador
//! (`pixpin_shell::vista_previa`). Se ven tal cual en Office, sin convertir.
//!
//! Los OOXML (`.docx`, `.xlsx`, `.pptx` y sus primos) NO pasan por aqui: los
//! abrira el motor nativo de PixPin, sin Office. Y Office solo vive mientras
//! el pin esta abierto (el usuario, 10-oct: «no es buena idea tener Office
//! abierto, consume RAM en segundo plano y va contra la filosofia de la
//! app»): al cerrar el pin, el servidor de Office se va.
//!
//! La ventana es la de los lectores como pin (`lector_pin`): del tamano de
//! un pin (nace sin fijar; su pin lo deja encima), la barra fuera con el asa, el nombre, pantalla
//! completa (F11, y Esc para volver), el pin y cerrar; se estira por los
//! bordes. Es una ventana «llana» (sin `WS_EX_NOREDIRECTIONBITMAP`): la
//! vista de Office es una ventana de Windows metida dentro, y bajo una
//! ventana de DirectComposition saldria negra.
//!
//! Sale en seguida con «Abriendo…» y la vista llega cuando Office la tiene
//! (Office tiene que arrancar cada vez). No le quita el foco a nadie:
//! abierto con el espacio en el Explorador, el teclado sigue en el
//! Explorador y otro espacio lo cierra.
//!
//! En la barra, segun lo que sea:
//! - **Abrir con…** (siempre, el ↗): el cuadro de Windows con las apps que
//!   abren el formato (Word, Excel, LibreOffice...); elegida, el pin se cierra.
//! - **Presentar** (`.ppt`, `.pps`, `.odp`): el modo de presentar de siempre.
//! - El nombre: lo mismo, «Abrir con…».
//!
//! Lo que viene de internet no se previsualiza (lo decide Windows en el
//! Explorador por seguridad, y aqui se respeta): sale el pin con el aviso y
//! «Abrir con…».
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

use crate::lector_pin::{AccionPin, BotonLector, PinLector};
use crate::overlay::Recursos;

const VK_ESCAPE: u32 = 0x1B;
const VK_F11: u32 = 0x7A;

/// Lo que abre el pin de Office, si el equipo tiene previsualizador para
/// ello: los formatos que el motor nativo NO lee. Word 97-2003 y RTF; Excel
/// 97-2003 y binario; PowerPoint 97-2003; OpenDocument; Visio; los correos
/// de Outlook. Ni los OOXML (motor nativo) ni un `.csv` (su hoja de calculo
/// o el Bloc de notas, como en el chat).
pub const EXTENSIONES: [&str; 17] = [
    "doc", "dot", "rtf", // Word
    "xls", "xlsb", "xlt", // Excel
    "ppt", "pps", "pot", // PowerPoint
    "odt", "ods", "odp", // OpenDocument
    "vsd", "vsdx", "vsdm", "vssx", // Visio
    "msg", // Outlook
];

/// Si la extension (sin punto, en minusculas) es de las del pin de Office.
pub fn es_de_office(ext: &str) -> bool {
    EXTENSIONES.contains(&ext)
}

/// **La decision, sin Windows** (para probarla): va al pin de Office si es
/// de los suyos, el equipo tiene previsualizador y el hilo de las vistas no
/// esta atascado (un Office colgado).
pub fn va_al_pin(ext: &str, hay_previsualizador: bool, atascado: bool) -> bool {
    es_de_office(ext) && hay_previsualizador && !atascado
}

/// **Si este fichero se abre en el pin de Office** en este equipo, ahora.
pub fn se_abre(ruta: &Path) -> bool {
    let ext = vista_previa::extension_de(ruta);
    if !es_de_office(&ext) {
        return false;
    }
    // Un Excel 97-2003 que el visor de hojas abre sin Office (rapido, sin
    // lanzar Office en segundo plano) ya no pasa por aqui. Si no lo
    // entiende (cifrado, roto, Excel 5), el pin de Office queda de respaldo.
    if matches!(ext.as_str(), "xls" | "xlt") && crate::visor_hoja::se_abre_fichero(ruta) {
        return false;
    }
    let hay = vista_previa::manejador_de_extension(&ext).is_some();
    va_al_pin(&ext, hay, vista_previa::atascado())
}

/// Abre `ruta` en un pin de Office, en su propio hilo.
pub fn lanzar(idioma: pixpin_store::Idioma, ubicacion: Ubicacion, ruta: &Path) {
    let ruta = ruta.to_path_buf();
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
const ABRIR_CON: u8 = 0;
const PRESENTAR: u8 = 2;

fn botones(nombre: &str) -> Vec<BotonLector> {
    let mut v = Vec::new();
    if crate::diapositivas::es_presentacion(nombre) {
        v.push(BotonLector { id: PRESENTAR, icono: &material::PLAY_ARROW, encendido: false });
    }
    v.push(BotonLector { id: ABRIR_CON, icono: &material::OPEN_IN_NEW, encendido: false });
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
    // Sin luna/sol: la vista previa la pinta Office en su ventana y no se
    // puede oscurecer desde aqui.
    let mut pin = PinLector::nuevo(area_pin, e).con_extension(&pixpin_docs::extension(&nombre_fichero)).sin_oscuro();
    let mut area = pin.area;
    let mut ventana = VentanaOverlay::nueva_llana(area, &format!("{nombre} — {}", textos.t("office-titulo"))).context("no se pudo abrir la ventana del pin de Office")?;
    // Como un pin pero sin fijar (lo fija su pin): al frente, una ventana
    // mas, y SIN quitarle el foco a nadie.
    pin.al_nacer(&ventana);
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
                    // «Abrir con…» (el ↗ y el nombre): el cuadro de Windows
                    // sobre el pin. Elegida la app, el pin se cierra (y
                    // Office suelta el fichero mientras la otra arranca);
                    // cerrado el cuadro sin elegir, el pin sigue.
                    Some(AccionPin::Propio(ABRIR_CON)) | Some(AccionPin::Nombre) => {
                        if crate::abrir_con::abrir_con_o_avisar(ventana.handle(), ruta, textos) {
                            vivo = false;
                        }
                    }
                    Some(AccionPin::Propio(PRESENTAR)) => {
                        despues = Some(PRESENTAR);
                        vivo = false;
                    }
                    Some(AccionPin::Propio(_)) => {}
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
            Estado::Lista { ms } if !contado => {
                contado = true;
                tracing::info!(%ext, ms, "pin de Office: listo");
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
        Some(PRESENTAR) => crate::diapositivas::lanzar(idioma, ubicacion.clone(), ruta),
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn van_al_pin_los_formatos_viejos_con_previsualizador() {
        for ext in ["doc", "dot", "rtf", "xls", "xlsb", "xlt", "ppt", "pps", "pot", "odt", "ods", "odp", "vsd", "vsdx", "vsdm", "vssx", "msg"] {
            assert!(va_al_pin(ext, true, false), "{ext}");
        }
    }

    #[test]
    fn caso_negativo_los_ooxml_son_del_motor_nativo() {
        for ext in ["docx", "docm", "dotx", "dotm", "xlsx", "xlsm", "pptx", "pptm", "ppsx", "potx"] {
            assert!(!va_al_pin(ext, true, false), "{ext}");
        }
    }

    #[test]
    fn caso_negativo_sin_previsualizador_o_atascado_o_no_es_de_office() {
        // Sin previsualizador (un equipo sin Office): a lo de siempre.
        assert!(!va_al_pin("xls", false, false));
        assert!(!va_al_pin("doc", false, false));
        // El hilo de las vistas atascado (Office colgado): tampoco.
        assert!(!va_al_pin("ppt", true, true));
        // Lo que no es de Office, aunque Windows tenga vista previa para ello.
        for ext in ["pdf", "png", "csv", "txt", "zip", "", "doc.txt"] {
            assert!(!va_al_pin(ext, true, false), "{ext}");
        }
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
        assert_eq!(ids("Clase.ppt"), vec![PRESENTAR, ABRIR_CON]);
        assert_eq!(ids("Clase.odp"), vec![PRESENTAR, ABRIR_CON]);
        // Caso negativo: un Excel o un Word no se presentan.
        assert_eq!(ids("Gastos.xls"), vec![ABRIR_CON]);
        assert_eq!(ids("Carta.doc"), vec![ABRIR_CON]);
    }
}
