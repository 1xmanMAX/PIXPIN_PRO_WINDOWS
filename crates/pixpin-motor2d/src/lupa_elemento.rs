//! **La lupa y el foco como elementos del dibujo**, con los nueve campos que
//! el PC guardaba sin entender.
//!
//! Ojo con el nombre, porque hay dos cosas distintas que se llaman lupa: la
//! del PC (`Herramienta::Lupa`, `gesto.rs`) es una **vista** que agranda
//! alrededor del cursor y no deja rastro; la del movil es un **elemento del
//! dibujo** —dos recuadros: el cristal, que es la caja del elemento, y el
//! foco, que es a donde mira—. Este modulo es la segunda, y por eso se llama
//! `lupa_elemento` y no `lupa`.
//!
//! El foco (`pixpin-spotlight`) comparte con ella casi todo: la misma caja, el
//! mismo `focoAncho`/`focoAlto`, la misma forma. Lo que cambia es que el foco
//! no agranda nada, **oscurece el resto**. Por eso van juntos aqui: son la
//! misma geometria con dos finales.
//!
//! # Lo que aqui se decide y lo que no
//!
//! Se decide toda la geometria —a donde mira, que recuadro recoge, cuanto
//! agranda, cuanto oscurece— y la ida y vuelta al JSON del movil. **No** se
//! pinta el contenido ampliado: para eso hacen falta los pixeles de debajo,
//! que el motor no tiene (lo mismo que el mosaico, ver `mosaico.rs`). Quien
//! pinta pide `region` y `cristal` y copia de uno a otro.
//!
//! Todos los numeros salen de `Lupa.kt` de la v0.72.0, no de una estimacion.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::vector::Punto2;

/// Lo menos que agranda una lupa: nada.
pub const AUMENTO_MINIMO: f32 = 1.0;
/// Y lo mas. Mas alla se ve el grano de la pantalla, no el detalle.
pub const AUMENTO_MAXIMO: f32 = 12.0;
/// Con cuanto nace: el doble, que es lo que se pide sin pensarlo.
pub const AUMENTO_POR_DEFECTO: f32 = 2.0;

/// Lo menos que puede oscurecer un foco, en por ciento.
pub const OSCURECER_MINIMO: u8 = 10;
/// Y lo mas: por encima de esto lo de alrededor deja de verse.
pub const OSCURECER_MAXIMO: u8 = 90;
/// Con cuanto nace un foco: se nota sin esconder el contexto.
///
/// **No es el 60 % que usa hoy el pintado del PC.** Un foco del movil abierto
/// aqui oscurecia de mas; con esto, los dos aparatos apagan lo mismo.
pub const OSCURECER_POR_DEFECTO: u8 = 45;

/// Cuanto sobresale el marco de un foco recien puesto, respecto a su figura.
pub const MARCO_DEL_FOCO: f32 = 1.8;
/// Lo menos que puede ocupar la zona iluminada dentro del marco.
pub const ZONA_MINIMA: f32 = 0.15;
/// Y lo mas: pegada al marco no queda anillo que oscurecer.
pub const ZONA_MAXIMA: f32 = 0.92;

/// Con que se senala de donde sale lo que se ve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GuiaDeLupa {
    #[default]
    Ninguna,
    /// Una flecha del detalle al cristal. Ocupa poco y se lee de lejos.
    Flecha,
    /// El cono de toda la vida en un plano: dos rayas que salen de los
    /// costados del detalle y se abren hasta los del cristal. Dice ademas
    /// **cuanto** se ha ampliado, porque la apertura se ve.
    DosLineas,
    /// Un punto en el sitio y una raya hasta la ventana: para senalar **un
    /// sitio y no una zona**, cuando lo mirado es una mota.
    Punto,
}

