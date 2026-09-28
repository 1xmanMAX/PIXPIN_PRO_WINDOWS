//! **El tema Cosmos fuera del universo** (M1; `ui/theme/Cosmos.kt` y
//! `Theme.kt` del movil).
//!
//! En el movil es una opcion del «Modo noche» (Sistema, Claro, Oscuro,
//! Automatico, **Cosmos**) y pone el cielo del sistema solar —degradado azul
//! noche, dos nebulosas y estrellas quietas— detras de las pantallas opacas,
//! con los colores oscuros de la galaxia. El chat conserva su papel: el
//! cielo se ve alrededor, en la lista de proyectos.
//!
//! Aqui es una casilla de Ajustes (`tema_cosmos`) y hace lo mismo en la
//! ventana del chat: fuerza los colores de noche (que en el PC ya son los
//! del Cosmos del movil, medidos en sus capturas) y pinta el cielo detras de
//! la columna de proyectos.
//!
//! **Barato en la maquina suelo** (HD 4000): el degradado y las nebulosas se
//! hornean una vez por tamano de ventana en un bitmap chico y opaco
//! (`universo::cielo`, medido alli: una pasada sin mezcla), y las estrellas
//! son 217 puntos quietos, sin animacion ni paralaje.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use pixpin_render::{Color, MotorRender, Pintor, RectF};

use crate::universo::cielo::Cielo;

/// Lo pone quien lee los ajustes (al arrancar y al cambiarlos). Un estatico
/// y no un parametro: lo mira el hilo del chat, que nace aparte, y asi no
/// hay que pasarlo por cada sitio que abre el chat.
static ACTIVO: AtomicBool = AtomicBool::new(false);

pub fn fijar(activo: bool) {
    ACTIVO.store(activo, Ordering::Relaxed);
}

pub fn activo() -> bool {
    ACTIVO.load(Ordering::Relaxed)
}

/// Cuantas estrellas de cada tamano (`Cosmos.kt`: 160, 45 y 12), de
/// muchas y tenues a pocas y brillantes.
pub const CUANTAS: [usize; 3] = [160, 45, 12];

/// Sus colores, con su alfa (`BRILLOS` del movil).
const BRILLOS: [(u32, f32); 3] = [
    (0xffffff, 0x55 as f32 / 255.0),
    (0xdce6ff, 0x88 as f32 / 255.0),
    (0xfff4d6, 0xcc as f32 / 255.0),
];

/// Las posiciones de 0 a 1 de las estrellas, siempre las mismas: el cielo
/// no cambia de una vez a otra. El movil usa `Random(11)` de Kotlin; aqui un
/// generador congruente con la misma semilla. No salen en el mismo sitio que
/// en el telefono, pero si con el mismo reparto.
pub fn estrellas() -> [Vec<(f32, f32)>; 3] {
    let mut s: u64 = 11;
    let mut azar = || {
        s = s
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((s >> 40) as f32) / (1u64 << 24) as f32
    };
    CUANTAS.map(|n| (0..n).map(|_| (azar(), azar())).collect())
}

/// El cielo de esta ventana, horneado. Uno por hilo: cada ventana con tema
/// vive en su hilo y sus recursos son de su dispositivo.
struct Fondo {
    cielo: Cielo,
    estrellas: [Vec<(f32, f32)>; 3],
}

thread_local! {
    static FONDO: RefCell<Option<Fondo>> = const { RefCell::new(None) };
}

/// Lo hornea si hace falta, FUERA del fotograma (crear un bitmap a medio
/// fotograma no se puede). Sin el tema no hace nada ni guarda nada.
pub fn preparar(motor: &MotorRender, ancho: f32, alto: f32) {
    if !activo() {
        return;
    }
    FONDO.with(|f| {
        let mut f = f.borrow_mut();
        let fondo = f.get_or_insert_with(|| Fondo {
            cielo: Cielo::default(),
            estrellas: estrellas(),
        });
        if let Err(e) = fondo.cielo.preparar(motor, ancho, alto) {
            // Sin el bitmap se pinta el color de siempre: se pierde el
            // adorno, no la ventana.
            tracing::warn!(?e, "sin cielo para el tema Cosmos");
        }
    });
}

