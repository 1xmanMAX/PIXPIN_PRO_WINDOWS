//! **El asistente**, visto desde Swift.
//!
//! El orden de motores lo decide la app (`Asistente/` en Swift):
//!
//! 1. **Foundation Models** (macOS 26, el modelo del sistema). No pasa por
//!    Rust: Swift declara las herramientas con el JSON de
//!    [`ia_catalogo_para`] y, cuando el modelo llama una, pasa el nombre y
//!    los argumentos a [`ia_llamar`] (validar con las reglas de siempre y,
//!    si dibuja, los elementos de Excalidraw). Una llamada por herramienta,
//!    no una por figura (§4bis del plan).
//! 2. **Un `llama-server` del usuario** escuchando en `127.0.0.1` (por
//!    omision el 8080, [`PUERTO_LLAMA`]): [`llama_local_activo`] lo mira y
//!    [`llama_local_preguntar`] hace un turno entero con el bucle de
//!    `pixpin_ia::agente` y el cliente HTTP de `pixpin_ia::servidor`. Sin
//!    streaming: la respuesta llega entera.
//! 3. Si no hay ninguno, Swift dice que no hay motor.
//!
//! Todo es bloqueante y sin hilos propios: Swift lo llama desde una `Task`
//! con prioridad `.utility`.

use pixpin_ia::agente::{self, Ejecutor};
use pixpin_ia::herramientas::Llamada;
use pixpin_ia::motor::{Mensaje, Motor, Peticion, Respuesta, Rol, Seguir, Trozo};
use pixpin_ia::{ErrorIa, Evento};
use serde_json::Value;

use crate::ErrorNucleo;

/// El puerto de fabrica de `llama-server` (`--port 8080`).
pub const PUERTO_LLAMA: u16 = 8080;

/// Un mensaje de la conversacion que guarda Swift, para la vuelta
/// siguiente. Solo texto: las llamadas a herramientas de turnos pasados no
/// se vuelven a mandar (un modelo pequeno se lia con ellas).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MensajeIa {
    /// `true` si lo escribio el usuario; `false`, el asistente.
    pub del_usuario: bool,
    pub texto: String,
}

/// Una herramienta que se uso en el turno.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HerramientaUsada {
    pub nombre: String,
    pub ok: bool,
    /// Lo que se le devolvio al modelo («dibujadas 4 figuras», el motivo).
    pub resumen: String,
}

/// Lo que contesto el motor local en un turno.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RespuestaIa {
    pub texto: String,
    pub herramientas: Vec<HerramientaUsada>,
    /// Lo dibujado en el turno, como un `.excalidraw` entero (el formato
    /// que lee el lienzo), o nada si no se dibujo.
    pub lienzo: Option<String>,
}

/// Las instrucciones de sistema del asistente (las mismas que en Windows).
#[uniffi::export]
pub fn ia_instrucciones() -> String {
    agente::instrucciones()
}

/// Todas las herramientas, como la lista `tools` de OpenAI (JSON).
#[uniffi::export]
pub fn ia_catalogo() -> String {
    pixpin_ia::puente::catalogo()
}

/// Las pocas herramientas que vienen a cuento para `pedido` (JSON, lista
/// `tools` de OpenAI): un modelo pequeno se pierde con el catalogo entero.
#[uniffi::export]
pub fn ia_catalogo_para(pedido: String, con_imagen: bool) -> String {
    pixpin_ia::puente::catalogo_para(&pedido, con_imagen)
}

/// Una llamada del modelo, de cualquier motor (ver
/// `pixpin_ia::puente::llamar`): devuelve el objeto JSON con `ok`,
/// `elementos`, `argumentos`... o `para_el_modelo` si estaba mal.
#[uniffi::export]
pub fn ia_llamar(
    nombre: String,
    argumentos: String,
    origen_x: f64,
    origen_y: f64,
    semilla: u64,
) -> String {
    pixpin_ia::puente::llamar(&nombre, &argumentos, (origen_x, origen_y), semilla)
}

/// Un `.excalidraw` entero con los `elementos` (texto JSON de una lista,
/// los que van juntando las llamadas de [`ia_llamar`]). Lo que no es una
/// lista da un lienzo vacio.
#[uniffi::export]
pub fn ia_lienzo_de(elementos: String) -> String {
    let lista = match serde_json::from_str::<Value>(&elementos) {
        Ok(Value::Array(v)) => v,
        _ => Vec::new(),
    };
    pixpin_ia::lienzo::documento(lista)
}

