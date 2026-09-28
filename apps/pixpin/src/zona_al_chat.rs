//! **La Zona, al chat del proyecto** (F8; `mandarLaZona` del movil).
//!
//! Con el interruptor «Al chat» encendido, la foto de la zona no se queda
//! como copia en el lienzo: va al chat del proyecto como **sublienzo**. Lo
//! mismo que hace `DrawEditorActivity.mandarLaZona`, paso a paso:
//!
//! 1. el lienzo de origen tiene que ser una hoja del proyecto; si no lo era,
//!    lo pasa a ser;
//! 2. la foto entra en el chat como adjunto (`archivos/…`);
//! 3. el sublienzo es un lienzo propio (`foto-<id del mensaje>`) con la foto
//!    clavada, como una foto del chat (`hojaDeLaFotoDelChat`);
//! 4. la hoja del sublienzo va **detras de su lienzo** (y de los sublienzos
//!    que ya tuviera), con `padre` y `deMensaje`, y se llama «↳ Zona de…»;
//! 5. el mensaje es una IMAGEN con `referencia` al sublienzo, `unido` y
//!    `vieneDe` («PDF «Plano» → página 3» o «Lienzo «Planta»»), que es la
//!    tarjeta que el chat ya sabe ensenar y abrir.
//!
//! La marca con enlace del origen la pone el editor
//! (`pixpin_motor2d::zona::marcar`), en un solo paso de deshacer.
//!
//! # Que lienzo es
//!
//! El editor no sabe de proyectos: lo abre quien sea (el chat, un pin, el
//! lienzo en blanco). Quien lo abre desde el chat dice de que hoja se trata
//! con [`en_hoja`], una guarda de hilo como la de las marcas
//! (`ventana_editor::marcas::junto_a`): el editor corre en el hilo de quien
//! lo llama y no vuelve hasta cerrarse. Sin guarda no hay interruptor.
//!
//! # Diferencia con el movil
//!
//! Un lienzo que no esta en ningun proyecto, en el movil, pregunta a cual
//! mandarla (`ElegirProyectoParaLaZona`). Aqui esos lienzos (el de un pin, el
//! de la bandeja) no ofrecen el interruptor: la zona sale como copia.

use pixpin_proyecto::{almacen, cuaderno, Hoja, Proyecto};
use pixpin_store::Catalogo;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// La hoja que tiene abierta el editor de este hilo, dicha por quien lo
/// abrio desde el chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HojaAbierta {
    pub raiz: PathBuf,
    /// El id del proyecto (su carpeta).
    pub proyecto: String,
    /// El id del dibujo (`lienzos/<dibujo>.excalidraw`).
    pub dibujo: String,
    /// La pagina del PDF del proyecto sobre la que se dibuja, si es una.
    pub pagina: Option<u32>,
    /// Como se llama en el chat (el nombre de su mensaje).
    pub nombre: String,
    /// El codigo unico de su mensaje: si la hoja se crea aqui, lleva este,
    /// y el chat no la ensena dos veces (`almacen::hojas_para_ensenar`).
    pub uid: Option<String>,
}

impl HojaAbierta {
    /// La hoja del mensaje `m` del chat, abierta desde `ruta`
    /// (`lienzos/<dibujo>.excalidraw`): el dibujo sale del nombre del
    /// fichero, que es el que de verdad se abrio (una pagina que aun no tenia
    /// dibujo lo estrena al abrirse y su mensaje no lo dice todavia).
    pub(crate) fn de(raiz: &Path, proyecto: &str, ruta: &Path, m: &cuaderno::Mensaje) -> HojaAbierta {
        let dibujo = ruta
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .or_else(|| m.referencia.clone())
            .unwrap_or_default();
        let nombre = m.nombre.strip_suffix(".excalidraw").unwrap_or(&m.nombre).to_string();
        HojaAbierta {
            raiz: raiz.to_path_buf(),
            proyecto: proyecto.to_string(),
            dibujo,
            pagina: m.pagina,
            nombre,
            uid: m.uid.clone(),
        }
    }
}

thread_local! {
    static ABIERTA: RefCell<Option<HojaAbierta>> = const { RefCell::new(None) };
}

/// Dice que hoja del proyecto tiene el editor mientras el valor vive. Se
/// pone justo antes de abrir el editor y se suelta al volver.
pub(crate) fn en_hoja(h: HojaAbierta) -> Guarda {
    ABIERTA.with(|a| *a.borrow_mut() = Some(h));
    Guarda(())
}

