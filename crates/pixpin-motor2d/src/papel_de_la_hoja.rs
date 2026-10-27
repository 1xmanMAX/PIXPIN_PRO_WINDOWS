//! **Una hoja lleva su propio papel** (27-oct-2026, puerto de
//! `PapelDeLaHoja.kt` del movil, dd1af9e2). El usuario: «el modo noche de
//! algunas figuras es feo: el blanco simplemente se transforma en negro y el
//! negro en blanco; arregla los disenos».
//!
//! El lienzo adapta cada tinta **contra el papel del lienzo**
//! ([`crate::contraste::adaptar`], `dibujo::tema` en la app), y eso esta bien
//! para lo que se dibuja suelto encima del papel. Pero una plantilla (un
//! planificador, un diario, un bloc) trae **su propia pagina**: el
//! rectangulo relleno de abajo que lo encierra todo. Contra la pizarra, la
//! pagina blanca se quedaba blanca (ya se lee) pero su tinta oscura se daba
//! la vuelta a clara: letras y rayas claras sobre la pagina blanca, o sea
//! casi nada. Lo mismo dentro de un simbolo: la tinta grafito, blanca dentro
//! de su cuerpo blanco. Y la gemela «(noche)» de una plantilla
//! (`figuras_de_serie::noche`, con su paleta hecha a mano) era peor: su papel
//! azul noche no llega a 3:1 contra la pizarra, asi que se **aclaraba**.
//!
//! La regla de ahora, la de `noche.rs` («la hoja lleva su propio rectangulo
//! de papel: lo que pongan detras no se nota»):
//!
//! - **Las piezas de una hoja se pintan como se disenaron**, sin adaptar: la
//!   pagina, su tinta, sus rayas y sus acentos. Nada se invierte.
//! - **Lo que se dibuja encima de una hoja se adapta a la hoja**, no al
//!   lienzo: el rotulador blanco del lienzo de noche sale oscuro sobre la
//!   pagina blanca de una plantilla de dia, y se lee.
//! - **Dentro de una figura, cada pieza se adapta a lo que tiene debajo en la
//!   propia figura.** La pata y el contorno de un simbolo, que van sobre el
//!   lienzo, se aclaran; la onda de dentro, sobre el cuerpo blanco, se queda
//!   oscura. Con un tope de piezas: cuesta el cuadrado.
//! - Lo demas, como siempre: contra el papel del lienzo.
//!
//! Una hoja es un **grupo** (el exterior, el que pone `biblioteca::estampar`)
//! cuya pieza de mas abajo es un rectangulo de relleno liso y opaco que tapa
//! la caja de todo el grupo. Sin grupo no hay hoja: un rectangulo relleno
//! suelto es dibujo, y se sigue adaptando como dibujo. Es mas estricto que
//! `hoja_de_plantilla` a proposito: esto se mira mientras se pinta y tiene
//! que costar una pasada.
//!
//! Es pintura, no dibujo, como todo el tema: los colores guardados no
//! cambian, y nada de esto se guarda ni viaja.

use std::collections::{HashMap, HashSet};

use crate::biblioteca::caja_de;
use crate::elemento::{ColorRgba, Elemento, Figura};
use crate::relleno::EstiloRelleno;

/// Lo que la pagina puede quedarse corta de la caja del grupo (su propio
/// trazo, redondeos).
const HOLGURA: f32 = 2.0;

/// Las figuras mas grandes no se miran pieza contra pieza: cuesta el
/// cuadrado de las piezas. Los simbolos tienen decenas; una tabla enorme
/// agrupada se queda como antes.
pub const PIEZAS_MAXIMAS_DE_UNA_FIGURA: usize = 400;

/// **Contra que se pinta la tinta de una pieza.**
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PapelDeLaPieza {
    /// Contra el papel del lienzo, como siempre.
    Lienzo,
    /// Es de una hoja: tal cual se diseno.
    ComoSeDiseno,
    /// Va encima de una hoja (o de un relleno de su figura): contra ese papel.
    Papel(ColorRgba),
}

