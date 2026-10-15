//! Las asas con la ventana de verdad pero **oculta**: el raton con mensajes
//! hechos a mano a la propia funcion (nada de raton de verdad).

use super::super::pruebas::{abrir, abrir_con, foto_de_prueba, guardar_png, pedido};
use super::super::*;
use super::*;

fn raton(e: &mut Estado, m: u32, x: i32, y: i32) -> bool {
    let msg = MSG {
        hwnd: e.edit,
        message: m,
        lParam: LPARAM((((y as u16) as isize) << 16) | (x as u16) as isize),
        ..Default::default()
    };
    super::raton(e, &msg)
}

fn centro(r: RECT) -> (i32, i32) {
    ((r.left + r.right) / 2, (r.top + r.bottom) / 2)
}

/// Pasa el raton por el renglon `linea` y da el asa de su bloque.
fn asa_del_renglon(e: &mut Estado, linea: usize) -> RECT {
    let ls = md_vivo::lineas(&leer(e.edit));
    let y = y_de(e, ls[linea].desde) + 4;
    let (izq, _) = columna(e);
    raton(e, WM_MOUSEMOVE, izq + 20, y);
    a_la_vista()
        .iter()
        .find(|(a, _)| matches!(a, Asa::Bloque(_)))
        .expect("sale el asa del bloque")
        .1
}

/// Coge `asa` y la suelta en `(x, y)`, mirando que la linea azul salga.
fn arrastrar(e: &mut Estado, asa: RECT, x: i32, y: i32) {
    let (x0, y0) = centro(asa);
    assert!(raton(e, WM_LBUTTONDOWN, x0, y0), "el asa se coge");
    assert!(raton(e, WM_MOUSEMOVE, x, y));
    assert!(
        linea_azul().is_some(),
        "mientras se arrastra, la linea azul ensena donde cae"
    );
    assert!(raton(e, WM_LBUTTONUP, x, y));
    assert!(linea_azul().is_none());
}

/// Lo bajo del texto (para soltar al final).
fn abajo_del_todo(e: &Estado) -> i32 {
    let n = leer(e.edit).encode_utf16().count();
    y_de(e, n) + 30
}

#[test]
fn el_asa_de_un_parrafo_lo_mueve_abajo_y_arriba_en_un_solo_paso_de_deshacer() {
    let mut e = abrir("# Plan\nuno\ndos\ntres");
    let asa = asa_del_renglon(&mut e, 1);
    let (x, _) = centro(asa);
    let y = abajo_del_todo(&e);
    arrastrar(&mut e, asa, x, y);
    assert_eq!(markdown(&e), "# Plan\ndos\ntres\nuno");
    // Deshacer lo deja como estaba, de una vez.
    assert!(congelar::deshacer(&mut e, false));
    assert_eq!(markdown(&e), "# Plan\nuno\ndos\ntres");
    // Y hacia arriba, delante del titulo.
    let asa = asa_del_renglon(&mut e, 3);
    let ls = md_vivo::lineas(&leer(e.edit));
    let arriba = y_de(&e, ls[0].desde);
    arrastrar(&mut e, asa, x, arriba - 2);
    assert_eq!(markdown(&e), "tres\n# Plan\nuno\ndos");
    desmontar(e);
}

#[test]
fn esc_a_mitad_de_arrastrar_lo_deja_todo_como_estaba() {
    let mut e = abrir("uno\ndos\ntres");
    let asa = asa_del_renglon(&mut e, 0);
    let (x, y) = centro(asa);
    assert!(raton(&mut e, WM_LBUTTONDOWN, x, y));
    let fondo = abajo_del_todo(&e);
    assert!(raton(&mut e, WM_MOUSEMOVE, x, fondo));
    assert!(cancelar(&e));
    assert!(linea_azul().is_none() && a_la_vista().is_empty());
    // Soltar despues ya no es de nadie.
    assert!(!raton(&mut e, WM_LBUTTONUP, x, fondo));
    assert_eq!(markdown(&e), "uno\ndos\ntres");
    // Caso negativo: sin arrastre, Esc no es suyo.
    assert!(!cancelar(&e));
    // Ni un clic en el asa sin moverse mueve nada.
    let asa = asa_del_renglon(&mut e, 0);
    let (x, y) = centro(asa);
    assert!(raton(&mut e, WM_LBUTTONDOWN, x, y));
    assert!(raton(&mut e, WM_LBUTTONUP, x, y));
    assert_eq!(markdown(&e), "uno\ndos\ntres");
    desmontar(e);
}

