//! Las pruebas de la pagina web anotada: **cada cosa atada a su bloque**,
//! los marcadores saltando a su zona, la caida a la fraccion cuando nunca
//! se midio y los documentos sin anotar saliendo limpios.
//!
//! `muestras_para_el_navegador` (ignorada) deja paginas de verdad para
//! mirarlas en un navegador: ver su comentario.

use super::*;
use pixpin_docs::documento::{Bloque, Clase, Trozo};
use pixpin_motor2d::vector::Punto2;
use pixpin_motor2d::{Elemento, Figura};

fn trazo(x: f32, y: f32) -> Elemento {
    Elemento {
        figura: Figura::Lapiz {
            puntos: (0..12)
                .map(|i| Punto2::nuevo(x + i as f32 * 5.0, y + (i % 3) as f32 * 2.0))
                .collect(),
            presiones: Vec::new(),
            opciones: Some(Default::default()),
        },
        grosor: 2.0,
        trazo: ColorRgba::opaco(1.0, 0.53, 0.53),
        ..Elemento::default()
    }
}

fn doc() -> Documento {
    Documento {
        titulo: "Memoria".into(),
        autor: String::new(),
        bloques: vec![
            Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano("La zapata corrida va a ochenta centimetros.")]),
            Bloque::nuevo(Clase::Titulo(2), vec![Trozo::llano("Estructura")]),
            Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano("Los pilares, de hormigon armado.")]),
        ],
        imagenes: Vec::new(),
    }
}

/// Cuatro entradas: el titulo y los tres bloques.
fn medidas() -> Medidas {
    Medidas {
        tops: vec![34.0, 80.0, 140.0, 200.0],
        alto: 300.0,
    }
}

fn con_marcadores(fracciones: &[(f32, &str)]) -> lectura::Ajustes {
    let mut a = lectura::Ajustes {
        columna: 500,
        ..Default::default()
    };
    for (i, (f, e)) in fracciones.iter().enumerate() {
        a.marcadores = lectura::con_marcador(&a.marcadores, *f, e, i as u64 + 1);
    }
    a
}

fn pagina_de(h: HojaDocumento) -> String {
    pagina(&[h], "Memoria").expect("pagina")
}

/// El `data-i` de la pieza de lo anotado que empieza a esa altura.
fn ancla_de_la_pieza(capa: &str, n: usize) -> i32 {
    let trozo = capa.split("<svg class=\"ppa\" data-i=\"").nth(n + 1).expect("pieza");
    trozo[..trozo.find('"').unwrap()].parse().unwrap()
}

#[test]
fn cada_anotacion_se_ata_al_parrafo_junto_al_que_se_escribio() {
    let mut escena = Escena::nueva();
    // Al lado del subtitulo (empieza en 140) y del primer parrafo (80).
    escena.anadir(trazo(520.0, 150.0));
    escena.anadir(trazo(-200.0, 84.0));
    let h = hoja_de_texto("Memoria", &doc(), &con_marcadores(&[]), &escena, 500.0, Some(&medidas()), &|_| None);
    let mut anclas: Vec<i32> = (0..2).map(|n| ancla_de_la_pieza(&h.capa, n)).collect();
    anclas.sort();
    assert_eq!(anclas, [1, 2], "{}", h.capa);
    // El margen izquierdo son equis de la caja: la columna empieza en el margen.
    let margen = vista::margen_de(500.0).round();
    assert_eq!(h.margen as f32, margen);
    let izquierdas: Vec<f32> = h
        .capa
        .split("left:")
        .skip(1)
        .map(|t| t[..t.find("px").unwrap()].parse().unwrap())
        .collect();
    assert!(izquierdas.iter().any(|x| *x < margen - 150.0 && *x > 0.0), "el del margen: {izquierdas:?}");
    assert!(izquierdas.iter().any(|x| *x > margen + 500.0), "el del otro margen: {izquierdas:?}");
    // Caso negativo: un trazo en otro sitio no se ata a un bloque que no toca.
    assert!(!h.capa.contains("data-i=\"0\"") && !h.capa.contains("data-i=\"3\""));
    assert_eq!(h.tops, [34.0, 80.0, 140.0, 200.0]);
}

