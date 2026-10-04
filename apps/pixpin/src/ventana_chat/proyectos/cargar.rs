//! Lo que se lee del disco para la pantalla de Proyectos, **en su hilo**.
//!
//! Leer las hojas de un proyecto es abrir su `proyecto.json`, su cuaderno y
//! cada lienzo para su miniatura: en un proyecto de treinta lienzos del movil
//! son cientos de milisegundos, y en el hilo de la ventana eso es un tiron al
//! pasar de proyecto. Aqui se hace aparte, **solo del que se ve y del que
//! asoma** (como `beyondViewportPageCount = 1` del movil), y la ventana se
//! despierta cuando esta. Sin nada pedido el hilo duerme: la pantalla quieta
//! no cuesta nada.
//!
//! Las paginas de un PDF sin dibujo **no se leen aqui**: su miniatura es la
//! pagina pintada por `pdf_en_chat`, y pedirlas todas de golpe es pintar las
//! doscientas de un plano para ensenar ocho. Se piden desde la ventana solo
//! las que se ven (`VistaProyectos::latido`), que es lo que hace
//! `PreparadorDeMiniaturas` en el movil.

use std::collections::{HashMap, VecDeque};
use std::sync::{Condvar, Mutex, OnceLock};

use pixpin_proyecto::almacen::Ficha;
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_store::Ubicacion;

use super::super::{Ojeada, leer_proyecto_json, leer_vista};

/// Una hoja del proyecto, lista para pintarse.
pub(crate) struct Hoja {
    /// El id de la hoja en `proyecto.json` (el de su padre, en `padre`).
    pub id: String,
    pub padre: Option<String>,
    /// Su nombre en el proyecto (vacio: el del mensaje).
    pub nombre: String,
    /// El mensaje que la representa: por el se abre, igual que en el chat.
    pub mensaje: Mensaje,
    pub vista: Option<Ojeada>,
    /// Una pagina del documento sin dibujo: su vista se pide al verla.
    pub por_pedir: bool,
    /// Lleva algo dibujado encima (solo cuenta en las paginas de un PDF).
    pub anotada: bool,
}

/// Las hojas de un proyecto, en el orden de su `proyecto.json` (el de
/// `HojasDelProyecto.paginas` del movil).
pub(crate) struct Hojas {
    pub id: String,
    /// El `tocado` de la ficha cuando se leyo: si cambia, se vuelve a leer.
    pub tocado: i64,
    pub hojas: Vec<Hoja>,
    pub total: usize,
    pub anotadas: usize,
    pub con_pdf: bool,
    pub archivado: bool,
}

/// Lo poco que hace falta de cada proyecto para ordenar la lista y decir
/// cuantas hojas tiene, sin leer ningun lienzo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Resumen {
    pub archivado: bool,
    pub hojas: usize,
    pub con_pdf: bool,
}

/// La marca de archivado de un proyecto nacido aqui, que no tiene
/// `proyecto.json` donde apuntarla: va en su ficha del indice.
pub(crate) const MARCA_ARCHIVADO: &str = "archivado";

fn archivado_en_ficha(f: &Ficha) -> bool {
    f.resto.get(MARCA_ARCHIVADO).and_then(|v| v.as_bool()) == Some(true)
}

/// El resumen de un proyecto: su `proyecto.json`, si lo tiene.
pub(crate) fn resumen_de(ubicacion: &Ubicacion, f: &Ficha) -> Resumen {
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &f.id);
    let p = leer_proyecto_json(&carpeta);
    Resumen {
        archivado: p.archivado || archivado_en_ficha(f),
        hojas: p.hojas.len(),
        con_pdf: p.pdf_origen.is_some() || carpeta.join("documento.pdf").is_file(),
    }
}

/// Si la hoja es una pagina del documento sin dibujo propio: su vista es la
/// pagina pintada y nada mas, y se pide solo cuando se ve.
fn pagina_sola(h: &pixpin_proyecto::Hoja, m: &Mensaje) -> bool {
    h.dibujo.is_none()
        && m.clase == Some(Clase::Pagina)
        && m.referencia.as_deref().is_none_or(str::is_empty)
}

/// Si una vista lleva algo dibujado de verdad (`CapaDeAnotacion.conContenido`
/// del movil): una pagina abierta y cerrada sin trazar nada no esta anotada.
pub(crate) fn con_trazos(v: Option<&Ojeada>) -> bool {
    match v {
        Some(Ojeada::Lienzo(l)) => !l.ordenes.is_empty() || !l.grafitos.is_empty(),
        _ => false,
    }
}

