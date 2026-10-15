//! Pruebas del pin de hoja: que se abre al tocar una burbuja, el paso de
//! ver a editar y de vuelta, guardar en el chat sin tocar el archivo, y
//! (ignorada) la ventana de verdad fotografiada.

use super::*;
use pixpin_proyecto::tabla::Ref;
use std::time::Duration;
use pixpin_proyecto::cuaderno::{Cuaderno, Sello};
use pixpin_proyecto::tabla::ref_de;
use pixpin_proyecto::tabla_xlsx::{HojaParaVer, a_xlsx};

fn r(t: &str) -> Ref {
    ref_de(t).unwrap()
}

/// Lo que escribe la orden `ESCRIBIR` de las pruebas en la hoja activa:
/// un total con formula, en negrita, y un rango elegido.
pub(super) fn escribir_muestra(e: &mut Editor) {
    e.elegir(r("D2"), false);
    for c in "Revisado".chars() {
        e.teclear(c);
    }
    e.mover(0, 1, false);
    for c in "=2+2*10".chars() {
        e.teclear(c);
    }
    e.confirmar();
    e.formato(Formato::Alternar(Interruptor::Negrita));
    e.elegir(r("A2"), false);
    e.elegir(r("B4"), true);
}

fn tabla_de(celdas: &[(&str, &str)]) -> Tabla {
    let mut t = Tabla::default();
    for (k, v) in celdas {
        t.poner(r(k), v);
    }
    t
}

