//! Lo que el gestor hace con las herramientas pineadas: sacarlas (palabra
//! magica, «Convertir en…»), atender sus toques y teclas, guardarlas, el
//! fondo de la pizarra y el panel de propiedades de la anotacion.
//!
//! Va aparte de `pines.rs` solo por tamano: es el mismo `impl Pines`, y como
//! modulo hijo ve sus campos sin hacerlos publicos.

use std::path::PathBuf;
use std::rc::Rc;

use anyhow::{Context, Result};
use pixpin_codec::codificar_png;
use pixpin_geom::{Monitor, Punto, Rect, recolocar_en_area};
use pixpin_pin::magia::{self, Clasificado, MiniApp};
use pixpin_pin::{CambioPin, Contenido, Paleta};

use super::herramienta::{self, Cuadro, Hecho, Herramienta};
use super::{Pines, pizarra, textos};

/// Lo que se guarda junto a la imagen de una pizarra (`<objeto>.pizarra`).
const EXTENSION_PIZARRA: &str = "pizarra";

/// Escribe `bytes` en `ruta` sin dejar nunca un fichero a medias: temporal y
/// renombrar, como el indice del almacen.
fn reescribir(ruta: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let temporal = ruta.with_extension("tmp");
    std::fs::write(&temporal, bytes)
        .with_context(|| format!("no se pudo escribir {}", temporal.display()))?;
    std::fs::rename(&temporal, ruta)
        .with_context(|| format!("no se pudo reemplazar {}", ruta.display()))?;
    Ok(())
}

impl Pines {
    /// **Un texto del portapapeles**: una palabra magica sola saca su
    /// herramienta, una tabla pegada sale como tabla y lo demas como nota
    /// (`ContentClassifier` del movil).
    pub fn pinear_texto(&mut self, texto: &str, monitor: &Monitor) -> Result<()> {
        match magia::clasificar(texto, &self.palabras) {
            Clasificado::Herramienta(app) => {
                tracing::info!(herramienta = app.nombre(), "palabra magica");
                self.pinear_herramienta(app, "", None, monitor).map(|_| ())
            }
            Clasificado::Tabla => self
                .pinear_documento(herramienta::TABLA, texto, None, monitor)
                .map(|_| ()),
            Clasificado::Texto | Clasificado::Vacio => self.pinear_nota(texto, monitor),
        }
    }

    /// Saca la herramienta `app`, sembrada con `texto`. `donde` es la esquina
    /// en la que nace (la de la nota que se convierte); sin ella, centrada.
    pub fn pinear_herramienta(
        &mut self,
        app: MiniApp,
        texto: &str,
        donde: Option<Punto>,
        monitor: &Monitor,
    ) -> Result<u64> {
        match herramienta::cual_de(app) {
            Some(cual) => self.pinear_documento(cual, texto, donde, monitor),
            None => self.pinear_dibujo(app, donde, monitor),
        }
    }

    /// Donde nace un pin de tamano `(w, h)`: en `donde` si se da, y si no
    /// centrado; siempre dentro del area de trabajo.
    fn sitio(
        &self,
        contenido: &Contenido,
        donde: Option<Punto>,
        monitor: &Monitor,
    ) -> Rect {
        let centrada = self.region_centrada(contenido, monitor);
        match donde {
            Some(p) => recolocar_en_area(
                Rect {
                    x: p.x,
                    y: p.y,
                    ..centrada
                },
                monitor.area_trabajo,
            ),
            None => centrada,
        }
    }

    /// Una mini-app o una tabla: su documento va al almacen como nota con
    /// el origen que dice que es, y el pin la pinta.
    fn pinear_documento(
        &mut self,
        cual: &str,
        texto: &str,
        donde: Option<Punto>,
        monitor: &Monitor,
    ) -> Result<u64> {
        let moneda = herramienta::moneda(textos());
        let documento = herramienta::documento_nuevo(cual, texto, &moneda)
            .context("esa herramienta no se sabe sacar")?;
        let h = Herramienta::nueva(cual, documento.clone());
        let (ancho, alto) = herramienta::tamano_natural(&h, monitor.escala_por_cien, &moneda);
        let contenido = Contenido::Herramienta { ancho, alto };
        let region = self.sitio(&contenido, donde, monitor);
        let id = self
            .almacen
            .borrow_mut()
            .guardar_nota(
                &documento,
                &herramienta::origen_de(cual),
                Some(Pines::guardado_desde(region, monitor.escala_por_cien, 100)),
            )
            .context("no se pudo guardar la herramienta")?;
        self.herramientas.insert(id, h);
        self.crear_ventana(id, contenido, region, monitor.escala_por_cien)?;
        tracing::info!(id, cual, "herramienta pineada");
        Ok(id)
    }