#[test]
fn los_trazos_del_mismo_parrafo_van_en_una_sola_pieza() {
    let mut escena = Escena::nueva();
    escena.anadir(trazo(10.0, 205.0));
    escena.anadir(trazo(300.0, 215.0));
    let h = hoja_de_texto("M", &doc(), &con_marcadores(&[]), &escena, 500.0, Some(&medidas()), &|_| None);
    assert_eq!(h.capa.matches("class=\"ppa\"").count(), 1);
    assert_eq!(ancla_de_la_pieza(&h.capa, 0), 3);
}

#[test]
fn el_marcador_lleva_a_su_zona_desde_el_riel() {
    // A la mitad de 300 = 150: el tercer bloque (140).
    let a = con_marcadores(&[(0.5, "⭐"), (0.0, "🔖")]);
    let h = hoja_de_texto("M", &doc(), &a, &Escena::nueva(), 500.0, Some(&medidas()), &|_| None);
    // En el orden del documento: primero el de arriba.
    // Arriba del todo no hay bloque: se queda a su altura (medida), sin fraccion.
    assert!(h.capa.contains("id=\"ppm-d-0\" data-i=\"-1\" data-y=\"0\" style="), "{}", h.capa);
    assert!(h.capa.contains("id=\"ppm-d-1\" data-i=\"2\" data-y=\"150\" style=\"left:"));
    assert!(h.riel.contains("<button data-m=\"ppm-d-1\" title=\"Ir al marcador\">⭐</button>"));
    let html = pagina_de(h);
    // El riel esta dentro de la hoja, que es donde el guion lo busca.
    let hoja = &html[html.find("class=\"hoja\"").unwrap()..html.find("<div id=\"estado\"").unwrap()];
    assert!(hoja.contains("<div class=\"pprail\">"));
}

#[test]
fn sin_medidas_lo_anotado_se_queda_donde_estaba_y_los_marcadores_van_por_fraccion() {
    let mut escena = Escena::nueva();
    escena.anadir(trazo(20.0, 150.0));
    let a = con_marcadores(&[(0.25, "💡")]);
    let h = hoja_de_texto("M", &doc(), &a, &escena, 500.0, None, &|_| None);
    assert_eq!(ancla_de_la_pieza(&h.capa, 0), -1);
    assert!(h.capa.contains("data-i=\"-1\" data-y=\"0\" data-f=\"0.25\""), "{}", h.capa);
    assert!(h.tops.is_empty());
    let html = pagina_de(h);
    assert!(html.contains("data-tops=\"\""));
}

#[test]
fn un_documento_sin_anotar_sale_limpio_con_su_texto_de_verdad() {
    let h = hoja_de_texto("M", &doc(), &lectura::Ajustes::default(), &Escena::nueva(), 680.0, Some(&medidas()), &|_| None);
    assert!(h.capa.is_empty() && h.riel.is_empty());
    let html = pagina_de(h);
    assert!(html.contains("<p data-b>La zapata corrida va a ochenta centimetros.</p>"));
    assert!(html.contains("<h2 data-b>Estructura</h2>"));
    assert!(!html.contains("class=\"ppa\""));
    assert!(!html.contains("class=\"ppm\""));
    assert!(!html.contains("<div class=\"pprail\">"));
    // Y con los mandos para anotarlo en el navegador, imprimirlo y guardarlo.
    for id in ["lapiz", "marcador", "goma", "deshacer", "guardar", "imprimir"] {
        assert!(html.contains(&format!("id=\"{id}\"")), "falta {id}");
    }
    // El papel del lector.
    assert!(html.contains(&format!("data-fondo=\"{}\"", hex(crate::lector::FONDO))));
}

