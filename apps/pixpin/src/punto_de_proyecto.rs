//! **El punto verde o rojo de la burbuja**: si lo que hay en el chat vive
//! tambien en los proyectos (`PuntoDeProyecto` y
//! `Proyectos.estaEnLosProyectos` del movil).
//!
//! Verde: esta en los dos sitios, el chat y las hojas de algun proyecto.
//! Rojo: solo en el chat. Antes aqui el punto salia verde en cualquier chat
//! de proyecto y rojo en «Mensajes guardados», que no contestaba la pregunta
//! que el usuario hace con el: dos `.md` mandados al chat de un proyecto
//! nuevo salian verdes sin haberse unido a nada (queja del 1-oct-2026).
//!
//! Se mira por lo que identifica a cada cosa, nunca por el nombre del
//! fichero: comparar nombres daria verdes falsos en cuanto dos se llamaran
//! igual. Abrir o editar un adjunto no lo une: solo cuenta que una hoja de un
//! proyecto apunte a el.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_proyecto::Proyecto;

/// Si el mensaje lleva punto. Solo lo que es un archivo: una nota escrita o
/// una nota de voz no estan «en los proyectos» ni dejan de estarlo, y un
/// punto ahi seria un adorno que hay que interpretar (igual que el movil).
pub fn lleva_punto(m: &Mensaje) -> bool {
    match m.clase.as_ref() {
        Some(Clase::Imagen | Clase::Archivo | Clase::Pagina | Clase::Dibujo | Clase::Proyecto) => {
            true
        }
        Some(Clase::Otra(c)) => c == "TABLA" || c == "CROQUIS",
        _ => false,
    }
}

/// Lo que dice de cada proyecto que algo es suyo, junto para preguntar sin
/// volver a leer el disco por cada burbuja.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Senas {
    /// Ids de mensaje a los que apunta una hoja (`deMensaje`).
    de_mensaje: HashSet<String>,
    /// Codigos unicos de las hojas: aqui una hoja que el cuaderno no tiene se
    /// ensena como mensaje con el codigo de su hoja.
    uids: HashSet<String>,
    dibujos: HashSet<String>,
    tablas: HashSet<String>,
    croquis: HashSet<String>,
    /// El PDF del que nace un proyecto: no es una hoja, es el documento.
    archivos: HashSet<String>,
    /// Los proyectos que existen, por id de carpeta y por el de dentro.
    proyectos: HashSet<String>,
}

impl Senas {
    /// Junta las senas de unos proyectos. `ids_de_carpeta` son los ids de la
    /// lista (las carpetas), que no siempre coinciden con el `id` de dentro
    /// de un proyecto que vino del movil.
    pub fn de(proyectos: &[Proyecto], ids_de_carpeta: &[String]) -> Senas {
        let mut s = Senas::default();
        s.proyectos.extend(ids_de_carpeta.iter().cloned());
        for p in proyectos {
            if !p.id.is_empty() {
                s.proyectos.insert(p.id.clone());
            }
            s.archivos.extend(p.pdf_origen.iter().cloned());
            s.archivos.extend(p.pdf_limpio.iter().cloned());
            s.croquis.extend(p.croquis.iter().cloned());
            for h in &p.hojas {
                if let Some(d) = h.resto.get("deMensaje").and_then(|v| v.as_str()) {
                    s.de_mensaje.insert(d.to_string());
                }
                s.uids.extend(h.uid.iter().cloned());
                s.dibujos.extend(h.dibujo.iter().cloned());
                s.croquis.extend(h.croquis.iter().cloned());
                if let Some(t) = h.resto.get("tabla").and_then(|v| v.as_str()) {
                    s.tablas.insert(t.to_string());
                }
            }
        }
        s
    }

    /// `estaEnLosProyectos` del movil, con las mismas preguntas en el mismo
    /// orden, mas la del codigo de la hoja (ver [`Senas::uids`]).
    pub fn esta(&self, m: &Mensaje) -> bool {
        let clase = m.clase.as_ref();
        let es = |c: &str| matches!(clase, Some(Clase::Otra(x)) if x == c);
        if clase == Some(&Clase::Archivo) && m.ruta.as_ref().is_some_and(|r| self.archivos.contains(r)) {
            return true;
        }
        if clase == Some(&Clase::Proyecto) {
            // Un mensaje que ES un proyecto esta mientras ese proyecto exista.
            return m.referencia.as_ref().is_some_and(|r| self.proyectos.contains(r));
        }
        if self.de_mensaje.contains(&m.id) {
            return true;
        }
        if m.uid.as_ref().is_some_and(|u| self.uids.contains(u)) {
            return true;
        }
        if es("TABLA") {
            return m.referencia.as_ref().is_some_and(|r| self.tablas.contains(r));
        }
        if es("CROQUIS") {
            return m.referencia.as_ref().is_some_and(|r| self.croquis.contains(r));
        }
        // El rastro viejo: una foto o un dibujo unidos antes de que existiera
        // el vinculo comparten id de dibujo con su hoja (`dibujoDeLaFoto`).
        let dibujo = m
            .referencia
            .clone()
            .unwrap_or_else(|| format!("foto-{}", m.id));
        self.dibujos.contains(&dibujo)
    }
}

