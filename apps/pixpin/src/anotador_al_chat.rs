//! **Guardar lo anotado sobre la pantalla en «Mensajes guardados».**
//!
//! Lo pidio el usuario el 2026-09-28: «que se tome captura de pantalla al
//! fondo y las tintas de encima se mantengan editables y se plasmen dentro de
//! un canvas; esto se envia directamente al chat a Mensajes guardados,
//! aparezca como si hubiera subido una imagen y hubiera anotado sobre ella».
//!
//! Por eso no se inventa un formato: es **una foto del chat con su lienzo**,
//! lo mismo que deja el movil al anotar una foto (K11,
//! `pixpin_proyecto::lienzo_de_la_foto`):
//!
//! 1. la captura de la pantalla SIN la tinta ni la barra entra en «Mensajes
//!    guardados» como un mensaje IMAGEN corriente (`meter_en_proyecto`, que
//!    usa `cuaderno::anadir`, con su cerrojo);
//! 2. se le crea su lienzo `foto-<id>` con la captura como primer elemento
//!    (bloqueada, a la medida con que el movil la carga) y la tinta encima
//!    como elementos sueltos y editables, llevada de pixeles de la captura a
//!    unidades del lienzo (`lienzo_de_la_foto::con_dibujo`), y se apunta la
//!    `referencia` del mensaje.
//!
//! La burbuja ensena la foto con la tinta (`foto_anotada::dibujo_de_la_foto`)
//! y al pulsarla se abre ese lienzo y se sigue editando. Y viaja al movil
//! como cualquier foto anotada alli.
//!
//! **Varios monitores**: una sola captura del escritorio virtual entero, no
//! una por monitor. La ventana del anotador cubre el escritorio virtual y la
//! tinta vive en sus pixeles (camara quieta a 1:1, `pantalla::camara_quieta`),
//! asi que una flecha que cruza de un monitor a otro cae entera en UNA foto y
//! en las mismas coordenadas, sin partirla ni recolocarla. Lo que ningun
//! monitor cubre (un escritorio en L) sale negro, como en la captura
//! congelada de siempre (`pantalla::unir_fotos`).

use std::io;
use std::path::{Path, PathBuf};

use pixpin_motor2d::Elemento;
use pixpin_proyecto::almacen;
#[cfg(test)]
use pixpin_proyecto::cuaderno;

/// El nombre del fichero de la captura en el chat. Uno fijo: el almacen le
/// pone `(1)`, `(2)`... si ya hay otro (`almacen::guardar_adjunto`).
pub const NOMBRE_DE_LA_CAPTURA: &str = "pantalla-anotada.png";

/// Lo que se guarda: la captura de debajo y la tinta de encima, en pixeles
/// de la captura.
pub struct Sesion {
    pub foto: pixpin_codec::ImagenRgba,
    pub tinta: Vec<Elemento>,
}

/// Donde quedo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guardado {
    /// El id de «Mensajes guardados» en el indice.
    pub proyecto: String,
    /// El id del mensaje IMAGEN.
    pub mensaje: String,
    /// El id del lienzo de la foto (`foto-<id del mensaje>`).
    pub lienzo: String,
    /// Cuantos trazos quedaron encima.
    pub trazos: usize,
}

/// **Guarda la sesion** en «Mensajes guardados» (creandolo si no esta).
pub fn guardar(raiz: &Path, aparato: &str, sesion: &Sesion, ahora: i64) -> io::Result<Guardado> {
    let ficha = almacen::asegurar_guardados(raiz, ahora, aparato)?;
    let png = pixpin_codec::codificar_png(&sesion.foto).map_err(io::Error::other)?;
    let mensajes = crate::ventana_chat::meter_en_proyecto(
        raiz,
        &ficha.id,
        &[(NOMBRE_DE_LA_CAPTURA.to_string(), png)],
        aparato,
    )?;
    let m = mensajes
        .into_iter()
        .next()
        .ok_or_else(|| io::Error::other("la captura no entro en el chat"))?;
    let ruta = m
        .ruta
        .as_deref()
        .ok_or_else(|| io::Error::other("el mensaje de la captura no tiene fichero"))?;
    let foto = almacen::carpeta(raiz, &ficha.id).join(ruta);
    let hecho = pixpin_proyecto::lienzo_de_la_foto::con_dibujo(
        raiz,
        &ficha.id,
        &m,
        &foto,
        (sesion.foto.ancho, sesion.foto.alto),
        &sesion.tinta,
        ahora,
    )?;
    Ok(Guardado {
        proyecto: ficha.id,
        mensaje: m.id,
        lienzo: hecho.id,
        trazos: hecho.adoptados,
    })
}

