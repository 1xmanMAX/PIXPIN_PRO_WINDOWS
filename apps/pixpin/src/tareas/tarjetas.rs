//! La logica pura de la vista de Tareas en tarjetas (tareas-v3): buscar,
//! agrupar, ordenar, colocar las tarjetas una tras otra y saber que hay bajo
//! el raton o adonde va el foco con las flechas.
//!
//! Aqui no se pinta nada: la ventana mide lo que ocupa cada tarjeta (el
//! texto se parte en renglones con DirectWrite) y se lo pasa a
//! [`disponer`]; lo demas se puede probar entero sin ventana.
//!
//! **Buscar** es como en el resto de la app (`pixpin_ui::resaltado`): sin
//! mirar mayusculas ni tildes, y han de estar TODAS las palabras, cada una en
//! el texto de la tarea, en el nombre de su lista o en el de su chat. Una
//! palabra que nombra un dia («hoy», «ayer», «anteayer», o `2026-10-01`)
//! vale tambien por la fecha en que se apunto la tarea.

#![forbid(unsafe_code)]

use pixpin_proyecto::mini;
use pixpin_ui::resaltado::hay_coincidencia;

use super::{Fila, Lista};

// ------------------------------------------------------------------ buscar

/// Las palabras de lo buscado. Sin palabras no se filtra nada.
pub fn palabras(consulta: &str) -> Vec<String> {
    consulta.split_whitespace().map(str::to_string).collect()
}

/// Una palabra en su forma de comparar: minusculas y sin tildes. Solo para
/// reconocer los nombres de los dias; buscar en el texto lo hace
/// `resaltado`, que ademas sabe donde cae lo encontrado.
fn plano(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            otra => otra,
        })
        .collect()
}

/// El dia que nombra una palabra, si nombra alguno.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dia {
    /// Hace tantos dias: hoy es 0, ayer 1.
    Hace(u32),
    /// Una fecha escrita entera (`2026-10-01`).
    El(mini::Fecha),
}

pub fn dia_de(palabra: &str) -> Option<Dia> {
    match plano(palabra).as_str() {
        "hoy" | "today" => Some(Dia::Hace(0)),
        "ayer" | "yesterday" => Some(Dia::Hace(1)),
        "anteayer" => Some(Dia::Hace(2)),
        otra => fecha_escrita(otra).map(Dia::El),
    }
}

/// `AAAA-MM-DD`, con un dia que exista en el calendario de a ojo (mes 1-12,
/// dia 1-31): basta para no confundir un numero cualquiera con una fecha.
fn fecha_escrita(s: &str) -> Option<mini::Fecha> {
    let mut t = s.split('-');
    let (a, m, d) = (t.next()?, t.next()?, t.next()?);
    if t.next().is_some() || a.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let f = mini::Fecha {
        anio: a.parse().ok()?,
        mes: m.parse().ok()?,
        dia: d.parse().ok()?,
    };
    ((1..=12).contains(&f.mes) && (1..=31).contains(&f.dia)).then_some(f)
}

impl Dia {
    /// Si una tarea creada el dia `creada` es de este dia.
    pub fn es(self, creada: Option<mini::Fecha>, hoy: mini::Fecha) -> bool {
        let Some(c) = creada else {
            return false;
        };
        match self {
            // Una fecha del futuro (el reloj de otro equipo) no es «hoy».
            Dia::Hace(n) => c.dias() <= hoy.dias() && mini::dias_desde(c, hoy) == n,
            Dia::El(f) => c == f,
        }
    }
}

/// Si la tarea `f` de la lista `l` sale con estas palabras.
pub fn coincide(l: &Lista, f: &Fila, palabras: &[String], hoy: mini::Fecha) -> bool {
    palabras.iter().all(|p| {
        hay_coincidencia(&f.texto, p)
            || hay_coincidencia(&l.titulo, p)
            || hay_coincidencia(&l.chat, p)
            || dia_de(p).is_some_and(|d| d.es(f.creada, hoy))
    })
}

