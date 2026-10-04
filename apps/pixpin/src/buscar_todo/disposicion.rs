//! **Donde cae cada cosa** del buscador, sin pintar: el campo, las
//! pestanas, las filas (con la elegida mas alta, por sus botones), las
//! fichas de busquedas recientes, las letras rapidas y el menu.
//!
//! Lo que se pinta y lo que se pulsa salen de aqui, asi no se separan. Los
//! anchos de los textos los mide quien pinta y llegan ya hechos: esto es
//! pura cuenta, y se prueba sin GPU.

use pixpin_render::RectF;

use super::modelo::{Boton, Linea, Pestana};

/// Medidas de la maqueta, en pixeles logicos.
pub const ANCHO: u32 = 820;
pub const ALTO: u32 = 600;
pub const ALTO_CAMPO: f32 = 62.0;
pub const ALTO_PESTANAS: f32 = 44.0;
pub const ALTO_PIE: f32 = 40.0;
pub const PANEL_INICIO: f32 = 300.0;
pub const PANEL: f32 = 290.0;
pub const FILA: f32 = 50.0;
pub const FILA_INICIO: f32 = 46.0;
pub const FILA_LETRA: f32 = 46.0;
pub const CABECERA: f32 = 30.0;
pub const FILA_ELEGIDA: f32 = 52.0;
pub const ALTO_BOTON: f32 = 34.0;
/// Lo que crece la fila elegida por su hilera de botones.
pub const BAJO_BOTONES: f32 = ALTO_BOTON + 12.0;
pub const ALTO_FICHA: f32 = 32.0;
pub const HUECO_FICHA: f32 = 8.0;
pub const ALTO_PISTA: f32 = 64.0;
pub const OPCION_MENU: f32 = 44.0;
/// Lo que sube el aviso de abajo («Copiado», «Captura borrada») sobre el pie.
pub const AVISO_ARRIBA: f32 = 52.0;
/// Lo minimo que mide algo que se pulsa (principio aprobado: >= 40 px).
pub const OBJETIVO: f32 = 40.0;

/// Lo que se puede pulsar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zona {
    /// El ✕ del campo: borrar lo escrito.
    Borrar,
    Pestana(Pestana),
    Boton(Boton),
    /// Una busqueda reciente (su indice) y su ✕.
    Ficha(usize),
    QuitarFicha(usize),
    BorrarTodas,
    Letra(char),
    OpcionMenu(usize),
    Conservar,
    Deshacer,
}

/// Lo que hace falta saber para colocar.
#[derive(Debug, Clone)]
pub struct Entrada<'a> {
    pub ancho: f32,
    pub alto: f32,
    /// Escala de pantalla (1,5 al 150 %).
    pub e: f32,
    /// La caja vacia en «Todo»: recientes a la izquierda y letras a la
    /// derecha.
    pub inicio: bool,
    pub lineas: &'a [Linea],
    /// La fila elegida, como posicion entre las visibles.
    pub elegido: usize,
    /// El ancho de cada pestana (rotulo y cuenta), en el orden de
    /// `Pestana::TODAS`.
    pub anchos_pestanas: &'a [f32],
    /// Los botones de la fila elegida, con su ancho.
    pub botones: &'a [(Boton, f32)],
    /// El ancho de cada ficha de busqueda reciente (con su ✕).
    pub anchos_fichas: &'a [f32],
    pub letras: &'a [char],
    /// Cuantas opciones tiene el menu abierto (0: cerrado).
    pub opciones_menu: usize,
    /// Si la vista previa ensena «Conservar» (una captura que caduca).
    pub conservar: bool,
    /// El aviso «Deshacer» de abajo.
    pub deshacer: bool,
    /// Lo desplazado antes, para no saltar.
    pub scroll: f32,
}

