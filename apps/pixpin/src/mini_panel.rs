//! **Que ensena y que hace el panel de cada mini-aplicacion.**
//!
//! En el movil esto es `mini/MiniActivity.kt`: una sola pantalla con un
//! despacho de siete casos (:155-163). Aqui esta lo mismo, pero **sin
//! ventana**: este modulo traduce un documento a «que se ve» ([`Vista`]) y un
//! boton pulsado a «que documento queda» ([`aplicar`]). Asi las siete
//! mini-apps se prueban enteras en `cargo test`, y `ventana_chat.rs` solo
//! tiene que pintar rectangulos y traducir rotulos.
//!
//! ## Por que los rotulos no son texto ya traducido
//!
//! Un [`Rotulo`] es una clave del catalogo o un numero literal. Los numeros
//! («−5», «12») no se traducen: son el propio dato. Lo demas sale del `.ftl`,
//! y este modulo no conoce el `Catalogo` a proposito: cargarlo aqui obligaria
//! a montar el idioma en cada prueba para comprobar una resta de minutos.
//!
//! ## Por que `ahora` se pasa y no se lee del reloj
//!
//! Un cronometro y un temporizador guardan **el instante de arranque**, no el
//! numero visible (`mini/Tiempos.kt:12-21`). Leyendo el reloj aqui dentro no
//! habria forma de comprobar que «parar» suma lo que llevaba: la prueba
//! tendria que dormir de verdad.

use pixpin_proyecto::mini::{self, gastos, tiempos};
use pixpin_ui::mini::Reparto;

/// Un rotulo de boton: o una clave del catalogo, o el dato mismo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rotulo {
    Clave(&'static str),
    Tal(String),
}

/// Lo que hace un boton o un toque en la lista.
///
/// Todas las ordenes llevan lo que necesitan dentro: `aplicar` no guarda
/// estado entre llamadas, y eso es lo que permite que el documento del disco
/// sea siempre la unica verdad.
#[derive(Debug, Clone, PartialEq)]
pub enum Orden {
    /// Marcar o desmarcar una tarea.
    Alternar(usize),
    /// Quitar la linea `n` de la lista.
    Quitar(usize),
    /// Meter lo tecleado en la caja de abajo.
    Anadir(String),
    /// Tareas: tirar las que ya estan hechas.
    LimpiarHechas,
    /// Cronometro y temporizador: el boton grande.
    ArrancarOParar,
    Vuelta,
    Reiniciar,
    /// Temporizador: sumar o restar minutos a la duracion.
    Minutos(i64),
    /// Contador.
    Mas,
    Menos,
    ConPaso(i64),
    /// Ruleta: el azar viene de fuera para poder probar el sorteo.
    Girar(f64),
    /// Alarma: encenderla o apagarla, y mover la hora.
    Activar,
    Horas(i64),
    MinutosDeAlarma(i64),
}

/// Un boton del panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Boton {
    pub rotulo: Rotulo,
    pub orden: Orden,
    /// Apagado se pinta, pero no se pulsa: «Vuelta» con el cronometro parado
    /// tiene que verse para que se sepa que existe (`MiniActivity.kt:404`).
    pub activo: bool,
    /// Se pinta relleno, no de contorno: es el boton principal de la fila.
    pub principal: bool,
}

/// Una linea de la lista.
#[derive(Debug, Clone, PartialEq)]
pub struct FilaLista {
    pub texto: String,
    /// A la derecha: el importe de un gasto, o el tiempo de una vuelta.
    pub detalle: String,
    /// Tachada y apagada: una tarea hecha.
    pub hecha: bool,
    /// Si se puede marcar pulsandola.
    pub se_marca: bool,
    /// Si lleva aspa de borrar al final.
    pub se_borra: bool,
}

