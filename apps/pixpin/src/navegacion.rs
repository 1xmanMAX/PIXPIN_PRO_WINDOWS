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
use pixpin_shell::gestos_tactiles::RuedaCruda;
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
// Solo la usan las pruebas: el editor pasa siempre por `zoom_de_rueda_entre`
// con el minimo de su navegador.
#[cfg(test)]
pub fn zoom_de_rueda(zoom: f32, delta_rueda: i32) -> f32 {
    zoom_de_rueda_entre(zoom, delta_rueda, ZOOM_MINIMO)
}

/// Como `zoom_de_rueda`, dejando bajar hasta `minimo`.
///
/// Por encima del 10 % es la rueda de Excalidraw tal cual. Por debajo no
/// puede serlo: su paso es de 0,1 fijo, y desde 0,1 una sola muesca lo
/// dejaria en cero. Ahi el paso pasa a ser proporcional, un 10 % por
/// muesca, que es como se siente igual a cualquier distancia. Solo lo usa
/// el universo, que es el unico lienzo que baja de 0,1.
pub fn zoom_de_rueda_entre(zoom: f32, delta_rueda: i32, minimo: f32) -> f32 {
    let delta_y = delta_y_css(delta_rueda);
    if delta_y == 0.0 {
        return zoom;
    }
    if minimo < ZOOM_MINIMO {
        let paso = delta_y.clamp(-PASO_ZOOM * 100.0, PASO_ZOOM * 100.0);
        let lineal = zoom_de_rueda_excalidraw(zoom, delta_rueda);
        if zoom < ZOOM_MINIMO || lineal <= ZOOM_MINIMO && paso > 0.0 {
            let nuevo = zoom * 1.1f32.powf(-paso / (PASO_ZOOM * 100.0));
            return ((nuevo * 1e6).round() / 1e6).clamp(minimo, ZOOM_MAXIMO);
        }
        return lineal;
    }
    zoom_de_rueda_excalidraw(zoom, delta_rueda)
}

fn zoom_de_rueda_excalidraw(zoom: f32, delta_rueda: i32) -> f32 {
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

/// VK_LBUTTON: boton principal del raton. Windows ya compensa el intercambio
/// de botones para zurdos -la documentacion de `GetAsyncKeyState` dice que
/// esta constante sigue significando "el principal"-, asi que no hace falta
/// distinguir zurdo de diestro aqui.
const VK_BOTON_PRINCIPAL: u32 = 0x01;
/// VK_RBUTTON: boton secundario del raton. Hace falta porque `overlay.rs`
/// traduce `WM_LBUTTONDOWN` Y `WM_RBUTTONDOWN` al mismo
/// `EventoOverlay::BotonPulsado` ("el boton derecho vale lo mismo que el
/// izquierdo"): un arrastre por espacio puede haberlo sujetado el derecho, y
/// sondear solo el izquierdo lo daria por soltado en el primer
/// `RatonMovido` (revision 2, hallazgo HIGH).
const VK_BOTON_SECUNDARIO: u32 = 0x02;
/// VK_MBUTTON: boton central (la rueda pulsada) del raton.
const VK_BOTON_CENTRAL: u32 = 0x04;

/// Lo que hay que sondear del sistema AHORA MISMO para dos casos donde el
/// evento de Windows que cerraria el gesto nunca llega (revision 1 de esta
/// tarea):
///
/// 1. Se suelta el espacio con un Alt+Tab, o dentro del bucle propio de
///    `pedir_medida`: `TeclaSoltada(VK_ESPACIO)` no se manda a esta ventana,
///    y sin comprobarlo el arrastre se activaria con una tecla que ya no
///    esta pulsada -el editor se quedaria pegado en "mover" para siempre-.
/// 2. Windows le quita la captura al raton a mitad de un arrastre: el
///    `BotonSoltado`/`BotonCentralSoltado` tampoco llega, y sin esto cada
///    `RatonMovido` seguiria moviendo el lienzo con el boton ya levantado.
///
/// `Navegador` sigue siendo puro -no llama a Windows-: quien lo rodea
/// rellena estos dos campos con `pixpin_shell::entrada::tecla_pulsada_ahora`,
/// y solo para los eventos donde hace falta (un `BotonPulsado` con el
/// espacio puesto, o un `RatonMovido`/`Muestra` mientras `arrastrando()`),
/// no en cada muestra del lapiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnVivo {
    /// Si la barra espaciadora sigue pulsada AHORA. Solo importa al llegar
    /// `BotonPulsado` con `self.espacio` ya puesto.
    pub espacio: bool,
    /// Si ALGUNO de los botones que pudo empezar el arrastre en curso sigue
    /// pulsado AHORA (para `Principal`, izquierdo O derecho: ver
    /// `botones_en_arrastre`). Solo importa mientras `arrastrando()` es
    /// `true`.
    pub boton_arrastre: bool,
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
    /// La rueda en el universo (`Navegador::mapa`): acercar o alejar con
    /// `foco` quieto, suave. Quien lo recibe lo persigue fotograma a
    /// fotograma (`universo::mapa::Suave`); `aplicar_con_minimo` lo aplica
    /// de golpe.
    ZoomSuave { foco: Punto2, delta: i32 },
    /// El pellizco del panel tactil: multiplicar el zoom por `factor` con
    /// `foco` quieto, de golpe (el pellizco ya llega continuo, paso a paso).
    Escalar { foco: Punto2, factor: f32 },
}

