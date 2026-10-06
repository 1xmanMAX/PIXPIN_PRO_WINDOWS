//! Muestras `#[ignore]` que pintan la ventana del timeline a un PNG para
//! mirarla, con datos de EJEMPLO en una carpeta temporal (nunca los del
//! usuario). `PIXPIN_MUESTRAS` dice donde dejarlas.
//!
//! `cargo test -p pixpin --bin pixpinmax -- --ignored --test-threads=1 muestra_del_timeline`

use super::*;
use pixpin_render::MotorRender;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;

const W: u32 = 900;
const H: u32 = 900;

/// Una foto de ejemplo: un degradado de dos colores con una franja.
fn foto(
    carpeta: &std::path::Path,
    nombre: &str,
    ancho: u32,
    alto: u32,
    a: [u8; 3],
    b: [u8; 3],
) -> PathBuf {
    let mut pixeles = Vec::with_capacity((ancho * alto * 4) as usize);
    for y in 0..alto {
        for x in 0..ancho {
            let t = (x + y) as f32 / (ancho + alto) as f32;
            let franja = ((y as f32 / alto as f32) - 0.62).abs() < 0.05;
            for c in 0..3 {
                let v = a[c] as f32 * (1.0 - t) + b[c] as f32 * t;
                pixeles.push(if franja { (v * 0.6) as u8 } else { v as u8 });
            }
            pixeles.push(255);
        }
    }
    let ruta = carpeta.join(nombre);
    pixpin_codec::guardar(
        &pixpin_codec::ImagenRgba {
            ancho,
            alto,
            pixeles,
        },
        &ruta,
        pixpin_codec::FormatoImagen::Png,
    )
    .expect("foto de ejemplo");
    ruta
}

