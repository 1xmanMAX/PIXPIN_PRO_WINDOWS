//! Las imagenes del Word en modo texto: donde caen, que se pulsa y como se
//! ven pintadas de verdad.

use super::super::pruebas::estado_de_prueba;
use super::super::*;
use super::*;
use pixpin_docs::documento::{Bloque, MARCA_IMAGEN, Trozo};
use pixpin_render::lectura::Medida;

/// 8 unidades por letra y renglones de su letra (como las pruebas de
/// `maqueta_movil`).
fn mide(texto: &str, tam: f32, ancho: f32, _t: &[Tramo], l: &Letra) -> Medida {
    let letras = texto.chars().count() as f32 * 8.0;
    let renglones = (letras / ancho.max(1.0)).ceil().max(1.0);
    Medida {
        ancho: letras.min(ancho),
        alto: renglones * tam * l.interlineado,
        minimo: 8.0,
        renglones: Vec::new(),
    }
}

fn p(t: &str) -> Bloque {
    Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano(t)])
}

/// Una PNG de verdad de `w` x `h`, de un color.
fn png(w: u32, h: u32) -> Vec<u8> {
    pixpin_codec::imagen::codificar_png(&ImagenRgba {
        ancho: w,
        alto: h,
        pixeles: [30u8, 120, 200, 255].repeat((w * h) as usize),
    })
    .unwrap()
}

fn imagen(datos: Vec<u8>) -> Imagen {
    Imagen {
        mime: "image/png".into(),
        datos,
    }
}

/// Texto, una imagen pequena, texto, una rota, una enorme y texto.
fn documento() -> Documento {
    Documento {
        bloques: vec![
            p("antes"),
            Bloque::nota(MARCA_IMAGEN),
            p("entre"),
            Bloque::nota(MARCA_IMAGEN),
            Bloque::nota(MARCA_IMAGEN),
            p("despues"),
        ],
        imagenes: vec![
            imagen(png(120, 60)),
            imagen(b"esto no es una imagen".to_vec()),
            imagen(png(3000, 1000)),
        ],
        ..Documento::default()
    }
}

fn colocados(doc: &Documento) -> Vec<Colocado> {
    let ajustes = lectura::Ajustes::default();
    colocar(doc, &ajustes, 600.0, Hoja::Word, &mide).0
}

#[test]
fn cada_imagen_va_en_su_sitio_a_su_tamano_y_sin_pasar_de_la_columna() {
    let doc = documento();
    let c = colocados(&doc);
    let con_imagen: Vec<&Colocado> = c.iter().filter(|c| c.imagen.is_some()).collect();
    assert_eq!(con_imagen.len(), 2, "la rota no lleva imagen");
    // La pequena, a su tamano, entre «antes» y «entre».
    let pequena = con_imagen[0];
    assert_eq!(pequena.imagen, Some(0));
    assert_eq!((pequena.ancho, pequena.alto), (120.0, 60.0));
    let y_de = |t: &str| c.iter().find(|x| x.texto == t).unwrap().y;
    assert!(y_de("antes") < pequena.y && pequena.y + pequena.alto <= y_de("entre"));
    // La enorme, al ancho de la columna y con su proporcion; su numero
    // sigue contando la rota.
    let enorme = con_imagen[1];
    assert_eq!(enorme.imagen, Some(2));
    assert!(enorme.ancho <= 600.0 + 0.01);
    assert!((enorme.alto - enorme.ancho / 3.0).abs() < 0.01);
    assert!(enorme.y + enorme.alto <= y_de("despues"));
}

#[test]
fn una_imagen_rota_deja_su_aviso_y_no_se_pulsa() {
    let doc = documento();
    let c = colocados(&doc);
    let rota = c
        .iter()
        .find(|c| c.recuadro && c.imagen.is_none())
        .expect("el hueco de la rota");
    assert!(rota.texto.contains("no se puede mostrar"), "{}", rota.texto);
    assert_eq!(imagen_en(&c, rota.sangria + 2.0, rota.y + 2.0), None);
}

