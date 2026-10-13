//! **El asislet modelo = TARJETA.with_borrow_mut(|t| elegido(t, &a.raiz));ente local en la ventana del chat** (encargo 1): lo que engancha
//! `crate::asistente` con la conversacion abierta.
//!
//! El chat solo llama a cinco cosas, en sitios contados:
//!
//! - [`al_enviar`], justo despues de guardar una nota: si era para el
//!   asistente (el chat «Asistente», o `@asistente` en otro), nace su burbuja
//!   y el turno se lanza en su hilo.
//! - [`latido`], una vez por vuelta del bucle: recoge lo que haya escrito el
//!   modelo (la burbuja crece en vivo), lo que haya que meter en un lienzo
//!   y la descarga.
//! - [`espera`]: cada cuanto volver a mirar mientras hay algo en marcha.
//! - [`ocupado`]/[`parar`]: el boton de enviar es «parar» mientras contesta.
//! - [`pintar_tarjeta`]/[`tocar`]: la tarjeta de bajar el modelo, la
//!   primera vez, en el chat «Asistente».
//!
//! Todo vive en el hilo de la ventana (`thread_local`): el bucle del chat es
//! uno y no hay que compartir nada con otros hilos salvo por los canales.

use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pixpin_ia::motor::Motor;
use pixpin_proyecto::cuaderno;

use super::*;
use crate::asistente::chat::{self as ia_chat, EnVivo};
use crate::asistente::descarga::{self as ia_descarga, AvisoDescarga, Descarga};
use crate::asistente::turno::{Aviso, Turno};
use crate::asistente::{al_lienzo, motor as ia_motor, turno as ia_turno};

/// Cada cuanto se escribe en el disco lo que va contestando (en memoria se
/// ve al momento). Un turno de minuto y medio no son cien escrituras.
const GUARDAR_CADA: Duration = Duration::from_secs(2);

/// Un turno en marcha y su burbuja.
struct Sesion {
    proyecto: String,
    /// El id de la nota de la respuesta.
    nota: String,
    turno: Turno,
    vivo: EnVivo,
    guardado: Instant,
}

