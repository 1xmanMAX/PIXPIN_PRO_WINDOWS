//! La orquestacion del overlay: capturar, mostrar, interactuar, decidir.
//!
//! Secuencia innegociable (spec 2.2): la instantanea de TODOS los monitores
//! se toma ANTES de crear ventana alguna; al reves, la captura se incluiria
//! a si misma. El objetivo atajo->overlay visible es < 50 ms y queda medido
//! en el log.
//!
//! Este modulo es `forbid(unsafe_code)`: todo el dibujo pasa por el pintor
//! seguro de `pixpin-render` y toda la interaccion por el estado puro de
//! `pixpin-ui`. Aqui solo se cablean piezas ya probadas.

use crate::captura2::disposicion::{
    self as d2, Accion, BarraAcciones, BarraAnotar, BarraModos, EnAnotar, EnModos, EnSelector,
    Modo, PanelLupa, Selector, Util,
};
use crate::captura2::pintar as p2;
use crate::captura2::{Campo, Contexto, Sesion};
use anyhow::{Context, Result};
use pixpin_capture::{
    Dispositivo, Duplicador, Instantanea, SesionViva, a_imagen, capturar_monitor, componer_region,
    enumerar_monitores,
};
use pixpin_codec::ImagenRgba;
use pixpin_geom::Candidato;
use pixpin_geom::{Monitor, Punto, Rect};
use pixpin_nivel::Nivel;
use pixpin_render::{Color, MotorRender, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin, VentanaOverlay, bucle_modal};
use pixpin_shell::uia::Uia;
use pixpin_shell::ventana::Continuar;
use pixpin_ui::{
    Efecto, EstadoOverlay, EventoEntrada, Fase, FormaCursor, FormatoColorLupa, PanelTodo,
    TeclaOverlay, texto_color,
};
use std::rc::Rc;
use std::time::Instant;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

/// Codigos de tecla virtuales que el overlay traduce. Numericos y con
/// nombre para no arrastrar mas features del crate windows al ejecutable.
const VK_ESCAPE: u32 = 0x1B;
const VK_RETURN: u32 = 0x0D;
const VK_SPACE: u32 = 0x20;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;
const VK_BACK: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_SHIFT: u32 = 0x10;
const VK_DELETE: u32 = 0x2E;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoConfirmacion {
    /// Confirmar muestra la barra de resultado.
    ConBarra,
    /// Confirmar copia al portapapeles y cierra, sin barra (Ctrl+Alt+C).
    DirectoAlPortapapeles,
    /// Confirmar deja el recorte flotando como pin, sin barra (Ctrl+Alt+F).
    Pinear,
    /// Confirmar cierra el overlay y arranca la captura con scroll de la
    /// region (Ctrl+Alt+S, D75).
    Scroll,
    /// Sin recuadro: un clic copia el color bajo el cursor y cierra
    /// (Ctrl+Alt+D, D78).
    Cuentagotas,
    /// Confirmar lee el texto del recorte y lo copia.
    Texto,
    /// Confirmar oculta el overlay y arranca la grabacion en GIF (P5).
    Gif,
    /// Confirmar deja la zona flotando como pin EN VIVO: se ve en directo
    /// lo que pase en ella.
    PinEnVivo,
}

/// Lo que el overlay decidio. La imagen ya esta recortada y en CPU.
pub enum AccionFinal {
    /// La region viaja para que la pila de capturas sepa en que monitor
    /// plantar su icono.
    Copiar {
        imagen: ImagenRgba,
        region: Rect,
    },
    /// El recorte del que hay que leer el texto (P4).
    Texto(ImagenRgba),
    Guardar(ImagenRgba),
    GuardarComo(ImagenRgba),
    /// El pin nace 1:1 exactamente donde se recorto (D26): la region viaja.
    Pinear {
        imagen: ImagenRgba,
        region: Rect,
    },
    /// La region elegida para la captura con scroll (D75). No hay imagen:
    /// se captura muchas veces DESPUES, con el overlay ya oculto.
    /// La region elegida para grabar en GIF. Como el scroll, no hay
    /// imagen: se captura muchas veces despues, con el overlay oculto.
    Gif {
        region: Rect,
    },
    Scroll {
        region: Rect,
    },
    /// La zona del pin en vivo. Tampoco hay imagen: la trae la captura en
    /// directo, con el overlay ya cerrado.
    PinEnVivo {
        region: Rect,
    },
    /// La captura (con lo anotado) al chat de un proyecto, elegido en el
    /// selector de «Al chat», con su comentario.
    AlChat {
        imagen: ImagenRgba,
        region: Rect,
        proyecto: String,
        comentario: String,
    },
    Nada,
}

/// Etiquetas ya traducidas: el overlay no conoce el catalogo. Lo de la
/// captura v2 va en `captura2::Textos`, dentro del `Contexto`.
pub struct TextosBarra {
    /// El boton del panel antes de seleccionar: la pantalla entera (solo sin
    /// la barra de modos, que ya tiene «Pantalla»).
    pub todo: String,
}

/// Que parte de un rectangulo global le toca dibujar a este monitor, en
/// coordenadas locales del monitor.
///
/// La seleccion puede cruzar monitores; cada overlay recorta y traduce.
fn parte_local(global: Rect, monitor: Rect) -> Option<Rect> {
    global.interseccion(monitor).map(|r| Rect {
        x: r.x - monitor.x,
        y: r.y - monitor.y,
        ancho: r.ancho,
        alto: r.alto,
    })
}

fn a_rectf(r: Rect) -> RectF {
    RectF {
        x: r.x as f32,
        y: r.y as f32,
        ancho: r.ancho as f32,
        alto: r.alto as f32,
    }
}

/// Ventana + superficie persistentes de UN monitor. Viven en `Recursos`
/// entre capturas (ocultas): crearlas costaba ~90 ms de los 50 permitidos.
struct PiezaBase {
    monitor: Monitor,
    ventana: VentanaOverlay,
    superficie: Superficie,
}

/// Lo que cambia en CADA captura de un monitor: la instantanea congelada y
/// su bitmap. La ventana y la superficie son de `PiezaBase`.
struct Pieza<'a> {
    base: &'a PiezaBase,
    instantanea: Instantanea,
    fondo: ID2D1Bitmap1,
    fondo_vivo: Option<ID2D1Bitmap1>,
    sesion: Option<SesionViva>,
}

impl Pieza<'_> {
    fn monitor(&self) -> &Monitor {
        &self.base.monitor
    }
    fn ventana(&self) -> &VentanaOverlay {
        &self.base.ventana
    }
}

/// Lo caro que sobrevive ENTRE capturas: el dispositivo (90 ms), las
/// ventanas con su DComp y swapchain (90 ms) y un Duplicador DXGI por
/// monitor (pull: cero coste en reposo). Con todo persistente, el atajo
/// solo paga congelar (milisegundos) y ensenar ventanas ya hechas.
pub struct Recursos {
    dispositivo: Dispositivo,
    /// En `Rc` porque los pines lo comparten: un solo motor D2D para todo.
    motor: Rc<MotorRender>,
    duplicadores: Vec<(u32, Duplicador)>,
    bases: Vec<PiezaBase>,
}

impl Recursos {
    pub fn nuevos() -> Result<Recursos> {
        let dispositivo = Dispositivo::nuevo().context("sin dispositivo de captura")?;
        tracing::info!(grafica = %dispositivo.adaptador(), "dispositivo grafico creado");
        let motor = MotorRender::nuevo(dispositivo.d3d()).context("sin motor de dibujo")?;
        Ok(Recursos {
            dispositivo,
            motor: Rc::new(motor),
            duplicadores: Vec::new(),
            bases: Vec::new(),
        })
    }

    /// El dispositivo D3D compartido. Clonar la interfaz es contar una
    /// referencia, no copiar nada.
    pub fn d3d(&self) -> ID3D11Device {
        self.dispositivo.d3d().clone()
    }

    pub fn motor(&self) -> Rc<MotorRender> {
        Rc::clone(&self.motor)
    }

    pub fn dispositivo(&self) -> &Dispositivo {
        &self.dispositivo
    }

    /// La pantalla de un monitor AHORA, por la via que este disponible.
    /// La usa la capa viva para quedarse con lo que el usuario veia.
    pub fn congelar_monitor(&mut self, m: &Monitor) -> Result<Instantanea> {
        self.congelar(m)
    }

    /// Deja `bases` con exactamente una ventana por monitor actual. Si la
    /// disposicion no cambio, no hace nada; si cambio (monitor conectado,
    /// resolucion nueva), reconstruye solo entonces.
    fn preparar_bases(&mut self, monitores: &[Monitor]) -> Result<()> {
        let coincide = self.bases.len() == monitores.len()
            && self
                .bases
                .iter()
                .zip(monitores)
                .all(|(b, m)| b.monitor == *m);
        if coincide {
            return Ok(());
        }
        self.bases.clear();
        for m in monitores {
            let ventana = VentanaOverlay::nueva(m.area).context("sin ventana de overlay")?;
            let superficie = Superficie::nueva(
                &self.motor,
                self.dispositivo.d3d(),
                ventana.handle(),
                m.area.ancho,
                m.area.alto,
            )
            .context("sin superficie de composicion")?;
            self.bases.push(PiezaBase {
                monitor: *m,
                ventana,
                superficie,
            });
        }
        Ok(())
    }

