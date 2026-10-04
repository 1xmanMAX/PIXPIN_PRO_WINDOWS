//! **Lo dibujado en el movil encima de una foto del chat.**
//!
//! En el movil, anotar una foto del chat no la toca: crea un lienzo propio,
//! `lienzos/<referencia o foto-<id del mensaje>>.excalidraw`
//! (`Mensaje.dibujoDeLaFoto` en `guardados/Mensajes.kt`), con la foto como
//! primer elemento (bloqueada, en `0,0` y a un ancho suyo, no a sus pixeles)
//! y las rayas, cotas y flechas encima. Ese lienzo viaja al sincronizar.
//!
//! Aqui la burbuja de la foto solo buscaba lo dibujado en el `.pixpin2d`
//! pegado a la foto (lo que se dibuja en este equipo), asi que de una foto
//! anotada en el movil **solo llegaba la foto**. Este modulo lee ese lienzo:
//! para ensenarlo en la burbuja (en coordenadas de la foto) y para abrirlo al
//! pulsarla, donde se sigue dibujando en el MISMO fichero que el movil.

use std::path::{Path, PathBuf};

use pixpin_motor2d::{Elemento, Escena, Figura, Orden, Punto2};
use pixpin_proyecto::cuaderno::Mensaje;

/// El id del lienzo de la foto, como el movil: su `referencia` si la tiene
/// (fotos de antes) y si no `foto-<id del mensaje>`.
pub fn id_del_lienzo(m: &Mensaje) -> String {
    pixpin_proyecto::lienzo_de_la_foto::id_del_lienzo(m)
}

/// **Deja la foto `indice` con su lienzo, como el movil** (K11): si no lo
/// tiene lo crea con la foto dentro, pasa a el lo que hubiera en su
/// `.pixpin2d` y apunta la `referencia` en el cuaderno y en `mensajes`. La
/// regla vive en `pixpin_proyecto::lienzo_de_la_foto`; aqui solo se mide la
/// foto y se deja constancia. `false` si no se pudo (la foto no esta, o no
/// se deja leer): quien llama sigue como antes, con el `.pixpin2d`.
pub fn asegurar_lienzo(
    raiz: &Path,
    proyecto: &str,
    mensajes: &mut [Mensaje],
    indice: usize,
) -> bool {
    let Some(m) = mensajes.get(indice) else {
        return false;
    };
    let Some(foto) = m
        .ruta
        .as_deref()
        .filter(|r| !r.is_empty())
        .and_then(|r| pixpin_proyecto::vista::ruta_real(raiz, proyecto, r))
    else {
        return false;
    };
    // Solo la cabecera: las rayas del `.pixpin2d` estan en pixeles de la foto.
    let medidas = match pixpin_codec::imagen::medidas(&foto) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(?e, ruta = %foto.display(), "foto que no se deja medir: sin lienzo propio");
            return false;
        }
    };
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    match pixpin_proyecto::lienzo_de_la_foto::asegurar(raiz, proyecto, m, &foto, medidas, ahora) {
        Ok(h) => {
            tracing::info!(
                %proyecto,
                lienzo = %h.id,
                creado = h.creado,
                adoptados = h.adoptados,
                referencia = h.referencia_puesta,
                "lienzo de la foto"
            );
            if mensajes[indice]
                .referencia
                .as_deref()
                .is_none_or(str::is_empty)
            {
                mensajes[indice].referencia = Some(h.id);
            }
            true
        }
        Err(e) => {
            tracing::warn!(?e, ruta = %foto.display(), "no se pudo crear el lienzo de la foto");
            false
        }
    }
}

/// Abre la foto `indice` en SU lienzo, como el movil: creandolo si hace
/// falta ([`asegurar_lienzo`]). `false` si no hay manera (entonces se abre
/// como antes, con la foto de fondo y el `.pixpin2d`).
pub fn abrir_en_su_lienzo(
    raiz: &Path,
    proyecto: &str,
    mensajes: &mut [Mensaje],
    indice: usize,
    opciones: crate::ventana_chat::OpcionesLienzo,
) -> bool {
    asegurar_lienzo(raiz, proyecto, mensajes, indice);
    abrir_si_tiene_lienzo(raiz, proyecto, mensajes, indice, opciones)
}

/// El lienzo de la foto en este equipo, si ya llego.
pub fn lienzo_de_la_foto(raiz: &Path, proyecto: &str, m: &Mensaje) -> Option<(String, PathBuf)> {
    let id = id_del_lienzo(m);
    let ruta = pixpin_proyecto::almacen::lienzo(raiz, proyecto, &id);
    ruta.is_file().then_some((id, ruta))
}

/// Lo dibujado encima de la foto, llevado a pixeles de la foto.
#[derive(Debug, Clone, PartialEq)]
pub struct DibujoDeLaFoto {
    pub ordenes: Vec<Orden>,
    /// La caja de lo dibujado, en pixeles de la foto (para «ver el dibujo
    /// entero», `encuadre_de_foto`).
    pub trazos: Option<(f32, f32, f32, f32)>,
}

