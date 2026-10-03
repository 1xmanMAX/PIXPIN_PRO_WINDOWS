//! Con la ventana entera pero **oculta**: el `RichEdit` de verdad, las
//! teclas y el raton con mensajes hechos a mano a la propia ventana (nada
//! de teclado ni raton de verdad), y las muestras pintadas en memoria.
use super::*;
use crate::editor::pruebas::{abrir, abrir_con, escribir, foto_de_prueba, guardar_png, pedido};
use crate::integracion::Integracion;
use super::super::{desmontar as cerrar, formato as un_formato};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_HOME;

fn msg(e: &Estado, m: u32, w: usize) -> MSG {
    MSG {
        hwnd: e.edit,
        message: m,
        wParam: WPARAM(w),
        lParam: LPARAM(0),
        ..Default::default()
    }
}

/// Una tecla como la ve el bucle: primero lo de aqui, si no la quiere el
/// control, y luego lo de despues.
fn pulsar(e: &mut Estado, vk: u16) {
    let m = msg(e, WM_KEYDOWN, vk as usize);
    if !tecla(e, &m) {
        enviar(e.edit, WM_KEYDOWN, vk as usize, 0);
    }
    despues(e, &m);
    enviar(e.edit, WM_KEYUP, vk as usize, 0);
}

/// Una letra escrita, como la ve el bucle.
fn teclear(e: &mut Estado, c: char) {
    let m = msg(e, WM_CHAR, c as usize);
    if !tecla(e, &m) {
        enviar(e.edit, WM_CHAR, c as usize, 0);
    }
    despues(e, &m);
}

/// La posicion UTF-16 de la primera vez que sale `trozo`.
fn en(e: &Estado, trozo: &str) -> usize {
    let t = leer(e.edit);
    t[..t.find(trozo).unwrap()].encode_utf16().count()
}

/// Los efectos de letra de la posicion `p`.
fn efectos(e: &Estado, p: usize) -> CHARFORMAT2W {
    let sel = seleccion(e.edit);
    let mut f = un_formato();
    f.Base.dwMask = CFM_HIDDEN | CFM_BOLD | CFM_SIZE | CFM_FACE;
    elegir(e.edit, p, p + 1);
    enviar(e.edit, EM_GETCHARFORMAT, SCF_SELECTION as usize, &mut f as *mut _ as isize);
    elegir(e.edit, sel.0, sel.1);
    f
}

/// Si la letra `p` esta escondida, mirado por un rango del documento: lo
/// elegido no sirve, el control lo saca del texto oculto.
fn escondida(e: &Estado, p: usize) -> bool {
    let d = e.doc.as_ref().expect("el control da su documento");
    // SAFETY: rango del documento vivo del control.
    unsafe { d.Range(p as i32, p as i32 + 1).and_then(|r| r.GetFont()).and_then(|f| f.GetHidden()).unwrap_or(0) != 0 }
}

#[test]
fn las_marcas_no_se_ven_ni_en_el_renglon_del_cursor() {
    let md = "a **bold** c\n# Titulo\n- uno\n- [ ] tarea\n3. tres\n> cita";
    let mut e = abrir(md);
    for cursor in [0, en(&e, "Titulo") + 2, en(&e, "uno"), en(&e, "tarea"), en(&e, "tres")] {
        elegir(e.edit, cursor, cursor);
        pintar(&mut e, None);
        for marca in ["**", "# ", "- ", "[ ]", "3.", "> "] {
            let p = en(&e, marca);
            assert!(escondida(&e, p), "«{marca}» se ve con el cursor en {cursor}");
        }
        // Caso negativo: el texto si se ve.
        for texto in ["bold", "Titulo", "uno", "tarea", "tres", "cita"] {
            assert!(!escondida(&e, en(&e, texto)), "«{texto}» no se ve");
        }
    }
    // El texto del control sigue siendo el Markdown.
    assert_eq!(markdown(&e), md);
    cerrar(e);
}

#[test]
fn una_lista_numerada_la_numera_windows_desde_su_primer_numero() {
    let mut e = abrir("4. cuatro\n4. cinco\n\n1) uno");
    pintar(&mut e, None);
    let parrafo = |e: &Estado, p: usize| {
        let mut x = PARAFORMAT2::default();
        x.Base.cbSize = std::mem::size_of::<PARAFORMAT2>() as u32;
        elegir(e.edit, p, p);
        enviar(e.edit, EM_GETPARAFORMAT, 0, &mut x as *mut _ as isize);
        x
    };
    let a = parrafo(&e, en(&e, "cuatro"));
    let b = parrafo(&e, en(&e, "cinco"));
    assert_eq!(a.Base.wNumbering.0, 2);
    assert_eq!((a.wNumberingStart, b.wNumberingStart), (4, 4), "la racha cuenta desde el primero");
    assert_eq!(a.wNumberingStyle.0, 0x200);
    // Otra racha, con parentesis.
    let c = parrafo(&e, en(&e, "uno"));
    assert_eq!((c.wNumberingStart, c.wNumberingStyle.0), (1, 0));
    cerrar(e);
}

