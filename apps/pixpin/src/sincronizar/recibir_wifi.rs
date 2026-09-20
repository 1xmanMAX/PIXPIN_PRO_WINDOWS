//! «Recibir por Wi-Fi», dentro de Sincronizar.
//!
//! Copia de `sincro/RecibirActivity.kt` del movil, con sus estados en el
//! mismo orden —pidiendo, buscando, la oferta, recibiendo, recibido, no se
//! pudo— y sus textos. Lo unico que no se copia es la camara: el ordenador no
//! la tiene, asi que donde el movil escanea aqui se escriben las seis cifras
//! o se pega el texto del QR (`pixpin-envio:1:...`, que trae donde esta quien
//! envia y ahorra buscarlo).
//!
//! Las dos formas de recibir acaban en lo mismo (`tramitar`): da igual quien
//! llamo a quien, hay un `Receptor` abierto, se ensena lo que trae, se acepta
//! o no y se guarda donde va todo lo recibido (`recibir::guardar_lo_recibido`).

use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use pixpin_render::icono::material;
use pixpin_sincro::envio::{self, DelQr, Oferta, Receptor};
use pixpin_store::Catalogo;

use super::{
    Accion, Aviso, CONTORNO, Contexto, ERROR, FONDO, Lienzo, PRIMARIO, SUAVE, TEXTO, Tecla,
    VARIANTE, VERDE, caja, menos, rect,
};
use crate::recibir::{Donde, Guardado};

/// Cuanto aguanta un socket de envio sin que llegue nada: quien envia puede
/// estar decidiendo si aprueba, y el movil espera hasta media hora.
pub(super) const ESPERA_LARGA: Duration = Duration::from_secs(30 * 60);

/// Lo que se ensena mientras se busca (`Estado.Buscando` del movil).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Busca {
    Buscando,
    Conectando,
}

/// Por que no se pudo, para decirlo con las palabras del movil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Falla {
    NoEncontrado,
    CodigoMalo,
    /// Se corto a medias: el motivo tal cual.
    Cortado(String),
    /// El otro lo dijo (p. ej. «no aprobo el envio a este aparato»).
    Dijo(String),
}

pub(super) enum Estado {
    Pidiendo,
    Buscando(Busca),
    /// `decide` suelta al hilo que espera: la tabla acepta —y dice, cosa por
    /// cosa, si se crea como nueva—, y soltarlo sin mandar nada es «no».
    Oferta {
        oferta: Oferta,
        /// Lo que aqui ES cada cosa que llega, por su `identidad`: lo que
        /// hace que aparezcan las dos opciones del movil.
        sustituye: std::collections::BTreeMap<String, String>,
        /// Lo elegido. Lo que no esta, «actualizar el que tengo», que es lo
        /// que viene puesto en el movil.
        como_nuevo: crate::recibir::ComoNuevo,
        decide: mpsc::Sender<crate::recibir::ComoNuevo>,
    },
    Recibiendo {
        de: String,
        hechos: u64,
        total: u64,
    },
    Hecho {
        de: String,
        guardados: Vec<Guardado>,
    },
    Fallo(Falla),
}

/// Lo que se pulsa en esta pantalla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Toque {
    Campo,
    Recibir,
    EnsenarMio,
    NoGracias,
    Aceptar,
    ProbarOtraVez,
    Cerrar,
    /// Las dos pastillas de una cosa que ya se tiene, por su `identidad`.
    Elegir(String, bool),
}

/// «Que me envien a mi»: el codigo, su QR y donde se espera.
struct Mio {
    codigo: String,
    qr: Option<qrcodegen::QrCode>,
}

pub(super) struct Recibir {
    pub(super) turno: u64,
    pub(super) estado: Estado,
    tecleado: String,
    mio: Option<Mio>,
    /// Mientras sea cierto, la escucha de «mi codigo» y su anuncio siguen.
    vivo: Arc<AtomicBool>,
    /// Hay una recepcion en marcha: una segunda llamada no la pisa.
    ocupado: Arc<AtomicBool>,
}

impl Drop for Recibir {
    fn drop(&mut self) {
        // Al salir de la pantalla la puerta se cierra, como al cerrar la
        // actividad en el movil.
        self.vivo.store(false, Ordering::SeqCst);
    }
}

