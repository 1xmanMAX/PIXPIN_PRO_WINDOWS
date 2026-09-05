//! Las ventanas y el reloj de la grabacion en dos fases (P5b).
//!
//! La geometria vive en `grabador`, que se puede probar sin abrir nada.
//! Aqui esta lo que necesita una pantalla: el marco alrededor de la zona,
//! la barra de control, y el bucle que atiende el raton mientras cuenta
//! fotogramas a su ritmo.
//!
//! El bucle no puede cederle el hilo a `GetMessage`, que se duerme hasta
//! que llega un mensaje y perderia el compas. Bombea los mensajes a mano y
//! recoge los eventos con `tomar_eventos_pendientes`, durmiendo a ratos
//! cortos para que el boton de parar responda al instante aunque se este
//! grabando a cinco fotogramas por segundo.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_geom::{Punto, Rect};
use pixpin_render::{Color, MotorRender, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};

use crate::grabador::{
    Asa, BARRA_ALTO, BARRA_ANCHO, Boton, Fase, Fin, GROSOR, Grabacion, MARGEN, RITMO_HUECO,
    RITMO_POR_DEFECTO, RITMOS, aplicar_asa, asa_en, boton_en, botones, marco_de, reloj,
    tope_segundos,
};
use crate::overlay::Recursos;

/// Cada cuanto se atiende el raton. Es el techo de lo que puede tardar el
/// boton de parar en responder, asi que va muy por debajo del paso de
/// captura mas lento (200 ms a cinco por segundo).
const LATIDO: Duration = Duration::from_millis(6);

const AZUL: Color = Color {
    r: 0.16,
    g: 0.51,
    b: 0.96,
    a: 1.0,
};
const ROJO: Color = Color {
    r: 0.90,
    g: 0.20,
    b: 0.18,
    a: 1.0,
};
const FONDO: Color = Color {
    r: 0.08,
    g: 0.08,
    b: 0.09,
    a: 0.94,
};
const TINTA: Color = Color {
    r: 0.95,
    g: 0.95,
    b: 0.96,
    a: 1.0,
};
const TENUE: Color = Color {
    r: 0.62,
    g: 0.63,
    b: 0.66,
    a: 1.0,
};
const TECLA: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.12,
};

/// El marco que rodea la zona: azul mientras se elige, rojo mientras se
/// graba.
///
/// El borde se pinta entero por FUERA de la zona. Si invadiera aunque
/// fuera un pixel, ese pixel saldria en el GIF y el resultado tendria un
/// reborde de colores que no estaba en la pantalla.
struct Marco {
    ventana: VentanaOverlay,
    superficie: Superficie,
    motor: std::rc::Rc<MotorRender>,
    escala: f32,
    zona: Rect,
}

impl Marco {
    fn nuevo(recursos: &Recursos, zona: Rect, escala: f32) -> Option<Marco> {
        let marco = marco_de(zona);
        let ventana = VentanaOverlay::nueva(marco).ok()?;
        let motor = recursos.motor();
        let superficie = Superficie::nueva(
            &motor,
            &recursos.d3d(),
            ventana.handle(),
            marco.ancho,
            marco.alto,
        )
        .ok()?;
        ventana.poner_hueco(Some(zona));
        ventana.mostrar();
        Some(Marco {
            ventana,
            superficie,
            motor,
            escala,
            zona,
        })
    }

    /// Lleva el marco a la zona nueva. El hueco va detras: si se quedara
    /// donde estaba, el raton dejaria de atravesar el centro.
    fn recolocar(&mut self, zona: Rect) {
        let antes = marco_de(self.zona);
        let ahora = marco_de(zona);
        self.zona = zona;
        self.ventana.mover(ahora);
        self.ventana.poner_hueco(Some(zona));
        if (antes.ancho, antes.alto) != (ahora.ancho, ahora.alto) {
            let _ = self.superficie.redimensionar(ahora.ancho, ahora.alto);
        }
    }

