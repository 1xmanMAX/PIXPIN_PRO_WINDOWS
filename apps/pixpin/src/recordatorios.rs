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
use std::sync::{Arc, Condvar, Mutex};
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
    Ok(Agenda::de_cuaderno(&Cuaderno::leer_de(carpeta)?))
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

    #[test]
    fn soltar_el_vigia_para_el_hilo_sin_colgarse() {
        // Con algo pendiente a un minuto vista: el Drop tiene que despertar al
        // hilo, no esperar a que venza.
        let mut a = Agenda::nueva();
        a.programar(rec("m1", i64::MAX));
        let v = Vigia::con_reloj(a, Arc::new(|| 0), |_| {});
        drop(v);
    }
}
