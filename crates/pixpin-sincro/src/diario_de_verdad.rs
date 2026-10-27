//! **El diario de cambios y lo que hace fiable la vuelta, de verdad**
//! (27-oct-2026).
//!
//! Puerto de `DiarioDeCambiosTest.kt` y `VueltaRapidaTest.kt` de Android:
//! dos «aparatos» ([DiscoAndroid]) por un socket de esta maquina, con el
//! protocolo y el cifrado de verdad. Que la segunda vuelta no mire nada, que
//! un cambio solo haga mirar su chat, que sin diario (o con el roto) se haga
//! lo de siempre y no se pierda nada, que lo cortado se reanude, que sin
//! sitio no se descuadre la conversacion, que lo que va primero (tareas,
//! compartidos) llegue aunque se corte, y que con un PixPin de antes todo
//! siga igual.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::PathBuf;

use serde_json::json;

use crate::canonico::Json;
use crate::diario;
use crate::disco::{self, Disco, GENERAL, Marca};
use crate::disco_android::DiscoAndroid;
use crate::disco_android::prueba::{Reloj, crear_grupo, presentar};
use crate::kotlin;
use crate::protocolo::{self, Hecho, Respondedor, Resultado, Sesion};

struct Par {
    dir: PathBuf,
    tel: DiscoAndroid,
    tab: DiscoAndroid,
    reloj: Reloj,
}

impl Drop for Par {
    fn drop(&mut self) {
        diario::poner_activo(true);
        disco::fingir_espacio(&self.tel.raiz(), None);
        disco::fingir_espacio(&self.tab.raiz(), None);
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn montar(etiqueta: &str) -> Par {
    diario::poner_activo(true);
    let dir = std::env::temp_dir().join(format!(
        "pixpin-diario-de-verdad-{etiqueta}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let tel = DiscoAndroid::nuevo(dir.join("telefono/files"), "tel");
    let tab = DiscoAndroid::nuevo(dir.join("tableta/data/files"), "tab");
    presentar(&tel, "id-tel", "Teléfono");
    presentar(&tab, "id-tab", "Tableta");
    Par {
        dir,
        tel,
        tab,
        reloj: Reloj::nuevo(),
    }
}

/// Corta la conexion cuando han salido `tope` bytes por aqui.
struct CortaTras {
    dentro: TcpStream,
    quedan: Option<u64>,
}

impl Read for CortaTras {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.dentro.read(b)
    }
}

impl Write for CortaTras {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if let Some(q) = self.quedan {
            if b.len() as u64 > q {
                let cabe = q as usize;
                if cabe > 0 {
                    let _ = self.dentro.write_all(&b[..cabe]);
                }
                let _ = self.dentro.flush();
                let _ = self.dentro.shutdown(Shutdown::Both);
                self.quedan = Some(0);
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "cortado a propósito",
                ));
            }
            self.quedan = Some(q - b.len() as u64);
        }
        self.dentro.write(b)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.dentro.flush()
    }
}

/// Como se conecta: lo que dice saber cada uno y donde se corta.
#[derive(Clone, Copy)]
struct Como<'a> {
    caps_responde: &'a [&'a str],
    caps_dirige: &'a [&'a str],
    cortar_dirige_tras: Option<u64>,
    cortar_responde_tras: Option<u64>,
}

impl Default for Como<'_> {
    fn default() -> Self {
        Como {
            caps_responde: &crate::CAPACIDADES,
            caps_dirige: &crate::CAPACIDADES,
            cortar_dirige_tras: None,
            cortar_responde_tras: None,
        }
    }
}

