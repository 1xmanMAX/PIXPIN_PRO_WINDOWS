//! Que **ningun fotograma** ensene una marca (H12, 1-oct). El espia apunta
//! lo que el control podria ensenar en cada momento en que no esta
//! congelado (tras cada mensaje que cambia el texto, tras cada cambio por un
//! rango del documento y al descongelar): en ninguno puede haber una marca
//! a la vista.

use super::super::pruebas::{abrir, abrir_con, foto_de_prueba, pedido};
use super::super::*;
use super::*;

fn msg(e: &Estado, m: u32, w: usize) -> MSG {
    MSG {
        hwnd: e.edit,
        message: m,
        wParam: WPARAM(w),
        ..Default::default()
    }
}

/// Una tecla o una letra como las ve el bucle: lo de antes del control, el
/// control si no se atendio, y lo de despues.
fn como_el_bucle(e: &mut Estado, m: MSG) {
    if !interceptar(e, &m, &mut |_| true) {
        enviar(e.edit, m.message, m.wParam.0, 0);
    }
    wysiwyg::despues(e, &m);
}

fn teclear(e: &mut Estado, s: &str) {
    for c in s.chars() {
        como_el_bucle(e, msg(e, WM_CHAR, c as usize));
    }
}

fn orden(e: &mut Estado, c: u16) {
    atender(e, Orden::Comando(c), &mut |_| true);
}

/// Lo que vio el espia mientras corria `f`: no vacio, y sin marcas.
fn sin_marcas_a_la_vista(e: &mut Estado, que: &str, f: impl FnOnce(&mut Estado)) {
    espiar();
    f(e);
    let vistos = fotogramas();
    assert!(
        !vistos.is_empty(),
        "{que}: el espia no vio ningun fotograma"
    );
    for (i, v) in vistos.iter().enumerate() {
        assert!(
            v.marcas_a_la_vista.is_empty(),
            "{que}: el fotograma {i} ensena marcas en {:?} de {:?}",
            v.marcas_a_la_vista,
            v.texto
        );
    }
}

fn en(e: &Estado, trozo: &str) -> usize {
    let t = leer(e.edit);
    t[..t.find(trozo).unwrap()].encode_utf16().count()
}

#[test]
fn el_espia_ve_las_marcas_de_un_cambio_hecho_sin_congelar() {
    // Caso negativo: lo que hacia el editor antes (meter `**` y esperar a
    // los 60 ms del formato) deja un fotograma con los asteriscos.
    let e = abrir("hola mundo");
    espiar();
    elegir(e.edit, 5, 10);
    enviar(
        e.edit,
        EM_REPLACESEL,
        1,
        ancho_nulo("**mundo**").as_ptr() as isize,
    );
    let vistos = fotogramas();
    assert!(
        vistos.iter().any(|v| !v.marcas_a_la_vista.is_empty()),
        "{vistos:?}"
    );
    desmontar(e);
}

#[test]
fn poner_formato_desde_el_menu_las_teclas_o_la_barra_no_ensena_marcas() {
    let mut e = abrir("hola mundo\notra linea\ny una tercera");
    let mundo = en(&e, "mundo");
    for (c, que) in [
        (C_NEGRITA, "negrita"),
        (C_CURSIVA, "cursiva"),
        (C_TACHADO, "tachado"),
        (C_CODIGO, "codigo"),
        (C_T1, "titulo"),
        (C_LISTA, "lista"),
        (C_NUMERADA, "numerada"),
        (C_CASILLA, "casilla"),
        (C_CITA, "cita"),
    ] {
        elegir(e.edit, mundo, mundo + 5);
        sin_marcas_a_la_vista(&mut e, que, |e| orden(e, c));
        // Y otra vez, que lo quita: tampoco se ve nada a medias.
        let m = en(&e, "mundo");
        elegir(e.edit, m, m + 5);
        sin_marcas_a_la_vista(&mut e, que, |e| orden(e, c));
    }
    assert_eq!(markdown(&e), "hola mundo\notra linea\ny una tercera");
    // La barra flotante pasa por lo mismo.
    elegir(e.edit, mundo, mundo + 5);
    sin_marcas_a_la_vista(&mut e, "barra", |e| {
        wysiwyg::clic_barra(e, crate::barra_flotante::BotonBarra::Negrita, &mut |_| true)
    });
    assert_eq!(markdown(&e), "hola **mundo**\notra linea\ny una tercera");
    desmontar(e);
}

