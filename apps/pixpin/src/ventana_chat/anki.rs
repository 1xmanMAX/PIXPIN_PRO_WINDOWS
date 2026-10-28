//! **«Enviar a Anki»** del menu de un mensaje (24-oct-2026). Las tarjetas
//! salen de `pixpin_anki::tarjetas`:
//!
//! - **Una clase grabada** (nota de voz con capturas, `grabar_clase`): una
//!   tarjeta por captura, con la imagen delante y lo dicho en ese tramo
//!   detras.
//! - **Una foto**: la imagen delante y su descripcion detras.
//! - **Una nota**: sus «pregunta :: respuesta», «P: / R:» y huecos `{{c1::…}}`.
//!
//! Van al mazo `PixPin::<proyecto>` con las etiquetas `pixpin` y la del
//! proyecto. Con Anki abierto (y AnkiConnect), directas; si no, un fichero
//! para «Archivo → Importar» en `<raiz>/anki/`, y las imagenes en la carpeta
//! de medios del perfil de Anki si se encuentra.

use std::path::{Path, PathBuf};

use pixpin_anki::{Imagen, Tarjeta, tarjetas};
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_store::Catalogo;

use super::{Abierto, Efecto, grabar_clase, ruta_del_mensaje};

/// Si el mensaje da alguna tarjeta (para ofrecerlo en el menu).
pub(super) fn se_puede(a: &Abierto, m: &Mensaje) -> bool {
    match m.clase {
        Some(Clase::Voz) => !grabar_clase::capturas_de(m).is_empty(),
        Some(Clase::Imagen) => ruta_del_mensaje(&a.raiz, &a.ficha.id, m).is_some(),
        _ => !tarjetas::de_la_nota(&m.texto).is_empty(),
    }
}

/// Las tarjetas de ese mensaje y cuantas de una clase dejo fuera Jev.
fn tarjetas_de(a: &Abierto, m: &Mensaje, textos: &Catalogo) -> (Vec<Tarjeta>, usize) {
    let mut quitadas = 0;
    let leer = |c: &Mensaje| -> Option<Imagen> {
        let ruta = ruta_del_mensaje(&a.raiz, &a.ficha.id, c)?;
        let bytes = std::fs::read(&ruta).ok()?;
        let ext = ruta
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_ascii_lowercase();
        Some(Imagen {
            // Unico en la carpeta de medios de Anki y estable: mandar dos
            // veces la misma clase no llena Anki de copias.
            nombre: format!("pixpin-{}.{ext}", c.id),
            bytes,
        })
    };
    let titulo = crate::biblioteca_audio::titulo_de_audio(m);
    let mut v = match m.clase {
        Some(Clase::Voz) => {
            let trozos: Vec<(i64, String)> = crate::voz::transcripcion_de(m)
                .map(crate::voz::trozos)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|t| Some((t.ms?, crate::voz::sin_marcas(&t.texto))))
                .collect();
            let mut capturas: Vec<(i64, Imagen)> = grabar_clase::capturas_de(m)
                .into_iter()
                .filter_map(|(ms, id)| {
                    let c = a.mensajes.iter().find(|c| c.id == id)?;
                    Some((ms, leer(c)?))
                })
                .collect();
            // En el orden de `de_la_clase` (por minuto, estable).
            capturas.sort_by_key(|(ms, _)| *ms);
            let tiempos: Vec<i64> = capturas.iter().map(|(ms, _)| *ms).collect();
            let todas =
                tarjetas::de_la_clase(&titulo, &textos.t("anki-pregunta-clase"), &trozos, capturas);
            // Con Jev, solo las que valen la pena (`pixpin_typesafe::anki`);
            // sin clave o si falla, todas, como siempre.
            match valer_la_pena(a, &titulo, &trozos, &tiempos) {
                Some(quedan) => {
                    quitadas = quedan.iter().filter(|q| !**q).count();
                    tarjetas::quedarse_con(todas, &quedan)
                }
                None => todas,
            }
        }
        Some(Clase::Imagen) => leer(m)
            .map(|i| vec![tarjetas::de_la_foto(i, &m.texto, &m.nombre)])
            .unwrap_or_default(),
        _ => tarjetas::de_la_nota(&m.texto),
    };
    let etiquetas = vec!["pixpin".to_string(), tarjetas::etiqueta(&a.ficha.nombre)];
    for t in &mut v {
        t.etiquetas = etiquetas.clone();
    }
    (v, quitadas)
}