    /// La pizarra, el lienzo y la hoja: pines de imagen con su fondo hecho
    /// aqui y lo dibujado encima, como en el movil.
    fn pinear_dibujo(&mut self, app: MiniApp, donde: Option<Punto>, monitor: &Monitor) -> Result<u64> {
        let (imagen, origen) = match app {
            MiniApp::Pizarra => (pizarra::fondo(0, 0), "pizarra"),
            MiniApp::Hoja => (pizarra::papel_de_la_hoja(), "hoja"),
            _ => (pizarra::papel_del_lienzo(), "lienzo"),
        };
        let contenido = Contenido::Imagen(imagen.clone());
        let region = self.sitio(&contenido, donde, monitor);
        let png = codificar_png(&imagen).context("no se pudo codificar el papel")?;
        let id = self
            .almacen
            .borrow_mut()
            .guardar_imagen(
                &png,
                origen,
                Some(Pines::guardado_desde(region, monitor.escala_por_cien, 100)),
            )
            .context("no se pudo guardar el papel")?;
        match app {
            MiniApp::Pizarra => {
                if let Some(ruta) = self.ruta_pizarra(id) {
                    reescribir(&ruta, pizarra::escribir_estado(0, 0).as_bytes())?;
                }
                self.pizarras.insert(id, (0, 0));
            }
            MiniApp::Hoja => {
                // La hoja nace con su marco de papel ya dibujado: es lo que
                // la distingue del lienzo (`escenaConHoja`).
                let ruta = self
                    .ruta_anotacion(id)
                    .context("la hoja no tiene donde guardar su dibujo")?;
                pixpin_motor2d::guardar(
                    &ruta,
                    &pizarra::escena_de_la_hoja(&textos().t("pin-hoja-nombre")),
                )
                .context("no se pudo guardar la hoja")?;
            }
            _ => {}
        }
        self.crear_ventana(id, contenido, region, monitor.escala_por_cien)?;
        // El lienzo y la hoja abren su editor ya: la palabra es la orden de
        // ponerse a dibujar, no la de mirar un papel en blanco en un pin
        // (`DrawEditorActivity.abrir` del movil). La pizarra se anota dentro.
        if matches!(app, MiniApp::Lienzo | MiniApp::Hoja) {
            self.pedir_lienzo(id)?;
        }
        tracing::info!(id, origen, "papel pineado");
        Ok(id)
    }

    /// El fichero que dice de que color y pauta es una pizarra.
    fn ruta_pizarra(&self, id: u64) -> Option<PathBuf> {
        let a = self.almacen.borrow();
        let e = a.entradas().iter().find(|e| e.id == id)?;
        (!e.objeto.is_empty()).then(|| a.ruta_objeto(e).with_extension(EXTENSION_PIZARRA))
    }

    /// Lo que se sabe de un pin al restaurarlo: si es una herramienta, su
    /// estado; si es una pizarra, su fondo. `None` si es un pin corriente.
    pub(super) fn reconocer_al_restaurar(
        &mut self,
        id: u64,
        origen: &str,
        texto: Option<&str>,
    ) -> Option<Contenido> {
        if let Some(cual) = herramienta::cual_de_origen(origen) {
            let h = Herramienta::nueva(cual, texto?.to_string());
            let (ancho, alto) = herramienta::tamano_natural(&h, 100, &herramienta::moneda(textos()));
            self.herramientas.insert(id, h);
            return Some(Contenido::Herramienta { ancho, alto });
        }
        if origen == "pizarra" {
            let estado = self
                .ruta_pizarra(id)
                .and_then(|r| std::fs::read_to_string(r).ok())
                .map_or((0, 0), |t| pizarra::leer_estado(&t));
            self.pizarras.insert(id, estado);
        }
        None
    }

    /// Cuelga en el pin recien creado lo que le toca: el pintor y el latido
    /// de una herramienta, o el submenu de una pizarra.
    pub(super) fn vestir(&self, id: u64) {
        if self.herramientas.contains_key(&id) {
            self.colgar_herramienta(id);
        } else if let (Some(pin), Some(estado)) = (self.vivos.get(&id), self.pizarras.get(&id)) {
            pin.poner_pizarra(Some(*estado));
        }
    }

