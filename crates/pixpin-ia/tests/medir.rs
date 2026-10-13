//! **Medir el asistente de verdad** en este PC: carga, tokens/s, memoria
//! pico, una llamada a herramienta por el bucle del agente y una imagen.
//!
//! No corre con `cargo test` normal: necesita el motor y un modelo bajados.
//!
//! ```text
//! PIXPIN_IA_MOTOR=<carpeta con llama-server.exe>
//! PIXPIN_IA_MODELO=<fichero .gguf>   PIXPIN_IA_VISION=<mmproj .gguf> (opcional)
//! PIXPIN_IA_IMAGEN=<png>  (opcional)   PIXPIN_IA_DISPOSITIVO=cpu|auto|Vulkan1
//! cargo test -p pixpin-ia --test medir -- --ignored --nocapture
//! ```

use std::path::PathBuf;
use std::time::Instant;

use pixpin_ia::agente::{self, Ejecutor, Evento};
use pixpin_ia::herramientas::{Llamada, catalogo};
use pixpin_ia::lienzo;
use pixpin_ia::motor::{Mensaje, Motor, Peticion, Seguir, Trozo};
use pixpin_ia::servidor::{Config, Dispositivo, MotorLlama, elegir_dispositivo, listar_dispositivos};

struct AlLienzo {
    documentos: Vec<String>,
}

impl Ejecutor for AlLienzo {
    fn ejecutar(&mut self, llamada: &Llamada) -> Result<String, String> {
        let els = lienzo::elementos(llamada, (0.0, 0.0), 1);
        let n = els.len();
        let doc = lienzo::documento(els);
        // Lo tiene que leer el lienzo de verdad.
        pixpin_motor2d::excalidraw::leer(&doc).map_err(|e| e.to_string())?;
        self.documentos.push(doc);
        Ok(format!("hecho: {n} elementos en el lienzo"))
    }
}

fn pico_mb(pid: u32) -> u64 {
    let salida = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {pid}).PeakWorkingSet64")])
        .output()
        .ok();
    salida
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u64>().ok())
        .unwrap_or(0)
        / (1024 * 1024)
}

#[test]
#[ignore = "necesita el motor y un modelo bajados (ver la cabecera)"]
fn medir_en_este_pc() {
    let motor_dir = PathBuf::from(std::env::var("PIXPIN_IA_MOTOR").expect("PIXPIN_IA_MOTOR"));
    let modelo = PathBuf::from(std::env::var("PIXPIN_IA_MODELO").expect("PIXPIN_IA_MODELO"));
    let vision = std::env::var("PIXPIN_IA_VISION").ok().map(PathBuf::from);
    let servidor = motor_dir.join("llama-server.exe");
    let mut config = Config::nueva(servidor.clone(), modelo.clone(), vision.clone());
    config.dispositivo = match std::env::var("PIXPIN_IA_DISPOSITIVO").as_deref() {
        Ok("auto") => {
            let bytes = std::fs::metadata(&modelo).map(|m| m.len()).unwrap_or(0);
            elegir_dispositivo(&listar_dispositivos(&servidor), bytes)
        }
        Ok("cpu") | Err(_) => Dispositivo::Cpu,
        Ok(otro) => Dispositivo::Gpu(otro.to_string()),
    };
    println!("dispositivo: {:?}", config.dispositivo);
    let motor = MotorLlama::nuevo(config);

    // 1. Carga.
    motor.precargar().unwrap();
    println!("carga: {:.2} s", motor.tiempo_de_carga().unwrap().as_secs_f32());

    // 2. Texto en vivo: tiempo al primer trozo y velocidad.
    let t0 = Instant::now();
    let mut primero = None;
    let r = motor
        .completar(
            &Peticion::nueva(vec![
                Mensaje::sistema(agente::instrucciones()),
                Mensaje::usuario("Explica en dos frases para que sirve fijar una captura en pantalla."),
            ]),
            &mut |t| {
                if primero.is_none() && matches!(t, Trozo::Texto(_)) {
                    primero = Some(t0.elapsed());
                }
                Seguir::Si
            },
        )
        .unwrap();
    println!(
        "texto: primer trozo {:.2} s, total {:.2} s, {} tok a {:.1} tok/s (lectura {:.1} tok/s)\n  «{}»",
        primero.unwrap_or_default().as_secs_f32(),
        t0.elapsed().as_secs_f32(),
        r.medidas.tokens_salida,
        r.medidas.tokens_por_segundo_escritura,
        r.medidas.tokens_por_segundo_lectura,
        r.texto.trim()
    );
    assert!(!r.texto.trim().is_empty());

    // 3. Un turno con herramientas por el bucle del agente.
    let mut ej = AlLienzo { documentos: Vec::new() };
    for pedido in [
        "Dibuja en el lienzo un circulo rojo a la izquierda y un cuadrado azul a la derecha, con el titulo CASA arriba.",
        "Hazme una grafica de barras de ventas: enero 120, febrero 95, marzo 140.",
    ] {
        let t0 = Instant::now();
        let mut h = vec![Mensaje::sistema(agente::instrucciones()), Mensaje::usuario(pedido)];
        let mut eventos = Vec::new();
        let f = agente::turno(&motor, &mut h, &catalogo(), &mut ej, &mut |e| {
            if !matches!(e, Evento::Texto(_)) {
                eventos.push(format!("{e:?}"));
            }
            Seguir::Si
        })
        .unwrap();
        println!(
            "herramientas: {:.2} s, hechas {}, reintentos {}\n  eventos: {}\n  final: «{}»",
            t0.elapsed().as_secs_f32(),
            f.herramientas_hechas,
            f.reintentos,
            eventos.join("\n           "),
            f.texto.trim()
        );
    }

    // 4. Imagen, si hay vision.
    if let (Some(_), Ok(png)) = (&vision, std::env::var("PIXPIN_IA_IMAGEN")) {
        let bytes = std::fs::read(png).unwrap();
        let t0 = Instant::now();
        let mut h = vec![
            Mensaje::sistema(agente::instrucciones()),
            Mensaje::usuario_con_imagen("Quiero esta imagen: dibujala dentro del lienzo.", bytes, "image/png"),
        ];
        let mut eventos = Vec::new();
        let f = agente::turno(&motor, &mut h, &catalogo(), &mut ej, &mut |e| {
            if !matches!(e, Evento::Texto(_)) {
                eventos.push(format!("{e:?}"));
            }
            Seguir::Si
        })
        .unwrap();
        println!(
            "imagen→lienzo: {:.2} s, hechas {}, reintentos {}\n  eventos: {}\n  final: «{}»",
            t0.elapsed().as_secs_f32(),
            f.herramientas_hechas,
            f.reintentos,
            eventos.join("\n           "),
            f.texto.trim()
        );
    }

    if let Some(pid) = motor.pid() {
        println!("memoria pico del motor: {} MB", pico_mb(pid));
    }
    if let Ok(d) = std::env::var("PIXPIN_IA_SALIDA") {
        for (i, doc) in ej.documentos.iter().enumerate() {
            std::fs::write(PathBuf::from(&d).join(format!("dibujo-{i}.excalidraw")), doc).unwrap();
        }
    }
    // 5. Soltar devuelve la memoria.
    motor.soltar();
    assert!(!motor.cargado());
}

