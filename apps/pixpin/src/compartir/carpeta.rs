//! **Una carpeta de paginas como PDF** (26-oct-2026, el usuario: «al
//! exportar, sale como un PDF: una pagina por elemento, y de cada lienzo solo
//! lo que va sobre la plantilla o sobre el marco; si no hay ninguno, todo lo
//! dibujado»).
//!
//! No es otro exportador: es la misma reunion de hojas de compartir
//! (`anadir_mensaje`) con una regla distinta para los lienzos —la de
//! `exportar::hojas_de_cuaderno`, la hoja de la plantilla, si no los marcos,
//! si no todo— y una sola pagina por foto. Lo escribe el escritor de PDF de
//! siempre, con el titulo de cada pagina («Dia 1», «Dia 2») en el indice de
//! marcadores del PDF.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use pixpin_motor2d::Escena;
use pixpin_motor2d::exportar::{self as ex, ID_PAPEL};
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_store::Catalogo;

use super::{
    BLANCO, Entero, Extra, FuenteImagen, Lector, Pieza, Preparado, anadir_mensaje, lienzo_de_hoja,
    pagina, papel_de_pagina, tapar_para_compartir,
};

/// Las hojas de un lienzo de la carpeta, con la regla del cuaderno.
/// Devuelve si puso alguna.
fn anadir_lienzo(
    p: &mut Preparado,
    clave: &str,
    escena: &Escena,
    papel: Option<(FuenteImagen, f32, f32)>,
    fotos: &dyn Fn(u64) -> Option<FuenteImagen>,
) -> bool {
    // Lo pixelado sale pixelado, como al compartir (`tapar_para_compartir`).
    let tapada = tapar_para_compartir(escena, papel.as_ref(), fotos);
    let escena = tapada.as_ref().map_or(escena, |(e, _)| e);
    let tam_papel = papel.as_ref().map(|(_, w, h)| (*w, *h));
    let fuente = |id: u64| {
        if let Some(img) = tapada.as_ref().and_then(|(_, m)| m.get(&id)) {
            return Some(FuenteImagen::Cargada(img.clone()));
        }
        if id == ID_PAPEL {
            papel.as_ref().map(|(f, _, _)| f.clone())
        } else {
            fotos(id)
        }
    };
    let mut ids = HashMap::new();
    let hojas = ex::hojas_de_cuaderno(escena, tam_papel);
    let puestas = !hojas.is_empty();
    for (k, mut h) in hojas.into_iter().enumerate() {
        p.traducir(&mut h, &mut ids, &fuente);
        p.pieza(
            pagina(format!("{clave}|{k}"), String::new(), String::new(), 0),
            h,
            BLANCO,
            false,
        );
    }
    puestas
}

/// Las paginas de un mensaje de la carpeta. Un lienzo, por su plantilla; lo
/// demas, como al compartirlo entre otros mensajes, y una foto en una sola
/// pagina (sin los marcos que lleve dibujados encima).
fn anadir_pagina(p: &mut Preparado, raiz: &Path, proyecto: &str, m: &Mensaje, t: &Catalogo) {
    let antes = p.piezas.len();
    match m.clase {
        Some(Clase::Dibujo) | Some(Clase::Pagina) => {
            let papel = m.pagina.and_then(|n| papel_de_pagina(raiz, proyecto, n));
            let referencia = m
                .referencia
                .as_deref()
                .filter(|r| !r.is_empty())
                // Una pagina del PDF que ya estreno su dibujo en el
                // `proyecto.json`: con el encima, como en el chat.
                .map(str::to_string)
                .or_else(|| {
                    crate::pdf_en_chat::es_pagina_sin_dibujo(m)
                        .then(|| crate::pdf_en_chat::dibujo_de_la_hoja(raiz, proyecto, m))
                        .flatten()
                });
            let (escena, fotos) = match referencia.as_deref() {
                Some(id) => match lienzo_de_hoja(raiz, proyecto, id) {
                    Some(v) => v,
                    None if papel.is_some() => (Escena::nueva(), HashMap::new()),
                    None => return,
                },
                None if papel.is_some() => (Escena::nueva(), HashMap::new()),
                None => return,
            };
            let fuente = |id: u64| fotos.get(&id).cloned().map(FuenteImagen::Fichero);
            anadir_lienzo(p, &m.id, &escena, papel, &fuente);
        }
        Some(Clase::Imagen) => {
            if anadir_mensaje(p, raiz, proyecto, m, false, t) {
                p.piezas.truncate(antes + 1);
            }
        }
        _ => {
            anadir_mensaje(p, raiz, proyecto, m, false, t);
        }
    }
}

