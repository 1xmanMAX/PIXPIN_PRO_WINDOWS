//! **Muestras en PNG de la tarjeta de Proyectos** en el panel derecho del
//! chat, pintadas con la GPU de verdad y sin ventana, con proyectos de
//! ejemplo: ancha de dia y de noche, la rejilla, otro proyecto, sin elegir,
//! estrecha con la lista y con la tarjeta, y el mismo proyecto en modo Chat.
//!
//! ```text
//! cargo test -p pixpin --bin pixpinmax muestra_de_proyectos -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Deja los PNG en `target/muestras-proyectos`.

use std::path::{Path, PathBuf};

use pixpin_motor2d::{ColorRgba, Elemento, Figura, Punto2};
use pixpin_proyecto::almacen::{self, Ficha, Indice};
use pixpin_render::MotorRender;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_store::{Catalogo, Idioma, Ubicacion};

use super::super::{
    Abierto, CLARO, Lista, OSCURO, Pinta, Tema, abrir_proyecto, filtrar, fotos_del_historial,
    papel_de, pintar as pintar_chat, pintar_cabecera, pintar_historial, pintar_redaccion,
};
use super::{VistaProyectos, cargar};

fn carpeta() -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-proyectos");
    std::fs::create_dir_all(&d).expect("crear la carpeta de muestras");
    d
}

fn guardar(nombre: &str, (ancho, alto, pixeles): (u32, u32, Vec<u8>)) {
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
        ancho,
        alto,
        pixeles,
    })
    .expect("codificar");
    let ruta = carpeta().join(format!("{nombre}.png"));
    std::fs::write(&ruta, png).expect("guardar");
    println!("{nombre}: {}", ruta.display());
}

/// Una pagina de plano: papel blanco con el cajetin y unos muros.
fn pagina_de_plano(n: u32) -> pixpin_codec::imagen::ImagenRgba {
    let (w, h) = (640u32, 905u32);
    let mut px = vec![255u8; (w * h * 4) as usize];
    let mut raya = |x0: u32, y0: u32, x1: u32, y1: u32, gris: u8| {
        for y in y0.min(h - 1)..=y1.min(h - 1) {
            for x in x0.min(w - 1)..=x1.min(w - 1) {
                let i = ((y * w + x) * 4) as usize;
                px[i] = gris;
                px[i + 1] = gris;
                px[i + 2] = gris;
            }
        }
    };
    // El marco y el cajetin.
    raya(20, 20, 620, 23, 40);
    raya(20, 882, 620, 885, 40);
    raya(20, 20, 23, 885, 40);
    raya(617, 20, 620, 885, 40);
    raya(380, 800, 620, 803, 40);
    raya(380, 800, 383, 885, 40);
    // Muros que cambian con la pagina.
    let d = 30 * (n % 4);
    raya(80, 120 + d, 540, 126 + d, 70);
    raya(80, 120 + d, 86, 640, 70);
    raya(534, 120 + d, 540, 640, 70);
    raya(80, 634, 540, 640, 70);
    raya(300, 120 + d, 304, 400, 110);
    raya(86, 400, 300, 404, 110);
    for k in 0..6 {
        raya(120 + k * 60, 700, 150 + k * 60, 730, 160);
    }
    pixpin_codec::imagen::ImagenRgba {
        ancho: w,
        alto: h,
        pixeles: px,
    }
}

fn color(r: f32, g: f32, b: f32) -> ColorRgba {
    ColorRgba { r, g, b, a: 1.0 }
}

fn lienzo(ruta: &Path, elementos: Vec<Elemento>) {
    let mut escena = pixpin_motor2d::Escena::nueva();
    for e in elementos {
        escena.anadir(e);
    }
    let l = pixpin_motor2d::excalidraw::con_escena(
        &pixpin_motor2d::excalidraw::Lienzo::vacio(),
        &escena,
    );
    std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
    std::fs::write(ruta, pixpin_motor2d::excalidraw::escribir(&l)).unwrap();
}

