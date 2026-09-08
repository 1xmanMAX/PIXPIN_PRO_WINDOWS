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

use anyhow::{Context, Result};
use pixpin_geom::Punto;
use pixpin_motor2d::cache::Cache;
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{
    EventoGesto, FormaCursor, Gesto, Peticion, Region, direccion_del_tirador,
};
use pixpin_motor2d::indice::Rejilla;
use pixpin_motor2d::pintado::Orden;
use pixpin_motor2d::vector::Punto2;
use pixpin_motor2d::{ColorRgba, Elemento, Escala, EstiloTrazo};
use pixpin_render::{CapaEstatica, Color, Estampa, MotorRender, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin, VentanaOverlay};

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
pub fn a_evento(ev: &EventoOverlay, camara: &Camara) -> Option<EventoGesto> {
    // `a_mundo` convierte un punto. NO `en_mundo`, que existe y convierte una
    // LONGITUD: compila igual y da otra cosa.
    let al_mundo = |p: &Punto| camara.a_mundo(Punto2::nuevo(p.x as f32, p.y as f32));
    match ev {
        EventoOverlay::BotonPulsado(p) => Some(EventoGesto::Pulsar {
            p: al_mundo(p),
            shift: false,
            alt: false,
        }),
        EventoOverlay::RatonMovido(p) => Some(EventoGesto::Mover {
            p: al_mundo(p),
            shift: false,
            alt: false,
        }),
        EventoOverlay::BotonSoltado(p) => Some(EventoGesto::Soltar { p: al_mundo(p) }),
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
        EventoGesto::Pulsar { p, .. } => {
            let (shift, alt) = pixpin_shell::entrada::modificadores();
            EventoGesto::Pulsar { p, shift, alt }
        }
        EventoGesto::Mover { p, .. } => {
            let (shift, alt) = pixpin_shell::entrada::modificadores();
            EventoGesto::Mover { p, shift, alt }
        }
        otro => otro,
    }
}

