//! «A las diez, recuerdame esto»: la hora que se le pone a un mensaje del
//! cuaderno y el hilo que despierta cuando llega.
//!
//! ## Donde vive el dato: en el mensaje, y en ninguna parte mas
//!
//! El movil lo guarda en UN campo, `Mensaje.recuerdaEn: Long?`
//! (`guardados/Mensajes.kt:283`), milisegundos desde 1970. **No hay fichero de
//! alarmas**, ni aqui ni alli. Eso es lo que hace que un recordatorio puesto en
//! el movil aparezca puesto en el PC en cuanto se sincroniza, sin inventar un
//! segundo almacen que habria que sincronizar aparte.
//!
//! El `Mensaje` de Rust no declara ese campo, asi que viaja en
//! `#[serde(flatten)] resto` (`cuaderno.rs:150-153`), igual que `picos` o
//! `vieneDe`. Por eso se lee y se escribe con [`hora_de`] y [`poner_hora`] y no
//! a mano: el nombre del campo esta escrito en un unico sitio, [`CAMPO`].
//!
//! ## Por que aqui basta un hilo y en Android hacia falta `AlarmManager`
//!
//! Android usa `setAlarmClock` (`pin/Recordatorios.kt:35-46`) por una sola
//! razon, que su propio comentario explica: **el sistema mata la aplicacion**
//! cuando necesita memoria, y un contador dentro del proceso muere con el. En
//! Windows no hay Doze ni matanza por memoria: mientras PixPin Max corra —y
//! corre siempre, es el de la bandeja— un hilo dormido hasta el instante
//! objetivo es exacto y no cuesta nada. Lo que Windows **no** da es que suene
//! con el programa cerrado; eso queda fuera, ver el final de este comentario.
//!
//! ## Las horas van en UTC
//!
//! `cuando_utc_ms` es UTC, como [`pixpin_shell::entorno::ahora_utc_ms`]. Es lo
//! mismo que guarda el movil y es lo unico que sobrevive a cambiar de huso o a
//! que entre el horario de verano entre que se pone el recordatorio y llega la
//! hora. A local se pasa **solo al pintar**, con
//! [`pixpin_shell::entorno::a_local`]; y lo que el usuario teclea en un reloj
//! local se pasa a UTC con [`de_local_a_utc`] antes de guardarlo.
//!
//! ## Lo que se queda fuera, a proposito
//!
//! - **Que salte con el programa cerrado.** Haria falta una tarea programada
//!   del sistema (`schtasks`) o un toast programado con AppUserModelID
//!   registrado, y para lo segundo hace falta instalador (ver
//!   `pixpin_shell::aviso`). Al arrancar, [`Agenda::de_cuaderno`] recoge todo lo
//!   que vencio mientras el programa estuvo cerrado y lo saca de golpe: llegar
//!   tarde es mucho mejor que no llegar, que es la misma decision que toma
//!   Android cuando no le dan permiso de alarma exacta.
//! - **Sacar el pin y el globo.** Este modulo no dibuja nada y no toca Windows:
//!   avisa por una llamada de vuelta. Quien la engancha es la pantalla.

use pixpin_proyecto::cuaderno::{self, Cuaderno, Mensaje};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

/// El nombre del campo en el JSONL. Es el de Android y **no se traduce ni se
/// renombra**: es un dato guardado que ya viaja entre aparatos.
pub const CAMPO: &str = "recuerdaEn";

/// Cuanto duerme el hilo como mucho de una vez cuando no hay nada pendiente.
///
/// No es que despertarse cueste algo: es que el reloj del sistema puede saltar
/// (ajuste de hora, volver de suspension, horario de verano) y un hilo dormido
/// tres dias no se enteraria. Despertando cada minuto, lo peor que pasa tras un
/// salto de reloj es un minuto de retraso.
const SIESTA_MAXIMA: Duration = Duration::from_secs(60);

/// Un recordatorio pendiente: que mensaje, a que hora y que hay que decir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recordatorio {
    /// El `id` del mensaje del cuaderno al que se le puso la hora.
    pub id: String,
    /// Milisegundos desde 1970, **en UTC**.
    pub cuando_utc_ms: i64,
    /// Lo que se enseña al vencer. Sale de `Mensaje::resumen()`, que ya
    /// prefiere la transcripcion al texto y el nombre al vacio.
    pub texto: String,
}

/// La hora que tiene puesta un mensaje, si tiene alguna.
///
/// Se descarta lo que no sea un entero: el campo lo escribe otro aparato y una
/// cadena o un `null` ahi significan «no hay hora», no un fallo del que avisar.
pub fn hora_de(m: &Mensaje) -> Option<i64> {
    m.resto.get(CAMPO)?.as_i64()
}

/// Le pone (o le cambia) la hora a un mensaje. No escribe en disco.
pub fn poner_hora(m: &mut Mensaje, cuando_utc_ms: i64) {
    m.resto
        .insert(CAMPO.to_string(), serde_json::Value::from(cuando_utc_ms));
}

/// Le quita la hora.
///
/// Se **borra la clave** en vez de dejarla a `null` para que el mensaje vuelva
/// a ser byte a byte lo que era antes de ponerla. El resumen de sincronizacion
/// es un hash del JSON canonico (`pixpin-sincro/src/kotlin.rs:3-10`): dejar un
/// `"recuerdaEn": null` colgando cambiaria ese hash y el mensaje viajaria
/// entero en cada vuelta sin que nada haya cambiado de verdad.
pub fn quitar_hora(m: &mut Mensaje) {
    m.resto.remove(CAMPO);
}

/// El recordatorio de un mensaje, si lo tiene.
pub fn de_un_mensaje(m: &Mensaje) -> Option<Recordatorio> {
    Some(Recordatorio {
        id: m.id.clone(),
        cuando_utc_ms: hora_de(m)?,
        texto: m.resumen(),
    })
}

/// Todos los recordatorios de un cuaderno, **del mas proximo al mas lejano**.
///
/// Esto es lo que hace que sobrevivan a cerrar y abrir el programa: no se lee
/// ningun estado guardado aparte, se vuelve a mirar el cuaderno.
pub fn del_cuaderno(c: &Cuaderno) -> Vec<Recordatorio> {
    let mut v: Vec<Recordatorio> = c.mensajes.iter().filter_map(de_un_mensaje).collect();
    v.sort_by(|a, b| {
        a.cuando_utc_ms
            .cmp(&b.cuando_utc_ms)
            .then_with(|| a.id.cmp(&b.id))
    });
    v
}