pub(crate) struct Guarda(());

impl Drop for Guarda {
    fn drop(&mut self) {
        ABIERTA.with(|a| *a.borrow_mut() = None);
    }
}

/// La hoja abierta en el editor de este hilo, si la dijo alguien.
pub(crate) fn hoja_abierta() -> Option<HojaAbierta> {
    ABIERTA.with(|a| a.borrow().clone())
}

/// Lo que quedo hecho: a que dibujo lleva la marca y a que chat fue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mandada {
    /// El dibujo del sublienzo: el `enlace` de la marca.
    pub dibujo: String,
    /// El nombre del proyecto, para decirlo.
    pub proyecto: String,
}

/// **Manda la zona al chat del proyecto** (`mandarLaZona`). `png` es la foto
/// de la zona tal cual (sin redondear: el movil solo redondea la copia) y
/// `ancho` x `alto` sus pixeles.
pub(crate) fn mandar(
    h: &HojaAbierta,
    png: &[u8],
    ancho: u32,
    alto: u32,
    ahora: i64,
    aparato: &str,
    textos: &Catalogo,
) -> std::io::Result<Mandada> {
    let raiz = &h.raiz;
    let carpeta = almacen::carpeta(raiz, &h.proyecto);
    let indice = almacen::Indice::leer(raiz);
    let mut p = leer_proyecto(&carpeta);
    if p.id.is_empty() {
        // Uno nacido aqui no tiene `proyecto.json` todavia: se empieza con
        // lo de su ficha, como «Unir» con una foto.
        p.id = h.proyecto.clone();
        p.nombre = indice
            .buscar(&h.proyecto)
            .map(|f| f.nombre.clone())
            .unwrap_or_default();
    }
    let es_pdf = h.pagina.is_some();

    // 1. El lienzo de origen, hoja del proyecto (si no lo era, lo pasa a ser).
    let origen = match p
        .hojas
        .iter()
        .position(|x| x.dibujo.as_deref() == Some(h.dibujo.as_str()) || (es_pdf && x.pagina == h.pagina))
    {
        Some(i) => p.hojas[i].clone(),
        None => {
            let nueva = Hoja {
                id: format!("h-{ahora}"),
                nombre: if h.nombre.trim().is_empty() {
                    textos.t("zona-hoja-lienzo")
                } else {
                    h.nombre.clone()
                },
                dibujo: Some(h.dibujo.clone()),
                pagina: h.pagina,
                uid: h.uid.clone(),
                ..Default::default()
            };
            p.hojas.push(nueva.clone());
            nueva
        }
    };

    // Los textos del movil: de donde viene y como se llama.
    let mut args = fluent_bundle::FluentArgs::new();
    let (de_donde, nombre) = if let Some(pagina) = h.pagina {
        args.set("proyecto", p.nombre.clone());
        args.set("pagina", (pagina + 1) as i64);
        (textos.t_args("zona-viene-pdf", &args), textos.t_args("zona-nombre-pagina", &args))
    } else {
        let de = if origen.nombre.trim().is_empty() { p.nombre.clone() } else { origen.nombre.clone() };
        args.set("nombre", de);
        let viene = textos.t_args("zona-viene-lienzo", &args);
        let mut n = fluent_bundle::FluentArgs::new();
        n.set(
            "nombre",
            if origen.nombre.trim().is_empty() { textos.t("zona-lienzo") } else { origen.nombre.clone() },
        );
        (viene, textos.t_args("zona-nombre-lienzo", &n))
    };
    let fichero = format!("{nombre}.png");

    // 2. La foto, adjunta al chat.
    let numero = match cuaderno::Cuaderno::leer_de(&carpeta) {
        Ok(c) => c.siguiente_numero(Some(&h.proyecto)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 1,
        Err(e) => return Err(e),
    };
    let ruta = almacen::guardar_adjunto(raiz, &h.proyecto, &fichero, png)?;
    let mut m = cuaderno::Mensaje::adjunto(
        cuaderno::Clase::Imagen,
        &fichero,
        &ruta,
        png.len() as i64,
        &cuaderno::Sello {
            cuando: ahora,
            numero,
            aparato: aparato.to_string(),
            proyecto: h.proyecto.clone(),
        },
    );
    let sublienzo = format!("foto-{}", m.id);

    // 3. El sublienzo: la foto clavada en su propio lienzo.
    let foto = format!("zona{ahora}");
    std::fs::create_dir_all(carpeta.join("imagenes"))?;
    std::fs::write(carpeta.join("imagenes").join(&foto), png)?;
    let lienzo = almacen::lienzo(raiz, &h.proyecto, &sublienzo);
    if let Some(dir) = lienzo.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if !lienzo.exists() {
        escribir_atomico(&lienzo, &lienzo_con_foto(&foto, ancho, alto, ahora))?;
    }

    // 4. Su hoja, detras de su lienzo y de los sublienzos que ya tuviera.
    let mut hoja = Hoja {
        id: format!("hoja-{ahora}"),
        nombre: format!("↳ {nombre}"),
        dibujo: Some(sublienzo.clone()),
        uid: Some(m.codigo_unico()),
        ..Default::default()
    };
    hoja.resto.insert("padre".into(), Value::String(origen.id.clone()));
    hoja.resto.insert("deMensaje".into(), Value::String(m.id.clone()));
    let donde = p
        .hojas
        .iter()
        .rposition(|x| x.id == origen.id || x.resto.get("padre").and_then(|v| v.as_str()) == Some(origen.id.as_str()))
        .map_or(p.hojas.len(), |i| i + 1);
    p.hojas.insert(donde.min(p.hojas.len()), hoja);
    p.tocado = ahora;
    escribir_atomico(
        &carpeta.join("proyecto.json"),
        &serde_json::to_string(&p).map_err(std::io::Error::other)?,
    )?;

    // 5. El mensaje, con los campos del movil.
    m.referencia = Some(sublienzo.clone());
    m.resto.insert("unido".into(), Value::Bool(true));
    let mut viene = serde_json::Map::new();
    viene.insert("texto".into(), Value::String(de_donde));
    viene.insert("dibujo".into(), Value::String(h.dibujo.clone()));
    if es_pdf && let Some(pdf) = &p.pdf_origen {
        viene.insert("pdf".into(), Value::String(pdf.clone()));
    }
    if let Some(pagina) = h.pagina {
        viene.insert("pagina".into(), json!(pagina));
    }
    viene.insert("proyecto".into(), Value::String(p.id.clone()));
    m.resto.insert("vieneDe".into(), Value::Object(viene));
    cuaderno::anadir(&carpeta, &m)?;

    // Y el proyecto sube en la lista, como con cualquier adjunto.
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == h.proyecto) {
        f.tocado = ahora;
        f.resumen = fichero.clone();
        if let Err(e) = indice.guardar(raiz) {
            tracing::warn!(?e, "no se pudo subir el proyecto en la lista");
        }
    }
    Ok(Mandada {
        dibujo: sublienzo,
        proyecto: p.nombre,
    })
}