/// **Como se pinta la tinta de cada pieza.** Las que no salen aqui, contra
/// el papel del lienzo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mapa {
    /// Las piezas de una hoja: tal cual.
    pub como_se_diseno: HashSet<u64>,
    /// Lo de encima de una hoja: contra el papel de esa hoja (opaco).
    pub papel_de: HashMap<u64, ColorRgba>,
}

impl Mapa {
    /// Si no cambia nada (lo normal: un dibujo sin grupos).
    pub fn esta_vacio(&self) -> bool {
        self.como_se_diseno.is_empty() && self.papel_de.is_empty()
    }

    /// Contra que se pinta la pieza `id`.
    pub fn de_la_pieza(&self, id: u64) -> PapelDeLaPieza {
        if self.como_se_diseno.contains(&id) {
            PapelDeLaPieza::ComoSeDiseno
        } else if let Some(p) = self.papel_de.get(&id) {
            PapelDeLaPieza::Papel(*p)
        } else {
            PapelDeLaPieza::Lienzo
        }
    }

    /// **La tinta `c` de la pieza `id` tal como se pinta** sobre el lienzo
    /// de papel `papel_del_lienzo`: tal cual si es de una hoja, adaptada a la
    /// hoja si va encima de una, y si no al lienzo (`None`: lo que no es una
    /// pieza, como la rejilla).
    pub fn tinta(&self, id: Option<u64>, c: ColorRgba, papel_del_lienzo: ColorRgba) -> ColorRgba {
        match id.map_or(PapelDeLaPieza::Lienzo, |id| self.de_la_pieza(id)) {
            PapelDeLaPieza::ComoSeDiseno => c,
            PapelDeLaPieza::Papel(p) => crate::contraste::adaptar(c, p),
            PapelDeLaPieza::Lienzo => crate::contraste::adaptar(c, papel_del_lienzo),
        }
    }
}

/// Una hoja encontrada: su posicion en el monton, su caja y su papel.
struct Hoja {
    indice: usize,
    caja: (f32, f32, f32, f32),
    papel: ColorRgba,
}

/// Las formas cuyo relleno hace de papel a lo que se pinta encima.
fn con_relleno(e: &Elemento) -> bool {
    matches!(
        e.figura,
        Figura::Rectangulo | Figura::Elipse | Figura::Rombo | Figura::Region { .. }
    )
}

/// El color de un relleno si es liso y opaco, o `None` (sin relleno,
/// rayado, translucido).
pub fn relleno_opaco(e: &Elemento) -> Option<ColorRgba> {
    let c = e.relleno?;
    (e.estilo_relleno == EstiloRelleno::Solido && e.opacidad >= 0.9 && c.a >= 0.995)
        .then_some(ColorRgba { a: 1.0, ..c })
}

fn centro(c: (f32, f32, f32, f32)) -> (f32, f32) {
    ((c.0 + c.2) / 2.0, (c.1 + c.3) / 2.0)
}

/// Si el punto `(x, y)` cae dentro del relleno de `e`, de caja `c`.
fn cubre(e: &Elemento, c: (f32, f32, f32, f32), x: f32, y: f32) -> bool {
    if x < c.0 || x > c.2 || y < c.1 || y > c.3 {
        return false;
    }
    let (rx, ry) = ((c.2 - c.0) / 2.0, (c.3 - c.1) / 2.0);
    if rx <= 0.0 || ry <= 0.0 {
        return false;
    }
    let (mx, my) = centro(c);
    let (dx, dy) = ((x - mx) / rx, (y - my) / ry);
    match e.figura {
        Figura::Elipse => dx * dx + dy * dy <= 1.0,
        Figura::Rombo => dx.abs() + dy.abs() <= 1.0,
        _ => true,
    }
}

