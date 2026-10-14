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
    /// El grupo de lo que se esta fabricando (`groupIds`).
    grupo: Option<String>,
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
            "groupIds": self.grupo.iter().collect::<Vec<_>>(),
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

    /// Una mancha rellena con agujeros (`pixpin-region`, la del bote de
    /// pintura): el unico poligono que el lienzo del PC y el del movil
    /// rellenan. Sus puntos y los de los agujeros, relativos al primero.
    fn region(&mut self, contorno: &[(f64, f64)], huecos: &[Vec<(f64, f64)>], trazo: &str, relleno: &str) {
        if contorno.len() < 3 {
            return;
        }
        self.trazo("line", contorno, trazo, Some(relleno));
        let (x0, y0) = contorno[0];
        if let Some(e) = self.salida.last_mut() {
            e["type"] = json!("pixpin-region");
            if let Some(m) = e.as_object_mut() {
                m.remove("startArrowhead");
                m.remove("endArrowhead");
            }
            e["huecos"] = Value::Array(
                huecos
                    .iter()
                    .map(|h| Value::Array(h.iter().map(|&(x, y)| json!([x - x0, y - y0])).collect()))
                    .collect(),
            );
        }
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
        grupo: None,
    };
    match llamada {
        Llamada::Dibujar(figuras) => figuras.iter().for_each(|fig| figura(&mut f, fig)),
        Llamada::Grafica(g) => grafica(&mut f, g),
        // No dibujan nada por si solas: el OCR y la ayuda son texto, la
        // accion de la app es un pedido y el calcado necesita la imagen
        // ([`elementos_de_calcado`]).
        Llamada::Ocr(_) | Llamada::Calcar(_) | Llamada::App(_) | Llamada::Ayuda(_) => {}
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
    let con_rotulo = fig.tipo != TipoFigura::Texto && !fig.texto.is_empty();
    // El grupo que pidio el modelo; si no, una figura con su rotulo va
    // agrupada con el, para que se muevan juntos.
    f.grupo = match &fig.grupo {
        Some(g) => Some(format!("ia-{:x}-g-{g}", f.semilla)),
        None if con_rotulo => Some(format!("ia-{:x}-r-{}", f.semilla, f.n + 1)),
        None => None,
    };
    let caja = [fig.x, fig.y, fig.ancho, fig.alto];
    // Los puntos (las figuras de antes de `puntos` traen solo los extremos).
    let puntos: Vec<(f64, f64)> = if fig.puntos.is_empty() {
        vec![(fig.x, fig.y), (fig.x2, fig.y2)]
    } else {
        fig.puntos.clone()
    };
    match fig.tipo {
        TipoFigura::Rectangulo => f.caja("rectangle", caja, trazo, relleno),
        TipoFigura::Elipse => f.caja("ellipse", caja, trazo, relleno),
        TipoFigura::Rombo => f.caja("diamond", caja, trazo, relleno),
        TipoFigura::Linea => f.trazo("line", &puntos, trazo, None),
        TipoFigura::Flecha => f.trazo("arrow", &puntos, trazo, None),
        // Un texto que se quedo sin texto (ver `colores_del_pedido`) no se pinta.
        TipoFigura::Texto if fig.texto.is_empty() => {}
        TipoFigura::Texto => f.texto(fig.x, fig.y, &fig.texto, fig.tam_letra, trazo, "left"),
        TipoFigura::Curva => {
            f.trazo("line", &puntos, trazo, None);
            // `roundness` en una linea: la curva suave que pasa por sus
            // puntos (la del lienzo y la del movil).
            if let Some(e) = f.salida.last_mut() {
                e["roundness"] = json!({"type": 2});
            }
        }
        // Lleno: la mancha rellena del bote (un `line` cerrado no se
        // rellena en el lienzo del PC). Vacio: una linea cerrada.
        TipoFigura::Poligono => match relleno {
            Some(r) => f.region(&puntos, &[], trazo, r),
            None => {
                let mut cerrado = puntos.clone();
                cerrado.push(puntos[0]);
                f.trazo("line", &cerrado, trazo, None);
            }
        },
    }
    if con_rotulo {
        let negro = "#1e1e1e";
        match fig.tipo {
            // Encima de la flecha o la linea, en su punto medio.
            TipoFigura::Linea | TipoFigura::Flecha | TipoFigura::Curva => {
                let (a, b) = (puntos[0], puntos[puntos.len() - 1]);
                let tam = fig.tam_letra.min(20.0);
                f.texto((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0 - tam * 1.25 - 4.0, &fig.texto, tam, negro, "center");
            }
            // Dentro, centrado, y que quepa.
            _ => {
                let letras = fig.texto.chars().count().max(1) as f64;
                let tam = fig.tam_letra.min(fig.ancho * 0.9 / (0.6 * letras)).max(8.0);
                let (cx, cy) = (fig.x + fig.ancho / 2.0, fig.y + fig.alto / 2.0);
                let color = if relleno.is_some_and(|r| es_oscuro(r)) { "#ffffff" } else { negro };
                f.texto(cx, cy - tam * 1.25 / 2.0, &fig.texto, tam, color, "center");
            }
        }
    }
    f.grupo = None;
}

/// Si un `#rrggbb` es oscuro (para escribir en blanco encima).
fn es_oscuro(hex: &str) -> bool {
    let h = hex.trim_start_matches('#');
    let c = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("ff"), 16).unwrap_or(255) as f64;
    0.299 * c(0) + 0.587 * c(2) + 0.114 * c(4) < 110.0
}

