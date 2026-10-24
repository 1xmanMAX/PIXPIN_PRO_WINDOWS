//! **La cola del hilo de las hojas**: que se dibuja primero, que se cancela
//! y que no se repite.
//!
//! `Windows.Data.Pdf` no tolera bien varios hilos sobre un documento, asi
//! que hay uno solo y lo que importa es **el orden y no hacer trabajo en
//! vano**:
//!
//! - **Prioridad**: primero la hoja que se ve (la mas cercana al centro de
//!   la ventana delante) y su trozo nitido si se esta muy cerca, luego las
//!   vecinas y al final el texto de la hoja bajo el raton (para elegir y
//!   para los enlaces). Miniaturas de fondo no hay: medido, dibujar una hoja
//!   a 192 pixeles cuesta casi lo mismo que a 1024 (90 frente a 150 ms en
//!   la tesis de muestra), asi que adelantarlas seria gastar CPU en hojas
//!   que quiza no se miren.
//! - **Cancelacion**: cada lista nueva sustituye a la anterior; lo que ya no
//!   esta en ella no se dibuja. Lo que esta en curso no se puede parar
//!   (WinRT no deja), pero se cuenta como «en vano» si ya no se queria.
//! - **Fusion**: lo repetido en una lista se pide una vez; y lo ya entregado
//!   que la ventana aun no ha visto (va por el canal) no se vuelve a dibujar
//!   aunque la ventana lo pida otra vez: la ventana dice hasta que entrega ha
//!   visto (`visto`) y la cola olvida las anteriores.
//!
//! Es logica pura, sin WinRT: se prueba sola.
//!
//! Partes traducidas de pdfcraft (https://github.com/storytold/pdfcraft), MIT OR Apache-2.0, Copyright (c) 2026 ArtCraft Team and the Pdfcraft contributors.
//! (la idea de `WorkQueue::replace`: lo deseado con su prioridad, lo en
//! curso y lo ya hecho fuera de la lista nueva).

use std::collections::{HashSet, VecDeque};

/// Lo que se le puede pedir al hilo de las hojas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Trabajo {
    /// La hoja entera a este ancho en pixeles.
    Hoja { hoja: usize, ancho: u32 },
    /// Un trozo de la hoja dibujada a `ancho` pixeles (muy de cerca): las
    /// casillas de [`LADO_TROZO`] pixeles de la `x0`/`y0` a la `x1`/`y1`
    /// (sin incluir). Ver [`trozo_en_fracciones`].
    Trozo {
        hoja: usize,
        ancho: u32,
        x0: u16,
        y0: u16,
        x1: u16,
        y1: u16,
    },
    /// El texto de la hoja con donde cae cada letra, y sus enlaces.
    Texto { hoja: usize },
}

impl Trabajo {
    pub(super) fn hoja(&self) -> usize {
        match *self {
            Trabajo::Hoja { hoja, .. } | Trabajo::Trozo { hoja, .. } | Trabajo::Texto { hoja } => {
                hoja
            }
        }
    }
}

/// Por que se pide: ordena la cola aunque la lista venga desordenada.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Prioridad {
    /// Lo que se ve.
    Visible,
    /// Lo que se vera al seguir leyendo.
    Vecina,
    /// El texto y los enlaces de la hoja bajo el raton.
    Texto,
}

/// La cola del hilo. Vive en el hilo de las hojas.
#[derive(Debug, Default)]
pub(super) struct Cola {
    pendientes: VecDeque<Trabajo>,
    /// Lo de la ultima lista: lo que se sigue queriendo.
    queridos: HashSet<Trabajo>,
    /// Lo entregado y aun no visto por la ventana, con su numero de entrega.
    entregados: Vec<(Trabajo, u64)>,
    serie: u64,
    /// Pendientes que una lista nueva quito antes de dibujarlos.
    pub(super) cancelados: usize,
    /// Dibujados que, al acabar, ya no se querian.
    pub(super) en_vano: usize,
}

