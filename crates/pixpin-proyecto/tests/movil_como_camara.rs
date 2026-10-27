//! **El movil como camara de la reunion que graba el PC**, de punta a punta
//! (27-oct-2026): por TCP, con el canal cifrado del grupo y el `Respondedor`
//! de verdad del PC.
//!
//! Un movil de mentira (el lado que tiene que copiar Android, ver
//! `docs/movil-como-camara.md`) pregunta como va la grabacion, lleva su
//! propio reloj de la grabacion con lo que le dice el PC y manda fotos y un
//! clip marcados con su minuto, durante una grabacion con una pausa y con
//! su reloj 7 s adelantado. El PC las guarda en la cola mientras graba, y
//! al «Terminar» entran en la clase en su minuto: en la nota, en su carpeta
//! y con sus bytes tal cual (el EXIF incluido). Una que sube tarde entra
//! sola en la clase ya guardada.

use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use pixpin_proyecto::almacen::{self, Ficha, Indice};
use pixpin_proyecto::camara_del_movil::{Camaras, Destino, Recepcion};
use pixpin_proyecto::carpeta_de_paginas::{self, Carpeta};
use pixpin_proyecto::cuaderno::{self, Clase, Cuaderno, Mensaje, Sello};
use pixpin_proyecto::identidad::{Aparato, Identidad};
use pixpin_proyecto::{clase, vista::DiscoPc};
use pixpin_sincro::al_lienzo::{Conexion, ErrorAlLienzo, Recibe};
use pixpin_sincro::camara::{self, Anuncio, FotoDeCamara, Reloj};
use pixpin_sincro::canal::{Canal, Tipo};
use pixpin_sincro::mensajes as m;
use pixpin_sincro::protocolo::Respondedor;

const CODIGO: &str = "CAMA2345";
const S: i64 = 1000;
const PARED: i64 = 1_761_550_000_000;
/// Lo que va adelantado el reloj del movil.
const ADELANTO: i64 = 7 * S;

/// El reloj del PC, que mueve la prueba.
static RELOJ: AtomicI64 = AtomicI64::new(PARED);

fn ahora_pc() -> i64 {
    RELOJ.load(Ordering::SeqCst)
}

fn ahora_movil() -> i64 {
    ahora_pc() + ADELANTO
}

/// Las pruebas comparten el reloj: una a la vez.
static UNA_A_LA_VEZ: Mutex<()> = Mutex::new(());

fn turno() -> std::sync::MutexGuard<'static, ()> {
    UNA_A_LA_VEZ.lock().unwrap_or_else(|e| e.into_inner())
}

fn a_las(segundos: i64) {
    RELOJ.store(PARED + segundos * S, Ordering::SeqCst);
}

static HORA: AtomicI64 = AtomicI64::new(PARED + 10_000_000);

fn hora_sin_repetir() -> i64 {
    HORA.fetch_add(1, Ordering::SeqCst)
}

/// Una foto JPEG pequena con su EXIF (APP1): lo que tiene que llegar tal
/// cual.
fn jpeg(semilla: u8) -> Vec<u8> {
    let mut b = vec![0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x10];
    b.extend_from_slice(b"Exif\0\0MM\0*\0\0\0\x08");
    b.extend((0..3000u32).map(|i| (i as u8).wrapping_mul(semilla)));
    b.extend_from_slice(&[0xFF, 0xD9]);
    b
}

fn mp4() -> Vec<u8> {
    let mut b = b"\0\0\0\x18ftypmp42\0\0\0\0mp42isom".to_vec();
    b.extend(std::iter::repeat_n(7u8, 5000));
    b
}

fn fichero(dir: &Path, nombre: &str, bytes: &[u8]) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let f = dir.join(nombre);
    std::fs::write(&f, bytes).unwrap();
    f
}

fn ficha(id: &str, nombre: &str, letra: &str) -> Aparato {
    Aparato {
        id: id.into(),
        nombre: nombre.into(),
        letra: Some(letra.into()),
        ..Default::default()
    }
}

fn al_cable(a: &Aparato) -> m::Aparato {
    m::Aparato {
        id: a.id.clone(),
        nombre: a.nombre.clone(),
        letra: a.letra.clone(),
        desde: 0,
    }
}

