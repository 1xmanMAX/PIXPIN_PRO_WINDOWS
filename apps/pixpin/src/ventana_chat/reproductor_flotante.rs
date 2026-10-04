//! **El reproductor flotante** (pedido «reproducir», el usuario, 3-oct):
//! «cuando le doy a enter a un audio este se reproduzca llevando la ui de
//! reproduccion de audio y su barra de progreso de forma que aparezca de
//! forma flotante sin que la app se abra y se pueda escuchar y al terminar
//! se cierre».
//!
//! Es la `BarraDelReproductor.kt` del movil —atras 10, play o pausa,
//! adelante 10, la velocidad en ciclo y el aspa— en una ventanita siempre
//! encima, sin boton en la barra de tareas y sin abrir el chat, como el
//! microfono flotante. Lo que cambia es el reparto: en el chat la linea de
//! avance son tres pixeles pegados abajo porque toda la barra salta; aqui
//! la ventana tambien se arrastra, asi que va en dos filas:
//!
//! 1. el titulo y el tiempo (por aqui se arrastra) y el aspa;
//! 2. los tres botones del transporte, la pista con su bolita (pinchar o
//!    arrastrar en ella salta) y la velocidad.
//!
//! **Uno solo.** Un segundo pedido mientras suena no abre otra ventana:
//! cambia la pista de la que ya esta ([`COLA`]). Al acabar el audio la
//! ventana se va sola; pausada se queda hasta que se cierra.
//!
//! **Varios a la vez** (abrir tres audios juntos desde el Explorador): suena
//! el primero y los demas esperan en fila; al acabar uno empieza el
//! siguiente en la misma ventana, y la ventana se va al acabar el ultimo. Un
//! pedido nuevo sustituye la fila entera, como sustituye la pista.
//!
//! Va en su hilo con su propio motor de sonido (`pixpin_audio::Salida` es
//! del hilo que lo crea). El reproductor del chat (`crate::audio`) se para
//! antes de lanzarlo, en el hilo principal: dos audios a la vez no se
//! entienden.

#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_audio::salida::Salida;
use pixpin_geom::{Punto, Rect};
use pixpin_render::{Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};

use super::{OSCURO, mi};
use crate::overlay::Recursos;
use crate::ventanita::{Botones, dentro};

/// El tamano logico de la ventana.
const ANCHO: u32 = 380;
const ALTO: u32 = 92;
/// Lo que se mueve el raton antes de que un clic pase a ser un arrastre.
const UMBRAL_ARRASTRE: i32 = 4;
/// Lo que se queda la barra llena al acabar, para que se vea que acabo.
const AL_ACABAR: Duration = Duration::from_millis(700);
/// Lo que se queda un fallo en pantalla antes de cerrarse.
const FALLO_DURA: Duration = Duration::from_secs(4);

const VK_ESCAPE: u32 = 0x1B;
const VK_ESPACIO: u32 = 0x20;
const VK_IZQUIERDA: u32 = 0x25;
const VK_DERECHA: u32 = 0x27;

/// Lo que suena: el fichero y lo que se lee en la barra.
#[derive(Debug, Clone)]
pub(crate) struct Pista {
    pub ruta: PathBuf,
    pub titulo: String,
}

/// Por donde se le manda otra pista a la ventana que ya esta abierta.
/// `None`: no hay ninguna. Se escribe y se lee siempre con el cerrojo, asi
/// que una pista no puede caer en una ventana que ya se esta yendo.
/// Cada envio es una fila: la primera suena ya y las demas esperan.
static COLA: Mutex<Option<Sender<Vec<Pista>>>> = Mutex::new(None);

/// **Hace sonar `pista` en el reproductor flotante**: la abre o, si ya hay
/// uno, le cambia la pista. `no_se_pudo` es el rotulo del fallo, traducido
/// en el hilo principal (el `Catalogo` no cruza hilos).
pub(crate) fn lanzar(pista: Pista, no_se_pudo: String) {
    lanzar_fila(vec![pista], no_se_pudo);
}

