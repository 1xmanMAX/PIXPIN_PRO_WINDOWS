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
//!
//! ## Los atajos del panel ([`tecla`] y [`caracter`])
//!
//! Solo mientras el panel esta abierto; fuera de el no existen.
//!
//! | Tecla | Que hace |
//! |---|---|
//! | ↑ ↓, Inicio, Fin | Elegir linea (tareas, gastos, ruleta) |
//! | Espacio / Intro con la caja vacia | El boton gordo: tachar la elegida, arrancar/parar, +1, activar, sortear |
//! | Alt+↑ ↓ | Subir o bajar la linea elegida (tareas, gastos) |
//! | Supr | Borrar la elegida (tras sortear, la que salio) |
//! | F2 | Corregir la elegida; sin ninguna, el nombre del documento |
//! | Esc | Deshacer lo escrito, luego la eleccion, y al final cerrar |
//! | ↑ ↓ / Re Pag Av Pag | Temporizador ±1/±5 min; alarma ±5 min/±1 h; contador ±paso |
//! | `+` `-` / `v` | Contador / vuelta del cronometro |
//! | «25», «1:30» + Intro | Lo que dura el temporizador |
//! | «7:30» + Intro | La hora de la alarma (y la enciende) |

// El teclado, la correccion y los avisos estan listos, pero la ventana del
// chat todavia no los llama: esa costura es de `ventana_chat.rs`, que lleva
// otra mano. Quitar esta linea en cuanto se enganchen, para que el
// compilador vuelva a avisar de lo que sobre de verdad.

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
    /// Cambiarle el nombre al documento. En el movil se edita en la barra
    /// de arriba (`MiniActivity.kt:136-148`); aqui con F2.
    Titulo(String),
    /// Corregir la linea `n`: el texto de una tarea, un gasto entero
    /// («Cena 42,50») o un nombre de la ruleta.
    Cambiar(usize, String),
    /// Llevar una linea a otro sitio de la lista (Alt+flechas).
    Mover {
        desde: usize,
        hasta: usize,
    },
    /// Temporizador: lo que dura, tecleado de una vez («7», «1:30»).
    Duracion(i64),
    /// Alarma: la hora tecleada de una vez («7:30»). La deja encendida.
    Hora {
        hora: u8,
        minuto: u8,
    },
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
    /// La que tiene el teclado encima: la que marca Espacio, borra Supr,
    /// corrige F2 y mueve Alt+flechas. La pone [`vista_con`]; [`vista`] a
    /// secas no sabe de teclado y la deja siempre apagada.
    pub marcada: bool,
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
    // El nombre es de todas por igual, y cada una sabe reescribirse con otro.
    if let Orden::Titulo(nuevo) = orden {
        return mini::con_titulo(cual, documento, nuevo, moneda)
            .unwrap_or_else(|| documento.to_string());
    }
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
                marcada: false,
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
        Orden::Cambiar(n, t) => mini::renombrar(documento, *n, t),
        Orden::Mover { desde, hasta } => mini::mover(documento, *desde, *hasta),
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
                marcada: false,
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
        // Corregir es teclear la linea entera otra vez, como al anadirla: el
        // importe es lo ultimo. Es lo que el movil no deja hacer —alli una
        // cifra mal puesta obliga a borrar y volver a apuntar— y aqui es F2.
        Orden::Cambiar(n, t) => {
            let decimales = libro.moneda.decimales();
            let solo_cifra = !t.trim().is_empty()
                && t.trim()
                    .chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '−'));
            // Una cifra sola, al corregir, es el importe de un gasto sin
            // nombre (los hay: el nucleo los admite, `gastos::anadir`). Al
            // anadir no, porque ahi se prefiere no apuntar gastos sin nombre.
            let (concepto, centimos) = match gastos::centimos_de(t, decimales) {
                Some(c) if solo_cifra => (String::new(), c),
                _ => partir_gasto(t, decimales),
            };
            if (concepto.is_empty() && centimos == 0) || *n >= libro.gastos.len() {
                return documento.to_string();
            }
            gastos::escribir(&gastos::cambiar(&libro, *n, &concepto, centimos))
        }
        Orden::Mover { desde, hasta } => gastos::escribir(&gastos::mover(&libro, *desde, *hasta)),
        _ => documento.to_string(),
    }
}

/// La linea de un gasto como se volveria a teclear: el concepto y el
/// importe detras, con la coma de aqui. Es lo que F2 pone en la caja para
/// corregirlo, y [`partir_gasto`] lo vuelve a leer igual.
pub fn gasto_tecleable(g: &gastos::Gasto, decimales: u8) -> String {
    let cifra = gastos::canonico(g.centimos, decimales).replace('.', ",");
    if g.concepto.is_empty() {
        cifra
    } else {
        format!("{} {cifra}", g.concepto)
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
                marcada: false,
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
            // La caja es para teclear lo que dura de una vez («7», «1:30»):
            // cuatro botones de minutos estan bien para el pulgar, pero con
            // teclado «25» e Intro es mas rapido que cinco veces «+5».
            con_anadir: true,
        },
        // Sin decimas, como el movil (`comoSeLeeCorto`, `MiniActivity.kt:447`):
        // una cuenta atras de minutos no se mira a la decima, y un digito
        // bailando al final solo distrae.
        tablero: tiempos::como_se_lee_corto(t.restante(ahora)),
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
        guia: Some("mini-guia-duracion"),
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
        // Lo tecleado sigue la misma regla que los botones: en marcha no.
        Orden::Duracion(_) if t.corriendo() => return documento.to_string(),
        Orden::Duracion(ms) => tiempos::con_duracion(&t, *ms, ahora),
        _ => return documento.to_string(),
    };
    tiempos::escribir_temporizador(&titulo, &nuevo)
}

