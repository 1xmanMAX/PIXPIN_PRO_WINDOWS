//! Que se ve y como (D218, D238).

use std::collections::{HashMap, HashSet};

use pixpin_motor2d::Camara;

use crate::astro::{Astro, Clase, IdAstro};
use crate::detalle::{self, Nivel};
use crate::universo::Universo;

pub const CELDA: f32 = 1000.0;
pub const TOPE_DETALLE: usize = 600;

fn celda(v: f32) -> i32 {
    (v / CELDA).floor() as i32
}

fn se_cruzan(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    a.0 <= b.2 && a.2 >= b.0 && a.1 <= b.3 && a.3 >= b.1
}

/// Rejilla propia: la del motor esta atada a `Escena`. Se reconstruye entera,
/// pero solo cuando cambia el universo (`Universo::cambios`), no en cada
/// fotograma: miles de astros son un milisegundo.
///
/// Las lunas van en su propia rejilla: son casi todos los astros, y desde
/// lejos no se ve ninguna. Asi el cosmos entero cuesta lo que sus galaxias y
/// no lo que sus cinco mil archivos (D239).
#[derive(Debug, Default)]
pub struct RejillaAstros {
    celdas: HashMap<(i32, i32), Vec<IdAstro>>,
    lunas: HashMap<(i32, i32), Vec<IdAstro>>,
    /// La luna mas grande, para saber desde que zoom no se ve ninguna.
    radio_luna: f32,
    hecha_con: Option<u64>,
}

impl RejillaAstros {
    pub fn al_dia(&mut self, u: &Universo) {
        if self.hecha_con == Some(u.cambios()) {
            return;
        }
        self.celdas.clear();
        self.lunas.clear();
        self.radio_luna = 0.0;
        for a in &u.astros {
            let es_luna = matches!(a.clase, Clase::Luna { .. });
            if es_luna {
                self.radio_luna = self.radio_luna.max(a.radio);
            }
            let destino = if es_luna {
                &mut self.lunas
            } else {
                &mut self.celdas
            };
            let (x0, y0, x1, y1) = a.caja();
            for cx in celda(x0)..=celda(x1) {
                for cy in celda(y0)..=celda(y1) {
                    destino.entry((cx, cy)).or_default().push(a.id);
                }
            }
        }
        self.hecha_con = Some(u.cambios());
    }

    /// Pueden sobrar, nunca faltar.
    pub fn candidatos(&self, caja: (f32, f32, f32, f32)) -> Vec<IdAstro> {
        self.candidatos_con(caja, true)
    }

