//! Por donde mira el editor.
//!
//! La camara del usuario (`camara`) trabaja en pixeles LOGICOS, los de
//! Excalidraw: zoom 1 es un pixel CSS, que Windows multiplica por su escala.
//! Con la pantalla al 150 % y la camara en pixeles fisicos, el mismo
//! `strokeWidth` salia un tercio mas fino que en excalidraw.com (D127).
//!
//! Puro: sin ventana ni GPU, para probarlo todo sin escritorio.

use pixpin_geom::Punto;
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::vector::Punto2;
use pixpin_shell::overlay::EventoOverlay;

/// La escala del monitor como factor. Un monitor que dijera 0 (no pasa, pero
/// un DPI mal leido no puede dejar la camara con zoom cero y dividir por el)
/// cuenta como 100 %.
pub fn escala_de(escala_por_cien: u32) -> f32 {
    if escala_por_cien == 0 {
        1.0
    } else {
        escala_por_cien as f32 / 100.0
    }
}

/// La camara con la que se pinta y se traduce el raton: la del usuario con
/// su zoom multiplicado por la escala del monitor.
///
/// Es la UNICA que ve el pintado y `a_evento`. Los ficheros no cambian: el
/// mundo sigue en las mismas unidades, solo cambia cuantos pixeles fisicos
/// ocupa cada una.
pub fn vista_efectiva(camara: &Camara, escala_por_cien: u32) -> Camara {
    Camara {
        x: camara.x,
        y: camara.y,
        zoom: camara.zoom * escala_de(escala_por_cien),
    }
}

/// Codigo virtual de la barra espaciadora.
pub const VK_ESPACIO: u32 = 0x20;
/// `WHEEL_DELTA`: lo que Windows manda por muesca.
pub const MUESCA: i32 = 120;
/// `ZOOM_STEP` de Excalidraw (packages/common/src/constants.ts:349 @afa3a65).
pub const PASO_ZOOM: f32 = 0.1;
/// `MIN_ZOOM` y `MAX_ZOOM` (constants.ts:350-351).
pub const ZOOM_MINIMO: f32 = 0.1;
pub const ZOOM_MAXIMO: f32 = 30.0;
/// El `deltaY` que Chromium en Windows da por muesca con el ajuste de
/// fabrica (tres lineas): 100 px CSS. Excalidraw no ve la muesca, ve esto.
pub const PX_POR_MUESCA: f32 = 100.0;

/// El `deltaY` del navegador para un giro de Windows. Signo al reves:
/// Windows cuenta positivo alejando la rueda; el navegador, bajando la pagina.
fn delta_y_css(delta_rueda: i32) -> f32 {
    -(delta_rueda as f32) * PX_POR_MUESCA / MUESCA as f32
}

/// El zoom tras un giro con Ctrl. Porte de `handleWheel`
/// (packages/excalidraw/components/App.tsx:14024-14045 @afa3a65) y de
/// `getNormalizedZoom` (packages/excalidraw/scene/normalize.ts:7-9), MIT,
/// Copyright (c) 2020 Excalidraw.
pub fn zoom_de_rueda(zoom: f32, delta_rueda: i32) -> f32 {
    let delta_y = delta_y_css(delta_rueda);
    if delta_y == 0.0 {
        return zoom;
    }
    let signo = delta_y.signum();
    let paso_maximo = PASO_ZOOM * 100.0;
    let absoluto = delta_y.abs();
    let delta = if absoluto > paso_maximo {
        paso_maximo * signo
    } else {
        delta_y
    };
    let mut nuevo = zoom - delta / 100.0;
    // Mas acercado, pasos mayores (solo por encima del 100 %).
    nuevo += zoom.max(1.0).log10() * -signo * (absoluto / 20.0).min(1.0);
    nuevo = nuevo.max(ZOOM_MINIMO);
    ((nuevo * 1e6).round() / 1e6).clamp(ZOOM_MINIMO, ZOOM_MAXIMO)
}

