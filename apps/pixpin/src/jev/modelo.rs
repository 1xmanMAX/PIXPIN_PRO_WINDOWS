//! **Que modelo contesta cada pedido al asistente** (`docs/jev.md`): antes
//! del turno, Jev dice lo dificil que es, si es una orden simple, si pide
//! razonar y si un error importaria (`pixpin_typesafe::modelo`); la tabla de
//! alli elige el modelo rapido (Claude Haiku, DeepSeek Flash: el de siempre)
//! o el fuerte del mismo proveedor y con la misma clave (Claude Sonnet,
//! DeepSeek V4 Pro).
//!
//! - Las ordenes del camino rapido (`asistente::rapido`) ni preguntan: se
//!   hacen en este PC, como siempre.
//! - La pregunta se hace **en el hilo del turno**, al pedirle la primera
//!   respuesta ([`MotorSegunJev`]): el chat no espera nada al enviar.
//! - La pastilla del modelo del chat ensena lo elegido ([`rotulo`]).
//! - Sin clave de TypeSafe, con la funcion apagada, o si Jev falla o tarda,
//!   el de siempre.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use pixpin_ia::motor::{Motor, Peticion, Respuesta, Seguir, Trozo};
use pixpin_ia::nube::{self, MotorNube, Proveedor};
use pixpin_store::Ubicacion;
use pixpin_typesafe::Cliente;
use pixpin_typesafe::modelo::{self as jm, Contexto, Ruta};

use super::Funcion;

/// El modelo fuerte de cada proveedor y como se llama en la pastilla.
pub fn fuerte_de(p: Proveedor) -> (&'static str, &'static str) {
    match p {
        Proveedor::Claude => (nube::claude::MODELO_CRITICO, "Claude Sonnet"),
        Proveedor::DeepSeek => (nube::deepseek::MODELO_PRO, "DeepSeek V4 Pro"),
    }
}

