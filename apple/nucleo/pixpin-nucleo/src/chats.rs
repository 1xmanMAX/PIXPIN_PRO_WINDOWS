//! Los chats (proyectos) y sus mensajes, **por paginas**.
//!
//! Un chat es un proyecto de `proyectos/indice.json` y sus mensajes viven en
//! `proyectos/<id>/guardados.jsonl`, una linea de JSON por mensaje
//! (`pixpin_proyecto::cuaderno`). Aqui no se inventa nada de eso: se lee y se
//! escribe con las mismas funciones que el chat de Windows.
//!
//! # Como se lee una pagina sin leer el chat entero
//!
//! La primera vez que se pide una pagina de un chat se hace su **indice de
//! lineas**: donde empieza y cuanto mide cada mensaje visible en el fichero.
//! Para eso cada linea se mira con un lector minimo ([`Cabeza`]: solo `id` y
//! `numero`), sin construir el `Mensaje` entero ni sus cadenas. Despues,
//! cada pagina lee **solo su tramo** del fichero (una lectura contigua) y
//! convierte en `Mensaje` solo esas lineas. Pasar de pagina en un chat de
//! diez mil mensajes cuesta lo que cuestan cincuenta.
//!
//! El indice se guarda en una cache con tope ([`TOPE_DE_CHATS`]) y se da por
//! bueno mientras el fichero tenga el mismo tamano y la misma fecha; si
//! cambia (otro hilo escribio, llego algo al sincronizar), se rehace. Cada
//! entrada son 12 bytes por mensaje.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::SystemTime;

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{self, Mensaje, Sello};

use crate::{ErrorNucleo, Nucleo};

/// Cuantos indices de chat se guardan a la vez. Ocho chats son de sobra para
/// ir y volver entre los abiertos; un indice de mas solo es un fichero que
/// se vuelve a recorrer.
pub const TOPE_DE_CHATS: usize = 8;

/// Lo mas que se devuelve en una pagina, pida Swift lo que pida: una pagina
/// enorme es leer el chat entero por la puerta de atras.
pub const TOPE_DE_PAGINA: u32 = 500;

/// Un chat en la lista.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FichaChat {
    pub id: String,
    pub nombre: String,
    /// Ultima vez que se toco, en milisegundos desde 1970. Ordena la lista.
    pub tocado: i64,
    /// La ultima linea, para ensenar debajo del nombre.
    pub resumen: String,
    pub hojas: u32,
    pub sin_leer: u32,
    /// «Mensajes guardados»: va siempre primero y no se renombra.
    pub es_guardados: bool,
    /// El chat del asistente local.
    pub es_asistente: bool,
    pub archivado: bool,
}

/// Un mensaje, con lo que hace falta para pintar su burbuja.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MensajeChat {
    pub id: String,
    /// Milisegundos desde 1970.
    pub cuando: i64,
    /// `NOTA`, `IMAGEN`, `ARCHIVO`, `VOZ`, `DIBUJO`, `PAGINA`, `PROYECTO`,
    /// `MINIAPP` o la palabra que traiga una version mas nueva del movil.
    pub clase: String,
    pub texto: String,
    pub nombre: String,
    /// Fichero del mensaje, relativo a la carpeta del chat.
    pub ruta: Option<String>,
    pub bytes: i64,
    /// Lo que se ensena en una linea (la transcripcion de una nota de voz,
    /// el texto o el nombre).
    pub resumen: String,
    /// `47·K7Q2`, `47a` o nada.
    pub codigo_chat: Option<String>,
    pub responde_a: Option<String>,
    pub emoji: Option<String>,
    pub fijado: bool,
    /// El dibujo o la hoja a la que apunta (un `DIBUJO`, una `PAGINA`).
    pub referencia: Option<String>,
    pub pagina: Option<u32>,
    /// Lo que dura el audio de una nota de voz, en milisegundos (0 si no es
    /// una o no se sabe).
    pub duracion_ms: i64,
    /// El texto de una nota de voz con sus minutos (`[0:05] lo dicho`), si
    /// ya se paso a texto. Ver [`crate::voz`].
    pub transcripcion: Option<String>,
    /// Los picos de la onda de una nota de voz: enteros de 0 a 32767, como
    /// mucho 256 (el campo `picos` del movil). Vacio si no los trae.
    pub picos: Vec<i32>,
}

