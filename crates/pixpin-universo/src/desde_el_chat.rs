//! **El universo se arma solo con la forma del chat** (H2; Android v0.84-v0.85,
//! `ui/UniversoModelo.kt:177-291`).
//!
//! En el movil el proyecto es un espacio; cada archivo del chat, un sistema
//! solar; lo que le contesta, sus planetas; y en un PDF las paginas son
//! planetas y lo comentado en una pagina, sus satelites. Cada cosa nueva cae
//! **en orbita** alrededor de su sol, atada a el con una raya, y el sol es
//! grande en el centro.
//!
//! El PC tiene la jerarquia fija galaxia > planeta > luna (`jerarquia`), asi
//! que la misma idea se lleva asi:
//!
//! - el proyecto es su **galaxia**, y su centro es el sol;
//! - un mensaje sin respuestas es una **luna** en orbita del sol;
//! - un mensaje con respuestas (o un PDF con sus paginas) es un **planeta
//!   sistema**: su luna en el centro, de sol, y lo que le contesta en orbita
//!   dentro del planeta, a escala menor. Es el subespacio del movil: en vez de
//!   «entrar» se acerca uno con el zoom, que es lo que el universo del PC ya
//!   hace (D218);
//! - lo que contesta a una respuesta (la tercera hondura del movil) se aplana
//!   en el mismo planeta: la jerarquia no baja mas, y en un monitor grande no
//!   hace falta.
//!
//! Igual que el movil: lo ya puesto **se respeta tal cual** (sitio, tamano,
//! color) y lo quitado a mano **no vuelve** (`Universo::quitados`).
//!
//! Puro, como el resto del crate: recibe los nodos ya traducidos del chat.

use std::collections::{HashMap, HashSet};

use crate::astro::{Astro, Clase, IdAstro, RADIO_LUNA};
use crate::universo::Universo;

/// Un trozo del chat tal como lo ve el universo (`NodoDelChat` del movil).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodoDelChat {
    /// `Mensaje::codigo_unico`: el mismo codigo que la luna.
    pub codigo: String,
    /// El codigo del mensaje al que contesta. `None`: cuelga del proyecto.
    pub responde_a: Option<String>,
    /// Si es una pagina de un documento: su numero, desde 1. Ordena las
    /// paginas dentro de su PDF.
    pub pagina: Option<u32>,
    /// Si lo que tiene dentro son paginas de un documento (un PDF del chat).
    pub es_documento: bool,
    /// Si es solo texto, un comentario, y no un archivo.
    pub es_texto: bool,
    /// Si ese texto es solo emoticonos: se ve como el emoji suelto que es.
    pub solo_emoji: bool,
}

/// Hasta donde se sigue un hilo hacia dentro (`HONDURA_MAXIMA` del movil):
/// un hilo que se responde a si mismo no da vueltas para siempre.
pub const HONDURA_MAXIMA: usize = 12;

/// Lo lejos que orbita el primer anillo, en dp del mundo del movil.
pub const RADIO_DE_LA_PRIMERA_ORBITA: f32 = 190.0;
/// El sol y un cuerpo del movil, en dp (`Universo.kt:928`, v0.85).
pub const RADIO_DEL_SOL_MOVIL: f32 = 110.0;
pub const RADIO_DEL_CUERPO_MOVIL: f32 = 30.0;
/// Cuantos cuerpos caben en un anillo.
const POR_VUELTA: usize = 8;

/// Un dp del movil en unidades del mundo de una galaxia: el cuerpo del movil
/// (30 dp) mide lo que una luna del PC (48). Asi las proporciones del movil
/// —sol, orbitas y cuerpos— se copian tal cual.
pub const ESCALA_GALAXIA: f32 = RADIO_LUNA / RADIO_DEL_CUERPO_MOVIL;
/// El sol del centro de la galaxia abierta, en mundo.
pub const RADIO_DEL_SOL: f32 = RADIO_DEL_SOL_MOVIL * ESCALA_GALAXIA;
/// Dentro de un planeta sistema todo va a esta escala: es el subespacio del
/// movil visto de lejos. Con 0,4 una respuesta es una luna de 12 de radio y
/// se lee (nivel `Ficha`) a zoom 4, lejos del tope de 30.
pub const ESCALA_SISTEMA: f32 = 0.4;
/// Lo que se deja entre lo de dentro y el borde de su contenedor.
const MARGEN: f32 = 20.0;

