//! **Sincronizar → Nube** (1-nov-2026): la direccion de la web, la llave
//! `pxn_…` del aparato (se guarda cifrada) y «Probar», que dice en que
//! espacio entra y con que persona. Lo de la red esta en [`super::nube`].

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use pindesk_render::icono::material;
use pindesk_store::Catalogo;

use super::compartidos_ui::campo_linea;
use super::{Accion, ERROR, Lienzo, SUAVE, TEXTO, Tecla, VERDE, caja, rect};
use crate::caja_texto::CajaLinea;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Toque {
    CampoDireccion,
    CampoLlave,
    Guardar,
    Probar,
    OlvidarLlave,
}

pub(super) struct Ajustes {
    raiz: PathBuf,
    direccion: CajaLinea,
    llave: CajaLinea,
    en_llave: bool,
    guardada: Option<String>,
    probando: Arc<AtomicBool>,
    aviso: Option<(&'static str, String, bool)>,
}

impl Ajustes {
    pub(super) fn nuevo(raiz: PathBuf) -> Ajustes {
        let d = super::nube::direccion(&raiz);
        Ajustes {
            direccion: CajaLinea::con_texto(&d).con_maximo(300).sin_parpadeo(),
            llave: CajaLinea::nueva().con_maximo(200).sin_parpadeo(),
            en_llave: false,
            guardada: super::nube::llave_a_la_vista(),
            probando: Arc::new(AtomicBool::new(false)),
            aviso: None,
            raiz,
        }
    }

