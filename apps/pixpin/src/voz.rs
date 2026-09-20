//! **Pasar una nota de voz a texto, sin congelar la aplicacion.**
//!
//! El trabajo de verdad esta en `pixpin-voz`; aqui solo esta el hilo, el
//! aviso de avance y la escritura en el mensaje del cuaderno. Es el mismo
//! reparto que con la grabacion: el crate no sabe de ventanas y la ventana
//! no sabe de Vosk.
//!
//! ## Por que un hilo y no «un momentito»
//!
//! Una nota de tres minutos tarda del orden de diez segundos en pasar por
//! el reconocedor, y antes hay que decodificar el `.m4a` entero. Hacerlo en
//! el hilo de la interfaz deja el chat clavado —sin repintar, sin responder
//! al raton— justo cuando el usuario espera ver algo moverse. Asi que va en
//! su hilo, avisa del avance por un atomico y deja el resultado en un canal
//! que el chat recoge cuando le viene bien.
//!
//! ## Donde acaba el texto
//!
//! En `Mensaje.transcripcion` y en `resto["estadoDelTexto"]`, **los mismos
//! campos que escribe el movil** (`MensajesStore.kt:183-189`), para que el
//! texto viaje por la sincronizacion y la nota aparezca ya transcrita en el
//! telefono. Ver [`aplicar`].

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use pixpin_proyecto::cuaderno::Mensaje;
use pixpin_store::{Idioma, Ubicacion};
use pixpin_voz::{Disponibilidad, ErrorVoz, Transcripcion};

/// El idioma con el que se habla, en la etiqueta que espera `pixpin-voz`.
///
/// El movil lo saca de Ajustes y cae al idioma del telefono
/// (`Transcriptor.idiomaElegido`, `Transcriptor.kt:47-48`). Aqui todavia no
/// hay un ajuste propio de «idioma de voz», asi que se usa el de la
/// interfaz: quien tiene PixPin en castellano dicta en castellano. El dia
/// que haya ajuste, esta funcion es el unico sitio que cambia.
pub fn idioma_de_voz(idioma: Idioma) -> &'static str {
    match idioma {
        Idioma::Espanol => "es",
        Idioma::Ingles => "en",
    }
}

/// Si hoy se puede transcribir, y si no, que falta.
///
/// Es lo que mira el chat **antes** de ensenar «Pasar a texto»: con el
/// resultado se decide si la entrada se ofrece o si se ofrece con el aviso
/// de lo que hay que descargar.
pub fn disponibilidad(ubicacion: &Ubicacion, idioma: Idioma) -> Disponibilidad {
    let exe = pixpin_shell::entorno::directorio_del_ejecutable().ok();
    pixpin_voz::idiomas::disponibilidad(ubicacion.raiz(), exe.as_deref(), idioma_de_voz(idioma))
}

/// Una transcripcion en marcha.
///
/// El chat se la guarda mientras dure y la mira en cada vuelta de su bucle:
/// [`EnMarcha::avance`] para pintar la barra y [`EnMarcha::recoger`] para
/// ver si ya termino. Tirarla cancela el trabajo.
pub struct EnMarcha {
    /// El `id` del mensaje que se esta transcribiendo: el chat tiene que
    /// saber a que burbuja pertenece el avance.
    id: String,
    /// Milesimas, de 0 a 1000. Atomico porque lo escribe el hilo de fondo y
    /// lo lee el de la interfaz sesenta veces por segundo; un cerrojo aqui
    /// seria un cerrojo en mitad del repintado.
    avance: Arc<AtomicU32>,
    cancelado: Arc<AtomicBool>,
    resultado: Receiver<Result<Transcripcion, ErrorVoz>>,
    terminada: bool,
}

impl EnMarcha {
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Por donde va, de 0 a 1.
    pub fn avance(&self) -> f32 {
        self.avance.load(Ordering::Relaxed) as f32 / 1000.0
    }

    /// Dice que pare. El hilo lo mira cada cuarto de segundo de audio, asi
    /// que muere casi en el acto en vez de moler cinco minutos para nada.
    pub fn cancelar(&self) {
        self.cancelado.store(true, Ordering::Relaxed);
    }

    /// El resultado, si ya esta. **No bloquea**: se llama desde el bucle de
    /// la ventana. Devuelve `Some` una sola vez.
    pub fn recoger(&mut self) -> Option<Result<Transcripcion, ErrorVoz>> {
        if self.terminada {
            return None;
        }
        match self.resultado.try_recv() {
            Ok(r) => {
                self.terminada = true;
                Some(r)
            }
            Err(TryRecvError::Empty) => None,
            // El hilo murio sin contestar (un panico dentro de Vosk). Se
            // cuenta como cancelada y no como «no se entiende nada»: no es
            // culpa del audio.
            Err(TryRecvError::Disconnected) => {
                self.terminada = true;
                Some(Err(ErrorVoz::Cancelada))
            }
        }
    }

    /// Si ya se recogio el resultado y esto se puede tirar.
    pub fn acabada(&self) -> bool {
        self.terminada
    }
}

impl Drop for EnMarcha {
    fn drop(&mut self) {
        // Cerrar el chat con una transcripcion a medias no puede dejar un
        // hilo comiendo procesador hasta que se cierre la aplicacion.
        self.cancelar();
    }
}

