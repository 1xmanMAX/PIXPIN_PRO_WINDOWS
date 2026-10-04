//! La pila de capturas, cableada al ejecutable.
//!
//! `pixpin-pila` trae las dos mitades que funcionan solas: la logica de la
//! tanda y la ventanita de la esquina. Aqui esta lo que les falta para ser
//! una funcion: guardar el PNG, fabricar la miniatura, saber en que monitor
//! se capturo, publicar el portapapeles y atender lo que el usuario pulsa en
//! el panel.
//!
//! **Con `apilar_segundos = 0` este modulo es transparente:** la captura va
//! al portapapeles como un mapa de bits suelto, no se escribe ningun fichero
//! y no nace ninguna ventana. Es el comportamiento de antes, intacto.
//!
//! # Agrupar con el recuadro (2-oct)
//!
//! Tras cada captura sale el recuadro en la esquina. Sin tocarlo, se va solo
//! a los `icono_segundos` y la siguiente captura es otra tanda. Un clic lo
//! ARMA (bordes azules): ya no se va solo y cada captura se suma, sin plazo,
//! hasta que otro clic lo suelta y el recuadro desaparece. Copiar desde el
//! panel (clic derecho) con la tanda armada NO la suelta: se sigue agrupando
//! hasta que el usuario lo diga. Decision tomada con lo que pidio: «eso es lo
//! que lo mantiene en trabajo; sin eso lo libera y desaparece».
//!
//! # Nada lento en el hilo principal
//!
//! El hilo principal es el que abre el overlay de la captura siguiente: lo
//! que se haga aqui despues de una captura retrasa la otra. Por eso el PNG se
//! escribe en un hilo aparte, la miniatura se saca por muestreo, el
//! portapapeles se publica desde otro hilo (montar el mapa de bits de una
//! captura grande y releer un PNG para copiarlo eran decenas de ms), y la
//! ventana del recuadro se REUTILIZA entre tandas: crearla es una superficie
//! de composicion entera. La lista de ficheros solo se publica cuando TODOS
//! estan escritos: mientras tanto el portapapeles lleva la imagen suelta.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_geom::Rect;
use pixpin_pila::{AccionPila, Captura, Copia, Efecto, Esquina, IconoPila, Pila, TextosPila};
use pixpin_shell::Bandeja;
use pixpin_store::{Capturas, Catalogo, EsquinaPila, Ubicacion};
use windows::Win32::Foundation::HWND;

use crate::overlay::Recursos;

/// Lado mayor de la miniatura. Ciento sesenta porque el icono al 150 % mide
/// 126 px y la celda del panel 123: con menos se veria borrosa, y con mas
/// serian megas por tanda para pintar un sello.
const LADO_MINIATURA: u32 = 160;

/// Lo que cuenta el hilo que escribe un PNG al acabar.
type Escrito = (PathBuf, Result<(), String>);

/// Lo que se le pide al hilo del portapapeles. Uno solo y en orden: la
/// imagen suelta de una captura tiene que llegar ANTES que la lista de
/// ficheros que la incluye, o el pegado seria el de la tanda vieja.
enum Trabajo {
    Imagen(Arc<ImagenRgba>),
    ImagenYFicheros(Arc<ImagenRgba>, Vec<PathBuf>),
    /// Una copia pedida desde el panel: hay que releer el PNG. Devuelve
    /// cuantas se copiaron, para el aviso.
    Copia(Copia),
}

/// Hace un trabajo del portapapeles. Devuelve `Some` solo para las copias
/// del panel, que llevan aviso.
fn hacer(t: Trabajo) -> Option<Result<usize, String>> {
    match t {
        Trabajo::Imagen(imagen) => {
            if let Err(e) = pixpin_codec::copiar_imagen(&imagen) {
                tracing::warn!(?e, "la captura no se pudo copiar al portapapeles");
            }
            None
        }
        Trabajo::ImagenYFicheros(imagen, rutas) => {
            match pixpin_codec::copiar_imagen_y_ficheros(&imagen, &rutas) {
                Ok(()) => tracing::info!(cuantas = rutas.len(), "capturas apiladas publicadas"),
                Err(e) => tracing::warn!(?e, "las capturas apiladas no se pudieron publicar"),
            }
            None
        }
        Trabajo::Copia(copia) => Some(
            publicar(&copia)
                .map(|()| copia.rutas().len())
                .map_err(|e| format!("{e:#}")),
        ),
    }
}

