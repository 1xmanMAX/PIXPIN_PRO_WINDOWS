//! **«Guardar» desde lo abierto de fuera** (27-oct-2026, Android 15a52aed,
//! «Modo de paso: abrir archivos de fuera en su visor sin guardarlos»).
//!
//! Un fichero abierto desde el Explorador, con «Abrir con PixPin» o con la
//! vista rapida (el espacio) se lee **de paso**: no es de ningun chat, y lo
//! que se anota encima se queda en este PC, junto a el (`<doc>.pixpin-*`) o
//! en la cache (la capa de un plano, los marcadores de un plano o un modelo).
//! Lo de paso no viaja.
//!
//! El boton «Guardar» de su visor (la barra de los lectores como pin,
//! `lector_pin`, y la de los planos y modelos de `pixpin-cad`) pregunta
//! donde, con un menu corto: «Mensajes guardados» primero y despues los
//! chats por el ultimo tocado. Al elegir:
//!
//! 1. el fichero se copia al chat y entra como un mensaje mas, con su codigo
//!    (`ventana_chat::meter_en_proyecto`, el camino de soltar un fichero);
//! 2. lo anotado de paso pasa a los ficheros de ese mensaje nuevo: lo que
//!    cuelga del nombre va junto a la copia, con el nombre de la copia
//!    ([`al_sitio_nuevo`]), y lo de la cache directamente a `anot-<uid>…`
//!    del codigo nuevo (`compartir::exportable::lo_anotado_del_suelto`, la
//!    misma cuenta del `.pixpin` de un fichero suelto);
//! 3. cada visor pasa en el acto lo de junto a la copia a `anot-<uid>…`
//!    con sus unidades ([`pasar_al_mensaje`]), lo mismo que haria al
//!    abrirlo desde el chat la primera vez. Asi, despues, solo viajan el
//!    mensaje y sus `anot-<uid>` de siempre.
//!
//! Lo de paso se queda donde estaba (como en `migrar`: lo viejo no se borra),
//! y el fichero de fuera no se toca.

use std::path::{Path, PathBuf};

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::Mensaje;
use pixpin_store::Catalogo;

/// Cuantos chats salen en el menu, ademas de «Mensajes guardados».
pub const CHATS_EN_EL_MENU: usize = 12;

/// **De que visor se guarda**: cada uno pasa su tinta con sus unidades.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visor {
    /// Word, EPUB, texto (`visor`): tinta corrida por la columna, maqueta.
    Documento,
    /// PDF y las paginas de Office (`lector_pdf`): una tinta por hoja.
    Pdf,
    /// Libro de calculo (`visor_hoja`): una tinta por hoja del libro.
    Libro,
    /// Pagina web o nota (`visor_html`, el lector de notas): tinta en
    /// pixeles de la pagina.
    Pagina,
    /// Plano o modelo (`pixpin-cad`): todo va ya en `anot-<uid>…`.
    Plano,
    /// Lo que no anota (la vista previa de Office).
    Otro,
}

/// **Si `ruta` se esta leyendo de paso**: un fichero de fuera del almacen,
/// que no es de ningun chat (`renombrar_doc::Dueno::Suelto`).
pub fn es_de_paso(raiz: &Path, ruta: &Path) -> bool {
    ruta.is_file()
        && matches!(
            crate::renombrar_doc::dueno(raiz, ruta),
            crate::renombrar_doc::Dueno::Suelto
        )
}

/// Un sitio del menu: el chat (`None`: «Mensajes guardados», que se crea si
/// aun no esta) y su nombre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destino {
    pub chat: Option<String>,
    pub nombre: String,
}

/// **Los sitios del menu**, puro: «Mensajes guardados» primero (aunque aun
/// no exista) y luego hasta `max` chats por el ultimo tocado, sin el del
/// Asistente (lo que se escribe alli es para el modelo).
pub fn destinos(indice: &Indice, max: usize) -> Vec<Destino> {
    let guardados = indice.proyectos.iter().find(|f| f.es_guardados());
    let mut v = vec![Destino {
        chat: guardados.map(|f| f.id.clone()),
        nombre: guardados.map_or_else(
            || almacen::NOMBRE_GUARDADOS.to_string(),
            |f| f.nombre.clone(),
        ),
    }];
    v.extend(
        indice
            .ordenadas()
            .into_iter()
            .filter(|f| !f.es_guardados() && !f.es_asistente())
            .take(max)
            .map(|f| Destino {
                chat: Some(f.id.clone()),
                nombre: f.nombre.clone(),
            }),
    );
    v
}