/// **Que tarjetas de la clase valen la pena**, segun Jev: lo dicho en el
/// tramo de cada captura, valorado de 0 a 3 y si dice que entra en el
/// examen; la tabla de `pixpin_typesafe::anki::elegir` decide. `None`: sin
/// Jev, o fallo (se mandan todas).
fn valer_la_pena(
    a: &Abierto,
    titulo: &str,
    trozos: &[(i64, String)],
    tiempos: &[i64],
) -> Option<Vec<bool>> {
    let u = pixpin_store::Ubicacion::Instalado {
        raiz: a.raiz.clone(),
    };
    let cliente = crate::jev::para(&u, crate::jev::Funcion::Anki)?;
    let tramos: Vec<pixpin_typesafe::anki::Tramo> = tarjetas::lo_dicho_por_captura(trozos, tiempos)
        .into_iter()
        .zip(tiempos)
        .map(|(dicho, ms)| pixpin_typesafe::anki::Tramo {
            minuto: pixpin_voz::marca_de_tiempo(*ms),
            dicho,
        })
        .collect();
    match pixpin_typesafe::anki::valorar(&cliente, titulo, &tramos, 4) {
        Ok(senales) => Some(pixpin_typesafe::anki::elegir(&senales)),
        Err(e) => {
            tracing::warn!("jev: sin valorar las tarjetas, van todas: {e}");
            None
        }
    }
}

/// El mazo de un proyecto.
fn mazo_de(nombre: &str) -> String {
    // `::` es el separador de submazos de Anki: dentro del nombre no.
    format!("PixPin::{}", nombre.trim().replace("::", ":"))
}

/// **Manda a Anki** las tarjetas del mensaje `i`.
pub(super) fn enviar(a: &Abierto, i: usize, textos: &Catalogo) -> Efecto {
    let Some(m) = a.mensajes.get(i) else {
        return Efecto::Nada;
    };
    let (tarjetas, quitadas) = tarjetas_de(a, m, textos);
    if tarjetas.is_empty() {
        return Efecto::Aviso(textos.t("anki-nada"));
    }
    // Lo que dejo fuera Jev, dicho al final del aviso.
    let jev = if quitadas > 0 {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("n", quitadas as i64);
        format!(" · {}", textos.t_args("anki-jev-quitadas", &args))
    } else {
        String::new()
    };
    let mazo = mazo_de(&a.ficha.nombre);
    let con = |clave: &str, n: usize, donde: &str| {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("n", n as i64);
        args.set("mazo", mazo.clone());
        args.set("donde", donde.to_string());
        format!("{}{jev}", textos.t_args(clave, &args))
    };
    match pixpin_anki::conectar::anadir(&mazo, &tarjetas) {
        Ok(0) => Efecto::Aviso(con("anki-ya-estaban", tarjetas.len(), "")),
        Ok(n) => {
            tracing::info!(n, %mazo, "tarjetas mandadas a Anki");
            Efecto::Aviso(con("anki-hecho", n, ""))
        }
        Err(pixpin_anki::conectar::Fallo::SinAnki) => match a_fichero(&a.raiz, &mazo, &tarjetas) {
            Ok(ruta) => Efecto::Aviso(con(
                "anki-fichero",
                tarjetas.len(),
                &ruta.display().to_string(),
            )),
            Err(e) => {
                tracing::warn!(?e, "no se pudo dejar el fichero para Anki");
                Efecto::Aviso(textos.t("chat-no-se-pudo"))
            }
        },
        Err(e) => {
            tracing::warn!(%e, "Anki no admitio las tarjetas");
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("motivo", e.to_string());
            Efecto::Aviso(textos.t_args("anki-fallo", &args))
        }
    }
}

/// Sin Anki abierto: el fichero para importar, y las imagenes en la carpeta
/// de medios del perfil de Anki usado mas tarde (si hay alguno).
fn a_fichero(raiz: &Path, mazo: &str, tarjetas: &[Tarjeta]) -> std::io::Result<PathBuf> {
    let carpeta = raiz.join("anki");
    std::fs::create_dir_all(&carpeta)?;
    let nombre: String = mazo
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let ruta = carpeta.join(format!(
        "{nombre}-{}.txt",
        pixpin_shell::entorno::ahora_utc_ms()
    ));
    std::fs::write(&ruta, pixpin_anki::fichero::texto(mazo, tarjetas))?;
    if let Some(medios) = medios_de_anki() {
        for i in tarjetas.iter().flat_map(|t| &t.imagenes) {
            let _ = std::fs::write(medios.join(&i.nombre), &i.bytes);
        }
    }
    Ok(ruta)
}

/// `%APPDATA%\Anki2\<perfil>\collection.media` del perfil tocado por ultima
/// vez, si Anki esta instalado.
fn medios_de_anki() -> Option<PathBuf> {
    let anki = PathBuf::from(std::env::var_os("APPDATA")?).join("Anki2");
    std::fs::read_dir(anki)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path().join("collection.media"))
        .filter(|p| p.is_dir())
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_mazo_cuelga_de_pixpin_sin_romper_los_submazos() {
        assert_eq!(mazo_de(" Física "), "PixPin::Física");
        assert_eq!(mazo_de("A::B"), "PixPin::A:B");
    }
}
