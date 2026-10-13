//! **Un motor por proceso de PixPin.**
//!
//! El modelo ocupa ~2,9 GB mientras esta cargado: dos ventanas de chat no
//! pueden tener cada una el suyo. Aqui vive el unico [`MotorLlama`] del
//! proceso, del modelo elegido en los ajustes; si el usuario cambia de
//! modelo, el viejo se suelta (y su proceso muere) al acabar el turno que lo
//! estuviera usando.
//!
//! Crear el motor no carga nada: el modelo se carga en la primera pregunta
//! (o con [`precargar`] al abrir el chat «Asistente») y se cierra solo tras
//! 5 min sin uso (`Config::reposo`), devolviendo la memoria entera.
//!
//! Elegir donde corre (`--list-devices`, que lanza el servidor un momento)
//! tambien se hace al primer uso y en el hilo de fondo, nunca en el de la
//! ventana: [`MotorPerezoso`].

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use pixpin_ia::ErrorIa;
use pixpin_ia::descarga;
use pixpin_ia::modelos::{LLAMA_CPP, Modelo, QWEN35_2B};
use pixpin_ia::motor::{Motor, Peticion, Respuesta, Seguir, Trozo};
use pixpin_ia::servidor::{Config, MotorLlama, elegir_dispositivo, listar_dispositivos};

/// Donde viven los modelos. `PIXPIN_IA_RAIZ` la cambia (pruebas de extremo a
/// extremo con modelos ya bajados en otra carpeta).
pub fn raiz() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(r) = RAIZ_DE_PRUEBA.with_borrow(Clone::clone) {
        return Some(r);
    }
    std::env::var_os("PIXPIN_IA_RAIZ")
        .map(PathBuf::from)
        .or_else(descarga::carpeta_base)
}

/// El modelo de los ajustes (`asistente_modelo`); sin ajuste, el 2B.
pub fn modelo_elegido(id: Option<&str>) -> Modelo {
    id.and_then(pixpin_ia::modelos::por_id).unwrap_or(QWEN35_2B)
}

/// Menos de esto de memoria y se sugiere el ligero (documento §5.4).
pub const RAM_PARA_EL_2B_MB: u64 = 8 * 1024;

/// **El motor llama.cpp que se monta al primer uso.**
pub struct MotorPerezoso {
    servidor: PathBuf,
    modelo: PathBuf,
    vision: Option<PathBuf>,
    bytes_modelo: u64,
    dentro: OnceLock<MotorLlama>,
}

impl MotorPerezoso {
    pub fn nuevo(raiz: &Path, m: &Modelo) -> MotorPerezoso {
        let carpeta = descarga::carpeta_modelo(raiz, m);
        MotorPerezoso {
            servidor: descarga::ruta_servidor(raiz, &LLAMA_CPP),
            modelo: carpeta.join(m.pesos.nombre),
            vision: m.vision.map(|v| carpeta.join(v.nombre)),
            bytes_modelo: m.pesos.bytes,
            dentro: OnceLock::new(),
        }
    }

    fn motor(&self) -> &MotorLlama {
        self.dentro.get_or_init(|| {
            let mut c = Config::nueva(self.servidor.clone(), self.modelo.clone(), self.vision.clone());
            c.dispositivo = elegir_dispositivo(&listar_dispositivos(&self.servidor), self.bytes_modelo);
            tracing::info!(dispositivo = ?c.dispositivo, "asistente: motor elegido");
            MotorLlama::nuevo(c)
        })
    }
}

impl Motor for MotorPerezoso {
    fn completar(&self, p: &Peticion, al_trozo: &mut dyn FnMut(&Trozo) -> Seguir) -> Result<Respuesta, ErrorIa> {
        if !self.servidor.is_file() {
            return Err(ErrorIa::Falta("el motor del asistente".into()));
        }
        if !self.modelo.is_file() {
            return Err(ErrorIa::Falta("el modelo del asistente".into()));
        }
        self.motor().completar(p, al_trozo)
    }

    fn soltar(&self) {
        if let Some(m) = self.dentro.get() {
            m.soltar();
        }
    }
}

static MOTOR: Mutex<Option<(&'static str, Arc<dyn Motor>)>> = Mutex::new(None);

/// El motor del proceso para este modelo. `Err` si falta bajarlo.
pub fn del_proceso(raiz: &Path, m: &Modelo) -> Result<Arc<dyn Motor>, String> {
    if !super::descarga::listo(raiz, m) {
        return Err(ErrorIa::Falta(m.nombre.to_string()).to_string());
    }
    let mut g = MOTOR.lock().map_err(|_| "candado envenenado".to_string())?;
    if let Some((id, motor)) = g.as_ref()
        && *id == m.id
    {
        return Ok(motor.clone());
    }
    if let Some((_, viejo)) = g.take() {
        viejo.soltar();
    }
    let nuevo: Arc<dyn Motor> = Arc::new(MotorPerezoso::nuevo(raiz, m));
    *g = Some((m.id, nuevo.clone()));
    Ok(nuevo)
}

/// Carga el modelo en un hilo, sin esperar (al abrir el chat «Asistente»,
/// para que la primera respuesta no tarde los 20 s de la carga).
pub fn precargar(motor: Arc<dyn Motor>) {
    let _ = std::thread::Builder::new().name("asistente-precarga".into()).spawn(move || {
        // Una peticion vacia de un token: carga el modelo y vuelve.
        let mut p = Peticion::nueva(vec![pixpin_ia::Mensaje::usuario("hola")]);
        p.max_tokens = 1;
        let _ = motor.completar(&p, &mut |_| Seguir::Si);
    });
}

/// Suelta el modelo ya (al cambiar de modelo en los ajustes).
pub fn soltar_todo() {
    if let Ok(mut g) = MOTOR.lock()
        && let Some((_, m)) = g.take()
    {
        m.soltar();
    }
}

#[cfg(test)]
thread_local! {
    static RAIZ_DE_PRUEBA: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Las pruebas apuntan los modelos a otra carpeta sin tocar el entorno.
#[cfg(test)]
pub fn raiz_de_prueba(r: Option<PathBuf>) {
    RAIZ_DE_PRUEBA.with_borrow_mut(|x| *x = r);
}