/// **El PDF de una carpeta**: sus paginas `(mensaje, titulo)` en orden, una
/// (o las que salgan de su plantilla) por cada una. Error si no sale
/// ninguna pagina.
pub(crate) fn pdf(
    raiz: &Path,
    proyecto: &str,
    titulo: &str,
    paginas: &[(Mensaje, String)],
    t: &Catalogo,
) -> Result<Vec<u8>> {
    let mut p = Preparado::nuevo(titulo);
    let mut indice = Vec::new();
    for (m, nombre) in paginas {
        let antes = p.piezas.len();
        anadir_pagina(&mut p, raiz, proyecto, m, t);
        if let Some(primera) = p.piezas.get(antes) {
            indice.push(pixpin_pdf::escribir::Marcador {
                titulo: nombre.clone(),
                hoja: antes,
                y: primera.hoja.caja.1,
            });
        }
        for x in &mut p.piezas[antes..] {
            x.pagina.nombre = nombre.clone();
        }
    }
    anyhow::ensure!(
        !p.piezas.is_empty(),
        "la carpeta no tiene nada que exportar"
    );
    let lector = Lector {
        fuentes: &p.imagenes,
        pdfs: Default::default(),
    };
    let piezas: Vec<&Pieza> = p.piezas.iter().collect();
    super::pdf::de_piezas_con_indice(&piezas, &lector, &indice).context("sin paginas")
}

// --- Compartir el cuaderno con todos los formatos -------------------------------

/// **Las paginas vivas de la carpeta `carpeta`**, `(mensaje, titulo)` en su
/// orden (en una grabacion, lo adjuntado detras), con `todos` el chat
/// entero. `None` si el mensaje no es una carpeta que se entienda.
pub(crate) fn paginas_de(carpeta: &Mensaje, todos: &[Mensaje]) -> Option<Vec<(Mensaje, String)>> {
    let c = pixpin_proyecto::carpeta_de_paginas::Carpeta::de_mensaje(carpeta)?;
    let mut vivas = c.vivas(todos);
    if c.es_grabacion() {
        vivas.sort_by_key(|(_, p)| p.adjunto);
    }
    Some(
        vivas
            .into_iter()
            .map(|(i, p)| {
                let m = todos[i].clone();
                let titulo = if p.titulo.trim().is_empty() {
                    pixpin_docs::sin_extension(m.nombre.trim())
                } else {
                    p.titulo.clone()
                };
                (m, titulo)
            })
            .collect(),
    )
}

/// **Lo marcado en el chat, con las carpetas abiertas** (28-oct-2026, como
/// `CompartirCarpeta.conSusPaginas` del movil): una carpeta marcada junto a
/// otros mensajes va con sus paginas (las que no esten ya), y no con el JSON
/// de su documento.
pub(crate) fn con_sus_paginas(lista: &[Mensaje], todos: &[Mensaje]) -> Vec<Mensaje> {
    let mut vistos = std::collections::HashSet::new();
    lista
        .iter()
        .flat_map(|m| {
            if pixpin_proyecto::carpeta_de_paginas::es_carpeta(m) {
                paginas_de(m, todos)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(p, _)| p)
                    .collect()
            } else {
                vec![m.clone()]
            }
        })
        .filter(|m| vistos.insert(m.id.clone()))
        .collect()
}

/// El texto de una pagina que lo lleva: una nota escrita o la transcripcion
/// de una nota de voz. Una mini-app o una foto, no.
fn texto_de(m: &Mensaje) -> Option<String> {
    let texto = match m.clase {
        None | Some(Clase::Nota) if m.miniapp.is_none() => m.texto.trim().to_string(),
        Some(Clase::Voz) => m.transcripcion.as_deref().unwrap_or("").trim().to_string(),
        _ => return None,
    };
    (!texto.is_empty()).then_some(texto)
}

