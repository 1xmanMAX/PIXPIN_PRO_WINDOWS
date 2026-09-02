//! El gestor de pines del ejecutable: la UNICA pieza que ve a la vez el
//! almacen (pixpin-store) y las ventanas (pixpin-pin), porque ambos son L2
//! y no pueden verse entre si. D21 en codigo: todo pasa por el almacen
//! primero; el Pin es la vista.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{Context, Result};
use pixpin_codec::{ImagenRgba, cargar, codificar_png};
use pixpin_geom::{DisposicionMonitores, Monitor, Rect, recolocar_en_area};
use pixpin_pin::{CambioPin, Contenido, Pin, icono_de, tamano_humano, tamano_natural};
use pixpin_render::MotorRender;
use pixpin_store::{Almacen, PinGuardado, TipoEntrada};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

/// La ficha de un archivo: icono real, nombre y tamano, o el aviso de que
/// la ruta ya no lleva a ninguna parte (D28).
fn ficha_de(ruta: &Path, texto_no_encontrado: &str) -> Contenido {
    let nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| ruta.to_string_lossy().into_owned());
    let metadatos = std::fs::metadata(ruta).ok();
    let existe = metadatos.is_some();
    let detalle = match &metadatos {
        Some(m) if m.is_dir() => "—".to_string(),
        Some(m) => tamano_humano(m.len()),
        None => texto_no_encontrado.to_string(),
    };
    Contenido::Archivo {
        nombre,
        detalle,
        icono: icono_de(ruta),
        existe,
    }
}

pub struct Pines {
    almacen: Rc<RefCell<Almacen>>,
    d3d: ID3D11Device,
    motor: Rc<MotorRender>,
    vivos: HashMap<u64, Pin>,
    /// Ids cerrados desde los callbacks; purgar() los drena en el bucle.
    cerrados: Rc<RefCell<Vec<u64>>>,
    /// Se lee una vez al arrancar (D33): un pin nuevo nace con el tema del
    /// momento, y los ya abiertos no cambian de color a media sesion.
    tema_claro: bool,
    /// Ya traducido: `pixpin-pin` no conoce el catalogo de idiomas.
    texto_no_encontrado: String,
}

impl Pines {
    pub fn nuevos(
        raiz: &Path,
        d3d: ID3D11Device,
        motor: Rc<MotorRender>,
        texto_no_encontrado: String,
    ) -> Result<Pines> {
        let almacen = Almacen::abrir(raiz).context("no se pudo abrir el almacen")?;
        Ok(Pines {
            almacen: Rc::new(RefCell::new(almacen)),
            d3d,
            motor,
            vivos: HashMap::new(),
            cerrados: Rc::new(RefCell::new(Vec::new())),
            tema_claro: pixpin_shell::entorno::tema_claro(),
            texto_no_encontrado,
        })
    }

    fn guardado_desde(region: Rect, escala: u32) -> PinGuardado {
        PinGuardado {
            x: region.x,
            y: region.y,
            ancho: region.ancho,
            alto: region.alto,
            escala_por_cien: escala,
        }
    }

    fn crear_ventana(
        &mut self,
        id: u64,
        contenido: Contenido,
        region: Rect,
        escala: u32,
    ) -> Result<()> {
        let almacen = Rc::clone(&self.almacen);
        let cerrados = Rc::clone(&self.cerrados);
        let pin = Pin::nuevo(
            &self.d3d,
            Rc::clone(&self.motor),
            contenido,
            region,
            escala,
            self.tema_claro,
            Box::new(move |cambio| {
                let resultado = match cambio {
                    CambioPin::Movido(r) | CambioPin::Redimensionado(r) => almacen
                        .borrow_mut()
                        .actualizar_pin(id, Some(Pines::guardado_desde(r, escala))),
                    CambioPin::Cerrado => {
                        cerrados.borrow_mut().push(id);
                        almacen.borrow_mut().actualizar_pin(id, None)
                    }
                };
                if let Err(e) = resultado {
                    // Perder una posicion no puede tumbar el pin: se
                    // registra y se sigue (el contenido ya esta a salvo).
                    tracing::warn!(?e, id, "no se pudo persistir el cambio del pin");
                }
            }),
        )
        .context("no se pudo crear la ventana del pin")?;
        self.vivos.insert(id, pin);
        Ok(())
    }

