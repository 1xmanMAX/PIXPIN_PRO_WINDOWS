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

use std::cell::Cell;
use std::path::{Path, PathBuf};
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

/// El boton de un visor.
pub(crate) struct EnLinea {
    doc: PathBuf,
    /// `None`: no es de un chat de la nube; `Some(true)`: sin conexion.
    visto: Cell<(Option<Instant>, Option<bool>)>,
    caja: Cell<Option<RectF>>,
}

impl EnLinea {
    pub(crate) fn de(doc: &Path) -> EnLinea {
        EnLinea {
            doc: doc.to_path_buf(),
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
        let v = crate::sincronizar::nube::documento(&self.doc)
            .map(|_| crate::sincronizar::nube::sin_conexion(&self.doc));
        self.visto.set((Some(Instant::now()), v));
        v
    }

    /// Pinta el boton arriba a la izquierda (`x`, `y` su esquina).
    pub(crate) fn pintar(&self, p: &Pintor, x: f32, y: f32, e: f32, textos: &Catalogo) {
        let Some(sin) = self.estado() else {
            self.caja.set(None);
            return;
        };
        let texto = textos.t(if sin {
            "nube-sin-conexion"
        } else {
            "nube-en-linea"
        });
        let tam = 13.0 * e;
        let (w, h) = p.medir_texto(&texto, tam);
        let lado = 18.0 * e;
        let caja = RectF {
            x,
            y,
            ancho: w + lado + 30.0 * e,
            alto: h.max(lado) + 14.0 * e,
        };
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
        let sin = self.estado().unwrap_or(false);
        crate::sincronizar::nube::poner_sin_conexion(&self.doc, !sin);
        self.visto.set((None, None));
        true
    }

    /// Al cerrar el visor, ya guardado lo suyo.
    pub(crate) fn al_cerrar(&self, textos: &Catalogo) {
        if self.estado() == Some(true) {
            crate::sincronizar::nube::al_cerrar(&self.doc, textos);
        }
    }
}
