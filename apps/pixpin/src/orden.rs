//! **La copia de paso de la linea de ordenes** (15-oct-2026): lo que hace
//! `pixpinmax.exe` cuando se le llama con un verbo (`pixpinmax tarea "…"`),
//! con `--pedido`, o con un enlace `pixpin://…` (el protocolo registrado por
//! `herramientas/instalar.ps1`, «Enviar a» y el menu del Explorador).
//!
//! Lo puro (que verbo da que pedido, que enlace se deja pasar) vive en
//! `pixpin_shell::orden`; aqui solo lo que necesita al proceso: leer el
//! disco para cambiar el nombre de un proyecto por su id, preguntar con un
//! cuadro lo que un enlace no puede hacer solo, leer texto de una imagen,
//! escribir en la consola y mandar los pedidos a la PixPin que corre
//! (`mensajero::enviar_pedidos_o_arrancar`, que la abre si no esta).
//!
//! **Va antes que todo** en `main`: esta copia no carga Direct2D, ni la GPU,
//! ni los ajustes, ni el registro a fichero. Nace, manda y se va; lo que
//! cuesta es arrancar el proceso (medido: `docs/integraciones.md`).

use std::path::{Path, PathBuf};

use pixpin_shell::orden::{self, Llamada, Orden, Origen, Seguridad, codigo};
use pixpin_store::Catalogo;
use serde_json::Value;

/// Si la linea de ordenes es una orden, la hace y devuelve el codigo con
/// que salir. `None`: no lo es, y el arranque sigue como siempre.
pub fn ejecutar_si_toca() -> Option<i32> {
    let mut args: Vec<String> = std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
    // El menu del Explorador lanza una copia por fichero elegido: se juntan
    // en una (`pixpin_shell::juntar`) y las demas se van sin hacer nada.
    if args.first().map(String::as_str) == Some("--juntar") && args.len() >= 2 {
        args.remove(0);
        if orden::verbo(&args[0]).is_none() {
            decir_fallo(&format!("--juntar: «{}» no es un verbo (pixpinmax ayuda)", args[0]), false);
            return Some(codigo::NO_SE_ENTIENDE);
        }
        let (ficheros, resto): (Vec<String>, Vec<String>) = args[1..].iter().cloned().partition(|a| Path::new(a).is_file());
        let clave = format!("{} {}", args[0], resto.join(" "));
        let todas = pixpin_shell::juntar::juntar(&clave, &ficheros);
        let Some(todas) = todas else { return Some(codigo::ACEPTADO) };
        args = std::iter::once(args[0].clone()).chain(resto).chain(todas).collect();
    }
    let llamada = orden::de_argumentos(&args, &|p: &Path| p.is_file())?;
    Some(match llamada {
        Ok(l) => ejecutar(l),
        Err(f) => {
            let enlace = args.iter().any(|a| orden::es_enlace(a));
            decir_fallo(&f.motivo, enlace);
            f.codigo
        }
    })
}

/// `PIXPIN_ORDEN_SIN_CUADROS`: ningun cuadro (para scripts desatendidos y
/// las pruebas). Lo que se avisaria va a la salida de errores, y lo que se
/// preguntaria se contesta «no».
fn sin_cuadros() -> bool {
    std::env::var_os("PIXPIN_ORDEN_SIN_CUADROS").is_some()
}

fn avisar(titulo: &str, texto: &str) {
    if sin_cuadros() {
        pixpin_shell::consola::escribir_error(&format!("pixpinmax: {texto}\n"));
    } else {
        pixpin_shell::dialogo::avisar_al_frente(titulo, texto);
    }
}

fn confirmar(titulo: &str, texto: &str) -> bool {
    if sin_cuadros() {
        pixpin_shell::consola::escribir_error(&format!("pixpinmax: sin cuadros, no se confirma:\n{texto}\n"));
        return false;
    }
    pixpin_shell::dialogo::confirmar_al_frente(titulo, texto)
}

/// Los textos, en el idioma del sistema. Se cargan solo si hacen falta (un
/// cuadro, un aviso): la orden que sale bien no los necesita.
fn textos() -> Catalogo {
    let lengua = pixpin_store::idioma::resolver_idioma(&pixpin_shell::entorno::locale_del_sistema(), Default::default());
    Catalogo::nuevo(lengua)
}

