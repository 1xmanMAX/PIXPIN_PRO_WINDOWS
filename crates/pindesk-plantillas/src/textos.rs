//! **Los textos de la interfaz**: el catalogo Fluent propio de Plantillas
//! (`i18n/<idioma>/main.ftl`, claves `plt-…`), con la misma red que
//! pindesk-store: el idioma pedido, luego ingles y luego español.
//!
//! Va aqui y no en `pindesk-store` a proposito: cuando la app se lleve a su
//! repositorio, sus textos se van con ella sin tocar los de PinDesk, y la
//! version de Android o Apple los toma del mismo sitio que el modelo
//! (`docs/plantillas.md`, «Idiomas»). `herramientas/idiomas.py` los revisa
//! con los de PinDesk.

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

/// Los catorce, en el orden de `Idioma::TODOS` de pindesk-store.
pub const IDIOMAS: [&str; 14] = [
    "es-ES", "en-US", "zh-CN", "hi-IN", "ar-SA", "pt-BR", "fr-FR", "de-DE", "ja-JP", "ru-RU",
    "ko-KR", "it-IT", "id-ID", "tr-TR",
];

/// Los catalogos, dentro del binario.
pub const CATALOGOS: [(&str, &str); 14] = [
    ("es-ES", include_str!("../i18n/es-ES/main.ftl")),
    ("en-US", include_str!("../i18n/en-US/main.ftl")),
    ("zh-CN", include_str!("../i18n/zh-CN/main.ftl")),
    ("hi-IN", include_str!("../i18n/hi-IN/main.ftl")),
    ("ar-SA", include_str!("../i18n/ar-SA/main.ftl")),
    ("pt-BR", include_str!("../i18n/pt-BR/main.ftl")),
    ("fr-FR", include_str!("../i18n/fr-FR/main.ftl")),
    ("de-DE", include_str!("../i18n/de-DE/main.ftl")),
    ("ja-JP", include_str!("../i18n/ja-JP/main.ftl")),
    ("ru-RU", include_str!("../i18n/ru-RU/main.ftl")),
    ("ko-KR", include_str!("../i18n/ko-KR/main.ftl")),
    ("it-IT", include_str!("../i18n/it-IT/main.ftl")),
    ("id-ID", include_str!("../i18n/id-ID/main.ftl")),
    ("tr-TR", include_str!("../i18n/tr-TR/main.ftl")),
];

/// **Los textos ya cargados.**
pub struct Textos {
    etiqueta: &'static str,
    bundles: Vec<FluentBundle<FluentResource>>,
}

impl Textos {
    /// Los de un idioma (`es`, `en-US`, `pt_PT`…; sin parecido, ingles).
    pub fn nuevos(idioma: &str) -> Textos {
        let etiqueta = crate::locale::de(idioma).etiqueta;
        let mut cadena = vec![etiqueta];
        for red in ["en-US", "es-ES"] {
            if !cadena.contains(&red) {
                cadena.push(red);
            }
        }
        let bundles = cadena
            .into_iter()
            .filter_map(|e| {
                let (_, texto) = CATALOGOS.iter().find(|(x, _)| *x == e)?;
                let recurso =
                    FluentResource::try_new((*texto).to_string()).unwrap_or_else(|(r, _)| r);
                let lang: LanguageIdentifier = e.parse().ok()?;
                let mut b = FluentBundle::new_concurrent(vec![lang]);
                b.set_use_isolating(false);
                b.add_resource_overriding(recurso);
                Some(b)
            })
            .collect();
        Textos { etiqueta, bundles }
    }