/// Todo lo que el panel ensena de una mini-app en este instante.
#[derive(Debug, Clone, PartialEq)]
pub struct Vista {
    pub reparto: Reparto,
    /// El numero grande. Vacio cuando la mini-app no tiene tablero.
    pub tablero: String,
    /// El tablero en rojo: un temporizador vencido (`MiniActivity.kt:446-453`).
    pub alerta: bool,
    /// Los botones, por filas.
    pub filas_de_botones: Vec<Vec<Boton>>,
    pub lista: Vec<FilaLista>,
    /// La clave del texto guia de la caja de abajo, si se puede anadir.
    pub guia: Option<&'static str>,
    /// Cada cuanto hay que repintar sin que nadie toque nada, en
    /// milisegundos. `None` es «esto no se mueve solo».
    ///
    /// El movil repinta el cronometro cada 100 ms y el temporizador cada 200
    /// (`MiniActivity.kt:369-374, 434-440`), y **sin tocar disco**: lo que
    /// cambia es el numero que se calcula, no el documento.
    pub late_cada_ms: Option<u64>,
}

/// Cuanto vale un paso de la fila de pasos del contador
/// (`MiniActivity.kt:600-611`).
pub const PASOS: [i64; 4] = [1, 5, 10, 12];

/// Lo que ensena el panel de esa mini-app, o `None` si no la conocemos.
///
/// `None` y no un panel vacio: una mini-app de una version futura tiene que
/// quedarse como texto en la lista, no abrir una pantalla en blanco que al
/// guardar pisaria su documento.
pub fn vista(cual: &str, documento: &str, moneda: &gastos::Moneda, ahora: i64) -> Option<Vista> {
    Some(match cual {
        mini::TAREAS => de_tareas(documento),
        mini::GASTOS => de_gastos(documento, moneda),
        mini::CRONOMETRO => de_cronometro(documento, ahora),
        mini::TEMPORIZADOR => de_temporizador(documento, ahora),
        mini::CONTADOR => de_contador(documento),
        mini::RULETA => de_ruleta(documento),
        mini::ALARMA => de_alarma(documento),
        _ => return None,
    })
}

/// El documento que queda tras pulsar. Devuelve el mismo si la orden no va
/// con esa mini-app: un boton de otra pantalla no puede borrar datos.
pub fn aplicar(
    cual: &str,
    documento: &str,
    moneda: &gastos::Moneda,
    orden: &Orden,
    ahora: i64,
) -> String {
    match cual {
        mini::TAREAS => tareas(documento, orden),
        mini::GASTOS => gastos_op(documento, moneda, orden),
        mini::CRONOMETRO => cronometro(documento, orden, ahora),
        mini::TEMPORIZADOR => temporizador(documento, orden, ahora),
        mini::CONTADOR => contador(documento, orden),
        mini::RULETA => ruleta(documento, orden),
        mini::ALARMA => alarma(documento, orden),
        _ => documento.to_string(),
    }
}

// --- Tareas -----------------------------------------------------------

fn de_tareas(documento: &str) -> Vista {
    let tareas = mini::leer_tareas(documento);
    let hay_hechas = tareas.iter().any(|t| t.hecha);
    Vista {
        reparto: Reparto {
            con_tablero: false,
            filas_de_botones: 1,
            con_lista: true,
            con_anadir: true,
        },
        tablero: String::new(),
        alerta: false,
        filas_de_botones: vec![vec![Boton {
            rotulo: Rotulo::Clave("mini-limpiar-hechas"),
            orden: Orden::LimpiarHechas,
            activo: hay_hechas,
            principal: false,
        }]],
        lista: tareas
            .iter()
            .map(|t| FilaLista {
                texto: t.texto.clone(),
                detalle: String::new(),
                hecha: t.hecha,
                se_marca: true,
                se_borra: true,
            })
            .collect(),
        guia: Some("mini-tarea-nueva"),
        late_cada_ms: None,
    }
}

fn tareas(documento: &str, orden: &Orden) -> String {
    match orden {
        Orden::Alternar(n) => mini::alternar(documento, *n),
        Orden::Anadir(t) => mini::anadir(documento, t),
        Orden::Quitar(n) => {
            let mut v = mini::leer_tareas(documento);
            if *n >= v.len() {
                return documento.to_string();
            }
            v.remove(*n);
            mini::escribir_tareas(&mini::titulo(documento), &v)
        }
        Orden::LimpiarHechas => {
            let v: Vec<_> = mini::leer_tareas(documento)
                .into_iter()
                .filter(|t| !t.hecha)
                .collect();
            mini::escribir_tareas(&mini::titulo(documento), &v)
        }
        _ => documento.to_string(),
    }
}