/// Lo que dura un temporizador tecleado, en milisegundos.
///
/// Un numero solo son **minutos** («7», «2,5»), porque es como se dice un
/// temporizador de cocina; con dos puntos es `m:ss` («1:30») y con dos pares
/// `h:mm:ss`. `None` si no es un tiempo (letras, «1:75», cero): la caja se
/// queda con lo escrito para corregirlo en vez de poner algo que no se pidio.
pub fn duracion_tecleada(texto: &str) -> Option<i64> {
    let limpio = texto.trim();
    if limpio.is_empty() || !limpio.is_ascii() {
        return None;
    }
    let ms = if limpio.contains(':') {
        let partes: Vec<&str> = limpio.split(':').map(str::trim).collect();
        if partes.len() > 3
            || partes
                .iter()
                .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        let n: Vec<i64> = partes
            .iter()
            .map(|p| p.parse::<i64>().ok())
            .collect::<Option<_>>()?;
        // Lo de detras del primero son minutos o segundos: mas de 59 es un
        // error de tecleo, no una forma rara de escribir.
        if n.iter().skip(1).any(|v| *v > 59) {
            return None;
        }
        n.iter()
            .fold(0i64, |acc, v| acc.saturating_mul(60).saturating_add(*v))
            * 1000
    } else {
        // Minutos, con decimales si se quiere («2,5» = dos y medio).
        let (enteros, fraccion) = match limpio.split_once([',', '.']) {
            Some((e, f)) => (e, f),
            None => (limpio, ""),
        };
        if enteros.is_empty() && fraccion.is_empty() {
            return None;
        }
        let cifras = |t: &str| t.bytes().all(|b| b.is_ascii_digit());
        if !cifras(enteros) || !cifras(fraccion) || enteros.len() > 6 {
            return None;
        }
        let e: i64 = if enteros.is_empty() {
            0
        } else {
            enteros.parse().ok()?
        };
        // Hasta la decima de minuto (seis segundos): mas finura con una coma
        // no la pide nadie, y para eso esta «m:ss».
        let decima: i64 = fraccion.bytes().next().map_or(0, |b| (b - b'0') as i64);
        e * 60_000 + decima * 6_000
    };
    (ms > 0).then_some(ms.min(tiempos::TOPE_DE_DURACION))
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
                marcada: false,
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
        // Corregir un nombre mal escrito sin sacarlo y volverlo a meter al
        // final, que le cambiaria el sitio en la lista.
        Orden::Cambiar(n, t) => {
            let limpio = t.replace("\r\n", " ").replace(['\r', '\n'], " ");
            let limpio = limpio.trim();
            if limpio.is_empty() || *n >= nombres.len() {
                return documento.to_string();
            }
            let mut v = nombres.clone();
            v[*n] = limpio.to_string();
            r::escribir(&titulo, &v)
        }
        // Girar no cambia el documento: el elegido no se guarda.
        _ => documento.to_string(),
    }
}

/// A quien le toca, de los nombres del documento. `None` si no hay ninguno.
///
/// La ventana sortea con [`girar`], que ademas deja marcado al elegido; esta
/// queda para las pruebas de la regla del sorteo.
#[cfg_attr(not(test), allow(dead_code))]
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
            // Para teclear la hora de una vez («7:30»): llegar de las 8:00 a
            // las 6:45 con los botones son cinco toques y la cuenta de cabeza.
            con_anadir: true,
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
        guia: Some("mini-guia-hora"),
        late_cada_ms: None,
    }
}

fn alarma(documento: &str, orden: &Orden) -> String {
    let mut a = tiempos::leer_alarma(documento);
    match orden {
        Orden::Activar => a.activa = !a.activa,
        // Teclear una hora es pedir que suene a esa hora: se enciende. Con
        // los botones no, porque alli se va de paso por horas que nadie pidio.
        Orden::Hora { hora, minuto } if *hora < 24 && *minuto < 60 => {
            a.hora = *hora;
            a.minuto = *minuto;
            a.activa = true;
        }
        // Aritmetica modular, como el movil (`MiniActivity.kt:544-555`): de
        // las 0:00 hacia atras se va a las 23:00, no a un numero negativo.
        Orden::Horas(h) => a.hora = ((a.hora as i64 + h + 24) % 24) as u8,
        Orden::MinutosDeAlarma(m) => a.minuto = ((a.minuto as i64 + m + 60) % 60) as u8,
        _ => return documento.to_string(),
    }
    tiempos::escribir_alarma(&mini::titulo(documento), &a)
}

/// La hora tecleada para una alarma: `7:30`, `730`, `7.30`, `19` (en punto).
///
/// Es la misma lectura que la de «Elegir la hora» de los recordatorios
/// ([`crate::recordatorios::hora_escrita`]), para que en todo el programa una
/// hora se escriba de la misma forma. `None` si no es una hora de verdad.
pub fn hora_tecleada(texto: &str) -> Option<(u8, u8)> {
    const DIA: i64 = 86_400_000;
    // Desde el instante cero, `hora_escrita` devuelve la hora de ese primer
    // dia; si es las 0:00 justas se va al dia siguiente, y el resto lo
    // devuelve a su sitio.
    let ms = crate::recordatorios::hora_escrita(texto, 0)?.rem_euclid(DIA);
    Some(((ms / 3_600_000) as u8, ((ms % 3_600_000) / 60_000) as u8))
}

// --- El teclado -----------------------------------------------------------
//
// En el movil todo es tocar. Aqui hay un teclado entero, y las mini-apps que
// se usan a diario ganan mucho con el: tachar la compra con flechas y
// Espacio, parar el cronometro sin buscar el boton, apuntar «Cena 42,50» e
// Intro. Todos estos atajos son **del panel**: solo existen mientras esta
// abierto y no le quitan ninguna tecla al resto del programa.
//
// El estado de la caja de abajo vive en [`Teclado`] y no en el documento:
// lo que se esta escribiendo no es un dato hasta que se pulsa Intro.

/// Que se esta escribiendo en la caja de abajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Edicion {
    /// Una linea nueva (o, en el temporizador y la alarma, un tiempo).
    #[default]
    Anadir,
    /// El nombre del documento (F2 sin ninguna fila marcada).
    Titulo,
    /// Corrigiendo la linea `n` (F2 con ella marcada).
    Fila(usize),
}

/// La caja de escribir y la fila marcada de un panel abierto.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Teclado {
    pub borrador: String,
    pub edicion: Edicion,
    pub marcada: Option<usize>,
}

/// Una tecla pulsada, con lo que habia mantenido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tecla {
    pub vk: u32,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// Que tiene que hacer la ventana con una tecla.
#[derive(Debug, Clone, PartialEq)]
pub enum Efecto {
    /// No es de este panel.
    Nada,
    /// Solo cambio la caja o la fila marcada: repintar y ya.
    Repintar,
    /// Cumplir esta orden y repintar. `Girar` lleva el azar a cero: lo pone
    /// quien la cumple, igual que con el boton, y lo cumple [`girar`].
    Hacer(Orden),
    /// Escape sin nada a medias: cerrar el panel.
    Cerrar,
}

const VK_RETROCESO: u32 = 0x08;
const VK_ENTRAR: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_RE_PAG: u32 = 0x21;
const VK_AV_PAG: u32 = 0x22;
const VK_FIN: u32 = 0x23;
const VK_INICIO: u32 = 0x24;
const VK_ARRIBA: u32 = 0x26;
const VK_ABAJO: u32 = 0x28;
const VK_SUPR: u32 = 0x2E;
const VK_F2: u32 = 0x71;

