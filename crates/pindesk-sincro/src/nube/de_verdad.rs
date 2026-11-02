//! **Contra la nube de referencia de verdad**, por HTTP en este equipo.
//!
//! Arranca el servidor de `agente-2.0-constrution` (su `AlmacenNube` y su
//! `atenderApi`, con `pruebas/nube-de-referencia.mts`) y le habla con el
//! cliente de aqui: la firma de `PUT /yo`, un chat compartido que Max sube y
//! Ana baja sin estar en la misma red, una raya que viaja como delta, y dos
//! personas que pintan a la vez (409, juntar, otra vez).
//!
//! Solo corre con `PINDESK_NUBE_AGENTE=<carpeta del repositorio>` (con su
//! `npm ci` hecho); si no, se salta. Con una nube sin `deltas1` prueba el
//! protocolo 1.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};

use serde_json::{Value, json};

use super::cliente::Nube;
use super::http::HttpLocal;
use super::sin_conexion::NadaRetenido;
use super::trazos;
use super::vuelta::{Opciones, conocer_espacio, vuelta_de_chat};
use crate::canonico::Json;
use crate::compartidos::{self, CompartidosEnDisco};
use crate::disco::Disco;
use crate::disco_android::DiscoAndroid;
use crate::disco_android::prueba::presentar;
use crate::kotlin;

const REL: &str = "pins/draw/d1.excalidraw.gz";

struct Servidor(Child);

impl Drop for Servidor {
    fn drop(&mut self) {
        drop(self.0.stdin.take());
        let _ = self.0.wait();
    }
}

fn arrancar() -> Option<(Servidor, Value)> {
    let agente = std::env::var("PINDESK_NUBE_AGENTE").ok()?;
    let tsx = std::path::Path::new(&agente).join("node_modules/.bin/tsx");
    if !tsx.is_file() {
        eprintln!("sin {}: se salta", tsx.display());
        return None;
    }
    let guion =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pruebas/nube-de-referencia.mts");
    let mut hijo = Command::new(tsx)
        .arg(guion)
        .current_dir(&agente)
        .env("PINDESK_NUBE_AGENTE", &agente)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("node");
    let mut linea = String::new();
    BufReader::new(hijo.stdout.take().unwrap())
        .read_line(&mut linea)
        .unwrap();
    Some((
        Servidor(hijo),
        serde_json::from_str(&linea).expect("la linea del servidor"),
    ))
}

fn escribir_lienzo(d: &DiscoAndroid, e: &Value) {
    let f = d.raiz().join(REL);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    let mut g = flate2::write::GzEncoder::new(
        std::fs::File::create(&f).unwrap(),
        flate2::Compression::default(),
    );
    g.write_all(e.to_string().as_bytes()).unwrap();
    g.finish().unwrap();
}

fn leer_lienzo(d: &DiscoAndroid) -> Value {
    let mut s = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(d.raiz().join(REL)).unwrap())
        .read_to_string(&mut s)
        .unwrap();
    serde_json::from_str(&s).unwrap()
}

fn trazo(id: &str, i: u64) -> Value {
    let p: Vec<Value> = (0..30u64)
        .map(|k| json!([(k * 7 + i * 13) % 101, (k * 11 + i) % 97]))
        .collect();
    json!({"id": id, "type": "freedraw", "version": 1, "versionNonce": i + 1, "isDeleted": false, "points": p})
}

fn anadir(d: &DiscoAndroid, id: &str, i: u64) {
    let mut e = leer_lienzo(d);
    e["elements"].as_array_mut().unwrap().push(trazo(id, i));
    escribir_lienzo(d, &e);
}

fn ids(d: &DiscoAndroid) -> Vec<String> {
    trazos::versiones(&leer_lienzo(d)).into_keys().collect()
}

fn mensaje(d: &DiscoAndroid, id: &str, texto: &str, clase: &str, refe: Option<&str>) {
    let m = kotlin::normalizar_mensaje(&Json::de_valor(&json!({
        "id": id, "cuando": 1_000, "clase": clase, "texto": texto, "proyecto": "p1",
        "referencia": refe, "numero": 1,
    })))
    .unwrap();
    d.aplicar_mensajes("p1", &[m.a_texto()], &[], 1_000)
        .unwrap();
}