/// **Los elementos de un calcado** (`calcar::calcar`), todos en un grupo
/// para moverlos de una vez (se desagrupa para tocar una pieza). Lisos:
/// rugosidad 0, que un calco a mano alzada ya no se pareceria al original.
pub fn elementos_de_calcado(c: &crate::calcar::Calcado, origen: (f64, f64), semilla: u64) -> Vec<Value> {
    use crate::calcar::Trazo;
    let mut f = Fabrica {
        semilla,
        n: 0,
        ox: origen.0,
        oy: origen.1,
        salida: Vec::new(),
        grupo: Some(format!("ia-{semilla:x}-calco")),
    };
    for t in &c.trazos {
        match t {
            Trazo::Fondo { caja, color } => f.caja("rectangle", *caja, color, Some(color)),
            Trazo::Mancha { contorno, huecos, color } => f.region(contorno, huecos, color, color),
            Trazo::Linea { puntos, color, .. } => f.trazo("line", puntos, color, None),
        }
        if let Some(e) = f.salida.last_mut() {
            e["roughness"] = json!(0);
            if !matches!(t, Trazo::Linea { .. }) {
                // 2: el borde del mismo color tapa la rendija que deja el
                // simplificado entre dos manchas vecinas.
                e["strokeWidth"] = json!(2);
            }
        }
    }
    f.salida
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

    #[test]
    fn las_figuras_nuevas_las_lee_el_lienzo() {
        let l = llamada(
            "dibujar_en_lienzo",
            r#"{"elementos":[
                {"tipo":"triangulo","caja":[280,170,240,110],"color":"rojo","relleno":"rojo","grupo":"casa"},
                {"tipo":"rectangulo","caja":[300,280,200,160],"color":"marron","grupo":"casa"},
                {"tipo":"poligono","puntos":[10,10,60,10,35,50],"color":"verde"},
                {"tipo":"curva","puntos":[0,300,100,250,200,350,300,300],"color":"azul"},
                {"tipo":"rectangulo","caja":[60,260,160,70],"texto":"Inicio","relleno":"azul"},
                {"tipo":"flecha","puntos":[220,295,320,295],"texto":"si"}]}"#,
        );
        let els = elementos(&l, (0.0, 0.0), 5);
        let doc = documento(els.clone());
        let lienzo = pixpin_motor2d::excalidraw::leer(&doc).unwrap();
        assert_eq!(lienzo.cuantos_ajenos(), 0);
        let leidos = lienzo.elementos();
        // triangulo, casa, poligono, curva, caja + rotulo, flecha + rotulo.
        assert_eq!(leidos.len(), 8);
        assert!(matches!(&leidos[0].figura, FiguraLienzo::Region { contorno, .. } if contorno.len() == 3));
        assert!(leidos[0].relleno.is_some());
        assert_eq!(leidos[0].grupos, leidos[1].grupos);
        assert!(!leidos[0].grupos.is_empty());
        // Sin relleno: linea cerrada (vuelve al primer punto).
        assert!(matches!(&leidos[2].figura, FiguraLienzo::Linea { puntos } if puntos.len() == 4 && puntos[0] == puntos[3]));
        // La curva: linea redondeada.
        assert!(matches!(&leidos[3].figura, FiguraLienzo::Linea { puntos } if puntos.len() == 4));
        assert!(leidos[3].redondo);
        // El rotulo, en blanco sobre azul, centrado y agrupado con su caja.
        match &leidos[5].figura {
            FiguraLienzo::Texto { texto, .. } => assert_eq!(texto, "Inicio"),
            otra => panic!("{otra:?}"),
        }
        assert_eq!(els[5]["strokeColor"], "#ffffff");
        assert_eq!(leidos[4].grupos, leidos[5].grupos);
        assert!(matches!(&leidos[7].figura, FiguraLienzo::Texto { texto, .. } if texto == "si"));
    }

    #[test]
    fn un_calcado_lo_lee_el_lienzo_en_un_grupo() {
        let (w, h) = (300u32, 200u32);
        let mut px = vec![255u8; (w * h * 4) as usize];
        for y in 50..150 {
            for x in 50..150 {
                let i = ((y * w + x) * 4) as usize;
                px[i..i + 3].copy_from_slice(&[200, 20, 20]);
            }
        }
        // Un anillo azul (con su agujero).
        for y in 0..h {
            for x in 0..w {
                let d = ((x as f64 - 230.0).powi(2) + (y as f64 - 100.0).powi(2)).sqrt();
                if (25.0..=50.0).contains(&d) {
                    let i = ((y * w + x) * 4) as usize;
                    px[i..i + 3].copy_from_slice(&[20, 40, 200]);
                }
            }
        }
        for p in [crate::calcar::Parametros::default(), crate::calcar::Parametros { solo_contornos: true, ..Default::default() }] {
            let c = crate::calcar::calcar(&px, w, h, &p);
            let els = elementos_de_calcado(&c, (0.0, 0.0), 9);
            assert!(els.len() >= 2, "{p:?}: {}", els.len());
            let lienzo = pixpin_motor2d::excalidraw::leer(&documento(els)).unwrap();
            assert_eq!(lienzo.cuantos_ajenos(), 0);
            let leidos = lienzo.elementos();
            assert!(leidos.iter().all(|e| e.grupos == leidos[0].grupos && !e.grupos.is_empty()));
            if !p.solo_contornos {
                let anillo = leidos
                    .iter()
                    .find_map(|e| match &e.figura {
                        FiguraLienzo::Region { huecos, .. } if !huecos.is_empty() => Some(huecos.len()),
                        _ => None,
                    })
                    .expect("el anillo con su agujero");
                assert_eq!(anillo, 1);
            }
        }
    }
}
