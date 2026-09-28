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
//!
//! Arriba del todo, **«Proyectos borrados»** (H6, v0.79 del movil): lo que
//! esta en la papelera se recupera entero —nombre, hojas en su orden, PDF y
//! chat— y deja de estar borrado para sincronizar. El movil lo saca de sus
//! copias; el PC tiene algo mejor, la carpeta entera en la papelera
//! (`pixpin_proyecto::almacen::en_papelera`), y es lo que devuelve. Se llega
//! sin tener ningun proyecto abierto: Sincronizar cuelga de la bandeja.

use std::path::PathBuf;

use pixpin_proyecto::almacen::EnPapelera;
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
    /// «Recuperar» de un proyecto borrado, por su sitio en la papelera. Sin
    /// dialogo, como en el movil: recuperar no pisa ni borra nada.
    Recuperar(usize),
    RecuperarTodos,
}

/// Como acabo una vuelta atras o una recuperacion, para decirlo donde
/// estaba la lista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Hecho {
    Bien(String),
    Mal(String),
    Recuperado(String),
    NoRecuperado { nombre: String, motivo: String },
    Recuperados { n: usize, total: usize },
}

pub(super) struct Copias {
    raiz: PathBuf,
    lista: Vec<Copia>,
    /// Lo que hay en la papelera, el borrado mas reciente primero.
    borrados: Vec<EnPapelera>,
    /// La que se va a restaurar, mientras se pregunta.
    preguntando: Option<usize>,
    hecho: Option<Hecho>,
}

impl Copias {
    pub(super) fn nuevo(raiz: PathBuf) -> Copias {
        let lista = leer(&raiz);
        let borrados = pixpin_proyecto::almacen::en_papelera(&raiz);
        Copias {
            raiz,
            lista,
            borrados,
            preguntando: None,
            hecho: None,
        }
    }

    /// Recupera uno de la papelera y dice como fue.
    fn recuperar(&self, en: &EnPapelera) -> Hecho {
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        match pixpin_proyecto::almacen::recuperar(&self.raiz, en, ahora) {
            Ok(f) => Hecho::Recuperado(f.nombre),
            Err(e) => {
                tracing::warn!(?e, carpeta = %en.carpeta.display(), "no se pudo recuperar el proyecto");
                Hecho::NoRecuperado {
                    nombre: en.ficha.nombre.clone(),
                    motivo: e.to_string(),
                }
            }
        }
    }

    /// Tras recuperar: la papelera y las copias se releen, y el chat se
    /// entera de que su lista cambio por fuera.
    fn releer(&mut self) {
        self.borrados = pixpin_proyecto::almacen::en_papelera(&self.raiz);
        self.lista = leer(&self.raiz);
        crate::ventana_chat::refrescar();
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
        Toque::Recuperar(i) => {
            c.preguntando = None;
            // Un toque que llegue tarde (la lista se releyo) no recupera
            // uno cualquiera: sin fila en ese sitio, nada.
            let Some(en) = c.borrados.get(i).cloned() else {
                return false;
            };
            c.hecho = Some(c.recuperar(&en));
            c.releer();
        }
        Toque::RecuperarTodos => {
            c.preguntando = None;
            let todos = c.borrados.clone();
            let bien = todos
                .iter()
                .filter(|en| matches!(c.recuperar(en), Hecho::Recuperado(_)))
                .count();
            c.hecho = Some(Hecho::Recuperados {
                n: bien,
                total: todos.len(),
            });
            c.releer();
        }
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

    // Lo que ha pasado va arriba del todo: tras recuperar, la fila se va de
    // la lista y el aviso tiene que verse donde se pulso.
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
            Hecho::Recuperado(nombre) => (
                con(textos, "papelera-ha-vuelto", &[("nombre", nombre.clone())]),
                VERDE,
            ),
            Hecho::NoRecuperado { nombre, motivo } => (
                con(
                    textos,
                    "papelera-no-se-pudo",
                    &[("nombre", nombre.clone()), ("motivo", motivo.clone())],
                ),
                ERROR,
            ),
            Hecho::Recuperados { n, total } => (
                con(
                    textos,
                    "papelera-recuperados",
                    &[("n", n.to_string()), ("total", total.to_string())],
                ),
                if n == total { VERDE } else { ERROR },
            ),
        };
        y += l.parrafo(&texto, x, y, w, 13.0 * e, color) + 12.0 * e;
    }

    if !c.borrados.is_empty() {
        y = pintar_borrados(l, textos, c, x, w, y);
    }

    l.titulo(&textos.t("cop-versiones"), x, &mut y);
    let cabecera = if c.lista.is_empty() {
        "cop-todavia-nada"
    } else {
        "cop-como-estaba"
    };
    y += l.parrafo(&textos.t(cabecera), x, y, w, 12.0 * e, SUAVE) + 12.0 * e;

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