// --- Gastos -----------------------------------------------------------

fn de_gastos(documento: &str, por_defecto: &gastos::Moneda) -> Vista {
    let libro = gastos::leer(documento, por_defecto);
    Vista {
        reparto: Reparto {
            con_tablero: true,
            filas_de_botones: 0,
            con_lista: true,
            con_anadir: true,
        },
        tablero: gastos::texto_de_importe(libro.total(), &libro.moneda),
        alerta: false,
        filas_de_botones: Vec::new(),
        lista: libro
            .gastos
            .iter()
            .map(|g| FilaLista {
                texto: g.concepto.clone(),
                detalle: gastos::texto_de_importe(g.centimos, &libro.moneda),
                hecha: false,
                se_marca: false,
                se_borra: true,
            })
            .collect(),
        guia: Some("mini-gasto-nuevo"),
        late_cada_ms: None,
    }
}

/// Parte «Cena 42,50» en concepto e importe.
///
/// El movil tiene dos cajas y un boton de signo (`MiniActivity.kt:298-318`)
/// porque su teclado decimal no trae el menos. Aqui hay teclado entero, asi
/// que se teclea de un tiron y **el importe es lo ultimo**, como en la propia
/// tabla del documento. Si lo ultimo no es un numero, entra todo como
/// concepto con importe cero: mejor apuntar el concepto y corregir la cifra
/// que perder la linea por una coma mal puesta.
pub fn partir_gasto(texto: &str, decimales: u8) -> (String, i64) {
    let limpio = texto.trim();
    if let Some((concepto, cifra)) = limpio.rsplit_once(char::is_whitespace)
        && let Some(centimos) = gastos::centimos_de(cifra, decimales)
        && !concepto.trim().is_empty()
    {
        return (concepto.trim().to_string(), centimos);
    }
    (limpio.to_string(), 0)
}

fn gastos_op(documento: &str, por_defecto: &gastos::Moneda, orden: &Orden) -> String {
    let libro = gastos::leer(documento, por_defecto);
    match orden {
        Orden::Anadir(t) => {
            let (concepto, centimos) = partir_gasto(t, libro.moneda.decimales());
            if concepto.is_empty() {
                return documento.to_string();
            }
            gastos::escribir(&gastos::anadir(&libro, &concepto, centimos))
        }
        Orden::Quitar(n) => gastos::escribir(&gastos::borrar(&libro, *n)),
        _ => documento.to_string(),
    }
}

// --- Cronometro -------------------------------------------------------

fn de_cronometro(documento: &str, ahora: i64) -> Vista {
    let c = tiempos::leer_cronometro(documento);
    let corriendo = c.corriendo();
    Vista {
        reparto: Reparto {
            con_tablero: true,
            filas_de_botones: 1,
            con_lista: true,
            con_anadir: false,
        },
        tablero: tiempos::como_se_lee(c.transcurrido(ahora)),
        alerta: false,
        filas_de_botones: vec![vec![
            Boton {
                rotulo: Rotulo::Clave(if corriendo {
                    "mini-parar"
                } else {
                    "mini-arrancar"
                }),
                orden: Orden::ArrancarOParar,
                activo: true,
                principal: true,
            },
            Boton {
                rotulo: Rotulo::Clave("mini-vuelta"),
                orden: Orden::Vuelta,
                // Solo con el cronometro en marcha, como en el movil: una
                // vuelta con el parado seria siempre la misma.
                activo: corriendo,
                principal: false,
            },
            Boton {
                rotulo: Rotulo::Clave("mini-reiniciar"),
                orden: Orden::Reiniciar,
                activo: c.llevado > 0 || corriendo || !c.vueltas.is_empty(),
                principal: false,
            },
        ]],
        // Del reves: la ultima vuelta arriba (`MiniActivity.kt:412-425`), que
        // es la que se acaba de tomar y la unica que se mira.
        lista: c
            .vueltas
            .iter()
            .enumerate()
            .rev()
            .map(|(n, ms)| FilaLista {
                texto: format!("{}", n + 1),
                detalle: tiempos::como_se_lee(*ms),
                hecha: false,
                se_marca: false,
                se_borra: false,
            })
            .collect(),
        guia: None,
        late_cada_ms: corriendo.then_some(100),
    }
}

