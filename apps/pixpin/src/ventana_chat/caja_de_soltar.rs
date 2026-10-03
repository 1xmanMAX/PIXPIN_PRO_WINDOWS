//! **La caja de soltar** (pedido «soltar» y «Añadir arrastrando…» del menu
//! del chat; el usuario, 2-oct): «que solo me salga un recuadro flotante, y
//! todo lo que suelte sobre esto se agregue al chat del proyecto sin tener
//! que abrir el chat».
//!
//! Un recuadro pequeño, siempre encima y sin boton en la barra de tareas,
//! con el nombre del proyecto y una zona rayada. Lo que se suelta encima
//! entra en el chat **por el mismo sitio que si se soltara en la ventana del
//! chat** (`meter_en_proyecto`, que es `adjuntar_en_proyecto` uno a uno: el
//! fichero copiado al proyecto, su mensaje ARCHIVO/IMAGEN/VOZ con el sello
//! de este equipo y el PDF a la cola de aligerar). Se queda abierta para
//! soltar varias veces; la cierra su aspa o Escape. Una por proyecto: pedirla
//! otra vez trae delante la que ya esta.
//!
//! Solo ficheros (`WM_DROPFILES`, lo que suelta el Explorador y casi todo
//! programa de escritorio). Una imagen o un texto arrastrados desde el
//! navegador llegan por OLE (`IDropTarget`) sin fichero detras, y eso es
//! otro camino entero que hoy tampoco tiene la ventana del chat.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_geom::{Punto, Rect};
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Catalogo;

use super::{OSCURO, mi};
use crate::overlay::Recursos;
use crate::ventanita::{Botones, VERDE, dentro};

/// Lo que se queda en pantalla «3 añadidos» antes de volver a la invitacion.
const AVISO_DURA: Duration = Duration::from_millis(2200);
/// Lo que se mueve el raton antes de que un clic pase a ser un arrastre.
const UMBRAL_ARRASTRE: i32 = 4;
const VK_ESCAPE: u32 = 0x1B;
/// El tamano logico de la caja.
const ANCHO: u32 = 280;
const ALTO: u32 = 176;

/// Que proyecto recibe lo soltado.
#[derive(Debug, Clone)]
pub(crate) struct Pedido {
    pub raiz: PathBuf,
    pub proyecto: String,
    /// El nombre que se ensena (el de la ficha).
    pub nombre: String,
    /// El codigo de este equipo, el del sello.
    pub aparato: String,
}

/// Los rotulos, ya traducidos (el `Catalogo` no cruza hilos).
#[derive(Debug, Clone)]
pub(crate) struct Rotulos {
    aqui: String,
    anadiendo: String,
    uno: String,
    varios: String,
    ninguno: String,
}

/// Lo que se cambia por el numero en `varios`.
const HUECO: &str = "\u{1}";

impl Rotulos {
    pub(crate) fn de(t: &Catalogo, proyecto: &str) -> Rotulos {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("proyecto", proyecto.to_string());
        let mut n = fluent_bundle::FluentArgs::new();
        n.set("n", HUECO);
        Rotulos {
            aqui: t.t_args("soltar-aqui", &args),
            anadiendo: t.t("soltar-anadiendo"),
            uno: t.t("soltar-anadido-uno"),
            varios: t.t_args("soltar-anadidos", &n),
            ninguno: t.t("soltar-ninguno"),
        }
    }

    /// Lo que se dice tras soltar `n` ficheros que entraron.
    fn de_hechos(&self, n: usize) -> String {
        match n {
            0 => self.ninguno.clone(),
            1 => self.uno.clone(),
            n => self.varios.replace(HUECO, &n.to_string()),
        }
    }
}

// --- Una por proyecto ------------------------------------------------------

/// Las cajas abiertas: el proyecto y su ventana (`0` mientras nace).
static ABIERTAS: Mutex<Vec<(String, isize)>> = Mutex::new(Vec::new());

