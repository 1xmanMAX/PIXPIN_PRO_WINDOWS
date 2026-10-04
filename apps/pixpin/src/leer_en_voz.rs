//! **Escuchar un documento en voz alta** en los lectores (Word, libro,
//! Markdown y PDF). Es `ui/LectorEnVoz.kt` y la `BarraDeEscuchar` de
//! `ui/VisorHtmlActivity.kt` del movil (22/23-sep-2026).
//!
//! En el movil habla la voz de Google del telefono, sin conexion; aqui, **la
//! voz de Windows** (`ISpVoice`, `pixpin_voz::sapi::Lector`): ya esta en el
//! equipo, no pesa nada y el texto no sale de el. Las cuentas (trozos,
//! idioma, velocidad, el marcador verde) estan en `pixpin_docs::voz_alta`.
//!
//! El documento se le da **por parrafos** y cada parrafo **en trozos**
//! (`voz_alta::trozos`): se dice un trozo, y cuando la voz acaba
//! ([`LeerEnVoz::vuelta`], que el lector llama en cada vuelta de su bucle)
//! se dice el siguiente. Cada parrafo nuevo se avisa ([`Suceso::Parrafo`])
//! para que el lector lo resalte, lo siga y mueva el verde.
//!
//! Diferencias con el movil, porque Windows es otro: no hay voces de
//! Microsoft Edge en linea ni auricular de llamadas, y la lectura vive
//! mientras el lector esta abierto (no hay notificacion de reproductor).
//! La pausa si es mejor: SAPI se para a media palabra y sigue ahi.

#![forbid(unsafe_code)]

use pixpin_docs::voz_alta;
use pixpin_render::icono::material;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_voz::sapi::Lector;

use crate::lector::{APAGADO, CRISTAL, TEXTO, ahora_ms, con_alfa};

/// El ambar del parrafo que suena (`.pixpin-leyendo` del movil:
/// `rgba(255,196,64,.30)`).
pub const AMBAR_LEYENDO: Color = Color {
    r: 1.0,
    g: 196.0 / 255.0,
    b: 64.0 / 255.0,
    a: 0.30,
};

/// Lo que tarda como poco la voz en ponerse a hablar tras pedirselo: antes
/// de esto su «he acabado» puede ser el de lo de antes.
const MS_PARA_EMPEZAR: u64 = 150;

/// Lo que paso en una vuelta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suceso {
    /// Empieza a sonar este parrafo.
    Parrafo(usize),
    /// Se acabo el documento.
    Acabado,
}

/// Los botones de la barra de escuchar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotonVoz {
    Anterior,
    Alternar,
    Siguiente,
    Velocidad,
    Cerrar,
}

pub struct LeerEnVoz {
    lector: Lector,
    parrafos: Vec<String>,
    /// El parrafo que suena (o donde se paro).
    pub actual: usize,
    trozos: Vec<String>,
    trozo: usize,
    pub leyendo: bool,
    /// La voz esta parada a media frase ([`LeerEnVoz::alternar`] sigue ahi).
    en_pausa: bool,
    pub velocidad: f32,
    hablado_en: u64,
}

impl LeerEnVoz {
    /// Una voz para `idioma` (o, si Windows no la tiene, para `reserva`) con
    /// el documento dicho en `parrafos`. No empieza a sonar: para eso,
    /// [`LeerEnVoz::leer`]. `None` si Windows no tiene ninguna de las dos.
    /// COM tiene que estar iniciado en el hilo (lo esta en los lectores).
    pub fn arrancar(
        idioma: &str,
        reserva: &str,
        parrafos: Vec<String>,
        velocidad: f32,
    ) -> Option<LeerEnVoz> {
        let lector = Lector::nuevo(idioma).or_else(|| Lector::nuevo(reserva))?;
        lector.poner_tasa(voz_alta::tasa_de_windows(velocidad));
        Some(LeerEnVoz {
            lector,
            parrafos,
            actual: 0,
            trozos: Vec::new(),
            trozo: 0,
            leyendo: false,
            en_pausa: false,
            velocidad,
            hablado_en: 0,
        })
    }

