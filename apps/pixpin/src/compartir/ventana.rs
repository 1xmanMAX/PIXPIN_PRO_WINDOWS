//! **La ventana de la hoja de compartir**: la tarjeta que sube desde abajo,
//! como la del movil, y sus cuatro salidas.
//!
//! Vive en su propio hilo, como el lector o el universo: quien la abre (el
//! chat, el editor, un lector) sigue a lo suyo, y lo que tarda en hacerse un
//! PDF de cuarenta paginas no para a nadie. Dentro hay un segundo hilo que
//! hace el fichero de lo elegido en cuanto se elige, para decir **cuanto
//! pesa de verdad** (el movil hace lo mismo): cuando se pulsa una salida, el
//! fichero ya esta hecho y es ese el que se manda.
//!
//! Las salidas son lo que Windows ya trae:
//!
//! - **Compartir**: el panel Compartir de Windows
//!   (`pixpin_shell::compartir`), el del Explorador, con Correo, Teams,
//!   Compartir en proximidad y lo que haya instalado. La hoja se queda
//!   abierta hasta que el panel termina: el panel cuelga de ella.
//! - **Guardar como…**: el dialogo de guardar de siempre, con el tipo ya
//!   puesto (o el de carpeta, si son varios ficheros).
//! - **Copiar**: al portapapeles; una imagen, como imagen (se pega en
//!   cualquier sitio), y lo demas como fichero (se pega en el Explorador).
//! - **Wi-Fi**: al aparato de otra persona, el «Enviar por Wi-Fi» del movil.
//!
//! Teclado: flechas para cambiar de formato, Intro comparte, Ctrl+S guarda,
//! Ctrl+C copia, Esc cierra.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_geom::Rect;
use pixpin_render::icono::{Icono, material as mi};
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma, Ubicacion};
use pixpin_ui::hoja_compartir::{
    self as hc, Caja, Compartible, Cuantas, Destino, Disposicion, Estado, Salida as Boton,
};

use super::{Cosa, Preparado, Salida};
use crate::caja_dibujo::hex;

const FONDO: Color = hex(0x1c1d22);
const REDONDEL: Color = hex(0x2a2b31);
const ENCIMA: Color = hex(0x34353c);
const TEXTO: Color = hex(0xe8e8ec);
const APAGADO: Color = hex(0x9a9aa2);
const ENCENDIDO: Color = hex(0x1e88e5);

/// Cuanto dura un aviso en el pie.
const AVISO: Duration = Duration::from_millis(3500);
/// Lo que se deja una carpeta de ficheros compartidos antes de borrarla.
const EDAD_MAXIMA: Duration = Duration::from_secs(24 * 3600);

