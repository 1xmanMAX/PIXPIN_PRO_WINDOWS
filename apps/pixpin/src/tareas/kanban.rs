//! **La vista Kanban de Tareas**: la logica pura.
//!
//! El usuario (17-oct-2026): «añadir la vista estilo Kanban». Cada lista es
//! una **columna** (como en Trello): su encabezado arriba, sus tarjetas
//! pendientes debajo, una tras otra, y al pie su pliegue de hechas. Arrastrar
//! una tarjeta a otra columna la **pasa a esa lista** (`tareas::mover`, el
//! mismo «Mover a…» del Inbox, que es lo que hace el movil al repartir).
//!
//! Columnas por lista y no por estado: una tarea solo tiene dos estados
//! (`- [ ]` y `- [x]`, el formato que comparte con Android) y un tercero
//! («en curso») romperia el documento en el movil. Pendiente y hecha ya se
//! ven dentro de cada columna (arriba lo pendiente, al pie lo hecho).
//!
//! Las tarjetas son **las mismas** que las de la vista en cuadrados (se
//! pintan con `pintar_cuadrado` y se miden con `alto_de_cuadrado`), y los
//! grupos los mismos de las otras dos vistas ([`super::tarjetas::agrupar`]):
//! aqui solo se decide donde va cada una. Lo que no cabe a lo ancho baja a
//! una segunda banda de columnas, asi nunca hace falta desplazarse de lado.

#![forbid(unsafe_code)]

use pixpin_render::RectF;

use super::rejilla::{Celda, Direccion};
use super::tarjetas::{Grupo, Pieza};

/// Las medidas del tablero, ya con la escala.
#[derive(Debug, Clone, Copy)]
pub struct Medidas {
    /// El ancho del contenido.
    pub ancho: f32,
    /// Lo menos que mide una columna: con menos sitio, la siguiente baja a
    /// otra banda.
    pub columna_min: f32,
    /// Lo mas que mide: en una ventana muy ancha con pocas listas no se
    /// estiran hasta ser renglones.
    pub columna_max: f32,
    /// Entre columna y columna, y entre tarjeta y tarjeta.
    pub hueco: f32,
    /// El relleno de dentro de cada columna (su fondo se ve alrededor).
    pub relleno: f32,
    pub encabezado: f32,
    pub pliegue: f32,
    /// Entre una banda de columnas y la siguiente.
    pub entre_bandas: f32,
    /// Lo menos que mide una hecha (una fila, como en los cuadrados).
    pub fila_hecha: f32,
}

/// Una columna: de que lista es y su caja entera (del encabezado al pie de
/// su banda), que es donde se suelta una tarjeta.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Columna {
    pub lista: usize,
    pub caja: RectF,
}

/// Cuantas columnas van en cada banda y el ancho de cada una.
pub fn reparto(m: &Medidas, n: usize) -> (usize, f32) {
    let caben = (((m.ancho + m.hueco) / (m.columna_min + m.hueco)).floor() as usize).max(1);
    let k = caben.min(n.max(1));
    let ancho = ((m.ancho - (k - 1) as f32 * m.hueco) / k as f32)
        .min(m.columna_max)
        .max(1.0);
    (k, ancho)
}

