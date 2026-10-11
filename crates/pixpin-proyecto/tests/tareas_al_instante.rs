//! **Tareas al instante** (10-oct-2026), medido de verdad.
//!
//! Dos PixPin de PC con su carpeta de datos cada uno (`DiscoPc`, como
//! `%APPDATA%\PixPinMax`) y un movil de mentira (`DiscoAndroid`), en el mismo
//! grupo y hablandose por un socket de esta maquina con el protocolo y el
//! cifrado de verdad. Se tacha una tarea en un PC y se mide cuanto tarda en
//! estar escrita en el otro con la vuelta recortada a su chat
//! (`vuelta::solo_estos`), y lo que tardaria la vuelta entera.
//!
//! Lo de la red es lo de la app: la app pone delante la calma de
//! `automatica::CALMA_TAREAS` (600 ms) y, al llamar, una sonda de un
//! paquete. El tiempo se mide con optimizaciones (`--release`); sin ellas
//! solo se comprueba que llega.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use pixpin_proyecto::almacen::{self, Ficha, Indice};
use pixpin_proyecto::cuaderno::{self, Cuaderno, Mensaje, Sello};
use pixpin_proyecto::mini;
use pixpin_proyecto::vista::DiscoPc;
use pixpin_sincro::disco::{Disco, GENERAL};
use pixpin_sincro::disco_android::DiscoAndroid;
use pixpin_sincro::disco_android::prueba::{Reloj, conectado, crear_grupo, presentar};
use pixpin_sincro::kotlin;
use pixpin_sincro::protocolo::Hecho;

/// Lo que se espera en la app antes de llamar (`automatica::CALMA_TAREAS`).
const CALMA_TAREAS_MS: u128 = 600;
/// El objetivo: de tachar a escrito en el otro, en la red de casa.
const OBJETIVO_MS: u128 = 2_000;

struct Tres {
    dir: PathBuf,
    pc: DiscoPc,
    otro_pc: DiscoPc,
    movil: DiscoAndroid,
    reloj: Reloj,
}

