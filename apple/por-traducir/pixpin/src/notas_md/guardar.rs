//! **De donde sale y adonde vuelve una nota**, sin ventanas: lo que se puede
//! probar con una carpeta de proyecto hecha en la prueba.
//!
//! Una nota Markdown de un proyecto vive como en el movil: en el movil es una
//! hoja con `nota = texto` dentro de `proyecto.json` (y su copia en
//! `notas/<id>.md`, que es la que se lee si falta la otra: `Proyecto::nota_de`);
//! en el chat del PC se ve como un mensaje `NOTA` con el mismo codigo unico
//! (`almacen::hojas_para_ensenar`). Guardar pone el texto **en los dos
//! sitios que la tengan**: el mensaje del cuaderno si esta, y la hoja del
//! proyecto si la hay. Asi el movil la abre igual en su editor
//! (`Proyectos.conNota`) y el chat la ensena al dia.
//!
//! Una nota nueva nace como mensaje `NOTA` del cuaderno, igual que el lienzo
//! o la tabla que se crean aqui: es lo que ya viaja con la sincronizacion.

use std::path::Path;

use pixpin_proyecto::{almacen, cuaderno};

use super::Destino;

/// El texto de la nota, o `None` si ya no esta (se borro, o el proyecto se
/// fue a la papelera): mejor no abrir que abrir en blanco y pisarla.
pub fn leer_texto(raiz: &Path, destino: &Destino) -> Option<String> {
    match destino {
        Destino::Nueva { .. } => Some(String::new()),
        Destino::Fichero { ruta } => std::fs::read(ruta)
            .ok()
            .map(|b| String::from_utf8_lossy(&b).into_owned()),
        Destino::Mensaje { proyecto, codigo } => {
            let carpeta = almacen::carpeta(raiz, proyecto);
            if let Ok(c) = cuaderno::Cuaderno::leer_de(&carpeta)
                && let Some(m) = c.mensajes.iter().find(|m| m.codigo_unico() == *codigo)
            {
                return Some(m.texto.clone());
            }
            let p = leer_proyecto(&carpeta)?;
            let h = p.hojas.iter().find(|h| h.codigo_unico() == *codigo)?;
            h.nota.clone().or_else(|| {
                std::fs::read_to_string(carpeta.join("notas").join(format!("{}.md", h.id))).ok()
            })
        }
    }
}

fn leer_proyecto(carpeta: &Path) -> Option<pixpin_proyecto::Proyecto> {
    let t = std::fs::read_to_string(carpeta.join("proyecto.json")).ok()?;
    serde_json::from_str(&t).ok()
}

/// Escribe a un temporal y cambia el nombre: si se va la luz a media
/// escritura queda la nota de antes entera.
fn escribir_atomico(ruta: &Path, datos: &[u8]) -> std::io::Result<()> {
    if let Some(padre) = ruta.parent() {
        std::fs::create_dir_all(padre)?;
    }
    let mut temporal = ruta.as_os_str().to_owned();
    temporal.push(".tmp");
    let temporal = std::path::PathBuf::from(temporal);
    std::fs::write(&temporal, datos)?;
    std::fs::rename(&temporal, ruta)
}