fn conectado<T>(
    desde: &DiscoAndroid,
    hacia: &DiscoAndroid,
    unirme: bool,
    codigo: Option<&str>,
    reloj: &Reloj,
    como: Como<'_>,
    uso: impl FnOnce(&mut Sesion<'_, DiscoAndroid, CortaTras>) -> Resultado<T>,
) -> Resultado<T> {
    let escucha = TcpListener::bind(("127.0.0.1", 0))?;
    let puerto = escucha.local_addr()?.port();
    std::thread::scope(|hilos| {
        hilos.spawn(|| {
            if let Ok((flujo, _)) = escucha.accept() {
                let flujo = CortaTras {
                    dentro: flujo,
                    quedan: como.cortar_responde_tras,
                };
                let r = Respondedor {
                    disco: hacia,
                    estado: &|_| {},
                    ahora: &|| reloj.ahora(),
                    mi_puerto: 0,
                    al_saludar: &|_, _| {},
                    suelto: None,
                };
                let _ = r.atender_con(flujo, [7u8; 32], None, como.caps_responde);
            }
        });
        let flujo = CortaTras {
            dentro: TcpStream::connect(("127.0.0.1", puerto))?,
            quedan: como.cortar_dirige_tras,
        };
        let mut s = Sesion::conectar_con(
            flujo,
            desde,
            unirme,
            codigo,
            || reloj.ahora(),
            0,
            [3u8; 32],
            como.caps_dirige,
        )?;
        let r = uso(&mut s);
        // Cortada, sin despedirse: el socket se cierra al soltar la sesion.
        match &r {
            Ok(_) => s.adios(),
            Err(_) => s.soltar(),
        }
        r
    })
}

/// Lo que paso en una vuelta.
struct Vuelta {
    hecho: Hecho,
    bytes: u64,
    enviados: u64,
}

impl Par {
    fn emparejar(&self) {
        crear_grupo(&self.tel, "ABCDE23456", self.reloj.ahora());
        conectado(
            &self.tab,
            &self.tel,
            true,
            Some("ABCDE23456"),
            &self.reloj,
            Como::default(),
            |_| Ok(()),
        )
        .unwrap();
    }

    /// Una vuelta entera («con todos») de `desde` a `hacia`.
    fn vuelta_de(
        &self,
        desde: &DiscoAndroid,
        hacia: &DiscoAndroid,
        como: Como<'_>,
    ) -> Resultado<Vuelta> {
        conectado(desde, hacia, false, None, &self.reloj, como, |s| {
            let mut hecho = Hecho::default();
            crate::vuelta::una(s, None, &mut hecho, "", &|| self.reloj.ahora(), &mut |_| {})?;
            Ok(Vuelta {
                bytes: s.enviados() + s.recibidos(),
                enviados: s.enviados(),
                hecho,
            })
        })
    }

    fn vuelta(&self) -> Vuelta {
        self.vuelta_de(&self.tel, &self.tab, Como::default())
            .unwrap()
    }
}

struct Nuevo<'a> {
    id: &'a str,
    texto: &'a str,
    clase: &'a str,
    proyecto: Option<&'a str>,
    ruta: Option<String>,
    referencia: Option<&'a str>,
    miniapp: Option<&'a str>,
}

fn nota<'a>(id: &'a str, texto: &'a str) -> Nuevo<'a> {
    Nuevo {
        id,
        texto,
        clase: "NOTA",
        proyecto: None,
        ruta: None,
        referencia: None,
        miniapp: None,
    }
}

/// Un mensaje nuevo **como la aplicacion** (`MensajesStore.anadir`): el
/// archivo y su apunte en el diario.
fn mensaje(d: &DiscoAndroid, reloj: &Reloj, n: Nuevo<'_>) {
    let numero = d
        .leer_mensajes()
        .iter()
        .filter(|m| kotlin::cadena(m, "proyecto") == n.proyecto)
        .filter_map(|m| kotlin::numero(m, "numero"))
        .max()
        .unwrap_or(0)
        + 1;
    let letra = d.identidad().unwrap().yo.letra;
    let m = Json::de_valor(&json!({
        "id": n.id, "cuando": reloj.tic(), "clase": n.clase, "texto": n.texto,
        "proyecto": n.proyecto, "ruta": n.ruta, "referencia": n.referencia,
        "miniapp": n.miniapp, "numero": numero, "letra": letra,
    }));
    d.anadir_mensaje(&m).unwrap();
}

/// Cambia el texto de un mensaje **como la aplicacion**.
fn retocar(d: &DiscoAndroid, id: &str, texto: &str) {
    let l: Vec<Json> = d
        .leer_mensajes()
        .into_iter()
        .map(|mut m| {
            if kotlin::cadena(&m, "id") == Some(id) {
                kotlin::poner(&mut m, "texto", Json::cadena(texto));
            }
            m
        })
        .collect();
    d.escribir_mensajes(&l).unwrap();
}

/// Lo mismo **a escondidas** (otro proceso, una version de antes): sin
/// apuntarlo en el diario.
fn retocar_a_escondidas(d: &DiscoAndroid, id: &str, texto: &str) {
    let texto_entero: String = d
        .leer_mensajes()
        .into_iter()
        .map(|mut m| {
            if kotlin::cadena(&m, "id") == Some(id) {
                kotlin::poner(&mut m, "texto", Json::cadena(texto));
            }
            m.a_texto() + "\n"
        })
        .collect();
    std::fs::write(d.raiz().join("guardados.jsonl"), texto_entero).unwrap();
}

