//! **El agente planificador de calcar** (18-oct-2026).
//!
//! El usuario: «todo tiene que ser un proceso agentico: primero un agente
//! reconoce el tema y de que se trata; despues manda varios agentes, cada uno
//! con su zona y sabiendo lo que es (texto, tabla, grafica...) y con que
//! herramientas hacerlo; que no copie la marca de agua; y que el primero haga
//! las cajas contenedoras, lo de la capa mas baja, y los demas pongan encima».
//!
//! Para que el modelo no tenga que inventar coordenadas (lo hace mal), Rust
//! pone las zonas que ya vio ([`hoja::paneles::seccionar`]) numeradas encima
//! de la imagen, y el planificador solo dice de cada numero que es y cuales
//! van juntos («set of marks»). Con eso:
//!
//! - **lo que es adorno o marca de agua** se quita;
//! - **los contenedores** (tarjetas, botones, barras) pasan a ser recuadros con
//!   su color, la capa de abajo, y salen los primeros;
//! - **cada bloque** (los renglones de una tabla, una figura y sus letras) es
//!   UNA zona con su tipo, y su especialista lo hace con sus herramientas: el
//!   texto con sus renglones corregidos, la tabla con la tabla del lienzo, la
//!   figura con geometria exacta, la foto recortada;
//! - **el tema** («teoremas de cuadrilateros inscritos», «captura de la web de
//!   la ONPE») va a cada especialista: tildes, simbolos y nombres de teoremas
//!   se corrigen sabiendo de que va.
//!
//! Si el modelo no contesta o contesta mal, la hoja sigue con el seccionado de
//! siempre: el plan solo mejora, nunca rompe.

use std::collections::{BTreeMap, HashMap, HashSet};

use pixpin_ia::hoja::paneles::Estilo;
use pixpin_ia::hoja::{Caja, Seccionado, Tipo, Zona, imagen, vision};
use pixpin_ia::motor::Motor;
use serde_json::Value;

use super::Pagina;

/// Lo que dijo el planificador.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    /// De que trata la hoja, en una o dos frases (va a cada especialista).
    pub tema: String,
    /// Por zona: lo que es.
    pub es: HashMap<usize, String>,
    /// Por zona: su grupo (las del mismo grupo son un solo bloque).
    pub grupo: HashMap<usize, String>,
}

const SISTEMA: &str = "Eres el planificador de una herramienta que pasa una imagen (una lamina, un apunte, una captura de pantalla, una tabla) a un lienzo EDITABLE. \
Recibes la imagen con unas zonas marcadas: cada una con su recuadro y su NUMERO en una etiqueta blanca. Tambien la lista de zonas con lo que leyo el OCR. \
1) Di de que trata la imagen (tema, en una frase: p. ej. «teoremas de cuadrilateros inscritos (Poncelet, Pitot)», «captura del local de votacion de la web de la ONPE», «tabla de variables e indicadores de una tesis»). \
2) Cada zona trae lo que «parece». Corrige SOLO las que estan mal, con UNA de estas palabras: \
texto (parrafo o renglones), titulo, tabla (las celdas de una tabla: todas sus zonas con el mismo grupo), figura (dibujo geometrico o esquema de lineas, circulos, flechas), grafica (ejes y curvas o barras), formula, foto (fotografia, logo, ilustracion), icono (un simbolo pequeno: flecha, lupa, mapa), contenedor (una caja, tarjeta, boton o barra de fondo que lleva cosas encima), ignorar (marca de agua, adorno palido, sombra, ruido). \
3) Agrupa: las zonas que son UNA sola cosa van juntas en un grupo (las filas de una tabla; una figura con sus letras y marcas; un parrafo partido en varias zonas). Un contenedor nunca se agrupa con lo de dentro. \
Una tabla es una REJILLA de celdas pegadas con rayas o filas; unas tarjetas sueltas, cada una con su fondo (Pabellon 100 | Piso 2 | Aula 108), NO son una tabla: son texto dentro de contenedores. Un simbolo suelto (flecha, mapa, lupa) es icono, nunca ignorar. \
Responde SOLO un JSON CORTO, solo con lo que cambia (lo que no nombres se queda como «parece»): {\"tema\":\"...\",\"grupos\":[{\"es\":\"tabla\",\"n\":[3,4,5]}],\"cambios\":[{\"n\":2,\"es\":\"titulo\"}],\"contenedores\":[7,9],\"ignorar\":[12]}.";

