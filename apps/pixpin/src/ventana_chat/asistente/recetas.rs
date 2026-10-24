//! **Las recetas del asistente en el chat** (encargo 3, 13-oct-2026).
//!
//! Los pedidos que el usuario repite, como botones encima de la caja de
//! escribir del chat «Asistente»: un clic lo manda como si se hubiera
//! escrito. Se crean desde una respuesta (o desde la propia pregunta) con
//! «Guardar como receta» en el menu del mensaje, y con el clic derecho en un
//! boton se cambia su nombre, su pedido o su icono, se mueve o se borra.
//!
//! Una receta es una **automatizacion `a_mano` con `boton: true`**
//! (`crate::asistente::recetas`): vive en el `automatizaciones.json` del chat
//! «Asistente» y viaja con el, solo entre los aparatos del usuario. Aqui solo
//! la fila de botones y su edicion: cambiar el nombre o el pedido se hace
//! **en la propia caja de escribir** (con la barra de encima diciendo que se
//! esta editando), Entrar guarda y Escape deja la caja como estaba.

use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use crate::asistente::recetas::{self as rc, NoSePuede, Receta};
use crate::automatizaciones as auto;

use super::*;
// `asistente` tiene su propio `Accion` (los botones de la tarjeta de bajar).
use super::super::Accion as AccionChat;

/// Que se hace con una receta desde su menu.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::ventana_chat) enum QueReceta {
    Renombrar,
    Editar,
    /// Abre la lista de iconos.
    Iconos,
    Icono(String),
    Izquierda,
    Derecha,
    Borrar,
}

/// Que campo se esta editando en la caja.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Campo {
    Nombre,
    Pedido,
}

struct Edicion {
    proyecto: String,
    id: String,
    campo: Campo,
    /// Lo que habia en la caja antes de editar: vuelve al acabar.
    antes: String,
}

struct Cache {
    raiz: PathBuf,
    cambiado: Option<SystemTime>,
    mirado: Instant,
    visibles: Vec<Receta>,
}

thread_local! {
    static CACHE: RefCell<Option<Cache>> = const { RefCell::new(None) };
    static EDITANDO: RefCell<Option<Edicion>> = const { RefCell::new(None) };
    /// Los botones como se pintaron: donde y que receta.
    static BOTONES: RefCell<Vec<(Rect, String)>> = const { RefCell::new(Vec::new()) };
}

fn ahora() -> i64 {
    pixpin_shell::entorno::ahora_utc_ms()
}

/// Las recetas del chat abierto (el «Asistente»), leidas del disco como
/// mucho una vez por segundo (y solo si el fichero cambio: lo trae la
/// sincronizacion o Ajustes).
fn visibles(a: &Abierto) -> Vec<Receta> {
    let carpeta = pixpin_proyecto::almacen::carpeta(&a.raiz, &a.ficha.id);
    CACHE.with_borrow_mut(|c| {
        if let Some(c) = c.as_ref()
            && c.raiz == carpeta
            && c.mirado.elapsed() < Duration::from_secs(1)
        {
            return c.visibles.clone();
        }
        let cambiado = std::fs::metadata(auto::fichero_en(&carpeta))
            .and_then(|m| m.modified())
            .ok();
        if let Some(viejo) = c.as_mut()
            && viejo.raiz == carpeta
            && viejo.cambiado == cambiado
        {
            viejo.mirado = Instant::now();
            return viejo.visibles.clone();
        }
        let v = match auto::leer_de(&carpeta) {
            Ok(l) => rc::visibles(&l),
            Err(e) => {
                tracing::warn!(%e, "recetas: las automatizaciones no se entienden");
                Vec::new()
            }
        };
        *c = Some(Cache {
            raiz: carpeta,
            cambiado,
            mirado: Instant::now(),
            visibles: v.clone(),
        });
        v
    })
}

/// Cambia las recetas en el fichero de las automatizaciones (leido de
/// nuevo, con su cerrojo: Ajustes o la sincronizacion pudieron tocarlo).
fn escribir<T>(
    raiz: &std::path::Path,
    aparato: &str,
    f: impl FnOnce(&mut auto::Lista) -> Result<T, NoSePuede>,
) -> Result<T, NoSePuede> {
    let mut motivo = None;
    let r = auto::cambiar(raiz, aparato, ahora(), |l| {
        f(l).map_err(|e| {
            motivo = Some(e.clone());
            format!("{e:?}")
        })
    });
    CACHE.with_borrow_mut(|c| *c = None);
    r.map_err(|e| {
        motivo.unwrap_or_else(|| {
            tracing::warn!(%e, "recetas: no se pudieron guardar");
            NoSePuede::NoEsta
        })
    })
}

