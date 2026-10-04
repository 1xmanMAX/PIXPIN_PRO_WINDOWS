//! Pruebas del editor con ventanas de verdad pero **ocultas** (nunca se
//! ensenan): el `RichEdit`, sus tablas y el marco, sin pantalla.

use super::*;

fn u(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

#[test]
fn la_diferencia_toca_solo_lo_que_cambia() {
    // Marcar una casilla es cambiar una letra.
    assert_eq!(diferencia(&u("- [ ] a"), &u("- [x] a")), (3, 4, 4));
    // Poner un titulo es meter dos al principio.
    assert_eq!(diferencia(&u("hola"), &u("# hola")), (0, 0, 2));
    // Quitar la negrita, cuatro menos.
    assert_eq!(diferencia(&u("x **y** z"), &u("x y z")), (2, 7, 3));
}

#[test]
fn la_diferencia_de_textos_iguales_o_repetidos_no_se_sale() {
    assert_eq!(diferencia(&u("aaa"), &u("aaa")), (3, 3, 3));
    assert_eq!(diferencia(&u("aa"), &u("aaaa")), (2, 2, 4));
    assert_eq!(diferencia(&u(""), &u("x")), (0, 0, 1));
}

#[test]
fn solo_se_abren_paginas_y_correo() {
    assert!(es_direccion("https://x.es/p"));
    assert!(es_direccion("mailto:a@b.es"));
    assert!(!es_direccion("file:///C:/Windows/calc.exe"));
    assert!(!es_direccion("C:\\a.exe"));
    assert!(!es_direccion("https://x.es con espacio"));
}

#[test]
fn el_nombre_de_la_copia_no_lleva_letras_prohibidas() {
    assert_eq!(sin_prohibidas("Obra: planta 1/2?"), "Obra_ planta 1_2_");
    assert_eq!(sin_prohibidas("fin. "), "fin");
}

fn estilos() -> Estilos {
    Estilos {
        tema: Tema::oscuro(),
        letras: Letras {
            cuerpo: "Segoe UI".into(),
            titulos: "Georgia".into(),
        },
        margen: 300,
        tamano: BASE,
    }
}

#[test]
fn una_marca_se_esconde_siempre_tambien_en_el_renglon_del_cursor() {
    let s = estilos();
    for activo in [false, true] {
        for estilo in [
            Estilo::Marca,
            Estilo::Numero,
            Estilo::Casilla { hecha: true },
            Estilo::Regla,
        ] {
            assert_eq!(
                formato_de(estilo, activo, &s).Base.dwEffects,
                CFE_HIDDEN,
                "{estilo:?}"
            );
        }
    }
    // Caso negativo: el texto de una negrita no se esconde.
    assert_eq!(
        formato_de(Estilo::Negrita, true, &s).Base.dwEffects.0 & CFE_HIDDEN.0,
        0
    );
}

#[test]
fn la_vineta_la_pone_windows_siempre_y_respeta_la_columna() {
    let s = estilos();
    let fuera = parrafo_de(Estilo::Vineta, false, &s).unwrap();
    assert_eq!(fuera.Base.wNumbering, PFN_BULLET);
    assert_eq!(fuera.Base.dxStartIndent, 300 + SANGRIA / 2);
    assert_eq!(
        parrafo_de(Estilo::Vineta, true, &s)
            .unwrap()
            .Base
            .wNumbering,
        PFN_BULLET
    );
    assert!(parrafo_de(Estilo::Negrita, false, &s).is_none());
}

#[test]
fn los_titulos_van_en_la_letra_de_titulos() {
    let s = estilos();
    let f = formato_de(Estilo::Titulo(1), false, &s);
    assert_eq!(
        String::from_utf16_lossy(&f.Base.szFaceName).trim_end_matches('\0'),
        "Georgia"
    );
    assert!(f.Base.yHeight > BASE);
    // El texto de una foto se esconde fuera del cursor, como una marca.
    assert_eq!(
        formato_de(Estilo::Imagen, false, &s).Base.dwEffects,
        CFE_HIDDEN
    );
}

// ---------------------------------------------------------------------------
// Con la ventana entera, oculta

fn rotulos() -> Rotulos {
    Rotulos {
        nueva: "Nota nueva".into(),
        sufijo: "Nota".into(),
        titulo1: "Título 1".into(),
        titulo2: "Título 2".into(),
        titulo3: "Título 3".into(),
        lista: "Lista con viñetas".into(),
        numerada: "Lista numerada".into(),
        casilla: "Casillas".into(),
        cita: "Cita".into(),
        bloque: "Bloque de código".into(),
        raya: "Separador".into(),
        tabla: "Tabla".into(),
        imagen: "Imagen".into(),
        fecha: "Fecha de hoy".into(),
        compartir: "Compartir".into(),
        pista_barra: "Escribe / en la nota para ver más".into(),
        boton_fila_mas: "+ Fila".into(),
        boton_fila_menos: "− Fila".into(),
        boton_columna_mas: "+ Columna".into(),
        boton_columna_menos: "− Columna".into(),
        meses: "ene feb mar abr may jun jul ago sept oct nov dic".into(),
        ..Default::default()
    }
}

pub(crate) fn pedido(texto: &str) -> Pedido {
    Pedido {
        texto: texto.into(),
        rotulos: rotulos(),
        nombre_de_fichero: None,
        colocacion: None,
        resolver: Box::new(|r: &str| Some(PathBuf::from(r))),
        adjuntar: Box::new(|_: &Path| None),
        compartir: None,
        integracion: Default::default(),
        comentarios: Default::default(),
    }
}

pub(crate) fn abrir(texto: &str) -> Estado {
    abrir_con(pedido(texto), false, (1000, 760))
}

pub(crate) fn abrir_con(p: Pedido, claro: bool, tamano: (i32, i32)) -> Estado {
    montar(
        p,
        Opciones {
            oculto: true,
            tamano: Some(tamano),
            claro: Some(claro),
        },
    )
    .expect("la ventana oculta se monta")
}

pub(crate) fn escribir(e: &Estado, s: &str) {
    enviar(
        e.edit,
        EM_REPLACESEL,
        1,
        ancho_nulo(&s.replace('\n', "\r")).as_ptr() as isize,
    );
}

const CON_TABLA: &str = "# Plan\nAntes de la tabla.\n| Objetivo | Tecnica |\n|:---|:---|\n| OE1 | **Pareto** |\n| OE2 | Likert 1\\|5 |\nDespues.\n";

#[test]
fn abrir_y_leer_una_nota_con_tabla_da_el_mismo_markdown() {
    let e = abrir(CON_TABLA);
    assert_eq!(markdown(&e), CON_TABLA);
    // Es una tabla de verdad del control, no texto con barras.
    let crudo = leer(e.edit);
    assert_eq!(md_tabla::tablas_en_control(&crudo).len(), 1);
    assert!(!crudo.contains("|:---|"));
    desmontar(e);
}

#[test]
fn la_alineacion_de_las_columnas_se_conserva() {
    let md = "| a | b | c |\n|:---|:---:|---:|\n| 1 | 2 | 3 |\n";
    let e = abrir(md);
    assert_eq!(markdown(&e), md);
    desmontar(e);
}

#[test]
fn una_tabla_mal_escrita_se_guarda_como_la_escribe_el_movil_solo_si_se_toca() {
    // Sin espacios ni dos puntos: se lee igual, y lo guardado es la forma
    // del movil; abrir y cerrar no cuenta como cambio.
    let e = abrir("|a|b|\n|-|-|\n|1|2|\n");
    assert_eq!(e.guardado, "| a | b |\n|:---|:---|\n| 1 | 2 |\n");
    assert_eq!(markdown(&e), e.guardado);
    desmontar(e);
}

#[test]
fn una_tabla_nueva_se_guarda_como_la_del_movil_y_se_escribe_en_sus_celdas() {
    let mut e = abrir("hola");
    comando(&mut e, C_TABLA, &mut |_| true);
    escribir(&e, "Obra");
    // Tab pasa a la celda de al lado (lo hace el control).
    let (t, _, c) = celda_del_cursor(&e).unwrap();
    assert_eq!(c, 0);
    ir_a_celda(&e, t.desde, 0, 1);
    escribir(&e, "Plazo");
    assert_eq!(
        markdown(&e),
        "hola\n| Obra | Plazo |\n|:---|:---|\n|  |  |\n|  |  |\n"
    );
    desmontar(e);
}

#[test]
fn filas_y_columnas_se_ponen_y_se_quitan_desde_la_celda_del_cursor() {
    let mut e = abrir("| a | b |\n|:---|:---|\n| 1 | 2 |\n");
    let t = md_tabla::tablas_en_control(&leer(e.edit)).remove(0);
    ir_a_celda(&e, t.desde, 1, 0);
    operar_tabla(&mut e, OpTabla::FilaDebajo);
    assert_eq!(markdown(&e), "| a | b |\n|:---|:---|\n| 1 | 2 |\n|  |  |\n");
    // El cursor quedo en la fila nueva.
    assert_eq!(celda_del_cursor(&e).map(|(_, f, c)| (f, c)), Some((2, 0)));
    operar_tabla(&mut e, OpTabla::ColDerecha);
    assert_eq!(
        markdown(&e),
        "| a |  | b |\n|:---|:---|:---|\n| 1 |  | 2 |\n|  |  |  |\n"
    );
    operar_tabla(&mut e, OpTabla::QuitarCol);
    operar_tabla(&mut e, OpTabla::QuitarFila);
    assert_eq!(markdown(&e), "| a | b |\n|:---|:---|\n| 1 | 2 |\n");
    operar_tabla(&mut e, OpTabla::QuitarTabla);
    assert_eq!(markdown(&e), "");
    desmontar(e);
}

#[test]
fn fuera_de_una_tabla_sus_ordenes_no_hacen_nada() {
    let mut e = abrir("solo texto");
    operar_tabla(&mut e, OpTabla::FilaDebajo);
    operar_tabla(&mut e, OpTabla::QuitarTabla);
    assert_eq!(markdown(&e), "solo texto");
    assert!(celda_del_cursor(&e).is_none());
    desmontar(e);
}

#[test]
fn intro_en_la_ultima_fila_anade_otra_y_en_las_demas_baja() {
    let mut e = abrir("| a |\n|:---|\n| 1 |\n");
    let t = md_tabla::tablas_en_control(&leer(e.edit)).remove(0);
    ir_a_celda(&e, t.desde, 0, 0);
    assert!(intro_en_tabla(&mut e));
    assert_eq!(celda_del_cursor(&e).map(|(_, f, _)| f), Some(1));
    assert!(intro_en_tabla(&mut e));
    assert_eq!(markdown(&e), "| a |\n|:---|\n| 1 |\n|  |\n");
    desmontar(e);
}

#[test]
fn el_menu_de_la_barra_filtra_lo_tecleado_y_elige_con_intro() {
    let mut e = abrir("hola\n");
    escribir(&e, "/cas");
    actualizar_barra(&mut e);
    assert!(matches!(e.abierto, Some(Abierto::Barra(5))));
    let ids: Vec<u16> = menu::VISTA.with(|v| {
        v.borrow()
            .as_ref()
            .unwrap()
            .menu
            .entradas
            .iter()
            .map(|x| x.id)
            .collect()
    });
    assert_eq!(ids.len(), 1);
    elegir_del_menu(&mut e, &mut |_| true);
    assert_eq!(markdown(&e), "hola\n- [ ] ");
    assert!(e.abierto.is_none());
    desmontar(e);
}

#[test]
fn la_tabla_y_la_fecha_tambien_se_ponen_desde_la_barra() {
    let mut e = abrir("");
    escribir(&e, "/tabla");
    actualizar_barra(&mut e);
    elegir_del_menu(&mut e, &mut |_| true);
    assert_eq!(markdown(&e), "|  |  |\n|:---|:---|\n|  |  |\n|  |  |\n");
    desmontar(e);
    let mut e = abrir("");
    escribir(&e, "/hoy");
    actualizar_barra(&mut e);
    elegir_del_menu(&mut e, &mut |_| true);
    let (d, m, a) = md_comandos::dia_de(pixpin_shell::entorno::ahora_local_ms());
    let meses = "ene feb mar abr may jun jul ago sept oct nov dic"
        .split(' ')
        .nth(m as usize - 1)
        .unwrap();
    assert_eq!(markdown(&e), format!("{d} {meses} {a}"));
    desmontar(e);
}

#[test]
fn una_barra_en_mitad_de_la_frase_o_sin_resultados_no_abre_menu() {
    let mut e = abrir("el 12");
    escribir(&e, "/03");
    actualizar_barra(&mut e);
    assert!(e.abierto.is_none());
    escribir(&e, "\n/zzz");
    actualizar_barra(&mut e);
    assert!(e.abierto.is_none());
    desmontar(e);
}

#[test]
fn esc_cierra_el_menu_de_la_barra_y_no_vuelve_mientras_se_escribe() {
    let mut e = abrir("");
    escribir(&e, "/t");
    actualizar_barra(&mut e);
    assert!(e.abierto.is_some());
    let esc = MSG {
        hwnd: e.edit,
        message: WM_KEYDOWN,
        wParam: WPARAM(VK_ESCAPE.0 as usize),
        ..Default::default()
    };
    assert!(interceptar(&mut e, &esc, &mut |_| true));
    assert!(e.abierto.is_none());
    escribir(&e, "a");
    actualizar_barra(&mut e);
    assert!(e.abierto.is_none(), "volvio a salir tras Esc");
    desmontar(e);
}

#[test]
fn las_flechas_mueven_la_eleccion_del_menu() {
    let mut e = abrir("");
    escribir(&e, "/t");
    actualizar_barra(&mut e);
    let abajo = MSG {
        hwnd: e.edit,
        message: WM_KEYDOWN,
        wParam: WPARAM(VK_DOWN.0 as usize),
        ..Default::default()
    };
    assert!(interceptar(&mut e, &abajo, &mut |_| true));
    let elegida = menu::VISTA.with(|v| v.borrow().as_ref().unwrap().menu.elegida);
    assert_eq!(elegida, 1);
    desmontar(e);
}

#[test]
fn cambiar_el_titulo_desde_la_cabecera_escribe_el_primer_titulo() {
    let mut e = abrir("## Obra vieja\ntexto");
    empezar_titulo(&mut e);
    let h = e.titulo_edit.unwrap();
    // SAFETY: control de la prueba.
    unsafe {
        let _ = SetWindowTextW(h, &HSTRING::from("Casa Lima"));
    }
    acabar_titulo(&mut e, true);
    assert_eq!(markdown(&e), "## Casa Lima\ntexto");
    assert!(e.titulo_edit.is_none());
    // Con Esc no cambia.
    empezar_titulo(&mut e);
    acabar_titulo(&mut e, false);
    assert_eq!(markdown(&e), "## Casa Lima\ntexto");
    desmontar(e);
}

#[test]
fn los_botones_de_la_tabla_salen_con_el_cursor_dentro() {
    let mut e = abrir("hola\n| a |\n|:---|\n| 1 |\n");
    elegir(e.edit, 0, 0);
    actualizar_en_tabla(&mut e);
    assert!(VISTA.with(|v| {
        v.borrow()
            .as_ref()
            .unwrap()
            .disp
            .caja(Boton::FilaMas)
            .is_none()
    }));
    let t = md_tabla::tablas_en_control(&leer(e.edit)).remove(0);
    ir_a_celda(&e, t.desde, 1, 0);
    actualizar_en_tabla(&mut e);
    assert!(VISTA.with(|v| {
        v.borrow()
            .as_ref()
            .unwrap()
            .disp
            .caja(Boton::FilaMas)
            .is_some()
    }));
    desmontar(e);
}

/// Una foto de prueba en una carpeta temporal.
pub(crate) fn foto_de_prueba(nombre: &str, an: u32, al: u32) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pixpin-notas-foto-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut px = Vec::with_capacity((an * al * 4) as usize);
    for y in 0..al {
        for x in 0..an {
            let cielo = y < al * 3 / 5;
            let (r, g, b) = if cielo {
                (90 + (x * 60 / an) as u8, 150, 220)
            } else {
                (70, 120 + (y * 60 / al) as u8, 60)
            };
            px.extend_from_slice(&[r, g, b, 255]);
        }
    }
    let img = pixpin_codec::ImagenRgba {
        ancho: an,
        alto: al,
        pixeles: px,
    };
    let ruta = dir.join(nombre);
    std::fs::write(&ruta, pixpin_codec::imagen::codificar_png(&img).unwrap()).unwrap();
    ruta
}