/// Las palabras de `es` que se entienden.
const TIPOS: &[&str] = &["texto", "titulo", "tabla", "figura", "grafica", "formula", "foto", "icono", "contenedor", "ignorar"];

/// Los numeros: una fuente de 3x5 (lo justo para leerlos en la etiqueta).
const DIGITOS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];

/// **La imagen con las zonas marcadas**: el recuadro de cada zona y su numero
/// en una etiqueta blanca en su esquina.
pub fn marcar(p: &Pagina, s: &Seccionado) -> pixpin_codec::ImagenRgba {
    let (w, h) = (p.ancho as i64, p.alto as i64);
    let mut px = p.rgba.clone();
    let mut poner = |px: &mut Vec<u8>, x: i64, y: i64, c: [u8; 3]| {
        if x >= 0 && y >= 0 && x < w && y < h {
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 3].copy_from_slice(&c);
            px[i + 3] = 255;
        }
    };
    const COLORES: [[u8; 3]; 6] = [[230, 0, 0], [0, 110, 230], [0, 160, 60], [200, 0, 200], [230, 120, 0], [0, 150, 150]];
    let k = ((p.ancho.max(p.alto) as f64 / 500.0).round() as i64).clamp(2, 6);
    for (i, z) in s.zonas.iter().enumerate() {
        let c = COLORES[i % COLORES.len()];
        let (x0, y0, x1, y1) = (z.caja.x as i64, z.caja.y as i64, z.caja.x1() as i64, z.caja.y1() as i64);
        for g in 0..(k / 2).max(1) {
            for x in x0..=x1 {
                poner(&mut px, x, y0 + g, c);
                poner(&mut px, x, y1 - g, c);
            }
            for y in y0..=y1 {
                poner(&mut px, x0 + g, y, c);
                poner(&mut px, x1 - g, y, c);
            }
        }
        // La etiqueta: fondo blanco, borde de su color, cifras negras.
        let cifras: Vec<usize> = i.to_string().bytes().map(|b| (b - b'0') as usize).collect();
        let (ew, eh) = ((cifras.len() as i64) * 4 * k + k, 7 * k);
        let (ex, ey) = (x0.max(0), (y0 - eh).max(0));
        for y in ey..ey + eh {
            for x in ex..ex + ew {
                let borde = y == ey || y == ey + eh - 1 || x == ex || x == ex + ew - 1;
                poner(&mut px, x, y, if borde { c } else { [255, 255, 255] });
            }
        }
        for (j, d) in cifras.iter().enumerate() {
            for (fy, fila) in DIGITOS[*d].iter().enumerate() {
                for fx in 0..3 {
                    if fila & (0b100 >> fx) != 0 {
                        for dy in 0..k {
                            for dx in 0..k {
                                poner(&mut px, ex + k + (j as i64) * 4 * k + fx as i64 * k + dx, ey + k + fy as i64 * k + dy, [0, 0, 0]);
                            }
                        }
                    }
                }
            }
        }
    }
    pixpin_codec::ImagenRgba { ancho: p.ancho, alto: p.alto, pixeles: px }
}

/// La lista de zonas para el modelo: numero, lo que parece y lo que leyo el OCR.
fn lista(s: &Seccionado) -> String {
    s.zonas
        .iter()
        .enumerate()
        .map(|(i, z)| {
            let t: String = z.texto().replace('\n', " / ").chars().take(70).collect();
            format!("{i}: parece {} [{:.0},{:.0} {:.0}x{:.0}] {t}\n", z.tipo.nombre(), z.caja.x, z.caja.y, z.caja.w, z.caja.h)
        })
        .collect()
}

