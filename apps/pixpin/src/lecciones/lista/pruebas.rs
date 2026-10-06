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
        ubicacion: Ubicacion::Portable {
            raiz: std::env::temp_dir().join("pixpin-lecciones-v2-sin-disco"),
        },
        idioma: Idioma::Espanol,
        aparato: "PC01".into(),
        proyecto: None,
        consulta: None,
        seleccion: None,
    });
    e.todas = ejemplos();
    e.indices = e
        .todas
        .iter()
        .map(|x| Indice::nuevo(x.leccion.clone()))
        .collect();
    e.proyectos = vec![
        ("f1".into(), "Obra Miraflores".into()),
        ("f2".into(), "Tesis".into()),
    ];
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
    assert_eq!(f.len(), 3, "sin fichas por area (simplificado 5-oct-2026)");
    assert_eq!(e.sel.as_deref(), Some("a"), "sin eleccion, la primera");
    e.sel = Some("c".into());
    e.filtro = Filtro::Repetidas;
    e.filtrar();
    assert_eq!(e.visibles, vec![0, 2]);
    assert_eq!(
        e.sel.as_deref(),
        Some("c"),
        "la elegida se ve: sigue elegida"
    );
    e.sel = Some("a".into());
    e.filtro = Filtro::Graves;
    e.filtrar();
    assert_eq!(e.visibles, vec![2]);
    assert_eq!(
        e.sel.as_deref(),
        Some("c"),
        "la elegida ya no se ve: la primera"
    );
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
    assert_eq!(
        a(None, VK_SUPRIMIR, false, false),
        Some(Atajo::Accion(A::Borrar))
    );
    assert_eq!(a(None, VK_Z, true, false), Some(Atajo::Accion(A::Deshacer)));
    assert_eq!(
        a(Some(Foco::Paso), VK_F, true, false),
        Some(Atajo::Accion(A::FocoBuscar))
    );
    assert_eq!(
        a(Some(Foco::Rapida), VK_ENTRAR, false, false),
        Some(Atajo::Accion(A::Guardar))
    );
    assert_eq!(
        a(Some(Foco::Rapida), VK_D, true, true),
        Some(Atajo::Accion(A::DictarRapida))
    );
    assert_eq!(
        a(Some(Foco::Paso), VK_ENTRAR, false, false),
        Some(Atajo::Terminar)
    );
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
    assert_eq!(
        r(None, false, VK_ESPACIO, false),
        Some(Atajo::Accion(A::Mostrar))
    );
    assert_eq!(r(None, true, VK_S, false), Some(Atajo::Accion(A::Saltar)));
    assert_eq!(
        r(None, true, VK_ENTRAR, false),
        Some(Atajo::Accion(A::Siguiente))
    );
    assert_eq!(
        r(Some(Foco::Respuesta), false, VK_ENTRAR, false),
        Some(Atajo::Accion(A::Mostrar))
    );
    assert_eq!(
        r(Some(Foco::Respuesta), false, VK_D, true),
        Some(Atajo::Accion(A::DictarRespuesta))
    );
    assert_eq!(
        r(Some(Foco::Respuesta), false, VK_ESCAPE, false),
        Some(Atajo::Atras)
    );
    // Caso negativo: escribiendo la respuesta, 1/2/3, S y Espacio son letras;
    // y R (repasar) o P (pinear) no hacen nada dentro del repaso.
    assert_eq!(r(Some(Foco::Respuesta), false, VK_1, false), None);
    assert_eq!(r(Some(Foco::Respuesta), false, VK_S, false), None);
    assert_eq!(r(Some(Foco::Respuesta), false, VK_ESPACIO, false), None);
    assert_eq!(r(None, true, VK_R, false), None);
    assert_eq!(r(None, true, VK_P, false), None);
}

