//! Pruebas de la ventana sin ventana: que hace cada tecla y cada clic sobre
//! el estado, con un timeline de ejemplo en una carpeta temporal.

use super::*;

fn ejemplo(etiqueta: &str) -> (Estado, Ubicacion) {
    let carpeta = std::env::temp_dir().join(format!(
        "pixpin-timeline-pruebas-{etiqueta}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&carpeta);
    std::fs::create_dir_all(&carpeta).expect("carpeta");
    let almacen = Almacen::en(&carpeta);
    let ahora = Estado::ahora();
    let h = 3_600_000;
    for (k, (cuando, titulo)) in [
        (ahora - 2 * h, "Cafe con Ana"),
        (ahora - h, "Reunión de obra"),
        (ahora - 3 * 24 * h, "Reunion con el proveedor"),
        (ahora - 40 * 24 * h, "Paseo"),
    ]
    .into_iter()
    .enumerate()
    {
        let m = Momento::nuevo(format!("tl-{k}"), cuando, titulo.into(), String::new());
        almacen.anadir(&m).expect("anadir");
    }
    let e = Estado::con_almacen(almacen, Idioma::Espanol, carpeta.clone(), "PC".into());
    (e, Ubicacion::Portable { raiz: carpeta })
}

fn tecla_(e: &mut Estado, vk: u32, ctrl: bool, u: &Ubicacion) -> bool {
    let textos = Catalogo::nuevo(Idioma::Espanol);
    tecla(e, vk, ctrl, false, &textos, u, Idioma::Espanol)
}

fn hacer_(e: &mut Estado, a: Accion, u: &Ubicacion) -> bool {
    let textos = Catalogo::nuevo(Idioma::Espanol);
    hacer(e, a, &textos, u, Idioma::Espanol)
}

#[test]
fn lo_buscado_tapa_el_dia_y_en_lecciones_filtra_sus_tarjetas() {
    let (mut e, _u) = ejemplo("modo");
    e.cambiar_pestana(Pestana::Estado);
    e.dia = Some(e.hoy());
    assert!(matches!(e.modo(), Modo::Dia(_)));
    e.busqueda.poner("reunion");
    assert_eq!(e.modo(), Modo::Buscar);
    // Sin tildes y de todo el timeline, no solo de lo que se veia.
    assert_eq!(e.visibles().len(), 2);
    // Caso negativo: en «Lecciones» lo buscado no cambia de modo.
    e.cambiar_pestana(Pestana::Lecciones);
    assert_eq!(e.modo(), Modo::Lecciones);
}

#[test]
fn esc_va_por_capas_detalle_busqueda_dia_pestana_y_cierra() {
    let (mut e, u) = ejemplo("esc");
    e.cambiar_pestana(Pestana::Momentos);
    e.dia = Some(e.hoy());
    e.busqueda.poner("cafe");
    e.abrir_detalle(0);
    assert!(tecla_(&mut e, VK_ESCAPE, false, &u));
    assert!(e.detalle.is_none());
    assert!(tecla_(&mut e, VK_ESCAPE, false, &u));
    assert!(e.busqueda.texto.is_empty());
    assert!(tecla_(&mut e, VK_ESCAPE, false, &u));
    assert!(e.dia.is_none());
    assert!(tecla_(&mut e, VK_ESCAPE, false, &u));
    assert_eq!(e.pestana, Pestana::Hoy);
    // Caso negativo: en «Hoy» y sin nada, Esc cierra (devuelve false).
    assert!(!tecla_(&mut e, VK_ESCAPE, false, &u));
}

#[test]
fn las_flechas_del_detalle_siguen_la_lista_desde_la_que_se_abrio() {
    let (mut e, u) = ejemplo("detalle");
    e.cambiar_pestana(Pestana::Momentos);
    // En «Momentos» el orden es del mas nuevo al mas viejo por dias.
    let i = e
        .momentos
        .iter()
        .position(|m| m.id == "tl-0")
        .expect("cafe");
    e.abrir_detalle(i);
    let lista = e
        .detalle
        .as_ref()
        .map(|d| d.lista.clone())
        .expect("detalle");
    assert_eq!(lista.len(), 4);
    let antes = e.item_en_detalle().cloned();
    tecla_(&mut e, VK_ABAJO, false, &u);
    assert_ne!(e.item_en_detalle().cloned(), antes);
    // Caso negativo: en el borde de arriba no da la vuelta.
    tecla_(&mut e, VK_ARRIBA, false, &u);
    tecla_(&mut e, VK_ARRIBA, false, &u);
    tecla_(&mut e, VK_ARRIBA, false, &u);
    assert_eq!(e.detalle.as_ref().map(|d| d.pos), Some(0));
}

#[test]
fn ctrl_clic_en_estado_elige_el_dia_y_sin_ctrl_lo_abre() {
    let (mut e, u) = ejemplo("estado");
    e.cambiar_pestana(Pestana::Estado);
    let n = e.hoy().numero();
    e.ctrl = true;
    hacer_(&mut e, Accion::DiaDelEstado(n), &u);
    assert!(e.elegidos.contains(&e.hoy()));
    assert!(e.dia.is_none());
    e.ctrl = false;
    hacer_(&mut e, Accion::DiaDelEstado(n), &u);
    assert_eq!(e.dia, Some(e.hoy()));
    // Caso negativo: Ctrl+clic otra vez lo suelta.
    e.dia = None;
    e.ctrl = true;
    hacer_(&mut e, Accion::DiaDelEstado(n), &u);
    assert!(e.elegidos.is_empty());
}

#[test]
fn las_letras_buscan_en_momentos_y_van_a_la_caja_en_hoy() {
    let (mut e, u) = ejemplo("letras");
    letra(&mut e, 'h');
    assert_eq!(e.campo.texto, "h");
    assert!(e.busqueda.texto.is_empty());
    hacer_(&mut e, Accion::Pestana(Pestana::Momentos), &u);
    letra(&mut e, 'p');
    assert!(e.buscando);
    assert_eq!(e.busqueda.texto, "p");
    // Caso negativo: con el detalle abierto, las letras no van a ningun sitio.
    e.abrir_detalle(0);
    letra(&mut e, 'x');
    assert_eq!(e.busqueda.texto, "p");
}

#[test]
fn ctrl_numero_cambia_de_pestana_y_ctrl_f_va_al_buscador() {
    let (mut e, u) = ejemplo("ctrl");
    tecla_(&mut e, VK_4, true, &u);
    assert_eq!(e.pestana, Pestana::Lecciones);
    tecla_(&mut e, VK_2, true, &u);
    assert_eq!(e.pestana, Pestana::Momentos);
    tecla_(&mut e, VK_F, true, &u);
    assert!(e.buscando);
    // Caso negativo: sin Ctrl, el 4 es una tecla mas y no cambia nada.
    tecla_(&mut e, VK_ESCAPE, false, &u);
    tecla_(&mut e, VK_4, false, &u);
    assert_eq!(e.pestana, Pestana::Momentos);
}

#[test]
fn marcar_desmarcar_y_marcar_reusa_la_leccion_y_no_crea_otra() {
    use super::lecciones::{AlMarcar, al_marcar};
    let mut m = Momento::nuevo("tl-1".into(), 0, "Algo".into(), String::new());
    let existe = |id: &str| id == "lec-1";
    assert_eq!(al_marcar(&m, &existe), AlMarcar::Crear);
    m.leccion = Some("lec-1".into());
    assert_eq!(al_marcar(&m, &existe), AlMarcar::Desmarcar);
    // Desmarcado: guarda cual era, y al volver a marcar se reusa.
    m.leccion_previa = m.leccion.take();
    assert_eq!(al_marcar(&m, &existe), AlMarcar::Reusar("lec-1".into()));
    // Caso negativo: si esa leccion ya no existe (se borro), se crea otra.
    let ninguna = |_: &str| false;
    assert_eq!(al_marcar(&m, &ninguna), AlMarcar::Crear);
}
