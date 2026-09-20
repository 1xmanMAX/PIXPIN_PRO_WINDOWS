//! «Copias de seguridad», dentro de Sincronizar.
//!
//! Copia de `sincro/CopiasActivity.kt` del movil, con sus dos textos de
//! cabecera y su dialogo de «¿Volver a esta copia?». Es la respuesta a la
//! pregunta del usuario tras perder lienzos de «Tesis»: si un envio o una
//! sincronizacion pisa algo, ¿como lo recupero?
//!
//! Las copias se hacen solas antes de recibir algo por Wi-Fi y antes de cada
//! vuelta de sincronizacion (ver `pixpin_sincro::copias`). Aqui se ven, con
//! su fecha y por que se hizo cada una, y se vuelve a cualquiera. Volver no
//! borra lo hecho despues y se puede deshacer, porque antes de volver se
//! guarda otra copia.
//!
//! Lo que NO se copia del movil es la lista de «lienzos sin proyecto»: en el
//! PC un lienzo vive dentro de la carpeta de su proyecto y no en un cajon
//! comun, asi que no hay lienzos sueltos que recoger.

use std::path::PathBuf;

use pixpin_sincro::copias::Copia;
use pixpin_store::Catalogo;

use super::{Accion, BORDE, ERROR, HISTORIAL, Lienzo, SUAVE, TEXTO, VELO, VERDE, caja, rect};

/// Lo que se pulsa en esta pantalla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Toque {
    /// «Volver» de una fila, por su sitio en la lista: abre el dialogo.
    Volver(usize),
    Confirmar,
    Cancelar,
    Cerrar,
}

/// Como acabo una vuelta atras, para decirlo donde estaba la lista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Hecho {
    Bien(String),
    Mal(String),
}

pub(super) struct Copias {
    raiz: PathBuf,
    lista: Vec<Copia>,
    /// La que se va a restaurar, mientras se pregunta.
    preguntando: Option<usize>,
    hecho: Option<Hecho>,
}

impl Copias {
    pub(super) fn nuevo(raiz: PathBuf) -> Copias {
        let lista = leer(&raiz);
        Copias {
            raiz,
            lista,
            preguntando: None,
            hecho: None,
        }
    }

    /// La copia por la que se pregunta ahora mismo.
    fn en_pregunta(&self) -> Option<&Copia> {
        self.lista.get(self.preguntando?)
    }
}

fn leer(raiz: &std::path::Path) -> Vec<Copia> {
    pixpin_sincro::copias::todas(&pixpin_proyecto::vista::DiscoPc::nuevo(raiz))
}

/// Un toque. Devuelve `true` si hay que volver a la portada.
pub(super) fn tocar(c: &mut Copias, t: Toque) -> bool {
    match t {
        Toque::Volver(i) => {
            c.hecho = None;
            c.preguntando = Some(i);
        }
        Toque::Cancelar => c.preguntando = None,
        Toque::Confirmar => {
            let Some(copia) = c.en_pregunta().cloned() else {
                return false;
            };
            c.preguntando = None;
            // En la ventana y no en un hilo: son unos pocos ficheros de la
            // carpeta de un proyecto, lo mismo que abrirlo.
            let disco = pixpin_proyecto::vista::DiscoPc::nuevo(&c.raiz);
            let ahora = pixpin_shell::entorno::ahora_utc_ms();
            c.hecho = Some(
                match pixpin_sincro::copias::restaurar(&disco, &copia, ahora) {
                    Ok(()) => Hecho::Bien(copia.nombre.clone()),
                    Err(e) => {
                        tracing::warn!(?e, copia = %copia.id, "no se pudo volver a la copia");
                        Hecho::Mal(e.to_string())
                    }
                },
            );
            // La lista cambia: la vuelta atras dejo antes otra copia.
            c.lista = leer(&c.raiz);
        }
        Toque::Cerrar => return true,
    }
    false
}

// ------------------------------------------------------------------ pintar

fn con(textos: &Catalogo, clave: &str, pares: &[(&str, String)]) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    for (k, v) in pares {
        args.set(*k, v.clone());
    }
    textos.t_args(clave, &args)
}

