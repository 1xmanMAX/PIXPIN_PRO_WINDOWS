//! **Aligerar los PDF que entran al chat**, con pdfsqueeze (el compresor del
//! propio autor, `crates/pdfsqueeze-core`), como hace el movil en
//! `pdf/PdfSqueezeService.kt` y `pdf/ComprimirPdf.kt`.
//!
//! Lo que se copia del movil, y por que:
//!
//! - **Al entrar no se espera a nadie.** El PDF se guarda tal cual, al
//!   instante, y se aligera DESPUES, en una cola de un solo trabajador. Al
//!   acabar se cambia en su sitio de un golpe (`rename`), junto con sus
//!   copias identicas de la misma carpeta (la «copia limpia» del proyecto y
//!   el documento, que nacen del mismo fichero). En el movil lo noto el
//!   usuario enseguida: comprimir en el hilo que guarda dejaba «nuevo
//!   proyecto» sin responder de 3 a 8 segundos.
//! - **Nunca a peor.** Solo se cambia si baja al menos un 15 % (un 2 % sin
//!   perdida, donde cualquier ganancia es gratis), si se sigue abriendo con
//!   las mismas paginas, y si mientras tanto nadie cambio el fichero. Un PDF
//!   cifrado se queda como esta, sin decir nada.
//! - **Plan B**: si pdfsqueeze no puede, lo intenta el aligerar propio de
//!   PixPin (`pixpin_pdf::aligerar`, solo fotos JPEG), igual que el movil cae
//!   a su compresor en Kotlin. Sin perdida no hay plan B: el B es con perdida.
//!
//! Lo que es de aqui:
//!
//! - **pdfsqueeze va en su propio ejecutable**, `pixpin-aligerar.exe`, al
//!   lado de `pixpinmax.exe` (ver `apps/pixpin-aligerar`). Pesa ~1,8 MB que
//!   el programa principal no carga en cada arranque, y como proceso aparte,
//!   un PDF raro que haga entrar en panico a lopdf (el release es
//!   `panic = "abort"`) mata al compresor y no a PixPin con lo que el usuario
//!   estuviera dibujando. Cancelar es matar el proceso; la memoria de las
//!   fotos descomprimidas se devuelve entera al acabar; corre por debajo de la
//!   prioridad normal (no en el «modo de fondo» de Windows: medido, veinte
//!   veces mas lento, ver `pixpin_shell::prioridad`). **Si el ejecutable no
//!   esta**, se aligera igual, con el plan B, y se dice en el registro.
//! - **Al entrar solo Exacto o Medio** ([`nivel_para_entrar`]). Chico y Max
//!   degradan el papel y las fotos a proposito y gastan mucha mas memoria
//!   (1,4 GB medidos en un escaneo de 20 hojas): se usan solo pedidos a mano.
//!   Si el ajuste dice Chico o Max, al entrar se usa Medio.
//! - **Modo cuidadoso** ([`modo_cuidadoso`]) en equipos de 4 GB o menos, o
//!   si el nivel de rendimiento es Ligero: un solo hilo, y si a Windows le
//!   queda poca memoria ([`falta_memoria`]) el siguiente PDF no empieza y el
//!   que va se congela, hasta que se recupere. En los demas, la mitad de los
//!   nucleos: cada hilo tiene una pagina escaneada descomprimida en memoria
//!   (~25 MB en A4 a 300 ppp).
//!
//! **Sin pasadas repetidas.** Cada PDF ya visto —el original y el aligerado—
//! se apunta por su huella (SHA-256) en `cache/pdf-aligerados.txt`. Unir al
//! proyecto un PDF que ya se aligero al entrar copia el ligero y no lo vuelve
//! a comprimir; y si el usuario lo une mientras aun se esta aligerando,
//! `unir` espera a que acabe (ver [`esperar`]), asi que el proyecto nace con
//! la version ligera. Las paginas no cambian de numero, y las hojas y lo
//! dibujado encima, que se guardan por numero de pagina, siguen donde
//! estaban.
//!
//! **Nada de esto corre en el hilo de la ventana**: lo que llama la ventana
//! ([`al_entrar`], [`a_mano`], [`cancelar`], [`progreso_de_mensaje`],
//! [`terminados`]) solo toca una cola en memoria.

use std::collections::{HashSet, VecDeque};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use pixpin_store::ajustes::{NivelPdf, Pdf};

/// El ejecutable del compresor, junto a `pixpinmax.exe`. Tiene que llamarse
/// como `pixpin_aligerar::NOMBRE_EXE` (lo vigila una prueba); no se importa
/// de alli para no enlazar pdfsqueeze en el programa principal.
const NOMBRE_EXE: &str = "pixpin-aligerar.exe";

// ---------------------------------------------------------------------------
// Los ajustes y el equipo

/// Lo puesto en Ajustes. `None` hasta que `main` lo ponga: asi las pruebas de
/// otros modulos que unen PDF no se encuentran un trabajador comprimiendo por
/// detras.
static AJUSTES: Mutex<Option<Pdf>> = Mutex::new(None);

/// Si el equipo va en modo cuidadoso. Lo fija `main` con los hechos del
/// equipo al arrancar.
static CUIDADOSO: AtomicBool = AtomicBool::new(false);

/// Lo mantiene al dia `main`: al arrancar y al cerrar la ventana de ajustes.
/// Es el `ComprimirPdf.nivel` del movil.
pub fn configurar(pdf: &Pdf) {
    if let Ok(mut a) = AJUSTES.lock() {
        *a = Some(pdf.clone());
    }
}

/// Lo llama `main` con la RAM del equipo y si el nivel de rendimiento
/// decidido es Ligero. Ver [`modo_cuidadoso`].
pub fn fijar_equipo(ram_fisica_bytes: u64, nivel_ligero: bool) {
    let c = modo_cuidadoso(ram_fisica_bytes, nivel_ligero);
    CUIDADOSO.store(c, Ordering::Relaxed);
    if c {
        tracing::info!(
            ram_fisica_bytes,
            nivel_ligero,
            "aligerar PDF en modo cuidadoso"
        );
    }
}

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

/// **Modo cuidadoso**: 4 GB de RAM o menos (Windows cuenta algo menos de lo
/// que dice la pegatina, por eso el «o menos»), o el nivel de rendimiento
/// Ligero. Una RAM desconocida (0) no cuenta: no se sabe, y el nivel ya lo
/// habra decidido con lo que haya.
pub fn modo_cuidadoso(ram_fisica_bytes: u64, nivel_ligero: bool) -> bool {
    nivel_ligero || (ram_fisica_bytes > 0 && ram_fisica_bytes <= 4 * GIB)
}

/// **Si a Windows le falta memoria** para empezar o seguir comprimiendo: menos
/// de 700 MB disponibles o menos del 15 % del total. Por debajo de ahi, el
/// compresor —cientos de megas con un escaneo— haria que Windows empezara a
/// sacar a disco lo que el usuario tiene abierto.
pub fn falta_memoria(total: u64, disponible: u64) -> bool {
    disponible < 700 * MIB || disponible.saturating_mul(100) < total.saturating_mul(15)
}

/// **El nivel con el que se aligera al entrar**: Exacto y Medio como estan;
/// Chico y Max se quedan para pedirlos a mano, y al entrar se usa Medio. Ver
/// la cabecera del modulo.
pub fn nivel_para_entrar(elegido: NivelPdf) -> NivelPdf {
    match elegido {
        NivelPdf::SinPerdida => NivelPdf::SinPerdida,
        NivelPdf::Equilibrado | NivelPdf::Pequeno | NivelPdf::Extremo => NivelPdf::Equilibrado,
    }
}

/// El nivel con el que se aligera al entrar, o `None` si esta apagado.
fn nivel_al_entrar() -> Option<NivelPdf> {
    let a = AJUSTES.lock().ok()?.clone()?;
    a.aligerar_al_entrar.then_some(nivel_para_entrar(a.nivel))
}

/// El nivel de «Aligerar el PDF» pedido a mano: el de Ajustes tal cual —Chico
/// y Max incluidos— aunque aligerar al entrar este apagado, que para eso se ha
/// pedido (`enSuSitio` del movil).
fn nivel_a_mano() -> NivelPdf {
    AJUSTES
        .lock()
        .ok()
        .and_then(|a| a.as_ref().map(|p| p.nivel))
        .unwrap_or_default()
}