/// Las que tienen una lista cuyas lineas se corrigen, se borran y se marcan.
fn lista_editable(cual: &str) -> bool {
    matches!(cual, mini::TAREAS | mini::GASTOS | mini::RULETA)
}

/// Las que tienen caja de escribir abajo.
fn con_caja(cual: &str) -> bool {
    lista_editable(cual) || matches!(cual, mini::TEMPORIZADOR | mini::ALARMA)
}

/// La vista con lo del teclado encima: la fila marcada y, si se esta
/// corrigiendo algo, la caja y su guia. Es la que hay que pintar; [`vista`]
/// a secas es la del documento solo.
pub fn vista_con(
    cual: &str,
    documento: &str,
    moneda: &gastos::Moneda,
    ahora: i64,
    t: &Teclado,
) -> Option<Vista> {
    let mut v = vista(cual, documento, moneda, ahora)?;
    if let Some(n) = t.marcada
        && let Some(f) = v.lista.get_mut(n)
    {
        f.marcada = true;
    }
    match t.edicion {
        Edicion::Anadir => {}
        // Renombrar tambien en las que no tienen caja: el contador y el
        // cronometro necesitan una mientras se escribe su nombre.
        Edicion::Titulo => {
            v.reparto.con_anadir = true;
            v.guia = Some("mini-guia-titulo");
        }
        Edicion::Fila(_) => {
            v.reparto.con_anadir = true;
            v.guia = Some("mini-guia-cambiar");
        }
    }
    Some(v)
}

/// Empieza a corregir: la fila `n` si se da y se puede, o si no el nombre.
/// Pone en la caja lo que hay, para cambiar una letra sin reescribirlo todo.
pub fn empezar_a_corregir(
    cual: &str,
    documento: &str,
    moneda: &gastos::Moneda,
    t: &mut Teclado,
    fila: Option<usize>,
) {
    let texto_de_fila = fila.filter(|_| lista_editable(cual)).and_then(|n| {
        let texto = match cual {
            mini::TAREAS => mini::leer_tareas(documento).get(n)?.texto.clone(),
            mini::GASTOS => {
                let libro = gastos::leer(documento, moneda);
                gasto_tecleable(libro.gastos.get(n)?, libro.moneda.decimales())
            }
            _ => pixpin_proyecto::mini::ruleta::leer(documento)
                .get(n)?
                .clone(),
        };
        Some((n, texto))
    });
    match texto_de_fila {
        Some((n, texto)) => {
            t.edicion = Edicion::Fila(n);
            t.borrador = texto;
        }
        None => {
            t.edicion = Edicion::Titulo;
            t.borrador = mini::titulo(documento);
        }
    }
}

/// Deja la caja como al abrir: sin nada escrito y anadiendo.
fn soltar_caja(t: &mut Teclado) {
    t.borrador.clear();
    t.edicion = Edicion::Anadir;
}

/// Un clic en una fila que no se marca con el raton (un gasto, un nombre):
/// la deja marcada para el teclado.
pub fn clic_en_fila(t: &mut Teclado, n: usize) {
    t.marcada = Some(n);
}

/// Sortea en la ruleta y deja marcado a quien le toco, para que Supr lo
/// saque de la lista (el boton «Borrar» del cartel del movil,
/// `MiniActivity.kt:650-655`). Devuelve su nombre.
pub fn girar(documento: &str, t: &mut Teclado, azar: f64) -> Option<String> {
    let nombres = pixpin_proyecto::mini::ruleta::leer(documento);
    let n = pixpin_proyecto::mini::ruleta::elegir(&nombres, azar)?;
    t.marcada = Some(n);
    nombres.get(n).cloned()
}

/// Una tecla, dentro del panel.
///
/// Espacio, `+`, `-` y las letras llegan por [`caracter`], no por aqui:
/// Windows manda las dos cosas y atenderlas en los dos sitios haria cada
/// cosa dos veces.
pub fn tecla(
    cual: &str,
    documento: &str,
    moneda: &gastos::Moneda,
    t: &mut Teclado,
    k: Tecla,
) -> Efecto {
    let Some(v) = vista(cual, documento, moneda, 0) else {
        return Efecto::Nada;
    };
    let cuantas = v.lista.len();
    let editando = t.edicion != Edicion::Anadir;
    match k.vk {
        // Escape deshace de dentro afuera: primero lo escrito, luego la
        // marca, y solo con todo limpio cierra. Asi arrepentirse de lo
        // tecleado nunca cuesta el panel.
        VK_ESCAPE => {
            if editando || !t.borrador.is_empty() {
                soltar_caja(t);
                Efecto::Repintar
            } else if t.marcada.take().is_some() {
                Efecto::Repintar
            } else {
                Efecto::Cerrar
            }
        }
        VK_RETROCESO => {
            if t.borrador.pop().is_some() {
                Efecto::Repintar
            } else {
                Efecto::Nada
            }
        }
        VK_ENTRAR => entrar(cual, documento, t, cuantas),
        VK_F2 if !editando => {
            empezar_a_corregir(cual, documento, moneda, t, t.marcada);
            Efecto::Repintar
        }
        // Mientras se escribe un nombre, las flechas no tocan nada: moverse
        // por la lista a medio corregir dejaria la correccion en otra fila.
        _ if editando => Efecto::Nada,
        VK_ARRIBA | VK_ABAJO => {
            let paso: i64 = if k.vk == VK_ARRIBA { -1 } else { 1 };
            flecha(cual, t, cuantas, paso, k.alt)
        }
        VK_RE_PAG | VK_AV_PAG => {
            let arriba = k.vk == VK_RE_PAG;
            match cual {
                mini::TEMPORIZADOR => Efecto::Hacer(Orden::Minutos(if arriba { 5 } else { -5 })),
                mini::ALARMA => Efecto::Hacer(Orden::Horas(if arriba { 1 } else { -1 })),
                _ => Efecto::Nada,
            }
        }
        VK_INICIO | VK_FIN if lista_editable(cual) && cuantas > 0 => {
            t.marcada = Some(if k.vk == VK_INICIO { 0 } else { cuantas - 1 });
            Efecto::Repintar
        }
        VK_SUPR if t.borrador.is_empty() => match t.marcada {
            Some(n) if n < cuantas && v.lista[n].se_borra => {
                // La marca se queda en el sitio, sobre la siguiente: borrar
                // varias seguidas es pulsar Supr varias veces.
                t.marcada = (cuantas > 1).then(|| n.min(cuantas - 2));
                Efecto::Hacer(Orden::Quitar(n))
            }
            _ => Efecto::Nada,
        },
        _ => Efecto::Nada,
    }
}

