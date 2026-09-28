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

/// Si el reconocedor que ya trae Windows (SAPI, sin red) sabe este idioma.
///
/// Hoy solo lo usa el dictado en vivo (clic derecho en el microfono): para
/// pasar notas a texto manda Whisper, que entiende mucho mejor (medido con
/// las notas del usuario el 2026-09-22), y las notas solo se pasan a texto
/// cuando el usuario lo pide. Lee el registro: barato, pero no para
/// llamarlo en cada fotograma.
pub fn windows_sabe(idioma: Idioma) -> bool {
    pixpin_voz::sapi::disponible(idioma_de_voz(idioma))
}

/// Si Whisper puede correr en este equipo: hay ONNX Runtime (el de Windows
/// o uno puesto a mano) y sabe el idioma. El modelo puede faltar aun: eso
/// es [`falta_el_modelo`]. Mira el disco, no carga nada.
pub fn whisper_posible(ubicacion: &Ubicacion, idioma: Idioma) -> bool {
    let exe = pixpin_shell::entorno::directorio_del_ejecutable().ok();
    pixpin_voz::whisper_posible(ubicacion.raiz(), exe.as_deref(), idioma_de_voz(idioma))
}

/// Si «Pasar a texto» tiene que **bajar antes el modelo de Whisper** (una
/// vez, unos 160 MB). El chat lo avisa y [`pasar_a_texto`] lo baja en su
/// hilo antes de transcribir.
pub fn falta_el_modelo(ubicacion: &Ubicacion, idioma: Idioma) -> bool {
    whisper_posible(ubicacion, idioma) && !pixpin_voz::whisper::modelo_listo(ubicacion.raiz())
}

/// **Baja el modelo de Whisper** a `<datos>/whisper/base/`, en el hilo de
/// quien llama (tarda: nunca desde el de la interfaz). `avance` de 0 a 1.
///
/// Ya bajado no hace nada: solo baja los ficheros que falten, asi que un
/// corte a medias se retoma donde quedo (el fichero cortado se baja entero
/// otra vez; los terminados no).
pub fn bajar_modelo(
    raiz: &Path,
    avance: &mut dyn FnMut(f32),
    cancelado: &AtomicBool,
) -> Result<(), ErrorVoz> {
    let t = std::time::Instant::now();
    let r = pixpin_voz::whisper::bajar_modelo(raiz, avance, cancelado);
    match &r {
        Ok(()) => tracing::info!(segundos = t.elapsed().as_secs(), "modelo de voz bajado"),
        Err(e) => tracing::warn!(?e, "no se pudo bajar el modelo de voz"),
    }
    r
}

/// Si hoy se puede transcribir con Vosk, y si no, que falta.
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
    /// Si el avance es el de bajar el modelo y no el de transcribir: la
    /// misma barra, con otra frase.
    bajando: Arc<AtomicBool>,
    cancelado: Arc<AtomicBool>,
    resultado: Receiver<Result<Transcripcion, ErrorVoz>>,
    terminada: bool,
}

