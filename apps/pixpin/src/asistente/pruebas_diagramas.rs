//! Pruebas del 15-oct: la imagen del lienzo como entrada de «calca esto», lo
//! nuevo a la vista y junto a su imagen (no en x = 15000), y que una lamina
//! (texto y figuras) va por figuras exactas y una foto no.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use pixpin_motor2d::{Elemento, Escena};

use super::al_lienzo;
use super::ejecutor::EjecutorDelChat;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Elemento {
    Elemento {
        x,
        y,
        ancho: w,
        alto: h,
        ..Default::default()
    }
}

fn se_pisan(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    a.0 < b.2 && a.2 > b.0 && a.1 < b.3 && a.3 > b.1
}

fn un_rectangulo(w: f64, h: f64) -> String {
    pixpin_ia::lienzo::documento(vec![serde_json::json!({
        "id": "r", "type": "rectangle", "x": 0, "y": 0, "width": w, "height": h,
        "strokeColor": "#1e1e1e", "backgroundColor": "transparent", "fillStyle": "solid", "strokeWidth": 2,
        "strokeStyle": "solid", "roughness": 0, "opacity": 100, "seed": 1, "version": 1, "isDeleted": false,
        "groupIds": [], "boundElements": null, "locked": false
    })])
}

#[test]
fn lo_nuevo_va_a_la_derecha_de_su_imagen_sin_tapar_nada() {
    let mut e = Escena::nueva();
    e.anadir(crate::imagenes_lienzo::elemento_imagen(
        1, 100.0, 100.0, 400.0, 300.0,
    ));
    let fuente = (100.0, 100.0, 500.0, 400.0);
    let vista = (0.0, 0.0, 1600.0, 1000.0);
    let (x, y) = al_lienzo::sitio_libre(&e, (400.0, 300.0), Some(fuente), Some(vista));
    assert!(
        x >= 500.0 && (y - 100.0).abs() < 1.0,
        "a la derecha: ({x}, {y})"
    );
    assert!(!se_pisan((x, y, x + 400.0, y + 300.0), fuente));
    // Con algo a la derecha: en otro sitio libre, y a la vista.
    e.anadir(rect(520.0, 80.0, 600.0, 400.0));
    let (x, y) = al_lienzo::sitio_libre(&e, (400.0, 300.0), Some(fuente), Some(vista));
    let nueva = (x, y, x + 400.0, y + 300.0);
    for c in e.visibles().map(Elemento::caja) {
        assert!(!se_pisan(nueva, c), "{nueva:?} pisa {c:?}");
    }
    assert!(
        nueva.0 >= vista.0 && nueva.1 >= vista.1 && nueva.2 <= vista.2 && nueva.3 <= vista.3,
        "a la vista: {nueva:?}"
    );
}

#[test]
fn en_un_lienzo_muy_ancho_lo_nuevo_no_acaba_en_x_15000() {
    // Lo de antes: «a la derecha de todo lo que habia».
    let mut e = Escena::nueva();
    e.anadir(rect(14_000.0, 0.0, 800.0, 600.0));
    e.anadir(crate::imagenes_lienzo::elemento_imagen(
        2, 0.0, 0.0, 1052.0, 592.0,
    ));
    let doc = un_rectangulo(500.0, 280.0);
    // Lienzo cerrado y sin decir de que imagen sale: junto a lo ultimo puesto (la imagen).
    let ids = al_lienzo::meter_en_sitio(&mut e, &doc, None, None).unwrap();
    let c = e.buscar(ids[0]).unwrap().caja();
    assert!(c.0 < 2000.0, "lo nuevo acabo en x = {}", c.0);
    // Con la imagen de origen y la vista sobre ella: a su lado y a la vista.
    let _ = al_lienzo::tomar_mostrar();
    let vista = (-100.0, -100.0, 1900.0, 1100.0);
    let ids = al_lienzo::meter_en_sitio(&mut e, &doc, Some((0.0, 0.0, 1052.0, 592.0)), Some(vista))
        .unwrap();
    let c = e.buscar(ids[0]).unwrap().caja();
    assert!(
        !se_pisan(c, (0.0, 0.0, 1052.0, 592.0)),
        "no tapa su imagen: {c:?}"
    );
    assert!(
        c.0 >= -100.0 && c.2 <= 1900.0 && c.1 >= -100.0 && c.3 <= 1100.0,
        "a la vista: {c:?}"
    );
    assert!(
        al_lienzo::tomar_mostrar().is_none(),
        "se veia: no hay que mover la vista"
    );
}