/// Lo que el movil dibujo encima de la foto `m` (de `doc` pixeles), o `None`
/// si no hay lienzo o solo tiene la foto.
pub fn dibujo_de_la_foto(
    raiz: &Path,
    proyecto: &str,
    m: &Mensaje,
    doc: (f32, f32),
) -> Option<DibujoDeLaFoto> {
    let (_, ruta) = lienzo_de_la_foto(raiz, proyecto, m)?;
    let texto = std::fs::read_to_string(&ruta)
        .inspect_err(
            |e| tracing::warn!(?e, ruta = %ruta.display(), "lienzo de foto que no se pudo leer"),
        )
        .ok()?;
    let lienzo = pixpin_motor2d::excalidraw::leer(&texto)
        .inspect_err(|e| tracing::warn!(?e, "lienzo de foto que no se entiende"))
        .ok()?;
    dibujo_encima(
        &lienzo.elementos(),
        doc,
        pixpin_motor2d::excalidraw::fondo(&lienzo),
    )
}

/// La cuenta, sin disco: todo lo que no es la foto, pasado de unidades del
/// lienzo a pixeles de la foto, con la tinta como la pinta el lienzo sobre
/// su `papel` (en papel de noche, adaptada: `dibujo::tema`). Sin esto la
/// tinta negra de un lienzo de pizarra salia negra en la burbuja.
fn dibujo_encima(
    elementos: &[Elemento],
    doc: (f32, f32),
    papel: pixpin_motor2d::ColorRgba,
) -> Option<DibujoDeLaFoto> {
    let visibles: Vec<&Elemento> = elementos.iter().filter(|e| !e.borrado).collect();
    // La foto es la imagen que el movil puso primero al crear el lienzo
    // (`foto-<hora>-0`); si el id no lo dice, la primera imagen.
    let es_foto = |e: &&Elemento| matches!(e.figura, Figura::Imagen { .. });
    let la_foto = visibles
        .iter()
        .copied()
        .filter(es_foto)
        .find(|e| {
            e.extras
                .id_de_fichero
                .as_deref()
                .is_some_and(|i| i.starts_with("foto-"))
        })
        .or_else(|| visibles.iter().copied().find(es_foto));
    let mut escena = Escena::nueva();
    for e in visibles
        .iter()
        .filter(|e| la_foto.is_none_or(|f| !std::ptr::eq(**e, f)))
    {
        escena.anadir((*e).clone());
    }
    let ordenes = crate::dibujo::tema::ordenes_como_en_el_lienzo(
        pixpin_motor2d::ordenes_de_escena(&escena),
        papel,
    );
    if ordenes.is_empty() {
        return None;
    }
    // De unidades del lienzo a pixeles de la foto: la foto del lienzo mide lo
    // que el movil le dio (1600 de ancho), no lo que mide el fichero. Sin
    // foto en el lienzo no hay de donde sacar la escala y se deja tal cual.
    let (ox, oy, sx, sy) = match la_foto {
        Some(f) if f.ancho > 0.0 && f.alto > 0.0 => (f.x, f.y, doc.0 / f.ancho, doc.1 / f.alto),
        _ => (0.0, 0.0, 1.0, 1.0),
    };
    let llevar = |p: Punto2| Punto2::nuevo((p.x - ox) * sx, (p.y - oy) * sy);
    let s = (sx + sy) / 2.0;
    let ordenes = ordenes
        .into_iter()
        .map(|o| llevar_orden(o, &llevar, s, (sx, sy)))
        .collect();
    let trazos = escena.caja().map(|(x0, y0, x1, y1)| {
        let (a, b) = (llevar(Punto2::nuevo(x0, y0)), llevar(Punto2::nuevo(x1, y1)));
        (a.x, a.y, b.x, b.y)
    });
    Some(DibujoDeLaFoto { ordenes, trazos })
}

/// Una orden pasada a otra escala: los puntos por `llevar`, los grosores y
/// la letra por `s`.
fn llevar_orden(
    o: Orden,
    llevar: &impl Fn(Punto2) -> Punto2,
    s: f32,
    (sx, sy): (f32, f32),
) -> Orden {
    let todos = |v: Vec<Punto2>| v.into_iter().map(llevar).collect::<Vec<_>>();
    match o {
        Orden::Poligono { puntos, color } => Orden::Poligono {
            puntos: todos(puntos),
            color,
        },
        Orden::Tinta { contorno, color } => Orden::Tinta {
            contorno: todos(contorno),
            color,
        },
        Orden::Relleno { puntos, color } => Orden::Relleno {
            puntos: todos(puntos),
            color,
        },
        Orden::Velo { hueco, color } => Orden::Velo {
            hueco: todos(hueco),
            color,
        },
        Orden::Polilinea {
            puntos,
            color,
            grosor,
            estilo,
        } => Orden::Polilinea {
            puntos: todos(puntos),
            color,
            grosor: grosor * s,
            estilo,
        },
        Orden::Texto {
            texto,
            x,
            y,
            tam,
            familia,
            color,
            ancho_max,
            negrita,
            cursiva,
        } => {
            let p = llevar(Punto2::nuevo(x, y));
            Orden::Texto {
                texto,
                x: p.x,
                y: p.y,
                tam: tam * s,
                familia,
                color,
                // «Sin partir» sigue siendo sin partir.
                ancho_max: if ancho_max >= pixpin_motor2d::texto::SIN_PARTIR {
                    ancho_max
                } else {
                    ancho_max * s
                },
                negrita,
                cursiva,
            }
        }
        Orden::Imagen {
            id_objeto,
            x,
            y,
            ancho,
            alto,
            opacidad,
            recorte,
            angulo,
        } => {
            let p = llevar(Punto2::nuevo(x, y));
            Orden::Imagen {
                id_objeto,
                x: p.x,
                y: p.y,
                ancho: ancho * sx,
                alto: alto * sy,
                opacidad,
                recorte,
                angulo,
            }
        }
        // El numero de una cota: su centro se lleva y su caja va con el, a
        // la escala de la letra.
        Orden::Rotulo {
            texto,
            x,
            y,
            tam,
            familia,
            color,
            halo,
            grosor_halo,
            centro,
            angulo,
        } => {
            let c = llevar(centro);
            Orden::Rotulo {
                texto,
                x: c.x - (centro.x - x) * s,
                y: c.y - (centro.y - y) * s,
                tam: tam * s,
                familia,
                color,
                halo,
                grosor_halo: grosor_halo * s,
                centro: c,
                angulo,
            }
        }
    }
}

