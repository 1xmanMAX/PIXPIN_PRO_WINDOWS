//! Las pruebas de la ventana de lecciones v2: lo que no necesita pantalla
//! (filtros, atajos, columnas, borrar con deshacer, la barra rapida) y dos
//! muestras `#[ignore]` que pintan la ventana a un PNG para mirarla.

use super::*;
use pixpin_lecciones::leccion::{DIA, TIPO_ERROR};

fn entrada(l: Leccion, ficha: &str, nombre: &str) -> Entrada {
    Entrada {
        leccion: l,
        mensaje: Default::default(),
        ficha: ficha.into(),
        nombre_chat: nombre.into(),
        general: ficha == "g",
        archivo: std::path::PathBuf::from("x.leccion"),
    }
}

/// 4-oct-2026, a media manana.
const AHORA: i64 = 20_730 * DIA + 10 * 3_600_000;

fn ejemplos() -> Vec<Entrada> {
    vec![
        entrada(
            Leccion {
                tipo: TIPO_ERROR.into(),
                area: "Construcción".into(),
                gravedad: 2,
                que_paso: "Pintamos el muro norte sin sellar y a la semana se marcó la grieta otra vez. Hubo que lijar y repintar todo el paño.".into(),
                por_que: "Prisa por entregar el viernes. No había sellador en obra y nadie lo pidió con la pintura.".into(),
                proxima: "Comprar el sellador junto con la pintura y esperar 24 h antes de la primera mano.".into(),
                etiquetas: vec!["grieta".into(), "pintura".into()],
                etiquetas_auto: vec!["muro".into()],
                repeticiones: vec![AHORA - 6 * DIA],
                caja: 3,
                repasar: AHORA - DIA,
                tocada: AHORA - 3 * DIA,
                ..Leccion::nueva("a", AHORA - 40 * DIA, "Sellar grietas antes de pintar")
            },
            "f1",
            "Obra Miraflores",
        ),
        entrada(
            Leccion {
                area: "Trabajo".into(),
                proxima: "Revisar la escala en la vista previa".into(),
                repasar: AHORA + 5 * DIA,
                tocada: AHORA - 14 * DIA,
                ..Leccion::nueva("b", AHORA - 14 * DIA, "Revisar la escala antes de imprimir")
            },
            "g",
            "Mensajes guardados",
        ),
        entrada(
            Leccion {
                area: "Estudio".into(),
                gravedad: 3,
                repeticiones: vec![1, 2],
                repasar: AHORA - 2 * DIA,
                tocada: AHORA - 35 * DIA,
                ..Leccion::nueva("c", AHORA - 35 * DIA, "Guardar la tesis en dos sitios")
            },
            "f2",
            "Tesis",
        ),
        entrada(
            Leccion {
                area: "Construcción".into(),
                repasar: AHORA + 60 * DIA,
                tocada: AHORA - 90 * DIA,
                ..Leccion::nueva("d", AHORA - 90 * DIA, "Medir dos veces el vano de la puerta")
            },
            "f1",
            "Obra Miraflores",
        ),
    ]
}

fn estado() -> Estado {
    let mut e = Estado::nuevo(Pedido {
        ubicacion: Ubicacion::Portable { raiz: std::env::temp_dir().join("pixpin-lecciones-v2-sin-disco") },
        idioma: Idioma::Espanol,
        aparato: "PC01".into(),
        proyecto: None,
        consulta: None,
        seleccion: None,
    });
    e.todas = ejemplos();
    e.indices = e.todas.iter().map(|x| Indice::nuevo(x.leccion.clone())).collect();
    e.proyectos = vec![("f1".into(), "Obra Miraflores".into()), ("f2".into(), "Tesis".into())];
    e.general = Some("g".into());
    e.ahora = AHORA;
    let lecciones: Vec<Leccion> = e.todas.iter().map(|x| x.leccion.clone()).collect();
    e.hoy = Repaso::de_hoy(&lecciones, AHORA, 20);
    e.filtrar();
    e
}

