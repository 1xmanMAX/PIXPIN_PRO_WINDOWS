//! **Marcar celdas leyendo una hoja de calculo** (2-nov-2026): la barrita
//! del PDF (copiar, los cinco colores, resaltar, raya ondulada, subrayar,
//! tachar, tapar y quitar) sale encima de un rango elegido con el raton.
//!
//! El usuario: «la herramienta de marcar el texto la quiero EN TODOS los
//! sitios donde se ve texto». En una hoja **se marca la celda entera**, no
//! un trozo de su texto: en una celda casi siempre hay una cifra o unas
//! pocas palabras, el texto de una celda se recorta o se desborda segun el
//! ancho de la columna (en otro aparato, otro ancho) y lo que se quiere
//! senalar es «esta fila», «estas cifras». Resaltar y tapar cubren el rango
//! entero; las rayas van fila a fila, de lado a lado del rango.
//!
//! Sale **solo leyendo** (no editando ni con el lapiz del riel) y solo con
//! dos celdas o mas elegidas: un clic en una celda es moverse por la hoja y
//! una barrita a cada clic estorbaria. Para una sola celda, se arrastra un
//! poco hasta la de al lado y se vuelve (o se elige con Mayus).
//!
//! **Donde van**: son dibujo normal de la capa de la hoja (la del lapiz y
//! del asistente, `capa`: `anot-<uid>-h<i>` de un libro del chat, o
//! `<libro>.pixpin-anotado/hoja-<n>.excalidraw` junto al fichero), en sus
//! unidades (`rejilla_de_la_capa`, las mismas en todos los aparatos) y con
//! el grupo `marca-texto-<ms>` (`pindesk_motor2d::marcas_texto`). Viajan y
//! se ven en el movil como cualquier trazo. Ctrl+Z / Ctrl+Y (viendo) las
//! quitan y las devuelven, la ultima primero.

use pindesk_motor2d::marcas_texto::{self as mt, Marca};
use pindesk_render::{Pintor, RectF};
use pindesk_store::Catalogo;
use pindesk_ui::barra_marcar::BotonMarcar;

use super::{Estado, hoja_office, vista_celdas};

/// Mas filas que esto no se rayan una a una (una columna entera elegida):
/// se marca hasta ahi.
const MAX_FILAS: u32 = 2000;

/// Lo de marcar del visor.
#[derive(Debug)]
pub(super) struct Marcar {
    /// El color elegido (`marcas_texto::COLORES`).
    color: u8,
    /// La barrita: su caja y sus botones, como se pinto la ultima vez.
    barrita: Option<(RectF, Vec<(RectF, BotonMarcar)>)>,
    /// Lo elegido se eligio con el raton (la barrita puede salir).
    con_raton: bool,
    /// Lo marcado, para Ctrl+Z: la hoja y los pasos de su capa despues.
    hechos: Vec<(usize, usize)>,
    deshechos: Vec<(usize, usize)>,
}

impl Default for Marcar {
    fn default() -> Self {
        Marcar {
            color: mt::COLOR_INICIAL,
            barrita: None,
            con_raton: false,
            hechos: Vec::new(),
            deshechos: Vec::new(),
        }
    }
}

fn dentro(r: &RectF, (x, y): (f32, f32)) -> bool {
    x >= r.x && x < r.x + r.ancho && y >= r.y && y < r.y + r.alto
}

/// Si la barrita puede salir ahora.
fn a_la_vista(est: &Estado) -> bool {
    let r = est.abierta().sel.rango();
    est.marcar.con_raton
        && !est.editando
        && !est.anotando
        && !est.poniendo_marca
        && !est.sin_interfaz
        && est.gesto.is_none()
        && (r.fila0 != r.fila1 || r.col0 != r.col1)
}

/// Lo elegido acabo de elegirse con el raton (al soltar).
pub(super) fn al_soltar(est: &mut Estado) {
    est.marcar.con_raton = true;
}

