//! **Banco de pruebas del asistente con el modelo de verdad**: ~30 pedidos
//! tipicos con su comprobacion automatica, para medir el % de acierto y ver
//! si un cambio en las herramientas, sus descripciones o las instrucciones lo
//! mejora o lo empeora.
//!
//! No corre con `cargo test` normal: necesita el motor y el modelo bajados.
//! Sin variables, los busca donde los deja la app
//! (`%LOCALAPPDATA%\PixPinMax\modelos`).
//!
//! ```text
//! PIXPIN_IA_MOTOR=<carpeta con llama-server.exe>   PIXPIN_IA_MODELO=<.gguf>
//! PIXPIN_IA_VISION=<mmproj .gguf>   PIXPIN_IA_DISPOSITIVO=cpu|auto|Vulkan1
//! PIXPIN_IA_SOLO=<texto>  (solo los casos cuyo nombre lo contiene)
//! PIXPIN_IA_MOTOR=local|gemma|claude|deepseek|grabado  (uno, varios
//!   separados por comas, o `todos` = los cuatro primeros; con la nube: la
//!   clave de ANTHROPIC_API_KEY / DEEPSEEK_API_KEY, o la guardada en
//!   Ajustes, y sin clave se SALTA; `gemma` es el de mejor vista, Gemma 4
//!   E2B, bajado por la app; para el local, una carpeta en PIXPIN_IA_MOTOR
//!   sigue valiendo como antes)
//! cargo test -p pixpin-ia --test banco -- --ignored --nocapture
//! ```
//!
//! Con varios motores se pasan uno detras de otro (un solo `llama-server`
//! cargado a la vez) y al final sale una **tabla comparativa** en Markdown:
//! cada caso con «✓/✗ y segundos» por motor, el acierto y el tiempo total.
//!
//! **Modo grabado** (evaluar a Haiku sin API, con un subagente como cerebro):
//! `volcar_para_haiku` escribe lo que la app manda a Claude (`sistema.txt`,
//! `herramientas.json`) y los casos (`casos.json` + PNG); el «cerebro» contesta
//! en un `respuestas.json` (`{"<caso>": {"texto": "...", "llamadas":
//! [{"nombre": "...", "entrada": {...}}]}}`) y el banco lo pasa por la misma
//! validacion y ejecucion que a los otros motores:
//!
//! ```text
//! PIXPIN_IA_VOLCADO=<carpeta>  (opcional; por omision la del scratchpad)
//! cargo test -p pixpin-ia --test banco -- --ignored --nocapture volcar_para_haiku
//! PIXPIN_IA_MOTOR=grabado PIXPIN_IA_GRABADO=<respuestas.json>
//! cargo test -p pixpin-ia --test banco -- --ignored --nocapture --exact banco
//! ```
//!
//! Nada se ejecuta de verdad: lo que el modelo pide se APUNTA (los pedidos a
//! la app nunca se mandan) y se comprueba lo apuntado.
//!
//! **Las imagenes, para calificarlas a ojo**: de cada caso que dibuja se
//! guardan la entrada (la foto adjunta, o el pedido escrito) y lo dibujado,
//! pintado por el mismo camino que el PDF del lienzo, en
//! `PIXPIN_IA_IMAGENES` (por omision `scratchpad\banco-imagenes`)
//! `\<motor>\<caso>-{entrada,resultado}.png`. Y se mide cuanto tarda en
//! verse algo (la primera letra en el chat y lo primero en el lienzo) y el
//! total. Los casos del lienzo nuevo se eligen con `PIXPIN_IA_SOLO=lienzo-`.

use std::path::PathBuf;
use std::time::Instant;

use serde_json::Value;

use pixpin_ia::agente::{self, Ejecutor, Evento};
use pixpin_ia::herramientas::Llamada;
use pixpin_ia::lienzo;
use pixpin_ia::motor::{Mensaje, Motor, Seguir};
use pixpin_ia::servidor::{Config, Dispositivo, MotorLlama, elegir_dispositivo, listar_dispositivos};

/// Lo que paso en un caso.
#[derive(Default, Debug)]
struct Resultado {
    /// (herramienta, lo que pidio en limpio).
    llamadas: Vec<(String, Value)>,
    /// Los elementos de Excalidraw que acabaron en el lienzo.
    elementos: Vec<Value>,
    texto: String,
    /// Segundos hasta la primera letra en el chat y hasta lo primero en el
    /// lienzo.
    t_texto: Option<f32>,
    t_dibujo: Option<f32>,
}

impl Resultado {
    fn llamo(&self, nombre: &str) -> Option<&Value> {
        self.llamadas.iter().find(|(n, _)| n == nombre).map(|(_, v)| v)
    }
    fn tipos(&self, tipo: &str) -> Vec<&Value> {
        self.elementos.iter().filter(|e| e["type"] == tipo).collect()
    }
    fn textos(&self) -> String {
        self.tipos("text")
            .iter()
            .filter_map(|e| e["text"].as_str())
            .collect::<Vec<_>>()
            .join(" | ")
            .to_lowercase()
    }
    fn colores(&self) -> Vec<(u8, u8, u8)> {
        self.elementos
            .iter()
            .flat_map(|e| [e["strokeColor"].as_str(), e["backgroundColor"].as_str()])
            .flatten()
            .filter_map(rgb)
            .collect()
    }
    fn con_color(&self, que: fn((u8, u8, u8)) -> bool) -> bool {
        self.colores().into_iter().any(que)
    }
    /// Elementos con al menos `n` puntos (polilineas, poligonos, trazos).
    fn con_puntos(&self, n: usize) -> usize {
        self.elementos
            .iter()
            .filter(|e| e["points"].as_array().is_some_and(|p| p.len() >= n))
            .count()
    }
    /// La respuesta final, en minusculas y sin tildes.
    fn todo_el_texto(&self) -> String {
        // Solo lo que lee el usuario: lo que devolvio ayuda_app no cuenta si
        // el modelo no lo dice.
        sin_tildes(&self.texto.to_lowercase())
    }
    /// Elementos con ese `material` (`cuadritos` es el grafito).
    fn con_material(&self, material: &str) -> usize {
        self.elementos.iter().filter(|e| e["material"] == material).count()
    }
    fn llamo_alguna(&self, nombres: &[&str]) -> bool {
        nombres.iter().any(|n| self.llamo(n).is_some())
    }
    fn pedido(&self, accion: &str) -> Option<&Value> {
        self.llamadas
            .iter()
            .map(|(_, v)| v)
            .find(|v| v.get("accion").and_then(Value::as_str) == Some(accion))
    }
}

fn sin_tildes(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            otro => otro,
        })
        .collect()
}

fn rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let h = hex.strip_prefix('#')?;
    if h.len() < 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some((p(0)?, p(2)?, p(4)?))
}

fn rojo((r, g, b): (u8, u8, u8)) -> bool {
    r > 150 && g < 110 && b < 110
}
fn azul((r, _g, b): (u8, u8, u8)) -> bool {
    b > 140 && r < 110
}
fn verde((r, g, b): (u8, u8, u8)) -> bool {
    g > 110 && r < 110 && b < 130
}
fn amarillo((r, g, b): (u8, u8, u8)) -> bool {
    r > 190 && g > 110 && b < 90
}

type Comprobar = fn(&Resultado) -> Result<(), String>;

struct Caso {
    nombre: &'static str,
    pedido: &'static str,
    imagen: Option<fn() -> Vec<u8>>,
    comprobar: Comprobar,
}

fn si(c: bool, motivo: &str) -> Result<(), String> {
    if c { Ok(()) } else { Err(motivo.to_string()) }
}

fn la_de_prueba() -> Vec<u8> {
    std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/medir/prueba.png")).unwrap()
}

fn la_casa() -> Vec<u8> {
    imagenes::casa_png()
}

fn la_persona() -> Vec<u8> {
    imagenes::persona_png()
}

/// La foto del perrito (fuera del repositorio: el caso `copiar-perrito` solo
/// entra si esta).
fn ruta_perrito() -> PathBuf {
    std::env::var("PIXPIN_IA_PERRITO").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from(
            r"C:\Users\MaxBook\AppData\Local\Temp\claude\F--THE-FORGE-PIXPIN-PC-VERSION-MAX\afe461c5-d9fa-4287-b8b5-3ab7961f6cb3\scratchpad\perrito\perrito.png",
        )
    })
}

/// El emoji del perrito: el de verdad si esta, y si no uno de mentira con
/// los mismos degradados (el caso `calcar-perrito-emoji` corre siempre).
fn el_perrito() -> Vec<u8> {
    std::fs::read(ruta_perrito()).unwrap_or_else(|_| imagenes::perrito_png())
}