/// Lo que toca al pedir la caja de `proyecto`.
#[derive(Debug, PartialEq, Eq)]
enum Reserva {
    /// No habia ninguna: queda apuntada y hay que abrirla.
    Nueva,
    /// Ya esta abierta en esa ventana: se trae delante.
    YaEsta(isize),
    /// Esta naciendo ahora mismo: no se hace nada.
    Naciendo,
}

fn reservar(abiertas: &mut Vec<(String, isize)>, proyecto: &str) -> Reserva {
    match abiertas.iter().find(|(p, _)| p == proyecto) {
        Some((_, 0)) => Reserva::Naciendo,
        Some((_, h)) => Reserva::YaEsta(*h),
        None => {
            abiertas.push((proyecto.to_string(), 0));
            Reserva::Nueva
        }
    }
}

fn apuntar_ventana(abiertas: &mut [(String, isize)], proyecto: &str, hwnd: isize) {
    if let Some(e) = abiertas.iter_mut().find(|(p, _)| p == proyecto) {
        e.1 = hwnd;
    }
}

fn soltar_reserva(abiertas: &mut Vec<(String, isize)>, proyecto: &str) {
    abiertas.retain(|(p, _)| p != proyecto);
}

fn con_abiertas<T>(f: impl FnOnce(&mut Vec<(String, isize)>) -> T) -> T {
    let mut g = ABIERTAS.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut g)
}

/// **Saca la caja de soltar de un proyecto**, o trae delante la que ya
/// esta. En su propio hilo, como el microfono flotante.
pub(crate) fn lanzar(pedido: Pedido, rotulos: Rotulos) {
    match con_abiertas(|v| reservar(v, &pedido.proyecto)) {
        Reserva::YaEsta(h) => {
            let hwnd = windows::Win32::Foundation::HWND(h as *mut _);
            VentanaOverlay::restaurar_de_hwnd(hwnd);
            return;
        }
        Reserva::Naciendo => return,
        Reserva::Nueva => {}
    }
    let proyecto = pedido.proyecto.clone();
    let hecho = std::thread::Builder::new()
        .name("caja-de-soltar".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            if let Err(e) = Recursos::nuevos().and_then(|r| flotar(&r, &pedido, &rotulos)) {
                tracing::warn!(?e, "no se pudo abrir la caja de soltar");
            }
            con_abiertas(|v| soltar_reserva(v, &pedido.proyecto));
        });
    if let Err(e) = hecho {
        con_abiertas(|v| soltar_reserva(v, &proyecto));
        tracing::warn!(?e, "no se pudo lanzar el hilo de la caja de soltar");
    }
}

/// **Mete en el chat de `proyecto` los ficheros soltados**, como la ventana
/// del chat, y le dice que se relea si esta abierta. Devuelve cuantos
/// entraron: uno que no se pueda leer (una carpeta, un fichero que se movio)
/// se salta y no se lleva los demas.
pub(super) fn anadir(raiz: &Path, proyecto: &str, aparato: &str, rutas: &[PathBuf]) -> usize {
    let ficheros = super::leer_ficheros(rutas);
    if ficheros.is_empty() {
        return 0;
    }
    match super::meter_en_proyecto(raiz, proyecto, &ficheros, aparato) {
        Ok(hechos) => {
            if !hechos.is_empty() {
                super::refrescar();
            }
            hechos.len()
        }
        Err(e) => {
            tracing::warn!(?e, "no se pudo leer el cuaderno para soltar");
            0
        }
    }
}

// --- La ventana -------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Cerrar,
}

enum Estado {
    Esperando,
    /// Metiendo lo soltado en su hilo (un video grande tarda en copiarse).
    Anadiendo(Receiver<usize>),
    Dicho { texto: String, bien: bool, desde: Instant },
}

/// Abajo a la derecha del monitor del raton, como el microfono.
fn marco_inicial(area: Rect, escala_por_cien: u32) -> Rect {
    let e = |v: u32| v * escala_por_cien / 100;
    let (w, h) = (e(ANCHO).min(area.ancho), e(ALTO).min(area.alto));
    Rect {
        x: area.x + area.ancho as i32 - w as i32 - e(24) as i32,
        y: area.y + area.alto as i32 - h as i32 - e(24) as i32,
        ancho: w,
        alto: h,
    }
}

