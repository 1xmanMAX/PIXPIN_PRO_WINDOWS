//! Mandar fotos y texto a un lienzo por su nombre: donde se apilan, el paso de
//! deshacer, el fichero de un lienzo cerrado y la entrega en vivo.

use super::*;
use pixpin_motor2d::geometria_exacta::recta;

fn foto(w: u32, h: u32) -> ImagenRgba {
    ImagenRgba {
        ancho: w,
        alto: h,
        pixeles: vec![200; (w * h * 4) as usize],
    }
}

fn p(x: f32, y: f32) -> Punto2 {
    Punto2::nuevo(x, y)
}

fn solapan(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    a.0 < b.2 && a.2 > b.0 && a.1 < b.3 && a.3 > b.1
}

#[test]
fn se_apilan_en_una_columna_a_la_derecha_sin_pisar_nada() {
    let mut escena = Escena::nueva();
    let raya = escena.anadir(recta(p(0.0, 0.0), p(300.0, 200.0)));
    let habia = escena.buscar(raya).unwrap().caja();
    let mut imagenes = ImagenesLienzo::nuevo(4096);
    let envio = Envio {
        texto: None,
        imagenes: vec![foto(100, 50)],
    };
    let a = apilar(&mut escena, &envio, &mut imagenes, 1);
    let ca = escena.buscar(a[0]).unwrap().caja();
    assert_eq!(
        (ca.0, ca.1),
        (habia.2 + AIRE_COLUMNA, habia.1),
        "a la derecha de lo que habia, arriba"
    );
    // La segunda, debajo de la primera.
    let b = apilar(&mut escena, &envio, &mut imagenes, 2);
    let cb = escena.buscar(b[0]).unwrap().caja();
    assert_eq!(cb.0, ca.0);
    assert_eq!(cb.1, ca.3 + HUECO);
    // Algo puesto en la columna: la siguiente baja hasta dejarlo libre.
    let estorbo = escena.anadir(recta(
        p(cb.0 - 10.0, cb.3 + 10.0),
        p(cb.0 + 50.0, cb.3 + 300.0),
    ));
    let c = apilar(&mut escena, &envio, &mut imagenes, 3);
    let cc = escena.buscar(c[0]).unwrap().caja();
    assert!(
        !solapan(cc, escena.buscar(estorbo).unwrap().caja()),
        "no pisa lo que hay"
    );
    assert!(cc.1 > cb.3);
    // Una foto ancha entra a lo ancho de la columna, en proporcion.
    assert_eq!(tamano_en_la_pila(960, 480), (ANCHO_COLUMNA, 240.0));
    assert_eq!(
        tamano_en_la_pila(100, 50),
        (100.0, 50.0),
        "una pequena no se agranda"
    );
}

#[test]
fn fotos_y_texto_entran_juntos_en_un_solo_paso_de_deshacer() {
    let mut escena = Escena::nueva();
    let mut imagenes = ImagenesLienzo::nuevo(4096);
    let envio = Envio {
        texto: Some("la puerta del fondo".into()),
        imagenes: vec![foto(40, 40), foto(60, 30)],
    };
    let ids = apilar(&mut escena, &envio, &mut imagenes, 7);
    assert_eq!(ids.len(), 3);
    // Todo marcado como de la pila, el texto el ultimo y debajo.
    for id in &ids {
        assert!(
            escena
                .buscar(*id)
                .unwrap()
                .extras
                .id_de_fichero
                .as_deref()
                .unwrap()
                .starts_with(PREFIJO)
        );
    }
    let texto = escena.buscar(ids[2]).unwrap();
    assert!(matches!(texto.figura, pixpin_motor2d::Figura::Texto { .. }));
    assert!(texto.caja().1 >= escena.buscar(ids[1]).unwrap().caja().3);
    assert!(escena.deshacer());
    assert_eq!(escena.cuantos_visibles(), 0, "un Ctrl+Z lo quita todo");
    // Caso negativo: nada que mandar no deja nada.
    assert!(Envio::default().vacio());
    assert!(apilar(&mut escena, &Envio::default(), &mut imagenes, 8).is_empty());
}

fn raiz_de_prueba(nombre: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!("pixpin-pila-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&r);
    std::fs::create_dir_all(&r).unwrap();
    r
}

fn cosas_en_disco(raiz: &Path, proyecto: &str, dibujo: &str) -> usize {
    let t =
        std::fs::read_to_string(pixpin_proyecto::almacen::lienzo(raiz, proyecto, dibujo)).unwrap();
    pixpin_motor2d::excalidraw::a_escena(&pixpin_motor2d::excalidraw::leer(&t).unwrap())
        .cuantos_visibles()
}

#[test]
fn cerrado_va_a_su_fichero_y_abierto_entra_en_vivo() {
    let raiz = raiz_de_prueba("vivo");
    let m = crate::ventana_chat::escribir_lienzo(&raiz, "p1", "PC", 1, Some("Plano de la casa"))
        .unwrap();
    let dib = m.referencia.clone().unwrap();
    // Por su nombre, sin tildes ni mayusculas y con un trozo.
    let (pr, r, n) = buscar(&raiz, Some("p1"), "PLANO de la casa").expect("lo encuentra");
    assert_eq!(
        (pr.as_str(), r.as_str(), n.as_str()),
        ("p1", dib.as_str(), "Plano de la casa")
    );
    assert!(
        buscar(&raiz, Some("p1"), "fachada").is_none(),
        "caso negativo: otro nombre"
    );
    // Cerrado: a su fichero, con la foto en imagenes/.
    let envio = Envio {
        texto: Some("hola".into()),
        imagenes: vec![foto(20, 20)],
    };
    assert_eq!(
        mandar(&raiz, "p1", &dib, envio.clone(), 5).unwrap(),
        Donde::EnDisco(2)
    );
    assert_eq!(cosas_en_disco(&raiz, "p1", &dib), 2);
    let fotos = std::fs::read_dir(pixpin_proyecto::almacen::carpeta(&raiz, "p1").join("imagenes"))
        .unwrap()
        .count();
    assert_eq!(fotos, 1);
    // Abierto: espera al editor, que lo recoge en su bucle.
    let apunte = abierto(&raiz, "p1", &dib);
    assert_eq!(
        mandar(&raiz, "p1", &dib, envio.clone(), 6).unwrap(),
        Donde::EnVivo
    );
    let mut escena = Escena::nueva();
    let mut imagenes = ImagenesLienzo::nuevo(4096);
    let ids = atender_hoja("p1", &dib, 0, &mut escena, &mut imagenes);
    assert_eq!(ids.len(), 2);
    assert!(
        atender_hoja("p1", &dib, 0, &mut escena, &mut imagenes).is_empty(),
        "una vez"
    );
    // Lo que el editor no llego a recoger va al fichero al cerrarse.
    mandar(&raiz, "p1", &dib, envio, 7).unwrap();
    assert_eq!(cosas_en_disco(&raiz, "p1", &dib), 2, "aun no: esta abierto");
    drop(apunte);
    assert_eq!(cosas_en_disco(&raiz, "p1", &dib), 4);
    let _ = std::fs::remove_dir_all(&raiz);
}