#[test]
fn inicio_deja_el_cursor_detras_de_la_marca_del_titulo() {
    let mut e = abrir("# Titulo\nfin");
    let fin = en(&e, "Titulo") + 6;
    elegir(e.edit, fin, fin);
    pulsar(&mut e, VK_HOME.0);
    assert_eq!(seleccion(e.edit), (2, 2));
    // Escribir ahi sigue siendo titulo.
    teclear(&mut e, 'X');
    assert_eq!(markdown(&e), "# XTitulo\nfin");
    cerrar(e);
}

#[test]
fn retroceso_tras_la_ultima_letra_de_una_negrita_no_deja_asteriscos() {
    let mut e = abrir("a **b** c");
    let tras_b = en(&e, "b") + 1;
    elegir(e.edit, tras_b, tras_b);
    pulsar(&mut e, VK_BACK.0);
    assert_eq!(markdown(&e), "a  c");
    assert_eq!(seleccion(e.edit), (2, 2));
    // Caso negativo: con mas letras, la negrita se queda.
    let mut e2 = abrir("**ab**");
    elegir(e2.edit, 4, 4);
    pulsar(&mut e2, VK_BACK.0);
    assert_eq!(markdown(&e2), "**a**");
    cerrar(e2);
    cerrar(e);
}

#[test]
fn retroceso_al_principio_de_una_lista_la_vuelve_parrafo() {
    let mut e = abrir("uno\n- dos");
    let p = en(&e, "dos");
    elegir(e.edit, p, p);
    pulsar(&mut e, VK_BACK.0);
    assert_eq!(markdown(&e), "uno\ndos");
    pulsar(&mut e, VK_BACK.0);
    assert_eq!(markdown(&e), "unodos");
    cerrar(e);
}

#[test]
fn escribir_encima_de_lo_elegido_borra_solo_lo_que_se_ve() {
    let mut e = abrir("a **bold** c");
    let n = leer(e.edit).encode_utf16().count();
    elegir(e.edit, en(&e, "ld"), n);
    teclear(&mut e, 'x');
    assert_eq!(markdown(&e), "a **box**");
    cerrar(e);
}

#[test]
fn cortar_no_deja_marcas_sueltas() {
    let mut e = abrir("a **bold** c");
    elegir(e.edit, en(&e, "bold"), en(&e, "bold") + 4);
    cortar(&mut e);
    assert_eq!(markdown(&e), "a  c");
    cerrar(e);
}

#[test]
fn corchetes_al_principio_se_vuelven_casilla_al_escribir_el_espacio() {
    let mut e = abrir("");
    escribir(&e, "[]");
    teclear(&mut e, ' ');
    teclear(&mut e, 'p');
    assert_eq!(markdown(&e), "- [ ] p");
    // Las demas ya son Markdown: `# ` es un titulo al momento.
    let mut e2 = abrir("");
    for c in "# Hola".chars() {
        teclear(&mut e2, c);
    }
    pintar(&mut e2, None);
    assert_eq!(markdown(&e2), "# Hola");
    assert!(escondida(&e2, 0));
    assert!(efectos(&e2, 3).Base.yHeight > BASE);
    // Caso negativo: en mitad de una frase no se convierte.
    let mut e3 = abrir("ver ");
    escribir(&e3, "[]");
    teclear(&mut e3, ' ');
    assert_eq!(markdown(&e3), "ver [] ");
    cerrar(e3);
    cerrar(e2);
    cerrar(e);
}

#[test]
fn ctrl_b_sin_nada_elegido_deja_la_negrita_para_lo_que_se_escriba() {
    let mut e = abrir("hola ");
    elegir(e.edit, 5, 5);
    formato(&mut e, "**");
    // Nada en el texto todavia: ni asteriscos sueltos.
    assert_eq!(markdown(&e), "hola ");
    teclear(&mut e, 'x');
    teclear(&mut e, 'y');
    assert_eq!(markdown(&e), "hola **xy**");
    // Caso negativo: si el cursor se fue a otro sitio, ya no vale.
    let mut e2 = abrir("ab");
    elegir(e2.edit, 2, 2);
    formato(&mut e2, "**");
    elegir(e2.edit, 0, 0);
    teclear(&mut e2, 'z');
    assert_eq!(markdown(&e2), "zab");
    cerrar(e2);
    cerrar(e);
}

