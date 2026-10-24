//! Pruebas de `anotar_inteligente` con documentos de verdad (un PDF con
//! capa de texto, el mismo escaneado sin ella, una foto, un plano DXF y un
//! Excel) y un modelo de mentira que contesta lo que se le dice, a trozos.

use pixpin_ia::motor::{MotorGuionado, Respuesta};
use pixpin_motor2d::{ColorRgba, Elemento, Figura};

use super::*;

pub(crate) fn carpeta(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pixpin-anotar-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Una hoja A4 en puntos.
const W: f32 = 595.0;
const H: f32 = 842.0;

pub(crate) const RENGLONES: &[&str] = &[
    "Capitulo 2. Diseno de la viga principal",
    "La viga se apoya en dos columnas y soporta una carga",
    "repartida de 12 kN/m en toda su luz de seis metros.",
    "El momento maximo vale M = q L^2 / 8 y aparece en el",
    "centro del vano, donde la flecha tambien es mayor.",
    "Para el calculo se usa el modelo elastico lineal y el",
    "acero tiene un limite de fluencia de 4200 kg/cm2.",
];

fn texto(t: &str, x: f32, y: f32, tam: f32) -> Elemento {
    Elemento {
        figura: Figura::Texto {
            texto: t.into(),
            tam,
            familia: "Segoe UI".into(),
        },
        x,
        y,
        ancho: pixpin_motor2d::exportar::ancho_de_texto(t, tam),
        alto: tam * 1.33,
        trazo: ColorRgba::opaco(0.05, 0.05, 0.05),
        grosor: 1.0,
        rugosidad: 0.0,
        ..Default::default()
    }
}

/// Un PDF de una hoja A4 por el camino del lienzo: con `renglones` escritos
/// (capa de texto de verdad) o, con `foto`, solo esa imagen (un escaneado).
pub(crate) fn pdf(ruta: &Path, renglones: &[&str], foto: Option<&pixpin_codec::ImagenRgba>) {
    let mut escena = pixpin_motor2d::Escena::nueva();
    match foto {
        Some(_) => {
            escena.anadir(Elemento {
                figura: Figura::Imagen { id_objeto: 7 },
                x: 0.0,
                y: 0.0,
                ancho: W,
                alto: H,
                ..Default::default()
            });
        }
        None => {
            for (i, t) in renglones.iter().enumerate() {
                escena.anadir(texto(t, 72.0, 96.0 + i as f32 * 24.0, 12.0));
            }
        }
    }
    let mut hojas = pixpin_motor2d::exportar::hojas(
        &escena,
        pixpin_motor2d::exportar::Alcance::Todo,
        &[],
        None,
    );
    if hojas.is_empty() {
        // Una hoja en blanco: un punto blanco para que haya hoja.
        escena.anadir(Elemento {
            x: 1.0,
            y: 1.0,
            ancho: 1.0,
            alto: 1.0,
            trazo: ColorRgba::opaco(1.0, 1.0, 1.0),
            ..Default::default()
        });
        hojas = pixpin_motor2d::exportar::hojas(
            &escena,
            pixpin_motor2d::exportar::Alcance::Todo,
            &[],
            None,
        );
    }
    hojas.truncate(1);
    hojas[0].caja = (0.0, 0.0, W, H);
    let resolver = |id: u64| {
        (id == 7)
            .then_some(foto)
            .flatten()
            .map(|i| pixpin_pdf::escribir::Pixeles {
                ancho: i.ancho,
                alto: i.alto,
                rgba: i.pixeles.clone(),
            })
    };
    let bytes = pixpin_pdf::escribir::de_hojas_a_medida(
        &hojas,
        &[(W, H)],
        &resolver,
        super::super::hoja_a_lienzo::letra_segoe(),
    )
    .unwrap();
    std::fs::write(ruta, bytes).unwrap();
}

/// Lo que contesta el modelo de mentira: un objeto por linea.
fn guion(lineas: &[Value]) -> MotorGuionado {
    let texto: Vec<String> = lineas.iter().map(Value::to_string).collect();
    MotorGuionado::nuevo(vec![Respuesta::de_texto(texto.join("\n"))])
}

fn tres_anotaciones() -> Vec<Value> {
    vec![
        json!({"tipo":"importante","pag":1,"ancla":"carga repartida de 12 kN/m","l":3,"nota":"Es la carga de diseno: entra en todas las formulas."}),
        json!({"tipo":"explicacion","pag":1,"ancla":"M = q L^2 / 8","l":4,"nota":"M: momento maximo; q: carga por metro; L: luz."}),
        json!({"tipo":"pregunta","pag":1,"ancla":"donde la flecha tambien es mayor","nota":"¿Por que la flecha maxima esta en el centro?"}),
    ]
}

fn del_turno<'a>(escena: &'a pixpin_motor2d::Escena, grupo: &str) -> Vec<&'a Elemento> {
    escena
        .visibles()
        .filter(|e| e.grupos.iter().any(|g| g == grupo))
        .collect()
}

