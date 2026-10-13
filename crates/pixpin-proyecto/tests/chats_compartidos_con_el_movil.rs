//! **Un chat compartido entre este PC y el movil de otra persona**, de
//! verdad (Android v0.122.0, 13-oct-2026).
//!
//! El PC es su almacen de siempre visto por `vista::DiscoPc`; «Ana» es un
//! movil de mentira (`DiscoAndroid`, la carpeta `files` de Android). No
//! estan en el mismo grupo. El PC comparte un proyecto, abre la puerta de la
//! invitacion, Ana entra con el codigo y se trae ese chat —y nada mas—, y
//! luego se sincronizan en los dos sentidos por el protocolo y el cifrado de
//! verdad (`PXI1` y `PXC1`). Lo que se mira aqui y no en `pixpin-sincro` es
//! la valla **del PC**: cada proyecto en su carpeta, y rutas como
//! `guardados/pc/<otro proyecto>/…` que saltarian a la de otro.

use std::collections::BTreeSet;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;

use pixpin_proyecto::almacen::{self, Ficha, Indice};
use pixpin_proyecto::cuaderno::{self, Clase, Cuaderno, Mensaje, Sello};
use pixpin_proyecto::vista::DiscoPc;
use pixpin_sincro::canal::{Canal, MAGIA_DE_PERSONA, Tipo};
use pixpin_sincro::canonico::Json;
use pixpin_sincro::compartidos::{self, CompartidosEnDisco};
use pixpin_sincro::disco::Disco;
use pixpin_sincro::disco_android::DiscoAndroid;
use pixpin_sincro::disco_android::prueba::{Reloj, presentar};
use pixpin_sincro::invitacion::{self, ConPersona, Puerta};
use pixpin_sincro::kotlin;
use pixpin_sincro::mensajes::{Aparato, Hola, Peticion, Respuesta};
use pixpin_sincro::protocolo::{Respondedor, Resultado};
use serde_json::json;

const CODIGO: &str = "QRST2345";

fn reloj_de_la_puerta() -> i64 {
    9_000_000
}

struct Dos {
    dir: PathBuf,
    pc: DiscoPc,
    ana: DiscoAndroid,
    reloj: Reloj,
    tesis: Ficha,
    privado: Ficha,
}

