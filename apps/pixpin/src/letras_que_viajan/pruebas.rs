//! Las letras con un movil de mentira (`DiscoAndroid`, el puerto de
//! `Disco.kt`) por el protocolo de verdad: ida y vuelta, sin re-mandar,
//! cambio, borrado en los dos sentidos y los casos negativos (chat
//! compartido, fichero que no es una letra). Letras de prueba: las de
//! Hershey de PixPin (libres) y cabeceras TrueType sinteticas.

use pixpin_proyecto::vista::{self, DiscoPc};
use pixpin_sincro::canonico::Json;
use pixpin_sincro::disco::Disco;
use pixpin_sincro::disco_android::DiscoAndroid;
use pixpin_sincro::disco_android::prueba::{Reloj, conectado, crear_grupo, presentar, sincronizar};
use pixpin_sincro::kotlin;
use serde_json::json;

use super::*;

const ROMANS: &[u8] = include_bytes!("../../../../crates/pixpin-cad/fuentes-shx/romans.shx");
const ROMAND: &[u8] = include_bytes!("../../../../crates/pixpin-cad/fuentes-shx/romand.shx");
const GREEKS: &[u8] = include_bytes!("../../../../crates/pixpin-cad/fuentes-shx/greeks.shx");

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
    let dir = std::env::temp_dir().join(format!("pixpin-letras-viajan-{etiqueta}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pc = DiscoPc::nuevo(&dir.join("PixPin Max"));
    let movil = DiscoAndroid::nuevo(dir.join("movil/files"), "movil");
    presentar(&pc, "id-pc", "Portátil");
    presentar(&movil, "id-movil", "Teléfono");
    let p = Par { dir, pc, movil, reloj: Reloj::nuevo() };
    crear_grupo(&p.movil, "ABCDE23456", p.reloj.ahora());
    conectado(&p.pc, &p.movil, true, Some("ABCDE23456"), &p.reloj, |_| {}).unwrap();
    p
}

impl Par {
    fn raiz(&self) -> PathBuf {
        self.pc.raiz()
    }
    /// El chat «Asistente» como lo ve la sincronizacion.
    fn chat(&self) -> String {
        let f = almacen::asegurar_asistente(&self.raiz(), 1, "PC01").unwrap();
        vista::chat_de_ficha(&self.raiz(), &f.id).unwrap()
    }
    fn desde_pc(&self) -> pixpin_sincro::protocolo::Hecho {
        sincronizar(&self.pc, &self.movil, &[&self.chat()], &self.reloj, &|| {}).0
    }
    fn desde_movil(&self) -> pixpin_sincro::protocolo::Hecho {
        sincronizar(&self.movil, &self.pc, &[&self.chat()], &self.reloj, &|| {}).0
    }
    fn en_el_movil(&self, nombre: &str) -> PathBuf {
        self.movil.raiz().join("guardados/fuentes-shx").join(nombre)
    }
    fn en_el_pc(&self, nombre: &str) -> PathBuf {
        carpeta_en(&self.raiz()).join(nombre)
    }
    fn ids_en_el_movil(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .movil
            .leer_mensajes()
            .iter()
            .filter_map(|m| kotlin::cadena(m, "id").map(str::to_string))
            .filter(|i| i.starts_with(PREFIJO_ID))
            .collect();
        v.sort();
        v
    }
    /// El movil anade una letra como lo haria su «Añadir letras…».
    fn letra_del_movil(&self, nombre: &str, bytes: &[u8]) {
        let rel = format!("guardados/fuentes-shx/{nombre}");
        let f = self.movil.raiz().join(&rel);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, bytes).unwrap();
        let m = Json::de_valor(&json!({
            "id": id_de(nombre), "cuando": self.reloj.tic(), "clase": "ARCHIVO", "ruta": self.movil.absoluta(&rel),
            "nombre": nombre, "bytes": bytes.len(), "numero": self.movil.leer_mensajes().len() + 1,
            "proyecto": self.chat(), "aparato": "MOV1",
        }));
        self.movil.anadir_mensaje(&m).unwrap();
    }
    /// Lo que hace el chat del movil al borrar un mensaje: fuera, su marca y su fichero.
    fn movil_quita(&self, nombre: &str) {
        let (ido, quedan): (Vec<Json>, Vec<Json>) =
            self.movil.leer_mensajes().into_iter().partition(|m| kotlin::cadena(m, "id") == Some(id_de(nombre).as_str()));
        self.movil.escribir_mensajes(&quedan).unwrap();
        self.movil.anotar_borrados(&pixpin_sincro::disco::marcas_de(&self.chat(), &ido, self.reloj.tic())).unwrap();
        let _ = std::fs::remove_file(self.en_el_movil(nombre));
    }
}

