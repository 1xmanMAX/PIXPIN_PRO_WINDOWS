//! Pruebas de las tablas anchas con su desplazamiento propio, con la
//! ventana oculta.

use super::*;

const LARGO: &str = "Diagrama de Pareto (tipo de proyecto y problemas en pavimentacion, corte al 80%), Ishikawa 8M + 5 porques";

/// Dos tablas que no caben en una ventana de 1000 y un texto entre ellas.
fn dos_anchas() -> String {
    format!(
        "# Obra\nantes de las tablas\n\n| Objetivo | Tecnica / herramienta | Fuente de datos | Entregable |\n|:---|:---|:---|:---|\n| OE1 | {LARGO} | Literatura citada + valorizaciones, cuaderno de obra, partes diarios | Diagnostico y linea base del proyecto |\n| OE2 | Juicio de expertos con escala Likert 1-5 | Ficha de validacion de la problematica | Problematica validada |\n\nentre las dos\n\n| N | Indicador | Tipo | Unidad | Descripcion |\n|---:|:---|:---|:---|:---|\n| 1 | Reduccion del Costo de No Calidad (CNC) por retrabajos | Cuantitativo | % del costo directo | Monto de partidas reejecutadas por errores respecto al costo directo contractual |\n| 2 | Tiempo de respuesta a consultas de obra | Cuantitativo | horas | Lo que tarda en contestarse una consulta del cuaderno de obra |\n\ndespues\n"
    )
}

const ESTRECHA: &str = "antes\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\ndespues\n";