#[test]
fn la_barra_flotante_sale_al_elegir_y_pone_y_quita_el_formato() {
    let mut e = abrir("hola mundo\notra");
    // Sin nada elegido no sale.
    elegir(e.edit, 2, 2);
    actualizar_barra_flotante(&mut e);
    assert!(e.vivo.barra_caja.is_none());
    elegir(e.edit, 5, 10);
    actualizar_barra_flotante(&mut e);
    let caja = e.vivo.barra_caja.expect("sale la barra");
    // Encima de lo elegido.
    let mut p = POINT::default();
    enviar(e.edit, EM_POSFROMCHAR, &mut p as *mut _ as usize, 5);
    // SAFETY: coordenadas de ventanas propias.
    unsafe {
        let _ = ClientToScreen(e.edit, &mut p);
        let _ = ScreenToClient(e.marco, &mut p);
    }
    assert!(caja.abajo() <= p.y || caja.y > p.y, "no tapa el renglon elegido");
    clic_barra(&mut e, BotonBarra::Negrita, &mut |_| true);
    assert_eq!(markdown(&e), "hola **mundo**\notra");
    // Sigue elegido lo mismo (lo que se ve): y su boton, pulsado.
    actualizar_barra_flotante(&mut e);
    let pulsados = barra_flotante::VISTA.with(|v| v.borrow().as_ref().unwrap().puestos.clone());
    assert!(pulsados.contains(&BotonBarra::Negrita));
    clic_barra(&mut e, BotonBarra::Negrita, &mut |_| true);
    assert_eq!(markdown(&e), "hola mundo\notra");
    clic_barra(&mut e, BotonBarra::Tachado, &mut |_| true);
    clic_barra(&mut e, BotonBarra::Codigo, &mut |_| true);
    assert_eq!(markdown(&e), "hola ~~`mundo`~~\notra");
    clic_barra(&mut e, BotonBarra::QuitarFormato, &mut |_| true);
    assert_eq!(markdown(&e), "hola mundo\notra");
    // «Aa ▾» y la lista abren su menu; elegir pone el bloque.
    clic_barra(&mut e, BotonBarra::Formato, &mut |_| true);
    assert_eq!(e.abierto, Some(Abierto::Vivo));
    let ids: Vec<u16> = menu::VISTA.with(|v| v.borrow().as_ref().unwrap().menu.entradas.iter().map(|x| x.id).collect());
    assert_eq!(ids, vec![C_TEXTO_NORMAL, C_T1, C_T2, C_T3]);
    cerrar_menu(&mut e);
    super::super::comando(&mut e, C_T2, &mut |_| true);
    assert_eq!(markdown(&e), "## hola mundo\notra");
    super::super::comando(&mut e, C_TEXTO_NORMAL, &mut |_| true);
    assert_eq!(markdown(&e), "hola mundo\notra");
    elegir(e.edit, 0, 15);
    super::super::comando(&mut e, C_NUMERADA, &mut |_| true);
    assert_eq!(markdown(&e), "1. hola mundo\n2. otra");
    super::super::comando(&mut e, C_NUMERADA, &mut |_| true);
    assert_eq!(markdown(&e), "hola mundo\notra");
    // Caso negativo: el subrayado no esta (Markdown no lo tiene).
    let d = barra_flotante::disponer(1.0);
    assert_eq!(d.botones.len(), 10);
    cerrar(e);
}

#[test]
fn el_emoji_se_mete_detras_de_lo_elegido() {
    let mut e = abrir("hola");
    elegir(e.edit, 0, 4);
    actualizar_barra_flotante(&mut e);
    clic_barra(&mut e, BotonBarra::Emoji, &mut |_| true);
    let n = menu::VISTA.with(|v| v.borrow().as_ref().unwrap().menu.entradas.len());
    assert_eq!(n, EMOJIS.len());
    cerrar_menu(&mut e);
    comando(&mut e, C_EMOJI + 6);
    assert_eq!(markdown(&e), "hola🙏");
    cerrar(e);
}

