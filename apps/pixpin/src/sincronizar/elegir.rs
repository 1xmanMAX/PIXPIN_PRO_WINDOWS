//! «¿Qué sincronizar?», la pantalla de elegir chats antes de una vuelta.
//!
//! Copia de `Eligiendo` de `sincro/SincronizarActivity.kt`: dos pestanas, lo
//! de este aparato y lo del otro; el punto verde es lo que esta en los dos y
//! el rojo lo que solo esta en uno y se copiara al otro; debajo, cual de las
//! dos versiones es la mas reciente, sin horas. Abajo, «Todo», «Nada» y
//! «Sincronizar N».
//!
//! **Antes de borrar, se pregunta aqui** (v0.79 del movil): lo que el otro
//! borro sale con su casilla y el aviso «SE BORRARA AQUI», y solo se borra
//! si se deja marcado; lo que se borro aqui y se borrara alli se dice en
//! rojo. Y el interruptor «Juntar / Lo mio manda» para cuando se vacio un
//! aparato y se quiere que el otro lo vuelva a llenar, no que se vacie
//! tambien. La cuenta de que se borra es `pixpin_sincro::vuelta`; aqui solo
//! se ensena.
//!
//! El hilo de la vuelta se queda esperando la respuesta por `responde`. Si la
//! pantalla se cierra sin contestar, soltarla contesta «cancelar»: un hilo
//! esperando para siempre dejaria el aparato ocupado.

use std::sync::mpsc;

use pixpin_render::icono::material;
use pixpin_sincro::vuelta::{self, Eleccion, Fila, Pregunta};
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
    /// El interruptor: `true` es «Lo mio manda».
    Modo(bool),
    Sincronizar,
    Cancelar,
}

pub(super) struct Eligiendo {
    /// Con quien, por su nombre.
    pub otro: String,
    filas: Vec<Fila>,
    marcados: Vec<bool>,
    /// Lo que se borrara alli porque se borro aqui.
    se_borran_alli: Vec<String>,
    lo_mio_manda: bool,
    pestana: usize,
    responde: Option<mpsc::Sender<Option<Eleccion>>>,
}

impl Eligiendo {
    pub fn nuevo(
        otro: String,
        pregunta: Pregunta,
        responde: mpsc::Sender<Option<Eleccion>>,
    ) -> Eligiendo {
        Eligiendo {
            otro,
            marcados: pregunta.filas.iter().map(|f| f.marcado).collect(),
            filas: pregunta.filas,
            se_borran_alli: pregunta.se_borran_alli,
            lo_mio_manda: pregunta.lo_mio_manda,
            pestana: 0,
            responde: Some(responde),
        }
    }

    fn cuantos(&self) -> usize {
        self.marcados.iter().filter(|m| **m).count()
    }