fn abrir_con(texto: &str, claro: bool, tamano: (i32, i32)) -> Estado {
    let p = Pedido {
        texto: texto.into(),
        rotulos: Rotulos::default(),
        nombre_de_fichero: None,
        colocacion: None,
        resolver: Box::new(|_: &str| None),
        adjuntar: Box::new(|_: &Path| None),
        compartir: None,
        integracion: Default::default(),
        comentarios: Default::default(),
    };
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

fn abrir(texto: &str) -> Estado {
    abrir_con(texto, false, (1000, 760))
}

fn x_de(e: &Estado, pos: usize) -> i32 {
    let mut pt = POINT::default();
    enviar(e.edit, EM_POSFROMCHAR, &mut pt as *mut _ as usize, pos as isize);
    pt.x
}

fn tablas_de(e: &Estado) -> Vec<TablaEnControl> {
    md_tabla::tablas_en_control(&leer(e.edit))
}

/// Lo desplazada que esta la tabla `k`.
fn desp(e: &Estado, k: usize) -> i32 {
    desplazamiento_en(e, tablas_de(e)[k].desde)
}

/// Donde se ve (x en el papel, sin el hueco) el principio de la celda.
fn x_celda(e: &Estado, k: usize, f: usize, c: usize) -> i32 {
    x_de(e, tablas_de(e)[k].celdas[f][c]) - tabla_ancha::hueco()
}

fn ancho_de(e: &Estado, k: usize) -> i32 {
    let doc = e.doc.as_ref().unwrap();
    leer_fila_entera(doc, tablas_de(e)[k].desde).unwrap().2 * e.ppp / 1440
}

#[test]
fn una_tabla_ancha_conserva_su_ancho_y_el_texto_no_se_mueve() {
    let e = abrir(&dos_anchas());
    let (vi, vd) = visible(e.edit);
    assert!(tabla_ancha::hueco() > 0, "con tablas anchas se abre el hueco");
    assert!(ancho_de(&e, 0) > vd - vi, "no se encoge a la ventana: {} > {}", ancho_de(&e, 0), vd - vi);
    // El texto sigue donde estaria sin tablas anchas.
    let x_texto = x_de(&e, leer(e.edit).find("antes").unwrap()) - tabla_ancha::hueco();
    desmontar(e);
    let sin = abrir("# Obra\nantes de las tablas\n");
    let x_sin = x_de(&sin, leer(sin.edit).find("antes").unwrap());
    assert_eq!(tabla_ancha::hueco(), 0);
    desmontar(sin);
    assert!((x_texto - x_sin).abs() <= 2, "{x_texto} y {x_sin}");
}

#[test]
fn una_tabla_ancha_empieza_donde_el_texto_y_lleva_barra() {
    let e = abrir(&dos_anchas());
    let (vi, vd) = visible(e.edit);
    let aire = tabla_rtf::AIRE_PX * e.ppp / 96;
    // Sin desplazar empieza donde el texto (el usuario, 1-oct), y lo que se
    // ve de ella es el papel entero, no la columna.
    let x_texto = x_de(&e, leer(e.edit).find("antes").unwrap());
    assert!((x_celda(&e, 0, 0, 0) + tabla_ancha::hueco() - (x_texto + aire)).abs() <= 2);
    assert!(vi < x_texto && vd - vi > columna(e.edit));
    // Del todo a la derecha: su borde derecho, donde acaba el texto.
    let v = VISTAS.with(|v| v.borrow()[0]);
    desplazar(&e, v.desde, 1_000_000);
    let v = VISTAS.with(|v| v.borrow()[0]);
    let derecha_tabla = x_texto - v.c.desplazamiento + v.ancho;
    assert!((derecha_tabla - (x_texto + columna(e.edit))).abs() <= 2, "{derecha_tabla}");
    assert_eq!(VISTAS.with(|v| v.borrow().len()), 2, "las dos llevan barra");
    desmontar(e);
}

#[test]
fn una_tabla_que_cabe_va_centrada_sin_hueco_ni_barra() {
    let e = abrir(ESTRECHA);
    assert_eq!(tabla_ancha::hueco(), 0);
    assert!(VISTAS.with(|v| v.borrow().is_empty()));
    assert_eq!(desp(&e, 0), 0);
    // Y pedir desplazarla no la mueve.
    let x = x_celda(&e, 0, 0, 0);
    desplazar(&e, tablas_de(&e)[0].desde, 200);
    assert_eq!(x_celda(&e, 0, 0, 0), x);
    desmontar(e);
}

#[test]
fn desplazar_una_tabla_no_mueve_la_otra_ni_el_texto_y_se_guarda_igual() {
    let md = dos_anchas();
    let e = abrir(&md);
    let texto = leer(e.edit);
    let p_entre = texto.find("entre las dos").unwrap();
    let antes = (x_celda(&e, 0, 1, 2), x_celda(&e, 1, 1, 2), x_de(&e, p_entre));
    desplazar(&e, tablas_de(&e)[0].desde, 300);
    assert_eq!(desp(&e, 0), 300);
    assert_eq!(x_celda(&e, 0, 1, 2), antes.0 - 300, "la primera se corre");
    assert_eq!(x_celda(&e, 1, 1, 2), antes.1, "la segunda no");
    assert_eq!(x_de(&e, p_entre), antes.2, "el texto tampoco");
    desplazar(&e, tablas_de(&e)[1].desde, 120);
    assert_eq!((desp(&e, 0), desp(&e, 1)), (300, 120));
    assert_eq!(markdown(&e), md, "el .md no sabe de desplazamientos");
    // Caso negativo: no se pasa de su final (su borde, donde acaba el texto).
    desplazar(&e, tablas_de(&e)[1].desde, 99_999);
    assert_eq!(desp(&e, 1), ancho_de(&e, 1) - columna(e.edit));
    desmontar(e);
}

#[test]
fn desplazar_no_es_un_paso_de_deshacer() {
    let e = abrir(&dos_anchas());
    desplazar(&e, tablas_de(&e)[0].desde, 200);
    // EM_CANUNDO
    assert_eq!(enviar(e.edit, 0x00C6, 0, 0), 0);
    desmontar(e);
}

#[test]
fn se_escribe_en_una_celda_tras_desplazar_y_la_tabla_se_queda_donde_estaba() {
    let mut e = abrir(&dos_anchas());
    // Hasta el final: la ultima columna a la vista.
    desplazar(&e, tablas_de(&e)[0].desde, 99_999);
    let d = desp(&e, 0);
    assert!(d > 0);
    let p = tablas_de(&e)[0].celdas[2][3];
    elegir(e.edit, p, p);
    let t = ancho_nulo("Muy ");
    enviar(e.edit, EM_REPLACESEL, 1, t.as_ptr() as isize);
    seguir(&e);
    assert!(markdown(&e).contains("| Muy Problematica validada |"));
    assert_eq!(desp(&e, 0), d, "escribir no la devuelve al principio");
    // Rehacer la tabla (una fila mas) tampoco.
    operar_tabla(&mut e, OpTabla::FilaDebajo);
    assert_eq!(tablas_de(&e)[0].celdas.len(), 4);
    assert_eq!(desp(&e, 0), d);
    desmontar(e);
}

#[test]
fn el_cursor_en_una_celda_que_no_se_ve_desplaza_su_tabla_lo_justo() {
    let e = abrir(&dos_anchas());
    let (vi, vd) = visible(e.edit);
    let p = tablas_de(&e)[0].celdas[1][3];
    assert!(x_de(&e, p) > vd, "la ultima columna no se ve");
    elegir(e.edit, p, p);
    seguir(&e);
    let d = desp(&e, 0);
    assert!(d > 0);
    let x = x_de(&e, p);
    assert!(x >= vi && x <= vd, "{x} en {vi}..{vd}");
    assert_eq!(desp(&e, 1), 0, "la otra no");
    // Caso negativo: con el cursor en el texto no se mueve nada.
    elegir(e.edit, 3, 3);
    seguir(&e);
    assert_eq!((desp(&e, 0), desp(&e, 1)), (d, 0));
    desmontar(e);
}

#[test]
fn las_combinadas_y_los_colores_sobreviven_al_desplazamiento() {
    let md = format!(
        "<table>\n  <tr>\n    <th colspan=\"2\" align=\"center\">Presupuesto</th>\n    <th>{LARGO}</th>\n    <th>{LARGO}</th>\n  </tr>\n  <tr>\n    <td rowspan=\"2\" style=\"background:#ffc9c9;color:#e03131\">Obra gruesa</td>\n    <td>Hormigon</td>\n    <td align=\"right\">3,20</td>\n    <td>x</td>\n  </tr>\n  <tr>\n    <td>Acero</td>\n    <td align=\"right\">1,10</td>\n    <td>y</td>\n  </tr>\n  <tr>\n    <td>Total</td>\n    <td></td>\n    <td align=\"right\">4,30</td>\n    <td style=\"background:#a5d8ff\">z</td>\n  </tr>\n</table>\n"
    );
    let e = abrir(&md);
    assert_eq!(markdown(&e), md, "se abre igual");
    assert_eq!(VISTAS.with(|v| v.borrow().len()), 1, "es ancha");
    let antes = modelo(&e, &tablas_de(&e)[0]);
    desplazar(&e, tablas_de(&e)[0].desde, 180);
    assert_eq!(desp(&e, 0), 180);
    assert_eq!(modelo(&e, &tablas_de(&e)[0]), antes);
    assert_eq!(markdown(&e), md);
    desmontar(e);
}

/// Un mensaje del raton para el control, hecho a mano (no es entrada: se
/// le pasa a [`raton`] directamente).
fn mensaje(e: &Estado, msg: u32, w: usize, x: i32, y: i32) -> MSG {
    MSG {
        hwnd: e.edit,
        message: msg,
        wParam: WPARAM(w),
        lParam: LPARAM(((y as u16 as isize) << 16) | (x as u16 as isize)),
        ..Default::default()
    }
}

#[test]
fn la_rueda_de_lado_sobre_una_tabla_la_desplaza_y_fuera_no() {
    let e = abrir(&dos_anchas());
    let tc = &tablas_de(&e)[0];
    let mut p = POINT::default();
    enviar(e.edit, EM_POSFROMCHAR, &mut p as *mut _ as usize, tc.celdas[1][1] as isize);
    // SAFETY: conversion de coordenadas de una ventana propia.
    unsafe {
        let _ = ClientToScreen(e.edit, &mut p);
    }
    let derecha = 120usize << 16;
    assert!(raton(&e, &mensaje(&e, WM_MOUSEHWHEEL, derecha, p.x, p.y)));
    assert_eq!(desp(&e, 0), PASO_PX * e.ppp / 96);
    assert_eq!(desp(&e, 1), 0);
    // Caso negativo: sobre el texto la rueda es del control.
    let mut q = POINT::default();
    enviar(e.edit, EM_POSFROMCHAR, &mut q as *mut _ as usize, 2);
    // SAFETY: igual.
    unsafe {
        let _ = ClientToScreen(e.edit, &mut q);
    }
    assert!(!raton(&e, &mensaje(&e, WM_MOUSEHWHEEL, derecha, q.x, q.y)));
    // Y la rueda de siempre (sin Mayus) tampoco es de la tabla.
    assert!(!raton(&e, &mensaje(&e, WM_MOUSEWHEEL, derecha, p.x, p.y)));
    desmontar(e);
}

#[test]
fn arrastrar_la_barra_desplaza_su_tabla() {
    let e = abrir(&dos_anchas());
    let v = VISTAS.with(|v| v.borrow()[0]);
    let (_, abajo, y, an) = caja_de(e.edit, &v, e.ppp);
    assert!(y > abajo);
    let (px, largo) = tabla_ancha::pulgar(v.vis_izq, an, v.ancho, &v.c, PULGAR_MINIMO_PX * e.ppp / 96);
    let x = px + largo / 2;
    assert!(raton(&e, &mensaje(&e, WM_LBUTTONDOWN, 1, x, y + 2)));
    assert_eq!(desp(&e, 0), 0, "pulsar en el pulgar no lo mueve");
    assert!(raton(&e, &mensaje(&e, WM_MOUSEMOVE, 1, x + 100, y + 2)));
    let d = desp(&e, 0);
    assert!(d > 100, "la tabla se corre mas que el raton: {d}");
    assert!(raton(&e, &mensaje(&e, WM_LBUTTONUP, 0, x + 100, y + 2)));
    // Soltada, mover el raton ya no la arrastra.
    assert!(!raton(&e, &mensaje(&e, WM_MOUSEMOVE, 0, x + 300, y + 2)));
    assert_eq!(desp(&e, 0), d);
    assert_eq!(desp(&e, 1), 0, "la otra no");
    desmontar(e);
}

#[test]
fn cambiar_el_tamano_de_la_ventana_recoloca_las_tablas() {
    let e = abrir(&dos_anchas());
    desplazar(&e, tablas_de(&e)[0].desde, 400);
    // Mucho mas ancha: todas caben y dejan de llevar barra.
    // SAFETY: ventana propia.
    unsafe {
        let _ = SetWindowPos(e.marco, None, 0, 0, 3200, 900, SWP_NOMOVE | SWP_NOZORDER);
    }
    colocar_todas(&e, None);
    let (vi, vd) = visible(e.edit);
    assert!(ancho_de(&e, 0) <= vd - vi, "{} {}", ancho_de(&e, 0), vd - vi);
    assert_eq!(VISTAS.with(|v| v.borrow().len()), 0);
    assert_eq!(desp(&e, 0), 0);
    desmontar(e);
}

// `PIXPIN_MUESTRA=<carpeta> cargo test -p pixpin-notas muestra_tablas_anchas -- --ignored`
fn muestra_de(claro: bool, nombre: &str) {
    let mut e = abrir_con(&dos_anchas(), claro, (1100, 900));
    desplazar(&e, tablas_de(&e)[1].desde, 330);
    let p = tablas_de(&e)[1].celdas[1][4];
    elegir(e.edit, p, p);
    pintar(&mut e, None);
    let arriba = POINT::default();
    enviar(e.edit, EM_SETSCROLLPOS, 0, &arriba as *const _ as isize);
    let img = muestra(&e);
    let carpeta = std::env::var("PIXPIN_MUESTRA").unwrap_or_else(|_| ".".into());
    let png = pixpin_codec::imagen::codificar_png(&img).unwrap();
    std::fs::write(std::path::Path::new(&carpeta).join(nombre), png).unwrap();
    desmontar(e);
}

#[test]
#[ignore]
fn muestra_tablas_anchas() {
    muestra_de(false, "nota-md-tablas-anchas-oscura.png");
    muestra_de(true, "nota-md-tablas-anchas-clara.png");
}

/// Cuanto cuesta desplazar una tabla ancha de 60 filas. A mano:
/// `cargo test --release -p pixpin-notas medir_desplazar -- --ignored --nocapture`.
#[test]
#[ignore]
fn medir_desplazar() {
    let mut md = String::from("| a | b | c | d |\n|---|---|---|---|\n");
    for i in 0..60 {
        md.push_str(&format!("| fila {i} | {LARGO} | {LARGO} | {LARGO} |\n"));
    }
    let e = abrir(&md);
    let desde = tablas_de(&e)[0].desde;
    let t = std::time::Instant::now();
    for i in 0..20 {
        desplazar(&e, desde, i * 30);
    }
    println!("tabla de 61 filas: un paso de desplazamiento {:?}", t.elapsed() / 20);
    let t = std::time::Instant::now();
    for _ in 0..20 {
        seguir(&e);
    }
    println!("seguir al cursor sin mover nada {:?}", t.elapsed() / 20);
    desmontar(e);
}