/// Escribe en el cuaderno la hora del mensaje `id` (o se la quita con `None`).
///
/// Devuelve si encontro el mensaje. Va por [`cuaderno::reemplazar`], que **copia
/// tal cual las lineas que no entiende**: poner una hora no puede ser la forma
/// de perder lo que escribio una version mas nueva del movil.
pub fn guardar(
    carpeta: &std::path::Path,
    id: &str,
    cuando_utc_ms: Option<i64>,
) -> std::io::Result<bool> {
    let c = Cuaderno::leer_de(carpeta)?;
    let Some(m) = c.mensajes.iter().find(|m| m.id == id) else {
        return Ok(false);
    };
    let mut m = m.clone();
    match cuando_utc_ms {
        Some(t) => poner_hora(&mut m, t),
        None => quitar_hora(&mut m),
    }
    cuaderno::reemplazar(carpeta, &m)
}

/// Pasa a UTC una hora leida en el reloj de la pared.
///
/// Es lo que hay que aplicar a lo que el usuario teclea («manana a las 8»)
/// antes de guardarlo. Al reves —para pintar— se usa
/// [`pixpin_shell::entorno::a_local`].
pub fn de_local_a_utc(local_ms: i64) -> i64 {
    local_ms - pixpin_shell::entorno::desfase_local_ms()
}

/// Lo que hay puesto, ordenado por hora.
///
/// Es una lista y no un monton ordenado (`BinaryHeap`) porque hacen falta las
/// dos cosas que un monton no da: enseñarla entera al usuario y **cancelar por
/// identificador**. Con la docena larga de recordatorios que puede tener una
/// conversacion, buscar en una lista es mas rapido que el monton.
#[derive(Debug, Clone, Default)]
pub struct Agenda {
    /// Siempre ordenada: el primero es el que vence antes.
    pendientes: Vec<Recordatorio>,
}

impl Agenda {
    pub fn nueva() -> Agenda {
        Agenda::default()
    }

    /// La agenda que sale de un cuaderno recien leido, tal cual se hace al
    /// arrancar el programa.
    pub fn de_cuaderno(c: &Cuaderno) -> Agenda {
        Agenda {
            pendientes: del_cuaderno(c),
        }
    }

    /// Pone uno, o **cambia** el que ya hubiera con ese `id`.
    ///
    /// Cambiar y no duplicar es lo que uno espera al corregir la hora de algo
    /// que ya habia puesto, y es lo que hace Android por construccion: su
    /// alarma se identifica por el pin (`pin/Recordatorios.kt:36-38`).
    pub fn programar(&mut self, r: Recordatorio) {
        self.pendientes.retain(|p| p.id != r.id);
        let sitio = self.pendientes.partition_point(|p| {
            (p.cuando_utc_ms, p.id.as_str()) < (r.cuando_utc_ms, r.id.as_str())
        });
        self.pendientes.insert(sitio, r);
    }

    /// Lo quita. Devuelve si habia algo que quitar.
    pub fn cancelar(&mut self, id: &str) -> bool {
        let antes = self.pendientes.len();
        self.pendientes.retain(|p| p.id != id);
        self.pendientes.len() != antes
    }

    /// Lo que queda por vencer, del mas proximo al mas lejano.
    pub fn pendientes(&self) -> &[Recordatorio] {
        &self.pendientes
    }

    /// El primero que vence, si hay alguno.
    pub fn proximo(&self) -> Option<&Recordatorio> {
        self.pendientes.first()
    }

    /// Saca de la agenda **todo** lo que ya vencio a las `ahora_utc_ms`, en
    /// orden.
    ///
    /// Que sea «todo» y no «el primero» es lo que hace que dos recordatorios
    /// puestos para el mismo instante salten los dos, y que los que vencieron
    /// con el programa cerrado salgan al arrancar en vez de quedarse ahi. Nada
    /// se pierde por llegar tarde.
    pub fn vencidos(&mut self, ahora_utc_ms: i64) -> Vec<Recordatorio> {
        let cuantos = self
            .pendientes
            .partition_point(|p| p.cuando_utc_ms <= ahora_utc_ms);
        self.pendientes.drain(..cuantos).collect()
    }

    /// Cuanto falta para el proximo, o `None` si no hay ninguno.
    ///
    /// Cero si ya vencio. Se acota a [`SIESTA_MAXIMA`] para que un
    /// recordatorio puesto dentro de un mes no deje al hilo dormido un mes sin
    /// enterarse de que alguien cambio la hora del sistema.
    pub fn espera(&self, ahora_utc_ms: i64) -> Option<Duration> {
        let p = self.proximo()?;
        let falta = p.cuando_utc_ms.saturating_sub(ahora_utc_ms);
        if falta <= 0 {
            return Some(Duration::ZERO);
        }
        Some(Duration::from_millis(falta as u64).min(SIESTA_MAXIMA))
    }
}

/// De donde sale «que hora es». Se inyecta para poder probar sin esperar.
type Reloj = Arc<dyn Fn() -> i64 + Send + Sync>;

/// Lo que comparten el hilo y quien lo mando hacer.
struct Compartido {
    estado: Mutex<Estado>,
    /// Se avisa al programar, al cancelar y al parar: el hilo puede estar
    /// dormido hasta dentro de un minuto y el recordatorio nuevo puede ser para
    /// dentro de diez segundos.
    campana: Condvar,
}

struct Estado {
    agenda: Agenda,
    parar: bool,
}

/// El hilo que duerme hasta que vence algo y entonces avisa.
///
/// Al soltarlo se para el hilo y **se le espera**: un recordatorio a medio
/// entregar mientras el programa se cierra saldria contra una ventana que ya no
/// existe.
pub struct Vigia {
    compartido: Arc<Compartido>,
    hilo: Option<std::thread::JoinHandle<()>>,
}

impl Vigia {
    /// Arranca el hilo con el reloj de verdad.
    ///
    /// `al_vencer` corre **en el hilo del vigia**, no en el de la interfaz: lo
    /// que haga tiene que ser o mandar un mensaje a la ventana
    /// (`pixpin_shell::despertar`) o cosas que no toquen ventanas.
    pub fn nuevo(agenda: Agenda, al_vencer: impl FnMut(Recordatorio) + Send + 'static) -> Vigia {
        Vigia::con_reloj(
            agenda,
            Arc::new(pixpin_shell::entorno::ahora_utc_ms),
            al_vencer,
        )
    }