impl GuiaDeLupa {
    /// Si este modo dibuja ademas el contorno de la zona mirada.
    pub fn dibuja_la_zona(self) -> bool {
        matches!(self, GuiaDeLupa::Flecha | GuiaDeLupa::DosLineas)
    }

    pub fn palabra(self) -> &'static str {
        match self {
            GuiaDeLupa::Ninguna => "NINGUNA",
            GuiaDeLupa::Flecha => "FLECHA",
            GuiaDeLupa::DosLineas => "DOS_LINEAS",
            GuiaDeLupa::Punto => "PUNTO",
        }
    }

    pub fn desde_palabra(p: &str) -> Option<GuiaDeLupa> {
        Some(match p {
            "NINGUNA" => GuiaDeLupa::Ninguna,
            "FLECHA" => GuiaDeLupa::Flecha,
            "DOS_LINEAS" => GuiaDeLupa::DosLineas,
            "PUNTO" => GuiaDeLupa::Punto,
            _ => return None,
        })
    }
}

/// Los campos propios de una lupa o de un foco.
///
/// Van en un struct aparte y no sueltos en `Elemento` por lo mismo que
/// `Extras`: cada campo suelto obliga a tocar los ocho sitios del proyecto que
/// construyen un `Elemento` entero a mano.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Cristal {
    /// A donde mira, **en coordenadas absolutas del dibujo**. Es lo que
    /// permite apartar el resultado sin mover lo que se esta mirando.
    /// `None` = mira a su propio centro, como una lupa dejada sobre el papel.
    #[serde(default)]
    pub foco: Option<Punto2>,
    /// Cuanto agranda, si se fijo a mano. Ver [`aumento_de`], que prefiere la
    /// geometria.
    #[serde(default)]
    pub aumento: Option<f32>,
    /// Lo que mide la zona mirada. **Se guarda, no se calcula**: es lo que no
    /// se mueve por mucho que crezca el cristal.
    #[serde(default)]
    pub foco_ancho: Option<f32>,
    #[serde(default)]
    pub foco_alto: Option<f32>,
    /// Cuanto oscurece un foco, en por ciento (10-90).
    #[serde(default)]
    pub oscurecer: Option<u8>,
    /// El cristal es redondo en vez de rectangular.
    #[serde(default)]
    pub redonda: bool,
    #[serde(default)]
    pub guia: GuiaDeLupa,
    /// El contorno del cristal **en proporcion de su caja** (0 a 1). Es lo que
    /// permite que una lupa tenga la forma de cualquier figura cerrada que se
    /// toque con la varita, y aguanta estirarla y girarla sin deformarse.
    #[serde(default)]
    pub forma: Option<Vec<Punto2>>,
}

/// Una caja `(x, y, ancho, alto)` del documento.
pub type Caja = (f32, f32, f32, f32);

fn centro(c: Caja) -> Punto2 {
    Punto2::nuevo(c.0 + c.2 / 2.0, c.1 + c.3 / 2.0)
}

/// **Cuanto agranda esta lupa de verdad.**
///
/// Sale de la geometria —el cristal partido por el foco— y no del numero
/// guardado, porque el cristal se estira con los tiradores: un numero aparte
/// se quedaria diciendo x2 mientras se esta viendo x5.
pub fn aumento_de(cr: &Cristal, caja: Caja) -> f32 {
    match cr.foco_ancho {
        Some(a) if a > 0.0 => (caja.2.abs() / a).clamp(AUMENTO_MINIMO, AUMENTO_MAXIMO),
        _ => cr
            .aumento
            .unwrap_or(AUMENTO_POR_DEFECTO)
            .clamp(AUMENTO_MINIMO, AUMENTO_MAXIMO),
    }
}

/// A donde mira. Sin foco puesto, a su propio centro.
pub fn foco_de(cr: &Cristal, caja: Caja) -> Punto2 {
    cr.foco.unwrap_or_else(|| centro(caja))
}