fn cronometro(documento: &str, orden: &Orden, ahora: i64) -> String {
    let c = tiempos::leer_cronometro(documento);
    let titulo = mini::titulo(documento);
    let nuevo = match orden {
        Orden::ArrancarOParar => {
            if c.corriendo() {
                tiempos::parar(&c, ahora)
            } else {
                tiempos::arrancar(&c, ahora)
            }
        }
        Orden::Vuelta => tiempos::vuelta(&c, ahora),
        Orden::Reiniciar => tiempos::reiniciar(&c),
        _ => return documento.to_string(),
    };
    tiempos::escribir_cronometro(&titulo, &nuevo)
}

// --- Temporizador -----------------------------------------------------

fn de_temporizador(documento: &str, ahora: i64) -> Vista {
    let t = tiempos::leer_temporizador(documento);
    let corriendo = t.corriendo();
    let minutos = |m: i64| Boton {
        rotulo: Rotulo::Tal(if m < 0 {
            // El menos de verdad («−»), no el guion del teclado: es lo que
            // pinta el movil y lo que se lee como un signo.
            format!("−{}", -m)
        } else {
            format!("+{m}")
        }),
        orden: Orden::Minutos(m),
        // Con el temporizador en marcha no se cambia la duracion: cambiarla
        // a mitad de cuenta no se sabe si adelanta el final o lo aplaza.
        activo: !corriendo,
        principal: false,
    };
    Vista {
        reparto: Reparto {
            con_tablero: true,
            filas_de_botones: 2,
            con_lista: false,
            con_anadir: false,
        },
        tablero: tiempos::como_se_lee(t.restante(ahora)),
        alerta: t.vencido(ahora),
        filas_de_botones: vec![
            vec![minutos(-5), minutos(-1), minutos(1), minutos(5)],
            vec![Boton {
                rotulo: Rotulo::Clave(if corriendo {
                    "mini-parar"
                } else {
                    "mini-arrancar"
                }),
                orden: Orden::ArrancarOParar,
                activo: t.duracion > 0,
                principal: true,
            }],
        ],
        lista: Vec::new(),
        guia: None,
        late_cada_ms: corriendo.then_some(200),
    }
}

fn temporizador(documento: &str, orden: &Orden, ahora: i64) -> String {
    let t = tiempos::leer_temporizador(documento);
    let titulo = mini::titulo(documento);
    let nuevo = match orden {
        Orden::ArrancarOParar => {
            if t.corriendo() {
                tiempos::detener(&t)
            } else {
                tiempos::lanzar(&t, ahora)
            }
        }
        // En marcha no se ajusta, y por eso los cuatro botones salen
        // apagados. El nucleo si sabe relanzarlo con otra duracion
        // (`tiempos::con_duracion`), pero aqui no se usa: a mitad de cuenta
        // nadie sabe si «+5» adelanta el final o lo aplaza cinco minutos.
        Orden::Minutos(_) if t.corriendo() => return documento.to_string(),
        Orden::Minutos(m) => tiempos::con_duracion(&t, t.duracion + m * 60 * 1000, ahora),
        _ => return documento.to_string(),
    };
    tiempos::escribir_temporizador(&titulo, &nuevo)
}

// --- Contador ---------------------------------------------------------

fn de_contador(documento: &str) -> Vista {
    use pixpin_proyecto::mini::contador as c;
    let cuenta = c::leer(documento);
    let mut pasos: Vec<Boton> = PASOS
        .iter()
        .map(|p| Boton {
            rotulo: Rotulo::Tal(format!("{p}")),
            orden: Orden::ConPaso(*p),
            activo: cuenta.paso != *p,
            principal: false,
        })
        .collect();
    pasos.push(Boton {
        rotulo: Rotulo::Clave("mini-reiniciar"),
        orden: Orden::Reiniciar,
        activo: cuenta.valor != 0,
        principal: false,
    });
    Vista {
        reparto: Reparto {
            con_tablero: true,
            filas_de_botones: 2,
            con_lista: false,
            con_anadir: false,
        },
        tablero: format!("{}", cuenta.valor),
        alerta: false,
        filas_de_botones: vec![
            vec![
                Boton {
                    rotulo: Rotulo::Tal("−".into()),
                    orden: Orden::Menos,
                    activo: true,
                    principal: true,
                },
                Boton {
                    rotulo: Rotulo::Tal("+".into()),
                    orden: Orden::Mas,
                    activo: true,
                    principal: true,
                },
            ],
            pasos,
        ],
        lista: Vec::new(),
        guia: None,
        late_cada_ms: None,
    }
}