/// Guarda `texto` donde toca. Devuelve el destino de aqui en adelante: una
/// nota nueva, una vez guardada, es ya un mensaje del proyecto.
pub fn guardar(
    raiz: &Path,
    destino: &Destino,
    texto: &str,
    aparato: &str,
    ahora: i64,
) -> std::io::Result<Destino> {
    match destino {
        Destino::Fichero { ruta } => {
            escribir_atomico(ruta, texto.as_bytes())?;
            Ok(destino.clone())
        }
        Destino::Mensaje { proyecto, codigo } => {
            let carpeta = almacen::carpeta(raiz, proyecto);
            let mut puesta = false;
            if let Ok(c) = cuaderno::Cuaderno::leer_de(&carpeta)
                && let Some(m) = c.mensajes.into_iter().find(|m| m.codigo_unico() == *codigo)
            {
                let m = cuaderno::Mensaje {
                    texto: texto.to_string(),
                    ..m
                };
                puesta |= cuaderno::reemplazar(&carpeta, &m)?;
            }
            if let Some(mut p) = leer_proyecto(&carpeta)
                && let Some(h) = p.hojas.iter_mut().find(|h| h.codigo_unico() == *codigo)
            {
                // Las dos copias del movil: la de dentro del proyecto y la
                // de `notas/`, que es la que se lee si falta la otra.
                h.nota = Some(texto.to_string());
                let id = h.id.clone();
                p.tocado = p.tocado.max(ahora);
                escribir_atomico(
                    &carpeta.join("notas").join(format!("{id}.md")),
                    texto.as_bytes(),
                )?;
                escribir_atomico(
                    &carpeta.join("proyecto.json"),
                    &serde_json::to_vec_pretty(&p).map_err(std::io::Error::other)?,
                )?;
                puesta = true;
            }
            if !puesta {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "la nota ya no esta en su proyecto",
                ));
            }
            tocar_ficha(raiz, proyecto, ahora, None)?;
            Ok(destino.clone())
        }
        Destino::Nueva { proyecto } => {
            let carpeta = almacen::carpeta(raiz, proyecto);
            // Un proyecto que viene del movil tiene sus hojas en
            // `proyecto.json`: la nota entra ahi, como la crea el boton
            // «Nota» de Proyectos (`Hoja(id = "n-$ahora", nota = "")`), y
            // el movil la vera en su rejilla y la abrira en su editor.
            if let Some(mut p) = leer_proyecto(&carpeta) {
                let uid = pixpin_proyecto::codigos::nuevo();
                let id = format!("n-{ahora}");
                p.hojas.push(pixpin_proyecto::Hoja {
                    id: id.clone(),
                    nota: Some(texto.to_string()),
                    uid: Some(uid.clone()),
                    ..Default::default()
                });
                p.tocado = p.tocado.max(ahora);
                escribir_atomico(
                    &carpeta.join("notas").join(format!("{id}.md")),
                    texto.as_bytes(),
                )?;
                escribir_atomico(
                    &carpeta.join("proyecto.json"),
                    &serde_json::to_vec_pretty(&p).map_err(std::io::Error::other)?,
                )?;
                tocar_ficha(
                    raiz,
                    proyecto,
                    ahora,
                    Some(pixpin_docs::md_vivo::titulo(texto)),
                )?;
                return Ok(Destino::Mensaje {
                    proyecto: proyecto.clone(),
                    codigo: uid,
                });
            }
            // Uno nacido aqui no tiene hojas: la nota es un mensaje del chat,
            // como el lienzo o la tabla que se crean aqui.
            let previos = cuaderno::Cuaderno::leer_de(&carpeta).unwrap_or_default();
            let numero = previos.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
            let m = cuaderno::Mensaje::nota(
                texto,
                &cuaderno::Sello {
                    cuando: ahora,
                    numero,
                    aparato: aparato.to_string(),
                    proyecto: proyecto.clone(),
                },
            );
            std::fs::create_dir_all(&carpeta)?;
            cuaderno::anadir(&carpeta, &m)?;
            tocar_ficha(
                raiz,
                proyecto,
                ahora,
                Some(pixpin_docs::md_vivo::titulo(texto)),
            )?;
            Ok(Destino::Mensaje {
                proyecto: proyecto.clone(),
                codigo: m.codigo_unico(),
            })
        }
    }
}