    fn pintar(&self, fase: Fase) {
        let Ok(destino) = self.superficie.empezar(&self.motor) else {
            return;
        };
        let e = self.escala;
        let marco = marco_de(self.zona);
        let (ancho, alto) = (marco.ancho as f32, marco.alto as f32);
        // El borde vive entre el margen de agarre y la zona: de `MARGEN -
        // GROSOR` a `MARGEN`, siempre por fuera de lo que se graba.
        let d = (MARGEN - GROSOR) as f32 * e;
        let g = GROSOR as f32 * e;
        let color = match fase {
            Fase::Esperando => AZUL,
            Fase::Grabando => ROJO,
            // Pausada: el mismo rojo apagado, para que se distinga de un
            // marco que sigue contando.
            Fase::Pausada => Color { a: 0.45, ..ROJO },
        };
        let _ = self.motor.dibujar(&destino, |p| {
            p.limpiar_transparente();
            let banda = |x: f32, y: f32, w: f32, h: f32| {
                p.rellenar_redondeado(
                    RectF {
                        x,
                        y,
                        ancho: w,
                        alto: h,
                    },
                    0.0,
                    color,
                );
            };
            banda(d, d, ancho - 2.0 * d, g);
            banda(d, alto - d - g, ancho - 2.0 * d, g);
            banda(d, d, g, alto - 2.0 * d);
            banda(ancho - d - g, d, g, alto - 2.0 * d);
        });
        let _ = self.superficie.presentar();
    }
}

/// La barra de control: los botones y el reloj.
struct Barra {
    ventana: VentanaOverlay,
    superficie: Superficie,
    motor: std::rc::Rc<MotorRender>,
    escala: f32,
    /// Hasta donde llega la pantalla por abajo. Es lo que decide si la
    /// barra cabe debajo del marco o tiene que ponerse encima.
    suelo: i32,
    /// El boton bajo el raton, para encenderlo al pasar por encima.
    resaltado: Option<Boton>,
}

impl Barra {
    fn nueva(recursos: &Recursos, zona: Rect, escala: f32, suelo: i32) -> Option<Barra> {
        let ventana = VentanaOverlay::nueva(Barra::sitio(zona, escala, suelo)).ok()?;
        let motor = recursos.motor();
        let (ancho, alto) = Barra::tamano(escala);
        let superficie =
            Superficie::nueva(&motor, &recursos.d3d(), ventana.handle(), ancho, alto).ok()?;
        ventana.mostrar();
        Some(Barra {
            ventana,
            superficie,
            motor,
            escala,
            suelo,
            resaltado: None,
        })
    }

    fn tamano(escala: f32) -> (u32, u32) {
        (
            (BARRA_ANCHO as f32 * escala) as u32,
            (BARRA_ALTO as f32 * escala) as u32,
        )
    }

    /// Debajo del marco, pegada a su borde izquierdo. Si no cabe debajo,
    /// encima; nunca dentro de la zona, que se grabaria a si misma.
    fn sitio(zona: Rect, escala: f32, suelo: i32) -> Rect {
        let (ancho, alto) = Barra::tamano(escala);
        let marco = marco_de(zona);
        let debajo = marco.abajo() + 6;
        let y = if debajo + alto as i32 > suelo {
            marco.arriba() - alto as i32 - 6
        } else {
            debajo
        };
        Rect {
            x: marco.x,
            y,
            ancho,
            alto,
        }
    }

    fn recolocar(&mut self, zona: Rect) {
        let sitio = Barra::sitio(zona, self.escala, self.suelo);
        self.ventana.mover(sitio);
    }

    /// El boton bajo un punto del escritorio, o `None` si el punto no cae
    /// en la barra.
    fn boton_bajo(&self, fase: Fase, punto: Punto) -> Option<Boton> {
        let sitio = self.ventana.area();
        if !sitio.contiene(punto) {
            return None;
        }
        let x = (punto.x - sitio.x) as f32 / self.escala;
        let y = (punto.y - sitio.y) as f32 / self.escala;
        boton_en(fase, x, y)
    }

