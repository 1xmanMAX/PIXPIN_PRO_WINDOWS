//! **Lo anotado en los lectores sobre un adjunto del chat, con los nombres
//! del movil** (v0.96, `sincro/AnotacionesDelAdjunto.kt`,
//! `ui/ExportarDocumentoAnotado.kt`, `pdf/LectorPdfActivity.kt`).
//!
//! Los lectores del PC (Word/libro en `visor`, PDF en `lector_pdf`) leen y
//! guardan por aqui. Si el documento es un adjunto de un chat del almacen,
//! lo que viaja va a los ficheros `anot-<uid>…` del mensaje (ver
//! `pixpin_proyecto::anotado`, donde esta el porque de una sola verdad):
//!
//! | Que | Word / libro | PDF suelto | PDF de un proyecto |
//! |---|---|---|---|
//! | tinta | `anot-<uid>` (corrida `izq`) | `anot-<uid>-p<n>` | sus hojas (ya viajaban) |
//! | marcadores | `.marcas` (`id:fraccion:emoji`) | `.marcas` (`id:x:y:emoji`) | `anot-<uid del proyecto>.marcas` |
//! | espacios | — | `.espacios` | `anot-<uid del proyecto>.espacios` |
//! | columna y letra | `.maqueta` | — | — |
//! | por donde iba | `.sitio` | — | — |
//!
//! Lo demas (el aumento, la pagina del PDF) sigue en `<doc>.pixpin-lectura`,
//! que es de esta pantalla, como en el movil siguen en sus preferencias. Lo
//! que el PC no tiene (el verde de la voz, lo de un PDF leido como texto) no
//! se toca: se queda en su fichero y se vuelve a mandar tal cual.
//!
//! **La primera vez** lo de antes (junto al documento) pasa a los ficheros
//! del mensaje si estos no existen aun, y no se borra nada (`migrar`).
//!
//! # La columna del movil y la del PC
//!
//! La maqueta es `columna,izq,der,tamano,grosor,tipo` con los numeros del
//! movil. La columna y el tamano valen tal cual (pixeles de lectura y tanto
//! por ciento). El grosor y el tipo son indices de sus cuatro pesos (300,
//! 400, 600, 800) y sus cuatro letras (serif, sans, mono, cursiva), **los
//! mismos del PC desde K16** (`lectura::TIPOS`/`GROSORES`): con otra letra
//! el texto se parte en otros renglones y la tinta cae en otra palabra, asi
//! que viajan tal cual. La tinta del movil cuenta desde el borde de la pagina (la
//! columna empieza en `izq`) y la del PC desde el borde de la columna: se
//! corre `izq` al leer y se devuelve al guardar. Una maqueta que el PC no
//! cambia no se reescribe: sus `izq`, `der`, grosor y tipo del movil quedan.

use std::path::{Path, PathBuf};

use pixpin_docs::lectura::{self, Ajustes};
use pixpin_proyecto::anotado::{self as an, Base};
use pixpin_sincro::anotado::{Maqueta, margen_de};

use crate::lector_tinta::{self, Capa};