impl Cola {
    /// **Una lista nueva** de lo que hace falta: sustituye a la anterior. Va
    /// por prioridad (estable: dentro de cada una, en el orden de la
    /// lista), sin repetidos y sin lo ya entregado que la ventana no ha
    /// visto todavia (entregas con numero mayor que `visto`).
    pub(super) fn pedir(&mut self, mut lista: Vec<(Prioridad, Trabajo)>, visto: u64) {
        self.entregados.retain(|(_, n)| *n > visto);
        lista.sort_by_key(|(p, _)| *p);
        let mut nuevos = HashSet::with_capacity(lista.len());
        let mut pendientes = VecDeque::with_capacity(lista.len());
        for (_, t) in lista {
            if !nuevos.insert(t) {
                continue;
            }
            if self.entregados.iter().any(|(e, _)| *e == t) {
                continue;
            }
            pendientes.push_back(t);
        }
        self.cancelados += self
            .pendientes
            .iter()
            .filter(|t| !nuevos.contains(t))
            .count();
        self.pendientes = pendientes;
        self.queridos = nuevos;
    }

    /// Lo siguiente que dibujar.
    pub(super) fn tomar(&mut self) -> Option<Trabajo> {
        self.pendientes.pop_front()
    }

    /// **Acabado `t`**: su numero de entrega (la ventana lo devuelve como
    /// `visto` cuando lo recoge) y si aun se queria. Lo que ya no se queria
    /// se cuenta en vano; se entrega igual, que la ventana decide.
    pub(super) fn entregar(&mut self, t: Trabajo) -> (u64, bool) {
        self.serie += 1;
        self.entregados.push((t, self.serie));
        let querido = self.queridos.contains(&t);
        if !querido {
            self.en_vano += 1;
        }
        (self.serie, querido)
    }

    pub(super) fn vacia(&self) -> bool {
        self.pendientes.is_empty()
    }
}

/// **Lo que cabe en el presupuesto de memoria**: de lo pintado (clave,
/// bytes, distancia a lo que se ve; 0 = se ve), se queda todo lo que se ve
/// aunque no quepa, y lo demas de lo mas cercano a lo mas lejano mientras
/// quepa en `tope` bytes. Lo que no sale, se suelta.
pub(super) fn lo_que_cabe<K: Copy + Eq + std::hash::Hash>(
    items: &[(K, usize, usize)],
    tope: usize,
) -> HashSet<K> {
    let mut orden: Vec<&(K, usize, usize)> = items.iter().collect();
    orden.sort_by_key(|(_, _, d)| *d);
    let mut usado = 0usize;
    let mut quedan = HashSet::new();
    for (k, bytes, distancia) in orden {
        if *distancia == 0 || usado.saturating_add(*bytes) <= tope {
            usado = usado.saturating_add(*bytes);
            quedan.insert(*k);
        }
    }
    quedan
}

/// El lado de las casillas en que se mide un trozo, en pixeles: un trozo
/// se pide por casillas enteras para que desplazarse un poco no pida otro.
pub(super) const LADO_TROZO: u32 = 256;

/// Lo que mide en pixeles la hoja dibujada a `ancho` con esta proporcion
/// (alto entre ancho).
pub(super) fn alto_en_pixeles(ancho: u32, proporcion: f32) -> u32 {
    ((ancho as f32 * proporcion).round() as u32).max(1)
}

/// **Un trozo en fracciones de la hoja** (x, y, ancho, alto, de 0 a 1) y lo
/// que mide en pixeles, para `renderizar_trozo` y para ponerlo en su sitio.
/// `None` si el trozo esta vacio o fuera de la hoja.
pub(super) fn trozo_en_fracciones(t: Trabajo, proporcion: f32) -> Option<([f32; 4], u32, u32)> {
    let Trabajo::Trozo {
        ancho,
        x0,
        y0,
        x1,
        y1,
        ..
    } = t
    else {
        return None;
    };
    let alto = alto_en_pixeles(ancho, proporcion);
    let px0 = (x0 as u32 * LADO_TROZO).min(ancho);
    let px1 = (x1 as u32 * LADO_TROZO).min(ancho);
    let py0 = (y0 as u32 * LADO_TROZO).min(alto);
    let py1 = (y1 as u32 * LADO_TROZO).min(alto);
    if px1 <= px0 || py1 <= py0 {
        return None;
    }
    let (w, h) = (px1 - px0, py1 - py0);
    Some((
        [
            px0 as f32 / ancho as f32,
            py0 as f32 / alto as f32,
            w as f32 / ancho as f32,
            h as f32 / alto as f32,
        ],
        w,
        h,
    ))
}