/// Intro: guardar lo escrito, o la accion principal si no hay nada escrito.
fn entrar(cual: &str, documento: &str, t: &mut Teclado, cuantas: usize) -> Efecto {
    let escrito = t.borrador.trim().to_string();
    match t.edicion {
        Edicion::Titulo => {
            soltar_caja(t);
            return Efecto::Hacer(Orden::Titulo(escrito));
        }
        Edicion::Fila(n) => {
            soltar_caja(t);
            t.marcada = Some(n);
            return Efecto::Hacer(Orden::Cambiar(n, escrito));
        }
        Edicion::Anadir => {}
    }
    if !escrito.is_empty() {
        let orden = match cual {
            mini::TEMPORIZADOR => duracion_tecleada(&escrito).map(Orden::Duracion),
            mini::ALARMA => {
                hora_tecleada(&escrito).map(|(hora, minuto)| Orden::Hora { hora, minuto })
            }
            _ if lista_editable(cual) => Some(Orden::Anadir(escrito)),
            _ => None,
        };
        return match orden {
            Some(o) => {
                soltar_caja(t);
                t.marcada = None;
                Efecto::Hacer(o)
            }
            // Un tiempo que no se entiende se queda en la caja para
            // corregirlo: borrarlo obligaria a teclearlo entero otra vez.
            None if con_caja(cual) => Efecto::Repintar,
            None => {
                soltar_caja(t);
                Efecto::Repintar
            }
        };
    }
    principal(cual, documento, t, cuantas)
}

/// Lo que hace Intro o Espacio con la caja vacia: el boton gordo.
fn principal(cual: &str, documento: &str, t: &Teclado, cuantas: usize) -> Efecto {
    match cual {
        mini::CRONOMETRO | mini::TEMPORIZADOR => Efecto::Hacer(Orden::ArrancarOParar),
        mini::ALARMA => Efecto::Hacer(Orden::Activar),
        mini::CONTADOR => Efecto::Hacer(Orden::Mas),
        mini::RULETA if pixpin_proyecto::mini::ruleta::leer(documento).len() >= 2 => {
            Efecto::Hacer(Orden::Girar(0.0))
        }
        mini::TAREAS => match t.marcada {
            Some(n) if n < cuantas => Efecto::Hacer(Orden::Alternar(n)),
            _ => Efecto::Nada,
        },
        _ => Efecto::Nada,
    }
}

/// Flecha arriba o abajo: por la lista si la hay, o el ajuste fino de la
/// que no tiene (minutos, hora, cuenta).
fn flecha(cual: &str, t: &mut Teclado, cuantas: usize, paso: i64, alt: bool) -> Efecto {
    if lista_editable(cual) {
        if cuantas == 0 {
            return Efecto::Nada;
        }
        let ultima = cuantas as i64 - 1;
        // Alt lleva la marcada consigo. La ruleta no se reordena: el orden
        // no cambia a quien le toca, y el movil tampoco lo ofrece.
        if alt && cual != mini::RULETA {
            let Some(desde) = t.marcada.filter(|n| *n < cuantas) else {
                return Efecto::Nada;
            };
            let hasta = (desde as i64 + paso).clamp(0, ultima) as usize;
            if hasta == desde {
                return Efecto::Nada;
            }
            t.marcada = Some(hasta);
            return Efecto::Hacer(Orden::Mover { desde, hasta });
        }
        let siguiente = match t.marcada {
            // Sin marca, la primera flecha entra por el extremo que toca.
            None if paso > 0 => 0,
            None => ultima,
            Some(n) => (n as i64 + paso).clamp(0, ultima),
        };
        t.marcada = Some(siguiente as usize);
        return Efecto::Repintar;
    }
    match cual {
        mini::TEMPORIZADOR => Efecto::Hacer(Orden::Minutos(-paso)),
        mini::ALARMA => Efecto::Hacer(Orden::MinutosDeAlarma(-paso * 5)),
        mini::CONTADOR => Efecto::Hacer(if paso < 0 { Orden::Mas } else { Orden::Menos }),
        _ => Efecto::Nada,
    }
}

/// Un caracter tecleado, dentro del panel.
pub fn caracter(cual: &str, documento: &str, t: &mut Teclado, c: char) -> Efecto {
    if c < ' ' || mini::resumen(cual, documento, &gastos::Moneda::euro()).is_none() {
        return Efecto::Nada;
    }
    if t.edicion != Edicion::Anadir {
        t.borrador.push(c);
        return Efecto::Repintar;
    }
    let cuantas = match cual {
        mini::TAREAS => mini::leer_tareas(documento).len(),
        mini::RULETA => pixpin_proyecto::mini::ruleta::leer(documento).len(),
        _ => 0,
    };
    // Espacio con la caja vacia es el boton gordo: un espacio delante de
    // lo escrito no lo quiere nadie, y asi la tecla mas grande hace lo mas
    // corriente. Con algo escrito es un espacio y ya.
    if c == ' ' && t.borrador.is_empty() {
        return principal(cual, documento, t, cuantas);
    }
    match cual {
        // Sin caja: las teclas que dicen lo que hacen.
        mini::CONTADOR => match c {
            '+' | '=' => Efecto::Hacer(Orden::Mas),
            '-' | '−' => Efecto::Hacer(Orden::Menos),
            _ => Efecto::Nada,
        },
        mini::CRONOMETRO => match c {
            'v' | 'V' => Efecto::Hacer(Orden::Vuelta),
            _ => Efecto::Nada,
        },
        _ if con_caja(cual) => {
            t.borrador.push(c);
            Efecto::Repintar
        }
        _ => Efecto::Nada,
    }
}

/// El desplazamiento de la lista para que la fila marcada se vea.
///
/// Solo se mueve lo justo: si ya se ve, no se toca, que una lista que salta
/// a cada flecha no deja leer.
pub fn scroll_para_ver(
    d: &pixpin_ui::mini::Disposicion,
    marcada: Option<usize>,
    cuantas: usize,
    scroll: i32,
    escala: u32,
) -> i32 {
    let Some(n) = marcada.filter(|n| *n < cuantas) else {
        return scroll;
    };
    let caja = d.fila(n, 0, escala);
    let arriba = caja.y - d.lista.y;
    let abajo = arriba + caja.alto as i32;
    let nuevo = if arriba < scroll {
        arriba
    } else if abajo > scroll + d.lista.alto as i32 {
        abajo - d.lista.alto as i32
    } else {
        scroll
    };
    nuevo.clamp(0, d.tope_scroll(cuantas, escala))
}