fn proyecto(d: &DiscoAndroid, id: &str, nombre: &str) {
    d.guardar_proyecto(&Json::de_valor(
        &json!({"id": id, "nombre": nombre, "tocado": 1}),
    ))
    .unwrap();
}

fn dibujo(d: &DiscoAndroid, id: &str, texto: &str) {
    let f = d.raiz().join(format!("pins/draw/{id}.excalidraw.gz"));
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    let mut e = flate2::write::GzEncoder::new(
        std::fs::File::create(&f).unwrap(),
        flate2::Compression::default(),
    );
    e.write_all(
        format!(r#"{{"elements":[{{"id":"e1","text":"{texto}"}}],"files":{{}}}}"#).as_bytes(),
    )
    .unwrap();
    e.finish().unwrap();
}

fn binario(d: &DiscoAndroid, rel: &str, bytes: usize, semilla: u64) -> Vec<u8> {
    let f = d.raiz().join(rel);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    // Un generador sencillo y fijo: los mismos bytes cada vez.
    let mut x = semilla
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let datos: Vec<u8> = (0..bytes)
        .map(|_| {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (x >> 33) as u8
        })
        .collect();
    std::fs::write(&f, &datos).unwrap();
    datos
}

fn textos(d: &DiscoAndroid) -> Vec<String> {
    d.leer_mensajes()
        .iter()
        .filter_map(|m| kotlin::cadena(m, "texto").map(str::to_string))
        .collect()
}

fn tiene(d: &DiscoAndroid, texto: &str) -> bool {
    textos(d).iter().any(|t| t == texto)
}

fn texto_del_dibujo(d: &DiscoAndroid, id: &str) -> String {
    let b = std::fs::read(d.raiz().join(format!("pins/draw/{id}.excalidraw.gz"))).unwrap();
    let mut s = String::new();
    flate2::read::GzDecoder::new(&b[..])
        .read_to_string(&mut s)
        .unwrap();
    s
}

/// Tres chats con algo en cada uno, sincronizados una vez.
fn tres_chats_al_dia(p: &Par) {
    p.emparejar();
    proyecto(&p.tel, "p1", "Uno");
    proyecto(&p.tel, "p2", "Dos");
    dibujo(&p.tel, "d1", "plano");
    mensaje(&p.tel, &p.reloj, nota("g1", "hola"));
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            proyecto: Some("p1"),
            ..nota("a1", "en uno")
        },
    );
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            proyecto: Some("p2"),
            ..nota("b1", "en dos")
        },
    );
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "DIBUJO",
            proyecto: Some("p1"),
            referencia: Some("d1"),
            ..nota("dib1", "")
        },
    );
    mensaje(&p.tab, &p.reloj, nota("t1", "de la tableta"));
    let v = p.vuelta();
    assert_eq!(v.hecho.quietos, 0);
    assert!(p.tab.raiz().join("pins/draw/d1.excalidraw.gz").is_file());
}

// ------------------------------------------------------------ la vuelta con diario

#[test]
fn la_segunda_vuelta_ni_mira_los_chats_y_no_mueve_nada_desde_los_dos_lados() {
    let p = montar("segunda");
    tres_chats_al_dia(&p);
    let v = p.vuelta();
    assert_eq!(v.hecho.quietos, 3);
    assert_eq!(
        v.hecho.traidos + v.hecho.enviados + v.hecho.fusionados + v.hecho.archivos,
        0
    );
    // Quien respondia tambien: su diario y el cursor que apunto al cerrar.
    let w = p.vuelta_de(&p.tab, &p.tel, Como::default()).unwrap();
    assert_eq!(w.hecho.quietos, 3);
    let mut a = textos(&p.tel);
    let mut b = textos(&p.tab);
    a.sort();
    b.sort();
    assert_eq!(a, b);
}

#[test]
fn un_cambio_de_la_app_solo_hace_mirar_su_chat() {
    let p = montar("un-cambio");
    tres_chats_al_dia(&p);
    retocar(&p.tel, "a1", "en uno, cambiado");
    let v = p.vuelta();
    assert_eq!(v.hecho.quietos, 2, "general y p2 quietos");
    assert!(tiene(&p.tab, "en uno, cambiado"));
    assert_eq!(p.vuelta().hecho.quietos, 3);
}

