//! Pruebas de `hoja_a_lienzo` sin red: una hoja dibujada aqui (palabras de
//! trazos con su texto aparte, como las daria el OCR), un modelo de mentira
//! y una escena de verdad como destino.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;

use pixpin_ia::hoja::{self, Opciones, PalabraHoja, Tipo};
use pixpin_ia::motor::{MotorGuionado, Respuesta};
use pixpin_motor2d::{Escena, Figura};

use super::super::al_lienzo::{self, EnHoja};
use super::*;

/// **Una escena como destino** (las pruebas y el banco): cada zona entra
/// como entraria en el lienzo abierto, con su sesion.
pub(crate) struct EnEscena {
    pub escena: Escena,
    pub imagenes: HashMap<u64, pixpin_codec::ImagenRgba>,
    pub entregas: usize,
}

impl EnEscena {
    pub(crate) fn nueva() -> EnEscena {
        EnEscena { escena: Escena::nueva(), imagenes: HashMap::new(), entregas: 0 }
    }
}

impl Destino for EnEscena {
    fn entregar(&mut self, h: Option<&Hecho>, en: EnHoja) -> Result<(), String> {
        let (doc, imgs) = documento_de(h);
        al_lienzo::meter_de_hoja(&mut self.escena, &doc, &en, Some((en.tam.0 / 2.0, en.tam.1 / 2.0)))?;
        self.imagenes.extend(imgs);
        self.entregas += 1;
        Ok(())
    }
}

/// Una hoja dibujada: «palabras» de trazos y su texto aparte.
pub(crate) struct Papel {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
    pub palabras: Vec<PalabraHoja>,
}

impl Papel {
    pub(crate) fn nuevo(w: u32, h: u32) -> Papel {
        Papel { w, h, rgba: vec![255; (w * h * 4) as usize], palabras: Vec::new() }
    }
    fn pixel(&mut self, x: i64, y: i64, g: u8) {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        self.rgba[i..i + 3].fill(g);
        self.rgba[i + 3] = 255;
    }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, g: u8) {
        for yy in y as i64..(y + h) as i64 {
            for xx in x as i64..(x + w) as i64 {
                self.pixel(xx, yy, g);
            }
        }
    }
    fn palabra(&mut self, texto: &str, x: f64, y: f64, alto: f64) -> f64 {
        let w = texto.chars().count() as f64 * alto * 0.5;
        for xx in (x as i64..(x + w) as i64).step_by(4) {
            for yy in (y + alto * 0.15) as i64..(y + alto * 0.85) as i64 {
                self.pixel(xx, yy, 20);
            }
        }
        self.palabras.push(PalabraHoja::nueva(texto, x, y, w, alto));
        w
    }
    fn parrafo(&mut self, x: f64, y: f64, ancho: f64, n: usize, alto: f64) -> f64 {
        let mut yy = y;
        for i in 0..n {
            let mut cx = x;
            let fin = if i + 1 == n { x + ancho * 0.6 } else { x + ancho };
            for k in 0.. {
                let p = ["lorem", "ipsum", "dolor", "sit", "amet", "consectetur"][k % 6];
                if cx + p.len() as f64 * alto * 0.5 > fin {
                    break;
                }
                cx += self.palabra(p, cx, yy, alto) + alto * 0.35;
            }
            yy += alto * 1.5;
        }
        yy
    }
    fn foto(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let mut s: u32 = 7;
        for yy in y as i64..(y + h) as i64 {
            for xx in x as i64..(x + w) as i64 {
                s = s.wrapping_mul(1103515245).wrapping_add(12345);
                let g = 70.0 + (xx as f64 - x) / w * 90.0 + ((s >> 16) % 40) as f64;
                self.pixel(xx, yy, g as u8);
            }
        }
    }
    pub(crate) fn pagina(&self) -> Pagina {
        Pagina { rgba: self.rgba.clone(), ancho: self.w, alto: self.h, palabras: self.palabras.clone(), ms_leer: 0 }
    }
}