#[test]
fn escribir_marcas_a_mano_las_esconde_desde_el_primer_fotograma() {
    let mut e = abrir("");
    // Un titulo, una lista, una casilla al vuelo, un separador y una negrita
    // cerrada a mano.
    sin_marcas_a_la_vista(&mut e, "titulo", |e| teclear(e, "# Plan"));
    sin_marcas_a_la_vista(&mut e, "intro", |e| {
        como_el_bucle(e, msg(e, WM_KEYDOWN, VK_RETURN.0 as usize))
    });
    sin_marcas_a_la_vista(&mut e, "lista", |e| teclear(e, "- uno"));
    sin_marcas_a_la_vista(&mut e, "intro en lista", |e| {
        como_el_bucle(e, msg(e, WM_KEYDOWN, VK_RETURN.0 as usize))
    });
    assert!(leer(e.edit).ends_with("- uno\n- "), "{:?}", leer(e.edit));
    // Intro en el renglon de lista vacio sale de la lista.
    sin_marcas_a_la_vista(&mut e, "salir de la lista", |e| {
        como_el_bucle(e, msg(e, WM_KEYDOWN, VK_RETURN.0 as usize))
    });
    sin_marcas_a_la_vista(&mut e, "casilla", |e| teclear(e, "[] tarea"));
    assert!(leer(e.edit).contains("- [ ] tarea"), "{:?}", leer(e.edit));
    sin_marcas_a_la_vista(&mut e, "negrita", |e| teclear(e, " y **fuerte** fin"));
    assert!(leer(e.edit).contains("**fuerte**"));
    desmontar(e);
}

#[test]
fn la_primera_letra_tras_una_marca_escondida_se_ve_en_el_acto() {
    // Detras de `# ` escondido, la letra no hereda lo escondido.
    let mut e = abrir("# ");
    elegir(e.edit, 2, 2);
    pintar(&mut e, None);
    teclear(&mut e, "T");
    let d = e.doc.as_ref().unwrap();
    // SAFETY: rango del documento vivo de la prueba.
    let oculta = unsafe {
        d.Range(2, 3)
            .and_then(|r| r.GetFont())
            .and_then(|f| f.GetHidden())
            .unwrap()
    };
    assert_eq!(oculta, 0, "la T se ve");
    assert_eq!(markdown(&e), "# T");
    desmontar(e);
}

#[test]
fn pegar_markdown_deshacer_y_rehacer_no_ensenan_marcas() {
    let mut e = abrir("antes ");
    let fin = leer(e.edit).encode_utf16().count();
    elegir(e.edit, fin, fin);
    sin_marcas_a_la_vista(&mut e, "pegar", |e| {
        wysiwyg::pegar_texto(e, "**pegado** y `codigo`\n- [ ] tarea")
    });
    assert_eq!(markdown(&e), "antes **pegado** y `codigo`\n- [ ] tarea");
    let ctrl_z = MSG {
        hwnd: e.edit,
        message: WM_KEYDOWN,
        wParam: WPARAM(b'Z' as usize),
        ..Default::default()
    };
    sin_marcas_a_la_vista(&mut e, "deshacer", |e| assert!(deshacer(e, false)));
    assert_eq!(markdown(&e), "antes ");
    sin_marcas_a_la_vista(&mut e, "rehacer", |e| assert!(deshacer(e, true)));
    assert_eq!(markdown(&e), "antes **pegado** y `codigo`\n- [ ] tarea");
    // Ctrl+Z sin Ctrl no es deshacer: es una z.
    assert!(!es_deshacer(&ctrl_z, false, false));
    assert!(es_deshacer(&ctrl_z, true, false));
    desmontar(e);
}

#[test]
fn meter_una_foto_nace_escondida_y_con_su_hueco() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let foto = foto_de_prueba("congelar-foto.png", 300, 200);
    let mut p = pedido("Arriba");
    let ruta = foto.clone();
    p.resolver = Box::new(move |_: &str| Some(ruta.clone()));
    p.adjuntar = Box::new(|r: &Path| Some(r.file_name().unwrap().to_string_lossy().to_string()));
    let mut e = abrir_con(p, false, (1000, 760));
    let fin = leer(e.edit).encode_utf16().count();
    elegir(e.edit, fin, fin);
    sin_marcas_a_la_vista(&mut e, "foto", |e| {
        assert_eq!(fotos::meter(e, std::slice::from_ref(&foto)), 1);
    });
    // En el mismo fotograma que el texto: la foto y el aire de su alto.
    let (linea, ..) = imagenes::puesta(0).expect("la foto se pinta ya");
    assert_eq!(linea, 1);
    let l = md_vivo::lineas(&leer(e.edit))[1];
    let d = e.doc.as_ref().unwrap();
    // SAFETY: rango del documento vivo de la prueba.
    let aire = unsafe {
        d.Range(l.desde as i32, l.desde as i32)
            .and_then(|r| r.GetPara())
            .and_then(|p| p.GetSpaceBefore())
            .unwrap()
    };
    assert!(
        aire >= 200.0 * 72.0 / e.ppp as f32,
        "el hueco es del alto de la foto: {aire}"
    );
    // Y escribir con el cursor en la foto abre un renglon debajo, sin
    // tocar el de la foto.
    teclear(&mut e, "x");
    assert!(markdown(&e).ends_with(")\nx"), "{:?}", markdown(&e));
    desmontar(e);
}