#[test]
fn las_dos_columnas_llenan_el_ancho_y_la_ficha_se_queda_lo_demas() {
    let (l, f) = columnas(1280.0, 820.0, 112.0, 1.0);
    assert_eq!(l.ancho, 390.0);
    assert!((l.ancho + f.ancho - 1280.0).abs() < 0.01);
    assert_eq!(f.x, l.ancho);
    assert_eq!(l.alto, 820.0 - 112.0);
    assert!(
        f.ancho >= 880.0,
        "sin la columna de datos, la ficha gana sus 290 px"
    );
    let (l2, f2) = columnas(1000.0, 700.0, 112.0, 1.0);
    assert_eq!(l2.ancho, 310.0);
    assert!(f2.ancho >= 650.0);
    // Caso negativo: una ventana diminuta no da anchos negativos ni deja
    // la lista mas ancha que la ventana.
    let (l3, f3) = columnas(200.0, 50.0, 112.0, 1.0);
    assert!(f3.ancho >= 0.0 && f3.alto >= 0.0);
    assert!(l3.ancho <= 200.0);
}

/// Si dos rectangulos se pisan.
fn se_pisan(a: RectF, b: RectF) -> bool {
    a.x < b.x + b.ancho && b.x < a.x + a.ancho && a.y < b.y + b.alto && b.y < a.y + a.alto
}

#[test]
fn la_fila_de_datos_cabe_en_una_linea_sin_pisarse_y_con_objetivos_de_40() {
    let f = fila_de_datos(100.0, 50.0, 800.0, 110.0, Some(44.0), 220.0, 130.0, 1.0);
    assert_eq!(f.alto, 40.0);
    let piezas = [f.gravedad, f.veces.unwrap(), f.otra_vez, f.mas];
    for (i, a) in piezas.iter().enumerate() {
        assert!(a.alto >= 40.0, "objetivo de 40 px");
        assert_eq!(a.y, 50.0, "todas en la misma linea");
        for b in &piezas[i + 1..] {
            assert!(!se_pisan(*a, *b), "{a:?} pisa {b:?}");
        }
    }
    assert_eq!(f.gravedad.x, 100.0, "la gravedad empieza la fila");
    assert_eq!(
        f.mas.x + f.mas.ancho,
        900.0,
        "«Mas campos…» al borde derecho"
    );
    // Sin repeticiones no hay «1×» y el boton se acerca.
    let g = fila_de_datos(100.0, 50.0, 800.0, 110.0, None, 220.0, 130.0, 1.0);
    assert_eq!(g.veces, None);
    assert!(g.otra_vez.x < f.otra_vez.x);
    // Caso negativo: estrecha, «Mas campos…» baja a otro renglon en vez de
    // montarse sobre «Me volvio a pasar».
    let h = fila_de_datos(0.0, 0.0, 420.0, 110.0, Some(44.0), 220.0, 130.0, 1.0);
    assert!(!se_pisan(h.mas, h.otra_vez));
    assert!(h.mas.y >= 40.0 && h.alto >= h.mas.y + h.mas.alto);
}

#[test]
fn la_tarjeta_de_repasar_es_de_una_linea_con_el_boton_dentro() {
    let (caja, b) = tarjeta_de_repaso(14.0, 120.0, 360.0, 130.0, 1.0);
    assert!(caja.alto <= 56.0, "una linea, no la tarjeta alta de antes");
    assert!(b.alto >= 40.0);
    assert!(b.x >= caja.x && b.x + b.ancho <= caja.x + caja.ancho);
    assert!(b.y >= caja.y && b.y + b.alto <= caja.y + caja.alto);
    assert!(
        (caja.x + caja.ancho - (b.x + b.ancho)) < 10.0,
        "el boton a la derecha"
    );
    // Caso negativo: un boton mas ancho que la tarjeta no se sale.
    let (caja, b) = tarjeta_de_repaso(0.0, 0.0, 100.0, 300.0, 1.0);
    assert!(b.x >= caja.x && b.x + b.ancho <= caja.x + caja.ancho);
}

