//! **Que herramientas se le ensenan al modelo en cada turno.**
//!
//! Con todas las de la app (unas 33) un modelo de 2B se pierde: elige mal,
//! mezcla parametros de una con otra, y el catalogo solo ya se come ~3000
//! tokens de cada vuelta. Asi que antes de preguntarle se eligen las pocas
//! que vienen a cuento (6 como mucho) por palabras clave del pedido —las de
//! la tabla de `acciones.rs` y las de aqui para las propias—:
//!
//! - una pregunta sobre la app («¿donde…?», «¿como…?») solo lleva
//!   `ayuda_app`: que explique, no que lo haga;
//! - un saludo o una charla corta no lleva ninguna: contesta con texto;
//! - con una imagen adjunta van primero `calcar_imagen` y `dibujar_en_lienzo`;
//! - si nada encaja, las de siempre (dibujar, grafica y ayuda).

use crate::acciones::{ACCIONES, normalizar};
use crate::herramientas::{AYUDA, CALCAR, DIBUJAR, GRAFICA, Herramienta, OCR, catalogo_base};

/// Cuantas herramientas como mucho por turno.
pub const MAXIMO: usize = 6;

/// Las claves de las herramientas propias.
fn claves_base(nombre: &str) -> &'static [&'static str] {
    match nombre {
        DIBUJAR => &[
            "dibuj", "pinta", "traza", "figura", "circulo", "cuadrado", "rectangulo", "triangulo", "estrella",
            "flecha", "linea", "diagrama", "flujo", "esquema", "casa", "muneco", "curva", "onda", "poligono",
            "titulo", "lienzo", "forma", "rombo", "elipse", "organigrama", "mapa", "croquis",
        ],
        GRAFICA => &["grafic", "barras", "pastel", "torta", "estadistic", "porcentaje", "chart", "encuesta", "datos"],
        OCR => &["ocr", "escanead", "texto de", "lee el", "leer el", "extrae", "reconoce"],
        CALCAR => &["calca", "copia", "imagen", "foto", "contorno", "pasa este", "pasa esta", "replica"],
        AYUDA => &["ayuda", "explica", "funciona", "sirve", "donde", "atajo", "menu"],
        _ => &[],
    }
}

/// Lo que puntua un juego de claves en un pedido normalizado.
fn puntuar(claves: &[&str], q: &str, palabras: &[&str]) -> u32 {
    claves
        .iter()
        .map(|c| {
            if c.contains(' ') || c.contains(':') {
                if q.contains(c) { 3 } else { 0 }
            } else if palabras.iter().any(|p| p.starts_with(c)) {
                2
            } else {
                0
            }
        })
        .sum()
}

/// Si el pedido es una pregunta sobre como o donde (no un encargo).
pub fn es_pregunta_de_ayuda(q: &str) -> bool {
    let q = q.trim_start_matches(|c: char| !c.is_alphanumeric());
    ["como ", "donde ", "que es ", "para que ", "cual ", "en que ", "por que ", "que hace", "se puede"]
        .iter()
        .any(|p| q.starts_with(p))
}

/// Un saludo o un «gracias»: sin herramientas.
fn es_charla(q: &str, palabras: &[&str]) -> bool {
    palabras.len() <= 4
        && palabras
            .iter()
            .any(|p| ["hola", "gracias", "buenas", "adios", "chao", "ok", "vale", "genial", "perfecto"].contains(p))
        || q.contains("que tal")
}

