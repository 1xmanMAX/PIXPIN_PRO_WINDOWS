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

/// Un lienzo suelto por Wi-Fi, en los dos sentidos, como el movil.
pub(crate) mod lienzo_suelto;

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
///
/// Los bytes de 250 en adelante se tiran: 256 no es multiplo de 10, y con
/// ellos las cifras del 0 al 5 saldrian algo mas que las demas.
pub(crate) fn codigo_nuevo() -> Option<String> {
    let mut salida = String::new();
    while salida.len() < pixpin_sincro::envio::CIFRAS {
        let mut bytes = [0u8; 16];
        if !pixpin_shell::azar(&mut bytes) {
            return None;
        }
        for b in bytes {
            if b < 250 && salida.len() < pixpin_sincro::envio::CIFRAS {
                salida.push(char::from(b'0' + b % 10));
            }
        }
    }
    Some(salida)
}

/// El QR de un texto, o nada si no cabe (no pasa con los de PixPin).
pub(crate) fn qr_de(texto: &str) -> Option<qrcodegen::QrCode> {
    qrcodegen::QrCode::encode_text(texto, qrcodegen::QrCodeEcc::Medium).ok()
}

/// El QR sobre papel blanco y con su margen: sin borde claro alrededor
/// muchos lectores no lo cogen.
pub(crate) fn pintar_qr(p: &Pintor, caja: RectF, qr: &qrcodegen::QrCode) {
    p.rellenar(caja, PAPEL);
    let modulos = qr.size().max(1) as f32;
    let margen = 4.0;
    let paso = caja.ancho / (modulos + margen * 2.0);
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

/// La IP de este equipo en la red local.
///
/// Se pregunta abriendo un socket UDP «hacia» una direccion de fuera: no
/// manda ni un byte —UDP no conecta de verdad—, pero obliga al sistema a
/// elegir la tarjeta por la que saldria, que es justo la que el movil tiene
/// que ver. Enumerar tarjetas daria varias y habria que adivinar cual.
pub(crate) fn ip_local() -> Option<String> {
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
            let hecho = crate::dispositivo_perdido::con_recursos("recibir", |r| {
                abrir(r, &textos, &ubicacion)
            });
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
    // En un puerto cualquiera, NO en el de siempre: ese lo tiene la presencia
    // (`sincronizar::presencia`), que vive mientras vive la aplicacion, y dos
    // sitios no pueden quedarse con el mismo. El movil no lo da por fijo
    // porque lo lee del QR.
    let escucha =
        std::net::TcpListener::bind(("0.0.0.0", 0)).context("no se pudo escuchar en la red")?;
    escucha
        .set_nonblocking(true)
        .context("la escucha tiene que poder mirarse sin bloquear")?;
    let puerto = escucha.local_addr().map(|d| d.port()).unwrap_or_default();
    let ip = ip_local().unwrap_or_default();
    let qr = qr_de(&pixpin_sincro::envio::texto_del_qr_de_recepcion(
        &codigo, &ip, puerto,
    ));
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
            // Este cartel no pregunta nada: lo que coincide se pone al dia,
            // que es lo que viene elegido en el movil. Elegir cosa por cosa
            // es de «Sincronizar → Recibir por Wi-Fi», que si tiene lista.
            guardar_lo_recibido(raiz, id, &cosas, &ComoNuevo::new());
            Estado::Hecho {
                cuantos: cosas.len(),
            }
        }
        Err(e) => Estado::Fallo {
            porque: e.to_string(),
        },
    }
}