/// Lee los proyectos de `raiz`: la lista y el `proyecto.json` de cada uno.
/// Un proyecto sin `proyecto.json` (uno nacido aqui sin hojas) no aporta
/// mas que su id.
pub fn leer(raiz: &Path) -> Senas {
    let indice = pixpin_proyecto::almacen::Indice::leer(raiz);
    let ids: Vec<String> = indice.proyectos.iter().map(|f| f.id.clone()).collect();
    let proyectos: Vec<Proyecto> = ids
        .iter()
        .filter_map(|id| {
            let t = std::fs::read_to_string(
                pixpin_proyecto::almacen::carpeta(raiz, id).join("proyecto.json"),
            )
            .ok()?;
            serde_json::from_str(&t).ok()
        })
        .collect();
    Senas::de(&proyectos, &ids)
}

/// Las fechas de los ficheros de los que salen las senas: si no cambia
/// ninguna, las senas guardadas siguen valiendo.
fn huella(raiz: &Path) -> Vec<Option<SystemTime>> {
    let fecha = |r: PathBuf| std::fs::metadata(r).and_then(|m| m.modified()).ok();
    let mut v = vec![fecha(pixpin_proyecto::almacen::ruta(raiz))];
    for f in pixpin_proyecto::almacen::Indice::leer(raiz).proyectos {
        v.push(fecha(
            pixpin_proyecto::almacen::carpeta(raiz, &f.id).join("proyecto.json"),
        ));
    }
    v
}

/// Lo que se espera entre una mirada al disco y la siguiente. Se pregunta
/// por cada burbuja en cada fotograma; mirar las fechas de una docena de
/// ficheros cuatro veces por segundo no se nota, y unir algo se ve verde
/// antes de que la mano deje el raton.
const CADA: Duration = Duration::from_millis(250);

