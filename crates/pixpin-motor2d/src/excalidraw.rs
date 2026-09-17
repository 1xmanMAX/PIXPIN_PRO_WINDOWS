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
use crate::medida::Escala;
use crate::relleno::EstiloRelleno;
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
    /// Que mide un pixel de este lienzo (D31). Vive al nivel del lienzo, no
    /// en `resto`, porque `leer`/`escribir` la traducen a su propio tipo en
    /// vez de dejarla como JSON crudo.
    pub escala: Option<Escala>,
    /// La clave `escala` cruda del JSON, cuando no supimos entenderla.
    ///
    /// `escala` es `None` tanto si no habia ninguna como si la habia y era
    /// invalida — eso esta bien para medir, pero no para escribir: «no la
    /// entiendo» y «no la hay» no pueden representarse igual, o una escala
    /// corrupta de una version futura del movil se destruye en silencio al
    /// pasar por Windows. Privado: solo lo usan `leer` y `escribir`.
    escala_no_entendida: Option<Value>,
}

impl Lienzo {
    /// Un lienzo recien creado, sin nada dentro.
    ///
    /// Hace falta porque `escala_no_entendida` es privado y desde fuera de
    /// este modulo no se puede escribir un `Lienzo` entero a mano. Lo demas
    /// —`type`, `version`, `appState`— lo pone `escribir`.
    pub fn vacio() -> Lienzo {
        Lienzo {
            entradas: Vec::new(),
            resto: Map::new(),
            escala: None,
            escala_no_entendida: None,
        }
    }

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
    let escala_cruda = mapa.remove("escala");
    let escala = escala_cruda.as_ref().and_then(escala_desde);
    // Si habia una clave y no supimos entenderla, se guarda tal cual para
    // devolverla intacta al escribir en vez de pisarla con null.
    let escala_no_entendida = match (&escala_cruda, &escala) {
        (Some(v), None) => Some(v.clone()),
        _ => None,
    };
    Ok(Lienzo {
        entradas,
        resto: mapa,
        escala,
        escala_no_entendida,
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
    // Se escribe siempre, tambien cuando es None: si solo se escribiera
    // cuando hay escala, la del original sobreviviria y el movil seguiria
    // midiendo con una escala que aqui se borro. Pero None no siempre
    // significa «no hay»: si habia una clave y no se entendio, se devuelve
    // tal cual en vez de null, para no destruir en silencio una escala
    // corrupta o de una version futura del movil.
    match (&lienzo.escala, &lienzo.escala_no_entendida) {
        (Some(e), _) => mapa.insert("escala".into(), escala_hacia(e)),
        (None, Some(cruda)) => mapa.insert("escala".into(), cruda.clone()),
        (None, None) => mapa.insert("escala".into(), Value::Null),
    };
    serde_json::to_string_pretty(&Value::Object(mapa)).unwrap_or_default()
}

/// La escala del JSON del movil, o `None` si no la hay o no vale.
///
/// Una escala imposible se ignora en vez de adoptarse (D36): mas vale no
/// medir que medir mal.
fn escala_desde(v: &Value) -> Option<Escala> {
    let upp = v.get("unidadesPorPixel")?.as_f64()? as f32;
    if !upp.is_finite() || upp <= 0.0 {
        return None;
    }
    Some(Escala {
        unidades_por_pixel: upp,
        unidad: v
            .get("unidad")
            .and_then(Value::as_str)
            .unwrap_or("m")
            .to_string(),
        // El mismo tope que `Escala::calibrando` (medida.rs): sin el, un
        // "decimales" disparatado del movil entraria tal cual en el tipo, y
        // solo se veria recortado de casualidad porque `formatear_valor`
        // vuelve a recortar a 6 al pintar.
        decimales: (v.get("decimales").and_then(Value::as_u64).unwrap_or(2) as u8).min(6),
    })
}

fn escala_hacia(e: &Escala) -> Value {
    let mut m = Map::new();
    m.insert(
        "unidadesPorPixel".into(),
        Value::from(e.unidades_por_pixel as f64),
    );
    m.insert("unidad".into(), Value::String(e.unidad.clone()));
    m.insert("decimales".into(), Value::from(e.decimales));
    Value::Object(m)
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

/// El `fillStyle` de Excalidraw, con sus tres palabras exactas.
///
/// Cualquier otra cosa —que no venga, que venga vacia, o que sea un estilo de
/// una version futura del movil— cae en el de por omision. No se rechaza el
/// elemento entero por esto: un relleno que no sabemos pintar no es motivo
/// para que un rectangulo deje de ser un rectangulo.
fn estilo_relleno_desde(v: Option<&Value>) -> EstiloRelleno {
    match v.and_then(Value::as_str) {
        Some("solid") => EstiloRelleno::Solido,
        Some("cross-hatch") => EstiloRelleno::Cruzado,
        _ => EstiloRelleno::Rayado,
    }
}

fn estilo_relleno_hacia(e: EstiloRelleno) -> &'static str {
    match e {
        EstiloRelleno::Solido => "solid",
        EstiloRelleno::Rayado => "hachure",
        EstiloRelleno::Cruzado => "cross-hatch",
    }
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
        // El marco de Excalidraw. Sus hijos llevan alli un `frameId`; aqui la
        // pertenencia se mira por la caja (ver `marco.rs`), asi que ese campo
        // se queda en `resto` y vuelve tal cual al guardar.
        "frame" => Figura::Marco {
            nombre: v
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or_default()
                .to_string(),
        },
        "ellipse" => Figura::Elipse,
        "line" => Figura::Linea {
            puntos: puntos_desde(v, x, y),
        },
        "arrow" => Figura::Flecha {
            puntos: puntos_desde(v, x, y),
            punta_inicio: v.get("startArrowhead").is_some_and(|a| !a.is_null()),
            punta_fin: v.get("endArrowhead").map(|a| !a.is_null()).unwrap_or(true),
        },
        "freedraw" => {
            // `simulatePressure: true` manda sobre `pressures`: Excalidraw las
            // ignora al pintar aunque vengan en el fichero.
            let simulada = v.get("simulatePressure").and_then(|b| b.as_bool()) == Some(true);
            Figura::Lapiz {
                puntos: puntos_desde(v, x, y),
                presiones: if simulada {
                    Vec::new()
                } else {
                    v.get("pressures")
                        .and_then(|p| p.as_array())
                        .map(|l| {
                            l.iter()
                                .filter_map(|p| p.as_f64())
                                .map(|p| p as f32)
                                .collect()
                        })
                        .unwrap_or_default()
                },
                // Todo freedraw de Excalidraw lleva el grosor en `strokeWidth`,
                // con o sin `strokeOptions`: nunca es legado.
                opciones: Some(
                    v.get("strokeOptions")
                        .and_then(|s| serde_json::from_value(s.clone()).ok())
                        .unwrap_or_default(),
                ),
            }
        }
        "text" => Figura::Texto {
            texto: v.get("text").and_then(|t| t.as_str()).unwrap_or("").into(),
            tam: num_o(v, "fontSize", 20.0),
            familia: "Segoe UI".into(),
        },
        "pixpin-measure" => {
            let puntos = puntos_desde(v, x, y);
            // Sin dos puntos no hay raya que dibujar ni que tocar
            // (pintado.rs y impacto.rs exigen `len() >= 2`): seria una cota
            // invisible e inseleccionable que se guarda para siempre. Mejor
            // que caiga al carril ajeno, que es donde sobrevivia intacta
            // antes de que aprendieramos a leer `pixpin-measure`.
            if puntos.len() < 2 {
                return None;
            }
            Figura::Cota { puntos }
        }
        "pixpin-scalebar" => Figura::EscalaGrafica,
        // El resto son suyos y no sabemos dibujarlos: `pixpin-mosaic`,
        // `pixpin-solid`, `pixpin-gantt`... y tambien `diamond` e `image`,
        // que son de Excalidraw pero todavia no tenemos. Se conservan como
        // ajenos.
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
        estilo_relleno: estilo_relleno_desde(v.get("fillStyle")),
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
        bloqueado: v.get("locked").and_then(Value::as_bool).unwrap_or(false),
        grupos: v
            .get("groupIds")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|g| g.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
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
    // Un `fillStyle` que no entendemos se lee como el de por omision, asi que
    // reescribirlo lo destruiria: un estilo de una version futura del movil
    // entraria como «hachure» y saldria como «hachure» para siempre. Mientras
    // el estilo siga siendo el de por omision -o sea, mientras aqui no se haya
    // tocado- se devuelve el original tal cual, la misma regla que ya protege
    // a la escala y a los elementos ajenos.
    let sin_entender = mapa
        .get("fillStyle")
        .and_then(Value::as_str)
        .is_some_and(|s| !matches!(s, "solid" | "hachure" | "cross-hatch"));
    if !(sin_entender && e.estilo_relleno == EstiloRelleno::default()) {
        mapa.insert(
            "fillStyle".into(),
            Value::String(estilo_relleno_hacia(e.estilo_relleno).into()),
        );
    }
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
    mapa.insert("locked".into(), Value::Bool(e.bloqueado));
    // Se escribe siempre, tambien vacio: si solo se escribiera cuando hay
    // grupos, desagrupar en Windows dejaria los groupIds viejos del
    // original y el movil los volveria a ver agrupados.
    mapa.insert(
        "groupIds".into(),
        Value::Array(e.grupos.iter().map(|g| Value::String(g.clone())).collect()),
    );
    match &e.figura {
        Figura::Lapiz {
            puntos,
            presiones,
            opciones,
        } => {
            mapa.insert("points".into(), puntos_hacia(puntos, e.x, e.y));
            mapa.insert(
                "pressures".into(),
                Value::Array(presiones.iter().map(|p| Value::from(*p as f64)).collect()),
            );
            mapa.insert("simulatePressure".into(), Value::Bool(presiones.is_empty()));
            let o = opciones.unwrap_or_default();
            mapa.insert(
                "strokeOptions".into(),
                serde_json::to_value(o).unwrap_or(Value::Null),
            );
            // Un trazo legado se exporta con su grosor ya convertido, para
            // que Excalidraw y el movil lo vean del mismo ancho que aqui.
            if opciones.is_none() {
                mapa.insert(
                    "strokeWidth".into(),
                    Value::from((e.grosor / crate::tinta::FACTOR_VARIABLE) as f64),
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
        Figura::Cota { puntos } => {
            mapa.insert("type".into(), Value::String("pixpin-measure".to_string()));
            mapa.insert("points".into(), puntos_hacia(puntos, e.x, e.y));
        }
        Figura::EscalaGrafica => {
            mapa.insert("type".into(), Value::String("pixpin-scalebar".to_string()));
        }
        Figura::Marco { nombre } => {
            mapa.insert("type".into(), Value::String("frame".to_string()));
            mapa.insert("name".into(), Value::String(nombre.clone()));
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
    use crate::medida::Escala;

    /// Un lienzo con un rectangulo nuestro y un cronograma suyo, en ese orden.
    ///
    /// El tipo ajeno de muestra tiene que ser uno que no vayamos a
    /// implementar nunca: `pixpin-measure` sirvio para esto hasta que la
    /// tarea 3 le enseno a `elemento_desde` a entenderlo, y estas mismas
    /// pruebas se pusieron rojas por casualidad, no por ningun fallo. El
    /// cronograma no esta en ninguna fase del plan, asi que es el candidato
    /// con menos riesgo de que vuelva a pasar.
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
            {"id":"c1","type":"pixpin-gantt","x":0,"y":0,"width":80,"height":10,
             "tareas":[],"periodos":[]}
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
            salida.contains("pixpin-gantt"),
            "se perdio el cronograma:\n{salida}"
        );
        assert!(salida.contains("\"tareas\""), "se perdieron sus campos");
        assert!(salida.contains("\"periodos\""));
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
        let pos_cronograma = salida.find("pixpin-gantt").unwrap();
        assert!(pos_rect < pos_cronograma, "el orden cambio");
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
    fn un_lienzo_vacio_se_escribe_y_se_vuelve_a_leer() {
        let json = escribir(&Lienzo::vacio());
        let leido = leer(&json).expect("lo que escribimos tiene que poder leerse");
        assert!(leido.entradas.is_empty());
        assert_eq!(leido.cuantos_ajenos(), 0);
        // Y se declara como lo que es, o el movil no lo abrira.
        let mapa: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(mapa["type"], "excalidraw");
        // Caso negativo: sin `elements` no hay lienzo que leer, por bien que
        // este el resto del JSON.
        assert!(leer(r#"{"type":"excalidraw"}"#).is_err());
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
    fn las_tres_palabras_del_fill_style_se_leen_y_se_escriben() {
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "backgroundColor":"#ffcc00","fillStyle":"cross-hatch","seed":1},
            {"type":"rectangle","x":20,"y":0,"width":10,"height":10,
             "backgroundColor":"#ffcc00","fillStyle":"solid","seed":2},
            {"type":"rectangle","x":40,"y":0,"width":10,"height":10,
             "backgroundColor":"#ffcc00","fillStyle":"hachure","seed":3}
        ]}"##;
        let l = leer(json).unwrap();
        let e = l.elementos();
        assert_eq!(e[0].estilo_relleno, EstiloRelleno::Cruzado);
        assert_eq!(e[1].estilo_relleno, EstiloRelleno::Solido);
        assert_eq!(e[2].estilo_relleno, EstiloRelleno::Rayado);

        let vuelta = escribir(&l);
        assert!(vuelta.contains("\"cross-hatch\""), "{vuelta}");
        assert!(vuelta.contains("\"solid\""));
        assert!(vuelta.contains("\"hachure\""));
        let estilos: Vec<EstiloRelleno> = leer(&vuelta)
            .unwrap()
            .elementos()
            .iter()
            .map(|x| x.estilo_relleno)
            .collect();
        assert_eq!(
            estilos,
            e.iter().map(|x| x.estilo_relleno).collect::<Vec<_>>(),
            "la ida y vuelta cambio algun estilo de relleno"
        );
    }

    #[test]
    fn un_elemento_sin_fill_style_se_lee_rayado_como_en_excalidraw() {
        // Su `currentItemFillStyle` es «hachure»: si aqui cayera en solido, el
        // mismo fichero se veria rayado en el movil y como una mancha aqui.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "backgroundColor":"#ffcc00","seed":1}
        ]}"##;
        assert_eq!(
            leer(json).unwrap().elementos()[0].estilo_relleno,
            EstiloRelleno::Rayado
        );
    }

    #[test]
    fn un_fill_style_desconocido_no_rompe_la_lectura_ni_se_pierde_al_escribir() {
        // Caso negativo: un estilo de una version futura del movil no puede
        // tumbar el elemento entero -un relleno raro no deja de ser un
        // rectangulo- ni desaparecer al pasar por Windows, que es la misma
        // regla que protege a la escala y a los elementos ajenos.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "backgroundColor":"#ffcc00","fillStyle":"zigzag-del-futuro","seed":1}
        ]}"##;
        let mut l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0, "sigue siendo un rectangulo nuestro");
        assert_eq!(
            l.elementos()[0].estilo_relleno,
            EstiloRelleno::Rayado,
            "lo que no se entiende cae en el de por omision"
        );
        assert!(
            escribir(&l).contains("zigzag-del-futuro"),
            "el estilo que no entendemos tiene que volver intacto"
        );

        // Pero en cuanto se cambia aqui, manda lo de aqui: si no, el estilo
        // elegido en Windows no llegaria nunca al movil.
        primero_mut(&mut l).estilo_relleno = EstiloRelleno::Solido;
        let vuelta = escribir(&l);
        assert!(!vuelta.contains("zigzag-del-futuro"), "{vuelta}");
        assert_eq!(
            leer(&vuelta).unwrap().elementos()[0].estilo_relleno,
            EstiloRelleno::Solido
        );
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

    /// El primer elemento nuestro del lienzo, para poder tocarlo.
    ///
    /// Va por `entradas` y no por `elementos()`, que devuelve clones y no
    /// serviria para modificar nada.
    fn primero_mut(l: &mut Lienzo) -> &mut Elemento {
        l.entradas
            .iter_mut()
            .find_map(|e| match e {
                Entrada::Nuestro { elemento, .. } => Some(elemento),
                Entrada::Ajeno(_) => None,
            })
            .expect("el lienzo trae al menos un elemento nuestro")
    }

    #[test]
    fn un_marco_va_y_vuelve_con_su_nombre() {
        let texto = r##"{"type":"excalidraw","elements":[
            {"id":"f1","type":"frame","x":0,"y":0,"width":300,"height":200,
             "name":"Lamina 1","strokeColor":"#000000","frameId":null}
        ]}"##;
        let lienzo = leer(texto).unwrap();
        let elementos = lienzo.elementos();
        assert_eq!(elementos.len(), 1);
        assert!(
            matches!(&elementos[0].figura, Figura::Marco { nombre } if nombre == "Lamina 1"),
            "no se leyo como marco: {:?}",
            elementos[0].figura
        );

        let salida = escribir(&lienzo);
        assert!(salida.contains("\"frame\""), "se perdio el tipo");
        assert!(salida.contains("Lamina 1"), "se perdio el nombre");
    }

