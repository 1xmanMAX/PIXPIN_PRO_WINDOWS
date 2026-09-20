//! **El telepronter: el texto baja solo y uno lo lee en voz alta, grabandose.**
//!
//! Es `guardados/TelepronterActivity.kt` del movil, y es lo que alli se
//! llama «leer en voz alta». Conviene decirlo porque suena a otra cosa:
//! **no hay voz sintetica en ninguna parte**. En todo PixPin Android no hay
//! un solo `TextToSpeech`; leer en voz alta lo hace el usuario, y el
//! aparato graba.
//!
//! La nota de voz que sale de aqui **ya trae su texto con minutos** sin
//! haber pasado por ningun reconocedor: como el texto baja a una velocidad
//! conocida, se sabe en que segundo cada parrafo cruzo la franja de
//! lectura. Esa cuenta es pura y vive en `pixpin_voz::telepronter`; aqui
//! esta la ventana.
//!
//! Estilo de `visor.rs`: hilo propio, `VentanaOverlay::nueva_normal`, todo
//! pintado con el `Pintor`. Este modulo no lleva `unsafe`.

#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use pixpin_audio::picos::MINIMO_MS;
use pixpin_audio::{Grabadora, reloj};
use pixpin_geom::Rect;
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma};
use pixpin_voz::telepronter::{
    self, CUENTA_ATRAS, Desfile, Medida, TAMANO_POR_DEFECTO, VELOCIDAD_POR_DEFECTO,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::caja_dibujo::hex;
use crate::overlay::Recursos;

// Papel, no interfaz: los mismos colores del visor limpio, que es la otra
// pantalla de esta casa hecha para leer.
const FONDO: Color = hex(0x121316);
const TEXTO: Color = hex(0xe4e2e6);
const APAGADO: Color = hex(0x9a9aa2);
const CRISTAL: Color = hex(0x26262c);
const ROJO: Color = hex(0xe05a4f);
const DORADO: Color = hex(0xe8c06a);

/// La franja de lectura, en un color que se ve sin robar la vista.
const FRANJA_COLOR: Color = hex(0x1d2733);

const VK_ESCAPE: u32 = 0x1B;
const VK_ESPACIO: u32 = 0x20;
const VK_INTRO: u32 = 0x0D;
const VK_PRIOR: u32 = 0x21;
const VK_NEXT: u32 = 0x22;
const VK_IZQUIERDA: u32 = 0x25;
const VK_ARRIBA: u32 = 0x26;
const VK_DERECHA: u32 = 0x27;
const VK_ABAJO: u32 = 0x28;
const VK_MAS: u32 = 0xBB;
const VK_MENOS: u32 = 0xBD;

/// Cuanto dura cada numero de la cuenta atras. «Un segundo escaso», como en
/// el movil (`TelepronterActivity.kt:206`): a 1000 ms se hace larga.
const MS_POR_NUMERO: u64 = 900;

/// Lo que sale de una lectura: justo lo que hay que escribir en el mensaje
/// de voz del cuaderno.
#[derive(Debug, Clone, PartialEq)]
pub struct Lectura {
    pub ruta: PathBuf,
    pub duracion_ms: i64,
    /// Los picos de la onda, 0..32767, como mucho 256. Los trae la
    /// grabadora ya repartidos.
    pub picos: Vec<i32>,
    /// El texto con sus minutos, `[m:ss] parrafo`. Va a
    /// `Mensaje.transcripcion`.
    pub cuerpo: String,
}

/// **Escribe la lectura en un mensaje de voz.**
///
/// El `Mensaje` lo crea quien llama —los tres codigos, el `id` y el
/// proyecto son suyos— y esto le pone lo que salio de la lectura: la clase,
/// el fichero, la duracion, los picos, el texto y su estado. El estado es
/// siempre `bien` porque el texto **no** salio de adivinar nada: estaba
/// escrito (`TelepronterActivity.kt:244-250`).
pub fn aplicar(mensaje: &mut Mensaje, lectura: &Lectura) {
    mensaje.clase = Some(Clase::Voz);
    mensaje.ruta = Some(lectura.ruta.display().to_string());
    mensaje.nombre = lectura
        .ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    mensaje.duracion_ms = lectura.duracion_ms;
    mensaje.transcripcion = Some(lectura.cuerpo.clone());
    mensaje.resto.insert(
        "picos".into(),
        serde_json::Value::Array(
            lectura
                .picos
                .iter()
                .map(|p| serde_json::Value::from(*p))
                .collect(),
        ),
    );
    mensaje.resto.insert(
        "estadoDelTexto".into(),
        serde_json::Value::String("bien".into()),
    );
}

/// **Abre el telepronter con ese texto, en su propio hilo.**
///
/// Devuelve el extremo de un canal: cuando el usuario termine de leer,
/// llega una [`Lectura`] y nada mas. Si cierra sin grabar, el canal se
/// cierra sin mandar nada. El chat lo mira con `try_recv` en su bucle, sin
/// bloquearse, igual que hace con el resto de ventanas de su propio hilo.
///
/// `destino` es la carpeta donde dejar el `.m4a` (ver
/// `crate::voz::carpeta_de_audios`).
pub fn lanzar(idioma: Idioma, texto: String, destino: PathBuf) -> Receiver<Lectura> {
    let (enviar, recibir) = channel();
    let lanzado = std::thread::Builder::new()
        .name("telepronter".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let hecho =
                Recursos::nuevos().and_then(|r| abrir(&r, idioma, &texto, &destino, &enviar));
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir el telepronter");
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del telepronter");
    }
    recibir
}

/// En que punto esta la sesion. Mismo reparto que `grabador::Fase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fase {
    /// El texto quieto: se puede colocar, cambiar la letra y la velocidad.
    Parado,
    /// Baja el texto pero no se graba: es el «Ensayar» del movil.
    Ensayando,
    /// Tres, dos, uno.
    Contando,
    /// Grabando de verdad.
    Grabando,
}

