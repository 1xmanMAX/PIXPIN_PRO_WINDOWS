//! **Un motor por proceso de PixPin.**
//!
//! El modelo ocupa ~2,9 GB mientras esta cargado: dos ventanas de chat no
//! pueden tener cada una el suyo. Aqui viven los motores locales del
//! proceso, en un **relevo** (`pixpin_ia::relevo`): el de los ajustes y,
//! si el usuario lo encendio y lo bajo, el de mejor vista (Gemma 4 E2B) para
//! mirar fotos ([`modelo_para_el_turno`]). Solo uno tiene su `llama-server`
//! cargado: antes de contestar uno, el otro se suelta (y su proceso muere).
//!
//! Crear el motor no carga nada: el modelo se carga en la primera pregunta
//! (o con [`precargar`] al abrir el chat «Asistente») y se cierra solo tras
//! 5 min sin uso (`Config::reposo`), devolviendo la memoria entera.
//!
//! Elegir donde corre (`--list-devices`, que lanza el servidor un momento)
//! tambien se hace al primer uso y en el hilo de fondo, nunca en el de la
//! ventana: [`MotorPerezoso`].

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use pixpin_ia::ErrorIa;
use pixpin_ia::descarga;
use pixpin_ia::modelos::{GEMMA4_E2B, LLAMA_CPP, MODELOS, Modelo, QWEN35_2B};
use pixpin_ia::relevo::Relevo;
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

/// El modelo de los ajustes (`asistente_modelo`); sin ajuste, el 2B. El de
/// mejor vista nunca es «el de siempre»: va aparte (`modelo_para_el_turno`).
pub fn modelo_elegido(id: Option<&str>) -> Modelo {
    id.and_then(|id| MODELOS.iter().copied().find(|m| m.id == id)).unwrap_or(QWEN35_2B)
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

    fn precalentar(&self) {
        if self.servidor.is_file() && self.modelo.is_file() {
            self.motor().precalentar();
        }
    }
}

/// Los motores locales del proceso: **un solo `llama-server` cargado a la
/// vez** (`pixpin_ia::relevo`). El de siempre y el de mejor vista se
/// relevan: antes de contestar uno, el otro se suelta y su memoria vuelve.
fn relevo() -> &'static Relevo {
    static RELEVO: OnceLock<Relevo> = OnceLock::new();
    RELEVO.get_or_init(Relevo::nuevo)
}

/// La llave de un motor en el relevo: el modelo y su carpeta (las pruebas
/// apuntan a otras carpetas).
fn llave(raiz: &Path, m: &Modelo) -> String {
    format!("{}@{}", m.id, raiz.display())
}

/// El motor del proceso para este modelo. `Err` si falta bajarlo.
pub fn del_proceso(raiz: &Path, m: &Modelo) -> Result<Arc<dyn Motor>, String> {
    if !super::descarga::listo(raiz, m) {
        return Err(ErrorIa::Falta(m.nombre.to_string()).to_string());
    }
    let (raiz, modelo) = (raiz.to_path_buf(), *m);
    Ok(relevo().motor(&llave(&raiz, m), move || Arc::new(MotorPerezoso::nuevo(&raiz, &modelo))))
}

/// **El modelo de ESTE turno**: el de mejor vista (Gemma 4 E2B) si esta
/// encendido en los ajustes, bajado, y lo que se pide es mirar la foto
/// (`seleccion::pide_mejor_vista`); si no, el de siempre.
pub fn modelo_para_el_turno(a: &pixpin_store::ajustes::Asistente, raiz: &Path, pedido: &str, con_foto: bool) -> Modelo {
    elegir_para_el_turno(a, || super::descarga::listo(raiz, &GEMMA4_E2B), pedido, con_foto)
}

/// La regla de [`modelo_para_el_turno`], con «¿esta bajado?» aparte (el
/// disco se mira solo si hace falta).
pub fn elegir_para_el_turno(
    a: &pixpin_store::ajustes::Asistente,
    gemma_bajado: impl FnOnce() -> bool,
    pedido: &str,
    con_foto: bool,
) -> Modelo {
    if a.mejor_vista && pixpin_ia::seleccion::pide_mejor_vista(pedido, con_foto) && gemma_bajado() {
        tracing::info!("asistente: foto para mirar, con el modelo de mejor vista");
        return GEMMA4_E2B;
    }
    modelo_elegido(Some(a.modelo.as_str()).filter(|m| !m.is_empty()))
}

/// La ayuda del interruptor «Mejor vista con fotos»: si esta bajado o
/// cuanto falta.
pub fn estado_mejor_vista(t: &pixpin_store::Catalogo) -> String {
    let bajado = raiz().is_some_and(|r| super::descarga::listo(&r, &GEMMA4_E2B));
    if bajado {
        t.t("ajustes2-ayuda-mejor-vista-lista")
    } else {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("tamano", super::descarga::tamano(GEMMA4_E2B.bytes()));
        t.t_args("ajustes2-ayuda-mejor-vista", &args)
    }
}

/// Carga el modelo en un hilo, sin esperar (al abrir el chat «Asistente»,
/// para que la primera respuesta no tarde los 20 s de la carga).
pub fn precargar(motor: Arc<dyn Motor>) {
    // Una sola a la vez: abrir y cerrar el chat deprisa no lanza diez.
    static EN_CURSO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if EN_CURSO.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let lanzado = std::thread::Builder::new().name("asistente-precarga".into()).spawn(move || {
        // Arranca `llama-server` y carga el modelo (la nube no hace nada):
        // la primera pregunta ya no espera la carga.
        let t0 = std::time::Instant::now();
        motor.precalentar();
        tracing::info!(ms = t0.elapsed().as_millis() as u64, "asistente: precargado");
        EN_CURSO.store(false, std::sync::atomic::Ordering::SeqCst);
    });
    if lanzado.is_err() {
        EN_CURSO.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Suelta los modelos ya (al cambiar de modelo en los ajustes).
pub fn soltar_todo() {
    relevo().soltar_todo();
}

/// Suelta el de mejor vista (se apago su interruptor).
pub fn soltar_mejor_vista() {
    if let Some(r) = raiz() {
        relevo().quitar(&llave(&r, &GEMMA4_E2B));
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