/// Una pagina de mensajes, del mas antiguo al mas nuevo.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaMensajes {
    pub mensajes: Vec<MensajeChat>,
    /// La posicion del primero de la pagina (0 es el mas antiguo).
    pub desde: u64,
    /// Cuantos mensajes visibles tiene el chat entero.
    pub total: u64,
    /// Lineas del cuaderno que no se entienden. Se dicen, no se callan: si
    /// faltan mensajes, el usuario tiene que saberlo.
    pub lineas_rotas: u64,
}

impl From<&almacen::Ficha> for FichaChat {
    fn from(f: &almacen::Ficha) -> Self {
        FichaChat {
            id: f.id.clone(),
            nombre: f.nombre.clone(),
            tocado: f.tocado,
            resumen: f.resumen.clone(),
            hojas: f.hojas,
            sin_leer: f.sin_leer,
            es_guardados: f.es_guardados(),
            es_asistente: f.es_asistente(),
            archivado: f
                .resto
                .get(pixpin_proyecto::vista::MARCA_ARCHIVADO)
                .and_then(|v| v.as_bool())
                == Some(true),
        }
    }
}

impl From<Mensaje> for MensajeChat {
    fn from(m: Mensaje) -> Self {
        let clase = m
            .clase
            .as_ref()
            .and_then(|c| serde_json::to_value(c).ok())
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let picos = crate::voz::picos_de(&m);
        MensajeChat {
            resumen: m.resumen(),
            codigo_chat: m.codigo_chat(),
            duracion_ms: m.duracion_ms,
            transcripcion: m.transcripcion,
            picos,
            id: m.id,
            cuando: m.cuando,
            clase,
            texto: m.texto,
            nombre: m.nombre,
            ruta: m.ruta,
            bytes: m.bytes,
            responde_a: m.responde_a,
            emoji: m.emoji,
            fijado: m.fijado,
            referencia: m.referencia,
            pagina: m.pagina,
        }
    }
}

/// Lo minimo de una linea para indexarla: serde salta el resto sin
/// construirlo.
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct Cabeza {
    id: String,
    numero: i64,
}

/// El indice de lineas de un cuaderno (ver la doc del modulo).
#[derive(Debug, Default)]
pub(crate) struct IndiceCuaderno {
    largo: u64,
    modificado: Option<SystemTime>,
    /// Inicio y largo en bytes de cada mensaje visible, en orden.
    lineas: Vec<(u64, u32)>,
    /// El mayor `numero` de todos (tambien los de sistema): el siguiente
    /// mensaje lleva este mas uno (`Cuaderno::siguiente_numero`).
    pub(crate) mayor_numero: i64,
    lineas_rotas: u64,
}

impl IndiceCuaderno {
    fn de_bytes(texto: &[u8], largo: u64, modificado: Option<SystemTime>) -> IndiceCuaderno {
        let mut ind = IndiceCuaderno {
            largo,
            modificado,
            ..Default::default()
        };
        let mut inicio = 0usize;
        while inicio < texto.len() {
            let fin = texto[inicio..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(texto.len(), |p| inicio + p);
            let linea = &texto[inicio..fin];
            if !linea.iter().all(u8::is_ascii_whitespace) {
                match serde_json::from_slice::<Cabeza>(linea) {
                    Ok(c) => {
                        ind.mayor_numero = ind.mayor_numero.max(c.numero);
                        // Los de sistema (`auto-...`) existen y viajan, pero
                        // no son conversacion: no salen como burbujas.
                        if !c.id.starts_with(cuaderno::PREFIJO_DE_SISTEMA) {
                            ind.lineas.push((inicio as u64, linea.len() as u32));
                        }
                    }
                    Err(_) => ind.lineas_rotas += 1,
                }
            }
            inicio = fin + 1;
        }
        ind
    }
}

/// Los indices de los ultimos chats leidos, el mas reciente al final.
#[derive(Debug, Default)]
pub struct Cache {
    cuadernos: Vec<(PathBuf, Arc<IndiceCuaderno>)>,
}

impl Cache {
    pub(crate) fn vaciar(&mut self) {
        self.cuadernos = Vec::new();
    }

    pub(crate) fn quitar(&mut self, fichero: &Path) {
        self.cuadernos.retain(|(r, _)| r != fichero);
    }

    /// Cuantos indices hay guardados (para las pruebas del tope).
    pub fn cuantos(&self) -> usize {
        self.cuadernos.len()
    }

