//! **Dos aparatos y una nube falsa**: lo que sube una raya, dos personas que
//! pintan a la vez, el 409, una nube del protocolo 1, compactar y «Sin
//! conexión».

use std::io::{Read, Write};

use serde_json::{Value, json};

use super::cliente::Nube;
use super::estado::EstadoDeChat;
use super::falsa::{NubeFalsa, linea};
use super::sin_conexion::{NadaRetenido, Retencion, SinConexion};
use super::trazos;
use super::vuelta::{Informe, Opciones, vuelta_de_chat};
use crate::canonico::Json;
use crate::disco::Disco;
use crate::disco_android::DiscoAndroid;
use crate::disco_android::prueba::presentar;
use crate::kotlin;

const REL: &str = "pins/draw/d1.excalidraw.gz";

struct Banco {
    dir: std::path::PathBuf,
    a: DiscoAndroid,
    b: DiscoAndroid,
}

impl Drop for Banco {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn trazo(id: &str, v: i64, puntos: usize) -> Value {
    // Puntos distintos en cada trazo (como a mano): si no, el gzip del
    // lienzo entero sale casi de nada y cualquier delta pesaria mas.
    let semilla = id
        .bytes()
        .fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
    let p: Vec<Value> = (0..puntos as u64)
        .map(|i| {
            let x = semilla.wrapping_mul(i + 3) % 997;
            json!([x as f64 / 7.0, (x * 13 % 991) as f64 / 3.0])
        })
        .collect();
    json!({"id": id, "type": "freedraw", "x": 10, "y": 20, "version": v,
           "versionNonce": v * 7 + 1, "isDeleted": false, "points": p,
           "strokeColor": "#1e1e1e", "strokeWidth": 2})
}

fn escribir_lienzo(d: &DiscoAndroid, escena: &Value) {
    let f = d.raiz().join(REL);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    let mut e = flate2::write::GzEncoder::new(
        std::fs::File::create(&f).unwrap(),
        flate2::Compression::default(),
    );
    e.write_all(escena.to_string().as_bytes()).unwrap();
    e.finish().unwrap();
}

fn leer_lienzo(d: &DiscoAndroid) -> Value {
    let mut s = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(d.raiz().join(REL)).unwrap())
        .read_to_string(&mut s)
        .unwrap();
    serde_json::from_str(&s).unwrap()
}

fn ids(e: &Value) -> Vec<String> {
    let mut v: Vec<String> = trazos::versiones(e).into_keys().collect();
    v.sort();
    v
}

fn mensaje(d: &DiscoAndroid, id: &str, texto: &str, clase: &str, referencia: Option<&str>) {
    let m = Json::de_valor(&json!({
        "id": id, "cuando": 1_000, "clase": clase, "texto": texto,
        "proyecto": "p1", "referencia": referencia, "numero": 1,
    }));
    let m = kotlin::normalizar_mensaje(&m).unwrap();
    d.aplicar_mensajes("p1", &[m.a_texto()], &[], 1_000)
        .unwrap();
}

fn textos(d: &DiscoAndroid) -> Vec<String> {
    let mut v: Vec<String> = d
        .mensajes("p1")
        .unwrap()
        .iter()
        .filter_map(|m| kotlin::cadena(m, "texto").map(str::to_string))
        .collect();
    v.sort();
    v
}

