//! **Los archivos del chat con el icono de su tipo** (v0.98.4 del movil,
//! 30-sep-2026): en la fila de un archivo, en vez del boton redondo con el
//! icono de documento, la hoja de su color con la esquina doblada y la
//! extension escrita, como Telegram. Rojo los PDF, azul los Word, verde las
//! hojas de calculo, naranja las presentaciones, amarillo los planos...
//!
//! El color sale de `pixpin_ui::color_de_extension` (la tabla del movil) y
//! la forma de `pixpin_render::icono::archivo` (su `Canvas`); aqui solo se
//! decide a que mensajes les toca y con que nombre, y se juntan las dos.

use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_render::icono::archivo::TEXTO_OSCURO;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_ui::color_de_extension as ce;

/// **El nombre por el que se elige el icono**, solo si el mensaje es un
/// archivo (`if (m.clase == Clase.ARCHIVO)`): una foto, una nota de voz o un
/// lienzo siguen con su boton redondo. Como el movil, el nombre y, si esta
/// en blanco, la ruta (`m.nombre.ifBlank { m.ruta }`).
pub(super) fn nombre_para_el_icono(m: &Mensaje) -> Option<String> {
    if m.clase != Some(Clase::Archivo) {
        return None;
    }
    if m.nombre.trim().is_empty() {
        Some(m.ruta.clone().unwrap_or_default())
    } else {
        Some(m.nombre.clone())
    }
}

/// El color de la hoja y el de la letra de un archivo llamado `nombre`.
pub(super) fn colores(nombre: &str) -> (Color, Color) {
    let f = ce::de(nombre);
    let (r, g, b) = ce::rgb(&f);
    let letra = if f.texto_oscuro {
        TEXTO_OSCURO
    } else {
        Color::BLANCO
    };
    (Color { r, g, b, a: 1.0 }, letra)
}

/// Pinta el icono de `nombre` en `caja` (el sitio del boton redondo, 44).
pub(super) fn pintar(p: &Pintor, nombre: &str, caja: RectF) {
    let (hoja, letra) = colores(nombre);
    p.icono_de_archivo(caja, hoja, &ce::rotulo(nombre), letra);
}

// --- Los iconos en PNG, para fuera (pedido «iconos») ----------------------

/// El lado de los PNG del pedido «iconos».
pub(super) const LADO_PNG: u32 = 64;

/// Donde quedan: `<raiz>/cache/iconos-de-extension/<ext>.png`. El plugin de
/// Flow Launcher los lee de ahi para ensenar cada archivo con el mismo icono
/// que el chat.
pub(super) fn carpeta_png(raiz: &std::path::Path) -> std::path::PathBuf {
    raiz.join("cache").join("iconos-de-extension")
}

/// Una extension tal como se acepta para nombrar un fichero: en minusculas,
/// sin el punto, solo letras y cifras ASCII y ocho como mucho. Lo demas no
/// es una extension (y `..\\` en un nombre de fichero seria salirse de la
/// carpeta).
pub(super) fn extension_saneada(ext: &str) -> Option<String> {
    let e = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    (!e.is_empty() && e.len() <= 8 && e.bytes().all(|b| b.is_ascii_alphanumeric())).then_some(e)
}

