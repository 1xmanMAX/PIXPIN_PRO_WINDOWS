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
use pixpin_geom::{DisposicionMonitores, Monitor, Punto, Rect, recolocar_en_area};
use pixpin_motor2d::Escena;
use pixpin_pin::{
    CambioPin, Contenido, CursorAnotacion, LupaPin, Paleta, Pin, Presentacion, TextosPin, icono_de,
    miniatura_de, presentacion_de, tamano_humano, tamano_natural,
};
use pixpin_render::MotorRender;
use pixpin_store::{Almacen, ColorGrupo, PinGuardado, TipoEntrada};
use pixpin_ui::{
    Anotador, BotonCaja, CajaHerramientas, EfectoAnotador, EventoAnotador, Herramienta, Lupa,
    TeclaAnotador,
};
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
/// El cursor de cada herramienta dentro del pin: cruz para dibujar, barra
/// para escribir, flecha para la mano.
fn cursor_pin_de(h: Herramienta) -> CursorAnotacion {
    match h {
        Herramienta::Mano => CursorAnotacion::Flecha,
        Herramienta::Texto => CursorAnotacion::Texto,
        _ => CursorAnotacion::Cruz,
    }
}

/// Donde esta guardado un pin ahora mismo, si el almacen lo da por abierto.
/// Se consulta justo antes de cerrarlo: al marcarlo cerrado esa posicion se
/// pierde, y sin ella no se puede devolver a su sitio.
fn posicion_guardada(almacen: &Rc<RefCell<Almacen>>, id: u64) -> Option<PinGuardado> {
    almacen
        .borrow()
        .entradas()
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| e.pin)
}

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
    /// Pila de los que se han cerrado, con la posicion que tenian: cerrar
    /// marca el pin como cerrado en el almacen y con ello se pierde donde
    /// estaba, asi que hay que guardarlo ANTES para poder devolverlo a su
    /// sitio. El ultimo cerrado es el primero en volver.
    reabrir: Rc<RefCell<Vec<(u64, PinGuardado)>>>,
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
    /// La coletilla del video que no se pudo reproducir, ya traducida: el
    /// usuario vio un video parado y no supo por que.
    texto_sin_codec: String,
    /// La ventana del bucle principal, para darle un toque cuando un pin
    /// deja algo pendiente que solo el gestor puede atender.
    hwnd_app: windows::Win32::Foundation::HWND,
    /// La anotacion en curso, si hay un pin en modo edicion. Solo uno a la
    /// vez: anotar dos pines a la vez no significa nada y complicaria el
    /// foco del teclado sin ganar nada.
    anotacion: Option<Anotacion>,
    /// Cada cuanto pregunta un pin de video por fotogramas (D67): lo decide
    /// el nivel de rendimiento al arrancar. `None` si el dispositivo no
    /// soporta video (D66): entonces los videos se ensenan como documento.
    ritmo_video: Option<u32>,
}

/// Un pin en modo anotacion: su dibujo, su maquina y su elemento en curso.
struct Anotacion {
    id: u64,
    escena: Escena,
    anotador: Anotador,
    /// El elemento que se esta arrastrando ahora mismo: se pinta pero no
    /// esta en la escena todavia, asi que no se guarda ni se deshace.
    en_curso: Option<pixpin_motor2d::Elemento>,
    /// La caja de herramientas junto al pin y la ventana que la muestra
    /// (D58). La paleta muere con la anotacion: su `Drop` la destruye.
    caja: CajaHerramientas,
    paleta: Paleta,
    escala_por_cien: u32,
    /// Donde estaba el raton la ultima vez, en coordenadas del contenido:
    /// la lupa se recalcula desde aqui cuando la rueda cambia el aumento.
    ultimo_cursor: Punto,
}

impl Anotacion {
    /// Las ordenes de dibujo de la escena mas, si lo hay, el trazo en curso.
    fn ordenes(&self) -> Vec<pixpin_motor2d::Orden> {
        let mut v = pixpin_motor2d::ordenes_de_escena(&self.escena);
        if let Some(e) = &self.en_curso {
            v.extend(pixpin_motor2d::ordenes(e));
        }
        // El marco va el ultimo: encima de todo, para que se vea que hay
        // algo elegido aunque quede debajo de otro trazo.
        if let Some(marco) = self.anotador.seleccion().and_then(|id| {
            pixpin_motor2d::marco_de_seleccion(
                &self.escena,
                id,
                self.escala_por_cien as f32 / 100.0,
            )
        }) {
            v.push(marco);
        }
        v
    }
}

