//! El gestor de pines del ejecutable: la UNICA pieza que ve a la vez el
//! almacen (pixpin-store) y las ventanas (pixpin-pin), porque ambos son L2
//! y no pueden verse entre si. D21 en codigo: todo pasa por el almacen
//! primero; el Pin es la vista.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use anyhow::{Context, Result};
use pixpin_codec::{ImagenRgba, cargar, codificar_png};
use pixpin_geom::{DisposicionMonitores, Rect, recolocar_en_area};
use pixpin_pin::{CambioPin, Pin};
use pixpin_render::MotorRender;
use pixpin_store::{Almacen, PinGuardado};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

pub struct Pines {
    almacen: Rc<RefCell<Almacen>>,
    d3d: ID3D11Device,
    motor: Rc<MotorRender>,
    vivos: HashMap<u64, Pin>,
    /// Ids cerrados desde los callbacks; purgar() los drena en el bucle.
    cerrados: Rc<RefCell<Vec<u64>>>,
}

impl Pines {
    pub fn nuevos(raiz: &Path, d3d: ID3D11Device, motor: Rc<MotorRender>) -> Result<Pines> {
        let almacen = Almacen::abrir(raiz).context("no se pudo abrir el almacen")?;
        Ok(Pines {
            almacen: Rc::new(RefCell::new(almacen)),
            d3d,
            motor,
            vivos: HashMap::new(),
            cerrados: Rc::new(RefCell::new(Vec::new())),
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
        imagen: &ImagenRgba,
        region: Rect,
        escala: u32,
    ) -> Result<()> {
        let almacen = Rc::clone(&self.almacen);
        let cerrados = Rc::clone(&self.cerrados);
        let pin = Pin::nuevo(
            &self.d3d,
            Rc::clone(&self.motor),
            imagen,
            region,
            escala,
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
        self.crear_ventana(id, imagen, region, escala)
    }

    /// Restaura los pines abiertos del almacen. Los fallos individuales se
    /// registran y no tumban el resto; devuelve cuantos volvieron.
    pub fn restaurar(&mut self, disposicion: &DisposicionMonitores) -> usize {
        let pendientes: Vec<(u64, PinGuardado, std::path::PathBuf)> = {
            let a = self.almacen.borrow();
            a.entradas()
                .iter()
                .filter_map(|e| e.pin.map(|p| (e.id, p, a.ruta_objeto(e))))
                .collect()
        };
        let mut restaurados = 0;
        for (id, guardado, ruta) in pendientes {
            let imagen = match cargar(&ruta) {
                Ok(i) => i,
                Err(e) => {
                    tracing::warn!(?e, id, "pin sin objeto legible; queda solo en el almacen");
                    continue;
                }
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
            match self.crear_ventana(id, &imagen, rect, escala) {
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
