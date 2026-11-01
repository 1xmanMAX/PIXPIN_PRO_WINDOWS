//! Idiomas de la interfaz, con Fluent.
//!
//! Se elige Fluent y no un mapa de cadenas porque maneja bien plurales y
//! genero, que en español hacen falta en cuanto aparece un contador (y en
//! arabe hay seis formas de plural, en ruso tres: ver `docs/idiomas.md`).
//!
//! `resolver_idioma` recibe el locale como texto en vez de consultarlo al
//! sistema: este crate tiene `#![forbid(unsafe_code)]` y no puede llamar a
//! Win32. El ejecutable se lo pasa desde `pixpin_sistema::entorno`.
//!
//! **Catalogos y red.** Español (`es-ES`) e ingles (`en-US`) estan siempre y
//! con las mismas claves (lo vigila un test). Los demas idiomas pueden estar
//! a medias o no estar: `build.rs` mete los ficheros que haya, y lo que le
//! falte a uno se busca en ingles y, si tampoco, en español. Cada idioma es
//! su propio `FluentBundle` con su locale, para que las reglas de plural de
//! CLDR sean las suyas (`[few]` en ruso, `[two]` en arabe…) y un texto que
//! cae al ingles se forme con las del ingles.

use std::sync::OnceLock;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use unic_langid::LanguageIdentifier;

use crate::ajustes::PreferenciaIdioma;

// `CATALOGOS`: `(etiqueta, Some(texto del .ftl) | None)` en el orden de
// `Idioma::TODOS`. Lo escribe `build.rs` con los ficheros que existen.
include!(concat!(env!("OUT_DIR"), "/catalogos.rs"));

/// Los idiomas de la interfaz.
///
/// `Espanol` e `Ingles` conservan su nombre de siempre: hay cientos de
/// sitios que los usan. Para las tablas que solo existen en esos dos (textos
/// escritos aun en el codigo) esta [`Idioma::base`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Idioma {
    Espanol,
    Ingles,
    /// Chino simplificado (`zh-CN`). El tradicional (Taiwan, Hong Kong) cae
    /// aqui mientras no tenga catalogo propio.
    ChinoSimplificado,
    Hindi,
    /// Se escribe de derecha a izquierda: ver [`Idioma::es_rtl`].
    Arabe,
    /// Portugues de Brasil; el de Portugal cae aqui.
    Portugues,
    Frances,
    Aleman,
    Japones,
    Ruso,
    Coreano,
    Italiano,
    Indonesio,
    Turco,
}

impl Idioma {
    /// Todos, en el orden del selector de los ajustes y de `build.rs`.
    pub const TODOS: [Idioma; 14] = [
        Idioma::Espanol,
        Idioma::Ingles,
        Idioma::ChinoSimplificado,
        Idioma::Hindi,
        Idioma::Arabe,
        Idioma::Portugues,
        Idioma::Frances,
        Idioma::Aleman,
        Idioma::Japones,
        Idioma::Ruso,
        Idioma::Coreano,
        Idioma::Italiano,
        Idioma::Indonesio,
        Idioma::Turco,
    ];

    fn indice(self) -> usize {
        self as usize
    }