/// Los textos del globo, ya traducidos (el hilo no tiene el catalogo).
pub struct Globo {
    pub titulo: String,
    pub hecho: String,
    pub fallo: String,
}

/// **Guarda sin parar el anotador**: comprimir la captura del escritorio
/// entero a PNG tarda (medido en `muestra_...`: cientos de milisegundos a
/// 3840x1080 en el portatil), y el anotador tiene que seguir dibujando al
/// ritmo del raton. Al acabar avisa con un globo en la bandeja (`hwnd` es la
/// ventana de mensajes de `main`) y despierta al chat para que lo ensene.
pub fn guardar_en_segundo_plano(raiz: PathBuf, sesion: Sesion, hwnd: isize, globo: Globo) {
    let hilo = std::thread::Builder::new()
        .name("pixpin-anotador-al-chat".into())
        .spawn(move || {
            let t0 = std::time::Instant::now();
            let aparato = pixpin_proyecto::identidad::Identidad::leer_o_crear(&raiz, "PC")
                .map(|i| i.yo.codigo())
                .unwrap_or_default();
            let ahora = pixpin_shell::entorno::ahora_utc_ms();
            let hecho = guardar(&raiz, &aparato, &sesion, ahora);
            let texto = match &hecho {
                Ok(g) => {
                    tracing::info!(
                        proyecto = %g.proyecto,
                        mensaje = %g.mensaje,
                        lienzo = %g.lienzo,
                        trazos = g.trazos,
                        ancho = sesion.foto.ancho,
                        alto = sesion.foto.alto,
                        ms = t0.elapsed().as_millis() as u64,
                        "pantalla anotada guardada en Mensajes guardados"
                    );
                    crate::ventana_chat::refrescar();
                    &globo.hecho
                }
                Err(e) => {
                    tracing::warn!(?e, "no se pudo guardar la pantalla anotada");
                    &globo.fallo
                }
            };
            let hwnd = windows::Win32::Foundation::HWND(hwnd as *mut _);
            let _ =
                pixpin_shell::aviso::Aviso::sobre_la_bandeja(hwnd).mostrar(&globo.titulo, texto);
        });
    if let Err(e) = hilo {
        tracing::warn!(?e, "no se pudo lanzar el guardado de la pantalla anotada");
    }
}