#[test]
fn mas_campos_parte_las_etiquetas_en_renglones_y_nada_se_pisa() {
    let etiquetas: Vec<String> = ["general", "✨ topografía", "obra", "pintura", "muro"]
        .iter()
        .map(|t| t.to_string())
        .collect();
    let q = PedidoMasCampos {
        rotulos: ["Área", "Proyecto", "Etiquetas"],
        area: "Vida diaria",
        etiquetas: &etiquetas,
        escribiendo: false,
        anadir: "+ Añadir",
        relacionadas: 2,
        completa: "Tipo, causas y palabras…",
    };
    // Una letra de 7 px: basta para probar sin pantalla.
    let medir = |t: &str| t.chars().count() as f32 * 7.0;
    let d = disponer_mas_campos(&q, medir, 100.0, 200.0, 420.0, 1.0);
    let fin = 100.0 + 420.0;
    assert_eq!(d.etiquetas.len(), etiquetas.len() + 1, "y «+ Añadir»");
    assert_eq!(d.etiquetas.last().unwrap().1, PiezaEtiqueta::Anadir);
    assert!(
        d.etiquetas.iter().any(|(r, _)| r.y > d.filas[2]),
        "en 420 px no caben en un renglon"
    );
    let mut todo: Vec<RectF> = d.etiquetas.iter().map(|(r, _)| *r).collect();
    todo.push(d.area);
    todo.extend(d.relacionadas.iter().copied());
    todo.push(d.completa);
    for (i, a) in todo.iter().enumerate() {
        assert!(a.alto >= 40.0, "objetivo de 40 px: {a:?}");
        assert!(a.x + a.ancho <= fin + 0.01, "se sale: {a:?}");
        assert!(a.y >= d.caja.y && a.y + a.alto <= d.caja.y + d.caja.alto);
        for b in &todo[i + 1..] {
            assert!(!se_pisan(*a, *b), "{a:?} pisa {b:?}");
        }
    }
    assert!(
        d.x_valor > d.x_rotulo,
        "los valores a la derecha del rotulo"
    );
    // Caso negativo: sin relacionadas no hay rotulo ni filas, y escribiendo
    // una etiqueta la caja sustituye a «+ Añadir».
    let q2 = PedidoMasCampos {
        relacionadas: 0,
        escribiendo: true,
        ..q
    };
    let d2 = disponer_mas_campos(&q2, medir, 100.0, 200.0, 420.0, 1.0);
    assert!(d2.rotulo_relacionadas.is_none() && d2.relacionadas.is_empty());
    assert!(d2.caja.alto < d.caja.alto);
    assert_eq!(d2.etiquetas.last().unwrap().1, PiezaEtiqueta::Escribiendo);
}

#[test]
fn la_gravedad_da_la_vuelta_y_mas_campos_se_despliega_y_se_pliega() {
    assert_eq!(siguiente_gravedad(1), 2);
    assert_eq!(siguiente_gravedad(2), 3);
    assert_eq!(siguiente_gravedad(3), 1, "Grave vuelve a Leve");
    // Caso negativo: un valor raro de otro aparato no se sale de 1..=3.
    assert_eq!(siguiente_gravedad(0), 2);
    assert_eq!(siguiente_gravedad(9), 1);
    let mut e = estado();
    assert!(!e.mas_campos, "plegado al abrir");
    hacer(&mut e, Accion::MasCampos);
    assert!(e.mas_campos);
    // Sigue desplegado al pasar a otra leccion.
    e.mover_sel(1);
    assert!(e.mas_campos);
    hacer(&mut e, Accion::MasCampos);
    assert!(!e.mas_campos);
    // Los rotulos nuevos existen en los dos idiomas (no sale la clave).
    for idioma in [Idioma::Espanol, Idioma::Ingles] {
        let tx = Catalogo::nuevo(idioma);
        for k in ["lecs-menos-campos", "lecs-ficha-completa"] {
            assert_ne!(tx.t(k), k);
        }
    }
}

#[test]
fn una_fila_medio_tapada_solo_responde_en_lo_que_se_ve() {
    let vista = RectF {
        x: 0.0,
        y: 200.0,
        ancho: 390.0,
        alto: 400.0,
    };
    let fila = RectF {
        x: 8.0,
        y: 180.0,
        ancho: 374.0,
        alto: 56.0,
    };
    let z = recortar(fila, vista).unwrap();
    assert_eq!((z.y, z.alto), (200.0, 36.0));
    // Caso negativo: una fila que no se ve no responde.
    let fuera = RectF { y: 100.0, ..fila };
    assert_eq!(recortar(fuera, vista), None);
}

