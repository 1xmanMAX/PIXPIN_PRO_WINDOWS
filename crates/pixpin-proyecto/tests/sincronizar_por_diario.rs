//! **El PC sincronizando por diario con el movil** (27-oct-2026).
//!
//! El «movil» es `DiscoAndroid` con la `Sesion` y el `Respondedor` de
//! verdad, que ya hablan `diario1` y `reanudar1` como el Android de hoy
//! (commit `261b67a5`); con `&[]` es un Android (o un PC) de antes, que no
//! dice ninguna capacidad. El PC es su almacen de siempre visto por
//! `vista::DiscoPc`. Lo que se prueba es lo de la guia de Android para el
//! PC (§7.2, §7.3 y §8): que lo quieto ni se mira en los dos sentidos, que
//! con uno de antes todo va como siempre (y aun asi se salta lo quieto por
//! la huella de su inventario), y que con cien chats sin cambios una vuelta
//! **no lee ni un cuaderno** del PC.

use std::path::PathBuf;

use pixpin_proyecto::almacen::{self, Ficha, Indice};
use pixpin_proyecto::cuaderno::{self, Cuaderno, Mensaje, Sello};
use pixpin_proyecto::vista::{self, DiscoPc};
use pixpin_sincro::canonico::Json;
use pixpin_sincro::disco::{Disco, GENERAL};
use pixpin_sincro::disco_android::DiscoAndroid;
use pixpin_sincro::disco_android::prueba::{Reloj, conectado_con, crear_grupo, presentar};
use pixpin_sincro::kotlin;
use pixpin_sincro::protocolo::Hecho;
use serde_json::json;

const NUEVO: &[&str] = &pixpin_sincro::CAPACIDADES;
const DE_ANTES: &[&str] = &[];

struct Par {
    dir: PathBuf,
    pc: DiscoPc,
    movil: DiscoAndroid,
    reloj: Reloj,
}