#[derive(Debug, Clone)]
pub struct Disposicion {
    pub campo: RectF,
    pub borrar: Option<RectF>,
    pub barra_pestanas: RectF,
    pub pestanas: Vec<(Pestana, RectF)>,
    /// Donde va la lista (se recorta a esto).
    pub lista: RectF,
    pub panel: RectF,
    pub pie: RectF,
    /// Las lineas que se ven, ya desplazadas: (posicion en `lineas`, caja).
    pub lineas: Vec<(usize, RectF)>,
    /// La caja de la fila elegida entera (con sus botones).
    pub tarjeta: Option<RectF>,
    pub botones: Vec<(Boton, RectF)>,
    pub cabecera_recientes: Option<RectF>,
    pub fichas: Vec<(usize, RectF, RectF)>,
    pub borrar_todas: Option<RectF>,
    pub cabecera_abiertos: Option<RectF>,
    pub pista_pegar: Option<RectF>,
    pub cabecera_letras: Option<RectF>,
    pub letras: Vec<(char, RectF)>,
    pub cabecera_menu: Option<RectF>,
    pub opciones_menu: Vec<(usize, RectF)>,
    pub conservar: Option<RectF>,
    pub deshacer: Option<RectF>,
    pub scroll: f32,
}
const CERO: RectF = RectF { x: 0.0, y: 0.0, ancho: 0.0, alto: 0.0 };impl Default for Disposicion {    fn default() -> Self {        Disposicion {            campo: CERO,            borrar: None,            barra_pestanas: CERO,            pestanas: Vec::new(),            lista: CERO,            panel: CERO,            pie: CERO,            lineas: Vec::new(),            tarjeta: None,            botones: Vec::new(),            cabecera_recientes: None,            fichas: Vec::new(),            borrar_todas: None,            cabecera_abiertos: None,            pista_pegar: None,            cabecera_letras: None,            letras: Vec::new(),            cabecera_menu: None,            opciones_menu: Vec::new(),            conservar: None,            deshacer: None,            scroll: 0.0,        }    }}

fn caja(x: f32, y: f32, ancho: f32, alto: f32) -> RectF {
    RectF { x, y, ancho, alto }
}

/// La caja agrandada (centrada) hasta `minimo` en cada lado, para pulsar.
pub fn agrandada(r: RectF, minimo: f32) -> RectF {
    let ancho = r.ancho.max(minimo);
    let alto = r.alto.max(minimo);
    caja(r.x - (ancho - r.ancho) / 2.0, r.y - (alto - r.alto) / 2.0, ancho, alto)
}

fn dentro(r: RectF, p: (f32, f32)) -> bool {
    p.0 >= r.x && p.0 < r.x + r.ancho && p.1 >= r.y && p.1 < r.y + r.alto
}

/// **Coloca todo.**
pub fn colocar(en: &Entrada) -> Disposicion {
    let e = en.e;
    let mut d = Disposicion::default();
    let (w, h) = (en.ancho, en.alto);
    d.campo = caja(0.0, 0.0, w, ALTO_CAMPO * e);
    if !en.inicio {
        let lado = 32.0 * e;
        d.borrar = Some(caja(w - 18.0 * e - lado, (ALTO_CAMPO * e - lado) / 2.0, lado, lado));
    }
    d.barra_pestanas = caja(0.0, ALTO_CAMPO * e, w, ALTO_PESTANAS * e);
    let mut x = 14.0 * e;
    let alto_pestana = 32.0 * e;
    let y_pestana = d.barra_pestanas.y + (d.barra_pestanas.alto - alto_pestana) / 2.0;
    for (i, p) in Pestana::TODAS.iter().enumerate() {
        let a = en.anchos_pestanas.get(i).copied().unwrap_or(80.0 * e);
        d.pestanas.push((*p, caja(x, y_pestana, a, alto_pestana)));
        x += a + 4.0 * e;
    }
    let arriba = (ALTO_CAMPO + ALTO_PESTANAS) * e;
    d.pie = caja(0.0, h - ALTO_PIE * e, w, ALTO_PIE * e);
    let ancho_panel = if en.inicio { PANEL_INICIO } else { PANEL } * e;
    let alto_cuerpo = (d.pie.y - arriba).max(0.0);
    d.panel = caja(w - ancho_panel, arriba, ancho_panel, alto_cuerpo);
    d.lista = caja(0.0, arriba, (w - ancho_panel).max(0.0), alto_cuerpo);

    if en.inicio {
        colocar_inicio(en, &mut d);
    } else {
        colocar_lista(en, &mut d);
        colocar_panel(en, &mut d);
    }
    if en.deshacer {
        // Pegado a la derecha del aviso de abajo, dentro de el.
        let a = 150.0 * e;
        let fin = d.lista.x + d.lista.ancho - 16.0 * e;
        d.deshacer = Some(caja(fin - a - 4.0 * e, d.pie.y - (AVISO_ARRIBA - 4.0) * e, a, 36.0 * e));
    }
    d
}

