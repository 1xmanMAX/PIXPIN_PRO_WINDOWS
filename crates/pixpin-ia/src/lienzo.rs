//! **De una llamada validada a elementos del lienzo, en formato Excalidraw.**
//!
//! Excalidraw es el formato que ya comparten el lienzo del PC
//! (`pixpin_motor2d::excalidraw`) y el de Android: lo que dibuja el asistente
//! sale como cualquier otro trazo —se mueve, se cambia de color, se exporta y
//! viaja al movil— y este crate no necesita ver el motor del lienzo. La UI
//! hace `excalidraw::leer(&documento(..))` y anade los elementos a la escena
//! en un solo paso de deshacer.
//!
//! Las coordenadas del modelo son de un lienzo de 800x600 ([`ANCHO_LIENZO`]);
//! aqui solo se desplazan a `origen` (donde mira el usuario).

use serde_json::{Value, json};

use crate::herramientas::{
    ALTO_LIENZO, ANCHO_LIENZO, Figura, Grafica, Llamada, TipoFigura, TipoGrafica,
};

/// Colores de las series, de la paleta de Excalidraw (la del lienzo).
const SERIES: [&str; 8] = [
    "#1971c2", "#e03131", "#2f9e44", "#f08c00", "#6741d9", "#0c8599", "#c2255c", "#868e96",
];

/// Fabrica de elementos con identificadores unicos y estables.
struct Fabrica {
    semilla: u64,
    n: u64,
    ox: f64,
    oy: f64,
    salida: Vec<Value>,
}

impl Fabrica {
    fn id(&mut self) -> String {
        self.n += 1;
        format!("ia-{:x}-{}", self.semilla, self.n)
    }

    fn base(&mut self, tipo: &str, [x, y, ancho, alto]: [f64; 4], trazo: &str, relleno: Option<&str>) -> Value {
        let id = self.id();
        json!({
            "id": id,
            "type": tipo,
            "x": self.ox + x,
            "y": self.oy + y,
            "width": ancho,
            "height": alto,
            "angle": 0,
            "strokeColor": trazo,
            "backgroundColor": relleno.unwrap_or("transparent"),
            "fillStyle": "solid",
            "strokeWidth": 2,
            "strokeStyle": "solid",
            "roughness": 1,
            "opacity": 100,
            "seed": self.semilla.wrapping_add(self.n) % 2_000_000_000,
            "version": 1,
            "isDeleted": false,
            "groupIds": [],
            "boundElements": null,
            "locked": false,
            "customData": {"origen": "pixpin-ia"},
        })
    }

    fn caja(&mut self, tipo: &str, caja: [f64; 4], trazo: &str, relleno: Option<&str>) {
        let e = self.base(tipo, caja, trazo, relleno);
        self.salida.push(e);
    }

    /// Una polilinea; `puntos` en coordenadas del modelo. Excalidraw los
    /// quiere relativos al primero.
    fn trazo(&mut self, tipo: &str, puntos: &[(f64, f64)], color: &str, relleno: Option<&str>) {
        let (x0, y0) = puntos[0];
        let (mut minx, mut miny, mut maxx, mut maxy) = (x0, y0, x0, y0);
        for &(x, y) in puntos {
            minx = minx.min(x);
            miny = miny.min(y);
            maxx = maxx.max(x);
            maxy = maxy.max(y);
        }
        let mut e = self.base(tipo, [x0, y0, maxx - minx, maxy - miny], color, relleno);
        e["points"] = Value::Array(puntos.iter().map(|&(x, y)| json!([x - x0, y - y0])).collect());
        if tipo == "arrow" {
            e["startArrowhead"] = Value::Null;
            e["endArrowhead"] = json!("arrow");
        } else {
            e["startArrowhead"] = Value::Null;
            e["endArrowhead"] = Value::Null;
        }
        self.salida.push(e);
    }