/// **Arranca la transcripcion de una nota de voz.**
///
/// Vuelve en el acto: el trabajo va en su hilo. `audio` es el fichero del
/// mensaje, y `id` el del mensaje, que se devuelve tal cual para que el
/// chat sepa donde poner el texto cuando acabe.
pub fn pasar_a_texto(id: &str, audio: &Path, ubicacion: &Ubicacion, idioma: Idioma) -> EnMarcha {
    let avance = Arc::new(AtomicU32::new(0));
    let cancelado = Arc::new(AtomicBool::new(false));
    let (enviar, resultado) = channel();

    let hilo = {
        let audio = audio.to_path_buf();
        let raiz = ubicacion.raiz().to_path_buf();
        let exe = pixpin_shell::entorno::directorio_del_ejecutable().ok();
        let lengua = idioma_de_voz(idioma);
        let avance = Arc::clone(&avance);
        let cancelado = Arc::clone(&cancelado);
        std::thread::Builder::new()
            .name("pixpin-transcribir".into())
            .spawn(move || {
                // COM por hilo: Media Foundation decodifica el audio aqui
                // dentro, igual que en el hilo del visor o el del chat.
                let _com = pixpin_shell::ComDelHilo::iniciar();
                let mut avisar = |f: f32| {
                    avance.store((f.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::Relaxed);
                };
                let salida = pixpin_voz::transcribir(
                    &audio,
                    &raiz,
                    exe.as_deref(),
                    lengua,
                    &mut avisar,
                    &cancelado,
                );
                if let Err(e) = &salida {
                    tracing::info!(?e, audio = %audio.display(), "no se pudo pasar a texto");
                }
                // Si el chat ya se cerro, el canal esta roto y no pasa nada.
                let _ = enviar.send(salida);
            })
    };

    if let Err(e) = hilo {
        tracing::warn!(?e, "no se pudo lanzar el hilo de transcribir");
    }

    EnMarcha {
        id: id.to_string(),
        avance,
        cancelado,
        resultado,
        terminada: false,
    }
}

/// **Escribe el texto en el mensaje**, en los campos del movil.
///
/// `transcripcion` esta declarado en el `Mensaje` de Rust; `estadoDelTexto`
/// no, asi que va en `resto`, que es por donde viajan los campos que solo
/// conoce Android (igual que `picos`). No se toca nada mas del mensaje:
/// quien guarda el cuaderno es el chat.
pub fn aplicar(mensaje: &mut Mensaje, t: &Transcripcion) {
    mensaje.transcripcion = Some(t.texto.clone());
    mensaje.resto.insert(
        "estadoDelTexto".into(),
        serde_json::Value::String(t.estado.palabra().to_string()),
    );
}

/// La carpeta donde se guardan los audios que nacen aqui.
///
/// Misma idea que el `filesDir/voz` del movil (`pin/Voz.kt:35-36`): al lado
/// de los datos, no en la galeria ni en «Mis documentos».
pub fn carpeta_de_audios(ubicacion: &Ubicacion) -> PathBuf {
    ubicacion.raiz().join("voz")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_voz::tiempos::{EstadoDelTexto, Segmento};

    fn transcripcion(texto: &str, estado: EstadoDelTexto) -> Transcripcion {
        Transcripcion {
            texto: texto.to_string(),
            segmentos: vec![Segmento::nuevo(0, texto)],
            estado,
            duracion_ms: 4_200,
        }
    }

    #[test]
    fn el_texto_va_al_campo_que_viaja_por_la_sincronizacion() {
        let mut m = Mensaje::default();
        aplicar(
            &mut m,
            &transcripcion("[0:00] llamar al aparejador", EstadoDelTexto::Bien),
        );
        assert_eq!(
            m.transcripcion.as_deref(),
            Some("[0:00] llamar al aparejador")
        );
        assert_eq!(
            m.resto.get("estadoDelTexto").and_then(|v| v.as_str()),
            Some("bien")
        );
    }

    #[test]
    fn transcribir_otra_vez_pisa_el_texto_viejo_y_su_estado() {
        let mut m = Mensaje::default();
        aplicar(&mut m, &transcripcion("primero", EstadoDelTexto::Aviso));
        aplicar(&mut m, &transcripcion("segundo", EstadoDelTexto::Bien));
        assert_eq!(m.transcripcion.as_deref(), Some("segundo"));
        assert_eq!(
            m.resto.get("estadoDelTexto").and_then(|v| v.as_str()),
            Some("bien")
        );
    }

    #[test]
    fn aplicar_no_toca_nada_mas_del_mensaje() {
        let mut m = Mensaje {
            id: "m1".into(),
            texto: "lo que habia escrito".into(),
            duracion_ms: 9_000,
            ..Mensaje::default()
        };
        aplicar(&mut m, &transcripcion("nuevo", EstadoDelTexto::Bien));
        assert_eq!(m.id, "m1");
        assert_eq!(m.texto, "lo que habia escrito");
        assert_eq!(m.duracion_ms, 9_000, "la duracion la puso quien grabo");
    }

    #[test]
    fn el_idioma_de_la_interfaz_decide_el_del_reconocedor() {
        assert_eq!(idioma_de_voz(Idioma::Espanol), "es");
        assert_eq!(idioma_de_voz(Idioma::Ingles), "en");
    }

    #[test]
    fn los_audios_propios_se_guardan_junto_a_los_datos() {
        let u = Ubicacion::Portable {
            raiz: PathBuf::from("C:/portatil/pixpin"),
        };
        assert_eq!(
            carpeta_de_audios(&u),
            PathBuf::from("C:/portatil/pixpin/voz")
        );
    }
}