const CON_TABLA: &str = "Antes.\n| A | B |\n|:---|:---|\n| 1 | x |\n| 2 | y |\n| 3 | z |\nDespues.";

#[test]
fn una_tabla_se_mueve_entera_con_su_asa_y_vuelve_con_deshacer() {
    let mut e = abrir(CON_TABLA);
    assert_eq!(markdown(&e), CON_TABLA);
    let asa = asa_del_renglon(&mut e, 1);
    assert!(
        matches!(a_la_vista()[0].0, Asa::Bloque(b) if b.desde == 1 && b.hasta > 1),
        "la tabla entera es un bloque"
    );
    let (x, _) = centro(asa);
    let y = abajo_del_todo(&e);
    arrastrar(&mut e, asa, x, y);
    let esperado =
        normal("Antes.\nDespues.\n| A | B |\n|:---|:---|\n| 1 | x |\n| 2 | y |\n| 3 | z |");
    assert!(
        esperado.starts_with("Antes.\nDespues.\n| A | B |"),
        "{esperado:?}"
    );
    assert_eq!(markdown(&e), esperado);
    assert!(congelar::deshacer(&mut e, false));
    assert_eq!(markdown(&e), CON_TABLA, "en un solo paso");
    // Un parrafo que pasa por encima de la tabla tampoco la rompe.
    let asa = asa_del_renglon(&mut e, 0);
    let fondo = abajo_del_todo(&e);
    arrastrar(&mut e, asa, x, fondo);
    assert_eq!(
        markdown(&e),
        "| A | B |\n|:---|:---|\n| 1 | x |\n| 2 | y |\n| 3 | z |\nDespues.\nAntes."
    );
    desmontar(e);
}

#[test]
fn una_foto_se_mueve_con_su_hueco() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let foto = foto_de_prueba("asas-foto.png", 300, 160);
    let mut p = pedido("arriba\n![x](asas-foto.png)\nabajo");
    p.resolver = Box::new(move |_: &str| Some(foto.clone()));
    let mut e = abrir_con(p, false, (1000, 900));
    let _ = muestra(&e);
    let asa = asa_del_renglon(&mut e, 1);
    let (x, _) = centro(asa);
    let fondo = abajo_del_todo(&e);
    arrastrar(&mut e, asa, x, fondo);
    assert_eq!(markdown(&e), "arriba\nabajo\n![x](asas-foto.png)");
    let _ = muestra(&e);
    let (linea, _, caja, _) = imagenes::puesta(0).expect("la foto se sigue viendo");
    assert_eq!(linea, 2);
    let ls = md_vivo::lineas(&leer(e.edit));
    assert!(
        caja.top >= y_de(&e, ls[1].desde),
        "debajo de «abajo», en su hueco"
    );
    desmontar(e);
}

/// Pasa el raton por la celda `(f, c)` de la primera tabla y da las dos
/// barritas que salen.
fn barritas(e: &mut Estado, f: usize, c: usize) -> (RECT, RECT) {
    let tc = md_tabla::tablas_en_control(&leer(e.edit)).remove(0);
    let p = tc.celdas[f][c];
    raton(e, WM_MOUSEMOVE, x_de(e, p) + 3, y_de(e, p) + 4);
    let v = a_la_vista();
    let fila = v
        .iter()
        .find(|(a, _)| matches!(a, Asa::Fila { f: x, .. } if *x == f))
        .expect("la barrita de la fila")
        .1;
    let col = v
        .iter()
        .find(|(a, _)| matches!(a, Asa::Columna { c: x, .. } if *x == c))
        .expect("la de la columna")
        .1;
    (fila, col)
}