fn casos() -> Vec<Caso> {
    let mut lista = casos_fijos();
    if ruta_perrito().exists() {
        lista.push(Caso {
            nombre: "copiar-perrito",
            pedido: "Copia este perrito en el lienzo.",
            imagen: Some(el_perrito),
            comprobar: |r| {
                r.llamo(pixpin_ia::herramientas::CALCAR).ok_or("no llamo a calcar_imagen")?;
                si(r.elementos.len() >= 3, "menos de 3 elementos calcados")
            },
        });
    }
    lista
}

fn casos_fijos() -> Vec<Caso> {
    let mut v = casos_del_lienzo();
    v.extend(casos_de_antes());
    v
}

/// **Los casos de las herramientas nuevas del lienzo** (14-oct): todas las
/// herramientas, SVG, ecuaciones, funciones, cronogramas e interpretar fotos.
fn casos_del_lienzo() -> Vec<Caso> {
    use pixpin_ia::herramientas::{CRONOGRAMA, DIBUJAR, ECUACION, FUNCION, SVG};
    vec![
        Caso {
            nombre: "lienzo-persona-con-figuras",
            pedido: "Dibuja esta persona con figuras en el lienzo (interpretala, no la calques).",
            imagen: Some(la_persona),
            comprobar: |r| {
                si(r.llamo(pixpin_ia::herramientas::CALCAR).is_none(), "la calco en vez de interpretarla")?;
                si(r.llamo_alguna(&[DIBUJAR, SVG]), "no dibujo")?;
                si(r.elementos.len() >= 5, "menos de 5 figuras")
            },
        },
        Caso {
            nombre: "lienzo-perro-a-grafito",
            pedido: "Dibuja este perro a lapiz, con grafito 6B.",
            imagen: Some(el_perrito),
            comprobar: |r| {
                si(r.llamo(pixpin_ia::herramientas::CALCAR).is_none(), "lo calco en vez de dibujarlo")?;
                si(r.elementos.len() >= 4, "menos de 4 trazos")?;
                si(r.con_material("cuadritos") >= 2, "sin trazos de grafito")
            },
        },
        Caso {
            nombre: "lienzo-arbol-isometrico",
            pedido: "Dibuja un arbol en isometrico para arquitectura, en tres posiciones distintas.",
            imagen: None,
            comprobar: |r| {
                si(r.llamo_alguna(&[SVG, DIBUJAR]), "no dibujo")?;
                si(r.elementos.len() >= 9, "menos de 9 figuras para tres arboles")?;
                si(r.con_color(verde), "sin verde de copa")
            },
        },
        Caso {
            nombre: "lienzo-ecuacion",
            pedido: "Escribe en el lienzo la formula general de la ecuacion de segundo grado, bien compuesta.",
            imagen: None,
            comprobar: |r| {
                r.llamo(ECUACION).ok_or("no llamo a escribir_ecuacion")?;
                let t = r.textos();
                si(t.contains('±') || t.contains("+/-"), "sin ±")?;
                si(r.tipos("line").len() >= 2, "sin raya de fraccion ni raiz")
            },
        },
        Caso {
            nombre: "lienzo-funciones",
            pedido: "Grafica en el lienzo y = x^2 - 2 y y = 2 sin(x) entre x = -3 y x = 3.",
            imagen: None,
            comprobar: |r| {
                r.llamo(FUNCION).ok_or("no llamo a graficar_funcion")?;
                si(!r.tipos("pixpin-axes").is_empty(), "sin plano cartesiano")?;
                si(r.con_puntos(50) >= 2, "menos de dos curvas")
            },
        },
        Caso {
            nombre: "lienzo-cronograma",
            pedido: "Hazme un cronograma de una obra pequena: excavacion del 2 al 6 de noviembre de 2026, cimientos del 9 al 20 de noviembre despues de la excavacion, muros del 23 de noviembre al 4 de diciembre despues de los cimientos, y el hito de entrega el 4 de diciembre.",
            imagen: None,
            comprobar: |r| {
                r.llamo(CRONOGRAMA).ok_or("no llamo a cronograma")?;
                si(!r.tipos("diamond").is_empty(), "sin el hito")?;
                si(r.tipos("arrow").len() >= 2, "sin dependencias")?;
                si(r.textos().contains("cimientos"), "sin el nombre de las tareas")
            },
        },
        Caso {
            nombre: "lienzo-plano-con-tabla",
            pedido: "Dibuja un plano cartesiano con los puntos (1, 2), (-3, 1) y (2, -2) en su tabla de coordenadas.",
            imagen: None,
            comprobar: |r| {
                si(!r.tipos("pixpin-axes").is_empty(), "sin plano cartesiano")?;
                si(r.tipos("pixpin-point").len() >= 3, "faltan los puntos")
            },
        },
        Caso {
            nombre: "lienzo-grafito-durezas",
            pedido: "Dibuja tres trazos ondulados a mano alzada con grafito: uno 4H, uno HB y uno 8B, uno debajo de otro.",
            imagen: None,
            comprobar: |r| {
                si(r.tipos("freedraw").len() >= 3, "menos de 3 trazos a mano alzada")?;
                si(r.con_material("cuadritos") >= 3, "no son de grafito")?;
                let d: Vec<&str> = r.elementos.iter().filter_map(|e| e["dureza"].as_str()).collect();
                si(d.contains(&"4H") && d.contains(&"8B"), "sin las durezas")
            },
        },
    ]
}