    /// La pantalla del monitor AHORA. Via rapida: duplicador persistente;
    /// caidas: recrear el duplicador si perdio el acceso, y WGC si el
    /// duplicador esta frio (arranque con pantalla quieta) o no existe.
    fn congelar(&mut self, m: &Monitor) -> Result<Instantanea> {
        for intento in 0..2 {
            let indice = match self.duplicadores.iter().position(|(id, _)| *id == m.id) {
                Some(i) => i,
                None => match Duplicador::nuevo(&self.dispositivo, m.id, m.area) {
                    Ok(d) => {
                        self.duplicadores.push((m.id, d));
                        self.duplicadores.len() - 1
                    }
                    Err(e) => {
                        tracing::info!(
                            ?e,
                            monitor = m.id,
                            "sin duplicador; congelando por WGC (lento)"
                        );
                        break;
                    }
                },
            };
            match self.duplicadores[indice].1.instantanea(&self.dispositivo) {
                Ok(inst) => return Ok(inst),
                Err(pixpin_capture::ErrorCaptura::AccesoPerdido) if intento == 0 => {
                    // Cambio de modo, pantalla exclusiva, sesion bloqueada:
                    // se recrea una vez y se reintenta.
                    self.duplicadores.remove(indice);
                }
                Err(e) => {
                    tracing::info!(
                        ?e,
                        monitor = m.id,
                        "duplicador sin fotograma; congelando por WGC (lento)"
                    );
                    break;
                }
            }
        }
        capturar_monitor(&self.dispositivo, m.id, m.area)
            .with_context(|| format!("no se pudo capturar el monitor {}", m.id))
    }
}

/// Lo que no cambia durante un overlay y necesitan casi todos los pasos.
struct Ctx<'r> {
    dispositivo: &'r Dispositivo,
    motor: &'r MotorRender,
    nivel: Nivel,
    /// Con que se abrio (el atajo o el gesto). Lo que hace confirmar ahora
    /// lo dice `Sesion::confirmacion`, que puede cambiar con la barra.
    modo: ModoConfirmacion,
    textos: &'r TextosBarra,
    gesto: bool,
    uia: &'r Uia,
}

#[allow(clippy::too_many_arguments)] // el contexto de la captura v2 entra aqui y en ningun otro sitio
pub fn ejecutar_overlay(
    recursos: &mut Recursos,
    nivel: Nivel,
    modo: ModoConfirmacion,
    textos: &TextosBarra,
    formato_color: FormatoColorLupa,
    inicio: Option<Punto>,
    desde_ms: u32,
    contexto: Contexto,
) -> Result<AccionFinal> {
    let t0 = Instant::now();
    // Lo que ya se habia ido entre el gesto o el atajo y llegar aqui: la cola
    // del hilo principal. Si esto crece, el hilo estaba ocupado con otra cosa.
    let espera_cola_ms = pixpin_shell::gestos::ms_entre(desde_ms, pixpin_shell::gestos::reloj_ms());

    // 1. Congelar TODOS los monitores antes de ensenar ventana alguna.
    let disposicion = enumerar_monitores().context("sin monitores")?;
    let mut capturas: Vec<Instantanea> = Vec::new();
    for m in disposicion.monitores() {
        let t = Instant::now();
        capturas.push(recursos.congelar(m)?);
        let ms = t.elapsed().as_millis() as u64;
        // Congelar va en milisegundos con el duplicador caliente; pasado esto
        // ha caido a WGC o al primer fotograma de un duplicador frio, y es lo
        // que se busca cuando «el overlay sale tarde».
        if ms > 40 {
            tracing::info!(monitor = m.id, ms, "congelar lento");
        }
    }
    let t_captura = t0.elapsed().as_millis() as u64;

    // 2. Ventanas persistentes (se crean solo si la disposicion cambio) y
    //    el bitmap fresco de cada monitor.
    recursos.preparar_bases(disposicion.monitores())?;
    let dispositivo = &recursos.dispositivo;
    let motor = &*recursos.motor;
    let bases = &recursos.bases;
    let t_motor = t0.elapsed().as_millis() as u64;
    let mut piezas: Vec<Pieza> = Vec::new();
    for (base, instantanea) in bases.iter().zip(capturas) {
        let fondo = motor
            .bitmap_desde_textura(instantanea.textura())
            .context("no se pudo envolver la captura")?;
        piezas.push(Pieza {
            base,
            instantanea,
            fondo,
            fondo_vivo: None,
            sesion: None,
        });
    }

    // 3. Estado puro, snap y mostrar. El primer overlay recibe los avisos.
    let gesto = inicio.is_some();
    let mut estado = EstadoOverlay::nuevo(disposicion.clone());
    let uia = Uia::nueva(piezas[0].ventana().handle());
    let mut sesion = Sesion::nueva(contexto, modo, gesto, formato_color, Vec::new());
    let mut muestra_color: [u8; 4] = [0, 0, 0, 255];
    // El cursor real desde el primer fotograma: sin el, la barra de modos y
    // la lupa saldrian en el monitor equivocado hasta mover el raton.
    let _ = estado.procesar(EventoEntrada::RatonMovido(
        pixpin_shell::entorno::posicion_del_cursor(),
    ));
    let ctx = Ctx {
        dispositivo,
        motor,
        nivel,
        modo,
        textos,
        gesto,
        uia: &uia,
    };

    // Pintar ANTES de mostrar: una ventana retenida ensenaria el fotograma
    // de la captura anterior durante un instante.
    let t_a = t0.elapsed().as_millis() as u64;
    for p in &piezas {
        pintar(p, &estado, &mut sesion, muestra_color, &ctx);
    }
    let t_b = t0.elapsed().as_millis() as u64;
    for p in &piezas {
        p.ventana().mostrar();
    }
    // "Visible" se mide AQUI: pintado y ensenado. El foco viene justo
    // despues y no es visibilidad — AttachThreadInput puede costar decenas
    // de ms y no debe contaminar el intocable.
    tracing::info!(
        ms = t0.elapsed().as_millis() as u64,
        // Del gesto o el atajo a visible: lo que siente la mano.
        desde_el_usuario_ms = espera_cola_ms as u64 + t0.elapsed().as_millis() as u64,
        espera_cola_ms,
        origen = if inicio.is_some() { "gesto" } else { "atajo" },
        captura_ms = t_captura,
        prep_ms = t_a - t_motor,
        pintar_ms = t_b - t_a,
        mostrar_ms = t0.elapsed().as_millis() as u64 - t_b,
        "overlay visible"
    );
    // Las ventanas con su nombre, para el modo «Ventana» y la etiqueta. Ya
    // con el overlay a la vista (no cuenta en lo de arriba): la lista no
    // incluye el overlay, que se reconoce por su clase.
    if sesion.activa {
        sesion.ventanas = pixpin_shell::ventanas_visibles::ventanas_visibles();
    }
    // El primero toma el foco: sin esto el overlay es sordo al teclado.
    piezas[0].ventana().enfocar();
    for p in &piezas {
        p.ventana().invalidar();
    }

    // Gesto con Alt (D81): el boton ya esta pulsado desde `inicio`, antes
    // de que el overlay existiera. Se toma la captura del raton y se
    // reproduce el pulsado, y el overlay arranca ya trazando; al soltar se
    // confirma solo. Si el boton se solto antes de llegar aqui (un clic sin
    // arrastre), el overlay abre normal, en exploracion.
    // Dos cosas distintas que antes iban en la misma bandera, y por eso el
    // gesto acababa pidiendo Enter: que el overlay lo haya abierto un gesto,
    // que es lo que decide si soltar confirma, y que el boton siga pulsado,
    // que es lo que decide si hay que reproducir el arrastre. Lo segundo
    // depende de un instante concreto y puede fallar; lo primero no.
    let arrastrando =
        gesto && (pixpin_shell::gesto_en_curso() || pixpin_shell::boton_del_raton_pulsado());
    // Si el arrastre ya acabo (el overlay tardo mas que la mano), el recorte
    // va de la pulsacion a la soltada que vio el gancho: nunca del sitio
    // donde estuviera el cursor al salir el overlay.
    let soltada = pixpin_shell::gestos::soltada_del_gesto().map(|(x, y)| Punto { x, y });
    let arranque = arranque_del_gesto(inicio, arrastrando, soltada);
    if gesto {
        tracing::info!(
            ?arranque,
            desde_pulsacion_ms =
                pixpin_shell::gestos::ms_entre(desde_ms, pixpin_shell::gestos::reloj_ms()),
            "arranque del gesto"
        );
    }
    let mut ya_decidido = false;
    if let Some(p) = inicio.filter(|_| arranque != ArranqueGesto::Ninguno) {
        let pieza = piezas
            .iter()
            .position(|z| z.monitor().area.contiene(p))
            .unwrap_or(0);
        let hwnd0 = piezas[pieza].ventana().handle();
        if matches!(arranque, ArranqueGesto::Arrastrando(_)) {
            piezas[pieza].ventana().capturar_raton();
        }
        for evento in eventos_de_arranque(arranque) {
            let seguir = procesar_evento(
                hwnd0,
                evento,
                &mut estado,
                &mut sesion,
                &mut muestra_color,
                &mut piezas,
                &ctx,
            );
            if seguir == Continuar::No {
                // La soltada reproducida ya confirmo: no hay nada que esperar.
                ya_decidido = true;
                break;
            }
        }
    }

    // 4. El bucle modal. Las ventanas viven en `piezas`; el slice del
    //    contrato queda vacio porque el bombeo no filtra por ventana.
    if !ya_decidido {
        bucle_modal(&[], |hwnd, evento| {
            // El gesto de Alt + central (D140) termina al soltar el central: para
            // la seleccion es la misma soltada que la del izquierdo. Fuera de un
            // gesto el central no significa nada aqui.
            let evento = match evento {
                EventoOverlay::BotonCentralSoltado(p) if gesto => {
                    if let Some(z) = piezas.first() {
                        z.ventana().soltar_raton();
                    }
                    EventoOverlay::BotonSoltado(p)
                }
                otro => otro,
            };
            procesar_evento(
                hwnd,
                evento,
                &mut estado,
                &mut sesion,
                &mut muestra_color,
                &mut piezas,
                &ctx,
            )
        });
    }

    // 5. Desmontar: sesiones fuera, ventanas OCULTAS (no destruidas: son
    //    persistentes) y el hilo UIA parado.
    let fuentes: Vec<Instantanea> = {
        let mut f = Vec::new();
        for mut p in piezas {
            if let Some(s) = p.sesion.take() {
                s.cerrar();
            }
            p.ventana().ocultar();
            f.push(p.instantanea);
        }
        f
    };
    uia.detener();

    // La imagen se materializa UNA vez, aqui, al final: es el unico punto
    // donde la seleccion cruza a la CPU.
    match PENDIENTE.take() {
        // La captura con scroll no materializa nada aqui: se captura muchas
        // veces despues, con las ventanas ya ocultas (D75).
        Some((QueAccion::Scroll, region)) => Ok(AccionFinal::Scroll { region }),
        // La grabacion tampoco materializa nada aqui: se captura muchas
        // veces despues, con las ventanas ya ocultas.
        Some((QueAccion::Gif, region)) => Ok(AccionFinal::Gif { region }),
        Some((QueAccion::PinEnVivo, region)) => Ok(AccionFinal::PinEnVivo { region }),
        Some((que, region)) => {
            let recorte = componer_region(dispositivo, &fuentes, region)
                .context("no se pudo recortar la seleccion")?;
            let mut imagen =
                a_imagen(dispositivo, &recorte).context("no se pudo bajar la seleccion a CPU")?;
            // Lo anotado entra en la imagen; el texto (OCR) se lee de la
            // captura limpia, que es la que se lee mejor.
            if !matches!(que, QueAccion::Texto) {
                if let Some(a) = sesion.anotacion.as_mut().filter(|a| !a.vacia()) {
                    if let Err(e) = a.hornear(&mut imagen, motor, dispositivo.d3d()) {
                        tracing::warn!(?e, "lo anotado no se pudo meter en la captura");
                    }
                }
            }
            Ok(match que {
                QueAccion::Copiar => AccionFinal::Copiar { imagen, region },
                QueAccion::Texto => AccionFinal::Texto(imagen),
                QueAccion::Guardar => AccionFinal::Guardar(imagen),
                QueAccion::GuardarComo => AccionFinal::GuardarComo(imagen),
                QueAccion::Pinear => AccionFinal::Pinear { imagen, region },
                QueAccion::AlChat => match sesion.al_chat.take() {
                    Some((proyecto, comentario)) => AccionFinal::AlChat {
                        imagen,
                        region,
                        proyecto,
                        comentario,
                    },
                    None => AccionFinal::Nada,
                },
                QueAccion::Scroll => AccionFinal::Scroll { region },
                QueAccion::Gif => AccionFinal::Gif { region },
                QueAccion::PinEnVivo => AccionFinal::PinEnVivo { region },
            })
        }
        None => Ok(AccionFinal::Nada),
    }
}