/// Un timeline de ejemplo: hoy, ayer, semanas atras y meses atras.
fn ejemplo() -> (Estado, PathBuf) {
    let carpeta =
        std::env::temp_dir().join(format!("pixpin-timeline-muestra-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&carpeta);
    std::fs::create_dir_all(&carpeta).expect("carpeta");
    let almacen = Almacen::en(&carpeta);
    let ahora = Estado::ahora();
    let h = 3_600_000;
    let f1 = foto(&carpeta, "f1.png", 1200, 800, [240, 150, 80], [90, 40, 120]);
    let f2 = foto(&carpeta, "f2.png", 800, 1100, [60, 160, 200], [20, 60, 90]);
    let f3 = foto(
        &carpeta,
        "f3.png",
        1000,
        1000,
        [120, 200, 120],
        [30, 80, 40],
    );
    let mut n = 0;
    let mut poner = |cuando: i64, titulo: &str, desc: &str, fotos: &[&PathBuf], audio: bool| {
        n += 1;
        let id = format!("tl-{cuando}-{n}");
        let mut m = Momento::nuevo(id.clone(), cuando, titulo.into(), desc.into());
        for (k, f) in fotos.iter().enumerate() {
            m.fotos.push(
                almacen
                    .copiar_medio(f, &format!("{id}-{k}"))
                    .expect("copiar"),
            );
        }
        if audio {
            m.audio = Some("no-existe.m4a".into());
            m.duracion_ms = 42_000;
        }
        almacen.anadir(&m).expect("anadir");
    };
    poner(
        ahora - 9 * h,
        "Café con Ana ☕",
        "Hablamos del viaje a Cusco.",
        &[],
        false,
    );
    poner(
        ahora - 6 * h,
        "Se fue la luz en la oficina",
        "Volvió a los 20 minutos; el SAI aguantó.",
        &[&f1],
        true,
    );
    poner(
        ahora - 2 * h,
        "Estudiando cálculo 📚",
        "",
        &[&f2, &f3],
        false,
    );
    poner(
        ahora - 20 * 60_000,
        "Pedido entregado",
        "Llegó bien embalado.",
        &[],
        true,
    );
    for d in 1..40 {
        if d % 3 == 0 {
            continue;
        }
        let emo = ["📚", "🏃", "🍕", "😴", ""][d as usize % 5];
        let fotos: Vec<&PathBuf> = match d % 4 {
            0 => vec![&f1],
            1 => vec![&f2, &f3],
            _ => Vec::new(),
        };
        poner(
            ahora - d * 24 * h,
            &format!("Estudiando para el examen {emo}"),
            "Repaso de integrales.",
            &fotos,
            d % 7 == 0,
        );
        if d % 5 == 0 {
            poner(
                ahora - d * 24 * h + h,
                "Paseo por el malecón",
                "",
                &[],
                false,
            );
        }
    }
    for d in [120, 125, 130, 400] {
        poner(
            ahora - d * 24 * h,
            "Viaje a Arequipa 🏔️",
            "Subimos al mirador.",
            &[&f3],
            false,
        );
    }
    let mut e = Estado::con_almacen(
        almacen,
        Idioma::Espanol,
        carpeta.clone(),
        "PC-EJEMPLO".into(),
    );
    e.lecciones = lecciones_de_ejemplo(&[&f1, &f2, &f3]);
    // Uno marcado como leccion, para ver su marca.
    if let Some(m) = e
        .momentos
        .iter_mut()
        .find(|m| m.titulo.starts_with("Se fue la luz"))
    {
        m.leccion = Some("lec-ejemplo".into());
    }
    (e, carpeta)
}

/// Lecciones de ejemplo, sin chat detras: solo para pintarlas.
fn lecciones_de_ejemplo(fotos: &[&PathBuf]) -> Vec<lecciones::TarjetaLeccion> {
    let ahora = Estado::ahora();
    let datos: [(&str, &str, &str, i64, usize, usize, bool); 5] = [
        (
            "Revisar el andamio antes de subir",
            "Una tabla estaba suelta en el segundo piso.",
            "Prisa por empezar",
            3,
            3,
            2,
            false,
        ),
        (
            "Confirmar el pedido por escrito",
            "El proveedor trajo otro color de cerámica.",
            "",
            2,
            1,
            1,
            true,
        ),
        (
            "Guardar las facturas el mismo día",
            "Perdí dos tickets de la ferretería.",
            "",
            1,
            1,
            0,
            false,
        ),
        ("Probar la bomba antes de llenar", "", "", 2, 2, 0, false),
        (
            "Llevar cargador al taller",
            "Me quedé sin batería a media visita.",
            "",
            1,
            1,
            1,
            false,
        ),
    ];
    datos
        .iter()
        .enumerate()
        .map(|(k, (titulo, paso, por_que, g, veces, n_fotos, voz))| {
            let mut l = pixpin_lecciones::Leccion::nueva(
                &format!("lec-{k}"),
                ahora - k as i64 * 86_400_000,
                titulo,
            );
            l.que_paso = paso.to_string();
            l.por_que = por_que.to_string();
            if k == 1 {
                l.proxima = "Pedirlo siempre por escrito, con foto del color.".into();
            }
            l.gravedad = *g;
            l.repeticiones = (1..*veces).map(|v| ahora - v as i64 * 1000).collect();
            lecciones::TarjetaLeccion {
                entrada: crate::lecciones::almacen::Entrada {
                    leccion: l,
                    mensaje: pixpin_proyecto::cuaderno::Mensaje::default(),
                    ficha: "ejemplo".into(),
                    nombre_chat: "Ejemplo".into(),
                    general: true,
                    archivo: PathBuf::new(),
                },
                fotos: fotos.iter().take(*n_fotos).map(|f| (*f).clone()).collect(),
                voz: voz.then(|| (PathBuf::from("no-existe.m4a"), 42_000)),
            }
        })
        .collect()
}

/// Pinta el estado como lo haria el bucle: primero carga las fotos que hagan
/// falta (fuera del dibujo), luego pinta.
/// Un solo dispositivo y motor para todas las muestras: el pintor guarda
/// recursos de su fabrica, y mezclarlos con los de otro motor falla.
struct Gpu {
    d: pixpin_capture::Dispositivo,
    motor: MotorRender,
}

fn gpu() -> Gpu {
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    Gpu { d, motor }
}

fn muestra(g: &Gpu, nombre: &str, e: &mut Estado) {
    let textos = Catalogo::nuevo(Idioma::Espanol);
    let (d, motor) = (&g.d, &g.motor);
    let marco = Rect {
        x: 0,
        y: 0,
        ancho: W,
        alto: H,
    };
    // Un primer pintado para que el estado se coloque (desplazamiento, etc.)
    // y luego las fotos hasta que no falte ninguna.
    let fuera = FueraDePantalla::nuevo(motor, d.d3d(), W, H).expect("superficie");
    for _ in 0..40 {
        let rutas = imagenes_a_cargar(e, W as f32, H as f32, 1.0);
        let mut falta = e.minis.asegurar(&rutas, motor);
        falta |= cargar_grande(e, marco, motor);
        motor
            .dibujar(&fuera.destino, |p| pintar_todo(e, p, marco, 1.0, &textos))
            .expect("pintar");
        if !falta {
            break;
        }
    }
    fuera.esperar_gpu().expect("esperar");
    let (_, _, pixeles) = fuera.leer_rgba().expect("leer");
    guardar_png(
        nombre,
        &pixpin_codec::ImagenRgba {
            ancho: W,
            alto: H,
            pixeles,
        },
    );
}

fn guardar_png(nombre: &str, img: &pixpin_codec::ImagenRgba) {
    let png = pixpin_codec::imagen::codificar_png(img).expect("codificar");
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let ruta = carpeta.join(format!("{nombre}.png"));
    std::fs::write(&ruta, png).expect("guardar");
    println!("{nombre}: {}", ruta.display());
}

#[test]
#[ignore = "necesita GPU; ejecutar con --ignored y mirar el PNG"]
fn muestra_del_timeline() {
    let (mut e, carpeta) = ejemplo();
    let g = gpu();
    muestra(&g, "timeline-hoy", &mut e);
    e.cambiar_pestana(Pestana::Momentos);
    muestra(&g, "timeline-momentos", &mut e);
    e.cambiar_pestana(Pestana::Estado);
    e.elegidos.insert(e.hoy());
    muestra(&g, "timeline-estado", &mut e);
    e.elegidos.clear();
    // El detalle de un momento con dos fotos, en la segunda.
    e.cambiar_pestana(Pestana::Hoy);
    let i = e
        .momentos
        .iter()
        .position(|m| m.fotos.len() == 2 && m.titulo.contains("cálculo"))
        .expect("momento con dos fotos");
    e.abrir_detalle(i);
    muestra(&g, "timeline-historia", &mut e);
    e.avanzar(1);
    muestra(&g, "timeline-historia-2", &mut e);
    // Uno sin foto y con audio.
    let j = e
        .momentos
        .iter()
        .position(|m| m.titulo == "Pedido entregado")
        .expect("pedido");
    e.abrir_detalle(j);
    muestra(&g, "timeline-historia-sin-foto", &mut e);
    e.detalle = None;
    // Buscando.
    e.busqueda.poner("examen integrales");
    e.buscando = true;
    muestra(&g, "timeline-buscar", &mut e);
    e.vaciar_busqueda();
    // Lo buscado tambien esta en lecciones: la franja.
    e.busqueda.poner("andamio");
    muestra(&g, "timeline-buscar-con-lecciones", &mut e);
    e.vaciar_busqueda();
    // Las lecciones en tarjetas, y el detalle de una.
    e.cambiar_pestana(Pestana::Lecciones);
    muestra(&g, "timeline-lecciones", &mut e);
    e.abrir_leccion(0);
    muestra(&g, "timeline-historia-leccion", &mut e);
    // La tarjeta compartida como imagen: una leccion y un momento.
    let textos = Catalogo::nuevo(Idioma::Espanol);
    for (nombre, item) in [
        ("timeline-compartida-leccion", Item::Leccion("lec-0".into())),
        (
            "timeline-compartida-momento",
            Item::Momento(e.momentos[i].id.clone()),
        ),
        (
            "timeline-compartida-sin-foto",
            Item::Momento(e.momentos[j].id.clone()),
        ),
    ] {
        let c = compartir::contenido_de(&e, &item, &textos).expect("contenido");
        let img = compartir::imagen(&mut e, &c, &g.motor, g.d.d3d(), &textos).expect("imagen");
        guardar_png(nombre, &img);
    }
    e.detalle = None;
    // Un dia abierto desde «Estado».
    e.cambiar_pestana(Pestana::Estado);
    e.dia = Some(Dia::de_numero(e.hoy().numero() - 5));
    muestra(&g, "timeline-dia", &mut e);
    let _ = std::fs::remove_dir_all(carpeta);
}