#[test]
fn lo_escrito_a_escondidas_tambien_llega_y_el_chat_entero_se_mira() {
    let p = montar("a-escondidas");
    tres_chats_al_dia(&p);
    retocar_a_escondidas(&p.tab, "b1", "en dos, por la puerta de atrás");
    let v = p.vuelta();
    assert_eq!(v.hecho.quietos, 0, "sin saber que chat, todos");
    assert!(tiene(&p.tel, "en dos, por la puerta de atrás"));
    assert_eq!(p.vuelta().hecho.quietos, 3);
}

#[test]
fn un_lienzo_reescrito_sin_avisar_se_encuentra_por_el_indice_y_solo_se_mira_su_chat() {
    let p = montar("lienzo");
    tres_chats_al_dia(&p);
    dibujo(&p.tab, "d1", "plano retocado");
    let v = p.vuelta();
    assert_eq!(v.hecho.quietos, 2);
    assert!(texto_del_dibujo(&p.tel, "d1").contains("plano retocado"));
}

#[test]
fn sin_diario_se_hace_lo_de_siempre_y_no_se_pierde_nada() {
    let p = montar("sin-diario");
    tres_chats_al_dia(&p);
    std::fs::remove_dir_all(p.tel.raiz().join("sincro/diario")).unwrap();
    retocar_a_escondidas(&p.tel, "g1", "tras perder el diario");
    let v = p.vuelta();
    assert_eq!(v.hecho.quietos, 0);
    assert!(tiene(&p.tab, "tras perder el diario"));
    assert_eq!(p.vuelta().hecho.quietos, 3);
}

#[test]
fn una_linea_rota_en_el_diario_no_se_fia_y_se_mira_todo() {
    let p = montar("linea-rota");
    tres_chats_al_dia(&p);
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(p.tel.raiz().join("sincro/diario/diario.jsonl"))
        .unwrap();
    f.write_all(b"{\"n\":99,\"k\":").unwrap();
    drop(f);
    assert_eq!(p.vuelta().hecho.quietos, 0, "lo roto es «todo»");
    assert_eq!(p.vuelta().hecho.quietos, 3);
}

#[test]
fn apagado_todo_como_antes_y_con_el_mismo_resultado() {
    let p = montar("apagado");
    tres_chats_al_dia(&p);
    diario::poner_activo(false);
    mensaje(&p.tel, &p.reloj, nota("g3", "sin diario"));
    let v = p.vuelta();
    assert_eq!(v.hecho.quietos, 0);
    assert!(tiene(&p.tab, "sin diario"));
    diario::poner_activo(true);
}

#[test]
fn cada_tantas_vueltas_se_mira_todo_aunque_nada_cambie() {
    let p = montar("verificar");
    tres_chats_al_dia(&p);
    p.tel.diario().cambiar_cursores("id-tab", |mut c| {
        c.vueltas = diario::VUELTAS_POR_VERIFICACION;
        c
    });
    assert_eq!(p.vuelta().hecho.quietos, 0, "la de verificar mira todo");
    assert_eq!(p.vuelta().hecho.quietos, 3);
}

#[test]
fn con_un_pixpin_sin_diario_tambien_se_salta_lo_quieto_por_su_inventario() {
    let p = montar("de-antes");
    p.emparejar();
    proyecto(&p.tel, "p1", "Uno");
    mensaje(&p.tel, &p.reloj, nota("g1", "hola"));
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            proyecto: Some("p1"),
            ..nota("a1", "en uno")
        },
    );
    let de_antes = Como {
        caps_responde: &[],
        ..Como::default()
    };
    let v = p.vuelta_de(&p.tel, &p.tab, de_antes).unwrap();
    assert_eq!(v.hecho.quietos, 0);
    let v = p.vuelta_de(&p.tel, &p.tab, de_antes).unwrap();
    assert_eq!(v.hecho.quietos, 2);
    // Un cambio alli (sin diario) se ve en su inventario y llega.
    retocar_a_escondidas(&p.tab, "a1", "cambiado en el de antes");
    let v = p.vuelta_de(&p.tel, &p.tab, de_antes).unwrap();
    assert_eq!(v.hecho.quietos, 1);
    assert!(tiene(&p.tel, "cambiado en el de antes"));
    // Y al reves, el de antes dirigiendo: todo como siempre.
    let v = p
        .vuelta_de(
            &p.tab,
            &p.tel,
            Como {
                caps_dirige: &[],
                ..Como::default()
            },
        )
        .unwrap();
    assert_eq!(v.hecho.quietos, 0);
}