    fn texto(&mut self, x: f64, y: f64, texto: &str, tam: f64, color: &str, alinear: &str) {
        let ancho = tam * 0.6 * texto.chars().count().max(1) as f64;
        let alto = tam * 1.25;
        let x = match alinear {
            "center" => x - ancho / 2.0,
            "right" => x - ancho,
            _ => x,
        };
        let mut e = self.base("text", [x, y, ancho, alto], color, None);
        e["text"] = json!(texto);
        e["originalText"] = json!(texto);
        e["fontSize"] = json!(tam);
        // 1 = Virgil, la letra a mano de Excalidraw (la del lienzo).
        e["fontFamily"] = json!(1);
        e["textAlign"] = json!(alinear);
        e["verticalAlign"] = json!("top");
        e["lineHeight"] = json!(1.25);
        e["containerId"] = Value::Null;
        self.salida.push(e);
    }
}

/// Los elementos de Excalidraw de una llamada. El OCR no dibuja nada: su
/// resultado es texto (lo pone el chat).
///
/// `origen`: donde cae el (0,0) del modelo en el lienzo real. `semilla`:
/// cualquier numero distinto en cada llamada (la hora vale), para que los
/// identificadores no choquen con los de otra respuesta.
pub fn elementos(llamada: &Llamada, origen: (f64, f64), semilla: u64) -> Vec<Value> {
    let mut f = Fabrica {
        semilla,
        n: 0,
        ox: origen.0,
        oy: origen.1,
        salida: Vec::new(),
    };
    match llamada {
        Llamada::Dibujar(figuras) => figuras.iter().for_each(|fig| figura(&mut f, fig)),
        Llamada::Grafica(g) => grafica(&mut f, g),
        Llamada::Ocr(_) => {}
    }
    f.salida
}

/// Un `.excalidraw` completo con esos elementos, listo para
/// `pixpin_motor2d::excalidraw::leer`.
pub fn documento(elementos: Vec<Value>) -> String {
    json!({
        "type": "excalidraw",
        "version": 2,
        "source": "pixpin-ia",
        "elements": elementos,
        "appState": {"viewBackgroundColor": "#ffffff"},
        "files": {},
    })
    .to_string()
}

fn figura(f: &mut Fabrica, fig: &Figura) {
    let trazo = fig.color.0.as_str();
    let relleno = fig.relleno.as_ref().map(|c| c.0.as_str());
    match fig.tipo {
        TipoFigura::Rectangulo => f.caja("rectangle", [fig.x, fig.y, fig.ancho, fig.alto], trazo, relleno),
        TipoFigura::Elipse => f.caja("ellipse", [fig.x, fig.y, fig.ancho, fig.alto], trazo, relleno),
        TipoFigura::Rombo => f.caja("diamond", [fig.x, fig.y, fig.ancho, fig.alto], trazo, relleno),
        TipoFigura::Linea => f.trazo("line", &[(fig.x, fig.y), (fig.x2, fig.y2)], trazo, None),
        TipoFigura::Flecha => f.trazo("arrow", &[(fig.x, fig.y), (fig.x2, fig.y2)], trazo, None),
        TipoFigura::Texto => f.texto(fig.x, fig.y, &fig.texto, fig.tam_letra, trazo, "left"),
    }
}

/// El hueco de la grafica dentro del lienzo de 800x600.
const MARGEN: f64 = 60.0;

