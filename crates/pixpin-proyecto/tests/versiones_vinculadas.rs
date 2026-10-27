//! **Las versiones vinculadas, sin red** (27-oct-2026): las mismas pruebas
//! que `VersionesVinculadasTest.kt` de PixPin Android v0.145.0. Que es
//! ajeno, que se versiona, como se llama la copia y que lleva, que el
//! vinculo se conserva al guardar y al fusionar, y que un borrador que no se
//! toco no deja nada. Ver `pixpin_proyecto::versiones`.

use std::path::{Path, PathBuf};

use pixpin_proyecto::almacen::{self, Ficha, Indice};
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje};
use pixpin_proyecto::versiones::{self as vv, Como};
use pixpin_ui::conversacion::{Lados, Quien};

fn raiz(etiqueta: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!(
        "pixpin-versiones-{etiqueta}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&r);
    std::fs::create_dir_all(&r).unwrap();
    r
}

/// Un almacen con un chat (su ficha) y nada mas.
fn con_un_chat(etiqueta: &str) -> (PathBuf, String) {
    let r = raiz(etiqueta);
    let ficha = Ficha::nueva("Obra", 5, "MAXX");
    let mut i = Indice::default();
    i.proyectos.push(ficha.clone());
    i.guardar(&r).unwrap();
    std::fs::create_dir_all(almacen::carpeta(&r, &ficha.id)).unwrap();
    (r, ficha.id)
}

/// Max (`MAXX`, y su PC `MAPC`) y Ana (`ANA1`) en un chat compartido, visto
/// desde el telefono de Max.
fn lados() -> Lados {
    Lados::nuevo(
        ["MAXX".to_string(), "MAPC".to_string()],
        [
            ("MAXX".to_string(), "Max".to_string()),
            ("ANA1".to_string(), "Ana".to_string()),
            ("MAPC".to_string(), "Max".to_string()),
        ],
    )
}

fn de_otro(l: &Lados) -> impl Fn(Option<&str>) -> bool + '_ {
    |a| matches!(l.quien(a), Quien::Otro(_))
}

#[allow(clippy::too_many_arguments)]
fn m(
    id: &str,
    clase: Clase,
    aparato: Option<&str>,
    texto: &str,
    ruta: Option<&str>,
    referencia: Option<&str>,
    miniapp: Option<&str>,
) -> Mensaje {
    let mut m = Mensaje {
        id: id.into(),
        cuando: 1_000,
        clase: Some(clase),
        texto: texto.into(),
        ruta: ruta.map(str::to_string),
        referencia: referencia.map(str::to_string),
        proyecto: Some("p1".into()),
        numero: 3,
        aparato: aparato.map(str::to_string),
        uid: Some(pixpin_proyecto::codigos::de(&format!("u:{id}"))),
        miniapp: miniapp.map(str::to_string),
        fijado: true,
        emoji: Some("⭐".into()),
        ..Default::default()
    };
    m.resto
        .insert("unido".into(), serde_json::Value::Bool(true));
    m
}

fn dibujo(clase: Clase, aparato: &str, referencia: &str) -> Mensaje {
    m("a", clase, Some(aparato), "", None, Some(referencia), None)
}

/// Un lienzo del PC (`lienzos/<id>.excalidraw`) con un elemento de texto.
fn lienzo(r: &Path, ficha: &str, id: &str, texto: &str, version: i64) {
    let f = almacen::lienzo(r, ficha, id);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(
        f,
        format!(
            r#"{{"elements":[{{"id":"e1","type":"text","text":"{texto}","version":{version},"versionNonce":1,"updated":1}}],"files":{{}}}}"#
        ),
    )
    .unwrap();
}

/// Donde esta en este equipo un archivo portatil del chat de `ficha`.
fn en_disco(r: &Path, ficha: &str, rel: &str) -> PathBuf {
    use pixpin_sincro::disco::Disco;
    let chat = pixpin_proyecto::vista::chat_de_ficha(r, ficha).unwrap();
    pixpin_proyecto::vista::DiscoPc::nuevo(r).ruta(&chat, rel)
}

// ------------------------------------------------------------------ decidir