fn montar(etiqueta: &str) -> Banco {
    let dir = std::env::temp_dir().join(format!("pindesk-nube-{etiqueta}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let a = DiscoAndroid::nuevo(dir.join("a/files"), "a");
    let b = DiscoAndroid::nuevo(dir.join("b/files"), "b");
    presentar(&a, "id-a", "PC de Max");
    presentar(&b, "id-b", "PC de Ana");
    let p = kotlin::normalizar_proyecto(&Json::de_valor(&json!({
        "id": "p1", "nombre": "Obra", "creado": 1, "tocado": 1,
    })))
    .unwrap();
    a.guardar_proyecto(&p).unwrap();
    mensaje(&a, "m1", "planta", "DIBUJO", Some("d1"));
    for i in 0..200 {
        mensaje(
            &a,
            &format!("n{i}"),
            &format!("nota numero {i} de la obra"),
            "NOTA",
            None,
        );
    }
    let trazos: Vec<Value> = (0..300).map(|i| trazo(&format!("t{i}"), 1, 30)).collect();
    escribir_lienzo(
        &a,
        &json!({"type": "excalidraw", "version": 2, "elements": trazos, "files": {}}),
    );
    Banco { dir, a, b }
}

fn op<'a>(deltas: bool, ahora: i64, r: &'a dyn Retencion, bajar: bool) -> Opciones<'a> {
    Opciones {
        deltas,
        ahora,
        retencion: r,
        bajar,
    }
}

fn nube(f: &NubeFalsa) -> Nube<'_> {
    Nube {
        direccion: "https://nube.ejemplo".into(),
        llave: "pxn_prueba".into(),
        t: f,
    }
}

fn anadir_trazo(d: &DiscoAndroid, id: &str) {
    let mut e = leer_lienzo(d);
    e["elements"].as_array_mut().unwrap().push(trazo(id, 1, 30));
    escribir_lienzo(d, &e);
}

#[test]
fn una_raya_sube_solo_la_raya() {
    let banco = montar("raya");
    let f = NubeFalsa::nueva(true);
    let n = nube(&f);
    let i = vuelta_de_chat(&banco.a, &n, "p1", &op(true, 10, &NadaRetenido, false)).unwrap();
    assert!(i.subido);
    let (todo, _) = f.cuenta();
    // B baja el chat entero.
    vuelta_de_chat(&banco.b, &n, "p1", &op(true, 11, &NadaRetenido, true)).unwrap();
    assert_eq!(textos(&banco.b).len(), 201);
    assert_eq!(ids(&leer_lienzo(&banco.b)).len(), 300);
    f.cuenta();

    // Una raya y un texto cambiado.
    anadir_trazo(&banco.a, "raya");
    mensaje(&banco.a, "n7", "nota cambiada", "NOTA", None);
    let i: Informe =
        vuelta_de_chat(&banco.a, &n, "p1", &op(true, 12, &NadaRetenido, false)).unwrap();
    let (bytes, pedidos) = f.cuenta();
    eprintln!(
        "una raya de 30 puntos + un mensaje: parche {} B, delta {} B (antes: {todo} B el chat entero); pedidos {pedidos:?}",
        i.bytes_de_parches, i.bytes_de_objetos
    );
    assert_eq!(i.objetos, 1, "solo el delta");
    assert!(i.bytes_de_objetos < 1_000, "{}", i.bytes_de_objetos);
    assert!(i.bytes_de_parches < 2_000, "{}", i.bytes_de_parches);
    assert!(bytes < todo / 20);
    let inv = f.inventario("p1");
    assert_eq!(linea(&inv, REL).deltas().len(), 1);

    // Y B se trae solo eso.
    vuelta_de_chat(&banco.b, &n, "p1", &op(true, 13, &NadaRetenido, true)).unwrap();
    assert!(ids(&leer_lienzo(&banco.b)).contains(&"raya".to_string()));
    assert!(textos(&banco.b).contains(&"nota cambiada".to_string()));
    // Sin cambios, nada sube.
    let i = vuelta_de_chat(&banco.b, &n, "p1", &op(true, 14, &NadaRetenido, false)).unwrap();
    assert!(!i.subido, "{i:?}");
}

