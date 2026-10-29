//! **Repasar en voz alta** (29-oct-2026, la idea de Jev probada sin
//! interfaz en `docs/probar-ideas-sin-interfaz.md`, prototipo
//! `repaso_oral`): desde «Estudiar la clase», el alumno explica la clase de
//! viva voz y ve, punto clave a punto clave, cual cubre ✓, cual dice mal ✗,
//! cual falta · y cual no se entiende ?. Lo dicho mal y lo que falta llevan
//! su minuto: un clic vuelve a la clase ahi.
//!
//! **Los puntos clave** salen de la clase (`puntos_clave`): lo que Jev
//! marco como «entra en el examen» y como decision (`resto["importantes"]`,
//! con el renglon de ese minuto), los marcadores con nota y los temas de la
//! reunion tratados; doce como mucho (los de examen y los marcadores
//! primero), en el orden de la clase.
//!
//! **Lo que se dice** entra por el dictado de Windows (`pixpin_voz::Dictado`,
//! el de la caja del chat): cada frase cerrada se suma a lo explicado y va a
//! `pixpin_typesafe::repaso::preguntar` (lo explicado entero y dos Nouls por
//! punto), y la tabla `Repaso::decidir` mueve los estados: lo dicho mal se
//! queda, lo cubierto no vuelve a faltar. Una pregunta en vuelo como mucho;
//! lo que se dice mientras tanto va en la siguiente.
//!
//! Sin clave o con `[jev] repaso` apagado el boton no sale. Si Jev falla, se
//! dice y los puntos se quedan como estaban (un fallo no es un «falta»).
//!
//! Teclado: espacio o Intro empieza y para de hablar, Escape cierra.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_geom::Rect;
use pixpin_proyecto::clase::{self, Importante, TipoImportante};
use pixpin_proyecto::temas as agenda;
use pixpin_proyecto::{almacen, cuaderno};
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Ubicacion};
use pixpin_typesafe::preguntas::repaso::POR_PETICION;
use pixpin_typesafe::repaso::{self, Estado as EstadoPunto, Repaso, SenalPunto};

use crate::overlay::Recursos;
use crate::ventanita::{
    APAGADO, AZUL, Botones, CRISTAL, DORADO, FONDO, ROJO, TEXTO, VERDE, centrado, centrado_en,
};

/// Lo que se espera a Jev por peticion.
const TIEMPO: Duration = Duration::from_secs(6);
/// Lo largo de un punto en pantalla y en la pregunta.
const LARGO_PUNTO: usize = 200;
/// Lo ultimo de lo explicado que se ensena abajo.
const LARGO_DICHO: usize = 220;

/// La ventana abierta (su HWND), -1 mientras nace, 0 sin ventana.
static ABIERTA: AtomicIsize = AtomicIsize::new(0);

/// **Si sale el boton** «Repasar en voz alta»: con la clave de TypeSafe y
/// `[jev] repaso` encendido. Se pinta en cada fotograma: los ajustes se
/// releen cada dos segundos como mucho.
pub(super) fn disponible(raiz: &Path) -> bool {
    static VISTO: Mutex<Option<(PathBuf, Instant, bool)>> = Mutex::new(None);
    if !crate::jev::hay_clave() {
        return false;
    }
    let mut g = VISTO.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((r, cuando, si)) = g.as_ref()
        && r == raiz
        && cuando.elapsed() < Duration::from_secs(2)
    {
        return *si;
    }
    let si = crate::jev::Funcion::Repaso.encendida_en(
        &pixpin_store::cargar(&ubicacion(raiz))
            .unwrap_or_default()
            .jev,
    );
    *g = Some((raiz.to_path_buf(), Instant::now(), si));
    si
}

