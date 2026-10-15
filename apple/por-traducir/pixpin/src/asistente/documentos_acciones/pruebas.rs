//! Pruebas de las herramientas de documentos: elegir el archivo por lo que
//! dice el usuario, las figuras del motor comun, y **lo anotado viaja al
//! movil** (PDF y plano, con el movil de mentira y el protocolo de verdad).

use std::sync::{Arc, Mutex};

use pixpin_proyecto::cuaderno::{self, Mensaje, Sello};
use pixpin_proyecto::vista::{self, DiscoPc};
use pixpin_sincro::canonico::Json;
use pixpin_sincro::disco::{Disco, GENERAL};
use pixpin_sincro::disco_android::DiscoAndroid;
use pixpin_sincro::disco_android::prueba::{Reloj, conectado, crear_grupo, presentar, sincronizar};
use pixpin_sincro::kotlin;

use super::*;

pub(crate) fn carpeta(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pixpin-docacc-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Un almacen con estos chats y sus adjuntos (`(chat, [(nombre, bytes)])`).
/// Devuelve la raiz y los ids de los chats.
pub(crate) fn almacen_con(raiz: &Path, chats: &[(&str, Vec<(&str, Vec<u8>)>)]) -> Vec<String> {
    let mut fichas = Vec::new();
    for (i, (nombre, _)) in chats.iter().enumerate() {
        fichas.push(almacen::Ficha::nueva(nombre, 1 + i as i64, "PC01"));
    }
    almacen::Indice { proyectos: fichas.clone(), ..Default::default() }.guardar(raiz).unwrap();
    for (f, (_, adjuntos)) in fichas.iter().zip(chats) {
        for (k, (nombre, bytes)) in adjuntos.iter().enumerate() {
            let rel = almacen::guardar_adjunto(raiz, &f.id, nombre, bytes).unwrap();
            let sello = Sello { cuando: 1_000 + k as i64, numero: 1 + k as i64, aparato: "PC01".into(), proyecto: f.id.clone() };
            let m = Mensaje::adjunto(Clase::Archivo, nombre, &rel, bytes.len() as i64, &sello);
            cuaderno::anadir(&almacen::carpeta(raiz, &f.id), &m).unwrap();
        }
    }
    fichas.into_iter().map(|f| f.id).collect()
}

/// Un plano DXF pequeno con textos de calles y una raya.
pub(crate) fn dxf_con_calles() -> Vec<u8> {
    let texto = |x: f64, y: f64, h: f64, t: &str| format!("0\nTEXT\n8\n0\n10\n{x}\n20\n{y}\n30\n0\n40\n{h}\n1\n{t}\n");
    let mut s = String::from("0\nSECTION\n2\nENTITIES\n");
    s += &texto(100.0, 50.0, 2.5, "JR. ANTISUYO");
    s += &texto(20.0, 120.0, 2.5, "AV. TAHUANTINSUYO");
    s += &texto(150.0, 10.0, 1.0, "nota: ver Jr Antisuyo");
    s += "0\nLINE\n8\n0\n10\n0\n20\n0\n30\n0\n11\n200\n21\n150\n31\n0\n";
    s += "0\nENDSEC\n0\nEOF\n";
    s.into_bytes()
}

fn sin_mandar() -> Mandar {
    Box::new(|_| 1)
}

fn contexto<'a>(raiz: &'a Path, proyecto: &'a str, adjuntos: &'a [Adjunto], mandar: &'a Mandar, parar: &'a AtomicBool) -> Contexto<'a> {
    Contexto { raiz, proyecto, adjuntos, mandar, parar, idioma: pixpin_store::Idioma::Espanol }
}