    #[test]
    fn los_grupos_del_movil_llegan_al_escritorio() {
        let json = r##"{
            "type": "excalidraw",
            "elements": [
                {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
                 "strokeColor":"#000000","seed":1,"groupIds":["g1","g2"]},
                {"type":"rectangle","x":20,"y":0,"width":10,"height":10,
                 "strokeColor":"#000000","seed":2,"groupIds":[]}
            ]
        }"##;
        let elementos = leer(json).unwrap().elementos();
        assert_eq!(elementos[0].grupos, vec!["g1", "g2"]);
        assert!(elementos[1].grupos.is_empty());
    }

    #[test]
    fn un_elemento_sin_grupos_se_lee_igual() {
        // Compatibilidad hacia atras: los ficheros que ya guardamos no llevan
        // el campo y tienen que seguir abriendo.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "strokeColor":"#000000","seed":1}
        ]}"##;
        assert!(leer(json).unwrap().elementos()[0].grupos.is_empty());
    }

    #[test]
    fn los_grupos_sobreviven_la_ida_y_la_vuelta() {
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "strokeColor":"#000000","seed":1,"groupIds":["g1"]}
        ]}"##;
        let lienzo = leer(json).unwrap();
        let otra_vez = leer(&escribir(&lienzo)).unwrap();
        assert_eq!(otra_vez.elementos()[0].grupos, vec!["g1"]);
    }

    #[test]
    fn agrupar_en_windows_se_ve_en_el_movil() {
        // Lo que hace util esta tarea: no solo conservar los grupos del
        // telefono, sino que los que se hagan aqui vuelvan alla.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "strokeColor":"#000000","seed":1}
        ]}"##;
        let mut lienzo = leer(json).unwrap();
        primero_mut(&mut lienzo).grupos = vec!["nuevo".to_string()];

        let vuelta = escribir(&lienzo);
        assert!(
            vuelta.contains("\"groupIds\""),
            "el JSON tiene que llevar groupIds: {vuelta}"
        );
        assert_eq!(leer(&vuelta).unwrap().elementos()[0].grupos, vec!["nuevo"]);
    }

    #[test]
    fn desagrupar_en_windows_no_deja_los_grupos_viejos() {
        // Si groupIds solo se escribiera cuando hay grupos, desagrupar aqui
        // dejaria intactos los del JSON original —que `escribir` reutiliza como
        // base— y el movil los seguiria viendo agrupados. Es el motivo de
        // escribirlo siempre, tambien vacio.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"rectangle","x":0,"y":0,"width":10,"height":10,
             "strokeColor":"#000000","seed":1,"groupIds":["viejo"]}
        ]}"##;
        let mut lienzo = leer(json).unwrap();
        primero_mut(&mut lienzo).grupos.clear();

        let otra_vez = leer(&escribir(&lienzo)).unwrap();
        assert!(
            otra_vez.elementos()[0].grupos.is_empty(),
            "desagrupado aqui, desagrupado alla"
        );
    }

    #[test]
    fn un_elemento_ajeno_sigue_viajando_intacto_con_sus_grupos() {
        // La garantia que no se puede romper: lo que Windows no entiende
        // sobrevive al viaje. Anadir un campo al lado no puede estropearlo.
        //
        // El tipo de muestra tiene que ser uno que no vayamos a implementar:
        // `pixpin-measure` sirvio hasta que la tarea 3 le enseno a
        // `elemento_desde` a entenderlo. El cronograma no esta en ninguna
        // fase del plan, asi que es el candidato con menos riesgo de que
        // esto vuelva a pasar con la proxima herramienta.
        let json = r#"{"type":"excalidraw","elements":[
            {"type":"pixpin-gantt","x":0,"y":0,"groupIds":["g9"],"tareas":[]}
        ]}"#;
        let lienzo = leer(json).unwrap();
        assert_eq!(lienzo.cuantos_ajenos(), 1);

        let vuelta = escribir(&lienzo);
        assert!(vuelta.contains("pixpin-gantt"), "el tipo ajeno sigue");
        assert!(vuelta.contains("\"g9\""), "y su grupo tambien");
        assert!(vuelta.contains("\"tareas\""), "y sus campos propios");
    }

    #[test]
    fn una_cota_del_movil_se_lee_como_cota() {
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"pixpin-measure","x":0,"y":0,"width":100,"height":0,
             "strokeColor":"#000000","seed":1,
             "points":[[0,0],[100,0]]}
        ]}"##;
        let l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0, "ya la entendemos");
        assert!(matches!(l.elementos()[0].figura, Figura::Cota { .. }));
    }

    #[test]
    fn una_barra_de_escala_del_movil_se_lee_como_barra() {
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"pixpin-scalebar","x":10,"y":20,"width":400,"height":24,
             "strokeColor":"#000000","seed":1}
        ]}"##;
        let l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 0);
        assert!(matches!(l.elementos()[0].figura, Figura::EscalaGrafica));
    }

    #[test]
    fn la_escala_del_lienzo_se_lee_y_se_devuelve() {
        let json = r##"{"type":"excalidraw","elements":[],
            "escala":{"unidadesPorPixel":0.03,"unidad":"m","decimales":2}}"##;
        let l = leer(json).unwrap();
        let e = l.escala.as_ref().expect("hay escala");
        assert!((e.unidades_por_pixel - 0.03).abs() < 1e-6);
        assert_eq!(e.unidad, "m");
        assert_eq!(e.decimales, 2);

        let vuelta = leer(&escribir(&l)).unwrap();
        assert_eq!(vuelta.escala, l.escala, "sobrevive la ida y la vuelta");
    }

    #[test]
    fn un_lienzo_sin_escala_se_lee_igual() {
        // Compatibilidad hacia atras: los ficheros que ya hay no la llevan.
        let json = r##"{"type":"excalidraw","elements":[]}"##;
        assert!(leer(json).unwrap().escala.is_none());
    }

    #[test]
    fn una_escala_imposible_del_movil_se_ignora() {
        // D36: mas vale no medir que medir mal. Si el movil escribiera una
        // escala rota, no se adopta.
        let json = r##"{"type":"excalidraw","elements":[],
            "escala":{"unidadesPorPixel":0,"unidad":"m","decimales":2}}"##;
        assert!(leer(json).unwrap().escala.is_none());
    }

    #[test]
    fn calibrar_en_windows_se_ve_en_el_movil() {
        let json = r##"{"type":"excalidraw","elements":[]}"##;
        let mut l = leer(json).unwrap();
        l.escala = Escala::calibrando(100.0, 3.0, "m", 2);

        let vuelta = escribir(&l);
        assert!(vuelta.contains("\"escala\""), "el JSON la lleva: {vuelta}");
        let otra = leer(&vuelta).unwrap();
        assert!((otra.escala.unwrap().unidades_por_pixel - 0.03).abs() < 1e-6);
    }

    #[test]
    fn quitar_la_escala_en_windows_la_quita_tambien_alla() {
        // Si solo se escribiera cuando existe, la del original sobreviviria y
        // el movil seguiria midiendo con una escala que aqui se borro.
        let json = r##"{"type":"excalidraw","elements":[],
            "escala":{"unidadesPorPixel":0.03,"unidad":"m","decimales":2}}"##;
        let mut l = leer(json).unwrap();
        l.escala = None;
        assert!(leer(&escribir(&l)).unwrap().escala.is_none());
    }

    #[test]
    fn una_cota_hecha_en_windows_vuelve_como_pixpin_measure() {
        let json = r##"{"type":"excalidraw","elements":[]}"##;
        let mut l = leer(json).unwrap();
        l.entradas.push(Entrada::Nuestro {
            elemento: Elemento {
                id: 1,
                figura: Figura::Cota {
                    puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
                },
                x: 0.0,
                y: 0.0,
                ancho: 100.0,
                alto: 0.0,
                angulo: 0.0,
                trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
                estilo_relleno: Default::default(),
                relleno: None,
                grosor: 2.0,
                estilo: EstiloTrazo::Solido,
                rugosidad: 1.0,
                opacidad: 1.0,
                semilla: 1,
                version: 0,
                borrado: false,
                grupos: Vec::new(),
                bloqueado: false,
            },
            original: Box::new(Value::Object(Map::new())),
        });

        let vuelta = escribir(&l);
        assert!(vuelta.contains("pixpin-measure"), "con el tipo del movil");
        assert!(matches!(
            leer(&vuelta).unwrap().elementos()[0].figura,
            Figura::Cota { .. }
        ));
    }

    #[test]
    fn una_escala_que_no_entendemos_sobrevive_intacta_y_no_se_pisa_con_null() {
        // Hallazgo 7: `leer` no quitaba «escala» de `resto`, y `escribir` la
        // pisaba siempre. Una escala corrupta -o de una version futura del
        // movil que aqui no se entiende- pasaba por Windows y salia con
        // "escala": null: «no la entiendo» y «no la hay» se representaban
        // igual, exactamente lo que `Entrada::Ajeno` existe para impedir en
        // los elementos.
        let json = r##"{"type":"excalidraw","elements":[],
            "escala":{"unidadesPorPixel":0,"unidad":"m","decimales":2,"deVersionFutura":true}}"##;
        let l = leer(json).unwrap();
        assert!(l.escala.is_none(), "no la entendemos, no mide con ella");

        let vuelta = escribir(&l);
        assert!(
            !vuelta.contains("\"escala\":null") && !vuelta.contains("\"escala\": null"),
            "la escala corrupta se piso con null:\n{vuelta}"
        );
        assert!(
            vuelta.contains("deVersionFutura"),
            "el campo que no entendemos tiene que sobrevivir tal cual:\n{vuelta}"
        );
    }

    #[test]
    fn el_decimales_de_una_escala_del_movil_tambien_se_recorta_a_seis() {
        // El mismo tope que `Escala::calibrando`, para que el invariante del
        // tipo lo sostengan los dos constructores y no solo uno.
        let json = r##"{"type":"excalidraw","elements":[],
            "escala":{"unidadesPorPixel":0.03,"unidad":"m","decimales":44}}"##;
        let l = leer(json).unwrap();
        assert_eq!(l.escala.unwrap().decimales, 6);
    }

    #[test]
    fn una_cota_del_movil_sin_puntos_entra_como_ajena_y_no_como_cota_fantasma() {
        // Hallazgo 8: sin dos puntos no hay raya que dibujar ni que tocar, y
        // antes de que `elemento_desde` aprendiera `pixpin-measure` este
        // mismo JSON sobrevivia intacto como ajeno. Una cota sin puntos no
        // es una cota: mejor que vuelva a caer ahi, donde no se pierde.
        let json = r##"{"type":"excalidraw","elements":[
            {"id":"m1","type":"pixpin-measure","x":0,"y":0,"width":0,"height":0,
             "strokeColor":"#000000","seed":1}
        ]}"##;
        let l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 1, "sin puntos, tiene que caer a ajeno");
        assert!(l.elementos().is_empty());

        let vuelta = escribir(&l);
        assert!(vuelta.contains("pixpin-measure"), "sobrevive intacta");
    }

    #[test]
    fn los_elementos_ajenos_siguen_viajando_intactos() {
        // La garantia que no se puede romper: lo que Windows no entiende
        // sobrevive. Anadir dos tipos nuevos no puede estropearlo.
        let json = r##"{"type":"excalidraw","elements":[
            {"type":"pixpin-gantt","x":0,"y":0,"tareas":[],"periodos":[]}
        ]}"##;
        let l = leer(json).unwrap();
        assert_eq!(l.cuantos_ajenos(), 1);
        let vuelta = escribir(&l);
        assert!(vuelta.contains("pixpin-gantt"));
        assert!(vuelta.contains("periodos"));
    }

    #[test]
    fn un_freedraw_de_excalidraw_trae_sus_opciones_de_pluma_y_vuelve_igual() {
        use crate::tinta::{OpcionesTinta, Variabilidad};
        let texto = r##"{"type":"excalidraw","version":2,"elements":[{"id":"a","type":"freedraw",
            "x":0,"y":0,"width":10,"height":10,"angle":0,"strokeColor":"#1e1e1e",
            "backgroundColor":"transparent","strokeWidth":2,"seed":1,
            "points":[[0,0],[5,5],[10,10]],"pressures":[0.2,0.5,0.9],"simulatePressure":false,
            "strokeOptions":{"variability":"constant","streamline":0.2}}]}"##;
        let escena = leer(texto).expect("se lee");
        let Figura::Lapiz {
            presiones,
            opciones,
            ..
        } = &escena.elementos()[0].figura
        else {
            panic!("tendria que ser un lapiz")
        };
        assert_eq!(presiones.len(), 3);
        assert_eq!(
            *opciones,
            Some(OpcionesTinta {
                variabilidad: Variabilidad::Constante,
                streamline: 0.2
            })
        );
        let vuelta = escribir(&escena);
        // `escribir` usa `to_string_pretty`: hay un espacio tras los dos puntos.
        assert!(vuelta.contains("\"variability\": \"constant\""), "{vuelta}");
        assert!(vuelta.contains("\"simulatePressure\": false"), "{vuelta}");
    }

    #[test]
    fn con_simulate_pressure_verdadero_se_ignoran_las_presiones_guardadas() {
        let texto = r##"{"type":"excalidraw","version":2,"elements":[{"id":"a","type":"freedraw",
            "x":0,"y":0,"width":10,"height":10,"strokeWidth":1,"seed":1,
            "points":[[0,0],[5,5]],"pressures":[0.2,0.5],"simulatePressure":true}]}"##;
        let escena = leer(texto).expect("se lee");
        let Figura::Lapiz {
            presiones,
            opciones,
            ..
        } = &escena.elementos()[0].figura
        else {
            panic!()
        };
        assert!(presiones.is_empty());
        assert_eq!(*opciones, Some(crate::tinta::OpcionesTinta::default()));
    }
}