/// **Donde va en el chat algo de lo anotado**, puro: lo que colgaba del
/// nombre del fichero de fuera (`archivos/<viejo>.pixpin-…`, como lo da
/// `lo_anotado_del_suelto`) cuelga del de la copia (`ruta_nueva`, la del
/// mensaje: `archivos/<nuevo>`, que puede llevar un `(1)`). Lo demas (los
/// `anot-<uid>…`, ya con el codigo nuevo) va tal cual.
pub fn al_sitio_nuevo(rel: &str, nombre_viejo: &str, ruta_nueva: &str) -> String {
    match rel.strip_prefix(&format!("archivos/{nombre_viejo}.pixpin-")) {
        Some(resto) => format!("{ruta_nueva}.pixpin-{resto}"),
        None => rel.to_string(),
    }
}

/// La hoja (desde 0) de una tinta `hoja-<n>.excalidraw` (con `n` desde 1,
/// como la escribe `lector_tinta::ruta_de_hoja`), puro.
pub fn hoja_de(nombre: &str) -> Option<usize> {
    let n: usize = nombre
        .strip_prefix("hoja-")?
        .strip_suffix(".excalidraw")?
        .parse()
        .ok()?;
    n.checked_sub(1)
}

/// Las hojas con tinta junto a `doc` (`<doc>.pixpin-anotado/hoja-<n>…`).
fn hojas_anotadas(doc: &Path) -> Vec<usize> {
    let mut v: Vec<usize> = std::fs::read_dir(crate::lector_tinta::carpeta_de(doc))
        .map(|d| {
            d.flatten()
                .filter_map(|e| hoja_de(&e.file_name().to_string_lossy()))
                .collect()
        })
        .unwrap_or_default();
    v.sort_unstable();
    v
}

/// Lo que quedo guardado.
#[derive(Debug, Clone)]
pub struct Guardado {
    pub chat: String,
    pub mensaje: Mensaje,
    /// La copia, dentro del chat.
    pub ruta: PathBuf,
}

/// **Guarda `original` en `chat`** (`None`: «Mensajes guardados») con lo
/// anotado de paso. `anotado`: lo que el visor tiene fuera de lo que cuelga
/// del nombre (la capa de un plano en la cache).
pub fn guardar(
    raiz: &Path,
    chat: Option<&str>,
    original: &Path,
    visor: Visor,
    anotado: &[PathBuf],
) -> std::io::Result<Guardado> {
    let aparato = crate::letras_que_viajan::aparato(raiz);
    let chat = match chat {
        Some(c) => c.to_string(),
        None => {
            almacen::asegurar_guardados(raiz, pixpin_shell::entorno::ahora_utc_ms(), &aparato)?.id
        }
    };
    let nombre = original
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| std::io::Error::other("el fichero no tiene nombre"))?;
    let bytes = std::fs::read(original)?;
    let mensaje =
        crate::ventana_chat::meter_en_proyecto(raiz, &chat, &[(nombre.clone(), bytes)], &aparato)?
            .pop()
            .ok_or_else(|| std::io::Error::other("no entro en el chat"))?;
    let (Some(uid), Some(rel)) = (mensaje.uid.clone(), mensaje.ruta.clone()) else {
        return Err(std::io::Error::other("el mensaje no tiene codigo ni ruta"));
    };
    let carpeta = almacen::carpeta(raiz, &chat);
    let ruta = carpeta.join(&rel);
    let mut pasados = 0;
    for (de, b) in
        crate::compartir::exportable::lo_anotado_del_suelto(Some(raiz), original, anotado, &uid)
    {
        let destino = carpeta.join(al_sitio_nuevo(&de, &nombre, &rel));
        // Un mensaje recien nacido no tiene nada: si algo esta, no se pisa.
        if destino.exists() {
            continue;
        }
        if let Some(p) = destino.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(&destino, b)?;
        pasados += 1;
    }
    pasar_al_mensaje(&ruta, visor);
    crate::ventana_chat::refrescar();
    tracing::info!(
        chat = %chat,
        uid = %uid,
        pasados,
        de = %original.display(),
        "de paso: guardado en el chat con lo anotado"
    );
    Ok(Guardado {
        chat,
        mensaje,
        ruta,
    })
}

