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
        Destino::Fichero { ruta } => std::fs::read(ruta).ok().map(|b| {
            if super::es_markdown(ruta) {
                return String::from_utf8_lossy(&b).into_owned();
            }
            // Un texto llano (tambien en UTF-16, como los de Windows).
            let texto = pixpin_docs::texto_de(&b);
            match lenguaje_de(ruta) {
                Some(l) => envolver(l, &texto),
                None => texto,
            }
        }),
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
            let texto = match lenguaje_de(ruta) {
                Some(l) => desenvolver(l, texto),
                None => texto,
            };
            escribir_atomico(ruta, texto.as_bytes())?;
            Ok(destino.clone())
        }
        Destino::Mensaje { proyecto, codigo } => {
            let carpeta = almacen::carpeta(raiz, proyecto);
            let mut puesta = false;
            if let Ok(c) = cuaderno::Cuaderno::leer_de(&carpeta)
                && let Some(v) =
                    version_de_la_nota(raiz, proyecto, &c.mensajes, codigo, texto, aparato, ahora)
            {
                // **La nota de otra persona en un chat compartido**
                // (27-oct-2026): lo escrito va a mi version, que contesta a la
                // suya, y la suya no cambia; lo que se siga escribiendo ya va
                // a la mia (el destino de aqui en adelante).
                cuaderno::anadir(&carpeta, &v)?;
                tocar_ficha(raiz, proyecto, ahora, None)?;
                return Ok(Destino::Mensaje {
                    proyecto: proyecto.clone(),
                    codigo: v.codigo_unico(),
                });
            }
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

/// **Mi version de la nota `codigo`**, con `texto` dentro, si es de otra
/// persona de un chat compartido y lo escrito la cambia
/// (`pixpin_proyecto::versiones::al_momento`); `None` si se guarda en su
/// sitio, como siempre (es mia, el chat no es compartido, o no cambio).
fn version_de_la_nota(
    raiz: &Path,
    proyecto: &str,
    mensajes: &[cuaderno::Mensaje],
    codigo: &str,
    texto: &str,
    aparato: &str,
    ahora: i64,
) -> Option<cuaderno::Mensaje> {
    use pixpin_proyecto::versiones as vv;
    let m = mensajes.iter().find(|m| m.codigo_unico() == codigo)?;
    if m.texto == texto || vv::como(m) != Some(vv::Como::AlMomento) {
        return None;
    }
    crate::ventana_chat::versiones::de_quien(raiz, proyecto, m)?;
    let v = vv::al_momento(
        raiz,
        proyecto,
        m,
        mensajes,
        Some(aparato).filter(|a| !a.is_empty()),
        ahora,
        |mut v| {
            v.texto = texto.to_string();
            v
        },
        &vv::id_nuevo(ahora),
        &pixpin_proyecto::codigos::nuevo(),
    );
    Some(vv::con_numero(mensajes, v))
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

/// El lenguaje con que se colorea un fichero de codigo o de datos abierto
/// en el lector de notas, o `None` si se lee como texto (un `.txt`, un
/// `.log`, unos subtitulos) o es un Markdown.
pub fn lenguaje_de(ruta: &Path) -> Option<&'static str> {
    if !super::es_texto_para_nota(ruta) {
        return None;
    }
    let ext = pixpin_docs::extension(&pixpin_docs::nombre(ruta));
    Some(match ext.as_str() {
        "txt" | "log" | "srt" | "vtt" | "nfo" | "rst" => return None,
        "rs" => "rust",
        "py" => "python",
        "js" | "mjs" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "c" | "h" => "c",
        "cpp" | "hpp" => "cpp",
        "cs" => "csharp",
        "kt" => "kotlin",
        "rb" => "ruby",
        "sh" => "bash",
        "ps1" => "powershell",
        "bat" | "cmd" => "batch",
        "yml" | "yaml" => "yaml",
        "jsonl" | "json" => "json",
        "tex" => "latex",
        "bib" => "bibtex",
        "env" | "cfg" | "conf" | "ini" => "ini",
        "xml" => "xml",
        "toml" => "toml",
        "sql" => "sql",
        "go" => "go",
        "php" => "php",
        "lua" => "lua",
        "css" => "css",
        "scss" => "scss",
        "java" => "java",
        "csv" => "csv",
        "tsv" => "tsv",
        _ => "text",
    })
}

/// Las comillas del bloque: tres, o mas que las que traiga el texto.
fn valla(texto: &str) -> String {
    let mut mas_larga = 0;
    let mut seguidas = 0;
    for c in texto.chars() {
        seguidas = if c == '`' { seguidas + 1 } else { 0 };
        mas_larga = mas_larga.max(seguidas);
    }
    "`".repeat((mas_larga + 1).max(3))
}

/// El texto de un fichero de codigo como bloque de codigo de Markdown.
pub fn envolver(lenguaje: &str, texto: &str) -> String {
    let v = valla(texto);
    let salto = if texto.ends_with('\n') || texto.is_empty() {
        ""
    } else {
        "\n"
    };
    format!("{v}{lenguaje}\n{texto}{salto}{v}\n")
}

/// Lo contrario de [`envolver`]: lo de dentro del bloque. Si el bloque ya no
/// esta entero (se borro una valla al escribir), el texto tal cual.
pub fn desenvolver<'a>(lenguaje: &str, texto: &'a str) -> &'a str {
    let Some((primera, resto)) = texto.split_once('\n') else {
        return texto;
    };
    let v = primera.trim_end_matches(lenguaje);
    if v.len() < 3 || !v.chars().all(|c| c == '`') || primera.trim_end() != format!("{v}{lenguaje}")
    {
        return texto;
    }
    let sin_final = resto.trim_end_matches(['\n', '\r']);
    match sin_final.strip_suffix(v) {
        Some(dentro) if dentro.is_empty() || dentro.ends_with('\n') => dentro,
        _ => texto,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_codigo_va_en_su_bloque_y_vuelve_tal_cual() {
        for (l, t) in [
            ("json", "{\n  \"a\": 1\n}\n"),
            ("python", "# titulo\nprint(1)\n"),
            ("rust", "fn main() {}\n"),
            ("text", ""),
            ("bash", "echo ```hola```\n"),
        ] {
            let e = envolver(l, t);
            assert!(e.starts_with("```"), "{e}");
            assert_eq!(desenvolver(l, &e), t, "{l}");
        }
        // Sin salto al final: vuelve con uno.
        assert_eq!(desenvolver("json", &envolver("json", "{}")), "{}\n");
        // Caso negativo: si se rompio el bloque, se guarda lo que haya.
        assert_eq!(desenvolver("json", "{}\n"), "{}\n");
        assert_eq!(desenvolver("json", "```json\n{}\n"), "```json\n{}\n");
    }

    #[test]
    fn los_textos_van_al_lector_de_notas_y_el_codigo_coloreado() {
        use std::path::Path;
        for n in [
            "a.txt", "b.log", "c.csv", "d.json", "e.py", "f.xml", "g.yaml",
        ] {
            assert!(super::super::es_texto_para_nota(Path::new(n)), "{n}");
        }
        for n in ["a.md", "b.html", "c.pdf", "d.png", "e.docx"] {
            assert!(!super::super::es_texto_para_nota(Path::new(n)), "{n}");
        }
        assert_eq!(lenguaje_de(Path::new("a.txt")), None);
        assert_eq!(lenguaje_de(Path::new("a.md")), None);
        assert_eq!(lenguaje_de(Path::new("a.py")), Some("python"));
        assert_eq!(lenguaje_de(Path::new("a.JSON")), Some("json"));
    }

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