/// Dice un fallo donde se vea: en la consola si hay una, y si no (un
/// enlace, el menu del Explorador, un Stream Deck) en un cuadro. Un enlace
/// mal escrito tambien lo dice en un cuadro: quien lo pulso no tiene otra
/// forma de saber por que no paso nada.
fn decir_fallo(motivo: &str, enlace: bool) {
    if !enlace && pixpin_shell::consola::hay_donde_escribir() {
        pixpin_shell::consola::escribir_error(&format!("pixpinmax: {motivo}\n"));
        return;
    }
    let t = textos();
    let mut a = fluent_bundle::FluentArgs::new();
    a.set("motivo", motivo.to_string());
    avisar(&t.t("orden-titulo"), &t.t_args("orden-no-se-pudo", &a));
}

fn ejecutar(l: Llamada) -> i32 {
    match l.orden {
        Orden::Ayuda => {
            pixpin_shell::consola::escribir(&orden::ayuda());
            codigo::ACEPTADO
        }
        Orden::Ocr { rutas, copiar } => ocr(&rutas, copiar),
        Orden::Preguntar { rutas, proyecto } => match preguntar(&rutas) {
            Some(pregunta) => mandar(vec![orden::pedido_al_asistente(&pregunta, &rutas, proyecto)], &l.origen, l.sin_arrancar),
            None => codigo::RECHAZADO,
        },
        Orden::Pedidos(pedidos) => mandar(pedidos, &l.origen, l.sin_arrancar),
    }
}

/// Cambia los nombres de proyecto por su id, mira la seguridad si viene
/// de un enlace y manda.
fn mandar(mut pedidos: Vec<Value>, origen: &Origen, sin_arrancar: bool) -> i32 {
    let enlace = matches!(origen, Origen::Enlace { .. });
    if pedidos.iter().any(|p| p.get("proyecto").is_some_and(Value::is_string) || p.get("que").and_then(|q| q.get("proyecto")).is_some_and(Value::is_string)) {
        if let Some(indice) = indice_de_proyectos() {
            for p in &mut pedidos {
                poner_ids(p, &indice);
            }
        }
    }
    if let Origen::Enlace { firmado } = origen {
        // Lo peor de todos los pedidos del enlace manda.
        let peor = pedidos.iter().map(|p| if *firmado { firmado_aun_asi(p) } else { orden::seguridad(p) }).max().unwrap_or(Seguridad::Directa);
        match peor {
            Seguridad::Directa => {}
            Seguridad::Prohibida(motivo) => {
                let t = textos();
                avisar(&t.t("orden-titulo"), &t.t(&format!("orden-prohibido-{motivo}")));
                return codigo::RECHAZADO;
            }
            Seguridad::Confirmar(motivo) => {
                let t = textos();
                let que: Vec<String> = pedidos.iter().map(orden::describir).collect();
                let cuerpo = format!(
                    "{}\n\n{}\n\n{}\n\n{}",
                    t.t("orden-confirmar-cabecera"),
                    que.join("\n\n"),
                    t.t(&format!("orden-motivo-{motivo}")),
                    t.t("orden-confirmar-pregunta")
                );
                if !confirmar(&t.t("orden-titulo"), &cuerpo) {
                    return codigo::RECHAZADO;
                }
            }
        }
    }
    let textos_json: Vec<String> = pedidos.iter().map(Value::to_string).collect();
    let exe = std::env::current_exe().ok();
    let inicio = std::time::Instant::now();
    let envio = pixpin_shell::mensajero::enviar_pedidos_o_arrancar(&textos_json, exe.as_deref().filter(|_| !sin_arrancar));
    let ms = inicio.elapsed().as_millis();
    use pixpin_shell::mensajero::Envio;
    let motivo = match &envio {
        Envio::Aceptado => None,
        Envio::VersionNo => Some(textos().t("orden-version")),
        Envio::NoSeEntiende => Some("PixPin no entiende el pedido (¿es mas antigua que esta orden?)".to_string()),
        Envio::SinApp(m) => {
            let mut a = fluent_bundle::FluentArgs::new();
            a.set("motivo", m.clone());
            Some(textos().t_args("orden-sin-app", &a))
        }
        Envio::SinRespuesta(_) => Some(textos().t("orden-sin-respuesta")),
    };
    match motivo {
        None => {
            // `PIXPIN_ORDEN_TIEMPO`: cuanto tardo en contestar (las medidas de
            // `docs/integraciones.md`). Sin la variable, una orden que sale
            // bien no escribe nada, como `copy`.
            if std::env::var_os("PIXPIN_ORDEN_TIEMPO").is_some() {
                pixpin_shell::consola::escribir(&format!("aceptado en {ms} ms\n"));
            }
        }
        Some(m) => {
            if enlace || !pixpin_shell::consola::hay_donde_escribir() {
                avisar(&textos().t("orden-titulo"), &m);
            } else {
                pixpin_shell::consola::escribir_error(&format!("pixpinmax: {m}\n"));
            }
        }
    }
    envio.codigo_de_salida()
}