#[test]
fn una_foto_se_ensena_en_su_hueco_y_el_markdown_no_cambia() {
    let ruta = foto_de_prueba("planta.png", 1600, 900);
    let md = "antes\n![planta](pixpin:files/guardados/pc/p1/notas/1-planta.png)\ndespues";
    let mut p = pedido(md);
    let r = ruta.clone();
    p.resolver = Box::new(move |s: &str| s.ends_with("1-planta.png").then(|| r.clone()));
    let e = abrir_con(p, false, (1000, 760));
    let puestas = imagenes::PUESTAS.with(|p| p.borrow().len());
    assert_eq!(puestas, 1);
    assert_eq!(markdown(&e), md);
    desmontar(e);
}

#[test]
fn una_foto_que_no_esta_se_queda_como_texto() {
    let mut p = pedido("![x](no-esta.png)\n");
    p.resolver = Box::new(|_: &str| None);
    let e = abrir_con(p, false, (1000, 760));
    assert_eq!(imagenes::PUESTAS.with(|p| p.borrow().len()), 0);
    desmontar(e);
}

#[test]
fn insertar_una_foto_la_pone_en_su_renglon_con_la_ruta_de_adjuntar() {
    let ruta = foto_de_prueba("alzado.png", 400, 300);
    let mut p = pedido("texto");
    p.adjuntar =
        Box::new(|_: &Path| Some("pixpin:files/guardados/pc/p1/notas/9-alzado.png".into()));
    let e = abrir_con(p, false, (1000, 760));
    // Lo que haria C_IMAGEN tras el dialogo.
    let md = "pixpin:files/guardados/pc/p1/notas/9-alzado.png".to_string();
    let _ = &ruta;
    insertar_renglon(&e, &format!("![alzado]({md})"));
    assert_eq!(markdown(&e), format!("texto\n![alzado]({md})"));
    desmontar(e);
}

