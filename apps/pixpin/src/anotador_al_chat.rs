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
//! **Se guarda solo al salir** (2026-09-29: «el anotar sobre la pantalla ...
//! tiene que guardarse automaticamente en el canvas dentro de Mensajes
//! guardados»): Escape, Salir o cerrar con tinta nueva lo llevan al chat sin
//! preguntar ([`EnElChat`], `pantalla::guardar_en_el_chat`). El boton de la
//! pastilla se queda como «guardar ahora»; lo dibujado despues pone al dia
//! EL MISMO lienzo ([`poner_al_dia`]), no manda otro mensaje. Sin tinta no
//! se guarda nada.
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
#[derive(Clone)]
pub struct Globo {
    pub titulo: String,
    pub hecho: String,
    pub fallo: String,
}

/// Lo que toca hacer con lo anotado, al pulsar Guardar o al salir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paso {
    /// Nada que guardar: sin tinta y sin guardar antes, o sin cambios desde
    /// lo ultimo guardado.
    Nada,
    /// La primera vez con tinta en esta sesion: captura de fondo, mensaje
    /// IMAGEN y su lienzo (`guardar`).
    Nuevo,
    /// Ya se guardo en esta sesion y se siguio dibujando (o se limpio): se
    /// pone al dia EL MISMO lienzo, sin otro mensaje (`poner_al_dia`).
    AlDia,
}

/// **Pura**: la regla de cuando se guarda (pedida el 2026-09-29: «tiene que
/// guardarse automaticamente en el canvas dentro de Mensajes guardados»).
/// `guardada` es la tinta tal como se mando la ultima vez en esta sesion.
pub fn que_hacer(guardada: Option<&[Elemento]>, ahora: &[Elemento]) -> Paso {
    match guardada {
        None if ahora.is_empty() => Paso::Nada,
        None => Paso::Nuevo,
        Some(antes) if antes == ahora => Paso::Nada,
        Some(_) => Paso::AlDia,
    }
}

/// **Pone al dia** el lienzo de una pantalla ya guardada con la tinta de
/// ahora (en pixeles de la captura, que mide `medidas`). Ni captura nueva ni
/// mensaje nuevo: la foto de fondo es la de la primera vez, que es sobre la
/// que se dibujo.
pub fn poner_al_dia(
    raiz: &Path,
    g: &Guardado,
    medidas: (u32, u32),
    tinta: &[Elemento],
) -> io::Result<Guardado> {
    let trazos = pixpin_proyecto::lienzo_de_la_foto::cambiar_dibujo(
        raiz,
        &g.proyecto,
        &g.lienzo,
        medidas,
        tinta,
    )?;
    Ok(Guardado {
        trazos,
        ..g.clone()
    })
}

/// **El guardado de UNA sesion del anotador** en «Mensajes guardados».
///
/// La primera vez con tinta crea la foto y su lienzo; las siguientes ponen
/// al dia ese mismo lienzo. Todo en hilos, porque comprimir la captura del
/// escritorio entero a PNG tarda (cientos de milisegundos a 3840x1080 en el
/// portatil) y el anotador tiene que seguir al ritmo del raton; y al salir,
/// la ventana se cierra sin esperar. Cada hilo espera al anterior: el que
/// pone al dia necesita saber donde quedo el primero.
#[derive(Default)]
pub struct EnElChat {
    /// La tinta tal como se mando la ultima vez.
    tinta: Option<Vec<Elemento>>,
    /// Lo que mide la captura de fondo (la tinta va en sus pixeles).
    medidas: (u32, u32),
    /// El ultimo guardado lanzado; devuelve donde quedo.
    hilo: Option<std::thread::JoinHandle<Option<Guardado>>>,
}

impl EnElChat {
    pub fn paso(&self, tinta: &[Elemento]) -> Paso {
        que_hacer(self.tinta.as_deref(), tinta)
    }