/// Un proyecto de prueba con un libro de dos hojas adjunto. Devuelve la
/// raiz, el id del proyecto, el mensaje del libro y la ruta del fichero.
fn proyecto_con_libro(nombre: &str) -> (PathBuf, String, Mensaje, PathBuf) {
    let raiz = std::env::temp_dir().join(format!("pixpin-hoja-pin-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    let ficha = pixpin_proyecto::almacen::Ficha::nueva("obra", 0, "PC");
    let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id);
    std::fs::create_dir_all(&carpeta).unwrap();
    let gastos = tabla_de(&[("A1", "Concepto"), ("B1", "Importe"), ("A2", "Cemento"), ("B2", "120"), ("A3", "Arena"), ("B3", "80"), ("B4", "=SUMA(B2:B3)")]);
    let resumen = tabla_de(&[("A1", "Total"), ("B1", "200")]);
    let bytes = a_xlsx(&[HojaParaVer { nombre: "Gastos", tabla: &gastos, valores: None }, HojaParaVer { nombre: "Resumen", tabla: &resumen, valores: None }], 1).unwrap();
    let ruta = carpeta.join("Cuentas.xlsx");
    std::fs::write(&ruta, &bytes).unwrap();
    let sello = Sello { cuando: 1_700_000_000_000, numero: 1, aparato: "PC".into(), proyecto: ficha.id.clone() };
    let m = Mensaje::adjunto(Clase::Archivo, "Cuentas.xlsx", "Cuentas.xlsx", bytes.len() as i64, &sello);
    pixpin_proyecto::cuaderno::anadir(&carpeta, &m).unwrap();
    (raiz, ficha.id, m, ruta)
}

#[test]
fn al_tocar_un_excel_o_una_tabla_se_abre_el_pin_y_lo_demas_no() {
    let libro = Mensaje { id: "1".into(), clase: Some(Clase::Archivo), nombre: "a.xlsx".into(), ruta: Some("a.xlsx".into()), ..Default::default() };
    assert_eq!(que_abrir(&libro), Some(Que::Libro));
    for n in ["b.xlsm", "c.ods", "d.csv", "e.tsv"] {
        let m = Mensaje { nombre: n.into(), ruta: Some(n.into()), ..libro.clone() };
        assert_eq!(que_abrir(&m), Some(Que::Libro), "{n}");
    }
    let tabla = Mensaje { clase: Some(Clase::MiniApp), miniapp: Some("tabla".into()), texto: "{}".into(), ..Default::default() };
    assert_eq!(que_abrir(&tabla), Some(Que::Tabla));
    // Casos negativos: un .xls antiguo sigue su camino (no se lee), un PDF,
    // otra mini-app y una nota no son hojas.
    let xls = Mensaje { nombre: "viejo.xls".into(), ruta: Some("viejo.xls".into()), ..libro.clone() };
    assert_eq!(que_abrir(&xls), None);
    let pdf = Mensaje { nombre: "a.pdf".into(), ruta: Some("a.pdf".into()), ..libro.clone() };
    assert_eq!(que_abrir(&pdf), None);
    let tareas = Mensaje { miniapp: Some("tareas".into()), ..tabla.clone() };
    assert_eq!(que_abrir(&tareas), None);
    let nota = Mensaje { clase: Some(Clase::Nota), ..Default::default() };
    assert_eq!(que_abrir(&nota), None);
    // Y un libro que no esta en este equipo no se abre (se dice que no).
    let u = Ubicacion::Portable { raiz: std::env::temp_dir().join("pixpin-hoja-pin-nada") };
    assert!(!abrir_desde_chat(pixpin_store::Idioma::Espanol, &u, "p", std::slice::from_ref(&libro), 0));
    assert!(!abrir_desde_chat(pixpin_store::Idioma::Espanol, &u, "p", &[], 3));
}

#[test]
fn las_teclas_del_modo_editar() {
    use AccionEd::*;
    let alternar = |i| Some(Formato(editor::Formato::Alternar(i)));
    assert_eq!(accion_de_tecla(0x5A, false, true, false), Some(Deshacer));
    assert_eq!(accion_de_tecla(0x5A, true, true, false), Some(Rehacer));
    assert_eq!(accion_de_tecla(0x59, false, true, false), Some(Rehacer));
    // Los atajos de formato de la Excel espanola y de la inglesa.
    for (vk, i) in [
        (0x4E, Interruptor::Negrita),
        (0x42, Interruptor::Negrita),
        (0x4B, Interruptor::Cursiva),
        (0x49, Interruptor::Cursiva),
        (0x53, Interruptor::Subrayado),
        (0x55, Interruptor::Subrayado),
        (0x35, Interruptor::Tachado),
    ] {
        assert_eq!(accion_de_tecla(vk, false, true, false), alternar(i), "{vk:#x}");
        assert_eq!(accion_de_tecla(vk, false, true, true), alternar(i), "{vk:#x} tambien escribiendo");
    }
    assert_eq!(accion_de_tecla(0x45, true, true, false), Some(Formato(editor::Formato::Alinear(Alineacion::Centro))));
    // Escribiendo, Intro, Tab y las flechas dejan lo escrito y mueven (el
    // visor); sin escribir son del visor.
    assert_eq!(accion_de_tecla(VK_ENTRAR, false, false, true), Some(Confirmar));
    assert_eq!(accion_de_tecla(VK_TAB, true, false, true), Some(Confirmar));
    assert_eq!(accion_de_tecla(0x27, false, false, true), Some(Confirmar));
    assert_eq!(accion_de_tecla(VK_ENTRAR, false, false, false), None);
    assert_eq!(accion_de_tecla(0x27, true, false, false), None);
    assert_eq!(accion_de_tecla(VK_F2, false, false, false), Some(Empezar));
    // Esc: escribiendo deshace lo tecleado; sin escribir, vuelve a ver.
    assert_eq!(accion_de_tecla(VK_ESCAPE, false, false, true), Some(Cancelar));
    assert_eq!(accion_de_tecla(VK_ESCAPE, false, false, false), Some(Ver));
    // Suprimir escribiendo no borra la celda entera.
    assert_eq!(accion_de_tecla(VK_SUPR, false, false, true), None);
    assert_eq!(accion_de_tecla(VK_SUPR, false, false, false), Some(Borrar));
    // Casos negativos: una letra sola es un caracter, Ctrl+A escribiendo no
    // elige la hoja, y Ctrl+0 (el zoom) es del visor.
    assert_eq!(accion_de_tecla(0x41, false, false, false), None);
    assert_eq!(accion_de_tecla(0x41, false, true, true), Some(Nada));
    assert_eq!(accion_de_tecla(0x41, false, true, false), None);
    assert_eq!(accion_de_tecla(0x30, false, true, false), None);
}

#[test]
fn ver_editar_ver_conserva_la_hoja_activa() {
    // Viendo el original: las hojas se casan por el nombre, aunque el libro
    // tenga una hoja de graficos que no es tabla.
    let vista = ["Gastos", "Grafico", "Resumen"];
    let tablas = ["Cuentas · Gastos", "Cuentas · Resumen"];
    assert_eq!(tabla_de_vista(true, &vista, &tablas, 2), Some(1), "Resumen es la segunda tabla");
    assert_eq!(tabla_de_vista(true, &vista, &tablas, 0), Some(0));
    // Caso negativo: la hoja de graficos no tiene tabla: no se edita.
    assert_eq!(tabla_de_vista(true, &vista, &tablas, 1), None);
    assert_eq!(tabla_de_vista(true, &vista, &tablas, 9), None);
    // Una sola tabla: es esa, se llame como se llame la hoja.
    assert_eq!(tabla_de_vista(true, &["Hoja1"], &["Cuentas"], 0), Some(0));
    // Viendo las tablas, la misma posicion.
    assert_eq!(tabla_de_vista(false, &["Gastos", "Resumen"], &tablas, 1), Some(1));
    assert_eq!(tabla_de_vista(false, &["Gastos", "Resumen"], &tablas, 5), None, "no se sale");
}

#[test]
fn los_botones_de_formato_y_sus_menus() {
    let mut t = Tabla::default();
    t.poner(r("A1"), "1");
    t.estilos.insert("A1".into(), pixpin_proyecto::tabla::EstiloDeCelda { k: true, a: Some("d".into()), f: Some("#d3f9d8".into()), ..Default::default() });
    let mut e = Editor::nuevo(vec![HojaEditable::nueva("x".into(), t, None)], 0).unwrap();
    let b = botones_de_editar(&e);
    let encendido = |id: u8| b.iter().find(|x| x.id == id).unwrap().encendido;
    assert!(encendido(BOTON_CURSIVA) && encendido(BOTON_RESALTAR) && encendido(BOTON_ALINEAR));
    assert!(!encendido(BOTON_NEGRITA) && !encendido(BOTON_TACHADO));
    let icono = |id: u8| b.iter().find(|x| x.id == id).unwrap().icono.trazos[0].d;
    assert_eq!(icono(BOTON_ALINEAR), material::FORMAT_ALIGN_RIGHT.trazos[0].d, "ensena la alineacion que tiene");
    assert!(!b.iter().any(|x| x.icono.trazos[0].d == material::CONTENT_COPY.trazos[0].d), "sin boton Copiar");
    assert_eq!(pide_boton(BOTON_VER), Some(PideBoton::Ver));
    assert_eq!(pide_boton(BOTON_ALINEAR), Some(PideBoton::Formato(Formato::AlinearSiguiente)));
    assert_eq!(pide_boton(3), None, "los del modo ver no son de editar");
    let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
    let m = entradas_de_menu(Menu::Resaltar, &e, &textos);
    assert_eq!(m.len(), editor::FONDOS.len() + 1, "la paleta del movil y sin color");
    assert!(m.iter().any(|(_, t, f)| t.starts_with('\u{2713}') && *f == Formato::Fondo(Some("#d3f9d8".into()))), "marca el que tiene");
    let n = entradas_de_menu(Menu::Numero, &e, &textos);
    assert_eq!(n.len(), 5);
    assert!(n[0].1.starts_with('\u{2713}'), "sin formato, General");
    // Elegir del menu aplica a lo elegido y se deshace.
    let (_, _, f) = n.iter().find(|(_, _, f)| *f == Formato::Numero(Some(FormatoNumero::Porcentaje))).unwrap().clone();
    assert!(e.formato(f));
    assert_eq!(e.hoja().tabla.estilos["A1"].nf.as_deref(), Some("porcentaje"));
    assert!(e.deshacer());
    assert_eq!(e.hoja().tabla.estilos["A1"].nf, None);
}

#[test]
fn una_tabla_del_chat_se_ve_en_el_visor_con_sus_valores() {
    let t = tabla_de(&[("A1", "Concepto"), ("B1", "Importe"), ("B2", "12"), ("B3", "=B2*2"), ("C1", "=1/0")]);
    let bytes = a_xlsx(&[HojaParaVer { nombre: "Gastos", tabla: &t, valores: None }], 0).unwrap();
    let libro = Libro::desde_bytes(bytes, Regional::es_pe()).unwrap();
    assert_eq!(libro.hojas.len(), 1);
    assert_eq!(libro.hojas[0].nombre, "Gastos");
    let mut h = libro.abrir_hoja(0);
    h.cargar_todo();
    let texto = |f: u32, c: u32| h.celda(f, c).and_then(|x| libro.contenido(&h, x)).map(|x| x.mostrado.texto).unwrap_or_default();
    assert_eq!(texto(0, 0), "Concepto");
    assert_eq!(texto(2, 1), "24", "la formula se ve con su resultado");
    assert!(texto(0, 2).starts_with('#'), "un error se ve como error: {}", texto(0, 2));
    assert_eq!(texto(5, 5), "", "lo vacio sigue vacio");
}

#[test]
fn ver_un_excel_no_lo_lee_como_tablas_y_editarlo_guarda_en_el_chat_sin_tocar_el_archivo() {
    let (raiz, proyecto, libro_m, ruta) = proyecto_con_libro("guardar");
    let original = std::fs::read(&ruta).unwrap();
    let u = Ubicacion::Portable { raiz: raiz.clone() };
    let origen = Origen::Libro { mensaje: libro_m.clone(), ruta: ruta.clone(), tablas: Vec::new() };
    let mut ed = Editable::nuevo(u.clone(), proyecto.clone(), origen);

    // Modo ver: el original, sin montar el editor (eso es al pulsar Editar).
    let visto = ed.libro_para_ver(Regional::es_pe()).unwrap();
    assert!(ed.vista_original);
    assert!(ed.editor.is_none(), "ver no lee el libro como tablas");
    assert_eq!(visto.hojas.iter().map(|h| h.nombre.as_str()).collect::<Vec<_>>(), ["Gastos", "Resumen"]);

    // Editar: se leen las tablas una vez, y no se releen.
    ed.montar_editor().unwrap();
    assert!(ed.primera_edicion_de_libro(), "la primera vez se avisa de que el original no cambia");
    let e = ed.editor.as_mut().unwrap();
    assert_eq!(e.hojas.len(), 2);
    e.cambiar_hoja(1);
    e.elegir(r("B1"), false);
    e.teclear('9');
    e.confirmar();
    let puntero = ed.editor.as_ref().map(|e| e as *const Editor);
    ed.montar_editor().unwrap();
    assert_eq!(ed.editor.as_ref().map(|e| e as *const Editor), puntero, "montar otra vez no relee");

    // Guardar: las dos hojas quedan como tablas del chat con la referencia
    // del libro, y el archivo sigue igual byte a byte.
    ed.guardar().unwrap();
    assert!(!ed.editor.as_ref().unwrap().tocado());
    assert_eq!(std::fs::read(&ruta).unwrap(), original, "el .xlsx original no cambia");
    let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, &proyecto);
    let c = Cuaderno::leer_de(&carpeta).unwrap();
    let hechas = libro::tablas_hechas(&c.mensajes, &libro_m);
    assert_eq!(hechas.len(), 2);
    let resumen = &c.mensajes[hechas[1]];
    assert_eq!(resumen.referencia.as_deref(), Some(libro::id_de_hoja(&libro_m, 1).as_str()));
    assert_eq!(Tabla::leer(&resumen.texto).unwrap().celda(r("B1")), "9");
    assert!(libro::editada(resumen), "la tabla cambiada se reconoce como editada");
    assert!(!libro::editada(&c.mensajes[hechas[0]]), "la que no se toco, no");

    // Otro cambio va a SU mensaje, sin anadir otro.
    let e = ed.editor.as_mut().unwrap();
    e.elegir(r("A2"), false);
    e.teclear('x');
    e.confirmar();
    ed.guardar().unwrap();
    let c = Cuaderno::leer_de(&carpeta).unwrap();
    assert_eq!(libro::tablas_hechas(&c.mensajes, &libro_m).len(), 2, "no se duplican");
    assert_eq!(c.mensajes.len(), 3, "el libro y sus dos tablas");

    // La proxima vez que se toca el libro se ven sus tablas editadas, no
    // el original.
    let tablas: Vec<Mensaje> = libro::tablas_hechas(&c.mensajes, &libro_m).into_iter().map(|i| c.mensajes[i].clone()).collect();
    let mut otra = Editable::nuevo(u, proyecto, Origen::Libro { mensaje: libro_m, ruta, tablas });
    let visto = otra.libro_para_ver(Regional::es_pe()).unwrap();
    assert!(!otra.vista_original);
    assert_eq!(visto.hojas.iter().map(|h| h.nombre.as_str()).collect::<Vec<_>>(), ["Gastos", "Resumen"]);
    assert!(!otra.primera_edicion_de_libro(), "ya no es la primera vez");
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn un_csv_y_una_tabla_del_chat_se_ven_por_sus_tablas() {
    let (raiz, proyecto, _, _) = proyecto_con_libro("csv");
    let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, &proyecto);
    let ruta = carpeta.join("datos.csv");
    std::fs::write(&ruta, "Nombre;Edad\nAna;31\n").unwrap();
    let m = Mensaje { id: "77".into(), clase: Some(Clase::Archivo), nombre: "datos.csv".into(), ruta: Some("datos.csv".into()), ..Default::default() };
    let u = Ubicacion::Portable { raiz: raiz.clone() };
    let mut ed = Editable::nuevo(u.clone(), proyecto.clone(), Origen::Libro { mensaje: m, ruta, tablas: Vec::new() });
    let visto = ed.libro_para_ver(Regional::es_pe()).unwrap();
    assert!(!ed.vista_original, "un CSV no es un .xlsx: se ve por su tabla");
    assert_eq!(visto.hojas.len(), 1);
    assert!(ed.editor.is_some(), "y su tabla ya esta leida para editar");

    // Una tabla del chat: guardar cambia su mensaje.
    let sello = Sello { cuando: 5, numero: 2, aparato: "PC".into(), proyecto: proyecto.clone() };
    let doc = tabla_de(&[("A1", "1")]).escribir().unwrap();
    let tabla = Mensaje::miniapp("tabla", "Notas", &doc, &sello);
    pixpin_proyecto::cuaderno::anadir(&carpeta, &tabla).unwrap();
    let mut ed = Editable::nuevo(u, proyecto, Origen::Tabla { mensaje: tabla.clone() });
    assert_eq!(ed.nombre(), "Notas");
    assert!(ed.ruta_original().is_none(), "una tabla no tiene archivo");
    ed.libro_para_ver(Regional::es_pe()).unwrap();
    assert!(!ed.primera_edicion_de_libro(), "una tabla no avisa del archivo");
    let e = ed.editor.as_mut().unwrap();
    e.elegir(r("A1"), false);
    e.teclear('5');
    e.confirmar();
    ed.guardar().unwrap();
    let c = Cuaderno::leer_de(&carpeta).unwrap();
    let guardada = c.mensajes.iter().find(|x| x.id == tabla.id).unwrap();
    assert_eq!(Tabla::leer(&guardada.texto).unwrap().celda(r("A1")), "5");
    // Caso negativo: una tabla que no se entiende no se abre para no pisarla.
    let rota = Mensaje { texto: "no es json".into(), ..tabla };
    let mut ed = Editable::nuevo(Ubicacion::Portable { raiz: raiz.clone() }, "p".into(), Origen::Tabla { mensaje: rota });
    assert!(ed.libro_para_ver(Regional::es_pe()).is_err());
    let _ = std::fs::remove_dir_all(&raiz);
}