    pub(super) fn animando(&self) -> bool {
        self.probando.load(Ordering::SeqCst)
    }
}

/// La caja donde se escribe.
pub(super) fn caja_visible(s: &mut Ajustes) -> Option<&mut CajaLinea> {
    Some(if s.en_llave {
        &mut s.llave
    } else {
        &mut s.direccion
    })
}

pub(super) fn tecla(s: &mut Ajustes, t: Tecla) -> bool {
    let campo = if s.en_llave {
        &mut s.llave
    } else {
        &mut s.direccion
    };
    match t {
        Tecla::Letra(c) => {
            campo.caracter(c);
        }
        Tecla::Borrar => {
            campo.tecla(
                0x08,
                false,
                false,
                &mut crate::caja_texto::PortapapelesWindows,
            );
        }
        Tecla::Caja { vk, shift, ctrl } => {
            return campo
                .tecla(vk, shift, ctrl, &mut crate::caja_texto::PortapapelesWindows)
                .atendida();
        }
        Tecla::Pegar(texto) => {
            campo.pegar_texto(texto.trim());
        }
        Tecla::Intro => return false,
    }
    true
}

pub(super) fn tocar(s: &mut Ajustes, t: Toque) {
    match t {
        Toque::CampoDireccion => s.en_llave = false,
        Toque::CampoLlave => s.en_llave = true,
        Toque::OlvidarLlave => {
            super::nube::olvidar_llave();
            s.guardada = None;
        }
        Toque::Guardar => {
            let llave = s.llave.trim().to_string();
            match super::nube::guardar(
                &s.raiz,
                s.direccion.trim(),
                (!llave.is_empty()).then_some(llave.as_str()),
            ) {
                Ok(()) => {
                    s.llave.poner_texto("");
                    s.guardada = super::nube::llave_a_la_vista();
                    s.aviso = Some(("nube-guardado", String::new(), true));
                }
                Err(e) if e == "direccion" => {
                    s.aviso = Some(("nube-direccion-mala", String::new(), false))
                }
                Err(e) if e == "llave" => s.aviso = Some(("nube-llave-mala", String::new(), false)),
                Err(e) => s.aviso = Some(("", e, false)),
            }
        }
        Toque::Probar => {
            if s.probando.swap(true, Ordering::SeqCst) {
                return;
            }
            s.aviso = None;
            let (raiz, probando) = (s.raiz.clone(), s.probando.clone());
            let _ = std::thread::Builder::new()
                .name("nube-probar".into())
                .spawn(move || {
                    let _ = super::nube::probar(&raiz);
                    probando.store(false, Ordering::SeqCst);
                    crate::ventana_chat::refrescar();
                });
        }
    }
}

pub(super) fn pintar(
    l: &mut Lienzo<'_>,
    textos: &Catalogo,
    s: &Ajustes,
    x: f32,
    w: f32,
    mut y: f32,
) -> f32 {
    let (p, e) = (l.p, l.e);
    let t = |a: Toque| Accion::Nube(a);
    y += 8.0 * e;
    let explica = textos.t("nube-explica");
    let h = l.alto_de(&explica, 14.0 * e, w - 32.0 * e) + 32.0 * e;
    caja(p, rect(x, y, w, h), e);
    l.parrafo(
        &explica,
        x + 16.0 * e,
        y + 16.0 * e,
        w - 32.0 * e,
        14.0 * e,
        TEXTO,
    );
    y += h + 20.0 * e;

    p.texto(&textos.t("nube-direccion"), x, y, 12.0 * e, SUAVE);
    y += 20.0 * e;
    campo_linea(
        l,
        rect(x, y, w, 48.0 * e),
        &s.direccion,
        !s.en_llave,
        t(Toque::CampoDireccion),
    );
    y += 48.0 * e + 16.0 * e;
    p.texto(&textos.t("nube-llave"), x, y, 12.0 * e, SUAVE);
    y += 20.0 * e;
    campo_linea(
        l,
        rect(x, y, w, 48.0 * e),
        &s.llave,
        s.en_llave,
        t(Toque::CampoLlave),
    );
    y += 48.0 * e + 6.0 * e;
    if let Some(g) = &s.guardada {
        let dicho = format!("{} {g}", textos.t("nube-llave-guardada"));
        p.texto_linea(&dicho, x, y, 12.0 * e, w - 100.0 * e, SUAVE);
        l.boton_texto(
            x + w - 100.0 * e,
            y - 12.0 * e,
            &textos.t("nube-olvidar"),
            ERROR,
            t(Toque::OlvidarLlave),
        );
    }
    y += 30.0 * e;
    let mitad = (w - 8.0 * e) / 2.0;
    l.boton(
        rect(x, y, mitad, 48.0 * e),
        &textos.t("nube-guardar"),
        None,
        true,
        t(Toque::Guardar),
    );
    l.boton(
        rect(x + mitad + 8.0 * e, y, mitad, 48.0 * e),
        &textos.t("nube-probar"),
        Some(&material::CLOUD),
        false,
        t(Toque::Probar),
    );
    y += 48.0 * e + 16.0 * e;

    if s.animando() {
        l.girando(x + w / 2.0, y + 12.0 * e, 10.0 * e, 3.0 * e);
        y += 30.0 * e;
    } else if let Some((clave, libre, bien)) = &s.aviso {
        let a = if clave.is_empty() {
            libre.clone()
        } else {
            textos.t(clave)
        };
        y += l.parrafo_centrado(&a, x, y, w, 14.0 * e, if *bien { VERDE } else { ERROR });
    } else if let Some(r) = super::nube::ultimo_saludo() {
        let (texto, bien) = match r {
            Ok(espacio) => {
                let mut a = fluent_bundle::FluentArgs::new();
                a.set("espacio", espacio);
                (textos.t_args("nube-probado", &a), true)
            }
            Err(e) => (format!("{} {e}", textos.t("nube-no-contesta")), false),
        };
        y += l.parrafo_centrado(&texto, x, y, w, 14.0 * e, if bien { VERDE } else { ERROR });
    }
    y += 16.0 * e;
    let pie = textos.t("nube-que-va");
    y += l.parrafo(&pie, x, y, w, 12.0 * e, SUAVE);
    y
}