/// El hilo del portapapeles. Nace con la primera captura y duerme en su
/// canal el resto del tiempo.
struct HiloPortapapeles {
    tx: Sender<Trabajo>,
}

impl HiloPortapapeles {
    fn lanzar(copiadas: Sender<Result<usize, String>>, aviso: HWND) -> Option<HiloPortapapeles> {
        let (tx, rx) = channel::<Trabajo>();
        let aviso = aviso.0 as isize;
        let lanzado = std::thread::Builder::new()
            .name("portapapeles-capturas".into())
            .spawn(move || {
                while let Ok(t) = rx.recv() {
                    if let Some(hecho) = hacer(t) {
                        let _ = copiadas.send(hecho);
                        pixpin_shell::despertar(HWND(aviso as *mut _));
                    }
                }
            });
        match lanzado {
            Ok(_) => Some(HiloPortapapeles { tx }),
            Err(e) => {
                tracing::warn!(?e, "no se pudo lanzar el hilo del portapapeles");
                None
            }
        }
    }
}

pub struct PilaCapturas {
    pila: Rc<RefCell<Pila>>,
    icono: Option<IconoPila>,
    /// Lo que el usuario pulso en el panel. La ventana solo apunta aqui y da
    /// un toque al bucle; el trabajo se hace en `atender`, fuera de su
    /// procedimiento de ventana.
    pedidos: Rc<RefCell<Vec<AccionPila>>>,
    esquina: Esquina,
    icono_ms: u64,
    /// La ventana principal, para despertar el bucle tras un clic del panel
    /// o cuando un PNG termina de escribirse.
    aviso: HWND,
    /// La ultima captura entera: es el mapa de bits que acompana a la lista
    /// de ficheros. Una sola en memoria; el portapapeles ya guarda otra igual.
    ultima: Option<Arc<ImagenRgba>>,
    /// Los PNG que todavia se estan escribiendo.
    escribiendo: HashSet<PathBuf>,
    hechos_tx: Sender<Escrito>,
    hechos_rx: Receiver<Escrito>,
    portapapeles: Option<HiloPortapapeles>,
    copiadas_tx: Sender<Result<usize, String>>,
    copiadas_rx: Receiver<Result<usize, String>>,
}

impl PilaCapturas {
    pub fn nueva(ajustes: &Capturas, aviso: HWND) -> PilaCapturas {
        let (hechos_tx, hechos_rx) = channel();
        let (copiadas_tx, copiadas_rx) = channel();
        PilaCapturas {
            pila: Rc::new(RefCell::new(Pila::nueva(ajustes.apilar_segundos))),
            icono: None,
            pedidos: Rc::new(RefCell::new(Vec::new())),
            esquina: match ajustes.esquina {
                EsquinaPila::ArribaIzquierda => Esquina::ArribaIzquierda,
                EsquinaPila::ArribaDerecha => Esquina::ArribaDerecha,
                EsquinaPila::AbajoIzquierda => Esquina::AbajoIzquierda,
                EsquinaPila::AbajoDerecha => Esquina::AbajoDerecha,
            },
            icono_ms: ajustes.icono_segundos as u64 * 1000,
            aviso,
            ultima: None,
            escribiendo: HashSet::new(),
            hechos_tx,
            hechos_rx,
            portapapeles: None,
            copiadas_tx,
            copiadas_rx,
        }
    }

    /// Manda un trabajo al hilo del portapapeles (lo lanza si hace falta).
    /// Si no hay hilo posible, se hace aqui: lento, pero la captura llega.
    fn al_portapapeles(&mut self, t: Trabajo) {
        if self.portapapeles.is_none() {
            self.portapapeles = HiloPortapapeles::lanzar(self.copiadas_tx.clone(), self.aviso);
        }
        let t = match &self.portapapeles {
            Some(h) => match h.tx.send(t) {
                Ok(()) => return,
                Err(std::sync::mpsc::SendError(t)) => {
                    self.portapapeles = None;
                    t
                }
            },
            None => t,
        };
        if let Some(hecho) = hacer(t) {
            let _ = self.copiadas_tx.send(hecho);
        }
    }

