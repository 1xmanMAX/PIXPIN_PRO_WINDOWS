//! El panel «Pines abiertos» (rediseno v2), del lado del gestor: que filas
//! salen y que hace cada boton. La ventana y su disposicion viven en
//! `pixpin_pin::panel_abiertos`; aqui esta lo que necesita el almacen.
//!
//! Antes no habia forma de encontrar un pin tapado por otra ventana, ni de
//! traer uno solo de un grupo oculto: solo Ctrl+2 (todos o ninguno) y el
//! menu de la bandeja por grupos. El panel los lista todos, ocultos
//! incluidos, y deja traer, ocultar o cerrar uno a uno.
//!
//! «Oculto» es una de tres cosas: escondido desde este panel (la ventana
//! sigue viva, `escondidos`), quitado de la pantalla con Ctrl+2 o con su
//! grupo (abierto en el almacen sin ventana), o un pin en vivo escondido.

use std::path::Path;

use anyhow::{Context, Result};
use pixpin_geom::Rect;
use pixpin_pin::panel_abiertos::{AccionPanel, FilaPanel, PanelAbiertos, TipoFila};
use pixpin_store::{PinGuardado, TipoEntrada};

use super::{Pines, posicion_guardada};

/// Las extensiones que el pin abre como video. Las mismas que
/// `contenido_de_archivo` manda al reproductor.
const VIDEO: [&str; 8] = ["mp4", "m4v", "mov", "mkv", "webm", "avi", "wmv", "mpg"];

/// Que tipo de fila es un fichero por referencia, por su extension.
pub(super) fn tipo_de_ruta(ruta: &Path) -> TipoFila {
    let ext = ruta
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if ext == "pdf" {
        TipoFila::Pdf
    } else if VIDEO.contains(&ext.as_str()) {
        TipoFila::Video
    } else {
        TipoFila::Archivo
    }
}

/// El nombre de una nota: su primera linea con algo, sin los signos de
/// Markdown del principio y cortada a lo que cabe en una fila.
pub(super) fn nombre_de_nota(texto: &str) -> String {
    let linea = texto
        .lines()
        .map(|l| l.trim().trim_start_matches(['#', '-', '*', '>', ' ']).trim())
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let mut nombre: String = linea.chars().take(60).collect();
    if linea.chars().count() > 60 {
        nombre.push('…');
    }
    nombre
}

/// El rect del pin centrado en `area`; si no cabe, pegado a su esquina.
pub(super) fn centrado(pin: Rect, area: Rect) -> Rect {
    let x = area.x + (area.ancho as i32 - pin.ancho as i32).max(0) / 2;
    let y = area.y + (area.alto as i32 - pin.alto as i32).max(0) / 2;
    Rect { x, y, ..pin }
}

impl Pines {
    /// Abre el panel, o lo trae delante si ya estaba abierto.
    pub fn abrir_panel_abiertos(&mut self) -> Result<()> {
        if let Some(p) = &self.panel {
            p.traer();
            self.refrescar_panel();
            return Ok(());
        }
        let d = pixpin_capture::enumerar_monitores().context("sin monitores")?;
        // En el monitor del primer pin a la vista, o en el principal.
        let monitor = self
            .vivos
            .values()
            .find(|p| p.visible())
            .map(|p| p.rect_contenido())
            .and_then(|r| {
                d.monitores()
                    .iter()
                    .find(|m| m.area.interseccion(r).is_some())
            })
            .or_else(|| d.principal())
            .context("sin monitor para el panel")?;
        let escala = monitor.escala_por_cien;
        let e = escala as f32 / 100.0;
        let area = monitor.area_trabajo;
        let ancho = pixpin_pin::panel_abiertos::ancho_panel(e);
        let margen = (16.0 * e) as i32;
        let alto = ((area.alto as f32 - 2.0 * margen as f32).min(880.0 * e)).max(300.0 * e) as u32;
        let rect = Rect {
            x: area.derecha() - ancho as i32 - margen,
            y: area.y + margen,
            ancho,
            alto,
        };
        let pedidos = std::rc::Rc::clone(&self.pedidos_panel);
        let hwnd_app = self.hwnd_app;
        let panel = PanelAbiertos::nuevo(
            &self.d3d,
            std::rc::Rc::clone(&self.motor),
            rect,
            escala,
            self.textos.v2.clone(),
            self.textos.colores.clone(),
            self.tema_claro,
            Box::new(move |a| {
                // Solo se apunta: el panel no puede verse cerrado a media
                // llamada, y el almacen se toca fuera, en `purgar`.
                pedidos.borrow_mut().push(a);
                pixpin_shell::despertar(hwnd_app);
            }),
        )
        .context("no se pudo abrir el panel de pines")?;
        self.panel = Some(panel);
        self.refrescar_panel();
        tracing::info!("panel de pines abiertos");
        Ok(())
    }