// ---------------------------------------------------------------------------
// Muestras en PNG, a mano:
// `PIXPIN_MUESTRA=<carpeta> cargo test -p pixpin-notas muestra_ -- --ignored`

const NOTA_DE_MUESTRA: &str = "# Objetivos e indicadores\n\
Versión para la segunda asesoría. El **objetivo general** se parte en seis específicos:\n\
\n\
1. **OE1.** Diagnosticar la situación actual del proyecto y establecer su línea base.\n\
2. **OE2.** Validar la problemática identificada mediante juicio de expertos.\n\
3. **OE3.** Diseñar el modelo de gestión de cambios asistido por un agente conversacional.\n\
\n\
## Cómo se cumple cada objetivo\n\
| Objetivo | Técnica / herramienta | Fuente de datos | Entregable |\n\
|:---|:---|:---|:---|\n\
| OE1 | Diagrama de Pareto (tipo de proyecto y problemas en pavimentación, corte al 80%), Ishikawa 8M + 5 porqués | Literatura citada + valorizaciones, cuaderno de obra, partes diarios | Diagnóstico y línea base del proyecto |\n\
| OE2 | Juicio de expertos con escala Likert 1–5 | Ficha de validación de la problemática | Problemática validada |\n\
| OE3 | Modelado de flujos, protocolos y roles | Resultados de OE1 y OE2 | Documento del modelo |\n\
\n\
OE3 no lleva indicador porque es un objetivo de entregable.\n\
\n\
## Tareas\n\
- [x] Pedir la grúa\n\
- [ ] Comprar `cemento 42,5`\n\
> Sin permiso no se corta la calle.\n";

