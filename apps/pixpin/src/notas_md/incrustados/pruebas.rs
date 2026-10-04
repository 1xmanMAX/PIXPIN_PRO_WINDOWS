//! Los incrustados con un proyecto de verdad en una carpeta temporal: su
//! cuaderno con una nota de voz transcrita, un PDF, un texto y una foto.

use super::*;
use pixpin_notas::incrustados::Medios;

struct Carpeta(PathBuf);
impl Drop for Carpeta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn carpeta(nombre: &str) -> Carpeta {
    let d = std::env::temp_dir().join(format!(
        "pixpin-incrustados-{nombre}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    Carpeta(d)
}

fn textos() -> Catalogo {
    Catalogo::nuevo(pixpin_store::Idioma::Espanol)
}

const VOZ: &str = "UIDVOZ0001";
const PDF: &str = "UIDPDF0001";
const TEXTO: &str = "UIDTEXTO01";
const LETRA: &str = "[0:00] llamar al aparejador\n\n[0:21] y pedir el presupuesto";

fn mensaje(id: &str, uid: &str, cuando: i64, numero: i64) -> Mensaje {
    Mensaje {
        id: id.into(),
        uid: Some(uid.into()),
        // Una fecha de verdad (con 1970 la hora local saldria negativa).
        cuando: 1_790_000_000_000 + cuando,
        numero,
        aparato: Some("K7Q2".into()),
        ..Mensaje::default()
    }
}

/// «Casa Lima» con una nota de voz transcrita, un PDF y un texto en su
/// chat, y sus ficheros en `archivos/`.
fn proyecto(raiz: &Path) -> FichaProyecto {
    let ficha = FichaProyecto::nueva("Casa Lima", 0, "PC");
    Indice {
        proyectos: vec![ficha.clone()],
        ..Indice::default()
    }
    .guardar(raiz)
    .unwrap();
    let c = almacen::carpeta(raiz, &ficha.id);
    std::fs::create_dir_all(c.join("archivos")).unwrap();
    std::fs::write(c.join("archivos").join("voz-1.m4a"), b"m4a").unwrap();
    std::fs::write(c.join("archivos").join("Plano.pdf"), b"%PDF-1.4").unwrap();
    let mensajes = vec![
        Mensaje {
            clase: Some(Clase::Voz),
            ruta: Some("archivos/voz-1.m4a".into()),
            nombre: "Aparejador.m4a".into(),
            duracion_ms: 65_000,
            transcripcion: Some(LETRA.into()),
            ..mensaje("m-voz", VOZ, 1_000, 45)
        },
        Mensaje {
            clase: Some(Clase::Archivo),
            ruta: Some("archivos/Plano.pdf".into()),
            nombre: "Plano.pdf".into(),
            bytes: 8,
            ..mensaje("m-pdf", PDF, 2_000, 47)
        },
        Mensaje {
            clase: Some(Clase::Nota),
            texto: "Pedir [la grua] para el **jueves**\nsegunda linea".into(),
            ..mensaje("m-texto", TEXTO, 3_000, 48)
        },
        Mensaje {
            clase: Some(Clase::Nota),
            texto: "temporal".into(),
            en_buzon: true,
            ..mensaje("m-buzon", "UIDBUZON01", 4_000, 49)
        },
    ];
    let lineas: Vec<String> = mensajes
        .iter()
        .map(|m| serde_json::to_string(m).unwrap())
        .collect();
    std::fs::write(c.join("guardados.jsonl"), lineas.join("\n") + "\n").unwrap();
    ficha
}

fn mensaje_de(raiz: &Path, f: &FichaProyecto, uid: &str) -> Mensaje {
    paginas_vivas::mensajes_de(raiz, f)
        .into_iter()
        .find(|m| m.uid.as_deref() == Some(uid))
        .unwrap()
}

fn medios(raiz: &Path, f: &FichaProyecto) -> MediosDeLaNota {
    let actual = Rc::new(RefCell::new(Destino::Nueva {
        proyecto: f.id.clone(),
    }));
    MediosDeLaNota::nuevo(
        pixpin_store::Idioma::Espanol,
        &Ubicacion::Portable {
            raiz: raiz.to_path_buf(),
        },
        &actual,
    )
}

#[test]
fn una_nota_de_voz_del_chat_entra_como_su_audio_con_su_transcripcion_y_sin_copiarse() {
    let d = carpeta("voz");
    let f = proyecto(&d.0);
    let m = mensaje_de(&d.0, &f, VOZ);
    let b = bloque_de(&d.0, &f, &m, &textos());
    // El audio del chat (no una copia) y su transcripcion debajo, como la
    // deja el movil al transcribir.
    let l = inc::letras(&b);
    assert_eq!(l.len(), 1, "{b}");
    assert!(l[0].ruta.ends_with("archivos/voz-1.m4a"), "{b}");
    assert_eq!(
        l[0].parrafos.iter().map(|p| p.1).collect::<Vec<_>>(),
        vec![0, 21_000]
    );
    assert!(b.starts_with("![Aparejador]("), "{b}");
    assert!(
        !almacen::carpeta(&d.0, &f.id).join("notas").exists(),
        "no se copia nada"
    );
    // Y la nota lo encuentra: su reproductor sabe cuanto dura.
    let mut x = medios(&d.0, &f);
    assert_eq!(
        x.ficha(&l[0].ruta),
        Some(Ficha::Audio(Audio {
            nombre: "voz-1.m4a".into(),
            duracion_ms: 65_000,
            falta: false,
        }))
    );
}

#[test]
fn un_archivo_del_chat_se_enlaza_a_su_mensaje_y_su_burbuja_lleva_hora_chapa_y_tamano() {
    let d = carpeta("pdf");
    let f = proyecto(&d.0);
    let m = mensaje_de(&d.0, &f, PDF);
    let b = bloque_de(&d.0, &f, &m, &textos());
    let codigo_proyecto = paginas_vivas::codigo_de_proyecto(&f);
    assert_eq!(
        b,
        format!("[Plano.pdf](pixpin:mensaje={codigo_proyecto}/{PDF})")
    );
    let mut x = medios(&d.0, &f);
    let Some(Ficha::Burbuja(burbuja)) = x.ficha(&format!("pixpin:mensaje={codigo_proyecto}/{PDF}"))
    else {
        panic!("sin burbuja");
    };
    assert_eq!(burbuja.codigo.as_deref(), Some("#47·K7Q2"));
    assert!(!burbuja.hora.is_empty());
    assert_eq!(
        burbuja.contenido,
        Contenido::Archivo(Archivo {
            nombre: "Plano.pdf".into(),
            detalle: "8 B · PDF".into(),
            falta: false,
        })
    );
}

#[test]
fn un_texto_del_chat_se_enlaza_con_su_primera_frase_y_sin_marcas() {
    let d = carpeta("texto");
    let f = proyecto(&d.0);
    let m = mensaje_de(&d.0, &f, TEXTO);
    let b = bloque_de(&d.0, &f, &m, &textos());
    assert!(b.starts_with("[Pedir la grua para el jueves]("), "{b}");
    assert_eq!(inc::de_renglon(&b).map(|x| x.0), Some(inc::Clase::Mensaje));
}

#[test]
fn un_mensaje_borrado_y_un_documento_que_no_esta_se_dicen() {
    let d = carpeta("borrado");
    let f = proyecto(&d.0);
    let mut x = medios(&d.0, &f);
    let codigo_proyecto = paginas_vivas::codigo_de_proyecto(&f);
    assert_eq!(
        x.ficha(&format!("pixpin:mensaje={codigo_proyecto}/NOESTA0001")),
        Some(Ficha::Borrado(String::new()))
    );
    assert_eq!(
        x.ficha("pixpin:mensaje=PROYECTOXX/NOESTA0001"),
        Some(Ficha::Borrado(String::new()))
    );
    match x.ficha("notas/9-Presupuesto.pdf") {
        Some(Ficha::Archivo(a)) => assert!(a.falta && a.nombre == "Presupuesto.pdf", "{a:?}"),
        otra => panic!("{otra:?}"),
    }
    // Caso negativo: un documento que si esta no falta.
    let copia = almacen::carpeta(&d.0, &f.id).join("notas");
    std::fs::create_dir_all(&copia).unwrap();
    std::fs::write(copia.join("9-Presupuesto.pdf"), b"%PDF").unwrap();
    match x.ficha("notas/9-Presupuesto.pdf") {
        Some(Ficha::Archivo(a)) => assert!(!a.falta && a.detalle == "4 B · PDF", "{a:?}"),
        otra => panic!("{otra:?}"),
    }
}

#[test]
fn abrir_un_mensaje_va_a_su_chat_y_un_documento_a_su_fichero() {
    let d = carpeta("abrir");
    let f = proyecto(&d.0);
    let nota = Destino::Nueva {
        proyecto: f.id.clone(),
    };
    let codigo_proyecto = paginas_vivas::codigo_de_proyecto(&f);
    assert_eq!(
        que_abre(
            &d.0,
            &nota,
            &format!("pixpin:mensaje={codigo_proyecto}/{PDF}")
        ),
        Apertura::Mensaje {
            proyecto: f.id.clone(),
            codigo: PDF.into(),
        }
    );
    assert_eq!(
        que_abre(&d.0, &nota, "archivos/Plano.pdf"),
        Apertura::Fichero(
            almacen::carpeta(&d.0, &f.id)
                .join("archivos")
                .join("Plano.pdf")
        )
    );
    // Lo que no es suyo (una foto, una hoja) lo abre quien abre las fotos;
    // lo que ya no esta, nada.
    assert_eq!(que_abre(&d.0, &nota, "notas/1-obra.png"), Apertura::NoEsMio);
    assert_eq!(que_abre(&d.0, &nota, "pixpin:hoja=P/H"), Apertura::NoEsMio);
    assert_eq!(que_abre(&d.0, &nota, "notas/no-esta.pdf"), Apertura::Nada);
    assert_eq!(
        que_abre(&d.0, &nota, "pixpin:mensaje=NOEXISTE/M"),
        Apertura::Nada
    );
}

#[test]
fn el_menu_del_chat_va_del_mas_nuevo_al_mas_viejo_sin_el_buzon_ni_la_propia_nota() {
    let d = carpeta("menu");
    let f = proyecto(&d.0);
    let actual = Rc::new(RefCell::new(Destino::Mensaje {
        proyecto: f.id.clone(),
        codigo: TEXTO.into(),
    }));
    let mut x = MediosDeLaNota::nuevo(
        pixpin_store::Idioma::Espanol,
        &Ubicacion::Portable { raiz: d.0.clone() },
        &actual,
    );
    let v = x.mensajes();
    assert_eq!(
        v.iter().map(|m| m.clave.as_str()).collect::<Vec<_>>(),
        vec![PDF, VOZ]
    );
    assert!(v[0].rotulo.contains("#47·K7Q2"), "{}", v[0].rotulo);
    // Lo elegido se escribe como su bloque.
    let b = x.insertar_mensaje(PDF).unwrap();
    assert!(b.contains(&format!("/{PDF})")), "{b}");
    assert_eq!(x.insertar_mensaje("NOESTA0001"), None);
    // Una nota suelta no tiene chat.
    let suelta = Rc::new(RefCell::new(Destino::Fichero {
        ruta: d.0.join("a.md"),
    }));
    let mut y = MediosDeLaNota::nuevo(
        pixpin_store::Idioma::Espanol,
        &Ubicacion::Portable { raiz: d.0.clone() },
        &suelta,
    );
    assert!(y.mensajes().is_empty());
}

#[test]
fn todo_mensaje_del_chat_se_puede_insertar_en_una_nota_menos_el_del_buzon() {
    let d = carpeta("insertar");
    let f = proyecto(&d.0);
    for uid in [VOZ, PDF, TEXTO] {
        assert!(
            paginas_vivas::se_puede_insertar(&mensaje_de(&d.0, &f, uid)),
            "{uid}"
        );
    }
    // Caso negativo: lo del buzon se va solo, y un mensaje sin id no es nada.
    let buzon = Mensaje {
        en_buzon: true,
        ..mensaje("m-buzon", "UIDBUZON01", 4_000, 49)
    };
    assert!(!paginas_vivas::se_puede_insertar(&buzon));
    assert!(!se_puede_enlazar(&Mensaje::default()));
}