fn caja(figura: Figura, x: f32, y: f32, ancho: f32, alto: f32, trazo: ColorRgba) -> Elemento {
    Elemento {
        figura,
        x,
        y,
        ancho,
        alto,
        grosor: 3.0,
        trazo,
        rugosidad: 0.0,
        ..Default::default()
    }
}

fn linea(puntos: &[(f32, f32)], trazo: ColorRgba) -> Elemento {
    let v: Vec<Punto2> = puntos.iter().map(|(x, y)| Punto2::nuevo(*x, *y)).collect();
    let (x0, y0) = (
        v.iter().map(|p| p.x).fold(f32::MAX, f32::min),
        v.iter().map(|p| p.y).fold(f32::MAX, f32::min),
    );
    let (x1, y1) = (
        v.iter().map(|p| p.x).fold(f32::MIN, f32::max),
        v.iter().map(|p| p.y).fold(f32::MIN, f32::max),
    );
    Elemento {
        figura: Figura::Linea {
            // Los puntos de una linea van en coordenadas del lienzo.
            puntos: v.clone(),
        },
        x: x0,
        y: y0,
        ancho: x1 - x0,
        alto: y1 - y0,
        grosor: 4.0,
        trazo,
        rugosidad: 0.0,
        ..Default::default()
    }
}