pub(crate) fn guardar_png(img: &pixpin_codec::ImagenRgba, nombre: &str) {
    let carpeta = std::env::var("PIXPIN_MUESTRA").unwrap_or_else(|_| ".".into());
    let png = pixpin_codec::imagen::codificar_png(img).unwrap();
    std::fs::write(std::path::Path::new(&carpeta).join(nombre), png).unwrap();
}

fn muestra_de(claro: bool, nombre: &str, con_menu: Option<&str>) {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let foto = foto_de_prueba("obra.png", 1400, 700);
    let texto = format!("{NOTA_DE_MUESTRA}\n![obra](obra.png)\n");
    let mut p = pedido(&texto);
    p.resolver = Box::new(move |_: &str| Some(foto.clone()));
    let mut e = abrir_con(p, claro, (1100, 1000));
    match con_menu {
        Some("mas") => clic(&mut e, Boton::Mas, &mut |_| true),
        Some("barra") => {
            // Al final de «OE3 no lleva…», un renglon nuevo con la barra.
            let t = leer(e.edit);
            let pos = t.find("entregable.").unwrap() + "entregable.".len();
            let pos = t[..pos].encode_utf16().count();
            elegir(e.edit, pos, pos);
            escribir(&e, "\n/");
            pintar(&mut e, None);
            actualizar_barra(&mut e);
        }
        _ => {
            // Con el cursor en una celda: salen los botones de la tabla.
            let t = md_tabla::tablas_en_control(&leer(e.edit)).remove(0);
            ir_a_celda(&e, t.desde, 1, 1);
            actualizar_en_tabla(&mut e);
            pintar(&mut e, None);
        }
    }
    // Desde arriba, como se abre para leerla.
    let arriba = POINT::default();
    enviar(e.edit, EM_SETSCROLLPOS, 0, &arriba as *const _ as isize);
    let img = muestra(&e);
    guardar_png(&img, nombre);
    desmontar(e);
}