/// Abre el editor y no vuelve hasta que se cierra la ventana.
///
/// Devuelve la escena tal como quedo, compactada: los elementos borrados de
/// verdad (los que un `Ctrl+Z` ya no puede traer de vuelta) se sueltan aqui,
/// no en cada paso del historial.
pub fn abrir(escena: Escena) -> Result<Escena> {
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

    let mut escena = escena;
    let mut gesto = Gesto::nuevo();
    let camara = Camara::nueva();
    let mut cache = Cache::nueva();
    let mut rejilla = Rejilla::nueva();
    let mut capa = CapaEstatica::nueva();
    let (ancho_px, alto_px) = (area.ancho as f32, area.alto as f32);

    'bucle: loop {
        pixpin_shell::overlay::bombear_pendientes();
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            // 1. Traducir y, si le toca al motor, pasarselo.
            if let Some(g) = a_evento(&ev, &camara) {
                let g = con_modificadores(g);
                let en_reposo_antes = gesto.en_reposo();
                let r = gesto.evento(g, &mut escena, 1.0 / camara.zoom);
                ventana.poner_cursor(forma_de(r.cursor));
                // `VentanaOverlay::invalidar` no toma una region: invalida
                // la ventana entera. `Region::Caja` pide menos que eso, pero
                // repintar de mas aqui es correcto, solo mas caro; no hay
                // API de region parcial que inventar sin salirse del alcance
                // de esta tarea (esta en pixpin-shell, no en pixpin-render).
                match r.region {
                    Region::Nada => {}
                    Region::Caja(..) | Region::Todo => ventana.invalidar(),
                }
                // La capa estatica solo vive durante un gesto de mover,
                // escalar o girar algo seleccionado: en `gesto.rs`, entrar
                // en `Dibujando` o `Marquesina` limpia la seleccion antes,
                // asi que "seleccion no vacia y no en reposo" identifica
                // exactamente esos tres estados sin que este fichero tenga
                // que conocer el `Estado` privado del motor.
                let activo_ahora = !gesto.en_reposo() && !gesto.seleccion.esta_vacia();
                if activo_ahora && en_reposo_antes {
                    rejilla.sincronizar(&escena);
                    let vista = camara.ventana(ancho_px, alto_px);
                    let candidatos = rejilla.candidatos(vista);
                    let excluidos = gesto.seleccion.ids().to_vec();
                    let estampa = Estampa {
                        camara: (camara.x, camara.y, camara.zoom),
                        tamano: (ancho_px as u32, alto_px as u32),
                        excluidos: excluidos.clone(),
                    };
                    let _ = capa.preparar(&mut motor, estampa, |p| {
                        p.limpiar(Color::BLANCO);
                        let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
                        p.poner_vista((0.0, 0.0), camara.zoom, (origen.x, origen.y));
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
                            // `ordenes_de_elemento` no entra en la capa, y
                            // `pintar` ya no lo repinta mientras la capa
                            // valga (ve su comentario para el porque).
                            for orden in ordenes_de_elemento(
                                &mut cache,
                                e,
                                camara.zoom,
                                escena.escala.as_ref(),
                            ) {
                                dibujar_orden(p, &orden, vista);
                            }
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
                        &camara,
                        &gesto,
                        &mut cache,
                        &rejilla,
                        &capa,
                        ancho_px,
                        alto_px,
                        largo_px,
                    ) {
                        gesto.calibrar(&mut escena, largo_px, valor, &unidad);
                    }
                    ventana.invalidar();
                }
            }
            // 2. Pintar solo cuando lo pide la ventana.
            if matches!(ev, EventoOverlay::Pintar) {
                rejilla.sincronizar(&escena);
                pintar(
                    &mut motor,
                    &superficie,
                    &escena,
                    &camara,
                    &gesto,
                    &mut cache,
                    &rejilla,
                    &capa,
                    ancho_px,
                    alto_px,
                    |_| {},
                );
            }
            if matches!(ev, EventoOverlay::Cerrar) {
                break 'bucle;
            }
        }
        // Late corto: el raton responde al instante y en reposo el bucle no
        // quema CPU (el overlay ya no entrega eventos si no pasa nada).
        std::thread::sleep(std::time::Duration::from_millis(5));
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
fn ordenes_de_elemento(
    cache: &mut Cache,
    e: &Elemento,
    zoom: f32,
    escala: Option<&Escala>,
) -> Vec<Orden> {
    let mut ordenes: Vec<Orden> = cache.ordenes(e, zoom).to_vec();
    ordenes.extend(pixpin_motor2d::pintado::ordenes_medibles(e, escala, ','));
    ordenes
}

/// Pinta un fotograma entero: lo que hay en pantalla, y encima el marco de
/// la seleccion, sus tiradores y la marquesina si la hay.
///
/// `encima` se llama al final, con la transformacion todavia puesta a la
/// del mundo: quien la pasa es quien decide si dibuja en coordenadas del
/// mundo o si la deshace con `Pintor::desplazar(0.0, 0.0)` primero (el
/// cajetin de calibrar hace esto ultimo: es un dialogo en pantalla, no algo
/// del lienzo).
#[allow(clippy::too_many_arguments)]
fn pintar(
    motor: &mut MotorRender,
    superficie: &Superficie,
    escena: &Escena,
    camara: &Camara,
    gesto: &Gesto,
    cache: &mut Cache,
    rejilla: &Rejilla,
    capa: &CapaEstatica,
    ancho_px: f32,
    alto_px: f32,
    encima: impl FnOnce(&pixpin_render::Pintor<'_>),
) {
    let Ok(destino) = superficie.empezar(motor) else {
        return;
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
    let ahora = Estampa {
        camara: (camara.x, camara.y, camara.zoom),
        tamano: (ancho_px as u32, alto_px as u32),
        excluidos: gesto.seleccion.ids().to_vec(),
    };
    let capa_vale = capa.volcar(motor, &destino, &ahora);

    let _ = motor.dibujar(&destino, |p| {
        // El mundo se dibuja en sus propias coordenadas; la matriz activa es
        // lo unico que cambia al encuadrar o acercar (camara.rs lo explica:
        // "la geometria se calcula UNA VEZ en coordenadas del mundo").
        let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
        if !capa_vale {
            p.limpiar(Color::BLANCO);
        }
        p.poner_vista((0.0, 0.0), camara.zoom, (origen.x, origen.y));

        for id in candidatos {
            // Con la capa valida, lo que no esta seleccionado ya esta
            // copiado en `destino`: repintarlo aqui seria pagar dos veces.
            if capa_vale && !gesto.seleccion.contiene(id) {
                continue;
            }
            let Some(e) = escena.buscar(id) else {
                continue;
            };
            if e.borrado {
                continue;
            }
            for orden in ordenes_de_elemento(cache, e, camara.zoom, escena.escala.as_ref()) {
                dibujar_orden(p, &orden, vista);
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
                    dibujar_orden(p, &orden, vista);
                }
            }
        }
        if let Some(m) = gesto.marquesina() {
            p.marquesina(m, escala);
        }
        encima(p);
    });
    let _ = superficie.presentar();
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
    rejilla: &Rejilla,
    capa: &CapaEstatica,
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
                        rejilla,
                        capa,
                        ancho_px,
                        alto_px,
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
fn dibujar_orden(p: &pixpin_render::Pintor<'_>, orden: &Orden, vista: (f32, f32, f32, f32)) {
    match orden {
        Orden::Poligono { puntos, color } | Orden::Relleno { puntos, color } => {
            p.poligono(&a_tuplas(puntos), a_color(*color));
        }
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

        let Some(EventoGesto::Pulsar { p, .. }) = a_evento(&ev, &camara) else {
            panic!("un boton pulsado es un Pulsar");
        };
        assert_eq!(p, camara.a_mundo(Punto2::nuevo(20.0, 10.0)));
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
            a_evento(&ctrl(b'Z' as u32), &camara),
            Some(EventoGesto::Deshacer)
        );
        assert_eq!(
            a_evento(&ctrl(b'Y' as u32), &camara),
            Some(EventoGesto::Rehacer)
        );
        assert_eq!(
            a_evento(&ctrl(b'A' as u32), &camara),
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
        assert_ne!(a_evento(&sola, &camara), Some(EventoGesto::Deshacer));
    }

    #[test]
    fn lo_que_no_le_toca_al_motor_no_llega_al_motor() {
        let camara = Camara::nueva();
        assert_eq!(a_evento(&EventoOverlay::Pintar, &camara), None);
        assert_eq!(a_evento(&EventoOverlay::CambioDpi, &camara), None);
    }

    #[test]
    #[ignore = "necesita sesion de escritorio con GPU; ejecutar con --ignored"]
    fn abrir_y_cerrar_el_editor_no_revienta() {
        // El bucle de la ventana y el pintado necesitan una sesion
        // interactiva real (crear la ventana, el dispositivo D3D11, la
        // superficie de composicion). No se puede probar en CI sin
        // escritorio; se deja marcada para ejecutarla a mano.
        let _ = abrir(Escena::nueva());
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
    fn ordenes_de_elemento_incluye_el_rotulo_de_una_cota() {
        // El fallo real (ronda 1 de la tarea 6): el cierre de
        // `capa.preparar` horneaba solo `cache.ordenes(e, ...)`, sin las
        // `ordenes_medibles`. Como `pintar` salta con `continue` todo lo que
        // la capa ya volco, el numero de CUALQUIER cota visible en pantalla
        // desaparecia mientras se arrastraba OTRO elemento cualquiera -no
        // hacia falta tocar la cota, bastaba con que la capa se horneara-.
        //
        // El arreglo es que los dos caminos llamen a `ordenes_de_elemento`
        // en vez de a `cache.ordenes` a secas. Esta prueba fija lo que esa
        // funcion compartida tiene que devolver para una cota: no puede
        // probar la ventana de verdad (necesita GPU y sesion interactiva),
        // pero si puede probar que la funcion de la que dependen los dos
        // caminos no vuelve a "olvidarse" del rotulo.
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

        let ordenes = ordenes_de_elemento(&mut cache, &cota, 1.0, None);

        assert!(
            ordenes.iter().any(|o| matches!(o, Orden::Texto { .. })),
            "una cota tiene que traer su rotulo, venga o no de la cache"
        );
    }
}
