//! **La maqueta del PC de antes de K16**, solo para mover una vez la tinta
//! que se hizo con ella.
//!
//! Hasta K16 el lector colocaba el texto a su manera (Segoe UI, renglones de
//! la propia letra, aire en «emes» delante de cada bloque, tablas con la
//! rejilla de Word) y la tinta se guardaba encima de esa disposicion. Ahora
//! coloca como el movil (`maqueta_movil`) y esa tinta quedaria sobre otras
//! palabras. Aqui estan las cuentas de antes, tal cual, para saber donde
//! empezaba cada bloque y llevar cada trazo al mismo sitio de su bloque en
//! la maqueta nueva (`mudar`). No pinta nada.

use super::*;
use pixpin_docs::tabla::Celda;

/// Lo que media un parrafo con la letra de antes: ancho y alto a un ancho.
pub(crate) type MideViejo<'a> = dyn Fn(&str, f32, f32, &[Tramo]) -> (f32, f32) + 'a;

/// Donde caia un bloque: su numero (ninguno, la cabecera), su y y su alto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Viejo {
    pub(crate) bloque: Option<usize>,
    pub(crate) y: f32,
    pub(crate) alto: f32,
}

/// El aire delante de cada clase, en «emes», y su tamano (lo de `pinta`).
fn aire(clase: Clase) -> (f32, f32) {
    match clase {
        Clase::Titulo(1) => (1.7, 1.2),
        Clase::Titulo(2) => (1.4, 1.1),
        Clase::Titulo(3) => (1.2, 1.0),
        Clase::Titulo(_) => (1.05, 0.9),
        Clase::Cita => (1.0, 0.8),
        Clase::Codigo => (0.9, 0.6),
        Clase::Nota => (0.9, 0.5),
        Clase::Capitulo | Clase::Regla => (1.0, 1.6),
        _ => (1.0, 0.35),
    }
}

/// La negrita y el ancho fijo de antes eran el grosor y el tipo del PC; con
/// los del movil, la «gruesa» es la 2 y el «ancho fijo» el 2.
fn todo(ajustes: &lectura::Ajustes, negrita: bool) -> EstiloTexto {
    EstiloTexto {
        negrita: negrita || ajustes.grosor >= 2,
        cursiva: false,
        mono: ajustes.tipo == 2,
    }
}

fn tramos_viejos(texto: &str, lista: &[pixpin_docs::documento::TramoDoc], t: EstiloTexto) -> Vec<Tramo> {
    let mut tramos: Vec<Tramo> = lista
        .iter()
        .map(|x| Tramo {
            inicio: x.inicio,
            longitud: x.longitud,
            estilo: EstiloTexto {
                negrita: x.estilo.negrita,
                cursiva: x.estilo.cursiva || x.estilo.enlace,
                mono: x.estilo.mono,
            },
        })
        .collect();
    if t != EstiloTexto::default() {
        tramos.insert(0, Tramo { inicio: 0, longitud: texto.encode_utf16().count() as u32, estilo: t });
    }
    tramos
}