#[test]
fn el_enlace_pide_su_direccion_y_la_completa() {
    let mut e = abrir("ver el plano");
    elegir(e.edit, 4, 12);
    super::super::comando(&mut e, C_ENLACE, &mut |_| true);
    let (h, a, b) = e.vivo.enlace.expect("sale el cuadro");
    assert_eq!((a, b), (4, 12));
    // SAFETY: control propio de la prueba.
    unsafe {
        let _ = SetWindowTextW(h, &HSTRING::from("x.es/p"));
    }
    acabar_enlace(&mut e, true);
    assert_eq!(markdown(&e), "ver [el plano](https://x.es/p)");
    assert!(e.vivo.enlace.is_none());
    // Sin nada elegido, la direccion hace de texto.
    let fin = leer(e.edit).encode_utf16().count();
    poner_enlace(&mut e, fin, fin, "https://y.es");
    assert!(markdown(&e).ends_with("[https://y.es](https://y.es)"));
    // Casos negativos: Esc no pone nada; un correo es correo.
    elegir(e.edit, 0, 3);
    empezar_enlace(&mut e);
    acabar_enlace(&mut e, false);
    assert!(!markdown(&e).starts_with('['));
    assert_eq!(completar_direccion("a@b.es"), "mailto:a@b.es");
    assert_eq!(completar_direccion("pixpin:hoja=P/H"), "pixpin:hoja=P/H");
    cerrar(e);
}

fn con_ajustes(md: &str, fichero: &std::path::Path) -> Estado {
    let mut p = pedido(md);
    p.integracion = Integracion {
        ajustes_vista: Some(fichero.to_path_buf()),
        clave_vista: Some(Box::new(|| "pins/notas/plan.md".to_string())),
        ..Default::default()
    };
    abrir_con(p, false, (1000, 760))
}

