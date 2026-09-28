//! **Lo que cuesta cada fotograma de ARRASTRAR**, medido sin ventana (la
//! queja del usuario del 27-sep: «la lupa se mueve lento», «la grafica se
//! mueve lento», «corrige todo lo que se mueve lento cuando lo muevo»).
//!
//! Reproduce, aviso a aviso, lo que hace el bucle del editor al mover lo
//! elegido: el gesto (`Gesto::evento`), la decision de si el aviso se
//! atiende en la capa de la tinta (`atender_en_capa`: la composicion lo
//! corre y la escena no se toca) y, si no, el fotograma de `pintar` (la capa
//! congelada volcada entera, lo elegido encima y la pasada de las lupas).
//! Lo que no se puede medir sin ventana —el `Commit` de DirectComposition y
//! la capa de la interfaz— se deja fuera, y se dice.
//!
//! ```text
//! cargo test --release -p pixpin --bin pixpinmax medir_el_arrastre -- --ignored --nocapture --test-threads=1
//! ```

use std::time::Instant;

use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::indice::Rejilla;
use pixpin_motor2d::vector::Punto2;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;

use super::*;

/// Una pantalla de portatil corriente a 100 %: la del equipo suelo.
const ANCHO: u32 = 1920;
const ALTO: u32 = 1080;
/// Avisos del raton por arrastre (medio segundo a 60 Hz).
const AVISOS: usize = 30;

struct Banco {
    dispositivo: pixpin_capture::Dispositivo,
    motor: MotorRender,
    destino: FueraDePantalla,
    /// La capa congelada: lo que no se mueve, horneado al pulsar.
    capa: FueraDePantalla,
    /// La capa de la tinta, donde va lo elegido cuando compone.
    tinta: FueraDePantalla,
    imagenes: ImagenesLienzo,
}

impl Banco {
    fn nuevo() -> Banco {
        let dispositivo = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(dispositivo.d3d()).expect("motor");
        let nuevo = |m: &MotorRender| {
            FueraDePantalla::nuevo(m, dispositivo.d3d(), ANCHO, ALTO).expect("textura")
        };
        let (destino, capa, tinta) = (nuevo(&motor), nuevo(&motor), nuevo(&motor));
        let imagenes = ImagenesLienzo::nuevo(motor.lado_maximo_bitmap());
        Banco {
            dispositivo,
            motor,
            destino,
            capa,
            tinta,
            imagenes,
        }
    }

    /// Pinta en `destino` los candidatos que `quien` acepta, como `pintar`.
    #[allow(clippy::too_many_arguments)]
    fn escena_en(
        &self,
        destino: &FueraDePantalla,
        escena: &Escena,
        rejilla: &Rejilla,
        cache: &mut Cache,
        cache_tinta: &mut pixpin_render::CacheTinta,
        camara: &Camara,
        limpiar: bool,
        quien: &dyn Fn(u64) -> bool,
    ) {
        let vista = camara.ventana(ANCHO as f32, ALTO as f32);
        let candidatos = rejilla.candidatos(vista);
        let imagenes = &self.imagenes;
        self.motor
            .dibujar(&destino.destino, |p| {
                if limpiar {
                    p.limpiar(crate::dibujo::pintar::a_color(escena.fondo));
                }
                let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
                p.poner_vista((0.0, 0.0), camara.zoom, (origen.x, origen.y));
                for id in candidatos {
                    if !quien(id) {
                        continue;
                    }
                    let Some(e) = elemento_por_id(escena, rejilla, id) else {
                        continue;
                    };
                    if e.borrado {
                        continue;
                    }
                    let mut indice = 0u32;
                    let grano = pixpin_motor2d::pintado::grano_de(e);
                    por_cada_orden(cache, e, camara.zoom, escena.escala.as_ref(), |orden| {
                        dibujar_orden(
                            p,
                            orden,
                            vista,
                            Some((&mut *cache_tinta, (e.id, e.version, indice))),
                            imagenes,
                            camara.zoom,
                            grano,
                        );
                        indice += 1;
                    });
                }
            })
            .expect("fotograma");
    }
}