/// Que hace cada trozo pulsable de la pantalla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Cerrar,
    Ensayar,
    Grabar,
    Terminar,
    Tirar,
    MasVelocidad,
    MenosVelocidad,
    MasLetra,
    MenosLetra,
}

struct Boton {
    caja: RectF,
    que: Accion,
}

struct Estado {
    parrafos: Vec<String>,
    desfile: Desfile,
    velocidad: f32,
    tamano: f32,
    fase: Fase,
    /// Cuando empezo la grabacion, en milisegundos del reloj.
    inicio: u64,
    /// Cuando cambia el numero de la cuenta atras.
    cuenta: u32,
    cuenta_hasta: u64,
    grabadora: Option<Grabadora>,
    /// Con que letra y con que ancho se midio lo que hay en el desfile.
    medido_con: (f32, u32),
    botones: Vec<Boton>,
    raton: (f32, f32),
    /// Un aviso corto abajo (la nota salio demasiado corta, no hay micro).
    aviso: Option<(String, u64)>,
}

fn abrir(
    recursos: &Recursos,
    idioma: Idioma,
    texto: &str,
    destino: &Path,
    enviar: &Sender<Lectura>,
) -> Result<()> {
    // El catalogo se carga **aqui dentro** y no se recibe hecho: `Catalogo`
    // no cruza hilos, y esta ventana vive en el suyo.
    let textos = Catalogo::nuevo(idioma);
    let textos = &textos;
    let parrafos = telepronter::parrafos_de(texto);
    if parrafos.is_empty() {
        // Sin texto no hay telepronter. Quien llama elige la fuente; abrir
        // una pantalla negra para no leer nada seria peor que no abrirla.
        anyhow::bail!("el telepronter necesita un texto con algo escrito");
    }

    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .principal()
        .context("sin monitor principal")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let marco: Rect = monitor.area;

    let ventana = VentanaOverlay::nueva_normal(marco, &t(textos, Texto::Titulo))
        .context("no se pudo abrir la ventana del telepronter")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para el telepronter")?;
    ventana.mostrar();
    ventana.enfocar();

    let mut e = Estado {
        desfile: Desfile::nuevo(parrafos.len()),
        parrafos,
        velocidad: VELOCIDAD_POR_DEFECTO,
        tamano: TAMANO_POR_DEFECTO,
        fase: Fase::Parado,
        inicio: 0,
        cuenta: 0,
        cuenta_hasta: 0,
        grabadora: None,
        medido_con: (0.0, 0),
        botones: Vec::new(),
        raton: (0.0, 0.0),
        aviso: None,
    };

    let mut ultimo = pixpin_shell::entorno::ahora_utc_ms() as u64;
    let mut vivo = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match evento {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    e.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                }
                EventoOverlay::BotonPulsado(p) => {
                    e.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    if let Some(que) = bajo_el_raton(&e) {
                        hacer(&mut e, que, textos, enviar, &mut vivo);
                    }
                }
                EventoOverlay::Rueda(muescas) => {
                    // Empujar a mano cuenta igual que el desfile: los
                    // minutos salen de donde esta el texto.
                    e.desfile
                        .empujar(-(muescas as f32) * e.tamano * 3.0 * escala);
                }
                EventoOverlay::Tecla { vk, .. } => match vk {
                    VK_ESCAPE => hacer(&mut e, Accion::Cerrar, textos, enviar, &mut vivo),
                    // Espacio: ensayar o parar de ensayar, que es el mismo
                    // boton en el movil.
                    VK_ESPACIO => hacer(&mut e, Accion::Ensayar, textos, enviar, &mut vivo),
                    VK_INTRO => {
                        let que = if e.fase == Fase::Grabando {
                            Accion::Terminar
                        } else {
                            Accion::Grabar
                        };
                        hacer(&mut e, que, textos, enviar, &mut vivo);
                    }
                    VK_ARRIBA | VK_DERECHA => {
                        hacer(&mut e, Accion::MasVelocidad, textos, enviar, &mut vivo)
                    }
                    VK_ABAJO | VK_IZQUIERDA => {
                        hacer(&mut e, Accion::MenosVelocidad, textos, enviar, &mut vivo)
                    }
                    VK_MAS => hacer(&mut e, Accion::MasLetra, textos, enviar, &mut vivo),
                    VK_MENOS => hacer(&mut e, Accion::MenosLetra, textos, enviar, &mut vivo),
                    VK_NEXT => e.desfile.empujar(marco.alto as f32 * 0.5),
                    VK_PRIOR => e.desfile.empujar(-(marco.alto as f32) * 0.5),
                    _ => {}
                },
                _ => {}
            }
        }
        if !vivo {
            break;
        }

        let ahora = pixpin_shell::entorno::ahora_utc_ms() as u64;
        let segundos = (ahora.saturating_sub(ultimo)) as f32 / 1000.0;
        ultimo = ahora;

        // La cuenta atras baja sola y al llegar a cero arranca la grabacion.
        if e.fase == Fase::Contando && ahora >= e.cuenta_hasta {
            if e.cuenta > 1 {
                e.cuenta -= 1;
                e.cuenta_hasta = ahora + MS_POR_NUMERO;
            } else {
                e.cuenta = 0;
                grabar_ya(&mut e, textos, destino, ahora);
            }
        }

        if matches!(e.fase, Fase::Ensayando | Fase::Grabando) {
            // `avanzar` devuelve «ya no hay mas que bajar»: el movil para
            // ahi el desplazamiento, no la grabacion.
            if e.desfile.avanzar(segundos, e.velocidad * escala) && e.fase == Fase::Ensayando {
                e.fase = Fase::Parado;
            }
        }
        if e.fase == Fase::Grabando {
            e.desfile.apuntar(ahora.saturating_sub(e.inicio) as i64);
        }
        if e.aviso.as_ref().is_some_and(|(_, hasta)| *hasta < ahora) {
            e.aviso = None;
        }

        if let Ok(destino_sup) = superficie.empezar(&motor) {
            let _ = motor.dibujar(&destino_sup, |p: &Pintor| {
                medir(&mut e, p, marco, escala);
                pintar(&mut e, p, marco, escala, textos, ahora);
            });
            let _ = superficie.presentar();
        }
        // Cuarenta veces por segundo mientras el texto se mueve: por debajo
        // de eso el desfile se ve a saltos y leerlo cansa. Quieto, veinte
        // bastan.
        let espera = if e.fase == Fase::Parado { 50 } else { 25 };
        pixpin_shell::overlay::esperar_eventos(Some(espera));
    }

    // Cerrar con la grabacion puesta tira el fichero: no se guarda a
    // escondidas lo que el usuario no dio por bueno.
    if let Some(g) = e.grabadora.take() {
        g.cancelar();
    }
    Ok(())
}