#[test]
fn es_ajeno_solo_lo_de_otra_persona_en_un_chat_compartido() {
    let l = lados();
    let f = de_otro(&l);
    let f: Option<vv::DeOtro> = Some(&f);
    assert!(vv::es_ajeno(&dibujo(Clase::Dibujo, "ANA1", "x"), f));
    // Lo de mis aparatos, lo sin codigo y lo del asistente es mio.
    assert!(!vv::es_ajeno(&dibujo(Clase::Dibujo, "MAXX", "x"), f));
    assert!(!vv::es_ajeno(&dibujo(Clase::Dibujo, "MAPC", "x"), f));
    let sin = m("d", Clase::Dibujo, None, "", None, Some("x"), None);
    assert!(!vv::es_ajeno(&sin, f));
    let ia = m("e", Clase::Nota, Some("IA"), "", None, None, None);
    assert!(!vv::es_ajeno(&ia, f));
    // El codigo largo de su aparato se acorta igual que en la burbuja.
    let largo = "id-del-movil-de-ana";
    let con_largo = Lados::nuevo(
        ["MAXX".to_string()],
        [(
            pixpin_proyecto::codigos::de_aparato(largo),
            "Ana".to_string(),
        )],
    );
    let g = de_otro(&con_largo);
    let nota = m("f", Clase::Nota, Some(largo), "", None, None, None);
    assert!(vv::es_ajeno(&nota, Some(&g)));
    // En un chat que no es compartido, nada es ajeno: todo se edita en su
    // sitio, como siempre.
    assert!(!vv::es_ajeno(&dibujo(Clase::Dibujo, "ANA1", "x"), None));
    assert!(!vv::se_versiona(&dibujo(Clase::Dibujo, "ANA1", "x"), false));
}

#[test]
fn que_se_versiona_y_como() {
    let c = |clase, ruta: Option<&str>, referencia: Option<&str>, miniapp: Option<&str>| {
        vv::como(&m(
            "1",
            clase,
            Some("ANA1"),
            "hola",
            ruta,
            referencia,
            miniapp,
        ))
    };
    assert_eq!(
        c(Clase::Dibujo, None, Some("x"), None),
        Some(Como::Borrador)
    );
    assert_eq!(
        c(
            Clase::Pagina,
            Some("pixpin:files/guardados/a.pdf"),
            Some("x"),
            None
        ),
        Some(Como::Borrador)
    );
    assert_eq!(
        c(
            Clase::Imagen,
            Some("pixpin:files/guardados/a.jpg"),
            None,
            None
        ),
        Some(Como::Borrador)
    );
    assert_eq!(
        c(
            Clase::Archivo,
            Some("pixpin:files/guardados/1_plano.pdf"),
            None,
            None
        ),
        Some(Como::Borrador)
    );
    assert_eq!(
        c(Clase::Archivo, Some("archivos/informe.docx"), None, None),
        Some(Como::Borrador)
    );
    assert_eq!(
        c(Clase::MiniApp, None, None, Some("gastos")),
        Some(Como::Provisional)
    );
    assert_eq!(
        c(Clase::MiniApp, None, None, Some("carpeta")),
        Some(Como::Provisional)
    );
    assert_eq!(c(Clase::Nota, None, None, None), Some(Como::AlMomento));
    // Lo que no: una lista de tareas (tachar en una compartida es lo que se
    // quiere), una tabla del chat (solo se mira), un video, un zip, un audio
    // y un lienzo sin dibujo.
    assert_eq!(c(Clase::MiniApp, None, None, Some("tareas")), None);
    assert_eq!(c(Clase::MiniApp, None, None, Some("tabla")), None);
    assert_eq!(
        c(
            Clase::Imagen,
            Some("pixpin:files/guardados/v.mp4"),
            None,
            None
        ),
        None
    );
    assert_eq!(
        c(
            Clase::Archivo,
            Some("pixpin:files/guardados/a.zip"),
            None,
            None
        ),
        None
    );
    assert_eq!(
        c(Clase::Voz, Some("pixpin:files/guardados/a.m4a"), None, None),
        None
    );
    assert_eq!(c(Clase::Dibujo, None, None, None), None);
    // Mis cosas no se versionan: se editan en su sitio.
    let l = lados();
    let f = de_otro(&l);
    let mio = dibujo(Clase::Dibujo, "MAXX", "x");
    assert!(!vv::se_versiona(&mio, vv::es_ajeno(&mio, Some(&f))));
    let suyo = dibujo(Clase::Dibujo, "ANA1", "x");
    assert!(vv::se_versiona(&suyo, vv::es_ajeno(&suyo, Some(&f))));
}