/// Lee las hojas de un proyecto. No escribe nada: ni completa el cuaderno ni
/// estrena dibujos, eso lo hace el chat al abrirlo.
pub(crate) fn hojas_de(ubicacion: &Ubicacion, f: &Ficha) -> Hojas {
    let raiz = ubicacion.raiz();
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &f.id);
    let p = leer_proyecto_json(&carpeta);
    let mut mensajes = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta)
        .map(|c| c.mensajes)
        .unwrap_or_default();
    mensajes.extend(pixpin_proyecto::almacen::hojas_para_ensenar(
        raiz,
        &f.id,
        f.aparato.as_deref().unwrap_or_default(),
    ));
    // Por su codigo unico, que es el que la hoja guarda (`hoja_de_mensaje`
    // se lo pone al unir; las del movil lo traen). Lo del buzon no cuenta:
    // aun no es de nadie.
    let mut por_codigo: HashMap<String, Mensaje> = HashMap::new();
    for m in mensajes.into_iter().filter(|m| !m.en_buzon) {
        por_codigo.entry(m.codigo_unico()).or_insert(m);
    }
    let mut hojas = Vec::new();
    for h in &p.hojas {
        let Some(m) = h.uid.as_ref().and_then(|u| por_codigo.remove(u)) else {
            continue;
        };
        let por_pedir = pagina_sola(h, &m);
        let vista = if por_pedir {
            None
        } else {
            leer_vista(ubicacion, &f.id, &m)
        };
        let anotada = m.pagina.is_some() && con_trazos(vista.as_ref());
        hojas.push(Hoja {
            id: h.id.clone(),
            nombre: h.nombre.clone(),
            padre: h
                .resto
                .get("padre")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            mensaje: m,
            vista,
            por_pedir,
            anotada,
        });
    }
    Hojas {
        id: f.id.clone(),
        tocado: f.tocado,
        anotadas: hojas.iter().filter(|h| h.anotada).count(),
        total: p.hojas.len(),
        con_pdf: p.pdf_origen.is_some() || carpeta.join("documento.pdf").is_file(),
        archivado: p.archivado || archivado_en_ficha(f),
        hojas,
    }
}

// --- El hilo ---------------------------------------------------------------

enum Trabajo {
    Hojas(Ubicacion, Box<Ficha>),
    Resumenes(Ubicacion, Vec<Ficha>),
}

pub(crate) enum Hecho {
    Hojas(Hojas),
    Resumenes(HashMap<String, Resumen>),
}

#[derive(Default)]
struct Cola {
    pedidos: VecDeque<Trabajo>,
    hechos: Vec<Hecho>,
    arrancado: bool,
}

fn cola() -> &'static (Mutex<Cola>, Condvar) {
    static COLA: OnceLock<(Mutex<Cola>, Condvar)> = OnceLock::new();
    COLA.get_or_init(|| (Mutex::new(Cola::default()), Condvar::new()))
}

fn arrancar(c: &mut Cola) {
    if c.arrancado {
        return;
    }
    let lanzado = std::thread::Builder::new()
        .name("proyectos-cargar".into())
        .spawn(hilo);
    match lanzado {
        Ok(_) => c.arrancado = true,
        Err(e) => tracing::warn!(?e, "sin hilo para leer los proyectos"),
    }
}

/// Pide las hojas de un proyecto. `primero` lo pone delante de lo que haya:
/// el que se esta mirando pasa por delante del que asoma.
pub(crate) fn pedir_hojas(ubicacion: &Ubicacion, f: &Ficha, primero: bool) {
    let (m, cv) = cola();
    let Ok(mut c) = m.lock() else { return };
    // Uno que ya espera no se pide dos veces: si cambio, lo que se lea saldra
    // con lo de ahora igual.
    let ya = c
        .pedidos
        .iter()
        .any(|t| matches!(t, Trabajo::Hojas(_, g) if g.id == f.id));
    if ya {
        return;
    }
    let t = Trabajo::Hojas(ubicacion.clone(), Box::new(f.clone()));
    if primero {
        c.pedidos.push_front(t);
    } else {
        c.pedidos.push_back(t);
    }
    arrancar(&mut c);
    cv.notify_one();
}

