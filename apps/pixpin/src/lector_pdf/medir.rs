//! **Medir el hilo de las hojas** con un PDF grande: cuanto tarda la primera
//! hoja, cuantas hojas se dibujan en vano al saltar y cuanta memoria se
//! queda en mapas de bits. Es una prueba `#[ignore]`: necesita GPU y
//! `E:\pixpin-demo\Tesis.pdf` (se copia; el original no se abre):
//!
//! `cargo test -p pixpin --bin pixpinmax medir_la_cola_de_hojas -- --ignored --nocapture`

use super::pruebas::{estado, marco};
use super::*;
use std::time::{Duration, Instant};

/// Un PDF de al menos `n` hojas hecho de copias de la tesis de muestra, en
/// la carpeta temporal (se reutiliza entre pasadas).
fn pdf_grande(n: u32) -> PathBuf {
    let destino = std::env::temp_dir().join(format!("pixpin-medir-cola-{n}.pdf"));
    if destino.is_file() {
        return destino;
    }
    let original = std::fs::read(r"E:\pixpin-demo\Tesis.pdf").expect("la tesis de muestra");
    let mut b = original.clone();
    while pixpin_pdf::union::contar_paginas(&b).unwrap_or(0) < n {
        b = pixpin_pdf::union::anadir_paginas(&b, &original).expect("pegar");
    }
    std::fs::write(&destino, &b).expect("escribir");
    destino
}

/// Lo medido en una escena.
#[derive(Debug, Default)]
struct Medida {
    dibujadas: usize,
    en_vano: usize,
    ms_hasta_nitida: u128,
}

struct Banco {
    e: Estado,
    m: Marco,
    tx: mpsc::Sender<Pedido>,
    rx: mpsc::Receiver<Llega>,
    motor: pixpin_render::MotorRender,
    textos: Catalogo,
    pico_bytes: usize,
    medida: Medida,
}

impl Banco {
    /// Un fotograma del bucle del lector: pide, recoge lo que llego y apunta
    /// lo dibujado en vano (lo que llega y ya no hace falta).
    fn fotograma(&mut self) {
        acotar(&mut self.e, self.m);
        pedir_hojas(&mut self.e, &self.tx, self.m);
        while let Ok(l) = self.rx.try_recv() {
            if let Some(i) = hoja_que_llega(&l) {
                self.medida.dibujadas += 1;
                let vis = visibles(&self.e, self.m);
                let cerca = vis.start.saturating_sub(1)..vis.end + 1;
                if !cerca.contains(&i) {
                    self.medida.en_vano += 1;
                }
            }
            recibir(&mut self.e, l, &self.motor, &self.textos);
        }
        pedir_hojas(&mut self.e, &self.tx, self.m);
        self.pico_bytes = self.pico_bytes.max(bytes_en_mapas(&self.e));
    }

    /// Fotogramas hasta que lo visible este nitido (o `tope`).
    fn hasta_nitida(&mut self, tope: Duration) -> bool {
        let t = Instant::now();
        while t.elapsed() < tope {
            self.fotograma();
            if nitidas(&self.e, self.m) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(8));
        }
        false
    }
}