    /// D26: el recorte queda flotando 1:1 exactamente donde estaba.
    pub fn pinear(&mut self, imagen: &ImagenRgba, region: Rect, escala: u32) -> Result<()> {
        let png = codificar_png(imagen).context("no se pudo codificar el pin")?;
        let id = self
            .almacen
            .borrow_mut()
            .guardar_imagen(&png, "recorte", Some(Pines::guardado_desde(region, escala)))
            .context("no se pudo guardar en el almacen")?;
        self.crear_ventana(id, Contenido::Imagen(imagen.clone()), region, escala)
    }

    /// Una nota del portapapeles: nace centrada en el monitor pedido (D32).
    pub fn pinear_nota(&mut self, texto: &str, monitor: &Monitor) -> Result<()> {
        let contenido = Contenido::Nota {
            texto: texto.to_string(),
        };
        let region = self.region_centrada(&contenido, monitor);
        let id = self
            .almacen
            .borrow_mut()
            .guardar_nota(
                texto,
                "portapapeles",
                Some(Pines::guardado_desde(region, monitor.escala_por_cien)),
            )
            .context("no se pudo guardar la nota")?;
        self.crear_ventana(id, contenido, region, monitor.escala_por_cien)
    }

    /// La ficha de un archivo o carpeta, por referencia (D28).
    pub fn pinear_archivo(&mut self, ruta: &Path, monitor: &Monitor) -> Result<()> {
        let contenido = ficha_de(ruta, &self.texto_no_encontrado);
        let region = self.region_centrada(&contenido, monitor);
        let id = self
            .almacen
            .borrow_mut()
            .guardar_archivo(
                ruta,
                Some(Pines::guardado_desde(region, monitor.escala_por_cien)),
            )
            .context("no se pudo guardar la referencia")?;
        self.crear_ventana(id, contenido, region, monitor.escala_por_cien)
    }

    /// Una imagen del portapapeles: no viene de ninguna region de pantalla,
    /// asi que nace centrada y, si no cabe, al 80 % del area de trabajo.
    pub fn pinear_imagen_centrada(&mut self, imagen: &ImagenRgba, monitor: &Monitor) -> Result<()> {
        let contenido = Contenido::Imagen(imagen.clone());
        let region = self.region_centrada(&contenido, monitor);
        let png = codificar_png(imagen).context("no se pudo codificar la imagen")?;
        let id = self
            .almacen
            .borrow_mut()
            .guardar_imagen(
                &png,
                "portapapeles",
                Some(Pines::guardado_desde(region, monitor.escala_por_cien)),
            )
            .context("no se pudo guardar en el almacen")?;
        self.crear_ventana(id, contenido, region, monitor.escala_por_cien)
    }

    /// Donde nace un pin que no viene de un recorte: centrado en el monitor
    /// del cursor, encogido al 80 % del area de trabajo si no cabe.
    fn region_centrada(&self, contenido: &Contenido, monitor: &Monitor) -> Rect {
        let motor = Rc::clone(&self.motor);
        let (mut w, mut h) = tamano_natural(contenido, monitor.escala_por_cien, &|t, tam, max| {
            motor.medir_texto(t, tam, max)
        });

        let tope_w = (monitor.area_trabajo.ancho as f32 * 0.8) as u32;
        let tope_h = (monitor.area_trabajo.alto as f32 * 0.8) as u32;
        if w > tope_w || h > tope_h {
            // Se encoge proporcionalmente: deformar una captura para que
            // quepa seria peor que enseñarla mas pequena.
            let f = (tope_w as f32 / w as f32).min(tope_h as f32 / h as f32);
            w = ((w as f32 * f) as u32).max(1);
            h = ((h as f32 * f) as u32).max(1);
        }

        // En cascada, no exactamente centrados: pegar tres archivos a la vez
        // ponia los tres pines en el mismo pixel y parecian uno solo. El
        // escalon se reinicia cada ocho para no bajar sin fin.
        let escalon = (self.vivos.len() % 8) as i32 * (28 * monitor.escala_por_cien / 100) as i32;

        let rect = Rect {
            x: monitor.area_trabajo.x
                + (monitor.area_trabajo.ancho as i32 - w as i32) / 2
                + escalon,
            y: monitor.area_trabajo.y + (monitor.area_trabajo.alto as i32 - h as i32) / 2 + escalon,
            ancho: w,
            alto: h,
        };
        recolocar_en_area(rect, monitor.area_trabajo)
    }