#[test]
fn la_letra_del_lector_viaja_en_la_hoja_de_estilo() {
    let a = lectura::Ajustes {
        tamano: 150,
        tipo: 1,
        grosor: 1,
        ..Default::default()
    };
    let h = hoja_de_texto("M", &doc(), &a, &Escena::nueva(), 600.0, None, &|_| None);
    assert_eq!(h.letra, 24.0);
    assert!(h.estilo.contains("font-family:Consolas"));
    assert!(h.estilo.contains("font-weight:700;line-height"));
    // Imprimir la columna, no la pantalla ampliada, y con el papel oscuro.
    assert!(h.estilo.contains("@media print{.doc-caja{transform:none!important"));
    assert!(h.estilo.contains("print-color-adjust:exact"));
}

#[test]
fn las_medidas_salen_una_por_bloque_y_los_vacios_ocupan_su_hueco() {
    let mut d = doc();
    d.bloques.insert(1, Bloque::nuevo(Clase::Parrafo, vec![Trozo::llano("   ")]));
    let a = lectura::Ajustes::default();
    // Cada renglon, su letra por 1,33; una linea por bloque.
    let mide = |_: &str, tam: f32, _: f32, _: &[pixpin_render::Tramo]| tam * 1.33;
    let (colocados, alto) = crate::visor::colocar(&d, &a, 500.0, &mide);
    let m = medidas_de(&d, &colocados, alto, crate::visor::tamano_base(&a));
    assert_eq!(m.tops.len(), pixpin_docs::documento::bloques_para_anotar(&d));
    assert!(m.tops.windows(2).all(|w| w[0] < w[1]), "{:?}", m.tops);
    // El vacio, entre sus vecinos.
    assert!(m.tops[2] > m.tops[1] && m.tops[2] < m.tops[3]);
    assert_eq!(m.alto, alto);
}

