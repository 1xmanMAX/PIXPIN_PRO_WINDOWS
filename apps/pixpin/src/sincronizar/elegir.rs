//! «¿Qué sincronizar?», la pantalla de elegir chats antes de una vuelta.
//!
//! Copia de `Eligiendo` de `sincro/SincronizarActivity.kt`: dos pestanas, lo
//! de este aparato y lo del otro; el punto verde es lo que esta en los dos y
//! el rojo lo que solo esta en uno y se copiara al otro; debajo, cual de las
//! dos versiones es la mas reciente, sin horas. Abajo, «Todo», «Nada» y
//! «Sincronizar N».
//!
//! El hilo de la vuelta se queda esperando la respuesta por `responde`. Si la
//! pantalla se cierra sin contestar, soltarla contesta «cancelar»: un hilo
//! esperando para siempre dejaria el aparato ocupado.

use std::collections::BTreeSet;
use std::sync::mpsc;

use pixpin_render::icono::material;
use pixpin_sincro::vuelta::Fila;
use pixpin_store::Catalogo;

use super::{
    Accion, BORDE, CONTORNO, FONDO, Lienzo, PRIMARIO, ROJO, SOBRE_PRIMARIO, SUAVE, TEXTO, VERDE,
    circulo, rect,
};

/// `Checkbox` del movil: `CheckBox` y `CheckBoxOutlineBlank` de Material.
const MARCADA: &pixpin_render::icono::Icono = &material::CHECK_BOX;
const SIN_MARCAR: &pixpin_render::icono::Icono = &material::CHECK_BOX_OUTLINE_BLANK;

/// Lo que se pulsa en esta pantalla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Toque {
    /// Una fila, por su sitio en `filas`.
    Fila(usize),
    Pestana(usize),
    Todo,
    Nada,
    Sincronizar,
    Cancelar,
}

pub(super) struct Eligiendo {
    /// Con quien, por su nombre.
    pub otro: String,
    filas: Vec<Fila>,
    marcados: Vec<bool>,
    pestana: usize,
    responde: Option<mpsc::Sender<Option<BTreeSet<String>>>>,
}

impl Eligiendo {
    pub fn nuevo(
        otro: String,
        filas: Vec<Fila>,
        responde: mpsc::Sender<Option<BTreeSet<String>>>,
    ) -> Eligiendo {
        Eligiendo {
            otro,
            marcados: filas.iter().map(|f| f.marcado).collect(),
            filas,
            pestana: 0,
            responde: Some(responde),
        }
    }

    fn cuantos(&self) -> usize {
        self.marcados.iter().filter(|m| **m).count()
    }

    fn contestar(&mut self, eleccion: Option<BTreeSet<String>>) {
        if let Some(r) = self.responde.take() {
            let _ = r.send(eleccion);
        }
    }
}

impl Drop for Eligiendo {
    fn drop(&mut self) {
        self.contestar(None);
    }
}

/// Atiende un toque. Devuelve si hay que volver a la portada (se eligio o
/// se cancelo).
pub(super) fn tocar(el: &mut Eligiendo, t: Toque) -> bool {
    match t {
        Toque::Fila(i) => {
            if let Some(m) = el.marcados.get_mut(i) {
                *m = !*m;
            }
            false
        }
        Toque::Pestana(p) => {
            el.pestana = p.min(1);
            false
        }
        Toque::Todo => {
            el.marcados.iter_mut().for_each(|m| *m = true);
            false
        }
        Toque::Nada => {
            el.marcados.iter_mut().for_each(|m| *m = false);
            false
        }
        Toque::Sincronizar => {
            if el.cuantos() == 0 {
                return false;
            }
            let elegidos = el
                .filas
                .iter()
                .zip(&el.marcados)
                .filter(|(_, m)| **m)
                .map(|(f, _)| f.id.clone())
                .collect();
            el.contestar(Some(elegidos));
            true
        }
        Toque::Cancelar => {
            el.contestar(None);
            true
        }
    }
}

fn con(textos: &Catalogo, clave: &str, pares: &[(&str, String)]) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    for (k, v) in pares {
        args.set(*k, v.clone());
    }
    textos.t_args(clave, &args)
}