#[test]
fn dos_personas_pintan_a_la_vez_el_409_junta_y_quedan_las_dos_rayas() {
    let banco = montar("vez");
    let f = NubeFalsa::nueva(true);
    let n = nube(&f);
    vuelta_de_chat(&banco.a, &n, "p1", &op(true, 10, &NadaRetenido, false)).unwrap();
    vuelta_de_chat(&banco.b, &n, "p1", &op(true, 11, &NadaRetenido, true)).unwrap();
    anadir_trazo(&banco.a, "de-max");
    anadir_trazo(&banco.b, "de-ana");
    mensaje(&banco.b, "ana1", "lo de Ana", "NOTA", None);
    vuelta_de_chat(&banco.a, &n, "p1", &op(true, 12, &NadaRetenido, false)).unwrap();
    // B sube sin haber bajado: 409, baja, junta y vuelve a subir.
    let i = vuelta_de_chat(&banco.b, &n, "p1", &op(true, 13, &NadaRetenido, false)).unwrap();
    assert_eq!(i.conflictos, 1);
    assert!(i.subido);
    vuelta_de_chat(&banco.a, &n, "p1", &op(true, 14, &NadaRetenido, true)).unwrap();
    for d in [&banco.a, &banco.b] {
        let v = ids(&leer_lienzo(d));
        assert!(v.contains(&"de-max".into()) && v.contains(&"de-ana".into()));
        assert_eq!(v.len(), 302);
        assert!(textos(d).contains(&"lo de Ana".into()));
    }
    // Lo que tiene la nube vale lo mismo.
    let inv = f.inventario("p1");
    let l = linea(&inv, REL);
    let base = super::vuelta::descomprimir(&f.objeto(&l.sha256).unwrap()).unwrap();
    let ds: Vec<Value> = l
        .deltas()
        .iter()
        .map(|d| {
            serde_json::from_str(
                &super::vuelta::descomprimir(&f.objeto(&d.sha256).unwrap()).unwrap(),
            )
            .unwrap()
        })
        .collect();
    assert_eq!(
        ids(&trazos::efectivo(&trazos::escena_de(&base), &ds)).len(),
        302
    );
}

#[test]
fn con_una_nube_del_protocolo_1_sube_el_inventario_y_el_409_tambien_junta() {
    let banco = montar("p1");
    let f = NubeFalsa::nueva(false);
    let n = nube(&f);
    vuelta_de_chat(&banco.a, &n, "p1", &op(false, 10, &NadaRetenido, false)).unwrap();
    // PUT, los objetos y el PUT que confirma: dos revisiones.
    assert_eq!(f.revision("p1"), 2);
    assert!(
        f.inventario("p1")
            .archivos
            .iter()
            .all(|a| a.deltas.is_none())
    );
    vuelta_de_chat(&banco.b, &n, "p1", &op(false, 11, &NadaRetenido, true)).unwrap();
    assert_eq!(textos(&banco.b).len(), 201);
    mensaje(&banco.b, "ana1", "lo de Ana", "NOTA", None);
    anadir_trazo(&banco.b, "de-ana");
    vuelta_de_chat(&banco.b, &n, "p1", &op(false, 12, &NadaRetenido, false)).unwrap();
    mensaje(&banco.a, "max1", "lo de Max", "NOTA", None);
    anadir_trazo(&banco.a, "de-max");
    let i = vuelta_de_chat(&banco.a, &n, "p1", &op(false, 13, &NadaRetenido, false)).unwrap();
    assert_eq!(i.conflictos, 1);
    vuelta_de_chat(&banco.b, &n, "p1", &op(false, 14, &NadaRetenido, true)).unwrap();
    for d in [&banco.a, &banco.b] {
        let t = textos(d);
        assert!(t.contains(&"lo de Ana".into()) && t.contains(&"lo de Max".into()));
        assert_eq!(ids(&leer_lienzo(d)).len(), 302);
    }
}