/// El area donde desfila el texto.
fn area_de_lectura(marco: Rect, escala: f32) -> RectF {
    let barra = 56.0 * escala;
    let pie = 76.0 * escala;
    RectF {
        x: 0.0,
        y: barra,
        ancho: marco.ancho as f32,
        alto: (marco.alto as f32 - barra - pie).max(1.0),
    }
}

fn margen(marco: Rect, escala: f32) -> f32 {
    // Una columna estrecha se lee de un vistazo; el ancho entero de un
    // monitor de 27 pulgadas obliga a mover la cabeza en cada renglon.
    (marco.ancho as f32 * 0.16).max(24.0 * escala)
}

/// Mide los parrafos si cambio la letra o el ancho.
fn medir(e: &mut Estado, p: &Pintor, marco: Rect, escala: f32) {
    let area = area_de_lectura(marco, escala);
    let ancho = (area.ancho - margen(marco, escala) * 2.0).max(40.0);
    let tam = e.tamano * escala;
    if e.medido_con == (tam, ancho as u32) {
        e.desfile.ajustar_ventana(area.alto);
        return;
    }
    let separacion = tam * 0.9;
    let mut y = 0.0;
    let mut medidas = Vec::with_capacity(e.parrafos.len());
    for parrafo in &e.parrafos {
        let (_, alto) = p.medir_texto_ajustado(parrafo, tam, ancho);
        medidas.push(Medida { cima: y, alto });
        y += alto + separacion;
    }
    e.desfile.medir(medidas, area.alto);
    e.medido_con = (tam, ancho as u32);
}