    /// Le da al pin lo que tiene que pintar ahora y su latido.
    fn colgar_herramienta(&self, id: u64) {
        let (Some(pin), Some(h)) = (self.vivos.get(&id), self.herramientas.get(&id)) else {
            return;
        };
        let moneda = herramienta::moneda(textos());
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        pin.poner_latido(h.late_cada_ms(&moneda, ahora));
        let cuadro = Cuadro {
            herramienta: h.clone(),
            escala: pin.escala_por_cien(),
            claro: self.tema_claro,
            moneda,
        };
        pin.poner_pintor_interior(Box::new(move |p, caja| cuadro.pintar(p, caja, textos())));
    }

    /// El documento de una herramienta, a su nota del almacen.
    fn guardar_herramienta(&self, id: u64) -> Result<()> {
        let Some(h) = self.herramientas.get(&id) else {
            return Ok(());
        };
        let ruta = {
            let a = self.almacen.borrow();
            let e = a
                .entradas()
                .iter()
                .find(|e| e.id == id)
                .context("la herramienta ya no esta en el almacen")?;
            a.ruta_objeto(e)
        };
        reescribir(&ruta, h.documento.as_bytes())
    }

    /// Cierra un pin como lo cierra su Esc: se apunta donde estaba para
    /// poder devolverlo y el almacen lo da por cerrado.
    fn cerrar_uno(&mut self, id: u64) {
        if let Some(g) = super::posicion_guardada(&self.almacen, id) {
            self.reabrir.borrow_mut().push((id, g));
        }
        if let Err(e) = self.almacen.borrow_mut().actualizar_pin(id, None) {
            tracing::warn!(?e, id, "no se pudo marcar el pin como cerrado");
        }
        self.vivos.remove(&id);
        self.herramientas.remove(&id);
        self.pizarras.remove(&id);
    }