    fn pintar(&self, fase: Fase, ritmo: u32, segundo: u64, tope: u64) {
        let Ok(destino) = self.superficie.empezar(&self.motor) else {
            return;
        };
        let e = self.escala;
        let (ancho, alto) = (BARRA_ANCHO as f32 * e, BARRA_ALTO as f32 * e);
        let resaltado = self.resaltado;
        let _ = self.motor.dibujar(&destino, |p| {
            p.limpiar_transparente();
            p.rellenar_redondeado(
                RectF {
                    x: 0.0,
                    y: 0.0,
                    ancho,
                    alto,
                },
                9.0 * e,
                FONDO,
            );
            let centrar = |texto: &str, r: RectF, tam: f32, color: Color| {
                let (w, h) = p.medir_texto(texto, tam);
                p.texto(
                    texto,
                    r.x + (r.ancho - w) / 2.0,
                    r.y + (r.alto - h) / 2.0,
                    tam,
                    color,
                );
            };
            for (boton, r) in botones(fase) {
                let r = RectF {
                    x: r.x * e,
                    y: r.y * e,
                    ancho: r.ancho * e,
                    alto: r.alto * e,
                };
                if resaltado == Some(boton) {
                    p.rellenar_redondeado(r, 6.0 * e, TECLA);
                }
                match boton {
                    Boton::Grabar => {
                        // El punto rojo delante del rotulo: se reconoce
                        // antes por la forma que por la palabra.
                        p.rellenar_redondeado(
                            RectF {
                                x: r.x + 11.0 * e,
                                y: r.y + (r.alto - 10.0 * e) / 2.0,
                                ancho: 10.0 * e,
                                alto: 10.0 * e,
                            },
                            5.0 * e,
                            ROJO,
                        );
                        p.texto(
                            "Grabar",
                            r.x + 29.0 * e,
                            r.y + (r.alto - 15.0 * e) / 2.0,
                            13.0 * e,
                            TINTA,
                        );
                    }
                    Boton::Parar => {
                        p.rellenar_redondeado(
                            RectF {
                                x: r.x + 11.0 * e,
                                y: r.y + (r.alto - 10.0 * e) / 2.0,
                                ancho: 10.0 * e,
                                alto: 10.0 * e,
                            },
                            1.0 * e,
                            ROJO,
                        );
                        p.texto(
                            "Parar",
                            r.x + 29.0 * e,
                            r.y + (r.alto - 15.0 * e) / 2.0,
                            13.0 * e,
                            TINTA,
                        );
                    }
                    Boton::Pausar => {
                        let texto = if fase == Fase::Pausada {
                            "Seguir"
                        } else {
                            "Pausa"
                        };
                        centrar(texto, r, 13.0 * e, TINTA);
                    }
                    Boton::MenosRitmo => centrar("-", r, 16.0 * e, TINTA),
                    Boton::MasRitmo => centrar("+", r, 16.0 * e, TINTA),
                    Boton::Cerrar => centrar("Cerrar", r, 13.0 * e, TENUE),
                }
            }
            match fase {
                Fase::Esperando => {
                    // El ritmo, entre los dos botones que lo cambian.
                    let fila = botones(fase);
                    let menos = fila
                        .iter()
                        .find(|(b, _)| *b == Boton::MenosRitmo)
                        .map(|(_, r)| r.x + r.ancho)
                        .unwrap_or(0.0);
                    centrar(
                        &format!("{ritmo}/s"),
                        RectF {
                            x: (menos + 6.0) * e,
                            y: 0.0,
                            ancho: RITMO_HUECO * e,
                            alto,
                        },
                        13.0 * e,
                        TINTA,
                    );
                }
                Fase::Grabando | Fase::Pausada => {
                    // El reloj: lo que llevas y hasta donde llega. El tope
                    // es el de verdad, ya descontada la memoria.
                    let fila = botones(fase);
                    let fin = fila
                        .iter()
                        .map(|(_, r)| r.x + r.ancho)
                        .fold(0.0_f32, f32::max);
                    centrar(
                        &format!("{} / {}", reloj(segundo), reloj(tope)),
                        RectF {
                            x: (fin + 6.0) * e,
                            y: 0.0,
                            ancho: (BARRA_ANCHO as f32 - fin - 15.0) * e,
                            alto,
                        },
                        13.0 * e,
                        TINTA,
                    );
                }
            }
        });
        let _ = self.superficie.presentar();
    }
}

/// Estado del arrastre en curso sobre el marco.
struct Arrastre {
    asa: Asa,
    desde: Punto,
    zona_inicial: Rect,
}