/// Lo que tiene que quedar, como mucho, para cambiar el fichero
/// (`loQueTieneQueBajar` del movil): sin perdida vale cualquier ganancia de
/// verdad; con perdida, un 15 %, que por menos no compensa haber recodificado
/// las fotos.
pub fn lo_que_puede_quedar(n: NivelPdf) -> f64 {
    if n == NivelPdf::SinPerdida {
        0.98
    } else {
        0.85
    }
}

/// Cuantos hilos usa el compresor: uno en modo cuidadoso; si no, la mitad de
/// los nucleos, al menos uno.
pub fn hilos_para(cuidadoso: bool, nucleos: usize) -> usize {
    if cuidadoso { 1 } else { (nucleos / 2).max(1) }
}

fn hilos() -> usize {
    let nucleos = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    hilos_para(CUIDADOSO.load(Ordering::Relaxed), nucleos)
}

/// Como se llama cada nivel para `pixpin-aligerar.exe`.
fn nombre_de(n: NivelPdf) -> &'static str {
    match n {
        NivelPdf::SinPerdida => "sin-perdida",
        NivelPdf::Equilibrado => "equilibrado",
        NivelPdf::Pequeno => "pequeno",
        NivelPdf::Extremo => "extremo",
    }
}

/// La memoria del sistema `(total, disponible)`. En las pruebas se puede
/// fingir, para ver que el trabajador espera sin tener que llenar la RAM.
fn memoria_del_sistema() -> Option<(u64, u64)> {
    #[cfg(test)]
    if let Some(m) = MEMORIA_FINGIDA.lock().ok().and_then(|m| *m) {
        return Some(m);
    }
    pixpin_shell::prioridad::memoria_del_sistema()
}

#[cfg(test)]
static MEMORIA_FINGIDA: Mutex<Option<(u64, u64)>> = Mutex::new(None);

/// En modo cuidadoso, si ahora mismo falta memoria. Fuera de el, nunca.
fn hay_que_esperar_a_la_memoria() -> bool {
    CUIDADOSO.load(Ordering::Relaxed)
        && memoria_del_sistema().is_some_and(|(t, d)| falta_memoria(t, d))
}

// ---------------------------------------------------------------------------
// La cola, que la ventana solo toca en memoria

#[derive(Clone, Debug)]
struct Pedido {
    ruta: PathBuf,
    raiz: PathBuf,
    nivel: NivelPdf,
    /// Pedido desde el menu: hay alguien esperando el resultado, asi que no
    /// se salta aunque ya se hubiera visto, va delante en la cola y se dice
    /// como acabo aunque sea que no se gano nada.
    a_mano: bool,
    ficha: Option<String>,
    mensaje: Option<String>,
}

/// Como acabo un PDF.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Desenlace {
    /// Cambiado en su sitio (y sus copias identicas).
    Aligerado { antes: u64, despues: u64 },
    /// No bajaba lo bastante: se queda el original.
    SinMejora,
    /// Cifrado: se queda el original. Al entrar, sin decir nada.
    Cifrado,
    /// Ya se habia aligerado (o intentado) antes: no se repite la pasada.
    YaEstaba,
    /// Se borro el mensaje o se cerro la aplicacion a mitad.
    Cancelado,
    /// No se pudo, y se queda el original. La razon va al registro.
    Fallo(String),
}

/// Un PDF acabado, para que la ventana ponga al dia su peso y lo diga.
#[derive(Clone, Debug)]
pub struct Terminado {
    pub ruta: PathBuf,
    pub ficha: Option<String>,
    pub mensaje: Option<String>,
    pub a_mano: bool,
    pub desenlace: Desenlace,
}

struct EnCurso {
    ruta: PathBuf,
    mensaje: Option<String>,
    por: u8,
    cancelado: Arc<AtomicBool>,
    hijo: Arc<Mutex<Option<std::process::Child>>>,
}

#[derive(Default)]
struct Estado {
    cola: VecDeque<Pedido>,
    en_curso: Option<EnCurso>,
    terminados: Vec<Terminado>,
    hay_hilo: bool,
}

fn estado() -> &'static (Mutex<Estado>, Condvar) {
    static E: OnceLock<(Mutex<Estado>, Condvar)> = OnceLock::new();
    E.get_or_init(|| (Mutex::new(Estado::default()), Condvar::new()))
}

/// Lo coge el trabajador mientras comprueba y cambia ficheros, y [`cancelar`]
/// despues de levantar la bandera: asi, cuando `cancelar` vuelve, nadie va a
/// escribir ya ese fichero, y quien lo borra justo despues no se lo encuentra
/// resucitado por un `rename` que llego tarde.
static CAMBIANDO: Mutex<()> = Mutex::new(());

/// La ventana a la que despertar cuando cambia el progreso o algo acaba.
static VENTANA: AtomicIsize = AtomicIsize::new(0);

/// A quien despertar. Lo pone la ventana del chat al abrirse.
pub fn avisar_a(hwnd: isize) {
    VENTANA.store(hwnd, Ordering::Relaxed);
}

/// Cuantas veces ha cambiado algo que se ve (la cola, un porcentaje, un
/// final). La ventana lo compara con el de la vez anterior para volver a
/// medir las burbujas solo cuando hace falta, y no en cada movimiento del
/// raton mientras se comprime.
static CAMBIOS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Ver [`CAMBIOS`].
pub fn cambios() -> u64 {
    CAMBIOS.load(Ordering::Acquire)
}

fn despertar() {
    CAMBIOS.fetch_add(1, Ordering::AcqRel);
    let h = VENTANA.load(Ordering::Relaxed);
    if h != 0 {
        pixpin_shell::overlay::despertar(h);
    }
}

fn encolar(p: Pedido) -> bool {
    let (m, cv) = estado();
    let Ok(mut e) = m.lock() else { return false };
    if e.en_curso.as_ref().is_some_and(|c| c.ruta == p.ruta) {
        return false;
    }
    if let Some(i) = e.cola.iter().position(|q| q.ruta == p.ruta) {
        // Ya estaba esperando: pedirlo a mano lo pasa delante y le da la
        // voz de quien espera.
        if p.a_mano {
            e.cola.remove(i);
            e.cola.push_front(p);
            cv.notify_all();
            return true;
        }
        return false;
    }
    if p.a_mano {
        e.cola.push_front(p);
    } else {
        e.cola.push_back(p);
    }
    if !e.hay_hilo {
        let lanzado = std::thread::Builder::new()
            .name("pdf-aligerar".into())
            .spawn(trabajador)
            .is_ok();
        e.hay_hilo = lanzado;
        if !lanzado {
            e.cola.clear();
            return false;
        }
    }
    cv.notify_all();
    drop(e);
    despertar();
    true
}

/// **Un PDF acaba de entrar al chat**: se aligera despues, si esta
/// encendido en Ajustes. No lee el fichero: solo lo apunta en la cola.
pub fn al_entrar(raiz: &Path, ficha: &str, mensaje: &str, ruta: &Path) -> bool {
    let Some(nivel) = nivel_al_entrar() else {
        return false;
    };
    encolar(Pedido {
        ruta: ruta.to_path_buf(),
        raiz: raiz.to_path_buf(),
        nivel,
        a_mano: false,
        ficha: Some(ficha.to_string()),
        mensaje: Some(mensaje.to_string()),
    })
}

/// **«Aligerar el PDF» del menu del mensaje.** Delante de la cola y con el
/// nivel de Ajustes aunque aligerar al entrar este apagado. Como acaba lo
/// dice [`terminados`].
pub fn a_mano(raiz: &Path, ficha: &str, mensaje: &str, ruta: &Path) -> bool {
    encolar(Pedido {
        ruta: ruta.to_path_buf(),
        raiz: raiz.to_path_buf(),
        nivel: nivel_a_mano(),
        a_mano: true,
        ficha: Some(ficha.to_string()),
        mensaje: Some(mensaje.to_string()),
    })
}

