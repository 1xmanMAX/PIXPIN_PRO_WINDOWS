//! Leer y escribir el JSON de Excalidraw, que es el formato en el que el
//! PixPin de Android guarda sus lienzos.
//!
//! Es el puente entre las dos mitades: capturas en el movil y sigues en el
//! escritorio. El `.pixpin` del Android es un ZIP con un `.excalidraw` por
//! hoja, en JSON plano — su propia documentacion dice que lo descomprime a
//! proposito «para que un editor de escritorio los lea sin mas».
//!
//! # Lo que NO se puede perder
//!
//! El Android tiene treinta y una herramientas y nosotros once. Un fichero
//! suyo trae cotas, mosaicos, solidos y cronogramas que aqui no sabemos ni
//! dibujar. **Esos elementos se guardan tal cual y vuelven a salir tal
//! cual**, en su sitio dentro del orden.
//!
//! Sin eso, abrir un plano en el escritorio y volver a guardarlo le borraria
//! las cotas al usuario. Un puente que pierde la mitad de la carga es peor
//! que no tener puente: al menos sin el, nadie confia en el.
//!
//! # Las tres diferencias de fondo
//!
//! - **Los puntos.** Los nuestros son ABSOLUTOS; los de Excalidraw van
//!   relativos al origen del elemento. Se suman al leer y se restan al
//!   escribir.
//! - **Los colores.** Los nuestros son cuatro numeros de cero a uno; los
//!   suyos, texto hexadecimal, con `"transparent"` como valor especial.
//! - **Los identificadores.** Los nuestros son numeros; los suyos, texto. El
//!   texto original se guarda para devolverlo intacto: cambiarselo romperia
//!   las ataduras entre flechas y figuras dentro del propio fichero.

use serde_json::{Map, Value};

use crate::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use crate::vector::Punto2;

#[derive(Debug, thiserror::Error)]
pub enum ErrorExcalidraw {
    #[error("el fichero no es JSON valido: {0}")]
    Json(#[from] serde_json::Error),
    #[error("el JSON no es un lienzo de Excalidraw: falta «elements» o no es una lista")]
    SinElementos,
}

/// Una entrada del lienzo, en su sitio dentro del orden de pintado.
///
/// El orden importa: en Excalidraw lo que va despues tapa a lo que va antes.
/// Separar lo conocido de lo ajeno en dos listas perderia ese entrelazado y
/// un mosaico dejaria de tapar lo que tapaba.
#[derive(Debug, Clone, PartialEq)]
pub enum Entrada {
    /// Un elemento que sabemos representar. Se guarda tambien su JSON
    /// original para devolver intactos los campos que no usamos.
    Nuestro {
        elemento: Elemento,
        original: Box<Value>,
    },
    /// Un elemento que no sabemos representar: viaja tal cual.
    Ajeno(Box<Value>),
}

/// Un lienzo leido.
#[derive(Debug, Clone)]
pub struct Lienzo {
    pub entradas: Vec<Entrada>,
    /// Todo lo demas del fichero — `type`, `version`, `appState`, `files` —
    /// tal cual venia. Se devuelve sin tocar al escribir.
    pub resto: Map<String, Value>,
}

impl Lienzo {
    /// Los elementos que sabemos dibujar, en orden.
    pub fn elementos(&self) -> Vec<Elemento> {
        self.entradas
            .iter()
            .filter_map(|e| match e {
                Entrada::Nuestro { elemento, .. } => Some(elemento.clone()),
                Entrada::Ajeno(_) => None,
            })
            .collect()
    }