/// **El boton de «Estudiar la clase»**, en la fila del titulo del texto y
/// por la derecha, si esta [`disponible`] y cabe tras el titulo (que acaba
/// en `fin_titulo`). Devuelve donde quedo, para apuntar su zona.
#[allow(clippy::too_many_arguments)]
pub(super) fn pintar_boton(
    p: &Pintor,
    e: f32,
    tema: &super::Tema,
    raton: (f32, f32),
    fila: RectF,
    fin_titulo: f32,
    textos: &Catalogo,
    raiz: &Path,
) -> Option<RectF> {
    if !disponible(raiz) {
        return None;
    }
    let tam = 12.0 * e;
    let alto = 22.0 * e;
    let rotulo = textos.t("estudiar-repasar");
    let (w, h) = p.medir_texto(&rotulo, tam);
    let boton = RectF {
        x: fila.x + fila.ancho - w - 20.0 * e - 10.0 * e,
        y: fila.y + (fila.alto - alto) / 2.0,
        ancho: w + 20.0 * e,
        alto,
    };
    if boton.x <= fin_titulo + 8.0 * e {
        return None;
    }
    let encima = crate::ventanita::dentro(boton, raton);
    p.rellenar_redondeado(
        boton,
        alto / 2.0,
        super::con_alfa(tema.enviar, if encima { 0.32 } else { 0.2 }),
    );
    p.texto(
        &rotulo,
        boton.x + 10.0 * e,
        boton.y + (alto - h) / 2.0,
        tam,
        tema.texto,
    );
    Some(boton)
}

fn ubicacion(raiz: &Path) -> Ubicacion {
    Ubicacion::Instalado {
        raiz: raiz.to_path_buf(),
    }
}

/// Un punto clave: lo que dice y el minuto de la clase donde se explico.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PuntoClave {
    pub texto: String,
    pub ms: Option<i64>,
}

/// **Los puntos clave de una clase** (ver la cabecera). `lineas`: los
/// trozos del texto con su minuto; `marcadores`: (ms, nota).
pub(super) fn puntos_clave(
    lineas: &[(Option<i64>, String)],
    importantes: &[Importante],
    marcadores: &[(i64, String)],
    temas: &[agenda::Tema],
) -> Vec<PuntoClave> {
    let tiempos: Vec<Option<i64>> = lineas.iter().map(|(ms, _)| *ms).collect();
    let renglon = |ms: i64| {
        clase::linea_en(&tiempos, ms)
            .and_then(|i| lineas.get(i))
            .map(|(_, t)| crate::voz::sin_marcas(t).trim().to_string())
            .filter(|t| !t.is_empty())
    };
    // (prioridad, punto): menos es antes.
    let mut v: Vec<(u8, PuntoClave)> = Vec::new();
    for i in importantes {
        let prioridad = match i.tipo {
            TipoImportante::Examen => 0,
            TipoImportante::Decision => 2,
            TipoImportante::Tarea | TipoImportante::Pregunta => continue,
        };
        if let Some(texto) = renglon(i.ms) {
            v.push((
                prioridad,
                PuntoClave {
                    texto,
                    ms: Some(i.ms),
                },
            ));
        }
    }
    for (ms, nota) in marcadores {
        let nota = nota.trim();
        if !nota.is_empty() {
            v.push((
                1,
                PuntoClave {
                    texto: nota.to_string(),
                    ms: Some(*ms),
                },
            ));
        }
    }
    for t in temas
        .iter()
        .filter(|t| t.hecho && !t.texto.trim().is_empty())
    {
        v.push((
            3,
            PuntoClave {
                texto: t.texto.trim().to_string(),
                ms: t.ms,
            },
        ));
    }
    // Sin repetir: el mismo renglon puede ser examen y decision.
    let mut vistos = std::collections::HashSet::new();
    v.sort_by_key(|(p, x)| (*p, x.ms));
    v.retain(|(_, x)| vistos.insert(x.texto.to_lowercase()));
    v.truncate(POR_PETICION);
    let mut puntos: Vec<PuntoClave> = v
        .into_iter()
        .map(|(_, mut x)| {
            x.texto = recortar(&x.texto, LARGO_PUNTO);
            x
        })
        .collect();
    puntos.sort_by_key(|x| x.ms.unwrap_or(i64::MAX));
    puntos
}

/// Recorta un texto a `n` letras, con «…».
fn recortar(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut r: String = s.chars().take(n).collect();
    r.push('…');
    r
}

/// Que repasar: la clase y sus puntos.
struct Pedido {
    raiz: PathBuf,
    proyecto: String,
    /// La nota de voz de la clase.
    id: String,
    nombre: String,
    puntos: Vec<PuntoClave>,
}