/// **Tras unir un PDF al proyecto como su documento.** Si el adjunto ya se
/// aligero, el documento es copia del ligero y la huella lo dice: el
/// trabajador lo salta sin comprimir. Si no (un adjunto de antes de esto, o
/// llegado del movil), el documento se aligera ahora, en la cola, y su copia
/// limpia con el por ser identica.
pub fn tras_unir(raiz: &Path, documento: &Path) {
    let Some(nivel) = nivel_al_entrar() else {
        return;
    };
    encolar(Pedido {
        ruta: documento.to_path_buf(),
        raiz: raiz.to_path_buf(),
        nivel,
        a_mano: false,
        ficha: None,
        mensaje: None,
    });
}

/// Si ese fichero esta en la cola o aligerandose.
pub fn pendiente(ruta: &Path) -> bool {
    let (m, _) = estado();
    m.lock().is_ok_and(|e| {
        e.en_curso.as_ref().is_some_and(|c| c.ruta == ruta) || e.cola.iter().any(|p| p.ruta == ruta)
    })
}

/// **Espera a que `ruta` acabe de aligerarse**, pasandola delante si aun
/// esperaba turno. Lo llama `unir` —en su hilo, nunca la ventana— para que el
/// proyecto nazca con la version ligera y no con la pesada: unir mientras se
/// comprime dejaba las paginas pesadas pegadas al documento. Tope de diez
/// minutos: un PDF que no acaba no puede dejar «Anadir al proyecto» colgado.
pub fn esperar(ruta: &Path) {
    let (m, cv) = estado();
    let Ok(mut e) = m.lock() else { return };
    if let Some(i) = e.cola.iter().position(|p| p.ruta == ruta)
        && i > 0
        && let Some(p) = e.cola.remove(i)
    {
        e.cola.push_front(p);
        cv.notify_all();
    }
    let hasta = std::time::Instant::now() + std::time::Duration::from_secs(600);
    loop {
        let sigue = e.en_curso.as_ref().is_some_and(|c| c.ruta == ruta)
            || e.cola.iter().any(|p| p.ruta == ruta);
        let queda = hasta.saturating_duration_since(std::time::Instant::now());
        if !sigue || queda.is_zero() {
            return;
        }
        e = match cv.wait_timeout(e, queda.min(std::time::Duration::from_millis(500))) {
            Ok((e, _)) => e,
            Err(_) => return,
        };
    }
}

/// **Deja de aligerar `ruta`**: se quita de la cola y, si ya iba, se mata su
/// proceso. Al volver, nadie va a escribir ese fichero: se puede borrar.
/// Lo llama la ventana al borrar el mensaje.
pub fn cancelar(ruta: &Path) {
    {
        let (m, cv) = estado();
        let Ok(mut e) = m.lock() else { return };
        e.cola.retain(|p| p.ruta != ruta);
        if let Some(c) = e.en_curso.as_ref().filter(|c| c.ruta == ruta) {
            c.cancelado.store(true, Ordering::Release);
            if let Ok(mut h) = c.hijo.lock()
                && let Some(h) = h.as_mut()
            {
                let _ = h.kill();
            }
        }
        cv.notify_all();
    }
    // Si el trabajador estaba justo cambiando el fichero, se espera a que
    // acabe: a partir de aqui ve la bandera y no toca nada mas.
    drop(CAMBIANDO.lock());
    despertar();
}

/// Lo para todo: la cola se vacia y el que va se mata. Al cerrar la
/// aplicacion no hace falta —el proceso hijo se va solo cuando se cierra su
/// entrada estandar, que es cuando muere PixPin—, pero salir por las buenas
/// no deja nada a medias en el disco.
pub fn cancelar_todo() {
    let ruta = {
        let (m, _) = estado();
        let Ok(mut e) = m.lock() else { return };
        e.cola.clear();
        e.en_curso.as_ref().map(|c| c.ruta.clone())
    };
    if let Some(r) = ruta {
        cancelar(&r);
    }
}

/// **Lo que ensena la burbuja**: `Some(n)` mientras el PDF de ese mensaje
/// se esta aligerando (n %) o espera turno (0).
pub fn progreso_de_mensaje(mensaje: &str) -> Option<u8> {
    let (m, _) = estado();
    let e = m.lock().ok()?;
    if let Some(c) = e.en_curso.as_ref()
        && c.mensaje.as_deref() == Some(mensaje)
    {
        return Some(c.por);
    }
    e.cola
        .iter()
        .any(|p| p.mensaje.as_deref() == Some(mensaje))
        .then_some(0)
}