/// Los modificadores que importan a la rueda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modificadores {
    pub ctrl: bool,
    pub shift: bool,
}

/// Lo que hay que hacerle a la camara del usuario.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Accion {
    /// Arrastrar el lienzo `dx`, `dy` pixeles LOGICOS, con el convenio de
    /// `Camara::desplazar` (el dibujo sigue a la mano).
    Desplazar { dx: f32, dy: f32 },
    /// Ctrl+rueda: el zoom lo decide `zoom_de_rueda` con el de la camara, y
    /// `foco` (pixeles logicos de la ventana) se queda quieto.
    ZoomRueda { foco: Punto2, delta: i32 },
}

/// Si el evento era del navegador y que hay que hacer.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Respuesta {
    /// `true`: el evento no puede llegar al gesto ni a la caja.
    pub consumido: bool,
    pub accion: Option<Accion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BotonArrastre {
    Principal,
    Central,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Arrastre {
    boton: BotonArrastre,
    /// Ultima posicion vista, en el escritorio virtual (pixeles fisicos).
    anterior: Punto,
}

/// Rueda, espacio + arrastrar y boton central, con la convencion de
/// Excalidraw (D136): `handleCanvasPanUsingWheelOrSpaceDrag`,
/// App.tsx:9075-9093, y la barra en App.tsx:5850-5853.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Navegador {
    espacio: bool,
    arrastre: Option<Arrastre>,
    /// Donde estaba el raton la ultima vez, en pixeles logicos de la
    /// ventana: el foco del zoom con Ctrl+rueda, que no trae posicion.
    ultimo: Punto2,
}

impl Default for Navegador {
    // WHY: `Punto2` (de `pixpin-motor2d`) no deriva `Default`, asi que
    // `#[derive(Default)]` en `Navegador` no compila (E0277). Se escribe a
    // mano con el mismo origen (0.0, 0.0) que hubiera dado el derive.
    fn default() -> Self {
        Self {
            espacio: false,
            arrastre: None,
            ultimo: Punto2::nuevo(0.0, 0.0),
        }
    }
}

impl Navegador {
    pub fn nuevo() -> Self {
        Self::default()
    }

    pub fn arrastrando(&self) -> bool {
        self.arrastre.is_some()
    }

    /// `en_reposo` es `Gesto::en_reposo`: con un trazo o un arrastre en curso
    /// no se empieza a mover el lienzo (el clic es del gesto).
    pub fn evento(
        &mut self,
        ev: &EventoOverlay,
        origen: Punto,
        escala_por_cien: u32,
        m: Modificadores,
        en_reposo: bool,
    ) -> Respuesta {
        let escala = escala_de(escala_por_cien);
        let consumido = Respuesta {
            consumido: true,
            accion: None,
        };
        match *ev {
            EventoOverlay::Tecla { vk: VK_ESPACIO, .. } => {
                self.espacio = true;
                consumido
            }
            EventoOverlay::TeclaSoltada(VK_ESPACIO) => {
                self.espacio = false;
                consumido
            }
            EventoOverlay::BotonPulsado(p) if self.espacio && en_reposo => {
                self.arrastre = Some(Arrastre {
                    boton: BotonArrastre::Principal,
                    anterior: p,
                });
                consumido
            }
            EventoOverlay::BotonCentralPulsado(p) => {
                if en_reposo {
                    self.arrastre = Some(Arrastre {
                        boton: BotonArrastre::Central,
                        anterior: p,
                    });
                }
                consumido
            }
            EventoOverlay::RatonMovido(p) => {
                self.ultimo = Punto2::nuevo(
                    (p.x - origen.x) as f32 / escala,
                    (p.y - origen.y) as f32 / escala,
                );
                match &mut self.arrastre {
                    Some(a) => {
                        let dx = (p.x - a.anterior.x) as f32 / escala;
                        let dy = (p.y - a.anterior.y) as f32 / escala;
                        a.anterior = p;
                        Respuesta {
                            consumido: true,
                            accion: Some(Accion::Desplazar { dx, dy }),
                        }
                    }
                    None => Respuesta::default(),
                }
            }
            // Mientras se arrastra el lienzo, los puntos finos tampoco son
            // tinta.
            EventoOverlay::Muestra(_) if self.arrastre.is_some() => consumido,
            EventoOverlay::BotonSoltado(_)
                if matches!(
                    self.arrastre,
                    Some(Arrastre {
                        boton: BotonArrastre::Principal,
                        ..
                    })
                ) =>
            {
                self.arrastre = None;
                consumido
            }
            EventoOverlay::BotonCentralSoltado(_) => {
                if matches!(
                    self.arrastre,
                    Some(Arrastre {
                        boton: BotonArrastre::Central,
                        ..
                    })
                ) {
                    self.arrastre = None;
                }
                consumido
            }
            EventoOverlay::Rueda(delta) => {
                let accion = if m.ctrl {
                    Accion::ZoomRueda {
                        foco: self.ultimo,
                        delta,
                    }
                } else if m.shift {
                    // App.tsx:14067-14072: Shift lleva el giro a horizontal.
                    Accion::Desplazar {
                        dx: -delta_y_css(delta),
                        dy: 0.0,
                    }
                } else {
                    // App.tsx:14075-14078: scrollY - deltaY / zoom.
                    Accion::Desplazar {
                        dx: 0.0,
                        dy: -delta_y_css(delta),
                    }
                };
                Respuesta {
                    consumido: true,
                    accion: Some(accion),
                }
            }
            _ => Respuesta::default(),
        }
    }
}

