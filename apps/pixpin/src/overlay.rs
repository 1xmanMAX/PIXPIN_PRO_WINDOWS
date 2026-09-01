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
    Dispositivo, Instantanea, SesionViva, a_imagen, capturar_monitor, componer_region,
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
    Lupa, TeclaOverlay, texto_color,
};
use std::time::Instant;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

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
}

/// Lo que el overlay decidio. La imagen ya esta recortada y en CPU.
pub enum AccionFinal {
    Copiar(ImagenRgba),
    Guardar(ImagenRgba),
    GuardarComo(ImagenRgba),
    Nada,
}

/// Etiquetas ya traducidas: el overlay no conoce el catalogo.
pub struct TextosBarra {
    pub copiar: String,
    pub guardar: String,
    pub guardar_como: String,
    pub descartar: String,
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

/// Una ventana de overlay con todo lo suyo.
struct Pieza {
    monitor: Monitor,
    instantanea: Instantanea,
    ventana: VentanaOverlay,
    superficie: Superficie,
    fondo: ID2D1Bitmap1,
    fondo_vivo: Option<ID2D1Bitmap1>,
    sesion: Option<SesionViva>,
}

pub fn ejecutar_overlay(
    nivel: Nivel,
    modo: ModoConfirmacion,
    textos: &TextosBarra,
    formato_color: FormatoColorLupa,
) -> Result<AccionFinal> {
    let t0 = Instant::now();

    // 1. Capturar TODOS los monitores antes de crear ventana alguna.
    let dispositivo = Dispositivo::nuevo().context("sin dispositivo de captura")?;
    let disposicion = enumerar_monitores().context("sin monitores")?;
    let mut capturas: Vec<(Monitor, Instantanea)> = Vec::new();
    for m in disposicion.monitores() {
        let inst = capturar_monitor(&dispositivo, m.id, m.area)
            .with_context(|| format!("no se pudo capturar el monitor {}", m.id))?;
        capturas.push((*m, inst));
    }

    // 2. Motor de dibujo y una ventana por monitor, ya con su textura.
    let motor = MotorRender::nuevo(dispositivo.d3d()).context("sin motor de dibujo")?;
    let mut piezas: Vec<Pieza> = Vec::new();
    for (monitor, instantanea) in capturas {
        let ventana = VentanaOverlay::nueva(monitor.area).context("sin ventana de overlay")?;
        let superficie = Superficie::nueva(
            &motor,
            dispositivo.d3d(),
            ventana.handle(),
            monitor.area.ancho,
            monitor.area.alto,
        )
        .context("sin superficie de composicion")?;
        let fondo = motor
            .bitmap_desde_textura(instantanea.textura())
            .context("no se pudo envolver la captura")?;
        piezas.push(Pieza {
            monitor,
            instantanea,
            ventana,
            superficie,
            fondo,
            fondo_vivo: None,
            sesion: None,
        });
    }

    // 3. Estado puro, snap y mostrar. El primer overlay recibe los avisos.
    let mut estado = EstadoOverlay::nuevo(disposicion.clone());
    let uia = Uia::nueva(piezas[0].ventana.handle());
    let mut barra: Option<Barra> = None;
    let mut muestra_color: [u8; 4] = [0, 0, 0, 255];

    for p in &piezas {
        p.ventana.mostrar();
    }
    // El primero toma el foco: sin esto el overlay es sordo al teclado.
    piezas[0].ventana.enfocar();
    tracing::info!(ms = t0.elapsed().as_millis() as u64, "overlay visible");
    for p in &piezas {
        p.ventana.invalidar();
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
            &piezas,
            &uia,
            &dispositivo,
            &motor,
            nivel,
            modo,
            textos,
            formato_color,
        )
    });

    // 5. Desmontar en orden: sesiones, uia, ventanas (drop de piezas).
    let fuentes: Vec<Instantanea> = {
        let mut f = Vec::new();
        for mut p in piezas {
            if let Some(s) = p.sesion.take() {
                s.cerrar();
            }
            f.push(p.instantanea);
        }
        f
    };
    uia.detener();

    // La imagen se materializa UNA vez, aqui, al final: es el unico punto
    // donde la seleccion cruza a la CPU.
    match PENDIENTE.take() {
        Some((que, region)) => {
            let recorte = componer_region(&dispositivo, &fuentes, region)
                .context("no se pudo recortar la seleccion")?;
            let imagen =
                a_imagen(&dispositivo, &recorte).context("no se pudo bajar la seleccion a CPU")?;
            Ok(match que {
                QueAccion::Copiar => AccionFinal::Copiar(imagen),
                QueAccion::Guardar => AccionFinal::Guardar(imagen),
                QueAccion::GuardarComo => AccionFinal::GuardarComo(imagen),
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
    Guardar,
    GuardarComo,
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
    piezas: &[Pieza],
    uia: &Uia,
    dispositivo: &Dispositivo,
    motor: &MotorRender,
    _nivel: Nivel, // reservado: la integracion del modo vivo lo consume
    modo: ModoConfirmacion,
    textos: &TextosBarra,
    formato_color: FormatoColorLupa,
) -> Continuar {
    let invalidar_todas = |piezas: &[Pieza]| {
        for p in piezas {
            p.ventana.invalidar();
        }
    };

    match evento {
        EventoOverlay::RatonMovido(p) => {
            uia.pedir(p);
            // El color bajo el cursor: un recorte de 1x1 y su bajada. Es
            // minusculo (4 bytes) y solo ocurre al mover el raton.
            if let Some(pieza) = piezas.iter().find(|z| z.monitor.area.contiene(p)) {
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
            for pieza in piezas {
                pieza.ventana.poner_cursor(forma);
            }
            // La lupa sigue al raton: se redibujan todas las que tocan algo.
            invalidar_todas(piezas);
            Continuar::Si
        }
        EventoOverlay::BotonPulsado(p) => {
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
            aplicar_efecto(efecto, estado, barra, piezas, modo)
        }
        EventoOverlay::Tecla { vk, shift } => {
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
                    aplicar_efecto(efecto, estado, barra, piezas, modo)
                }
                None => Continuar::Si,
            }
        }
        EventoOverlay::Despierta => {
            // Hoy el unico emisor de MSG_DESPIERTA es el hilo UIA; el modo
            // vivo (sesiones + fondo_vivo) se integra al final de la fase,
            // sobre las SesionViva ya probadas.
            let _ = estado.procesar(EventoEntrada::Candidatos(uia.candidatos()));
            invalidar_todas(piezas);
            Continuar::Si
        }
        EventoOverlay::Pintar => {
            if let Some(pieza) = piezas.iter().find(|z| z.ventana.handle() == hwnd) {
                pintar(
                    pieza,
                    estado,
                    barra.as_ref(),
                    *muestra_color,
                    motor,
                    textos,
                    formato_color,
                );
            }
            Continuar::Si
        }
        EventoOverlay::CambioDpi => Continuar::Si,
        // Alt+F4 sobre el overlay: cancelar limpiamente.
        EventoOverlay::Cerrar => Continuar::No,
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

fn aplicar_efecto(
    efecto: Efecto,
    estado: &mut EstadoOverlay,
    barra: &mut Option<Barra>,
    piezas: &[Pieza],
    modo: ModoConfirmacion,
) -> Continuar {
    match efecto {
        Efecto::Nada => Continuar::Si,
        Efecto::Redibujar | Efecto::AlternarVivo => {
            // El modo vivo (sesiones + fondo_vivo) se integra al final de la
            // fase sobre las SesionViva ya probadas; el estado ya conmuta.
            for p in piezas {
                p.ventana.invalidar();
            }
            Continuar::Si
        }
        Efecto::Cancelar => Continuar::No,
        Efecto::Confirmar(region) => match modo {
            ModoConfirmacion::DirectoAlPortapapeles => {
                PENDIENTE.poner(QueAccion::Copiar, region);
                Continuar::No
            }
            ModoConfirmacion::ConBarra => {
                let monitor = piezas
                    .iter()
                    .find(|p| p.monitor.area.interseccion(region).is_some())
                    .map(|p| p.monitor)
                    .unwrap_or(piezas[0].monitor);
                *barra = Some(Barra::colocar(
                    region,
                    monitor.area_trabajo,
                    monitor.escala_por_cien,
                ));
                let _ = estado;
                for p in piezas {
                    p.ventana.invalidar();
                }
                Continuar::Si
            }
        },
    }
}

/// Dibuja el fotograma completo de una pieza. Solo lectura del estado.
fn pintar(
    pieza: &Pieza,
    estado: &EstadoOverlay,
    barra: Option<&Barra>,
    muestra_color: [u8; 4],
    motor: &MotorRender,
    textos: &TextosBarra,
    formato_color: FormatoColorLupa,
) {
    let monitor = pieza.monitor.area;
    let escala = pieza.monitor.escala_por_cien as f32 / 100.0;
    let Ok(destino) = pieza.superficie.empezar(motor) else {
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
            let lupa = Lupa::por_defecto(pieza.monitor.escala_por_cien);
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
    let _ = pieza.superficie.presentar();
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