/// De donde viene una rueda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuenteRueda {
    /// Una rueda de muescas: acerca y aleja.
    Raton,
    /// Un panel tactil que no es de precision (o uno de precision sin
    /// DirectManipulation): dos dedos, desplazar.
    Panel,
}

/// Cuanto dura una racha del panel tactil: un giro de 120 justo que llegue
/// menos de esto despues de un giro a trozos sigue siendo del panel. Los
/// paneles viejos mandan a veces un 120 entre sus trozos, y la inercia de
/// Synaptics sigue mandando giros un rato. Cambiar de la mano del panel a la
/// rueda del raton en menos de un cuarto de segundo no se puede.
pub const RACHA_PANEL_MS: u32 = 250;

/// Raton o panel, para la rueda que SI llega como rueda (con
/// DirectManipulation, el panel de precision ya no llega asi:
/// `pixpin_shell::gestos_tactiles`).
///
/// La regla: una rueda de muescas manda SIEMPRE multiplos de `WHEEL_DELTA`
/// (120), por rapido que gire. Un giro que no lo es solo puede venir de un
/// panel (o de un raton de rueda libre de alta resolucion, que desplaza:
/// fallar hacia desplazar no pierde nada, fallar hacia acercar marea). Y un
/// 120 solo cuenta como panel si llega en plena racha de panel. Asi una
/// rueda de muescas nunca es panel; el unico panel que se toma por raton
/// es uno que solo mande 120 justos, que no se puede distinguir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Clasificador {
    ultimo_panel: Option<u32>,
}

impl Clasificador {
    pub fn fuente(&mut self, r: &RuedaCruda) -> FuenteRueda {
        let a_trozos = r.delta % MUESCA != 0;
        // `wrapping_sub`: `GetMessageTime` da la vuelta cada 49 dias.
        let en_racha = self
            .ultimo_panel
            .is_some_and(|t| r.ms.wrapping_sub(t) <= RACHA_PANEL_MS);
        if a_trozos || en_racha {
            self.ultimo_panel = Some(r.ms);
            FuenteRueda::Panel
        } else {
            self.ultimo_panel = None;
            FuenteRueda::Raton
        }
    }
}