    pub fn cuantos(&self) -> usize {
        self.parrafos.len()
    }

    /// «Microsoft Helena Desktop · Windows», para la barra.
    pub fn nombre(&self) -> String {
        let n = self.lector.nombre();
        if n.is_empty() {
            "Windows".into()
        } else {
            format!("{n} · Windows")
        }
    }

    /// Empieza a leer en el parrafo `desde`, por su principio.
    pub fn leer(&mut self, desde: usize) {
        if self.parrafos.is_empty() {
            return;
        }
        self.actual = desde.min(self.parrafos.len() - 1);
        self.trozos = voz_alta::trozos(&self.parrafos[self.actual], voz_alta::TOPE);
        self.trozo = 0;
        self.leyendo = true;
        self.en_pausa = false;
        self.decir();
    }

    fn decir(&mut self) {
        match self.trozos.get(self.trozo) {
            Some(t) => self.lector.leer(t),
            // Un parrafo sin nada que decir: se pasa en la siguiente vuelta.
            None => self.lector.callar(),
        }
        self.hablado_en = ahora_ms();
    }

    /// Play/pausa.
    pub fn alternar(&mut self) {
        if self.leyendo {
            self.lector.pausar();
            self.leyendo = false;
            self.en_pausa = true;
        } else if self.en_pausa {
            self.lector.reanudar();
            self.leyendo = true;
            self.en_pausa = false;
        } else {
            self.leer(self.actual);
        }
    }

    /// Un parrafo adelante (+1) o atras (−1), desde su principio. Devuelve
    /// el parrafo en el que queda.
    pub fn saltar(&mut self, cuantos: isize) -> usize {
        if self.parrafos.is_empty() {
            return 0;
        }
        let a =
            (self.actual as isize + cuantos).clamp(0, self.parrafos.len() as isize - 1) as usize;
        if self.leyendo {
            self.leer(a);
        } else {
            self.lector.callar();
            self.en_pausa = false;
            self.actual = a;
        }
        a
    }

    /// La velocidad siguiente; sonando, se vuelve a empezar el trozo con ella.
    pub fn otra_velocidad(&mut self) -> f32 {
        self.velocidad = voz_alta::siguiente_velocidad(self.velocidad);
        self.lector
            .poner_tasa(voz_alta::tasa_de_windows(self.velocidad));
        if self.leyendo {
            self.decir();
        } else if self.en_pausa {
            // Lo pausado seguiria a la velocidad de antes: se empieza el trozo.
            self.lector.callar();
            self.en_pausa = false;
        }
        self.velocidad
    }

    /// **Lo que paso desde la ultima vuelta**: si la voz acabo su trozo, se
    /// dice el siguiente, o el siguiente parrafo, o se acaba el documento.
    pub fn vuelta(&mut self) -> Option<Suceso> {
        if !self.leyendo || ahora_ms() < self.hablado_en + MS_PARA_EMPEZAR || !self.lector.acabado()
        {
            return None;
        }
        if self.trozo + 1 < self.trozos.len() {
            self.trozo += 1;
            self.decir();
            return None;
        }
        if self.actual + 1 < self.parrafos.len() {
            self.leer(self.actual + 1);
            return Some(Suceso::Parrafo(self.actual));
        }
        self.leyendo = false;
        self.actual = 0;
        Some(Suceso::Acabado)
    }
}

impl Drop for LeerEnVoz {
    fn drop(&mut self) {
        self.lector.callar();
    }
}