/// Lo acabado desde la ultima vez.
pub fn terminados() -> Vec<Terminado> {
    let (m, _) = estado();
    m.lock()
        .map(|mut e| std::mem::take(&mut e.terminados))
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// El trabajador

fn trabajador() {
    // Leer, calcular huellas y copiar el resultado tambien van a baja
    // prioridad, no solo el proceso que comprime.
    por_debajo_de_lo_normal_el_hilo();
    // Abrir el PDF aligerado con Windows para comprobarlo pide COM.
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let mut limpias: HashSet<PathBuf> = HashSet::new();
    loop {
        let (m, cv) = estado();
        let (pedido, cancelado, hijo) = {
            let Ok(mut e) = m.lock() else { return };
            let p = loop {
                // **Modo cuidadoso**: con poca memoria libre el siguiente PDF
                // no empieza; se mira otra vez cada segundo. Sigue en la
                // cola, asi que la burbuja dice «Aligerando… 0 %» y se puede
                // cancelar.
                let esperar = !e.cola.is_empty() && hay_que_esperar_a_la_memoria();
                if !esperar && let Some(p) = e.cola.pop_front() {
                    break p;
                }
                e = if esperar {
                    match cv.wait_timeout(e, std::time::Duration::from_secs(1)) {
                        Ok((e, _)) => e,
                        Err(_) => return,
                    }
                } else {
                    match cv.wait(e) {
                        Ok(e) => e,
                        Err(_) => return,
                    }
                };
            };
            let cancelado = Arc::new(AtomicBool::new(false));
            let hijo = Arc::new(Mutex::new(None));
            e.en_curso = Some(EnCurso {
                ruta: p.ruta.clone(),
                mensaje: p.mensaje.clone(),
                por: 0,
                cancelado: cancelado.clone(),
                hijo: hijo.clone(),
            });
            (p, cancelado, hijo)
        };
        // Lo que dejo a medias una sesion anterior que se cerro comprimiendo.
        if limpias.insert(pedido.raiz.clone()) {
            limpiar_temporales(&pedido.raiz);
        }
        let avance = |por: u8| {
            let cambio = m.lock().is_ok_and(|mut e| match e.en_curso.as_mut() {
                Some(c) if c.por != por => {
                    c.por = por;
                    true
                }
                _ => false,
            });
            if cambio {
                despertar();
            }
        };
        let t = std::time::Instant::now();
        // En modo cuidadoso, sin coincidir con una transcripcion de Whisper
        // (ver `turno_pesado`): los dos juntos no caben en 4 GB.
        let turno = crate::turno_pesado::tomar("aligerar PDF");
        let desenlace = procesar(&pedido, &cancelado, &hijo, &avance);
        drop(turno);
        tracing::info!(
            ruta = %pedido.ruta.display(),
            nivel = nombre_de(pedido.nivel),
            ?desenlace,
            ms = t.elapsed().as_millis() as u64,
            "PDF aligerado (o no)"
        );
        if let Ok(mut e) = m.lock() {
            e.en_curso = None;
            e.terminados.push(Terminado {
                ruta: pedido.ruta.clone(),
                ficha: pedido.ficha.clone(),
                mensaje: pedido.mensaje.clone(),
                a_mano: pedido.a_mano,
                desenlace,
            });
        }
        cv.notify_all();
        despertar();
    }
}

fn temporales(raiz: &Path) -> PathBuf {
    raiz.join("cache").join("aligerar")
}

fn limpiar_temporales(raiz: &Path) {
    if let Ok(l) = std::fs::read_dir(temporales(raiz)) {
        for f in l.flatten() {
            let _ = std::fs::remove_file(f.path());
        }
    }
}

/// Borra el temporal pase lo que pase: cancelado, fallado o ya copiado.
struct BorrarAlSalir(PathBuf);

impl Drop for BorrarAlSalir {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn parece_pdf(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(1024)]
        .windows(5)
        .any(|v| v == b"%PDF-")
}

/// Lo que salio del compresor.
#[derive(Debug)]
enum Salida {
    /// Escrito en el temporal, y mas pequeno que el original.
    Menor,
    /// pdfsqueeze no gano nada y devolvio el original.
    NoMenor,
    Cifrado,
    Cancelado,
    Fallo(String),
}

fn procesar(
    p: &Pedido,
    cancelado: &AtomicBool,
    hijo: &Mutex<Option<std::process::Child>>,
    avance: &dyn Fn(u8),
) -> Desenlace {
    let original = match std::fs::read(&p.ruta) {
        Ok(b) => b,
        Err(e) => return Desenlace::Fallo(format!("no se lee: {e}")),
    };
    if !parece_pdf(&original) {
        return Desenlace::Fallo("no es un PDF".into());
    }
    let huella = huella(&original);
    if !p.a_mano && ya_visto(&p.raiz, &huella) {
        return Desenlace::YaEstaba;
    }
    let carpeta = temporales(&p.raiz);
    if let Err(e) = std::fs::create_dir_all(&carpeta) {
        return Desenlace::Fallo(format!("sin sitio para el temporal: {e}"));
    }
    let salida = carpeta.join(format!("{}-{}.pdf", &huella[..16], marca_de_tiempo()));
    let _borrar = BorrarAlSalir(salida.clone());

    let hecho = comprimir(&p.ruta, &salida, p.nivel, p.a_mano, cancelado, hijo, avance);
    if cancelado.load(Ordering::Acquire) || matches!(hecho, Salida::Cancelado) {
        return Desenlace::Cancelado;
    }
    if matches!(hecho, Salida::Cifrado) {
        apuntar(&p.raiz, &[&huella]);
        return Desenlace::Cifrado;
    }
    let tope = (original.len() as f64 * lo_que_puede_quedar(p.nivel)) as u64;
    let cabe = |r: &Path| std::fs::metadata(r).is_ok_and(|m| m.len() > 0 && m.len() <= tope);
    let mut vale = matches!(hecho, Salida::Menor) && cabe(&salida);
    // **Plan B, como el movil**: el aligerar propio (solo fotos JPEG). Sin
    // perdida no: el plan B recodifica fotos.
    if !vale && p.nivel != NivelPdf::SinPerdida {
        let _ = std::fs::remove_file(&salida);
        vale = matches!(
            pixpin_pdf::aligerar::aligerar(&p.ruta, &salida),
            Ok(pixpin_pdf::aligerar::Resultado::Aligerado { .. })
        ) && cabe(&salida);
    }
    if cancelado.load(Ordering::Acquire) {
        return Desenlace::Cancelado;
    }
    if !vale {
        apuntar(&p.raiz, &[&huella]);
        return match hecho {
            Salida::Fallo(e) => Desenlace::Fallo(e),
            _ => Desenlace::SinMejora,
        };
    }
    // **Que Windows lo abra con las mismas paginas y lo dibuje.** Es quien lo
    // va a pintar en el chat y en el lector: lo que pdfsqueeze da por bueno
    // tiene que darlo por bueno tambien el lector de verdad.
    let paginas_antes = pixpin_pdf::Documento::abrir(&p.ruta)
        .ok()
        .map(|d| d.paginas());
    if !se_abre_y_se_dibuja(&salida, paginas_antes) {
        apuntar(&p.raiz, &[&huella]);
        return Desenlace::Fallo("el aligerado no se abre igual".into());
    }
    let Ok(ligero) = std::fs::read(&salida) else {
        return Desenlace::Fallo("el temporal desaparecio".into());
    };
    // Las copias identicas, **miradas antes de cambiar nada** (el movil,
    // igual): la copia limpia del proyecto nace del mismo fichero.
    let gemelos = gemelos(&p.ruta, &original);

    let _cambiando = CAMBIANDO.lock();
    if cancelado.load(Ordering::Acquire) {
        return Desenlace::Cancelado;
    }
    // Si mientras tanto alguien lo cambio —se anoto, llego otra version del
    // movil—, no se pisa.
    if !sigue_igual(&p.ruta, &original) {
        return Desenlace::Fallo("cambio mientras se aligeraba".into());
    }
    if let Err(e) = poner_en_su_sitio(&p.ruta, &ligero) {
        return Desenlace::Fallo(format!("no se pudo cambiar: {e}"));
    }
    for g in gemelos {
        if sigue_igual(&g, &original)
            && let Err(e) = poner_en_su_sitio(&g, &ligero)
        {
            tracing::warn!(?e, ruta = %g.display(), "copia identica que se queda pesada");
        }
    }
    apuntar(&p.raiz, &[&huella, &self::huella(&ligero)]);
    Desenlace::Aligerado {
        antes: original.len() as u64,
        despues: ligero.len() as u64,
    }
}

/// Los `.pdf` de la misma carpeta con los mismos bytes que `original`.
fn gemelos(ruta: &Path, original: &[u8]) -> Vec<PathBuf> {
    let Some(carpeta) = ruta.parent() else {
        return Vec::new();
    };
    let Ok(l) = std::fs::read_dir(carpeta) else {
        return Vec::new();
    };
    l.flatten()
        .map(|e| e.path())
        .filter(|g| {
            g.as_path() != ruta
                && g.extension().is_some_and(|x| x.eq_ignore_ascii_case("pdf"))
                && std::fs::metadata(g)
                    .is_ok_and(|m| m.is_file() && m.len() == original.len() as u64)
        })
        .filter(|g| sigue_igual(g, original))
        .collect()
}

fn sigue_igual(ruta: &Path, original: &[u8]) -> bool {
    std::fs::metadata(ruta).is_ok_and(|m| m.len() == original.len() as u64)
        && std::fs::read(ruta).is_ok_and(|b| b == original)
}

/// Escribe al lado y cambia de un golpe: quien lo abre en ese instante ve el
/// de antes o el nuevo, nunca uno a medias. Si el fichero esta abierto sin
/// dejar sustituirlo, se reintenta un momento y si no, se deja el de antes.
fn poner_en_su_sitio(destino: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut nombre = destino.file_name().unwrap_or_default().to_os_string();
    nombre.push(".ligero");
    let temporal = destino.with_file_name(nombre);
    let _borrar = BorrarAlSalir(temporal.clone());
    std::fs::write(&temporal, bytes)?;
    let mut intento = 0;
    loop {
        match std::fs::rename(&temporal, destino) {
            Ok(()) => return Ok(()),
            Err(e) if intento >= 4 => return Err(e),
            Err(_) => {
                intento += 1;
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
        }
    }
}

/// Se abre, tiene las mismas paginas (si las del original se sabian) y se
/// dejan dibujar la primera y la ultima, que son las dos puntas del arbol de
/// paginas y ejercitan fuentes y recursos. `PageCount` solo mira el catalogo.
fn se_abre_y_se_dibuja(ruta: &Path, paginas_antes: Option<u32>) -> bool {
    let Ok(d) = pixpin_pdf::Documento::abrir(ruta) else {
        return false;
    };
    let n = d.paginas();
    if n == 0 || paginas_antes.is_some_and(|a| a != n) {
        return false;
    }
    d.renderizar(0, 64).is_ok() && (n == 1 || d.renderizar(n - 1, 64).is_ok())
}

fn marca_de_tiempo() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Lo ya visto, por huella

fn huella(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn fichero_de_vistos(raiz: &Path) -> PathBuf {
    raiz.join("cache").join("pdf-aligerados.txt")
}

fn ya_visto(raiz: &Path, huella: &str) -> bool {
    std::fs::read_to_string(fichero_de_vistos(raiz))
        .is_ok_and(|t| t.lines().any(|l| l.trim() == huella))
}

fn apuntar(raiz: &Path, huellas: &[&str]) {
    let ruta = fichero_de_vistos(raiz);
    if let Some(p) = ruta.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let hecho = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ruta)
        .and_then(|mut f| {
            for h in huellas {
                writeln!(f, "{h}")?;
            }
            Ok(())
        });
    if let Err(e) = hecho {
        tracing::info!(
            ?e,
            "no se pudo apuntar el PDF aligerado: se podria repetir la pasada"
        );
    }
}

// ---------------------------------------------------------------------------
// El compresor: `pixpin-aligerar.exe` (la aplicacion) o el hilo (las pruebas)

/// Codigos de salida de `pixpin-aligerar.exe` (los de `pixpin_aligerar::codigo`;
/// una prueba vigila que coincidan).
mod codigo {
    pub const MENOR: i32 = 0;
    pub const NO_MENOR: i32 = 10;
    pub const CIFRADO: i32 = 11;
    pub const CANCELADO: i32 = 13;
}

/// En las pruebas no hay `pixpin-aligerar.exe` al lado del ejecutable de
/// pruebas, y alli un panico no aborta: se comprime en el propio hilo con la
/// misma funcion que usa el ejecutable.
#[cfg(test)]
static EN_EL_HILO: AtomicBool = AtomicBool::new(true);

fn comprimir(
    entrada: &Path,
    salida: &Path,
    nivel: NivelPdf,
    a_mano: bool,
    cancelado: &AtomicBool,
    hijo: &Mutex<Option<std::process::Child>>,
    avance: &dyn Fn(u8),
) -> Salida {
    #[cfg(test)]
    if EN_EL_HILO.load(Ordering::Relaxed) {
        return comprimir_en_el_hilo(entrada, salida, nivel, cancelado, avance);
    }
    comprimir_en_su_proceso(entrada, salida, nivel, a_mano, cancelado, hijo, avance)
}

fn salida_de(codigo: Option<i32>) -> Salida {
    match codigo {
        Some(codigo::MENOR) => Salida::Menor,
        Some(codigo::NO_MENOR) => Salida::NoMenor,
        Some(codigo::CIFRADO) => Salida::Cifrado,
        Some(codigo::CANCELADO) => Salida::Cancelado,
        // Un panico dentro (aborta) o cualquier otro fallo: el original se
        // queda y PixPin sigue abierto, que es a lo que se vino aqui.
        otro => Salida::Fallo(format!("el compresor acabo con {otro:?}")),
    }
}

#[cfg(test)]
fn comprimir_en_el_hilo(
    entrada: &Path,
    salida: &Path,
    nivel: NivelPdf,
    cancelado: &AtomicBool,
    avance: &dyn Fn(u8),
) -> Salida {
    let Some(perfil) = pixpin_aligerar::perfil_de(nombre_de(nivel)) else {
        return Salida::Fallo("nivel sin perfil".into());
    };
    let (tx, rx) = std::sync::mpsc::channel::<u8>();
    let r = std::thread::scope(|s| {
        // El avance llega desde los hilos de rayon; `avance` no es `Sync`,
        // asi que se le pasa por un canal a este hilo.
        let trabajo = s.spawn(|| {
            let tx = Mutex::new(tx);
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                pixpin_aligerar::comprimir(entrada, salida, perfil, hilos(), &|por| {
                    if let Ok(t) = tx.lock() {
                        let _ = t.send(por);
                    }
                    !cancelado.load(Ordering::Acquire)
                })
            }))
        });
        for por in rx {
            avance(por);
        }
        trabajo.join()
    });
    match r {
        Ok(Ok(c)) => salida_de(Some(c)),
        _ => Salida::Fallo("pdfsqueeze entro en panico".into()),
    }
}