/// **El trozo que hace falta** para ver de la hoja lo que va de `vista`
/// (x0, y0, x1, y1 en fracciones de la hoja) dibujada a `ancho`: las
/// casillas que se ven y una de mas alrededor, para que desplazarse un poco
/// no pida otro. `None` si no se ve nada de la hoja.
pub(super) fn trozo_para(
    hoja: usize,
    ancho: u32,
    proporcion: f32,
    vista: [f32; 4],
) -> Option<Trabajo> {
    let alto = alto_en_pixeles(ancho, proporcion);
    let [a, b, c, d] = vista.map(|v| v.clamp(0.0, 1.0));
    if c <= a || d <= b {
        return None;
    }
    let lado = LADO_TROZO as f32;
    let casillas_x = ancho.div_ceil(LADO_TROZO);
    let casillas_y = alto.div_ceil(LADO_TROZO);
    let x0 = ((a * ancho as f32 / lado).floor() as u32).saturating_sub(1);
    let y0 = ((b * alto as f32 / lado).floor() as u32).saturating_sub(1);
    let x1 = ((c * ancho as f32 / lado).ceil() as u32 + 1).min(casillas_x);
    let y1 = ((d * alto as f32 / lado).ceil() as u32 + 1).min(casillas_y);
    (x1 > x0 && y1 > y0).then_some(Trabajo::Trozo {
        hoja,
        ancho,
        x0: x0.min(u16::MAX as u32) as u16,
        y0: y0.min(u16::MAX as u32) as u16,
        x1: x1.min(u16::MAX as u32) as u16,
        y1: y1.min(u16::MAX as u32) as u16,
    })
}