/// Pide el resumen de todos los proyectos (para ordenarlos como el movil).
pub(crate) fn pedir_resumenes(ubicacion: &Ubicacion, fichas: &[Ficha]) {
    let (m, cv) = cola();
    let Ok(mut c) = m.lock() else { return };
    c.pedidos.retain(|t| !matches!(t, Trabajo::Resumenes(..)));
    c.pedidos
        .push_back(Trabajo::Resumenes(ubicacion.clone(), fichas.to_vec()));
    arrancar(&mut c);
    cv.notify_one();
}

/// Lo que ya esta leido desde la ultima vez que se pregunto.
pub(crate) fn recoger() -> Vec<Hecho> {
    let (m, _) = cola();
    m.lock()
        .map(|mut c| std::mem::take(&mut c.hechos))
        .unwrap_or_default()
}

fn hilo() {
    loop {
        let (m, cv) = cola();
        let trabajo = {
            let Ok(mut c) = m.lock() else { return };
            loop {
                if let Some(t) = c.pedidos.pop_front() {
                    break t;
                }
                c = match cv.wait(c) {
                    Ok(c) => c,
                    Err(_) => return,
                };
            }
        };
        let hecho = match trabajo {
            Trabajo::Hojas(u, f) => {
                let desde = std::time::Instant::now();
                let h = hojas_de(&u, &f);
                tracing::debug!(
                    proyecto = %f.id,
                    hojas = h.hojas.len(),
                    ms = desde.elapsed().as_millis() as u64,
                    "hojas del proyecto leidas"
                );
                Hecho::Hojas(h)
            }
            Trabajo::Resumenes(u, fichas) => Hecho::Resumenes(
                fichas
                    .iter()
                    .map(|f| (f.id.clone(), resumen_de(&u, f)))
                    .collect(),
            ),
        };
        if let Ok(mut c) = m.lock() {
            c.hechos.push(hecho);
        }
        let h = super::super::ABIERTA.load(std::sync::atomic::Ordering::SeqCst);
        if h != 0 {
            pixpin_shell::overlay::despertar(h);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::cuaderno::{self, Sello};

    fn almacen(nombre: &str) -> (Ubicacion, std::path::PathBuf) {
        let raiz = std::env::temp_dir().join(format!(
            "pixpin-proyectos-cargar-{nombre}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        (Ubicacion::Portable { raiz: raiz.clone() }, raiz)
    }

    fn ficha(id: &str) -> Ficha {
        Ficha {
            id: id.into(),
            nombre: "Casa".into(),
            tocado: 5,
            ..Default::default()
        }
    }

    fn sello(n: i64) -> Sello {
        Sello {
            cuando: 1000 + n,
            numero: n,
            aparato: "PC".into(),
            proyecto: "p1".into(),
        }
    }

    #[test]
    fn las_hojas_salen_en_el_orden_del_proyecto_y_no_en_el_del_chat() {
        let (u, raiz) = almacen("orden");
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, "p1");
        let a = cuaderno::Mensaje::nota("primera en el chat", &sello(1));
        let b = cuaderno::Mensaje::nota("segunda en el chat", &sello(2));
        let suelta = cuaderno::Mensaje::nota("no es una hoja", &sello(3));
        for m in [&a, &b, &suelta] {
            cuaderno::anadir(&carpeta, m).unwrap();
        }
        let hoja = |m: &Mensaje, id: &str| pixpin_proyecto::Hoja {
            id: id.into(),
            nota: Some(m.texto.clone()),
            uid: Some(m.codigo_unico()),
            ..Default::default()
        };
        let p = pixpin_proyecto::Proyecto {
            id: "p1".into(),
            hojas: vec![hoja(&b, "hb"), hoja(&a, "ha")],
            ..Default::default()
        };
        std::fs::write(
            carpeta.join("proyecto.json"),
            serde_json::to_string(&p).unwrap(),
        )
        .unwrap();
        let h = hojas_de(&u, &ficha("p1"));
        let ids: Vec<&str> = h.hojas.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["hb", "ha"]);
        assert_eq!(h.total, 2);
        assert!(!h.con_pdf);
        assert!(!h.archivado);
    }

    #[test]
    fn un_proyecto_nacido_aqui_sin_proyecto_json_no_tiene_hojas_ni_falla() {
        let (u, _raiz) = almacen("vacio");
        let h = hojas_de(&u, &ficha("nuevo"));
        assert!(h.hojas.is_empty());
        assert_eq!(h.total, 0);
        assert_eq!(resumen_de(&u, &ficha("nuevo")), Resumen::default());
    }

    #[test]
    fn una_hoja_cuyo_mensaje_no_esta_no_sale_pero_cuenta_en_el_total() {
        let (u, raiz) = almacen("huerfana");
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, "p1");
        std::fs::create_dir_all(&carpeta).unwrap();
        let p = pixpin_proyecto::Proyecto {
            id: "p1".into(),
            hojas: vec![pixpin_proyecto::Hoja {
                id: "h".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        std::fs::write(
            carpeta.join("proyecto.json"),
            serde_json::to_string(&p).unwrap(),
        )
        .unwrap();
        let h = hojas_de(&u, &ficha("p1"));
        assert!(h.hojas.is_empty());
        assert_eq!(h.total, 1);
    }

    #[test]
    fn archivado_se_lee_del_proyecto_o_de_la_marca_de_la_ficha() {
        let (u, raiz) = almacen("archivado");
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, "p1");
        std::fs::create_dir_all(&carpeta).unwrap();
        let p = pixpin_proyecto::Proyecto {
            id: "p1".into(),
            archivado: true,
            ..Default::default()
        };
        std::fs::write(
            carpeta.join("proyecto.json"),
            serde_json::to_string(&p).unwrap(),
        )
        .unwrap();
        assert!(resumen_de(&u, &ficha("p1")).archivado);
        let mut aqui = ficha("p2");
        aqui.resto
            .insert(MARCA_ARCHIVADO.into(), serde_json::Value::Bool(true));
        assert!(resumen_de(&u, &aqui).archivado);
        assert!(!resumen_de(&u, &ficha("p3")).archivado);
    }

    #[test]
    fn una_pagina_del_documento_sin_dibujo_se_deja_para_cuando_se_vea() {
        let (u, raiz) = almacen("pagina");
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, "p1");
        std::fs::create_dir_all(&carpeta).unwrap();
        let p = pixpin_proyecto::Proyecto {
            id: "p1".into(),
            hojas: vec![pixpin_proyecto::Hoja {
                id: "pag3".into(),
                nombre: "Pag. 3".into(),
                pagina: Some(2),
                uid: Some("u-pag3".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        std::fs::write(
            carpeta.join("proyecto.json"),
            serde_json::to_string(&p).unwrap(),
        )
        .unwrap();
        let h = hojas_de(&u, &ficha("p1"));
        assert_eq!(
            h.hojas.len(),
            1,
            "la pagina sale aunque el chat no la tenga"
        );
        assert!(h.hojas[0].por_pedir);
        assert!(h.hojas[0].vista.is_none());
        assert!(!h.hojas[0].anotada, "sin dibujo no esta anotada");
    }

    #[test]
    fn las_paginas_de_un_pdf_metido_en_el_chat_salen_todas_en_la_tarjeta() {
        // El chat solo lleva el mensaje del PDF; las paginas viven aqui.
        let (u, raiz) = almacen("pdf-en-tarjeta");
        let f = ficha("p1");
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, "p1");
        std::fs::create_dir_all(&carpeta).unwrap();
        let imgs: Vec<_> = (0..4)
            .map(|_| pixpin_codec::imagen::ImagenRgba {
                ancho: 20,
                alto: 30,
                pixeles: [200, 90, 40, 255].repeat(600),
            })
            .collect();
        let doc = raiz.join("plano.pdf");
        std::fs::write(&doc, pixpin_pdf::union::de_imagenes(&imgs).unwrap()).unwrap();
        let mut m = cuaderno::Mensaje::adjunto(
            cuaderno::Clase::Archivo,
            "plano.pdf",
            "archivos/plano.pdf",
            1,
            &sello(1),
        );
        m.id = "m-pdf".into();
        cuaderno::anadir(&carpeta, &m).unwrap();
        crate::pdf_en_chat::unir(&raiz, &f, &doc, "m-pdf", "plano", 1000, &|_, _| {}).unwrap();

        let h = hojas_de(&u, &f);
        assert_eq!(h.total, 4);
        assert_eq!(h.hojas.len(), 4, "las cuatro paginas en la tira");
        assert!(
            h.hojas.iter().all(|h| h.por_pedir),
            "pintadas desde el PDF, sin leer nada"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
