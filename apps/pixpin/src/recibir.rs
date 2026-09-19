//! Recibir del movil por la wifi: el ordenador espera y ensena su codigo.
//!
//! Es el sentido invertido del envio puntual de PixPin Android (D147): alli
//! espera quien envia y aqui espera quien recibe, que es lo que hace falta
//! para traerse cosas al ordenador sin teclear nada en el. El movil escanea
//! el QR —que lleva dentro el codigo, la IP y el puerto—, llama, y como
//! **el canal lo empieza quien llama**, aqui se saluda con `inicia = false`.
//!
//! Lo que llega se guarda en `recibidos/` y se le pasa a la ventana de
//! mensajes, que es quien sabe abrir un `.pixpin` como proyecto y lo demas
//! como pin. Asi esto no duplica ninguna de esas dos decisiones.

use anyhow::{Context, Result};
use pixpin_geom::Rect;
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Ubicacion};

use crate::caja_dibujo::hex;
use crate::overlay::Recursos;

/// Lo que mide la ventana en pixeles logicos. Pequena a proposito: es un
/// cartel con un codigo, no una pantalla de trabajo.
const ANCHO_LOGICO: u32 = 420;
const ALTO_LOGICO: u32 = 560;

const FONDO: Color = hex(0x1f2023);
const TEXTO: Color = hex(0xf2f2f2);
const APAGADO: Color = hex(0x9a9a9e);
const PAPEL: Color = hex(0xffffff);
const TINTA: Color = hex(0x000000);
const ROJO: Color = hex(0xe06c75);

/// El codigo de seis cifras de este envio, sacado del azar del sistema.
///
/// Si el azar del sistema falla no se inventa uno peor: sin codigo no hay
/// envio, y uno adivinable es justo lo que no puede ser.
fn codigo_nuevo() -> Option<String> {
    let mut bytes = [0u8; pixpin_sincro::envio::CIFRAS];
    if !pixpin_shell::azar(&mut bytes) {
        return None;
    }
    Some(bytes.iter().map(|b| (b % 10).to_string()).collect())
}

