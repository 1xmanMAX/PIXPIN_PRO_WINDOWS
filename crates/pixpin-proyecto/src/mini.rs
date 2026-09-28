//! Las mini-aplicaciones del cuaderno: documentos que viajan como texto.
//!
//! En Android son siete (`mini/MiniApps.kt`): tareas, gastos, cronometro,
//! temporizador, contador, ruleta y alarma. Aqui estan las siete, cada una
//! con su documento escrito **letra por letra como lo escribe el movil**,
//! porque ese texto viaja por la sincronizacion y el movil tiene que poder
//! abrirlo.
//!
//! **El documento ES el texto del mensaje.** No hay fichero aparte: un
//! mensaje de clase `MINIAPP` lleva el documento entero en `texto` y la
//! palabra de su tipo en `miniapp`. Esa palabra es un dato guardado y no se
//! renombra: cambiarla dejaria a las mini-apps ya escritas sin dueno.
//!
//! El formato es Markdown corriente —un titulo con `#` y casillas de
//! GitHub—, y eso no es casualidad: un aparato que no conozca la mini-app
//! ensena el texto tal cual y se entiende igual.

pub mod contador;
pub mod gastos;
pub mod ruleta;
pub mod tiempos;

/// La palabra que va en `Mensaje.miniapp` para una lista de tareas.
pub const TAREAS: &str = "tareas";

/// Conceptos con importe y su total. Ver [`gastos`].
pub const GASTOS: &str = "gastos";

/// Tiempo que sube. Ver [`tiempos::Cronometro`].
pub const CRONOMETRO: &str = "cronometro";

/// Tiempo que baja. Ver [`tiempos::Temporizador`].
pub const TEMPORIZADOR: &str = "temporizador";

/// Un numero que sube y baja. Ver [`contador`].
pub const CONTADOR: &str = "contador";

/// Nombres y un sorteo. Ver [`ruleta`].
pub const RULETA: &str = "ruleta";

/// Una hora a la que avisar. Ver [`tiempos::Alarma`].
pub const ALARMA: &str = "alarma";

/// Las siete palabras, en el orden en que Android las ofrece
/// (`MiniApps.kt:100-190`). El orden es parte de la pantalla, no del
/// documento, pero se copia igual para que las dos aplicaciones ensenen el
/// mismo menu.
pub const TODAS: [&str; 7] = [
    TAREAS,
    GASTOS,
    CRONOMETRO,
    TEMPORIZADOR,
    CONTADOR,
    RULETA,
    ALARMA,
];

/// Lo que la burbuja ensena de una mini-app **sin abrirla**.
///
/// Es el `ResumenMini` de `MiniApps.kt:79-89`. El texto viene ya compuesto
/// —«3 de 7», «60,50 €»— porque componerlo necesita saber de tareas o de
/// monedas, que es justo lo que sabe este nucleo y no sabe la ventana. Los
/// numeros van aparte por si una pantalla quiere decirlo de otra forma sin
/// volver a contar.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Resumen {
    pub texto: String,
    /// Cuantas van, cuando la mini-app cuenta cosas.
    pub hechas: usize,
    /// Cuantas hay en total.
    pub de: usize,
    /// De 0 a 1, para una barrita de avance. `None` cuando no hay avance que
    /// ensenar: 0 de 0 no es «nada hecho», es «nada que hacer».
    pub avance: Option<f32>,
    /// Todavia no tiene nada dentro: recien creada y sin tocar.
    pub vacia: bool,
}