// --- Los avisos del temporizador y la alarma ------------------------------
//
// En el movil, arrancar un temporizador o encender una alarma pone una alarma
// del sistema con el identificador `"mini:" + id del mensaje`
// (`MiniActivity.kt:473-486, 505-520`, `RecordatorioReceiver.kt:17-47`), y al
// llegar la hora saca el titulo en un pin. Aqui ese trabajo ya lo hace el
// vigia de los recordatorios: se le da la hora con el mismo prefijo y, al
// vencer, sale el pin y el globo como con cualquier recordatorio. Nada nuevo
// que vigilar ni otro hilo.

/// Lo que va delante del id del mensaje para que el aviso sea de una mini-app
/// y no la hora del propio mensaje (`RecordatorioReceiver.DE_UNA_MINIAPP`).
pub const PREFIJO_AVISO: &str = "mini:";

pub fn id_de_aviso(id_mensaje: &str) -> String {
    format!("{PREFIJO_AVISO}{id_mensaje}")
}

/// Cuando tiene que avisar esta mini-app, en UTC, o `None`.
///
/// - Temporizador en marcha: a su `finEn`, si aun no ha pasado. Uno que ya
///   vencio no vuelve a avisar al releerlo (al arrancar o tras sincronizar):
///   ya aviso cuando tocaba.
/// - Alarma encendida: la proxima vez que el reloj de la pared marque su
///   hora, hoy o manana (`proximaVezQueSean` del movil). `desfase_local_ms`
///   es lo que hay que sumar a UTC para tener la hora local.
pub fn cuando_avisa(
    cual: &str,
    documento: &str,
    ahora_utc: i64,
    desfase_local_ms: i64,
) -> Option<i64> {
    const DIA: i64 = 86_400_000;
    match cual {
        mini::TEMPORIZADOR => tiempos::leer_temporizador(documento)
            .fin_en
            .filter(|f| *f > ahora_utc),
        mini::ALARMA => {
            let a = tiempos::leer_alarma(documento);
            if !a.activa {
                return None;
            }
            let ahora_local = ahora_utc + desfase_local_ms;
            let mut cuando = ahora_local.div_euclid(DIA) * DIA
                + a.hora as i64 * 3_600_000
                + a.minuto as i64 * 60_000;
            if cuando <= ahora_local {
                cuando += DIA;
            }
            Some(cuando - desfase_local_ms)
        }
        _ => None,
    }
}

/// Lo que dice el aviso: el titulo del documento, o el nombre del mensaje si
/// no tiene (`RecordatorioReceiver.kt:42-44`).
pub fn texto_del_aviso(m: &pixpin_proyecto::cuaderno::Mensaje) -> String {
    let t = mini::titulo(&m.texto);
    if t.is_empty() { m.nombre.clone() } else { t }
}

/// Los avisos pendientes de las mini-apps de un cuaderno.
///
/// Es lo que hay que sumar a la agenda al arrancar y tras sincronizar: un
/// temporizador puesto en el movil llega como texto de un mensaje y, sin
/// esto, no sonaria en el PC.
pub fn avisos_del_cuaderno(
    c: &pixpin_proyecto::cuaderno::Cuaderno,
    ahora_utc: i64,
    desfase_local_ms: i64,
) -> Vec<crate::recordatorios::Recordatorio> {
    c.mensajes
        .iter()
        .filter_map(|m| {
            let cual = m.miniapp.as_deref()?;
            Some(crate::recordatorios::Recordatorio {
                id: id_de_aviso(&m.id),
                cuando_utc_ms: cuando_avisa(cual, &m.texto, ahora_utc, desfase_local_ms)?,
                texto: texto_del_aviso(m),
            })
        })
        .collect()
}