    /// Las filas de ahora: todo lo que el almacen da por abierto y los pines
    /// en vivo.
    fn filas_panel(&self) -> Vec<FilaPanel> {
        let t = &self.textos.v2;
        let a = self.almacen.borrow();
        let mut filas = Vec::new();
        for e in a.entradas().iter().filter(|e| e.pin.is_some()) {
            let pin = self.vivos.get(&e.id);
            let oculto = match pin {
                Some(p) => self.escondidos.contains(&e.id) || !p.visible(),
                None => true,
            };
            let grupo = a.grupo_de(e.id).map(|g| g.color.indice());
            let opacidad = pin.map_or(e.pin.map_or(100, |g| g.opacidad), |p| p.opacidad());
            let con_opacidad = |base: &str| {
                if opacidad < 100 {
                    format!("{base} · {} {opacidad} %", t.opacidad.to_lowercase())
                } else {
                    base.to_string()
                }
            };
            let (tipo, nombre, detalle) = match e.tipo {
                TipoEntrada::Imagen => (
                    TipoFila::Foto,
                    format!("{} · {}", t.tipo_foto, e.creado.get(..10).unwrap_or("")),
                    con_opacidad(&t.tipo_foto),
                ),
                TipoEntrada::Nota => {
                    let texto = std::fs::read_to_string(a.ruta_objeto(e)).unwrap_or_default();
                    let herramienta =
                        self.herramientas.contains_key(&e.id) || e.origen.starts_with("mini");
                    let nombre = nombre_de_nota(&texto);
                    if herramienta {
                        (TipoFila::Herramienta, nombre, t.tipo_herramienta.clone())
                    } else {
                        (TipoFila::Nota, nombre, con_opacidad(&t.tipo_nota))
                    }
                }
                TipoEntrada::Archivo => {
                    let ruta = e.ruta.clone().unwrap_or_default();
                    let nombre = ruta
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let tipo = tipo_de_ruta(&ruta);
                    let detalle = match tipo {
                        TipoFila::Pdf => match pin.and_then(|p| p.paginas().map(|n| (p.pagina(), n))) {
                            Some((p, n)) => format!("{} · {}/{}", t.tipo_pdf, p + 1, n),
                            None => t.tipo_pdf.clone(),
                        },
                        TipoFila::Video => t.tipo_video.clone(),
                        _ => t.tipo_archivo.clone(),
                    };
                    (tipo, nombre, detalle)
                }
            };
            filas.push(FilaPanel {
                id: e.id,
                nombre: if nombre.is_empty() {
                    detalle.clone()
                } else {
                    nombre
                },
                detalle,
                tipo,
                grupo,
                oculto,
            });
        }
        let mut en_vivo: Vec<_> = self.en_vivo.iter().collect();
        en_vivo.sort_by_key(|(id, _)| **id);
        for (id, (pin, _)) in en_vivo {
            filas.push(FilaPanel {
                id: *id,
                nombre: t.tipo_vivo.clone(),
                detalle: t.en_vivo.clone(),
                tipo: TipoFila::Vivo,
                grupo: None,
                oculto: !pin.visible(),
            });
        }
        filas
    }

    /// Vuelve a pintar el panel con lo de ahora, si esta abierto.
    pub(super) fn refrescar_panel(&self) {
        if let Some(p) = &self.panel {
            p.poner_filas(self.filas_panel(), self.deshacer_cierre.is_some());
        }
    }

