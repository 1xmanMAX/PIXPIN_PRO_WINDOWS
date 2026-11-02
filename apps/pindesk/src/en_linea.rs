//! **«En línea / Sin conexión» en los visores** (1-nov-2026).
//!
//! Lo pidio el usuario: en un chat compartido por la nube, al abrir un plano
//! o un documento y anotar, arriba un boton dice «En línea» o «Sin
//! conexión». Sin conexion, lo anotado se queda en este equipo; al volver a
//! en linea se sube (solo los trazos nuevos). Si se sale del documento sin
//! conexion, lo anotado sin conexion se descarta, para no chocar con lo que
//! hicieron los demas; se pregunta antes, por si acaso.
//!
//! Para tocar lo minimo cada visor: el visor guarda un [EnLinea] de su
//! documento, lo pinta encima de lo suyo ([EnLinea::pintar]), le pasa los
//! clics antes que a lo suyo ([EnLinea::pulsar]) y lo llama al cerrar, ya
//! guardado lo suyo ([EnLinea::al_cerrar]). Fuera de un chat de la nube no
//! pinta nada ni atiende nada. Lo de dentro: `sincronizar::nube` y
//! `pindesk_sincro::nube::sin_conexion`.
//!
//! **Que se retiene** ([Clave]): de un documento adjunto (PDF, Word, hoja,
//! plano, modelo), lo anotado `anot-<uid>…` de su mensaje; de un lienzo (una
//! hoja DIBUJO o PAGINA, o el lienzo de una foto), su referencia:
//! `pins/draw/<referencia>.excalidraw.gz` y lo de su lado. El lienzo no
//! sabe de que chat es: quien lo abre lo dice con [en_lienzo] mientras dura.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use pindesk_render::icono::material;
use pindesk_render::{Color, Pintor, RectF};
use pindesk_store::Catalogo;

use crate::lector::{CRISTAL, DORADO, TEXTO, con_alfa};

/// Cada cuanto se vuelve a mirar si el documento es de un chat de la nube.
const MIRAR_CADA: Duration = Duration::from_secs(2);
const VERDE: Color = Color {
    r: 0.36,
    g: 0.78,
    b: 0.50,
    a: 1.0,
};

/// De que es lo anotado del visor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Clave {
    /// Un adjunto del chat: lo anotado `anot-<uid>…` de su mensaje.
    Documento(PathBuf),
    /// El lienzo `referencia` de la ficha `ficha`.
    Lienzo {
        raiz: PathBuf,
        ficha: String,
        referencia: String,
    },
}

/// **La referencia del lienzo de una hoja del chat** (DIBUJO o PAGINA, o la
/// foto con su lienzo, que se abre con la referencia ya puesta): la misma
/// con la que se abre su `lienzos/<referencia>.excalidraw`. Sin referencia,
/// no hay lienzo que retener.
pub(crate) fn referencia_de_la_hoja(m: &pindesk_proyecto::cuaderno::Mensaje) -> Option<&str> {
    m.referencia.as_deref().filter(|r| !r.is_empty())
}

impl Clave {
    fn sitio(&self) -> Option<crate::sincronizar::nube::Sitio> {
        match self {
            Clave::Documento(doc) => crate::sincronizar::nube::documento(doc),
            Clave::Lienzo {
                raiz,
                ficha,
                referencia,
            } => crate::sincronizar::nube::lienzo(raiz, ficha, referencia),
        }
    }
}

/// El boton de un visor.
pub(crate) struct EnLinea {
    clave: Clave,
    /// `None`: no es de un chat de la nube; `Some(true)`: sin conexion.
    visto: Cell<(Option<Instant>, Option<bool>)>,
    caja: Cell<Option<RectF>>,
}

impl EnLinea {
    pub(crate) fn de(doc: &Path) -> EnLinea {
        EnLinea::con(Clave::Documento(doc.to_path_buf()))
    }