/// **Donde cae el que hace el numero `cual` en la orbita** (`enOrbita` del
/// movil, en dp): un anillo por tanda de ocho, cada uno mas lejos, y el
/// angulo girado para que dos vueltas no se tapen.
pub fn en_orbita(cual: usize, hondura: usize) -> (f32, f32) {
    let vuelta = cual / POR_VUELTA;
    let radio = RADIO_DE_LA_PRIMERA_ORBITA * (1.0 + vuelta as f32 * 0.55);
    let angulo = (cual % POR_VUELTA) as f64 * (2.0 * std::f64::consts::PI / POR_VUELTA as f64)
        + vuelta as f64 * 0.4
        + hondura as f64 * 0.7;
    (
        (radio as f64 * angulo.cos()) as f32,
        (radio as f64 * angulo.sin()) as f32,
    )
}

/// El tamano con que nace un cuerpo (v0.85): un comentario, pequeno; lo que
/// lleva algo dentro, un pelo mas grande; lo demas, entero.
pub fn tamano(n: &NodoDelChat, con_hijos: bool) -> f32 {
    if n.es_texto {
        0.75
    } else if con_hijos || n.es_documento {
        1.2
    } else {
        1.0
    }
}

/// El radio de un planeta sistema con `hijos` cuerpos en orbita: hasta su
/// ultimo anillo, el cuerpo mas grande y un margen.
pub fn radio_de_sistema(hijos: usize) -> f32 {
    let vuelta = hijos.saturating_sub(1) / POR_VUELTA;
    let orbita = RADIO_DE_LA_PRIMERA_ORBITA * (1.0 + vuelta as f32 * 0.55);
    (orbita + RADIO_DEL_CUERPO_MOVIL * 1.2) * ESCALA_SISTEMA + MARGEN
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InformeChat {
    /// Lunas nuevas en el cielo.
    pub lunas: usize,
    /// Planetas sistema nuevos.
    pub sistemas: usize,
}

/// Lo que cuelga de cada nodo, ya en el orden en que se pone.
struct Arbol<'a> {
    hijos: HashMap<&'a str, Vec<&'a NodoDelChat>>,
    raices: Vec<&'a NodoDelChat>,
}

impl<'a> Arbol<'a> {
    fn de(nodos: &'a [NodoDelChat]) -> Self {
        let hay: HashSet<&str> = nodos.iter().map(|n| n.codigo.as_str()).collect();
        let mut hijos: HashMap<&str, Vec<&NodoDelChat>> = HashMap::new();
        let mut raices = Vec::new();
        for n in nodos {
            // Una respuesta a algo que no esta en el chat (o a si misma)
            // colgaria del vacio: sube al proyecto, como en el movil.
            match n.responde_a.as_deref() {
                Some(p) if hay.contains(p) && p != n.codigo => {
                    hijos.entry(p).or_default().push(n)
                }
                _ => raices.push(n),
            }
        }
        // Las paginas en su orden; lo demas, en el del chat (orden estable).
        for v in hijos.values_mut() {
            v.sort_by_key(|n| n.pagina.unwrap_or(u32::MAX));
        }
        Self { hijos, raices }
    }
}