struct Pc {
    raiz: PathBuf,
    camaras: Arc<Mutex<Camaras>>,
}

impl Drop for Pc {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.raiz);
    }
}

fn pc(nombre: &str) -> Pc {
    let raiz = std::env::temp_dir().join(format!("pixpin-camara-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();
    Identidad {
        yo: ficha("pc", "Portatil", "A"),
        codigo: Some(CODIGO.into()),
        miembros: vec![ficha("pc", "Portatil", "A"), ficha("tel", "Telefono", "B")],
        ..Default::default()
    }
    .guardar(&raiz)
    .unwrap();
    let mut c = Camaras::nuevas(raiz.join("cola-camara"), "Portatil");
    c.codecs = vec![camara::H264.into(), camara::HEVC.into()];
    Pc {
        raiz,
        camaras: Arc::new(Mutex::new(c)),
    }
}

/// El PC escuchando UNA conexion con su `Respondedor`: con camara si
/// `con_camara`; si no, como un PC de antes.
fn escuchar(pc: &Pc, con_camara: bool) -> (u16, std::thread::JoinHandle<()>) {
    let escucha = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let puerto = escucha.local_addr().unwrap().port();
    let raiz = pc.raiz.clone();
    let camaras = pc.camaras.clone();
    let hilo = std::thread::spawn(move || {
        let (flujo, _) = escucha.accept().unwrap();
        let disco = DiscoPc::nuevo(&raiz);
        let fabrica = |otro: &m::Aparato| -> Box<dyn Recibe> {
            Box::new(Recepcion {
                camaras: camaras.clone(),
                de_id: otro.id.clone(),
                de_nombre: otro.nombre.clone(),
                ahora: ahora_pc,
            })
        };
        let r = Respondedor {
            disco: &disco,
            estado: &|_| {},
            ahora: &ahora_pc,
            mi_puerto: 0,
            al_saludar: &|_, _| {},
            suelto: if con_camara { Some(&fabrica) } else { None },
        };
        let capacidades: &[&str] = if con_camara {
            &pixpin_sincro::CAPACIDADES_CON_CAMARA
        } else {
            &pixpin_sincro::CAPACIDADES
        };
        r.atender_con(flujo, [5; 32], None, capacidades).unwrap();
    });
    (puerto, hilo)
}

/// El movil llama al PC, como para mandarle algo suelto.
fn llamar(puerto: u16) -> Conexion<TcpStream> {
    let yo = ficha("tel", "Telefono", "B");
    let hola = m::Hola {
        yo: al_cable(&yo),
        miembros: vec![al_cable(&ficha("pc", "Portatil", "A")), al_cable(&yo)],
        reloj: ahora_movil(),
        capacidades: vec![camara::CAPACIDAD.into()],
        ..Default::default()
    };
    let flujo = TcpStream::connect(("127.0.0.1", puerto)).unwrap();
    Conexion::abrir(flujo, CODIGO, [6; 32], hola).unwrap()
}

/// **El reloj de la grabacion en el movil**: lo que le dijo el PC (en que
/// minuto iba y si estaba en pausa) y lo que paso desde entonces en SU
/// reloj. Es lo que tiene que hacer Android.
struct RelojDelMovil {
    reloj: Reloj,
}

impl RelojDelMovil {
    fn de(a: &Anuncio) -> RelojDelMovil {
        let mut r = Reloj::empezar(ahora_movil(), a.ms_actual);
        if a.en_pausa {
            r.pausar(ahora_movil());
        }
        RelojDelMovil { reloj: r }
    }

    fn al_dia(&mut self, a: &Anuncio) {
        self.reloj
            .seguir_a(ahora_movil(), a.ms_actual, a.en_pausa, 250);
    }

    fn ms(&self) -> i64 {
        self.reloj.ms_en(ahora_movil())
    }
}

fn leer(raiz: &Path, proyecto: &str) -> Vec<Mensaje> {
    Cuaderno::leer_de(&almacen::carpeta(raiz, proyecto))
        .unwrap()
        .mensajes
}

/// Lo que hace `grabar_clase::guardar` al «Terminar»: la nota de voz y su
/// carpeta.
fn guardar_la_clase(raiz: &Path, duracion: i64) -> (String, String, String) {
    let f = Ficha::nueva("Reunion", PARED, "K7Q2");
    let mut i = Indice::leer(raiz);
    i.proyectos.push(f.clone());
    i.guardar(raiz).unwrap();
    let carpeta = almacen::carpeta(raiz, &f.id);
    let sello = |n| Sello {
        cuando: hora_sin_repetir(),
        numero: n,
        aparato: "K7Q2".into(),
        proyecto: f.id.clone(),
    };
    let ruta = almacen::guardar_adjunto(raiz, &f.id, "Clase.m4a", b"m4a").unwrap();
    let mut voz = Mensaje::adjunto(Clase::Voz, "Clase.m4a", &ruta, 3, &sello(1));
    voz.duracion_ms = duracion;
    voz.resto
        .insert("grabacion".into(), serde_json::json!("clase"));
    cuaderno::anadir(&carpeta, &voz).unwrap();
    let mut c = Carpeta::grabacion(&voz.id, duracion);
    c.anadir_al_minuto(&voz.id, "", 0);
    let m = carpeta_de_paginas::mensaje_de_grabacion("Clase", &c, &sello(2));
    cuaderno::anadir(&carpeta, &m).unwrap();
    (f.id, voz.id, m.id)
}

#[test]
fn el_movil_hace_de_camara_con_pausa_y_reloj_adelantado() {
    let _turno = turno();
    let pc = pc("bien");
    a_las(0);
    pc.camaras
        .lock()
        .unwrap()
        .empezar("g-1", "Clase 27-10 10h05", ahora_pc(), 0);

    // --- A los 10 s: el movil pregunta y empieza su reloj ---------------
    a_las(10);
    pc.camaras
        .lock()
        .unwrap()
        .seguir_a(ahora_pc(), 10 * S, false);
    let (puerto, hilo) = escuchar(&pc, true);
    let mut c = llamar(puerto);
    assert!(c.sabe_de_camara(), "el PC dice camara1 en el hola");
    let (anuncio, adelanto) = c.preguntar_camara(&ahora_movil).unwrap();
    assert!(anuncio.grabando());
    assert_eq!(anuncio.titulo, "Clase 27-10 10h05");
    assert_eq!(anuncio.aparato, "Portatil");
    assert_eq!(anuncio.ms_actual, 10 * S);
    assert_eq!(anuncio.codec_del_clip(), Some(camara::HEVC));
    assert_eq!(adelanto, ADELANTO, "el desfase, medido como NTP");
    let mut reloj = RelojDelMovil::de(&anuncio);

    // --- A los 12 s, la primera foto, con su minuto del movil -----------
    a_las(12);
    let dir = pc.raiz.join("movil");
    let uno = jpeg(3);
    let ms = c
        .mandar_foto(
            &fichero(&dir, "IMG_0001.jpg", &uno),
            "IMG_0001.jpg",
            &FotoDeCamara {
                grabacion: anuncio.grabacion.clone(),
                ms: Some(reloj.ms()),
                tomada_en: ahora_movil(),
                reloj: adelanto,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(ms, 12 * S);

    // --- De los 20 a los 50 s, en pausa ---------------------------------
    a_las(20);
    pc.camaras
        .lock()
        .unwrap()
        .seguir_a(ahora_pc(), 20 * S, true);
    a_las(25);
    let (a, _) = c.preguntar_camara(&ahora_movil).unwrap();
    assert!(a.en_pausa);
    reloj.al_dia(&a);
    a_las(50);
    pc.camaras
        .lock()
        .unwrap()
        .seguir_a(ahora_pc(), 20 * S, false);
    let (a, _) = c.preguntar_camara(&ahora_movil).unwrap();
    assert!(!a.en_pausa);
    reloj.al_dia(&a);

    // --- A los 55 s un clip de 5 s que empezo a los 50 (el 20 de la
    //     grabacion); a los 60, una foto de un movil que solo sabe su hora
    a_las(55);
    assert_eq!(reloj.ms(), 25 * S, "la pausa no cuenta en el movil");
    let ms = c
        .mandar_foto(
            &fichero(&dir, "VID_0001.mp4", &mp4()),
            "VID_0001.mp4",
            &FotoDeCamara {
                grabacion: anuncio.grabacion.clone(),
                ms: Some(reloj.ms() - 5 * S),
                duracion_ms: 5 * S,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(ms, 20 * S);
    a_las(60);
    let dos = jpeg(5);
    let ms = c
        .mandar_foto(
            &fichero(&dir, "IMG_0002.jpg", &dos),
            "IMG_0002.jpg",
            &FotoDeCamara {
                grabacion: anuncio.grabacion.clone(),
                ms: None,
                tomada_en: ahora_movil(),
                reloj: ADELANTO,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(ms, 30 * S, "60 s de pared menos 30 de pausa");

    // --- Casos negativos: no se niega nada que rompa la conexion --------
    let e = c
        .mandar_foto(
            &fichero(&dir, "apuntes.pdf", b"%PDF-1.7"),
            "apuntes.pdf",
            &FotoDeCamara {
                grabacion: anuncio.grabacion.clone(),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(matches!(e, ErrorAlLienzo::Remoto(_)), "{e:?}");
    // Dice ser un JPEG y no lo es: se tira al llegar.
    let e = c
        .mandar_foto(
            &fichero(&dir, "falsa.jpg", b"no soy una foto"),
            "falsa.jpg",
            &FotoDeCamara {
                grabacion: anuncio.grabacion.clone(),
                ms: Some(1),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(e.to_string().contains("Solo se aceptan"), "{e}");
    // De otra grabacion.
    let e = c
        .mandar_foto(
            &fichero(&dir, "IMG_9.jpg", &jpeg(9)),
            "IMG_9.jpg",
            &FotoDeCamara {
                grabacion: "no-existe".into(),
                ms: Some(1),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(e.to_string().contains("ya no está"), "{e}");

    // Grabando, todo espera: aun no hay clase donde ponerlo.
    assert_eq!(pc.camaras.lock().unwrap().recibidas(), 3);
    let nuevas = pc.camaras.lock().unwrap().tomar_nuevas();
    assert_eq!(
        nuevas.iter().map(|n| (n.ms, n.clip)).collect::<Vec<_>>(),
        vec![(12 * S, false), (20 * S, true), (30 * S, false)]
    );
    c.adios();
    hilo.join().unwrap();

    // --- A los 80 s, «Terminar»: la clase se guarda y entra todo --------
    a_las(80);
    let duracion = 50 * S;
    pc.camaras.lock().unwrap().acabar(ahora_pc());
    let (proyecto, voz, carpeta) = guardar_la_clase(&pc.raiz, duracion);
    let destino = Destino {
        raiz: pc.raiz.clone(),
        proyecto: proyecto.clone(),
        voz: Some(voz.clone()),
        carpeta: Some(carpeta.clone()),
        aparato: "K7Q2".into(),
        hora: hora_sin_repetir,
    };
    let hechas = pc
        .camaras
        .lock()
        .unwrap()
        .guardada(destino, Some(duracion), ahora_pc());
    assert_eq!(hechas, 3);

    // --- A los 300 s, una foto que el movil subio tarde -----------------
    a_las(300);
    let (puerto, hilo) = escuchar(&pc, true);
    let mut c = llamar(puerto);
    let (a, _) = c.preguntar_camara(&ahora_movil).unwrap();
    assert!(!a.activa, "ya acabo");
    assert_eq!((a.grabacion.as_str(), a.duracion), ("g-1", duracion));
    let tres = jpeg(7);
    let ms = c
        .mandar_foto(
            &fichero(&dir, "IMG_0003.jpg", &tres),
            "IMG_0003.jpg",
            &FotoDeCamara {
                grabacion: "g-1".into(),
                ms: Some(45 * S),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(ms, 45 * S);
    c.adios();
    hilo.join().unwrap();

    // --- Lo que quedo en la clase ----------------------------------------
    let mensajes = leer(&pc.raiz, &proyecto);
    let del_movil: Vec<&Mensaje> = mensajes
        .iter()
        .filter(|m| pixpin_proyecto::camara_del_movil::es_del_movil(m))
        .collect();
    assert_eq!(del_movil.len(), 4);
    let en = |m: &Mensaje| -> (i64, Option<i64>) {
        let g = &m.resto["enGrabacion"];
        assert_eq!(g["de"], serde_json::json!(voz));
        (g["ms"].as_i64().unwrap(), g["duracionMs"].as_i64())
    };
    let mut vistos: Vec<(i64, Option<i64>, Option<Clase>)> = del_movil
        .iter()
        .map(|m| {
            let (ms, d) = en(m);
            (ms, d, m.clase.clone())
        })
        .collect();
    vistos.sort_by_key(|v| v.0);
    assert_eq!(
        vistos,
        vec![
            (12 * S, None, Some(Clase::Imagen)),
            (20 * S, Some(5 * S), Some(Clase::Archivo)),
            (30 * S, None, Some(Clase::Imagen)),
            (45 * S, None, Some(Clase::Imagen)),
        ]
    );
    // El clip es un video para el chat.
    let clip = del_movil.iter().find(|m| cuaderno::es_video(m)).unwrap();
    assert_eq!(clip.nombre, "VID_0001.mp4");
    // Los bytes tal cual, con su EXIF.
    let primera = del_movil
        .iter()
        .find(|m| m.nombre == "IMG_0001.jpg")
        .unwrap();
    let ruta = almacen::carpeta(&pc.raiz, &proyecto).join(primera.ruta.as_deref().unwrap());
    assert_eq!(std::fs::read(ruta).unwrap(), uno);
    assert!(primera.texto.starts_with("📱 ⏱ 0:12"), "{}", primera.texto);
    assert_eq!(
        primera.resto.get("deAparato").and_then(|d| d.as_str()),
        Some("Telefono")
    );

    // En la nota: las capturas y el clip, en orden.
    let nota = mensajes.iter().find(|m| m.id == voz).unwrap();
    let capturas: Vec<i64> = clase::capturas_de(&nota.resto)
        .into_iter()
        .map(|(ms, _)| ms)
        .collect();
    assert_eq!(capturas, vec![12 * S, 30 * S, 45 * S]);
    let clips = clase::clips_de(&nota.resto);
    assert_eq!(clips.len(), 1);
    assert_eq!((clips[0].ms, clips[0].duracion_ms), (20 * S, 5 * S));
    // Y en su carpeta, en el orden de la grabacion.
    let c = Carpeta::de_mensaje(mensajes.iter().find(|m| m.id == carpeta).unwrap()).unwrap();
    let minutos: Vec<Option<i64>> = c.paginas.iter().map(|p| p.ms).collect();
    assert_eq!(
        minutos,
        vec![
            Some(0),
            Some(12 * S),
            Some(20 * S),
            Some(30 * S),
            Some(45 * S)
        ]
    );
    // La cola quedo vacia.
    let cola = pc.raiz.join("cola-camara");
    let quedan = std::fs::read_dir(&cola)
        .map(|d| {
            d.flatten()
                .flat_map(|e| std::fs::read_dir(e.path()).into_iter().flatten())
                .count()
        })
        .unwrap_or(0);
    assert_eq!(quedan, 0, "lo de la cola entro en la clase");
}

/// **El PC avisa al movil** (al empezar, pausar, seguir y acabar) y mide de
/// paso el desfase: con el entra bien la foto de un movil que no manda ni
/// `ms` ni su `reloj`.
#[test]
fn el_pc_avisa_al_movil_y_mide_su_reloj() {
    let _turno = turno();
    let pc = pc("aviso");
    a_las(0);
    pc.camaras
        .lock()
        .unwrap()
        .empezar("g-2", "Reunion", ahora_pc(), 0);
    a_las(30);
    pc.camaras
        .lock()
        .unwrap()
        .seguir_a(ahora_pc(), 30 * S, false);
    // El movil de mentira: atiende el aviso como el `Respondedor` de
    // Android y contesta con su reloj.
    let escucha = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let puerto = escucha.local_addr().unwrap().port();
    let movil = std::thread::spawn(move || -> Anuncio {
        let (flujo, _) = escucha.accept().unwrap();
        let clave = pixpin_sincro::codigo::clave_de_grupo(CODIGO);
        let mut c = Canal::saludar(flujo, &clave, [8; 32], false).unwrap();
        let (_, hola) = c.recibir().unwrap();
        assert!(String::from_utf8_lossy(&hola).contains("\"hola\""));
        let r = m::Respuesta {
            hola: Some(m::Hola {
                yo: al_cable(&ficha("tel", "Telefono", "B")),
                capacidades: vec![camara::CAPACIDAD.into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        c.mandar(Tipo::Json, &serde_json::to_vec(&r).unwrap())
            .unwrap();
        let (_, datos) = c.recibir().unwrap();
        let aviso: Anuncio = serde_json::from_slice(&datos).unwrap();
        let contesta = Anuncio {
            reloj: ahora_movil(),
            ..Anuncio::nada()
        };
        c.mandar(Tipo::Json, &serde_json::to_vec(&contesta).unwrap())
            .unwrap();
        // El adios.
        let _ = c.recibir();
        let _ = c.mandar(Tipo::Json, b"{}");
        aviso
    });
    let flujo = TcpStream::connect(("127.0.0.1", puerto)).unwrap();
    let hola = m::Hola {
        yo: al_cable(&ficha("pc", "Portatil", "A")),
        reloj: ahora_pc(),
        capacidades: pixpin_sincro::CAPACIDADES_CON_CAMARA
            .iter()
            .map(|c| c.to_string())
            .collect(),
        ..Default::default()
    };
    let mut c = Conexion::abrir(flujo, CODIGO, [9; 32], hola).unwrap();
    assert!(c.sabe_de_camara());
    let anuncio = pc.camaras.lock().unwrap().anuncio(ahora_pc()).unwrap();
    let desfase = c.anunciar_camara(&anuncio, &ahora_pc).unwrap();
    assert_eq!(desfase, ADELANTO);
    c.adios();
    let visto = movil.join().unwrap();
    assert_eq!(visto.ms_actual, 30 * S);
    assert_eq!(visto.grabacion, "g-2");
    assert_eq!(visto.reloj, PARED + 30 * S, "el t0 del PC");
    pc.camaras.lock().unwrap().medir("tel", desfase);

    // Una foto sin `ms` ni `reloj`, hecha a los 40 s del PC (47 del movil).
    a_las(45);
    let (puerto, hilo) = escuchar(&pc, true);
    let mut c = llamar(puerto);
    let dir = pc.raiz.join("movil");
    let ms = c
        .mandar_foto(
            &fichero(&dir, "a.jpg", &jpeg(2)),
            "a.jpg",
            &FotoDeCamara {
                grabacion: "g-2".into(),
                tomada_en: PARED + 40 * S + ADELANTO,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(ms, 40 * S, "con el desfase medido por el PC");
    c.adios();
    hilo.join().unwrap();
}

/// **Caso negativo: un PC de antes** no sabe de la camara. Contesta «No sé
/// qué es» y la conexion sigue sana (el movil no ensena la camara).
#[test]
fn un_pc_de_antes_contesta_no_se_que_es_y_sigue() {
    let _turno = turno();
    let pc = pc("antes");
    let (puerto, hilo) = escuchar(&pc, false);
    let mut c = llamar(puerto);
    assert!(!c.sabe_de_camara());
    let e = c.preguntar_camara(&ahora_movil).unwrap_err();
    assert!(e.to_string().contains("camara_remota"), "{e}");
    // Sigue hablando: una foto a la grabacion se niega antes de los trozos.
    let dir = pc.raiz.join("movil");
    let e = c
        .mandar_foto(
            &fichero(&dir, "a.jpg", &jpeg(1)),
            "a.jpg",
            &FotoDeCamara {
                grabacion: "g".into(),
                ms: Some(1),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(
        !matches!(e, ErrorAlLienzo::Canal(_) | ErrorAlLienzo::Io(_)),
        "{e:?}"
    );
    c.adios();
    hilo.join().unwrap();
    assert!(Indice::leer(&pc.raiz).proyectos.is_empty(), "no entro nada");
}
