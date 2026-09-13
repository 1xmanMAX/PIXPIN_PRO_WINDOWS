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

use crate::navegacion::vista_efectiva;
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
use pixpin_motor2d::{ColorRgba, Elemento, Escala, EstiloTrazo};
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
        _ => None,
    }
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
            gesto.herramienta = h;
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
    match gesto.trazo_en_curso() {
        Some(id) => vec![id],
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
pub fn abrir(
    escena: Escena,
    ajustes_iman: pixpin_motor2d::enganche::Ajustes,
    nivel: pixpin_nivel::Nivel,
) -> Result<Escena> {
    let dispositivo =
        pixpin_capture::Dispositivo::nuevo().context("sin dispositivo para el editor")?;
    let mut motor = MotorRender::nuevo(dispositivo.d3d()).context("sin motor de dibujo")?;

    let disposicion =
        pixpin_capture::enumerar_monitores().context("sin monitores para el editor")?;
    let monitor = disposicion
        .principal()
        .context("sin monitor principal para el editor")?;
    let area = monitor.area_trabajo;

    let ventana = VentanaOverlay::nueva(area).context("no se pudo abrir el editor")?;
    let superficie = Superficie::nueva(
        &motor,
        dispositivo.d3d(),
        ventana.handle(),
        area.ancho,
        area.alto,
    )
    .context("sin superficie para el editor")?;
    ventana.mostrar();
    ventana.enfocar();
    ventana.pedir_entrada_fina();

    let mut escena = escena;
    let mut gesto = gesto_inicial(ajustes_iman);
    let camara = Camara::nueva();
    // D127: la camara del usuario va en pixeles logicos; `efectiva` es la
    // que pinta y traduce el raton. Se recalcula si cambia la escala.
    let mut escala_por_cien = monitor.escala_por_cien;
    let mut efectiva = vista_efectiva(&camara, escala_por_cien);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    let retardo = pixpin_render::retardo_nitido(nivel);
    // El ultimo zoom visto y desde cuando esta ahi, para `decidir_zoom`. La
    // camara del editor aun no hace zoom (E4), pero el mecanismo queda
    // puesto y probado.
    let mut zoom_visto = efectiva.zoom;
    let mut zoom_desde: Option<std::time::Instant> = None;
    let mut rejilla = Rejilla::nueva();
    let mut capa = CapaEstatica::nueva();
    let (ancho_px, alto_px) = (area.ancho as f32, area.alto as f32);
    // "Contenido" y area de trabajo son el mismo rectangulo, como en
    // `CapaViva::nueva` (`capa.rs`): aqui el contenido ES la pantalla
    // entera, no hay un pin ni una ventana mas pequena de referencia.
    let mut caja = CajaHerramientas::colocar(area, area, escala_por_cien, &BOTONES_EDITOR);

    // Zona sucia acumulada durante la vuelta: se pinta un solo fotograma
    // DESPUES de vaciar la cola de eventos, no uno por evento (pintar a
    // mitad de la cola era lo que hacia perder puntos del lapiz).
    let mut hay_que_pintar = false;
    let mut sucio: Option<(f32, f32, f32, f32)> = None;
    let mut todo_sucio = false;

    'bucle: loop {
        pixpin_shell::overlay::bombear_pendientes();
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
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
                    caja = CajaHerramientas::colocar(area, area, escala_por_cien, &BOTONES_EDITOR);
                    // La capa congelada se horneo a la escala vieja.
                    capa.soltar();
                    todo_sucio = true;
                    ventana.invalidar();
                }
                continue;
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
            if let EventoOverlay::BotonPulsado(p) = ev {
                match caja.destino(p) {
                    DestinoClic::Boton(boton) => {
                        if !pulsar_boton(boton, &mut gesto, &mut escena) {
                            break 'bucle;
                        }
                        // El cursor se pone al vuelo con el siguiente
                        // `RatonMovido`: no hace falta calcularlo aqui, y
                        // `cursor_en` es privado de `gesto.rs` a proposito.
                        ventana.invalidar();
                        todo_sucio = true;
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
            if let EventoOverlay::Caracter(c) = ev {
                if let Some(h) = tecla_a_herramienta(c) {
                    gesto.herramienta = h;
                    ventana.invalidar();
                    todo_sucio = true;
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
            // 1. Traducir y, si le toca al motor, pasarselo.
            if let Some(g) = a_evento(
                &ev,
                &efectiva,
                Punto {
                    x: area.x,
                    y: area.y,
                },
            ) {
                let g = con_modificadores(g);
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
                        ventana.invalidar();
                    }
                }
                // La capa estatica vive mientras algo cambia en cada
                // fotograma: mover, escalar o girar la seleccion (como
                // antes) y, desde E1, dibujar a mano (D120). Lo excluido se
                // repinta encima; lo demas se copia de la capa.
                let excluidos = excluidos_de(&gesto);
                let activo_ahora = !gesto.en_reposo() && !excluidos.is_empty();
                if activo_ahora && en_reposo_antes {
                    rejilla.sincronizar(&escena);
                    let vista = efectiva.ventana(ancho_px, alto_px);
                    let candidatos = rejilla.candidatos(vista);
                    let estampa = Estampa {
                        camara: (efectiva.x, efectiva.y, efectiva.zoom),
                        tamano: (ancho_px as u32, alto_px as u32),
                        excluidos: excluidos.clone(),
                    };
                    let _ = capa.preparar(&mut motor, estampa, |p| {
                        p.limpiar(Color::BLANCO);
                        let origen = efectiva.a_pantalla(Punto2::nuevo(0.0, 0.0));
                        p.poner_vista((0.0, 0.0), efectiva.zoom, (origen.x, origen.y));
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
        // Un solo fotograma por vuelta, despues de haber pasado TODOS los
        // puntos al gesto: pintar a mitad de la cola era lo que hacia perder
        // puntos. Presentar con vsync bloquea hasta el refresco; mientras,
        // Windows guarda los movimientos y la Tarea 7 los recupera.
        if hay_que_pintar {
            rejilla.sincronizar(&escena);
            // La decision de si la capa congelada vale para ESTE fotograma
            // (`capa_vale`, dentro de `pintar`) es la que manda sobre si la
            // zona parcial es segura: `capa.lista()` solo dice que hay una
            // capa horneada, no que `volcar` la vaya a usar ahora mismo.
            let zona = if todo_sucio {
                None
            } else {
                sucio.map(|(x0, y0, x1, y1)| {
                    (
                        x0.floor() as i32 - 2,
                        y0.floor() as i32 - 2,
                        x1.ceil() as i32 + 2,
                        y1.ceil() as i32 + 2,
                    )
                })
            };
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
                &caja,
                escala_por_cien,
                ancho_px,
                alto_px,
                zona,
                |_| {},
            );
            if pintado {
                hay_que_pintar = false;
                sucio = None;
                todo_sucio = false;
            } else {
                // `superficie.empezar` fallo: el fotograma se salto. La
                // zona sucia acumulada NO se puede dar por pintada -si se
                // limpiara aqui, el proximo present parcial se quedaria sin
                // el trozo que este fotograma no llego a cubrir-, asi que
                // se fuerza el fotograma que si llegue a ser completo.
                todo_sucio = true;
            }
        }
        // Zoom quieto durante `retardo`: se rehace la tinta nitida a esa
        // escala y se suelta la capa (sus copias venian de la escala vieja).
        // La decision (y el tope de espera de mas abajo) sale de
        // `decidir_zoom`, pura: aqui solo se aplican sus efectos. La camara
        // del editor aun no hace zoom (E4); el mecanismo queda puesto y
        // probado para cuando lo haga.
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
        pixpin_shell::overlay::esperar_eventos(decision.tope_ms);
    }

    ventana.ocultar();
    escena.compactar();
    Ok(escena)
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
/// Devuelve si de verdad pinto y presento un fotograma. `false` solo pasa si
/// `superficie.empezar` fallo (el backbuffer no estaba listo): quien llama
/// no puede dar la zona sucia por cubierta ni la pantalla por al dia si esto
/// devuelve `false`, o el proximo present parcial se dejaria un trozo sin
/// pintar creyendo que ya se habia pintado en este fotograma que se saltó.
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
    caja_herramientas: &CajaHerramientas,
    escala_por_cien: u32,
    ancho_px: f32,
    alto_px: f32,
    zona: Option<(i32, i32, i32, i32)>,
    encima: impl FnOnce(&pixpin_render::Pintor<'_>),
) -> bool {
    let Ok(destino) = superficie.empezar(motor) else {
        return false;
    };
    // La rejilla dice que PUEDE verse; la camara filtra lo que de verdad se
    // ve. Sin la rejilla, esto recorreria los ocho mil elementos.
    let vista = camara.ventana(ancho_px, alto_px);
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
        tamano: (ancho_px as u32, alto_px as u32),
        excluidos: ahora_excluidos.clone(),
    };
    let capa_vale = capa.volcar(motor, &destino, &ahora);
    // Lo que de verdad importa para presentar solo un trozo es si la capa
    // congelada SE USO en este fotograma (`capa_vale`), no si `capa.lista()`
    // dice que hay una capa horneada: si no vale, `destino` se limpia y se
    // pinta entero mas abajo, y presentar con una zona pequena dejaria el
    // resto de la pantalla con lo que hubiera antes.
    let zona = if capa_vale { zona } else { None };

    let error = motor.dibujar(&destino, |p| {
        // El mundo se dibuja en sus propias coordenadas; la matriz activa es
        // lo unico que cambia al encuadrar o acercar (camara.rs lo explica:
        // "la geometria se calcula UNA VEZ en coordenadas del mundo").
        let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
        if !capa_vale {
            p.limpiar(Color::BLANCO);
        }
        p.poner_vista((0.0, 0.0), camara.zoom, (origen.x, origen.y));

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
            // Lo excluido es lo que se arrastra o el trazo en curso: su
            // version sube en CADA fotograma, asi que cachear su tinta
            // seria teselar de nuevo cada vez sin acertar nunca -pagar la
            // realizacion sin cobrar el ahorro-. Se pinta sin cache, como
            // antes de esta tarea.
            let mut indice = 0u32;
            let clave_tinta = !ahora_excluidos.contains(&e.id);
            por_cada_orden(cache, e, camara.zoom, escena.escala.as_ref(), |orden| {
                let tinta = clave_tinta.then_some((&mut *cache_tinta, (e.id, e.version, indice)));
                dibujar_orden(p, orden, vista, tinta);
                indice += 1;
            });
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
                    dibujar_orden(p, &orden, vista, None);
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
            );
        }
        // La caja es un dialogo en pantalla, no algo del lienzo: no se mueve
        // ni se escala con la camara. `desplazar(0.0, 0.0)` deshace la vista
        // del mundo que `poner_vista` dejo puesta arriba, igual que hace
        // `dibujar_cajetin` mas abajo.
        p.desplazar(0.0, 0.0);
        crate::caja_dibujo::pintar_caja(
            p,
            caja_herramientas,
            gesto.herramienta,
            escala_por_cien,
            Punto { x: 0, y: 0 },
        );
        encima(p);
    });
    if error.is_err() {
        // Dispositivo perdido: las realizaciones de tinta son del dispositivo
        // viejo y ya no valen (D2D las rechazaria en el siguiente fotograma).
        cache_tinta.vaciar();
    }
    let _ = superficie.presentar_sincronizado(zona);
    true
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
                        caja,
                        escala_por_cien,
                        ancho_px,
                        alto_px,
                        None,
                        |p| dibujar_cajetin(p, ancho_px, alto_px, largo_px, &texto, unidad),
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
    ancho_px: f32,
    alto_px: f32,
    largo_px: f32,
    texto: &str,
    unidad: &str,
) {
    p.desplazar(0.0, 0.0);

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
fn dibujar_orden(
    p: &pixpin_render::Pintor<'_>,
    orden: &Orden,
    vista: (f32, f32, f32, f32),
    tinta: Option<(&mut pixpin_render::CacheTinta, (u64, u32, u32))>,
) {
    match orden {
        Orden::Poligono { puntos, color } | Orden::Relleno { puntos, color } => {
            p.poligono(&a_tuplas(puntos), a_color(*color));
        }
        Orden::Tinta { contorno, color } => match tinta {
            Some((c, clave)) => {
                // Acierto: se pinta con la realizacion ya cacheada sin
                // volver a convertir `contorno` en `Vec<(f32, f32)>` -esa
                // reserva es la que se pagaba cada fotograma sin usarla
                // para nada en cuanto habia acierto de cache-. Solo si
                // `pintar_realizada` no encuentra nada (o falla) se paga la
                // conversion y se rehace.
                if !p.pintar_realizada(c, clave, a_color(*color)) {
                    p.tinta_cacheada(c, clave, &a_tuplas(contorno), a_color(*color));
                }
            }
            None => p.tinta(&a_tuplas(contorno), a_color(*color)),
        },
        Orden::Polilinea {
            puntos,
            color,
            grosor,
            estilo,
        } => {
            let v = a_tuplas(puntos);
            match estilo {
                EstiloTrazo::Solido => p.polilinea(&v, *grosor, a_color(*color)),
                // `Pintor` todavia no distingue rayas de puntos (nadie mas
                // en el proyecto dibuja punteado con Direct2D); a rayas es
                // la aproximacion mas cercana a lo discontinuo.
                EstiloTrazo::Discontinuo | EstiloTrazo::Punteado => {
                    p.polilinea_discontinua(&v, *grosor, a_color(*color))
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
        Orden::Imagen { .. } => {
            // Resolver el bitmap por `id_objeto` es tarea futura: nada de lo
            // que se puede dibujar en esta entrega produce `Figura::Imagen`.
        }
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

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::Tirador;
    use pixpin_motor2d::camara::Camara;
    use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
    use pixpin_motor2d::escena::Escena;
    use pixpin_motor2d::gesto::{EventoGesto, Gesto};
    use std::f32::consts::{FRAC_PI_2, PI};

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
        let _ = abrir(
            Escena::nueva(),
            pixpin_motor2d::enganche::Ajustes::default(),
            pixpin_nivel::Nivel::Completo,
        );
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