/// Lee la clase del cuaderno: su nombre y sus puntos clave.
fn leer(raiz: &Path, proyecto: &str, id: &str) -> Result<(String, Vec<PuntoClave>)> {
    let m = cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, proyecto))
        .context("sin cuaderno")?
        .mensajes
        .into_iter()
        .find(|m| m.id == id)
        .context("la clase ya no esta en el chat")?;
    let lineas: Vec<(Option<i64>, String)> = super::trozos_de_la_letra(&m)
        .into_iter()
        .map(|t| (t.ms, t.texto))
        .collect();
    let marcadores: Vec<(i64, String)> = clase::marcadores_de(&m.resto)
        .into_iter()
        .map(|x| (x.ms, x.texto))
        .collect();
    let puntos = puntos_clave(
        &lineas,
        &clase::importantes_de(&m.resto).unwrap_or_default(),
        &marcadores,
        &agenda::temas_de(&m.resto),
    );
    let nombre = if m.nombre.trim().is_empty() {
        m.texto.clone()
    } else {
        m.nombre.trim_end_matches(".m4a").to_string()
    };
    Ok((nombre, puntos))
}

/// **Abre el repaso de la clase** `id` (su nota de voz) en su ventana, o
/// trae delante la que ya este abierta.
pub(super) fn abrir(raiz: &Path, proyecto: &str, id: &str) {
    let ya = ABIERTA.load(Ordering::SeqCst);
    if ya > 0 {
        VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(ya as *mut _));
        return;
    }
    if ya != 0 {
        return;
    }
    let (nombre, puntos) = match leer(raiz, proyecto, id) {
        Ok(x) => x,
        Err(e) => {
            tracing::warn!(?e, "no se pudo leer la clase para repasarla");
            return;
        }
    };
    let pedido = Pedido {
        raiz: raiz.to_path_buf(),
        proyecto: proyecto.to_string(),
        id: id.to_string(),
        nombre,
        puntos,
    };
    ABIERTA.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new()
        .name("repaso-oral".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let hecho =
                crate::dispositivo_perdido::con_recursos("repaso oral", |r| ventana(r, &pedido));
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir el repaso oral");
            }
            ABIERTA.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del repaso oral");
        ABIERTA.store(0, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Cerrar,
    /// Empezar o parar de hablar.
    Hablar,
    /// Empezar otra vez: lo explicado y los estados, de cero.
    OtraVez,
    /// Volver a la clase en el minuto de ese punto.
    Ir(usize),
}

/// Lo que se le pregunto a Jev y aun no contesto.
type EnVuelo = Receiver<Result<Vec<(String, SenalPunto)>, String>>;

struct Estado {
    repaso: Repaso,
    ms: Vec<Option<i64>>,
    dictado: Option<pixpin_voz::Dictado>,
    oyendo: bool,
    provisional: String,
    en_vuelo: Option<EnVuelo>,
    /// Se dijo algo mientras habia una en vuelo: otra al acabar.
    pendiente: bool,
    botones: Botones<Accion>,
    /// Un aviso abajo y hasta cuando (ms del reloj).
    aviso: Option<(String, u64)>,
}

impl Estado {
    fn nuevo(p: &Pedido) -> Estado {
        let textos: Vec<String> = p.puntos.iter().map(|x| x.texto.clone()).collect();
        Estado {
            repaso: Repaso::nuevo(&p.nombre, &textos),
            ms: p.puntos.iter().map(|x| x.ms).collect(),
            dictado: None,
            oyendo: false,
            provisional: String::new(),
            en_vuelo: None,
            pendiente: false,
            botones: Botones::default(),
            aviso: None,
        }
    }

    /// Pregunta por lo explicado hasta ahora, si no hay otra en vuelo.
    fn preguntar(&mut self, cliente: &pixpin_typesafe::Cliente) {
        if self.en_vuelo.is_some() {
            self.pendiente = true;
            return;
        }
        let puntos = self.repaso.por_preguntar();
        if puntos.is_empty() || self.repaso.explicacion.trim().is_empty() {
            return;
        }
        self.pendiente = false;
        let (clase, explicado) = (self.repaso.clase.clone(), self.repaso.explicacion.clone());
        let cliente = cliente.clone();
        let (enviar, recibir) = channel();
        let lanzado = std::thread::Builder::new()
            .name("jev-repaso".into())
            .spawn(move || {
                let r = repaso::preguntar(&cliente, &clase, &explicado, &puntos)
                    .map_err(|e| e.to_string());
                let _ = enviar.send(r);
            });
        if lanzado.is_ok() {
            self.en_vuelo = Some(recibir);
        }
    }
}