#[test]
fn el_golpe_cae_en_la_imagen_y_no_fuera() {
    let doc = documento();
    let c = colocados(&doc);
    let k = c.iter().find(|c| c.imagen == Some(0)).unwrap();
    assert_eq!(imagen_en(&c, k.sangria + 1.0, k.y + 1.0), Some(0));
    assert_eq!(
        imagen_en(&c, k.sangria + k.ancho - 1.0, k.y + k.alto - 1.0),
        Some(0)
    );
    // Casos negativos: a su derecha (la columna sigue, la imagen no),
    // encima, en el texto y debajo de todo.
    assert_eq!(imagen_en(&c, k.sangria + k.ancho + 5.0, k.y + 5.0), None);
    assert_eq!(imagen_en(&c, k.sangria + 5.0, k.y - 1.0), None);
    let antes = c.iter().find(|x| x.texto == "antes").unwrap();
    assert_eq!(imagen_en(&c, antes.sangria + 2.0, antes.y + 2.0), None);
    assert_eq!(imagen_en(&c, 10.0, 1.0e6), None);
}

#[test]
fn el_hueco_de_la_imagen_no_depende_de_descodificarla() {
    // El hueco sale de la cabecera: medir dos veces da lo mismo y pintar
    // la imagen dentro no puede mover la tinta. Sin las imagenes (solo sus
    // marcas), el documento es mas bajo: el hueco es el de la imagen.
    let doc = documento();
    let ajustes = lectura::Ajustes::default();
    let (_, alto) = colocar(&doc, &ajustes, 600.0, Hoja::Word, &mide);
    let (_, otra_vez) = colocar(&doc, &ajustes, 600.0, Hoja::Word, &mide);
    assert_eq!(alto, otra_vez);
    let mut sin = doc.clone();
    sin.imagenes.clear();
    let (_, alto_sin) = colocar(&sin, &ajustes, 600.0, Hoja::Word, &mide);
    assert!(alto > alto_sin, "las imagenes ocupan su alto");
}

#[test]
fn una_imagen_en_una_celda_va_dentro_y_las_de_despues_no_se_corren() {
    use pixpin_docs::tabla::{Celda, FilaDeTabla};
    // Una fila con «texto» | imagen 0, y despues una imagen suelta (la 1).
    let mut fila = Bloque::nuevo(Clase::Fila, vec![Trozo::llano("x")]);
    fila.fila = Some(FilaDeTabla {
        tabla: 0,
        rejilla: Vec::new(),
        pagina: 0,
        celdas: vec![
            Celda::llana(vec![Trozo::llano("texto")]),
            Celda::llana(vec![Trozo::llano(MARCA_IMAGEN)]),
        ],
    });
    let doc = Documento {
        bloques: vec![fila, p("entre"), Bloque::nota(MARCA_IMAGEN)],
        imagenes: vec![imagen(png(900, 300)), imagen(png(50, 40))],
        ..Documento::default()
    };
    let c = colocados(&doc);
    let en_celda = c
        .iter()
        .find(|c| c.imagen == Some(0))
        .expect("la de la celda");
    assert_eq!(en_celda.clase, Clase::Fila);
    let k = en_celda.caja.expect("abre su celda: lleva la caja");
    // Dentro de su celda, encogida a ella y con su proporcion.
    assert!(en_celda.sangria >= k.x && en_celda.sangria + en_celda.ancho <= k.x + k.ancho);
    assert!(en_celda.y >= k.y && en_celda.y + en_celda.alto <= k.y + k.alto);
    assert!(en_celda.ancho < 900.0);
    assert!((en_celda.alto - en_celda.ancho / 3.0).abs() < 0.01);
    // La suelta es la segunda, no la primera otra vez.
    let suelta = c
        .iter()
        .find(|c| c.clase == Clase::Nota && c.recuadro)
        .unwrap();
    assert_eq!(suelta.imagen, Some(1));
    assert_eq!((suelta.ancho, suelta.alto), (50.0, 40.0));
    // Caso negativo: una celda con la marca entre texto no es una imagen
    // suelta (se queda como texto), pero sigue contando.
    let mut mezcla = doc.clone();
    if let Some(f) = mezcla.bloques[0].fila.as_mut() {
        f.celdas[1] = Celda::llana(vec![Trozo::llano("antes "), Trozo::llano(MARCA_IMAGEN)]);
    }
    let c = colocados(&mezcla);
    assert!(c.iter().all(|c| c.imagen != Some(0)));
    let suelta = c
        .iter()
        .find(|c| c.clase == Clase::Nota && c.recuadro)
        .unwrap();
    assert_eq!(suelta.imagen, Some(1));
}