#[test]
fn el_archivo_se_elige_por_lo_que_dice_el_usuario_y_si_hay_dos_se_pregunta() {
    let raiz = carpeta("elegir");
    let ids = almacen_con(
        &raiz,
        &[
            (
                "Obra Florida",
                vec![
                    ("senal-vertical-tahuantinsuyo.dwg", b"x".to_vec()),
                    ("senal-vertical-florida.dwg", b"x".to_vec()),
                    ("Memoria descriptiva.pdf", b"%PDF".to_vec()),
                ],
            ),
            ("Casa", vec![("plano casa.pdf", b"%PDF".to_vec())]),
        ],
    );
    let (mandar, parar) = (sin_mandar(), AtomicBool::new(false));
    let cx = contexto(&raiz, &ids[1], &[], &mandar, &parar);
    let c = elegir(&cx, Some("el plano de señalización de Tahuantinsuyo"), Sirve::Cualquiera).unwrap();
    assert_eq!(c.nombre, "senal-vertical-tahuantinsuyo.dwg");
    assert_eq!(c.proyecto_nombre, "Obra Florida");
    assert!(c.ruta.is_file());
    // Dos que encajan igual: se pregunta con la lista corta.
    let e = elegir(&cx, Some("plano de señalización vertical"), Sirve::Cualquiera).unwrap_err();
    assert!(e.contains("¿Cual?") && e.contains("tahuantinsuyo") && e.contains("florida"), "{e}");
    // El tipo dicho cuenta: «el pdf de la memoria».
    assert_eq!(elegir(&cx, Some("el pdf de la memoria"), Sirve::Cualquiera).unwrap().nombre, "Memoria descriptiva.pdf");
    // Sin nombre: lo mas reciente de este chat.
    assert_eq!(elegir(&cx, None, Sirve::Documento).unwrap().nombre, "plano casa.pdf");
    // Casos negativos: lo que no esta no se inventa, y una imagen no se anota.
    assert!(elegir(&cx, Some("presupuesto de la piscina"), Sirve::Cualquiera).unwrap_err().contains("no hay ningun archivo"));
    let vacio = contexto(&raiz, "no-existe", &[], &mandar, &parar);
    assert!(elegir(&vacio, None, Sirve::Escaneado).is_err());
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn abrir_por_nombre_manda_el_pedido_abrir_con_la_ruta() {
    let raiz = carpeta("abrir");
    let ids = almacen_con(&raiz, &[("Obra", vec![("senal-vertical-tahuantinsuyo.dwg", b"x".to_vec())])]);
    let mandados = Arc::new(Mutex::new(Vec::<String>::new()));
    let m2 = mandados.clone();
    let mandar: Mandar = Box::new(move |j| {
        m2.lock().unwrap().push(j.to_string());
        1
    });
    let parar = AtomicBool::new(false);
    let cx = contexto(&raiz, &ids[0], &[], &mandar, &parar);
    let a = pixpin_ia::acciones::validar(pixpin_ia::acciones::por_nombre(ABRIR).unwrap(), &serde_json::from_str(r#"{"descripcion":"plano de tahuantinsuyo"}"#).unwrap()).unwrap();
    let r = hacer(&a, &cx).unwrap();
    assert!(r.contains("tahuantinsuyo"), "{r}");
    let p = crate::pedidos::leer(&mandados.lock().unwrap()[0]).unwrap();
    assert!(matches!(p, crate::pedidos::Pedido::Abrir { que: crate::pedidos::Que::Fichero { .. } }), "{p:?}");
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn las_figuras_son_las_del_motor_comun() {
    let caja = [[100.0, 200.0, 300.0, 220.0]];
    let r = figuras(ComoAnotar::Resaltar, &caja, (255, 212, 59), None, &[], 20.0);
    assert_eq!(r.len(), 1);
    assert!(matches!(r[0].figura, Figura::Rectangulo) && r[0].opacidad < 0.5 && r[0].relleno.is_some());
    let s = figuras(ComoAnotar::Subrayar, &caja, (224, 49, 49), None, &[], 20.0);
    assert!(matches!(s[0].figura, Figura::Linea { .. }) && s[0].y >= 220.0);
    let e = figuras(ComoAnotar::Encerrar, &caja, (224, 49, 49), None, &[], 20.0);
    assert!(matches!(e[0].figura, Figura::Elipse) && e[0].x < 100.0 && e[0].x + e[0].ancho > 300.0);
    let n = figuras(ComoAnotar::Nota, &caja, (0, 0, 0), Some("Revisar esta calle con el topografo antes del viernes"), &[], 20.0);
    assert_eq!(n.len(), 2);
    assert!(matches!(&n[1].figura, Figura::Texto { texto, .. } if texto.contains('\n')), "la nota se parte en renglones");
    assert!(n[0].x >= 300.0, "la nota va al lado");
    let d = figuras(ComoAnotar::Dibujar, &[], (0, 0, 0), None, &[(0.0, 0.0), (10.0, 5.0), (20.0, 0.0)], 20.0);
    assert!(matches!(&d[0].figura, Figura::Linea { puntos } if puntos.len() == 3));
    // Casos negativos: sin texto no hay nota, sin dos puntos no hay trazo.
    assert!(figuras(ComoAnotar::Nota, &caja, (0, 0, 0), Some("  "), &[], 20.0).is_empty());
    assert!(figuras(ComoAnotar::Dibujar, &[], (0, 0, 0), None, &[(1.0, 1.0)], 20.0).is_empty());
    for e in r.iter().chain(&e).chain(&n) {
        assert!(e.grupos.iter().any(|g| g == GRUPO || g == crate::pines::marcas_texto::GRUPO));
    }
    assert_eq!(rgb(Some("verde"), ComoAnotar::Resaltar).1 > 100, true);
    assert_eq!(rgb(Some("no-es-color"), ComoAnotar::Marco), (0x3d, 0x8b, 0xff));
}

#[test]
fn los_textos_del_plano_se_encuentran_y_se_anota_alli() {
    let raiz = carpeta("plano");
    let plano = raiz.join("calles.dxf");
    std::fs::write(&plano, dxf_con_calles()).unwrap();
    let parar = AtomicBool::new(false);
    let e = buscar_dentro(&raiz, &plano, "Jr. Antisuyo", &parar).unwrap();
    assert_eq!(e.cuantos, 2, "{}", e.texto);
    // El grande primero, y el visor va alli.
    let z = e.pedido["que"]["zona"].as_array().unwrap();
    let (x, y) = (z[0].as_f64().unwrap(), z[1].as_f64().unwrap());
    assert!((95.0..130.0).contains(&x) && (45.0..60.0).contains(&y), "{z:?}");
    assert!(crate::pedidos::leer(&e.pedido.to_string()).is_ok());
    assert!(buscar_dentro(&raiz, &plano, "Collasuyo", &parar).is_err());
    // Anotar: encerrar el texto, en la capa del plano.
    let a = anotar_en(&raiz, &plano, &json!({"tipo": "encerrar", "texto": "JR. ANTISUYO"}), &parar).unwrap();
    assert_eq!(a.figuras, 1);
    assert!(a.vista.is_some() && a.pagina.is_none());
    let rutas = crate::plano_cad::anotar::rutas_de_la_capa(&raiz, &plano);
    assert!(rutas.hoja.is_file(), "la .hoja va al lado");
    let capa = crate::lector_tinta::Capa::leer(&rutas.tinta);
    let el: Vec<&Elemento> = capa.escena.elementos.iter().filter(|e| e.grupos.iter().any(|g| g == GRUPO)).collect();
    assert_eq!(el.len(), 1);
    assert!(matches!(el[0].figura, Figura::Elipse));
    // Un marco para imprimir es el del visor (lo lee como marco).
    anotar_en(&raiz, &plano, &json!({"tipo": "marco", "texto": "AV. TAHUANTINSUYO"}), &parar).unwrap();
    let m = crate::plano_cad::modelo(&raiz, &plano).unwrap();
    let (_, marcos) = crate::plano_cad::anotar::cotas_y_marcos_de(&rutas, &m);
    assert_eq!(marcos.len(), 1);
    // Y lo de antes sigue (no se pisa).
    let capa = crate::lector_tinta::Capa::leer(&rutas.tinta);
    assert!(capa.escena.elementos.iter().any(|e| matches!(e.figura, Figura::Elipse)));
    // Caso negativo: un texto que no esta no deja nada.
    assert!(anotar_en(&raiz, &plano, &json!({"tipo": "resaltar", "texto": "Collasuyo"}), &parar).is_err());
    let _ = std::fs::remove_dir_all(&raiz);
}

// ---------------------------------------------------------------------------
// Ida y vuelta con el movil

struct Par {
    dir: PathBuf,
    pc: DiscoPc,
    movil: DiscoAndroid,
    reloj: Reloj,
}

impl Drop for Par {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn montar(etiqueta: &str) -> Par {
    let dir = carpeta(&format!("movil-{etiqueta}"));
    let pc = DiscoPc::nuevo(&dir.join("PixPin Max"));
    let movil = DiscoAndroid::nuevo(dir.join("movil/files"), "movil");
    presentar(&pc, "id-pc", "Portátil");
    presentar(&movil, "id-movil", "Teléfono");
    let p = Par { dir, pc, movil, reloj: Reloj::nuevo() };
    crear_grupo(&p.movil, "ABCDE23456", p.reloj.ahora());
    conectado(&p.pc, &p.movil, true, Some("ABCDE23456"), &p.reloj, |_| {}).unwrap();
    p
}

impl Par {
    fn raiz(&self) -> PathBuf {
        self.pc.raiz()
    }
    fn desde_pc(&self) {
        sincronizar(&self.pc, &self.movil, &[GENERAL], &self.reloj, &|| {});
    }
    fn desde_movil(&self) {
        sincronizar(&self.movil, &self.pc, &[GENERAL], &self.reloj, &|| {});
    }
    /// Un adjunto del chat general del movil; su codigo unico.
    fn adjunto_del_movil(&self, id: &str, fichero: &str, bytes: &[u8]) -> String {
        let rel = format!("guardados/{fichero}");
        let f = self.movil.raiz().join(&rel);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, bytes).unwrap();
        let m = Json::de_valor(&json!({
            "id": id, "cuando": self.reloj.tic(), "clase": "ARCHIVO", "ruta": self.movil.absoluta(&rel),
            "nombre": fichero, "numero": self.movil.leer_mensajes().len() + 1, "letra": "a",
        }));
        self.movil.anadir_mensaje(&m).unwrap();
        kotlin::unico(&m)
    }
    fn en_el_pc(&self, id: &str) -> PathBuf {
        let g = self.pc.ficha_de(GENERAL).unwrap();
        let c = Cuaderno::leer_de(&almacen::carpeta(&self.raiz(), &g.id)).unwrap();
        let m = c.mensajes.into_iter().find(|m| m.id == id).unwrap();
        vista::ruta_real(&self.raiz(), &g.id, m.ruta.as_deref().unwrap()).unwrap()
    }
    fn en_el_movil(&self, rel: &str) -> PathBuf {
        self.movil.raiz().join(rel)
    }
}

fn de_gz(ruta: &Path) -> String {
    use std::io::Read;
    let b = std::fs::read(ruta).unwrap_or_else(|_| panic!("no llego {}", ruta.display()));
    let mut s = String::new();
    flate2::read::GzDecoder::new(&b[..]).read_to_string(&mut s).unwrap();
    s
}

fn gz(ruta: &Path, texto: &str) {
    use std::io::Write;
    let mut e = flate2::write::GzEncoder::new(std::fs::File::create(ruta).unwrap(), flate2::Compression::default());
    e.write_all(texto.as_bytes()).unwrap();
    e.finish().unwrap();
    let t = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
    std::fs::OpenOptions::new().write(true).open(ruta).unwrap().set_modified(t).unwrap();
}

fn del_asistente(texto: &str) -> Vec<Value> {
    let v: Value = serde_json::from_str(texto).unwrap();
    v["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["groupIds"].as_array().is_some_and(|g| g.iter().any(|g| g == GRUPO || g == crate::pines::marcas_texto::GRUPO)))
        .cloned()
        .collect()
}

/// **Lo que el asistente anota en un plano del chat llega al movil** con
/// el nombre del mensaje y su `.hoja`, y lo que el movil borra vuelve
/// borrado.
#[test]
fn lo_que_anota_el_asistente_en_un_plano_viaja_y_vuelve() {
    let p = montar("plano");
    let u = p.adjunto_del_movil("3001", "calles.dxf", &dxf_con_calles());
    p.desde_movil();
    let plano = p.en_el_pc("3001");
    let parar = AtomicBool::new(false);
    anotar_en(&p.raiz(), &plano, &json!({"tipo": "encerrar", "texto": "JR. ANTISUYO", "color": "rojo"}), &parar).unwrap();
    anotar_en(&p.raiz(), &plano, &json!({"tipo": "nota", "texto": "AV. TAHUANTINSUYO", "nota": "Falta la senal R-1"}), &parar).unwrap();
    p.desde_pc();
    let capa = p.en_el_movil(&format!("pins/draw/anot-{u}.excalidraw.gz"));
    let llegado = de_gz(&capa);
    let nuestros = del_asistente(&llegado);
    assert_eq!(nuestros.len(), 3, "elipse + papel + texto de la nota: {llegado}");
    assert!(nuestros.iter().any(|e| e["type"] == "ellipse"));
    assert!(nuestros.iter().any(|e| e["type"] == "text" && e["text"].as_str().unwrap_or("").contains("R-1")));
    assert!(p.en_el_movil(&format!("pins/draw/anot-{u}.hoja")).is_file(), "la .hoja viaja");
    // El movil borra la elipse y vuelve.
    let mut v: Value = serde_json::from_str(&llegado).unwrap();
    for e in v["elements"].as_array_mut().unwrap() {
        if e["type"] == "ellipse" {
            e["isDeleted"] = json!(true);
            e["version"] = json!(e["version"].as_u64().unwrap_or(1) + 5);
            e["updated"] = json!(p.reloj.tic());
        }
    }
    gz(&capa, &v.to_string());
    p.desde_movil();
    let rutas = crate::plano_cad::anotar::rutas_de_la_capa(&p.raiz(), &plano);
    let capa_pc = crate::lector_tinta::Capa::leer(&rutas.tinta);
    let elipses: Vec<&Elemento> = capa_pc.escena.elementos.iter().filter(|e| matches!(e.figura, Figura::Elipse)).collect();
    assert!(elipses.iter().all(|e| e.borrado), "lo borrado en el movil vuelve borrado");
    assert!(capa_pc.escena.elementos.iter().any(|e| matches!(&e.figura, Figura::Texto { texto, .. } if texto.contains("R-1")) && !e.borrado));
}

/// La muestra de PDF con texto, si esta (`E:\pixpin-demo-om\Tesis.pdf`).
pub(crate) fn tesis() -> Option<Vec<u8>> {
    let r = std::env::var("PIXPIN_TESIS").unwrap_or_else(|_| r"E:\pixpin-demo-om\Tesis.pdf".into());
    std::fs::read(r).ok()
}

/// **Lo que el asistente resalta en un PDF del chat llega al movil** en la
/// capa de su pagina (`anot-<uid>-p<n>`) con su `.hoja`, en unidades de la
/// hoja (1400 de ancho), y vuelve.
#[test]
fn lo_que_resalta_el_asistente_en_un_pdf_viaja_y_vuelve() {
    let Some(bytes) = tesis() else {
        println!("sin la muestra Tesis.pdf: no se prueba");
        return;
    };
    let p = montar("pdf");
    let u = p.adjunto_del_movil("4001", "Tesis.pdf", &bytes);
    p.desde_movil();
    let pdf = p.en_el_pc("4001");
    let parar = AtomicBool::new(false);
    // Una palabra que este en el texto: la primera larga de la pagina 2.
    let pags = paginas(&pdf, &parar).unwrap();
    let palabra = pags.textos[1.min(pags.textos.len() - 1)]
        .texto
        .split_whitespace()
        .find(|w| w.chars().count() >= 7 && w.chars().all(char::is_alphabetic))
        .expect("una palabra larga")
        .to_string();
    let a = anotar_en(&p.raiz(), &pdf, &json!({"tipo": "resaltar", "texto": palabra}), &parar).unwrap();
    let hoja = a.pagina.unwrap();
    assert!(a.figuras >= 1);
    // El lector la lee donde la busca (`DondeVa`).
    let d = crate::lector_pdf_proyecto::DondeVa::de(&p.raiz(), &pdf, 0);
    let capa = d.leer_capa(&pdf, hoja, 0, pags.altos[hoja]);
    let r: Vec<&Elemento> = capa.escena.elementos.iter().filter(|e| e.grupos.iter().any(|g| g == GRUPO)).collect();
    assert!(!r.is_empty());
    assert!(r.iter().all(|e| e.x >= 0.0 && e.x + e.ancho <= ANCHO_HOJA + 50.0 && e.y >= 0.0 && e.y <= pags.altos[hoja]), "en la hoja");
    p.desde_pc();
    let capa_movil = p.en_el_movil(&format!("pins/draw/anot-{u}-p{hoja}.excalidraw.gz"));
    let llegado = de_gz(&capa_movil);
    assert!(!del_asistente(&llegado).is_empty(), "{llegado}");
    let marco = std::fs::read_to_string(p.en_el_movil(&format!("pins/draw/anot-{u}-p{hoja}.hoja"))).expect("la .hoja viaja");
    assert!(marco.starts_with("0,0,1400,"), "{marco}");
    // Vuelta: el movil pone su propio trazo y llega al PC sin perder lo nuestro.
    let mut v: Value = serde_json::from_str(&llegado).unwrap();
    v["elements"].as_array_mut().unwrap().push(json!({
        "id": "del-movil", "type": "rectangle", "x": 10, "y": 10, "width": 50, "height": 30, "angle": 0,
        "strokeColor": "#000000", "backgroundColor": "transparent", "fillStyle": "solid", "strokeWidth": 2,
        "strokeStyle": "solid", "roughness": 0, "opacity": 100, "groupIds": [], "seed": 1, "version": 1,
        "versionNonce": 1, "isDeleted": false, "updated": p.reloj.tic(), "boundElements": null, "locked": false
    }));
    gz(&capa_movil, &v.to_string());
    p.desde_movil();
    let capa = d.leer_capa(&pdf, hoja, 0, pags.altos[hoja]);
    assert!(capa.escena.elementos.iter().any(|e| matches!(e.figura, Figura::Rectangulo) && (e.ancho - 50.0).abs() < 0.5));
    assert!(capa.escena.elementos.iter().any(|e| e.grupos.iter().any(|g| g == GRUPO) && !e.borrado));
    // Caso negativo: lo que no esta en el PDF no se anota.
    assert!(anotar_en(&p.raiz(), &pdf, &json!({"tipo": "subrayar", "texto": "zxqwv inexistente"}), &parar).is_err());
}
