//! **La baliza con sockets de verdad** (28-oct-2026): un «PC» que empieza a
//! grabar y un «movil» que lo oye por la difusion de la red de loopback, con
//! los tiempos medidos. Lo que pidio el usuario se mide aqui: que el movil
//! vea «PC grabando» en unos 2 s (aqui, con el aviso al momento, en
//! milisegundos; sin el, como mucho un latido de [`CADA_GRABANDO_MS`]) y que
//! la llamada para subir una foto empiece enseguida.

use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use crate::baliza::*;

fn clave() -> [u8; 32] {
    crate::codigo::clave_de_grupo("K7Q2M9XMPA")
}

fn ahora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn loopback() -> Interfaz {
    Interfaz {
        ip: Ipv4Addr::LOCALHOST,
        prefijo: 8,
    }
}

fn ms(desde: Instant) -> i64 {
    desde.elapsed().as_millis() as i64
}

/// El «PC»: manda balizas al ritmo de [`Ritmo`] mientras `vivo`, grabando
/// desde que `graba` se pone (y con `cambio`, al momento).
fn pc(
    puerto_baliza: u16,
    puerto_tcp: u16,
    graba: Arc<AtomicBool>,
    avisar_al_momento: bool,
    vivo: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let emisor = Emisor::nuevo(puerto_baliza, &[loopback()]).unwrap();
        let mut ritmo = Ritmo::default();
        let empezo = Instant::now();
        let mut antes = false;
        let mut secuencia = 0;
        while vivo.load(Ordering::SeqCst) {
            let g = graba.load(Ordering::SeqCst);
            let cambio = avisar_al_momento && g != antes;
            antes = g;
            if ritmo.toca(ms(empezo), g, cambio) {
                secuencia += 1;
                let b = Baliza {
                    etiqueta: etiqueta_cruda(&clave()),
                    id: "pc".into(),
                    puerto: puerto_tcp,
                    reloj: ahora(),
                    secuencia,
                    banderas: if g { GRABANDO | CAMARA } else { 0 },
                    grabacion: if g { "g-1".into() } else { String::new() },
                    ms_actual: 0,
                };
                emisor.difundir(&b.escribir(&clave()));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    })
}

/// El «movil»: oye hasta la primera baliza que diga «grabando» y la cuenta
/// con su origen.
fn movil(oyente: Oyente, tx: mpsc::Sender<(Instant, Baliza, SocketAddr)>) {
    std::thread::spawn(move || {
        let mut buf = [0u8; TOPE];
        let mut oidas = Oidas::default();
        let tope = Instant::now() + Duration::from_secs(10);
        while Instant::now() < tope {
            if let Ok(Some((n, de))) = oyente.oir(&mut buf)
                && let Ok(b) = Baliza::leer(&buf[..n], &clave())
                && oidas.admitir(&b, ahora())
                && b.graba()
            {
                let _ = tx.send((Instant::now(), b, de));
                return;
            }
        }
    });
}

fn medir(avisar_al_momento: bool, tras_reposo_ms: u64) -> (Duration, Baliza, SocketAddr) {
    let oyente = Oyente::abrir(0, &[loopback()]).unwrap();
    let puerto = oyente.puerto().unwrap();
    let (tx, rx) = mpsc::channel();
    movil(oyente, tx);
    let graba = Arc::new(AtomicBool::new(false));
    let vivo = Arc::new(AtomicBool::new(true));
    let hilo = pc(
        puerto,
        47474,
        graba.clone(),
        avisar_al_momento,
        vivo.clone(),
    );
    // Sin grabar, el PC ya mando su primera baliza de reposo: la siguiente
    // no toca hasta dentro de 15 s.
    std::thread::sleep(Duration::from_millis(tras_reposo_ms));
    let empezo = Instant::now();
    graba.store(true, Ordering::SeqCst);
    let (oida, b, de) = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("el movil no oyo la baliza");
    vivo.store(false, Ordering::SeqCst);
    hilo.join().unwrap();
    (oida - empezo, b, de)
}

#[test]
fn el_movil_ve_que_el_pc_graba_al_momento() {
    let (latencia, b, de) = medir(true, 300);
    eprintln!("baliza: «PC grabando» oido en {latencia:?} (aviso al momento)");
    assert!(latencia < Duration::from_millis(500), "{latencia:?}");
    assert_eq!(b.id, "pc");
    assert_eq!(b.grabacion, "g-1");
    assert_eq!(de.ip().to_string(), "127.0.0.1", "de donde llamar al PC");
}

