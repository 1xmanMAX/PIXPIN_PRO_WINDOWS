//! **Cambiar el nombre desde el lector** (D5): F2, o un clic en la pastilla
//! del nombre, y se escribe el nuevo ahi mismo.
//!
//! Es la pastilla del nombre de `VisorHtmlActivity.kt` del movil: al tocar
//! el nombre sale un campo con el nombre **sin la extension** (hasta 80
//! letras, sin saltos de linea), Intro lo guarda **con la extension de
//! antes** (`conSuExtension`: si el nuevo no la lleva se le pone, si ya la
//! lleva no se repite) y un nombre vacio o igual no cambia nada. Alli el
//! nombre que cambia es **el del mensaje del chat** del que salio el
//! documento, no el del fichero, y aqui igual: el fichero del proyecto es
//! compartido con el movil por la sincronizacion (su `ruta` viaja en el
//! mensaje), y moverlo por debajo lo dejaria huerfano en el otro aparato.
//! El chat, que tiene el cuaderno en memoria, se entera con
//! `ventana_chat::refrescar` y lo relee.
//!
//! Lo que el movil no tiene es el documento **suelto**, abierto desde el
//! Explorador («Abrir con»): ese no es de ningun mensaje y lo unico que se
//! puede renombrar es el fichero. Se renombra en el disco, y con el **sus
//! hermanos** (`<nombre>.pixpin-lectura`, la carpeta
//! `<nombre>.pixpin-anotado` con la tinta...), que cuelgan de su nombre: sin
//! ellos, lo anotado y el sitio por donde se iba se quedarian atras.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use pixpin_proyecto::cuaderno::{Cuaderno, Mensaje};

/// Cuantas letras admite el nombre, como el campo del movil.
const LARGO_MAXIMO: usize = 80;

/// El nombre sin su extension: lo que se ensena para editar.
pub fn sin_extension(nombre: &str) -> String {
    pixpin_docs::sin_extension(nombre)
}

/// **El nombre nuevo**, a partir de lo escrito y del de antes, o `None` si
/// no hay que cambiar nada (vacio, o igual que antes).
///
/// Lleva la extension de antes (`conSuExtension` del movil) y se limpia de
/// lo que Windows no admite en un nombre de fichero: aunque en el chat solo
/// cambie el mensaje, ese nombre acaba siendo el del fichero al compartirlo
/// o guardarlo.
pub fn nombre_nuevo(escrito: &str, antes: &str) -> Option<String> {
    let limpio: String = escrito
        .chars()
        .filter(|c| *c != '\n' && *c != '\r')
        .take(LARGO_MAXIMO)
        .map(|c| {
            if "\\/:*?\"<>|".contains(c) || c < ' ' {
                '-'
            } else {
                c
            }
        })
        .collect();
    // Windows no admite un nombre que acabe en punto o en espacio.
    let limpio = limpio.trim().trim_end_matches(['.', ' ']).trim();
    if limpio.is_empty() {
        return None;
    }
    let base = limpio
        .split('.')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_uppercase();
    let reservado = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base.as_bytes()[3].is_ascii_digit());
    if reservado {
        return None;
    }
    let nuevo = con_su_extension(limpio, antes);
    (nuevo != antes).then_some(nuevo)
}

/// `conSuExtension` del movil: la extension de antes, si la tenia y es de
/// las de verdad (cinco letras o menos), y si lo escrito no la lleva ya.
fn con_su_extension(nuevo: &str, antes: &str) -> String {
    let ext = match antes.rsplit_once('.') {
        Some((_, e)) => e,
        None => "",
    };
    if ext.is_empty() || ext.chars().count() > 5 {
        return nuevo.to_string();
    }
    if nuevo
        .to_lowercase()
        .ends_with(&format!(".{}", ext.to_lowercase()))
    {
        nuevo.to_string()
    } else {
        format!("{nuevo}.{ext}")
    }
}

/// De que es el documento que se esta leyendo.
// Uno por documento abierto; meter el mensaje en Box no gana nada.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Default)]
pub enum Dueno {
    /// Un mensaje del chat: `carpeta` es la del proyecto, donde esta su
    /// cuaderno.
    Chat { carpeta: PathBuf, mensaje: Mensaje },
    /// Un fichero suelto, fuera del almacen.
    #[default]
    Suelto,
    /// Esta dentro del almacen pero no es de ningun mensaje que se sepa (un
    /// adjunto del movil a medio llegar, un resto): ni se toca.
    DelAlmacen,
}