    fn candidatos_con(&self, caja: (f32, f32, f32, f32), con_lunas: bool) -> Vec<IdAstro> {
        let mut vistos = HashSet::new();
        let mapas: &[&HashMap<(i32, i32), Vec<IdAstro>>] = if con_lunas {
            &[&self.celdas, &self.lunas]
        } else {
            &[&self.celdas]
        };
        for mapa in mapas {
            // Menos celdas ocupadas que celdas en la ventana (el cosmos
            // entero son miles): entonces se recorren las ocupadas.
            let (cx0, cx1, cy0, cy1) = (celda(caja.0), celda(caja.2), celda(caja.1), celda(caja.3));
            let en_ventana = (cx1 - cx0 + 1) as i64 * (cy1 - cy0 + 1) as i64;
            if (mapa.len() as i64) < en_ventana {
                for ((cx, cy), ids) in mapa.iter() {
                    if (cx0..=cx1).contains(cx) && (cy0..=cy1).contains(cy) {
                        vistos.extend(ids.iter().copied());
                    }
                }
            } else {
                for cx in cx0..=cx1 {
                    for cy in cy0..=cy1 {
                        if let Some(ids) = mapa.get(&(cx, cy)) {
                            vistos.extend(ids.iter().copied());
                        }
                    }
                }
            }
        }
        let mut v: Vec<IdAstro> = vistos.into_iter().collect();
        v.sort();
        v
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Visto {
    pub id: IdAstro,
    pub nivel: Nivel,
    pub radio_px: f32,
}

fn orden_de(a: &Astro) -> u8 {
    match a.clase {
        Clase::Galaxia { .. } => 0,
        Clase::Planeta => 1,
        Clase::Luna { .. } => 2,
    }
}

/// Si la cadena de padres de `id` esta abierta a este zoom, sin memoria.
/// Para padres que no estan en la ventana.
fn cadena_abierta(u: &Universo, id: IdAstro, zoom: f32) -> bool {
    let Some(a) = u.astro(id) else { return true };
    let n = detalle::nivel(detalle::tipo_de(a), a.radio * zoom);
    detalle::abre_hijos(a, n) && a.padre.is_none_or(|p| cadena_abierta(u, p, zoom))
}

/// Lo que se pinta este fotograma, en orden de pintado: galaxias, planetas,
/// lunas. `memoria` guarda el nivel de cada uno para la histeresis.
pub fn visibles(
    u: &Universo,
    rejilla: &RejillaAstros,
    camara: &Camara,
    ancho_px: f32,
    alto_px: f32,
    memoria: &mut HashMap<IdAstro, Nivel>,
) -> Vec<Visto> {
    let ventana = camara.ventana(ancho_px, alto_px);
    // Una luna mas pequena que esto sale oculta seguro: ni se miran.
    let con_lunas =
        rejilla.radio_luna * camara.zoom >= detalle::radio_siempre_oculto(detalle::Tipo::Luna);
    let mut cands: Vec<&Astro> = rejilla
        .candidatos_con(ventana, con_lunas)
        .into_iter()
        .filter_map(|id| u.astro(id))
        .filter(|a| se_cruzan(a.caja(), ventana))
        .collect();
    cands.sort_by_key(|a| (orden_de(a), a.id));

    let mut nivel_de: HashMap<IdAstro, Nivel> = HashMap::with_capacity(cands.len());
    let mut cerrados: HashSet<IdAstro> = HashSet::new();
    let mut salida = Vec::with_capacity(cands.len());
    for a in cands {
        let padre_abierto = match a.padre {
            None => true,
            Some(p) if cerrados.contains(&p) => false,
            Some(p) => match (u.astro(p), nivel_de.get(&p)) {
                (Some(pa), Some(np)) => detalle::abre_hijos(pa, *np),
                (Some(_), None) => cadena_abierta(u, p, camara.zoom),
                (None, _) => true,
            },
        };
        // Con el padre cerrado no hace falta ni su nivel: no se pinta, y su
        // memoria se iba a borrar al final igual. Solo un contenedor puede
        // ser padre de otro, asi que solo esos se apuntan como cerrados.
        if !padre_abierto {
            if a.es_contenedor() {
                cerrados.insert(a.id);
            }
            continue;
        }
        let r = a.radio * camara.zoom;
        let n = detalle::nivel_con_memoria(detalle::tipo_de(a), r, memoria.get(&a.id).copied());
        memoria.insert(a.id, n);
        if n == Nivel::Oculto {
            if a.es_contenedor() {
                cerrados.insert(a.id);
            }
            continue;
        }
        nivel_de.insert(a.id, n);
        salida.push(Visto {
            id: a.id,
            nivel: n,
            radio_px: r,
        });
    }

    // D238.3: las mas pequenas bajan a icono.
    let mut detalladas: Vec<usize> = (0..salida.len())
        .filter(|i| salida[*i].nivel >= Nivel::Ficha)
        .collect();
    if detalladas.len() > TOPE_DETALLE {
        detalladas.sort_by(|a, b| salida[*b].radio_px.total_cmp(&salida[*a].radio_px));
        for i in &detalladas[TOPE_DETALLE..] {
            salida[*i].nivel = Nivel::Icono;
        }
    }
    memoria.retain(|id, _| nivel_de.contains_key(id));
    salida
}

/// El astro visible que representa a `id`: el mismo, o su antepasado mas
/// cercano que se vea. Asi una linea a una luna oculta sale de su galaxia
/// (D207).
fn representante(u: &Universo, vistos: &HashSet<IdAstro>, id: IdAstro) -> Option<IdAstro> {
    let mut actual = Some(id);
    while let Some(i) = actual {
        if vistos.contains(&i) {
            return Some(i);
        }
        actual = u.astro(i).and_then(|a| a.padre);
    }
    None
}

/// `(id de la conexion, desde, hasta)` ya con representantes. Las que caen
/// dentro de un mismo astro no se pintan.
pub fn conexiones_visibles(u: &Universo, vistos: &[Visto]) -> Vec<(u64, IdAstro, IdAstro)> {
    let set: HashSet<IdAstro> = vistos.iter().map(|v| v.id).collect();
    u.conexiones
        .iter()
        .filter_map(|c| {
            let a = representante(u, &set, c.desde)?;
            let b = representante(u, &set, c.hasta)?;
            (a != b).then_some((c.id, a, b))
        })
        .collect()
}

/// Los puntos donde la linea toca el borde de cada circulo.
pub fn extremos(a: &Astro, b: &Astro) -> ((f32, f32), (f32, f32)) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let d = (dx * dx + dy * dy).sqrt();
    if d == 0.0 || d <= a.radio + b.radio {
        return ((a.x, a.y), (b.x, b.y));
    }
    let (ux, uy) = (dx / d, dy / d);
    (
        (a.x + ux * a.radio, a.y + uy * a.radio),
        (b.x - ux * b.radio, b.y - uy * b.radio),
    )
}

/// El astro visible mas pequeno que contiene el punto: el que se pulsa.
pub fn astro_en(u: &Universo, vistos: &[Visto], x: f32, y: f32) -> Option<IdAstro> {
    vistos
        .iter()
        .filter_map(|v| u.astro(v.id))
        .filter(|a| a.contiene(x, y))
        .min_by(|a, b| a.radio.total_cmp(&b.radio))
        .map(|a| a.id)
}

/// La conexion visible que pasa por el punto, dentro de `tolerancia` (en
/// mundo): la que se pulsa para elegirla y poder borrarla.
///
/// Gana la mas cercana, no la primera: dos lineas que se cruzan comparten
/// ese punto y hay que quedarse con la que el usuario esta senalando.
pub fn conexion_en(u: &Universo, vistos: &[Visto], x: f32, y: f32, tolerancia: f32) -> Option<u64> {
    let mut mejor: Option<(u64, f32)> = None;
    for (id, da, db) in conexiones_visibles(u, vistos) {
        let (Some(a), Some(b)) = (u.astro(da), u.astro(db)) else {
            continue;
        };
        let ((x0, y0), (x1, y1)) = extremos(a, b);
        let d = distancia_a_segmento(x, y, x0, y0, x1, y1);
        if d <= tolerancia && mejor.is_none_or(|(_, m)| d < m) {
            mejor = Some((id, d));
        }
    }
    mejor.map(|(id, _)| id)
}

/// Del punto al segmento, no a la recta: fuera de los extremos vale la
/// distancia al extremo, que es lo que se ve en pantalla.
fn distancia_a_segmento(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> f32 {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let largo2 = dx * dx + dy * dy;
    let t = if largo2 == 0.0 {
        0.0
    } else {
        (((x - x0) * dx + (y - y0) * dy) / largo2).clamp(0.0, 1.0)
    };
    let (px, py) = (x0 + t * dx, y0 + t * dy);
    ((x - px) * (x - px) + (y - py) * (y - py)).sqrt()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::astro::{Astro, Conexion, IdAstro};
    use pixpin_motor2d::Camara;

    fn galaxia_con_lunas(n: usize) -> Universo {
        let mut u = Universo::nuevo();
        let g = u.nuevo_id();
        u.astros.push(Astro::galaxia(g, "p", 0.0, 0.0));
        for i in 0..n {
            let id = u.nuevo_id();
            let mut l = Astro::luna(
                id,
                &format!("m:{i}"),
                "p",
                (i % 70) as f32 * 20.0 - 700.0,
                (i / 70) as f32 * 20.0 - 700.0,
            );
            l.padre = Some(g);
            u.astros.push(l);
        }
        u.marcar_cambio();
        u
    }

    fn camara(zoom: f32) -> Camara {
        // Centrada en el origen en una pantalla de 1920 x 1080.
        Camara {
            x: -960.0 / zoom,
            y: -540.0 / zoom,
            zoom,
        }
    }

    fn ver(u: &Universo, zoom: f32) -> Vec<Visto> {
        let mut r = RejillaAstros::default();
        r.al_dia(u);
        visibles(
            u,
            &r,
            &camara(zoom),
            1920.0,
            1080.0,
            &mut Default::default(),
        )
    }

    #[test]
    fn una_galaxia_lejana_es_un_disco_y_no_ensena_sus_lunas() {
        let u = galaxia_con_lunas(100);
        let v = ver(&u, 0.03); // radio 60 px: Disco
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].nivel, Nivel::Disco);
    }