/// La caja de lo elegido que se ve, en pixeles de la ventana.
fn zona(est: &Estado) -> Option<[f32; 4]> {
    let ab = est.abierta();
    let r = ab.sel.rango();
    let (v, celdas) = est.vista();
    let vc = vista_celdas(&v, celdas);
    let mut zona: Option<[f32; 4]> = None;
    // Las cuatro esquinas (las que se ven) y, si ninguna, lo que se ve.
    for (f, c) in [
        (r.fila0, r.col0),
        (r.fila0, r.col1),
        (r.fila1, r.col0),
        (r.fila1, r.col1),
    ] {
        if let Some(k) = hoja_office::caja_de_celda(&ab.rejilla, &vc, celdas, f, c) {
            zona = Some(match zona {
                None => k,
                Some(z) => [
                    z[0].min(k[0]),
                    z[1].min(k[1]),
                    z[2].max(k[2]),
                    z[3].max(k[3]),
                ],
            });
        }
    }
    zona.map(|[x0, y0, x1, y1]| {
        [
            x0.max(celdas.x),
            y0.max(celdas.y),
            x1.min(celdas.x + celdas.ancho),
            y1.min(celdas.y + celdas.alto),
        ]
    })
}

/// **Pinta la barrita** encima de lo elegido (lo ultimo, por encima de todo).
pub(super) fn pintar(p: &Pintor, est: &mut Estado, textos: &Catalogo) {
    est.marcar.barrita = None;
    if !a_la_vista(est) {
        return;
    }
    let Some(z) = zona(est) else { return };
    let (caja, botones) = crate::lector::pintar_barrita_de_marcar(
        p,
        z,
        (est.ancho, est.alto),
        est.e,
        est.marcar.color,
        est.raton,
        textos,
    );
    est.marcar.barrita = Some((caja, botones));
}

/// **Un clic**: en un boton de la barrita lo hace y se lo queda (`true`);
/// en otro sitio la barrita se va hasta que se vuelva a elegir con el
/// raton.
pub(super) fn pulsar(est: &mut Estado, textos: &Catalogo) -> bool {
    let raton = est.raton;
    let Some((caja, botones)) = est.marcar.barrita.clone() else {
        est.marcar.con_raton = false;
        return false;
    };
    if !dentro(&caja, raton) {
        est.marcar.con_raton = false;
        return false;
    }
    match botones
        .iter()
        .find(|(r, _)| dentro(r, raton))
        .map(|(_, b)| *b)
    {
        Some(BotonMarcar::Copiar) => est.copiar(textos),
        Some(BotonMarcar::Color(k)) => est.marcar.color = k,
        Some(BotonMarcar::Marcar(m)) => {
            aplicar(est, Some(m));
        }
        Some(BotonMarcar::Quitar) => {
            aplicar(est, None);
            est.marcar.con_raton = false;
        }
        // Entre dos botones: no es de la hoja.
        None => {}
    }
    true
}

/// Las cajas de lo elegido en las unidades de la capa: el rango entero
/// (resaltar, tapar, quitar) o una por fila (las rayas).
fn cajas(est: &Estado, fila_a_fila: bool) -> Vec<[f32; 4]> {
    let ab = est.abierta();
    let r = ab.sel.rango();
    let rc = crate::asistente::anotar_inteligente::rejilla_de_la_capa(&*est.libro, &ab.hoja);
    let fila1 = r.fila1.min(r.fila0 + MAX_FILAS);
    let (x0, x1) = (rc.x(r.col0), rc.x(r.col1) + rc.ancho(r.col1));
    let caja = |f0: u32, f1: u32| {
        [
            x0 as f32,
            rc.y(f0) as f32,
            x1 as f32,
            (rc.y(f1) + rc.alto(f1)) as f32,
        ]
    };
    if fila_a_fila {
        (r.fila0..=fila1)
            .filter(|&f| rc.alto(f) > 0.0)
            .map(|f| caja(f, f))
            .collect()
    } else {
        vec![caja(r.fila0, fila1)]
    }
}

