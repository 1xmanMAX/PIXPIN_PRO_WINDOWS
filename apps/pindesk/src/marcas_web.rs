//! **Las marcas de texto de las paginas que se reajustan** (2-nov-2026):
//! resaltar, raya ondulada, subrayar, tachar y tapar lo elegido leyendo una
//! nota (el editor de notas, `notas_md::lector_web`) o una pagina web (el
//! visor de HTML), con los mismos botones y colores que la barrita del PDF.
//!
//! El usuario: «la herramienta de marcar el texto la quiero EN TODOS los
//! sitios donde se ve texto». En el PDF y el Word cada marca es dibujo de la
//! capa de su hoja (`pindesk_motor2d::marcas_texto`); una nota o una pagina
//! se recolocan con el ancho, asi que aqui la marca guarda su cita y la
//! pagina la vuelve a buscar al abrir (`pindesk_docs::ancla_texto`, las
//! cuentas en `herramientas/lector-md/src/marcas-texto.mjs`). La pagina
//! elige, pinta, deshace y manda la lista entera; aqui solo el fichero:
//!
//! - De un adjunto del chat o de una nota de un proyecto,
//!   `anot-<uid>.marcas-texto.json` de su mensaje: viaja con el
//!   (`pindesk_sincro::anotado::MARCAS_TEXTO`) y no se borra nunca para decir
//!   «vacio» (se escribe la lista vacia).
//! - Si no, `<doc>.pixpin-marcas-texto.json` a su lado (sin marcas, se quita).
//!
//! Al guardar se junta con lo que haya llegado al disco mientras tanto
//! (`ancla_texto::fusionar`), y lo que llegue con la pagina abierta se le
//! manda ([`MarcasDeLaPagina::mirar`]).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use pindesk_docs::ancla_texto::{self as at, MarcasDeTexto};
use pindesk_store::Catalogo;
use serde_json::{Value, json};

/// `apuntes.md` → `apuntes.md.pixpin-marcas-texto.json` (como los
/// `.pixpin-marcos` de junto al documento).
pub fn junto_a(doc: &Path) -> PathBuf {
    let mut s = doc.as_os_str().to_owned();
    s.push(".pixpin-marcas-texto.json");
    PathBuf::from(s)
}

/// Donde van las de `doc` y si viajan (con lo de junto al fichero pasado al
/// mensaje la primera vez).
pub fn ruta(doc: &Path) -> (PathBuf, bool) {
    crate::anotado_del_adjunto::del_mensaje_o_junto(
        doc,
        pindesk_sincro::anotado::MARCAS_TEXTO,
        junto_a(doc),
    )
}

type Sello = Option<(SystemTime, u64)>;

fn sello_de(f: &Path) -> Sello {
    std::fs::metadata(f)
        .ok()
        .and_then(|m| Some((m.modified().ok()?, m.len())))
}

/// Las marcas de la pagina abierta.
pub struct MarcasDeLaPagina {
    /// El fichero y si viaja. `None`: una nota nueva aun sin guardar (solo
    /// mientras este abierta; al guardarse, [`MarcasDeLaPagina::mudar`]).
    fichero: Option<(PathBuf, bool)>,
    /// Lo que habia en el disco al leer (la base de la fusion).
    base: MarcasDeTexto,
    /// Lo de aqui.
    mio: MarcasDeTexto,
    /// El fichero no se entiende: no se escribe encima (seria perderlo).
    roto: bool,
    sello: Sello,
}

impl MarcasDeLaPagina {
    /// Las de `doc` (`None`: sin fichero). `sobre`: el texto en que cuentan
    /// las posiciones (`ancla_texto::SOBRE_MARKDOWN` o `SOBRE_HTML`).
    pub fn abrir(doc: Option<&Path>, sobre: &str) -> Self {
        let mut m = MarcasDeLaPagina {
            fichero: doc.map(ruta),
            base: MarcasDeTexto::default(),
            mio: MarcasDeTexto::default(),
            roto: false,
            sello: None,
        };
        m.mio.sobre = sobre.to_string();
        if let Some((f, _)) = &m.fichero {
            m.sello = sello_de(f);
            match pindesk_proyecto::comentarios_de_notas::leer(f).map(|t| at::leer(&t)) {
                Some(Ok(l)) => {
                    m.base = l.clone();
                    m.mio = l;
                    if m.mio.sobre.is_empty() {
                        m.mio.sobre = sobre.to_string();
                    }
                }
                Some(Err(e)) => {
                    tracing::warn!(%e, fichero = %f.display(), "las marcas de texto no se entienden: no se tocan");
                    m.roto = true;
                }
                None => m.roto = true,
            }
        }
        m
    }

