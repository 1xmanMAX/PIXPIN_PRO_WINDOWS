//! Idiomas de la interfaz, con Fluent.
//!
//! Se elige Fluent y no un mapa de cadenas porque maneja bien plurales y
//! genero, que en español hacen falta en cuanto aparece un contador.
//!
//! `resolver_idioma` recibe el locale como texto en vez de consultarlo al
//! sistema: este crate tiene `#![forbid(unsafe_code)]` y no puede llamar a
//! Win32. El ejecutable se lo pasa desde `pixpin_shell::entorno`.

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use unic_langid::LanguageIdentifier;

use crate::ajustes::PreferenciaIdioma;

const FTL_ES: &str = include_str!("../i18n/es-ES/main.ftl");
const FTL_EN: &str = include_str!("../i18n/en-US/main.ftl");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Idioma {
    Espanol,
    Ingles,
}

impl Idioma {
    fn etiqueta(self) -> &'static str {
        match self {
            Idioma::Espanol => "es-ES",
            Idioma::Ingles => "en-US",
        }
    }

    fn fuente(self) -> &'static str {
        match self {
            Idioma::Espanol => FTL_ES,
            Idioma::Ingles => FTL_EN,
        }
    }
}

/// Decide el idioma final. La preferencia explicita del usuario siempre gana.
pub fn resolver_idioma(locale_sistema: &str, preferencia: PreferenciaIdioma) -> Idioma {
    match preferencia {
        PreferenciaIdioma::Espanol => Idioma::Espanol,
        PreferenciaIdioma::Ingles => Idioma::Ingles,
        // Solo miramos la parte de idioma: es-MX y es-ES comparten catalogo.
        PreferenciaIdioma::Sistema => {
            if locale_sistema.split(['-', '_']).next().unwrap_or("") == "es" {
                Idioma::Espanol
            } else {
                Idioma::Ingles
            }
        }
    }
}

/// Catalogo de textos ya cargado.
pub struct Catalogo {
    bundle: FluentBundle<FluentResource>,
}

impl Catalogo {
    pub fn nuevo(idioma: Idioma) -> Self {
        let lang: LanguageIdentifier = idioma
            .etiqueta()
            .parse()
            .expect("las etiquetas de idioma son constantes del propio codigo");

        let recurso = FluentResource::try_new(idioma.fuente().to_string())
            .expect("los catalogos se validan por test en tiempo de compilacion");

        let mut bundle = FluentBundle::new_concurrent(vec![lang]);
        // Fluent inserta marcas de direccion invisibles alrededor de los
        // argumentos. Utiles en arabe o hebreo; aqui solo ensucian el texto y
        // rompen las comparaciones exactas de los tests.
        bundle.set_use_isolating(false);
        bundle
            .add_resource(recurso)
            .expect("no puede haber claves duplicadas en un unico recurso");

        Self { bundle }
    }

    /// Texto sin argumentos. Si la clave falta, devuelve la propia clave.
    pub fn t(&self, clave: &str) -> String {
        self.con_argumentos(clave, None)
    }

    /// Texto con argumentos, por ejemplo `{ $atajo }`.
    pub fn t_args(&self, clave: &str, args: &FluentArgs) -> String {
        self.con_argumentos(clave, Some(args))
    }

    fn con_argumentos(&self, clave: &str, args: Option<&FluentArgs>) -> String {
        let Some(mensaje) = self.bundle.get_message(clave) else {
            return clave.to_string();
        };
        let Some(patron) = mensaje.value() else {
            return clave.to_string();
        };
        let mut errores = Vec::new();
        let texto = self.bundle.format_pattern(patron, args, &mut errores);
        if !errores.is_empty() {
            tracing::warn!(clave, ?errores, "error al formatear un texto");
        }
        texto.into_owned()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_preferencia_explicita_gana_al_sistema() {
        assert_eq!(
            resolver_idioma("es-ES", PreferenciaIdioma::Ingles),
            Idioma::Ingles
        );
        assert_eq!(
            resolver_idioma("en-US", PreferenciaIdioma::Espanol),
            Idioma::Espanol
        );
    }

    #[test]
    fn en_modo_sistema_se_mira_el_locale() {
        assert_eq!(
            resolver_idioma("es-ES", PreferenciaIdioma::Sistema),
            Idioma::Espanol
        );
        assert_eq!(
            resolver_idioma("es-MX", PreferenciaIdioma::Sistema),
            Idioma::Espanol
        );
        assert_eq!(
            resolver_idioma("en-GB", PreferenciaIdioma::Sistema),
            Idioma::Ingles
        );
    }

    #[test]
    fn un_idioma_no_soportado_cae_a_ingles() {
        // Ingles y no español: es el reparto habitual para un idioma que no
        // conocemos, y quien tenga el sistema en aleman entendera antes ingles.
        assert_eq!(
            resolver_idioma("de-DE", PreferenciaIdioma::Sistema),
            Idioma::Ingles
        );
        assert_eq!(
            resolver_idioma("", PreferenciaIdioma::Sistema),
            Idioma::Ingles
        );
        assert_eq!(
            resolver_idioma("basura", PreferenciaIdioma::Sistema),
            Idioma::Ingles
        );
    }

    #[test]
    fn los_dos_catalogos_devuelven_texto_traducido() {
        let es = Catalogo::nuevo(Idioma::Espanol);
        let en = Catalogo::nuevo(Idioma::Ingles);

        assert_eq!(es.t("bandeja-salir"), "Salir");
        assert_eq!(en.t("bandeja-salir"), "Exit");
        // El nombre del producto no se traduce.
        assert_eq!(es.t("app-nombre"), "PixPin Max");
        assert_eq!(en.t("app-nombre"), "PixPin Max");
    }

    #[test]
    fn una_clave_que_falta_devuelve_la_propia_clave() {
        // Devolver la clave en vez de entrar en panico: un texto que falta es
        // feo, pero una aplicacion que se cierra por eso es inaceptable.
        let es = Catalogo::nuevo(Idioma::Espanol);
        assert_eq!(es.t("clave-que-no-existe"), "clave-que-no-existe");
    }

    #[test]
    fn los_dos_catalogos_tienen_exactamente_las_mismas_claves() {
        // Este test es la red que impide que una traduccion se quede atras.
        let claves_es = claves_de(include_str!("../i18n/es-ES/main.ftl"));
        let claves_en = claves_de(include_str!("../i18n/en-US/main.ftl"));

        let solo_es: Vec<_> = claves_es.difference(&claves_en).collect();
        let solo_en: Vec<_> = claves_en.difference(&claves_es).collect();

        assert!(
            solo_es.is_empty() && solo_en.is_empty(),
            "catalogos desincronizados. Solo en es-ES: {solo_es:?}. Solo en en-US: {solo_en:?}"
        );
    }

    /// Extrae los identificadores de un fichero .ftl sin usar el parser de
    /// Fluent: basta con las lineas que empiezan por identificador y `=`.
    fn claves_de(ftl: &str) -> std::collections::BTreeSet<String> {
        ftl.lines()
            .filter(|l| !l.trim_start().starts_with('#') && !l.starts_with(char::is_whitespace))
            .filter_map(|l| l.split_once('='))
            .map(|(clave, _)| clave.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect()
    }
}