/// El documento con el que **nace** una mini-app, o `None` si esa palabra no
/// la conocemos.
///
/// `None` y no un fallo: un mensaje guardado por una version posterior, con
/// una mini-app que aqui todavia no existe, tiene que poder seguir en la
/// lista como lo que es —un texto— en vez de tumbar la conversacion
/// (`MiniApp.de`, `MiniApps.kt:206-213`).
///
/// Nace con su cabecera y sin ninguna fila: un ejemplo dentro habria que
/// borrarlo antes de empezar.
pub fn documento_nuevo(miniapp: &str, titulo: &str, moneda: &gastos::Moneda) -> Option<String> {
    Some(match miniapp {
        TAREAS => escribir_tareas(titulo, &[]),
        GASTOS => gastos::escribir(&gastos::Libro {
            titulo: titulo.to_string(),
            moneda: moneda.clone(),
            gastos: Vec::new(),
        }),
        CRONOMETRO => tiempos::escribir_cronometro(titulo, &tiempos::Cronometro::default()),
        TEMPORIZADOR => tiempos::escribir_temporizador(titulo, &tiempos::Temporizador::default()),
        CONTADOR => contador::escribir(titulo, &contador::Cuenta::default()),
        RULETA => ruleta::escribir(titulo, &[]),
        ALARMA => tiempos::escribir_alarma(titulo, &tiempos::Alarma::default()),
        _ => return None,
    })
}

/// Lo que ensena la burbuja de esa mini-app, o `None` si no la conocemos.
///
/// La moneda solo se usa para los gastos, y solo cuando el documento no dice
/// la suya: es la del aparato al crear, no al leer, para que unos gastos
/// apuntados en un viaje no cambien de moneda al volver a casa.
pub fn resumen(miniapp: &str, documento: &str, moneda: &gastos::Moneda) -> Option<Resumen> {
    Some(match miniapp {
        TAREAS => resumen_tareas(documento),
        GASTOS => gastos::resumen(documento, moneda),
        CRONOMETRO => tiempos::resumen_cronometro(documento),
        TEMPORIZADOR => tiempos::resumen_temporizador(documento),
        CONTADOR => contador::resumen(documento),
        RULETA => ruleta::resumen(documento),
        ALARMA => tiempos::resumen_alarma(documento),
        _ => return None,
    })
}

/// El documento con otro titulo, o `None` si no conocemos esa mini-app.
///
/// En el movil el titulo se edita en la barra de arriba de la pantalla
/// (`MiniActivity.kt:136-148`) pegando `Cabecera.linea(nuevo)` delante de
/// `Cabecera.cuerpo(actual)`. Aqui se relee y se reescribe con el escritor de
/// cada una en vez de pegar trozos: el cuerpo de Android conserva el renglon
/// en blanco de debajo del titulo viejo, y cada cambio de nombre le va
/// sumando uno. Reescribiendo sale el mismo documento que dejaria el movil al
/// siguiente toque, que es el que se lee igual en los dos lados.
///
/// Un titulo vacio vale, como alli: el documento se queda sin cabecera.
pub fn con_titulo(
    miniapp: &str,
    documento: &str,
    nuevo: &str,
    moneda: &gastos::Moneda,
) -> Option<String> {
    Some(match miniapp {
        TAREAS => escribir_tareas(nuevo, &leer_tareas(documento)),
        GASTOS => {
            let mut libro = gastos::leer(documento, moneda);
            libro.titulo = nuevo.to_string();
            gastos::escribir(&libro)
        }
        CRONOMETRO => tiempos::escribir_cronometro(nuevo, &tiempos::leer_cronometro(documento)),
        TEMPORIZADOR => {
            tiempos::escribir_temporizador(nuevo, &tiempos::leer_temporizador(documento))
        }
        CONTADOR => contador::escribir(nuevo, &contador::leer(documento)),
        RULETA => ruleta::escribir(nuevo, &ruleta::leer(documento)),
        ALARMA => tiempos::escribir_alarma(nuevo, &tiempos::leer_alarma(documento)),
        _ => return None,
    })
}

/// Una linea de la lista: lo que hay que hacer, y si ya esta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tarea {
    pub texto: String,
    pub hecha: bool,
}