#[test]
fn sin_aviso_al_momento_basta_el_latido_de_un_segundo() {
    // El peor caso: el cambio no se avisa y se espera al siguiente latido.
    // Grabando va cada segundo, asi que como mucho ~1 s (lejos de los 2 s).
    let (latencia, ..) = medir(false, 300);
    eprintln!("baliza: «PC grabando» oido en {latencia:?} (solo el latido)");
    assert!(latencia < Duration::from_millis(2_000), "{latencia:?}");
}

#[test]
fn quien_oye_la_baliza_llama_enseguida_por_tcp() {
    // El PC escucha la sincronizacion; el movil, con la baliza, sabe donde
    // y llama sin buscar por mDNS ni esperar a la sonda.
    let escucha = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let puerto_tcp = escucha.local_addr().unwrap().port();
    let oyente = Oyente::abrir(0, &[loopback()]).unwrap();
    let puerto = oyente.puerto().unwrap();
    let (tx, rx) = mpsc::channel();
    movil(oyente, tx);
    let graba = Arc::new(AtomicBool::new(false));
    let vivo = Arc::new(AtomicBool::new(true));
    let hilo = pc(puerto, puerto_tcp, graba.clone(), true, vivo.clone());
    let empezo = Instant::now();
    graba.store(true, Ordering::SeqCst);
    let (_, b, de) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let destino = SocketAddr::new(de.ip(), b.puerto);
    let _flujo = TcpStream::connect_timeout(&destino, Duration::from_secs(1)).unwrap();
    let (_, _) = escucha.accept().unwrap();
    let total = empezo.elapsed();
    vivo.store(false, Ordering::SeqCst);
    hilo.join().unwrap();
    eprintln!("baliza: de empezar a grabar a la conexion del movil, {total:?}");
    assert!(total < Duration::from_secs(1), "{total:?}");
}

#[test]
fn directa_llega_aunque_no_haya_difusion() {
    // Una Wi-Fi que filtra la difusion: la baliza directa a la ultima
    // direccion conocida (o al barrido) llega igual.
    let oyente = Oyente::abrir(0, &[]).unwrap();
    let puerto = oyente.puerto().unwrap();
    let emisor = Emisor::nuevo(puerto, &[]).unwrap();
    let b = Baliza {
        etiqueta: etiqueta_cruda(&clave()),
        id: "movil".into(),
        puerto: 47474,
        reloj: ahora(),
        secuencia: 1,
        banderas: PREGUNTA,
        ..Default::default()
    };
    let empezo = Instant::now();
    assert_eq!(emisor.a(&b.escribir(&clave()), &[Ipv4Addr::LOCALHOST]), 1);
    let mut buf = [0u8; TOPE];
    let (n, _) = loop {
        if let Some(x) = oyente.oir(&mut buf).unwrap() {
            break x;
        }
        assert!(empezo.elapsed() < Duration::from_secs(2), "no llego");
    };
    let leida = Baliza::leer(&buf[..n], &clave()).unwrap();
    assert!(leida.pregunta() && !leida.graba());
    // Caso negativo: lo de otro grupo por el mismo puerto no se cree.
    let otra = crate::codigo::clave_de_grupo("K7Q2M9XMPB");
    assert_eq!(Baliza::leer(&buf[..n], &otra), Err(Rechazo::OtroGrupo));
}

#[test]
fn mientras_se_graba_la_camara_va_primero_y_al_acabar_la_grabacion() {
    // Una cola de trabajos que llegan mientras alguien graba, despachada por
    // `que_toca` como lo hace la automatica del PC: grabando solo sale lo de
    // la camara; al acabar, los ficheros de la grabacion antes que nada.
    use Trabajo::*;
    let mut espera = vec![VueltaEntera, Personas, Camara, Tareas, Camara];
    let mut hechos = Vec::new();
    let mut grabando = true;
    let empezo = Instant::now();
    for paso in 0..8 {
        if paso == 3 {
            // Termina la grabacion: entra lo suyo.
            grabando = false;
            espera.push(ArchivosDeLaGrabacion);
        }
        // Grabando y sin nada de la camara (el paso 2): no sale nada.
        if let Some(t) = que_toca(&espera, grabando) {
            let i = espera.iter().position(|x| *x == t).unwrap();
            espera.remove(i);
            hechos.push((t, grabando));
        }
    }
    let orden: Vec<Trabajo> = hechos.iter().map(|(t, _)| *t).collect();
    eprintln!("prioridad: {orden:?} en {:?}", empezo.elapsed());
    assert_eq!(
        orden,
        [
            Camara,
            Camara,
            ArchivosDeLaGrabacion,
            Tareas,
            VueltaEntera,
            Personas
        ]
    );
    assert!(
        hechos.iter().all(|(t, g)| !*g || *t == Camara),
        "grabando no salio nada mas que la camara"
    );
}
