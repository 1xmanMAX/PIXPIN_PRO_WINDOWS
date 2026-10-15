//! **Notas de voz**: guardar una grabada en el Mac y escribir su texto.
//!
//! No se inventa nada: es lo que hace Windows, con los campos del movil.
//!
//! - El audio es un `.m4a` (MPEG-4 con AAC mono, como `pixpin-audio` y el
//!   `MediaRecorder` del movil) que graba Swift con AVFoundation. Aqui entra
//!   con [`Nucleo::adjuntar_voz`]: se copia como cualquier adjunto
//!   (`guardar_adjunto`, clase `VOZ`) con el nombre `voz_<hora>.m4a` de
//!   Windows (`ventana_chat::nombre_de_la_voz`), y la misma linea lleva
//!   `duracionMs` y `picos` (de 0 a 32767, como mucho 256: el contrato de
//!   `pixpin_audio::picos`). Windows escribe la linea y luego la reescribe
//!   con eso; aqui se escribe una sola vez.
//! - El texto lo saca Swift (`SFSpeechRecognizer` en el aparato) y lo
//!   guarda [`Nucleo::poner_transcripcion`] igual que `voz::aplicar` de
//!   Windows: `transcripcion` con el formato `[1:23] lo dicho` (parrafos de
//!   hasta 20 s separados por un renglon en blanco,
//!   `pixpin_voz::tiempos::con_tiempos`) y `estadoDelTexto` en `resto`.
//!
//! `pixpin-voz` sigue en `por-adaptar/` (SAPI, ONNX), asi que lo poco que
//! hace falta de el (`con_tiempos`, `marca_de_tiempo`, `estado_de`) esta
//! copiado aqui **al caracter**: el campo viaja por la sincronizacion y el
//! movil lo lee para saltar al minuto. Cuando `pixpin-voz` compile fuera de
//! Windows, se usa el suyo y se borra esta copia.

use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje};

use crate::chats::validar_chat;
use crate::{ErrorNucleo, MensajeChat, Nucleo};

/// Cuanto abarca un parrafo del texto con tiempos, como mucho
/// (`pixpin_voz::tiempos::PARRAFO_MS`, `Transcriptor.kt:100`).
pub const PARRAFO_MS: i64 = 20_000;

/// Cuantos picos se guardan por nota, como mucho
/// (`pixpin_audio::PICOS_GUARDADOS`, `Onda.kt:96`).
pub const PICOS_GUARDADOS: usize = 256;

/// Lo mas alto que puede valer un pico: el de una muestra de 16 bits
/// (`pixpin_audio::PICO_MAXIMO`).
pub const PICO_MAXIMO: i32 = 32767;

/// Un trozo de lo reconocido y en que milisegundo del audio empieza (el
/// `Segmento` de `pixpin-voz`; Swift lo saca de `SFTranscriptionSegment`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SegmentoVoz {
    pub desde_ms: i64,
    pub texto: String,
}

/// `1:23`, `12:05`, `1:02:09`: sin cero delante en la primera cifra, como
/// `marcaDeTiempo` del movil. Un negativo cuenta como cero.
pub fn marca_de_tiempo(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let seg = s % 60;
    if h > 0 {
        format!("{h}:{m:02}:{seg:02}")
    } else {
        format!("{m}:{seg:02}")
    }
}

/// El texto con sus tiempos: una linea por parrafo de hasta [`PARRAFO_MS`],
/// separadas por un renglon en blanco. Copia de
/// `pixpin_voz::tiempos::con_tiempos` con el prefijo vacio (una nota normal).
pub fn con_tiempos(segmentos: &[SegmentoVoz]) -> String {
    let mut lineas: Vec<String> = Vec::new();
    let mut desde: i64 = -1;
    let mut trozo = String::new();
    for sg in segmentos {
        if desde >= 0 && sg.desde_ms - desde >= PARRAFO_MS {
            lineas.push(format!("[{}] {}", marca_de_tiempo(desde), trozo.trim()));
            trozo.clear();
            desde = -1;
        }
        if desde < 0 {
            desde = sg.desde_ms;
        }
        trozo.push_str(&sg.texto);
        trozo.push(' ');
    }
    if !trozo.trim().is_empty() {
        lineas.push(format!(
            "[{}] {}",
            marca_de_tiempo(desde.max(0)),
            trozo.trim()
        ));
    }
    lineas.join("\n\n")
}