    /// Lo que pide una herramienta o una pizarra. Devuelve `false` si el
    /// pedido no era de estos, para que siga su camino.
    pub(super) fn atender_herramienta(&mut self, id: u64, cambio: CambioPin) -> Result<bool> {
        match cambio {
            CambioPin::ConvertirPedido(i) => {
                let app = MiniApp::por_indice(i).context("herramienta fuera de la lista")?;
                self.convertir(id, app)?;
            }
            CambioPin::PizarraPedida { color, pauta } => self.pizarra_pedida(id, color, pauta)?,
            CambioPin::PanelPulsado(p) => self.panel_pulsado(id, p)?,
            CambioPin::ClicInterior { .. }
            | CambioPin::RuedaInterior { .. }
            | CambioPin::TeclaInterior { .. }
            | CambioPin::CaracterInterior(_)
            | CambioPin::Latido => self.dentro(id, cambio)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Un toque, una tecla, la rueda o el latido de una herramienta.
    fn dentro(&mut self, id: u64, cambio: CambioPin) -> Result<()> {
        let Some(pin) = self.vivos.get(&id) else {
            return Ok(());
        };
        let rect = pin.rect_contenido();
        let tam = (rect.ancho, rect.alto);
        let escala = pin.escala_por_cien();
        let moneda = herramienta::moneda(textos());
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        let azar = pixpin_motor2d::azar::Azar::nuevo(ahora as u32).siguiente() as f64;
        let Some(h) = self.herramientas.get_mut(&id) else {
            return Ok(());
        };
        let hecho = match cambio {
            CambioPin::ClicInterior { x, y } => {
                let e = h.clic(Punto { x, y }, tam, escala, &moneda, ahora);
                h.cumplir(e, tam, escala, &moneda, ahora, azar)
            }
            CambioPin::TeclaInterior { vk, shift, ctrl, alt } => h.tecla(
                crate::mini_panel::Tecla {
                    vk,
                    shift,
                    ctrl,
                    alt,
                },
                tam,
                escala,
                &moneda,
                ahora,
                azar,
            ),
            CambioPin::CaracterInterior(c) => h.caracter(c, tam, escala, &moneda, ahora, azar),
            CambioPin::RuedaInterior { delta } => {
                if h.rueda(delta, tam, escala, &moneda, ahora) {
                    Hecho::Repintar
                } else {
                    Hecho::Nada
                }
            }
            CambioPin::Latido => {
                // El pin ya se repinto solo; aqui solo el aviso del
                // temporizador (`onTimerFinished`: el globo en vez del toast).
                if h.vencio(&moneda, ahora) {
                    let mut titulo = pixpin_proyecto::mini::titulo(&h.documento);
                    if titulo.is_empty() {
                        titulo = textos().t(herramienta::clave_del_nombre(&h.cual));
                    }
                    let mut aviso = pixpin_shell::aviso::Aviso::sobre_la_bandeja(self.hwnd_app);
                    if let Err(e) = aviso.mostrar(&titulo, &textos().t("pin-tiempo-cumplido")) {
                        tracing::warn!(?e, "no se pudo avisar del temporizador");
                    }
                    // Ya en cero: el latido se apaga.
                    Hecho::Repintar
                } else {
                    Hecho::Nada
                }
            }
            _ => Hecho::Nada,
        };
        match hecho {
            Hecho::Nada => {}
            Hecho::Repintar => self.colgar_herramienta(id),
            Hecho::Guardar => {
                self.colgar_herramienta(id);
                if let Err(e) = self.guardar_herramienta(id) {
                    tracing::warn!(?e, id, "no se pudo guardar la herramienta");
                }
            }
            Hecho::Cerrar => self.cerrar_uno(id),
        }
        Ok(())
    }

    /// «Convertir en…» una nota: la herramienta nace en su sitio con sus
    /// lineas, y la nota se cierra (se puede devolver con «restaurar el
    /// ultimo cerrado», como cualquier pin cerrado).
    fn convertir(&mut self, id: u64, app: MiniApp) -> Result<()> {
        let (texto, rect, escala) = {
            let pin = self.vivos.get(&id).context("la nota ya no esta abierta")?;
            let a = self.almacen.borrow();
            let e = a
                .entradas()
                .iter()
                .find(|e| e.id == id)
                .context("la nota ya no esta en el almacen")?;
            let texto = std::fs::read_to_string(a.ruta_objeto(e)).unwrap_or_default();
            (texto, pin.rect_contenido(), pin.escala_por_cien())
        };
        let disposicion = pixpin_capture::enumerar_monitores().context("sin monitores")?;
        let monitor = disposicion
            .monitores()
            .iter()
            .find(|m| m.area.interseccion(rect).is_some())
            .or_else(|| disposicion.principal())
            .copied()
            .context("sin monitor donde convertir")?;
        let donde = Punto {
            x: rect.x,
            y: rect.y,
        };
        let nuevo = self.pinear_herramienta(app, &texto, Some(donde), &monitor)?;
        tracing::info!(id, nuevo, herramienta = app.nombre(), escala, "nota convertida");
        self.cerrar_uno(id);
        Ok(())
    }

    /// Cambia el fondo de una pizarra: se rehace la imagen del mismo tamano
    /// y lo dibujado encima se queda (`regenerateBoard` del movil).
    fn pizarra_pedida(&mut self, id: u64, color: u8, pauta: u8) -> Result<()> {
        let imagen = pizarra::fondo(color, pauta);
        let png = codificar_png(&imagen).context("no se pudo codificar la pizarra")?;
        let objeto = {
            let a = self.almacen.borrow();
            let e = a
                .entradas()
                .iter()
                .find(|e| e.id == id)
                .context("la pizarra ya no esta en el almacen")?;
            a.ruta_objeto(e)
        };
        // Primero el disco y luego la pantalla: si no se pudo guardar, el pin
        // sigue ensenando lo que de verdad hay.
        reescribir(&objeto, &png)?;
        if let Some(ruta) = self.ruta_pizarra(id) {
            reescribir(&ruta, pizarra::escribir_estado(color, pauta).as_bytes())?;
        }
        self.pizarras.insert(id, (color, pauta));
        if let Some(pin) = self.vivos.get(&id) {
            pin.poner_imagen(imagen);
            pin.poner_pizarra(Some((color, pauta)));
        }
        tracing::info!(id, color, pauta, "fondo de la pizarra cambiado");
        Ok(())
    }

    // --- El panel de propiedades de la anotacion -----------------------

    /// La ventana del panel, creada escondida: sale en cuanto hay algo que
    /// ajustar (`repintar_panel`).
    pub(super) fn panel_nuevo(&self, id: u64) -> Result<Paleta> {
        let pedidos = Rc::clone(&self.pedidos);
        let hwnd_app = self.hwnd_app;
        let panel = Paleta::nueva(
            &self.d3d,
            Rc::clone(&self.motor),
            Rect {
                x: -10_000,
                y: -10_000,
                ancho: 1,
                alto: 1,
            },
            Box::new(move |p| {
                pedidos.borrow_mut().push((id, CambioPin::PanelPulsado(p)));
                pixpin_shell::despertar(hwnd_app);
            }),
        )
        .context("no se pudo crear el panel del pin")?;
        panel.recolocar(None);
        Ok(panel)
    }

    /// El `area` del panel para el pin que se anota, en el escritorio.
    fn area_del_panel(&self) -> Option<(Rect, u32)> {
        let a = self.anotacion.as_ref()?;
        let pin = self.vivos.get(&a.id)?;
        let trabajo = a.trabajo;
        Some((
            super::panel::area_para(pin.rect_contenido(), trabajo, a.escala_por_cien),
            a.escala_por_cien,
        ))
    }

    /// Vuelve a montar el panel con lo de ahora (la herramienta, lo elegido)
    /// y lo pone donde toca, o lo esconde si no hay nada que ajustar.
    pub(super) fn repintar_panel(&self) {
        let Some((area, escala)) = self.area_del_panel() else {
            return;
        };
        let Some(a) = &self.anotacion else {
            return;
        };
        match crate::panel_dibujo::panel_para(&a.gesto, &a.escena, area, escala) {
            None => a.panel.recolocar(None),
            Some(panel) => {
                let marco = super::panel::marco_de(&panel, escala);
                a.panel.recolocar(Some(marco));
                a.panel.poner_pintor(Box::new(move |p| {
                    p.desplazar(-(marco.x as f32), -(marco.y as f32));
                    crate::panel_dibujo::pintar(p, &panel, escala);
                    p.desplazar(0.0, 0.0);
                }));
            }
        }
    }

    /// Un clic en el panel, en coordenadas de su ventana: lo lleva por el
    /// mismo camino que el lienzo (`panel_dibujo::atender_clic`), pulsar y
    /// soltar, porque la ventanita solo avisa al soltar.
    fn panel_pulsado(&mut self, id: u64, p: Punto) -> Result<()> {
        let Some((area, escala)) = self.area_del_panel() else {
            return Ok(());
        };
        let Some(a) = self.anotacion.as_mut().filter(|a| a.id == id) else {
            return Ok(());
        };
        let Some(panel) = crate::panel_dibujo::panel_para(&a.gesto, &a.escena, area, escala) else {
            return Ok(());
        };
        let marco = super::panel::marco_de(&panel, escala);
        let global = Punto {
            x: p.x + marco.x,
            y: p.y + marco.y,
        };
        use crate::panel_dibujo::ClicPanel;
        let pulsado = crate::panel_dibujo::atender_clic(
            global,
            true,
            &mut a.gesto,
            &mut a.escena,
            area,
            escala,
        );
        let _ = crate::panel_dibujo::atender_clic(
            global,
            false,
            &mut a.gesto,
            &mut a.escena,
            area,
            escala,
        );
        // Un color nuevo para lo elegido cambia el dibujo; uno para la
        // herramienta, solo el panel. Se repintan los dos: es un clic.
        if matches!(pulsado, ClicPanel::Suyo { repintar: true } | ClicPanel::Fuera { cerro: true })
        {
            self.repintar_anotacion(id);
        }
        self.repintar_panel();
        Ok(())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn reescribir_deja_el_fichero_nuevo_entero_y_sin_temporal() {
        let dir = std::env::temp_dir().join(format!("pixpin-sacar-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("000001.txt");
        std::fs::write(&ruta, "viejo").unwrap();
        reescribir(&ruta, b"# Compra\n\n- [ ] pan").unwrap();
        assert_eq!(std::fs::read_to_string(&ruta).unwrap(), "# Compra\n\n- [ ] pan");
        assert!(!ruta.with_extension("tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reescribir_en_una_carpeta_que_no_existe_falla_sin_crear_nada() {
        let ruta = std::env::temp_dir()
            .join("pixpin-no-existe-jamas-8812")
            .join("x.txt");
        assert!(reescribir(&ruta, b"x").is_err());
        assert!(!ruta.exists());
    }

    #[test]
    fn la_imagen_de_la_pizarra_se_codifica_y_vuelve_igual() {
        let img: pixpin_codec::ImagenRgba = pizarra::fondo(1, 4);
        let png = codificar_png(&img).unwrap();
        let ruta = std::env::temp_dir().join(format!("pixpin-pizarra-{}.png", std::process::id()));
        std::fs::write(&ruta, &png).unwrap();
        let vuelta = pixpin_codec::cargar(&ruta).unwrap();
        std::fs::remove_file(&ruta).unwrap();
        assert_eq!((vuelta.ancho, vuelta.alto), (img.ancho, img.alto));
        assert_eq!(vuelta.pixeles, img.pixeles);
    }
}