/// **El recuadro que se recoge**, centrado en el foco, en coordenadas del
/// documento y como `(x, y, ancho, alto)`.
///
/// Mide lo guardado; y solo si no lo lleva —una lupa de antes— se deduce del
/// cristal partido por el aumento, que era la forma vieja de calcularlo.
pub fn region(cr: &Cristal, caja: Caja) -> Caja {
    let z = cr.aumento.unwrap_or(AUMENTO_POR_DEFECTO).max(f32::EPSILON);
    let ancho = cr.foco_ancho.unwrap_or(caja.2.abs() / z);
    let alto = cr.foco_alto.unwrap_or(caja.3.abs() / z);
    let f = foco_de(cr, caja);
    (f.x - ancho / 2.0, f.y - alto / 2.0, ancho, alto)
}

/// Si el punto cae en la zona mirada, con margen.
///
/// Se pica **el recuadro entero** y no solo su centro: en una lupa de mucho
/// aumento la zona es diminuta y acertarle a un punto seria imposible.
pub fn toca_el_foco(cr: &Cristal, caja: Caja, p: Punto2, margen: f32) -> bool {
    let (x, y, ancho, alto) = region(cr, caja);
    // Con forma propia, y tambien con cristal rectangular, se pica la caja: en
    // un contorno raro y diminuto exigir acertar dentro del garabato seria no
    // poder cogerlo.
    if cr.forma.is_some() || !cr.redonda {
        return p.x >= x - margen
            && p.x <= x + ancho + margen
            && p.y >= y - margen
            && p.y <= y + alto + margen;
    }
    // Redondo se pica el ovalo: con la caja, las cuatro esquinas respondian a
    // un sitio donde no hay foco dibujado.
    let (rx, ry) = (ancho / 2.0 + margen, alto / 2.0 + margen);
    if rx <= 0.0 || ry <= 0.0 {
        return false;
    }
    let dx = (p.x - (x + ancho / 2.0)) / rx;
    let dy = (p.y - (y + alto / 2.0)) / ry;
    dx * dx + dy * dy <= 1.0
}

/// Cuanto oscurece este foco, en por ciento y siempre dentro de lo razonable.
pub fn oscurecimiento_de(cr: &Cristal) -> u8 {
    cr.oscurecer
        .unwrap_or(OSCURECER_POR_DEFECTO)
        .clamp(OSCURECER_MINIMO, OSCURECER_MAXIMO)
}

/// Que parte del marco ocupa ahora la zona iluminada de un foco, de 0 a 1.
pub fn zona_de(cr: &Cristal, caja: Caja) -> f32 {
    let ancho = caja.2.abs();
    if ancho <= 0.0 {
        return 1.0 / MARCO_DEL_FOCO;
    }
    match cr.foco_ancho {
        Some(s) => (s / ancho).clamp(ZONA_MINIMA, ZONA_MAXIMA),
        None => 1.0 / MARCO_DEL_FOCO,
    }
}

/// El cristal que le toca a una lupa para agrandar `z` veces, y la caja nueva.
///
/// **Mueve la ventana, no la zona mirada**, y crece desde su propio centro
/// para que subir el aumento no la desplace de donde se habia dejado.
pub fn con_aumento(cr: &Cristal, caja: Caja, z: f32) -> (Cristal, Caja) {
    let (_, _, ra, rh) = region(cr, caja);
    if ra <= 0.0 || rh <= 0.0 {
        return (cr.clone(), caja);
    }
    let z = z.clamp(AUMENTO_MINIMO, AUMENTO_MAXIMO);
    let c = centro(caja);
    let (w, h) = (ra * z, rh * z);
    (
        Cristal {
            aumento: Some(z),
            // Se fija el tamano del foco al primer ajuste: hasta entonces
            // podia venir del cristal, y cambiar el cristal lo habria
            // cambiado tambien.
            foco_ancho: Some(ra),
            foco_alto: Some(rh),
            ..cr.clone()
        },
        (c.x - w / 2.0, c.y - h / 2.0, w, h),
    )
}

