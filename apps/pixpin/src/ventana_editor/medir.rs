//! El fotograma del EDITOR medido sin ventana, como ya se medía el del
//! universo (`universo/sesion/medir.rs`).
//!
//! Separa lo que cuesta la CPU en decidir qué se ve (`preparar`: la rejilla
//! y sus candidatos) de lo que cuesta encargarle a Direct2D cada primitiva
//! (`encargar`), y de lo que tarda de verdad con la GPU terminada (`total`).
//! Y cuenta los objetos de Direct2D/DirectWrite creados por fotograma
//! (`MotorRender::contadores`), que es el número que la investigación del
//! 2026-09-19 quería ver en cero durante un paneo.
//!
//! Va con `#[ignore]` porque necesita GPU: se ejecuta a mano, en `--release`,
//!
//! ```text
//! cargo test --release -p pixpin --bin pixpinmax medir_fotograma_del_editor -- --ignored --nocapture --test-threads=1
//! ```

use std::time::Instant;

use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::indice::Rejilla;
use pixpin_motor2d::tinta::OpcionesTinta;
use pixpin_motor2d::{ColorRgba, Elemento, EstiloRelleno, EstiloTrazo, Figura};
use pixpin_render::fuera_de_pantalla::FueraDePantalla;

use super::*;

/// La pantalla del equipo del usuario al 150 %, igual que el banco del
/// universo: los dos números son comparables entre sí.
const ANCHO: u32 = 3000;
const ALTO: u32 = 2000;
const ESCALA: u32 = 150;
/// Fotogramas por guion.
const VUELTAS: usize = 60;

/// Milisegundos medios por fotograma, el peor total, y cuántos objetos de
/// Direct2D/DirectWrite se crearon de media en cada uno.
#[derive(Debug, Clone, Copy, Default)]
struct Medida {
    preparar: f64,
    encargar: f64,
    total: f64,
    peor: f64,
    objetos: f64,
}

struct Banco {
    motor: MotorRender,
    destino: FueraDePantalla,
    imagenes: ImagenesLienzo,
}

impl Banco {
    fn nuevo() -> Banco {
        let dispositivo = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(dispositivo.d3d()).expect("motor");
        let destino =
            FueraDePantalla::nuevo(&motor, dispositivo.d3d(), ANCHO, ALTO).expect("destino");
        let imagenes = ImagenesLienzo::nuevo(motor.lado_maximo_bitmap());
        Banco {
            motor,
            destino,
            imagenes,
        }
    }

    /// Un fotograma de la ESCENA: lo mismo que hornea `CapaEstatica::preparar`
    /// y lo mismo que repinta `pintar` cuando la cámara se mueve. No incluye
    /// la barra ni el panel: son unas decenas de primitivas fijas que no
    /// cambian con el tamaño del documento, y meterlas solo añadiría ruido.
    ///
    /// `formas_cacheadas` a `false` reproduce el camino de ANTES de A1: la
    /// tinta con su realizacion (D121) y las formas de rough.js creando su
    /// geometria en cada fotograma. Es lo que hace que el banco imprima el
    /// antes y el despues en la misma vuelta, con la misma GPU, la misma
    /// escena y la misma camara: dos ejecuciones distintas no serian
    /// comparables.
    fn fotograma(
        &self,
        escena: &Escena,
        rejilla: &Rejilla,
        cache: &mut Cache,
        cache_tinta: &mut pixpin_render::CacheTinta,
        camara: &Camara,
        formas_cacheadas: bool,
    ) -> (f64, f64, f64, u32) {
        let efectiva = crate::navegacion::vista_efectiva(camara, ESCALA);
        let vista = efectiva.ventana(ANCHO as f32, ALTO as f32);
        let t0 = Instant::now();
        let candidatos = rejilla.candidatos(vista);
        let t1 = Instant::now();
        self.motor
            .dibujar(&self.destino.destino, |p| {
                p.limpiar(Color::BLANCO);
                let origen = efectiva.a_pantalla(Punto2::nuevo(0.0, 0.0));
                p.poner_vista((0.0, 0.0), efectiva.zoom, (origen.x, origen.y));
                for id in &candidatos {
                    let Some(e) = escena.buscar(*id) else {
                        continue;
                    };
                    if e.borrado {
                        continue;
                    }
                    let mut indice = 0u32;
                    por_cada_orden(cache, e, efectiva.zoom, escena.escala.as_ref(), |orden| {
                        let con_cache = formas_cacheadas || matches!(orden, Orden::Tinta { .. });
                        dibujar_orden(
                            p,
                            orden,
                            vista,
                            con_cache.then_some((&mut *cache_tinta, (e.id, e.version, indice))),
                            &self.imagenes,
                            efectiva.zoom,
                        );
                        indice += 1;
                    });
                }
            })
            .expect("fotograma");
        let objetos = self.motor.contadores().total();
        let t2 = Instant::now();
        self.destino.esperar_gpu().expect("GPU");
        let t3 = Instant::now();
        let ms = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1000.0;
        (ms(t0, t1), ms(t1, t2), ms(t0, t3), objetos)
    }