    #[test]
    fn al_abrirse_la_galaxia_aparecen_sus_lunas() {
        let u = galaxia_con_lunas(100);
        let v = ver(&u, 0.5); // radio de galaxia 1000 px, de luna 24 px
        assert!(v.iter().any(|x| x.nivel == Nivel::Icono));
        assert!(v.len() > 1);
    }

    #[test]
    fn desde_lejos_las_lunas_ni_se_miran_pero_la_rejilla_las_sigue_teniendo() {
        let u = galaxia_con_lunas(100);
        let mut r = RejillaAstros::default();
        r.al_dia(&u);
        let caja = camara(0.03).ventana(1920.0, 1080.0);
        // A este zoom una luna mediria 1,4 px: ni se piden.
        assert_eq!(r.candidatos_con(caja, false).len(), 1, "solo la galaxia");
        // Caso negativo: quien pide candidatos sin mas las sigue teniendo.
        assert_eq!(r.candidatos(caja).len(), 101);
    }

    #[test]
    fn una_exoluna_huerfana_de_padre_se_ve_de_cerca_como_antes() {
        // Una luna sin padre (su planeta se borro a mano) no depende de
        // ningun contenedor: de cerca sale, de lejos no.
        let mut u = Universo::nuevo();
        let l = u.nuevo_id();
        u.astros.push(Astro::luna(l, "m:a", "q", 0.0, 0.0));
        u.marcar_cambio();
        assert!(ver(&u, 0.5).iter().any(|x| x.id == l));
        assert!(ver(&u, 0.03).iter().all(|x| x.id != l));
    }