/// Se perdio el dispositivo: el bitmap era del viejo.
pub fn soltar() {
    FONDO.with(|f| *f.borrow_mut() = None);
}

/// Pinta el trozo `zona` del cielo de una ventana de `ancho` x `alto`, con
/// sus estrellas: el degradado sigue siendo el de la ventana entera, solo se
/// rellena lo que se ve. Devuelve si lo hizo: si no (sin tema, o sin
/// bitmap), queda lo que hubiera debajo.
pub fn pintar(p: &Pintor, ancho: f32, alto: f32, escala: f32, zona: RectF) -> bool {
    if !activo() || zona.ancho <= 0.0 || zona.alto <= 0.0 {
        return false;
    }
    FONDO.with(|f| {
        let f = f.borrow();
        let Some(fondo) = f.as_ref() else {
            return false;
        };
        if !fondo.cielo.pintar_zona(p, ancho, alto, zona) {
            return false;
        }
        let dentro = |x: f32, y: f32| {
            x >= zona.x && x < zona.x + zona.ancho && y >= zona.y && y < zona.y + zona.alto
        };
        for (k, capa) in fondo.estrellas.iter().enumerate() {
            let (rgb, a) = BRILLOS[k];
            let color = Color {
                r: ((rgb >> 16) & 0xff) as f32 / 255.0,
                g: ((rgb >> 8) & 0xff) as f32 / 255.0,
                b: (rgb & 0xff) as f32 / 255.0,
                a,
            };
            // `strokeWidth = (k + 1) * 1,1 dp` con punta redonda: un disco
            // de ese diametro.
            let r = (k as f32 + 1.0) * 0.55 * escala;
            for (x, y) in capa {
                let (x, y) = (x * ancho, y * alto);
                if dentro(x, y) {
                    p.circulo((x, y), r, color);
                }
            }
        }
        true
    })
}

/// Lo que mide la ventana del chat entera, de su disposicion: la barra de
/// arriba y, debajo, la mas alta de las dos columnas.
pub fn tamano_de(d: &pixpin_ui::chat::Disposicion) -> (f32, f32) {
    (
        d.barra.ancho.max(d.lista.ancho + d.chat.ancho) as f32,
        (d.barra.alto + d.lista.alto.max(d.chat.alto)) as f32,
    )
}

