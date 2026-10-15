//! La palabra magica: copiar `timer`, `todo`, `gastos`, `pizarra` o `hoja`
//! **sola** y pinearla saca la herramienta en vez de un pin de texto.
//!
//! Puerto de `clipboard/MagicWord.kt` y del orden de `ContentClassifier.kt`
//! del movil, regla a regla. Puro: se prueba sin escritorio.
//!
//! La regla de que la palabra vaya sola no es un capricho: cada palabra
//! magica es una palabra que se deja de poder pinear como texto. Copiar
//! «time» de un documento para pegarlo en otro sitio tiene que seguir dando
//! una nota; solo cuando lo UNICO copiado es esa palabra se entiende que se
//! queria la herramienta. Por eso tambien son pocas y raras en aislado:
//! «nota», «lista», «plano», «dibujo» o «papel» se descartaron en el movil
//! porque se copian solas demasiado a menudo.

use std::collections::BTreeMap;

/// Lo que abre una palabra magica. Mismo orden y mismos nombres guardados
/// que `MiniApp` del movil (`TIMER`, `STOPWATCH`...): un ajuste de palabras
/// escrito alli se lee aqui igual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MiniApp {
    Temporizador,
    Cronometro,
    Tareas,
    Contador,
    Gastos,
    /// Un lienzo liso con cuatro fondos y cinco pautas, sobre el que se
    /// anota. En el movil es un pin de imagen con el fondo generado, y aqui
    /// igual: hereda el dibujo, el lienzo y la exportacion sin codigo nuevo.
    Pizarra,
    Ruleta,
    /// El lienzo infinito.
    Lienzo,
    /// **La hoja**: el mismo lienzo pero acotado, con su marco A4 puesto
    /// desde el primer momento. «Piensa sin limites» frente a «apunta esto
    /// en un papel»: dos intenciones y dos palabras, un solo motor.
    Hoja,
}

impl MiniApp {
    /// Todas, en el orden del movil. Es el orden del menu «Convertir en…».
    pub const TODAS: [MiniApp; 9] = [
        MiniApp::Temporizador,
        MiniApp::Cronometro,
        MiniApp::Tareas,
        MiniApp::Contador,
        MiniApp::Gastos,
        MiniApp::Pizarra,
        MiniApp::Ruleta,
        MiniApp::Lienzo,
        MiniApp::Hoja,
    ];

    /// El nombre con que se guarda, el del enum del movil.
    pub fn nombre(self) -> &'static str {
        match self {
            MiniApp::Temporizador => "TIMER",
            MiniApp::Cronometro => "STOPWATCH",
            MiniApp::Tareas => "CHECKLIST",
            MiniApp::Contador => "COUNTER",
            MiniApp::Gastos => "LEDGER",
            MiniApp::Pizarra => "BOARD",
            MiniApp::Ruleta => "RULETA",
            MiniApp::Lienzo => "DRAW",
            MiniApp::Hoja => "SHEET",
        }
    }

    /// El contrario de [`MiniApp::nombre`], sin mirar mayusculas.
    pub fn de_nombre(nombre: &str) -> Option<MiniApp> {
        let n = nombre.trim().to_uppercase();
        MiniApp::TODAS.into_iter().find(|m| m.nombre() == n)
    }

    /// Su sitio en [`MiniApp::TODAS`]: lo que viaja en el comando del menu.
    pub fn indice(self) -> u8 {
        MiniApp::TODAS.iter().position(|m| *m == self).unwrap_or(0) as u8
    }

    pub fn por_indice(i: u8) -> Option<MiniApp> {
        MiniApp::TODAS.get(i as usize).copied()
    }
}

/// Lo mas larga que puede ser una palabra magica (`MagicWord.MAXIMO`).
pub const MAXIMO: usize = 24;