/// La accion elegida dentro del bucle, pendiente de materializar la imagen
/// al salir. Thread-local del hilo de interfaz, como las colas del overlay.
#[derive(Clone, Copy)]
enum QueAccion {
    Copiar,
    Gif,
    /// Leer el texto del recorte y copiarlo, en vez de la imagen.
    Texto,
    Guardar,
    GuardarComo,
    Pinear,
    PinEnVivo,
    Scroll,
    /// Al chat del proyecto elegido en el selector (`Sesion::al_chat`).
    AlChat,
}

thread_local! {
    static PENDIENTE_ACCION: std::cell::Cell<Option<(QueAccion, Rect)>> =
        const { std::cell::Cell::new(None) };
}

struct Pendiente;
static PENDIENTE: Pendiente = Pendiente;

impl Pendiente {
    fn poner(&self, que: QueAccion, region: Rect) {
        PENDIENTE_ACCION.with(|p| p.set(Some((que, region))));
    }
    fn take(&self) -> Option<(QueAccion, Rect)> {
        PENDIENTE_ACCION.with(|p| p.take())
    }
}

/// Como arranca un overlay abierto por un gesto con Alt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArranqueGesto {
    /// Sin gesto, o un gesto sin arrastre: el overlay abre en exploracion.
    Ninguno,
    /// El boton sigue abajo: se reproduce la pulsacion y se sigue trazando.
    Arrastrando(Punto),
    /// El arrastre acabo antes de que el overlay saliera: se reproduce
    /// entero, de la pulsacion a la soltada, y se confirma solo.
    YaSoltado { desde: Punto, hasta: Punto },
}

/// Decide el arranque. El punto de partida es SIEMPRE el de la pulsacion
/// que vio el gancho (`inicio`), nunca donde este el cursor al salir.
fn arranque_del_gesto(
    inicio: Option<Punto>,
    sigue_pulsado: bool,
    soltada: Option<Punto>,
) -> ArranqueGesto {
    match (inicio, sigue_pulsado, soltada) {
        (None, _, _) => ArranqueGesto::Ninguno,
        (Some(p), true, _) => ArranqueGesto::Arrastrando(p),
        (Some(desde), false, Some(hasta)) => ArranqueGesto::YaSoltado { desde, hasta },
        (Some(_), false, None) => ArranqueGesto::Ninguno,
    }
}

/// Los eventos que reproducen ese arranque, en orden.
fn eventos_de_arranque(a: ArranqueGesto) -> Vec<EventoOverlay> {
    match a {
        ArranqueGesto::Ninguno => Vec::new(),
        ArranqueGesto::Arrastrando(p) => {
            vec![
                EventoOverlay::RatonMovido(p),
                EventoOverlay::BotonPulsado(p),
            ]
        }
        ArranqueGesto::YaSoltado { desde, hasta } => vec![
            EventoOverlay::RatonMovido(desde),
            EventoOverlay::BotonPulsado(desde),
            EventoOverlay::RatonMovido(hasta),
            EventoOverlay::BotonSoltado(hasta),
        ],
    }
}

/// Cada cuanto se baja de la GPU el color bajo el cursor MIENTRAS se arrastra.
/// Bajarlo es esperar a que la GPU acabe todo lo pendiente; hacerlo en cada
/// movimiento del raton hacia que el recuadro fuera detras de la mano en un
/// equipo con la grafica integrada. Treinta milisegundos no se notan en el
/// numero de la lupa.
const MUESTRA_COLOR_ARRASTRANDO_MS: u128 = 30;

thread_local! {
    static ULTIMA_MUESTRA: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) };
}

/// Si toca bajar el color ahora. Quieto o explorando, siempre; trazando, como
/// mucho una vez cada `MUESTRA_COLOR_ARRASTRANDO_MS`.
fn toca_muestrear(fase: Fase) -> bool {
    if fase != Fase::Trazando {
        ULTIMA_MUESTRA.with(|u| u.set(Some(Instant::now())));
        return true;
    }
    ULTIMA_MUESTRA.with(|u| {
        let toca = u
            .get()
            .is_none_or(|t| t.elapsed().as_millis() >= MUESTRA_COLOR_ARRASTRANDO_MS);
        if toca {
            u.set(Some(Instant::now()));
        }
        toca
    })
}

/// Que se ensena ahora de la captura v2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vista {
    /// Nada nuevo: un gesto, el cuentagotas, o se esta trazando (nada
    /// encima de la mano).
    Nada,
    /// Eligiendo: la barra de modos, las pistas, la lupa y la etiqueta.
    Elegir,
    /// Zona lista con un modo que pregunta: acciones y anotar.
    Despues,
    /// Zona lista con un modo que hace lo suyo (scroll, GIF, pin en vivo,
    /// texto): la barra de modos y un boton de confirmar.
    Confirmar,
}

fn vista(fase: Fase, activa: bool, confirmacion: ModoConfirmacion) -> Vista {
    if !activa {
        return Vista::Nada;
    }
    match fase {
        Fase::Explorando => Vista::Elegir,
        Fase::Lista if confirmacion == ModoConfirmacion::ConBarra => Vista::Despues,
        Fase::Lista => Vista::Confirmar,
        Fase::Trazando | Fase::Moviendo | Fase::Redimensionando => Vista::Nada,
    }
}

fn vista_de(estado: &EstadoOverlay, sesion: &Sesion, modo: ModoConfirmacion) -> Vista {
    vista(estado.fase(), sesion.activa, sesion.confirmacion(modo))
}

/// El monitor bajo un punto, o el primero.
fn monitor_en(piezas: &[Pieza], p: Punto) -> Monitor {
    piezas
        .iter()
        .find(|z| z.monitor().area.contiene(p))
        .map(|z| *z.monitor())
        .unwrap_or(*piezas[0].monitor())
}

/// El monitor de una region: el primero que toca.
fn monitor_de_region(piezas: &[Pieza], r: Rect) -> Monitor {
    piezas
        .iter()
        .find(|z| z.monitor().area.interseccion(r).is_some())
        .map(|z| *z.monitor())
        .unwrap_or(*piezas[0].monitor())
}

fn barra_modos(estado: &EstadoOverlay, sesion: &Sesion, piezas: &[Pieza]) -> (BarraModos, Monitor) {
    let m = monitor_en(piezas, estado.cursor());
    (
        BarraModos::colocar(
            m.area_trabajo,
            m.escala_por_cien,
            sesion.contexto.ultima_region.is_some(),
        ),
        m,
    )
}

fn barras_despues(
    estado: &EstadoOverlay,
    piezas: &[Pieza],
) -> (BarraAnotar, BarraAcciones, Monitor) {
    let sel = estado.seleccion();
    let m = monitor_de_region(piezas, sel);
    let (a, b) = d2::colocar_despues(sel, m.area_trabajo, m.escala_por_cien);
    (a, b, m)
}