// ----------------------------------------------------------------- agrupar

/// Un grupo de tarjetas: las de una lista que salen, ya ordenadas. Los
/// numeros son el sitio de cada tarea en `Lista::filas` (su `indice`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grupo {
    /// El sitio de la lista en las listas de la ventana.
    pub lista: usize,
    /// Lo pendiente (y lo recien tachado, que se queda en su sitio unos
    /// segundos), como lo ordena `Lista::a_la_vista`.
    pub arriba: Vec<usize>,
    /// Lo hecho, plegable, en el orden del documento.
    pub hechas: Vec<usize>,
}

impl Grupo {
    /// Cuantas de arriba siguen pendientes (lo recien tachado no cuenta).
    pub fn pendientes(&self, l: &Lista) -> usize {
        self.arriba
            .iter()
            .filter(|&&i| l.filas.get(i).is_some_and(|f| !f.hecha))
            .count()
    }
}

/// Los grupos que se ensenan, en el orden de las listas (el Inbox primero,
/// ver `ordenar_listas`). Sin buscar, todas las listas con alguna tarea;
/// buscando, solo lo que coincide, y una lista sin nada que coincida no
/// sale.
pub fn agrupar(
    listas: &[Lista],
    palabras: &[String],
    hoy: mini::Fecha,
    sigue_arriba: &dyn Fn(&Lista, &Fila) -> bool,
) -> Vec<Grupo> {
    listas
        .iter()
        .enumerate()
        .filter_map(|(li, l)| {
            let (arriba, hechas) = l.a_la_vista(&|f| sigue_arriba(l, f));
            let vale = |f: &&Fila| coincide(l, f, palabras, hoy);
            let arriba: Vec<usize> = arriba.into_iter().filter(vale).map(|f| f.indice).collect();
            let hechas: Vec<usize> = hechas.into_iter().filter(vale).map(|f| f.indice).collect();
            (!arriba.is_empty() || !hechas.is_empty()).then_some(Grupo {
                lista: li,
                arriba,
                hechas,
            })
        })
        .collect()
}

// ---------------------------------------------------------------- disponer

/// Lo que hay en cada sitio de la columna.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pieza {
    /// El encabezado de un grupo (su lista).
    Encabezado(usize),
    /// La tarjeta de la tarea `.1` (su `indice`) de la lista `.0`.
    Tarjeta(usize, usize),
    /// «Hechas (N)» de la lista `.0`.
    Pliegue(usize),
}

/// Una pieza en su sitio: `y` desde lo alto del contenido.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colocada {
    pub pieza: Pieza,
    pub y: f32,
    pub alto: f32,
}

/// Las medidas fijas de la columna, ya con la escala.
#[derive(Debug, Clone, Copy)]
pub struct Medidas {
    pub encabezado: f32,
    pub pliegue: f32,
    /// Entre tarjeta y tarjeta.
    pub hueco: f32,
    /// Entre un grupo y el siguiente.
    pub entre_grupos: f32,
}