/// El titulo del documento, o vacio si no empieza por uno.
///
/// Es `Cabecera.titulo` (`MiniApps.kt:227`), `^#{1,6}\s+(.*)$` sobre la
/// primera linea: de una a seis almohadillas y **algun blanco** detras. Siete
/// almohadillas, o `#texto` pegado, no son un titulo de Markdown y el movil no
/// los lee como tal; si aqui si, el mismo documento tendria nombre en un
/// aparato y en el otro no, y renombrarlo en el PC borraria esa linea.
pub fn titulo(documento: &str) -> String {
    let primera = documento.split('\n').next().unwrap_or("").trim();
    let almohadillas = primera.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&almohadillas) {
        return String::new();
    }
    let resto = &primera[almohadillas..];
    if !resto.starts_with(es_blanco) {
        return String::new();
    }
    resto.trim().to_string()
}

/// Un blanco de los que entiende el `\s` de las expresiones de Java, que es
/// lo que usa el movil para leer estos documentos.
fn es_blanco(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\x0B' | '\x0C' | '\r' | '\n')
}

/// El documento **sin** su linea de titulo, que es lo que leen las mini-apps
/// de clave y valor.
///
/// Sin esto, un titulo como `# valor: 3` se leeria como un dato y el contador
/// arrancaria en tres. Es `Cabecera.cuerpo` (`MiniApps.kt:236-239`): si no hay
/// titulo se devuelve el documento entero, y si lo hay se corta **solo** la
/// primera linea —el renglon en blanco se queda, porque quitarlo cambiaria la
/// cuenta de lineas de todo lo demas.
pub fn cuerpo(documento: &str) -> &str {
    if titulo(documento).is_empty() {
        return documento;
    }
    match documento.split_once('\n') {
        Some((_, resto)) => resto,
        None => "",
    }
}

/// Las tareas de un documento.
///
/// Lo que no sea una casilla **se ignora sin quejarse**: un titulo, un
/// renglon en blanco o un parrafo que alguien dejo al editar la nota a mano.
/// Ignorar es mejor que fallar —una linea rara no puede hacer desaparecer la
/// lista entera— y mejor que convertirla en tarea, porque entonces el titulo
/// saldria como la primera cosa que hacer.
pub fn leer_tareas(documento: &str) -> Vec<Tarea> {
    documento.lines().filter_map(tarea_de_linea).collect()
}

/// Una linea suelta, si es una casilla.
///
/// Se admiten los tres marcadores de lista (`-`, `*`, `+`) al leer aunque
/// solo se escriba con `-`: es lo que hace cualquier lector de Markdown, y
/// una lista pegada de fuera que use asteriscos no tiene por que perderse.
fn tarea_de_linea(linea: &str) -> Option<Tarea> {
    let resto = linea.trim_start();
    let resto = resto.strip_prefix(['-', '*', '+'])?;
    // Tiene que haber al menos un espacio entre el marcador y la casilla.
    if !resto.starts_with(' ') && !resto.starts_with('\t') {
        return None;
    }
    let resto = resto.trim_start();
    let resto = resto.strip_prefix('[')?;
    let marca = resto.chars().next()?;
    let resto = resto.get(marca.len_utf8()..)?;
    let resto = resto.strip_prefix(']')?;
    let hecha = match marca {
        'x' | 'X' => true,
        ' ' => false,
        _ => return None,
    };
    Some(Tarea {
        texto: resto.trim().to_string(),
        hecha,
    })
}