/// Si hay un `llama-server` con el modelo cargado en `127.0.0.1:puerto`
/// (`GET /health` da 200). Tarda como mucho unos 300 ms si no hay nadie.
#[uniffi::export]
pub fn llama_local_activo(puerto: u16) -> bool {
    pixpin_ia::servidor::salud(puerto)
}

/// **Un turno con el `llama-server` del usuario**: `historial` es lo dicho
/// antes (del mas antiguo al mas nuevo) y `pedido` lo nuevo. Usa las
/// herramientas que vienen a cuento; las que dibujan se juntan en
/// [`RespuestaIa::lienzo`] y las que la app del Mac aun no sabe hacer
/// vuelven al modelo como fallo, para que lo diga.
#[uniffi::export]
pub fn llama_local_preguntar(
    puerto: u16,
    historial: Vec<MensajeIa>,
    pedido: String,
) -> Result<RespuestaIa, ErrorNucleo> {
    let pedido = pedido.trim().to_string();
    if pedido.is_empty() {
        return Err(ErrorNucleo::Argumento {
            detalle: "el pedido esta vacio".into(),
        });
    }
    let motor = LlamaDelUsuario { puerto };
    turno_con_motor(&motor, historial, &pedido)
}

/// El turno con cualquier motor (las pruebas usan uno guionado).
pub(crate) fn turno_con_motor(
    motor: &dyn Motor,
    historial: Vec<MensajeIa>,
    pedido: &str,
) -> Result<RespuestaIa, ErrorNucleo> {
    let mut mensajes = Vec::with_capacity(historial.len() + 2);
    mensajes.push(Mensaje::sistema(agente::instrucciones()));
    for m in historial {
        if m.texto.trim().is_empty() {
            continue;
        }
        let rol = if m.del_usuario {
            Rol::Usuario
        } else {
            Rol::Asistente
        };
        mensajes.push(Mensaje::texto(rol, m.texto));
    }
    mensajes.push(Mensaje::usuario(pedido));
    let herramientas = pixpin_ia::seleccion::para_el_pedido(pedido, false);
    let mut ejecutor = EjecutorMac::default();
    let mut usadas: Vec<HerramientaUsada> = Vec::new();
    let mut al_evento = |e: Evento| {
        if let Evento::Resultado { nombre, ok, texto } = e {
            usadas.push(HerramientaUsada {
                nombre,
                ok,
                resumen: texto,
            });
        }
        Seguir::Si
    };
    let fin = agente::turno(
        motor,
        &mut mensajes,
        &herramientas,
        &mut ejecutor,
        &mut al_evento,
    )
    .map_err(|e| ErrorNucleo::Asistente {
        detalle: agente::para_el_usuario(&e),
    })?;
    let lienzo = (!ejecutor.elementos.is_empty())
        .then(|| pixpin_ia::lienzo::documento(std::mem::take(&mut ejecutor.elementos)));
    Ok(RespuestaIa {
        texto: fin.texto,
        herramientas: usadas,
        lienzo,
    })
}

/// El `llama-server` que arranco el usuario (no PixPin): no se lanza ni se
/// mata, solo se le habla.
struct LlamaDelUsuario {
    puerto: u16,
}

impl Motor for LlamaDelUsuario {
    fn completar(
        &self,
        peticion: &Peticion,
        al_trozo: &mut dyn FnMut(&Trozo) -> Seguir,
    ) -> Result<Respuesta, ErrorIa> {
        // Sin `--api-key` el servidor no mira la clave.
        pixpin_ia::servidor::pedir(self.puerto, "pixpin", peticion, al_trozo)
    }
}

/// Lo que el Mac ya sabe hacer de verdad: dibujar (juntando los elementos
/// para el lienzo) y contestar sobre la app. Lo demas vuelve al modelo como
/// fallo.
#[derive(Default)]
struct EjecutorMac {
    elementos: Vec<Value>,
    semilla: u64,
}