/// **Pura**: la tinta que se guarda. Lo que se ve (lo borrado o deshecho no)
/// y sin imagenes pegadas: el lienzo de la foto solo lleva la foto como
/// fichero, y una imagen suelta sin fichero que viaje se perderia al abrirlo.
pub fn tinta_de(escena: &pixpin_motor2d::Escena) -> Vec<Elemento> {
    escena
        .visibles()
        .filter(|e| !matches!(e.figura, pixpin_motor2d::Figura::Imagen { .. }))
        .cloned()
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::{Figura, Punto2};

    fn raiz(etiqueta: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!(
            "pixpin-anotador-chat-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    /// Una «pantalla» inventada: dos monitores de colores, lado a lado.
    fn pantalla(ancho: u32, alto: u32) -> pixpin_codec::ImagenRgba {
        let mut pixeles = Vec::with_capacity((ancho * alto * 4) as usize);
        for _ in 0..alto {
            for x in 0..ancho {
                let c = if x < ancho / 2 {
                    [40, 90, 200]
                } else {
                    [235, 235, 240]
                };
                pixeles.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        pixpin_codec::ImagenRgba {
            ancho,
            alto,
            pixeles,
        }
    }

    fn raya(puntos: &[(f32, f32)]) -> Elemento {
        Elemento {
            figura: Figura::Lapiz {
                puntos: puntos.iter().map(|&(x, y)| Punto2::nuevo(x, y)).collect(),
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            },
            trazo: pixpin_motor2d::ColorRgba::opaco(0.88, 0.19, 0.19),
            grosor: 2.0,
            ..Elemento::default()
        }
    }

    fn flecha(de: (f32, f32), a: (f32, f32)) -> Elemento {
        Elemento {
            figura: Figura::Flecha {
                // Los puntos van en coordenadas del mundo, como los deja el gesto.
                puntos: vec![Punto2::nuevo(de.0, de.1), Punto2::nuevo(a.0, a.1)],
                punta_inicio: pixpin_motor2d::formas::TipoPunta::Ninguna,
                punta_fin: pixpin_motor2d::formas::TipoPunta::Flecha,
                codos: false,
            },
            x: de.0,
            y: de.1,
            ancho: (a.0 - de.0).abs(),
            alto: (a.1 - de.1).abs(),
            trazo: pixpin_motor2d::ColorRgba::opaco(0.88, 0.19, 0.19),
            grosor: 4.0,
            ..Elemento::default()
        }
    }

    #[test]
    fn guardar_la_pantalla_anotada_deja_una_foto_en_mensajes_guardados_con_su_lienzo_editable() {
        let r = raiz("guardar");
        let sesion = Sesion {
            foto: pantalla(64, 18),
            tinta: vec![raya(&[(10.0, 4.0), (50.0, 14.0)])],
        };
        let g = guardar(&r, "PC01", &sesion, 1_790_000_000_000).unwrap();
        assert_eq!(g.lienzo, format!("foto-{}", g.mensaje));
        assert_eq!(g.trazos, 1);
        // En el indice, «Mensajes guardados».
        let indice = almacen::Indice::leer(&r);
        let ficha = indice
            .proyectos
            .iter()
            .find(|f| f.id == g.proyecto)
            .unwrap();
        assert!(ficha.es_guardados());
        // Un mensaje IMAGEN como una captura cualquiera, con su referencia.
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&r, &g.proyecto)).unwrap();
        assert_eq!(c.mensajes.len(), 1);
        let m = &c.mensajes[0];
        assert_eq!(m.clase, Some(cuaderno::Clase::Imagen));
        assert_eq!(m.referencia.as_deref(), Some(g.lienzo.as_str()));
        // La foto es la captura tal cual (un PNG que se lee con sus medidas).
        let foto = almacen::carpeta(&r, &g.proyecto).join(m.ruta.as_deref().unwrap());
        assert_eq!(pixpin_codec::imagen::medidas(&foto).unwrap(), (64, 18));
        // Y la burbuja la ensena con la tinta encima, en pixeles de la foto.
        let d = crate::foto_anotada::dibujo_de_la_foto(&r, &g.proyecto, m, (64.0, 18.0)).unwrap();
        // (La caja lleva el margen del trazo: se mira su centro, el de la raya.)
        let (x0, y0, x1, y1) = d.trazos.unwrap();
        let centro = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        assert!(
            (centro.0 - 30.0).abs() < 1.0 && (centro.1 - 9.0).abs() < 1.0,
            "{:?}",
            d.trazos
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn dos_sesiones_seguidas_son_dos_fotos_y_no_pisan_la_primera() {
        let r = raiz("dos");
        let s = Sesion {
            foto: pantalla(8, 4),
            tinta: vec![raya(&[(1.0, 1.0), (6.0, 3.0)])],
        };
        let a = guardar(&r, "PC01", &s, 1).unwrap();
        let b = guardar(&r, "PC01", &s, 2).unwrap();
        assert_eq!(a.proyecto, b.proyecto, "las dos en Mensajes guardados");
        assert_ne!(a.mensaje, b.mensaje);
        assert_ne!(a.lienzo, b.lienzo);
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&r, &a.proyecto)).unwrap();
        let rutas: Vec<_> = c.mensajes.iter().map(|m| m.ruta.clone().unwrap()).collect();
        assert_eq!(rutas.len(), 2);
        assert_ne!(
            rutas[0], rutas[1],
            "la segunda captura no pisa el fichero de la primera"
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_tinta_que_se_guarda_es_la_que_se_ve_sin_lo_borrado_ni_imagenes_sueltas() {
        let mut e = pixpin_motor2d::Escena::nueva();
        e.anadir(raya(&[(0.0, 0.0), (5.0, 5.0)]));
        let borrada = e.anadir(raya(&[(1.0, 1.0), (2.0, 2.0)]));
        e.borrar(borrada);
        e.anadir(Elemento {
            figura: Figura::Imagen { id_objeto: 7 },
            ancho: 10.0,
            alto: 10.0,
            ..Elemento::default()
        });
        let t = tinta_de(&e);
        assert_eq!(t.len(), 1);
        assert!(matches!(t[0].figura, Figura::Lapiz { .. }));
        // Caso negativo: una escena vacia no da nada.
        assert!(tinta_de(&pixpin_motor2d::Escena::nueva()).is_empty());
    }

    /// **La burbuja del chat con la pantalla y la tinta guardadas**: se
    /// guarda de verdad (en una carpeta temporal) una «pantalla» de dos
    /// monitores con una flecha que cruza de uno a otro y un trazo, y se
    /// pinta como la pinta la burbuja: la foto y encima lo que devuelve
    /// `dibujo_de_la_foto` leyendo el lienzo guardado.
    /// `cargo test -p pixpin --bin pixpinmax muestra_de_la_pantalla_anotada -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_de_la_pantalla_anotada_en_su_burbuja() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let r = raiz("muestra");
        let (w, h) = (1600u32, 450u32);
        let sesion = Sesion {
            foto: pantalla(w, h),
            tinta: vec![
                flecha((500.0, 300.0), (1100.0, 120.0)),
                // Un circulo a mano alzada alrededor de algo, con los puntos
                // seguidos que da el raton.
                raya(
                    &(0..=64)
                        .map(|i| {
                            let a = i as f32 / 60.0 * std::f32::consts::TAU;
                            (1250.0 + 110.0 * a.cos(), 110.0 + 70.0 * a.sin())
                        })
                        .collect::<Vec<_>>(),
                ),
            ],
        };
        let t0 = std::time::Instant::now();
        let g = guardar(&r, "PC01", &sesion, 5).unwrap();
        println!("guardar {w}x{h}: {} ms", t0.elapsed().as_millis());
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&r, &g.proyecto)).unwrap();
        let d = crate::foto_anotada::dibujo_de_la_foto(
            &r,
            &g.proyecto,
            &c.mensajes[0],
            (w as f32, h as f32),
        )
        .unwrap();
        // Como la burbuja: a la mitad.
        let escala = 0.5;
        let (bw, bh) = (
            (w as f32 * escala) as u32 + 24,
            (h as f32 * escala) as u32 + 24,
        );
        let disp = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(disp.d3d()).expect("motor");
        let fuera = FueraDePantalla::nuevo(&motor, disp.d3d(), bw, bh).expect("superficie");
        let mut imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(4096);
        assert!(imagenes.guardar_con_id(1, sesion.foto.clone()));
        imagenes.asegurar(&motor);
        motor
            .dibujar(&fuera.destino, |p| {
                p.limpiar(pixpin_render::Color {
                    r: 0.93,
                    g: 0.94,
                    b: 0.96,
                    a: 1.0,
                });
                p.poner_vista((12.0, 12.0), escala, (0.0, 0.0));
                let vista = (0.0, 0.0, w as f32, h as f32);
                let foto = pixpin_motor2d::Orden::Imagen {
                    id_objeto: 1,
                    x: 0.0,
                    y: 0.0,
                    ancho: w as f32,
                    alto: h as f32,
                    opacidad: 1.0,
                    recorte: None,
                    angulo: 0.0,
                };
                crate::dibujo::pintar::dibujar_orden(
                    p, &foto, vista, None, &imagenes, escala, None,
                );
                for o in &d.ordenes {
                    crate::dibujo::pintar::dibujar_orden(
                        p, o, vista, None, &imagenes, escala, None,
                    );
                }
            })
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        let (_, _, pixeles) = fuera.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba {
            ancho: bw,
            alto: bh,
            pixeles,
        })
        .unwrap();
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let ruta = carpeta.join("pantalla-anotada-en-burbuja.png");
        std::fs::write(&ruta, png).unwrap();
        println!("pantalla-anotada-en-burbuja: {}", ruta.display());
        let _ = std::fs::remove_dir_all(&r);
    }
}
