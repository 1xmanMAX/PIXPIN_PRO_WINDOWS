//! **Las marcas sobre el texto** (8-oct-2026 en los pines, 1-nov-2026 en
//! todos los visores): resaltar, raya ondulada, subrayar, tachar y tapar lo
//! que se eligio con el raton.
//!
//! Es **el componente comun**: los pines (sobre el texto reconocido de una
//! foto), el lector de PDF y el de Word y PowerPoint (sobre el texto de sus
//! hojas, o el reconocido de una hoja escaneada) piden aqui las mismas
//! figuras, con los mismos colores y el mismo grosor. Cada visor solo pone
//! las cajas de lo elegido en las unidades de su dibujo.
//!
//! Cada marca es **dibujo normal** (un rectangulo o una linea del motor), no
//! pixeles pintados: se ve encima del documento, sale al exportar o
//! imprimir con lo anotado, viaja al movil como cualquier trazo y se puede
//! borrar. Lo unico que la distingue de lo dibujado a mano es su grupo,
//! `marca-texto-<n>` ([`GRUPO`]): asi el circulo tachado quita las marcas
//! y no lo dibujado.
//!
//! Para los documentos que se reajustan al cambiar el ancho (una nota, un
//! texto, una pagina web) las cajas no sirven de ancla: ver
//! `pindesk_docs::ancla_texto`.

use pindesk_geom::Rect;

use crate::{ColorRgba, Elemento, Figura, Punto2};

/// Como se marca el texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Marca {
    /// Un fondo translucido detras de cada renglon.
    Resaltar,
    /// Una raya ondulada debajo.
    Ondulada,
    /// Una raya recta debajo.
    Subrayar,
    /// Una raya por el medio.
    Tachar,
    /// Un bloque macizo que no deja leer lo de debajo.
    Tapar,
}

impl Marca {
    /// El nombre corto, el que va en un fichero (ver `pindesk_docs::ancla_texto`).
    pub fn nombre(self) -> &'static str {
        match self {
            Marca::Resaltar => "resaltar",
            Marca::Ondulada => "ondulada",
            Marca::Subrayar => "subrayar",
            Marca::Tachar => "tachar",
            Marca::Tapar => "tapar",
        }
    }

    /// La vuelta de [`Marca::nombre`]; `None` con un nombre que no se conoce
    /// (de una version mas nueva): quien lee la deja como esta.
    pub fn de_nombre(n: &str) -> Option<Marca> {
        MARCAS.into_iter().find(|m| m.nombre() == n)
    }
}

/// Las marcas, en el orden de la barra.
pub const MARCAS: [Marca; 5] = [
    Marca::Resaltar,
    Marca::Ondulada,
    Marca::Subrayar,
    Marca::Tachar,
    Marca::Tapar,
];

/// Rosa, lila, azul, verde y amarillo: los de la captura del usuario.
pub const COLORES: [(u8, u8, u8); 5] = [
    (0xF4, 0x72, 0x86),
    (0xB9, 0x9C, 0xF5),
    (0x5B, 0xA4, 0xF5),
    (0x3F, 0xC8, 0x8C),
    (0xF5, 0xC2, 0x18),
];

/// El amarillo, el de un rotulador fluorescente de los de siempre.
pub const COLOR_INICIAL: u8 = 4;

/// El color `k` de [`COLORES`]; uno fuera de rango es el ultimo.
pub fn rgb(k: u8) -> (u8, u8, u8) {
    COLORES[(k as usize).min(COLORES.len() - 1)]
}

/// Lo que llevan delante los grupos de las marcas: `marca-texto-<n>`.
pub const GRUPO: &str = "marca-texto";

/// Cuanto se agranda el resaltado alrededor de las letras: las cajas del
/// reconocimiento van justas y un fondo ceñido parece cortado.
const AIRE: f32 = 0.12;

/// Opacidad del resaltado: deja leer lo de debajo.
const OPACIDAD_RESALTADO: f32 = 0.40;