impl Drop for Dos {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn aparato_pc() -> String {
    pixpin_proyecto::codigos::de_aparato("id-pc")
}

fn proyecto(raiz: &std::path::Path, reloj: &Reloj, nombre: &str, nota: &str, adjunto: &[u8]) -> Ficha {
    let ficha = Ficha::nueva(nombre, reloj.tic(), &aparato_pc());
    let mut i = Indice::leer(raiz);
    i.proyectos.push(ficha.clone());
    i.guardar(raiz).unwrap();
    let carpeta = almacen::carpeta(raiz, &ficha.id);
    let sello = |numero| Sello {
        cuando: reloj.tic(),
        numero,
        aparato: aparato_pc(),
        proyecto: ficha.id.clone(),
    };
    cuaderno::anadir(&carpeta, &Mensaje::nota(nota, &sello(1))).unwrap();
    let ruta = almacen::guardar_adjunto(raiz, &ficha.id, "doc.pdf", adjunto).unwrap();
    cuaderno::anadir(
        &carpeta,
        &Mensaje::adjunto(Clase::Archivo, "doc.pdf", &ruta, adjunto.len() as i64, &sello(2)),
    )
    .unwrap();
    ficha
}

fn montar(etiqueta: &str) -> Dos {
    let dir = std::env::temp_dir().join(format!(
        "pixpin-pc-compartido-{etiqueta}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let pc = DiscoPc::nuevo(&dir.join("PixPin Max"));
    let ana = DiscoAndroid::nuevo(dir.join("ana/files"), "ana");
    presentar(&pc, "id-pc", "Portátil");
    presentar(&ana, "id-ana", "Móvil de Ana");
    let reloj = Reloj::nuevo();
    let raiz = pc.raiz();
    let tesis = proyecto(&raiz, &reloj, "Tesis", "hola equipo", b"%PDF tesis");
    let privado = proyecto(&raiz, &reloj, "Privado", "secreto del privado", b"%PDF privado");
    let c = CompartidosEnDisco::nuevo(&raiz);
    c.poner_nombre("Max").unwrap();
    CompartidosEnDisco::nuevo(&ana.raiz()).poner_nombre("Ana").unwrap();
    let yo = c.yo_de(&pc).unwrap();
    c.poner(&compartidos::crear(&tesis.id, "Tesis", &yo, reloj.tic()))
        .unwrap();
    Dos {
        dir,
        pc,
        ana,
        reloj,
        tesis,
        privado,
    }
}

impl Dos {
    fn textos_pc(&self, ficha: &Ficha) -> BTreeSet<String> {
        Cuaderno::leer_de(&almacen::carpeta(&self.pc.raiz(), &ficha.id))
            .map(|c| c.mensajes.into_iter().map(|m| m.texto).collect())
            .unwrap_or_default()
    }

    fn textos_ana(&self, chat: &str) -> BTreeSet<String> {
        self.ana
            .leer_mensajes()
            .iter()
            .filter(|m| kotlin::chat_de(m) == chat)
            .map(|m| kotlin::cadena(m, "texto").unwrap_or_default().to_string())
            .collect()
    }

    /// Las rutas portatiles de los archivos de un proyecto del PC.
    fn rutas(&self, ficha: &Ficha) -> Vec<String> {
        self.pc
            .alcance(&ficha.id)
            .unwrap()
            .into_iter()
            .map(|(r, _)| r)
            .collect()
    }

    fn nota_ana(&self, id: &str, texto: &str, ruta: Option<String>) {
        let numero = self
            .ana
            .leer_mensajes()
            .iter()
            .filter(|m| kotlin::cadena(m, "proyecto") == Some(self.tesis.id.as_str()))
            .filter_map(|m| kotlin::numero(m, "numero"))
            .max()
            .unwrap_or(0)
            + 1;
        let clase = if ruta.is_some() { "ARCHIVO" } else { "NOTA" };
        self.ana
            .anadir_mensaje(&Json::de_valor(&json!({
                "id": id, "cuando": self.reloj.tic(), "clase": clase, "texto": texto,
                "proyecto": self.tesis.id, "numero": numero, "ruta": ruta,
                "aparato": pixpin_sincro::grupo::codigo_de_aparato("id-ana"),
            })))
            .unwrap();
    }
}

/// Ana entra con el codigo por la puerta del PC y se trae el chat.
fn ana_entra(d: &Dos) -> ConPersona {
    let puerta = Puerta::abrir_con(
        Arc::new(DiscoPc::nuevo(&d.pc.raiz())),
        &d.tesis.id,
        CODIGO,
        ("127.0.0.1", 0),
        0,
        reloj_de_la_puerta,
        |_| {},
    )
    .unwrap();
    let f = TcpStream::connect(("127.0.0.1", puerta.puerto)).unwrap();
    let e = invitacion::entrar(f, &d.ana, CODIGO, d.reloj.ahora(), [1u8; 32]).unwrap();
    assert_eq!(e.chat.chat, d.tesis.id);
    assert_eq!(e.quien.id, "id-pc");
    let f = TcpStream::connect(("127.0.0.1", puerta.puerto)).unwrap();
    invitacion::vuelta_con_persona(
        f,
        &d.ana,
        Some("id-pc"),
        None,
        &|| d.reloj.ahora(),
        0,
        [2u8; 32],
        &mut |_| {},
    )
    .unwrap()
}

fn atender<D: Disco>(d: &D, reloj: &Reloj, flujo: TcpStream) -> Resultado<()> {
    let r = Respondedor {
        disco: d,
        estado: &|_| {},
        ahora: &|| reloj.ahora(),
        mi_puerto: 0,
        al_saludar: &|_, _| {},
        suelto: None,
    };
    let mut magia = [0u8; 4];
    let _ = flujo.peek(&mut magia);
    if &magia == MAGIA_DE_PERSONA {
        r.atender_persona(flujo, [9u8; 32])
    } else {
        r.atender(flujo, [9u8; 32])
    }
}

fn con_escucha<D: Disco + Sync, T>(hacia: &D, reloj: &Reloj, uso: impl FnOnce(TcpStream) -> T) -> T {
    let escucha = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let puerto = escucha.local_addr().unwrap().port();
    std::thread::scope(|h| {
        h.spawn(|| {
            if let Ok((f, _)) = escucha.accept() {
                let _ = atender(hacia, reloj, f);
            }
        });
        uso(TcpStream::connect(("127.0.0.1", puerto)).unwrap())
    })
}

fn vuelta<A: Disco + Sync, B: Disco + Sync>(desde: &A, hacia: &B, reloj: &Reloj) -> ConPersona {
    con_escucha(hacia, reloj, |f| {
        invitacion::vuelta_con_persona(f, desde, None, None, &|| reloj.ahora(), 0, [3u8; 32], &mut |_| {})
            .unwrap()
    })
}

#[test]
fn el_movil_de_otra_persona_se_trae_el_proyecto_compartido_del_pc_y_nada_mas() {
    let d = montar("trae");
    let r = ana_entra(&d);
    assert_eq!(r.chats, [d.tesis.id.clone()]);
    assert!(d.textos_ana(&d.tesis.id).contains("hola equipo"));
    assert!(d.textos_ana(&d.privado.id).is_empty());
    // El adjunto del proyecto compartido llego; el del otro, no.
    for rel in d.rutas(&d.tesis) {
        assert!(d.ana.raiz().join(&rel).is_file(), "no llegó {rel}");
    }
    for rel in d.rutas(&d.privado) {
        assert!(!d.ana.raiz().join(&rel).is_file(), "se coló {rel}");
    }
    // Ana no entro en ningun grupo.
    assert!(d.ana.identidad().unwrap().codigo.is_none());
}

#[test]
fn lo_que_escribe_la_otra_persona_llega_al_pc_con_su_nombre_y_al_reves() {
    let d = montar("dos-sentidos");
    ana_entra(&d);
    d.nota_ana("a1", "he revisado la planta", None);
    // Esta vez llama el PC al movil de Ana.
    vuelta(&d.pc, &d.ana, &d.reloj);
    assert!(d.textos_pc(&d.tesis).contains("he revisado la planta"));
    let autores = compartidos::autores(
        &CompartidosEnDisco::nuevo(&d.pc.raiz())
            .de(&d.tesis.id)
            .unwrap(),
    );
    let codigo_ana = pixpin_sincro::grupo::codigo_de_aparato("id-ana");
    assert_eq!(autores.get(&codigo_ana).map(String::as_str), Some("Ana"));
    assert_eq!(autores.get(&aparato_pc()).map(String::as_str), Some("Max"));
    // Y lo del PC llega a Ana llamando ella.
    let carpeta = almacen::carpeta(&d.pc.raiz(), &d.tesis.id);
    cuaderno::anadir(
        &carpeta,
        &Mensaje::nota(
            "gracias",
            &Sello {
                cuando: d.reloj.tic(),
                numero: 9,
                aparato: aparato_pc(),
                proyecto: d.tesis.id.clone(),
            },
        ),
    )
    .unwrap();
    vuelta(&d.ana, &d.pc, &d.reloj);
    assert!(d.textos_ana(&d.tesis.id).contains("gracias"));
    assert!(d.textos_ana(&d.privado.id).is_empty());
    assert!(!d.textos_pc(&d.tesis).contains("secreto del privado"));
}

#[test]
fn ni_pidiendo_a_mano_ni_con_un_mensaje_trampa_sale_un_archivo_de_otro_proyecto_del_pc() {
    let d = montar("valla-pc");
    ana_entra(&d);
    let privadas = d.rutas(&d.privado);
    assert!(!privadas.is_empty());
    // Un mensaje trampa en el chat compartido que senala el PDF del otro
    // proyecto por su ruta del PC.
    let ruta_privada = privadas
        .iter()
        .find(|r| r.ends_with(".pdf"))
        .unwrap()
        .clone();
    d.nota_ana("trampa", "", Some(format!("pixpin:files/{ruta_privada}")));
    d.nota_ana("a1", "esto sí", None);
    vuelta(&d.ana, &d.pc, &d.reloj);
    vuelta(&d.pc, &d.ana, &d.reloj);
    assert!(d.textos_pc(&d.tesis).contains("esto sí"));
    assert!(!d.ana.raiz().join(&ruta_privada).is_file());
    // Y a mano, por el canal de persona, tampoco.
    con_escucha(&d.pc, &d.reloj, |f| {
        let llaves = CompartidosEnDisco::nuevo(&d.ana.raiz()).llaves().unwrap();
        let (mut c, _) = Canal::entre_personas(f, &llaves, [4u8; 32], true, |_| true).unwrap();
        let pedir = |c: &mut Canal<TcpStream>, p: &Peticion| -> Respuesta {
            c.mandar(Tipo::Json, &serde_json::to_vec(p).unwrap()).unwrap();
            serde_json::from_slice(&c.recibir().unwrap().1).unwrap()
        };
        let hola = Peticion {
            hola: Some(Hola {
                yo: Aparato {
                    id: "id-ana".into(),
                    nombre: "Ana".into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Peticion::de("hola")
        };
        assert!(pedir(&mut c, &hola).error.is_none());
        let catalogo = pedir(&mut c, &Peticion::de("catalogo"));
        assert_eq!(
            catalogo.chats.iter().map(|x| x.id.clone()).collect::<Vec<_>>(),
            [d.tesis.id.clone()]
        );
        for rel in &privadas {
            for chat in [&d.tesis.id, &d.privado.id] {
                let r = pedir(
                    &mut c,
                    &Peticion {
                        ruta: Some(rel.clone()),
                        ..Peticion::sobre("dame", chat)
                    },
                );
                assert!(r.error.is_some(), "se dio {rel} por {chat}");
            }
        }
        let r = pedir(&mut c, &Peticion::sobre("inventario", &d.privado.id));
        assert!(r.error.is_some());
        assert!(pedir(&mut c, &Peticion::de("adios")).error.is_none());
    });
    // Lo del PC sigue en su sitio.
    for rel in &privadas {
        assert!(d.pc.ruta(&d.privado.id, rel).is_file());
    }
    assert!(d.textos_pc(&d.privado).contains("secreto del privado"));
}