impl Recibir {
    pub(super) fn nuevo(turno: u64) -> Recibir {
        Recibir {
            turno,
            estado: Estado::Pidiendo,
            tecleado: String::new(),
            mio: None,
            vivo: Arc::new(AtomicBool::new(true)),
            ocupado: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Lo escrito, si vale para recibir: seis cifras, o el QR de quien
    /// envia. El de quien espera recibir (`pixpin-recibe`) no: ese es para
    /// mandarle, desde Enviar.
    fn destino(&self) -> Option<DelQr> {
        envio::leer_tecleado(&self.tecleado).filter(|q| !q.espera_recibir)
    }

    /// Si hay algo que gira y hay que repintar sin que pase nada.
    pub(super) fn animando(&self) -> bool {
        matches!(self.estado, Estado::Buscando(_))
    }
}

/// Lo que se escribe en el campo. Devuelve si cambio algo.
pub(super) fn tecla(r: &mut Recibir, t: Tecla, cx: &Contexto) -> bool {
    if !matches!(r.estado, Estado::Pidiendo) {
        return false;
    }
    match t {
        Tecla::Letra(c) => {
            if r.tecleado.chars().count() < 80 {
                r.tecleado.push(c);
            }
        }
        Tecla::Borrar => {
            r.tecleado.pop();
        }
        Tecla::Pegar(texto) => r.tecleado = texto.trim().chars().take(80).collect(),
        Tecla::Intro => {
            tocar(r, Toque::Recibir, cx);
        }
    }
    true
}

/// Un toque. Devuelve `true` si hay que volver a la portada.
pub(super) fn tocar(r: &mut Recibir, t: Toque, cx: &Contexto) -> bool {
    match t {
        Toque::Campo => {}
        Toque::Recibir => {
            if let Some(q) = r.destino()
                && !r.ocupado.swap(true, Ordering::SeqCst)
            {
                r.estado = Estado::Buscando(Busca::Buscando);
                recibir_de(q, cx, r.turno, r.vivo.clone(), r.ocupado.clone());
            }
        }
        Toque::EnsenarMio => {
            if r.mio.is_none() {
                match esperar_a_que_me_manden(cx, r.turno, r.vivo.clone(), r.ocupado.clone()) {
                    Ok(m) => r.mio = Some(m),
                    Err(e) => r.estado = Estado::Fallo(Falla::Cortado(e.to_string())),
                }
            }
        }
        Toque::NoGracias => {
            // Sin mandar nada: soltar el canal es «no, gracias».
            r.estado = Estado::Pidiendo;
        }
        Toque::Aceptar => {
            if let Estado::Oferta {
                decide,
                oferta,
                como_nuevo,
                ..
            } = &r.estado
            {
                let _ = decide.send(como_nuevo.clone());
                r.estado = Estado::Recibiendo {
                    de: oferta.de.clone(),
                    hechos: 0,
                    total: oferta.elementos.iter().map(|e| e.bytes.max(0) as u64).sum(),
                };
            }
        }
        Toque::Elegir(identidad, nuevo) => {
            if let Estado::Oferta { como_nuevo, .. } = &mut r.estado {
                como_nuevo.insert(identidad, nuevo);
            }
        }
        Toque::ProbarOtraVez => r.estado = Estado::Pidiendo,
        Toque::Cerrar => return true,
    }
    false
}

// ------------------------------------------------------------------ la red

/// `Red.conectar` para un envio: 8 s para llegar y media hora de espera.
pub(super) fn conectar(host: &str, puerto: u16) -> std::io::Result<TcpStream> {
    let dir = (host, puerto)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::other("direccion sin resolver"))?;
    let s = TcpStream::connect_timeout(&dir, Duration::from_secs(8))?;
    s.set_read_timeout(Some(ESPERA_LARGA))?;
    s.set_nodelay(true)?;
    Ok(s)
}

/// Busca por la red, hasta 20 s como el movil, quien anuncia `servicio` con
/// la etiqueta del codigo, y conecta con el.
pub(super) fn buscar_y_conectar(
    servicio: &str,
    codigo: &str,
    vivo: &AtomicBool,
) -> Option<TcpStream> {
    let etiqueta = envio::etiqueta(codigo);
    let empezo = Instant::now();
    while empezo.elapsed() < Duration::from_secs(20) && vivo.load(Ordering::SeqCst) {
        match pixpin_shell::mdns::buscar(servicio, Duration::from_secs(3)) {
            Ok(vistos) => {
                if let Some(v) = vistos
                    .into_iter()
                    .find(|v| v.datos.get("g") == Some(&etiqueta))
                {
                    return conectar(&v.host.to_string(), v.puerto).ok();
                }
            }
            Err(e) => {
                tracing::warn!(?e, "no se pudo buscar en la red");
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
    None
}

/// Directo a la direccion del QR si la hay; si no responde, buscando.
fn conectar_con(q: &DelQr, vivo: &AtomicBool) -> Option<TcpStream> {
    if let Some(h) = &q.host
        && q.puerto > 0
        && let Ok(s) = conectar(h, q.puerto)
    {
        return Some(s);
    }
    buscar_y_conectar(envio::SERVICIO, &q.codigo, vivo)
}

pub(super) fn nonce() -> Option<[u8; pixpin_sincro::canal::NONCE]> {
    let mut n = [0u8; pixpin_sincro::canal::NONCE];
    pixpin_shell::azar(&mut n).then_some(n)
}

fn avisar(cx: &Contexto, turno: u64, estado: Estado) -> bool {
    cx.tx.send(Aviso::Recibir(turno, estado)).is_ok()
}

/// Recibir de quien envia, con el codigo que se tecleo (`recibir` del movil).
fn recibir_de(
    q: DelQr,
    cx: &Contexto,
    turno: u64,
    vivo: Arc<AtomicBool>,
    ocupado: Arc<AtomicBool>,
) {
    let cx = cx.clone();
    let lanzado = std::thread::Builder::new()
        .name("recibir-wifi".into())
        .spawn(move || {
            let Some(s) = conectar_con(&q, &vivo) else {
                avisar(&cx, turno, Estado::Fallo(Falla::NoEncontrado));
                ocupado.store(false, Ordering::SeqCst);
                return;
            };
            avisar(&cx, turno, Estado::Buscando(Busca::Conectando));
            let fin = match nonce() {
                None => Estado::Fallo(Falla::Cortado("el sistema no dio azar".into())),
                Some(n) => match Receptor::conectar(s, &q.codigo, &cx.nombre, &cx.id, n, true) {
                    Ok(r) => tramitar(r, &cx, turno),
                    Err(e) => Estado::Fallo(falla_al_conectar(&e)),
                },
            };
            avisar(&cx, turno, fin);
            ocupado.store(false, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de recibir");
    }
}

/// Quien envia con otro codigo corta en cuanto no descifra el saludo, y aqui
/// eso llega como un corte: el movil lo cuenta como «el codigo no es el
/// bueno», y asi se cuenta.
fn falla_al_conectar(e: &envio::ErrorEnvio) -> Falla {
    use std::io::ErrorKind as K;
    match e {
        envio::ErrorEnvio::Canal(pixpin_sincro::canal::ErrorCanal::Io(io))
            if matches!(
                io.kind(),
                K::UnexpectedEof | K::ConnectionReset | K::ConnectionAborted
            ) =>
        {
            Falla::CodigoMalo
        }
        envio::ErrorEnvio::Dijo(m) => Falla::Dijo(m.clone()),
        otro => Falla::Cortado(otro.to_string()),
    }
}

/// De la oferta a lo guardado. Se queda esperando a que se decida en la
/// ventana; si la pantalla se cierra antes, el canal de la decision se suelta
/// y cuenta como «no».
fn tramitar(mut r: Receptor<TcpStream>, cx: &Contexto, turno: u64) -> Estado {
    let (decide, decision) = mpsc::channel();
    let oferta = r.oferta.clone();
    // **Que de esto ya se tiene**, antes de preguntar: es lo que hace que
    // salgan «Actualizar el que tengo» y «Crear como nuevo» como en el movil.
    // Se mira aqui, en el hilo de la red, porque lee el disco entero.
    let sustituye = crate::recibir::que_sustituye(&cx.raiz, &oferta.elementos);
    if !avisar(
        cx,
        turno,
        Estado::Oferta {
            oferta: oferta.clone(),
            sustituye,
            como_nuevo: Default::default(),
            decide,
        },
    ) {
        r.rechazar();
        return Estado::Pidiendo;
    }
    let Ok(como_nuevo) = decision.recv_timeout(ESPERA_LARGA) else {
        r.rechazar();
        return Estado::Pidiendo;
    };
    let carpeta = cx.raiz.join("recibidos");
    let mut ultimo = Instant::now();
    let llegados = r.aceptar(&carpeta, |hechos, total| {
        // Cada 120 ms basta para que la barra se mueva sin inundar la
        // ventana de avisos, como en el movil.
        if ultimo.elapsed() > Duration::from_millis(120) || hechos == total {
            ultimo = Instant::now();
            avisar(
                cx,
                turno,
                Estado::Recibiendo {
                    de: oferta.de.clone(),
                    hechos,
                    total,
                },
            );
        }
    });
    match llegados {
        Ok(cosas) => Estado::Hecho {
            de: oferta.de.clone(),
            guardados: crate::recibir::guardar_lo_recibido(&cx.raiz, &cx.id, &cosas, &como_nuevo),
        },
        Err(e) => Estado::Fallo(Falla::Cortado(e.to_string())),
    }
}

/// «Que me envien a mi» (`esperarAQueMeManden` del movil): se abre la
/// puerta, se anuncia en la red y se espera. Quien llama es el otro, asi
/// que el canal lo empieza el (`inicia = false`).
///
/// En un puerto cualquiera y no en el de siempre: el de siempre ya lo tiene
/// la escucha de la sincronizacion de esta misma ventana.
fn esperar_a_que_me_manden(
    cx: &Contexto,
    turno: u64,
    vivo: Arc<AtomicBool>,
    ocupado: Arc<AtomicBool>,
) -> anyhow::Result<Mio> {
    use anyhow::Context as _;
    let codigo = crate::recibir::codigo_nuevo().context("el sistema no dio azar")?;
    let escucha = TcpListener::bind(("0.0.0.0", 0))?;
    escucha.set_nonblocking(true)?;
    let puerto = escucha.local_addr()?.port();
    let ip = crate::recibir::ip_local().unwrap_or_default();
    let qr = crate::recibir::qr_de(&envio::texto_del_qr_de_recepcion(&codigo, &ip, puerto));
    let cx = cx.clone();
    let para_hilo = codigo.clone();
    std::thread::Builder::new()
        .name("recibir-wifi-espera".into())
        .spawn(move || {
            let codigo = para_hilo;
            // La etiqueta cuesta 60 000 vueltas de PBKDF2: en el hilo, no en
            // la ventana. Y anunciar tarda casi un segundo en Windows.
            let etiqueta = envio::etiqueta(&codigo);
            let corto: String = cx.nombre.chars().take(40).collect();
            let anuncio = pixpin_shell::mdns::anunciar(
                envio::SERVICIO_RECIBIR,
                &format!("PixPin {}", cx.nombre),
                puerto,
                &[("g", &etiqueta), ("n", &corto)],
            );
            if let Err(e) = &anuncio {
                tracing::warn!(?e, "no se pudo anunciar la espera");
            }
            while vivo.load(Ordering::SeqCst) {
                match escucha.accept() {
                    Ok((flujo, de)) => {
                        if ocupado.swap(true, Ordering::SeqCst) {
                            // Ya se esta recibiendo algo: el otro vera el
                            // corte y podra volver a probar.
                            continue;
                        }
                        tracing::info!(%de, "llaman para enviarme algo");
                        let _ = flujo.set_nonblocking(false);
                        let _ = flujo.set_read_timeout(Some(ESPERA_LARGA));
                        let fin = match nonce() {
                            None => Estado::Fallo(Falla::Cortado("el sistema no dio azar".into())),
                            Some(n) => match Receptor::conectar(
                                flujo, &codigo, &cx.nombre, &cx.id, n, false,
                            ) {
                                Ok(r) => tramitar(r, &cx, turno),
                                Err(e) => Estado::Fallo(Falla::Cortado(e.to_string())),
                            },
                        };
                        avisar(&cx, turno, fin);
                        ocupado.store(false, Ordering::SeqCst);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(150));
                    }
                    Err(e) => {
                        tracing::warn!(?e, "la espera de recibir fallo");
                        return;
                    }
                }
            }
            drop(anuncio);
        })?;
    Ok(Mio { codigo, qr })
}

// ------------------------------------------------------------------ pintar

/// El `OutlinedTextField` grande del codigo: cifras en grande y centradas.
pub(super) fn campo_grande(
    l: &mut Lienzo<'_>,
    r: pixpin_render::RectF,
    texto: &str,
    pista: &str,
    a: Accion,
) {
    let (p, e) = (l.p, l.e);
    p.rellenar_redondeado(r, 6.0 * e, PRIMARIO);
    p.rellenar_redondeado(menos(r, 2.0 * e), 5.0 * e, FONDO);
    let (mostrar, color, tam) = if texto.is_empty() {
        (pista.to_string(), SUAVE, 26.0 * e)
    } else if texto.chars().all(|c| c.is_ascii_digit() || c == ' ') {
        (envio::legible(&envio::limpiar(texto)), TEXTO, 26.0 * e)
    } else {
        // Un QR pegado es largo: mas pequeno, para que se lea entero.
        (texto.to_string(), TEXTO, 13.0 * e)
    };
    let (w, h) = p.medir_texto(&mostrar, tam);
    let x = if w <= r.ancho - 24.0 * e {
        r.x + (r.ancho - w) / 2.0
    } else {
        r.x + 12.0 * e
    };
    p.texto_linea(
        &mostrar,
        x,
        r.y + (r.alto - h) / 2.0,
        tam,
        r.ancho - 24.0 * e,
        color,
    );
    if !texto.is_empty() {
        let xc = (x + w + 2.0 * e).min(r.x + r.ancho - 10.0 * e);
        p.rellenar(
            rect(xc, r.y + 14.0 * e, 2.0 * e, r.alto - 28.0 * e),
            PRIMARIO,
        );
    }
    l.zonas.push((r, a));
}

/// Un `Button` apagado (`enabled = false`): se ve, pero no se pulsa.
pub(super) fn boton_apagado(l: &Lienzo<'_>, r: pixpin_render::RectF, texto: &str) {
    let (p, e) = (l.p, l.e);
    p.rellenar_redondeado(r, r.alto / 2.0, VARIANTE);
    let tam = 14.0 * e;
    let (w, h) = p.medir_texto(texto, tam);
    p.texto(
        texto,
        r.x + (r.ancho - w) / 2.0,
        r.y + (r.alto - h) / 2.0,
        tam,
        CONTORNO,
    );
}

/// Una `Caja` con el icono de la Wi-Fi y un texto: la del movil.
pub(super) fn caja_wifi(
    l: &Lienzo<'_>,
    x: f32,
    y: f32,
    w: f32,
    titulo: Option<&str>,
    texto: &str,
) -> f32 {
    let (p, e) = (l.p, l.e);
    let xt = x + 16.0 * e + 34.0 * e;
    let wt = w - 32.0 * e - 34.0 * e;
    let ht = titulo.map_or(0.0, |_| 22.0 * e);
    let tam = if titulo.is_some() { 12.0 } else { 14.0 } * e;
    let h = 16.0 * e + ht + l.alto_de(texto, tam, wt) + 16.0 * e;
    caja(p, rect(x, y, w, h), e);
    let lado = 24.0 * e;
    p.icono(
        &material::WIFI,
        rect(x + 16.0 * e, y + 14.0 * e, lado, lado),
        PRIMARIO,
    );
    if let Some(t) = titulo {
        p.texto_linea(t, xt, y + 16.0 * e, 15.0 * e, wt, TEXTO);
    }
    l.parrafo(texto, xt, y + 16.0 * e + ht, wt, tam, TEXTO);
    h
}

fn con(textos: &Catalogo, clave: &str, pares: &[(&str, String)]) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    for (k, v) in pares {
        args.set(*k, v.clone());
    }
    textos.t_args(clave, &args)
}

pub(super) fn nombre_de(textos: &Catalogo, tipo: &str, nombre: &str) -> String {
    match tipo {
        envio::PROYECTO => con(textos, "rw-proyecto", &[("nombre", nombre.to_string())]),
        envio::LIENZO => con(textos, "rw-lienzo", &[("nombre", nombre.to_string())]),
        _ => nombre.to_string(),
    }
}

pub(super) fn tamano(bytes: u64) -> String {
    pixpin_ui::chat::tamano_corto(bytes)
}

/// «Ya tienes «X» con los mismos códigos».
fn ya_tienes(textos: &Catalogo, suyo: &str) -> String {
    con(textos, "rw-ya-tienes", &[("nombre", suyo.to_string())])
}

/// Que va a pasar con lo elegido, en una linea, como en el movil.
fn explicacion(textos: &Catalogo, nuevo: bool) -> String {
    textos.t(if nuevo {
        "rw-entra-aparte"
    } else {
        "rw-encima-de-lo-tuyo"
    })
}

/// Una de dos opciones, como pastilla (`Eleccion` de `RecibirActivity.kt`).
///
/// Pastilla y no boton redondo del todo: son dos opciones de las que siempre
/// hay una puesta, no dos acciones que se disparan.
fn pastilla(l: &mut Lienzo<'_>, r: pixpin_render::RectF, texto: &str, puesta: bool, a: Accion) {
    let (p, e) = (l.p, l.e);
    p.rellenar_redondeado(r, 10.0 * e, if puesta { PRIMARIO } else { VARIANTE });
    let tam = 13.0 * e;
    let color = if puesta {
        super::SOBRE_PRIMARIO
    } else {
        CONTORNO
    };
    let (w, h) = p.medir_texto(texto, tam);
    let x = if w <= r.ancho - 12.0 * e {
        r.x + (r.ancho - w) / 2.0
    } else {
        r.x + 6.0 * e
    };
    p.texto_linea(
        texto,
        x,
        r.y + (r.alto - h) / 2.0,
        tam,
        r.ancho - 12.0 * e,
        color,
    );
    l.zonas.push((r, a));
}

/// Pinta la pantalla desde `y` y devuelve donde acaba.
pub(super) fn pintar(
    l: &mut Lienzo<'_>,
    textos: &Catalogo,
    r: &Recibir,
    x: f32,
    w: f32,
    mut y: f32,
) -> f32 {
    let (p, e) = (l.p, l.e);
    let t = |a: Toque| Accion::Recibir(a);
    y += 8.0 * e;
    match &r.estado {
        Estado::Pidiendo => {
            y += caja_wifi(l, x, y, w, None, &textos.t("rw-misma-wifi"));
            y += 18.0 * e;
            l.centrado(&textos.t("rw-o-escribe"), x, w, y, 14.0 * e, SUAVE);
            y += 26.0 * e;
            let wc = w.min(260.0 * e);
            campo_grande(
                l,
                rect(x + (w - wc) / 2.0, y, wc, 64.0 * e),
                &r.tecleado,
                "000 000",
                t(Toque::Campo),
            );
            y += 72.0 * e;
            y += l.parrafo_centrado(&textos.t("rw-pista-pega"), x, y, w, 12.0 * e, SUAVE);
            y += 12.0 * e;
            let rb = rect(x, y, w, 48.0 * e);
            if r.destino().is_some() {
                l.boton(rb, &textos.t("rw-recibir"), None, true, t(Toque::Recibir));
            } else {
                boton_apagado(l, rb, &textos.t("rw-recibir"));
            }
            y += 48.0 * e + 20.0 * e;
            let wd = w.min(320.0 * e);
            p.rellenar(rect(x + (w - wd) / 2.0, y, wd, e.max(1.0)), super::BORDE);
            y += 16.0 * e;
            match &r.mio {
                None => {
                    y += l.parrafo_centrado(&textos.t("rw-prefieres"), x, y, w, 14.0 * e, TEXTO);
                    y += 10.0 * e;
                    l.boton(
                        rect(x, y, w, 48.0 * e),
                        &textos.t("rw-ensenar"),
                        None,
                        false,
                        t(Toque::EnsenarMio),
                    );
                    y += 48.0 * e;
                }
                Some(m) => {
                    l.centrado(&textos.t("rw-que-te-escaneen"), x, w, y, 16.0 * e, TEXTO);
                    y += 30.0 * e;
                    let lado = w.min(280.0 * e);
                    let rq = rect(x + (w - lado) / 2.0, y, lado, lado);
                    p.rellenar_redondeado(rq, 18.0 * e, hex_blanco());
                    if let Some(qr) = &m.qr {
                        crate::recibir::pintar_qr(p, menos(rq, 10.0 * e), qr);
                    }
                    y += lado + 8.0 * e;
                    l.centrado(&envio::legible(&m.codigo), x, w, y, 24.0 * e, TEXTO);
                    y += 36.0 * e;
                    y +=
                        l.parrafo_centrado(&textos.t("rw-esperando-mio"), x, y, w, 12.0 * e, SUAVE);
                }
            }
        }
        Estado::Buscando(b) => {
            y += 40.0 * e;
            l.girando(x + w / 2.0, y + 20.0 * e, 18.0 * e, 4.0 * e);
            y += 54.0 * e;
            let texto = match b {
                Busca::Buscando => textos.t("rw-buscando"),
                Busca::Conectando => textos.t("rw-conectando"),
            };
            y += l.parrafo_centrado(&texto, x, y, w, 14.0 * e, TEXTO);
        }
        Estado::Oferta {
            oferta,
            sustituye,
            como_nuevo,
            ..
        } => {
            y += 12.0 * e;
            y += l.parrafo_centrado(
                &con(textos, "rw-quiere", &[("de", oferta.de.clone())]),
                x,
                y,
                w,
                20.0 * e,
                TEXTO,
            );
            if !oferta.de_codigo.is_empty() {
                l.centrado(
                    &con(
                        textos,
                        "rw-aparato",
                        &[("codigo", oferta.de_codigo.clone())],
                    ),
                    x,
                    w,
                    y,
                    12.0 * e,
                    SUAVE,
                );
                y += 18.0 * e;
            }
            y += 12.0 * e;
            // La caja se mide antes de pintarla: lo que ya se tiene lleva
            // debajo su aviso y sus dos pastillas, y sin medirlo primero el
            // fondo quedaria mas corto que el contenido.
            let wi = w - 32.0 * e;
            let alto_de_uno = |el: &envio::Elemento| {
                let mut h = 30.0 * e;
                if let Some(suyo) = sustituye.get(&el.identidad) {
                    h += l.alto_de(&ya_tienes(textos, suyo), 12.0 * e, wi) + 6.0 * e;
                    h += 34.0 * e + 6.0 * e;
                    let nuevo = como_nuevo.get(&el.identidad).copied().unwrap_or(false);
                    h += l.alto_de(&explicacion(textos, nuevo), 12.0 * e, wi) + 8.0 * e;
                }
                h
            };
            let hc = 16.0 * e + oferta.elementos.iter().map(alto_de_uno).sum::<f32>() + 6.0 * e;
            caja(p, rect(x, y, w, hc), e);
            let mut yf = y + 16.0 * e;
            for el in &oferta.elementos {
                let peso = tamano(el.bytes.max(0) as u64);
                let (wp, _) = p.medir_texto(&peso, 12.0 * e);
                p.texto(&peso, x + w - 16.0 * e - wp, yf + 2.0 * e, 12.0 * e, TEXTO);
                p.texto_linea(
                    &nombre_de(textos, &el.tipo, &el.nombre),
                    x + 16.0 * e,
                    yf,
                    15.0 * e,
                    w - 44.0 * e - wp,
                    TEXTO,
                );
                yf += 30.0 * e;
                let Some(suyo) = sustituye.get(&el.identidad) else {
                    continue;
                };
                // **Lo que ya se tiene**: se dice y se elige, cosa por cosa,
                // como en `RecibirActivity.Oferta` del movil.
                yf += l.parrafo(
                    &ya_tienes(textos, suyo),
                    x + 16.0 * e,
                    yf,
                    wi,
                    12.0 * e,
                    PRIMARIO,
                ) + 6.0 * e;
                let nuevo = como_nuevo.get(&el.identidad).copied().unwrap_or(false);
                let media = (wi - 8.0 * e) / 2.0;
                pastilla(
                    l,
                    rect(x + 16.0 * e, yf, media, 34.0 * e),
                    &textos.t("rw-actualizar"),
                    !nuevo,
                    t(Toque::Elegir(el.identidad.clone(), false)),
                );
                pastilla(
                    l,
                    rect(x + 16.0 * e + media + 8.0 * e, yf, media, 34.0 * e),
                    &textos.t("rw-como-nuevo"),
                    nuevo,
                    t(Toque::Elegir(el.identidad.clone(), true)),
                );
                yf += 34.0 * e + 6.0 * e;
                yf += l.parrafo(
                    &explicacion(textos, nuevo),
                    x + 16.0 * e,
                    yf,
                    wi,
                    12.0 * e,
                    SUAVE,
                ) + 8.0 * e;
            }
            y += hc + 10.0 * e;
            if sustituye.is_empty() {
                y += l.parrafo_centrado(&textos.t("rw-nada-coincide"), x, y, w, 12.0 * e, SUAVE);
            }
            y += 20.0 * e;
            let medio = (w - 10.0 * e) / 2.0;
            l.boton(
                rect(x, y, medio, 48.0 * e),
                &textos.t("rw-no-gracias"),
                None,
                false,
                t(Toque::NoGracias),
            );
            l.boton(
                rect(x + medio + 10.0 * e, y, medio, 48.0 * e),
                &textos.t("rw-aceptar"),
                None,
                true,
                t(Toque::Aceptar),
            );
            y += 48.0 * e;
        }
        Estado::Recibiendo { de, hechos, total } => {
            y += 40.0 * e;
            l.centrado(
                &con(textos, "rw-recibiendo", &[("de", de.clone())]),
                x,
                w,
                y,
                16.0 * e,
                TEXTO,
            );
            y += 34.0 * e;
            let parte = if *total > 0 {
                *hechos as f32 / *total as f32
            } else {
                0.0
            };
            l.barra(rect(x, y, w, 4.0 * e), parte);
            y += 10.0 * e;
            l.centrado(
                &con(
                    textos,
                    "rw-de",
                    &[("hechos", tamano(*hechos)), ("total", tamano(*total))],
                ),
                x,
                w,
                y,
                12.0 * e,
                TEXTO,
            );
            y += 20.0 * e;
        }
        Estado::Hecho { de, guardados } => {
            y += 30.0 * e;
            let lado = 64.0 * e;
            p.icono(
                &material::CHECK_CIRCLE,
                rect(x + (w - lado) / 2.0, y, lado, lado),
                VERDE,
            );
            y += lado + 10.0 * e;
            y += l.parrafo_centrado(
                &con(textos, "rw-recibido", &[("de", de.clone())]),
                x,
                y,
                w,
                20.0 * e,
                TEXTO,
            );
            y += 10.0 * e;
            if guardados.is_empty() {
                y += l.parrafo_centrado(&textos.t("rw-no-guardado"), x, y, w, 14.0 * e, ERROR);
            }
            for g in guardados {
                let clave = match g.donde {
                    Donde::Proyecto => "rw-en-proyectos",
                    Donde::ProyectoAlDia => "rw-proyecto-al-dia",
                    Donde::Guardados => "rw-en-guardados",
                    Donde::GuardadosAlDia => "rw-guardados-al-dia",
                    Donde::Carpeta => "rw-en-carpeta",
                };
                y += l.parrafo_centrado(
                    &con(textos, clave, &[("nombre", g.nombre.clone())]),
                    x,
                    y,
                    w,
                    14.0 * e,
                    TEXTO,
                ) + 4.0 * e;
            }
            y += 20.0 * e;
            l.boton(
                rect(x, y, w, 48.0 * e),
                &textos.t("rw-cerrar"),
                None,
                true,
                t(Toque::Cerrar),
            );
            y += 48.0 * e;
        }
        Estado::Fallo(f) => {
            y += 40.0 * e;
            l.centrado(&textos.t("rw-no-se-pudo"), x, w, y, 20.0 * e, TEXTO);
            y += 36.0 * e;
            let texto = match f {
                Falla::NoEncontrado => textos.t("rw-no-encontrado"),
                Falla::CodigoMalo => textos.t("rw-codigo-malo"),
                Falla::Cortado(m) => con(textos, "rw-se-corto", &[("motivo", m.clone())]),
                Falla::Dijo(m) => m.clone(),
            };
            y += l.parrafo_centrado(&texto, x, y, w, 14.0 * e, TEXTO);
            y += 20.0 * e;
            l.boton(
                rect(x, y, w, 48.0 * e),
                &textos.t("rw-probar-otra-vez"),
                None,
                true,
                t(Toque::ProbarOtraVez),
            );
            y += 48.0 * e;
        }
    }
    y
}

/// El papel del QR: blanco de verdad, como el `Color.White` del movil.
pub(super) fn hex_blanco() -> pixpin_render::Color {
    crate::caja_dibujo::hex(0xffffff)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn para_recibir_vale_el_qr_de_quien_envia_y_no_el_de_quien_espera() {
        let mut r = Recibir::nuevo(1);
        r.tecleado = "482 913".into();
        assert!(r.destino().is_some());
        r.tecleado = "pixpin-envio:1:482913:192.168.1.4:47474".into();
        assert_eq!(r.destino().and_then(|q| q.host), Some("192.168.1.4".into()));
        // Caso negativo: el QR de quien espera recibir es para mandarle.
        r.tecleado = "pixpin-recibe:1:482913:192.168.1.4:47474".into();
        assert!(r.destino().is_none());
        r.tecleado = "48291".into();
        assert!(r.destino().is_none());
    }

    #[test]
    fn un_corte_al_saludar_se_cuenta_como_codigo_equivocado() {
        let eof = envio::ErrorEnvio::Canal(pixpin_sincro::canal::ErrorCanal::Io(
            std::io::ErrorKind::UnexpectedEof.into(),
        ));
        assert_eq!(falla_al_conectar(&eof), Falla::CodigoMalo);
        let dijo = envio::ErrorEnvio::Dijo("no".into());
        assert_eq!(falla_al_conectar(&dijo), Falla::Dijo("no".into()));
    }

    #[test]
    fn al_salir_de_la_pantalla_la_puerta_se_cierra() {
        let r = Recibir::nuevo(1);
        let vivo = r.vivo.clone();
        drop(r);
        assert!(!vivo.load(Ordering::SeqCst));
    }
}