fn selector_de(sesion: &Sesion, acciones: &BarraAcciones, m: &Monitor) -> Option<Selector> {
    let es = sesion.selector.as_ref()?;
    let boton = acciones.boton(Accion::AlChat)?;
    let n = es.visibles(&sesion.proyectos).len();
    Some(Selector::colocar(
        boton,
        m.area_trabajo,
        n,
        m.escala_por_cien,
    ))
}

fn boton_confirmar(estado: &EstadoOverlay, piezas: &[Pieza]) -> (Rect, Monitor) {
    let sel = estado.seleccion();
    let m = monitor_de_region(piezas, sel);
    (
        d2::boton_confirmar(sel, m.area_trabajo, m.escala_por_cien),
        m,
    )
}

/// Lo que miden la zona elegida o lo resaltado, para los campos y la
/// etiqueta. `(0, 0)` si no hay nada.
fn medidas(estado: &EstadoOverlay) -> (u32, u32) {
    match estado.fase() {
        Fase::Explorando => estado
            .rect_resaltado()
            .map_or((0, 0), |r| (r.ancho, r.alto)),
        _ => {
            let s = estado.seleccion();
            (s.ancho, s.alto)
        }
    }
}

/// Que hay de lo nuevo bajo un punto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bajo {
    Nada,
    Modos(EnModos),
    Anotar(EnAnotar),
    Acciones(Option<Accion>),
    Selector(EnSelector),
    /// Fuera del selector abierto: el clic lo cierra y nada mas.
    FueraSelector,
    Confirmar,
}

fn que_hay(
    p: Punto,
    estado: &EstadoOverlay,
    sesion: &Sesion,
    piezas: &[Pieza],
    modo: ModoConfirmacion,
) -> Bajo {
    match vista_de(estado, sesion, modo) {
        Vista::Nada => Bajo::Nada,
        Vista::Despues => {
            let (a, b, m) = barras_despues(estado, piezas);
            if let Some(s) = selector_de(sesion, &b, &m) {
                return match s.en(p) {
                    Some(x) => Bajo::Selector(x),
                    None => Bajo::FueraSelector,
                };
            }
            if let Some(x) = a.en(p) {
                return Bajo::Anotar(x);
            }
            if let Some(x) = b.en(p) {
                return Bajo::Acciones(x);
            }
            Bajo::Nada
        }
        v @ (Vista::Elegir | Vista::Confirmar) => {
            let (b, _) = barra_modos(estado, sesion, piezas);
            if let Some(x) = b.en(p) {
                return Bajo::Modos(x);
            }
            if v == Vista::Confirmar && boton_confirmar(estado, piezas).0.contiene(p) {
                return Bajo::Confirmar;
            }
            Bajo::Nada
        }
    }
}

/// Lo resaltado en los modos que no usan UI Automation: la ventana de
/// delante bajo el raton, o la pantalla entera.
fn candidatos_propios(sesion: &Sesion, piezas: &[Pieza], p: Punto) -> Vec<Candidato> {
    let rect = match sesion.modo {
        Modo::Ventana => sesion.ventana_en(p).map(|v| v.rect),
        Modo::Pantalla => Some(monitor_en(piezas, p).area),
        _ => None,
    };
    rect.map(|rect| {
        vec![Candidato {
            rect,
            profundidad: 0,
        }]
    })
    .unwrap_or_default()
}

fn cambiar_modo(
    m: Modo,
    estado: &mut EstadoOverlay,
    sesion: &mut Sesion,
    piezas: &[Pieza],
    ctx: &Ctx,
) {
    if sesion.anotada() {
        return;
    }
    sesion.modo = m;
    let c = if sesion.usa_uia() {
        ctx.uia.candidatos()
    } else {
        candidatos_propios(sesion, piezas, estado.cursor())
    };
    let _ = estado.procesar(EventoEntrada::Candidatos(c));
}

/// «Repetir la ultima zona»: la deja elegida, lista para su accion.
fn repetir(estado: &mut EstadoOverlay, sesion: &Sesion) {
    if sesion.anotada() {
        return;
    }
    if let Some(r) = sesion.contexto.ultima_region {
        let _ = estado.poner_seleccion(r);
    }
}

/// Aplica lo tecleado en los campos de medida.
fn aplicar_medidas(estado: &mut EstadoOverlay, sesion: &mut Sesion, piezas: &[Pieza]) {
    sesion.campo = None;
    let (w0, h0) = medidas(estado);
    let w = sesion.ancho.valor().unwrap_or(w0);
    let h = sesion.alto.valor().unwrap_or(h0);
    sesion.ancho = d2::Cifras::default();
    sesion.alto = d2::Cifras::default();
    if w == 0 || h == 0 || sesion.anotada() {
        return;
    }
    let sel = (estado.fase() == Fase::Lista).then(|| estado.seleccion());
    let m = monitor_en(piezas, estado.cursor());
    let r = d2::zona_de_medidas(w, h, sel, m.area);
    let _ = estado.poner_seleccion(r);
}

/// Hace una accion de despues. `No` cierra el overlay.
fn hacer_accion(a: Accion, estado: &EstadoOverlay, sesion: &mut Sesion) -> Continuar {
    let region = estado.seleccion();
    match a {
        Accion::Copiar => PENDIENTE.poner(QueAccion::Copiar, region),
        Accion::Pinear => PENDIENTE.poner(QueAccion::Pinear, region),
        Accion::Texto => PENDIENTE.poner(QueAccion::Texto, region),
        Accion::Guardar => PENDIENTE.poner(QueAccion::Guardar, region),
        Accion::Descartar => {}
        Accion::AlChat => {
            if sesion.selector.is_some() {
                sesion.selector = None;
            } else {
                sesion.abrir_selector();
            }
            return Continuar::Si;
        }
    }
    Continuar::No
}

/// Manda la captura al proyecto elegido del selector.
fn enviar(estado: &EstadoOverlay, sesion: &mut Sesion) -> Continuar {
    let Some(es) = &sesion.selector else {
        return Continuar::Si;
    };
    let Some(id) = es.destino(&sesion.proyectos) else {
        return Continuar::Si;
    };
    sesion.al_chat = Some((id, es.comentario.clone()));
    PENDIENTE.poner(QueAccion::AlChat, estado.seleccion());
    Continuar::No
}

/// El clic (pulsar y soltar) en algo de lo nuevo.
fn actuar(
    bajo: Bajo,
    estado: &mut EstadoOverlay,
    sesion: &mut Sesion,
    piezas: &mut [Pieza],
    ctx: &Ctx,
) -> Continuar {
    match bajo {
        Bajo::Nada => {}
        Bajo::FueraSelector => sesion.selector = None,
        Bajo::Selector(x) => match x {
            EnSelector::Fila(i) => {
                if let Some(es) = sesion.selector.as_mut() {
                    es.elegida = i;
                }
                return enviar(estado, sesion);
            }
            EnSelector::Enviar => return enviar(estado, sesion),
            EnSelector::Buscar => {
                if let Some(es) = sesion.selector.as_mut() {
                    es.en_comentario = false;
                }
            }
            EnSelector::Comentario => {
                if let Some(es) = sesion.selector.as_mut() {
                    es.en_comentario = true;
                }
            }
            EnSelector::Fondo => {}
        },
        Bajo::Acciones(Some(a)) => return hacer_accion(a, estado, sesion),
        Bajo::Acciones(None) => {}
        Bajo::Anotar(x) => {
            let sel = estado.seleccion();
            let escala = monitor_de_region(piezas, sel).escala_por_cien;
            match x {
                EnAnotar::Util(u) => sesion.tomar(u, sel, escala),
                EnAnotar::Color(i) => sesion.poner_color(i, sel, escala),
                EnAnotar::Grosor(i) => sesion.poner_grosor(i, sel, escala),
                EnAnotar::Deshacer => {
                    if let Some(a) = sesion.anotacion.as_mut() {
                        a.deshacer();
                    }
                }
                EnAnotar::Rehacer => {
                    if let Some(a) = sesion.anotacion.as_mut() {
                        a.rehacer();
                    }
                }
                EnAnotar::Fondo => {}
            }
        }
        Bajo::Modos(x) => match x {
            EnModos::Modo(m) => cambiar_modo(m, estado, sesion, piezas, ctx),
            EnModos::Ancho => sesion.campo = Some(Campo::Ancho),
            EnModos::Alto => sesion.campo = Some(Campo::Alto),
            EnModos::Proporcion(pr) => {
                sesion.proporcion = pr;
                if !sesion.anotada() {
                    let _ = estado.poner_proporcion(pr.par());
                }
            }
            EnModos::Repetir => repetir(estado, sesion),
            EnModos::Fondo => {}
        },
        Bajo::Confirmar => {
            let efecto = estado.procesar(EventoEntrada::Tecla(TeclaOverlay::Enter));
            return aplicar_efecto(efecto, estado, sesion, piezas, ctx);
        }
    }
    for p in piezas.iter() {
        p.ventana().invalidar();
    }
    Continuar::Si
}