/// **Las herramientas para este pedido.** `con_imagen`: si el mensaje trae
/// una foto (para calcarla).
pub fn para_el_pedido(pedido: &str, con_imagen: bool) -> Vec<Herramienta> {
    let base = catalogo_base();
    let de = |n: &str| base.iter().find(|h| h.nombre == n).cloned();
    let q = normalizar(pedido);
    let palabras: Vec<&str> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .collect();
    if es_pregunta_de_ayuda(&q) {
        return de(AYUDA).into_iter().collect();
    }
    if !con_imagen && es_charla(&q, &palabras) {
        return Vec::new();
    }
    let mut puntuadas: Vec<(u32, Herramienta)> = Vec::new();
    for h in &base {
        let mut s = puntuar(claves_base(&h.nombre), &q, &palabras);
        if con_imagen && (h.nombre == CALCAR || h.nombre == DIBUJAR) {
            s += if h.nombre == CALCAR { 20 } else { 10 };
        }
        if h.nombre == CALCAR && !con_imagen {
            // Sin imagen no hay nada que calcar.
            continue;
        }
        puntuadas.push((s, h.clone()));
    }
    for a in ACCIONES {
        puntuadas.push((puntuar(a.claves, &q, &palabras), crate::acciones::herramienta(a)));
    }
    // Estable: a igual puntuacion, el orden del catalogo.
    puntuadas.sort_by(|a, b| b.0.cmp(&a.0));
    let elegidas: Vec<Herramienta> = puntuadas
        .into_iter()
        .filter(|(s, _)| *s >= 2)
        .take(MAXIMO)
        .map(|(_, h)| h)
        .collect();
    if elegidas.is_empty() {
        return [DIBUJAR, GRAFICA, AYUDA].iter().filter_map(|n| de(n)).collect();
    }
    elegidas
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn nombres(pedido: &str, img: bool) -> Vec<String> {
        para_el_pedido(pedido, img).into_iter().map(|h| h.nombre).collect()
    }

    #[test]
    fn elige_pocas_y_las_que_vienen_a_cuento() {
        for (pedido, img, debe) in [
            ("Dibuja un circulo rojo y un cuadrado azul", false, "dibujar_en_lienzo"),
            ("Hazme una grafica de barras de ventas: enero 120", false, "grafica"),
            ("Copia esta imagen en el lienzo", true, "calcar_imagen"),
            ("Apunta la tarea: llamar al ingeniero", false, "anadir_tarea"),
            ("Crea una lista de tareas que se llame Obra", false, "lista_nueva"),
            ("Abre la ventana de tareas", false, "abrir_ventana"),
            ("Enseñame la galeria de capturas", false, "abrir_ventana"),
            ("Haz una captura de una zona de la pantalla", false, "capturar_pantalla"),
            ("Saca como pin la ultima captura", false, "pinear_ultima_captura"),
            ("Crea un lienzo nuevo llamado Croquis", false, "lienzo_nuevo"),
            ("Graba una nota de voz que se llame reunion", false, "grabar_voz"),
            (r"Abre el archivo C:\Users\yo\plano.pdf", false, "abrir_fichero"),
            ("Busca en mis lecciones aprendidas las de encofrado", false, "buscar_lecciones"),
            ("Crea una nota nueva que diga: comprar cemento", false, "nota_nueva"),
            ("Sincroniza con el movil", false, "sincronizar_movil"),
            ("Lee el texto de este pdf escaneado", false, "ocr"),
        ] {
            let n = nombres(pedido, img);
            assert!(n.iter().any(|x| x == debe), "«{pedido}» → {n:?}");
            assert!(n.len() <= MAXIMO, "«{pedido}» → {n:?}");
        }
    }

    #[test]
    fn una_pregunta_solo_lleva_la_ayuda() {
        assert_eq!(nombres("¿Cómo hago una captura de pantalla?", false), vec!["ayuda_app"]);
        assert_eq!(nombres("¿Donde estan los ajustes de PixPin?", false), vec!["ayuda_app"]);
        // Pedir algo con una pregunta de cortesia no es pedir ayuda.
        assert!(nombres("¿Puedes abrir la galeria?", false).contains(&"abrir_ventana".to_string()));
    }

    #[test]
    fn un_saludo_no_lleva_herramientas_y_lo_raro_lleva_las_de_siempre() {
        assert!(nombres("Hola, ¿que tal?", false).is_empty());
        assert!(nombres("gracias", false).is_empty());
        assert_eq!(nombres("hazme algo bonito", false), vec!["dibujar_en_lienzo", "grafica", "ayuda_app"]);
    }

    #[test]
    fn sin_imagen_no_se_ofrece_calcar() {
        assert!(!nombres("copia el circulo", false).contains(&"calcar_imagen".to_string()));
        assert_eq!(nombres("hola", true)[0], "calcar_imagen");
    }
}
