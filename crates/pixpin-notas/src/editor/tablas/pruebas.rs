//! Pruebas de pegar, combinar y colorear, con la ventana oculta.

use super::*;
use pixpin_docs::md_tabla_html::{leer_pegado, leer_tsv};

fn pedido(texto: &str) -> Pedido {
    Pedido {
        texto: texto.into(),
        rotulos: Rotulos {
            boton_fila_mas: "+ Fila".into(),
            boton_fila_menos: "− Fila".into(),
            boton_columna_mas: "+ Columna".into(),
            boton_columna_menos: "− Columna".into(),
            ..Rotulos::default()
        },
        nombre_de_fichero: None,
        colocacion: None,
        resolver: Box::new(|_: &str| None),
        adjuntar: Box::new(|_: &Path| None),
        compartir: None,
        integracion: Default::default(),
        comentarios: Default::default(),
    }
}

fn abrir_con(texto: &str, claro: bool, tamano: (i32, i32)) -> Estado {
    montar(
        pedido(texto),
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

fn tabla(e: &Estado) -> TablaEnControl {
    md_tabla::tablas_en_control(&leer(e.edit)).remove(0)
}

/// Elige de la celda `a` a la celda `b` (las dos dentro), como al
/// arrastrar con el raton.
fn elegir_celdas(e: &Estado, a: (usize, usize), b: (usize, usize)) {
    let t = tabla(e);
    elegir(e.edit, t.celdas[a.0][a.1], t.celdas[b.0][b.1] + 1);
    recordar_arrastre(t.celdas[a.0][a.1], t.celdas[b.0][b.1]);
}

#[test]
fn elegir_varias_filas_lo_alarga_el_control_y_vale_el_rectangulo_arrastrado() {
    let e = abrir(TRES_POR_TRES);
    let t = tabla(&e);
    elegir(e.edit, t.celdas[1][1], t.celdas[2][1] + 1);
    let (a, b) = seleccion(e.edit);
    assert!(a <= t.celdas[1][0] && b > t.celdas[2][2], "filas enteras: {a}..{b}");
    // Sin arrastre apuntado, filas enteras; con el, sus celdas.
    ARRASTRE.with(|x| x.set(None));
    assert_eq!(elegido(&e).map(|x| (x.1, x.2)), Some(((1, 0), (2, 2))));
    recordar_arrastre(t.celdas[1][1], t.celdas[2][1]);
    assert_eq!(elegido(&e).map(|x| (x.1, x.2)), Some(((1, 1), (2, 1))));
    // Un arrastre viejo que no es lo elegido no cuenta.
    elegir(e.edit, t.celdas[0][0], t.celdas[0][0]);
    assert_eq!(elegido(&e).map(|x| (x.1, x.2)), Some(((0, 0), (0, 0))));
    desmontar(e);
}

/// Lo que escribiria el movil (`Tablas.aHtml`) con una combinada a lo
/// ancho, otra a lo alto, colores y alineaciones de celda.
const PRESUPUESTO: &str = "# Obra\n<table>\n  <tr>\n    <th colspan=\"3\" align=\"center\">Presupuesto</th>\n  </tr>\n  <tr>\n    <td rowspan=\"2\" style=\"background:#ffc9c9;color:#e03131\">Obra gruesa</td>\n    <td>Hormigon</td>\n    <td align=\"right\">3,20</td>\n  </tr>\n  <tr>\n    <td>Acero</td>\n    <td align=\"right\">1,10</td>\n  </tr>\n  <tr>\n    <td style=\"background:#ffec99\">**Total**</td>\n    <td style=\"background:#ffec99\"></td>\n    <td align=\"right\" style=\"background:#ffec99\">4,30</td>\n  </tr>\n</table>\nfin\n";

const DE_SHEETS: &str = "<google-sheets-html-origin><table><tbody><tr><td style=\"background-color:#4a86e8;font-weight:bold;color:#ffffff;text-align:center;\" colspan=\"3\">Mensualidad</td></tr><tr><td rowspan=\"2\" style=\"background-color:#ffec99;\">Enero</td><td>Luz</td><td style=\"text-align:right;\">120</td></tr><tr><td>Agua</td><td style=\"text-align:right;color:#e03131;\">45</td></tr><tr><td>Febrero</td><td>Luz</td><td style=\"text-align:right;\">98</td></tr></tbody></table>";

#[test]
fn una_tabla_html_con_combinadas_y_colores_se_abre_editable_y_se_guarda_igual() {
    let e = abrir(PRESUPUESTO);
    assert_eq!(md_tabla::tablas_en_control(&leer(e.edit)).len(), 1, "es una tabla del control");
    assert_eq!(markdown(&e), PRESUPUESTO);
    assert_eq!(e.guardado, PRESUPUESTO, "abrir y cerrar no la reescribe");
    desmontar(e);
}

#[test]
fn las_combinadas_y_los_colores_se_leen_del_control() {
    let e = abrir(PRESUPUESTO);
    let t = modelo(&e, &tabla(&e));
    assert_eq!(t.formato(0, 0).columnas, 3);
    assert!(t.formato(0, 1).tapada && t.formato(0, 2).tapada);
    let obra = t.formato(1, 0);
    assert_eq!((obra.filas, obra.fondo, obra.letra), (2, Some(0xffc9c9), Some(0xe03131)));
    assert!(t.formato(2, 0).tapada);
    assert_eq!(t.formato(3, 2).fondo, Some(0xffec99));
    assert_eq!(t.alineacion_de(2, 2), Alineacion::Derecha);
    assert_eq!(t.filas[2][0], "", "la tapada no tiene texto");
    desmontar(e);
}

fn color_de_la_letra_en(e: &Estado, pos: usize) -> u32 {
    let doc = e.doc.as_ref().unwrap();
    // SAFETY: rango del documento vivo.
    unsafe { doc.Range(pos as i32, pos as i32 + 1).unwrap().GetFont().unwrap().GetForeColor().unwrap() as u32 }
}

#[test]
fn la_letra_de_una_celda_pintada_sobrevive_al_formato_en_vivo() {
    let mut e = abrir(PRESUPUESTO);
    pintar(&mut e, None);
    let texto = leer(e.edit);
    let en = |s: &str| texto[..texto.find(s).unwrap()].encode_utf16().count();
    assert_eq!(color_de_la_letra_en(&e, en("Obra gruesa")), bgr(0xe03131), "la suya");
    assert_eq!(color_de_la_letra_en(&e, en("4,30")), bgr(tabla_rtf::letra_sobre(0xffec99)), "oscura sobre el amarillo");
    assert_eq!(color_de_la_letra_en(&e, en("Hormigon")), bgr(e.estilos.tema.texto), "las demas, la del tema");
    // Y con el cursor yendo y viniendo (repintado de dos renglones).
    elegir(e.edit, en("Obra gruesa"), en("Obra gruesa"));
    let n = md_vivo::linea_de(&md_vivo::lineas(&texto), en("Obra gruesa"));
    pintar(&mut e, Some(&[n]));
    assert_eq!(color_de_la_letra_en(&e, en("Obra gruesa") + 2), bgr(0xe03131));
    desmontar(e);
}

#[test]
fn pegar_una_tabla_de_sheets_fuera_de_una_tabla_la_crea_con_sus_combinadas() {
    let mut e = abrir("Gastos\n");
    elegir(e.edit, 7, 7);
    pegar_tabla(&mut e, &leer_pegado(DE_SHEETS, None).unwrap());
    let md = markdown(&e);
    assert!(md.starts_with("Gastos\n<table>\n"), "{md}");
    assert!(md.contains(
        "<th colspan=\"3\" align=\"center\" style=\"background:#4a86e8;color:#ffffff\">**Mensualidad**</th>"
    ), "{md}");
    assert!(md.contains("<td rowspan=\"2\" style=\"background:#ffec99\">Enero</td>"), "{md}");
    assert!(md.contains("<td align=\"right\" style=\"color:#e03131\">45</td>"));
    desmontar(e);
    // Ida y vuelta: abierta otra vez, el mismo Markdown.
    let e = abrir(&md);
    assert_eq!(markdown(&e), md);
    desmontar(e);
}

#[test]
fn pegar_una_tabla_sin_colores_ni_combinadas_la_deja_en_barras() {
    let mut e = abrir("");
    pegar_tabla(&mut e, &leer_tsv("Partida\tImporte\nArena\t12,5\n").unwrap());
    assert_eq!(markdown(&e), "| Partida | Importe |\n|:---|:---|\n| Arena | 12,5 |\n");
    desmontar(e);
}

#[test]
fn pegar_dentro_de_una_tabla_rellena_desde_la_celda_del_cursor_y_la_agranda() {
    let mut e = abrir("| a | b |\n|:---|:---|\n| 1 | 2 |\n");
    ir_a_celda(&e, tabla(&e).desde, 1, 1);
    pegar_tabla(&mut e, &leer_tsv("x\ty\nz\tw").unwrap());
    assert_eq!(
        markdown(&e),
        "| a | b |  |\n|:---|:---|:---|\n| 1 | x | y |\n|  | z | w |\n"
    );
    assert_eq!(celda_del_cursor(&e).map(|(_, f, c)| (f, c)), Some((1, 1)));
    desmontar(e);
}

const TRES_POR_TRES: &str = "| a | b | c |\n|:---|:---|:---|\n| 1 | 2 | 3 |\n| 4 | 5 | 6 |\n";

#[test]
fn combinar_lo_elegido_y_separar_vuelve_a_las_barras() {
    let mut e = abrir(TRES_POR_TRES);
    elegir_celdas(&e, (1, 0), (2, 1));
    assert_eq!(se_puede(&e), (true, false));
    comando(&mut e, C_COMBINAR);
    let md = markdown(&e);
    // El texto junto en renglones, como lo junta el movil.
    assert!(md.contains("<td colspan=\"2\" rowspan=\"2\">1\n2\n4\n5</td>"), "{md}");
    assert!(md.contains("<td>3</td>\n  </tr>\n  <tr>\n    <td>6</td>"), "las tapadas no se escriben");
    // El cursor quedo en la combinada: el boton separa.
    assert_eq!(se_puede(&e), (false, true));
    combinar_o_separar(&mut e);
    assert_eq!(
        markdown(&e),
        "| a | b | c |\n|:---|:---|:---|\n| 1 2 4 5 |  | 3 |\n|  |  | 6 |\n"
    );
    desmontar(e);
}

#[test]
fn colorear_una_fila_y_una_columna_y_quitarlo_deja_la_tabla_como_estaba() {
    let mut e = abrir(TRES_POR_TRES);
    ir_a_celda(&e, tabla(&e).desde, 1, 1);
    comando(&mut e, C_FONDO + 5 + 1);
    let md = markdown(&e);
    assert_eq!(md.matches("background:#ffc9c9").count(), 3, "la fila entera\n{md}");
    comando(&mut e, C_LETRA + 10 + 3);
    let md = markdown(&e);
    assert_eq!(md.matches("color:#1971c2").count(), 3, "la columna entera\n{md}");
    assert!(md.contains("<td style=\"background:#ffc9c9;color:#1971c2\">2</td>"));
    comando(&mut e, C_FONDO + 5);
    comando(&mut e, C_LETRA + 10);
    assert_eq!(markdown(&e), TRES_POR_TRES);
    desmontar(e);
}

#[test]
fn la_paleta_de_la_barra_pinta_las_celdas_elegidas() {
    let mut e = abrir(TRES_POR_TRES);
    elegir_celdas(&e, (1, 1), (2, 2));
    let ids: Vec<u16> = entradas_de_color(&e).iter().map(|x| x.id).collect();
    assert_eq!(ids.len(), 10);
    comando(&mut e, ids[4]);
    let md = markdown(&e);
    assert_eq!(md.matches("background:#ffec99").count(), 4, "{md}");
    desmontar(e);
}

#[test]
fn fuera_de_una_tabla_o_con_una_sola_celda_no_se_combina_ni_se_pinta() {
    let mut e = abrir("solo texto");
    comando(&mut e, C_COMBINAR);
    comando(&mut e, C_FONDO + 1);
    combinar_o_separar(&mut e);
    assert_eq!(markdown(&e), "solo texto");
    assert_eq!(se_puede(&e), (false, false));
    desmontar(e);
    let mut e = abrir(TRES_POR_TRES);
    ir_a_celda(&e, tabla(&e).desde, 1, 1);
    comando(&mut e, C_COMBINAR);
    comando(&mut e, C_SEPARAR);
    assert_eq!(markdown(&e), TRES_POR_TRES);
    desmontar(e);
}

#[test]
fn cambiar_de_tema_no_vuelve_color_el_gris_de_la_cabecera() {
    let mut e = abrir(PRESUPUESTO);
    cambiar_tema(&mut e, Tema::claro());
    assert_eq!(markdown(&e), PRESUPUESTO);
    cambiar_tema(&mut e, Tema::oscuro());
    assert_eq!(markdown(&e), PRESUPUESTO);
    desmontar(e);
    let mut e = abrir(TRES_POR_TRES);
    cambiar_tema(&mut e, Tema::claro());
    assert_eq!(markdown(&e), TRES_POR_TRES);
    desmontar(e);
}

#[test]
fn una_fila_nueva_dentro_de_una_combinada_la_alarga() {
    let mut e = abrir(PRESUPUESTO);
    let t = tabla(&e);
    ir_a_celda(&e, t.desde, 1, 1);
    operar_tabla(&mut e, OpTabla::FilaDebajo);
    let md = markdown(&e);
    assert!(md.contains("rowspan=\"3\""), "{md}");
    desmontar(e);
}

// ---------------------------------------------------------------------------
// Muestras en PNG, a mano:
// `PIXPIN_MUESTRA=<carpeta> cargo test -p pixpin-notas muestra_tabla -- --ignored`

fn guardar_png(img: &pixpin_codec::ImagenRgba, nombre: &str) {
    let carpeta = std::env::var("PIXPIN_MUESTRA").unwrap_or_else(|_| ".".into());
    let png = pixpin_codec::imagen::codificar_png(img).unwrap();
    std::fs::write(std::path::Path::new(&carpeta).join(nombre), png).unwrap();
}

fn muestra_pegada(claro: bool, nombre: &str, paleta: bool) {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let mut e = abrir_con("# Gastos de la casa\nCopiado de la hoja de Sheets:\n", claro, (1000, 720));
    let fin = leer(e.edit).encode_utf16().count();
    elegir(e.edit, fin, fin);
    pegar_tabla(&mut e, &leer_pegado(DE_SHEETS, None).unwrap());
    // Y la fila de febrero pintada de verde con la paleta, como se haria a
    // mano; el cursor en una celda (salen los botones de la tabla).
    ir_a_celda(&e, tabla(&e).desde, 3, 1);
    comando(&mut e, C_FONDO + 5 + 2);
    // Y «Luz» y «Agua» combinadas a mano: el texto junto, en dos renglones.
    elegir_celdas(&e, (1, 1), (2, 1));
    comando(&mut e, C_COMBINAR);
    ir_a_celda(&e, tabla(&e).desde, 2, 2);
    actualizar_en_tabla(&mut e);
    pintar(&mut e, None);
    if paleta {
        clic(&mut e, Boton::Color, &mut |_| true);
    }
    guardar_png(&muestra(&e), nombre);
    desmontar(e);
}

#[test]
#[ignore]
fn muestra_tabla_pegada() {
    muestra_pegada(false, "nota-md-tabla-pegada-oscura.png", false);
    muestra_pegada(true, "nota-md-tabla-pegada-clara.png", false);
}

#[test]
#[ignore]
fn muestra_tabla_menu_de_color() {
    muestra_pegada(false, "nota-md-tabla-color-oscura.png", true);
    muestra_pegada(true, "nota-md-tabla-color-clara.png", true);
}

/// Lo que cuesta una tabla larga con colores y una combinada, frente a la
/// misma sin nada. A mano:
/// `cargo test -p pixpin-notas medir_tabla_coloreada -- --ignored --nocapture`.
#[test]
#[ignore]
fn medir_tabla_coloreada() {
    let mut gfm = String::from("# Notas\n| a | b | c |\n|:---|:---|:---|\n");
    for i in 0..60 {
        gfm.push_str(&format!("| fila {i} | **dato** | 3,20 |\n"));
    }
    let mut t = md_tabla::leer_gfm(&gfm).unwrap();
    t.combinar((1, 0), (2, 0));
    t.poner_fondo((0, 0), (0, 2), Some(0xa5d8ff));
    t.poner_letra((1, 2), (60, 2), Some(0xe03131));
    let con_color = format!("# Notas\n{}\n", md_tabla::a_texto(&t));
    for (que, md) in [("sin nada", gfm.as_str()), ("con color", con_color.as_str())] {
        let t0 = std::time::Instant::now();
        let mut e = abrir(md);
        let abrir_ms = t0.elapsed();
        let t0 = std::time::Instant::now();
        pintar(&mut e, None);
        let entero = t0.elapsed();
        let t0 = std::time::Instant::now();
        pintar(&mut e, Some(&[5, 30]));
        let dos = t0.elapsed();
        let t0 = std::time::Instant::now();
        let vuelta = markdown(&e);
        let a_md = t0.elapsed();
        assert_eq!(vuelta, md);
        println!("{que}: abrir {abrir_ms:?}, repintado {entero:?}, dos renglones {dos:?}, a Markdown {a_md:?}");
        desmontar(e);
    }
}