/// **Donde caia cada bloque con la maqueta de antes.**
pub(crate) fn colocar(doc: &Documento, ajustes: &lectura::Ajustes, columna: f32, mide: &MideViejo<'_>) -> (Vec<Viejo>, f32) {
    let base = 16.0 * ajustes.tamano as f32 / 100.0;
    let mut salida = Vec::with_capacity(doc.bloques.len() + 2);
    let mut y = base * 2.0;
    let mut imagen = 0usize;
    let mut cabecera = Vec::new();
    if !doc.titulo.trim().is_empty() {
        cabecera.push((Clase::Titulo(1), doc.titulo.trim().to_string()));
    }
    if !doc.autor.trim().is_empty() {
        cabecera.push((Clase::Nota, doc.autor.trim().to_string()));
    }
    let bloques = cabecera
        .iter()
        .map(|(c, t)| (*c, t.clone(), Vec::new(), 0.0f32, None))
        .chain(doc.bloques.iter().enumerate().map(|(i, b)| {
            let (texto, tramos) = texto_y_tramos(b);
            let sangria = match b.clase {
                Clase::Lista => 1.2,
                Clase::Cita => 1.4,
                _ => 0.0,
            };
            (b.clase, texto, tramos, sangria, Some(i))
        }))
        .collect::<Vec<_>>();
    let mut tabla_hasta = 0usize;
    for (clase, mut texto, lista, sangria_em, bloque) in bloques {
        if clase == Clase::Fila
            && let Some(i) = bloque
        {
            if i < tabla_hasta {
                continue;
            }
            let mut hasta = i + 1;
            while hasta < doc.bloques.len() && pixpin_docs::tabla::misma_tabla(&doc.bloques[hasta - 1], &doc.bloques[hasta]) {
                hasta += 1;
            }
            y = tabla(doc, i, hasta, ajustes, columna, mide, y, &mut salida);
            tabla_hasta = hasta;
            continue;
        }
        let (factor, aire_em) = aire(clase);
        let negrita = matches!(clase, Clase::Titulo(_));
        let tam = base * factor;
        y += base * aire_em;
        if clase == Clase::Regla || clase == Clase::Capitulo {
            salida.push(Viejo { bloque, y, alto: 1.0 });
            y += base * aire_em;
            continue;
        }
        if clase == Clase::Lista {
            texto = format!("• {texto}");
        }
        if clase == Clase::Nota && texto == pixpin_docs::documento::MARCA_IMAGEN {
            let peso = doc.imagenes.get(imagen).map(|i| i.datos.len() / 1024).unwrap_or(0);
            imagen += 1;
            texto = format!("🖼 imagen ({peso} kB) — se ve al guardar la página");
        }
        if texto.trim().is_empty() {
            y += tam * 0.6;
            continue;
        }
        let sangria = sangria_em * base;
        let tramos = tramos_viejos(&texto, &lista, todo(ajustes, negrita));
        let alto = mide(&texto, tam, columna - sangria, &tramos).1;
        salida.push(Viejo { bloque, y, alto });
        y += alto;
    }
    (salida, y + base * 2.0)
}

/// El texto de una celda como lo pintaba antes: sus parrafos con algo,
/// separados por un salto (los vacios no dejaban renglon).
fn texto_de_celda(c: &Celda) -> String {
    c.texto().split('\n').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("\n")
}

#[allow(clippy::too_many_arguments)] // documento, filas, letra, columna, medida, altura y salida
fn tabla(
    doc: &Documento,
    desde: usize,
    hasta: usize,
    ajustes: &lectura::Ajustes,
    columna: f32,
    mide: &MideViejo<'_>,
    mut y: f32,
    salida: &mut Vec<Viejo>,
) -> f32 {
    let base = 16.0 * ajustes.tamano as f32 / 100.0;
    let (aire_y, aire_x) = (base * 0.25, base * 0.5);
    let filas: Vec<Vec<Celda>> = doc.bloques[desde..hasta].iter().map(pixpin_docs::tabla::celdas_de).collect();
    let n = pixpin_docs::tabla::columnas_de(&filas);
    let mut palabras: Vec<std::collections::BTreeSet<String>> = vec![Default::default(); n];
    let mut largas: Vec<String> = vec![String::new(); n];
    for f in &filas {
        let mut g = 0usize;
        for c in f {
            let t = texto_de_celda(c);
            if c.columnas == 1 && !c.sigue && g < n {
                palabras[g].extend(t.split(char::is_whitespace).filter(|p| !p.is_empty()).map(str::to_string));
                let parrafo = pixpin_docs::tabla::parrafo_mas_largo(&t);
                if parrafo.chars().count() > largas[g].chars().count() {
                    largas[g] = parrafo.to_string();
                }
            }
            g += c.columnas.max(1) as usize;
        }
    }
    let medir_ancho = |t: &str| {
        if t.is_empty() {
            0.0
        } else {
            let tramos = tramos_viejos(t, &[], todo(ajustes, true));
            mide(t, base, 1.0e6, &tramos).0
        }
    };
    let minimo: Vec<f32> = palabras
        .iter()
        .map(|ps| {
            let mut v: Vec<&String> = ps.iter().collect();
            v.sort_by_key(|p| std::cmp::Reverse(p.chars().count()));
            let mas_ancha = v.iter().take(6).map(|p| medir_ancho(p)).fold(0.0, f32::max);
            mas_ancha.min(base * 9.0) + 2.0 * aire_x + 1.0
        })
        .collect();
    let maximo: Vec<f32> = largas.iter().map(|p| medir_ancho(p) + 2.0 * aire_x + 1.0).collect();
    let preferidas = pixpin_docs::tabla::anchos_de_word(doc.bloques[desde].fila.as_ref(), n, columna);
    let tope = columna + vista::margen_de(columna);
    let anchos = pixpin_docs::tabla::repartir(&minimo, &maximo, preferidas.as_deref(), columna, tope);
    let mut xs = vec![0.0f32];
    for a in &anchos {
        xs.push(xs.last().copied().unwrap_or(0.0) + a);
    }
    y += base * 0.8;
    for (k, f) in filas.iter().enumerate() {
        let mut alto_fila = base * 1.45 + 2.0 * aire_y;
        let mut g = 0usize;
        for c in f {
            if g >= n {
                break;
            }
            let fin = (g + c.columnas.max(1) as usize).min(n);
            let t = texto_de_celda(c);
            let caja = (xs[fin] - xs[g] - 2.0 * aire_x).max(1.0);
            if !t.trim().is_empty() {
                let b = pixpin_docs::documento::Bloque::nuevo(Clase::Parrafo, c.trozos.clone());
                let (_, lista) = texto_y_tramos(&b);
                let tramos = tramos_viejos(&t, &lista, todo(ajustes, false));
                alto_fila = alto_fila.max(mide(&t, base, caja, &tramos).1 + 2.0 * aire_y);
            }
            g = fin;
        }
        salida.push(Viejo { bloque: Some(desde + k), y: y + aire_y, alto: alto_fila - aire_y });
        y += alto_fila;
    }
    y + base * 0.8
}