impl EnMarcha {
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Si todavia esta bajando el modelo (la primera vez).
    pub fn bajando(&self) -> bool {
        self.bajando.load(Ordering::Relaxed)
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
pub fn pasar_a_texto(
    id: &str,
    audio: &Path,
    ubicacion: &Ubicacion,
    idioma: Idioma,
    turnos: Vec<pixpin_voz::turnos::Turno>,
    practicado: Option<String>,
) -> EnMarcha {
    let avance = Arc::new(AtomicU32::new(0));
    let cancelado = Arc::new(AtomicBool::new(false));
    // Se decide aqui y no en el hilo para que la primera vuelta del chat ya
    // pinte «bajando» y no un «pasando a texto 0 %» que no avanza.
    let bajar = falta_el_modelo(ubicacion, idioma);
    let bajando = Arc::new(AtomicBool::new(bajar));
    let (enviar, resultado) = channel();

    let hilo = {
        let audio = audio.to_path_buf();
        let raiz = ubicacion.raiz().to_path_buf();
        let exe = pixpin_shell::entorno::directorio_del_ejecutable().ok();
        // Los idiomas se leen aqui, al pedirlo: cambiar el segundo idioma en
        // Ajustes vale para la siguiente nota sin reabrir el chat. Lo que
        // se practica en Pronunciar manda sobre los dos.
        let idiomas = match practicado {
            Some(p) => pixpin_voz::Idiomas::uno(&p),
            None => idiomas_de(ubicacion, idioma),
        };
        let avance = Arc::clone(&avance);
        let bajando = Arc::clone(&bajando);
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
                // La primera vez: el modelo antes que la nota, con la misma
                // barra. Si no se puede bajar, ese es el fallo que se cuenta.
                if bajar {
                    if let Err(e) = bajar_modelo(&raiz, &mut avisar, &cancelado) {
                        let _ = enviar.send(Err(e));
                        return;
                    }
                    bajando.store(false, Ordering::Relaxed);
                    avisar(0.0);
                }
                // En modo cuidadoso, de una en una y sin coincidir con aligerar
                // un PDF: los dos juntos no caben en 4 GB (ver `turno_pesado`).
                let _turno = crate::turno_pesado::tomar("transcribir");
                let salida = pixpin_voz::transcribir_con(
                    &audio,
                    &raiz,
                    exe.as_deref(),
                    &idiomas,
                    &turnos,
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
        bajando,
        cancelado,
        resultado,
        terminada: false,
    }
}

/// **Los idiomas con los que se pasa a texto** (B6): el primero es el de la
/// interfaz ([`idioma_de_voz`]) y el segundo, el de Ajustes (`[voz]`), con
/// su modo. Sin ajustes legibles, solo el primero: una nota se pasa a texto
/// igual aunque el fichero de ajustes este roto.
pub fn idiomas_de(ubicacion: &Ubicacion, idioma: Idioma) -> pixpin_voz::Idiomas {
    let principal = idioma_de_voz(idioma);
    let Ok(a) = pixpin_store::cargar(ubicacion) else {
        return pixpin_voz::Idiomas::uno(principal);
    };
    idiomas_con(principal, &a.voz)
}

/// Lo mismo sin disco: el primero y la seccion `[voz]`.
pub fn idiomas_con(principal: &str, voz: &pixpin_store::Voz) -> pixpin_voz::Idiomas {
    let modo = match voz.modo_de_idiomas {
        pixpin_store::ModoDeIdiomas::CadaUno => pixpin_voz::ModoDeIdiomas::CadaUno,
        pixpin_store::ModoDeIdiomas::TodoEnUno => pixpin_voz::ModoDeIdiomas::TodoEnUno,
    };
    // Un segundo idioma que Whisper no sabe es «ninguno», no un error.
    let segundo = pixpin_voz::dos_idiomas::codigo(&voz.segundo_idioma, pixpin_voz::whisper::sabe)
        .unwrap_or_default();
    pixpin_voz::Idiomas::con(principal, &segundo, modo)
}

/// **Quien hablo cuando** en una nota, si es una conversacion (`turnos` del
/// movil). Con turnos, pasarla a texto saca los nombres.
pub fn turnos_de(m: &Mensaje) -> Vec<pixpin_voz::turnos::Turno> {
    m.resto
        .get("turnos")
        .map(pixpin_voz::turnos::de_json)
        .unwrap_or_default()
}

// --- Banderitas (`marcas`) y la letra a mano (B7) ------------------------
//
// `Mensaje.marcas: List<Int>` del movil (`LetraActivity`): milisegundos del
// audio, sin repetir y en orden. Viaja en `resto` como los demas campos que
// el `Mensaje` de Rust no declara. Vacia, **la clave se quita**: el resumen
// de sincronizacion es un hash del JSON canonico y el movil la cuenta como
// `[]` por omision, asi que las dos formas dan lo mismo alli, y aqui el
// mensaje vuelve a ser byte a byte el de antes.

/// El campo del movil.
pub const MARCAS: &str = "marcas";

/// Las banderitas de una nota, en orden.
pub fn marcas_de(m: &Mensaje) -> Vec<i64> {
    let mut v: Vec<i64> = m
        .resto
        .get(MARCAS)
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
        .unwrap_or_default();
    v.sort_unstable();
    v.dedup();
    v
}

fn escribir_marcas(m: &mut Mensaje, v: Vec<i64>) {
    if v.is_empty() {
        m.resto.remove(MARCAS);
    } else {
        m.resto.insert(
            MARCAS.into(),
            serde_json::Value::Array(v.into_iter().map(serde_json::Value::from).collect()),
        );
    }
}

/// Una banderita en `ms` (`(it.marcas + ms).distinct().sorted()`). Devuelve
/// si cambio algo: una segunda en el mismo milisegundo no es otra.
pub fn poner_marca(m: &mut Mensaje, ms: i64) -> bool {
    let mut v = marcas_de(m);
    if v.contains(&ms.max(0)) {
        return false;
    }
    v.push(ms.max(0));
    v.sort_unstable();
    escribir_marcas(m, v);
    true
}

/// Quitar la de `ms` (mantener pulsado en el movil; clic derecho aqui).
pub fn quitar_marca(m: &mut Mensaje, ms: i64) -> bool {
    let mut v = marcas_de(m);
    let antes = v.len();
    v.retain(|x| *x != ms);
    let cambio = v.len() != antes;
    if cambio {
        escribir_marcas(m, v);
    }
    cambio
}

/// **La letra escrita a mano** vuelve al mensaje, como el editor del movil
/// (`MarkdownEditorActivity`: `transcripcion = texto`, y `estadoDelTexto`
/// el que tuviera o `letra` si no tenia). Devuelve si cambio.
pub fn escribir_letra(m: &mut Mensaje, texto: &str) -> bool {
    let texto = texto.replace("\r\n", "\n");
    if m.transcripcion.as_deref() == Some(texto.as_str()) {
        return false;
    }
    m.transcripcion = Some(texto);
    if !m.resto.contains_key("estadoDelTexto") {
        m.resto.insert(
            "estadoDelTexto".into(),
            serde_json::Value::String(pixpin_voz::EstadoDelTexto::Letra.palabra().into()),
        );
    }
    true
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

// --- Leer lo que quedo escrito ---------------------------------------------
//
// La transcripcion se guarda como `[1:23] lo que se dijo`, un parrafo por
// trozo y un renglon en blanco entre ellos (`pixpin_voz::con_tiempos`). La
// burbuja y la pantalla de la letra la leen de vuelta partida en trozos:
// el minuto no se ensena dentro del texto —lo que se lee es lo que se
// dijo—, pero es lo que permite saltar al audio y saber por donde va.

/// Un trozo de la transcripcion: donde empieza en el audio y que dice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trozo {
    /// `None` si el parrafo no lleva minuto: una letra pegada a mano, o un
    /// texto de una version vieja. Ese no salta a ningun sitio.
    pub ms: Option<i64>,
    pub texto: String,
}

/// La transcripcion partida en trozos, con su minuto ya quitado del texto.
///
/// Es `trozosDeLaTranscripcion` del movil (`MensajesActivity.kt:6822`): se
/// parte por renglones en blanco, no por renglones, porque un parrafo de
/// veinte segundos puede venir partido en varias lineas si alguien lo edito.
/// Los parrafos vacios se tiran: un renglon en blanco de mas al final no es
/// un trozo al que saltar.
pub fn trozos(texto: &str) -> Vec<Trozo> {
    let texto = texto.replace("\r\n", "\n");
    let mut v = Vec::new();
    let mut bloque = String::new();
    // A mano y no con una expresion regular: `\n\s*\n` es «una linea que
    // solo tiene blancos», y eso se ve mirando cada linea.
    let cerrar = |bloque: &mut String, v: &mut Vec<Trozo>| {
        let limpio = bloque.trim();
        if !limpio.is_empty() {
            v.push(match pixpin_voz::tiempo_de(limpio) {
                Some((ms, resto)) => Trozo {
                    ms: Some(ms),
                    texto: resto.trim().to_string(),
                },
                None => Trozo {
                    ms: None,
                    texto: limpio.to_string(),
                },
            });
        }
        bloque.clear();
    };
    for linea in texto.split('\n') {
        if linea.trim().is_empty() {
            cerrar(&mut bloque, &mut v);
        } else {
            if !bloque.is_empty() {
                bloque.push('\n');
            }
            bloque.push_str(linea);
        }
    }
    cerrar(&mut bloque, &mut v);
    v
}

/// El trozo por el que va el audio: el ultimo cuyo minuto ya paso.
///
/// `None` antes del primer minuto o si ningun trozo lleva minuto: con eso
/// no se sabe por donde va, y resaltar uno al azar seria mentir.
pub fn por_donde_va(trozos: &[Trozo], posicion_ms: i64) -> Option<usize> {
    trozos
        .iter()
        .rposition(|t| t.ms.is_some_and(|ms| (0..=posicion_ms).contains(&ms)))
}

/// Como quedo el texto de esta nota, si lo dice (`estadoDelTexto`).
///
/// Viaja en `resto` porque el `Mensaje` de Rust no lo declara; ver
/// [`aplicar`]. Una palabra que no se conoce es `None`, no un fallo.
pub fn estado_del_texto(m: &Mensaje) -> Option<pixpin_voz::EstadoDelTexto> {
    m.resto
        .get("estadoDelTexto")
        .and_then(|v| v.as_str())
        .and_then(pixpin_voz::EstadoDelTexto::de)
}

/// La transcripcion si tiene algo que leer. Un campo con solo blancos es
/// lo mismo que ninguno: la burbuja ofrece pasarla a texto otra vez.
pub fn transcripcion_de(m: &Mensaje) -> Option<&str> {
    m.transcripcion
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// Lo que se ensena de un trozo en la pantalla de la letra: sin las marcas
/// de Markdown que no aportan leyendo (`#`, `**`, `__`) y sin las imagenes
/// (`![audio](...)`), como `sinMarcas` de `LetraActivity.kt`. Una letra
/// pegada a mano suele traerlas.
pub fn sin_marcas(texto: &str) -> String {
    texto
        .lines()
        .filter(|l| !l.trim_start().starts_with("!["))
        .map(|l| {
            l.trim_start_matches(['#', ' '])
                .replace("**", "")
                .replace("__", "")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_voz::tiempos::{EstadoDelTexto, Segmento};

    #[test]
    fn la_transcripcion_se_parte_en_trozos_con_su_minuto_fuera_del_texto() {
        let t = trozos("[0:00] llamar al aparejador\n\n[0:21] y pedir el presupuesto");
        assert_eq!(
            t,
            vec![
                Trozo {
                    ms: Some(0),
                    texto: "llamar al aparejador".into()
                },
                Trozo {
                    ms: Some(21_000),
                    texto: "y pedir el presupuesto".into()
                },
            ]
        );
    }

    #[test]
    fn un_parrafo_sin_minuto_se_queda_entero_y_no_salta() {
        let t = trozos("la letra pegada\nen dos renglones\n\n\n[1:05] con minuto\r\n\r\n  \n");
        assert_eq!(t.len(), 2, "los renglones en blanco de mas no son trozos");
        assert_eq!(t[0].ms, None);
        assert_eq!(t[0].texto, "la letra pegada\nen dos renglones");
        assert_eq!(t[1].ms, Some(65_000));
    }

    #[test]
    fn un_texto_vacio_no_da_ningun_trozo() {
        assert!(trozos("").is_empty());
        assert!(trozos(" \n\n \n").is_empty());
    }

    #[test]
    fn el_trozo_que_suena_es_el_ultimo_cuyo_minuto_ya_paso() {
        let t = trozos("[0:00] uno\n\n[0:20] dos\n\n[0:40] tres");
        assert_eq!(por_donde_va(&t, 0), Some(0));
        assert_eq!(por_donde_va(&t, 19_999), Some(0));
        assert_eq!(por_donde_va(&t, 20_000), Some(1));
        assert_eq!(por_donde_va(&t, 99_000), Some(2));
    }

    #[test]
    fn sin_minutos_no_se_sabe_por_donde_va() {
        let t = trozos("una letra\n\notra estrofa");
        assert_eq!(por_donde_va(&t, 30_000), None);
        let t = trozos("[0:10] empieza tarde");
        assert_eq!(por_donde_va(&t, 5_000), None, "antes del primer minuto");
    }

    #[test]
    fn el_estado_del_texto_se_lee_del_resto_y_una_palabra_rara_no_es_nada() {
        let mut m = Mensaje::default();
        assert_eq!(estado_del_texto(&m), None);
        m.resto.insert("estadoDelTexto".into(), "aviso".into());
        assert_eq!(estado_del_texto(&m), Some(EstadoDelTexto::Aviso));
        m.resto.insert("estadoDelTexto".into(), "futuro".into());
        assert_eq!(estado_del_texto(&m), None);
    }

    #[test]
    fn una_transcripcion_en_blanco_cuenta_como_ninguna() {
        let mut m = Mensaje::default();
        assert_eq!(transcripcion_de(&m), None);
        m.transcripcion = Some("  \n ".into());
        assert_eq!(transcripcion_de(&m), None);
        m.transcripcion = Some(" [0:00] hola ".into());
        assert_eq!(transcripcion_de(&m), Some("[0:00] hola"));
    }

    #[test]
    fn la_letra_se_lee_sin_las_marcas_de_markdown() {
        assert_eq!(
            sin_marcas("## Estribillo\n![audio](voz.m4a)\n**la la** __la__"),
            "Estribillo\nla la la"
        );
        assert_eq!(sin_marcas("texto llano"), "texto llano");
    }

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
    fn el_modelo_de_whisper_solo_falta_donde_whisper_puede_correr_y_no_esta_bajado() {
        let raiz = std::env::temp_dir().join(format!("pixpin-voz-falta-{}", std::process::id()));
        let u = Ubicacion::Portable { raiz: raiz.clone() };
        // Sin ONNX Runtime (un Windows 10 viejo) no hay nada que bajar: se
        // queda el reconocedor de Windows.
        assert_eq!(
            falta_el_modelo(&u, Idioma::Espanol),
            whisper_posible(&u, Idioma::Espanol)
        );
        let carpeta = pixpin_voz::whisper::carpeta(&raiz);
        std::fs::create_dir_all(&carpeta).unwrap();
        for (n, _) in pixpin_voz::whisper::FICHEROS {
            std::fs::write(carpeta.join(n), b"x").unwrap();
        }
        assert!(!falta_el_modelo(&u, Idioma::Espanol), "ya esta bajado");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn las_banderitas_van_en_orden_sin_repetir_en_el_campo_del_movil() {
        let mut m = Mensaje::default();
        assert!(marcas_de(&m).is_empty());
        assert!(poner_marca(&mut m, 9_000));
        assert!(poner_marca(&mut m, 2_500));
        assert!(!poner_marca(&mut m, 9_000), "la misma no es otra");
        assert_eq!(marcas_de(&m), vec![2_500, 9_000]);
        assert_eq!(m.resto.get(MARCAS), Some(&serde_json::json!([2_500, 9_000])));
    }

    #[test]
    fn quitar_la_ultima_banderita_deja_el_mensaje_como_estaba() {
        let mut m = Mensaje::default();
        poner_marca(&mut m, 1_000);
        assert!(!quitar_marca(&mut m, 5_000), "no estaba: nada cambia");
        assert!(quitar_marca(&mut m, 1_000));
        assert!(!m.resto.contains_key(MARCAS), "sin un [] colgando");
    }

    #[test]
    fn unas_marcas_rotas_del_otro_aparato_se_leen_sin_lo_que_no_es_numero() {
        let mut m = Mensaje::default();
        m.resto
            .insert(MARCAS.into(), serde_json::json!([3_000, "x", 1_000, 3_000]));
        assert_eq!(marcas_de(&m), vec![1_000, 3_000]);
        m.resto.insert(MARCAS.into(), serde_json::json!("nada"));
        assert!(marcas_de(&m).is_empty());
    }

    #[test]
    fn la_letra_a_mano_va_a_la_transcripcion_y_sin_estado_queda_como_letra() {
        let mut m = Mensaje::default();
        assert!(escribir_letra(&mut m, "la la\r\nla"));
        assert_eq!(m.transcripcion.as_deref(), Some("la la\nla"));
        assert_eq!(estado_del_texto(&m), Some(pixpin_voz::EstadoDelTexto::Letra));
        assert!(!escribir_letra(&mut m, "la la\nla"), "lo mismo no es un cambio");
    }

    #[test]
    fn corregir_una_transcripcion_no_le_cambia_el_estado() {
        let mut m = Mensaje::default();
        aplicar(&mut m, &transcripcion("[0:00] ola", EstadoDelTexto::Aviso));
        assert!(escribir_letra(&mut m, "[0:00] hola"));
        assert_eq!(estado_del_texto(&m), Some(EstadoDelTexto::Aviso));
    }

    #[test]
    fn los_turnos_de_una_conversacion_se_leen_del_mensaje() {
        let mut m = Mensaje::default();
        assert!(turnos_de(&m).is_empty());
        m.resto.insert(
            "turnos".into(),
            serde_json::json!([{"quien": "Ana", "desdeMs": 0, "hastaMs": 900}]),
        );
        assert_eq!(turnos_de(&m), vec![pixpin_voz::turnos::Turno::nuevo("Ana", 0, 900)]);
    }

    #[test]
    fn el_segundo_idioma_de_ajustes_llega_a_whisper_y_uno_raro_es_ninguno() {
        let voz = pixpin_store::Voz {
            segundo_idioma: "en".into(),
            modo_de_idiomas: pixpin_store::ModoDeIdiomas::TodoEnUno,
        };
        let i = idiomas_con("es", &voz);
        assert_eq!(i.otro.as_deref(), Some("en"));
        assert_eq!(i.modo, pixpin_voz::ModoDeIdiomas::TodoEnUno);
        let raro = pixpin_store::Voz {
            segundo_idioma: "klingon".into(),
            ..Default::default()
        };
        assert_eq!(idiomas_con("es", &raro).otro, None);
        assert_eq!(idiomas_con("es", &pixpin_store::Voz::default()).otro, None);
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
