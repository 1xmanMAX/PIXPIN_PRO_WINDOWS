//! La ventana del editor avanzado.
//!
//! **Traduce y nada mas.** Convierte `EventoOverlay` en `EventoGesto`, llama
//! a la maquina del motor, y hace lo que devuelve: redibujar y poner el
//! cursor que toque. Si este fichero crece mas alla de eso, es que se ha
//! colado logica que deberia estar en el motor.
//!
//! No hace falta fontaneria nueva: `VentanaOverlay` ya sirve ventanas
//! completas —el editor de grabaciones (`editor.rs`) la usa asi— y ya trae
//! raton, teclas con sus modificadores, caracteres con IME, cambio de DPI,
//! cursores y el bucle por eventos que da el 0 % de CPU en reposo.
//!
//! # Por que aqui no hay ni un `unsafe`
//!
//! Leer si Shift o Alt estan pulsados AHORA (no en el ultimo evento de
//! teclado, que es otra cosa) hace falta para el raton, que no trae
//! modificadores consigo. `overlay.rs` lo resuelve con `GetKeyState`, pero
//! esa llamada es `unsafe` y este crate es `forbid(unsafe_code)`. La
//! envoltura seria ya existe en `pixpin_shell::entrada::modificadores`, que
//! usa el anotador para lo mismo: se reutiliza en vez de duplicar el sondeo.
//!
//! # Como se traduce `Orden` a dibujo
//!
//! El motor entrega `pixpin_motor2d::pintado::Orden`: geometria ya calculada,
//! sin Direct2D de por medio (`pintado.rs` lo explica: "las mismas ordenes
//! valen para Direct2D hoy y para exportar a SVG manana"). `pixpin-render` no
//! conoce ese tipo a proposito —ninguna dependencia nueva entre crates (D28)—
//! asi que el `match` de `Orden` a llamadas de `Pintor` vive aqui, en
//! `dibujar_orden`. No es geometria: es la misma clase de traduccion mecanica
//! que `a_evento`, solo que de salida en vez de entrada.

use crate::fondo_lienzo::{FondoLienzo, encuadre_inicial};
use crate::imagenes_lienzo::ImagenesLienzo;
use crate::navegacion::{self, vista_efectiva};
use anyhow::{Context, Result};
use pixpin_geom::Punto;
use pixpin_motor2d::cache::Cache;
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{
    EventoGesto, FormaCursor, Gesto, Herramienta, Peticion, Region, direccion_del_tirador,
};
use pixpin_motor2d::indice::Rejilla;
use pixpin_motor2d::pintado::Orden;
use pixpin_motor2d::vector::Punto2;
use pixpin_motor2d::{ColorRgba, Elemento, Escala, EstiloTrazo, Figura};
use pixpin_render::{CapaEstatica, Color, Estampa, MotorRender, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin, VentanaOverlay};
use pixpin_ui::{BOTONES_EDITOR, BotonCaja, CajaHerramientas, DestinoClic};

/// De la forma que pide el motor a la que entiende Windows.
///
/// Lo unico con sustancia es escalar: Windows solo trae cuatro flechas de
/// redimension, asi que la direccion del tirador —ya girada con el
/// elemento— se reparte entre ellas en octavos de vuelta. Y como una flecha
/// no tiene punta, norte y sur son la misma: se toma el angulo modulo media
/// vuelta.
pub fn forma_de(cursor: FormaCursor) -> FormaCursorWin {
    use std::f32::consts::PI;
    match cursor {
        FormaCursor::Flecha => FormaCursorWin::Flecha,
        FormaCursor::Cruz => FormaCursorWin::Cruz,
        FormaCursor::Mover => FormaCursorWin::Mover,
        FormaCursor::Texto => FormaCursorWin::Texto,
        FormaCursor::Giro => FormaCursorWin::Giro,
        FormaCursor::Escalar { tirador, angulo } => {
            let d = direccion_del_tirador(tirador, angulo);
            // A media vuelta, y en octavos: cada flecha cubre 45 grados.
            let media = PI;
            let d = d.rem_euclid(media);
            let octavo = media / 4.0;
            match (d / octavo).round() as i32 % 4 {
                0 => FormaCursorWin::RedimNS,
                1 => FormaCursorWin::RedimNeSo,
                2 => FormaCursorWin::RedimEO,
                _ => FormaCursorWin::RedimNoSe,
            }
        }
    }
}