fn ventana(recursos: &Recursos, pedido: &Pedido) -> Result<()> {
    let u = ubicacion(&pedido.raiz);
    let idioma = crate::compartir::idioma_de(&u);
    let textos = Catalogo::nuevo(idioma);
    let textos = &textos;
    let cliente = crate::jev::para(&u, crate::jev::Funcion::Repaso)
        .context("sin Jev para el repaso")?
        .con_tiempo(TIEMPO);
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = pixpin_shell::pantalla_de::monitor_de_la_ventana_activa()
        .and_then(|r| monitores.monitores().iter().find(|m| m.area == r))
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let marco = centrado(monitor.area_trabajo, 760, 680, monitor.escala_por_cien);
    let ventana = VentanaOverlay::nueva_normal(marco, &textos.t("repaso-titulo"))
        .context("no se pudo abrir el repaso oral")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para el repaso oral")?;
    ventana.mostrar();
    ventana.enfocar();
    ABIERTA.store(ventana.handle().0 as isize, Ordering::SeqCst);

    let mut e = Estado::nuevo(pedido);
    let mut vivo = true;
    let mut pintar = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        let ahora = pixpin_shell::entorno::ahora_utc_ms() as u64;
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            pintar = true;
            let accion = match ev {
                EventoOverlay::Cerrar => Some(Accion::Cerrar),
                EventoOverlay::RatonMovido(p) => {
                    e.botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    None
                }
                EventoOverlay::BotonPulsado(p) => {
                    e.botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    e.botones.bajo_el_raton()
                }
                EventoOverlay::Tecla { vk, .. } => match vk {
                    0x1B => Some(Accion::Cerrar),
                    0x0D | 0x20 => Some(Accion::Hablar),
                    _ => None,
                },
                _ => None,
            };
            match accion {
                Some(Accion::Cerrar) => vivo = false,
                Some(Accion::Hablar) => {
                    if e.dictado.take().is_none() {
                        if crate::voz::windows_sabe(idioma) {
                            e.dictado = Some(pixpin_voz::Dictado::empezar(
                                crate::voz::idioma_de_voz(idioma),
                            ));
                            e.oyendo = false;
                        } else {
                            e.aviso = Some((textos.t("chat-voz-sin-reconocedor"), ahora + 4_000));
                        }
                    }
                    e.provisional.clear();
                }
                Some(Accion::OtraVez) => {
                    let pendiente_de_jev = e.en_vuelo.take().is_some();
                    let dictado = e.dictado.take();
                    e = Estado::nuevo(pedido);
                    e.dictado = dictado;
                    if pendiente_de_jev {
                        tracing::debug!("repaso: se tira la respuesta en vuelo");
                    }
                }
                Some(Accion::Ir(i)) => {
                    if let Some(ms) = e.ms.get(i).copied().flatten() {
                        super::estudiar_clase::abrir(
                            idioma,
                            u.clone(),
                            pedido.proyecto.clone(),
                            pedido.id.clone(),
                            ms,
                        );
                    }
                }
                None => {}
            }
        }
        if !vivo {
            break;
        }
        // Lo que se va diciendo.
        let mut dicho = false;
        if let Some(d) = &e.dictado {
            for aviso in d.recoger() {
                pintar = true;
                match aviso {
                    pixpin_voz::AvisoDeDictado::Oyendo => e.oyendo = true,
                    pixpin_voz::AvisoDeDictado::Provisional(t) => e.provisional = t,
                    pixpin_voz::AvisoDeDictado::Frase(t) => {
                        e.repaso.oir(&t);
                        e.provisional.clear();
                        dicho = true;
                    }
                    pixpin_voz::AvisoDeDictado::Fallo(razon) => {
                        let mut args = fluent_bundle::FluentArgs::new();
                        args.set("razon", razon);
                        e.aviso = Some((textos.t_args("chat-dictado-fallo", &args), ahora + 5_000));
                    }
                }
            }
            if d.terminado() {
                e.dictado = None;
                pintar = true;
            }
        }
        if dicho {
            e.preguntar(&cliente);
        }
        // Lo que dijo Jev.
        if let Some(r) = &e.en_vuelo
            && let Ok(llegado) = r.try_recv()
        {
            e.en_vuelo = None;
            pintar = true;
            match llegado {
                Ok(senales) => {
                    for (id, antes, ahora) in e.repaso.decidir(&senales) {
                        tracing::debug!(id, antes = antes.id(), ahora = ahora.id(), "repaso");
                    }
                }
                Err(err) => {
                    tracing::info!("jev: el repaso no se pregunto: {err}");
                    e.aviso = Some((textos.t("repaso-jev-fallo"), ahora + 4_000));
                }
            }
            if e.pendiente {
                e.preguntar(&cliente);
            }
        }
        if e.aviso.as_ref().is_some_and(|(_, hasta)| *hasta < ahora) {
            e.aviso = None;
            pintar = true;
        }
        if pintar {
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| {
                    pintar_todo(&mut e, p, marco, escala, textos)
                });
                let _ = superficie.presentar();
            }
            pintar = false;
        }
        let espera = if e.dictado.is_some() || e.en_vuelo.is_some() {
            100
        } else {
            500
        };
        pixpin_shell::overlay::esperar_eventos(Some(espera));
    }
    Ok(())
}

