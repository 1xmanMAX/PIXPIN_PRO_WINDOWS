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
use pixpin_pin::{CambioPin, Contenido, Pin, TextosPin, icono_de, tamano_humano, tamano_natural};
use pixpin_render::MotorRender;
use pixpin_store::{Almacen, ColorGrupo, PinGuardado, TipoEntrada};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

/// La paleta de grupos en RGB (D35). Vive aqui porque es la traduccion
/// entre dos crates de la MISMA capa que no pueden verse: `pixpin-store`
/// sabe que color es cada grupo, `pixpin-pin` solo entiende numeros.
fn rgb_de(color: ColorGrupo) -> (f32, f32, f32) {
    match color {
        ColorGrupo::Rojo => (0.86, 0.20, 0.18),
        ColorGrupo::Naranja => (0.95, 0.45, 0.10),
        ColorGrupo::Ambar => (0.95, 0.68, 0.10),
        ColorGrupo::Verde => (0.20, 0.66, 0.33),
        ColorGrupo::Cian => (0.10, 0.66, 0.68),
        ColorGrupo::Azul => (0.16, 0.44, 0.86),
        ColorGrupo::Violeta => (0.48, 0.28, 0.78),
        ColorGrupo::Rosa => (0.87, 0.28, 0.60),
    }
}

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
    /// Peticiones del menu y del teclado que el pin no puede resolver.
    /// Se atienden en `purgar`, ya fuera del callback: dentro, el almacen
    /// esta prestado y volver a pedirlo entraria en panico.
    pedidos: Rc<RefCell<Vec<(u64, CambioPin)>>>,
    /// Se lee una vez al arrancar (D33): un pin nuevo nace con el tema del
    /// momento, y los ya abiertos no cambian de color a media sesion.
    tema_claro: bool,
    /// Ya traducido: `pixpin-pin` no conoce el catalogo de idiomas.
    texto_no_encontrado: String,
    /// Etiquetas del menu del pin, tambien ya traducidas.
    textos: TextosPin,
    /// El aviso antes de borrar del almacen, ya traducido.
    texto_confirmar_eliminar: String,
}