#[test]
#[ignore]
fn muestra_oscura_con_tabla_ancha() {
    muestra_de(false, "nota-md-oscura.png", None);
}

#[test]
#[ignore]
fn muestra_clara_con_tabla_ancha() {
    muestra_de(true, "nota-md-clara.png", None);
}

#[test]
#[ignore]
fn muestra_menu_mas() {
    muestra_de(false, "nota-md-menu-mas.png", Some("mas"));
}

#[test]
#[ignore]
fn muestra_menu_barra() {
    muestra_de(false, "nota-md-menu-barra.png", Some("barra"));
    muestra_de(true, "nota-md-menu-barra-clara.png", Some("barra"));
}

/// Cuanto tarda el repintado de una nota larga. Se corre a mano:
/// `cargo test --release -p pixpin-notas medir_repintado -- --ignored --nocapture`.
#[test]
#[ignore]
fn medir_repintado() {
    let mut nota = String::new();
    for i in 0..100 {
        nota.push_str(&format!(
            "## Parte {i}\nTexto con **negrita**, *cursiva* y `codigo` y [un enlace](https://x.es).\n- [ ] tarea {i}\n- [x] hecha {i}\n> una cita\n\n"
        ));
    }
    nota.push_str("| a | b | c |\n|:---|:---|:---|\n");
    for i in 0..60 {
        nota.push_str(&format!("| fila {i} | **dato** | 3,20 |\n"));
    }
    let t = std::time::Instant::now();
    let mut e = abrir(&nota);
    let abrir_ms = t.elapsed();
    let t = std::time::Instant::now();
    pintar(&mut e, None);
    let entero = t.elapsed();
    let t = std::time::Instant::now();
    pintar(&mut e, Some(&[3, 40]));
    let dos = t.elapsed();
    let t = std::time::Instant::now();
    let md = markdown(&e);
    let leer_md = t.elapsed();
    assert_eq!(md, nota);
    println!(
        "nota de {} renglones con una tabla de 61 filas: abrir {abrir_ms:?}, repintado entero {entero:?}, dos renglones {dos:?}, a Markdown {leer_md:?}",
        nota.lines().count()
    );
    desmontar(e);
}