/// Como [`lanzar`], con varias pistas seguidas: suena la primera y las
/// demas despues, por orden. Una fila vacia no hace nada.
pub(crate) fn lanzar_fila(pistas: Vec<Pista>, no_se_pudo: String) {
    if pistas.is_empty() {
        return;
    }
    let mut cola = COLA.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(tx) = cola.as_ref() {
        match tx.send(pistas) {
            Ok(()) => return,
            Err(e) => {
                *cola = None;
                return lanzar_hilo(e.0, no_se_pudo, &mut cola);
            }
        }
    }
    lanzar_hilo(pistas, no_se_pudo, &mut cola);
}

fn lanzar_hilo(pistas: Vec<Pista>, no_se_pudo: String, cola: &mut Option<Sender<Vec<Pista>>>) {
    let (tx, rx) = channel();
    *cola = Some(tx);
    let hecho = std::thread::Builder::new()
        .name("reproductor-flotante".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let mut pistas = pistas;
            loop {
                if let Err(e) = Recursos::nuevos().and_then(|r| flotar(&r, pistas, &rx, &no_se_pudo)) {
                    tracing::warn!(?e, "no se pudo abrir el reproductor flotante");
                }
                // Lo que llego mientras se cerraba vuelve a abrirla; si no
                // llego nada, se quita la cola con el cerrojo cogido, y a
                // partir de ahi el siguiente pedido abre otra.
                let mut cola = COLA.lock().unwrap_or_else(|e| e.into_inner());
                match rx.try_recv() {
                    Ok(otras) if !otras.is_empty() => {
                        drop(cola);
                        pistas = otras;
                    }
                    _ => {
                        *cola = None;
                        break;
                    }
                }
            }
        });
    if let Err(e) = hecho {
        *cola = None;
        tracing::warn!(?e, "no se pudo lanzar el hilo del reproductor flotante");
    }
}

// --- El reparto (puro) ----------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Boton {
    Atras,
    Tocar,
    Adelante,
    Velocidad,
    Cerrar,
    /// La pista: pinchar en ella salta.
    Pista,
}

/// Donde cae cada cosa, en pixeles fisicos de la ventana.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Reparto {
    titulo: RectF,
    cerrar: RectF,
    atras: RectF,
    tocar: RectF,
    adelante: RectF,
    velocidad: RectF,
    /// La zona que se pincha (alta, para no tener que apuntar).
    pista: RectF,
    /// La linea que se pinta, centrada en `pista`.
    linea: RectF,
}

fn repartir(w: f32, h: f32, e: f32) -> Reparto {
    let margen = 10.0 * e;
    let lado = 34.0 * e;
    let fila1 = 36.0 * e;
    let cerrar = RectF {
        x: w - margen - 28.0 * e,
        y: 6.0 * e,
        ancho: 28.0 * e,
        alto: 28.0 * e,
    };
    let y2 = fila1 + ((h - fila1) - lado) / 2.0 - 2.0 * e;
    let redondo = |x: f32| RectF {
        x,
        y: y2,
        ancho: lado,
        alto: lado,
    };
    let atras = redondo(margen);
    let tocar = redondo(atras.x + lado + 2.0 * e);
    let adelante = redondo(tocar.x + lado + 2.0 * e);
    let velocidad = RectF {
        x: w - margen - 44.0 * e,
        y: y2,
        ancho: 44.0 * e,
        alto: lado,
    };
    let x_pista = adelante.x + lado + 10.0 * e;
    let ancho_pista = (velocidad.x - 10.0 * e - x_pista).max(0.0);
    let pista = RectF {
        x: x_pista,
        y: y2,
        ancho: ancho_pista,
        alto: lado,
    };
    let grueso = 4.0 * e;
    Reparto {
        titulo: RectF {
            x: 16.0 * e,
            y: 0.0,
            ancho: (cerrar.x - 24.0 * e).max(0.0),
            alto: fila1,
        },
        cerrar,
        atras,
        tocar,
        adelante,
        velocidad,
        pista,
        linea: RectF {
            x: x_pista,
            y: y2 + (lado - grueso) / 2.0,
            ancho: ancho_pista,
            alto: grueso,
        },
    }
}