/// Coloca los grupos uno tras otro: su encabezado, sus tarjetas de arriba,
/// y si tiene hechas su pliegue y, abiertas, las hechas. `alto_de(lista,
/// fila)` es lo que ocupa cada tarjeta (lo mide quien pinta). Devuelve las
/// piezas y el alto total.
pub fn disponer(
    grupos: &[Grupo],
    abierta: &dyn Fn(usize) -> bool,
    alto_de: &mut dyn FnMut(usize, usize) -> f32,
    m: Medidas,
) -> (Vec<Colocada>, f32) {
    let mut v = Vec::new();
    let mut y = 0.0;
    for (k, g) in grupos.iter().enumerate() {
        if k > 0 {
            y += m.entre_grupos;
        }
        let mut poner = |pieza: Pieza, alto: f32, y: &mut f32| {
            v.push(Colocada { pieza, y: *y, alto });
            *y += alto;
        };
        poner(Pieza::Encabezado(g.lista), m.encabezado, &mut y);
        let mut tarjetas = |filas: &[usize], y: &mut f32, v: &mut Vec<Colocada>| {
            for (n, &fi) in filas.iter().enumerate() {
                if n > 0 {
                    *y += m.hueco;
                }
                let alto = alto_de(g.lista, fi);
                v.push(Colocada {
                    pieza: Pieza::Tarjeta(g.lista, fi),
                    y: *y,
                    alto,
                });
                *y += alto;
            }
        };
        tarjetas(&g.arriba, &mut y, &mut v);
        if !g.hechas.is_empty() {
            if !g.arriba.is_empty() {
                y += m.hueco;
            }
            v.push(Colocada {
                pieza: Pieza::Pliegue(g.lista),
                y,
                alto: m.pliegue,
            });
            y += m.pliegue;
            if abierta(g.lista) {
                tarjetas(&g.hechas, &mut y, &mut v);
            }
        }
    }
    (v, y)
}

/// La pieza que hay a la altura `y` del contenido (los huecos entre piezas
/// no son de nadie). Las piezas van en orden, asi que se busca a saltos.
pub fn pieza_en(colocadas: &[Colocada], y: f32) -> Option<Pieza> {
    let i = colocadas.partition_point(|c| c.y + c.alto < y);
    colocadas
        .get(i)
        .filter(|c| y >= c.y && y <= c.y + c.alto)
        .map(|c| c.pieza)
}

/// Donde quedo la tarjeta `(lista, fila)`.
pub fn sitio_de(colocadas: &[Colocada], lista: usize, fila: usize) -> Option<Colocada> {
    colocadas
        .iter()
        .find(|c| c.pieza == Pieza::Tarjeta(lista, fila))
        .copied()
}

/// Las tarjetas en el orden en que se ven: por donde pasan las flechas.
pub fn en_orden(colocadas: &[Colocada]) -> Vec<(usize, usize)> {
    colocadas
        .iter()
        .filter_map(|c| match c.pieza {
            Pieza::Tarjeta(l, f) => Some((l, f)),
            _ => None,
        })
        .collect()
}

// ------------------------------------------------------------------- foco

/// La tarjeta a `paso` de la que tiene el foco (+1 la de abajo, -1 la de
/// arriba). Sin foco, la primera. `None` al pasarse por arriba: el foco
/// sube a la caja de apuntar. Por abajo se queda en la ultima.
pub fn vecina(
    orden: &[(usize, usize)],
    actual: Option<(usize, usize)>,
    paso: i32,
) -> Option<(usize, usize)> {
    let Some(a) = actual.and_then(|a| orden.iter().position(|&x| x == a)) else {
        return orden.first().copied();
    };
    let i = a as i64 + i64::from(paso);
    if i < 0 {
        return None;
    }
    orden
        .get((i as usize).min(orden.len().saturating_sub(1)))
        .copied()
}

/// Lo que hay que desplazar para que se vea entero lo que va de `y` a
/// `y + alto` (del contenido) en una vista de `vista` de alto: lo justo, y
/// nada si ya se ve.
pub fn a_la_vista(y: f32, alto: f32, scroll: f32, vista: f32) -> f32 {
    if y < scroll {
        y
    } else if y + alto > scroll + vista {
        (y + alto - vista).min(y)
    } else {
        scroll
    }
}

/// Donde quedo, tras releer, la tarea que tenia el foco: la de la misma
/// lista (por su clave) y el mismo texto guardado.
pub fn reubicar(listas: &[Lista], clave: &str, crudo: &str) -> Option<(usize, usize)> {
    let li = listas.iter().position(|l| l.clave() == clave)?;
    let fi = listas[li].filas.iter().position(|f| f.crudo == crudo)?;
    Some((li, fi))
}

// ----------------------------------------------------------------- Escape

/// Lo que hace Esc, por capas: lo de encima primero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Escape {
    SoltarMenu,
    VaciarCaja,
    VaciarBusqueda,
    Cerrar,
}

