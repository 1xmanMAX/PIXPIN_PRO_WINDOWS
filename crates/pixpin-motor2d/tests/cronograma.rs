//! **El cronograma contra la API publica del gesto** (F12): se crea
//! arrastrando la caja con su herramienta y sus barras se arrastran con la
//! mano, como en el movil (`Tool.CRONOGRAMA`, `Gesture.BarraDelPlan`).

use pixpin_motor2d::cronograma;
use pixpin_motor2d::elemento::Figura;
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::vector::Punto2;

fn arrastrar(g: &mut Gesto, e: &mut Escena, a: (f32, f32), b: (f32, f32)) {
    g.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(a.0, a.1),
            shift: false,
            alt: false,
            presion: None,
        },
        e,
        1.0,
    );
    for k in 1..=10 {
        let t = k as f32 / 10.0;
        g.evento(
            EventoGesto::Mover {
                p: Punto2::nuevo(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t),
                shift: false,
                alt: false,
                presion: None,
            },
            e,
            1.0,
        );
    }
    g.evento(
        EventoGesto::Soltar {
            p: Punto2::nuevo(b.0, b.1),
        },
        e,
        1.0,
    );
}

fn tareas(e: &Escena, id: u64) -> Vec<cronograma::Tarea> {
    match &e.buscar(id).unwrap().figura {
        Figura::Cronograma { tareas, .. } => tareas.clone(),
        otra => panic!("no es un cronograma: {otra:?}"),
    }
}

fn nuevo_plan(escena: &mut Escena) -> (Gesto, u64) {
    let mut g = Gesto::nuevo();
    g.enganche.activo = false;
    g.tomar_herramienta(Herramienta::Cronograma);
    arrastrar(&mut g, escena, (0.0, 0.0), (600.0, 300.0));
    let id = escena.visibles().last().unwrap().id;
    (g, id)
}

#[test]
fn arrastrar_con_el_cronograma_deja_un_plan_de_tres_filas_del_tamano_arrastrado() {
    let mut escena = Escena::nueva();
    let (_, id) = nuevo_plan(&mut escena);
    let e = escena.buscar(id).unwrap();
    assert_eq!((e.ancho, e.alto), (600.0, 300.0));
    let t = tareas(&escena, id);
    assert_eq!(t.len(), 3);
    assert_eq!((t[0].desde, t[1].desde, t[2].desde), (0.0, 1.0, 2.0));
}

#[test]
fn con_la_mano_la_barra_se_arrastra_a_cuartos_de_columna_y_se_deshace_de_una_vez() {
    let mut escena = Escena::nueva();
    let (mut g, id) = nuevo_plan(&mut escena);
    g.tomar_herramienta(Herramienta::Mano);
    g.seleccion.poner(id);
    let plan = escena.buscar(id).unwrap().clone();
    let (x0, y0, x1, y1) = cronograma::barra_de_tarea(&plan, 0).unwrap();
    let col = cronograma::ancho_de_columna(&plan);
    // Por el cuerpo, dos columnas y un pelo a la derecha: se mueve 2.
    let desde = ((x0 + x1) / 2.0 - 10.0, (y0 + y1) / 2.0);
    arrastrar(&mut g, &mut escena, desde, (desde.0 + 2.1 * col, desde.1));
    assert_eq!(tareas(&escena, id)[0].desde, 2.0);
    // La lamina no se movio: se movio la barra.
    assert_eq!(escena.buscar(id).unwrap().x, plan.x);
    escena.deshacer();
    assert_eq!(tareas(&escena, id)[0].desde, 0.0);
}

#[test]
fn fuera_de_las_barras_la_mano_mueve_la_lamina_entera() {
    let mut escena = Escena::nueva();
    let (mut g, id) = nuevo_plan(&mut escena);
    g.tomar_herramienta(Herramienta::Mano);
    g.seleccion.poner(id);
    let antes = tareas(&escena, id);
    // En la columna de los nombres no hay barra.
    arrastrar(&mut g, &mut escena, (20.0, 150.0), (120.0, 150.0));
    assert_eq!(escena.buscar(id).unwrap().x, 100.0);
    assert_eq!(tareas(&escena, id), antes, "las filas van con la lamina");
}
