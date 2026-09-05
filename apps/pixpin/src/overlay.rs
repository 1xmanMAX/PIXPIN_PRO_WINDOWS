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

use anyhow::{Context, Result};
use pixpin_capture::{
    Dispositivo, Duplicador, Instantanea, SesionViva, a_imagen, capturar_monitor, componer_region,
    enumerar_monitores,
};
use pixpin_codec::ImagenRgba;
use pixpin_geom::{Monitor, Punto, Rect};
use pixpin_nivel::Nivel;
use pixpin_render::{Color, MotorRender, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin, VentanaOverlay, bucle_modal};
use pixpin_shell::uia::Uia;
use pixpin_shell::ventana::Continuar;
use pixpin_ui::{
    AccionBarra, Barra, Efecto, EstadoOverlay, EventoEntrada, Fase, FormaCursor, FormatoColorLupa,
    Lupa, PanelTodo, TeclaOverlay, texto_color,
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
}

/// Lo que el overlay decidio. La imagen ya esta recortada y en CPU.
pub enum AccionFinal {
    Copiar(ImagenRgba),
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
    Nada,
}

/// Etiquetas ya traducidas: el overlay no conoce el catalogo.
pub struct TextosBarra {
    pub copiar: String,
    pub guardar: String,
    pub guardar_como: String,
    pub descartar: String,
    /// El boton del panel antes de seleccionar: la pantalla entera.
    pub todo: String,
}

impl TextosBarra {
    fn de(&self, accion: AccionBarra) -> &str {
        match accion {
            AccionBarra::Copiar => &self.copiar,
            AccionBarra::Guardar => &self.guardar,
            AccionBarra::GuardarComo => &self.guardar_como,
            AccionBarra::Descartar => &self.descartar,
        }
    }
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
                        tracing::debug!(?e, "sin duplicador; congelando por WGC");
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
                    tracing::debug!(?e, "duplicador sin fotograma; congelando por WGC");
                    break;
                }
            }
        }
        capturar_monitor(&self.dispositivo, m.id, m.area)
            .with_context(|| format!("no se pudo capturar el monitor {}", m.id))
    }
}