/// Unas letras fuera de la carpeta (las que elige el usuario).
fn elegidas(p: &Par) -> Vec<PathBuf> {
    let d = p.dir.join("descargas");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("romans.shx"), ROMANS).unwrap();
    // Una TrueType sintetica: solo su cabecera.
    std::fs::write(d.join("Libre.ttf"), [0u8, 1, 0, 0, 0, 4, 0, 0]).unwrap();
    // Caso negativo: un programa rebautizado.
    std::fs::write(d.join("trampa.shx"), b"MZ\x90\0esto no es una letra").unwrap();
    vec![d.join("romans.shx"), d.join("Libre.ttf"), d.join("trampa.shx")]
}

/// **Ida y vuelta**: el PC anade (lo que no es letra no entra), llega al
/// movil donde el lo lee; el movil anade la suya y llega a la carpeta del
/// PC (la huella cambia: el visor relee). Otra vuelta no manda nada; una
/// letra cambiada viaja sin mensaje nuevo.
#[test]
fn las_letras_van_al_movil_y_vuelven_sin_re_mandar() {
    let p = montar("idavuelta");
    let raiz = p.raiz();
    assert_eq!(anadir(&raiz, "PC01", 5, &elegidas(&p)), 2, "la trampa no entra");
    assert!(!p.en_el_pc("trampa.shx").exists());
    let h = p.desde_pc();
    assert!(h.archivos >= 2, "{h:?}");
    assert_eq!(std::fs::read(p.en_el_movil("romans.shx")).unwrap(), ROMANS);
    assert!(p.en_el_movil("Libre.ttf").is_file());
    assert_eq!(p.ids_en_el_movil(), vec![id_de("libre.ttf"), id_de("romans.shx")]);
    let m = p.movil.leer_mensajes().into_iter().find(|m| kotlin::cadena(m, "id") == Some(id_de("romans.shx").as_str())).unwrap();
    assert_eq!(kotlin::cadena(&m, "ruta").unwrap(), p.movil.absoluta("guardados/fuentes-shx/romans.shx"));

    // La del movil llega a la carpeta del usuario del PC.
    let antes = pixpin_cad::letras::huella(&carpeta_en(&raiz));
    p.letra_del_movil("greeks.shx", GREEKS);
    p.desde_movil();
    assert_eq!(std::fs::read(p.en_el_pc("greeks.shx")).unwrap(), GREEKS);
    // La letra que llega no es «lo ultimo» de la lista de chats (es de sistema).
    assert!(!asistente(&raiz).unwrap().resumen.contains("greeks"));
    assert_ne!(pixpin_cad::letras::huella(&carpeta_en(&raiz)), antes, "el visor abierto relee");

    // Otra vuelta: nada que mandar.
    let h = p.desde_pc();
    assert_eq!(h.archivos, 0, "sin re-mandar: {h:?}");
    // Volver a senalar no duplica.
    assert_eq!(senalar(&raiz, "PC01", 9).unwrap(), 0);

    // Cambiada en el PC (mismo nombre): viaja el fichero, no un mensaje nuevo.
    let d = p.dir.join("otra");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("romans.shx"), ROMAND).unwrap();
    assert_eq!(anadir(&raiz, "PC01", 10, &[d.join("romans.shx")]), 1);
    let h = p.desde_pc();
    assert_eq!(h.archivos, 1, "{h:?}");
    assert_eq!(std::fs::read(p.en_el_movil("romans.shx")).unwrap(), ROMAND);
    assert_eq!(p.ids_en_el_movil().len(), 3);
}

/// **Borrado en los dos sentidos**: el movil quita una y se va del PC;
/// «Quitar las mias» en el PC se lleva todas del movil.
#[test]
fn quitar_una_letra_la_quita_en_el_otro_aparato() {
    let p = montar("borrar");
    let raiz = p.raiz();
    anadir(&raiz, "PC01", 5, &elegidas(&p));
    p.letra_del_movil("greeks.shx", GREEKS);
    p.desde_movil();
    p.desde_pc();
    assert!(p.en_el_pc("greeks.shx").is_file() && p.en_el_movil("romans.shx").is_file());

    p.movil_quita("romans.shx");
    p.desde_movil();
    assert!(!p.en_el_pc("romans.shx").exists(), "se fue con su mensaje");
    assert!(p.en_el_pc("greeks.shx").is_file() && p.en_el_pc("Libre.ttf").is_file(), "las otras siguen");

    assert_eq!(quitar_todas(&raiz, p.reloj.tic()), 2);
    p.desde_pc();
    assert!(p.ids_en_el_movil().is_empty());
    assert!(!p.en_el_movil("greeks.shx").exists() && !p.en_el_movil("Libre.ttf").exists());
    // Y no vuelven: otra vuelta desde el movil no las resucita.
    p.desde_movil();
    assert!(pixpin_cad::letras::letras_en(&carpeta_en(&raiz)).is_empty());
}