/// **Pide el plan.** `Err` si el modelo no contesta o no se entiende.
pub fn pedir(motor: &dyn Motor, p: &Pagina, s: &Seccionado) -> Result<Plan, String> {
    let marcada = marcar(p, s);
    let (rgba, w, h) = pixpin_ia::hoja::montaje::reducir(&marcada.pixeles, marcada.ancho, marcada.alto, 1280);
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba { ancho: w, alto: h, pixeles: rgba }).map_err(|e| format!("{e:?}"))?;
    let pregunta = format!("Zonas ({}):\n{}", s.zonas.len(), lista(s));
    // (Una vez mas si no contesta o contesta algo que no es un plan.)
    let mut ultimo = String::new();
    for _ in 0..2 {
        match vision::preguntar_con(motor, SISTEMA, png.clone(), &pregunta, 3000) {
            Ok(t) => match leer(&t, s.zonas.len()) {
                Some(p) => {
                    if std::env::var_os("PIXPIN_PLAN_DEPURAR").is_some() {
                        eprintln!("plan: tema {:?}\n  es {:?}\n  grupos {:?}", p.tema, p.es, p.grupo);
                    }
                    return Ok(p);
                }
                None => ultimo = format!("el planificador no dio un plan: «{}»", t.chars().take(80).collect::<String>()),
            },
            Err(e) => ultimo = e,
        }
    }
    Err(ultimo)
}

/// Lee el JSON del plan (con o sin bloque de codigo). Solo los numeros que
/// existen y las palabras que se entienden.
pub fn leer(t: &str, n: usize) -> Option<Plan> {
    let (a, b) = (t.find('{')?, t.rfind('}')?);
    let v: Value = serde_json::from_str(t.get(a..=b)?).ok()?;
    let mut plan = Plan { tema: v["tema"].as_str().unwrap_or("").trim().to_string(), ..Default::default() };
    // El corto (grupos, cambios, contenedores, ignorar)...
    let numero = |x: &Value| x.as_u64().map(|x| x as usize).filter(|&i| i < n);
    let tipo = |x: &Value| x.as_str().map(pixpin_ia::acciones::normalizar).filter(|e| TIPOS.contains(&e.as_str()));
    for (k, g) in v["grupos"].as_array().into_iter().flatten().enumerate() {
        let es = tipo(&g["es"]);
        for i in g["n"].as_array().into_iter().flatten().filter_map(numero) {
            plan.grupo.insert(i, format!("g{k}"));
            if let Some(e) = &es {
                plan.es.insert(i, e.clone());
            }
        }
    }
    for c in v["cambios"].as_array().into_iter().flatten() {
        if let (Some(i), Some(e)) = (numero(&c["n"]), tipo(&c["es"])) {
            plan.es.insert(i, e);
        }
    }
    for (clave, es) in [("contenedores", "contenedor"), ("ignorar", "ignorar")] {
        for i in v[clave].as_array().into_iter().flatten().filter_map(numero) {
            plan.es.insert(i, es.to_string());
        }
    }
    // ...y el largo, una fila por zona.
    for z in v["zonas"].as_array().into_iter().flatten() {
        let Some(i) = z["n"].as_u64().map(|x| x as usize).filter(|&i| i < n) else { continue };
        if let Some(es) = z["es"].as_str().map(|e| pixpin_ia::acciones::normalizar(e)).filter(|e| TIPOS.contains(&e.as_str())) {
            plan.es.insert(i, es);
        }
        if let Some(g) = z["grupo"].as_str().map(str::trim).filter(|g| !g.is_empty()) {
            plan.grupo.insert(i, g.to_string());
        }
    }
    (!plan.es.is_empty() || !plan.grupo.is_empty() || !plan.tema.is_empty()).then_some(plan)
}

/// El tipo de zona de lo que dijo el planificador.
fn tipo_de(es: &str) -> Option<Tipo> {
    Some(match es {
        "texto" => Tipo::Parrafo,
        "titulo" => Tipo::Encabezado,
        "tabla" => Tipo::Tabla,
        "figura" => Tipo::Dibujo,
        "grafica" => Tipo::Grafica,
        "formula" => Tipo::Formula,
        "foto" | "icono" => Tipo::Foto,
        _ => return None,
    })
}