    pub fn etiqueta(&self) -> &'static str {
        self.etiqueta
    }

    /// Un texto. Si la clave no existe en ninguno, la propia clave.
    pub fn t(&self, clave: &str) -> String {
        self.con(clave, &[])
    }

    /// Un texto con variables (`{ $n }`).
    pub fn con(&self, clave: &str, vars: &[(&str, String)]) -> String {
        let mut args = FluentArgs::new();
        for (k, v) in vars {
            match v.parse::<f64>() {
                Ok(n) => args.set(*k, FluentValue::from(n)),
                Err(_) => args.set(*k, FluentValue::from(v.clone())),
            }
        }
        for b in &self.bundles {
            let Some(p) = b.get_message(clave).and_then(|m| m.value()) else {
                continue;
            };
            let mut errores = Vec::new();
            let s = b.format_pattern(
                p,
                if vars.is_empty() { None } else { Some(&args) },
                &mut errores,
            );
            if errores.is_empty() {
                return s.into_owned();
            }
        }
        clave.to_string()
    }

    /// Si la clave existe en el idioma pedido (sin contar la red).
    pub fn traducida(&self, clave: &str) -> bool {
        self.bundles
            .first()
            .and_then(|b| b.get_message(clave))
            .is_some_and(|m| m.value().is_some())
    }

    /// Una lista separada por comas en el catalogo («Agua, Leer, …»).
    pub fn lista(&self, clave: &str) -> Vec<String> {
        self.t(clave)
            .split([',', '،', '、'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::collections::BTreeSet;

    fn claves(texto: &str) -> BTreeSet<String> {
        texto
            .lines()
            .filter(|l| !l.starts_with(' ') && !l.starts_with('#') && l.contains('='))
            .filter_map(|l| l.split('=').next())
            .map(|k| k.trim().to_string())
            .collect()
    }

    fn variables(texto: &str, clave: &str) -> BTreeSet<String> {
        let mut dentro = false;
        let mut v = BTreeSet::new();
        for l in texto.lines() {
            if !l.starts_with(' ') {
                dentro = l.split('=').next().map(str::trim) == Some(clave);
            }
            if dentro {
                let mut resto = l;
                while let Some(i) = resto.find('$') {
                    let nombre: String = resto[i + 1..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                        .collect();
                    v.insert(nombre);
                    resto = &resto[i + 1..];
                }
            }
        }
        v
    }

    /// Español e ingles con las mismas claves, y los demas sin claves de
    /// mas ni variables distintas; todos se leen sin errores de Fluent.
    #[test]
    fn los_catalogos_estan_completos_y_cuadran() {
        let es = claves(CATALOGOS[0].1);
        let en = claves(CATALOGOS[1].1);
        assert_eq!(es, en, "es-ES y en-US: las mismas claves");
        assert!(en.len() > 100);
        for (etiqueta, texto) in CATALOGOS {
            assert!(
                FluentResource::try_new(texto.to_string()).is_ok(),
                "{etiqueta}: Fluent mal escrito"
            );
            let propias = claves(texto);
            let sobran: Vec<_> = propias.difference(&en).collect();
            assert!(sobran.is_empty(), "{etiqueta}: sobran {sobran:?}");
            let faltan: Vec<_> = en.difference(&propias).collect();
            assert!(faltan.is_empty(), "{etiqueta}: faltan {faltan:?}");
            for k in &propias {
                assert_eq!(
                    variables(texto, k),
                    variables(CATALOGOS[1].1, k),
                    "{etiqueta}: «{k}» con otras variables"
                );
            }
        }
    }

    #[test]
    fn el_nombre_del_producto_y_la_red() {
        let es = Textos::nuevos("es");
        assert_eq!(es.t("plt-producto"), crate::NOMBRE_PRODUCTO);
        assert_eq!(
            Textos::nuevos("ja").t("plt-producto"),
            crate::NOMBRE_PRODUCTO
        );
        assert_eq!(es.t("no-existe"), "no-existe");
        assert!(es.con("plt-paginas", &[("n", "3".into())]).contains('3'));
        assert_eq!(Textos::nuevos("xx").etiqueta(), "en-US");
        assert!(Textos::nuevos("de").traducida("plt-guardar"));
    }
}