fn casos_de_antes() -> Vec<Caso> {
    vec![
        // --- Dibujar -------------------------------------------------------
        Caso {
            nombre: "dibujo-circulo-y-cuadrado",
            pedido: "Dibuja un circulo rojo a la izquierda y un cuadrado azul a la derecha.",
            imagen: None,
            comprobar: |r| {
                si(!r.tipos("ellipse").is_empty(), "sin elipse")?;
                si(!r.tipos("rectangle").is_empty(), "sin rectangulo")?;
                si(r.con_color(rojo) && r.con_color(azul), "faltan los colores")
            },
        },
        Caso {
            nombre: "dibujo-triangulo",
            pedido: "Dibuja un triangulo verde en el centro del lienzo.",
            imagen: None,
            comprobar: |r| {
                let poligono = r.con_puntos(3) > 0;
                let tres_lineas = r.tipos("line").len() >= 3;
                si(poligono || tres_lineas, "no hay triangulo")?;
                si(r.con_color(verde), "no es verde")
            },
        },
        Caso {
            nombre: "dibujo-casa",
            pedido: "Dibuja una casa: un cuadrado, un techo triangular encima y una puerta.",
            imagen: None,
            comprobar: |r| {
                si(r.elementos.len() >= 3, "menos de 3 figuras")?;
                si(!r.tipos("rectangle").is_empty(), "sin rectangulo")?;
                si(r.con_puntos(3) > 0 || r.tipos("line").len() >= 2, "sin techo")
            },
        },
        Caso {
            nombre: "dibujo-diagrama-de-flujo",
            pedido: "Haz un diagrama de flujo con tres cajas: Inicio, Proceso y Fin, unidas con flechas.",
            imagen: None,
            comprobar: |r| {
                let t = r.textos();
                si(t.contains("inicio") && t.contains("proceso") && t.contains("fin"), "faltan rotulos")?;
                si(r.tipos("arrow").len() >= 2, "menos de 2 flechas")
            },
        },
        Caso {
            nombre: "dibujo-titulo",
            pedido: "Escribe el titulo «Plan 2026» en grande arriba del lienzo.",
            imagen: None,
            comprobar: |r| {
                let t = r.tipos("text");
                let bien = t.iter().any(|e| {
                    e["text"].as_str().is_some_and(|s| s.contains("Plan 2026"))
                        && e["y"].as_f64().is_some_and(|y| y < 200.0)
                });
                si(bien, "sin el titulo arriba")
            },
        },
        Caso {
            nombre: "dibujo-estrella",
            pedido: "Dibuja una estrella de cinco puntas amarilla.",
            imagen: None,
            comprobar: |r| {
                let poligono = r.con_puntos(10) > 0;
                let lineas = r.tipos("line").len() >= 5;
                si(poligono || lineas, "no hay estrella")?;
                si(r.con_color(amarillo), "no es amarilla")
            },
        },
        Caso {
            nombre: "dibujo-onda",
            pedido: "Dibuja una curva suave en forma de onda, de color azul, de izquierda a derecha.",
            imagen: None,
            comprobar: |r| {
                si(r.con_puntos(4) > 0, "no hay curva de varios puntos")?;
                si(r.con_color(azul), "no es azul")
            },
        },
        Caso {
            nombre: "dibujo-flecha-con-texto",
            pedido: "Dibuja una flecha de izquierda a derecha con el texto «paso 1».",
            imagen: None,
            comprobar: |r| {
                si(!r.tipos("arrow").is_empty(), "sin flecha")?;
                si(r.textos().contains("paso 1"), "sin el texto")
            },
        },
        Caso {
            nombre: "dibujo-muneco",
            pedido: "Dibuja un muñeco de palitos: cabeza redonda, cuerpo, brazos y piernas.",
            imagen: None,
            comprobar: |r| {
                si(!r.tipos("ellipse").is_empty(), "sin cabeza")?;
                si(r.elementos.len() >= 4, "menos de 4 trazos")
            },
        },
        // --- Copiar una imagen ----------------------------------------------
        Caso {
            nombre: "calcar-copiar-imagen",
            pedido: "Copia esta imagen en el lienzo.",
            imagen: Some(la_de_prueba),
            comprobar: |r| {
                si(r.elementos.len() >= 3, "menos de 3 elementos")?;
                si(r.con_color(rojo) && r.con_color(azul), "sin el rojo y el azul de la foto")
            },
        },
        Caso {
            nombre: "calcar-solo-contornos",
            pedido: "Calca solo los contornos de esta imagen en el lienzo.",
            imagen: Some(la_de_prueba),
            comprobar: |r| {
                // El circulo calcado es una curva de muchos puntos.
                si(r.con_puntos(8) >= 2, "no hay contornos calcados")
            },
        },
        Caso {
            nombre: "calcar-dibujo-casa",
            pedido: "Pasa este dibujo al lienzo con sus colores.",
            imagen: Some(la_casa),
            comprobar: |r| {
                si(r.elementos.len() >= 4, "menos de 4 elementos")?;
                si(r.con_color(rojo), "falta el tejado rojo")
            },
        },
        Caso {
            // Medido el 14-oct por el coordinador: con este pedido y el emoji
            // del perrito, el 2B dibujaba 4 triangulos en vez de calcarlo.
            nombre: "calcar-perrito-emoji",
            pedido: "Dibuja este perrito de la imagen adjunta dentro del lienzo.",
            imagen: Some(el_perrito),
            comprobar: |r| {
                si(r.llamo("calcar_imagen").is_some(), "no lo calco")?;
                si(r.llamo("dibujar_en_lienzo").is_none(), "lo dibujo a mano")?;
                si(r.elementos.len() >= 4, "menos de 4 elementos")?;
                si(r.con_color(|(r, g, b)| r > 180 && g < 110 && b > 90), "falta la lengua rosa")
            },
        },
        // --- Graficas -------------------------------------------------------
        Caso {
            nombre: "grafica-barras",
            pedido: "Hazme una grafica de barras de ventas: enero 120, febrero 95, marzo 140.",
            imagen: None,
            comprobar: |r| {
                let g = r.llamo("grafica").ok_or("no llamo a grafica")?;
                si(g["tipo"] == "barras", "no es de barras")?;
                si(g["valores"] == serde_json::json!([120.0, 95.0, 140.0]), "valores mal")
            },
        },
        Caso {
            nombre: "grafica-pastel",
            pedido: "Grafica de pastel de la encuesta: si 60, no 40.",
            imagen: None,
            comprobar: |r| {
                let g = r.llamo("grafica").ok_or("no llamo a grafica")?;
                si(g["tipo"] == "pastel", "no es de pastel")
            },
        },
        Caso {
            nombre: "grafica-lineas",
            pedido: "Dibuja una grafica de lineas de la temperatura: lunes 20, martes 22, miercoles 19, jueves 25.",
            imagen: None,
            comprobar: |r| {
                let g = r.llamo("grafica").ok_or("no llamo a grafica")?;
                si(g["tipo"] == "lineas", "no es de lineas")?;
                si(g["valores"].as_array().is_some_and(|v| v.len() == 4), "no son 4 valores")
            },
        },
        // --- La app ---------------------------------------------------------
        Caso {
            nombre: "app-nota-nueva",
            pedido: "Crea una nota nueva que diga: comprar cemento y arena.",
            imagen: None,
            comprobar: |r| {
                let p = r.pedido("nota_nueva").ok_or("no pidio nota_nueva")?;
                si(p["texto"].as_str().is_some_and(|t| t.contains("cemento")), "sin el texto")
            },
        },
        Caso {
            nombre: "app-anadir-tarea",
            pedido: "Apunta la tarea: llamar al ingeniero mañana.",
            imagen: None,
            comprobar: |r| {
                let p = r.pedido("anadir_tarea").ok_or("no pidio anadir_tarea")?;
                si(p["texto"].as_str().is_some_and(|t| t.contains("ingeniero")), "sin el texto")
            },
        },
        Caso {
            nombre: "app-lista-nueva",
            pedido: "Crea una lista de tareas que se llame Obra.",
            imagen: None,
            comprobar: |r| {
                let p = r.pedido("lista_nueva").ok_or("no pidio lista_nueva")?;
                si(p["titulo"].as_str().is_some_and(|t| t.contains("Obra")), "sin el titulo")
            },
        },
        Caso {
            nombre: "app-ventana-tareas",
            pedido: "Abre la ventana de tareas.",
            imagen: None,
            comprobar: |r| {
                let p = r.pedido("ventana").ok_or("no pidio ventana")?;
                si(p["cual"] == "tareas", "otra ventana")
            },
        },
        Caso {
            nombre: "app-galeria",
            pedido: "Enseñame la galeria de capturas.",
            imagen: None,
            comprobar: |r| {
                let p = r.pedido("ventana").ok_or("no pidio ventana")?;
                si(p["cual"] == "galeria", "otra ventana")
            },
        },
        Caso {
            nombre: "app-capturar",
            pedido: "Haz una captura de una zona de la pantalla.",
            imagen: None,
            comprobar: |r| si(r.pedido("capturar").is_some(), "no pidio capturar"),
        },
        Caso {
            nombre: "app-pinear-ultima",
            pedido: "Saca como pin la ultima captura.",
            imagen: None,
            comprobar: |r| si(r.pedido("pinear_ultima").is_some(), "no pidio pinear_ultima"),
        },
        Caso {
            nombre: "app-lienzo-nuevo",
            pedido: "Crea un lienzo nuevo llamado Croquis.",
            imagen: None,
            comprobar: |r| {
                let p = r.pedido("lienzo_nuevo").ok_or("no pidio lienzo_nuevo")?;
                si(p["nombre"].as_str().is_some_and(|t| t.contains("Croquis")), "sin el nombre")
            },
        },
        Caso {
            nombre: "app-grabar",
            pedido: "Graba una nota de voz que se llame reunion.",
            imagen: None,
            comprobar: |r| {
                let p = r.pedido("grabar").ok_or("no pidio grabar")?;
                si(p["nombre"].as_str().is_some_and(|t| t.to_lowercase().contains("reunion")), "sin el nombre")
            },
        },
        Caso {
            nombre: "app-abrir-fichero",
            pedido: r"Abre el archivo C:\Users\yo\Desktop\plano.pdf",
            imagen: None,
            comprobar: |r| {
                let p = r.pedido("abrir").ok_or("no pidio abrir")?;
                si(p["que"]["tipo"] == "fichero", "no es un fichero")?;
                si(p["que"]["ruta"].as_str().is_some_and(|t| t.ends_with("plano.pdf")), "otra ruta")
            },
        },
        Caso {
            nombre: "app-lecciones",
            pedido: "Busca en mis lecciones aprendidas las de encofrado.",
            imagen: None,
            comprobar: |r| {
                let p = r.pedido("lecciones").ok_or("no pidio lecciones")?;
                si(p["consulta"].as_str().is_some_and(|t| t.contains("encofrado")), "sin la consulta")
            },
        },
        Caso {
            nombre: "app-borrar-lista-pide-permiso",
            pedido: "Borra la lista de tareas con codigo m-123.",
            imagen: None,
            comprobar: |r| {
                // Lo que borra no puede salir sin preguntar.
                match r.pedido("borrar_lista") {
                    Some(p) => si(p["__confirmar"] == true, "borro sin preguntar"),
                    None => Ok(()),
                }
            },
        },
        // --- Preguntas sobre la app ----------------------------------------
        Caso {
            nombre: "ayuda-ajustes",
            pedido: "¿Donde estan los ajustes de PixPin?",
            imagen: None,
            comprobar: |r| si(r.todo_el_texto().contains("bandeja"), "no dice que estan en la bandeja"),
        },
        Caso {
            nombre: "ayuda-como-capturar",
            pedido: "¿Como hago una captura de pantalla con PixPin?",
            imagen: None,
            comprobar: |r| {
                si(r.pedido("capturar").is_none(), "capturo en vez de explicar")?;
                let t = r.todo_el_texto();
                si(t.contains("ctrl+alt+x") || t.contains("atajo"), "no dice el atajo")
            },
        },
        Caso {
            nombre: "ayuda-sincronizar",
            pedido: "¿Como paso mis cosas al movil?",
            imagen: None,
            comprobar: |r| si(r.todo_el_texto().contains("sincroniz"), "no habla de sincronizar"),
        },
        Caso {
            nombre: "ayuda-tareas",
            pedido: "¿Donde veo mis tareas pendientes?",
            imagen: None,
            comprobar: |r| {
                let t = r.todo_el_texto();
                let abrio = r.pedido("ventana").is_some_and(|p| p["cual"] == "tareas");
                si(abrio || t.contains("ventana de tareas") || t.contains("inbox"), "no lo dice")
            },
        },
        // --- Sin herramientas ------------------------------------------------
        Caso {
            nombre: "charla-saludo",
            pedido: "Hola, ¿que tal?",
            imagen: None,
            comprobar: |r| {
                si(r.llamadas.is_empty(), "uso una herramienta para saludar")?;
                si(!r.texto.trim().is_empty(), "no contesto")
            },
        },
    ]
}