#[test]
fn si_no_cabe_a_la_vista_la_vista_va_a_ello() {
    let _ = al_lienzo::tomar_mostrar();
    let mut e = Escena::nueva();
    // La vista, llena.
    for i in 0..6 {
        for j in 0..4 {
            e.anadir(rect(i as f32 * 300.0, j as f32 * 300.0, 290.0, 290.0));
        }
    }
    let vista = (0.0, 0.0, 1800.0, 1200.0);
    let en = al_lienzo::EnHoja {
        sesion: 77_001,
        zona: 0,
        rehacer: false,
        ultima: true,
        tam: (800.0, 600.0),
        junto_a: None,
    };
    al_lienzo::meter_de_hoja_en(&mut e, &un_rectangulo(800.0, 600.0), &en, Some(vista)).unwrap();
    let caja = al_lienzo::tomar_mostrar().expect("pide mover la vista");
    for c in e.visibles().take(24).map(Elemento::caja) {
        assert!(!se_pisan(caja, c), "{caja:?} pisa {c:?}");
    }
    let (dx, dy) = al_lienzo::desplazar_para_ver(vista, caja);
    assert!(dx != 0.0 || dy != 0.0);
    let movida = (vista.0 + dx, vista.1 + dy, vista.2 + dx, vista.3 + dy);
    assert!(
        caja.0 >= movida.0 - 0.5
            && caja.2 <= movida.2 + 0.5
            && caja.1 >= movida.1 - 0.5
            && caja.3 <= movida.3 + 0.5,
        "{caja:?} en {movida:?}"
    );
}

#[test]
fn la_imagen_del_lienzo_es_la_elegida_o_la_ultima() {
    let mut e = Escena::nueva();
    e.anadir(crate::imagenes_lienzo::elemento_imagen(
        11, 0.0, 0.0, 100.0, 80.0,
    ));
    e.anadir(rect(500.0, 0.0, 50.0, 50.0));
    e.anadir(crate::imagenes_lienzo::elemento_imagen(
        12, 200.0, 0.0, 120.0, 90.0,
    ));
    assert_eq!(
        al_lienzo::imagen_de_escena(&e, &[]).map(|x| x.0),
        Some(12),
        "la ultima pegada"
    );
    let primera = e
        .visibles()
        .find(|x| matches!(x.figura, pixpin_motor2d::Figura::Imagen { id_objeto: 11 }))
        .unwrap()
        .id;
    assert_eq!(
        al_lienzo::imagen_de_escena(&e, &[primera]).map(|x| x.0),
        Some(11),
        "la elegida"
    );
    assert!(al_lienzo::imagen_de_escena(&Escena::nueva(), &[]).is_none());
}