/// **Arma lo que falte de la galaxia de `proyecto` con la forma del chat.**
///
/// No pasa por el deshacer: es la verdad de fuera, como `galaxias::
/// sincronizar`. Devuelve cuanto puso.
pub fn armar(u: &mut Universo, proyecto: &str, nodos: &[NodoDelChat]) -> InformeChat {
    let mut inf = InformeChat::default();
    let Some(g) = u
        .astros
        .iter()
        .find(|a| matches!(&a.clase, Clase::Galaxia { proyecto: p } if p == proyecto))
        .map(|a| a.id)
    else {
        return inf;
    };
    // Lo quitado que el deshacer devolvio al cielo ya no cuenta como quitado.
    let quitados_propios: Vec<String> = u
        .quitados
        .iter()
        .filter(|c| u.luna_de(c).is_none())
        .cloned()
        .collect();
    let quitados: HashSet<&str> = quitados_propios.iter().map(String::as_str).collect();
    let arbol = Arbol::de(nodos);
    for raiz in &arbol.raices {
        if quitados.contains(raiz.codigo.as_str()) {
            continue;
        }
        let hijos = preorden(&arbol, &raiz.codigo, &quitados);
        if hijos.is_empty() {
            if u.luna_de(&raiz.codigo).is_none() {
                let r = RADIO_LUNA * tamano(raiz, false);
                let (x, y) = hueco(u, g, r, 0, RADIO_DEL_SOL, ESCALA_GALAXIA);
                poner_luna(u, g, proyecto, &raiz.codigo, x, y, r);
                inf.lunas += 1;
            }
            continue;
        }
        let Some(planeta) = sistema(u, g, proyecto, raiz, hijos.len(), &mut inf) else {
            continue;
        };
        for (n, _h) in &hijos {
            if u.luna_de(&n.codigo).is_some() {
                continue;
            }
            let con_hijos = arbol.hijos.contains_key(n.codigo.as_str());
            let r = RADIO_DEL_CUERPO_MOVIL * ESCALA_SISTEMA * tamano(n, con_hijos);
            let sol = RADIO_DEL_SOL_MOVIL * ESCALA_SISTEMA;
            // Todo lo del planeta en la misma hondura: aplanado, lo de la
            // tercera hondura tiene que caer en los mismos anillos y no
            // girado encima de ellos.
            let (x, y) = hueco(u, planeta, r, 1, sol, ESCALA_SISTEMA);
            poner_luna(u, planeta, proyecto, &n.codigo, x, y, r);
            inf.lunas += 1;
        }
    }
    if inf != InformeChat::default() {
        u.marcar_cambio();
    }
    inf
}

/// Lo que cuelga de `raiz`, aplanado en preorden (la pagina y enseguida lo
/// comentado en ella), sin lo quitado ni lo que pase de `HONDURA_MAXIMA`.
fn preorden<'a>(
    arbol: &Arbol<'a>,
    raiz: &str,
    quitados: &HashSet<&str>,
) -> Vec<(&'a NodoDelChat, usize)> {
    let mut salida = Vec::new();
    let mut vistos: HashSet<String> = HashSet::from([raiz.to_string()]);
    fn bajar<'a>(
        arbol: &Arbol<'a>,
        p: &str,
        h: usize,
        quitados: &HashSet<&str>,
        vistos: &mut HashSet<String>,
        salida: &mut Vec<(&'a NodoDelChat, usize)>,
    ) {
        if h > HONDURA_MAXIMA {
            return;
        }
        let Some(hs) = arbol.hijos.get(p) else {
            return;
        };
        for n in hs {
            if quitados.contains(n.codigo.as_str()) || !vistos.insert(n.codigo.clone()) {
                continue;
            }
            salida.push((*n, h));
            bajar(arbol, &n.codigo, h + 1, quitados, vistos, salida);
        }
    }
    bajar(arbol, raiz, 1, quitados, &mut vistos, &mut salida);
    salida
}