/// La onda de la raya ondulada, de `x0` a `x1` alrededor de `y`.
pub fn onda(x0: f32, x1: f32, y: f32, amplitud: f32, paso: f32) -> Vec<(f32, f32)> {
    let paso = paso.max(0.5);
    let n = (((x1 - x0) / paso).ceil() as usize).max(1);
    (0..=n)
        .map(|k| {
            let x = (x0 + k as f32 * paso).min(x1);
            // Una ola cada 4 pasos.
            let fase = k as f32 / 4.0 * std::f32::consts::TAU;
            (x, y + amplitud * fase.sin())
        })
        .collect()
}

/// Los elementos de una marca sobre `cajas` (una por renglon), del color
/// `rgb`, todos en el grupo `grupo`. Las cajas en enteros: las del texto
/// reconocido de una foto, en sus pixeles.
pub fn elementos(marca: Marca, rgb: (u8, u8, u8), cajas: &[Rect], grupo: &str) -> Vec<Elemento> {
    let cajas: Vec<[f32; 4]> = cajas
        .iter()
        .map(|c| {
            let (x, y) = (c.x as f32, c.y as f32);
            [x, y, x + c.ancho as f32, y + c.alto as f32]
        })
        .collect();
    elementos_en(marca, rgb, &cajas, grupo)
}