// ---------------------------------------------------------------------------
// La ventana de verdad

/// Fotografia lo que se ve de la ventana del pin (y su barra, encima) en
/// la pantalla. `PrintWindow` no sirve: la ventana no tiene superficie GDI
/// (`WS_EX_NOREDIRECTIONBITMAP`) y sale negra (ver `superficie.rs`); se
/// recorta la captura del monitor a la ventana, que es lo que el usuario ve.
fn fotografiar(nombre: &str) -> PathBuf {
    let (_, area, _) = prueba::ventana().expect("la ventana esta abierta");
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
    let ms = pixpin_capture::enumerar_monitores().expect("monitores");
    let centro = (area.x + area.ancho as i32 / 2, area.y + area.alto as i32 / 2);
    let m = ms
        .monitores()
        .iter()
        .find(|m| centro.0 >= m.area.x && centro.1 >= m.area.y && centro.0 < m.area.x + m.area.ancho as i32 && centro.1 < m.area.y + m.area.alto as i32)
        .or_else(|| ms.principal())
        .expect("un monitor")
        .clone();
    let e = m.escala_por_cien as f32 / 100.0;
    // Con la barra del pin, que va encima.
    let arriba = ((56.0 * e) as i32).min(area.y - m.area.y).max(0);
    let region = pixpin_geom::Rect { x: area.x, y: area.y - arriba, ancho: area.ancho, alto: area.alto + arriba as u32 };
    let region = region.interseccion(m.area).expect("la ventana esta en su monitor");
    let foto = pixpin_capture::capturar_monitor(&d, m.id, m.area).expect("captura");
    let recorte = foto.recortar(&d, region).expect("recorte");
    let img = pixpin_capture::a_imagen(&d, &recorte).expect("imagen");
    let png = pixpin_codec::imagen::codificar_png(&img).expect("png");
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let salida = carpeta.join(format!("hoja-pin-{nombre}.png"));
    std::fs::write(&salida, png).expect("guardar");
    println!("{nombre}: {}", salida.display());
    salida
}