/// El planeta sistema de `raiz`: el que ya tiene, o uno nuevo. `None` si su
/// luna esta puesta a mano donde no cabe un planeta alrededor: entonces sus
/// respuestas se quedan en la nebulosa, que es mejor que mover lo del
/// usuario o taparle lo de al lado.
fn sistema(
    u: &mut Universo,
    g: IdAstro,
    proyecto: &str,
    raiz: &NodoDelChat,
    hijos: usize,
    inf: &mut InformeChat,
) -> Option<IdAstro> {
    if let Some(p) = u
        .astros
        .iter()
        .find(|a| a.sistema_de.as_deref() == Some(raiz.codigo.as_str()))
    {
        return Some(p.id);
    }
    let radio = radio_de_sistema(hijos);
    let id = match u.luna_de(&raiz.codigo).map(|l| (l.id, l.x, l.y, l.padre)) {
        None => {
            let (x, y) = hueco(u, g, radio, 0, RADIO_DEL_SOL, ESCALA_GALAXIA);
            let id = nuevo_planeta(u, g, &raiz.codigo, x, y, radio);
            let r = RADIO_DEL_SOL_MOVIL * ESCALA_SISTEMA;
            poner_luna(u, id, proyecto, &raiz.codigo, x, y, r);
            // El sol de su sistema no es un cuerpo en orbita.
            if let Some(l) = u.astros.last_mut() {
                l.atado = false;
            }
            inf.lunas += 1;
            id
        }
        Some((luna, x, y, padre)) => {
            if padre != Some(g) || !libre(u, g, x, y, radio, &[luna]) {
                return None;
            }
            let id = nuevo_planeta(u, g, &raiz.codigo, x, y, radio);
            if let Some(l) = u.astros.iter_mut().find(|a| a.id == luna) {
                l.padre = Some(id);
            }
            id
        }
    };
    inf.sistemas += 1;
    Some(id)
}

fn nuevo_planeta(u: &mut Universo, g: IdAstro, codigo: &str, x: f32, y: f32, r: f32) -> IdAstro {
    let id = u.nuevo_id();
    let mut p = Astro::planeta(id, x, y, r);
    p.padre = Some(g);
    p.atado = true;
    p.sistema_de = Some(codigo.to_string());
    u.astros.push(p);
    crecer(u, g, x, y, r);
    id
}

#[allow(clippy::too_many_arguments)] // una luna es todo esto
fn poner_luna(
    u: &mut Universo,
    padre: IdAstro,
    proyecto: &str,
    codigo: &str,
    x: f32,
    y: f32,
    r: f32,
) {
    let id = u.nuevo_id();
    let mut l = Astro::luna(id, codigo, proyecto, x, y);
    l.radio = r;
    l.padre = Some(padre);
    l.atado = true;
    u.astros.push(l);
    crecer(u, padre, x, y, r);
}

/// Si el contenedor se queda corto, crece: lo puesto no se sale nunca de el
/// (si se saliera, arrastrarlo luego se rechazaria por «fuera de galaxia»).
fn crecer(u: &mut Universo, contenedor: IdAstro, x: f32, y: f32, r: f32) {
    let margen = match u.astro(contenedor).map(|a| &a.clase) {
        Some(Clase::Galaxia { .. }) => 2.0 * RADIO_LUNA,
        _ => MARGEN,
    };
    if let Some(c) = u.astros.iter_mut().find(|a| a.id == contenedor) {
        let hace_falta = ((x - c.x).powi(2) + (y - c.y).powi(2)).sqrt() + r + margen;
        if hace_falta > c.radio {
            c.radio = hace_falta;
        }
    }
}

/// Si un circulo de radio `r` en `(x, y)` no pisa a nadie de dentro de
/// `contenedor` (sin contar `ignorar`).
fn libre(u: &Universo, contenedor: IdAstro, x: f32, y: f32, r: f32, ignorar: &[IdAstro]) -> bool {
    u.hijos(contenedor)
        .filter(|a| !ignorar.contains(&a.id))
        .all(|a| ((a.x - x).powi(2) + (a.y - y).powi(2)).sqrt() >= a.radio + r)
}