/// Lo elegido en el ultimo pedido de cada chat: cuando y el rotulo.
static ELEGIDO: Mutex<Option<HashMap<String, (Instant, &'static str)>>> = Mutex::new(None);

/// Lo que dura en la pastilla lo elegido para el ultimo pedido.
const EN_LA_PASTILLA: Duration = Duration::from_secs(15 * 60);

fn apuntar(proyecto: &str, rotulo: &'static str) {
    let mut g = ELEGIDO.lock().unwrap_or_else(|e| e.into_inner());
    g.get_or_insert_with(HashMap::new)
        .insert(proyecto.to_string(), (Instant::now(), rotulo));
    // El chat repinta la pastilla.
    crate::ventana_chat::refrescar();
}

/// **El rotulo de la pastilla del modelo**: lo que eligio Jev para el ultimo
/// pedido de este chat («Claude Sonnet · Jev»), si fue hace poco.
pub fn rotulo(proyecto: &str) -> Option<String> {
    let g = ELEGIDO.lock().ok()?;
    let (cuando, r) = g.as_ref()?.get(proyecto)?;
    (cuando.elapsed() < EN_LA_PASTILLA).then(|| format!("{r} · Jev"))
}

/// **Un motor que elige su modelo** al pedirle la primera respuesta. Los dos
/// son del mismo proveedor: ven lo mismo y aguantan lo mismo.
pub struct MotorSegunJev {
    rapido: Arc<dyn Motor>,
    fuerte: Arc<dyn Motor>,
    rotulos: (&'static str, &'static str),
    cliente: Cliente,
    contexto: Contexto,
    proyecto: String,
    elegido: OnceLock<Arc<dyn Motor>>,
}

impl MotorSegunJev {
    fn elegido(&self) -> &Arc<dyn Motor> {
        self.elegido.get_or_init(|| {
            let empezo = Instant::now();
            match jm::elegir(&self.cliente, &self.contexto) {
                Ok((ruta, motivo)) => {
                    tracing::info!(
                        ?ruta,
                        ?motivo,
                        ms = empezo.elapsed().as_millis() as u64,
                        "jev: modelo del pedido"
                    );
                    let (motor, rotulo) = match ruta {
                        Ruta::Fuerte => (&self.fuerte, self.rotulos.1),
                        Ruta::Rapido => (&self.rapido, self.rotulos.0),
                    };
                    apuntar(&self.proyecto, rotulo);
                    motor.clone()
                }
                Err(e) => {
                    // Un fallo no elige nada: el de siempre.
                    tracing::warn!("jev: sin modelo elegido, el de siempre: {e}");
                    self.rapido.clone()
                }
            }
        })
    }
}

impl Motor for MotorSegunJev {
    fn completar(
        &self,
        peticion: &Peticion,
        al_trozo: &mut dyn FnMut(&Trozo) -> Seguir,
    ) -> Result<Respuesta, pixpin_ia::ErrorIa> {
        self.elegido().completar(peticion, al_trozo)
    }

    fn catalogo_completo(&self) -> bool {
        self.rapido.catalogo_completo()
    }

    fn ve_imagenes(&self) -> bool {
        self.rapido.ve_imagenes()
    }

    fn a_la_vez(&self) -> usize {
        self.rapido.a_la_vez()
    }
}

/// Los ultimos mensajes de la conversacion, con quien los dijo.
pub fn recientes(conversacion: &[(bool, String)]) -> Vec<(String, String)> {
    let n = conversacion.len();
    conversacion
        .iter()
        .skip(n.saturating_sub(jm::RECIENTES))
        .map(|(del_usuario, t)| {
            (
                if *del_usuario { "usuario" } else { "asistente" }.to_string(),
                t.clone(),
            )
        })
        .collect()
}

/// **El motor de este pedido**: `motor` (el de los ajustes) o, con Jev, uno
/// que elige entre ese y el fuerte. `p` es el proveedor elegido; `pensar`,
/// si piensa este turno.
pub fn segun_jev(
    u: &Ubicacion,
    proyecto: &str,
    motor: Arc<dyn Motor>,
    p: Proveedor,
    pensar: bool,
    pedido: &str,
    conversacion: &[(bool, String)],
) -> Arc<dyn Motor> {
    // Las ordenes simples se hacen en este PC (camino rapido): sin Jev.
    if crate::asistente::rapido::es_orden(pedido) {
        return motor;
    }
    let Some(cliente) = super::para(u, Funcion::Modelo) else {
        return motor;
    };
    let Some(clave) = nube::claves::leer(p) else {
        return motor;
    };
    let (modelo, nombre) = fuerte_de(p);
    let fuerte: Arc<dyn Motor> = Arc::new(
        MotorNube::nuevo(p, clave)
            .con_modelo(modelo)
            .pensando(pensar),
    );
    Arc::new(MotorSegunJev {
        rapido: motor,
        fuerte,
        rotulos: (crate::asistente::nube::nombre_corto(p), nombre),
        cliente,
        contexto: Contexto {
            pedido: pedido.to_string(),
            recientes: recientes(conversacion),
        },
        proyecto: proyecto.to_string(),
        elegido: OnceLock::new(),
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_recientes_son_los_ultimos_con_quien() {
        let c: Vec<(bool, String)> = (1..=5).map(|i| (i % 2 == 1, format!("m{i}"))).collect();
        let r = recientes(&c);
        assert_eq!(r.len(), jm::RECIENTES);
        assert_eq!(r[0], ("usuario".to_string(), "m3".to_string()));
        assert_eq!(r[1].0, "asistente");
    }

    #[test]
    fn el_fuerte_de_cada_proveedor() {
        assert_eq!(fuerte_de(Proveedor::Claude).0, "claude-sonnet-5-5");
        assert_eq!(fuerte_de(Proveedor::DeepSeek).0, nube::deepseek::MODELO_PRO);
    }

    #[test]
    fn sin_jev_el_motor_es_el_mismo() {
        let u = Ubicacion::Instalado {
            raiz: std::env::temp_dir().join("pixpin-jev-modelo-sin-clave"),
        };
        let m: Arc<dyn Motor> = Arc::new(pixpin_ia::motor::MotorGuionado::nuevo(vec![]));
        let r = segun_jev(
            &u,
            "p",
            m.clone(),
            Proveedor::Claude,
            false,
            "explica la entropía",
            &[],
        );
        assert!(Arc::ptr_eq(&m, &r));
        assert!(rotulo("p").is_none());
    }
}