/// Lo que cuesta un arrastre, repartido por aviso.
#[derive(Default, Debug)]
struct Arrastre {
    avisos: usize,
    gesto_ms: f64,
    /// Avisos atendidos por composicion (sin repintar la escena).
    compuestos: usize,
    /// Avisos que rehicieron el fotograma entero.
    enteros: usize,
    fotograma_ms: f64,
    fotograma_peor: f64,
    /// De `fotograma_ms`, la pasada de las lupas.
    lupas_ms: f64,
    /// Lo que costo pintar lo elegido en su capa al empezar.
    empezar_ms: f64,
    /// Cuantas veces se volvio a pintar lo de dentro de la lupa.
    rehechas: u64,
    /// De los enteros, cuantos rehicieron solo el trozo de lo elegido.
    en_zona: usize,
}

impl Arrastre {
    fn por_aviso(&self) -> f64 {
        (self.gesto_ms + self.fotograma_ms) / self.avisos.max(1) as f64
    }
}

/// Un punto que agarra lo elegido: el primero de una rejilla por las cajas
/// de lo elegido que el picado reconoce.
fn punto_que_agarra(escena: &Escena, gesto: &Gesto, escala: f32) -> Punto2 {
    // Ni el borde de la caja ni un tirador: ahi se estira, no se mueve.
    let tiradores = gesto.tiradores(escena, escala);
    for id in gesto.seleccion.ids() {
        let Some(e) = escena.buscar(*id) else { continue };
        let (x0, y0, x1, y1) = e.caja();
        for i in 1..8 {
            for j in 1..8 {
                let p = Punto2::nuevo(
                    x0 + (x1 - x0) * i as f32 / 8.0,
                    y0 + (y1 - y0) * j as f32 / 8.0,
                );
                if pixpin_motor2d::impacto::toca(e, p)
                    && tiradores.as_ref().is_none_or(|t| t.en(p, escala).is_none())
                {
                    return p;
                }
            }
        }
    }
    panic!("nada de lo elegido se deja agarrar");
}

