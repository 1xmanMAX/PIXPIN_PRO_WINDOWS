//! **Muestras en PNG del universo armado con el chat** (H2) y de las
//! herramientas nuevas (H3), pintadas con la GPU de verdad y sin ventana.
//!
//! ```text
//! cargo test -p pixpin --bin pixpinmax muestra_del_universo -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Deja los PNG en `target/muestras-universo`.

use pixpin_universo::desde_el_chat::{NodoDelChat, armar};
use pixpin_universo::ficha::ClaseLuna;

use super::medir::{Banco, sesion_de};
use super::*;

fn carpeta() -> PathBuf {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-universo");
    std::fs::create_dir_all(&d).expect("crear la carpeta de muestras");
    d
}

fn guardar(nombre: &str, (ancho, alto, pixeles): (u32, u32, Vec<u8>)) {
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
        ancho,
        alto,
        pixeles,
    })
    .expect("codificar");
    let ruta = carpeta().join(format!("{nombre}.png"));
    std::fs::write(&ruta, png).expect("guardar");
    println!("{nombre}: {}", ruta.display());
}

/// Un proyecto de obra: un plano en PDF con diez paginas y comentarios en
/// dos de ellas, fotos, notas de voz, comentarios sueltos y dos «👍».
fn chat() -> (Vec<NodoDelChat>, HashMap<String, FichaLuna>) {
    let mut nodos = Vec::new();
    let mut fichas = HashMap::new();
    let mut poner = |codigo: &str,
                     responde: Option<&str>,
                     clase: ClaseLuna,
                     nombre: &str,
                     pagina: Option<u32>| {
        let es_texto = clase == ClaseLuna::Nota;
        let solo_emoji = es_texto && pixpin_universo::ficha::es_solo_emoji(nombre);
        nodos.push(NodoDelChat {
            codigo: codigo.into(),
            responde_a: responde.map(String::from),
            pagina,
            es_documento: nombre.ends_with(".pdf"),
            es_texto,
            solo_emoji,
        });
        fichas.insert(
            codigo.to_string(),
            FichaLuna {
                codigo: codigo.into(),
                proyecto: "p1".into(),
                clase,
                nombre: nombre.into(),
                extracto: nombre.into(),
                bytes: if es_texto { 0 } else { 245_000 },
                codigo_chat: Some(format!("{}·K7Q2", nodos.len())),
                en_equipo: true,
                solo_emoji,
                ..Default::default()
            },
        );
    };
    poner("pdf", None, ClaseLuna::Archivo, "Plano estructural.pdf", None);
    for n in 1..=10 {
        poner(
            &format!("pag{n}"),
            Some("pdf"),
            ClaseLuna::Pagina,
            &format!("Plano · página {n}"),
            Some(n),
        );
    }
    poner("c1", Some("pag1"), ClaseLuna::Nota, "Ojo con la cota del eje B", None);
    poner("c2", Some("c1"), ClaseLuna::Nota, "Corregido en obra", None);
    poner("c3", Some("pag4"), ClaseLuna::Nota, "Falta el detalle de la viga", None);
    for n in 1..=5 {
        poner(
            &format!("foto{n}"),
            None,
            ClaseLuna::Imagen,
            &format!("IMG_20{n}1.jpg"),
            None,
        );
    }
    poner("voz", None, ClaseLuna::Voz, "Nota de voz 0:42", None);
    poner("pres", None, ClaseLuna::Archivo, "Presupuesto.xlsx", None);
    poner("f1r", Some("foto1"), ClaseLuna::Nota, "Esta grieta ya estaba", None);
    for (i, t) in [
        "Hay que pedir el acero",
        "Mañana llega el camión",
        "Revisar la losa del 2.º piso",
    ]
    .iter()
    .enumerate()
    {
        poner(&format!("n{i}"), None, ClaseLuna::Nota, t, None);
    }
    poner("e1", None, ClaseLuna::Nota, "👍", None);
    poner("e2", Some("pres"), ClaseLuna::Nota, "✅", None);
    (nodos, fichas)
}

fn sesion_armada() -> Sesion {
    let mut u = Universo::nuevo();
    pixpin_universo::galaxias::sincronizar(&mut u, &["p1".to_string(), "p2".to_string()]);
    let (nodos, fichas) = chat();
    let inf = armar(&mut u, "p1", &nodos);
    println!("armado: {inf:?}");
    let nombres = [
        ("p1".to_string(), "Casa Lima".to_string()),
        ("p2".to_string(), "Oficina".to_string()),
    ]
    .into_iter()
    .collect();
    let raiz = std::env::temp_dir().join("pixpin-muestra-universo-vacio");
    let mut s = sesion_de(raiz, u, nombres, pixpin_nivel::Nivel::Completo);
    s.fichas = fichas;
    s
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn muestra_del_universo_armado_con_el_chat() {
    let b = Banco::nuevo();
    let mut s = sesion_armada();
    let g = s.galaxia_de("p1").expect("galaxia");
    let caja = agrandar(s.u.astro(g).unwrap().caja(), 1.05);
    let camara = s.encajar(caja);
    guardar("h2-galaxia", b.foto(&mut s, &camara));

    // Mas cerca del sol: los cuerpos ya en ficha.
    let centro = s.u.astro(g).unwrap().clone();
    let cerca = s.encajar((
        centro.x - 900.0,
        centro.y - 600.0,
        centro.x + 900.0,
        centro.y + 600.0,
    ));
    guardar("h2-cerca-del-sol", b.foto(&mut s, &cerca));

    // Dentro del sistema del PDF: sus paginas en orbita y lo comentado.
    let sistema = s
        .u
        .astros
        .iter()
        .find(|a| a.sistema_de.as_deref() == Some("pdf"))
        .expect("el PDF abre sistema")
        .clone();
    let dentro = s.encajar(agrandar(sistema.caja(), 1.15));
    guardar("h2-sistema-del-pdf", b.foto(&mut s, &dentro));

    // H3: la tira de figuras con la herramienta puesta y un rotulo a medias.
    s.herramienta = Some(HerramientaUniverso::Figura);
    s.figura_elegida = FiguraUniverso::Estrella;
    let q = Punto2::nuevo(centro.x - 700.0, centro.y - 450.0);
    s.rotulo = Some((q, 60.0, "Fase 2 · Estructura".into()));
    guardar("h3-figuras-y-rotulo", b.foto(&mut s, &cerca));
}