#[test]
fn las_barritas_de_una_tabla_reordenan_filas_y_columnas() {
    let mut e = abrir(CON_TABLA);
    // La fila del 3, arriba del todo de lo de dentro (delante de la del 1).
    let (fila, _) = barritas(&mut e, 3, 0);
    let tc = md_tabla::tablas_en_control(&leer(e.edit)).remove(0);
    let y = y_de(&e, tc.celdas[1][0]) - 2;
    arrastrar(&mut e, fila, centro(fila).0, y);
    assert_eq!(
        markdown(&e),
        "Antes.\n| A | B |\n|:---|:---|\n| 3 | z |\n| 1 | x |\n| 2 | y |\nDespues."
    );
    // La columna B delante de la A.
    let (_, col) = barritas(&mut e, 1, 1);
    let tc = md_tabla::tablas_en_control(&leer(e.edit)).remove(0);
    let x = x_de(&e, tc.celdas[0][0]) - 4;
    arrastrar(&mut e, col, x, centro(col).1);
    assert_eq!(
        markdown(&e),
        "Antes.\n| B | A |\n|:---|:---|\n| z | 3 |\n| x | 1 |\n| y | 2 |\nDespues."
    );
    // Y deshacer, un paso cada una.
    assert!(congelar::deshacer(&mut e, false));
    assert!(markdown(&e).contains("| A | B |"));
    desmontar(e);
}

#[test]
fn los_mas_de_una_tabla_anaden_una_fila_y_una_columna_al_final() {
    let mut e = abrir(CON_TABLA);
    let _ = barritas(&mut e, 1, 0);
    let v = a_la_vista();
    let mas = |que: fn(&Asa) -> bool| v.iter().find(|(a, _)| que(a)).expect("sale el «+»").1;
    let fila = mas(|a| matches!(a, Asa::MasFila { .. }));
    let col = mas(|a| matches!(a, Asa::MasColumna { .. }));
    // El de la fila, debajo de la tabla; el de la columna, a su derecha.
    let tc = md_tabla::tablas_en_control(&leer(e.edit)).remove(0);
    assert!(fila.top > y_de(&e, *tc.celdas.last().unwrap().first().unwrap()));
    assert!(col.left > x_de(&e, *tc.celdas[0].last().unwrap()));
    let (x, y) = centro(fila);
    assert!(raton(&mut e, WM_LBUTTONDOWN, x, y));
    assert!(raton(&mut e, WM_LBUTTONUP, x, y));
    assert_eq!(
        markdown(&e),
        "Antes.\n| A | B |\n|:---|:---|\n| 1 | x |\n| 2 | y |\n| 3 | z |\n|  |  |\nDespues."
    );
    // El cursor, en la fila nueva.
    let tc = md_tabla::tablas_en_control(&leer(e.edit)).remove(0);
    assert_eq!(seleccion(e.edit).0, tc.celdas[4][0]);
    // Y la columna: el raton otra vez por la tabla, que ya cambio.
    let _ = barritas(&mut e, 1, 0);
    let col = a_la_vista()
        .iter()
        .find(|(a, _)| matches!(a, Asa::MasColumna { .. }))
        .unwrap()
        .1;
    let (x, y) = centro(col);
    assert!(raton(&mut e, WM_LBUTTONDOWN, x, y));
    assert!(raton(&mut e, WM_LBUTTONUP, x, y));
    assert!(
        markdown(&e).starts_with("Antes.\n| A | B |  |\n"),
        "{}",
        markdown(&e)
    );
    // Un paso de deshacer cada uno.
    assert!(congelar::deshacer(&mut e, false));
    assert!(markdown(&e).starts_with("Antes.\n| A | B |\n"));
    desmontar(e);
}

