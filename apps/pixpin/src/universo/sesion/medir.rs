//! D239 medido sin ventana: el fotograma entero del universo pintado sobre
//! una textura de 3000 x 2000 (la pantalla del equipo del usuario al 150 %),
//! con la GPU de verdad.
//!
//! Separa lo que cuesta la CPU (`preparar`: que se ve, cuadernos,
//! miniaturas; `encargar`: recorrer los astros y pedirle a Direct2D cada
//! primitiva, texto incluido) del total con la GPU terminada. Va con
//! `#[ignore]` porque necesita GPU: se ejecuta a mano, en `--release`,
//!
//! ```text
//! cargo test --release -p pixpin --bin pixpinmax medir_fotograma -- --ignored --nocapture --test-threads=1
//! ```
//!
//! `PIXPIN_UNIVERSO_REAL=<carpeta>` mide tambien el universo del usuario: la
//! carpeta es una COPIA de `%APPDATA%\PixPinMax` (se leen sus cuadernos).

use std::time::Instant;

use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_universo::Astro;
use pixpin_universo::ficha::ClaseLuna;

use super::*;

const ANCHO: u32 = 3000;
const ALTO: u32 = 2000;
const ESCALA: u32 = 150;
/// Fotogramas por camara quieta, y los del paneo.
const VUELTAS: usize = 60;

/// Milisegundos medios por fotograma, y el peor total.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Medida {
    pub preparar: f64,
    pub encargar: f64,
    pub total: f64,
    pub peor: f64,
}

pub(super) struct Banco {
    motor: MotorRender,
    destino: FueraDePantalla,
}

impl Banco {
    pub fn nuevo() -> Banco {
        let dispositivo = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(dispositivo.d3d()).expect("motor");
        let destino =
            FueraDePantalla::nuevo(&motor, dispositivo.d3d(), ANCHO, ALTO).expect("destino");
        Banco { motor, destino }
    }

    /// Un fotograma como lo pinta el editor con el universo detras: el cielo
    /// y los astros, y encima los paneles.
    pub fn fotograma(&self, s: &mut Sesion, camara: &Camara) -> (f64, f64, f64) {
        let efectiva = crate::navegacion::vista_efectiva(camara, ESCALA);
        let (w, h) = (ANCHO as f32, ALTO as f32);
        let t0 = Instant::now();
        s.preparar(&self.motor, &efectiva);
        let t1 = Instant::now();
        self.motor
            .dibujar(&self.destino.destino, |p| {
                p.desplazar(0.0, 0.0);
                s.pintar_detras(p, &efectiva);
                s.pintar_delante(p, &efectiva, w, h);
            })
            .expect("fotograma");
        let t2 = Instant::now();
        self.destino.esperar_gpu().expect("GPU");
        let t3 = Instant::now();
        let ms = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1000.0;
        (ms(t0, t1), ms(t1, t2), ms(t0, t3))
    }

    /// `VUELTAS` fotogramas con las camaras que diga `camara(i)`.
    pub fn medir(&self, s: &mut Sesion, camara: impl Fn(usize) -> Camara) -> Medida {
        // Calentar: estrellas, pinceles, cuadernos.
        for _ in 0..3 {
            self.fotograma(s, &camara(0));
        }
        // Con la camara quieta, lo que se mide es el detalle entero: el
        // cambio de camara de antes no cuenta como moverse.
        let quieta = camara(0) == camara(1);
        let mut m = Medida::default();
        for i in 0..VUELTAS {
            if quieta {
                s.movida = None;
            }
            let (a, b, t) = self.fotograma(s, &camara(i));
            m.preparar += a;
            m.encargar += b;
            m.total += t;
            m.peor = m.peor.max(t);
        }
        let n = VUELTAS as f64;
        m.preparar /= n;
        m.encargar /= n;
        m.total /= n;
        m
    }
}

pub(super) fn sesion_de(
    raiz: PathBuf,
    u: Universo,
    nombres: HashMap<String, String>,
    nivel: pixpin_nivel::Nivel,
) -> Sesion {
    let mut s = Sesion::nueva(
        raiz,
        u,
        nombres,
        nivel,
        Catalogo::nuevo(pixpin_store::Idioma::Espanol),
        None,
    );
    s.tamano = (ANCHO as f32, ALTO as f32);
    s.escala_por_cien = ESCALA;
    s
}