/// Baja de verdad el motor (32 MB) y el modelo ligero (740 MB) a
/// `PIXPIN_IA_RAIZ`, comprobando huellas, y los arranca.
#[test]
#[ignore = "baja ~770 MB de internet"]
fn bajar_y_arrancar_el_ligero() {
    use pixpin_ia::descarga;
    use pixpin_ia::modelos::{LLAMA_CPP, QWEN35_08B};
    use std::sync::atomic::AtomicBool;

    let raiz = PathBuf::from(std::env::var("PIXPIN_IA_RAIZ").expect("PIXPIN_IA_RAIZ"));
    let t0 = Instant::now();
    let mut ultimo = 0.0;
    descarga::asegurar(
        &raiz,
        &QWEN35_08B,
        &LLAMA_CPP,
        &mut |p| {
            let f = p.fraccion();
            if f - ultimo >= 0.1 || p.verificando {
                ultimo = f;
                println!("{:>5.1}% {}{}", f * 100.0, p.fichero, if p.verificando { " (huella)" } else { "" });
            }
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    println!("bajado y verificado en {:.1} s", t0.elapsed().as_secs_f32());
    assert!(descarga::runtime_listo(&raiz, &LLAMA_CPP));
    assert!(descarga::modelo_listo(&raiz, &QWEN35_08B));
    let carpeta = descarga::carpeta_modelo(&raiz, &QWEN35_08B);
    let motor = MotorLlama::nuevo(Config::nueva(
        descarga::ruta_servidor(&raiz, &LLAMA_CPP),
        carpeta.join(QWEN35_08B.pesos.nombre),
        QWEN35_08B.vision.map(|v| carpeta.join(v.nombre)),
    ));
    let r = motor
        .completar(&Peticion::nueva(vec![Mensaje::usuario("Di hola en una palabra.")]), &mut |_| Seguir::Si)
        .unwrap();
    println!("«{}» en {:.1} s (carga {:.1} s)", r.texto.trim(), t0.elapsed().as_secs_f32(), motor.tiempo_de_carga().unwrap().as_secs_f32());
    assert!(!r.texto.trim().is_empty());
    motor.soltar();
}