#[test]
fn la_reduccion_respeta_la_proporcion_y_nunca_agranda() {
    assert_eq!(medida_reducida(4000, 2000, 2048), (2048, 1024));
    assert_eq!(medida_reducida(1000, 3000, 300), (100, 300));
    // Casos negativos: lo que ya cabe no se toca, y nada queda de cero.
    assert_eq!(medida_reducida(120, 60, 2048), (120, 60));
    assert_eq!(medida_reducida(5000, 1, 100), (100, 1));
}

#[test]
fn el_fichero_de_una_imagen_es_siempre_el_mismo_y_con_su_tipo() {
    let a = imagen(png(4, 4));
    let b = imagen(png(5, 4));
    assert_eq!(nombre_de_archivo(&a), nombre_de_archivo(&a.clone()));
    assert_ne!(nombre_de_archivo(&a), nombre_de_archivo(&b));
    assert!(nombre_de_archivo(&a).ends_with(".png"));
    let jpg = Imagen {
        mime: "image/jpeg".into(),
        datos: vec![1, 2, 3],
    };
    assert!(nombre_de_archivo(&jpg).ends_with(".jpg"));
}

#[test]
fn el_hilo_descodifica_y_reduce_y_lo_roto_no_se_reintenta() {
    let carpeta = std::env::temp_dir().join("pixpin-visor-imagenes-hilo");
    let doc = documento();
    let mut im = Imagenes::nueva(carpeta.clone());
    // La enorme se pide reducida a 300 de lado largo.
    let t = Trabajo {
        n: 2,
        datos: doc.imagenes[2].datos.clone(),
        archivo: im.archivo_de(&doc.imagenes[2]),
        lado: 300,
    };
    let i = descodificar(&t).expect("se lee");
    assert_eq!((i.ancho, i.alto), (300, 100));
    assert!(t.archivo.is_file(), "queda en la cache para el pin");
    // Caso negativo: lo que no es una imagen no se lee.
    let rota = Trabajo {
        n: 1,
        datos: doc.imagenes[1].datos.clone(),
        archivo: carpeta.join("rota.png"),
        lado: 300,
    };
    assert!(descodificar(&rota).is_none());
    im.rotas.insert(1);
    assert!(matches!(im.ver(1, &doc.imagenes[1], 300), Vista::Rota));
    assert!(im.pedidas.is_empty(), "no se vuelve a pedir");
}

fn con_documento() -> (Estado, Marco) {
    let mut e = estado_de_prueba();
    e.doc = documento();
    e.colocados = colocados(&e.doc);
    e.columna = 600.0;
    let m = Marco {
        ancho: 1200.0,
        alto: 800.0,
        e: 1.0,
        escala_por_cien: 100,
        area: pixpin_geom::Rect {
            x: 0,
            y: 0,
            ancho: 1200,
            alto: 800,
        },
    };
    (e, m)
}

/// El raton sobre la imagen `n` (con la vista en el origen y a escala 1,
/// la pantalla y el documento coinciden).
fn sobre(e: &mut Estado, n: usize) {
    let k = e.colocados.iter().find(|c| c.imagen == Some(n)).unwrap();
    e.raton = (k.sangria + 10.0, k.y + 10.0);
}

fn clic(e: &mut Estado, m: Marco, arrastrar: f32) {
    let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
    let raiz = std::env::temp_dir().join("pixpin-visor-imagenes-clic");
    let ubicacion = Ubicacion::Portable { raiz: raiz.clone() };
    let ruta = raiz.join("doc.docx");
    pulsar(e, &textos, &ruta, &ubicacion, m);
    if arrastrar > 0.0 {
        e.raton.1 += arrastrar;
        mover_raton(e, m);
    }
    soltar(e, m);
}

#[test]
fn un_clic_en_una_imagen_la_saca_como_pin_con_la_mano_encima() {
    let (mut e, m) = con_documento();
    sobre(&mut e, 0);
    assert_eq!(
        cursor_leyendo(&mut e, m),
        Some(pixpin_shell::overlay::FormaCursorWin::Giro),
        "la mano"
    );
    clic(&mut e, m, 0.0);
    assert_eq!(e.pinear, Some(0));
    // Y no saca la pastilla, que es lo que hace un clic en el texto.
    assert_eq!(e.pastilla_hasta, 0);
}