/// Donde quedo una cosa recibida, para decirlo en «Recibido de X».
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Donde {
    /// Entro en la lista de proyectos.
    Proyecto,
    /// Ya estaba y se puso al dia el que habia.
    ProyectoAlDia,
    /// Un archivo suelto, en «Mensajes guardados».
    Guardados,
    /// Ya estaba en «Mensajes guardados» y se puso al dia.
    GuardadosAlDia,
    /// Llego, pero solo quedo en la carpeta `recibidos/`.
    Carpeta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Guardado {
    pub nombre: String,
    pub donde: Donde,
}

/// Que hacer con cada cosa que llega y ya se tiene, por su `identidad`:
/// `true` es «crear como nuevo» y lo que no esta en la tabla, «actualizar el
/// que tengo», que es lo que viene puesto en el movil.
pub(crate) type ComoNuevo = std::collections::BTreeMap<String, bool>;

/// La ficha que describe un proyecto que llega, solo con sus tres codigos:
/// es lo unico que hace falta para saber si aqui ya esta esa misma cosa.
///
/// El `id` sale de la identidad (`proyecto:<id>`) porque un proyecto de antes
/// de los codigos no trae `uid`, y entonces el codigo unico se saca de su id,
/// igual que en Android.
fn ficha_que_llega(e: &pixpin_sincro::envio::Elemento) -> pixpin_proyecto::almacen::Ficha {
    pixpin_proyecto::almacen::Ficha {
        id: e
            .identidad
            .strip_prefix("proyecto:")
            .unwrap_or(&e.identidad)
            .to_string(),
        nombre: e.nombre.clone(),
        uid: e.uid.clone(),
        creado: e.creado,
        aparato: e.aparato.clone(),
        ..Default::default()
    }
}

/// Si un mensaje que ya esta aqui es esa misma cosa que llega: los tres
/// codigos iguales (`Codigos.mismos` del movil). Sin codigo de chat no se
/// puede afirmar, y ante la duda se duplica en vez de pisar.
fn mismo_mensaje(
    m: &pixpin_proyecto::cuaderno::Mensaje,
    e: &pixpin_sincro::envio::Elemento,
) -> bool {
    m.uid.is_some()
        && m.uid == e.uid
        && m.codigo_chat().is_some()
        && m.codigo_chat() == e.codigo_de_chat
        && m.cuando == e.creado
}

/// **Lo que aqui ES cada cosa que llega**, por su nombre, para poder ofrecer
/// ponerlo al dia (`Recepcion.queSustituye` del movil).
///
/// Va por los tres codigos y nada mas: mismo nombre no es la misma cosa, y
/// dos proyectos que se llamen «Tesis» en dos aparatos distintos son dos
/// proyectos. Lo que no coincide en los tres no sale aqui y entra aparte.
pub(crate) fn que_sustituye(
    raiz: &std::path::Path,
    elementos: &[pixpin_sincro::envio::Elemento],
) -> std::collections::BTreeMap<String, String> {
    use pixpin_proyecto::almacen::Indice;
    let indice = Indice::leer(raiz);
    let mut salida = std::collections::BTreeMap::new();
    for e in elementos {
        let nombre = if e.tipo == pixpin_sincro::envio::PROYECTO {
            indice.misma(&ficha_que_llega(e)).map(|f| f.nombre.clone())
        } else {
            mensaje_que_sustituye(raiz, &indice, e).map(|(_, _, m)| {
                if m.nombre.trim().is_empty() {
                    e.nombre.clone()
                } else {
                    m.nombre.clone()
                }
            })
        };
        if let Some(n) = nombre {
            salida.insert(e.identidad.clone(), n);
        }
    }
    salida
}

/// El mensaje de cualquier chat de este equipo con los tres codigos de lo que
/// llega: en que proyecto esta, en que linea del cuaderno y cual es.
fn mensaje_que_sustituye(
    raiz: &std::path::Path,
    indice: &pixpin_proyecto::almacen::Indice,
    e: &pixpin_sincro::envio::Elemento,
) -> Option<(String, usize, pixpin_proyecto::cuaderno::Mensaje)> {
    // Sin codigo unico o sin fecha no hay tres codigos que comparar.
    if e.uid.is_none() || e.creado <= 0 {
        return None;
    }
    for f in &indice.proyectos {
        let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &f.id);
        let Ok(cuaderno) = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta) else {
            continue;
        };
        if let Some((i, m)) = cuaderno
            .mensajes
            .iter()
            .enumerate()
            .find(|(_, m)| mismo_mensaje(m, e))
        {
            return Some((f.id.clone(), i, m.clone()));
        }
    }
    None
}