/// **Lo que hace el editor al soltar la zona con «Al chat»**: manda la foto
/// y, si llego, deja la marca con enlace en el origen, en un solo paso de
/// deshacer (`controller.marcarZona`). Devuelve lo que se dice, bien o mal
/// (los dos `Toast` del movil). Si no se pudo mandar, el lienzo no cambia.
pub(crate) fn mandar_y_marcar(
    escena: &mut pixpin_motor2d::Escena,
    foto: &pixpin_codec::ImagenRgba,
    caja: (f32, f32, f32, f32),
    h: &HojaAbierta,
    textos: &Catalogo,
) -> Result<String, String> {
    let hecho = (|| -> std::io::Result<Mandada> {
        let png = pixpin_codec::codificar_png(foto).map_err(std::io::Error::other)?;
        let aparato = pixpin_proyecto::identidad::Identidad::leer_o_crear(&h.raiz, "PC")
            .map(|i| i.yo.codigo())
            .unwrap_or_default();
        mandar(
            h,
            &png,
            foto.ancho,
            foto.alto,
            pixpin_shell::entorno::ahora_utc_ms(),
            &aparato,
            textos,
        )
    })();
    match hecho {
        Ok(m) => {
            pixpin_motor2d::zona::marcar(escena, caja, &m.dibujo);
            // El chat de detras tiene que ensenar el mensaje nuevo en cuanto
            // se vuelva a el.
            crate::ventana_chat::refrescar();
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("proyecto", m.proyecto);
            Ok(textos.t_args("zona-mandada", &args))
        }
        Err(e) => {
            tracing::warn!(?e, "no se pudo mandar la zona al chat");
            Err(textos.t("zona-no-mandada"))
        }
    }
}

