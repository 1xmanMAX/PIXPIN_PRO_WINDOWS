//! Los iconos de linea del panel de la bandeja, copiados de la maqueta
//! (`Bandeja2.dc.html`): trazo de 2 en una caja de 24, extremos y uniones
//! redondos. Los `<rect>` y `<circle>` de la maqueta van reescritos como
//! trayectos, que es lo unico que sabe leer el motor.

use pixpin_render::icono::{Icono, Pintura, TrazoIcono};

const VISTA: (f32, f32, f32, f32) = (0.0, 0.0, 24.0, 24.0);

const fn linea(d: &'static str) -> TrazoIcono {
    TrazoIcono {
        d,
        relleno: Pintura::Nada,
        trazo: Pintura::Actual,
        grosor: 2.0,
        extremo_redondo: true,
        union_redonda: true,
        opacidad: 1.0,
        par_impar: false,
        matriz: None,
        mascara: None,
    }
}

macro_rules! icono {
    ($nombre:ident, $($d:expr),+ $(,)?) => {
        pub const $nombre: Icono = Icono {
            vista: VISTA,
            trazos: &[$(linea($d)),+],
        };
    };
}

icono!(
    CAPTURAR,
    "M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3"
);
icono!(
    ZONA,
    "M4 5h3M10 5h4M17 5h3v3M20 11v2M20 16v3h-3M14 19h-4M7 19H4v-3M4 13v-2M4 8V5"
);
icono!(
    SCROLL,
    "M8 2h8a2 2 0 0 1 2 2v16a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2zM12 7v10M9 14l3 3 3-3"
);
icono!(GRABAR, "M21 12a9 9 0 1 1-18 0a9 9 0 1 1 18 0z");
icono!(TEXTO, "M4 7V5h16v2M12 5v14M9 19h6");
icono!(
    RETARDO,
    "M20 13a8 8 0 1 1-16 0a8 8 0 1 1 16 0zM12 9v4l2 2M9 2h6"
);
icono!(
    CUENTAGOTAS,
    "M14.5 4.5l5 5M12 7l5 5M16.5 2.5a2.1 2.1 0 0 1 3 3L17 8l-1-1zM13 8l-8 8v3h3l8-8"
);
icono!(PIN, "M12 17v5M5 17h14l-2-5V4H7v8z");
icono!(LAPIZ, "M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z");
icono!(MAS, "M12 5v14M5 12h14");
icono!(
    OJO_TACHADO,
    "M3 3l18 18M10.6 5.1A10 10 0 0 1 12 5c6 0 10 7 10 7a17 17 0 0 1-3.2 3.9M6.6 6.6A17 17 0 0 0 2 12s4 7 10 7a9.6 9.6 0 0 0 5.4-1.6"
);
icono!(CURSOR, "M5 3l14 7-6 2-2 6z");
icono!(
    TECLADO,
    "M4 6h16a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2zM6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8"
);
icono!(
    CHAT,
    "M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.6A8 8 0 1 1 21 12z"
);
icono!(
    TAREAS,
    "M7 3h10a4 4 0 0 1 4 4v10a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4V7a4 4 0 0 1 4-4zM8 12l3 3 5-6"
);
icono!(
    BOMBILLA,
    "M9 18h6M10 21h4M12 3a6 6 0 0 0-3.5 10.9c.6.5 1 1.2 1 2.1h5c0-.9.4-1.6 1-2.1A6 6 0 0 0 12 3z"
);
icono!(
    CARPETA,
    "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"
);
icono!(
    GALERIA,
    "M6 4h14a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zM4 14l5-5 4 4 3-3 6 6M2 8v12a2 2 0 0 0 2 2h14"
);
icono!(
    SALIR,
    "M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9"
);
icono!(
    SINCRONIZAR,
    "M21 12a9 9 0 0 1-15.5 6.2L3 16M3 12a9 9 0 0 1 15.5-6.2L21 8M21 3v5h-5M3 21v-5h5"
);
icono!(
    AJUSTES,
    "M15 12a3 3 0 1 1-6 0a3 3 0 1 1 6 0zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-2.9 1.2V21a2 2 0 0 1-4 0v-.1A1.7 1.7 0 0 0 7 19.4l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1A1.7 1.7 0 0 0 3 13.6H3a2 2 0 0 1 0-4h.1A1.7 1.7 0 0 0 4.6 7l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1A1.7 1.7 0 0 0 10.4 3V3a2 2 0 0 1 4 0v.1A1.7 1.7 0 0 0 17 4.6l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0 1.2 2.9h.1a2 2 0 0 1 0 4H21a1.7 1.7 0 0 0-1.6 1z"
);
icono!(
    BUSCAR,
    "M18 11a7 7 0 1 1-14 0a7 7 0 1 1 14 0zM20 20l-3.5-3.5"
);
icono!(
    COPIAR,
    "M11 9h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-8a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2zM5 15V5a2 2 0 0 1 2-2h8"
);
icono!(BORRAR, "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3");
icono!(VISTO, "M5 12l5 5 9-10");
icono!(
    PORTAPAPELES,
    "M9 4h6v3H9zM9 5H6a1 1 0 0 0-1 1v14a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1h-3"
);
icono!(DEVOLVER, "M9 14l-5-5 5-5M4 9h10a6 6 0 0 1 0 12h-3");
icono!(CERRAR, "M6 6l12 12M18 6L6 18");
icono!(ENCIMA, "M12 3v12M7 8l5-5 5 5M4 21h16");
icono!(GRUPO, "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z");