/// Dos rutas que son el mismo fichero, sin mirar mayusculas ni barras (en
/// Windows `C:\a\B.pdf` y `c:/a/b.pdf` son el mismo).
fn misma_ruta(a: &Path, b: &Path) -> bool {
    let norma = |p: &Path| p.to_string_lossy().replace('/', "\\").to_lowercase();
    norma(a) == norma(b)
}

fn esta_dentro(ruta: &Path, carpeta: &Path) -> bool {
    let r = ruta.to_string_lossy().replace('/', "\\").to_lowercase();
    let mut c = carpeta.to_string_lossy().replace('/', "\\").to_lowercase();
    if !c.ends_with('\\') {
        c.push('\\');
    }
    r.starts_with(&c)
}

/// **De quien es este documento.** Primero se mira si esta dentro de la
/// carpeta de un proyecto (subiendo hasta encontrar su `guardados.jsonl`:
/// asi se reconoce tambien un proyecto guardado en otra carpeta y abierto
/// desde el Explorador); si no, si lo senala algun mensaje del almacen (lo
/// que llego del movil vive aparte, en `pixpin:files`).
pub fn dueno(raiz: &Path, ruta: &Path) -> Dueno {
    for carpeta in ruta.ancestors().skip(1).take(4) {
        if carpeta.join("guardados.jsonl").is_file() {
            let Ok(c) = Cuaderno::leer_de(carpeta) else {
                return Dueno::DelAlmacen;
            };
            let rel = ruta
                .strip_prefix(carpeta)
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            return match c.mensajes.into_iter().find(|m| {
                m.ruta
                    .as_deref()
                    .is_some_and(|r| r.replace('\\', "/") == rel)
            }) {
                Some(mensaje) => Dueno::Chat {
                    carpeta: carpeta.to_path_buf(),
                    mensaje,
                },
                None => Dueno::DelAlmacen,
            };
        }
    }
    if !esta_dentro(ruta, raiz) {
        return Dueno::Suelto;
    }
    let indice = pixpin_proyecto::almacen::Indice::leer(raiz);
    for f in &indice.proyectos {
        let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &f.id);
        let Ok(c) = Cuaderno::leer_de(&carpeta) else {
            continue;
        };
        if let Some(mensaje) = c.mensajes.into_iter().find(|m| {
            m.ruta
                .as_deref()
                .and_then(|r| pixpin_proyecto::vista::ruta_real(raiz, &f.id, r))
                .is_some_and(|r| misma_ruta(&r, ruta))
        }) {
            return Dueno::Chat { carpeta, mensaje };
        }
    }
    Dueno::DelAlmacen
}

/// El nombre que se ensena en la pastilla: el del mensaje si es del chat
/// (el que el usuario le puso alli), el del fichero si no.
pub fn nombre_a_la_vista(dueno: &Dueno, ruta: &Path) -> String {
    match dueno {
        Dueno::Chat { mensaje, .. } if !mensaje.nombre.trim().is_empty() => mensaje.nombre.clone(),
        _ => pixpin_docs::nombre(ruta),
    }
}

/// Lo que paso al cambiar el nombre.
#[derive(Debug, Clone, PartialEq)]
pub enum Renombrado {
    /// Cambio el nombre del mensaje; el fichero sigue donde estaba.
    EnElChat { nombre: String },
    /// Se movio el fichero (y sus hermanos) a `ruta`.
    EnDisco { nombre: String, ruta: PathBuf },
}

#[derive(Debug)]
pub enum NoSeRenombro {
    /// Ya hay un fichero con ese nombre: no se pisa.
    YaExiste(String),
    /// Es del almacen pero de ningun mensaje: no se toca.
    DelAlmacen,
    Disco(std::io::Error),
}