/// Abre la sesion de grabacion sobre `zona_inicial` y no vuelve hasta que
/// el usuario para o cancela.
///
/// Devuelve `None` si no hay nada aprovechable: cancelo, o no dio tiempo a
/// juntar dos fotogramas, que es lo minimo para que haya animacion.
pub fn ejecutar_sesion(
    recursos: &mut Recursos,
    zona_inicial: Rect,
    id_atajo_gif: Option<u32>,
) -> Result<Option<Grabacion>> {
    if zona_inicial.ancho == 0 || zona_inicial.alto == 0 {
        return Ok(None);
    }
    let disposicion = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = disposicion
        .monitores()
        .iter()
        .find(|m| m.area.interseccion(zona_inicial).is_some())
        .or_else(|| disposicion.principal())
        .context("sin monitor para la zona")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;

    let mut zona = zona_inicial;
    let mut fase = Fase::Esperando;
    let mut indice_ritmo = RITMO_POR_DEFECTO;
    let mut fotogramas: Vec<ImagenRgba> = Vec::new();
    let mut arrastre: Option<Arrastre> = None;

    let mut marco = Marco::nuevo(recursos, zona, escala);
    let mut barra = Barra::nueva(recursos, zona, escala, monitor.area.abajo());
    let mut repintar = true;
    // Reloj de la grabacion: `desde` es cuando arranco el tramo actual y
    // `acumulado` lo que sumaron los tramos anteriores. Pausar cierra un
    // tramo, seguir abre otro: asi el tiempo en pausa no cuenta.
    let mut desde: Option<Instant> = None;
    let mut acumulado = Duration::ZERO;
    let mut siguiente = Instant::now();
    let mut segundo_pintado = u64::MAX;

    let fin = 'sesion: loop {
        pixpin_shell::overlay::bombear_pendientes();
        let ritmo = RITMOS[indice_ritmo];
        let tope = tope_segundos(zona, ritmo);
        let transcurrido = acumulado + desde.map(|d| d.elapsed()).unwrap_or(Duration::ZERO);

        // El mismo atajo que abrio la sesion la para: es lo que hace el
        // original, y evita tener que buscar el boton con el raton.
        for id in pixpin_shell::ventana::tomar_atajos_pendientes() {
            if Some(id) != id_atajo_gif {
                continue;
            }
            match fase {
                Fase::Esperando => {
                    fase = Fase::Grabando;
                    desde = Some(Instant::now());
                    siguiente = Instant::now();
                    repintar = true;
                }
                Fase::Grabando | Fase::Pausada => break 'sesion Fin::Usuario,
            }
        }

        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            let en_barra = barra.as_ref().is_some_and(|b| b.ventana.handle() == hwnd);
            let en_marco = marco.as_ref().is_some_and(|m| m.ventana.handle() == hwnd);
            match evento {
                EventoOverlay::BotonPulsado(punto) if en_barra => {
                    let Some(boton) = barra.as_ref().and_then(|b| b.boton_bajo(fase, punto)) else {
                        continue;
                    };
                    match boton {
                        Boton::Cerrar => break 'sesion Fin::Escape,
                        Boton::Parar => break 'sesion Fin::Usuario,
                        Boton::Grabar => {
                            fase = Fase::Grabando;
                            desde = Some(Instant::now());
                            siguiente = Instant::now();
                        }
                        Boton::Pausar => {
                            if fase == Fase::Pausada {
                                fase = Fase::Grabando;
                                desde = Some(Instant::now());
                                siguiente = Instant::now();
                            } else {
                                fase = Fase::Pausada;
                                acumulado += desde.take().map_or(Duration::ZERO, |d| d.elapsed());
                            }
                        }
                        // El ritmo solo se toca antes de empezar: cambiarlo
                        // a media grabacion dejaria fotogramas de dos
                        // velocidades en el mismo fichero, y el GIF declara
                        // una sola.
                        Boton::MenosRitmo => indice_ritmo = indice_ritmo.saturating_sub(1),
                        Boton::MasRitmo => indice_ritmo = (indice_ritmo + 1).min(RITMOS.len() - 1),
                    }
                    repintar = true;
                }
                EventoOverlay::RatonMovido(punto) if en_barra => {
                    let antes = barra.as_ref().and_then(|b| b.resaltado);
                    let ahora = barra.as_ref().and_then(|b| b.boton_bajo(fase, punto));
                    if antes != ahora {
                        if let Some(b) = barra.as_mut() {
                            b.resaltado = ahora;
                        }
                        repintar = true;
                    }
                }
                // El marco solo se agarra mientras se elige: durante la
                // grabacion, cambiar el tamano daria fotogramas de medidas
                // distintas y no habria GIF que armar con ellos.
                EventoOverlay::BotonPulsado(punto) if en_marco && fase == Fase::Esperando => {
                    if let Some(asa) = asa_en(zona, punto) {
                        arrastre = Some(Arrastre {
                            asa,
                            desde: punto,
                            zona_inicial: zona,
                        });
                    }
                }
                EventoOverlay::RatonMovido(punto) if en_marco => {
                    if let Some(a) = &arrastre {
                        zona = aplicar_asa(
                            a.zona_inicial,
                            a.asa,
                            punto.x - a.desde.x,
                            punto.y - a.desde.y,
                        );
                        if let Some(m) = marco.as_mut() {
                            m.recolocar(zona);
                        }
                        if let Some(b) = barra.as_mut() {
                            b.recolocar(zona);
                        }
                        repintar = true;
                    } else if fase == Fase::Esperando {
                        if let Some(asa) = asa_en(zona, punto) {
                            if let Some(m) = &marco {
                                m.ventana.poner_cursor(asa.cursor());
                            }
                        }
                    }
                }
                EventoOverlay::BotonSoltado(_) => arrastre = None,
                EventoOverlay::Cerrar => break 'sesion Fin::Escape,
                EventoOverlay::Tecla { vk: 0x0D, .. } if fase == Fase::Esperando => {
                    // Intro empieza, igual que el atajo.
                    fase = Fase::Grabando;
                    desde = Some(Instant::now());
                    siguiente = Instant::now();
                    repintar = true;
                }
                // Y una vez grabando, la misma tecla termina.
                EventoOverlay::Tecla { vk: 0x0D, .. } => break 'sesion Fin::Usuario,
                _ => {}
            }
        }

        // Escape se sondea y no llega por ningun WndProc: durante la
        // grabacion el foco esta en la aplicacion que se graba, no en
        // ninguna ventana nuestra.
        if pixpin_shell::escape_pulsado() {
            break match fase {
                Fase::Esperando => Fin::Escape,
                _ => Fin::Usuario,
            };
        }

        if fase == Fase::Grabando {
            if transcurrido.as_secs() >= tope {
                break Fin::Tiempo;
            }
            let bytes = zona.ancho as usize * zona.alto as usize * 4;
            if (fotogramas.len() + 1) * bytes > crate::grabador::MEMORIA_MAXIMA {
                break Fin::Memoria;
            }
            if Instant::now() >= siguiente {
                match crate::scroll::capturar(recursos, &monitor, zona) {
                    Ok(f) => fotogramas.push(f),
                    // Un fotograma perdido no tira la grabacion entera: se
                    // anota y se sigue, que es mejor que perderlo todo.
                    Err(e) => tracing::warn!(?e, "fotograma perdido durante la grabacion"),
                }
                // El compas va contra un instante ABSOLUTO y no durmiendo
                // intervalos: dormir «lo que falta» acumula el error de
                // cada vuelta y el GIF acaba yendo mas lento de lo que
                // declara. Si una captura tardo de mas, se salta al hueco
                // siguiente en vez de encadenar capturas para recuperar.
                let paso = Duration::from_millis(1000 / ritmo as u64);
                siguiente += paso;
                let ahora = Instant::now();
                if siguiente <= ahora {
                    siguiente = ahora + paso;
                }
            }
        }

        let segundo = transcurrido.as_secs();
        if segundo != segundo_pintado && fase != Fase::Esperando {
            segundo_pintado = segundo;
            repintar = true;
        }
        if repintar {
            repintar = false;
            if let Some(m) = &marco {
                m.pintar(fase);
            }
            if let Some(b) = &barra {
                b.pintar(fase, ritmo, segundo, tope);
            }
        }
        std::thread::sleep(LATIDO);
    };

    // Fuera antes de codificar: si no, el marco y la barra se quedarian en
    // pantalla durante el rato que cuesta comprimir el GIF.
    marco.take();
    barra.take();
    pixpin_shell::overlay::bombear_pendientes();

    tracing::info!(
        fotogramas = fotogramas.len(),
        por_segundo = RITMOS[indice_ritmo],
        ?fin,
        "grabacion terminada"
    );
    // Con menos de dos no hay animacion que ensenar.
    if fotogramas.len() < 2 {
        return Ok(None);
    }
    Ok(Some(Grabacion {
        fotogramas,
        fin,
        por_segundo: RITMOS[indice_ritmo],
    }))
}