/// Una hoja con titulo, dos parrafos, una formula, una tabla rayada y una
/// foto.
fn hoja_completa() -> Papel {
    let mut p = Papel::nuevo(1000, 1400);
    p.palabra("Informe", 400.0, 60.0, 44.0);
    let y = p.parrafo(100.0, 160.0, 800.0, 4, 20.0);
    for (k, t) in ["E", "=", "m", "c^2"].iter().enumerate() {
        p.palabra(t, 430.0 + k as f64 * 40.0, y + 30.0, 22.0);
    }
    let y = p.parrafo(100.0, y + 100.0, 800.0, 3, 20.0);
    // Tabla rayada 3x3.
    let (tx, ty) = (200.0, y + 40.0);
    for f in 0..3 {
        for c in 0..3 {
            p.palabra(["Cemento", "12", "450"][c], tx + c as f64 * 200.0 + 12.0, ty + f as f64 * 44.0 + 12.0, 18.0);
        }
    }
    for f in 0..=3 {
        p.rect(tx, ty + f as f64 * 44.0, 600.0, 2.0, 0);
    }
    for c in 0..=3 {
        p.rect(tx + c as f64 * 200.0, ty, 2.0, 132.0, 0);
    }
    p.foto(300.0, ty + 180.0, 400.0, 260.0);
    p
}

fn guion(n: usize, latex: &str) -> Arc<dyn pixpin_ia::Motor> {
    Arc::new(MotorGuionado::nuevo((0..n).map(|_| Respuesta::de_texto(latex)).collect()))
}

#[test]
fn una_hoja_entera_entra_en_el_lienzo_zona_a_zona_y_se_deshace_de_una_vez() {
    let papel = hoja_completa();
    let mut d = EnEscena::nueva();
    let mut avances = Vec::new();
    let o = Opciones { comprobar: false, ..Default::default() };
    let inf = pasar(&papel.pagina(), Some(guion(4, "E = mc^2")), &o, false, 41, &AtomicBool::new(false), &mut d, &mut |a, b| avances.push((a, b)))
        .unwrap();
    let s = &inf.seccionado;
    for t in [Tipo::Encabezado, Tipo::Parrafo, Tipo::Formula, Tipo::Tabla, Tipo::Foto] {
        assert!(s.cuantas(t) >= 1, "falta {t:?}: {:?}", s.zonas.iter().map(|z| z.tipo).collect::<Vec<_>>());
    }
    // El progreso cuenta todas las zonas, de una en una.
    assert_eq!(avances.last(), Some(&(s.zonas.len(), s.zonas.len())));
    assert_eq!(inf.peticiones, s.cuantas(Tipo::Formula) + s.cuantas(Tipo::Grafica) + s.cuantas(Tipo::Dudosa));
    assert!(inf.ms_primera.is_some());
    // La formula salio compuesta (no como imagen) y la foto como imagen.
    let f = inf.hechos.iter().find(|h| h.tipo == Tipo::Formula).unwrap();
    assert!(f.imagenes.is_empty(), "{}", f.nota);
    let foto = inf.hechos.iter().find(|h| h.tipo == Tipo::Foto).unwrap();
    assert_eq!(foto.imagenes.len(), 1);
    assert!(d.imagenes.contains_key(&foto.imagenes[0].id));
    // En la escena, con la foto, el texto y la tabla.
    let els: Vec<_> = d.escena.visibles().cloned().collect();
    assert!(els.iter().any(|e| matches!(e.figura, Figura::Imagen { .. })));
    assert!(els.iter().any(|e| matches!(&e.figura, Figura::Texto { texto, .. } if texto.contains("lorem"))));
    assert!(els.iter().any(|e| matches!(&e.figura, Figura::Texto { texto, .. } if texto == "Cemento")));
    // Ancho de la hoja: 1000 px por omision.
    let caja = d.escena.caja().unwrap();
    assert!(caja.2 - caja.0 <= 1010.0, "{caja:?}");
    // Un Ctrl+Z quita la hoja entera.
    assert!(!al_lienzo::hay_sesion(41));
    assert!(d.escena.deshacer());
    assert_eq!(d.escena.cuantos_visibles(), 0);
}

#[test]
fn sin_modelo_las_formulas_van_como_imagen_y_nada_se_pierde() {
    let papel = hoja_completa();
    let mut d = EnEscena::nueva();
    let o = Opciones { comprobar: false, ..Default::default() };
    let inf = pasar(&papel.pagina(), None, &o, false, 42, &AtomicBool::new(false), &mut d, &mut |_, _| {}).unwrap();
    assert_eq!(inf.peticiones, 0);
    let f = inf.hechos.iter().find(|h| h.tipo == Tipo::Formula).unwrap();
    assert_eq!(f.imagenes.len(), 1, "{}", f.nota);
    assert_eq!(inf.hechos.len(), inf.seccionado.zonas.len());
}