#[test]
#[ignore]
fn muestra_foto() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let foto = foto_de_prueba("obra2.png", 1400, 700);
    let texto = "# Visita de obra\nLa losa ya esta hormigonada:\n![losa del segundo piso](obra2.png)\nFalta el curado.";
    let mut p = pedido(texto);
    p.resolver = Box::new(move |_: &str| Some(foto.clone()));
    for (claro, activa, nombre) in [
        (false, false, "nota-md-foto.png"),
        (true, true, "nota-md-foto-cursor-clara.png"),
    ] {
        let mut e = abrir_con(
            Pedido {
                texto: p.texto.clone(),
                rotulos: rotulos(),
                nombre_de_fichero: None,
                colocacion: None,
                resolver: Box::new({
                    let f = foto_de_prueba("obra2.png", 1400, 700);
                    move |_: &str| Some(f.clone())
                }),
                adjuntar: Box::new(|_: &Path| None),
                compartir: None,
                integracion: Default::default(),
                comentarios: Default::default(),
            },
            claro,
            (900, 820),
        );
        let t = leer(e.edit);
        let pos = if activa { t.find("![").unwrap() + 4 } else { 0 };
        let pos = t[..pos].encode_utf16().count();
        elegir(e.edit, pos, pos);
        pintar(&mut e, None);
        let arriba = POINT::default();
        enviar(e.edit, EM_SETSCROLLPOS, 0, &arriba as *const _ as isize);
        guardar_png(&muestra(&e), nombre);
        desmontar(e);
    }
    let _ = p;
}