    #[test]
    fn nunca_salen_mas_de_seiscientas_lunas_con_ficha_o_vista() {
        let u = galaxia_con_lunas(5000);
        let v = ver(&u, 2.0); // lunas de 96 px: Ficha
        let detalladas = v.iter().filter(|x| x.nivel >= Nivel::Ficha).count();
        assert!(detalladas <= TOPE_DETALLE, "{detalladas}");
    }

    #[test]
    fn las_lunas_de_un_planeta_que_aun_es_disco_no_se_ven_aunque_serian_punto() {
        let mut u = Universo::nuevo();
        let p = u.nuevo_id();
        u.astros.push(Astro::planeta(p, 0.0, 0.0, 250.0)); // exoplaneta
        let l = u.nuevo_id();
        let mut luna = Astro::luna(l, "m:a", "q", 0.0, 0.0);
        luna.padre = Some(p);
        u.astros.push(luna);
        u.marcar_cambio();
        // Zoom 0,1: planeta de 25 px (Disco); la luna mediria 4,8 px (Punto).
        let v = ver(&u, 0.1);
        assert!(v.iter().any(|x| x.id == p && x.nivel == Nivel::Disco));
        assert!(
            v.iter().all(|x| x.id != l),
            "el planeta cerrado no ensena sus lunas"
        );
        // Caso contrario: a zoom 0,2 el planeta mide 50 px (Icono) y la luna sale.
        assert!(ver(&u, 0.2).iter().any(|x| x.id == l));
    }