/// Lo que hace cada herramienta en el banco: dibujar va a una lista de
/// elementos; lo demas se apunta.
struct Apunta<'a> {
    r: &'a mut Resultado,
    imagen: Option<Vec<u8>>,
    t0: Instant,
}

impl Ejecutor for Apunta<'_> {
    fn ejecutar(&mut self, llamada: &Llamada) -> Result<String, String> {
        let (nombre, limpio, elementos) = banco_ia::resumir(llamada, self.imagen.as_deref())?;
        let n = elementos.len();
        if n > 0 && self.r.t_dibujo.is_none() {
            self.r.t_dibujo = Some(self.t0.elapsed().as_secs_f32());
        }
        self.r.elementos.extend(elementos);
        let respuesta = limpio.get("respuesta").and_then(Value::as_str).map(str::to_string);
        self.r.llamadas.push((nombre, limpio));
        Ok(match respuesta {
            Some(r) => r,
            None if n > 0 => format!("hecho: {n} elementos en el lienzo"),
            None => "hecho".into(),
        })
    }
}

/// Los motores que se pueden medir, por su nombre en `PIXPIN_IA_MOTOR`.
const MOTORES: [&str; 5] = ["local", "gemma", "claude", "deepseek", "grabado"];

/// Los motores pedidos: `PIXPIN_IA_MOTOR=local,gemma` (uno o varios,
/// separados por comas), `todos`, o nada (el local). Una carpeta sigue
/// valiendo como antes (el local con ese `llama-server`).
fn pedidos() -> Vec<String> {
    let v = std::env::var("PIXPIN_IA_MOTOR").unwrap_or_default();
    let v = v.trim().to_lowercase();
    if v.is_empty() {
        return vec!["local".into()];
    }
    if v == "todos" {
        return MOTORES[..4].iter().map(|s| s.to_string()).collect();
    }
    if !v.contains(',') && !MOTORES.contains(&v.as_str()) {
        // Una carpeta con llama-server.exe: el local de siempre.
        return vec!["local".into()];
    }
    v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// **El motor `nombre`**, o por que no se puede medir (y se salta):
/// - `local`: llama.cpp con Qwen3.5-2B (o `PIXPIN_IA_MODELO`);
/// - `gemma`: llama.cpp con Gemma 4 E2B, el de mejor vista;
/// - `claude`/`deepseek`: la nube, solo con su clave en
///   `ANTHROPIC_API_KEY`/`DEEPSEEK_API_KEY` (o la guardada en Ajustes). No
///   se pide nunca: sin clave, se salta.
fn motor(nombre: &str) -> Result<Box<dyn Motor>, String> {
    use pixpin_ia::nube::{MotorNube, Proveedor, claves};
    if let Some(p) = Proveedor::por_id(nombre) {
        let var = match p {
            Proveedor::Claude => "ANTHROPIC_API_KEY",
            Proveedor::DeepSeek => "DEEPSEEK_API_KEY",
        };
        let clave = std::env::var(var)
            .ok()
            .filter(|c| !c.trim().is_empty())
            .or_else(|| claves::leer(p))
            .ok_or_else(|| format!("sin clave de {} (pon {var})", p.nombre()))?;
        println!("motor: {} ({})", p.nombre(), p.modelo());
        return Ok(Box::new(MotorNube::nuevo(p, clave)));
    }
    match nombre {
        "local" => motor_local(None).map(|m| Box::new(m) as Box<dyn Motor>),
        "gemma" => motor_local(Some(pixpin_ia::modelos::GEMMA4_E2B)).map(|m| Box::new(m) as Box<dyn Motor>),
        otro => Err(format!("motor desconocido «{otro}» (valen: {})", MOTORES.join(", "))),
    }
}

/// llama.cpp con el modelo de siempre (o `PIXPIN_IA_MODELO`), o con
/// `modelo` si se da (Gemma). `Err` si falta algo por bajar.
fn motor_local(modelo: Option<pixpin_ia::modelos::Modelo>) -> Result<MotorLlama, String> {
    let base = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap()).join("PixPinMax").join("modelos");
    let motor_dir = std::env::var("PIXPIN_IA_MOTOR")
        .ok()
        .filter(|m| PathBuf::from(m).join("llama-server.exe").is_file())
        .map(PathBuf::from)
        .unwrap_or_else(|| base.join("llama.cpp").join("b11580"));
    let (pesos, vision) = match modelo {
        Some(m) => {
            let c = pixpin_ia::descarga::carpeta_modelo(&base, &m);
            (c.join(m.pesos.nombre), m.vision.map(|v| c.join(v.nombre)))
        }
        None => (
            std::env::var("PIXPIN_IA_MODELO")
                .map(PathBuf::from)
                .unwrap_or_else(|_| base.join("qwen3.5-2b").join("Qwen3.5-2B-Q4_K_M.gguf")),
            std::env::var("PIXPIN_IA_VISION")
                .map(PathBuf::from)
                .ok()
                .or_else(|| Some(base.join("qwen3.5-2b").join("mmproj-F16.gguf"))),
        ),
    };
    let vision = vision.filter(|p| p.exists());
    let servidor = motor_dir.join("llama-server.exe");
    if !servidor.is_file() {
        return Err(format!("falta {}", servidor.display()));
    }
    if !pesos.is_file() {
        return Err(format!("falta {}", pesos.display()));
    }
    let mut config = Config::nueva(servidor.clone(), pesos.clone(), vision);
    config.dispositivo = match std::env::var("PIXPIN_IA_DISPOSITIVO").as_deref() {
        Ok("cpu") => Dispositivo::Cpu,
        Ok("auto") | Err(_) => {
            let bytes = std::fs::metadata(&pesos).map(|m| m.len()).unwrap_or(0);
            elegir_dispositivo(&listar_dispositivos(&servidor), bytes)
        }
        Ok(otro) => Dispositivo::Gpu(otro.to_string()),
    };
    println!("motor: llama.cpp con {} en {:?}", pesos.display(), config.dispositivo);
    Ok(MotorLlama::nuevo(config))
}

/// De donde salen las respuestas: un motor, o lo grabado caso por caso.
enum Fuente {
    Motor(Box<dyn Motor>),
    Grabado(Value),
}

/// Lo que dio un caso con un motor: si salio bien y cuanto tardo.
type Nota = (bool, f32);