    /// La etiqueta BCP 47 del catalogo (`es-ES`, `zh-CN`…): la carpeta de
    /// `i18n/` y el locale que se le da a Fluent y a DirectWrite.
    pub fn etiqueta(self) -> &'static str {
        CATALOGOS[self.indice()].0
    }

    /// Solo el idioma (`es`, `zh`, `ar`…), en ISO 639-1: lo que piden
    /// Whisper, las paginas web (`lang`) y quien no distingue regiones.
    pub fn codigo(self) -> &'static str {
        let e = self.etiqueta();
        e.split('-').next().unwrap_or(e)
    }

    /// El nombre del idioma en el propio idioma: el que se ve en el
    /// selector, para que lo encuentre quien no lee el idioma actual.
    pub fn nombre_propio(self) -> &'static str {
        match self {
            Idioma::Espanol => "Español",
            Idioma::Ingles => "English",
            Idioma::ChinoSimplificado => "中文 (简体)",
            Idioma::Hindi => "हिन्दी",
            Idioma::Arabe => "العربية",
            Idioma::Portugues => "Português (Brasil)",
            Idioma::Frances => "Français",
            Idioma::Aleman => "Deutsch",
            Idioma::Japones => "日本語",
            Idioma::Ruso => "Русский",
            Idioma::Coreano => "한국어",
            Idioma::Italiano => "Italiano",
            Idioma::Indonesio => "Bahasa Indonesia",
            Idioma::Turco => "Türkçe",
        }
    }

    /// Si se escribe de derecha a izquierda. Lo usa el pintor para la
    /// direccion de lectura de los rotulos.
    pub fn es_rtl(self) -> bool {
        matches!(self, Idioma::Arabe)
    }

    /// `Espanol` o `Ingles`: para los textos que aun estan escritos en el
    /// codigo en esos dos idiomas. Español solo para el español; cualquier
    /// otro idioma lee antes ingles (la misma regla que `resolver_idioma`
    /// con un idioma sin catalogo).
    pub fn base(self) -> Idioma {
        match self {
            Idioma::Espanol => Idioma::Espanol,
            _ => Idioma::Ingles,
        }
    }

    /// Si su catalogo esta en el binario. Español e ingles, siempre.
    pub fn tiene_catalogo(self) -> bool {
        CATALOGOS[self.indice()].1.is_some()
    }

    /// El idioma de una etiqueta exacta de catalogo (`fr-FR`), si lo es.
    pub fn de_etiqueta(etiqueta: &str) -> Option<Idioma> {
        Idioma::TODOS
            .into_iter()
            .find(|i| i.etiqueta().eq_ignore_ascii_case(etiqueta))
    }

    /// Donde buscar un texto, en orden: el propio idioma, ingles y español.
    fn cadena(self) -> Vec<Idioma> {
        let mut v = vec![self];
        for red in [Idioma::Ingles, Idioma::Espanol] {
            if !v.contains(&red) {
                v.push(red);
            }
        }
        v
    }
}

/// El recurso de un idioma, analizado una sola vez por proceso: hay muchas
/// ventanas que crean su `Catalogo` y cada catalogo ahora mira hasta tres.
fn recurso(idioma: Idioma) -> Option<&'static FluentResource> {
    static RECURSOS: [OnceLock<Option<FluentResource>>; 14] = [const { OnceLock::new() }; 14];
    RECURSOS[idioma.indice()]
        .get_or_init(|| {
            let texto = CATALOGOS[idioma.indice()].1?;
            Some(match FluentResource::try_new(texto.to_string()) {
                Ok(r) => r,
                // Se queda con lo que si se entendio: un error de un
                // traductor no puede dejar todo el idioma en ingles. Los
                // tests no dejan pasar ninguno.
                Err((r, errores)) => {
                    tracing::warn!(idioma = idioma.etiqueta(), ?errores, "catalogo con errores");
                    r
                }
            })
        })
        .as_ref()
}

/// Decide el idioma final. La preferencia explicita del usuario siempre gana.
pub fn resolver_idioma(locale_sistema: &str, preferencia: PreferenciaIdioma) -> Idioma {
    match preferencia.idioma() {
        Some(i) => i,
        None => idioma_del_locale(locale_sistema),
    }
}

/// El idioma soportado mas cercano a un locale de Windows o POSIX
/// (`es-MX`, `zh-Hans-CN`, `pt_PT`…). Se mira la parte de idioma, salvo en
/// chino, donde la escritura decide. Sin parecido, ingles.
fn idioma_del_locale(locale: &str) -> Idioma {
    let partes: Vec<String> = locale
        .split(['-', '_', '.', '@'])
        .map(str::to_ascii_lowercase)
        .collect();
    let primera = partes.first().map(String::as_str).unwrap_or("");
    match primera {
        "es" => Idioma::Espanol,
        "en" => Idioma::Ingles,
        // zh-Hans, zh-CN, zh-SG: simplificado. zh-Hant, zh-TW, zh-HK, zh-MO
        // serian tradicional; mientras no haya catalogo, simplificado antes
        // que ingles (se lee, aunque con otras formas de los caracteres).
        "zh" => Idioma::ChinoSimplificado,
        "hi" => Idioma::Hindi,
        "ar" => Idioma::Arabe,
        // pt-PT cae al de Brasil: se entiende mucho mejor que el ingles.
        "pt" => Idioma::Portugues,
        "fr" => Idioma::Frances,
        "de" => Idioma::Aleman,
        "ja" => Idioma::Japones,
        "ru" => Idioma::Ruso,
        "ko" => Idioma::Coreano,
        "it" => Idioma::Italiano,
        // `in` es el codigo viejo del indonesio (Java lo sigue dando).
        "id" | "in" => Idioma::Indonesio,
        "tr" => Idioma::Turco,
        _ => Idioma::Ingles,
    }
}