/// **Aplica el plan**: quita lo que se ignora, hace recuadros de los
/// contenedores, junta cada grupo en una zona y le pone su tipo. Los
/// recuadros van primero (la capa de abajo) y lo demas en el orden de antes.
pub fn aplicar(plan: &Plan, s: &Seccionado, p: &Pagina) -> Seccionado {
    let es = |i: usize| plan.es.get(&i).map(String::as_str);
    let mut contenedores: Vec<Zona> = Vec::new();
    // Grupo -> zonas (en el orden de lectura de la primera).
    let mut grupos: BTreeMap<usize, (Vec<usize>, Option<&str>)> = BTreeMap::new();
    let mut de_grupo: HashMap<&str, usize> = HashMap::new();
    let mut sueltas: HashSet<usize> = HashSet::new();
    // **Un contenedor de verdad lleva cosas dentro** (18-oct, TikTok: el
    // corazon con su «82» salia como una caja vacia). Si no hay otra zona
    // dentro y tiene letras, el plan se equivoco: se queda como era.
    let lleva_algo = |i: usize| {
        let c = s.zonas[i].caja.crecer(2.0);
        s.zonas.iter().enumerate().any(|(k, o)| k != i && o.caja.area() < c.area() * 0.9 && o.caja.dentro_de(&c) >= 0.8)
    };
    // **Lo escrito sobre un boton de color no es marca de agua** (19-oct,
    // ONPE: el «ver» claro de «ver Mapa» se ignoraba). La marca de agua va
    // sobre el papel.
    let sobre_color = |i: usize| {
        let z = &s.zonas[i];
        !z.renglones.is_empty()
            && s.zonas.iter().any(|o| {
                o.tipo == Tipo::Recuadro
                    && o.caja.contiene(z.caja.cx(), z.caja.cy())
                    && o.panel.as_ref().and_then(|e| e.relleno).is_some_and(|r| r.iter().map(|&c| (255.0 - c as f64).powi(2)).sum::<f64>().sqrt() > 60.0)
            })
    };
    for (i, z) in s.zonas.iter().enumerate() {
        let falso = es(i) == Some("contenedor") && z.tipo != Tipo::Recuadro && !lleva_algo(i);
        match es(i) {
            Some("ignorar") if !sobre_color(i) => continue,
            Some("ignorar") => {}
            Some("contenedor") if !falso => {
                let mut c = z.clone();
                if c.tipo != Tipo::Recuadro || c.panel.is_none() {
                    // Un contenedor que el seccionado vio como foto o dudosa:
                    // su color de fondo (la mediana del recorte).
                    let (sub, sw, sh) = imagen::recortar(&p.rgba, p.ancho, p.alto, &c.caja);
                    let mut canales: [Vec<u8>; 3] = Default::default();
                    for px in sub.chunks_exact(4).step_by(3) {
                        for k in 0..3 {
                            canales[k].push(px[k]);
                        }
                    }
                    let relleno = (sw > 0 && sh > 0 && !canales[0].is_empty()).then(|| {
                        let mut m = [0u8; 3];
                        for k in 0..3 {
                            canales[k].sort_unstable();
                            m[k] = canales[k][canales[k].len() / 2];
                        }
                        m
                    });
                    c.tipo = Tipo::Recuadro;
                    c.renglones.clear();
                    c.panel = Some(Estilo { relleno, borde: None, grosor: 0.0, redondo: true });
                }
                contenedores.push(c);
                continue;
            }
            _ => {}
        }
        if z.tipo == Tipo::Recuadro {
            // Un recuadro del seccionado que el plan no nombro: se queda.
            contenedores.push(z.clone());
            continue;
        }
        match plan.grupo.get(&i).map(String::as_str) {
            Some(g) => {
                let k = *de_grupo.entry(g).or_insert(i);
                let e = grupos.entry(k).or_insert((Vec::new(), es(i)));
                e.0.push(i);
            }
            None => {
                sueltas.insert(i);
                grupos.insert(i, (vec![i], if falso { None } else { es(i) }));
            }
        }
    }
    let mut zonas: Vec<Zona> = contenedores;
    for (_, (ids, es)) in grupos {
        let mut z = s.zonas[ids[0]].clone();
        for &k in &ids[1..] {
            let o = &s.zonas[k];
            z.caja = z.caja.union(&o.caja);
            z.renglones.extend(o.renglones.iter().cloned());
            z.rayas.extend(o.rayas.iter().copied());
            z.dentro.extend(o.dentro.iter().copied());
            // (Una tabla que el OCR leyo como tabla en una de sus zonas.)
            if z.tabla.is_none() {
                z.tabla = o.tabla.clone();
            }
        }
        z.renglones.sort_by(|a, b| a.caja.y.total_cmp(&b.caja.y).then(a.caja.x.total_cmp(&b.caja.x)));
        if let Some(t) = es.and_then(tipo_de) {
            z.tipo = t;
            z.confianza = z.confianza.max(0.8);
        }
        // Una foto o un icono no lleva renglones (el texto de alrededor no es suyo).
        if z.tipo == Tipo::Foto {
            z.renglones.clear();
        }
        // **Una tabla, con sus celdas** (18-oct, la de KPIs): los recuadros de
        // sus celdas no van aparte; la tabla los dibuja, y su recorte tiene
        // que llevar las rayas para medir la rejilla.
        if z.tipo == Tipo::Tabla {
            // (Tambien las pegadas, como la fila de cabecera que el plan
            // dejo fuera: hasta que no se anada ninguna.)
            loop {
                let casco = z.caja.crecer(s.alto_letra.max(4.0));
                let pegada = |c: &Zona| {
                    c.tipo == Tipo::Recuadro
                        && c.caja.area() < casco.area() * 0.95
                        && (casco.contiene(c.caja.cx(), c.caja.cy()) && c.caja.comun(&z.caja) > 0.0
                            || c.caja.comun(&z.caja.crecer(4.0)) > 0.0 && c.caja.x >= casco.x && c.caja.x1() <= casco.x1())
                };
                let celdas: Vec<usize> = zonas.iter().enumerate().filter(|(_, c)| pegada(c)).map(|(k, _)| k).collect();
                if celdas.is_empty() {
                    break;
                }
                for &k in celdas.iter().rev() {
                    z.caja = z.caja.union(&zonas[k].caja);
                    zonas.remove(k);
                }
            }
        }
        zonas.push(z);
    }
    // Lo suelto que cae dentro de una tabla es de la tabla (la lee entera).
    let tablas: Vec<Caja> = zonas.iter().filter(|z| z.tipo == Tipo::Tabla).map(|z| z.caja).collect();
    zonas.retain(|z| z.tipo == Tipo::Tabla || z.tipo == Tipo::Recuadro || !tablas.iter().any(|t| t.contiene(z.caja.cx(), z.caja.cy())));
    for (i, z) in zonas.iter_mut().enumerate() {
        z.id = i;
        z.orden = i;
    }
    let _ = sueltas;
    Seccionado { zonas, paneles: s.paneles.clone(), ..s.clone() }
}