#[test]
#[ignore = "necesita GPU y E:\\pixpin-demo\\Tesis.pdf; mide la cola de hojas"]
fn medir_la_cola_de_hojas() {
    let ruta = pdf_grande(300);
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = pixpin_render::MotorRender::nuevo(d.d3d()).expect("motor");
    let antes_mem = pixpin_shell::prioridad::memoria_pico().unwrap_or(0);

    let (tx, rx_p) = mpsc::channel::<Pedido>();
    let (tx_l, rx) = mpsc::channel::<Llega>();
    let t0 = Instant::now();
    let ruta_hilo = ruta.clone();
    let hilo = std::thread::spawn(move || {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        hilo_de_hojas(ruta_hilo, rx_p, tx_l, 0)
    });
    let mut b = Banco {
        e: estado(0),
        m: marco(),
        tx,
        rx,
        motor,
        textos: Catalogo::nuevo(pixpin_store::Idioma::Espanol),
        pico_bytes: 0,
        medida: Medida::default(),
    };
    // Abrir: hasta las medidas y la primera hoja nitida.
    while b.e.hojas.cuantas() == 0 {
        if let Ok(l) = b.rx.recv_timeout(Duration::from_secs(30)) {
            recibir(&mut b.e, l, &b.motor, &b.textos);
        } else {
            panic!("no llegaron las medidas");
        }
    }
    let ms_medidas = t0.elapsed().as_millis();
    b.e.y = 0.0;
    assert!(b.hasta_nitida(Duration::from_secs(20)), "la primera hoja");
    let ms_primera = t0.elapsed().as_millis();
    let hojas = b.e.hojas.cuantas();

    // 1. Pasar 100 hojas con la rueda, deprisa: en 40 fotogramas de 16 ms.
    b.medida = Medida::default();
    let meta = b.e.hojas.y_de(100.0);
    let paso = (meta - b.e.y) / 40.0;
    let t = Instant::now();
    for _ in 0..40 {
        b.e.y += paso;
        b.fotograma();
        std::thread::sleep(Duration::from_millis(16));
    }
    b.e.y = meta;
    assert!(b.hasta_nitida(Duration::from_secs(20)), "la hoja 100");
    b.medida.ms_hasta_nitida = t.elapsed().as_millis();
    let rueda = std::mem::take(&mut b.medida);

    // 2. Saltar 100 hojas de golpe (Fin, un marcador, la barra).
    let meta = b.e.hojas.y_de(200.0);
    let t = Instant::now();
    b.e.y = meta;
    assert!(b.hasta_nitida(Duration::from_secs(20)), "la hoja 200");
    b.medida.ms_hasta_nitida = t.elapsed().as_millis();
    let salto = std::mem::take(&mut b.medida);

    // 3. Acercar al maximo en la 200 y esperar a que este nitida.
    let t = Instant::now();
    b.e.raton = (b.m.ancho / 2.0, b.m.alto / 2.0);
    for _ in 0..30 {
        acercar(&mut b.e, 1.0, b.m);
        b.fotograma();
        std::thread::sleep(Duration::from_millis(16));
    }
    b.e.zoom_cambiado = 0;
    let nitida_zoom = b.hasta_nitida(Duration::from_secs(30));
    b.medida.ms_hasta_nitida = t.elapsed().as_millis();
    let zoom = std::mem::take(&mut b.medida);

    let pico_mem = pixpin_shell::prioridad::memoria_pico().unwrap_or(0);
    let pico_bytes = b.pico_bytes;
    drop(b);
    let _ = hilo.join();
    println!("MEDIDA hojas={hojas} medidas_ms={ms_medidas} primera_ms={ms_primera}");
    println!("MEDIDA rueda100: {rueda:?}");
    println!("MEDIDA salto100: {salto:?}");
    println!("MEDIDA zoom6: {zoom:?} nitida={nitida_zoom}");
    println!(
        "MEDIDA pico_mapas_mb={:.1} pico_proceso_mb={:.1} (antes {:.1})",
        bytes_mb(pico_bytes),
        pico_mem as f64 / 1048576.0,
        antes_mem as f64 / 1048576.0
    );
}

fn bytes_mb(b: usize) -> f64 {
    b as f64 / 1048576.0
}

// --- Lo que depende de como guarda el lector lo pintado ---

/// La hoja de una hoja entera o un trozo que llega (el texto no cuenta).
fn hoja_que_llega(l: &Llega) -> Option<usize> {
    match l {
        Llega::Hecho { trabajo, .. } => Some(trabajo.hoja()),
        _ => None,
    }
}

/// Si todo lo que se ve esta pintado a la resolucion que toca (y, de muy
/// cerca, con su trozo nitido).
fn nitidas(e: &Estado, m: Marco) -> bool {
    lista_de_trabajos(e, m, false)
        .iter()
        .all(|(p, _)| *p != cola::Prioridad::Visible)
        && visibles(e, m).all(|i| e.pintadas.contains_key(&i))
}
#[test]
#[ignore = "mide cuanto cuesta dibujar una hoja segun su ancho"]
fn medir_el_coste_por_ancho() {
    let ruta = pdf_grande(300);
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let d = pixpin_pdf::Documento::abrir(&ruta).expect("abre");
    let _ = d.renderizar(0, 256);
    for ancho in [192u32, 256, 512, 1024, 2048] {
        let t = Instant::now();
        for i in 10..30 {
            let _ = d.renderizar(i, ancho);
        }
        println!(
            "MEDIDA ancho={ancho} ms_por_hoja={:.1}",
            t.elapsed().as_secs_f64() * 1000.0 / 20.0
        );
    }
    let t = Instant::now();
    for i in 10..30 {
        let _ = d.renderizar_trozo(i, (0.25, 0.25, 0.25, 0.18), 512, 512);
    }
    println!(
        "MEDIDA trozo512 ms={:.1}",
        t.elapsed().as_secs_f64() * 1000.0 / 20.0
    );
    let bytes = std::fs::read(&ruta).unwrap();
    let t = Instant::now();
    let n = pixpin_pdf::union::contar_paginas(&bytes);
    println!(
        "MEDIDA leer_indice_ms={:.1} {n:?}",
        t.elapsed().as_secs_f64() * 1000.0
    );
}