/// «13/09/26 · 16:44». La hora se guarda en UTC y se pinta en la local, como
/// en el resto de PixPin.
pub(super) fn cuando(utc_ms: i64) -> String {
    if utc_ms <= 0 {
        return String::new();
    }
    let local = pixpin_shell::entorno::a_local(utc_ms);
    const DIA: i64 = 86_400_000;
    let del_dia = local.rem_euclid(DIA) / 1000;
    let fecha = pixpin_ui::chat::etiqueta_fecha(local, 0);
    format!(
        "{fecha} · {:02}:{:02}",
        del_dia / 3600,
        (del_dia % 3600) / 60
    )
}

/// Lo que se lee bajo la fecha: de que proyecto es, por que se hizo y que
/// tiene dentro.
fn detalle(textos: &Catalogo, c: &Copia) -> String {
    let mut t = format!("«{}» · {}\n", c.nombre, c.motivo);
    t.push_str(&con(
        textos,
        "cop-hojas-mensajes",
        &[
            ("hojas", c.hojas.to_string()),
            ("mensajes", c.mensajes.len().to_string()),
        ],
    ));
    if !c.sin_copiar.is_empty() {
        t.push_str(&textos.t("cop-sin-grandes"));
    }
    t
}

/// Pinta la pantalla desde `y` y devuelve donde acaba.
pub(super) fn pintar(
    l: &mut Lienzo<'_>,
    textos: &Catalogo,
    c: &Copias,
    x: f32,
    w: f32,
    mut y: f32,
) -> f32 {
    let (p, e) = (l.p, l.e);
    y += 8.0 * e;
    l.titulo(&textos.t("cop-versiones"), x, &mut y);
    let cabecera = if c.lista.is_empty() {
        "cop-todavia-nada"
    } else {
        "cop-como-estaba"
    };
    y += l.parrafo(&textos.t(cabecera), x, y, w, 12.0 * e, SUAVE) + 12.0 * e;

    if let Some(h) = &c.hecho {
        let (texto, color) = match h {
            Hecho::Bien(nombre) => (
                con(textos, "cop-ha-vuelto", &[("nombre", nombre.clone())]),
                VERDE,
            ),
            Hecho::Mal(motivo) => (
                con(textos, "cop-no-se-pudo", &[("motivo", motivo.clone())]),
                ERROR,
            ),
        };
        y += l.parrafo(&texto, x, y, w, 13.0 * e, color) + 12.0 * e;
    }

    for (i, copia) in c.lista.iter().enumerate() {
        let texto = detalle(textos, copia);
        // El boton «Volver» va a la derecha; el texto ocupa lo que queda.
        let wb = 92.0 * e;
        let wt = w - 32.0 * e - wb - 10.0 * e;
        let alto = 16.0 * e + 20.0 * e + l.alto_de(&texto, 12.0 * e, wt) + 16.0 * e;
        let r = rect(x, y, w, alto.max(64.0 * e));
        caja(p, r, e);
        p.texto_linea(
            &cuando(copia.cuando),
            x + 16.0 * e,
            y + 16.0 * e,
            15.0 * e,
            wt,
            TEXTO,
        );
        l.parrafo(&texto, x + 16.0 * e, y + 38.0 * e, wt, 12.0 * e, SUAVE);
        l.boton(
            rect(
                x + w - 16.0 * e - wb,
                y + (r.alto - 40.0 * e) / 2.0,
                wb,
                40.0 * e,
            ),
            &textos.t("cop-volver"),
            None,
            false,
            Accion::Copias(Toque::Volver(i)),
        );
        y += r.alto + 10.0 * e;
    }

    y += 10.0 * e;
    p.rellenar(rect(x, y, w, e.max(1.0)), BORDE);
    y += 12.0 * e;
    y += l.parrafo(&textos.t("cop-como-se-hacen"), x, y, w, 12.0 * e, SUAVE);
    y + 10.0 * e
}