fn procesar_evento(
    hwnd: HWND,
    evento: EventoOverlay,
    estado: &mut EstadoOverlay,
    sesion: &mut Sesion,
    muestra_color: &mut [u8; 4],
    piezas: &mut [Pieza],
    ctx: &Ctx,
) -> Continuar {
    let invalidar_todas = |piezas: &[Pieza]| {
        for p in piezas {
            p.ventana().invalidar();
        }
    };
    let modo = ctx.modo;

    match evento {
        EventoOverlay::RatonMovido(p) => {
            if sesion.dibujando {
                if let Some(a) = sesion.anotacion.as_mut() {
                    a.mover(p);
                }
                invalidar_todas(piezas);
                return Continuar::Si;
            }
            if sesion.usa_uia() {
                ctx.uia.pedir(p);
            }
            // El color bajo el cursor: un recorte de 1x1 y su bajada. Es
            // minusculo (4 bytes) y solo ocurre al mover el raton. Con las
            // barras de despues no hay lupa: no se baja nada.
            let con_lupa = vista_de(estado, sesion, modo) != Vista::Despues;
            if let Some(pieza) = piezas
                .iter()
                .find(|z| z.monitor().area.contiene(p))
                .filter(|_| con_lupa && toca_muestrear(estado.fase()))
            {
                let uno = Rect {
                    x: p.x,
                    y: p.y,
                    ancho: 1,
                    alto: 1,
                };
                if let Ok(rec) = pieza.instantanea.recortar(ctx.dispositivo, uno) {
                    if let Ok(img) = a_imagen(ctx.dispositivo, &rec) {
                        if img.pixeles.len() == 4 {
                            *muestra_color = [
                                img.pixeles[0],
                                img.pixeles[1],
                                img.pixeles[2],
                                img.pixeles[3],
                            ];
                        }
                    }
                }
            }
            let _ = estado.procesar(EventoEntrada::RatonMovido(p));
            if !sesion.usa_uia() && estado.fase() == Fase::Explorando {
                let c = candidatos_propios(sesion, piezas, p);
                let _ = estado.procesar(EventoEntrada::Candidatos(c));
            }
            let forma = forma_del_cursor(p, estado, sesion, piezas, modo);
            for pieza in piezas.iter() {
                pieza.ventana().poner_cursor(forma);
            }
            // La lupa sigue al raton: se redibujan todas las que tocan algo.
            invalidar_todas(piezas);
            Continuar::Si
        }
        EventoOverlay::BotonPulsado(p) => {
            // El cuentagotas (D78): el clic copia el color que la lupa esta
            // ensenando y cierra. No hay recuadro que empezar.
            if matches!(modo, ModoConfirmacion::Cuentagotas) {
                copiar_color(sesion.formato, *muestra_color);
                return Continuar::No;
            }
            let bajo = que_hay(p, estado, sesion, piezas, modo);
            // Un clic fuera de los campos de medida los suelta.
            if !matches!(bajo, Bajo::Modos(EnModos::Ancho | EnModos::Alto)) {
                sesion.campo = None;
            }
            if bajo != Bajo::Nada {
                // El clic se resuelve al soltar; tragarse el pulsado evita
                // que el estado empiece un trazado bajo la barra.
                sesion.pulsado_ui = true;
                invalidar_todas(piezas);
                return Continuar::Si;
            }
            sesion.pulsado_ui = false;
            if vista_de(estado, sesion, modo) == Vista::Despues {
                let sel = estado.seleccion();
                let con_util = sesion
                    .anotacion
                    .as_ref()
                    .map_or(sesion.util, |a| a.util)
                    .is_some();
                if con_util && sel.contiene(p) {
                    let escala = monitor_de_region(piezas, sel).escala_por_cien;
                    sesion.anotacion_en(sel, escala).pulsar(p);
                    sesion.dibujando = true;
                    if let Some(z) = piezas.iter().find(|z| z.ventana().handle() == hwnd) {
                        z.ventana().capturar_raton();
                    }
                    invalidar_todas(piezas);
                    return Continuar::Si;
                }
                // Con algo dibujado la zona ya no se mueve: lo anotado va
                // pegado a lo que tiene debajo.
                if sesion.anotada() {
                    return Continuar::Si;
                }
            }
            // El panel «Seleccionar todo», solo sin la barra de modos (que
            // ya tiene «Pantalla») y mientras no hay seleccion.
            if !sesion.activa && estado.fase() == Fase::Explorando {
                let en_panel = piezas.iter().any(|z| {
                    let m = z.monitor();
                    m.area.contiene(p) && PanelTodo::colocar(m.area, m.escala_por_cien).contiene(p)
                });
                if en_panel {
                    let _ = estado.procesar(EventoEntrada::Tecla(TeclaOverlay::SeleccionarTodo));
                    invalidar_todas(piezas);
                    return Continuar::Si;
                }
            }
            let _ = estado.procesar(EventoEntrada::BotonPulsado(p));
            invalidar_todas(piezas);
            Continuar::Si
        }
        EventoOverlay::BotonSoltado(p) => {
            if sesion.dibujando {
                sesion.dibujando = false;
                if let Some(a) = sesion.anotacion.as_mut() {
                    a.soltar(p);
                }
                if let Some(z) = piezas.iter().find(|z| z.ventana().handle() == hwnd) {
                    z.ventana().soltar_raton();
                }
                invalidar_todas(piezas);
                return Continuar::Si;
            }
            if sesion.pulsado_ui {
                sesion.pulsado_ui = false;
                let bajo = que_hay(p, estado, sesion, piezas, modo);
                return actuar(bajo, estado, sesion, piezas, ctx);
            }
            let efecto = estado.procesar(EventoEntrada::BotonSoltado(p));
            let seguir = aplicar_efecto(efecto, estado, sesion, piezas, ctx);
            // En un gesto con Alt (D81) soltar ya es confirmar: no hay
            // segundo paso. Solo si el arrastre dejo una seleccion; un clic
            // sin arrastre deja el overlay abierto para seleccionar a mano.
            if seguir == Continuar::Si && ctx.gesto && estado.fase() == Fase::Lista {
                let efecto = estado.procesar(EventoEntrada::Tecla(TeclaOverlay::Enter));
                return aplicar_efecto(efecto, estado, sesion, piezas, ctx);
            }
            seguir
        }
        EventoOverlay::Caracter(c) => {
            if let Some(campo) = sesion.campo {
                match campo {
                    Campo::Ancho => sesion.ancho.escribir(c),
                    Campo::Alto => sesion.alto.escribir(c),
                };
            } else if let Some(es) = sesion.selector.as_mut() {
                // Con la busqueda vacia, el numero de una fila la elige y
                // manda: es la chapita que se ve en cada fila.
                let fila = c
                    .to_digit(10)
                    .filter(|d| (1..=d2::FILAS_SELECTOR as u32).contains(d))
                    .map(|d| d as usize - 1)
                    .filter(|i| {
                        !es.en_comentario
                            && es.busqueda.is_empty()
                            && *i < es.visibles(&sesion.proyectos).len()
                    });
                match fila {
                    Some(i) => {
                        es.elegida = i;
                        return enviar(estado, sesion);
                    }
                    None => es.escribir(c),
                }
            } else if let Some(a) = sesion.anotacion.as_mut().filter(|a| a.escribiendo()) {
                a.caracter(c);
            }
            invalidar_todas(piezas);
            Continuar::Si
        }
        EventoOverlay::TeclaSoltada(vk) => {
            // Mayus sola, sin otra tecla en medio: cambia HEX/RGB. Al
            // soltarla y no al pulsarla, porque pulsada se repite sola.
            if vk == VK_SHIFT && sesion.mayus_sola {
                sesion.mayus_sola = false;
                if vista_de(estado, sesion, modo) != Vista::Despues {
                    sesion.cambiar_formato();
                    invalidar_todas(piezas);
                }
            }
            Continuar::Si
        }
        EventoOverlay::Tecla {
            vk, shift, ctrl, ..
        } => {
            if vk == VK_SHIFT {
                sesion.mayus_sola = !ctrl;
                return Continuar::Si;
            }
            sesion.mayus_sola = false;
            let seguir = tecla(vk, shift, ctrl, estado, sesion, muestra_color, piezas, ctx);
            invalidar_todas(piezas);
            seguir
        }
        EventoOverlay::Despierta => {
            // Dos emisores comparten MSG_DESPIERTA: el hilo UIA (candidatos
            // frescos) y las sesiones en vivo (fotograma nuevo). Atender
            // ambos es mas barato que distinguirlos.
            if sesion.usa_uia() {
                let _ = estado.procesar(EventoEntrada::Candidatos(ctx.uia.candidatos()));
            }
            if estado.vivo() {
                for p in piezas.iter_mut() {
                    if let Some(textura) = p.sesion.as_ref().and_then(|s| s.ultimo()) {
                        // Si envolver falla (textura en transito), se ignora:
                        // el proximo fotograma lo reintenta.
                        if let Ok(b) = ctx.motor.bitmap_desde_textura(&textura) {
                            p.fondo_vivo = Some(b);
                        }
                    }
                }
            }
            invalidar_todas(piezas);
            Continuar::Si
        }
        // El overlay de captura no acepta ficheros soltados (no llama a
        // `aceptar_ficheros`), asi que esto no deberia llegar nunca; si
        // llegara, se ignora antes que robarle el soltar a la ventana de
        // debajo.
        EventoOverlay::FicherosSoltados => Continuar::Si,
        EventoOverlay::Pintar => {
            if let Some(pieza) = piezas.iter().find(|z| z.ventana().handle() == hwnd) {
                pintar(pieza, estado, sesion, *muestra_color, ctx);
            }
            Continuar::Si
        }
        EventoOverlay::CambioDpi => Continuar::Si,
        // Alt+F4 sobre el overlay: cancelar limpiamente.
        EventoOverlay::Cerrar => Continuar::No,
        // El overlay de captura no usa la rueda.
        // Un atajo global pulsado con el overlay abierto se descarta: si
        // volviera a la cola principal, reabriria el overlay al cerrarlo.
        // Esta ventana no pidio entrada fina (D106): no deberia llegar
        // Muestra, pero si llegara no hay nada que hacer con ella aqui.
        EventoOverlay::Rueda(_)
        | EventoOverlay::RuedaHorizontal(_)
        // Solo llegan a ventanas que pidieron gestos tactiles; esta no.
        | EventoOverlay::RuedaFina(_)
        | EventoOverlay::DeslizTactil(_)
        | EventoOverlay::PellizcoTactil(_)
        | EventoOverlay::Atajo(_)
        | EventoOverlay::Muestra(_)
        | EventoOverlay::BotonCentralPulsado(_)
        // El derecho ya se convirtio en izquierdo antes de llegar aqui: en la
        // captura vale lo mismo (el gesto de Alt + derecho).
        | EventoOverlay::BotonDerechoPulsado(_)
        | EventoOverlay::BotonCentralSoltado(_) => Continuar::Si,
    }
}