/// La IP de este equipo en la red local.
///
/// Se pregunta abriendo un socket UDP «hacia» una direccion de fuera: no
/// manda ni un byte —UDP no conecta de verdad—, pero obliga al sistema a
/// elegir la tarjeta por la que saldria, que es justo la que el movil tiene
/// que ver. Enumerar tarjetas daria varias y habria que adivinar cual.
fn ip_local() -> Option<String> {
    let s = std::net::UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    s.connect(("8.8.8.8", 53)).ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

/// Abre la ventana de recibir en su propio hilo, como el chat: el hilo
/// principal tiene que seguir atendiendo atajos y gestos mientras espera.
pub fn lanzar(idioma: pixpin_store::Idioma, ubicacion: Ubicacion) {
    let lanzado = std::thread::Builder::new()
        .name("recibir".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            let hecho = Recursos::nuevos().and_then(|r| abrir(&r, &textos, &ubicacion));
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la ventana de recibir");
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de recibir");
    }
}

/// En que anda el envio. Es lo unico que cambia en la ventana.
enum Estado {
    Esperando,
    Hecho { cuantos: usize },
    Fallo { porque: String },
}

pub fn abrir(recursos: &Recursos, textos: &Catalogo, ubicacion: &Ubicacion) -> Result<()> {
    let Some(codigo) = codigo_nuevo() else {
        anyhow::bail!("el sistema no dio azar para el codigo");
    };
    // El puerto de siempre si esta libre; si no, cualquiera: el movil no lo
    // da por fijo porque lo lee del QR.
    let escucha = std::net::TcpListener::bind(("0.0.0.0", pixpin_sincro::PUERTO))
        .or_else(|_| std::net::TcpListener::bind(("0.0.0.0", 0)))
        .context("no se pudo escuchar en la red")?;
    escucha
        .set_nonblocking(true)
        .context("la escucha tiene que poder mirarse sin bloquear")?;
    let puerto = escucha.local_addr().map(|d| d.port()).unwrap_or_default();
    let ip = ip_local().unwrap_or_default();
    let qr = qrcodegen::QrCode::encode_text(
        &pixpin_sincro::envio::texto_del_qr_de_recepcion(&codigo, &ip, puerto),
        qrcodegen::QrCodeEcc::Medium,
    )
    .ok();
    tracing::info!(%ip, puerto, "esperando un envio del movil");

    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .principal()
        .context("sin monitor principal")?
        .to_owned();
    let escala = monitor.escala_por_cien;
    let e = |v: u32| v * escala / 100;
    let marco = Rect {
        x: monitor.area.x + (monitor.area.ancho as i32 - e(ANCHO_LOGICO) as i32) / 2,
        y: monitor.area.y + (monitor.area.alto as i32 - e(ALTO_LOGICO) as i32) / 2,
        ancho: e(ANCHO_LOGICO),
        alto: e(ALTO_LOGICO),
    };
    let ventana = VentanaOverlay::nueva_normal(marco, &textos.t("recibir-titulo"))
        .context("no se pudo abrir la ventana de recibir")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para recibir")?;
    ventana.mostrar();
    ventana.enfocar();

    let nombre = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PixPin Max".into());
    let id = pixpin_proyecto::identidad::Identidad::leer_o_crear(ubicacion.raiz(), &nombre)
        .map(|i| i.yo.id)
        .unwrap_or_default();
    let carpeta = ubicacion.raiz().join("recibidos");

    let mut estado = Estado::Esperando;
    let mut hay_que_pintar = true;
    loop {
        pixpin_shell::overlay::bombear_pendientes();
        let mut cerrar = false;
        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match evento {
                EventoOverlay::Cerrar => cerrar = true,
                // Escape cierra, como cualquier cartel.
                EventoOverlay::Tecla { vk: 0x1B, .. } => cerrar = true,
                EventoOverlay::Pintar => hay_que_pintar = true,
                _ => {}
            }
        }
        if cerrar {
            break;
        }

        // Un envio por ventana: al terminar uno, ese codigo ya no vale.
        if matches!(estado, Estado::Esperando) {
            match escucha.accept() {
                Ok((flujo, de)) => {
                    tracing::info!(%de, "llamada del movil");
                    let _ = flujo.set_nonblocking(false);
                    estado = atender(flujo, &codigo, &nombre, &id, &carpeta, ubicacion.raiz());
                    hay_que_pintar = true;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    estado = Estado::Fallo {
                        porque: e.to_string(),
                    };
                    hay_que_pintar = true;
                }
            }
        }

        if hay_que_pintar {
            hay_que_pintar = false;
            if let Ok(destino) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&destino, |p: &Pintor| {
                    pintar(
                        p,
                        marco,
                        escala,
                        textos,
                        &codigo,
                        &ip,
                        puerto,
                        qr.as_ref(),
                        &estado,
                    );
                });
                let _ = superficie.presentar();
            }
        }
        // Se mira la escucha diez veces por segundo: no hay evento de
        // ventana que avise de una conexion, y dormir del todo dejaria al
        // movil llamando a una puerta que nadie abre.
        pixpin_shell::overlay::esperar_eventos(Some(100));
    }
    Ok(())
}