/// Pinta el estado fuera de pantalla y lo guarda en PNG.
fn a_png(
    b: &Banco,
    fuera: &pixpin_render::fuera_de_pantalla::FueraDePantalla,
    e: &mut Estado,
    nombre: &str,
) -> PathBuf {
    b.motor
        .dibujar(&fuera.destino, |p| pintar(e, p, b.m, &b.textos))
        .expect("pintar");
    fuera.esperar_gpu().expect("esperar");
    let (ancho, alto, pixeles) = fuera.leer_rgba().expect("leer");
    let png = pixpin_codec::imagen::codificar_png(&ImagenRgba {
        ancho,
        alto,
        pixeles,
    })
    .expect("png");
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let salida = carpeta.join(nombre);
    std::fs::write(&salida, png).expect("guardar");
    println!("PNG {}", salida.display());
    salida
}

/// **El lector de verdad sobre una copia de la tesis**: el hilo de las
/// hojas real, texto elegido en una hoja con enlaces, y de muy cerca con el
/// trozo nitido. Saca PNG para mirarlos:
/// `PIXPIN_MUESTRAS=<carpeta> cargo test -p pixpin --bin pixpinmax muestra_del_lector_de_pdf_de_cerca -- --ignored --nocapture`
#[test]
#[ignore = "necesita GPU y la tesis de muestra; genera PNG"]
fn muestra_del_lector_de_pdf_de_cerca_y_eligiendo() {
    // Una copia: el lector nunca abre el original.
    let dir = std::env::temp_dir().join("pixpin-muestra-cerca");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ruta = dir.join("Tesis.pdf");
    std::fs::copy(r"E:\pixpin-demo\Tesis.pdf", &ruta).expect("copia");
    let bytes = std::fs::read(&ruta).unwrap();
    // La primera hoja con enlaces internos (el indice).
    let n = pixpin_pdf::union::contar_paginas(&bytes).unwrap() as usize;
    let con_enlaces = (0..n.min(40))
        .find(|&i| {
            pixpin_pdf::texto::hoja_con_enlaces(&bytes, i).is_ok_and(|(_, l)| {
                l.iter()
                    .any(|x| matches!(x.destino, pixpin_pdf::enlaces::Destino::Hoja { .. }))
            })
        })
        .unwrap_or(2);
    let (_, enlaces) = pixpin_pdf::texto::hoja_con_enlaces(&bytes, con_enlaces).unwrap();
    println!(
        "hoja con enlaces: {} ({} enlaces) {:?}",
        con_enlaces + 1,
        enlaces.len(),
        enlaces.first()
    );

    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = pixpin_render::MotorRender::nuevo(d.d3d()).expect("motor");
    let fuera =
        pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), 1920, 1080)
            .expect("superficie");
    let (tx, rx_p) = mpsc::channel::<Pedido>();
    let (tx_l, rx) = mpsc::channel::<Llega>();
    let ruta_hilo = ruta.clone();
    let hilo = std::thread::spawn(move || {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        hilo_de_hojas(ruta_hilo, rx_p, tx_l, 0)
    });
    let mut b = Banco {
        e: estado(0),
        m: marco(),
        tx,
        rx,
        motor,
        textos: Catalogo::nuevo(pixpin_store::Idioma::Espanol),
        pico_bytes: 0,
        medida: Medida::default(),
    };
    while b.e.hojas.cuantas() == 0 {
        let l = b.rx.recv_timeout(Duration::from_secs(30)).expect("medidas");
        recibir(&mut b.e, l, &b.motor, &b.textos);
    }
    b.e.y = b.e.hojas.arriba[con_enlaces] - 20.0;
    assert!(b.hasta_nitida(Duration::from_secs(20)));
    // El raton sobre la hoja pide su texto y sus enlaces.
    b.e.elegir.falta = Some(con_enlaces);
    let t = Instant::now();
    while !b.e.elegir.hojas.contains_key(&con_enlaces) && t.elapsed() < Duration::from_secs(20) {
        b.fotograma();
        std::thread::sleep(Duration::from_millis(8));
    }
    println!("texto de la hoja en {} ms", t.elapsed().as_millis());
    let h = b.e.elegir.hojas[&con_enlaces].clone();
    // Un triple clic en el primer renglon con letras: el renglon entero; y
    // arrastrando, unas cuantas lineas mas.
    let k = h.texto.cajas.iter().position(|c| c.is_some()).unwrap_or(0);
    let fin = (k + 400).min(h.texto.cajas.len());
    b.e.elegir.seleccion = Some(office::Seleccion {
        ancla: (con_enlaces, k),
        punta: (con_enlaces, fin),
    });
    println!(
        "elegido: {:?}",
        elegir::texto_elegido(&b.e)
            .chars()
            .take(120)
            .collect::<String>()
    );
    let mut e = std::mem::replace(&mut b.e, estado(0));
    a_png(&b, &fuera, &mut e, "lector-pdf-eligiendo.png");
    b.e = e;

    // De muy cerca, en medio de la hoja: la hoja de fondo y el trozo nitido.
    b.e.elegir.seleccion = None;
    b.e.raton = (b.m.ancho / 2.0, b.m.alto / 2.0);
    for _ in 0..30 {
        acercar(&mut b.e, 1.0, b.m);
    }
    b.e.zoom_cambiado = 0;
    assert!(b.hasta_nitida(Duration::from_secs(30)));
    println!(
        "de cerca: zoom {} trozos {} mapas {:.1} MB",
        b.e.zoom,
        b.e.trozos.len(),
        bytes_mb(bytes_en_mapas(&b.e))
    );
    let mut e = std::mem::replace(&mut b.e, estado(0));
    a_png(&b, &fuera, &mut e, "lector-pdf-de-cerca.png");
    // Y sin el trozo, para comparar: solo la hoja de fondo estirada.
    e.trozos.clear();
    a_png(&b, &fuera, &mut e, "lector-pdf-de-cerca-sin-trozo.png");
    drop(b);
    let _ = hilo.join();
    let _ = std::fs::remove_dir_all(&dir);
}

