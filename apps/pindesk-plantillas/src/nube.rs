//! **La IA de verdad**: el [`ClienteIa`] del nucleo hecho con `pindesk-ia`,
//! el mismo Claude o DeepSeek de PinDesk, con la clave cifrada que guarda
//! PinDesk (`pindesk_ia::nube::claves`) y el motor elegido en sus ajustes
//! (`[asistente] motor`). No hay claves propias: sin clave de PinDesk, la
//! app usa `ia::sin_modelo`.
//!
//! Se le ofrece UNA herramienta con su esquema y se devuelven sus
//! argumentos; si el modelo contesta con texto, ese texto (el nucleo busca
//! el JSON dentro y lo repara).

use nucleo::ia::{ClienteIa, Pedido};
use pindesk_ia::Herramienta;
use pindesk_ia::motor::{Mensaje, Motor, Peticion, Seguir};
use pindesk_ia::nube::{MotorNube, Proveedor, claves};

/// Lo que puede tardar en escribir una plantilla entera.
const MAX_TOKENS: u32 = 6000;

/// El cliente: proveedor y clave.
pub struct Nube {
    motor: MotorNube,
}

impl Nube {
    /// **El de PinDesk**, o `None` si no hay ninguna clave.
    pub fn de_pindesk() -> Option<Nube> {
        let elegido = Proveedor::por_id(&crate::rutas::ajustes().asistente.motor);
        let p = elegido
            .filter(|p| claves::hay(*p))
            .or_else(|| Proveedor::TODOS.into_iter().find(|p| claves::hay(*p)))?;
        let clave = claves::leer(p)?;
        // Sin pensar: el diseño es una sola llamada con JSON y conviene que
        // vuelva pronto.
        Some(Nube {
            motor: MotorNube::nuevo(p, clave).ligero(),
        })
    }
}

impl ClienteIa for Nube {
    fn pedir(&self, p: &Pedido) -> Result<String, String> {
        let mut peticion = Peticion::nueva(vec![
            Mensaje::sistema(&p.sistema),
            Mensaje::usuario(&p.usuario),
        ]);
        peticion.herramientas = vec![Herramienta {
            nombre: p.herramienta.to_string(),
            descripcion: p.descripcion.clone(),
            parametros: p.esquema.clone(),
        }];
        peticion.max_tokens = MAX_TOKENS;
        peticion.temperatura = 0.4;
        let r = self
            .motor
            .completar(&peticion, &mut |_| Seguir::Si)
            .map_err(|e| e.to_string())?;
        if let Some(l) = r
            .llamadas
            .iter()
            .find(|l| l.nombre == p.herramienta)
            .or(r.llamadas.first())
        {
            return Ok(l.argumentos.clone());
        }
        if r.texto.trim().is_empty() {
            return Err("respuesta vacia".into());
        }
        Ok(r.texto)
    }
}