#[test]
fn si_el_modelo_falla_la_zona_sale_igual() {
    let papel = hoja_completa();
    let mut d = EnEscena::nueva();
    let o = Opciones { comprobar: false, ..Default::default() };
    // Un guion vacio: cada pregunta falla.
    let inf = pasar(&papel.pagina(), Some(guion(0, "")), &o, false, 43, &AtomicBool::new(false), &mut d, &mut |_, _| {}).unwrap();
    assert!(!inf.fallos.is_empty());
    assert_eq!(inf.hechos.len(), inf.seccionado.zonas.len());
}

#[test]
fn la_comprobacion_mide_cada_zona() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let papel = hoja_completa();
    let mut d = EnEscena::nueva();
    let o = Opciones::default();
    let inf = pasar(&papel.pagina(), Some(guion(4, "E = mc^2")), &o, false, 44, &AtomicBool::new(false), &mut d, &mut |_, _| {}).unwrap();
    assert_eq!(inf.parecidos.len(), inf.hechos.len());
    // La foto es el mismo recorte: casi igual.
    let foto = inf.hechos.iter().find(|h| h.tipo == Tipo::Foto).unwrap().zona;
    let p = inf.parecidos.iter().find(|(z, _)| *z == foto).unwrap().1;
    assert!(p > 0.8, "foto {p}");
    // Lo rehecho sustituyo a lo de antes: un solo grupo por zona.
    assert!(inf.rehechas <= inf.hechos.len());
    assert!(d.escena.deshacer());
    assert_eq!(d.escena.cuantos_visibles(), 0, "todo, rehecho incluido, en un paso");
}

#[test]
fn una_hoja_en_blanco_se_dice_y_no_toca_el_lienzo() {
    let papel = Papel::nuevo(800, 1000);
    let mut d = EnEscena::nueva();
    let e = pasar(&papel.pagina(), None, &Opciones::default(), false, 45, &AtomicBool::new(false), &mut d, &mut |_, _| {}).unwrap_err();
    assert!(e.contains("en blanco"), "{e}");
    assert_eq!(d.entregas, 0);
}

#[test]
fn una_foto_sin_texto_entra_como_una_foto() {
    let mut papel = Papel::nuevo(800, 600);
    papel.foto(0.0, 0.0, 800.0, 600.0);
    let mut d = EnEscena::nueva();
    let o = Opciones { comprobar: false, ..Default::default() };
    let inf = pasar(&papel.pagina(), None, &o, false, 46, &AtomicBool::new(false), &mut d, &mut |_, _| {}).unwrap();
    assert_eq!(inf.hechos.len(), 1);
    assert_eq!(inf.hechos[0].tipo, Tipo::Foto);
    assert_eq!(d.escena.cuantos_visibles(), 1);
}

#[test]
fn una_sola_columna_en_modo_fiel_pone_cada_renglon() {
    let mut papel = Papel::nuevo(900, 700);
    papel.parrafo(80.0, 80.0, 740.0, 6, 20.0);
    let mut d = EnEscena::nueva();
    let o = Opciones { comprobar: false, ..Default::default() };
    let inf = pasar(&papel.pagina(), None, &o, true, 47, &AtomicBool::new(false), &mut d, &mut |_, _| {}).unwrap();
    assert_eq!(inf.seccionado.columnas, 1);
    assert_eq!(inf.hechos.len(), 1);
    let textos = d.escena.visibles().filter(|e| matches!(e.figura, Figura::Texto { .. })).count();
    assert_eq!(textos, 6, "un texto por renglon");
}