fn flotar(recursos: &Recursos, pedido: &Pedido, rotulos: &Rotulos) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .monitor_en(pixpin_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let mut marco = marco_inicial(monitor.area_trabajo, monitor.escala_por_cien);
    let mut ventana = VentanaOverlay::nueva(marco).context("no se pudo abrir la caja")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para la caja")?;
    ventana.aceptar_ficheros(true);
    ventana.mostrar();
    ventana.traer_encima();
    ventana.enfocar();
    con_abiertas(|v| apuntar_ventana(v, &pedido.proyecto, ventana.handle().0 as isize));

    let mut estado = Estado::Esperando;
    let mut botones: Botones<Accion> = Botones::default();
    let mut pulsado: Option<(Punto, Rect, Option<Accion>, bool)> = None;
    // Lo soltado mientras se metia lo anterior espera su turno.
    let mut en_cola: Vec<PathBuf> = Vec::new();
    let mut vivo = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        let mut accion = None;
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match ev {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::Tecla { vk: VK_ESCAPE, .. } => vivo = false,
                EventoOverlay::FicherosSoltados => {
                    en_cola.extend(pixpin_shell::overlay::ficheros_soltados());
                }
                EventoOverlay::RatonMovido(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    if let Some((desde, origen, _, arrastre)) = pulsado.as_mut() {
                        let (dx, dy) = (p.x - desde.x, p.y - desde.y);
                        if !*arrastre && (dx.abs() > UMBRAL_ARRASTRE || dy.abs() > UMBRAL_ARRASTRE) {
                            *arrastre = true;
                        }
                        if *arrastre {
                            marco = Rect {
                                x: origen.x + dx,
                                y: origen.y + dy,
                                ..*origen
                            };
                            ventana.mover(marco);
                            botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                        }
                    }
                }
                EventoOverlay::BotonPulsado(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    pulsado = Some((p, marco, botones.bajo_el_raton(), false));
                }
                EventoOverlay::BotonSoltado(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    if let Some((_, _, Some(a), false)) = pulsado.take()
                        && botones.bajo_el_raton() == Some(a)
                    {
                        accion = Some(a);
                    }
                }
                _ => {}
            }
        }
        if accion == Some(Accion::Cerrar) {
            vivo = false;
        }
        if !vivo {
            break;
        }

        estado = match estado {
            Estado::Anadiendo(rx) => match rx.try_recv() {
                Ok(n) => Estado::Dicho {
                    texto: rotulos.de_hechos(n),
                    bien: n > 0,
                    desde: Instant::now(),
                },
                Err(TryRecvError::Empty) => Estado::Anadiendo(rx),
                Err(TryRecvError::Disconnected) => Estado::Dicho {
                    texto: rotulos.ninguno.clone(),
                    bien: false,
                    desde: Instant::now(),
                },
            },
            Estado::Dicho { desde, .. } if desde.elapsed() >= AVISO_DURA => Estado::Esperando,
            otro => otro,
        };
        // Lo soltado se mete en otro hilo: copiar un video de un giga no
        // puede dejar la caja sin pintar ni atender a Escape.
        if !en_cola.is_empty() && !matches!(estado, Estado::Anadiendo(_)) {
            let rutas = std::mem::take(&mut en_cola);
            let (tx, rx) = std::sync::mpsc::channel();
            let (raiz, proyecto, aparato) = (
                pedido.raiz.clone(),
                pedido.proyecto.clone(),
                pedido.aparato.clone(),
            );
            let lanzado = std::thread::Builder::new()
                .name("soltar-en-proyecto".into())
                .spawn(move || {
                    let _ = tx.send(anadir(&raiz, &proyecto, &aparato, &rutas));
                });
            if let Err(e) = lanzado {
                tracing::warn!(?e, "no se pudo lanzar el hilo de soltar");
            }
            estado = Estado::Anadiendo(rx);
        }

        if let Ok(d) = superficie.empezar(&motor) {
            let _ = motor.dibujar(&d, |p: &Pintor| {
                pintar(p, marco, escala, &estado, pedido, rotulos, &mut botones)
            });
            let _ = superficie.presentar();
        }
        let latido = if matches!(estado, Estado::Esperando) { 500 } else { 60 };
        pixpin_shell::overlay::esperar_eventos(Some(latido));
    }
    ventana.ocultar();
    // Lo que estaba entrando termina de entrar: soltar y cerrar enseguida no
    // puede perder ficheros.
    if let Estado::Anadiendo(rx) = estado {
        let _ = rx.recv_timeout(Duration::from_secs(60));
    }
    if !en_cola.is_empty() {
        anadir(&pedido.raiz, &pedido.proyecto, &pedido.aparato, &en_cola);
    }
    Ok(())
}