impl Pines {
    pub fn nuevos(
        raiz: &Path,
        d3d: ID3D11Device,
        motor: Rc<MotorRender>,
        texto_no_encontrado: String,
        textos: TextosPin,
        texto_confirmar_eliminar: String,
    ) -> Result<Pines> {
        let almacen = Almacen::abrir(raiz).context("no se pudo abrir el almacen")?;
        Ok(Pines {
            almacen: Rc::new(RefCell::new(almacen)),
            d3d,
            motor,
            vivos: HashMap::new(),
            cerrados: Rc::new(RefCell::new(Vec::new())),
            pedidos: Rc::new(RefCell::new(Vec::new())),
            tema_claro: pixpin_shell::entorno::tema_claro(),
            texto_no_encontrado,
            textos,
            texto_confirmar_eliminar,
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
        let pedidos = Rc::clone(&self.pedidos);
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
                    // El pin no sabe hacer nada de esto: no conoce ni el
                    // portapapeles ni el almacen ni su propia entrada. Se
                    // apuntan y el bucle los atiende, ya fuera del prestamo.
                    otro => {
                        pedidos.borrow_mut().push((id, otro));
                        Ok(())
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
        pin.poner_textos(self.textos.clone());
        // Un pin restaurado nace ya con el color de su grupo: pintarlo negro
        // y retenirlo despues daria un parpadeo al arrancar.
        if let Some(g) = self.almacen.borrow().grupo_de(id) {
            pin.poner_color(Some(rgb_de(g.color)));
        }
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
            let ocultos: Vec<u32> = a
                .grupos()
                .iter()
                .filter(|g| g.oculto)
                .map(|g| g.id)
                .collect();
            a.entradas()
                .iter()
                // Un grupo oculto NO vuelve solo: ni al arrancar ni al
                // restaurar otro. Conserva su `pin` para saber donde
                // devolverlo cuando el usuario lo pida desde la bandeja.
                .filter(|e| !e.grupo.is_some_and(|g| ocultos.contains(&g)))
                // Ni se duplica uno que ya esta en pantalla: `restaurar`
                // tambien se usa al mostrar un grupo, con otros ya abiertos.
                .filter(|e| !self.vivos.contains_key(&e.id))
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
        let pedidos: Vec<(u64, CambioPin)> = self.pedidos.borrow_mut().drain(..).collect();
        for (id, cambio) in pedidos {
            if let Err(e) = self.atender(id, cambio) {
                tracing::warn!(?e, id, ?cambio, "no se pudo atender la peticion del pin");
            }
        }
    }

    /// Lo que el pin pidio y no podia hacer solo.
    fn atender(&mut self, id: u64, cambio: CambioPin) -> Result<()> {
        match cambio {
            CambioPin::CopiarPedido => self.copiar(id),
            CambioPin::GrupoPedido(indice) => {
                let color = match indice {
                    None => None,
                    Some(i) => Some(
                        ColorGrupo::por_indice(i).context("indice de color fuera de la paleta")?,
                    ),
                };
                self.poner_grupo(id, color)
            }
            CambioPin::OcultarGrupoPedido => self.ocultar_grupo_de(id),
            CambioPin::EliminarPedido => self.eliminar(id),
            CambioPin::AbrirPedido => self.abrir(id, false),
            CambioPin::AbrirUbicacionPedido => self.abrir(id, true),
            CambioPin::GuardarComoPedido => self.guardar_como(id),
            // Movido, Redimensionado y Cerrado los resuelve el callback.
            _ => Ok(()),
        }
    }

    /// Oculta en bloque el grupo del pin: cierra sus ventanas pero DEJA sus
    /// `pin` en el indice, que es lo que permite devolverlos a su sitio
    /// exacto al mostrarlos (D24).
    fn ocultar_grupo_de(&mut self, id: u64) -> Result<()> {
        let grupo = self
            .almacen
            .borrow()
            .grupo_de(id)
            .context("el pin no tiene grupo que ocultar")?;
        self.almacen
            .borrow_mut()
            .poner_grupo_oculto(grupo.id, true)
            .context("no se pudo marcar el grupo como oculto")?;

        let del_grupo: Vec<u64> = self
            .almacen
            .borrow()
            .entradas()
            .iter()
            .filter(|e| e.grupo == Some(grupo.id))
            .map(|e| e.id)
            .collect();
        for otro in del_grupo {
            self.vivos.remove(&otro);
        }
        Ok(())
    }

    /// Los grupos ocultos con su etiqueta ya montada, para el menu de la
    /// bandeja: es la unica via de vuelta de unos pines que ya no se ven.
    pub fn grupos_ocultos(&self, textos: &pixpin_store::Catalogo) -> Vec<(u32, String)> {
        let a = self.almacen.borrow();
        a.grupos()
            .iter()
            .filter(|g| g.oculto)
            .map(|g| {
                let cuantos = a
                    .entradas()
                    .iter()
                    .filter(|e| e.grupo == Some(g.id))
                    .count();
                let color = textos.t(match g.color {
                    ColorGrupo::Rojo => "pin-color-rojo",
                    ColorGrupo::Naranja => "pin-color-naranja",
                    ColorGrupo::Ambar => "pin-color-ambar",
                    ColorGrupo::Verde => "pin-color-verde",
                    ColorGrupo::Cian => "pin-color-cian",
                    ColorGrupo::Azul => "pin-color-azul",
                    ColorGrupo::Violeta => "pin-color-violeta",
                    ColorGrupo::Rosa => "pin-color-rosa",
                });
                (g.id, format!("● {color} ({cuantos})"))
            })
            .collect()
    }

    /// Devuelve a la pantalla los pines de un grupo oculto, cada uno donde
    /// estaba. Devuelve cuantos volvieron.
    pub fn mostrar_grupo(&mut self, id_grupo: u32, disposicion: &DisposicionMonitores) -> usize {
        if let Err(e) = self
            .almacen
            .borrow_mut()
            .poner_grupo_oculto(id_grupo, false)
        {
            tracing::warn!(?e, id_grupo, "no se pudo desmarcar el grupo");
            return 0;
        }
        self.restaurar(disposicion)
    }

    /// La unica accion destructiva (menu 4.3): borra la entrada y su objeto.
    /// Pregunta antes, con el «No» por defecto.
    fn eliminar(&mut self, id: u64) -> Result<()> {
        let hwnd = self
            .vivos
            .get(&id)
            .map(|p| p.hwnd())
            .context("el pin ya no esta abierto")?;
        if !pixpin_shell::confirmar_destructivo(
            hwnd,
            &self.textos.eliminar,
            &self.texto_confirmar_eliminar,
        ) {
            return Ok(());
        }
        self.almacen
            .borrow_mut()
            .eliminar(id)
            .context("no se pudo eliminar del almacen")?;
        self.vivos.remove(&id);
        Ok(())
    }

    /// Abre el archivo referenciado, o el Explorador con el seleccionado.
    fn abrir(&self, id: u64, ubicacion: bool) -> Result<()> {
        let ruta = {
            let a = self.almacen.borrow();
            a.entradas()
                .iter()
                .find(|e| e.id == id)
                .and_then(|e| e.ruta.clone())
                .context("esta entrada no referencia ningun fichero")?
        };
        if ubicacion {
            pixpin_shell::abrir_ubicacion(&ruta).context("no se pudo abrir la ubicacion")
        } else {
            pixpin_shell::abrir(&ruta).context("no se pudo abrir el fichero")
        }
    }

    /// Guarda una copia del contenido donde el usuario diga.
    fn guardar_como(&self, id: u64) -> Result<()> {
        let (tipo, objeto) = {
            let a = self.almacen.borrow();
            let e = a
                .entradas()
                .iter()
                .find(|e| e.id == id)
                .context("la entrada ya no esta en el almacen")?;
            (e.tipo, a.ruta_objeto(e))
        };
        let sugerido = match tipo {
            TipoEntrada::Nota => "nota.txt",
            _ => "captura.png",
        };
        let hwnd = self
            .vivos
            .get(&id)
            .map(|p| p.hwnd())
            .context("el pin ya no esta abierto")?;
        // Cancelar no es un fallo: el usuario cambio de idea.
        if let Some(destino) = pixpin_shell::guardar::pedir_ruta_guardado(hwnd, sugerido) {
            std::fs::copy(&objeto, &destino).context("no se pudo escribir el fichero")?;
            tracing::info!(?destino, "contenido del pin guardado");
        }
        Ok(())
    }

    /// `Ctrl+C` sobre un pin: imagen como mapa de bits, nota como texto,
    /// archivo como su ruta (spec 4.2). Se lee del ALMACEN, no de la
    /// ventana: el almacen es la verdad (D21).
    fn copiar(&self, id: u64) -> Result<()> {
        let (tipo, objeto, ruta) = {
            let a = self.almacen.borrow();
            let e = a
                .entradas()
                .iter()
                .find(|e| e.id == id)
                .context("la entrada ya no esta en el almacen")?;
            (e.tipo, a.ruta_objeto(e), e.ruta.clone())
        };
        match tipo {
            TipoEntrada::Imagen => {
                let img = cargar(&objeto).context("no se pudo leer la imagen del almacen")?;
                pixpin_codec::copiar_imagen(&img).context("no se pudo copiar la imagen")?;
            }
            TipoEntrada::Nota => {
                let texto = std::fs::read_to_string(&objeto).context("no se pudo leer la nota")?;
                pixpin_codec::copiar_texto(&texto).context("no se pudo copiar la nota")?;
            }
            // La ruta como texto: pegarla en una terminal o en un dialogo de
            // abrir es exactamente lo que se espera.
            TipoEntrada::Archivo => {
                let r = ruta.context("la entrada de archivo no tiene ruta")?;
                pixpin_codec::copiar_texto(&r.to_string_lossy())
                    .context("no se pudo copiar la ruta")?;
            }
        }
        Ok(())
    }

    /// Asigna (o quita) el grupo de un pin y retiñe su sombra al momento.
    pub fn poner_grupo(&mut self, id: u64, color: Option<ColorGrupo>) -> Result<()> {
        self.almacen
            .borrow_mut()
            .poner_grupo(id, color)
            .context("no se pudo guardar el grupo")?;
        if let Some(pin) = self.vivos.get(&id) {
            pin.poner_color(color.map(rgb_de));
        }
        Ok(())
    }

    pub fn abiertos(&self) -> usize {
        self.vivos.len()
    }
}