/// **Pasa todos los casos por un motor.** Imprime cada uno al momento y
/// los fallos con su detalle al final.
fn correr(fuente: &Fuente) -> Vec<(&'static str, Nota)> {
    let solo = std::env::var("PIXPIN_IA_SOLO").ok();
    let mut notas = Vec::new();
    let mut fallos = Vec::new();
    for caso in casos() {
        if solo.as_deref().is_some_and(|s| !caso.nombre.contains(s)) {
            continue;
        }
        // Con `grabado` no hay motor comun: uno por caso, con su respuesta.
        let de_este_caso;
        let motor: &dyn Motor = match fuente {
            Fuente::Grabado(g) => match grabado::MotorGrabado::del_caso(g, caso.nombre) {
                Ok(m) => {
                    de_este_caso = m;
                    &de_este_caso
                }
                Err(motivo) => {
                    println!("MAL  {:<32}   0.0 s    <- {motivo}", caso.nombre);
                    fallos.push(format!("{}: {motivo}", caso.nombre));
                    notas.push((caso.nombre, (false, 0.0)));
                    continue;
                }
            },
            Fuente::Motor(m) => m.as_ref(),
        };
        let imagen = caso.imagen.map(|f| f());
        let (mut historial, mut herramientas) = banco_ia::preparar(caso.pedido, imagen.clone());
        // Como en la app (`asistente::turno`): la nube recibe el catalogo entero.
        if motor.catalogo_completo() {
            herramientas = pixpin_ia::herramientas::catalogo();
        }
        let mut r = Resultado::default();
        let t0 = Instant::now();
        let mut eventos = Vec::new();
        let mut invalidas = Vec::new();
        let fin = {
            let mut ej = Apunta {
                r: &mut r,
                imagen: imagen.clone(),
                t0,
            };
            let mut t_texto = None;
            let fin = agente::turno(motor, &mut historial, &herramientas, &mut ej, &mut |e| {
                if matches!(e, Evento::Texto(_)) && t_texto.is_none() {
                    t_texto = Some(t0.elapsed().as_secs_f32());
                }
                if let Evento::Reintento { motivo } = &e {
                    invalidas.push(motivo.clone());
                }
                if !matches!(e, Evento::Texto(_)) {
                    eventos.push(format!("{e:?}").chars().take(300).collect::<String>());
                }
                Seguir::Si
            });
            ej.r.t_texto = t_texto;
            fin
        };
        let t_fin = t0.elapsed().as_secs_f32();
        if !r.elementos.is_empty() {
            imagenes_del_caso(&caso, imagen.as_deref(), &r.elementos);
        }
        match fin {
            Ok(f) => r.texto = f.texto,
            Err(e) => r.texto = format!("ERROR DEL MOTOR: {e}"),
        }
        let mut veredicto = (caso.comprobar)(&r);
        // Grabado no hay segunda oportunidad: una llamada que no pasa la
        // validacion es un fallo de lo grabado, y se dice tal cual.
        if matches!(fuente, Fuente::Grabado(_)) && !invalidas.is_empty() {
            veredicto = Err(format!("entrada invalida: {}", invalidas.join(" / ")));
        }
        let ok = veredicto.is_ok();
        let segundos = t_fin;
        let visible = |t: Option<f32>| t.map_or("  -  ".to_string(), |t| format!("{t:>5.1}"));
        println!(
            "{} {:<32} {:>5.1} s (texto {} s, lienzo {} s, {} elementos)  {}{}",
            if ok { "BIEN" } else { "MAL " },
            caso.nombre,
            t_fin,
            visible(r.t_texto),
            visible(r.t_dibujo),
            r.elementos.len(),
            r.llamadas.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(","),
            veredicto.as_ref().err().map(|m| format!("  <- {m}")).unwrap_or_default()
        );
        if !ok {
            fallos.push(format!(
                "{}: {}\n    eventos: {}\n    pidio: {}\n    texto: «{}»",
                caso.nombre,
                veredicto.unwrap_err(),
                eventos.join("\n             "),
                historial
                    .iter()
                    .flat_map(|m| m.llamadas.iter())
                    .map(|l| format!("{} {}", l.nombre, l.argumentos.chars().take(600).collect::<String>()))
                    .collect::<Vec<_>>()
                    .join("\n           "),
                r.texto.trim().chars().take(300).collect::<String>()
            ));
        }
        notas.push((caso.nombre, (ok, segundos)));
    }
    println!("\n--- fallos ---\n{}", fallos.join("\n"));
    notas
}

/// **La tabla comparativa** (Markdown): una fila por caso, una columna por
/// motor («✓ 3,2 s» / «✗ 3,2 s»), y abajo el acierto y el tiempo total.
fn tabla(motores: &[(String, Result<Vec<(&'static str, Nota)>, String>)]) -> String {
    let mut casos: Vec<&'static str> = Vec::new();
    for (_, r) in motores {
        if let Ok(v) = r {
            for (c, _) in v {
                if !casos.contains(c) {
                    casos.push(c);
                }
            }
        }
    }
    let mut s = String::new();
    s.push_str("| caso |");
    for (n, _) in motores {
        s.push_str(&format!(" {n} |"));
    }
    s.push_str("\n|---|");
    s.push_str(&"---|".repeat(motores.len()));
    s.push('\n');
    let celda = |r: &Result<Vec<(&'static str, Nota)>, String>, caso: &str| match r {
        Err(_) => "—".to_string(),
        Ok(v) => match v.iter().find(|(c, _)| *c == caso) {
            Some((_, (ok, t))) => format!("{} {:.1} s", if *ok { "✓" } else { "✗" }, t),
            None => "—".into(),
        },
    };
    for caso in &casos {
        s.push_str(&format!("| {caso} |"));
        for (_, r) in motores {
            s.push_str(&format!(" {} |", celda(r, caso)));
        }
        s.push('\n');
    }
    s.push_str("| **acierto** |");
    for (_, r) in motores {
        s.push_str(&match r {
            Ok(v) => {
                let bien = v.iter().filter(|(_, (ok, _))| *ok).count();
                format!(" **{bien}/{} = {:.0} %** |", v.len(), bien as f64 * 100.0 / v.len().max(1) as f64)
            }
            Err(e) => format!(" saltado: {e} |"),
        });
    }
    s.push_str("\n| **tiempo** |");
    for (_, r) in motores {
        s.push_str(&match r {
            Ok(v) => format!(" {:.0} s |", v.iter().map(|(_, (_, t))| t).sum::<f32>()),
            Err(_) => " — |".into(),
        });
    }
    s.push('\n');
    s
}

#[test]
#[ignore = "necesita el motor y el modelo bajados (ver la cabecera)"]
fn banco() {
    let mut resultados = Vec::new();
    for nombre in pedidos() {
        println!("\n===== {nombre} =====");
        let fuente = if nombre == "grabado" {
            grabado::respuestas().map(Fuente::Grabado).ok_or_else(|| "sin respuestas grabadas".to_string())
        } else {
            motor(&nombre).map(Fuente::Motor)
        };
        let r = fuente.map(|m| {
            let t = Instant::now();
            let notas = correr(&m);
            let bien = notas.iter().filter(|(_, (ok, _))| *ok).count();
            println!(
                "\nBANCO {nombre}: {bien}/{} = {:.0} % en {:.0} s",
                notas.len(),
                bien as f64 * 100.0 / notas.len().max(1) as f64,
                t.elapsed().as_secs_f32()
            );
            // Un solo llama-server a la vez: el siguiente motor no carga
            // hasta que este suelta su memoria.
            if let Fuente::Motor(m) = &m {
                m.soltar();
            }
            notas
        });
        if let Err(e) = &r {
            println!("SALTADO {nombre}: {e}");
        }
        resultados.push((nombre, r));
    }
    println!("\n{}", tabla(&resultados));
}

#[test]
fn la_tabla_compara_y_dice_lo_saltado() {
    let r = vec![
        ("local".to_string(), Ok(vec![("a", (true, 1.0)), ("b", (false, 2.5))])),
        ("gemma".to_string(), Ok(vec![("a", (true, 3.0)), ("b", (true, 4.0))])),
        ("claude".to_string(), Err("sin clave de Claude".to_string())),
    ];
    let t = tabla(&r);
    assert!(t.starts_with("| caso | local | gemma | claude |"), "{t}");
    assert!(t.contains("| b | ✗ 2.5 s | ✓ 4.0 s | — |"), "{t}");
    assert!(t.contains("**1/2 = 50 %**") && t.contains("**2/2 = 100 %**"), "{t}");
    assert!(t.contains("saltado: sin clave de Claude"), "{t}");
}

#[test]
fn un_motor_desconocido_o_sin_clave_se_salta_sin_romper() {
    assert!(motor("gpt-9").err().unwrap().contains("desconocido"));
    // Sin la variable ni clave guardada, la nube se salta (no se pide).
    if std::env::var("DEEPSEEK_API_KEY").is_err() && pixpin_ia::nube::claves::leer(pixpin_ia::nube::Proveedor::DeepSeek).is_none() {
        assert!(motor("deepseek").err().unwrap().contains("sin clave"));
    }
}

/// El nombre del motor para la carpeta de las imagenes.
fn nombre_del_motor() -> String {
    match std::env::var("PIXPIN_IA_MOTOR").unwrap_or_default().to_lowercase().as_str() {
        "claude" => "claude".into(),
        "deepseek" => "deepseek".into(),
        "grabado" => "grabado".into(),
        _ => "local".into(),
    }
}

fn carpeta_de_imagenes() -> PathBuf {
    std::env::var("PIXPIN_IA_IMAGENES").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from(
            r"C:\Users\MaxBook\AppData\Local\Temp\claude\F--THE-FORGE-PIXPIN-PC-VERSION-MAX\afe461c5-d9fa-4287-b8b5-3ab7961f6cb3\scratchpad\banco-imagenes",
        )
    })
}

/// **La entrada y el resultado de un caso, a PNG**, para calificarlos a ojo.
fn imagenes_del_caso(caso: &Caso, imagen: Option<&[u8]>, elementos: &[Value]) {
    let dir = carpeta_de_imagenes().join(nombre_del_motor());
    let _ = std::fs::create_dir_all(&dir);
    let entrada = dir.join(format!("{}-entrada.png", caso.nombre));
    match imagen {
        Some(b) => {
            let _ = std::fs::write(&entrada, b);
        }
        None => pintar::pedido_a_png(caso.pedido, &entrada),
    }
    pintar::elementos_a_png(elementos, &dir.join(format!("{}-resultado.png", caso.nombre)));
}

/// Pintar a PNG por el camino del PDF del lienzo (exportar → PDF de
/// vectores → pixeles de Windows): lo mismo que vera el usuario al exportar.
mod pintar {
    use std::path::Path;

    use pixpin_motor2d::Escena;
    use serde_json::Value;

    pub fn escena_a_png(escena: &Escena, ruta: &Path) {
        let hojas = pixpin_motor2d::exportar::hojas(escena, pixpin_motor2d::exportar::Alcance::Todo, &[], None);
        if hojas.is_empty() {
            return;
        }
        let blanco = pixpin_motor2d::ColorRgba::opaco(1.0, 1.0, 1.0);
        let Some(pdf) = pixpin_pdf::escribir::de_hojas(&hojas, Some(blanco), &|_| None) else { return };
        let tmp = ruta.with_extension("pdf");
        if std::fs::write(&tmp, pdf).is_err() {
            return;
        }
        let _com = pixpin_shell::ComDelHilo::iniciar();
        if let Ok(img) = pixpin_pdf::Documento::abrir(&tmp).and_then(|d| d.renderizar(0, 1100)) {
            if let Ok(png) = pixpin_codec::imagen::codificar_png(&img) {
                let _ = std::fs::write(ruta, png);
            }
        }
        let _ = std::fs::remove_file(&tmp);
    }

    pub fn elementos_a_png(elementos: &[Value], ruta: &Path) {
        let doc = pixpin_ia::lienzo::documento(elementos.to_vec());
        let Ok(lienzo) = pixpin_motor2d::excalidraw::leer(&doc) else { return };
        let escena = pixpin_motor2d::excalidraw::a_escena(&lienzo);
        escena_a_png(&escena, ruta);
    }

    /// El pedido escrito, como entrada de un caso sin foto.
    pub fn pedido_a_png(pedido: &str, ruta: &Path) {
        let mut renglones = Vec::new();
        let mut actual = String::new();
        for p in pedido.split_whitespace() {
            if actual.chars().count() + p.chars().count() > 48 {
                renglones.push(std::mem::take(&mut actual));
            }
            if !actual.is_empty() {
                actual.push(' ');
            }
            actual.push_str(p);
        }
        renglones.push(actual);
        let texto = renglones.join("\n");
        let elementos = vec![serde_json::json!({
            "id": "pedido", "type": "text", "x": 0, "y": 0, "width": 560, "height": 30 * renglones.len(),
            "text": texto, "originalText": texto, "fontSize": 22, "fontFamily": 2, "textAlign": "left",
            "verticalAlign": "top", "lineHeight": 1.25, "strokeColor": "#1e1e1e", "backgroundColor": "transparent",
            "fillStyle": "solid", "strokeWidth": 1, "roughness": 0, "opacity": 100, "seed": 1, "version": 1,
            "isDeleted": false, "groupIds": [], "angle": 0
        })];
        elementos_a_png(&elementos, ruta);
    }
}

/// **Volcado para evaluar a Haiku sin API**: lo que `nube::claude` manda de
/// verdad (sistema y herramientas, sacados de una peticion real del agente
/// con el catalogo entero) y los casos con sus imagenes.
#[test]
#[ignore = "escribe ficheros fuera del repositorio (ver la cabecera)"]
fn volcar_para_haiku() {
    use std::collections::HashMap;
    let dir = std::env::var("PIXPIN_IA_VOLCADO").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from(
            r"C:\Users\MaxBook\AppData\Local\Temp\claude\F--THE-FORGE-PIXPIN-PC-VERSION-MAX\afe461c5-d9fa-4287-b8b5-3ab7961f6cb3\scratchpad\eval-haiku",
        )
    });
    std::fs::create_dir_all(&dir).unwrap();

    // La peticion tal cual la arma `agente::turno` (topes incluidos), con el
    // catalogo entero como en la app con la nube.
    let captura = grabado::Captura::default();
    let caso = &casos()[0];
    let (mut historial, _) = banco_ia::preparar(caso.pedido, None);
    let herramientas = pixpin_ia::herramientas::catalogo();
    agente::turno(&captura, &mut historial, &herramientas, &mut grabado::NoEjecuta, &mut |_| Seguir::Si).unwrap();
    let peticion = captura.primera();
    let cuerpo = pixpin_ia::nube::claude::cuerpo(&peticion, &HashMap::new(), pixpin_ia::nube::claude::MAX_TOKENS);
    let sistema = cuerpo["system"][0]["text"].as_str().unwrap();
    std::fs::write(dir.join("sistema.txt"), sistema).unwrap();
    std::fs::write(dir.join("herramientas.json"), serde_json::to_string_pretty(&cuerpo["tools"]).unwrap()).unwrap();

    let mut lista = Vec::new();
    for caso in casos() {
        let imagen = caso.imagen.map(|f| {
            let ruta = dir.join(format!("{}.png", caso.nombre));
            std::fs::write(&ruta, f()).unwrap();
            ruta.display().to_string()
        });
        lista.push(serde_json::json!({"nombre": caso.nombre, "pedido": caso.pedido, "imagen": imagen}));
    }
    std::fs::write(dir.join("casos.json"), serde_json::to_string_pretty(&lista).unwrap()).unwrap();
    println!(
        "volcado en {}: {} casos, {} herramientas, sistema de {} caracteres",
        dir.display(),
        lista.len(),
        cuerpo["tools"].as_array().map_or(0, Vec::len),
        sistema.chars().count()
    );
}