fn editando_en(a: &Abierto) -> Option<(String, Campo)> {
    EDITANDO.with_borrow(|e| {
        e.as_ref()
            .filter(|e| e.proyecto == a.ficha.id)
            .map(|e| (e.id.clone(), e.campo))
    })
}

/// Si se esta editando una receta en la caja de este chat.
pub(in crate::ventana_chat) fn editando(a: &Abierto) -> bool {
    editando_en(a).is_some()
}

/// **Si va la fila de recetas** encima de la caja: en el chat «Asistente»
/// con alguna receta, o editando una.
pub(in crate::ventana_chat) fn hay_barra(a: &Abierto) -> bool {
    a.ficha.es_asistente() && (editando(a) || !visibles(a).is_empty())
}

/// Al empezar a pintar la caja: los botones de antes ya no valen (otra
/// barra puede ocupar el sitio este fotograma).
pub(in crate::ventana_chat) fn olvidar_botones() {
    BOTONES.with_borrow_mut(Vec::clear);
}

/// **Pinta la fila** (o, editando, la barra que lo dice).
pub(in crate::ventana_chat) fn pintar(p: &Pintor, barra: Rect, c: &Pinta, a: &Abierto) {
    let (tema, textos, e) = (c.tema, c.textos, c.escala as f32 / 100.0);
    let caja = RectF {
        alto: (barra.alto as f32 - 6.0 * e).max(1.0),
        ..rf(barra)
    };
    if let Some((id, campo)) = editando_en(a) {
        p.rellenar_redondeado(caja, 16.0 * e, tema.campo);
        let icono = 18.0 * e;
        p.icono(
            &mi::EDIT,
            RectF {
                x: caja.x + 12.0 * e,
                y: caja.y + (caja.alto - icono) / 2.0,
                ancho: icono,
                alto: icono,
            },
            tema.enviar,
        );
        let nombre = visibles(a)
            .into_iter()
            .find(|r| r.id == id)
            .map(|r| r.nombre)
            .unwrap_or_default();
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("nombre", nombre);
        let clave = match campo {
            Campo::Nombre => "asistente-receta-editando-nombre",
            Campo::Pedido => "asistente-receta-editando-pedido",
        };
        let rotulo = textos.t_args(clave, &args);
        let (_, h) = p.medir_texto(&rotulo, 12.0 * e);
        let x = caja.x + 40.0 * e;
        p.texto_linea(
            &rotulo,
            x,
            caja.y + (caja.alto - h) / 2.0,
            12.0 * e,
            caja.ancho - 52.0 * e,
            tema.texto,
        );
        return;
    }
    let tam = 13.0 * e;
    let alto = caja.alto.min(30.0 * e);
    let y = caja.y + (caja.alto - alto) / 2.0;
    let mut x = caja.x;
    let fin = caja.x + caja.ancho;
    BOTONES.with_borrow_mut(|b| {
        b.clear();
        for r in visibles(a) {
            let rotulo = if r.icono.is_empty() {
                r.nombre.clone()
            } else {
                format!("{} {}", r.icono, r.nombre)
            };
            let (w, h) = p.medir_texto(&rotulo, tam);
            let ancho = (w + 24.0 * e).min(220.0 * e);
            if x + ancho > fin {
                break;
            }
            let pastilla = RectF { x, y, ancho, alto };
            // Pastilla con su borde redondeado: el borde entero y el fondo encima.
            let filete = (1.0 * e).max(1.0);
            p.rellenar_redondeado(pastilla, alto / 2.0, con_alfa(tema.enviar, 0.55));
            p.rellenar_redondeado(encoger(pastilla, filete), alto / 2.0 - filete, tema.isla);
            p.texto_linea(
                &rotulo,
                x + 12.0 * e,
                y + (alto - h) / 2.0,
                tam,
                ancho - 24.0 * e,
                tema.texto,
            );
            b.push((
                Rect {
                    x: pastilla.x as i32,
                    y: pastilla.y as i32,
                    ancho: pastilla.ancho as u32,
                    alto: pastilla.alto as u32,
                },
                r.id.clone(),
            ));
            x += ancho + 6.0 * e;
        }
    });
}