/// **Un arrastre de lo elegido** con lo que haria el bucle del editor en
/// cada aviso. `camara` es la efectiva; la escena se devuelve como estaba.
fn arrastrar(
    b: &mut Banco,
    escena: &mut Escena,
    ids: &[u64],
    camara: &Camara,
    estirar: bool,
) -> Arrastre {
    let copia = escena.clone();
    let mut gesto = Gesto::nuevo();
    gesto.herramienta = Herramienta::Mano;
    gesto.seleccion.poner_todos(ids.iter().copied());
    let mut rejilla = Rejilla::nueva();
    rejilla.sincronizar(escena);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    cache_tinta.fijar_escala(camara.zoom);
    let escala = 1.0 / camara.zoom;
    // Calentar: la escena entera una vez, como la ve el usuario antes.
    b.escena_en(&b.destino, escena, &rejilla, &mut cache, &mut cache_tinta, camara, true, &|_| true);
    let _ = lupas::pasar(
        &mut b.motor,
        &b.destino.destino,
        escena,
        camara,
        None,
        &b.imagenes,
        ANCHO,
        ALTO,
        &|_| false,
    );
    b.destino.esperar_gpu().expect("GPU");

    // Estirar: por la esquina de abajo a la derecha (el tirador sureste).
    let desde = if estirar {
        gesto.tiradores(escena, escala).expect("tiradores").tamano[4].1
    } else {
        punto_que_agarra(escena, &gesto, escala)
    };
    gesto.evento(
        EventoGesto::Pulsar {
            p: desde,
            shift: false,
            alt: false,
            presion: None,
        },
        escena,
        escala,
    );
    let rehechas_antes = lupas::rehechas();
    let mut a = Arrastre::default();
    let mut en_capa = false;
    let mut capa_lista = false;
    let mut zona_antes: Option<(i32, i32, i32, i32)> = None;
    for i in 1..=AVISOS {
        let q = Punto2::nuevo(desde.x + i as f32 * 6.0 * escala, desde.y + i as f32 * 3.0 * escala);
        let t = Instant::now();
        gesto.evento(
            EventoGesto::Mover {
                p: q,
                shift: false,
                alt: false,
                presion: None,
            },
            escena,
            escala,
        );
        a.gesto_ms += t.elapsed().as_secs_f64() * 1000.0;
        a.avisos += 1;
        // La misma criba que `atender_en_capa`.
        let caja = gesto.seleccion.caja(escena);
        let dentro = caja.is_some_and(|c| {
            let r = caja_en_pantalla(camara, c, 16.0);
            r.0 >= 0 && r.1 >= 0 && r.2 <= ANCHO as i32 && r.3 <= ALTO as i32
        });
        let compone = en_capa
            || (gesto.moviendo() && gesto.flechas_que_siguen().is_empty() && dentro);
        if compone {
            if !en_capa {
                // Empezar: lo elegido, una vez, en su capa.
                let t = Instant::now();
                let sel = gesto.seleccion.clone();
                b.escena_en(&b.tinta, escena, &rejilla, &mut cache, &mut cache_tinta, camara, true, &|id| {
                    sel.contiene(id)
                });
                b.tinta.esperar_gpu().expect("GPU");
                a.empezar_ms = t.elapsed().as_secs_f64() * 1000.0;
                en_capa = true;
            }
            a.compuestos += 1;
            continue;
        }
        // El fotograma entero de `pintar`: la capa congelada (horneada una
        // vez, al primer aviso: `capa.preparar`), volcada entera, lo elegido
        // encima sin cache de tinta y la pasada de las lupas.
        rejilla.sincronizar(escena);
        if !capa_lista {
            let sel = gesto.seleccion.clone();
            b.escena_en(&b.capa, escena, &rejilla, &mut cache, &mut cache_tinta, camara, true, &|id| {
                !sel.contiene(id)
            });
            b.capa.esperar_gpu().expect("GPU");
            capa_lista = true;
        }
        let t = Instant::now();
        // Estirando: solo el trozo de lo elegido, el de antes y el de ahora
        // (`congelar::zona_al_transformar`), como el bucle del editor.
        let ahora = congelar::zona_al_transformar(escena, &gesto, camara, &mut cache);
        let zona = match (ahora, zona_antes) {
            (Some(z), Some(p)) if capa_lista => Some((p.0.min(z.0), p.1.min(z.1), p.2.max(z.2), p.3.max(z.3))),
            _ => None,
        };
        zona_antes = ahora;
        a.en_zona += zona.is_some() as usize;
        b.destino.copiar_desde(&b.capa, zona).expect("volcar");
        let sel = gesto.seleccion.clone();
        let vista = camara.ventana(ANCHO as f32, ALTO as f32);
        let imagenes = &b.imagenes;
        b.motor
            .dibujar(&b.destino.destino, |p| {
                let origen = camara.a_pantalla(Punto2::nuevo(0.0, 0.0));
                p.poner_vista((0.0, 0.0), camara.zoom, (origen.x, origen.y));
                for e in escena.elementos.iter().filter(|e| sel.contiene(e.id) && !e.borrado) {
                    let grano = pixpin_motor2d::pintado::grano_de(e);
                    por_cada_orden(&mut cache, e, camara.zoom, escena.escala.as_ref(), |orden| {
                        dibujar_orden(p, orden, vista, None, imagenes, camara.zoom, grano);
                    });
                }
            })
            .expect("lo elegido");
        let tl = Instant::now();
        let _ = lupas::pasar(
            &mut b.motor,
            &b.destino.destino,
            escena,
            camara,
            None,
            &b.imagenes,
            ANCHO,
            ALTO,
            &|_| false,
        );
        b.destino.esperar_gpu().expect("GPU");
        let f = t.elapsed().as_secs_f64() * 1000.0;
        a.lupas_ms += tl.elapsed().as_secs_f64() * 1000.0;
        a.enteros += 1;
        a.fotograma_ms += f;
        a.fotograma_peor = a.fotograma_peor.max(f);
    }
    a.rehechas = lupas::rehechas() - rehechas_antes;
    *escena = copia;
    a
}