// ------------------------------------------------------------ fiabilidad

#[test]
fn una_vuelta_cortada_a_mitad_de_un_archivo_grande_sigue_donde_se_quedo_al_mandar() {
    let p = montar("reanudar-mandar");
    p.emparejar();
    let datos = binario(&p.tel, "guardados/obra.mp4", 4 << 20, 1);
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "ARCHIVO",
            ruta: Some(p.tel.absoluta("guardados/obra.mp4")),
            ..nota("v1", "")
        },
    );
    let cortada = p.vuelta_de(
        &p.tel,
        &p.tab,
        Como {
            cortar_dirige_tras: Some((5 << 20) / 2),
            ..Como::default()
        },
    );
    assert!(cortada.is_err(), "tenia que cortarse");
    assert!(!p.tab.raiz().join("guardados/obra.mp4").exists());
    let llegado =
        std::fs::metadata(p.tab.raiz().join("guardados/obra.mp4.parcial")).map_or(0, |m| m.len());
    assert!(llegado >= 1 << 20, "algo llego: {llegado}");
    let v = p.vuelta();
    assert_eq!(
        std::fs::read(p.tab.raiz().join("guardados/obra.mp4")).unwrap(),
        datos
    );
    assert!(
        v.enviados < (4 << 20) - llegado / 2,
        "no se mando entero otra vez: {}",
        v.enviados
    );
    assert!(!p.tab.raiz().join("guardados/obra.mp4.parcial").exists());
}

#[test]
fn una_vuelta_cortada_a_mitad_de_un_archivo_grande_sigue_donde_se_quedo_al_traer() {
    let p = montar("reanudar-traer");
    p.emparejar();
    let datos = binario(&p.tab, "guardados/clase.m4a", 4 << 20, 2);
    mensaje(
        &p.tab,
        &p.reloj,
        Nuevo {
            clase: "VOZ",
            ruta: Some(p.tab.absoluta("guardados/clase.m4a")),
            ..nota("v1", "")
        },
    );
    let cortada = p.vuelta_de(
        &p.tel,
        &p.tab,
        Como {
            cortar_responde_tras: Some((5 << 20) / 2),
            ..Como::default()
        },
    );
    assert!(cortada.is_err(), "tenia que cortarse");
    let llegado =
        std::fs::metadata(p.tel.raiz().join("guardados/clase.m4a.parcial")).map_or(0, |m| m.len());
    assert!(llegado >= 1 << 20, "algo llego: {llegado}");
    let v = p.vuelta();
    assert_eq!(
        std::fs::read(p.tel.raiz().join("guardados/clase.m4a")).unwrap(),
        datos
    );
    assert!(
        v.bytes < (4 << 20) - llegado / 2,
        "solo lo que faltaba: {}",
        v.bytes
    );
    assert!(v.hecho.ahorrados >= llegado as i64);
}

#[test]
fn lo_llegado_a_medias_de_otra_version_no_se_aprovecha_y_sale_entero_y_bien() {
    let p = montar("otra-version");
    p.emparejar();
    binario(&p.tel, "guardados/obra.mp4", 3 << 20, 1);
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "ARCHIVO",
            ruta: Some(p.tel.absoluta("guardados/obra.mp4")),
            ..nota("v1", "")
        },
    );
    let _ = p.vuelta_de(
        &p.tel,
        &p.tab,
        Como {
            cortar_dirige_tras: Some(2 << 20),
            ..Como::default()
        },
    );
    let nueva = binario(&p.tel, "guardados/obra.mp4", 3 << 20, 9);
    p.vuelta();
    assert_eq!(
        std::fs::read(p.tab.raiz().join("guardados/obra.mp4")).unwrap(),
        nueva
    );
}