/// Pone, cambia o quita el aviso de una mini-app tras tocarla.
///
/// Se rehace entero en cada cambio, como el movil (`MiniActivity.kt:505-509`):
/// quitarlo y volverlo a poner es una llamada, y asi no queda uno viejo
/// sonando a una hora que ya nadie pidio.
pub fn avisar(carpeta: &std::path::Path, m: &pixpin_proyecto::cuaderno::Mensaje) {
    let Some(cual) = m.miniapp.as_deref() else {
        return;
    };
    let id = id_de_aviso(&m.id);
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    match cuando_avisa(
        cual,
        &m.texto,
        ahora,
        pixpin_shell::entorno::desfase_local_ms(),
    ) {
        Some(cuando) => crate::recordatorios::programar(carpeta, &id, &texto_del_aviso(m), cuando),
        None => crate::recordatorios::cancelar(&id),
    }
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
        assert_eq!(v.tablero, "5:00", "los cinco minutos de serie, sin decimas");
        let d = aplicar(mini::TEMPORIZADOR, &d, &euro(), &Orden::Minutos(-1), 0);
        let v = vista(mini::TEMPORIZADOR, &d, &euro(), 0).unwrap();
        assert_eq!(v.tablero, "4:00");

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
        assert_eq!(vencido.tablero, "0:00", "no baja de cero");
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

    fn k(vk: u32) -> Tecla {
        Tecla {
            vk,
            shift: false,
            ctrl: false,
            alt: false,
        }
    }

    fn alt(vk: u32) -> Tecla {
        Tecla { alt: true, ..k(vk) }
    }

    /// Lo que haria la ventana: cumplir lo que diga el efecto y devolver el
    /// documento que queda. El azar de la ruleta, a medias.
    fn cumplir(cual: &str, d: &str, t: &mut Teclado, e: Efecto) -> String {
        match e {
            Efecto::Hacer(Orden::Girar(_)) => {
                girar(d, t, 0.5);
                d.to_string()
            }
            Efecto::Hacer(o) => aplicar(cual, d, &euro(), &o, 1_000),
            _ => d.to_string(),
        }
    }

    fn teclear(cual: &str, d: &str, t: &mut Teclado, texto: &str) -> String {
        let mut d = d.to_string();
        for c in texto.chars() {
            let e = caracter(cual, &d, t, c);
            d = cumplir(cual, &d, t, e);
        }
        d
    }

    fn pulsar(cual: &str, d: &str, t: &mut Teclado, tecla_: Tecla) -> String {
        let e = tecla(cual, d, &euro(), t, tecla_);
        cumplir(cual, d, t, e)
    }

    #[test]
    fn la_compra_se_apunta_y_se_tacha_sin_tocar_el_raton() {
        let mut t = Teclado::default();
        let mut d = nueva(mini::TAREAS);
        for cosa in ["pan", "leche", "sal"] {
            d = teclear(mini::TAREAS, &d, &mut t, cosa);
            d = pulsar(mini::TAREAS, &d, &mut t, k(VK_ENTRAR));
        }
        assert_eq!(mini::leer_tareas(&d).len(), 3);
        assert!(t.borrador.is_empty(), "Intro vacia la caja");
        // Flecha abajo entra por la primera; otra, la segunda; Espacio tacha.
        d = pulsar(mini::TAREAS, &d, &mut t, k(VK_ABAJO));
        d = pulsar(mini::TAREAS, &d, &mut t, k(VK_ABAJO));
        d = teclear(mini::TAREAS, &d, &mut t, " ");
        let v = vista_con(mini::TAREAS, &d, &euro(), 0, &t).unwrap();
        assert!(v.lista[1].hecha && v.lista[1].marcada, "{:?}", v.lista);
        assert!(!v.lista[0].hecha);
        // Alt+flecha arriba la sube y la marca va con ella.
        d = pulsar(mini::TAREAS, &d, &mut t, alt(VK_ARRIBA));
        assert_eq!(mini::leer_tareas(&d)[0].texto, "leche");
        assert_eq!(t.marcada, Some(0));
        // Alt en el tope no hace nada: no hay mas arriba.
        assert_eq!(pulsar(mini::TAREAS, &d, &mut t, alt(VK_ARRIBA)), d);
        // Supr la borra y la marca pasa a la siguiente.
        d = pulsar(mini::TAREAS, &d, &mut t, k(VK_SUPR));
        assert_eq!(mini::leer_tareas(&d).len(), 2);
        assert_eq!(t.marcada, Some(0));
        assert_eq!(mini::leer_tareas(&d)[0].texto, "pan");
    }

    #[test]
    fn un_espacio_con_algo_escrito_es_un_espacio_y_no_un_tachon() {
        let mut t = Teclado::default();
        let mut d = aplicar(
            mini::TAREAS,
            &nueva(mini::TAREAS),
            &euro(),
            &Orden::Anadir("a".into()),
            0,
        );
        t.marcada = Some(0);
        d = teclear(mini::TAREAS, &d, &mut t, "dos cosas");
        assert_eq!(t.borrador, "dos cosas");
        assert!(
            !mini::leer_tareas(&d)[0].hecha,
            "no se tacho a medias de escribir"
        );
    }

    #[test]
    fn f2_corrige_la_marcada_o_si_no_el_nombre() {
        let mut t = Teclado::default();
        let mut d = aplicar(
            mini::TAREAS,
            &nueva(mini::TAREAS),
            &euro(),
            &Orden::Anadir("pna".into()),
            0,
        );
        // Sin marca: el nombre del documento.
        d = pulsar(mini::TAREAS, &d, &mut t, k(VK_F2));
        assert_eq!(t.edicion, Edicion::Titulo);
        assert_eq!(t.borrador, "P", "la caja trae el nombre que habia");
        let v = vista_con(mini::TAREAS, &d, &euro(), 0, &t).unwrap();
        assert_eq!(v.guia, Some("mini-guia-titulo"));
        d = pulsar(mini::TAREAS, &d, &mut t, k(VK_RETROCESO));
        d = teclear(mini::TAREAS, &d, &mut t, "Compra");
        d = pulsar(mini::TAREAS, &d, &mut t, k(VK_ENTRAR));
        assert_eq!(mini::titulo(&d), "Compra");
        assert_eq!(t.edicion, Edicion::Anadir);
        // Con marca: esa linea.
        t.marcada = Some(0);
        d = pulsar(mini::TAREAS, &d, &mut t, k(VK_F2));
        assert_eq!(t.edicion, Edicion::Fila(0));
        t.borrador = "pan".into();
        d = pulsar(mini::TAREAS, &d, &mut t, k(VK_ENTRAR));
        assert_eq!(mini::leer_tareas(&d)[0].texto, "pan");
        // Escape a medias no cambia nada y deja la caja como estaba.
        d = pulsar(mini::TAREAS, &d, &mut t, k(VK_F2));
        t.borrador = "otra cosa".into();
        let antes = d.clone();
        assert_eq!(
            tecla(mini::TAREAS, &d, &euro(), &mut t, k(VK_ESCAPE)),
            Efecto::Repintar
        );
        assert_eq!(d, antes);
        assert_eq!(
            t,
            Teclado {
                marcada: Some(0),
                ..Teclado::default()
            }
        );
    }

    #[test]
    fn escape_deshace_de_dentro_afuera_y_solo_al_final_cierra() {
        let mut t = Teclado {
            borrador: "x".into(),
            edicion: Edicion::Anadir,
            marcada: Some(0),
        };
        let d = aplicar(
            mini::TAREAS,
            &nueva(mini::TAREAS),
            &euro(),
            &Orden::Anadir("a".into()),
            0,
        );
        assert_eq!(
            tecla(mini::TAREAS, &d, &euro(), &mut t, k(VK_ESCAPE)),
            Efecto::Repintar
        );
        assert!(
            t.borrador.is_empty() && t.marcada.is_some(),
            "primero lo escrito"
        );
        assert_eq!(
            tecla(mini::TAREAS, &d, &euro(), &mut t, k(VK_ESCAPE)),
            Efecto::Repintar
        );
        assert!(t.marcada.is_none(), "luego la marca");
        assert_eq!(
            tecla(mini::TAREAS, &d, &euro(), &mut t, k(VK_ESCAPE)),
            Efecto::Cerrar
        );
    }

    #[test]
    fn un_gasto_mal_apuntado_se_corrige_con_f2() {
        let mut t = Teclado::default();
        let mut d = teclear(mini::GASTOS, &nueva(mini::GASTOS), &mut t, "Cena 42,5");
        d = pulsar(mini::GASTOS, &d, &mut t, k(VK_ENTRAR));
        d = pulsar(mini::GASTOS, &d, &mut t, k(VK_ABAJO));
        d = pulsar(mini::GASTOS, &d, &mut t, k(VK_F2));
        assert_eq!(
            t.borrador, "Cena 42,50",
            "la linea como se volveria a teclear"
        );
        t.borrador = "Cena con vino 52,50".into();
        d = pulsar(mini::GASTOS, &d, &mut t, k(VK_ENTRAR));
        let libro = gastos::leer(&d, &euro());
        assert_eq!(libro.gastos.len(), 1, "se corrige, no se anade otro");
        assert_eq!(libro.gastos[0].concepto, "Cena con vino");
        assert_eq!(libro.total(), 5250);
        // Caso negativo: vaciar la linea al corregir no la deja en blanco.
        assert_eq!(
            aplicar(
                mini::GASTOS,
                &d,
                &euro(),
                &Orden::Cambiar(0, "  ".into()),
                0
            ),
            d
        );
        // Y una cifra sola, al corregir, es el importe de un gasto sin nombre.
        let sin_nombre = aplicar(mini::GASTOS, &d, &euro(), &Orden::Cambiar(0, "7".into()), 0);
        let l = gastos::leer(&sin_nombre, &euro());
        assert_eq!(
            (l.gastos[0].concepto.as_str(), l.gastos[0].centimos),
            ("", 700)
        );
    }

    #[test]
    fn los_gastos_se_reordenan_con_alt_y_flechas() {
        let mut d = nueva(mini::GASTOS);
        for g in ["A 1", "B 2", "C 3"] {
            d = aplicar(mini::GASTOS, &d, &euro(), &Orden::Anadir(g.into()), 0);
        }
        let mut t = Teclado {
            marcada: Some(2),
            ..Teclado::default()
        };
        d = pulsar(mini::GASTOS, &d, &mut t, alt(VK_ARRIBA));
        let orden: Vec<_> = gastos::leer(&d, &euro())
            .gastos
            .into_iter()
            .map(|g| g.concepto)
            .collect();
        assert_eq!(orden, ["A", "C", "B"]);
        assert_eq!(t.marcada, Some(1));
    }

    #[test]
    fn el_cronometro_y_el_contador_van_con_espacio_y_teclas_sueltas() {
        let mut t = Teclado::default();
        let d = nueva(mini::CRONOMETRO);
        assert_eq!(
            caracter(mini::CRONOMETRO, &d, &mut t, ' '),
            Efecto::Hacer(Orden::ArrancarOParar)
        );
        assert_eq!(
            caracter(mini::CRONOMETRO, &d, &mut t, 'v'),
            Efecto::Hacer(Orden::Vuelta)
        );
        // Una letra cualquiera no se escribe en una caja que no existe.
        assert_eq!(caracter(mini::CRONOMETRO, &d, &mut t, 'x'), Efecto::Nada);
        assert!(t.borrador.is_empty());

        let mut d = nueva(mini::CONTADOR);
        d = teclear(mini::CONTADOR, &d, &mut t, "+++ -");
        assert_eq!(
            pixpin_proyecto::mini::contador::leer(&d).valor,
            3,
            "tres, uno por el espacio, menos uno"
        );
        d = pulsar(mini::CONTADOR, &d, &mut t, k(VK_ARRIBA));
        assert_eq!(pixpin_proyecto::mini::contador::leer(&d).valor, 4);
        d = pulsar(mini::CONTADOR, &d, &mut t, k(VK_ABAJO));
        assert_eq!(pixpin_proyecto::mini::contador::leer(&d).valor, 3);
    }

    #[test]
    fn el_temporizador_se_pone_tecleando_lo_que_dura() {
        assert_eq!(duracion_tecleada("7"), Some(7 * 60_000));
        assert_eq!(duracion_tecleada("2,5"), Some(150_000));
        assert_eq!(duracion_tecleada("1:30"), Some(90_000));
        assert_eq!(duracion_tecleada("1:02:03"), Some(3_723_000));
        assert_eq!(
            duracion_tecleada("100000"),
            Some(tiempos::TOPE_DE_DURACION),
            "un dia de tope"
        );
        // Casos negativos: lo que no es un tiempo no pone nada.
        for malo in ["", "0", "abc", "1:75", "1::2", "-3", "1:2:3:4", "5 min"] {
            assert_eq!(duracion_tecleada(malo), None, "{malo:?}");
        }
        let mut t = Teclado::default();
        let mut d = teclear(mini::TEMPORIZADOR, &nueva(mini::TEMPORIZADOR), &mut t, "25");
        d = pulsar(mini::TEMPORIZADOR, &d, &mut t, k(VK_ENTRAR));
        assert_eq!(tiempos::leer_temporizador(&d).duracion, 25 * 60_000);
        // Algo ilegible se queda en la caja para corregirlo.
        let antes = d.clone();
        d = teclear(mini::TEMPORIZADOR, &d, &mut t, "2x");
        d = pulsar(mini::TEMPORIZADOR, &d, &mut t, k(VK_ENTRAR));
        assert_eq!(d, antes);
        assert_eq!(t.borrador, "2x");
        t.borrador.clear();
        // Re Pag suma cinco; Intro con la caja vacia lo arranca.
        d = pulsar(mini::TEMPORIZADOR, &d, &mut t, k(VK_RE_PAG));
        assert_eq!(tiempos::leer_temporizador(&d).duracion, 30 * 60_000);
        d = pulsar(mini::TEMPORIZADOR, &d, &mut t, k(VK_ENTRAR));
        assert!(tiempos::leer_temporizador(&d).corriendo());
    }

    #[test]
    fn la_alarma_se_pone_tecleando_la_hora_y_queda_encendida() {
        assert_eq!(hora_tecleada("7:30"), Some((7, 30)));
        assert_eq!(hora_tecleada("730"), Some((7, 30)));
        assert_eq!(
            hora_tecleada("0:00"),
            Some((0, 0)),
            "medianoche no se va al dia siguiente"
        );
        assert_eq!(hora_tecleada("23:59"), Some((23, 59)));
        assert_eq!(hora_tecleada("25:00"), None);
        assert_eq!(hora_tecleada("mañana"), None);
        let mut t = Teclado::default();
        let mut d = teclear(mini::ALARMA, &nueva(mini::ALARMA), &mut t, "6:45");
        d = pulsar(mini::ALARMA, &d, &mut t, k(VK_ENTRAR));
        assert!(d.ends_with("- hora: 6:45\n- activa: sí"), "{d:?}");
        // Av Pag resta una hora, con la vuelta del reloj.
        d = pulsar(mini::ALARMA, &d, &mut t, k(VK_AV_PAG));
        assert!(d.contains("- hora: 5:45"), "{d:?}");
    }

    #[test]
    fn la_ruleta_sortea_con_intro_y_supr_saca_al_elegido() {
        let mut t = Teclado::default();
        let mut d = nueva(mini::RULETA);
        // Con uno solo Intro no sortea: no hay sorteo que valga.
        d = teclear(mini::RULETA, &d, &mut t, "Ana");
        d = pulsar(mini::RULETA, &d, &mut t, k(VK_ENTRAR));
        assert_eq!(
            tecla(mini::RULETA, &d, &euro(), &mut t, k(VK_ENTRAR)),
            Efecto::Nada
        );
        for n in ["Luis", "Marta"] {
            d = teclear(mini::RULETA, &d, &mut t, n);
            d = pulsar(mini::RULETA, &d, &mut t, k(VK_ENTRAR));
        }
        d = pulsar(mini::RULETA, &d, &mut t, k(VK_ENTRAR));
        assert_eq!(
            t.marcada,
            Some(1),
            "0,5 de tres cae en Luis y queda marcado"
        );
        d = pulsar(mini::RULETA, &d, &mut t, k(VK_SUPR));
        assert_eq!(pixpin_proyecto::mini::ruleta::leer(&d), ["Ana", "Marta"]);
        // La ruleta no se reordena con Alt.
        t.marcada = Some(1);
        assert_eq!(
            tecla(mini::RULETA, &d, &euro(), &mut t, alt(VK_ARRIBA)),
            Efecto::Repintar
        );
        assert_eq!(pixpin_proyecto::mini::ruleta::leer(&d), ["Ana", "Marta"]);
    }

    #[test]
    fn una_mini_app_de_otra_version_no_atiende_teclas() {
        let mut t = Teclado::default();
        assert_eq!(caracter("horoscopo", "x", &mut t, 'a'), Efecto::Nada);
        assert_eq!(
            tecla("horoscopo", "x", &euro(), &mut t, k(VK_ESCAPE)),
            Efecto::Nada
        );
        assert!(t.borrador.is_empty());
    }

    #[test]
    fn el_temporizador_avisa_a_su_hora_y_uno_vencido_ya_no() {
        let d = tiempos::escribir_temporizador(
            "Pasta",
            &tiempos::Temporizador {
                duracion: 1_000,
                fin_en: Some(10_000),
                otros: Vec::new(),
            },
        );
        assert_eq!(cuando_avisa(mini::TEMPORIZADOR, &d, 5_000, 0), Some(10_000));
        assert_eq!(
            cuando_avisa(mini::TEMPORIZADOR, &d, 10_000, 0),
            None,
            "ya aviso"
        );
        assert_eq!(
            cuando_avisa(mini::TEMPORIZADOR, &nueva(mini::TEMPORIZADOR), 0, 0),
            None,
            "parado"
        );
        assert_eq!(
            cuando_avisa(mini::CONTADOR, &nueva(mini::CONTADOR), 0, 0),
            None
        );
        assert_eq!(
            id_de_aviso("123"),
            "mini:123",
            "el mismo prefijo que el movil"
        );
    }

    #[test]
    fn la_alarma_avisa_la_proxima_vez_que_el_reloj_marque_su_hora() {
        const H: i64 = 3_600_000;
        const DIA: i64 = 24 * H;
        let d = "# D\n\n- hora: 7:30\n- activa: sí";
        let desfase = 2 * H; // Madrid en verano.
        // A las 6:00 locales (4:00 UTC) de un dia cualquiera: hoy a las 7:30.
        let dia = 20_000 * DIA;
        let ahora = dia + 6 * H - desfase;
        assert_eq!(
            cuando_avisa(mini::ALARMA, d, ahora, desfase),
            Some(dia + 7 * H + H / 2 - desfase)
        );
        // A las 8:00 locales ya paso: manana.
        let tarde = dia + 8 * H - desfase;
        assert_eq!(
            cuando_avisa(mini::ALARMA, d, tarde, desfase),
            Some(dia + DIA + 7 * H + H / 2 - desfase)
        );
        // Apagada no avisa.
        assert_eq!(
            cuando_avisa(mini::ALARMA, "- hora: 7:30\n- activa: no", ahora, desfase),
            None
        );
    }

    #[test]
    fn los_avisos_del_cuaderno_llevan_el_titulo_o_el_nombre() {
        use pixpin_proyecto::cuaderno::{Cuaderno, Mensaje, Sello};
        let sello = |n: i64| Sello {
            cuando: n,
            numero: n,
            aparato: "pc".into(),
            proyecto: "p".into(),
        };
        let con_titulo = Mensaje::miniapp(
            mini::TEMPORIZADOR,
            "Temporizador",
            "# Pasta\n\n- duracion: 1000\n- finEn: 9000",
            &sello(1),
        );
        let sin_titulo = Mensaje::miniapp(
            mini::TEMPORIZADOR,
            "Huevos",
            "- duracion: 1000\n- finEn: 9000",
            &sello(2),
        );
        let parado = Mensaje::miniapp(
            mini::TEMPORIZADOR,
            "X",
            &nueva(mini::TEMPORIZADOR),
            &sello(3),
        );
        let nota = Mensaje {
            texto: "- finEn: 9000".into(),
            ..Mensaje::default()
        };
        let c = Cuaderno {
            mensajes: vec![con_titulo, sin_titulo, parado, nota],
            ..Cuaderno::default()
        };
        let v = avisos_del_cuaderno(&c, 5_000, 0);
        let textos: Vec<_> = v
            .iter()
            .map(|r| (r.id.as_str(), r.texto.as_str()))
            .collect();
        assert_eq!(textos, [("mini:1", "Pasta"), ("mini:2", "Huevos")]);
    }

    #[test]
    fn la_fila_marcada_se_trae_a_la_vista_solo_si_hace_falta() {
        use pixpin_geom::Rect;
        let hueco = Rect {
            x: 0,
            y: 0,
            ancho: 300,
            alto: 300,
        };
        let d = pixpin_ui::mini::Disposicion::calcular(hueco, 100, Reparto::lista_con_botones());
        let fila = pixpin_ui::mini::FILA_ALTO as i32;
        let caben = d.lista.alto as i32 / fila;
        // Una que ya se ve no mueve nada.
        assert_eq!(scroll_para_ver(&d, Some(0), 50, 0, 100), 0);
        // Una por debajo baja lo justo para que asome entera.
        let n = caben as usize + 3;
        let s = scroll_para_ver(&d, Some(n), 50, 0, 100);
        assert_eq!(s, (n as i32 + 1) * fila - d.lista.alto as i32);
        // Y volver a la primera sube del todo.
        assert_eq!(scroll_para_ver(&d, Some(0), 50, s, 100), 0);
        // Sin marca, o fuera de la lista, se queda como estaba.
        assert_eq!(scroll_para_ver(&d, None, 50, 17, 100), 17);
        assert_eq!(scroll_para_ver(&d, Some(99), 50, 17, 100), 17);
    }
}