/// **Cambia el nombre.** `antes` es el nombre que se ensenaba y `escrito`
/// lo que se tecleo. `Ok(None)` si no habia nada que cambiar.
pub fn renombrar(
    raiz: &Path,
    ruta: &Path,
    dueno: &Dueno,
    antes: &str,
    escrito: &str,
) -> Result<Option<Renombrado>, NoSeRenombro> {
    let Some(nuevo) = nombre_nuevo(escrito, antes) else {
        return Ok(None);
    };
    match dueno {
        Dueno::Chat { carpeta, mensaje } => {
            let mut m = mensaje.clone();
            m.nombre = nuevo.clone();
            match pixpin_proyecto::cuaderno::reemplazar(carpeta, &m) {
                Ok(true) => {
                    let _ = raiz;
                    Ok(Some(Renombrado::EnElChat { nombre: nuevo }))
                }
                // El mensaje ya no esta (se borro mientras se leia): nada.
                Ok(false) => Ok(None),
                Err(e) => Err(NoSeRenombro::Disco(e)),
            }
        }
        Dueno::DelAlmacen => Err(NoSeRenombro::DelAlmacen),
        Dueno::Suelto => {
            let destino = ruta.with_file_name(&nuevo);
            // Solo cambian mayusculas: en Windows es el mismo fichero y
            // `exists` diria que si.
            let mismo = misma_ruta(&destino, ruta);
            if !mismo && destino.exists() {
                return Err(NoSeRenombro::YaExiste(nuevo));
            }
            mover_con_hermanos(ruta, &destino).map_err(NoSeRenombro::Disco)?;
            Ok(Some(Renombrado::EnDisco {
                nombre: nuevo,
                ruta: destino,
            }))
        }
    }
}

/// Mueve el fichero y todo lo que cuelga de su nombre (`<nombre>.pixpin-*`).
/// El fichero primero: si falla no se ha tocado nada. Un hermano que no se
/// deje mover se dice en el registro y no deshace lo demas: el documento ya
/// tiene su nombre, y lo peor que pasa es perder donde se iba.
fn mover_con_hermanos(ruta: &Path, destino: &Path) -> std::io::Result<()> {
    std::fs::rename(ruta, destino)?;
    let (Some(carpeta), Some(viejo), Some(nuevo)) = (
        ruta.parent(),
        ruta.file_name().map(|n| n.to_string_lossy().to_string()),
        destino.file_name().map(|n| n.to_string_lossy().to_string()),
    ) else {
        return Ok(());
    };
    let prefijo = format!("{viejo}.pixpin-");
    let Ok(entradas) = std::fs::read_dir(carpeta) else {
        return Ok(());
    };
    for e in entradas.flatten() {
        let nombre = e.file_name().to_string_lossy().to_string();
        if let Some(resto) = nombre.strip_prefix(&prefijo) {
            let a = carpeta.join(format!("{nuevo}.pixpin-{resto}"));
            if let Err(err) = std::fs::rename(e.path(), &a) {
                tracing::warn!(?err, hermano = %nombre, "no se pudo mover un hermano del documento");
            }
        }
    }
    Ok(())
}

/// Lo que se dice al usuario tras intentarlo.
pub fn aviso(
    textos: &pixpin_store::Catalogo,
    r: &Result<Option<Renombrado>, NoSeRenombro>,
) -> Option<String> {
    let mut args = fluent_bundle::FluentArgs::new();
    match r {
        Ok(None) => None,
        Ok(Some(Renombrado::EnElChat { nombre } | Renombrado::EnDisco { nombre, .. })) => {
            args.set("nombre", sin_extension(nombre));
            Some(textos.t_args("renombrar-hecho", &args))
        }
        Err(NoSeRenombro::YaExiste(nombre)) => {
            args.set("nombre", nombre.clone());
            Some(textos.t_args("renombrar-ya-existe", &args))
        }
        Err(NoSeRenombro::Disco(e)) => {
            // El porque va al registro: al usuario le basta con saber que no.
            tracing::warn!(error = %e, "no se pudo cambiar el nombre en el disco");
            Some(textos.t("renombrar-no-se-pudo"))
        }
        Err(NoSeRenombro::DelAlmacen) => Some(textos.t("renombrar-no-se-pudo")),
    }
}

const VK_BACK: u32 = 0x08;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_F2: u32 = 0x71;

/// **La pastilla del nombre** de los dos lectores: de quien es el
/// documento, que nombre se ensena y, si se esta cambiando, lo escrito.
#[derive(Debug, Clone, Default)]
pub struct Pastilla {
    pub dueno: Dueno,
    pub visto: String,
    pub editando: Option<String>,
}