/// Lo mismo con cajas `[x0, y0, x1, y1]` en coma flotante: las de una hoja
/// de PDF en sus unidades, que no son pixeles y no se redondean.
pub fn elementos_en(
    marca: Marca,
    (r, g, b): (u8, u8, u8),
    cajas: &[[f32; 4]],
    grupo: &str,
) -> Vec<Elemento> {
    let color = ColorRgba::opaco(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let base = Elemento {
        trazo: color,
        rugosidad: 0.0,
        grupos: vec![grupo.to_string()],
        ..Default::default()
    };
    cajas
        .iter()
        .filter(|c| c[2] > c[0] && c[3] > c[1] && c.iter().all(|v| v.is_finite()))
        .map(|c| {
            let (x, y, w, h) = (c[0], c[1], c[2] - c[0], c[3] - c[1]);
            let grosor = (h * 0.09).max(1.5);
            match marca {
                Marca::Resaltar | Marca::Tapar => {
                    let aire = h * AIRE;
                    Elemento {
                        figura: Figura::Rectangulo,
                        x: x - aire,
                        y: y - aire,
                        ancho: w + 2.0 * aire,
                        alto: h + 2.0 * aire,
                        relleno: Some(color),
                        estilo_relleno: crate::relleno::EstiloRelleno::Solido,
                        // Sin borde: un contorno encima del relleno translucido
                        // saldria mas oscuro que el resto.
                        trazo: ColorRgba { a: 0.0, ..color },
                        grosor: 1.0,
                        opacidad: if marca == Marca::Tapar {
                            1.0
                        } else {
                            OPACIDAD_RESALTADO
                        },
                        ..base.clone()
                    }
                }
                Marca::Subrayar | Marca::Tachar | Marca::Ondulada => {
                    let altura = match marca {
                        Marca::Tachar => y + h * 0.55,
                        _ => y + h + grosor,
                    };
                    let puntos: Vec<Punto2> = if marca == Marca::Ondulada {
                        let paso = (h * 0.09).max(1.5);
                        onda(x, x + w, altura, paso, paso)
                            .into_iter()
                            .map(|(px, py)| Punto2::nuevo(px, py))
                            .collect()
                    } else {
                        vec![Punto2::nuevo(x, altura), Punto2::nuevo(x + w, altura)]
                    };
                    let (alto_min, alto_max) = puntos
                        .iter()
                        .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
                    Elemento {
                        figura: Figura::Linea { puntos },
                        x,
                        y: alto_min,
                        ancho: w,
                        alto: alto_max - alto_min,
                        grosor: if marca == Marca::Ondulada {
                            grosor * 0.8
                        } else {
                            grosor
                        },
                        ..base.clone()
                    }
                }
            }
        })
        .collect()
}

/// El grupo de una marca nueva: uno por marca (asi se mueve entera en el
/// lienzo y se quita entera), con la hora para que no se repita.
pub fn grupo_nuevo(ahora_ms: i64) -> String {
    format!("{GRUPO}-{ahora_ms}")
}

/// Si `e` es una marca de texto.
pub fn es_marca(e: &Elemento) -> bool {
    e.grupos.iter().any(|g| g.starts_with(GRUPO))
}

/// Si la marca `e` cae sobre alguna de `cajas` (`[x0, y0, x1, y1]`).
pub fn toca_en(e: &Elemento, cajas: &[[f32; 4]]) -> bool {
    let (x0, y0, x1, y1) = e.caja();
    cajas
        .iter()
        .any(|c| x0 < c[2] && c[0] < x1 && y0 < c[3] && c[1] < y1)
}

/// Si la marca `e` cae sobre alguna de `cajas`.
pub fn toca(e: &Elemento, cajas: &[Rect]) -> bool {
    let cajas: Vec<[f32; 4]> = cajas
        .iter()
        .map(|c| {
            let (x, y) = (c.x as f32, c.y as f32);
            [x, y, x + c.ancho as f32, y + c.alto as f32]
        })
        .collect();
    toca_en(e, &cajas)
}

/// Las marcas a la vista de `escena` que caen sobre `cajas`: las que quita
/// el circulo tachado, y las que cambia marcar encima con otra.
pub fn las_que_tocan(escena: &crate::Escena, cajas: &[[f32; 4]]) -> Vec<u64> {
    escena
        .visibles()
        .filter(|e| es_marca(e) && toca_en(e, cajas))
        .map(|e| e.id)
        .collect()
}

/// La marca bajo un punto, si hay: su grupo entero (todas sus rayas o
/// fondos, uno por renglon). Es lo que elige un clic encima de una marca.
pub fn grupo_en(escena: &crate::Escena, x: f32, y: f32, holgura: f32) -> Option<Vec<u64>> {
    let e = escena
        .elementos
        .iter()
        .rev()
        .filter(|e| !e.borrado)
        .find(|e| {
            let (x0, y0, x1, y1) = e.caja();
            es_marca(e)
                && x >= x0 - holgura
                && x <= x1 + holgura
                && y >= y0 - holgura
                && y <= y1 + holgura
        })?;
    let g = e.grupos.iter().find(|g| g.starts_with(GRUPO))?.clone();
    Some(
        escena
            .visibles()
            .filter(|o| o.grupos.contains(&g))
            .map(|o| o.id)
            .collect(),
    )
}

/// **Lo que la lupa tiene que volver a tapar**: los mosaicos, como siempre,
/// y ademas lo cubierto con «Tapar». La lupa amplia la foto SIN lo dibujado,
/// y sin esto bastaba pasarla por encima para leer lo tapado.
pub fn tapadas(elementos: &[Elemento]) -> Vec<(f32, f32, f32, f32)> {
    let mut v = crate::mosaico::cajas_tapadas(elementos);
    v.extend(
        elementos
            .iter()
            .filter(|e| {
                !e.borrado
                    && es_marca(e)
                    && matches!(e.figura, Figura::Rectangulo)
                    && e.opacidad >= 1.0
            })
            .map(crate::mosaico::caja_girada),
    );
    v
}

/// Un paso de las marcas, para `Ctrl+Z` (el de los pines, que guardan su
/// dibujo en un fichero y no en una escena viva).
#[derive(Debug, Clone)]
pub enum Paso {
    /// Se anadieron estos elementos (sus ids en el dibujo).
    Anadidos(Vec<u64>),
    /// Se quitaron estos, tal como estaban.
    Quitados(Vec<Elemento>),
}

/// Lo que hizo un cambio en el dibujo: nada (y no se guarda), o algo, con
/// el paso que lo deshace si se apila.
#[derive(Debug)]
pub enum Cambio {
    Nada,
    Hecho(Option<Paso>),
}

/// Cuantos pasos se recuerdan por pin.
pub const PASOS: usize = 50;

/// **Una hoja en la pantalla**: donde cae su esquina, cuantos pixeles vale
/// una unidad de la hoja y cuantos cuartos de vuelta va girada (en el
/// sentido de las agujas). Es la cuenta de ida y vuelta que hace falta para
/// poner la barra junto a lo elegido y para saber que hay bajo el raton.
/// Las marcas no la usan: van en unidades de la hoja y se pintan con la
/// misma transformada que ella, asi que ni acercar ni girar las separa.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HojaEnPantalla {
    /// La esquina de arriba a la izquierda de la hoja ya girada, en pixeles.
    pub x: f32,
    pub y: f32,
    /// Pixeles por unidad de la hoja.
    pub escala: f32,
    /// Cuartos de vuelta (0..4).
    pub giro: u8,
    /// Lo que mide la hoja sin girar, en sus unidades.
    pub ancho: f32,
    pub alto: f32,
}