impl Drop for Tres {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn montar(etiqueta: &str) -> Tres {
    let dir = std::env::temp_dir().join(format!(
        "pixpin-tareas-al-instante-{etiqueta}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let t = Tres {
        pc: DiscoPc::nuevo(&dir.join("pc-1/PixPinMax")),
        otro_pc: DiscoPc::nuevo(&dir.join("pc-2/PixPinMax")),
        movil: DiscoAndroid::nuevo(dir.join("movil/files"), "movil"),
        dir,
        reloj: Reloj::nuevo(),
    };
    presentar(&t.pc, "id-pc", "Portátil");
    presentar(&t.otro_pc, "id-pc2", "Sobremesa");
    presentar(&t.movil, "id-movil", "Teléfono");
    crear_grupo(&t.pc, "ABCDE23456", t.reloj.ahora());
    conectado(
        &t.otro_pc,
        &t.pc,
        true,
        Some("ABCDE23456"),
        &t.reloj,
        |_| {},
    )
    .unwrap();
    conectado(&t.movil, &t.pc, true, Some("ABCDE23456"), &t.reloj, |_| {}).unwrap();
    t
}

fn aparato() -> String {
    pixpin_proyecto::codigos::de_aparato("id-pc")
}

/// «Mensajes guardados» del primer PC con una lista de tareas, y `chats`
/// chats mas con `notas` notas cada uno (lo que hace lenta la vuelta entera).
fn llenar(t: &Tres, chats: usize, notas: usize) -> (Ficha, Mensaje) {
    let raiz = t.pc.raiz();
    let guardados = almacen::asegurar_guardados(&raiz, 1, &aparato()).unwrap();
    let doc = mini::escribir_tareas(
        "Compra",
        &[
            mini::Tarea {
                texto: "pan".into(),
                hecha: false,
            },
            mini::Tarea {
                texto: "leche".into(),
                hecha: false,
            },
        ],
    );
    let lista = Mensaje::miniapp(
        mini::TAREAS,
        "Compra",
        &doc,
        &Sello {
            cuando: t.reloj.tic(),
            numero: 1,
            aparato: aparato(),
            proyecto: guardados.id.clone(),
        },
    );
    cuaderno::anadir(&almacen::carpeta(&raiz, &guardados.id), &lista).unwrap();
    let mut indice = Indice::leer(&raiz);
    for c in 0..chats {
        let ficha = Ficha::nueva(&format!("Obra {c}"), t.reloj.tic(), &aparato());
        indice.proyectos.push(ficha.clone());
        let lineas: Vec<String> = (1..=notas)
            .map(|n| {
                let m = Mensaje::nota(
                    &format!("nota {n} de la obra {c}: medir, comprar y apuntar lo que falte"),
                    &Sello {
                        cuando: t.reloj.tic(),
                        numero: n as i64,
                        aparato: aparato(),
                        proyecto: ficha.id.clone(),
                    },
                );
                cuaderno::linea_de(&m).unwrap()
            })
            .collect();
        let carpeta = almacen::carpeta(&raiz, &ficha.id);
        std::fs::create_dir_all(&carpeta).unwrap();
        cuaderno::anadir_lineas(&carpeta, &lineas).unwrap();
    }
    indice.guardar(&raiz).unwrap();
    (guardados, lista)
}

/// Tacha la tarea `n` de la lista en el primer PC, como la ventana de
/// Tareas (`cuaderno::reemplazar` del mensaje).
fn tachar(t: &Tres, guardados: &Ficha, lista: &Mensaje, n: usize) {
    let carpeta = almacen::carpeta(&t.pc.raiz(), &guardados.id);
    let mut m = Cuaderno::leer_de(&carpeta)
        .unwrap()
        .mensajes
        .into_iter()
        .find(|m| m.id == lista.id)
        .unwrap();
    m.texto = mini::alternar(&m.texto, n);
    assert!(cuaderno::reemplazar(&carpeta, &m).unwrap());
}

fn hechas_en_otro_pc(t: &Tres) -> usize {
    let raiz = t.otro_pc.raiz();
    let g = Indice::leer(&raiz)
        .proyectos
        .into_iter()
        .find(|f| f.es_guardados())
        .unwrap();
    Cuaderno::leer_de(&almacen::carpeta(&raiz, &g.id))
        .unwrap()
        .mensajes
        .iter()
        .filter(|m| m.miniapp.as_deref() == Some(mini::TAREAS))
        .map(|m| {
            mini::leer_tareas(&m.texto)
                .iter()
                .filter(|x| x.hecha)
                .count()
        })
        .sum()
}

fn hechas_en_el_movil(t: &Tres) -> usize {
    t.movil
        .leer_mensajes()
        .iter()
        .filter(|m| kotlin::chat_de(m) == GENERAL)
        .filter_map(|m| kotlin::cadena(m, "texto").map(str::to_string))
        .map(|x| mini::leer_tareas(&x).iter().filter(|x| x.hecha).count())
        .sum()
}

/// La vuelta recortada desde el primer PC hacia `otro`, contando desde
/// antes de conectar (lo que hace la app: conectar, saludar y juntar).
fn rapida<D: Disco + Sync>(t: &Tres, otro: &D) -> Duration {
    let chats = BTreeSet::from([GENERAL.to_string()]);
    let t0 = Instant::now();
    conectado(&t.pc, otro, false, None, &t.reloj, |s| {
        pixpin_sincro::vuelta::solo_estos(
            s,
            &chats,
            &mut Hecho::default(),
            "",
            &|| t.reloj.ahora(),
            &mut |_| {},
        )
        .unwrap()
    })
    .unwrap();
    t0.elapsed()
}

fn entera<D: Disco + Sync>(t: &Tres, otro: &D) -> Duration {
    let t0 = Instant::now();
    conectado(&t.pc, otro, false, None, &t.reloj, |s| {
        pixpin_sincro::vuelta::una(
            s,
            None,
            &mut Hecho::default(),
            "",
            &|| t.reloj.ahora(),
            &mut |_| {},
        )
        .unwrap()
    })
    .unwrap();
    t0.elapsed()
}

/// Lo medido: las rapidas al otro PC y al movil, y la entera al otro PC (ms).
struct Medida {
    al_pc: Vec<u128>,
    al_movil: Vec<u128>,
    entera: u128,
}

impl Medida {
    fn peor(&self) -> u128 {
        self.al_pc
            .iter()
            .chain(&self.al_movil)
            .copied()
            .max()
            .unwrap_or(0)
    }
}

/// Monta los tres con `chats` obras de `notas` notas, junta todo una vez y
/// mide tres tachaduras seguidas, cada una hacia el otro PC y el movil.
fn medir(etiqueta: &str, chats: usize, notas: usize) -> Medida {
    let t = montar(etiqueta);
    let (guardados, lista) = llenar(&t, chats, notas);
    // La primera vez viaja todo: eso no es lo que se mide.
    entera(&t, &t.otro_pc);
    entera(&t, &t.movil);
    assert_eq!(hechas_en_otro_pc(&t), 0);
    let mut m = Medida {
        al_pc: Vec::new(),
        al_movil: Vec::new(),
        entera: 0,
    };
    for n in 0..3 {
        tachar(&t, &guardados, &lista, n % 2);
        let hechas = if n % 2 == 0 { 1 } else { 2 };
        m.al_pc.push(rapida(&t, &t.otro_pc).as_millis());
        assert_eq!(
            hechas_en_otro_pc(&t),
            hechas,
            "vuelta {n}: tachada en el otro PC"
        );
        m.al_movil.push(rapida(&t, &t.movil).as_millis());
        assert_eq!(
            hechas_en_el_movil(&t),
            hechas,
            "vuelta {n}: tachada en el movil"
        );
    }
    // Lo mismo con la vuelta entera, para comparar.
    tachar(&t, &guardados, &lista, 0);
    m.entera = entera(&t, &t.otro_pc).as_millis();
    assert_eq!(hechas_en_otro_pc(&t), 2, "la entera tambien la lleva");
    eprintln!(
        "tareas al instante, {} chats y {} notas: PC->PC {:?} ms · PC->movil {:?} ms · vuelta entera {} ms · \
         peor rapida {} ms + calma {CALMA_TAREAS_MS} ms = {} ms",
        chats + 1,
        chats * notas,
        m.al_pc,
        m.al_movil,
        m.entera,
        m.peor(),
        m.peor() + CALMA_TAREAS_MS,
    );
    m
}

/// Del tamano de los datos del usuario multiplicado por diez (el 10-oct
/// tenia 12 chats y 112 mensajes): 30 obras de 40 notas.
#[test]
fn una_tarea_tachada_en_un_pc_esta_escrita_en_el_otro_y_en_el_movil_en_menos_de_dos_segundos() {
    let m = medir("medir", 30, 40);
    // Sin optimizar (cargo test a secas) el tiempo no dice nada de la app.
    if !cfg!(debug_assertions) {
        assert!(
            m.peor() + CALMA_TAREAS_MS < OBJETIVO_MS,
            "de tachar a escrito: {} ms",
            m.peor() + CALMA_TAREAS_MS
        );
        assert!(
            m.peor() < m.entera,
            "la recortada tiene que ganar a la entera"
        );
    }
}

/// Lo mismo con 9000 notas, para ver como crece: tarda minutos en montarse,
/// asi que va aparte (`-- --ignored`). Solo se apunta, no se exige.
#[test]
#[ignore]
fn con_nueve_mil_notas_la_rapida_sigue_ganando_a_la_entera() {
    let m = medir("medir-grande", 30, 300);
    if !cfg!(debug_assertions) {
        assert!(m.peor() < m.entera);
    }
}

#[test]
fn la_vuelta_rapida_no_se_lleva_los_otros_chats() {
    let t = montar("solo-el-suyo");
    let (guardados, lista) = llenar(&t, 2, 3);
    tachar(&t, &guardados, &lista, 0);
    rapida(&t, &t.otro_pc);
    assert_eq!(hechas_en_otro_pc(&t), 1);
    // Caso negativo: las obras no viajaron (solo «Guardados»).
    let otras = Indice::leer(&t.otro_pc.raiz())
        .proyectos
        .iter()
        .filter(|f| !f.es_guardados())
        .count();
    assert_eq!(otras, 0, "solo el chat de la lista");
}