/// **El mapa de un dibujo**, en el orden del monton (de abajo arriba). Un
/// dibujo sin grupos (lo normal) sale enseguida y vacio.
pub fn de(elementos: &[Elemento]) -> Mapa {
    if elementos.iter().all(|e| e.grupos.is_empty()) {
        return Mapa::default();
    }
    // Por grupo exterior, en el orden del monton.
    let mut grupos: Vec<(&str, Vec<(usize, &Elemento)>)> = Vec::new();
    let mut donde: HashMap<&str, usize> = HashMap::new();
    for (i, e) in elementos.iter().enumerate() {
        if e.borrado || matches!(e.figura, Figura::Marco { .. }) {
            continue;
        }
        let Some(g) = e.grupos.last() else { continue };
        let k = *donde.entry(g.as_str()).or_insert_with(|| {
            grupos.push((g.as_str(), Vec::new()));
            grupos.len() - 1
        });
        grupos[k].1.push((i, e));
    }
    let mut mapa = Mapa::default();
    let mut hojas: Vec<Hoja> = Vec::new();
    for (_, piezas) in &grupos {
        if piezas.len() < 2 {
            continue;
        }
        if let Some(h) = hoja_de(piezas) {
            mapa.como_se_diseno.extend(piezas.iter().map(|(_, e)| e.id));
            hojas.push(h);
        } else if piezas.len() <= PIEZAS_MAXIMAS_DE_UNA_FIGURA {
            papeles_dentro_de(piezas, &mut mapa.papel_de);
        }
    }
    if hojas.is_empty() {
        return mapa;
    }
    for (i, e) in elementos.iter().enumerate() {
        // Lo que ya tiene papel dentro de su figura se queda con el: la onda
        // de un simbolo puesto sobre una hoja va sobre el cuerpo del simbolo.
        if e.borrado || mapa.como_se_diseno.contains(&e.id) || mapa.papel_de.contains_key(&e.id) {
            continue;
        }
        let (mx, my) = centro(e.caja());
        // La de mas arriba que tenga debajo su centro.
        let Some(h) = hojas.iter().rev().find(|h| {
            h.indice < i && mx >= h.caja.0 && mx <= h.caja.2 && my >= h.caja.1 && my <= h.caja.3
        }) else {
            continue;
        };
        mapa.papel_de.insert(e.id, h.papel);
    }
    mapa
}

/// La hoja de un grupo: su pieza de mas abajo, si es una pagina opaca que
/// lo tapa todo.
fn hoja_de(piezas: &[(usize, &Elemento)]) -> Option<Hoja> {
    let (indice, pagina) = *piezas.first()?;
    if !matches!(pagina.figura, Figura::Rectangulo) {
        return None;
    }
    let papel = relleno_opaco(pagina)?;
    let caja = pagina.caja();
    let todas: Vec<Elemento> = piezas.iter().map(|(_, e)| (*e).clone()).collect();
    let todo = caja_de(&todas)?;
    if todo.0 < caja.0 - HOLGURA
        || todo.1 < caja.1 - HOLGURA
        || todo.2 > caja.2 + HOLGURA
        || todo.3 > caja.3 + HOLGURA
    {
        return None;
    }
    Some(Hoja {
        indice,
        caja,
        papel,
    })
}

/// Un relleno opaco de una figura: su posicion en el monton, la pieza, su
/// caja y su color.
type Relleno<'a> = (usize, &'a Elemento, (f32, f32, f32, f32), ColorRgba);