// ---------------------------------------------------------------------------
// Mudar la tinta

/// **Lleva cada trazo de la maqueta vieja a la nueva**: al bloque en el que
/// caia (el ultimo que empezaba por encima de su centro, como el ancla de la
/// pagina web), al mismo sitio en proporcion dentro de ese bloque. A lo
/// ancho, lo que estaba en el texto va en proporcion a la caja de texto
/// nueva, y lo del margen conserva su distancia al borde del texto.
///
/// `viejos` y `nuevos` son los tops y altos de cada bloque (`bloque`, `y`,
/// `alto`) en una y otra; `columna` la de antes y `(x, ancho)` la caja de
/// texto nueva. Devuelve cuanto hay que correr cada elemento.
pub(crate) fn mudanza(
    centro: (f32, f32),
    viejos: &[Viejo],
    nuevos: &[(usize, f32, f32)],
    columna: f32,
    (x, ancho): (f32, f32),
) -> (f32, f32) {
    let dx = if centro.0 < 0.0 {
        x
    } else if centro.0 > columna {
        x + ancho - columna
    } else {
        x + centro.0 * (ancho / columna.max(1.0)) - centro.0
    };
    let ancla = viejos
        .iter()
        .filter(|v| v.bloque.is_some() && v.y <= centro.1 + 6.0)
        .max_by(|a, b| a.y.total_cmp(&b.y));
    let dy = match ancla.and_then(|v| {
        let b = v.bloque?;
        nuevos.iter().find(|n| n.0 == b).map(|n| (v, n))
    }) {
        Some((v, &(_, y, alto))) => {
            let dentro = ((centro.1 - v.y) / v.alto.max(1.0)).clamp(0.0, 1.5);
            y + dentro * alto - centro.1
        }
        None => 0.0,
    };
    (dx, dy)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_trazo_del_texto_viejo_cae_en_el_mismo_sitio_de_su_bloque_nuevo() {
        let viejos = [
            Viejo { bloque: None, y: 32.0, alto: 30.0 },
            Viejo { bloque: Some(0), y: 100.0, alto: 40.0 },
            Viejo { bloque: Some(1), y: 150.0, alto: 80.0 },
        ];
        let nuevos = [(0usize, 30.0, 50.0), (1usize, 90.0, 100.0)];
        // En la mitad del bloque 1 viejo (y 190), en la mitad del nuevo (140).
        let (dx, dy) = mudanza((400.0, 190.0), &viejos, &nuevos, 800.0, (16.0, 704.0));
        assert!((190.0 + dy - 140.0).abs() < 1e-3, "{dy}");
        // A lo ancho, en proporcion a la caja de texto nueva.
        assert!((400.0 + dx - (16.0 + 400.0 * 704.0 / 800.0)).abs() < 1e-3, "{dx}");
        // En el margen izquierdo conserva su distancia al borde del texto.
        let (dx, _) = mudanza((-50.0, 190.0), &viejos, &nuevos, 800.0, (16.0, 704.0));
        assert_eq!(dx, 16.0);
        let (dx, _) = mudanza((850.0, 190.0), &viejos, &nuevos, 800.0, (16.0, 704.0));
        assert_eq!(850.0 + dx, 16.0 + 704.0 + 50.0);
        // Caso negativo: por encima de todo bloque no se mueve de alto.
        assert_eq!(mudanza((10.0, 5.0), &viejos, &nuevos, 800.0, (0.0, 800.0)).1, 0.0);
    }
}