#[test]
fn los_filtros_cuentan_y_filtran_y_la_eleccion_sigue_si_se_ve() {
    let mut e = estado();
    let f = e.filtros();
    assert_eq!(f[0], (Filtro::Todas, 4));
    assert_eq!(f[1], (Filtro::Repetidas, 2));
    assert_eq!(f[2], (Filtro::Graves, 1));
    assert_eq!(f[3], (Filtro::Area("Construcción".into()), 2), "el area con mas va primero");
    assert_eq!(e.sel.as_deref(), Some("a"), "sin eleccion, la primera");
    e.sel = Some("d".into());
    e.filtro = Filtro::Area("Construcción".into());
    e.filtrar();
    assert_eq!(e.visibles, vec![0, 3]);
    assert_eq!(e.sel.as_deref(), Some("d"), "la elegida se ve: sigue elegida");
    e.filtro = Filtro::Graves;
    e.filtrar();
    assert_eq!(e.visibles, vec![2]);
    assert_eq!(e.sel.as_deref(), Some("c"), "la elegida ya no se ve: la primera");
    // Caso negativo: una busqueda sin nada deja la lista vacia y sin eleccion.
    e.filtro = Filtro::Todas;
    e.consulta.poner("zzzz qqqq");
    e.filtrar();
    assert!(e.visibles.is_empty());
    assert_eq!(e.sel, None);
}

#[test]
fn las_flechas_mueven_la_eleccion_sin_salirse() {
    let mut e = estado();
    e.mover_sel(1);
    assert_eq!(e.sel.as_deref(), Some("b"));
    e.mover_sel(10);
    assert_eq!(e.sel.as_deref(), Some("d"));
    // Caso negativo: arriba del todo no da la vuelta.
    e.mover_sel(-10);
    e.mover_sel(-1);
    assert_eq!(e.sel.as_deref(), Some("a"));
}

#[test]
fn los_atajos_de_una_tecla_solo_valen_sin_caja_de_texto() {
    use Accion as A;
    let a = |foco, vk, ctrl, shift| atajo(foco, false, false, vk, ctrl, shift);
    assert_eq!(a(None, VK_R, false, false), Some(Atajo::Accion(A::Repasar)));
    assert_eq!(a(None, VK_P, false, false), Some(Atajo::Accion(A::Pinear)));
    assert_eq!(a(None, VK_SUPRIMIR, false, false), Some(Atajo::Accion(A::Borrar)));
    assert_eq!(a(None, VK_Z, true, false), Some(Atajo::Accion(A::Deshacer)));
    assert_eq!(a(Some(Foco::Paso), VK_F, true, false), Some(Atajo::Accion(A::FocoBuscar)));
    assert_eq!(a(Some(Foco::Rapida), VK_ENTRAR, false, false), Some(Atajo::Accion(A::Guardar)));
    assert_eq!(a(Some(Foco::Rapida), VK_D, true, true), Some(Atajo::Accion(A::DictarRapida)));
    assert_eq!(a(Some(Foco::Paso), VK_ENTRAR, false, false), Some(Atajo::Terminar));
    assert_eq!(a(None, VK_ABAJO, false, false), Some(Atajo::Abajo));
    // Caso negativo: escribiendo, R, P y Supr son de la caja; Mayus+Intro
    // en un bloque parte el renglon; Ctrl+Z en un bloque no deshace borrar.
    assert_eq!(a(Some(Foco::Rapida), VK_R, false, false), None);
    assert_eq!(a(Some(Foco::Titulo), VK_P, false, false), None);
    assert_eq!(a(Some(Foco::Buscar), VK_SUPRIMIR, false, false), None);
    assert_eq!(a(Some(Foco::Paso), VK_ENTRAR, false, true), None);
    assert_eq!(a(Some(Foco::Proxima), VK_Z, true, false), None);
    assert_eq!(a(Some(Foco::Paso), VK_ABAJO, false, false), None);
}