/// **Pinta a PNG los iconos que falten** de `extensiones` (64 px, fondo
/// transparente) y devuelve cuantos pinto. Los que ya estan no se tocan:
/// el plugin los pide cada vez que busca y repintarlos seria trabajo tirado.
///
/// Fuera de pantalla, como las muestras (`ventanita::muestra`): una textura
/// de la GPU, leida y pasada a PNG. Bloquea mientras la GPU pinta, asi que
/// lo llama un hilo propio, nunca la interfaz.
pub(super) fn pintar_los_que_faltan(
    raiz: &std::path::Path,
    extensiones: &[String],
) -> anyhow::Result<usize> {
    use anyhow::Context;
    use pixpin_render::MotorRender;
    use pixpin_render::fuera_de_pantalla::FueraDePantalla;

    let carpeta = carpeta_png(raiz);
    let mut faltan: Vec<String> = extensiones
        .iter()
        .filter_map(|e| extension_saneada(e))
        .filter(|e| !carpeta.join(format!("{e}.png")).is_file())
        .collect();
    faltan.sort();
    faltan.dedup();
    if faltan.is_empty() {
        return Ok(0);
    }
    std::fs::create_dir_all(&carpeta)?;
    let d = pixpin_capture::Dispositivo::nuevo().context("sin GPU para los iconos")?;
    let motor = MotorRender::nuevo(d.d3d()).context("sin motor para los iconos")?;
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), LADO_PNG, LADO_PNG)
        .context("sin superficie para los iconos")?;
    let lado = LADO_PNG as f32;
    // Un pixel de aire alrededor: el borde suavizado de la hoja no se corta.
    let caja = RectF {
        x: 1.0,
        y: 1.0,
        ancho: lado - 2.0,
        alto: lado - 2.0,
    };
    for ext in &faltan {
        let nombre = format!("x.{ext}");
        motor
            .dibujar(&fuera.destino, |p| {
                p.limpiar_transparente();
                pintar(p, &nombre, caja);
            })
            .context("pintar el icono")?;
        fuera.esperar_gpu().context("esperar a la GPU")?;
        let (ancho, alto, pixeles) = fuera.leer_rgba().context("leer el icono")?;
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
            ancho,
            alto,
            pixeles,
        })
        .context("codificar el icono")?;
        // Por un temporal: el plugin puede estar leyendo esa carpeta, y un
        // PNG a medio escribir se veria como un icono roto.
        let destino = carpeta.join(format!("{ext}.png"));
        let temporal = carpeta.join(format!("{ext}.png.tmp"));
        std::fs::write(&temporal, png)?;
        std::fs::rename(&temporal, &destino)?;
    }
    Ok(faltan.len())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn mensaje(clase: Clase, nombre: &str, ruta: Option<&str>) -> Mensaje {
        Mensaje {
            id: "m".into(),
            clase: Some(clase),
            nombre: nombre.into(),
            ruta: ruta.map(str::to_string),
            ..Mensaje::default()
        }
    }

    #[test]
    fn solo_un_archivo_lleva_el_icono_de_su_tipo() {
        let pdf = mensaje(Clase::Archivo, "Tesis.pdf", Some("archivos/a.pdf"));
        assert_eq!(nombre_para_el_icono(&pdf).as_deref(), Some("Tesis.pdf"));
        // Casos negativos: lo que no es un archivo sigue con su redondo.
        for clase in [Clase::Voz, Clase::Imagen, Clase::Dibujo, Clase::Nota] {
            let m = mensaje(clase.clone(), "recado.m4a", Some("archivos/recado.m4a"));
            assert_eq!(nombre_para_el_icono(&m), None, "{clase:?}");
        }
        assert_eq!(nombre_para_el_icono(&Mensaje::default()), None);
    }

    #[test]
    fn sin_nombre_el_icono_sale_de_la_ruta_como_en_el_movil() {
        let m = mensaje(Clase::Archivo, "  ", Some("archivos/planta.dwg"));
        assert_eq!(
            nombre_para_el_icono(&m).as_deref(),
            Some("archivos/planta.dwg")
        );
        assert_eq!(ce::de("archivos/planta.dwg"), ce::PLANO);
    }

    #[test]
    fn solo_una_extension_de_verdad_nombra_un_png() {
        assert_eq!(extension_saneada(" .PDF ").as_deref(), Some("pdf"));
        assert_eq!(extension_saneada("docx").as_deref(), Some("docx"));
        // Casos negativos: lo que se saldria de la carpeta o no es extension.
        for mala in ["", ".", "..\\x", "a/b", "tar.gz", "muylargaext", "ñ", "p d"] {
            assert_eq!(extension_saneada(mala), None, "{mala:?}");
        }
    }

    /// Necesita GPU (la de cualquier equipo con Windows, o WARP).
    #[test]
    fn un_icono_se_pinta_a_png_con_su_hoja_y_no_se_repinta() {
        let raiz = std::env::temp_dir().join(format!("pixpin-iconos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let pedidas = vec!["PDF".to_string(), "pdf".into(), "../x".into()];
        assert_eq!(pintar_los_que_faltan(&raiz, &pedidas).unwrap(), 1);
        let png = carpeta_png(&raiz).join("pdf.png");
        let img = pixpin_codec::imagen::cargar(&png).unwrap();
        assert_eq!((img.ancho, img.alto), (LADO_PNG, LADO_PNG));
        let opacos = img.pixeles.chunks_exact(4).filter(|p| p[3] > 200).count();
        let vacios = img.pixeles.chunks_exact(4).filter(|p| p[3] == 0).count();
        assert!(opacos > 1000, "la hoja tiene que verse: {opacos}");
        assert!(vacios > 0, "el fondo tiene que ser transparente");
        // La hoja es roja (la de los PDF): en el centro manda el rojo.
        let centro = ((LADO_PNG / 2 + 12) * LADO_PNG + 8) as usize * 4;
        assert!(
            img.pixeles[centro] > img.pixeles[centro + 2],
            "{:?}",
            &img.pixeles[centro..centro + 4]
        );
        // La segunda vez ya esta y no se repinta.
        assert_eq!(pintar_los_que_faltan(&raiz, &pedidas).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn sobre_el_amarillo_la_letra_va_oscura_y_sobre_el_rojo_blanca() {
        assert_eq!(colores("planta.dwg").1, TEXTO_OSCURO);
        assert_eq!(colores("tesis.pdf").1, Color::BLANCO);
    }

    /// `cargo test -p pixpin --bin pixpinmax muestra_del_icono_de_tipo --
    /// --ignored --nocapture`: filas de archivo de varias extensiones sobre
    /// el chat claro y el oscuro.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_del_icono_de_tipo() {
        let nombres = [
            "Tesis final.pdf",
            "Informe.docx",
            "Cuentas.xlsx",
            "Charla.pptx",
            "Planta baja.dwg",
            "Modelo.ifc",
            "Fotos.zip",
            "pagina.html",
            "PixPin-0.98.4.apk",
            "proyecto.pixpin",
            "datos.xyz",
            "LEEME",
        ];
        for (etiqueta, tema) in [
            ("claro", &super::super::CLARO),
            ("oscuro", &super::super::OSCURO),
        ] {
            let (ancho, fila) = (360u32, 56.0f32);
            let alto = (fila * nombres.len() as f32 + 16.0) as u32;
            crate::ventanita::muestra(&format!("icono-de-tipo-{etiqueta}"), ancho, alto, |p, _| {
                p.limpiar(tema.chat);
                for (i, n) in nombres.iter().enumerate() {
                    let y = 8.0 + i as f32 * fila;
                    p.rellenar_redondeado(
                        RectF {
                            x: 8.0,
                            y,
                            ancho: 300.0,
                            alto: fila - 6.0,
                        },
                        12.0,
                        tema.burbuja_otra,
                    );
                    pintar(
                        p,
                        n,
                        RectF {
                            x: 14.0,
                            y: y + 3.0,
                            ancho: 44.0,
                            alto: 44.0,
                        },
                    );
                    p.texto(n, 70.0, y + 14.0, 14.0, tema.texto);
                }
            });
        }
    }
}