/// El dialogo de «¿Volver a esta copia?», si hay una en pregunta. Devuelve si
/// lo pinto: la ventana no pone el suyo encima.
pub(super) fn dialogo(
    l: &mut Lienzo<'_>,
    textos: &Catalogo,
    c: &Copias,
    ancho: f32,
    alto: f32,
) -> bool {
    let Some(copia) = c.en_pregunta() else {
        return false;
    };
    let (p, e) = (l.p, l.e);
    // El velo: lo de detras sigue viendose pero no se toca.
    p.rellenar(rect(0.0, 0.0, ancho, alto), VELO);
    let w = (ancho - 48.0 * e).min(380.0 * e);
    let x = (ancho - w) / 2.0;
    let mut minuscula = copia.motivo.chars();
    let motivo = match minuscula.next() {
        Some(a) => a.to_lowercase().collect::<String>() + minuscula.as_str(),
        None => String::new(),
    };
    let texto = con(
        textos,
        "cop-volver-texto",
        &[
            ("nombre", copia.nombre.clone()),
            ("cuando", cuando(copia.cuando)),
            ("motivo", motivo),
            ("hojas", copia.hojas.to_string()),
        ],
    );
    let ht = l.alto_de(&texto, 14.0 * e, w - 48.0 * e);
    let h = 24.0 * e + 26.0 * e + 12.0 * e + ht + 20.0 * e + 48.0 * e + 24.0 * e;
    let y = (alto - h) / 2.0;
    p.rellenar_redondeado(rect(x, y, w, h), 20.0 * e, super::DIALOGO);
    let xi = x + 24.0 * e;
    let wi = w - 48.0 * e;
    p.texto_linea(
        &textos.t("cop-volver-titulo"),
        xi,
        y + 24.0 * e,
        18.0 * e,
        wi,
        TEXTO,
    );
    l.parrafo(&texto, xi, y + 62.0 * e, wi, 14.0 * e, SUAVE);
    let yb = y + h - 24.0 * e - 48.0 * e;
    let medio = (wi - 10.0 * e) / 2.0;
    l.boton(
        rect(xi, yb, medio, 48.0 * e),
        &textos.t("cop-cancelar"),
        None,
        false,
        Accion::Copias(Toque::Cancelar),
    );
    l.boton(
        rect(xi + medio + 10.0 * e, yb, medio, 48.0 * e),
        &textos.t("cop-volver"),
        Some(&HISTORIAL),
        true,
        Accion::Copias(Toque::Confirmar),
    );
    true
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn copia(id: &str, cuando: i64, motivo: &str) -> Copia {
        Copia {
            id: id.into(),
            chat: "p1".into(),
            cuando,
            motivo: motivo.into(),
            nombre: "Tesis".into(),
            hojas: 3,
            ..Default::default()
        }
    }

    #[test]
    fn preguntar_no_restaura_nada_hasta_que_se_confirma() {
        let mut c = Copias::nuevo(std::env::temp_dir().join("pixpin-copias-vacia"));
        c.lista = vec![copia("1", 1000, "Antes de sincronizar con Tableta")];
        assert!(!tocar(&mut c, Toque::Volver(0)));
        assert!(c.en_pregunta().is_some(), "se pregunta antes de tocar nada");
        // Caso negativo: cancelar deja todo como estaba y no apunta nada.
        assert!(!tocar(&mut c, Toque::Cancelar));
        assert!(c.en_pregunta().is_none());
        assert!(c.hecho.is_none(), "cancelar no es haber restaurado");
    }

    #[test]
    fn confirmar_sin_copia_en_pregunta_no_hace_nada() {
        // Caso negativo: un toque que llegue tarde (la lista se recargo) no
        // puede restaurar una copia cualquiera.
        let mut c = Copias::nuevo(std::env::temp_dir().join("pixpin-copias-vacia"));
        assert!(!tocar(&mut c, Toque::Confirmar));
        assert!(c.hecho.is_none());
    }

    #[test]
    fn la_hora_se_pinta_con_fecha_y_minutos_y_la_que_no_hay_no_se_inventa() {
        assert!(cuando(0).is_empty(), "sin hora no se pinta una falsa");
        let t = cuando(1_757_939_357_123);
        assert!(t.contains('·') && t.contains(':'), "{t}");
    }
}