/// La ficha del proyecto en el indice: cuando se toco y, al crear, el
/// resumen que ensena la lista (como hace el chat al crear un lienzo).
fn tocar_ficha(
    raiz: &Path,
    proyecto: &str,
    ahora: i64,
    resumen: Option<String>,
) -> std::io::Result<()> {
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == proyecto) {
        f.tocado = f.tocado.max(ahora);
        if let Some(r) = resumen {
            f.resumen = r;
        }
        indice.guardar(raiz)?;
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    struct Carpeta(std::path::PathBuf);
    impl Drop for Carpeta {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn carpeta(nombre: &str) -> Carpeta {
        let d = std::env::temp_dir().join(format!("pixpin-notas-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Carpeta(d)
    }

    fn sello(cuando: i64) -> cuaderno::Sello {
        cuaderno::Sello {
            cuando,
            numero: 1,
            aparato: "K7Q2".into(),
            proyecto: "p1".into(),
        }
    }

    #[test]
    fn una_nota_del_chat_se_lee_y_se_guarda_en_su_mensaje() {
        let c = carpeta("chat");
        let dir = almacen::carpeta(&c.0, "p1");
        std::fs::create_dir_all(&dir).unwrap();
        let m = cuaderno::Mensaje::nota("# Hola", &sello(10));
        cuaderno::anadir(&dir, &m).unwrap();
        let d = Destino::Mensaje {
            proyecto: "p1".into(),
            codigo: m.codigo_unico(),
        };
        assert_eq!(leer_texto(&c.0, &d).as_deref(), Some("# Hola"));
        guardar(&c.0, &d, "# Hola\n- [x] hecho", "K7Q2", 20).unwrap();
        let leido = cuaderno::Cuaderno::leer_de(&dir).unwrap();
        assert_eq!(leido.mensajes.len(), 1);
        assert_eq!(leido.mensajes[0].texto, "# Hola\n- [x] hecho");
        // El resto del mensaje no cambia: el movil lo reconoce por sus codigos.
        assert_eq!(leido.mensajes[0].uid, m.uid);
        assert_eq!(leido.mensajes[0].cuando, 10);
    }

    #[test]
    fn una_hoja_nota_del_movil_se_guarda_en_el_proyecto_y_en_su_copia() {
        let c = carpeta("hoja");
        let dir = almacen::carpeta(&c.0, "p1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("proyecto.json"),
            r#"{"id":"p1","nombre":"Obra","hojas":[{"id":"n-5","nombre":"","nota":"vieja","uid":"U1","queSeYo":7}],"campoNuevo":true}"#,
        )
        .unwrap();
        let d = Destino::Mensaje {
            proyecto: "p1".into(),
            codigo: "U1".into(),
        };
        assert_eq!(leer_texto(&c.0, &d).as_deref(), Some("vieja"));
        guardar(&c.0, &d, "**nueva**", "K7Q2", 99).unwrap();
        let p: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("proyecto.json")).unwrap())
                .unwrap();
        assert_eq!(p["hojas"][0]["nota"], "**nueva**");
        // Lo que el PC no entiende viaja igual: guardar no lo pierde.
        assert_eq!(p["hojas"][0]["queSeYo"], 7);
        assert_eq!(p["campoNuevo"], true);
        assert_eq!(
            std::fs::read_to_string(dir.join("notas").join("n-5.md")).unwrap(),
            "**nueva**"
        );
        // No se inventa un mensaje en el cuaderno: la hoja ya se ensena.
        assert!(cuaderno::Cuaderno::leer_de(&dir).is_err());
    }

    #[test]
    fn si_la_hoja_solo_tiene_su_copia_en_notas_se_lee_de_ahi() {
        let c = carpeta("copia");
        let dir = almacen::carpeta(&c.0, "p1");
        std::fs::create_dir_all(dir.join("notas")).unwrap();
        std::fs::write(
            dir.join("proyecto.json"),
            r#"{"id":"p1","hojas":[{"id":"n-9","uid":"U9"}]}"#,
        )
        .unwrap();
        std::fs::write(dir.join("notas").join("n-9.md"), "de la copia").unwrap();
        let d = Destino::Mensaje {
            proyecto: "p1".into(),
            codigo: "U9".into(),
        };
        assert_eq!(leer_texto(&c.0, &d).as_deref(), Some("de la copia"));
    }

    #[test]
    fn una_nota_que_ya_no_esta_ni_se_abre_ni_se_guarda() {
        let c = carpeta("falta");
        let d = Destino::Mensaje {
            proyecto: "p1".into(),
            codigo: "nadie".into(),
        };
        assert_eq!(leer_texto(&c.0, &d), None);
        assert!(guardar(&c.0, &d, "x", "K7Q2", 1).is_err());
    }

    #[test]
    fn una_nota_nueva_nace_como_mensaje_nota_y_pasa_a_ser_ese_mensaje() {
        let c = carpeta("nueva");
        let d = Destino::Nueva {
            proyecto: "p1".into(),
        };
        assert_eq!(leer_texto(&c.0, &d).as_deref(), Some(""));
        let despues = guardar(&c.0, &d, "## Presupuesto\n1. cemento", "K7Q2", 1234).unwrap();
        let dir = almacen::carpeta(&c.0, "p1");
        let leido = cuaderno::Cuaderno::leer_de(&dir).unwrap();
        assert_eq!(leido.mensajes.len(), 1);
        let m = &leido.mensajes[0];
        assert_eq!(m.clase, Some(cuaderno::Clase::Nota));
        assert_eq!(m.aparato.as_deref(), Some("K7Q2"));
        assert_eq!(
            despues,
            Destino::Mensaje {
                proyecto: "p1".into(),
                codigo: m.codigo_unico()
            }
        );
        // La segunda vez ya reescribe ese mismo mensaje, no crea otro.
        guardar(&c.0, &despues, "## Presupuesto", "K7Q2", 1300).unwrap();
        assert_eq!(cuaderno::Cuaderno::leer_de(&dir).unwrap().mensajes.len(), 1);
    }

    #[test]
    fn una_nota_nueva_en_un_proyecto_del_movil_es_una_hoja_como_alli() {
        let c = carpeta("nueva-hoja");
        let dir = almacen::carpeta(&c.0, "p1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("proyecto.json"),
            r#"{"id":"p1","nombre":"Obra","hojas":[{"id":"d1","dibujo":"d1","uid":"U0"}]}"#,
        )
        .unwrap();
        let d = Destino::Nueva {
            proyecto: "p1".into(),
        };
        let despues = guardar(&c.0, &d, "# Portada", "K7Q2", 777).unwrap();
        let p: pixpin_proyecto::Proyecto =
            serde_json::from_str(&std::fs::read_to_string(dir.join("proyecto.json")).unwrap())
                .unwrap();
        assert_eq!(p.hojas.len(), 2);
        let h = &p.hojas[1];
        assert_eq!(h.id, "n-777");
        assert_eq!(h.nota.as_deref(), Some("# Portada"));
        assert_eq!(
            std::fs::read_to_string(dir.join("notas").join("n-777.md")).unwrap(),
            "# Portada"
        );
        assert_eq!(
            despues,
            Destino::Mensaje {
                proyecto: "p1".into(),
                codigo: h.uid.clone().unwrap()
            }
        );
        // Y ya se lee y se reescribe como cualquier hoja nota.
        assert_eq!(leer_texto(&c.0, &despues).as_deref(), Some("# Portada"));
        guardar(&c.0, &despues, "# Portada 2", "K7Q2", 800).unwrap();
        assert_eq!(leer_texto(&c.0, &despues).as_deref(), Some("# Portada 2"));
    }

    #[test]
    fn un_md_suelto_se_escribe_tal_cual() {
        let c = carpeta("fichero");
        let ruta = c.0.join("apuntes.md");
        std::fs::write(&ruta, "antes").unwrap();
        let d = Destino::Fichero { ruta: ruta.clone() };
        assert_eq!(leer_texto(&c.0, &d).as_deref(), Some("antes"));
        guardar(&c.0, &d, "- [ ] despues", "K7Q2", 1).unwrap();
        assert_eq!(std::fs::read_to_string(&ruta).unwrap(), "- [ ] despues");
        assert!(!c.0.join("apuntes.md.tmp").exists());
    }
}