/// La forma del cursor: la flecha sobre los botones, la cruz o la barra de
/// escribir al anotar, y la del estado en lo demas.
fn forma_del_cursor(
    p: Punto,
    estado: &EstadoOverlay,
    sesion: &Sesion,
    piezas: &[Pieza],
    modo: ModoConfirmacion,
) -> FormaCursorWin {
    if que_hay(p, estado, sesion, piezas, modo) != Bajo::Nada {
        return FormaCursorWin::Flecha;
    }
    if vista_de(estado, sesion, modo) == Vista::Despues {
        let util = sesion.anotacion.as_ref().map_or(sesion.util, |a| a.util);
        if estado.seleccion().contiene(p) {
            match util {
                Some(Util::Texto) => return FormaCursorWin::Texto,
                Some(_) => return FormaCursorWin::Cruz,
                None => {}
            }
        }
        if sesion.anotada() {
            return FormaCursorWin::Flecha;
        }
    }
    match estado.forma_cursor() {
        FormaCursor::Cruz => FormaCursorWin::Cruz,
        FormaCursor::Mover => FormaCursorWin::Mover,
        FormaCursor::RedimNS => FormaCursorWin::RedimNS,
        FormaCursor::RedimEO => FormaCursorWin::RedimEO,
        FormaCursor::RedimNeSo => FormaCursorWin::RedimNeSo,
        FormaCursor::RedimNoSe => FormaCursorWin::RedimNoSe,
    }
}

/// Una tecla pulsada. El orden decide quien se la queda: primero lo que se
/// esta escribiendo (un campo, el selector, un texto), despues las barras,
/// y al final el estado de siempre.
#[allow(clippy::too_many_arguments)]
fn tecla(
    vk: u32,
    shift: bool,
    ctrl: bool,
    estado: &mut EstadoOverlay,
    sesion: &mut Sesion,
    muestra_color: &[u8; 4],
    piezas: &mut [Pieza],
    ctx: &Ctx,
) -> Continuar {
    let modo = ctx.modo;
    let letra = |c: u8| vk == u32::from(c);

    // Cuentagotas: Enter o C copian como el clic; Escape cancela.
    if matches!(modo, ModoConfirmacion::Cuentagotas) {
        return match vk {
            VK_RETURN => {
                copiar_color(sesion.formato, *muestra_color);
                Continuar::No
            }
            _ if letra(b'C') && !ctrl => {
                copiar_color(sesion.formato, *muestra_color);
                Continuar::No
            }
            VK_ESCAPE => Continuar::No,
            _ => Continuar::Si,
        };
    }

    // 1. Un campo de medida: las cifras llegan por `Caracter`.
    if let Some(campo) = sesion.campo {
        match vk {
            VK_ESCAPE => {
                sesion.campo = None;
                sesion.ancho = d2::Cifras::default();
                sesion.alto = d2::Cifras::default();
            }
            VK_BACK => {
                match campo {
                    Campo::Ancho => sesion.ancho.borrar(),
                    Campo::Alto => sesion.alto.borrar(),
                };
            }
            VK_TAB => {
                sesion.campo = match campo {
                    Campo::Ancho => Some(Campo::Alto),
                    Campo::Alto => Some(Campo::Ancho),
                }
            }
            VK_RETURN => aplicar_medidas(estado, sesion, piezas),
            _ => {}
        }
        return Continuar::Si;
    }

    // 2. El selector de proyecto.
    if let Some(es) = sesion.selector.as_mut() {
        let n = es.visibles(&sesion.proyectos).len();
        match vk {
            VK_ESCAPE => sesion.selector = None,
            VK_RETURN => return enviar(estado, sesion),
            VK_UP => es.mover(-1, n),
            VK_DOWN => es.mover(1, n),
            VK_TAB => es.en_comentario = !es.en_comentario,
            VK_BACK => es.borrar(),
            _ => {}
        }
        return Continuar::Si;
    }

    // 3. Un texto de la anotacion: las letras llegan por `Caracter`.
    if let Some(a) = sesion.anotacion.as_mut().filter(|a| a.escribiendo()) {
        if vk == VK_ESCAPE {
            a.cerrar_texto();
        } else {
            a.tecla_de_texto(vk);
        }
        return Continuar::Si;
    }

    let v = vista_de(estado, sesion, modo);

    // 4. Despues de elegir.
    if v == Vista::Despues {
        if ctrl && shift && letra(b'S') {
            PENDIENTE.poner(QueAccion::GuardarComo, estado.seleccion());
            return Continuar::No;
        }
        if ctrl && (letra(b'Z') || letra(b'Y')) {
            if let Some(a) = sesion.anotacion.as_mut() {
                if letra(b'Z') {
                    a.deshacer();
                } else {
                    a.rehacer();
                }
            }
            return Continuar::Si;
        }
        if let Some(a) = Accion::de_tecla(vk, ctrl, shift) {
            return hacer_accion(a, estado, sesion);
        }
        if !ctrl {
            if let Some(u) = Util::de_tecla(vk) {
                let sel = estado.seleccion();
                let escala = monitor_de_region(piezas, sel).escala_por_cien;
                sesion.tomar(u, sel, escala);
                return Continuar::Si;
            }
        }
        if vk == VK_DELETE {
            if let Some(a) = sesion.anotacion.as_mut() {
                a.suprimir();
            }
            return Continuar::Si;
        }
        if sesion.anotada() {
            // La zona ya no se mueve ni cambia: el resto no hace nada.
            return Continuar::Si;
        }
    }

    // 5. Las letras de la barra de modos.
    if sesion.activa && !ctrl && matches!(v, Vista::Elegir | Vista::Confirmar) {
        if let Some(m) = Modo::de_tecla(vk) {
            cambiar_modo(m, estado, sesion, piezas, ctx);
            return Continuar::Si;
        }
        if letra(b'R') && sesion.contexto.ultima_region.is_some() {
            repetir(estado, sesion);
            return Continuar::Si;
        }
    }

    // C copia el color de la lupa mientras se elige.
    if !ctrl && letra(b'C') && matches!(estado.fase(), Fase::Explorando | Fase::Trazando) {
        copiar_color(sesion.formato, *muestra_color);
        return Continuar::No;
    }

    // Ctrl+A: la pantalla entera bajo el cursor, lista para confirmar. Mismo
    // camino que el boton del panel.
    if ctrl && letra(b'A') {
        let _ = estado.procesar(EventoEntrada::Tecla(TeclaOverlay::SeleccionarTodo));
        return Continuar::Si;
    }

    let paso = if shift { 10 } else { 1 };
    let t = match vk {
        VK_ESCAPE => Some(TeclaOverlay::Escape),
        VK_RETURN => Some(TeclaOverlay::Enter),
        VK_SPACE => Some(TeclaOverlay::Espacio),
        VK_LEFT => Some(TeclaOverlay::Flecha { dx: -paso, dy: 0 }),
        VK_RIGHT => Some(TeclaOverlay::Flecha { dx: paso, dy: 0 }),
        VK_UP => Some(TeclaOverlay::Flecha { dx: 0, dy: -paso }),
        VK_DOWN => Some(TeclaOverlay::Flecha { dx: 0, dy: paso }),
        _ => None,
    };
    match t {
        Some(t) => {
            let efecto = estado.procesar(EventoEntrada::Tecla(t));
            aplicar_efecto(efecto, estado, sesion, piezas, ctx)
        }
        None => Continuar::Si,
    }
}

/// El cuentagotas (D78): el color bajo el cursor, en el formato configurado,
/// al portapapeles. Un fallo del portapapeles se registra y se cierra igual.
fn copiar_color(formato: FormatoColorLupa, muestra: [u8; 4]) {
    let texto = texto_color(formato, muestra);
    match pixpin_codec::copiar_texto(&texto) {
        Ok(()) => tracing::info!(color = %texto, "color copiado"),
        Err(e) => tracing::warn!(?e, color = %texto, "no se pudo copiar el color"),
    }
}