/// **El Markdown de las notas del cuaderno**: el titulo del cuaderno arriba
/// y cada pagina con texto bajo su titulo. `None` si ninguna lleva texto.
pub(crate) fn markdown(titulo: &str, paginas: &[(Mensaje, String)]) -> Option<String> {
    let trozos: Vec<String> = paginas
        .iter()
        .filter_map(|(m, t)| Some(format!("## {t}\n\n{}", texto_de(m)?)))
        .collect();
    if trozos.is_empty() {
        return None;
    }
    Some(format!("# {titulo}\n\n{}\n", trozos.join("\n\n")))
}

/// El mismo texto sin las marcas de los titulos, para el `.txt`.
fn texto_llano(titulo: &str, paginas: &[(Mensaje, String)]) -> Option<String> {
    let trozos: Vec<String> = paginas
        .iter()
        .filter_map(|(m, t)| Some(format!("{t}\n\n{}", texto_de(m)?)))
        .collect();
    if trozos.is_empty() {
        return None;
    }
    Some(format!("{titulo}\n\n{}\n", trozos.join("\n\n")))
}

/// **Un cuaderno en la hoja de compartir, con todos sus formatos**
/// (29-oct-2026, como el movil 201d327; el usuario: «mejora el compartir de
/// los cuadernos para que anada todos los formatos que tienen los demas:
/// PDF, HTML, PixPin, fotos… Esa interfaz tiene que estar siempre
/// unificada»). Se usa desde su ventana, desde «Compartir» en el menu de su
/// mensaje del chat y con lo marcado:
///
/// - **PDF, pagina web, PNG, JPG y SVG** de sus paginas, con la regla del
///   cuaderno (un lienzo, por su plantilla o sus marcos; una foto, en una
///   sola pagina) y el titulo de cada una («Dia 1», «Dia 2»).
/// - **Word, Markdown y Texto**, si alguna pagina lleva texto (una nota, la
///   transcripcion de una nota de voz), cada uno bajo su titulo.
/// - **Original**: los ficheros de sus paginas tal cual (fotos, PDF, audios).
/// - **PixPin (.pixpin)**: el chat entero con la carpeta, que se abre en
///   otro PC y en el movil con su carpeta.
///
/// La carpeta de una grabacion no pasa por aqui: se comparte como la
/// grabacion que es (ver `de_mensajes`).
pub(crate) fn preparar(
    raiz: &Path,
    proyecto: &str,
    carpeta: &Mensaje,
    todos: &[Mensaje],
    t: &Catalogo,
) -> Option<Preparado> {
    let paginas = paginas_de(carpeta, todos)?;
    let titulo = match carpeta.nombre.trim() {
        "" => t.t("compartir-sin-nombre"),
        n => n.to_string(),
    };
    let mut p = Preparado::nuevo(titulo.clone());
    for (m, nombre) in &paginas {
        let antes = p.piezas.len();
        anadir_pagina(&mut p, raiz, proyecto, m, t);
        if !nombre.is_empty() {
            for x in &mut p.piezas[antes..] {
                x.pagina.nombre = nombre.clone();
            }
        }
    }
    // Todas sus paginas marcadas al abrir: el cuaderno es lo que se comparte
    // (una nota marcaria solo la suya).
    p.marcadas = None;
    let base = super::nombre_de_fichero(&titulo);
    if let Some(md) = markdown(&titulo, &paginas) {
        let resolver = |_: &str| None;
        if let Some((bytes, _)) = pixpin_notas::word::exportar(
            &md,
            &titulo,
            &pixpin_notas::vista::Vista::default(),
            None,
            &resolver,
            "",
            pixpin_shell::entorno::ahora_utc_ms(),
        ) {
            p.extras.push(Extra {
                id: "word",
                clave: "compartir-word",
                que: Entero::Escrito {
                    fichero: format!("{base}.docx"),
                    bytes,
                },
            });
        }
        p.extras.push(Extra {
            id: "md",
            clave: "compartir-md",
            que: Entero::Escrito {
                fichero: format!("{base}.md"),
                bytes: md.into_bytes(),
            },
        });
    }
    if let Some(txt) = texto_llano(&titulo, &paginas) {
        p.extras.push(Extra {
            id: "texto",
            clave: "compartir-texto",
            que: Entero::Escrito {
                fichero: format!("{base}.txt"),
                bytes: txt.into_bytes(),
            },
        });
    }
    let mensajes: Vec<Mensaje> = paginas.into_iter().map(|(m, _)| m).collect();
    let rutas = super::originales(raiz, proyecto, &mensajes);
    p.adjuntos = super::adjuntos::de_originales(&rutas);
    if !rutas.is_empty() {
        let (id, clave) = if rutas.len() == 1 {
            ("original", "compartir-original")
        } else {
            ("originales", "compartir-originales")
        };
        super::anadir_original(&mut p, id, clave, rutas);
    }
    super::anadir_pixpin_del_chat(&mut p, raiz, proyecto);
    Some(p)
}