#[test]
fn en_un_pdf_las_notas_se_anclan_al_texto_y_se_quitan_con_un_solo_deshacer() {
    let dir = carpeta("pdf");
    let ruta = dir.join("viga.pdf");
    pdf(&ruta, RENGLONES, None);
    let motor = guion(&tres_anotaciones());
    let parar = AtomicBool::new(false);
    let mut avisos = Vec::new();
    let h = anotar_en(
        &dir,
        &ruta,
        &json!({"pedido": "señalame lo importante", "pagina": 1}),
        &motor,
        &parar,
        &mut |h, primera| {
            avisos.push((h.puestas, primera));
        },
    )
    .unwrap();
    assert_eq!(h.puestas, 3, "{}", h.resumen);
    assert_eq!(
        avisos.first(),
        Some(&(1, true)),
        "la primera avisa en cuanto esta"
    );
    assert_eq!(h.con_ocr, 0, "tiene capa de texto: sin OCR");
    // El modelo vio el texto con sus renglones numerados.
    let pedido = motor.recibido.lock().unwrap()[0].mensajes[1].texto_entero();
    assert!(pedido.contains("L3: repartida de 12 kN/m"), "{pedido}");
    // En la capa que lee el lector, y la primera anotacion encima de su texto.
    let d = crate::lector_pdf_proyecto::DondeVa::de(&dir, &ruta, 0);
    let alto = ANCHO_HOJA * H / W;
    let mut capa = d.leer_capa(&ruta, 0, 0, alto);
    let mias: Vec<Elemento> = del_turno(&capa.escena, &h.grupo)
        .into_iter()
        .cloned()
        .collect();
    assert!(mias.len() >= 3 * 4, "{}", mias.len());
    // «carga repartida de 12 kN/m» empieza al final del segundo renglon y
    // sigue en el tercero: el recuadro arranca en el segundo (y de 96+24 pt).
    let k = ANCHO_HOJA / W;
    let y_renglon = (96.0 + 24.0) * k;
    let recuadro = mias
        .iter()
        .find(|e| matches!(e.figura, Figura::Rectangulo) && e.relleno.is_none())
        .unwrap();
    assert!(
        (recuadro.y - y_renglon).abs() < 20.0 * k,
        "{} vs {y_renglon}",
        recuadro.y
    );
    // Un solo Ctrl+Z quita el turno entero; Ctrl+Y lo devuelve.
    assert!(
        capa.turno.as_ref().is_some_and(|t| t.arriba(&capa.escena)),
        "lo del turno es lo ultimo hecho"
    );
    assert!(capa.deshacer());
    assert!(
        del_turno(&capa.escena, &h.grupo).is_empty(),
        "un Ctrl+Z quita las tres anotaciones"
    );
    assert!(
        capa.turno
            .as_ref()
            .is_some_and(|t| t.deshecho(&capa.escena))
    );
    assert!(capa.rehacer());
    assert_eq!(del_turno(&capa.escena, &h.grupo).len(), mias.len());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn un_escaneado_sin_capa_de_texto_se_lee_por_el_ocr() {
    if !pixpin_ocr::disponible() {
        println!("sin OCR de Windows en este equipo: no se prueba");
        return;
    }
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let dir = carpeta("escaneado");
    // La hoja con texto, pintada, y metida como foto en otro PDF.
    let con_texto = dir.join("original.pdf");
    pdf(&con_texto, RENGLONES, None);
    let img = pixpin_pdf::Documento::abrir(&con_texto)
        .unwrap()
        .renderizar(0, 1700)
        .unwrap();
    let ruta = dir.join("escaneado.pdf");
    pdf(&ruta, &[], Some(&img));
    assert!(
        pixpin_pdf::texto::de_fichero(&ruta)
            .map_or(true, |t| t.iter().all(|p| p.texto.trim().is_empty())),
        "sin capa de texto"
    );
    let motor = guion(&tres_anotaciones());
    let parar = AtomicBool::new(false);
    let h = anotar_en(
        &dir,
        &ruta,
        &json!({"pedido": "lo importante"}),
        &motor,
        &parar,
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(h.con_ocr, 1);
    assert!(
        h.puestas >= 2,
        "el OCR tiene que dar para anclar casi todo: {}",
        h.resumen
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn un_documento_vacio_no_se_le_pregunta_al_modelo() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let dir = carpeta("vacio");
    let ruta = dir.join("blanco.pdf");
    pdf(&ruta, &[], None);
    let motor = guion(&tres_anotaciones());
    let parar = AtomicBool::new(false);
    let e = anotar_en(
        &dir,
        &ruta,
        &json!({"pedido": "resume la pagina"}),
        &motor,
        &parar,
        &mut |_, _| {},
    )
    .unwrap_err();
    assert!(
        e.contains("no hay nada que anotar") || e.contains("no tiene texto"),
        "{e}"
    );
    assert!(
        motor.recibido.lock().unwrap().is_empty(),
        "sin texto no se gasta una llamada"
    );
    // Y si el modelo dice que no hay nada, no se escribe nada.
    let ruta2 = dir.join("viga.pdf");
    pdf(&ruta2, RENGLONES, None);
    let nada = guion(&[json!({"tipo":"nada","nota":"no hay errores"})]);
    let h = anotar_en(
        &dir,
        &ruta2,
        &json!({"pedido": "marca los errores"}),
        &nada,
        &parar,
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!((h.puestas, h.nada.as_deref()), (0, Some("no hay errores")));
    assert!(h.capas.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn en_una_foto_un_plano_y_un_excel_va_a_la_capa_de_cada_uno() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let dir = carpeta("formatos");
    let parar = AtomicBool::new(false);
    // Un plano DXF con sus calles: el ancla es un texto del plano.
    let plano = dir.join("calles.dxf");
    std::fs::write(&plano, dxf_con_calles()).unwrap();
    let motor = guion(&[
        json!({"tipo":"importante","ancla":"JR. ANTISUYO","nota":"Calle principal: aqui va la senal R-1."}),
    ]);
    let h = anotar_en(
        &dir,
        &plano,
        &json!({"pedido": "señala lo importante"}),
        &motor,
        &parar,
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(h.puestas, 1, "{}", h.resumen);
    let rutas = crate::plano_cad::anotar::rutas_de_la_capa(&dir, &plano);
    let capa = crate::lector_tinta::Capa::leer(&rutas.tinta);
    assert!(!del_turno(&capa.escena, &h.grupo).is_empty());
    // Un Excel: el ancla es una celda.
    let xlsx = super::super::documentos_acciones::escribir_excel(
        "precios",
        &[
            vec!["Producto".into(), "Precio".into()],
            vec!["Cemento".into(), "28".into()],
            vec!["Total".into(), "-5".into()],
        ],
        true,
    )
    .unwrap();
    let motor = guion(&[
        json!({"tipo":"error","ancla":"B3","nota":"Un total negativo no puede ser.","corr":"28"}),
    ]);
    let h = anotar_en(
        &dir,
        &xlsx,
        &json!({"pedido": "marca los errores"}),
        &motor,
        &parar,
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(h.puestas, 1, "{}", h.resumen);
    let capa = crate::lector_tinta::Capa::leer(&crate::lector_tinta::ruta_de_hoja(&xlsx, 0));
    let mias: Vec<Elemento> = del_turno(&capa.escena, &h.grupo)
        .into_iter()
        .cloned()
        .collect();
    assert!(!mias.is_empty());
    // La nota, a la derecha de lo usado (columna B), nunca a la izquierda.
    let libro = pixpin_hojas::Libro::abrir(&xlsx, pixpin_hojas::Regional::es_pe()).unwrap();
    let mut hoja = libro.abrir_hoja(0);
    hoja.cargar_todo();
    let rejilla = rejilla_de_la_capa(&libro, &hoja);
    assert!(
        mias.iter().all(|e| e.x >= -1.0),
        "nada a la izquierda de la columna A"
    );
    assert!(
        mias.iter().any(|e| e.x > rejilla.x(2) as f32),
        "la nota, despues de la columna B"
    );
    let _ = std::fs::remove_file(&xlsx);
    // Una foto: el OCR de la foto, y el dibujo junto a ella.
    if pixpin_ocr::disponible() {
        let con_texto = dir.join("hoja.pdf");
        pdf(&con_texto, RENGLONES, None);
        let img = pixpin_pdf::Documento::abrir(&con_texto)
            .unwrap()
            .renderizar(0, 1400)
            .unwrap();
        let foto = dir.join("foto.png");
        std::fs::write(&foto, pixpin_codec::imagen::codificar_png(&img).unwrap()).unwrap();
        let motor = guion(&tres_anotaciones());
        let h = anotar_en(
            &dir,
            &foto,
            &json!({"pedido": "lo importante"}),
            &motor,
            &parar,
            &mut |_, _| {},
        )
        .unwrap();
        assert!(h.puestas >= 2, "{}", h.resumen);
        let mut dibujo = foto.as_os_str().to_owned();
        dibujo.push(".pixpin2d");
        let (escena, _) = crate::pines::escena_para_lienzo(Path::new(&dibujo)).unwrap();
        assert!(!del_turno(&escena, &h.grupo).is_empty());
        // Al abrir la foto en el lienzo, el turno es un solo paso.
        let mut e = escena.clone();
        assert!(e.deshacer());
        assert!(del_turno(&e, &h.grupo).is_empty());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn los_renglones_salen_de_las_letras_con_sus_saltos() {
    // Dos renglones sin salto de linea entre ellos: los separa la altura.
    let mut t = PaginaDeTexto::default();
    let mut poner = |s: &str, y: f32| {
        for (i, c) in s.chars().enumerate() {
            t.texto.push(c);
            let x = 0.1 + i as f32 * 0.01;
            t.cajas
                .push((c != ' ').then_some([x, y, x + 0.009, y + 0.012]));
        }
    };
    poner("hola mundo", 0.1);
    poner("adios", 0.2);
    let r = renglones_de(&t, 1400.0, 1980.0);
    let textos: Vec<String> = r.iter().map(Renglon::texto).collect();
    assert_eq!(textos, vec!["hola mundo".to_string(), "adios".to_string()]);
    assert!((r[0].palabras[0].caja[0] - 140.0).abs() < 0.01);
    assert!(renglones_de(&PaginaDeTexto::default(), 1400.0, 1980.0).is_empty());
}

/// Un plano DXF pequeno con textos de calles y una raya.
fn dxf_con_calles() -> Vec<u8> {
    let texto = |x: f64, y: f64, h: f64, t: &str| {
        format!("0\nTEXT\n8\n0\n10\n{x}\n20\n{y}\n30\n0\n40\n{h}\n1\n{t}\n")
    };
    let mut s = String::from("0\nSECTION\n2\nENTITIES\n");
    s += &texto(100.0, 50.0, 2.5, "JR. ANTISUYO");
    s += &texto(20.0, 120.0, 2.5, "AV. TAHUANTINSUYO");
    s += &texto(150.0, 10.0, 1.0, "nota: ver Jr Antisuyo");
    s += "0\nLINE\n8\n0\n10\n0\n20\n0\n30\n0\n11\n200\n21\n150\n31\n0\n";
    s += "0\nENDSEC\n0\nEOF\n";
    s.into_bytes()
}
