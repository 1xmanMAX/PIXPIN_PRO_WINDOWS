//! **El PC y un movil de mentira, sincronizandose de verdad.**
//!
//! El «movil» es `DiscoAndroid`: `sincro/Disco.kt` portado sobre una carpeta
//! `files` como la del telefono, con los mensajes tal como los escribe
//! kotlinx. El PC es su almacen de siempre (`proyectos/indice.json`, una
//! carpeta por proyecto) visto por `vista::DiscoPc`. Se hablan por un socket
//! de esta maquina con el protocolo y el cifrado de verdad, en los dos
//! sentidos: el PC dirige y el movil responde, y al reves.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use pixpin_proyecto::almacen::{self, Ficha, Indice};
use pixpin_proyecto::cuaderno::{self, Clase, Cuaderno, Mensaje, Sello};
use pixpin_proyecto::vista::{self, DiscoPc};
use pixpin_sincro::canonico::{self, Json};
use pixpin_sincro::disco::{Disco, GENERAL};
use pixpin_sincro::disco_android::DiscoAndroid;
use pixpin_sincro::disco_android::prueba::{Reloj, conectado, crear_grupo, presentar, sincronizar};
use pixpin_sincro::kotlin;
use pixpin_sincro::protocolo::Hecho;
use serde_json::json;

struct Par {
    dir: PathBuf,
    pc: DiscoPc,
    movil: DiscoAndroid,
    reloj: Reloj,
}