/// El mensaje del chat cuya hoja es `dibujo`, leido del cuaderno. Lo usa el
/// salto por enlace: una zona recien mandada no esta aun en la lista que el
/// chat tenia al abrir el lienzo.
pub(crate) fn mensaje_con_dibujo(raiz: &Path, proyecto: &str, dibujo: &str) -> Option<cuaderno::Mensaje> {
    cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, proyecto))
        .ok()?
        .mensajes
        .into_iter()
        .rev()
        .find(|m| m.referencia.as_deref() == Some(dibujo))
}

fn leer_proyecto(carpeta: &Path) -> Proyecto {
    std::fs::read_to_string(carpeta.join("proyecto.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Al lado y luego de un tiron: un corte a mitad no deja el fichero roto.
fn escribir_atomico(ruta: &Path, texto: &str) -> std::io::Result<()> {
    let mut temporal = ruta.as_os_str().to_owned();
    temporal.push(".zona.tmp");
    let temporal = PathBuf::from(temporal);
    std::fs::write(&temporal, texto)?;
    std::fs::rename(&temporal, ruta)
}

/// El `.excalidraw` del sublienzo: la foto a su tamano en pixeles, en el
/// origen y bloqueada, como `hojaDeLaFotoDelChat` del movil. La foto va por
/// ruta (`imagenes/<id>`), no en base64.
fn lienzo_con_foto(foto: &str, ancho: u32, alto: u32, ahora: i64) -> String {
    json!({
        "type": "excalidraw",
        "version": 2,
        "source": "pixpin-max",
        "elements": [{
            "id": format!("foto-{ahora}-0"),
            "type": "image",
            "x": 0, "y": 0,
            "width": ancho.max(1) as f64,
            "height": alto.max(1) as f64,
            "angle": 0,
            "strokeColor": "transparent",
            "backgroundColor": "transparent",
            "fillStyle": "solid",
            "strokeWidth": 1,
            "strokeStyle": "solid",
            "roughness": 0,
            "opacity": 100,
            "groupIds": [],
            "seed": 1,
            "version": 1,
            "versionNonce": 1,
            "isDeleted": false,
            "boundElements": null,
            "locked": true,
            "fileId": foto,
            "status": "saved",
            "scale": [1, 1]
        }],
        "appState": {"viewBackgroundColor": "#ffffff"},
        "files": {
            foto: {"id": foto, "mimeType": "image/png", "path": format!("imagenes/{foto}"), "created": ahora}
        }
    })
    .to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_store::Idioma;

    fn raiz(etiqueta: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("pixpin-zona-chat-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    fn hoja(raiz: &Path, pagina: Option<u32>) -> HojaAbierta {
        HojaAbierta {
            raiz: raiz.to_path_buf(),
            proyecto: "p1".into(),
            dibujo: "dib-1".into(),
            pagina,
            nombre: "Planta".into(),
            uid: Some("u-planta".into()),
        }
    }

    fn proyecto_con(raiz: &Path, p: serde_json::Value) {
        let c = almacen::carpeta(raiz, "p1");
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(c.join("proyecto.json"), p.to_string()).unwrap();
    }

    fn textos() -> Catalogo {
        Catalogo::nuevo(Idioma::Espanol)
    }

    #[test]
    fn la_zona_llega_al_chat_como_imagen_con_viene_de_y_su_sublienzo() {
        let r = raiz("lienzo");
        proyecto_con(
            &r,
            json!({"id": "p1", "nombre": "Casa", "hojas": [{"id": "h-planta", "nombre": "Planta", "dibujo": "dib-1"}]}),
        );
        let hecho = mandar(&hoja(&r, None), b"PNG", 30, 20, 1_000, "K7Q2", &textos()).unwrap();
        assert_eq!(hecho.proyecto, "Casa");
        let carpeta = almacen::carpeta(&r, "p1");
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        assert_eq!(c.mensajes.len(), 1);
        let m = &c.mensajes[0];
        assert_eq!(m.clase, Some(cuaderno::Clase::Imagen));
        assert_eq!(m.nombre, "Zona de Planta.png");
        assert_eq!(m.referencia.as_deref(), Some(hecho.dibujo.as_str()));
        assert_eq!(hecho.dibujo, format!("foto-{}", m.id), "el sublienzo se llama como en el movil");
        assert_eq!(m.aparato.as_deref(), Some("K7Q2"));
        assert_eq!(m.resto.get("unido"), Some(&Value::Bool(true)));
        let v = m.resto.get("vieneDe").unwrap();
        assert_eq!(v["texto"], "Lienzo «Planta»");
        assert_eq!(v["dibujo"], "dib-1");
        assert_eq!(v["proyecto"], "p1");
        assert!(v.get("pagina").is_none() && v.get("pdf").is_none(), "un lienzo no es una pagina");
        // La foto del mensaje esta donde dice.
        assert_eq!(std::fs::read(carpeta.join(m.ruta.as_ref().unwrap())).unwrap(), b"PNG");
        // El sublienzo abre con la foto clavada.
        let texto = std::fs::read_to_string(almacen::lienzo(&r, "p1", &hecho.dibujo)).unwrap();
        let l: Value = serde_json::from_str(&texto).unwrap();
        assert_eq!(l["elements"][0]["type"], "image");
        assert_eq!(l["elements"][0]["locked"], true);
        assert_eq!(l["elements"][0]["width"], 30.0);
        let ruta_foto = l["files"][l["elements"][0]["fileId"].as_str().unwrap()]["path"].as_str().unwrap();
        assert_eq!(std::fs::read(carpeta.join(ruta_foto)).unwrap(), b"PNG");
        // Y el salto por enlace lo encuentra leyendo el cuaderno.
        assert_eq!(mensaje_con_dibujo(&r, "p1", &hecho.dibujo).map(|x| x.id), Some(m.id.clone()));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_hoja_del_sublienzo_cuelga_de_su_lienzo_y_va_detras_de_las_suyas() {
        let r = raiz("orden");
        proyecto_con(
            &r,
            json!({"id": "p1", "nombre": "Casa", "hojas": [
                {"id": "h-planta", "nombre": "Planta", "dibujo": "dib-1"},
                {"id": "h-alzado", "nombre": "Alzado", "dibujo": "dib-2"}
            ]}),
        );
        let a = mandar(&hoja(&r, None), b"A", 10, 10, 1_000, "K7Q2", &textos()).unwrap();
        let b = mandar(&hoja(&r, None), b"B", 10, 10, 2_000, "K7Q2", &textos()).unwrap();
        let p = leer_proyecto(&almacen::carpeta(&r, "p1"));
        let ids: Vec<_> = p.hojas.iter().map(|h| h.dibujo.clone().unwrap_or_default()).collect();
        // Detras de su lienzo, en el orden en que se mandaron, y antes del
        // lienzo siguiente: no al final del proyecto.
        assert_eq!(ids, ["dib-1", a.dibujo.as_str(), b.dibujo.as_str(), "dib-2"]);
        let sub = &p.hojas[1];
        assert_eq!(sub.nombre, "↳ Zona de Planta");
        assert_eq!(sub.resto["padre"], "h-planta");
        assert!(sub.resto["deMensaje"].as_str().is_some_and(|s| !s.is_empty()));
        // Lleva el codigo de su mensaje: el chat no la ensena dos veces.
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&r, "p1")).unwrap();
        assert_eq!(sub.uid, Some(c.mensajes[0].codigo_unico()));
        assert_eq!(c.mensajes[1].numero, c.mensajes[0].numero + 1, "numeros seguidos");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn un_lienzo_que_no_era_hoja_pasa_a_serlo_una_sola_vez() {
        let r = raiz("sin-hoja");
        // Un proyecto nacido aqui, sin `proyecto.json`.
        std::fs::create_dir_all(almacen::carpeta(&r, "p1")).unwrap();
        mandar(&hoja(&r, None), b"A", 10, 10, 1_000, "K7Q2", &textos()).unwrap();
        mandar(&hoja(&r, None), b"B", 10, 10, 2_000, "K7Q2", &textos()).unwrap();
        let p = leer_proyecto(&almacen::carpeta(&r, "p1"));
        assert_eq!(p.id, "p1");
        let origenes: Vec<_> = p.hojas.iter().filter(|h| h.dibujo.as_deref() == Some("dib-1")).collect();
        assert_eq!(origenes.len(), 1, "caso negativo: la segunda zona no la duplica");
        assert_eq!(origenes[0].nombre, "Planta");
        assert_eq!(origenes[0].uid.as_deref(), Some("u-planta"));
        assert_eq!(p.hojas.len(), 3);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn en_una_pagina_del_pdf_dice_de_que_pagina_viene() {
        let r = raiz("pdf");
        proyecto_con(
            &r,
            json!({"id": "p1", "nombre": "Plano", "pdfOrigen": "archivos/doc-1.pdf",
                   "hojas": [{"id": "h-2", "nombre": "", "pagina": 2}]}),
        );
        let h = HojaAbierta { dibujo: "dib-pag".into(), ..hoja(&r, Some(2)) };
        mandar(&h, b"A", 10, 10, 1_000, "K7Q2", &textos()).unwrap();
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&r, "p1")).unwrap();
        let v = &c.mensajes[0].resto["vieneDe"];
        assert_eq!(v["texto"], "PDF «Plano» → página 3");
        assert_eq!(v["pagina"], 2);
        assert_eq!(v["pdf"], "archivos/doc-1.pdf");
        assert_eq!(c.mensajes[0].nombre, "Zona de la página 3.png");
        // La hoja de la pagina se reconoce por su pagina: no se crea otra.
        let p = leer_proyecto(&almacen::carpeta(&r, "p1"));
        assert_eq!(p.hojas.len(), 2);
        assert_eq!(p.hojas[1].resto["padre"], "h-2");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn mandar_y_marcar_deja_la_marca_con_enlace_de_un_solo_deshacer() {
        let r = raiz("marcar");
        proyecto_con(&r, json!({"id": "p1", "nombre": "Casa", "hojas": []}));
        let mut escena = pixpin_motor2d::Escena::nueva();
        let foto = pixpin_codec::ImagenRgba { ancho: 2, alto: 1, pixeles: vec![255; 8] };
        let dicho = mandar_y_marcar(&mut escena, &foto, (0.0, 0.0, 40.0, 30.0), &hoja(&r, None), &textos()).unwrap();
        assert_eq!(dicho, "Zona mandada al chat de «Casa»");
        let marca = escena.elementos.iter().find(|e| e.enlace.is_some()).expect("la marca");
        let m = mensaje_con_dibujo(&r, "p1", marca.enlace.as_deref().unwrap()).expect("lleva a su mensaje");
        assert_eq!(m.clase, Some(cuaderno::Clase::Imagen));
        assert!(escena.deshacer());
        assert!(escena.elementos.iter().all(|e| e.borrado || e.enlace.is_none()));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn si_no_se_puede_mandar_el_lienzo_no_cambia() {
        let r = raiz("roto");
        // Caso negativo: donde iria el proyecto hay un fichero, no una carpeta.
        std::fs::create_dir_all(r.join("proyectos")).unwrap();
        std::fs::write(almacen::carpeta(&r, "p1"), b"no soy una carpeta").unwrap();
        let mut escena = pixpin_motor2d::Escena::nueva();
        let foto = pixpin_codec::ImagenRgba { ancho: 1, alto: 1, pixeles: vec![255; 4] };
        let dicho = mandar_y_marcar(&mut escena, &foto, (0.0, 0.0, 40.0, 30.0), &hoja(&r, None), &textos());
        assert_eq!(dicho, Err("No se pudo mandar la zona".to_string()));
        assert!(escena.elementos.is_empty());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_hoja_abierta_sale_del_fichero_que_se_abrio_y_de_su_mensaje() {
        let m = cuaderno::Mensaje {
            nombre: "Planta.excalidraw".into(),
            referencia: Some("dib-viejo".into()),
            pagina: Some(4),
            uid: Some("u1".into()),
            ..Default::default()
        };
        let r = Path::new("C:/raiz");
        let h = HojaAbierta::de(r, "p1", &almacen::lienzo(r, "p1", "dib-9"), &m);
        assert_eq!(h.dibujo, "dib-9", "manda el fichero abierto");
        assert_eq!(h.nombre, "Planta");
        assert_eq!((h.pagina, h.uid.as_deref(), h.proyecto.as_str()), (Some(4), Some("u1"), "p1"));
        // Caso negativo: un nombre sin extension de lienzo se queda entero.
        let otro = cuaderno::Mensaje { nombre: "Alzado".into(), ..m };
        assert_eq!(HojaAbierta::de(r, "p1", Path::new("x.excalidraw"), &otro).nombre, "Alzado");
    }

    #[test]
    fn sin_guarda_no_hay_hoja_y_al_soltarla_se_olvida() {
        let r = raiz("guarda");
        assert_eq!(hoja_abierta(), None, "caso negativo: nadie la dijo");
        {
            let _g = en_hoja(hoja(&r, None));
            assert_eq!(hoja_abierta().map(|h| h.dibujo), Some("dib-1".into()));
        }
        assert_eq!(hoja_abierta(), None);
        // Y un enlace que no es de ninguna hoja no se inventa.
        assert!(mensaje_con_dibujo(&r, "p1", "foto-nada").is_none());
        let _ = std::fs::remove_dir_all(&r);
    }
}