fn aplicar_efecto(
    efecto: Efecto,
    estado: &mut EstadoOverlay,
    sesion: &mut Sesion,
    piezas: &mut [Pieza],
    ctx: &Ctx,
) -> Continuar {
    match efecto {
        Efecto::Nada => Continuar::Si,
        Efecto::Redibujar => {
            for p in piezas {
                p.ventana().invalidar();
            }
            Continuar::Si
        }
        Efecto::AlternarVivo => {
            if estado.vivo() {
                // Abrir una sesion WGC por monitor. El tope de FPS es el
                // primer consumidor real del nivel (D14/5.2): Completo sin
                // tope, Ligero a 30 fps — sobre una iGPU compartida,
                // refrescar a 60 Hz roba lo que la captura final necesita.
                let tope = match ctx.nivel {
                    Nivel::Completo => std::time::Duration::ZERO,
                    Nivel::Ligero => std::time::Duration::from_millis(33),
                };
                for p in piezas.iter_mut() {
                    let aviso = Some((
                        p.ventana().handle().0 as isize,
                        pixpin_shell::overlay::MSG_DESPIERTA,
                    ));
                    match SesionViva::nueva(ctx.dispositivo, p.monitor().id, tope, aviso) {
                        Ok(s) => p.sesion = Some(s),
                        Err(e) => {
                            tracing::warn!(?e, "sin sesion en vivo; el monitor queda congelado");
                        }
                    }
                }
            } else {
                // Volver a congelado: cerrar sesiones y soltar los fondos
                // vivos. La instantanea original sigue siendo el fondo.
                for p in piezas.iter_mut() {
                    if let Some(s) = p.sesion.take() {
                        // El total de fotogramas aceptados queda en el log:
                        // es como se verifica el tope de 30 en Ligero.
                        tracing::info!(aceptados = s.aceptados(), "sesion en vivo cerrada");
                        s.cerrar();
                    }
                    p.fondo_vivo = None;
                }
            }
            for p in piezas {
                p.ventana().invalidar();
            }
            Continuar::Si
        }
        Efecto::Cancelar => Continuar::No,
        Efecto::Confirmar(region) => match sesion.confirmacion(ctx.modo) {
            ModoConfirmacion::DirectoAlPortapapeles => {
                PENDIENTE.poner(QueAccion::Copiar, region);
                Continuar::No
            }
            ModoConfirmacion::Pinear => {
                PENDIENTE.poner(QueAccion::Pinear, region);
                Continuar::No
            }
            ModoConfirmacion::Gif => {
                PENDIENTE.poner(QueAccion::Gif, region);
                Continuar::No
            }
            ModoConfirmacion::Texto => {
                PENDIENTE.poner(QueAccion::Texto, region);
                Continuar::No
            }
            ModoConfirmacion::Scroll => {
                PENDIENTE.poner(QueAccion::Scroll, region);
                Continuar::No
            }
            ModoConfirmacion::PinEnVivo => {
                PENDIENTE.poner(QueAccion::PinEnVivo, region);
                Continuar::No
            }
            // El cuentagotas no confirma regiones: su clic se resuelve al
            // pulsar, antes de llegar aqui.
            ModoConfirmacion::Cuentagotas => Continuar::No,
            // Confirmar con «preguntar» deja la zona elegida: las barras de
            // despues salen solas en cuanto la zona esta lista. Enter con
            // la zona ya lista es Copiar (lo atiende `tecla` antes).
            ModoConfirmacion::ConBarra => {
                if estado.fase() == Fase::Lista && estado.seleccion() == region {
                    PENDIENTE.poner(QueAccion::Copiar, region);
                    return Continuar::No;
                }
                let _ = estado.poner_seleccion(region);
                for p in piezas {
                    p.ventana().invalidar();
                }
                Continuar::Si
            }
        },
    }
}

/// Dibuja el fotograma completo de una pieza.
fn pintar(
    pieza: &Pieza,
    estado: &EstadoOverlay,
    sesion: &mut Sesion,
    muestra_color: [u8; 4],
    ctx: &Ctx,
) {
    let motor = ctx.motor;
    let monitor = pieza.monitor().area;
    let escala = pieza.monitor().escala_por_cien as f32 / 100.0;
    let l = p2::Local {
        ox: monitor.x,
        oy: monitor.y,
        e: escala,
    };
    let v = vista_de(estado, sesion, ctx.modo);
    let sel = estado.seleccion();
    // Lo anotado necesita su foto tapada antes de pintar (fuera del
    // fotograma: sube un bitmap).
    if v == Vista::Despues && monitor.interseccion(sel) == Some(sel) {
        if let Some(a) = sesion.anotacion.as_mut() {
            a.preparar(motor, || {
                let rec = pieza.instantanea.recortar(ctx.dispositivo, sel).ok()?;
                a_imagen(ctx.dispositivo, &rec).ok()
            });
        }
    }
    let Ok(destino) = pieza.base.superficie.empezar(motor) else {
        return;
    };
    let fondo = pieza.fondo_vivo.as_ref().unwrap_or(&pieza.fondo);
    let cursor = estado.cursor();
    let en_este = monitor.contiene(cursor);
    let modos_aqui = matches!(v, Vista::Elegir | Vista::Confirmar)
        && monitor_en(std::slice::from_ref(pieza), cursor).id == pieza.monitor().id
        && en_este;

    let _ = motor.dibujar(&destino, |p| {
        let todo = RectF {
            x: 0.0,
            y: 0.0,
            ancho: monitor.ancho as f32,
            alto: monitor.alto as f32,
        };
        p.bitmap(fondo, todo, None, false);

        let seleccion_local = if estado.fase() == Fase::Explorando {
            None
        } else {
            parte_local(sel, monitor)
        };

        // El velo: cuatro rectangulos alrededor del hueco (la seleccion, o
        // lo resaltado con la barra nueva), o entero.
        let hueco = match seleccion_local {
            Some(s) if !s.esta_vacio() => Some(s),
            Some(_) => None,
            None if sesion.activa => estado
                .rect_resaltado()
                .and_then(|r| parte_local(r, monitor))
                .filter(|r| !r.esta_vacio()),
            None => None,
        };
        let velo = Color::oscurecido();
        match hueco {
            Some(s) => {
                let (sx, sy) = (s.x as f32, s.y as f32);
                let (sw, sh) = (s.ancho as f32, s.alto as f32);
                for r in [
                    RectF {
                        x: 0.0,
                        y: 0.0,
                        ancho: todo.ancho,
                        alto: sy,
                    },
                    RectF {
                        x: 0.0,
                        y: sy + sh,
                        ancho: todo.ancho,
                        alto: todo.alto - sy - sh,
                    },
                    RectF {
                        x: 0.0,
                        y: sy,
                        ancho: sx,
                        alto: sh,
                    },
                    RectF {
                        x: sx + sw,
                        y: sy,
                        ancho: todo.ancho - sx - sw,
                        alto: sh,
                    },
                ] {
                    p.rellenar(r, velo);
                }
            }
            None => p.rellenar(todo, velo),
        }

        match seleccion_local {
            Some(s) if !s.esta_vacio() => {
                let (sx, sy) = (s.x as f32, s.y as f32);
                let (sw, sh) = (s.ancho as f32, s.alto as f32);
                let caja = RectF {
                    x: sx,
                    y: sy,
                    ancho: sw,
                    alto: sh,
                };
                // Lo anotado, encima de la foto y dentro de la zona entera
                // (aunque cruce monitores: la ventana recorta lo que sobra).
                if v == Vista::Despues {
                    if let Some(a) = sesion.anotacion.as_mut() {
                        let zona = RectF {
                            x: (sel.x - monitor.x) as f32,
                            y: (sel.y - monitor.y) as f32,
                            ancho: sel.ancho as f32,
                            alto: sel.alto as f32,
                        };
                        a.pintar(p, zona);
                    }
                }
                p.trazar(caja, 2.0 * escala, Color::ACENTO);
                if sesion.activa {
                    // Con algo dibujado la zona ya no se toca: sin tiradores.
                    if !sesion.anotada() {
                        p2::pintar_tiradores(p, caja, escala);
                    }
                } else {
                    let lado = 8.0 * escala;
                    for (tx, ty) in [
                        (sx, sy),
                        (sx + sw / 2.0, sy),
                        (sx + sw, sy),
                        (sx + sw, sy + sh / 2.0),
                        (sx + sw, sy + sh),
                        (sx + sw / 2.0, sy + sh),
                        (sx, sy + sh),
                        (sx, sy + sh / 2.0),
                    ] {
                        let cuadro = RectF {
                            x: tx - lado / 2.0,
                            y: ty - lado / 2.0,
                            ancho: lado,
                            alto: lado,
                        };
                        p.rellenar(cuadro, Color::BLANCO);
                        p.trazar(cuadro, 1.0 * escala, Color::ACENTO);
                    }
                }

                // Dimensiones y coordenadas, la spec pide ambas. Con las
                // barras de despues ya van en la de anotar.
                if v != Vista::Despues {
                    let etiqueta =
                        format!("{}\u{d7}{} ({}, {})", sel.ancho, sel.alto, sel.x, sel.y);
                    let tam = 14.0 * escala;
                    let ty = if sy > tam * 2.5 {
                        sy - tam * 2.2
                    } else {
                        sy + 4.0 * escala
                    };
                    p.texto_con_fondo(
                        &etiqueta,
                        sx,
                        ty,
                        tam,
                        Color::BLANCO,
                        Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.7,
                        },
                    );
                }
            }
            _ => {
                if let Some(res) = estado.rect_resaltado() {
                    if let Some(local) = parte_local(res, monitor) {
                        if sesion.activa {
                            // Lo resaltado, como en la maqueta: borde azul
                            // firme y un velo azul muy suave.
                            let r = a_rectf(local);
                            p.rellenar(
                                r,
                                Color {
                                    r: 0.04,
                                    g: 0.52,
                                    b: 1.0,
                                    a: 0.08,
                                },
                            );
                            p.trazar(r, 3.0 * escala, p2::AZUL);
                        } else {
                            // El resaltado del snap, discontinuo.
                            p.trazar_discontinuo(a_rectf(local), 2.0 * escala, Color::ACENTO);
                        }
                    }
                }
            }
        }

        // El panel «Seleccionar todo», mientras no hay seleccion, sin la
        // barra nueva y sin el cuentagotas (ahi no hay nada que seleccionar).
        if !sesion.activa
            && estado.fase() == Fase::Explorando
            && !matches!(ctx.modo, ModoConfirmacion::Cuentagotas)
        {
            let panel = PanelTodo::colocar(pieza.monitor().area, pieza.monitor().escala_por_cien);
            if let Some(local) = parte_local(panel.rect, monitor) {
                let caja = a_rectf(local);
                p.rellenar_redondeado(
                    caja,
                    8.0 * escala,
                    Color {
                        r: 0.12,
                        g: 0.12,
                        b: 0.14,
                        a: 0.92,
                    },
                );
                let tam = 13.0 * escala;
                let (tw, th) = p.medir_texto(&ctx.textos.todo, tam);
                p.texto(
                    &ctx.textos.todo,
                    caja.x + (caja.ancho - tw) / 2.0,
                    caja.y + (caja.alto - th) / 2.0,
                    tam,
                    Color::BLANCO,
                );
            }
        }

        let t2 = &sesion.contexto.textos;
        let medidas_ahora = medidas(estado);

        // Lo de elegir: pistas arriba, etiqueta de lo resaltado, barra de
        // modos abajo, en el monitor del raton.
        let mut sobre_barra = false;
        if modos_aqui {
            let m = pieza.monitor();
            if v == Vista::Elegir && sesion.contexto.gestos {
                p2::pintar_pistas(
                    p,
                    l,
                    d2::fila_de_pistas(m.area_trabajo, m.escala_por_cien),
                    t2,
                );
            }
            if v == Vista::Elegir {
                if let Some(res) = estado.rect_resaltado() {
                    let titulo = match sesion.modo {
                        Modo::Pantalla => String::new(),
                        _ => sesion
                            .ventana_en(cursor)
                            .map(|w| w.titulo.clone())
                            .unwrap_or_default(),
                    };
                    let pista = if sesion.modo == Modo::Ventana {
                        &t2.pista_ventana
                    } else {
                        &t2.pista_zona
                    };
                    let ancho =
                        p2::ancho_etiqueta(p, escala, &titulo, (res.ancho, res.alto), pista);
                    let r = d2::etiqueta_de_ventana(res, ancho, m.area, m.escala_por_cien);
                    p2::pintar_etiqueta(p, l, r, &titulo, (res.ancho, res.alto), pista);
                }
            }
            let (b, _) = barra_modos(estado, sesion, std::slice::from_ref(pieza));
            sobre_barra = b.panel.contiene(cursor);
            p2::pintar_modos(p, l, &b, sesion, t2, cursor, medidas_ahora);
        }
        if v == Vista::Confirmar {
            let (r, m) = boton_confirmar(estado, std::slice::from_ref(pieza));
            if m.id == pieza.monitor().id && monitor.interseccion(sel).is_some() {
                p2::pintar_confirmar(p, l, r, t2.modo(sesion.modo), cursor);
            }
        }
        if v == Vista::Despues {
            let (a, b, m) = barras_despues(estado, std::slice::from_ref(pieza));
            if m.id == pieza.monitor().id && monitor.interseccion(sel).is_some() {
                p2::pintar_despues(p, l, &a, &b, sesion, t2, cursor, medidas_ahora);
                if let (Some(s), Some(es)) = (selector_de(sesion, &b, &m), sesion.selector.as_ref())
                {
                    let ultimo = crate::captura2::al_chat::ultimo();
                    p2::pintar_selector(
                        p,
                        l,
                        &s,
                        es,
                        &sesion.proyectos,
                        ultimo.as_deref(),
                        t2,
                        cursor,
                    );
                }
            }
        }

        // La lupa, si el cursor esta en este monitor, eligiendo y no encima
        // de la barra.
        let con_lupa = matches!(
            estado.fase(),
            Fase::Explorando | Fase::Trazando | Fase::Redimensionando
        ) || !sesion.activa;
        if en_este && con_lupa && !sobre_barra && v != Vista::Despues {
            if sesion.activa || matches!(ctx.modo, ModoConfirmacion::Cuentagotas) {
                let lupa = PanelLupa::colocar(cursor, monitor, pieza.monitor().escala_por_cien);
                let region = lupa.region(cursor, monitor);
                let fuente = a_rectf(parte_local(region, monitor).unwrap_or(region));
                p2::pintar_lupa(
                    p,
                    l,
                    &lupa,
                    &pieza.fondo,
                    fuente,
                    cursor,
                    muestra_color,
                    sesion,
                    t2,
                );
            } else {
                pintar_lupa_vieja(p, pieza, cursor, muestra_color, sesion.formato, escala);
            }
        }
    });
    let _ = pieza.base.superficie.presentar();
}