impl Ejecutor for EjecutorMac {
    fn ejecutar(&mut self, llamada: &Llamada) -> Result<String, String> {
        if llamada.dibuja() {
            self.semilla += 1;
            // Cada dibujo, debajo del anterior, para que no se pisen.
            let y = self
                .elementos
                .iter()
                .filter_map(|e| Some(e["y"].as_f64()? + e["height"].as_f64().unwrap_or(0.0)))
                .fold(0.0_f64, f64::max);
            let origen = (
                0.0,
                if self.elementos.is_empty() {
                    0.0
                } else {
                    y + 40.0
                },
            );
            let nuevos = pixpin_ia::lienzo::elementos(llamada, origen, self.semilla);
            let n = nuevos.len();
            self.elementos.extend(nuevos);
            return Ok(format!("dibujadas {n} figuras"));
        }
        match llamada {
            Llamada::Ayuda(pregunta) => Ok(pixpin_ia::conocimiento::responder(pregunta)),
            otra => Err(format!(
                "{} aun no se puede hacer en PixPin para Mac",
                otra.nombre()
            )),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_ia::herramientas;
    use pixpin_ia::motor::MotorGuionado;

    #[test]
    fn el_catalogo_y_la_llamada_cruzan_como_texto_json() {
        let v: Value =
            serde_json::from_str(&ia_catalogo_para("dibuja un circulo".into(), false)).unwrap();
        assert!(!v.as_array().unwrap().is_empty());
        let r: Value = serde_json::from_str(&ia_llamar(
            herramientas::DIBUJAR.into(),
            r#"{"elementos":[{"tipo":"elipse","x":0,"y":0,"ancho":50,"alto":50,"color":"azul"}]}"#
                .into(),
            0.0,
            0.0,
            3,
        ))
        .unwrap();
        assert_eq!(r["ok"], true, "{r}");
        assert!(!ia_instrucciones().is_empty());
        assert!(
            serde_json::from_str::<Value>(&ia_catalogo())
                .unwrap()
                .is_array()
        );
        let doc = ia_lienzo_de(r["elementos"].to_string());
        assert!(pixpin_motor2d::excalidraw::leer(&doc).is_ok());
        assert!(pixpin_motor2d::excalidraw::leer(&ia_lienzo_de("roto".into())).is_ok());
    }

    #[test]
    fn sin_servidor_no_hay_motor_local() {
        // Un puerto libre que nadie escucha.
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = l.local_addr().unwrap().port();
        drop(l);
        assert!(!llama_local_activo(puerto));
        let e = llama_local_preguntar(puerto, Vec::new(), "hola".into()).unwrap_err();
        assert!(matches!(e, ErrorNucleo::Asistente { .. }), "{e:?}");
        assert!(matches!(
            llama_local_preguntar(puerto, Vec::new(), "  ".into()),
            Err(ErrorNucleo::Argumento { .. })
        ));
    }

    #[test]
    fn un_turno_que_dibuja_devuelve_el_lienzo_y_el_texto() {
        let motor = MotorGuionado::nuevo(vec![
            Respuesta::de_llamada(
                herramientas::DIBUJAR,
                r#"{"elementos":[{"tipo":"rectangulo","x":0,"y":0,"ancho":80,"alto":40,"color":"rojo"}]}"#,
            ),
            Respuesta::de_texto("Listo, dibuje un rectangulo rojo."),
        ]);
        let historial = vec![
            MensajeIa {
                del_usuario: true,
                texto: "hola".into(),
            },
            MensajeIa {
                del_usuario: false,
                texto: "Hola, ¿que dibujo?".into(),
            },
        ];
        let r = turno_con_motor(&motor, historial, "dibuja un rectangulo rojo").unwrap();
        assert_eq!(r.texto, "Listo, dibuje un rectangulo rojo.");
        assert_eq!(r.herramientas.len(), 1);
        assert!(r.herramientas[0].ok);
        let doc = r.lienzo.expect("lo dibujado");
        assert!(pixpin_motor2d::excalidraw::leer(&doc).is_ok());
        // El historial llego al motor: sistema + 2 + el pedido.
        let recibido = motor.recibido.lock().unwrap();
        assert!(recibido[0].mensajes.len() >= 4);
    }

    #[test]
    fn lo_que_el_mac_no_sabe_hacer_vuelve_al_modelo_como_fallo() {
        let motor = MotorGuionado::nuevo(vec![
            Respuesta::de_llamada(herramientas::OCR, r#"{"idioma":"es"}"#),
            Respuesta::de_texto("No puedo leer ese PDF aqui todavia."),
        ]);
        let r = turno_con_motor(&motor, Vec::new(), "pasa por ocr el pdf escaneado").unwrap();
        assert!(r.lienzo.is_none());
        assert!(r.herramientas.iter().all(|h| !h.ok), "{:?}", r.herramientas);
        assert!(!r.texto.is_empty());
    }
}