/// Atiende una llamada entera: saludo, oferta, aceptar y guardar.
fn atender(
    flujo: std::net::TcpStream,
    codigo: &str,
    nombre: &str,
    id: &str,
    carpeta: &std::path::Path,
    raiz: &std::path::Path,
) -> Estado {
    let mut nonce = [0u8; pixpin_sincro::canal::NONCE];
    if !pixpin_shell::azar(&mut nonce) {
        return Estado::Fallo {
            porque: "el sistema no dio azar".into(),
        };
    }
    // `inicia = false`: aqui se esperaba y llamo el movil, asi que el canal
    // lo empieza el (`Envio.kt`, parametro `inicia`).
    let receptor =
        pixpin_sincro::envio::Receptor::conectar(flujo, codigo, nombre, id, nonce, false);
    let mut receptor = match receptor {
        Ok(r) => r,
        Err(e) => {
            return Estado::Fallo {
                porque: e.to_string(),
            };
        }
    };
    match receptor.aceptar(carpeta, |_, _| {}) {
        Ok(cosas) => {
            // Un proyecto entero lo abre la ventana de mensajes; lo suelto va
            // al cuaderno de «Del movil», que es donde el usuario lo busca:
            // «recibi un archivo pero no lo veo, tendria que estar en
            // mensajes guardados».
            let rutas: Vec<std::path::PathBuf> = cosas
                .iter()
                .filter(|(e, _)| e.tipo == "proyecto")
                .map(|(e, r)| {
                    // El movil manda el nombre tal cual, a veces sin
                    // extension: sin `.pixpin` detras, abrirlo no sabria que
                    // es un proyecto y acabaria como un fichero cualquiera.
                    let con_extension = r.with_file_name(nombre_con_extension(e));
                    if con_extension != *r && std::fs::rename(r, &con_extension).is_ok() {
                        con_extension
                    } else {
                        r.clone()
                    }
                })
                .collect();
            for (elemento, ruta) in &cosas {
                // Los tres codigos se registran a proposito: son con lo que
                // se comprueba despues si algo que vuelve del movil es la
                // misma cosa o una nueva.
                tracing::info!(
                    tipo = %elemento.tipo,
                    nombre = %elemento.nombre,
                    uid = ?elemento.uid,
                    codigo_de_chat = ?elemento.codigo_de_chat,
                    creado = elemento.creado,
                    ruta = %ruta.display(),
                    "recibido del movil"
                );
            }
            // Un proyecto entra en la LISTA de proyectos, no como pines
            // sueltos: un `.pixpin` es una conversacion entera, y cada chat
            // es un proyecto.
            let mut proyectos = 0;
            for ruta in &rutas {
                let hecho = pixpin_proyecto::Paquete::abrir(ruta)
                    .map_err(|e| e.to_string())
                    .and_then(|p| {
                        pixpin_proyecto::almacen::importar_paquete(
                            raiz,
                            &p,
                            &pixpin_proyecto::codigos::de_aparato(id),
                        )
                        .map_err(|e| e.to_string())
                    });
                match hecho {
                    Ok(ficha) => {
                        proyectos += 1;
                        tracing::info!(proyecto = %ficha.id, nombre = %ficha.nombre, "proyecto del movil");
                    }
                    Err(e) => {
                        tracing::warn!(%e, ruta = %ruta.display(), "no se pudo abrir el paquete")
                    }
                }
            }
            let _ = proyectos;
            // Y lo suelto, al cuaderno, para que se vea en el chat.
            // El codigo CORTO del equipo (`6ARJ`), no su identificador entero:
            // es lo que va en el codigo de chat de cada mensaje, y con el
            // largo la chapa salia mas ancha que el propio mensaje.
            match al_cuaderno(raiz, &pixpin_proyecto::codigos::de_aparato(id), &cosas) {
                Ok(0) => {}
                Ok(cuantos) => tracing::info!(cuantos, "guardados en el cuaderno del movil"),
                Err(e) => tracing::error!(?e, "no se pudo guardar lo recibido en el cuaderno"),
            }
            Estado::Hecho {
                cuantos: cosas.len(),
            }
        }
        Err(e) => Estado::Fallo {
            porque: e.to_string(),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn pintar(
    p: &Pintor,
    marco: Rect,
    escala: u32,
    textos: &Catalogo,
    codigo: &str,
    ip: &str,
    puerto: u16,
    qr: Option<&qrcodegen::QrCode>,
    estado: &Estado,
) {
    let e = escala as f32 / 100.0;
    let ancho = marco.ancho as f32;
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho,
            alto: marco.alto as f32,
        },
        FONDO,
    );

    let centrado = |texto: &str, y: f32, tam: f32, color: Color| {
        let (w, _) = p.medir_texto(texto, tam);
        p.texto(texto, (ancho - w) / 2.0, y, tam, color);
    };

    centrado(&textos.t("recibir-titulo"), 18.0 * e, 16.0 * e, TEXTO);
    centrado(&textos.t("recibir-como"), 44.0 * e, 12.0 * e, APAGADO);

    // El codigo, grande: es lo que se teclea cuando no se escanea.
    centrado(
        &pixpin_sincro::envio::legible(codigo),
        72.0 * e,
        34.0 * e,
        TEXTO,
    );

    // El QR sobre papel blanco y con su margen: sin borde claro alrededor
    // muchos lectores no lo cogen.
    if let Some(qr) = qr {
        let lado = 240.0 * e;
        let caja = RectF {
            x: (ancho - lado) / 2.0,
            y: 130.0 * e,
            ancho: lado,
            alto: lado,
        };
        p.rellenar(caja, PAPEL);
        let modulos = qr.size().max(1) as f32;
        let margen = 4.0;
        let paso = lado / (modulos + margen * 2.0);
        for y in 0..qr.size() {
            for x in 0..qr.size() {
                if !qr.get_module(x, y) {
                    continue;
                }
                p.rellenar(
                    RectF {
                        x: caja.x + (x as f32 + margen) * paso,
                        y: caja.y + (y as f32 + margen) * paso,
                        // Un pelo mas de lado para que no queden rayas de
                        // fondo entre modulo y modulo al redondear.
                        ancho: paso + 0.5,
                        alto: paso + 0.5,
                    },
                    TINTA,
                );
            }
        }
    }

    centrado(&format!("{ip}:{puerto}"), 390.0 * e, 12.0 * e, APAGADO);

    let (linea, color) = match estado {
        Estado::Esperando => (textos.t("recibir-esperando"), APAGADO),
        Estado::Hecho { cuantos } => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("cuantos", cuantos.to_string());
            (textos.t_args("recibir-hecho", &args), TEXTO)
        }
        Estado::Fallo { porque } => (porque.clone(), ROJO),
    };
    centrado(&linea, 430.0 * e, 13.0 * e, color);
    centrado(&textos.t("recibir-salir"), 470.0 * e, 11.0 * e, APAGADO);
}