    /// Atiende lo que pidio el panel. Lo llama `purgar`.
    pub(super) fn atender_panel(&mut self, a: AccionPanel) -> Result<()> {
        // Todo lo que no sea deshacer olvida el ultimo «Cerrar todos»: deshacer
        // despues de haber cambiado otras cosas traeria sorpresas.
        if !matches!(a, AccionPanel::Deshacer | AccionPanel::CerrarPanel) {
            self.deshacer_cierre = None;
        }
        match a {
            AccionPanel::CerrarPanel => {
                self.panel = None;
                return Ok(());
            }
            AccionPanel::Traer(id) => {
                self.ensenar_uno(id)?;
                self.centrar(id);
                if let Some(p) = self.pin_de(id) {
                    p.resaltar();
                }
            }
            AccionPanel::Alternar(id) => {
                let a_la_vista = match (self.vivos.get(&id), self.en_vivo.get(&id)) {
                    (Some(p), _) => p.visible() && !self.escondidos.contains(&id),
                    (None, Some((p, _))) => p.visible(),
                    (None, None) => false,
                };
                if a_la_vista {
                    if let Some(p) = self.vivos.get(&id) {
                        p.esconder(true);
                        self.escondidos.insert(id);
                    } else if let Some((p, _)) = self.en_vivo.get(&id) {
                        p.esconder(true);
                    }
                } else {
                    self.ensenar_uno(id)?;
                }
            }
            AccionPanel::Cerrar(id) => self.cerrar_desde_panel(id),
            AccionPanel::MostrarTodos => {
                for id in std::mem::take(&mut self.escondidos) {
                    if let Some(p) = self.vivos.get(&id) {
                        p.esconder(false);
                    }
                }
                // Tambien los grupos ocultos: «todos» es todos.
                let ocultos: Vec<u32> = self
                    .almacen
                    .borrow()
                    .grupos()
                    .iter()
                    .filter(|g| g.oculto)
                    .map(|g| g.id)
                    .collect();
                for g in ocultos {
                    self.almacen.borrow_mut().poner_grupo_oculto(g, false).ok();
                }
                let d = pixpin_capture::enumerar_monitores().context("sin monitores")?;
                self.mostrar_todos(&d);
            }
            AccionPanel::OcultarTodos => {
                self.ocultar_todos();
            }
            AccionPanel::CerrarTodos => {
                let abiertos: Vec<(u64, PinGuardado)> = self
                    .almacen
                    .borrow()
                    .entradas()
                    .iter()
                    .filter_map(|e| e.pin.map(|g| (e.id, g)))
                    .collect();
                for (id, _) in &abiertos {
                    self.cerrar_desde_panel(*id);
                }
                // Los en vivo no tienen sitio que recordar: se cierran sin mas.
                self.en_vivo.clear();
                tracing::info!(cuantos = abiertos.len(), "pines cerrados desde el panel");
                self.deshacer_cierre = Some(abiertos);
            }
            AccionPanel::Deshacer => {
                if let Some(cerrados) = self.deshacer_cierre.take() {
                    for (id, g) in cerrados {
                        let sigue = self
                            .almacen
                            .borrow()
                            .entradas()
                            .iter()
                            .any(|e| e.id == id && e.pin.is_none());
                        if sigue {
                            self.almacen.borrow_mut().actualizar_pin(id, Some(g)).ok();
                        }
                    }
                    let d = pixpin_capture::enumerar_monitores().context("sin monitores")?;
                    self.restaurar(&d);
                }
            }
        }
        self.refrescar_panel();
        Ok(())
    }

    fn pin_de(&self, id: u64) -> Option<&pixpin_pin::Pin> {
        self.vivos
            .get(&id)
            .or_else(|| self.en_vivo.get(&id).map(|(p, _)| p))
    }