    /// Lo mismo con el reloj puesto a mano. Es lo que usan las pruebas para no
    /// esperar de verdad ni dejar nada programado en la maquina de nadie.
    pub fn con_reloj(
        agenda: Agenda,
        reloj: Reloj,
        mut al_vencer: impl FnMut(Recordatorio) + Send + 'static,
    ) -> Vigia {
        let compartido = Arc::new(Compartido {
            estado: Mutex::new(Estado {
                agenda,
                parar: false,
            }),
            campana: Condvar::new(),
        });
        let suyo = Arc::clone(&compartido);
        let hilo = std::thread::Builder::new()
            .name("recordatorios".into())
            .spawn(move || {
                let mut estado = suyo.estado.lock().unwrap_or_else(|e| e.into_inner());
                loop {
                    if estado.parar {
                        return;
                    }
                    let vencidos = estado.agenda.vencidos(reloj());
                    if !vencidos.is_empty() {
                        // Se suelta el candado ANTES de avisar: `al_vencer` es
                        // codigo de quien llama y puede tardar, o querer
                        // programar otro recordatorio desde dentro. Con el
                        // candado puesto eso seria un abrazo mortal.
                        drop(estado);
                        for r in vencidos {
                            al_vencer(r);
                        }
                        estado = suyo.estado.lock().unwrap_or_else(|e| e.into_inner());
                        continue;
                    }
                    let espera = estado.agenda.espera(reloj()).unwrap_or(SIESTA_MAXIMA);
                    let (otro, _) = suyo
                        .campana
                        .wait_timeout(estado, espera)
                        .unwrap_or_else(|e| e.into_inner());
                    estado = otro;
                }
            })
            .expect("el sistema no dejo crear el hilo de los recordatorios");
        Vigia {
            compartido,
            hilo: Some(hilo),
        }
    }

    /// Pone uno, o cambia el que hubiera con ese `id`.
    pub fn programar(&self, r: Recordatorio) {
        self.con_la_agenda(|a| a.programar(r));
    }

    /// Lo quita. Devuelve si habia algo que quitar.
    pub fn cancelar(&self, id: &str) -> bool {
        let mut habia = false;
        self.con_la_agenda(|a| habia = a.cancelar(id));
        habia
    }

    /// Lo que queda por vencer, del mas proximo al mas lejano.
    pub fn pendientes(&self) -> Vec<Recordatorio> {
        let estado = self
            .compartido
            .estado
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        estado.agenda.pendientes().to_vec()
    }

    /// Cambia la agenda entera. Es lo que hay que llamar cuando la
    /// sincronizacion trae mensajes nuevos: se vuelve a leer el cuaderno y se
    /// deja esto, en vez de ir adivinando altas y bajas una a una.
    pub fn refrescar(&self, agenda: Agenda) {
        self.con_la_agenda(|a| *a = agenda);
    }

    fn con_la_agenda(&self, f: impl FnOnce(&mut Agenda)) {
        {
            let mut estado = self
                .compartido
                .estado
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            f(&mut estado.agenda);
        }
        self.compartido.campana.notify_all();
    }
}

impl Drop for Vigia {
    fn drop(&mut self) {
        {
            let mut estado = self
                .compartido
                .estado
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            estado.parar = true;
        }
        self.compartido.campana.notify_all();
        if let Some(h) = self.hilo.take() {
            let _ = h.join();
        }
    }
}

/// Deja el cuaderno sin la hora del recordatorio que acaba de saltar.
///
/// Es lo que hace Android nada mas avisar (`pin/RecordatorioReceiver.kt:58`:
/// `recuerdaEn = null`), y por la misma razon: si no, la conversacion queda con
/// una alarma fantasma que ya sono y que al siguiente arranque volveria a sonar.
pub fn olvidar(carpeta: &std::path::Path, id: &str) -> std::io::Result<bool> {
    guardar(carpeta, id, None)
}

/// Los recordatorios que hay que volver a poner tras una sincronizacion o al
/// arrancar, leyendo el cuaderno de `carpeta`.
pub fn agenda_de(carpeta: &std::path::Path) -> std::io::Result<Agenda> {
    let c = Cuaderno::leer_de(carpeta)?;
    let mut agenda = Agenda::de_cuaderno(&c);
    // Los temporizadores y alarmas de las mini-apps tambien avisan. Sin esto
    // no se volverian a poner al arrancar, y releer tras una sincronizacion
    // los borraria.
    for r in crate::mini_panel::avisos_del_cuaderno(
        &c,
        pixpin_shell::entorno::ahora_utc_ms(),
        pixpin_shell::entorno::desfase_local_ms(),
    ) {
        agenda.programar(r);
    }
    Ok(agenda)
}

// ─────────────── El enganche con el resto del programa ───────────────
//
// Todo lo de arriba no sabe de ventanas, ni de donde viven los proyectos, ni
// de que hay mas de una conversacion. Lo de aqui es el cable entre ese hilo y
// las dos pantallas que lo usan: el chat, que pone y quita horas, y el bucle
// principal, que es el unico que tiene la bandeja y los pines.

/// El vigia del programa: uno solo, desde que arranca hasta que termina.
///
/// Vive en un estatico y no en una variable de `main` porque **el chat corre
/// en otro hilo** (`ventana_chat::lanzar`) y no tiene forma de alcanzar lo que
/// este en la pila del principal. Es el mismo trato que ya tienen `ABIERTA` y
/// `REFRESCAR` del chat.
///
/// No se suelta nunca, asi que su hilo muere con el proceso. Es lo correcto
/// aqui: un recordatorio que se cancela porque el programa se esta cerrando no
/// lo quiere nadie, y el hilo esta dormido.
static VIGIA: OnceLock<Vigia> = OnceLock::new();

/// De que carpeta salio cada recordatorio.
///
/// Hay una conversacion por carpeta y el `id` de un mensaje solo dice cual es
/// **dentro de la suya**; sin esto, al vencer habria que buscarlo por todos los
/// cuadernos para poder quitarle la hora. Es una lista y no un mapa por lo
/// mismo que [`Agenda`]: son pocos, y `HashMap::new` no es `const`, asi que un
/// mapa pediria envolverlo en otro `OnceLock` para nada.
static CARPETAS: Mutex<Vec<(String, PathBuf)>> = Mutex::new(Vec::new());