/// Si el trozo `t` cubre lo que se ve (`vista`, en fracciones de la hoja):
/// entonces no hace falta otro.
pub(super) fn cubre(t: Trabajo, proporcion: f32, vista: [f32; 4]) -> bool {
    let Some(([x, y, w, h], _, _)) = trozo_en_fracciones(t, proporcion) else {
        return false;
    };
    let [a, b, c, d] = vista.map(|v| v.clamp(0.0, 1.0));
    let tol = 1e-4;
    a >= x - tol && b >= y - tol && c <= x + w + tol && d <= y + h + tol
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_trozo_cubre_lo_que_se_ve_con_una_casilla_de_mas_y_no_se_sale_de_la_hoja() {
        // Una A4 (1,414) a 8192 de ancho: 11583 de alto.
        let p = 1.414;
        let vista = [0.40, 0.30, 0.55, 0.36];
        let t = trozo_para(7, 8192, p, vista).unwrap();
        assert!(cubre(t, p, vista));
        let ([x, y, w, h], pw, ph) = trozo_en_fracciones(t, p).unwrap();
        assert!(x < 0.40 && y < 0.30 && x + w > 0.55 && y + h > 0.36);
        // Cuesta lo que se ve y poco mas, no la hoja entera.
        assert!((pw as u64 * ph as u64) < 8192 * 11583 / 20, "{pw}x{ph}");
        // Desplazarse un poco no pide otro trozo.
        assert!(cubre(t, p, [0.405, 0.305, 0.555, 0.365]));
        // Desplazarse mucho, si.
        assert!(!cubre(t, p, [0.60, 0.30, 0.75, 0.36]));
        // En el borde no se sale de la hoja.
        let t = trozo_para(0, 8192, p, [0.9, 0.95, 1.0, 1.0]).unwrap();
        let ([x, y, w, h], _, _) = trozo_en_fracciones(t, p).unwrap();
        assert!(x + w <= 1.0 + 1e-6 && y + h <= 1.0 + 1e-6);
        // Casos negativos: nada que ver, nada que pedir; y una hoja entera no
        // es un trozo.
        assert_eq!(trozo_para(0, 8192, p, [0.5, 0.5, 0.5, 0.6]), None);
        assert_eq!(trozo_para(0, 8192, p, [1.2, 0.0, 1.5, 0.5]), None);
        assert_eq!(
            trozo_en_fracciones(Trabajo::Hoja { hoja: 0, ancho: 10 }, p),
            None
        );
    }

    fn hoja(i: usize) -> Trabajo {
        Trabajo::Hoja {
            hoja: i,
            ancho: 1024,
        }
    }

    #[test]
    fn lo_que_se_ve_va_antes_que_las_vecinas_y_el_texto() {
        let mut c = Cola::default();
        c.pedir(
            vec![
                (Prioridad::Texto, Trabajo::Texto { hoja: 9 }),
                (Prioridad::Vecina, hoja(4)),
                (Prioridad::Texto, Trabajo::Texto { hoja: 5 }),
                (Prioridad::Visible, hoja(5)),
                (Prioridad::Visible, hoja(6)),
            ],
            0,
        );
        let orden: Vec<Trabajo> = std::iter::from_fn(|| c.tomar()).collect();
        assert_eq!(
            orden,
            vec![
                hoja(5),
                hoja(6),
                hoja(4),
                Trabajo::Texto { hoja: 9 },
                Trabajo::Texto { hoja: 5 }
            ]
        );
        assert!(c.vacia());
    }

    #[test]
    fn una_lista_nueva_cancela_lo_que_ya_no_se_ve_y_funde_lo_repetido() {
        let mut c = Cola::default();
        c.pedir((0..5).map(|i| (Prioridad::Visible, hoja(i))).collect(), 0);
        assert_eq!(c.tomar(), Some(hoja(0)));
        // Se salto cien hojas: lo de antes no se dibuja.
        c.pedir(
            vec![
                (Prioridad::Visible, hoja(100)),
                (Prioridad::Visible, hoja(100)),
                (Prioridad::Vecina, hoja(101)),
            ],
            0,
        );
        assert_eq!(c.cancelados, 4, "las cuatro pendientes de antes");
        assert_eq!(c.tomar(), Some(hoja(100)));
        assert_eq!(c.tomar(), Some(hoja(101)));
        assert_eq!(c.tomar(), None, "la repetida va una vez");
        // La que estaba en curso al saltar llega en vano.
        let (_, querida) = c.entregar(hoja(0));
        assert!(!querida);
        assert_eq!(c.en_vano, 1);
        let (_, querida) = c.entregar(hoja(100));
        assert!(querida);
        assert_eq!(c.en_vano, 1);
    }

    #[test]
    fn lo_entregado_que_la_ventana_aun_no_vio_no_se_dibuja_dos_veces() {
        let mut c = Cola::default();
        c.pedir(vec![(Prioridad::Visible, hoja(3))], 0);
        let t = c.tomar().unwrap();
        let (n, _) = c.entregar(t);
        // La ventana aun no lo recogio y lo vuelve a pedir: no se repite.
        c.pedir(vec![(Prioridad::Visible, hoja(3))], n - 1);
        assert_eq!(c.tomar(), None);
        // Ya lo vio (y si lo vuelve a pedir es que lo solto): se dibuja.
        c.pedir(vec![(Prioridad::Visible, hoja(3))], n);
        assert_eq!(c.tomar(), Some(hoja(3)));
        // Caso negativo: otro ancho de la misma hoja es otro trabajo.
        let mut c = Cola::default();
        c.pedir(vec![(Prioridad::Visible, hoja(3))], 0);
        let t = c.tomar().unwrap();
        let (n, _) = c.entregar(t);
        c.pedir(
            vec![(
                Prioridad::Visible,
                Trabajo::Hoja {
                    hoja: 3,
                    ancho: 2048,
                },
            )],
            n - 1,
        );
        assert_eq!(
            c.tomar(),
            Some(Trabajo::Hoja {
                hoja: 3,
                ancho: 2048
            })
        );
    }

    #[test]
    fn una_lista_vacia_lo_cancela_todo_y_no_se_queda_con_nada() {
        let mut c = Cola::default();
        c.pedir(
            vec![
                (Prioridad::Visible, hoja(1)),
                (Prioridad::Texto, Trabajo::Texto { hoja: 2 }),
            ],
            0,
        );
        c.pedir(Vec::new(), 0);
        assert_eq!(c.cancelados, 2);
        assert!(c.vacia());
        assert_eq!(c.tomar(), None);
    }

    #[test]
    fn el_presupuesto_guarda_lo_visible_y_lo_mas_cercano_que_quepa() {
        // (hoja, bytes, distancia)
        let items = [
            (10, 50, 0),
            (11, 50, 0),
            (12, 30, 1),
            (9, 30, 1),
            (13, 30, 2),
            (40, 1, 30),
        ];
        let q = lo_que_cabe(&items, 161);
        assert!(q.contains(&10) && q.contains(&11), "lo visible siempre");
        assert!(q.contains(&12) && q.contains(&9), "las vecinas caben");
        assert!(!q.contains(&13), "la de dos hojas mas alla ya no cabe");
        assert!(
            q.contains(&40),
            "lo pequeno que aun cabe, aunque este lejos"
        );
        // Caso negativo: lo visible no se suelta aunque se pase del tope.
        let q = lo_que_cabe(&[(1, 500, 0), (2, 1, 1)], 100);
        assert!(q.contains(&1));
        assert!(!q.contains(&2));
    }
}
