//! Las pegatinas: un emoji sobre la esquina del pin (`PinWindowController`,
//! «Pegatinas», y `PinChrome.kt` del movil).
//!
//! La lista es la del movil, fija, en su orden: el que pone una pegatina en
//! un aparato y abre el mismo pin en otro reconoce el mismo dibujo. En un
//! temporizador la pegatina deja de ser adorno y hace de mando: cada emoji de
//! tiempo (`TIMER_PRESETS`) arranca esos minutos.
//!
//! Va en la esquina de arriba a la derecha, girada 30 grados y pisando el
//! borde: medio dentro y medio fuera de la tarjeta. Fuera cabe porque el pin
//! ya tiene alrededor el margen de la sombra (24 px), mas ancho que lo que
//! la pegatina sobresale (20).

/// Las pegatinas del menu (`PIN_EMOJIS`), en el orden del movil.
pub const PEGATINAS: [&str; 24] = [
    "⭐", "🔥", "❗", "✅", "❌", "⏳", "📌", "💡", "🔒", "💰", "📈", "🎯", "❤️", "👀", "🔔", "📅",
    "✏️", "🧠", "🚀", "🐛", "☕", "🎵", "📷", "🗑️",
];

/// Las de tiempo de un temporizador (`TIMER_PRESETS`): el emoji y sus
/// minutos.
pub const DE_TIEMPO: [(&str, u32); 4] = [("5️⃣", 5), ("🕒", 15), ("🕧", 30), ("🕐", 60)];

/// Lado de la caja de la pegatina, en px logicos (`STICKER_SIZE_DP`).
pub const LADO_LOGICO: f32 = 34.0;
/// Cuanto sobresale de la tarjeta por arriba y por la derecha
/// (`STICKER_INSET_DP`).
pub const SOBRESALE_LOGICO: f32 = 20.0;
/// El cuerpo del emoji (22 en el pin, como en el movil).
pub const CUERPO_LOGICO: f32 = 22.0;
/// El giro, en grados.
pub const GIRO_GRADOS: f32 = 30.0;

/// Las elegibles en un pin, en el orden del menu: primero las de tiempo si
/// es un temporizador, despues las de siempre. El indice que viaja en el
/// comando del menu es la posicion en esta lista.
pub fn elegibles(temporizador: bool) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    if temporizador {
        v.extend(DE_TIEMPO.iter().map(|(e, _)| *e));
    }
    v.extend(PEGATINAS);
    v
}

/// Los minutos que arranca una pegatina, si es de las de tiempo.
pub fn minutos_de(emoji: &str) -> Option<u32> {
    DE_TIEMPO.iter().find(|(e, _)| *e == emoji).map(|(_, m)| *m)
}

/// Si una pegatina guardada es de las conocidas. Una que no lo sea (un
/// fichero tocado a mano, una de una version futura) no se ensena.
pub fn conocida(emoji: &str) -> bool {
    PEGATINAS.contains(&emoji) || minutos_de(emoji).is_some()
}

/// La caja de la pegatina, en las coordenadas de quien pinta: `tarjeta` es
/// el rectangulo del contenido (x, y, ancho, alto) y `escala` el factor de
/// pixeles por px logico. Devuelve (x, y, lado).
pub fn caja(tarjeta: (f32, f32, f32, f32), escala: f32) -> (f32, f32, f32) {
    let (x, y, ancho, _) = tarjeta;
    let lado = LADO_LOGICO * escala;
    let fuera = SOBRESALE_LOGICO * escala;
    (x + ancho + fuera - lado, y - fuera, lado)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_de_tiempo_solo_salen_en_un_temporizador_y_van_primero() {
        let normal = elegibles(false);
        assert_eq!(normal.len(), PEGATINAS.len());
        assert_eq!(normal[0], "⭐");
        assert!(!normal.contains(&"🕒"), "caso negativo: no es temporizador");
        let reloj = elegibles(true);
        assert_eq!(reloj.len(), PEGATINAS.len() + DE_TIEMPO.len());
        assert_eq!(reloj[0], "5️⃣");
        assert_eq!(reloj[4], "⭐");
    }

    #[test]
    fn cada_pegatina_de_tiempo_arranca_sus_minutos() {
        assert_eq!(minutos_de("5️⃣"), Some(5));
        assert_eq!(minutos_de("🕒"), Some(15));
        assert_eq!(minutos_de("🕧"), Some(30));
        assert_eq!(minutos_de("🕐"), Some(60));
        assert_eq!(minutos_de("⭐"), None);
    }

    #[test]
    fn solo_se_reconocen_las_de_la_lista() {
        assert!(conocida("🔥"));
        assert!(conocida("🕐"));
        assert!(!conocida("hola"));
        assert!(!conocida(""));
    }

    #[test]
    fn la_pegatina_pisa_la_esquina_y_cabe_en_el_margen_de_la_sombra() {
        let (x, y, lado) = caja((24.0, 24.0, 200.0, 100.0), 1.0);
        // Sobresale 20 por arriba y por la derecha...
        assert_eq!(y, 4.0);
        assert_eq!(x + lado, 24.0 + 200.0 + 20.0);
        // ...y pisa la tarjeta.
        assert!(x < 224.0 && y + lado > 24.0);
        // Sin salirse del margen de la sombra.
        assert!(SOBRESALE_LOGICO < crate::ventana::MARGEN_SOMBRA_LOGICO as f32);
    }
}
