//! **Lo que entra desde Windows sin pasar por PixPin** (15-oct-2026):
//!
//! - **«Imprimir a PixPin»**: la impresora «PixPin» deja el PDF en
//!   `<raiz>\cache\impresora\`; en cuanto esta entero se mueve a
//!   `<raiz>\impresos\` y se **abre en el lector** como pin
//!   (`pixpin_shell::impresora`). Como el «Compartir con PixPin» de Android
//!   (`ShareReceiverActivity`: un archivo recibido sale como pin que se abre),
//!   y el lector ya tiene buscar, anotar, OCR, leer en voz alta y mandarlo al
//!   chat. Si la impresora no esta instalada (no hay carpeta), no se arranca
//!   nada.
//! - **Compartir de Windows y el menu contextual** (paquete de identidad):
//!   lo compartido va al chat «Mensajes guardados» (`pixpin_shell::compartido`).
//!
//! Los dos son adaptadores finos hacia el protocolo de pedidos (`abrir`,
//! `chat`), el mismo camino que Flow y la Jump List.

use pixpin_shell::{compartido, impresora};
use pixpin_store::Ubicacion;

/// El pedido de lo que se compartio con este arranque (`--al-chat <rutas>`
/// o la activacion por Compartir), si lo hay. Sin identidad ni la orden, no
/// mira nada mas: cuesta lo que leer los argumentos.
pub fn pedido_del_arranque() -> Option<String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(r) = compartido::de_argumentos(&args) {
        return Some(r.pedido());
    }
    if !compartido::tiene_identidad() {
        return None;
    }
    // Aun no se sabe donde vive PixPin (esto va antes de la instancia
    // unica): se resuelve igual que en el arranque.
    let dir_exe = pixpin_shell::entorno::directorio_del_ejecutable().ok()?;
    let appdata = pixpin_shell::entorno::appdata().ok()?;
    let ubicacion = pixpin_store::rutas::resolver(&dir_exe, &appdata);
    let destino = ubicacion
        .raiz()
        .join("cache")
        .join("compartido")
        .join(pixpin_shell::entorno::ahora_utc_ms().to_string());
    let r = compartido::leer_activacion(&destino)?;
    tracing::info!(
        imagenes = r.imagenes.len(),
        archivos = r.archivos.len(),
        "recibido por Compartir"
    );
    Some(r.pedido())
}

/// El pedido de abrir un impreso en el lector.
fn pedido_abrir(ruta: &std::path::Path) -> String {
    serde_json::json!({
        "pixpin": 1,
        "accion": "abrir",
        "que": { "tipo": "fichero", "ruta": ruta },
    })
    .to_string()
}

/// Vigila la carpeta de la impresora «PixPin», si esta instalada.
pub fn iniciar_impresora(ubicacion: &Ubicacion) {
    let raiz = ubicacion.raiz();
    let vigilando = impresora::vigilar(
        impresora::carpeta(raiz),
        impresora::carpeta_impresos(raiz),
        pixpin_shell::entorno::ahora_local_ms,
        |ruta| {
            pixpin_shell::mensajero::enviar_pedido(&pedido_abrir(&ruta));
        },
    );
    if vigilando.is_some() {
        tracing::info!("impresora «PixPin»: vigilando su carpeta");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El pedido que manda la impresora es un `abrir` de fichero que el
    /// protocolo entiende (con la ruta tal cual, espacios y tildes).
    #[test]
    fn el_impreso_se_abre_con_el_pedido_de_siempre() {
        let ruta = std::path::Path::new(
            r"C:\Users\yo\AppData\Roaming\PixPinMax\impresos\Impreso 2026-10-15 14.03.22.pdf",
        );
        let p = pedido_abrir(ruta);
        let v: serde_json::Value = serde_json::from_str(&p).unwrap();
        assert_eq!(v["accion"], "abrir");
        assert_eq!(v["que"]["tipo"], "fichero");
        assert_eq!(v["que"]["ruta"].as_str().unwrap(), ruta.to_str().unwrap());
        assert!(
            crate::pedidos::leer(&p).is_ok(),
            "el protocolo no lo entiende: {p}"
        );
    }

    /// Lo compartido llega como un `chat` que el protocolo entiende, y sin
    /// la orden ni identidad el arranque no trae nada.
    #[test]
    fn lo_compartido_es_un_chat_valido() {
        let d = std::env::temp_dir().join(format!("pixpin-recibir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let pdf = d.join("plano.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        let r = compartido::juntar(
            Some("de WhatsApp".into()),
            None,
            None,
            &[pdf],
            &d.join("dest"),
        )
        .unwrap();
        assert!(crate::pedidos::leer(&r.pedido()).is_ok());
        // Las pruebas no se lanzan con `--al-chat` ni empaquetadas.
        assert!(pedido_del_arranque().is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