/// **Dentro de una figura que no es hoja**: cada pieza, contra el relleno
/// opaco de la propia figura que tiene debajo de su centro (el de mas
/// arriba de los de debajo). Las que no tienen ninguno debajo no salen: van
/// contra el lienzo.
fn papeles_dentro_de(piezas: &[(usize, &Elemento)], papel_de: &mut HashMap<u64, ColorRgba>) {
    let rellenos: Vec<Relleno<'_>> = piezas
        .iter()
        .filter(|(_, e)| con_relleno(e))
        .filter_map(|(i, e)| Some((*i, *e, e.caja(), relleno_opaco(e)?)))
        .collect();
    if rellenos.is_empty() {
        return;
    }
    for (i, e) in piezas {
        let (mx, my) = centro(e.caja());
        if let Some((_, _, _, papel)) = rellenos
            .iter()
            .rev()
            .find(|(j, r, c, _)| j < i && cubre(r, *c, mx, my))
        {
            papel_de.insert(e.id, *papel);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::biblioteca::{FiguraGuardada, estampar};
    use crate::contraste::{adaptar, contraste_entre};
    use crate::figuras_de_serie::catalogo;
    use crate::vector::Punto2;

    fn hex(rgb: u32) -> ColorRgba {
        ColorRgba::opaco(
            ((rgb >> 16) & 0xff) as f32 / 255.0,
            ((rgb >> 8) & 0xff) as f32 / 255.0,
            (rgb & 0xff) as f32 / 255.0,
        )
    }

    const PIZARRA: u32 = 0x121212;

    fn de_serie(id: &str) -> FiguraGuardada {
        catalogo()
            .into_iter()
            .find(|f| f.id == id)
            .unwrap_or_else(|| panic!("{id}"))
    }

    fn estampada(id: &str) -> Vec<Elemento> {
        let f = de_serie(id);
        let (w, h) = f.tamano();
        estampar(&f, Punto2::nuevo(w / 2.0, h / 2.0), 1, "g1")
    }

    fn trazo(id: u64, x: f32, y: f32, color: u32) -> Elemento {
        Elemento {
            id,
            figura: Figura::Rectangulo,
            x: x - 20.0,
            y: y - 5.0,
            ancho: 40.0,
            alto: 10.0,
            trazo: hex(color),
            relleno: None,
            ..Default::default()
        }
    }

    fn pagina(id: u64) -> Elemento {
        Elemento {
            id,
            figura: Figura::Rectangulo,
            ancho: 100.0,
            alto: 100.0,
            relleno: Some(hex(0xffffff)),
            estilo_relleno: EstiloRelleno::Solido,
            ..Default::default()
        }
    }

    #[test]
    fn las_piezas_de_una_plantilla_estampada_se_pintan_como_se_disenaron() {
        let hoja = estampada("serie-diario-noche");
        let mapa = de(&hoja);
        let ids: HashSet<u64> = hoja.iter().filter(|e| !e.borrado).map(|e| e.id).collect();
        assert_eq!(mapa.como_se_diseno, ids);
        // Su papel azul noche, sobre la pizarra: igual que se diseno (antes
        // se aclaraba).
        let papel = hex(0x1b2030);
        assert_eq!(mapa.tinta(Some(hoja[0].id), papel, hex(PIZARRA)), papel);
        assert_ne!(
            adaptar(papel, hex(PIZARRA)),
            papel,
            "sin la hoja se aclaraba"
        );
    }

    #[test]
    fn una_hoja_de_dia_en_el_lienzo_de_noche_no_sale_en_negativo() {
        let hoja = estampada("serie-diario");
        let mapa = de(&hoja);
        let blanco = hex(0xffffff);
        assert_eq!(relleno_opaco(&hoja[0]), Some(blanco), "la pagina es blanca");
        // La tinta oscura de la hoja se queda oscura sobre su pagina blanca
        // (antes se aclaraba contra la pizarra).
        let tinta = hex(0x22304a);
        assert_eq!(mapa.tinta(Some(hoja[0].id), tinta, hex(PIZARRA)), tinta);
        assert_ne!(adaptar(tinta, hex(PIZARRA)), tinta);
        // Cada tinta de la hoja, la suya: ninguna se da la vuelta.
        for e in &hoja {
            assert_eq!(mapa.tinta(Some(e.id), e.trazo, hex(PIZARRA)), e.trazo);
        }
    }

    #[test]
    fn lo_escrito_encima_se_adapta_a_la_hoja_y_lo_de_fuera_al_lienzo() {
        let mut v = estampada("serie-diario");
        let c = v[0].caja();
        let (mx, my) = centro(c);
        v.push(trazo(9000, mx, my, 0xffffff));
        v.push(trazo(9001, c.2 + 500.0, c.3 + 500.0, 0x000000));
        let mapa = de(&v);
        assert_eq!(mapa.de_la_pieza(9000), PapelDeLaPieza::Papel(hex(0xffffff)));
        assert_eq!(mapa.de_la_pieza(9001), PapelDeLaPieza::Lienzo);
        // El rotulador blanco del lienzo de noche, sobre la pagina blanca:
        // oscuro, se lee.
        let encima = mapa.tinta(Some(9000), hex(0xffffff), hex(PIZARRA));
        assert!(contraste_entre(encima, hex(0xffffff)) >= 4.5, "{encima:?}");
        // Fuera de la hoja, como siempre.
        assert_eq!(
            mapa.tinta(Some(9001), hex(0x000000), hex(PIZARRA)),
            adaptar(hex(0x000000), hex(PIZARRA))
        );
    }

    #[test]
    fn lo_que_queda_debajo_de_la_hoja_no_va_sobre_ella() {
        let hoja = estampada("serie-diario");
        let (mx, my) = centro(hoja[0].caja());
        let mut v = vec![trazo(9000, mx, my, 0x000000)];
        v.extend(hoja);
        assert_eq!(de(&v).de_la_pieza(9000), PapelDeLaPieza::Lienzo);
    }

    #[test]
    fn sin_grupo_o_sin_pagina_opaca_no_hay_hoja() {
        let rect = pagina(1);
        let t = trazo(2, 50.0, 50.0, 0xffffff);
        assert!(de(&[rect.clone(), t.clone()]).esta_vacio(), "sin grupo");
        let g = |mut e: Elemento| {
            e.grupos = vec!["g".into()];
            e
        };
        let translucida = Elemento {
            opacidad: 0.5,
            ..g(rect.clone())
        };
        assert!(de(&[translucida, g(t.clone())]).como_se_diseno.is_empty());
        let sin_relleno = Elemento {
            relleno: None,
            ..g(rect.clone())
        };
        assert!(de(&[sin_relleno, g(t.clone())]).como_se_diseno.is_empty());
        let rayada = Elemento {
            estilo_relleno: EstiloRelleno::Rayado,
            ..g(rect.clone())
        };
        assert!(de(&[rayada, g(t.clone())]).como_se_diseno.is_empty());
        let fuera = trazo(2, 350.0, 50.0, 0xffffff);
        assert!(
            de(&[g(rect.clone()), g(fuera)]).como_se_diseno.is_empty(),
            "no lo tapa todo"
        );
        assert_eq!(de(&[g(rect), g(t)]).como_se_diseno, HashSet::from([1, 2]));
    }

    #[test]
    fn en_un_simbolo_lo_de_dentro_del_cuerpo_blanco_no_se_vuelve_blanco() {
        let simbolo = estampada("serie-elec-fuente-corriente-ca");
        let mapa = de(&simbolo);
        assert!(mapa.como_se_diseno.is_empty(), "no es una hoja");
        let cuerpo = simbolo
            .iter()
            .find(|e| matches!(e.figura, Figura::Elipse))
            .expect("el cuerpo");
        let grafito = hex(0x2b2f3a);
        // El cuerpo va sobre el lienzo: se aclara.
        assert_eq!(mapa.de_la_pieza(cuerpo.id), PapelDeLaPieza::Lienzo);
        assert_ne!(mapa.tinta(Some(cuerpo.id), grafito, hex(PIZARRA)), grafito);
        // Lo de dentro del cuerpo (la onda), contra su blanco: se queda oscuro.
        let blanco = relleno_opaco(cuerpo).expect("cuerpo relleno");
        let dentro: Vec<&Elemento> = simbolo
            .iter()
            .filter(|e| mapa.de_la_pieza(e.id) == PapelDeLaPieza::Papel(blanco))
            .collect();
        assert!(!dentro.is_empty());
        for e in dentro {
            assert_eq!(mapa.tinta(Some(e.id), grafito, hex(PIZARRA)), grafito);
        }
    }

    #[test]
    fn fuera_de_una_pieza_la_tinta_va_contra_el_lienzo() {
        let negro = hex(0x000000);
        assert_eq!(
            Mapa::default().tinta(None, negro, hex(PIZARRA)),
            adaptar(negro, hex(PIZARRA))
        );
        assert!(de(&[trazo(1, 0.0, 0.0, 0)]).esta_vacio());
    }
}