/// La palabra de `estadoDelTexto` para un texto reconocido: `bien` si hay
/// algo aprovechable y `mal` si no (`pixpin_voz::estado_de` sin avisos: el
/// reconocedor de Apple no marca trozos sin entender).
pub fn estado_de(segmentos: &[SegmentoVoz]) -> &'static str {
    if segmentos.iter().any(|s| !s.texto.trim().is_empty()) {
        "bien"
    } else {
        "mal"
    }
}

/// Junta los picos de dos en dos promediando (`pixpin_audio::picos::a_mitad`).
fn a_mitad(picos: &[i32]) -> Vec<i32> {
    picos
        .chunks(2)
        .map(|par| match par {
            [a, b] => (a + b) / 2,
            [a] => *a,
            _ => 0,
        })
        .collect()
}

/// Los picos como los guarda el movil: cada uno de 0 a [`PICO_MAXIMO`] y
/// como mucho [`PICOS_GUARDADOS`] (se juntan de dos en dos hasta caber,
/// conservando la forma entera de la nota).
pub fn acotar_picos(picos: &[i32]) -> Vec<i32> {
    let mut v: Vec<i32> = picos.iter().map(|p| (*p).clamp(0, PICO_MAXIMO)).collect();
    while v.len() > PICOS_GUARDADOS {
        v = a_mitad(&v);
    }
    v
}

/// Los picos que trae un mensaje en `resto["picos"]`, ya acotados. Lo que
/// no es un numero se salta.
pub fn picos_de(m: &Mensaje) -> Vec<i32> {
    let Some(lista) = m.resto.get("picos").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let crudos: Vec<i32> = lista
        .iter()
        .filter_map(|v| v.as_i64())
        .map(|n| n.clamp(0, i64::from(PICO_MAXIMO)) as i32)
        .collect();
    acotar_picos(&crudos)
}

/// El nombre de una nota de voz nueva sin nombre puesto: `voz_<ms>.m4a`,
/// como `ventana_chat::nombre_de_la_voz(None, ahora)` de Windows.
pub fn nombre_de_la_voz(ahora_ms: i64) -> String {
    format!("voz_{ahora_ms}.m4a")
}

#[uniffi::export]
impl Nucleo {
    /// **Guarda una nota de voz grabada aqui** en un chat: copia el `.m4a`
    /// de `ruta_origen` dentro del proyecto como `voz_<hora>.m4a` (clase
    /// `VOZ`) y escribe su linea con `duracionMs` y `picos`, como Windows
    /// (`guardar_la_voz`). Bloqueante: Swift lo llama fuera del hilo
    /// principal. El fichero de origen no se toca (lo borra quien lo grabo).
    pub fn adjuntar_voz(
        &self,
        chat: String,
        ruta_origen: String,
        duracion_ms: i64,
        picos: Vec<i32>,
    ) -> Result<MensajeChat, ErrorNucleo> {
        let picos = acotar_picos(&picos);
        let nombre = nombre_de_la_voz(crate::ahora_ms());
        self.adjuntar_con(chat, ruta_origen, Some(nombre), move |m| {
            m.duracion_ms = duracion_ms.max(0);
            m.resto
                .insert("picos".into(), serde_json::Value::from(picos));
        })
    }