/// El motor que contesta lo grabado por otro «cerebro», y lo que hace falta
/// para volcar una peticion sin mandarla.
mod grabado {
    use std::sync::Mutex;

    use pixpin_ia::agente::Ejecutor;
    use pixpin_ia::herramientas::Llamada;
    use pixpin_ia::motor::{Fin, LlamadaCruda, Medidas, Motor, Peticion, Respuesta, Seguir, Trozo};
    use pixpin_ia::ErrorIa;
    use serde_json::Value;

    /// `Some` con `PIXPIN_IA_MOTOR=grabado`: el JSON de `PIXPIN_IA_GRABADO`.
    pub fn respuestas() -> Option<Value> {
        if !std::env::var("PIXPIN_IA_MOTOR").is_ok_and(|m| m.eq_ignore_ascii_case("grabado")) {
            return None;
        }
        let ruta = std::env::var("PIXPIN_IA_GRABADO").expect("modo grabado sin PIXPIN_IA_GRABADO=<respuestas.json>");
        let texto = std::fs::read_to_string(&ruta).unwrap_or_else(|e| panic!("no se puede leer {ruta}: {e}"));
        let v: Value = serde_json::from_str(&texto).unwrap_or_else(|e| panic!("{ruta} no es JSON: {e}"));
        assert!(v.is_object(), "{ruta}: se espera un objeto {{\"<caso>\": {{...}}}}");
        println!("motor: grabado ({ruta})");
        Some(v)
    }

    /// Primera vuelta: las llamadas grabadas; despues: el texto grabado.
    pub struct MotorGrabado {
        texto: String,
        llamadas: Vec<LlamadaCruda>,
        vueltas: Mutex<u32>,
    }

    impl MotorGrabado {
        pub fn del_caso(todas: &Value, caso: &str) -> Result<MotorGrabado, String> {
            let r = todas.get(caso).ok_or("sin respuesta grabada para este caso")?;
            let texto = r.get("texto").and_then(Value::as_str).unwrap_or_default().to_string();
            let mut llamadas = Vec::new();
            match r.get("llamadas") {
                None | Some(Value::Null) => {}
                Some(Value::Array(a)) => {
                    for (i, l) in a.iter().enumerate() {
                        let nombre = l
                            .get("nombre")
                            .and_then(Value::as_str)
                            .ok_or(format!("respuesta grabada mal formada: la llamada {i} no tiene «nombre»"))?;
                        // Un texto se pasa tal cual (para probar JSON roto).
                        let argumentos = match l.get("entrada") {
                            Some(Value::String(s)) => s.clone(),
                            Some(v) => v.to_string(),
                            None => "{}".into(),
                        };
                        llamadas.push(LlamadaCruda {
                            id: format!("grabada-{i}"),
                            nombre: nombre.to_string(),
                            argumentos,
                        });
                    }
                }
                Some(_) => return Err("respuesta grabada mal formada: «llamadas» no es una lista".into()),
            }
            Ok(MotorGrabado {
                texto,
                llamadas,
                vueltas: Mutex::new(0),
            })
        }
    }

    impl Motor for MotorGrabado {
        fn completar(&self, _: &Peticion, al_trozo: &mut dyn FnMut(&Trozo) -> Seguir) -> Result<Respuesta, ErrorIa> {
            let mut n = self.vueltas.lock().unwrap();
            *n += 1;
            if *n == 1 && !self.llamadas.is_empty() {
                for l in &self.llamadas {
                    al_trozo(&Trozo::Llamando { nombre: l.nombre.clone() });
                }
                return Ok(Respuesta {
                    texto: String::new(),
                    pensamiento: String::new(),
                    llamadas: self.llamadas.clone(),
                    fin: Fin::Herramientas,
                    medidas: Medidas::default(),
                });
            }
            al_trozo(&Trozo::Texto(self.texto.clone()));
            Ok(Respuesta::de_texto(self.texto.clone()))
        }