#[test]
fn en_el_repaso_valen_1_2_3_espacio_y_s() {
    use Accion as A;
    let r = |foco, mostrada, vk, ctrl| atajo(foco, true, mostrada, vk, ctrl, false);
    assert_eq!(r(None, true, VK_1, false), Some(Atajo::Accion(A::Nota(0))));
    assert_eq!(r(None, true, VK_3, false), Some(Atajo::Accion(A::Nota(2))));
    assert_eq!(r(None, false, VK_ESPACIO, false), Some(Atajo::Accion(A::Mostrar)));
    assert_eq!(r(None, true, VK_S, false), Some(Atajo::Accion(A::Saltar)));
    assert_eq!(r(None, true, VK_ENTRAR, false), Some(Atajo::Accion(A::Siguiente)));
    assert_eq!(r(Some(Foco::Respuesta), false, VK_ENTRAR, false), Some(Atajo::Accion(A::Mostrar)));
    assert_eq!(r(Some(Foco::Respuesta), false, VK_D, true), Some(Atajo::Accion(A::DictarRespuesta)));
    assert_eq!(r(Some(Foco::Respuesta), false, VK_ESCAPE, false), Some(Atajo::Atras));
    // Caso negativo: escribiendo la respuesta, 1/2/3, S y Espacio son letras;
    // y R (repasar) o P (pinear) no hacen nada dentro del repaso.
    assert_eq!(r(Some(Foco::Respuesta), false, VK_1, false), None);
    assert_eq!(r(Some(Foco::Respuesta), false, VK_S, false), None);
    assert_eq!(r(Some(Foco::Respuesta), false, VK_ESPACIO, false), None);
    assert_eq!(r(None, true, VK_R, false), None);
    assert_eq!(r(None, true, VK_P, false), None);
}

#[test]
fn las_columnas_llenan_el_ancho_y_se_encogen_en_pantallas_estrechas() {
    let (l, f, d) = columnas(1280.0, 820.0, 112.0, 1.0);
    assert_eq!((l.ancho, d.ancho), (390.0, 290.0));
    assert!((l.ancho + f.ancho + d.ancho - 1280.0).abs() < 0.01);
    assert_eq!(f.x, l.ancho);
    assert_eq!(l.alto, 820.0 - 112.0);
    let (l2, f2, d2) = columnas(1000.0, 700.0, 112.0, 1.0);
    assert_eq!((l2.ancho, d2.ancho), (310.0, 240.0));
    assert!(f2.ancho >= 400.0, "la ficha conserva sitio para leer");
    // Caso negativo: una ventana diminuta no da anchos negativos.
    let (_, f3, _) = columnas(300.0, 50.0, 112.0, 1.0);
    assert!(f3.ancho >= 0.0 && f3.alto >= 0.0);
}

#[test]
fn borrar_se_puede_deshacer_y_la_de_debajo_queda_elegida() {
    let mut e = estado();
    e.sel = Some("b".into());
    e.borrar_elegida();
    assert!(e.borrando.as_ref().is_some_and(|(x, _)| x.leccion.id == "b"));
    assert!(!e.visibles.iter().any(|i| e.todas[*i].leccion.id == "b"), "ya no se ve");
    assert_eq!(e.sel.as_deref(), Some("c"), "la de debajo");
    assert_eq!(e.filtros()[0].1, 3, "ni cuenta");
    e.deshacer();
    assert!(e.borrando.is_none());
    assert_eq!(e.sel.as_deref(), Some("b"));
    assert_eq!(e.visibles.len(), 4);
    // Caso negativo: deshacer sin nada borrado no hace nada.
    e.deshacer();
    assert_eq!(e.visibles.len(), 4);
}