    /// El indice de `fichero`, rehecho si cambio. `None` si no existe.
    pub(crate) fn indice(
        &mut self,
        fichero: &Path,
    ) -> std::io::Result<Option<Arc<IndiceCuaderno>>> {
        let meta = match std::fs::metadata(fichero) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.quitar(fichero);
                return Ok(None);
            }
            Err(e) => return Err(e),
        };
        let modificado = meta.modified().ok();
        if let Some(n) = self.cuadernos.iter().position(|(r, _)| r == fichero) {
            let (_, ind) = &self.cuadernos[n];
            if ind.largo == meta.len() && ind.modificado == modificado {
                let par = self.cuadernos.remove(n);
                let ind = Arc::clone(&par.1);
                self.cuadernos.push(par);
                return Ok(Some(ind));
            }
            self.cuadernos.remove(n);
        }
        let texto = std::fs::read(fichero)?;
        let ind = Arc::new(IndiceCuaderno::de_bytes(
            &texto,
            texto.len() as u64,
            modificado,
        ));
        if self.cuadernos.len() >= TOPE_DE_CHATS {
            self.cuadernos.remove(0);
        }
        self.cuadernos
            .push((fichero.to_path_buf(), Arc::clone(&ind)));
        Ok(Some(ind))
    }
}

/// Un id de chat que no puede salir de `proyectos/`: viene de Swift, y un
/// `../` no puede decidir donde se lee o se escribe.
pub(crate) fn validar_chat(chat: &str) -> Result<(), ErrorNucleo> {
    if chat.is_empty() || chat == "." || chat == ".." || chat.contains(['/', '\\', '\0']) {
        return Err(ErrorNucleo::Argumento {
            detalle: format!("id de chat no valido: {chat:?}"),
        });
    }
    Ok(())
}

/// Las lineas `[desde, desde+cuantos)` del indice, ya como mensajes: una sola
/// lectura del tramo del fichero que las contiene.
fn leer_tramo(
    fichero: &Path,
    ind: &IndiceCuaderno,
    desde: usize,
    hasta: usize,
) -> std::io::Result<Vec<MensajeChat>> {
    if desde >= hasta {
        return Ok(Vec::new());
    }
    let (inicio, _) = ind.lineas[desde];
    let (ultimo, largo) = ind.lineas[hasta - 1];
    let fin = ultimo + u64::from(largo);
    let mut f = std::fs::File::open(fichero)?;
    f.seek(SeekFrom::Start(inicio))?;
    let mut tramo = vec![0u8; (fin - inicio) as usize];
    f.read_exact(&mut tramo)?;
    Ok(ind.lineas[desde..hasta]
        .iter()
        .filter_map(|&(i, l)| {
            let a = (i - inicio) as usize;
            serde_json::from_slice::<Mensaje>(&tramo[a..a + l as usize]).ok()
        })
        .map(MensajeChat::from)
        .collect())
}

/// La hora de un mensaje nuevo: la de ahora, pero siempre mayor que la del
/// anterior de este proceso. El id de una nota ES su hora en milisegundos
/// (`Mensaje::nota`), y dos notas en el mismo milisegundo chocarian.
pub(crate) fn hora_unica() -> i64 {
    static ULTIMA: AtomicI64 = AtomicI64::new(0);
    let ahora = crate::ahora_ms();
    let mut previa = ULTIMA.load(Ordering::SeqCst);
    loop {
        let nueva = ahora.max(previa + 1);
        match ULTIMA.compare_exchange(previa, nueva, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return nueva,
            Err(otra) => previa = otra,
        }
    }
}

#[uniffi::export]
impl Nucleo {
    /// Los chats, en el orden de la lista (lo ultimo tocado arriba y
    /// «Mensajes guardados» primero). Es el indice, pequeno: no se abre
    /// ningun proyecto.
    pub fn chats(&self) -> Result<Vec<FichaChat>, ErrorNucleo> {
        let indice = Indice::leer_si_esta(self.ruta())?;
        Ok(indice
            .ordenadas()
            .into_iter()
            .map(FichaChat::from)
            .collect())
    }

    /// Crea un chat vacio. No crea su carpeta: eso lo hace el primer
    /// mensaje, como en Windows.
    pub fn crear_chat(&self, nombre: String) -> Result<FichaChat, ErrorNucleo> {
        let aparato = self.identidad()?.yo.codigo();
        let ficha = almacen::Ficha::nueva(&nombre, crate::ahora_ms(), &aparato);
        // `leer_si_esta`: se reescribe la lista entera, y una que no se pudo
        // leer no puede pasar por vacia.
        let mut indice = Indice::leer_si_esta(self.ruta())?;
        indice.proyectos.push(ficha.clone());
        indice.guardar(self.ruta())?;
        Ok(FichaChat::from(&ficha))
    }