/// `pixpin-aligerar.exe`, al lado del ejecutable que corre.
fn ejecutable_del_compresor() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join(NOMBRE_EXE)).filter(|r| r.is_file())
}

fn comprimir_en_su_proceso(
    entrada: &Path,
    salida: &Path,
    nivel: NivelPdf,
    a_mano: bool,
    cancelado: &AtomicBool,
    hijo: &Mutex<Option<std::process::Child>>,
    avance: &dyn Fn(u8),
) -> Salida {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
    let Some(exe) = ejecutable_del_compresor() else {
        // Se sigue con el plan B (ver `procesar`): el PDF se aligera igual,
        // peor. Que conste por que.
        static DICHO: AtomicBool = AtomicBool::new(false);
        if !DICHO.swap(true, Ordering::Relaxed) {
            tracing::warn!(
                "no esta {NOMBRE_EXE} junto a pixpinmax.exe: los PDF se aligeran con el plan B"
            );
        }
        return Salida::Fallo(format!("falta {NOMBRE_EXE}"));
    };
    let lanzado = Command::new(exe)
        .arg(entrada)
        .arg(salida)
        .arg(nombre_de(nivel))
        .arg(hilos().to_string())
        .arg(if a_mano { "normal" } else { "baja" })
        // La entrada estandar es el cordon: cuando PixPin muere, se cierra,
        // y el hijo se va con el.
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS)
        .spawn();
    let mut h = match lanzado {
        Ok(h) => h,
        Err(e) => return Salida::Fallo(format!("no se pudo lanzar: {e}")),
    };
    let Some(salida_hijo) = h.stdout.take() else {
        let _ = h.kill();
        return Salida::Fallo("sin salida del hijo".into());
    };
    let pid = h.id();
    if let Ok(mut g) = hijo.lock() {
        *g = Some(h);
    }
    // Una cancelacion que llego entre lanzarlo y guardarlo no lo vio.
    if cancelado.load(Ordering::Acquire)
        && let Ok(mut g) = hijo.lock()
        && let Some(h) = g.as_mut()
    {
        let _ = h.kill();
    }
    let acabado = AtomicBool::new(false);
    let estado = std::thread::scope(|s| {
        // **Modo cuidadoso**: si a Windows le falta memoria mientras
        // comprime, se congela el compresor hasta que se recupere. Congelado
        // no pide mas, y lo que ya tiene puede ir a disco.
        if CUIDADOSO.load(Ordering::Relaxed) {
            s.spawn(|| vigilar_la_memoria(pid, &acabado, cancelado));
        }
        for linea in std::io::BufReader::new(salida_hijo).lines() {
            let Ok(linea) = linea else { break };
            if let Some(por) = linea
                .strip_prefix("p ")
                .and_then(|n| n.trim().parse::<u8>().ok())
            {
                avance(por);
            } else if let Some(bytes) = linea.strip_prefix("memoria ") {
                tracing::info!(pico = bytes.trim(), "memoria pico del compresor de PDF");
            }
        }
        let estado = hijo
            .lock()
            .ok()
            .and_then(|mut g| g.take())
            .map(|mut h| h.wait());
        acabado.store(true, Ordering::Release);
        estado
    });
    match estado {
        Some(Ok(_)) if cancelado.load(Ordering::Acquire) => Salida::Cancelado,
        Some(Ok(s)) => salida_de(s.code()),
        _ => Salida::Fallo("no se supo como acabo el compresor".into()),
    }
}