/// Que hace una rueda, sabida su fuente. `foco`: donde esta el raton.
///
/// - Horizontal (inclinar la rueda, o dos dedos de lado): desplazar de lado,
///   aunque lleve Ctrl.
/// - Ctrl: zoom, venga de donde venga (asi llega tambien el pellizco de un
///   panel sin DirectManipulation).
/// - Shift: desplazar de lado.
/// - Sin nada: el raton acerca hacia el cursor; el panel desplaza.
pub fn decidir_rueda(r: &RuedaCruda, fuente: FuenteRueda, foco: Punto2) -> Accion {
    if r.horizontal {
        // Positivo es a la derecha: el signo del `deltaX` del navegador.
        return Accion::Desplazar {
            dx: delta_y_css(r.delta),
            dy: 0.0,
        };
    }
    if r.ctrl {
        return Accion::ZoomSuave {
            foco,
            delta: r.delta,
        };
    }
    if r.shift {
        return Accion::Desplazar {
            dx: -delta_y_css(r.delta),
            dy: 0.0,
        };
    }
    match fuente {
        FuenteRueda::Raton => Accion::ZoomSuave {
            foco,
            delta: r.delta,
        },
        FuenteRueda::Panel => Accion::Desplazar {
            dx: 0.0,
            dy: -delta_y_css(r.delta),
        },
    }
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
    /// Hasta donde deja alejarse este lienzo. Un dibujo se queda en el
    /// minimo del motor; el universo baja mucho mas para ver todas las
    /// galaxias a la vez. Quien lo cambia lo pasa a `aplicar_con_minimo`.
    pub zoom_minimo: f32,
    /// Con el universo detras: la rueda acerca y aleja (sin Ctrl), como en
    /// un mapa. En un lienzo normal es `false` y la rueda desplaza, como en
    /// Excalidraw.
    ///
    /// Solo cuenta para `Rueda`: la `RuedaFina` de las ventanas con gestos
    /// tactiles acerca con el raton en cualquier lienzo (`decidir_rueda`).
    pub mapa: bool,
    clasificador: Clasificador,
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
            // El de la rueda de Excalidraw, que ya era el suelo de hecho: la
            // camara del motor admite 0,05 pero la rueda nunca bajaba de 0,1.
            zoom_minimo: ZOOM_MINIMO,
            mapa: false,
            clasificador: Clasificador::default(),
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

    /// Los codigos virtuales que pueden estar sujetando el arrastre en
    /// curso: vacio si no hay arrastre. `Principal` da DOS codigos -el
    /// izquierdo y el derecho-, porque `overlay.rs` traduce ambos botones al
    /// mismo `BotonPulsado` (revision 2): quien llama tiene que mirar si
    /// CUALQUIERA de los dos sigue pulsado, no solo el primero, o un
    /// arrastre sujeto con el derecho se daria por soltado de mentira en el
    /// primer `RatonMovido`. `Central` da uno solo. Quien llama sondea cada
    /// codigo con `pixpin_shell::entrada::tecla_pulsada_ahora` antes de
    /// mandar un `RatonMovido`/`Muestra` (fix 2).
    pub fn botones_en_arrastre(&self) -> &'static [u32] {
        match self.arrastre.map(|a| a.boton) {
            Some(BotonArrastre::Principal) => &[VK_BOTON_PRINCIPAL, VK_BOTON_SECUNDARIO],
            Some(BotonArrastre::Central) => &[VK_BOTON_CENTRAL],
            None => &[],
        }
    }

    /// `en_reposo` es `Gesto::en_reposo`: con un trazo o un arrastre en curso
    /// no se empieza a mover el lienzo (el clic es del gesto). `vivo` es el
    /// sondeo en vivo que documenta `EnVivo`: solo se mira en los dos casos
    /// donde hace falta.
    pub fn evento(
        &mut self,
        ev: &EventoOverlay,
        origen: Punto,
        escala_por_cien: u32,
        m: Modificadores,
        en_reposo: bool,
        vivo: EnVivo,
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
                if !vivo.espacio {
                    // Fix 1 (revision 1): el espacio ya no esta pulsado de
                    // verdad -Alt+Tab, o se solto dentro del bucle propio de
                    // `pedir_medida`-, aunque nunca llegara su
                    // `TeclaSoltada`. Sin este sondeo el clic habria
                    // arrancado un arrastre con una tecla ya levantada.
                    self.espacio = false;
                    return Respuesta::default();
                }
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
                        if !vivo.boton_arrastre {
                            // Fix 2 (revision 1): el boton que arrastraba ya
                            // no esta pulsado -Windows le quito la captura al
                            // raton a mitad de camino- y su evento de soltar
                            // nunca va a llegar. No se consume: el
                            // movimiento vuelve a ser del gesto, para que el
                            // cursor lo refleje.
                            self.arrastre = None;
                            return Respuesta::default();
                        }
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
            // tinta -salvo que el boton ya se soltara sin avisar (fix 2)-.
            EventoOverlay::Muestra(_) if self.arrastre.is_some() => {
                if !vivo.boton_arrastre {
                    self.arrastre = None;
                    return Respuesta::default();
                }
                consumido
            }
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
                let accion = if self.mapa && !m.shift {
                    Accion::ZoomSuave {
                        foco: self.ultimo,
                        delta,
                    }
                } else if m.ctrl {
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
            // Dos dedos de lado en el panel tactil. Con el vertical da el
            // movimiento libre: Windows manda una diagonal como los dos
            // mensajes seguidos. El registro lo confirmo: `WM_MOUSEHWHEEL`
            // llegaba a la ventana y aqui no habia quien lo atendiera.
            EventoOverlay::RuedaHorizontal(delta) => Respuesta {
                consumido: true,
                accion: Some(Accion::Desplazar {
                    // Windows cuenta positivo hacia la DERECHA, que ya es el
                    // signo del `deltaX` del navegador: justo al reves que el
                    // vertical. Por eso aqui no se niega `delta_y_css`, que
                    // trae su propia negacion: scrollX - deltaX.
                    dx: delta_y_css(delta),
                    dy: 0.0,
                }),
            },
            // La rueda de una ventana con gestos tactiles: raton o panel.
            EventoOverlay::RuedaFina(r) => {
                let fuente = self.clasificador.fuente(&r);
                Respuesta {
                    consumido: true,
                    accion: (r.delta != 0).then(|| decidir_rueda(&r, fuente, self.ultimo)),
                }
            }
            // Dos dedos en el panel de precision: el lienzo sigue a los
            // dedos, en cualquier direccion a la vez, inercia incluida.
            EventoOverlay::DeslizTactil(d) => Respuesta {
                consumido: true,
                accion: Some(Accion::Desplazar {
                    dx: d.dx() / escala,
                    dy: d.dy() / escala,
                }),
            },
            // Pellizco: el punto bajo el cursor se queda quieto.
            EventoOverlay::PellizcoTactil(p) => {
                self.ultimo = Punto2::nuevo(
                    (p.en.x - origen.x) as f32 / escala,
                    (p.en.y - origen.y) as f32 / escala,
                );
                Respuesta {
                    consumido: true,
                    accion: Some(Accion::Escalar {
                        foco: self.ultimo,
                        factor: p.factor(),
                    }),
                }
            }
            _ => Respuesta::default(),
        }
    }
}

/// Aplica la accion a la camara del usuario (logica). Devuelve si cambio.
/// Con el minimo de zoom del motor: es lo que quiere todo lienzo que no sea
/// el universo.
#[cfg(test)]
pub fn aplicar(camara: &mut Camara, accion: Accion) -> bool {
    aplicar_con_minimo(camara, accion, ZOOM_MINIMO)
}