/// Lo que hizo una tecla en la pastilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tecla {
    NoEsMia,
    Consumida,
    /// Intro: hay que guardar lo escrito (`confirmar`).
    Confirmar,
}

/// Lo que queda tras confirmar: el aviso para el usuario y, si el fichero
/// se movio, su ruta nueva (el lector guarda sus cosas alli desde ahora).
#[derive(Debug, Default)]
pub struct Confirmado {
    pub aviso: Option<String>,
    pub ruta_nueva: Option<PathBuf>,
}

impl Pastilla {
    pub fn de(raiz: &Path, ruta: &Path) -> Pastilla {
        let dueno = dueno(raiz, ruta);
        let visto = nombre_a_la_vista(&dueno, ruta);
        Pastilla {
            dueno,
            visto,
            editando: None,
        }
    }

    /// Si se puede cambiar: lo del almacen que no es de ningun mensaje, no.
    pub fn se_puede(&self) -> bool {
        !matches!(self.dueno, Dueno::DelAlmacen)
    }

    /// Empieza a escribir, con el nombre sin su extension (como el campo del
    /// movil).
    pub fn empezar(&mut self) {
        if self.se_puede() {
            self.editando = Some(sin_extension(&self.visto));
        }
    }

    /// F2 empieza; escribiendo, Intro guarda, Esc deja el nombre como estaba
    /// y las letras son del campo (no atajos del lector).
    pub fn tecla(&mut self, vk: u32, ctrl: bool) -> Tecla {
        if self.editando.is_none() {
            if vk == VK_F2 && !ctrl && self.se_puede() {
                self.empezar();
                return Tecla::Consumida;
            }
            return Tecla::NoEsMia;
        }
        match vk {
            VK_RETURN => Tecla::Confirmar,
            VK_ESCAPE => {
                self.editando = None;
                Tecla::Consumida
            }
            // Con Ctrl sigue siendo del lector (Ctrl+S, Ctrl+Z...); todo lo
            // demas se escribe o llega luego como caracter.
            _ if ctrl => Tecla::NoEsMia,
            VK_BACK => Tecla::Consumida,
            0x21..=0x28 => Tecla::NoEsMia,
            _ => Tecla::Consumida,
        }
    }

    /// Un caracter tecleado. `false` si no se esta escribiendo.
    pub fn caracter(&mut self, c: char) -> bool {
        let Some(t) = self.editando.as_mut() else {
            return false;
        };
        if c == '\u{8}' {
            t.pop();
        } else if c >= ' ' && c != '\u{7f}' && t.chars().count() < LARGO_MAXIMO {
            t.push(c);
        }
        true
    }

