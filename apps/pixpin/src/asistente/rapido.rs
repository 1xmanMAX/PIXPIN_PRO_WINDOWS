//! **El camino rapido, igual para todos los que preguntan** (14-oct): el
//! chat (`ventana_chat::asistente`), el pedido `asistente` de Flow Launcher
//! y los pasos `asistente` de las automatizaciones
//! (`automatizaciones::encargo`).
//!
//! Las ordenes simples («¿que tengo pendiente?», «apunta comprar pan») las
//! hace `pixpin_ia::rapido` dentro del turno (`agente::Opciones::rapido`),
//! en milisegundos y sin tocar el modelo. Aqui va lo poco que tienen que
//! compartir los tres para que eso pase: el reloj local, saber si una orden
//! es rapida y un «motor» de relleno cuando no hay ninguno (sin bajar, sin
//! clave), para que una orden rapida no se quede sin hacer por eso.

use std::sync::Arc;

use pixpin_ia::fechas::Momento;
use pixpin_ia::motor::{Motor, Peticion, Respuesta, Seguir, Trozo};

/// Lo que se ve en la burbuja (o el aviso flotante) desde el primer
/// instante, antes de que llegue nada del modelo.
pub const ESTADO_INICIAL: &str = "💭 Pensando…";

/// Ahora, en hora local (el «mañana» de las tareas).
pub fn ahora() -> Momento {
    Momento::de_ms_locales(pixpin_shell::entorno::ahora_local_ms())
}

/// Si el mensaje es una orden simple que el camino rapido hace sin modelo.
pub fn es_orden(pregunta: &str) -> bool {
    pixpin_ia::rapido::interpretar(pregunta, ahora()).is_some()
}

/// El «motor» cuando no hay ninguno: el camino rapido no lo necesita, y si
/// hiciera falta, dice por que no hay.
pub struct SinMotor(pub String);

impl Motor for SinMotor {
    fn completar(
        &self,
        _: &Peticion,
        _: &mut dyn FnMut(&Trozo) -> Seguir,
    ) -> Result<Respuesta, pixpin_ia::ErrorIa> {
        Err(pixpin_ia::ErrorIa::Nube(self.0.clone()))
    }
}

/// **El motor para esta pregunta**: el de los ajustes, o, si no lo hay y la
/// orden es rapida, [`SinMotor`] (la orden se hace igual). Si no lo hay y la
/// orden no es rapida, el aviso de por que.
pub fn motor_o_sin(
    motor: Result<Arc<dyn Motor>, String>,
    pregunta: &str,
) -> Result<Arc<dyn Motor>, String> {
    match motor {
        Ok(m) => Ok(m),
        Err(aviso) if es_orden(pregunta) => Ok(Arc::new(SinMotor(aviso))),
        Err(aviso) => Err(aviso),
    }
}