    /// La primera vez: la captura de debajo con la tinta, a un mensaje nuevo.
    pub fn guardar_nuevo(&mut self, raiz: PathBuf, sesion: Sesion, hwnd: isize, globo: Globo) {
        self.medidas = (sesion.foto.ancho, sesion.foto.alto);
        self.tinta = Some(sesion.tinta.clone());
        let previo = self.hilo.take();
        self.hilo = lanzar(move || {
            // Uno anterior que fallo no cuenta: este es el primero bueno.
            if let Some(h) = previo {
                let _ = h.join();
            }
            let t0 = std::time::Instant::now();
            let aparato = pixpin_proyecto::identidad::Identidad::leer_o_crear(&raiz, "PC")
                .map(|i| i.yo.codigo())
                .unwrap_or_default();
            let hecho = guardar(
                &raiz,
                &aparato,
                &sesion,
                pixpin_shell::entorno::ahora_utc_ms(),
            );
            avisar(
                hecho,
                t0,
                (sesion.foto.ancho, sesion.foto.alto),
                hwnd,
                &globo,
            )
        });
    }

    /// Las siguientes: el mismo lienzo con la tinta de ahora.
    pub fn poner_al_dia(&mut self, raiz: PathBuf, tinta: Vec<Elemento>, hwnd: isize, globo: Globo) {
        self.tinta = Some(tinta.clone());
        let medidas = self.medidas;
        let previo = self.hilo.take();
        self.hilo = lanzar(move || {
            let t0 = std::time::Instant::now();
            let Some(g) = previo.and_then(|h| h.join().ok().flatten()) else {
                tracing::warn!(
                    "la pantalla anotada no llego a guardarse: no hay lienzo que poner al dia"
                );
                return avisar(
                    Err(io::Error::other("sin guardado previo")),
                    t0,
                    medidas,
                    hwnd,
                    &globo,
                );
            };
            avisar(
                poner_al_dia(&raiz, &g, medidas, &tinta),
                t0,
                medidas,
                hwnd,
                &globo,
            )
        });
    }

    /// Espera a que acabe lo lanzado y dice donde quedo (para las pruebas y
    /// para quien quiera saberlo; el anotador no espera).
    pub fn esperar(&mut self) -> Option<Guardado> {
        let g = self.hilo.take()?.join().ok().flatten();
        // Lo sabido sigue valiendo para la siguiente puesta al dia.
        if let Some(g) = &g {
            let copia = g.clone();
            self.hilo = lanzar(move || Some(copia));
        }
        g
    }
}

fn lanzar(
    f: impl FnOnce() -> Option<Guardado> + Send + 'static,
) -> Option<std::thread::JoinHandle<Option<Guardado>>> {
    std::thread::Builder::new()
        .name("pixpin-anotador-al-chat".into())
        .spawn(f)
        .inspect_err(|e| tracing::warn!(?e, "no se pudo lanzar el guardado de la pantalla anotada"))
        .ok()
}