        /// Como Claude: el catalogo entero.
        fn catalogo_completo(&self) -> bool {
            true
        }
    }

    /// Apunta la peticion y contesta «ok» sin llamar a nada.
    #[derive(Default)]
    pub struct Captura(Mutex<Vec<Peticion>>);

    impl Captura {
        pub fn primera(&self) -> Peticion {
            self.0.lock().unwrap().first().cloned().expect("no se pidio nada")
        }
    }

    impl Motor for Captura {
        fn completar(&self, p: &Peticion, _: &mut dyn FnMut(&Trozo) -> Seguir) -> Result<Respuesta, ErrorIa> {
            self.0.lock().unwrap().push(p.clone());
            Ok(Respuesta::de_texto("ok"))
        }
        fn catalogo_completo(&self) -> bool {
            true
        }
    }

    pub struct NoEjecuta;

    impl Ejecutor for NoEjecuta {
        fn ejecutar(&mut self, _: &Llamada) -> Result<String, String> {
            Err("no se ejecuta nada al volcar".into())
        }
    }
}

/// Lo que cambia de una version del asistente a otra: que se le manda al
/// modelo y como se apunta lo que pide.
mod banco_ia {
    use super::*;

    /// Las instrucciones de siempre (con el resumen de la app) y solo las
    /// herramientas que vienen a cuento (`seleccion::para_el_pedido`), como
    /// hace el chat.
    pub fn preparar(pedido: &str, imagen: Option<Vec<u8>>) -> (Vec<Mensaje>, Vec<pixpin_ia::Herramienta>) {
        let herramientas = pixpin_ia::seleccion::para_el_pedido(pedido, imagen.is_some());
        let usuario = match imagen {
            Some(b) => Mensaje::usuario_con_imagen(pedido, b, "image/png"),
            None => Mensaje::usuario(pedido),
        };
        (vec![Mensaje::sistema(agente::instrucciones()), usuario], herramientas)
    }

    /// (nombre, lo pedido en JSON limpio, elementos que acaban en el lienzo).
    pub fn resumir(llamada: &Llamada, imagen: Option<&[u8]>) -> Result<(String, Value, Vec<Value>), String> {
        let nombre = llamada.nombre().to_string();
        Ok(match llamada {
            Llamada::Dibujar(_) => (nombre, Value::Null, lienzo::elementos(llamada, (0.0, 0.0), 1)),
            Llamada::Svg(_) | Llamada::Funciones(_) | Llamada::Ecuacion(_) | Llamada::Cronograma(_) => {
                (nombre, Value::Null, lienzo::elementos(llamada, (0.0, 0.0), 1))
            }
            Llamada::Figuras(p) => (nombre, serde_json::json!({"figuras": format!("{p:?}")}), Vec::new()),
            Llamada::Grafica(g) => (
                nombre,
                serde_json::json!({"tipo": format!("{:?}", g.tipo).to_lowercase(), "valores": g.valores, "etiquetas": g.etiquetas}),
                lienzo::elementos(llamada, (0.0, 0.0), 1),
            ),
            Llamada::Ocr(_) => (nombre, Value::Null, Vec::new()),
            Llamada::Calcar(p) => {
                let bytes = imagen.ok_or("no hay ninguna imagen adjunta que calcar")?;
                let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?.to_rgba8();
                let c = pixpin_ia::calcar::calcar(img.as_raw(), img.width(), img.height(), p);
                let els = lienzo::elementos_de_calcado(&c, (0.0, 0.0), 1);
                (nombre, serde_json::json!({"solo_contornos": p.solo_contornos}), els)
            }
            Llamada::App(a) => {
                let mut v = a.pedido.clone();
                // Nunca se manda: se apunta, con si habria preguntado antes.
                v["__confirmar"] = Value::Bool(a.confirmar.is_some());
                (nombre, v, Vec::new())
            }
            Llamada::Ayuda(q) => (
                nombre,
                serde_json::json!({"pregunta": q, "respuesta": pixpin_ia::conocimiento::responder(q)}),
                Vec::new(),
            ),
            // El banco no tiene documentos: se apunta lo pedido.
            Llamada::Buscar(b) => (
                nombre,
                serde_json::json!({"consulta": b.consulta, "proyecto": b.proyecto}),
                Vec::new(),
            ),
        })
    }
}

/// Imagenes de prueba hechas aqui, para no depender de ficheros.
mod imagenes {
    /// Un emoji de perrito con degradados (cara, orejas, hocico, nariz y
    /// lengua) sobre blanco. 512x512.
    pub fn perrito_png() -> Vec<u8> {
        let w = 512u32;
        let mut px = vec![255u8; (w * w * 3) as usize];
        let c = w as f64 / 2.0;
        let elipse = |fx: f64, fy: f64, cx: f64, cy: f64, rx: f64, ry: f64| ((fx - cx) / rx).powi(2) + ((fy - cy) / ry).powi(2);
        for y in 0..w {
            for x in 0..w {
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                let mut col: Option<[f64; 3]> = None;
                let d = elipse(fx, fy, c, c + 20.0, 180.0, 170.0);
                if d <= 1.0 {
                    col = Some([245.0 - 40.0 * d, 195.0 - 60.0 * d, 150.0 - 60.0 * d]);
                }
                for ox in [110.0, 402.0] {
                    let e = elipse(fx, fy, ox, 190.0, 55.0, 95.0);
                    if e <= 1.0 {
                        col = Some([140.0 - 40.0 * e, 95.0 - 30.0 * e, 85.0 - 25.0 * e]);
                    }
                }
                let hocico = elipse(fx, fy, c, 350.0, 110.0, 70.0);
                if hocico <= 1.0 {
                    col = Some([250.0 - 15.0 * hocico, 215.0 - 25.0 * hocico, 175.0 - 25.0 * hocico]);
                }
                if elipse(fx, fy, c, 310.0, 24.0, 16.0) <= 1.0 {
                    col = Some([80.0, 50.0, 55.0]);
                }
                for ox in [205.0, 307.0] {
                    if elipse(fx, fy, ox, 240.0, 10.0, 20.0) <= 1.0 {
                        col = Some([70.0, 45.0, 50.0]);
                    }
                }
                let lengua = elipse(fx, fy, c, 420.0, 40.0, 50.0);
                if lengua <= 1.0 && fy > 390.0 {
                    col = Some([215.0, 45.0 + 30.0 * lengua, 125.0]);
                }
                if let Some(k) = col {
                    let i = ((y * w + x) * 3) as usize;
                    px[i..i + 3].copy_from_slice(&[k[0] as u8, k[1] as u8, k[2] as u8]);
                }
            }
        }
        png_rgb(w, w, &px)
    }

    /// **Un retrato** (para «dibuja esta persona»): fondo azul grisaceo en
    /// degradado, hombros con camisa roja, cuello, cara con sombra, pelo
    /// castano, ojos, cejas, nariz y boca. 384x512.
    pub fn persona_png() -> Vec<u8> {
        let (w, h) = (384u32, 512u32);
        let mut px = vec![255u8; (w * h * 3) as usize];
        let e = |fx: f64, fy: f64, cx: f64, cy: f64, rx: f64, ry: f64| ((fx - cx) / rx).powi(2) + ((fy - cy) / ry).powi(2);
        for y in 0..h {
            for x in 0..w {
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                let t = fy / h as f64;
                let mut c = [150.0 - 40.0 * t, 170.0 - 40.0 * t, 195.0 - 30.0 * t];
                // Hombros y camisa.
                let hombros = e(fx, fy, 192.0, 520.0, 175.0, 150.0);
                if hombros <= 1.0 {
                    c = [190.0 - 50.0 * hombros, 45.0, 50.0 + 10.0 * hombros];
                }
                // Cuello.
                if (162.0..=222.0).contains(&fx) && (300.0..=390.0).contains(&fy) {
                    c = [205.0, 150.0, 115.0];
                }
                // Orejas.
                for ox in [105.0, 279.0] {
                    if e(fx, fy, ox, 220.0, 16.0, 30.0) <= 1.0 {
                        c = [215.0, 160.0, 125.0];
                    }
                }
                // Cara, con sombra hacia el borde.
                let cara = e(fx, fy, 192.0, 215.0, 88.0, 112.0);
                if cara <= 1.0 {
                    c = [235.0 - 35.0 * cara, 185.0 - 35.0 * cara, 150.0 - 30.0 * cara];
                }
                // Pelo: un casquete por arriba de la cara.
                let pelo = e(fx, fy, 192.0, 170.0, 100.0, 92.0);
                if pelo <= 1.0 && (fy < 150.0 || (cara > 1.0 && fy < 260.0)) {
                    c = [85.0 - 20.0 * pelo, 55.0 - 15.0 * pelo, 35.0];
                }
                // Cejas, ojos, nariz y boca.
                for ox in [155.0, 229.0] {
                    if e(fx, fy, ox, 182.0, 22.0, 5.0) <= 1.0 {
                        c = [70.0, 45.0, 30.0];
                    }
                    if e(fx, fy, ox, 205.0, 16.0, 9.0) <= 1.0 {
                        c = [250.0, 250.0, 250.0];
                    }
                    if e(fx, fy, ox, 205.0, 7.0, 7.0) <= 1.0 {
                        c = [60.0, 90.0, 60.0];
                    }
                }
                if e(fx, fy, 192.0, 250.0, 9.0, 22.0) <= 1.0 && fx > 192.0 {
                    c = [200.0, 140.0, 110.0];
                }
                let boca = e(fx, fy, 192.0, 288.0, 32.0, 12.0);
                if boca <= 1.0 && fy > 286.0 {
                    c = [180.0, 70.0, 75.0];
                }
                let i = ((y * w + x) * 3) as usize;
                px[i..i + 3].copy_from_slice(&[c[0] as u8, c[1] as u8, c[2] as u8]);
            }
        }
        png_rgb(w, h, &px)
    }

