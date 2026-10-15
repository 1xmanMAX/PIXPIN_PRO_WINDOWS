//! Con la ventana entera pero **oculta**: el `RichEdit` de verdad, las
//! fotos pintadas en memoria (`muestra`) para que tengan su sitio, y el
//! raton con mensajes hechos a mano (nada de mover el raton de verdad).

use std::cell::RefCell as Celda;
use std::rc::Rc as Comun;

use super::*;
use crate::integracion::{GrupoDeHojas, HojaElegible, Integracion, RotulosFotos};

const R: &str = "pixpin:files/guardados/pc/p1/notas/1-planta.png";
const VIVA: &str = "pixpin:files/guardados/pc/p1/notas/vivo-K7Q2ABCDEF.png";

fn carpeta() -> PathBuf {
    let d = std::env::temp_dir().join(format!("pixpin-notas-fotos-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Un PNG de `an` x `al` liso.
fn png(nombre: &str, an: u32, al: u32) -> PathBuf {
    png_de(nombre, an, al, [90, 150, 220])
}

fn png_de(nombre: &str, an: u32, al: u32, c: [u8; 3]) -> PathBuf {
    let img = pixpin_codec::ImagenRgba {
        ancho: an,
        alto: al,
        pixeles: [c[0], c[1], c[2], 255].repeat((an * al) as usize),
    };
    let r = carpeta().join(nombre);
    std::fs::write(&r, pixpin_codec::imagen::codificar_png(&img).unwrap()).unwrap();
    r
}

fn rotulos() -> Rotulos {
    Rotulos {
        fotos: RotulosFotos {
            captura: "Captura".into(),
            hoja_borrada: "La hoja ya no está".into(),
            ver_grande: "Ver en grande".into(),
            abrir_hoja: "Abrir la hoja".into(),
            ..Default::default()
        },
        pagina_viva: "Página de un proyecto".into(),
        enlace_hoja: "Enlace a una hoja".into(),
        // El marco pinta estos: vacios, el pintor de muestras se cae.
        nueva: "Nota nueva".into(),
        sufijo: "Nota".into(),
        compartir: "Compartir".into(),
        boton_fila_mas: "+ Fila".into(),
        boton_fila_menos: "- Fila".into(),
        boton_columna_mas: "+ Columna".into(),
        boton_columna_menos: "- Columna".into(),
        ..Default::default()
    }
}

type Resolver = Box<dyn Fn(&str) -> Option<PathBuf>>;
type Adjuntar = Box<dyn FnMut(&Path) -> Option<String>>;

fn abrir_con(
    texto: &str,
    resolver: Resolver,
    adjuntar: Adjuntar,
    integracion: Integracion,
) -> Estado {
    // Pintar en memoria (`muestra`) necesita COM en el hilo, como las muestras;
    // se queda iniciado lo que dure el hilo de la prueba.
    std::mem::forget(pixpin_shell::ComDelHilo::iniciar());
    montar(
        Pedido {
            texto: texto.into(),
            rotulos: rotulos(),
            nombre_de_fichero: None,
            colocacion: None,
            resolver,
            adjuntar,
            compartir: None,
            integracion,
            comentarios: Default::default(),
        },
        Opciones {
            oculto: true,
            tamano: Some((1000, 760)),
            claro: Some(false),
        },
    )
    .expect("la ventana oculta se monta")
}

/// Apunta lo que adjuntar recibio (nombre y si existia) y da la ruta del
/// Markdown: la de la carpeta de notas del proyecto, con la hora 9.
fn adjuntar_que_apunta(recibido: Comun<Celda<Vec<(String, bool)>>>) -> Adjuntar {
    Box::new(move |r: &Path| {
        let nombre = nombre_de(r);
        recibido.borrow_mut().push((nombre.clone(), r.is_file()));
        Some(format!(
            "pixpin:files/guardados/pc/p1/notas/9-{}",
            nombre.replace(' ', "_")
        ))
    })
}

fn pulsar(e: &mut Estado, m: u32, x: i32, y: i32) -> bool {
    let m = msg(e, m, x, y);
    raton(e, &m)
}

fn msg(e: &Estado, m: u32, x: i32, y: i32) -> MSG {
    MSG {
        hwnd: e.edit,
        message: m,
        wParam: WPARAM(0),
        lParam: LPARAM((((y as u16) as isize) << 16) | (x as u16) as isize),
        ..Default::default()
    }
}

#[test]
fn pegar_una_captura_la_copia_junto_a_la_nota_y_escribe_su_renglon() {
    let recibido = Comun::new(Celda::new(Vec::new()));
    let mut e = abrir_con(
        "Visita de obra",
        Box::new(|_: &str| None),
        adjuntar_que_apunta(recibido.clone()),
        Integracion::default(),
    );
    let img = pixpin_codec::ImagenRgba {
        ancho: 4,
        alto: 3,
        pixeles: vec![255; 48],
    };
    assert!(pegar_imagen(&mut e, &img));
    // Se copio un PNG que existia al copiarlo, con el nombre de captura.
    assert_eq!(*recibido.borrow(), vec![("Captura.png".to_string(), true)]);
    assert_eq!(
        markdown(&e),
        "Visita de obra\n![Captura](pixpin:files/guardados/pc/p1/notas/9-Captura.png)"
    );
    desmontar(e);
}

#[test]
fn soltar_fotos_las_mete_donde_se_soltaron_y_lo_que_no_es_foto_no() {
    let recibido = Comun::new(Celda::new(Vec::new()));
    let mut e = abrir_con(
        "antes\ndespues",
        Box::new(|_: &str| None),
        adjuntar_que_apunta(recibido.clone()),
        Integracion::default(),
    );
    let foto = png("alzado.png", 8, 6);
    let pdf = carpeta().join("plano.pdf");
    std::fs::write(&pdf, b"%PDF").unwrap();
    // Soltadas al final de «antes» (letra 5).
    assert_eq!(soltar(&mut e, vec![foto, pdf.clone()], Some(5)), 1);
    assert_eq!(
        markdown(&e),
        "antes\n![alzado](pixpin:files/guardados/pc/p1/notas/9-alzado.png)\ndespues"
    );
    // Caso negativo: solo un PDF no entra ni toca el texto.
    let antes = markdown(&e);
    assert_eq!(soltar(&mut e, vec![pdf], Some(0)), 0);
    assert_eq!(markdown(&e), antes);
    assert_eq!(recibido.borrow().len(), 1);
    desmontar(e);
}

#[test]
fn el_arrastre_de_windows_llega_al_bucle_y_mete_sus_fotos() {
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    let recibido = Comun::new(Celda::new(Vec::new()));
    let mut e = abrir_con(
        "",
        Box::new(|_: &str| None),
        adjuntar_que_apunta(recibido.clone()),
        Integracion::default(),
    );
    let foto = png("soltada.png", 5, 5);
    let bytes = pixpin_codec::portapapeles::construir_hdrop(&[foto]).unwrap();
    // SAFETY: memoria global propia del tamano justo; la suelta
    // `DragFinish` al leer el arrastre, como con uno de Windows.
    let h = unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).unwrap();
        let p = GlobalLock(h) as *mut u8;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        let _ = GlobalUnlock(h);
        h
    };
    mensaje_del_marco(WM_DROPFILES, WPARAM(h.0 as usize), LPARAM(0));
    assert!(COLA.with(|c| c.borrow().contains(&Orden::Soltar)));
    COLA.with(|c| c.borrow_mut().clear());
    soltar_lo_soltado(&mut e);
    assert_eq!(
        markdown(&e),
        "![soltada](pixpin:files/guardados/pc/p1/notas/9-soltada.png)"
    );
    // Caso negativo: no queda nada pendiente por soltar otra vez.
    soltar_lo_soltado(&mut e);
    assert_eq!(recibido.borrow().len(), 1);
    desmontar(e);
}

/// Una nota con una foto de 800x400 en el renglon 1, pintada en memoria
/// con el cursor en su renglon (borde y asa a la vista).
fn con_foto_elegida(integracion: Integracion) -> Estado {
    let foto = png("planta.png", 800, 400);
    let mut e = abrir_con(
        &format!("# Obra\n![Planta]({R})\nfin"),
        Box::new(move |_: &str| Some(foto.clone())),
        Box::new(|_: &Path| None),
        integracion,
    );
    let t = leer(e.edit);
    let fin_foto = md_vivo::lineas(&t)[1].hasta;
    elegir(e.edit, fin_foto, fin_foto);
    pintar(&mut e, None);
    let _ = muestra(&e);
    e
}

#[test]
fn el_asa_cambia_el_ancho_y_el_ancho_va_en_el_texto_de_la_foto() {
    let mut e = con_foto_elegida(Integracion::default());
    let (_, _, caja, ancho) = imagenes::puesta(0).expect("la foto esta puesta");
    assert!(ancho > 0 && caja.right > caja.left);
    let asa = (caja.right - 3, caja.bottom - 3);
    assert!(pulsar(&mut e, WM_LBUTTONDOWN, asa.0, asa.1));
    // Hasta la mitad: el ancho nuevo es la mitad del de la foto.
    let centro = (caja.left + caja.right) / 2;
    let mitad = centro + (caja.right - caja.left) / 4;
    assert!(pulsar(&mut e, WM_MOUSEMOVE, mitad, asa.1));
    assert!(pulsar(&mut e, WM_LBUTTONUP, mitad, asa.1));
    let md = markdown(&e);
    let f = md_imagen::leer(md.lines().nth(1).unwrap()).unwrap();
    let esperado = (caja.right - caja.left) as u32 / 2 * 96 / e.ppp as u32;
    assert!(f.ancho.unwrap().abs_diff(esperado) <= 2, "{md}");
    assert_eq!(f.ruta, R);
    // Caso negativo: sin arrastre, mover el raton no es cosa de las fotos.
    assert!(!pulsar(&mut e, WM_MOUSEMOVE, 5, 5));
    desmontar(e);
}

#[test]
fn sin_elegir_la_foto_su_esquina_no_es_un_asa() {
    let mut e = con_foto_elegida(Integracion::default());
    // El cursor al principio: la foto ya no esta elegida.
    elegir(e.edit, 0, 0);
    pintar(&mut e, Some(&[0, 1]));
    let (_, _, caja, _) = imagenes::puesta(0).unwrap();
    assert_eq!(
        imagenes::tocar(caja.right - 3, caja.bottom - 3),
        Some((0, false))
    );
    desmontar(e);
}

#[test]
fn el_menu_de_la_foto_cambia_su_tamano_y_la_quita() {
    let mut e = con_foto_elegida(Integracion::default());
    e.foto_del_menu = Some(1);
    orden_de_foto(&mut e, C_FOTO_MEDIANA);
    assert_eq!(markdown(&e), format!("# Obra\n![Planta|360]({R})\nfin"));
    e.foto_del_menu = Some(1);
    orden_de_foto(&mut e, C_FOTO_COLUMNA);
    assert_eq!(markdown(&e), format!("# Obra\n![Planta]({R})\nfin"));
    // Caso negativo: el renglon del titulo no es una foto.
    e.foto_del_menu = Some(0);
    orden_de_foto(&mut e, C_QUITAR_FOTO);
    assert_eq!(markdown(&e), format!("# Obra\n![Planta]({R})\nfin"));
    e.foto_del_menu = Some(1);
    orden_de_foto(&mut e, C_QUITAR_FOTO);
    assert_eq!(markdown(&e), "# Obra\nfin");
    desmontar(e);
}

#[test]
fn una_foto_con_ancho_va_y_vuelve_igual_y_se_ensena_a_ese_ancho() {
    let foto = png("ancha.png", 1600, 800);
    let md = format!("![Planta|300]({R})\n![Viva]({VIVA})\n");
    let e = abrir_con(
        &md,
        Box::new(move |_: &str| Some(foto.clone())),
        Box::new(|_: &Path| None),
        Integracion::default(),
    );
    assert_eq!(markdown(&e), md);
    let anchos: Vec<i32> =
        imagenes::PUESTAS.with(|p| p.borrow().iter().map(|x| x.foto.ancho).collect());
    assert_eq!(anchos[0], 300 * e.ppp / 96);
    // La de su tamano llena la columna, sin salirse de lo que se ve.
    let mut dentro = RECT::default();
    enviar(e.edit, 0x00B2, 0, &mut dentro as *mut _ as isize);
    assert!(anchos[1] > anchos[0] && anchos[1] <= dentro.right - dentro.left);
    assert!(anchos[1] <= tabla_rtf::COLUMNA_PX * e.ppp / 96);
    desmontar(e);
}

#[test]
fn doble_clic_en_una_pagina_viva_abre_su_hoja_y_un_enlace_a_hoja_tambien() {
    let abiertas = Comun::new(Celda::new(Vec::<String>::new()));
    let a = abiertas.clone();
    let foto = png("viva.png", 600, 300);
    let mut e = abrir_con(
        &format!("![Planta baja]({VIVA})\n[ver](pixpin:hoja=P9/K7Q2ABCDEF)"),
        Box::new(move |_: &str| Some(foto.clone())),
        Box::new(|_: &Path| None),
        Integracion {
            abrir: Some(Box::new(move |r: &str| a.borrow_mut().push(r.to_string()))),
            ..Default::default()
        },
    );
    let _ = muestra(&e);
    let (_, _, caja, _) = imagenes::puesta(0).unwrap();
    let centro = ((caja.left + caja.right) / 2, (caja.top + caja.bottom) / 2);
    assert!(pulsar(&mut e, WM_LBUTTONDBLCLK, centro.0, centro.1));
    assert_eq!(*abiertas.borrow(), vec![VIVA.to_string()]);
    // El enlace: lo que hace Ctrl+clic con el.
    let t = leer(e.edit);
    let en_ver = t[..t.find("[ver]").unwrap() + 2].encode_utf16().count();
    let url = md_vivo::enlace_en(&t, en_ver).unwrap();
    assert!(es_enlace_a_hoja(&url));
    abrir(&mut e, &url);
    assert_eq!(abiertas.borrow()[1], "pixpin:hoja=P9/K7Q2ABCDEF");
    // Caso negativo: un doble clic fuera de la foto no abre nada.
    assert!(!pulsar(&mut e, WM_LBUTTONDBLCLK, 2, caja.bottom + 200));
    assert_eq!(abiertas.borrow().len(), 2);
    assert!(!es_enlace_a_hoja("https://x.es"));
    desmontar(e);
}

type Respuestas = Comun<Celda<Vec<Vec<(String, Viva)>>>>;

/// La aplicacion de mentira: una hoja en el selector, su renglon, y lo
/// que dice `vigilar` cada vez.
fn integracion_de_prueba(
    respuestas: Respuestas,
    vistas: Comun<Celda<Vec<Vec<String>>>>,
) -> Integracion {
    Integracion {
        hojas: Some(Box::new(|| {
            vec![GrupoDeHojas {
                proyecto: "Casa Lima".into(),
                propio: true,
                hojas: vec![HojaElegible {
                    clave: "p1\u{1f}K7Q2ABCDEF".into(),
                    nombre: "Planta baja".into(),
                }],
            }]
        })),
        insertar_hoja: Some(Box::new(|clave: &str, enlace: bool| {
            assert_eq!(clave, "p1\u{1f}K7Q2ABCDEF");
            Some(if enlace {
                md_imagen::enlace_a_hoja("Planta baja", "P9", "K7Q2ABCDEF")
            } else {
                format!("![Planta baja]({VIVA})")
            })
        })),
        vigilar: Some(Box::new(move |rutas: &[String]| {
            vistas.borrow_mut().push(rutas.to_vec());
            respuestas.borrow_mut().pop().unwrap_or_default()
        })),
        ..Default::default()
    }
}

#[test]
fn una_pagina_viva_se_ve_en_cuanto_la_aplicacion_la_pinta_y_se_relee_al_cambiar() {
    let respuestas: Respuestas = Comun::default();
    let vistas = Comun::new(Celda::new(Vec::new()));
    let fichero = carpeta().join("vivo-K7Q2ABCDEF.png");
    let _ = std::fs::remove_file(&fichero);
    let f = fichero.clone();
    let mut e = abrir_con(
        "Plano:",
        Box::new(move |r: &str| md_imagen::hoja_de_viva(r).map(|_| f.clone())),
        Box::new(|_: &Path| None),
        integracion_de_prueba(respuestas.clone(), vistas.clone()),
    );
    assert!(meter_hoja(&mut e, "p1\u{1f}K7Q2ABCDEF", false));
    assert_eq!(markdown(&e), format!("Plano:\n![Planta baja]({VIVA})"));
    pintar(&mut e, None);
    // Aun sin pintar: el renglon se queda como texto.
    assert_eq!(imagenes::PUESTAS.with(|p| p.borrow().len()), 0);
    // La aplicacion la pinta y lo dice.
    png("vivo-K7Q2ABCDEF.png", 400, 200);
    respuestas
        .borrow_mut()
        .push(vec![(VIVA.to_string(), Viva::Renovada)]);
    vigilar(&mut e);
    assert_eq!(vistas.borrow().last().unwrap(), &vec![VIVA.to_string()]);
    assert_eq!(
        imagenes::PUESTAS.with(|p| p.borrow()[0].foto.ancho),
        400 * e.ppp / 96
    );
    // La hoja cambio: otra imagen; sin que la aplicacion lo diga no se relee...
    png("vivo-K7Q2ABCDEF.png", 300, 300);
    vigilar(&mut e);
    pintar(&mut e, None);
    assert_eq!(
        imagenes::PUESTAS.with(|p| p.borrow()[0].foto.ancho),
        400 * e.ppp / 96
    );
    // ...y diciendolo, si.
    respuestas
        .borrow_mut()
        .push(vec![(VIVA.to_string(), Viva::Renovada)]);
    vigilar(&mut e);
    assert_eq!(
        imagenes::PUESTAS.with(|p| p.borrow()[0].foto.ancho),
        300 * e.ppp / 96
    );
    // El Markdown no cambia por repintarse.
    assert_eq!(markdown(&e), format!("Plano:\n![Planta baja]({VIVA})"));
    desmontar(e);
}

#[test]
fn una_pagina_viva_cuya_hoja_se_borro_se_queda_con_su_copia_y_un_aviso() {
    let respuestas: Respuestas =
        Comun::new(Celda::new(vec![vec![(VIVA.to_string(), Viva::SinHoja)]]));
    let foto = png("vivo-borrada.png", 400, 200);
    let mut e = abrir_con(
        &format!("![Planta baja]({VIVA})"),
        Box::new(move |_: &str| Some(foto.clone())),
        Box::new(|_: &Path| None),
        integracion_de_prueba(respuestas, Comun::default()),
    );
    vigilar(&mut e);
    let aviso = imagenes::PUESTAS.with(|p| p.borrow()[0].aviso.clone());
    assert_eq!(aviso.as_deref(), Some("La hoja ya no está"));
    assert_eq!(markdown(&e), format!("![Planta baja]({VIVA})"));
    desmontar(e);
}

#[test]
fn el_enlace_a_una_hoja_va_en_el_cursor_con_lo_elegido_como_texto() {
    let mut e = abrir_con(
        "ver plano hoy",
        Box::new(|_: &str| None),
        Box::new(|_: &Path| None),
        integracion_de_prueba(Comun::default(), Comun::default()),
    );
    elegir(e.edit, 4, 9);
    assert!(meter_hoja(&mut e, "p1\u{1f}K7Q2ABCDEF", true));
    assert_eq!(markdown(&e), "ver [plano](pixpin:hoja=P9/K7Q2ABCDEF) hoy");
    // Sin nada elegido, con el nombre de la hoja.
    let fin = leer(e.edit).encode_utf16().count();
    elegir(e.edit, fin, fin);
    meter_hoja(&mut e, "p1\u{1f}K7Q2ABCDEF", true);
    assert!(markdown(&e).ends_with(" hoy[Planta baja](pixpin:hoja=P9/K7Q2ABCDEF)"));
    desmontar(e);
}

#[test]
fn lo_mandado_desde_fuera_entra_en_la_nota_al_mirar() {
    let mut lista = vec![format!("![Planta baja]({VIVA})")];
    let mut e = abrir_con(
        "Notas",
        Box::new(|_: &str| None),
        Box::new(|_: &Path| None),
        Integracion {
            pendientes: Some(Box::new(move || std::mem::take(&mut lista))),
            ..Default::default()
        },
    );
    vigilar(&mut e);
    assert_eq!(markdown(&e), format!("Notas\n![Planta baja]({VIVA})"));
    // Caso negativo: lo ya metido no se repite.
    vigilar(&mut e);
    assert_eq!(markdown(&e).matches("vivo-").count(), 1);
    desmontar(e);
}

#[test]
fn sin_aplicacion_detras_no_se_ofrecen_las_hojas() {
    let ids_del_mas = |e: &mut Estado| -> Vec<u16> {
        clic(e, Boton::Mas, &mut |_| true);
        let ids = menu::VISTA.with(|v| {
            v.borrow()
                .as_ref()
                .unwrap()
                .menu
                .entradas
                .iter()
                .map(|x| x.id)
                .collect()
        });
        cerrar_menu(e);
        ids
    };
    let mut e = abrir_con(
        "x",
        Box::new(|_: &str| None),
        Box::new(|_: &Path| None),
        Integracion::default(),
    );
    let ids = ids_del_mas(&mut e);
    assert!(!ids.contains(&C_PAGINA_VIVA) && !ids.contains(&C_ENLACE_HOJA));
    // Y pedirlas no hace nada.
    hoja_de_un_proyecto(&mut e, false);
    assert_eq!(markdown(&e), "x");
    desmontar(e);
    // Con aplicacion, si.
    let mut e = abrir_con(
        "x",
        Box::new(|_: &str| None),
        Box::new(|_: &Path| None),
        integracion_de_prueba(Comun::default(), Comun::default()),
    );
    let ids = ids_del_mas(&mut e);
    assert!(ids.contains(&C_PAGINA_VIVA) && ids.contains(&C_ENLACE_HOJA));
    desmontar(e);
}

/// Muestra a mano: una foto elegida (borde y asa) y una pagina viva sin su
/// hoja (con el aviso). `PIXPIN_MUESTRA=<carpeta> cargo test -p pixpin-notas
/// muestra_foto_elegida -- --ignored`
#[test]
#[ignore]
fn muestra_foto_elegida_y_aviso() {
    let respuestas: Respuestas =
        Comun::new(Celda::new(vec![vec![(VIVA.to_string(), Viva::SinHoja)]]));
    let (planta, viva) = (
        png_de("planta-m.png", 800, 260, [70, 140, 90]),
        png_de("viva-m.png", 900, 200, [235, 225, 205]),
    );
    let mut e = abrir_con(
        &format!("# Obra\n![Planta]({R})\n![Planta baja]({VIVA})\nfin"),
        Box::new(move |r: &str| Some(if r == R { planta.clone() } else { viva.clone() })),
        Box::new(|_: &Path| None),
        integracion_de_prueba(respuestas, Comun::default()),
    );
    vigilar(&mut e);
    let t = leer(e.edit);
    let fin = md_vivo::lineas(&t)[1].hasta;
    elegir(e.edit, fin, fin);
    pintar(&mut e, None);
    let arriba = POINT::default();
    enviar(e.edit, EM_SETSCROLLPOS, 0, &arriba as *const _ as isize);
    let img = muestra(&e);
    let carpeta = std::env::var("PIXPIN_MUESTRA").unwrap_or_else(|_| ".".into());
    std::fs::write(
        Path::new(&carpeta).join("nota-md-foto-elegida.png"),
        pixpin_codec::imagen::codificar_png(&img).unwrap(),
    )
    .unwrap();
    desmontar(e);
}