    #[test]
    fn la_linea_sale_del_borde_de_cada_circulo() {
        let a = Astro::planeta(IdAstro(1), 0.0, 0.0, 100.0);
        let b = Astro::planeta(IdAstro(2), 1000.0, 0.0, 200.0);
        let (p, q) = extremos(&a, &b);
        assert_eq!(p, (100.0, 0.0));
        assert_eq!(q, (800.0, 0.0));
    }

    #[test]
    fn dos_circulos_solapados_se_unen_por_el_centro() {
        let a = Astro::planeta(IdAstro(1), 0.0, 0.0, 100.0);
        let b = Astro::planeta(IdAstro(2), 150.0, 0.0, 100.0);
        assert_eq!(extremos(&a, &b), ((0.0, 0.0), (150.0, 0.0)));
    }

    #[test]
    fn una_conexion_a_una_luna_oculta_se_ata_a_su_galaxia_y_entre_la_misma_no_se_pinta() {
        let mut u = galaxia_con_lunas(2);
        let g2 = u.nuevo_id();
        u.astros.push(Astro::galaxia(g2, "q", 6000.0, 0.0));
        u.conexiones.push(Conexion::nueva(50, IdAstro(2), g2)); // luna de g1 -> g2
        u.conexiones
            .push(Conexion::nueva(51, IdAstro(2), IdAstro(3))); // luna -> luna, misma galaxia
        u.marcar_cambio();
        let v = ver(&u, 0.02);
        let c = conexiones_visibles(&u, &v);
        assert_eq!(c, vec![(50, IdAstro(1), g2)]);
    }

    #[test]
    fn se_pulsa_la_linea_mas_cercana_y_lejos_de_ella_no_hay_ninguna() {
        let mut u = galaxia_con_lunas(1);
        let g2 = u.nuevo_id();
        u.astros.push(Astro::galaxia(g2, "q", 6000.0, 0.0));
        let g3 = u.nuevo_id();
        u.astros.push(Astro::galaxia(g3, "r", 3000.0, 6000.0));
        u.conexiones.push(Conexion::nueva(50, IdAstro(1), g2));
        u.conexiones.push(Conexion::nueva(51, IdAstro(1), g3));
        u.marcar_cambio();
        let v = ver(&u, 0.02);

        // La linea 50 va de (0,0) a (6000,0): a la mitad y un poco por
        // encima se pulsa esa, no la que sube hacia g3.
        assert_eq!(conexion_en(&u, &v, 3000.0, 30.0, 200.0), Some(50));
        // Caso negativo: lejos de las dos no hay ninguna.
        assert_eq!(conexion_en(&u, &v, 3000.0, 3000.0, 200.0), None);
        // Caso negativo: mas alla del extremo tampoco, aunque este en la
        // recta que prolonga el segmento.
        assert_eq!(conexion_en(&u, &v, 20_000.0, 0.0, 200.0), None);
    }

    #[test]
    fn la_distancia_a_un_segmento_no_es_la_distancia_a_su_recta() {
        // Dentro del segmento: la perpendicular.
        assert!((distancia_a_segmento(5.0, 3.0, 0.0, 0.0, 10.0, 0.0) - 3.0).abs() < 1e-5);
        // Pasado el extremo: la distancia al extremo, no 3.
        assert!((distancia_a_segmento(14.0, 3.0, 0.0, 0.0, 10.0, 0.0) - 5.0).abs() < 1e-5);
        // Un segmento de largo cero es un punto y no divide por cero.
        assert!((distancia_a_segmento(3.0, 4.0, 1.0, 1.0, 1.0, 1.0) - 13f32.sqrt()).abs() < 1e-4);
    }
}