#[test]
fn las_zonas_de_una_hoja_van_juntas_y_rehacer_cambia_solo_la_suya() {
    let doc = |x: f64| {
        pixpin_ia::lienzo::documento(vec![serde_json::json!({
            "id": format!("z{x}"), "type": "rectangle", "x": x, "y": 10.0, "width": 50.0, "height": 20.0,
            "strokeColor": "#000000", "backgroundColor": "transparent", "groupIds": []
        })])
    };
    let mut e = Escena::nueva();
    // Algo que ya habia (lienzo cerrado: lo de la hoja va a su derecha).
    pixpin_motor2d::portapapeles::pegar(&mut e, &al_lienzo::elementos_de(&doc(0.0)).unwrap(), 0.0, 0.0);
    let en = |zona, rehacer, ultima| EnHoja { sesion: 9001, zona, rehacer, ultima, tam: (1000.0, 1400.0) };
    al_lienzo::meter_de_hoja(&mut e, &doc(100.0), &en(0, false, false), None).unwrap();
    al_lienzo::meter_de_hoja(&mut e, &doc(300.0), &en(1, false, false), None).unwrap();
    let xs: Vec<f32> = e.visibles().map(|x| x.x).collect();
    // El mismo desplazamiento para las dos: siguen a 200 una de otra.
    assert!((xs[2] - xs[1] - 200.0).abs() < 1e-3, "{xs:?}");
    assert!(xs[1] > 50.0, "a la derecha de lo que habia: {xs:?}");
    // Rehacer la zona 0: se va la vieja y entra la nueva; la 1 sigue.
    let antes = xs.clone();
    al_lienzo::meter_de_hoja(&mut e, &doc(120.0), &en(0, true, false), None).unwrap();
    let xs: Vec<f32> = e.visibles().map(|x| x.x).collect();
    assert_eq!(xs.len(), 3, "{xs:?}");
    assert!(!xs.iter().any(|x| (x - antes[1]).abs() < 1e-3), "la vieja se fue: {xs:?}");
    assert!(xs.iter().any(|x| (x - (antes[1] + 20.0)).abs() < 1e-3), "{xs:?}");
    assert!(xs.iter().any(|x| (x - antes[2]).abs() < 1e-3), "la otra zona sigue: {xs:?}");
    al_lienzo::meter_de_hoja(&mut e, "", &en(0, false, true), None).unwrap();
    assert!(!al_lienzo::hay_sesion(9001));
    // Un solo Ctrl+Z quita la hoja y deja lo de antes.
    assert!(e.deshacer());
    assert_eq!(e.cuantos_visibles(), 1);
}

/// Banco del 15-oct (cuadratica 32/65): dos zonas con los MISMOS ids (y
/// iguales a algo que ya habia en el lienzo). Rehacer una no toca la otra ni
/// lo de antes.
#[test]
fn rehacer_una_zona_no_toca_las_demas_aunque_repitan_ids() {
    let doc = |x: f64| {
        pixpin_ia::lienzo::documento(vec![
            serde_json::json!({
                "id": "ia-1-1", "type": "rectangle", "x": x, "y": 10.0, "width": 50.0, "height": 20.0,
                "strokeColor": "#000000", "backgroundColor": "transparent", "groupIds": [],
                "boundElements": [{"id": "ia-1-2", "type": "text"}]
            }),
            serde_json::json!({
                "id": "ia-1-2", "type": "text", "x": x + 5.0, "y": 12.0, "width": 30.0, "height": 16.0, "text": "a",
                "fontSize": 14, "fontFamily": 5, "containerId": "ia-1-1",
                "strokeColor": "#000000", "backgroundColor": "transparent", "groupIds": []
            }),
        ])
    };
    let mut e = Escena::nueva();
    pixpin_motor2d::portapapeles::pegar(&mut e, &al_lienzo::elementos_de(&doc(0.0)).unwrap(), 0.0, 0.0);
    let en = |zona, rehacer, ultima| EnHoja { sesion: 9003, zona, rehacer, ultima, tam: (1000.0, 1400.0) };
    al_lienzo::meter_de_hoja(&mut e, &doc(100.0), &en(0, false, false), None).unwrap();
    al_lienzo::meter_de_hoja(&mut e, &doc(300.0), &en(1, false, false), None).unwrap();
    assert_eq!(e.cuantos_visibles(), 6);
    let ids: std::collections::HashSet<String> = e.visibles().map(pixpin_motor2d::enlace::id_de_texto_de).collect();
    assert_eq!(ids.len(), 6, "ningun id repetido: {ids:?}");
    // Cada rotulo sigue en SU caja (el contenedor se renombro con ella).
    for t in e.visibles().filter(|x| matches!(x.figura, Figura::Texto { .. })) {
        let c = t.extras.contenedor.clone().expect("con caja");
        let caja = e.visibles().find(|x| pixpin_motor2d::enlace::id_de_texto_de(x) == c).expect("su caja existe");
        assert!((t.x - caja.x - 5.0).abs() < 1e-3, "rotulo en su caja");
    }
    let rect_x = |e: &Escena| {
        let mut v: Vec<f32> = e.visibles().filter(|x| matches!(x.figura, Figura::Rectangulo)).map(|x| x.x).collect();
        v.sort_by(f32::total_cmp);
        v
    };
    let antes = rect_x(&e);
    al_lienzo::meter_de_hoja(&mut e, &doc(120.0), &en(0, true, false), None).unwrap();
    let despues = rect_x(&e);
    assert_eq!(e.cuantos_visibles(), 6, "{despues:?}");
    assert_eq!(despues[0], antes[0], "lo de antes sigue");
    assert_eq!(despues[2], antes[2], "la otra zona sigue");
    assert!((despues[1] - antes[1] - 20.0).abs() < 1e-3, "la zona rehecha cambio: {antes:?} {despues:?}");
    al_lienzo::meter_de_hoja(&mut e, "", &en(0, false, true), None).unwrap();
}