/// Un enlace firmado lo hizo la propia app: se hace sin preguntar, salvo
/// lo que no se hace nunca por enlace con una ruta de fuera.
fn firmado_aun_asi(p: &Value) -> Seguridad {
    match orden::seguridad(p) {
        Seguridad::Prohibida(m) if m == "ruta-no-local" => Seguridad::Prohibida(m),
        _ => Seguridad::Directa,
    }
}

/// Los proyectos de la PixPin de este usuario (id y nombre), leidos del
/// disco como los lee el plugin de Flow. `None` si no hay donde mirar.
fn indice_de_proyectos() -> Option<Vec<(String, String)>> {
    let dir_exe = pixpin_shell::entorno::directorio_del_ejecutable().ok()?;
    let appdata = pixpin_shell::entorno::appdata().ok()?;
    let ubicacion = pixpin_store::rutas::resolver(&dir_exe, &appdata);
    let indice = pixpin_proyecto::almacen::Indice::leer_si_esta(ubicacion.raiz()).ok()?;
    Some(indice.proyectos.into_iter().map(|f| (f.id, f.nombre)).collect())
}

/// `--proyecto Obra` → el id de «Obra». Un id que ya existe se deja; un
/// nombre se busca sin mirar mayusculas; «Mensajes guardados» es `null`.
/// Si no se encuentra se deja tal cual: la app dira que no existe.
fn poner_ids(pedido: &mut Value, proyectos: &[(String, String)]) {
    fn uno(v: &mut Value, proyectos: &[(String, String)]) {
        let Some(t) = v.as_str().map(|s| s.trim().to_string()) else { return };
        let bajo = t.to_lowercase();
        if bajo == "guardados" || bajo == "mensajes guardados" {
            *v = Value::Null;
            return;
        }
        if proyectos.iter().any(|(id, _)| *id == t) {
            return;
        }
        if let Some((id, _)) = proyectos.iter().find(|(_, n)| n.trim().to_lowercase() == bajo) {
            *v = Value::String(id.clone());
        }
    }
    if let Some(v) = pedido.get_mut("proyecto") {
        uno(v, proyectos);
    }
    if let Some(v) = pedido.get_mut("que").and_then(|q| q.get_mut("proyecto")) {
        uno(v, proyectos);
    }
}

/// `pixpinmax ocr`: el texto de cada imagen, en orden de lectura (el mismo
/// camino que «Copiar texto» del pin, `texto_de_imagen`), a la consola y,
/// con `--copiar`, al portapapeles. Aqui mismo: no hace falta la app.
fn ocr(rutas: &[PathBuf], copiar: bool) -> i32 {
    let mut todo = Vec::new();
    for r in rutas {
        let leido = pixpin_codec::imagen::cargar(r).map_err(|e| e.to_string()).and_then(|img| crate::texto_de_imagen(&img).map_err(|e| format!("{e:#}")));
        match leido {
            Ok(t) => todo.push(t),
            Err(e) => {
                decir_fallo(&format!("{}: {e}", r.display()), false);
                return codigo::FALLO;
            }
        }
    }
    let texto = todo.join("\n\n");
    if copiar && let Err(e) = pixpin_codec::portapapeles::copiar_texto(&texto) {
        decir_fallo(&e.to_string(), false);
        return codigo::FALLO;
    }
    pixpin_shell::consola::escribir(&format!("{texto}\n"));
    if texto.trim().is_empty() { codigo::FALLO } else { codigo::ACEPTADO }
}