    /// **Guarda lo escrito.** Si era del chat, el chat relee su cuaderno.
    pub fn confirmar(
        &mut self,
        raiz: &Path,
        ruta: &Path,
        textos: &pixpin_store::Catalogo,
    ) -> Confirmado {
        let Some(escrito) = self.editando.take() else {
            return Confirmado::default();
        };
        let r = renombrar(raiz, ruta, &self.dueno, &self.visto, &escrito);
        let aviso = aviso(textos, &r);
        let mut hecho = Confirmado {
            aviso,
            ruta_nueva: None,
        };
        match r {
            Ok(Some(Renombrado::EnElChat { nombre })) => {
                if let Dueno::Chat { mensaje, .. } = &mut self.dueno {
                    mensaje.nombre = nombre.clone();
                }
                self.visto = nombre;
                crate::ventana_chat::refrescar();
            }
            Ok(Some(Renombrado::EnDisco { nombre, ruta })) => {
                self.visto = nombre;
                hecho.ruta_nueva = Some(ruta);
            }
            Ok(None) => {}
            Err(e) => tracing::warn!(?e, ruta = %ruta.display(), "no se pudo cambiar el nombre"),
        }
        hecho
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn f2_empieza_intro_guarda_y_escape_deja_el_nombre_como_estaba() {
        let mut p = Pastilla {
            visto: "Informe.docx".into(),
            ..Default::default()
        };
        assert_eq!(
            p.tecla(0x54, false),
            Tecla::NoEsMia,
            "sin escribir, la T es del lector"
        );
        assert_eq!(p.tecla(VK_F2, false), Tecla::Consumida);
        assert_eq!(
            p.editando.as_deref(),
            Some("Informe"),
            "sin la extension, como el movil"
        );
        assert_eq!(
            p.tecla(0x54, false),
            Tecla::Consumida,
            "escribiendo, la T es del campo"
        );
        assert!(p.caracter('\u{8}'));
        assert!(p.caracter('X'));
        assert_eq!(p.editando.as_deref(), Some("InformX"));
        assert_eq!(
            p.tecla(0x53, true),
            Tecla::NoEsMia,
            "Ctrl+S sigue siendo del lector"
        );
        assert_eq!(p.tecla(VK_RETURN, false), Tecla::Confirmar);
        assert_eq!(p.tecla(VK_ESCAPE, false), Tecla::Consumida);
        assert_eq!(p.editando, None);
        assert_eq!(p.visto, "Informe.docx");
        assert!(
            !p.caracter('a'),
            "sin escribir, los caracteres no son suyos"
        );
    }

    #[test]
    fn lo_del_almacen_sin_mensaje_no_se_deja_cambiar() {
        let mut p = Pastilla {
            dueno: Dueno::DelAlmacen,
            visto: "x.pdf".into(),
            editando: None,
        };
        assert_eq!(p.tecla(VK_F2, false), Tecla::NoEsMia);
        p.empezar();
        assert_eq!(p.editando, None);
    }

    fn carpeta_de_prueba(nombre: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("pixpin-renombrar-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn el_nombre_nuevo_lleva_la_extension_de_antes_sin_repetirla() {
        assert_eq!(
            nombre_nuevo("Informe final", "borrador.docx").as_deref(),
            Some("Informe final.docx")
        );
        assert_eq!(
            nombre_nuevo("Informe.DOCX", "borrador.docx").as_deref(),
            Some("Informe.DOCX")
        );
        // Una «extension» de mas de cinco letras no lo es: no se pega.
        assert_eq!(
            nombre_nuevo("Nuevo", "acta.reunion-larga").as_deref(),
            Some("Nuevo")
        );
        assert_eq!(
            nombre_nuevo("Nuevo", "sin extension").as_deref(),
            Some("Nuevo")
        );
    }

    #[test]
    fn un_nombre_vacio_igual_o_reservado_no_cambia_nada() {
        assert_eq!(nombre_nuevo("", "a.pdf"), None);
        assert_eq!(nombre_nuevo("   \n", "a.pdf"), None);
        assert_eq!(nombre_nuevo("a", "a.pdf"), None, "igual que antes");
        assert_eq!(
            nombre_nuevo("CON", "a.pdf"),
            None,
            "nombre reservado de Windows"
        );
        assert_eq!(nombre_nuevo("com1", "a.pdf"), None);
        assert_eq!(nombre_nuevo("...", "a.pdf"), None);
    }

    #[test]
    fn lo_que_windows_no_admite_se_cambia_y_el_largo_se_corta() {
        assert_eq!(
            nombre_nuevo("a/b:c?", "x.pdf").as_deref(),
            Some("a-b-c-.pdf")
        );
        assert_eq!(
            nombre_nuevo("dos\nlineas", "x.pdf").as_deref(),
            Some("doslineas.pdf")
        );
        assert_eq!(
            nombre_nuevo("acaba en punto.", "x.pdf").as_deref(),
            Some("acaba en punto.pdf")
        );
        let largo = "a".repeat(200);
        assert_eq!(
            nombre_nuevo(&largo, "x.pdf").unwrap().chars().count(),
            LARGO_MAXIMO + 4
        );
    }

    #[test]
    fn un_fichero_suelto_se_mueve_con_su_tinta_y_su_sitio_de_lectura() {
        let d = carpeta_de_prueba("suelto");
        let raiz = d.join("almacen");
        std::fs::create_dir_all(&raiz).unwrap();
        let doc = d.join("libro.epub");
        std::fs::write(&doc, b"x").unwrap();
        std::fs::write(d.join("libro.epub.pixpin-lectura"), b"{}").unwrap();
        std::fs::create_dir_all(d.join("libro.epub.pixpin-anotado")).unwrap();
        std::fs::write(
            d.join("libro.epub.pixpin-anotado").join("capa.excalidraw"),
            b"{}",
        )
        .unwrap();
        // Un vecino con un nombre parecido no se toca.
        std::fs::write(d.join("libro.epub2.pixpin-lectura"), b"{}").unwrap();

        let dueno = dueno(&raiz, &doc);
        assert!(matches!(dueno, Dueno::Suelto));
        let r = renombrar(&raiz, &doc, &dueno, "libro.epub", "Novela").unwrap();
        let nueva = d.join("Novela.epub");
        assert_eq!(
            r,
            Some(Renombrado::EnDisco {
                nombre: "Novela.epub".into(),
                ruta: nueva.clone()
            })
        );
        assert!(nueva.is_file() && !doc.exists());
        assert!(d.join("Novela.epub.pixpin-lectura").is_file());
        assert!(
            d.join("Novela.epub.pixpin-anotado")
                .join("capa.excalidraw")
                .is_file()
        );
        assert!(
            d.join("libro.epub2.pixpin-lectura").is_file(),
            "el vecino sigue igual"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn no_se_pisa_un_fichero_que_ya_existe() {
        let d = carpeta_de_prueba("pisar");
        let doc = d.join("a.pdf");
        std::fs::write(&doc, b"a").unwrap();
        std::fs::write(d.join("b.pdf"), b"b").unwrap();
        let r = renombrar(&d.join("almacen"), &doc, &Dueno::Suelto, "a.pdf", "b");
        assert!(matches!(r, Err(NoSeRenombro::YaExiste(n)) if n == "b.pdf"));
        assert_eq!(
            std::fs::read(d.join("b.pdf")).unwrap(),
            b"b",
            "el otro sigue intacto"
        );
        assert!(doc.is_file());
        // Cambiar solo mayusculas si se deja: es el mismo fichero.
        assert!(renombrar(&d.join("almacen"), &doc, &Dueno::Suelto, "a.pdf", "A").is_ok());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_documento_del_chat_cambia_el_nombre_del_mensaje_y_no_el_fichero() {
        let d = carpeta_de_prueba("chat");
        let raiz = d.join("almacen");
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, "p1");
        std::fs::create_dir_all(carpeta.join("archivos")).unwrap();
        let doc = carpeta.join("archivos").join("doc-1.docx");
        std::fs::write(&doc, b"x").unwrap();
        let m = Mensaje::adjunto(
            pixpin_proyecto::cuaderno::Clase::Archivo,
            "Contrato.docx",
            "archivos/doc-1.docx",
            1,
            &pixpin_proyecto::cuaderno::Sello {
                cuando: 1,
                numero: 1,
                aparato: "PC".into(),
                proyecto: "p1".into(),
            },
        );
        pixpin_proyecto::cuaderno::anadir(&carpeta, &m).unwrap();

        let dueno = dueno(&raiz, &doc);
        assert!(matches!(&dueno, Dueno::Chat { mensaje, .. } if mensaje.id == m.id));
        assert_eq!(nombre_a_la_vista(&dueno, &doc), "Contrato.docx");
        let r = renombrar(&raiz, &doc, &dueno, "Contrato.docx", "Contrato firmado").unwrap();
        assert_eq!(
            r,
            Some(Renombrado::EnElChat {
                nombre: "Contrato firmado.docx".into()
            })
        );
        // El fichero no se mueve: su ruta viaja al movil en el mensaje.
        assert!(doc.is_file());
        let c = Cuaderno::leer_de(&carpeta).unwrap();
        assert_eq!(c.mensajes[0].nombre, "Contrato firmado.docx");
        assert_eq!(c.mensajes[0].ruta.as_deref(), Some("archivos/doc-1.docx"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_fichero_del_almacen_que_no_es_de_ningun_mensaje_no_se_toca() {
        let d = carpeta_de_prueba("almacen");
        let raiz = d.join("almacen");
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, "p1");
        std::fs::create_dir_all(&carpeta).unwrap();
        std::fs::write(carpeta.join("guardados.jsonl"), "").unwrap();
        let doc = carpeta.join("huerfano.pdf");
        std::fs::write(&doc, b"x").unwrap();
        let dueno = dueno(&raiz, &doc);
        assert!(matches!(dueno, Dueno::DelAlmacen));
        assert!(matches!(
            renombrar(&raiz, &doc, &dueno, "huerfano.pdf", "otro"),
            Err(NoSeRenombro::DelAlmacen)
        ));
        assert!(doc.is_file());
        let _ = std::fs::remove_dir_all(&d);
    }
}