/// Un almacen con tres proyectos: una obra con su plano de seis paginas (una
/// anotada), un lienzo y una nota; una oficina con dos lienzos; y una tienda
/// archivada sin hojas. Mas «Mensajes guardados», que no debe salir.
fn almacen_de_ejemplo() -> (Ubicacion, Vec<Ficha>) {
    let raiz =
        std::env::temp_dir().join(format!("pixpin-muestra-proyectos-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();
    let u = Ubicacion::Portable { raiz: raiz.clone() };
    let hoja = |id: &str, nombre: &str| pixpin_proyecto::Hoja {
        id: id.into(),
        nombre: nombre.into(),
        uid: Some(format!("u-{id}")),
        ..Default::default()
    };
    let escribir = |id: &str, p: &pixpin_proyecto::Proyecto| {
        let c = almacen::carpeta(&raiz, id);
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(c.join("proyecto.json"), serde_json::to_string(p).unwrap()).unwrap();
    };

    // La obra.
    let obra = almacen::carpeta(&raiz, "obra");
    std::fs::create_dir_all(obra.join("archivos")).unwrap();
    let mut hojas = Vec::new();
    for n in 0..6u32 {
        let png = pixpin_codec::imagen::codificar_png(&pagina_de_plano(n)).unwrap();
        std::fs::write(
            obra.join("archivos")
                .join(format!("pagina-{:02}.png", n + 1)),
            png,
        )
        .unwrap();
        let mut h = hoja(&format!("pag{n}"), &format!("Pág. {}", n + 1));
        h.pagina = Some(n);
        if n == 1 {
            // La pagina 2, anotada: una cota en rojo y un circulo.
            h.dibujo = Some("dib-pag2".into());
            lienzo(
                &almacen::lienzo(&raiz, "obra", "dib-pag2"),
                vec![
                    caja(
                        Figura::Elipse,
                        330.0,
                        470.0,
                        140.0,
                        140.0,
                        color(0.85, 0.2, 0.2),
                    ),
                    linea(&[(90.0, 655.0), (530.0, 665.0)], color(0.85, 0.2, 0.2)),
                ],
            );
        }
        hojas.push(h);
    }
    let mut detalle = hoja("esc", "Detalle escalera");
    detalle.dibujo = Some("dib-escalera".into());
    lienzo(
        &almacen::lienzo(&raiz, "obra", "dib-escalera"),
        vec![
            linea(
                &[
                    (0.0, 400.0),
                    (80.0, 400.0),
                    (80.0, 320.0),
                    (160.0, 320.0),
                    (160.0, 240.0),
                    (240.0, 240.0),
                    (240.0, 160.0),
                    (320.0, 160.0),
                    (320.0, 80.0),
                    (400.0, 80.0),
                ],
                color(0.1, 0.1, 0.1),
            ),
            caja(
                Figura::Rectangulo,
                20.0,
                430.0,
                360.0,
                40.0,
                color(0.12, 0.45, 0.8),
            ),
        ],
    );
    hojas.push(detalle);
    let mut nota = hoja("pres", "Presupuesto");
    nota.nota = Some(
        "# Presupuesto\n\nAcero: 1 200 €\nCemento: 640 €\nMano de obra: 3 días\n\n- [ ] Pedir el acero\n- [x] Medir la losa"
            .into(),
    );
    hojas.push(nota);
    escribir(
        "obra",
        &pixpin_proyecto::Proyecto {
            id: "obra".into(),
            nombre: "Casa Lima".into(),
            hojas,
            tocado: 300,
            pdf_origen: Some("/storage/emulated/0/Download/plano.pdf".into()),
            ..Default::default()
        },
    );

    // La oficina, con dos lienzos.
    let mut a = hoja("of1", "Planta");
    a.dibujo = Some("dib-planta".into());
    lienzo(
        &almacen::lienzo(&raiz, "oficina", "dib-planta"),
        vec![
            caja(
                Figura::Rectangulo,
                0.0,
                0.0,
                600.0,
                400.0,
                color(0.1, 0.1, 0.1),
            ),
            caja(
                Figura::Rectangulo,
                20.0,
                20.0,
                200.0,
                160.0,
                color(0.18, 0.66, 0.31),
            ),
            caja(
                Figura::Elipse,
                380.0,
                220.0,
                160.0,
                120.0,
                color(0.88, 0.5, 0.23),
            ),
        ],
    );
    let mut b = hoja("of2", "Muebles");
    b.dibujo = Some("dib-muebles".into());
    lienzo(
        &almacen::lienzo(&raiz, "oficina", "dib-muebles"),
        vec![
            caja(
                Figura::Rombo,
                0.0,
                0.0,
                200.0,
                200.0,
                color(0.61, 0.35, 0.82),
            ),
            caja(
                Figura::Rectangulo,
                240.0,
                40.0,
                200.0,
                120.0,
                color(0.07, 0.65, 0.65),
            ),
        ],
    );
    escribir(
        "oficina",
        &pixpin_proyecto::Proyecto {
            id: "oficina".into(),
            nombre: "Oficina Miraflores".into(),
            hojas: vec![a, b],
            tocado: 200,
            ..Default::default()
        },
    );

    // La tienda, archivada y vacia.
    escribir(
        "tienda",
        &pixpin_proyecto::Proyecto {
            id: "tienda".into(),
            nombre: "Tienda Barranco".into(),
            archivado: true,
            tocado: 400,
            ..Default::default()
        },
    );

    let ficha = |id: &str, nombre: &str, tocado: i64| Ficha {
        id: id.into(),
        nombre: nombre.into(),
        tocado,
        ..Default::default()
    };
    let mut guardados = ficha("guardados", "Mensajes guardados", 999);
    guardados
        .resto
        .insert("guardados".into(), serde_json::Value::Bool(true));
    let fichas = vec![
        guardados,
        ficha("obra", "Casa Lima", 300),
        ficha("oficina", "Oficina Miraflores", 200),
        ficha("tienda", "Tienda Barranco", 400),
    ];
    Indice {
        proyectos: fichas.clone(),
        ..Default::default()
    }
    .guardar(&raiz)
    .unwrap();
    (u, fichas)
}

struct Banco {
    motor: MotorRender,
    d3d: windows::Win32::Graphics::Direct3D11::ID3D11Device,
}

/// Lo que se fotografia: la ventana del chat con su lista, el proyecto
/// elegido y, si se pasa, su chat abierto (modo Chat) en vez de la tarjeta.
struct Escena<'a> {
    u: &'a Ubicacion,
    fichas: &'a [Ficha],
    tamano: (u32, u32),
    claro: bool,
    elegido: Option<&'a str>,
    chat: Option<&'a Abierto>,
    /// Lo escrito en el buscador de la lista.
    busqueda: &'a str,
    /// Con el tema Cosmos: el cielo detras de la lista y los colores de noche.
    cosmos: bool,
    /// El ancho de la lista, si no es el de salida (p. ej. plegada).
    ancho_lista: Option<u32>,
}

impl Banco {
    fn nuevo() -> Banco {
        let dispositivo = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let d3d = dispositivo.d3d().clone();
        let motor = MotorRender::nuevo(&d3d).expect("motor");
        Banco { motor, d3d }
    }

    /// La ventana del chat como la pinta el bucle: la barra, la lista y, en
    /// el panel derecho, la tarjeta del elegido (o su chat), con el
    /// interruptor arriba.
    fn foto(&self, v: &mut VistaProyectos, s: &Escena) -> (u32, u32, Vec<u8>) {
        let (ancho, alto) = s.tamano;
        let destino = FueraDePantalla::nuevo(&self.motor, &self.d3d, ancho, alto).expect("destino");
        // Como la ventana: con Cosmos, siempre los colores de noche.
        let claro = s.claro && !s.cosmos;
        let tema: &Tema = if claro { &CLARO } else { &OSCURO };
        crate::tema_cosmos::fijar(s.cosmos);
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let escala = 100;
        // Estrecha, un solo panel: la tarjeta (o el chat) si hay proyecto
        // elegido y si no la lista, como `disponer`.
        let vista = if s.elegido.is_some() {
            pixpin_ui::chat::Vista::SoloChat
        } else {
            pixpin_ui::chat::Vista::SoloLista
        };
        let d = pixpin_ui::chat::Disposicion::calcular(
            ancho,
            alto,
            escala,
            s.ancho_lista
                .unwrap_or(pixpin_ui::chat::ancho_inicial(ancho, escala)),
            vista,
        );
        let (cw, ch) = crate::tema_cosmos::tamano_de(&d);
        crate::tema_cosmos::preparar(&self.motor, cw, ch);
        let marcados = Default::default();
        let orden = filtrar(s.fichas, s.busqueda);
        let elegida = s
            .elegido
            .and_then(|id| s.fichas.iter().position(|f| f.id == id));
        v.seguir(s.elegido);
        if s.chat.is_some() {
            v.ver_chat();
        } else {
            v.ver_proyectos();
        }
        let mut previas = crate::miniaturas::Miniaturas::con_lado(super::super::PREVIA_LADO);
        let miniaturas = crate::miniaturas::Miniaturas::nuevo();
        // Unas vueltas: la primera apunta lo que se ve, las siguientes lo
        // suben (cuatro por fotograma) y lo pintan. La ultima se mide.
        for vuelta in 0..5 {
            let t0 = std::time::Instant::now();
            v.preparar(&mut previas, &self.motor);
            if let Some(a) = s.chat {
                let rutas = fotos_del_historial(a, &d, escala);
                if !rutas.is_empty() {
                    previas.asegurar(&rutas, &self.motor);
                }
            }
            let lista = Lista {
                fichas: s.fichas,
                orden: &orden,
                scroll: 0,
                sobre: None,
                elegida,
                ahora: 0,
                textos: &textos,
                busqueda: s.busqueda,
                marcados: &marcados,
            };
            let c = Pinta {
                tema,
                escala,
                textos: &textos,
                ahora: 0,
                miniaturas: &miniaturas,
                previas: &previas,
            };
            self.motor
                .dibujar(&destino.destino, |p| {
                    pintar_chat(
                        p,
                        &d,
                        tema,
                        papel_de(0, claro),
                        escala,
                        &textos,
                        None,
                        &lista,
                    );
                    match (s.chat, elegida) {
                        (Some(a), _) => {
                            a.zonas.borrow_mut().clear();
                            let alto_texto = (pixpin_ui::chat::REDACCION_TAM * 1.3).ceil() as u32;
                            a.alto_caja.set(alto_texto);
                            pintar_historial(p, &d, &c, a);
                            pintar_redaccion(p, &d, &c, a, alto_texto);
                            pintar_cabecera(p, &d, &c, a);
                        }
                        (None, Some(i)) => {
                            let f = &s.fichas[i];
                            super::pintar(p, v, &d, &c, f, &f.nombre, false, claro, s.u.raiz());
                        }
                        (None, None) => {}
                    }
                    super::pintar_interruptor(p, v, &d, tema, escala, &textos);
                })
                .expect("fotograma");
            if vuelta == 4 {
                destino.esperar_gpu().expect("GPU");
                println!(
                    "  fotograma quieto: {:.2} ms (preparar + pintar + GPU)",
                    t0.elapsed().as_secs_f64() * 1000.0
                );
            }
        }
        destino.esperar_gpu().expect("GPU");
        // El cielo es de este dispositivo: fuera antes de que muera.
        crate::tema_cosmos::soltar();
        crate::tema_cosmos::fijar(false);
        destino.leer_rgba().expect("leer")
    }
}

/// Lee las hojas de los proyectos en este hilo (en la ventana van en el
/// suyo) y pide sus paginas ya, que aqui nadie va a esperar al reintento.
fn cargar_todo(v: &mut VistaProyectos, u: &Ubicacion, fichas: &[Ficha]) {
    let mut resumenes = std::collections::HashMap::new();
    for f in fichas.iter().filter(|f| !f.es_guardados()) {
        resumenes.insert(f.id.clone(), cargar::resumen_de(u, f));
        let mut h = cargar::hojas_de(u, f);
        for hoja in &mut h.hojas {
            if hoja.por_pedir {
                hoja.vista = super::super::leer_vista(u, &f.id, &hoja.mensaje);
            }
        }
        v.meter_datos(h);
    }
    v.meter_resumenes(resumenes);
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn muestra_de_proyectos() {
    let (u, _) = almacen_de_ejemplo();
    // La lista como la lee la ventana: el indice en su orden.
    let fichas: Vec<Ficha> = Indice::leer(u.raiz())
        .ordenadas()
        .into_iter()
        .cloned()
        .collect();
    let b = Banco::nuevo();
    let mut v = VistaProyectos::nueva(&u);
    cargar_todo(&mut v, &u, &fichas);
    let escena = |tamano, claro, elegido| Escena {
        u: &u,
        fichas: &fichas,
        tamano,
        claro,
        elegido,
        chat: None,
        busqueda: "",
        cosmos: false,
        ancho_lista: None,
    };
    let ancha = (1100, 720);
    let estrecha = (480, 820);
    // La portada en la pagina anotada.
    v.con_tarjeta("obra", |t| t.en_foco = 1);
    guardar(
        "proyectos-ancha-claro",
        b.foto(&mut v, &escena(ancha, true, Some("obra"))),
    );
    guardar(
        "proyectos-ancha-oscuro",
        b.foto(&mut v, &escena(ancha, false, Some("obra"))),
    );
    // La rejilla.
    v.con_tarjeta("obra", |t| t.rejilla = true);
    guardar(
        "proyectos-rejilla-oscuro",
        b.foto(&mut v, &escena(ancha, false, Some("obra"))),
    );
    // Dos paginas marcadas con el derecho (E9: se fusionan desde aqui).
    let marcar: Vec<String> = v
        .datos("obra")
        .map(|d| {
            d.hojas
                .iter()
                .filter(|h| h.mensaje.pagina.is_some())
                .take(2)
                .map(|h| h.id.clone())
                .collect()
        })
        .unwrap_or_default();
    v.con_tarjeta("obra", |t| t.marcadas = marcar.into_iter().collect());
    guardar(
        "proyectos-rejilla-marcadas-claro",
        b.foto(&mut v, &escena(ancha, true, Some("obra"))),
    );
    v.con_tarjeta("obra", |t| {
        t.marcadas.clear();
        t.rejilla = false;
    });
    // Otro proyecto elegido en la lista.
    guardar(
        "proyectos-oficina-claro",
        b.foto(&mut v, &escena(ancha, true, Some("oficina"))),
    );
    // Sin proyecto elegido: el «Elige un proyecto» del chat.
    guardar(
        "proyectos-sin-elegir-claro",
        b.foto(&mut v, &escena(ancha, true, None)),
    );
    // Una ventana muy apaisada y baja: tarjeta en dos columnas.
    guardar(
        "proyectos-apaisada-claro",
        b.foto(&mut v, &escena((1400, 560), true, Some("obra"))),
    );
    // Estrecha: la lista sola, y al elegir, la tarjeta sola con volver.
    guardar(
        "proyectos-estrecha-lista-claro",
        b.foto(&mut v, &escena(estrecha, true, None)),
    );
    guardar(
        "proyectos-estrecha-tarjeta-claro",
        b.foto(&mut v, &escena(estrecha, true, Some("obra"))),
    );
    guardar(
        "proyectos-estrecha-tarjeta-oscuro",
        b.foto(&mut v, &escena(estrecha, false, Some("obra"))),
    );
    // Y el mismo proyecto en modo Chat, con el interruptor en «Chat».
    let obra = fichas.iter().find(|f| f.id == "obra").unwrap();
    let a = abrir_proyecto(&u, obra);
    guardar(
        "chat-casa-lima-claro",
        b.foto(
            &mut v,
            &Escena {
                chat: Some(&a),
                ..escena(ancha, true, Some("obra"))
            },
        ),
    );
    // Leer las hojas de un proyecto, lo que hace el hilo al llegar a el.
    let t0 = std::time::Instant::now();
    let h = cargar::hojas_de(&u, obra);
    println!(
        "  leer las {} hojas de «Casa Lima»: {:.1} ms",
        h.hojas.len(),
        t0.elapsed().as_secs_f64() * 1000.0
    );
}

/// La lista de chats en todos sus casos, para buscar texto fuera de sitio
/// (la queja: «se nota un texto en la parte del fondo que no corresponde
/// ahi»): modo Chat y Proyectos, ancha, estrecha y plegada, de dia, de
/// noche y con Cosmos, con muchos y pocos proyectos, y con el buscador
/// vacio, con resultados y sin ninguno.
///
/// ```text
/// cargo test -p pixpin --bin pixpinmax muestra_de_la_lista_de_chats -- --ignored --nocapture --test-threads=1
/// ```
#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn muestra_de_la_lista_de_chats() {
    let (u, _) = almacen_de_ejemplo();
    let pocas: Vec<Ficha> = Indice::leer(u.raiz())
        .ordenadas()
        .into_iter()
        .cloned()
        .collect();
    // Muchas: las de verdad y treinta mas sin carpeta, solo en el indice.
    let mut muchas = pocas.clone();
    for n in 0..30 {
        muchas.push(Ficha {
            id: format!("extra{n}"),
            nombre: format!("Proyecto de relleno {n}"),
            tocado: 100 - n,
            ..Default::default()
        });
    }
    let b = Banco::nuevo();
    let mut v = VistaProyectos::nueva(&u);
    cargar_todo(&mut v, &u, &pocas);
    let obra = pocas.iter().find(|f| f.id == "obra").unwrap();
    let a = abrir_proyecto(&u, obra);
    let ancha = (1100, 720);
    let estrecha = (480, 820);
    for (nombre_fichas, fichas) in [("pocos", &pocas), ("muchos", &muchas)] {
        for (tema, claro, cosmos) in [
            ("claro", true, false),
            ("oscuro", false, false),
            ("cosmos", false, true),
        ] {
            for (busca, busqueda) in [("", ""), ("-busca", "casa"), ("-nada", "zzz")] {
                let base = Escena {
                    u: &u,
                    fichas,
                    tamano: ancha,
                    claro,
                    elegido: Some("obra"),
                    chat: None,
                    busqueda,
                    cosmos,
                    ancho_lista: None,
                };
                let n = |que: &str| format!("lista-{nombre_fichas}-{que}-{tema}{busca}");
                guardar(&n("proyectos-ancha"), b.foto(&mut v, &base));
                guardar(
                    &n("chat-ancha"),
                    b.foto(
                        &mut v,
                        &Escena {
                            chat: Some(&a),
                            ..base
                        },
                    ),
                );
                guardar(
                    &n("sin-elegir-ancha"),
                    b.foto(
                        &mut v,
                        &Escena {
                            elegido: None,
                            ..base
                        },
                    ),
                );
                guardar(
                    &n("estrecha"),
                    b.foto(
                        &mut v,
                        &Escena {
                            elegido: None,
                            tamano: estrecha,
                            ..base
                        },
                    ),
                );
                guardar(
                    &n("plegada"),
                    b.foto(
                        &mut v,
                        &Escena {
                            ancho_lista: Some(66),
                            chat: Some(&a),
                            ..base
                        },
                    ),
                );
            }
        }
    }
}

/// La queja de verdad: un mensaje de varias lineas (una tabla pegada, con
/// saltos y tabuladores) es el ultimo de «Mensajes guardados», y su resumen
/// en la lista se salia de su fila y se veia detras de las de abajo.
///
/// ```text
/// cargo test -p pixpin --bin pixpinmax muestra_de_la_lista_con_un_resumen_de_varias_lineas -- --ignored --nocapture --test-threads=1
/// ```
#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn muestra_de_la_lista_con_un_resumen_de_varias_lineas() {
    let (u, _) = almacen_de_ejemplo();
    let mut fichas: Vec<Ficha> = Indice::leer(u.raiz())
        .ordenadas()
        .into_iter()
        .cloned()
        .collect();
    fichas[0].resumen = "DESCRIPCION \tMONTO\r\nALQUILER \t690\r\n\t687.5\r\n\t685.4\r\nCELULAR\t39.95\r\n\t19.9\r\nLUZ \t29.6\r\nINTERNET\t64.99\r\n\t\r\n\t2517.34".into();
    let b = Banco::nuevo();
    let mut v = VistaProyectos::nueva(&u);
    cargar_todo(&mut v, &u, &fichas);
    for (tema, claro, cosmos) in [("claro", true, false), ("cosmos", false, true)] {
        for (forma, tamano) in [("estrecha", (570, 900)), ("ancha", (1100, 720))] {
            let s = Escena {
                u: &u,
                fichas: &fichas,
                tamano,
                claro,
                elegido: None,
                chat: None,
                busqueda: "",
                cosmos,
                ancho_lista: None,
            };
            let con_tabla = b.foto(&mut v, &s);
            // Lo mismo con un resumen de una linea: de la segunda fila para
            // abajo la lista tiene que ser identica. Si el resumen se sale
            // de la suya, se ve ahi.
            let mut corta = fichas.clone();
            corta[0].resumen = "DESCRIPCION".into();
            let sin_tabla = b.foto(
                &mut v,
                &Escena {
                    fichas: &corta,
                    ..s
                },
            );
            let d = pixpin_ui::chat::Disposicion::calcular(
                tamano.0,
                tamano.1,
                100,
                pixpin_ui::chat::ancho_inicial(tamano.0, 100),
                pixpin_ui::chat::Vista::SoloLista,
            );
            let desde = d.fila(1, 0, 100).y.max(0) as u32;
            let (ancho, alto, a) = (&con_tabla.0, &con_tabla.1, &con_tabla.2);
            let distintos = (desde..*alto)
                .flat_map(|y| {
                    (0..d.lista.ancho.min(*ancho)).map(move |x| ((y * ancho + x) * 4) as usize)
                })
                .filter(|&i| a[i..i + 4] != sin_tabla.2[i..i + 4])
                .count();
            guardar(
                &format!("lista-resumen-varias-lineas-{forma}-{tema}"),
                con_tabla,
            );
            assert_eq!(
                distintos, 0,
                "{forma} {tema}: el resumen de la primera fila pinta {distintos} pixeles en las de abajo"
            );
        }
    }
}