#[test]
fn con_muchos_deltas_se_compacta() {
    let banco = montar("compactar");
    let f = NubeFalsa::nueva(true);
    let n = nube(&f);
    vuelta_de_chat(&banco.a, &n, "p1", &op(true, 10, &NadaRetenido, false)).unwrap();
    for k in 0..21 {
        anadir_trazo(&banco.a, &format!("r{k}"));
        vuelta_de_chat(&banco.a, &n, "p1", &op(true, 11 + k, &NadaRetenido, false)).unwrap();
        let nd = linea(&f.inventario("p1"), REL).deltas().len();
        assert_eq!(nd, if k < 20 { k as usize + 1 } else { 0 }, "vuelta {k}");
    }
    let est = EstadoDeChat::leer(&banco.a.sincro(), "p1");
    assert!(est.archivos[REL].deltas.is_empty());
}

#[test]
fn sin_conexion_no_sube_ve_lo_de_otros_y_al_descartar_queda_lo_de_la_nube() {
    let banco = montar("sin-conexion");
    let f = NubeFalsa::nueva(true);
    let n = nube(&f);
    vuelta_de_chat(&banco.a, &n, "p1", &op(true, 10, &NadaRetenido, false)).unwrap();
    vuelta_de_chat(&banco.b, &n, "p1", &op(true, 11, &NadaRetenido, true)).unwrap();
    let sc = SinConexion::nuevo(&banco.a.sincro());
    sc.retener(&banco.a, "p1", "d1", 12).unwrap();
    assert!(sc.retenido("p1", REL));
    anadir_trazo(&banco.a, "sin-red");
    let i = vuelta_de_chat(&banco.a, &n, "p1", &op(true, 13, &sc, false)).unwrap();
    assert!(!i.subido, "lo de sin conexion no sube");
    // Ana pinta; a Max le llega al fichero y a la copia.
    anadir_trazo(&banco.b, "de-ana");
    vuelta_de_chat(&banco.b, &n, "p1", &op(true, 14, &NadaRetenido, false)).unwrap();
    vuelta_de_chat(&banco.a, &n, "p1", &op(true, 15, &sc, true)).unwrap();
    let v = ids(&leer_lienzo(&banco.a));
    assert!(v.contains(&"de-ana".into()) && v.contains(&"sin-red".into()));
    // Descartar: queda lo de Ana, se va lo de sin conexion.
    sc.descartar(&banco.a, "p1", "d1").unwrap();
    let v = ids(&leer_lienzo(&banco.a));
    assert!(v.contains(&"de-ana".into()) && !v.contains(&"sin-red".into()));
    assert!(!sc.retenido("p1", REL));

    // Otra vez, ahora volviendo a en linea: sube como delta.
    sc.retener(&banco.a, "p1", "d1", 16).unwrap();
    anadir_trazo(&banco.a, "sin-red-2");
    vuelta_de_chat(&banco.a, &n, "p1", &op(true, 17, &sc, false)).unwrap();
    sc.volver("p1", "d1").unwrap();
    let i = vuelta_de_chat(&banco.a, &n, "p1", &op(true, 18, &sc, false)).unwrap();
    assert!(i.subido && i.objetos == 1);
    vuelta_de_chat(&banco.b, &n, "p1", &op(true, 19, &NadaRetenido, true)).unwrap();
    assert!(ids(&leer_lienzo(&banco.b)).contains(&"sin-red-2".into()));
}

#[test]
fn la_firma_de_yo_es_la_del_texto_del_manual() {
    let ll = crate::compartidos::Llaves::nuevas();
    let t = super::texto_de_yo("ap1", "id-a", &ll.clave());
    assert_eq!(t, format!("pixpin-nube|yo|ap1|id-a|{}", ll.clave()));
    let f = super::firma_de_yo(&ll, "ap1", "id-a");
    assert!(crate::compartidos::Llaves::verificar(
        &ll.clave(),
        t.as_bytes(),
        &f
    ));
    assert!(super::llave_valida("pxn_abcdefghijklmnop"));
    assert!(!super::llave_valida("sk_abcdefghijklmnop"));
}