/// El simbolo y el color de cada estado.
fn marca(estado: EstadoPunto) -> (&'static str, Color) {
    match estado {
        EstadoPunto::Cubierto => ("✓", VERDE),
        EstadoPunto::Mal => ("✗", ROJO),
        EstadoPunto::Falta => ("·", APAGADO),
        EstadoPunto::Dudoso => ("?", DORADO),
    }
}

/// Lo ultimo de un texto, con «…» delante si se corto.
fn lo_ultimo(texto: &str, largo: usize) -> String {
    let letras: Vec<char> = texto.trim().chars().collect();
    if letras.len() <= largo {
        return letras.into_iter().collect();
    }
    let mut s = String::from("…");
    s.extend(&letras[letras.len() - largo..]);
    s
}

fn pintar_todo(e: &mut Estado, p: &Pintor, marco: Rect, escala: f32, textos: &Catalogo) {
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    let s = escala;
    p.limpiar(FONDO);
    e.botones.vaciar();
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: h,
    };
    let margen = 28.0 * s;
    let ancho = w - 2.0 * margen;
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("clase", e.repaso.clase.clone());
    centrado_en(
        p,
        &textos.t_args("repaso-cabecera", &args),
        todo,
        22.0 * s,
        19.0 * s,
        TEXTO,
    );
    let (_, alto_ayuda) = p.medir_texto_ajustado(&textos.t("repaso-explicacion"), 13.0 * s, ancho);
    p.parrafo(
        &textos.t("repaso-explicacion"),
        margen,
        54.0 * s,
        13.0 * s,
        ancho,
        &[],
        APAGADO,
    );
    let mut y = 54.0 * s + alto_ayuda + 18.0 * s;

    if e.repaso.puntos.is_empty() {
        p.parrafo(
            &textos.t("repaso-sin-puntos"),
            margen,
            y,
            14.0 * s,
            ancho,
            &[],
            TEXTO,
        );
    }
    // Los puntos, uno por fila: su marca, su texto y, si esta mal o falta,
    // su minuto para volver a la clase.
    let tam = 14.0 * s;
    let tope_lista = h - 190.0 * s;
    for (i, ((_, texto), estado)) in e
        .repaso
        .puntos
        .iter()
        .zip(e.repaso.estados.iter().copied())
        .enumerate()
    {
        let ms = e.ms.get(i).copied().flatten();
        let ir = ms.filter(|_| matches!(estado, EstadoPunto::Mal | EstadoPunto::Falta));
        let rotulo_ir = ir.map(|ms| format!("▶ {}", pixpin_voz::marca_de_tiempo(ms)));
        let ancho_ir = rotulo_ir
            .as_ref()
            .map_or(0.0, |r| p.medir_texto(r, 12.5 * s).0 + 20.0 * s);
        let ancho_texto = ancho - 44.0 * s - ancho_ir - 8.0 * s;
        let (_, alto_texto) = p.medir_texto_ajustado(texto, tam, ancho_texto.max(1.0));
        let alto = alto_texto.max(22.0 * s) + 14.0 * s;
        if y + alto > tope_lista {
            break;
        }
        let fila = RectF {
            x: margen,
            y,
            ancho,
            alto,
        };
        p.rellenar_redondeado(fila, 10.0 * s, CRISTAL);
        let (simbolo, color) = marca(estado);
        let centro = (fila.x + 22.0 * s, fila.y + alto / 2.0);
        p.circulo(centro, 11.0 * s, color);
        let (sw, sh) = p.medir_texto(simbolo, 14.0 * s);
        p.texto(
            simbolo,
            centro.0 - sw / 2.0,
            centro.1 - sh / 2.0,
            14.0 * s,
            FONDO,
        );
        p.parrafo(
            texto,
            fila.x + 44.0 * s,
            fila.y + (alto - alto_texto) / 2.0,
            tam,
            ancho_texto.max(1.0),
            &[],
            if estado == EstadoPunto::Falta {
                APAGADO
            } else {
                TEXTO
            },
        );
        if let Some(r) = rotulo_ir {
            let caja = RectF {
                x: fila.x + fila.ancho - ancho_ir - 8.0 * s,
                y: fila.y + (alto - 28.0 * s) / 2.0,
                ancho: ancho_ir,
                alto: 28.0 * s,
            };
            e.botones.boton(p, caja, Accion::Ir(i), &r, None, s);
        }
        y += alto + 8.0 * s;
    }

    // Abajo: el recuento, lo ultimo dicho y los botones.
    let cuenta = |x: EstadoPunto| e.repaso.estados.iter().filter(|s| **s == x).count() as i64;
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("cubiertos", cuenta(EstadoPunto::Cubierto));
    args.set("total", e.repaso.puntos.len() as i64);
    args.set("mal", cuenta(EstadoPunto::Mal));
    args.set("faltan", cuenta(EstadoPunto::Falta));
    args.set("dudosos", cuenta(EstadoPunto::Dudoso));
    if !e.repaso.puntos.is_empty() {
        centrado_en(
            p,
            &textos.t_args("repaso-recuento", &args),
            todo,
            h - 172.0 * s,
            13.0 * s,
            APAGADO,
        );
    }
    let mut dicho = lo_ultimo(&e.repaso.explicacion, LARGO_DICHO);
    if !e.provisional.is_empty() {
        if !dicho.is_empty() {
            dicho.push(' ');
        }
        dicho.push_str(&e.provisional);
    }
    let (dicho, color) = if !dicho.is_empty() {
        (dicho, TEXTO)
    } else if e.dictado.is_some() && !e.oyendo {
        (textos.t("repaso-preparando"), APAGADO)
    } else if e.dictado.is_some() {
        (textos.t("repaso-te-escucho"), APAGADO)
    } else {
        (textos.t("repaso-pulsa-hablar"), APAGADO)
    };
    let caja_dicho = RectF {
        x: margen,
        y: h - 146.0 * s,
        ancho,
        alto: 62.0 * s,
    };
    p.rellenar_redondeado(caja_dicho, 10.0 * s, CRISTAL);
    p.parrafo(
        &dicho,
        caja_dicho.x + 12.0 * s,
        caja_dicho.y + 8.0 * s,
        12.5 * s,
        caja_dicho.ancho - 24.0 * s,
        &[],
        color,
    );
    let bw = 200.0 * s;
    let bh = 40.0 * s;
    let by = h - 66.0 * s;
    let hablando = e.dictado.is_some();
    if !e.repaso.puntos.is_empty() {
        e.botones.boton(
            p,
            RectF {
                x: w / 2.0 - bw - 8.0 * s,
                y: by,
                ancho: bw,
                alto: bh,
            },
            Accion::Hablar,
            &textos.t(if hablando {
                "repaso-parar"
            } else {
                "repaso-hablar"
            }),
            Some(if hablando { ROJO } else { AZUL }),
            s,
        );
    }
    e.botones.boton(
        p,
        RectF {
            x: if e.repaso.puntos.is_empty() {
                w / 2.0 - bw / 2.0
            } else {
                w / 2.0 + 8.0 * s
            },
            y: by,
            ancho: bw,
            alto: bh,
        },
        if e.repaso.puntos.is_empty() || e.repaso.explicacion.is_empty() {
            Accion::Cerrar
        } else {
            Accion::OtraVez
        },
        &textos.t(
            if e.repaso.puntos.is_empty() || e.repaso.explicacion.is_empty() {
                "repaso-cerrar"
            } else {
                "repaso-otra-vez"
            },
        ),
        None,
        s,
    );
    if let Some((t, _)) = &e.aviso {
        centrado_en(p, t, todo, h - 20.0 * s, 12.5 * s, DORADO);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_store::Idioma;

    fn tema(texto: &str, hecho: bool, ms: Option<i64>) -> agenda::Tema {
        agenda::Tema {
            id: texto.into(),
            texto: texto.into(),
            hecho,
            ms,
            p: None,
            por_jev: false,
            a_mano: false,
        }
    }

    fn importante(ms: i64, tipo: TipoImportante) -> Importante {
        Importante {
            ms,
            tipo,
            p: 0.9,
            quien: None,
            cuando: None,
            en_tareas: false,
        }
    }

    #[test]
    fn los_puntos_clave_salen_de_lo_importante_los_marcadores_y_los_temas() {
        let lineas = vec![
            (Some(0), "Hoy vemos la fotosíntesis.".to_string()),
            (
                Some(60_000),
                "Esto entra: la planta produce oxígeno.".into(),
            ),
            (Some(120_000), "Necesita luz para hacerla.".into()),
            (Some(180_000), "Traed el cuaderno el lunes.".into()),
        ];
        let importantes = vec![
            importante(60_000, TipoImportante::Examen),
            importante(120_000, TipoImportante::Decision),
            // Las tareas y las preguntas no son puntos clave.
            importante(180_000, TipoImportante::Tarea),
            // El mismo renglon dos veces: una.
            importante(60_000, TipoImportante::Decision),
        ];
        let marcadores = vec![
            (90_000, "El agua entra por la raíz".into()),
            (5, " ".into()),
        ];
        let temas = vec![
            tema("La clorofila", true, Some(150_000)),
            tema("Sin tratar", false, None),
        ];
        let v = puntos_clave(&lineas, &importantes, &marcadores, &temas);
        let textos: Vec<&str> = v.iter().map(|x| x.texto.as_str()).collect();
        assert_eq!(
            textos,
            vec![
                "Esto entra: la planta produce oxígeno.",
                "El agua entra por la raíz",
                "Necesita luz para hacerla.",
                "La clorofila",
            ]
        );
        assert_eq!(v[1].ms, Some(90_000));
    }

    #[test]
    fn como_mucho_los_que_caben_en_una_peticion_y_primero_el_examen() {
        let lineas: Vec<(Option<i64>, String)> = (0..20)
            .map(|i| (Some(i * 1000), format!("renglon {i}")))
            .collect();
        let mut importantes: Vec<Importante> = (0..15)
            .map(|i| importante(i * 1000, TipoImportante::Decision))
            .collect();
        importantes.push(importante(19_000, TipoImportante::Examen));
        let v = puntos_clave(&lineas, &importantes, &[], &[]);
        assert_eq!(v.len(), POR_PETICION);
        assert!(v.iter().any(|x| x.texto == "renglon 19"));
    }

    #[test]
    fn lo_ultimo_dicho_se_corta_por_delante() {
        assert_eq!(lo_ultimo("  hola  ", 10), "hola");
        assert_eq!(lo_ultimo("abcdef", 3), "…def");
    }

    #[test]
    fn los_rotulos_del_repaso_estan_en_los_dos_idiomas() {
        for idioma in [Idioma::Espanol, Idioma::Ingles] {
            let c = Catalogo::nuevo(idioma);
            for k in [
                "repaso-titulo",
                "repaso-cabecera",
                "repaso-explicacion",
                "repaso-sin-puntos",
                "repaso-recuento",
                "repaso-preparando",
                "repaso-te-escucho",
                "repaso-pulsa-hablar",
                "repaso-hablar",
                "repaso-parar",
                "repaso-otra-vez",
                "repaso-cerrar",
                "repaso-jev-fallo",
                "estudiar-repasar",
                "chat-anotar-en",
                "chat-anotar-donde",
                "chat-anotar-tecla",
                "chat-anotado-en",
                "ajustes2-jev-dictado",
                "ajustes2-ayuda-jev-dictado",
                "ajustes2-jev-repaso",
                "ajustes2-ayuda-jev-repaso",
            ] {
                assert_ne!(c.t(k), k, "falta {k} en {idioma:?}");
            }
        }
    }
}