    /// Una captura recien hecha que iba al portapapeles. `region` es donde se
    /// recorto, en pixeles fisicos del escritorio virtual: decide el monitor
    /// del icono.
    ///
    /// El portapapeles se publica desde otro hilo; que falle el PNG o el
    /// icono se registra y se sigue: perder el montoncito es una molestia,
    /// perder la captura es el fallo.
    pub fn entrar(
        &mut self,
        imagen: ImagenRgba,
        region: Rect,
        recursos: &Recursos,
        ubicacion: &Ubicacion,
        textos: &Catalogo,
    ) -> Result<()> {
        let t0 = Instant::now();
        let imagen = Arc::new(imagen);
        // Lo primero y siempre: la imagen suelta al portapapeles, como antes
        // de que existiera la pila. Los ficheros, si tocan, llegan despues.
        self.al_portapapeles(Trabajo::Imagen(Arc::clone(&imagen)));
        if !self.pila.borrow().apilar_encendido() {
            return Ok(());
        }
        let ruta = match reservar_ruta(ubicacion) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(?e, "la captura no entra en la pila; queda copiada suelta");
                return Ok(());
            }
        };
        let captura = Captura {
            ruta: ruta.clone(),
            miniatura: miniatura_rapida(&imagen),
            ancho: imagen.ancho,
            alto: imagen.alto,
            elegida: true,
        };
        self.escribir_aparte(ruta, Arc::clone(&imagen));
        self.ultima = Some(imagen);

        let (monitor, escala) = monitor_de(region);
        let efecto = self.pila.borrow_mut().anadir(captura, monitor, escala);

        // Una tanda nueva puede caer en otro monitor con otra escala: solo
        // entonces el icono viejo se suelta y nace otro. Con la misma escala
        // se reutiliza (`refrescar` lo recoloca en su monitor).
        if efecto == Efecto::Empezada {
            self.pedidos.borrow_mut().clear();
            if self
                .icono
                .as_ref()
                .is_some_and(|i| i.escala_por_cien() != Some(escala))
            {
                self.icono = None;
            }
        }
        if let Err(e) = self.mostrar(recursos, textos) {
            tracing::warn!(?e, "el icono de la pila no se pudo mostrar");
        }
        tracing::info!(
            ?efecto,
            apiladas = self.pila.borrow().cuantas(),
            armada = self.pila.borrow().armada(),
            ms = t0.elapsed().as_millis() as u64,
            "captura en la pila"
        );
        Ok(())
    }

    /// Escribe el PNG en otro hilo y da un toque al bucle al acabar.
    fn escribir_aparte(&mut self, ruta: PathBuf, imagen: Arc<ImagenRgba>) {
        self.escribiendo.insert(ruta.clone());
        let tx = self.hechos_tx.clone();
        // Un HWND no cruza hilos como tal; su valor si, y PostMessage es de
        // las pocas llamadas que se pueden hacer desde cualquier hilo.
        let aviso = self.aviso.0 as isize;
        std::thread::spawn(move || {
            let hecho = pixpin_codec::guardar(&imagen, &ruta, pixpin_codec::FormatoImagen::Png)
                .map_err(|e| e.to_string());
            let _ = tx.send((ruta, hecho));
            pixpin_shell::despertar(HWND(aviso as *mut _));
        });
    }

    /// El dispositivo grafico se perdio y `recursos` ya es el nuevo: el
    /// icono (su superficie y sus miniaturas eran del viejo) se suelta y,
    /// si se veia, renace igual. Las capturas apiladas no se tocan.
    pub fn cambiar_dispositivo(&mut self, recursos: &Recursos, textos: &Catalogo) {
        if self.icono.take().is_some()
            && let Err(e) = self.mostrar(recursos, textos)
        {
            tracing::warn!(?e, "el icono de la pila no pudo renacer");
        }
    }

    /// Crea el icono si no existe, lo pone al dia y rearma su desvanecido.
    fn mostrar(&mut self, recursos: &Recursos, textos: &Catalogo) -> Result<()> {
        let titulo = titulo_de(textos, self.pila.borrow().cuantas());
        match &self.icono {
            Some(icono) => {
                icono.poner_titulo(titulo);
                icono.refrescar();
            }
            None => {
                let pedidos = Rc::clone(&self.pedidos);
                let aviso = self.aviso;
                let icono = IconoPila::nuevo(
                    &recursos.d3d(),
                    recursos.motor(),
                    Rc::clone(&self.pila),
                    self.esquina,
                    TextosPila {
                        titulo,
                        copiar_elegidas: textos.t("pila-copiar-elegidas"),
                        copiar_todas: textos.t("pila-copiar-todas"),
                        quitar: textos.t("pila-quitar"),
                    },
                    // Se llama desde el procedimiento de la ventanita: apuntar
                    // y volver. Sin el toque, el pedido esperaria a que pasara
                    // cualquier otra cosa en la ventana principal (la misma
                    // trampa que `WM_DESPERTAR` resolvio para los pines).
                    Box::new(move |accion| {
                        pedidos.borrow_mut().push(accion);
                        pixpin_shell::despertar(aviso);
                    }),
                )
                .context("no se pudo crear el icono de la pila")?;
                self.icono = Some(icono);
            }
        }
        self.armar_desvanecido();
        Ok(())
    }

    /// El recuadro sin armar se va solo a los `icono_segundos` de la ultima
    /// captura (cero: se queda hasta que lo quiten). Armado, no se va nunca
    /// solo: lo mantiene el usuario. Con el panel abierto tampoco.
    fn armar_desvanecido(&self) {
        let Some(icono) = &self.icono else {
            return;
        };
        icono.armar_desvanecido(ms_del_desvanecido(
            self.pila.borrow().armada(),
            icono.abierto(),
            self.icono_ms,
        ));
    }

    /// Atiende lo que haya pendiente: los PNG que acabaron de escribirse, las
    /// copias que acabo el hilo del portapapeles y lo que se pulso en el
    /// panel. Se llama en cada vuelta del bucle principal; sin nada pendiente
    /// no hace nada.
    pub fn atender(&mut self, textos: &Catalogo, bandeja: &mut Bandeja) {
        self.recoger_escritos(textos);

        while let Ok(hecho) = self.copiadas_rx.try_recv() {
            match hecho {
                Ok(cuantas) => {
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("cuantas", cuantas.to_string());
                    let _ = bandeja.avisar(
                        &textos.t("app-nombre"),
                        &textos.t_args("pila-copiadas", &args),
                    );
                    tracing::info!(cuantas, "pila copiada");
                }
                Err(e) => tracing::warn!(%e, "la pila no se pudo copiar"),
            }
        }

        let pedidos: Vec<AccionPila> = self.pedidos.borrow_mut().drain(..).collect();
        for pedido in pedidos {
            match pedido {
                AccionPila::CopiarElegidas | AccionPila::CopiarTodas => {
                    // Con un PNG a medio escribir, copiar publicaria un
                    // fichero truncado. El pedido vuelve a la cola: el hilo
                    // que escribe da un toque al acabar y entonces se atiende.
                    if !self.escribiendo.is_empty() {
                        self.pedidos.borrow_mut().push(pedido);
                        continue;
                    }
                    let solo_elegidas = pedido == AccionPila::CopiarElegidas;
                    let copia = self.pila.borrow().que_copiar(solo_elegidas);
                    // Sin nada elegido el boton no hace nada, y el panel se
                    // queda abierto para que se elija.
                    let Some(copia) = copia else { continue };
                    self.al_portapapeles(Trabajo::Copia(copia));
                    if self.pila.borrow().armada() {
                        // Armada se sigue agrupando: solo se cierra el panel.
                        if let Some(icono) = &self.icono {
                            icono.cerrar_panel();
                        }
                        self.armar_desvanecido();
                    } else {
                        self.cerrar();
                    }
                }
                AccionPila::Cambiada => self.poner_al_dia(textos),
                AccionPila::Armada => {
                    tracing::info!(
                        apiladas = self.pila.borrow().cuantas(),
                        "pila armada: se agrupa hasta soltarla"
                    );
                    self.armar_desvanecido();
                }
                // El portapapeles NO se toca: quitar el montoncito de la
                // esquina no es arrepentirse de lo copiado.
                AccionPila::Cerrar => {
                    tracing::info!(apiladas = self.pila.borrow().cuantas(), "pila soltada");
                    self.cerrar();
                }
            }
        }
    }

    /// Recoge los PNG terminados y, si ya no queda ninguno a medias y la
    /// tanda tiene varias, publica la lista de ficheros junto a la imagen.
    fn recoger_escritos(&mut self, textos: &Catalogo) {
        let mut alguno = false;
        while let Ok((ruta, hecho)) = self.hechos_rx.try_recv() {
            alguno = true;
            self.escribiendo.remove(&ruta);
            if let Err(e) = hecho {
                // Un fichero que no existe no se puede pegar: fuera de la
                // pila. La imagen suelta sigue en el portapapeles.
                tracing::warn!(%e, ruta = %ruta.display(), "una captura no se pudo guardar");
                let _ = std::fs::remove_file(&ruta);
                let i = self
                    .pila
                    .borrow()
                    .capturas()
                    .iter()
                    .position(|c| c.ruta == ruta);
                if let Some(i) = i {
                    self.pila.borrow_mut().quitar(i);
                    self.poner_al_dia(textos);
                }
            }
        }
        if !alguno || !self.escribiendo.is_empty() {
            return;
        }
        let copia = self.pila.borrow().que_copiar(false);
        if let (Some(Copia::Ficheros { rutas, .. }), Some(imagen)) = (copia, self.ultima.clone()) {
            self.al_portapapeles(Trabajo::ImagenYFicheros(imagen, rutas));
        }
    }

    /// Tras un cambio en la pila: cierra si quedo vacia; si no, repinta y
    /// rearma el desvanecido que toque (el panel pudo cerrarse).
    fn poner_al_dia(&mut self, textos: &Catalogo) {
        let cuantas = self.pila.borrow().cuantas();
        if cuantas == 0 {
            self.cerrar();
        } else if let Some(icono) = &self.icono {
            icono.poner_titulo(titulo_de(textos, cuantas));
            icono.refrescar();
            self.armar_desvanecido();
        }
    }

    fn cerrar(&mut self) {
        self.icono = None;
        self.ultima = None;
        self.pila.borrow_mut().vaciar();
    }
}