/// Como `aplicar`, con el tope de alejamiento que diga quien llama (el
/// `zoom_minimo` de su `Navegador`).
pub fn aplicar_con_minimo(camara: &mut Camara, accion: Accion, zoom_minimo: f32) -> bool {
    match accion {
        Accion::Desplazar { dx, dy } => {
            if dx == 0.0 && dy == 0.0 {
                return false;
            }
            camara.desplazar(dx, dy);
            true
        }
        Accion::ZoomSuave { foco, delta } => camara.acercar_en_entre(
            foco,
            crate::universo::mapa::factor_de_rueda(delta),
            zoom_minimo,
            pixpin_motor2d::camara::ZOOM_MAXIMO,
        ),
        Accion::Escalar { foco, factor } => {
            if !factor.is_finite() || factor <= 0.0 {
                return false;
            }
            camara.acercar_en_entre(foco, factor, zoom_minimo, pixpin_motor2d::camara::ZOOM_MAXIMO)
        }
        Accion::ZoomRueda { foco, delta } => {
            let nuevo = zoom_de_rueda_entre(camara.zoom, delta, zoom_minimo);
            camara.acercar_en_entre(
                foco,
                nuevo / camara.zoom,
                zoom_minimo,
                pixpin_motor2d::camara::ZOOM_MAXIMO,
            )
        }
    }
}