// ------------------------------------------------------------------ la copia

#[test]
fn la_version_es_otro_mensaje_mio_que_contesta_al_original() {
    let mut original = m(
        "orig",
        Clase::Dibujo,
        Some("ANA1"),
        "planta",
        None,
        Some("lienzo-ana"),
        None,
    );
    original.letra = Some("a".into());
    original.origen = Some("ANA1".into());
    original.en_buzon = true;
    original
        .resto
        .insert("recuerdaEn".into(), serde_json::json!(99));
    original
        .resto
        .insert("recibidoDe".into(), serde_json::json!("Ana"));
    original
        .resto
        .insert("hojaDelTexto".into(), serde_json::json!("h1"));
    original
        .resto
        .insert("escaneado".into(), serde_json::json!({"x": 1}));
    let v = vv::nueva(&original, "nuevo", "NUEVOUID23", Some("MAXX"), 5_000, 2);
    assert_eq!(v.id, "nuevo");
    assert_eq!(v.uid.as_deref(), Some("NUEVOUID23"));
    assert_eq!(v.aparato.as_deref(), Some("MAXX"));
    assert_eq!(v.cuando, 5_000);
    assert_eq!(v.version_de, Some(original.codigo_unico()));
    assert_eq!(v.version_n, Some(2));
    assert_eq!(v.responde_a.as_deref(), Some("orig"));
    assert_eq!(v.proyecto.as_deref(), Some("p1"));
    assert_eq!(v.texto, "planta");
    // Lo que es de la organizacion del chat o de este aparato no se copia; el
    // numero lo pone quien guarda.
    assert_eq!(v.numero, 0);
    assert!(!v.fijado);
    assert_eq!(v.emoji, None);
    assert!(!v.en_buzon);
    assert_eq!((v.letra.as_deref(), v.origen.as_deref()), (None, None));
    for k in ["unido", "recuerdaEn", "recibidoDe", "hojaDelTexto"] {
        assert!(!v.resto.contains_key(k), "{k} no se copia");
    }
    // Lo demas, si: lo que el PC no entiende viaja con la copia.
    assert_eq!(v.resto.get("escaneado"), Some(&serde_json::json!({"x": 1})));
    // Y se numera como cualquier mensaje nuevo de su chat.
    assert_eq!(vv::con_numero(std::slice::from_ref(&original), v).numero, 4);
    // Una nota que abre una hoja de proyecto: la copia ya no es esa hoja.
    let nota = m(
        "n",
        Clase::Nota,
        Some("ANA1"),
        "hola",
        None,
        Some("hoja"),
        None,
    );
    assert_eq!(
        vv::nueva(&nota, "x", "XXXXXXXXXX", None, 1, 1).referencia,
        None
    );
}

#[test]
fn el_vinculo_se_guarda_se_lee_y_se_fusiona_sin_perderse() {
    use pixpin_sincro::canonico::Json;
    use pixpin_sincro::fusion::{self, Criterio, Cuenta};
    let original = dibujo(Clase::Dibujo, "ANA1", "lienzo-ana");
    let v = vv::nueva(&original, "nuevo", "NUEVOUID23", Some("MAXX"), 5_000, 1);
    let linea = cuaderno::linea_de(&v).unwrap();
    let obj: serde_json::Value = serde_json::from_str(&linea).unwrap();
    assert_eq!(obj["versionDe"], serde_json::json!(original.codigo_unico()));
    assert_eq!(obj["versionN"], serde_json::json!(1));
    let leida = cuaderno::Cuaderno::leer(&linea).mensajes.remove(0);
    assert_eq!(leida.version_de, v.version_de);
    assert_eq!(leida.version_n, v.version_n);
    // Un mensaje normal no lleva las claves.
    let normal = cuaderno::linea_de(&original).unwrap();
    assert!(!normal.contains("versionDe"));
    assert!(!normal.contains("versionN"));
    // La fusion de tres vias lo conserva: yo lo fijo aqui y la otra persona
    // no lo toca.
    let base = Json::analizar(&linea).unwrap();
    let mut fijada = v.clone();
    fijada.fijado = true;
    let mio = Json::analizar(&cuaderno::linea_de(&fijada).unwrap()).unwrap();
    let junto = fusion::json(
        Some(&base),
        Some(&mio),
        Some(&base),
        &Criterio::default(),
        &mut Cuenta::default(),
    )
    .unwrap()
    .a_valor();
    assert_eq!(
        junto["versionDe"],
        serde_json::json!(original.codigo_unico())
    );
    assert_eq!(junto["fijado"], serde_json::json!(true));
    // Reenviar una version a otro chat ya no es version de nada de alli.
    let mut reenviada = v.clone();
    vv::sin_vinculo(&mut reenviada);
    assert_eq!((reenviada.version_de, reenviada.version_n), (None, None));
}