/// Las de fabrica, letra por letra las del movil (`POR_DEFECTO`).
///
/// En una funcion y no en una constante porque es un mapa; se queda publica
/// porque «restablecer» tiene que dar exactamente esto.
pub fn por_defecto() -> BTreeMap<String, MiniApp> {
    use MiniApp::*;
    [
        ("time", Temporizador),
        ("timer", Temporizador),
        ("pomodoro", Temporizador),
        ("temporizador", Temporizador),
        ("crono", Cronometro),
        ("cronometro", Cronometro),
        ("cronómetro", Cronometro),
        ("stopwatch", Cronometro),
        ("todo", Tareas),
        ("checklist", Tareas),
        ("compras", Tareas),
        ("tareas", Tareas),
        ("count", Contador),
        ("contador", Contador),
        ("gastos", Gastos),
        ("cuentas", Gastos),
        ("money", Gastos),
        ("board", Pizarra),
        ("pizarra", Pizarra),
        // «random» y «azar» fuera: se copian solas dentro de cualquier texto.
        ("ruleta", Ruleta),
        ("roulette", Ruleta),
        ("choose", Ruleta),
        ("sorteo", Ruleta),
        ("sortear", Ruleta),
        ("elegir", Ruleta),
        // «draw» y «dibujo» fuera por lo mismo que «plano».
        ("canvas", Lienzo),
        ("lienzo", Lienzo),
        ("excalidraw", Lienzo),
        ("hoja", Hoja),
        ("sheet", Hoja),
    ]
    .into_iter()
    .map(|(p, m)| (p.to_string(), m))
    .collect()
}

/// La herramienta que abre `texto`, o `None` si es texto corriente.
///
/// Mayusculas y espacios alrededor dan igual; cualquier otra cosa
/// acompanandola la descarta, y un blanco por dentro tambien: «el time es
/// oro» es una frase, no una orden.
pub fn detectar(texto: &str, palabras: &BTreeMap<String, MiniApp>) -> Option<MiniApp> {
    let t = texto.trim();
    if t.is_empty() || palabras.is_empty() {
        return None;
    }
    let mas_larga = palabras.keys().map(|k| k.chars().count()).max()?;
    if t.chars().count() > mas_larga {
        return None;
    }
    if t.chars().any(char::is_whitespace) {
        return None;
    }
    palabras.get(&t.to_lowercase()).copied()
}

/// Una palabra tal como se guarda, o `None` si nunca podria dispararse
/// (vacia, con blancos o larguisima): vale mas no dejarla escribir que
/// guardarla y que no haga nada nunca.
pub fn normalizar(palabra: &str) -> Option<String> {
    let limpia = palabra.trim().to_lowercase();
    if limpia.is_empty() || limpia.chars().count() > MAXIMO {
        return None;
    }
    if limpia.chars().any(char::is_whitespace) {
        return None;
    }
    Some(limpia)
}

/// Lee las palabras guardadas, `palabra=APP` una por linea.
///
/// `None` es «no lo he tocado» y mandan las de fabrica; una cadena vacia es
/// «no quiero ninguna», que es otra respuesta y se respeta. Lo que no se
/// entiende se salta en silencio —tirar los ajustes enteros por una linea
/// rota seria peor— y si una palabra sale dos veces manda la primera.
pub fn leer(guardadas: Option<&str>) -> BTreeMap<String, MiniApp> {
    let Some(texto) = guardadas else {
        return por_defecto();
    };
    let mut salida = BTreeMap::new();
    for linea in texto.lines() {
        let Some(corte) = linea.find('=').filter(|c| *c > 0) else {
            continue;
        };
        let Some(palabra) = normalizar(&linea[..corte]) else {
            continue;
        };
        let Some(app) = MiniApp::de_nombre(&linea[corte + 1..]) else {
            continue;
        };
        salida.entry(palabra).or_insert(app);
    }
    salida
}