/// [`Senas::esta`] contra los proyectos de `raiz`, leidos solo cuando
/// cambian.
pub fn esta(raiz: &Path, m: &Mensaje) -> bool {
    struct Guardado {
        raiz: PathBuf,
        mirado: Instant,
        huella: Vec<Option<SystemTime>>,
        senas: Senas,
    }
    thread_local! {
        static CACHE: std::cell::RefCell<Option<Guardado>> = const { std::cell::RefCell::new(None) };
    }
    CACHE.with_borrow_mut(|c| {
        let fresco = c
            .as_ref()
            .is_some_and(|g| g.raiz == raiz && g.mirado.elapsed() < CADA);
        if !fresco {
            let h = huella(raiz);
            let igual = c.as_ref().is_some_and(|g| g.raiz == raiz && g.huella == h);
            if igual {
                if let Some(g) = c.as_mut() {
                    g.mirado = Instant::now();
                }
            } else {
                *c = Some(Guardado {
                    raiz: raiz.to_path_buf(),
                    mirado: Instant::now(),
                    huella: h,
                    senas: leer(raiz),
                });
            }
        }
        c.as_ref().is_some_and(|g| g.senas.esta(m))
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::Hoja;
    use serde_json::{json, Value};

    fn mensaje(id: &str, clase: Clase) -> Mensaje {
        Mensaje {
            id: id.into(),
            clase: Some(clase),
            ..Default::default()
        }
    }

    fn hoja_de(de_mensaje: &str) -> Hoja {
        let mut h = Hoja {
            id: format!("h-{de_mensaje}"),
            ..Default::default()
        };
        h.resto
            .insert("deMensaje".into(), Value::String(de_mensaje.into()));
        h
    }

    #[test]
    fn un_md_mandado_al_chat_de_un_proyecto_sin_unir_sale_rojo() {
        // El caso de la queja: proyecto nuevo, dos .md en su chat, ninguna hoja.
        let mut m = mensaje("1790825714183", Clase::Archivo);
        m.ruta = Some("archivos/Objetivos (1).md".into());
        m.nombre = "Objetivos (1).md".into();
        let senas = Senas::de(&[], &["5QX7DBS6BV".into()]);
        assert!(lleva_punto(&m));
        assert!(!senas.esta(&m), "mandarlo al chat no lo une");
    }

    #[test]
    fn lo_que_una_hoja_senala_con_de_mensaje_sale_verde() {
        let p = Proyecto {
            hojas: vec![hoja_de("m1")],
            ..Default::default()
        };
        let senas = Senas::de(&[p], &[]);
        assert!(senas.esta(&mensaje("m1", Clase::Imagen)));
        assert!(!senas.esta(&mensaje("m2", Clase::Imagen)), "otro mensaje no");
    }

    #[test]
    fn el_mismo_nombre_de_fichero_no_basta_para_salir_verde() {
        let p = Proyecto {
            hojas: vec![Hoja {
                nombre: "plano.pdf".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut m = mensaje("m9", Clase::Archivo);
        m.nombre = "plano.pdf".into();
        m.ruta = Some("archivos/plano.pdf".into());
        assert!(!Senas::de(&[p], &[]).esta(&m));
    }

    #[test]
    fn el_pdf_del_que_nace_un_proyecto_sale_verde_por_su_ruta_exacta() {
        let p = Proyecto {
            pdf_origen: Some("archivos/obra.pdf".into()),
            ..Default::default()
        };
        let senas = Senas::de(&[p], &[]);
        let mut m = mensaje("m1", Clase::Archivo);
        m.ruta = Some("archivos/obra.pdf".into());
        assert!(senas.esta(&m));
        m.ruta = Some("archivos/otra/obra.pdf".into());
        assert!(!senas.esta(&m), "otra ruta con el mismo nombre no");
    }

    #[test]
    fn un_mensaje_que_es_un_proyecto_sale_verde_mientras_el_proyecto_exista() {
        let mut m = mensaje("m1", Clase::Proyecto);
        m.referencia = Some("pr-1".into());
        assert!(Senas::de(&[], &["pr-1".into()]).esta(&m));
        assert!(!Senas::de(&[], &["pr-2".into()]).esta(&m), "borrado: rojo");
    }

    #[test]
    fn una_foto_unida_antes_del_vinculo_se_reconoce_por_su_dibujo() {
        let p = Proyecto {
            hojas: vec![
                Hoja {
                    dibujo: Some("foto-m1".into()),
                    ..Default::default()
                },
                Hoja {
                    dibujo: Some("lienzo-7".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let senas = Senas::de(&[p], &[]);
        assert!(senas.esta(&mensaje("m1", Clase::Imagen)), "sin referencia: foto-<id>");
        let mut d = mensaje("m2", Clase::Dibujo);
        d.referencia = Some("lienzo-7".into());
        assert!(senas.esta(&d), "con referencia: la referencia");
        assert!(!senas.esta(&mensaje("m3", Clase::Imagen)));
    }

    #[test]
    fn una_hoja_que_se_ensena_como_mensaje_sale_verde_por_su_codigo() {
        let p = Proyecto {
            hojas: vec![Hoja {
                uid: Some("H1".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut m = mensaje("sintetico", Clase::Pagina);
        m.uid = Some("H1".into());
        assert!(Senas::de(&[p.clone()], &[]).esta(&m));
        m.uid = Some("H2".into());
        assert!(!Senas::de(&[p], &[]).esta(&m));
    }

    #[test]
    fn la_nota_y_la_voz_no_llevan_punto() {
        assert!(!lleva_punto(&mensaje("n", Clase::Nota)));
        assert!(!lleva_punto(&mensaje("v", Clase::Voz)));
        assert!(!lleva_punto(&mensaje("a", Clase::MiniApp)));
        assert!(lleva_punto(&mensaje("t", Clase::Otra("TABLA".into()))));
    }

    #[test]
    fn leer_mira_las_hojas_de_todos_los_proyectos_del_disco() {
        let dir = std::env::temp_dir().join(format!("pixpin-punto-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("proyectos/A")).unwrap();
        std::fs::create_dir_all(dir.join("proyectos/B")).unwrap();
        std::fs::write(
            dir.join("proyectos/indice.json"),
            json!({"proyectos": [{"id": "A"}, {"id": "B"}]}).to_string(),
        )
        .unwrap();
        // B no tiene proyecto.json: nacio aqui y no tiene hojas.
        std::fs::write(
            dir.join("proyectos/A/proyecto.json"),
            json!({"id": "A", "hojas": [{"id": "h", "deMensaje": "de-guardados"}]}).to_string(),
        )
        .unwrap();
        // Un mensaje de otro chat unido a A, como hace el movil desde
        // «Mensajes guardados», sale verde; uno suelto, rojo.
        assert!(esta(&dir, &mensaje("de-guardados", Clase::Imagen)));
        assert!(!esta(&dir, &mensaje("suelto", Clase::Archivo)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