#[test]
fn si_el_usuario_dibuja_en_medio_cada_zona_se_deshace_por_su_cuenta() {
    let doc = |x: f64| {
        pixpin_ia::lienzo::documento(vec![serde_json::json!({
            "id": format!("q{x}"), "type": "rectangle", "x": x, "y": 10.0, "width": 50.0, "height": 20.0,
            "strokeColor": "#000000", "backgroundColor": "transparent", "groupIds": []
        })])
    };
    let mut e = Escena::nueva();
    let en = |zona, ultima| EnHoja { sesion: 9002, zona, rehacer: false, ultima, tam: (1000.0, 1400.0) };
    al_lienzo::meter_de_hoja(&mut e, &doc(0.0), &en(0, false), Some((500.0, 700.0))).unwrap();
    // El usuario pega algo suyo entre dos zonas.
    pixpin_motor2d::portapapeles::pegar(&mut e, &al_lienzo::elementos_de(&doc(900.0)).unwrap(), 0.0, 0.0);
    al_lienzo::meter_de_hoja(&mut e, &doc(200.0), &en(1, true), Some((500.0, 700.0))).unwrap();
    assert!(e.deshacer());
    assert_eq!(e.cuantos_visibles(), 2, "solo la ultima zona: lo suyo sigue");
}

#[test]
fn con_el_motor_local_no_se_intenta() {
    let (tx, _rx) = channel();
    let parar = Arc::new(AtomicBool::new(false));
    let adj = vec![super::super::turno::Adjunto { nombre: "hoja.png".into(), ruta: std::env::temp_dir().join("no-existe-hoja.png") }];
    let mut ej = super::super::ejecutor::EjecutorDelChat::nuevo(tx, "p".into(), adj, parar);
    let fila = pixpin_ia::acciones::por_nombre(hoja::NOMBRE).unwrap();
    let a = pixpin_ia::acciones::validar(fila, &serde_json::Map::new()).unwrap();
    use pixpin_ia::agente::Ejecutor;
    let r = ej.ejecutar(&pixpin_ia::herramientas::Llamada::App(a));
    assert_eq!(r.unwrap_err(), hoja::SOLO_NUBE);
    // Y el turno con un motor local ni pregunta al modelo.
    let motor = MotorGuionado::nuevo(Vec::new());
    let mut h = vec![pixpin_ia::Mensaje::usuario("pasa esta hoja al lienzo")];
    struct Nada;
    impl pixpin_ia::agente::Ejecutor for Nada {
        fn ejecutar(&mut self, _: &pixpin_ia::herramientas::Llamada) -> Result<String, String> {
            panic!("no se ejecuta nada")
        }
    }
    let f = pixpin_ia::agente::turno_con(&motor, &mut h, &pixpin_ia::herramientas::catalogo(), &mut Nada, &Default::default(), &mut |_| {
        pixpin_ia::motor::Seguir::Si
    })
    .unwrap();
    assert_eq!(f.texto, hoja::SOLO_NUBE);
    // Ni se le ofrece al local; a la nube, si.
    assert!(!pixpin_ia::seleccion::para_el_pedido("pasa esta hoja al lienzo", true).iter().any(|h| h.nombre == hoja::NOMBRE));
    assert!(pixpin_ia::herramientas::catalogo().iter().any(|h| h.nombre == hoja::NOMBRE));
}

#[test]
fn sin_hoja_adjunta_se_dice() {
    let r = elegir(&[], None).unwrap_err();
    assert!(r.contains("adjunte"), "{r}");
    let adj = vec![super::super::turno::Adjunto { nombre: "acta.docx".into(), ruta: "acta.docx".into() }];
    assert!(elegir(&adj, None).is_err());
    let adj = vec![
        super::super::turno::Adjunto { nombre: "uno.pdf".into(), ruta: "uno.pdf".into() },
        super::super::turno::Adjunto { nombre: "dos.png".into(), ruta: "dos.png".into() },
    ];
    assert_eq!(elegir(&adj, None).unwrap().nombre, "dos.png");
    assert_eq!(elegir(&adj, Some("uno")).unwrap().nombre, "uno.pdf");
}