#[test]
fn borrar_se_puede_deshacer_y_la_de_debajo_queda_elegida() {
    let mut e = estado();
    e.sel = Some("b".into());
    e.borrar_elegida();
    assert!(
        e.borrando
            .as_ref()
            .is_some_and(|(x, _)| x.leccion.id == "b")
    );
    assert!(
        !e.visibles.iter().any(|i| e.todas[*i].leccion.id == "b"),
        "ya no se ve"
    );
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
    e.rapida.campo.poner(
        "La escalera de Miraflores resbala con el polvo de yeso; barrer antes de bajar material",
    );
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
    e.rapida
        .campo
        .poner("sellar las grietas antes de pintar el muro");
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
    let l = Leccion {
        caja: 3,
        ..Leccion::nueva("x", 0, "x")
    };
    let dias: Vec<String> = pixpin_lecciones::Nota::TODAS
        .iter()
        .map(|n| repaso::cuando_vuelve(&tx, Repaso::dias_hasta(&l, *n)))
        .collect();
    assert_eq!(
        dias,
        vec!["Vuelve en 30 días", "Vuelve en 7 días", "Vuelve mañana"]
    );
    // Caso negativo: la clave existe (no sale el nombre de la clave).
    assert!(!dias[0].contains("lec2-"));
}

#[test]
fn hace_y_el_proximo_repaso_se_leen_bien() {
    let tx = Catalogo::nuevo(Idioma::Espanol);
    assert_eq!(
        pintar::hace_texto(&tx, crate::lecciones::ui::Hace::Dias(3)),
        "hace 3 días"
    );
    assert_eq!(
        pintar::hace_texto(&tx, crate::lecciones::ui::Hace::Semanas(1)),
        "hace 1 semana"
    );
    let l = Leccion {
        repasar: AHORA - DIA,
        ..Leccion::nueva("x", 0, "x")
    };
    assert_eq!(
        pintar::proximo_repaso(&tx, &l, AHORA),
        "Próximo repaso: hoy"
    );
    let l = Leccion {
        repasar: AHORA + 5 * DIA,
        ..l
    };
    assert_eq!(
        pintar::proximo_repaso(&tx, &l, AHORA),
        "Próximo repaso: en 5 días"
    );
    // Caso negativo: en ingles no sale el castellano.
    let en = Catalogo::nuevo(Idioma::Ingles);
    assert_ne!(
        pintar::hace_texto(&en, crate::lecciones::ui::Hace::Ayer),
        "ayer"
    );
}

fn muestra(nombre: &str, e: &mut Estado, ancho: u32, alto: u32) {
    let marco = Rect {
        x: 0,
        y: 0,
        ancho,
        alto,
    };
    crate::ventanita::muestra(nombre, ancho, alto, |p, _| pintar::todo(e, p, marco, 1.0));
}

#[test]
#[ignore = "necesita GPU; ejecutar con --ignored y mirar el PNG"]
fn muestra_de_la_ventana_v2() {
    let mut e = estado();
    // En reposo, como se ve al abrirla (lo que el usuario vio saturado).
    e.foco = None;
    muestra("lecciones-v2-reposo", &mut e, 1280, 820);
    e.rapida.campo.poner("La escalera del sótano en Miraflores resbala con el polvo de yeso; barrer antes de bajar material");
    e.rapida.fotos.push(("x.png".into(), Vec::new()));
    e.recalcular_rapida();
    e.foco = Some(Foco::Rapida);
    e.botones.raton = (700.0, 400.0);
    muestra("lecciones-v2", &mut e, 1280, 820);
    // Editando «Por que», con «Mas campos…» desplegado y su menu de area
    // abierto.
    e.foco = None;
    e.mas_campos = true;
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
        r.respuesta
            .poner("Sellar la grieta primero y dejar secar antes de pintar.");
    }
    e.foco = Some(Foco::Respuesta);
    muestra("lecciones-v2-repaso-pensar", &mut e, 1280, 820);
    hacer(&mut e, Accion::Mostrar);
    hacer(&mut e, Accion::Nota(0));
    muestra("lecciones-v2-repaso", &mut e, 900, 700);
    hacer(&mut e, Accion::Saltar);
    muestra("lecciones-v2-repaso-hecho", &mut e, 900, 700);
}