#[test]
fn max_y_ana_por_la_nube_de_referencia() {
    let Some((_servidor, info)) = arrancar() else {
        eprintln!("sin PINDESK_NUBE_AGENTE: se salta la prueba contra la nube de referencia");
        return;
    };
    let dir = std::env::temp_dir().join(format!("pindesk-nube-verdad-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let max = DiscoAndroid::nuevo(dir.join("max/files"), "max");
    let ana = DiscoAndroid::nuevo(dir.join("ana/files"), "ana");
    presentar(&max, "id-max", "PC de Max");
    presentar(&ana, "id-ana", "PC de Ana");
    let (cmax, cana) = (
        CompartidosEnDisco::nuevo(&max.raiz()),
        CompartidosEnDisco::nuevo(&ana.raiz()),
    );
    let (ymax, yana) = (cmax.yo_de(&max).unwrap(), cana.yo_de(&ana).unwrap());

    let url = format!("http://127.0.0.1:{}", info["puerto"]);
    let t = HttpLocal;
    let nmax = Nube {
        direccion: url.clone(),
        llave: info["max"]["llave"].as_str().unwrap().into(),
        t: &t,
    };
    let nana = Nube {
        direccion: url,
        llave: info["ana"]["llave"].as_str().unwrap().into(),
        t: &t,
    };

    // La firma de PUT /yo, como la comprueba la nube; una ajena no vale.
    let hola = nmax.hola().unwrap();
    let deltas = hola.sabe(super::DELTAS);
    eprintln!("nube de referencia: capacidades {:?}", hola.capacidades);
    let firma = super::firma_de_yo(&ymax.llaves, &hola.aparato.id, "id-max");
    nmax.presentar("id-max", &ymax.clave(), &firma).unwrap();
    let hana = nana.hola().unwrap();
    assert!(nana.presentar("id-ana", &yana.clave(), &firma).is_err());
    nana.presentar(
        "id-ana",
        &yana.clave(),
        &super::firma_de_yo(&yana.llaves, &hana.aparato.id, "id-ana"),
    )
    .unwrap();

    // Max crea el chat compartido con Ana dentro.
    let p = kotlin::normalizar_proyecto(&Json::de_valor(
        &json!({"id": "p1", "nombre": "Obra", "creado": 1, "tocado": 1}),
    ))
    .unwrap();
    max.guardar_proyecto(&p).unwrap();
    let c = compartidos::crear("p1", "Obra", &ymax, 10);
    let c = compartidos::invitar(&c, &ymax, &yana.ficha(20), 20).unwrap();
    cmax.poner(&c).unwrap();
    mensaje(&max, "m1", "planta", "DIBUJO", Some("d1"));
    mensaje(&max, "m2", "hola Ana", "NOTA", None);
    let tr: Vec<Value> = (0..100).map(|i| trazo(&format!("t{i}"), i)).collect();
    escribir_lienzo(
        &max,
        &json!({"type": "excalidraw", "version": 2, "elements": tr, "files": {}}),
    );
    let op = |ahora, bajar| Opciones {
        deltas,
        ahora,
        retencion: &NadaRetenido,
        bajar,
    };
    vuelta_de_chat(&max, &nmax, "p1", &op(30, false)).unwrap();
    let espacio = super::espacio_compartido(&ymax.id, "p1");
    assert_eq!(
        super::estado::EstadoDeChat::leer(&max.sincro(), "p1").espacio,
        espacio
    );

    // Ana lo ve en su lista y lo baja.
    let lista = nana.chats().unwrap();
    let suyo = lista
        .iter()
        .find(|x| x.chat == "p1")
        .expect("Ana ve el chat");
    assert_eq!(suyo.espacio, espacio);
    conocer_espacio(&ana.sincro(), "p1", &suyo.espacio).unwrap();
    vuelta_de_chat(&ana, &nana, "p1", &op(31, true)).unwrap();
    assert_eq!(ids(&ana).len(), 100);
    assert!(cana.de("p1").is_some(), "la lista firmada llega");
    assert_eq!(ana.mensajes("p1").unwrap().len(), 2);

    // Una raya de Max: solo la raya (con deltas1).
    anadir(&max, "raya", 500);
    let i = vuelta_de_chat(&max, &nmax, "p1", &op(32, false)).unwrap();
    eprintln!("una raya contra la nube de referencia: {i:?}");
    if deltas {
        assert_eq!(i.objetos, 1);
        assert!(
            i.bytes_de_objetos < 1_000 && i.bytes_de_parches < 1_000,
            "{i:?}"
        );
    }

    // Las dos pintan a la vez: la segunda recibe 409, junta y sube.
    anadir(&max, "de-max", 600);
    anadir(&ana, "de-ana", 700);
    mensaje(&ana, "a1", "lo de Ana", "NOTA", None);
    vuelta_de_chat(&max, &nmax, "p1", &op(33, false)).unwrap();
    let i = vuelta_de_chat(&ana, &nana, "p1", &op(34, false)).unwrap();
    assert!(i.conflictos >= 1, "{i:?}");
    vuelta_de_chat(&max, &nmax, "p1", &op(35, true)).unwrap();
    for d in [&max, &ana] {
        let v = ids(d);
        for k in ["raya", "de-max", "de-ana"] {
            assert!(v.contains(&k.to_string()), "{k} en {v:?}");
        }
        assert_eq!(v.len(), 103);
        assert_eq!(d.mensajes("p1").unwrap().len(), 3);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