/// Lo que se le cuenta a cada especialista del tema de la hoja.
pub fn contexto(plan: &Plan) -> String {
    if plan.tema.is_empty() { String::new() } else { format!("La imagen trata de: {}. Esta en espanol.", plan.tema) }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_ia::hoja::{Alineacion, Renglon};

    fn zona(tipo: Tipo, x: f64, y: f64, w: f64, h: f64, texto: &str) -> Zona {
        let renglones = if texto.is_empty() {
            Vec::new()
        } else {
            vec![Renglon { texto: texto.into(), caja: Caja::nueva(x, y, w, h), palabras: Vec::new(), negrita: false, color: None }]
        };
        Zona { id: 0, tipo, caja: Caja::nueva(x, y, w, h), orden: 0, columna: 0, confianza: 0.5, renglones, alineacion: Alineacion::Izquierda, tabla: None, rayas: Vec::new(), dentro: Vec::new(), panel: None }
    }

    #[test]
    fn el_plan_se_lee_y_se_aplica_sin_perder_nada_que_no_se_ignore() {
        let s = Seccionado {
            ancho: 400,
            alto: 300,
            zonas: vec![
                zona(Tipo::Parrafo, 10.0, 10.0, 200.0, 20.0, "Elemento Descripcion"),
                zona(Tipo::Parrafo, 10.0, 40.0, 200.0, 20.0, "Variable Modelo"),
                zona(Tipo::Foto, 220.0, 100.0, 100.0, 100.0, ""),
                zona(Tipo::Dudosa, 0.0, 0.0, 400.0, 300.0, ""),
            ],
            alto_letra: 12.0,
            ..Default::default()
        };
        let t = "```json\n{\"tema\":\"tabla de una tesis\",\"zonas\":[{\"n\":0,\"es\":\"tabla\",\"grupo\":\"t\"},{\"n\":1,\"es\":\"tabla\",\"grupo\":\"t\"},{\"n\":2,\"es\":\"ignorar\"},{\"n\":3,\"es\":\"contenedor\"},{\"n\":9,\"es\":\"texto\"}]}\n```";
        let plan = leer(t, 4).expect("se lee");
        assert_eq!(plan.tema, "tabla de una tesis");
        assert!(!plan.es.contains_key(&9), "un numero que no existe no entra");
        let p = Pagina { rgba: vec![200; 400 * 300 * 4], ancho: 400, alto: 300, palabras: Vec::new(), ms_leer: 0 };
        let r = aplicar(&plan, &s, &p);
        // El contenedor primero (capa de abajo), y la tabla como UNA zona con sus dos renglones.
        assert_eq!(r.zonas.len(), 2, "{:?}", r.zonas.iter().map(|z| z.tipo).collect::<Vec<_>>());
        assert_eq!(r.zonas[0].tipo, Tipo::Recuadro);
        assert!(r.zonas[0].panel.unwrap().relleno.is_some());
        assert_eq!(r.zonas[1].tipo, Tipo::Tabla);
        assert_eq!(r.zonas[1].renglones.len(), 2);
        // Lo ignorado (la marca de agua) no esta.
        assert!(r.zonas.iter().all(|z| z.tipo != Tipo::Foto));
        // Negativo: sin JSON no hay plan, y la hoja sigue como estaba.
        assert!(leer("no se", 4).is_none());
    }

    #[test]
    fn la_imagen_marcada_lleva_los_numeros() {
        let s = Seccionado { ancho: 200, alto: 100, zonas: vec![zona(Tipo::Parrafo, 20.0, 40.0, 100.0, 30.0, "x")], ..Default::default() };
        let p = Pagina { rgba: vec![255; 200 * 100 * 4], ancho: 200, alto: 100, palabras: Vec::new(), ms_leer: 0 };
        let m = marcar(&p, &s);
        // Algo negro (la cifra 0) en la etiqueta, encima de la zona.
        let negros = m.pixeles.chunks_exact(4).filter(|c| c[0] == 0 && c[1] == 0 && c[2] == 0).count();
        assert!(negros > 4);
    }
}

#[cfg(test)]
mod pruebas_corto {
    #[test]
    fn el_plan_corto_solo_trae_lo_que_cambia() {
        let t = r#"{"tema":"tabla KPI","grupos":[{"es":"tabla","n":[1,2,3]}],"cambios":[{"n":0,"es":"titulo"}],"contenedores":[4],"ignorar":[5,99]}"#;
        let p = super::leer(t, 6).unwrap();
        assert_eq!(p.es.get(&0).map(String::as_str), Some("titulo"));
        assert_eq!(p.es.get(&2).map(String::as_str), Some("tabla"));
        assert_eq!(p.grupo.get(&1), p.grupo.get(&3));
        assert_eq!(p.es.get(&4).map(String::as_str), Some("contenedor"));
        assert_eq!(p.es.get(&5).map(String::as_str), Some("ignorar"));
        assert!(!p.es.contains_key(&99));
    }
}