// ------------------------------------------------------------------ borradores

#[test]
fn un_lienzo_ajeno_se_edita_en_una_copia_y_el_original_no_cambia() {
    let (r, ficha) = con_un_chat("lienzo");
    lienzo(&r, &ficha, "lienzo-ana", "planta de Ana", 1);
    let original = m(
        "orig",
        Clase::Dibujo,
        Some("ANA1"),
        "",
        None,
        Some("lienzo-ana"),
        None,
    );
    let lista = vec![original.clone()];
    let e = vv::empezar(
        &r,
        &ficha,
        &original,
        &lista,
        Some("MAXX"),
        5_000,
        "v1",
        "VERSIONUID",
    )
    .unwrap();
    assert!(!e.provisional);
    assert_eq!(e.version.referencia.as_deref(), Some("dib-v1"));
    assert!(almacen::lienzo(&r, &ficha, "dib-v1").is_file());
    // Sin tocar: nada que anadir, y el borrador espera por si el editor sigue
    // abierto.
    assert!(vv::cerrar_en(&r, &ficha, &lista, 6_000).unwrap().nada());
    assert_eq!(vv::en_curso(&r).len(), 1);
    // Volver a abrirlo sigue con el mismo borrador.
    let otra_vez = vv::empezar(
        &r,
        &ficha,
        &original,
        &lista,
        Some("MAXX"),
        7_000,
        "zz",
        "ZZZZZZZZZZ",
    )
    .unwrap();
    assert_eq!(otra_vez.version.id, "v1");
    // El editor escribe en la copia (otra `version` del elemento).
    lienzo(&r, &ficha, "dib-v1", "planta de Ana con lo de Max", 2);
    let c = vv::cerrar_en(&r, &ficha, &lista, 8_000).unwrap();
    assert_eq!(
        c.anadir.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
        ["v1"]
    );
    assert_eq!(c.anadir[0].cuando, 8_000);
    assert!(vv::en_curso(&r).is_empty());
    assert!(
        !vv::hay_en_curso(&r),
        "sin nada apuntado no queda el fichero"
    );
    let chat = vv::con_versiones(&lista, &c);
    assert_eq!(
        chat.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["orig", "v1"]
    );
    assert_eq!(
        vv::versiones_de(&chat, &original)
            .iter()
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>(),
        ["v1"]
    );
    assert_eq!(
        vv::original_de(&chat, &chat[1]).map(|m| m.id.as_str()),
        Some("orig")
    );
    // El original, tal cual.
    assert_eq!(chat[0].referencia.as_deref(), Some("lienzo-ana"));
    assert_eq!(
        vv::huella_de_dibujo(&almacen::lienzo(&r, &ficha, "lienzo-ana"), false),
        "e1:1"
    );
    // Abrir otra vez el original hace la version 2, otra copia.
    let otra = vv::empezar(
        &r,
        &ficha,
        &original,
        &chat,
        Some("MAXX"),
        9_000,
        "v2",
        "VERSIONUI2",
    )
    .unwrap();
    assert_eq!(otra.version.version_n, Some(2));
    assert_eq!(otra.version.referencia.as_deref(), Some("dib-v2"));
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn un_borrador_que_nadie_toca_se_tira_a_las_doce_horas() {
    let (r, ficha) = con_un_chat("caduca");
    lienzo(&r, &ficha, "lienzo-ana", "planta", 1);
    let original = dibujo(Clase::Dibujo, "ANA1", "lienzo-ana");
    let lista = vec![original.clone()];
    vv::empezar(
        &r,
        &ficha,
        &original,
        &lista,
        Some("MAXX"),
        5_000,
        "v1",
        "VERSIONUID",
    )
    .unwrap();
    assert!(
        vv::cerrar_en(&r, &ficha, &lista, 5_000 + vv::CADUCA_MS + 1)
            .unwrap()
            .nada()
    );
    assert!(vv::en_curso(&r).is_empty());
    assert!(!almacen::lienzo(&r, &ficha, "dib-v1").exists());
    assert!(almacen::lienzo(&r, &ficha, "lienzo-ana").exists());
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn mi_tinta_sobre_el_pdf_de_otra_persona_va_en_mi_copia_del_pdf() {
    let (r, ficha) = con_un_chat("pdf");
    let pdf = en_disco(&r, &ficha, "guardados/1761_plano.pdf");
    std::fs::create_dir_all(pdf.parent().unwrap()).unwrap();
    std::fs::write(&pdf, "%PDF plano").unwrap();
    let mut original = m(
        "orig",
        Clase::Archivo,
        Some("ANA1"),
        "",
        Some("pixpin:files/guardados/1761_plano.pdf"),
        None,
        None,
    );
    original.nombre = "plano.pdf".into();
    let suyo = original.codigo_unico();
    // Lo que Ana ya anoto: su tinta en la pagina 0 y sus marcadores.
    let tinta_de_ana = en_disco(
        &r,
        &ficha,
        &format!("pins/draw/anot-{suyo}-p0.excalidraw.gz"),
    );
    lienzo(&r, &ficha, &format!("anot-{suyo}-p0"), "flecha de Ana", 1);
    let marcas = en_disco(&r, &ficha, &format!("pins/draw/anot-{suyo}.marcas"));
    std::fs::create_dir_all(marcas.parent().unwrap()).unwrap();
    std::fs::write(&marcas, "1|Portada").unwrap();
    let lista = vec![original.clone()];
    let e = vv::empezar(
        &r,
        &ficha,
        &original,
        &lista,
        Some("MAXX"),
        5_000,
        "v1",
        "VERSIONUID",
    )
    .unwrap();
    // La copia, con el nombre del movil y en la ruta portatil de siempre.
    assert_eq!(
        e.version.ruta.as_deref(),
        Some("pixpin:files/guardados/5000_v1_plano.pdf")
    );
    let copia = en_disco(&r, &ficha, "guardados/5000_v1_plano.pdf");
    assert_eq!(std::fs::read_to_string(&copia).unwrap(), "%PDF plano");
    assert_eq!(e.version.nombre, "plano.pdf");
    // Lo anotado viene con la copia, con el codigo de la version, y el lector
    // lo encuentra por la ruta.
    assert!(en_disco(&r, &ficha, "pins/draw/anot-VERSIONUID-p0.excalidraw.gz").is_file());
    assert_eq!(
        std::fs::read_to_string(en_disco(&r, &ficha, "pins/draw/anot-VERSIONUID.marcas")).unwrap(),
        "1|Portada"
    );
    let (_, uid) = vv::borrador_de(&r, &copia).expect("es un borrador");
    assert_eq!(uid, "VERSIONUID");
    let x = pixpin_proyecto::anotado::adjunto_de(&r, &copia).expect("el lector la encuentra");
    assert_eq!(x.uid, "VERSIONUID");
    // Leer (por donde iba) no es cambiar.
    std::fs::write(
        en_disco(&r, &ficha, "pins/draw/anot-VERSIONUID.sitio"),
        "0.5",
    )
    .unwrap();
    assert!(vv::cerrar_en(&r, &ficha, &lista, 6_000).unwrap().nada());
    // Mi raya, si.
    lienzo(
        &r,
        &ficha,
        "anot-VERSIONUID-p0",
        "flecha de Ana y raya de Max",
        3,
    );
    let c = vv::cerrar_en(&r, &ficha, &lista, 7_000).unwrap();
    assert_eq!(c.anadir[0].ruta, e.version.ruta);
    assert_eq!(vv::huella_de_dibujo(&tinta_de_ana, false), "e1:1");
    assert_eq!(std::fs::read_to_string(&pdf).unwrap(), "%PDF plano");
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn una_foto_sin_dibujo_no_cuenta_la_foto_de_fondo_como_cambio() {
    let (r, ficha) = con_un_chat("foto");
    let foto = en_disco(&r, &ficha, "guardados/1_foto.jpg");
    std::fs::create_dir_all(foto.parent().unwrap()).unwrap();
    std::fs::write(&foto, "jpg").unwrap();
    let original = m(
        "orig",
        Clase::Imagen,
        Some("ANA1"),
        "",
        Some("pixpin:files/guardados/1_foto.jpg"),
        None,
        None,
    );
    let lista = vec![original.clone()];
    let e = vv::empezar(
        &r,
        &ficha,
        &original,
        &lista,
        Some("MAXX"),
        5_000,
        "v1",
        "VERSIONUID",
    )
    .unwrap();
    assert_eq!(e.version.referencia, None);
    assert!(e.sin_fotos);
    // El editor pone la foto de fondo al abrir: no es un cambio.
    let f = almacen::lienzo(&r, &ficha, "foto-v1");
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(
        &f,
        r#"{"elements":[{"id":"bg","type":"image","version":1}]}"#,
    )
    .unwrap();
    assert!(vv::cerrar_en(&r, &ficha, &lista, 6_000).unwrap().nada());
    std::fs::write(
        &f,
        r#"{"elements":[{"id":"bg","type":"image","version":1},{"id":"r","type":"line","version":1}]}"#,
    )
    .unwrap();
    assert_eq!(
        vv::cerrar_en(&r, &ficha, &lista, 7_000)
            .unwrap()
            .anadir
            .len(),
        1
    );
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn una_mini_app_provisional_que_no_se_toca_se_quita() {
    let (r, ficha) = con_un_chat("mini");
    let original = m(
        "orig",
        Clase::MiniApp,
        Some("ANA1"),
        "# Viaje\n- 10 gasolina",
        None,
        None,
        Some("gastos"),
    );
    let lista = vec![original.clone()];
    let e = vv::empezar(
        &r,
        &ficha,
        &original,
        &lista,
        Some("MAXX"),
        5_000,
        "v1",
        "VERSIONUID",
    )
    .unwrap();
    assert!(e.provisional);
    let entra = vv::Cierre {
        anadir: vec![e.version.clone()],
        ..Default::default()
    };
    let chat = vv::con_versiones(&lista, &entra);
    assert_eq!(chat.len(), 2);
    assert_eq!(
        vv::cerrar_en(&r, &ficha, &chat, 6_000).unwrap().quitar,
        ["v1".to_string()].into()
    );
    // Y si se cambia, se queda.
    let e2 = vv::empezar(
        &r,
        &ficha,
        &original,
        &lista,
        Some("MAXX"),
        7_000,
        "v2",
        "VERSIONUI2",
    )
    .unwrap();
    let entra = vv::Cierre {
        anadir: vec![e2.version.clone()],
        ..Default::default()
    };
    let mut chat = vv::con_versiones(&lista, &entra);
    chat[1].texto.push_str("\n- 20 peaje");
    assert!(vv::cerrar_en(&r, &ficha, &chat, 8_000).unwrap().nada());
    assert!(vv::en_curso(&r).is_empty());
    assert_eq!(chat[0].texto, "# Viaje\n- 10 gasolina");
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn un_cambio_de_un_toque_sobre_una_nota_ajena_hace_la_version_con_el_cambio() {
    let (r, ficha) = con_un_chat("nota");
    let original = m(
        "orig",
        Clase::Nota,
        Some("ANA1"),
        "quedamos a las 5",
        None,
        None,
        None,
    );
    let v = vv::al_momento(
        &r,
        &ficha,
        &original,
        std::slice::from_ref(&original),
        Some("MAXX"),
        5_000,
        |mut c| {
            c.texto = "quedamos a las 6".into();
            c.id = "trampa".into();
            c
        },
        "v1",
        "VERSIONUID",
    );
    assert_eq!(v.id, "v1");
    assert_eq!(v.texto, "quedamos a las 6");
    assert_eq!(v.version_de, Some(original.codigo_unico()));
    assert_eq!(v.responde_a.as_deref(), Some("orig"));
    assert!(vv::en_curso(&r).is_empty());
    let _ = std::fs::remove_dir_all(&r);
}

#[test]
fn al_volver_la_version_cambiada_entra_en_el_cuaderno_con_su_numero() {
    let (r, ficha) = con_un_chat("cerrar");
    lienzo(&r, &ficha, "lienzo-ana", "planta", 1);
    let carpeta = almacen::carpeta(&r, &ficha);
    let original = dibujo(Clase::Dibujo, "ANA1", "lienzo-ana");
    cuaderno::anadir(&carpeta, &original).unwrap();
    // Sin nada apuntado no se lee nada.
    assert!(vv::cerrar(&r, 1).unwrap().is_empty());
    let lista = vec![original.clone()];
    vv::empezar(
        &r,
        &ficha,
        &original,
        &lista,
        Some("MAXX"),
        5_000,
        "v1",
        "VERSIONUID",
    )
    .unwrap();
    lienzo(&r, &ficha, "dib-v1", "planta con lo mio", 2);
    let hechas = vv::cerrar(&r, 6_000).unwrap();
    assert_eq!(hechas.len(), 1);
    assert_eq!(hechas[0].0, ficha);
    let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
    assert_eq!(c.mensajes.len(), 2);
    assert_eq!(c.mensajes[1].id, "v1");
    assert_eq!(c.mensajes[1].numero, 4, "uno mas que el mayor");
    assert_eq!(c.mensajes[1].version_n, Some(1));
    // Caso negativo: una segunda vuelta no la anade otra vez.
    assert!(vv::cerrar(&r, 7_000).unwrap().is_empty());
    assert_eq!(
        cuaderno::Cuaderno::leer_de(&carpeta)
            .unwrap()
            .mensajes
            .len(),
        2
    );
    let _ = std::fs::remove_dir_all(&r);
}

// ------------------------------------------------------------------ los rotulos

#[test]
fn los_rotulos_dicen_de_quien_es_el_original_y_cuantas_versiones_tiene() {
    use vv::{Cabecera, DeQuien, EnLaLista};
    let de_ana = m("a", Clase::Dibujo, Some("ANA1"), "", None, Some("x"), None);
    let mio = m("b", Clase::Dibujo, Some("MAXX"), "", None, Some("y"), None);
    let mi_version = vv::nueva(&de_ana, "va", "VERSIONUIA", Some("MAXX"), 2_000, 1);
    let su_version = vv::nueva(&mio, "vb", "VERSIONUIB", Some("ANA1"), 3_000, 1);
    let otra_mia = vv::nueva(&de_ana, "va2", "VERSIONUIC", Some("MAXX"), 4_000, 2);
    let chat = vec![
        de_ana.clone(),
        mio.clone(),
        mi_version.clone(),
        su_version,
        otra_mia,
    ];
    let l = lados();
    let autor = |m: &Mensaje| -> Option<String> {
        l.quien(vv::aparato_corto(m).as_deref())
            .nombre()
            .map(str::to_string)
    };
    let r = vv::rotulos(&chat, true, autor);
    assert_eq!(
        r.cabecera["va"],
        Cabecera {
            original: Some("a".into()),
            n: None,
            de: DeQuien::De("Ana".into())
        }
    );
    assert_eq!(r.cabecera["va2"].n, Some(2));
    assert_eq!(r.cabecera["vb"].de, DeQuien::Tuyo);
    assert_eq!(
        r.versiones["a"],
        vec![
            EnLaLista {
                id: "va".into(),
                n: 1,
                quien: Some(None)
            },
            EnLaLista {
                id: "va2".into(),
                n: 2,
                quien: Some(None)
            },
        ]
    );
    assert_eq!(r.versiones["b"][0].quien, Some(Some("Ana".into())));
    assert!(!r.versiones.contains_key("va"));
    // Sin el original en el chat, el rotulo no lleva a ningun sitio.
    let solo = vv::rotulos(std::slice::from_ref(&mi_version), true, autor);
    assert_eq!(solo.cabecera["va"].original, None);
    assert_eq!(solo.cabecera["va"].de, DeQuien::OtroMensaje);
    // Un chat que ya no es compartido no nombra a nadie, pero el rotulo sale.
    assert_eq!(
        vv::rotulos(&chat, false, autor).cabecera["va"].de,
        DeQuien::SinNombrar
    );
    assert!(vv::rotulos(&[de_ana, mio], true, autor).cabecera.is_empty());
}