/// La escena «normal»: la sintetica de 300 elementos (trazos, formas con
/// sombreado, flechas y rotulos) y, en la parte que se ve, una grafica, una
/// tabla, un grupo de cuarenta figuras, dos figuras sueltas y una lupa.
struct Lamina {
    escena: Escena,
    camara: Camara,
    grafica: Vec<u64>,
    tabla: Vec<u64>,
    grupo: Vec<u64>,
    suelta: u64,
    lupa: u64,
}

fn lamina(b: &Banco) -> Lamina {
    use pixpin_motor2d::grafica::{self, Peticion};
    pixpin_motor2d::texto::instalar_medidor(crate::dibujo::pintar::medir_para_el_motor);
    let mut escena = super::medir::escena_sintetica(300);
    let mut gesto = Gesto::nuevo();
    let medir = figuras::medidor(&b.motor, pixpin_motor2d::texto::nombre_de_familia(None));
    let estilo = figuras::estilo_del_pincel(&gesto);
    let p = Peticion {
        formulas: vec!["sin(x)/x".into(), "sqrt(x+4) - 1".into(), "x^2/4 si x < 0; -x/2 si x >= 0".into()],
        x_desde: -6.0,
        x_hasta: 6.0,
        y_desde: -3.0,
        y_hasta: 3.0,
        escala: 50.0,
    };
    let v = grafica::elementos(&p, &estilo, &medir).expect("grafica");
    figuras::estampar_en_la_vista(&mut escena, &mut gesto, Punto2::nuevo(700.0, 500.0), "grafica", &v);
    let grafica = gesto.seleccion.ids().to_vec();
    let filas: Vec<Vec<String>> = (0..8)
        .map(|f| (0..5).map(|c| format!("celda {f},{c}")).collect())
        .collect();
    let v = pixpin_motor2d::tabla_dibujada::elementos_de_tabla(&filas, &estilo, Punto2::nuevo(0.0, 0.0), &medir, true);
    figuras::estampar_en_la_vista(&mut escena, &mut gesto, Punto2::nuevo(1500.0, 500.0), "tabla", &v);
    let tabla = gesto.seleccion.ids().to_vec();
    let mut piezas = Vec::new();
    for i in 0..40 {
        let mut e = Elemento {
            figura: if i % 2 == 0 { Figura::Rectangulo } else { Figura::Elipse },
            x: (i % 8) as f32 * 40.0,
            y: (i / 8) as f32 * 40.0,
            ancho: 34.0,
            alto: 34.0,
            trazo: ColorRgba::opaco(0.1, 0.3, 0.7),
            relleno: Some(ColorRgba::opaco(0.8, 0.9, 1.0)),
            grosor: 2.0,
            ..Default::default()
        };
        e.semilla = 7 + i;
        piezas.push(e);
    }
    figuras::estampar_en_la_vista(&mut escena, &mut gesto, Punto2::nuevo(700.0, 1000.0), "grupo", &piezas);
    let grupo = gesto.seleccion.ids().to_vec();
    let suelta = escena.anadir(Elemento {
        figura: Figura::Rectangulo,
        x: 1300.0,
        y: 950.0,
        ancho: 120.0,
        alto: 80.0,
        trazo: ColorRgba::opaco(0.8, 0.2, 0.2),
        relleno: Some(ColorRgba::opaco(1.0, 0.9, 0.9)),
        grosor: 2.0,
        ..Default::default()
    });
    // La lupa: un circulo sobre la grafica convertido con la varita y
    // apartado a la derecha, como la deja el usuario.
    let circulo = escena.anadir(Elemento {
        figura: Figura::Elipse,
        x: 600.0,
        y: 420.0,
        ancho: 160.0,
        alto: 160.0,
        trazo: ColorRgba::opaco(0.2, 0.2, 0.2),
        grosor: 2.0,
        ..Default::default()
    });
    let lupa = crate::dibujo::lupa::convertir(&mut escena, &mut gesto, circulo).expect("lupa");
    escena.mover(lupa, 400.0, 250.0);
    let camara = Camara {
        x: 0.0,
        y: 0.0,
        zoom: 0.6,
    };
    let _ = ALTO;
    Lamina {
        escena,
        camara,
        grafica,
        tabla,
        grupo,
        suelta,
        lupa,
    }
}