/// A que fraccion de la pista lleva un punto `x` (de 0 a 1).
fn fraccion_en(r: &Reparto, x: f32) -> f32 {
    if r.linea.ancho <= 0.0 {
        return 0.0;
    }
    ((x - r.linea.x) / r.linea.ancho).clamp(0.0, 1.0)
}

// --- La ventana ------------------------------------------------------------

/// Lo que se sabe de lo que suena, leido de la tarjeta en cada latido.
struct Suena {
    salida: Option<Salida>,
    pista: Pista,
    velocidad: f32,
    sonando: bool,
    posicion_ms: i64,
    duracion_ms: i64,
    /// Si acabo y cuando: la ventana se va poco despues.
    acabo: Option<Instant>,
    fallo: Option<Instant>,
}

impl Suena {
    fn cargar(&mut self, pista: Pista) {
        self.pista = pista;
        self.posicion_ms = 0;
        self.duracion_ms = 0;
        self.acabo = None;
        self.fallo = None;
        // El mismo motor si ya lo habia: crearlo cuesta decenas de ms.
        let cargado = match self.salida.as_ref() {
            Some(s) => s.cambiar_fuente(&self.pista.ruta),
            None => Salida::abrir(&self.pista.ruta).map(|s| {
                self.salida = Some(s);
            }),
        };
        match cargado.and_then(|()| match self.salida.as_ref() {
            Some(s) => s.tocar(self.velocidad),
            None => Ok(()),
        }) {
            Ok(()) => self.sonando = true,
            Err(e) => {
                tracing::warn!(?e, ruta = %self.pista.ruta.display(), "no se pudo hacer sonar el audio");
                self.sonando = false;
                self.fallo = Some(Instant::now());
            }
        }
    }

    fn latido(&mut self) {
        let Some(s) = self.salida.as_ref() else {
            return;
        };
        if self.fallo.is_none() && s.fallo() {
            self.fallo = Some(Instant::now());
            self.sonando = false;
        }
        if s.lista() {
            self.duracion_ms = s.duracion_ms();
        }
        if s.termino() {
            self.sonando = false;
            self.posicion_ms = self.duracion_ms;
            self.acabo = Some(Instant::now());
        } else if self.acabo.is_none() {
            self.posicion_ms = s.posicion_ms().min(self.duracion_ms.max(0));
        }
    }

    fn alternar(&mut self) {
        let Some(s) = self.salida.as_ref() else {
            return;
        };
        if self.sonando {
            s.pausar();
            self.sonando = false;
        } else {
            if self.acabo.take().is_some() {
                s.ir_a_ms(0);
            }
            if s.tocar(self.velocidad).is_ok() {
                self.sonando = true;
            }
        }
    }

    fn saltar(&mut self, ms: i64) {
        if let Some(s) = self.salida.as_ref() {
            let destino = pixpin_audio::reloj::destino_al_saltar(s.posicion_ms(), ms, self.duracion_ms);
            s.ir_a_ms(destino);
            self.posicion_ms = destino;
        }
    }

    fn ir_a(&mut self, fraccion: f32) {
        if let Some(s) = self.salida.as_ref()
            && let Some(ms) = pixpin_audio::reloj::destino_de_fraccion(fraccion, self.duracion_ms)
        {
            s.ir_a_ms(ms);
            self.posicion_ms = ms;
            self.acabo = None;
        }
    }

    fn otra_velocidad(&mut self) {
        self.velocidad = pixpin_audio::reloj::siguiente_velocidad(self.velocidad);
        if let Some(s) = self.salida.as_ref()
            && self.sonando
        {
            s.poner_velocidad(self.velocidad);
        }
    }
}