/// Lo que ya vencio y espera a que el bucle principal lo saque.
static VENCIDOS: Mutex<Vec<Vencido>> = Mutex::new(Vec::new());

/// Un recordatorio que acaba de vencer, con la carpeta en la que hay que
/// [`olvidar`] su hora.
///
/// `carpeta` puede faltar si el proyecto se borro entre que se puso la hora y
/// que llego: el aviso sale igual —es lo que el usuario pidio— y no hay nada
/// que reescribir.
#[derive(Debug, Clone)]
pub struct Vencido {
    pub carpeta: Option<PathBuf>,
    pub recordatorio: Recordatorio,
}

/// La agenda de **todas** las conversaciones, y de que carpeta sale cada una.
///
/// Se juntan todas en una sola agenda y no una por chat porque el vigia es uno
/// y el usuario tambien: lo que quiere es que suene lo siguiente, venga del
/// proyecto que venga. Un cuaderno ilegible se salta en vez de tumbar la
/// lista: es lo mismo que hace `Indice::leer` con un indice roto.
pub fn de_todos_los_proyectos(raiz: &Path) -> (Agenda, Vec<(String, PathBuf)>) {
    let mut agenda = Agenda::nueva();
    let mut carpetas = Vec::new();
    for ficha in pixpin_proyecto::almacen::Indice::leer(raiz).proyectos {
        let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &ficha.id);
        let Ok(suya) = agenda_de(&carpeta) else {
            continue;
        };
        for r in suya.pendientes().to_vec() {
            carpetas.push((r.id.clone(), carpeta.clone()));
            agenda.programar(r);
        }
    }
    (agenda, carpetas)
}

/// Arranca el vigia con lo que haya puesto en los cuadernos. Una vez, al
/// abrir el programa.
///
/// `hwnd` es la ventana principal **como entero**, porque `HWND` no cruza
/// hilos (igual que en [`pixpin_shell::overlay::despertar`]). Lo unico que
/// hace la llamada de vuelta es apuntar lo vencido y dar un toque a esa
/// ventana: el pin y el globo los saca su bucle, que es quien tiene la bandeja
/// y los pines. Sacarlos desde aqui seria tocar Windows desde un hilo que no
/// es el de la interfaz.
pub fn vigilar(raiz: &Path, hwnd: isize) {
    if VIGIA.get().is_some() {
        tracing::warn!("el vigia de los recordatorios ya estaba puesto");
        return;
    }
    let (agenda, carpetas) = de_todos_los_proyectos(raiz);
    tracing::info!(cuantos = agenda.pendientes().len(), "recordatorios puestos");
    anotar_carpetas(carpetas);
    let _ = VIGIA.set(Vigia::nuevo(agenda, move |r| {
        anotar_vencido(r);
        pixpin_shell::despertar(windows::Win32::Foundation::HWND(hwnd as *mut _));
    }));
}

/// Vuelve a leer todos los cuadernos y deja eso como agenda.
///
/// Es lo que hay que llamar **tras sincronizar**: un recordatorio puesto en el
/// movil llega aqui como un campo mas del mensaje, y sin esto no sonaria hasta
/// el siguiente arranque.
pub fn releer(raiz: &Path) {
    let Some(v) = VIGIA.get() else {
        return;
    };
    let (agenda, carpetas) = de_todos_los_proyectos(raiz);
    anotar_carpetas(carpetas);
    v.refrescar(agenda);
    // Queda escrito cuantos hay: es lo primero que hace falta saber el dia que
    // alguien diga «puse la hora en el movil y aqui no sono».
    tracing::info!(cuantos = v.pendientes().len(), "agenda releida");
}

/// Le da al vigia una hora recien puesta. **No escribe en disco**: eso lo hace
/// quien la pone, que es el que tiene el mensaje a la vista y lo actualiza
/// tambien en la pantalla.
pub fn programar(carpeta: &Path, id: &str, texto: &str, cuando_utc_ms: i64) {
    anotar_carpeta(id, carpeta);
    if let Some(v) = VIGIA.get() {
        v.programar(Recordatorio {
            id: id.to_string(),
            cuando_utc_ms,
            texto: texto.to_string(),
        });
    }
}

/// **Vuelve a poner la hora de la nota `id` a `cuando_utc_ms`** («Volver a
/// llamar en» de la llamada secreta, v0.98.6 del movil:
/// `Recordatorios.poner` + `recuerdaEn = cuando`): en el cuaderno, que es lo
/// que viaja y sobrevive a cerrar el programa, y en el vigia. `Ok(false)` si
/// la nota ya no esta (se borro mientras sonaba).
pub fn volver_a_poner(carpeta: &Path, id: &str, cuando_utc_ms: i64) -> std::io::Result<bool> {
    if !guardar(carpeta, id, Some(cuando_utc_ms))? {
        return Ok(false);
    }
    let texto = Cuaderno::leer_de(carpeta)?
        .mensajes
        .iter()
        .find(|m| m.id == id)
        .map(Mensaje::resumen)
        .unwrap_or_default();
    programar(carpeta, id, &texto, cuando_utc_ms);
    Ok(true)
}

/// Le quita al vigia una hora. Tampoco escribe en disco.
pub fn cancelar(id: &str) {
    if let Some(v) = VIGIA.get() {
        v.cancelar(id);
    }
}

/// Lo que vencio desde la ultima vez, y deja la cola vacia.
///
/// La vacia entera de una vez: dos recordatorios para el mismo minuto tienen
/// que salir los dos, y la ventana da una sola vuelta por los dos toques.
pub fn tomar_vencidos() -> Vec<Vencido> {
    VENCIDOS
        .lock()
        .map(|mut v| std::mem::take(&mut *v))
        .unwrap_or_default()
}

fn anotar_vencido(r: Recordatorio) {
    let vencido = Vencido {
        carpeta: carpeta_de(&r.id),
        recordatorio: r,
    };
    if let Ok(mut v) = VENCIDOS.lock() {
        v.push(vencido);
    }
}

fn anotar_carpetas(nuevas: Vec<(String, PathBuf)>) {
    if let Ok(mut g) = CARPETAS.lock() {
        *g = nuevas;
    }
}