/// La caja que ocupa `caja` girada `angulo` alrededor de su centro: lo que
/// hay que encuadrar para que una foto girada quepa entera en su burbuja.
pub fn caja_girada(caja: (f32, f32, f32, f32), angulo: f32) -> (f32, f32, f32, f32) {
    if angulo == 0.0 {
        return caja;
    }
    let (x0, y0, x1, y1) = caja;
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let esquinas = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
        .map(|(x, y)| Punto2::nuevo(x, y).girar(centro, angulo));
    esquinas.iter().fold(
        (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
        |(a, b, c, d), p| (a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)),
    )
}

/// Abre en el lienzo el dibujo que el movil hizo sobre la foto `indice`, si
/// lo hay: el mismo fichero, con la foto y todo lo dibujado, y lo que se
/// dibuje aqui vuelve alli. `false` si la foto no tiene lienzo (entonces se
/// abre como siempre, con la foto de fondo).
pub fn abrir_si_tiene_lienzo(
    raiz: &Path,
    proyecto: &str,
    mensajes: &[Mensaje],
    indice: usize,
    opciones: crate::ventana_chat::OpcionesLienzo,
) -> bool {
    let Some(m) = mensajes.get(indice) else {
        return false;
    };
    let Some((id, _)) = lienzo_de_la_foto(raiz, proyecto, m) else {
        return false;
    };
    // La hoja se abre por su `referencia`: una foto nueva del movil no la
    // lleva (su lienzo se llama por el id del mensaje), asi que se le pone
    // en una copia, sin tocar el mensaje de verdad.
    let mut copia = mensajes.to_vec();
    copia[indice].referencia = Some(id);
    crate::ventana_chat::abrir_hojas(raiz, proyecto, &copia, indice, opciones);
    true
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un lienzo de foto con la MISMA forma que los del movil: la `Scene`
    /// entera, la foto primero (bloqueada, 1600 de ancho en `0,0`, sin
    /// `points`), y encima una elipse y un trazo a mano con los puntos en
    /// pares, relativos al elemento. Datos inventados.
    const LIENZO_DE_FOTO: &str = r##"{"elements":[
      {"id":"foto-1790000000000-0","type":"image","x":0,"y":0,"width":1600,"height":1200,"angle":0,
       "strokeColor":"#1e1e1e","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":2,
       "strokeStyle":"solid","roughness":1,"opacity":100,"seed":1,"version":1,"versionNonce":0,
       "isDeleted":false,"groupIds":[],"updated":0,"locked":true,"reference":false,
       "simulatePressure":true,"presionFirme":false,"elbowed":false,"negrita":false,"cursiva":false,
       "tachado":false,"cota":0,"giroEnPlanta":0,"formaSolida":"caja","inclinacion":0,
       "esqueleto":false,"enElSuelo":false,"pauta":"lisa","tareas":[],"periodos":6,"material":"lisa",
       "mosaicBlur":false,"lupaRedonda":true,"lupaFlecha":true,"lupaDosLineas":false,
       "fileId":"AbCdEfGhIjKlMnOpQrStU","scale":[1,1]},
      {"id":"elip-1","type":"ellipse","x":400,"y":300,"width":200,"height":100,"angle":0,
       "strokeColor":"#ffffff","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":6,
       "strokeStyle":"solid","roughness":1,"opacity":100,"seed":1954946640,"version":1,
       "versionNonce":-776232402,"isDeleted":false,"groupIds":[],"updated":1790000000001,
       "locked":false,"reference":false,"simulatePressure":true,"material":"lisa","scale":[1,1]},
      {"id":"raya-1","type":"freedraw","x":800,"y":600,"width":100,"height":50,"angle":0,
       "strokeColor":"#1971c2","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":2,
       "strokeStyle":"solid","roughness":1,"opacity":100,"seed":7,"version":3,"versionNonce":5,
       "isDeleted":false,"groupIds":[],"updated":1790000000002,"locked":false,
       "points":[[0,0],[50,25],[100,50]],"pressures":[],"simulatePressure":true,
       "material":"lisa","scale":[1,1]}],
      "files":{"AbCdEfGhIjKlMnOpQrStU":{"id":"AbCdEfGhIjKlMnOpQrStU","mimeType":"image/jpeg",
        "path":"pixpin:files/pins/draw/files/AbCdEfGhIjKlMnOpQrStU","created":1790000000000}},
      "viewport":{"scrollX":0,"scrollY":0,"zoom":1},"style":{"strokeColor":"#1e1e1e"},
      "backgroundColor":"#ffffff","luces":{"encendidas":true,"fuerza":1},"tablas":[],
      "referenciasVisibles":true,"alfileres":[],"vista":"cero"}"##;

    fn raiz(etiqueta: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!(
            "pixpin-foto-anotada-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(r.join("proyectos").join("p1").join("lienzos")).unwrap();
        r
    }

    fn foto(id: &str, referencia: Option<&str>) -> Mensaje {
        let sello = pixpin_proyecto::cuaderno::Sello {
            cuando: 1_790_000_000_000,
            numero: 1,
            aparato: "Z5WA".into(),
            proyecto: String::new(),
        };
        let mut m = Mensaje::adjunto(
            pixpin_proyecto::cuaderno::Clase::Imagen,
            "foto.jpg",
            "pixpin:files/guardados/foto.jpg",
            10,
            &sello,
        );
        m.id = id.into();
        m.referencia = referencia.map(str::to_string);
        m
    }

    fn poner(raiz: &Path, id: &str, json: &str) {
        std::fs::write(pixpin_proyecto::almacen::lienzo(raiz, "p1", id), json).unwrap();
    }

    fn puntos(ordenes: &[Orden]) -> Vec<Punto2> {
        ordenes
            .iter()
            .flat_map(|o| match o {
                Orden::Poligono { puntos, .. }
                | Orden::Tinta {
                    contorno: puntos, ..
                }
                | Orden::Polilinea { puntos, .. }
                | Orden::Relleno { puntos, .. }
                | Orden::Velo { hueco: puntos, .. } => puntos.clone(),
                Orden::Texto { x, y, .. } | Orden::Imagen { x, y, .. } => {
                    vec![Punto2::nuevo(*x, *y)]
                }
                Orden::Rotulo { centro, .. } => vec![*centro],
            })
            .collect()
    }

    #[test]
    fn lo_dibujado_en_el_movil_sobre_una_foto_llega_a_su_burbuja_en_pixeles_de_la_foto() {
        let r = raiz("llega");
        poner(&r, "foto-m1", LIENZO_DE_FOTO);
        // La foto de verdad mide el doble que en el lienzo (1600 -> 3200).
        let d = dibujo_de_la_foto(&r, "p1", &foto("m1", None), (3200.0, 2400.0))
            .expect("la foto anotada trae su dibujo");
        assert!(!d.ordenes.is_empty());
        assert!(
            !d.ordenes.iter().any(|o| matches!(o, Orden::Imagen { .. })),
            "la foto no se pinta dos veces encima de si misma"
        );
        // La elipse (400..600, 300..400) y la raya (800..900, 600..650), al doble.
        let (x0, y0, x1, y1) = d.trazos.expect("con caja");
        assert!(
            (x0 - 800.0).abs() < 30.0 && (y0 - 600.0).abs() < 30.0,
            "{x0} {y0}"
        );
        assert!(
            (x1 - 1800.0).abs() < 30.0 && (y1 - 1300.0).abs() < 30.0,
            "{x1} {y1}"
        );
        for p in puntos(&d.ordenes) {
            assert!(
                p.x > 700.0 && p.y > 500.0,
                "todo cae sobre lo anotado: {p:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&r);
    }

    /// Los colores de unas ordenes (las que pintan tinta).
    fn colores(ordenes: &[Orden]) -> Vec<pixpin_motor2d::ColorRgba> {
        ordenes
            .iter()
            .filter_map(|o| match o {
                Orden::Poligono { color, .. }
                | Orden::Tinta { color, .. }
                | Orden::Polilinea { color, .. }
                | Orden::Relleno { color, .. }
                | Orden::Texto { color, .. } => Some(*color),
                _ => None,
            })
            .collect()
    }

    /// El lienzo de foto de ejemplo con otro papel y la raya en negro.
    fn con_papel_y_raya_negra(papel: &str) -> String {
        let mut v: serde_json::Value = serde_json::from_str(LIENZO_DE_FOTO).unwrap();
        v["backgroundColor"] = serde_json::Value::String(papel.into());
        v["elements"][2]["strokeColor"] = serde_json::Value::String("#1e1e1e".into());
        v.to_string()
    }

    #[test]
    fn en_la_burbuja_la_tinta_negra_de_un_lienzo_de_pizarra_se_ve_clara_como_en_el_lienzo() {
        let r = raiz("pizarra");
        poner(&r, "foto-m7", &con_papel_y_raya_negra("#000000"));
        let d =
            dibujo_de_la_foto(&r, "p1", &foto("m7", None), (1600.0, 1200.0)).expect("con dibujo");
        let c = colores(&d.ordenes);
        assert!(!c.is_empty());
        // La raya negra sale clara (como la pinta el lienzo) y la elipse
        // blanca sigue blanca: nada queda negro sobre el papel negro.
        for k in &c {
            assert!(k.r > 0.8 && k.g > 0.8 && k.b > 0.8, "{k:?}");
        }
        // Caso negativo: el mismo dibujo en papel blanco no cambia su tinta.
        poner(&r, "foto-m7", &con_papel_y_raya_negra("#ffffff"));
        let d =
            dibujo_de_la_foto(&r, "p1", &foto("m7", None), (1600.0, 1200.0)).expect("con dibujo");
        let c = colores(&d.ordenes);
        assert!(
            c.iter().any(|k| (k.r - 30.0 / 255.0).abs() < 0.01),
            "la raya sigue negra: {c:?}"
        );
        assert!(
            c.iter().any(|k| k.r > 0.99 && k.g > 0.99),
            "la elipse sigue blanca: {c:?}"
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_referencia_de_una_foto_vieja_manda_sobre_el_id_del_mensaje() {
        let r = raiz("referencia");
        poner(&r, "foto-antigua", LIENZO_DE_FOTO);
        let m = foto("m2", Some("foto-antigua"));
        assert_eq!(id_del_lienzo(&m), "foto-antigua");
        assert!(dibujo_de_la_foto(&r, "p1", &m, (1600.0, 1200.0)).is_some());
        // Y sin referencia se busca por el id, que aqui no esta.
        assert!(dibujo_de_la_foto(&r, "p1", &foto("m2", None), (1600.0, 1200.0)).is_none());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn una_foto_sin_lienzo_o_con_solo_la_foto_no_trae_dibujo() {
        let r = raiz("sin");
        // Caso negativo: no hay lienzo.
        assert!(dibujo_de_la_foto(&r, "p1", &foto("m3", None), (1600.0, 1200.0)).is_none());
        // Caso negativo: un lienzo con solo la foto (se abrio y no se dibujo).
        let solo: serde_json::Value = serde_json::from_str(LIENZO_DE_FOTO).unwrap();
        let mut solo = solo;
        solo["elements"].as_array_mut().unwrap().truncate(1);
        poner(&r, "foto-m3", &solo.to_string());
        assert!(dibujo_de_la_foto(&r, "p1", &foto("m3", None), (1600.0, 1200.0)).is_none());
        // Caso negativo: un fichero roto no rompe la burbuja.
        poner(&r, "foto-m3", "{no es json");
        assert!(dibujo_de_la_foto(&r, "p1", &foto("m3", None), (1600.0, 1200.0)).is_none());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn lo_borrado_en_el_movil_no_sale_en_la_burbuja() {
        let r = raiz("borrado");
        let mut v: serde_json::Value = serde_json::from_str(LIENZO_DE_FOTO).unwrap();
        for e in v["elements"].as_array_mut().unwrap().iter_mut().skip(1) {
            e["isDeleted"] = serde_json::Value::Bool(true);
        }
        poner(&r, "foto-m4", &v.to_string());
        assert!(dibujo_de_la_foto(&r, "p1", &foto("m4", None), (1600.0, 1200.0)).is_none());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn una_foto_girada_un_cuarto_de_vuelta_encuadra_de_pie_y_una_derecha_igual() {
        let caja = (0.0, 0.0, 400.0, 200.0);
        let (x0, y0, x1, y1) = caja_girada(caja, std::f32::consts::FRAC_PI_2);
        assert!((x1 - x0 - 200.0).abs() < 1e-3 && (y1 - y0 - 400.0).abs() < 1e-3);
        // Alrededor de su centro (200, 100): no se va de sitio.
        assert!(((x0 + x1) / 2.0 - 200.0).abs() < 1e-3 && ((y0 + y1) / 2.0 - 100.0).abs() < 1e-3);
        // Caso negativo: sin giro, la misma caja.
        assert_eq!(caja_girada(caja, 0.0), caja);
    }

    /// Una foto inventada: cuatro cuartos de colores (rojo arriba a la
    /// izquierda), para ver a ojo hacia donde esta girada.
    fn foto_de_cuartos(ancho: u32, alto: u32) -> pixpin_codec::imagen::ImagenRgba {
        let mut pixeles = Vec::with_capacity((ancho * alto * 4) as usize);
        for y in 0..alto {
            for x in 0..ancho {
                let c = match (x < ancho / 2, y < alto / 2) {
                    (true, true) => [220, 40, 40],
                    (false, true) => [40, 170, 60],
                    (true, false) => [40, 90, 220],
                    (false, false) => [240, 200, 40],
                };
                pixeles.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        pixpin_codec::imagen::ImagenRgba {
            ancho,
            alto,
            pixeles,
        }
    }

    /// Pinta sin ventana y guarda el PNG en `PIXPIN_MUESTRAS` (o el temporal).
    fn a_png(
        nombre: &str,
        (ancho, alto): (u32, u32),
        fotos: Vec<(u64, pixpin_codec::imagen::ImagenRgba)>,
        pintar: impl FnOnce(&pixpin_render::Pintor<'_>, &crate::imagenes_lienzo::ImagenesLienzo),
    ) {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ancho, alto).expect("superficie");
        let mut imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(4096);
        for (id, img) in fotos {
            assert!(imagenes.guardar_con_id(id, img));
        }
        imagenes.asegurar(&motor);
        motor
            .dibujar(&fuera.destino, |p| pintar(p, &imagenes))
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        let (_, _, pixeles) = fuera.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
            ancho,
            alto,
            pixeles,
        })
        .expect("codificar");
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let ruta = carpeta.join(format!("{nombre}.png"));
        std::fs::write(&ruta, png).expect("guardar");
        println!("{nombre}: {}", ruta.display());
    }

    /// **El lienzo del movil tal cual llega**: papel «Azul noche» arriba en
    /// la escena, la foto girada un cuarto de vuelta y encima tiza blanca y
    /// una elipse roja. Antes: papel blanco (la tiza no se veia) y la foto
    /// derecha. `cargo test -p pixpin --bin pixpinmax muestra_del_lienzo_del_movil
    /// -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_del_lienzo_del_movil_girado_y_con_papel() {
        let json = r##"{"elements":[
          {"id":"img-1","type":"image","x":100,"y":150,"width":400,"height":300,"angle":1.5813,
           "strokeColor":"#1e1e1e","backgroundColor":"transparent","seed":1,"version":2,
           "isDeleted":false,"locked":false,"fileId":"Qwertyuiopasdfghjklzx","scale":[1,1]},
          {"id":"t-1","type":"freedraw","x":80,"y":80,"width":440,"height":40,"angle":0,
           "strokeColor":"#ffffff","backgroundColor":"transparent","strokeWidth":4,"seed":3,
           "version":1,"isDeleted":false,"points":[[0,20],[110,0],[220,40],[330,0],[440,20]],
           "pressures":[],"simulatePressure":true},
          {"id":"e-1","type":"ellipse","x":220,"y":220,"width":160,"height":160,"angle":0,
           "strokeColor":"#ff6b6b","backgroundColor":"transparent","strokeWidth":4,"roughness":1,
           "seed":9,"version":1,"isDeleted":false},
          {"id":"l-1","type":"line","x":60,"y":560,"width":480,"height":0,"angle":0,
           "strokeColor":"#ffffff","backgroundColor":"transparent","strokeWidth":3,"roughness":0,
           "seed":4,"version":1,"isDeleted":false,"points":[[0,0],[480,0]]}],
          "files":{"Qwertyuiopasdfghjklzx":{"id":"Qwertyuiopasdfghjklzx","mimeType":"image/png",
            "path":"pixpin:files/pins/draw/files/Qwertyuiopasdfghjklzx","created":1}},
          "viewport":{"scrollX":0,"scrollY":0,"zoom":1},"style":{"backgroundColor":"transparent"},
          "backgroundColor":"#14213d","vista":"cero"}"##;
        let lienzo = pixpin_motor2d::excalidraw::leer(json).unwrap();
        let escena = pixpin_motor2d::excalidraw::a_escena(&lienzo);
        let (id_foto, _) = pixpin_motor2d::excalidraw::ficheros(&lienzo)[0].clone();
        let papel = escena.fondo;
        a_png(
            "lienzo-del-movil-girado",
            (600, 620),
            vec![(id_foto, foto_de_cuartos(400, 300))],
            |p, imagenes| {
                p.limpiar(crate::dibujo::pintar::a_color(papel));
                let mut cache = pixpin_motor2d::cache::Cache::nueva();
                let mut tinta = pixpin_render::CacheTinta::nueva();
                crate::dibujo::pintar::pintar_escena(
                    p,
                    &escena,
                    &mut cache,
                    &mut tinta,
                    imagenes,
                    (0.0, 0.0, 600.0, 620.0),
                    1.0,
                    |_| false,
                );
            },
        );
    }

    /// **La burbuja de una foto anotada en el movil**: la foto (a su tamano
    /// real, el doble que en el lienzo) y encima lo dibujado alli, llevado a
    /// sus pixeles. Antes: la foto sola.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_de_la_foto_anotada_en_su_burbuja() {
        let r = raiz("muestra");
        poner(&r, "foto-m9", LIENZO_DE_FOTO);
        let (w, h) = (1600u32, 1200u32);
        let d = dibujo_de_la_foto(
            &r,
            "p1",
            &foto("m9", None),
            (w as f32 * 2.0, h as f32 * 2.0),
        )
        .unwrap();
        let escala = 0.25;
        a_png(
            "foto-anotada-en-burbuja",
            (800, 600),
            vec![(1, foto_de_cuartos(w * 2, h * 2))],
            |p, imagenes| {
                p.limpiar(pixpin_render::Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                });
                p.poner_vista((0.0, 0.0), escala, (0.0, 0.0));
                let vista = (0.0, 0.0, w as f32 * 2.0, h as f32 * 2.0);
                let foto = Orden::Imagen {
                    id_objeto: 1,
                    x: 0.0,
                    y: 0.0,
                    ancho: w as f32 * 2.0,
                    alto: h as f32 * 2.0,
                    opacidad: 1.0,
                    recorte: None,
                    angulo: 0.0,
                };
                crate::dibujo::pintar::dibujar_orden(p, &foto, vista, None, imagenes, escala, None);
                for o in &d.ordenes {
                    crate::dibujo::pintar::dibujar_orden(p, o, vista, None, imagenes, escala, None);
                }
            },
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    /// Una foto del PC de verdad (un PNG pequeno, pero con la cabecera de
    /// `ancho`x`alto`), apuntada en el cuaderno y en el indice. Datos inventados.
    fn foto_del_pc(r: &Path, id: &str, (ancho, alto): (u32, u32)) -> Mensaje {
        let carpeta = r.join("proyectos").join("p1");
        std::fs::create_dir_all(carpeta.join("archivos")).unwrap();
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
            ancho,
            alto,
            pixeles: vec![200; (ancho * alto * 4) as usize],
        })
        .unwrap();
        std::fs::write(carpeta.join("archivos").join(format!("{id}.png")), png).unwrap();
        let mut indice = pixpin_proyecto::almacen::Indice::leer(r);
        if indice.buscar("p1").is_none() {
            let mut f = pixpin_proyecto::almacen::Ficha::nueva("Obra", 1, "PC01");
            f.id = "p1".into();
            indice.proyectos.push(f);
            indice.guardar(r).unwrap();
        }
        let mut m = foto(id, None);
        m.ruta = Some(format!("archivos/{id}.png"));
        pixpin_proyecto::cuaderno::anadir(&carpeta, &m).unwrap();
        m
    }

    fn cuaderno(r: &Path) -> Vec<Mensaje> {
        pixpin_proyecto::cuaderno::Cuaderno::leer_de(&r.join("proyectos").join("p1"))
            .unwrap()
            .mensajes
    }

    #[test]
    fn anotar_en_el_pc_una_foto_sin_lienzo_lo_estrena_como_el_movil_y_viaja() {
        use pixpin_sincro::disco::Disco;
        let r = raiz("estrena");
        let mut mensajes = vec![foto_del_pc(&r, "m10", (4200, 100))];
        assert!(asegurar_lienzo(&r, "p1", &mut mensajes, 0));
        // La referencia, en la lista del chat y en el cuaderno.
        assert_eq!(mensajes[0].referencia.as_deref(), Some("foto-m10"));
        assert_eq!(cuaderno(&r)[0].referencia.as_deref(), Some("foto-m10"));
        // El lienzo, con la foto a su ancho del movil (4200 -> /4 = 1050).
        let (id, ruta) = lienzo_de_la_foto(&r, "p1", &mensajes[0]).expect("lienzo creado");
        assert_eq!(id, "foto-m10");
        let l = pixpin_motor2d::excalidraw::leer(&std::fs::read_to_string(ruta).unwrap()).unwrap();
        let f = &l.elementos()[0];
        assert!(matches!(f.figura, Figura::Imagen { .. }) && f.bloqueado);
        assert_eq!((f.ancho, f.alto), (1050.0, 25.0));
        // Sin nada dibujado, la burbuja no pinta nada encima.
        assert!(dibujo_de_la_foto(&r, "p1", &mensajes[0], (4200.0, 100.0)).is_none());
        // Y entra en lo que se sincroniza, con su foto.
        let d = pixpin_proyecto::vista::DiscoPc::nuevo(&r);
        let chat = pixpin_proyecto::vista::chat_de_ficha(&r, "p1").unwrap();
        let alcance: Vec<String> = d
            .alcance(&chat)
            .unwrap()
            .into_iter()
            .map(|(x, _)| x)
            .collect();
        assert!(
            alcance.contains(&"pins/draw/foto-m10.excalidraw.gz".to_string()),
            "{alcance:?}"
        );
        assert!(
            alcance.iter().any(|x| x.contains("/imagenes/foto")),
            "{alcance:?}"
        );
        // Caso negativo: la segunda vez no crea otro ni reescribe el cuaderno.
        let antes = std::fs::read_to_string(r.join("proyectos/p1/guardados.jsonl")).unwrap();
        assert!(asegurar_lienzo(&r, "p1", &mut mensajes, 0));
        assert_eq!(
            std::fs::read_to_string(r.join("proyectos/p1/guardados.jsonl")).unwrap(),
            antes
        );
        assert_eq!(
            std::fs::read_dir(r.join("proyectos/p1/imagenes"))
                .unwrap()
                .count(),
            1
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn lo_dibujado_en_el_pc_antes_de_tener_lienzo_se_sigue_viendo_en_su_sitio_tras_adoptarlo() {
        let r = raiz("adopta");
        let mut mensajes = vec![foto_del_pc(&r, "m11", (4200, 100))];
        let foto = r.join("proyectos/p1/archivos/m11.png");
        let mut escena = Escena::nueva();
        escena.anadir(Elemento {
            figura: Figura::Lapiz {
                puntos: vec![Punto2::nuevo(1000.0, 20.0), Punto2::nuevo(3000.0, 80.0)],
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            },
            grosor: 8.0,
            ..Elemento::default()
        });
        let viejo = pixpin_proyecto::lienzo_de_la_foto::pixpin2d_de(&foto);
        pixpin_motor2d::guardar(&viejo, &escena).unwrap();
        assert!(asegurar_lienzo(&r, "p1", &mut mensajes, 0));
        assert!(!viejo.exists(), "el .pixpin2d se aparta");
        // La burbuja lo lee ahora del lienzo, y cae donde estaba en la foto.
        let d =
            dibujo_de_la_foto(&r, "p1", &mensajes[0], (4200.0, 100.0)).expect("lo adoptado se ve");
        let (x0, y0, x1, y1) = d.trazos.unwrap();
        assert!(
            (x0 - 1000.0).abs() < 60.0 && (x1 - 3000.0).abs() < 60.0,
            "{x0} {x1}"
        );
        assert!(y0 > 0.0 && y1 < 100.0 + 40.0, "{y0} {y1}");
        let _ = std::fs::remove_dir_all(&r);
    }

    /// **Lo dibujado en el PC antes de K11, ya en el lienzo de la foto**: una
    /// foto de 4400x3200 (en el lienzo, 2200x1600) con una elipse roja que
    /// rodea el cuarto verde y una raya azul del centro abajo a la derecha,
    /// dibujadas en el `.pixpin2d`, adoptadas y pintadas desde el lienzo.
    /// Tienen que caer en el mismo sitio de la foto que antes.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_de_lo_adoptado_en_su_burbuja() {
        let r = raiz("muestra-adoptado");
        let (w, h) = (4400u32, 3200u32);
        let mut mensajes = vec![foto_del_pc(&r, "m20", (8, 8))];
        let foto = r.join("proyectos/p1/archivos/m20.png");
        let png = pixpin_codec::imagen::codificar_png(&foto_de_cuartos(w, h)).unwrap();
        std::fs::write(&foto, png).unwrap();
        let mut escena = Escena::nueva();
        escena.anadir(Elemento {
            figura: Figura::Elipse,
            x: 2400.0,
            y: 200.0,
            ancho: 1800.0,
            alto: 1200.0,
            trazo: pixpin_motor2d::ColorRgba {
                r: 0.9,
                g: 0.1,
                b: 0.1,
                a: 1.0,
            },
            grosor: 30.0,
            ..Elemento::default()
        });
        escena.anadir(Elemento {
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(2200.0, 1600.0), Punto2::nuevo(4000.0, 3000.0)],
            },
            x: 2200.0,
            y: 1600.0,
            ancho: 1800.0,
            alto: 1400.0,
            trazo: pixpin_motor2d::ColorRgba {
                r: 0.1,
                g: 0.2,
                b: 0.9,
                a: 1.0,
            },
            grosor: 40.0,
            ..Elemento::default()
        });
        pixpin_motor2d::guardar(
            &pixpin_proyecto::lienzo_de_la_foto::pixpin2d_de(&foto),
            &escena,
        )
        .unwrap();
        assert!(asegurar_lienzo(&r, "p1", &mut mensajes, 0));
        let d = dibujo_de_la_foto(&r, "p1", &mensajes[0], (w as f32, h as f32)).unwrap();
        let escala = 0.2;
        a_png(
            "foto-adoptada-en-burbuja",
            (880, 640),
            vec![(1, foto_de_cuartos(w, h))],
            |p, imagenes| {
                p.limpiar(pixpin_render::Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                });
                p.poner_vista((0.0, 0.0), escala, (0.0, 0.0));
                let vista = (0.0, 0.0, w as f32, h as f32);
                let fondo = Orden::Imagen {
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
                    p, &fondo, vista, None, imagenes, escala, None,
                );
                for o in &d.ordenes {
                    crate::dibujo::pintar::dibujar_orden(p, o, vista, None, imagenes, escala, None);
                }
            },
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn sin_foto_legible_no_se_estrena_lienzo_y_se_sigue_como_antes() {
        let r = raiz("ilegible");
        // Caso negativo: un mensaje sin ruta.
        let mut sin_ruta = vec![foto("m12", None)];
        sin_ruta[0].ruta = None;
        assert!(!asegurar_lienzo(&r, "p1", &mut sin_ruta, 0));
        // Caso negativo: una «foto» que no es una imagen.
        let mut mensajes = vec![foto_del_pc(&r, "m13", (4, 4))];
        std::fs::write(r.join("proyectos/p1/archivos/m13.png"), b"no soy una foto").unwrap();
        assert!(!asegurar_lienzo(&r, "p1", &mut mensajes, 0));
        assert!(lienzo_de_la_foto(&r, "p1", &mensajes[0]).is_none());
        assert_eq!(mensajes[0].referencia, None);
        assert_eq!(cuaderno(&r)[0].referencia, None);
        // Caso negativo: un indice que no esta.
        assert!(!asegurar_lienzo(&r, "p1", &mut mensajes, 7));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn sin_foto_en_el_lienzo_lo_dibujado_se_queda_en_sus_unidades() {
        // Caso raro: el lienzo no trae la foto (se borro alli). No se inventa
        // una escala: lo dibujado va tal cual.
        let r = raiz("sinfoto");
        let mut v: serde_json::Value = serde_json::from_str(LIENZO_DE_FOTO).unwrap();
        v["elements"].as_array_mut().unwrap().remove(0);
        poner(&r, "foto-m5", &v.to_string());
        let d = dibujo_de_la_foto(&r, "p1", &foto("m5", None), (3200.0, 2400.0)).unwrap();
        let (x0, _, x1, _) = d.trazos.unwrap();
        assert!(
            (x0 - 400.0).abs() < 15.0 && (x1 - 900.0).abs() < 15.0,
            "{x0} {x1}"
        );
        let _ = std::fs::remove_dir_all(&r);
    }
}