/// El foco con su zona iluminada mas grande o mas pequena, **dentro del mismo
/// marco**.
///
/// Es lo contrario de [`con_aumento`], y con motivo: en una lupa lo que se
/// ajusta es cuanto agranda, asi que crece la ventana; en un foco lo que se
/// ajusta es cuanto ilumina, con la sombra llegando siempre hasta el marco.
/// Cambiar el marco aqui moveria el borde de la sombra, que es lo unico que el
/// usuario habia colocado a mano.
pub fn con_zona(cr: &Cristal, caja: Caja, parte: f32) -> Cristal {
    let (ancho, alto) = (caja.2.abs(), caja.3.abs());
    if ancho <= 0.0 || alto <= 0.0 {
        return cr.clone();
    }
    let p = parte.clamp(ZONA_MINIMA, ZONA_MAXIMA);
    Cristal {
        foco_ancho: Some(ancho * p),
        foco_alto: Some(alto * p),
        ..cr.clone()
    }
}

/// El contorno del cristal en coordenadas del documento, si lleva forma
/// propia. `None` = el cristal de siempre, recuadro u ovalo segun `redonda`.
pub fn contorno_del_cristal(cr: &Cristal, caja: Caja) -> Option<Vec<Punto2>> {
    let forma = cr.forma.as_ref()?;
    if forma.len() < 3 {
        return None;
    }
    Some(
        forma
            .iter()
            .map(|p| Punto2::nuevo(caja.0 + p.x * caja.2, caja.1 + p.y * caja.3))
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// El puente
// ---------------------------------------------------------------------------

fn num(v: &Value, clave: &str) -> Option<f32> {
    v.get(clave).and_then(Value::as_f64).map(|n| n as f32)
}

/// Lee los campos propios de una lupa o un foco del JSON del movil.
///
/// Lo que no venga se queda en `None`, que **no es lo mismo que un cero**:
/// `foco` ausente significa «mira a su centro» y `focoAncho` ausente significa
/// «deducelo del cristal». Machacarlos con ceros convertiria una lupa en una
/// ventana que no mira a ninguna parte.
pub fn leer(v: &Value) -> Cristal {
    Cristal {
        foco: match (
            v.get("foco").and_then(|f| num(f, "x")),
            v.get("foco").and_then(|f| num(f, "y")),
        ) {
            (Some(x), Some(y)) => Some(Punto2::nuevo(x, y)),
            _ => None,
        },
        aumento: num(v, "aumento"),
        foco_ancho: num(v, "focoAncho"),
        foco_alto: num(v, "focoAlto"),
        oscurecer: v
            .get("oscurecer")
            .and_then(Value::as_i64)
            .map(|n| n.clamp(0, 255) as u8),
        redonda: v.get("lupaRedonda").and_then(Value::as_bool) == Some(true),
        guia: v
            .get("guia")
            .and_then(Value::as_str)
            .and_then(GuiaDeLupa::desde_palabra)
            // Las lupas de antes no llevan campo: se traduce lo que tenian.
            .unwrap_or(
                if v.get("lupaFlecha").and_then(Value::as_bool) == Some(true) {
                    GuiaDeLupa::Flecha
                } else if v.get("lupaDosLineas").and_then(Value::as_bool) == Some(true) {
                    GuiaDeLupa::DosLineas
                } else {
                    GuiaDeLupa::Ninguna
                },
            ),
        forma: v.get("forma").and_then(Value::as_array).map(|a| {
            a.iter()
                .filter_map(|p| Some(Punto2::nuevo(num(p, "x")?, num(p, "y")?)))
                .collect()
        }),
    }
}

/// Escribe los campos propios sobre el mapa del elemento.
///
/// **Lo que vale `None` se borra en vez de escribirse a cero**, por lo mismo
/// que en `leer`. Y `lupaFlecha`/`lupaDosLineas` se reescriben a juego con
/// `guia` para que una lupa editada aqui se abra igual en un movil viejo.
pub fn escribir(mapa: &mut Map<String, Value>, cr: &Cristal) {
    fn poner(mapa: &mut Map<String, Value>, clave: &str, v: Option<f64>) {
        match v {
            Some(n) => {
                mapa.insert(clave.into(), Value::from(n));
            }
            None => {
                mapa.remove(clave);
            }
        }
    }
    match cr.foco {
        Some(p) => {
            let mut m = Map::new();
            m.insert("x".into(), Value::from(p.x as f64));
            m.insert("y".into(), Value::from(p.y as f64));
            mapa.insert("foco".into(), Value::Object(m));
        }
        None => {
            mapa.remove("foco");
        }
    }
    poner(mapa, "aumento", cr.aumento.map(|n| n as f64));
    poner(mapa, "focoAncho", cr.foco_ancho.map(|n| n as f64));
    poner(mapa, "focoAlto", cr.foco_alto.map(|n| n as f64));
    match cr.oscurecer {
        Some(n) => {
            mapa.insert("oscurecer".into(), Value::from(n));
        }
        None => {
            mapa.remove("oscurecer");
        }
    }
    mapa.insert("lupaRedonda".into(), Value::Bool(cr.redonda));
    mapa.insert("guia".into(), Value::String(cr.guia.palabra().to_string()));
    mapa.insert(
        "lupaFlecha".into(),
        Value::Bool(cr.guia == GuiaDeLupa::Flecha),
    );
    mapa.insert(
        "lupaDosLineas".into(),
        Value::Bool(cr.guia == GuiaDeLupa::DosLineas),
    );
    match &cr.forma {
        Some(f) => {
            let puntos: Vec<Value> = f
                .iter()
                .map(|p| {
                    let mut m = Map::new();
                    m.insert("x".into(), Value::from(p.x as f64));
                    m.insert("y".into(), Value::from(p.y as f64));
                    Value::Object(m)
                })
                .collect();
            mapa.insert("forma".into(), Value::Array(puntos));
        }
        None => {
            mapa.remove("forma");
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const CAJA: Caja = (100.0, 100.0, 200.0, 100.0);

    #[test]
    fn una_lupa_sin_foco_mira_a_su_propio_centro() {
        // Una lupa recien dejada sobre el papel agranda lo que tiene debajo.
        let cr = Cristal::default();
        assert_eq!(foco_de(&cr, CAJA), Punto2::nuevo(200.0, 150.0));
        // Y recoge el cristal partido por el aumento de fabrica.
        assert_eq!(region(&cr, CAJA), (150.0, 125.0, 100.0, 50.0));
    }

    #[test]
    fn el_aumento_sale_de_la_geometria_y_no_del_numero_guardado() {
        // El caso que da sentido a la regla: el cristal se estiro con los
        // tiradores y el numero guardado se quedo diciendo x2.
        let cr = Cristal {
            aumento: Some(2.0),
            foco_ancho: Some(50.0),
            ..Default::default()
        };
        assert_eq!(
            aumento_de(&cr, CAJA),
            4.0,
            "200 de cristal sobre 50 de zona"
        );
        // Sin zona guardada si manda el numero.
        let cr = Cristal {
            aumento: Some(3.0),
            ..Default::default()
        };
        assert_eq!(aumento_de(&cr, CAJA), 3.0);
    }

    #[test]
    fn el_aumento_no_se_sale_de_sus_topes_ni_con_una_zona_absurda() {
        // Caso negativo: una zona de un pixel daria x200 y se veria el grano
        // de la pantalla, no el detalle.
        let cr = Cristal {
            foco_ancho: Some(0.1),
            ..Default::default()
        };
        assert_eq!(aumento_de(&cr, CAJA), AUMENTO_MAXIMO);
        // Y una zona mas grande que el cristal no puede encoger el dibujo.
        let cr = Cristal {
            foco_ancho: Some(10_000.0),
            ..Default::default()
        };
        assert_eq!(aumento_de(&cr, CAJA), AUMENTO_MINIMO);
    }

    #[test]
    fn subir_el_aumento_mueve_la_ventana_y_no_la_zona_mirada() {
        let cr = Cristal {
            foco: Some(Punto2::nuevo(500.0, 500.0)),
            foco_ancho: Some(40.0),
            foco_alto: Some(20.0),
            ..Default::default()
        };
        let (nuevo, caja) = con_aumento(&cr, CAJA, 5.0);
        assert_eq!(
            region(&nuevo, caja),
            (480.0, 490.0, 40.0, 20.0),
            "la zona no se movio"
        );
        assert_eq!(
            (caja.2, caja.3),
            (200.0, 100.0),
            "la ventana es la zona por cinco"
        );
        // Y crece desde su propio centro.
        assert_eq!(centro(caja), centro(CAJA));
    }

    #[test]
    fn un_foco_sin_valor_oscurece_lo_mismo_que_el_movil() {
        // 45 y no 60: el 60 era el de fabrica del pintado del PC, y hacia que
        // un foco del movil apagara aqui mas de lo que apagaba alli.
        assert_eq!(oscurecimiento_de(&Cristal::default()), 45);
        // Y lo que venga fuera de rango se recorta, no se acepta.
        let cr = Cristal {
            oscurecer: Some(200),
            ..Default::default()
        };
        assert_eq!(oscurecimiento_de(&cr), OSCURECER_MAXIMO);
        let cr = Cristal {
            oscurecer: Some(0),
            ..Default::default()
        };
        assert_eq!(oscurecimiento_de(&cr), OSCURECER_MINIMO);
    }

    #[test]
    fn ajustar_la_zona_de_un_foco_no_mueve_su_marco() {
        // Es lo contrario que la lupa, y es lo que hace que el borde de la
        // sombra se quede donde el usuario lo puso.
        let cr = con_zona(&Cristal::default(), CAJA, 0.5);
        assert_eq!(cr.foco_ancho, Some(100.0));
        assert_eq!(cr.foco_alto, Some(50.0));
        assert_eq!(zona_de(&cr, CAJA), 0.5);
        // Y la zona no puede comerse el marco entero ni desaparecer.
        assert_eq!(zona_de(&con_zona(&cr, CAJA, 9.0), CAJA), ZONA_MAXIMA);
        assert_eq!(zona_de(&con_zona(&cr, CAJA, 0.0), CAJA), ZONA_MINIMA);
    }

    #[test]
    fn el_foco_redondo_no_se_agarra_por_las_esquinas_de_su_caja() {
        // Caso negativo: con la caja, las cuatro esquinas respondian a un
        // sitio donde no hay nada dibujado.
        let cr = Cristal {
            redonda: true,
            foco_ancho: Some(100.0),
            foco_alto: Some(100.0),
            ..Default::default()
        };
        let c = foco_de(&cr, CAJA);
        assert!(toca_el_foco(&cr, CAJA, c, 0.0), "el centro si");
        let esquina = Punto2::nuevo(c.x + 49.0, c.y + 49.0);
        assert!(!toca_el_foco(&cr, CAJA, esquina, 0.0), "la esquina no");
        // Rectangular, esa misma esquina si.
        let cr = Cristal {
            redonda: false,
            ..cr
        };
        assert!(toca_el_foco(&cr, CAJA, esquina, 0.0));
    }

    #[test]
    fn la_forma_propia_se_devuelve_en_coordenadas_del_dibujo() {
        let cr = Cristal {
            forma: Some(vec![
                Punto2::nuevo(0.0, 0.0),
                Punto2::nuevo(1.0, 0.0),
                Punto2::nuevo(0.5, 1.0),
            ]),
            ..Default::default()
        };
        let c = contorno_del_cristal(&cr, CAJA).unwrap();
        assert_eq!(c[0], Punto2::nuevo(100.0, 100.0));
        assert_eq!(c[1], Punto2::nuevo(300.0, 100.0));
        assert_eq!(c[2], Punto2::nuevo(200.0, 200.0));
        // Una forma que no encierra nada no es una forma.
        let cr = Cristal {
            forma: Some(vec![Punto2::nuevo(0.0, 0.0)]),
            ..Default::default()
        };
        assert!(contorno_del_cristal(&cr, CAJA).is_none());
    }

    #[test]
    fn la_lupa_del_movil_va_y_vuelve_con_sus_nueve_campos() {
        let json = serde_json::json!({
            "id": "l1", "type": "pixpin-lupa",
            "x": 100, "y": 100, "width": 200, "height": 100,
            "foco": {"x": 500.0, "y": 640.0},
            "aumento": 3.5,
            "focoAncho": 57.0, "focoAlto": 28.5,
            "oscurecer": 70,
            "lupaRedonda": true,
            "guia": "DOS_LINEAS",
            "forma": [{"x": 0.0, "y": 0.0}, {"x": 1.0, "y": 0.5}, {"x": 0.0, "y": 1.0}]
        });
        let cr = leer(&json);
        assert_eq!(cr.foco, Some(Punto2::nuevo(500.0, 640.0)));
        assert_eq!(cr.aumento, Some(3.5));
        assert_eq!(cr.foco_ancho, Some(57.0));
        assert_eq!(cr.oscurecer, Some(70));
        assert!(cr.redonda);
        assert_eq!(cr.guia, GuiaDeLupa::DosLineas);
        assert_eq!(cr.forma.as_ref().unwrap().len(), 3);

        let mut mapa = json.as_object().unwrap().clone();
        escribir(&mut mapa, &cr);
        let vuelta = leer(&Value::Object(mapa));
        assert_eq!(cr, vuelta, "la ida y vuelta cambio algo");
    }

    #[test]
    fn una_lupa_vieja_sin_guia_conserva_lo_que_tenia() {
        // Las de antes no llevan campo `guia`, solo los dos si/no. Sin esta
        // traduccion, abrir una lupa vieja aqui le quitaria la flecha.
        let cr = leer(&serde_json::json!({"lupaFlecha": true}));
        assert_eq!(cr.guia, GuiaDeLupa::Flecha);
        let cr = leer(&serde_json::json!({"lupaDosLineas": true}));
        assert_eq!(cr.guia, GuiaDeLupa::DosLineas);
        // Y al escribirla vuelve con los dos campos viejos a juego.
        let mut m = Map::new();
        escribir(&mut m, &cr);
        assert_eq!(m["guia"], Value::String("DOS_LINEAS".into()));
        assert_eq!(m["lupaDosLineas"], Value::Bool(true));
        assert_eq!(m["lupaFlecha"], Value::Bool(false));
    }

    #[test]
    fn lo_que_no_esta_no_se_inventa_a_cero() {
        // **La prueba que importa del puente.** Un `focoAncho` escrito a cero
        // seria una zona de area nula: la lupa dejaria de mirar a ninguna
        // parte y no habria forma de recuperar a donde miraba.
        let cr = leer(&serde_json::json!({}));
        assert_eq!(cr.foco, None);
        assert_eq!(cr.foco_ancho, None);
        assert_eq!(cr.aumento, None);
        assert_eq!(cr.oscurecer, None);
        let mut m = Map::new();
        m.insert("focoAncho".into(), Value::from(12.0));
        escribir(&mut m, &cr);
        assert!(!m.contains_key("focoAncho"), "se escribio un cero: {m:?}");
        assert!(!m.contains_key("foco"));
    }
}