/// **Lo de junto a la copia, a los ficheros de su mensaje**, con las
/// unidades de cada visor: lo mismo que hace el visor al abrir un adjunto
/// la primera vez (`anotado_del_adjunto`, `marcos_doc`).
pub fn pasar_al_mensaje(doc: &Path, visor: Visor) {
    use crate::anotado_del_adjunto as ad;
    match visor {
        Visor::Documento => {
            let _ = ad::leer(doc);
            let _ = ad::leer_capa(doc);
        }
        Visor::Pdf => {
            let _ = ad::leer(doc);
            let adjunto = ad::adjunto_del_pdf(doc);
            for i in hojas_anotadas(doc) {
                let _ = ad::hoja_del_pdf(adjunto.as_ref(), doc, i);
            }
        }
        Visor::Libro => {
            for i in hojas_anotadas(doc) {
                let _ = ad::tinta_de_la_hoja_del_libro(doc, i);
            }
        }
        Visor::Pagina => {
            let _ = ad::tinta_de_la_pagina(doc);
        }
        Visor::Plano | Visor::Otro => {}
    }
    if !matches!(visor, Visor::Plano | Visor::Otro) {
        use pixpin_sincro::anotado::{MARCOS, VISTAS};
        let _ = ad::del_mensaje_o_junto(doc, MARCOS, crate::marcos_doc::ruta(doc));
        let _ = ad::del_mensaje_o_junto(doc, VISTAS, crate::marcos_doc::ruta_de_vistas(doc));
    }
}

/// El texto con el nombre del chat.
fn con_chat(textos: &Catalogo, clave: &str, chat: &str) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("chat", chat.to_string());
    textos.t_args(clave, &args)
}