    /// «Mensajes guardados», creandolo si no esta.
    pub fn chat_guardados(&self) -> Result<FichaChat, ErrorNucleo> {
        let aparato = self.identidad()?.yo.codigo();
        let f = almacen::asegurar_guardados(self.ruta(), crate::ahora_ms(), &aparato)?;
        Ok(FichaChat::from(&f))
    }

    /// Una pagina de mensajes contando desde el mas antiguo: `desde` es la
    /// posicion del primero (0 el mas antiguo) y `cuantos` el largo (hasta
    /// [`TOPE_DE_PAGINA`]). Un chat sin nada escrito da una pagina vacia.
    pub fn mensajes(
        &self,
        chat: String,
        desde: u64,
        cuantos: u32,
    ) -> Result<PaginaMensajes, ErrorNucleo> {
        self.pagina(&chat, Some(desde), cuantos)
    }

    /// La ultima pagina: los `cuantos` mensajes mas nuevos. Es la que se pide
    /// al abrir un chat; las anteriores, con [`Nucleo::mensajes`] y el
    /// `desde` de esta.
    pub fn ultimos_mensajes(
        &self,
        chat: String,
        cuantos: u32,
    ) -> Result<PaginaMensajes, ErrorNucleo> {
        self.pagina(&chat, None, cuantos)
    }

    /// Escribe una nota en un chat: **una linea al final** de su cuaderno,
    /// sellada con el codigo de este aparato y el siguiente numero, y sube
    /// el chat en la lista. Igual que la caja de escribir de Windows
    /// (`ventana_chat::escribir_nota`).
    pub fn escribir_nota(
        &self,
        chat: String,
        texto: String,
        responde_a: Option<String>,
    ) -> Result<MensajeChat, ErrorNucleo> {
        validar_chat(&chat)?;
        let raiz = self.ruta();
        let mut indice = Indice::leer_si_esta(raiz)?;
        if indice.buscar(&chat).is_none() {
            return Err(ErrorNucleo::NoExiste { que: chat });
        }
        let aparato = self.identidad()?.yo.codigo();
        let carpeta = almacen::carpeta(raiz, &chat);
        let fichero = carpeta.join("guardados.jsonl");
        // Con la cache tomada de principio a fin: dos notas a la vez en este
        // Nucleo no pueden sacar el mismo numero.
        let mut cache = self.cache_tomada();
        let numero = cache.indice(&fichero)?.map_or(0, |i| i.mayor_numero) + 1;
        let cuando = hora_unica();
        let mut m = Mensaje::nota(
            &texto,
            &Sello {
                cuando,
                numero,
                aparato,
                proyecto: chat.clone(),
            },
        );
        m.responde_a = responde_a;
        cuaderno::anadir(&carpeta, &m)?;
        cache.quitar(&fichero);
        drop(cache);
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == chat) {
            f.tocado = cuando;
            f.resumen = m.resumen();
        }
        indice.guardar(raiz)?;
        Ok(MensajeChat::from(m))
    }
}

impl Nucleo {
    fn pagina(
        &self,
        chat: &str,
        desde: Option<u64>,
        cuantos: u32,
    ) -> Result<PaginaMensajes, ErrorNucleo> {
        validar_chat(chat)?;
        let fichero = almacen::carpeta(self.ruta(), chat).join("guardados.jsonl");
        let ind = self.cache_tomada().indice(&fichero)?;
        let Some(ind) = ind else {
            return Ok(PaginaMensajes {
                mensajes: Vec::new(),
                desde: 0,
                total: 0,
                lineas_rotas: 0,
            });
        };
        let total = ind.lineas.len();
        let cuantos = cuantos.min(TOPE_DE_PAGINA) as usize;
        let desde = match desde {
            Some(d) => (d.min(total as u64)) as usize,
            None => total.saturating_sub(cuantos),
        };
        let hasta = (desde + cuantos).min(total);
        let mensajes = leer_tramo(&fichero, &ind, desde, hasta)?;
        Ok(PaginaMensajes {
            mensajes,
            desde: desde as u64,
            total: total as u64,
            lineas_rotas: ind.lineas_rotas,
        })
    }
}