/// 20 galaxias y 5.000 lunas, 250 por galaxia en rejilla, con su ficha
/// (nombre, tamano y clase variada): el caso grande de D239.
pub(super) fn sintetico(nivel: pixpin_nivel::Nivel) -> Sesion {
    let mut u = Universo::nuevo();
    let proyectos: Vec<String> = (0..20).map(|i| format!("p{i}")).collect();
    pixpin_universo::galaxias::sincronizar(&mut u, &proyectos);
    let galaxias: Vec<(IdAstro, f32, f32, String)> = u
        .astros
        .iter()
        .map(|a| (a.id, a.x, a.y, a.proyecto().unwrap_or_default().to_string()))
        .collect();
    let clases = [
        ClaseLuna::Archivo,
        ClaseLuna::Imagen,
        ClaseLuna::Nota,
        ClaseLuna::Dibujo,
        ClaseLuna::Voz,
        ClaseLuna::Pagina,
    ];
    let mut fichas = HashMap::new();
    for i in 0..5000 {
        let (g, gx, gy, p) = &galaxias[i % 20];
        let id = u.nuevo_id();
        let k = (i / 20) as f32;
        let codigo = format!("m:{i}");
        let mut l = Astro::luna(
            id,
            &codigo,
            p,
            gx + (k % 15.0) * 100.0 - 700.0,
            gy + (k / 15.0).floor() * 100.0 - 700.0,
        );
        l.padre = Some(*g);
        u.astros.push(l);
        fichas.insert(
            codigo.clone(),
            FichaLuna {
                codigo,
                proyecto: p.clone(),
                clase: clases[i % clases.len()],
                nombre: format!("Documento numero {i} del proyecto.pdf"),
                bytes: 12_345 + i as i64,
                codigo_chat: Some(format!("{i}·K7Q2")),
                extracto: "Una nota con algo de texto dentro".into(),
                en_equipo: i % 7 != 0,
                ..Default::default()
            },
        );
    }
    u.marcar_cambio();
    let nombres = proyectos
        .iter()
        .enumerate()
        .map(|(i, p)| (p.clone(), format!("Proyecto numero {i}")))
        .collect();
    let raiz = std::env::temp_dir().join("pixpin-medir-universo-vacio");
    let mut s = sesion_de(raiz, u, nombres, nivel);
    s.fichas = fichas;
    s
}

/// La copia del universo del usuario, si se dio su carpeta.
fn real(nivel: pixpin_nivel::Nivel) -> Option<Sesion> {
    let raiz = PathBuf::from(std::env::var_os("PIXPIN_UNIVERSO_REAL")?);
    let indice = pixpin_proyecto::almacen::Indice::leer(&raiz);
    let fichas = indice.ordenadas();
    let ids: Vec<String> = fichas.iter().map(|f| f.id.clone()).collect();
    let nombres = fichas
        .iter()
        .map(|f| (f.id.clone(), f.nombre.clone()))
        .collect();
    let cargado =
        pixpin_universo::formato::cargar(&pixpin_universo::formato::ruta(&raiz), 0).ok()?;
    let mut u = cargado.universo;
    pixpin_universo::galaxias::sincronizar(&mut u, &ids);
    u.anotaciones = Escena::nueva();
    Some(sesion_de(raiz, u, nombres, nivel))
}