fn anotar_carpeta(id: &str, carpeta: &Path) {
    if let Ok(mut g) = CARPETAS.lock() {
        g.retain(|(i, _)| i != id);
        g.push((id.to_string(), carpeta.to_path_buf()));
    }
}

fn carpeta_de(id: &str) -> Option<PathBuf> {
    let g = CARPETAS.lock().ok()?;
    g.iter().find(|(i, _)| i == id).map(|(_, c)| c.clone())
}

/// Las horas que se ofrecen al poner un recordatorio, **en hora local** y en
/// el orden en que salen.
///
/// Son las cuatro del movil (`MensajesActivity.atajosDeRecordatorio`) mas
/// «dentro de diez minutos», que alli no hace falta —el movil lo tienes en la
/// mano— y aqui es el caso mas comun: apartar algo que estas mirando ahora.
pub fn atajos(ahora_local_ms: i64) -> Vec<(&'static str, i64)> {
    vec![
        ("chat-recordar-10-min", ahora_local_ms + 10 * 60 * 1000),
        ("chat-recordar-1-hora", ahora_local_ms + 60 * 60 * 1000),
        ("chat-recordar-3-horas", ahora_local_ms + 3 * 60 * 60 * 1000),
        ("chat-recordar-esta-tarde", a_las(ahora_local_ms, 18, 0)),
        ("chat-recordar-manana", a_las(ahora_local_ms, 9, 1)),
    ]
}

/// «Elegir la hora…» (v0.72, el `TimePicker` del movil): la hora tecleada,
/// en hora local, como la proxima vez que el reloj la marque. Si hoy ya
/// paso, es la de manana, igual que alli.
///
/// Se aceptan las formas que salen solas al teclear una hora: `18:30`,
/// `18.30`, `1830`, `930` (las 9:30), `9` y `18` (en punto). Sin separador,
/// las dos ultimas cifras son los minutos solo si hay tres o cuatro: con dos,
/// `18` es una hora y no «un minuto dieciocho». `None` si no es una hora de
/// verdad (`25:00`, `9:75`, letras), para decirlo en vez de adivinar.
pub fn hora_escrita(escrito: &str, ahora_local_ms: i64) -> Option<i64> {
    let limpio = escrito.trim();
    // Partir por bytes mas abajo solo es seguro con ASCII; una hora no
    // lleva otra cosa.
    if !limpio.is_ascii() {
        return None;
    }
    let (h, m) = match limpio.split_once([':', '.', 'h', ' ']) {
        Some((h, m)) => (h.trim(), m.trim()),
        None if limpio.len() >= 3 => limpio.split_at(limpio.len() - 2),
        None => (limpio, "00"),
    };
    let cifras = |t: &str, maximo: usize| {
        (!t.is_empty() && t.len() <= maximo && t.bytes().all(|b| b.is_ascii_digit()))
            .then(|| t.parse::<i64>().ok())
            .flatten()
    };
    let hora = cifras(h, 2)?;
    // `18:` a medio escribir es las 18 en punto.
    let minuto = if m.is_empty() { 0 } else { cifras(m, 2)? };
    if hora > 23 || minuto > 59 {
        return None;
    }
    Some(a_las_y_minuto(ahora_local_ms, hora, minuto))
}

/// Como [`a_las`] con minutos y siempre desde hoy: la proxima vez que el
/// reloj marque `hora:minuto`.
fn a_las_y_minuto(ahora_local_ms: i64, hora: i64, minuto: i64) -> i64 {
    const DIA: i64 = 86_400_000;
    let mut cuando = ahora_local_ms.div_euclid(DIA) * DIA + hora * 3_600_000 + minuto * 60_000;
    if cuando <= ahora_local_ms {
        cuando += DIA;
    }
    cuando
}

/// La proxima vez que el reloj de la pared marque `hora` en punto, `dias`
/// despues de hoy.
///
/// Si esa hora **ya paso**, es la de manana: «esta tarde» pulsado a las ocho
/// de la noche no puede ser una hora que ya fue, que saltaria en el acto. Es
/// la misma correccion que hace el movil.
fn a_las(ahora_local_ms: i64, hora: i64, dias: i64) -> i64 {
    const DIA: i64 = 86_400_000;
    let mut cuando = (ahora_local_ms.div_euclid(DIA) + dias) * DIA + hora * 3_600_000;
    if cuando <= ahora_local_ms {
        cuando += DIA;
    }
    cuando
}