/// Coloca los grupos como columnas. `alto_de(lista, fila, ancho)` mide una
/// tarjeta al ancho que le toca. Devuelve las piezas (con las mismas
/// `Celda` que la rejilla: se pinta, se acierta y se mueve el foco igual),
/// las columnas y el alto total.
pub fn disponer(
    grupos: &[Grupo],
    abierta: &dyn Fn(usize) -> bool,
    alto_de: &mut dyn FnMut(usize, usize, f32) -> f32,
    m: &Medidas,
) -> (Vec<Celda>, Vec<Columna>, f32) {
    let (k, ancho_col) = reparto(m, grupos.len());
    let dentro = (ancho_col - 2.0 * m.relleno).max(1.0);
    let mut celdas = Vec::new();
    let mut columnas = Vec::new();
    let mut y_banda = 0.0f32;
    for (b, banda) in grupos.chunks(k).enumerate() {
        if b > 0 {
            y_banda += m.entre_bandas;
        }
        let primera = columnas.len();
        let mut alto_banda = 0.0f32;
        for (c, g) in banda.iter().enumerate() {
            let x0 = c as f32 * (ancho_col + m.hueco);
            let x = x0 + m.relleno;
            let mut y = y_banda;
            celdas.push(Celda {
                pieza: Pieza::Encabezado(g.lista),
                caja: RectF {
                    x,
                    y,
                    ancho: dentro,
                    alto: m.encabezado,
                },
            });
            y += m.encabezado;
            for &fi in &g.arriba {
                // Como en los cuadrados: del alto de su texto, y como poco
                // media tarjeta.
                let alto = alto_de(g.lista, fi, dentro).max(dentro * 0.5);
                celdas.push(Celda {
                    pieza: Pieza::Tarjeta(g.lista, fi),
                    caja: RectF {
                        x,
                        y,
                        ancho: dentro,
                        alto,
                    },
                });
                y += alto + m.hueco;
            }
            if !g.hechas.is_empty() {
                celdas.push(Celda {
                    pieza: Pieza::Pliegue(g.lista),
                    caja: RectF {
                        x,
                        y,
                        ancho: dentro,
                        alto: m.pliegue,
                    },
                });
                y += m.pliegue;
                if abierta(g.lista) {
                    for &fi in &g.hechas {
                        // Una fila de un renglon, como en los cuadrados: medida
                        // como cuadrado, a lo estrecho de una columna, pedia
                        // el doble para algo ya tachado.
                        let alto = m.fila_hecha;
                        celdas.push(Celda {
                            pieza: Pieza::Tarjeta(g.lista, fi),
                            caja: RectF {
                                x,
                                y,
                                ancho: dentro,
                                alto,
                            },
                        });
                        y += alto + m.hueco * 0.5;
                    }
                }
            }
            alto_banda = alto_banda.max(y - y_banda + m.relleno);
            columnas.push(Columna {
                lista: g.lista,
                caja: RectF {
                    x: x0,
                    y: y_banda,
                    ancho: ancho_col,
                    alto: 0.0,
                },
            });
        }
        // Todas las columnas de una banda, del alto de la mas larga: se
        // puede soltar en cualquier punto de una columna corta.
        for col in &mut columnas[primera..] {
            col.caja.alto = alto_banda;
        }
        y_banda += alto_banda;
    }
    (celdas, columnas, y_banda)
}

/// La columna bajo el punto `(x, y)` del contenido, si hay alguna.
pub fn columna_en(columnas: &[Columna], x: f32, y: f32) -> Option<usize> {
    columnas
        .iter()
        .find(|c| {
            x >= c.caja.x
                && x <= c.caja.x + c.caja.ancho
                && y >= c.caja.y
                && y <= c.caja.y + c.caja.alto
        })
        .map(|c| c.lista)
}

/// Adonde va una tarjeta arrastrada y soltada en `(x, y)`: la lista de la
/// columna, si es otra que la suya. Soltada en su misma columna o fuera de
/// todas, no se mueve (`None`).
pub fn destino_al_soltar(columnas: &[Columna], desde: usize, x: f32, y: f32) -> Option<usize> {
    columna_en(columnas, x, y).filter(|&l| l != desde)
}

/// Lo que se mueve el raton antes de que pulsar una tarjeta sea arrastrarla
/// (en pixeles ya con la escala): menos es un clic que tiembla.
pub const UMBRAL_ARRASTRE: f32 = 6.0;

/// Si el raton se ha ido lo bastante lejos de donde se pulso para que sea
/// un arrastre.
pub fn es_arrastre(desde: (f32, f32), ahora: (f32, f32), escala: f32) -> bool {
    let (dx, dy) = (ahora.0 - desde.0, ahora.1 - desde.1);
    dx * dx + dy * dy >= (UMBRAL_ARRASTRE * escala).powi(2)
}