/// El documento entero: su titulo y sus casillas.
///
/// Escribe SOLO tareas: esta pantalla ensena casillas y nada mas, asi que
/// guardar un parrafo invisible seria guardar algo que el usuario no puede
/// ver ni borrar.
pub fn escribir_tareas(titulo: &str, tareas: &[Tarea]) -> String {
    let cuerpo = tareas
        .iter()
        .map(|t| {
            let marca = if t.hecha { "x" } else { " " };
            format!("- [{marca}] {}", en_una_linea(&t.texto))
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{}{cuerpo}", linea_de_titulo(titulo))
}

/// La linea del titulo lista para encabezar un documento, con su renglon en
/// blanco. Vacia si no hay titulo: un `#` suelto ensuciaria el documento con
/// una linea que no dice nada.
pub fn linea_de_titulo(titulo: &str) -> String {
    let limpio = en_una_linea(titulo);
    let limpio = limpio.trim_start_matches(['#', ' ']).trim();
    if limpio.is_empty() {
        String::new()
    } else {
        format!("# {limpio}\n\n")
    }
}

/// El texto en UNA sola linea, que es lo que cabe en una tarea.
///
/// Sin esto, un parrafo de tres renglones pegado en una tarea sale al
/// guardar con sus renglones dos y tres SIN casilla delante: dejan de ser
/// tareas y se convierten en texto suelto. Se tratan las tres formas de
/// partir linea, porque el texto puede venir del portapapeles de cualquier
/// sitio y una sola sin tratar es el documento roto.
fn en_una_linea(texto: &str) -> String {
    texto
        .replace("\r\n", " ")
        .replace(['\r', '\n'], " ")
        .trim()
        .to_string()
}

/// Cambia una tarea de estado y devuelve el documento nuevo.
///
/// Se reescribe el documento entero a proposito: es corto, y tocar solo su
/// linea obligaria a mantener dos formas de escribirlo —una para crear y
/// otra para editar— que se separarian en el primer cambio de formato.
pub fn alternar(documento: &str, cual: usize) -> String {
    let mut tareas = leer_tareas(documento);
    let Some(t) = tareas.get_mut(cual) else {
        return documento.to_string();
    };
    t.hecha = !t.hecha;
    escribir_tareas(&titulo(documento), &tareas)
}

/// Anade una tarea al final. Lo vacio no entra: una casilla sin texto no
/// dice que hay que hacer y ademas cuenta en el «3 de 7» de la burbuja.
///
/// Pasa por [`saneado`], como `Tareas.anadir` (`Tareas.kt:102-106`): sin eso,
/// el mismo «pegar `- [x] pan`» daria dos documentos distintos en los dos
/// aparatos.
pub fn anadir(documento: &str, texto: &str) -> String {
    let limpio = saneado(texto);
    if limpio.is_empty() {
        return documento.to_string();
    }
    let mut tareas = leer_tareas(documento);
    tareas.push(Tarea {
        texto: limpio,
        hecha: false,
    });
    escribir_tareas(&titulo(documento), &tareas)
}

/// Le cambia el texto a una tarea (`Tareas.renombrar`, `Tareas.kt:120-125`).
///
/// Si el texto nuevo queda vacio **no se toca**: vaciar una tarea no es la
/// forma de borrarla —para eso esta su aspa— y una casilla en blanco que nadie
/// pidio seguiria contando en el «3 de 7».
pub fn renombrar(documento: &str, cual: usize, texto: &str) -> String {
    let mut tareas = leer_tareas(documento);
    let limpio = saneado(texto);
    let Some(t) = tareas.get_mut(cual) else {
        return documento.to_string();
    };
    if limpio.is_empty() || t.texto == limpio {
        return documento.to_string();
    }
    t.texto = limpio;
    escribir_tareas(&titulo(documento), &tareas)
}

/// Mueve una tarea de sitio (`Tareas.mover`, `Tareas.kt:140-145`).
///
/// `hasta` se **recorta** en vez de rechazarse: pasarse del final quiere decir
/// «al final», y cancelar el gesto por pasarse una fila es lo que hace que
/// reordenar se sienta roto.
pub fn mover(documento: &str, desde: usize, hasta: usize) -> String {
    let mut tareas = leer_tareas(documento);
    if desde >= tareas.len() {
        return documento.to_string();
    }
    let destino = hasta.min(tareas.len() - 1);
    if destino == desde {
        return documento.to_string();
    }
    let t = tareas.remove(desde);
    tareas.insert(destino, t);
    escribir_tareas(&titulo(documento), &tareas)
}

/// El texto de una tarea listo para guardarse (`Tareas.saneado`,
/// `Tareas.kt:199-209`).
///
/// En una linea, y **sin el marcador de lista de delante**, las veces que haga
/// falta. Pegar «- [x] comprar pan» copiado de otra lista guardaria una tarea
/// con ese texto que, al releer el documento, se leeria como una tarea **ya
/// hecha** llamada «comprar pan»: una tarea que se tacha sola. Quitando la
/// marca, lo que se ve escrito es lo que se guarda.
///
/// El bucle tiene tope porque el texto viene de fuera: cien mil guiones
/// pegados no pueden dejar la ventana pensando.
pub fn saneado(texto: &str) -> String {
    let linea = en_una_linea(texto);
    let mut t = linea.as_str();
    for _ in 0..MAXIMO_DE_MARCAS {
        match sin_marca(t) {
            Some(resto) if resto.len() < t.len() => t = resto,
            _ => break,
        }
    }
    t.trim().to_string()
}

/// Cuantos marcadores encadenados se quitan antes de dejarlo estar.
const MAXIMO_DE_MARCAS: usize = 8;

/// Lo que queda tras UN marcador de lista, o `None` si no empieza por uno.
///
/// Es `MARCA` de `Tareas.kt:54`, `^\s*(?:[-*+]\s+(?:\[[ xX]]\s?)?|\d+[.)]\s+)`,
/// escrito a mano para no traer un motor de expresiones por una linea. El
/// blanco detras del marcador es obligatorio: «-5 cajas» es un texto, no una
/// lista.
fn sin_marca(t: &str) -> Option<&str> {
    let s = t.trim_start_matches(es_blanco);
    // Cuantos bytes de blanco hay al principio; al menos uno, o no vale.
    let blancos = |r: &str| -> Option<usize> {
        let n = r.len() - r.trim_start_matches(es_blanco).len();
        (n > 0).then_some(n)
    };
    if let Some(r) = s.strip_prefix(['-', '*', '+']) {
        let r = &r[blancos(r)?..];
        // La casilla, si la hay, se va con su marcador.
        let b = r.as_bytes();
        if b.len() >= 3 && b[0] == b'[' && matches!(b[1], b' ' | b'x' | b'X') && b[2] == b']' {
            let r = &r[3..];
            return Some(r.strip_prefix(es_blanco).unwrap_or(r));
        }
        return Some(r);
    }
    let cifras = s.bytes().take_while(u8::is_ascii_digit).count();
    if cifras == 0 {
        return None;
    }
    let r = s[cifras..].strip_prefix(['.', ')'])?;
    Some(&r[blancos(r)?..])
}

/// Cuantas hechas de cuantas: lo que se ensena en la burbuja.
pub fn cuenta(tareas: &[Tarea]) -> (usize, usize) {
    (tareas.iter().filter(|t| t.hecha).count(), tareas.len())
}

/// «3 de 7», sin abrir la lista (`Tareas.kt:165-179`).
///
/// Se cuenta sobre el documento guardado y no sobre un contador aparte,
/// porque un contador se desincroniza en cuanto alguien edita el texto por
/// otro camino y entonces la burbuja miente, que es peor que no decir nada.
pub fn resumen_tareas(documento: &str) -> Resumen {
    let tareas = leer_tareas(documento);
    let (hechas, de) = cuenta(&tareas);
    Resumen {
        texto: format!("{hechas} de {de}"),
        hechas,
        de,
        avance: if de == 0 {
            None
        } else {
            Some(hechas as f32 / de as f32)
        },
        vacia: de == 0,
    }
}

/// Las lineas `- clave: valor` del cuerpo, en el orden en que estan.
///
/// Es `valores()` de `Tiempos.kt:78-86` y de `Contador.kt:32-39`, que son la
/// misma funcion escrita dos veces: se quita **un** guion de delante, se corta
/// en el primer `:` y la clave se compara en minusculas. Una linea sin `:`, o
/// que empiece por `:`, no es un dato y se salta.
///
/// La clave se devuelve **tal como estaba escrita** y solo se baja al
/// comparar ([`valor`], [`otras_claves`]). Guardada ya bajada, una
/// `- ritmoBPM: 3` de un movil mas nuevo volveria como `- ritmobpm: 3` al
/// tocar cualquier boton: el movil la seguiria leyendo, pero el texto habria
/// cambiado sin que nadie lo tocara, y el resumen de sincronizacion con el.
///
/// Se devuelve una lista y no un mapa a proposito: hace falta saber que claves
/// NO se entendieron para volver a escribirlas, que es lo que impide que
/// marcar una casilla se lleve por delante un dato de una version mas nueva
/// del movil.
pub(crate) fn valores(documento: &str) -> Vec<(String, String)> {
    cuerpo(documento)
        .lines()
        .filter_map(|linea| {
            let limpia = linea.trim();
            let limpia = limpia.strip_prefix('-').unwrap_or(limpia).trim();
            let corte = limpia.find(':')?;
            if corte == 0 {
                return None;
            }
            Some((
                limpia[..corte].trim().to_string(),
                limpia[corte + 1..].trim().to_string(),
            ))
        })
        .collect()
}

/// El valor de esa clave, el **ultimo** si esta repetida.
///
/// El ultimo porque en Kotlin estas lineas acaban en un `toMap()`, y ahi la
/// repetida pisa a la anterior. Un documento con dos `- valor:` tiene que
/// decir lo mismo en los dos aparatos.
pub(crate) fn valor<'a>(valores: &'a [(String, String)], clave: &str) -> Option<&'a str> {
    valores
        .iter()
        .rev()
        .find(|(c, _)| c.to_lowercase() == clave)
        .map(|(_, v)| v.as_str())
}