    /// El de un lienzo de la ficha `ficha` (ver [Clave::Lienzo]).
    pub(crate) fn de_lienzo(raiz: &Path, ficha: &str, referencia: &str) -> EnLinea {
        EnLinea::con(Clave::Lienzo {
            raiz: raiz.to_path_buf(),
            ficha: ficha.to_string(),
            referencia: referencia.to_string(),
        })
    }

    pub(crate) fn con(clave: Clave) -> EnLinea {
        EnLinea {
            clave,
            visto: Cell::new((None, None)),
            caja: Cell::new(None),
        }
    }

    /// `None` fuera de la nube; si no, si esta sin conexion.
    pub(crate) fn estado(&self) -> Option<bool> {
        let (cuando, v) = self.visto.get();
        if cuando.is_some_and(|c| c.elapsed() < MIRAR_CADA) {
            return v;
        }
        let v = self
            .clave
            .sitio()
            .map(|s| crate::sincronizar::nube::sin_conexion(&s));
        self.visto.set((Some(Instant::now()), v));
        v
    }

    /// Pinta el boton arriba a la izquierda (`x`, `y` su esquina).
    pub(crate) fn pintar(&self, p: &Pintor, x: f32, y: f32, e: f32, textos: &Catalogo) {
        let Some(sin) = self.estado() else {
            self.caja.set(None);
            return;
        };
        let texto = rotulo(sin, textos);
        let tam = 13.0 * e;
        let lado = 18.0 * e;
        let (ancho, alto, h) = medidas(p, &texto, e);
        let caja = RectF { x, y, ancho, alto };
        p.rellenar_redondeado(caja, caja.alto / 2.0, con_alfa(CRISTAL, 0.94));
        let color = if sin { DORADO } else { VERDE };
        p.icono(
            if sin {
                &material::CLOUD_OFF
            } else {
                &material::CLOUD_DONE
            },
            RectF {
                x: caja.x + 10.0 * e,
                y: caja.y + (caja.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            color,
        );
        p.texto(
            &texto,
            caja.x + 16.0 * e + lado,
            caja.y + (caja.alto - h) / 2.0,
            tam,
            if sin { DORADO } else { TEXTO },
        );
        self.caja.set(Some(caja));
    }

    /// Un clic en `(x, y)` (en las coordenadas de la ventana): si cae en el
    /// boton, cambia de en linea a sin conexion o al reves. Dice si era suyo.
    pub(crate) fn pulsar(&self, x: f32, y: f32) -> bool {
        let Some(c) = self.caja.get() else {
            return false;
        };
        if x < c.x || y < c.y || x > c.x + c.ancho || y > c.y + c.alto {
            return false;
        }
        self.cambiar();
        true
    }

    /// Pasa de en linea a sin conexion o al reves (el boton de una barra
    /// que no es la nuestra: el plano y el modelo, `pindesk_cad`).
    pub(crate) fn cambiar(&self) {
        let sin = self.estado().unwrap_or(false);
        if let Some(s) = self.clave.sitio() {
            crate::sincronizar::nube::poner_sin_conexion(&s, !sin);
        }
        self.visto.set((None, None));
    }

    /// Al cerrar el visor, ya guardado lo suyo.
    pub(crate) fn al_cerrar(&self, textos: &Catalogo) {
        if self.estado() == Some(true)
            && let Some(s) = self.clave.sitio()
        {
            crate::sincronizar::nube::al_cerrar(&s, textos);
        }
    }
}

/// **El boton para la barra del plano o del modelo** (`pindesk_cad`, que
/// pinta su barra con sus medios): `None` fuera de un chat de la nube. El
/// `EnLinea` que va dentro es el mismo que se devuelve, para su `al_cerrar`
/// cuando la ventana se cierra.
pub(crate) fn para_cad(doc: &Path) -> (Rc<EnLinea>, Option<pindesk_cad::ventana::EnLinea>) {
    let n = Rc::new(EnLinea::de(doc));
    let boton = n.estado().is_some().then(|| {
        let (a, b) = (n.clone(), n.clone());
        pindesk_cad::ventana::EnLinea {
            sin_conexion: Box::new(move || a.estado() == Some(true)),
            cambiar: Box::new(move || b.cambiar()),
        }
    });
    (n, boton)
}

fn rotulo(sin: bool, textos: &Catalogo) -> String {
    textos.t(if sin {
        "nube-sin-conexion"
    } else {
        "nube-en-linea"
    })
}

/// El ancho y el alto del boton con `texto`, y el alto del texto.
fn medidas(p: &Pintor, texto: &str, e: f32) -> (f32, f32, f32) {
    let (w, h) = p.medir_texto(texto, 13.0 * e);
    let lado = 18.0 * e;
    (w + lado + 30.0 * e, h.max(lado) + 14.0 * e, h)
}

// ------------------------------------------------------------- el lienzo

thread_local! {
    /// El boton del lienzo que se esta abriendo en este hilo (el editor
    /// tiene su propio bucle y no sabe de chats).
    static DEL_LIENZO: RefCell<Option<Rc<EnLinea>>> = const { RefCell::new(None) };
}

/// **El lienzo que se abre en este hilo es `e`** mientras vive lo que
/// devuelve: el editor lo pinta y lo atiende ([del_lienzo]); quien lo abrio
/// llama a `al_cerrar` despues de guardar.
pub(crate) fn en_lienzo(e: Rc<EnLinea>) -> Guarda {
    DEL_LIENZO.with(|d| *d.borrow_mut() = Some(e));
    Guarda(())
}

/// El boton del lienzo abierto en este hilo ([en_lienzo]), si lo hay.
pub(crate) fn del_lienzo() -> Option<Rc<EnLinea>> {
    DEL_LIENZO.with(|d| d.borrow().clone())
}

pub(crate) struct Guarda(());

impl Drop for Guarda {
    fn drop(&mut self) {
        DEL_LIENZO.with(|d| *d.borrow_mut() = None);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn hoja(referencia: Option<&str>) -> pindesk_proyecto::cuaderno::Mensaje {
        let sello = pindesk_proyecto::cuaderno::Sello {
            cuando: 1_790_000_000_000,
            numero: 1,
            aparato: "Z5WA".into(),
            proyecto: String::new(),
        };
        let mut m = pindesk_proyecto::cuaderno::Mensaje::adjunto(
            pindesk_proyecto::cuaderno::Clase::Imagen,
            "foto.jpg",
            "pixpin:files/guardados/foto.jpg",
            10,
            &sello,
        );
        m.referencia = referencia.map(str::to_string);
        m
    }

    #[test]
    fn la_clave_de_un_lienzo_es_su_referencia() {
        assert_eq!(referencia_de_la_hoja(&hoja(Some("d1"))), Some("d1"));
        // Caso negativo: sin referencia (o vacia) no hay lienzo que retener.
        assert_eq!(referencia_de_la_hoja(&hoja(None)), None);
        assert_eq!(referencia_de_la_hoja(&hoja(Some(""))), None);
    }

    #[test]
    fn fuera_de_un_chat_de_la_nube_no_hay_boton() {
        // Un fichero suelto (sin almacen alrededor) y un lienzo sin chat.
        let d = EnLinea::de(Path::new("C:/no/esta/plano.dwg"));
        assert_eq!(d.estado(), None);
        assert!(!d.pulsar(1.0, 1.0));
        let l = EnLinea::de_lienzo(Path::new(""), "p1", "d1");
        assert_eq!(l.estado(), None);
    }

    #[test]
    fn el_lienzo_abierto_se_suelta_al_volver() {
        assert!(del_lienzo().is_none());
        {
            let _g = en_lienzo(Rc::new(EnLinea::de_lienzo(Path::new(""), "p1", "d1")));
            assert_eq!(
                del_lienzo().map(|e| e.clave.clone()),
                Some(Clave::Lienzo {
                    raiz: PathBuf::new(),
                    ficha: "p1".into(),
                    referencia: "d1".into()
                })
            );
        }
        assert!(del_lienzo().is_none());
    }
}