    /// Una casa de dibujo infantil: cielo blanco, sol amarillo, paredes
    /// marrones, tejado rojo, puerta azul y cesped verde. 512x384.
    pub fn casa_png() -> Vec<u8> {
        let (w, h) = (512u32, 384u32);
        let mut px = vec![255u8; (w * h * 3) as usize];
        let pon = |x: i32, y: i32, c: [u8; 3], px: &mut Vec<u8>| {
            if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                let i = ((y as u32 * w + x as u32) * 3) as usize;
                px[i..i + 3].copy_from_slice(&c);
            }
        };
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                // Sol.
                if (x - 420).pow(2) + (y - 70).pow(2) < 40 * 40 {
                    pon(x, y, [250, 200, 20], &mut px);
                }
                // Cesped.
                if y > 320 {
                    pon(x, y, [40, 160, 60], &mut px);
                }
                // Paredes.
                if (120..=330).contains(&x) && (180..=320).contains(&y) {
                    pon(x, y, [150, 100, 60], &mut px);
                }
                // Tejado: triangulo de (100,180) (350,180) (225,80).
                let dentro = y <= 180 && y >= 80 && (x - 225).abs() as f32 <= (y - 80) as f32 * 1.25;
                if dentro {
                    pon(x, y, [210, 30, 30], &mut px);
                }
                // Puerta.
                if (200..=250).contains(&x) && (240..=320).contains(&y) {
                    pon(x, y, [30, 60, 200], &mut px);
                }
            }
        }
        png_rgb(w, h, &px)
    }

    /// Un PNG sin comprimir (bloques «stored» de zlib): sin dependencias.
    pub fn png_rgb(w: u32, h: u32, rgb: &[u8]) -> Vec<u8> {
        let mut crudo = Vec::new();
        for y in 0..h as usize {
            crudo.push(0u8);
            crudo.extend_from_slice(&rgb[y * w as usize * 3..(y + 1) * w as usize * 3]);
        }
        let mut z = vec![0x78, 0x01];
        for (i, trozo) in crudo.chunks(65535).enumerate() {
            let ultimo = (i + 1) * 65535 >= crudo.len();
            z.push(u8::from(ultimo));
            let n = trozo.len() as u16;
            z.extend_from_slice(&n.to_le_bytes());
            z.extend_from_slice(&(!n).to_le_bytes());
            z.extend_from_slice(trozo);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &x in &crudo {
            a = (a + x as u32) % 65521;
            b = (b + a) % 65521;
        }
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        let bloque = |tipo: &[u8; 4], datos: &[u8], png: &mut Vec<u8>| {
            png.extend_from_slice(&(datos.len() as u32).to_be_bytes());
            let mut c = Vec::with_capacity(4 + datos.len());
            c.extend_from_slice(tipo);
            c.extend_from_slice(datos);
            png.extend_from_slice(&c);
            png.extend_from_slice(&crc32(&c).to_be_bytes());
        };
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        bloque(b"IHDR", &ihdr, &mut png);
        bloque(b"IDAT", &z, &mut png);
        bloque(b"IEND", &[], &mut png);
        png
    }

    fn crc32(datos: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for &b in datos {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
}

/// **Las herramientas nuevas sin modelo**: lo que sale de llamadas escritas
/// a mano, pintado a PNG en `banco-imagenes\referencia` (para comparar con
/// lo que hace cada motor). No necesita red ni modelo.
#[test]
#[ignore = "escribe PNG fuera del repositorio"]
fn muestras_del_lienzo() {
    use pixpin_ia::herramientas::{CRONOGRAMA, DIBUJAR, ECUACION, FUNCION, SVG, catalogo, validar};
    use pixpin_ia::motor::LlamadaCruda;
    let dir = carpeta_de_imagenes().join("referencia");
    std::fs::create_dir_all(&dir).unwrap();
    let arbol = r##"<svg viewBox="0 0 300 120"><g id="arbol"><path d="M30 95 L45 103 L60 95 L45 87 Z" fill="#b0b8c0"/><path d="M42 70 L48 70 L48 98 L42 98 Z" fill="#8b5a2b"/><path d="M45 20 C70 30 75 60 60 75 C50 85 40 85 30 75 C15 60 20 30 45 20 Z" fill="#2f9e44"/><path d="M45 20 C70 30 75 60 60 75 C55 80 50 82 45 82 Z" fill="#237a35"/></g><use href="#arbol" x="100" y="5" transform="scale(1)"/><g transform="translate(200 10) scale(0.8)"><path d="M30 95 L45 103 L60 95 L45 87 Z" fill="#b0b8c0"/><rect x="42" y="60" width="6" height="38" fill="#8b5a2b"/><circle cx="45" cy="45" r="28" fill="#40c057"/></g></svg>"##;
    let casos: Vec<(&str, &str, String)> = vec![
        ("svg-arboles", SVG, serde_json::json!({"svg": arbol}).to_string()),
        ("svg-arboles-grafito", SVG, serde_json::json!({"svg": arbol, "trazo": "grafito", "dureza": "6B"}).to_string()),
        ("ecuacion", ECUACION, serde_json::json!({"ecuacion": r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a} \\ E = mc^2"}).to_string()),
        ("funciones", FUNCION, serde_json::json!({"funciones": ["x^2 - 2", "2sin(x)"], "x_desde": -3, "x_hasta": 3, "titulo": "Dos funciones"}).to_string()),
        ("cronograma", CRONOGRAMA, serde_json::json!({"titulo": "Obra", "tareas": [
            {"nombre": "Excavacion", "inicio": "2026-11-02", "fin": "2026-11-06"},
            {"nombre": "Cimientos", "inicio": "2026-11-09", "fin": "2026-11-20", "depende_de": ["Excavacion"]},
            {"nombre": "Muros", "inicio": "2026-11-23", "fin": "2026-12-04", "depende_de": ["Cimientos"]},
            {"nombre": "Entrega", "inicio": "2026-12-04", "hito": true, "depende_de": ["Muros"]}]}).to_string()),
        ("plano-con-tabla", DIBUJAR, r#"{"elementos":[{"tipo":"instrumento","ejes":2,"caja":[100,100,320,320],"tabla":[[1,2],[-3,1],[2,-2]]}]}"#.into()),
        ("grafito-durezas", DIBUJAR, r#"{"elementos":[
            {"tipo":"lapiz","puntos":[100,100,200,80,300,120,400,90,500,110],"dureza":"4H","grosor":4},
            {"tipo":"lapiz","puntos":[100,200,200,180,300,220,400,190,500,210],"dureza":"HB","grosor":4},
            {"tipo":"lapiz","puntos":[100,300,200,280,300,320,400,290,500,310],"dureza":"8B","grosor":4},
            {"tipo":"rectangulo","caja":[560,100,150,100],"relleno":"azul","relleno_estilo":"rayado","trazo":"discontinuo","capa":1},
            {"tipo":"elipse","caja":[600,150,120,120],"relleno":"rojo","opacidad":50,"capa":2}]}"#.into()),
    ];
    for (nombre, herramienta, args) in casos {
        let t0 = Instant::now();
        let v = validar(&LlamadaCruda { id: "1".into(), nombre: herramienta.into(), argumentos: args }, &catalogo())
            .unwrap_or_else(|e| panic!("{nombre}: {e}"));
        let els = lienzo::elementos(&v.llamada, (0.0, 0.0), 7);
        let t_hecho = t0.elapsed();
        pintar::elementos_a_png(&els, &dir.join(format!("{nombre}.png")));
        println!("{nombre}: {} elementos en {:?} (PNG en {:?})", els.len(), t_hecho, t0.elapsed());
    }
}