#[test]
fn la_barra_rellena_sola_y_lo_cambiado_a_mano_gana() {
    let mut e = estado();
    e.rapida.campo.poner("La escalera de Miraflores resbala con el polvo de yeso; barrer antes de bajar material");
    e.recalcular_rapida();
    assert_eq!(e.gravedad_rapida(), 2);
    assert_eq!(e.proyecto_rapida().as_deref(), Some("f1"));
    hacer(&mut e, Accion::CicloGravedad);
    assert_eq!(e.gravedad_rapida(), 3);
    hacer(&mut e, Accion::CicloGravedad);
    assert_eq!(e.gravedad_rapida(), 1, "da la vuelta");
    // Elegir en el menu: «Sin proyecto» es el ultimo.
    hacer(&mut e, Accion::AbrirMenu(Menu::ProyectoRapida));
    let n = e.opciones_del_menu(Menu::ProyectoRapida).len();
    hacer(&mut e, Accion::Opcion(n - 1));
    assert_eq!(e.proyecto_rapida(), None);
    assert!(e.menu.is_none());
    hacer(&mut e, Accion::AbrirMenu(Menu::AreaRapida));
    hacer(&mut e, Accion::Opcion(2));
    assert_eq!(e.area_rapida().as_deref(), Some("Estudio"));
    // Caso negativo: un proyecto propuesto que ya no existe no se usa.
    e.rapida.proyecto = None;
    e.rapida.relleno.proyecto = Some("borrado".into());
    assert_eq!(e.proyecto_rapida(), None);
}

#[test]
fn la_parecida_se_ofrece_como_me_volvio_a_pasar() {
    let mut e = estado();
    e.rapida.campo.poner("sellar las grietas antes de pintar el muro");
    e.recalcular_rapida();
    assert_eq!(e.rapida.parecida.as_ref().map(|l| l.id.as_str()), Some("a"));
    // Caso negativo: una frase de otra cosa no ofrece ninguna.
    e.rapida.campo.poner("comprar pan");
    e.recalcular_rapida();
    assert!(e.rapida.parecida.is_none());
}

#[test]
fn el_repaso_avanza_cuenta_y_acaba() {
    let mut e = estado();
    e.empezar_repaso();
    let r = e.repaso.as_ref().unwrap();
    assert_eq!(r.total(), 2, "las que tocan: a y c");
    assert_eq!(r.actual(), Some("c"), "la grave primero");
    assert_eq!(e.foco, Some(Foco::Respuesta));
    // Siguiente sin destapar destapa; sin nota, avisa y no pasa.
    hacer(&mut e, Accion::Siguiente);
    assert!(e.repaso.as_ref().unwrap().mostrada);
    hacer(&mut e, Accion::Siguiente);
    assert_eq!(e.repaso.as_ref().unwrap().actual(), Some("c"));
    assert!(e.aviso.is_some());
    // Saltar no cuenta.
    hacer(&mut e, Accion::Saltar);
    let r = e.repaso.as_ref().unwrap();
    assert_eq!((r.actual(), r.hechas, r.mostrada), (Some("a"), 0, false));
    hacer(&mut e, Accion::Saltar);
    assert_eq!(e.repaso.as_ref().unwrap().actual(), None, "terminado");
    hacer(&mut e, Accion::Siguiente);
    assert!(e.repaso.is_none(), "Siguiente al acabar vuelve a la lista");
    // Caso negativo: sin nada que repasar no se entra.
    e.hoy.clear();
    e.empezar_repaso();
    assert!(e.repaso.is_none());
}

#[test]
fn cada_boton_del_repaso_dice_cuando_vuelve() {
    let tx = Catalogo::nuevo(Idioma::Espanol);
    let l = Leccion { caja: 3, ..Leccion::nueva("x", 0, "x") };
    let dias: Vec<String> = pixpin_lecciones::Nota::TODAS
        .iter()
        .map(|n| repaso::cuando_vuelve(&tx, Repaso::dias_hasta(&l, *n)))
        .collect();
    assert_eq!(dias, vec!["Vuelve en 30 días", "Vuelve en 7 días", "Vuelve mañana"]);
    // Caso negativo: la clave existe (no sale el nombre de la clave).
    assert!(!dias[0].contains("lec2-"));
}

