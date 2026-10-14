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
//! PIXPIN_IA_MOTOR=claude|deepseek|local  (con la nube: la clave de
//!   ANTHROPIC_API_KEY / DEEPSEEK_API_KEY, o la guardada en Ajustes; para el
//!   local, una carpeta en PIXPIN_IA_MOTOR sigue valiendo como antes)
//! cargo test -p pixpin-ia --test banco -- --ignored --nocapture
//! ```
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

/// La foto del perrito (fuera del repositorio: el caso solo entra si esta).
fn ruta_perrito() -> PathBuf {
    std::env::var("PIXPIN_IA_PERRITO").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from(
            r"C:\Users\MaxBook\AppData\Local\Temp\claude\F--THE-FORGE-PIXPIN-PC-VERSION-MAX\afe461c5-d9fa-4287-b8b5-3ab7961f6cb3\scratchpad\perrito\perrito.png",
        )
    })
}

fn el_perrito() -> Vec<u8> {
    std::fs::read(ruta_perrito()).unwrap()
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
}

impl Ejecutor for Apunta<'_> {
    fn ejecutar(&mut self, llamada: &Llamada) -> Result<String, String> {
        let (nombre, limpio, elementos) = banco_ia::resumir(llamada, self.imagen.as_deref())?;
        let n = elementos.len();
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

/// El motor a medir: `PIXPIN_IA_MOTOR=claude|deepseek` usa la nube (mismo
/// catalogo e instrucciones que la app, traducidos por su adaptador);
/// `local`, nada o una carpeta, llama.cpp.
fn motor() -> Box<dyn Motor> {
    use pixpin_ia::nube::{MotorNube, Proveedor, claves};
    let eleccion = std::env::var("PIXPIN_IA_MOTOR").unwrap_or_default();
    if let Some(p) = Proveedor::por_id(&eleccion.to_lowercase()) {
        let var = match p {
            Proveedor::Claude => "ANTHROPIC_API_KEY",
            Proveedor::DeepSeek => "DEEPSEEK_API_KEY",
        };
        let clave = std::env::var(var)
            .ok()
            .or_else(|| claves::leer(p))
            .unwrap_or_else(|| panic!("sin clave de {}: pon {var} o guardala en Ajustes", p.nombre()));
        println!("motor: {} ({})", p.nombre(), p.modelo());
        return Box::new(MotorNube::nuevo(p, clave));
    }
    Box::new(motor_local())
}

fn motor_local() -> MotorLlama {
    let base = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap()).join("PixPinMax").join("modelos");
    let motor_dir = std::env::var("PIXPIN_IA_MOTOR")
        .ok()
        .filter(|m| !m.is_empty() && m != "local")
        .ok_or(())
        .map(PathBuf::from)
        .unwrap_or_else(|_| base.join("llama.cpp").join("b11580"));
    let modelo = std::env::var("PIXPIN_IA_MODELO")
        .map(PathBuf::from)
        .unwrap_or_else(|_| base.join("qwen3.5-2b").join("Qwen3.5-2B-Q4_K_M.gguf"));
    let vision = std::env::var("PIXPIN_IA_VISION")
        .map(PathBuf::from)
        .ok()
        .or_else(|| Some(base.join("qwen3.5-2b").join("mmproj-F16.gguf")))
        .filter(|p| p.exists());
    let servidor = motor_dir.join("llama-server.exe");
    let mut config = Config::nueva(servidor.clone(), modelo.clone(), vision);
    config.dispositivo = match std::env::var("PIXPIN_IA_DISPOSITIVO").as_deref() {
        Ok("cpu") => Dispositivo::Cpu,
        Ok("auto") | Err(_) => {
            let bytes = std::fs::metadata(&modelo).map(|m| m.len()).unwrap_or(0);
            elegir_dispositivo(&listar_dispositivos(&servidor), bytes)
        }
        Ok(otro) => Dispositivo::Gpu(otro.to_string()),
    };
    println!("dispositivo: {:?}", config.dispositivo);
    MotorLlama::nuevo(config)
}

#[test]
#[ignore = "necesita el motor y el modelo bajados (ver la cabecera)"]
fn banco() {
    // Con `grabado` no hay motor comun: uno por caso, con su respuesta.
    let grabadas = grabado::respuestas();
    let comun = grabadas.is_none().then(motor);
    let solo = std::env::var("PIXPIN_IA_SOLO").ok();
    let mut bien = 0;
    let mut total = 0;
    let mut fallos = Vec::new();
    let t_total = Instant::now();
    for caso in casos() {
        if solo.as_deref().is_some_and(|s| !caso.nombre.contains(s)) {
            continue;
        }
        total += 1;
        let de_este_caso;
        let motor: &dyn Motor = match (&grabadas, &comun) {
            (Some(g), _) => match grabado::MotorGrabado::del_caso(g, caso.nombre) {
                Ok(m) => {
                    de_este_caso = m;
                    &de_este_caso
                }
                Err(motivo) => {
                    println!("MAL  {:<32}   0.0 s    <- {motivo}", caso.nombre);
                    fallos.push(format!("{}: {motivo}", caso.nombre));
                    continue;
                }
            },
            (None, Some(m)) => m.as_ref(),
            (None, None) => unreachable!(),
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
            };
            agente::turno(motor, &mut historial, &herramientas, &mut ej, &mut |e| {
                if let Evento::Reintento { motivo } = &e {
                    invalidas.push(motivo.clone());
                }
                if !matches!(e, Evento::Texto(_)) {
                    eventos.push(format!("{e:?}").chars().take(300).collect::<String>());
                }
                Seguir::Si
            })
        };
        match fin {
            Ok(f) => r.texto = f.texto,
            Err(e) => r.texto = format!("ERROR DEL MOTOR: {e}"),
        }
        let mut veredicto = (caso.comprobar)(&r);
        // Grabado no hay segunda oportunidad: una llamada que no pasa la
        // validacion es un fallo de lo grabado, y se dice tal cual.
        if grabadas.is_some() && !invalidas.is_empty() {
            veredicto = Err(format!("entrada invalida: {}", invalidas.join(" / ")));
        }
        let ok = veredicto.is_ok();
        bien += usize::from(ok);
        println!(
            "{} {:<32} {:>5.1} s  {}{}",
            if ok { "BIEN" } else { "MAL " },
            caso.nombre,
            t0.elapsed().as_secs_f32(),
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
    }
    println!("\n--- fallos ---\n{}", fallos.join("\n"));
    println!(
        "\nBANCO: {bien}/{total} = {:.0} % en {:.0} s",
        bien as f64 * 100.0 / total.max(1) as f64,
        t_total.elapsed().as_secs_f32()
    );
    if let Some(m) = comun {
        m.soltar();
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
    let cuerpo = pixpin_ia::nube::claude::cuerpo(&peticion, &HashMap::new(), true, pixpin_ia::nube::claude::MAX_TOKENS);
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
        })
    }
}

/// Imagenes de prueba hechas aqui, para no depender de ficheros.
mod imagenes {
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