impl HojaEnPantalla {
    /// Un punto de la hoja (en sus unidades) en la pantalla.
    pub fn a_pantalla(&self, (u, v): (f32, f32)) -> (f32, f32) {
        let (gx, gy) = match self.giro % 4 {
            0 => (u, v),
            1 => (self.alto - v, u),
            2 => (self.ancho - u, self.alto - v),
            _ => (v, self.ancho - u),
        };
        (self.x + gx * self.escala, self.y + gy * self.escala)
    }

    /// La vuelta: un punto de la pantalla en unidades de la hoja.
    pub fn a_hoja(&self, (px, py): (f32, f32)) -> (f32, f32) {
        let k = if self.escala.abs() > f32::EPSILON {
            self.escala
        } else {
            1.0
        };
        let (gx, gy) = ((px - self.x) / k, (py - self.y) / k);
        match self.giro % 4 {
            0 => (gx, gy),
            1 => (gy, self.alto - gx),
            2 => (self.ancho - gx, self.alto - gy),
            _ => (self.ancho - gy, gx),
        }
    }

    /// Una caja de la hoja `[x0, y0, x1, y1]` en la pantalla (la que la
    /// abarca, ya girada).
    pub fn caja_a_pantalla(&self, c: [f32; 4]) -> [f32; 4] {
        let a = self.a_pantalla((c[0], c[1]));
        let b = self.a_pantalla((c[2], c[3]));
        [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)]
    }
}