/// Abajo a la derecha del monitor del raton, encima de la barra de tareas,
/// como el microfono flotante.
fn marco_para(area: Rect, escala_por_cien: u32) -> Rect {
    let e = |v: u32| v * escala_por_cien / 100;
    let (w, h) = (e(ANCHO).min(area.ancho), e(ALTO).min(area.alto));
    Rect {
        x: area.x + area.ancho as i32 - w as i32 - e(24) as i32,
        y: area.y + area.alto as i32 - h as i32 - e(24) as i32,
        ancho: w,
        alto: h,
    }
}

/// Parte una fila en la que suena ya y las que esperan. `None` si no trae
/// ninguna.
fn partir_fila(pistas: Vec<Pista>) -> Option<(Pista, VecDeque<Pista>)> {
    let mut fila: VecDeque<Pista> = pistas.into();
    let primera = fila.pop_front()?;
    Some((primera, fila))
}

fn flotar(
    recursos: &Recursos,
    pistas: Vec<Pista>,
    cola: &Receiver<Vec<Pista>>,
    no_se_pudo: &str,
) -> Result<()> {
    let Some((pista, mut siguientes)) = partir_fila(pistas) else {
        return Ok(());
    };
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .monitor_en(pixpin_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let e = monitor.escala_por_cien as f32 / 100.0;
    let mut marco = marco_para(monitor.area_trabajo, monitor.escala_por_cien);
    let mut ventana = VentanaOverlay::nueva(marco).context("no se pudo abrir la ventana")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(&motor, &recursos.d3d(), ventana.handle(), marco.ancho, marco.alto)
        .context("sin superficie para el reproductor")?;

    let mut suena = Suena {
        salida: None,
        pista: pista.clone(),
        velocidad: 1.0,
        sonando: false,
        posicion_ms: 0,
        duracion_ms: 0,
        acabo: None,
        fallo: None,
    };
    suena.cargar(pista);

    ventana.mostrar();
    ventana.traer_encima();
    // El foco, para que Espacio y Escape lleguen: el lanzador dio permiso
    // de pasar al frente con `AllowSetForegroundWindow`.
    ventana.enfocar();

    let reparto = repartir(marco.ancho as f32, marco.alto as f32, e);
    let mut botones: Botones<Boton> = Botones::default();
    // Pulsado: donde, el marco de entonces, que habia debajo y si ya es un
    // arrastre de la ventana.
    let mut pulsado: Option<(Punto, Rect, Option<Boton>, bool)> = None;
    let mut vivo = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        while let Ok(otras) = cola.try_recv() {
            if let Some((otra, resto)) = partir_fila(otras) {
                suena.cargar(otra);
                siguientes = resto;
            }
        }
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match ev {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    if let Some((desde, origen, debajo, arrastre)) = pulsado.as_mut() {
                        if *debajo == Some(Boton::Pista) {
                            // Arrastrar por la pista va moviendo el punto.
                            suena.ir_a(fraccion_en(&reparto, botones.raton.0));
                        } else if debajo.is_none() {
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
                }
                EventoOverlay::BotonPulsado(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    let debajo = botones.bajo_el_raton();
                    if debajo == Some(Boton::Pista) {
                        suena.ir_a(fraccion_en(&reparto, botones.raton.0));
                    }
                    pulsado = Some((p, marco, debajo, false));
                }
                EventoOverlay::BotonSoltado(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    if let Some((_, _, Some(b), _)) = pulsado.take()
                        && botones.bajo_el_raton() == Some(b)
                    {
                        match b {
                            Boton::Atras => suena.saltar(-pixpin_audio::SALTO_MS),
                            Boton::Adelante => suena.saltar(pixpin_audio::SALTO_MS),
                            Boton::Tocar => suena.alternar(),
                            Boton::Velocidad => suena.otra_velocidad(),
                            Boton::Cerrar => vivo = false,
                            Boton::Pista => {}
                        }
                    }
                }
                EventoOverlay::Tecla { vk, .. } => match vk {
                    VK_ESCAPE => vivo = false,
                    VK_ESPACIO => suena.alternar(),
                    VK_IZQUIERDA => suena.saltar(-pixpin_audio::SALTO_MS),
                    VK_DERECHA => suena.saltar(pixpin_audio::SALTO_MS),
                    _ => {}
                },
                _ => {}
            }
        }
        if !vivo {
            break;
        }
        suena.latido();
        let se_va = suena.acabo.is_some_and(|t| t.elapsed() >= AL_ACABAR)
            || suena.fallo.is_some_and(|t| t.elapsed() >= FALLO_DURA);
        if se_va {
            // La siguiente de la fila, en la misma ventana; sin mas, se va.
            match siguientes.pop_front() {
                Some(otra) => suena.cargar(otra),
                None => break,
            }
        }

        if let Ok(d) = superficie.empezar(&motor) {
            let _ = motor.dibujar(&d, |p: &Pintor| {
                pintar(p, marco, e, &reparto, &suena, no_se_pudo, &mut botones)
            });
            let _ = superficie.presentar();
        }
        // Sonando, unas 20 veces por segundo (la bolita se mueve suave);
        // parado solo espera al raton.
        let latido = if suena.sonando || pulsado.is_some() { 50 } else { 250 };
        pixpin_shell::overlay::esperar_eventos(Some(latido));
    }
    ventana.ocultar();
    // Soltar la tarjeta antes de irse (el `Drop` llama a `Shutdown`).
    suena.salida = None;
    Ok(())
}