/// El nombre del fichero que se sugiere al guardar: el de la carpeta.
pub(crate) fn nombre_de_fichero(titulo: &str) -> String {
    // Sin titulo, el de serie (el de compartir pondria «dibujo»).
    if titulo.trim().is_empty() {
        return "Carpeta.pdf".into();
    }
    let base = super::nombre_de_fichero(titulo);
    if base.trim().is_empty() {
        "Carpeta.pdf".into()
    } else {
        format!("{base}.pdf")
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn textos() -> Catalogo {
        Catalogo::nuevo(pixpin_store::Idioma::Espanol)
    }

    fn raiz_temporal(etiqueta: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-carpeta-pdf-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Un lienzo del chat con esta escena, escrito en el proyecto.
    fn lienzo_con(raiz: &Path, proyecto: &str, id: &str, escena: &Escena) -> Mensaje {
        let ruta = pixpin_proyecto::almacen::lienzo(raiz, proyecto, id);
        std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
        let l = pixpin_motor2d::excalidraw::con_escena(
            &pixpin_motor2d::excalidraw::Lienzo::vacio(),
            escena,
        );
        std::fs::write(&ruta, pixpin_motor2d::excalidraw::escribir(&l)).unwrap();
        Mensaje {
            id: format!("m-{id}"),
            clase: Some(Clase::Dibujo),
            referencia: Some(id.into()),
            ..Default::default()
        }
    }

    fn diario() -> pixpin_motor2d::biblioteca::FiguraGuardada {
        pixpin_motor2d::figuras_de_serie::catalogo()
            .into_iter()
            .find(|f| f.id == "serie-diario")
            .unwrap()
    }

    #[test]
    fn una_pagina_por_lienzo_y_de_cada_uno_su_plantilla() {
        let raiz = raiz_temporal("plantilla");
        let f = diario();
        let (w, h) = f.tamano();
        let mut dia1 = Escena::nueva();
        pixpin_motor2d::biblioteca::estampar_en_escena(
            &mut dia1,
            &f,
            pixpin_motor2d::vector::Punto2::nuevo(w / 2.0, h / 2.0),
        );
        // Algo lejos de la hoja: no sale.
        dia1.anadir(pixpin_motor2d::Elemento {
            x: 9000.0,
            y: 9000.0,
            ancho: 30.0,
            alto: 30.0,
            figura: pixpin_motor2d::Figura::Rectangulo,
            ..Default::default()
        });
        let dia2 = Escena::nueva();
        let a = lienzo_con(&raiz, "p", "d1", &dia1);
        let b = lienzo_con(&raiz, "p", "d2", &dia2);
        let nota = Mensaje {
            id: "n".into(),
            clase: Some(Clase::Nota),
            texto: "Lo que paso entre medias".into(),
            ..Default::default()
        };
        let bytes = pdf(
            &raiz,
            "p",
            "Diario",
            &[
                (a, "Día 1".into()),
                (nota, "Nota".into()),
                (b, "Día 2".into()),
            ],
            &textos(),
        )
        .unwrap();
        let texto = String::from_utf8_lossy(&bytes);
        assert!(texto.starts_with("%PDF"));
        // El lienzo vacio no da pagina; el diario una y la nota otra.
        assert_eq!(texto.matches("/Type /Page ").count(), 2, "dos paginas");
        assert!(texto.contains("/Outlines"), "con el indice de titulos");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn una_carpeta_sin_nada_que_exportar_es_un_error() {
        let raiz = raiz_temporal("vacia");
        // Caso negativo: un lienzo que no esta en el disco no da paginas.
        let fantasma = Mensaje {
            id: "x".into(),
            clase: Some(Clase::Dibujo),
            referencia: Some("no-esta".into()),
            ..Default::default()
        };
        assert!(pdf(&raiz, "p", "X", &[(fantasma, "Día 1".into())], &textos()).is_err());
        assert!(pdf(&raiz, "p", "X", &[], &textos()).is_err());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// Un cuaderno con un lienzo y una nota, y el mensaje de su carpeta.
    fn cuaderno(raiz: &Path) -> (Mensaje, Vec<Mensaje>) {
        let mut escena = Escena::nueva();
        escena.anadir(pixpin_motor2d::Elemento {
            x: 10.0,
            y: 10.0,
            ancho: 60.0,
            alto: 40.0,
            figura: pixpin_motor2d::Figura::Rectangulo,
            ..Default::default()
        });
        let lienzo = lienzo_con(raiz, "p", "d1", &escena);
        let nota = Mensaje {
            id: "n".into(),
            clase: Some(Clase::Nota),
            texto: "Lo que paso entre medias".into(),
            ..Default::default()
        };
        let mut c = pixpin_proyecto::carpeta_de_paginas::Carpeta::default();
        c.anadir(&lienzo.id, "Día 1");
        c.anadir(&nota.id, "Día 2");
        let carpeta = Mensaje {
            id: "c".into(),
            clase: Some(Clase::MiniApp),
            miniapp: Some(pixpin_proyecto::carpeta_de_paginas::MINIAPP.into()),
            nombre: "Diario".into(),
            texto: c.escribir(),
            ..Default::default()
        };
        let todos = vec![lienzo, nota, carpeta.clone()];
        (carpeta, todos)
    }

    /// El caso del usuario (28-oct-2026): un cuaderno se comparte con todos
    /// los formatos, no con el JSON de su documento.
    #[test]
    fn un_cuaderno_se_comparte_con_sus_paginas_y_los_formatos_de_texto() {
        let raiz = raiz_temporal("compartir");
        let (carpeta, todos) = cuaderno(&raiz);
        let p = preparar(&raiz, "p", &carpeta, &todos, &textos()).expect("es una carpeta");
        assert_eq!(p.titulo, "Diario");
        let nombres: Vec<&str> = p.piezas.iter().map(|x| x.pagina.nombre.as_str()).collect();
        assert_eq!(nombres, ["Día 1", "Día 2"], "cada pagina con su titulo");
        assert!(p.marcadas.is_none(), "todas marcadas al abrir");
        let ids: Vec<&str> = p.extras.iter().map(|e| e.id).collect();
        for id in ["word", "md", "texto"] {
            assert!(ids.contains(&id), "{id} en {ids:?}");
        }
        let md = markdown("Diario", &paginas_de(&carpeta, &todos).unwrap()).unwrap();
        assert!(md.starts_with("# Diario"), "{md}");
        assert!(md.contains("## Día 2\n\nLo que paso entre medias"), "{md}");
        assert!(!md.contains("Día 1"), "un lienzo no aporta texto: {md}");
        let c = super::super::compartible(&p, &textos());
        let formatos: Vec<&str> = c.formatos.iter().map(|f| f.id.as_str()).collect();
        assert!(
            formatos.contains(&"pdf") && formatos.contains(&"png"),
            "{formatos:?}"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn lo_marcado_con_un_cuaderno_lleva_sus_paginas_sin_repetir() {
        let raiz = raiz_temporal("marcado");
        let (carpeta, todos) = cuaderno(&raiz);
        let nota = todos[1].clone();
        let abiertos = con_sus_paginas(&[nota, carpeta], &todos);
        let ids: Vec<&str> = abiertos.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["n", "m-d1"]);
        // Caso negativo: un mensaje que no es carpeta no es un cuaderno.
        assert!(preparar(&raiz, "p", &todos[1], &todos, &textos()).is_none());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_fichero_se_llama_como_la_carpeta() {
        assert_eq!(nombre_de_fichero("Diario de campo"), "Diario de campo.pdf");
        assert_eq!(nombre_de_fichero("  "), "Carpeta.pdf");
    }
}