/// Lo que llego, a su sitio: los proyectos a la lista, lo suelto a
/// «Mensajes guardados». Es lo mismo para quien espera con su QR y para quien
/// llama con un codigo, y por eso esta aqui una sola vez.
pub(crate) fn guardar_lo_recibido(
    raiz: &std::path::Path,
    id: &str,
    cosas: &[(pixpin_sincro::envio::Elemento, std::path::PathBuf)],
    como_nuevo: &ComoNuevo,
) -> Vec<Guardado> {
    for (elemento, ruta) in cosas {
        // Los tres codigos se registran a proposito: son con lo que se
        // comprueba despues si algo que vuelve del movil es la misma cosa o
        // una nueva.
        tracing::info!(
            tipo = %elemento.tipo,
            nombre = %elemento.nombre,
            uid = ?elemento.uid,
            codigo_de_chat = ?elemento.codigo_de_chat,
            creado = elemento.creado,
            ruta = %ruta.display(),
            "recibido"
        );
    }
    // El codigo CORTO del equipo (`6ARJ`), no su identificador entero: es lo
    // que va en el codigo de chat de cada mensaje, y con el largo la chapa
    // salia mas ancha que el propio mensaje.
    let aparato = pixpin_proyecto::codigos::de_aparato(id);
    let mut guardados = Vec::new();
    // Un proyecto entra en la LISTA de proyectos, no como pines sueltos: un
    // `.pixpin` es una conversacion entera, y cada chat es un proyecto.
    for (e, r) in cosas.iter().filter(|(e, _)| e.tipo == "proyecto") {
        // El movil manda el nombre tal cual, a veces sin extension: sin
        // `.pixpin` detras, abrirlo no sabria que es un proyecto.
        let con_extension = r.with_file_name(nombre_con_extension(e));
        let ruta = if con_extension != *r && std::fs::rename(r, &con_extension).is_ok() {
            con_extension
        } else {
            r.clone()
        };
        let nuevo = como_nuevo.get(&e.identidad).copied().unwrap_or(false);
        let hecho = pixpin_proyecto::Paquete::abrir(&ruta)
            .map_err(|e| e.to_string())
            .and_then(|p| {
                guardar_un_proyecto(raiz, e, p, &aparato, nuevo).map_err(|x| x.to_string())
            });
        let donde = match hecho {
            Ok((ficha, al_dia)) => {
                tracing::info!(proyecto = %ficha.id, nombre = %ficha.nombre, al_dia, "proyecto recibido");
                if al_dia {
                    Donde::ProyectoAlDia
                } else {
                    Donde::Proyecto
                }
            }
            Err(e) => {
                tracing::warn!(%e, ruta = %ruta.display(), "no se pudo abrir el paquete");
                Donde::Carpeta
            }
        };
        guardados.push(Guardado {
            nombre: e.nombre.clone(),
            donde,
        });
    }
    // **Un lienzo suelto, a su proyecto** (`Recepcion.guardarLienzo` del
    // movil): llega como un `.pixpin` de una hoja. Antes caia en «Del movil»
    // como `<nombre>.excalidraw` con el ZIP dentro, y no abria nada.
    let mut resto = Vec::new();
    for (e, r) in cosas.iter().filter(|(e, _)| e.tipo != "proyecto") {
        if e.tipo != pixpin_sincro::envio::LIENZO {
            resto.push((e.clone(), r.clone()));
            continue;
        }
        let nuevo = como_nuevo.get(&e.identidad).copied().unwrap_or(false);
        let motivo = format!("Antes de recibir «{}»", e.nombre);
        let hecho = lienzo_suelto::guardar(
            raiz,
            e,
            r,
            &aparato,
            nuevo,
            pixpin_shell::entorno::ahora_utc_ms(),
            &|ficha| copia_antes_de_tocar(raiz, ficha, &motivo),
        );
        match hecho {
            Ok(llegada) => {
                tracing::info!(?llegada, nombre = %e.nombre, "lienzo recibido");
                guardados.push(Guardado {
                    nombre: e.nombre.clone(),
                    donde: match llegada {
                        lienzo_suelto::Llegada::AlDia(_) => Donde::ProyectoAlDia,
                        _ => Donde::Proyecto,
                    },
                });
            }
            // Si no se entiende como lienzo, al menos queda en el chat.
            Err(x) => {
                tracing::warn!(%x, ruta = %r.display(), "lienzo que no se pudo poner en su proyecto");
                resto.push((e.clone(), r.clone()));
            }
        }
    }
    // Y lo suelto, al cuaderno, para que se vea en el chat.
    match al_cuaderno(raiz, &aparato, &resto, como_nuevo) {
        Ok(sueltos) => guardados.extend(sueltos),
        Err(e) => {
            tracing::error!(?e, "no se pudo guardar lo recibido en el cuaderno");
            for (e, _) in &resto {
                guardados.push(Guardado {
                    nombre: e.nombre.clone(),
                    donde: Donde::Carpeta,
                });
            }
        }
    }
    // Lo que acaba de entrar en el almacen y en el cuaderno tiene que verse
    // en la ventana de chat que ya estuviera abierta: la escritura ocurre
    // DEBAJO de ella y sin este aviso no sale hasta cerrarla y reabrirla.
    if !guardados.is_empty() {
        crate::ventana_chat::refrescar();
        // Un `.pixpin` puede traer mensajes con hora puesta desde el movil:
        // se vuelve a leer la agenda para que suenen aqui tambien.
        crate::recordatorios::releer(raiz);
    }
    guardados
}