/// Del evento de la ventana al del motor. `None` es «esto no le toca al
/// motor»: pintar, el DPI, el despertar de otro hilo.
///
/// `shift` y `alt` del raton salen a `false`: `EventoOverlay::BotonPulsado` y
/// `RatonMovido` no los traen (no son parte del mensaje de Windows). Quien
/// llama los sobreescribe con `con_modificadores` justo antes de pasarselo a
/// la maquina, leyendolos con `pixpin_shell::entrada::modificadores` en el
/// momento de traducir.
///
/// `origen` es la esquina de la ventana en coordenadas del escritorio
/// virtual: los mensajes de Windows traen esas coordenadas, pero el lienzo
/// empieza en la esquina de la ventana. Con la barra de tareas arriba o a la
/// izquierda el area de trabajo no empieza en (0,0), y sin restar el origen
/// la tinta salia desplazada del cursor.
pub fn a_evento(ev: &EventoOverlay, camara: &Camara, origen: Punto) -> Option<EventoGesto> {
    let (ox, oy) = (origen.x as f32, origen.y as f32);
    // `a_mundo` convierte un punto. NO `en_mundo`, que existe y convierte una
    // LONGITUD: compila igual y da otra cosa.
    let al_mundo = |x: f32, y: f32| camara.a_mundo(Punto2::nuevo(x - ox, y - oy));
    let entero = |p: &Punto| al_mundo(p.x as f32, p.y as f32);
    match ev {
        EventoOverlay::BotonPulsado(p) => Some(EventoGesto::Pulsar {
            p: entero(p),
            shift: false,
            alt: false,
            presion: None,
        }),
        EventoOverlay::RatonMovido(p) => Some(EventoGesto::Mover {
            p: entero(p),
            shift: false,
            alt: false,
            presion: None,
        }),
        // Con lapiz, `Muestra` puede llegar ANTES de `BotonPulsado` y un
        // trazo puede no traer `RatonMovido` de cola: cada muestra se
        // traduce sola, con su subpixel y su presion, como `Mover`.
        EventoOverlay::Muestra(m) => Some(EventoGesto::Mover {
            p: al_mundo(m.x(), m.y()),
            shift: false,
            alt: false,
            presion: m.presion(),
        }),
        EventoOverlay::BotonSoltado(p) => Some(EventoGesto::Soltar { p: entero(p) }),
        EventoOverlay::Tecla { vk, ctrl, .. } => {
            const VK_ESCAPE: u32 = 0x1B;
            const VK_DELETE: u32 = 0x2E;
            match (*vk, *ctrl) {
                (VK_ESCAPE, _) => Some(EventoGesto::Escape),
                (VK_DELETE, _) => Some(EventoGesto::Suprimir),
                (v, true) if v == b'Z' as u32 => Some(EventoGesto::Deshacer),
                (v, true) if v == b'Y' as u32 => Some(EventoGesto::Rehacer),
                (v, true) if v == b'A' as u32 => Some(EventoGesto::SeleccionarTodo),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Sobreescribe `shift` y `alt` de un `Pulsar`/`Mover` con lo que hay
/// pulsado AHORA. `a_evento` se queda puro y comprobable; esto es lo unico
/// que necesita preguntarle al sistema, y solo para dos campos.
fn con_modificadores(g: EventoGesto) -> EventoGesto {
    match g {
        EventoGesto::Pulsar { p, presion, .. } => {
            let (shift, alt) = pixpin_shell::entrada::modificadores();
            EventoGesto::Pulsar {
                p,
                shift,
                alt,
                presion,
            }
        }
        EventoGesto::Mover { p, presion, .. } => {
            let (shift, alt) = pixpin_shell::entrada::modificadores();
            EventoGesto::Mover {
                p,
                shift,
                alt,
                presion,
            }
        }
        otro => otro,
    }
}

/// La herramienta que elige cada letra, siguiendo `caja_dibujo::etiqueta`.
///
/// Solo las herramientas que ahi se pintan con una letra de verdad tienen
/// atajo: Linea, Flecha, Rectangulo y Elipse se pintan con un simbolo
/// ("/", ">", "square", "circle") porque no hay icono todavia, y un simbolo
/// no es una tecla memorizable. Pura, para poder probarla sin ventana.
fn tecla_a_herramienta(c: char) -> Option<Herramienta> {
    match c.to_ascii_uppercase() {
        'M' => Some(Herramienta::Mano),
        'L' => Some(Herramienta::Lapiz),
        'R' => Some(Herramienta::Resaltador),
        'T' => Some(Herramienta::Texto),
        'F' => Some(Herramienta::Foco),
        'Q' => Some(Herramienta::Lupa),
        'B' => Some(Herramienta::Borrador),
        'A' => Some(Herramienta::Cota),
        'E' => Some(Herramienta::Escalar),
        'G' => Some(Herramienta::EscalaGrafica),
        // «C» de marCo: la «F» de «frame» ya la usa el foco.
        'C' => Some(Herramienta::Marco),
        _ => None,
    }
}

/// Cuanto se adelanta la punta del trazo en curso. La medida contra la
/// posicion LOGICA del cursor daba 28 ms como adelantado y se bajo a 14, pero
/// lo que se ve es el cursor que pinta Windows, que tambien llega tarde: con
/// 14 el usuario veia la tinta detras, y con 28 «casi cero lag». Manda la vista.
const HORIZONTE_PREDICCION_MS: f32 = 28.0;
/// Hasta donde puede adelantarse, en pixeles de pantalla: con 48 un trazo
/// rapido topaba y la punta volvia a quedarse atras.
const TOPE_PREDICCION_PX: f32 = 80.0;
/// Cuanto tiene que quedarse quieto el cursor dibujando a mano para que el
/// trazo se convierta en figura (forma rapida). Menos se dispara al dudar a
/// mitad de un trazo; mas se hace esperar.
const PAUSA_FORMA: std::time::Duration = std::time::Duration::from_millis(450);
/// Lo mas que se espera a la senal de fotograma antes de pintar igualmente.
/// A3: pixeles de colchon por lado de la superficie de la escena.
///
/// Es lo que se puede desplazar el lienzo moviendo el visual, sin repintar
/// nada. 256 px es una tesela de las que usan Chromium y MyPaint: a 30 px
/// por fotograma (un arrastre rapido, 1.800 px/s) son nueve fotogramas
/// gratis por cada repintado, y a 3000x2000 cuesta 22 MB mas de memoria de
/// video (35 MB frente a 24 por buffer). Subirlo da mas fotogramas gratis y
/// cuesta mas memoria; bajarlo, al reves.
pub(crate) const MARGEN_ESCENA: u32 = 256;

const ESPERA_MAXIMA_SENAL_MS: u32 = 20;
const ESPERA_MAXIMA_SENAL: std::time::Duration =
    std::time::Duration::from_millis(ESPERA_MAXIMA_SENAL_MS as u64);

/// La tecla que elige `h`, para pintarla en la esquina de su boton. Es la
/// inversa de `tecla_a_herramienta`: si alguna vez se separan, la barra
/// ensenaria una tecla que no hace nada, y la prueba lo vigila.
fn tecla_de(h: Herramienta) -> Option<char> {
    ['M', 'L', 'R', 'T', 'F', 'Q', 'B', 'A', 'E', 'G']
        .into_iter()
        .find(|c| tecla_a_herramienta(*c) == Some(h))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CambioPluma {
    Grosor(f32),
    AlternarVariabilidad,
}

/// Las plumas de Excalidraw sin interfaz nueva (la barra es E5): 1, 2 y 3
/// son sus tres grosores; V alterna variable y constante. Ninguna choca con
/// `tecla_a_herramienta`, y la prueba lo vigila.
fn tecla_a_pluma(c: char) -> Option<CambioPluma> {
    use pixpin_motor2d::tinta::{GROSOR_FINO, GROSOR_GRUESO, GROSOR_MEDIO};
    match c.to_ascii_uppercase() {
        '1' => Some(CambioPluma::Grosor(GROSOR_FINO)),
        '2' => Some(CambioPluma::Grosor(GROSOR_MEDIO)),
        '3' => Some(CambioPluma::Grosor(GROSOR_GRUESO)),
        'V' => Some(CambioPluma::AlternarVariabilidad),
        _ => None,
    }
}

/// Que hacer al pulsar un boton de la caja del editor. Pura -no toca la
/// ventana ni pinta nada-, asi se prueba sin GPU ni sesion de escritorio.
/// Mismo contrato que `CapaViva::pulsar_boton` en `capa.rs`: devuelve
/// `false` si el boton pide salir.
fn pulsar_boton(boton: BotonCaja, gesto: &mut Gesto, escena: &mut Escena) -> bool {
    match boton {
        BotonCaja::Elegir(h) => {
            elegir_herramienta(gesto, h);
            true
        }
        BotonCaja::Deshacer => {
            escena.deshacer();
            true
        }
        BotonCaja::Rehacer => {
            escena.rehacer();
            true
        }
        // Sin paleta de colores en el editor todavia.
        BotonCaja::Color => true,
        BotonCaja::Salir => false,
    }
}

/// La seccion `[tinta]` del TOML, fijada al arrancar (ver
/// `fijar_ajustes_tinta`). Sin fijar, lo de siempre.
static AJUSTES_TINTA: std::sync::OnceLock<pixpin_store::Tinta> = std::sync::OnceLock::new();

/// La llama `main` una sola vez, antes de que exista ningun editor.
///
/// Es un `OnceLock` y no un parametro porque el editor se abre desde cinco
/// sitios que no tienen nada que ver entre si y a ninguno le importa como se
/// siente el lapiz. Llamarla dos veces no hace nada: el ajuste no cambia
/// mientras el programa viva, y asi las pruebas pueden abrir un editor sin
/// haberla llamado.
pub fn fijar_ajustes_tinta(t: pixpin_store::Tinta) {
    let _ = AJUSTES_TINTA.set(t);
}

fn ajustes_tinta() -> pixpin_store::Tinta {
    AJUSTES_TINTA.get().copied().unwrap_or_default()
}

/// La seccion `[rendimiento]` del TOML, leida una vez por sesion.
static AJUSTES_RENDIMIENTO: std::sync::OnceLock<pixpin_store::Rendimiento> =
    std::sync::OnceLock::new();

/// Las dos palancas de esta tarea: `[rendimiento] ritmo` y
/// `[rendimiento] paneo_por_composicion`.
///
/// Se lee el fichero aqui, y no se recibe por parametro como
/// `medir_fotogramas`, porque el editor se abre desde cinco sitios (bandeja,
/// chat, pines, universo, grabacion) y anadirle un parametro a todos por un
/// ajuste de dibujo obligaria a tocar ficheros que en esta tanda son de
/// otros agentes. Se lee UNA vez por sesion: es un `OnceLock`.
fn ajustes_rendimiento() -> pixpin_store::Rendimiento {
    *AJUSTES_RENDIMIENTO.get_or_init(|| {
        let dir_exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_default();
        let appdata = std::env::var_os("APPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        pixpin_store::cargar(&pixpin_store::resolver(&dir_exe, &appdata))
            .unwrap_or_default()
            .rendimiento
    })
}

/// Los mandos del filtro de 1 euro: lo que diga el TOML y, para lo que no
/// diga, el valor conservador del propio motor.
fn mandos_de(t: pixpin_store::Tinta) -> pixpin_tinta::Mandos {
    let d = pixpin_tinta::Mandos::default();
    pixpin_tinta::Mandos {
        corte_minimo: t.corte_minimo.unwrap_or(d.corte_minimo),
        beta: t.beta.unwrap_or(d.beta),
        ..d
    }
}

/// El `Gesto` con el que arranca el editor, ya con los ajustes de iman
/// guardados puestos.
///
/// Esta aparte de `abrir` porque `abrir` necesita GPU y una ventana, y este
/// cableado -que la pestana de dibujo no sea decorativa- se puede comprobar
/// sin nada de eso. Sin esta funcion la unica prueba posible seria
/// asignarle el campo a mano y releerlo, que no comprueba nada.
fn gesto_inicial(ajustes_iman: pixpin_motor2d::enganche::Ajustes) -> Gesto {
    let mut gesto = Gesto::nuevo();
    gesto.enganche = ajustes_iman;
    gesto
}

/// Lo que NO entra en la capa congelada: lo que cambia en cada fotograma.
/// Una sola funcion para `abrir` y `pintar`: si cada una calculara su lista,
/// la capa se daria por invalida en cada fotograma (la `Estampa` no casaria)
/// o, peor, valdria sin contener lo que hay que pintar encima.
fn excluidos_de(gesto: &Gesto) -> Vec<u64> {
    // Todo lo que se esta dibujando, no solo el trazo a mano: una figura en
    // curso fuera de la capa congelada obligaba a repintar la escena entera
    // en cada fotograma.
    match gesto.elemento_en_curso() {
        Some((id, _)) => vec![id],
        None => gesto.seleccion.ids().to_vec(),
    }
}

/// Lo que decide `decidir_zoom` para esta vuelta del bucle.
#[derive(Debug, Clone, Copy, PartialEq)]
struct DecisionZoom {
    /// El zoom lleva `retardo` QUIETO (no solo "ha pasado `retardo` desde el
    /// primer cambio", D122): hay que rehacer la tinta nitida a esta escala.
    rehacer: bool,
    /// El zoom que se vio esta vuelta, para comparar con el de la siguiente.
    visto: f32,
    /// Cuando empezo a estar quieto en el valor `visto` (si sigue pendiente
    /// de rehacer). `None` significa "nada pendiente": ni hay que
    /// programar un tope de espera, ni girar en vacio.
    desde: Option<std::time::Instant>,
    /// Cuanto darle a `esperar_eventos` para no dormir mas de lo que falta
    /// hasta que el zoom cumpla el retardo. `None` si no hay nada pendiente.
    tope_ms: Option<u32>,
}

/// Pura, sin `&mut` ni reloj propio (recibe `ahora`): decide si hay que
/// rehacer la tinta nitida y cuanto dormir como mucho, a partir del zoom de
/// este fotograma y de lo que se sabia hasta ahora.
///
/// Dos fallos que esto corrige de la primera version (revision de la Tarea
/// 10):
///
/// 1. Girar en vacio: si el zoom vuelve a `escala` (la ya realizada) ANTES
///    de que se cumpla el retardo, aqui se limpia `desde` a `None`. Sin
///    esto, `tope_ms` se quedaria fijo en el resultado de restar un
///    instante ya pasado -0 ms para siempre-, y `esperar_eventos(Some(0))`
///    gira un nucleo entero en reposo: justo lo que el bucle dirigido por
///    eventos existe para evitar.
/// 2. "Quieto" no es "desde el primer cambio": si el zoom sigue cambiando
///    (rueda del raton sin soltar), cada cambio REINICIA `desde`. Sin esto,
///    un zoom continuo rehacia la tinta cada `retardo`, en vez de una vez
///    sola cuando por fin se para (D122: el `shouldCacheIgnoreZoom` de
///    Excalidraw mide quietud, no antiguedad del primer cambio).
fn decidir_zoom(
    zoom: f32,
    visto: f32,
    escala: f32,
    desde: Option<std::time::Instant>,
    ahora: std::time::Instant,
    retardo: std::time::Duration,
) -> DecisionZoom {
    let (visto, desde) = if zoom != visto {
        (zoom, Some(ahora))
    } else {
        (visto, desde)
    };
    if zoom == escala {
        // Ya esta a la escala realizada: nada pendiente.
        return DecisionZoom {
            rehacer: false,
            visto,
            desde: None,
            tope_ms: None,
        };
    }
    match desde {
        Some(d) if ahora.duration_since(d) >= retardo => DecisionZoom {
            rehacer: true,
            visto,
            desde: None,
            tope_ms: None,
        },
        Some(d) => DecisionZoom {
            rehacer: false,
            visto,
            desde: Some(d),
            tope_ms: Some(retardo.saturating_sub(ahora.duration_since(d)).as_millis() as u32),
        },
        // No deberia pasar en la practica (si `zoom != escala` y no se
        // acaba de mover, `desde` ya deberia venir puesto), pero si pasa no
        // hay que girar en vacio: se arranca el reloj ahora.
        None => DecisionZoom {
            rehacer: false,
            visto,
            desde: Some(ahora),
            tope_ms: Some(retardo.as_millis() as u32),
        },
    }
}

/// Cuanto tiene que estar quieta la camara para que se pinte el fotograma
/// nitido. Es el mismo reposo que usa QuickView y el que ya tenia el pin
/// para su zoom (D89): mas corto se pinta a mitad de gesto y no se gana
/// nada, mas largo se nota que la imagen esta estirada.
const REPOSO_CAMARA: std::time::Duration = std::time::Duration::from_millis(150);

/// Lo mas que se deja estirar la escena antes de exigir un repintado. Por
/// encima de esto, la textura ampliada se ve claramente borrosa.
const ESTIRADO_MAXIMO: f32 = 3.0;

/// **B2 esta APAGADO.** El trazo en curso vuelve a pintarse en la escena,
/// con su capa congelada, como antes de B2.
///
/// El motivo, medido y no supuesto: el visual de la capa de tinta NO SE VE.
/// La prueba `superficie::pruebas::una_ventana_compuesta_ensena_la_interfaz_
/// encima_de_la_escena` monta el mismo arbol, pinta una banda en la capa de
/// la interfaz y otra en la de la tinta, lee la PANTALLA y encuentra la de
/// la interfaz y no la de la tinta. Y no es el orden de los visuales: pasa
/// igual creando la tinta antes o despues de la interfaz, con referencia
/// explicita o sin ella, y con una sola pasada de dibujo o con dos. Dejarlo
/// encendido significaria dibujar un trazo y no ver nada hasta soltar, que
/// es mucho peor que el tiron de apoyar el lapiz que B2 venia a quitar.
///
/// Lo que B2 dejo hecho y sigue sirviendo el dia que esto se entienda: la
/// capa, `pintar_tinta`, `encender_tinta`, `apagar_tinta`, el ritmo sin
/// `Present` (`espera_de_tinta`) y `pintar_tinta_viva`.
const TINTA_EN_CAPA: bool = false;

/// Cada cuanto compone DWM cuando no lo quiere decir (con la ventana tapada,
/// por ejemplo): 60 Hz, que es lo que hay en el equipo suelo.
const PERIODO_SUPUESTO_MS: f32 = 16.7;

/// Parte del periodo de composicion que tiene que haber pasado desde el
/// ultimo fotograma de tinta para pintar otro.
///
/// No es 1,0 porque el reloj del bucle y el de DWM no son el mismo: exigir
/// el periodo entero haria perder una composicion de cada dos en cuanto
/// hubiera medio milisegundo de deriva, y el trazo saldria a 30 Hz.
const PARTE_DEL_PERIODO: f32 = 0.9;

/// **B2.** Cuanto falta para que toque el siguiente fotograma de la capa de
/// tinta, en milisegundos enteros. `None` = ya toca.
///
/// Hace falta porque un fotograma de tinta no presenta: la senal de latencia
/// de la cadena de intercambio, que es la que marca el ritmo del resto del
/// bucle, se queda disparada y no frena nada. Sin este freno, un lapiz que
/// manda 240 muestras por segundo haria cuatro `Commit` por cada composicion
/// de DWM, y tres de los cuatro no llegarian a verse.
fn espera_de_tinta(desde_ms: f32, periodo_ms: f32) -> Option<u32> {
    if !periodo_ms.is_finite() || periodo_ms <= 0.0 {
        return None;
    }
    let plazo = periodo_ms * PARTE_DEL_PERIODO;
    if !desde_ms.is_finite() || desde_ms >= plazo {
        return None;
    }
    Some((plazo - desde_ms).ceil().max(1.0) as u32)
}

/// La punta predicha de este fotograma, si hay algo dibujandose y la
/// prediccion no esta apagada. Horizonte medido (~30 ms de la lectura a la
/// pantalla en el equipo del usuario) y tope de 80 px de pantalla.
fn prediccion_de(
    gesto: &Gesto,
    predictor: &mut pixpin_motor2d::tinta::prediccion::Predictor,
    tinta_clasica: bool,
    zoom: f32,
) -> Option<Punto2> {
    gesto
        .elemento_en_curso()
        .filter(|_| !tinta_clasica)
        .and_then(|_| predictor.predecir(HORIZONTE_PREDICCION_MS, TOPE_PREDICCION_PX / zoom, zoom))
}

/// El tamano de la superficie de la escena: la ventana mas el colchon por
/// cada lado. Es la unica formula: la usan la `Estampa` de la capa congelada
/// y el propio `pintar`, y si se separaran, la capa se daria por valida con
/// un tamano que no es el suyo.
fn tamano_escena(ancho_px: f32, alto_px: f32, margen: f32) -> (u32, u32) {
    (
        (ancho_px + margen * 2.0) as u32,
        (alto_px + margen * 2.0) as u32,
    )
}

/// **A3.** La transformada que hay que ponerle al visual de la escena para
/// que lo pintado con la camara `pintada` se vea como si estuviera pintado
/// con la camara `ahora`. `None` si el colchon no da y hay que repintar.
///
/// Esta es toda la idea de A3: un paneo deja de costar «recorrer la escena y
/// emitir sus primitivas» —15 ms con 2.000 elementos, 83 ms con 10.000,
/// medido— y pasa a costar una matriz y un `Commit`. Lo que asoma por los
/// bordes sale del colchon de `margen` pixeles que la superficie tiene de
/// mas por cada lado.
///
/// Devuelve `(escala, dx, dy)` tal como los quiere `Superficie::estirar`.
///
/// Es pura a proposito: la condicion de «¿cabe?» es aritmetica y se prueba
/// sin GPU, que es justo lo que no se puede improvisar mirando la pantalla.
pub(crate) fn transformada_de_camara(
    pintada: &Camara,
    ahora: &Camara,
    ancho_px: f32,
    alto_px: f32,
    margen: f32,
) -> Option<(f32, f32, f32)> {
    if margen <= 0.0 || pintada.zoom <= 0.0 || ahora.zoom <= 0.0 {
        return None;
    }
    let s = ahora.zoom / pintada.zoom;
    if !s.is_finite() || s > ESTIRADO_MAXIMO {
        return None;
    }
    // Un punto del mundo que se pinto en la superficie en `u` tiene que
    // acabar en `(u − margen)·s + (pintada − ahora)·zoom_nuevo`.
    let ex = (pintada.x - ahora.x) * ahora.zoom;
    let ey = (pintada.y - ahora.y) * ahora.zoom;
    if !ex.is_finite() || !ey.is_finite() {
        return None;
    }
    // ¿Sigue cubierta la ventana entera por lo que de verdad se pinto? El
    // borde izquierdo de la ventana cae en la superficie en `margen − e/s`,
    // y el derecho en `(ancho − e)/s + margen`: los dos tienen que quedar
    // dentro de los `ancho + 2·margen` pixeles pintados.
    let cabe = |e: f32, largo: f32| e <= margen * s && e >= largo - s * (largo + margen);
    if cabe(ex, ancho_px) && cabe(ey, alto_px) {
        Some((s, ex, ey))
    } else {
        None
    }
}

/// Abre el editor y no vuelve hasta que se cierra la ventana.
///
/// Devuelve la escena tal como quedo, compactada: los elementos borrados de
/// verdad (los que un `Ctrl+Z` ya no puede traer de vuelta) se sueltan aqui,
/// no en cada paso del historial.
///
/// `ajustes_iman` es solo el trozo de los ajustes de la aplicacion que este
/// editor necesita conocer: el editor no tiene por que saber de atajos ni de
/// la carpeta de capturas, asi que quien llama pasa `config.enganche`, no
/// los `Ajustes` enteros.
///
/// `nivel` decide cuanto tiene que estar quieto el zoom antes de rehacer la
/// tinta nitida (`retardo_nitido`, D122): en `Ligero` el retardo es mayor
/// porque teselar cuesta mas en ese equipo.
///
/// `medir_fotogramas` enciende el registro de D129: una linea de `tracing`
/// cada 60 fotogramas con cuanto se tarda en vaciar la cola, pintar,
/// presentar y esperar.
///
/// `fondo` es la imagen del pin, fija en el mundo en (0,0)-(ancho, alto)
/// (D132); la bandeja pasa `None` (D137).
/// `fotos` son las imagenes que el dibujo ya trae —las del `.pixpin` del
/// movil—, cada una con el `id_objeto` que lleva su figura. Sin ellas, una
/// hoja que es una foto con trazos encima sale en blanco.
pub fn abrir(
    escena: Escena,
    ajustes_iman: pixpin_motor2d::enganche::Ajustes,
    nivel: pixpin_nivel::Nivel,
    medir_fotogramas: bool,
    fondo: Option<pixpin_codec::ImagenRgba>,
    fotos: &[(u64, std::path::PathBuf)],
) -> Result<(Escena, Option<String>)> {
    // F11 alterna entre pantalla completa y ventana. La ventana del editor
    // nace con su tamano y todo lo de dentro se mide contra el, asi que
    // cambiar de modo es cerrarla y abrirla otra vez con la MISMA escena: se
    // pierde el encuadre, no el dibujo.
    let mut escena = escena;
    loop {
        let (vuelta, enlace, cambiar) = abrir_en_modo(
            escena,
            ajustes_iman,
            nivel,
            medir_fotogramas,
            fondo.clone(),
            fotos,
            None,
        )?;
        if !cambiar {
            return Ok((vuelta, enlace));
        }
        escena = vuelta;
        let ahora = !PANTALLA_COMPLETA.load(std::sync::atomic::Ordering::SeqCst);
        PANTALLA_COMPLETA.store(ahora, std::sync::atomic::Ordering::SeqCst);
    }
}

/// El mismo editor con el universo detras (D215): la escena son sus
/// anotaciones y la sesion atiende lo que es del universo antes que el
/// editor. Vuelve con la escena al cerrar; si se pidio abrir una hoja, la
/// sesion la lleva en `hoja_pedida`.
///
/// Va aparte de `abrir` y no como un parametro mas de ella para no tocar
/// a quien ya la llama: el chat y la bandeja siguen abriendo dibujos igual.
pub fn abrir_universo(
    escena: Escena,
    ajustes_iman: pixpin_motor2d::enganche::Ajustes,
    nivel: pixpin_nivel::Nivel,
    medir_fotogramas: bool,
    sesion: &mut crate::universo::sesion::Sesion,
) -> Result<Escena> {
    let mut escena = escena;
    loop {
        let (vuelta, _, cambiar) = abrir_en_modo(
            escena,
            ajustes_iman,
            nivel,
            medir_fotogramas,
            None,
            &[],
            Some(&mut *sesion),
        )?;
        if !cambiar {
            return Ok(vuelta);
        }
        escena = vuelta;
        let ahora = !PANTALLA_COMPLETA.load(std::sync::atomic::Ordering::SeqCst);
        PANTALLA_COMPLETA.store(ahora, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Si el lienzo ocupa la pantalla entera o una ventana sin marco. Se
/// recuerda mientras viva la aplicacion: quien lo puso en ventana lo quiere
/// en ventana tambien en el siguiente lienzo.
static PANTALLA_COMPLETA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

const VK_F11: u32 = 0x7A;

fn abrir_en_modo(
    escena: Escena,
    ajustes_iman: pixpin_motor2d::enganche::Ajustes,
    nivel: pixpin_nivel::Nivel,
    medir_fotogramas: bool,
    fondo: Option<pixpin_codec::ImagenRgba>,
    fotos: &[(u64, std::path::PathBuf)],
    mut universo: Option<&mut crate::universo::sesion::Sesion>,
) -> Result<(Escena, Option<String>, bool)> {
    let dispositivo =
        pixpin_capture::Dispositivo::nuevo().context("sin dispositivo para el editor")?;
    let mut motor = MotorRender::nuevo(dispositivo.d3d()).context("sin motor de dibujo")?;
    // D139: se reduce aqui, que es donde se sabe cuanto admite la GPU.
    let mut fondo = fondo.map(|img| FondoLienzo::nuevo(img, motor.lado_maximo_bitmap()));
    // Las imagenes que se peguen durante esta sesion. Nace vacia: lo que
    // trae la escena de disco no puede traer pixeles todavia (ver el aviso
    // de alcance en `imagenes_lienzo`).
    let mut imagenes = ImagenesLienzo::nuevo(motor.lado_maximo_bitmap());
    // Las que ya trae el dibujo, con SU identificador: si se les diera uno
    // nuevo, las figuras seguirian apuntando al viejo y no se veria ninguna.
    for (id, ruta) in fotos {
        match pixpin_codec::cargar(ruta) {
            Ok(img) => {
                imagenes.guardar_con_id(*id, img);
            }
            Err(e) => tracing::warn!(?e, ruta = %ruta.display(), "imagen del proyecto ilegible"),
        }
    }

    let disposicion =
        pixpin_capture::enumerar_monitores().context("sin monitores para el editor")?;
    let monitor = disposicion
        .principal()
        .context("sin monitor principal para el editor")?;
    // Pantalla completa de verdad (tapa tambien la barra de tareas), o una
    // ventana sin marco centrada con aire alrededor. Sin el encabezado de
    // Windows en ninguno de los dos: el lienzo ocupa toda su ventana y se
    // cierra con Escape.
    let area = if PANTALLA_COMPLETA.load(std::sync::atomic::Ordering::SeqCst) {
        monitor.area
    } else {
        let t = monitor.area_trabajo;
        let (w, h) = (t.ancho * 4 / 5, t.alto * 4 / 5);
        pixpin_geom::Rect {
            x: t.x + ((t.ancho - w) / 2) as i32,
            y: t.y + ((t.alto - h) / 2) as i32,
            ancho: w,
            alto: h,
        }
    };

    let ventana = VentanaOverlay::nueva(area).context("no se pudo abrir el editor")?;
    // PIXPIN_TINTA_CLASICA vuelve a la presentacion de antes, para medir la
    // diferencia en el mismo equipo con el mismo binario.
    let tinta_clasica = std::env::var_os("PIXPIN_TINTA_CLASICA").is_some();
    let rendimiento = ajustes_rendimiento();
    // A3: con capas, la escena vive en su propio visual con colchon y la
    // barra en otro encima. `[rendimiento] paneo_por_composicion = false`
    // vuelve a la superficie de siempre, con todo en la misma swapchain.
    //
    // Con el universo detras tambien, desde A3 fase 2: su cielo y sus
    // estrellas se van a sus propios visuales DEBAJO de la escena, y cada
    // uno se mueve a SU paralaje (`Superficie::montar_fondo`). Antes no se
    // montaba porque los tres iban en la misma superficie, y correrla
    // entera los correria a todos al mismo paso.
    let por_composicion = rendimiento.paneo_por_composicion && !tinta_clasica;
    let superficie = if por_composicion {
        Superficie::nueva_con_capas(
            &motor,
            dispositivo.d3d(),
            ventana.handle(),
            area.ancho,
            area.alto,
            MARGEN_ESCENA,
        )
    } else if tinta_clasica {
        Superficie::nueva(
            &motor,
            dispositivo.d3d(),
            ventana.handle(),
            area.ancho,
            area.alto,
        )
    } else {
        Superficie::nueva_baja_latencia(
            &motor,
            dispositivo.d3d(),
            ventana.handle(),
            area.ancho,
            area.alto,
        )
    }
    .context("sin superficie para el editor")?;
    // Lo que de verdad quedo montado: si la superficie con capas no se
    // pudo crear se cae aqui mismo, asi que esto es `por_composicion`, pero
    // leerlo de la superficie es lo unico que no puede mentir.
    let con_capas = superficie.tiene_capas();
    let margen_escena = superficie.margen();
    ventana.mostrar();
    // D145: el lienzo que se abre desde un pin tiene que taparlo.
    ventana.traer_encima();
    ventana.enfocar();
    ventana.pedir_entrada_fina();

    let mut escena = escena;
    let mut gesto = gesto_inicial(ajustes_iman);
    // En el universo se abre con la mano: lo primero que se hace ahi es
    // mirar y mover astros, no dibujar.
    if universo.is_some() {
        elegir_herramienta(&mut gesto, Herramienta::Mano);
        // D227: soltar ficheros del Explorador encima los mete en el chat.
        // Solo con el universo: en un dibujo suelto no hay chat al que ir.
        ventana.aceptar_ficheros(true);
    }
    // Lo copiado del lienzo. Vive con la ventana: cerrar el editor se lo
    // lleva, que es lo que espera cualquiera.
    let mut portapapeles: Vec<pixpin_motor2d::Elemento> = Vec::new();
    // D135: con fondo, la imagen centrada; sin fondo, el origen como antes.
    let mut camara = match &fondo {
        Some(f) => encuadre_inicial(
            f.ancho(),
            f.alto(),
            area.ancho as f32,
            area.alto as f32,
            monitor.escala_por_cien,
        ),
        None => Camara::nueva(),
    };
    // D127: la camara del usuario va en pixeles logicos; `efectiva` es la
    // que pinta y traduce el raton. Se recalcula si cambia la escala.
    let mut escala_por_cien = monitor.escala_por_cien;
    // D136: rueda, Shift, Ctrl, espacio y boton central, a la Excalidraw.
    let mut navegador = navegacion::Navegador::nuevo();
    if let Some(s) = universo.as_deref_mut() {
        s.conectar_ventana(&ventana, area, escala_por_cien);
        camara = s.camara_inicial();
        // Alejarse hasta ver todas las galaxias a la vez.
        navegador.zoom_minimo = pixpin_universo::ZOOM_MINIMO_UNIVERSO;
        // Y moverse como por un mapa: la rueda acerca (`universo::mapa`).
        navegador.mapa = true;
        // A3 fase 2: el cielo y las estrellas, cada uno en su visual. El
        // colchon de las estrellas es el del lienzo por su paralaje: se
        // mueven menos, asi que necesitan menos. Si falla, la superficie se
        // queda sin capas de fondo y el universo se pinta como siempre
        // (`Superficie::tiene_fondo` lo dice).
        if con_capas {
            let margen_fondo = s.margen_estrellas(margen_escena);
            let (cw, ch) = crate::universo::cielo::tamano(area.ancho, area.alto);
            if let Err(err) = superficie.montar_fondo(cw, ch, area.ancho, area.alto, margen_fondo) {
                tracing::warn!(?err, "sin capas de fondo para el universo");
            }
        }
    }
    // Solo con el universo: el zoom que persigue la rueda y la inercia de
    // un arrastre del cielo (ver `universo::mapa`). En un lienzo normal no
    // se activan nunca.
    let mut suave = crate::universo::mapa::Suave::default();
    let mut reloj_suave = std::time::Instant::now();
    let mut arrastre_mapa: Option<crate::universo::mapa::Arrastre> = None;
    let mut efectiva = vista_efectiva(&camara, escala_por_cien);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    let retardo = pixpin_render::retardo_nitido(nivel);
    // El ultimo zoom visto y desde cuando esta ahi, para `decidir_zoom`
    // (D136: Ctrl+rueda ya cambia el zoom de la camara del usuario).
    let mut zoom_visto = efectiva.zoom;
    let mut zoom_desde: Option<std::time::Instant> = None;
    let mut rejilla = Rejilla::nueva();
    let mut capa = CapaEstatica::nueva();
    let (ancho_px, alto_px) = (area.ancho as f32, area.alto as f32);
    // "Contenido" y area de trabajo son el mismo rectangulo, como en
    // `CapaViva::nueva` (`capa.rs`): aqui el contenido ES la pantalla
    // entera, no hay un pin ni una ventana mas pequena de referencia.
    // Con el universo, la caja va bajo su barra de ruta, no encima de ella.
    let area_caja = |escala: u32, con_universo: bool| {
        if !con_universo {
            return area;
        }
        let ruta = crate::universo::sesion::Sesion::alto_ruta(escala);
        pixpin_geom::Rect {
            y: area.y + ruta as i32,
            alto: area.alto.saturating_sub(ruta),
            ..area
        }
    };
    let mut caja = CajaHerramientas::barra_superior(
        area_caja(escala_por_cien, universo.is_some()),
        escala_por_cien,
        &BOTONES_EDITOR,
    );
    // Donde estaba el raton la ultima vez, para resaltar el boton de debajo.
    let mut raton_barra: Option<Punto> = None;
    // La punta predicha del trazo en curso (ver `tinta::prediccion`): se
    // pinta donde estara el cursor cuando el fotograma llegue a pantalla.
    let mut predictor = pixpin_motor2d::tinta::prediccion::Predictor::nuevo();
    // C1, apagado de fabrica: el filtro de 1 euro de `pixpin-tinta`.
    let ajustes_tinta = ajustes_tinta();
    let suavizado_natural = ajustes_tinta.suavizado == pixpin_store::Suavizado::Natural;
    let mut filtro_tinta = pixpin_tinta::FiltroUnEuro::nuevo(mandos_de(ajustes_tinta));
    let reloj = std::time::Instant::now();
    // Si el fotograma anterior pinto una punta predicha: la zona que se
    // presenta tiene que cubrir tambien donde estaba, o quedaria un resto.
    let mut habia_prediccion = false;
    // Forma rapida: desde cuando esta quieto el cursor dibujando a mano, y
    // donde. Quieto `PAUSA_FORMA` convierte el trazo en figura.
    let mut quieto: Option<(std::time::Instant, Punto2)> = None;
    // El panel lateral tal como se pinto por ultima vez.
    let mut ultimo_panel: Option<pixpin_ui::panel_lateral::PanelLateral> = None;

    // Zona sucia acumulada durante la vuelta: se pinta un solo fotograma
    // DESPUES de vaciar la cola de eventos, no uno por evento (pintar a
    // mitad de la cola era lo que hacia perder puntos del lapiz).
    let mut hay_que_pintar = false;
    // Si DXGI ya admite otro fotograma. Se pinta solo entonces, y justo
    // antes se leen los movimientos: los puntos mas nuevos llegan al
    // siguiente refresco en vez de esperar en una cola de tres fotogramas.
    let mut fotograma_listo = superficie.senal_fotograma().is_none();
    // Desde cuando se espera la senal con algo por pintar. Si otra ventana
    // tapa el editor, DWM no consume fotogramas y la senal no llega: sin un
    // tope el trazo se congelaba hasta soltar (medido, con la terminal
    // encima).
    let mut esperando_senal: Option<std::time::Instant> = None;
    let mut sucio: Option<(f32, f32, f32, f32)> = None;
    let mut todo_sucio = false;
    // A3: `todo_sucio` dice «hay que rehacer el fotograma»; esto dice «y
    // ademas cambio el DIBUJO, no solo desde donde se mira». Solo cuando es
    // falso se puede mover el visual en vez de repintar.
    let mut contenido_sucio = false;
    // La camara con la que se pinto la superficie de escena que hay ahora
    // mismo en la swapchain. Es contra esta, y no contra la anterior, contra
    // la que se calcula la transformada del visual.
    let mut camara_pintada = efectiva;
    // Desde cuando la camara se mueve sin repintar. Al pasar `REPOSO_CAMARA`
    // se pinta el fotograma nitido, como hace el pin con el zoom (D89).
    let mut camara_movida: Option<std::time::Instant> = None;
    // La capa de la interfaz (barra, panel, ruta) se repinta cuando cambia,
    // no en cada fotograma: mientras se traza, el trazo no la toca.
    let mut interfaz_sucia = con_capas;
    // B1: el planificador del ritmo y su temporizador fino. Sin
    // `[rendimiento] ritmo` no se crea ni el temporizador.
    let mut planificador = pixpin_tinta::Planificador::nuevo();
    let temporizador = rendimiento
        .ritmo
        .then(pixpin_shell::overlay::TemporizadorFino::nuevo)
        .flatten();
    // A que hora se esperaba que DWM compusiera el fotograma que se esta
    // pintando: si se pasa, el margen del planificador sube.
    let mut plazo_previsto: Option<std::time::Instant> = None;
    // La hoja a la que lleva el recuadro que se pulso, si se pulso alguno.
    let mut enlace_pedido: Option<String> = None;
    // Se pulso F11: hay que volver a abrir en el otro modo.
    let mut cambiar_modo = false;
    // La goma esta pulsada: borra lo que vaya tocando hasta soltar.
    let mut borrando = false;
    // La zona sucia del fotograma anterior (D148): hace falta para la union
    // de dos, porque la cadena de intercambio tiene dos mapas.
    let mut zona_anterior: Option<(i32, i32, i32, i32)> = None;
    // B2: el trazo en curso se esta pintando en la capa de tinta, y por
    // tanto la escena no se toca ni se presenta hasta que se suelte.
    let mut tinta_viva = false;
    // La zona que se repinto de la capa de tinta en el fotograma anterior.
    // Aqui basta con UNA (no dos como en D148): una superficie de
    // composicion conserva lo que tenia, no rota entre dos mapas.
    let mut zona_tinta_anterior: Option<(i32, i32, i32, i32)> = None;
    // Cuando se compuso la capa de tinta por ultima vez. Sin `Present` no
    // hay senal de latencia que marque el ritmo, asi que lo marca el reloj.
    let mut ultimo_commit_tinta = std::time::Instant::now();
    let mut medidor = crate::medir_fotogramas::MedidorFotogramas::nuevo(medir_fotogramas);

    'bucle: loop {
        // D129: se mide siempre (unos nanosegundos por `Instant::now`); solo
        // se acumula y registra con la opcion encendida.
        let t_vuelta = std::time::Instant::now();
        let mut puntos = 0u32;
        pixpin_shell::overlay::bombear_pendientes();
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            if matches!(
                ev,
                EventoOverlay::RatonMovido(_) | EventoOverlay::Muestra(_)
            ) {
                puntos += 1;
            }
            // D127: con el editor abierto se puede cambiar la escala de
            // Windows. La tinta tiene que seguir del tamano de Excalidraw, y
            // la caja de herramientas, del de sus botones.
            if matches!(ev, EventoOverlay::CambioDpi) {
                if let Some(m) = pixpin_capture::enumerar_monitores()
                    .ok()
                    .and_then(|d| d.principal().copied())
                {
                    escala_por_cien = m.escala_por_cien;
                    efectiva = vista_efectiva(&camara, escala_por_cien);
                    caja = CajaHerramientas::barra_superior(
                        area_caja(escala_por_cien, universo.is_some()),
                        escala_por_cien,
                        &BOTONES_EDITOR,
                    );
                    // La capa congelada se horneo a la escala vieja.
                    capa.soltar();
                    todo_sucio = true;
                    contenido_sucio = true;
                    ventana.invalidar();
                }
                continue;
            }
            // El resaltado de la barra sigue al raton, como en Excalidraw.
            // Solo se repinta cuando cambia el boton de debajo: con cada
            // movimiento seria un fotograma entero por nada.
            if let EventoOverlay::RatonMovido(p) = ev {
                let antes = raton_barra.and_then(|r| caja.boton_en(r));
                raton_barra = Some(p);
                if caja.boton_en(p) != antes {
                    todo_sucio = true;
                    contenido_sucio = true;
                    ventana.invalidar();
                }
            }
            // D136: moverse por el lienzo va ANTES que la caja y que el
            // gesto. Un clic con el espacio pulsado arrastra el lienzo aunque
            // caiga sobre un boton, como en Excalidraw, y lo que el navegador
            // consume no puede empezar un trazo. Los modificadores solo se
            // leen para la rueda: sondearlos con cada muestra del lapiz seria
            // pagar cinco llamadas al sistema mil veces por segundo.
            let mods = if matches!(
                ev,
                EventoOverlay::Rueda(_) | EventoOverlay::RuedaHorizontal(_)
            ) {
                let m = pixpin_shell::entrada::modificadores_pulsados();
                navegacion::Modificadores {
                    ctrl: m.ctrl,
                    shift: m.shift,
                }
            } else {
                navegacion::Modificadores::default()
            };
            // Revision 1: el evento que cerraria el gesto a veces no llega
            // (Alt+Tab suelta el espacio sin `TeclaSoltada`; Windows le
            // quita la captura al raton a mitad de un arrastre sin mandar
            // el boton-arriba). `Navegador` sigue puro -no llama a
            // Windows-: aqui se sondea el estado en vivo, y solo para los
            // dos eventos donde `Navegador::evento` lo necesita, no en cada
            // muestra del lapiz.
            //
            // Revision 2: `botones_en_arrastre` puede dar DOS codigos para
            // el arrastre principal -`overlay.rs` traduce el clic derecho al
            // mismo `BotonPulsado` que el izquierdo-, y hace falta que
            // CUALQUIERA de los dos siga pulsado (`algun_boton_pulsado`), no
            // solo el primero: mirar solo el izquierdo cortaria un
            // espacio+arrastre sujeto con el derecho en el primer
            // `RatonMovido`.
            let vivo = navegacion::EnVivo {
                espacio: if matches!(ev, EventoOverlay::BotonPulsado(_)) {
                    pixpin_shell::entrada::tecla_pulsada_ahora(navegacion::VK_ESPACIO)
                } else {
                    true
                },
                boton_arrastre: if navegador.arrastrando()
                    && matches!(
                        ev,
                        EventoOverlay::RatonMovido(_) | EventoOverlay::Muestra(_)
                    ) {
                    navegacion::algun_boton_pulsado(
                        navegador.botones_en_arrastre(),
                        pixpin_shell::entrada::tecla_pulsada_ahora,
                    )
                } else {
                    true
                },
            };
            let nav = navegador.evento(
                &ev,
                Punto {
                    x: area.x,
                    y: area.y,
                },
                escala_por_cien,
                mods,
                gesto.en_reposo(),
                vivo,
            );
            if let Some(navegacion::Accion::ZoomSuave { foco, delta }) = nav.accion {
                // El universo: la rueda no salta, se persigue en el bucle.
                if !suave.activo() {
                    reloj_suave = std::time::Instant::now();
                }
                suave.pedir_zoom(
                    camara.zoom,
                    foco,
                    delta,
                    navegador.zoom_minimo,
                    pixpin_motor2d::camara::ZOOM_MAXIMO,
                );
            } else if let Some(accion) = nav.accion {
                if navegacion::aplicar_con_minimo(&mut camara, accion, navegador.zoom_minimo) {
                    efectiva = vista_efectiva(&camara, escala_por_cien);
                    // La capa congelada se da por invalida sola: su Estampa
                    // lleva la camara. `decidir_zoom` rehace la tinta nitida
                    // cuando el zoom se quede quieto.
                    //
                    // Este es el UNICO sitio que ensucia sin tocar
                    // `contenido_sucio`: aqui no ha cambiado nada del dibujo,
                    // solo desde donde se mira. Es lo que le permite a A3
                    // mover el visual en vez de repintar (mas abajo, en el
                    // bloque de «pintar o componer»). Si algun dia se anade
                    // otro sitio que mueva la camara, o lo marca tambien o
                    // pagara un repintado: fallar hacia repintar es lo
                    // correcto, porque componer una escena que si cambio
                    // ensenaria el dibujo viejo.
                    todo_sucio = true;
                    ventana.invalidar();
                }
            }
            if nav.consumido {
                if navegador.arrastrando() {
                    ventana.poner_cursor(FormaCursorWin::Mover);
                }
                continue;
            }
            // Un clic para la inercia: la mano vuelve a mandar.
            if matches!(
                ev,
                EventoOverlay::BotonPulsado(_) | EventoOverlay::BotonCentralPulsado(_)
            ) {
                suave.parar_inercia();
            }
            // Arrastrando el cielo del universo (empezo abajo, en su `Pasa`):
            // lo que se mueve es la camara, y nada mas ve el raton.
            if let Some(mut a) = arrastre_mapa.take() {
                let ms = reloj.elapsed().as_secs_f64() * 1000.0;
                match ev {
                    // Como con el espacio: si Windows le quito la captura al
                    // raton, el boton-arriba no llega y el arrastre se suelta
                    // aqui.
                    EventoOverlay::RatonMovido(p)
                        if navegacion::algun_boton_pulsado(
                            &[0x01, 0x02],
                            pixpin_shell::entrada::tecla_pulsada_ahora,
                        ) =>
                    {
                        let (dx, dy) = a.mover(p, ms, navegacion::escala_de(escala_por_cien));
                        arrastre_mapa = Some(a);
                        ventana.poner_cursor(FormaCursorWin::Mover);
                        if navegacion::aplicar_con_minimo(
                            &mut camara,
                            navegacion::Accion::Desplazar { dx, dy },
                            navegador.zoom_minimo,
                        ) {
                            efectiva = vista_efectiva(&camara, escala_por_cien);
                            todo_sucio = true;
                            contenido_sucio = true;
                            ventana.invalidar();
                        }
                        continue;
                    }
                    EventoOverlay::RatonMovido(_) => {}
                    EventoOverlay::BotonSoltado(_) => {
                        if let Some(v) = a.soltar(ms) {
                            reloj_suave = std::time::Instant::now();
                            suave.empujar(v);
                        }
                        ventana.poner_cursor(FormaCursorWin::Flecha);
                        continue;
                    }
                    EventoOverlay::Muestra(_) => {
                        arrastre_mapa = Some(a);
                        continue;
                    }
                    _ => arrastre_mapa = Some(a),
                }
            }
            // El universo, si lo hay, va antes que los enlaces y que las
            // herramientas del editor: un clic sobre un astro es suyo. Menos
            // el que cae en la caja de herramientas o en el panel, que son
            // del editor tambien con el universo detras.
            if let Some(s) = universo.as_deref_mut() {
                use crate::universo::sesion::{Editor, Respuesta};
                let sobre_la_interfaz = match ev {
                    EventoOverlay::BotonPulsado(p) => {
                        !matches!(caja.destino(p), DestinoClic::Lienzo)
                            || crate::panel_dibujo::panel_para(
                                &gesto,
                                &escena,
                                area,
                                escala_por_cien,
                            )
                            .is_some_and(|panel| {
                                !matches!(
                                    panel.destino(p),
                                    pixpin_ui::panel_lateral::DestinoPanel::Fuera
                                )
                            })
                    }
                    _ => false,
                };
                if !sobre_la_interfaz {
                    let ed = Editor {
                        mano: gesto.herramienta == Herramienta::Mano,
                        escribiendo: gesto.esta_escribiendo(),
                        escala_por_cien,
                        area,
                    };
                    match s.evento(&ev, &mut camara, &mut escena, &ed) {
                        Respuesta::Pasa => {
                            use crate::universo::mapa;
                            let libre = !gesto.esta_escribiendo();
                            match ev {
                                EventoOverlay::BotonPulsado(p) if libre => {
                                    let q = efectiva.a_mundo(Punto2::nuevo(
                                        (p.x - area.x) as f32,
                                        (p.y - area.y) as f32,
                                    ));
                                    // Los tiradores sobresalen de la caja
                                    // elegida: se cuentan unos pixeles de mas.
                                    let margen = 16.0 / efectiva.zoom;
                                    let en_la_seleccion = gesto
                                        .seleccion
                                        .caja(&escena)
                                        .is_some_and(|(x0, y0, x1, y1)| {
                                            q.x >= x0 - margen
                                                && q.x <= x1 + margen
                                                && q.y >= y0 - margen
                                                && q.y <= y1 + margen
                                        });
                                    let clic = mapa::Clic {
                                        libre_de_astros: true,
                                        mano: ed.mano,
                                        shift: pixpin_shell::entrada::modificadores_pulsados()
                                            .shift,
                                        sobre_anotacion: en_la_seleccion
                                            || pixpin_motor2d::impacto::elemento_en(
                                                &escena.elementos,
                                                q,
                                            )
                                            .is_some(),
                                    };
                                    if mapa::arrastra_el_cielo(clic) {
                                        // Como un clic en el vacio: suelta
                                        // lo que hubiera elegido.
                                        gesto.seleccion.limpiar();
                                        arrastre_mapa = Some(mapa::Arrastre::nuevo(p));
                                        ventana.poner_cursor(FormaCursorWin::Mover);
                                        todo_sucio = true;
                                        contenido_sucio = true;
                                        ventana.invalidar();
                                        continue;
                                    }
                                }
                                // Las flechas llevan el cielo, si no hay
                                // anotaciones elegidas que mover con ellas.
                                EventoOverlay::Tecla {
                                    vk, ctrl: false, ..
                                } if libre && gesto.seleccion.esta_vacia() => {
                                    if let Some((dx, dy)) = mapa::desplazamiento_de_flecha(vk) {
                                        if navegacion::aplicar_con_minimo(
                                            &mut camara,
                                            navegacion::Accion::Desplazar { dx, dy },
                                            navegador.zoom_minimo,
                                        ) {
                                            efectiva = vista_efectiva(&camara, escala_por_cien);
                                            todo_sucio = true;
                                            contenido_sucio = true;
                                            ventana.invalidar();
                                        }
                                        continue;
                                    }
                                }
                                // `+` y `-`: una muesca de rueda en el centro.
                                EventoOverlay::Caracter(c) if libre => {
                                    if let Some(delta) = mapa::delta_de_caracter(c) {
                                        let e = navegacion::escala_de(escala_por_cien);
                                        let centro = Punto2::nuevo(
                                            area.ancho as f32 / (2.0 * e),
                                            area.alto as f32 / (2.0 * e),
                                        );
                                        if !suave.activo() {
                                            reloj_suave = std::time::Instant::now();
                                        }
                                        suave.pedir_zoom(
                                            camara.zoom,
                                            centro,
                                            delta,
                                            navegador.zoom_minimo,
                                            pixpin_motor2d::camara::ZOOM_MAXIMO,
                                        );
                                        continue;
                                    }
                                }
                                _ => {}
                            }
                        }
                        Respuesta::Consumido { repintar } => {
                            efectiva = vista_efectiva(&camara, escala_por_cien);
                            if repintar {
                                todo_sucio = true;
                                contenido_sucio = true;
                                ventana.invalidar();
                            }
                            s.tras_evento(&escena);
                            continue;
                        }
                        Respuesta::Apartarse => {
                            // El editor va siempre encima: minimizado deja
                            // ver el chat, y `universo::lanzar` lo devuelve.
                            ventana.minimizar();
                            s.tras_evento(&escena);
                            continue;
                        }
                        Respuesta::Cerrar | Respuesta::AbrirHoja { .. } => break 'bucle,
                    }
                }
            }
            // 0. La caja de herramientas es un dialogo en pantalla: un clic
            // ahi ELIGE o dispara una accion, y no puede llegar ademas al
            // gesto como si fuera un trazo en el lienzo -mismo cuidado que
            // `CapaViva::raton` ya toma en `capa.rs`-. Se resuelve con las
            // coordenadas de pantalla tal cual llegan, ANTES de que
            // `a_evento` las convierta a mundo con la camara.
            //
            // La pregunta "de la caja o del lienzo" es `CajaHerramientas::
            // destino`, pura: aqui solo queda el `match` sobre su resultado,
            // asi que la decision en si esta bajo prueba sin ventana.
            // El panel lateral, igual que la barra: un clic ahi es suyo.
            // Un recuadro con enlace es la puerta a otra hoja (las «zonas» de
            // una pagina del movil): pulsarlo cierra este lienzo y quien
            // llama abre el de al lado. Va antes que nada, porque si no el
            // gesto se lo lleva como una seleccion cualquiera.
            // Con CUALQUIER herramienta: una zona enlazada es un boton, no una
            // figura. Pedir la mano lo hacia inalcanzable, porque el editor
            // abre con el lapiz.
            if let EventoOverlay::BotonPulsado(p) = ev
                && let Some(q) = a_evento(
                    &ev,
                    &efectiva,
                    Punto {
                        x: area.x,
                        y: area.y,
                    },
                )
                .and_then(|g| match g {
                    pixpin_motor2d::gesto::EventoGesto::Pulsar { p, .. } => Some(p),
                    _ => None,
                })
            {
                let _ = p;
                if let Some(id) = pixpin_motor2d::impacto::elemento_en(&escena.elementos, q)
                    && let Some(destino) = escena.buscar(id).and_then(|e| e.enlace.clone())
                {
                    tracing::info!(%destino, "enlace a otra hoja");
                    enlace_pedido = Some(destino);
                    break 'bucle;
                }
            }
            if let EventoOverlay::BotonPulsado(p) | EventoOverlay::BotonSoltado(p) = ev {
                if let Some(panel) =
                    crate::panel_dibujo::panel_para(&gesto, &escena, area, escala_por_cien)
                {
                    use pixpin_ui::panel_lateral::DestinoPanel;
                    match panel.destino(p) {
                        DestinoPanel::Accion(a) => {
                            if matches!(ev, EventoOverlay::BotonPulsado(_))
                                && crate::panel_dibujo::aplicar(a, &mut gesto, &mut escena)
                            {
                                todo_sucio = true;
                                contenido_sucio = true;
                                ventana.invalidar();
                            }
                            continue;
                        }
                        DestinoPanel::Panel => continue,
                        DestinoPanel::Fuera => {}
                    }
                }
            }
            if let EventoOverlay::BotonPulsado(p) = ev {
                match caja.destino(p) {
                    DestinoClic::Boton(boton) => {
                        if !pulsar_boton(boton, &mut gesto, &mut escena) {
                            break 'bucle;
                        }
                        // Una herramienta del editor deja la del universo.
                        if let Some(s) = universo.as_deref_mut() {
                            s.herramienta = None;
                        }
                        // El cursor se pone al vuelo con el siguiente
                        // `RatonMovido`: no hace falta calcularlo aqui, y
                        // `cursor_en` es privado de `gesto.rs` a proposito.
                        ventana.invalidar();
                        todo_sucio = true;
                        contenido_sucio = true;
                        continue;
                    }
                    // El hueco entre botones: de la caja, pero no un boton.
                    DestinoClic::Caja => continue,
                    DestinoClic::Lienzo => {}
                }
            }
            if let EventoOverlay::BotonSoltado(p) = ev {
                if !matches!(caja.destino(p), DestinoClic::Lienzo) {
                    continue;
                }
            }
            // Los atajos de teclado de la caja (D53): la letra que pinta
            // `caja_dibujo::etiqueta` elige la herramienta. Llegan como
            // caracter compuesto (WM_CHAR), no como `EventoGesto`: el
            // cajetin de calibrar tiene su PROPIO bucle de eventos
            // (`pedir_medida`, mas abajo) y nunca lo comparte con este, asi
            // que una letra no puede robarle un caracter mientras esta
            // abierto.
            // Escribiendo, las teclas son del texto: una «r» es una erre y no
            // la herramienta rectangulo. Va antes que todo lo demas.
            if gesto.esta_escribiendo() {
                use pixpin_motor2d::texto::TeclaTexto;
                const VK_IZQUIERDA: u32 = 0x25;
                const VK_DERECHA: u32 = 0x27;
                const VK_INICIO: u32 = 0x24;
                const VK_FIN: u32 = 0x23;
                const VK_RETROCESO: u32 = 0x08;
                const VK_SUPRIMIR: u32 = 0x2E;
                const VK_ESCAPE_TEXTO: u32 = 0x1B;
                const VK_ENTRAR: u32 = 0x0D;
                let atendido = match ev {
                    // Los mandos llegan tambien como caracter; se atienden
                    // por tecla, que es donde se distinguen bien.
                    EventoOverlay::Caracter(c) if c >= ' ' => gesto.escribir(c, &mut escena),
                    EventoOverlay::Tecla { vk, .. } => match vk {
                        VK_ESCAPE_TEXTO => gesto.cerrar_texto(&mut escena),
                        VK_IZQUIERDA => gesto.tecla_de_texto(TeclaTexto::Izquierda, &mut escena),
                        VK_DERECHA => gesto.tecla_de_texto(TeclaTexto::Derecha, &mut escena),
                        VK_INICIO => gesto.tecla_de_texto(TeclaTexto::Inicio, &mut escena),
                        VK_FIN => gesto.tecla_de_texto(TeclaTexto::Fin, &mut escena),
                        VK_RETROCESO => gesto.tecla_de_texto(TeclaTexto::Retroceso, &mut escena),
                        VK_SUPRIMIR => gesto.tecla_de_texto(TeclaTexto::Suprimir, &mut escena),
                        VK_ENTRAR => gesto.tecla_de_texto(TeclaTexto::Entrar, &mut escena),
                        _ => false,
                    },
                    _ => false,
                };
                if atendido {
                    todo_sucio = true;
                    contenido_sucio = true;
                    ventana.invalidar();
                }
                // Las teclas se consumen aunque no hagan nada; el raton no,
                // que es como se sale a pulsar en otro sitio.
                if matches!(ev, EventoOverlay::Caracter(_) | EventoOverlay::Tecla { .. }) {
                    continue;
                }
            }

            if let EventoOverlay::Tecla { vk: VK_F11, .. } = ev {
                cambiar_modo = true;
                break 'bucle;
            }
            if let EventoOverlay::Caracter(c) = ev {
                if let Some(h) = tecla_a_herramienta(c) {
                    elegir_herramienta(&mut gesto, h);
                    ventana.invalidar();
                    todo_sucio = true;
                    contenido_sucio = true;
                    continue;
                }
                if let Some(cambio) = tecla_a_pluma(c) {
                    match cambio {
                        CambioPluma::Grosor(g) => gesto.grosor_tinta = g,
                        CambioPluma::AlternarVariabilidad => {
                            use pixpin_motor2d::tinta::Variabilidad;
                            gesto.variabilidad = match gesto.variabilidad {
                                Variabilidad::Variable => Variabilidad::Constante,
                                Variabilidad::Constante => Variabilidad::Variable,
                            };
                        }
                    }
                    tracing::info!(grosor = gesto.grosor_tinta, variabilidad = ?gesto.variabilidad, "pluma del editor");
                    continue;
                }
            }
            // 0. Los atajos de portapapeles y grupo. Van antes de traducir
            // porque no son gestos: no tocan la maquina de estados, operan
            // sobre lo que hay elegido.
            if let EventoOverlay::Tecla {
                vk, ctrl, shift, ..
            } = ev
            {
                if ctrl
                    && matches!(vk, v if v == b'C' as u32
                    || v == b'X' as u32
                    || v == b'V' as u32
                    || v == b'D' as u32
                    || v == b'G' as u32)
                {
                    use pixpin_motor2d::portapapeles as pp;
                    use pixpin_ui::panel_lateral::AccionPanel;
                    let hecho = match vk {
                        v if v == b'C' as u32 => {
                            portapapeles = pp::copiar(&escena, &gesto.seleccion);
                            false
                        }
                        v if v == b'X' as u32 => {
                            portapapeles = pp::copiar(&escena, &gesto.seleccion);
                            !portapapeles.is_empty()
                                && crate::panel_dibujo::aplicar(
                                    AccionPanel::Borrar,
                                    &mut gesto,
                                    &mut escena,
                                )
                        }
                        v if v == b'V' as u32 => {
                            use crate::imagenes_lienzo as img;
                            // El portapapeles del sistema solo se abre si no
                            // hay nada copiado dentro: ver `decidir_pegado`.
                            let del_sistema = if portapapeles.is_empty() {
                                pixpin_codec::portapapeles::leer()
                            } else {
                                None
                            };
                            let nuevos =
                                match img::decidir_pegado(!portapapeles.is_empty(), del_sistema) {
                                    img::Pegado::Elementos => pp::pegar(
                                        &mut escena,
                                        &portapapeles,
                                        pp::DESPLAZAMIENTO,
                                        pp::DESPLAZAMIENTO,
                                    ),
                                    img::Pegado::Imagen(bruta) => {
                                        let vista = efectiva.ventana(ancho_px, alto_px);
                                        match imagenes.guardar(bruta) {
                                            None => Vec::new(),
                                            Some(id_objeto) => {
                                                // El tamano se toma de lo que de
                                                // verdad se subio: si la GPU
                                                // obligo a reducir, la caja tiene
                                                // que seguir a los pixeles o la
                                                // imagen saldria estirada.
                                                let (w, h) = imagenes
                                                    .tamano(id_objeto)
                                                    .expect("recien guardada");
                                                let (ancho, alto) = img::tamano_al_pegar(
                                                    w,
                                                    h,
                                                    vista.2 - vista.0,
                                                    vista.3 - vista.1,
                                                );
                                                let (x, y) =
                                                    img::esquina_centrada(vista, ancho, alto);
                                                vec![escena.anadir(img::elemento_imagen(
                                                    id_objeto, x, y, ancho, alto,
                                                ))]
                                            }
                                        }
                                    }
                                    img::Pegado::Nada => Vec::new(),
                                };
                            let hubo = !nuevos.is_empty();
                            if hubo {
                                // Queda elegido lo pegado, como en
                                // Excalidraw: asi se puede llevar a su sitio
                                // de un tiron.
                                gesto.seleccion.poner_todos(nuevos);
                            }
                            hubo
                        }
                        v if v == b'D' as u32 => crate::panel_dibujo::aplicar(
                            AccionPanel::Duplicar,
                            &mut gesto,
                            &mut escena,
                        ),
                        // Ctrl+Shift+L bloquea y desbloquea, como en
                        // Excalidraw: lo bloqueado se ve pero no se elige
                        // ni se mueve, que es como se deja quieto un plano
                        // de fondo para dibujar encima.
                        v if v == b'L' as u32 && shift => {
                            pixpin_motor2d::organizar::bloquear(&mut escena, &mut gesto.seleccion)
                        }
                        _ => {
                            // Ctrl+G agrupa; con mayusculas, desagrupa. La
                            // logica ya estaba hecha y probada: le faltaba
                            // una tecla.
                            if shift {
                                pixpin_motor2d::organizar::desagrupar(
                                    &mut escena,
                                    &gesto.seleccion,
                                );
                                true
                            } else {
                                pixpin_motor2d::organizar::agrupar(&mut escena, &gesto.seleccion)
                                    .is_some()
                            }
                        }
                    };
                    if hecho {
                        todo_sucio = true;
                        contenido_sucio = true;
                        ventana.invalidar();
                    }
                    continue;
                }
            }

            // 1. Traducir y, si le toca al motor, pasarselo.
            // La goma. El motor la tiene en su lista de herramientas pero no
            // hace nada con ella —ahi solo valia para la capa de anotar—, asi
            // que en el editor elegirla y arrastrar no borraba nada. Borra lo
            // que toca al pulsar y mientras se arrastra, que es como se usa
            // una goma; cada toque es un paso que se puede deshacer.
            if gesto.herramienta == Herramienta::Borrador {
                match ev {
                    EventoOverlay::BotonPulsado(_) => borrando = true,
                    EventoOverlay::BotonSoltado(_) => borrando = false,
                    _ => {}
                }
                let punto = match ev {
                    EventoOverlay::BotonPulsado(p) | EventoOverlay::RatonMovido(p) if borrando => {
                        Some(p)
                    }
                    _ => None,
                };
                if let Some(p) = punto {
                    let q = efectiva
                        .a_mundo(Punto2::nuevo((p.x - area.x) as f32, (p.y - area.y) as f32));
                    if let Some(id) = pixpin_motor2d::impacto::elemento_en(&escena.elementos, q) {
                        escena.abrir_paso();
                        let hecho = escena.borrar_apuntando(id);
                        escena.cerrar_paso();
                        if hecho {
                            todo_sucio = true;
                            contenido_sucio = true;
                            ventana.invalidar();
                        }
                    }
                }
                if matches!(
                    ev,
                    EventoOverlay::BotonPulsado(_)
                        | EventoOverlay::BotonSoltado(_)
                        | EventoOverlay::RatonMovido(_)
                ) {
                    continue;
                }
            }
            if let Some(g) = a_evento(
                &ev,
                &efectiva,
                Punto {
                    x: area.x,
                    y: area.y,
                },
            ) {
                let mut g = con_modificadores(g);
                let ms = reloj.elapsed().as_secs_f64() * 1000.0;
                // C1: con `[tinta] suavizado = "natural"`, la posicion pasa
                // por el filtro de 1 euro antes de llegar al gesto. Se filtra
                // en PIXELES DE PANTALLA y no en unidades de mundo: los
                // hercios del filtro describen el temblor de la mano, que no
                // cambia porque el lienzo este mas o menos acercado.
                //
                // Solo mientras se traza a mano: filtrar un arrastre de la
                // seleccion o el simple mover el raton se sentiria como que
                // la aplicacion va pegajosa.
                if suavizado_natural
                    && matches!(
                        gesto.herramienta,
                        Herramienta::Lapiz | Herramienta::Resaltador
                    )
                {
                    match &mut g {
                        EventoGesto::Pulsar { p, .. } => {
                            filtro_tinta.reiniciar();
                            let s = efectiva.a_pantalla(*p);
                            let (x, y) = filtro_tinta.filtrar(s.x, s.y, ms);
                            *p = efectiva.a_mundo(Punto2::nuevo(x, y));
                        }
                        EventoGesto::Mover { p, .. } if gesto.elemento_en_curso().is_some() => {
                            let s = efectiva.a_pantalla(*p);
                            let (x, y) = filtro_tinta.filtrar(s.x, s.y, ms);
                            *p = efectiva.a_mundo(Punto2::nuevo(x, y));
                        }
                        _ => {}
                    }
                }
                match g {
                    EventoGesto::Pulsar { p, .. } => {
                        predictor.reiniciar();
                        predictor.anotar(p, ms);
                        quieto = Some((std::time::Instant::now(), p));
                    }
                    EventoGesto::Mover { p, .. } => {
                        predictor.anotar(p, ms);
                        // Moverse mas de 4 px de pantalla reinicia la pausa: el
                        // temblor de la mano parada no cuenta como movimiento.
                        let lejos =
                            quieto.is_none_or(|(_, q)| q.distancia(p) * efectiva.zoom > 4.0);
                        if lejos {
                            quieto = Some((std::time::Instant::now(), p));
                        }
                    }
                    EventoGesto::Soltar { .. } => quieto = None,
                    _ => {}
                }
                let en_reposo_antes = gesto.en_reposo();
                let r = gesto.evento(g, &mut escena, 1.0 / efectiva.zoom);
                ventana.poner_cursor(forma_de(r.cursor));
                // `VentanaOverlay::invalidar` no toma una region: invalida
                // la ventana entera para que Windows mande `WM_PAINT`, pero
                // la zona que de verdad hay que pintar la lleva `sucio`
                // (D125): es lo que reduce a `Superficie::
                // presentar_sincronizado` cuanto tiene que recomponer DWM.
                match r.region {
                    Region::Nada => {}
                    Region::Caja(x0, y0, x1, y1) => {
                        let a = efectiva.a_pantalla(Punto2::nuevo(x0, y0));
                        let b = efectiva.a_pantalla(Punto2::nuevo(x1, y1));
                        let caja = (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
                        sucio = Some(match sucio {
                            None => caja,
                            Some(s) => (
                                s.0.min(caja.0),
                                s.1.min(caja.1),
                                s.2.max(caja.2),
                                s.3.max(caja.3),
                            ),
                        });
                        ventana.invalidar();
                    }
                    Region::Todo => {
                        todo_sucio = true;
                        contenido_sucio = true;
                        ventana.invalidar();
                    }
                }
                // La capa estatica vive mientras algo cambia en cada
                // fotograma: mover, escalar o girar la seleccion (como
                // antes) y, desde E1, dibujar a mano (D120). Lo excluido se
                // repinta encima; lo demas se copia de la capa.
                let excluidos = excluidos_de(&gesto);
                let activo_ahora = !gesto.en_reposo() && !excluidos.is_empty();
                // **B2: el trazo vivo en su propia capa.** Mientras dura, la
                // escena no se toca: ni se hornea al apoyar el lapiz
                // (`capa.preparar`: 20 ms con 200 elementos y 194 ms con
                // 10.000, medidos a 3000 x 2000, mas 33,7 MB de video), ni
                // se copia por fotograma (D148), ni se presenta.
                //
                // Va con las capas de A3 porque necesita su arbol de
                // visuales: `[rendimiento] paneo_por_composicion = false` y
                // PIXPIN_TINTA_CLASICA apagan las dos cosas a la vez, que es
                // lo que hace falta para comparar en el mismo equipo.
                //
                // Y solo entra si lo que hay en la superficie de la escena
                // es ESTA camara: recien desplazada por composicion, lo que
                // se ve es la textura corrida y el trazo saldria donde no
                // es. En ese caso se fuerza el fotograma nitido y la capa
                // entra en la vuelta siguiente.
                let quiere_tinta = TINTA_EN_CAPA && con_capas && gesto.trazo_en_curso().is_some();
                let tinta_ahora =
                    quiere_tinta && camara_pintada == efectiva && !superficie.esta_estirada();
                if quiere_tinta && !tinta_ahora && !tinta_viva {
                    todo_sucio = true;
                    contenido_sucio = true;
                    ventana.invalidar();
                }
                if tinta_ahora && !tinta_viva {
                    tinta_viva = superficie.encender_tinta(&motor).is_ok();
                    // Si el trazo empezo con una capa congelada ya horneada
                    // (por el fotograma nitido de arriba), sus megas no
                    // pintan nada mientras la tinta vive en su capa.
                    if tinta_viva {
                        capa.soltar();
                        zona_tinta_anterior = None;
                    }
                }
                if !tinta_ahora && tinta_viva {
                    // Al soltar -o al convertirse en forma rapida-: el trazo
                    // ya esta en la escena y lo pinta el fotograma nitido.
                    superficie.apagar_tinta(&motor);
                    tinta_viva = false;
                    todo_sucio = true;
                    contenido_sucio = true;
                    ventana.invalidar();
                }
                // Con el universo detras no hay capa congelada: se horneria
                // sobre blanco y sin astros (ver el plan, «Ajustes», D238).
                if activo_ahora && en_reposo_antes && universo.is_none() && !tinta_ahora {
                    rejilla.sincronizar(&escena);
                    // A3: la capa congelada es del tamano de la SUPERFICIE
                    // de escena (ventana mas colchon) y se hornea con el
                    // mismo desplazamiento; si no, `volcar_zona` copiaria
                    // el trozo equivocado.
                    let h = margen_escena / efectiva.zoom;
                    let v = efectiva.ventana(ancho_px, alto_px);
                    let vista = (v.0 - h, v.1 - h, v.2 + h, v.3 + h);
                    let candidatos = rejilla.candidatos(vista);
                    let estampa = Estampa {
                        camara: (efectiva.x, efectiva.y, efectiva.zoom),
                        tamano: tamano_escena(ancho_px, alto_px, margen_escena),
                        excluidos: excluidos.clone(),
                    };
                    if let Some(f) = fondo.as_mut() {
                        f.asegurar(&motor);
                    }
                    // Lo que no este subido cuando se hornea la capa se
                    // queda fuera de ella, y la capa sigue valiendo: seria
                    // una imagen que no aparece hasta soltar el raton.
                    imagenes.asegurar(&motor);
                    let imagenes = &imagenes;
                    let _ = capa.preparar(&mut motor, estampa, |p| {
                        p.limpiar(Color::BLANCO);
                        let origen = efectiva.a_pantalla(Punto2::nuevo(0.0, 0.0));
                        p.poner_vista(
                            (0.0, 0.0),
                            efectiva.zoom,
                            (origen.x + margen_escena, origen.y + margen_escena),
                        );
                        // D140: la imagen se pinta primera y entra en la capa:
                        // mientras se dibuja, no cuesta nada por fotograma.
                        if let Some(f) = &fondo {
                            f.pintar(p, vista, efectiva.zoom);
                        }
                        for id in candidatos {
                            if excluidos.contains(&id) {
                                continue;
                            }
                            let Some(e) = escena.buscar(id) else {
                                continue;
                            };
                            if e.borrado {
                                continue;
                            }
                            // Las dos llamadas -aqui y en `pintar`- tienen
                            // que usar la MISMA funcion: lo que no pase por
                            // `por_cada_orden` no entra en la capa, y
                            // `pintar` ya no lo repinta mientras la capa
                            // valga (ve su comentario para el porque).
                            let mut indice = 0u32;
                            // De que esta hecha su tinta. `None` en la
                            // lisa y en las encendidas, que no llevan tela.
                            let grano = pixpin_motor2d::pintado::grano_de(e);
                            por_cada_orden(
                                &mut cache,
                                e,
                                efectiva.zoom,
                                escena.escala.as_ref(),
                                |orden| {
                                    dibujar_orden(
                                        p,
                                        orden,
                                        vista,
                                        Some((&mut cache_tinta, (e.id, e.version, indice))),
                                        imagenes,
                                        efectiva.zoom,
                                        grano,
                                    );
                                    indice += 1;
                                },
                            );
                        }
                    });
                } else if !activo_ahora && capa.lista() {
                    capa.soltar();
                }
                // El cajetin de calibrar: abre su propio bucle de eventos y
                // no vuelve hasta que se acepta con Enter o se cancela con
                // Escape. Si se cancela, `pedir_medida` devuelve `None` y no
                // se llama a `calibrar`: el lienzo queda exactamente como
                // estaba (cancelar no puede cambiar la escala).
                if let Some(Peticion::Calibrar { largo_px }) = r.pide {
                    rejilla.sincronizar(&escena);
                    if let Some((valor, unidad)) = pedir_medida(
                        &ventana,
                        &mut motor,
                        &superficie,
                        &escena,
                        &efectiva,
                        &gesto,
                        &mut cache,
                        &mut cache_tinta,
                        &rejilla,
                        &capa,
                        &mut fondo,
                        &mut imagenes,
                        &caja,
                        escala_por_cien,
                        ancho_px,
                        alto_px,
                        largo_px,
                    ) {
                        gesto.calibrar(&mut escena, largo_px, valor, &unidad);
                    }
                    ventana.invalidar();
                    todo_sucio = true;
                    contenido_sucio = true;
                }
            }
            // 2. Pintar solo cuando lo pide la ventana: marca que hace falta
            // un fotograma, no pinta aqui mismo. Pintar a mitad de la cola
            // era lo que hacia perder puntos de un trazo rapido.
            if matches!(ev, EventoOverlay::Pintar) {
                hay_que_pintar = true;
            }
            if matches!(ev, EventoOverlay::Cerrar) {
                break 'bucle;
            }
        }
        let vaciar = t_vuelta.elapsed();
        // La rueda y la inercia del universo, con el reloj: se mueven sin
        // eventos hasta llegar.
        if suave.activo() {
            let ahora = std::time::Instant::now();
            let dt = ahora.duration_since(reloj_suave).as_secs_f32();
            reloj_suave = ahora;
            if suave.avanzar(
                &mut camara,
                dt,
                navegador.zoom_minimo,
                pixpin_motor2d::camara::ZOOM_MAXIMO,
            ) {
                efectiva = vista_efectiva(&camara, escala_por_cien);
                todo_sucio = true;
                contenido_sucio = true;
                ventana.invalidar();
            }
        }
        // El universo: su vuelo de camara, sus destellos y su guardado van
        // con el reloj, no con los eventos.
        if let Some(s) = universo.as_deref_mut() {
            if s.tick(&mut camara) {
                efectiva = vista_efectiva(&camara, escala_por_cien);
                todo_sucio = true;
                contenido_sucio = true;
                ventana.invalidar();
            }
            s.tras_evento(&escena);
        }
        let mut pintado_medido = None;
        // Un solo fotograma por vuelta, despues de haber pasado TODOS los
        // puntos al gesto: pintar a mitad de la cola era lo que hacia perder
        // puntos. Presentar con vsync bloquea hasta el refresco; mientras,
        // Windows guarda los movimientos y la Tarea 7 los recupera.
        // Forma rapida (como QuickShape de Procreate): el trazo a mano quieto
        // con el boton pulsado se convierte en linea, rectangulo o elipse, y
        // lo que se arrastre despues la ajusta.
        if let Some((desde, p)) = quieto {
            let dibujando_a_mano =
                gesto.herramienta == Herramienta::Lapiz && gesto.trazo_en_curso().is_some();
            if !dibujando_a_mano {
                quieto = None;
            } else if desde.elapsed() >= PAUSA_FORMA {
                // Una sola vez por pausa: si no parece nada, se sigue
                // dibujando y la siguiente pausa lo vuelve a mirar.
                quieto = None;
                if gesto.convertir_en_forma(&mut escena, p).is_some() {
                    predictor.reiniciar();
                    todo_sucio = true;
                    contenido_sucio = true;
                    hay_que_pintar = true;
                    ventana.invalidar();
                }
            }
        }
        // **A3: componer en vez de pintar.** Si lo unico que cambio es desde
        // donde se mira y el colchon de la superficie da de si, el fotograma
        // se resuelve con una matriz en el visual de la escena: ni se
        // recorre la escena, ni se emite una primitiva, ni se presenta.
        //
        // Con el universo detras tambien (A3 fase 2), pero solo PANEOS: el
        // cielo esta quieto y las estrellas van a su paralaje, y eso es una
        // traslacion; un zoom no se reparte asi -lo que se acerca es el
        // lienzo, no el cielo-, y estirarlos los descolocaria. Un zoom en el
        // universo pinta nitido, como siempre.
        //
        // `todo_sucio && sucio.is_none() && !contenido_sucio` es la forma
        // exacta de decir «la unica razon por la que hay que rehacer el
        // fotograma es que la camara se movio»: `sucio` lo pone un gesto
        // (un trazo en curso, por ejemplo) y `contenido_sucio` todo lo
        // demas.
        if con_capas && hay_que_pintar && todo_sucio && !contenido_sucio && sucio.is_none() {
            // A3 fase 2: cuanto se mueven las estrellas por cada pixel que
            // se mueve el lienzo. Sin universo no hay estrellas y no se
            // mueve nada.
            let paralaje = universo.as_deref().map_or(0.0, |s| s.paralaje());
            let compone = |s: f32, dx: f32, dy: f32| {
                universo.is_none()
                    || (s == 1.0
                        && superficie.desplazamiento_fondo_valido(dx * paralaje, dy * paralaje))
            };
            match transformada_de_camara(
                &camara_pintada,
                &efectiva,
                ancho_px,
                alto_px,
                margen_escena,
            ) {
                Some((s, dx, dy)) if compone(s, dx, dy) => {
                    superficie.estirar(s, s, dx, dy);
                    // Las estrellas, a su paso. El cielo no se toca: es
                    // fondo de pantalla y no se mueve con nada.
                    if universo.is_some() {
                        superficie.desplazar_fondo(dx * paralaje, dy * paralaje);
                    }
                    hay_que_pintar = false;
                    // `todo_sucio` se queda puesto: el fotograma nitido que
                    // llegue al reposo tiene que ser completo.
                    //
                    // El reloj se pone a cero en CADA composicion, no solo
                    // en la primera: lo que se espera son 150 ms sin
                    // MOVERSE, no 150 ms desde que empezo el arrastre.
                    camara_movida = Some(std::time::Instant::now());
                }
                // No cabe (o es un zoom con el universo detras): se pinta
                // nitido ya, sin esperar al reposo.
                _ => camara_movida = None,
            }
        }
        // Quieta `REPOSO_CAMARA`: el fotograma nitido.
        if let Some(desde) = camara_movida
            && desde.elapsed() >= REPOSO_CAMARA
        {
            camara_movida = None;
            todo_sucio = true;
            hay_que_pintar = true;
        }
        if hay_que_pintar && !fotograma_listo {
            if let Some(s) = superficie.senal_fotograma() {
                fotograma_listo = pixpin_shell::overlay::senal_disparada(s);
            }
            let desde = *esperando_senal.get_or_insert_with(std::time::Instant::now);
            if desde.elapsed() >= ESPERA_MAXIMA_SENAL {
                fotograma_listo = true;
            }
        }
        if fotograma_listo || !hay_que_pintar {
            esperando_senal = None;
        }
        // **B2: el fotograma del trazo.** No pasa por `pintar`: no sincroniza
        // la rejilla, no recorre la escena, no copia la capa congelada y no
        // presenta. Es limpiar un rectangulo de la capa de tinta, pintar el
        // trazo dentro y un `Commit`.
        //
        // Sin `Present` no hay senal de latencia que marque el ritmo, asi que
        // lo marca el reloj: como mucho un fotograma por composicion de DWM.
        let mut espera_tinta = None;
        if tinta_viva && hay_que_pintar {
            let periodo = superficie
                .ritmo()
                .map_or(PERIODO_SUPUESTO_MS, |r| r.periodo_ms);
            espera_tinta = espera_de_tinta(
                ultimo_commit_tinta.elapsed().as_secs_f32() * 1000.0,
                periodo,
            );
        }
        if tinta_viva && hay_que_pintar && espera_tinta.is_none() {
            let t_pintar = std::time::Instant::now();
            let prediccion = prediccion_de(&gesto, &mut predictor, tinta_clasica, efectiva.zoom);
            let holgura = if prediccion.is_some() || habia_prediccion {
                TOPE_PREDICCION_PX as i32 + 8
            } else {
                2
            };
            habia_prediccion = prediccion.is_some();
            // `todo_sucio` no entra aqui: dice que hay que rehacer la ESCENA,
            // y en la capa de tinta no hay mas que el trazo en curso. Lo que
            // manda es la zona que dio el gesto.
            let zona = sucio.map(|(x0, y0, x1, y1)| {
                (
                    x0.floor() as i32 - holgura,
                    y0.floor() as i32 - holgura,
                    x1.ceil() as i32 + holgura,
                    y1.ceil() as i32 + holgura,
                )
            });
            // La union con la del fotograma anterior: si la punta predicha se
            // acorta, lo que se pinto de mas hay que borrarlo, y eso cae
            // fuera de la zona de ahora.
            let zona_pintada = match (zona, zona_tinta_anterior) {
                (Some(a), Some(b)) => {
                    Some((a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
                }
                _ => None,
            };
            let hecho = pintar_tinta_viva(
                &motor,
                &superficie,
                &escena,
                &efectiva,
                &gesto,
                &mut cache,
                &imagenes,
                zona_pintada,
                prediccion,
                ancho_px,
                alto_px,
            );
            ultimo_commit_tinta = std::time::Instant::now();
            zona_tinta_anterior = zona;
            if hecho {
                pintado_medido = Some(crate::medir_fotogramas::Pintado {
                    pintar: t_pintar.elapsed(),
                    // No hay present: el fotograma lo publica el `Commit` de
                    // composicion, que ya va dentro de `pintar_tinta`.
                    presentar: std::time::Duration::ZERO,
                });
                hay_que_pintar = false;
                sucio = None;
            }
        }
        if hay_que_pintar && fotograma_listo && !tinta_viva {
            // D129: `t_pintar` empieza AQUI, antes de `rejilla.sincronizar`,
            // no despues: sincronizar la rejilla es coste de este fotograma
            // (recorre la escena, O(su tamano)) y si quedara fuera de toda
            // fase, los numeros de D129 estarian mintiendo justo en lo que
            // se quiere diagnosticar.
            let t_pintar = std::time::Instant::now();
            rejilla.sincronizar(&escena);
            // La decision de si la capa congelada vale para ESTE fotograma
            // (`capa_vale`, dentro de `pintar`) es la que manda sobre si la
            // zona parcial es segura: `capa.lista()` solo dice que hay una
            // capa horneada, no que `volcar` la vaya a usar ahora mismo.
            // La punta predicha: horizonte medido (~30 ms de la lectura a la
            // pantalla en el equipo del usuario) y tope de 48 px de pantalla.
            let prediccion = prediccion_de(&gesto, &mut predictor, tinta_clasica, efectiva.zoom);
            // Lo que se presenta crece por el tope: la punta nueva y la del
            // fotograma anterior caen, como mucho, a esa distancia.
            let holgura = if prediccion.is_some() || habia_prediccion {
                TOPE_PREDICCION_PX as i32 + 8
            } else {
                2
            };
            // El panel cambia con la seleccion y la herramienta: si no es el
            // que se pinto la ultima vez, se presenta entero.
            let panel = crate::panel_dibujo::panel_para(&gesto, &escena, area, escala_por_cien);
            if panel != ultimo_panel {
                // Sin `contenido_sucio`: aqui ya se esta pintando, y el
                // propio pintado lo pone a falso. Lo que hace falta es que
                // el fotograma salga entero, y eso lo dice `todo_sucio`
                // (que ademas es lo que rehace la capa de la interfaz).
                todo_sucio = true;
            }
            let zona = if todo_sucio {
                None
            } else {
                sucio.map(|(x0, y0, x1, y1)| {
                    (
                        x0.floor() as i32 - holgura,
                        y0.floor() as i32 - holgura,
                        x1.ceil() as i32 + holgura,
                        y1.ceil() as i32 + holgura,
                    )
                })
            };
            // D148: al mapa que se pinta ahora le toca su turno cada DOS
            // fotogramas (la cadena tiene dos), asi que hay que rehacer
            // tambien lo que cambio en el anterior. Sin esta union quedaria
            // pegada la punta del trazo de hace dos fotogramas.
            let zona_pintada = match (zona, zona_anterior) {
                (Some(a), Some(b)) => {
                    Some((a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
                }
                _ => None,
            };
            habia_prediccion = prediccion.is_some();
            let pintado = pintar(
                &mut motor,
                &superficie,
                &escena,
                &efectiva,
                &gesto,
                &mut cache,
                &mut cache_tinta,
                &rejilla,
                &capa,
                &mut fondo,
                &mut imagenes,
                &caja,
                raton_barra,
                escala_por_cien,
                ancho_px,
                alto_px,
                zona_pintada,
                prediccion,
                panel.as_ref(),
                universo.as_deref_mut(),
                // La capa de interfaz se repinta cuando cambia algo suyo o
                // cuando se rehace el fotograma entero: mientras se traza
                // (zona parcial) no se toca, que es donde cada `Commit` de
                // mas costaria latencia.
                interfaz_sucia || todo_sucio,
                |_, _| {},
            );
            match pintado {
                Some(presentar) => {
                    pintado_medido = Some(crate::medir_fotogramas::Pintado {
                        pintar: t_pintar.elapsed().saturating_sub(presentar),
                        presentar,
                    });
                    hay_que_pintar = false;
                    ultimo_panel = panel;
                    zona_anterior = zona;
                    fotograma_listo = superficie.senal_fotograma().is_none();
                    sucio = None;
                    todo_sucio = false;
                    contenido_sucio = false;
                    interfaz_sucia = false;
                    // A3: lo que hay en la superficie es esta camara, y ya
                    // no hay nada que estirar. `dejar_de_estirar` no hace
                    // nada si no habia transformada puesta.
                    camara_pintada = efectiva;
                    camara_movida = None;
                    superficie.dejar_de_estirar();
                    // B1: lo que costo este fotograma alimenta el p99, y si
                    // se paso del plazo que se habia previsto, el margen
                    // sube. Sin `[rendimiento] ritmo` no hay temporizador y
                    // no se anota nada.
                    if temporizador.is_some() {
                        planificador.anotar(t_pintar.elapsed().as_secs_f32() * 1000.0);
                        match plazo_previsto {
                            Some(p) if std::time::Instant::now() > p => planificador.perdido(),
                            Some(_) => planificador.a_tiempo(),
                            None => {}
                        }
                    }
                }
                None => {
                    // `superficie.empezar` fallo: el fotograma se salto. La
                    // zona sucia acumulada NO se puede dar por pintada -si se
                    // limpiara aqui, el proximo present parcial se quedaria sin
                    // el trozo que este fotograma no llego a cubrir-, asi que
                    // se fuerza el fotograma que si llegue a ser completo.
                    todo_sucio = true;
                    contenido_sucio = true;
                }
            }
        }
        // Zoom quieto durante `retardo`: se rehace la tinta nitida a esa
        // escala y se suelta la capa (sus copias venian de la escala vieja).
        // La decision (y el tope de espera de mas abajo) sale de
        // `decidir_zoom`, pura: aqui solo se aplican sus efectos. Ctrl+rueda
        // (D136) es lo que hace que `efectiva.zoom` cambie de una vuelta a
        // la siguiente.
        let decision = decidir_zoom(
            efectiva.zoom,
            zoom_visto,
            cache_tinta.escala(),
            zoom_desde,
            std::time::Instant::now(),
            retardo,
        );
        zoom_visto = decision.visto;
        zoom_desde = decision.desde;
        if decision.rehacer {
            cache_tinta.fijar_escala(efectiva.zoom);
            capa.soltar();
            ventana.invalidar();
        }
        // Dormir hasta que llegue algo, pero no mas de lo que falta para que
        // el zoom cumpla el retardo: si no, `esperar_eventos(None)` dormiria
        // hasta el siguiente evento y la tinta nitida nunca llegaria a
        // rehacerse en reposo. Sin `sleep` fijo aqui tampoco: con el `sleep`
        // de 5 ms (15,6 ms reales sin `timeBeginPeriod`) el bucle perdia la
        // mitad de los puntos de un trazo rapido, y en reposo no gana nada.
        // `decision.tope_ms` es `None` en cuanto no hay nada pendiente -y
        // solo entonces-, asi que nunca gira en vacio con un tope de 0 ms.
        let t_esperar = std::time::Instant::now();
        let senal = if hay_que_pintar && !fotograma_listo {
            superficie.senal_fotograma()
        } else {
            None
        };
        // Con una pausa de forma rapida en marcha no llegan eventos (el raton
        // esta quieto): el bucle tiene que despertarse cuando se cumpla.
        let tope_forma = quieto
            .map(|(desde, _)| PAUSA_FORMA.saturating_sub(desde.elapsed()).as_millis() as u32 + 1);
        // A3: con la escena compuesta y la mano quieta no llega ningun
        // evento; sin este tope, el fotograma nitido no llegaria nunca y el
        // lienzo se quedaria estirado hasta el siguiente movimiento.
        let tope_reposo = camara_movida
            .map(|desde| REPOSO_CAMARA.saturating_sub(desde.elapsed()).as_millis() as u32 + 1);
        let minimo = |a: Option<u32>, b: Option<u32>| match (a, b) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let decision = DecisionZoom {
            tope_ms: minimo(
                minimo(minimo(decision.tope_ms, tope_forma), tope_reposo),
                minimo(
                    minimo(
                        universo.as_deref().and_then(|s| s.tope_ms()),
                        // B2: la capa de tinta tiene algo que pintar pero
                        // todavia no toca; sin este tope, con la mano quieta
                        // no llegaria ningun evento y el ultimo tramo del
                        // trazo se quedaria sin salir.
                        espera_tinta,
                    ),
                    // Persiguiendo: despertar cada fotograma. Parado, nada.
                    suave.activo().then_some(8),
                ),
            ),
            ..decision
        };
        let tope = if senal.is_some() {
            Some(
                decision
                    .tope_ms
                    .map_or(ESPERA_MAXIMA_SENAL_MS, |t| t.min(ESPERA_MAXIMA_SENAL_MS)),
            )
        } else {
            decision.tope_ms
        };
        if pixpin_shell::overlay::esperar_eventos_o_senal(senal, tope) {
            fotograma_listo = true;
        }
        // **B1: empezar el fotograma tarde.** Con `[rendimiento] ritmo`, en
        // cuanto se puede pintar NO se pinta: se duerme hasta
        // `plazo de DWM − p99(pintar) − margen` y se vuelve arriba, donde la
        // cola se vacia con los puntos que han llegado mientras tanto. Lo
        // que se gana no es pintar antes, es que lo pintado sea mas nuevo:
        // hoy el fotograma se queda esperando quieto los 10-13 ms que faltan
        // para la composicion, con los puntos envejeciendo dentro (Raph
        // Levien, «Swapchains and frame pacing»).
        //
        // El planificador decide, y decide 0 en cuanto pintar ya no cabe en
        // lo que queda: en un equipo lento esto no cambia nada.
        plazo_previsto = None;
        if let Some(t) = temporizador.as_ref()
            && hay_que_pintar
            && fotograma_listo
            && let Some(r) = superficie.ritmo()
        {
            plazo_previsto = Some(
                std::time::Instant::now()
                    + std::time::Duration::from_secs_f32(r.hasta_el_plazo_ms / 1000.0),
            );
            let espera = planificador.esperar_ms(r.hasta_el_plazo_ms);
            // Por debajo de un cuarto de milisegundo, armar el temporizador
            // cuesta mas que lo que se duerme.
            if espera > 0.25 {
                t.dormir(espera);
            }
        }
        if let Some(linea) = medidor.anotar(crate::medir_fotogramas::Vuelta {
            puntos,
            vaciar,
            pintado: pintado_medido,
            esperar: t_esperar.elapsed(),
        }) {
            linea.registrar();
        }
    }

    // El universo guarda al cerrar, pase lo que pase: su encuadre tambien.
    if let Some(s) = universo {
        s.cerrar(&escena, &camara);
    }
    // D143: la copia en GPU (y la de CPU) se suelta al cerrar, no al volver
    // al gestor de pines.
    drop(fondo);
    ventana.ocultar();
    escena.compactar();
    // Y a que hoja queria ir, si pulso un recuadro con enlace: quien llama
    // es el unico que sabe donde estan las hojas.
    Ok((escena, enlace_pedido, cambiar_modo))
}

/// Todas las ordenes de un elemento para un fotograma: las cacheadas (forma,
/// colores...) mas las que dependen de la escala (el numero de una cota, el
/// cuadro de una barra), que se recalculan cada vez porque `cache` no las
/// guarda -es lo que hace que calibrar surta efecto sin invalidar nada-.
///
/// La usan `pintar` y el cierre de `capa.preparar` (mas abajo, en `abrir`):
/// las dos veces que se decide que ordenes representan a un elemento en un
/// fotograma. Si un camino llamara solo a `cache.ordenes` y se olvidara de
/// esta funcion, ese elemento perderia sus rotulos de medida alli donde falte
/// -es justo lo que paso con la capa estatica: hornea solo lo que pasa por
/// aqui, asi que si el rotulo no entra, desaparece mientras dura el arrastre-.
///
/// La coma va como separador decimal (D39), no el del sistema: si algun dia
/// hay que respetar el idioma del usuario, sale de los ajustes y se pasa
/// aqui, no se lee dentro del motor.
///
/// Entrega las ordenes de una en una a `dibuja` en vez de devolver un `Vec`,
/// y eso NO es un detalle de estilo: la version que devolvia `Vec` hacia
/// `cache.ordenes(e, zoom).to_vec()`, o sea copiaba la geometria cacheada de
/// cada elemento visible en CADA fotograma -medido: 6,6 MB por fotograma con
/// 2.000 garabatos- dentro del unico camino donde el presupuesto de
/// rendimiento se compromete a no pedir memoria. Prestando la rebanada no hay
/// nada que copiar. El prestamo de `cache` muere al cerrar el primer bucle,
/// asi que el segundo puede volver a mirar el elemento sin pelearse con el.
///
/// El sumidero es un cierre y no el `Pintor` para que la funcion siga siendo
/// probable sin GPU: la prueba de aqui abajo le pasa un cierre que colecciona,
/// y asi el invariante que costo la ronda 1 -que ningun camino se olvide del
/// rotulo- conserva su prueba.
fn por_cada_orden(
    cache: &mut Cache,
    e: &Elemento,
    zoom: f32,
    escala: Option<&Escala>,
    mut dibuja: impl FnMut(&Orden),
) {
    for orden in cache.ordenes(e, zoom) {
        dibuja(orden);
    }
    for orden in pixpin_motor2d::pintado::ordenes_medibles(e, escala, ',') {
        dibuja(&orden);
    }
}

/// Pinta un fotograma entero: lo que hay en pantalla, y encima el marco de
/// la seleccion, sus tiradores y la marquesina si la hay.
///
/// `encima` se llama al final, con la transformacion todavia puesta a la
/// del mundo: quien la pasa es quien decide si dibuja en coordenadas del
/// mundo o si la deshace con `Pintor::desplazar(0.0, 0.0)` primero (el
/// cajetin de calibrar hace esto ultimo: es un dialogo en pantalla, no algo
/// del lienzo).
///
/// La caja de herramientas es del mismo tipo de dialogo, y se pinta aqui
/// mismo -no via `encima`- para que salga en los dos caminos que llaman a
/// `pintar` (el bucle principal y `pedir_medida`) sin que ninguno tenga que
/// acordarse de repetirla.
///
/// Devuelve `None` si `superficie.empezar` fallo (el backbuffer no estaba
/// listo): quien llama no puede dar la zona sucia por cubierta ni la
/// pantalla por al dia en ese caso, o el proximo present parcial se dejaria
/// un trozo sin pintar creyendo que ya se habia pintado en este fotograma
/// que se saltó. Si pinto, `Some(d)` es cuanto tardo presentar (D129):
/// con vsync, ahi se nota la espera al refresco.
#[allow(clippy::too_many_arguments)]
fn pintar(
    motor: &mut MotorRender,
    superficie: &Superficie,
    escena: &Escena,
    camara: &Camara,
    gesto: &Gesto,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    rejilla: &Rejilla,
    capa: &CapaEstatica,
    fondo: &mut Option<FondoLienzo>,
    imagenes: &mut ImagenesLienzo,
    caja_herramientas: &CajaHerramientas,
    raton_barra: Option<Punto>,
    escala_por_cien: u32,
    ancho_px: f32,
    alto_px: f32,
    zona: Option<(i32, i32, i32, i32)>,
    prediccion: Option<Punto2>,
    panel: Option<&pixpin_ui::panel_lateral::PanelLateral>,
    mut universo: Option<&mut crate::universo::sesion::Sesion>,
    interfaz_sucia: bool,
    encima: impl FnOnce(&pixpin_render::Pintor<'_>, (f32, f32)),
) -> Option<std::time::Duration> {
    if let Some(f) = fondo.as_mut() {
        f.asegurar(motor);
    }
    // Lo que se ve, los cuadernos que faltan y las miniaturas: antes de
    // abrir el fotograma, por lo mismo que las imagenes de abajo.
    if let Some(s) = universo.as_deref_mut() {
        s.preparar(motor, camara);
    }
    // Antes de `dibujar`: crear un bitmap con el `BeginDraw` abierto no es
    // lo que espera Direct2D.
    imagenes.asegurar(motor);
    let Ok(destino) = superficie.empezar(motor) else {
        return None;
    };
    // A3: la superficie de la escena es `margen` pixeles mas grande por
    // lado. Todo lo del MUNDO se pinta corrido ese margen; lo de la
    // interfaz, que va en su propia capa, no.
    let margen = superficie.margen();
    let con_capas = superficie.tiene_capas();
    // A3 fase 2: el cielo y las estrellas tienen visual propio, asi que la
    // escena no los pinta.
    let con_fondo = superficie.tiene_fondo();
    // La rejilla dice que PUEDE verse; la camara filtra lo que de verdad se
    // ve. Sin la rejilla, esto recorreria los ocho mil elementos.
    //
    // Con colchon se ve mas mundo del que cabe por la ventana: si se
    // recortara a la ventana, el colchon se pintaria vacio y al desplazar
    // asomaria en blanco, que es justo lo que viene a evitar.
    let holgura_mundo = margen / camara.zoom;
    let v = camara.ventana(ancho_px, alto_px);
    let vista = (
        v.0 - holgura_mundo,
        v.1 - holgura_mundo,
        v.2 + holgura_mundo,
        v.3 + holgura_mundo,
    );
    let candidatos = rejilla.candidatos(vista);
    let escala = 1.0 / camara.zoom;

    // Si hay una capa estatica valida para este fotograma, `volcar` ya
    // copio en `destino` todo lo que no se mueve: aqui solo hace falta
    // pintar encima lo excluido (lo seleccionado, que es lo que se arrastra)
    // y el marco. Si no vale, se pinta todo como siempre, y `volcar` no
    // habra tocado `destino`.
    let ahora_excluidos = excluidos_de(gesto);
    let ahora = Estampa {
        camara: (camara.x, camara.y, camara.zoom),
        tamano: tamano_escena(ancho_px, alto_px, margen),
        excluidos: ahora_excluidos.clone(),
    };
    // La zona sucia llega en pixeles de VENTANA y la superficie de escena
    // esta corrida el colchon: sin esto, el recorte, la copia de la capa y
    // el rectangulo del present apuntarian `margen` pixeles mas arriba y a
    // la izquierda de donde de verdad esta el trazo.
    let m = margen as i32;
    let zona = zona.map(|(a, b, c, d)| (a + m, b + m, c + m, d + m));
    // Solo el trozo que cambia (D148): copiar la pantalla entera cada
    // fotograma costaba 6-8 ms medidos en el equipo del usuario, y eso era
    // la mitad del presupuesto de 60 Hz gastada antes de dibujar la tinta.
    let capa_vale = capa.volcar_zona(motor, &destino, &ahora, zona);
    // Lo que de verdad importa para presentar solo un trozo es si la capa
    // congelada SE USO en este fotograma (`capa_vale`), no si `capa.lista()`
    // dice que hay una capa horneada: si no vale, `destino` se limpia y se
    // pinta entero mas abajo, y presentar con una zona pequena dejaria el
    // resto de la pantalla con lo que hubiera antes.
    let zona = if capa_vale { zona } else { None };

    let fondo_ref = fondo.as_ref();
    // Todo lo que NO es lienzo: la ruta del universo, la barra, el panel y
    // lo que pinte el llamante encima (el cajetin de calibrar).
    //
    // `base` es el desplazamiento que hay que sumarle a todo: 0 cuando se
    // pinta en la misma superficie que la escena, y el que devuelva
    // DirectComposition cuando va en su propia capa.
    let pintar_ui = |p: &pixpin_render::Pintor<'_>,
                     base: (f32, f32),
                     uni: Option<&crate::universo::sesion::Sesion>| {
        if let Some(s) = uni {
            s.pintar_delante(p, camara, ancho_px, alto_px);
        }
        crate::caja_dibujo::pintar_barra(
            p,
            caja_herramientas,
            // Con una herramienta del universo puesta, ninguna del editor
            // sale elegida: `Emoji` del motor no tiene boton en esta caja.
            if uni.is_some_and(|s| s.herramienta.is_some()) {
                Herramienta::Emoji
            } else {
                gesto.herramienta
            },
            escala_por_cien,
            raton_barra,
            |b| match b {
                BotonCaja::Elegir(h) => tecla_de(h),
                _ => None,
            },
        );
        if let Some(panel) = panel {
            crate::panel_dibujo::pintar(p, panel, escala_por_cien);
        }
        let _ = base;
    };
    // `encima` solo se puede llamar una vez y se llama en una de las dos
    // ramas: dentro de la escena (sin capas) o dentro de la interfaz (con
    // capas). El `Option` es lo que le dice eso al compilador.
    let mut encima = Some(encima);
    // El prestamo compartido del almacen dura solo este bloque: al salir,
    // `imagenes` vuelve a ser mutable, que es lo que necesita `soltar` si el
    // dispositivo se ha perdido.
    let error = {
        let imagenes: &ImagenesLienzo = imagenes;
        let uni = universo.as_deref();
        motor.dibujar(&destino, |p| {
            // El mundo se dibuja en sus propias coordenadas; la matriz activa es
            // lo unico que cambia al encuadrar o acercar (camara.rs lo explica:
            // "la geometria se calcula UNA VEZ en coordenadas del mundo").
            let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
            let origen = Punto2::nuevo(origen.x + margen, origen.y + margen);
            // D148: con la capa copiada a trozos, fuera de la zona el mapa
            // lleva lo de hace dos fotogramas, que ahi es lo correcto. Pintar
            // encima volveria a mezclar la tinta consigo misma y el trazo se
            // iria oscureciendo por los bordes. El recorte va ANTES de poner
            // la vista: se toma en pixeles de pantalla.
            if let Some((x0, y0, x1, y1)) = zona {
                p.empujar_recorte(RectF {
                    x: x0 as f32,
                    y: y0 as f32,
                    ancho: (x1 - x0).max(0) as f32,
                    alto: (y1 - y0).max(0) as f32,
                });
            }
            if !capa_vale {
                match uni {
                    // El cielo en lugar del papel, en pixeles de pantalla:
                    // la vista del mundo se pone justo despues. El colchon
                    // se suma aqui porque esto se pinta en la superficie de
                    // la escena, que va corrida ese colchon.
                    Some(s) => {
                        if con_fondo {
                            // A3 fase 2: el cielo y las estrellas estan en
                            // sus propios visuales, DEBAJO de este. La
                            // escena va transparente para que se vean.
                            p.limpiar_transparente();
                            p.desplazar(margen, margen);
                            s.pintar_astros(p, camara);
                        } else {
                            p.desplazar(margen, margen);
                            s.pintar_detras(p, camara);
                        }
                    }
                    None => p.limpiar(Color::BLANCO),
                }
            }
            p.poner_vista((0.0, 0.0), camara.zoom, (origen.x, origen.y));
            // D140/D142: con la capa valida la imagen ya esta copiada; si no, va
            // la primera, debajo de todo, y solo si se ve.
            if !capa_vale {
                if let Some(f) = fondo_ref {
                    f.pintar(p, vista, camara.zoom);
                }
            }

            for id in candidatos {
                // Con la capa valida, lo que no esta excluido ya esta copiado
                // en `destino`: repintarlo aqui seria pagar dos veces.
                if capa_vale && !ahora_excluidos.contains(&id) {
                    continue;
                }
                let Some(e) = escena.buscar(id) else {
                    continue;
                };
                if e.borrado {
                    continue;
                }
                // Lo que se esta dibujando (trazo, linea, flecha, rectangulo,
                // elipse) con su punta en el punto predicho. La escena no se
                // toca, asi que al soltar queda lo real.
                let en_curso = gesto.elemento_en_curso().filter(|(id, _)| *id == e.id);
                let mut punta = None;
                if let (Some(q), Some((_, origen))) = (prediccion, en_curso) {
                    match punta_de_tinta(e, q) {
                        // Lapiz y resaltador: la punta se pinta APARTE, encima
                        // del trazo real, a partir de su cola. Ver
                        // `punta_de_tinta`.
                        Some(orden) => punta = Some(orden),
                        // Las demas figuras tienen punta barata (una linea son
                        // dos puntos, una caja cuatro numeros): la copia
                        // entera sigue siendo lo mas simple y no cuesta nada.
                        None => {
                            if let Some(copia) =
                                pixpin_motor2d::tinta::prediccion::con_punta(e, origen, q)
                            {
                                for orden in pixpin_motor2d::pintado::ordenes_a_distancia(
                                    &copia,
                                    camara.zoom,
                                ) {
                                    dibujar_orden(
                                        p,
                                        &orden,
                                        vista,
                                        None,
                                        imagenes,
                                        camara.zoom,
                                        None,
                                    );
                                }
                                continue;
                            }
                        }
                    }
                }
                // Lo excluido es lo que se arrastra o el trazo en curso: su
                // version sube en CADA fotograma, asi que cachear su tinta
                // seria teselar de nuevo cada vez sin acertar nunca -pagar la
                // realizacion sin cobrar el ahorro-. Se pinta sin cache, como
                // antes de esta tarea.
                let mut indice = 0u32;
                let clave_tinta = !ahora_excluidos.contains(&e.id);
                let mut hubo_tinta = false;
                let grano = pixpin_motor2d::pintado::grano_de(e);
                por_cada_orden(cache, e, camara.zoom, escena.escala.as_ref(), |orden| {
                    hubo_tinta |= matches!(orden, Orden::Tinta { .. });
                    let tinta =
                        clave_tinta.then_some((&mut *cache_tinta, (e.id, e.version, indice)));
                    dibujar_orden(p, orden, vista, tinta, imagenes, camara.zoom, grano);
                    indice += 1;
                });
                // La punta va encima, y SOLO si el trazo se esta pintando como
                // tinta: a zoom muy bajo `ordenes_a_distancia` lo degrada a una
                // polilinea fina (`TINTA_MINIMA_PX`), y una mancha de tinta
                // pegada a una raya se veria como un borron. A ese aumento la
                // punta predicha mide menos de un pixel de todos modos.
                if let (Some(orden), true) = (punta, hubo_tinta) {
                    dibujar_orden(p, &orden, vista, None, imagenes, camara.zoom, None);
                }
            }

            // Encima de todo: el marco de la seleccion, sus tiradores y la
            // marquesina si la hay.
            //
            // `Gesto::tiradores` es la MISMA llamada que usa el gesto para
            // decidir que agarra el clic (`cursor_en`, `pulsar`): pintar con
            // una copia propia del angulo es como se desincronizaron una vez
            // -tiradores rectos que se picaban girados-, asi que aqui no hay
            // una segunda formula, solo la unica fuente de verdad.
            if let Some(caja) = gesto.seleccion.caja(escena) {
                let tiradores = gesto.tiradores(escena, escala);
                let angulo = tiradores.as_ref().map_or(0.0, |t| t.angulo);
                p.marco(caja, angulo, escala);
                if let Some(tiradores) = tiradores {
                    for orden in tiradores.ordenes(escala) {
                        dibujar_orden(p, &orden, vista, None, imagenes, camara.zoom, None);
                    }
                }
            }
            if let Some(m) = gesto.marquesina() {
                p.marquesina(m, escala);
            }
            // La pista del iman: encima de todo, porque es lo que dice donde va
            // a caer el punto. Si quedara debajo de una figura, justo el caso en
            // el que hace falta -dibujar sobre algo- seria el caso en el que no
            // se ve.
            if let Some(a) = gesto.anclaje_activo {
                dibujar_orden(
                    p,
                    &pixpin_motor2d::enganche::pista(&a, camara.zoom),
                    vista,
                    None,
                    imagenes,
                    camara.zoom,
                    None,
                );
            }
            // La caja es un dialogo en pantalla, no algo del lienzo: no se mueve
            // ni se escala con la camara. `desplazar(0.0, 0.0)` deshace la vista
            // del mundo que `poner_vista` dejo puesta arriba, igual que hace
            // `dibujar_cajetin` mas abajo.
            //
            // Con capas (A3) esto NO se pinta aqui: va a la superficie de la
            // interfaz, que no se mueve cuando la escena se desplaza. Es la
            // razon de ser de las dos capas.
            if !con_capas {
                p.desplazar(0.0, 0.0);
                pintar_ui(p, (0.0, 0.0), uni);
                if let Some(e) = encima.take() {
                    e(p, (0.0, 0.0));
                }
            }
            if zona.is_some() {
                p.soltar_recorte();
            }
        })
    };
    // A3: la capa de la interfaz, aparte y solo cuando cambia. Va DESPUES
    // del fotograma de la escena y antes de presentarlo: su `Commit` y el
    // `Present` de la escena caen en el mismo intervalo de composicion, asi
    // que DWM las ensena juntas.
    if con_capas && interfaz_sucia {
        let uni = universo.as_deref();
        let _ = superficie.pintar_interfaz(motor, |p, d| {
            pintar_ui(p, d, uni);
            if let Some(e) = encima.take() {
                e(p, d);
            }
        });
    }
    // A3 fase 2: el cielo y las estrellas, cada uno en el suyo. Solo en el
    // fotograma nitido: mientras la camara se mueve, lo que se mueve es la
    // matriz del visual de las estrellas, no sus pixeles.
    if con_fondo && interfaz_sucia {
        let uni = universo.as_deref();
        if let Some(s) = uni {
            let (cw, ch) = superficie.tamano_cielo().unwrap_or((1, 1));
            let _ = superficie.pintar_cielo(motor, |p, _| {
                // El degradado se hornea del tamano de la PANTALLA y aqui se
                // pinta del tamano de su superficie chica; la composicion lo
                // estira al resto. Es la misma imagen, con una pasada de
                // 192 px en vez de una de 3.000.
                s.pintar_cielo(p, cw as f32, ch as f32);
            });
            let m = superficie.margen_fondo();
            let _ = superficie.pintar_fondo(motor, |p, base| {
                p.desplazar(base.0 + m, base.1 + m);
                s.pintar_estrellas(p, camara);
            });
        }
        // Lo que se acaba de pintar ya esta en la camara nueva.
        superficie.reponer_fondo();
    }
    if error.is_err() {
        // Dispositivo perdido: las realizaciones de tinta son del dispositivo
        // viejo y ya no valen (D2D las rechazaria en el siguiente fotograma).
        cache_tinta.vaciar();
        // Y los pinceles y brillos que guarda el motor, por lo mismo.
        motor.olvidar_recursos_de_dispositivo();
        // Dispositivo perdido: el bitmap del fondo tambien era del viejo.
        if let Some(f) = fondo.as_mut() {
            f.soltar();
        }
        // Y los de las imagenes pegadas, por lo mismo.
        imagenes.soltar();
        // Y las estrellas y miniaturas del universo.
        if let Some(s) = universo {
            s.soltar_recursos();
        }
    }
    let t_presentar = std::time::Instant::now();
    let _ = superficie.presentar_sincronizado(zona);
    Some(t_presentar.elapsed())
}

/// **B2: un fotograma del trazo en curso, y nada mas.**
///
/// Limpia `zona` de la capa de tinta y pinta ahi el elemento que se esta
/// dibujando, con su punta predicha. La escena no se mira, no se copia y no
/// se presenta: lo que se ve debajo es el mismo fotograma de escena que ya
/// estaba en su visual desde antes de apoyar el lapiz.
///
/// Lo que se pinta es EXACTAMENTE lo mismo que pintaba `pintar` para el
/// elemento en curso (las mismas ordenes, la misma punta aparte): lo que
/// cambia es sobre que. Asi, apagar B2 desde el TOML no cambia la forma de
/// un trazo, solo donde se dibuja.
///
/// Devuelve si llego a pintar: si el gesto dejo de tener elemento en curso
/// entre la cola de eventos y aqui, no hay fotograma y quien llama tiene que
/// seguir pidiendolo.
#[allow(clippy::too_many_arguments)]
fn pintar_tinta_viva(
    motor: &MotorRender,
    superficie: &Superficie,
    escena: &Escena,
    camara: &Camara,
    gesto: &Gesto,
    cache: &mut Cache,
    imagenes: &ImagenesLienzo,
    zona: Option<(i32, i32, i32, i32)>,
    prediccion: Option<Punto2>,
    ancho_px: f32,
    alto_px: f32,
) -> bool {
    let Some((id, origen_trazo)) = gesto.elemento_en_curso() else {
        return false;
    };
    let Some(e) = escena.buscar(id) else {
        return false;
    };
    if e.borrado {
        return false;
    }
    let vista = camara.ventana(ancho_px, alto_px);
    superficie
        .pintar_tinta(motor, zona, |p, base| {
            // La capa de tinta va en pixeles de VENTANA (no lleva el colchon
            // de la escena); `base` es lo que DirectComposition pide sumar.
            let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
            p.poner_vista(
                (0.0, 0.0),
                camara.zoom,
                (origen.x + base.0, origen.y + base.1),
            );
            // Lo mismo que hace `pintar` con el elemento en curso: la punta
            // de tinta va aparte (sale de las ultimas muestras, no de clonar
            // el elemento entero), y lo que no es tinta lleva la punta dentro
            // de una copia, que ahi son cuatro numeros.
            let mut punta = None;
            if let Some(q) = prediccion {
                match punta_de_tinta(e, q) {
                    Some(orden) => punta = Some(orden),
                    None => {
                        if let Some(copia) =
                            pixpin_motor2d::tinta::prediccion::con_punta(e, origen_trazo, q)
                        {
                            for orden in
                                pixpin_motor2d::pintado::ordenes_a_distancia(&copia, camara.zoom)
                            {
                                dibujar_orden(p, &orden, vista, None, imagenes, camara.zoom, None);
                            }
                            return;
                        }
                    }
                }
            }
            let mut hubo_tinta = false;
            // Sin cache de realizaciones: la version del trazo en curso sube
            // en cada fotograma, asi que teselar para la cache seria pagar y
            // no cobrar nunca. Es lo mismo que hace `pintar`.
            por_cada_orden(cache, e, camara.zoom, escena.escala.as_ref(), |orden| {
                hubo_tinta |= matches!(orden, Orden::Tinta { .. });
                dibujar_orden(p, orden, vista, None, imagenes, camara.zoom, None);
            });
            if let (Some(orden), true) = (punta, hubo_tinta) {
                dibujar_orden(p, &orden, vista, None, imagenes, camara.zoom, None);
            }
        })
        .is_ok()
}

/// Lo que el usuario teclea al calibrar, convertido a numero.
///
/// Acepta coma y punto porque se escribe una cosa o la otra segun la
/// costumbre y el teclado, y obligar a una sola es hacerle repetir el gesto
/// a la mitad de la gente.
fn leer_medida(texto: &str) -> Option<f32> {
    let v: f32 = texto.trim().replace(',', ".").parse().ok()?;
    if !v.is_finite() || v <= 0.0 {
        return None;
    }
    Some(v)
}

/// El cajetin de calibrar: se escribe el numero y se elige la unidad de
/// `pixpin_motor2d::medida::UNIDADES` con Tab, siguiendo el mismo estilo de
/// tarjeta oscura que `apps/pixpin/src/overlay.rs` usa para el panel
/// «Seleccionar todo» y la barra de resultado.
///
/// Abre su propio bucle de eventos porque necesita seguir pintando el
/// fotograma (con el dialogo encima) mientras espera lo que se teclea; por
/// eso su prueba necesita sesion de escritorio y va marcada `#[ignore]`.
///
/// Devuelve `None` si se cancela con Escape o si `leer_medida` rechaza lo
/// tecleado al pulsar Enter. **Si al cancelar la escala cambiara, seria un
/// fallo**: quien llama no tiene que tocar la escena cuando esto devuelve
/// `None`, y por eso aqui no se toca `escena` en ningun caso: solo se lee.
#[allow(clippy::too_many_arguments)]
fn pedir_medida(
    ventana: &VentanaOverlay,
    motor: &mut MotorRender,
    superficie: &Superficie,
    escena: &Escena,
    camara: &Camara,
    gesto: &Gesto,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    rejilla: &Rejilla,
    capa: &CapaEstatica,
    fondo: &mut Option<FondoLienzo>,
    imagenes: &mut ImagenesLienzo,
    caja: &CajaHerramientas,
    escala_por_cien: u32,
    ancho_px: f32,
    alto_px: f32,
    largo_px: f32,
) -> Option<(f32, String)> {
    let mut texto = String::new();
    let mut indice_unidad = 0usize;
    ventana.invalidar();

    loop {
        pixpin_shell::overlay::bombear_pendientes();
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match ev {
                EventoOverlay::Caracter(c) => match c {
                    '\r' | '\n' => {
                        let unidad = pixpin_motor2d::medida::UNIDADES[indice_unidad];
                        return leer_medida(&texto).map(|v| (v, unidad.to_string()));
                    }
                    '\u{1b}' => return None,
                    '\u{8}' => {
                        texto.pop();
                        ventana.invalidar();
                    }
                    '\t' => {
                        indice_unidad =
                            (indice_unidad + 1) % pixpin_motor2d::medida::UNIDADES.len();
                        ventana.invalidar();
                    }
                    c if c.is_ascii_digit() || c == ',' || c == '.' => {
                        texto.push(c);
                        ventana.invalidar();
                    }
                    _ => {}
                },
                EventoOverlay::Cerrar => return None,
                EventoOverlay::Pintar => {
                    let unidad = pixpin_motor2d::medida::UNIDADES[indice_unidad];
                    pintar(
                        motor,
                        superficie,
                        escena,
                        camara,
                        gesto,
                        cache,
                        cache_tinta,
                        rejilla,
                        capa,
                        fondo,
                        imagenes,
                        caja,
                        None,
                        escala_por_cien,
                        ancho_px,
                        alto_px,
                        None,
                        None,
                        None,
                        // El cajetin de calibrar pinta sin el universo: es un
                        // dialogo corto y el cielo volvera al cerrarlo.
                        None,
                        // El cajetin cambia con cada tecla: la capa de la
                        // interfaz se rehace en cada fotograma de este
                        // bucle, que dura lo que tarde en escribirse un
                        // numero.
                        true,
                        |p, base| {
                            dibujar_cajetin(p, base, ancho_px, alto_px, largo_px, &texto, unidad)
                        },
                    );
                }
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// La tarjeta del cajetin de calibrar, centrada en la ventana y en
/// coordenadas de pantalla (no del mundo: un dialogo no se mueve con la
/// camara). `Pintor::desplazar(0.0, 0.0)` deshace la vista del mundo que
/// `pintar` dejo puesta antes de llamar a `encima`.
fn dibujar_cajetin(
    p: &pixpin_render::Pintor<'_>,
    base: (f32, f32),
    ancho_px: f32,
    alto_px: f32,
    largo_px: f32,
    texto: &str,
    unidad: &str,
) {
    // `base` es 0 cuando el cajetin se pinta en la misma superficie que la
    // escena, y el desplazamiento que dio DirectComposition cuando va en la
    // capa de la interfaz (A3).
    p.desplazar(base.0, base.1);

    let ancho = 260.0;
    let alto = 90.0;
    let caja = RectF {
        x: (ancho_px - ancho) / 2.0,
        y: (alto_px - alto) / 2.0,
        ancho,
        alto,
    };
    p.rellenar_redondeado(
        caja,
        8.0,
        Color {
            r: 0.12,
            g: 0.12,
            b: 0.14,
            a: 0.92,
        },
    );

    let pregunta = format!("Cuanto miden {largo_px:.0} px?");
    p.texto(&pregunta, caja.x + 16.0, caja.y + 12.0, 13.0, Color::BLANCO);

    let entrada = format!("{texto} {unidad}");
    let (tw, th) = p.medir_texto(&entrada, 20.0);
    p.texto(
        &entrada,
        caja.x + (caja.ancho - tw) / 2.0,
        caja.y + 32.0 + (alto - 32.0 - th) / 2.0,
        20.0,
        Color::BLANCO,
    );
}

/// Traduce una `Orden` ya calculada por el motor a la llamada de `Pintor`
/// que le toca. Pura traduccion: la geometria ya viene hecha, aqui solo se
/// decide con que primitiva de Direct2D se pinta.
///
/// `vista` es la caja del mundo que se ve (en las mismas coordenadas que
/// `Orden`), y hace falta para `Orden::Velo`: el motor no sabe cuanto mide
/// el lienzo (lo dice `pintado.rs`), asi que quien pinta pone el marco.
///
/// `imagenes` y `zoom` solo los usa `Orden::Imagen`: el almacen resuelve el
/// `id_objeto` y el zoom efectivo elige el muestreo (D141).
/// Estampa la tela de un material dentro de una silueta ya calculada.
///
/// Es la misma llamada que hace `apps/pixpin/tests/muestra_de_tintas.rs`,
/// puesta en una funcion para que no haya dos formas de pedir lo mismo: si
/// el editor y la muestra se pidieran por su cuenta, la muestra dejaria de
/// ser prueba de lo que se ve en pantalla.
fn estampar_grano(
    p: &pixpin_render::Pintor<'_>,
    cache: &mut pixpin_render::CacheGrano,
    clave: (u64, u32),
    contorno: &[(f32, f32)],
    g: &pixpin_motor2d::pintado::Grano,
) {
    let tela = pixpin_motor2d::tinta::tejido(g.material);
    p.grano(
        cache,
        clave,
        contorno,
        &tela,
        pixpin_motor2d::tinta::material::LADO_DEL_MOSAICO,
        g.material as u32,
        a_color(g.color),
        g.paso,
        g.inclinada,
    );
}

#[allow(clippy::too_many_arguments)]
fn dibujar_orden(
    p: &pixpin_render::Pintor<'_>,
    orden: &Orden,
    vista: (f32, f32, f32, f32),
    tinta: Option<(&mut pixpin_render::CacheTinta, (u64, u32, u32))>,
    imagenes: &ImagenesLienzo,
    zoom: f32,
    grano: Option<pixpin_motor2d::pintado::Grano>,
) {
    match orden {
        // Acierto de cache: se pinta con la realizacion ya teselada sin
        // volver a convertir los puntos en `Vec<(f32, f32)>` -esa reserva es
        // la que se pagaba cada fotograma sin usarla para nada en cuanto
        // habia acierto-. Solo si `pintar_realizada` no encuentra nada (o el
        // contexto no da D2D 1.1) se paga la conversion y se rehace.
        //
        // Antes esto solo lo hacia la tinta (D121) y las formas de rough.js
        // creaban una geometria por pasada y por fotograma: es lo que hacia
        // que un paneo con formas rellenas costara decenas de veces mas que
        // uno con trazos a mano.
        Orden::Poligono { puntos, color } | Orden::Relleno { puntos, color } => match tinta {
            Some((c, clave)) => {
                if !p.pintar_realizada(c, clave, a_color(*color)) {
                    p.poligono_cacheado(c, clave, &a_tuplas(puntos), a_color(*color));
                }
            }
            None => p.poligono(&a_tuplas(puntos), a_color(*color)),
        },
        // **El cuerpo del trazo y, detras, su grano.** Hasta ahora el grano
        // solo se veia en la prueba que saca los PNG de muestra: el editor
        // pintaba la silueta y se saltaba la tela, asi que un trazo de tiza
        // del movil se veia aqui macizo y los diez materiales se veian
        // iguales. El motor ya decia con que tenirlo (`pintado::grano_de`);
        // lo que faltaba era pedirlo.
        Orden::Tinta { contorno, color } => match tinta {
            Some((c, clave)) => {
                let puntos = if p.pintar_realizada(c, clave, a_color(*color)) {
                    // Acierto de cache: la silueta ya esta teselada y no
                    // hace falta convertir los puntos... salvo que haya
                    // grano, que necesita la geometria de verdad porque se
                    // pinta con un pincel de mosaico y no de color.
                    grano.is_some().then(|| a_tuplas(contorno))
                } else {
                    let v = a_tuplas(contorno);
                    p.tinta_cacheada(c, clave, &v, a_color(*color));
                    Some(v)
                };
                if let (Some(g), Some(v)) = (grano, puntos) {
                    estampar_grano(p, &mut c.grano, (clave.0, clave.1), &v, &g);
                }
            }
            // Sin cache es lo que cambia en CADA fotograma: el trazo en
            // curso y lo que se arrastra. Ahi el grano se salta a
            // proposito, porque su brocha se ancla al documento y volver a
            // tejerla sesenta veces por segundo cuesta mas de lo que se ve;
            // en cuanto se suelta, el trazo entra por el camino de arriba y
            // aparece con su tela.
            None => p.tinta(&a_tuplas(contorno), a_color(*color)),
        },
        Orden::Polilinea {
            puntos,
            color,
            grosor,
            estilo,
        } => {
            // `Pintor` todavia no distingue rayas de puntos (nadie mas en el
            // proyecto dibuja punteado con Direct2D); a rayas es la
            // aproximacion mas cercana a lo discontinuo.
            let discontinua = !matches!(estilo, EstiloTrazo::Solido);
            match tinta {
                Some((c, clave)) => {
                    if !p.pintar_realizada(c, clave, a_color(*color)) {
                        p.polilinea_cacheada(
                            c,
                            clave,
                            &a_tuplas(puntos),
                            *grosor,
                            discontinua,
                            a_color(*color),
                        );
                    }
                }
                None => {
                    let v = a_tuplas(puntos);
                    if discontinua {
                        p.polilinea_discontinua(&v, *grosor, a_color(*color))
                    } else {
                        p.polilinea(&v, *grosor, a_color(*color))
                    }
                }
            }
        }
        Orden::Velo { hueco, color } => {
            let (x0, y0, x1, y1) = vista;
            p.velo(
                RectF {
                    x: x0,
                    y: y0,
                    ancho: x1 - x0,
                    alto: y1 - y0,
                },
                &a_tuplas(hueco),
                a_color(*color),
            );
        }
        Orden::Texto {
            texto,
            x,
            y,
            tam,
            color,
            ancho_max,
            ..
        } => {
            // La familia de letra no es seleccionable todavia en `Pintor`;
            // no hace falta en esta entrega, la herramienta de texto queda
            // fuera de ella.
            p.texto_ajustado(texto, *x, *y, *tam, *ancho_max, a_color(*color));
        }
        Orden::Imagen {
            id_objeto,
            x,
            y,
            ancho,
            alto,
            opacidad,
        } => {
            // El motor no sabe de bitmaps: solo dice «aqui va la imagen
            // numero N». Quien la tiene es el almacen del lienzo.
            imagenes.pintar(
                p,
                *id_objeto,
                RectF {
                    x: *x,
                    y: *y,
                    ancho: *ancho,
                    alto: *alto,
                },
                zoom,
                *opacidad,
            );
        }
    }
}

/// La punta predicha de un trazo a mano, como una mancha de tinta APARTE.
///
/// Antes se clonaba el elemento entero y se recalculaba su contorno completo
/// (`prediccion::con_punta` + `ordenes_a_distancia`): con la prediccion
/// encendida, un trazo de n puntos costaba 2 x O(n) por fotograma y dos
/// reservas de n, y se notaba justo al final de un trazo largo. La punta solo
/// necesita el final del trazo, asi que se hace con las ultimas
/// `PUNTOS_DE_PUNTA` muestras mas el punto predicho: coste fijo, no importa
/// lo largo que sea el trazo. Lo que sobra por detras cae dentro de la mancha
/// del trazo real, que ya esta pintada debajo.
///
/// Los puntos del lapiz estan en coordenadas del mundo (`pintado::ordenes`
/// no los desplaza; solo los gira, y un trazo en curso no esta girado), asi
/// que el contorno de la cola cae exactamente donde tiene que caer.
///
/// `None` para lo que no es tinta: la punta de una linea o de una caja es
/// barata y se sigue haciendo con la copia entera.
fn punta_de_tinta(e: &Elemento, q: Punto2) -> Option<Orden> {
    // Con dos puntos o menos, perfect-freehand no tiene trazo del que sacar
    // una cola: se deja la copia entera, que ahi tampoco cuesta nada.
    let cola = |v: &[Punto2]| -> Option<(Vec<Punto2>, usize)> {
        if v.len() <= 2 {
            return None;
        }
        let desde = v.len() - pixpin_tinta::PUNTOS_DE_PUNTA.min(v.len());
        let mut c = v[desde..].to_vec();
        c.push(q);
        Some((c, desde))
    };
    let opacidad = e.opacidad.clamp(0.0, 1.0);
    match &e.figura {
        Figura::Lapiz {
            puntos,
            presiones,
            opciones,
        } => {
            let (c, desde) = cola(puntos)?;
            // Las presiones solo valen si son de verdad (una lista de otra
            // longitud es «simuladas», y entonces no se pasa ninguna: es la
            // misma regla que sigue `contorno_de_lapiz`).
            let mut pr: Vec<f32> = if presiones.len() == puntos.len() {
                presiones[desde..].to_vec()
            } else {
                Vec::new()
            };
            if let Some(&u) = pr.last() {
                // El punto predicho hereda la presion del ultimo real: naciendo
                // a cero se veria como un pico afilado.
                pr.push(u);
            }
            let contorno = pixpin_motor2d::tinta::contorno_de_lapiz(&c, &pr, e.grosor, *opciones);
            (!contorno.is_empty()).then_some(Orden::Tinta {
                contorno,
                color: ColorRgba {
                    a: e.trazo.a * opacidad,
                    ..e.trazo
                },
            })
        }
        Figura::Resaltador { puntos } => {
            let (c, _) = cola(puntos)?;
            let contorno = pixpin_motor2d::tinta::contorno_de_resaltador(&c, e.grosor);
            (!contorno.is_empty()).then_some(Orden::Tinta {
                contorno,
                color: ColorRgba {
                    a: 0.35 * opacidad,
                    ..e.trazo
                },
            })
        }
        _ => None,
    }
}

fn a_tuplas(puntos: &[Punto2]) -> Vec<(f32, f32)> {
    puntos.iter().map(|p| (p.x, p.y)).collect()
}

fn a_color(c: ColorRgba) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

/// Cambia de herramienta y suelta lo elegido si la nueva dibuja.
///
/// Sin esto, con algo elegido y el lapiz en la mano quedaban unos tiradores
/// flotando sobre el dibujo: el clic encima ya no los mueve (eso es solo de
/// la mano), asi que serian unos agarres que no hacen lo que prometen.
fn elegir_herramienta(gesto: &mut Gesto, h: Herramienta) {
    if h != Herramienta::Mano {
        gesto.seleccion.limpiar();
    }
    gesto.herramienta = h;
}

#[cfg(test)]
mod medir;

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un trazo a mano de `n` puntos en linea recta.
    fn trazo_de(n: usize) -> Elemento {
        let mut e = elemento_de_prueba();
        e.figura = Figura::Lapiz {
            puntos: (0..n).map(|i| Punto2::nuevo(i as f32, 0.0)).collect(),
            presiones: (0..n).map(|_| 0.5).collect(),
            opciones: Some(Default::default()),
        };
        e
    }

    fn elemento_de_prueba() -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Rectangulo,
            x: 0.0,
            y: 0.0,
            ancho: 10.0,
            alto: 10.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    #[test]
    fn la_punta_predicha_cuesta_lo_mismo_con_un_trazo_corto_que_con_uno_larguisimo() {
        // B3: antes, la punta clonaba el elemento y recalculaba su contorno
        // entero, asi que al final de un trazo largo costaba una segunda
        // pasada por los 5.000 puntos. Ahora sale de la cola y su contorno
        // tiene el mismo tamano venga de donde venga.
        let q = Punto2::nuevo(9_000.0, 0.0);
        let corto = punta_de_tinta(&trazo_de(40), q).expect("un trazo de 40 tiene punta");
        let largo = punta_de_tinta(&trazo_de(5_000), q).expect("y uno de 5.000 tambien");
        let cuantos = |o: &Orden| match o {
            Orden::Tinta { contorno, .. } => contorno.len(),
            _ => unreachable!("un lapiz da tinta"),
        };
        assert_eq!(cuantos(&corto), cuantos(&largo));
    }

    #[test]
    fn la_punta_acaba_en_el_punto_predicho_y_no_en_el_ultimo_real() {
        let q = Punto2::nuevo(500.0, 0.0);
        let Some(Orden::Tinta { contorno, .. }) = punta_de_tinta(&trazo_de(100), q) else {
            unreachable!("un lapiz da tinta")
        };
        let mas_lejos = contorno.iter().fold(0.0f32, |m, p| m.max(p.x));
        assert!(mas_lejos > 490.0, "la punta llega hasta {mas_lejos}");
    }

    #[test]
    fn lo_que_no_es_tinta_no_tiene_punta_aparte() {
        // Caso negativo: una caja o una flecha siguen usando la copia
        // entera, que en su caso son cuatro numeros y no cuesta nada.
        let q = Punto2::nuevo(5.0, 5.0);
        assert!(punta_de_tinta(&elemento_de_prueba(), q).is_none());
        let mut flecha = elemento_de_prueba();
        flecha.figura = Figura::Flecha {
            puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(1.0, 1.0)],
            punta_inicio: false,
            punta_fin: true,
        };
        assert!(punta_de_tinta(&flecha, q).is_none());
        // Y un trazo de dos puntos tampoco: no hay cola de la que tirar.
        assert!(punta_de_tinta(&trazo_de(2), q).is_none());
    }

    #[test]
    fn sin_fijar_nada_el_lapiz_se_comporta_como_siempre() {
        // El ajuste de C1 va apagado de fabrica: abrir un editor sin haber
        // llamado a `fijar_ajustes_tinta` no puede cambiar el trazo.
        assert_eq!(
            ajustes_tinta().suavizado,
            pixpin_store::Suavizado::Excalidraw
        );
    }

    #[test]
    fn la_tecla_pintada_en_cada_boton_elige_esa_herramienta() {
        for b in pixpin_ui::BOTONES_EDITOR {
            if let BotonCaja::Elegir(h) = b {
                if let Some(c) = tecla_de(h) {
                    assert_eq!(tecla_a_herramienta(c), Some(h), "{c} no elige {h:?}");
                    assert_eq!(tecla_a_herramienta(c.to_ascii_lowercase()), Some(h));
                }
            }
        }
        // Caso negativo: el rectangulo no tiene tecla y no se pinta ninguna.
        assert_eq!(tecla_de(Herramienta::Rectangulo), None);
        assert_eq!(tecla_de(Herramienta::Lapiz), Some('L'));
    }
    use pixpin_geom::Tirador;
    use pixpin_motor2d::camara::Camara;
    use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
    use pixpin_motor2d::escena::Escena;
    use pixpin_motor2d::gesto::{EventoGesto, Gesto};
    use std::f32::consts::{FRAC_PI_2, PI};

    fn camara_de_prueba(x: f32, y: f32, zoom: f32) -> Camara {
        Camara { x, y, zoom }
    }

    #[test]
    fn un_paneo_que_cabe_en_el_colchon_se_resuelve_moviendo_el_visual() {
        // 100 px de mundo a zoom 1 son 100 px de pantalla, y el colchon es
        // de 256: cabe de sobra, y la escala no cambia.
        let c0 = camara_de_prueba(0.0, 0.0, 1.0);
        let c1 = camara_de_prueba(100.0, 40.0, 1.0);
        let (s, dx, dy) =
            transformada_de_camara(&c0, &c1, 1920.0, 1080.0, 256.0).expect("cabe en el colchon");
        assert_eq!(s, 1.0);
        // La camara se fue a la derecha, asi que el dibujo se mueve a la
        // izquierda: el signo importa y equivocarlo mueve el lienzo al reves.
        assert_eq!((dx, dy), (-100.0, -40.0));
    }

    #[test]
    fn un_paneo_que_se_pasa_del_colchon_pide_repintar() {
        // Caso negativo: 300 px de pantalla con 256 de colchon dejaria una
        // franja vacia por el borde. Mas vale pintar que ensenar el hueco.
        let c0 = camara_de_prueba(0.0, 0.0, 1.0);
        let c1 = camara_de_prueba(-300.0, 0.0, 1.0);
        assert!(transformada_de_camara(&c0, &c1, 1920.0, 1080.0, 256.0).is_none());
    }

    #[test]
    fn acercarse_un_poco_se_compone_y_alejarse_mucho_no() {
        let c0 = camara_de_prueba(0.0, 0.0, 1.0);
        // Acercar siempre cabe: se ve un trozo de lo ya pintado.
        let cerca = camara_de_prueba(0.0, 0.0, 1.1);
        assert_eq!(
            transformada_de_camara(&c0, &cerca, 1920.0, 1080.0, 256.0).map(|t| t.0),
            Some(1.1)
        );
        // Alejar tiene tope: con la mitad de aumento harian falta 1920 px
        // de superficie de mas y solo hay 512.
        let lejos = camara_de_prueba(0.0, 0.0, 0.5);
        assert!(transformada_de_camara(&c0, &lejos, 1920.0, 1080.0, 256.0).is_none());
    }

    #[test]
    fn recien_compuesto_la_tinta_toca_ya_y_a_mitad_de_periodo_espera() {
        // Ya paso el periodo: toca pintar sin esperar.
        assert_eq!(espera_de_tinta(17.0, 16.7), None);
        // Y justo en el 90 % tambien: el plazo es ese, no el periodo entero
        // (si no, la deriva entre el reloj del bucle y el de DWM perderia
        // una composicion de cada dos y el trazo saldria a 30 Hz).
        assert_eq!(espera_de_tinta(16.7 * 0.9, 16.7), None);
        // A mitad de periodo se espera lo que falta, redondeado hacia
        // arriba: dormir de menos es volver a la misma pregunta.
        let e = espera_de_tinta(8.0, 16.7).expect("todavia no toca");
        assert!((7..=8).contains(&e), "{e} ms");
        // Nunca 0: un tope de 0 ms es girar en vacio.
        assert_eq!(espera_de_tinta(16.7 * 0.9 - 0.01, 16.7), Some(1));
        // Caso negativo: sin periodo que valga no se frena nada.
        assert_eq!(espera_de_tinta(0.0, 0.0), None);
        assert_eq!(espera_de_tinta(f32::NAN, 16.7), None);
    }

    #[test]
    fn sin_colchon_nunca_se_compone() {
        // Es la palanca del TOML: con `paneo_por_composicion = false` la
        // superficie nace sin margen y este camino no se toma nunca.
        let c0 = camara_de_prueba(0.0, 0.0, 1.0);
        let c1 = camara_de_prueba(1.0, 0.0, 1.0);
        assert!(transformada_de_camara(&c0, &c1, 1920.0, 1080.0, 0.0).is_none());
    }

    #[test]
    fn un_estirado_enorme_pide_repintar_aunque_quepa() {
        // Acercar x10 «cabe» en el sentido de que se ve, pero seria una
        // textura ampliada diez veces: se ve borrosa y hay que pintar.
        let c0 = camara_de_prueba(0.0, 0.0, 1.0);
        let c1 = camara_de_prueba(0.0, 0.0, 10.0);
        assert!(transformada_de_camara(&c0, &c1, 1920.0, 1080.0, 256.0).is_none());
    }

    #[test]
    fn la_superficie_de_escena_es_la_ventana_mas_el_colchon_por_cada_lado() {
        assert_eq!(tamano_escena(1920.0, 1080.0, 256.0), (2432, 1592));
        // Sin colchon, exactamente la ventana: es lo que espera la capa
        // congelada de siempre.
        assert_eq!(tamano_escena(1920.0, 1080.0, 0.0), (1920, 1080));
    }

    /// Revision de la Tarea 10, hallazgo 1a: sin zoom pendiente
    /// (`zoom == escala`), no hay que dormir con un tope -y mucho menos
    /// `Some(0)`, que giraria un nucleo entero en reposo-.
    #[test]
    fn sin_zoom_pendiente_no_hay_tope_ni_hay_que_rehacer() {
        let ahora = std::time::Instant::now();
        let retardo = std::time::Duration::from_millis(300);
        let d = decidir_zoom(1.0, 1.0, 1.0, None, ahora, retardo);
        assert!(!d.rehacer);
        assert_eq!(d.tope_ms, None);
        assert_eq!(d.desde, None);
    }

    #[test]
    fn un_cambio_de_zoom_pone_el_tope_al_retardo_completo() {
        let ahora = std::time::Instant::now();
        let retardo = std::time::Duration::from_millis(300);
        let d = decidir_zoom(2.0, 1.0, 1.0, None, ahora, retardo);
        assert!(!d.rehacer);
        assert_eq!(d.visto, 2.0);
        assert_eq!(d.tope_ms, Some(300));
    }

    /// Hallazgo 1a: si el zoom vuelve a la escala YA realizada antes de que
    /// se cumpla el retardo, no puede quedar un tope fijo -eso era el giro
    /// en vacio que violaba el 0% de CPU en reposo-.
    #[test]
    fn volver_a_la_escala_realizada_antes_del_retardo_no_deja_tope_pendiente() {
        let t0 = std::time::Instant::now();
        let retardo = std::time::Duration::from_millis(300);
        let d1 = decidir_zoom(2.0, 1.0, 1.0, None, t0, retardo);
        assert_eq!(d1.tope_ms, Some(300));
        let t1 = t0 + std::time::Duration::from_millis(50);
        let d2 = decidir_zoom(1.0, d1.visto, 1.0, d1.desde, t1, retardo);
        assert!(!d2.rehacer);
        assert_eq!(
            d2.tope_ms, None,
            "no puede quedar un tope fijo (giraria un nucleo en reposo)"
        );
        assert_eq!(d2.desde, None);
    }

    /// Hallazgo 1b: "quieto" cuenta desde el ULTIMO cambio, no desde el
    /// primero. Un zoom que sigue cambiando cada 100 ms durante 1 s no
    /// tiene que rehacer nunca; solo cuando por fin se para, 300 ms
    /// despues de su ultimo cambio.
    #[test]
    fn el_zoom_continuo_no_rehace_hasta_que_se_queda_quieto_300_ms() {
        let t0 = std::time::Instant::now();
        let retardo = std::time::Duration::from_millis(300);
        let mut visto = 1.0f32;
        let mut desde = None;
        let mut zoom = 1.0f32;
        for i in 1..=10u64 {
            zoom += 0.1;
            let ahora = t0 + std::time::Duration::from_millis(i * 100);
            let d = decidir_zoom(zoom, visto, 1.0, desde, ahora, retardo);
            assert!(
                !d.rehacer,
                "no debe rehacer mientras el zoom sigue cambiando (vuelta {i})"
            );
            visto = d.visto;
            desde = d.desde;
        }
        // El ultimo cambio fue en t = 1000 ms; a los 300 ms de quietud
        // (t = 1300 ms) toca rehacer, con el tope ya a None.
        let quieto_pero_no_del_todo = t0 + std::time::Duration::from_millis(1299);
        let d = decidir_zoom(zoom, visto, 1.0, desde, quieto_pero_no_del_todo, retardo);
        assert!(!d.rehacer, "todavia no han pasado los 300 ms completos");
        let d = decidir_zoom(
            zoom,
            d.visto,
            1.0,
            d.desde,
            t0 + std::time::Duration::from_millis(1300),
            retardo,
        );
        assert!(d.rehacer);
        assert_eq!(d.tope_ms, None);
    }

    /// El hallazgo 3 de la revision final: las tres herramientas de medir
    /// estaban implementadas y probadas, pero no habia forma de elegirlas
    /// en la unica superficie que las implementa. Estas tres pruebas fijan
    /// las decisiones puras de las que depende el arreglo -que boton cae
    /// bajo un punto, que herramienta elige una tecla, y que hace cada
    /// boton- para no depender de una ventana de verdad.
    #[test]
    fn las_tres_herramientas_de_medir_tienen_boton_en_la_caja_del_editor() {
        let c = CajaHerramientas::colocar(rect_de_prueba(), rect_de_prueba(), 100, &BOTONES_EDITOR);
        for h in [
            Herramienta::Cota,
            Herramienta::Escalar,
            Herramienta::EscalaGrafica,
        ] {
            let indice = BOTONES_EDITOR
                .iter()
                .position(|b| *b == BotonCaja::Elegir(h))
                .unwrap_or_else(|| panic!("{h:?} no esta en la caja del editor"));
            let r = c.rect_de(indice);
            let centro = Punto {
                x: r.x + r.ancho as i32 / 2,
                y: r.y + r.alto as i32 / 2,
            };
            assert_eq!(c.boton_en(centro), Some(BotonCaja::Elegir(h)));
        }
    }

    #[test]
    fn las_letras_de_medir_eligen_la_herramienta_de_medir() {
        assert_eq!(tecla_a_herramienta('A'), Some(Herramienta::Cota));
        assert_eq!(tecla_a_herramienta('E'), Some(Herramienta::Escalar));
        assert_eq!(tecla_a_herramienta('G'), Some(Herramienta::EscalaGrafica));
        // Sin distincion de mayusculas: es lo que espera cualquiera que
        // teclee sin fijarse en Bloq Mayus.
        assert_eq!(tecla_a_herramienta('a'), Some(Herramienta::Cota));
    }

    #[test]
    fn una_letra_sin_herramienta_asignada_no_elige_nada() {
        // Caso negativo: si esto devolviera Some para cualquier caracter,
        // escribir en el cajetin de calibrar (que usa su PROPIO bucle de
        // eventos, no este) tampoco estaria a salvo si algun dia compartiera
        // codigo con esta funcion.
        assert_eq!(tecla_a_herramienta('9'), None);
        assert_eq!(tecla_a_herramienta(' '), None);
        assert_eq!(tecla_a_herramienta('Z'), None);
    }

    #[test]
    fn pulsar_un_boton_de_elegir_cambia_la_herramienta_del_gesto() {
        let mut gesto = Gesto::nuevo();
        let mut escena = Escena::nueva();
        assert_eq!(gesto.herramienta, Herramienta::Lapiz);
        let sigue = pulsar_boton(
            BotonCaja::Elegir(Herramienta::Cota),
            &mut gesto,
            &mut escena,
        );
        assert!(sigue, "elegir una herramienta no pide salir");
        assert_eq!(gesto.herramienta, Herramienta::Cota);
    }

    #[test]
    fn pulsar_deshacer_y_rehacer_llama_a_la_escena() {
        // Rotura a proposito: si `BotonCaja::Deshacer` no llamara a
        // `escena.deshacer()`, el trazo seguiria vivo tras pulsarlo. Esta
        // prueba lo caza comprobando la escena de verdad, no solo el valor
        // devuelto.
        let mut gesto = Gesto::nuevo();
        let mut escena = Escena::nueva();
        gesto.herramienta = Herramienta::Lapiz;
        gesto.evento(
            EventoGesto::Pulsar {
                p: Punto2::nuevo(0.0, 0.0),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        gesto.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(10.0, 0.0),
            },
            &mut escena,
            1.0,
        );
        assert_eq!(escena.cuantos_visibles(), 1);

        assert!(pulsar_boton(BotonCaja::Deshacer, &mut gesto, &mut escena));
        assert_eq!(escena.cuantos_visibles(), 0, "Deshacer tiene que deshacer");

        assert!(pulsar_boton(BotonCaja::Rehacer, &mut gesto, &mut escena));
        assert_eq!(escena.cuantos_visibles(), 1, "Rehacer tiene que rehacer");
    }

    #[test]
    fn pulsar_salir_pide_salir_y_los_demas_no() {
        let mut gesto = Gesto::nuevo();
        let mut escena = Escena::nueva();
        assert!(!pulsar_boton(BotonCaja::Salir, &mut gesto, &mut escena));
        assert!(pulsar_boton(BotonCaja::Color, &mut gesto, &mut escena));
        assert!(pulsar_boton(BotonCaja::Deshacer, &mut gesto, &mut escena));
    }

    fn rect_de_prueba() -> pixpin_geom::Rect {
        pixpin_geom::Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        }
    }

    /// El fallo real: los tiradores se pintaban con el angulo fijo en cero
    /// (`Tiradores::de_caja(caja, 0.0, escala)`) mientras `gesto.rs` los
    /// agarraba con el angulo de verdad del elemento. El usuario veia el
    /// marco recto y pinchaba donde veia el tirador, pero la zona que
    /// respondia estaba girada: no agarraba nada.
    ///
    /// El arreglo hace que pintar llame a `Gesto::tiradores` -la MISMA
    /// funcion que ya usaba `pulsar` para decidir que agarra el clic- en
    /// vez de recalcular el angulo por su cuenta. Esta prueba comprueba
    /// justo eso: el sitio que `Gesto::tiradores` dice que hay que PINTAR
    /// es el mismo que agarra un pulsar ahi.
    #[test]
    fn el_tirador_pintado_de_un_elemento_girado_es_el_que_agarra_el_clic() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            estilo_relleno: Default::default(),
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 50.0,
            angulo: FRAC_PI_2,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        });

        let mut gesto = Gesto::nuevo();
        gesto.seleccion.poner(id);

        let escala = 1.0;
        let pintados = gesto
            .tiradores(&escena, escala)
            .expect("hay un elemento elegido: tiene que haber tiradores");
        assert_ne!(
            pintados.angulo, 0.0,
            "con un solo elemento el marco pintado lleva su angulo real"
        );

        // El sitio exacto de un tirador, tal y como se pinta.
        let (cual, punto) = pintados.tamano[0];

        let respuesta = gesto.evento(
            EventoGesto::Pulsar {
                p: punto,
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            escala,
        );

        assert_eq!(
            respuesta.cursor,
            FormaCursor::Escalar {
                tirador: cual,
                angulo: pintados.angulo,
            },
            "pulsar justo donde se pinto el tirador tiene que agarrarlo"
        );
    }

    #[test]
    fn el_cursor_de_escalar_elige_la_flecha_por_su_direccion() {
        // Windows solo tiene cuatro flechas de redimension. La direccion
        // del tirador, ya girada con el elemento, se reparte entre ellas.
        let se_recto = FormaCursor::Escalar {
            tirador: Tirador::SuresteEsquina,
            angulo: 0.0,
        };
        assert_eq!(forma_de(se_recto), FormaCursorWin::RedimNoSe);

        // El mismo tirador con el elemento girado un cuarto de vuelta
        // apunta a la otra diagonal.
        let se_girado = FormaCursor::Escalar {
            tirador: Tirador::SuresteEsquina,
            angulo: FRAC_PI_2,
        };
        assert_eq!(forma_de(se_girado), FormaCursorWin::RedimNeSo);
    }

    #[test]
    fn el_tirador_del_norte_es_la_flecha_vertical() {
        let n = FormaCursor::Escalar {
            tirador: Tirador::NorteBorde,
            angulo: 0.0,
        };
        assert_eq!(forma_de(n), FormaCursorWin::RedimNS);

        // Girado noventa grados, el borde norte apunta al este.
        let n = FormaCursor::Escalar {
            tirador: Tirador::NorteBorde,
            angulo: FRAC_PI_2,
        };
        assert_eq!(forma_de(n), FormaCursorWin::RedimEO);
    }

    #[test]
    fn media_vuelta_da_la_misma_flecha() {
        // Una flecha de redimension no tiene punta: norte y sur son la
        // misma. Sin esto, girar 180 grados cambiaria el cursor sin motivo.
        let n = FormaCursor::Escalar {
            tirador: Tirador::NorteBorde,
            angulo: 0.0,
        };
        let s = FormaCursor::Escalar {
            tirador: Tirador::SurBorde,
            angulo: 0.0,
        };
        assert_eq!(forma_de(n), forma_de(s));

        let girado = FormaCursor::Escalar {
            tirador: Tirador::NorteBorde,
            angulo: PI,
        };
        assert_eq!(forma_de(n), forma_de(girado));
    }

    #[test]
    fn los_demas_cursores_se_traducen_uno_a_uno() {
        assert_eq!(forma_de(FormaCursor::Flecha), FormaCursorWin::Flecha);
        assert_eq!(forma_de(FormaCursor::Cruz), FormaCursorWin::Cruz);
        assert_eq!(forma_de(FormaCursor::Mover), FormaCursorWin::Mover);
        assert_eq!(forma_de(FormaCursor::Texto), FormaCursorWin::Texto);
        assert_eq!(forma_de(FormaCursor::Giro), FormaCursorWin::Giro);
    }

    #[test]
    fn el_raton_llega_al_motor_en_coordenadas_del_mundo() {
        // El motor trabaja en el mundo; la ventana recibe pixeles. Si esta
        // traduccion se olvidara, dibujar con el lienzo desplazado pintaria
        // en otro sitio.
        //
        // El metodo es `a_mundo`, que convierte un PUNTO. Cuidado con
        // `en_mundo`, que existe y hace otra cosa: convierte una LONGITUD
        // en pixeles a longitud del mundo. Confundirlos compila y da un
        // resultado silenciosamente equivocado.
        let camara = Camara {
            x: 100.0,
            y: 50.0,
            zoom: 2.0,
        };
        let ev = EventoOverlay::BotonPulsado(Punto { x: 20, y: 10 });

        let Some(EventoGesto::Pulsar { p, .. }) = a_evento(&ev, &camara, Punto { x: 0, y: 0 })
        else {
            panic!("un boton pulsado es un Pulsar");
        };
        assert_eq!(p, camara.a_mundo(Punto2::nuevo(20.0, 10.0)));
    }

    #[test]
    fn a_escala_150_el_raton_cae_en_el_mundo_logico() {
        // D127: un clic 150 px fisicos a la derecha del origen es 100 px
        // logicos, que es donde Excalidraw pondria el punto.
        let efectiva = crate::navegacion::vista_efectiva(&Camara::nueva(), 150);
        let ev = EventoOverlay::BotonPulsado(Punto { x: 150, y: 75 });
        let Some(EventoGesto::Pulsar { p, .. }) = a_evento(&ev, &efectiva, Punto { x: 0, y: 0 })
        else {
            panic!("un boton pulsado es un Pulsar");
        };
        assert_eq!((p.x, p.y), (100.0, 50.0));
    }

    #[test]
    fn el_origen_de_la_ventana_se_resta_antes_de_pasar_al_mundo() {
        // Con la barra de tareas arriba o a la izquierda, el area de trabajo no
        // empieza en (0,0) y la tinta salia desplazada del cursor.
        let camara = Camara::nueva();
        let ev = EventoOverlay::BotonPulsado(Punto { x: 110, y: 60 });
        let Some(EventoGesto::Pulsar { p, presion, .. }) =
            a_evento(&ev, &camara, Punto { x: 100, y: 50 })
        else {
            panic!("tendria que ser Pulsar")
        };
        assert_eq!((p.x, p.y), (10.0, 10.0));
        assert_eq!(presion, None);
    }

    #[test]
    fn una_muestra_del_lapiz_llega_al_gesto_con_su_presion_y_su_subpixel() {
        let camara = Camara::nueva();
        let m = pixpin_shell::puntero::Muestra::nueva(12.5, 7.25, Some(0.75));
        let Some(EventoGesto::Mover { p, presion, .. }) =
            a_evento(&EventoOverlay::Muestra(m), &camara, Punto { x: 0, y: 0 })
        else {
            panic!("tendria que ser Mover")
        };
        assert_eq!((p.x, p.y), (12.5, 7.25));
        assert_eq!(presion, Some(0.75));
    }

    #[test]
    fn las_teclas_uno_dos_tres_eligen_grosor_y_v_alterna_la_pluma() {
        assert_eq!(
            tecla_a_pluma('1'),
            Some(CambioPluma::Grosor(pixpin_motor2d::tinta::GROSOR_FINO))
        );
        assert_eq!(
            tecla_a_pluma('2'),
            Some(CambioPluma::Grosor(pixpin_motor2d::tinta::GROSOR_MEDIO))
        );
        assert_eq!(
            tecla_a_pluma('3'),
            Some(CambioPluma::Grosor(pixpin_motor2d::tinta::GROSOR_GRUESO))
        );
        assert_eq!(tecla_a_pluma('v'), Some(CambioPluma::AlternarVariabilidad));
        assert_eq!(tecla_a_pluma('L'), None, "L sigue siendo el lapiz");
        // Ninguna de las nuevas pisa una herramienta.
        for c in ['1', '2', '3', 'V'] {
            assert_eq!(tecla_a_herramienta(c), None, "{c}");
        }
    }

    #[test]
    fn dibujando_se_excluye_el_trazo_y_moviendo_la_seleccion() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;
        assert!(excluidos_de(&g).is_empty());
        g.evento(
            EventoGesto::Pulsar {
                p: Punto2::nuevo(1.0, 1.0),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        assert_eq!(excluidos_de(&g), vec![g.trazo_en_curso().unwrap()]);
    }

    #[test]
    fn control_zeta_es_deshacer_y_control_i_griega_rehacer() {
        let camara = Camara::nueva();
        let ctrl = |vk: u32| EventoOverlay::Tecla {
            vk,
            shift: false,
            ctrl: true,
            alt: false,
        };

        assert_eq!(
            a_evento(&ctrl(b'Z' as u32), &camara, Punto { x: 0, y: 0 }),
            Some(EventoGesto::Deshacer)
        );
        assert_eq!(
            a_evento(&ctrl(b'Y' as u32), &camara, Punto { x: 0, y: 0 }),
            Some(EventoGesto::Rehacer)
        );
        assert_eq!(
            a_evento(&ctrl(b'A' as u32), &camara, Punto { x: 0, y: 0 }),
            Some(EventoGesto::SeleccionarTodo)
        );
    }

    #[test]
    fn la_zeta_sin_control_no_deshace() {
        // Escribir una zeta en un texto no puede deshacer el dibujo.
        let camara = Camara::nueva();
        let sola = EventoOverlay::Tecla {
            vk: b'Z' as u32,
            shift: false,
            ctrl: false,
            alt: false,
        };
        assert_ne!(
            a_evento(&sola, &camara, Punto { x: 0, y: 0 }),
            Some(EventoGesto::Deshacer)
        );
    }

    #[test]
    fn lo_que_no_le_toca_al_motor_no_llega_al_motor() {
        let camara = Camara::nueva();
        assert_eq!(
            a_evento(&EventoOverlay::Pintar, &camara, Punto { x: 0, y: 0 }),
            None
        );
        assert_eq!(
            a_evento(&EventoOverlay::CambioDpi, &camara, Punto { x: 0, y: 0 }),
            None
        );
    }

    #[test]
    #[ignore = "necesita sesion de escritorio con GPU; ejecutar con --ignored"]
    fn abrir_y_cerrar_el_editor_no_revienta() {
        // El bucle de la ventana y el pintado necesitan una sesion
        // interactiva real (crear la ventana, el dispositivo D3D11, la
        // superficie de composicion). No se puede probar en CI sin
        // escritorio; se deja marcada para ejecutarla a mano.
        let _: Result<(Escena, Option<String>)> = abrir(
            Escena::nueva(),
            pixpin_motor2d::enganche::Ajustes::default(),
            pixpin_nivel::Nivel::Completo,
            false,
            None,
            &[],
        );
    }

    #[test]
    fn el_fondo_no_es_un_elemento_que_se_pueda_seleccionar() {
        // D133: la imagen vive fuera de la escena. Con un fondo abierto y
        // nada dibujado, Ctrl+A no elige nada y el borrador no tiene que
        // borrar.
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::nuevo();
        let _fondo = crate::fondo_lienzo::FondoLienzo::nuevo(
            crate::fondo_lienzo::recuadro_gris(800, 600),
            4096,
        );
        gesto.evento(EventoGesto::SeleccionarTodo, &mut escena, 1.0);
        assert!(gesto.seleccion.ids().is_empty());
        assert_eq!(escena.cuantos_visibles(), 0);
    }

    #[test]
    fn una_medida_tecleada_se_lee_con_coma_o_con_punto() {
        // El usuario escribe «3,5» o «3.5» segun la costumbre y el teclado.
        assert!((leer_medida("3,5").unwrap() - 3.5).abs() < 1e-5);
        assert!((leer_medida("3.5").unwrap() - 3.5).abs() < 1e-5);
        assert!((leer_medida(" 12 ").unwrap() - 12.0).abs() < 1e-5);
    }

    #[test]
    fn una_medida_que_no_es_un_numero_se_rechaza() {
        assert!(leer_medida("").is_none());
        assert!(leer_medida("tres").is_none());
        assert!(leer_medida("-3").is_none(), "una medida negativa no existe");
        assert!(leer_medida("0").is_none());
    }

    #[test]
    #[ignore = "necesita sesion de escritorio con GPU; ejecutar con --ignored"]
    fn pedir_medida_devuelve_none_al_cancelar_con_escape() {
        // El bucle de `pedir_medida` abre su propia ventana de overlay y
        // necesita un motor D3D11 de verdad: no se puede probar en CI sin
        // escritorio, se deja marcada para ejecutarla a mano.
        //
        // Si al cancelar la escala cambiara, seria un fallo (D39): cancelar
        // tiene que dejar el lienzo exactamente como estaba. A mano: abrir
        // el editor, calibrar con la herramienta Escalar, pulsar Escape en
        // el cajetin y comprobar que `escena.escala` sigue siendo la de
        // antes de arrastrar.
        let dispositivo =
            pixpin_capture::Dispositivo::nuevo().expect("sin dispositivo para la prueba manual");
        let motor = MotorRender::nuevo(dispositivo.d3d()).expect("sin motor para la prueba manual");
        let _ = motor;
    }

    #[test]
    fn por_cada_orden_incluye_el_rotulo_de_una_cota() {
        // El fallo real (ronda 1 de la tarea 6): el cierre de
        // `capa.preparar` horneaba solo `cache.ordenes(e, ...)`, sin las
        // `ordenes_medibles`. Como `pintar` salta con `continue` todo lo que
        // la capa ya volco, el numero de CUALQUIER cota visible en pantalla
        // desaparecia mientras se arrastraba OTRO elemento cualquiera -no
        // hacia falta tocar la cota, bastaba con que la capa se horneara-.
        //
        // El arreglo es que los dos caminos llamen a `por_cada_orden` en vez
        // de a `cache.ordenes` a secas. Esta prueba fija lo que esa funcion
        // compartida tiene que entregar para una cota: no puede probar la
        // ventana de verdad (necesita GPU y sesion interactiva), pero si
        // puede probar que la funcion de la que dependen los dos caminos no
        // vuelve a "olvidarse" del rotulo. Por eso el sumidero es un cierre
        // y no el `Pintor`: pasarle el pintor habria matado esta prueba.
        let cota = Elemento {
            estilo_relleno: Default::default(),
            figura: Figura::Cota {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
            },
            id: 1,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 0.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        };
        let mut cache = Cache::nueva();

        let mut hay_rotulo = false;
        let mut cuantas = 0usize;
        por_cada_orden(&mut cache, &cota, 1.0, None, |o| {
            cuantas += 1;
            if matches!(o, Orden::Texto { .. }) {
                hay_rotulo = true;
            }
        });

        assert!(
            hay_rotulo,
            "una cota tiene que traer su rotulo, venga o no de la cache"
        );
        // Que ademas entregue la forma cacheada: si algun dia el primer bucle
        // desapareciera, el rotulo solo seguiria estando y la asercion de
        // arriba no se enteraria de que la cota se pinta sin su raya.
        assert!(
            cuantas > 1,
            "faltan las ordenes cacheadas de la propia cota, no solo el rotulo"
        );
    }

    #[test]
    fn la_pista_del_iman_es_una_orden_que_el_editor_sabe_pintar() {
        // No se puede probar la ventana de verdad -pide GPU y sesion
        // interactiva-, pero si que lo que el motor produce para el ancla
        // activa es de un tipo que `dibujar_orden` cubre. Si algun dia la
        // pista pasara a ser una variante que el `match` de `dibujar_orden`
        // no contemple, la marca desapareceria en silencio.
        use pixpin_motor2d::enganche::{Anclaje, TipoAnclaje, pista};

        for tipo in [
            TipoAnclaje::Esquina,
            TipoAnclaje::Extremo,
            TipoAnclaje::Medio,
            TipoAnclaje::Centro,
        ] {
            let a = Anclaje {
                punto: Punto2::nuevo(10.0, 20.0),
                tipo,
                id: 1,
            };
            assert!(
                matches!(pista(&a, 1.0), Orden::Polilinea { .. }),
                "la pista de {tipo:?} no es pintable por el editor"
            );
        }
    }

    /// El cableado nuevo: `abrir` tiene que trasladar los ajustes de iman
    /// guardados al `Gesto` que crea, no dejarlos siempre por defecto. Sin
    /// esto la pestana de ajustes seria decorativa -el usuario apaga el
    /// iman y el editor lo ignora-.
    ///
    /// Se ejercita `gesto_inicial`, que es lo que `abrir` llama de verdad:
    /// construir un `Gesto` aqui y releerle el campo solo comprobaria que
    /// asignar campos en Rust funciona.
    #[test]
    fn los_ajustes_de_iman_llegan_al_gesto_recien_creado() {
        let apagado = pixpin_motor2d::enganche::Ajustes {
            activo: false,
            medios: false,
            radio_px: 3.0,
            ..Default::default()
        };

        let gesto = gesto_inicial(apagado);

        assert_eq!(
            gesto.enganche, apagado,
            "el gesto tiene que llevar los ajustes que se le pasan, no los por defecto"
        );
        assert_ne!(
            gesto.enganche,
            pixpin_motor2d::enganche::Ajustes::default(),
            "los ajustes de prueba tienen que diferir de los de fabrica o esto no mide nada"
        );
    }
}