/// «Proyectos borrados»: una caja por proyecto con «Recuperar», y
/// «Recuperar todos» si hay mas de uno (`CopiasActivity.kt`, v0.79).
fn pintar_borrados(
    l: &mut Lienzo<'_>,
    textos: &Catalogo,
    c: &Copias,
    x: f32,
    w: f32,
    mut y: f32,
) -> f32 {
    let (p, e) = (l.p, l.e);
    l.titulo(&textos.t("papelera-titulo"), x, &mut y);
    y += l.parrafo(&textos.t("papelera-como"), x, y, w, 12.0 * e, SUAVE) + 12.0 * e;
    for (i, en) in c.borrados.iter().enumerate() {
        let detalle = con(
            textos,
            "papelera-detalle",
            &[
                ("hojas", en.ficha.hojas.to_string()),
                ("mensajes", en.mensajes.to_string()),
                ("cuando", cuando(en.borrado)),
            ],
        );
        let wb = 112.0 * e;
        let wt = w - 32.0 * e - wb - 10.0 * e;
        let alto =
            (16.0 * e + 22.0 * e + l.alto_de(&detalle, 12.0 * e, wt) + 14.0 * e).max(64.0 * e);
        let r = rect(x, y, w, alto);
        caja(p, r, e);
        p.texto_linea(
            &format!("«{}»", en.ficha.nombre),
            x + 16.0 * e,
            y + 14.0 * e,
            15.0 * e,
            wt,
            TEXTO,
        );
        l.parrafo(&detalle, x + 16.0 * e, y + 38.0 * e, wt, 12.0 * e, SUAVE);
        l.boton(
            rect(
                x + w - 16.0 * e - wb,
                y + (alto - 40.0 * e) / 2.0,
                wb,
                40.0 * e,
            ),
            &textos.t("papelera-recuperar"),
            None,
            true,
            Accion::Copias(Toque::Recuperar(i)),
        );
        y += alto + 10.0 * e;
    }
    if c.borrados.len() > 1 {
        l.boton(
            rect(x, y, w, 44.0 * e),
            &textos.t("papelera-recuperar-todos"),
            None,
            false,
            Accion::Copias(Toque::RecuperarTodos),
        );
        y += 44.0 * e + 10.0 * e;
    }
    y += 10.0 * e;
    p.rellenar(rect(x, y, w, e.max(1.0)), BORDE);
    y + 16.0 * e
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

    fn raiz_con_un_borrado(etiqueta: &str) -> PathBuf {
        use pixpin_proyecto::almacen::{self, Ficha, Indice};
        let raiz = std::env::temp_dir().join(format!(
            "pixpin-papelera-ui-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&raiz);
        let mut i = Indice::default();
        i.proyectos.push(Ficha {
            id: "p1".into(),
            nombre: "Tesis".into(),
            tocado: 10,
            ..Default::default()
        });
        i.guardar(&raiz).unwrap();
        std::fs::create_dir_all(almacen::carpeta(&raiz, "p1")).unwrap();
        std::fs::write(almacen::carpeta(&raiz, "p1").join("guardados.jsonl"), "").unwrap();
        almacen::borrar_proyectos(&raiz, &["p1".to_string()], 99).unwrap();
        raiz
    }

    #[test]
    fn recuperar_de_la_papelera_lo_devuelve_a_la_lista_y_lo_quita_de_la_papelera() {
        let raiz = raiz_con_un_borrado("recuperar");
        let mut c = Copias::nuevo(raiz.clone());
        assert_eq!(c.borrados.len(), 1, "la papelera se lee al entrar");
        assert!(
            !tocar(&mut c, Toque::Recuperar(0)),
            "se queda en la pantalla"
        );
        assert_eq!(c.hecho, Some(Hecho::Recuperado("Tesis".into())));
        assert!(c.borrados.is_empty(), "ya no esta en la papelera");
        assert!(
            pixpin_proyecto::almacen::Indice::leer(&raiz)
                .buscar("p1")
                .is_some()
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_toque_de_recuperar_fuera_de_la_lista_no_recupera_nada() {
        // Caso negativo: la lista se releyo y el sitio ya no existe.
        let raiz = raiz_con_un_borrado("fuera");
        let mut c = Copias::nuevo(raiz.clone());
        assert!(!tocar(&mut c, Toque::Recuperar(7)));
        assert!(c.hecho.is_none());
        assert_eq!(c.borrados.len(), 1, "sigue en la papelera");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_hora_se_pinta_con_fecha_y_minutos_y_la_que_no_hay_no_se_inventa() {
        assert!(cuando(0).is_empty(), "sin hora no se pinta una falsa");
        let t = cuando(1_757_939_357_123);
        assert!(t.contains('·') && t.contains(':'), "{t}");
    }
}