fn pintar(
    p: &Pintor,
    marco: Rect,
    e: f32,
    estado: &Estado,
    pedido: &Pedido,
    r: &Rotulos,
    botones: &mut Botones<Accion>,
) {
    let tema = &OSCURO;
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    p.limpiar_transparente();
    p.rellenar_redondeado(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: h,
        },
        18.0 * e,
        tema.isla_borde,
    );
    p.rellenar_redondeado(
        RectF {
            x: 1.0,
            y: 1.0,
            ancho: w - 2.0,
            alto: h - 2.0,
        },
        17.0 * e,
        tema.isla,
    );
    botones.vaciar();

    // Arriba: la carpeta, el nombre del proyecto y el aspa.
    let disco = 24.0 * e;
    let (x0, y0) = (14.0 * e, 12.0 * e);
    p.circulo((x0 + disco / 2.0, y0 + disco / 2.0), disco / 2.0, tema.carpeta);
    let lado = 15.0 * e;
    p.icono(
        &mi::FOLDER,
        RectF {
            x: x0 + (disco - lado) / 2.0,
            y: y0 + (disco - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        tema.carpeta_icono,
    );
    let aspa = RectF {
        x: w - 40.0 * e,
        y: 10.0 * e,
        ancho: 28.0 * e,
        alto: 28.0 * e,
    };
    let tam = 14.0 * e;
    let (_, th) = p.medir_texto(&pedido.nombre, tam);
    let tx = x0 + disco + 8.0 * e;
    p.texto_linea(
        &pedido.nombre,
        tx,
        y0 + (disco - th) / 2.0,
        tam,
        (aspa.x - tx - 6.0 * e).max(0.0),
        tema.pildora_texto,
    );
    if dentro(aspa, botones.raton) {
        p.rellenar_redondeado(aspa, aspa.alto / 2.0, tema.boton_sobre);
    }
    let l = 18.0 * e;
    p.icono(
        &mi::CLOSE,
        RectF {
            x: aspa.x + (aspa.ancho - l) / 2.0,
            y: aspa.y + (aspa.alto - l) / 2.0,
            ancho: l,
            alto: l,
        },
        tema.campo_apagado,
    );
    botones.zona(aspa, Accion::Cerrar);

    // La zona rayada donde se suelta.
    let zona = RectF {
        x: 14.0 * e,
        y: 46.0 * e,
        ancho: w - 28.0 * e,
        alto: h - 60.0 * e,
    };
    p.rellenar_redondeado(zona, 12.0 * e, Color { a: 0.55, ..tema.campo });
    p.trazar_discontinuo(zona, (1.5 * e).max(1.0), tema.campo_apagado);
    let (icono, texto, color): (&pixpin_render::icono::Icono, &str, Color) = match estado {
        Estado::Esperando => (&mi::ATTACH_FILE, &r.aqui, tema.texto),
        Estado::Anadiendo(_) => (&mi::ATTACH_FILE, &r.anadiendo, tema.campo_apagado),
        Estado::Dicho { texto, bien: true, .. } => (&mi::CHECK_CIRCLE, texto, VERDE),
        Estado::Dicho { texto, .. } => (&mi::CLOSE, texto, tema.campo_apagado),
    };
    let li = 26.0 * e;
    p.icono(
        icono,
        RectF {
            x: zona.x + (zona.ancho - li) / 2.0,
            y: zona.y + 14.0 * e,
            ancho: li,
            alto: li,
        },
        color,
    );
    let ancho_max = zona.ancho - 24.0 * e;
    let tam = 13.0 * e;
    let (tw, _) = p.medir_texto_ajustado(texto, tam, ancho_max);
    p.texto_ajustado(
        texto,
        zona.x + (zona.ancho - tw) / 2.0,
        zona.y + 14.0 * e + li + 8.0 * e,
        tam,
        ancho_max,
        color,
    );
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_caja_por_proyecto() {
        let mut v = Vec::new();
        assert_eq!(reservar(&mut v, "casa"), Reserva::Nueva);
        // Mientras nace, pedirla otra vez no abre otra.
        assert_eq!(reservar(&mut v, "casa"), Reserva::Naciendo);
        apuntar_ventana(&mut v, "casa", 77);
        assert_eq!(reservar(&mut v, "casa"), Reserva::YaEsta(77));
        // Otro proyecto tiene la suya.
        assert_eq!(reservar(&mut v, "obra"), Reserva::Nueva);
        soltar_reserva(&mut v, "casa");
        assert_eq!(reservar(&mut v, "casa"), Reserva::Nueva);
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn lo_dicho_cuenta_los_que_entraron() {
        let r = Rotulos {
            aqui: String::new(),
            anadiendo: String::new(),
            uno: "1 añadido".into(),
            varios: format!("{HUECO} añadidos"),
            ninguno: "nada".into(),
        };
        assert_eq!(r.de_hechos(0), "nada");
        assert_eq!(r.de_hechos(1), "1 añadido");
        assert_eq!(r.de_hechos(3), "3 añadidos");
    }

    #[test]
    fn lo_soltado_entra_en_el_chat_como_en_la_ventana_del_chat() {
        let raiz = std::env::temp_dir().join(format!("pixpin-soltar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let f = pixpin_proyecto::almacen::Ficha::nueva("Casa", 1, "PC01");
        pixpin_proyecto::almacen::Indice {
            proyectos: vec![f.clone()],
            ..Default::default()
        }
        .guardar(&raiz)
        .unwrap();
        let fuera = raiz.join("fuera");
        std::fs::create_dir_all(&fuera).unwrap();
        let foto = fuera.join("foto.png");
        let doc = fuera.join("informe.docx");
        std::fs::write(&foto, b"png").unwrap();
        std::fs::write(&doc, b"docx").unwrap();
        // Una que no existe y una carpeta se saltan sin llevarse las demas.
        let rutas = vec![foto, fuera.join("no-esta.txt"), doc, fuera.clone()];
        assert_eq!(anadir(&raiz, &f.id, "PC01", &rutas), 2);
        let c = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&pixpin_proyecto::almacen::carpeta(
            &raiz, &f.id,
        ))
        .unwrap();
        use pixpin_proyecto::cuaderno::Clase;
        let clases: Vec<_> = c.mensajes.iter().map(|m| m.clase.clone()).collect();
        assert_eq!(clases, vec![Some(Clase::Imagen), Some(Clase::Archivo)]);
        let numeros: Vec<_> = c.mensajes.iter().map(|m| m.numero).collect();
        assert_eq!(numeros, vec![1, 2]);
        assert!(c.mensajes.iter().all(|m| m.aparato.as_deref() == Some("PC01")));
        // El fichero esta copiado dentro del proyecto.
        for m in &c.mensajes {
            let r = super::super::fichero_del_mensaje(&raiz, &f.id, m).unwrap();
            assert!(r.starts_with(pixpin_proyecto::almacen::carpeta(&raiz, &f.id)));
        }
        // Caso negativo: nada que leer no escribe nada.
        assert_eq!(anadir(&raiz, &f.id, "PC01", &[fuera.join("tampoco")]), 0);
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