/// La caja vacia: fichas de busquedas, abiertos hace poco y la pista de
/// pegar a la izquierda; las letras a la derecha.
fn colocar_inicio(en: &Entrada, d: &mut Disposicion) {
    let e = en.e;
    let x0 = d.lista.x + 8.0 * e;
    let ancho = d.lista.ancho - 16.0 * e;
    let mut y = d.lista.y + 6.0 * e;
    if !en.anchos_fichas.is_empty() {
        let cab = caja(x0, y, ancho, CABECERA * e);
        let a = 96.0 * e;
        d.borrar_todas = Some(caja(cab.x + cab.ancho - a, cab.y, a, cab.alto));
        d.cabecera_recientes = Some(cab);
        y += CABECERA * e;
        let mut x = x0 + 12.0 * e;
        let limite = x0 + ancho - 12.0 * e;
        // Las fichas parten renglon, como el texto; dos renglones como
        // mucho: lo demas no cabe y tampoco hace falta.
        let mut renglon = 0;
        for (i, a) in en.anchos_fichas.iter().enumerate() {
            let a = a.min(limite - x0 - 12.0 * e);
            if x + a > limite && x > x0 + 12.0 * e {
                renglon += 1;
                if renglon >= 2 {
                    break;
                }
                x = x0 + 12.0 * e;
                y += (ALTO_FICHA + HUECO_FICHA) * e;
            }
            let f = caja(x, y, a, ALTO_FICHA * e);
            let lado = 22.0 * e;
            let quitar = caja(f.x + f.ancho - 6.0 * e - lado, f.y + (f.alto - lado) / 2.0, lado, lado);
            d.fichas.push((i, f, quitar));
            x += a + HUECO_FICHA * e;
        }
        y += (ALTO_FICHA + 6.0 + 6.0) * e;
    }
    let pista_y = d.lista.y + d.lista.alto - (ALTO_PISTA + 8.0) * e;
    d.pista_pegar = Some(caja(x0 + 4.0 * e, pista_y, ancho - 8.0 * e, ALTO_PISTA * e));
    if !en.lineas.is_empty() {
        d.cabecera_abiertos = Some(caja(x0, y, ancho, CABECERA * e));
        y += CABECERA * e;
        for (i, _) in en.lineas.iter().enumerate() {
            let alto = FILA_INICIO * e;
            if y + alto > pista_y - 4.0 * e {
                break;
            }
            let r = caja(x0, y, ancho, alto);
            if i == en.elegido {
                d.tarjeta = Some(r);
            }
            d.lineas.push((i, r));
            y += alto;
        }
    }
    // Las letras.
    let px = d.panel.x + 8.0 * e;
    let pancho = d.panel.ancho - 16.0 * e;
    let mut y = d.panel.y + 6.0 * e;
    d.cabecera_letras = Some(caja(px, y, pancho, CABECERA * e));
    y += CABECERA * e;
    for l in en.letras {
        if y + FILA_LETRA * e > d.panel.y + d.panel.alto {
            break;
        }
        d.letras.push((*l, caja(px, y, pancho, FILA_LETRA * e)));
        y += FILA_LETRA * e;
    }
}

/// El alto de una linea (la elegida crece por sus botones).
fn alto_de(en: &Entrada, l: Linea, es_elegida: bool) -> f32 {
    let e = en.e;
    match l {
        Linea::Cabecera(_) => CABECERA * e,
        Linea::Fila(_) if es_elegida && !en.botones.is_empty() => (FILA_ELEGIDA + BAJO_BOTONES) * e,
        Linea::Fila(_) => FILA * e,
    }
}