/// La tarjeta de la primera vez.
#[derive(Default)]
struct Tarjeta {
    /// Lo elegido en la tarjeta (`None`: lo de los ajustes).
    ligero: Option<bool>,
    /// Se pulso «Bajar» y se espera el «Si».
    confirmando: bool,
    /// La ultima noticia (cancelada, fallo).
    noticia: Option<String>,
    /// Lo pulsable, como se pinto.
    botones: Vec<(Rect, Accion)>,
    /// Si el modelo esta listo, mirado hace poco (mirar el disco en cada
    /// fotograma no).
    listo: Option<(Instant, &'static str, bool)>,
    /// El modelo de los ajustes, leido hace poco (el TOML no se lee en cada
    /// fotograma).
    ajuste: Option<(Instant, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Accion {
    Elegir { ligero: bool },
    Bajar,
    Confirmar,
    Cancelar,
}

thread_local! {
    static SESIONES: RefCell<Vec<Sesion>> = const { RefCell::new(Vec::new()) };
    static DESCARGA: RefCell<Option<Descarga>> = const { RefCell::new(None) };
    static TARJETA: RefCell<Tarjeta> = RefCell::new(Tarjeta::default());
}

/// Si el asistente esta contestando en este chat.
pub(super) fn ocupado(proyecto: &str) -> bool {
    SESIONES.with_borrow(|s| s.iter().any(|x| x.proyecto == proyecto && !x.turno.acabado()))
}

/// «Parar»: corta en el siguiente trozo.
pub(super) fn parar(proyecto: &str) {
    SESIONES.with_borrow(|s| {
        s.iter()
            .filter(|x| x.proyecto == proyecto)
            .for_each(|x| x.turno.parar())
    });
}

/// Cada cuanto volver a mirar (ms) mientras hay algo en marcha.
pub(super) fn espera() -> Option<u32> {
    let turnos = SESIONES.with_borrow(|s| !s.is_empty());
    let bajando = DESCARGA.with_borrow(|d| d.is_some());
    (turnos || al_lienzo::hay_pedidos()).then_some(80).or(bajando.then_some(250))
}

fn modelo_de_los_ajustes(ubicacion: &Ubicacion) -> pixpin_ia::modelos::Modelo {
    let aj = pixpin_store::cargar(ubicacion).unwrap_or_default();
    ia_motor::modelo_elegido(Some(aj.asistente.modelo.as_str()).filter(|m| !m.is_empty()))
}

/// **Despues de guardar una nota**: si era para el asistente, lanza el
/// turno. Devuelve un aviso para el usuario si no se pudo.
pub(super) fn al_enviar(ubicacion: &Ubicacion, a: &mut Abierto, textos: &Catalogo) -> Option<String> {
    let nueva = a.mensajes.last()?;
    let pregunta = if a.ficha.es_asistente() {
        nueva.texto.trim().to_string()
    } else {
        ia_chat::mencion(&nueva.texto)?
    };
    if ocupado(&a.ficha.id) {
        return Some(textos.t("asistente-ocupado"));
    }
    let Some(raiz) = ia_motor::raiz() else {
        return Some(textos.t("asistente-sin-modelo"));
    };
    let modelo = modelo_de_los_ajustes(ubicacion);
    let motor = match ia_motor::del_proceso(&raiz, &modelo) {
        Ok(m) => m,
        Err(_) => return Some(textos.t("asistente-sin-modelo")),
    };
    match lanzar(ubicacion, a, motor, pregunta) {
        Ok(()) => None,
        Err(e) => {
            tracing::warn!(?e, "asistente: no se pudo empezar a contestar");
            Some(textos.t("asistente-fallo"))
        }
    }
}

/// Lanza el turno con este motor (las pruebas le pasan uno de mentira).
pub(super) fn lanzar(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    motor: Arc<dyn Motor>,
    pregunta: String,
) -> std::io::Result<()> {
    let raiz = ubicacion.raiz().to_path_buf();
    let proyecto = a.ficha.id.clone();
    let n = a.mensajes.len();
    let (anteriores, nueva) = a.mensajes.split_at(n.saturating_sub(1));
    let pregunta_id = nueva.first().map(|m| m.id.clone());
    let conversacion = ia_chat::conversacion(anteriores);
    let foto = ia_chat::foto_reciente(anteriores).and_then(|m| ruta_del_mensaje(&raiz, &proyecto, m));
    let adjuntos = ia_chat::adjuntos(&a.mensajes, |m| ruta_del_mensaje(&raiz, &proyecto, m).filter(|r| r.is_file()));
    let historial = ia_turno::historial_para_el_modelo(&conversacion, &pregunta, None);

    // La burbuja de la respuesta nace ya, con el cursor: el usuario ve que
    // se le esta contestando aunque el modelo tarde en cargar.
    let mut vivo = EnVivo::default();
    vivo.estado = Some("💭 …".into());
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    // En otro chat la respuesta cuelga de la pregunta, para que se sepa a
    // que contesta entre los mensajes de los demas.
    let responde_a = if a.ficha.es_asistente() { None } else { pregunta_id };
    let nota = escribir_nota(
        &raiz,
        &proyecto,
        ia_chat::APARATO,
        &vivo.mostrar(),
        descripcion::hora_sin_repetir(),
        numero,
        responde_a,
    )?;
    let id = nota.id.clone();
    a.vistas.push(None);
    a.mensajes.push(nota);
    a.colocado.borrow_mut().ancho = 0;

    let turno = Turno::lanzar(motor, proyecto.clone(), historial, adjuntos, foto);
    SESIONES.with_borrow_mut(|s| {
        s.push(Sesion {
            proyecto,
            nota: id,
            turno,
            vivo,
            guardado: Instant::now(),
        })
    });
    Ok(())
}

/// **Una vez por vuelta del bucle.** Devuelve si hay que repintar y un
/// aviso, si lo hay.
pub(super) fn latido(ubicacion: &Ubicacion, mut abierto: Option<&mut Abierto>) -> (bool, Option<String>) {
    let mut cambio = false;
    let mut aviso = None;

    // 1. Lo que hay que meter en un lienzo, si no hay ninguno abierto (si lo
    //    hay, lo atiende su bucle).
    if !al_lienzo::hay_editor() {
        for p in al_lienzo::tomar() {
            let r = meter_en_el_lienzo_del_chat(ubicacion, abierto.as_deref_mut(), &p.proyecto, &p.documento);
            cambio = true;
            p.responder(r);
        }
    }

    // 2. Los turnos en marcha.
    let raiz = ubicacion.raiz().to_path_buf();
    let mut leidos: Vec<(String, ia_turno::Adjunto, String)> = Vec::new();
    SESIONES.with_borrow_mut(|sesiones| {
        for s in sesiones.iter_mut() {
            let avisos = s.turno.recibir();
            if avisos.is_empty() {
                continue;
            }
            let mut acabo = false;
            for av in avisos {
                match av {
                    Aviso::Evento(e) => s.vivo.aplicar(&e),
                    Aviso::TextoLeido { de, texto } => leidos.push((s.proyecto.clone(), de, texto)),
                    Aviso::Fin { resultado } => {
                        s.vivo.acabar(&resultado);
                        acabo = true;
                    }
                }
            }
            let texto = s.vivo.mostrar();
            if let Some(a) = abierto.as_deref_mut().filter(|a| a.ficha.id == s.proyecto)
                && let Some(m) = a.mensajes.iter_mut().find(|m| m.id == s.nota)
                && m.texto != texto
            {
                m.texto = texto.clone();
                a.colocado.borrow_mut().ancho = 0;
                cambio = true;
            }
            if acabo || s.guardado.elapsed() >= GUARDAR_CADA {
                s.guardado = Instant::now();
                let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, &s.proyecto);
                if let Err(e) = cuaderno::cambiar(&carpeta, &s.nota, |m| {
                    m.texto = texto.clone();
                    true
                }) {
                    tracing::warn!(?e, "asistente: no se pudo guardar la respuesta");
                }
                if acabo {
                    let resumen: String = texto.chars().take(120).collect();
                    let _ = subir_en_la_lista(&raiz, &s.proyecto, pixpin_shell::entorno::ahora_utc_ms(), &resumen);
                }
            }
        }
        sesiones.retain(|s| !s.vivo.acabado);
    });

    // 3. El texto entero de un OCR: al chat, como nota y como .txt.
    for (proyecto, de, texto) in leidos {
        match dejar_texto_leido(&raiz, abierto.as_deref_mut(), &proyecto, &de, &texto) {
            Ok(()) => cambio = true,
            Err(e) => tracing::warn!(?e, "asistente: no se pudo dejar el texto leido"),
        }
    }

    // 4. La descarga.
    let fin = DESCARGA.with_borrow_mut(|d| {
        let des = d.as_mut()?;
        if des.mirar() {
            cambio = true;
        }
        let fin = des.acabada.clone()?;
        let modelo = des.modelo;
        *d = None;
        Some((fin, modelo))
    });
    if let Some((fin, modelo)) = fin {
        TARJETA.with_borrow_mut(|t| {
            t.listo = None;
            t.confirmando = false;
            t.noticia = match &fin {
                AvisoDescarga::Cancelada => Some("asistente-cancelada".into()),
                AvisoDescarga::Fallo(m) => Some(m.clone()),
                _ => None,
            };
        });
        if fin == AvisoDescarga::Lista
            && let Some(raiz_m) = ia_motor::raiz()
            && let Ok(m) = ia_motor::del_proceso(&raiz_m, &modelo)
        {
            // Que la primera pregunta no espere los 20 s de la carga.
            ia_motor::precargar(m);
            aviso = Some("asistente-listo".to_string());
        }
        cambio = true;
    }
    (cambio, aviso)
}

/// El OCR entero en el chat: una nota con el texto (se lee y se copia ahi
/// mismo) y el mismo texto como `.txt` adjunto (se guarda o se envia como
/// cualquier fichero). Un PDF con capa de texto no se puede escribir con lo
/// que hay en el repo (ver `asistente::ocr`).
fn dejar_texto_leido(
    raiz: &std::path::Path,
    mut abierto: Option<&mut Abierto>,
    proyecto: &str,
    de: &ia_turno::Adjunto,
    texto: &str,
) -> std::io::Result<()> {
    let base = std::path::Path::new(&de.nombre)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "texto".into());
    let numero = siguiente_numero_en(raiz, proyecto)?;
    let nota = escribir_nota(
        raiz,
        proyecto,
        ia_chat::APARATO,
        &format!("📄 Texto de «{}»:\n\n{}", de.nombre, texto),
        descripcion::hora_sin_repetir(),
        numero,
        None,
    )?;
    let fichero = adjuntar_con_pie(
        raiz,
        proyecto,
        ia_chat::APARATO,
        &format!("{base} (texto).txt"),
        texto.as_bytes(),
        numero + 1,
        "",
        None,
    )?;
    if let Some(a) = abierto.as_deref_mut().filter(|a| a.ficha.id == proyecto) {
        for m in [nota, fichero] {
            a.vistas.push(None);
            a.mensajes.push(m);
        }
        a.colocado.borrow_mut().ancho = 0;
    }
    Ok(())
}

/// **Lo dibujado, al ultimo lienzo del chat** (o a uno nuevo), sin abrirlo:
/// leerlo, pegar en un paso y guardarlo por el camino de siempre.
fn meter_en_el_lienzo_del_chat(
    ubicacion: &Ubicacion,
    mut abierto: Option<&mut Abierto>,
    proyecto: &str,
    documento: &str,
) -> Result<String, String> {
    let raiz = ubicacion.raiz();
    let es_el_abierto = abierto.as_deref().is_some_and(|a| a.ficha.id == proyecto);
    let mensajes: Vec<cuaderno::Mensaje> = match abierto.as_deref() {
        Some(a) if es_el_abierto => a.mensajes.clone(),
        _ => cuaderno::Cuaderno::leer_de(&pixpin_proyecto::almacen::carpeta(raiz, proyecto))
            .map(|c| c.mensajes)
            .unwrap_or_default(),
    };
    let (indice, mensaje, nuevo) = match ia_chat::ultimo_lienzo(&mensajes) {
        Some(i) => (Some(i), mensajes[i].clone(), false),
        None => {
            let numero = mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
            let m = escribir_lienzo(raiz, proyecto, ia_chat::APARATO, numero, Some("Asistente"))
                .map_err(|e| format!("no se pudo crear un lienzo: {e}"))?;
            (None, m, true)
        }
    };
    let hoja = preparar_hoja(raiz, proyecto, &mensaje, pixpin_shell::entorno::ahora_utc_ms())
        .ok_or("no se pudo abrir el lienzo")?;
    let mut escena = hoja.escena;
    let n = al_lienzo::meter_al_lado(&mut escena, documento)?;
    if !guardar_hoja_dibujada(&hoja.ruta, &hoja.lienzo, &escena) {
        return Err("no se pudo guardar el lienzo".into());
    }
    if let Some(a) = abierto.as_deref_mut().filter(|_| es_el_abierto) {
        let vista = leer_vista(ubicacion, proyecto, &mensaje);
        match indice {
            Some(i) if i < a.vistas.len() => a.vistas[i] = vista,
            _ => {
                a.vistas.push(vista);
                a.mensajes.push(mensaje.clone());
            }
        }
        a.colocado.borrow_mut().ancho = 0;
    }
    let nombre = if mensaje.nombre.is_empty() { "Asistente".to_string() } else { mensaje.nombre.clone() };
    Ok(format!(
        "{} «{nombre}»{}",
        al_lienzo::resumen(n),
        if nuevo { " (lienzo nuevo en este chat)" } else { "" }
    ))
}

// ---------------------------------------------------------------------------
// La tarjeta de la primera vez

/// Si la tarjeta se ensena: en el chat «Asistente» mientras falte el
/// modelo elegido (o se este bajando).
fn hace_falta_tarjeta(a: &Abierto) -> bool {
    if !a.ficha.es_asistente() {
        return false;
    }
    if DESCARGA.with_borrow(|d| d.is_some()) {
        return true;
    }
    let Some(raiz) = ia_motor::raiz() else { return false };
    TARJETA.with_borrow_mut(|t| {
        let modelo = elegido(t, &a.raiz);
        match t.listo {
            Some((cuando, id, listo)) if id == modelo.id && cuando.elapsed() < Duration::from_secs(5) => !listo,
            _ => {
                let listo = ia_descarga::listo(&raiz, &modelo);
                t.listo = Some((Instant::now(), modelo.id, listo));
                !listo
            }
        }
    })
}

/// El modelo de la tarjeta: lo elegido en ella o, si no, lo de los ajustes;
/// sin nada, el ligero si el equipo tiene poca memoria.
fn elegido(t: &mut Tarjeta, raiz_datos: &std::path::Path) -> pixpin_ia::modelos::Modelo {
    use pixpin_ia::modelos::{QWEN35_08B, QWEN35_2B};
    match t.ligero {
        Some(true) => QWEN35_08B,
        Some(false) => QWEN35_2B,
        None => {
            let fresco = t.ajuste.as_ref().is_some_and(|(c, _)| c.elapsed() < Duration::from_secs(5));
            if !fresco {
                let u = Ubicacion::Instalado { raiz: raiz_datos.to_path_buf() };
                let aj = pixpin_store::cargar(&u).unwrap_or_default();
                t.ajuste = Some((Instant::now(), aj.asistente.modelo));
            }
            let id = t.ajuste.as_ref().map(|(_, m)| m.as_str()).unwrap_or_default();
            if !id.is_empty() {
                ia_motor::modelo_elegido(Some(id))
            } else if poca_memoria() {
                QWEN35_08B
            } else {
                QWEN35_2B
            }
        }
    }
}

fn poca_memoria() -> bool {
    let ram = pixpin_shell::hechos::recolectar().ram_fisica_bytes;
    ram > 0 && ram / (1024 * 1024) < ia_motor::RAM_PARA_EL_2B_MB
}

/// **Pinta la tarjeta** encima del historial del chat «Asistente».
pub(super) fn pintar_tarjeta(p: &Pintor, d: &Disposicion, c: &Pinta, a: &Abierto) {
    if !hace_falta_tarjeta(a) {
        TARJETA.with_borrow_mut(|t| t.botones.clear());
        return;
    }
    let (tema, textos) = (c.tema, c.textos);
    let e = c.escala as f32 / 100.0;
    let area = area_del_historial(d, a, c.escala);
    let ancho = (460.0 * e).min(area.ancho as f32 - 32.0 * e).max(120.0);
    let x = area.x as f32 + (area.ancho as f32 - ancho) / 2.0;
    let y0 = area.y as f32 + 16.0 * e;
    let pad = 16.0 * e;
    let dentro = ancho - 2.0 * pad;
    let raiz_m = ia_motor::raiz();
    let bajando = DESCARGA.with_borrow(|d| d.as_ref().map(|d| (d.fraccion(), d.ultimo.clone(), d.modelo)));

    TARJETA.with_borrow_mut(|t| {
        t.botones.clear();
        let modelo = elegido(t, &a.raiz);
        let explica = textos.t("asistente-explica");
        let (_, alto_explica) = p.medir_texto_ajustado(&explica, 13.0 * e, dentro);
        let fila = 48.0 * e;
        let alto = pad + 22.0 * e + alto_explica + 12.0 * e + 2.0 * (fila + 8.0 * e) + 30.0 * e + 36.0 * e + pad;
        let caja = RectF { x, y: y0, ancho, alto };
        p.rellenar_redondeado(caja, 16.0 * e, tema.burbuja_otra);
        p.trazar(caja, (1.0 * e).max(1.0), tema.isla_borde);

        let mut y = y0 + pad;
        p.texto(&format!("🤖 {}", textos.t("asistente-titulo")), x + pad, y, 16.0 * e, tema.texto);
        y += 22.0 * e;
        p.texto_ajustado(&explica, x + pad, y, 13.0 * e, dentro, tema.apagado);
        y += alto_explica + 12.0 * e;

        // Los dos modelos, para elegir.
        for (ligero, m) in [(false, pixpin_ia::modelos::QWEN35_2B), (true, pixpin_ia::modelos::QWEN35_08B)] {
            let r = RectF { x: x + pad, y, ancho: dentro, alto: fila };
            let es = m.id == modelo.id;
            p.rellenar_redondeado(r, 10.0 * e, if es { con_alfa(tema.enviar, 0.18) } else { tema.campo });
            if es {
                p.trazar(r, (1.5 * e).max(1.0), tema.enviar);
            }
            let centro = (r.x + 16.0 * e, r.y + fila / 2.0);
            p.anillo(centro, 7.0 * e, (1.5 * e).max(1.0), tema.enviar);
            if es {
                p.circulo(centro, 4.0 * e, tema.enviar);
            }
            let falta = raiz_m.as_deref().map_or(m.bytes(), |rz| ia_descarga::falta(rz, &m));
            let clave = if ligero { "asistente-modelo-ligero" } else { "asistente-modelo-normal" };
            p.texto_linea(
                &format!("{} · {}", textos.t(clave), ia_descarga::tamano(falta)),
                r.x + 32.0 * e,
                r.y + 7.0 * e,
                14.0 * e,
                dentro - 40.0 * e,
                tema.texto,
            );
            let detalle = if ligero { "asistente-detalle-ligero" } else { "asistente-detalle-normal" };
            p.texto_linea(&textos.t(detalle), r.x + 32.0 * e, r.y + 27.0 * e, 12.0 * e, dentro - 40.0 * e, tema.apagado);
            if bajando.is_none() {
                t.botones.push((a_rect(r), Accion::Elegir { ligero }));
            }
            y += fila + 8.0 * e;
        }

        // La linea de estado: progreso, la pregunta de confirmar, o la
        // ultima noticia.
        let falta = raiz_m.as_deref().map_or(modelo.bytes(), |rz| ia_descarga::falta(rz, &modelo));
        let estado = match &bajando {
            Some((fr, Some(pr), _)) if pr.verificando => {
                format!("{} {} ({:.0} %)", textos.t("asistente-comprobando"), pr.fichero, fr * 100.0)
            }
            Some((fr, pr, _)) => format!(
                "{} {} / {} ({:.0} %)",
                textos.t("asistente-bajando"),
                ia_descarga::tamano(pr.as_ref().map_or(0, |p| p.hecho)),
                ia_descarga::tamano(pr.as_ref().map_or(falta, |p| p.total)),
                fr * 100.0
            ),
            None if t.confirmando => format!(
                "{} {} → {}",
                textos.t("asistente-pregunta"),
                ia_descarga::tamano(falta),
                raiz_m.as_deref().map(|r| r.display().to_string()).unwrap_or_default()
            ),
            None => match &t.noticia {
                Some(n) if n.starts_with("asistente-") => textos.t(n),
                Some(n) => format!("⚠️ {n}"),
                None if poca_memoria() => textos.t("asistente-poca-memoria"),
                None => String::new(),
            },
        };
        p.texto_linea(&estado, x + pad, y, 12.0 * e, dentro, tema.apagado);
        y += 18.0 * e;
        if let Some((fr, _, _)) = &bajando {
            let barra = RectF { x: x + pad, y, ancho: dentro, alto: 6.0 * e };
            p.rellenar_redondeado(barra, 3.0 * e, tema.campo);
            p.rellenar_redondeado(RectF { ancho: dentro * fr.clamp(0.0, 1.0), ..barra }, 3.0 * e, tema.enviar);
        }
        y += 12.0 * e;

        // Los botones, a la derecha.
        let (principal, accion_p, secundario): (String, Accion, Option<(String, Accion)>) = if bajando.is_some() {
            (textos.t("asistente-cancelar"), Accion::Cancelar, None)
        } else if t.confirmando {
            (
                format!("{} ({})", textos.t("asistente-si-bajar"), ia_descarga::tamano(falta)),
                Accion::Confirmar,
                Some((textos.t("asistente-cancelar"), Accion::Cancelar)),
            )
        } else {
            (
                format!("{} ({})", textos.t("asistente-bajar"), ia_descarga::tamano(falta)),
                Accion::Bajar,
                None,
            )
        };
        let tam = 13.0 * e;
        let (w, _) = p.medir_texto(&principal, tam);
        let boton = RectF {
            x: x + ancho - pad - w - 28.0 * e,
            y,
            ancho: w + 28.0 * e,
            alto: 32.0 * e,
        };
        p.rellenar_redondeado(boton, 16.0 * e, tema.enviar);
        p.texto(&principal, boton.x + 14.0 * e, boton.y + 8.0 * e, tam, hex(0xffffff));
        t.botones.push((a_rect(boton), accion_p));
        if let Some((rotulo, acc)) = secundario {
            let (w2, _) = p.medir_texto(&rotulo, tam);
            let otro = RectF {
                x: boton.x - w2 - 36.0 * e,
                ancho: w2 + 28.0 * e,
                ..boton
            };
            p.trazar(otro, (1.0 * e).max(1.0), tema.isla_borde);
            p.texto(&rotulo, otro.x + 14.0 * e, otro.y + 8.0 * e, tam, tema.texto);
            t.botones.push((a_rect(otro), acc));
        }
    });
}

fn a_rect(r: RectF) -> Rect {
    Rect {
        x: r.x.round() as i32,
        y: r.y.round() as i32,
        ancho: r.ancho.max(0.0).round() as u32,
        alto: r.alto.max(0.0).round() as u32,
    }
}

/// **Un clic en la tarjeta.** `true` si era suyo.
pub(super) fn tocar(ubicacion: &Ubicacion, a: &Abierto, l: Punto) -> bool {
    if !a.ficha.es_asistente() {
        return false;
    }
    let Some(accion) = TARJETA.with_borrow(|t| t.botones.iter().find(|(r, _)| r.contiene(l)).map(|(_, a)| *a)) else {
        return false;
    };
    cumplir(ubicacion, a, accion);
    true
}

fn cumplir(ubicacion: &Ubicacion, a: &Abierto, accion: Accion) {
    match accion {
        Accion::Elegir { ligero } => {
            let id = if ligero { pixpin_ia::modelos::QWEN35_08B.id } else { pixpin_ia::modelos::QWEN35_2B.id };
            TARJETA.with_borrow_mut(|t| {
                t.ligero = Some(ligero);
                t.confirmando = false;
                t.noticia = None;
                t.listo = None;
                t.ajuste = None;
            });
            guardar_modelo(ubicacion, id);
        }
        Accion::Bajar => TARJETA.with_borrow_mut(|t| {
            t.confirmando = true;
            t.noticia = None;
        }),
        Accion::Confirmar => {
            let modelo = TARJETA.with_borrow_mut(|t| elegido(t, &a.raiz));
            guardar_modelo(ubicacion, modelo.id);
            if let Some(raiz) = ia_motor::raiz() {
                DESCARGA.with_borrow_mut(|d| {
                    if d.is_none() {
                        *d = Some(Descarga::lanzar(raiz, modelo));
                    }
                });
            }
            TARJETA.with_borrow_mut(|t| t.confirmando = false);
        }
        Accion::Cancelar => {
            DESCARGA.with_borrow(|d| {
                if let Some(d) = d {
                    d.cancelar();
                }
            });
            TARJETA.with_borrow_mut(|t| t.confirmando = false);
        }
    }
}

/// El modelo elegido, a los ajustes (`[asistente] modelo`). Si cambia, el
/// motor del otro se suelta.
fn guardar_modelo(ubicacion: &Ubicacion, id: &str) {
    let mut aj = pixpin_store::cargar(ubicacion).unwrap_or_default();
    if aj.asistente.modelo == id {
        return;
    }
    aj.asistente.modelo = id.to_string();
    if let Err(e) = pixpin_store::ajustes::guardar_conservando(ubicacion, &aj) {
        tracing::warn!(?e, "asistente: no se pudo guardar el modelo elegido");
    }
    ia_motor::soltar_todo();
}

#[cfg(test)]
pub(super) fn lanzar_descarga_para_pruebas(d: Descarga) {
    DESCARGA.with_borrow_mut(|x| *x = Some(d));
}

#[cfg(test)]
pub(super) fn olvidar_para_pruebas() {
    SESIONES.with_borrow_mut(Vec::clear);
    DESCARGA.with_borrow_mut(|d| *d = None);
    TARJETA.with_borrow_mut(|t| *t = Tarjeta::default());
}

#[cfg(test)]
pub(super) fn noticia_para_pruebas() -> Option<String> {
    TARJETA.with_borrow(|t| t.noticia.clone())
}

#[cfg(test)]
mod pruebas;