/// Cuanto espera el recuadro para irse solo. Cero es «no se va solo»: armado
/// lo mantiene el usuario, abierto se esta eligiendo, y `icono_segundos = 0`
/// lo pide el TOML.
fn ms_del_desvanecido(armada: bool, abierto: bool, icono_ms: u64) -> u32 {
    if armada || abierto {
        0
    } else {
        icono_ms.min(u32::MAX as u64) as u32
    }
}

fn titulo_de(textos: &Catalogo, cuantas: usize) -> String {
    if cuantas <= 1 {
        return textos.t("pila-titulo-una");
    }
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("cuantas", cuantas.to_string());
    textos.t_args("pila-titulo-varias", &args)
}

/// Elige la ruta del PNG y la RESERVA creando el fichero vacio.
///
/// Reservar importa ahora que el PNG se escribe despues: sin el fichero en
/// su sitio, dos capturas seguidas verian libre el mismo `captura-NNNN.png`
/// y la segunda pisaria a la primera.
fn reservar_ruta(ubicacion: &Ubicacion) -> Result<PathBuf> {
    let ruta = crate::ruta_captura_libre(ubicacion)?;
    // `CF_HDROP` rechaza las rutas relativas; en portable la raiz sale del
    // ejecutable y ya es absoluta, pero no cuesta nada asegurarlo.
    let ruta: PathBuf = std::path::absolute(&ruta).unwrap_or(ruta);
    reservar(&ruta)?;
    Ok(ruta)
}