/// Catalogo de textos ya cargado.
pub struct Catalogo {
    /// El idioma elegido y despues la red (ingles, español), cada uno con
    /// su locale. Los idiomas sin fichero no estan.
    bundles: Vec<(Idioma, FluentBundle<&'static FluentResource>)>,
}

impl Catalogo {
    pub fn nuevo(idioma: Idioma) -> Self {
        Self::con_recursos(
            idioma
                .cadena()
                .into_iter()
                .filter_map(|i| recurso(i).map(|r| (i, r)))
                .collect(),
        )
    }

    fn con_recursos(recursos: Vec<(Idioma, &'static FluentResource)>) -> Self {
        let bundles = recursos
            .into_iter()
            .map(|(idioma, recurso)| {
                let lang: LanguageIdentifier = idioma
                    .etiqueta()
                    .parse()
                    .expect("las etiquetas de idioma son constantes del propio codigo");
                let mut bundle = FluentBundle::new_concurrent(vec![lang]);
                // Fluent inserta marcas de direccion invisibles alrededor de
                // los argumentos. Utiles en arabe; aqui ensucian el texto y
                // rompen las comparaciones exactas de los tests y del codigo
                // que busca un texto dentro de otro. El pintor decide la
                // direccion de cada rotulo (`pixpin_render::idioma`).
                bundle.set_use_isolating(false);
                // Una clave repetida en un catalogo se queda con la ultima
                // en vez de perder el idioma entero.
                bundle.add_resource_overriding(recurso);
                (idioma, bundle)
            })
            .collect();
        Self { bundles }
    }

    /// El idioma con el que se creo.
    pub fn idioma(&self) -> Idioma {
        self.bundles
            .first()
            .map(|(i, _)| *i)
            .unwrap_or(Idioma::Ingles)
    }

    /// Texto sin argumentos. Si la clave falta, devuelve la propia clave.
    pub fn t(&self, clave: &str) -> String {
        self.con_argumentos(clave, None)
    }

    /// Texto con argumentos, por ejemplo `{ $atajo }`.
    pub fn t_args(&self, clave: &str, args: &FluentArgs) -> String {
        self.con_argumentos(clave, Some(args))
    }

    /// Si la clave existe en el idioma elegido (sin contar la red). Para
    /// medir cuanto esta traducido.
    pub fn traducida(&self, clave: &str) -> bool {
        self.bundles
            .first()
            .and_then(|(_, b)| b.get_message(clave))
            .is_some_and(|m| m.value().is_some())
    }

    fn con_argumentos(&self, clave: &str, args: Option<&FluentArgs>) -> String {
        let mut primero: Option<String> = None;
        for (idioma, bundle) in &self.bundles {
            let Some(patron) = bundle.get_message(clave).and_then(|m| m.value()) else {
                continue;
            };
            let mut errores = Vec::new();
            let texto = bundle.format_pattern(patron, args, &mut errores);
            if errores.is_empty() {
                return texto.into_owned();
            }
            tracing::warn!(
                clave,
                idioma = idioma.etiqueta(),
                ?errores,
                "error al formatear un texto"
            );
            // Español e ingles se dan como salen (como siempre); un texto
            // traducido que falla (un `{ $variable }` mal copiado) cede al
            // siguiente de la cadena, que esta revisado.
            let traducido = !matches!(idioma, Idioma::Espanol | Idioma::Ingles);
            let texto = texto.into_owned();
            if !traducido {
                return primero.unwrap_or(texto);
            }
            primero.get_or_insert(texto);
        }
        primero.unwrap_or_else(|| clave.to_string())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use fluent_syntax::ast;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn los_textos_de_captura_estan_en_los_dos_idiomas() {
        // El test de paridad ya comprueba que las claves coinciden, pero no
        // que existan. Este falla si alguien las olvida en ambos catalogos.
        let es = Catalogo::nuevo(Idioma::Espanol);
        let en = Catalogo::nuevo(Idioma::Ingles);
        for clave in ["captura-guardada", "captura-fallo"] {
            assert_ne!(es.t(clave), clave, "falta {clave} en es-ES");
            assert_ne!(en.t(clave), clave, "falta {clave} en en-US");
        }
    }

    #[test]
    fn los_rotulos_de_la_web_del_asistente_llevan_el_dominio() {
        let mut args = FluentArgs::new();
        args.set("dominio", "ugr.es");
        for (idioma, leyendo) in [
            (Idioma::Espanol, "🌐 leyendo ugr.es…"),
            (Idioma::Ingles, "🌐 reading ugr.es…"),
        ] {
            let c = Catalogo::nuevo(idioma);
            assert_eq!(c.t_args("asistente-web-leyendo", &args), leyendo);
            for clave in ["asistente-web-buscando", "asistente-web-leyendo-pagina"] {
                assert_ne!(c.t(clave), clave, "falta {clave}");
            }
        }
    }

    #[test]
    fn los_textos_de_la_pila_estan_en_los_dos_idiomas() {
        // La paridad comprueba que las claves coinciden; esto, que existen.
        let es = Catalogo::nuevo(Idioma::Espanol);
        let en = Catalogo::nuevo(Idioma::Ingles);
        for clave in [
            "pila-titulo-una",
            "pila-titulo-varias",
            "pila-copiar-elegidas",
            "pila-copiar-todas",
            "pila-quitar",
            "pila-copiadas",
        ] {
            assert_ne!(es.t(clave), clave, "falta {clave} en es-ES");
            assert_ne!(en.t(clave), clave, "falta {clave} en en-US");
        }
    }

    #[test]
    fn el_titulo_de_la_pila_interpola_cuantas_son() {
        let es = Catalogo::nuevo(Idioma::Espanol);
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("cuantas", 5);
        let texto = es.t_args("pila-titulo-varias", &args);
        assert!(texto.contains('5'), "deberia decir cuantas son: {texto:?}");
        assert!(
            !texto.contains('\u{2068}'),
            "sin marcas de aislamiento: {texto:?}"
        );
    }

    #[test]
    fn los_textos_de_la_barra_estan_en_los_dos_idiomas() {
        let es = Catalogo::nuevo(Idioma::Espanol);
        let en = Catalogo::nuevo(Idioma::Ingles);
        for clave in [
            "barra-copiar",
            "barra-guardar",
            "barra-guardar-como",
            "barra-descartar",
        ] {
            assert_ne!(es.t(clave), clave, "falta {clave} en es-ES");
            assert_ne!(en.t(clave), clave, "falta {clave} en en-US");
        }
    }

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
        // conocemos, y quien tenga el sistema en neerlandes entendera antes
        // ingles.
        assert_eq!(
            resolver_idioma("nl-NL", PreferenciaIdioma::Sistema),
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

        assert_eq!(es.t("comando-salir"), "Salir");
        assert_eq!(en.t("comando-salir"), "Quit");
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
    fn t_args_sustituye_el_argumento_y_no_deja_marcas_de_aislamiento() {
        // set_use_isolating(false) existe para esto: sin ella, Fluent envuelve
        // cada argumento interpolado entre U+2068 y U+2069 (marcas de
        // aislamiento direccional, utiles en arabe o hebreo). Son invisibles
        // en pantalla, asi que dos cadenas que "se ven iguales" pueden diferir
        // en bytes. Este test compara la cadena exacta para que quien quite
        // esa linea lo vea aqui, no en un bug reportado por un usuario.
        let es = Catalogo::nuevo(Idioma::Espanol);
        let en = Catalogo::nuevo(Idioma::Ingles);

        let mut args = FluentArgs::new();
        args.set("atajo", "Ctrl+Alt+X");

        let texto_es = es.t_args("error-atajo-ocupado", &args);
        let texto_en = en.t_args("error-atajo-ocupado", &args);

        assert_eq!(
            texto_es,
            "No se pudo registrar Ctrl+Alt+X: otra aplicación lo está usando."
        );
        assert_eq!(
            texto_en,
            "Could not register Ctrl+Alt+X: another application is using it."
        );

        for (idioma, texto) in [("es-ES", &texto_es), ("en-US", &texto_en)] {
            assert!(
                !texto.contains('\u{2068}') && !texto.contains('\u{2069}'),
                "el texto de {idioma} lleva marcas de aislamiento direccional \
                 (U+2068/U+2069) alrededor del argumento interpolado; falta \
                 `bundle.set_use_isolating(false)` en Catalogo::nuevo. Texto: {texto:?}"
            );
        }
    }

    #[test]
    fn build_rs_y_idioma_todos_van_en_el_mismo_orden() {
        for (i, idioma) in Idioma::TODOS.into_iter().enumerate() {
            assert_eq!(idioma.indice(), i, "{idioma:?}");
            assert_eq!(Idioma::de_etiqueta(idioma.etiqueta()), Some(idioma));
            assert!(idioma.etiqueta().starts_with(idioma.codigo()));
        }
        assert!(Idioma::Espanol.tiene_catalogo() && Idioma::Ingles.tiene_catalogo());
    }

    #[test]
    fn cada_idioma_se_nombra_en_su_lengua_y_solo_el_arabe_va_al_reves() {
        let nombres: BTreeSet<_> = Idioma::TODOS.iter().map(|i| i.nombre_propio()).collect();
        assert_eq!(nombres.len(), Idioma::TODOS.len(), "nombres repetidos");
        assert_eq!(Idioma::Arabe.nombre_propio(), "العربية");
        assert_eq!(Idioma::ChinoSimplificado.nombre_propio(), "中文 (简体)");
        let rtl: Vec<_> = Idioma::TODOS.into_iter().filter(|i| i.es_rtl()).collect();
        assert_eq!(rtl, [Idioma::Arabe]);
        assert_eq!(Idioma::Japones.base(), Idioma::Ingles);
        assert_eq!(Idioma::Espanol.base(), Idioma::Espanol);
    }

    #[test]
    fn el_locale_del_sistema_cae_en_el_idioma_mas_cercano() {
        let s = |l| resolver_idioma(l, PreferenciaIdioma::Sistema);
        for (locale, idioma) in [
            ("zh-CN", Idioma::ChinoSimplificado),
            ("zh-Hans-CN", Idioma::ChinoSimplificado),
            ("zh-SG", Idioma::ChinoSimplificado),
            // Tradicional: aun sin catalogo propio, al simplificado.
            ("zh-TW", Idioma::ChinoSimplificado),
            ("zh-Hant-HK", Idioma::ChinoSimplificado),
            ("pt-PT", Idioma::Portugues),
            ("pt-BR", Idioma::Portugues),
            ("hi-IN", Idioma::Hindi),
            ("ar-EG", Idioma::Arabe),
            ("fr-CA", Idioma::Frances),
            ("de-AT", Idioma::Aleman),
            ("ja-JP", Idioma::Japones),
            ("ru-RU", Idioma::Ruso),
            ("ko-KR", Idioma::Coreano),
            ("it-CH", Idioma::Italiano),
            ("id-ID", Idioma::Indonesio),
            ("in-ID", Idioma::Indonesio),
            ("tr-TR", Idioma::Turco),
            ("fr_FR.UTF-8", Idioma::Frances),
            ("ES-es", Idioma::Espanol),
            ("sw-KE", Idioma::Ingles),
        ] {
            assert_eq!(s(locale), idioma, "{locale}");
        }
        assert_eq!(
            resolver_idioma("es-ES", PreferenciaIdioma::Japones),
            Idioma::Japones
        );
    }

    /// Un catalogo de prueba que vive todo el proceso, como los de verdad.
    fn de_prueba(ftl: &str) -> &'static FluentResource {
        Box::leak(Box::new(
            FluentResource::try_new(ftl.to_string()).expect("el de prueba esta bien escrito"),
        ))
    }

    fn con_red(idioma: Idioma, ftl: &str) -> Catalogo {
        let mut v = vec![(idioma, de_prueba(ftl))];
        for red in [Idioma::Ingles, Idioma::Espanol] {
            v.push((red, recurso(red).unwrap()));
        }
        Catalogo::con_recursos(v)
    }

    #[test]
    fn lo_que_falta_en_un_idioma_a_medias_sale_en_ingles() {
        let fr = con_red(
            Idioma::Frances,
            "comando-salir = Quitter\nerror-atajo-ocupado = Raccourci { $tecla_mal } occupé\n",
        );
        assert_eq!(fr.idioma(), Idioma::Frances);
        assert_eq!(fr.t("comando-salir"), "Quitter");
        assert!(fr.traducida("comando-salir"));
        // No esta en el de prueba: el del ingles.
        assert!(!fr.traducida("barra-copiar"));
        assert_eq!(
            fr.t("barra-copiar"),
            Catalogo::nuevo(Idioma::Ingles).t("barra-copiar")
        );
        // Un texto traducido que falla (variable mal copiada) cede al ingles.
        let mut args = FluentArgs::new();
        args.set("atajo", "Ctrl+Alt+X");
        assert_eq!(
            fr.t_args("error-atajo-ocupado", &args),
            "Could not register Ctrl+Alt+X: another application is using it."
        );
        assert_eq!(fr.t("clave-que-no-existe"), "clave-que-no-existe");
    }

    #[test]
    fn un_idioma_sin_fichero_es_el_ingles_entero() {
        // Con o sin fichero, la cadena empieza por el idioma y acaba en
        // ingles y español: nunca se ve una clave que exista en ingles.
        for idioma in Idioma::TODOS {
            let c = Catalogo::nuevo(idioma);
            assert_ne!(c.t("comando-salir"), "comando-salir", "{idioma:?}");
            assert_eq!(c.t("app-nombre"), "PixPin Max", "{idioma:?}");
        }
    }

    #[test]
    fn los_plurales_siguen_las_reglas_de_cldr_de_cada_idioma() {
        let ftl = "n = { $n ->\n    [zero] cero\n    [one] uno\n    [two] dos\n    [few] pocos\n    [many] muchos\n   *[other] otros\n  }\n";
        let pide = |c: &Catalogo, n: i64| {
            let mut a = FluentArgs::new();
            a.set("n", n);
            c.t_args("n", &a)
        };
        let ar = con_red(Idioma::Arabe, ftl);
        for (n, forma) in [
            (0, "cero"),
            (1, "uno"),
            (2, "dos"),
            (3, "pocos"),
            (11, "muchos"),
            (100, "otros"),
        ] {
            assert_eq!(pide(&ar, n), forma, "arabe {n}");
        }
        let ru = con_red(Idioma::Ruso, ftl);
        for (n, forma) in [(1, "uno"), (2, "pocos"), (5, "muchos"), (21, "uno")] {
            assert_eq!(pide(&ru, n), forma, "ruso {n}");
        }
        // En japones todo es `other`.
        let ja = con_red(Idioma::Japones, ftl);
        assert_eq!(pide(&ja, 1), "otros");
    }

    // --- Los catalogos ---------------------------------------------------

    /// Las variables (`$x`) de cada mensaje y termino, con el parser de
    /// Fluent: tambien las de los selectores, las variantes y los atributos.
    fn variables_de(ftl: &str) -> BTreeMap<String, BTreeSet<String>> {
        fn en_patron(p: &ast::Pattern<&str>, v: &mut BTreeSet<String>) {
            for e in &p.elements {
                if let ast::PatternElement::Placeable { expression } = e {
                    en_expresion(expression, v);
                }
            }
        }
        fn en_expresion(e: &ast::Expression<&str>, v: &mut BTreeSet<String>) {
            match e {
                ast::Expression::Select { selector, variants } => {
                    en_linea(selector, v);
                    for variante in variants {
                        en_patron(&variante.value, v);
                    }
                }
                ast::Expression::Inline(i) => en_linea(i, v),
            }
        }
        fn en_linea(e: &ast::InlineExpression<&str>, v: &mut BTreeSet<String>) {
            match e {
                ast::InlineExpression::VariableReference { id } => {
                    v.insert(id.name.to_string());
                }
                ast::InlineExpression::FunctionReference { arguments, .. } => {
                    for a in &arguments.positional {
                        en_linea(a, v);
                    }
                    for a in &arguments.named {
                        en_linea(&a.value, v);
                    }
                }
                ast::InlineExpression::Placeable { expression } => en_expresion(expression, v),
                _ => {}
            }
        }
        let recurso = FluentResource::try_new(ftl.to_string()).unwrap_or_else(|(r, _)| r);
        let mut salida = BTreeMap::new();
        for entrada in recurso.entries() {
            let (clave, valor, atributos) = match entrada {
                ast::Entry::Message(m) => (m.id.name.to_string(), m.value.as_ref(), &m.attributes),
                ast::Entry::Term(t) => (format!("-{}", t.id.name), Some(&t.value), &t.attributes),
                _ => continue,
            };
            let mut v = BTreeSet::new();
            if let Some(p) = valor {
                en_patron(p, &mut v);
            }
            for a in atributos {
                en_patron(&a.value, &mut v);
            }
            salida.insert(clave, v);
        }
        salida
    }

    fn errores_de(ftl: &str) -> Vec<String> {
        match FluentResource::try_new(ftl.to_string()) {
            Ok(_) => Vec::new(),
            Err((_, e)) => e.iter().map(|e| format!("{e:?}")).collect(),
        }
    }

    fn catalogo(idioma: Idioma) -> Option<&'static str> {
        CATALOGOS[idioma.indice()].1
    }

    #[test]
    fn los_dos_catalogos_tienen_exactamente_las_mismas_claves() {
        // Este test es la red que impide que una traduccion se quede atras.
        let es = catalogo(Idioma::Espanol).unwrap();
        let en = catalogo(Idioma::Ingles).unwrap();
        assert_eq!(errores_de(es), Vec::<String>::new(), "es-ES no se entiende");
        assert_eq!(errores_de(en), Vec::<String>::new(), "en-US no se entiende");
        let claves_es: BTreeSet<_> = variables_de(es).into_keys().collect();
        let claves_en: BTreeSet<_> = variables_de(en).into_keys().collect();
        // Y coinciden con el recuento a ojo de siempre (lineas `clave =`):
        // si no, hay un mensaje que Fluent no ve.
        assert_eq!(claves_es, claves_de(es));

        let solo_es: Vec<_> = claves_es.difference(&claves_en).collect();
        let solo_en: Vec<_> = claves_en.difference(&claves_es).collect();

        assert!(
            solo_es.is_empty() && solo_en.is_empty(),
            "catalogos desincronizados. Solo en es-ES: {solo_es:?}. Solo en en-US: {solo_en:?}"
        );
    }

    /// Los demas idiomas pueden ir a medias (lo que falta sale en ingles),
    /// pero lo que traen tiene que entenderse, existir en en-US y usar las
    /// mismas variables: un `{ $n }` traducido como `{ $numero }` sale
    /// roto. Ver `docs/idiomas.md` y `herramientas/idiomas.py`.
    #[test]
    fn los_demas_catalogos_se_entienden_y_no_inventan_claves_ni_variables() {
        let en = variables_de(catalogo(Idioma::Ingles).unwrap());
        let mut fallos = Vec::new();
        for idioma in Idioma::TODOS {
            if matches!(idioma, Idioma::Espanol | Idioma::Ingles) {
                continue;
            }
            let etiqueta = idioma.etiqueta();
            let Some(ftl) = catalogo(idioma) else {
                println!("{etiqueta}: sin catalogo (todo en ingles)");
                continue;
            };
            for e in errores_de(ftl) {
                fallos.push(format!("{etiqueta}: no se entiende: {e}"));
            }
            let propio = variables_de(ftl);
            for (clave, vars) in &propio {
                match en.get(clave) {
                    None => fallos.push(format!("{etiqueta}: «{clave}» no existe en en-US")),
                    Some(v) if v != vars => {
                        fallos.push(format!("{etiqueta}: «{clave}» usa {vars:?} y en-US {v:?}"))
                    }
                    Some(_) => {}
                }
            }
            let hechas = propio.keys().filter(|c| en.contains_key(*c)).count();
            println!(
                "{etiqueta}: {hechas}/{} claves traducidas ({:.1} %)",
                en.len(),
                100.0 * hechas as f64 / en.len().max(1) as f64
            );
        }
        assert!(fallos.is_empty(), "{}", fallos.join("\n"));
    }

    #[test]
    fn la_comprobacion_de_variables_ve_selectores_y_funciones() {
        let v = variables_de(
            "a = { $n ->\n    [one] uno\n   *[other] { $n } de { $total }\n  }\nb = { NUMBER($x) } { -marca }\n-marca = PixPin Max\n",
        );
        assert_eq!(
            v["a"],
            BTreeSet::from(["n".to_string(), "total".to_string()])
        );
        assert_eq!(v["b"], BTreeSet::from(["x".to_string()]));
        assert!(v["-marca"].is_empty());
    }

    /// Extrae los identificadores de un fichero .ftl sin usar el parser de
    /// Fluent: basta con las lineas que empiezan por identificador y `=`.
    /// Solo sirve de contraste con el parser en el test de paridad.
    fn claves_de(ftl: &str) -> BTreeSet<String> {
        ftl.lines()
            .filter(|l| !l.trim_start().starts_with('#') && !l.starts_with(char::is_whitespace))
            .filter_map(|l| l.split_once('='))
            .map(|(clave, _)| clave.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect()
    }
}
