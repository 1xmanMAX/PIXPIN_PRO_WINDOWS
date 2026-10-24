//! **Ver la pantalla**: una foto del monitor (o de una ventana) que vuelve
//! al cliente como contenido `image`, para que Claude vea lo que el usuario
//! tiene delante cuando se lo pide.
//!
//! La misma captura por GPU que la app (`pixpin-capture`: un fotograma de
//! Windows.Graphics.Capture por monitor, sin borde amarillo, y el recorte
//! en la GPU con `componer_region`), y la lista de ventanas del modo
//! «Ventana» de la captura (`pixpin_shell::ventanas_visibles`). Una ventana
//! se saca de lo que se ve en pantalla: si otra la tapa, sale tapada (como
//! la captura de ventana de la app).
//!
//! No se guarda nada: ni en la galeria ni en disco. Se reduce a
//! `ancho_maximo` (1600 por defecto) para no mandar 8 MB de PNG por una 4K.

use serde_json::{Map, Value};

use pixpin_geom::Rect;

use crate::herramientas::{Contenido, Resultado};

/// El ancho maximo por defecto (px): lo que un modelo ve bien sin pagar de
/// mas.
pub const ANCHO_MAXIMO: u32 = 1600;
/// Lo mas que se manda en PNG; si pasa, JPEG (un MB de PNG son 1,4 MB de
/// base64 en cada mensaje).
pub const TOPE_PNG: usize = 1024 * 1024;

/// Coordenadas fisicas: como pixpinmax (su manifiesto es PerMonitorV2). Sin
/// esto Windows escalaria las coordenadas de las ventanas y no casarian con
/// los pixeles de la captura.
fn dpi_fisico() {
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };
    // SAFETY: cambia el contexto DPI del propio proceso; si ya estaba puesto
    // falla y no pasa nada.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

/// **`listar_ventanas`**.
pub fn listar_ventanas(_: &Map<String, Value>) -> Resultado {
    dpi_fisico();
    let v = pixpin_shell::ventanas_visibles::ventanas_visibles();
    if v.is_empty() {
        return Ok(vec![Contenido::Texto("No se ve ninguna ventana.".into())]);
    }
    let s: Vec<String> = v
        .iter()
        .map(|v| format!("- {} ({}×{})", v.titulo, v.rect.ancho, v.rect.alto))
        .collect();
    Ok(vec![Contenido::Texto(s.join("\n"))])
}

/// La ventana cuyo titulo contiene `buscado` (sin mayusculas), la de mas
/// delante.
pub fn elegir_ventana<'a>(
    ventanas: &'a [pixpin_shell::ventanas_visibles::VentanaVisible],
    buscado: &str,
) -> Result<&'a pixpin_shell::ventanas_visibles::VentanaVisible, String> {
    let q = buscado.trim().to_lowercase();
    ventanas
        .iter()
        .find(|v| v.titulo.to_lowercase() == q)
        .or_else(|| {
            ventanas
                .iter()
                .find(|v| v.titulo.to_lowercase().contains(&q))
        })
        .ok_or_else(|| {
            let hay: Vec<&str> = ventanas
                .iter()
                .take(15)
                .map(|v| v.titulo.as_str())
                .collect();
            format!(
                "no se ve ninguna ventana que se llame «{buscado}»; se ven: {}",
                hay.join(" | ")
            )
        })
}

/// **`ver_pantalla`**.
pub fn ver_pantalla(args: &Map<String, Value>) -> Resultado {
    dpi_fisico();
    let ancho_maximo = args
        .get("ancho_maximo")
        .and_then(Value::as_u64)
        .map(|n| n.clamp(200, 8000) as u32)
        .unwrap_or(ANCHO_MAXIMO);
    let monitores = pixpin_capture::enumerar_monitores()
        .map_err(|e| format!("no se pudieron ver los monitores: {e}"))?;
    let todos = monitores.monitores();
    if todos.is_empty() {
        return Err("no hay ningun monitor".into());
    }
    let ventana = args
        .get("ventana")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let (region, que) = match ventana {
        Some(t) => {
            let vs = pixpin_shell::ventanas_visibles::ventanas_visibles();
            let v = elegir_ventana(&vs, t)?;
            let r = monitores
                .recortar_al_escritorio(v.rect)
                .ok_or_else(|| format!("la ventana «{}» esta fuera de la pantalla", v.titulo))?;
            (r, format!("la ventana «{}»", v.titulo))
        }
        None => {
            let m = match args.get("monitor").and_then(Value::as_u64) {
                Some(n) => *todos
                    .get((n as usize).wrapping_sub(1))
                    .ok_or_else(|| format!("no hay monitor {n}; hay {}", todos.len()))?,
                None => *monitores.principal().unwrap_or(&todos[0]),
            };
            (
                m.area,
                format!("el monitor {} ({}×{})", m.id + 1, m.area.ancho, m.area.alto),
            )
        }
    };
    let (datos, mime) = capturar(region, todos, ancho_maximo)?;
    let mut texto = format!("Captura de {que}.");
    if todos.len() > 1 && ventana.is_none() {
        texto.push_str(&format!(" Hay {} monitores (usa «monitor»).", todos.len()));
    }
    Ok(vec![
        Contenido::Imagen { datos, mime },
        Contenido::Texto(texto),
    ])
}