/// Un proyecto que llega, a su sitio: encima del que ya hay si el usuario
/// eligio «actualizar el que tengo», o aparte si eligio «crear como nuevo».
/// Devuelve la ficha y si fue lo primero.
///
/// **Antes de escribir encima, la copia de seguridad** (`Copias` del movil,
/// 15-sep-2026): el usuario perdio lienzos de «Tesis» al recibir un envio, y
/// preguntó lo que habia que haber resuelto antes: si algo sale mal, ¿como
/// vuelvo a lo de antes? Si la copia no se puede hacer, tampoco se escribe.
fn guardar_un_proyecto(
    raiz: &std::path::Path,
    e: &pixpin_sincro::envio::Elemento,
    paquete: pixpin_proyecto::Paquete,
    aparato: &str,
    como_nuevo: bool,
) -> std::io::Result<(pixpin_proyecto::almacen::Ficha, bool)> {
    use pixpin_proyecto::almacen;
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let mismo = if como_nuevo {
        None
    } else {
        almacen::Indice::leer(raiz)
            .misma(&ficha_que_llega(e))
            .map(|f| f.id.clone())
    };
    let Some(id) = mismo else {
        let p = if como_nuevo {
            almacen::renovar(&paquete, ahora)
        } else {
            paquete
        };
        return almacen::importar_paquete(raiz, &p, aparato).map(|f| (f, false));
    };
    copia_antes_de_tocar(raiz, &id, &format!("Antes de recibir «{}»", e.nombre))?;
    almacen::actualizar_paquete(raiz, &id, &paquete).map(|f| (f, true))
}