/// Mira la memoria cada medio segundo y congela o descongela el compresor.
/// Se va cuando el compresor acaba; si se cancela, lo deja descongelado para
/// que matarlo no se quede a medias.
fn vigilar_la_memoria(pid: u32, acabado: &AtomicBool, cancelado: &AtomicBool) {
    let mut congelado = false;
    while !acabado.load(Ordering::Acquire) {
        let falta = !cancelado.load(Ordering::Acquire) && hay_que_esperar_a_la_memoria();
        if falta != congelado && pixpin_shell::prioridad::congelar(pid, falta) {
            congelado = falta;
            tracing::info!(congelado, "compresor de PDF: memoria del sistema");
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    if congelado {
        let _ = pixpin_shell::prioridad::congelar(pid, false);
    }
}

fn por_debajo_de_lo_normal_el_hilo() {
    let _ = pixpin_shell::prioridad::hilo_por_debajo_de_lo_normal();
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pdfsqueeze_core::testgen::{self, ImgEnc, Synth};
    use pixpin_proyecto::almacen;

    /// Deja los ajustes como antes al acabar, pase lo que pase: sin esto,
    /// las pruebas de `pdf_en_chat` que corren despues se encontrarian la
    /// cola encendida.
    struct Configurado;
    impl Configurado {
        fn con(al_entrar: bool, nivel: NivelPdf) -> Self {
            configurar(&Pdf {
                aligerar_al_entrar: al_entrar,
                nivel,
            });
            Configurado
        }
    }
    impl Drop for Configurado {
        fn drop(&mut self) {
            if let Ok(mut a) = AJUSTES.lock() {
                *a = None;
            }
        }
    }

    fn raiz(etiqueta: &str) -> PathBuf {
        let r =
            std::env::temp_dir().join(format!("pixpin-aligerar-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(r.join("archivos")).unwrap();
        r
    }

    /// Un PDF pesado de verdad: fotos de 1600x1200 en JPEG al 92 % dibujadas
    /// en dos pulgadas por pulgada y media, o sea a unos 800 ppp. Equilibrado
    /// las baja a 200 ppp y el fichero se queda en una fraccion.
    fn pesado(paginas: usize) -> Vec<u8> {
        let mut s = Synth::new();
        for i in 0..paginas {
            let img = testgen::photo_seeded(1600, 1200, i as u64 + 7);
            s.image_page(&img, ImgEnc::Jpeg(92), 50.0, 400.0, 144.0, 108.0);
        }
        s.finish()
    }

    fn paginas(ruta: &Path) -> u32 {
        pixpin_pdf::Documento::abrir(ruta).unwrap().paginas()
    }

    /// Espera a que `ruta` acabe y devuelve como acabo.
    fn desenlace_de(ruta: &Path) -> Desenlace {
        esperar(ruta);
        let hasta = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            if let Some(t) = terminados().into_iter().rev().find(|t| t.ruta == ruta) {
                return t.desenlace;
            }
            assert!(std::time::Instant::now() < hasta, "no acabo nunca");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    fn sobras(carpeta: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(carpeta)
            .map(|l| {
                l.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.to_string_lossy().ends_with(".ligero"))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn un_pdf_pesado_que_entra_se_aligera_en_su_sitio_y_sus_gemelos_con_el() {
        let _c = Configurado::con(true, NivelPdf::Equilibrado);
        let r = raiz("pesado");
        let ruta = r.join("archivos").join("escaneo.pdf");
        let gemelo = r.join("archivos").join("limpio-1.pdf");
        let otro = r.join("archivos").join("otro.pdf");
        let original = pesado(2);
        std::fs::write(&ruta, &original).unwrap();
        std::fs::write(&gemelo, &original).unwrap();
        // Caso negativo: un PDF distinto de la misma carpeta no se toca.
        let distinto = pesado(1);
        std::fs::write(&otro, &distinto).unwrap();
        assert!(al_entrar(&r, "p", "m1", &ruta));
        let d = desenlace_de(&ruta);
        let Desenlace::Aligerado { antes, despues } = d else {
            panic!("tenia que aligerarse: {d:?}");
        };
        assert_eq!(antes, original.len() as u64);
        let ligero = std::fs::read(&ruta).unwrap();
        assert_eq!(despues, ligero.len() as u64);
        assert!(
            (despues as f64) <= antes as f64 * 0.85,
            "de {antes} a {despues}: no bajo lo bastante"
        );
        assert_eq!(paginas(&ruta), 2, "las paginas no cambian de numero");
        assert_eq!(
            std::fs::read(&gemelo).unwrap(),
            ligero,
            "la copia identica va a la par"
        );
        assert_eq!(std::fs::read(&otro).unwrap(), distinto);
        assert!(sobras(&r.join("archivos")).is_empty());
        // **Sin pasadas repetidas**: el ya aligerado, o una copia suya, se
        // salta sin comprimir.
        let copia = r.join("archivos").join("doc-2.pdf");
        std::fs::write(&copia, &ligero).unwrap();
        assert!(al_entrar(&r, "p", "m2", &copia));
        assert_eq!(desenlace_de(&copia), Desenlace::YaEstaba);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn soltar_un_pdf_grande_no_para_la_ventana() {
        let _c = Configurado::con(true, NivelPdf::Equilibrado);
        let r = raiz("soltar");
        let ruta = r.join("archivos").join("grande.pdf");
        std::fs::write(&ruta, pesado(6)).unwrap();
        // Lo que hace el hilo de la ventana: apuntarlo, preguntar por su
        // progreso al medir la burbuja y recoger lo terminado.
        let t = std::time::Instant::now();
        assert!(al_entrar(&r, "p", "m-grande", &ruta));
        let por = progreso_de_mensaje("m-grande");
        let _ = terminados();
        let _ = cambios();
        let tardo = t.elapsed();
        assert!(
            tardo.as_millis() < 50,
            "el hilo de la ventana se paro {tardo:?}"
        );
        assert!(
            por.is_some(),
            "la burbuja tiene que decir que se esta aligerando"
        );
        // Caso negativo: otro mensaje no dice nada.
        assert_eq!(progreso_de_mensaje("m-otro"), None);
        // Y mientras comprime, preguntar sigue siendo instantaneo.
        let mut peor = std::time::Duration::ZERO;
        while pendiente(&ruta) {
            let t = std::time::Instant::now();
            let _ = progreso_de_mensaje("m-grande");
            let _ = cambios();
            peor = peor.max(t.elapsed());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            peor.as_millis() < 50,
            "preguntar el progreso tardo {peor:?}"
        );
        assert!(matches!(desenlace_de(&ruta), Desenlace::Aligerado { .. }));
        assert_eq!(
            progreso_de_mensaje("m-grande"),
            None,
            "acabado ya no dice nada"
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn con_el_ajuste_apagado_al_entrar_no_se_comprime() {
        let _c = Configurado::con(false, NivelPdf::Extremo);
        let r = raiz("apagado");
        let ruta = r.join("archivos").join("a.pdf");
        let original = pesado(1);
        std::fs::write(&ruta, &original).unwrap();
        assert!(!al_entrar(&r, "p", "m1", &ruta));
        assert!(!pendiente(&ruta));
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(std::fs::read(&ruta).unwrap(), original);
        // Pero pedido a mano si, con el nivel de Ajustes.
        assert!(a_mano(&r, "p", "m1", &ruta));
        assert!(matches!(desenlace_de(&ruta), Desenlace::Aligerado { .. }));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn un_pdf_cifrado_se_queda_como_esta_sin_error_a_la_vista() {
        let _c = Configurado::con(true, NivelPdf::Equilibrado);
        let r = raiz("cifrado");
        let ruta = r.join("archivos").join("cifrado.pdf");
        let mut s = Synth::new();
        s.image_page(
            &testgen::photo(1600, 1200),
            ImgEnc::Jpeg(92),
            50.0,
            400.0,
            144.0,
            108.0,
        );
        let mut d = lopdf::Dictionary::new();
        d.set("Filter", "Standard");
        d.set("V", 1);
        d.set("R", 2);
        d.set("P", -4);
        d.set("O", lopdf::Object::string_literal(vec![0u8; 32]));
        d.set("U", lopdf::Object::string_literal(vec![0u8; 32]));
        let clave = s.doc.add_object(d);
        s.doc.trailer.set("Encrypt", clave);
        let original = s.finish();
        std::fs::write(&ruta, &original).unwrap();
        assert!(al_entrar(&r, "p", "m1", &ruta));
        let d = desenlace_de(&ruta);
        assert!(d == Desenlace::Cifrado, "un cifrado no se aligera: {d:?}");
        assert_eq!(std::fs::read(&ruta).unwrap(), original);
        assert!(sobras(&r.join("archivos")).is_empty());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn un_pdf_que_no_mejora_se_queda_el_original() {
        let _c = Configurado::con(true, NivelPdf::Equilibrado);
        let r = raiz("sin-mejora");
        let ruta = r.join("archivos").join("ligero.pdf");
        // Lo que ya salio de pdfsqueeze no tiene un 15 % mas que quitar.
        let (ya, _) = pdfsqueeze_core::compress(
            &pesado(1),
            &pdfsqueeze_core::Options::from_profile(pdfsqueeze_core::Profile::Extreme),
        )
        .unwrap();
        std::fs::write(&ruta, &ya).unwrap();
        let fecha = std::fs::metadata(&ruta).unwrap().modified().unwrap();
        assert!(al_entrar(&r, "p", "m1", &ruta));
        assert_eq!(desenlace_de(&ruta), Desenlace::SinMejora);
        assert_eq!(std::fs::read(&ruta).unwrap(), ya);
        assert_eq!(
            std::fs::metadata(&ruta).unwrap().modified().unwrap(),
            fecha,
            "ni se reescribe con lo mismo"
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn cancelar_a_mitad_no_deja_ficheros_rotos_ni_temporales() {
        let _c = Configurado::con(true, NivelPdf::Equilibrado);
        let r = raiz("cancelar");
        let ruta = r.join("archivos").join("largo.pdf");
        let original = pesado(12);
        std::fs::write(&ruta, &original).unwrap();
        assert!(al_entrar(&r, "p", "m-largo", &ruta));
        // A que empiece de verdad (que haya pasado de cargar).
        let hasta = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while progreso_de_mensaje("m-largo").unwrap_or(0) < 16 && std::time::Instant::now() < hasta
        {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        cancelar(&ruta);
        assert_eq!(desenlace_de(&ruta), Desenlace::Cancelado);
        assert_eq!(
            std::fs::read(&ruta).unwrap(),
            original,
            "el original, entero"
        );
        assert!(sobras(&r.join("archivos")).is_empty());
        let quedan = std::fs::read_dir(temporales(&r))
            .map(|l| l.count())
            .unwrap_or(0);
        assert_eq!(quedan, 0, "ni un temporal en la cache");
        // Y cancelado no se apunta como visto: se puede volver a intentar.
        assert!(!ya_visto(&r, &huella(&original)));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn cancelar_lo_que_espera_turno_lo_quita_de_la_cola() {
        let _c = Configurado::con(true, NivelPdf::Equilibrado);
        let r = raiz("cola");
        let primero = r.join("archivos").join("1.pdf");
        let segundo = r.join("archivos").join("2.pdf");
        std::fs::write(&primero, pesado(3)).unwrap();
        let original = pesado(1);
        std::fs::write(&segundo, &original).unwrap();
        assert!(al_entrar(&r, "p", "m1", &primero));
        assert!(al_entrar(&r, "p", "m2", &segundo));
        cancelar(&segundo);
        assert!(!pendiente(&segundo));
        let _ = desenlace_de(&primero);
        assert_eq!(std::fs::read(&segundo).unwrap(), original);
        let _ = std::fs::remove_dir_all(&r);
    }

    fn proyecto(etiqueta: &str) -> (PathBuf, almacen::Ficha) {
        let raiz = std::env::temp_dir().join(format!(
            "pixpin-aligerar-unir-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&raiz);
        let ficha = almacen::Ficha::nueva("Obra", 5, "PC01");
        let mut indice = almacen::Indice::default();
        indice.proyectos.push(ficha.clone());
        indice.guardar(&raiz).unwrap();
        std::fs::create_dir_all(almacen::carpeta(&raiz, &ficha.id)).unwrap();
        (raiz, ficha)
    }

    #[test]
    fn unir_mientras_se_aligera_deja_el_proyecto_con_la_version_ligera() {
        let _c = Configurado::con(true, NivelPdf::Equilibrado);
        let (raiz, ficha) = proyecto("documento");
        let original = pesado(3);
        let relativa = almacen::guardar_adjunto(&raiz, &ficha.id, "plano.pdf", &original).unwrap();
        let adjunto = almacen::carpeta(&raiz, &ficha.id).join(&relativa);
        assert!(al_entrar(&raiz, &ficha.id, "m1", &adjunto));
        // «Anadir al proyecto» enseguida, con el PDF aun comprimiendose.
        let u = crate::pdf_en_chat::unir(&raiz, &ficha, &adjunto, "m1", "plano", 1000, &|_, _| {})
            .unwrap();
        assert_eq!(u.hojas, 3, "una hoja por pagina, las mismas");
        let ligero = std::fs::read(&adjunto).unwrap();
        assert!(
            ligero.len() < original.len() * 85 / 100,
            "el adjunto ya es el ligero"
        );
        let doc = pixpin_proyecto::vista::documento_del_proyecto(&raiz, &ficha.id).unwrap();
        assert_eq!(
            std::fs::read(&doc).unwrap(),
            ligero,
            "el documento nace ligero"
        );
        let limpio = almacen::carpeta(&raiz, &ficha.id).join("archivos/limpio-1000.pdf");
        assert_eq!(
            std::fs::read(&limpio).unwrap(),
            ligero,
            "y su copia limpia tambien"
        );
        // El documento es copia del ya aligerado: la cola no lo repite.
        assert_eq!(desenlace_de(&doc), Desenlace::YaEstaba);
        assert_eq!(paginas(&doc), 3);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn unir_mientras_se_aligera_pega_detras_las_paginas_ligeras() {
        let _c = Configurado::con(true, NivelPdf::Equilibrado);
        let (raiz, ficha) = proyecto("pegar");
        // Un proyecto que ya tiene documento, pequeno, de dos paginas.
        let hoja = |c: u8| pixpin_codec::imagen::ImagenRgba {
            ancho: 20,
            alto: 30,
            pixeles: [c, 90, 200, 255].repeat(600),
        };
        let primero = pixpin_pdf::union::de_imagenes(&[hoja(9), hoja(90)]).unwrap();
        let a = almacen::carpeta(&raiz, &ficha.id)
            .join(almacen::guardar_adjunto(&raiz, &ficha.id, "a.pdf", &primero).unwrap());
        crate::pdf_en_chat::unir(&raiz, &ficha, &a, "m1", "a", 1000, &|_, _| {}).unwrap();
        let doc = pixpin_proyecto::vista::documento_del_proyecto(&raiz, &ficha.id).unwrap();
        let antes = std::fs::read(&doc).unwrap().len();
        // Entra uno pesado y se une enseguida.
        let original = pesado(2);
        let b = almacen::carpeta(&raiz, &ficha.id)
            .join(almacen::guardar_adjunto(&raiz, &ficha.id, "b.pdf", &original).unwrap());
        assert!(al_entrar(&raiz, &ficha.id, "m2", &b));
        let u = crate::pdf_en_chat::unir(&raiz, &ficha, &b, "m2", "b", 2000, &|_, _| {}).unwrap();
        assert_eq!(u.hojas, 2);
        assert_eq!(paginas(&doc), 4, "las paginas no cambian de numero");
        let despues = std::fs::read(&doc).unwrap().len();
        assert!(
            despues < antes + original.len() * 85 / 100,
            "se pegaron las paginas pesadas: {despues} bytes"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn cada_nivel_llega_al_compresor_con_su_perfil_y_su_umbral() {
        use pdfsqueeze_core::Profile;
        let perfil = |n| pixpin_aligerar::perfil_de(nombre_de(n));
        assert_eq!(perfil(NivelPdf::default()), Some(Profile::Balanced));
        assert_eq!(perfil(NivelPdf::SinPerdida), Some(Profile::Lossless));
        assert_eq!(perfil(NivelPdf::Pequeno), Some(Profile::Small));
        assert_eq!(perfil(NivelPdf::Extremo), Some(Profile::Extreme));
        assert_eq!(lo_que_puede_quedar(NivelPdf::SinPerdida), 0.98);
        assert_eq!(lo_que_puede_quedar(NivelPdf::Extremo), 0.85);
    }

    #[test]
    fn el_nombre_del_ejecutable_y_sus_codigos_son_los_del_compresor() {
        // Se copian y no se importan para no enlazar pdfsqueeze en
        // pixpinmax.exe: esta prueba es la que evita que se separen.
        assert_eq!(NOMBRE_EXE, pixpin_aligerar::NOMBRE_EXE);
        assert_eq!(codigo::MENOR, pixpin_aligerar::codigo::MENOR);
        assert_eq!(codigo::NO_MENOR, pixpin_aligerar::codigo::NO_MENOR);
        assert_eq!(codigo::CIFRADO, pixpin_aligerar::codigo::CIFRADO);
        assert_eq!(codigo::CANCELADO, pixpin_aligerar::codigo::CANCELADO);
        // Caso negativo: un codigo que no es de exito no se toma por exito.
        assert!(matches!(
            salida_de(Some(pixpin_aligerar::codigo::FALLO)),
            Salida::Fallo(_)
        ));
        assert!(matches!(salida_de(None), Salida::Fallo(_)));
    }

    #[test]
    fn al_entrar_chico_y_max_se_quedan_en_medio_y_exacto_y_medio_no_cambian() {
        assert_eq!(nivel_para_entrar(NivelPdf::Pequeno), NivelPdf::Equilibrado);
        assert_eq!(nivel_para_entrar(NivelPdf::Extremo), NivelPdf::Equilibrado);
        // Casos negativos: los dos de entrar no se tocan.
        assert_eq!(
            nivel_para_entrar(NivelPdf::SinPerdida),
            NivelPdf::SinPerdida
        );
        assert_eq!(
            nivel_para_entrar(NivelPdf::Equilibrado),
            NivelPdf::Equilibrado
        );
    }

    #[test]
    fn con_el_ajuste_en_max_al_entrar_se_usa_medio_y_a_mano_max() {
        let _c = Configurado::con(true, NivelPdf::Extremo);
        assert_eq!(nivel_al_entrar(), Some(NivelPdf::Equilibrado));
        assert_eq!(nivel_a_mano(), NivelPdf::Extremo, "a mano, el elegido");
        let _c = Configurado::con(false, NivelPdf::Pequeno);
        assert_eq!(nivel_al_entrar(), None, "apagado no entra nada");
        assert_eq!(nivel_a_mano(), NivelPdf::Pequeno);
    }

    #[test]
    fn el_modo_cuidadoso_es_para_cuatro_gigas_o_menos_o_el_nivel_ligero() {
        assert!(modo_cuidadoso(4 * GIB, false));
        assert!(
            modo_cuidadoso(3_900 * MIB, false),
            "lo que Windows cuenta de 4 GB"
        );
        assert!(
            modo_cuidadoso(16 * GIB, true),
            "Ligero manda aunque sobre RAM"
        );
        // Casos negativos.
        assert!(!modo_cuidadoso(8 * GIB, false));
        assert!(!modo_cuidadoso(4 * GIB + 1, false));
        assert!(
            !modo_cuidadoso(0, false),
            "una RAM que no se sabe no cuenta"
        );
    }

    #[test]
    fn falta_memoria_por_debajo_de_700_megas_o_del_quince_por_ciento() {
        assert!(falta_memoria(4 * GIB, 600 * MIB));
        assert!(falta_memoria(16 * GIB, 2 * GIB), "12 % de 16 GB");
        // Casos negativos.
        assert!(!falta_memoria(4 * GIB, 1200 * MIB), "29 % y mas de 700 MB");
        assert!(!falta_memoria(16 * GIB, 3 * GIB));
    }

    #[test]
    fn en_modo_cuidadoso_un_solo_hilo_y_si_no_la_mitad_de_los_nucleos() {
        assert_eq!(hilos_para(true, 8), 1);
        assert_eq!(hilos_para(true, 1), 1);
        assert_eq!(hilos_para(false, 8), 4);
        assert_eq!(hilos_para(false, 4), 2, "el i3 de la maquina suelo");
        assert_eq!(hilos_para(false, 1), 1, "nunca cero");
    }

    /// Modo cuidadoso y memoria fingida, deshechos al acabar pase lo que pase.
    struct Equipo;
    impl Equipo {
        fn cuidadoso_con(memoria: (u64, u64)) -> Self {
            CUIDADOSO.store(true, Ordering::Relaxed);
            *MEMORIA_FINGIDA.lock().unwrap() = Some(memoria);
            Equipo
        }
    }
    impl Drop for Equipo {
        fn drop(&mut self) {
            CUIDADOSO.store(false, Ordering::Relaxed);
            if let Ok(mut m) = MEMORIA_FINGIDA.lock() {
                *m = None;
            }
        }
    }

    #[test]
    fn con_poca_memoria_en_modo_cuidadoso_el_pdf_no_empieza_hasta_que_se_recupera() {
        let _c = Configurado::con(true, NivelPdf::Equilibrado);
        let _e = Equipo::cuidadoso_con((4 * GIB, 300 * MIB));
        let r = raiz("poca-memoria");
        let ruta = r.join("archivos").join("espera.pdf");
        let original = pesado(1);
        std::fs::write(&ruta, &original).unwrap();
        assert!(al_entrar(&r, "p", "m-espera", &ruta));
        std::thread::sleep(std::time::Duration::from_millis(1500));
        assert!(pendiente(&ruta), "sigue esperando turno");
        assert_eq!(progreso_de_mensaje("m-espera"), Some(0));
        assert_eq!(std::fs::read(&ruta).unwrap(), original);
        // Windows recupera memoria: empieza solo.
        *MEMORIA_FINGIDA.lock().unwrap() = Some((4 * GIB, 2 * GIB));
        assert!(matches!(desenlace_de(&ruta), Desenlace::Aligerado { .. }));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn fuera_del_modo_cuidadoso_la_poca_memoria_no_para_nada() {
        let _c = Configurado::con(true, NivelPdf::Equilibrado);
        let _e = Equipo::cuidadoso_con((16 * GIB, 300 * MIB));
        CUIDADOSO.store(false, Ordering::Relaxed);
        let r = raiz("sin-cuidado");
        let ruta = r.join("archivos").join("sigue.pdf");
        std::fs::write(&ruta, pesado(1)).unwrap();
        assert!(al_entrar(&r, "p", "m1", &ruta));
        assert!(matches!(desenlace_de(&ruta), Desenlace::Aligerado { .. }));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn sin_el_compresor_al_lado_se_aligera_igual_con_el_plan_b() {
        // Junto al ejecutable de pruebas no hay pixpin-aligerar.exe: es lo
        // que pasa si la instalacion lo pierde.
        struct EnSuProceso;
        impl Drop for EnSuProceso {
            fn drop(&mut self) {
                EN_EL_HILO.store(true, Ordering::Relaxed);
            }
        }
        assert!(ejecutable_del_compresor().is_none());
        EN_EL_HILO.store(false, Ordering::Relaxed);
        let _p = EnSuProceso;
        let _c = Configurado::con(true, NivelPdf::Equilibrado);
        let r = raiz("plan-b");
        let ruta = r.join("archivos").join("planb.pdf");
        // Con la tabla xref clasica: el plan B no entiende las de flujo, que
        // es lo que lopdf escribe por defecto.
        let mut s = Synth::new();
        s.doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
        // Mas ancha que un A4 a 200 ppp: el plan B mide contra la pagina.
        s.image_page(
            &testgen::photo(3000, 2250),
            ImgEnc::Jpeg(92),
            0.0,
            0.0,
            612.0,
            459.0,
        );
        let original = s.finish();
        std::fs::write(&ruta, &original).unwrap();
        assert!(al_entrar(&r, "p", "m1", &ruta));
        let d = desenlace_de(&ruta);
        assert!(
            matches!(d, Desenlace::Aligerado { .. }),
            "el plan B baja las fotos: {d:?}"
        );
        assert!(std::fs::read(&ruta).unwrap().len() < original.len() * 85 / 100);
        assert_eq!(paginas(&ruta), 1);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn un_pdf_abierto_en_el_lector_mientras_se_aligera_se_cambia_igual() {
        // El lector (Windows.Data.Pdf) lo tiene abierto mientras tanto: el
        // cambio de golpe tiene que poder con eso o, si no, dejar el original.
        let _c = Configurado::con(true, NivelPdf::Equilibrado);
        let r = raiz("abierto");
        let ruta = r.join("archivos").join("abierto.pdf");
        let original = pesado(2);
        std::fs::write(&ruta, &original).unwrap();
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let lector = pixpin_pdf::Documento::abrir(&ruta).unwrap();
        assert!(lector.renderizar(0, 64).is_ok());
        assert!(al_entrar(&r, "p", "m1", &ruta));
        let d = desenlace_de(&ruta);
        let ahora = std::fs::read(&ruta).unwrap();
        match d {
            Desenlace::Aligerado { despues, .. } => assert_eq!(ahora.len() as u64, despues),
            _ => assert_eq!(ahora, original, "si no se pudo, el original entero"),
        }
        // El lector sigue pintando lo que tenia abierto.
        assert!(lector.renderizar(1, 64).is_ok());
        assert!(sobras(&r.join("archivos")).is_empty());
        drop(lector);
        let _ = std::fs::remove_dir_all(&r);
    }
}