fn boton_en(l: Punto) -> Option<String> {
    BOTONES.with_borrow(|b| {
        b.iter()
            .find(|(r, _)| r.contiene(l))
            .map(|(_, id)| id.clone())
    })
}

/// **Un clic en un boton**: el pedido que hay que mandar.
pub(in crate::ventana_chat) fn pedido_en(a: &Abierto, l: Punto) -> Option<String> {
    if !a.ficha.es_asistente() || editando(a) {
        return None;
    }
    let id = boton_en(l)?;
    visibles(a)
        .into_iter()
        .find(|r| r.id == id)
        .map(|r| r.pedido)
}

/// **Clic derecho en un boton**: su menu.
pub(in crate::ventana_chat) fn menu_en(
    a: &Abierto,
    l: Punto,
    textos: &Catalogo,
) -> Option<Vec<EntradaMenu>> {
    if !a.ficha.es_asistente() || editando(a) {
        return None;
    }
    let id = boton_en(l)?;
    let fila = visibles(a);
    let i = fila.iter().position(|r| r.id == id)?;
    let hacer = |q: QueReceta| AccionChat::Receta(id.clone(), q);
    let mut v = vec![
        entrada(
            Some(&mi::EDIT),
            textos.t("asistente-receta-renombrar"),
            hacer(QueReceta::Renombrar),
        ),
        entrada(
            Some(&mi::SUBTITLES),
            textos.t("asistente-receta-editar"),
            hacer(QueReceta::Editar),
        ),
        entrada(
            Some(&mi::PALETTE),
            textos.t("asistente-receta-icono"),
            hacer(QueReceta::Iconos),
        ),
    ];
    if i > 0 {
        v.push(
            entrada(
                Some(&mi::ARROW_BACK),
                textos.t("asistente-receta-izquierda"),
                hacer(QueReceta::Izquierda),
            )
            .separada(),
        );
    }
    if i + 1 < fila.len() {
        let e = entrada(
            Some(&mi::ARROW_FORWARD),
            textos.t("asistente-receta-derecha"),
            hacer(QueReceta::Derecha),
        );
        v.push(if i == 0 { e.separada() } else { e });
    }
    let mut borrar = entrada(
        Some(&mi::DELETE),
        textos.t("asistente-receta-borrar"),
        hacer(QueReceta::Borrar),
    )
    .separada();
    borrar.peligro = true;
    v.push(borrar);
    Some(v)
}

/// **Lo que se pide desde un mensaje**: el de una pregunta del usuario, su
/// texto; el de una respuesta del asistente, la pregunta que contesta (la
/// que cuelga, o la ultima nota del usuario de antes). Sin la mencion
/// `@asistente`.
pub(in crate::ventana_chat) fn pedido_de(a: &Abierto, i: usize) -> Option<String> {
    use crate::asistente::chat as ia_chat;
    let m = a.mensajes.get(i)?;
    let es_nota = |m: &cuaderno::Mensaje| matches!(m.clase, None | Some(cuaderno::Clase::Nota));
    let pregunta = if ia_chat::es_del_asistente(m) {
        let colgada = m
            .responde_a
            .as_deref()
            .and_then(|r| a.mensajes.iter().find(|x| x.id == r));
        colgada.or_else(|| {
            a.mensajes[..i]
                .iter()
                .rev()
                .find(|x| !ia_chat::es_del_asistente(x) && es_nota(x))
        })?
    } else if es_nota(m) {
        m
    } else {
        return None;
    };
    let texto =
        ia_chat::mencion(&pregunta.texto).unwrap_or_else(|| pregunta.texto.trim().to_string());
    (!texto.trim().is_empty()).then_some(texto)
}

/// Si el menu del mensaje `i` lleva «Guardar como receta»: en una
/// conversacion con el asistente y con algo que pedir.
pub(in crate::ventana_chat) fn se_puede_guardar(a: &Abierto, i: usize) -> bool {
    super::conversacion_con_ia(a) && pedido_de(a, i).is_some()
}

/// **«Guardar como receta»**: nace al final de la fila con el nombre y el
/// icono de serie; se cambian con el clic derecho.
pub(in crate::ventana_chat) fn guardar_desde(
    ubicacion: &Ubicacion,
    aparato: &str,
    a: &Abierto,
    i: usize,
    textos: &Catalogo,
) -> Efecto {
    let Some(pedido) = pedido_de(a, i) else {
        return Efecto::Nada;
    };
    match escribir(ubicacion.raiz(), aparato, |v| {
        rc::anadir(v, "", "", &pedido, ahora())
    }) {
        Ok(r) => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("nombre", format!("{} {}", r.icono, r.nombre));
            Efecto::Aviso(textos.t_args("asistente-receta-guardada", &args))
        }
        Err(NoSePuede::Llenas) => Efecto::Aviso(textos.t("asistente-receta-llenas")),
        Err(_) => Efecto::Aviso(textos.t("chat-no-se-pudo")),
    }
}