#[test]
fn sin_sitio_para_recibir_no_se_descuadra_la_conversacion_y_llega_en_cuanto_lo_hay() {
    let p = montar("sin-sitio");
    p.emparejar();
    let foto = binario(&p.tel, "guardados/foto.jpg", 20_000, 3);
    dibujo(&p.tel, "d1", "plano");
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "IMAGEN",
            ruta: Some(p.tel.absoluta("guardados/foto.jpg")),
            ..nota("f1", "")
        },
    );
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "DIBUJO",
            referencia: Some("d1"),
            ..nota("d1m", "")
        },
    );
    mensaje(&p.tel, &p.reloj, nota("n1", "y una nota"));
    disco::fingir_espacio(&p.tab.raiz(), Some(1));
    let v = p.vuelta();
    assert!(tiene(&p.tab, "y una nota"), "los mensajes llegan");
    assert!(
        !v.hecho.saltados.is_empty(),
        "se quedaron sin pasar: {:?}",
        v.hecho.saltados
    );
    assert!(!p.tab.raiz().join("guardados/foto.jpg").exists());
    disco::fingir_espacio(&p.tab.raiz(), None);
    p.vuelta();
    assert_eq!(
        std::fs::read(p.tab.raiz().join("guardados/foto.jpg")).unwrap(),
        foto
    );
    assert!(p.tab.raiz().join("pins/draw/d1.excalidraw.gz").is_file());
    // Y al traer: sin sitio aqui, se dice.
    binario(&p.tab, "guardados/otra.jpg", 20_000, 4);
    mensaje(
        &p.tab,
        &p.reloj,
        Nuevo {
            clase: "IMAGEN",
            ruta: Some(p.tab.absoluta("guardados/otra.jpg")),
            ..nota("f2", "")
        },
    );
    disco::fingir_espacio(&p.tel.raiz(), Some(1));
    let v = p.vuelta();
    assert!(v.hecho.sin_espacio);
    disco::fingir_espacio(&p.tel.raiz(), None);
}

#[test]
fn los_restos_de_una_vuelta_muerta_se_limpian_al_empezar_la_siguiente() {
    let p = montar("restos");
    tres_chats_al_dia(&p);
    let raiz = p.tab.raiz();
    let resto = raiz.join("guardados/x.pdf.sincro");
    std::fs::create_dir_all(resto.parent().unwrap()).unwrap();
    std::fs::write(&resto, "medio").unwrap();
    let viejo = raiz.join("guardados/y.mp4.parcial");
    std::fs::write(&viejo, "viejo").unwrap();
    let hace_ocho_dias =
        std::time::SystemTime::now() - std::time::Duration::from_secs(8 * 24 * 3600);
    std::fs::File::options()
        .write(true)
        .open(&viejo)
        .unwrap()
        .set_modified(hace_ocho_dias)
        .unwrap();
    let nuevo = raiz.join("guardados/z.mp4.parcial");
    std::fs::write(&nuevo, "nuevo").unwrap();
    p.vuelta();
    assert!(!resto.exists());
    assert!(!viejo.exists());
    assert!(nuevo.exists(), "lo reciente se guarda para reanudar");
}

#[test]
fn dos_a_la_vez_el_segundo_oye_ocupado_nada_se_rompe_y_luego_va() {
    let p = montar("dos-a-la-vez");
    tres_chats_al_dia(&p);
    assert!(protocolo::ocupar(&p.tab));
    mensaje(&p.tel, &p.reloj, nota("g9", "mientras tanto"));
    let r = p.vuelta_de(&p.tel, &p.tab, Como::default());
    protocolo::soltar(&p.tab);
    match r {
        Err(e) => assert_eq!(e.to_string(), crate::mensajes::OCUPADO),
        Ok(_) => panic!("tenia que estar ocupado"),
    }
    assert!(protocolo::ocupar(&p.tel), "el que dirigia quedo libre");
    protocolo::soltar(&p.tel);
    p.vuelta();
    assert!(tiene(&p.tab, "mientras tanto"));
}

#[test]
fn borrado_en_uno_y_cambiado_en_el_otro_se_queda_lo_cambiado_en_los_dos() {
    let p = montar("borrado-cambiado");
    tres_chats_al_dia(&p);
    let antes = p.tel.leer_mensajes();
    let quitado: Vec<Json> = antes
        .iter()
        .filter(|m| kotlin::cadena(m, "id") == Some("b1"))
        .cloned()
        .collect();
    let quedan: Vec<Json> = antes
        .iter()
        .filter(|m| kotlin::cadena(m, "id") != Some("b1"))
        .cloned()
        .collect();
    p.tel.escribir_mensajes(&quedan).unwrap();
    let marcas: Vec<Marca> = disco::marcas_de("p2", &quitado, p.reloj.tic());
    p.tel.anotar_borrados(&marcas).unwrap();
    p.reloj.saltar(10);
    retocar(&p.tab, "b1", "en dos, retocado");
    p.vuelta();
    assert!(tiene(&p.tel, "en dos, retocado"));
    assert!(tiene(&p.tab, "en dos, retocado"));
}