/// **La barra de escuchar**, abajo y de cristal como la del movil: atras un
/// parrafo, play/pausa, adelante uno, la velocidad (rota al pulsarla) y
/// cerrar. Debajo, que voz lee y por donde va. Devuelve donde cae cada boton.
pub fn pintar_barra(
    p: &Pintor,
    ancho: f32,
    alto: f32,
    e: f32,
    voz: &LeerEnVoz,
) -> Vec<(RectF, BotonVoz)> {
    let mut botones = Vec::new();
    let lado = 40.0 * e;
    let rotulo = voz_alta::rotulo(voz.velocidad);
    let tam = 14.0 * e;
    let (w_vel, _) = p.medir_texto(&rotulo, tam);
    let ancho_vel = w_vel + 20.0 * e;
    let fila = 4.0 * lado + ancho_vel;
    let debajo = format!(
        "{} · {} / {}",
        voz.nombre(),
        voz.actual + 1,
        voz.cuantos().max(1)
    );
    let tam_debajo = 11.0 * e;
    let (w_debajo, h_debajo) = p.medir_texto(&debajo, tam_debajo);
    let total = fila.max(w_debajo + 24.0 * e) + 12.0 * e;
    let caja = RectF {
        x: (ancho - total) / 2.0,
        y: alto - 24.0 * e - lado - h_debajo - 10.0 * e,
        ancho: total,
        alto: lado + h_debajo + 10.0 * e,
    };
    p.rellenar_redondeado(caja, 20.0 * e, con_alfa(CRISTAL, 0.95));
    let mut x = caja.x + (total - fila) / 2.0;
    let y = caja.y;
    let icono = |p: &Pintor, i: &pixpin_render::icono::Icono, r: RectF, l: f32| {
        p.icono(
            i,
            RectF {
                x: r.x + (r.ancho - l) / 2.0,
                y: r.y + (r.alto - l) / 2.0,
                ancho: l,
                alto: l,
            },
            TEXTO,
        );
    };
    let texto = |p: &Pintor, t: &str, r: RectF, tam: f32| {
        let (w, h) = p.medir_texto(t, tam);
        p.texto(
            t,
            r.x + (r.ancho - w) / 2.0,
            r.y + (r.alto - h) / 2.0,
            tam,
            TEXTO,
        );
    };
    let r = RectF {
        x,
        y,
        ancho: lado,
        alto: lado,
    };
    texto(p, "⏮", r, 16.0 * e);
    botones.push((r, BotonVoz::Anterior));
    x += lado;
    let r = RectF {
        x,
        y,
        ancho: lado,
        alto: lado,
    };
    icono(
        p,
        if voz.leyendo {
            &material::PAUSE
        } else {
            &material::PLAY_ARROW
        },
        r,
        28.0 * e,
    );
    botones.push((r, BotonVoz::Alternar));
    x += lado;
    let r = RectF {
        x,
        y,
        ancho: lado,
        alto: lado,
    };
    texto(p, "⏭", r, 16.0 * e);
    botones.push((r, BotonVoz::Siguiente));
    x += lado;
    let r = RectF {
        x,
        y,
        ancho: ancho_vel,
        alto: lado,
    };
    texto(p, &rotulo, r, tam);
    botones.push((r, BotonVoz::Velocidad));
    x += ancho_vel;
    let r = RectF {
        x,
        y,
        ancho: lado,
        alto: lado,
    };
    icono(p, &material::CLOSE, r, 20.0 * e);
    botones.push((r, BotonVoz::Cerrar));
    p.texto(
        &debajo,
        caja.x + (total - w_debajo) / 2.0,
        y + lado,
        tam_debajo,
        APAGADO,
    );
    botones
}

/// Lo que se le dice al usuario cuando Windows no tiene voz de un idioma.
pub fn sin_voz(textos: &pixpin_store::Catalogo, idioma: &str) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("idioma", voz_alta::partes(idioma).0);
    textos.t_args("lector-sin-voz", &args)
}

/// El idioma de reserva: el de la aplicacion (`es`/`en`).
pub fn idioma_de_la_app(textos: &pixpin_store::Catalogo) -> String {
    textos.t("lector-voz-idioma")
}