    /// Restaura los pines abiertos del almacen. Los fallos individuales se
    /// registran y no tumban el resto; devuelve cuantos volvieron.
    pub fn restaurar(&mut self, disposicion: &DisposicionMonitores) -> usize {
        struct Pendiente {
            id: u64,
            guardado: PinGuardado,
            tipo: TipoEntrada,
            objeto: PathBuf,
            ruta: Option<PathBuf>,
        }

        let pendientes: Vec<Pendiente> = {
            let a = self.almacen.borrow();
            a.entradas()
                .iter()
                .filter_map(|e| {
                    e.pin.map(|p| Pendiente {
                        id: e.id,
                        guardado: p,
                        tipo: e.tipo,
                        objeto: a.ruta_objeto(e),
                        ruta: e.ruta.clone(),
                    })
                })
                .collect()
        };
        let mut restaurados = 0;
        for p in pendientes {
            let (id, guardado) = (p.id, p.guardado);
            let contenido = match p.tipo {
                TipoEntrada::Imagen => match cargar(&p.objeto) {
                    Ok(i) => Contenido::Imagen(i),
                    Err(e) => {
                        tracing::warn!(?e, id, "pin sin objeto legible; queda solo en el almacen");
                        continue;
                    }
                },
                TipoEntrada::Nota => match std::fs::read_to_string(&p.objeto) {
                    Ok(texto) => Contenido::Nota { texto },
                    Err(e) => {
                        tracing::warn!(?e, id, "nota sin fichero legible; queda en el almacen");
                        continue;
                    }
                },
                // La referencia rota se restaura igual y se MUESTRA como "no
                // encontrado" (D28): esconderla perderia el rastro de algo
                // que el usuario dejo pineado a proposito.
                TipoEntrada::Archivo => match &p.ruta {
                    Some(r) => ficha_de(r, &self.texto_no_encontrado),
                    None => {
                        tracing::warn!(id, "entrada de archivo sin ruta; se ignora");
                        continue;
                    }
                },
            };
            let rect = Rect {
                x: guardado.x,
                y: guardado.y,
                ancho: guardado.ancho,
                alto: guardado.alto,
            };
            // Si el monitor de origen ya no existe (o el pin quedo fuera),
            // se desliza al area de trabajo mas razonable (spec 5.2).
            let (rect, escala) = match disposicion
                .monitores()
                .iter()
                .find(|m| m.area.interseccion(rect).is_some())
            {
                Some(m) => (recolocar_en_area(rect, m.area_trabajo), m.escala_por_cien),
                None => match disposicion.principal() {
                    Some(p) => (recolocar_en_area(rect, p.area_trabajo), p.escala_por_cien),
                    None => (rect, guardado.escala_por_cien),
                },
            };
            match self.crear_ventana(id, contenido, rect, escala) {
                Ok(()) => restaurados += 1,
                Err(e) => tracing::warn!(?e, id, "no se pudo restaurar el pin"),
            }
        }
        restaurados
    }

    /// Saca de la lista los pines que se cerraron desde su propio WndProc.
    /// Llamar desde el bucle principal; barato (dos punteros si esta vacia).
    pub fn purgar(&mut self) {
        let cerrados: Vec<u64> = self.cerrados.borrow_mut().drain(..).collect();
        for id in cerrados {
            self.vivos.remove(&id);
        }
    }

    pub fn abiertos(&self) -> usize {
        self.vivos.len()
    }
}