/// Las claves que esta mini-app no entiende, tal cual venian.
///
/// Android las tira al guardar; aqui se conservan y se vuelven a escribir
/// detras de las conocidas. Cuesta nada y evita que un PC con una version
/// vieja borre, con solo tocar un boton, un dato que escribio un movil mas
/// nuevo. Para el movil no cambia nada: su lector se salta lo que no conoce.
pub(crate) fn otras_claves(
    valores: &[(String, String)],
    conocidas: &[&str],
) -> Vec<(String, String)> {
    valores
        .iter()
        .filter(|(c, _)| !conocidas.contains(&c.to_lowercase().as_str()))
        .cloned()
        .collect()
}

/// Un documento de `- clave: valor`, con su titulo delante
/// (`Tiempos.kt:88-89`).
pub(crate) fn documento_de_claves(titulo: &str, pares: &[(String, String)]) -> String {
    let cuerpo = pares
        .iter()
        .map(|(c, v)| format!("- {c}: {v}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{}{cuerpo}", linea_de_titulo(titulo))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_leen_las_casillas_y_se_ignora_lo_demas() {
        let d = "# Compra\n\nlo que sea\n- [ ] pan\n* [x] vino\n+ [X] sal\n\ntexto suelto";
        let t = leer_tareas(d);
        assert_eq!(t.len(), 3, "los tres marcadores valen: {t:?}");
        assert_eq!(t[0].texto, "pan");
        assert!(!t[0].hecha);
        assert!(t[1].hecha && t[2].hecha, "x y X son lo mismo");
        assert_eq!(titulo(d), "Compra");
    }

    #[test]
    fn una_casilla_vacia_sigue_siendo_una_tarea() {
        // Es lo que deja pulsar intro sin escribir: tiene que poder leerse
        // para que se pueda borrar o rellenar despues.
        let t = leer_tareas("- [ ]");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].texto, "");
    }

    #[test]
    fn lo_que_no_es_una_casilla_no_se_cuela() {
        assert!(leer_tareas("- pan").is_empty(), "sin casilla no es tarea");
        assert!(leer_tareas("-[ ] pan").is_empty(), "sin espacio tampoco");
        assert!(leer_tareas("- [?] pan").is_empty(), "una marca rara no");
        assert_eq!(titulo("#sin espacio"), "", "eso no es un titulo");
    }

    #[test]
    fn la_ida_y_vuelta_conserva_lo_que_importa() {
        let d = escribir_tareas(
            "Obra",
            &[
                Tarea {
                    texto: "medir".into(),
                    hecha: false,
                },
                Tarea {
                    texto: "pedir cemento".into(),
                    hecha: true,
                },
            ],
        );
        assert!(d.starts_with("# Obra\n\n"));
        let vuelta = leer_tareas(&d);
        assert_eq!(vuelta.len(), 2);
        assert!(vuelta[1].hecha);
        assert_eq!(cuenta(&vuelta), (1, 2));
    }

    #[test]
    fn alternar_solo_cambia_la_suya() {
        let d = escribir_tareas(
            "L",
            &[
                Tarea {
                    texto: "a".into(),
                    hecha: false,
                },
                Tarea {
                    texto: "b".into(),
                    hecha: false,
                },
            ],
        );
        let d = alternar(&d, 1);
        let t = leer_tareas(&d);
        assert!(!t[0].hecha && t[1].hecha);
        // Caso negativo: una que no existe deja el documento igual.
        assert_eq!(alternar(&d, 9), d);
    }

    #[test]
    fn las_siete_nacen_con_su_documento_y_se_leen_a_la_vuelta() {
        let moneda = gastos::Moneda::euro();
        for palabra in TODAS {
            let d = documento_nuevo(palabra, "Prueba", &moneda)
                .unwrap_or_else(|| panic!("{palabra} tiene que nacer"));
            assert!(
                d.starts_with("# Prueba\n\n"),
                "{palabra} nace con su titulo: {d:?}"
            );
            assert_eq!(titulo(&d), "Prueba", "{palabra}");
            let r = resumen(palabra, &d, &moneda).unwrap_or_else(|| panic!("{palabra}"));
            assert!(r.vacia, "{palabra} nace vacia: {r:?}");
        }
    }

    #[test]
    fn una_mini_app_de_una_version_futura_no_tumba_nada() {
        // `None` y no un fallo: un mensaje escrito por un movil mas nuevo
        // tiene que seguir en la lista como texto, no reventar la ventana.
        let moneda = gastos::Moneda::euro();
        assert_eq!(documento_nuevo("horoscopo", "X", &moneda), None);
        assert_eq!(resumen("horoscopo", "lo que sea", &moneda), None);
        assert_eq!(resumen("", "", &moneda), None);
        // Y la hoja de calculo del PC, que Android no conoce, tampoco entra
        // aqui: la suya la lleva `tabla.rs`.
        assert_eq!(resumen("tabla", "{}", &moneda), None);
    }

    #[test]
    fn el_cuerpo_deja_fuera_el_titulo_y_nada_mas() {
        assert_eq!(cuerpo("# T\n\n- valor: 3"), "\n- valor: 3");
        assert_eq!(cuerpo("- valor: 3"), "- valor: 3", "sin titulo, todo");
        assert_eq!(cuerpo("# T"), "", "solo el titulo no deja cuerpo");
        assert_eq!(cuerpo(""), "");
    }

    #[test]
    fn el_resumen_de_tareas_no_ensena_avance_cuando_no_hay_nada_que_hacer() {
        let r = resumen_tareas("# L\n\n- [x] a\n- [ ] b\n- [ ] c");
        assert_eq!(r.texto, "1 de 3");
        assert_eq!((r.hechas, r.de), (1, 3));
        assert_eq!(r.avance, Some(1.0 / 3.0));
        assert!(!r.vacia);
        let vacio = resumen_tareas("# L\n\n");
        assert_eq!(vacio.texto, "0 de 0");
        assert_eq!(vacio.avance, None, "0 de 0 no es «nada hecho»");
        assert!(vacio.vacia);
    }

    #[test]
    fn un_parrafo_pegado_en_una_tarea_no_parte_el_documento() {
        let d = anadir("# L\n\n", "una cosa\ny otra\r\ny otra mas");
        assert_eq!(leer_tareas(&d).len(), 1, "sigue siendo UNA tarea: {d:?}");
        assert_eq!(leer_tareas(&d)[0].texto, "una cosa y otra y otra mas");
        // Y lo vacio no entra.
        assert_eq!(anadir(&d, "   "), d);
    }

    #[test]
    fn pegar_una_casilla_de_otra_lista_no_la_deja_tachada() {
        // El accidente de `Tareas.saneado`: sin quitar la marca, la tarea
        // «- [x] pan» se releeria como una tarea hecha llamada «pan».
        assert_eq!(
            anadir("# L\n\n", "- [x] comprar pan"),
            "# L\n\n- [ ] comprar pan"
        );
        assert_eq!(saneado("1. medir"), "medir");
        assert_eq!(saneado("* - [ ] doble"), "doble", "se quitan encadenadas");
        assert_eq!(saneado("3) llamar"), "llamar");
        // Casos negativos: sin blanco detras no es un marcador.
        assert_eq!(saneado("-5 cajas"), "-5 cajas");
        assert_eq!(saneado("12.50 euros"), "12.50 euros");
        assert_eq!(saneado("[x] suelta"), "[x] suelta");
        // Y el tope: muchas marcas encadenadas no cuelgan nada ni lo borran.
        let mucho = "- ".repeat(50_000) + "fin";
        assert!(saneado(&mucho).ends_with("fin"));
    }

    #[test]
    fn una_tarea_se_renombra_y_se_mueve_sin_perder_su_marca() {
        let d = "# L\n\n- [x] a\n- [ ] b\n- [ ] c";
        assert_eq!(
            renombrar(d, 0, "  - aa  "),
            "# L\n\n- [x] aa\n- [ ] b\n- [ ] c"
        );
        // Vaciarla no la borra ni la deja en blanco.
        assert_eq!(renombrar(d, 1, "   "), d);
        assert_eq!(renombrar(d, 9, "x"), d, "una que no existe");
        assert_eq!(
            mover(d, 0, 99),
            "# L\n\n- [ ] b\n- [ ] c\n- [x] a",
            "pasarse del final es al final"
        );
        assert_eq!(mover(d, 5, 0), d);
        assert_eq!(mover(d, 1, 1), d);
    }

    #[test]
    fn el_titulo_es_el_de_markdown_y_nada_mas() {
        assert_eq!(titulo("###### Seis\n"), "Seis");
        assert_eq!(titulo("#\tTab"), "Tab", "el tabulador tambien separa");
        assert_eq!(titulo("####### Siete"), "", "siete ya no es un titulo");
        assert_eq!(titulo("#"), "");
        assert_eq!(titulo("# "), "", "recortado no queda blanco detras");
    }

    #[test]
    fn una_clave_desconocida_vuelve_con_sus_mayusculas() {
        let v = valores("# T\n\n- ritmoBPM: 3\n- Valor: 7");
        assert_eq!(valor(&v, "valor"), Some("7"), "se compara sin mayusculas");
        assert_eq!(
            otras_claves(&v, &["valor"]),
            vec![("ritmoBPM".to_string(), "3".to_string())]
        );
    }

    #[test]
    fn renombrar_cambia_solo_la_cabecera_de_las_siete() {
        let moneda = gastos::Moneda::euro();
        for palabra in TODAS {
            let d = documento_nuevo(palabra, "Viejo", &moneda).unwrap();
            let n = con_titulo(palabra, &d, "Nuevo\nnombre", &moneda).unwrap();
            assert_eq!(titulo(&n), "Nuevo nombre", "{palabra}: {n:?}");
            assert_eq!(cuerpo(&n), cuerpo(&d), "{palabra}: el cuerpo no se toca");
            // Dos veces seguidas no acumula renglones en blanco.
            let dos = con_titulo(palabra, &n, "Otro", &moneda).unwrap();
            assert_eq!(cuerpo(&dos), cuerpo(&d), "{palabra}");
        }
        assert_eq!(con_titulo("horoscopo", "x", "y", &moneda), None);
    }
}