/// `en_la_caja`: el foco esta en la caja de apuntar. Lo que se esta
/// escribiendo se vacia antes que la busqueda; la busqueda antes de cerrar.
pub fn escape(menu: bool, en_la_caja: bool, caja_vacia: bool, busqueda_vacia: bool) -> Escape {
    if menu {
        Escape::SoltarMenu
    } else if en_la_caja && !caja_vacia {
        Escape::VaciarCaja
    } else if !busqueda_vacia {
        Escape::VaciarBusqueda
    } else if !caja_vacia {
        Escape::VaciarCaja
    } else {
        Escape::Cerrar
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::tareas::filas_de;

    fn dia(d: u8) -> mini::Fecha {
        mini::Fecha {
            anio: 2026,
            mes: 10,
            dia: d,
        }
    }

    fn lista(chat: &str, titulo: &str, cuando: i64, doc: &str) -> Lista {
        Lista {
            proyecto: chat.to_lowercase(),
            chat: chat.into(),
            guardados: false,
            codigo: format!("m{cuando}"),
            titulo: titulo.into(),
            cuando,
            filas: filas_de(doc),
        }
    }

    fn muestra() -> Vec<Lista> {
        vec![
            lista(
                "Mensajes guardados",
                "Inbox",
                30,
                "- [ ] Llamar al fontanero ➕ 2026-10-03\n- [x] pagar la luz ➕ 2026-10-02\n- [ ] comprar pan ➕ 2026-10-02",
            ),
            lista(
                "Obra Miraflores",
                "Pendientes",
                20,
                "- [ ] revisar puntales ➕ 2026-10-01\n- [x] pedir yeso",
            ),
        ]
    }

    fn nada(_: &Lista, _: &Fila) -> bool {
        false
    }

    #[test]
    fn buscar_sin_tildes_ni_mayusculas_y_con_todas_las_palabras() {
        let v = muestra();
        let hoy = dia(3);
        let sale = |q: &str| {
            agrupar(&v, &palabras(q), hoy, &nada)
                .iter()
                .flat_map(|g| {
                    g.arriba
                        .iter()
                        .chain(&g.hechas)
                        .map(|&i| v[g.lista].filas[i].texto.clone())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(sale("FONTANÉRO"), ["Llamar al fontanero"]);
        assert_eq!(sale("llamar fontanero"), ["Llamar al fontanero"]);
        // Por el nombre de la lista o del chat salen todas las suyas.
        assert_eq!(sale("miraflores"), ["revisar puntales", "pedir yeso"]);
        assert_eq!(sale("inbox pan"), ["comprar pan"]);
        // Sin buscar, todo, pendiente primero.
        assert_eq!(sale("  ").len(), 5);
        // Caso negativo: una palabra que no esta deja fuera aunque las demas
        // esten, y un grupo sin nada no sale.
        assert!(sale("fontanero yeso").is_empty());
        assert_eq!(agrupar(&v, &palabras("yeso"), hoy, &nada).len(), 1);
    }

    #[test]
    fn hoy_y_ayer_buscan_por_la_fecha_en_que_se_apunto() {
        let v = muestra();
        let hoy = dia(3);
        let g = agrupar(&v, &palabras("hoy"), hoy, &nada);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].arriba, [0]);
        let g = agrupar(&v, &palabras("Ayer"), hoy, &nada);
        assert_eq!(
            (g[0].arriba.clone(), g[0].hechas.clone()),
            (vec![2], vec![1])
        );
        let g = agrupar(&v, &palabras("2026-10-01"), hoy, &nada);
        assert_eq!((g[0].lista, g[0].arriba.clone()), (1, vec![0]));
        // Y se combina con palabras: «ayer pan».
        let g = agrupar(&v, &palabras("ayer pan"), hoy, &nada);
        assert_eq!(g[0].arriba, [2]);
        assert!(g[0].hechas.is_empty());
        // Caso negativo: una tarea sin fecha no es de hoy, una del futuro
        // tampoco, y un numero suelto no es una fecha.
        assert!(!Dia::Hace(0).es(None, hoy));
        assert!(!Dia::Hace(0).es(Some(dia(9)), hoy));
        assert_eq!(dia_de("2026-13-01"), None);
        assert_eq!(dia_de("2026"), None);
        assert_eq!(dia_de("hoyo"), None);
    }

    #[test]
    fn agrupar_deja_lo_pendiente_arriba_y_lo_hecho_aparte() {
        let v = muestra();
        let g = agrupar(&v, &[], dia(3), &nada);
        assert_eq!(g.len(), 2);
        // La mas vieja arriba (comprar pan es del 2, el fontanero del 3).
        assert_eq!(g[0].arriba, [2, 0]);
        assert_eq!(g[0].hechas, [1]);
        assert_eq!(g[0].pendientes(&v[0]), 2);
        // Lo recien tachado se queda arriba, pero no cuenta como pendiente.
        let g = agrupar(&v, &[], dia(3), &|_, f| f.texto == "pagar la luz");
        assert_eq!(g[0].arriba.len(), 3);
        assert_eq!(g[0].pendientes(&v[0]), 2);
        // Caso negativo: una lista vacia no hace grupo.
        let vacia = vec![lista("X", "Nada", 1, "# Nada\n")];
        assert!(agrupar(&vacia, &[], dia(3), &nada).is_empty());
    }

    const M: Medidas = Medidas {
        encabezado: 40.0,
        pliegue: 30.0,
        hueco: 8.0,
        entre_grupos: 20.0,
    };

    #[test]
    fn disponer_pone_las_tarjetas_una_tras_otra_y_el_pliegue_detras() {
        let v = muestra();
        let g = agrupar(&v, &[], dia(3), &nada);
        let (c, total) = disponer(&g, &|_| false, &mut |_, _| 60.0, M);
        let piezas: Vec<Pieza> = c.iter().map(|c| c.pieza).collect();
        assert_eq!(
            piezas,
            [
                Pieza::Encabezado(0),
                Pieza::Tarjeta(0, 2),
                Pieza::Tarjeta(0, 0),
                Pieza::Pliegue(0),
                Pieza::Encabezado(1),
                Pieza::Tarjeta(1, 0),
                Pieza::Pliegue(1),
            ]
        );
        assert_eq!(c[1].y, 40.0);
        assert_eq!(c[2].y, 40.0 + 60.0 + 8.0);
        assert_eq!(c[3].y, 40.0 + 60.0 + 8.0 + 60.0 + 8.0);
        assert_eq!(c[4].y, c[3].y + 30.0 + 20.0);
        assert_eq!(total, c[6].y + 30.0);
        // Abierto el pliegue del primero, sus hechas van debajo.
        let (c2, total2) = disponer(&g, &|l| l == 0, &mut |_, _| 60.0, M);
        assert_eq!(c2[4].pieza, Pieza::Tarjeta(0, 1));
        assert_eq!(total2, total + 60.0);
        // Caso negativo: plegado, lo hecho no ocupa sitio ni es tarjeta.
        assert!(!en_orden(&c).contains(&(0, 1)));
    }

    #[test]
    fn pieza_en_atina_y_los_huecos_no_son_de_nadie() {
        let v = muestra();
        let g = agrupar(&v, &[], dia(3), &nada);
        // Alturas distintas: un texto largo ocupa mas renglones.
        let (c, _) = disponer(
            &g,
            &|_| false,
            &mut |_, f| if f == 2 { 90.0 } else { 56.0 },
            M,
        );
        assert_eq!(pieza_en(&c, 5.0), Some(Pieza::Encabezado(0)));
        assert_eq!(pieza_en(&c, 40.0 + 89.0), Some(Pieza::Tarjeta(0, 2)));
        assert_eq!(
            pieza_en(&c, 40.0 + 90.0 + 8.0 + 1.0),
            Some(Pieza::Tarjeta(0, 0))
        );
        assert_eq!(sitio_de(&c, 0, 0).map(|x| x.alto), Some(56.0));
        // Caso negativo: el hueco entre tarjetas, antes de todo y despues
        // de todo no dan nada.
        assert_eq!(pieza_en(&c, 40.0 + 90.0 + 4.0), None);
        assert_eq!(pieza_en(&c, -1.0), None);
        assert_eq!(pieza_en(&c, 10_000.0), None);
        assert_eq!(sitio_de(&c, 9, 9), None);
    }

    #[test]
    fn las_flechas_pasan_de_tarjeta_en_tarjeta() {
        let orden = [(0, 2), (0, 0), (1, 0)];
        assert_eq!(
            vecina(&orden, None, 1),
            Some((0, 2)),
            "sin foco, la primera"
        );
        assert_eq!(vecina(&orden, Some((0, 2)), 1), Some((0, 0)));
        assert_eq!(
            vecina(&orden, Some((0, 0)), 1),
            Some((1, 0)),
            "salta de grupo"
        );
        assert_eq!(
            vecina(&orden, Some((1, 0)), 1),
            Some((1, 0)),
            "abajo se queda"
        );
        assert_eq!(vecina(&orden, Some((1, 0)), -1), Some((0, 0)));
        // Caso negativo: por arriba de la primera no hay tarjeta (sube a la
        // caja), y sin tarjetas no hay foco.
        assert_eq!(vecina(&orden, Some((0, 2)), -1), None);
        assert_eq!(vecina(&[], None, 1), None);
        assert_eq!(vecina(&[], Some((0, 0)), 1), None);
    }

    #[test]
    fn a_la_vista_desplaza_lo_justo() {
        assert_eq!(a_la_vista(100.0, 50.0, 0.0, 400.0), 0.0, "ya se ve");
        assert_eq!(a_la_vista(500.0, 50.0, 0.0, 400.0), 150.0, "baja lo justo");
        assert_eq!(
            a_la_vista(20.0, 50.0, 300.0, 400.0),
            20.0,
            "sube hasta ella"
        );
        // Caso negativo: una tarjeta mas alta que la vista se ensena desde
        // arriba, no desde su final.
        assert_eq!(a_la_vista(500.0, 900.0, 0.0, 400.0), 500.0);
    }

    #[test]
    fn el_foco_sigue_a_su_tarea_al_releer() {
        let mut v = muestra();
        let clave = v[0].clave();
        let crudo = v[0].filas[2].crudo.clone();
        // Llega del movil una tarea nueva delante: el numero cambia.
        v[0].filas = filas_de(
            "- [ ] nueva\n- [ ] Llamar al fontanero ➕ 2026-10-03\n- [x] pagar la luz ➕ 2026-10-02\n- [ ] comprar pan ➕ 2026-10-02",
        );
        assert_eq!(reubicar(&v, &clave, &crudo), Some((0, 3)));
        // Caso negativo: si ya no esta (se movio o se borro), no hay foco.
        assert_eq!(reubicar(&v, &clave, "otra"), None);
        assert_eq!(reubicar(&v, "no/existe", &crudo), None);
    }

    #[test]
    fn escape_va_por_capas() {
        assert_eq!(escape(true, true, false, false), Escape::SoltarMenu);
        assert_eq!(escape(false, true, false, false), Escape::VaciarCaja);
        assert_eq!(escape(false, false, false, false), Escape::VaciarBusqueda);
        assert_eq!(escape(false, true, true, false), Escape::VaciarBusqueda);
        assert_eq!(escape(false, false, false, true), Escape::VaciarCaja);
        assert_eq!(escape(false, false, true, true), Escape::Cerrar);
        // Caso negativo: con algo buscado nunca cierra a la primera.
        assert_ne!(escape(false, false, true, false), Escape::Cerrar);
    }
}