/// Pinta la lista desde `y` y devuelve donde acaba. `yo` es el nombre de
/// este aparato, el de la primera pestana.
pub(super) fn pintar(
    l: &mut Lienzo<'_>,
    textos: &Catalogo,
    el: &Eligiendo,
    yo: &str,
    ancho: f32,
    mut y: f32,
) -> f32 {
    let (p, e) = (l.p, l.e);
    let t = |x: Toque| Accion::Elegir(x);
    let x0 = 20.0 * e;
    let w = ancho - 40.0 * e;
    y += 6.0 * e;
    y += l.parrafo(
        &con(textos, "sinc-elegir-con", &[("otro", el.otro.clone())]),
        x0,
        y,
        w,
        12.0 * e,
        SUAVE,
    ) + 10.0 * e;
    // La leyenda: verde en los dos, rojo solo en uno.
    circulo(p, x0 + 4.0 * e, y + 8.0 * e, 8.0 * e, VERDE);
    let en_los_dos = textos.t("sinc-en-los-dos");
    p.texto(&en_los_dos, x0 + 14.0 * e, y, 12.0 * e, TEXTO);
    let (wd, _) = p.medir_texto(&en_los_dos, 12.0 * e);
    let xr = x0 + 32.0 * e + wd;
    circulo(p, xr + 4.0 * e, y + 8.0 * e, 8.0 * e, ROJO);
    p.texto(
        &textos.t("sinc-solo-en-uno"),
        xr + 14.0 * e,
        y,
        12.0 * e,
        TEXTO,
    );
    y += 28.0 * e;

    // Las dos pestanas, como `PrimaryTabRow`.
    let lados = [(yo.to_string(), true), (el.otro.clone(), false)];
    let wp = ancho / 2.0;
    for (i, (nombre, es_mio)) in lados.iter().enumerate() {
        let n = el
            .filas
            .iter()
            .filter(|f| {
                if *es_mio {
                    f.aqui.is_some()
                } else {
                    f.alli.is_some()
                }
            })
            .count();
        let r = rect(i as f32 * wp, y, wp, 52.0 * e);
        let color = if el.pestana == i { PRIMARIO } else { SUAVE };
        l.centrado(
            nombre,
            r.x + 8.0 * e,
            wp - 16.0 * e,
            y + 8.0 * e,
            14.0 * e,
            color,
        );
        let bajo = if *es_mio {
            con(textos, "sinc-este-aparato-n", &[("n", n.to_string())])
        } else {
            n.to_string()
        };
        l.centrado(
            &bajo,
            r.x + 8.0 * e,
            wp - 16.0 * e,
            y + 28.0 * e,
            11.0 * e,
            color,
        );
        if el.pestana == i {
            p.rellenar_redondeado(
                rect(r.x + wp * 0.2, y + 49.0 * e, wp * 0.6, 3.0 * e),
                1.5 * e,
                PRIMARIO,
            );
        }
        l.zonas.push((r, t(Toque::Pestana(i))));
    }
    y += 52.0 * e;
    p.rellenar(rect(0.0, y, ancho, e.max(1.0)), BORDE);
    y += 6.0 * e;

    let es_mio = el.pestana == 0;
    let otro_nombre = if es_mio { el.otro.as_str() } else { yo };
    for (i, fila) in el.filas.iter().enumerate() {
        let (suyo, del_otro) = if es_mio {
            (&fila.aqui, &fila.alli)
        } else {
            (&fila.alli, &fila.aqui)
        };
        if suyo.is_none() {
            continue;
        }
        let alto = 56.0 * e;
        let r = rect(0.0, y, ancho, alto);
        let cy = y + alto / 2.0;
        let lado = 24.0 * e;
        let marcada = el.marcados.get(i).copied().unwrap_or(false);
        p.icono(
            if marcada { MARCADA } else { SIN_MARCAR },
            rect(x0, cy - lado / 2.0, lado, lado),
            if marcada { PRIMARIO } else { SUAVE },
        );
        circulo(
            p,
            x0 + lado + 18.0 * e,
            cy,
            10.0 * e,
            if del_otro.is_some() { VERDE } else { ROJO },
        );
        let xt = x0 + lado + 34.0 * e;
        let wt = ancho - xt - 16.0 * e;
        p.texto_linea(&fila.nombre, xt, cy - 19.0 * e, 15.0 * e, wt, TEXTO);
        let (estado, reciente) = fila.estado(es_mio, otro_nombre);
        p.texto_linea(
            &estado,
            xt,
            cy + 3.0 * e,
            12.0 * e,
            wt,
            if reciente { VERDE } else { SUAVE },
        );
        l.zonas.push((r, t(Toque::Fila(i))));
        y += alto;
    }
    y += 6.0 * e;
    p.rellenar(rect(0.0, y, ancho, e.max(1.0)), BORDE);
    y += 10.0 * e;
    y += l.parrafo(
        &textos.t("sinc-elegir-como"),
        x0 - 4.0 * e,
        y,
        w + 8.0 * e,
        12.0 * e,
        SUAVE,
    );
    y + 10.0 * e
}

/// La barra de abajo, fija: «Todo», «Nada» y «Sincronizar N».
pub(super) fn pintar_pie(
    l: &mut Lienzo<'_>,
    textos: &Catalogo,
    el: &Eligiendo,
    ancho: f32,
    y: f32,
    alto: f32,
) {
    let (p, e) = (l.p, l.e);
    p.rellenar(rect(0.0, y, ancho, alto), FONDO);
    p.rellenar(rect(0.0, y, ancho, e.max(1.0)), BORDE);
    let yb = y + (alto - 40.0 * e) / 2.0;
    let x = 12.0 * e;
    let w1 = l.boton_texto(
        x,
        yb,
        &textos.t("sinc-todo"),
        PRIMARIO,
        Accion::Elegir(Toque::Todo),
    );
    l.boton_texto(
        x + w1,
        yb,
        &textos.t("sinc-nada"),
        PRIMARIO,
        Accion::Elegir(Toque::Nada),
    );
    let n = el.cuantos();
    let texto = con(textos, "sinc-sincronizar-n", &[("n", n.to_string())]);
    let (wt, _) = p.medir_texto(&texto, 14.0 * e);
    let wb = wt + 64.0 * e;
    let r = rect(
        ancho - 12.0 * e - wb,
        y + (alto - 48.0 * e) / 2.0,
        wb,
        48.0 * e,
    );
    if n > 0 {
        l.boton(
            r,
            &texto,
            Some(&super::SINCRO),
            true,
            Accion::Elegir(Toque::Sincronizar),
        );
    } else {
        // `enabled = false`: el boton esta pero no hace nada.
        p.rellenar_redondeado(r, r.alto / 2.0, CONTORNO);
        let (w, h) = p.medir_texto(&texto, 14.0 * e);
        p.texto(
            &texto,
            r.x + (r.ancho - w) / 2.0,
            r.y + (r.alto - h) / 2.0,
            14.0 * e,
            SOBRE_PRIMARIO,
        );
    }
}