fn colocar_lista(en: &Entrada, d: &mut Disposicion) {
    let e = en.e;
    let x0 = d.lista.x + 8.0 * e;
    let ancho = d.lista.ancho - 16.0 * e;
    // Que linea es la elegida.
    let mut elegida_linea = None;
    let mut n = 0;
    for (i, l) in en.lineas.iter().enumerate() {
        if let Linea::Fila(_) = l {
            if n == en.elegido {
                elegida_linea = Some(i);
            }
            n += 1;
        }
    }
    // Las posiciones sin desplazar.
    let mut ys = Vec::with_capacity(en.lineas.len());
    let mut y = 4.0 * e;
    for (i, l) in en.lineas.iter().enumerate() {
        let alto = alto_de(en, *l, Some(i) == elegida_linea);
        ys.push((y, alto));
        y += alto;
    }
    let total = y + 4.0 * e;
    let vista = d.lista.alto;
    // El desplazamiento: el de antes, movido lo justo para que la elegida
    // se vea entera (y su cabecera, si es la primera de su grupo).
    let mut scroll = en.scroll.clamp(0.0, (total - vista).max(0.0));
    if let Some(i) = elegida_linea {
        let (ye, alto) = ys[i];
        let arriba = if i > 0 && matches!(en.lineas[i - 1], Linea::Cabecera(_)) { ys[i - 1].0 } else { ye };
        if arriba < scroll {
            scroll = arriba - 4.0 * e;
        }
        if ye + alto > scroll + vista {
            scroll = ye + alto - vista + 4.0 * e;
        }
        scroll = scroll.clamp(0.0, (total - vista).max(0.0));
    }
    d.scroll = scroll;
    for (i, (yl, alto)) in ys.iter().enumerate() {
        let r = caja(x0, d.lista.y + yl - scroll, ancho, *alto);
        if r.y + r.alto < d.lista.y || r.y > d.lista.y + d.lista.alto {
            continue;
        }
        d.lineas.push((i, r));
        if Some(i) == elegida_linea {
            d.tarjeta = Some(r);
            let mut bx = r.x + 12.0 * e;
            let by = r.y + FILA_ELEGIDA * e;
            for (b, a) in en.botones {
                let rb = caja(bx, by, *a, ALTO_BOTON * e);
                d.botones.push((*b, rb));
                bx += a + 8.0 * e;
            }
        }
    }
}

fn colocar_panel(en: &Entrada, d: &mut Disposicion) {
    let e = en.e;
    let px = d.panel.x + 16.0 * e;
    let pancho = d.panel.ancho - 32.0 * e;
    if en.opciones_menu > 0 {
        let mut y = d.panel.y + 10.0 * e;
        d.cabecera_menu = Some(caja(px, y, pancho, CABECERA * e));
        y += 50.0 * e;
        for i in 0..en.opciones_menu {
            if y + OPCION_MENU * e > d.panel.y + d.panel.alto {
                break;
            }
            d.opciones_menu.push((i, caja(px - 8.0 * e, y, pancho + 16.0 * e, OPCION_MENU * e)));
            y += OPCION_MENU * e;
        }
        return;
    }
    if en.conservar {
        // La banda «Caduca en…» la coloca quien pinta (depende de la foto);
        // el boton va siempre a su derecha, a esta altura desde abajo.
        let a = 92.0 * e;
        d.conservar = Some(caja(px + pancho - a - 6.0 * e, d.panel.y + d.panel.alto - 110.0 * e, a, 32.0 * e));
    }
}

impl Disposicion {
    /// **Que hay bajo el raton.** Cada objetivo se agranda hasta 40 px
    /// para pulsarlo; lo de encima (botones, ✕) gana a la fila.
    pub fn zona_en(&self, p: (f32, f32)) -> Option<Zona> {
        if let Some(r) = self.deshacer {
            if dentro(agrandada(r, OBJETIVO), p) {
                return Some(Zona::Deshacer);
            }
        }
        if let Some(r) = self.borrar {
            if dentro(agrandada(r, OBJETIVO), p) {
                return Some(Zona::Borrar);
            }
        }
        for (pe, r) in &self.pestanas {
            if dentro(agrandada(*r, OBJETIVO), p) {
                return Some(Zona::Pestana(*pe));
            }
        }
        for (b, r) in &self.botones {
            if dentro(agrandada(*r, OBJETIVO), p) && dentro(self.lista, p) {
                return Some(Zona::Boton(*b));
            }
        }
        for (i, _, x) in &self.fichas {
            if dentro(agrandada(*x, 28.0), p) {
                return Some(Zona::QuitarFicha(*i));
            }
        }
        for (i, f, _) in &self.fichas {
            if dentro(agrandada(*f, OBJETIVO), p) {
                return Some(Zona::Ficha(*i));
            }
        }
        if let Some(r) = self.borrar_todas {
            if dentro(r, p) {
                return Some(Zona::BorrarTodas);
            }
        }
        for (l, r) in &self.letras {
            if dentro(*r, p) {
                return Some(Zona::Letra(*l));
            }
        }
        for (i, r) in &self.opciones_menu {
            if dentro(*r, p) {
                return Some(Zona::OpcionMenu(*i));
            }
        }
        if let Some(r) = self.conservar {
            if dentro(agrandada(r, OBJETIVO), p) {
                return Some(Zona::Conservar);
            }
        }
        None
    }