/// La lupa redonda de antes, para los gestos con Alt: mas pequena, porque
/// ahi la mano ya va arrastrando y no hay nada que leer.
fn pintar_lupa_vieja(
    p: &pixpin_render::Pintor,
    pieza: &Pieza,
    cursor: Punto,
    muestra_color: [u8; 4],
    formato: FormatoColorLupa,
    escala: f32,
) {
    let monitor = pieza.monitor().area;
    let lupa = pixpin_ui::Lupa::por_defecto(pieza.monitor().escala_por_cien);
    let fuente_global = lupa.region_fuente(cursor, monitor);
    let fuente_local = parte_local(fuente_global, monitor).unwrap_or(fuente_global);
    let pos = lupa.colocar(cursor, monitor);
    let d = lupa.diametro as f32;
    let destino_lupa = RectF {
        x: (pos.x - monitor.x) as f32,
        y: (pos.y - monitor.y) as f32,
        ancho: d,
        alto: d,
    };
    p.bitmap(
        &pieza.fondo,
        destino_lupa,
        Some(a_rectf(fuente_local)),
        true,
    );
    p.trazar(destino_lupa, 2.0 * escala, Color::ACENTO);
    let cx = destino_lupa.x + d / 2.0;
    let cy = destino_lupa.y + d / 2.0;
    let cruz = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.6,
    };
    p.linea((cx, destino_lupa.y), (cx, destino_lupa.y + d), 1.0, cruz);
    p.linea((destino_lupa.x, cy), (destino_lupa.x + d, cy), 1.0, cruz);
    let texto = texto_color(formato, muestra_color);
    p.texto_con_fondo(
        &texto,
        destino_lupa.x + 6.0 * escala,
        destino_lupa.y + d + 6.0 * escala,
        12.0 * escala,
        Color::BLANCO,
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.7,
        },
    );
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_gesto_arranca_siempre_en_la_pulsacion_y_no_donde_esta_el_cursor() {
        let p = Punto { x: -1887, y: 449 };
        let r = Punto { x: -1500, y: 700 };
        assert_eq!(
            arranque_del_gesto(Some(p), true, None),
            ArranqueGesto::Arrastrando(p)
        );
        // Lo que fallaba (2-oct): el overlay salia con el boton ya suelto y
        // abria en exploracion; el clic siguiente empezaba en otro sitio.
        assert_eq!(
            arranque_del_gesto(Some(p), false, Some(r)),
            ArranqueGesto::YaSoltado { desde: p, hasta: r }
        );
        // Una soltada vieja no gana a un boton que sigue abajo.
        assert_eq!(
            arranque_del_gesto(Some(p), true, Some(r)),
            ArranqueGesto::Arrastrando(p)
        );
    }

    #[test]
    fn sin_gesto_o_sin_soltada_conocida_el_overlay_abre_en_exploracion() {
        // Caso negativo: un atajo no reproduce nada aunque haya soltadas.
        let r = Punto { x: 5, y: 5 };
        assert_eq!(
            arranque_del_gesto(None, true, Some(r)),
            ArranqueGesto::Ninguno
        );
        assert_eq!(
            arranque_del_gesto(Some(Punto { x: 1, y: 1 }), false, None),
            ArranqueGesto::Ninguno
        );
        assert!(eventos_de_arranque(ArranqueGesto::Ninguno).is_empty());
    }

    #[test]
    fn un_arrastre_ya_soltado_se_reproduce_entero_y_en_orden() {
        let desde = Punto { x: 10, y: 20 };
        let hasta = Punto { x: 300, y: 200 };
        let ev = eventos_de_arranque(ArranqueGesto::YaSoltado { desde, hasta });
        assert_eq!(ev.len(), 4);
        assert!(matches!(ev[0], EventoOverlay::RatonMovido(q) if q == desde));
        assert!(matches!(ev[1], EventoOverlay::BotonPulsado(q) if q == desde));
        assert!(matches!(ev[2], EventoOverlay::RatonMovido(q) if q == hasta));
        assert!(matches!(ev[3], EventoOverlay::BotonSoltado(q) if q == hasta));
    }

    #[test]
    fn arrastrando_el_color_se_muestrea_con_freno_y_quieto_siempre() {
        assert!(toca_muestrear(Fase::Explorando));
        assert!(toca_muestrear(Fase::Explorando), "explorando, cada vez");
        assert!(
            !toca_muestrear(Fase::Trazando),
            "justo despues de una muestra, trazando no se repite"
        );
        std::thread::sleep(std::time::Duration::from_millis(
            MUESTRA_COLOR_ARRASTRANDO_MS as u64 + 5,
        ));
        assert!(toca_muestrear(Fase::Trazando), "pasado el freno, si");
    }

    #[test]
    fn una_seleccion_que_cruza_dos_monitores_se_reparte_bien() {
        let izquierdo = Rect {
            x: -1920,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        let derecho = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        let seleccion = Rect {
            x: -100,
            y: 100,
            ancho: 300,
            alto: 200,
        };

        let en_izq = parte_local(seleccion, izquierdo).unwrap();
        let en_der = parte_local(seleccion, derecho).unwrap();
        // En el monitor izquierdo (que empieza en -1920), x local = 1820.
        assert_eq!(
            en_izq,
            Rect {
                x: 1820,
                y: 100,
                ancho: 100,
                alto: 200
            }
        );
        assert_eq!(
            en_der,
            Rect {
                x: 0,
                y: 100,
                ancho: 200,
                alto: 200
            }
        );
        // Caso negativo: un monitor que no toca la seleccion no dibuja nada.
        let tercero = Rect {
            x: 0,
            y: -1080,
            ancho: 1920,
            alto: 1080,
        };
        assert_eq!(parte_local(seleccion, tercero), None);
    }
}