fn pintar(
    p: &Pintor,
    marco: Rect,
    e: f32,
    r: &Reparto,
    suena: &Suena,
    no_se_pudo: &str,
    botones: &mut Botones<Boton>,
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
        20.0 * e,
        tema.isla_borde,
    );
    p.rellenar_redondeado(
        RectF {
            x: 1.0,
            y: 1.0,
            ancho: w - 2.0,
            alto: h - 2.0,
        },
        19.0 * e,
        tema.isla,
    );
    botones.vaciar();

    let tam = 13.0 * e;
    // La primera fila: el titulo y, a su derecha, el tiempo.
    let tiempo = format!(
        "{} / {}",
        pixpin_audio::duracion_legible(suena.posicion_ms),
        pixpin_audio::duracion_legible(suena.duracion_ms)
    );
    let (ancho_tiempo, alto_t) = p.medir_texto(&tiempo, 12.0 * e);
    let y1 = r.titulo.y + (r.titulo.alto - alto_t) / 2.0 + 2.0 * e;
    p.texto(
        &tiempo,
        r.titulo.x + r.titulo.ancho - ancho_tiempo,
        y1,
        12.0 * e,
        tema.campo_apagado,
    );
    let (titulo, color_titulo) = if suena.fallo.is_some() {
        (no_se_pudo, crate::ventanita::ROJO)
    } else {
        (suena.pista.titulo.as_str(), tema.texto)
    };
    p.texto_linea(
        titulo,
        r.titulo.x,
        y1,
        tam,
        (r.titulo.ancho - ancho_tiempo - 12.0 * e).max(0.0),
        color_titulo,
    );

    // Los botones redondos, con su sombra al pasar por encima.
    let raton = botones.raton;
    let encima = |caja: RectF| dentro(caja, raton);
    let lado_icono = 22.0 * e;
    let tocar = if suena.sonando { &mi::PAUSE } else { &mi::PLAY_ARROW };
    for (icono, caja, que, lado) in [
        (&mi::CLOSE, r.cerrar, Boton::Cerrar, 18.0 * e),
        (&mi::REPLAY_10, r.atras, Boton::Atras, lado_icono),
        (tocar, r.tocar, Boton::Tocar, 26.0 * e),
        (&mi::FORWARD_10, r.adelante, Boton::Adelante, lado_icono),
    ] {
        if encima(caja) {
            p.rellenar_redondeado(caja, caja.ancho / 2.0, tema.boton_sobre);
        }
        p.icono(
            icono,
            RectF {
                x: caja.x + (caja.ancho - lado) / 2.0,
                y: caja.y + (caja.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            tema.texto,
        );
        botones.zona(caja, que);
    }

    // La velocidad se escribe, como en el movil («1,5×»).
    if encima(r.velocidad) {
        p.rellenar_redondeado(r.velocidad, 10.0 * e, tema.boton_sobre);
    }
    let vel = pixpin_audio::velocidad_legible(suena.velocidad);
    let (vw, vh) = p.medir_texto(&vel, tam);
    p.texto(
        &vel,
        r.velocidad.x + (r.velocidad.ancho - vw) / 2.0,
        r.velocidad.y + (r.velocidad.alto - vh) / 2.0,
        tam,
        tema.enviar,
    );
    botones.zona(r.velocidad, Boton::Velocidad);

    // La pista: el fondo, lo andado y la bolita. Sin duracion todavia solo
    // el fondo: una barra llena al empezar seria mentira.
    let radio = r.linea.alto / 2.0;
    p.rellenar_redondeado(r.linea, radio, tema.boton_sobre);
    if suena.duracion_ms > 0 {
        let f = pixpin_audio::reloj::fraccion(suena.posicion_ms, suena.duracion_ms);
        p.rellenar_redondeado(
            RectF {
                ancho: (r.linea.ancho * f).max(r.linea.alto),
                ..r.linea
            },
            radio,
            tema.enviar,
        );
        let bola = if encima(r.pista) { 7.0 } else { 5.5 } * e;
        p.circulo(
            (r.linea.x + r.linea.ancho * f, r.linea.y + radio),
            bola,
            tema.texto,
        );
    }
    botones.zona(r.pista, Boton::Pista);
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn nada_se_monta_y_la_pista_queda_entre_los_botones_y_la_velocidad() {
        for e in [1.0, 1.25, 1.5, 2.0] {
            let r = repartir(ANCHO as f32 * e, ALTO as f32 * e, e);
            assert!(r.atras.x + r.atras.ancho <= r.tocar.x);
            assert!(r.tocar.x + r.tocar.ancho <= r.adelante.x);
            assert!(r.adelante.x + r.adelante.ancho < r.pista.x);
            assert!(r.pista.x + r.pista.ancho < r.velocidad.x);
            assert!(r.velocidad.x + r.velocidad.ancho <= ANCHO as f32 * e);
            assert!(r.cerrar.y + r.cerrar.alto <= r.atras.y, "el aspa va en la fila de arriba");
            assert!(r.atras.y + r.atras.alto <= ALTO as f32 * e);
            assert!(r.pista.ancho > 150.0 * e, "una pista que se pueda apuntar");
        }
    }

    #[test]
    fn pinchar_en_la_pista_lleva_a_su_fraccion_y_no_se_sale() {
        let r = repartir(380.0, 92.0, 1.0);
        assert_eq!(fraccion_en(&r, r.linea.x), 0.0);
        assert_eq!(fraccion_en(&r, r.linea.x + r.linea.ancho), 1.0);
        assert!((fraccion_en(&r, r.linea.x + r.linea.ancho / 2.0) - 0.5).abs() < 1e-4);
        assert_eq!(fraccion_en(&r, -50.0), 0.0);
        assert_eq!(fraccion_en(&r, 5000.0), 1.0);
    }

    #[test]
    fn sale_abajo_a_la_derecha_dentro_del_area_de_trabajo() {
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        let m = marco_para(area, 150);
        assert_eq!(m.ancho, ANCHO * 150 / 100);
        assert!(m.x + m.ancho as i32 <= 1920 && m.y + m.alto as i32 <= 1040);
        assert!(m.x > 1000 && m.y > 800);
    }

    #[test]
    fn abrir_varios_audios_suena_el_primero_y_los_demas_esperan_en_orden() {
        let p = |n: &str| Pista {
            ruta: PathBuf::from(format!("{n}.mp3")),
            titulo: n.into(),
        };
        let (ya, fila) = partir_fila(vec![p("a"), p("b"), p("c")]).expect("hay fila");
        assert_eq!(ya.titulo, "a");
        let resto: Vec<_> = fila.iter().map(|x| x.titulo.as_str()).collect();
        assert_eq!(resto, ["b", "c"]);
        // Caso negativo: una fila vacia no abre ventana.
        assert!(partir_fila(Vec::new()).is_none());
    }
}
