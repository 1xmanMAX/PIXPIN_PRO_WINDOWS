//! **Una lista de tareas en la hoja de compartir** (1-nov-2026, el usuario:
//! «mejora la apariencia de mis quehaceres cuando los exporto»).
//!
//! Antes la mini-app `tareas` salia como cualquier texto: renglones con
//! `☐`/`☑` y las marcas crudas a la vista (`⏫`, `➕ 2026-10-28`, los
//! enlaces de las fotos). Ahora cada formato sale de lo ya entendido
//! (`pindesk_proyecto::tareas_exportar`):
//!
//! - **Pagina web**: la pagina propia de las tareas, con el resumen, las
//!   chapitas, las fotos dentro y lo hecho plegado (si todo lo marcado son
//!   listas; entre otras cosas, la lista va dibujada como las demas).
//! - **PDF, PNG, JPG y SVG**: las hojas A4 dibujadas con el mismo aire.
//! - **Word, Markdown y texto**, cuando la lista va sola.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use pindesk_frase::fechas::Momento;
use pindesk_proyecto::cuaderno::Mensaje;
use pindesk_proyecto::tareas_exportar::{self as te, ListaParaExportar, Rotulos};
use pindesk_store::Catalogo;

use super::{BLANCO, Entero, Extra, FuenteImagen, Pieza, Preparado, escribir, pagina};

/// El lado mayor de una foto dentro de la pagina: se ensena en miniatura
/// (y se abre en grande si se pulsa con el visor del navegador).
const LADO_FOTO: u32 = 360;

/// **Una lista lista para sacarla**: lo entendido y, de cada foto que esta
/// en este equipo, su fichero.
pub(crate) struct ListaPreparada {
    pub lista: ListaParaExportar,
    pub fotos: HashMap<String, PathBuf>,
}

/// Si el mensaje es una lista de tareas.
pub(crate) fn es_lista(m: &Mensaje) -> bool {
    m.miniapp.as_deref() == Some(pindesk_proyecto::mini::TAREAS)
}

/// La hora local de quien exporta.
pub(crate) fn ahora() -> Momento {
    Momento::de_ms_locales(pindesk_shell::entorno::ahora_local_ms())
}

/// Lo que dicen las paginas, del catalogo. Los patrones llevan sus huecos
/// entre llaves, que llena `tareas_exportar` (el catalogo no aisla las
/// variables, asi que pasan tal cual).
pub(crate) fn rotulos(t: &Catalogo) -> Rotulos {
    let con = |clave: &str, pares: &[(&str, &str)]| {
        let mut args = fluent_bundle::FluentArgs::new();
        for (k, v) in pares {
            args.set(*k, v.to_string());
        }
        t.t_args(clave, &args)
    };
    let fecha = [("dia", "{dia}"), ("mes", "{mes}"), ("ano", "{ano}")];
    Rotulos {
        idioma: t.t("grabacion-idioma"),
        tareas: t.t("tareas-exportar-titulo"),
        exportado: con("tareas-exportar-exportado", &[("fecha", "{fecha}")]),
        pendientes: t.t("tareas-exportar-pendientes"),
        hechas: t.t("tareas-exportar-hechas"),
        completado: t.t("tareas-exportar-completado"),
        pendiente: t.t("tareas-exportar-pendiente"),
        hecha: t.t("tareas-exportar-hecha"),
        todo_hecho: t.t("tareas-exportar-todo-hecho"),
        vacia: t.t("tareas-exportar-vacia"),
        urgente: t.t("tareas-urgente"),
        muy_urgente: t.t("tareas-muy-urgente"),
        recordatorio: t.t("tareas-exportar-recordatorio"),
        pasada: t.t("tareas-exportar-pasada"),
        creada: con("tareas-exportar-creada", &[("fecha", "{fecha}")]),
        hoy: t.t("tareas-selector-hoy"),
        manana: t.t("tareas-selector-manana"),
        ayer: t.t("chat-ayer"),
        de: con(
            "tareas-exportar-de",
            &[("hechas", "{hechas}"), ("total", "{total}")],
        ),
        foto: t.t("tareas-exportar-foto"),
        dias: std::array::from_fn(|i| t.t(&format!("tareas-dia-{i}"))),
        meses: std::array::from_fn(|i| t.t(&format!("chat-mes-{}", i + 1))),
        fecha: con("chat-fecha", &fecha),
        fecha_con_ano: con("chat-fecha-con-ano", &fecha),
    }
}