fn pintar(e: &mut Estado, p: &Pintor, marco: Rect, escala: f32, textos: &Catalogo, ahora: u64) {
    let ancho = marco.ancho as f32;
    let alto = marco.alto as f32;
    p.limpiar(FONDO);

    let area = area_de_lectura(marco, escala);
    let izquierda = margen(marco, escala);
    let ancho_texto = (area.ancho - izquierda * 2.0).max(40.0);
    let tam = e.tamano * escala;

    // La franja de lectura: donde hay que tener los ojos.
    let linea = area.y + e.desfile.linea_de_lectura();
    p.rellenar(
        RectF {
            x: 0.0,
            y: linea - tam * 0.35,
            ancho,
            alto: tam * 1.5,
        },
        FRANJA_COLOR,
    );

    // Solo lo que se ve: un texto de mil parrafos no se pinta entero en
    // cada fotograma.
    p.con_recorte(area, |p| {
        for i in e.desfile.visibles() {
            let Some(m) = e.desfile.medida(i) else {
                continue;
            };
            let y = area.y + m.cima - e.desfile.y;
            let color = if e.desfile.tiempo(i).is_some() {
                APAGADO
            } else {
                TEXTO
            };
            p.texto_ajustado(&e.parrafos[i], izquierda, y, tam, ancho_texto, color);
        }
    });

    // Barra de arriba: volver, y el reloj de la grabacion.
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho,
            alto: area.y,
        },
        CRISTAL,
    );
    let rotulo = if e.fase == Fase::Grabando {
        format!(
            "● {}",
            reloj::duracion_legible(ahora.saturating_sub(e.inicio) as i64)
        )
    } else {
        t(textos, Texto::Titulo)
    };
    let color_rotulo = if e.fase == Fase::Grabando {
        ROJO
    } else {
        TEXTO
    };
    p.texto(
        &rotulo,
        64.0 * escala,
        16.0 * escala,
        17.0 * escala,
        color_rotulo,
    );

    // Pie: los mandos.
    let pie = RectF {
        x: 0.0,
        y: area.y + area.alto,
        ancho,
        alto: alto - area.y - area.alto,
    };
    p.rellenar(pie, CRISTAL);

    e.botones.clear();
    let alto_boton = 36.0 * escala;
    let y_boton = pie.y + (pie.alto - alto_boton) / 2.0;

    // Volver, arriba a la izquierda.
    boton(
        e,
        p,
        RectF {
            x: 12.0 * escala,
            y: 10.0 * escala,
            ancho: 44.0 * escala,
            alto: 36.0 * escala,
        },
        Accion::Cerrar,
        "←",
        false,
        escala,
    );

    let mut x = 12.0 * escala;
    for (que, rotulo, ancho_b) in [
        (Accion::MenosLetra, "A-", 44.0),
        (Accion::MasLetra, "A+", 44.0),
        (Accion::MenosVelocidad, "−", 44.0),
        (Accion::MasVelocidad, "+", 44.0),
    ] {
        let caja = RectF {
            x,
            y: y_boton,
            ancho: ancho_b * escala,
            alto: alto_boton,
        };
        boton(e, p, caja, que, rotulo, false, escala);
        x += (ancho_b + 6.0) * escala;
    }
    p.texto(
        &t(textos, Texto::Velocidad),
        x + 8.0 * escala,
        y_boton + 10.0 * escala,
        14.0 * escala,
        APAGADO,
    );

    // A la derecha, los dos grandes.
    let ancho_grande = 150.0 * escala;
    let (que, rotulo) = match e.fase {
        Fase::Grabando => (Accion::Terminar, t(textos, Texto::Terminar)),
        _ => (Accion::Grabar, t(textos, Texto::Grabar)),
    };
    boton(
        e,
        p,
        RectF {
            x: ancho - ancho_grande - 12.0 * escala,
            y: y_boton,
            ancho: ancho_grande,
            alto: alto_boton,
        },
        que,
        &rotulo,
        true,
        escala,
    );
    let (que2, rotulo2) = if e.fase == Fase::Grabando {
        (Accion::Tirar, t(textos, Texto::Tirar))
    } else if e.fase == Fase::Ensayando {
        (Accion::Ensayar, t(textos, Texto::Parar))
    } else {
        (Accion::Ensayar, t(textos, Texto::Ensayar))
    };
    boton(
        e,
        p,
        RectF {
            x: ancho - ancho_grande * 2.0 - 20.0 * escala,
            y: y_boton,
            ancho: ancho_grande,
            alto: alto_boton,
        },
        que2,
        &rotulo2,
        false,
        escala,
    );

    if let Some((aviso, _)) = &e.aviso {
        p.texto(
            aviso,
            izquierda,
            pie.y - 28.0 * escala,
            15.0 * escala,
            DORADO,
        );
    }

    // **Tres, dos, uno**: encima de todo, que es lo unico que hay que mirar
    // mientras se cuenta.
    if e.fase == Fase::Contando && e.cuenta > 0 {
        p.rellenar(
            RectF {
                x: 0.0,
                y: 0.0,
                ancho,
                alto,
            },
            hex(0x0c0c10),
        );
        let numero = e.cuenta.to_string();
        let tam_numero = 96.0 * escala;
        let (w, _) = p.medir_texto(&numero, tam_numero);
        p.texto(
            &numero,
            (ancho - w) / 2.0,
            alto / 2.0 - tam_numero,
            tam_numero,
            TEXTO,
        );
        let listo = t(textos, Texto::Preparate);
        let listo = listo.as_str();
        let (w2, _) = p.medir_texto(listo, 20.0 * escala);
        p.texto(
            listo,
            (ancho - w2) / 2.0,
            alto / 2.0 + 20.0 * escala,
            20.0 * escala,
            APAGADO,
        );
    }
}

