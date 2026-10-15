//! **El sistema de diseno v2** (6-oct-2026): los colores, radios, letras y
//! medidas que comparten las cuatro ventanas del rediseno (galeria, tareas,
//! timeline y lecciones), y las piezas que se repetian en cada una (el aviso
//! de abajo, el boton de icono, la tarjeta, el estado vacio, pegar del
//! portapapeles).
//!
//! POR QUE un modulo aparte: cada ventana se habia hecho su propia copia de
//! `blanco()`, `mezcla()`, `encoger()`, su aviso y su boton, con valores que
//! ya no casaban (un aviso con radio 10 y otro con 14, tres azules). El
//! revisor lo marco como lo que mas restaba. Aqui hay UNA fuente: si un valor
//! cambia, cambia en las cuatro.
//!
//! Los valores son los mas usados hoy en las maquetas v2 (`*2*.dc.html`), en
//! pixeles logicos: se multiplican por la escala del monitor al pintar.

#![forbid(unsafe_code)]

pub mod aviso;
pub mod boton;
pub mod color;
pub mod geom;
pub mod pegar;
pub mod pista;
pub mod tarjeta;

pub use boton::boton_icono;
pub use tarjeta::vacio;

use pixpin_render::Color;

use crate::caja_dibujo::hex;

// ---------------------------------------------------------------- colores

/// El fondo de las ventanas (oscuras: el v2 no tiene tema claro).
pub const FONDO: Color = hex(0x1C1C1E);
/// Una tarjeta en reposo.
pub const TARJETA: Color = hex(0x2A2A2D);
/// La tarjeta con el raton encima.
pub const TARJETA_ENCIMA: Color = hex(0x313134);
/// Una tarjeta ya hecha (tarea marcada): mas hundida que las demas.
#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub const TARJETA_HECHA: Color = hex(0x222225);
/// Las cajas de texto y los botones normales.
pub const CAJA: Color = hex(0x2C2C2E);
/// Las rayas que separan: blanco al 7 %.
#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub const LINEA: Color = Color {
    a: 0.07,
    ..Color::BLANCO
};
/// Titulos y lo que se lee primero.
pub const TEXTO: Color = hex(0xF5F5F7);
/// El cuerpo de un texto largo y las chapitas.
pub const CUERPO: Color = hex(0xD1D1D6);
/// Lo secundario: fechas, cuentas, pistas.
pub const GRIS: Color = hex(0x98989D);
/// El azul de lo elegido y del foco (anillos, enlaces de accion).
pub const ACENTO: Color = hex(0x0A84FF);
/// El azul de relleno de la accion principal: con letra blanca encima se
/// lee mejor que el ACENTO.
pub const AZUL_LLENO: Color = hex(0x0060DF);
/// Un enlace sobre fondo negro (el «Deshacer» del aviso).
pub const ENLACE: Color = hex(0x64D2FF);
/// Lo que se va: borrar, caducar.
pub const ROJO: Color = hex(0xFF453A);
/// El rojo de una letra (mas claro: el ROJO como letra vibra sobre oscuro).
pub const ROJO_TEXTO: Color = hex(0xFF6961);
pub const VERDE: Color = hex(0x30D158);
#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub const NARANJA: Color = hex(0xFF9F0A);
#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub const AMARILLO: Color = hex(0xFFD60A);

// ---------------------------------------------------------------- radios

/// La chapita de un atajo o un numero.
#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub const RADIO_CHAPA: f32 = 5.0;
pub const RADIO_BOTON: f32 = 10.0;
pub const RADIO_TARJETA: f32 = 12.0;
/// Lo que flota sobre lo demas: menu, aviso, barra, caja.
pub const RADIO_FLOTANTE: f32 = 14.0;

// ---------------------------------------------------------------- letras

#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub const LETRA_TITULO_VENTANA: f32 = 17.0;
#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub const LETRA_SUBTITULO: f32 = 12.0;
pub const LETRA_TITULO_TARJETA: f32 = 15.0;
pub const LETRA_CUERPO: f32 = 14.0;
pub const LETRA_SECUNDARIO: f32 = 13.0;
#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub const LETRA_CHAPA: f32 = 11.5;

// ---------------------------------------------------------------- medidas

#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub const CABECERA: f32 = 64.0;
/// Alto de un boton y de una caja de texto.
pub const BOTON: f32 = 40.0;
#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub const FILA_MENU: f32 = 44.0;
#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub const CHAPA: f32 = 20.0;
pub const MARGEN: f32 = 16.0;
pub const HUECO: f32 = 8.0;
pub const HUECO_GRANDE: f32 = 12.0;
/// Lo minimo que mide algo que se pulsa, aunque se vea mas pequeno: con
/// menos, el raton se queda corto (principio aprobado del v2).
pub const OBJETIVO_MINIMO: f32 = 40.0;