#[test]
fn dos_vueltas_seguidas_sin_cambios_no_mueven_nada_y_es_idempotente() {
    let p = montar("idempotente");
    tres_chats_al_dia(&p);
    let primero = std::fs::read_to_string(p.tab.raiz().join("guardados.jsonl")).unwrap();
    for _ in 0..2 {
        let h = p.vuelta().hecho;
        assert_eq!(
            h.traidos + h.enviados + h.borrados + h.archivos + h.fusionados,
            0
        );
    }
    assert_eq!(
        std::fs::read_to_string(p.tab.raiz().join("guardados.jsonl")).unwrap(),
        primero
    );
}

// ------------------------------------------------------------ prioridades

#[test]
fn cortada_a_mitad_del_video_las_tareas_y_el_chat_compartido_ya_llegaron() {
    let p = montar("prioridad-corte");
    p.emparejar();
    proyecto(&p.tel, "p-tareas", "Obra");
    proyecto(&p.tel, "p-compartido", "Con Ana");
    crate::compartidos::CompartidosEnDisco::nuevo(&p.tel.raiz())
        .cambiar(|_| -> io::Result<_> {
            let persona = |id: &str, nombre: &str| crate::compartidos::Persona {
                id: id.into(),
                nombre: nombre.into(),
                codigo: String::new(),
                clave: String::new(),
                desde: 10,
                invito: "id-tel".into(),
                firma: String::new(),
                fuera: 0,
                quito: None,
                firma_fuera: None,
            };
            Ok(vec![crate::compartidos::ChatCompartido {
                chat: "p-compartido".into(),
                nombre: "Con Ana".into(),
                creador: "id-tel".into(),
                creado: 10,
                personas: vec![persona("id-tel", "Yo"), persona("id-ana", "Ana")],
            }])
        })
        .unwrap();
    binario(&p.tel, "guardados/clase.mp4", 4 << 20, 5);
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "ARCHIVO",
            ruta: Some(p.tel.absoluta("guardados/clase.mp4")),
            ..nota("vid", "")
        },
    );
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "MINIAPP",
            proyecto: Some("p-tareas"),
            miniapp: Some("tareas"),
            ..nota("tar", "- [ ] comprar cemento")
        },
    );
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            proyecto: Some("p-compartido"),
            ..nota("ana", "para Ana")
        },
    );
    let r = p.vuelta_de(
        &p.tel,
        &p.tab,
        Como {
            cortar_dirige_tras: Some(2 << 20),
            ..Como::default()
        },
    );
    assert!(r.is_err(), "tenia que cortarse");
    assert!(tiene(&p.tab, "- [ ] comprar cemento"), "las tareas");
    assert!(tiene(&p.tab, "para Ana"), "el compartido");
    assert!(!p.tab.raiz().join("guardados/clase.mp4").exists());
}

#[test]
fn los_archivos_van_por_niveles_y_lo_pesado_al_final() {
    let p = montar("niveles");
    p.emparejar();
    binario(&p.tel, "guardados/a.mp4", 300_000, 1);
    binario(&p.tel, "guardados/b.pdf", 200_000, 2);
    dibujo(&p.tel, "c", "lienzo");
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "ARCHIVO",
            ruta: Some(p.tel.absoluta("guardados/a.mp4")),
            ..nota("v", "")
        },
    );
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "ARCHIVO",
            ruta: Some(p.tel.absoluta("guardados/b.pdf")),
            ..nota("p", "")
        },
    );
    mensaje(
        &p.tel,
        &p.reloj,
        Nuevo {
            clase: "DIBUJO",
            referencia: Some("c"),
            ..nota("l", "")
        },
    );
    let orden = conectado(
        &p.tel,
        &p.tab,
        false,
        None,
        &p.reloj,
        Como::default(),
        |s| {
            let mut hecho = Hecho::default();
            let prep = s.preparar(GENERAL)?;
            s.aplicar(&prep, &mut hecho)?;
            let arch = s.preparar_archivos(&prep)?;
            let orden: Vec<String> = arch
                .en_orden()
                .iter()
                .map(|x| x.sena().to_string())
                .collect();
            s.aplicar_archivos(&arch, &mut hecho, &mut |_| {})?;
            s.cerrar(&prep, p.reloj.ahora())?;
            Ok(orden)
        },
    )
    .unwrap();
    assert_eq!(
        orden,
        [
            "guardados/b.pdf",
            "pins/draw/c.excalidraw.gz",
            "guardados/a.mp4"
        ]
    );
    for rel in &orden {
        assert!(p.tab.raiz().join(rel).is_file(), "{rel}");
    }
}