/// Si ALGUNO de `botones` esta pulsado, segun `pulsado`. Separada de
/// `Navegador::evento` para poder probar el "o" sin Windows: quien la llama
/// de verdad (`ventana_editor.rs`) pasa
/// `pixpin_shell::entrada::tecla_pulsada_ahora`; las pruebas de aqui abajo
/// inyectan una funcion de mentira. Hace falta el "o" y no basta con mirar
/// un solo codigo porque `overlay.rs` traduce el clic derecho al mismo
/// `EventoOverlay::BotonPulsado` que el izquierdo (revision 2, hallazgo
/// HIGH): un espacio+arrastre sujeto con el derecho no puede darse por
/// soltado solo porque el izquierdo no lo este.
pub fn algun_boton_pulsado(botones: &[u32], pulsado: impl Fn(u32) -> bool) -> bool {
    botones.iter().any(|vk| pulsado(*vk))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_shell::gestos_tactiles::{Desliz, Pellizco};

    const ORIGEN: Punto = Punto { x: 0, y: 0 };
    const NADA: Modificadores = Modificadores {
        ctrl: false,
        shift: false,
    };
    /// El sondeo en vivo cuando no hace falta desmentir a nadie: la tecla o
    /// el boton siguen pulsados, como asumian las pruebas de antes de la
    /// revision 1.
    const SIEMPRE: EnVivo = EnVivo {
        espacio: true,
        boton_arrastre: true,
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
    fn con_un_minimo_propio_la_rueda_baja_del_diez_por_ciento_a_pasos_proporcionales() {
        // El universo: de 0,1 hacia fuera sigue alejando, un 10 % por muesca.
        let z = zoom_de_rueda_entre(0.1, -MUESCA, 0.002);
        assert!((z - 0.1 / 1.1).abs() < 1e-5, "{z}");
        // Y de vuelta hacia dentro, proporcional tambien.
        let v = zoom_de_rueda_entre(0.01, MUESCA, 0.002);
        assert!((v - 0.011).abs() < 1e-5, "{v}");
        // Nunca por debajo de su minimo.
        assert_eq!(zoom_de_rueda_entre(0.002, -MUESCA, 0.002), 0.002);
        // Caso negativo: con el minimo de siempre, es la rueda de Excalidraw.
        for z in [0.1, 0.5, 1.0, 3.0] {
            for d in [-MUESCA, MUESCA, -3 * MUESCA] {
                assert_eq!(zoom_de_rueda_entre(z, d, ZOOM_MINIMO), zoom_de_rueda(z, d));
            }
        }
    }

    #[test]
    fn en_modo_mapa_la_rueda_acerca_donde_esta_el_raton_y_con_shift_sigue_desplazando() {
        let mut n = Navegador::nuevo();
        n.mapa = true;
        n.evento(
            &EventoOverlay::RatonMovido(Punto { x: 300, y: 150 }),
            ORIGEN,
            150,
            NADA,
            true,
            SIEMPRE,
        );
        let r = n.evento(
            &EventoOverlay::Rueda(MUESCA),
            ORIGEN,
            150,
            NADA,
            true,
            SIEMPRE,
        );
        assert_eq!(
            r.accion,
            Some(Accion::ZoomSuave {
                foco: Punto2::nuevo(200.0, 100.0),
                delta: MUESCA
            })
        );
        // Caso negativo: con Shift la rueda es horizontal tambien en el mapa.
        let shift = Modificadores {
            ctrl: false,
            shift: true,
        };
        let r = n.evento(
            &EventoOverlay::Rueda(MUESCA),
            ORIGEN,
            100,
            shift,
            true,
            SIEMPRE,
        );
        assert!(matches!(r.accion, Some(Accion::Desplazar { dy: 0.0, .. })));
    }

    #[test]
    fn un_lienzo_normal_no_esta_en_modo_mapa_y_su_rueda_no_cambia() {
        // Lo que vigila que el universo no se lleve la rueda de todos.
        let n = Navegador::nuevo();
        assert!(!n.mapa);
        for mods in [
            NADA,
            Modificadores {
                ctrl: true,
                shift: false,
            },
        ] {
            let mut n = Navegador::nuevo();
            let r = n.evento(
                &EventoOverlay::Rueda(MUESCA),
                ORIGEN,
                100,
                mods,
                true,
                SIEMPRE,
            );
            assert!(
                !matches!(r.accion, Some(Accion::ZoomSuave { .. })),
                "{mods:?}"
            );
        }
    }

    #[test]
    fn la_rueda_sola_desplaza_en_vertical_como_excalidraw() {
        let mut n = Navegador::nuevo();
        let r = n.evento(
            &EventoOverlay::Rueda(-MUESCA),
            ORIGEN,
            100,
            NADA,
            true,
            SIEMPRE,
        );
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
        let r = n.evento(
            &EventoOverlay::Rueda(-MUESCA),
            ORIGEN,
            100,
            shift,
            true,
            SIEMPRE,
        );
        assert_eq!(
            r.accion,
            Some(Accion::Desplazar {
                dx: -100.0,
                dy: 0.0
            })
        );
    }

    #[test]
    fn dos_dedos_a_la_derecha_llevan_la_vista_a_la_derecha() {
        let mut n = Navegador::nuevo();
        // Positivo es a la derecha en `WM_MOUSEHWHEEL`: el contenido se va a
        // la izquierda, igual que con Shift y la rueda hacia abajo.
        let r = n.evento(
            &EventoOverlay::RuedaHorizontal(MUESCA),
            ORIGEN,
            100,
            Modificadores::default(),
            true,
            SIEMPRE,
        );
        assert!(r.consumido);
        assert_eq!(
            r.accion,
            Some(Accion::Desplazar {
                dx: -100.0,
                dy: 0.0
            })
        );
    }

    #[test]
    fn dos_dedos_de_lado_no_hacen_zoom_ni_con_control() {
        let mut n = Navegador::nuevo();
        let ctrl = Modificadores {
            ctrl: true,
            shift: false,
        };
        let r = n.evento(
            &EventoOverlay::RuedaHorizontal(-MUESCA),
            ORIGEN,
            100,
            ctrl,
            true,
            SIEMPRE,
        );
        assert_eq!(r.accion, Some(Accion::Desplazar { dx: 100.0, dy: 0.0 }));
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
            SIEMPRE,
        );
        let ctrl = Modificadores {
            ctrl: true,
            shift: false,
        };
        let r = n.evento(
            &EventoOverlay::Rueda(MUESCA),
            ORIGEN,
            100,
            ctrl,
            true,
            SIEMPRE,
        );
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
        assert!(
            n.evento(&espacio, ORIGEN, 100, NADA, true, SIEMPRE)
                .consumido
        );
        let p = n.evento(
            &EventoOverlay::BotonPulsado(Punto { x: 100, y: 100 }),
            ORIGEN,
            100,
            NADA,
            true,
            SIEMPRE,
        );
        assert!(p.consumido && n.arrastrando());
        let m = n.evento(
            &EventoOverlay::RatonMovido(Punto { x: 130, y: 110 }),
            ORIGEN,
            100,
            NADA,
            true,
            SIEMPRE,
        );
        assert!(m.consumido);
        assert_eq!(m.accion, Some(Accion::Desplazar { dx: 30.0, dy: 10.0 }));
        let s = n.evento(
            &EventoOverlay::BotonSoltado(Punto { x: 130, y: 110 }),
            ORIGEN,
            100,
            NADA,
            true,
            SIEMPRE,
        );
        assert!(s.consumido && !n.arrastrando());
        let _ = n.evento(
            &EventoOverlay::TeclaSoltada(VK_ESPACIO),
            ORIGEN,
            100,
            NADA,
            true,
            SIEMPRE,
        );
        // Soltado el espacio, el siguiente clic vuelve a ser del gesto.
        let otro = n.evento(
            &EventoOverlay::BotonPulsado(Punto { x: 5, y: 5 }),
            ORIGEN,
            100,
            NADA,
            true,
            SIEMPRE,
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
            SIEMPRE,
        );
        let m = n.evento(
            &EventoOverlay::RatonMovido(Punto { x: 4, y: 30 }),
            ORIGEN,
            100,
            NADA,
            true,
            SIEMPRE,
        );
        assert_eq!(m.accion, Some(Accion::Desplazar { dx: -6.0, dy: 20.0 }));
        let s = n.evento(
            &EventoOverlay::BotonCentralSoltado(Punto { x: 4, y: 30 }),
            ORIGEN,
            100,
            NADA,
            true,
            SIEMPRE,
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
            let r = n.evento(&ev, ORIGEN, 100, NADA, true, SIEMPRE);
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
        let _ = n.evento(&espacio, ORIGEN, 100, NADA, false, SIEMPRE);
        let p = n.evento(
            &EventoOverlay::BotonPulsado(Punto { x: 1, y: 1 }),
            ORIGEN,
            100,
            NADA,
            false,
            SIEMPRE,
        );
        assert!(!p.consumido && !n.arrastrando());
    }

    #[test]
    fn el_espacio_soltado_sin_evento_no_roba_el_clic() {
        // Revision 1, fix 1: Alt+Tab (o el bucle propio de `pedir_medida`)
        // suelta el espacio sin mandar `TeclaSoltada`. El sondeo en vivo
        // tiene que desmentir a `self.espacio` en el siguiente clic.
        let mut n = Navegador::nuevo();
        let espacio = EventoOverlay::Tecla {
            vk: VK_ESPACIO,
            shift: false,
            ctrl: false,
            alt: false,
        };
        let _ = n.evento(&espacio, ORIGEN, 100, NADA, true, SIEMPRE);
        let vivo = EnVivo {
            espacio: false,
            boton_arrastre: true,
        };
        let r = n.evento(
            &EventoOverlay::BotonPulsado(Punto { x: 1, y: 1 }),
            ORIGEN,
            100,
            NADA,
            true,
            vivo,
        );
        assert!(!r.consumido && !n.arrastrando());
        // Y queda limpio de verdad: un clic posterior con SIEMPRE tampoco
        // arrastra (si `self.espacio` no se hubiera limpiado, este si).
        let otro = n.evento(
            &EventoOverlay::BotonPulsado(Punto { x: 2, y: 2 }),
            ORIGEN,
            100,
            NADA,
            true,
            SIEMPRE,
        );
        assert!(!otro.consumido && !n.arrastrando());
    }

    #[test]
    fn el_boton_soltado_sin_evento_termina_el_arrastre_sin_consumir() {
        // Revision 1, fix 2: Windows le quita la captura al raton a mitad de
        // un arrastre y el `BotonCentralSoltado` nunca llega. El sondeo en
        // vivo tiene que cerrar el arrastre solo, y devolver el movimiento
        // al gesto (no consumirlo) para que el cursor lo refleje.
        let mut n = Navegador::nuevo();
        let _ = n.evento(
            &EventoOverlay::BotonCentralPulsado(Punto { x: 10, y: 10 }),
            ORIGEN,
            100,
            NADA,
            true,
            SIEMPRE,
        );
        assert!(n.arrastrando());
        assert_eq!(n.botones_en_arrastre(), &[0x04]);
        let vivo = EnVivo {
            espacio: true,
            boton_arrastre: false,
        };
        let m = n.evento(
            &EventoOverlay::RatonMovido(Punto { x: 40, y: 40 }),
            ORIGEN,
            100,
            NADA,
            true,
            vivo,
        );
        assert!(!m.consumido && m.accion.is_none() && !n.arrastrando());
    }

    #[test]
    fn los_botones_en_arrastre_son_los_del_boton_que_lo_empezo() {
        let mut n = Navegador::nuevo();
        assert_eq!(n.botones_en_arrastre(), &[] as &[u32]);
        let espacio = EventoOverlay::Tecla {
            vk: VK_ESPACIO,
            shift: false,
            ctrl: false,
            alt: false,
        };
        let _ = n.evento(&espacio, ORIGEN, 100, NADA, true, SIEMPRE);
        let _ = n.evento(
            &EventoOverlay::BotonPulsado(Punto { x: 1, y: 1 }),
            ORIGEN,
            100,
            NADA,
            true,
            SIEMPRE,
        );
        // Principal da los DOS codigos: overlay.rs traduce el clic derecho
        // al mismo `BotonPulsado` que el izquierdo (revision 2).
        assert_eq!(n.botones_en_arrastre(), &[0x01, 0x02]);
    }

    #[test]
    fn algun_boton_pulsado_es_un_o_no_un_y() {
        // Revision 2, hallazgo HIGH: `overlay.rs` traduce WM_RBUTTONDOWN al
        // mismo `BotonPulsado` que el izquierdo, asi que un espacio+arrastre
        // puede estar sujeto por CUALQUIERA de los dos. `algun_boton_pulsado`
        // es la funcion pura que hace ese "o" -inyectando `pulsado` en vez de
        // llamar a Windows- para poder probarla sin sesion de escritorio.
        let principal = Navegador {
            arrastre: Some(Arrastre {
                boton: BotonArrastre::Principal,
                anterior: ORIGEN,
            }),
            ..Navegador::nuevo()
        };
        // Solo el derecho pulsado: el arrastre sigue vivo.
        assert!(algun_boton_pulsado(
            principal.botones_en_arrastre(),
            |vk| vk == VK_BOTON_SECUNDARIO
        ));
        // Solo el izquierdo pulsado: tambien.
        assert!(algun_boton_pulsado(
            principal.botones_en_arrastre(),
            |vk| vk == VK_BOTON_PRINCIPAL
        ));
        // Ninguno de los dos: se acabo.
        assert!(!algun_boton_pulsado(
            principal.botones_en_arrastre(),
            |_| false
        ));

        let central = Navegador {
            arrastre: Some(Arrastre {
                boton: BotonArrastre::Central,
                anterior: ORIGEN,
            }),
            ..Navegador::nuevo()
        };
        // El central no cambia: un solo codigo, sin "o" que valga.
        assert!(algun_boton_pulsado(central.botones_en_arrastre(), |vk| vk == VK_BOTON_CENTRAL));
        assert!(!algun_boton_pulsado(central.botones_en_arrastre(), |_| {
            false
        }));
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
            SIEMPRE,
        );
        let m = n.evento(
            &EventoOverlay::RatonMovido(Punto { x: 150, y: 0 }),
            ORIGEN,
            150,
            NADA,
            true,
            SIEMPRE,
        );
        let mut c = Camara::nueva();
        let punto = Punto2::nuevo(40.0, 0.0);
        let antes = vista_efectiva(&c, 150).a_pantalla(punto);
        assert!(aplicar(&mut c, m.accion.unwrap()));
        let despues = vista_efectiva(&c, 150).a_pantalla(punto);
        assert!((despues.x - antes.x - 150.0).abs() < 1e-3);
    }

    // --- Raton o panel tactil (`RuedaFina`, `DeslizTactil`, `PellizcoTactil`).

    fn rueda(delta: i32, ms: u32) -> EventoOverlay {
        EventoOverlay::RuedaFina(RuedaCruda {
            delta,
            horizontal: false,
            ms,
            ctrl: false,
            shift: false,
        })
    }

    fn con_raton_en(n: &mut Navegador, x: i32, y: i32, escala: u32) {
        let _ = n.evento(
            &EventoOverlay::RatonMovido(Punto { x, y }),
            ORIGEN,
            escala,
            NADA,
            true,
            SIEMPRE,
        );
    }

    fn ev(n: &mut Navegador, e: EventoOverlay) -> Respuesta {
        n.evento(&e, ORIGEN, 100, NADA, true, SIEMPRE)
    }

    #[test]
    fn la_rueda_de_un_raton_acerca_hacia_el_cursor_sin_ctrl() {
        let mut n = Navegador::nuevo();
        con_raton_en(&mut n, 300, 150, 150);
        let r = n.evento(&rueda(MUESCA, 1000), ORIGEN, 150, NADA, true, SIEMPRE);
        assert!(r.consumido);
        let Some(accion @ Accion::ZoomSuave { foco, delta }) = r.accion else {
            panic!("la rueda del raton es zoom, dio {:?}", r.accion);
        };
        assert_eq!((foco, delta), (Punto2::nuevo(200.0, 100.0), MUESCA));
        let mut c = Camara::nueva();
        let antes = c.a_mundo(foco);
        assert!(aplicar(&mut c, accion));
        assert!(c.zoom > 1.0, "rueda arriba acerca");
        let despues = c.a_mundo(foco);
        assert!((antes.x - despues.x).abs() < 1e-3 && (antes.y - despues.y).abs() < 1e-3);
        // Y hacia abajo aleja.
        let r = ev(&mut n, rueda(-MUESCA, 1100));
        assert!(matches!(r.accion, Some(Accion::ZoomSuave { delta, .. }) if delta < 0));
    }

    #[test]
    fn una_rueda_de_muescas_nunca_se_toma_por_panel_aunque_gire_muy_rapido() {
        // Una rueda libre (o una muesca doble, 240) manda multiplos de 120
        // cada pocos milisegundos: sigue siendo zoom, nunca desplazar.
        let mut n = Navegador::nuevo();
        for (i, d) in [120, 120, 240, -120, 360, -240, 120].into_iter().enumerate() {
            let r = ev(&mut n, rueda(d, 5000 + i as u32));
            assert!(
                matches!(r.accion, Some(Accion::ZoomSuave { .. })),
                "muesca {d}: {:?}",
                r.accion
            );
        }
    }

    #[test]
    fn dos_dedos_en_un_panel_sin_precision_desplazan_y_nunca_acercan() {
        // El panel viejo manda la rueda a trozos (no multiplos de 120) y a
        // veces un 120 justo en medio: dentro de la racha sigue siendo panel.
        let mut n = Navegador::nuevo();
        for (d, ms) in [(-17, 100), (-33, 110), (-120, 120), (-8, 135), (-120, 300)] {
            let r = ev(&mut n, rueda(d, ms));
            let Some(Accion::Desplazar { dx, dy }) = r.accion else {
                panic!("{d} a los {ms} ms: {:?}", r.accion);
            };
            assert_eq!(dx, 0.0);
            // Convenio de `Camara::desplazar`: negativo = mirar mas abajo.
            assert!(dy < 0.0, "giro abajo: mirar mas abajo, {dy}");
        }
    }

    #[test]
    fn tras_una_pausa_la_rueda_de_muescas_vuelve_a_ser_del_raton() {
        let mut n = Navegador::nuevo();
        let _ = ev(&mut n, rueda(-17, 1000));
        let r = ev(&mut n, rueda(MUESCA, 1000 + RACHA_PANEL_MS + 1));
        assert!(matches!(r.accion, Some(Accion::ZoomSuave { .. })), "{:?}", r.accion);
    }

    #[test]
    fn la_racha_del_panel_sobrevive_a_la_vuelta_del_reloj_de_windows() {
        let mut n = Navegador::nuevo();
        let _ = ev(&mut n, rueda(-17, u32::MAX - 10));
        let r = ev(&mut n, rueda(-MUESCA, 20));
        assert!(matches!(r.accion, Some(Accion::Desplazar { .. })), "{:?}", r.accion);
    }

    #[test]
    fn control_y_rueda_es_zoom_venga_del_raton_o_del_pellizco_de_un_panel_viejo() {
        let mut n = Navegador::nuevo();
        for d in [MUESCA, 13, -7] {
            let r = ev(
                &mut n,
                EventoOverlay::RuedaFina(RuedaCruda {
                    delta: d,
                    horizontal: false,
                    ms: 10,
                    ctrl: true,
                    shift: false,
                }),
            );
            assert!(matches!(r.accion, Some(Accion::ZoomSuave { delta, .. }) if delta == d));
        }
    }

    #[test]
    fn shift_y_la_rueda_del_raton_desplaza_en_horizontal() {
        let mut n = Navegador::nuevo();
        let r = ev(
            &mut n,
            EventoOverlay::RuedaFina(RuedaCruda {
                delta: -MUESCA,
                horizontal: false,
                ms: 10,
                ctrl: false,
                shift: true,
            }),
        );
        assert_eq!(r.accion, Some(Accion::Desplazar { dx: -100.0, dy: 0.0 }));
    }

    #[test]
    fn la_rueda_horizontal_desplaza_de_lado_aunque_lleve_control() {
        let mut n = Navegador::nuevo();
        for ctrl in [false, true] {
            let r = ev(
                &mut n,
                EventoOverlay::RuedaFina(RuedaCruda {
                    delta: MUESCA,
                    horizontal: true,
                    ms: 10,
                    ctrl,
                    shift: false,
                }),
            );
            assert_eq!(r.accion, Some(Accion::Desplazar { dx: -100.0, dy: 0.0 }), "{ctrl}");
        }
    }

    #[test]
    fn una_rueda_fina_sin_giro_no_hace_nada_pero_no_se_escapa_al_gesto() {
        let mut n = Navegador::nuevo();
        let r = ev(&mut n, rueda(0, 10));
        assert!(r.consumido);
        assert_eq!(r.accion, None);
    }

    #[test]
    fn el_desliz_tactil_mueve_el_lienzo_en_diagonal_pixel_a_pixel_a_escala_150() {
        let mut n = Navegador::nuevo();
        let r = n.evento(
            &EventoOverlay::DeslizTactil(Desliz::nuevo(30.0, -15.0)),
            ORIGEN,
            150,
            NADA,
            true,
            SIEMPRE,
        );
        assert!(r.consumido);
        assert_eq!(r.accion, Some(Accion::Desplazar { dx: 20.0, dy: -10.0 }));
        let mut c = Camara::nueva();
        let p = Punto2::nuevo(5.0, 5.0);
        let antes = vista_efectiva(&c, 150).a_pantalla(p);
        assert!(aplicar(&mut c, r.accion.unwrap()));
        let despues = vista_efectiva(&c, 150).a_pantalla(p);
        assert!((despues.x - antes.x - 30.0).abs() < 1e-3);
        assert!((despues.y - antes.y + 15.0).abs() < 1e-3);
        // Nunca zoom al deslizar.
        assert_eq!(c.zoom, 1.0);
    }

    #[test]
    fn el_pellizco_acerca_con_el_punto_del_cursor_quieto() {
        let mut n = Navegador::nuevo();
        let r = n.evento(
            &EventoOverlay::PellizcoTactil(Pellizco::nuevo(1.5, Punto { x: 450, y: 300 })),
            Punto { x: 150, y: 0 },
            150,
            NADA,
            true,
            SIEMPRE,
        );
        let Some(accion @ Accion::Escalar { foco, factor }) = r.accion else {
            panic!("el pellizco es zoom, dio {:?}", r.accion);
        };
        assert_eq!(foco, Punto2::nuevo(200.0, 200.0));
        assert!((factor - 1.5).abs() < 1e-5);
        let mut c = Camara::nueva();
        let antes = c.a_mundo(foco);
        assert!(aplicar(&mut c, accion));
        assert!((c.zoom - 1.5).abs() < 1e-4);
        let despues = c.a_mundo(foco);
        assert!((antes.x - despues.x).abs() < 1e-3 && (antes.y - despues.y).abs() < 1e-3);
    }

    #[test]
    fn el_pellizco_no_pasa_de_los_limites_de_zoom() {
        let foco = Punto2::nuevo(0.0, 0.0);
        let mut c = Camara::nueva();
        c.zoom = ZOOM_MAXIMO;
        assert!(!aplicar(&mut c, Accion::Escalar { foco, factor: 2.0 }));
        assert_eq!(c.zoom, ZOOM_MAXIMO);
        c.zoom = ZOOM_MINIMO;
        assert!(!aplicar(&mut c, Accion::Escalar { foco, factor: 0.5 }));
        assert_eq!(c.zoom, ZOOM_MINIMO);
    }

    #[test]
    fn la_rueda_del_raton_no_pasa_de_los_limites_de_zoom() {
        let foco = Punto2::nuevo(0.0, 0.0);
        let mut c = Camara::nueva();
        c.zoom = ZOOM_MAXIMO;
        let _ = aplicar(&mut c, Accion::ZoomSuave { foco, delta: MUESCA });
        assert!(c.zoom <= ZOOM_MAXIMO);
        c.zoom = ZOOM_MINIMO;
        let _ = aplicar(&mut c, Accion::ZoomSuave { foco, delta: -MUESCA });
        assert!(c.zoom >= ZOOM_MINIMO);
    }

    #[test]
    fn el_boton_central_sigue_arrastrando_con_los_gestos_tactiles_pedidos() {
        // Caso negativo: los eventos nuevos no le quitan la mano al central.
        let mut n = Navegador::nuevo();
        let _ = ev(&mut n, rueda(-17, 1));
        let _ = ev(&mut n, EventoOverlay::BotonCentralPulsado(Punto { x: 0, y: 0 }));
        let m = ev(&mut n, EventoOverlay::RatonMovido(Punto { x: 7, y: -3 }));
        assert_eq!(m.accion, Some(Accion::Desplazar { dx: 7.0, dy: -3.0 }));
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