/// Pinta un boton y lo apunta para el raton.
fn boton(
    e: &mut Estado,
    p: &Pintor,
    caja: RectF,
    que: Accion,
    rotulo: &str,
    encendido: bool,
    escala: f32,
) {
    let dentro = e.raton.0 >= caja.x
        && e.raton.0 <= caja.x + caja.ancho
        && e.raton.1 >= caja.y
        && e.raton.1 <= caja.y + caja.alto;
    let fondo = if encendido {
        ROJO
    } else if dentro {
        hex(0x36363e)
    } else {
        hex(0x2c2c34)
    };
    p.rellenar_redondeado(caja, 8.0 * escala, fondo);
    let tam = 15.0 * escala;
    let (w, h) = p.medir_texto(rotulo, tam);
    p.texto(
        rotulo,
        caja.x + (caja.ancho - w) / 2.0,
        caja.y + (caja.alto - h) / 2.0,
        tam,
        TEXTO,
    );
    e.botones.push(Boton { caja, que });
}

fn bajo_el_raton(e: &Estado) -> Option<Accion> {
    e.botones
        .iter()
        .find(|b| {
            e.raton.0 >= b.caja.x
                && e.raton.0 <= b.caja.x + b.caja.ancho
                && e.raton.1 >= b.caja.y
                && e.raton.1 <= b.caja.y + b.caja.alto
        })
        .map(|b| b.que)
}