/// El nombre con el que se guarda algo que llega, ya con su extension.
///
/// El movil manda el nombre tal cual lo tiene, y a veces viene sin
/// extension: el primer envio de prueba llego como `123` siendo un proyecto
/// entero, y sin `.pixpin` detras la aplicacion no podia saber que lo era.
/// El tipo del elemento SI lo dice, asi que manda el tipo.
fn nombre_con_extension(elemento: &pixpin_sincro::envio::Elemento) -> String {
    let nombre = pixpin_sincro::envio::nombre_sano(&elemento.nombre);
    let ext = match elemento.tipo.as_str() {
        "proyecto" => ".pixpin",
        "lienzo" => ".excalidraw",
        _ => return nombre,
    };
    if nombre.to_ascii_lowercase().ends_with(ext) {
        nombre
    } else {
        format!("{nombre}{ext}")
    }
}

/// Mete en el cuaderno de un proyecto lo que llego y no es un proyecto.
///
/// El usuario, tras el primer envio: «recibí un archivo pero no lo veo,
/// tendría que estar en mensajes guardados». Un fichero suelto no es un
/// proyecto entero, asi que va a uno propio —«Del movil»— que se crea la
/// primera vez y se reutiliza despues: asi se ve en el chat como cualquier
/// otra cosa guardada, en vez de quedarse en una carpeta.
fn al_cuaderno(
    raiz: &std::path::Path,
    aparato: &str,
    cosas: &[(pixpin_sincro::envio::Elemento, std::path::PathBuf)],
) -> std::io::Result<usize> {
    use pixpin_proyecto::{almacen, cuaderno};
    let sueltos: Vec<_> = cosas.iter().filter(|(e, _)| e.tipo != "proyecto").collect();
    if sueltos.is_empty() {
        return Ok(0);
    }
    let ficha =
        almacen::asegurar_guardados(raiz, pixpin_shell::entorno::ahora_local_ms(), aparato)?;
    let carpeta = almacen::carpeta(raiz, &ficha.id);
    let previos = cuaderno::Cuaderno::leer_de(&carpeta).unwrap_or_default();
    // El numero sigue al ultimo del cuaderno: es el que ordena la
    // conversacion y el que entra en el codigo de chat.
    let primero = previos.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let mut hechos = 0;
    for (numero, (elemento, ruta)) in (primero..).zip(sueltos) {
        let bytes = std::fs::read(ruta)?;
        let nombre = nombre_con_extension(elemento);
        let relativa = almacen::guardar_adjunto(raiz, &ficha.id, &nombre, &bytes)?;
        let cuando = if elemento.creado > 0 {
            elemento.creado
        } else {
            pixpin_shell::entorno::ahora_local_ms()
        };
        let mut m = cuaderno::Mensaje::adjunto(
            cuaderno::clase_de_nombre(&nombre),
            &nombre,
            &relativa,
            bytes.len() as i64,
            &cuaderno::Sello {
                cuando,
                numero,
                aparato: aparato.to_string(),
                proyecto: ficha.id.clone(),
            },
        );
        // Los tres codigos que traia se conservan: son con lo que se sabra
        // despues si esto que llego es lo mismo que ya habia o algo nuevo.
        m.uid = elemento.uid.clone();
        m.aparato = elemento.aparato.clone().or(Some(aparato.to_string()));
        cuaderno::anadir(&carpeta, &m)?;
        hechos += 1;
    }
    // La ficha sube en la lista, como con cualquier mensaje nuevo.
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == ficha.id) {
        f.tocado = pixpin_shell::entorno::ahora_local_ms();
        let _ = indice.guardar(raiz);
    }
    Ok(hechos)
}