/// Un rectangulo de la ventana relleno con `color`, o nada si esta el
/// cielo: con el tema, la lista de proyectos va sobre el cielo, como en el
/// movil (`background = Transparent`).
pub fn rellenar_o_cielo(p: &Pintor, r: RectF, color: Color) {
    if !activo() {
        p.rellenar(r, color);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_estrellas_son_las_del_movil_en_numero_y_siempre_las_mismas() {
        let a = estrellas();
        assert_eq!(a.each_ref().map(Vec::len), CUANTAS);
        assert_eq!(a, estrellas(), "el cielo no cambia de una vez a otra");
        assert!(
            a.iter()
                .flatten()
                .all(|(x, y)| (0.0..1.0).contains(x) && (0.0..1.0).contains(y))
        );
    }

    #[test]
    fn las_estrellas_se_reparten_por_toda_la_ventana_y_no_en_un_rincon() {
        let todas: Vec<(f32, f32)> = estrellas().into_iter().flatten().collect();
        let cuadrantes = |fx: fn(f32) -> bool, fy: fn(f32) -> bool| {
            todas.iter().filter(|(x, y)| fx(*x) && fy(*y)).count()
        };
        let izq = |v: f32| v < 0.5;
        let der = |v: f32| v >= 0.5;
        for n in [
            cuadrantes(izq, izq),
            cuadrantes(izq, der),
            cuadrantes(der, izq),
            cuadrantes(der, der),
        ] {
            // 217 entre cuatro son ~54: ni vacio ni amontonado.
            assert!((25..=90).contains(&n), "cuadrante con {n}");
        }
    }

    /// La ventana del chat con el tema, de mentira: el cielo detras, la
    /// columna de proyectos encima sin fondo y el papel del chat a la
    /// derecha, como lo pinta `ventana_chat::pintar`. Y lo que cuesta.
    ///
    /// `cargo test -p pixpin --bin pixpinmax muestra_del_tema_cosmos -- --ignored --nocapture`
    #[test]
    #[ignore = "necesita GPU y sesion de escritorio"]
    fn muestra_del_tema_cosmos_en_el_chat() {
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let (w, h) = (1600u32, 1000u32);
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), w, h).expect("destino");
        fijar(true);
        preparar(&motor, w as f32, h as f32);
        let hex = |v: u32| Color {
            r: ((v >> 16) & 0xff) as f32 / 255.0,
            g: ((v >> 8) & 0xff) as f32 / 255.0,
            b: (v & 0xff) as f32 / 255.0,
            a: 1.0,
        };
        let lista = RectF {
            x: 0.0,
            y: 40.0,
            ancho: 460.0,
            alto: h as f32 - 40.0,
        };
        let pintar_todo = |p: &Pintor| {
            p.limpiar(hex(0x151e27));
            assert!(pintar(p, w as f32, h as f32, 1.25, lista));
            // La barra de titulo, opaca, y el papel del chat de noche.
            p.rellenar(RectF { x: 0.0, y: 0.0, ancho: w as f32, alto: 40.0 }, hex(0x242f3d));
            p.rellenar(RectF { x: 460.0, y: 40.0, ancho: 1140.0, alto: 960.0 }, hex(0x151e27));
            for i in 0..9 {
                let y = 70.0 + i as f32 * 76.0;
                p.circulo((50.0, y + 24.0), 24.0, hex(0x5288c1));
                p.texto("Proyecto de obra", 90.0, y + 6.0, 16.0, hex(0xf5f5f5));
                p.texto("Plano estructural.pdf", 90.0, y + 30.0, 13.0, hex(0x7f91a4));
            }
        };
        motor.dibujar(&fuera.destino, pintar_todo).expect("pintar");
        fuera.esperar_gpu().expect("gpu");
        // Lo que cuesta de mas un fotograma con el cielo, en caliente: el
        // mismo fotograma con y sin el, limpiando la ventana en los dos.
        let medir = |con: bool| {
            let t = std::time::Instant::now();
            for _ in 0..60 {
                motor
                    .dibujar(&fuera.destino, |p| {
                        p.limpiar(hex(0x151e27));
                        if con {
                            pintar(p, w as f32, h as f32, 1.25, lista);
                        }
                    })
                    .expect("pintar");
                fuera.esperar_gpu().expect("gpu");
            }
            t.elapsed().as_secs_f64() * 1000.0 / 60.0
        };
        let (sin, con) = (medir(false), medir(true));
        println!("fotograma {w}x{h}: {sin:.2} ms sin cielo, {con:.2} ms con el cielo en la lista");
        motor.dibujar(&fuera.destino, pintar_todo).expect("pintar");
        fuera.esperar_gpu().expect("gpu");
        let (ancho, alto, pixeles) = fuera.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
            ancho,
            alto,
            pixeles,
        })
        .expect("png");
        let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-universo");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("m1-tema-cosmos.png"), png).unwrap();
        // Sus recursos son de este dispositivo: fuera antes de que muera.
        soltar();
        fijar(false);
    }

    #[test]
    fn sin_el_tema_no_se_pinta_ni_se_prepara_nada() {
        fijar(false);
        assert!(!activo());
        fijar(true);
        assert!(activo());
        fijar(false);
    }
}