fn reservar(ruta: &Path) -> Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(ruta)
        .map(|_| ())
        .with_context(|| format!("no se pudo reservar {}", ruta.display()))
}

/// El tamano de la miniatura: lado mayor a `LADO_MINIATURA`, sin deformar y
/// sin agrandar nunca una captura que ya es pequena.
fn tamano_miniatura(ancho: u32, alto: u32) -> (u32, u32) {
    let mayor = ancho.max(alto);
    if mayor <= LADO_MINIATURA {
        return (ancho.max(1), alto.max(1));
    }
    let escalar = |v: u32| ((v as u64 * LADO_MINIATURA as u64) / mayor as u64).max(1) as u32;
    (escalar(ancho), escalar(alto))
}

/// La miniatura por muestreo: cada pixel de salida es la media de una rejilla
/// de 3x3 puntos de su trozo de la captura.
///
/// No usa `pixpin_codec::redimensionar`, que es el filtro bueno pero recorre
/// la captura entera (y obliga a clonarla): aqui corre en el hilo del gancho
/// de raton y cada milisegundo es uno en que los gestos van sordos. Para un
/// sello de 160 px, nueve muestras por pixel no se distinguen del filtro.
fn miniatura_rapida(imagen: &ImagenRgba) -> ImagenRgba {
    let (ancho, alto) = tamano_miniatura(imagen.ancho, imagen.alto);
    let mut pixeles = Vec::with_capacity((ancho * alto * 4) as usize);
    // Una imagen incoherente no puede tirar la app desde aqui: sale gris.
    let coherente = imagen.ancho > 0
        && imagen.alto > 0
        && imagen.pixeles.len() == (imagen.ancho as usize * imagen.alto as usize * 4);
    for y in 0..alto {
        for x in 0..ancho {
            if !coherente {
                pixeles.extend_from_slice(&[128, 128, 128, 255]);
                continue;
            }
            let mut suma = [0u32; 4];
            for j in 0..3u64 {
                for i in 0..3u64 {
                    // El punto (i, j) de la rejilla, centrado en su tercio.
                    let sx = ((x as u64 * 6 + i * 2 + 1) * imagen.ancho as u64
                        / (ancho as u64 * 6))
                        .min(imagen.ancho as u64 - 1);
                    let sy = ((y as u64 * 6 + j * 2 + 1) * imagen.alto as u64 / (alto as u64 * 6))
                        .min(imagen.alto as u64 - 1);
                    let k = ((sy * imagen.ancho as u64 + sx) * 4) as usize;
                    for (c, s) in suma.iter_mut().enumerate() {
                        *s += imagen.pixeles[k + c] as u32;
                    }
                }
            }
            pixeles.extend(suma.iter().map(|s| (s / 9) as u8));
        }
    }
    ImagenRgba {
        ancho,
        alto,
        pixeles,
    }
}