fn esperar(que: impl Fn() -> bool, tope: Duration) {
    let t = Instant::now();
    while !que() {
        assert!(t.elapsed() < tope, "no llego a tiempo");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// **El pin de verdad** con un Excel del chat: abre en ver, pasa a editar,
/// escribe, vuelve a ver (con lo escrito) y cierra; fotografia cada paso.
/// Despues, una tabla del chat en ver. Las ordenes van por
/// `hoja_pin::prueba` (sin teclas ni clics simulados).
///
/// `PIXPIN_XLSX=<libro> PIXPIN_MUESTRAS=<carpeta> cargo test -p pixpin --bin pixpinmax hoja_pin::pruebas::pin_de_verdad_en_ver_y_en_editar -- --ignored --nocapture`
#[test]
#[ignore = "abre la ventana del pin en el escritorio y la fotografia"]
fn pin_de_verdad_en_ver_y_en_editar() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let (raiz, proyecto, libro_m, ruta) = proyecto_con_libro("ventana");
    // Un Excel de verdad si se da (una COPIA: el lector no escribe al lado).
    if let Some(x) = std::env::var_os("PIXPIN_XLSX") {
        std::fs::copy(x, &ruta).expect("copiar el libro de muestra");
    }
    let original = std::fs::read(&ruta).unwrap();
    let u = Ubicacion::Portable { raiz: raiz.clone() };
    let pausa = Duration::from_millis(1300);

    prueba::olvidar();
    let ed = Editable::nuevo(u.clone(), proyecto.clone(), Origen::Libro { mensaje: libro_m.clone(), ruta: ruta.clone(), tablas: Vec::new() });
    let t0 = Instant::now();
    crate::visor_hoja::lanzar_editable(pixpin_store::Idioma::Espanol, ed);
    esperar(|| prueba::ventana().is_some(), Duration::from_secs(20));
    println!("ventana abierta en {:?}", t0.elapsed());
    std::thread::sleep(pausa);
    fotografiar("1-ver");

    prueba::ordenar(prueba::EDITAR);
    esperar(|| prueba::ventana().is_some_and(|v| v.2), Duration::from_secs(30));
    std::thread::sleep(pausa);
    fotografiar("2-editar");

    prueba::ordenar(prueba::ESCRIBIR);
    std::thread::sleep(pausa);
    fotografiar("3-editar-escrito");

    prueba::ordenar(prueba::VER);
    esperar(|| prueba::ventana().is_some_and(|v| !v.2), Duration::from_secs(30));
    std::thread::sleep(pausa);
    fotografiar("4-ver-con-lo-escrito");

    prueba::ordenar(prueba::CERRAR);
    std::thread::sleep(Duration::from_millis(800));
    assert_eq!(std::fs::read(&ruta).unwrap(), original, "el archivo original no cambia");
    let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, &proyecto);
    let c = Cuaderno::leer_de(&carpeta).unwrap();
    let hechas = libro::tablas_hechas(&c.mensajes, &libro_m);
    assert!(!hechas.is_empty(), "lo editado quedo en tablas del chat");

    // Una tabla del chat, en ver.
    prueba::olvidar();
    let tabla = &c.mensajes[hechas[0]];
    let ed = Editable::nuevo(u, proyecto, Origen::Tabla { mensaje: tabla.clone() });
    crate::visor_hoja::lanzar_editable(pixpin_store::Idioma::Espanol, ed);
    esperar(|| prueba::ventana().is_some(), Duration::from_secs(20));
    std::thread::sleep(pausa);
    fotografiar("5-tabla-del-chat");
    prueba::ordenar(prueba::CERRAR);
    std::thread::sleep(Duration::from_millis(800));
    let _ = std::fs::remove_dir_all(&raiz);
}