fn contador(documento: &str, orden: &Orden) -> String {
    use pixpin_proyecto::mini::contador as c;
    let cuenta = c::leer(documento);
    let titulo = mini::titulo(documento);
    let nueva = match orden {
        Orden::Mas => c::mas(&cuenta),
        Orden::Menos => c::menos(&cuenta),
        Orden::Reiniciar => c::reiniciar(&cuenta),
        Orden::ConPaso(p) => c::con_paso(&cuenta, *p),
        _ => return documento.to_string(),
    };
    c::escribir(&titulo, &nueva)
}

// --- Ruleta -----------------------------------------------------------

/// El nombre que salio en el ultimo sorteo no se guarda en el documento —el
/// movil tampoco lo guarda (`mini/Contador.kt:68-91`)—, asi que vive en el
/// panel mientras esta abierto y se pierde al cerrarlo, igual que alli.
fn de_ruleta(documento: &str) -> Vista {
    let nombres = pixpin_proyecto::mini::ruleta::leer(documento);
    Vista {
        reparto: Reparto {
            con_tablero: true,
            filas_de_botones: 1,
            con_lista: true,
            con_anadir: true,
        },
        tablero: String::new(),
        alerta: false,
        filas_de_botones: vec![vec![Boton {
            rotulo: Rotulo::Clave("mini-sortear"),
            orden: Orden::Girar(0.0),
            // Con menos de dos no hay sorteo que valga la pena
            // (`MiniActivity.kt:691-699`).
            activo: nombres.len() >= 2,
            principal: true,
        }]],
        lista: nombres
            .iter()
            .map(|n| FilaLista {
                texto: n.clone(),
                detalle: String::new(),
                hecha: false,
                se_marca: false,
                se_borra: true,
            })
            .collect(),
        guia: Some("mini-nombre-nuevo"),
        late_cada_ms: None,
    }
}

fn ruleta(documento: &str, orden: &Orden) -> String {
    use pixpin_proyecto::mini::ruleta as r;
    let nombres = r::leer(documento);
    let titulo = mini::titulo(documento);
    match orden {
        Orden::Anadir(t) => r::escribir(&titulo, &r::anadir(&nombres, t)),
        Orden::Quitar(n) => r::escribir(&titulo, &r::quitar(&nombres, *n)),
        // Girar no cambia el documento: el elegido no se guarda.
        _ => documento.to_string(),
    }
}

/// A quien le toca, de los nombres del documento. `None` si no hay ninguno.
pub fn sorteo(documento: &str, azar: f64) -> Option<usize> {
    let nombres = pixpin_proyecto::mini::ruleta::leer(documento);
    pixpin_proyecto::mini::ruleta::elegir(&nombres, azar)
}

// --- Alarma -----------------------------------------------------------

fn de_alarma(documento: &str) -> Vista {
    let a = tiempos::leer_alarma(documento);
    let mover = |rotulo: &str, orden: Orden| Boton {
        rotulo: Rotulo::Tal(rotulo.to_string()),
        orden,
        activo: true,
        principal: false,
    };
    Vista {
        reparto: Reparto {
            con_tablero: true,
            filas_de_botones: 2,
            con_lista: false,
            con_anadir: false,
        },
        // Sin rellenar la hora a dos cifras, igual que el movil
        // (`Tiempos.kt:170`): el minuto si, la hora no.
        tablero: format!("{}:{:02}", a.hora, a.minuto),
        alerta: false,
        filas_de_botones: vec![
            vec![
                mover("−1 h", Orden::Horas(-1)),
                mover("+1 h", Orden::Horas(1)),
                mover("−5 min", Orden::MinutosDeAlarma(-5)),
                mover("+5 min", Orden::MinutosDeAlarma(5)),
            ],
            vec![Boton {
                rotulo: Rotulo::Clave(if a.activa {
                    "mini-alarma-apagar"
                } else {
                    "mini-alarma-encender"
                }),
                orden: Orden::Activar,
                activo: true,
                principal: a.activa,
            }],
        ],
        lista: Vec::new(),
        guia: None,
        late_cada_ms: None,
    }
}