/// Deja constancia, despierta al chat y saca el globo (`hwnd` es la ventana
/// de mensajes de `main`, duena del icono de la bandeja; 0 = sin globo).
fn avisar(
    hecho: io::Result<Guardado>,
    t0: std::time::Instant,
    (ancho, alto): (u32, u32),
    hwnd: isize,
    globo: &Globo,
) -> Option<Guardado> {
    let (texto, g) = match hecho {
        Ok(g) => {
            tracing::info!(
                proyecto = %g.proyecto,
                mensaje = %g.mensaje,
                lienzo = %g.lienzo,
                trazos = g.trazos,
                ancho,
                alto,
                ms = t0.elapsed().as_millis() as u64,
                "pantalla anotada guardada en Mensajes guardados"
            );
            crate::ventana_chat::refrescar();
            (&globo.hecho, Some(g))
        }
        Err(e) => {
            tracing::warn!(?e, "no se pudo guardar la pantalla anotada");
            (&globo.fallo, None)
        }
    };
    if hwnd != 0 {
        let hwnd = windows::Win32::Foundation::HWND(hwnd as *mut _);
        let _ = pixpin_shell::aviso::Aviso::sobre_la_bandeja(hwnd).mostrar(&globo.titulo, texto);
    }
    g
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

    fn globo_mudo() -> Globo {
        Globo {
            titulo: String::new(),
            hecho: String::new(),
            fallo: String::new(),
        }
    }

    /// El mensaje de la foto en «Mensajes guardados» y su lienzo, leidos.
    fn lo_guardado(r: &Path, g: &Guardado) -> (Vec<cuaderno::Mensaje>, Vec<Elemento>) {
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(r, &g.proyecto)).unwrap();
        let texto = std::fs::read_to_string(almacen::lienzo(r, &g.proyecto, &g.lienzo)).unwrap();
        let l = pixpin_motor2d::excalidraw::leer(&texto).unwrap();
        (c.mensajes, l.elementos())
    }

    #[test]
    fn la_regla_de_guardar_primero_crea_luego_pone_al_dia_y_sin_cambios_no_hace_nada() {
        let una = vec![raya(&[(1.0, 1.0), (5.0, 5.0)])];
        let dos = vec![
            raya(&[(1.0, 1.0), (5.0, 5.0)]),
            raya(&[(9.0, 9.0), (2.0, 2.0)]),
        ];
        assert_eq!(que_hacer(None, &una), Paso::Nuevo);
        assert_eq!(que_hacer(Some(&una), &dos), Paso::AlDia);
        // Limpiar lo ya guardado tambien se lleva al lienzo.
        assert_eq!(que_hacer(Some(&una), &[]), Paso::AlDia);
        // Casos negativos: sin tinta y sin guardar antes, o sin cambios.
        assert_eq!(que_hacer(None, &[]), Paso::Nada);
        assert_eq!(que_hacer(Some(&una), &una), Paso::Nada);
    }

    #[test]
    fn salir_con_tinta_guarda_la_foto_en_mensajes_guardados_con_la_tinta_editable_encima_de_la_foto_bloqueada()
     {
        let r = raiz("salir");
        let tinta = vec![
            raya(&[(10.0, 4.0), (50.0, 14.0)]),
            flecha((5.0, 15.0), (60.0, 2.0)),
        ];
        let mut chat = EnElChat::default();
        assert_eq!(chat.paso(&tinta), Paso::Nuevo);
        chat.guardar_nuevo(
            r.clone(),
            Sesion {
                foto: pantalla(64, 18),
                tinta: tinta.clone(),
            },
            0,
            globo_mudo(),
        );
        let g = chat.esperar().expect("guardado");
        let (mensajes, els) = lo_guardado(&r, &g);
        assert_eq!(mensajes.len(), 1, "un mensaje IMAGEN");
        assert_eq!(mensajes[0].clase, Some(cuaderno::Clase::Imagen));
        assert_eq!(mensajes[0].referencia.as_deref(), Some(g.lienzo.as_str()));
        assert_eq!(els.len(), 3, "la foto y los dos trazos");
        assert!(
            matches!(els[0].figura, Figura::Imagen { .. }) && els[0].bloqueado,
            "la foto primero y bloqueada"
        );
        assert!(els[1..].iter().all(|e| !e.bloqueado), "la tinta, editable");
        // Salir otra vez sin haber tocado nada no guarda otra.
        assert_eq!(chat.paso(&tinta), Paso::Nada);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn salir_sin_tinta_no_guarda_nada_en_el_chat() {
        let r = raiz("salir-vacio");
        let chat = EnElChat::default();
        let e = pixpin_motor2d::Escena::nueva();
        assert_eq!(chat.paso(&tinta_de(&e)), Paso::Nada);
        // Ni se crea «Mensajes guardados» por haber entrado y salido.
        assert!(almacen::Indice::leer(&r).proyectos.is_empty());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn guardar_seguir_dibujando_y_salir_pone_al_dia_el_mismo_lienzo_sin_otro_mensaje() {
        let r = raiz("al-dia");
        let mut tinta = vec![raya(&[(10.0, 4.0), (50.0, 14.0)])];
        let mut chat = EnElChat::default();
        // El boton de guardar, a mitad de la sesion.
        chat.guardar_nuevo(
            r.clone(),
            Sesion {
                foto: pantalla(64, 18),
                tinta: tinta.clone(),
            },
            0,
            globo_mudo(),
        );
        // Se sigue dibujando SIN esperar al primer guardado: el segundo hilo
        // espera al primero.
        tinta.push(raya(&[(2.0, 2.0), (8.0, 16.0)]));
        tinta.push(flecha((30.0, 2.0), (62.0, 16.0)));
        assert_eq!(chat.paso(&tinta), Paso::AlDia);
        chat.poner_al_dia(r.clone(), tinta.clone(), 0, globo_mudo());
        let g = chat.esperar().expect("puesto al dia");
        assert_eq!(g.trazos, 3);
        let (mensajes, els) = lo_guardado(&r, &g);
        assert_eq!(mensajes.len(), 1, "el mismo mensaje, no otro");
        assert_eq!(
            els.len(),
            4,
            "la foto y los tres trazos, sin duplicar el primero"
        );
        assert!(els[0].bloqueado);
        // Y otra puesta al dia tras esperar sigue yendo al mismo sitio.
        tinta.pop();
        chat.poner_al_dia(r.clone(), tinta.clone(), 0, globo_mudo());
        let g2 = chat.esperar().unwrap();
        assert_eq!((g2.mensaje.as_str(), g2.trazos), (g.mensaje.as_str(), 2));
        assert_eq!(lo_guardado(&r, &g2).1.len(), 3);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn poner_al_dia_sin_un_guardado_previo_que_llegara_no_inventa_un_mensaje() {
        let r = raiz("al-dia-sin-previo");
        let mut chat = EnElChat::default();
        chat.poner_al_dia(
            r.clone(),
            vec![raya(&[(1.0, 1.0), (2.0, 2.0)])],
            0,
            globo_mudo(),
        );
        assert_eq!(chat.esperar(), None);
        assert!(almacen::Indice::leer(&r).proyectos.is_empty());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn abrir_la_burbuja_da_el_lienzo_con_la_foto_bloqueada_debajo_y_la_tinta_editable_que_se_guarda_al_cerrar()
     {
        let r = raiz("abrir");
        let s = Sesion {
            foto: pantalla(64, 18),
            tinta: vec![
                raya(&[(10.0, 4.0), (50.0, 14.0)]),
                flecha((5.0, 15.0), (60.0, 2.0)),
            ],
        };
        let g = guardar(&r, "PC01", &s, 7).unwrap();
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&r, &g.proyecto)).unwrap();
        let m = &c.mensajes[0];
        // Lo que recibe el editor al tocar la burbuja (o la tarjeta de
        // Proyectos): la escena del lienzo, no la foto plana.
        let azul = pixpin_motor2d::ColorRgba::opaco(0.1, 0.3, 0.9);
        let (abierta, fotos, guardada) =
            crate::ventana_chat::hoja_abierta_y_guardada(&r, &g.proyecto, m, |e| {
                let ids: Vec<u64> = e.visibles().map(|x| x.id).collect();
                // La goma se lleva la raya; la flecha se mueve y cambia de color.
                assert!(e.borrar_apuntando(ids[1]));
                assert!(e.mover(ids[2], 20.0, 0.0));
                e.buscar_mut(ids[2]).unwrap().trazo = azul;
            })
            .expect("se abre su lienzo");
        let els: Vec<&Elemento> = abierta.visibles().collect();
        assert_eq!(els.len(), 3);
        assert!(
            matches!(els[0].figura, Figura::Imagen { .. }),
            "la foto primero"
        );
        assert!(els[0].bloqueado, "la foto bloqueada");
        assert!(els[1..].iter().all(|e| !e.bloqueado), "la tinta editable");
        // La foto tiene su fichero, que el editor carga.
        assert_eq!(fotos.len(), 1);
        assert!(fotos[0].1.is_file(), "{}", fotos[0].1.display());
        // Y al cerrar se guarda sin aplanar nada: la foto sigue debajo.
        assert!(guardada);
        let (mensajes, despues) = lo_guardado(&r, &g);
        assert_eq!(mensajes.len(), 1);
        assert_eq!(despues.len(), 2, "la foto y la flecha");
        assert!(despues[0].bloqueado && matches!(despues[0].figura, Figura::Imagen { .. }));
        assert!(matches!(despues[1].figura, Figura::Flecha { .. }));
        assert!(
            (despues[1].trazo.b - azul.b).abs() < 0.01
                && (despues[1].trazo.r - azul.r).abs() < 0.01,
            "el color nuevo"
        );
        assert!((despues[1].x - (els[2].x + 20.0)).abs() < 1e-3);
        // Caso negativo: abrir y cerrar sin tocar no reescribe el lienzo.
        let (_, _, otra) =
            crate::ventana_chat::hoja_abierta_y_guardada(&r, &g.proyecto, m, |_| {}).unwrap();
        assert!(!otra);
        let _ = std::fs::remove_dir_all(&r);
    }

    /// **El lienzo que abre la burbuja**, pintado como lo pinta el editor:
    /// la escena que recibe (`hoja_abierta_y_guardada`), con la foto de su
    /// fichero y la tinta encima. Arriba recien guardado al salir del
    /// anotador; abajo tras editarlo (la goma se llevo el circulo, la flecha
    /// se movio y paso a azul) y volverlo a abrir. Cada elemento lleva su
    /// caja: roja la foto bloqueada, verde lo editable.
    /// `cargo test -p pixpin --bin pixpinmax muestra_del_lienzo_abierto -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_del_lienzo_abierto_de_la_pantalla_anotada() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let r = raiz("muestra-abierto");
        let (w, h) = (1600u32, 450u32);
        let s = Sesion {
            foto: pantalla(w, h),
            tinta: vec![
                flecha((500.0, 300.0), (1100.0, 120.0)),
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
        let mut chat = EnElChat::default();
        chat.guardar_nuevo(r.clone(), s, 0, globo_mudo());
        let g = chat.esperar().unwrap();
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&r, &g.proyecto)).unwrap();
        let m = c.mensajes[0].clone();
        let (antes, fotos, _) =
            crate::ventana_chat::hoja_abierta_y_guardada(&r, &g.proyecto, &m, |e| {
                let ids: Vec<u64> = e.visibles().map(|x| x.id).collect();
                e.borrar_apuntando(ids[2]);
                e.mover(ids[1], 0.0, 60.0);
                e.buscar_mut(ids[1]).unwrap().trazo =
                    pixpin_motor2d::ColorRgba::opaco(0.1, 0.35, 0.95);
            })
            .unwrap();
        let (despues, _, _) =
            crate::ventana_chat::hoja_abierta_y_guardada(&r, &g.proyecto, &m, |_| {}).unwrap();
        // La foto del lienzo mide lo del movil (1600 ya cabe): una unidad por
        // pixel. Dos paneles, uno encima del otro, a la mitad.
        let escala = 0.5;
        let (pw, ph) = ((w as f32 * escala) as u32, (h as f32 * escala) as u32);
        let (bw, bh) = (pw + 24, 2 * ph + 36);
        let disp = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(disp.d3d()).expect("motor");
        let fuera = FueraDePantalla::nuevo(&motor, disp.d3d(), bw, bh).expect("superficie");
        let mut imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(4096);
        for (id, ruta) in &fotos {
            assert!(imagenes.guardar_con_id(*id, pixpin_codec::imagen::cargar(ruta).unwrap()));
        }
        imagenes.asegurar(&motor);
        motor
            .dibujar(&fuera.destino, |p| {
                p.limpiar(pixpin_render::Color {
                    r: 0.93,
                    g: 0.94,
                    b: 0.96,
                    a: 1.0,
                });
                for (i, escena) in [&antes, &despues].into_iter().enumerate() {
                    let y0 = 12.0 + i as f32 * (ph as f32 + 12.0);
                    p.poner_vista((12.0, y0), escala, (0.0, 0.0));
                    let vista = (0.0, 0.0, w as f32, h as f32);
                    for o in pixpin_motor2d::ordenes_de_escena(escena) {
                        crate::dibujo::pintar::dibujar_orden(
                            p, &o, vista, None, &imagenes, escala, None,
                        );
                    }
                    p.desplazar(0.0, 0.0);
                    for e in escena.visibles() {
                        let Some((x0, y0e, x1, y1)) =
                            pixpin_motor2d::biblioteca::caja_de(std::slice::from_ref(e))
                        else {
                            continue;
                        };
                        let color = if e.bloqueado {
                            pixpin_render::Color {
                                r: 0.9,
                                g: 0.1,
                                b: 0.1,
                                a: 1.0,
                            }
                        } else {
                            pixpin_render::Color {
                                r: 0.1,
                                g: 0.7,
                                b: 0.2,
                                a: 1.0,
                            }
                        };
                        p.trazar(
                            pixpin_render::RectF {
                                x: 12.0 + x0 * escala,
                                y: y0 + y0e * escala,
                                ancho: (x1 - x0) * escala,
                                alto: (y1 - y0e) * escala,
                            },
                            2.0,
                            color,
                        );
                    }
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
        let ruta = carpeta.join("pantalla-anotada-lienzo-abierto.png");
        std::fs::write(&ruta, png).unwrap();
        println!("pantalla-anotada-lienzo-abierto: {}", ruta.display());
        let _ = std::fs::remove_dir_all(&r);
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