#[test]
fn la_letra_y_el_tamano_son_de_la_vista_y_no_cambian_el_markdown() {
    let dir = std::env::temp_dir().join(format!("pixpin-vista-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fichero = dir.join("vista-de-notas.txt");
    let _ = std::fs::remove_file(&fichero);
    let md = "# Plan\nTexto **fuerte**.\n- [ ] tarea";
    let mut e = con_ajustes(md, &fichero);
    comando(&mut e, C_LETRA + 1);
    comando(&mut e, C_TAMANO + 2);
    comando(&mut e, C_LETRA_TITULOS + 2);
    // El texto no cambia: nada que guardar.
    assert_eq!(markdown(&e), md);
    assert_eq!(markdown(&e), e.guardado);
    // La letra y el tamano, en el texto del control y en el fichero.
    let f = efectos(&e, en(&e, "Texto"));
    assert_eq!(f.Base.yHeight, 18 * 15);
    let cara = String::from_utf16_lossy(&f.Base.szFaceName).trim_end_matches('\0').to_string();
    assert_eq!(cara, letras::nombre_gdi("Nunito", "x"));
    // El titulo, en proporcion.
    assert!(efectos(&e, en(&e, "Plan")).Base.yHeight > 18 * 15 * 2);
    let guardado = std::fs::read_to_string(&fichero).unwrap();
    assert_eq!(guardado, "*\tNunito\tLilita One\t18\n");
    // Ctrl+: de uno en uno (Ctrl+rueda ya no: es el zoom de la pagina).
    comando(&mut e, C_LETRA_MAS);
    assert_eq!(e.vivo.vista.px, 19);
    // Solo en esta nota.
    comando(&mut e, C_SOLO_ESTA_NOTA);
    let guardado = std::fs::read_to_string(&fichero).unwrap();
    assert!(guardado.contains("pins/notas/plan.md\tNunito\tLilita One\t19"), "{guardado}");
    // Lo que se cambie ahora es solo de esta nota: la general se queda en 19.
    comando(&mut e, C_TAMANO + 3);
    let guardado = std::fs::read_to_string(&fichero).unwrap();
    assert!(guardado.contains("pins/notas/plan.md\tNunito\tLilita One\t21"), "{guardado}");
    assert!(guardado.contains("*\tNunito\tLilita One\t19"), "{guardado}");
    cerrar(e);
    // Al abrirla otra vez se ve igual; otra nota, con la general.
    let e = con_ajustes(md, &fichero);
    assert_eq!((e.vivo.vista.px, e.vivo.de_la_nota), (21, true));
    assert_eq!(e.estilos.tamano, 21 * 15);
    cerrar(e);
    let mut p = pedido("otra");
    p.integracion = Integracion {
        ajustes_vista: Some(fichero.clone()),
        clave_vista: Some(Box::new(|| "otra.md".to_string())),
        ..Default::default()
    };
    let e = abrir_con(p, false, (1000, 760));
    assert_eq!((e.vivo.vista.px, e.vivo.de_la_nota), (19, false));
    cerrar(e);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn abrir_y_cerrar_una_nota_con_todo_no_cambia_el_markdown() {
    let md = "# Obra\n## Fases\nTexto con **negrita**, *cursiva*, ~~tachado~~, `codigo` y [un enlace](https://x.es).\n\n- uno\n  - anidado\n- [x] hecha\n- [ ] pendiente\n1. primero\n2. segundo\n> una cita\n---\nfin";
    let mut e = abrir(md);
    pintar(&mut e, None);
    assert_eq!(markdown(&e), md);
    assert_eq!(e.guardado, md);
    // Poner y quitar formato vuelve a lo mismo.
    let p = en(&e, "fin");
    elegir(e.edit, p, p + 3);
    formato(&mut e, "**");
    formato(&mut e, "**");
    assert_eq!(markdown(&e), md);
    cerrar(e);
}

#[test]
fn pellizcar_acerca_la_pagina_entera_y_no_cambia_la_letra() {
    let mut nota = String::from("# Titulo\n");
    for i in 0..40 {
        nota.push_str(&format!("Renglon {i} de la nota\n"));
    }
    let mut e = abrir(&nota);
    ir_a(&e, 0);
    let px = e.vivo.vista.px;
    let renglon = |e: &Estado| {
        let (a, b) = (en(e, "Renglon 1 "), en(e, "Renglon 2 "));
        let (mut p, mut q) = (POINT::default(), POINT::default());
        enviar(e.edit, EM_POSFROMCHAR, &mut p as *mut _ as usize, a as isize);
        enviar(e.edit, EM_POSFROMCHAR, &mut q as *mut _ as usize, b as isize);
        q.y - p.y
    };
    let alto = renglon(&e);
    // El pellizco llega como Ctrl+rueda: dos pasos hacia arriba, un 21 %.
    let pellizco = MSG {
        hwnd: e.edit,
        message: WM_MOUSEWHEEL,
        wParam: WPARAM((240usize << 16) | 0x0008),
        ..Default::default()
    };
    assert!(super::rueda(&mut e, &pellizco));
    assert_eq!(zoom_de(&e.vivo), 1210);
    // La letra elegida no cambia: crece la pagina entera.
    assert_eq!(e.vivo.vista.px, px);
    let grande = renglon(&e);
    assert!((grande - alto * 121 / 100).abs() <= 3, "{alto} -> {grande}");
    // Las marcas siguen escondidas y el texto es el mismo.
    assert_eq!(markdown(&e), e.guardado);
    assert!(escondida(&e, 0), "el # del titulo sigue escondido");
    // Ctrl 0: sin zoom otra vez.
    poner_zoom(&mut e, 1000);
    assert_eq!(e.vivo.zoom, 0);
    assert!((renglon(&e) - alto).abs() <= 1);
    // Los topes.
    assert_eq!(zoom_tras(1000, -40.0), ZOOM_MIN);
    assert_eq!(zoom_tras(1000, 40.0), ZOOM_MAX);
    cerrar(e);
}

#[test]
fn la_rueda_desplaza_por_pixeles_y_no_por_renglones_enteros() {
    let mut nota = String::new();
    for i in 0..80 {
        nota.push_str(&format!("Renglon {i} de la nota\n"));
    }
    let mut e = abrir(&nota);
    ir_a(&e, 0);
    let paso = paso_de_rueda(&e);
    let abajo = MSG {
        hwnd: e.edit,
        message: WM_MOUSEWHEEL,
        wParam: WPARAM((-120i16 as u16 as usize) << 16),
        ..Default::default()
    };
    assert!(super::rueda(&mut e, &abajo));
    let y = desplazado(&e);
    assert!((y - paso).abs() <= 2, "{y} frente a {paso}");
    // Un panel tactil: pasos pequenos, de pocos pixeles.
    let fino = MSG {
        wParam: WPARAM((-12i16 as u16 as usize) << 16),
        ..abajo
    };
    assert!(super::rueda(&mut e, &fino));
    assert!(desplazado(&e) > y && desplazado(&e) - y <= paso / 5 + 2);
    // La animacion: pasos hasta llegar, sin pasarse.
    e.vivo.objetivo = Some(y + 300);
    for _ in 0..40 {
        paso_de_prueba(&mut e);
    }
    assert_eq!(desplazado(&e), y + 300);
    assert!(e.vivo.objetivo.is_none());
    // Caso negativo: arriba del todo no se sube mas.
    ir_a(&e, 0);
    let arriba = MSG {
        wParam: WPARAM((120usize) << 16),
        ..abajo
    };
    super::rueda(&mut e, &arriba);
    assert_eq!(desplazado(&e), 0);
    cerrar(e);
}

fn paso_de_prueba(e: &mut Estado) {
    paso(e);
}

#[test]
fn la_barra_fina_se_coge_y_lleva_la_nota() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let mut nota = String::new();
    for i in 0..120 {
        nota.push_str(&format!("Renglon {i}\n"));
    }
    let mut e = abrir(&nota);
    let _ = muestra(&e);
    let (pista, asidero) = imagenes::BARRA.with(|b| b.get()).expect("hay barra");
    let x = (pista.left + pista.right) / 2;
    let edit = e.edit;
    let m = |tipo: u32, y: i32| MSG {
        hwnd: edit,
        message: tipo,
        lParam: LPARAM(((y as u16 as isize) << 16) | (x as u16) as isize),
        ..Default::default()
    };
    assert!(raton_barra(&mut e, &m(WM_LBUTTONDOWN, asidero.top + 2)));
    assert!(raton_barra(&mut e, &m(WM_MOUSEMOVE, pista.bottom)));
    assert!(raton_barra(&mut e, &m(WM_LBUTTONUP, pista.bottom)));
    assert_eq!(desplazado(&e), tope(&e), "al fondo");
    // Caso negativo: un clic lejos de la barra no es suyo.
    let lejos = MSG {
        lParam: LPARAM((10isize << 16) | 10),
        ..m(WM_LBUTTONDOWN, 10)
    };
    assert!(!raton_barra(&mut e, &lejos));
    cerrar(e);
}

#[test]
fn un_clic_en_la_casilla_pintada_la_marca() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let e = abrir("- [ ] comprar cemento\nfin");
    let _ = muestra(&e);
    // La casilla esta a la izquierda de su texto, en la sangria.
    let mut p = POINT::default();
    enviar(e.edit, EM_POSFROMCHAR, &mut p as *mut _ as usize, en(&e, "comprar") as isize);
    let r = renglon_px(&e);
    let n = imagenes::casilla_en(e.edit, p.x - 8 * e.ppp / 96 - 4, p.y + r / 2);
    assert_eq!(n, Some(0));
    alternar_casilla(&e, 0);
    assert_eq!(markdown(&e), "- [x] comprar cemento\nfin");
    // Caso negativo: en el texto no hay casilla.
    assert_eq!(imagenes::casilla_en(e.edit, p.x + 40, p.y + r / 2), None);
    cerrar(e);
}

// ---------------------------------------------------------------------------
// Las fotos: el hueco siempre del alto de la foto

const VIVA: &str = "pixpin:files/guardados/pc/p1/notas/vivo-K7Q2ABCDEF.png";

fn con_foto(md: &str, an: u32, al: u32) -> Estado {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    std::mem::forget(_com);
    let foto = foto_de_prueba(&format!("viva-{an}x{al}.png"), an, al);
    let mut p = pedido(md);
    p.resolver = Box::new(move |_: &str| Some(foto.clone()));
    abrir_con(p, false, (1000, 900))
}

fn y_de(e: &Estado, pos: usize) -> i32 {
    let mut p = POINT::default();
    enviar(e.edit, EM_POSFROMCHAR, &mut p as *mut _ as usize, pos as isize);
    p.y
}

/// La foto `i` no pisa el texto de los renglones de alrededor.
fn sin_pisar(e: &Estado) {
    let t = leer(e.edit);
    let ls = md_vivo::lineas(&t);
    let (linea, _, caja, _) = imagenes::puesta(0).unwrap();
    if linea > 0 {
        let arriba = ls[linea - 1];
        assert!(caja.top >= y_de(e, arriba.desde) + renglon_px(e) - 4, "pisa el renglon de encima");
    }
    if let Some(abajo) = ls.get(linea + 1) {
        assert!(caja.bottom <= y_de(e, abajo.desde), "pisa el renglon de debajo: {} > {}", caja.bottom, y_de(e, abajo.desde));
    }
}

#[test]
fn escribir_encima_de_una_pagina_viva_no_la_pinta_sobre_el_texto() {
    let mut e = con_foto(&format!("arriba\n![Hoja]({VIVA})\nabajo"), 900, 500);
    let _ = muestra(&e);
    sin_pisar(&e);
    // Un renglon nuevo arriba, sin repintar el formato todavia (los 60 ms):
    // la foto va donde esta su renglon ahora, no donde estaba.
    elegir(e.edit, 0, 0);
    escribir(&e, "nuevo\n");
    let _ = muestra(&e);
    let (linea, ..) = imagenes::puesta(0).unwrap();
    assert_eq!(linea, 2);
    pintar(&mut e, None);
    let _ = muestra(&e);
    sin_pisar(&e);
    cerrar(e);
}

#[test]
fn el_asa_cambia_el_tamano_en_vivo_y_el_hueco_la_sigue() {
    let mut e = con_foto(&format!("arriba\n![Hoja]({VIVA})\nabajo"), 900, 500);
    let t = leer(e.edit);
    let fin = md_vivo::lineas(&t)[1].hasta;
    elegir(e.edit, fin, fin);
    pintar(&mut e, None);
    let _ = muestra(&e);
    let (_, _, caja, _) = imagenes::puesta(0).unwrap();
    let pulsar_raton = |e: &mut Estado, m: u32, x: i32, y: i32| {
        let msg = MSG {
            hwnd: e.edit,
            message: m,
            lParam: LPARAM((((y as u16) as isize) << 16) | (x as u16) as isize),
            ..Default::default()
        };
        fotos::raton(e, &msg)
    };
    assert!(pulsar_raton(&mut e, WM_LBUTTONDOWN, caja.right - 3, caja.bottom - 3));
    let centro = (caja.left + caja.right) / 2;
    // A la mitad: mientras se arrastra, la foto y su hueco ya miden eso.
    assert!(pulsar_raton(&mut e, WM_MOUSEMOVE, centro + (caja.right - caja.left) / 4, caja.bottom));
    let _ = muestra(&e);
    let (_, _, ahora, _) = imagenes::puesta(0).unwrap();
    assert!(((ahora.right - ahora.left) - (caja.right - caja.left) / 2).abs() <= 2);
    sin_pisar(&e);
    assert!(pulsar_raton(&mut e, WM_LBUTTONUP, centro + (caja.right - caja.left) / 4, caja.bottom));
    pintar(&mut e, None);
    let _ = muestra(&e);
    sin_pisar(&e);
    let md = markdown(&e);
    let f = pixpin_docs::md_imagen::leer(md.lines().nth(1).unwrap()).unwrap();
    assert!(f.ancho.is_some());
    // Proporcional: el alto, en la misma proporcion que el ancho.
    let (_, _, c, _) = imagenes::puesta(0).unwrap();
    let (an, al) = (c.right - c.left, c.bottom - c.top);
    assert!((al * 900 - an * 500).abs() <= 900 * 2, "{an}x{al}");
    cerrar(e);
}

#[test]
fn soltar_el_asa_deja_la_foto_tal_cual_se_solto_sin_rebote() {
    let mut e = con_foto(&format!("arriba\n![Hoja]({VIVA})\nabajo"), 900, 500);
    let fin = md_vivo::lineas(&leer(e.edit))[1].hasta;
    elegir(e.edit, fin, fin);
    pintar(&mut e, None);
    let _ = muestra(&e);
    let (_, _, caja, _) = imagenes::puesta(0).unwrap();
    let raton = |e: &mut Estado, m: u32, x: i32, y: i32| {
        let msg = MSG {
            hwnd: e.edit,
            message: m,
            lParam: LPARAM((((y as u16) as isize) << 16) | (x as u16) as isize),
            ..Default::default()
        };
        fotos::raton(e, &msg)
    };
    let medida = |r: RECT| (r.right - r.left, r.bottom - r.top);
    let abajo = |e: &Estado| y_de(e, md_vivo::lineas(&leer(e.edit))[2].desde);
    assert!(raton(&mut e, WM_LBUTTONDOWN, caja.right - 3, caja.bottom - 3));
    let centro = (caja.left + caja.right) / 2;
    let x = centro + (caja.right - caja.left) / 3 + 1;
    assert!(raton(&mut e, WM_MOUSEMOVE, x, caja.bottom));
    let _ = muestra(&e);
    let arrastrada = medida(imagenes::puesta(0).unwrap().2);
    let abajo_arrastrando = abajo(&e);
    // Se suelta unos pixeles mas alla: queda lo ultimo que se enseno.
    assert!(raton(&mut e, WM_LBUTTONUP, x + 3, caja.bottom));
    // El primer fotograma tras soltar, sin pintar nada mas.
    let _ = muestra(&e);
    assert_eq!(medida(imagenes::puesta(0).unwrap().2), arrastrada, "ni se achica ni crece al soltar");
    assert_eq!(abajo(&e), abajo_arrastrando, "el hueco tampoco");
    // Ni con el formato entero de despues (el respiro de siempre).
    pintar(&mut e, None);
    let _ = muestra(&e);
    assert_eq!(medida(imagenes::puesta(0).unwrap().2), arrastrada);
    assert_eq!(abajo(&e), abajo_arrastrando);
    // Y lo escrito es lo que se ve.
    let f = pixpin_docs::md_imagen::leer(markdown(&e).lines().nth(1).unwrap()).unwrap();
    assert_eq!((f.ancho.unwrap() as f32 * e.ppp as f32 / 96.0) as i32, arrastrada.0);
    // Caso negativo: un clic en el asa sin arrastrar no cambia nada.
    let antes = markdown(&e);
    let (_, _, caja, _) = imagenes::puesta(0).unwrap();
    assert!(raton(&mut e, WM_LBUTTONDOWN, caja.right - 3, caja.bottom - 3));
    assert!(raton(&mut e, WM_LBUTTONUP, caja.right - 3, caja.bottom - 3));
    assert_eq!(markdown(&e), antes);
    cerrar(e);
}

#[test]
fn una_hoja_pequena_se_agranda_con_el_asa_hasta_la_columna_y_no_mas() {
    let e = con_foto(&format!("![Hoja|600]({VIVA})\nfin"), 200, 100);
    let _ = muestra(&e);
    let (_, _, c, _) = imagenes::puesta(0).unwrap();
    assert_eq!(c.right - c.left, 600 * e.ppp / 96, "con el ancho puesto, se agranda");
    cerrar(e);
    // Caso negativo: sin ancho puesto, una pequena se ve a su tamano.
    let e = con_foto(&format!("![Hoja]({VIVA})\nfin"), 200, 100);
    let _ = muestra(&e);
    assert_eq!(imagenes::puesta(0).unwrap().2.right - imagenes::puesta(0).unwrap().2.left, 200);
    cerrar(e);
}

// ---------------------------------------------------------------------------
// Muestras en PNG, a mano:
// `PIXPIN_MUESTRA=<carpeta> cargo test -p pixpin-notas wysiwyg::pruebas::muestra_ -- --ignored`

const NOTA: &str = "# Visita de obra\n\
Revisión del **segundo piso** con el *residente*. Queda ~~pendiente~~ el `curado` y ver [el plano](https://x.es).\n\
\n\
## Tareas\n\
- [x] Pedir la grúa\n\
- [ ] Comprar cemento\n\
1. Encofrar\n\
2. Hormigonar\n\
- Avisar al vecino\n\
\x20\x20- Del ruido\n\
> Sin permiso no se corta la calle.\n\
---\n\
Fin del parte.";

fn muestra_de_nota(claro: bool, elegir_algo: bool, nombre: &str) {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let mut e = abrir_con(pedido(NOTA), claro, (900, 760));
    if elegir_algo {
        let a = en(&e, "segundo piso");
        elegir(e.edit, a, a + 12);
        pintar(&mut e, None);
        actualizar_barra_flotante(&mut e);
    } else {
        // El cursor en el titulo: tampoco se ven sus marcas.
        elegir(e.edit, 3, 3);
        pintar(&mut e, None);
    }
    ir_a(&e, 0);
    guardar_png(&muestra(&e), nombre);
    cerrar(e);
}

#[test]
#[ignore]
fn muestra_barra_flotante() {
    muestra_de_nota(false, true, "nota-md-barra-flotante.png");
    muestra_de_nota(true, true, "nota-md-barra-flotante-clara.png");
}

#[test]
#[ignore]
fn muestra_sin_marcas() {
    muestra_de_nota(false, false, "nota-md-sin-marcas.png");
    muestra_de_nota(true, false, "nota-md-sin-marcas-clara.png");
}

#[test]
#[ignore]
fn muestra_otra_letra_y_tamano() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    for (cuerpo, titulos, px, nombre) in [
        ("Nunito", "Lilita One", 18, "nota-md-letra-nunito-18.png"),
        ("Caveat", "Caveat", 21, "nota-md-letra-caveat-21.png"),
        ("Courier New", "Courier New", 14, "nota-md-letra-courier-14.png"),
    ] {
        let mut e = abrir_con(pedido(NOTA), false, (900, 760));
        aplicar_vista(&mut e, Vista { cuerpo: cuerpo.into(), titulos: titulos.into(), px });
        elegir(e.edit, 0, 0);
        pintar(&mut e, None);
        ir_a(&e, 0);
        guardar_png(&muestra(&e), nombre);
        assert_eq!(markdown(&e), NOTA);
        cerrar(e);
    }
}

#[test]
#[ignore]
fn muestra_pagina_viva_redimensionada() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let md = format!("# Planta\nLa hoja del lienzo, viva:\n![Planta baja]({VIVA})\nDebajo sigue el texto, sin que la hoja lo pise.\nOtro renglon.");
    let mut e = con_foto(&md, 1200, 800);
    let t = leer(e.edit);
    let fin = md_vivo::lineas(&t)[2].hasta;
    elegir(e.edit, fin, fin);
    pintar(&mut e, None);
    ir_a(&e, 0);
    guardar_png(&muestra(&e), "nota-md-viva-antes.png");
    fotos::poner_ancho(&e, 2, Some(380));
    pintar(&mut e, None);
    ir_a(&e, 0);
    guardar_png(&muestra(&e), "nota-md-viva-despues.png");
    cerrar(e);
}