#[test]
fn las_medidas_que_deja_el_lector_sirven_sin_poder_medir_y_solo_con_su_letra() {
    let dir = std::env::temp_dir().join(format!("pixpin-docweb-medidas-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ruta = dir.join("apuntes.md");
    std::fs::write(&ruta, "# Apuntes\n\nUno.\n\nDos.\n").unwrap();
    let a = con_marcadores(&[(0.5, "⭐")]);
    lectura::escribir(&ruta, &a).unwrap();
    // Sin nada que exportar no se deja carpeta al lado del documento.
    guardar_medidas(&ruta, &lectura::Ajustes::default(), 500.0, &medidas(), false).unwrap();
    assert!(!ruta_de_medidas(&ruta).exists());
    guardar_medidas(&ruta, &a, 500.0, &medidas(), false).unwrap();
    let (c, m) = medidas_guardadas(&ruta, &a).expect("guardadas");
    assert_eq!(c, 500.0);
    assert_eq!(m, medidas());
    // Caso negativo: con otra letra no valen.
    let otra = lectura::Ajustes { tamano: 130, ..a.clone() };
    assert!(medidas_guardadas(&ruta, &otra).is_none());
    // Sin tarjeta para medir, la pagina sale con lo guardado.
    let sin_tarjeta = |_: &Documento, _: &lectura::Ajustes, _: f32| -> Result<Medidas> { anyhow::bail!("sin tarjeta") };
    let html = web_de_texto_con(&ruta, &sin_tarjeta).unwrap();
    assert!(html.contains("data-tops=\"34.0,80.0,140.0,200.0\""), "con lo guardado");
    assert!(html.contains("data-i=\"2\" data-y=\"150\""), "el marcador, en su bloque");
    // Y sin nada guardado, por fraccion.
    std::fs::remove_file(ruta_de_medidas(&ruta)).unwrap();
    let html = web_de_texto_con(&ruta, &sin_tarjeta).unwrap();
    assert!(html.contains("data-tops=\"\""));
    assert!(html.contains("data-f=\"0.5\""));
    let _ = std::fs::remove_dir_all(&dir);
}

fn hoja_pdf(pagina: u32, tinta: Escena) -> HojaPdf {
    HojaPdf {
        foto: Some("data:image/jpeg;base64,AAAA".into()),
        plano: None,
        alto: 1980.0,
        tinta,
        pagina,
    }
}

#[test]
fn en_el_pdf_lo_anotado_va_con_su_hoja_y_los_marcadores_saltan_a_su_altura() {
    let mut escena = Escena::nueva();
    // En el margen izquierdo de la segunda hoja.
    escena.anadir(trazo(-300.0, 700.0));
    // La marca de la tercera hoja del PDF a un cuarto de su alto; solo van
    // la primera y la tercera (se eligieron esas en la hoja de compartir).
    let marcas = pixpin_motor2d::marcas::a_texto(&pixpin_motor2d::marcas::con(&[], 0.5, 2.25, "🔥", 1));
    let h = hoja_de_pdf("Planos", &[hoja_pdf(0, Escena::nueva()), hoja_pdf(2, escena)], &marcas);
    let k = (COLUMNA_PDF / vista::ANCHO_HOJA) as f64;
    let alto = 1980.0 * k;
    assert_eq!(h.tops.len(), 2);
    assert!((h.tops[1] - (alto + ENTRE_HOJAS as f64)).abs() < 0.01, "{:?}", h.tops);
    assert_eq!(h.cuerpo.matches("<img data-b").count(), 2);
    assert!(h.cuerpo.contains("alt=\"Hoja 3\""));
    // La tinta, atada a la hoja en la que esta (la segunda de la pagina).
    assert_eq!(ancla_de_la_pieza(&h.capa, 0), 1);
    let y_marca = alto + ENTRE_HOJAS as f64 + 0.25 * alto;
    assert!(h.capa.contains(&format!("data-i=\"1\" data-y=\"{}\"", dw_num(y_marca))), "{}", h.capa);
    assert!(h.riel.contains("<button data-m=\"ppm-pdf-0\""));
    // Caso negativo: una marca de una hoja que no va, no va.
    let solo_primera = hoja_de_pdf("Planos", &[hoja_pdf(0, Escena::nueva())], &marcas);
    assert!(!solo_primera.capa.contains("ppm"));
}

/// El numero como lo escribe la capa: un decimal.
fn dw_num(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r == r.trunc() { format!("{}", r as i64) } else { format!("{r}") }
}

#[test]
fn un_pdf_de_verdad_sale_con_sus_hojas_nitidas_y_lo_anotado() {
    let dir = std::env::temp_dir().join(format!("pixpin-docweb-pdf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pdf = dir.join("memoria.pdf");
    let hojas = super::super::hojas_de_texto(Some("Memoria"), &"Parrafo de prueba. ".repeat(600));
    let bytes = pixpin_pdf::escribir::de_hojas(&hojas[..2], Some(super::super::BLANCO), &|_| None).unwrap();
    std::fs::write(&pdf, bytes).unwrap();
    let mut capa = Escena::nueva();
    capa.anadir(trazo(200.0, 300.0));
    let ruta_capa = crate::lector_tinta::ruta_de_hoja(&pdf, 1);
    std::fs::create_dir_all(ruta_capa.parent().unwrap()).unwrap();
    std::fs::write(&ruta_capa, pixpin_motor2d::excalidraw::escribir(&pixpin_motor2d::excalidraw::con_escena(&pixpin_motor2d::excalidraw::Lienzo::vacio(), &capa))).unwrap();
    let html = web_de_pdf(&pdf, None).unwrap();
    // Un PDF de texto escrito aqui es vectorial: sus hojas van como lineas,
    // sin foto, y el guion que las despliega va una sola vez.
    assert_eq!(html.matches("<svg data-b class=\"plano-hoja\"").count(), 2);
    assert_eq!(html.matches("class=\"plano-datos\"").count(), 2);
    assert_eq!(html.matches("function inflar(").count(), 1);
    assert!(!html.contains("src=\"data:image/jpeg;base64,"), "sin fotos de las hojas");
    assert!(html.contains("<svg class=\"ppa\" data-i=\"1\""));
    // Pedir solo la primera: una hoja y sin la tinta de la segunda.
    let una = web_de_pdf(&pdf, Some(&[0])).unwrap();
    assert_eq!(una.matches("<svg data-b class=\"plano-hoja\"").count(), 1);
    assert!(!una.contains("class=\"ppa\""));
    // Caso negativo: hojas que no hay.
    assert!(web_de_pdf(&pdf, Some(&[9])).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// **Un escaneo sigue yendo como foto**: sus hojas son una imagen y nada
/// mas, y como lineas no habria nada que ver.
#[test]
fn un_pdf_escaneado_sale_con_sus_hojas_en_jpeg_y_sin_el_guion_del_plano() {
    let dir = std::env::temp_dir().join(format!("pixpin-docweb-escaneo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pdf = dir.join("escaneo.pdf");
    let hoja = ImagenRgba {
        ancho: 60,
        alto: 80,
        pixeles: [200u8, 190, 180, 255].repeat(60 * 80),
    };
    std::fs::write(&pdf, pixpin_pdf::union::de_imagenes(&[hoja.clone(), hoja]).unwrap()).unwrap();
    let html = web_de_pdf(&pdf, None).unwrap();
    assert_eq!(html.matches("<img data-b").count(), 2);
    assert!(html.contains("src=\"data:image/jpeg;base64,"));
    assert!(!html.contains("class=\"plano-hoja\""));
    assert!(!html.contains("function inflar("), "el guion solo va si hace falta");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn una_hoja_como_lineas_no_puede_cerrar_su_guion_antes_de_tiempo() {
    let mut h = hoja_pdf(0, Escena::nueva());
    h.foto = None;
    h.plano = Some("{\"capas\":[{\"n\":\"</script><b>\"}]}".into());
    let d = hoja_de_pdf("Plano", &[h], "");
    assert!(!d.cuerpo.contains("</script><b>"));
    assert!(d.cuerpo.contains("<\\/script><b>"));
    assert!(!d.cuerpo.contains("<img"), "con lineas no va la foto");
}

#[test]
fn una_foto_grande_se_encoge_y_una_pequena_va_tal_cual() {
    let pequena = Imagen {
        mime: "image/png".into(),
        datos: vec![1, 2, 3],
    };
    assert_eq!(imagen_para_la_web(&pequena).unwrap(), "data:image/png;base64,AQID");
    // Una foto de 2400 de ancho con ruido (no se comprime sola).
    let mut pixeles = Vec::with_capacity(2400 * 300 * 4);
    let mut semilla = 7u32;
    for _ in 0..2400 * 300 {
        semilla = semilla.wrapping_mul(1_103_515_245).wrapping_add(12345);
        pixeles.extend_from_slice(&[(semilla >> 16) as u8, (semilla >> 8) as u8, semilla as u8, 255]);
    }
    let img = ImagenRgba { ancho: 2400, alto: 300, pixeles };
    let grande = Imagen {
        mime: "image/png".into(),
        datos: pixpin_codec::codificar_png(&img).unwrap(),
    };
    assert!(grande.datos.len() > FOTO_LIGERA);
    let src = imagen_para_la_web(&grande).unwrap();
    assert!(src.starts_with("data:image/jpeg;base64,"), "opaca: JPEG");
    assert!(src.len() < grande.datos.len(), "pesa menos");
    // Caso negativo: una imagen vacia no se pone.
    assert!(imagen_para_la_web(&Imagen { mime: "image/png".into(), datos: Vec::new() }).is_none());
}

/// **Muestras para mirar en un navegador.** Con `PIXPIN_MUESTRAS_DOCWEB`
/// apuntando a una carpeta con documentos (`.docx`, `.epub`, `.md`, `.pdf`),
/// a cada uno se le pone tinta y marcadores junto al documento y se deja su
/// pagina web al lado, `<nombre>.html`. Se ignora sin la variable.
#[test]
#[ignore]
fn muestras_para_el_navegador() {
    let Some(dir) = std::env::var_os("PIXPIN_MUESTRAS_DOCWEB") else {
        return;
    };
    let dir = PathBuf::from(dir);
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let ruta = e.path();
        let nombre = pixpin_docs::nombre(&ruta);
        let es_pdf = crate::lector_pdf::se_abre(&nombre);
        if !es_pdf && pixpin_docs::formato_de(&nombre).is_none_or(|f| f == pixpin_docs::Formato::Html) {
            continue;
        }
        if nombre.ends_with(".html") {
            continue;
        }
        if es_pdf {
            let mut a = lectura::leer(&ruta);
            let mut m = pixpin_motor2d::marcas::con(&[], 0.5, 0.3, "⭐", 1);
            m = pixpin_motor2d::marcas::con(&m, 0.5, 1.6, "🔥", 2);
            a.marcas = pixpin_motor2d::marcas::a_texto(&m);
            lectura::escribir(&ruta, &a).unwrap();
            for (i, (x, y)) in [(300.0, 400.0), (-500.0, 900.0)].iter().enumerate() {
                let mut capa = Escena::nueva();
                capa.anadir(trazo(*x, *y));
                capa.anadir(Elemento {
                    figura: Figura::Resaltador {
                        puntos: vec![Punto2::nuevo(150.0, 260.0), Punto2::nuevo(900.0, 262.0)],
                    },
                    grosor: 30.0,
                    trazo: ColorRgba::opaco(1.0, 0.83, 0.23),
                    ..Elemento::default()
                });
                let r = crate::lector_tinta::ruta_de_hoja(&ruta, i);
                std::fs::create_dir_all(r.parent().unwrap()).unwrap();
                guardar_como_el_lector(&r, capa);
            }
            let html = web_de_pdf(&ruta, None).unwrap();
            std::fs::write(ruta.with_extension("pdf.html"), html).unwrap();
            continue;
        }
        let a = con_marcadores(&[(0.02, "🔖"), (0.4, "⭐"), (0.8, "💡")]);
        lectura::escribir(&ruta, &a).unwrap();
        // Tinta junto a varios parrafos, medidos de verdad.
        let d = pixpin_docs::abrir(&ruta).unwrap();
        let m = medir_ahora(&d, &a, 500.0).unwrap();
        let mut capa = Escena::nueva();
        for (n, t) in m.tops.iter().enumerate().skip(1).step_by(3).take(6) {
            if n % 2 == 0 {
                capa.anadir(trazo(510.0, t + 8.0));
            } else {
                capa.anadir(trazo(-150.0, t + 4.0));
                capa.anadir(Elemento {
                    figura: Figura::Resaltador {
                        puntos: vec![Punto2::nuevo(0.0, t + 10.0), Punto2::nuevo(180.0, t + 10.0)],
                    },
                    grosor: 16.0,
                    trazo: ColorRgba::opaco(1.0, 0.83, 0.23),
                    ..Elemento::default()
                });
            }
        }
        let r = crate::lector_tinta::ruta_de_capa(&ruta);
        std::fs::create_dir_all(r.parent().unwrap()).unwrap();
        guardar_como_el_lector(&r, capa);
        let html = web_de_texto(&ruta).unwrap();
        let ext = pixpin_docs::extension(&nombre);
        std::fs::write(ruta.with_extension(format!("{ext}.html")), html).unwrap();
    }
}

/// La capa escrita como la escribe el lector, con su resaltador translucido.
fn guardar_como_el_lector(ruta: &Path, escena: Escena) {
    let mut c = crate::lector_tinta::Capa::default();
    c.escena = escena;
    c.sucia = true;
    c.guardar(ruta).unwrap();
}