/// **Casos negativos**: las letras solo estan en el chat «Asistente»; para
/// una persona son ficheros vedados; una ruta de letras en un chat
/// compartido va a la carpeta de ese chat, no a la de letras; y lo que
/// llega con nombre de letra pero no lo es se rechaza en los dos aparatos.
#[test]
fn ni_un_chat_compartido_ni_un_fichero_que_no_es_letra() {
    let p = montar("negativo");
    let raiz = p.raiz();
    anadir(&raiz, "PC01", 5, &elegidas(&p));
    // Otro chat (uno que se podria compartir) no tiene nada de letras.
    let obra = almacen::Ficha::nueva("Obra", 6, "PC01");
    let mut ind = Indice::leer(&raiz);
    ind.proyectos.push(obra.clone());
    ind.guardar(&raiz).unwrap();
    assert!(Cuaderno::leer_de(&almacen::carpeta(&raiz, &obra.id)).map(|c| c.mensajes.is_empty()).unwrap_or(true));
    // Para una persona sin chats en comun, el fichero de la letra esta vedado.
    let acceso = pixpin_sincro::compartidos::AccesoDePersona::nuevo(&p.pc, "id-pc", "id-persona");
    assert!(acceso.prohibidos().contains(&p.en_el_pc("romans.shx")));
    assert!(!acceso.puede_escribir(&p.chat(), "guardados/fuentes-shx/romans.shx"));
    // La misma ruta en otro chat lleva a su carpeta: inerte.
    let chat_obra = vista::chat_de_ficha(&raiz, &obra.id).unwrap();
    let ajena = p.pc.ruta(&chat_obra, "guardados/fuentes-shx/romans.shx");
    assert!(!ajena.starts_with(carpeta_en(&raiz)), "{ajena:?}");
    assert_eq!(p.pc.ruta(&p.chat(), "guardados/fuentes-shx/romans.shx"), p.en_el_pc("romans.shx"));

    // Del movil llega una «letra» que no lo es: se rechaza al abrir un plano
    // (fichero y mensaje), y el rechazo vuelve al movil.
    p.letra_del_movil("falsa.shx", b"MZ\x90\0programa");
    p.letra_del_movil("leeme.txt", b"hola");
    p.desde_movil();
    assert!(p.en_el_pc("falsa.shx").is_file());
    assert!(limpiar(&raiz, p.reloj.tic()) >= 2);
    assert!(!p.en_el_pc("falsa.shx").exists());
    assert!(p.en_el_pc("romans.shx").is_file(), "las buenas siguen");
    p.desde_pc();
    assert!(!p.en_el_movil("falsa.shx").exists() && !p.en_el_movil("leeme.txt").exists());
    assert_eq!(p.ids_en_el_movil(), vec![id_de("libre.ttf"), id_de("romans.shx")]);
}

#[test]
fn los_topes_y_los_nombres_que_no_viajan_se_quedan_en_este_equipo() {
    let raiz = std::env::temp_dir().join(format!("pixpin-letras-topes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    let c = carpeta_en(&raiz);
    std::fs::create_dir_all(&c).unwrap();
    let mut grande = vec![0u8; (TOPE_POR_LETRA + 1) as usize];
    grande[..4].copy_from_slice(&[0, 1, 0, 0]);
    std::fs::write(c.join("enorme.ttf"), &grande).unwrap();
    std::fs::write(c.join("rara (1).shx"), ROMANS).unwrap();
    std::fs::write(c.join("romans.shx"), ROMANS).unwrap();
    assert_eq!(senalar(&raiz, "PC01", 5).unwrap(), 1);
    let f = asistente(&raiz).unwrap();
    let ids: Vec<String> = Cuaderno::leer_de(&almacen::carpeta(&raiz, &f.id)).unwrap().mensajes.into_iter().map(|m| m.id).collect();
    assert_eq!(ids, vec![id_de("romans.shx")]);
    // Siguen en la carpeta: aqui se usan igual.
    assert_eq!(pixpin_cad::letras::letras_en(&c).len(), 3);
    // Sin letras, ni se crea el chat «Asistente».
    let vacia = raiz.join("otra");
    assert_eq!(senalar(&vacia, "PC01", 6).unwrap(), 0);
    assert!(asistente(&vacia).is_none());
    let _ = std::fs::remove_dir_all(&raiz);
}