/// **El PDF exportado con tinta**, guardado para abrirlo con otro lector
/// (Edge): tres hojas de una copia de la tesis con un circulo rojo en la
/// primera. `PIXPIN_MUESTRAS=<carpeta>`.
#[test]
#[ignore = "necesita la tesis de muestra; deja un PDF para mirarlo con otro lector"]
fn muestra_del_pdf_exportado_para_otro_lector() {
    use pixpin_motor2d::gesto::Herramienta;
    let original = std::fs::read(r"E:\pixpin-demo\Tesis.pdf").expect("tesis");
    let base = pixpin_pdf::union::solo_paginas(&original, &[0, 1, 2]).expect("tres hojas");
    let mut e = estado(3);
    e.tinta.gesto.estilo.trazo = pixpin_motor2d::ColorRgba::opaco(0.88, 0.19, 0.19);
    let capa = e.capas.entry(0).or_default();
    let circulo: Vec<Punto2> = (0..=40)
        .map(|k| {
            let a = k as f32 / 40.0 * std::f32::consts::TAU;
            Punto2::nuevo(700.0 + 300.0 * a.cos(), 900.0 + 120.0 * a.sin())
        })
        .collect();
    e.tinta.trazar(capa, &circulo);
    crate::dibujo::teclas::elegir_herramienta(&mut e.tinta.gesto, Herramienta::Rectangulo);
    e.tinta.trazar(
        capa,
        &[Punto2::nuevo(300.0, 1200.0), Punto2::nuevo(1100.0, 1400.0)],
    );
    let mut tinta = HashMap::new();
    tinta.insert(0, pintado::ordenes_de_escena(&e.capas[&0].escena));
    let bytes =
        crate::compartir::pdf_anotado::con_tinta(&base, &tinta, (0.0, 0.0), &[]).expect("anotado");
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let salida = carpeta.join("exportado-con-tinta.pdf");
    std::fs::write(&salida, bytes).expect("guardar");
    println!("PDF {}", salida.display());
}