#[test]
fn arrastrar_desde_una_imagen_mueve_y_no_saca_pin() {
    let (mut e, m) = con_documento();
    sobre(&mut e, 0);
    clic(&mut e, m, 40.0);
    assert_eq!(e.pinear, None);
}

#[test]
fn anotando_el_clic_en_una_imagen_no_saca_pin() {
    let (mut e, m) = con_documento();
    e.anotando = true;
    sobre(&mut e, 0);
    assert_eq!(cursor_leyendo(&mut e, m), None, "el cursor es de la tinta");
    clic(&mut e, m, 0.0);
    assert_eq!(e.pinear, None);
}

#[test]
fn fuera_de_una_imagen_o_si_no_se_dejo_leer_no_hay_pin_ni_mano() {
    let (mut e, m) = con_documento();
    let antes = e.colocados.iter().find(|c| c.texto == "antes").unwrap();
    e.raton = (antes.sangria + 2.0, antes.y + 2.0);
    assert_eq!(
        cursor_leyendo(&mut e, m),
        Some(pixpin_shell::overlay::FormaCursorWin::Flecha)
    );
    clic(&mut e, m, 0.0);
    assert_eq!(e.pinear, None);
    // Una que el hilo no pudo leer tampoco sale.
    e.imagenes.rotas.insert(0);
    sobre(&mut e, 0);
    clic(&mut e, m, 0.0);
    assert_eq!(e.pinear, None);
}

/// **El Word en modo texto con sus imagenes, pintado de verdad**, en PNG
/// para mirarlo. Necesita GPU y un Word con imagenes (una COPIA):
/// `PIXPIN_DOCX=<copia.docx> PIXPIN_MUESTRAS=<carpeta> cargo test -p pixpin
/// --bin pixpinmax muestra_word_con_imagenes -- --ignored --nocapture`.
#[test]
#[ignore = "necesita GPU y un docx; genera PNG para mirarlos"]
fn muestra_word_con_imagenes() {
    use pixpin_render::MotorRender;
    use pixpin_render::fuera_de_pantalla::FueraDePantalla;

    let docx = std::path::PathBuf::from(std::env::var_os("PIXPIN_DOCX").expect("PIXPIN_DOCX"));
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let (w, h) = (1000u32, 1300u32);
    let m = Marco {
        ancho: w as f32,
        alto: h as f32,
        e: 1.0,
        escala_por_cien: 100,
        area: pixpin_geom::Rect {
            x: 0,
            y: 0,
            ancho: w,
            alto: h,
        },
    };
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), w, h).expect("superficie");
    let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
    let mut e = estado_de_prueba();
    e.doc = pixpin_docs::abrir(&docx).expect("abrir el docx");
    e.indice = indice::de(&e.doc);
    e.imagenes = Imagenes::nueva(carpeta.join("imagenes-doc"));
    e.alto_doc = 0.0;
    println!("{} imagenes", e.doc.imagenes.len());

    let foto = |e: &mut Estado, nombre: &str| {
        // Un fotograma pide las visibles; se espera a que lleguen y se
        // pinta otra vez, como hace el bucle.
        for _ in 0..200 {
            motor
                .dibujar(&fuera.destino, |p| {
                    medir(e, p, m);
                    acotar(e, m);
                    pintar(e, p, m, &textos);
                })
                .expect("pintar");
            fuera.esperar_gpu().expect("esperar");
            if e.imagenes.pedidas.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
            e.imagenes.recibir(&motor);
        }
        let (ancho, alto, pixeles) = fuera.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&ImagenRgba {
            ancho,
            alto,
            pixeles,
        })
        .expect("png");
        let ruta = carpeta.join(format!("word-texto-{nombre}.png"));
        std::fs::write(&ruta, png).expect("guardar");
        println!(
            "{nombre}: {} ({} kB en la cache)",
            ruta.display(),
            e.imagenes.cache.bytes() / 1024
        );
    };

    foto(&mut e, "principio");
    // Las primeras imagenes, con algo de texto encima.
    let ys: Vec<f32> = e
        .colocados
        .iter()
        .filter(|c| c.imagen.is_some())
        .map(|c| c.y)
        .take(3)
        .collect();
    for (i, y) in ys.into_iter().enumerate() {
        e.y = (y - 150.0).max(0.0);
        foto(&mut e, &format!("imagen-{}", i + 1));
    }
}