impl Drop for Par {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn montar(etiqueta: &str) -> Par {
    let dir =
        std::env::temp_dir().join(format!("pixpin-pc-movil-{etiqueta}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let pc = DiscoPc::nuevo(&dir.join("PixPin Max"));
    let movil = DiscoAndroid::nuevo(dir.join("movil/files"), "movil");
    presentar(&pc, "id-pc", "Portátil");
    presentar(&movil, "id-movil", "Teléfono");
    let p = Par {
        dir,
        pc,
        movil,
        reloj: Reloj::nuevo(),
    };
    // El grupo lo crea el movil y el PC se une, como lo hizo el usuario.
    crear_grupo(&p.movil, "ABCDE23456", p.reloj.ahora());
    conectado(&p.pc, &p.movil, true, Some("ABCDE23456"), &p.reloj, |_| {}).unwrap();
    p
}

impl Par {
    fn raiz(&self) -> PathBuf {
        self.pc.raiz()
    }

    /// El PC dirige.
    fn desde_pc(&self, chats: &[&str]) -> Hecho {
        sincronizar(&self.pc, &self.movil, chats, &self.reloj, &|| {}).0
    }

    /// El movil dirige y el PC responde.
    fn desde_movil(&self, chats: &[&str]) -> Hecho {
        sincronizar(&self.movil, &self.pc, chats, &self.reloj, &|| {}).0
    }

    /// Una vuelta entera como la de la pantalla, sin preguntar.
    fn vuelta_desde_pc(&self) -> Hecho {
        let mut hecho = Hecho::default();
        conectado(&self.pc, &self.movil, false, None, &self.reloj, |s| {
            pixpin_sincro::vuelta::una(s, None, &mut hecho, "", &|| self.reloj.ahora(), &mut |_| {})
                .unwrap()
        })
        .unwrap();
        hecho
    }

    fn aparato_pc(&self) -> String {
        pixpin_proyecto::codigos::de_aparato("id-pc")
    }

    /// «Mensajes guardados» del PC.
    fn guardados(&self) -> Ficha {
        almacen::asegurar_guardados(&self.raiz(), 1, &self.aparato_pc()).unwrap()
    }

    /// Una nota escrita en el PC, como la escribe su chat.
    fn nota_pc(&self, ficha: &Ficha, texto: &str) -> Mensaje {
        let carpeta = almacen::carpeta(&self.raiz(), &ficha.id);
        let numero = Cuaderno::leer_de(&carpeta)
            .map(|c| c.mensajes.iter().map(|m| m.numero).max().unwrap_or(0))
            .unwrap_or(0)
            + 1;
        let m = Mensaje::nota(
            texto,
            &Sello {
                cuando: self.reloj.tic(),
                numero,
                aparato: self.aparato_pc(),
                proyecto: ficha.id.clone(),
            },
        );
        cuaderno::anadir(&carpeta, &m).unwrap();
        m
    }

    fn mensajes_pc(&self, ficha_id: &str) -> Vec<Mensaje> {
        Cuaderno::leer_de(&almacen::carpeta(&self.raiz(), ficha_id))
            .map(|c| c.mensajes)
            .unwrap_or_default()
    }

    fn nota_movil(&self, id: &str, texto: &str, proyecto: Option<&str>) {
        let numero = self
            .movil
            .leer_mensajes()
            .iter()
            .filter(|m| kotlin::cadena(m, "proyecto") == proyecto)
            .filter_map(|m| kotlin::numero(m, "numero"))
            .max()
            .unwrap_or(0)
            + 1;
        self.movil
            .anadir_mensaje(&Json::de_valor(&json!({
                "id": id, "cuando": self.reloj.tic(), "clase": "NOTA", "texto": texto,
                "proyecto": proyecto, "numero": numero, "letra": "a",
            })))
            .unwrap();
    }

    fn textos_movil(&self, chat: &str) -> BTreeSet<String> {
        self.movil
            .leer_mensajes()
            .iter()
            .filter(|m| kotlin::chat_de(m) == chat)
            .map(|m| kotlin::cadena(m, "texto").unwrap_or_default().to_string())
            .collect()
    }

    fn textos_pc(&self, ficha_id: &str) -> BTreeSet<String> {
        self.mensajes_pc(ficha_id)
            .into_iter()
            .map(|m| m.texto)
            .collect()
    }

    fn sello(&self, chat: &str) -> (String, String) {
        (
            self.pc.base("id-movil", chat).unwrap().sello(),
            self.movil.base("id-pc", chat).unwrap().sello(),
        )
    }

    /// Nada que hacer en una vuelta mas, dirija quien dirija.
    fn quieto(&self, chat: &str) {
        for pc_dirige in [true, false] {
            let comprobar = |s: &mut dyn FnMut(&str) -> (usize, usize)| {
                let (m, a) = s(chat);
                assert_eq!((m, a), (0, 0), "{chat}: mensajes y archivos quietos");
            };
            if pc_dirige {
                conectado(&self.pc, &self.movil, false, None, &self.reloj, |s| {
                    comprobar(&mut |c| {
                        let p = s.preparar(c).unwrap();
                        s.aplicar(&p, &mut Hecho::default()).unwrap();
                        (p.pasos.len(), s.preparar_archivos(&p).unwrap().pasos.len())
                    })
                })
                .unwrap();
            } else {
                conectado(&self.movil, &self.pc, false, None, &self.reloj, |s| {
                    comprobar(&mut |c| {
                        let p = s.preparar(c).unwrap();
                        s.aplicar(&p, &mut Hecho::default()).unwrap();
                        (p.pasos.len(), s.preparar_archivos(&p).unwrap().pasos.len())
                    })
                })
                .unwrap();
            }
        }
    }
}

fn gz(ruta: &Path, texto: &str) {
    std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
    let mut e = flate2::write::GzEncoder::new(
        std::fs::File::create(ruta).unwrap(),
        flate2::Compression::default(),
    );
    e.write_all(texto.as_bytes()).unwrap();
    e.finish().unwrap();
}

fn de_gz(ruta: &Path) -> String {
    let b = std::fs::read(ruta).unwrap();
    let mut s = String::new();
    flate2::read::GzDecoder::new(&b[..])
        .read_to_string(&mut s)
        .unwrap();
    s
}

/// Un proyecto nacido en el PC: una nota, una foto adjunta y un lienzo con
/// una imagen dentro, como los deja su chat.
fn proyecto_del_pc(p: &Par) -> Ficha {
    let raiz = p.raiz();
    let ficha = Ficha::nueva("Obra del PC", p.reloj.tic(), &p.aparato_pc());
    let mut i = Indice::leer(&raiz);
    i.proyectos.push(ficha.clone());
    i.guardar(&raiz).unwrap();
    p.nota_pc(&ficha, "medir la cocina");
    let ruta = almacen::guardar_adjunto(&raiz, &ficha.id, "captura.png", b"PNG del PC").unwrap();
    let carpeta = almacen::carpeta(&raiz, &ficha.id);
    let sello = Sello {
        cuando: p.reloj.tic(),
        numero: 2,
        aparato: p.aparato_pc(),
        proyecto: ficha.id.clone(),
    };
    cuaderno::anadir(
        &carpeta,
        &Mensaje::adjunto(Clase::Imagen, "captura.png", &ruta, 10, &sello),
    )
    .unwrap();
    std::fs::create_dir_all(carpeta.join("lienzos")).unwrap();
    std::fs::create_dir_all(carpeta.join("imagenes")).unwrap();
    std::fs::write(carpeta.join("imagenes/img1"), b"foto del lienzo").unwrap();
    std::fs::write(
        carpeta.join("lienzos/d9.excalidraw"),
        r#"{"type":"excalidraw","elements":[{"id":"A","type":"rectangle","x":1,"version":1,"versionNonce":3,"updated":1}],"files":{"img1":{"id":"img1","mimeType":"image/png","path":"imagenes/img1"}}}"#,
    )
    .unwrap();
    let mut dib = Mensaje::adjunto(
        Clase::Dibujo,
        "d9.excalidraw",
        "lienzos/d9.excalidraw",
        100,
        &Sello {
            cuando: p.reloj.tic(),
            numero: 3,
            ..sello
        },
    );
    dib.referencia = Some("d9".into());
    cuaderno::anadir(&carpeta, &dib).unwrap();
    ficha
}

/// Un proyecto del movil: un lienzo con su foto y un PDF adjunto.
fn proyecto_del_movil(p: &Par) {
    let m = &p.movil;
    let foto = m.raiz().join("pins/draw/files/foto1");
    std::fs::create_dir_all(foto.parent().unwrap()).unwrap();
    std::fs::write(&foto, [1, 2, 3]).unwrap();
    gz(
        &m.raiz().join("pins/draw/d1.excalidraw.gz"),
        &format!(
            r#"{{"elements":[{{"id":"A","type":"rectangle","x":0,"version":1,"versionNonce":7,"updated":1}}],"files":{{"foto1":{{"path":"{}"}}}}}}"#,
            m.absoluta("pins/draw/files/foto1")
        ),
    );
    let pdf = m.raiz().join("guardados/123_plano.pdf");
    std::fs::create_dir_all(pdf.parent().unwrap()).unwrap();
    std::fs::write(&pdf, b"%PDF-1.4 plano").unwrap();
    m.guardar_proyecto(
        &kotlin::normalizar_proyecto(&Json::de_valor(&json!({
            "id": "pr-1", "nombre": "Reforma", "tocado": 5,
            "hojas": [{"id": "h1", "nombre": "Planta", "dibujo": "d1"}],
            "uid": "RRRRRRRRRR", "creado": 1_726_000_000_000_i64, "aparato": "MOVI",
        })))
        .unwrap(),
    )
    .unwrap();
    for (id, clase, extra) in [
        (
            "m1",
            "DIBUJO",
            json!({"referencia": "d1", "nombre": "Planta"}),
        ),
        (
            "m2",
            "ARCHIVO",
            json!({"ruta": m.absoluta("guardados/123_plano.pdf"), "nombre": "plano.pdf"}),
        ),
    ] {
        let mut v = json!({"id": id, "cuando": p.reloj.tic(), "clase": clase, "proyecto": "pr-1",
                           "numero": if id == "m1" {1} else {2}, "letra": "a"});
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        m.anadir_mensaje(&Json::de_valor(&v)).unwrap();
    }
}

// ------------------------------------------------------------------ pruebas

#[test]
fn la_primera_vez_junta_todo_sin_pisar_y_la_segunda_no_mueve_nada() {
    let p = montar("primera");
    let g = p.guardados();
    p.nota_pc(&g, "del PC en guardados");
    p.nota_movil("n1", "del movil en la general", None);
    let obra = proyecto_del_pc(&p);
    proyecto_del_movil(&p);

    let hecho = p.vuelta_desde_pc();
    assert_eq!(hecho.fusionados, 0);

    // «Mensajes guardados» es la conversacion general del movil.
    assert_eq!(
        p.textos_movil(GENERAL),
        [
            "del PC en guardados".to_string(),
            "del movil en la general".into()
        ]
        .into()
    );
    assert_eq!(p.textos_pc(&g.id), p.textos_movil(GENERAL));

    // El proyecto del PC llega al movil con su nota, su foto y su lienzo.
    let chat_obra = vista::chat_de_ficha(&p.raiz(), &obra.id).unwrap();
    let del_movil = p
        .movil
        .leer_proyectos()
        .into_iter()
        .find(|x| kotlin::cadena(x, "id") == Some(chat_obra.as_str()))
        .expect("el proyecto del PC esta en el movil");
    assert_eq!(kotlin::cadena(&del_movil, "nombre"), Some("Obra del PC"));
    assert!(p.textos_movil(&chat_obra).contains("medir la cocina"));
    let captura = format!("guardados/pc/{chat_obra}/archivos/captura.png");
    assert_eq!(
        std::fs::read(p.movil.raiz().join(&captura)).unwrap(),
        b"PNG del PC"
    );
    let imagen = p
        .movil
        .leer_mensajes()
        .into_iter()
        .find(|m| kotlin::cadena(m, "clase") == Some("IMAGEN"))
        .unwrap();
    assert_eq!(
        kotlin::cadena(&imagen, "ruta"),
        Some(p.movil.absoluta(&captura).as_str())
    );
    let lienzo = de_gz(&p.movil.raiz().join("pins/draw/d9.excalidraw.gz"));
    let foto = format!("guardados/pc/{chat_obra}/imagenes/img1");
    assert!(lienzo.contains(&p.movil.absoluta(&foto)), "{lienzo}");
    assert_eq!(
        std::fs::read(p.movil.raiz().join(&foto)).unwrap(),
        b"foto del lienzo"
    );

    // El proyecto del movil llega al PC como una ficha mas, con su chat.
    let reforma = p.pc.ficha_de("pr-1").expect("ficha del proyecto del movil");
    assert_eq!(reforma.nombre, "Reforma");
    assert_eq!(reforma.id, "pr-1", "su carpeta se llama como su chat");
    let carpeta = almacen::carpeta(&p.raiz(), "pr-1");
    let lienzo = std::fs::read_to_string(carpeta.join("lienzos/d1.excalidraw")).unwrap();
    assert!(
        lienzo.contains("pixpin:files/pins/draw/files/foto1"),
        "{lienzo}"
    );
    assert_eq!(
        std::fs::read(carpeta.join("android/pins/draw/files/foto1")).unwrap(),
        [1, 2, 3]
    );
    let pdf = p
        .mensajes_pc("pr-1")
        .into_iter()
        .find(|m| m.clase == Some(Clase::Archivo))
        .unwrap();
    let real = vista::ruta_real(&p.raiz(), "pr-1", pdf.ruta.as_deref().unwrap()).unwrap();
    assert_eq!(std::fs::read(real).unwrap(), b"%PDF-1.4 plano");
    // El chat del PC lee el cuaderno como siempre.
    assert_eq!(p.mensajes_pc("pr-1").len(), 2);
    // Y no se inventan mensajes por sus hojas al abrirlo.
    assert_eq!(almacen::completar_hojas(&p.raiz(), "pr-1", "X").unwrap(), 0);

    // Lo acordado, con el mismo sello en los dos.
    for chat in [GENERAL, "pr-1", chat_obra.as_str()] {
        let (a, b) = p.sello(chat);
        assert_eq!(a, b, "{chat}");
        p.quieto(chat);
    }
}

#[test]
fn cuando_llama_el_movil_el_pc_responde_y_quedan_iguales() {
    let p = montar("llama-el-movil");
    let g = p.guardados();
    p.nota_pc(&g, "escrito en el PC");
    p.nota_movil("n1", "escrito en el movil", None);
    proyecto_del_movil(&p);
    p.desde_movil(&[GENERAL, "pr-1"]);
    assert_eq!(p.textos_pc(&g.id), p.textos_movil(GENERAL));
    assert_eq!(p.mensajes_pc("pr-1").len(), 2);
    assert!(
        almacen::carpeta(&p.raiz(), "pr-1")
            .join("lienzos/d1.excalidraw")
            .is_file()
    );
    let (a, b) = p.sello("pr-1");
    assert_eq!(a, b);
    p.quieto(GENERAL);
    p.quieto("pr-1");
}

#[test]
fn lo_cambiado_en_un_lado_pasa_y_en_los_dos_se_junta() {
    let p = montar("cambios");
    let g = p.guardados();
    let m = p.nota_pc(&g, "Introducción\n\nMétodo");
    p.desde_pc(&[GENERAL]);
    // Cambiado solo en el PC: pasa al movil, dirija quien dirija.
    let mut editado = m.clone();
    editado.texto = "Introducción\n\nMétodo corregido".into();
    cuaderno::reemplazar(&almacen::carpeta(&p.raiz(), &g.id), &editado).unwrap();
    assert_eq!(p.desde_movil(&[GENERAL]).fusionados, 0);
    assert_eq!(
        p.textos_movil(GENERAL),
        ["Introducción\n\nMétodo corregido".to_string()].into()
    );

    // Cambiado en los dos: se junta por parrafos.
    let mut en_pc = editado.clone();
    en_pc.texto = "Introducción ampliada\n\nMétodo corregido".into();
    cuaderno::reemplazar(&almacen::carpeta(&p.raiz(), &g.id), &en_pc).unwrap();
    let lista: Vec<Json> = p
        .movil
        .leer_mensajes()
        .into_iter()
        .map(|mut x| {
            kotlin::poner(
                &mut x,
                "texto",
                Json::cadena("Introducción\n\nMétodo corregido\n\nConclusiones"),
            );
            x
        })
        .collect();
    p.movil.escribir_mensajes(&lista).unwrap();
    assert_eq!(p.desde_pc(&[GENERAL]).fusionados, 1);
    let esperado = "Introducción ampliada\n\nMétodo corregido\n\nConclusiones".to_string();
    assert_eq!(p.textos_pc(&g.id), [esperado.clone()].into());
    assert_eq!(p.textos_movil(GENERAL), [esperado].into());
    p.quieto(GENERAL);
}

#[test]
fn lo_borrado_en_el_pc_deja_marca_y_se_borra_en_el_movil_sin_volver() {
    let p = montar("borrar-mensaje");
    let g = p.guardados();
    let a = p.nota_pc(&g, "para borrar");
    p.nota_pc(&g, "se queda");
    p.desde_pc(&[GENERAL]);
    // Como hace el chat del PC al borrar: fuera del cuaderno y su marca.
    let carpeta = almacen::carpeta(&p.raiz(), &g.id);
    let quedan: Vec<String> = std::fs::read_to_string(carpeta.join("guardados.jsonl"))
        .unwrap()
        .lines()
        .filter(|l| !l.contains("para borrar"))
        .map(str::to_string)
        .collect();
    std::fs::write(carpeta.join("guardados.jsonl"), quedan.join("\n") + "\n").unwrap();
    vista::anotar_borrados(&p.raiz(), &g.id, std::slice::from_ref(&a), p.reloj.tic()).unwrap();
    p.desde_movil(&[GENERAL]);
    assert_eq!(p.textos_movil(GENERAL), ["se queda".to_string()].into());
    p.desde_pc(&[GENERAL]);
    assert_eq!(
        p.textos_pc(&g.id),
        ["se queda".to_string()].into(),
        "no resucita"
    );
}

#[test]
fn un_proyecto_borrado_viaja_con_su_lapida_en_los_dos_sentidos() {
    let p = montar("lapidas");
    proyecto_del_movil(&p);
    let obra = proyecto_del_pc(&p);
    let chat_obra = vista::chat_de_ficha(&p.raiz(), &obra.id).unwrap();
    p.vuelta_desde_pc();
    assert_eq!(p.movil.leer_proyectos().len(), 2);

    p.reloj.saltar(1_000_000);
    // Borrado en el PC como lo borra su lista: a la papelera, con lapida.
    almacen::borrar_proyectos(&p.raiz(), std::slice::from_ref(&obra.id), p.reloj.ahora()).unwrap();
    p.vuelta_desde_pc();
    let ids: Vec<String> = p
        .movil
        .leer_proyectos()
        .iter()
        .map(|x| kotlin::cadena(x, "id").unwrap().to_string())
        .collect();
    assert_eq!(ids, ["pr-1"], "{chat_obra} borrado tambien en el movil");

    // Y al reves: borrado en el movil, se va del PC a la papelera.
    p.reloj.saltar(1_000_000);
    p.movil
        .borrar_chat("pr-1", "prueba", p.reloj.ahora(), "")
        .unwrap();
    p.vuelta_desde_pc();
    assert!(p.pc.ficha_de("pr-1").is_none());
    assert!(almacen::papelera(&p.raiz()).is_dir());
}

#[test]
fn un_lienzo_cambiado_en_los_dos_se_junta_y_uno_grande_viaja_en_parche() {
    let p = montar("lienzos");
    proyecto_del_movil(&p);
    p.desde_pc(&["pr-1"]);
    let en_pc = almacen::carpeta(&p.raiz(), "pr-1").join("lienzos/d1.excalidraw");
    let texto = std::fs::read_to_string(&en_pc).unwrap();
    // El PC anade B; el movil anade C.
    let con =
        |t: &str, fig: &str| t.replacen("\"elements\":[", &format!("\"elements\":[{fig},"), 1);
    std::fs::write(
        &en_pc,
        con(
            &texto,
            r#"{"id":"B","type":"ellipse","x":5,"version":1,"versionNonce":1,"updated":2}"#,
        ),
    )
    .unwrap();
    let movil = p.movil.raiz().join("pins/draw/d1.excalidraw.gz");
    let del_movil = de_gz(&movil);
    gz(
        &movil,
        &con(
            &del_movil,
            r#"{"id":"C","type":"line","x":9,"version":1,"versionNonce":1,"updated":3}"#,
        ),
    );
    let hecho = p.desde_movil(&["pr-1"]);
    assert_eq!(hecho.fusionados, 1);
    let rel = "pins/draw/d1.excalidraw.gz";
    let a = p.pc.texto_de("pr-1", rel).unwrap();
    let b = p.movil.texto_de("pr-1", rel).unwrap();
    assert_eq!(canonico::de(&a), canonico::de(&b));
    for f in ["\"A\"", "\"B\"", "\"C\""] {
        assert!(a.contains(f), "{f} en {a}");
    }
    p.quieto("pr-1");

    // Uno grande cambiado en una figura: viaja solo el cambio.
    let muchas: Vec<String> = (0..400)
        .map(|i| format!(r#"{{"id":"f{i}","type":"rectangle","x":{i},"version":1,"versionNonce":1,"updated":1}}"#))
        .collect();
    std::fs::write(
        &en_pc,
        format!(r#"{{"elements":[{}],"files":{{}}}}"#, muchas.join(",")),
    )
    .unwrap();
    p.desde_pc(&["pr-1"]);
    let t = std::fs::read_to_string(&en_pc).unwrap();
    std::fs::write(
        &en_pc,
        t.replace(
            r#""id":"f7","type":"rectangle","x":7,"version":1"#,
            r#""id":"f7","type":"rectangle","x":777,"version":2"#,
        ),
    )
    .unwrap();
    let hecho = p.desde_pc(&["pr-1"]);
    assert!(hecho.ahorrados > 0, "{hecho:?}");
    assert!(de_gz(&movil).contains("777"));
}

#[test]
fn un_proyecto_del_movil_sale_identico_tras_recibirlo_guardarlo_y_mandarlo() {
    let p = montar("identico");
    proyecto_del_movil(&p);
    p.desde_movil(&["pr-1"]);
    let del_movil = p.movil.proyecto_portatil("pr-1").unwrap().unwrap();
    let del_pc = p.pc.proyecto_portatil("pr-1").unwrap().unwrap();
    assert_eq!(del_pc.a_texto(), del_movil.a_texto(), "byte a byte");
    // Y sus mensajes, con el mismo resumen que calcula el movil.
    let resumenes = |d: &dyn Fn() -> Vec<Json>| -> BTreeSet<String> {
        d().iter().map(kotlin::resumen_de).collect()
    };
    assert_eq!(
        resumenes(&|| p.pc.mensajes("pr-1").unwrap()),
        resumenes(&|| p.movil.mensajes("pr-1").unwrap())
    );
    let textos = |l: Vec<Json>| -> BTreeSet<String> { l.iter().map(Json::a_texto).collect() };
    assert_eq!(
        textos(p.pc.mensajes("pr-1").unwrap()),
        textos(p.movil.mensajes("pr-1").unwrap()),
        "cada mensaje, byte a byte"
    );
}

#[test]
fn un_mensaje_del_movil_da_en_el_pc_el_resumen_que_da_en_el_movil() {
    // El vector: la linea tal cual la escribe `Disco.JSON` en el movil, y el
    // resumen derivado a mano de las reglas de `Disco.resumenDe` (sin
    // recuerdaEn, uid ni aparato; canonico; SHA-256). No se puede ejecutar
    // Kotlin aqui: el vector es por razonamiento.
    let linea = r#"{"id":"1757939357123","cuando":1757939357123,"clase":"ARCHIVO","texto":"","ruta":"pixpin:files/guardados/1757939357123_plano.pdf","nombre":"plano.pdf","bytes":2048,"referencia":null,"pagina":null,"duracionMs":0,"picos":[],"miniapp":null,"proyecto":"pr-1","emoji":"📐","soloLaFoto":true,"fijado":true,"respondeA":null,"enBuzon":false,"unido":false,"transcripcion":null,"estadoDelTexto":null,"hojaDelTexto":null,"marcas":[],"turnos":[],"numero":3,"letra":null,"uid":"RVK5YHKCX7","aparato":"K7Q2","origen":null,"recibidoDe":null,"vieneDe":null,"recuerdaEn":null}"#;
    let canonico_a_mano = r#"{"bytes":2048,"clase":"ARCHIVO","cuando":1757939357123,"duracionMs":0,"emoji":"📐","enBuzon":false,"fijado":true,"id":"1757939357123","marcas":[],"nombre":"plano.pdf","numero":3,"picos":[],"proyecto":"pr-1","ruta":"pixpin:files/guardados/1757939357123_plano.pdf","soloLaFoto":true,"texto":"","turnos":[],"unido":false}"#;
    let esperado = canonico::sha256_hex(canonico_a_mano.as_bytes());

    let p = montar("vector");
    p.pc.aplicar_mensajes("pr-1", &[linea.to_string()], &[], 1)
        .unwrap();
    let en_pc = p.pc.mensajes("pr-1").unwrap();
    assert_eq!(en_pc.len(), 1);
    assert_eq!(kotlin::texto_de_base(&en_pc[0]), canonico_a_mano);
    assert_eq!(kotlin::resumen_de(&en_pc[0]), esperado);
    assert_eq!(en_pc[0].a_texto(), linea, "sale como entro");
    // Lo lee el chat del PC.
    let m = &p.mensajes_pc("pr-1")[0];
    assert_eq!(m.nombre, "plano.pdf");
    assert_eq!(m.codigo_chat().as_deref(), Some("3·K7Q2"));
}