/// **Pone la lista** del mensaje `m` como paginas (y, sola, su Word, su
/// Markdown y su texto). Devuelve si puso alguna.
#[allow(clippy::too_many_arguments)] // lo preparado, de donde, la fila y si va sola
pub(crate) fn anadir(
    p: &mut Preparado,
    raiz: &Path,
    proyecto: &str,
    clave: &str,
    nombre: &str,
    m: &Mensaje,
    solo: bool,
    t: &Catalogo,
) -> bool {
    let lista = ListaParaExportar::del_documento(&m.texto, nombre);
    let r = rotulos(t);
    let ahora = ahora();
    let mut fotos = HashMap::new();
    for e in lista.tareas.iter().flat_map(|x| &x.imagenes) {
        if let Some(ruta) = crate::tareas::ruta_de_imagen(raiz, proyecto, e) {
            fotos.insert(e.clone(), ruta);
        }
    }
    let unas = std::slice::from_ref(&lista);
    let hojas = te::hojas(unas, &lista.titulo, &r, ahora, &mut |e| {
        let ruta = fotos.get(e)?;
        let (w, h) = pindesk_codec::imagen::medidas(ruta).ok()?;
        Some((
            p.imagen(FuenteImagen::Fichero(ruta.clone())),
            w as f32,
            h as f32,
        ))
    });
    for (i, h) in hojas.into_iter().enumerate() {
        let (k, n, nivel) = if i == 0 {
            (clave.to_string(), nombre.to_string(), 0)
        } else {
            (
                format!("{clave}|p{}", i + 1),
                format!("{} {}", t.t("compartir-pagina"), i + 1),
                1,
            )
        };
        let detalle = if i == 0 {
            t.t("compartir-tipo-tareas")
        } else {
            String::new()
        };
        p.pieza(pagina(&k, n, detalle, nivel), h, BLANCO, false);
        p.marcar(&k);
    }
    if solo {
        let base = super::nombre_de_fichero(&p.titulo);
        let md = te::markdown(unas, &lista.titulo, &r, ahora);
        if let Some((bytes, _)) = pindesk_notas::word::exportar(
            &md,
            &lista.titulo,
            &pindesk_notas::vista::Vista::default(),
            None,
            &|_: &str| None,
            "",
            pindesk_shell::entorno::ahora_utc_ms(),
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
        p.extras.push(Extra {
            id: "texto",
            clave: "compartir-texto",
            que: Entero::Escrito {
                fichero: format!("{base}.txt"),
                bytes: te::texto(unas, &lista.titulo, &r, ahora).into_bytes(),
            },
        });
    }
    p.rotulos_tareas = r;
    p.tareas
        .insert(clave.to_string(), ListaPreparada { lista, fotos });
    true
}

/// La lista de la que sale una pagina (`clave` o `clave|pN`).
fn lista_de<'a>(p: &'a Preparado, clave: &str) -> Option<(&'a str, &'a ListaPreparada)> {
    let base = clave.split_once("|p").map_or(clave, |(b, _)| b);
    p.tareas.get_key_value(base).map(|(k, l)| (k.as_str(), l))
}

/// Si todas las paginas marcadas son de listas de tareas: entonces la
/// pagina web es la de las tareas.
pub(crate) fn todas_son_listas(p: &Preparado, piezas: &[&Pieza]) -> bool {
    !piezas.is_empty()
        && piezas
            .iter()
            .all(|x| lista_de(p, &x.pagina.clave).is_some())
}

/// Una foto en miniatura, como `data:` (JPEG si es opaca).
fn miniatura(ruta: &Path) -> Option<String> {
    let mut img = pindesk_codec::cargar(ruta).ok()?;
    let lado = img.ancho.max(img.alto);
    if lado > LADO_FOTO {
        let k = LADO_FOTO as f32 / lado as f32;
        let (w, h) = (
            ((img.ancho as f32 * k).round() as u32).max(1),
            ((img.alto as f32 * k).round() as u32).max(1),
        );
        img = pindesk_codec::imagen::redimensionar(img, w, h).ok()?;
    }
    let (mime, bytes) = if img.pixeles.chunks_exact(4).all(|q| q[3] == 255) {
        (
            "image/jpeg",
            pindesk_codec::imagen::codificar_jpg(&img, 82).ok()?,
        )
    } else {
        ("image/png", pindesk_codec::codificar_png(&img).ok()?)
    };
    Some(format!(
        "data:{mime};base64,{}",
        pindesk_motor2d::exportar::base64(&bytes)
    ))
}

/// **La pagina web de las listas marcadas**, en `carpeta`.
pub(crate) fn web(
    p: &Preparado,
    piezas: &[&Pieza],
    base: &str,
    carpeta: &Path,
) -> Result<Vec<PathBuf>> {
    let mut vistas: Vec<&str> = Vec::new();
    let mut listas: Vec<&ListaPreparada> = Vec::new();
    for x in piezas {
        if let Some((k, l)) = lista_de(p, &x.pagina.clave)
            && !vistas.contains(&k)
        {
            vistas.push(k);
            listas.push(l);
        }
    }
    let solo: Vec<ListaParaExportar> = listas.iter().map(|l| l.lista.clone()).collect();
    let titulo = match solo.as_slice() {
        [una] if !una.titulo.trim().is_empty() => una.titulo.clone(),
        _ => p.titulo.clone(),
    };
    let foto = |e: &str| {
        listas
            .iter()
            .find_map(|l| l.fotos.get(e))
            .and_then(|r| miniatura(r))
    };
    let pagina = te::html(&solo, &titulo, &p.rotulos_tareas, ahora(), &foto);
    let ruta = carpeta.join(format!("{base}.html"));
    escribir(&ruta, pagina.as_bytes())?;
    Ok(vec![ruta])
}