fn es_pdf(doc: &Path) -> bool {
    doc.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// La base de lo anotado de `doc`, si es un adjunto del chat (o el PDF de un proyecto).
fn base_de(doc: &Path) -> Option<Base> {
    let raiz = an::raiz_de(doc)?;
    if es_pdf(doc) {
        let proyecto = pixpin_proyecto::capas_del_pdf::de_este_pdf(&raiz, doc).map(|p| p.ficha);
        an::base_del_pdf(&raiz, doc, proyecto.as_deref())
    } else {
        an::base_del_documento(&raiz, doc)
    }
}

// ------------------------------------------------ la letra del movil y la del PC

/// El grosor del PC como indice de los del movil: desde K16 son los mismos
/// cuatro (300, 400, 600, 800).
pub fn grosor_al_movil(g: u8) -> i32 {
    i32::from(g.min(lectura::GROSORES as u8 - 1))
}

/// Uno de los cuatro grosores del movil, el mismo en el PC (uno fuera de
/// rango, al mas cercano).
pub fn grosor_del_movil(g: i32) -> u8 {
    g.clamp(0, lectura::GROSORES as i32 - 1) as u8
}

/// La letra del PC como indice de las del movil: las mismas cuatro (serif,
/// sans, ancho fijo, cursiva).
pub fn tipo_al_movil(t: u8) -> i32 {
    i32::from(t.min(lectura::TIPOS as u8 - 1))
}

/// Una de las cuatro letras del movil, la misma en el PC.
pub fn tipo_del_movil(t: i32) -> u8 {
    t.clamp(0, lectura::TIPOS as i32 - 1) as u8
}

/// **El fichero de la tinta del mensaje** de un Word o un libro del chat
/// (`anot-<uid>.excalidraw`), exista o no; `None` si no es un adjunto. El
/// lector mira si ya estaba antes de leer la capa: la de ahi va en la
/// maqueta del movil, la de junto al documento puede ser de la vieja (K16).
pub fn tinta_del_mensaje(doc: &Path) -> Option<PathBuf> {
    base_de(doc).filter(|_| !es_pdf(doc)).map(|b| b.tinta())
}

/// La maqueta que corresponde a lo fijado en el PC, conservando del movil lo
/// que el PC no sabe decir si la columna no cambio. `None` si lo que hay ya
/// dice eso mismo (no se reescribe).
pub fn maqueta_para(a: &Ajustes, actual: Option<Maqueta>) -> Option<Maqueta> {
    let columna = a.columna as i32;
    if let Some(m) = actual
        && m.columna == columna
        && lectura::tamano_valido(m.tamano.max(0) as u32) == a.tamano
        && grosor_del_movil(m.grosor) == a.grosor
        && tipo_del_movil(m.tipo) == a.tipo
    {
        return None;
    }
    let (izq, der) = match actual {
        Some(m) if m.columna == columna => (m.izq, m.der),
        _ => (margen_de(columna), margen_de(columna)),
    };
    Some(Maqueta {
        columna,
        izq,
        der,
        tamano: a.tamano as i32,
        grosor: grosor_al_movil(a.grosor),
        tipo: tipo_al_movil(a.tipo),
    })
}

/// Los marcadores de un Word como los escribe `Lectura.aTexto`.
fn marcadores_a_texto(a: &Ajustes) -> String {
    a.marcadores
        .iter()
        .map(|m| format!("{}:{}:{}", m.id, m.fraccion, m.emoji))
        .collect::<Vec<_>>()
        .join("|")
}

// ------------------------------------------------------------ leer y escribir

/// **Lo que se recuerda de un documento**: lo de junto al documento y, si
/// es un adjunto del chat, lo que viaja encima (manda lo de su mensaje).
pub fn leer(doc: &Path) -> Ajustes {
    let mut a = lectura::leer(doc);
    let Some(b) = base_de(doc) else {
        return a;
    };
    migrar(&b, &a, es_pdf(doc));
    poner_lo_del_mensaje(&b, &mut a, es_pdf(doc));
    a
}

/// Lo de antes a los ficheros del mensaje, si aun no los tiene (`migrar`).
fn migrar(b: &Base, a: &Ajustes, pdf: bool) {
    if pdf {
        an::poner_si_falta(&b.fichero(".marcas"), Some(&a.marcas).filter(|m| !m.is_empty()).map(String::as_str));
        an::poner_si_falta(&b.fichero(".espacios"), (a.espacios != 0).then(|| a.espacios.to_string()).as_deref());
        return;
    }
    let marcas = (!a.marcadores.is_empty()).then(|| marcadores_a_texto(a));
    an::poner_si_falta(&b.fichero(".marcas"), marcas.as_deref());
    let maqueta = (a.columna > 0).then(|| maqueta_para(a, None).map(|m| m.a_texto())).flatten();
    an::poner_si_falta(&b.fichero(".maqueta"), maqueta.as_deref());
    an::poner_si_falta(&b.fichero(".sitio"), (a.sitio > 0.0).then(|| a.sitio.to_string()).as_deref());
}

fn poner_lo_del_mensaje(b: &Base, a: &mut Ajustes, pdf: bool) {
    let leer = |t: &str| an::leer(&b.fichero(t));
    if pdf {
        if let Some(t) = leer(".marcas") {
            a.marcas = t.replace(['\n', '\r'], "").trim().to_string();
        }
        // Lo que no se entiende no pisa lo de aqui (`toIntOrNull() ?: prefs`).
        if let Some(e) = leer(".espacios").and_then(|t| t.trim().parse::<i64>().ok()) {
            a.espacios = (e & 3) as u8;
        }
        return;
    }
    if let Some(t) = leer(".marcas") {
        a.marcadores = lectura::marcadores_de_texto(t.trim());
    }
    if let Some(t) = leer(".maqueta") {
        match Maqueta::de_texto(&t) {
            Some(m) => {
                a.columna = m.columna.clamp(0, 20_000) as u32;
                a.tamano = lectura::tamano_valido(m.tamano.max(0) as u32);
                a.grosor = grosor_del_movil(m.grosor);
                a.tipo = tipo_del_movil(m.tipo);
            }
            // Vacia: la columna se solto (en el PC, al quitar toda la tinta).
            None => a.columna = 0,
        }
    }
    if let Some(s) = leer(".sitio").and_then(|t| t.trim().parse::<f32>().ok()).filter(|s| s.is_finite()) {
        a.sitio = s.clamp(0.0, 1.0);
    }
}

/// **Guarda lo que se recuerda de un documento**: junto a el, como
/// siempre, y lo que viaja en los ficheros de su mensaje. Solo se escribe
/// lo que cambia (cada fecha nueva es algo que mandar).
pub fn escribir(doc: &Path, a: &Ajustes) -> std::io::Result<()> {
    let junto = lectura::escribir(doc, a);
    if let Some(b) = base_de(doc) {
        escribir_lo_del_mensaje(&b, a, es_pdf(doc))?;
    }
    junto
}

fn escribir_lo_del_mensaje(b: &Base, a: &Ajustes, pdf: bool) -> std::io::Result<()> {
    // Lo que no esta y no dice nada no se crea: un fichero vacio mas en el
    // otro aparato por cada documento que solo se abrio.
    let poner = |t: &str, texto: String, nada: bool| -> std::io::Result<()> {
        let f = b.fichero(t);
        if nada && !f.exists() {
            return Ok(());
        }
        an::escribir(&f, &texto)
    };
    if pdf {
        let marcas = a.marcas.replace(['\n', '\r'], "");
        poner(".marcas", marcas.clone(), marcas.is_empty())?;
        return poner(".espacios", a.espacios.to_string(), a.espacios == 0);
    }
    let marcas = marcadores_a_texto(a);
    poner(".marcas", marcas.clone(), marcas.is_empty())?;
    let f = b.fichero(".maqueta");
    let actual = an::leer(&f).and_then(|t| Maqueta::de_texto(&t));
    if a.columna == 0 {
        // Sin columna, vacia y no borrada: borrada seria «no se sabe».
        if actual.is_some() {
            an::escribir(&f, "")?;
        }
    } else if let Some(m) = maqueta_para(a, actual) {
        an::escribir(&f, &m.a_texto())?;
    }
    let f = b.fichero(".sitio");
    let igual = an::leer(&f).and_then(|t| t.trim().parse::<f32>().ok()) == Some(a.sitio);
    if !igual {
        poner(".sitio", a.sitio.to_string(), a.sitio <= 0.0)?;
    }
    Ok(())
}

// ------------------------------------------------------------------ la tinta

/// Cuanto se corre la tinta del movil: su margen izquierdo (`izq`), o el
/// que el PC le pondria a esta columna si aun no hay maqueta.
fn corrida(b: &Base, a: &Ajustes) -> f32 {
    an::leer(&b.fichero(".maqueta"))
        .and_then(|t| Maqueta::de_texto(&t))
        .map(|m| m.izq)
        .unwrap_or_else(|| margen_de(a.columna as i32)) as f32
}

/// **La capa de tinta de un Word o un libro.** De un adjunto del chat, la de
/// su mensaje (con lo de antes pasado la primera vez); si no, la de junto al
/// documento.
pub fn leer_capa(doc: &Path) -> Capa {
    let vieja = lector_tinta::ruta_de_capa(doc);
    let Some(b) = base_de(doc).filter(|_| !es_pdf(doc)) else {
        return Capa::leer(&vieja);
    };
    let a = leer(doc);
    let dx = corrida(&b, &a);
    let nueva = b.tinta();
    if !nueva.exists() && vieja.is_file() {
        // La de antes esta en las unidades del PC: se escribe corrida.
        let mut c = Capa::leer(&vieja);
        c.sucia = true;
        match c.guardar_corrida(&nueva, dx) {
            Ok(()) => an::marcar_tinta_pasada(&vieja),
            Err(e) => tracing::warn!(?e, "no se pudo pasar la tinta de antes al mensaje"),
        }
    } else {
        // Si el movil ya mando la suya, la de aqui se junta (una vez): con
        // solo copiar si falta, lo anotado aqui antes no viajaba nunca.
        an::pasar_tinta_de_antes(&vieja, &nueva, dx as f64);
    }
    Capa::leer_corrida(&nueva, dx)
}

/// Guarda la capa donde [leer_capa] la leyo.
pub fn guardar_capa(doc: &Path, capa: &mut Capa) -> std::io::Result<()> {
    match base_de(doc).filter(|_| !es_pdf(doc)) {
        Some(b) => {
            let a = leer(doc);
            capa.guardar_corrida(&b.tinta(), corrida(&b, &a))
        }
        None => capa.guardar(&lector_tinta::ruta_de_capa(doc)),
    }
}

/// **La tinta de la hoja `i` de un PDF suelto** que es un adjunto del chat
/// (`anot-<uid>-p<i>`), con la de antes copiada la primera vez (mismas
/// unidades que el movil: se copia tal cual). `None` si no es un adjunto.
pub fn hoja_del_pdf(adjunto: Option<&(PathBuf, pixpin_proyecto::anotado::Adjunto)>, pdf: &Path, i: usize) -> Option<PathBuf> {
    use pixpin_sincro::disco::Disco;
    let (raiz, x) = adjunto?;
    let pagina = u32::try_from(i).ok()?;
    let rel = pixpin_sincro::anotado::rel(&pixpin_sincro::anotado::de_pagina(&x.uid, pagina), ".excalidraw.gz");
    let nueva = pixpin_proyecto::vista::DiscoPc::nuevo(raiz).ruta(&x.chat, &rel);
    let vieja = lector_tinta::ruta_de_hoja(pdf, i);
    if an::copiar_si_falta(&vieja, &nueva) {
        an::marcar_tinta_pasada(&vieja);
    } else {
        // La hoja que ya mando el movil se junta con la de aqui, una vez.
        an::pasar_tinta_de_antes(&vieja, &nueva, 0.0);
    }
    Some(nueva)
}

/// El adjunto del chat que es este PDF, una vez por lector abierto.
pub fn adjunto_del_pdf(pdf: &Path) -> Option<(PathBuf, pixpin_proyecto::anotado::Adjunto)> {
    let raiz = an::raiz_de(pdf)?;
    let x = an::adjunto_de(&raiz, pdf)?;
    Some((raiz, x))
}

/// **Los marcadores de un lienzo**: junto a su dibujo del movil si es un
/// lienzo de un chat (`<dibujo>.marcas`, que viaja con el), con los de antes
/// pasados la primera vez; si no, el fichero de siempre.
pub fn marcas_del_lienzo(lienzo: &Path, de_siempre: PathBuf) -> PathBuf {
    match an::marcas_del_lienzo(lienzo) {
        Some(nueva) => {
            an::copiar_si_falta(&de_siempre, &nueva);
            nueva
        }
        None => de_siempre,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::almacen::{self, Ficha, Indice};
    use pixpin_proyecto::cuaderno::{self, Clase, Mensaje, Sello};

    fn raiz(etiqueta: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("pixpin-anotado-app-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    /// Un almacen con un adjunto `nombre` en el chat de «Tesis».
    fn con_adjunto(r: &Path, nombre: &str, bytes: &[u8]) -> PathBuf {
        let ficha = Ficha::nueva("Tesis", 5, "PC01");
        let mut i = Indice::default();
        i.proyectos.push(ficha.clone());
        i.guardar(r).unwrap();
        let ruta = almacen::guardar_adjunto(r, &ficha.id, nombre, bytes).unwrap();
        let sello = Sello { cuando: 10, numero: 1, aparato: "PC01".into(), proyecto: ficha.id.clone() };
        let carpeta = almacen::carpeta(r, &ficha.id);
        cuaderno::anadir(&carpeta, &Mensaje::adjunto(Clase::Archivo, nombre, &ruta, 2, &sello)).unwrap();
        carpeta.join(&ruta)
    }

    fn base(doc: &Path) -> Base {
        base_de(doc).expect("es un adjunto del chat")
    }

    #[test]
    fn la_letra_del_movil_es_la_misma_en_el_pc_y_vuelve_igual() {
        // K16: las cuatro letras y los cuatro grosores del movil, tal cual.
        for g in 0..4u8 {
            assert_eq!(grosor_del_movil(grosor_al_movil(g)), g);
            assert_eq!(tipo_del_movil(tipo_al_movil(g)), g);
        }
        assert_eq!([0, 1, 2, 3].map(grosor_del_movil), [0, 1, 2, 3]);
        assert_eq!([0, 1, 2, 3].map(tipo_del_movil), [0, 1, 2, 3]);
        // Caso negativo: lo que no existe cae en el extremo mas cercano.
        assert_eq!([-1, 9].map(tipo_del_movil), [0, 3]);
        assert_eq!([-5, 7].map(grosor_del_movil), [0, 3]);
    }

    #[test]
    fn lo_que_llega_del_movil_sobre_un_word_se_lee_encima_de_lo_de_aqui() {
        let r = raiz("leer-word");
        let doc = con_adjunto(&r, "tesis.docx", b"PK");
        lectura::escribir(
            &doc,
            &Ajustes {
                columna: 860,
                zoom: 0.8,
                marcadores: lectura::marcadores_de_texto("7:0.1:X"),
                ..Ajustes::default()
            },
        )
        .unwrap();
        let b = base(&doc);
        an::escribir(&b.fichero(".maqueta"), "384,256,256,115,3,2").unwrap();
        an::escribir(&b.fichero(".marcas"), "1790218080412:0.34022403:P").unwrap();
        an::escribir(&b.fichero(".sitio"), "0.7799783").unwrap();
        let a = leer(&doc);
        assert_eq!((a.columna, a.tamano, a.grosor, a.tipo), (384, 115, 3, 2), "la columna y la letra del movil");
        assert_eq!(a.marcadores.len(), 1);
        assert_eq!(a.marcadores[0].emoji, "P");
        assert!((a.sitio - 0.7799783).abs() < 1e-6);
        assert_eq!(a.zoom, 0.8, "lo de esta pantalla sigue siendo de aqui");
        // Guardar sin cambiar la letra no reescribe la maqueta del movil:
        // sus margenes, su peso negro y su letra de ancho fijo se quedan.
        escribir(&doc, &a).unwrap();
        assert_eq!(an::leer(&b.fichero(".maqueta")).as_deref(), Some("384,256,256,115,3,2"));
        assert_eq!(an::leer(&b.fichero(".marcas")).as_deref(), Some("1790218080412:0.34022403:P"));
        assert_eq!(an::leer(&b.fichero(".sitio")).as_deref(), Some("0.7799783"));
        // Cambiada en el PC, se reescribe con los margenes que tenia.
        escribir(&doc, &Ajustes { tamano: 130, ..a.clone() }).unwrap();
        assert_eq!(an::leer(&b.fichero(".maqueta")).as_deref(), Some("384,256,256,130,3,2"));
        // Y sin columna (se quito toda la tinta), vacia, no borrada.
        escribir(&doc, &Ajustes { columna: 0, ..a }).unwrap();
        assert_eq!(an::leer(&b.fichero(".maqueta")).as_deref(), Some(""));
        assert_eq!(leer(&doc).columna, 0);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn lo_de_antes_de_un_word_pasa_una_vez_a_su_mensaje_y_no_se_borra() {
        let r = raiz("migrar-word");
        let doc = con_adjunto(&r, "acta.docx", b"PK");
        let antes = Ajustes {
            columna: 860,
            sitio: 0.6239229,
            marcadores: lectura::marcadores_de_texto("1790218080412:0.34022403:P"),
            ..Ajustes::default()
        };
        lectura::escribir(&doc, &antes).unwrap();
        // La tinta de antes, en las unidades del PC (desde el borde de la columna).
        let vieja = lector_tinta::ruta_de_capa(&doc);
        std::fs::create_dir_all(vieja.parent().unwrap()).unwrap();
        std::fs::write(
            &vieja,
            r#"{"type":"excalidraw","elements":[{"id":"pc1","type":"freedraw","x":10,"y":40,"width":2,"height":2,"points":[[0,0],[2,2]]}]}"#,
        )
        .unwrap();
        let capa = leer_capa(&doc);
        let b = base(&doc);
        assert_eq!(an::leer(&b.fichero(".maqueta")).as_deref(), Some("860,573,573,100,1,0"));
        assert_eq!(an::leer(&b.fichero(".marcas")).as_deref(), Some("1790218080412:0.34022403:P"));
        assert_eq!(an::leer(&b.fichero(".sitio")).as_deref(), Some("0.6239229"));
        // En el fichero del mensaje cuenta desde el borde de la pagina; en el
        // lector, igual que antes.
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(b.tinta()).unwrap()).unwrap();
        assert_eq!(v["elements"][0]["x"].as_f64(), Some(583.0));
        assert_eq!(capa.escena.visibles().next().unwrap().x, 10.0);
        assert!(vieja.is_file() && lectura::ruta_de_ajustes(&doc).is_file(), "lo viejo se queda");
        // La segunda vez no se pisa lo que ya tiene el mensaje.
        std::fs::write(&vieja, r#"{"type":"excalidraw","elements":[]}"#).unwrap();
        assert_eq!(leer_capa(&doc).escena.cuantos_visibles(), 1);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn lo_anotado_aqui_antes_se_junta_con_lo_que_ya_mando_el_movil_en_un_word_y_en_un_pdf() {
        // Queja del 28-sep: «lo que se modifica aqui no se envia». El movil
        // mando su tinta antes de que el lector del PC abriera el documento
        // y lo anotado aqui antes se quedaba junto al documento para siempre.
        let r = raiz("juntar");
        let doc = con_adjunto(&r, "biblio.docx", b"PK");
        let b = base(&doc);
        an::escribir(&b.fichero(".maqueta"), "384,256,256,100,1,0").unwrap();
        std::fs::create_dir_all(b.tinta().parent().unwrap()).unwrap();
        std::fs::write(
            b.tinta(),
            r#"{"elements":[{"id":"m1","type":"freedraw","x":300,"y":10,"width":4,"height":4,"seed":1,"points":[{"x":0.0,"y":0.0},{"x":4.0,"y":4.0}]}],"files":{}}"#,
        )
        .unwrap();
        let vieja = lector_tinta::ruta_de_capa(&doc);
        std::fs::create_dir_all(vieja.parent().unwrap()).unwrap();
        std::fs::write(
            &vieja,
            r#"{"type":"excalidraw","elements":[{"id":"pc1","type":"freedraw","x":10,"y":40,"width":0,"height":0,"points":[[0,0],[2,2]]}]}"#,
        )
        .unwrap();
        let capa = leer_capa(&doc);
        assert_eq!(capa.escena.cuantos_visibles(), 2, "lo del movil y lo de aqui");
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(b.tinta()).unwrap()).unwrap();
        assert_eq!(v["elements"][1]["x"].as_f64(), Some(266.0), "lo de aqui, corrido a la pagina");
        // Borrado despues, no vuelve al abrir otra vez.
        std::fs::write(b.tinta(), r#"{"elements":[]}"#).unwrap();
        assert_eq!(leer_capa(&doc).escena.cuantos_visibles(), 0);

        // Lo mismo con una hoja de un PDF, sin correr.
        let pdf = con_adjunto(&r, "plano.pdf", b"%PDF-1.4");
        let adjunto = adjunto_del_pdf(&pdf);
        let (raiz, x) = adjunto.clone().unwrap();
        let nueva = {
            use pixpin_sincro::disco::Disco;
            let rel = pixpin_sincro::anotado::rel(&pixpin_sincro::anotado::de_pagina(&x.uid, 2), ".excalidraw.gz");
            pixpin_proyecto::vista::DiscoPc::nuevo(&raiz).ruta(&x.chat, &rel)
        };
        std::fs::create_dir_all(nueva.parent().unwrap()).unwrap();
        std::fs::write(&nueva, r#"{"elements":[{"id":"m2","type":"freedraw","x":5,"y":5,"width":1,"height":1,"points":[{"x":0.0,"y":0.0},{"x":1.0,"y":1.0}]}]}"#).unwrap();
        let hoja_vieja = lector_tinta::ruta_de_hoja(&pdf, 2);
        std::fs::create_dir_all(hoja_vieja.parent().unwrap()).unwrap();
        std::fs::write(&hoja_vieja, r#"{"elements":[{"id":"pc1","type":"freedraw","x":70,"y":80,"points":[[0,0],[9,9]]}]}"#).unwrap();
        let h = hoja_del_pdf(adjunto.as_ref(), &pdf, 2).unwrap();
        assert_eq!(h, nueva);
        assert_eq!(Capa::leer(&h).escena.cuantos_visibles(), 2);
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&h).unwrap()).unwrap();
        assert_eq!(v["elements"][1]["x"].as_f64(), Some(70.0), "en un PDF las unidades son las mismas");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_tinta_del_movil_se_corre_su_margen_y_lo_que_no_se_toca_vuelve_a_su_numero_exacto() {
        let r = raiz("corrida");
        let doc = con_adjunto(&r, "libro.epub", b"PK");
        let b = base(&doc);
        an::escribir(&b.fichero(".maqueta"), "384,256,256,100,1,0").unwrap();
        std::fs::create_dir_all(b.tinta().parent().unwrap()).unwrap();
        std::fs::write(
            b.tinta(),
            r#"{"elements":[{"id":"m1","type":"freedraw","x":10.123456789,"y":10278.29,"width":4,"height":4,"seed":1,"version":1,"versionNonce":1,"updated":1,"points":[{"x":0.0,"y":0.0},{"x":4.0,"y":4.0}]}],"files":{}}"#,
        )
        .unwrap();
        let mut capa = leer_capa(&doc);
        let e = capa.escena.visibles().next().unwrap().clone();
        assert!((e.x - (10.123456789 - 256.0)).abs() < 1e-3, "{}", e.x);
        // Un trazo nuevo del PC, en el margen de la izquierda.
        let mut nuevo = e.clone();
        nuevo.x = -100.0;
        capa.escena.anadir(nuevo);
        capa.sucia = true;
        guardar_capa(&doc, &mut capa).unwrap();
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(b.tinta()).unwrap()).unwrap();
        let xs: Vec<f64> = v["elements"].as_array().unwrap().iter().map(|e| e["x"].as_f64().unwrap()).collect();
        assert_eq!(xs.len(), 2);
        assert!((xs[0] - 10.123456789).abs() < 1e-9, "el del movil, intacto: {xs:?}");
        assert_eq!(xs[1], 156.0, "el nuevo, corrido al borde de la pagina");
        // Caso negativo: un documento fuera del almacen, junto a el y sin correr.
        let fuera = r.join("fuera.docx");
        std::fs::write(&fuera, b"PK").unwrap();
        assert!(base_de(&fuera).is_none());
        let mut c = leer_capa(&fuera);
        c.escena.anadir(e);
        c.sucia = true;
        guardar_capa(&fuera, &mut c).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(lector_tinta::ruta_de_capa(&fuera)).unwrap()).unwrap();
        assert!(v["elements"][0]["x"].as_f64().unwrap() < 0.0, "sin correr");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn un_pdf_del_chat_lee_y_guarda_sus_marcadores_y_espacios_con_el_codigo_del_mensaje() {
        let r = raiz("pdf");
        let pdf = con_adjunto(&r, "plano.pdf", b"%PDF-1.4");
        // Lo de antes: marcas y espacios junto al PDF, y la tinta de la hoja 4.
        lectura::escribir(
            &pdf,
            &Ajustes { espacios: 2, marcas: "1:0.5:3.61:B".into(), pagina: 4.5, ..Ajustes::default() },
        )
        .unwrap();
        let hoja_vieja = lector_tinta::ruta_de_hoja(&pdf, 3);
        std::fs::create_dir_all(hoja_vieja.parent().unwrap()).unwrap();
        std::fs::write(&hoja_vieja, r#"{"elements":[]}"#).unwrap();
        let a = leer(&pdf);
        let b = base(&pdf);
        assert_eq!(an::leer(&b.fichero(".marcas")).as_deref(), Some("1:0.5:3.61:B"));
        assert_eq!(an::leer(&b.fichero(".espacios")).as_deref(), Some("2"));
        assert_eq!(a.pagina, 4.5);
        // Lo que manda el movil gana.
        an::escribir(&b.fichero(".espacios"), "1").unwrap();
        an::escribir(&b.fichero(".marcas"), "m1:0.5:2.25:E").unwrap();
        let a = leer(&pdf);
        assert_eq!((a.espacios, a.marcas.as_str()), (1, "m1:0.5:2.25:E"));
        escribir(&pdf, &Ajustes { espacios: 3, ..a }).unwrap();
        assert_eq!(an::leer(&b.fichero(".espacios")).as_deref(), Some("3"));
        // Basura en los espacios no pisa lo de aqui.
        an::escribir(&b.fichero(".espacios"), "x").unwrap();
        assert_eq!(leer(&pdf).espacios, 3);
        // La tinta de cada hoja, con el codigo del mensaje y la de antes copiada.
        let adjunto = adjunto_del_pdf(&pdf);
        let nueva = hoja_del_pdf(adjunto.as_ref(), &pdf, 3).unwrap();
        assert!(nueva.to_string_lossy().ends_with(&format!("{}-p3.excalidraw", b.base)), "{nueva:?}");
        assert!(nueva.is_file() && hoja_vieja.is_file());
        assert_eq!(hoja_del_pdf(None, &pdf, 3), None);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn un_pdf_que_no_es_del_chat_no_crea_nada_que_viaje() {
        let r = raiz("pdf-fuera");
        let del_chat = con_adjunto(&r, "otro.pdf", b"%PDF");
        let carpeta = del_chat.parent().unwrap().parent().unwrap().to_path_buf();
        // Dentro de la carpeta del chat pero sin mensaje que lo senale.
        let pdf = carpeta.join("archivos/suelto.pdf");
        std::fs::write(&pdf, b"%PDF").unwrap();
        escribir(&pdf, &Ajustes { espacios: 2, marcas: "1:0:0:E".into(), ..Ajustes::default() }).unwrap();
        assert_eq!(leer(&pdf).espacios, 2);
        assert!(adjunto_del_pdf(&pdf).is_none());
        assert!(!carpeta.join("android").exists(), "nada con codigo");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn los_marcadores_de_un_lienzo_del_chat_pasan_junto_a_su_dibujo_una_vez() {
        let r = raiz("lienzo");
        let doc = con_adjunto(&r, "x.docx", b"PK");
        let carpeta = doc.parent().unwrap().parent().unwrap().to_path_buf();
        let lienzo = carpeta.join("lienzos/d9.excalidraw");
        std::fs::create_dir_all(lienzo.parent().unwrap()).unwrap();
        let vieja = carpeta.join("lienzos/d9.excalidraw.pixpin-marcas");
        std::fs::write(&vieja, "k:10.0:20.0:H").unwrap();
        let nueva = marcas_del_lienzo(&lienzo, vieja.clone());
        assert_eq!(nueva, carpeta.join("android/pins/draw/d9.marcas"));
        assert_eq!(an::leer(&nueva).as_deref(), Some("k:10.0:20.0:H"));
        std::fs::write(&vieja, "otra").unwrap();
        marcas_del_lienzo(&lienzo, vieja.clone());
        assert_eq!(an::leer(&nueva).as_deref(), Some("k:10.0:20.0:H"), "no se pisa");
        // Caso negativo: fuera del almacen, el de siempre.
        let fuera = r.join("d.excalidraw");
        assert_eq!(
            marcas_del_lienzo(&fuera, r.join("d.excalidraw.pixpin-marcas")),
            r.join("d.excalidraw.pixpin-marcas")
        );
        let _ = std::fs::remove_dir_all(&r);
    }
}

/// **Con una copia de los datos del usuario** (nunca los de verdad): lo que
/// mando el movil v0.96 se ve en los lectores del PC. Se lanza a mano:
/// `PIXPIN_DATOS_COPIA=<copia> cargo test -p pixpin --bin pixpinmax con_los_datos_copiados -- --ignored --nocapture`.
#[cfg(test)]
mod con_datos_reales {
    use super::*;

    #[test]
    #[ignore = "necesita una copia de los datos del usuario"]
    fn con_los_datos_copiados_lo_del_movil_se_ve_en_los_lectores() {
        let raiz = PathBuf::from(std::env::var("PIXPIN_DATOS_COPIA").expect("PIXPIN_DATOS_COPIA"));
        let carpeta = raiz.join("proyectos/N5T7C46GZE/archivos");
        let docx = carpeta.join("Bibliografia_Papers_Tesis_Max.docx");
        let a = leer(&docx);
        let capa = leer_capa(&docx);
        let xs: Vec<f32> = capa.escena.visibles().map(|e| e.x).collect();
        println!("docx: base {:?}", base_de(&docx).map(|b| b.base));
        println!("docx: columna {} tamano {} sitio {} marcadores {}", a.columna, a.tamano, a.sitio, a.marcadores.len());
        println!("docx: {} trazos, x {:?}", xs.len(), xs);
        let pdf = carpeta.join("Transcripcion_Voz_260904_201352.pdf");
        let a = leer(&pdf);
        println!("pdf: base {:?} espacios {} marcas {:?}", base_de(&pdf).map(|b| b.base), a.espacios, a.marcas);
        let adj = adjunto_del_pdf(&pdf);
        for i in 0..7 {
            let h = hoja_del_pdf(adj.as_ref(), &pdf, i).unwrap();
            if h.is_file() {
                println!("pdf hoja {i}: {} trazos ({})", Capa::leer(&h).escena.cuantos_visibles(), h.display());
            }
        }
        let lienzo = raiz.join("proyectos/pr-1788574477233/lienzos/hoja-pr-1788574477233-0.excalidraw");
        let m = marcas_del_lienzo(&lienzo, lienzo.with_extension("excalidraw.pixpin-marcas"));
        println!("lienzo: {} -> {:?}", m.display(), std::fs::read_to_string(&m).ok());
        assert_eq!(base_de(&docx).unwrap().base, "anot-9KKUS3XF5W");
        assert_eq!(base_de(&pdf).unwrap().base, "anot-34FJGS8TT2");
    }
}