/// Aplica la accion a la camara del usuario (logica). Devuelve si cambio.
pub fn aplicar(camara: &mut Camara, accion: Accion) -> bool {
    match accion {
        Accion::Desplazar { dx, dy } => {
            if dx == 0.0 && dy == 0.0 {
                return false;
            }
            camara.desplazar(dx, dy);
            true
        }
        Accion::ZoomRueda { foco, delta } => {
            let nuevo = zoom_de_rueda(camara.zoom, delta);
            camara.acercar_en(foco, nuevo / camara.zoom)
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const ORIGEN: Punto = Punto { x: 0, y: 0 };
    const NADA: Modificadores = Modificadores {
        ctrl: false,
        shift: false,
    };

    #[test]
    fn el_paso_de_zoom_de_la_rueda_es_el_de_excalidraw() {
        // App.tsx:14024-14045 @afa3a65 con deltaY = -100 por muesca arriba.
        assert!((zoom_de_rueda(1.0, MUESCA) - 1.1).abs() < 1e-4);
        assert!((zoom_de_rueda(1.0, -MUESCA) - 0.9).abs() < 1e-4);
        // Por encima del 100 % el paso crece con log10(zoom).
        assert!((zoom_de_rueda(2.0, MUESCA) - 2.40103).abs() < 1e-4);
        // Topes: MIN_ZOOM y MAX_ZOOM.
        assert_eq!(zoom_de_rueda(30.0, MUESCA), ZOOM_MAXIMO);
        assert_eq!(zoom_de_rueda(0.1, -MUESCA), ZOOM_MINIMO);
        // Caso negativo: una rueda sin giro no cambia nada.
        assert_eq!(zoom_de_rueda(1.7, 0), 1.7);
    }

    #[test]
    fn la_rueda_sola_desplaza_en_vertical_como_excalidraw() {
        let mut n = Navegador::nuevo();
        let r = n.evento(&EventoOverlay::Rueda(-MUESCA), ORIGEN, 100, NADA, true);
        assert!(r.consumido);
        assert_eq!(
            r.accion,
            Some(Accion::Desplazar {
                dx: 0.0,
                dy: -100.0
            })
        );
        let mut c = Camara::nueva();
        assert!(aplicar(&mut c, r.accion.unwrap()));
        assert_eq!((c.x, c.y), (0.0, 100.0), "rueda abajo = mirar mas abajo");
    }

    #[test]
    fn shift_y_rueda_desplaza_en_horizontal() {
        let mut n = Navegador::nuevo();
        let shift = Modificadores {
            ctrl: false,
            shift: true,
        };
        let r = n.evento(&EventoOverlay::Rueda(-MUESCA), ORIGEN, 100, shift, true);
        assert_eq!(
            r.accion,
            Some(Accion::Desplazar {
                dx: -100.0,
                dy: 0.0
            })
        );
    }

    #[test]
    fn control_y_rueda_acerca_hacia_el_cursor() {
        let mut n = Navegador::nuevo();
        let _ = n.evento(
            &EventoOverlay::RatonMovido(Punto { x: 300, y: 200 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        let ctrl = Modificadores {
            ctrl: true,
            shift: false,
        };
        let r = n.evento(&EventoOverlay::Rueda(MUESCA), ORIGEN, 100, ctrl, true);
        let Some(accion @ Accion::ZoomRueda { foco, .. }) = r.accion else {
            panic!("Ctrl+rueda es zoom, dio {:?}", r.accion);
        };
        let mut c = Camara::nueva();
        let antes = c.a_mundo(foco);
        assert!(aplicar(&mut c, accion));
        assert!((c.zoom - 1.1).abs() < 1e-4);
        let despues = c.a_mundo(foco);
        assert!((antes.x - despues.x).abs() < 1e-3 && (antes.y - despues.y).abs() < 1e-3);
    }

    #[test]
    fn espacio_y_arrastrar_desplaza_y_no_llega_al_gesto() {
        let mut n = Navegador::nuevo();
        let espacio = EventoOverlay::Tecla {
            vk: VK_ESPACIO,
            shift: false,
            ctrl: false,
            alt: false,
        };
        assert!(n.evento(&espacio, ORIGEN, 100, NADA, true).consumido);
        let p = n.evento(
            &EventoOverlay::BotonPulsado(Punto { x: 100, y: 100 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        assert!(p.consumido && n.arrastrando());
        let m = n.evento(
            &EventoOverlay::RatonMovido(Punto { x: 130, y: 110 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        assert!(m.consumido);
        assert_eq!(m.accion, Some(Accion::Desplazar { dx: 30.0, dy: 10.0 }));
        let s = n.evento(
            &EventoOverlay::BotonSoltado(Punto { x: 130, y: 110 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        assert!(s.consumido && !n.arrastrando());
        let _ = n.evento(
            &EventoOverlay::TeclaSoltada(VK_ESPACIO),
            ORIGEN,
            100,
            NADA,
            true,
        );
        // Soltado el espacio, el siguiente clic vuelve a ser del gesto.
        let otro = n.evento(
            &EventoOverlay::BotonPulsado(Punto { x: 5, y: 5 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        assert!(!otro.consumido);
    }

    #[test]
    fn el_boton_central_arrastra_sin_espacio() {
        let mut n = Navegador::nuevo();
        let _ = n.evento(
            &EventoOverlay::BotonCentralPulsado(Punto { x: 10, y: 10 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        let m = n.evento(
            &EventoOverlay::RatonMovido(Punto { x: 4, y: 30 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        assert_eq!(m.accion, Some(Accion::Desplazar { dx: -6.0, dy: 20.0 }));
        let s = n.evento(
            &EventoOverlay::BotonCentralSoltado(Punto { x: 4, y: 30 }),
            ORIGEN,
            100,
            NADA,
            true,
        );
        assert!(s.consumido && !n.arrastrando());
    }

    #[test]
    fn sin_espacio_el_clic_y_el_movimiento_llegan_al_gesto() {
        let mut n = Navegador::nuevo();
        for ev in [
            EventoOverlay::BotonPulsado(Punto { x: 1, y: 1 }),
            EventoOverlay::RatonMovido(Punto { x: 9, y: 9 }),
            EventoOverlay::BotonSoltado(Punto { x: 9, y: 9 }),
        ] {
            let r = n.evento(&ev, ORIGEN, 100, NADA, true);
            assert!(!r.consumido, "{ev:?} no es del navegador");
            assert_eq!(r.accion, None);
        }
    }

    #[test]
    fn con_un_trazo_en_curso_el_espacio_no_roba_el_arrastre() {
        // Caso negativo: pulsar espacio a mitad de un trazo y seguir
        // moviendo tiene que seguir dibujando, no mover el lienzo.
        let mut n = Navegador::nuevo();
        let espacio = EventoOverlay::Tecla {
            vk: VK_ESPACIO,
            shift: false,
            ctrl: false,
            alt: false,
        };
        let _ = n.evento(&espacio, ORIGEN, 100, NADA, false);
        let p = n.evento(
            &EventoOverlay::BotonPulsado(Punto { x: 1, y: 1 }),
            ORIGEN,
            100,
            NADA,
            false,
        );
        assert!(!p.consumido && !n.arrastrando());
    }

    #[test]
    fn a_escala_150_el_lienzo_sigue_al_raton_pixel_a_pixel() {
        let mut n = Navegador::nuevo();
        let _ = n.evento(
            &EventoOverlay::BotonCentralPulsado(Punto { x: 0, y: 0 }),
            ORIGEN,
            150,
            NADA,
            true,
        );
        let m = n.evento(
            &EventoOverlay::RatonMovido(Punto { x: 150, y: 0 }),
            ORIGEN,
            150,
            NADA,
            true,
        );
        let mut c = Camara::nueva();
        let punto = Punto2::nuevo(40.0, 0.0);
        let antes = vista_efectiva(&c, 150).a_pantalla(punto);
        assert!(aplicar(&mut c, m.accion.unwrap()));
        let despues = vista_efectiva(&c, 150).a_pantalla(punto);
        assert!((despues.x - antes.x - 150.0).abs() < 1e-3);
    }

    #[test]
    fn a_escala_cien_la_vista_es_la_camara_del_usuario() {
        let c = Camara {
            x: 12.0,
            y: -7.0,
            zoom: 2.5,
        };
        assert_eq!(vista_efectiva(&c, 100), c);
    }

    #[test]
    fn ida_y_vuelta_de_pantalla_a_mundo_con_escala_150_y_zoom_arbitrario() {
        let c = Camara {
            x: -340.25,
            y: 118.5,
            zoom: 0.37,
        };
        let v = vista_efectiva(&c, 150);
        for p in [
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(123.5, -44.25),
            Punto2::nuevo(-900.0, 2048.0),
        ] {
            let vuelta = v.a_mundo(v.a_pantalla(p));
            assert!(
                (vuelta.x - p.x).abs() < 1e-2 && (vuelta.y - p.y).abs() < 1e-2,
                "{p:?} volvio como {vuelta:?}"
            );
        }
    }

    #[test]
    fn un_grosor_de_uno_a_escala_150_ocupa_uno_y_medio_lo_de_escala_100() {
        let c = Camara::nueva();
        let ancho = |escala| {
            let v = vista_efectiva(&c, escala);
            v.a_pantalla(Punto2::nuevo(1.0, 0.0)).x - v.a_pantalla(Punto2::nuevo(0.0, 0.0)).x
        };
        assert!((ancho(100) - 1.0).abs() < 1e-6);
        assert!((ancho(150) - 1.5).abs() < 1e-6);
        assert!((ancho(150) / ancho(100) - 1.5).abs() < 1e-6);
    }

    #[test]
    fn una_escala_de_cero_no_deja_la_camara_sin_zoom() {
        // Caso negativo: con zoom 0, `a_mundo` dividiria por cero y el raton
        // caeria en el infinito.
        let v = vista_efectiva(&Camara::nueva(), 0);
        assert_eq!(v.zoom, 1.0);
    }
}