    /// La nota se guardo por primera vez (o cambio de sitio): lo marcado
    /// mientras tanto va a su fichero.
    pub fn mudar(&mut self, doc: Option<&Path>) {
        let antes = self.fichero.as_ref().map(|(f, _)| f.clone());
        self.fichero = doc.map(ruta);
        if self.fichero.as_ref().map(|(f, _)| f) != antes.as_ref() {
            self.base = MarcasDeTexto::default();
            self.sello = None;
            if !self.mio.marcas.is_empty() {
                self.guardar();
            }
        }
    }

    /// La lista para la pagina (JSON, la misma forma que manda ella).
    pub fn lista(&self) -> Value {
        serde_json::to_value(&self.mio.marcas).unwrap_or_else(|_| json!([]))
    }

    /// **Lo que mando la pagina** (la lista entera tras cada cambio): se
    /// guarda. Si al juntarla con el disco sale otra cosa (llego algo del
    /// movil), la lista nueva para mandarsela.
    pub fn de_la_pagina(&mut self, v: &Value) -> Option<Value> {
        let suyas = at::de_la_pagina(v);
        self.mio.marcas = suyas.clone();
        self.guardar();
        (self.mio.marcas != suyas).then(|| self.lista())
    }

    fn guardar(&mut self) {
        let Some((f, viaja)) = self.fichero.clone() else {
            return;
        };
        if self.roto {
            return;
        }
        // Lo que haya ahora en el disco (la sincronizacion pudo traer otras).
        let disco = match pindesk_proyecto::comentarios_de_notas::leer(&f).map(|t| at::leer(&t)) {
            Some(Ok(d)) => d,
            _ => self.base.clone(),
        };
        let junto = at::fusionar(&self.base, &self.mio, &disco);
        if junto.marcas.is_empty() && (!viaja || !f.exists()) {
            // Junto al fichero, sin marcas no queda fichero; del mensaje, uno
            // que no estaba no se crea vacio.
            if !viaja {
                let _ = std::fs::remove_file(&f);
            }
        } else if let Err(e) = pindesk_proyecto::anotado::escribir(&f, &at::escribir(&junto)) {
            tracing::warn!(?e, fichero = %f.display(), "no se pudieron guardar las marcas de texto");
            return;
        }
        self.base = junto.clone();
        self.mio = junto;
        self.sello = sello_de(&f);
    }

    /// **Si el fichero cambio por fuera** (la sincronizacion): se junta con
    /// lo de aqui y devuelve la lista nueva para la pagina.
    pub fn mirar(&mut self) -> Option<Value> {
        let (f, _) = self.fichero.as_ref()?;
        let ahora = sello_de(f);
        if ahora == self.sello {
            return None;
        }
        self.sello = ahora;
        let disco = at::leer(&pindesk_proyecto::comentarios_de_notas::leer(f)?).ok()?;
        let junto = at::fusionar(&self.base, &self.mio, &disco);
        self.base = disco;
        if junto.marcas == self.mio.marcas {
            return None;
        }
        self.mio = junto;
        Some(self.lista())
    }
}

/// **El guion de marcar de las paginas web**, construido con
/// `npm run build` en `herramientas/lector-md` (`src/marcas-pagina.js`). El
/// visor de HTML lo pone en cada pagina antes que lo suyo
/// (`pindesk_web::VisorHtml::nuevo`).
pub const GUION_PAGINA: &str = include_str!("../recursos/marcas-pagina.js");

/// Lo que manda el guion de una pagina web (`{"pixpin_marcas": …}`).
#[derive(Debug, Clone, PartialEq)]
pub enum DeLaPaginaWeb {
    /// Acaba de cargar: quiere las marcas y los rotulos.
    Pedir,
    /// Cambiaron: la lista entera.
    Lista(Value),
}