/// La tarjeta adonde va el foco con una flecha en el tablero: arriba y
/// abajo, la de encima o la de debajo en su columna; izquierda y derecha,
/// la de la columna de al lado mas cercana en alto. Sin foco, la primera.
/// `None` solo al subir desde lo alto de una columna: el foco sale a la caja
/// de apuntar (como en las otras vistas). En los bordes se queda.
pub fn vecina(
    celdas: &[Celda],
    actual: Option<(usize, usize)>,
    d: Direccion,
) -> Option<(usize, usize)> {
    let tarjetas: Vec<(usize, usize, RectF)> = celdas
        .iter()
        .filter_map(|c| match c.pieza {
            Pieza::Tarjeta(l, f) => Some((l, f, c.caja)),
            _ => None,
        })
        .collect();
    let primera = tarjetas.first().map(|&(l, f, _)| (l, f));
    let Some(i) = actual.and_then(|a| tarjetas.iter().position(|&(l, f, _)| (l, f) == a)) else {
        return primera;
    };
    let (l0, f0, r0) = tarjetas[i];
    // Misma columna: misma `x` y misma banda (la lista lo dice).
    let suya: Vec<&(usize, usize, RectF)> = tarjetas.iter().filter(|t| t.0 == l0).collect();
    let k = suya.iter().position(|t| t.1 == f0).unwrap_or(0);
    match d {
        Direccion::Arriba => (k > 0).then(|| (suya[k - 1].0, suya[k - 1].1)),
        Direccion::Abajo => {
            let t = suya[(k + 1).min(suya.len() - 1)];
            Some((t.0, t.1))
        }
        Direccion::Izquierda | Direccion::Derecha => {
            let derecha = d == Direccion::Derecha;
            // La columna de al lado en la misma banda: la `x` mas cercana
            // por ese lado entre las que se solapan en alto con la banda.
            let al_lado = tarjetas
                .iter()
                .filter(|t| {
                    if derecha {
                        t.2.x > r0.x + 0.5
                    } else {
                        t.2.x < r0.x - 0.5
                    }
                })
                .map(|t| t.2.x)
                .fold(None::<f32>, |m, x| {
                    Some(match m {
                        None => x,
                        Some(m) if derecha => m.min(x),
                        Some(m) => m.max(x),
                    })
                });
            let Some(cx) = al_lado else {
                return Some((l0, f0));
            };
            let centro = r0.y + r0.alto / 2.0;
            tarjetas
                .iter()
                .filter(|t| (t.2.x - cx).abs() <= 0.5)
                .min_by(|a, b| {
                    let da = (a.2.y + a.2.alto / 2.0 - centro).abs();
                    let db = (b.2.y + b.2.alto / 2.0 - centro).abs();
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|t| (t.0, t.1))
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn medidas(ancho: f32) -> Medidas {
        Medidas {
            ancho,
            columna_min: 220.0,
            columna_max: 360.0,
            hueco: 12.0,
            relleno: 8.0,
            encabezado: 40.0,
            pliegue: 40.0,
            entre_bandas: 18.0,
            fila_hecha: 44.0,
        }
    }

    fn grupo(lista: usize, arriba: &[usize], hechas: &[usize]) -> Grupo {
        Grupo {
            lista,
            arriba: arriba.to_vec(),
            hechas: hechas.to_vec(),
        }
    }

    fn caja_de(celdas: &[Celda], p: Pieza) -> RectF {
        celdas.iter().find(|c| c.pieza == p).unwrap().caja
    }

    #[test]
    fn cada_lista_es_una_columna_con_su_encabezado_sus_tarjetas_y_sus_hechas() {
        let grupos = [grupo(0, &[0, 1], &[2]), grupo(3, &[0], &[])];
        let (celdas, columnas, alto) =
            disponer(&grupos, &|_| true, &mut |_, _, _| 100.0, &medidas(700.0));
        // Dos columnas, una al lado de otra, en el orden de las listas.
        assert_eq!(
            columnas.iter().map(|c| c.lista).collect::<Vec<_>>(),
            vec![0, 3]
        );
        assert!(columnas[1].caja.x > columnas[0].caja.x + columnas[0].caja.ancho);
        assert_eq!(columnas[0].caja.y, columnas[1].caja.y);
        // En la primera: encabezado, las dos pendientes, el pliegue y la hecha,
        // de arriba abajo y todo dentro de su columna.
        let orden = [
            Pieza::Encabezado(0),
            Pieza::Tarjeta(0, 0),
            Pieza::Tarjeta(0, 1),
            Pieza::Pliegue(0),
            Pieza::Tarjeta(0, 2),
        ];
        let ys: Vec<f32> = orden.iter().map(|&p| caja_de(&celdas, p).y).collect();
        assert!(ys.windows(2).all(|w| w[0] < w[1]), "{ys:?}");
        for p in orden {
            let r = caja_de(&celdas, p);
            assert!(
                r.x >= columnas[0].caja.x
                    && r.x + r.ancho <= columnas[0].caja.x + columnas[0].caja.ancho
            );
        }
        // Las dos columnas, del alto de la mas larga: se suelta en cualquier sitio.
        assert_eq!(columnas[0].caja.alto, columnas[1].caja.alto);
        assert!(alto >= columnas[0].caja.alto);
    }

    #[test]
    fn plegadas_las_hechas_no_salen_y_lo_que_no_cabe_baja_a_otra_banda() {
        let grupos = [
            grupo(0, &[0], &[1]),
            grupo(1, &[0], &[]),
            grupo(2, &[0], &[]),
        ];
        // 480 de ancho: caben dos columnas de 220; la tercera baja.
        let (celdas, columnas, _) =
            disponer(&grupos, &|_| false, &mut |_, _, _| 100.0, &medidas(480.0));
        assert!(!celdas.iter().any(|c| c.pieza == Pieza::Tarjeta(0, 1)));
        assert!(celdas.iter().any(|c| c.pieza == Pieza::Pliegue(0)));
        assert_eq!(columnas[2].caja.x, columnas[0].caja.x);
        assert!(columnas[2].caja.y >= columnas[0].caja.y + columnas[0].caja.alto);
        // Caso negativo: una ventana estrechisima sigue con una columna, sin
        // anchos negativos.
        let (k, a) = reparto(&medidas(50.0), 3);
        assert_eq!(k, 1);
        assert!(a > 0.0);
        // Y una muy ancha con una lista no la estira mas de la cuenta.
        assert_eq!(reparto(&medidas(2000.0), 1), (1, 360.0));
    }

    #[test]
    fn soltar_en_otra_columna_la_mueve_y_en_la_suya_o_fuera_no() {
        let grupos = [grupo(0, &[0], &[]), grupo(5, &[0], &[])];
        let (_, columnas, _) = disponer(&grupos, &|_| true, &mut |_, _, _| 100.0, &medidas(700.0));
        let otra = columnas[1].caja;
        let (cx, cy) = (otra.x + otra.ancho / 2.0, otra.y + otra.alto - 2.0);
        assert_eq!(destino_al_soltar(&columnas, 0, cx, cy), Some(5));
        // En la suya, nada.
        let suya = columnas[0].caja;
        assert_eq!(
            destino_al_soltar(&columnas, 0, suya.x + 5.0, suya.y + 5.0),
            None
        );
        // Fuera de todas (debajo del tablero), nada.
        assert_eq!(
            destino_al_soltar(&columnas, 0, cx, otra.y + otra.alto + 50.0),
            None
        );
        // Un clic que tiembla no es un arrastre; seis pixeles (a escala 1) si.
        assert!(!es_arrastre((10.0, 10.0), (13.0, 12.0), 1.0));
        assert!(es_arrastre((10.0, 10.0), (16.0, 10.0), 1.0));
        assert!(!es_arrastre((10.0, 10.0), (16.0, 10.0), 2.0));
    }

    #[test]
    fn la_vista_kanban_elegida_se_recuerda_al_volver_a_abrir() {
        use super::super::rejilla::{Recordada, Vista};
        let raiz =
            std::env::temp_dir().join(format!("pixpin-tareas-kanban-{}", std::process::id()));
        std::fs::create_dir_all(&raiz).unwrap();
        Vista::Kanban.guardar(&raiz);
        assert_eq!(
            std::fs::read_to_string(raiz.join("tareas-vista.txt")).unwrap(),
            "kanban"
        );
        assert_eq!(Vista::leer(&raiz), Vista::Kanban);
        // Volver a otra vista la sustituye.
        Vista::Tarjetas.guardar(&raiz);
        assert_eq!(Vista::leer(&raiz), Vista::Tarjetas);
        // Caso negativo: una palabra parecida no es el tablero.
        assert_eq!(Vista::de_texto("kanbann"), Vista::Tarjetas);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn las_flechas_bajan_por_la_columna_y_saltan_a_la_de_al_lado() {
        let grupos = [grupo(0, &[0, 1, 2], &[]), grupo(1, &[0, 1], &[])];
        let (celdas, _, _) = disponer(&grupos, &|_| true, &mut |_, _, _| 100.0, &medidas(700.0));
        assert_eq!(vecina(&celdas, None, Direccion::Abajo), Some((0, 0)));
        assert_eq!(
            vecina(&celdas, Some((0, 0)), Direccion::Abajo),
            Some((0, 1))
        );
        assert_eq!(
            vecina(&celdas, Some((0, 2)), Direccion::Abajo),
            Some((0, 2))
        );
        assert_eq!(
            vecina(&celdas, Some((0, 1)), Direccion::Derecha),
            Some((1, 1))
        );
        assert_eq!(
            vecina(&celdas, Some((1, 0)), Direccion::Izquierda),
            Some((0, 0))
        );
        assert_eq!(
            vecina(&celdas, Some((1, 0)), Direccion::Derecha),
            Some((1, 0))
        );
        // Subir desde lo alto de una columna devuelve el foco a la caja.
        assert_eq!(vecina(&celdas, Some((1, 0)), Direccion::Arriba), None);
    }
}