/// **El boton «Guardar»**: el menu de donde (junto al raton) y, si se
/// elige, guardar y decirlo sobre `padre`. `true` si quedo guardado.
pub fn preguntar_y_guardar(
    padre: windows::Win32::Foundation::HWND,
    raiz: &Path,
    original: &Path,
    visor: Visor,
    anotado: &[PathBuf],
    textos: &Catalogo,
) -> bool {
    use pixpin_shell::exportar::{EntradaMenu, informar, menu_emergente};
    let sitios = destinos(&Indice::leer(raiz), CHATS_EN_EL_MENU);
    let mut entradas = Vec::new();
    for (i, d) in sitios.iter().enumerate() {
        if i == 1 {
            entradas.push(EntradaMenu {
                id: 0,
                texto: String::new(),
            });
        }
        entradas.push(EntradaMenu {
            id: i as u32 + 1,
            texto: if i == 0 {
                textos.t("guardar-de-paso-guardados")
            } else {
                con_chat(textos, "guardar-de-paso-en", &d.nombre)
            },
        });
    }
    let raton = pixpin_shell::overlay::raton_en_pantalla();
    let Some(elegido) = menu_emergente(padre, raton.x, raton.y, &entradas)
        .and_then(|id| sitios.get(id as usize - 1))
    else {
        return false;
    };
    match guardar(raiz, elegido.chat.as_deref(), original, visor, anotado) {
        Ok(g) => {
            tracing::debug!(
                chat = %g.chat,
                mensaje = %g.mensaje.id,
                copia = %g.ruta.display(),
                "de paso: guardado desde su visor"
            );
            informar(
                padre,
                &textos.t("guardar-de-paso"),
                &con_chat(textos, "guardar-de-paso-hecho", &elegido.nombre),
            );
            true
        }
        Err(e) => {
            tracing::warn!(?e, ruta = %original.display(), "de paso: no se pudo guardar");
            informar(
                padre,
                &textos.t("guardar-de-paso"),
                &textos.t("guardar-de-paso-fallo"),
            );
            false
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::almacen::Ficha;

    fn raiz(etiqueta: &str) -> PathBuf {
        let r =
            std::env::temp_dir().join(format!("pixpin-de-paso-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    #[test]
    fn lo_que_colgaba_del_nombre_cuelga_del_de_la_copia() {
        assert_eq!(
            al_sitio_nuevo(
                "archivos/Informe.docx.pixpin-anotado/capa.excalidraw",
                "Informe.docx",
                "archivos/Informe (1).docx"
            ),
            "archivos/Informe (1).docx.pixpin-anotado/capa.excalidraw"
        );
        assert_eq!(
            al_sitio_nuevo(
                "archivos/Informe.docx.pixpin-lectura",
                "Informe.docx",
                "archivos/Informe.docx"
            ),
            "archivos/Informe.docx.pixpin-lectura"
        );
        // Lo que ya es del mensaje (anot-<uid>…) va tal cual.
        assert_eq!(
            al_sitio_nuevo(
                "android/pins/draw/anot-abc.vistas.json",
                "Informe.docx",
                "archivos/Informe (1).docx"
            ),
            "android/pins/draw/anot-abc.vistas.json"
        );
        // Caso negativo: otro fichero que empieza igual no es un hermano.
        assert_eq!(
            al_sitio_nuevo(
                "archivos/Informe.docx2.pixpin-lectura",
                "Informe.docx",
                "archivos/x.docx"
            ),
            "archivos/Informe.docx2.pixpin-lectura"
        );
    }

    #[test]
    fn las_hojas_cuentan_desde_uno_en_el_nombre() {
        assert_eq!(hoja_de("hoja-1.excalidraw"), Some(0));
        assert_eq!(hoja_de("hoja-12.excalidraw"), Some(11));
        for n in [
            "hoja-0.excalidraw",
            "capa.excalidraw",
            "hoja-x.excalidraw",
            "hoja-2.hoja",
        ] {
            assert_eq!(hoja_de(n), None, "{n}");
        }
    }

    #[test]
    fn el_menu_empieza_por_guardados_y_sigue_por_el_ultimo_tocado() {
        let mut indice = Indice::default();
        // Sin «Mensajes guardados» todavia: sale igual, para crearlo.
        assert_eq!(
            destinos(&indice, 5),
            vec![Destino {
                chat: None,
                nombre: almacen::NOMBRE_GUARDADOS.to_string(),
            }]
        );
        let mut viejo = Ficha::nueva("Viejo", 1, "PC");
        viejo.tocado = 10;
        let mut nuevo = Ficha::nueva("Nuevo", 1, "PC");
        nuevo.tocado = 20;
        let mut asistente = Ficha::nueva(almacen::NOMBRE_ASISTENTE, 1, "PC");
        almacen::poner_marca_asistente(&mut asistente);
        indice.proyectos = vec![viejo.clone(), asistente, nuevo.clone()];
        let d = destinos(&indice, 5);
        let nombres: Vec<&str> = d.iter().map(|d| d.nombre.as_str()).collect();
        assert_eq!(nombres, [almacen::NOMBRE_GUARDADOS, "Nuevo", "Viejo"]);
        assert_eq!(d[1].chat.as_deref(), Some(nuevo.id.as_str()));
        // Con tope: solo los mas recientes.
        assert_eq!(destinos(&indice, 1).len(), 2);
    }

    /// **Lo anotado de paso pasa al mensaje nuevo**: la tinta de un PDF de
    /// fuera acaba en `anot-<uid>-p<n>` con el codigo del mensaje, y sus
    /// marcos en `anot-<uid>.marcos.json`; el fichero y lo de fuera se quedan.
    #[test]
    fn un_pdf_de_fuera_entra_en_guardados_con_su_tinta_y_sus_marcos() {
        let r = raiz("pdf");
        let datos = r.join("datos");
        std::fs::create_dir_all(&datos).unwrap();
        let fuera = r.join("Descargas");
        std::fs::create_dir_all(&fuera).unwrap();
        let pdf = fuera.join("Plano de obra.pdf");
        std::fs::write(&pdf, b"%PDF-1.4 de prueba").unwrap();
        let tinta = crate::lector_tinta::ruta_de_hoja(&pdf, 1);
        std::fs::create_dir_all(tinta.parent().unwrap()).unwrap();
        let excalidraw = r#"{"type":"excalidraw","version":2,"elements":[]}"#;
        std::fs::write(&tinta, excalidraw).unwrap();
        std::fs::write(crate::marcos_doc::ruta(&pdf), "[[0,0,1,1]]").unwrap();
        assert!(es_de_paso(&datos, &pdf));

        let g = guardar(&datos, None, &pdf, Visor::Pdf, &[]).expect("guardado");
        let indice = Indice::leer(&datos);
        assert!(indice.buscar(&g.chat).is_some_and(|f| f.es_guardados()));
        assert_eq!(std::fs::read(&g.ruta).unwrap(), b"%PDF-1.4 de prueba");
        let uid = g.mensaje.uid.clone().unwrap();
        let base = pixpin_proyecto::anotado::Base {
            raiz: datos.clone(),
            chat: g.chat.clone(),
            base: pixpin_sincro::anotado::de_pagina(&uid, 1),
        };
        assert!(base.tinta().is_file(), "{}", base.tinta().display());
        let marcos = pixpin_proyecto::anotado::Base {
            base: pixpin_sincro::anotado::del_pdf(&uid),
            ..base
        }
        .fichero(pixpin_sincro::anotado::MARCOS);
        assert_eq!(std::fs::read_to_string(marcos).unwrap(), "[[0,0,1,1]]");
        // Lo de fuera no se toca y la copia ya no es de paso.
        assert!(pdf.is_file() && tinta.is_file());
        assert!(!es_de_paso(&datos, &g.ruta));
        // Otra vez el mismo: otro mensaje, con su propio codigo.
        let g2 = guardar(&datos, Some(&g.chat), &pdf, Visor::Pdf, &[]).expect("otra vez");
        assert_ne!(g2.mensaje.uid, g.mensaje.uid);
        assert_ne!(g2.ruta, g.ruta);
        let _ = std::fs::remove_dir_all(&r);
    }
}