/// **Lo del menu de un boton.**
pub(in crate::ventana_chat) fn cambiar(
    ubicacion: &Ubicacion,
    aparato: &str,
    a: &mut Abierto,
    id: &str,
    que: QueReceta,
    textos: &Catalogo,
) -> Efecto {
    let raiz = ubicacion.raiz();
    let r = match que {
        QueReceta::Renombrar | QueReceta::Editar => {
            let Some(rec) = visibles(a).into_iter().find(|r| r.id == id) else {
                return Efecto::Nada;
            };
            let (campo, valor) = if que == QueReceta::Renombrar {
                (Campo::Nombre, rec.nombre)
            } else {
                (Campo::Pedido, rec.pedido)
            };
            empezar_a_editar(a, id, campo, valor);
            return Efecto::Nada;
        }
        QueReceta::Iconos => {
            let v = rc::ICONOS
                .iter()
                .map(|ic| {
                    entrada(
                        None,
                        (*ic).to_string(),
                        AccionChat::Receta(id.to_string(), QueReceta::Icono((*ic).to_string())),
                    )
                })
                .collect();
            return Efecto::Menu(v);
        }
        QueReceta::Icono(ic) => escribir(raiz, aparato, |v| {
            rc::cambiar(v, id, ahora(), |r| r.icono = ic)
        }),
        QueReceta::Izquierda => escribir(raiz, aparato, |v| rc::mover(v, id, -1, ahora())),
        QueReceta::Derecha => escribir(raiz, aparato, |v| rc::mover(v, id, 1, ahora())),
        QueReceta::Borrar => escribir(raiz, aparato, |v| rc::borrar(v, id)),
    };
    match r {
        Ok(()) => Efecto::Nada,
        Err(_) => Efecto::Aviso(textos.t("chat-no-se-pudo")),
    }
}

fn empezar_a_editar(a: &mut Abierto, id: &str, campo: Campo, valor: String) {
    let antes = std::mem::replace(&mut a.borrador, valor);
    let n = a.borrador.len();
    a.cursor.poner(&a.borrador, n, false);
    a.colocado.borrow_mut().ancho = 0;
    EDITANDO.with_borrow_mut(|e| {
        *e = Some(Edicion {
            proyecto: a.ficha.id.clone(),
            id: id.to_string(),
            campo,
            antes,
        })
    });
}

/// **Entrar o Escape mientras se edita** una receta en la caja. `true` si
/// era suya. Entrar guarda lo escrito (vacio no vale: se queda editando);
/// Escape no guarda. Las dos devuelven la caja como estaba.
pub(in crate::ventana_chat) fn tecla(
    ubicacion: &Ubicacion,
    aparato: &str,
    a: &mut Abierto,
    vk: u32,
) -> bool {
    let Some((id, campo)) = editando_en(a) else {
        return false;
    };
    match vk {
        VK_ENTRAR => {
            let escrito = a.borrador.trim().to_string();
            if escrito.is_empty() {
                return true;
            }
            let r = escribir(ubicacion.raiz(), aparato, |v| {
                rc::cambiar(v, &id, ahora(), |r| match campo {
                    Campo::Nombre => r.nombre = escrito.clone(),
                    Campo::Pedido => r.pedido = escrito.clone(),
                })
            });
            if let Err(e) = r {
                tracing::warn!(?e, "recetas: no se pudo cambiar");
            }
        }
        VK_ESCAPE => {}
        _ => return false,
    }
    terminar_de_editar(a);
    true
}

fn terminar_de_editar(a: &mut Abierto) {
    if let Some(e) = EDITANDO.with_borrow_mut(Option::take) {
        a.borrador = e.antes;
        let n = a.borrador.len();
        a.cursor.poner(&a.borrador, n, false);
        a.colocado.borrow_mut().ancho = 0;
    }
}

#[cfg(test)]
pub(in crate::ventana_chat) fn olvidar_para_pruebas() {
    CACHE.with_borrow_mut(|c| *c = None);
    EDITANDO.with_borrow_mut(|e| *e = None);
    BOTONES.with_borrow_mut(Vec::clear);
}