/// Captura la region (pixeles fisicos del escritorio virtual) y la da en
/// PNG (el texto se lee nitido), reducida si pasa de `ancho_maximo`; si el
/// PNG pasa de [`TOPE_PNG`] (una foto de fondo, un video), en JPEG.
/// `PIXPIN_MCP_MEDIR` dice por stderr cuanto tarda cada paso.
pub fn capturar(
    region: Rect,
    monitores: &[pixpin_geom::Monitor],
    ancho_maximo: u32,
) -> Result<(Vec<u8>, &'static str), String> {
    let medir = std::env::var_os("PIXPIN_MCP_MEDIR").is_some();
    let t0 = std::time::Instant::now();
    let paso = |que: &str| {
        if medir {
            eprintln!("ver_pantalla: {que} a los {:?}", t0.elapsed());
        }
    };
    let disp = pixpin_capture::Dispositivo::nuevo()
        .map_err(|e| format!("no se pudo usar la GPU para capturar: {e}"))?;
    paso("dispositivo");
    let fotos: Vec<pixpin_capture::Instantanea> = monitores
        .iter()
        .filter(|m| m.area.interseccion(region).is_some())
        .map(|m| pixpin_capture::capturar_monitor(&disp, m.id, m.area))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("no se pudo capturar: {e}"))?;
    paso("captura");
    let inst = pixpin_capture::componer_region(&disp, &fotos, region)
        .map_err(|e| format!("no se pudo recortar: {e}"))?;
    let mut img = pixpin_capture::a_imagen(&disp, &inst)
        .map_err(|e| format!("no se pudo leer la captura: {e}"))?;
    paso("imagen");
    if img.ancho > ancho_maximo {
        let alto = ((img.alto as u64 * ancho_maximo as u64) / img.ancho as u64).max(1) as u32;
        img = pixpin_codec::imagen::redimensionar(img, ancho_maximo, alto)
            .map_err(|e| e.to_string())?;
    }
    paso("reducida");
    let png = pixpin_codec::imagen::codificar_png(&img).map_err(|e| e.to_string())?;
    paso("png");
    if png.len() <= TOPE_PNG {
        return Ok((png, "image/png"));
    }
    let jpg = pixpin_codec::imagen::codificar_jpg(&img, 85).map_err(|e| e.to_string())?;
    paso("jpg");
    Ok((jpg, "image/jpeg"))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_shell::ventanas_visibles::VentanaVisible;

    #[test]
    fn la_ventana_se_elige_por_un_trozo_del_titulo() {
        let v = |t: &str| VentanaVisible {
            rect: Rect {
                x: 0,
                y: 0,
                ancho: 10,
                alto: 10,
            },
            titulo: t.into(),
        };
        let vs = vec![v("Plano.pdf - Lector"), v("Excel - Costos"), v("Costos")];
        assert_eq!(
            elegir_ventana(&vs, "costos").unwrap().titulo,
            "Costos",
            "la igual antes que la que la contiene"
        );
        assert_eq!(
            elegir_ventana(&vs, "plano").unwrap().titulo,
            "Plano.pdf - Lector"
        );
        let e = elegir_ventana(&vs, "Word").unwrap_err();
        assert!(e.contains("Excel - Costos"), "dice cuales hay: {e}");
    }

    /// Una captura de verdad del monitor principal: necesita escritorio y
    /// GPU. Solo mide y mira el tamano (no guarda ni ensena la imagen).
    #[test]
    #[ignore = "captura la pantalla de verdad"]
    fn captura_el_monitor_principal_y_la_reduce() {
        let t0 = std::time::Instant::now();
        let r = ver_pantalla(&Map::new()).unwrap();
        let Contenido::Imagen { datos: png, mime } = &r[0] else {
            panic!()
        };
        println!("{mime}");
        println!(
            "ver_pantalla: {} KB en {:?}",
            png.len() / 1024,
            t0.elapsed()
        );
        if *mime == "image/png" {
            assert_eq!(&png[1..4], b"PNG");
            let ancho = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
            assert!(ancho <= ANCHO_MAXIMO, "{ancho}");
        } else {
            assert_eq!(&png[..2], &[0xFF, 0xD8]);
        }
    }
}