// ------------------------------------------------------------ lo que gana

/// `VueltaRapidaTest`: 30 chats × 120 mensajes + 6 lienzos + 6 archivos de
/// 64 KB. Sin cambios, y con un mensaje y un lienzo cambiados en dos chats,
/// con el diario y sin el. Lo que cuenta es la proporcion (la maquina de las
/// pruebas esta cargada con otras cosas).
#[test]
fn la_vuelta_con_diario_es_mucho_mas_rapida_y_mueve_mucho_menos() {
    let p = montar("rapida");
    p.emparejar();
    let mut todos = Vec::new();
    for c in 0..30 {
        let pr = format!("p{c:02}");
        proyecto(&p.tel, &pr, &format!("Chat {c}"));
        for i in 0..120 {
            let id = format!("{pr}-m{i}");
            let texto = format!("mensaje {i} del chat {c}");
            let m = Json::de_valor(&json!({
                "id": id, "cuando": p.reloj.tic(), "clase": "NOTA", "texto": texto,
                "proyecto": pr, "numero": i + 1, "letra": "a",
            }));
            todos.push(kotlin::normalizar_mensaje(&m).unwrap());
        }
        if c % 5 == 0 {
            let d = format!("d{c}");
            dibujo(&p.tel, &d, "lienzo");
            let m = Json::de_valor(&json!({
                "id": format!("{pr}-dib"), "cuando": p.reloj.tic(), "clase": "DIBUJO",
                "referencia": d, "proyecto": pr, "numero": 121, "letra": "a",
            }));
            todos.push(kotlin::normalizar_mensaje(&m).unwrap());
            let rel = format!("guardados/arch{c}.pdf");
            binario(&p.tel, &rel, 64 * 1024, c as u64);
            let m = Json::de_valor(&json!({
                "id": format!("{pr}-arch"), "cuando": p.reloj.tic(), "clase": "ARCHIVO",
                "ruta": p.tel.absoluta(&rel), "proyecto": pr, "numero": 122, "letra": "a",
            }));
            todos.push(kotlin::normalizar_mensaje(&m).unwrap());
        }
    }
    let mut actuales = p.tel.leer_mensajes();
    actuales.extend(todos);
    p.tel.escribir_mensajes(&actuales).unwrap();
    p.vuelta();
    let medir = |activo: bool| {
        diario::poner_activo(activo);
        let t = std::time::Instant::now();
        let v = p.vuelta();
        (t.elapsed().as_millis(), v.bytes, v.hecho.quietos)
    };
    // Sin cambios.
    let (ms_sin, bytes_sin, _) = medir(false);
    let (ms_con, bytes_con, quietos) = medir(true);
    assert!(quietos >= 30, "quietos: {quietos}");
    // Un mensaje y un lienzo cambiados en dos chats.
    let cambiar = |n: u32| {
        retocar(&p.tel, "p03-m7", &format!("cambiado {n}"));
        dibujo(&p.tel, "d10", &format!("lienzo cambiado {n}"));
    };
    cambiar(1);
    let (ms_sin2, bytes_sin2, _) = medir(false);
    cambiar(2);
    let (ms_con2, bytes_con2, quietos2) = medir(true);
    diario::poner_activo(true);
    eprintln!(
        "vuelta sin cambios: sin diario {ms_sin} ms · {bytes_sin} bytes; con diario {ms_con} ms · {bytes_con} bytes"
    );
    eprintln!(
        "un mensaje y un lienzo en dos chats: sin diario {ms_sin2} ms · {bytes_sin2} bytes; con diario {ms_con2} ms · {bytes_con2} bytes ({quietos2} quietos)"
    );
    assert!(bytes_con * 20 < bytes_sin, "{bytes_con} contra {bytes_sin}");
    assert!(
        bytes_con2 * 3 < bytes_sin2,
        "{bytes_con2} contra {bytes_sin2}"
    );
    assert!(
        quietos2 >= 29,
        "solo se miran los dos chats tocados: {quietos2}"
    );
    assert!(texto_del_dibujo(&p.tab, "d10").contains("lienzo cambiado 2"));
}