/// «Preguntar al asistente» del Explorador: la caja de escribir de siempre
/// (`caja_de_texto`), con el nombre de lo que se pregunta. `None` si se
/// cancela o se deja vacia.
fn preguntar(rutas: &[PathBuf]) -> Option<String> {
    if sin_cuadros() {
        return None;
    }
    let t = textos();
    let mut a = fluent_bundle::FluentArgs::new();
    a.set("cuantos", rutas.len() as i64);
    a.set("nombre", rutas.first().and_then(|r| r.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
    let caja = pixpin_shell::caja_de_texto::abrir(pixpin_shell::caja_de_texto::Pedido {
        titulo: t.t_args("orden-preguntar-titulo", &a),
        texto: String::new(),
        guardar: t.t("orden-preguntar-enviar"),
        cancelar: t.t("orden-preguntar-cancelar"),
    });
    let escrito = caja.recv().ok().flatten()?;
    let escrito = pixpin_shell::caja_de_texto::de_windows(&escrito).trim().to_string();
    (!escrito.is_empty()).then_some(escrito)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    fn proyectos() -> Vec<(String, String)> {
        vec![("pr-1".into(), "Obra Antisuyo".into()), ("pr-2".into(), "Tesis".into())]
    }

    #[test]
    fn el_nombre_del_proyecto_se_cambia_por_su_id() {
        let mut p = json!({"pixpin":1,"accion":"anadir_tarea","texto":"x","proyecto":"obra antisuyo"});
        poner_ids(&mut p, &proyectos());
        assert_eq!(p["proyecto"], json!("pr-1"));
        let mut p = json!({"pixpin":1,"accion":"abrir","que":{"tipo":"proyecto","proyecto":"Tesis"}});
        poner_ids(&mut p, &proyectos());
        assert_eq!(p["que"]["proyecto"], json!("pr-2"));
        let mut p = json!({"pixpin":1,"accion":"chat","texto":"x","proyecto":"Mensajes guardados"});
        poner_ids(&mut p, &proyectos());
        assert_eq!(p["proyecto"], Value::Null);
    }

    #[test]
    fn caso_negativo_un_proyecto_que_no_existe_se_deja_para_que_lo_diga_la_app() {
        let mut p = json!({"pixpin":1,"accion":"chat","texto":"x","proyecto":"Cocina"});
        poner_ids(&mut p, &proyectos());
        assert_eq!(p["proyecto"], json!("Cocina"));
        // Un id que ya existe no se toca aunque otro proyecto se llame asi.
        let mut p = json!({"pixpin":1,"accion":"chat","texto":"x","proyecto":"pr-2"});
        poner_ids(&mut p, &[("pr-2".into(), "x".into()), ("pr-9".into(), "pr-2".into())]);
        assert_eq!(p["proyecto"], json!("pr-2"));
    }

    #[test]
    fn un_enlace_firmado_no_pregunta_pero_una_ruta_de_red_sigue_sin_pasar() {
        assert_eq!(firmado_aun_asi(&json!({"pixpin":1,"accion":"borrar_lista","codigo":"1"})), Seguridad::Directa);
        assert!(matches!(firmado_aun_asi(&json!({"pixpin":1,"accion":"pinear","ruta":"\\\\srv\\x.png"})), Seguridad::Prohibida(_)));
    }

    #[test]
    fn cada_motivo_tiene_su_texto_en_los_dos_idiomas() {
        let motivos = ["ficheros", "asistente", "microfono", "otros-aparatos", "automatizacion", "borra", "cambia"];
        for lengua in [pixpin_store::Idioma::Espanol, pixpin_store::Idioma::Ingles] {
            let t = Catalogo::nuevo(lengua);
            for m in motivos {
                let clave = format!("orden-motivo-{m}");
                assert_ne!(t.t(&clave), clave, "{lengua:?} {clave}");
            }
            for m in ["ruta-no-local", "crear-automatizacion"] {
                let clave = format!("orden-prohibido-{m}");
                assert_ne!(t.t(&clave), clave, "{lengua:?} {clave}");
            }
        }
        // Y cada motivo que da la seguridad es uno de esos.
        for a in pixpin_shell::mensajero::ACCIONES {
            if let Seguridad::Confirmar(m) = orden::seguridad(&json!({"pixpin":1,"accion":a})) {
                assert!(motivos.contains(&m.as_str()), "{a}: {m}");
            }
        }
    }
}