fn grafica(f: &mut Fabrica, g: &Grafica) {
    let negro = "#1e1e1e";
    let arriba = if g.titulo.is_empty() {
        MARGEN
    } else {
        f.texto(ANCHO_LIENZO / 2.0, 16.0, &g.titulo, 28.0, negro, "center");
        MARGEN + 20.0
    };
    let (x0, x1) = (MARGEN + 20.0, ANCHO_LIENZO - MARGEN);
    let (y0, y1) = (arriba, ALTO_LIENZO - MARGEN);
    match g.tipo {
        TipoGrafica::Pastel => pastel(f, g, (x0 + x1) / 2.0, (y0 + y1) / 2.0, ((y1 - y0) / 2.0).min(220.0)),
        TipoGrafica::Barras | TipoGrafica::Lineas => {
            let min = g.valores.iter().copied().fold(0.0_f64, f64::min);
            let max = g.valores.iter().copied().fold(0.0_f64, f64::max);
            let rango = if max - min > 0.0 { max - min } else { 1.0 };
            let a_y = |v: f64| y1 - (v - min) / rango * (y1 - y0);
            let cero = a_y(0.0);
            // Ejes.
            f.trazo("line", &[(x0, y0), (x0, y1)], negro, None);
            f.trazo("line", &[(x0, cero), (x1, cero)], negro, None);
            f.texto(x0 - 8.0, a_y(max) - 10.0, &bonito(max), 16.0, negro, "right");
            if min < 0.0 {
                f.texto(x0 - 8.0, a_y(min) - 10.0, &bonito(min), 16.0, negro, "right");
            }
            let n = g.valores.len() as f64;
            let paso = (x1 - x0) / n;
            let mut puntos = Vec::new();
            for (i, (v, etiqueta)) in g.valores.iter().zip(&g.etiquetas).enumerate() {
                let centro = x0 + paso * (i as f64 + 0.5);
                let y = a_y(*v);
                if g.tipo == TipoGrafica::Barras {
                    let ancho = paso * 0.6;
                    let color = SERIES[i % SERIES.len()];
                    f.caja("rectangle", [centro - ancho / 2.0, y.min(cero), ancho, (cero - y).abs().max(1.0)], color, Some(color));
                } else {
                    puntos.push((centro, y));
                }
                f.texto(centro, y.min(cero) - 24.0, &bonito(*v), 16.0, negro, "center");
                f.texto(centro, y1 + 8.0, etiqueta, 16.0, negro, "center");
            }
            if puntos.len() >= 2 {
                f.trazo("line", &puntos, SERIES[0], None);
            } else if let Some(&(x, y)) = puntos.first() {
                f.caja("ellipse", [x - 5.0, y - 5.0, 10.0, 10.0], SERIES[0], Some(SERIES[0]));
            }
        }
    }
}

fn pastel(f: &mut Fabrica, g: &Grafica, cx: f64, cy: f64, r: f64) {
    let total: f64 = g.valores.iter().sum();
    let mut angulo = -std::f64::consts::FRAC_PI_2;
    for (i, (v, etiqueta)) in g.valores.iter().zip(&g.etiquetas).enumerate() {
        let barrido = v / total * std::f64::consts::TAU;
        if barrido <= 0.0 {
            continue;
        }
        let color = SERIES[i % SERIES.len()];
        // La cuna como poligono cerrado: centro, arco muestreado, centro.
        let pasos = ((barrido / 0.1).ceil() as usize).max(2);
        let mut puntos = vec![(cx, cy)];
        for k in 0..=pasos {
            let a = angulo + barrido * k as f64 / pasos as f64;
            puntos.push((cx + r * a.cos(), cy + r * a.sin()));
        }
        puntos.push((cx, cy));
        f.trazo("line", &puntos, "#ffffff", Some(color));
        let medio = angulo + barrido / 2.0;
        let porcentaje = v / total * 100.0;
        f.texto(
            cx + (r + 30.0) * medio.cos(),
            cy + (r + 30.0) * medio.sin() - 10.0,
            &format!("{etiqueta} ({porcentaje:.0}%)"),
            16.0,
            "#1e1e1e",
            "center",
        );
        angulo += barrido;
    }
}