/// Espera a que lleguen los cuadernos pedidos al pintar `camara`.
fn esperar_cuadernos(b: &Banco, s: &mut Sesion, camara: &Camara) {
    let mut c = *camara;
    for _ in 0..60 {
        b.fotograma(s, &c);
        s.al_despertar(&mut c);
        if !s.cargador.leyendo() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// La galaxia con mas lunas.
fn galaxia_mayor(s: &Sesion) -> IdAstro {
    let mut cuenta: HashMap<IdAstro, usize> = HashMap::new();
    for a in &s.u.astros {
        if let Some(p) = a.padre {
            *cuenta.entry(p).or_default() += 1;
        }
    }
    s.u.astros
        .iter()
        .filter(|a| matches!(a.clase, Clase::Galaxia { .. }))
        .max_by_key(|a| cuenta.get(&a.id).copied().unwrap_or(0))
        .map(|a| a.id)
        .expect("hay galaxias")
}

fn camara_cosmos(s: &mut Sesion) -> Camara {
    let mut c = Camara::nueva();
    s.encajar_cosmos(&mut c);
    s.vuelo.take().map(|v| v.hasta).unwrap_or(c)
}

fn camara_galaxia(s: &mut Sesion) -> Camara {
    let g = galaxia_mayor(s);
    let caja = agrandar(s.u.astro(g).expect("galaxia").caja(), 1.1);
    s.encajar(caja)
}

fn escribir(nombre: &str, m: Medida) {
    println!(
        "{nombre:<34} preparar {:>6.2} ms | encargar {:>6.2} ms | CPU {:>6.2} ms | total {:>6.2} ms (peor {:>6.2})",
        m.preparar,
        m.encargar,
        m.preparar + m.encargar,
        m.total,
        m.peor
    );
}

/// Las cuatro camaras: cosmos quieto, galaxia quieta, y dos paneos de 60
/// fotogramas seguidos (a zoom intermedio, con las galaxias como discos, y
/// dentro de una galaxia).
pub(super) fn medir_todo(b: &Banco, s: &mut Sesion, que: &str) -> [Medida; 4] {
    let cosmos = camara_cosmos(s);
    let galaxia = camara_galaxia(s);
    esperar_cuadernos(b, s, &galaxia);
    let m_cosmos = b.medir(s, |_| cosmos);
    escribir(&format!("{que} cosmos"), m_cosmos);
    let m_galaxia = b.medir(s, |_| galaxia);
    escribir(&format!("{que} galaxia"), m_galaxia);
    // 30 px logicos por fotograma: un arrastre rapido (1.800 px/s a 60 Hz).
    let intermedio = acercar_al_centro(&cosmos, 3.0);
    let m_paneo = b.medir(s, |i| Camara {
        x: intermedio.x + i as f32 * 30.0 / intermedio.zoom,
        y: intermedio.y + i as f32 * 12.0 / intermedio.zoom,
        ..intermedio
    });
    escribir(&format!("{que} paneo entre galaxias"), m_paneo);
    let m_paneo_g = b.medir(s, |i| Camara {
        x: galaxia.x + i as f32 * 30.0 / galaxia.zoom,
        ..galaxia
    });
    escribir(&format!("{que} paneo dentro de galaxia"), m_paneo_g);
    [m_cosmos, m_galaxia, m_paneo, m_paneo_g]
}

#[test]
#[ignore = "necesita GPU real; ejecutar en --release con --ignored --nocapture"]
fn medir_fotograma_del_universo_a_3000_por_2000() {
    let b = Banco::nuevo();
    for nivel in [pixpin_nivel::Nivel::Ligero, pixpin_nivel::Nivel::Completo] {
        let n = format!("{nivel:?}");
        let mut s = sintetico(nivel);
        medir_todo(&b, &mut s, &format!("[{n}] sintetico"));
        if let Some(mut s) = real(nivel) {
            medir_todo(&b, &mut s, &format!("[{n}] real"));
        }
    }
}

/// Cuanto se lleva cada capa: cada fila pinta lo de la anterior y algo mas.
fn desglose(b: &Banco, s: &mut Sesion, camara: &Camara, que: &str) {
    type Capa = fn(&Pintor, &Sesion, &Camara, f32, f32);
    let capas: [(&str, Capa); 5] = [
        ("limpiar", |p, _, _, _, _| p.limpiar(PALETA.espacio)),
        ("+ estrellas", |p, s, c, _w, _h| {
            pintar::fondo(p, Some(&s.cielo), s.estrellas.as_ref(), c, false, _w, _h)
        }),
        ("+ astros", |p, s, c, _w, _h| {
            pintar::fondo(p, Some(&s.cielo), s.estrellas.as_ref(), c, false, _w, _h);
            s.con_contexto(|ctx| pintar::astros(p, ctx, c));
        }),
        ("+ lineas y nebulosas", |p, s, c, _w, _h| {
            s.pintar_detras(p, c)
        }),
        ("+ paneles", |p, s, c, w, h| {
            s.pintar_detras(p, c);
            s.pintar_delante(p, c, w, h);
        }),
    ];
    let efectiva = crate::navegacion::vista_efectiva(camara, ESCALA);
    let (w, h) = (ANCHO as f32, ALTO as f32);
    for (nombre, capa) in capas {
        let (mut encargar, mut total) = (0.0, 0.0);
        for i in 0..(VUELTAS + 3) {
            s.preparar(&b.motor, &efectiva);
            s.movida = None;
            let t1 = Instant::now();
            b.motor
                .dibujar(&b.destino.destino, |p| {
                    p.desplazar(0.0, 0.0);
                    capa(p, s, &efectiva, w, h);
                })
                .expect("fotograma");
            let t2 = Instant::now();
            b.destino.esperar_gpu().expect("GPU");
            let t3 = Instant::now();
            if i >= 3 {
                encargar += (t2 - t1).as_secs_f64() * 1000.0;
                total += (t3 - t1).as_secs_f64() * 1000.0;
            }
        }
        let n = VUELTAS as f64;
        println!(
            "{que:<28} {nombre:<22} encargar {:>6.2} ms | total {:>6.2} ms",
            encargar / n,
            total / n
        );
    }
}

#[test]
#[ignore = "necesita GPU real; ejecutar en --release con --ignored --nocapture"]
fn medir_fotograma_por_capas() {
    let b = Banco::nuevo();
    for nivel in [pixpin_nivel::Nivel::Ligero, pixpin_nivel::Nivel::Completo] {
        let mut s = sintetico(nivel);
        let cosmos = camara_cosmos(&mut s);
        let galaxia = camara_galaxia(&mut s);
        desglose(&b, &mut s, &cosmos, &format!("[{nivel:?}] cosmos"));
        desglose(&b, &mut s, &galaxia, &format!("[{nivel:?}] galaxia"));
    }
}

/// Guarda en PNG lo que se mide, para mirarlo a ojo: que el brillo en
/// bitmap y las estrellas en sprites se ven como antes. La carpeta la da
/// `PIXPIN_UNIVERSO_RETRATO`; sin ella no hace nada.
#[test]
#[ignore = "necesita GPU real; ejecutar con --ignored"]
fn retratar_el_universo() {
    let Some(dir) = std::env::var_os("PIXPIN_UNIVERSO_RETRATO").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).expect("carpeta");
    let b = Banco::nuevo();
    let mut s = sintetico(pixpin_nivel::Nivel::Completo);
    let cosmos = camara_cosmos(&mut s);
    let galaxia = camara_galaxia(&mut s);
    let intermedio = acercar_al_centro(&cosmos, 3.0);
    // Dentro de una galaxia, tan cerca que las lunas son fichas con su
    // nombre: es donde se ve el vidrio de las tarjetas y las chapas.
    let lunas = acercar_al_centro(&galaxia, 4.0);
    for (nombre, c) in [
        ("cosmos", cosmos),
        ("intermedio", intermedio),
        ("galaxia", galaxia),
        ("lunas", lunas),
    ] {
        b.fotograma(&mut s, &c);
        s.movida = None;
        b.fotograma(&mut s, &c);
        let (ancho, alto, pixeles) = b.destino.leer_rgba().expect("leer");
        let png = pixpin_codec::codificar_png(&pixpin_codec::ImagenRgba {
            ancho,
            alto,
            pixeles,
        })
        .expect("png");
        std::fs::write(dir.join(format!("{nombre}.png")), png).expect("escribir");
    }
}

/// `c` acercada `factor` veces sin mover el centro de la pantalla.
fn acercar_al_centro(c: &Camara, factor: f32) -> Camara {
    let e = crate::navegacion::escala_de(ESCALA);
    let (w, h) = (ANCHO as f32 / e, ALTO as f32 / e);
    let centro = (c.x + w / (2.0 * c.zoom), c.y + h / (2.0 * c.zoom));
    let zoom = c.zoom * factor;
    Camara {
        x: centro.0 - w / (2.0 * zoom),
        y: centro.1 - h / (2.0 * zoom),
        zoom,
    }
}