#[test]
fn un_clic_en_una_barrita_elige_su_fila_o_su_columna() {
    let mut e = abrir(CON_TABLA);
    let (fila, _) = barritas(&mut e, 2, 1);
    let (x, y) = centro(fila);
    assert!(raton(&mut e, WM_LBUTTONDOWN, x, y));
    assert!(raton(&mut e, WM_LBUTTONUP, x, y));
    let tc = md_tabla::tablas_en_control(&leer(e.edit)).remove(0);
    let (a, b) = seleccion(e.edit);
    assert!(
        a <= tc.celdas[2][0] && b > tc.celdas[2][1],
        "la fila entera: {a}..{b}"
    );
    assert_eq!(markdown(&e), CON_TABLA, "elegir no cambia nada");
    desmontar(e);
}

#[test]
fn fuera_de_un_bloque_no_sale_ningun_asa() {
    let mut e = abrir("uno\n\ndos");
    let ls = md_vivo::lineas(&leer(e.edit));
    let (izq, _) = columna(&e);
    let y = y_de(&e, ls[1].desde) + 2;
    raton(&mut e, WM_MOUSEMOVE, izq + 20, y);
    assert!(
        a_la_vista()
            .iter()
            .all(|(a, _)| !matches!(a, Asa::Bloque(b) if b.desde == 1)),
        "el renglon vacio no se coge"
    );
    desmontar(e);
}

// ---------------------------------------------------------------------------
// Muestras en PNG, a mano:
// `PIXPIN_MUESTRA=<carpeta> cargo test -p pixpin-notas asas::pruebas::muestra_ -- --ignored`

const NOTA: &str = "# Plan de obra\nLo primero es revisar el **segundo piso** con el residente.\n| Objetivo | Tecnica | Fuente |\n|:---|:---|:---|\n| OE1 | Pareto | Partes diarios |\n| OE2 | Likert 1-5 | Ficha de validacion |\n| OE3 | Flujos | Resultados de OE1 |\nDespues de la tabla sigue el texto.\n- Avisar al vecino\n- Pedir la grua";

/// `en_tabla`: el raton en una celda (el asa de la tabla y sus barritas) y
/// arrastrando la columna del medio a la izquierda; si no, arrastrando el
/// parrafo de arriba hasta despues de la tabla.
fn muestra_de(claro: bool, en_tabla: bool, nombre: &str) {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let mut e = abrir_con(pedido(NOTA), claro, (980, 640));
    elegir(e.edit, 0, 0);
    pintar(&mut e, None);
    if en_tabla {
        let (_, col) = barritas(&mut e, 2, 1);
        let (x, y) = centro(col);
        assert!(raton(&mut e, WM_LBUTTONDOWN, x, y));
        let tc = md_tabla::tablas_en_control(&leer(e.edit)).remove(0);
        let x0 = x_de(&e, tc.celdas[0][0]) + 6;
        raton(&mut e, WM_MOUSEMOVE, x0, y);
    } else {
        let asa = asa_del_renglon(&mut e, 1);
        let (x, y) = centro(asa);
        assert!(raton(&mut e, WM_LBUTTONDOWN, x, y));
        let ls = md_vivo::lineas(&leer(e.edit));
        let n = ls
            .iter()
            .position(|l| {
                leer(e.edit)
                    .encode_utf16()
                    .skip(l.desde)
                    .take(7)
                    .eq("Despues".encode_utf16())
            })
            .unwrap();
        let yn = y_de(&e, ls[n].desde) + 6;
        raton(&mut e, WM_MOUSEMOVE, x, yn);
    }
    guardar_png(&muestra(&e), nombre);
    cancelar(&e);
    desmontar(e);
}

#[test]
#[ignore]
fn muestra_asas() {
    muestra_de(false, false, "nota-md-asas-oscura.png");
    muestra_de(true, false, "nota-md-asas-clara.png");
    muestra_de(false, true, "nota-md-asas-tabla-oscura.png");
    muestra_de(true, true, "nota-md-asas-tabla-clara.png");
}

/// Lo que da el editor al abrir `md` y leerlo: una tabla al final lleva
/// detras el parrafo que el control necesita.
fn normal(md: &str) -> String {
    let e = abrir(md);
    let m = markdown(&e);
    desmontar(e);
    m
}