/// Marca lo elegido con `que` (o quita lo que hay, con `None`).
fn aplicar(est: &mut Estado, que: Option<Marca>) -> bool {
    let fila_a_fila = !matches!(que, None | Some(Marca::Resaltar | Marca::Tapar));
    let cajas = cajas(est, fila_a_fila);
    if cajas.is_empty() {
        return false;
    }
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    let grupo = mt::grupo_nuevo(ms);
    let rgb = mt::rgb(est.marcar.color);
    let i = est.activa;
    let Some(ab) = est.hojas.get(i).and_then(Option::as_ref) else {
        return false;
    };
    let hecho = est.anotado.marcar(
        (&*est.libro, &ab.hoja, i),
        &ab.rejilla,
        &cajas,
        que.map(|m| (m, rgb)),
        &grupo,
    );
    tracing::info!(marca = ?que, cajas = cajas.len(), hecho = hecho.is_some(), "hoja: celdas marcadas");
    let Some(pasos) = hecho else { return false };
    est.marcar.hechos.push((i, pasos));
    est.marcar.deshechos.clear();
    true
}

/// **Ctrl+Z** (viendo): la ultima marca de esta hoja, si sigue arriba.
pub(super) fn deshacer(est: &mut Estado) -> bool {
    let i = est.activa;
    let Some(&(h, pasos)) = est.marcar.hechos.last() else {
        return false;
    };
    if h != i || !est.anotado.deshacer_marca(pasos, false) {
        return false;
    }
    est.marcar.hechos.pop();
    est.marcar.deshechos.push((h, pasos));
    true
}

/// **Ctrl+Y**: la devuelve.
pub(super) fn rehacer(est: &mut Estado) -> bool {
    let i = est.activa;
    let Some(&(h, pasos)) = est.marcar.deshechos.last() else {
        return false;
    };
    if h != i || !est.anotado.deshacer_marca(pasos, true) {
        return false;
    }
    est.marcar.deshechos.pop();
    est.marcar.hechos.push((h, pasos));
    true
}

#[cfg(test)]
mod pruebas {
    use super::super::logica::Seleccion;
    use super::super::pruebas::{abierta_de_prueba, libro_de_tabla};
    use super::*;

    /// Rayas fila a fila, resaltar de una pieza, quitar lo que cae en lo
    /// elegido, y Ctrl+Z / Ctrl+Y de la ultima primero.
    #[test]
    fn marcar_celdas_y_deshacer() {
        let mut libro = libro_de_tabla(50, 10);
        let mut est = abierta_de_prueba(&mut libro, 900, 700);
        est.abierta_mut().sel = Seleccion {
            ancla: (2, 1),
            activa: (4, 3),
        };
        // Caso negativo: sin elegir con el raton no sale.
        assert!(!a_la_vista(&est));
        al_soltar(&mut est);
        assert!(a_la_vista(&est));
        let z = zona(&est).expect("lo elegido se ve");
        assert!(z[2] > z[0] && z[3] > z[1]);
        assert!(aplicar(&mut est, Some(Marca::Tachar)));
        assert_eq!(est.anotado.elementos_de_marcas(), 3, "una raya por fila");
        assert!(aplicar(&mut est, Some(Marca::Resaltar)));
        assert_eq!(
            est.anotado.elementos_de_marcas(),
            4,
            "el fondo, de una pieza"
        );
        assert!(deshacer(&mut est));
        assert_eq!(est.anotado.elementos_de_marcas(), 3);
        assert!(rehacer(&mut est));
        assert_eq!(est.anotado.elementos_de_marcas(), 4);
        // Quitar sobre otra zona no toca nada; sobre la suya, todo.
        est.abierta_mut().sel = Seleccion {
            ancla: (20, 5),
            activa: (22, 6),
        };
        assert!(!aplicar(&mut est, None));
        est.abierta_mut().sel = Seleccion {
            ancla: (3, 2),
            activa: (3, 3),
        };
        assert!(aplicar(&mut est, None));
        assert_eq!(est.anotado.elementos_de_marcas(), 0);
        // Caso negativo: una celda sola no saca la barrita.
        est.abierta_mut().sel = Seleccion::en(5, 5);
        assert!(!a_la_vista(&est));
    }
}