#[test]
fn hace_y_el_proximo_repaso_se_leen_bien() {
    let tx = Catalogo::nuevo(Idioma::Espanol);
    assert_eq!(pintar::hace_texto(&tx, crate::lecciones::ui::Hace::Dias(3)), "hace 3 días");
    assert_eq!(pintar::hace_texto(&tx, crate::lecciones::ui::Hace::Semanas(1)), "hace 1 semana");
    assert_eq!(pintar::fecha_corta(&tx, 20_724 * DIA), "28 sep");
    let l = Leccion { repasar: AHORA - DIA, ..Leccion::nueva("x", 0, "x") };
    assert_eq!(pintar::proximo_repaso(&tx, &l, AHORA), "Próximo repaso: hoy");
    let l = Leccion { repasar: AHORA + 5 * DIA, ..l };
    assert_eq!(pintar::proximo_repaso(&tx, &l, AHORA), "Próximo repaso: en 5 días");
    // Caso negativo: en ingles no sale el castellano.
    let en = Catalogo::nuevo(Idioma::Ingles);
    assert_ne!(pintar::hace_texto(&en, crate::lecciones::ui::Hace::Ayer), "ayer");
}

fn muestra(nombre: &str, e: &mut Estado, ancho: u32, alto: u32) {
    let marco = Rect { x: 0, y: 0, ancho, alto };
    crate::ventanita::muestra(nombre, ancho, alto, |p, _| pintar::todo(e, p, marco, 1.0));
}

#[test]
#[ignore = "necesita GPU; ejecutar con --ignored y mirar el PNG"]
fn muestra_de_la_ventana_v2() {
    let mut e = estado();
    e.rapida.campo.poner("La escalera del sótano en Miraflores resbala con el polvo de yeso; barrer antes de bajar material");
    e.rapida.fotos.push(("x.png".into(), Vec::new()));
    e.recalcular_rapida();
    e.foco = Some(Foco::Rapida);
    e.botones.raton = (700.0, 400.0);
    muestra("lecciones-v2", &mut e, 1280, 820);
    // Editando «Por que», con el menu de area abierto.
    e.foco = None;
    e.editar(Foco::PorQue);
    e.menu = Some(Menu::AreaFicha);
    muestra("lecciones-v2-editando", &mut e, 1280, 820);
    // Borrada, con «Deshacer».
    e.menu = None;
    e.foco = None;
    e.borrar_elegida();
    muestra("lecciones-v2-deshacer", &mut e, 1280, 820);
    // Estrecha (1000 x 700).
    e.deshacer();
    muestra("lecciones-v2-estrecha", &mut e, 1000, 700);
}

#[test]
#[ignore = "necesita GPU; ejecutar con --ignored y mirar el PNG"]
fn muestra_del_repaso_v2() {
    let mut e = estado();
    e.empezar_repaso();
    // La primera es «c» (grave); se salta para ensenar la de la maqueta.
    hacer(&mut e, Accion::Saltar);
    if let Some(r) = e.repaso.as_mut() {
        r.respuesta.poner("Sellar la grieta primero y dejar secar antes de pintar.");
    }
    e.foco = Some(Foco::Respuesta);
    muestra("lecciones-v2-repaso-pensar", &mut e, 1280, 820);
    hacer(&mut e, Accion::Mostrar);
    hacer(&mut e, Accion::Nota(0));
    muestra("lecciones-v2-repaso", &mut e, 900, 700);
    hacer(&mut e, Accion::Saltar);
    muestra("lecciones-v2-repaso-hecho", &mut e, 900, 700);
}