fn hacer(
    e: &mut Estado,
    que: Accion,
    textos: &Catalogo,
    enviar: &Sender<Lectura>,
    vivo: &mut bool,
) {
    let ahora = pixpin_shell::entorno::ahora_utc_ms() as u64;
    match que {
        Accion::Cerrar => *vivo = false,
        Accion::Ensayar => {
            e.fase = match e.fase {
                Fase::Ensayando => Fase::Parado,
                // Ensayar mientras se graba no tiene sentido: ya esta
                // bajando. Se deja como esta.
                Fase::Grabando | Fase::Contando => e.fase,
                Fase::Parado => Fase::Ensayando,
            };
        }
        Accion::Grabar => {
            if e.fase != Fase::Grabando {
                e.fase = Fase::Contando;
                e.cuenta = CUENTA_ATRAS;
                e.cuenta_hasta = ahora + MS_POR_NUMERO;
            }
        }
        Accion::Tirar => {
            if let Some(g) = e.grabadora.take() {
                g.cancelar();
            }
            e.fase = Fase::Parado;
            e.desfile.reiniciar();
        }
        Accion::Terminar => terminar(e, textos, enviar, vivo),
        Accion::MasVelocidad => e.velocidad = telepronter::acotar_velocidad(e.velocidad + 5.0),
        Accion::MenosVelocidad => e.velocidad = telepronter::acotar_velocidad(e.velocidad - 5.0),
        Accion::MasLetra => {
            e.tamano = telepronter::acotar_tamano(e.tamano + 2.0);
            e.medido_con = (0.0, 0);
        }
        Accion::MenosLetra => {
            e.tamano = telepronter::acotar_tamano(e.tamano - 2.0);
            e.medido_con = (0.0, 0);
        }
    }
}

/// Arranca la grabacion de verdad, cuando la cuenta llega a cero.
fn grabar_ya(e: &mut Estado, textos: &Catalogo, destino: &Path, ahora: u64) {
    let fichero = destino.join(format!("lectura-{ahora}.m4a"));
    match Grabadora::empezar(&fichero) {
        Ok(g) => {
            e.grabadora = Some(g);
            e.inicio = ahora;
            e.desfile.reiniciar();
            e.fase = Fase::Grabando;
        }
        Err(err) => {
            // Que no se pueda grabar es **normal** —otra aplicacion tiene
            // el microfono, el permiso esta quitado— y hay que decirlo en
            // el acto en vez de fingir que se graba.
            tracing::info!(?err, "no se pudo grabar la lectura");
            e.fase = Fase::Parado;
            e.aviso = Some((t(textos, Texto::SinMicro), ahora + 4_000));
        }
    }
}

fn terminar(e: &mut Estado, textos: &Catalogo, enviar: &Sender<Lectura>, vivo: &mut bool) {
    e.fase = Fase::Parado;
    let Some(g) = e.grabadora.take() else {
        return;
    };
    let ahora = pixpin_shell::entorno::ahora_utc_ms() as u64;
    let grabacion = match g.parar() {
        Ok(g) => g,
        Err(err) => {
            tracing::info!(?err, "la lectura no se pudo cerrar");
            e.aviso = Some((t(textos, Texto::MuyCorta), ahora + 4_000));
            return;
        }
    };
    if grabacion.duracion_ms < MINIMO_MS {
        let _ = std::fs::remove_file(&grabacion.ruta);
        e.aviso = Some((t(textos, Texto::MuyCorta), ahora + 4_000));
        return;
    }

    let lectura = Lectura {
        cuerpo: e.desfile.cuerpo(&e.parrafos, grabacion.duracion_ms),
        ruta: grabacion.ruta,
        duracion_ms: grabacion.duracion_ms,
        picos: grabacion.picos,
    };
    // Si el chat ya se cerro, el canal esta roto: el fichero se queda en su
    // carpeta y no se pierde nada.
    let _ = enviar.send(lectura);
    *vivo = false;
}