/// Las cajas de una hoja en fracciones de ella (`0..1`, las de su texto)
/// llevadas a sus unidades de dibujo, de `ancho` x `alto`.
pub fn de_fracciones(cajas: &[[f32; 4]], ancho: f32, alto: f32) -> Vec<[f32; 4]> {
    cajas
        .iter()
        .map(|c| [c[0] * ancho, c[1] * alto, c[2] * ancho, c[3] * alto])
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn renglon(y: i32) -> Rect {
        Rect {
            x: 10,
            y,
            ancho: 200,
            alto: 20,
        }
    }

    #[test]
    fn una_marca_por_renglon_con_su_grupo() {
        let cajas = [renglon(10), renglon(40)];
        for m in MARCAS {
            let v = elementos(m, (255, 200, 0), &cajas, "marca-texto-1");
            assert_eq!(v.len(), 2, "{m:?}");
            assert!(v.iter().all(es_marca));
        }
        // Una caja vacia no deja marca.
        let vacia = Rect {
            ancho: 0,
            ..renglon(0)
        };
        assert!(elementos(Marca::Tachar, (0, 0, 0), &[vacia], "marca-texto-1").is_empty());
        // Ni una con numeros rotos.
        assert!(
            elementos_en(Marca::Tachar, (0, 0, 0), &[[0.0, 0.0, f32::NAN, 1.0]], "g").is_empty()
        );
    }

    #[test]
    fn el_nombre_de_cada_marca_va_y_vuelve() {
        for m in MARCAS {
            assert_eq!(Marca::de_nombre(m.nombre()), Some(m));
        }
        // Caso negativo: una de una version mas nueva no se inventa.
        assert_eq!(Marca::de_nombre("de-otra-version"), None);
    }

    #[test]
    fn resaltar_deja_leer_y_tapar_no() {
        let r = &elementos(Marca::Resaltar, (255, 200, 0), &[renglon(10)], "g")[0];
        let t = &elementos(Marca::Tapar, (255, 200, 0), &[renglon(10)], "g")[0];
        assert!(r.opacidad < 0.6);
        assert_eq!(t.opacidad, 1.0);
        // Los dos cubren las letras enteras, con un poco de aire.
        for e in [r, t] {
            assert!(e.x < 10.0 && e.y < 10.0);
            assert!(e.x + e.ancho > 210.0 && e.y + e.alto > 30.0);
            assert!(e.relleno.is_some());
            assert_eq!(e.trazo.a, 0.0);
        }
    }

    #[test]
    fn subrayar_va_debajo_tachar_por_el_medio_y_la_onda_sube_y_baja() {
        let caja = renglon(10);
        let y_de = |m| match &elementos(m, (0, 0, 0), &[caja], "g")[0].figura {
            Figura::Linea { puntos } => puntos.iter().map(|p| p.y).collect::<Vec<_>>(),
            otra => panic!("{otra:?}"),
        };
        let sub = y_de(Marca::Subrayar);
        let tac = y_de(Marca::Tachar);
        assert!(sub.iter().all(|y| *y > 30.0), "debajo de las letras");
        assert!(tac.iter().all(|y| *y > 15.0 && *y < 25.0), "por el medio");
        let onda = y_de(Marca::Ondulada);
        let (min, max) = onda
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(*y), b.max(*y)));
        assert!(max - min > 2.0 && min > 25.0);
    }

    #[test]
    fn quitar_solo_toca_las_marcas_que_caen_en_la_seleccion() {
        let v = elementos(
            Marca::Resaltar,
            (0, 0, 0),
            &[renglon(10), renglon(100)],
            "g",
        );
        assert!(toca(&v[0], &[renglon(12)]));
        assert!(!toca(&v[1], &[renglon(12)]));
        // Un rectangulo dibujado a mano no es una marca.
        let a_mano = Elemento::default();
        assert!(!es_marca(&a_mano));
    }

    #[test]
    fn en_una_escena_se_encuentran_las_que_tocan_y_el_grupo_bajo_el_raton() {
        let mut escena = crate::Escena::nueva();
        let a_mano = escena.anadir(Elemento {
            figura: Figura::Rectangulo,
            x: 0.0,
            y: 0.0,
            ancho: 500.0,
            alto: 500.0,
            ..Default::default()
        });
        let ids: Vec<u64> = elementos_en(
            Marca::Subrayar,
            (0, 0, 0),
            &[[10.0, 10.0, 200.0, 30.0], [10.0, 40.0, 120.0, 60.0]],
            "marca-texto-7",
        )
        .into_iter()
        .map(|e| escena.anadir(e))
        .collect();
        let otra = escena.anadir(
            elementos_en(
                Marca::Tachar,
                (0, 0, 0),
                &[[10.0, 300.0, 200.0, 320.0]],
                "marca-texto-8",
            )
            .remove(0),
        );
        let tocan = las_que_tocan(&escena, &[[50.0, 25.0, 60.0, 65.0]]);
        assert_eq!(tocan, ids, "lo dibujado a mano no cuenta");
        // Un clic en el segundo renglon elige la marca entera, los dos.
        let g = grupo_en(&escena, 60.0, 62.0, 3.0).unwrap();
        assert_eq!(g, ids);
        assert_eq!(grupo_en(&escena, 60.0, 314.0, 3.0), Some(vec![otra]));
        // Caso negativo: en blanco (sobre el rectangulo a mano) no hay marca.
        assert_eq!(grupo_en(&escena, 400.0, 200.0, 3.0), None);
        assert!(escena.buscar(a_mano).is_some());
    }

    #[test]
    fn la_lupa_vuelve_a_tapar_lo_tapado_pero_no_lo_resaltado() {
        let mut v = elementos(Marca::Tapar, (0, 0, 0), &[renglon(10)], "marca-texto-1");
        v.extend(elementos(
            Marca::Resaltar,
            (0, 0, 0),
            &[renglon(100)],
            "marca-texto-1",
        ));
        // Caso negativo: un tapado a mano (sin grupo de marca) no es asunto
        // de este modulo.
        assert!(tapadas(&elementos(Marca::Tapar, (0, 0, 0), &[renglon(10)], "otro")).is_empty());
        let t = tapadas(&v);
        assert_eq!(t.len(), 1);
        assert!(t[0].1 < 10.0 && t[0].3 > 30.0);
    }

    #[test]
    fn la_onda_sube_y_baja_y_acaba_justo_en_el_final() {
        let v = onda(0.0, 20.0, 10.0, 2.0, 1.0);
        assert_eq!(v.first().unwrap().0, 0.0);
        assert_eq!(v.last().unwrap().0, 20.0);
        assert!(v.iter().any(|p| p.1 > 11.5) && v.iter().any(|p| p.1 < 8.5));
        assert_eq!(rgb(99), COLORES[COLORES.len() - 1]);
    }

    fn cerca(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
    }

    #[test]
    fn de_la_hoja_a_la_pantalla_y_vuelta_con_cualquier_aumento_y_giro() {
        for escala in [0.25, 1.0, 2.5, 7.0] {
            for giro in 0..4 {
                let h = HojaEnPantalla {
                    x: 30.0,
                    y: -400.0,
                    escala,
                    giro,
                    ancho: 1400.0,
                    alto: 1980.0,
                };
                for p in [(0.0, 0.0), (700.0, 990.0), (1400.0, 12.5), (3.0, 1980.0)] {
                    assert!(cerca(h.a_hoja(h.a_pantalla(p)), p), "{p:?} {escala} {giro}");
                }
            }
        }
        // Sin girar: la esquina cae en (x, y) y una unidad son `escala` pixeles.
        let h = HojaEnPantalla {
            x: 10.0,
            y: 20.0,
            escala: 2.0,
            giro: 0,
            ancho: 1400.0,
            alto: 1980.0,
        };
        assert!(cerca(h.a_pantalla((100.0, 50.0)), (210.0, 120.0)));
        assert_eq!(
            h.caja_a_pantalla([100.0, 50.0, 200.0, 60.0]),
            [210.0, 120.0, 410.0, 140.0]
        );
        // Un cuarto de vuelta: la esquina de arriba a la izquierda de la hoja
        // queda arriba a la derecha.
        let g = HojaEnPantalla { giro: 1, ..h };
        assert!(cerca(g.a_pantalla((0.0, 0.0)), (10.0 + 1980.0 * 2.0, 20.0)));
        let c = g.caja_a_pantalla([100.0, 50.0, 200.0, 60.0]);
        assert!(c[2] - c[0] < c[3] - c[1], "lo ancho pasa a ser alto");
    }

    #[test]
    fn las_fracciones_pasan_a_las_unidades_de_la_hoja() {
        let v = de_fracciones(&[[0.1, 0.5, 0.2, 0.51]], 1400.0, 2000.0);
        assert!((v[0][0] - 140.0).abs() < 1e-3 && (v[0][3] - 1020.0).abs() < 1e-3);
        assert!(de_fracciones(&[], 1.0, 1.0).is_empty());
    }
}