fn escribir(que: &str, a: &Arrastre) {
    println!(
        "{que:<40} {:>6.2} ms/aviso | gesto {:>6.3} ms | compuestos {:>2}/{:<2} enteros {:>2} (media {:>6.2}, peor {:>6.2}, lupas {:>6.2}) | empezar {:>6.2} ms | lupa rehecha {} | en zona {}",
        a.por_aviso(),
        a.gesto_ms / a.avisos.max(1) as f64,
        a.compuestos,
        a.avisos,
        a.enteros,
        a.fotograma_ms / a.enteros.max(1) as f64,
        a.fotograma_peor,
        a.lupas_ms / a.enteros.max(1) as f64,
        a.empezar_ms,
        a.rehechas,
        a.en_zona,
    );
}

#[test]
#[ignore = "necesita GPU real; ejecutar en --release con --ignored --nocapture"]
fn medir_el_arrastre_de_lupa_grafica_tabla_grupo_y_sueltas() {
    let mut b = Banco::nuevo();
    let l = lamina(&b);
    let mut escena = l.escena.clone();
    // La pasada de las lupas con nada cambiado: solo copiar lo de dentro.
    {
        let _ = lupas::pasar(&mut b.motor, &b.destino.destino, &escena, &l.camara, None, &b.imagenes, ANCHO, ALTO, &|_| false);
        b.destino.esperar_gpu().expect("GPU");
        let antes = lupas::rehechas();
        let t = Instant::now();
        for _ in 0..AVISOS {
            let _ = lupas::pasar(&mut b.motor, &b.destino.destino, &escena, &l.camara, None, &b.imagenes, ANCHO, ALTO, &|_| false);
            b.destino.esperar_gpu().expect("GPU");
        }
        println!(
            "pasada de lupas sin cambios: {:.2} ms (rehechas {})",
            t.elapsed().as_secs_f64() * 1000.0 / AVISOS as f64,
            lupas::rehechas() - antes
        );
    }
    let casos: [(&str, Vec<u64>); 5] = [
        ("lupa", vec![l.lupa]),
        ("grafica", l.grafica.clone()),
        ("tabla", l.tabla.clone()),
        ("grupo de 40", l.grupo.clone()),
        ("figura suelta", vec![l.suelta]),
    ];
    for con_clavos in [false, true] {
        if con_clavos {
            // Un clavo lejos de todo lo que se arrastra: dos trazos de la
            // esquina de abajo, fuera de la vista.
            let a = escena.elementos[0].clone();
            let (x0, y0, x1, y1) = a.caja();
            let _ = (x0, y0);
            {
                // Dos rayas cruzadas fuera de la vista, clavadas en su cruce.
                let r = |x0: f32, y0: f32, x1: f32, y1: f32| Elemento {
                    figura: Figura::Linea {
                        puntos: vec![Punto2::nuevo(x0, y0), Punto2::nuevo(x1, y1)],
                    },
                    x: x0.min(x1),
                    y: y0.min(y1),
                    ancho: (x1 - x0).abs(),
                    alto: (y1 - y0).abs(),
                    grosor: 2.0,
                    ..Default::default()
                };
                escena.anadir(r(x1 + 5000.0, y1, x1 + 5100.0, y1 + 100.0));
                escena.anadir(r(x1 + 5000.0, y1 + 100.0, x1 + 5100.0, y1));
                assert!(pixpin_motor2d::nudos::soldar(
                    &mut escena,
                    Punto2::nuevo(x1 + 5050.0, y1 + 50.0),
                    10.0
                ));
            }
            assert!(!escena.alfileres.is_empty());
        }
        let clavos = if con_clavos { "con clavo lejos" } else { "sin clavo" };
        for (que, ids) in &casos {
            let a = arrastrar(&mut b, &mut escena, ids, &l.camara, false);
            escribir(&format!("mover {que} ({clavos})"), &a);
        }
        for (que, ids) in casos.iter().filter(|(_, ids)| ids.len() > 1) {
            let a = arrastrar(&mut b, &mut escena, ids, &l.camara, true);
            escribir(&format!("estirar {que} ({clavos})"), &a);
        }
    }
    // Apuntar la lupa a otro sitio (`MoviendoElFoco`): cada aviso cambia lo
    // mirado, asi que lo de dentro se rehace si o si; lo que se mide es el
    // resto del fotograma.
    let a = apuntar_la_lupa(&mut b, &mut escena, l.lupa, &l.camara);
    escribir("apuntar la lupa (mover lo mirado)", &a);
    lupas::olvidar();
    let _ = &b.dispositivo;
}