    /// `VUELTAS` fotogramas con las cámaras que diga `camara(i)`.
    ///
    /// Las cachés (la de órdenes y la de realizaciones) se crean aquí y
    /// viven todo el guion: es lo que hace el editor, y medir con cachés
    /// frías mediría el primer fotograma sesenta veces.
    fn medir(
        &self,
        escena: &Escena,
        formas_cacheadas: bool,
        camara: impl Fn(usize) -> Camara,
    ) -> Medida {
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(escena);
        let mut cache = Cache::nueva();
        let mut cache_tinta = pixpin_render::CacheTinta::nueva();
        cache_tinta.fijar_escala(crate::navegacion::vista_efectiva(&camara(0), ESCALA).zoom);
        // Calentar: teselaciones, pinceles, disposiciones de texto. Sin esto
        // el primero paga toda la escena y la media no diría nada del paneo.
        for i in 0..3 {
            self.fotograma(
                escena,
                &rejilla,
                &mut cache,
                &mut cache_tinta,
                &camara(i),
                formas_cacheadas,
            );
        }
        let mut m = Medida::default();
        for i in 0..VUELTAS {
            let (a, b, t, o) = self.fotograma(
                escena,
                &rejilla,
                &mut cache,
                &mut cache_tinta,
                &camara(i),
                formas_cacheadas,
            );
            m.preparar += a;
            m.encargar += b;
            m.total += t;
            m.objetos += o as f64;
            m.peor = m.peor.max(t);
        }
        let n = VUELTAS as f64;
        m.preparar /= n;
        m.encargar /= n;
        m.total /= n;
        m.objetos /= n;
        m
    }
}

/// Un elemento con todo lo que no interesa puesto a lo de siempre.
fn base(id: u64, figura: Figura, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
    Elemento {
        id,
        figura,
        x,
        y,
        ancho,
        alto,
        angulo: 0.0,
        trazo: ColorRgba::opaco(0.1, 0.1, 0.1),
        relleno: None,
        estilo_relleno: EstiloRelleno::Rayado,
        grosor: 2.0,
        estilo: EstiloTrazo::Solido,
        rugosidad: 1.0,
        opacidad: 1.0,
        // La semilla varía con el id: rough.js dibuja cada forma distinta y
        // una escena con todas iguales mediría un caso que no existe.
        semilla: 1 + id as u32 * 2_654_435_761u32.wrapping_rem(1_000_003),
        version: 1,
        borrado: false,
        grupos: Vec::new(),
        bloqueado: false,
        enlace: None,
        redondo: false,
    }
}

/// Un trazo a mano de `puntos` puntos, ondulado, dentro de una caja de
/// 220 x 160 en `(x, y)`.
fn trazo(id: u64, x: f32, y: f32, puntos: usize) -> Elemento {
    let p: Vec<Punto2> = (0..puntos)
        .map(|i| {
            let t = i as f32 / puntos as f32;
            Punto2::nuevo(
                x + t * 220.0,
                y + 80.0 + 60.0 * (t * 9.0 + id as f32).sin() + 12.0 * (t * 41.0).cos(),
            )
        })
        .collect();
    let mut e = base(id, Figura::Rectangulo, x, y, 220.0, 160.0);
    e.figura = Figura::Lapiz {
        presiones: (0..puntos)
            .map(|i| 0.4 + 0.5 * (i as f32 / puntos as f32 * 3.0).sin().abs())
            .collect(),
        puntos: p,
        opciones: Some(OpcionesTinta::default()),
    };
    e
}

/// Una escena con el reparto que pidió la investigación: mitad trazos a
/// mano, una cuarta parte formas de rough.js CON relleno de sombreado (lo
/// caro), y el resto flechas y textos.
fn escena_sintetica(cuantos: usize) -> Escena {
    let mut escena = Escena::nueva();
    // Rejilla ancha: a la cámara del guion se ven unos cuantos cientos, que
    // es el caso real. Todos encima seria medir otra cosa.
    let por_fila = (cuantos as f32).sqrt().ceil() as usize;
    for i in 0..cuantos {
        let id = i as u64 + 1;
        let (fx, fy) = ((i % por_fila) as f32, (i / por_fila) as f32);
        let (x, y) = (fx * 260.0, fy * 200.0);
        let e = match i % 4 {
            0 | 1 => trazo(id, x, y, 90),
            2 => {
                let mut e = if i % 8 == 2 {
                    base(id, Figura::Rectangulo, x, y, 200.0, 140.0)
                } else {
                    base(id, Figura::Elipse, x, y, 200.0, 140.0)
                };
                // El relleno de sombreado es lo que genera decenas de
                // polilíneas por forma: es el peor caso de rough.js.
                e.relleno = Some(ColorRgba::opaco(0.85, 0.9, 1.0));
                e
            }
            _ => {
                if i % 8 == 3 {
                    let mut e = base(id, Figura::Rectangulo, x, y, 240.0, 40.0);
                    e.figura = Figura::Texto {
                        texto: format!("Un rótulo del elemento número {i}"),
                        tam: 20.0,
                        familia: "Segoe UI".into(),
                    };
                    e
                } else {
                    let mut e = base(id, Figura::Rectangulo, x, y, 220.0, 120.0);
                    e.figura = Figura::Flecha {
                        puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(220.0, 120.0)],
                        punta_inicio: false,
                        punta_fin: true,
                    };
                    e
                }
            }
        };
        escena.elementos.push(e);
    }
    escena.siguiente_id = cuantos as u64 + 1;
    escena
}