/// El monitor que contiene la region y su escala; el principal si no toca
/// ninguno, y uno de relleno si ni siquiera se pueden enumerar (el icono
/// saldra en la esquina del principal, que es mejor que no salir).
fn monitor_de(region: Rect) -> (Rect, u32) {
    let relleno = (
        Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        },
        100,
    );
    match pixpin_capture::enumerar_monitores() {
        Ok(d) => d
            .monitores()
            .iter()
            .find(|m| m.area.interseccion(region).is_some())
            .or_else(|| d.principal())
            // El area de TRABAJO y no la completa: abajo a la derecha, con la
            // completa, el icono naceria encima de la barra de tareas.
            .map_or(relleno, |m| (m.area_trabajo, m.escala_por_cien)),
        Err(_) => relleno,
    }
}

/// Publica una copia pedida desde el panel. Aqui si hay que releer el PNG de
/// la que viaja como mapa de bits: en memoria solo queda la ultima.
fn publicar(copia: &Copia) -> Result<()> {
    let imagen = pixpin_codec::cargar(copia.imagen())
        .with_context(|| format!("no se pudo releer {}", copia.imagen().display()))?;
    match copia {
        Copia::Imagen { .. } => pixpin_codec::copiar_imagen(&imagen)?,
        Copia::Ficheros { rutas, .. } => pixpin_codec::copiar_imagen_y_ficheros(&imagen, rutas)?,
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn lisa(ancho: u32, alto: u32, color: [u8; 4]) -> ImagenRgba {
        ImagenRgba {
            ancho,
            alto,
            pixeles: color.repeat((ancho * alto) as usize),
        }
    }

    #[test]
    fn la_miniatura_no_deforma_ni_pasa_del_lado_mayor() {
        assert_eq!(tamano_miniatura(3200, 1600), (160, 80));
        assert_eq!(tamano_miniatura(1000, 4000), (40, 160));
    }

    #[test]
    fn solo_el_recuadro_sin_armar_y_cerrado_se_va_solo() {
        assert_eq!(ms_del_desvanecido(false, false, 8_000), 8_000);
        // Armado lo mantiene el usuario: no se va nunca solo.
        assert_eq!(ms_del_desvanecido(true, false, 8_000), 0);
        // Con el panel abierto se esta eligiendo.
        assert_eq!(ms_del_desvanecido(false, true, 8_000), 0);
        // `icono_segundos = 0`: se queda hasta que lo quiten.
        assert_eq!(ms_del_desvanecido(false, false, 0), 0);
    }

    #[test]
    fn una_captura_pequena_no_se_agranda() {
        assert_eq!(tamano_miniatura(90, 60), (90, 60));
        assert_eq!(tamano_miniatura(160, 160), (160, 160));
    }

    #[test]
    fn una_tira_finisima_no_se_queda_en_cero_pixeles() {
        // Caso negativo: 8000x3 daria alto 0 y la miniatura saldria vacia.
        assert_eq!(tamano_miniatura(8000, 3), (160, 1));
        let m = miniatura_rapida(&lisa(8000, 3, [10, 20, 30, 255]));
        assert_eq!((m.ancho, m.alto), (160, 1));
        assert_eq!(m.pixeles.len(), 160 * 4);
    }

    #[test]
    fn la_miniatura_rapida_conserva_el_color_y_el_tamano_de_los_bytes() {
        let m = miniatura_rapida(&lisa(1920, 1080, [200, 100, 50, 255]));
        assert_eq!((m.ancho, m.alto), (160, 90));
        assert_eq!(m.pixeles.len(), 160 * 90 * 4);
        assert!(m.pixeles.chunks(4).all(|p| p == [200, 100, 50, 255]));
    }

    #[test]
    fn la_miniatura_rapida_pone_cada_mitad_en_su_lado() {
        // Izquierda roja, derecha azul: si el muestreo cruzara los ejes o
        // leyera fuera de su trozo, los colores saldrian mezclados o girados.
        let (ancho, alto) = (640u32, 320u32);
        let mut pixeles = Vec::new();
        for _ in 0..alto {
            for x in 0..ancho {
                pixeles.extend_from_slice(if x < ancho / 2 {
                    &[255, 0, 0, 255]
                } else {
                    &[0, 0, 255, 255]
                });
            }
        }
        let m = miniatura_rapida(&ImagenRgba {
            ancho,
            alto,
            pixeles,
        });
        let pixel = |x: u32, y: u32| {
            let k = ((y * m.ancho + x) * 4) as usize;
            [m.pixeles[k], m.pixeles[k + 1], m.pixeles[k + 2]]
        };
        assert_eq!(pixel(2, 2), [255, 0, 0]);
        assert_eq!(pixel(m.ancho - 3, m.alto - 3), [0, 0, 255]);
    }

    #[test]
    fn una_imagen_incoherente_da_una_miniatura_gris_y_no_un_panico() {
        // Caso negativo: menos bytes de los que dicen ancho y alto.
        let rota = ImagenRgba {
            ancho: 400,
            alto: 300,
            pixeles: vec![0; 16],
        };
        let m = miniatura_rapida(&rota);
        assert_eq!(m.pixeles.len(), (m.ancho * m.alto * 4) as usize);
    }

    #[test]
    fn reservar_dos_veces_la_misma_ruta_falla() {
        // Es lo que impide que dos capturas seguidas se pisen el fichero
        // mientras el primero aun se esta escribiendo.
        let ruta =
            std::env::temp_dir().join(format!("pixpin-pila-reserva-{}.png", std::process::id()));
        let _ = std::fs::remove_file(&ruta);
        assert!(reservar(&ruta).is_ok());
        assert!(reservar(&ruta).is_err());
        let _ = std::fs::remove_file(&ruta);
    }
}