/// Guarda como esta un chat antes de que un envio lo pise. Ver
/// `pixpin_sincro::copias`, que es el puerto de `sincro/Copias.kt`.
fn copia_antes_de_tocar(raiz: &std::path::Path, ficha: &str, motivo: &str) -> std::io::Result<()> {
    let Some(chat) = pixpin_proyecto::vista::chat_de_ficha(raiz, ficha) else {
        return Ok(());
    };
    let disco = pixpin_proyecto::vista::DiscoPc::nuevo(raiz);
    pixpin_sincro::copias::hacer(&disco, &chat, motivo, pixpin_shell::entorno::ahora_utc_ms())
        .map(|_| ())
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

    if let Some(qr) = qr {
        let lado = 240.0 * e;
        let caja = RectF {
            x: (ancho - lado) / 2.0,
            y: 130.0 * e,
            ancho: lado,
            alto: lado,
        };
        pintar_qr(p, caja, qr);
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
        // Un lienzo del movil es un `.pixpin` de una hoja (`deLienzo`).
        "lienzo" => ".pixpin",
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
    como_nuevo: &ComoNuevo,
) -> std::io::Result<Vec<Guardado>> {
    use pixpin_proyecto::{almacen, cuaderno};
    let sueltos: Vec<_> = cosas.iter().filter(|(e, _)| e.tipo != "proyecto").collect();
    if sueltos.is_empty() {
        return Ok(Vec::new());
    }
    let ficha = almacen::asegurar_guardados(raiz, pixpin_shell::entorno::ahora_utc_ms(), aparato)?;
    let carpeta = almacen::carpeta(raiz, &ficha.id);
    let previos = cuaderno::Cuaderno::leer_de(&carpeta).unwrap_or_default();
    // El numero sigue al ultimo del cuaderno: es el que ordena la
    // conversacion y el que entra en el codigo de chat.
    let mut numero = previos.mensajes.iter().map(|m| m.numero).max().unwrap_or(0);
    let indice = almacen::Indice::leer(raiz);
    let mut salida = Vec::new();
    for (elemento, ruta) in sueltos {
        let nuevo = como_nuevo
            .get(&elemento.identidad)
            .copied()
            .unwrap_or(false);
        // **Lo que ya esta aqui con los tres codigos** se pone al dia en su
        // sitio, sin crear otra burbuja: el chat sigue senalando lo mismo.
        let antes = (!nuevo)
            .then(|| mensaje_que_sustituye(raiz, &indice, elemento))
            .flatten();
        let bytes = std::fs::read(ruta)?;
        let nombre = nombre_con_extension(elemento);
        let donde = match antes {
            Some((suyo, _, viejo)) => {
                copia_antes_de_tocar(raiz, &suyo, &format!("Antes de recibir «{nombre}»"))?;
                poner_al_dia(raiz, &suyo, &viejo, &nombre, &bytes)?;
                Donde::GuardadosAlDia
            }
            None => {
                numero += 1;
                let relativa = almacen::guardar_adjunto(raiz, &ficha.id, &nombre, &bytes)?;
                let cuando = match (nuevo, elemento.creado) {
                    // «Crear como nuevo» estrena tambien la fecha: es otra
                    // cosa desde ahora, y con la de antes se seguiria
                    // pareciendo a la que ya hay.
                    (false, c) if c > 0 => c,
                    _ => pixpin_shell::entorno::ahora_utc_ms(),
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
                // Los tres codigos que traia se conservan: son con lo que se
                // sabra despues si esto que llego es lo mismo que ya habia o
                // algo nuevo. Al crear como nuevo NO, que para eso se eligio:
                // los que le puso `Mensaje::adjunto` son suyos.
                if !nuevo {
                    m.uid = elemento.uid.clone();
                    m.aparato = elemento.aparato.clone().or(Some(aparato.to_string()));
                }
                cuaderno::anadir(&carpeta, &m)?;
                Donde::Guardados
            }
        };
        salida.push(Guardado {
            nombre: elemento.nombre.clone(),
            donde,
        });
    }
    // La ficha sube en la lista, como con cualquier mensaje nuevo.
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == ficha.id) {
        f.tocado = pixpin_shell::entorno::ahora_utc_ms();
        let _ = indice.guardar(raiz);
    }
    Ok(salida)
}

/// Escribe lo que llega **encima del archivo que ya tenia** ese mensaje, sin
/// tocar el mensaje de sitio: asi el chat, que senala esa ruta, ensena lo
/// nuevo en la misma burbuja de siempre (`Recepcion.guardarArchivo` del movil
/// cuando `antes != null`).
fn poner_al_dia(
    raiz: &std::path::Path,
    ficha: &str,
    viejo: &pixpin_proyecto::cuaderno::Mensaje,
    nombre: &str,
    bytes: &[u8],
) -> std::io::Result<()> {
    use pixpin_proyecto::{almacen, cuaderno};
    let carpeta = almacen::carpeta(raiz, ficha);
    let relativa = match viejo.ruta.as_deref().filter(|r| !r.is_empty()) {
        Some(r) => {
            let destino = carpeta.join(r);
            if let Some(padre) = destino.parent() {
                std::fs::create_dir_all(padre)?;
            }
            std::fs::write(&destino, bytes)?;
            r.to_string()
        }
        // Un mensaje sin archivo (una nota) que ahora llega con uno: se le
        // guarda al lado en vez de perderlo.
        None => almacen::guardar_adjunto(raiz, ficha, nombre, bytes)?,
    };
    let puesto = cuaderno::Mensaje {
        ruta: Some(relativa),
        bytes: bytes.len() as i64,
        ..viejo.clone()
    };
    cuaderno::reemplazar(&carpeta, &puesto).map(|_| ())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::almacen;
    use pixpin_sincro::envio::Elemento;

    fn carpeta_temporal(etiqueta: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("pixpin-recibir-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// Un `.pixpin` como el que manda el movil, con sus tres codigos.
    fn paquete(nombre: &str, uid: &str, creado: i64) -> pixpin_proyecto::Paquete {
        let proyecto = pixpin_proyecto::Proyecto {
            id: "p-movil".into(),
            nombre: nombre.into(),
            hojas: vec![pixpin_proyecto::Hoja {
                id: "h1".into(),
                nombre: "Plano".into(),
                dibujo: Some("d1".into()),
                uid: Some("AAAAAAAAAA".into()),
                ..Default::default()
            }],
            tocado: creado,
            uid: Some(uid.into()),
            creado,
            aparato: Some("K7Q2".into()),
            ..Default::default()
        };
        let mut p = pixpin_proyecto::Paquete::nuevo(Default::default(), proyecto);
        p.poner_entrada("lienzos/d1.excalidraw", b"{}".to_vec());
        pixpin_proyecto::Paquete::desde_bytes(&p.a_bytes().unwrap()).unwrap()
    }

    fn elemento_de_proyecto(nombre: &str, uid: &str, creado: i64) -> Elemento {
        Elemento {
            tipo: pixpin_sincro::envio::PROYECTO.into(),
            nombre: nombre.into(),
            bytes: 10,
            mime: None,
            identidad: "proyecto:p-movil".into(),
            proyecto: None,
            proyecto_nombre: None,
            creado,
            uid: Some(uid.into()),
            codigo_de_chat: None,
            aparato: Some("K7Q2".into()),
        }
    }

    #[test]
    fn un_proyecto_que_vuelve_del_movil_se_reconoce_y_uno_parecido_no() {
        let raiz = carpeta_temporal("sustituye");
        let p = paquete("Tesis", "VVT587BFCA", 1_757_939_357_123);
        almacen::importar_paquete(&raiz, &p, "ZZZZ").unwrap();

        let vuelve = elemento_de_proyecto("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let hay = que_sustituye(&raiz, std::slice::from_ref(&vuelve));
        assert_eq!(
            hay.get("proyecto:p-movil").map(String::as_str),
            Some("Tesis")
        );

        // Casos negativos: mismo nombre no es la misma cosa. Con otro codigo
        // unico, otra fecha u otro aparato entra aparte y no pisa nada.
        for otro in [
            elemento_de_proyecto("Tesis", "2222222222", 1_757_939_357_123),
            elemento_de_proyecto("Tesis", "VVT587BFCA", 1),
            Elemento {
                aparato: Some("OTRO".into()),
                ..elemento_de_proyecto("Tesis", "VVT587BFCA", 1_757_939_357_123)
            },
        ] {
            assert!(
                que_sustituye(&raiz, std::slice::from_ref(&otro)).is_empty(),
                "{otro:?} no es lo mismo"
            );
        }
    }

    #[test]
    fn actualizar_pone_al_dia_el_que_habia_y_deja_copia_de_como_estaba() {
        let raiz = carpeta_temporal("actualizar");
        let p = paquete("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let antes = almacen::importar_paquete(&raiz, &p, "ZZZZ").unwrap();

        let e = elemento_de_proyecto("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let vuelve = paquete("Tesis al día", "VVT587BFCA", 1_757_939_357_123);
        let (ficha, al_dia) = guardar_un_proyecto(&raiz, &e, vuelve, "ZZZZ", false).unwrap();
        assert!(al_dia);
        assert_eq!(ficha.id, antes.id, "el proyecto de aqui conserva su id");
        assert_eq!(ficha.nombre, "Tesis al día");
        assert_eq!(
            almacen::Indice::leer(&raiz).proyectos.len(),
            1,
            "no duplica"
        );

        // Y queda como estaba antes, en «copias», que es lo que pidio el
        // usuario tras perder lienzos: poder volver.
        let chat = pixpin_proyecto::vista::chat_de_ficha(&raiz, &ficha.id).unwrap();
        let disco = pixpin_proyecto::vista::DiscoPc::nuevo(&raiz);
        let copias = pixpin_sincro::copias::lista(&disco, &chat);
        assert_eq!(copias.len(), 1, "una copia de antes de recibir");
        assert!(
            copias[0].motivo.contains("Antes de recibir"),
            "{:?}",
            copias[0].motivo
        );
    }

    #[test]
    fn crear_como_nuevo_deja_los_dos_y_no_toca_el_que_habia() {
        let raiz = carpeta_temporal("como-nuevo");
        let p = paquete("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let antes = almacen::importar_paquete(&raiz, &p, "ZZZZ").unwrap();

        let e = elemento_de_proyecto("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let vuelve = paquete("Tesis", "VVT587BFCA", 1_757_939_357_123);
        let (ficha, al_dia) = guardar_un_proyecto(&raiz, &e, vuelve, "ZZZZ", true).unwrap();
        assert!(!al_dia);
        assert_ne!(ficha.id, antes.id);
        assert!(!ficha.misma_que(&antes), "estrena codigos y conviven");
        assert_eq!(almacen::Indice::leer(&raiz).proyectos.len(), 2);
    }
}