/// Los rotulos de esta ventana, ya en el catalogo de `pixpin-store`.
///
/// El enum se queda —y no se llama a `textos.t("...")` a pelo— porque una
/// clave mal escrita en Fluent no deja de compilar: devuelve la propia clave
/// y el boton sale rotulado `telepronter-grabar`. Con el enum, inventarse un
/// rotulo es un error del compilador.
#[derive(Debug, Clone, Copy)]
enum Texto {
    Titulo,
    Ensayar,
    Parar,
    Grabar,
    Terminar,
    Tirar,
    Velocidad,
    Preparate,
    MuyCorta,
    SinMicro,
}

fn t(textos: &Catalogo, que: Texto) -> String {
    textos.t(match que {
        Texto::Titulo => "telepronter-titulo",
        Texto::Ensayar => "telepronter-ensayar",
        Texto::Parar => "telepronter-parar",
        Texto::Grabar => "telepronter-grabar",
        Texto::Terminar => "telepronter-terminar",
        Texto::Tirar => "telepronter-tirar",
        Texto::Velocidad => "telepronter-velocidad",
        Texto::Preparate => "telepronter-preparate",
        Texto::MuyCorta => "telepronter-muy-corta",
        Texto::SinMicro => "telepronter-sin-micro",
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn lectura() -> Lectura {
        Lectura {
            ruta: PathBuf::from("C:/datos/voz/lectura-1758351600000.m4a"),
            duracion_ms: 12_500,
            picos: vec![10, 20_000, 32_767],
            cuerpo: "[0:00] primero\n\n[0:07] segundo".into(),
        }
    }

    #[test]
    fn la_lectura_se_escribe_como_una_nota_de_voz_con_su_texto() {
        let mut m = Mensaje::default();
        aplicar(&mut m, &lectura());
        assert_eq!(m.clase, Some(Clase::Voz));
        assert_eq!(m.nombre, "lectura-1758351600000.m4a");
        assert_eq!(m.duracion_ms, 12_500);
        assert_eq!(
            m.transcripcion.as_deref(),
            Some("[0:00] primero\n\n[0:07] segundo")
        );
    }

    #[test]
    fn los_picos_viajan_como_enteros_crudos_en_el_campo_del_movil() {
        let mut m = Mensaje::default();
        aplicar(&mut m, &lectura());
        let picos = m.resto.get("picos").and_then(|v| v.as_array()).unwrap();
        assert_eq!(picos.len(), 3);
        assert_eq!(picos[2].as_i64(), Some(32_767));
    }

    #[test]
    fn el_texto_de_una_lectura_nace_como_bien_porque_no_lo_adivino_nadie() {
        let mut m = Mensaje::default();
        aplicar(&mut m, &lectura());
        assert_eq!(
            m.resto.get("estadoDelTexto").and_then(|v| v.as_str()),
            Some("bien")
        );
    }

    #[test]
    fn una_lectura_sin_picos_no_escribe_una_onda_falsa() {
        let mut l = lectura();
        l.picos.clear();
        let mut m = Mensaje::default();
        aplicar(&mut m, &l);
        assert_eq!(
            m.resto
                .get("picos")
                .and_then(|v| v.as_array())
                .map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn los_rotulos_estan_en_los_dos_idiomas() {
        let es = Catalogo::nuevo(Idioma::Espanol);
        let en = Catalogo::nuevo(Idioma::Ingles);
        assert_eq!(t(&es, Texto::Grabar), "Grabar");
        assert_eq!(t(&en, Texto::Grabar), "Record");
    }

    /// Un rotulo que falte en el catalogo sale como su propia clave, y eso
    /// no deja de compilar: lo tiene que cazar una prueba.
    #[test]
    fn ningun_rotulo_del_telepronter_sale_como_su_clave() {
        for idioma in [Idioma::Espanol, Idioma::Ingles] {
            let c = Catalogo::nuevo(idioma);
            for que in [
                Texto::Titulo,
                Texto::Ensayar,
                Texto::Parar,
                Texto::Grabar,
                Texto::Terminar,
                Texto::Tirar,
                Texto::Velocidad,
                Texto::Preparate,
                Texto::MuyCorta,
                Texto::SinMicro,
            ] {
                let rotulo = t(&c, que);
                assert!(
                    !rotulo.starts_with("telepronter-"),
                    "falta {rotulo} en el catalogo de {idioma:?}"
                );
            }
        }
    }
}