/// Una cámara que encaja la escena entera y luego se acerca `factor`.
fn camara_de(escena: &Escena, factor: f32) -> Camara {
    let caja = escena.caja().unwrap_or((0.0, 0.0, 100.0, 100.0));
    let e = crate::navegacion::escala_de(ESCALA);
    let (w, h) = (ANCHO as f32 / e, ALTO as f32 / e);
    let c = Camara::encajar(caja, w, h, 1.0);
    let centro = (c.x + w / (2.0 * c.zoom), c.y + h / (2.0 * c.zoom));
    let zoom = c.zoom * factor;
    Camara {
        x: centro.0 - w / (2.0 * zoom),
        y: centro.1 - h / (2.0 * zoom),
        zoom,
    }
}

fn escribir(nombre: &str, m: Medida) {
    println!(
        "{nombre:<38} preparar {:>6.2} ms | encargar {:>7.2} ms | CPU {:>7.2} ms | total {:>7.2} ms (peor {:>7.2}) | objetos D2D/fotograma {:>8.1}",
        m.preparar,
        m.encargar,
        m.preparar + m.encargar,
        m.total,
        m.peor,
        m.objetos
    );
}

/// Los tres guiones: cámara quieta, paneo de 60 fotogramas a 30 px lógicos
/// (un arrastre rápido: 1.800 px/s a 60 Hz) y zoom continuo de ×1 a ×2 sin
/// reposar (mientras el zoom no reposa, las realizaciones se estiran: D122).
fn medir_todo(b: &Banco, escena: &Escena, que: &str) {
    let quieta = camara_de(escena, 4.0);
    let paneo = move |i: usize| Camara {
        x: quieta.x + i as f32 * 30.0 / quieta.zoom,
        y: quieta.y + i as f32 * 12.0 / quieta.zoom,
        ..quieta
    };
    let zoom = move |i: usize| Camara {
        zoom: quieta.zoom * (1.0 + i as f32 / VUELTAS as f32),
        ..quieta
    };
    for (como, cacheadas) in [("antes", false), ("ahora", true)] {
        escribir(
            &format!("[{como}] {que} quieta"),
            b.medir(escena, cacheadas, |_| quieta),
        );
        escribir(
            &format!("[{como}] {que} paneo"),
            b.medir(escena, cacheadas, paneo),
        );
        escribir(
            &format!("[{como}] {que} zoom continuo"),
            b.medir(escena, cacheadas, zoom),
        );
    }
}

#[test]
#[ignore = "necesita GPU real; ejecutar en --release con --ignored --nocapture"]
fn medir_fotograma_del_editor_a_3000_por_2000() {
    let b = Banco::nuevo();
    for cuantos in [200usize, 2_000, 10_000] {
        let escena = escena_sintetica(cuantos);
        medir_todo(&b, &escena, &format!("mezcla {cuantos}"));
    }
}

#[test]
fn la_escena_sintetica_trae_de_todo_y_la_mitad_son_trazos() {
    let e = escena_sintetica(200);
    assert_eq!(e.elementos.len(), 200);
    let cuenta = |f: fn(&Figura) -> bool| e.elementos.iter().filter(|x| f(&x.figura)).count();
    assert_eq!(cuenta(|f| matches!(f, Figura::Lapiz { .. })), 100);
    assert!(cuenta(|f| matches!(f, Figura::Texto { .. })) > 0, "textos");
    assert!(
        cuenta(|f| matches!(f, Figura::Flecha { .. })) > 0,
        "flechas"
    );
    // Caso negativo: las formas cerradas van CON relleno, que es lo caro;
    // sin esto el banco mediría rough.js sin sombreado y no diría nada.
    assert!(
        e.elementos
            .iter()
            .filter(|x| matches!(x.figura, Figura::Elipse | Figura::Rectangulo))
            .all(|x| x.relleno.is_some()),
        "las formas cerradas del banco tienen que llevar relleno"
    );
}