/// Un numero sin decimales de sobra.
fn bonito(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}").trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::herramientas::{catalogo, validar};
    use crate::motor::LlamadaCruda;
    use pixpin_motor2d::elemento::Figura as FiguraLienzo;

    fn llamada(nombre: &str, args: &str) -> Llamada {
        validar(
            &LlamadaCruda {
                id: "1".into(),
                nombre: nombre.into(),
                argumentos: args.into(),
            },
            &catalogo(),
        )
        .unwrap()
        .llamada
    }

    #[test]
    fn el_dibujo_lo_lee_el_lienzo_de_verdad() {
        let l = llamada(
            "dibujar_en_lienzo",
            r##"{"elementos":[
                {"tipo":"elipse","x":40,"y":60,"ancho":150,"alto":150,"color":"rojo","relleno":"rojo"},
                {"tipo":"rectangulo","x":280,"y":60,"ancho":160,"alto":160,"color":"azul"},
                {"tipo":"linea","x":40,"y":300,"x2":470,"y2":300,"color":"verde"},
                {"tipo":"flecha","x":10,"y":10,"x2":60,"y2":40},
                {"tipo":"rombo","x":500,"y":60},
                {"tipo":"texto","x":60,"y":315,"texto":"HOLA PIXPIN 2026"}]}"##,
        );
        let doc = documento(elementos(&l, (1000.0, 2000.0), 7));
        let lienzo = pixpin_motor2d::excalidraw::leer(&doc).unwrap();
        assert_eq!(lienzo.cuantos_ajenos(), 0, "todo tiene que ser editable");
        let els = lienzo.elementos();
        assert_eq!(els.len(), 6);
        assert!(matches!(els[0].figura, FiguraLienzo::Elipse));
        assert!(matches!(els[1].figura, FiguraLienzo::Rectangulo));
        assert!(matches!(els[2].figura, FiguraLienzo::Linea { .. }));
        assert!(matches!(els[3].figura, FiguraLienzo::Flecha { .. }));
        assert!(matches!(els[4].figura, FiguraLienzo::Rombo));
        match &els[5].figura {
            FiguraLienzo::Texto { texto, .. } => assert_eq!(texto, "HOLA PIXPIN 2026"),
            otra => panic!("{otra:?}"),
        }
        // Desplazado al origen pedido.
        let v: Value = serde_json::from_str(&doc).unwrap();
        assert_eq!(v["elements"][0]["x"], 1040.0);
        assert_eq!(v["elements"][2]["points"][1], json!([430.0, 0.0]));
    }

    #[test]
    fn los_identificadores_no_se_repiten() {
        let l = llamada("grafica", r#"{"tipo":"barras","etiquetas":["a","b","c"],"valores":[1,2,3],"titulo":"t"}"#);
        let els = elementos(&l, (0.0, 0.0), 99);
        let mut ids: Vec<&str> = els.iter().map(|e| e["id"].as_str().unwrap()).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn las_tres_graficas_salen_y_se_leen() {
        for (args, minimo) in [
            (r#"{"tipo":"barras","etiquetas":["Enero","Febrero","Marzo"],"valores":[120,95,140],"titulo":"Ventas"}"#, 3 + 3 * 3),
            (r#"{"tipo":"lineas","etiquetas":["a","b","c","d"],"valores":[1,-2,3.5,0]}"#, 3 + 4 * 2 + 1),
            (r#"{"tipo":"pastel","etiquetas":["si","no"],"valores":[3,1]}"#, 4),
        ] {
            let els = elementos(&llamada("grafica", args), (0.0, 0.0), 1);
            assert!(els.len() >= minimo, "{args}: {}", els.len());
            let lienzo = pixpin_motor2d::excalidraw::leer(&documento(els)).unwrap();
            assert_eq!(lienzo.cuantos_ajenos(), 0, "{args}");
        }
    }

    #[test]
    fn el_ocr_no_dibuja() {
        assert!(elementos(&llamada("ocr", "{}"), (0.0, 0.0), 1).is_empty());
    }

    #[test]
    fn numeros_bonitos() {
        assert_eq!(bonito(140.0), "140");
        assert_eq!(bonito(3.5), "3.5");
        assert_eq!(bonito(-2.0), "-2");
    }
}