/// **El siguiente sitio en orbita** dentro de `contenedor`. Empieza donde el
/// movil (el numero de cuerpos que ya tiene) y, si ese sitio esta pisado —lo
/// quitado deja huecos y el numero vuelve a caer en uno lleno—, sigue al
/// siguiente. Tampoco se mete debajo del sol.
fn hueco(
    u: &Universo,
    contenedor: IdAstro,
    r: f32,
    hondura: usize,
    sol: f32,
    escala: f32,
) -> (f32, f32) {
    let Some(c) = u.astro(contenedor) else {
        return (0.0, 0.0);
    };
    let (cx, cy) = (c.x, c.y);
    // Sin contar el sol de un sistema, que esta en el centro y no en orbita.
    let cuantos = u
        .hijos(contenedor)
        .filter(|a| !(a.x == cx && a.y == cy))
        .count();
    // Los de dentro, una vez: mirarlos en el `Universo` por cada sitio
    // probado era recorrer todos los astros cada vez (medido: un chat de 600
    // mensajes tardaba minutos sin optimizar).
    let dentro: Vec<(f32, f32, f32)> = u.hijos(contenedor).map(|a| (a.x, a.y, a.radio)).collect();
    let mut primero = None;
    for cual in cuantos..cuantos + 4000 {
        let (dx, dy) = en_orbita(cual, hondura);
        let (x, y) = (cx + dx * escala, cy + dy * escala);
        primero.get_or_insert((x, y));
        let lejos_del_sol = (dx * escala).hypot(dy * escala) - r >= sol;
        if lejos_del_sol
            && dentro
                .iter()
                .all(|(ax, ay, ar)| (ax - x).hypot(ay - y) >= ar + r)
        {
            return (x, y);
        }
    }
    primero.unwrap_or((cx, cy))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::galaxias::sincronizar;

    fn nodo(codigo: &str) -> NodoDelChat {
        NodoDelChat {
            codigo: codigo.into(),
            ..Default::default()
        }
    }

    fn responde(codigo: &str, a: &str) -> NodoDelChat {
        NodoDelChat {
            codigo: codigo.into(),
            responde_a: Some(a.into()),
            ..Default::default()
        }
    }

    fn texto(codigo: &str, a: Option<&str>) -> NodoDelChat {
        NodoDelChat {
            codigo: codigo.into(),
            responde_a: a.map(String::from),
            es_texto: true,
            ..Default::default()
        }
    }

    fn pagina(codigo: &str, de: &str, n: u32) -> NodoDelChat {
        NodoDelChat {
            codigo: codigo.into(),
            responde_a: Some(de.into()),
            pagina: Some(n),
            ..Default::default()
        }
    }

    /// El chat de la prueba del movil (`UniversoDesdeElChatTest`).
    fn chat() -> Vec<NodoDelChat> {
        vec![
            NodoDelChat {
                codigo: "pdf".into(),
                es_documento: true,
                ..Default::default()
            },
            // Las paginas llegan desordenadas a proposito.
            pagina("p2", "pdf", 2),
            pagina("p1", "pdf", 1),
            texto("c1", Some("p1")),
            texto("c2", Some("c1")),
            nodo("foto"),
            texto("suelto", None),
        ]
    }

    fn con_galaxia() -> Universo {
        let mut u = Universo::nuevo();
        sincronizar(&mut u, &["pr-1".to_string()]);
        u
    }

    fn galaxia(u: &Universo) -> &Astro {
        u.astros
            .iter()
            .find(|a| a.proyecto() == Some("pr-1") && a.es_contenedor())
            .unwrap()
    }

    fn armado() -> Universo {
        let mut u = con_galaxia();
        armar(&mut u, "pr-1", &chat());
        u
    }

    fn luna<'a>(u: &'a Universo, c: &str) -> &'a Astro {
        u.luna_de(c).unwrap_or_else(|| panic!("falta la luna {c}"))
    }

    #[test]
    fn la_orbita_es_la_del_movil_anillos_de_ocho_con_el_radio_creciendo_un_55_por_ciento() {
        let (x, y) = en_orbita(0, 0);
        assert!((x - 190.0).abs() < 1e-3 && y.abs() < 1e-3);
        // El noveno abre el segundo anillo, girado 0,4 rad.
        let (x, y) = en_orbita(8, 0);
        assert!((x.hypot(y) - 190.0 * 1.55).abs() < 1e-2);
        assert!((y.atan2(x) - 0.4).abs() < 1e-4);
        // La hondura gira 0,7 rad.
        let (x, y) = en_orbita(0, 1);
        assert!((y.atan2(x) - 0.7).abs() < 1e-4);
    }

    #[test]
    fn los_archivos_del_chat_orbitan_el_sol_de_su_galaxia_y_quedan_atados() {
        let u = armado();
        let g = galaxia(&u).clone();
        for c in ["foto", "suelto"] {
            let l = luna(&u, c);
            assert_eq!(l.padre, Some(g.id), "{c} cuelga de la galaxia");
            assert!(l.atado, "{c} atado al sol");
            let d = (l.x - g.x).hypot(l.y - g.y);
            assert!(d - l.radio >= RADIO_DEL_SOL, "{c} no cae bajo el sol: {d}");
        }
        // Un comentario suelto nace pequeno; un archivo, entero.
        assert!(luna(&u, "suelto").radio < luna(&u, "foto").radio);
        assert_eq!(luna(&u, "foto").radio, RADIO_LUNA);
    }

    #[test]
    fn el_pdf_es_un_sistema_con_el_de_sol_y_sus_paginas_en_orbita_por_su_orden() {
        let u = armado();
        let g = galaxia(&u).id;
        let sistema = u
            .astros
            .iter()
            .find(|a| a.sistema_de.as_deref() == Some("pdf"))
            .expect("el PDF abre su sistema");
        assert_eq!(sistema.padre, Some(g));
        assert!(matches!(sistema.clase, Clase::Planeta));
        let sol = luna(&u, "pdf");
        assert_eq!(sol.padre, Some(sistema.id));
        assert_eq!((sol.x, sol.y), (sistema.x, sistema.y), "el sol en el centro");
        // Las paginas, sus planetas, en el orden de sus numeros.
        let (p1, p2) = (luna(&u, "p1"), luna(&u, "p2"));
        assert_eq!(p1.padre, Some(sistema.id));
        let angulo = |l: &Astro| (l.y - sistema.y).atan2(l.x - sistema.x);
        assert!(angulo(p1) < angulo(p2), "la pagina 1 va antes que la 2");
        // Lo comentado en una pagina y el comentario al comentario, aplanados
        // en el mismo sistema: la jerarquia del PC no baja mas.
        assert_eq!(luna(&u, "c1").padre, Some(sistema.id));
        assert_eq!(luna(&u, "c2").padre, Some(sistema.id));
        // Todo dentro del planeta.
        for c in ["p1", "p2", "c1", "c2"] {
            let l = luna(&u, c);
            let d = (l.x - sistema.x).hypot(l.y - sistema.y);
            assert!(d + l.radio <= sistema.radio, "{c} se sale del sistema");
        }
    }

    #[test]
    fn lo_que_no_tiene_respuestas_no_abre_sistema() {
        let u = armado();
        assert!(
            u.astros
                .iter()
                .all(|a| a.sistema_de.as_deref() != Some("foto"))
        );
        assert_eq!(
            u.astros.iter().filter(|a| a.sistema_de.is_some()).count(),
            1
        );
    }

    #[test]
    fn volver_a_armarlo_no_duplica_ni_mueve_lo_que_el_usuario_coloco() {
        let mut u = armado();
        let antes = u.astros.len();
        let id = luna(&u, "foto").id;
        u.astros.iter_mut().find(|a| a.id == id).unwrap().x += 77.0;
        let x = luna(&u, "foto").x;
        let inf = armar(&mut u, "pr-1", &chat());
        assert_eq!(inf, InformeChat::default());
        assert_eq!(u.astros.len(), antes, "no se mete dos veces");
        assert_eq!(luna(&u, "foto").x, x);
    }

    #[test]
    fn lo_nuevo_del_chat_entra_donde_le_toca() {
        let mut u = armado();
        let mut mas = chat();
        mas.push(texto("c3", Some("p2")));
        mas.push(nodo("nuevo"));
        let inf = armar(&mut u, "pr-1", &mas);
        assert_eq!(inf.lunas, 2);
        let sistema = u
            .astros
            .iter()
            .find(|a| a.sistema_de.as_deref() == Some("pdf"))
            .unwrap()
            .id;
        assert_eq!(luna(&u, "c3").padre, Some(sistema));
        assert_eq!(luna(&u, "nuevo").padre, Some(galaxia(&u).id));
    }

    #[test]
    fn lo_que_se_quito_a_mano_no_vuelve_solo_y_se_lleva_lo_suyo() {
        let mut u = con_galaxia();
        u.quitados = vec!["foto".into(), "p1".into()];
        armar(&mut u, "pr-1", &chat());
        assert!(u.luna_de("foto").is_none(), "la foto se quedo fuera");
        assert!(u.luna_de("p1").is_none());
        // Lo comentado en la pagina quitada tampoco sale, como en el movil.
        assert!(u.luna_de("c1").is_none() && u.luna_de("c2").is_none());
        assert!(u.luna_de("pdf").is_some() && u.luna_de("p2").is_some());
    }

    #[test]
    fn un_hilo_que_se_responde_a_si_mismo_no_da_vueltas_para_siempre() {
        let mut u = con_galaxia();
        // a <- b <- a2, y un par que se contesta en circulo (sin raiz).
        let nodos = vec![
            nodo("a"),
            responde("b", "a"),
            responde("a2", "b"),
            responde("x", "y"),
            responde("y", "x"),
            responde("z", "z"),
        ];
        let inf = armar(&mut u, "pr-1", &nodos);
        assert!(u.luna_de("a2").is_some());
        // Lo que contesta a si mismo sube al proyecto; el circulo no tiene
        // por donde entrar y no se pone.
        assert!(u.luna_de("z").is_some());
        assert!(u.luna_de("x").is_none());
        assert_eq!(inf.lunas, 4);
    }

    #[test]
    fn sin_galaxia_del_proyecto_no_pone_nada() {
        let mut u = con_galaxia();
        assert_eq!(armar(&mut u, "otro", &chat()), InformeChat::default());
        assert_eq!(u.astros.len(), 1);
    }

    #[test]
    fn muchos_cuerpos_no_se_pisan_y_la_galaxia_crece_para_que_quepan() {
        let mut u = con_galaxia();
        let nodos: Vec<NodoDelChat> = (0..150).map(|i| nodo(&format!("m{i}"))).collect();
        armar(&mut u, "pr-1", &nodos);
        let g = galaxia(&u).clone();
        let lunas: Vec<&Astro> = u.astros.iter().filter(|a| a.padre == Some(g.id)).collect();
        assert_eq!(lunas.len(), 150);
        for (i, a) in lunas.iter().enumerate() {
            let d = (a.x - g.x).hypot(a.y - g.y);
            assert!(d + a.radio <= g.radio, "se sale de la galaxia");
            for b in &lunas[i + 1..] {
                assert!(
                    (a.x - b.x).hypot(a.y - b.y) >= a.radio + b.radio,
                    "dos cuerpos pisados"
                );
            }
        }
    }

    #[test]
    fn un_hueco_que_deja_lo_quitado_no_hace_caer_lo_nuevo_encima_de_otro() {
        let mut u = con_galaxia();
        let nodos: Vec<NodoDelChat> = (0..5).map(|i| nodo(&format!("m{i}"))).collect();
        armar(&mut u, "pr-1", &nodos);
        // Se quita la segunda: ahora hay 4 y el sitio 4 esta ocupado por m4.
        let id = u.luna_de("m1").unwrap().id;
        let mut escena = pixpin_motor2d::Escena::nueva();
        u.borrar(&[id], &mut escena);
        armar(&mut u, "pr-1", &[nodo("nuevo")]);
        let n = u.luna_de("nuevo").unwrap().clone();
        for a in u.astros.iter().filter(|a| a.codigo().is_some() && a.id != n.id) {
            assert!((a.x - n.x).hypot(a.y - n.y) >= a.radio + n.radio);
        }
        assert!(u.luna_de("m1").is_none(), "y la quitada no vuelve");
    }

    #[test]
    fn una_luna_puesta_a_mano_con_respuestas_nuevas_se_vuelve_sistema_si_cabe() {
        let mut u = con_galaxia();
        let g = galaxia(&u).clone();
        u.colocar_luna("pdf", "pr-1", g.x + 900.0, g.y).unwrap();
        armar(&mut u, "pr-1", &[nodo("pdf"), pagina("p1", "pdf", 1)]);
        let l = luna(&u, "pdf");
        assert_eq!((l.x, l.y), (g.x + 900.0, g.y), "no se mueve");
        let s = u.astro(l.padre.unwrap()).unwrap();
        assert_eq!(s.sistema_de.as_deref(), Some("pdf"));
        assert_eq!(luna(&u, "p1").padre, Some(s.id));
    }

    #[test]
    fn una_luna_puesta_a_mano_pegada_a_otra_no_se_envuelve_y_sus_respuestas_esperan() {
        let mut u = con_galaxia();
        let g = galaxia(&u).clone();
        u.colocar_luna("pdf", "pr-1", g.x + 900.0, g.y).unwrap();
        u.colocar_luna("vecina", "pr-1", g.x + 1000.0, g.y).unwrap();
        armar(&mut u, "pr-1", &[nodo("pdf"), pagina("p1", "pdf", 1)]);
        assert_eq!(luna(&u, "pdf").padre, Some(g.id));
        assert!(u.luna_de("p1").is_none(), "se queda en la nebulosa");
        assert!(u.astros.iter().all(|a| a.sistema_de.is_none()));
    }

    /// Se arma en el hilo de la ventana al llegar el cuaderno: tiene que ser
    /// poco. 600 mensajes con 40 PDF de 8 paginas.
    #[test]
    fn armar_un_chat_de_seiscientos_mensajes_cuesta_poco() {
        let mut nodos = Vec::new();
        for d in 0..40 {
            nodos.push(NodoDelChat {
                codigo: format!("d{d}"),
                es_documento: true,
                ..Default::default()
            });
            for p in 0..8 {
                nodos.push(pagina(&format!("d{d}p{p}"), &format!("d{d}"), p + 1));
            }
        }
        for n in 0..240 {
            nodos.push(texto(&format!("n{n}"), None));
        }
        let mut u = con_galaxia();
        let t = std::time::Instant::now();
        let inf = armar(&mut u, "pr-1", &nodos);
        let primera = t.elapsed();
        let t = std::time::Instant::now();
        armar(&mut u, "pr-1", &nodos);
        let segunda = t.elapsed();
        println!("armar 600: {primera:?} la primera vez, {segunda:?} la siguiente ({inf:?})");
        assert_eq!(inf.lunas, 600);
        assert_eq!(inf.sistemas, 40);
        // Holgado a proposito (sin optimizar): se vigila que no sean segundos.
        assert!(primera.as_millis() < 1500 && segunda.as_millis() < 300);
    }

    #[test]
    fn un_universo_guardado_con_quitados_y_atados_vuelve_igual_y_uno_viejo_se_lee() {
        let u = armado();
        let mut u = u;
        u.quitados.push("x".into());
        let texto = serde_json::to_string(&u).unwrap();
        let v: Universo = serde_json::from_str(&texto).unwrap();
        assert_eq!(v.quitados, vec!["x".to_string()]);
        assert_eq!(v.astros, u.astros);
        // Caso negativo: un fichero de antes, sin los campos nuevos.
        let viejo: Universo =
            serde_json::from_str(r#"{"version":1,"astros":[{"id":1,"clase":{"tipo":"planeta"}}]}"#)
                .unwrap();
        assert!(viejo.quitados.is_empty());
        assert!(!viejo.astros[0].atado && viejo.astros[0].sistema_de.is_none());
    }
}