pub fn ejecutar_overlay(
    recursos: &mut Recursos,
    nivel: Nivel,
    modo: ModoConfirmacion,
    textos: &TextosBarra,
    formato_color: FormatoColorLupa,
    inicio: Option<Punto>,
) -> Result<AccionFinal> {
    let t0 = Instant::now();

    // 1. Congelar TODOS los monitores antes de ensenar ventana alguna.
    let disposicion = enumerar_monitores().context("sin monitores")?;
    let mut capturas: Vec<Instantanea> = Vec::new();
    for m in disposicion.monitores() {
        capturas.push(recursos.congelar(m)?);
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
    let mut estado = EstadoOverlay::nuevo(disposicion.clone());
    let uia = Uia::nueva(piezas[0].ventana().handle());
    let mut barra: Option<Barra> = None;
    let mut muestra_color: [u8; 4] = [0, 0, 0, 255];

    // Pintar ANTES de mostrar: una ventana retenida ensenaria el fotograma
    // de la captura anterior durante un instante.
    let t_a = t0.elapsed().as_millis() as u64;
    for p in &piezas {
        pintar(
            p,
            &estado,
            None,
            muestra_color,
            motor,
            textos,
            formato_color,
            modo,
        );
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
        captura_ms = t_captura,
        prep_ms = t_a - t_motor,
        pintar_ms = t_b - t_a,
        mostrar_ms = t0.elapsed().as_millis() as u64 - t_b,
        "overlay visible"
    );
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
    let gesto = inicio.is_some();
    let arrastrando =
        gesto && (pixpin_shell::gesto_en_curso() || pixpin_shell::boton_del_raton_pulsado());
    if let Some(p) = inicio.filter(|_| arrastrando) {
        let pieza = piezas
            .iter()
            .position(|z| z.monitor().area.contiene(p))
            .unwrap_or(0);
        let hwnd0 = piezas[pieza].ventana().handle();
        piezas[pieza].ventana().capturar_raton();
        for evento in [
            EventoOverlay::RatonMovido(p),
            EventoOverlay::BotonPulsado(p),
        ] {
            procesar_evento(
                hwnd0,
                evento,
                &mut estado,
                &mut barra,
                &mut muestra_color,
                &mut piezas,
                &uia,
                dispositivo,
                motor,
                nivel,
                modo,
                textos,
                formato_color,
                gesto,
            );
        }
    }

    // 4. El bucle modal. Las ventanas viven en `piezas`; el slice del
    //    contrato queda vacio porque el bombeo no filtra por ventana.
    bucle_modal(&[], |hwnd, evento| {
        procesar_evento(
            hwnd,
            evento,
            &mut estado,
            &mut barra,
            &mut muestra_color,
            &mut piezas,
            &uia,
            dispositivo,
            motor,
            nivel,
            modo,
            textos,
            formato_color,
            gesto,
        )
    });

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
        Some((que, region)) => {
            let recorte = componer_region(dispositivo, &fuentes, region)
                .context("no se pudo recortar la seleccion")?;
            let imagen =
                a_imagen(dispositivo, &recorte).context("no se pudo bajar la seleccion a CPU")?;
            Ok(match que {
                QueAccion::Copiar => AccionFinal::Copiar(imagen),
                QueAccion::Texto => AccionFinal::Texto(imagen),
                QueAccion::Guardar => AccionFinal::Guardar(imagen),
                QueAccion::GuardarComo => AccionFinal::GuardarComo(imagen),
                QueAccion::Pinear => AccionFinal::Pinear { imagen, region },
                QueAccion::Scroll => AccionFinal::Scroll { region },
                QueAccion::Gif => AccionFinal::Gif { region },
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
    Scroll,
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

#[allow(clippy::too_many_arguments)]
fn procesar_evento(
    hwnd: HWND,
    evento: EventoOverlay,
    estado: &mut EstadoOverlay,
    barra: &mut Option<Barra>,
    muestra_color: &mut [u8; 4],
    piezas: &mut [Pieza],
    uia: &Uia,
    dispositivo: &Dispositivo,
    motor: &MotorRender,
    nivel: Nivel,
    modo: ModoConfirmacion,
    textos: &TextosBarra,
    formato_color: FormatoColorLupa,
    gesto: bool,
) -> Continuar {
    let invalidar_todas = |piezas: &[Pieza]| {
        for p in piezas {
            p.ventana().invalidar();
        }
    };

    match evento {
        EventoOverlay::RatonMovido(p) => {
            uia.pedir(p);
            // El color bajo el cursor: un recorte de 1x1 y su bajada. Es
            // minusculo (4 bytes) y solo ocurre al mover el raton.
            if let Some(pieza) = piezas.iter().find(|z| z.monitor().area.contiene(p)) {
                let uno = Rect {
                    x: p.x,
                    y: p.y,
                    ancho: 1,
                    alto: 1,
                };
                if let Ok(rec) = pieza.instantanea.recortar(dispositivo, uno) {
                    if let Ok(img) = a_imagen(dispositivo, &rec) {
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
            let forma = match estado.forma_cursor() {
                FormaCursor::Cruz => FormaCursorWin::Cruz,
                FormaCursor::Mover => FormaCursorWin::Mover,
                FormaCursor::RedimNS => FormaCursorWin::RedimNS,
                FormaCursor::RedimEO => FormaCursorWin::RedimEO,
                FormaCursor::RedimNeSo => FormaCursorWin::RedimNeSo,
                FormaCursor::RedimNoSe => FormaCursorWin::RedimNoSe,
            };
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
                copiar_color(formato_color, *muestra_color);
                return Continuar::No;
            }
            // El panel «Seleccionar todo», solo mientras no hay seleccion.
            if estado.fase() == Fase::Explorando {
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
            if let Some(b) = barra {
                if b.origen.contiene(p) {
                    // El clic se resuelve al soltar; tragarse el pulsado
                    // evita que el estado empiece un trazado bajo la barra.
                    return Continuar::Si;
                }
                *barra = None;
            }
            let _ = estado.procesar(EventoEntrada::BotonPulsado(p));
            invalidar_todas(piezas);
            Continuar::Si
        }
        EventoOverlay::BotonSoltado(p) => {
            if let Some(b) = barra {
                if let Some(accion) = b.boton_en(p) {
                    return decidir_accion(accion, estado.seleccion());
                }
                if b.origen.contiene(p) {
                    return Continuar::Si;
                }
            }
            let efecto = estado.procesar(EventoEntrada::BotonSoltado(p));
            let seguir = aplicar_efecto(
                efecto,
                estado,
                barra,
                piezas,
                dispositivo,
                motor,
                nivel,
                modo,
            );
            // En un gesto con Alt (D81) soltar ya es confirmar: no hay
            // segundo paso. Solo si el arrastre dejo una seleccion; un clic
            // sin arrastre deja el overlay abierto para seleccionar a mano.
            if seguir == Continuar::Si && gesto && estado.fase() == Fase::Lista {
                let efecto = estado.procesar(EventoEntrada::Tecla(TeclaOverlay::Enter));
                return aplicar_efecto(
                    efecto,
                    estado,
                    barra,
                    piezas,
                    dispositivo,
                    motor,
                    nivel,
                    modo,
                );
            }
            seguir
        }
        EventoOverlay::Tecla { vk, shift, ctrl } => {
            // Ctrl+A: la pantalla entera bajo el cursor, lista para
            // confirmar. Mismo camino que el boton del panel.
            if ctrl && vk == u32::from(b'A') && !matches!(modo, ModoConfirmacion::Cuentagotas) {
                let _ = estado.procesar(EventoEntrada::Tecla(TeclaOverlay::SeleccionarTodo));
                invalidar_todas(piezas);
                return Continuar::Si;
            }
            // Cuentagotas: Enter copia como el clic; Escape cancela.
            if matches!(modo, ModoConfirmacion::Cuentagotas) {
                match vk {
                    VK_RETURN => {
                        copiar_color(formato_color, *muestra_color);
                        return Continuar::No;
                    }
                    VK_ESCAPE => return Continuar::No,
                    _ => return Continuar::Si,
                }
            }
            if barra.is_some() {
                match vk {
                    VK_RETURN => {
                        return decidir_accion(AccionBarra::Copiar, estado.seleccion());
                    }
                    VK_ESCAPE => return Continuar::No,
                    _ => {}
                }
            }
            let paso = if shift { 10 } else { 1 };
            let tecla = match vk {
                VK_ESCAPE => Some(TeclaOverlay::Escape),
                VK_RETURN => Some(TeclaOverlay::Enter),
                VK_SPACE => Some(TeclaOverlay::Espacio),
                VK_LEFT => Some(TeclaOverlay::Flecha { dx: -paso, dy: 0 }),
                VK_RIGHT => Some(TeclaOverlay::Flecha { dx: paso, dy: 0 }),
                VK_UP => Some(TeclaOverlay::Flecha { dx: 0, dy: -paso }),
                VK_DOWN => Some(TeclaOverlay::Flecha { dx: 0, dy: paso }),
                _ => None,
            };
            match tecla {
                Some(t) => {
                    let efecto = estado.procesar(EventoEntrada::Tecla(t));
                    aplicar_efecto(
                        efecto,
                        estado,
                        barra,
                        piezas,
                        dispositivo,
                        motor,
                        nivel,
                        modo,
                    )
                }
                None => Continuar::Si,
            }
        }
        EventoOverlay::Despierta => {
            // Dos emisores comparten MSG_DESPIERTA: el hilo UIA (candidatos
            // frescos) y las sesiones en vivo (fotograma nuevo). Atender
            // ambos es mas barato que distinguirlos.
            let _ = estado.procesar(EventoEntrada::Candidatos(uia.candidatos()));
            if estado.vivo() {
                for p in piezas.iter_mut() {
                    if let Some(textura) = p.sesion.as_ref().and_then(|s| s.ultimo()) {
                        // Si envolver falla (textura en transito), se ignora:
                        // el proximo fotograma lo reintenta.
                        if let Ok(b) = motor.bitmap_desde_textura(&textura) {
                            p.fondo_vivo = Some(b);
                        }
                    }
                }
            }
            invalidar_todas(piezas);
            Continuar::Si
        }
        EventoOverlay::Pintar => {
            if let Some(pieza) = piezas.iter().find(|z| z.ventana().handle() == hwnd) {
                pintar(
                    pieza,
                    estado,
                    barra.as_ref(),
                    *muestra_color,
                    motor,
                    textos,
                    formato_color,
                    modo,
                );
            }
            Continuar::Si
        }
        EventoOverlay::CambioDpi => Continuar::Si,
        // Alt+F4 sobre el overlay: cancelar limpiamente.
        EventoOverlay::Cerrar => Continuar::No,
        // El overlay de captura no usa la rueda, ni escribe texto, ni
        // reacciona a las teclas soltadas; la capa de anotacion de S3-C si.
        // Un atajo global pulsado con el overlay abierto se descarta: si
        // volviera a la cola principal, reabriria el overlay al cerrarlo.
        EventoOverlay::Rueda(_)
        | EventoOverlay::Caracter(_)
        | EventoOverlay::TeclaSoltada(_)
        | EventoOverlay::Atajo(_) => Continuar::Si,
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

fn decidir_accion(accion: AccionBarra, region: Rect) -> Continuar {
    match accion {
        AccionBarra::Copiar => PENDIENTE.poner(QueAccion::Copiar, region),
        AccionBarra::Guardar => PENDIENTE.poner(QueAccion::Guardar, region),
        AccionBarra::GuardarComo => PENDIENTE.poner(QueAccion::GuardarComo, region),
        AccionBarra::Descartar => {}
    }
    Continuar::No
}

#[allow(clippy::too_many_arguments)] // los brazos comparten el contexto entero del bucle
fn aplicar_efecto(
    efecto: Efecto,
    estado: &mut EstadoOverlay,
    barra: &mut Option<Barra>,
    piezas: &mut [Pieza],
    dispositivo: &Dispositivo,
    _motor: &MotorRender, // simetria con procesar_evento; el vivo usa el del Despierta
    nivel: Nivel,
    modo: ModoConfirmacion,
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
                let tope = match nivel {
                    Nivel::Completo => std::time::Duration::ZERO,
                    Nivel::Ligero => std::time::Duration::from_millis(33),
                };
                for p in piezas.iter_mut() {
                    let aviso = Some((
                        p.ventana().handle().0 as isize,
                        pixpin_shell::overlay::MSG_DESPIERTA,
                    ));
                    match SesionViva::nueva(dispositivo, p.monitor().id, tope, aviso) {
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
        Efecto::Confirmar(region) => match modo {
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
            // El cuentagotas no confirma regiones: su clic se resuelve al
            // pulsar, antes de llegar aqui.
            ModoConfirmacion::Cuentagotas => Continuar::No,
            ModoConfirmacion::ConBarra => {
                let monitor = piezas
                    .iter()
                    .find(|p| p.monitor().area.interseccion(region).is_some())
                    .map(|p| *p.monitor())
                    .unwrap_or(*piezas[0].monitor());
                *barra = Some(Barra::colocar(
                    region,
                    monitor.area_trabajo,
                    monitor.escala_por_cien,
                ));
                let _ = estado;
                for p in piezas {
                    p.ventana().invalidar();
                }
                Continuar::Si
            }
        },
    }
}

/// Dibuja el fotograma completo de una pieza. Solo lectura del estado.
#[allow(clippy::too_many_arguments)] // el fotograma se pinta con todo el contexto del bucle
fn pintar(
    pieza: &Pieza,
    estado: &EstadoOverlay,
    barra: Option<&Barra>,
    muestra_color: [u8; 4],
    motor: &MotorRender,
    textos: &TextosBarra,
    formato_color: FormatoColorLupa,
    modo: ModoConfirmacion,
) {
    let monitor = pieza.monitor().area;
    let escala = pieza.monitor().escala_por_cien as f32 / 100.0;
    let Ok(destino) = pieza.base.superficie.empezar(motor) else {
        return;
    };
    let fondo = pieza.fondo_vivo.as_ref().unwrap_or(&pieza.fondo);

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
            parte_local(estado.seleccion(), monitor)
        };

        // El velo: cuatro rectangulos alrededor de la seleccion (o entero).
        match seleccion_local {
            Some(s) if !s.esta_vacio() => {
                let (sx, sy) = (s.x as f32, s.y as f32);
                let (sw, sh) = (s.ancho as f32, s.alto as f32);
                let velo = Color::oscurecido();
                p.rellenar(
                    RectF {
                        x: 0.0,
                        y: 0.0,
                        ancho: todo.ancho,
                        alto: sy,
                    },
                    velo,
                );
                p.rellenar(
                    RectF {
                        x: 0.0,
                        y: sy + sh,
                        ancho: todo.ancho,
                        alto: todo.alto - sy - sh,
                    },
                    velo,
                );
                p.rellenar(
                    RectF {
                        x: 0.0,
                        y: sy,
                        ancho: sx,
                        alto: sh,
                    },
                    velo,
                );
                p.rellenar(
                    RectF {
                        x: sx + sw,
                        y: sy,
                        ancho: todo.ancho - sx - sw,
                        alto: sh,
                    },
                    velo,
                );

                // Borde y tiradores.
                let grosor = 2.0 * escala;
                p.trazar(
                    RectF {
                        x: sx,
                        y: sy,
                        ancho: sw,
                        alto: sh,
                    },
                    grosor,
                    Color::ACENTO,
                );
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

                // Dimensiones y coordenadas, la spec pide ambas.
                let sel = estado.seleccion();
                let etiqueta = format!("{}\u{d7}{} ({}, {})", sel.ancho, sel.alto, sel.x, sel.y);
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
            _ => {
                p.rellenar(todo, Color::oscurecido());
                // El resaltado del snap, discontinuo.
                if let Some(res) = estado.rect_resaltado() {
                    if let Some(local) = parte_local(res, monitor) {
                        p.trazar_discontinuo(a_rectf(local), 2.0 * escala, Color::ACENTO);
                    }
                }
            }
        }

        // El panel «Seleccionar todo», mientras no hay seleccion y no es el
        // cuentagotas (ahi no hay nada que seleccionar).
        if estado.fase() == Fase::Explorando && !matches!(modo, ModoConfirmacion::Cuentagotas) {
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
                let (tw, th) = p.medir_texto(&textos.todo, tam);
                p.texto(
                    &textos.todo,
                    caja.x + (caja.ancho - tw) / 2.0,
                    caja.y + (caja.alto - th) / 2.0,
                    tam,
                    Color::BLANCO,
                );
            }
        }

        // La barra de resultado.
        if let Some(b) = barra {
            if let Some(local) = parte_local(b.origen, monitor) {
                let desplazado = |r: Rect| RectF {
                    x: (r.x - b.origen.x + local.x) as f32,
                    y: (r.y - b.origen.y + local.y) as f32,
                    ancho: r.ancho as f32,
                    alto: r.alto as f32,
                };
                p.rellenar_redondeado(
                    a_rectf(local),
                    6.0 * escala,
                    Color {
                        r: 0.12,
                        g: 0.12,
                        b: 0.14,
                        a: 0.95,
                    },
                );
                for accion in Barra::ACCIONES {
                    let rb = b.rect_boton(accion);
                    let rl = desplazado(rb);
                    if rb.contiene(estado.cursor()) {
                        p.rellenar_redondeado(
                            rl,
                            4.0 * escala,
                            Color {
                                r: 0.25,
                                g: 0.45,
                                b: 0.75,
                                a: 0.9,
                            },
                        );
                    }
                    let etiqueta = textos.de(accion);
                    let tam = 13.0 * escala;
                    let (tw, th) = p.medir_texto(etiqueta, tam);
                    p.texto(
                        etiqueta,
                        rl.x + (rl.ancho - tw) / 2.0,
                        rl.y + (rl.alto - th) / 2.0,
                        tam,
                        Color::BLANCO,
                    );
                }
            }
        }

        // La lupa, si el cursor esta en este monitor y no hay barra activa.
        let cursor = estado.cursor();
        if barra.is_none() && monitor.contiene(cursor) {
            let lupa = Lupa::por_defecto(pieza.monitor().escala_por_cien);
            let fuente_global = lupa.region_fuente(cursor, monitor);
            let fuente_local = parte_local(fuente_global, monitor).unwrap_or(fuente_global);
            let pos = lupa.colocar(cursor, monitor);
            let pos_local = Punto {
                x: pos.x - monitor.x,
                y: pos.y - monitor.y,
            };
            let d = lupa.diametro as f32;
            let destino_lupa = RectF {
                x: pos_local.x as f32,
                y: pos_local.y as f32,
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
            // Reticula al centro.
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
            // El color bajo el cursor, en el formato configurado.
            let texto = texto_color(formato_color, muestra_color);
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
    });
    let _ = pieza.base.superficie.presentar();
}

#[cfg(test)]
mod pruebas {
    use super::*;

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