/// **Arrastrar lo mirado de una lupa** como lo hace el bucle del editor:
/// `ManoDeLupa` cambia el foco y pide el fotograma entero. Con `capa`, lo
/// quieto sale de la capa congelada (horneada sin la lupa, al empezar); sin
/// ella, se pinta la escena entera en cada aviso.
fn apuntar_la_lupa(b: &mut Banco, escena: &mut Escena, id: u64, camara: &Camara) -> Arrastre {
    let copia = escena.clone();
    let mut rejilla = Rejilla::nueva();
    rejilla.sincronizar(escena);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    cache_tinta.fijar_escala(camara.zoom);
    b.escena_en(&b.destino, escena, &rejilla, &mut cache, &mut cache_tinta, camara, true, &|_| true);
    let _ = lupas::pasar(&mut b.motor, &b.destino.destino, escena, camara, None, &b.imagenes, ANCHO, ALTO, &|_| false);
    b.destino.esperar_gpu().expect("GPU");
    let con_capa = ventana_editor_apunta_con_capa();
    if con_capa {
        b.escena_en(&b.capa, escena, &rejilla, &mut cache, &mut cache_tinta, camara, true, &|x| x != id);
        b.capa.esperar_gpu().expect("GPU");
    }
    let mut a = Arrastre::default();
    for _ in 0..AVISOS {
        let t = Instant::now();
        let antes = lupas::huella_elegida(escena, &[id], camara);
        if let Some(e) = escena.buscar_mut(id)
            && let Figura::Lupa { cristal } = &mut e.figura
        {
            let f = cristal.foco.unwrap_or(Punto2::nuevo(0.0, 0.0));
            cristal.foco = Some(Punto2::nuevo(f.x + 4.0, f.y + 2.0));
            e.tocar();
        }
        a.gesto_ms += t.elapsed().as_secs_f64() * 1000.0;
        a.avisos += 1;
        let t = Instant::now();
        rejilla.sincronizar(escena);
        if con_capa {
            // Solo el trozo de la lupa, el de antes y el de ahora.
            let ahora = lupas::huella_elegida(escena, &[id], camara);
            let u = [antes, ahora].into_iter().flatten().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |s, c| {
                (s.0.min(c.0), s.1.min(c.1), s.2.max(c.2), s.3.max(c.3))
            });
            let zona = (u.0.max(0.0) as i32, u.1.max(0.0) as i32, (u.2 as i32).min(ANCHO as i32), (u.3 as i32).min(ALTO as i32));
            b.destino.copiar_desde(&b.capa, Some(zona)).expect("volcar");
            b.escena_en(&b.destino, escena, &rejilla, &mut cache, &mut cache_tinta, camara, false, &|x| x == id);
        } else {
            b.escena_en(&b.destino, escena, &rejilla, &mut cache, &mut cache_tinta, camara, true, &|_| true);
        }
        let tl = Instant::now();
        let _ = lupas::pasar(&mut b.motor, &b.destino.destino, escena, camara, None, &b.imagenes, ANCHO, ALTO, &|_| false);
        b.destino.esperar_gpu().expect("GPU");
        a.lupas_ms += tl.elapsed().as_secs_f64() * 1000.0;
        let f = t.elapsed().as_secs_f64() * 1000.0;
        a.enteros += 1;
        a.fotograma_ms += f;
        a.fotograma_peor = a.fotograma_peor.max(f);
    }
    *escena = copia;
    a
}

/// Si el editor hornea la capa congelada al apuntar una lupa (lo que hace
/// el bucle de verdad: ver `ventana_editor.rs`, «apuntando una lupa»).
fn ventana_editor_apunta_con_capa() -> bool {
    true
}