#[test]
fn el_rotulo_cuenta_las_zonas() {
    assert_eq!(super::super::chat::rotulo_de("hoja_a_lienzo:3/9"), "📄 pasando la hoja al lienzo… 3/9 zonas");
    assert_eq!(super::super::chat::rotulo_de("hoja_a_lienzo"), "📄 pasando la hoja al lienzo…");
}

#[test]
fn los_trozos_del_pdf_se_parten_en_palabras() {
    let t = pixpin_pdf::texto::TrozoDeTexto { x: 10.0, y: 100.0, fin: 110.0, alto: 10.0, texto: "hola mundo".into(), negrita: true, cursiva: false };
    let p = palabras_de_trozos(&[t], 2.0);
    assert_eq!(p.len(), 2);
    assert_eq!(p[0].texto, "hola");
    assert!((p[0].caja.x - 20.0).abs() < 1e-6);
    assert!((p[1].caja.x - (10.0 + 5.0 * 10.0) * 2.0).abs() < 1e-6, "{:?}", p[1].caja);
    assert!(p.iter().all(|w| w.negrita));
}

#[test]
#[ignore = "depurar el OCR de una hoja del banco"]
fn depurar_renglones() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let ruta = std::path::PathBuf::from(std::env::var("PIXPIN_HOJA_PNG").unwrap());
    let p = leer(&ruta, None).unwrap();
    let (y0, y1) = (400.0, 500.0);
    for w in p.palabras.iter().filter(|w| w.caja.y > y0 && w.caja.y < y1 && w.caja.x < 850.0) {
        println!("{:>14} {:.0},{:.0} {:.0}x{:.0}", w.texto, w.caja.x, w.caja.y, w.caja.w, w.caja.h);
    }
    let hm = pixpin_ia::hoja::mediana(p.palabras.iter().map(|w| w.caja.h).collect()).unwrap();
    println!("hm {hm}");
    for r in pixpin_ia::hoja::seccion::renglones(&p.palabras, &[], hm).iter().filter(|r| r.caja.y > y0 && r.caja.y < y1) {
        println!("R {:.0},{:.0} {:.0}x{:.0} {}", r.caja.x, r.caja.y, r.caja.w, r.caja.h, r.texto);
    }
}

/// Un modelo que no contesta nunca (la red colgada, medido con DeepSeek el
/// 14-oct): la hoja acaba igual, al plazo, con las formulas como imagen.
#[test]
fn si_la_nube_se_cuelga_la_hoja_acaba_al_plazo() {
    struct Colgado;
    impl pixpin_ia::Motor for Colgado {
        fn completar(&self, _: &pixpin_ia::Peticion, _: &mut dyn FnMut(&pixpin_ia::motor::Trozo) -> pixpin_ia::motor::Seguir) -> Result<Respuesta, pixpin_ia::ErrorIa> {
            std::thread::sleep(std::time::Duration::from_secs(20));
            Err(pixpin_ia::ErrorIa::Cancelado)
        }
    }
    let papel = hoja_completa();
    let mut d = EnEscena::nueva();
    let o = Opciones { comprobar: false, plazo_nube: std::time::Duration::from_millis(400), ..Default::default() };
    let t = std::time::Instant::now();
    let inf = pasar(&papel.pagina(), Some(Arc::new(Colgado)), &o, false, 48, &AtomicBool::new(false), &mut d, &mut |_, _| {}).unwrap();
    assert!(t.elapsed() < std::time::Duration::from_secs(5), "{:?}", t.elapsed());
    assert_eq!(inf.hechos.len(), inf.seccionado.zonas.len());
    assert!(inf.fallos.iter().any(|f| f.contains("a tiempo")), "{:?}", inf.fallos);
}

/// Banco del 15-oct (sistema escaneado): el renglon que el OCR leyo aparte
/// sobra dentro del sistema compuesto; la etiqueta «a)» no.
#[test]
fn un_texto_que_la_formula_ya_dice_sobra_y_una_etiqueta_no() {
    let sistema = r"\begin{cases} 2x + 3y = 7 \ x - y = 1 \end{cases}";
    assert!(ya_lo_dice(sistema, "x- y= 1"));
    assert!(ya_lo_dice(sistema, "2x + 3y = 7"));
    assert!(!ya_lo_dice(sistema, "a)"));
    assert!(!ya_lo_dice(sistema, "Resuelve los sistemas por sustitucion"));
}