/// Si el mensaje `m` de la pagina es del guion de marcar, que dice. Los
/// demas (los del riel) siguen su camino.
pub fn de_la_pagina_web(m: &str) -> Option<DeLaPaginaWeb> {
    if !m.contains("pixpin_marcas") {
        return None;
    }
    let v: Value = serde_json::from_str(m).ok()?;
    match v.get("pixpin_marcas")?.as_str()? {
        "pedir" => Some(DeLaPaginaWeb::Pedir),
        "lista" => Some(DeLaPaginaWeb::Lista(v.get("marcas")?.clone())),
        _ => None,
    }
}

/// El guion que le deja a la pagina sus marcas (y los rotulos, si van).
pub fn guion_recibir(marcas: &Value, rotulos: Option<&Value>) -> String {
    let mut d = json!({ "marcas": marcas });
    if let Some(r) = rotulos {
        d["rotulos"] = r.clone();
    }
    format!("window.__pixpinMarcas&&window.__pixpinMarcas.recibir({d})")
}

impl MarcasDeLaPagina {
    /// **Lo que pide el guion de una pagina web**: el guion que la contesta
    /// (si hay que contestar).
    pub fn atender_web(&mut self, p: DeLaPaginaWeb, textos: &Catalogo) -> Option<String> {
        match p {
            DeLaPaginaWeb::Pedir => Some(guion_recibir(&self.lista(), Some(&rotulos(textos)))),
            DeLaPaginaWeb::Lista(l) => self.de_la_pagina(&l).map(|l| guion_recibir(&l, None)),
        }
    }
}