#[test]
fn calca_esto_con_la_imagen_en_el_lienzo_la_toma_como_entrada() {
    al_lienzo::vaciar_para_pruebas();
    let editor = al_lienzo::EditorAbierto::nuevo(9);
    let parar = Arc::new(AtomicBool::new(false));
    let hecho = Arc::new(AtomicBool::new(false));
    // El lienzo abierto, en su hilo, con una imagen pegada.
    let atiende = {
        let hecho = hecho.clone();
        std::thread::spawn(move || {
            let mut e = Escena::nueva();
            let mut imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(16_384);
            let img = pixpin_codec::ImagenRgba {
                ancho: 40,
                alto: 30,
                pixeles: vec![200; 40 * 30 * 4],
            };
            imagenes.guardar_con_id(33, img);
            e.anadir(crate::imagenes_lienzo::elemento_imagen(
                33, 10.0, 20.0, 400.0, 300.0,
            ));
            let hasta = Instant::now() + Duration::from_secs(10);
            while !hecho.load(std::sync::atomic::Ordering::SeqCst) && Instant::now() < hasta {
                al_lienzo::atender_en_editor_con(
                    &mut e,
                    (0.0, 0.0, 1600.0, 900.0),
                    &[],
                    &mut imagenes,
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        })
    };
    let (tx, _rx) = channel();
    let mut ej = EjecutorDelChat::nuevo(tx, "pruebas-fuente".into(), Vec::new(), parar);
    let ruta = ej.imagen_del_lienzo();
    hecho.store(true, std::sync::atomic::Ordering::SeqCst);
    atiende.join().unwrap();
    drop(editor);
    let ruta = ruta.expect("la imagen del lienzo");
    let img = pixpin_codec::imagen::cargar(&ruta).unwrap();
    assert_eq!((img.ancho, img.alto), (40, 30));
    let _ = std::fs::remove_file(&ruta);
}

#[test]
fn que_pedidos_toman_la_imagen_del_lienzo() {
    let si = super::turno::pide_imagen_del_lienzo;
    assert!(si("calca esto"));
    assert!(si(
        "dibújalo usando las herramientas correctas, no solo lápiz"
    ));
    assert!(si("pásalo al lienzo"));
    assert!(!si("¿qué hora es?"));
    assert!(!si("recorta la foto"));
}

/// Una «foto» de paisaje con grano.
fn foto(w: u32, h: u32) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    let mut s: u32 = 5;
    for y in 0..h {
        for x in 0..w {
            s = s.wrapping_mul(1103515245).wrapping_add(12345);
            let r = ((s >> 16) % 40) as f32 - 20.0;
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let c = if fy > 0.6 + 0.1 * (fx * 9.0).sin() {
                [70.0 + 40.0 * fy, 130.0, 50.0]
            } else {
                [120.0 + 60.0 * fy, 170.0, 230.0]
            };
            let i = ((y * w + x) * 4) as usize;
            for k in 0..3 {
                px[i + k] = (c[k] + r).clamp(0.0, 255.0) as u8;
            }
            px[i + 3] = 255;
        }
    }
    px
}

#[test]
fn una_lamina_va_por_figuras_exactas_y_una_foto_no() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    if !pixpin_ocr::disponible() {
        return;
    }
    let p = super::hoja_a_lienzo::banco_diagramas::lamina_tales();
    assert!(
        p.palabras.len() >= 4,
        "el OCR leyo {} palabras",
        p.palabras.len()
    );
    assert!(super::hoja_a_lienzo::es_lamina(&p), "la lamina es lamina");
    let foto = super::hoja_a_lienzo::Pagina {
        rgba: foto(500, 360),
        ancho: 500,
        alto: 360,
        palabras: Vec::new(),
        ms_leer: 0,
    };
    assert!(
        !super::hoja_a_lienzo::es_lamina(&foto),
        "una foto no es lamina"
    );
    // Y por la hoja, sin modelo, la figura sale con figuras exactas.
    let mut d = super::hoja_a_lienzo::pruebas::EnEscena::nueva();
    let t = Instant::now();
    let inf = super::hoja_a_lienzo::pasar(
        &p,
        None,
        &pixpin_ia::hoja::Opciones::default(),
        false,
        4321,
        &AtomicBool::new(false),
        &mut d,
        &mut |_, _| {},
    )
    .unwrap();
    eprintln!(
        "lamina de Tales: primera zona {:?} ms, total {} ms, {} zonas, {} rehechas, pasos {}",
        inf.ms_primera,
        t.elapsed().as_millis(),
        inf.hechos.len(),
        inf.rehechas,
        d.escena.pasos_cerrados()
    );
    let notas: Vec<(pixpin_ia::hoja::Tipo, String)> = inf
        .hechos
        .iter()
        .map(|h| (h.tipo, h.nota.clone()))
        .collect();
    assert!(
        inf.hechos
            .iter()
            .any(|h| h.tipo == pixpin_ia::hoja::Tipo::Diagrama),
        "{notas:?}"
    );
    let arcos = d
        .escena
        .visibles()
        .filter(|e| {
            matches!(
                e.figura,
                pixpin_motor2d::Figura::Arco { .. } | pixpin_motor2d::Figura::Elipse
            )
        })
        .count();
    assert!(arcos >= 1, "la semicircunferencia como arco: {notas:?}");
    // Un solo paso de deshacer para toda la hoja.
    assert!(d.escena.cuantos_visibles() > 0);
    assert!(d.escena.deshacer());
    assert_eq!(
        d.escena.cuantos_visibles(),
        0,
        "un solo Ctrl+Z quita la lamina entera"
    );
}