    fn contestar(&mut self, eleccion: Option<Eleccion>) {
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
        Toque::Modo(lo_mio_manda) => {
            el.lo_mio_manda = lo_mio_manda;
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
            let lo_mio_manda = el.lo_mio_manda;
            el.contestar(Some(Eleccion {
                elegidos,
                lo_mio_manda,
            }));
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

/// Lo que se lee bajo el nombre de una fila, y en que color. Una fila que
/// el otro borro lo dice siempre, en las dos pestanas: es lo que mas importa
/// de ella.
fn estado_de_fila(
    textos: &Catalogo,
    fila: &Fila,
    marcada: bool,
    lo_mio_manda: bool,
    es_mio: bool,
    otro: &str,
    otro_de_la_pestana: &str,
) -> (String, pixpin_render::Color) {
    if fila.se_borra_aqui {
        let o = [("otro", otro.to_string())];
        return if lo_mio_manda {
            (con(textos, "sincro-borrado-alli-vuelve", &o), VERDE)
        } else if marcada {
            (con(textos, "sincro-borrado-alli-se-borra", &o), ROJO)
        } else {
            (con(textos, "sincro-borrado-alli-se-queda", &o), SUAVE)
        };
    }
    let (texto, reciente) = fila.estado(es_mio, otro_de_la_pestana);
    (texto, if reciente { VERDE } else { SUAVE })
}

/// El aviso en rojo de lo que se va a borrar, si hay algo. Con «Lo mio
/// manda» no se borra nada aqui y lo de alli se dice en su explicacion.
fn aviso_de_borrados(textos: &Catalogo, el: &Eligiendo) -> Option<String> {
    if el.lo_mio_manda {
        return None;
    }
    let lista = |v: Vec<String>| {
        v.iter()
            .map(|n| format!("«{n}»"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut partes = Vec::new();
    let aqui = vuelta::se_borrarian_aqui(&el.filas, &el.marcados, false);
    if !aqui.is_empty() {
        partes.push(con(
            textos,
            "sincro-se-borran-aqui",
            &[
                ("otro", el.otro.clone()),
                (
                    "lista",
                    lista(aqui.iter().map(|f| f.nombre.clone()).collect()),
                ),
            ],
        ));
    }
    if !el.se_borran_alli.is_empty() {
        partes.push(con(
            textos,
            "sincro-se-borran-alli",
            &[
                ("otro", el.otro.clone()),
                ("lista", lista(el.se_borran_alli.clone())),
            ],
        ));
    }
    (!partes.is_empty()).then(|| partes.join("\n"))
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
        let (estado, color) = estado_de_fila(
            textos,
            fila,
            marcada,
            el.lo_mio_manda,
            es_mio,
            &el.otro,
            otro_nombre,
        );
        p.texto_linea(&estado, xt, cy + 3.0 * e, 12.0 * e, wt, color);
        l.zonas.push((r, t(Toque::Fila(i))));
        y += alto;
    }
    y += 6.0 * e;
    p.rellenar(rect(0.0, y, ancho, e.max(1.0)), BORDE);
    y += 10.0 * e;

    // «Juntar | Lo mio manda» (`DosModos` del movil): dos botones, el
    // elegido relleno.
    let medio = (w - 8.0 * e) / 2.0;
    let modos = [
        (textos.t("sincro-juntar"), false),
        (textos.t("sincro-lo-mio-manda"), true),
    ];
    for (k, (texto, manda)) in modos.iter().enumerate() {
        l.boton(
            rect(x0 + k as f32 * (medio + 8.0 * e), y, medio, 40.0 * e),
            texto,
            None,
            el.lo_mio_manda == *manda,
            t(Toque::Modo(*manda)),
        );
    }
    y += 40.0 * e + 10.0 * e;

    if let Some(aviso) = aviso_de_borrados(textos, el) {
        y += l.parrafo(&aviso, x0 - 4.0 * e, y, w + 8.0 * e, 12.0 * e, ROJO) + 8.0 * e;
    }
    let como = if el.lo_mio_manda {
        con(
            textos,
            "sincro-lo-mio-manda-como",
            &[("yo", yo.to_string()), ("otro", el.otro.clone())],
        )
    } else {
        textos.t("sinc-elegir-como")
    };
    y += l.parrafo(&como, x0 - 4.0 * e, y, w + 8.0 * e, 12.0 * e, SUAVE);
    y + 10.0 * e
}

/// La barra de abajo, fija: «Todo», «Nada» y «Sincronizar N» (o «Mandar N»
/// con «Lo mio manda»).
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
    let clave = if el.lo_mio_manda {
        "sincro-mandar-n"
    } else {
        "sinc-sincronizar-n"
    };
    let texto = con(textos, clave, &[("n", n.to_string())]);
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

#[cfg(test)]
mod pruebas {
    use super::*;

    fn fila(id: &str, marcado: bool, se_borra_aqui: bool) -> Fila {
        Fila {
            id: id.into(),
            nombre: id.to_uppercase(),
            aqui: None,
            alli: None,
            marcado,
            se_borra_aqui,
        }
    }

    fn eligiendo(filas: Vec<Fila>) -> (Eligiendo, mpsc::Receiver<Option<Eleccion>>) {
        let (tx, rx) = mpsc::channel();
        let el = Eligiendo::nuevo(
            "Móvil".into(),
            Pregunta {
                filas,
                se_borran_alli: Vec::new(),
                lo_mio_manda: false,
            },
            tx,
        );
        (el, rx)
    }

    #[test]
    fn sincronizar_sin_tocar_la_casilla_no_manda_lo_que_se_borraria_aqui() {
        let (mut el, rx) = eligiendo(vec![fila("a", true, false), fila("b", false, true)]);
        assert!(tocar(&mut el, Toque::Sincronizar));
        let e = rx.recv().unwrap().unwrap();
        assert!(e.elegidos.contains("a"));
        assert!(
            !e.elegidos.contains("b"),
            "sin marcar no se elige, y no se borra"
        );
        assert!(!e.lo_mio_manda);
    }

    #[test]
    fn marcar_lo_que_se_borraria_aqui_lo_elige_y_lo_mio_manda_viaja_en_la_respuesta() {
        let (mut el, rx) = eligiendo(vec![fila("a", true, false), fila("b", false, true)]);
        tocar(&mut el, Toque::Fila(1));
        tocar(&mut el, Toque::Modo(true));
        assert!(tocar(&mut el, Toque::Sincronizar));
        let e = rx.recv().unwrap().unwrap();
        assert!(e.elegidos.contains("b"));
        assert!(e.lo_mio_manda);
    }

    #[test]
    fn cerrar_sin_contestar_es_cancelar() {
        // Caso negativo: una pantalla que se cierra no puede dejar la vuelta
        // esperando ni contestar «si».
        let (el, rx) = eligiendo(vec![fila("a", true, false)]);
        drop(el);
        assert_eq!(rx.recv().unwrap(), None);
    }

    #[test]
    fn con_nada_marcado_sincronizar_no_contesta() {
        let (mut el, rx) = eligiendo(vec![fila("a", false, false)]);
        assert!(!tocar(&mut el, Toque::Sincronizar));
        assert!(rx.try_recv().is_err(), "el boton esta apagado");
    }
}