    /// **Escribe el texto de una nota de voz** (`mensaje` es su id) en su
    /// linea del cuaderno: `transcripcion` con sus minutos y
    /// `estadoDelTexto`, igual que `voz::aplicar` de Windows. Lo demas de la
    /// linea (tambien lo que solo entiende el movil) queda como estaba.
    /// Devuelve el mensaje como quedo. Bloqueante (reescribe el cuaderno).
    pub fn poner_transcripcion(
        &self,
        chat: String,
        mensaje: String,
        segmentos: Vec<SegmentoVoz>,
    ) -> Result<MensajeChat, ErrorNucleo> {
        validar_chat(&chat)?;
        let carpeta = almacen::carpeta(self.ruta(), &chat);
        let fichero = carpeta.join("guardados.jsonl");
        if !fichero.exists() {
            return Err(ErrorNucleo::NoExiste { que: chat });
        }
        let texto = con_tiempos(&segmentos);
        let estado = estado_de(&segmentos);
        let mut no_es_voz = false;
        // La cache tomada mientras se reescribe, como al escribir: nadie
        // saca un numero de un indice que esta a punto de cambiar.
        let mut cache = self.cache_tomada();
        let hecho = cuaderno::cambiar(&carpeta, &mensaje, |m| {
            if m.clase != Some(Clase::Voz) {
                no_es_voz = true;
                return false;
            }
            m.transcripcion = Some(texto);
            m.resto.insert(
                "estadoDelTexto".into(),
                serde_json::Value::String(estado.to_string()),
            );
            true
        })?;
        cache.quitar(&fichero);
        drop(cache);
        match hecho {
            None => Err(ErrorNucleo::NoExiste { que: mensaje }),
            Some(_) if no_es_voz => Err(ErrorNucleo::Argumento {
                detalle: format!("no es una nota de voz: {mensaje}"),
            }),
            Some(m) => Ok(MensajeChat::from(m)),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn sg(ms: i64, t: &str) -> SegmentoVoz {
        SegmentoVoz {
            desde_ms: ms,
            texto: t.into(),
        }
    }

    #[test]
    fn la_marca_de_tiempo_es_la_del_movil() {
        assert_eq!(marca_de_tiempo(0), "0:00");
        assert_eq!(marca_de_tiempo(83_000), "1:23");
        assert_eq!(marca_de_tiempo(725_999), "12:05");
        assert_eq!(marca_de_tiempo(3_729_000), "1:02:09");
        assert_eq!(marca_de_tiempo(-5), "0:00");
    }

    #[test]
    fn los_segmentos_se_juntan_en_parrafos_de_veinte_segundos() {
        let t = con_tiempos(&[
            sg(0, "llamar"),
            sg(900, "al aparejador"),
            sg(21_000, "y pedir"),
            sg(22_000, "los niveles"),
        ]);
        assert_eq!(
            t,
            "[0:00] llamar al aparejador\n\n[0:21] y pedir los niveles"
        );
        assert_eq!(con_tiempos(&[]), "");
        assert_eq!(con_tiempos(&[sg(0, "  ")]), "");
    }

    #[test]
    fn el_estado_es_bien_con_texto_y_mal_sin_el() {
        assert_eq!(estado_de(&[sg(0, "hola")]), "bien");
        assert_eq!(estado_de(&[sg(0, " ")]), "mal");
        assert_eq!(estado_de(&[]), "mal");
    }

    #[test]
    fn los_picos_se_acotan_al_rango_y_al_tope() {
        assert_eq!(acotar_picos(&[-3, 10, 40_000]), vec![0, 10, PICO_MAXIMO]);
        let muchos: Vec<i32> = (0..1000).map(|i| i % 300).collect();
        let v = acotar_picos(&muchos);
        assert!(v.len() <= PICOS_GUARDADOS && v.len() > PICOS_GUARDADOS / 2);
        assert_eq!(a_mitad(&[2, 4, 7]), vec![3, 7]);
    }

    #[test]
    fn el_nombre_es_el_de_windows() {
        assert_eq!(nombre_de_la_voz(1_725_500_001_000), "voz_1725500001000.m4a");
        assert_eq!(cuaderno::clase_de_nombre(&nombre_de_la_voz(1)), Clase::Voz);
    }
}
