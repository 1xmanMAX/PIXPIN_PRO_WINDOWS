//! **El asistente local en el chat** (encargos 1-3 de
//! `docs/investigacion/2026-10-13-ia-local.md` §7).
//!
//! El cerebro esta en `crates/pixpin-ia` (motor llama.cpp en proceso aparte,
//! protocolo de herramientas, bucle `agente::turno`). Aqui va lo que solo
//! sabe la app:
//!
//! - [`motor`]: **un motor por proceso** de PixPin, del modelo elegido en
//!   los ajustes (2B o 0.8B), creado al primer uso. El propio motor cierra
//!   el modelo a los 5 min sin uso y devuelve la memoria entera.
//! - [`descarga`]: bajar el motor y el modelo en un hilo, con progreso y
//!   cancelar, para la tarjeta de la primera vez.
//! - [`turno`]: un turno del agente **en un hilo de fondo**. La UI no espera
//!   nunca: mira el canal una vez por vuelta de su bucle, como hace con la
//!   voz o la transcripcion, y pinta lo que llegue (texto en vivo, «dibujando…»,
//!   el final). «Parar» corta en el siguiente trozo.
//! - [`ejecutor`]: hace cada herramienta. Lo del lienzo se le pide a la UI
//!   (el lienzo vive en su hilo) y se espera su respuesta; el OCR corre en el
//!   propio hilo del turno ([`ocr`]).
//! - [`al_lienzo`]: meter lo dibujado en un lienzo de verdad, centrado en la
//!   vista y en **un solo paso de deshacer**.
//! - [`foto`]: reducir la foto adjunta a ~768 px antes de mandarla.
//! - [`pensado`]: lo que penso cada respuesta, su peticion mejorada y la
//!   hoja de trabajo del chat, guardados con la nota (13-oct).
//! - [`nube`]: contestar con Claude Haiku 5.5 o DeepSeek V4.1 Flash (la
//!   clave cifrada con DPAPI) y, si fallan, seguir con el local.
//! - [`documentos`]: `buscar_en_mis_cosas`: el indice local de los PDF,
//!   Office, notas y chats del usuario, al dia en segundo plano, y el
//!   permiso de mandar trozos a la nube.
//!
//! Android todavia no tiene asistente (comprobado el 13-oct con `gh`): aqui
//! se abre camino y el protocolo (`catalogo_json`, Excalidraw) es el que
//! portara el movil.

pub mod acciones;
pub mod al_lienzo;
pub mod anotar_inteligente;
pub mod chat;
pub mod conexiones;
pub mod descarga;
pub mod documentos;
pub mod documentos_acciones;
pub mod ejecutor;
pub mod en_el_chat;
pub mod externa;
pub mod foto;
pub mod hoja_a_lienzo;
pub mod memoria;
pub mod motor;
pub mod nube;
pub mod ocr;
pub mod ojos;
pub mod pensado;
pub mod permiso;
pub mod rapido;
pub mod recetas;
pub mod turno;

#[cfg(test)]
mod banco_documentos;
#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_diagramas;
#[cfg(test)]
mod pruebas_pensado;

/// **Hora local menos UTC** (ms), para dar al modelo de la nube las fechas
/// al minuto en la hora del usuario (`pixpin_ia::fechas::fecha_y_hora`).
/// Redondeado al cuarto de hora: las dos lecturas del reloj no caen en el
/// mismo milisegundo, y ningun huso va por debajo de 15 min.
pub fn desfase_local_ms() -> i64 {
    let d = pixpin_shell::entorno::ahora_local_ms() - pixpin_shell::entorno::ahora_utc_ms();
    const CUARTO: i64 = 15 * 60_000;
    (d as f64 / CUARTO as f64).round() as i64 * CUARTO
}

/// «Ahora: vie 17-oct-2026 14:05 (hora local).» al final del pedido, solo
/// con la nube (17-oct-2026): con la hora de ahora y la de cada archivo, el
/// modelo entiende «lo de esta mañana» o «el ultimo que subi» sin preguntar.
pub fn nota_de_ahora(ahora_utc_ms: i64, desfase_ms: i64) -> String {
    match pixpin_ia::fechas::fecha_y_hora(ahora_utc_ms, desfase_ms) {
        Some(f) => format!("\n\n(Ahora: {f}, hora local del usuario.)"),
        None => String::new(),
    }
}

#[cfg(test)]
mod pruebas_de_la_hora {
    #[test]
    fn la_nube_sabe_que_hora_es_al_minuto() {
        // 3-oct-2026 19:42 UTC en Lima (UTC-5).
        let t = 1_790_985_600_000 + (19 * 60 + 42) * 60_000;
        assert_eq!(
            super::nota_de_ahora(t, -5 * 3_600_000),
            "\n\n(Ahora: sáb 3-oct-2026 14:42, hora local del usuario.)"
        );
        assert_eq!(super::nota_de_ahora(0, 0), "", "sin reloj no se dice nada");
        // El desfase de este PC sale en cuartos de hora.
        assert_eq!(super::desfase_local_ms() % (15 * 60_000), 0);
    }
}