impl Pines {
    #[allow(clippy::too_many_arguments)] // lo que el gestor recibe una vez y no cambia
    pub fn nuevos(
        raiz: &Path,
        d3d: ID3D11Device,
        motor: Rc<MotorRender>,
        texto_no_encontrado: String,
        textos: TextosPin,
        texto_confirmar_eliminar: String,
        texto_sin_codec: String,
        hwnd_app: windows::Win32::Foundation::HWND,
        ritmo_video: Option<u32>,
    ) -> Result<Pines> {
        let almacen = Almacen::abrir(raiz).context("no se pudo abrir el almacen")?;
        Ok(Pines {
            almacen: Rc::new(RefCell::new(almacen)),
            d3d,
            motor,
            vivos: HashMap::new(),
            cerrados: Rc::new(RefCell::new(Vec::new())),
            reabrir: Rc::new(RefCell::new(Vec::new())),
            pedidos: Rc::new(RefCell::new(Vec::new())),
            tema_claro: pixpin_shell::entorno::tema_claro(),
            texto_no_encontrado,
            textos,
            texto_confirmar_eliminar,
            texto_sin_codec,
            hwnd_app,
            anotacion: None,
            ritmo_video,
        })
    }

    fn guardado_desde(region: Rect, escala: u32, zoom_por_cien: u32) -> PinGuardado {
        PinGuardado {
            x: region.x,
            y: region.y,
            ancho: region.ancho,
            alto: region.alto,
            escala_por_cien: escala,
            zoom_por_cien,
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
        let reabrir = Rc::clone(&self.reabrir);
        let pedidos = Rc::clone(&self.pedidos);
        let hwnd_app = self.hwnd_app;
        let pin = Pin::nuevo(
            &self.d3d,
            Rc::clone(&self.motor),
            contenido,
            region,
            escala,
            self.tema_claro,
            self.ritmo_video.unwrap_or(16),
            Box::new(move |cambio| {
                let resultado = match cambio {
                    CambioPin::Movido(r, zoom) | CambioPin::Redimensionado(r, zoom) => almacen
                        .borrow_mut()
                        .actualizar_pin(id, Some(Pines::guardado_desde(r, escala, zoom))),
                    CambioPin::Cerrado => {
                        cerrados.borrow_mut().push(id);
                        // Apuntar DONDE estaba antes de marcarlo cerrado:
                        // marcarlo borra esa posicion del almacen, y sin
                        // ella «restaurar el ultimo cerrado» no sabria
                        // adonde devolverlo.
                        if let Some(g) = posicion_guardada(&almacen, id) {
                            reabrir.borrow_mut().push((id, g));
                        }
                        pixpin_shell::despertar(hwnd_app);
                        almacen.borrow_mut().actualizar_pin(id, None)
                    }
                    // El pin no sabe hacer nada de esto: no conoce ni el
                    // portapapeles ni el almacen ni su propia entrada. Se
                    // apuntan y el bucle los atiende, ya fuera del prestamo.
                    otro => {
                        pedidos.borrow_mut().push((id, otro));
                        // Y se le da un toque al bucle: sin esto la peticion
                        // se quedaba en la cola hasta que el usuario pulsara
                        // un atajo, porque el WndProc del pin no produce
                        // ningun evento de la ventana principal. Lo encontro
                        // la prueba de extremo a extremo del menu.
                        pixpin_shell::despertar(hwnd_app);
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
        // Y con lo que tuviera dibujado encima. Sin esto el pin volvia
        // limpio tras reiniciar y la anotacion parecia perdida, aunque su
        // fichero siguiera ahi: lo encontro la prueba de extremo a extremo.
        self.recargar_anotaciones(id);
        Ok(())
    }

    /// Vuelve a pintar en el pin lo que haya en su fichero de anotacion.
    /// Un fichero ilegible se registra y no impide que el pin exista: el
    /// contenido original es lo importante.
    fn recargar_anotaciones(&self, id: u64) {
        let Some(ruta) = self.ruta_anotacion(id) else {
            return;
        };
        match pixpin_motor2d::cargar(&ruta) {
            Ok(escena) if escena.cuantos_visibles() > 0 => {
                if let Some(pin) = self.vivos.get(&id) {
                    pin.poner_anotaciones(pixpin_motor2d::ordenes_de_escena(&escena));
                }
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(?e, id, "anotacion ilegible; el pin sale sin ella"),
        }
    }

    /// D26: el recorte queda flotando 1:1 exactamente donde estaba.
    pub fn pinear(&mut self, imagen: &ImagenRgba, region: Rect, escala: u32) -> Result<()> {
        let png = codificar_png(imagen).context("no se pudo codificar el pin")?;
        let id = self
            .almacen
            .borrow_mut()
            .guardar_imagen(
                &png,
                "recorte",
                Some(Pines::guardado_desde(region, escala, 100)),
            )
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
                Some(Pines::guardado_desde(region, monitor.escala_por_cien, 100)),
            )
            .context("no se pudo guardar la nota")?;
        self.crear_ventana(id, contenido, region, monitor.escala_por_cien)
    }

    /// Como se ensena un archivo por referencia (D62/D65): video si la
    /// extension lo dice y el dispositivo puede reproducirlo; si no,
    /// documento cuando la Shell tiene miniatura, y ficha en ultimo caso.
    /// `tamano_guardado`: al restaurar, el rect del pin ya esta en el indice y
    /// no hace falta pedir la miniatura del video para saber su proporcion
    /// (ahorra cientos de ms por video al arrancar).
    fn contenido_de_archivo(&self, ruta: &Path, tamano_guardado: bool) -> Contenido {
        let nombre = ruta
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| ruta.to_string_lossy().into_owned());
        let es_video = presentacion_de(ruta) == Presentacion::Video
            && self.ritmo_video.is_some()
            && ruta.is_file();
        if es_video {
            // La proporcion de la miniatura decide el tamano al nacer; el
            // tamano nativo llega con los metadatos y solo afecta al 100 %.
            // Sin miniatura, el provisional (D71).
            let (ancho, alto) = if tamano_guardado {
                None
            } else {
                miniatura_de(ruta, 512)
            }
            .map(|m| {
                let base = 960.0;
                let f = base / m.ancho.max(1) as f32;
                (
                    (m.ancho as f32 * f).round() as u32,
                    (m.alto as f32 * f).round() as u32,
                )
            })
            .unwrap_or((0, 0));
            return Contenido::Video {
                nombre,
                ruta: ruta.to_path_buf(),
                ancho,
                alto,
            };
        }
        match miniatura_de(ruta, 1024) {
            Some(vista) => Contenido::Documento { nombre, vista },
            None => ficha_de(ruta, &self.texto_no_encontrado),
        }
    }

    /// Media Foundation no pudo con el video (D72): el pin se vuelve a
    /// crear como documento o ficha, en el mismo sitio y con el mismo id.
    fn degradar_video(&mut self, id: u64) -> Result<()> {
        let Some(pin) = self.vivos.remove(&id) else {
            return Ok(());
        };
        let region = pin.rect_contenido();
        let escala = pin.escala_por_cien();
        drop(pin);
        let ruta = {
            let a = self.almacen.borrow();
            a.entradas()
                .iter()
                .find(|e| e.id == id)
                .and_then(|e| e.ruta.clone())
                .context("el video no referencia ningun fichero")?
        };
        tracing::warn!(id, ruta = %ruta.display(), "video no reproducible; se ensena como documento o ficha");
        let contenido = match miniatura_de(&ruta, 1024) {
            // El nombre lleva la coletilla «sin codec de video»: un video
            // parado sin explicacion parece un fallo del programa.
            Some(vista) => Contenido::Documento {
                nombre: format!(
                    "{} · {}",
                    ruta.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    self.texto_sin_codec
                ),
                vista,
            },
            None => {
                let mut ficha = ficha_de(&ruta, &self.texto_no_encontrado);
                if let Contenido::Archivo { detalle, .. } = &mut ficha {
                    *detalle = format!("{detalle} · {}", self.texto_sin_codec);
                }
                ficha
            }
        };
        // Conserva el ancho del pin; el alto se adapta al contenido nuevo.
        let motor = Rc::clone(&self.motor);
        let (nw, nh) = tamano_natural(&contenido, escala, &|t, tam, max, tramos| {
            motor.medir_parrafo(t, tam, max, tramos)
        });
        let alto = if contenido.solo_ancho() {
            // La ficha tiene alto fijo: el contenido manda, no el ancho.
            nh
        } else if nw > 0 {
            ((region.ancho as f32) * (nh as f32 / nw as f32))
                .round()
                .max(1.0) as u32
        } else {
            region.alto
        };
        let nueva = Rect {
            x: region.x,
            y: region.y,
            ancho: region.ancho,
            alto,
        };
        self.crear_ventana(id, contenido, nueva, escala)
    }

    /// La ficha de un archivo o carpeta, por referencia (D28).
    pub fn pinear_archivo(&mut self, ruta: &Path, monitor: &Monitor) -> Result<()> {
        let t0 = std::time::Instant::now();
        let contenido = self.contenido_de_archivo(ruta, false);
        let tipo = match &contenido {
            Contenido::Video { .. } => "video",
            Contenido::Documento { .. } => "documento",
            _ => "ficha",
        };
        let region = self.region_centrada(&contenido, monitor);
        let id = self
            .almacen
            .borrow_mut()
            .guardar_archivo(
                ruta,
                Some(Pines::guardado_desde(region, monitor.escala_por_cien, 100)),
            )
            .context("no se pudo guardar la referencia")?;
        let hecho = self.crear_ventana(id, contenido, region, monitor.escala_por_cien);
        tracing::info!(
            id,
            tipo,
            ms = t0.elapsed().as_millis() as u64,
            "archivo pineado"
        );
        hecho
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
                Some(Pines::guardado_desde(region, monitor.escala_por_cien, 100)),
            )
            .context("no se pudo guardar en el almacen")?;
        self.crear_ventana(id, contenido, region, monitor.escala_por_cien)
    }

    /// Donde nace un pin que no viene de un recorte: centrado en el monitor
    /// del cursor, encogido al 80 % del area de trabajo si no cabe.
    fn region_centrada(&self, contenido: &Contenido, monitor: &Monitor) -> Rect {
        let motor = Rc::clone(&self.motor);
        let (mut w, mut h) = tamano_natural(
            contenido,
            monitor.escala_por_cien,
            &|t, tam, max, tramos| motor.medir_parrafo(t, tam, max, tramos),
        );

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
                    Some(r) => self.contenido_de_archivo(r, true),
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
                Ok(()) => {
                    restaurados += 1;
                    // El zoom del texto de una nota vuelve con ella.
                    if guardado.zoom_por_cien != 100 {
                        if let Some(pin) = self.vivos.get(&id) {
                            pin.poner_zoom_por_cien(guardado.zoom_por_cien);
                        }
                    }
                }
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
            CambioPin::AnotarPedido => self.entrar_a_anotar(id),
            CambioPin::PunteroPulsado(p) => self.anotar(id, EventoAnotador::Pulsar(a_punto2(p))),
            CambioPin::PunteroMovido(p) => self.anotar(id, EventoAnotador::Mover(a_punto2(p))),
            CambioPin::PunteroSoltado(p) => self.anotar(id, EventoAnotador::Soltar(a_punto2(p))),
            CambioPin::RuedaGirada { delta, cursor } => {
                // Anotando, la rueda cambia el grosor; si no, hace zoom del
                // pin, que es lo que pidio el usuario (D55).
                if self.anotacion.as_ref().is_some_and(|a| a.id == id) {
                    self.anotar(id, EventoAnotador::Rueda(delta))
                } else {
                    self.zoom(id, delta, cursor)
                }
            }
            CambioPin::EscapeAnotando => {
                self.anotar(id, EventoAnotador::Tecla(TeclaAnotador::Escape))
            }
            CambioPin::CaracterAnotando(c) => self.anotar(id, EventoAnotador::Caracter(c)),
            CambioPin::EnterAnotando => {
                self.anotar(id, EventoAnotador::Tecla(TeclaAnotador::Enter))
            }
            CambioPin::RetrocesoAnotando => {
                self.anotar(id, EventoAnotador::Tecla(TeclaAnotador::Retroceso))
            }
            CambioPin::PaletaPulsada(p) => self.paleta_pulsada(id, p),
            CambioPin::VideoFallido => self.degradar_video(id),
            // Movido, Redimensionado y Cerrado los resuelve el callback.
            _ => Ok(()),
        }
    }

    /// Donde vive el dibujo de un pin: junto a su objeto, mismo nombre y
    /// otra extension (D48). El objeto original nunca se toca.
    fn ruta_anotacion(&self, id: u64) -> Option<PathBuf> {
        let a = self.almacen.borrow();
        let e = a.entradas().iter().find(|e| e.id == id)?;
        if e.objeto.is_empty() {
            // Una ficha de archivo no tiene objeto propio que anotar.
            return None;
        }
        Some(a.ruta_objeto(e).with_extension(pixpin_motor2d::EXTENSION))
    }

    /// Doble clic: entra en modo anotacion cargando lo que ya hubiera.
    fn entrar_a_anotar(&mut self, id: u64) -> Result<()> {
        let t0 = std::time::Instant::now();
        // Salir del anterior guardando: dos pines anotandose a la vez no
        // significa nada y enredaria el foco del teclado.
        self.salir_de_anotar()?;

        let ruta = self
            .ruta_anotacion(id)
            .context("este pin no tiene contenido que anotar")?;
        let escena = pixpin_motor2d::cargar(&ruta).context("no se pudo leer la anotacion")?;
        // La semilla arranca del id: dos pines distintos no dibujan igual.
        let anotador = Anotador::nuevo((id as u32).wrapping_mul(2_654_435_761) | 1);

        let pin = self.vivos.get(&id).context("el pin no esta en pantalla")?;
        // La paleta se coloca respecto al contenido del pin y al area de
        // trabajo de SU monitor: en otro monitor se saldria de la pantalla.
        let contenido = pin.rect_contenido();
        let disposicion = pixpin_capture::enumerar_monitores().context("sin monitores")?;
        let monitor = disposicion
            .monitores()
            .iter()
            .find(|m| {
                m.area.contiene(Punto {
                    x: contenido.x,
                    y: contenido.y,
                })
            })
            .or_else(|| disposicion.principal())
            .copied()
            .context("sin monitor para la paleta")?;
        let caja =
            CajaHerramientas::colocar(contenido, monitor.area_trabajo, monitor.escala_por_cien);
        let pedidos = Rc::clone(&self.pedidos);
        let hwnd_app = self.hwnd_app;
        let paleta = Paleta::nueva(
            &self.d3d,
            Rc::clone(&self.motor),
            caja.marco,
            Box::new(move |p| {
                // Mismo camino que el menu del pin: se apunta y se despierta
                // al bucle, que lo atiende fuera del prestamo.
                pedidos.borrow_mut().push((id, CambioPin::PaletaPulsada(p)));
                pixpin_shell::despertar(hwnd_app);
            }),
        )
        .context("no se pudo crear la paleta del pin")?;

        pin.poner_modo_anotacion(true);
        pin.poner_cursor_anotacion(cursor_pin_de(anotador.herramienta()));
        pin.poner_anotaciones(pixpin_motor2d::ordenes_de_escena(&escena));
        self.anotacion = Some(Anotacion {
            id,
            escena,
            anotador,
            en_curso: None,
            caja,
            paleta,
            escala_por_cien: monitor.escala_por_cien,
            ultimo_cursor: Punto { x: 0, y: 0 },
        });
        self.repintar_paleta();
        tracing::info!(id, ms = t0.elapsed().as_millis() as u64, "modo anotacion");
        Ok(())
    }

    /// Vuelve a pintar la paleta con la herramienta activa resaltada. El
    /// pintor captura COPIAS: la paleta lo reusa en cada `WM_PAINT`.
    fn repintar_paleta(&self) {
        let Some(a) = &self.anotacion else {
            return;
        };
        let caja = a.caja;
        let activa = a.anotador.herramienta();
        let escala = a.escala_por_cien;
        let origen = Punto {
            x: caja.marco.x,
            y: caja.marco.y,
        };
        a.paleta.poner_pintor(Box::new(move |p| {
            crate::caja_dibujo::pintar_caja(p, &caja, activa, escala, origen);
        }));
    }

    /// Un clic en la paleta, en coordenadas de la paleta (D58).
    fn paleta_pulsada(&mut self, id: u64, p: Punto) -> Result<()> {
        let Some(a) = self.anotacion.as_ref().filter(|a| a.id == id) else {
            return Ok(());
        };
        let global = Punto {
            x: p.x + a.caja.marco.x,
            y: p.y + a.caja.marco.y,
        };
        let Some(boton) = a.caja.boton_en(global) else {
            return Ok(());
        };
        match boton {
            BotonCaja::Elegir(h) => {
                self.anotar(id, EventoAnotador::CambiarHerramienta(h))?;
                self.repintar_paleta();
                // El cursor sigue a la herramienta (lo pidio el usuario).
                if let Some(pin) = self.vivos.get(&id) {
                    pin.poner_cursor_anotacion(cursor_pin_de(h));
                }
            }
            BotonCaja::Deshacer => {
                self.anotar(id, EventoAnotador::Tecla(TeclaAnotador::Deshacer))?
            }
            BotonCaja::Rehacer => self.anotar(id, EventoAnotador::Tecla(TeclaAnotador::Rehacer))?,
            // La paleta de colores llega con los ajustes visuales (S3-D).
            BotonCaja::Color => {}
            BotonCaja::Salir => self.salir_de_anotar()?,
        }
        Ok(())
    }

    /// Sale del modo anotacion guardando el dibujo.
    pub fn salir_de_anotar(&mut self) -> Result<()> {
        let Some(a) = self.anotacion.take() else {
            return Ok(());
        };
        if let Some(pin) = self.vivos.get(&a.id) {
            pin.poner_modo_anotacion(false);
        }
        let Some(ruta) = self.ruta_anotacion(a.id) else {
            return Ok(());
        };
        pixpin_motor2d::guardar(&ruta, &a.escena).context("no se pudo guardar la anotacion")?;
        tracing::info!(
            id = a.id,
            elementos = a.escena.cuantos_visibles(),
            "anotacion guardada"
        );
        Ok(())
    }

    /// Un evento del puntero o del teclado mientras se anota.
    fn anotar(&mut self, id: u64, evento: EventoAnotador) -> Result<()> {
        let Some(a) = self.anotacion.as_mut().filter(|a| a.id == id) else {
            return Ok(());
        };
        if let EventoAnotador::Mover(p) | EventoAnotador::Pulsar(p) = &evento {
            a.ultimo_cursor = Punto {
                x: p.x as i32,
                y: p.y as i32,
            };
        }
        // El anotador es puro y no lee el teclado: se le dicen los
        // modificadores justo antes de cada gesto del puntero.
        if matches!(
            evento,
            EventoAnotador::Pulsar(_) | EventoAnotador::Mover(_) | EventoAnotador::Soltar(_)
        ) {
            let (shift, alt) = pixpin_shell::modificadores();
            a.anotador.poner_modificadores(shift, alt);
        }
        let efecto = a.anotador.procesar(evento);
        let mut repintar = true;
        let mut salir = false;

        match efecto {
            EfectoAnotador::Nada => repintar = false,
            EfectoAnotador::Repintar => a.en_curso = None,
            EfectoAnotador::EnCurso(e) => a.en_curso = Some(*e),
            EfectoAnotador::Terminado(e) => {
                a.en_curso = None;
                a.escena.anadir(*e);
            }
            EfectoAnotador::BorrarEn(p) => {
                if let Some(victima) = a.escena.elemento_en(p) {
                    a.escena.borrar_apuntando(victima);
                }
            }
            EfectoAnotador::Deshacer => {
                a.escena.deshacer();
            }
            EfectoAnotador::Rehacer => {
                a.escena.rehacer();
            }
            // La mano: el anotador no ve la escena y pregunta.
            EfectoAnotador::SeleccionarEn(p) => {
                let elegido = a.escena.elemento_en(p);
                a.anotador.poner_seleccion(elegido);
            }
            EfectoAnotador::MoverSeleccion { dx, dy } => {
                if let Some(sel) = a.anotador.seleccion() {
                    a.escena.mover(sel, dx, dy);
                }
            }
            EfectoAnotador::MovimientoTerminado { dx, dy } => {
                if let Some(sel) = a.anotador.seleccion() {
                    a.escena.apuntar_movimiento(sel, dx, dy);
                }
            }
            EfectoAnotador::DuplicarSeleccion => {
                if let Some(copia) = a
                    .anotador
                    .seleccion()
                    .and_then(|sel| a.escena.buscar(sel).cloned())
                {
                    let nuevo = a.escena.anadir(copia);
                    a.anotador.poner_seleccion(Some(nuevo));
                }
            }
            EfectoAnotador::BorrarSeleccion => {
                if let Some(sel) = a.anotador.seleccion() {
                    a.escena.borrar_apuntando(sel);
                    a.anotador.poner_seleccion(None);
                }
            }
            EfectoAnotador::Salir => salir = true,
        }

        if repintar && !salir {
            let ordenes = a.ordenes();
            let escribiendo = a.anotador.editando_texto();
            let con_lupa = a.anotador.herramienta() == Herramienta::Lupa;
            let aumento = a.anotador.lupa();
            let cursor = a.ultimo_cursor;
            let escala = a.escala_por_cien;
            if let Some(pin) = self.vivos.get(&id) {
                // Con un texto abierto, el IME compone al lado (D57).
                if let Some(p) = escribiendo {
                    pin.poner_posicion_ime(Punto {
                        x: p.x as i32,
                        y: p.y as i32,
                    });
                }
                // La lupa (D52): la aritmetica aqui, los pixeles en el pin.
                // Se coloca DENTRO del contenido, huyendo del cursor.
                let r = pin.rect_contenido();
                let l = Lupa::con_aumento(escala, aumento);
                // En un pin mas pequeno que la lupa no cabe: sin lupa, en
                // vez de una lupa que tape el pin entero.
                let cabe = r.ancho > l.diametro && r.alto > l.diametro;
                let lupa = if con_lupa && cabe {
                    let local = Rect {
                        x: 0,
                        y: 0,
                        ancho: r.ancho,
                        alto: r.alto,
                    };
                    let pos = l.colocar(cursor, local);
                    Some(LupaPin {
                        fuente: l.region_fuente(cursor, local),
                        destino: Rect {
                            x: pos.x,
                            y: pos.y,
                            ancho: l.diametro,
                            alto: l.diametro,
                        },
                    })
                } else {
                    None
                };
                pin.poner_lupa(lupa);
                pin.poner_anotaciones(ordenes);
            }
        }
        if salir {
            self.salir_de_anotar()?;
        }
        Ok(())
    }

    /// Rueda sobre un pin que no se esta anotando: agranda o encoge (D55).
    fn zoom(&mut self, id: u64, delta: i32, cursor: Punto) -> Result<()> {
        let Some(pin) = self.vivos.get(&id) else {
            return Ok(());
        };
        // La ficha no se redimensiona, tampoco con la rueda.
        if !pin.redimensionable() {
            return Ok(());
        }
        // Desde el DESTINO en curso, no desde el fotograma intermedio: una
        // rueda que sigue girando encadena pasos y la animacion los sigue
        // sin saltos (el usuario los veia «de salto en salto»).
        let r = pin.rect_objetivo();
        if r.ancho == 0 || r.alto == 0 {
            return Ok(());
        }
        // Proporcional al giro: una rueda fina (tactil) da deltas pequenos
        // y pasos pequenos; una muesca entera, el 10 %.
        let paso = 1.1f32.powf(delta as f32 / 120.0);
        // Anclado en el CURSOR, no en el centro: lo que el usuario esta
        // mirando se queda bajo el puntero mientras crece todo lo demas. Con
        // el centro, el detalle se le escapaba de debajo del raton.
        // El tope es el mismo que el del zoom por arrastre: la ventana se
        // recorta al escritorio, asi que un pin enorme no cuesta memoria.
        let nuevo = pixpin_pin::escalar_anclado(
            r,
            paso,
            cursor,
            pixpin_pin::MINIMO_LOGICO,
            pixpin_pin::MAXIMO_FISICO,
        );
        let zoom = pin.zoom_objetivo_por_cien(nuevo);
        pin.escalar_persiguiendo(nuevo);
        self.almacen
            .borrow_mut()
            // Con la escala REAL del pin: guardar 100 en un monitor al 150 %
            // hacia que el pin volviera 1,5 veces mas grande tras reiniciar.
            .actualizar_pin(
                id,
                Some(Pines::guardado_desde(nuevo, pin.escala_por_cien(), zoom)),
            )
            .ok();
        Ok(())
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

    /// Cierra todos los pines de la pantalla, dejandolos en el almacen y
    /// apuntados para poder devolverlos uno a uno. Devuelve cuantos cerro.
    pub fn cerrar_todos(&mut self) -> usize {
        let ids: Vec<u64> = self.vivos.keys().copied().collect();
        for id in &ids {
            if let Some(g) = posicion_guardada(&self.almacen, *id) {
                self.reabrir.borrow_mut().push((*id, g));
            }
            if let Err(e) = self.almacen.borrow_mut().actualizar_pin(*id, None) {
                tracing::warn!(?e, id = *id, "no se pudo marcar el pin como cerrado");
            }
            self.vivos.remove(id);
        }
        ids.len()
    }

    /// Quita los pines de la pantalla SIN cerrarlos: el almacen los sigue
    /// dando por abiertos, asi que `mostrar_todos` los devuelve enteros.
    /// Es la diferencia con `cerrar_todos`, que si los cierra.
    pub fn ocultar_todos(&mut self) -> usize {
        let cuantos = self.vivos.len();
        self.vivos.clear();
        cuantos
    }

    /// Devuelve a la pantalla todo lo que el almacen da por abierto. Los
    /// grupos ocultos siguen ocultos: para eso estan.
    pub fn mostrar_todos(&mut self, disposicion: &DisposicionMonitores) -> usize {
        self.restaurar(disposicion)
    }

    /// Un solo comando para las dos cosas, como en el original: si hay algo
    /// en pantalla lo esconde, y si no, lo saca. Devuelve si escondio y
    /// cuantos pines movio.
    pub fn alternar_todos(&mut self, disposicion: &DisposicionMonitores) -> (bool, usize) {
        if self.vivos.is_empty() {
            (false, self.mostrar_todos(disposicion))
        } else {
            (true, self.ocultar_todos())
        }
    }

    /// Devuelve el ultimo pin cerrado a donde estaba. `false` si no queda
    /// ninguno por devolver o si su entrada ya no existe en el almacen.
    pub fn restaurar_ultimo_cerrado(&mut self, disposicion: &DisposicionMonitores) -> bool {
        loop {
            // El `pop` va en su propia sentencia para soltar el prestamo de
            // la pila antes de tocar el almacen y las ventanas: encadenarlo
            // en el `while let` lo mantendria vivo todo el cuerpo.
            let siguiente = self.reabrir.borrow_mut().pop();
            let Some((id, guardado)) = siguiente else {
                return false;
            };
            // Puede haberse eliminado del almacen despues de cerrarlo: eso
            // es definitivo y no se deshace, asi que se pasa al siguiente.
            let existe = self
                .almacen
                .borrow()
                .entradas()
                .iter()
                .any(|e| e.id == id && e.pin.is_none());
            if !existe {
                continue;
            }
            if let Err(e) = self.almacen.borrow_mut().actualizar_pin(id, Some(guardado)) {
                tracing::warn!(?e, id, "no se pudo devolver el pin al almacen");
                continue;
            }
            self.restaurar(disposicion);
            return self.vivos.contains_key(&id);
        }
    }

    pub fn abiertos(&self) -> usize {
        self.vivos.len()
    }
}

/// Del punto entero del pin al punto en coma flotante del motor.
fn a_punto2(p: pixpin_geom::Punto) -> pixpin_motor2d::Punto2 {
    pixpin_motor2d::Punto2::nuevo(p.x as f32, p.y as f32)
}