    /// Lo devuelve a la pantalla donde estaba, sea cual sea la razon por la
    /// que no se veia.
    fn ensenar_uno(&mut self, id: u64) -> Result<()> {
        if let Some((p, _)) = self.en_vivo.get(&id) {
            p.esconder(false);
            return Ok(());
        }
        if let Some(p) = self.vivos.get(&id) {
            p.esconder(false);
            self.escondidos.remove(&id);
            return Ok(());
        }
        let d = pixpin_capture::enumerar_monitores().context("sin monitores")?;
        self.restaurar_donde(&d, Some(id));
        Ok(())
    }

    /// Lo pone en el centro del monitor del panel, para que se vea si
    /// estaba tapado o fuera de la pantalla, y guarda el sitio nuevo.
    fn centrar(&mut self, id: u64) {
        let Some(panel) = &self.panel else { return };
        let Ok(d) = pixpin_capture::enumerar_monitores() else {
            return;
        };
        let rect_panel = panel.rect();
        let Some(m) = d
            .monitores()
            .iter()
            .find(|m| m.area.interseccion(rect_panel).is_some())
            .or_else(|| d.principal())
        else {
            return;
        };
        let area = m.area_trabajo;
        let Some(pin) = self.pin_de(id) else { return };
        let nuevo = centrado(pin.rect_contenido(), area);
        pin.poner_rect(nuevo);
        if let Some(mut g) = posicion_guardada(&self.almacen, id) {
            g.x = nuevo.x;
            g.y = nuevo.y;
            self.almacen.borrow_mut().actualizar_pin(id, Some(g)).ok();
        }
    }

    /// Cierra un pin (con su sitio apuntado para «devolver el ultimo
    /// cerrado»), tenga ventana o no.
    fn cerrar_desde_panel(&mut self, id: u64) {
        if self.en_vivo.remove(&id).is_some() {
            return;
        }
        if let Some(g) = posicion_guardada(&self.almacen, id) {
            self.reabrir.borrow_mut().push((id, g));
        }
        if let Err(e) = self.almacen.borrow_mut().actualizar_pin(id, None) {
            tracing::warn!(?e, id, "no se pudo marcar el pin como cerrado");
        }
        self.vivos.remove(&id);
        self.escondidos.remove(&id);
        self.herramientas.remove(&id);
        self.pizarras.remove(&id);
    }
}


#[cfg(test)]
mod pruebas {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn el_tipo_de_un_fichero_sale_de_su_extension() {
        assert_eq!(tipo_de_ruta(&PathBuf::from("C:\\a\\Tesis.PDF")), TipoFila::Pdf);
        assert_eq!(tipo_de_ruta(&PathBuf::from("clase.mp4")), TipoFila::Video);
        // Caso negativo: lo demas es una ficha, aunque se parezca.
        assert_eq!(tipo_de_ruta(&PathBuf::from("informe.docx")), TipoFila::Archivo);
        assert_eq!(tipo_de_ruta(&PathBuf::from("sin_extension")), TipoFila::Archivo);
    }

    #[test]
    fn el_nombre_de_una_nota_es_su_primera_linea_sin_markdown() {
        assert_eq!(nombre_de_nota("\n\n# Compras temu\n- nivel"), "Compras temu");
        assert_eq!(nombre_de_nota("- [ ] pan"), "[ ] pan");
        // Caso negativo: una nota vacia no se inventa nombre.
        assert_eq!(nombre_de_nota("   \n  "), "");
        let larga = "x".repeat(80);
        let n = nombre_de_nota(&larga);
        assert_eq!(n.chars().count(), 61, "60 y los puntos");
    }

    #[test]
    fn traer_al_centro_y_caso_negativo_uno_enorme_va_a_la_esquina() {
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        let pin = Rect {
            x: 5000,
            y: -300,
            ancho: 400,
            alto: 200,
        };
        assert_eq!(
            centrado(pin, area),
            Rect {
                x: 760,
                y: 420,
                ancho: 400,
                alto: 200
            }
        );
        let enorme = Rect {
            ancho: 3000,
            alto: 2000,
            ..pin
        };
        let c = centrado(enorme, area);
        assert_eq!((c.x, c.y), (0, 0));
        assert_eq!((c.ancho, c.alto), (3000, 2000), "no se le cambia el tamano");
    }
}