/// Los rotulos de los botones, en el idioma de la app (los de la barra del
/// pin).
pub fn rotulos(t: &Catalogo) -> Value {
    json!({
        "copiar": t.t("pin-v2-marca-copiar"),
        "resaltar": t.t("pin-v2-marca-resaltar"),
        "ondulada": t.t("pin-v2-marca-ondulada"),
        "subrayar": t.t("pin-v2-marca-subrayar"),
        "tachar": t.t("pin-v2-marca-tachar"),
        "tapar": t.t("pin-v2-marca-tapar"),
        "quitar": t.t("pin-v2-marca-quitar"),
        "color": t.t("marcas-texto-color"),
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta(etiqueta: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!(
            "pindesk-marcas-web-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    fn marca(id: &str, exact: &str) -> Value {
        json!({"id": id, "marca": "resaltar", "color": "#F5C218", "cuando": 1,
            "cita": {"type": "TextQuoteSelector", "exact": exact, "prefix": "", "suffix": ""},
            "posicion": {"type": "TextPositionSelector", "start": 0, "end": exact.len()}})
    }

    #[test]
    fn junto_a_un_fichero_suelto_y_sin_marcas_no_queda_fichero() {
        let d = carpeta("suelto");
        let doc = d.join("pagina.html");
        std::fs::write(&doc, "<p>hola</p>").unwrap();
        let mut m = MarcasDeLaPagina::abrir(Some(&doc), at::SOBRE_HTML);
        assert_eq!(m.lista(), json!([]));
        assert_eq!(
            m.de_la_pagina(&json!([marca("marca-texto-1", "hola")])),
            None
        );
        let f = junto_a(&doc);
        assert!(f.ends_with("pagina.html.pixpin-marcas-texto.json"));
        let leido = at::leer(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(leido.sobre, "html");
        assert_eq!(leido.marcas.len(), 1);
        // Otra vez abierta, ahi estan.
        let otra = MarcasDeLaPagina::abrir(Some(&doc), at::SOBRE_HTML);
        assert_eq!(otra.lista().as_array().unwrap().len(), 1);
        // Quitadas todas: fuera el fichero.
        m.de_la_pagina(&json!([]));
        assert!(!f.exists());
    }

    #[test]
    fn lo_que_llega_al_disco_con_la_pagina_abierta_se_junta_y_no_se_pisa() {
        let d = carpeta("juntar");
        let doc = d.join("nota.md");
        let mut m = MarcasDeLaPagina::abrir(Some(&doc), at::SOBRE_MARKDOWN);
        m.de_la_pagina(&json!([marca("marca-texto-1", "losa")]));
        // El movil anade otra al fichero.
        let f = junto_a(&doc);
        let mut disco = at::leer(&std::fs::read_to_string(&f).unwrap()).unwrap();
        disco
            .marcas
            .extend(at::de_la_pagina(&json!([marca("marca-texto-2", "obra")])));
        // Que el sello cambie aunque el reloj del disco sea grueso.
        std::fs::write(&f, at::escribir(&disco) + "\n\n").unwrap();
        let nueva = m.mirar().expect("se nota lo nuevo");
        assert_eq!(nueva.as_array().unwrap().len(), 2);
        // Aqui se pone una tercera con la lista vieja de la pagina: la del
        // movil no se pierde y se le devuelve a la pagina.
        let devuelta = m
            .de_la_pagina(&json!([
                marca("marca-texto-1", "losa"),
                marca("marca-texto-3", "piso")
            ]))
            .expect("la pagina tenia otra lista");
        let ids: Vec<&str> = devuelta
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["marca-texto-1", "marca-texto-3", "marca-texto-2"]);
        // Caso negativo: sin cambios en el disco no hay nada que mandar.
        assert_eq!(m.mirar(), None);
    }

    #[test]
    fn caso_negativo_un_fichero_roto_no_se_pisa() {
        let d = carpeta("roto");
        let doc = d.join("nota.md");
        let f = junto_a(&doc);
        std::fs::write(&f, "{roto").unwrap();
        let mut m = MarcasDeLaPagina::abrir(Some(&doc), at::SOBRE_MARKDOWN);
        m.de_la_pagina(&json!([marca("marca-texto-1", "losa")]));
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "{roto");
    }

    #[test]
    fn una_nota_nueva_las_guarda_al_tener_sitio() {
        let d = carpeta("nueva");
        let mut m = MarcasDeLaPagina::abrir(None, at::SOBRE_MARKDOWN);
        m.de_la_pagina(&json!([marca("marca-texto-1", "losa")]));
        let doc = d.join("nueva.md");
        m.mudar(Some(&doc));
        let leido = at::leer(&std::fs::read_to_string(junto_a(&doc)).unwrap()).unwrap();
        assert_eq!(leido.marcas.len(), 1);
    }

    #[test]
    fn la_pagina_web_pide_y_manda_por_pixpin_marcas() {
        assert_eq!(
            de_la_pagina_web(r#"{"pixpin_marcas":"pedir"}"#),
            Some(DeLaPaginaWeb::Pedir)
        );
        assert_eq!(
            de_la_pagina_web(r#"{"pixpin_marcas":"lista","marcas":[]}"#),
            Some(DeLaPaginaWeb::Lista(json!([])))
        );
        // Caso negativo: lo del riel no es suyo.
        assert_eq!(de_la_pagina_web(r#"{"pixpin_riel":"sitio","x":0}"#), None);
        assert_eq!(de_la_pagina_web(r#"{"pixpin_marcas":"otra"}"#), None);
        let d = carpeta("web");
        let doc = d.join("p.html");
        let mut m = MarcasDeLaPagina::abrir(Some(&doc), at::SOBRE_HTML);
        let t = Catalogo::nuevo(pindesk_store::Idioma::Espanol);
        let js = m.atender_web(DeLaPaginaWeb::Pedir, &t).unwrap();
        assert!(js.starts_with("window.__pixpinMarcas&&window.__pixpinMarcas.recibir({"));
        assert!(js.contains("\"rotulos\""));
        // Guardar no contesta si la lista no cambia al juntarla.
        assert_eq!(
            m.atender_web(
                DeLaPaginaWeb::Lista(json!([marca("marca-texto-1", "x")])),
                &t
            ),
            None
        );
        assert!(junto_a(&doc).exists());
        // El guion de la pagina va dentro y habla por `pixpin_marcas`.
        assert!(GUION_PAGINA.contains("pixpin_marcas") && GUION_PAGINA.contains("__pixpinMarcas"));
    }

    #[test]
    fn cada_rotulo_existe_en_los_catalogos() {
        for idioma in [
            pindesk_store::Idioma::Espanol,
            pindesk_store::Idioma::Ingles,
        ] {
            let t = Catalogo::nuevo(idioma);
            for (k, v) in rotulos(&t).as_object().unwrap() {
                let v = v.as_str().unwrap();
                assert!(
                    !v.is_empty() && !v.contains("marcas-texto-") && !v.contains("pin-v2-"),
                    "{k}: {v}"
                );
            }
        }
    }
}