/// Como se guardan. Ver [`leer`].
pub fn escribir(palabras: &BTreeMap<String, MiniApp>) -> String {
    palabras
        .iter()
        .map(|(p, m)| format!("{p}={}", m.nombre()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Las palabras de una herramienta, en el orden del mapa (`deLaApp`).
///
/// El mapa del PC va por orden alfabetico (un `BTreeMap`) y el del movil por
/// orden de llegada: la lista sale ordenada, pero las palabras son las mismas
/// y se guardan con el mismo formato.
pub fn de_la_app(palabras: &BTreeMap<String, MiniApp>, app: MiniApp) -> Vec<String> {
    palabras
        .iter()
        .filter(|(_, m)| **m == app)
        .map(|(p, _)| p.clone())
        .collect()
}

/// Cambia de golpe las palabras de una herramienta y deja las demas
/// (`conLasDe`). Una palabra solo abre una cosa, asi que darsela a esta se
/// la quita a la que la tuviera; lo que no podria dispararse nunca (vacio,
/// con blancos, larguisimo) se cae, y lo repetido cuenta una vez.
pub fn con_las_de(
    palabras: &BTreeMap<String, MiniApp>,
    app: MiniApp,
    nuevas: &[&str],
) -> BTreeMap<String, MiniApp> {
    let limpias: Vec<String> = nuevas.iter().filter_map(|p| normalizar(p)).collect();
    let mut salida: BTreeMap<String, MiniApp> = palabras
        .iter()
        .filter(|(p, m)| **m != app && !limpias.contains(p))
        .map(|(p, m)| (p.clone(), *m))
        .collect();
    for p in limpias {
        salida.insert(p, app);
    }
    salida
}

/// Lo escrito en la caja de una herramienta, partido como en el dialogo del
/// movil: por comas y por saltos de linea.
pub fn partir_escrito(escrito: &str) -> Vec<&str> {
    escrito.split([',', '\n']).collect()
}

/// Las palabras que manda el usuario, puestas por quien lee los ajustes
/// (`fijar`). Vacio hasta entonces: mandan las de fabrica.
static VIGENTES: std::sync::Mutex<Option<BTreeMap<String, MiniApp>>> =
    std::sync::Mutex::new(None);

/// Fija las palabras guardadas (`None` = las de fabrica). Lo llama quien
/// tiene los ajustes, al arrancar y al cerrarse la ventana de ajustes.
pub fn fijar(guardadas: Option<&str>) {
    if let Ok(mut v) = VIGENTES.lock() {
        *v = Some(leer(guardadas));
    }
}

/// Las palabras que valen ahora: las fijadas, o las de fabrica.
pub fn vigentes() -> BTreeMap<String, MiniApp> {
    VIGENTES
        .lock()
        .ok()
        .and_then(|v| v.clone())
        .unwrap_or_else(por_defecto)
}

/// Lo que se pinea con un texto del portapapeles (`ContentClassifier`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clasificado {
    /// Una palabra magica: la herramienta.
    Herramienta(MiniApp),
    /// Un color (`ColorPin`): una muestra que se copia de un toque. El valor
    /// es ARGB, ver [`crate::color`].
    Color(u32),
    /// Texto con tabuladores (o barras de Markdown) pegado de una hoja de
    /// calculo: sin rejilla se lee en crudo justo cuando mas importa que las
    /// cifras queden alineadas.
    Tabla,
    Texto,
    Vacio,
}

/// El orden del movil: primero la palabra magica, luego el color, luego la
/// tabla y si no, texto. La palabra va antes que el color: ninguna es un
/// nombre de color CSS, pero si algun dia lo fuera, mandaria la herramienta.
pub fn clasificar(texto: &str, palabras: &BTreeMap<String, MiniApp>) -> Clasificado {
    let t = texto.trim();
    if t.is_empty() {
        return Clasificado::Vacio;
    }
    if let Some(m) = detectar(t, palabras) {
        return Clasificado::Herramienta(m);
    }
    if let Some(c) = crate::color::leer(t) {
        return Clasificado::Color(c);
    }
    if crate::tabla::parece_tabla(t) {
        return Clasificado::Tabla;
    }
    Clasificado::Texto
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_palabra_magica_sola_abre_su_herramienta_sin_mirar_mayusculas() {
        let p = por_defecto();
        assert_eq!(detectar("timer", &p), Some(MiniApp::Temporizador));
        assert_eq!(detectar("  TODO \n", &p), Some(MiniApp::Tareas));
        assert_eq!(detectar("Gastos", &p), Some(MiniApp::Gastos));
        assert_eq!(detectar("board", &p), Some(MiniApp::Pizarra));
        assert_eq!(detectar("hoja", &p), Some(MiniApp::Hoja));
        assert_eq!(detectar("cronómetro", &p), Some(MiniApp::Cronometro));
    }

    #[test]
    fn una_frase_con_la_palabra_dentro_sigue_siendo_texto() {
        let p = por_defecto();
        assert_eq!(detectar("el time es oro", &p), None);
        assert_eq!(detectar("todo bien", &p), None);
        assert_eq!(detectar("timers", &p), None, "ni una letra de mas");
        assert_eq!(detectar("", &p), None);
    }

    #[test]
    fn las_palabras_que_se_copian_solas_a_menudo_no_son_magicas() {
        // Decision del movil: perderlas como texto costaria mas que el atajo.
        let p = por_defecto();
        for palabra in [
            "nota", "lista", "plano", "dibujo", "draw", "papel", "random",
        ] {
            assert_eq!(detectar(palabra, &p), None, "{palabra}");
        }
    }

    #[test]
    fn un_texto_mas_largo_que_la_palabra_mas_larga_ni_se_busca() {
        let p = por_defecto();
        assert_eq!(detectar(&"a".repeat(200), &p), None);
    }

    #[test]
    fn sin_palabras_nada_es_magico() {
        assert_eq!(detectar("timer", &BTreeMap::new()), None);
    }

    #[test]
    fn normalizar_tira_lo_que_nunca_podria_dispararse() {
        assert_eq!(normalizar("  Pomodoro "), Some("pomodoro".into()));
        assert_eq!(normalizar(""), None);
        assert_eq!(normalizar("dos palabras"), None);
        assert_eq!(normalizar(&"x".repeat(MAXIMO + 1)), None);
        assert_eq!(normalizar(&"x".repeat(MAXIMO)), Some("x".repeat(MAXIMO)));
    }

    #[test]
    fn sin_tocar_mandan_las_de_fabrica_y_vacio_es_ninguna() {
        assert_eq!(leer(None), por_defecto());
        assert!(leer(Some("")).is_empty());
    }

    #[test]
    fn lo_guardado_vuelve_igual_y_las_lineas_rotas_se_saltan() {
        let p = por_defecto();
        assert_eq!(leer(Some(&escribir(&p))), p);
        let raro = "ya=TIMER\n=BOARD\nsin igual\nfoo=NOEXISTE\nYA=COUNTER\ncroquis=croquis";
        let l = leer(Some(raro));
        assert_eq!(l.len(), 1, "{l:?}");
        assert_eq!(
            l.get("ya"),
            Some(&MiniApp::Temporizador),
            "manda la primera"
        );
    }

    #[test]
    fn el_nombre_guardado_es_el_del_movil_y_vuelve() {
        assert_eq!(MiniApp::Pizarra.nombre(), "BOARD");
        assert_eq!(MiniApp::Hoja.nombre(), "SHEET");
        for m in MiniApp::TODAS {
            assert_eq!(MiniApp::de_nombre(m.nombre()), Some(m));
            assert_eq!(MiniApp::por_indice(m.indice()), Some(m));
        }
        assert_eq!(MiniApp::por_indice(99), None);
    }

    #[test]
    fn primero_la_palabra_despues_la_tabla_y_si_no_texto() {
        let p = por_defecto();
        assert_eq!(
            clasificar("pizarra", &p),
            Clasificado::Herramienta(MiniApp::Pizarra)
        );
        assert_eq!(clasificar("a\tb\n1\t2", &p), Clasificado::Tabla);
        assert_eq!(clasificar("hola que tal", &p), Clasificado::Texto);
        assert_eq!(clasificar("  \n ", &p), Clasificado::Vacio);
    }

    // ---- `classify devuelve el tipo correcto` (ContentClassifierTest) ----

    #[test]
    fn el_color_va_despues_de_la_palabra_y_antes_de_la_tabla() {
        let p = por_defecto();
        assert_eq!(clasificar("#29B8DB", &p), Clasificado::Color(0xFF29B8DB));
        assert_eq!(clasificar("  orange \n", &p), Clasificado::Color(0xFFFF9800));
        assert_eq!(clasificar("texto cualquiera", &p), Clasificado::Texto);
        assert_eq!(clasificar("   ", &p), Clasificado::Vacio);
        // Una palabra propia que se llama como un color abre la herramienta.
        let mias = leer(Some("red=TIMER"));
        assert_eq!(
            clasificar("red", &mias),
            Clasificado::Herramienta(MiniApp::Temporizador)
        );
        // Caso negativo: un color dentro de una frase es texto.
        assert_eq!(clasificar("el #fff de fondo", &p), Clasificado::Texto);
    }

    // ---- Las que pone el usuario (MagicWordTest) ----

    #[test]
    fn una_palabra_propia_abre_su_herramienta_y_las_de_fabrica_ya_no() {
        let mias = leer(Some("reloj=TIMER\nsorteito=RULETA"));
        assert_eq!(detectar("Reloj", &mias), Some(MiniApp::Temporizador));
        assert_eq!(detectar("sorteito", &mias), Some(MiniApp::Ruleta));
        assert_eq!(detectar("time", &mias), None);
    }

    #[test]
    fn sin_ninguna_palabra_no_se_abre_nada() {
        let ninguna = leer(Some(""));
        assert!(ninguna.is_empty());
        assert_eq!(detectar("time", &ninguna), None);
    }

    #[test]
    fn lo_que_no_se_entiende_se_salta() {
        let mias = leer(Some("reloj=TIMER\nbasura\n=TIMER\nx=NOEXISTE\ndos palabras=TIMER"));
        assert_eq!(mias.len(), 1);
        assert_eq!(mias.get("reloj"), Some(&MiniApp::Temporizador));
    }

    #[test]
    fn una_palabra_solo_puede_abrir_una_cosa() {
        let antes: BTreeMap<String, MiniApp> = [("x".to_string(), MiniApp::Temporizador)].into();
        let despues = con_las_de(&antes, MiniApp::Ruleta, &["x"]);
        assert_eq!(despues.get("x"), Some(&MiniApp::Ruleta));
        assert_eq!(despues.len(), 1);
    }

    #[test]
    fn cambiar_las_de_una_herramienta_deja_las_demas() {
        let antes: BTreeMap<String, MiniApp> = [
            ("time".to_string(), MiniApp::Temporizador),
            ("crono".to_string(), MiniApp::Cronometro),
        ]
        .into();
        let despues = con_las_de(&antes, MiniApp::Temporizador, &["reloj", "temporizador"]);
        assert_eq!(despues.get("crono"), Some(&MiniApp::Cronometro));
        assert_eq!(despues.get("time"), None);
        assert_eq!(
            de_la_app(&despues, MiniApp::Temporizador),
            vec!["reloj".to_string(), "temporizador".to_string()]
        );
    }

    #[test]
    fn al_guardar_se_limpian_los_espacios_las_mayusculas_y_lo_repetido() {
        let r = con_las_de(
            &BTreeMap::new(),
            MiniApp::Ruleta,
            &partir_escrito("  Sorteo ,sorteo,, dos palabras\nazar"),
        );
        // En orden alfabetico (el movil las deja en el de llegada).
        assert_eq!(
            de_la_app(&r, MiniApp::Ruleta),
            vec!["azar".to_string(), "sorteo".to_string()]
        );
    }

    #[test]
    fn una_herramienta_se_puede_quedar_sin_palabras() {
        let r = con_las_de(&por_defecto(), MiniApp::Temporizador, &[]);
        assert!(de_la_app(&r, MiniApp::Temporizador).is_empty());
        assert_eq!(detectar("time", &r), None);
        assert_eq!(detectar("crono", &r), Some(MiniApp::Cronometro));
    }

    #[test]
    fn las_palabras_retiradas_del_croquis_no_abren_nada() {
        let p = por_defecto();
        for palabra in ["croquis", "cad", "sketch", "hazme un croquis"] {
            assert_eq!(detectar(palabra, &p), None, "{palabra}");
        }
        assert_eq!(detectar("Excalidraw", &p), Some(MiniApp::Lienzo));
    }

    #[test]
    fn las_fijadas_mandan_y_sin_fijar_las_de_fabrica() {
        // Es la unica prueba que toca el estado global.
        fijar(Some("reloj=TIMER"));
        assert_eq!(detectar("reloj", &vigentes()), Some(MiniApp::Temporizador));
        assert_eq!(detectar("time", &vigentes()), None);
        fijar(None);
        assert_eq!(vigentes(), por_defecto());
    }
}
