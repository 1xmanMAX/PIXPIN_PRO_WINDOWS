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
    /// B2: la capa de la tinta viva. Fuera de pantalla es otra textura del
    /// tamano de la VENTANA (la de verdad es una `IDCompositionSurface`, que
    /// no se puede crear sin ventana); lo que se mide con ella es lo unico
    /// que cambia entre los dos caminos: sobre que se pinta el trazo y
    /// cuanto hay que mover antes de pintarlo.
    tinta: FueraDePantalla,
    imagenes: ImagenesLienzo,
}

impl Banco {
    fn nuevo() -> Banco {
        let dispositivo = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(dispositivo.d3d()).expect("motor");
        let destino =
            FueraDePantalla::nuevo(&motor, dispositivo.d3d(), ANCHO, ALTO).expect("destino");
        let tinta = FueraDePantalla::nuevo(&motor, dispositivo.d3d(), ANCHO, ALTO).expect("tinta");
        let imagenes = ImagenesLienzo::nuevo(motor.lado_maximo_bitmap());
        Banco {
            motor,
            destino,
            tinta,
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

impl Banco {
    /// El mismo guion de cámara, pero con A3: se pinta solo cuando
    /// `transformada_de_camara` dice que el colchón ya no da.
    ///
    /// Devuelve la medida REPARTIDA entre los `VUELTAS` fotogramas del
    /// guion (que es lo que cuesta de media un fotograma de paneo) y cuántos
    /// repintados hubo. Los fotogramas compuestos no se miden porque no hay
    /// nada que medir: ni recorrido de escena, ni primitivas, ni present.
    ///
    /// **Lo que este número se deja fuera, dicho claro:** el repintado de
    /// verdad cubre la ventana MÁS el colchón (a 3000x2000 con 256 px de
    /// colchón, un 47 % más de superficie y de candidatos), y aquí se pinta
    /// solo la ventana, porque el destino del banco es del tamaño de la
    /// pantalla. Así que el coste por repintado está subestimado en ese
    /// orden; el número de repintados, que es donde está la ganancia, no.
    fn medir_por_composicion(
        &self,
        escena: &Escena,
        camara: impl Fn(usize) -> Camara,
    ) -> (Medida, usize) {
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(escena);
        let mut cache = Cache::nueva();
        let mut cache_tinta = pixpin_render::CacheTinta::nueva();
        cache_tinta.fijar_escala(crate::navegacion::vista_efectiva(&camara(0), ESCALA).zoom);
        for i in 0..3 {
            self.fotograma(
                escena,
                &rejilla,
                &mut cache,
                &mut cache_tinta,
                &camara(i),
                true,
            );
        }
        let mut m = Medida::default();
        let mut repintados = 0usize;
        let mut pintada = crate::navegacion::vista_efectiva(&camara(0), ESCALA);
        for i in 0..VUELTAS {
            let efectiva = crate::navegacion::vista_efectiva(&camara(i), ESCALA);
            if crate::ventana_editor::transformada_de_camara(
                &pintada,
                &efectiva,
                ANCHO as f32,
                ALTO as f32,
                MARGEN_ESCENA as f32,
            )
            .is_some()
            {
                continue;
            }
            repintados += 1;
            pintada = efectiva;
            let (a, b, t, o) = self.fotograma(
                escena,
                &rejilla,
                &mut cache,
                &mut cache_tinta,
                &camara(i),
                true,
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
        (m, repintados)
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
    // A3: el mismo paneo, pero pintando SOLO cuando el colchon se agota.
    // Los fotogramas que no se pintan cuestan una matriz y un `Commit`, que
    // no se miden aquí porque no tocan ni la CPU del recorrido ni la GPU:
    // la media por fotograma es la de los repintados dividida entre los 60
    // fotogramas del guion, que es exactamente lo que siente la mano.
    let m = b.medir_por_composicion(escena, paneo);
    escribir(&format!("[A3] {que} paneo por composicion"), m.0);
    println!(
        "{:<38} {} repintados de {VUELTAS} fotogramas (colchon {} px)",
        format!("[A3] {que} paneo por composicion"),
        m.1,
        MARGEN_ESCENA
    );
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

/// Cuantos puntos tiene el trazo que se esta dibujando en el banco de B2.
/// Un trazo largo de verdad: es donde el coste por fotograma se nota.
const PUNTOS_DEL_TRAZO: usize = 1_200;
/// Medio lado de la zona sucia de un fotograma de trazo, en pixeles de
/// pantalla: el tramo nuevo mas la holgura de la punta predicha.
const ZONA_TRAZO: i32 = 200;

/// Lo que se prepara una sola vez para medir (y para retratar) un trazo en
/// curso: la camara, el trazo, su punta y la zona sucia por los dos caminos.
struct Trazando {
    camara: Camara,
    efectiva: Camara,
    vista: (f32, f32, f32, f32),
    vivo: Elemento,
    punta: Option<Punto2>,
    /// La zona sucia en pixeles de ventana (capa de tinta) y en pixeles de
    /// la superficie de escena, que va corrida el colchon.
    zona_ventana: (i32, i32, i32, i32),
    zona_escena: (i32, i32, i32, i32),
    estampa: Estampa,
    candidatos: Vec<u64>,
}

fn trazando(escena: &Escena, rejilla: &Rejilla) -> Trazando {
    let camara = camara_de(escena, 4.0);
    let efectiva = crate::navegacion::vista_efectiva(&camara, ESCALA);
    let (w, h) = (ANCHO as f32, ALTO as f32);
    let vista = efectiva.ventana(w, h);
    // El trazo que se esta dibujando: en medio de lo que se ve, para que la
    // zona sucia caiga sobre escena de verdad y no sobre papel en blanco.
    let centro = (
        efectiva.x + w / (2.0 * efectiva.zoom),
        efectiva.y + h / (2.0 * efectiva.zoom),
    );
    let ancho_mundo = 220.0;
    let vivo = trazo(
        u64::from(u32::MAX),
        centro.0 - ancho_mundo / 2.0,
        centro.1 - 80.0,
        PUNTOS_DEL_TRAZO,
    );
    // La punta predicha, como la pone el editor: el ultimo punto adelantado.
    let punta = match &vivo.figura {
        Figura::Lapiz { puntos, .. } => puntos.last().copied(),
        _ => None,
    }
    .map(|p| Punto2::nuevo(p.x + 12.0, p.y + 6.0));
    let q = efectiva.a_pantalla(punta.unwrap_or(Punto2::nuevo(centro.0, centro.1)));
    let zona_ventana = (
        q.x as i32 - ZONA_TRAZO,
        q.y as i32 - ZONA_TRAZO,
        q.x as i32 + ZONA_TRAZO,
        q.y as i32 + ZONA_TRAZO,
    );
    let m = MARGEN_ESCENA as i32;
    Trazando {
        camara,
        efectiva,
        vista,
        punta,
        zona_ventana,
        zona_escena: (
            zona_ventana.0 + m,
            zona_ventana.1 + m,
            zona_ventana.2 + m,
            zona_ventana.3 + m,
        ),
        estampa: Estampa {
            camara: (efectiva.x, efectiva.y, efectiva.zoom),
            tamano: tamano_escena(w, h, MARGEN_ESCENA as f32),
            excluidos: vec![vivo.id],
        },
        candidatos: rejilla.candidatos(vista),
        vivo,
    }
}

fn rect_de(z: (i32, i32, i32, i32)) -> RectF {
    RectF {
        x: z.0 as f32,
        y: z.1 as f32,
        ancho: (z.2 - z.0) as f32,
        alto: (z.3 - z.1) as f32,
    }
}

/// **B2 medido.** Lo que cuesta APOYAR el lapiz y lo que cuesta cada uno de
/// los fotogramas siguientes, por los dos caminos, en la misma vuelta y con
/// la misma GPU.
///
/// - «antes»: al pulsar se hornea la capa congelada -un `ID2D1Bitmap1` del
///   tamano de la superficie de escena, 3512 x 2512 a 3000 x 2000 con
///   colchon, y toda la escena pintada dentro-, y cada fotograma copia de
///   ella la zona sucia (D148) antes de pintar el trazo encima.
/// - «B2»: al pulsar no se hornea nada -la escena ya esta en su visual y no
///   se toca- y cada fotograma limpia la zona sucia de la capa de tinta y
///   pinta el trazo ahi.
///
/// **Lo que este banco NO ve, dicho claro:** no ve lo que se ahorra al NO
/// presentar la cadena de intercambio de la escena en cada fotograma del
/// trazo, ni la recomposicion que DWM se ahorra. Eso solo se mide con la
/// ventana abierta. O sea, la diferencia de los fotogramas «siguientes» que
/// sale aqui es el SUELO de la real.
fn medir_trazo(b: &mut Banco, escena: &Escena, que: &str) {
    let mut rejilla = Rejilla::nueva();
    rejilla.sincronizar(escena);
    let t = trazando(escena, &rejilla);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    cache_tinta.fijar_escala(t.efectiva.zoom);
    let ms = |i: Instant| i.elapsed().as_secs_f64() * 1000.0;
    let vueltas_primer = 5;

    // --- Antes: apoyar el lapiz hornea la escena entera ---
    let mut capa = CapaEstatica::nueva();
    let mut hornear = 0.0;
    for _ in 0..vueltas_primer {
        capa.soltar();
        let i = Instant::now();
        hornear_capa(b, escena, &t, &mut cache, &mut cache_tinta, &mut capa);
        b.destino.esperar_gpu().expect("GPU");
        hornear += ms(i);
    }
    println!(
        "{:<44} primer fotograma {:>7.2} ms | {:>5.1} MB de video ({} x {})",
        format!("[antes] {que} apoyar el lapiz"),
        hornear / vueltas_primer as f64,
        capa.bytes() as f64 / (1024.0 * 1024.0),
        t.estampa.tamano.0,
        t.estampa.tamano.1
    );

    // --- Antes: cada fotograma copia su zona de la capa y pinta encima ---
    let mut seguir = 0.0;
    for _ in 0..VUELTAS {
        let i = Instant::now();
        capa.volcar_zona(
            &mut b.motor,
            &b.destino.destino,
            &t.estampa,
            Some(t.zona_escena),
        );
        fotograma_de_trazo(b, &t, true, false);
        b.destino.esperar_gpu().expect("GPU");
        seguir += ms(i);
    }
    println!(
        "{:<44} cada fotograma  {:>7.2} ms",
        format!("[antes] {que} trazando"),
        seguir / VUELTAS as f64
    );
    capa.soltar();

    // --- B2: apoyar el lapiz solo vacia la capa de tinta ---
    let mut encender = 0.0;
    for _ in 0..vueltas_primer {
        let i = Instant::now();
        b.motor
            .dibujar(&b.tinta.destino, |p| p.limpiar_transparente())
            .expect("vaciar la capa de tinta");
        b.tinta.esperar_gpu().expect("GPU");
        encender += ms(i);
    }
    println!(
        "{:<44} primer fotograma {:>7.2} ms | {:>5.1} MB de video ({ANCHO} x {ALTO})",
        format!("[B2] {que} apoyar el lapiz"),
        encender / vueltas_primer as f64,
        (ANCHO as f64 * ALTO as f64 * 4.0) / (1024.0 * 1024.0)
    );

    // --- B2: cada fotograma limpia su zona de la capa y pinta el trazo ---
    let mut seguir_b2 = 0.0;
    for _ in 0..VUELTAS {
        let i = Instant::now();
        fotograma_de_trazo(b, &t, false, true);
        b.tinta.esperar_gpu().expect("GPU");
        seguir_b2 += ms(i);
    }
    println!(
        "{:<44} cada fotograma  {:>7.2} ms",
        format!("[B2] {que} trazando"),
        seguir_b2 / VUELTAS as f64
    );
}

/// Hornea la capa congelada como lo hace el editor al apoyar el lapiz.
fn hornear_capa(
    b: &mut Banco,
    escena: &Escena,
    t: &Trazando,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    capa: &mut CapaEstatica,
) {
    let m = MARGEN_ESCENA as f32;
    let imagenes = &b.imagenes;
    capa.preparar(&mut b.motor, t.estampa.clone(), |p| {
        p.limpiar(Color::BLANCO);
        let origen = t.efectiva.a_pantalla(Punto2::nuevo(0.0, 0.0));
        p.poner_vista((0.0, 0.0), t.efectiva.zoom, (origen.x + m, origen.y + m));
        for id in &t.candidatos {
            let Some(e) = escena.buscar(*id) else {
                continue;
            };
            if e.borrado {
                continue;
            }
            let mut indice = 0u32;
            por_cada_orden(cache, e, t.efectiva.zoom, escena.escala.as_ref(), |orden| {
                dibujar_orden(
                    p,
                    orden,
                    t.vista,
                    Some((&mut *cache_tinta, (e.id, e.version, indice))),
                    imagenes,
                    t.efectiva.zoom,
                );
                indice += 1;
            });
        }
    })
    .expect("hornear la capa");
}

/// Un fotograma del trazo: el contorno entero y su punta, recortados a la
/// zona sucia. `en_escena` lo pinta sobre la superficie de escena (camino de
/// antes, con el colchon); si no, sobre la capa de tinta, que ademas se
/// limpia primero porque ahi no hay escena debajo que tape lo anterior.
fn fotograma_de_trazo(b: &Banco, t: &Trazando, en_escena: bool, limpiar: bool) {
    let m = if en_escena { MARGEN_ESCENA as f32 } else { 0.0 };
    let zona = if en_escena {
        t.zona_escena
    } else {
        t.zona_ventana
    };
    let destino = if en_escena {
        &b.destino.destino
    } else {
        &b.tinta.destino
    };
    b.motor
        .dibujar(destino, |p| {
            p.empujar_recorte(rect_de(zona));
            if limpiar {
                p.limpiar_transparente();
            }
            let origen = t.efectiva.a_pantalla(Punto2::nuevo(0.0, 0.0));
            p.poner_vista((0.0, 0.0), t.efectiva.zoom, (origen.x + m, origen.y + m));
            for orden in pixpin_motor2d::pintado::ordenes_a_distancia(&t.vivo, t.efectiva.zoom) {
                dibujar_orden(p, &orden, t.vista, None, &b.imagenes, t.efectiva.zoom);
            }
            if let Some(o) = t.punta.and_then(|q| punta_de_tinta(&t.vivo, q)) {
                dibujar_orden(p, &o, t.vista, None, &b.imagenes, t.efectiva.zoom);
            }
            p.soltar_recorte();
        })
        .expect("fotograma de trazo");
}

#[test]
#[ignore = "necesita GPU real; ejecutar en --release con --ignored --nocapture"]
fn medir_apoyar_el_lapiz_y_trazar_a_3000_por_2000() {
    let mut b = Banco::nuevo();
    for cuantos in [200usize, 2_000, 10_000] {
        let escena = escena_sintetica(cuantos);
        medir_trazo(&mut b, &escena, &format!("mezcla {cuantos}"));
    }
}

/// Guarda en PNG lo que ve el usuario con B2: la escena en su visual, la
/// tinta en el suyo, y las dos compuestas como las compone DWM.
///
/// Es la unica forma, sin abrir la aplicacion, de comprobar que el trazo cae
/// donde tiene que caer: la capa de tinta pinta en pixeles de VENTANA y la
/// escena en pixeles de superficie (corridos el colchon), y equivocarse en
/// ese colchon se ve como un trazo 256 px desplazado. La carpeta la da
/// `PIXPIN_TRAZO_RETRATO`; sin ella no hace nada.
#[test]
#[ignore = "necesita GPU real; ejecutar con --ignored"]
fn retratar_el_trazo_en_su_capa() {
    let Some(dir) = std::env::var_os("PIXPIN_TRAZO_RETRATO").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).expect("carpeta");
    let b = Banco::nuevo();
    let escena = escena_sintetica(200);
    let mut rejilla = Rejilla::nueva();
    rejilla.sincronizar(&escena);
    let t = trazando(&escena, &rejilla);
    let mut cache = Cache::nueva();
    let mut cache_tinta = pixpin_render::CacheTinta::nueva();
    cache_tinta.fijar_escala(t.efectiva.zoom);

    // La escena tal como se ve POR LA VENTANA, que es lo que ensena su
    // visual: el colchon se lo come la transformada del visual, asi que en
    // pantalla la escena y la tinta comparten el mismo origen. Si no fuera
    // asi -si `pintar_tinta_viva` le sumara el colchon-, el trazo saldria
    // 256 px corrido y este retrato lo ensenaria.
    b.fotograma(
        &escena,
        &rejilla,
        &mut cache,
        &mut cache_tinta,
        &t.camara,
        true,
    );
    b.destino.esperar_gpu().expect("GPU");
    let (w, h, escena_px) = b.destino.leer_rgba().expect("leer escena");

    // Y la capa de tinta: vacia, y con el fotograma del trazo dentro.
    b.motor
        .dibujar(&b.tinta.destino, |p| p.limpiar_transparente())
        .expect("vaciar");
    fotograma_de_trazo(&b, &t, false, true);
    b.tinta.esperar_gpu().expect("GPU");
    let (_, _, tinta_px) = b.tinta.leer_rgba().expect("leer tinta");

    // Compuestas: la tinta encima, con su alfa. Es lo que hace DWM con los
    // dos visuales, hecho aqui en CPU para poder mirarlo.
    let mut juntas = escena_px.clone();
    for (d, s) in juntas.chunks_exact_mut(4).zip(tinta_px.chunks_exact(4)) {
        let a = s[3] as f32 / 255.0;
        for k in 0..3 {
            d[k] = (s[k] as f32 * a + d[k] as f32 * (1.0 - a)).round() as u8;
        }
    }
    for (nombre, px) in [
        ("escena", escena_px),
        ("tinta", tinta_px),
        ("compuesto", juntas),
    ] {
        let png = pixpin_codec::codificar_png(&pixpin_codec::ImagenRgba {
            ancho: w,
            alto: h,
            pixeles: px,
        })
        .expect("png");
        std::fs::write(dir.join(format!("{nombre}.png")), png).expect("escribir");
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