impl Drop for Par {
    fn drop(&mut self) {
        pixpin_sincro::diario::poner_activo(true);
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn montar(etiqueta: &str) -> Par {
    pixpin_sincro::diario::poner_activo(true);
    let dir = std::env::temp_dir().join(format!(
        "pixpin-pc-diario-{etiqueta}-{}",
        std::process::id()
    ));
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
    crear_grupo(&p.movil, "ABCDE23456", p.reloj.ahora());
    conectado_con(
        &p.pc,
        &p.movil,
        true,
        Some("ABCDE23456"),
        &p.reloj,
        NUEVO,
        NUEVO,
        |_| {},
    )
    .unwrap();
    p
}

impl Par {
    fn raiz(&self) -> PathBuf {
        self.pc.raiz()
    }

    /// Una vuelta entera desde el PC, «con todos».
    fn desde_pc(&self, caps_pc: &[&str], caps_movil: &[&str]) -> Hecho {
        let mut hecho = Hecho::default();
        conectado_con(
            &self.pc,
            &self.movil,
            false,
            None,
            &self.reloj,
            caps_pc,
            caps_movil,
            |s| {
                pixpin_sincro::vuelta::una(
                    s,
                    None,
                    &mut hecho,
                    "",
                    &|| self.reloj.ahora(),
                    &mut |_| {},
                )
                .unwrap()
            },
        )
        .unwrap();
        hecho
    }

    /// Una vuelta entera desde el movil, con el PC respondiendo.
    fn desde_movil(&self, caps_movil: &[&str], caps_pc: &[&str]) -> Hecho {
        let mut hecho = Hecho::default();
        conectado_con(
            &self.movil,
            &self.pc,
            false,
            None,
            &self.reloj,
            caps_movil,
            caps_pc,
            |s| {
                pixpin_sincro::vuelta::una(
                    s,
                    None,
                    &mut hecho,
                    "",
                    &|| self.reloj.ahora(),
                    &mut |_| {},
                )
                .unwrap()
            },
        )
        .unwrap();
        hecho
    }

    fn aparato_pc(&self) -> String {
        pixpin_proyecto::codigos::de_aparato("id-pc")
    }

    /// Un proyecto nacido en el PC con `notas` notas, como los deja su chat.
    fn proyecto_pc(&self, nombre: &str, notas: usize) -> Ficha {
        let raiz = self.raiz();
        let ficha = Ficha::nueva(nombre, self.reloj.tic(), &self.aparato_pc());
        let mut i = Indice::leer(&raiz);
        i.proyectos.push(ficha.clone());
        i.guardar(&raiz).unwrap();
        for n in 0..notas {
            self.nota_pc(&ficha, &format!("{nombre}: nota {n}"));
        }
        ficha
    }

    /// Una nota escrita en el PC, como la escribe su chat (`cuaderno::anadir`,
    /// que la apunta en el diario).
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

    /// Cambia el texto de una nota del movil **a escondidas** (un PixPin de
    /// antes, sin diario).
    fn retocar_movil_a_escondidas(&self, id: &str, texto: &str) {
        let todo: String = self
            .movil
            .leer_mensajes()
            .into_iter()
            .map(|mut m| {
                if kotlin::cadena(&m, "id") == Some(id) {
                    kotlin::poner(&mut m, "texto", Json::cadena(texto));
                }
                m.a_texto() + "\n"
            })
            .collect();
        std::fs::write(self.movil.raiz().join("guardados.jsonl"), todo).unwrap();
    }

    fn textos_movil(&self) -> Vec<String> {
        self.movil
            .leer_mensajes()
            .iter()
            .filter_map(|m| kotlin::cadena(m, "texto").map(str::to_string))
            .collect()
    }

    fn textos_pc(&self, ficha_id: &str) -> Vec<String> {
        Cuaderno::leer_de(&almacen::carpeta(&self.raiz(), ficha_id))
            .map(|c| c.mensajes.into_iter().map(|m| m.texto).collect())
            .unwrap_or_default()
    }

    /// El chat del PC de una ficha.
    fn chat(&self, f: &Ficha) -> String {
        vista::chat_de_ficha(&self.raiz(), &f.id).unwrap()
    }
}

/// Dos proyectos del PC y uno del movil, sincronizados una vez.
fn tres_y_la_general(p: &Par) -> (Ficha, Ficha) {
    let a = p.proyecto_pc("Obra", 3);
    let b = p.proyecto_pc("Casa", 2);
    p.movil
        .guardar_proyecto(&Json::de_valor(
            &json!({"id": "pr-m", "nombre": "Del movil", "tocado": 1}),
        ))
        .unwrap();
    p.nota_movil("m1", "del movil", Some("pr-m"));
    p.nota_movil("g1", "en la general", None);
    let h = p.desde_pc(NUEVO, NUEVO);
    assert_eq!(h.quietos, 0);
    assert!(p.textos_movil().iter().any(|t| t == "Obra: nota 0"));
    (a, b)
}

#[test]
fn el_pc_y_el_android_nuevo_se_saltan_lo_quieto_en_los_dos_sentidos() {
    let p = montar("nuevo-nuevo");
    let (a, _) = tres_y_la_general(&p);
    // §7.2: con los dos con `diario1`, la vuelta sin cambios es «novedades».
    let h = p.desde_pc(NUEVO, NUEVO);
    assert_eq!(h.quietos, 4, "general, Obra, Casa y el del movil");
    assert_eq!(h.traidos + h.enviados + h.archivos + h.fusionados, 0);
    let h = p.desde_movil(NUEVO, NUEVO);
    assert_eq!(h.quietos, 4, "y dirigiendo el movil, el PC responde igual");
    // Una nota en el PC solo hace mirar su chat, y llega.
    p.nota_pc(&a, "Obra: una mas");
    let h = p.desde_movil(NUEVO, NUEVO);
    assert_eq!(h.quietos, 3);
    assert!(p.textos_movil().iter().any(|t| t == "Obra: una mas"));
    // Y una del movil, lo mismo hacia el PC.
    p.nota_movil("m2", "otra del movil", Some("pr-m"));
    let h = p.desde_pc(NUEVO, NUEVO);
    assert_eq!(h.quietos, 3);
    let ficha_m = Indice::leer(&p.raiz())
        .proyectos
        .into_iter()
        .find(|f| f.nombre == "Del movil")
        .unwrap();
    assert!(
        p.textos_pc(&ficha_m.id)
            .iter()
            .any(|t| t == "otra del movil")
    );
    assert_eq!(p.desde_pc(NUEVO, NUEVO).quietos, 4);
    assert_eq!(p.desde_movil(NUEVO, NUEVO).quietos, 4);
}

#[test]
fn el_pc_nuevo_con_un_android_de_antes_va_como_siempre_y_se_salta_lo_quieto_por_su_inventario() {
    let p = montar("nuevo-antes");
    let a = p.proyecto_pc("Obra", 2);
    p.nota_movil("g1", "en la general", None);
    // §7.3: el de antes no dice capacidades; la primera vuelta lo mira todo.
    let h = p.desde_pc(NUEVO, DE_ANTES);
    assert_eq!(h.quietos, 0);
    // La siguiente, por la huella de su inventario y el sello del cursor.
    let h = p.desde_pc(NUEVO, DE_ANTES);
    assert_eq!(h.quietos, 2, "general y Obra, sin «novedades»");
    assert_eq!(h.traidos + h.enviados + h.archivos, 0);
    // Lo que cambio alli sin diario se ve en su inventario y llega.
    p.retocar_movil_a_escondidas("g1", "cambiado en el de antes");
    let h = p.desde_pc(NUEVO, DE_ANTES);
    assert_eq!(h.quietos, 1);
    let general = Indice::leer(&p.raiz())
        .proyectos
        .into_iter()
        .find(|f| f.es_guardados())
        .unwrap();
    assert!(
        p.textos_pc(&general.id)
            .iter()
            .any(|t| t == "cambiado en el de antes")
    );
    // El de antes dirigiendo: el PC le responde como siempre.
    p.nota_pc(&a, "Obra: para el de antes");
    let h = p.desde_movil(DE_ANTES, NUEVO);
    assert_eq!(h.quietos, 0, "el de antes no sabe de diarios");
    assert!(
        p.textos_movil()
            .iter()
            .any(|t| t == "Obra: para el de antes")
    );
    // Y un PC de antes con el Android nuevo, en los dos sentidos.
    p.nota_movil("g2", "para el PC de antes", None);
    let h = p.desde_movil(NUEVO, DE_ANTES);
    assert!(
        p.textos_pc(&general.id)
            .iter()
            .any(|t| t == "para el PC de antes")
    );
    assert_eq!(h.borrados, 0);
    let h = p.desde_pc(DE_ANTES, NUEVO);
    assert_eq!(h.quietos, 0);
    assert_eq!(h.traidos + h.enviados, 0, "ya estaban iguales");
}

#[test]
fn con_cien_chats_sin_cambios_una_vuelta_no_lee_ni_un_cuaderno_del_pc() {
    let p = montar("cien");
    let mut fichas = Vec::new();
    for i in 0..110 {
        fichas.push(p.proyecto_pc(&format!("Chat {i:03}"), 3));
    }
    // Lo escrito para preparar la prueba es de hace un rato: si no, el barrido
    // tendria que mirar el CRC de lo recien escrito (lo «racily clean»).
    let hace_un_rato = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
    for f in &fichas {
        let c = almacen::carpeta(&p.raiz(), &f.id).join("guardados.jsonl");
        std::fs::File::options()
            .write(true)
            .open(&c)
            .unwrap()
            .set_modified(hace_un_rato)
            .unwrap();
    }
    let t = std::time::Instant::now();
    let h = p.desde_pc(NUEVO, NUEVO);
    let primera = t.elapsed();
    assert_eq!(h.quietos, 0);
    assert!(h.enviados >= 330, "enviados: {}", h.enviados);
    // Sin cambios, dirigiendo el PC.
    let antes = vista::lecturas_de_cuadernos();
    let t = std::time::Instant::now();
    let h = p.desde_pc(NUEVO, NUEVO);
    let con_diario = t.elapsed();
    let leidos = vista::lecturas_de_cuadernos() - antes;
    assert_eq!(h.quietos, 111, "los 110 y la general");
    assert_eq!(leidos, 0, "ni un cuaderno leido dirigiendo el PC");
    // Y respondiendo el PC.
    let antes = vista::lecturas_de_cuadernos();
    let h = p.desde_movil(NUEVO, NUEVO);
    assert_eq!(h.quietos, 111);
    assert_eq!(
        vista::lecturas_de_cuadernos() - antes,
        0,
        "ni uno respondiendo"
    );
    // Lo mismo sin diario, para comparar.
    pixpin_sincro::diario::poner_activo(false);
    let antes = vista::lecturas_de_cuadernos();
    let t = std::time::Instant::now();
    let h = p.desde_pc(NUEVO, NUEVO);
    let sin_diario = t.elapsed();
    let leidos_sin = vista::lecturas_de_cuadernos() - antes;
    pixpin_sincro::diario::poner_activo(true);
    assert_eq!(h.quietos, 0);
    assert!(leidos_sin >= 110, "sin diario se leen todos: {leidos_sin}");
    eprintln!(
        "110 chats del PC: primera vuelta {} ms; sin cambios con diario {} ms (0 cuadernos), sin diario {} ms ({leidos_sin} lecturas de cuadernos)",
        primera.as_millis(),
        con_diario.as_millis(),
        sin_diario.as_millis()
    );
    // Uno cambiado: solo se lee lo suyo y llega.
    p.nota_pc(&fichas[7], "Chat 007: cambiada");
    let h = p.desde_pc(NUEVO, NUEVO);
    assert_eq!(h.quietos, 110);
    assert!(p.textos_movil().iter().any(|t| t == "Chat 007: cambiada"));
    let _ = p.chat(&fichas[7]);
    let _ = GENERAL;
}