/// Una hora local escrita para que el usuario la lea: «18:00» si es de hoy y
/// «21/09 09:00» si no.
///
/// La hora sola bastaria para hoy y mentiria para manana, que es justo el caso
/// que mas se usa.
pub fn cuando_legible(local_ms: i64, ahora_local_ms: i64) -> String {
    const DIA: i64 = 86_400_000;
    let del_dia = local_ms.rem_euclid(DIA) / 1000;
    let reloj = format!("{:02}:{:02}", del_dia / 3600, (del_dia % 3600) / 60);
    if local_ms.div_euclid(DIA) == ahora_local_ms.div_euclid(DIA) {
        return reloj;
    }
    let fecha = pixpin_ui::chat::etiqueta_hora(local_ms, ahora_local_ms);
    format!("{fecha} {reloj}")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::mpsc;

    fn nota(id: &str, cuando: Option<i64>, texto: &str) -> Mensaje {
        let mut m = Mensaje {
            id: id.into(),
            texto: texto.into(),
            ..Default::default()
        };
        if let Some(t) = cuando {
            poner_hora(&mut m, t);
        }
        m
    }

    fn rec(id: &str, cuando: i64) -> Recordatorio {
        Recordatorio {
            id: id.into(),
            cuando_utc_ms: cuando,
            texto: format!("lo de {id}"),
        }
    }

    fn carpeta(nombre: &str) -> std::path::PathBuf {
        let c = std::env::temp_dir().join(format!(
            "pixpin-recordatorios-{nombre}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&c);
        std::fs::create_dir_all(&c).unwrap();
        c
    }

    #[test]
    fn la_hora_viaja_en_el_campo_que_escribe_el_movil() {
        let m = nota("m1", Some(1_758_351_600_000), "llamar al arquitecto");
        let linea = serde_json::to_string(&m).unwrap();
        assert!(
            linea.contains("\"recuerdaEn\":1758351600000"),
            "el movil lee ese nombre exacto y un entero, no una cadena: {linea}"
        );
        let vuelta: Mensaje = serde_json::from_str(&linea).unwrap();
        assert_eq!(hora_de(&vuelta), Some(1_758_351_600_000));
    }

    #[test]
    fn quitar_la_hora_deja_el_mensaje_como_estaba_y_no_un_nulo_colgando() {
        // El resumen de sincronizacion es un hash del JSON: un "recuerdaEn":
        // null de mas haria viajar el mensaje entero en cada vuelta.
        let limpio = serde_json::to_string(&nota("m1", None, "hola")).unwrap();
        let mut m = nota("m1", Some(7), "hola");
        quitar_hora(&mut m);
        assert_eq!(serde_json::to_string(&m).unwrap(), limpio);
    }

    #[test]
    fn un_recuerdaen_que_no_es_un_numero_se_lee_como_si_no_hubiera_hora() {
        let mut m = nota("m1", None, "hola");
        m.resto
            .insert(CAMPO.into(), serde_json::Value::String("manana".into()));
        assert_eq!(hora_de(&m), None, "una cadena ahi no es una hora");
        assert!(de_un_mensaje(&m).is_none());

        m.resto.insert(CAMPO.into(), serde_json::Value::Null);
        assert_eq!(hora_de(&m), None, "un nulo tampoco");
    }

    #[test]
    fn un_recordatorio_en_el_pasado_salta_ya_y_no_se_pierde() {
        let mut a = Agenda::nueva();
        a.programar(rec("viejo", 1_000));
        a.programar(rec("futuro", 9_000));

        assert_eq!(
            a.espera(5_000),
            Some(Duration::ZERO),
            "ya vencio: no espera"
        );
        let salen = a.vencidos(5_000);
        assert_eq!(salen.len(), 1);
        assert_eq!(salen[0].id, "viejo");
        assert_eq!(a.pendientes().len(), 1, "el de despues sigue puesto");
    }

    #[test]
    fn dos_para_el_mismo_instante_saltan_los_dos() {
        let mut a = Agenda::nueva();
        a.programar(rec("uno", 5_000));
        a.programar(rec("otro", 5_000));
        let salen = a.vencidos(5_000);
        assert_eq!(
            salen.len(),
            2,
            "sacar solo el primero dejaria el segundo esperando a un vencimiento que ya paso"
        );
    }

    #[test]
    fn volver_a_ponerlo_cambia_la_hora_en_vez_de_duplicarlo() {
        let mut a = Agenda::nueva();
        a.programar(rec("m1", 9_000));
        a.programar(rec("m1", 2_000));
        assert_eq!(a.pendientes().len(), 1);
        assert_eq!(a.proximo().unwrap().cuando_utc_ms, 2_000);
    }

    #[test]
    fn cancelar_algo_que_no_esta_puesto_dice_que_no_y_no_toca_lo_demas() {
        let mut a = Agenda::nueva();
        a.programar(rec("m1", 9_000));
        assert!(!a.cancelar("m9"), "no habia nada con ese id");
        assert_eq!(a.pendientes().len(), 1);
        assert!(a.cancelar("m1"));
        assert!(a.pendientes().is_empty());
        assert_eq!(a.espera(0), None, "sin nada puesto no hay nada que esperar");
    }

    #[test]
    fn una_agenda_vacia_no_pide_esperar_ni_saca_nada() {
        let mut a = Agenda::nueva();
        assert!(a.proximo().is_none());
        assert!(a.vencidos(i64::MAX).is_empty());
    }

    #[test]
    fn la_espera_no_desborda_con_una_hora_absurda() {
        let mut a = Agenda::nueva();
        a.programar(rec("lejos", i64::MAX));
        let e = a.espera(i64::MIN).expect("hay uno puesto");
        assert_eq!(e, SIESTA_MAXIMA, "se acota a la siesta, no desborda");
    }

    #[test]
    fn la_agenda_sale_del_cuaderno_ordenada_y_sin_los_que_no_tienen_hora() {
        let c = Cuaderno {
            mensajes: vec![
                nota("tarde", Some(9_000), "b"),
                nota("sinhora", None, "c"),
                nota("pronto", Some(1_000), "a"),
            ],
            ..Default::default()
        };
        let a = Agenda::de_cuaderno(&c);
        let ids: Vec<&str> = a.pendientes().iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["pronto", "tarde"]);
        assert_eq!(a.pendientes()[0].texto, "a", "el texto sale del resumen");
    }

    #[test]
    fn un_recordatorio_puesto_sobrevive_a_cerrar_y_abrir() {
        let c = carpeta("sobrevive");
        cuaderno::anadir(&c, &nota("m1", None, "llamar al arquitecto")).unwrap();
        cuaderno::anadir(&c, &nota("m2", None, "otra cosa")).unwrap();

        assert!(guardar(&c, "m1", Some(1_758_351_600_000)).unwrap());

        // «Cerrar y abrir» es exactamente esto: no queda nada en memoria, se
        // vuelve a leer el cuaderno del disco.
        let a = agenda_de(&c).unwrap();
        assert_eq!(a.pendientes().len(), 1);
        assert_eq!(a.pendientes()[0].id, "m1");
        assert_eq!(a.pendientes()[0].cuando_utc_ms, 1_758_351_600_000);
        assert_eq!(a.pendientes()[0].texto, "llamar al arquitecto");

        // Y al saltar se le quita la hora, o volveria a sonar en el siguiente
        // arranque.
        assert!(olvidar(&c, "m1").unwrap());
        assert!(agenda_de(&c).unwrap().pendientes().is_empty());

        let _ = std::fs::remove_dir_all(&c);
    }

    #[test]
    fn guardar_la_hora_no_se_lleva_por_delante_lo_que_escribio_el_movil() {
        let c = carpeta("conserva");
        // Una linea con un campo que este programa no declara: tiene que seguir
        // ahi despues de tocarle la hora.
        std::fs::write(
            c.join("guardados.jsonl"),
            "{\"id\":\"m1\",\"texto\":\"hola\",\"turnos\":[{\"quien\":\"Ana\"}]}\n",
        )
        .unwrap();

        assert!(guardar(&c, "m1", Some(4_242)).unwrap());
        let escrito = std::fs::read_to_string(c.join("guardados.jsonl")).unwrap();
        assert!(
            escrito.contains("\"turnos\""),
            "se perdio un campo ajeno: {escrito}"
        );
        assert!(escrito.contains("\"recuerdaEn\":4242"));

        let _ = std::fs::remove_dir_all(&c);
    }

    #[test]
    fn ponerle_hora_a_un_mensaje_que_no_existe_dice_que_no_en_vez_de_inventarlo() {
        let c = carpeta("nohay");
        cuaderno::anadir(&c, &nota("m1", None, "hola")).unwrap();
        assert!(
            !guardar(&c, "m9", Some(1)).unwrap(),
            "no hay mensaje m9: no puede decir que si"
        );
        let _ = std::fs::remove_dir_all(&c);
    }

    /// «Volver a llamar en 15 min» (v0.98.6): la nota, que ya sono y perdio
    /// su hora, la vuelve a tener en el campo del movil, y viaja.
    #[test]
    fn volver_a_llamar_pone_otra_vez_la_hora_en_el_campo_del_movil() {
        let c = carpeta("volver");
        cuaderno::anadir(&c, &nota("m1", None, "recado")).unwrap();
        assert!(volver_a_poner(&c, "m1", 1_758_351_600_000).unwrap());
        let cu = Cuaderno::leer_de(&c).unwrap();
        assert_eq!(hora_de(&cu.mensajes[0]), Some(1_758_351_600_000));
        // Caso negativo: una nota que ya no esta no se inventa.
        assert!(!volver_a_poner(&c, "m9", 1).unwrap());
        assert_eq!(Cuaderno::leer_de(&c).unwrap().mensajes.len(), 1);
        let _ = std::fs::remove_dir_all(&c);
    }

    #[test]
    fn el_vigia_saca_lo_ya_vencido_sin_esperar_a_nada() {
        // Reloj clavado: ni se espera de verdad ni se deja nada programado en
        // la maquina de quien ejecute las pruebas.
        let (envia, recibe) = mpsc::channel();
        let mut a = Agenda::nueva();
        a.programar(rec("viejo", 1_000));
        a.programar(rec("tambien", 1_000));
        a.programar(rec("manana", 99_000_000));

        let v = Vigia::con_reloj(a, Arc::new(|| 5_000), move |r| {
            let _ = envia.send(r.id);
        });

        let mut salieron = vec![
            recibe.recv_timeout(Duration::from_secs(5)).unwrap(),
            recibe.recv_timeout(Duration::from_secs(5)).unwrap(),
        ];
        salieron.sort();
        assert_eq!(salieron, ["tambien", "viejo"]);
        assert_eq!(
            v.pendientes().len(),
            1,
            "el de manana sigue puesto y los otros dos ya no"
        );
    }

    #[test]
    fn programar_algo_ya_vencido_con_el_vigia_dormido_lo_despierta() {
        let (envia, recibe) = mpsc::channel();
        // Sin nada puesto, el hilo se duerme la siesta entera. Si programar no
        // tocara la campana, esta prueba tardaria un minuto en pasar.
        let v = Vigia::con_reloj(Agenda::nueva(), Arc::new(|| 5_000), move |r| {
            let _ = envia.send(r.id);
        });
        v.programar(rec("ya", 1_000));
        assert_eq!(recibe.recv_timeout(Duration::from_secs(5)).unwrap(), "ya");
    }

    #[test]
    fn cancelar_antes_de_la_hora_evita_que_suene() {
        let (envia, recibe) = mpsc::channel();
        let mut a = Agenda::nueva();
        a.programar(rec("m1", 9_000));
        let v = Vigia::con_reloj(a, Arc::new(|| 5_000), move |r| {
            let _ = envia.send(r.id);
        });
        assert!(v.cancelar("m1"));
        assert!(v.pendientes().is_empty());
        assert!(
            recibe.recv_timeout(Duration::from_millis(200)).is_err(),
            "no debia sonar nada"
        );
    }

    /// Un dia entero en milisegundos, para escribir las pruebas de las horas
    /// sin contar ceros.
    const DIA: i64 = 86_400_000;

    /// Las nueve y media de la manana de un dia cualquiera, en hora local.
    const MANANA: i64 = 20_000 * DIA + 9 * 3_600_000 + 30 * 60_000;

    #[test]
    fn ninguna_de_las_horas_que_se_ofrecen_cae_en_el_pasado() {
        // A las nueve y media, a las seis y media de la tarde (con «esta
        // tarde» a punto de pasar) y a las once y media de la noche, que es
        // cuando «esta tarde» y «manana» se cruzan.
        for ahora in [MANANA, MANANA + 9 * 3_600_000, MANANA + 14 * 3_600_000] {
            for (clave, cuando) in atajos(ahora) {
                assert!(
                    cuando > ahora,
                    "«{clave}» a las {ahora} sale en el pasado ({cuando}): sonaria en el acto"
                );
            }
        }
    }

    #[test]
    fn esta_tarde_pulsado_de_noche_es_la_tarde_de_manana() {
        let noche = 20_000 * DIA + 22 * 3_600_000;
        let tarde = a_las(noche, 18, 0);
        assert_eq!(
            tarde,
            20_001 * DIA + 18 * 3_600_000,
            "las seis de hoy ya pasaron: toca las de manana"
        );
    }

    #[test]
    fn manana_por_la_manana_es_el_dia_siguiente_a_las_nueve() {
        assert_eq!(a_las(MANANA, 9, 1), 20_001 * DIA + 9 * 3_600_000);
    }

    #[test]
    fn la_hora_se_lee_con_su_fecha_cuando_no_es_de_hoy() {
        assert_eq!(cuando_legible(MANANA, MANANA - 60_000), "09:30");
        // De otro dia: sin la fecha, «09:00» pareceria de hoy.
        let manana = a_las(MANANA, 9, 1);
        assert!(
            cuando_legible(manana, MANANA).ends_with(" 09:00"),
            "{}",
            cuando_legible(manana, MANANA)
        );
    }

    #[test]
    fn la_agenda_junta_las_conversaciones_y_se_salta_las_que_no_tienen_cuaderno() {
        use pixpin_proyecto::almacen;
        let raiz = carpeta("todos");
        let uno = almacen::Ficha::nueva("Obra", 1, "pc");
        let otro = almacen::Ficha::nueva("Casa", 1, "pc");
        // Un tercero apuntado en el indice pero sin carpeta en el disco: es lo
        // que queda tras borrar un proyecto a mano, y no puede tumbar la lista.
        let fantasma = almacen::Ficha::nueva("Fantasma", 1, "pc");
        almacen::Indice {
            proyectos: vec![uno.clone(), otro.clone(), fantasma],
            ..Default::default()
        }
        .guardar(&raiz)
        .unwrap();
        let c1 = almacen::carpeta(&raiz, &uno.id);
        let c2 = almacen::carpeta(&raiz, &otro.id);
        cuaderno::anadir(&c1, &nota("m1", Some(9_000), "llamar al arquitecto")).unwrap();
        cuaderno::anadir(&c1, &nota("m2", None, "sin hora")).unwrap();
        cuaderno::anadir(&c2, &nota("m3", Some(1_000), "sacar la basura")).unwrap();

        let (agenda, carpetas) = de_todos_los_proyectos(&raiz);
        let ids: Vec<&str> = agenda.pendientes().iter().map(|r| r.id.as_str()).collect();
        assert_eq!(
            ids,
            ["m3", "m1"],
            "del mas proximo al mas lejano, mezclados"
        );
        assert_eq!(carpetas.len(), 2, "solo los que tienen hora");
        assert!(carpetas.contains(&("m3".to_string(), c2)));
        assert!(carpetas.contains(&("m1".to_string(), c1)));

        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn poner_dos_veces_la_hora_al_mismo_mensaje_no_lo_duplica_en_el_disco() {
        use pixpin_proyecto::almacen;
        let raiz = carpeta("dosveces");
        let ficha = almacen::Ficha::nueva("Obra", 1, "pc");
        almacen::Indice {
            proyectos: vec![ficha.clone()],
            ..Default::default()
        }
        .guardar(&raiz)
        .unwrap();
        let c = almacen::carpeta(&raiz, &ficha.id);
        cuaderno::anadir(&c, &nota("m1", None, "llamar")).unwrap();

        assert!(guardar(&c, "m1", Some(9_000)).unwrap());
        assert!(guardar(&c, "m1", Some(4_000)).unwrap());

        let (agenda, carpetas) = de_todos_los_proyectos(&raiz);
        assert_eq!(
            agenda.pendientes().len(),
            1,
            "cambiar la hora no anade otra"
        );
        assert_eq!(
            agenda.pendientes()[0].cuando_utc_ms,
            4_000,
            "vale la ultima"
        );
        assert_eq!(carpetas.len(), 1);

        // Y quitarla la deja sin nada que sonar.
        assert!(olvidar(&c, "m1").unwrap());
        assert!(de_todos_los_proyectos(&raiz).0.pendientes().is_empty());

        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn lo_que_vence_espera_en_la_cola_a_que_la_ventana_lo_saque() {
        // El camino de verdad, pero con el reloj clavado y sin ventana: el
        // vigia apunta, y la cola se vacia de una vez.
        let mut a = Agenda::nueva();
        a.programar(rec("ya", 1_000));
        let _v = Vigia::con_reloj(a, Arc::new(|| 5_000), anotar_vencido);
        let mut salieron = Vec::new();
        for _ in 0..500 {
            salieron = tomar_vencidos();
            if !salieron.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(salieron.len(), 1, "el vigia no llego a apuntar nada");
        assert_eq!(salieron[0].recordatorio.id, "ya");
        assert!(
            tomar_vencidos().is_empty(),
            "la cola se vacia al tomarla: si no, sonaria dos veces"
        );
    }

    #[test]
    fn programar_y_cancelar_sin_vigia_no_revienta() {
        // Es lo que pasa en las pruebas y en cualquier ventana abierta antes
        // de que `vigilar` corra: se escribe en el cuaderno y ya esta.
        let c = carpeta("sinvigia");
        programar(&c, "m1", "hola", 1_000);
        cancelar("m1");
        assert_eq!(
            carpeta_de("m1").as_deref(),
            Some(c.as_path()),
            "la carpeta queda apuntada aunque no haya vigia"
        );
        let _ = std::fs::remove_dir_all(&c);
    }

    #[test]
    fn soltar_el_vigia_para_el_hilo_sin_colgarse() {
        // Con algo pendiente a un minuto vista: el Drop tiene que despertar al
        // hilo, no esperar a que venza.
        let mut a = Agenda::nueva();
        a.programar(rec("m1", i64::MAX));
        let v = Vigia::con_reloj(a, Arc::new(|| 0), |_| {});
        drop(v);
    }

    /// Las 10:00 del 20 de septiembre de 2026, en hora local.
    const DIEZ: i64 = 1_789_898_400_000;
    const HORA: i64 = 3_600_000;
    const DIA_MS: i64 = 86_400_000;

    #[test]
    fn una_hora_tecleada_que_aun_no_ha_llegado_es_de_hoy() {
        assert_eq!(DIEZ.rem_euclid(DIA_MS), 10 * HORA, "la base son las diez");
        assert_eq!(
            hora_escrita("18:30", DIEZ),
            Some(DIEZ + 8 * HORA + 30 * 60_000)
        );
        assert_eq!(hora_escrita("18.30", DIEZ), hora_escrita("18:30", DIEZ));
        assert_eq!(hora_escrita("1830", DIEZ), hora_escrita("18:30", DIEZ));
        assert_eq!(hora_escrita(" 18h30 ", DIEZ), hora_escrita("18:30", DIEZ));
        assert_eq!(hora_escrita("18", DIEZ), Some(DIEZ + 8 * HORA), "en punto");
        assert_eq!(hora_escrita("18:", DIEZ), Some(DIEZ + 8 * HORA));
    }

    #[test]
    fn una_hora_que_ya_paso_es_la_de_manana() {
        assert_eq!(hora_escrita("930", DIEZ), Some(DIEZ - 30 * 60_000 + DIA_MS));
        assert_eq!(hora_escrita("9", DIEZ), Some(DIEZ - HORA + DIA_MS));
        // La misma hora que ahora no salta en el acto: es manana.
        assert_eq!(hora_escrita("10:00", DIEZ), Some(DIEZ + DIA_MS));
    }

    #[test]
    fn lo_que_no_es_una_hora_no_se_adivina() {
        for mal in [
            "", "25:00", "9:75", "abc", "1:2:3", "12345", "9:5x", "ñ12", "-1",
        ] {
            assert_eq!(hora_escrita(mal, DIEZ), None, "«{mal}» no es una hora");
        }
    }
}
