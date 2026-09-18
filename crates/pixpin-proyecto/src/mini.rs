//! Las mini-aplicaciones del cuaderno: documentos que viajan como texto.
//!
//! En Android son siete (`mini/MiniApps.kt`): tareas, gastos, cronometro,
//! temporizador, contador, ruleta y alarma. Aqui se empieza por **tareas**,
//! que es la que se usa a diario en una obra, y se anaden las demas segun
//! hagan falta.
//!
//! **El documento ES el texto del mensaje.** No hay fichero aparte: un
//! mensaje de clase `MINIAPP` lleva el documento entero en `texto` y la
//! palabra de su tipo en `miniapp`. Esa palabra es un dato guardado y no se
//! renombra: cambiarla dejaria a las mini-apps ya escritas sin dueno.
//!
//! El formato es Markdown corriente —un titulo con `#` y casillas de
//! GitHub—, y eso no es casualidad: un aparato que no conozca la mini-app
//! ensena el texto tal cual y se entiende igual.

/// La palabra que va en `Mensaje.miniapp` para una lista de tareas.
pub const TAREAS: &str = "tareas";

/// Una linea de la lista: lo que hay que hacer, y si ya esta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tarea {
    pub texto: String,
    pub hecha: bool,
}

/// El titulo del documento, o vacio si no empieza por uno.
pub fn titulo(documento: &str) -> String {
    let primera = documento.lines().next().unwrap_or("").trim();
    let sin = primera.trim_start_matches('#');
    // `#` sin espacio detras no es un titulo de Markdown, es texto.
    if sin.len() == primera.len() || !sin.starts_with(' ') {
        return String::new();
    }
    sin.trim().to_string()
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
pub fn anadir(documento: &str, texto: &str) -> String {
    let limpio = en_una_linea(texto);
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

/// Cuantas hechas de cuantas: lo que se ensena en la burbuja.
pub fn cuenta(tareas: &[Tarea]) -> (usize, usize) {
    (tareas.iter().filter(|t| t.hecha).count(), tareas.len())
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
    fn un_parrafo_pegado_en_una_tarea_no_parte_el_documento() {
        let d = anadir("# L\n\n", "una cosa\ny otra\r\ny otra mas");
        assert_eq!(leer_tareas(&d).len(), 1, "sigue siendo UNA tarea: {d:?}");
        assert_eq!(leer_tareas(&d)[0].texto, "una cosa y otra y otra mas");
        // Y lo vacio no entra.
        assert_eq!(anadir(&d, "   "), d);
    }
}