    /// Cuantos elementos no supimos representar.
    ///
    /// Es el numero que hay que ensenarle al usuario: «este plano trae siete
    /// cosas que aqui no se pueden editar, pero no se van a perder».
    pub fn cuantos_ajenos(&self) -> usize {
        self.entradas
            .iter()
            .filter(|e| matches!(e, Entrada::Ajeno(_)))
            .count()
    }
}

/// Lee un lienzo.
pub fn leer(json: &str) -> Result<Lienzo, ErrorExcalidraw> {
    let raiz: Value = serde_json::from_str(json)?;
    let Value::Object(mut mapa) = raiz else {
        return Err(ErrorExcalidraw::SinElementos);
    };
    let Some(Value::Array(lista)) = mapa.remove("elements") else {
        return Err(ErrorExcalidraw::SinElementos);
    };
    let entradas = lista
        .into_iter()
        .map(|v| match elemento_desde(&v) {
            Some(elemento) => Entrada::Nuestro {
                elemento,
                original: Box::new(v),
            },
            None => Entrada::Ajeno(Box::new(v)),
        })
        .collect();
    Ok(Lienzo {
        entradas,
        resto: mapa,
    })
}

/// Escribe un lienzo, devolviendo lo ajeno intacto y en su sitio.
pub fn escribir(lienzo: &Lienzo) -> String {
    let elementos: Vec<Value> = lienzo
        .entradas
        .iter()
        .map(|e| match e {
            Entrada::Nuestro { elemento, original } => elemento_hacia(elemento, original),
            Entrada::Ajeno(v) => (**v).clone(),
        })
        .collect();
    let mut mapa = lienzo.resto.clone();
    mapa.insert("elements".into(), Value::Array(elementos));
    // Un lienzo que salga de aqui tiene que declararse como lo que es,
    // aunque el que entro no lo hiciera.
    mapa.entry("type")
        .or_insert_with(|| Value::String("excalidraw".into()));
    serde_json::to_string_pretty(&Value::Object(mapa)).unwrap_or_default()
}

// --- Traduccion de un elemento ---

fn num(v: &Value, clave: &str) -> Option<f32> {
    v.get(clave).and_then(|x| x.as_f64()).map(|x| x as f32)
}

fn num_o(v: &Value, clave: &str, si_no: f32) -> f32 {
    num(v, clave).unwrap_or(si_no)
}

/// Los puntos de un trazo, pasados de relativos a ABSOLUTOS.
fn puntos_desde(v: &Value, x: f32, y: f32) -> Vec<Punto2> {
    v.get("points")
        .and_then(|p| p.as_array())
        .map(|lista| {
            lista
                .iter()
                .filter_map(|p| {
                    let par = p.as_array()?;
                    Some(Punto2 {
                        x: x + par.first()?.as_f64()? as f32,
                        y: y + par.get(1)?.as_f64()? as f32,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Y al reves, de absolutos a relativos.
fn puntos_hacia(puntos: &[Punto2], x: f32, y: f32) -> Value {
    Value::Array(
        puntos
            .iter()
            .map(|p| {
                Value::Array(vec![
                    Value::from((p.x - x) as f64),
                    Value::from((p.y - y) as f64),
                ])
            })
            .collect(),
    )
}

/// Traduce un elemento de Excalidraw al nuestro. `None` si no sabemos que es.
fn elemento_desde(v: &Value) -> Option<Elemento> {
    let tipo = v.get("type")?.as_str()?;
    // Lo borrado en Excalidraw sigue en el fichero con `isDeleted`. Se salta:
    // no es un elemento ajeno que haya que conservar, es basura que el propio
    // formato marca como tal.
    if v.get("isDeleted").and_then(|b| b.as_bool()) == Some(true) {
        return None;
    }
    let x = num_o(v, "x", 0.0);
    let y = num_o(v, "y", 0.0);
    let figura = match tipo {
        "rectangle" => Figura::Rectangulo,
        "ellipse" => Figura::Elipse,
        "line" => Figura::Linea {
            puntos: puntos_desde(v, x, y),
        },
        "arrow" => Figura::Flecha {
            puntos: puntos_desde(v, x, y),
            punta_inicio: v.get("startArrowhead").is_some_and(|a| !a.is_null()),
            punta_fin: v.get("endArrowhead").map(|a| !a.is_null()).unwrap_or(true),
        },
        "freedraw" => Figura::Lapiz {
            puntos: puntos_desde(v, x, y),
            presiones: v
                .get("pressures")
                .and_then(|p| p.as_array())
                .map(|l| {
                    l.iter()
                        .filter_map(|p| p.as_f64())
                        .map(|p| p as f32)
                        .collect()
                })
                .unwrap_or_default(),
        },
        "text" => Figura::Texto {
            texto: v.get("text").and_then(|t| t.as_str()).unwrap_or("").into(),
            tam: num_o(v, "fontSize", 20.0),
            familia: "Segoe UI".into(),
        },
        // El resto son suyos y no sabemos dibujarlos: `pixpin-mosaic`,
        // `pixpin-measure`, `pixpin-solid`, `pixpin-gantt`... y tambien
        // `diamond` e `image`, que son de Excalidraw pero todavia no
        // tenemos. Se conservan como ajenos.
        _ => return None,
    };
    Some(Elemento {
        // El identificador de texto no cabe en el nuestro. Se guarda uno
        // derivado para que sea estable dentro de la sesion, y el original
        // vuelve del JSON al escribir.
        id: id_estable(v.get("id").and_then(|i| i.as_str()).unwrap_or("")),
        figura,
        x,
        y,
        ancho: num_o(v, "width", 0.0),
        alto: num_o(v, "height", 0.0),
        angulo: num_o(v, "angle", 0.0),
        trazo: color_desde(v.get("strokeColor")).unwrap_or(ColorRgba::opaco(0.1, 0.1, 0.1)),
        relleno: color_desde(v.get("backgroundColor")),
        grosor: num_o(v, "strokeWidth", 2.0),
        estilo: match v.get("strokeStyle").and_then(|s| s.as_str()) {
            Some("dashed") => EstiloTrazo::Discontinuo,
            Some("dotted") => EstiloTrazo::Punteado,
            _ => EstiloTrazo::Solido,
        },
        rugosidad: num_o(v, "roughness", 1.0),
        // El suyo va de 0 a 100 y el nuestro de 0 a 1.
        opacidad: (num_o(v, "opacity", 100.0) / 100.0).clamp(0.0, 1.0),
        semilla: v
            .get("seed")
            .and_then(|s| s.as_u64())
            .map(|s| s as u32)
            .unwrap_or(1),
        version: v
            .get("version")
            .and_then(|s| s.as_u64())
            .map(|s| s as u32)
            .unwrap_or(1),
        borrado: false,
    })
}

/// Devuelve el elemento al JSON, encima del original.
///
/// Encima y no de cero: el original trae campos que no usamos —`groupIds`,
/// `boundElements`, `link`, `frameId`— y que atan unos elementos con otros.
/// Escribir solo lo que entendemos desharia esas ataduras en silencio.
fn elemento_hacia(e: &Elemento, original: &Value) -> Value {
    let mut mapa = match original {
        Value::Object(m) => m.clone(),
        _ => Map::new(),
    };
    mapa.insert("x".into(), Value::from(e.x as f64));
    mapa.insert("y".into(), Value::from(e.y as f64));
    mapa.insert("width".into(), Value::from(e.ancho as f64));
    mapa.insert("height".into(), Value::from(e.alto as f64));
    mapa.insert("angle".into(), Value::from(e.angulo as f64));
    mapa.insert("strokeColor".into(), Value::String(color_hacia(e.trazo)));
    mapa.insert(
        "backgroundColor".into(),
        Value::String(match e.relleno {
            Some(c) => color_hacia(c),
            None => "transparent".into(),
        }),
    );
    mapa.insert("strokeWidth".into(), Value::from(e.grosor as f64));
    mapa.insert(
        "strokeStyle".into(),
        Value::String(
            match e.estilo {
                EstiloTrazo::Solido => "solid",
                EstiloTrazo::Discontinuo => "dashed",
                EstiloTrazo::Punteado => "dotted",
            }
            .into(),
        ),
    );
    mapa.insert("roughness".into(), Value::from(e.rugosidad as f64));
    mapa.insert(
        "opacity".into(),
        Value::from((e.opacidad * 100.0).round() as i64),
    );
    mapa.insert("seed".into(), Value::from(e.semilla));
    mapa.insert("isDeleted".into(), Value::Bool(e.borrado));
    match &e.figura {
        Figura::Lapiz { puntos, presiones } => {
            mapa.insert("points".into(), puntos_hacia(puntos, e.x, e.y));
            if !presiones.is_empty() {
                mapa.insert(
                    "pressures".into(),
                    Value::Array(presiones.iter().map(|p| Value::from(*p as f64)).collect()),
                );
            }
        }
        Figura::Resaltador { puntos } | Figura::Linea { puntos } => {
            mapa.insert("points".into(), puntos_hacia(puntos, e.x, e.y));
        }
        Figura::Flecha { puntos, .. } => {
            mapa.insert("points".into(), puntos_hacia(puntos, e.x, e.y));
        }
        Figura::Texto { texto, tam, .. } => {
            mapa.insert("text".into(), Value::String(texto.clone()));
            mapa.insert("fontSize".into(), Value::from(*tam as f64));
        }
        Figura::Rectangulo | Figura::Elipse | Figura::Foco { .. } | Figura::Imagen { .. } => {}
    }
    Value::Object(mapa)
}

/// Un numero estable a partir del identificador de texto.
///
/// Los suyos son cadenas y los nuestros numeros. No hace falta que sea
/// reversible —el original vuelve del JSON— pero SI que el mismo texto de
/// siempre el mismo numero: si cambiara entre dos lecturas del mismo
/// fichero, deshacer y seleccionar dejarian de encontrar sus elementos.
fn id_estable(texto: &str) -> u64 {
    // FNV-1a de 64 bits. Cabe en cuatro lineas, no necesita dependencias y
    // reparte bien para lo que hace falta aqui, que es no chocar dentro de
    // un dibujo de unos cientos de elementos.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in texto.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// --- Colores ---

/// Lee un color hexadecimal de Excalidraw.
///
/// `None` para `"transparent"`, que es como dice «sin relleno». Devolver
/// negro transparente en su lugar pintaria una caja invisible que si
/// responde al raton.
pub fn color_desde(v: Option<&Value>) -> Option<ColorRgba> {
    let texto = v?.as_str()?.trim();
    if texto.eq_ignore_ascii_case("transparent") || texto.is_empty() {
        return None;
    }
    let h = texto.strip_prefix('#').unwrap_or(texto);
    let canal = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    let corto = |i: usize| {
        let c = u8::from_str_radix(h.get(i..i + 1)?, 16).ok()?;
        // El corto duplica el digito: #f0a es #ff00aa, no #f00a00.
        Some(c * 17)
    };
    let (r, g, b, a) = match h.len() {
        3 => (corto(0)?, corto(1)?, corto(2)?, 255),
        6 => (canal(0)?, canal(2)?, canal(4)?, 255),
        8 => (canal(0)?, canal(2)?, canal(4)?, canal(6)?),
        _ => return None,
    };
    Some(ColorRgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: a as f32 / 255.0,
    })
}

/// Y al reves. Sin alfa si es opaco: es lo que escribe Excalidraw, y un
/// `#ff0000ff` donde se esperaba `#ff0000` ensucia la comparacion de dos
/// ficheros que deberian ser iguales.
pub fn color_hacia(c: ColorRgba) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    if c.a >= 1.0 {
        format!("#{:02x}{:02x}{:02x}", byte(c.r), byte(c.g), byte(c.b))
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            byte(c.r),
            byte(c.g),
            byte(c.b),
            byte(c.a)
        )
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un lienzo con un rectangulo nuestro y una cota suya, en ese orden.
    fn lienzo_mixto() -> &'static str {
        r##"{
          "type": "excalidraw",
          "version": 2,
          "source": "pixpin-android",
          "elements": [
            {"id":"a1","type":"rectangle","x":10,"y":20,"width":100,"height":50,
             "strokeColor":"#1e1e1e","backgroundColor":"transparent","strokeWidth":2,
             "strokeStyle":"solid","roughness":1,"opacity":100,"seed":12345,
             "groupIds":["g1"],"boundElements":[{"id":"b1","type":"arrow"}]},
            {"id":"c1","type":"pixpin-measure","x":0,"y":0,"width":80,"height":10,
             "medida":4.5,"unidad":"m"}
          ],
          "appState": {"viewBackgroundColor": "#ffffff"},
          "files": {}
        }"##
    }

    #[test]
    fn lo_que_entendemos_se_traduce() {
        let l = leer(lienzo_mixto()).unwrap();
        let e = l.elementos();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].figura, Figura::Rectangulo);
        assert_eq!(
            (e[0].x, e[0].y, e[0].ancho, e[0].alto),
            (10.0, 20.0, 100.0, 50.0)
        );
        assert_eq!(e[0].opacidad, 1.0, "el suyo va de 0 a 100");
        assert_eq!(e[0].relleno, None, "«transparent» es sin relleno");
        assert_eq!(e[0].semilla, 12345);
    }

    #[test]
    fn lo_que_no_entendemos_no_se_pierde() {
        // Es la razon de ser de este modulo. Si esto fallara, abrir un plano
        // en el escritorio y guardarlo le borraria las cotas al usuario.
        let l = leer(lienzo_mixto()).unwrap();
        assert_eq!(l.cuantos_ajenos(), 1);
        let salida = escribir(&l);
        assert!(
            salida.contains("pixpin-measure"),
            "se perdio la cota:\n{salida}"
        );
        assert!(salida.contains("\"medida\""), "se perdieron sus campos");
        assert!(salida.contains("\"unidad\""));
    }

    #[test]
    fn el_orden_se_respeta() {
        // Lo que va despues tapa a lo que va antes. Separar lo conocido de
        // lo ajeno en dos listas perderia el entrelazado y un mosaico
        // dejaria de tapar lo que tapaba.
        let l = leer(lienzo_mixto()).unwrap();
        assert!(matches!(l.entradas[0], Entrada::Nuestro { .. }));
        assert!(matches!(l.entradas[1], Entrada::Ajeno(_)));
        let salida = escribir(&l);
        let pos_rect = salida.find("rectangle").unwrap();
        let pos_cota = salida.find("pixpin-measure").unwrap();
        assert!(pos_rect < pos_cota, "el orden cambio");
    }

    #[test]
    fn los_campos_que_no_usamos_vuelven_intactos() {
        // `groupIds` y `boundElements` atan unos elementos con otros.
        // Escribir solo lo que entendemos las desharia en silencio.
        let salida = escribir(&leer(lienzo_mixto()).unwrap());
        assert!(salida.contains("groupIds"), "se perdieron los grupos");
        assert!(
            salida.contains("boundElements"),
            "se perdieron las ataduras"
        );
        assert!(
            salida.contains("viewBackgroundColor"),
            "se perdio el appState"
        );
    }

    #[test]
    fn los_puntos_pasan_de_relativos_a_absolutos_y_vuelven() {
        // Los suyos van relativos al origen del elemento y los nuestros son
        // absolutos. Sin la suma, un trazo dibujado en el movil aparece
        // pegado a la esquina al abrirlo aqui.
        let json = r#"{"elements":[
          {"id":"t","type":"freedraw","x":100,"y":200,"width":10,"height":10,
           "points":[[0,0],[5,5],[10,10]]}
        ]}"#;
        let l = leer(json).unwrap();
        let Figura::Lapiz { puntos, .. } = &l.elementos()[0].figura else {
            panic!("deberia ser lapiz");
        };
        assert_eq!(puntos[0], Punto2 { x: 100.0, y: 200.0 });
        assert_eq!(puntos[2], Punto2 { x: 110.0, y: 210.0 });
        // Y al escribir vuelven a ser relativos: el primero en el origen.
        let vuelta = leer(&escribir(&l)).unwrap();
        let Figura::Lapiz { puntos: p2, .. } = &vuelta.elementos()[0].figura else {
            panic!("deberia seguir siendo lapiz");
        };
        assert_eq!(p2, puntos, "la ida y vuelta movio el trazo");
    }

    #[test]
    fn los_colores_van_y_vuelven() {
        let rojo = color_desde(Some(&Value::String("#ff0000".into()))).unwrap();
        assert_eq!((rojo.r, rojo.g, rojo.b, rojo.a), (1.0, 0.0, 0.0, 1.0));
        assert_eq!(color_hacia(rojo), "#ff0000");
        // El corto duplica el digito: #f0a es #ff00aa.
        let corto = color_desde(Some(&Value::String("#f0a".into()))).unwrap();
        assert_eq!(color_hacia(corto), "#ff00aa");
        // Con alfa, ocho digitos.
        let medio = color_desde(Some(&Value::String("#00ff0080".into()))).unwrap();
        assert_eq!(color_hacia(medio), "#00ff0080");
    }

    #[test]
    fn transparente_es_sin_relleno_y_no_negro_invisible() {
        // Caso negativo: devolver negro con alfa cero pintaria una caja
        // invisible que ademas responde al raton.
        assert_eq!(
            color_desde(Some(&Value::String("transparent".into()))),
            None
        );
        assert_eq!(color_desde(Some(&Value::String("".into()))), None);
        assert_eq!(color_desde(None), None);
        // Y un color mal escrito tampoco inventa nada.
        assert_eq!(color_desde(Some(&Value::String("#zzz".into()))), None);
        assert_eq!(color_desde(Some(&Value::String("#12345".into()))), None);
    }

    #[test]
    fn lo_borrado_no_vuelve_a_la_vida() {
        // Excalidraw deja lo borrado dentro del fichero con `isDeleted`. No
        // es un elemento ajeno que haya que conservar: es basura que el
        // propio formato marca como tal.
        let json = r#"{"elements":[
          {"id":"x","type":"rectangle","x":0,"y":0,"width":1,"height":1,"isDeleted":true}
        ]}"#;
        let l = leer(json).unwrap();
        assert!(l.elementos().is_empty());
    }

    #[test]
    fn el_identificador_de_texto_da_siempre_el_mismo_numero() {
        // Si cambiara entre dos lecturas del mismo fichero, deshacer y
        // seleccionar dejarian de encontrar sus elementos.
        assert_eq!(id_estable("abc123"), id_estable("abc123"));
        assert_ne!(id_estable("abc123"), id_estable("abc124"));
        assert_ne!(id_estable(""), id_estable("a"));
    }

    #[test]
    fn un_json_que_no_es_un_lienzo_se_rechaza() {
        // Caso negativo: sin esto, arrastrar un fichero cualquiera daria un
        // lienzo vacio y pareceria que el dibujo se perdio.
        assert!(matches!(leer("no soy json"), Err(ErrorExcalidraw::Json(_))));
        assert!(matches!(
            leer("[1,2,3]"),
            Err(ErrorExcalidraw::SinElementos)
        ));
        assert!(matches!(
            leer(r#"{"type":"excalidraw"}"#),
            Err(ErrorExcalidraw::SinElementos)
        ));
        // Pero un lienzo vacio SI es valido: es un dibujo sin nada.
        assert_eq!(leer(r#"{"elements":[]}"#).unwrap().entradas.len(), 0);
    }

    #[test]
    fn la_flecha_conserva_sus_puntas() {
        let json = r#"{"elements":[
          {"id":"f","type":"arrow","x":0,"y":0,"width":10,"height":0,
           "points":[[0,0],[10,0]],"startArrowhead":null,"endArrowhead":"arrow"}
        ]}"#;
        let l = leer(json).unwrap();
        let Figura::Flecha {
            punta_inicio,
            punta_fin,
            ..
        } = &l.elementos()[0].figura
        else {
            panic!("deberia ser flecha");
        };
        assert!(!punta_inicio, "no llevaba punta al principio");
        assert!(punta_fin);
    }
}