    /// La fila bajo el raton, como posicion entre las visibles. Aparte de
    /// [`Disposicion::zona_en`] porque necesita saber que linea es fila.
    pub fn fila_en(&self, lineas: &[Linea], p: (f32, f32)) -> Option<usize> {
        if !dentro(self.lista, p) {
            return None;
        }
        let (i, _) = self.lineas.iter().find(|(_, r)| dentro(*r, p))?;
        if !matches!(lineas.get(*i), Some(Linea::Fila(_))) {
            return None;
        }
        Some(lineas[..*i].iter().filter(|l| matches!(l, Linea::Fila(_))).count())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::buscar_todo::modelo::Grupo;

    fn entrada<'a>(lineas: &'a [Linea], botones: &'a [(Boton, f32)], e: f32) -> Entrada<'a> {
        Entrada {
            ancho: ANCHO as f32 * e,
            alto: ALTO as f32 * e,
            e,
            inicio: false,
            lineas,
            elegido: 0,
            anchos_pestanas: &[60.0, 90.0, 80.0, 100.0, 90.0, 90.0],
            botones,
            anchos_fichas: &[],
            letras: &[],
            opciones_menu: 0,
            conservar: false,
            deshacer: false,
            scroll: 0.0,
        }
    }

    fn muchas(n: usize) -> Vec<Linea> {
        let mut v = vec![Linea::Cabecera(Grupo::Mejor)];
        v.extend((0..n).map(Linea::Fila));
        v
    }

    const BOTONES: [(Boton, f32); 4] =
        [(Boton::Principal, 110.0), (Boton::VerEnChat, 170.0), (Boton::Copiar, 110.0), (Boton::Mas, 34.0)];

    #[test]
    fn filas_de_44_o_mas_y_objetivos_de_40_o_mas_a_cualquier_escala() {
        for e in [1.0, 1.25, 1.5, 2.0] {
            let l = muchas(5);
            let d = colocar(&entrada(&l, &BOTONES, e));
            for (i, r) in &d.lineas {
                if matches!(l[*i], Linea::Fila(_)) {
                    assert!(r.alto >= 44.0 * e, "fila de {} a {e}", r.alto);
                }
            }
            for (b, r) in &d.botones {
                let g = agrandada(*r, OBJETIVO);
                assert!(g.alto >= OBJETIVO && g.ancho >= OBJETIVO, "{b:?}");
            }
            // Las pestanas, igual.
            for (_, r) in &d.pestanas {
                assert!(agrandada(*r, OBJETIVO).alto >= OBJETIVO);
            }
        }
    }

    #[test]
    fn la_elegida_crece_por_sus_botones_y_estos_caen_dentro_de_ella() {
        let l = muchas(3);
        let d = colocar(&entrada(&l, &BOTONES, 1.0));
        let t = d.tarjeta.expect("tarjeta");
        assert_eq!(t.alto, FILA_ELEGIDA + BAJO_BOTONES);
        assert_eq!(d.botones.len(), 4);
        for (_, b) in &d.botones {
            assert!(b.y >= t.y && b.y + b.alto <= t.y + t.alto);
        }
        // El azul el primero, a la izquierda.
        assert_eq!(d.botones[0].0, Boton::Principal);
        assert!(d.botones.windows(2).all(|w| w[0].1.x < w[1].1.x));
        // Caso negativo: sin botones la elegida no crece.
        let d = colocar(&entrada(&l, &[], 1.0));
        assert_eq!(d.tarjeta.unwrap().alto, FILA);
    }

    #[test]
    fn bajar_mucho_desplaza_lo_justo_para_ver_la_elegida() {
        let l = muchas(40);
        let mut en = entrada(&l, &BOTONES, 1.0);
        en.elegido = 30;
        let d = colocar(&en);
        let t = d.tarjeta.expect("la elegida se ve");
        assert!(t.y >= d.lista.y && t.y + t.alto <= d.lista.y + d.lista.alto + 0.5, "{t:?} en {:?}", d.lista);
        assert!(d.scroll > 0.0);
        // Volver arriba vuelve a 0 y ensena la cabecera del grupo.
        en.elegido = 0;
        en.scroll = d.scroll;
        let d = colocar(&en);
        assert_eq!(d.scroll, 0.0);
        assert!(d.lineas.iter().any(|(i, _)| *i == 0), "la cabecera «Mejor resultado» se ve");
    }

    #[test]
    fn el_raton_encuentra_fila_boton_y_pestana() {
        let l = muchas(4);
        let mut en = entrada(&l, &BOTONES, 1.0);
        en.elegido = 1;
        let d = colocar(&en);
        // Un boton de la elegida gana a la fila.
        let (_, b) = d.botones[1];
        assert_eq!(d.zona_en((b.x + 4.0, b.y + 4.0)), Some(Zona::Boton(Boton::VerEnChat)));
        // La fila de abajo.
        let (_, r) = d.lineas.iter().find(|(i, _)| *i == 3).copied().unwrap();
        assert_eq!(d.fila_en(&l, (r.x + 30.0, r.y + 10.0)), Some(2));
        let (_, p) = d.pestanas[2];
        assert_eq!(d.zona_en((p.x + 2.0, p.y + 2.0)), Some(Zona::Pestana(Pestana::Tareas)));
        // Caso negativo: la cabecera no es una fila; el pie no es nada.
        let (_, c) = d.lineas[0];
        assert_eq!(d.fila_en(&l, (c.x + 5.0, c.y + 5.0)), None);
        assert_eq!(d.zona_en((d.pie.x + 300.0, d.pie.y + 20.0)), None);
        assert_eq!(d.fila_en(&l, (d.pie.x + 30.0, d.pie.y + 20.0)), None);
    }

    #[test]
    fn el_inicio_pone_fichas_en_dos_renglones_como_mucho_y_las_letras_a_la_derecha() {
        let l: Vec<Linea> = (0..4).map(Linea::Fila).collect();
        let fichas = [120.0; 12];
        let letras = ['t', 'n', 'l', 'g', 'c', 'u', 'a'];
        let mut en = entrada(&l, &[], 1.0);
        en.inicio = true;
        en.anchos_fichas = &fichas;
        en.letras = &letras;
        let d = colocar(&en);
        let renglones: std::collections::BTreeSet<i32> = d.fichas.iter().map(|(_, f, _)| f.y as i32).collect();
        assert_eq!(renglones.len(), 2, "dos renglones");
        assert!(d.fichas.len() < 12, "lo que no cabe no se pinta");
        assert_eq!(d.letras.len(), 7, "caben las siete letras");
        for (_, r) in &d.letras {
            assert!(r.x >= d.panel.x && r.alto >= 44.0);
        }
        // Las filas de abiertos no pisan la pista de pegar.
        let pista = d.pista_pegar.unwrap();
        for (_, r) in &d.lineas {
            assert!(r.y + r.alto <= pista.y);
        }
        // El ✕ de una ficha gana a la ficha.
        let (_, f, x) = d.fichas[0];
        assert_eq!(d.zona_en((x.x + x.ancho / 2.0, x.y + x.alto / 2.0)), Some(Zona::QuitarFicha(0)));
        assert_eq!(d.zona_en((f.x + 8.0, f.y + 8.0)), Some(Zona::Ficha(0)));
        assert_eq!(d.zona_en((d.letras[0].1.x + 50.0, d.letras[0].1.y + 10.0)), Some(Zona::Letra('t')));
        // Caso negativo: sin busquedas recientes no hay cabecera ni «Borrar
        // todas», y en el inicio no hay ✕ de borrar lo escrito.
        en.anchos_fichas = &[];
        let d = colocar(&en);
        assert!(d.cabecera_recientes.is_none() && d.borrar_todas.is_none() && d.borrar.is_none());
    }
}