fn alarma(documento: &str, orden: &Orden) -> String {
    let mut a = tiempos::leer_alarma(documento);
    match orden {
        Orden::Activar => a.activa = !a.activa,
        // Aritmetica modular, como el movil (`MiniActivity.kt:544-555`): de
        // las 0:00 hacia atras se va a las 23:00, no a un numero negativo.
        Orden::Horas(h) => a.hora = ((a.hora as i64 + h + 24) % 24) as u8,
        Orden::MinutosDeAlarma(m) => a.minuto = ((a.minuto as i64 + m + 60) % 60) as u8,
        _ => return documento.to_string(),
    }
    tiempos::escribir_alarma(&mini::titulo(documento), &a)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn euro() -> gastos::Moneda {
        gastos::Moneda::euro()
    }

    fn nueva(cual: &str) -> String {
        mini::documento_nuevo(cual, "P", &euro()).unwrap()
    }

    #[test]
    fn las_siete_tienen_panel_y_una_de_otra_version_no() {
        for palabra in mini::TODAS {
            let d = nueva(palabra);
            assert!(
                vista(palabra, &d, &euro(), 0).is_some(),
                "{palabra} no ensena nada"
            );
        }
        assert!(vista("horoscopo", "lo que sea", &euro(), 0).is_none());
        // Y una orden que no es suya deja el documento intacto.
        let d = nueva(mini::TAREAS);
        assert_eq!(aplicar("horoscopo", &d, &euro(), &Orden::Mas, 0), d);
        assert_eq!(aplicar(mini::TAREAS, &d, &euro(), &Orden::Mas, 0), d);
    }

    #[test]
    fn una_tarea_se_anade_se_marca_y_se_borra() {
        let mut d = nueva(mini::TAREAS);
        d = aplicar(mini::TAREAS, &d, &euro(), &Orden::Anadir("pan".into()), 0);
        d = aplicar(mini::TAREAS, &d, &euro(), &Orden::Anadir("sal".into()), 0);
        assert_eq!(vista(mini::TAREAS, &d, &euro(), 0).unwrap().lista.len(), 2);
        d = aplicar(mini::TAREAS, &d, &euro(), &Orden::Alternar(0), 0);
        let v = vista(mini::TAREAS, &d, &euro(), 0).unwrap();
        assert!(v.lista[0].hecha && !v.lista[1].hecha);
        assert!(v.filas_de_botones[0][0].activo, "ya hay algo que limpiar");
        d = aplicar(mini::TAREAS, &d, &euro(), &Orden::LimpiarHechas, 0);
        let v = vista(mini::TAREAS, &d, &euro(), 0).unwrap();
        assert_eq!(v.lista.len(), 1);
        assert_eq!(v.lista[0].texto, "sal");
        assert!(!v.filas_de_botones[0][0].activo, "ya no queda nada hecho");
        // Y el titulo sobrevive a todo el trasiego.
        assert_eq!(mini::titulo(&d), "P");
        // Caso negativo: borrar una que no existe no cambia nada.
        assert_eq!(aplicar(mini::TAREAS, &d, &euro(), &Orden::Quitar(9), 0), d);
    }

    #[test]
    fn el_importe_es_lo_ultimo_que_se_teclea() {
        assert_eq!(partir_gasto("Cena 42,50", 2), ("Cena".into(), 4250));
        assert_eq!(
            partir_gasto("Tren de ida 18", 2),
            ("Tren de ida".into(), 1800)
        );
        assert_eq!(
            partir_gasto("Devolución -5", 2),
            ("Devolución".into(), -500)
        );
        // Sin cifra al final entra todo como concepto: se corrige luego.
        assert_eq!(partir_gasto("Comida", 2), ("Comida".into(), 0));
        // Y una cifra sola sin concepto tampoco se parte, o quedaria un
        // gasto sin nombre.
        assert_eq!(partir_gasto("12", 2), ("12".into(), 0));
    }

    #[test]
    fn los_gastos_suman_en_el_tablero_y_se_borran_por_su_fila() {
        let mut d = nueva(mini::GASTOS);
        d = aplicar(
            mini::GASTOS,
            &d,
            &euro(),
            &Orden::Anadir("Cena 42,50".into()),
            0,
        );
        d = aplicar(
            mini::GASTOS,
            &d,
            &euro(),
            &Orden::Anadir("Tren 18".into()),
            0,
        );
        let v = vista(mini::GASTOS, &d, &euro(), 0).unwrap();
        assert_eq!(v.lista.len(), 2);
        assert!(v.tablero.contains("60"), "el total: {}", v.tablero);
        d = aplicar(mini::GASTOS, &d, &euro(), &Orden::Quitar(0), 0);
        let v = vista(mini::GASTOS, &d, &euro(), 0).unwrap();
        assert_eq!(v.lista.len(), 1);
        assert_eq!(v.lista[0].texto, "Tren");
    }

    #[test]
    fn el_cronometro_cuenta_desde_que_arranca_y_la_vuelta_solo_corriendo() {
        let d = nueva(mini::CRONOMETRO);
        let parado = vista(mini::CRONOMETRO, &d, &euro(), 1_000).unwrap();
        assert!(
            !parado.filas_de_botones[0][1].activo,
            "Vuelta con el parado"
        );
        assert_eq!(parado.late_cada_ms, None, "parado no hace falta repintar");

        let d = aplicar(mini::CRONOMETRO, &d, &euro(), &Orden::ArrancarOParar, 1_000);
        let v = vista(mini::CRONOMETRO, &d, &euro(), 6_000).unwrap();
        assert_eq!(v.tablero, "0:05,0", "cinco segundos: {}", v.tablero);
        assert_eq!(v.late_cada_ms, Some(100));
        assert!(v.filas_de_botones[0][1].activo);

        let d = aplicar(mini::CRONOMETRO, &d, &euro(), &Orden::Vuelta, 6_000);
        let v = vista(mini::CRONOMETRO, &d, &euro(), 6_000).unwrap();
        assert_eq!(v.lista.len(), 1);

        // Parar congela lo llevado: el numero ya no depende del reloj.
        let d = aplicar(mini::CRONOMETRO, &d, &euro(), &Orden::ArrancarOParar, 9_000);
        let v = vista(mini::CRONOMETRO, &d, &euro(), 900_000).unwrap();
        assert_eq!(v.tablero, "0:08,0", "parado no sigue contando");
    }

    #[test]
    fn el_temporizador_se_ajusta_parado_y_avisa_al_vencer() {
        let d = nueva(mini::TEMPORIZADOR);
        let v = vista(mini::TEMPORIZADOR, &d, &euro(), 0).unwrap();
        assert_eq!(v.tablero, "5:00,0", "los cinco minutos de serie");
        let d = aplicar(mini::TEMPORIZADOR, &d, &euro(), &Orden::Minutos(-1), 0);
        let v = vista(mini::TEMPORIZADOR, &d, &euro(), 0).unwrap();
        assert_eq!(v.tablero, "4:00,0");

        let d = aplicar(mini::TEMPORIZADOR, &d, &euro(), &Orden::ArrancarOParar, 0);
        let v = vista(mini::TEMPORIZADOR, &d, &euro(), 0).unwrap();
        assert!(!v.filas_de_botones[0][0].activo, "en marcha no se ajusta");
        // Y pulsar un mas o un menos en marcha no cambia el documento.
        assert_eq!(
            aplicar(mini::TEMPORIZADOR, &d, &euro(), &Orden::Minutos(5), 0),
            d
        );
        let vencido = vista(mini::TEMPORIZADOR, &d, &euro(), 999_999).unwrap();
        assert!(vencido.alerta, "vencido se pinta en rojo");
        assert_eq!(vencido.tablero, "0:00,0", "no baja de cero");
    }

    #[test]
    fn el_contador_sube_y_baja_con_el_paso_elegido() {
        let mut d = nueva(mini::CONTADOR);
        d = aplicar(mini::CONTADOR, &d, &euro(), &Orden::ConPaso(12), 0);
        d = aplicar(mini::CONTADOR, &d, &euro(), &Orden::Mas, 0);
        d = aplicar(mini::CONTADOR, &d, &euro(), &Orden::Mas, 0);
        assert_eq!(vista(mini::CONTADOR, &d, &euro(), 0).unwrap().tablero, "24");
        d = aplicar(mini::CONTADOR, &d, &euro(), &Orden::Menos, 0);
        assert_eq!(vista(mini::CONTADOR, &d, &euro(), 0).unwrap().tablero, "12");
        // El paso elegido sale apagado: pulsarlo otra vez no hace nada.
        let v = vista(mini::CONTADOR, &d, &euro(), 0).unwrap();
        let doce = v.filas_de_botones[1]
            .iter()
            .find(|b| b.orden == Orden::ConPaso(12))
            .unwrap();
        assert!(!doce.activo);
        d = aplicar(mini::CONTADOR, &d, &euro(), &Orden::Reiniciar, 0);
        assert_eq!(vista(mini::CONTADOR, &d, &euro(), 0).unwrap().tablero, "0");
    }

    #[test]
    fn la_ruleta_no_sortea_con_menos_de_dos_nombres() {
        let mut d = nueva(mini::RULETA);
        assert!(
            !vista(mini::RULETA, &d, &euro(), 0)
                .unwrap()
                .filas_de_botones[0][0]
                .activo
        );
        d = aplicar(mini::RULETA, &d, &euro(), &Orden::Anadir("Ana".into()), 0);
        assert!(
            !vista(mini::RULETA, &d, &euro(), 0)
                .unwrap()
                .filas_de_botones[0][0]
                .activo
        );
        d = aplicar(mini::RULETA, &d, &euro(), &Orden::Anadir("Luis".into()), 0);
        let v = vista(mini::RULETA, &d, &euro(), 0).unwrap();
        assert!(v.filas_de_botones[0][0].activo);
        assert_eq!(v.lista.len(), 2);
        assert_eq!(sorteo(&d, 0.0), Some(0));
        assert_eq!(sorteo(&d, 0.99), Some(1));
        // Girar no toca el documento: el elegido no se guarda.
        assert_eq!(aplicar(mini::RULETA, &d, &euro(), &Orden::Girar(0.5), 0), d);
        assert_eq!(sorteo(&nueva(mini::RULETA), 0.5), None);
    }

    #[test]
    fn la_alarma_da_la_vuelta_al_reloj_y_se_guarda_con_tilde() {
        let mut d = nueva(mini::ALARMA);
        assert_eq!(vista(mini::ALARMA, &d, &euro(), 0).unwrap().tablero, "8:00");
        for _ in 0..9 {
            d = aplicar(mini::ALARMA, &d, &euro(), &Orden::Horas(-1), 0);
        }
        assert_eq!(
            vista(mini::ALARMA, &d, &euro(), 0).unwrap().tablero,
            "23:00",
            "de las 0 hacia atras se va a las 23"
        );
        d = aplicar(mini::ALARMA, &d, &euro(), &Orden::MinutosDeAlarma(-5), 0);
        assert_eq!(
            vista(mini::ALARMA, &d, &euro(), 0).unwrap().tablero,
            "23:55"
        );
        d = aplicar(mini::ALARMA, &d, &euro(), &Orden::Activar, 0);
        assert!(
            d.contains("activa: sí"),
            "sin la tilde el movil la apaga: {d:?}"
        );
    }

    #[test]
    fn nada_de_esto_pierde_lo_que_no_entiende() {
        // Una clave de una version mas nueva del movil, en medio del
        // documento: tocar un boton no puede llevarsela por delante.
        let d = "# C\n\n- valor: 3\n- paso: 1\n- color: azul";
        let nuevo = aplicar(mini::CONTADOR, d, &euro(), &Orden::Mas, 0);
        assert!(nuevo.contains("- color: azul"), "{nuevo:?}");
    }
}