/// **Abre la hoja** para `cosa`, en su propio hilo. `ubicacion` hace falta
/// para mandar por la wifi.
pub(crate) fn abrir(idioma: Idioma, ubicacion: Ubicacion, cosa: Cosa) {
    let lanzado = std::thread::Builder::new()
        .name("hoja-compartir".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            if let Err(e) = hilo(&textos, idioma, ubicacion, cosa) {
                tracing::warn!(?e, "la hoja de compartir no se pudo abrir");
                pixpin_shell::exportar::informar(
                    windows::Win32::Foundation::HWND::default(),
                    &textos.t("compartir-titulo"),
                    &format!("{}\n\n{e:#}", textos.t("compartir-no-se-pudo")),
                );
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de la hoja de compartir");
    }
}

/// La clave de un fichero hecho: el formato y las paginas.
type Clave = (String, Vec<String>);

struct Hecho {
    clave: Clave,
    salida: std::result::Result<Salida, String>,
}

/// El hilo que hace los ficheros: siempre lo ultimo que se pidio, que lo
/// anterior ya no se mira.
fn hilo_de_ficheros(
    p: Arc<Preparado>,
    pedidos: mpsc::Receiver<Clave>,
    hechos: mpsc::Sender<Hecho>,
    ventana: isize,
) {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let base = super::carpeta_temporal();
    let mut n = 0u32;
    while let Ok(mut clave) = pedidos.recv() {
        while let Ok(otra) = pedidos.try_recv() {
            clave = otra;
        }
        n += 1;
        let carpeta = base.join(format!("{}-{}-{n}", std::process::id(), marca_de_tiempo()));
        let salida = super::generar(&p, &clave.0, &clave.1, &carpeta).map_err(|e| {
            tracing::warn!(?e, formato = %clave.0, "no se pudo hacer lo que se comparte");
            format!("{e:#}")
        });
        if hechos.send(Hecho { clave, salida }).is_err() {
            return;
        }
        pixpin_shell::overlay::despertar(ventana);
    }
}

fn marca_de_tiempo() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Lo que se ve en el pie mientras tanto.
enum Pie {
    Preparando,
    Listo(u64, usize),
    Fallo,
    MarcaAlguna,
}

struct Hoja {
    c: Compartible,
    e: Estado,
    escala: f32,
    raton: (f32, f32),
    hechos: HashMap<Clave, std::result::Result<Arc<Salida>, String>>,
    pedido: Option<Clave>,
    /// La salida pulsada antes de que el fichero estuviera: se hace al
    /// llegar.
    pendiente: Option<Boton>,
    aviso: Option<(String, Instant)>,
    /// El panel de Windows esta abierto sobre la hoja.
    esperando_panel: bool,
}

impl Hoja {
    fn clave(&self) -> Option<Clave> {
        if !self.e.hay_que_preparar(&self.c) {
            return None;
        }
        // Con su interruptor quitado se genera otra cosa (el PDF limpio).
        Some((self.e.id_a_generar(&self.c)?, self.e.elegidas(&self.c)))
    }

    fn pie(&self) -> Pie {
        let Some(clave) = self.clave() else {
            return Pie::MarcaAlguna;
        };
        match self.hechos.get(&clave) {
            Some(Ok(s)) => Pie::Listo(s.bytes, s.ficheros.len()),
            Some(Err(_)) => Pie::Fallo,
            None => Pie::Preparando,
        }
    }

    fn lista(&self) -> Option<Arc<Salida>> {
        self.hechos.get(&self.clave()?)?.as_ref().ok().cloned()
    }

    fn avisar(&mut self, texto: String) {
        self.aviso = Some((texto, Instant::now()));
    }
}

fn hilo(textos: &Catalogo, idioma: Idioma, ubicacion: Ubicacion, cosa: Cosa) -> Result<()> {
    super::limpiar_viejos(&super::carpeta_temporal(), EDAD_MAXIMA);
    let preparado = super::preparar(cosa, textos)?;
    let c = super::compartible(&preparado, textos);
    if c.formatos.is_empty() {
        pixpin_shell::exportar::informar(
            windows::Win32::Foundation::HWND::default(),
            &textos.t("compartir-titulo"),
            &textos.t("compartir-nada"),
        );
        return Ok(());
    }
    let preparado = Arc::new(preparado);

    // En el monitor del raton, abajo y centrada: donde sale la hoja del movil.
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let cursor = pixpin_shell::posicion_del_cursor();
    let monitor = monitores
        .monitor_en(cursor)
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let (w, h) = hc::tamano(&c, escala);
    let (w, h) = (w.ceil() as u32, h.ceil() as u32);
    let util = monitor.area_trabajo;
    let area = Rect {
        x: util.x + (util.ancho as i32 - w as i32) / 2,
        y: util.abajo() - h as i32 - (16.0 * escala) as i32,
        ancho: w,
        alto: h,
    };
    let recursos = crate::overlay::Recursos::nuevos()?;
    let ventana = VentanaOverlay::nueva_normal(area, &textos.t("compartir-titulo"))
        .context("no se pudo abrir la ventana de compartir")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(&motor, &recursos.d3d(), ventana.handle(), w, h)
        .context("sin superficie para la hoja de compartir")?;
    ventana.mostrar();
    ventana.enfocar();

    let (tx_pedidos, rx_pedidos) = mpsc::channel::<Clave>();
    let (tx_hechos, rx_hechos) = mpsc::channel::<Hecho>();
    let hwnd = ventana.handle().0 as isize;
    let para_el_hilo = preparado.clone();
    std::thread::Builder::new()
        .name("hoja-compartir-ficheros".into())
        .spawn(move || hilo_de_ficheros(para_el_hilo, rx_pedidos, tx_hechos, hwnd))
        .context("sin hilo para hacer los ficheros")?;

    let mut e = Estado::nuevo(&c);
    // Con varias paginas marcadas se empieza por un formato que las lleve
    // todas; con una, por el primero.
    let primero = if e.marcadas.len() > 1 {
        c.formatos
            .iter()
            .position(|f| f.cuantas != Cuantas::Una)
            .unwrap_or(0)
    } else {
        0
    };
    e.elegir_formato(&c, primero);
    let mut hoja = Hoja {
        c,
        e,
        escala,
        raton: (-1.0, -1.0),
        hechos: HashMap::new(),
        pedido: None,
        pendiente: None,
        aviso: None,
        esperando_panel: false,
    };
    let terminado = Arc::new(AtomicBool::new(false));
    let coma = textos
        .t("compartir-coma-decimal")
        .chars()
        .next()
        .unwrap_or(',');

    let mut hay_que_pintar = true;
    let mut vivo = true;
    // Donde se agarro el boton «Arrastrar», mientras no se suelte.
    let mut agarre: Option<pixpin_geom::Punto> = None;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        while let Ok(hecho) = rx_hechos.try_recv() {
            if hoja.pedido.as_ref() == Some(&hecho.clave) {
                hoja.pedido = None;
            }
            hoja.hechos.insert(hecho.clave, hecho.salida.map(Arc::new));
            hay_que_pintar = true;
        }
        let mut arrastrar_ya = false;
        for (hw, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hw != ventana.handle() {
                continue;
            }
            hay_que_pintar = true;
            let local = |p: pixpin_geom::Punto| ((p.x - area.x) as f32, (p.y - area.y) as f32);
            match evento {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    hoja.raton = local(p);
                    // Agarrar «Arrastrar» y moverse ya es llevarse el
                    // fichero, sin pasar por la Salida.
                    if let Some(desde) = agarre
                        && crate::salida::supera_umbral((desde.x, desde.y), (p.x, p.y), escala)
                    {
                        agarre = None;
                        arrastrar_ya = true;
                    }
                }
                EventoOverlay::BotonSoltado(p) => {
                    hoja.raton = local(p);
                    // Pulsar «Arrastrar» sin moverse: el fichero se queda en
                    // pantalla (la Salida), para arrastrarlo con calma.
                    if agarre.take().is_some() {
                        let d = hc::disponer(&hoja.c, &hoja.e, hoja.escala);
                        if hc::destino_en(&d, hoja.raton.0, hoja.raton.1)
                            == Destino::Salida(Boton::Arrastrar)
                        {
                            vivo = pulsar(
                                &mut hoja,
                                Boton::Arrastrar,
                                &ventana,
                                &preparado,
                                textos,
                                idioma,
                                &ubicacion,
                                &terminado,
                            );
                        }
                    }
                }
                EventoOverlay::BotonPulsado(p) => {
                    hoja.raton = local(p);
                    let d = hc::disponer(&hoja.c, &hoja.e, hoja.escala);
                    match hc::destino_en(&d, hoja.raton.0, hoja.raton.1) {
                        // Se decide al soltar o al moverse: puede ser un
                        // clic o el principio de un arrastre.
                        Destino::Salida(Boton::Arrastrar)
                            if matches!(hoja.pie(), Pie::Listo(..)) =>
                        {
                            agarre = Some(p);
                        }
                        Destino::Cerrar => vivo = false,
                        Destino::Formato(i) => hoja.e.elegir_formato(&hoja.c, i),
                        Destino::Pagina(k) => hoja.e.tocar_pagina(&hoja.c, &k),
                        Destino::Todas => hoja.e.todas(&hoja.c),
                        Destino::Ninguna => hoja.e.ninguna(),
                        Destino::Interruptor => hoja.e.alternar_interruptor(&hoja.c),
                        Destino::Salida(s) => {
                            vivo = pulsar(
                                &mut hoja, s, &ventana, &preparado, textos, idioma, &ubicacion,
                                &terminado,
                            );
                        }
                        Destino::Nada => {}
                    }
                }
                EventoOverlay::Rueda(delta) => {
                    let filas = -(delta as f32 / 120.0).round() as i32;
                    let filas = if filas == 0 { -delta.signum() } else { filas };
                    hoja.e.correr(&hoja.c, filas);
                }
                EventoOverlay::Tecla { vk, ctrl, .. } => match (vk, ctrl) {
                    (0x1B, _) => vivo = false,
                    (0x0D, _) => {
                        vivo = pulsar(
                            &mut hoja,
                            Boton::Compartir,
                            &ventana,
                            &preparado,
                            textos,
                            idioma,
                            &ubicacion,
                            &terminado,
                        )
                    }
                    (0x53, true) => {
                        vivo = pulsar(
                            &mut hoja,
                            Boton::Guardar,
                            &ventana,
                            &preparado,
                            textos,
                            idioma,
                            &ubicacion,
                            &terminado,
                        )
                    }
                    (0x43, true) => {
                        vivo = pulsar(
                            &mut hoja,
                            Boton::Copiar,
                            &ventana,
                            &preparado,
                            textos,
                            idioma,
                            &ubicacion,
                            &terminado,
                        )
                    }
                    (0x25 | 0x27, false) => {
                        let visibles = hoja.e.visibles(&hoja.c);
                        if let Some(k) = visibles.iter().position(|&i| i == hoja.e.formato) {
                            let otro = if vk == 0x25 {
                                k.checked_sub(1)
                            } else {
                                Some(k + 1).filter(|&n| n < visibles.len())
                            };
                            if let Some(n) = otro {
                                hoja.e.elegir_formato(&hoja.c, visibles[n]);
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        if !vivo {
            break;
        }
        if arrastrar_ya && let Some(s) = hoja.lista() {
            // La captura fuera: `DoDragDrop` lleva el raton el solo.
            ventana.soltar_raton();
            let carga = match s.ficheros.as_slice() {
                [uno] => pixpin_pin::Carga::Fichero(uno.clone()),
                varios => pixpin_pin::Carga::Ficheros(varios.to_vec()),
            };
            match pixpin_pin::arrastrar(carga) {
                Ok(pixpin_pin::ResultadoArrastre::Soltado) => {
                    hoja.avisar(textos.t("salida-soltado"))
                }
                Ok(pixpin_pin::ResultadoArrastre::Cancelado) => {}
                Err(e) => {
                    tracing::warn!(?e, "no se pudo arrastrar desde la hoja de compartir");
                    hoja.avisar(textos.t("compartir-no-se-pudo"));
                }
            }
            hay_que_pintar = true;
        }
        // El panel de Windows ya se cerro, mandando algo o no: la hoja ya
        // no tiene nada que hacer.
        if terminado.swap(false, Ordering::SeqCst) {
            break;
        }

        // Lo elegido, a hacer si no esta hecho ni pedido.
        if let Some(clave) = hoja.clave()
            && !hoja.hechos.contains_key(&clave)
            && hoja.pedido.as_ref() != Some(&clave)
        {
            hoja.pedido = Some(clave.clone());
            let _ = tx_pedidos.send(clave);
        }
        // Una salida que esperaba a su fichero.
        if let Some(s) = hoja.pendiente
            && !matches!(hoja.pie(), Pie::Preparando)
        {
            hoja.pendiente = None;
            vivo = pulsar(
                &mut hoja, s, &ventana, &preparado, textos, idioma, &ubicacion, &terminado,
            );
            hay_que_pintar = true;
        }
        if hoja
            .aviso
            .as_ref()
            .is_some_and(|(_, desde)| desde.elapsed() > AVISO)
        {
            hoja.aviso = None;
            hay_que_pintar = true;
        }
        if hay_que_pintar {
            hay_que_pintar = false;
            if let Ok(destino) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&destino, |p: &Pintor| pintar(&hoja, p, textos, coma));
                let _ = superficie.presentar();
            }
        }
        pixpin_shell::overlay::esperar_eventos(Some(
            if hoja.pedido.is_some() || hoja.aviso.is_some() {
                100
            } else {
                500
            },
        ));
    }
    Ok(())
}

/// **Una salida pulsada.** Devuelve si la hoja sigue abierta.
#[allow(clippy::too_many_arguments)] // la hoja, la salida y lo que cada salida necesita
fn pulsar(
    hoja: &mut Hoja,
    s: Boton,
    ventana: &VentanaOverlay,
    p: &Preparado,
    textos: &Catalogo,
    idioma: Idioma,
    ubicacion: &Ubicacion,
    terminado: &Arc<AtomicBool>,
) -> bool {
    let salida = match hoja.pie() {
        Pie::MarcaAlguna => {
            hoja.avisar(textos.t("compartir-marca-alguna"));
            return true;
        }
        Pie::Preparando => {
            // Se hace en cuanto este: pulsar y esperar sin que pase nada
            // parece un clic perdido.
            hoja.pendiente = Some(s);
            hoja.avisar(textos.t("compartir-en-cuanto-este"));
            return true;
        }
        Pie::Fallo => {
            hoja.avisar(textos.t("compartir-no-se-pudo"));
            return true;
        }
        Pie::Listo(..) => match hoja.lista() {
            Some(s) => s,
            None => return true,
        },
    };
    match s {
        Boton::Compartir => {
            if hoja.esperando_panel {
                return true;
            }
            let titulo = match salida.ficheros.as_slice() {
                [uno] => uno
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.titulo.clone()),
                _ => p.titulo.clone(),
            };
            let aviso = terminado.clone();
            let hwnd_hoja = ventana.handle().0 as isize;
            let al_terminar: pixpin_shell::compartir::AlTerminar = Arc::new(move || {
                aviso.store(true, Ordering::SeqCst);
                pixpin_shell::overlay::despertar(hwnd_hoja);
            });
            match pixpin_shell::compartir::compartir_avisando(
                ventana.handle(),
                &salida.ficheros,
                &titulo,
                al_terminar,
            ) {
                Ok(()) => {
                    hoja.esperando_panel = true;
                    hoja.avisar(textos.t("compartir-panel-abierto"));
                }
                Err(e) => {
                    // Un Windows sin el panel o una directiva de empresa: se
                    // deja en el portapapeles, que el gesto no se quede en
                    // nada.
                    tracing::warn!(?e, "el panel Compartir de Windows no se abrio");
                    let _ = pixpin_codec::portapapeles::copiar_ficheros(&salida.ficheros);
                    hoja.avisar(textos.t("chat-compartir-sin-panel"));
                }
            }
            true
        }
        Boton::Guardar => {
            let hecho = guardar(ventana, &salida, hoja, textos);
            match hecho {
                Ok(Some(ruta)) => {
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("nombre", pixpin_docs::nombre(&ruta));
                    hoja.avisar(textos.t_args("compartir-guardado", &args));
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(?e, "no se pudo guardar lo compartido");
                    hoja.avisar(textos.t("compartir-no-se-pudo"));
                }
            }
            true
        }
        Boton::Copiar => {
            let hecho = match (&salida.imagen, salida.ficheros.as_slice()) {
                (Some(img), _) => {
                    pixpin_codec::portapapeles::copiar_imagen_y_ficheros(img, &salida.ficheros)
                }
                (None, [uno]) if es_texto(uno) => match std::fs::read_to_string(uno) {
                    Ok(t) => pixpin_codec::portapapeles::copiar_texto(&t),
                    Err(_) => pixpin_codec::portapapeles::copiar_ficheros(&salida.ficheros),
                },
                _ => pixpin_codec::portapapeles::copiar_ficheros(&salida.ficheros),
            };
            match hecho {
                Ok(()) => hoja.avisar(textos.t("compartir-copiado")),
                Err(e) => {
                    tracing::warn!(?e, "no se pudo copiar lo compartido");
                    hoja.avisar(textos.t("compartir-no-se-pudo"));
                }
            }
            true
        }
        Boton::Wifi => {
            // La ventana de sincronizar se abre en su hilo con los ficheros:
            // la hoja ya ha cumplido.
            crate::sincronizar::enviar_por_wifi(idioma, ubicacion.clone(), salida.ficheros.clone());
            false
        }
        Boton::Arrastrar => {
            // El fichero hecho, en pantalla para arrastrarlo: la hoja tapa
            // lo que hay detras y ya ha cumplido.
            crate::salida::mostrar(salida.ficheros.clone(), textos.t("salida-titulo"));
            false
        }
    }
}

/// Lo que al copiarse se quiere pegar como texto, no como fichero.
fn es_texto(ruta: &std::path::Path) -> bool {
    matches!(
        ruta.extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .as_deref(),
        Some("txt" | "csv" | "md")
    )
}

/// «Guardar como…»: un fichero, con su tipo ya puesto; varios, en una
/// carpeta. Devuelve donde quedo, o `None` si se cancelo.
fn guardar(
    ventana: &VentanaOverlay,
    salida: &Salida,
    hoja: &Hoja,
    textos: &Catalogo,
) -> Result<Option<PathBuf>> {
    match salida.ficheros.as_slice() {
        [uno] => {
            let ext = uno
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default();
            let tronco = uno
                .file_stem()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let tipo = hoja
                .e
                .formato(&hoja.c)
                .map(|f| f.nombre.clone())
                .unwrap_or_else(|| ext.to_uppercase());
            let Some(destino) =
                pixpin_shell::guardar::pedir_ruta_para(ventana.handle(), &tronco, &tipo, &ext)
            else {
                return Ok(None);
            };
            if destino != *uno {
                std::fs::copy(uno, &destino)
                    .with_context(|| format!("no se pudo guardar {}", destino.display()))?;
            }
            Ok(Some(destino))
        }
        varios => {
            let Some(carpeta) = pixpin_shell::guardar::pedir_carpeta(ventana.handle()) else {
                return Ok(None);
            };
            for r in varios {
                let Some(nombre) = r.file_name() else {
                    continue;
                };
                std::fs::copy(r, carpeta.join(nombre))
                    .with_context(|| format!("no se pudo guardar {}", r.display()))?;
            }
            let _ = textos;
            Ok(Some(carpeta))
        }
    }
}

// ---------------------------------------------------------------------------
// Pintar

/// **El interruptor del formato** (`Compartible.Interruptor` del movil): la
/// pastilla con su bola, azul puesta y gris quitada, y su nombre con lo que
/// hace al lado. Todo el renglon se pulsa.
fn pintar_interruptor(h: &Hoja, p: &Pintor, d: &Disposicion, sobre: bool) {
    let (Some(caja), Some(puesto)) = (d.interruptor, h.e.interruptor(&h.c)) else {
        return;
    };
    let Some(i) = h.e.formato(&h.c).and_then(|f| f.interruptor.as_ref()) else {
        return;
    };
    let k = h.escala;
    let (ancho, alto) = (34.0 * k, 18.0 * k);
    let pastilla = RectF {
        x: caja.x,
        y: caja.y + (caja.alto - alto) / 2.0,
        ancho,
        alto,
    };
    let fondo = if puesto {
        ENCENDIDO
    } else if sobre {
        ENCIMA
    } else {
        REDONDEL
    };
    p.rellenar_redondeado(pastilla, alto / 2.0, fondo);
    let radio = alto / 2.0 - 3.0 * k;
    let cx = if puesto {
        pastilla.x + ancho - alto / 2.0
    } else {
        pastilla.x + alto / 2.0
    };
    p.circulo(
        (cx, pastilla.y + alto / 2.0),
        radio,
        if puesto { TEXTO } else { APAGADO },
    );
    let x = pastilla.x + ancho + 10.0 * k;
    let resto = (caja.x + caja.ancho - x).max(0.0);
    p.texto_linea(
        &i.nombre,
        x,
        caja.y + caja.alto / 2.0 - 9.0 * k,
        14.0 * k,
        resto,
        TEXTO,
    );
}

fn rf(c: Caja) -> RectF {
    RectF {
        x: c.x,
        y: c.y,
        ancho: c.ancho,
        alto: c.alto,
    }
}

fn con_alfa(c: Color, a: f32) -> Color {
    Color { a: c.a * a, ..c }
}

/// El icono de cada formato: los del movil donde hay uno igual en la
/// coleccion de iconos, y el mas parecido donde no.
fn icono_de(id: &str) -> &'static Icono {
    match id {
        super::WEB => &mi::PUBLIC,
        super::PDF => &mi::DESCRIPTION,
        super::PNG | super::JPG => &mi::IMAGE,
        super::SVG => &mi::DRAW,
        "pixpin" => &mi::FOLDER,
        "excalidraw" => &mi::EDIT,
        "texto" => &mi::LIST,
        "csv" => &mi::TABLE_CHART,
        // Una nota como Word: un documento, como el PDF.
        "word" => &mi::DESCRIPTION,
        _ => &mi::ATTACH_FILE,
    }
}

/// Un texto centrado en una caja a lo ancho, cortado si no cabe.
fn centrado(p: &Pintor, texto: &str, caja: RectF, tam: f32, color: Color) {
    let (w, _) = p.medir_texto(texto, tam);
    let x = caja.x + ((caja.ancho - w) / 2.0).max(0.0);
    p.texto_linea(texto, x, caja.y, tam, caja.ancho, color);
}

fn pintar(h: &Hoja, p: &Pintor, textos: &Catalogo, coma: char) {
    let k = h.escala;
    let d: Disposicion = hc::disponer(&h.c, &h.e, k);
    let encima = hc::destino_en(&d, h.raton.0, h.raton.1);
    p.limpiar_transparente();
    p.rellenar_redondeado(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: d.ancho,
            alto: d.alto,
        },
        14.0 * k,
        FONDO,
    );

    // La cabecera: que se comparte, y cerrar.
    p.texto_linea(
        &h.c.titulo,
        d.titulo.x,
        d.titulo.y + 18.0 * k,
        16.0 * k,
        d.titulo.ancho,
        TEXTO,
    );
    if encima == Destino::Cerrar {
        p.circulo(
            (
                d.cerrar.x + d.cerrar.ancho / 2.0,
                d.cerrar.y + d.cerrar.alto / 2.0,
            ),
            d.cerrar.ancho / 2.0,
            ENCIMA,
        );
    }
    let m = 6.0 * k;
    p.icono(
        &mi::CLOSE,
        RectF {
            x: d.cerrar.x + m,
            y: d.cerrar.y + m,
            ancho: d.cerrar.ancho - 2.0 * m,
            alto: d.cerrar.alto - 2.0 * m,
        },
        APAGADO,
    );

    // Los formatos.
    for (i, redondel, nombre) in &d.formatos {
        let f = &h.c.formatos[*i];
        let puesto = *i == h.e.formato;
        let centro = (
            redondel.x + redondel.ancho / 2.0,
            redondel.y + redondel.alto / 2.0,
        );
        let fondo = if puesto {
            ENCENDIDO
        } else if encima == Destino::Formato(*i) {
            ENCIMA
        } else {
            REDONDEL
        };
        p.circulo(centro, redondel.ancho / 2.0, fondo);
        let lado = 26.0 * k;
        p.icono(
            icono_de(&f.id),
            RectF {
                x: centro.0 - lado / 2.0,
                y: centro.1 - lado / 2.0,
                ancho: lado,
                alto: lado,
            },
            TEXTO,
        );
        centrado(
            p,
            &f.nombre,
            rf(*nombre),
            12.0 * k,
            if puesto { TEXTO } else { APAGADO },
        );
    }

    // Que paginas.
    if let Some(franja) = d.paginas {
        p.rellenar(
            RectF {
                x: hc::MARGEN * k,
                y: franja.y,
                ancho: d.ancho - 2.0 * hc::MARGEN * k,
                alto: 1.0,
            },
            ENCIMA,
        );
        let cuantas = h.e.formato(&h.c).map(|f| f.cuantas);
        let rotulo = match cuantas {
            Some(Cuantas::Varias) => {
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("marcadas", h.e.elegidas(&h.c).len());
                args.set("total", h.c.paginas.len());
                textos.t_args("compartir-paginas-de", &args)
            }
            Some(Cuantas::Una) => textos.t("compartir-una-pagina"),
            _ => textos.t("compartir-va-entero"),
        };
        p.texto_linea(
            &rotulo,
            d.rotulo_paginas.x,
            d.rotulo_paginas.y + 12.0 * k,
            13.0 * k,
            d.rotulo_paginas.ancho * 0.6,
            APAGADO,
        );
        for (caja, destino, clave) in [
            (d.todas, Destino::Todas, "compartir-todas"),
            (d.ninguna, Destino::Ninguna, "compartir-ninguna"),
        ] {
            let Some(caja) = caja else { continue };
            if encima == destino {
                p.rellenar_redondeado(rf(caja), 6.0 * k, ENCIMA);
            }
            centrado(
                p,
                &textos.t(clave),
                RectF {
                    y: caja.y + 8.0 * k,
                    ..rf(caja)
                },
                12.0 * k,
                ENCENDIDO,
            );
        }
        for (clave, fila) in &d.filas {
            let Some(pg) = h.c.paginas.iter().find(|x| &x.clave == clave) else {
                continue;
            };
            if encima == Destino::Pagina(clave.clone()) {
                p.rellenar(rf(*fila), ENCIMA);
            }
            let marcada = h.e.marcadas.contains(clave);
            let x = (hc::MARGEN + 20.0 * pg.nivel as f32) * k;
            let lado = 20.0 * k;
            let cy = fila.y + fila.alto / 2.0;
            if cuantas == Some(Cuantas::Una) {
                p.anillo(
                    (x + lado / 2.0, cy),
                    lado / 2.0 - 1.5 * k,
                    2.0 * k,
                    if marcada { ENCENDIDO } else { APAGADO },
                );
                if marcada {
                    p.circulo((x + lado / 2.0, cy), lado / 4.0, ENCENDIDO);
                }
            } else {
                p.icono(
                    if marcada {
                        &mi::CHECK_BOX
                    } else {
                        &mi::CHECK_BOX_OUTLINE_BLANK
                    },
                    RectF {
                        x,
                        y: cy - lado / 2.0,
                        ancho: lado,
                        alto: lado,
                    },
                    if marcada { ENCENDIDO } else { APAGADO },
                );
            }
            let tx = x + lado + 10.0 * k;
            let resto = d.ancho - tx - hc::MARGEN * k;
            if pg.detalle.is_empty() {
                p.texto_linea(&pg.nombre, tx, cy - 9.0 * k, 14.0 * k, resto, TEXTO);
            } else {
                p.texto_linea(&pg.nombre, tx, fila.y + 3.0 * k, 14.0 * k, resto, TEXTO);
                p.texto_linea(&pg.detalle, tx, fila.y + 21.0 * k, 11.0 * k, resto, APAGADO);
            }
        }
        // Hay mas filas de las que se ven: una pista de que la rueda corre.
        if h.c.paginas.len() > hc::FILAS_VISIBLES && cuantas != Some(Cuantas::Ninguna) {
            let total = h.c.paginas.len() as f32;
            let alto_lista = hc::ALTO_FILA * hc::FILAS_VISIBLES as f32 * k;
            let y0 = franja.y + hc::ALTO_TITULO_PAGINAS * k;
            let barra = alto_lista * hc::FILAS_VISIBLES as f32 / total;
            let y = y0 + alto_lista * h.e.primera_fila as f32 / total;
            p.rellenar_redondeado(
                RectF {
                    x: d.ancho - 6.0 * k,
                    y,
                    ancho: 3.0 * k,
                    alto: barra,
                },
                1.5 * k,
                APAGADO,
            );
        }
    }

    // El pie: cuanto pesa y las salidas.
    let pie = match &h.aviso {
        Some((t, _)) => t.clone(),
        None => match encima {
            Destino::Salida(Boton::Copiar) => textos.t("compartir-copiar"),
            Destino::Salida(Boton::Wifi) => textos.t("compartir-wifi"),
            Destino::Salida(Boton::Arrastrar) => textos.t("compartir-arrastrar"),
            // Encima del interruptor, que hace.
            Destino::Interruptor => {
                h.e.formato(&h.c)
                    .and_then(|f| f.interruptor.as_ref())
                    .map(|i| i.detalle.clone())
                    .unwrap_or_default()
            }
            _ => match h.pie() {
                Pie::Preparando => textos.t("compartir-preparando"),
                Pie::MarcaAlguna => textos.t("compartir-marca-alguna"),
                Pie::Fallo => textos.t("compartir-no-se-pudo"),
                Pie::Listo(bytes, n) => {
                    let peso = hc::peso_legible(bytes, coma);
                    if n > 1 {
                        let mut args = fluent_bundle::FluentArgs::new();
                        args.set("peso", peso);
                        args.set("n", n);
                        textos.t_args("compartir-peso-ficheros", &args)
                    } else {
                        peso
                    }
                }
            },
        },
    };
    pintar_interruptor(h, p, &d, encima == Destino::Interruptor);
    p.texto_linea(
        &pie,
        d.peso.x,
        d.peso.y + d.peso.alto / 2.0 - 9.0 * k,
        14.0 * k,
        d.peso.ancho,
        TEXTO,
    );
    let listo = matches!(h.pie(), Pie::Listo(..));
    for (s, caja) in &d.salidas {
        let r = rf(*caja);
        let sobre = encima == Destino::Salida(*s);
        let alfa = if listo || matches!(h.pie(), Pie::Preparando) {
            1.0
        } else {
            0.4
        };
        match s {
            Boton::Compartir => {
                p.rellenar_redondeado(
                    r,
                    8.0 * k,
                    con_alfa(if sobre { hex(0x3196ec) } else { ENCENDIDO }, alfa),
                );
                let lado = 18.0 * k;
                p.icono(
                    &mi::IOS_SHARE,
                    RectF {
                        x: r.x + 12.0 * k,
                        y: r.y + (r.alto - lado) / 2.0,
                        ancho: lado,
                        alto: lado,
                    },
                    TEXTO,
                );
                p.texto_linea(
                    &textos.t("compartir-boton"),
                    r.x + 36.0 * k,
                    r.y + r.alto / 2.0 - 9.0 * k,
                    14.0 * k,
                    r.ancho - 40.0 * k,
                    TEXTO,
                );
            }
            Boton::Guardar => {
                p.rellenar_redondeado(
                    r,
                    8.0 * k,
                    con_alfa(if sobre { ENCIMA } else { REDONDEL }, alfa),
                );
                centrado(
                    p,
                    &textos.t("compartir-guardar"),
                    RectF {
                        y: r.y + r.alto / 2.0 - 9.0 * k,
                        ..r
                    },
                    14.0 * k,
                    con_alfa(TEXTO, alfa),
                );
            }
            Boton::Copiar | Boton::Wifi => {
                p.rellenar_redondeado(
                    r,
                    8.0 * k,
                    con_alfa(if sobre { ENCIMA } else { REDONDEL }, alfa),
                );
                let lado = 20.0 * k;
                p.icono(
                    if *s == Boton::Copiar {
                        &mi::CONTENT_COPY
                    } else {
                        &mi::WIFI
                    },
                    RectF {
                        x: r.x + (r.ancho - lado) / 2.0,
                        y: r.y + (r.alto - lado) / 2.0,
                        ancho: lado,
                        alto: lado,
                    },
                    con_alfa(TEXTO, alfa),
                );
            }
            // Los puntitos de agarre, los de las tarjetas de la Salida: «esto
            // se arrastra».
            Boton::Arrastrar => {
                p.rellenar_redondeado(
                    r,
                    8.0 * k,
                    con_alfa(if sobre { ENCIMA } else { REDONDEL }, alfa),
                );
                crate::salida::agarre(
                    p,
                    r.x + r.ancho / 2.0,
                    r.y + r.alto / 2.0,
                    con_alfa(TEXTO, alfa),
                    k * 1.2,
                );
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn hoja_de(c: Compartible, escala: f32) -> Hoja {
        let mut e = Estado::nuevo(&c);
        e.elegir_formato(&c, 0);
        Hoja {
            c,
            e,
            escala,
            raton: (-1.0, -1.0),
            hechos: HashMap::new(),
            pedido: None,
            pendiente: None,
            aviso: None,
            esperando_panel: false,
        }
    }

    /// **La muestra de la hoja**: pintada fuera de pantalla con el mismo
    /// `pintar` de la ventana, para mirarla sin abrir la aplicacion. Con
    /// `PIXPIN_MUESTRAS_COMPARTIR` se queda en esa carpeta.
    #[test]
    fn la_hoja_se_pinta_entera_y_dice_cuanto_pesa() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let p = super::super::preparar(
            Cosa::Lienzo(super::super::LienzoSuelto {
                escena: {
                    let mut e = pixpin_motor2d::Escena::nueva();
                    for (i, n) in ["Planta", "Alzado", "Seccion"].iter().enumerate() {
                        e.anadir(pixpin_motor2d::Elemento {
                            figura: pixpin_motor2d::Figura::Marco {
                                nombre: (*n).into(),
                            },
                            y: i as f32 * 300.0,
                            ancho: 200.0,
                            alto: 200.0,
                            ..Default::default()
                        });
                        e.anadir(pixpin_motor2d::Elemento {
                            figura: pixpin_motor2d::Figura::Rectangulo,
                            x: 20.0,
                            y: i as f32 * 300.0 + 20.0,
                            ancho: 50.0,
                            alto: 50.0,
                            ..Default::default()
                        });
                    }
                    e
                },
                papel: None,
                fotos: HashMap::new(),
                nombre: "Casa de muestra".into(),
            }),
            &textos,
        )
        .unwrap();
        let c = super::super::compartible(&p, &textos);
        let escala = 1.25;
        let mut h = hoja_de(c, escala);
        // Ya hecho: el pie dice el peso.
        let clave = h.clave().expect("hay algo que hacer");
        assert_eq!(clave.0, super::super::WEB);
        h.hechos.insert(
            clave,
            Ok(Arc::new(Salida {
                ficheros: vec![PathBuf::from("x.html")],
                imagen: None,
                bytes: 1_300_000,
            })),
        );
        assert!(matches!(h.pie(), Pie::Listo(1_300_000, 1)));
        let (w, alto) = hc::tamano(&h.c, escala);
        let (w, alto) = (w.ceil() as u32, alto.ceil() as u32);
        let d = pixpin_capture::Dispositivo::nuevo().unwrap();
        let motor = pixpin_render::MotorRender::nuevo(d.d3d()).unwrap();
        let destino =
            pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), w, alto)
                .unwrap();
        motor
            .dibujar(&destino.destino, |p| pintar(&h, p, &textos, ','))
            .unwrap();
        let (ancho, alto, pixeles) = destino.leer_rgba().unwrap();
        let img = pixpin_codec::ImagenRgba {
            ancho,
            alto,
            pixeles,
        };
        // Algo pintado en el centro de la tarjeta, y las esquinas redondeadas
        // transparentes.
        assert_eq!(img.pixeles[3], 0, "la esquina, fuera de la tarjeta");
        let centro = ((alto / 2 * ancho + ancho / 2) * 4 + 3) as usize;
        assert!(img.pixeles[centro] > 200, "la tarjeta es opaca");
        let dir = std::env::var_os("PIXPIN_MUESTRAS_COMPARTIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        std::fs::write(
            dir.join("hoja-de-compartir.png"),
            pixpin_codec::codificar_png(&img).unwrap(),
        )
        .unwrap();

        // Caso negativo: sin paginas marcadas no hay nada que preparar y el
        // pie lo dice.
        h.e.ninguna();
        assert!(h.clave().is_none());
        assert!(matches!(h.pie(), Pie::MarcaAlguna));
    }
}
